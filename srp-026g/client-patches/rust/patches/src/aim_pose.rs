//! Patch 172: the aimed view is aligned to the gun's idle pose
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
//! Right after the call (`hands_ik.aligned`) the locator is the weapon's own
//! again, on its bone: the record is the same in every game at every step,
//! for what the simulation reads of it (patch 200). The gun posed last is
//! kept with its idle pose, so every later idle frame takes it again: a pose
//! taken from a world that was not at rest lasts until the next one. Drawing
//! only: nothing a match computes.
//!
//! Patch 197's switch ([`show`], [`Kick::Back`]) shows less of the kick: the
//! aim is carried by the gun's body (its fire point's bone) as the clips move
//! it, so the sights stay on the view, where a bullet goes, whatever the
//! clip turns; of the body's move only its way back along the view is left,
//! and of that a share. The same site tells that patch how far the sight is
//! raised ([`take_aim_progress`]).

#![forbid(unsafe_code)]

use crate::affine::{inverse, product, row};
use crate::vostok::aim::{AimLocator, AimedGun, AlignedGun, Bone, GunId};
use crate::vostok::{Context, Flow, Lock, NoExit, Thread};

/// What the aimed view shows of the gun's kick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kick {
    /// The gun's shot clip, whole: the view on the idle pose.
    Clip,
    /// The sights on the view; `share` of the clip's kick back along it.
    Back { share: f32 },
}

/// The gun this patch posed last.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Kept {
    Nothing,
    /// `gun`'s locator, at `place` on `bone` in its record, was given the
    /// alignment as `shown` on none; `rest` is its idle pose.
    Gun { gun: GunId, bone: Bone, place: [f32; 16], rest: Rest, shown: [f32; 16] },
}

/// A gun's idle pose: its aim in the gun's model, and its body's bone.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Rest {
    aim: [f32; 16],
    body: Option<Body>,
}

/// The bone the gun's body hangs on, and what undoes its idle matrix.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Body {
    bone: Bone,
    from_rest: [f32; 16],
}

/// The sight between the hands' pass and the camera's.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Aim {
    /// No aimed frame since the progress was last taken twice.
    Lowered,
    /// The progress of the last aimed frame, not taken yet.
    Seen(f32),
    /// Taken once since: one frame without an aimed pass is still aimed
    /// (the two passes are not in one order every frame).
    Taken(f32),
}

struct State {
    kept: Kept,
    kick: Kick,
    aim: Aim,
}

/// mt_draw runs on the game's thread or a task's, one at a time; the camera
/// on the game's.
static STATE: Lock<State> = Lock::new(State { kept: Kept::Nothing, kick: Kick::Clip, aim: Aim::Lowered });

/// What the aimed view shows of the kick from now on.
pub fn show(thread: &Thread, kick: Kick) {
    STATE.with(thread, |state| state.kick = kick);
}

/// How far the sight is raised (0 from the hip to 1 fully aimed), as the
/// last aimed frame had it: for a pass a frame, which takes it once.
pub fn take_aim_progress(thread: &Thread) -> f32 {
    STATE.with(thread, |state| {
        let (next, progress) = taken(state.aim);
        state.aim = next;
        progress
    })
}

/// The sight once its progress is taken, and that progress.
fn taken(aim: Aim) -> (Aim, f32) {
    match aim {
        Aim::Seen(progress) => (Aim::Taken(progress), progress),
        Aim::Taken(progress) => (Aim::Lowered, progress),
        Aim::Lowered => (Aim::Lowered, 0.0),
    }
}

/// process_hands_ik before its calculate_aim_alignment.
pub fn on_aim_alignment(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(mut gun) = AimedGun::at_alignment(ctx) {
        let place = {
            let (idle, live) = (gun.idle_bones(), gun.bones());
            let idle = idle.as_ref().map(|bones| |bone| bones.bone(bone));
            let (id, locator, body, progress) = (gun.id(), gun.locator(), gun.body_bone(), gun.progress());
            STATE.with(ctx.thread(), |state| {
                state.aim = Aim::Seen(progress);
                let (next, place) = posed(state.kept, id, locator, body, state.kick, idle, |bone| live.bone(bone));
                state.kept = next;
                place
            })
        };
        if let Some(place) = place {
            gun.pose_locator(place);
        }
    }
    Flow::Continue
}

