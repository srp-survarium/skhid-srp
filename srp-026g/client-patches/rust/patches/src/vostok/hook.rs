//! How the game enters Rust: the hook context.
//!
//! The game's frames are aligned to 4 and Rust's i686 code assumes 16 at a
//! call, so the game never calls Rust directly. A hook is a row of
//! patch_026g.py's `RUST_HOOKS`; link_026g.py writes its NASM entry
//! (`RUST_HOOK`, rust_hook_026g.inc: every register and the flags saved,
//! the stack aligned, the shim called, all put back, the replaced call
//! made) and its shim (`hooks_026g.rs`), which makes the [`Hook`], the
//! row's typed arguments from it, and calls the patch's function.

use core::marker::PhantomData;

/// The registers at a `RUST_HOOK` entry, as `pushfd` then `pushad` left
/// them; what a shim writes here (but `esp`) is what the game's code after
/// the entry gets.
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

impl Regs {
    /// ESP as the game had it at the entry (`[esp]` the return address of a
    /// replaced call).
    pub fn entry_esp(&self) -> u32 {
        self.esp.wrapping_add(4)
    }
}

/// Proof that the code runs inside a hook, on the game's thread, while the
/// game waits for it. Only a generated shim makes one; every handle
/// borrows it.
pub struct Hook<'r> {
    regs: &'r mut Regs,
    /// Not `Send`: the game's thread only.
    thread: PhantomData<*mut ()>,
}

impl<'r> Hook<'r> {
    /// # Safety
    /// Called by a generated shim, whose `RUST_HOOK` entry the game called
    /// on its own thread; `regs` are that entry's.
    pub unsafe fn enter(regs: &'r mut Regs) -> Hook<'r> {
        Hook { regs, thread: PhantomData }
    }

    /// The registers at the entry.
    pub fn regs(&self) -> &Regs {
        self.regs
    }

    /// The dword `index` of the game's stack at the entry (`[esp + 4 x
    /// index]`; 0 is a replaced call's return address).
    pub fn stack(&self, index: usize) -> u32 {
        let slot = (self.regs.entry_esp() as usize + 4 * index) as *const u32;
        // SAFETY: inside a hook (enter's contract) the game's frame is above
        // the entry's ESP and stays there until the hook returns; a row
        // names only slots of its site's frame.
        unsafe { slot.read() }
    }
}

/// A handle a hook's argument holds (a register or a stack dword): `None`
/// when it is null.
pub trait FromHook<'h>: Sized {
    /// # Safety
    /// `raw` is what the hook's row says it is: an object of this kind, or
    /// null.
    unsafe fn from_hook(hook: &'h Hook<'_>, raw: u32) -> Option<Self>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stack_at_the_entry() {
        let frame = [0x1111u32, 0x2222, 0x3333];
        let mut regs = Regs { edi: 0, esi: 0, ebp: 0, esp: 0, ebx: 0, edx: 0, ecx: 0, eax: 0, eflags: 0 };
        regs.esp = (frame.as_ptr() as u32).wrapping_sub(4);
        // SAFETY: `frame` stands in for the game's stack.
        let hook = unsafe { Hook::enter(&mut regs) };
        assert_eq!((hook.stack(0), hook.stack(2)), (0x1111, 0x3333));
    }
}
