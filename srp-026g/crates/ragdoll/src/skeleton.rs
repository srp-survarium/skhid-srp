//! The one human skeleton of 0.26g (`animations/skeletons/scavengers_01.skeleton`,
//! which `base_character_full`'s `skeleton` names) and the ragdoll's parts on it.
//!
//! The bones are listed in the file's order with their parents. The game's
//! own index of a bone is found by name at run time (the arena), so this
//! order is ours only.

use crate::math::Quat;

/// The bones (69, `object_movement`, the root-motion entry, left out) and
/// each one's parent, by index in this table.
pub const BONES: [(&str, Option<u8>); BONE_COUNT] = [
    ("Root", None),
    ("Hip", Some(0)),
    ("LeftUpLeg", Some(1)),
    ("LeftUpLeg_Roll", Some(2)),
    ("LeftKnee", Some(2)),
    ("LeftLeg", Some(4)),
    ("LeftFoot", Some(5)),
    ("LeftToe", Some(6)),
    ("RightUpLeg", Some(1)),
    ("RightUpLeg_Roll", Some(8)),
    ("RightKnee", Some(8)),
    ("RightLeg", Some(10)),
    ("RightFoot", Some(11)),
    ("RightToe", Some(12)),
    ("Spin", Some(1)),
    ("Spin_1", Some(14)),
    ("Chest", Some(15)),
    ("RightClavicle", Some(16)),
    ("RightArm", Some(17)),
    ("RightForeArm", Some(18)),
    ("RightForeArm_Roll", Some(19)),
    ("RightHand", Some(19)),
    ("RightIndex1", Some(21)),
    ("RightIndex2", Some(22)),
    ("RightIndex3", Some(23)),
    ("RightThumb1", Some(21)),
    ("RightThumb2", Some(25)),
    ("RightThumb3", Some(26)),
    ("RightRing1", Some(21)),
    ("RightRing2", Some(28)),
    ("RightRing3", Some(29)),
    ("RightMiddle1", Some(21)),
    ("RightMiddle2", Some(31)),
    ("RightMiddle3", Some(32)),
    ("RightPinky1", Some(21)),
    ("RightPinky2", Some(34)),
    ("RightPinky3", Some(35)),
    ("RightArm_Roll", Some(18)),
    ("RightShoulder", Some(17)),
    ("Neck", Some(16)),
    ("Head", Some(39)),
    ("LeftEye", Some(40)),
    ("Jaw", Some(40)),
    ("RightEye", Some(40)),
    ("LeftClavicle", Some(16)),
    ("LeftShoulder", Some(44)),
    ("LeftArm", Some(44)),
    ("LeftArm_Roll", Some(46)),
    ("LeftForeArm", Some(46)),
    ("LeftHand", Some(48)),
    ("LeftRing1", Some(49)),
    ("LeftRing2", Some(50)),
    ("LeftRing3", Some(51)),
    ("LeftIndex1", Some(49)),
    ("LeftIndex2", Some(53)),
    ("LeftIndex3", Some(54)),
    ("LeftThumb1", Some(49)),
    ("LeftThumb2", Some(56)),
    ("LeftThumb3", Some(57)),
    ("LeftMiddle1", Some(49)),
    ("LeftMiddle2", Some(59)),
    ("LeftMiddle3", Some(60)),
    ("LeftPinky1", Some(49)),
    ("LeftPinky2", Some(62)),
    ("LeftPinky3", Some(63)),
    ("LeftForeArm_Roll", Some(48)),
    ("Weapon", Some(0)),
    ("Pistol", Some(0)),
    ("Item", Some(0)),
];

pub const BONE_COUNT: usize = 69;

/// A bone of [`BONES`], by its index there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bone(pub u8);

impl Bone {
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    pub fn name(self) -> &'static str {
        BONES[self.index()].0
    }

    pub fn parent(self) -> Option<Bone> {
        BONES[self.index()].1.map(Bone)
    }

    /// The bone of this name.
    pub fn named(name: &str) -> Option<Bone> {
        BONES
            .iter()
            .position(|(n, _)| *n == name)
            .map(|i| Bone(i as u8))
    }
}

