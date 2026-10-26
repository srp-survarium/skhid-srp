//! A `lohook`'s context: the machine where a mid-code detour sits, and what
//! it does next ([`Flow`]). The generated NASM entry (rust_patches.md) saves
//! every register, the flags and the FPU/SSE state (`fxsave`), fills a
//! [`Frame`], calls the handler, applies what it wrote and takes its
//! [`Flow`]. A handler names a general register by [`Reg`], reads a stack
//! dword or a typed frame local, sets the flags for a following `jcc`, and
//! writes an XMM or the x87 top as the overwritten instructions' result.
//!
//! No `lohook` is generated yet: the first port that needs one adds the
//! entry (its register save/restore). The API and the [`Frame`] are here so
//! the design is whole and tested.

use super::hook::{Regs, Thread};

/// A general-purpose register.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reg {
    Eax,
    Ecx,
    Edx,
    Ebx,
    Esp,
    Ebp,
    Esi,
    Edi,
}

/// A flag a handler may set for the code after the detour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flag {
    /// Zero.
    Zf = 1 << 6,
    /// Carry.
    Cf = 1 << 0,
    /// Sign.
    Sf = 1 << 7,
    /// Overflow.
    Of = 1 << 11,
}

/// What the entry saved, laid out for it to read back. The generated entry
/// owns it; a handler reaches it only through [`Context`].
#[repr(C)]
pub struct Frame {
    pub regs: Regs,
    /// xmm0..=xmm7, as the entry's `fxsave` holds them (low 8 bytes each).
    pub xmm: [u64; 8],
    /// The x87 top to load on the way out, when `st0_set` is non-zero.
    pub st0: f64,
    pub st0_set: u32,
}

/// The machine at a `lohook`, borrowing the hook's [`Thread`].
pub struct Context<'t> {
    frame: &'t mut Frame,
    thread: Thread,
}

impl<'t> Context<'t> {
    /// # Safety
    /// Called by a generated `lohook` shim: `frame` is its entry's, on the
    /// game's thread.
    pub unsafe fn new(frame: &'t mut Frame) -> Context<'t> {
        // SAFETY: the shim's contract is this function's.
        Context { frame, thread: unsafe { Thread::new() } }
    }

    /// The hook's thread, to build a handle.
    pub fn thread(&self) -> &Thread {
        &self.thread
    }

    fn reg_mut(&mut self, reg: Reg) -> &mut u32 {
        let r = &mut self.frame.regs;
        match reg {
            Reg::Eax => &mut r.eax,
            Reg::Ecx => &mut r.ecx,
            Reg::Edx => &mut r.edx,
            Reg::Ebx => &mut r.ebx,
            Reg::Esp => &mut r.esp,
            Reg::Ebp => &mut r.ebp,
            Reg::Esi => &mut r.esi,
            Reg::Edi => &mut r.edi,
        }
    }

    pub fn reg(&self, reg: Reg) -> u32 {
        let r = &self.frame.regs;
        match reg {
            Reg::Eax => r.eax,
            Reg::Ecx => r.ecx,
            Reg::Edx => r.edx,
            Reg::Ebx => r.ebx,
            Reg::Esp => r.esp,
            Reg::Ebp => r.ebp,
            Reg::Esi => r.esi,
            Reg::Edi => r.edi,
        }
    }

    pub fn set_reg(&mut self, reg: Reg, value: u32) {
        *self.reg_mut(reg) = value;
    }

    /// Sets a register's low byte (al, bl, ..), leaving the rest.
    pub fn set_reg8(&mut self, reg: Reg, value: u8) {
        let slot = self.reg_mut(reg);
        *slot = (*slot & !0xFF) | u32::from(value);
    }

    pub fn flag(&self, flag: Flag) -> bool {
        self.frame.regs.eflags & flag as u32 != 0
    }

    pub fn set_flag(&mut self, flag: Flag, on: bool) {
        if on {
            self.frame.regs.eflags |= flag as u32;
        } else {
            self.frame.regs.eflags &= !(flag as u32);
        }
    }

    pub fn xmm_f32(&self, index: usize) -> f32 {
        f32::from_bits(self.frame.xmm[index] as u32)
    }

    pub fn set_xmm_f32(&mut self, index: usize, value: f32) {
        self.frame.xmm[index] = u64::from(value.to_bits());
    }

    pub fn xmm_f64(&self, index: usize) -> f64 {
        f64::from_bits(self.frame.xmm[index])
    }

    pub fn set_xmm_f64(&mut self, index: usize, value: f64) {
        self.frame.xmm[index] = value.to_bits();
    }

    /// Loads the x87 top with `value` on the way out (a float the
    /// overwritten instructions would have left in st0).
    pub fn set_st0(&mut self, value: f64) {
        self.frame.st0 = value;
        self.frame.st0_set = 1;
    }
}

/// Where the game goes when the handler returns.
pub enum Flow<E> {
    /// Run the overwritten instructions, then go on (the default).
    Continue,
    /// Skip the overwritten instructions.
    Skip,
    /// Jump to one of the hook's named exits.
    Exit(E),
}

/// A hook with no named exits (its only non-default flow is [`Flow::Skip`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoExit {}

/// How a `splice` or `call` handler returns on behalf of the host: either
/// the function's value, or an [`Exit`] the hook named.
pub enum Ret<R, E> {
    Value(R),
    Exit(E),
}

/// A named exit's kind, for the generator (rust_patches.md): a jump inside
/// the host function, or a return from it. A patch names an exit by an enum
/// the generator emits; this is only the shape the TOML declares.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Exit {
    /// `jmp` to an address inside the host function.
    Jump(u32),
    /// Return from the host function, popping `pop` bytes, EAX = `value`.
    HostReturn { pop: u32, value: u32 },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame() -> Frame {
        Frame { regs: Regs { edi: 0, esi: 0, ebp: 0, esp: 0, ebx: 0, edx: 0, ecx: 0, eax: 0, eflags: 0 },
                xmm: [0; 8], st0: 0.0, st0_set: 0 }
    }

    #[test]
    fn registers_flags_and_xmm() {
        let mut f = frame();
        // SAFETY: the test owns the frame and stands in for the thread.
        let mut ctx = unsafe { Context::new(&mut f) };
        ctx.set_reg(Reg::Eax, 0x1234_56FF);
        ctx.set_reg8(Reg::Eax, 0x01);
        assert_eq!(ctx.reg(Reg::Eax), 0x1234_5601);
        ctx.set_flag(Flag::Zf, true);
        assert!(ctx.flag(Flag::Zf));
        ctx.set_flag(Flag::Zf, false);
        assert!(!ctx.flag(Flag::Zf));
        ctx.set_xmm_f32(3, 1.5);
        assert_eq!(ctx.xmm_f32(3), 1.5);
        ctx.set_st0(2.5);
        assert_eq!((f.st0, f.st0_set), (2.5, 1));
    }
}
