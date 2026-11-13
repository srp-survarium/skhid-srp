//! How the game enters Rust, and the pieces a generated shim hands a patch.
//!
//! The game's frames are aligned to 4 and Rust's i686 code assumes 16 at a
//! call, so the game never calls Rust directly: link_026g.py writes a NASM
//! entry (`hook_entries_026g.asm`) that aligns the stack and calls the
//! `extern "C"` shim it also writes (`hooks_026g.rs`, in [`crate::vostok`]).
//! The shim builds the hook's typed arguments and calls the patch's
//! function. A patch module is `#![forbid(unsafe_code)]`, so it can touch
//! none of the unsafe pieces here: it only receives what the shim built.
//!
//! The install kinds (rust_patches.md), each a row of a patch's
//! `<patch>.toml`:
//! - `call`: a game `call F` becomes a call of the shim; the handler gets
//!   F's typed arguments and `Original`, and returns F's result (or a
//!   named `Exit`);
//! - `splice`: a function's start; the handler gets its arguments and
//!   `Original` (a trampoline) and returns the result or an `Exit`;
//! - `lohook`: a detour anywhere; the handler gets `&mut Context`
//!   and returns a `Flow`;
//! - `pointer`, `vtable`, `callback`: a function pointer, a vtable slot, a
//!   callback the game is handed;
//! - `write`, `code`: bytes the build writes.

use core::ffi::c_void;
use core::marker::PhantomData;

/// The registers at a `lohook` entry, as `pushfd` then `pushad` left them;
/// [`Context`](super::Context) reads and writes through this.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Regs {
    pub edi: u32,
    pub esi: u32,
    pub ebp: u32,
    /// ESP before `pushad` (read only: `popad` skips it).
    pub esp: u32,
    pub ebx: u32,
    pub edx: u32,
    pub ecx: u32,
    pub eax: u32,
    pub eflags: u32,
}

const _: () = assert!(size_of::<Regs>() == 36);

/// The thread the game entered a hook on, while it waits for the hook to
/// return. A handle borrows it, so a patch cannot keep a handle past the
/// hook (the borrow ends with the call) nor make one on another thread.
/// Only a generated shim mints it.
pub struct Thread {
    /// Not `Send` or `Sync`: the game's thread only.
    _not_send: PhantomData<*mut ()>,
}

impl Thread {
    /// # Safety
    /// Called by a generated shim, from a NASM entry the game called on its
    /// own thread, which does not return until the shim does.
    pub unsafe fn new() -> Thread {
        Thread { _not_send: PhantomData }
    }
}

/// An out-pointer argument the game gave a hooked function: the handler
/// writes the game's result through it. Built by the generated shim from a
/// raw pointer (its argument), borrowing the [`Thread`].
pub struct Out<'t, T: Copy> {
    at: *mut T,
    thread: PhantomData<&'t Thread>,
}

impl<'t, T: Copy> Out<'t, T> {
    /// # Safety
    /// `at` is the out-pointer of a hooked function, valid for the hook
    /// (the [`Thread`]); null is not allowed (a hooked function's
    /// out-pointer is the caller's local).
    pub unsafe fn new(_: &'t Thread, at: *mut T) -> Out<'t, T> {
        Out { at, thread: PhantomData }
    }

    pub fn set(&mut self, value: T) {
        // SAFETY: new's contract: a valid out-pointer for this hook.
        unsafe { self.at.write(value) }
    }

    pub fn get(&self) -> T {
        // SAFETY: as set; the game wrote it or left the caller's local.
        unsafe { self.at.read() }
    }

    /// The raw out-pointer, for a generated `Original` that passes it back
    /// to the game's own function.
    pub(in crate::vostok) fn at(&self) -> *mut T {
        self.at
    }
}

/// A generated entry that a patch hands the game as a callback (a `callback`
/// site's, in the convention its marker `F` names). Only the generated glue
/// makes one (`<patch>_<handler>_entry()`), so a callback the game calls is
/// always an entry of ours in the convention the game calls it in.
pub struct CallbackEntry<F> {
    entry: *const c_void,
    convention: PhantomData<fn() -> F>,
}

impl<F> Clone for CallbackEntry<F> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<F> Copy for CallbackEntry<F> {}

impl<F> CallbackEntry<F> {
    /// # Safety
    /// `entry` is a generated entry in `F`'s convention, alive as long as
    /// the executable.
    pub unsafe fn from_entry(entry: *const c_void) -> CallbackEntry<F> {
        CallbackEntry { entry, convention: PhantomData }
    }

