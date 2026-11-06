//! The few float types the solver needs, in plain IEEE f32.
//!
//! The crate also runs in the game's arena, where nothing of libm or
//! compiler_builtins is linked: every function here is arithmetic,
//! `sqrt` (one SSE instruction) and polynomials. No `sin`, `atan2` or
//! `floor` of the standard library is called.

use core::ops::{Add, AddAssign, Mul, Neg, Sub, SubAssign};

pub const PI: f32 = core::f32::consts::PI;

/// The square root (`sqrtss`); 0 for a negative or NaN argument.
#[inline]
pub fn sqrt(x: f32) -> f32 {
    if x > 0.0 {
        core::f32::math::sqrt(x)
    } else {
        0.0
    }
}

#[inline]
pub fn abs(x: f32) -> f32 {
    if x < 0.0 { -x } else { x }
}

#[inline]
pub fn min(a: f32, b: f32) -> f32 {
    if a < b { a } else { b }
}

#[inline]
pub fn max(a: f32, b: f32) -> f32 {
    if a > b { a } else { b }
}

/// `x` within `[lo, hi]`; `lo` for NaN.
#[inline]
pub fn clamp(x: f32, lo: f32, hi: f32) -> f32 {
    if x >= lo {
        if x > hi { hi } else { x }
    } else {
        lo
    }
}

/// A finite float (not NaN, not infinite).
#[inline]
pub fn finite(x: f32) -> bool {
    x.is_finite()
}

/// e^-x for x >= 0 (a series on x / 16, squared four times; 0 from 20,
/// where e^-x is 2e-9: past y = 1.25 the series turns and grows again, and
/// its 16th power reached 1e17 by x = 79).
pub fn exp_neg(x: f32) -> f32 {
    if x >= 20.0 {
        return 0.0;
    }
    let y = clamp(x, 0.0, 20.0) * (1.0 / 16.0);
    let mut e =
        1.0 - y + y * y / 2.0 - y * y * y / 6.0 + y * y * y * y / 24.0 - y * y * y * y * y / 120.0;
    for _ in 0..4 {
        e *= e;
    }
    e
}

/// atan2(y, x) within about 1e-5 rad (a minimax polynomial of atan on
/// [0, 1], the octants by symmetry). atan2(0, 0) is 0.
pub fn atan2(y: f32, x: f32) -> f32 {
    let (ax, ay) = (abs(x), abs(y));
    let (lo, hi) = if ax > ay { (ay, ax) } else { (ax, ay) };
    if hi == 0.0 {
        return 0.0;
    }
    let t = lo / hi;
    let s = t * t;
    // atan(t) on [0, 1]
    let mut r = ((((-0.013_480_47 * s + 0.057_477_31) * s - 0.121_239_07) * s + 0.195_635_93) * s
        - 0.332_994_6)
        * s
        + 0.999_995_6;
    r *= t;
    if ay > ax {
        r = PI / 2.0 - r;
    }
    if x < 0.0 {
        r = PI - r;
    }
    if y < 0.0 { -r } else { r }
}

