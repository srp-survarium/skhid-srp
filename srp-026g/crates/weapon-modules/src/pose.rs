//! Where things are, in the client's own terms. The game keeps a locator as
//! a record: an offset matrix and the bone of the gun's model it hangs on
//! (`weapon::load_weapon` 0x243A30: the barrel, the scope and the aim), and
//! evaluates it each frame as `weapon::calculate_locator` 0x243BC0 does:
//!
//! ```text
//! frame = extra · half turn · offset · bones[bone] · world      (row vectors)
//! ```
//!
//! A [`Placement`] is such a record's content. A model drawn at a frame has
//! its own locators by the same rule, so a locator of a part on a part on
//! the gun is again one record on the gun's bone ([`Placement::child`]):
//! nothing is evaluated per part and frame but the record.

use crate::BoneIndex;
use crate::math::Affine;
use crate::table::HAND_LOCATORS;

/// The fingers the game corrects: thumb 1..3, index, middle, ring, pinky.
pub const FINGERS: usize = 15;

/// A locator record's content: an offset on a bone of the gun's model.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub bone: BoneIndex,
    pub offset: Affine,
}

impl Placement {
    /// A locator of the model drawn at this placement, as a placement of its
    /// own: `locator · half turn · offset` on the same bone. The table's
    /// locators are written for this rule (the porter's renders check each
    /// against 0.69d's); it is the one place that says how a part's own
    /// locators stand on it.
    pub fn child(&self, locator: &Affine) -> Placement {
        Placement { bone: self.bone, offset: locator.then(&Affine::HALF_TURN).then(&self.offset) }
    }

    /// The way from this placement to `other` at `t` (0 this, 1 `other`):
    /// the rotation turned along the shorter arc (the normalised blend of
    /// the two quaternions: no trigonometry in `no_std`; for the 45 degrees
    /// of a side sight its angle is within 2 percent of the arc's), the
    /// place on the line between; None when they hang on different bones or
    /// either offset is no rotation (scaled or skewed). 0.69d blends the two
    /// aim slots' alignments by its switch's fraction
    /// (`align_weapon_bone_to_aim` 0x2002b8cf0); this is that blend on the
    /// eye's record, whose alignment the game makes.
    pub fn toward(&self, other: &Placement, t: f32) -> Option<Placement> {
        if self.bone != other.bone || !t.is_finite() {
            return None;
        }
        let t = t.clamp(0.0, 1.0);
        let (a, b) = (Quaternion::of(&self.offset)?, Quaternion::of(&other.offset)?);
        let turned = a.slerp(b, t).rows();
        let (from, to) = (self.offset.t, other.offset.t);
        let place = [from[0] + (to[0] - from[0]) * t, from[1] + (to[1] - from[1]) * t, from[2] + (to[2] - from[2]) * t];
        let [x, y, z] = turned;
        let offset = Affine { x, y, z, t: place };
        offset.is_finite().then_some(Placement { bone: self.bone, offset })
    }

    /// The matrix `k` with this placement's frame = `k` · `other`'s frame in
    /// every pose, which there is when both hang on one bone: half turn ·
    /// offset · inverse(other's offset) · half turn.
    pub fn frame_over(&self, other: &Placement) -> Option<Affine> {
        let between = self.offset.then(&other.offset.inverse()?);
        (self.bone == other.bone).then(|| Affine::HALF_TURN.then(&between).then(&Affine::HALF_TURN))
    }
}

/// `weapon::calculate_locator` 0x243BC0: a record's frame for the gun's
/// bones of this frame (`bone`, a matrix in the model's space) and the gun's
/// world matrix. The engine's formula, for the porter's renders and this
/// crate's tests; the game runs its own.
pub fn locator_frame(extra: &Affine, at: &Placement, bone: &Affine, world: &Affine) -> Affine {
    extra.then(&Affine::HALF_TURN).then(&at.offset).then(bone).then(world)
}

/// What the eye is fixed to while aiming through an optic. The game moves
/// the gun until the eye's frame sits on the camera
/// (`calculate_aim_alignment` 0x244D40), so whatever the eye's record hangs
/// on stands still on the screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EyeAnchor {
    /// The optic itself: its reticle is at the screen's centre at every
    /// moment of every clip, and what a clip does to the optic's bone (the
    /// shot's kick) does not show while aiming.
    Optic,
    /// The bone the gun's own `aim` hangs on, set so that the optic is
    /// centred in the gun's aimed idle pose: a clip that moves the optic's
    /// bone against it (the kick) shows, as it does over iron sights.
    GunIdle,
}

