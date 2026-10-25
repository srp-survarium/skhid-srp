//! The corpse machine's tests: deaths, rollbacks, re-simulated deaths, the
//! pool and the local camera.

use super::*;
use crate::math::Quat;
use crate::ragdoll::tests::{Ground, standing};

const UP: Vec3 = Vec3::Y;

fn dead(key: u32) -> Seen {
    Seen::Dead(Death {
        key: DeathKey(key),
        cause: Cause::Unknown,
        gun: None,
        stood_on: Some((Vec3::ZERO, UP)),
    })
}

/// A frame: the step, then each place's draw of the given skeleton.
fn frame(
    corpses: &mut Corpses,
    places: &[Seen],
    local: Option<usize>,
    ground: &mut Ground,
) -> [Drawn; 4] {
    corpses.step(
        &Frame {
            dt: 1.0 / 60.0,
            up: UP,
            places,
            local,
        },
        ground,
    );
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
    assert_eq!(
        frame(&mut corpses, &alive, None, &mut ground),
        [Drawn::Game; 4]
    );
    let d = [dead(7), Seen::Alive];
    // two draws of the clip, then the ragdoll draws it
    assert_eq!(frame(&mut corpses, &d, None, &mut ground)[0], Drawn::Game);
    assert_eq!(stage(&corpses, 0), "dying");
    assert_eq!(frame(&mut corpses, &d, None, &mut ground)[0], Drawn::Game);
    assert_eq!(
        frame(&mut corpses, &d, None, &mut ground)[0],
        Drawn::Ragdoll
    );
    assert_eq!(stage(&corpses, 0), "ragdoll");
    for _ in 0..400 {
        frame(&mut corpses, &d, None, &mut ground);
    }
    assert_eq!(stage(&corpses, 0), "settled");
    assert_eq!(
        frame(&mut corpses, &d, None, &mut ground)[0],
        Drawn::Ragdoll
    );
    // the respawn
    assert_eq!(
        frame(&mut corpses, &alive, None, &mut ground)[0],
        Drawn::Game
    );
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
    assert!(matches!(
        (before[0], corpses.pool[0]),
        (Slot::Ragdoll { .. }, Slot::Ragdoll { .. })
    ));
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
    assert_eq!(
        corpses
            .pool
            .iter()
            .filter(|s| !matches!(s, Slot::Free))
            .count(),
        1
    );
}

#[test]
fn the_pool_gives_the_oldest_settled_slot_to_a_new_death() {
    let mut corpses = Corpses::new();
    let mut ground = Ground::flat();
    let mut seen = [Seen::Alive; MAX_PLACES];
    // POOL deaths, settled in order
    for place in 0..POOL {
        seen[place] = dead(100 + place as u32);
        corpses.step(
            &Frame {
                dt: 1.0 / 60.0,
                up: UP,
                places: &seen,
                local: None,
            },
            &mut ground,
        );
        for _ in 0..3 {
            if let Corpse::Body { slot, .. } = corpses.place(place) {
                corpses.slot_mut(slot).draw(&mut standing(
                    Quat::IDENTITY,
                    Vec3::new(place as f32 * 3.0, 0.0, 0.0),
                ));
            }
            corpses.step(
                &Frame {
                    dt: 1.0 / 60.0,
                    up: UP,
                    places: &seen,
                    local: None,
                },
                &mut ground,
            );
        }
        for _ in 0..400 {
            corpses.step(
                &Frame {
                    dt: 1.0 / 60.0,
                    up: UP,
                    places: &seen,
                    local: None,
                },
                &mut ground,
            );
        }
        assert_eq!(stage(&corpses, place), "settled", "place {place}");
    }
    // one more death: the first settled gives its slot
    seen[POOL] = dead(999);
    corpses.step(
        &Frame {
            dt: 1.0 / 60.0,
            up: UP,
            places: &seen,
            local: None,
        },
        &mut ground,
    );
    assert_eq!(stage(&corpses, POOL), "dying");
    assert_eq!(stage(&corpses, 0), "canned");
    assert_eq!(stage(&corpses, 1), "settled");
    // the canned corpse stays canned for its death, and is dropped at its respawn
    corpses.step(
        &Frame {
            dt: 1.0 / 60.0,
            up: UP,
            places: &seen,
            local: None,
        },
        &mut ground,
    );
    assert_eq!(stage(&corpses, 0), "canned");
    seen[0] = Seen::Alive;
    corpses.step(
        &Frame {
            dt: 1.0 / 60.0,
            up: UP,
            places: &seen,
            local: None,
        },
        &mut ground,
    );
    assert_eq!(stage(&corpses, 0), "alive");
    // places past MAX_PLACES are ignored
    let many = [dead(5); MAX_PLACES + 4];
    corpses.step(
        &Frame {
            dt: 1.0 / 60.0,
            up: UP,
            places: &many,
            local: None,
        },
        &mut ground,
    );
}