/// sin and cos of `a` (any finite angle; reduced to [-pi, pi] without
/// `floor`), within about 1e-6.
pub fn sin_cos(a: f32) -> (f32, f32) {
    let mut a = a;
    if !finite(a) {
        return (0.0, 1.0);
    }
    let turns = a * (1.0 / (2.0 * PI));
    // round to the nearest integer without floor: the cast truncates toward 0
    let k = (turns + if turns < 0.0 { -0.5 } else { 0.5 }) as i32;
    a -= k as f32 * 2.0 * PI;
    // into [-pi/2, pi/2] with the sign of cos kept apart
    let (b, flip) = if a > PI / 2.0 {
        (PI - a, true)
    } else if a < -PI / 2.0 {
        (-PI - a, true)
    } else {
        (a, false)
    };
    let s2 = b * b;
    let sin = b
        * (1.0
            + s2 * (-1.0 / 6.0
                + s2 * (1.0 / 120.0 + s2 * (-1.0 / 5040.0 + s2 * (1.0 / 362_880.0)))));
    let cos = 1.0
        + s2 * (-0.5
            + s2 * (1.0 / 24.0 + s2 * (-1.0 / 720.0 + s2 * (1.0 / 40_320.0 - s2 / 3_628_800.0))));
    (sin, if flip { -cos } else { cos })
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub const ZERO: Vec3 = Vec3::new(0.0, 0.0, 0.0);
    pub const X: Vec3 = Vec3::new(1.0, 0.0, 0.0);
    pub const Y: Vec3 = Vec3::new(0.0, 1.0, 0.0);
    pub const Z: Vec3 = Vec3::new(0.0, 0.0, 1.0);

    pub const fn new(x: f32, y: f32, z: f32) -> Vec3 {
        Vec3 { x, y, z }
    }

    pub fn dot(self, o: Vec3) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }

    pub fn cross(self, o: Vec3) -> Vec3 {
        Vec3::new(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }

    pub fn length_squared(self) -> f32 {
        self.dot(self)
    }

    pub fn length(self) -> f32 {
        sqrt(self.dot(self))
    }

    /// The unit vector along `self`; `None` when it is too short (or not
    /// finite) to have a direction.
    pub fn direction(self) -> Option<Vec3> {
        let l = self.length();
        if l > 1e-6 && finite(l) {
            Some(self * (1.0 / l))
        } else {
            None
        }
    }

    /// The unit vector along `self`, or `fallback` when it has none.
    pub fn direction_or(self, fallback: Vec3) -> Vec3 {
        self.direction().unwrap_or(fallback)
    }

    /// `self` without its component along the unit `axis`.
    pub fn reject(self, axis: Vec3) -> Vec3 {
        self - axis * self.dot(axis)
    }

    pub fn mul_each(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x * o.x, self.y * o.y, self.z * o.z)
    }

    pub fn is_finite(self) -> bool {
        finite(self.x) && finite(self.y) && finite(self.z)
    }

    /// Any unit vector perpendicular to the unit `self`.
    pub fn any_perpendicular(self) -> Vec3 {
        let other = if abs(self.x) < 0.6 { Vec3::X } else { Vec3::Y };
        self.cross(other).direction_or(Vec3::Z)
    }

    pub fn lerp(self, o: Vec3, t: f32) -> Vec3 {
        self + (o - self) * t
    }
}

impl Add for Vec3 {
    type Output = Vec3;
    fn add(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}

impl Sub for Vec3 {
    type Output = Vec3;
    fn sub(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}

impl Mul<f32> for Vec3 {
    type Output = Vec3;
    fn mul(self, k: f32) -> Vec3 {
        Vec3::new(self.x * k, self.y * k, self.z * k)
    }
}

impl Neg for Vec3 {
    type Output = Vec3;
    fn neg(self) -> Vec3 {
        Vec3::new(-self.x, -self.y, -self.z)
    }
}

impl AddAssign for Vec3 {
    fn add_assign(&mut self, o: Vec3) {
        *self = *self + o;
    }
}

impl SubAssign for Vec3 {
    fn sub_assign(&mut self, o: Vec3) {
        *self = *self - o;
    }
}

/// A unit quaternion: a rotation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quat {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl Quat {
    pub const IDENTITY: Quat = Quat {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        w: 1.0,
    };

    /// The rotation by `angle` about the unit `axis`.
    pub fn from_axis_angle(axis: Vec3, angle: f32) -> Quat {
        let (s, c) = sin_cos(angle * 0.5);
        Quat {
            x: axis.x * s,
            y: axis.y * s,
            z: axis.z * s,
            w: c,
        }
    }