// The bones the parts are made from (indices in BONES).
pub const ROOT: Bone = Bone(0);
pub const HIP: Bone = Bone(1);
pub const LEFT_UP_LEG: Bone = Bone(2);
pub const LEFT_LEG: Bone = Bone(5);
pub const LEFT_FOOT: Bone = Bone(6);
pub const LEFT_TOE: Bone = Bone(7);
pub const RIGHT_UP_LEG: Bone = Bone(8);
pub const RIGHT_LEG: Bone = Bone(11);
pub const RIGHT_FOOT: Bone = Bone(12);
pub const RIGHT_TOE: Bone = Bone(13);
pub const SPIN: Bone = Bone(14);
pub const RIGHT_ARM: Bone = Bone(18);
pub const RIGHT_FORE_ARM: Bone = Bone(19);
pub const RIGHT_HAND: Bone = Bone(21);
pub const NECK: Bone = Bone(39);
pub const HEAD: Bone = Bone(40);
pub const LEFT_ARM: Bone = Bone(46);
pub const LEFT_FORE_ARM: Bone = Bone(48);
pub const LEFT_HAND: Bone = Bone(49);
pub const WEAPON: Bone = Bone(66);
/// The camera's bone: player+44Ch is it times the player's transform
/// (`computed_head_transform` 0x2DA4B0).
pub const ITEM: Bone = Bone(68);

/// A loose hand's fingers (nothing held, no muscle): each joint's turn on
/// its parent, the splines' Euler angles in rad, the knuckle first. Both
/// hands' finger bones are laid out alike (a curl is +Z), so one table
/// serves both. The spread of the knuckles and the thumb are the open
/// palm's of the artefact carry (`scripts/artefact-carry/pose.json`); the
/// curls are a resting hand's, more toward the little finger.
const LOOSE_FINGERS: [(&str, [[f32; 3]; 3]); 5] = [
    ("Thumb", [[1.5, -0.65, 0.45], [0.0, 0.0, 0.35], [0.0, 0.0, 0.25]]),
    ("Index", [[0.02, -0.06, 0.45], [0.0, 0.0, 0.65], [0.0, 0.0, 0.35]]),
    ("Middle", [[-0.03, -0.05, 0.5], [0.0, 0.0, 0.7], [0.0, 0.0, 0.4]]),
    ("Ring", [[-0.07, -0.02, 0.55], [0.0, 0.0, 0.75], [0.0, 0.0, 0.4]]),
    ("Pinky", [[-0.12, 0.0, 0.6], [0.0, 0.0, 0.8], [0.0, 0.0, 0.45]]),
];

impl Bone {
    /// A finger joint's turn on its parent in a loose hand; None for a bone
    /// that is no finger joint.
    pub fn loose_finger(self) -> Option<Quat> {
        let name = self.name();
        let finger = name.strip_prefix("Left").or_else(|| name.strip_prefix("Right"))?;
        let (finger, joint) = finger.split_at(finger.len().checked_sub(1)?);
        let joint = match joint {
            "1" => 0,
            "2" => 1,
            "3" => 2,
            _ => return None,
        };
        let (_, joints) = LOOSE_FINGERS.iter().find(|(name, _)| *name == finger)?;
        Some(Quat::from_spline_euler(joints[joint]))
    }
}

/// The ragdoll's eleven bodies. Hands go with the forearms, feet with the
/// shins, the neck with the head.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Pelvis,
    Torso,
    Head,
    LeftThigh,
    LeftShin,
    RightThigh,
    RightShin,
    LeftUpperArm,
    LeftForearm,
    RightUpperArm,
    RightForearm,
}

pub const PARTS: usize = 11;

impl Part {
    pub const ALL: [Part; PARTS] = [
        Part::Pelvis,
        Part::Torso,
        Part::Head,
        Part::LeftThigh,
        Part::LeftShin,
        Part::RightThigh,
        Part::RightShin,
        Part::LeftUpperArm,
        Part::LeftForearm,
        Part::RightUpperArm,
        Part::RightForearm,
    ];

    pub const fn index(self) -> usize {
        self as usize
    }

    /// The bone the body moves (the others follow it, or their parent).
    pub const fn bone(self) -> Bone {
        match self {
            Part::Pelvis => HIP,
            Part::Torso => SPIN,
            Part::Head => NECK,
            Part::LeftThigh => LEFT_UP_LEG,
            Part::LeftShin => LEFT_LEG,
            Part::RightThigh => RIGHT_UP_LEG,
            Part::RightShin => RIGHT_LEG,
            Part::LeftUpperArm => LEFT_ARM,
            Part::LeftForearm => LEFT_FORE_ARM,
            Part::RightUpperArm => RIGHT_ARM,
            Part::RightForearm => RIGHT_FORE_ARM,
        }
    }

    /// The side of the body the part is on (the trunk and the head: none).
    pub const fn side(self) -> Option<Side> {
        match self {
            Part::LeftThigh | Part::LeftShin | Part::LeftUpperArm | Part::LeftForearm => {
                Some(Side::Left)
            }
            Part::RightThigh | Part::RightShin | Part::RightUpperArm | Part::RightForearm => {
                Some(Side::Right)
            }
            Part::Pelvis | Part::Torso | Part::Head => None,
        }
    }

