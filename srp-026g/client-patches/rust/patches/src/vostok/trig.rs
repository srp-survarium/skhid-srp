//! The engine's `vostok::math::sin`, `cos` and `atan2` (0x38FF0, 0x39000,
//! 0x79C40: `fld`, then the x87's `fsin`, `fcos` or `fpatan`, the result in
//! ST0) as a caller that stores the result as a float gets it, computed with
//! SSE2 doubles where that float is provably the same (patch 134,
//! `animation_math`).
//!
//! The x87 instructions take about 100 cycles each (25 ns on a Ryzen 5900X;
//! these below about 6). Each answer here is `None` unless the float it gives
//! is the one the x87's result rounds to; the caller then asks the game's
//! own function. The argument: the double `y` here is within 2^-50 (relative)
//! of the true value (fdlibm's kernels, < 2^-58, on a reduced argument good to
//! 2^-52), and the x87's within 2^-55 where it is used (Intel's fsin/fcos:
//! one ulp of its 64-bit mantissa after a reduction by a 66-bit pi, an
//! absolute error under 2^-60 for |x| <= 64, which is why a reduced result
//! under 1/32 is refused; fpatan: one ulp). Both round to the same float
//! unless the true value is that close to the midpoint between two floats,
//! so `y` is refused when its 29 bits under a float's 24 are within 2^18 of
//! that midpoint (2^-34 relative, far wider than both errors together).
//!
//! What the arguments can be is not assumed: anything else (NaN, an infinity,
//! |x| > 64, atan2 off its upper half plane) is refused. The tests below
//! check every float of sin and cos (and two billion pairs of atan2) against
//! this CPU's x87 (`cargo test -- --ignored`); zero differ, and with no margin
//! at all zero differ too on the Ryzen.

use core::f64::consts::{FRAC_2_PI, FRAC_PI_2, FRAC_PI_4, PI};

/// pi/2's first 33 bits, and the rest (fdlibm's pio2_1, pio2_1t): k * the
/// first is exact for |k| < 2^20, and x - it is exact for a float x.
const PIO2_1: f64 = 1.570_796_326_734_125_6;
const PIO2_1T: f64 = 6.077_100_506_506_192e-11;
/// 1.5 * 2^52: adding and subtracting it rounds a double to an integer.
const ROUND: f64 = 6_755_399_441_055_744.0;

// fdlibm k_sin.c and k_cos.c, |r| <= pi/4
const S1: f64 = -1.666_666_666_666_663_2e-1;
const S2: f64 = 8.333_333_333_322_49e-3;
const S3: f64 = -1.984_126_982_985_795e-4;
const S4: f64 = 2.755_731_370_707_006_8e-6;
const S5: f64 = -2.505_076_025_340_686_3e-8;
const S6: f64 = 1.589_690_995_211_55e-10;
const C1: f64 = 4.166_666_666_666_66e-2;
const C2: f64 = -1.388_888_888_887_411e-3;
const C3: f64 = 2.480_158_728_947_673e-5;
const C4: f64 = -2.755_731_435_139_066_3e-7;
const C5: f64 = 2.087_572_321_298_175e-9;
const C6: f64 = -1.135_964_755_778_819_5e-11;

// fdlibm s_atan.c
const ATAN_HI: [f64; 4] = [4.636_476_090_008_061e-1, FRAC_PI_4, 9.827_937_232_473_29e-1, FRAC_PI_2];
const ATAN_LO: [f64; 4] = [2.269_877_745_296_168_7e-17, 3.061_616_997_868_383e-17, 1.390_331_103_123_099_8e-17, 6.123_233_995_736_766e-17];
const AT: [f64; 11] = [
    3.333_333_333_333_293e-1, -1.999_999_999_987_648_3e-1, 1.428_571_427_250_346_6e-1, -1.111_111_040_546_235_6e-1,
    9.090_887_133_436_507e-2, -7.691_876_205_044_83e-2, 6.661_073_137_387_531e-2, -5.833_570_133_790_573_5e-2,
    4.976_877_994_615_932_4e-2, -3.653_157_274_421_691_6e-2, 1.628_582_011_536_578_2e-2,
];
const PI_LO: f64 = 1.224_646_799_147_353_2e-16;

/// The widest |x| the sine and cosine take (fsin's reduction error stays
/// under 2^-60 there).
const LARGE: f32 = 64.0;
/// Under this |x| the sine is x and the cosine 1 as floats (x^3/6 and x^2/2
/// are under 2^-41 of them, a float's half step 2^-25).
const SMALL: f32 = 1.0 / 1_048_576.0;
/// A reduced (k != 0) sine or cosine under this is refused: near a zero the
/// x87's 66-bit pi is not good enough to compare with.
const NEAR_ZERO: f64 = 1.0 / 32.0;
/// The low 29 bits of a double (under a float's mantissa), their midpoint and
/// how near it a result is refused.
const LOW: u64 = (1 << 29) - 1;
const MID: i64 = 1 << 28;
const MARGIN: i64 = 1 << 18;

