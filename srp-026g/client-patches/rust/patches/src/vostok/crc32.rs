//! The game's CRC-32 step (0x42050, 43 callers: a few bytes of a record, or
//! a whole DDS file at 0x12F340 and 0x3C0FF0): every byte `crc =
//! T[(crc ^ byte) & 0xFF] ^ (crc >> 8)`, T the table 0x1C210 makes at
//! 0x225FD68 from the polynomial 04C11DB7h reflected; its callers set the
//! start and the end. A register function: EAX &the CRC (updated), EDX the
//! bytes, [esp+4] how many, `ret 4`; it leaves ECX the new CRC (the count
//! when it is 0) and EDX past the bytes.

use core::arch::x86::{__cpuid, __m128i, _mm_and_si128, _mm_clmulepi64_si128, _mm_cvtsi32_si128, _mm_cvtsi128_si32,
                      _mm_loadu_si128, _mm_set_epi32, _mm_set_epi64x, _mm_srli_si128, _mm_xor_si128};
use core::sync::atomic::{AtomicU8, Ordering};

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

// ---- the CRC-32 itself --------------------------------------------------------

/// The polynomial 04C11DB7h reflected (the game's table at 0x225FD68).
const POLYNOMIAL: u32 = 0xEDB8 << 16 | 0x8320;

/// The game's table (T0) and the seven a step of eight bytes needs: Tk[i]
/// the CRC of i followed by k zero bytes.
static SLICES: [[u32; 256]; 8] = slices();

const fn slices() -> [[u32; 256]; 8] {
    let mut out = [[0u32; 256]; 8];
    let mut i = 0;
    while i < 256 {
        let mut c = i as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 { (c >> 1) ^ POLYNOMIAL } else { c >> 1 };
            k += 1;
        }
        out[0][i] = c;
        i += 1;
    }
    let mut t = 1;
    while t < 8 {
        let mut i = 0;
        while i < 256 {
            out[t][i] = (out[t - 1][i] >> 8) ^ out[0][(out[t - 1][i] & 0xFF) as usize];
            i += 1;
        }
        t += 1;
    }
    out
}

/// Eight bytes a step through the eight tables, the rest a byte a step
/// through T0 (the game's).
fn sliced(mut crc: u32, bytes: &[u8]) -> u32 {
    let t = &SLICES;
    let mut eights = bytes.chunks_exact(8);
    for b in &mut eights {
        let low = u32::from_le_bytes([b[0], b[1], b[2], b[3]]) ^ crc;
        let high = u32::from_le_bytes([b[4], b[5], b[6], b[7]]);
        crc = t[7][(low & 0xFF) as usize] ^ t[6][(low >> 8 & 0xFF) as usize] ^ t[5][(low >> 16 & 0xFF) as usize]
            ^ t[4][(low >> 24) as usize] ^ t[3][(high & 0xFF) as usize] ^ t[2][(high >> 8 & 0xFF) as usize]
            ^ t[1][(high >> 16 & 0xFF) as usize] ^ t[0][(high >> 24) as usize];
    }
    for &b in eights.remainder() {
        crc = t[0][((crc ^ u32::from(b)) & 0xFF) as usize] ^ (crc >> 8);
    }
    crc
}

/// Fewer bytes than this go the sliced way: the carry-less fold takes 64 at
/// its start and pays its last reductions once.
const FOLDED_FROM: usize = 128;

/// What CPUID told of the carry-less multiply: UNASKED until the first long
/// step asks (any thread; two that ask at once store the same answer).
static FOLDS: AtomicU8 = AtomicU8::new(UNASKED);
const UNASKED: u8 = 0;
const NO_FOLD: u8 = 1;
const FOLD: u8 = 2;

/// Whether the processor multiplies carry-less (PCLMULQDQ, CPUID 1 ECX bit
/// 1; SSE2, EDX bit 26, which the game itself needs).
fn folds() -> bool {
    let known = FOLDS.load(Ordering::Relaxed);
    if known != UNASKED {
        return known == FOLD;
    }
    // leaf 1 is on every processor the game runs on (it needs SSE2)
    let leaf = __cpuid(1);
    let fold = leaf.ecx & 1 << 1 != 0 && leaf.edx & 1 << 26 != 0;
    FOLDS.store(if fold { FOLD } else { NO_FOLD }, Ordering::Relaxed);
    fold
}

