//! Patch 127's runtime (the profiler is `patches::memory_profile`,
//! `docs/notes/memory-profile.md`): the variable that switches it on, its
//! gates written into dlmalloc's and ptmalloc3's code only then
//! ([`start`]: an off game runs the original allocator byte for byte),
//! room of our own for the profiler's tables, the snapshot key, and what a
//! gate's call tells, read once here into typed values:
//!
//! - a [`HeapCall`]: the block mspace_malloc or mspace_realloc gave, or the
//!   one given back to mspace_free or mspace_realloc, its chunk's bytes and
//!   the place in the game that asked ([`CallSite`]: the first return
//!   address out of the allocator layers, unwound through the frames
//!   fields_026g.tsv's `game_call` rows give);
//! - an [`OutOfMemory`]: a fixed arena's heap that had no room.
//!
//! Every allocation of the engine's heaps passes a gate, on any of the
//! game's threads; nothing here allocates from them or calls the engine:
//! the room is kernel32's (VirtualAlloc), the reads are of the calling
//! thread's own stack and of the block's head word.

use super::context::Context;
use super::hook::Thread;
use super::world::read;
use crate::raw;
use core::num::NonZeroU32;
use core::ptr::null_mut;
use core::ffi::c_void;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicBool, Ordering};

/// The profile is on: its gates are written. Set once, by [`start`].
static ON: AtomicBool = AtomicBool::new(false);
/// [`start`] has been asked (once only, whatever it answered).
static STARTED: AtomicBool = AtomicBool::new(false);

/// The gates are written (an event hook's one test while off).
pub fn is_on() -> bool {
    ON.load(Ordering::Acquire)
}

/// Why [`start`] left the profile off. Nothing of it is left: no gate is
/// written, every page has its protection, the tables are given back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartRefused {
    /// Not memory.arenas_ready's context inside allocate_region called by
    /// engine_world's constructor, or not the first time.
    NotTheStart,
    /// A gate's site (its bytes, its page) refused.
    Site(super::InstallRefused),
    /// `tables` found no room.
    NoRoom,
}

/// Switches the profile on, at memory.arenas_ready alone: writes the gates
/// into the allocator's code (`memory_profile.toml`'s hooks installed on
/// demand: dlmalloc's mspace_malloc, mspace_realloc and mspace_free,
/// ptmalloc3's two frees of a mapped block, out_of_memory_with_crash's
/// entry), with `tables` (the profiler's room, [`reserve`]) made in
/// between: every site's bytes checked and every page made writable first,
/// then the tables made, then the jumps written. A refusal at any step
/// leaves the game as it was, its pages restored and the room `tables`
/// made dropped (given back).
///
/// Its precondition is checked, not trusted: `ctx` must be a context whose
/// frame is allocate_region's (its EBP's return address engine_world's
/// constructor's call, 0x13BF5), and only the first call goes on. There
/// only the main thread runs the game's code: the exe makes its threads with
/// threading::spawn (0xE3770) and the CRT's _beginthreadex (0x422C69: for
/// boost.asio, the renderer, Scaleform), and none is reachable from the
/// constructor's code before that call; the crash reporter's and Wine's
/// own threads never run the game's allocator. So no thread runs the sites'
/// code while they are written.
pub fn start<T>(ctx: &Context<'_>, tables: impl FnOnce(&Thread) -> Option<T>) -> Result<T, StartRefused> {
    // SAFETY: the context's EBP as a frame of allocate_region's kind; its
    // return word is read only after its own check (a stack word of this
    // thread when the context is the start's: EBP points into the stack).
    let at_start = unsafe {
        raw::site::memory_arenas_ready::frame(ctx).is_some_and(|frame| {
            let ebp = frame.address() as usize;
            let esp = ctx.reg(super::Reg::Esp) as usize;
            // EBP a frame of this stack just above ESP (allocate_region's, its _alloca of the
            // arenas' list included), before its word is read
            ebp > esp && ebp - esp < 0x4000 && read(frame.return_address()) == raw::image::ALLOCATE_REGION_RETURN
        })
    };
    if !at_start || STARTED.swap(true, Ordering::AcqRel) {
        return Err(StartRefused::NotTheStart);
    }
    let thread = ctx.thread();
    let sites = super::memory_profile_on_demand();
    // SAFETY: checked above: inside allocate_region at the start, the main
    // thread alone runs the game's code until the gates are written.
    let unlocked = unsafe { super::hook::unlock_on_demand(thread, &sites) }.map_err(StartRefused::Site)?;
    let Some(tables) = tables(thread) else {
        return Err(StartRefused::NoRoom); // `unlocked` drops: the pages restored, nothing written
    };
    unlocked.write();
    ON.store(true, Ordering::Release);
    Ok(tables)
}

/// Scroll Lock as user32's GetAsyncKeyState tells it (the snapshot key),
/// read from any thread.
#[derive(Clone, Copy, Debug)]
pub struct ScrollLock(NonNull<c_void>);