/// An optic's eye, either way.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Eye {
    optic: Placement,
    gun_idle: Placement,
}

impl Eye {
    /// The eye at `eye` (the optic's own locator, already a placement on the
    /// optic's bone), for a gun whose bones stand at `optic_bone` and
    /// `aim_bone` (matrices in the model's space) in its aimed idle pose.
    /// None when the aim bone's pose has no inverse.
    pub(crate) fn new(eye: Placement, optic_bone: &Affine, aim_bone: (BoneIndex, &Affine)) -> Option<Eye> {
        let offset = eye.offset.then(optic_bone).then(&aim_bone.1.inverse()?);
        Some(Eye { optic: eye, gun_idle: Placement { bone: aim_bone.0, offset } })
    }

    /// The aim record's content.
    pub fn placed(&self, anchor: EyeAnchor) -> Placement {
        match anchor {
            EyeAnchor::Optic => self.optic,
            EyeAnchor::GunIdle => self.gun_idle,
        }
    }
}

/// The left hand on a grip.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hand {
    /// The hand's locator: the hand IK's idle record.
    pub wrist: Placement,
    fingers: [Affine; FINGERS],
}

impl Hand {
    /// The hand stands at the grip's position in the gun's axes, not turned
    /// with the grip: laid over the thirteen grips' meshes, the locators fit
    /// that way, and the one grip whose frame is turned on its gun (the PKP's,
    /// a quarter turn) holds the hand 139 mm off it otherwise.
    pub(crate) fn new(grip: &Placement, locators: &[Affine; HAND_LOCATORS]) -> Option<Hand> {
        let [.., wrist] = locators;
        let [x, y, z] = grip.offset.t;
        let at = Placement { bone: grip.bone, offset: Affine::translation(x, y, z) };
        Some(Hand { wrist: at.child(wrist), fingers: fingers(locators)? })
    }

    /// The finger corrector's fifteen matrices: the grip's own, whole. The
    /// game writes a finger's matrix with its translation
    /// (`fingers_to_weapon_corrector`'s tick 0x390900), which is the joint's
    /// place in the hand's skeleton; the grips' locators carry 0.26g's own
    /// hand's joints (all thirteen of 0.69d0's, within 3.4 micrometres of
    /// its bind pose), so nothing of the gun's own set is kept.
    pub fn fingers(&self) -> [Affine; FINGERS] {
        self.fingers
    }
}

/// What `fingers_to_weapon_corrector::set_weapon_model` 0x390620 keeps of
/// sixteen locators: each finger's times the inverse of its parent's (a
/// finger's first joint the hand's, the next the joint before). None when a
/// parent has no inverse.
pub(crate) fn fingers(locators: &[Affine; HAND_LOCATORS]) -> Option<[Affine; FINGERS]> {
    let [joints @ .., wrist] = locators;
    let mut out = [Affine::IDENTITY; FINGERS];
    let mut parent = *wrist;
    for (nth, (joint, relative)) in joints.iter().zip(&mut out).enumerate() {
        if nth.is_multiple_of(3) {
            parent = *wrist;
        }
        *relative = joint.then(&parent.inverse()?);
        parent = *joint;
    }
    Some(out)
}

/// A unit quaternion (w, x, y, z) of a rotation's rows (row vectors).
#[derive(Clone, Copy, Debug)]
struct Quaternion([f32; 4]);

