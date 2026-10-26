//! A `lohook`'s context: the machine where a mid-code detour sits, and what
//! it does next ([`Flow`]). The generated NASM entry (rust_patches.md) saves
//! every register, the flags and the FPU/SSE state (`fxsave`), fills a
//! [`Frame`], calls the handler, applies what it wrote and takes its
//! [`Flow`]. A handler names a general register by [`Reg`], reads a stack
//! dword or a typed frame local, sets the flags for a following `jcc`, and
//! writes an XMM or the x87 top as the overwritten instructions' result.
//!
//! The site's stack is read and written by its ESP offsets ([`Context::stack`]),
//! and an x87 result is pushed as an `fld` would ([`Context::push_st`]).
//! Patch 76 (the ragdoll) is the first to ship lohooks (`ragdoll.toml`).

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

    /// The low 32 bits of xmm`index`, as they are (a float's bits, a
    /// not-a-number's too).
    pub fn xmm_u32(&self, index: usize) -> u32 {
        // SAFETY: as xmm_f32.
        unsafe { self.xmm(index).cast::<u32>().read_unaligned() }
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

    /// ESP at the site, as the game's code there has it (the entry's slot
    /// and `pushfd` are below it: [`Regs::esp`] is 8 lower).
    pub fn site_esp(&self) -> u32 {
        self.regs().esp.wrapping_add(8)
    }

    /// The dword at `[esp + offset]` of the site (its frame: the pushed
    /// arguments, the locals the game's code there addresses by ESP).
    pub fn stack(&self, offset: u32) -> u32 {
        let at = self.site_esp().wrapping_add(offset) as usize as *const u32;
        // SAFETY: the site's own stack, which the game's code there reads
        // at these offsets (a site row's `what` names them).
        unsafe { at.read_unaligned() }
    }

    /// Writes the dword at `[esp + offset]` of the site.
    pub fn set_stack(&mut self, offset: u32, value: u32) {
        let at = self.site_esp().wrapping_add(offset) as usize as *mut u32;
        // SAFETY: as stack: the site's frame, written as its own code does.
        unsafe { at.write_unaligned(value) }
    }

    /// The byte at `[esp + offset]` of the site.
    pub fn stack_u8(&self, offset: u32) -> u8 {
        let at = self.site_esp().wrapping_add(offset) as usize as *const u8;
        // SAFETY: as stack.
        unsafe { at.read() }
    }

    /// Writes the byte at `[esp + offset]` of the site.
    pub fn set_stack_u8(&mut self, offset: u32, value: u8) {
        let at = self.site_esp().wrapping_add(offset) as usize as *mut u8;
        // SAFETY: as set_stack.
        unsafe { at.write(value) }
    }

    /// Pushes `value` on the x87 stack, as an `fld` would (the code after the
    /// detour finds it in ST0): the saved state's TOP one lower, ST1.. the
    /// old ST0.., ST0 `value` exactly (an f64 is exact in 80 bits), its tag
    /// valid, C1 clear. The `fld` this stands for has room at its sites (the
    /// game's x87 stack is empty there); a full stack's old ST7 is dropped,
    /// as an emulator's `fld` drops it (a CPU's would fault: masked, the
    /// indefinite NaN), never at the patches' sites.
    pub fn push_st(&mut self, value: f64) {
        let fx = self.frame.fxsave;
        // SAFETY: new's contract: `fxsave` is the entry's 512-byte area;
        // FSW at +2, the abridged tag byte at +4, ST0..7 at +32 (16 each).
        unsafe {
            let fsw = fx.add(2).cast::<u16>().read_unaligned();
            let top = ((fsw >> 11) & 7) as u8;
            let new_top = top.wrapping_sub(1) & 7;
            let tags = fx.add(4).read();
            for i in (0..7).rev() {
                core::ptr::copy_nonoverlapping(fx.add(32 + 16 * i), fx.add(32 + 16 * (i + 1)), 10);
            }
            core::ptr::copy_nonoverlapping(ext80(value).as_ptr(), fx.add(32), 10);
            fx.add(4).write(tags | (1 << new_top));
            let fsw = (fsw & !(7 << 11) & !(1 << 9)) | (u16::from(new_top) << 11);
            fx.add(2).cast::<u16>().write_unaligned(fsw);
        }
    }
}