fn kernel_sin(r: f64) -> f64 {
    let z = r * r;
    let v = z * r;
    let p = S2 + z * (S3 + z * (S4 + z * (S5 + z * S6)));
    r + v * (S1 + z * p)
}

fn kernel_cos(r: f64) -> f64 {
    let z = r * r;
    let p = z * (C1 + z * (C2 + z * (C3 + z * (C4 + z * (C5 + z * C6)))));
    1.0 - (0.5 * z - z * p)
}

/// `y` as a float, unless it is too near a float midpoint to be sure.
fn settled(y: f64) -> Option<f32> {
    let low = (y.to_bits() & LOW) as i64;
    if (low - MID).abs() <= MARGIN {
        return None;
    }
    Some(y as f32)
}

#[derive(Clone, Copy)]
enum Wave {
    Sine,
    Cosine,
}

fn wave(x: f32, wave: Wave) -> Option<f32> {
    let ax = x.abs();
    if ax.is_nan() || ax > LARGE {
        return None;
    }
    if ax < SMALL {
        return Some(match wave {
            Wave::Sine => x,
            Wave::Cosine => 1.0,
        });
    }
    let xd = f64::from(x);
    let kd = (xd * FRAC_2_PI + ROUND) - ROUND;
    let r = (xd - kd * PIO2_1) - kd * PIO2_1T;
    let k = kd as i32;
    let quarter = (k + match wave {
        Wave::Sine => 0,
        Wave::Cosine => 1,
    }) & 3;
    let y = match quarter {
        0 => kernel_sin(r),
        1 => kernel_cos(r),
        2 => -kernel_sin(r),
        _ => -kernel_cos(r),
    };
    if k != 0 && y.abs() < NEAR_ZERO {
        return None;
    }
    settled(y)
}

/// `fsin` of `x` as a float, when that is sure.
pub fn sin(x: f32) -> Option<f32> {
    wave(x, Wave::Sine)
}

/// `fcos` of `x` as a float, when that is sure.
pub fn cos(x: f32) -> Option<f32> {
    wave(x, Wave::Cosine)
}

/// atan(t) for t >= 0 (fdlibm).
fn atan_positive(t: f64) -> f64 {
    let (part, t) = if t < 0.4375 {
        (None, t)
    } else if t < 0.6875 {
        (Some(0), (2.0 * t - 1.0) / (2.0 + t))
    } else if t < 1.1875 {
        (Some(1), (t - 1.0) / (t + 1.0))
    } else if t < 2.4375 {
        (Some(2), (t - 1.5) / (1.0 + 1.5 * t))
    } else {
        (Some(3), -1.0 / t)
    };
    let z = t * t;
    let w = z * z;
    let s1 = z * (AT[0] + w * (AT[2] + w * (AT[4] + w * (AT[6] + w * (AT[8] + w * AT[10])))));
    let s2 = w * (AT[1] + w * (AT[3] + w * (AT[5] + w * (AT[7] + w * AT[9]))));
    match part {
        None => t - t * (s1 + s2),
        Some(i) => ATAN_HI[i] - ((t * (s1 + s2) - ATAN_LO[i]) - t),
    }
}

