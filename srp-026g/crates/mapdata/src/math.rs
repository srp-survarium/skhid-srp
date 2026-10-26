//! The engine's vector math, as far as map data needs it.
//!
//! # Frames
//!
//! Everything in this crate is in the **map frame**: the frame of the
//! project files (`client_project`) and the meshes. It is left-handed, Y up,
//! Z forward; a rotation is three Euler angles turned into a matrix by the
//! engine's `math::create_rotation(float3)` ([`Rotation::from_angles`]), and
//! points are row vectors: `world = local * R + position`.
//!
//! The physics engine (Bullet) works in its own frame, the map frame with z
//! negated. The client's `bullet_character_controller` state that the
//! patched clients upload (`m_last_pos`, `docs/notes/player_bots.md` §4) is
//! in that frame, and it is the capsule's centre, not the feet. Use
//! [`PhysicsPoint`] for such values so the flip is never forgotten.

use std::ops::{Add, Mul, Neg, Sub};

/// A point or direction in the map frame (metres).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub const ZERO: Self = Self::new(0.0, 0.0, 0.0);
    pub const ONE: Self = Self::new(1.0, 1.0, 1.0);
    pub const Y: Self = Self::new(0.0, 1.0, 0.0);

    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub const fn from_array([x, y, z]: [f32; 3]) -> Self {
        Self { x, y, z }
    }

    pub const fn to_array(self) -> [f32; 3] {
        [self.x, self.y, self.z]
    }

    pub fn dot(self, other: Self) -> f32 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    pub fn cross(self, other: Self) -> Self {
        Self::new(
            self.y * other.z - self.z * other.y,
            self.z * other.x - self.x * other.z,
            self.x * other.y - self.y * other.x,
        )
    }

    pub fn length(self) -> f32 {
        self.dot(self).sqrt()
    }

    /// The distance on the ground plane (x, z).
    ///
    /// The same value as `(self.x - other.x).hypot(self.z - other.z)` on glibc 2.35 and later, whose `hypotf` is
    /// this formula (the squares of two floats are exact in a double, the sum and the root are one rounding each,
    /// the result one more); done here so the search's heuristic (a call for every floor it pushes) is not a call
    /// into the libm, and so the result is the same whichever libm the build runs on.
    pub fn distance_xz(self, other: Self) -> f32 {
        let dx = f64::from(self.x - other.x);
        let dz = f64::from(self.z - other.z);
        (dx * dx + dz * dz).sqrt() as f32
    }

    pub fn min(self, other: Self) -> Self {
        Self::new(
            self.x.min(other.x),
            self.y.min(other.y),
            self.z.min(other.z),
        )
    }

    pub fn max(self, other: Self) -> Self {
        Self::new(
            self.x.max(other.x),
            self.y.max(other.y),
            self.z.max(other.z),
        )
    }

    /// Component by axis: 0 x, 1 y, 2 z.
    pub fn axis(self, axis: usize) -> f32 {
        match axis {
            0 => self.x,
            1 => self.y,
            _ => self.z,
        }
    }

    /// Per-component product.
    pub fn scaled(self, scale: Self) -> Self {
        Self::new(self.x * scale.x, self.y * scale.y, self.z * scale.z)
    }

    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }
}

impl Add for Vec3 {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self::new(self.x + other.x, self.y + other.y, self.z + other.z)
    }
}

impl Sub for Vec3 {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        Self::new(self.x - other.x, self.y - other.y, self.z - other.z)
    }
}

impl Mul<f32> for Vec3 {
    type Output = Self;
    fn mul(self, k: f32) -> Self {
        Self::new(self.x * k, self.y * k, self.z * k)
    }
}

impl Neg for Vec3 {
    type Output = Self;
    fn neg(self) -> Self {
        Self::new(-self.x, -self.y, -self.z)
    }
}

/// A point in the physics (Bullet) frame: the map frame with z negated.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PhysicsPoint(pub Vec3);

impl PhysicsPoint {
    pub fn from_map(point: Vec3) -> Self {
        Self(Vec3::new(point.x, point.y, -point.z))
    }

    pub fn to_map(self) -> Vec3 {
        Vec3::new(self.0.x, self.0.y, -self.0.z)
    }
}

/// A rotation matrix with rows `i`, `j`, `k` (where local x, y, z go).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rotation {
    pub i: Vec3,
    pub j: Vec3,
    pub k: Vec3,
}