// SAFETY: a function of user32, loaded for good, callable from any thread.
unsafe impl Send for ScrollLock {}

impl ScrollLock {
    /// user32's GetAsyncKeyState (user32 loaded if it is not yet); None
    /// when it cannot be had.
    pub fn resolve(_: &Thread) -> Option<ScrollLock> {
        // SAFETY: the exe's imports with NUL-ended names; LoadLibraryA of a
        // system DLL, kept loaded.
        unsafe {
            let user32 = (raw::engine_load_library_a)(c"user32.dll".as_ptr().cast());
            if user32.is_null() {
                return None;
            }
            NonNull::new((raw::engine_get_proc_address)(user32, c"GetAsyncKeyState".as_ptr().cast())).map(ScrollLock)
        }
    }

    /// The key is down now (the state's bit 8000h).
    pub fn down(&self) -> bool {
        const VK_SCROLL: u32 = 0x91;
        // SAFETY: GetAsyncKeyState (resolve), one virtual-key code, no state of ours.
        unsafe { raw::engine_get_async_key_state(self.0, VK_SCROLL) & 0x8000 != 0 }
    }
}

/// The process's environment variable `name` (a C string: its bytes with
/// the NUL), its value read into `into`; None when it is unset, empty or
/// longer than `into`.
pub fn environment<'b>(_: &Thread, name: &[u8], into: &'b mut [u8]) -> Option<&'b [u8]> {
    if name.last() != Some(&0) || name.iter().filter(|&&byte| byte == 0).count() != 1 {
        return None;
    }
    // SAFETY: the exe's import, a NUL-ended name and our own buffer, its
    // size given. It answers 0 for none, the value's length when it fit,
    // the room it needs (with the NUL) when not.
    let length = unsafe { (raw::engine_get_environment_variable_a)(name.as_ptr(), into.as_mut_ptr(), into.len() as u32) };
    let length = length as usize;
    (length > 0 && length < into.len()).then(|| into.get(..length)).flatten()
}

/// Room of our own: `T`s in pages kernel32 committed for us (never an
/// engine heap). Dropped, the pages go back (VirtualFree); [`Room::keep`]
/// keeps them while the game runs.
pub struct Room<T: 'static> {
    items: NonNull<T>,
    count: usize,
    bytes: u32,
}

// SAFETY: the pages are this Room's alone; T: Send moves with it.
unsafe impl<T: Send> Send for Room<T> {}

impl<T> Room<T> {
    /// Where it lies and its bytes, for the report (the address space counts it).
    pub fn place(&self) -> (u32, u32) {
        (self.items.as_ptr() as usize as u32, self.bytes)
    }

    /// The items, kept for good: the pages are never given back.
    pub fn keep(self) -> &'static mut [T] {
        let room = core::mem::ManuallyDrop::new(self);
        // SAFETY: the pages are ours alone, every item written by reserve;
        // this Room is consumed without its Drop: the one `&'static mut` of them.
        unsafe { core::slice::from_raw_parts_mut(room.items.as_ptr(), room.count) }
    }
}

impl<T> Drop for Room<T> {
    fn drop(&mut self) {
        const MEM_RELEASE: u32 = 0x8000;
        // SAFETY: the exe's import on the pages reserve committed, nothing of them lent out.
        unsafe { (raw::engine_virtual_free)(self.items.as_ptr().cast(), 0, MEM_RELEASE) };
    }
}

/// `count` fresh `T`s, each `T::default()`; None when the room cannot be
/// had (the address space has no such hole).
pub fn reserve<T: Copy + Default>(_: &Thread, count: usize) -> Option<Room<T>> {
    const MEM_COMMIT: u32 = 0x1000;
    const MEM_RESERVE: u32 = 0x2000;
    const PAGE_READWRITE: u32 = 0x04;
    let bytes = u32::try_from(count.checked_mul(core::mem::size_of::<T>())?).ok().filter(|&bytes| bytes > 0)?;
    // SAFETY: the exe's import: fresh pages of our own, zeroed, aligned to
    // 64 KiB (any T's alignment), or null.
    let base = unsafe { (raw::engine_virtual_alloc)(null_mut(), bytes, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE) };
    let items = NonNull::new(base.cast::<T>())?;
    for index in 0..count {
        // SAFETY: inside the pages just committed (count T's of them), each
        // written once before anything reads them.
        unsafe { items.as_ptr().add(index).write(T::default()) };
    }
    Some(Room { items, count, bytes })
}

/// A heap's block: the address dlmalloc handed out, its chunk's head the
/// word before it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Block(NonZeroU32);

impl Block {
    pub fn address(self) -> u32 {
        self.0.get()
    }

    /// The block at `address` (tests; the game's come from a gate).
    #[cfg(test)]
    pub fn at(address: u32) -> Option<Block> {
        NonZeroU32::new(address).map(Block)
    }
}