impl Quaternion {
    /// The rotation of `m`'s rows; None when they are not orthonormal
    /// (within 1e-3) or turn the hand.
    fn of(m: &Affine) -> Option<Quaternion> {
        let rows = [m.x, m.y, m.z];
        let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        for (i, a) in rows.iter().enumerate() {
            for (j, b) in rows.iter().enumerate() {
                let want = if i == j { 1.0 } else { 0.0 };
                if (dot(*a, *b) - want).abs() > 1e-3 {
                    return None;
                }
            }
        }
        let cross = [m.x[1] * m.y[2] - m.x[2] * m.y[1], m.x[2] * m.y[0] - m.x[0] * m.y[2], m.x[0] * m.y[1] - m.x[1] * m.y[0]];
        if dot(cross, m.z) < 0.0 {
            return None;
        }
        // The column-vector matrix is the transpose of the rows.
        let r = |i: usize, j: usize| rows.get(j).and_then(|row| row.get(i)).copied().unwrap_or(0.0);
        let trace = r(0, 0) + r(1, 1) + r(2, 2);
        let q = if trace > 0.0 {
            let s = (trace + 1.0).sqrt_newton() * 2.0;
            [0.25 * s, (r(2, 1) - r(1, 2)) / s, (r(0, 2) - r(2, 0)) / s, (r(1, 0) - r(0, 1)) / s]
        } else if r(0, 0) > r(1, 1) && r(0, 0) > r(2, 2) {
            let s = (1.0 + r(0, 0) - r(1, 1) - r(2, 2)).sqrt_newton() * 2.0;
            [(r(2, 1) - r(1, 2)) / s, 0.25 * s, (r(0, 1) + r(1, 0)) / s, (r(0, 2) + r(2, 0)) / s]
        } else if r(1, 1) > r(2, 2) {
            let s = (1.0 + r(1, 1) - r(0, 0) - r(2, 2)).sqrt_newton() * 2.0;
            [(r(0, 2) - r(2, 0)) / s, (r(0, 1) + r(1, 0)) / s, 0.25 * s, (r(1, 2) + r(2, 1)) / s]
        } else {
            let s = (1.0 + r(2, 2) - r(0, 0) - r(1, 1)).sqrt_newton() * 2.0;
            [(r(1, 0) - r(0, 1)) / s, (r(0, 2) + r(2, 0)) / s, (r(1, 2) + r(2, 1)) / s, 0.25 * s]
        };
        let q = Quaternion(q).unit()?;
        Some(q)
    }

    fn unit(self) -> Option<Quaternion> {
        let [w, x, y, z] = self.0;
        let n = (w * w + x * x + y * y + z * z).sqrt_newton();
        (n.is_finite() && n > 1e-6).then(|| Quaternion([w / n, x / n, y / n, z / n]))
    }

    fn slerp(self, other: Quaternion, t: f32) -> Quaternion {
        let [a0, a1, a2, a3] = self.0;
        let [mut b0, mut b1, mut b2, mut b3] = other.0;
        if a0 * b0 + a1 * b1 + a2 * b2 + a3 * b3 < 0.0 {
            (b0, b1, b2, b3) = (-b0, -b1, -b2, -b3);
        }
        let (wa, wb) = (1.0 - t, t);
        let mixed = Quaternion([wa * a0 + wb * b0, wa * a1 + wb * b1, wa * a2 + wb * b2, wa * a3 + wb * b3]);
        mixed.unit().unwrap_or(self)
    }

    /// Its rotation's rows (row vectors).
    fn rows(self) -> [[f32; 3]; 3] {
        let [w, x, y, z] = self.0;
        // the column-vector matrix, transposed into rows
        let m = [
            [1.0 - 2.0 * (y * y + z * z), 2.0 * (x * y - z * w), 2.0 * (x * z + y * w)],
            [2.0 * (x * y + z * w), 1.0 - 2.0 * (x * x + z * z), 2.0 * (y * z - x * w)],
            [2.0 * (x * z - y * w), 2.0 * (y * z + x * w), 1.0 - 2.0 * (x * x + y * y)],
        ];
        let at = |i: usize, j: usize| m.get(i).and_then(|row| row.get(j)).copied().unwrap_or(0.0);
        [[at(0, 0), at(1, 0), at(2, 0)], [at(0, 1), at(1, 1), at(2, 1)], [at(0, 2), at(1, 2), at(2, 2)]]
    }
}

/// A square root by Newton's steps from the bits' halved exponent (`no_std`
/// has none): within an ulp or two for the values here (0 to 4).
trait SqrtNewton {
    fn sqrt_newton(self) -> f32;
}

impl SqrtNewton for f32 {
    fn sqrt_newton(self) -> f32 {
        if self == 0.0 {
            return 0.0;
        }
        if self.is_nan() || self < 0.0 || self.is_infinite() {
            return f32::NAN;
        }
        let mut x = f32::from_bits((self.to_bits() >> 1).wrapping_add(0x1FBD_1DF5));
        for _ in 0..4 {
            x = 0.5 * (x + self / x);
        }
        x
    }
}