impl Rotation {
    pub const IDENTITY: Self = Self {
        i: Vec3::new(1.0, 0.0, 0.0),
        j: Vec3::new(0.0, 1.0, 0.0),
        k: Vec3::new(0.0, 0.0, 1.0),
    };

    /// `math::create_rotation(float3 const& angles)`
    /// (`math_float4x4_inline.h:253`): `Rx(a.x) · Ry(a.y) · Rz(a.z)` for row
    /// vectors, so x turns first.
    pub fn from_angles(angles: Vec3) -> Self {
        let (xs, xc) = angles.x.sin_cos();
        let (ys, yc) = angles.y.sin_cos();
        let (zs, zc) = angles.z.sin_cos();
        Self {
            i: Vec3::new(yc * zc, -yc * zs, ys),
            j: Vec3::new(ys * xs * zc + xc * zs, -xs * ys * zs + xc * zc, -xs * yc),
            k: Vec3::new(-xc * ys * zc + xs * zs, xc * ys * zs + xs * zc, xc * yc),
        }
    }

    /// `v * R`.
    pub fn apply(&self, v: Vec3) -> Vec3 {
        self.i * v.x + self.j * v.y + self.k * v.z
    }

    /// `self` then `then` (row vectors: `v * self * then`).
    pub fn then(&self, then: &Self) -> Self {
        Self {
            i: then.apply(self.i),
            j: then.apply(self.j),
            k: then.apply(self.k),
        }
    }

    /// The inverse (the transpose; rows are orthonormal).
    pub fn transposed(&self) -> Self {
        Self {
            i: Vec3::new(self.i.x, self.j.x, self.k.x),
            j: Vec3::new(self.i.y, self.j.y, self.k.y),
            k: Vec3::new(self.i.z, self.j.z, self.k.z),
        }
    }
}

/// How the engine places a collision object: local point × the library's
/// `#[sx][sy][sz]` scale (already in the library's vertices here) ×
/// [`Rotation::from_angles`] + position. The object's own `scale` field is
/// **not** applied: physics keeps only rotation and translation of the
/// object matrix. (0.100b srp's `combat/collision.rs` applied it on top of
/// the suffix: scaled twice.)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    pub rotation: Rotation,
    pub position: Vec3,
}

impl Transform {
    pub fn new(angles: Vec3, position: Vec3) -> Self {
        Self {
            rotation: Rotation::from_angles(angles),
            position,
        }
    }

    pub fn apply(&self, local: Vec3) -> Vec3 {
        self.rotation.apply(local) + self.position
    }
}

/// Where an object faces: local +z through its rotation (map frame).
pub fn forward(angles: Vec3) -> Vec3 {
    Rotation::from_angles(angles).k
}

