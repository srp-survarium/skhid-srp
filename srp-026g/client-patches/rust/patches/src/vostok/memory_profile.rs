//! Patch 127's runtime (the profiler is `patches::memory_profile`,
//! `docs/notes/memory-profile.md`): the switch dlmalloc's gates test
//! (memory_profile_026g.asm), the variable that sets it, room of our own
//! for the profiler's tables, and what a gate's call tells, read once
//! here into typed values:
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
use core::sync::atomic::{AtomicU8, Ordering};

/// The gates' switch: 0, each runs the game's own body and nothing else;
/// 1, each tells the profiler. Its byte is the arena's symbol
/// `SRP_MEMORY_PROFILE` (memory_profile_026g.asm `cmp byte [..], 0`); a
/// zero static, so `patches.state`. Written once, by [`switch_on`].
#[unsafe(no_mangle)]
pub static SRP_MEMORY_PROFILE: AtomicU8 = AtomicU8::new(0);

/// The gates tell the profiler.
pub fn is_on() -> bool {
    SRP_MEMORY_PROFILE.load(Ordering::Acquire) != 0
}

/// Every gate tells the profiler from now on.
pub fn switch_on(_: &Thread) {
    SRP_MEMORY_PROFILE.store(1, Ordering::Release);
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

/// Room of our own: `items` in pages kernel32 committed for us (never an
/// engine heap), kept while the game runs; `base` and `bytes` its place,
/// for the report (the address space counts it).
pub struct Room<T: 'static> {
    pub items: &'static mut [T],
    pub base: u32,
    pub bytes: u32,
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
    if base.is_null() {
        return None;
    }
    let first = base.cast::<T>();
    for index in 0..count {
        // SAFETY: inside the pages just committed (count T's of them), each
        // written once before the slice is made.
        unsafe { first.add(index).write(T::default()) };
    }
    // SAFETY: the pages are ours alone, every item written, never freed:
    // the one `&'static mut` of them.
    let items = unsafe { core::slice::from_raw_parts_mut(first, count) };
    Some(Room { items, base: base as usize as u32, bytes })
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

/// Wrappers unwound at most (malloc_impl over mspace_malloc, ptmalloc3's
/// malloc under mt_allocator's call_malloc: two).
const UNWOUND: usize = 4;

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