    /// The entry, for the game function it is handed to.
    pub(in crate::vostok) fn entry(self) -> *const c_void {
        self.entry
    }
}

/// A game object a hooked function's argument points to, built by the
/// generated shim from the argument's raw handle (typed by the function's
/// row; an unsafe step a patch cannot take). A patch receives the handle; it
/// cannot conjure one, because every way to make one is `unsafe` and a
/// patch module forbids `unsafe`.
pub trait Handle<'t>: Sized {
    /// The raw layer's handle this one wraps.
    type Raw;

    /// None for a null argument.
    ///
    /// # Safety
    /// `raw` is alive while the hook runs, on its thread.
    unsafe fn from_raw(thread: &'t Thread, raw: Option<Self::Raw>) -> Option<Self>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn out_reads_and_writes() {
        let mut slot = 7u32;
        // SAFETY: the test owns `slot` and the stand-in thread.
        let thread = unsafe { Thread::new() };
        let mut out = unsafe { Out::new(&thread, &mut slot) };
        assert_eq!(out.get(), 7);
        out.set(2);
        assert_eq!(slot, 2);
    }

    #[test]
    fn only_holds_its_values_and_none_past_the_limit() {
        let only = Only::of(&[0, 4, 31, 32, 49, 255]);
        let held: [u32; 6] = core::array::from_fn(|i| [0, 4, 31, 32, 49, 255][i]);
        for value in 0..300 {
            assert_eq!(only.holds(value), held.contains(&value), "{value}");
        }
        assert!(!Only::of(&[]).holds(0));
    }
}

/// The values a filtered lohook's entry calls its handler for (`only` in its TOML,
/// hooks_026g.py): a bit each, below [`Only::LIMIT`]. The entry tests the site's register
/// against it (`bt`) before it saves anything and replays the site's bytes for any other value.
/// Only the generated glue makes one, from a constant of the patch.
#[repr(C)]
pub struct Only([u32; 8]);

impl Only {
    /// The entry's compare: a value at or past it is not one.
    pub const LIMIT: u32 = 256;

    /// The set of `values`; a value past the limit stops the build.
    pub const fn of(values: &[u32]) -> Self {
        let mut words = [0u32; 8];
        let mut i = 0;
        while i < values.len() {
            let value = values[i];
            assert!(value < Self::LIMIT, "an `only` value is below Only::LIMIT");
            words[(value / 32) as usize] |= 1 << (value % 32);
            i += 1;
        }
        Self(words)
    }

    /// Whether the entry calls its handler for `value` (its `cmp`, then its `bt`).
    pub fn holds(&self, value: u32) -> bool {
        value < Self::LIMIT && self.0[(value / 32) as usize] & (1 << (value % 32)) != 0
    }
}

/// A hook its patch installs at run time (`install = "on_demand"`,
/// hooks_026g.py): its site keeps the game's bytes in the exe until
/// vostok writes the jump to its entry (`unlock_on_demand`, then
/// `Unlocked::write`). Only the generated glue makes one (`<patch>_on_demand()`).
#[derive(Clone, Copy, Debug)]
pub struct OnDemand {
    site: u32,
    original: &'static [u8],
    entry: u32,
}

impl OnDemand {
    /// # Safety
    /// `site` is a span of the game's code of `original`'s bytes, branch-free
    /// and at least 5 long (the generator checked it against the exe), and
    /// `entry` the address of the generated or linked entry for it, which
    /// goes on past the span.
    pub const unsafe fn new(site: u32, original: &'static [u8], entry: u32) -> OnDemand {
        OnDemand { site, original, entry }
    }

    /// The site's address, for a log line.
    pub fn site(&self) -> u32 {
        self.site
    }
}

/// Why [`unlock_on_demand`] refused: nothing written, every page as it was.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstallRefused {
    /// A site's bytes are not the game's (someone wrote there), or its span
    /// is not 5 to 32 bytes.
    Changed { site: u32 },
    /// kernel32 refused to make a site writable.
    Protect { site: u32 },
    /// More sites than [`Unlocked`] keeps protections for.
    TooMany,
}

