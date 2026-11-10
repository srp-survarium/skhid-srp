//! A dropped thing among corpses, on the made-up skeleton and ground.

use super::*;
use crate::corpse::tests::DrawGame;
use crate::corpse::{Corpse, Death, Frame, Seen};
use crate::math::Quat;
use crate::ragdoll::Cause;
use crate::ragdoll::tests::{Ground, standing};
use crate::skeleton::Part;

const UP: Vec3 = Vec3::Y;
/// The battery's: 0.6 m long, 0.24 m thick.
const SHAPE: DroppedShape =
    DroppedShape { centre: Vec3::new(0.0, 0.12, 0.0), capsule: Capsule { half: 0.18, radius: 0.12 }, mass: 6.0 };

fn dead(key: u32) -> Seen {
    Seen::Dead(Death::new(DeathKey(key), Cause::Unknown, None, Some((Vec3::ZERO, UP))))
}

/// A frame of the corpses (their step and draws) and of the thing.
fn frame(corpses: &mut Corpses, places: &[Seen], thing: &mut Dropped, ground: &mut Ground) {
    corpses.step(&Frame { dt: 1.0 / 60.0, up: UP, places, local: None }, ground);
    if let Corpse::Body { slot, .. } = corpses.place(0) {
        let mut skeleton = standing(Quat::IDENTITY, Vec3::ZERO);
        corpses.slot_mut(slot).draw_game(&mut skeleton);
    }
    thing.step(1.0 / 60.0, UP, corpses, ground);
}

fn lowest(thing: &Dropped) -> f32 {
    let body = thing.alone.body;
    let (a, b) = body.shape.ends(body.pose);
    a.y.min(b.y) - body.shape.radius
}

fn until_lying(corpses: &mut Corpses, places: &[Seen], thing: &mut Dropped, ground: &mut Ground) -> usize {
    for n in 1..=900 {
        frame(corpses, places, thing, ground);
        assert!(thing.model().is_finite());
        assert!(lowest(thing) > -0.02, "under the floor: {}", lowest(thing));
        if matches!(thing.lie, Lie::Lying { .. }) && corpses.body(0).is_none_or(|(_, _, moving)| !moving) {
            return n;
        }
    }
    panic!("never at rest: {:?}", thing.lie);
}

#[test]
fn a_thing_dropped_on_open_ground_falls_and_lies_on_it() {
    let mut corpses = Corpses::new();
    let mut ground = Ground::flat();
    let model = Pose::new(Quat::IDENTITY, Vec3::new(5.0, 1.0, 0.0));
    let mut thing = Dropped::new(model, SHAPE);
    assert!((thing.model().position - model.position).length() < 1e-5, "it starts where it is drawn");
    let frames = until_lying(&mut corpses, &[Seen::Alive], &mut thing, &mut ground);
    assert!(frames < 300, "{frames} frames");
    assert_eq!(thing.lie, Lie::Lying { on: Places(0) });
    let lies = thing.model();
    assert!(lowest(&thing).abs() < 0.02, "on the ground: {}", lowest(&thing));
    for _ in 0..60 {
        frame(&mut corpses, &[Seen::Alive], &mut thing, &mut ground);
    }
    assert_eq!(thing.model(), lies, "at rest it does not creep");
}

#[test]
fn a_thing_dropped_on_a_corpse_is_stopped_by_it_and_wakes_when_the_corpse_goes() {
    let mut corpses = Corpses::new();
    let mut ground = Ground::flat();
    let d = [dead(7)];
    // a far thing while the body falls and settles
    let mut far = Dropped::new(Pose::new(Quat::IDENTITY, Vec3::new(50.0, 0.0, 0.0)), SHAPE);
    until_lying(&mut corpses, &d, &mut far, &mut ground);
    let (_, ragdoll, _) = corpses.body(0).unwrap();
    let chest = ragdoll.parts()[Part::Torso.index()];
    let top = chest.pose.position.y + chest.shape.radius;
    // let go half a metre above the chest, across it
    let along = chest.pose.rotation.rotate(Vec3::X).reject(UP).direction_or(Vec3::X);
    let across = Quat::from_basis(UP.cross(along), UP, along);
    let model = Pose::new(across, Vec3::new(chest.pose.position.x, top + 0.5, chest.pose.position.z));
    let mut thing = Dropped::new(model, SHAPE);
    // the chest stops its fall (it then rolls off the round chest, or stays)
    let mut deepest = 0.0f32;
    let mut stopped = false;
    for _ in 0..600 {
        frame(&mut corpses, &d, &mut thing, &mut ground);
        let (_, ragdoll, _) = corpses.body(0).unwrap();
        let body = thing.alone.body;
        let (c, e) = body.shape.ends(body.pose);
        for p in ragdoll.parts() {
            let (a, b) = p.shape.ends(p.pose);
            let (q, r) = crate::ragdoll::closest_points(a, b, c, e);
            deepest = deepest.max(p.shape.radius + body.shape.radius - (q - r).length());
        }
        stopped |= body.velocity.length() < 0.5 && lowest(&thing) > top - 0.05;
    }
    assert!(stopped, "the chest did not stop it");
    assert!(deepest < 0.03, "{deepest} m into the body");
    // laid on the ground against the body's side, it rests on the body too
    let side = chest.pose.position + UP.cross(along) * (chest.shape.radius + SHAPE.capsule.radius - 0.005);
    let leaning = Quat::from_basis(along, UP, along.cross(UP));
    let mut thing = Dropped::new(Pose::new(leaning, Vec3::new(side.x, 0.0, side.z)), SHAPE);
    until_lying(&mut corpses, &d, &mut thing, &mut ground);
    assert_eq!(thing.lie, Lie::Lying { on: Places(1) }, "against the corpse");
    frame(&mut corpses, &[Seen::Alive], &mut thing, &mut ground);
    assert_eq!(thing.lie, Lie::Falling, "woken as the body goes");
    // the respawn: the body goes, the thing falls to the ground
    until_lying(&mut corpses, &[Seen::Alive], &mut thing, &mut ground);
    assert_eq!(thing.lie, Lie::Lying { on: Places(0) });
    assert!(lowest(&thing).abs() < 0.02, "on the ground: {}", lowest(&thing));
}

#[test]
fn a_corpse_that_dies_on_a_lying_thing_does_not_throw_it() {
    let mut corpses = Corpses::new();
    let mut ground = Ground::flat();
    // the thing lies at the feet of the player who then dies over it
    let mut thing = Dropped::new(Pose::new(Quat::IDENTITY, Vec3::new(0.0, 0.0, 0.1)), SHAPE);
    until_lying(&mut corpses, &[Seen::Alive], &mut thing, &mut ground);
    let lay = thing.model().position;
    let d = [dead(7)];
    let mut fastest = 0.0f32;
    for _ in 0..600 {
        frame(&mut corpses, &d, &mut thing, &mut ground);
        fastest = fastest.max(thing.alone.body.velocity.length());
        assert!(lowest(&thing) > -0.02, "under the floor: {}", lowest(&thing));
    }
    assert!(fastest <= crate::tuning::LOOSE_PUSH_SPEED + 1.0, "thrown at {fastest} m/s");
    assert!((thing.model().position - lay).length() < 1.0, "it stays about where it lay");
}
