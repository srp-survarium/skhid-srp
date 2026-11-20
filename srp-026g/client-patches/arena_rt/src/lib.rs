//! What every Rust crate of the arena needs and the game does not give it.
//!
//! - `memcpy` and `memset`: LLVM calls them for a large copy or fill, and
//!   the arena has no C library. Weak, so that each crate may bring them and
//!   the link keeps one; written as `rep movsb` / `rep stosb`, which LLVM
//!   cannot turn back into a call of themselves.
//! - The panic handler: a loud crash at a known address (`ud2`), never a
//!   wrong world.
//! - [`c_u32`]: a float made unsigned as the first build's C did it.
//!
//! A crate of the arena names this one by path and is entered from the game
//! only through a NASM stub that aligns the stack to 16 (README "The arena").

#![no_std]
#![feature(linkage)]

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

/// `(unsigned)value` as gcc -m32 compiles it (the C patches' flags): x87
/// `fistp` to 64 bits, truncating, and the low 32 bits. So a negative value
/// wraps (-1.0 is FFFFFFFFh), and NaN or one of 2^63 and more either way
/// (the x87's "integer indefinite") is 0. Rust's `as u32` would saturate.
#[inline(never)]
pub fn c_u32(value: f32) -> u32 {
    const LIMIT: f32 = 9_223_372_036_854_775_808.0; // 2^63
    if value > -LIMIT && value < LIMIT { value as i64 as u32 } else { 0 }
}

#[cfg(not(test))]
#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {
        unsafe { core::arch::asm!("ud2", options(noreturn)) }
    }
}
