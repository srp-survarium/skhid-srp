//! A `lohook`'s context: the machine where a mid-code detour sits, and what
//! it does next ([`Flow`]). The generated NASM entry (rust_patches.md) saves
//! every register, the flags and the FPU/SSE state (`fxsave`), fills a
//! [`Frame`], calls the handler, applies what it wrote and takes its
//! [`Flow`]. A handler names a general register by [`Reg`], reads a stack
//! dword or a typed frame local, sets the flags for a following `jcc`, and
//! writes an XMM or the x87 top as the overwritten instructions' result.
//!
//! Patch 76 (the ragdoll) is the first to ship lohooks (`ragdoll.toml`).

use super::hook::{Regs, Thread};

/// A general-purpose register (raw's: a site's typed registers are read by it).
pub use crate::raw::Register as Reg;

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

/// The two pointers the generated `lohook` entry hands its shim: the
/// registers it saved (`pushfd`/`pushad`, as [`Regs`]) and its 512-byte
/// `fxsave` area (the FPU/SSE state; xmm0 at offset 160, 16 bytes each). The
/// entry reloads both (`fxrstor`, `popad`/`popfd`), so what the handler
/// writes through [`Context`] is what the game's code after the detour gets.
#[repr(C)]
pub struct Frame {
    pub regs: *mut Regs,
    pub fxsave: *mut u8,
}

const XMM0: usize = 160;        // xmm0's offset in the fxsave area; 16 bytes each

/// The machine at a `lohook`, borrowing the hook's [`Thread`].
pub struct Context<'t> {
    frame: &'t mut Frame,
    thread: Thread,
}

impl<'t> Context<'t> {
    /// # Safety
    /// Called by a generated `lohook` shim: `frame`'s pointers are its
    /// entry's saved registers and fxsave area, on the game's thread.
    pub unsafe fn new(frame: &'t mut Frame) -> Context<'t> {
        // SAFETY: the shim's contract is this function's.
        Context { frame, thread: unsafe { Thread::new() } }
    }

    /// The hook's thread, to build a handle.
    pub fn thread(&self) -> &Thread {
        &self.thread
    }

    fn regs(&self) -> &Regs {
        // SAFETY: new's contract: `regs` is the entry's saved-register block.
        unsafe { &*self.frame.regs }
    }

    fn regs_mut(&mut self) -> &mut Regs {
        // SAFETY: as regs.
        unsafe { &mut *self.frame.regs }
    }

    fn reg_mut(&mut self, reg: Reg) -> &mut u32 {
        let r = self.regs_mut();
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
        let r = self.regs();
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
        self.regs().eflags & flag as u32 != 0
    }

    pub fn set_flag(&mut self, flag: Flag, on: bool) {
        if on {
            self.regs_mut().eflags |= flag as u32;
        } else {
            self.regs_mut().eflags &= !(flag as u32);
        }
    }

    fn xmm(&self, index: usize) -> *mut u8 {
        assert!(index < 8, "xmm0..7");
        // SAFETY: new's contract: `fxsave` is the entry's 512-byte area.
        unsafe { self.frame.fxsave.add(XMM0 + 16 * index) }
    }

    pub fn xmm_f32(&self, index: usize) -> f32 {
        // SAFETY: the fxsave area holds 16 bytes per xmm register.
        unsafe { self.xmm(index).cast::<f32>().read_unaligned() }
    }

    pub fn set_xmm_f32(&mut self, index: usize, value: f32) {
        // SAFETY: as xmm_f32.
        unsafe { self.xmm(index).cast::<f32>().write_unaligned(value) }
    }

    pub fn xmm_f64(&self, index: usize) -> f64 {
        // SAFETY: as xmm_f32.
        unsafe { self.xmm(index).cast::<f64>().read_unaligned() }
    }

    pub fn set_xmm_f64(&mut self, index: usize, value: f64) {
        // SAFETY: as xmm_f32.
        unsafe { self.xmm(index).cast::<f64>().write_unaligned(value) }
    }
}

/// What a site's typed registers (sites_026g.tsv `holds`, raw's `site`) are read from.
impl crate::raw::Machine for Context<'_> {
    fn register(&self, register: Reg) -> u32 {
        self.reg(register)
    }

    /// The entry pushed its dispatch slot and the flags before `pushad` saved
    /// ESP (hooks_026g.lohook_entry): the site's ESP is 8 above it.
    fn site_esp(&self) -> u32 {
        self.reg(Reg::Esp).wrapping_add(8)
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

    #[test]
    fn registers_flags_and_xmm() {
        let mut regs = Regs { edi: 0, esi: 0, ebp: 0, esp: 0, ebx: 0, edx: 0, ecx: 0, eax: 0, eflags: 0 };
        let mut fxsave = [0u8; 512];
        let mut frame = Frame { regs: &mut regs, fxsave: fxsave.as_mut_ptr() };
        // SAFETY: the test owns the registers and the fxsave area, and stands
        // in for the thread.
        let mut ctx = unsafe { Context::new(&mut frame) };
        ctx.set_reg(Reg::Eax, 0x1234_56FF);
        ctx.set_reg8(Reg::Eax, 0x01);
        assert_eq!(ctx.reg(Reg::Eax), 0x1234_5601);
        ctx.set_flag(Flag::Zf, true);
        assert!(ctx.flag(Flag::Zf));
        ctx.set_flag(Flag::Zf, false);
        assert!(!ctx.flag(Flag::Zf));
        ctx.set_xmm_f32(3, 1.5);
        assert_eq!(ctx.xmm_f32(3), 1.5);
        assert_eq!(regs.eax, 0x1234_5601);
        // xmm3 lives at fxsave + 160 + 48
        assert_eq!(f32::from_le_bytes(fxsave[208..212].try_into().unwrap()), 1.5);
    }
}
