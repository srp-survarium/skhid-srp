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
//! The gun posed last is kept with what its locator was, so every later
//! idle frame poses it again: a pose taken from a world that was not at
//! rest lasts until the next one. Drawing only: nothing a match computes.
//!
//! Patch 197's switch ([`show`], [`Kick::Back`]) shows less of the kick: the
//! aim is carried by the gun's body (its fire point's bone) as the clips move
//! it, so the sights stay on the view, where a bullet goes, whatever the
//! clip turns; of the body's move only its way back along the view is left,
//! and of that a share. The same site tells that patch how far the sight is
//! raised ([`take_aim_progress`]).

#![forbid(unsafe_code)]

use ragdoll::math::Vec3;

use crate::vostok::aim::{AimLocator, AimedGun, Bone, GunId};
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
    /// `gun`'s locator, which was at `place` on `bone`, holds `shown` on
    /// none; `rest` is its idle pose.
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

/// An aimed frame of `gun`: what is kept after it, and where its locator
/// goes, if anywhere. Its idle pose is taken while the idle clip alone poses
/// the gun (`idle`, a bone's matrix then) and kept after; with none yet the
/// alignment is the original's. Only a locator on a bone or the kept gun's
/// own is moved (a later weapon at its address has another). `live` is a
/// bone's matrix this frame, `body` the bone of the gun's body.
fn posed(
    kept: Kept, gun: GunId, locator: AimLocator, body: Option<Bone>, kick: Kick,
    idle: Option<impl Fn(Bone) -> [f32; 16]>, live: impl Fn(Bone) -> [f32; 16],
) -> (Kept, Option<[f32; 16]>) {
    let known = match kept {
        Kept::Gun { gun: of, bone, place, rest, shown } if of == gun => Some((bone, place, rest, shown)),
        _ => None,
    };
    let (bone, place, rested) = match (locator, known) {
        (AimLocator::OnBone { bone, place }, _) => (bone, place, None),
        (AimLocator::Posed { place: now }, Some((bone, place, rest, shown))) if shown == now => (bone, place, Some(rest)),
        (AimLocator::Posed { .. }, _) => return (kept, None),
    };
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
    let moved = locator != AimLocator::Posed { place: shown };
    (Kept::Gun { gun, bone, place, rest, shown }, moved.then_some(shown))
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

/// What undoes a row-vector affine 4x4; none for one that flattens space.
fn inverse(m: &[f32; 16]) -> Option<[f32; 16]> {
    let (x, y, z, place) = (row(m, 0), row(m, 1), row(m, 2), row(m, 3));
    let volume = x.dot(y.cross(z));
    if !volume.is_finite() || volume.abs() < 1e-12 {
        return None;
    }
    // the columns of the 3x3's inverse
    let (a, b, c) = (y.cross(z) * (1.0 / volume), z.cross(x) * (1.0 / volume), x.cross(y) * (1.0 / volume));
    let mut out = [0.0; 16];
    for (at, column) in [a, b, c].into_iter().enumerate() {
        (out[at], out[4 + at], out[8 + at]) = (column.x, column.y, column.z);
        out[12 + at] = -place.dot(column);
    }
    out[15] = 1.0;
    Some(out)
}

/// A row-vector 4x4's row (0 to 2 its axes, 3 its place).
fn row(m: &[f32; 16], at: usize) -> Vec3 {
    Vec3::new(m[at * 4], m[at * 4 + 1], m[at * 4 + 2])
}

#[cfg(test)]
mod tests {
    use super::*;

    const IDENTITY: [f32; 16] = [1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.];
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

    fn on_bone(place: [f32; 16]) -> AimLocator {
        AimLocator::OnBone { bone: bone(), place }
    }

    fn gun(id: u32) -> GunId {
        GunId::test(id)
    }

    fn near(a: &[f32; 16], b: &[f32; 16]) -> bool {
        a.iter().zip(b).all(|(a, b)| (a - b).abs() < 1e-5)
    }

    /// Gun 7's locator, once at (0.5, 1, 0.25) on its bone, posed by `by`
    /// with no body bone.
    fn kept(by: [f32; 16]) -> (Kept, [f32; 16]) {
        let place = at(0.5, 1.0, 0.25);
        let posed = product(&place, &by);
        (Kept::Gun { gun: gun(7), bone: bone(), place, rest: Rest { aim: posed, body: None }, shown: posed }, posed)
    }

    /// The clip's whole kick, no body bone: patch 172 as it was.
    fn clip(
        kept: Kept, gun: GunId, locator: AimLocator, idle: Option<impl Fn(Bone) -> [f32; 16]>,
    ) -> (Kept, Option<[f32; 16]>) {
        posed(kept, gun, locator, None, Kick::Clip, idle, |_| panic!("the clip's kick reads no live bone"))
    }

    #[test]
    fn a_locator_on_a_bone_is_posed_by_the_idle_bone() {
        let idle = turned(0.0, 0.25, 2.0);
        let (next, place) = clip(Kept::Nothing, gun(7), on_bone(at(0.5, 1.0, 0.25)), Some(|_| idle));
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
    fn nothing_moves_while_other_clips_pose_the_gun() {
        assert_eq!(clip(Kept::Nothing, gun(7), on_bone(at(0.0, 0.5, 0.5)), NOT_IDLE), (Kept::Nothing, None));
        let (was, now) = kept(IDENTITY);
        assert_eq!(clip(was, gun(7), AimLocator::Posed { place: now }, NOT_IDLE), (was, None));
    }

    #[test]
    fn the_same_idle_pose_writes_nothing() {
        let idle = turned(0.0, 0.25, 2.0);
        let (was, now) = kept(idle);
        assert_eq!(clip(was, gun(7), AimLocator::Posed { place: now }, Some(|_| idle)), (was, None));
    }

    #[test]
    fn a_pose_taken_off_rest_is_taken_again_at_rest() {
        let (kicked, idle) = (turned(0.0, 0.25, 1.5), turned(0.0, 0.25, 2.0));
        let (was, now) = kept(kicked);
        let (right, place) = kept(idle);
        assert_eq!(clip(was, gun(7), AimLocator::Posed { place: now }, Some(|_| idle)), (right, Some(place)));
    }

    #[test]
    fn another_guns_posed_locator_stays() {
        let (was, now) = kept(IDENTITY);
        assert_eq!(clip(was, gun(8), AimLocator::Posed { place: now }, Some(|_| IDENTITY)), (was, None));
        assert_eq!(clip(was, gun(7), AimLocator::Posed { place: at(9.0, 9.0, 9.0) }, Some(|_| turned(1.0, 2.0, 3.0))),
                   (was, None));
    }

    #[test]
    fn a_new_gun_takes_the_kept_ones_place() {
        let idle = turned(0.0, 0.25, 2.0);
        let (was, _) = kept(IDENTITY);
        let (next, place) = clip(was, gun(8), on_bone(at(1.0, 0.0, 0.0)), Some(|_| idle));
        let posed = product(&at(1.0, 0.0, 0.0), &idle);
        assert_eq!(place, Some(posed));
        assert_eq!(next, Kept::Gun { gun: gun(8), bone: bone(), place: at(1.0, 0.0, 0.0), rest: Rest { aim: posed, body: None }, shown: posed });
    }

    #[test]
    fn a_new_weapon_at_the_kept_guns_place_waits_for_its_idle_pose() {
        let (was, _) = kept(IDENTITY);
        assert_eq!(clip(was, gun(7), on_bone(at(1.0, 0.0, 0.0)), NOT_IDLE), (was, None));
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

    #[test]
    fn the_inverse_undoes() {
        for m in [at(1.0, -2.0, 3.0), turned(0.5, 0.25, 2.0), tilted(0.1, 0.2, 0.3)] {
            assert!(near(&product(&m, &inverse(&m).unwrap()), &IDENTITY), "{m:?}");
            assert!(near(&product(&inverse(&m).unwrap(), &m), &IDENTITY), "{m:?}");
        }
        let mut flat = IDENTITY;
        flat[10] = 0.0;
        assert_eq!(inverse(&flat), None);
    }

    /// Gun 7 at rest: the aim on the body's bone, both bones as `idle`.
    fn rested(idle: [f32; 16]) -> (Kept, [f32; 16]) {
        let (next, place) = posed(Kept::Nothing, gun(7), on_bone(at(0.0, 0.1, 0.0)), Some(body()), BACK, Some(|_| idle), |_| idle);
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
        let (next, place) = posed(was, gun(7), AimLocator::Posed { place: aim }, Some(body()), BACK, NOT_IDLE, |_| at(0.0, 0.0, 0.02));
        let place = place.unwrap();
        // the camera stays half the way behind the carried aim: the gun is seen 0.01 back
        assert!(near(&place, &at(0.0, 0.1, 0.01)), "{place:?}");
        let Kept::Gun { shown, rest, .. } = next else { panic!("nothing kept") };
        assert_eq!((shown, rest.aim), (place, aim));
    }

    #[test]
    fn the_sights_stay_on_the_view_when_the_clip_turns_the_body() {
        let (was, aim) = rested(IDENTITY);
        let kicked = tilted(0.0, 0.03, 0.0);
        let (_, place) = posed(was, gun(7), AimLocator::Posed { place: aim }, Some(body()), BACK, NOT_IDLE, |_| kicked);
        let place = place.unwrap();
        // the locator turns with the body, so the alignment undoes the turn
        assert_eq!(&place[..12], &kicked[..12]);
        // the carried aim is at (0, 0.11, 0.06), 0.042 back along its own z
        // (0, -0.6, 0.8) from the rest's: half of that is left
        assert!((place[12]).abs() < 1e-6 && (place[13] - 0.1226).abs() < 1e-6 && (place[14] - 0.0432).abs() < 1e-6, "{place:?}");
    }

    #[test]
    fn the_whole_clip_is_shown_by_a_gun_with_no_body_bone() {
        let idle = turned(0.0, 0.25, 2.0);
        let (next, place) = posed(Kept::Nothing, gun(7), on_bone(at(0.5, 1.0, 0.25)), None, BACK, Some(|_| idle), |_| panic!("no body to read"));
        assert_eq!(place, Some(product(&at(0.5, 1.0, 0.25), &idle)));
        assert_eq!(next, kept(idle).0);
    }

    #[test]
    fn a_frame_like_the_last_writes_nothing() {
        let (was, aim) = rested(IDENTITY);
        let kicked = at(0.0, 0.0, 0.02);
        let (next, place) = posed(was, gun(7), AimLocator::Posed { place: aim }, Some(body()), BACK, NOT_IDLE, |_| kicked);
        assert_eq!(posed(next, gun(7), AimLocator::Posed { place: place.unwrap() }, Some(body()), BACK, NOT_IDLE, |_| kicked), (next, None));
    }

    #[test]
    fn the_progress_lasts_a_frame_without_an_aimed_pass() {
        assert_eq!(taken(Aim::Seen(0.75)), (Aim::Taken(0.75), 0.75));
        assert_eq!(taken(Aim::Taken(0.75)), (Aim::Lowered, 0.75));
        assert_eq!(taken(Aim::Lowered), (Aim::Lowered, 0.0));
    }
}