/// The CRC-32 step over `bytes` from `crc`, the same value as the game's
/// byte a step (0x42050: `crc = T[(crc ^ byte) & 0xFF] ^ (crc >> 8)`, no
/// inversion, the callers do the start and the end) for every input. Long
/// runs are folded 64 bytes a step with carry-less multiplies where the
/// processor has them (about ten times the sliced speed), else eight bytes
/// a step through eight tables.
pub fn update(crc: u32, bytes: &[u8]) -> u32 {
    if bytes.len() >= FOLDED_FROM && folds() {
        // SAFETY: folds() saw PCLMULQDQ and SSE2.
        unsafe { folded(crc, bytes) }
    } else {
        sliced(crc, bytes)
    }
}

/// A constant of 33 bits, from its five bytes (no literal of an address's
/// size in this crate: hooks_026g.py's layer check).
const fn bits33(b: [u8; 5]) -> i64 {
    i64::from_be_bytes([0, 0, 0, b[0], b[1], b[2], b[3], b[4]])
}

/// The fold's constants (Intel, "Fast CRC Computation for Generic
/// Polynomials Using PCLMULQDQ", 2009, for 04C11DB7h reflected: x^(4*128+32)
/// mod P and x^(4*128-32) mod P, the same for 128, x^64 mod P, P and its
/// Barrett quotient, each bit-reflected and shifted one).
const K1: i64 = bits33([0x01, 0x54, 0x44, 0x2B, 0xD4]);
const K2: i64 = bits33([0x01, 0xC6, 0xE4, 0x15, 0x96]);
const K3: i64 = bits33([0x01, 0x75, 0x19, 0x97, 0xD0]);
const K4: i64 = bits33([0x00, 0xCC, 0xAA, 0x00, 0x9E]);
const K5: i64 = bits33([0x01, 0x63, 0xCD, 0x61, 0x24]);
const P_X: i64 = bits33([0x01, 0xDB, 0x71, 0x06, 0x41]);
const U_PRIME: i64 = bits33([0x01, 0xF7, 0x01, 0x16, 0x41]);

/// The next 16 bytes of `bytes`, and `bytes` past them.
#[target_feature(enable = "sse2")]
fn take(bytes: &mut &[u8]) -> __m128i {
    let (head, rest) = bytes.split_at(16);
    *bytes = rest;
    let head: &[u8; 16] = head.try_into().expect("16 bytes");
    // SAFETY: 16 bytes readable (unaligned load).
    unsafe { _mm_loadu_si128(head.as_ptr().cast()) }
}

/// `a` folded over 128 bits onto `b`: a.low x keys.low ^ a.high x keys.high ^ b.
#[target_feature(enable = "sse2,pclmulqdq")]
fn fold(a: __m128i, b: __m128i, keys: __m128i) -> __m128i {
    let low = _mm_clmulepi64_si128(a, keys, 0x00);
    let high = _mm_clmulepi64_si128(a, keys, 0x11);
    _mm_xor_si128(_mm_xor_si128(b, low), high)
}