/// Sites whose bytes were checked to be the game's and whose pages are
/// writable now: [`Unlocked::write`] writes every jump; dropped unwritten,
/// each page gets its protection back and nothing was written.
pub(in crate::vostok) struct Unlocked<'s> {
    sites: &'s [OnDemand],
    old: [u32; Unlocked::MOST],
    made: usize,
}

impl Unlocked<'_> {
    const MOST: usize = 16;

    /// Each page its protection back, the last site first: a site on a page
    /// made writable for an earlier one was told that writable protection
    /// as its old one, so the earliest site's (the page's own) must be set
    /// last, or the page would stay writable and executable.
    fn restore(&mut self) {
        for (site, &old) in self.sites.iter().zip(&self.old).take(self.made).rev() {
            let mut was = 0u32;
            // SAFETY: the exe's import on a span `unlock_on_demand` made writable.
            unsafe { (crate::raw::engine_virtual_protect)(site.site as usize as *mut c_void, site.original.len() as u32, old, &mut was) };
        }
        self.made = 0;
    }

    /// Writes each site's jump to its entry (a `jmp rel32`, then nops to the
    /// span's end), gives each page its protection back and flushes the
    /// instruction cache.
    pub(in crate::vostok) fn write(mut self) {
        // SAFETY: kernel32 is loaded in every process; its export's address or null.
        let flush = unsafe {
            let kernel32 = (crate::raw::engine_get_module_handle_a)(c"kernel32.dll".as_ptr().cast());
            if kernel32.is_null() {
                None
            } else {
                core::ptr::NonNull::new((crate::raw::engine_get_proc_address)(kernel32, c"FlushInstructionCache".as_ptr().cast()))
            }
        };
        for site in self.sites {
            let at = site.site as usize as *mut u8;
            let relative = site.entry.wrapping_sub(site.site.wrapping_add(5));
            let mut bytes = [0x90u8; 32];
            bytes[0] = 0xE9;
            bytes[1..5].copy_from_slice(&relative.to_le_bytes());
            // SAFETY: the span (5 to 32 bytes, checked), writable now; unlock_on_demand's
            // contract: no thread runs it.
            unsafe { core::ptr::copy_nonoverlapping(bytes.as_ptr(), at, site.original.len()) };
        }
        self.restore();
        if let Some(flush) = flush {
            for site in self.sites {
                // SAFETY: FlushInstructionCache of this process (the pseudo-handle -1) over the span.
                unsafe {
                    crate::raw::engine_flush_instruction_cache(flush, usize::MAX as *mut c_void,
                        site.site as usize as *const c_void, site.original.len() as u32)
                };
            }
        }
    }
}

impl Drop for Unlocked<'_> {
    fn drop(&mut self) {
        self.restore();
    }
}

/// Every site's bytes checked to be the game's, then every page made
/// writable: all of them, or none (each page made writable before a
/// refusal gets its protection back). Nothing is written yet.
///
/// # Safety
/// No other thread may run any of the sites' code from now until the
/// [`Unlocked`] is written or dropped.
pub(in crate::vostok) unsafe fn unlock_on_demand<'s>(_: &Thread, sites: &'s [OnDemand]) -> Result<Unlocked<'s>, InstallRefused> {
    const PAGE_EXECUTE_READWRITE: u32 = 0x40;
    if sites.len() > Unlocked::MOST {
        return Err(InstallRefused::TooMany);
    }
    for site in sites {
        if !(5..=32).contains(&site.original.len()) {
            return Err(InstallRefused::Changed { site: site.site });
        }
        // SAFETY: the site's span is the game's code, mapped and readable.
        let now = unsafe { core::slice::from_raw_parts(site.site as usize as *const u8, site.original.len()) };
        if now != site.original {
            return Err(InstallRefused::Changed { site: site.site });
        }
    }
    let mut unlocked = Unlocked { sites, old: [0; Unlocked::MOST], made: 0 };
    for (index, site) in sites.iter().enumerate() {
        let mut old = 0u32;
        // SAFETY: the exe's import on the game's own code pages.
        let made = unsafe {
            (crate::raw::engine_virtual_protect)(site.site as usize as *mut c_void, site.original.len() as u32, PAGE_EXECUTE_READWRITE, &mut old)
        };
        if made == 0 {
            return Err(InstallRefused::Protect { site: site.site }); // `unlocked` drops: the pages before it restored
        }
        if let Some(slot) = unlocked.old.get_mut(index) {
            *slot = old;
        }
        unlocked.made = index + 1;
    }
    Ok(unlocked)
}
