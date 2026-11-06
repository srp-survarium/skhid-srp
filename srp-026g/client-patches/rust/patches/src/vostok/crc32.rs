//! The game's CRC-32 step (0x42050, 43 callers: a few bytes of a record, or
//! a whole DDS file at 0x12F340 and 0x3C0FF0): every byte `crc =
//! T[(crc ^ byte) & 0xFF] ^ (crc >> 8)`, T the table 0x1C210 makes at
//! 0x225FD68 from the polynomial 04C11DB7h reflected; its callers set the
//! start and the end. A register function: EAX &the CRC (updated), EDX the
//! bytes, [esp+4] how many, `ret 4`; it leaves ECX the new CRC (the count
//! when it is 0) and EDX past the bytes.

use super::context::{Context, Reg};
use super::world::read;
use crate::raw;

/// The step called on bytes (`crc32.step`, its start).
pub struct CrcStep {
    crc: raw::Crc32Value,
    bytes: raw::Crc32Bytes,
    count: u32,
}

/// What the step was called with.
pub enum CrcCall {
    /// A count of 0: the step reads nothing, not even the CRC.
    Empty,
    /// Bytes to go over, from the CRC so far.
    Bytes(CrcStep),
}

impl CrcCall {
    /// The call at `crc32.step`; none when it has bytes to go over but no
    /// CRC or no bytes to read (the game's own step then faults as it would).
    pub fn at(ctx: &Context<'_>) -> Option<CrcCall> {
        // SAFETY: the site is the step's start: [esp+4] its count, EAX &the
        // CRC and EDX the bytes as its callers give them.
        unsafe {
            let count = read(raw::site::crc32_step::frame(ctx)?.count());
            if count == 0 {
                return Some(CrcCall::Empty);
            }
            Some(CrcCall::Bytes(CrcStep {
                crc: raw::site::crc32_step::crc(ctx)?,
                bytes: raw::site::crc32_step::bytes(ctx)?,
                count,
            }))
        }
    }
}

impl CrcStep {
    /// The bytes to go over.
    pub fn bytes(&self) -> &[u8] {
        // SAFETY: the caller's buffer of `count` bytes (EDX), readable while
        // the step runs.
        unsafe { core::slice::from_raw_parts(self.bytes.first(), self.count as usize) }
    }

    /// The CRC so far.
    pub fn crc(&self) -> u32 {
        // SAFETY: the caller's CRC (EAX), a dword.
        unsafe { read(self.crc.value()) }
    }

    /// Leaves as the game's step leaves: the new CRC in [EAX] and ECX, EDX
    /// past the bytes.
    pub fn done(self, ctx: &mut Context<'_>, crc: u32) {
        // SAFETY: as crc.
        unsafe { self.crc.value().write_volatile(crc) }
        ctx.set_reg(Reg::Ecx, crc);
        ctx.set_reg(Reg::Edx, self.bytes.address().wrapping_add(self.count));
    }
}

/// A count of 0 left as the game's step leaves it: ECX the count, nothing
/// else touched.
pub fn done_empty(ctx: &mut Context<'_>) {
    ctx.set_reg(Reg::Ecx, 0);
}
