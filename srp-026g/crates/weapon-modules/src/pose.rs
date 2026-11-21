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