/// process_hands_ik after its calculate_aim_alignment: the weapon's own
/// locator back in its record.
pub fn on_aligned(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(mut gun) = AlignedGun::at_aligned(ctx) {
        let (id, locator) = (gun.id(), gun.locator());
        if let Some((bone, place)) = STATE.with(ctx.thread(), |state| own(state.kept, id, locator)) {
            gun.hang_locator(bone, place);
        }
    }
    Flow::Continue
}

/// The locator `gun`'s record had before this frame's alignment was given
/// the kept pose: its bone and its place on it. None when the record holds
/// anything else.
fn own(kept: Kept, gun: GunId, locator: AimLocator) -> Option<(Bone, [f32; 16])> {
    match kept {
        Kept::Gun { gun: of, bone, place, shown, .. } if of == gun && locator == (AimLocator::Posed { place: shown }) => Some((bone, place)),
        _ => None,
    }
}

/// An aimed frame of `gun`: what is kept after it, and where its locator
/// goes for the alignment, if anywhere. Its idle pose is taken while the
/// idle clip alone poses the gun (`idle`, a bone's matrix then) and kept
/// after for the same locator; with none yet the alignment is the
/// original's. Only a locator on a bone is moved (one on none is its
/// model's, or was not put back: left). `live` is a bone's matrix this
/// frame, `body` the bone of the gun's body.
fn posed(
    kept: Kept, gun: GunId, locator: AimLocator, body: Option<Bone>, kick: Kick,
    idle: Option<impl Fn(Bone) -> [f32; 16]>, live: impl Fn(Bone) -> [f32; 16],
) -> (Kept, Option<[f32; 16]>) {
    let known = match kept {
        Kept::Gun { gun: of, bone, place, rest, shown } if of == gun => Some((bone, place, rest, shown)),
        _ => None,
    };
    let AimLocator::OnBone { bone, place } = locator else { return (kept, None) };
    // the kept gun's own record: its idle pose is known (a later weapon at
    // its address with the same record is the same model)
    let rested = known.and_then(|(of, at, rest, _)| (of == bone && at == place).then_some(rest));
    let rest = match (idle, rested) {
        (Some(idle), _) => {
            let body = body.and_then(|bone| Some(Body { bone, from_rest: inverse(&idle(bone))? }));
            Rest { aim: product(&place, &idle(bone)), body }
        }
        (None, Some(rest)) => rest,
        (None, None) => return (kept, None),
    };
    let shown = match (kick, rest.body) {
        (Kick::Back { share }, Some(body)) => held(&rest.aim, &body.from_rest, &live(body.bone), share),
        _ => rest.aim,
    };
    (Kept::Gun { gun, bone, place, rest, shown }, Some(shown))
}

