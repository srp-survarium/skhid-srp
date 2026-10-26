//! What a `no_std` crate in the game needs and the game does not give it.
//!
//! **A panic** is a bug of ours (an index past a slice, a broken
//! invariant), never a way to refuse input: a hook refuses what it cannot
//! take with a value (an `Option`, an enum of outcomes) and its NASM entry
//! takes the game's own path then. There is no unwinding (panic=abort): the
//! handler writes the panic's place to `srp_patches.log` and stops at `ud2`,
//! so the game's crash reporter catches it with our code's address, the line
//! in the log beside its dump. A loud stop, never a world computed wrong and
//! never undefined behaviour; nothing goes on with a half-done change.
//!
//! **memcpy and memset**: LLVM calls them for a large copy or fill, and the
//! game gives none to the arena (one crate: one copy); `rep movsb` and `rep
//! stosb`, which LLVM cannot turn back into a call of themselves. Not in
//! `cargo test`'s binaries (std has its own).

/// `(unsigned)value` as gcc -m32 compiles it (the C patches' flags): x87
/// `fistp` to 64 bits, truncating, and the low 32 bits. A negative value
/// wraps (-1.0 is FFFFFFFFh), NaN and a value of 2^63 or more either way
/// (the x87's "integer indefinite") give 0, where Rust's `as u32` saturates:
/// a port of C that converts a float keeps the C's results with this.
#[inline(never)]
pub fn c_u32(value: f32) -> u32 {
    const LIMIT: f32 = 9_223_372_036_854_775_808.0; // 2^63
    if value > -LIMIT && value < LIMIT { value as i64 as u32 } else { 0 }
}

#[cfg(not(test))]
mod game {
    /// # Safety
    /// As C's: `to` and `from` valid for `size` bytes, not overlapping.
    #[unsafe(no_mangle)]
    #[linkage = "weak"]
    pub unsafe extern "C" fn memcpy(to: *mut u8, from: *const u8, size: usize) -> *mut u8 {
        // ESI is LLVM's to keep (no operand may name it): swapped in and back.
        unsafe {
            core::arch::asm!("xchg esi, {from}", "rep movsb", "mov esi, {from}", from = inout(reg) from => _,
                             inout("ecx") size => _, inout("edi") to => _, options(nostack, preserves_flags));
        }
        to
    }

    /// # Safety
    /// As C's: `to` valid for `size` bytes.
    #[unsafe(no_mangle)]
    #[linkage = "weak"]
    pub unsafe extern "C" fn memset(to: *mut u8, value: i32, size: usize) -> *mut u8 {
        unsafe {
            core::arch::asm!("rep stosb", inout("ecx") size => _, inout("edi") to => _, in("al") value as u8,
                             options(nostack, preserves_flags));
        }
        to
    }

    #[panic_handler]
    fn panic(info: &core::panic::PanicInfo) -> ! {
        use crate::vostok::log::{Piece::*, line};
        match info.location() {
            Some(at) => line(&[Text("a patch panicked at "), Text(at.file()), Text(":"), Dec(at.line())]),
            None => line(&[Text("a patch panicked")]),
        }
        // `ud2` faults; the crash reporter takes it, our address in the log.
        unsafe { core::arch::asm!("ud2", options(noreturn)) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floats_made_unsigned_as_gcc_does() {
        assert_eq!(c_u32(5.9), 5);
        assert_eq!(c_u32(-1.0), u32::MAX);
        assert_eq!(c_u32(-5.5), (-5i32) as u32);
        assert_eq!(c_u32(4_294_967_296.0), 0); // 2^32: the low 32 bits
        assert_eq!(c_u32(1e19), 0); // past 2^63: indefinite
        assert_eq!(c_u32(f32::NAN), 0);
        assert_eq!(c_u32(f32::INFINITY), 0);
    }
}
