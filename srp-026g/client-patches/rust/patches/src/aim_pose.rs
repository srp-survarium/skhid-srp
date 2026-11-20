//! Patch 165: the aimed view is aligned to the gun's idle pose
//! (docs/notes/aim-pose.md).
//!
//! At full aim the game makes the gun's `aim` locator the camera, each frame,
//! from the gun's bones as its clips pose them (calculate_aim_alignment
//! 0x244D40). A gun whose `aim` hangs on the bone its fire clip kicks shows
//! no kick: the alignment follows the bone. 0.45a6 aligns to that bone's
//! pose in the idle clip instead, kept from the weapon's creation, so the
//! fire clip's kick is seen in sights and scopes.
//!
//! Here the idle pose is taken where the game has it: at `hands_ik.
//! aim_alignment`, the first aimed frame the gun's idle clip alone poses its
//! bones, the weapon's aim locator is moved from its bone into the gun's
//! model at that pose (locator x bone, on no bone), which is what the game's
//! own alignment then reads, for as long as the weapon lives. Until then the
//! alignment is the original's. Drawing only: nothing a match computes.

#![forbid(unsafe_code)]

use crate::vostok::aim::{AimLocator, AimedGun, Bone};
use crate::vostok::{Context, Flow, NoExit};

/// process_hands_ik before its calculate_aim_alignment.
pub fn on_aim_alignment(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(mut gun) = AimedGun::at_alignment(ctx) {
        let place = {
            let idle = gun.idle_bones();
            posed(gun.locator(), idle.as_ref().map(|bones| |bone| bones.bone(bone)))
        };
        if let Some(place) = place {
            gun.pose_locator(place);
        }
    }
    Flow::Continue
}

/// Where a locator goes in the gun's model: only one on a bone, and only
/// while the idle clip alone poses the gun (`idle`, a bone's matrix then).
fn posed(locator: AimLocator, idle: Option<impl Fn(Bone) -> [f32; 16]>) -> Option<[f32; 16]> {
    match (locator, idle) {
        (AimLocator::OnBone { bone, place }, Some(idle)) => Some(product(&place, &idle(bone))),
        (AimLocator::OnBone { .. }, None) | (AimLocator::Posed, _) => None,
    }
}

/// `first` then `second`, row-vector affine 4x4s (the game's
/// float4x4::multiply_affine 0xEE700: the last column is 0, 0, 0, 1).
fn product(first: &[f32; 16], second: &[f32; 16]) -> [f32; 16] {
    let mut out = [0.0; 16];
    for row in 0..4 {
        for column in 0..3 {
            let turned: f32 = (0..3).map(|k| first[row * 4 + k] * second[k * 4 + column]).sum();
            out[row * 4 + column] = if row == 3 { turned + second[12 + column] } else { turned };
        }
    }
    out[15] = 1.0;
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const IDENTITY: [f32; 16] = [1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.];

    fn at(x: f32, y: f32, z: f32) -> [f32; 16] {
        let mut m = IDENTITY;
        (m[12], m[13], m[14]) = (x, y, z);
        m
    }

    /// A quarter turn about y (x to -z), then a move.
    fn turned(x: f32, y: f32, z: f32) -> [f32; 16] {
        [0., 0., -1., 0., 0., 1., 0., 0., 1., 0., 0., 0., x, y, z, 1.]
    }

    fn on_bone(place: [f32; 16]) -> AimLocator {
        AimLocator::OnBone { bone: Bone::parse(1).unwrap(), place }
    }

    #[test]
    fn a_locator_on_a_bone_is_posed_by_the_idle_bone() {
        let bone = turned(0.0, 0.25, 2.0);
        let place = posed(on_bone(at(0.5, 1.0, 0.25)), Some(|_| bone)).unwrap();
        // the locator's place, turned by the bone, then moved by it
        assert_eq!(&place[12..15], &[0.25, 1.25, 1.5]);
        assert_eq!(&place[..12], &bone[..12]);
        assert_eq!(place[15], 1.0);
    }

    #[test]
    fn the_idle_pose_asked_for_is_the_locators_bone() {
        let asked = core::cell::Cell::new(None);
        posed(on_bone(IDENTITY), Some(|bone| {
            asked.set(Some(bone));
            IDENTITY
        }));
        assert_eq!(asked.get(), Bone::parse(1));
    }

    #[test]
    fn nothing_moves_while_other_clips_pose_the_gun() {
        assert_eq!(posed(on_bone(at(0.0, 0.5, 0.5)), None::<fn(Bone) -> [f32; 16]>), None);
    }

    #[test]
    fn a_posed_locator_stays() {
        assert_eq!(posed(AimLocator::Posed, Some(|_| turned(1.0, 2.0, 3.0))), None);
        assert_eq!(posed(AimLocator::Posed, None::<fn(Bone) -> [f32; 16]>), None);
    }

    #[test]
    fn no_bone_is_no_bone() {
        assert_eq!(Bone::parse(0xFFFF), None);
        assert!(Bone::parse(0).is_some());
    }

    #[test]
    fn the_product_applies_the_first_first() {
        let p = product(&at(1.0, 0.0, 0.0), &turned(0.0, 0.0, 5.0));
        assert_eq!(&p[12..15], &[0.0, 0.0, 4.0]);
    }
}