/// The aim as the gun's body carries it: its idle `aim`, by the body bone's
/// move from its idle matrix (`from_rest`, then `live`), so the sights stay
/// on the view however the clip turns the body. Of the body's way along the
/// view (the locator's z, toward the eye) `share` is left: the gun comes
/// back that far.
fn held(aim: &[f32; 16], from_rest: &[f32; 16], live: &[f32; 16], share: f32) -> [f32; 16] {
    let mut carried = product(&product(aim, from_rest), live);
    let Some(back) = row(&carried, 2).direction() else { return *aim };
    let moved = (row(&carried, 3) - row(aim, 3)).dot(back) * share;
    let place = row(&carried, 3) - back * moved;
    carried[12..15].copy_from_slice(&[place.x, place.y, place.z]);
    carried
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::affine::IDENTITY;

    const NOT_IDLE: Option<fn(Bone) -> [f32; 16]> = None;
    const BACK: Kick = Kick::Back { share: 0.5 };

    fn at(x: f32, y: f32, z: f32) -> [f32; 16] {
        let mut m = IDENTITY;
        (m[12], m[13], m[14]) = (x, y, z);
        m
    }

    /// A quarter turn about y (x to -z), then a move.
    fn turned(x: f32, y: f32, z: f32) -> [f32; 16] {
        [0., 0., -1., 0., 0., 1., 0., 0., 1., 0., 0., 0., x, y, z, 1.]
    }

    /// A turn about x by an angle of sine 0.6 (y toward z), then a move.
    fn tilted(x: f32, y: f32, z: f32) -> [f32; 16] {
        [1., 0., 0., 0., 0., 0.8, 0.6, 0., 0., -0.6, 0.8, 0., x, y, z, 1.]
    }

    fn bone() -> Bone {
        Bone::parse(1).unwrap()
    }

    fn body() -> Bone {
        Bone::parse(2).unwrap()
    }

    /// The record of gun 7: at (0.5, 1, 0.25) on its bone.
    const PLACE: [f32; 16] = [1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0.5, 1.0, 0.25, 1.];

    fn on_bone(place: [f32; 16]) -> AimLocator {
        AimLocator::OnBone { bone: bone(), place }
    }

    fn gun(id: u32) -> GunId {
        GunId::test(id)
    }

    fn near(a: &[f32; 16], b: &[f32; 16]) -> bool {
        a.iter().zip(b).all(|(a, b)| (a - b).abs() < 1e-5)
    }

    /// Gun 7 with its idle pose taken while its bone was `by`, no body bone.
    fn kept(by: [f32; 16]) -> (Kept, [f32; 16]) {
        let posed = product(&PLACE, &by);
        (Kept::Gun { gun: gun(7), bone: bone(), place: PLACE, rest: Rest { aim: posed, body: None }, shown: posed }, posed)
    }

    /// The clip's whole kick, no body bone: patch 172 alone.
    fn clip(
        kept: Kept, gun: GunId, locator: AimLocator, idle: Option<impl Fn(Bone) -> [f32; 16]>,
    ) -> (Kept, Option<[f32; 16]>) {
        posed(kept, gun, locator, None, Kick::Clip, idle, |_| panic!("the clip's kick reads no live bone"))
    }

    #[test]
    fn a_locator_on_a_bone_is_posed_by_the_idle_bone() {
        let idle = turned(0.0, 0.25, 2.0);
        let (next, place) = clip(Kept::Nothing, gun(7), on_bone(PLACE), Some(|_| idle));
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
        clip(Kept::Nothing, gun(7), on_bone(IDENTITY), Some(|bone| {
            asked.set(Some(bone));
            IDENTITY
        }));
        assert_eq!(asked.get(), Some(bone()));
    }

    #[test]
    fn a_gun_never_seen_at_rest_keeps_the_games_alignment() {
        assert_eq!(clip(Kept::Nothing, gun(7), on_bone(PLACE), NOT_IDLE), (Kept::Nothing, None));
    }

    #[test]
    fn once_at_rest_every_aimed_frame_is_given_the_idle_pose() {
        let (was, rest) = kept(turned(0.0, 0.25, 2.0));
        // the record is the weapon's own again each frame: the kick's frames too
        assert_eq!(clip(was, gun(7), on_bone(PLACE), NOT_IDLE), (was, Some(rest)));
    }

    #[test]
    fn a_pose_taken_off_rest_is_taken_again_at_rest() {
        let (kicked, idle) = (turned(0.0, 0.25, 1.5), turned(0.0, 0.25, 2.0));
        let (was, _) = kept(kicked);
        let (right, place) = kept(idle);
        assert_eq!(clip(was, gun(7), on_bone(PLACE), Some(|_| idle)), (right, Some(place)));
    }

    #[test]
    fn a_locator_on_no_bone_is_left() {
        let (was, now) = kept(IDENTITY);
        for idle in [Some(|_| turned(1.0, 2.0, 3.0)), None] {
            assert_eq!(clip(was, gun(7), AimLocator::Posed { place: now }, idle), (was, None));
            assert_eq!(clip(Kept::Nothing, gun(7), AimLocator::Posed { place: PLACE }, idle), (Kept::Nothing, None));
        }
    }

    #[test]
    fn another_gun_or_another_record_waits_for_its_own_idle_pose() {
        let (was, _) = kept(IDENTITY);
        assert_eq!(clip(was, gun(8), on_bone(PLACE), NOT_IDLE), (was, None));
        assert_eq!(clip(was, gun(7), on_bone(at(1.0, 0.0, 0.0)), NOT_IDLE), (was, None));
        let idle = turned(0.0, 0.25, 2.0);
        let (next, place) = clip(was, gun(8), on_bone(at(1.0, 0.0, 0.0)), Some(|_| idle));
        let posed = product(&at(1.0, 0.0, 0.0), &idle);
        assert_eq!(place, Some(posed));
        assert_eq!(next, Kept::Gun { gun: gun(8), bone: bone(), place: at(1.0, 0.0, 0.0), rest: Rest { aim: posed, body: None }, shown: posed });
    }

    #[test]
    fn the_weapons_own_locator_goes_back_after_the_alignment() {
        let (was, shown) = kept(turned(0.0, 0.25, 2.0));
        assert_eq!(own(was, gun(7), AimLocator::Posed { place: shown }), Some((bone(), PLACE)));
        // not ours to put back: another gun's, a model's own on no bone, one still on its bone
        assert_eq!(own(was, gun(8), AimLocator::Posed { place: shown }), None);
        assert_eq!(own(was, gun(7), AimLocator::Posed { place: at(9.0, 9.0, 9.0) }), None);
        assert_eq!(own(was, gun(7), on_bone(PLACE)), None);
        assert_eq!(own(Kept::Nothing, gun(7), AimLocator::Posed { place: shown }), None);
    }

    #[test]
    fn no_bone_is_no_bone() {
        assert_eq!(Bone::parse(0xFFFF), None);
        assert!(Bone::parse(0).is_some());
    }

    /// Gun 7 at rest with a body bone: both bones as `idle`.
    fn rested(idle: [f32; 16]) -> (Kept, [f32; 16]) {
        let (next, place) = posed(Kept::Nothing, gun(7), on_bone(at(0.0, 0.1, 0.0)), Some(body()), BACK, Some(|_| idle), |_| idle);
        (next, place.unwrap())
    }

    /// A later frame of that gun, its body's bone at `live`.
    fn frame(was: Kept, live: [f32; 16]) -> (Kept, [f32; 16]) {
        let (next, place) = posed(was, gun(7), on_bone(at(0.0, 0.1, 0.0)), Some(body()), BACK, NOT_IDLE, |_| live);
        (next, place.unwrap())
    }

    #[test]
    fn at_rest_the_held_aim_is_the_idle_pose() {
        let idle = turned(0.0, 0.25, 2.0);
        let (_, place) = rested(idle);
        assert!(near(&place, &product(&at(0.0, 0.1, 0.0), &idle)), "{place:?}");
    }

    #[test]
    fn of_a_kick_straight_back_its_share_is_left() {
        let (was, aim) = rested(IDENTITY);
        // the body 0.02 back along the locator's z, toward the eye
        let (next, place) = frame(was, at(0.0, 0.0, 0.02));
        // the camera stays half the way behind the carried aim: the gun is seen 0.01 back
        assert!(near(&place, &at(0.0, 0.1, 0.01)), "{place:?}");
        let Kept::Gun { shown, rest, .. } = next else { panic!("nothing kept") };
        assert_eq!((shown, rest.aim), (place, aim));
        assert_eq!(own(next, gun(7), AimLocator::Posed { place }), Some((bone(), at(0.0, 0.1, 0.0))));
    }

    #[test]
    fn the_sights_stay_on_the_view_when_the_clip_turns_the_body() {
        let (was, _) = rested(IDENTITY);
        let kicked = tilted(0.0, 0.03, 0.0);
        let (_, place) = frame(was, kicked);
        // the locator turns with the body, so the alignment undoes the turn
        assert_eq!(&place[..12], &kicked[..12]);
        // the carried aim is at (0, 0.11, 0.06), 0.042 back along its own z
        // (0, -0.6, 0.8) from the rest's: half of that is left
        assert!((place[12]).abs() < 1e-6 && (place[13] - 0.1226).abs() < 1e-6 && (place[14] - 0.0432).abs() < 1e-6, "{place:?}");
    }

    #[test]
    fn the_whole_clip_is_shown_by_a_gun_with_no_body_bone() {
        let idle = turned(0.0, 0.25, 2.0);
        let (next, place) = posed(Kept::Nothing, gun(7), on_bone(PLACE), None, BACK, Some(|_| idle), |_| panic!("no body to read"));
        assert_eq!(place, Some(product(&PLACE, &idle)));
        assert_eq!(next, kept(idle).0);
    }

    #[test]
    fn the_progress_lasts_a_frame_without_an_aimed_pass() {
        assert_eq!(taken(Aim::Seen(0.75)), (Aim::Taken(0.75), 0.75));
        assert_eq!(taken(Aim::Taken(0.75)), (Aim::Lowered, 0.75));
        assert_eq!(taken(Aim::Lowered), (Aim::Lowered, 0.0));
    }
}
