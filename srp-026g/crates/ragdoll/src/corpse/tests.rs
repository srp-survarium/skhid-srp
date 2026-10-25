//! The corpse machine's tests: deaths, rollbacks, re-simulated deaths, the
//! pool and the local camera.

use super::*;
use crate::ragdoll::tests::{Ground, standing};
use crate::math::Quat;

const UP: Vec3 = Vec3::Y;

fn dead(key: u32) -> Seen {
    Seen::Dead(Death { key: DeathKey(key), cause: Cause::Unknown, gun: None })
}

/// A frame: the step, then each place's draw of the given skeleton.
fn frame(corpses: &mut Corpses, places: &[Seen], local: Option<usize>, ground: &mut Ground) -> [Drawn; 4] {
    corpses.step(&Frame { dt: 1.0 / 60.0, up: UP, places, local }, ground);
    let mut out = [Drawn::Game; 4];
    for (place, o) in out.iter_mut().enumerate() {
        if let Corpse::Body { slot, .. } = corpses.place(place) {
            let mut skeleton = standing(Quat::IDENTITY, Vec3::new(place as f32 * 3.0, 0.0, 0.0));
            *o = corpses.slot_mut(slot).draw(&mut skeleton);
        }
    }
    out
}

fn stage(corpses: &Corpses, place: usize) -> &'static str {
    match corpses.place(place) {
        Corpse::Alive => "alive",
        Corpse::Canned { .. } => "canned",
        Corpse::Body { slot, .. } => match corpses.slot(slot) {
            Slot::Free => "free",
            Slot::Dying { .. } => "dying",
            Slot::Ragdoll { .. } => "ragdoll",
            Slot::Settled { .. } => "settled",
        },
    }
}

#[test]
fn a_death_goes_dying_ragdoll_settled_and_a_respawn_drops_it() {
    let mut corpses = Corpses::new();
    let mut ground = Ground::flat();
    let alive = [Seen::Alive; 2];
    assert_eq!(frame(&mut corpses, &alive, None, &mut ground), [Drawn::Game; 4]);
    let d = [dead(7), Seen::Alive];
    // two draws of the clip, then the ragdoll draws it
    assert_eq!(frame(&mut corpses, &d, None, &mut ground)[0], Drawn::Game);
    assert_eq!(stage(&corpses, 0), "dying");
    assert_eq!(frame(&mut corpses, &d, None, &mut ground)[0], Drawn::Game);
    assert_eq!(frame(&mut corpses, &d, None, &mut ground)[0], Drawn::Ragdoll);
    assert_eq!(stage(&corpses, 0), "ragdoll");
    for _ in 0..400 {
        frame(&mut corpses, &d, None, &mut ground);
    }
    assert_eq!(stage(&corpses, 0), "settled");
    assert_eq!(frame(&mut corpses, &d, None, &mut ground)[0], Drawn::Ragdoll);
    // the respawn
    assert_eq!(frame(&mut corpses, &alive, None, &mut ground)[0], Drawn::Game);
    assert_eq!(stage(&corpses, 0), "alive");
    assert!(corpses.pool.iter().all(|s| matches!(s, Slot::Free)));
}

#[test]
fn a_rollback_that_undoes_the_death_drops_the_ragdoll_and_the_same_death_keeps_it() {
    let mut corpses = Corpses::new();
    let mut ground = Ground::flat();
    let d = [dead(7)];
    for _ in 0..10 {
        frame(&mut corpses, &d, None, &mut ground);
    }
    assert_eq!(stage(&corpses, 0), "ragdoll");
    let before = corpses.pool;
    // the world re-simulated: the same death (same key) keeps the ragdoll as it was
    frame(&mut corpses, &d, None, &mut ground);
    assert_eq!(stage(&corpses, 0), "ragdoll");
    assert!(matches!((before[0], corpses.pool[0]), (Slot::Ragdoll { .. }, Slot::Ragdoll { .. })));
    // a rollback before the death: alive at once
    frame(&mut corpses, &[Seen::Alive], None, &mut ground);
    assert_eq!(stage(&corpses, 0), "alive");
    // and another death (another key) starts over, from the clip
    frame(&mut corpses, &[dead(8)], None, &mut ground);
    assert_eq!(stage(&corpses, 0), "dying");
    for _ in 0..3 {
        frame(&mut corpses, &[dead(8)], None, &mut ground);
    }
    // a different death of the place while a ragdoll moves: dying again
    frame(&mut corpses, &[dead(9)], None, &mut ground);
    assert_eq!(stage(&corpses, 0), "dying");
    assert_eq!(corpses.pool.iter().filter(|s| !matches!(s, Slot::Free)).count(), 1);
}