/// The cost of a frame: 20 places die in the same frame (a grenade's worst
/// case), every one a ragdoll from the pool, stepped at 60 frames a second
/// until all settle; the level a flat floor (its queries counted, not
/// timed: the game's ray_test is the rest of the cost).
///
/// `cargo test --release -p ragdoll cost -- --ignored --nocapture`
#[test]
#[ignore = "a measurement, not a check"]
fn cost_of_twenty_ragdolls() {
    extern crate std;
    use std::println;
    use std::time::Instant;
    let mut corpses = Corpses::new();
    let mut ground = Ground::flat();
    let mut seen = [Seen::Alive; 20];
    let turn = |p: usize| Quat::from_axis_angle(Vec3::Y, p as f32 * 0.7);
    // two frames of the clip each, then all of them ragdolls
    for p in 0..20 {
        // shot in the body, each from its own side (a body standing dead
        // still, balanced on its feet, would take seconds to tip over)
        let (s, c) = crate::math::sin_cos(p as f32 * 1.3);
        let cause = Cause::Body {
            part: crate::skeleton::Part::Torso,
            direction: Vec3::new(s, 0.0, c),
            heavy: false,
        };
        seen[p] = Seen::Dead(Death {
            key: DeathKey(1000 + p as u32),
            cause,
            gun: None,
            stood_on: Some((Vec3::ZERO, UP)),
        });
    }
    for _ in 0..3 {
        corpses.step(
            &Frame {
                dt: 1.0 / 60.0,
                up: UP,
                places: &seen,
                local: Some(0),
            },
            &mut ground,
        );
        for p in 0..20 {
            if let Corpse::Body { slot, .. } = corpses.place(p) {
                let mut s = standing(turn(p), Vec3::new(p as f32 * 3.0, 0.0, 0.0));
                corpses.slot_mut(slot).draw(&mut s);
            }
        }
    }
    let mut frames = std::vec::Vec::new();
    // the game's clip each frame: the pose drawn the frame before (a clip
    // that falls as the body does; a standing clip held for ever would keep
    // the drive's floor, 0.03, pulling every body up)
    let mut clip: std::vec::Vec<_> = (0..20)
        .map(|p| standing(turn(p), Vec3::new(p as f32 * 3.0, 0.0, 0.0)))
        .collect();
    loop {
        let asked = ground.queries;
        let start = Instant::now();
        corpses.step(
            &Frame {
                dt: 1.0 / 60.0,
                up: UP,
                places: &seen,
                local: Some(0),
            },
            &mut ground,
        );
        let step = start.elapsed();
        // each place's draw: the clip captured, the bones written
        let start = Instant::now();
        for p in 0..20 {
            if let Corpse::Body { slot, .. } = corpses.place(p) {
                corpses.slot_mut(slot).draw(&mut clip[p]);
            }
        }
        let draw = start.elapsed();
        let moving = corpses
            .pool
            .iter()
            .filter(|s| matches!(s, Slot::Ragdoll { .. }))
            .count();
        frames.push((
            step.as_secs_f64() * 1e6,
            draw.as_secs_f64() * 1e6,
            ground.queries - asked,
            moving,
        ));
        if moving == 0 || frames.len() > 600 {
            break;
        }
    }
    let worst = frames.iter().map(|f| f.0).fold(0.0, f64::max);
    let mean = frames.iter().map(|f| f.0).sum::<f64>() / frames.len() as f64;
    let draws = frames.iter().map(|f| f.1).fold(0.0, f64::max);
    let rays = frames.iter().map(|f| f.2).max().unwrap();
    println!(
        "20 ragdolls: {} frames until all settled ({:.2} s)",
        frames.len(),
        frames.len() as f64 / 60.0
    );
    println!("step: worst {worst:.0} us, mean {mean:.0} us a frame (all 20 moving at first)");
    println!("draws: worst {draws:.0} us a frame for all 20");
    println!(
        "queries: worst {rays} a frame, {} in all",
        frames.iter().map(|f| f.2).sum::<usize>()
    );
    for (i, f) in frames.iter().enumerate().step_by(30) {
        println!(
            "  frame {i:3}: step {:6.0} us, draws {:5.0} us, {:3} queries, {:2} moving",
            f.0, f.1, f.2, f.3
        );
    }
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
        assert!(
            (eye.position - last).length() < 0.15,
            "a jump of the camera"
        );
        assert!(eye.position.y > 0.15);
        last = eye.position;
    }
    // someone else's place: no camera
    frame(&mut corpses, &d, Some(0), &mut ground);
    assert_eq!(corpses.eye(), None);
    frame(&mut corpses, &[Seen::Alive; 2], Some(1), &mut ground);
    assert_eq!(corpses.eye(), None);
}