/// Where in the game a block was asked for: a return address of the
/// game's code.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CallSite(pub u32);

/// What a gate's call tells.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeapCall {
    /// dlmalloc's own call (mspace_realloc's move or cut tail,
    /// mspace_memalign's): the outer call, gated itself or not, tells it.
    Own,
    /// No block: an allocation that failed, a free of 0.
    Nothing,
    /// `block`, its chunk `bytes` long (the head's size: what the arena
    /// spends on it), asked for at `site`.
    Block { block: Block, bytes: u32, site: CallSite },
}

/// The gates' frame (memory_profile.allocated, .freeing): the game's
/// call and the block.
fn heap_call(frame: Option<raw::ProfileFrame>) -> HeapCall {
    let Some(frame) = frame else { return HeapCall::Nothing };
    // SAFETY: the gate's own frame on this thread's stack (its two pushes),
    // and the game's call it points at: the ESP the game's `call` left, its
    // return address there, the caller's frames above it on the same
    // stack, live while the gate runs.
    unsafe {
        let Some(call) = frame.call() else { return HeapCall::Nothing };
        let first = read(call.return_address());
        if (raw::image::DLMALLOC_START..raw::image::DLMALLOC_END).contains(&first) {
            return HeapCall::Own;
        }
        let Some(block) = NonZeroU32::new(read(frame.memory())).map(Block) else { return HeapCall::Nothing };
        // a chunk's head: its size, the low three bits its flags
        let head = (block.address().wrapping_sub(4) as usize as *const u32).read_volatile();
        HeapCall::Block { block, bytes: head & !7, site: CallSite(caller(call)) }
    }
}

/// Wrappers unwound at most (the deepest known: mt_allocator::call_realloc,
/// its realloc, pt3realloc over mspace_realloc: three).
const UNWOUND: usize = 6;

/// The first return address out of the allocator layers the game's call
/// went through.
///
/// # Safety
/// `call` is a live call of this thread's stack (a gate's, at its entry).
unsafe fn caller(mut call: raw::GameCall) -> u32 {
    for _ in 0..UNWOUND {
        // SAFETY: the function's contract; each outer call is the wrapper's
        // own, at the offset its frame has at that return (fields_026g.tsv).
        call = unsafe {
            match read(call.return_address()) {
                raw::image::MALLOC_IMPL_HEAP_RETURN => call.malloc_impl_call(),
                raw::image::REALLOC_IMPL_MALLOC_RETURN => call.realloc_impl_malloc_call(),
                raw::image::REALLOC_IMPL_REALLOC_RETURN => call.realloc_impl_realloc_call(),
                raw::image::PT_MALLOC_HEAP_RETURN => call.pt_malloc_call(),
                raw::image::MT_MALLOC_RETURN => call.mt_malloc_call(),
                raw::image::CALL_MALLOC_RETURN => call.call_malloc_call(),
                raw::image::CALL_REALLOC_RETURN => call.call_realloc_call(),
                raw::image::PT_REALLOC_HEAP_RETURN => call.pt_realloc_call(),
                raw::image::MT_REALLOC_RETURN => call.mt_realloc_call(),
                raw::image::MT_CALL_REALLOC_RETURN => call.mt_call_realloc_call(),
                raw::image::MT_MALLOC_IMPL_RETURN => call.mt_malloc_impl_call(),
                site => return site,
            }
        };
    }
    // SAFETY: as above.
    unsafe { read(call.return_address()) }
}

/// The block mspace_malloc or mspace_realloc gave (memory_profile.allocated).
pub fn allocated(ctx: &Context<'_>) -> HeapCall {
    // SAFETY: the context is this entry's: its ESP the gate's frame.
    heap_call(unsafe { raw::site::memory_profile_allocated::frame(ctx) })
}

/// The block about to be given back to mspace_free or mspace_realloc
/// (memory_profile.freeing).
pub fn freeing(ctx: &Context<'_>) -> HeapCall {
    // SAFETY: as allocated.
    heap_call(unsafe { raw::site::memory_profile_freeing::frame(ctx) })
}

/// A fixed arena's heap with no room for a block (memory.out_of_memory):
/// out_of_memory_with_crash ends the game after this.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutOfMemory {
    /// The arena's mspace (at its start).
    pub space: u32,
    /// The bytes asked, when mspace_malloc asked.
    pub asked: Option<u32>,
}

pub fn out_of_memory(ctx: &Context<'_>) -> Option<OutOfMemory> {
    use raw::site::memory_out_of_memory as site;
    // SAFETY: the callback's entry: its ESP the return address, then its
    // three stdcall arguments; EBP mspace_malloc's bytes when it returns
    // there (0x101335).
    unsafe {
        let frame = site::frame(ctx)?;
        let asked = (read(frame.return_address()) == raw::image::HEAP_OUT_OF_MEMORY_RETURN).then(|| site::request(ctx));
        Some(OutOfMemory { space: read(frame.space()), asked })
    }
}