    /// The rotation whose columns (the images of X, Y and Z) are the
    /// orthonormal, right-handed `x`, `y`, `z`.
    pub fn from_basis(x: Vec3, y: Vec3, z: Vec3) -> Quat {
        let (m00, m11, m22) = (x.x, y.y, z.z);
        let trace = m00 + m11 + m22;
        let q = if trace > 0.0 {
            let s = sqrt(trace + 1.0) * 2.0;
            Quat {
                w: 0.25 * s,
                x: (y.z - z.y) / s,
                y: (z.x - x.z) / s,
                z: (x.y - y.x) / s,
            }
        } else if m00 > m11 && m00 > m22 {
            let s = sqrt(1.0 + m00 - m11 - m22) * 2.0;
            Quat {
                w: (y.z - z.y) / s,
                x: 0.25 * s,
                y: (y.x + x.y) / s,
                z: (z.x + x.z) / s,
            }
        } else if m11 > m22 {
            let s = sqrt(1.0 + m11 - m00 - m22) * 2.0;
            Quat {
                w: (z.x - x.z) / s,
                x: (y.x + x.y) / s,
                y: 0.25 * s,
                z: (z.y + y.z) / s,
            }
        } else {
            let s = sqrt(1.0 + m22 - m00 - m11) * 2.0;
            Quat {
                w: (x.y - y.x) / s,
                x: (z.x + x.z) / s,
                y: (z.y + y.z) / s,
                z: 0.25 * s,
            }
        };
        q.normalized()
    }

    pub fn conjugate(self) -> Quat {
        Quat {
            x: -self.x,
            y: -self.y,
            z: -self.z,
            w: self.w,
        }
    }

    pub fn dot(self, o: Quat) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z + self.w * o.w
    }

    /// `self` made unit length; the identity when it has no length.
    pub fn normalized(self) -> Quat {
        let l = sqrt(self.dot(self));
        if l > 1e-9 && finite(l) {
            let k = 1.0 / l;
            Quat {
                x: self.x * k,
                y: self.y * k,
                z: self.z * k,
                w: self.w * k,
            }
        } else {
            Quat::IDENTITY
        }
    }

    /// The vector part.
    pub fn vector(self) -> Vec3 {
        Vec3::new(self.x, self.y, self.z)
    }

    pub fn rotate(self, v: Vec3) -> Vec3 {
        let u = self.vector();
        let t = u.cross(v) * 2.0;
        v + t * self.w + u.cross(t)
    }

    /// `self` turned by the small rotation vector `omega` (a first-order
    /// step: q + 1/2 [omega, 0] q, normalized).
    pub fn integrate(self, omega: Vec3) -> Quat {
        let dq = Quat {
            x: omega.x,
            y: omega.y,
            z: omega.z,
            w: 0.0,
        } * self;
        Quat {
            x: self.x + 0.5 * dq.x,
            y: self.y + 0.5 * dq.y,
            z: self.z + 0.5 * dq.z,
            w: self.w + 0.5 * dq.w,
        }
        .normalized()
    }

    /// The rotation vector (axis times angle, the angle within [0, pi])
    /// that takes `from` to `self`: self = r * from.
    pub fn rotation_from(self, from: Quat) -> Vec3 {
        let mut d = self * from.conjugate();
        if d.w < 0.0 {
            d = Quat {
                x: -d.x,
                y: -d.y,
                z: -d.z,
                w: -d.w,
            };
        }
        let s = d.vector().length();
        if s < 1e-7 {
            return d.vector() * 2.0;
        }
        let angle = 2.0 * atan2(s, d.w);
        d.vector() * (angle / s)
    }

    /// Normalized linear interpolation along the shorter arc.
    pub fn nlerp(self, o: Quat, t: f32) -> Quat {
        let o = if self.dot(o) < 0.0 {
            Quat {
                x: -o.x,
                y: -o.y,
                z: -o.z,
                w: -o.w,
            }
        } else {
            o
        };
        Quat {
            x: self.x + (o.x - self.x) * t,
            y: self.y + (o.y - self.y) * t,
            z: self.z + (o.z - self.z) * t,
            w: self.w + (o.w - self.w) * t,
        }
        .normalized()
    }

    pub fn is_finite(self) -> bool {
        finite(self.x) && finite(self.y) && finite(self.z) && finite(self.w)
    }
}

