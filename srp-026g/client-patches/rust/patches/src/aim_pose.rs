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
//! aim_alignment`, on an aimed frame the gun's idle clip alone poses its
//! bones, the weapon's aim locator is moved from its bone into the gun's
//! model at that pose (locator x bone, on no bone), which is what the game's
//! own alignment then reads. Until then the alignment is the original's.
//! The gun posed last is kept with what its locator was, so every later
//! idle frame poses it again: a pose taken from a world that was not at
//! rest lasts until the next one. Drawing only: nothing a match computes.

#![forbid(unsafe_code)]

use crate::vostok::aim::{AimLocator, AimedGun, Bone, GunId};
use crate::vostok::{Context, Flow, Lock, NoExit};

/// The gun this patch posed last.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Kept {
    Nothing,
    /// `gun`'s locator, which was at `place` on `bone`, is at `posed` on none.
    Gun { gun: GunId, bone: Bone, place: [f32; 16], posed: [f32; 16] },
}

/// mt_draw runs on the game's thread or a task's, one at a time.
static KEPT: Lock<Kept> = Lock::new(Kept::Nothing);

/// process_hands_ik before its calculate_aim_alignment.
pub fn on_aim_alignment(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(mut gun) = AimedGun::at_alignment(ctx) {
        let place = {
            let idle = gun.idle_bones();
            let idle = idle.as_ref().map(|bones| |bone| bones.bone(bone));
            KEPT.with(ctx.thread(), |kept| {
                let (next, place) = posed(*kept, gun.id(), gun.locator(), idle);
                *kept = next;
                place
            })
        };
        if let Some(place) = place {
            gun.pose_locator(place);
        }
    }
    Flow::Continue
}

/// An aimed frame of `gun`: what is kept after it, and where its locator
/// goes, if anywhere. Only while the idle clip alone poses the gun (`idle`,
/// a bone's matrix then): a locator on a bone, or the kept gun's own posed
/// one (a later weapon at its address has another locator).
fn posed(
    kept: Kept, gun: GunId, locator: AimLocator, idle: Option<impl Fn(Bone) -> [f32; 16]>,
) -> (Kept, Option<[f32; 16]>) {
    let Some(idle) = idle else { return (kept, None) };
    let (bone, place) = match (locator, kept) {
        (AimLocator::OnBone { bone, place }, _) => (bone, place),
        (AimLocator::Posed { place: now }, Kept::Gun { gun: kept, bone, place, posed })
            if kept == gun && posed == now => (bone, place),
        (AimLocator::Posed { .. }, _) => return (kept, None),
    };
    let posed = product(&place, &idle(bone));
    let moved = locator != AimLocator::Posed { place: posed };
    (Kept::Gun { gun, bone, place, posed }, moved.then_some(posed))
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
    const NOT_IDLE: Option<fn(Bone) -> [f32; 16]> = None;

    fn at(x: f32, y: f32, z: f32) -> [f32; 16] {
        let mut m = IDENTITY;
        (m[12], m[13], m[14]) = (x, y, z);
        m
    }

    /// A quarter turn about y (x to -z), then a move.
    fn turned(x: f32, y: f32, z: f32) -> [f32; 16] {
        [0., 0., -1., 0., 0., 1., 0., 0., 1., 0., 0., 0., x, y, z, 1.]
    }

    fn bone() -> Bone {
        Bone::parse(1).unwrap()
    }

    fn on_bone(place: [f32; 16]) -> AimLocator {
        AimLocator::OnBone { bone: bone(), place }
    }

    fn gun(id: u32) -> GunId {
        GunId::test(id)
    }

    /// Gun 7's locator, once at (0.5, 1, 0.25) on its bone, posed by `by`.
    fn kept(by: [f32; 16]) -> (Kept, [f32; 16]) {
        let place = at(0.5, 1.0, 0.25);
        let posed = product(&place, &by);
        (Kept::Gun { gun: gun(7), bone: bone(), place, posed }, posed)
    }

    #[test]
    fn a_locator_on_a_bone_is_posed_by_the_idle_bone() {
        let idle = turned(0.0, 0.25, 2.0);
        let (next, place) = posed(Kept::Nothing, gun(7), on_bone(at(0.5, 1.0, 0.25)), Some(|_| idle));
        let place = place.unwrap();
        // the locator's place, turned by the bone, then moved by it
        assert_eq!(&place[12..15], &[0.25, 1.25, 1.5]);
        assert_eq!(&place[..12], &idle[..12]);
        assert_eq!(place[15], 1.0);
        assert_eq!(next, kept(idle).0);
    }

    #[test]
    fn the_idle_pose_asked_for_is_the_locators_bone() {
        let asked = core::cell::Cell::new(None);
        posed(Kept::Nothing, gun(7), on_bone(IDENTITY), Some(|bone| {
            asked.set(Some(bone));
            IDENTITY
        }));
        assert_eq!(asked.get(), Some(bone()));
    }

    #[test]
    fn nothing_moves_while_other_clips_pose_the_gun() {
        assert_eq!(posed(Kept::Nothing, gun(7), on_bone(at(0.0, 0.5, 0.5)), NOT_IDLE), (Kept::Nothing, None));
        let (was, now) = kept(IDENTITY);
        assert_eq!(posed(was, gun(7), AimLocator::Posed { place: now }, NOT_IDLE), (was, None));
    }

    #[test]
    fn the_same_idle_pose_writes_nothing() {
        let idle = turned(0.0, 0.25, 2.0);
        let (was, now) = kept(idle);
        assert_eq!(posed(was, gun(7), AimLocator::Posed { place: now }, Some(|_| idle)), (was, None));
    }

    #[test]
    fn a_pose_taken_off_rest_is_taken_again_at_rest() {
        let (kicked, idle) = (turned(0.0, 0.25, 1.5), turned(0.0, 0.25, 2.0));
        let (was, now) = kept(kicked);
        let (right, place) = kept(idle);
        assert_eq!(posed(was, gun(7), AimLocator::Posed { place: now }, Some(|_| idle)), (right, Some(place)));
    }

    #[test]
    fn another_guns_posed_locator_stays() {
        let (was, now) = kept(IDENTITY);
        // a model's own locator on no bone
        assert_eq!(posed(Kept::Nothing, gun(7), AimLocator::Posed { place: now }, Some(|_| turned(1.0, 2.0, 3.0))),
                   (Kept::Nothing, None));
        // a gun posed before another was
        assert_eq!(posed(was, gun(8), AimLocator::Posed { place: now }, Some(|_| turned(1.0, 2.0, 3.0))), (was, None));
        // a later weapon at the kept gun's address, its locator its model's
        assert_eq!(posed(was, gun(7), AimLocator::Posed { place: at(9.0, 9.0, 9.0) }, Some(|_| turned(1.0, 2.0, 3.0))),
                   (was, None));
    }

    #[test]
    fn a_new_gun_takes_the_kept_ones_place() {
        let idle = turned(0.0, 0.25, 2.0);
        let (was, _) = kept(IDENTITY);
        let (next, place) = posed(was, gun(8), on_bone(at(1.0, 0.0, 0.0)), Some(|_| idle));
        let posed = product(&at(1.0, 0.0, 0.0), &idle);
        assert_eq!(place, Some(posed));
        assert_eq!(next, Kept::Gun { gun: gun(8), bone: bone(), place: at(1.0, 0.0, 0.0), posed });
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
