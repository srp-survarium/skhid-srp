//! The game's transforms: row vectors (`v · M`), three rows of rotation and
//! scale, then the translation. The engine's `float4x4` is the same with a
//! last column of 0, 0, 0, 1.

/// An affine transform, applied to a row vector from the left.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Affine {
    pub x: [f32; 3],
    pub y: [f32; 3],
    pub z: [f32; 3],
    pub t: [f32; 3],
}

impl Default for Affine {
    fn default() -> Affine {
        Affine::IDENTITY
    }
}

impl Affine {
    pub const IDENTITY: Affine =
        Affine { x: [1.0, 0.0, 0.0], y: [0.0, 1.0, 0.0], z: [0.0, 0.0, 1.0], t: [0.0, 0.0, 0.0] };

    /// The half turn about y every locator's frame takes: the exe's own
    /// `create_rotation_y(pi)` inside `weapon::calculate_locator` (0x243BDC),
    /// rows (-1, 0, 0), (0, 1, 0), (0, 0, -1).
    pub const HALF_TURN: Affine =
        Affine { x: [-1.0, 0.0, 0.0], y: [0.0, 1.0, 0.0], z: [0.0, 0.0, -1.0], t: [0.0, 0.0, 0.0] };

    pub const fn translation(x: f32, y: f32, z: f32) -> Affine {
        Affine { t: [x, y, z], ..Affine::IDENTITY }
    }

    /// `v` through the rotation rows alone.
    fn turn(&self, v: [f32; 3]) -> [f32; 3] {
        let ([a, b, c], [x0, x1, x2], [y0, y1, y2], [z0, z1, z2]) = (v, self.x, self.y, self.z);
        [a * x0 + b * y0 + c * z0, a * x1 + b * y1 + c * z1, a * x2 + b * y2 + c * z2]
    }

    /// `self · next`: `self` first, then `next`.
    pub fn then(&self, next: &Affine) -> Affine {
        let [t0, t1, t2] = next.turn(self.t);
        let [n0, n1, n2] = next.t;
        Affine { x: next.turn(self.x), y: next.turn(self.y), z: next.turn(self.z), t: [t0 + n0, t1 + n1, t2 + n2] }
    }

    /// The inverse; None for a transform that has none (a flat one, or one
    /// that is not finite).
    pub fn inverse(&self) -> Option<Affine> {
        let (a, b, c) = (self.x, self.y, self.z);
        let (bc, ca, ab) = (cross(b, c), cross(c, a), cross(a, b));
        let det = dot(a, bc);
        if !(det.is_finite() && det.abs() > f32::MIN_POSITIVE) {
            return None;
        }
        let k = 1.0 / det;
        let ([p0, p1, p2], [q0, q1, q2], [r0, r1, r2]) = (bc, ca, ab);
        let rotation = Affine {
            x: [p0 * k, q0 * k, r0 * k],
            y: [p1 * k, q1 * k, r1 * k],
            z: [p2 * k, q2 * k, r2 * k],
            t: [0.0; 3],
        };
        let [t0, t1, t2] = rotation.turn(self.t);
        let out = Affine { t: [-t0, -t1, -t2], ..rotation };
        out.is_finite().then_some(out)
    }

    pub fn is_finite(&self) -> bool {
        self.floats().iter().all(|f| f.is_finite())
    }

    /// The twelve floats in the table's order: x, y, z, t.
    pub fn floats(&self) -> [f32; 12] {
        let ([a, b, c], [d, e, f], [g, h, i], [j, k, l]) = (self.x, self.y, self.z, self.t);
        [a, b, c, d, e, f, g, h, i, j, k, l]
    }

    pub fn from_floats(floats: [f32; 12]) -> Affine {
        let [a, b, c, d, e, f, g, h, i, j, k, l] = floats;
        Affine { x: [a, b, c], y: [d, e, f], z: [g, h, i], t: [j, k, l] }
    }

    /// The engine's `float4x4` (a locator record's +20h, a model's world
    /// matrix).
    pub fn matrix(&self) -> [f32; 16] {
        let ([a, b, c], [d, e, f], [g, h, i], [j, k, l]) = (self.x, self.y, self.z, self.t);
        [a, b, c, 0.0, d, e, f, 0.0, g, h, i, 0.0, j, k, l, 1.0]
    }

    /// An engine `float4x4` without its last column.
    pub fn from_matrix(m: &[f32; 16]) -> Affine {
        let [a, b, c, _, d, e, f, _, g, h, i, _, j, k, l, _] = *m;
        Affine { x: [a, b, c], y: [d, e, f], z: [g, h, i], t: [j, k, l] }
    }
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    let ([a0, a1, a2], [b0, b1, b2]) = (a, b);
    [a1 * b2 - a2 * b1, a2 * b0 - a0 * b2, a0 * b1 - a1 * b0]
}

pub(crate) fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    let ([a0, a1, a2], [b0, b1, b2]) = (a, b);
    a0 * b0 + a1 * b1 + a2 * b2
}