impl Mul for Quat {
    type Output = Quat;
    fn mul(self, o: Quat) -> Quat {
        Quat {
            w: self.w * o.w - self.x * o.x - self.y * o.y - self.z * o.z,
            x: self.w * o.x + self.x * o.w + self.y * o.z - self.z * o.y,
            y: self.w * o.y - self.x * o.z + self.y * o.w + self.z * o.x,
            z: self.w * o.z + self.x * o.y - self.y * o.x + self.z * o.w,
        }
    }
}

/// A rigid transform: rotate, then translate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub rotation: Quat,
    pub position: Vec3,
}

impl Pose {
    pub const IDENTITY: Pose = Pose {
        rotation: Quat::IDENTITY,
        position: Vec3::ZERO,
    };

    pub fn new(rotation: Quat, position: Vec3) -> Pose {
        Pose { rotation, position }
    }

    pub fn apply(self, p: Vec3) -> Vec3 {
        self.rotation.rotate(p) + self.position
    }

    pub fn inverse(self) -> Pose {
        let r = self.rotation.conjugate();
        Pose {
            rotation: r,
            position: -r.rotate(self.position),
        }
    }

    /// `self` after `inner`: (self * inner)(p) = self(inner(p)).
    pub fn then(self, inner: Pose) -> Pose {
        Pose {
            rotation: (self.rotation * inner.rotation).normalized(),
            position: self.apply(inner.position),
        }
    }

    pub fn is_finite(self) -> bool {
        self.rotation.is_finite() && self.position.is_finite()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32, eps: f32) -> bool {
        abs(a - b) <= eps
    }

    #[test]
    fn exp_neg_matches_std_and_never_grows() {
        let mut last = 1.0f32;
        for i in 0..=8000 {
            let x = i as f32 * 0.01;
            let e = exp_neg(x);
            let want = (-x as f64).exp() as f32;
            assert!(
                close(e, want, 1e-3) && e <= want * 1.01 + 1e-7,
                "exp_neg({x}) = {e}, e^-x = {want}"
            );
            assert!((0.0..=last).contains(&e), "exp_neg({x}) = {e} after {last}");
            last = e;
        }
    }

    #[test]
    fn atan2_and_sin_cos_match_std() {
        for i in -200..=200 {
            for j in [-3.0f32, -1.0, -0.2, 0.0, 0.3, 1.0, 2.5] {
                let y = i as f32 * 0.037;
                assert!(close(atan2(y, j), y.atan2(j), 2e-5), "atan2({y}, {j})");
            }
            let a = i as f32 * 0.21;
            let (s, c) = sin_cos(a);
            assert!(
                close(s, a.sin(), 2e-5) && close(c, a.cos(), 2e-5),
                "sin_cos({a})"
            );
        }
        assert_eq!(atan2(0.0, 0.0), 0.0);
        assert_eq!(sin_cos(f32::NAN), (0.0, 1.0));
    }

    #[test]
    fn quaternions_rotate_compose_and_invert() {
        let q = Quat::from_axis_angle(Vec3::new(1.0, 2.0, 3.0).direction().unwrap(), 1.1);
        let r = Quat::from_axis_angle(Vec3::Y, -0.4);
        let v = Vec3::new(0.3, -1.2, 2.0);
        let a = (q * r).rotate(v);
        let b = q.rotate(r.rotate(v));
        assert!((a - b).length() < 1e-5);
        let back = (q * r).rotation_from(r);
        let expected = Vec3::new(1.0, 2.0, 3.0).direction().unwrap() * 1.1;
        assert!((back - expected).length() < 1e-4, "{back:?}");
        let pose = Pose::new(q, Vec3::new(1.0, 2.0, 3.0));
        assert!((pose.inverse().apply(pose.apply(v)) - v).length() < 1e-5);
        let basis = Quat::from_basis(q.rotate(Vec3::X), q.rotate(Vec3::Y), q.rotate(Vec3::Z));
        assert!(abs(abs(basis.dot(q)) - 1.0) < 1e-5);
    }
}