    /// The body's mass in kg (the Bodycam study's table, given for an 80 kg
    /// player; its parts sum to 69.1 kg).
    pub const fn mass(self) -> f32 {
        match self {
            Part::Pelvis => 12.0,
            Part::Torso => 17.0,
            Part::Head => 6.5,
            Part::LeftThigh | Part::RightThigh => 8.0,
            Part::LeftShin | Part::RightShin => 4.8,
            Part::LeftUpperArm | Part::RightUpperArm => 2.2,
            Part::LeftForearm | Part::RightForearm => 1.8,
        }
    }

    /// The capsule's radius in m (the hit targets' of
    /// `base_character_full.skinned_model/hit_targets`, a little less where
    /// two of them overlap; the pelvis has none and takes the thighs').
    pub const fn radius(self) -> f32 {
        match self {
            Part::Pelvis => 0.12,
            Part::Torso => 0.15,
            Part::Head => 0.1,
            Part::LeftThigh | Part::RightThigh => 0.09,
            Part::LeftShin | Part::RightShin => 0.07,
            Part::LeftUpperArm | Part::RightUpperArm => 0.06,
            Part::LeftForearm | Part::RightForearm => 0.05,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

/// The ten joints: (parent, child, the bone at the joint, its kind).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JointKind {
    Waist,
    Neck,
    Hip,
    Knee,
    Shoulder,
    Elbow,
}

pub const JOINTS: usize = 10;

pub const JOINT_TABLE: [(Part, Part, Bone, JointKind); JOINTS] = [
    (Part::Pelvis, Part::Torso, SPIN, JointKind::Waist),
    (Part::Torso, Part::Head, NECK, JointKind::Neck),
    (Part::Pelvis, Part::LeftThigh, LEFT_UP_LEG, JointKind::Hip),
    (Part::LeftThigh, Part::LeftShin, LEFT_LEG, JointKind::Knee),
    (Part::Pelvis, Part::RightThigh, RIGHT_UP_LEG, JointKind::Hip),
    (
        Part::RightThigh,
        Part::RightShin,
        RIGHT_LEG,
        JointKind::Knee,
    ),
    (
        Part::Torso,
        Part::LeftUpperArm,
        LEFT_ARM,
        JointKind::Shoulder,
    ),
    (
        Part::LeftUpperArm,
        Part::LeftForearm,
        LEFT_FORE_ARM,
        JointKind::Elbow,
    ),
    (
        Part::Torso,
        Part::RightUpperArm,
        RIGHT_ARM,
        JointKind::Shoulder,
    ),
    (
        Part::RightUpperArm,
        Part::RightForearm,
        RIGHT_FORE_ARM,
        JointKind::Elbow,
    ),
];

/// Whether two parts share a joint (they do not collide with each other).
pub fn adjacent(a: Part, b: Part) -> bool {
    JOINT_TABLE
        .iter()
        .any(|&(p, c, _, _)| (p == a && c == b) || (p == b && c == a))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_thirty_finger_joints_have_a_loose_turn_and_no_other_bone() {
        let loose = |i: usize| Bone(i as u8).loose_finger().is_some();
        assert_eq!((0..BONE_COUNT).filter(|&i| loose(i)).count(), 30);
        for (i, (name, parent)) in BONES.iter().enumerate() {
            let finger = ["Thumb", "Index", "Middle", "Ring", "Pinky"].iter().any(|f| name.contains(f));
            assert_eq!(loose(i), finger, "{name}");
            // a finger joint hangs on its hand or on the joint before it
            assert!(!finger || parent.is_some_and(|p| BONES[p as usize].0.ends_with("Hand") || loose(p as usize)), "{name}");
        }
    }

    #[test]
    fn the_table_is_parent_before_child_and_the_constants_name_their_bones() {
        for (i, (_, parent)) in BONES.iter().enumerate() {
            assert!(parent.is_none_or(|p| (p as usize) < i), "bone {i}");
        }
        assert_eq!(BONES.iter().filter(|(_, p)| p.is_none()).count(), 1);
        for (bone, name) in [
            (HIP, "Hip"),
            (LEFT_LEG, "LeftLeg"),
            (RIGHT_TOE, "RightToe"),
            (SPIN, "Spin"),
            (RIGHT_HAND, "RightHand"),
            (NECK, "Neck"),
            (HEAD, "Head"),
            (LEFT_FORE_ARM, "LeftForeArm"),
            (LEFT_HAND, "LeftHand"),
            (WEAPON, "Weapon"),
            (ITEM, "Item"),
        ] {
            assert_eq!(bone.name(), name);
            assert_eq!(Bone::named(name), Some(bone));
        }
        let mass: f32 = Part::ALL.iter().map(|p| p.mass()).sum();
        assert!((mass - 69.1).abs() < 0.01, "{mass}");
        for (i, p) in Part::ALL.iter().enumerate() {
            assert_eq!(p.index(), i);
        }
    }
}
