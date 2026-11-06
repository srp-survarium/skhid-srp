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
}