/// The heading of a direction on the ground plane, by the engine's
/// `create_rotation_y` convention: angle `a` turns +z to `(-sin a, 0, cos a)`,
/// so `a = atan2(-x, z)`. (How a client's input yaw relates to this angle is
/// not checked here.)
pub fn heading(direction: Vec3) -> f32 {
    (-direction.x).atan2(direction.z)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Vec3, b: Vec3) -> bool {
        (a - b).length() < 1e-5
    }

    #[test]
    fn the_distance_on_the_ground_plane_is_the_root_of_the_squares() {
        let d = |a: (f32, f32), b: (f32, f32)| {
            Vec3::new(a.0, 7.0, a.1).distance_xz(Vec3::new(b.0, -3.0, b.1))
        };
        assert_eq!(d((0.0, 0.0), (3.0, 4.0)), 5.0);
        assert_eq!(d((3.0, 4.0), (0.0, 0.0)), 5.0);
        assert_eq!(d((-3.0, -4.0), (3.0, 4.0)), 10.0);
        assert_eq!(d((1.5, 2.5), (1.5, 2.5)), 0.0);
        assert_eq!(d((0.0, 0.0), (-0.0, 0.0)), 0.0);
        // Beyond a float's range squared, and below it: a double does not overflow or flush them.
        for scale in [2f32.powi(100), 2f32.powi(-100)] {
            assert_eq!(d((3.0 * scale, 0.0), (0.0, 4.0 * scale)), 5.0 * scale);
        }
        assert!(d((f32::NAN, 0.0), (0.0, 0.0)).is_nan());
    }

    /// `distance_xz` is `f32::hypot` to the last bit where `hypotf` is glibc's (2.35 and later): the saving of
    /// the call must not move a cost the search compares. Random, wide-range and grid-like inputs.
    #[test]
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    fn the_distance_on_the_ground_plane_is_hypot_to_the_last_bit() {
        let mut s = 88172645463325252u64;
        let mut unit = move || {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            (s >> 11) as f64 * (1.0 / 9007199254740992.0)
        };
        let check = |x: f32, z: f32| {
            let new = Vec3::new(x, 0.0, z).distance_xz(Vec3::ZERO);
            let old = x.hypot(z);
            assert_eq!(new.to_bits(), old.to_bits(), "{x:?} {z:?}: {new:?} {old:?}");
        };
        for _ in 0..2_000_000 {
            check((unit() * 600.0 - 300.0) as f32, (unit() * 600.0 - 300.0) as f32);
        }
        for _ in 0..1_000_000 {
            let wide = |u: f64, v: f64| ((u * 2.0 - 1.0) * 10f64.powf(v * 9.0 - 6.0)) as f32;
            check(wide(unit(), unit()), wide(unit(), unit()));
        }
        for _ in 0..1_000_000 {
            // Cell centres of a 0.25 m grid against a goal anywhere.
            let (gx, gz) = ((unit() * 400.0 - 200.0) as f32, (unit() * 400.0 - 200.0) as f32);
            let (a, b) = ((unit() * 1600.0) as i32 - 800, (unit() * 1600.0) as i32 - 800);
            check(-200.0 + (a as f32 + 0.5) * 0.25 - gx, -200.0 + (b as f32 + 0.5) * 0.25 - gz);
        }
    }

    fn rotation_x(a: f32) -> Rotation {
        let (s, c) = a.sin_cos();
        Rotation {
            i: Vec3::new(1.0, 0.0, 0.0),
            j: Vec3::new(0.0, c, -s),
            k: Vec3::new(0.0, s, c),
        }
    }

    fn rotation_y(a: f32) -> Rotation {
        let (s, c) = a.sin_cos();
        Rotation {
            i: Vec3::new(c, 0.0, s),
            j: Vec3::new(0.0, 1.0, 0.0),
            k: Vec3::new(-s, 0.0, c),
        }
    }

    fn rotation_z(a: f32) -> Rotation {
        let (s, c) = a.sin_cos();
        Rotation {
            i: Vec3::new(c, -s, 0.0),
            j: Vec3::new(s, c, 0.0),
            k: Vec3::new(0.0, 0.0, 1.0),
        }
    }

    #[test]
    fn create_rotation_is_x_then_y_then_z() {
        for angles in [
            Vec3::new(0.3, -1.1, 2.0),
            Vec3::new(3.1, -0.639, -3.0),
            Vec3::new(-0.2, 0.7, 0.05),
        ] {
            let r = Rotation::from_angles(angles);
            let p = rotation_x(angles.x)
                .then(&rotation_y(angles.y))
                .then(&rotation_z(angles.z));
            assert!(close(r.i, p.i) && close(r.j, p.j) && close(r.k, p.k));
            let v = Vec3::new(0.4, -2.0, 1.5);
            assert!(close(r.transposed().apply(r.apply(v)), v));
        }
    }

    #[test]
    fn heading_follows_create_rotation_y() {
        for a in [0.0f32, 0.5, -1.2, 2.9] {
            let f = forward(Vec3::new(0.0, a, 0.0));
            assert!(close(f, Vec3::new(-a.sin(), 0.0, a.cos())));
            assert!((heading(f) - a).abs() < 1e-5);
        }
    }

    #[test]
    fn physics_frame_flips_z() {
        let p = Vec3::new(1.0, 2.0, 3.0);
        assert_eq!(PhysicsPoint::from_map(p).0, Vec3::new(1.0, 2.0, -3.0));
        assert_eq!(PhysicsPoint::from_map(p).to_map(), p);
    }

    #[test]
    fn transform_rotates_then_moves() {
        let place = Transform::new(Vec3::new(0.0, std::f32::consts::FRAC_PI_2, 0.0), Vec3::Y);
        // Row vectors: local +x goes to row i = (cos, 0, sin) of a yaw.
        assert!(close(
            place.apply(Vec3::new(1.0, 0.0, 0.0)),
            Vec3::new(0.0, 1.0, 1.0)
        ));
    }
}