/// `fpatan` of (`y`, `x`) -- the angle of the point (x, y) -- as a float,
/// when that is sure: y > 0 and finite, x finite and not 0.
pub fn atan2(y: f32, x: f32) -> Option<f32> {
    if !(y > 0.0 && y <= f32::MAX && x.abs() <= f32::MAX) {
        return None;
    }
    let (yd, xd) = (f64::from(y), f64::from(x));
    let angle = if x > 0.0 {
        atan_positive(yd / xd)
    } else if x < 0.0 {
        (PI - atan_positive(yd / -xd)) + PI_LO
    } else {
        return None;
    };
    // a quotient past the doubles' range (y / x under 2^-1022) is not asked
    if angle.is_nan() || angle < 1.0e-30 {
        return None;
    }
    settled(angle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::arch::asm;

    /// The x87's answer, stored as a float (as the game's callers store it).
    fn x87(x: f32, op: u8) -> f32 {
        let mut out = 0f32;
        // SAFETY: loads one float, runs one x87 instruction on it and stores
        // the result, leaving the x87 stack as it was.
        unsafe {
            match op {
                0 => asm!("fld dword ptr [{i}]", "fsin", "fstp dword ptr [{o}]", i = in(reg) &x, o = in(reg) &mut out,
                          out("st(0)") _, out("st(1)") _, out("st(2)") _, out("st(3)") _,
                          out("st(4)") _, out("st(5)") _, out("st(6)") _, out("st(7)") _),
                _ => asm!("fld dword ptr [{i}]", "fcos", "fstp dword ptr [{o}]", i = in(reg) &x, o = in(reg) &mut out,
                          out("st(0)") _, out("st(1)") _, out("st(2)") _, out("st(3)") _,
                          out("st(4)") _, out("st(5)") _, out("st(6)") _, out("st(7)") _),
            }
        }
        out
    }

    fn x87_atan2(y: f32, x: f32) -> f32 {
        let mut out = 0f32;
        // SAFETY: as x87 above: the game's `fld y / fld x / fpatan`.
        unsafe {
            asm!("fld dword ptr [{y}]", "fld dword ptr [{x}]", "fpatan", "fstp dword ptr [{o}]",
                 y = in(reg) &y, x = in(reg) &x, o = in(reg) &mut out,
                 out("st(0)") _, out("st(1)") _, out("st(2)") _, out("st(3)") _,
                 out("st(4)") _, out("st(5)") _, out("st(6)") _, out("st(7)") _);
        }
        out
    }

    /// Every float from `from` by `step` (bits): the answers given are the x87's; the count refused.
    fn waves(from: u32, step: u32) -> u64 {
        let mut refused = 0;
        let mut bits = from as u64;
        while bits <= u64::from(u32::MAX) {
            let x = f32::from_bits(bits as u32);
            for (op, mine) in [(0, sin(x)), (1, cos(x))] {
                match mine {
                    Some(v) => assert_eq!(v.to_bits(), x87(x, op).to_bits(), "op {op} at {x:e} ({bits:#x})"),
                    None => refused += 1,
                }
            }
            bits += u64::from(step);
        }
        refused
    }

    #[test]
    fn sine_and_cosine_are_the_x87s() {
        waves(0, 4099);
    }

    #[test]
    fn the_small_and_special_ones() {
        for x in [0.0f32, -0.0, 1e-30, -1e-30, 1.0 / 2_097_152.0, f32::MIN_POSITIVE, 1e-45] {
            assert_eq!(sin(x).map(f32::to_bits), Some(x87(x, 0).to_bits()), "{x:e}");
            assert_eq!(cos(x).map(f32::to_bits), Some(x87(x, 1).to_bits()), "{x:e}");
        }
        for x in [f32::NAN, f32::INFINITY, -f32::INFINITY, 64.5, -1e9] {
            assert_eq!((sin(x), cos(x)), (None, None), "{x:e}");
        }
        // a multiple of pi, and of pi/2: near a zero, refused
        assert_eq!(sin(core::f32::consts::PI), None);
        assert_eq!(cos(core::f32::consts::FRAC_PI_2), None);
        assert_eq!(atan2(0.0, 1.0), None);
        assert_eq!(atan2(-1.0, 1.0), None);
        assert_eq!(atan2(1.0, 0.0), None);
        assert_eq!(atan2(f32::NAN, 1.0), None);
    }

    #[test]
    fn most_are_answered() {
        // the animation's half angles: refused almost never
        let (mut asked, mut refused) = (0, 0);
        let mut x = -7.0f32;
        while x < 7.0 {
            asked += 2;
            refused += u32::from(sin(x).is_none()) + u32::from(cos(x).is_none());
            x += 0.000_37;
        }
        assert!(refused * 50 < asked, "{refused} of {asked}");
    }

    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
        fn unit(&mut self) -> f32 {
            (self.next() >> 40) as f32 / 16_777_216.0
        }
    }

    /// `count` pairs of four kinds: a unit quaternion's |xyz| and w (the
    /// game's), any bits, small y, and |y| about |x| (atan near pi/4, 3pi/4).
    fn pairs(seed: u64, count: u64) -> u64 {
        let mut rng = Rng(seed);
        let mut refused = 0;
        for i in 0..count {
            let (y, x) = match i % 4 {
                0 => {
                    let q = [rng.unit() * 2.0 - 1.0, rng.unit() * 2.0 - 1.0, rng.unit() * 2.0 - 1.0, rng.unit() * 2.0 - 1.0];
                    let n = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
                    let q = q.map(|c| c / n);
                    ((q[0] * q[0] + q[1] * q[1] + q[2] * q[2]).sqrt(), q[3])
                }
                1 => (f32::from_bits(rng.next() as u32), f32::from_bits(rng.next() as u32)),
                2 => (rng.unit() * 1e-3, rng.unit() * 2.0 - 1.0),
                _ => {
                    let x = rng.unit() * 2.0 - 1.0;
                    (x.abs() * (1.0 + (rng.unit() - 0.5) * 1e-5), if rng.next() & 1 == 0 { x } else { -x })
                }
            };
            match atan2(y, x) {
                Some(v) => assert_eq!(v.to_bits(), x87_atan2(y, x).to_bits(), "atan2({y:e}, {x:e})"),
                None => refused += 1,
            }
        }
        refused
    }

    #[test]
    fn atan2_is_the_x87s() {
        pairs(0x9E37_79B9_7F4A_7C15, 2_000_000);
    }

    /// Every float (sin and cos) and 2.4 billion pairs (atan2): about a minute on 12 threads.
    #[test]
    #[ignore]
    fn all_of_them() {
        let threads: Vec<_> = (0..12u32)
            .map(|t| std::thread::spawn(move || (waves(t, 12), pairs(0x9E37_79B9_7F4A_7C15 ^ u64::from(t), 200_000_000))))
            .collect();
        for t in threads {
            t.join().unwrap();
        }
    }
}