/// The step by folding: four lanes of 16 bytes folded 64 bytes on, then
/// one, reduced to 64 bits, to 32, and by Barrett to the CRC; the bytes
/// past the last 16 the sliced way. At least FOLDED_FROM bytes.
#[target_feature(enable = "sse2,pclmulqdq")]
fn folded(crc: u32, mut bytes: &[u8]) -> u32 {
    let mut x3 = take(&mut bytes);
    let mut x2 = take(&mut bytes);
    let mut x1 = take(&mut bytes);
    let mut x0 = take(&mut bytes);
    x3 = _mm_xor_si128(x3, _mm_cvtsi32_si128(crc as i32));
    let k1k2 = _mm_set_epi64x(K2, K1);
    while bytes.len() >= 64 {
        x3 = fold(x3, take(&mut bytes), k1k2);
        x2 = fold(x2, take(&mut bytes), k1k2);
        x1 = fold(x1, take(&mut bytes), k1k2);
        x0 = fold(x0, take(&mut bytes), k1k2);
    }
    let k3k4 = _mm_set_epi64x(K4, K3);
    let mut x = fold(x3, x2, k3k4);
    x = fold(x, x1, k3k4);
    x = fold(x, x0, k3k4);
    while bytes.len() >= 16 {
        x = fold(x, take(&mut bytes), k3k4);
    }
    // 128 bits to 64, then to 32 (with 32 zero bits after the bytes)
    let low32 = _mm_set_epi32(0, 0, 0, !0);
    let x = _mm_xor_si128(_mm_clmulepi64_si128(x, k3k4, 0x10), _mm_srli_si128::<8>(x));
    let x = _mm_xor_si128(_mm_clmulepi64_si128(_mm_and_si128(x, low32), _mm_set_epi64x(0, K5), 0x00),
                          _mm_srli_si128::<4>(x));
    // Barrett
    let pu = _mm_set_epi64x(U_PRIME, P_X);
    let t1 = _mm_clmulepi64_si128(_mm_and_si128(x, low32), pu, 0x10);
    let t2 = _mm_clmulepi64_si128(_mm_and_si128(t1, low32), pu, 0x00);
    let crc = _mm_cvtsi128_si32(_mm_srli_si128::<4>(_mm_xor_si128(x, t2))) as u32;
    sliced(crc, bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The game's step, a byte at a time (0x42050).
    fn bytewise(mut crc: u32, bytes: &[u8]) -> u32 {
        for &b in bytes {
            crc = SLICES[0][((crc ^ u32::from(b)) & 0xFF) as usize] ^ (crc >> 8);
        }
        crc
    }

    fn noise(len: usize) -> [u8; 4096] {
        let mut out = [0u8; 4096];
        let mut x = 0x2545_F491u32 ^ len as u32;
        for b in out.iter_mut() {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            *b = x as u8;
        }
        out
    }

    #[test]
    fn every_way_is_the_byte_step() {
        assert_eq!(SLICES[0][1], 0x7707_3096);
        assert_eq!(!update(!0, b"123456789"), 0xCBF4_3926);
        assert!(folds(), "the test's processor folds (else only the sliced way is tested)");
        let bytes = noise(0);
        for len in [0, 1, 7, 8, 9, 15, 16, 17, 63, 64, 65, 127, 128, 129, 143, 144, 191, 192, 255, 256, 257, 1000,
                    4000, 4089] {
            for start in 0..4 {
                let run = &bytes[start..start + len];
                let crc = 0x1234_5678 ^ (len as u32).wrapping_mul(0x9E37_79B9) ^ start as u32;
                let want = bytewise(crc, run);
                assert_eq!(sliced(crc, run), want, "sliced, {len} bytes from {start}");
                assert_eq!(update(crc, run), want, "update, {len} bytes from {start}");
                if len >= FOLDED_FROM {
                    // SAFETY: folds() above.
                    assert_eq!(unsafe { folded(crc, run) }, want, "folded, {len} bytes from {start}");
                }
            }
        }
    }

    /// Run alone: `cargo test --release -- --ignored --nocapture speed`.
    #[test]
    #[ignore]
    fn speed() {
        extern crate std;
        let mut big = std::vec![0u8; 6 << 20];
        let fill = noise(1);
        for (i, b) in big.iter_mut().enumerate() {
            *b = fill[i % 4096] ^ (i >> 12) as u8;
        }
        for (name, f) in [("bytewise", bytewise as fn(u32, &[u8]) -> u32), ("sliced", sliced), ("update", update)] {
            let start = std::time::Instant::now();
            let mut crc = 0;
            for _ in 0..10 {
                crc = f(crc, &big);
            }
            let per = start.elapsed() / 10;
            std::println!("{name}: {per:?} for 6 MiB ({:.2} GB/s) {crc:08x}",
                          (6 << 20) as f64 / per.as_secs_f64() / 1e9);
        }
    }
}
