//! The game's CRC-32 step (0x42050, 43 callers: a few bytes of a record, or
//! a whole DDS file at 0x12F340 and 0x3C0FF0): every byte `crc =
//! T[(crc ^ byte) & 0xFF] ^ (crc >> 8)`, T the table 0x1C210 makes at
//! 0x225FD68 from the polynomial 04C11DB7h reflected; its callers set the
//! start and the end. A register function: EAX &the CRC (updated), EDX the
//! bytes, [esp+4] how many, `ret 4`; it leaves ECX the new CRC (the count
//! when it is 0) and EDX past the bytes.

use super::context::{Context, Reg};
use super::object::{read, write};

/// `crc32.step`, the step's start: EAX &the CRC, EDX the bytes, [esp+4] how
/// many.
pub struct CrcStep {
    crc_at: u32,
    bytes: u32,
    count: u32,
}

impl CrcStep {
    pub fn at(ctx: &Context<'_>) -> CrcStep {
        CrcStep { crc_at: ctx.reg(Reg::Eax), bytes: ctx.reg(Reg::Edx), count: ctx.stack(4) }
    }

    /// The bytes to go over (none for a count of 0).
    pub fn bytes(&self) -> &[u8] {
        if self.count == 0 {
            return &[];
        }
        // SAFETY: the caller's buffer of `count` bytes (EDX), readable while
        // the step runs.
        unsafe { core::slice::from_raw_parts(self.bytes as usize as *const u8, self.count as usize) }
    }

    /// The CRC so far.
    pub fn crc(&self) -> u32 {
        // SAFETY: the caller's CRC (EAX), a dword.
        unsafe { read(self.crc_at as usize as *mut u32) }
    }

    /// Leaves as the game's step leaves: the new CRC in [EAX] and ECX, EDX
    /// past the bytes; a count of 0 touches nothing, ECX the count.
    pub fn done(self, ctx: &mut Context<'_>, crc: u32) {
        if self.count == 0 {
            ctx.set_reg(Reg::Ecx, 0);
            return;
        }
        // SAFETY: as crc.
        unsafe { write(self.crc_at as usize as *mut u32, crc) }
        ctx.set_reg(Reg::Ecx, crc);
        ctx.set_reg(Reg::Edx, self.bytes.wrapping_add(self.count));
    }
}
