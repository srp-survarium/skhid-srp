//! The hook ABI: how the game enters Rust (`client-patches/
//! rust_hook_026g.inc`, `docs/notes/rust_patches.md`).
//!
//! The game's frames are aligned to 4 and Rust's i686 code assumes 16 at a
//! call, so the game never calls Rust directly: a NASM entry made by one of
//! the include's macros aligns the stack and calls an `extern "C"` function.
//!
//! - `RUST_HOOK target`, in the middle of the game's code (a replaced call
//!   or a jump to us): every register and the flags saved, `target(&mut
//!   Regs)` called, the registers and flags put back from [`Regs`] (so the
//!   hook reads any register, a stack slot of the game's frame, and may set
//!   registers for the code after it), then the entry goes on (the replaced
//!   instructions, a `jmp` back). The Rust side:
//!   `extern "C" fn target(regs: &mut Regs)`.
//! - `RUST_THISCALL name, target, n`: a function the game calls with ECX
//!   and `n` dwords on the stack, popping them (a vtable slot, a delegate):
//!   `extern "C" fn target(this: *mut T, a: u32, ..) -> R`, EAX the result.
//! - `RUST_CDECL name, target, n`: a function the game calls cdecl with `n`
//!   dwords: `extern "C" fn target(a: u32, ..) -> R`.

/// The registers at a `RUST_HOOK` entry, as `pushfd` then `pushad` left
/// them; what the hook writes here (but `esp`) is what the game's code after
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
    /// The flags' zero bit.
    pub const ZF: u32 = 1 << 6;
    /// The flags' carry bit.
    pub const CF: u32 = 1 << 0;

    /// ESP as the game had it at the entry (`[esp]` the return address of a
    /// replaced call).
    pub fn entry_esp(&self) -> u32 {
        self.esp.wrapping_add(4)
    }

    /// Where the dword `index` of the game's stack at the entry is
    /// (`[esp + 4 x index]`).
    pub fn stack_slot(&self, index: usize) -> *mut u32 {
        (self.entry_esp() as usize + 4 * index) as *mut u32
    }

    /// The dword `index` of the game's stack at the entry.
    ///
    /// # Safety
    /// The slot is the game's frame (a hook entry's own stack).
    pub unsafe fn stack(&self, index: usize) -> u32 {
        unsafe { self.stack_slot(index).read() }
    }

    /// AL, the low byte of EAX (a `bool` the game's functions return).
    pub fn al(&self) -> u8 {
        self.eax as u8
    }

    pub fn set_al(&mut self, value: u8) {
        self.eax = self.eax & !0xFF | u32::from(value);
    }

    /// ZF for the code after the entry (a `jz` / `jnz` there).
    pub fn set_zero_flag(&mut self, zero: bool) {
        self.eflags = if zero { self.eflags | Self::ZF } else { self.eflags & !Self::ZF };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn low_byte_and_flags() {
        let mut regs = Regs { edi: 0, esi: 0, ebp: 0, esp: 0x1000, ebx: 0, edx: 0, ecx: 0, eax: 0x1234_56FF, eflags: 0 };
        regs.set_al(1);
        assert_eq!(regs.eax, 0x1234_5601);
        regs.set_zero_flag(true);
        assert_eq!(regs.eflags, Regs::ZF);
        assert_eq!(regs.entry_esp(), 0x1004);
        assert_eq!(regs.stack_slot(2) as usize, 0x100C);
    }
}