#[test]
fn the_pool_gives_the_oldest_settled_slot_to_a_new_death() {
    let mut corpses = Corpses::new();
    let mut ground = Ground::flat();
    let mut seen = [Seen::Alive; MAX_PLACES];
    // POOL deaths, settled in order
    for place in 0..POOL {
        seen[place] = dead(100 + place as u32);
        corpses.step(&Frame { dt: 1.0 / 60.0, up: UP, places: &seen, local: None }, &mut ground);
        for _ in 0..3 {
            if let Corpse::Body { slot, .. } = corpses.place(place) {
                corpses.slot_mut(slot).draw(&mut standing(Quat::IDENTITY, Vec3::new(place as f32 * 3.0, 0.0, 0.0)));
            }
            corpses.step(&Frame { dt: 1.0 / 60.0, up: UP, places: &seen, local: None }, &mut ground);
        }
        for _ in 0..400 {
            corpses.step(&Frame { dt: 1.0 / 60.0, up: UP, places: &seen, local: None }, &mut ground);
        }
        assert_eq!(stage(&corpses, place), "settled", "place {place}");
    }
    // one more death: the first settled gives its slot
    seen[POOL] = dead(999);
    corpses.step(&Frame { dt: 1.0 / 60.0, up: UP, places: &seen, local: None }, &mut ground);
    assert_eq!(stage(&corpses, POOL), "dying");
    assert_eq!(stage(&corpses, 0), "canned");
    assert_eq!(stage(&corpses, 1), "settled");
    // the canned corpse stays canned for its death, and is dropped at its respawn
    corpses.step(&Frame { dt: 1.0 / 60.0, up: UP, places: &seen, local: None }, &mut ground);
    assert_eq!(stage(&corpses, 0), "canned");
    seen[0] = Seen::Alive;
    corpses.step(&Frame { dt: 1.0 / 60.0, up: UP, places: &seen, local: None }, &mut ground);
    assert_eq!(stage(&corpses, 0), "alive");
    // places past MAX_PLACES are ignored
    let many = [dead(5); MAX_PLACES + 4];
    corpses.step(&Frame { dt: 1.0 / 60.0, up: UP, places: &many, local: None }, &mut ground);
}

#[test]
fn the_local_players_camera_follows_its_ragdoll_and_stops_at_the_respawn() {
    let mut corpses = Corpses::new();
    let mut ground = Ground::flat();
    let d = [Seen::Alive, dead(3)];
    frame(&mut corpses, &d, Some(1), &mut ground);
    assert_eq!(corpses.eye(), None, "no camera while the clip plays");
    for _ in 0..3 {
        frame(&mut corpses, &d, Some(1), &mut ground);
    }
    let first = corpses.eye().expect("on the ragdoll's head");
    let mut last = first.position;
    for _ in 0..200 {
        frame(&mut corpses, &d, Some(1), &mut ground);
        let eye = corpses.eye().unwrap();
        assert!((eye.position - last).length() < 0.15, "a jump of the camera");
        assert!(eye.position.y > 0.15);
        last = eye.position;
    }
    // someone else's place: no camera
    frame(&mut corpses, &d, Some(0), &mut ground);
    assert_eq!(corpses.eye(), None);
    frame(&mut corpses, &[Seen::Alive; 2], Some(1), &mut ground);
    assert_eq!(corpses.eye(), None);
}