impl Context<'_> {
    /// Multiplies ST0 by `factor` as an `fmul dword [m32]` at the site
    /// would: in the x87, under the game's precision and rounding control
    /// (the control word the entry kept: the hardware's while the handler
    /// runs), the 80-bit result left in the saved ST0. The status word's
    /// flags the multiply raises (C1, the sticky precision bit) are not
    /// kept: the entry restores the site's. ST0 must hold a value.
    pub fn fmul_st0(&mut self, factor: f32) {
        // SAFETY: new's contract: `fxsave` is the entry's 512-byte area,
        // ST0 at +32 (10 bytes, stack order). The asm loads it, multiplies
        // and stores it back, the x87 stack as it was (empty: the handler's
        // Rust code keeps it so across calls).
        unsafe {
            core::arch::asm!("fld tbyte ptr [{st}]", "fmul dword ptr [{f}]", "fstp tbyte ptr [{st}]",
                             st = in(reg) self.frame.fxsave.add(32), f = in(reg) &factor,
                             out("st(0)") _, out("st(1)") _, out("st(2)") _, out("st(3)") _,
                             out("st(4)") _, out("st(5)") _, out("st(6)") _, out("st(7)") _,
                             options(nostack, preserves_flags));
        }
    }
}

/// `value` in the x87's 80-bit extended format (sign, 15-bit exponent
/// biased 16383, 64-bit significand with its integer bit): exact for every
/// f64, a subnormal normalized, an infinity or NaN kept.
pub(crate) fn ext80(value: f64) -> [u8; 10] {
    let bits = value.to_bits();
    let sign = ((bits >> 63) as u16) << 15;
    let exponent = ((bits >> 52) & 0x7FF) as u16;
    let fraction = bits & ((1 << 52) - 1);
    let (e, m) = match exponent {
        0 if fraction == 0 => (0, 0),
        0 => {
            let shift = fraction.leading_zeros();
            (16383 - 1011 - shift as u16, fraction << shift)
        }
        0x7FF => (0x7FFF, (1 << 63) | (fraction << 11)),
        _ => (exponent - 1023 + 16383, (1 << 63) | (fraction << 11)),
    };
    let mut out = [0u8; 10];
    out[..8].copy_from_slice(&m.to_le_bytes());
    out[8..].copy_from_slice(&(sign | e).to_le_bytes());
    out
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

    #[test]
    fn extended_precision() {
        let e = |v: f64| { let b = ext80(v); (u16::from_le_bytes([b[8], b[9]]), u64::from_le_bytes(b[..8].try_into().unwrap())) };
        assert_eq!(e(1.0), (0x3FFF, 1 << 63));
        assert_eq!(e(-4.0), (0xC001, 1 << 63));
        assert_eq!(e(0.0), (0, 0));
        assert_eq!(e(1.5), (0x3FFF, 0xC000_0000_0000_0000));
        assert_eq!(e(f64::from_bits(1)), (16383 - 1074, 1 << 63));
        assert_eq!(e(f64::INFINITY), (0x7FFF, 1 << 63));
    }

    #[test]
    fn x87_multiply() {
        let mut regs = Regs { edi: 0, esi: 0, ebp: 0, esp: 0, ebx: 0, edx: 0, ecx: 0, eax: 0, eflags: 0 };
        #[repr(align(16))]
        struct Area([u8; 512]);
        let mut fx = Area([0u8; 512]);
        fx.0[32..42].copy_from_slice(&ext80(2.5));
        let mut frame = Frame { regs: &mut regs, fxsave: fx.0.as_mut_ptr() };
        // SAFETY: the test owns the registers and the fxsave area.
        let mut ctx = unsafe { Context::new(&mut frame) };
        ctx.fmul_st0(100.0);
        ctx.fmul_st0(0.5);
        assert_eq!(fx.0[32..42], ext80(125.0));
        assert_eq!(fx.0[42..48], [0; 6]);
    }

    #[test]
    fn stack_and_x87_push() {
        let mut words = [0u32; 8];
        let site = words.as_mut_ptr() as usize as u32 + 8;          // the slot and pushfd below the site
        let mut regs = Regs { edi: 0, esi: 0, ebp: 0, esp: site - 8, ebx: 0, edx: 0, ecx: 0, eax: 0, eflags: 0 };
        let mut fxsave = [0u8; 512];
        fxsave[2..4].copy_from_slice(&(7u16 << 11).to_le_bytes());   // TOP 7
        fxsave[4] = 1 << 7;                                          // physical 7 (ST0) valid
        fxsave[32..42].copy_from_slice(&ext80(2.0));
        let mut frame = Frame { regs: &mut regs, fxsave: fxsave.as_mut_ptr() };
        // SAFETY: the test owns the registers, the stack words and the fxsave area.
        let mut ctx = unsafe { Context::new(&mut frame) };
        ctx.set_stack(4, 0xABCD);
        ctx.set_stack_u8(8, 7);
        assert_eq!((ctx.stack(4), ctx.stack_u8(8)), (0xABCD, 7));
        ctx.push_st(5.0);
        assert_eq!(words[3], 0xABCD);
        assert_eq!(u16::from_le_bytes([fxsave[2], fxsave[3]]) >> 11 & 7, 6);
        assert_eq!(fxsave[4], (1 << 7) | (1 << 6));
        assert_eq!(fxsave[32..42], ext80(5.0));
        assert_eq!(fxsave[48..58], ext80(2.0));
    }
}
