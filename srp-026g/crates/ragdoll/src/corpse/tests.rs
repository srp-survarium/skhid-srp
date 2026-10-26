//! The corpse machine's tests: deaths, rollbacks, re-simulated deaths, the
//! pool and the local camera.

use super::*;
use crate::math::Quat;
use crate::ragdoll::tests::{Ground, standing};
use crate::shot::Shot;

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

/// A place's drawn ragdoll part's pose.
fn part_pose(corpses: &Corpses, place: usize, part: Part) -> Pose {
    let Corpse::Body { slot, .. } = corpses.place(place) else {
        panic!("no body")
    };
    corpses.slot(slot).body().expect("a ragdoll").poses().0[part.index()]
}

/// Runs place 0's death until it settles; the frames it took.
fn until_settled(corpses: &mut Corpses, d: &[Seen], ground: &mut Ground) -> usize {
    for n in 0..1200 {
        if stage(corpses, 0) == "settled" {
            return n;
        }
        frame(corpses, d, None, ground);
    }
    panic!("never settled");
}

/// A level shot at a part's centre from 3 m to its side.
fn shot_at(corpses: &Corpses, part: Part, along: Vec3) -> Shot {
    let target = part_pose(corpses, 0, part).position;
    Shot::new(target - along * 3.0, along, 6.0).unwrap()
}

#[test]
fn a_shot_pushes_the_part_it_hits_and_wakes_a_settled_body_that_settles_again() {
    let mut corpses = Corpses::new();
    let mut ground = Ground::flat();
    let d = [dead(7)];
    until_settled(&mut corpses, &d, &mut ground);
    let along = Vec3::new(1.0, 0.0, 0.0);
    let shot = shot_at(&corpses, Part::Head, along);
    let hit = corpses.trace(&shot).expect("the head is hit");
    assert_eq!(
        (hit.place, hit.death, hit.hit.part),
        (0, DeathKey(7), Part::Head)
    );
    assert!(
        hit.hit.normal.dot(along) < 0.0,
        "the normal faces the shooter"
    );
    let head = part_pose(&corpses, 0, Part::Head).position;
    let pelvis = part_pose(&corpses, 0, Part::Pelvis).position;
    let j = crate::shot::impulse(along, Some(0.4));
    assert!(corpses.push(0, DeathKey(7), hit.hit.part, hit.hit.point, j));
    assert_eq!(stage(&corpses, 0), "ragdoll", "woken");
    for _ in 0..20 {
        frame(&mut corpses, &d, None, &mut ground);
    }
    let moved = part_pose(&corpses, 0, Part::Head).position - head;
    let pelvis_moved = (part_pose(&corpses, 0, Part::Pelvis).position - pelvis).length();
    assert!(
        moved.dot(along) > 0.08,
        "the head went with the shot: {moved:?}"
    );
    assert!(
        pelvis_moved < moved.length(),
        "the head moved more than the pelvis"
    );
    let frames = until_settled(&mut corpses, &d, &mut ground);
    assert!(frames < 6 * 60, "settled again in {frames} frames");
    // still on the ground, not through it
    assert!(part_pose(&corpses, 0, Part::Head).position.y > 0.05);
}

#[test]
fn a_shot_pushes_a_moving_ragdoll_and_a_miss_does_nothing() {
    let mut corpses = Corpses::new();
    let mut ground = Ground::flat();
    let d = [dead(7)];
    for _ in 0..6 {
        frame(&mut corpses, &d, None, &mut ground);
    }
    assert_eq!(stage(&corpses, 0), "ragdoll");
    let along = Vec3::new(0.0, 0.0, 1.0);
    // a miss: beside the body, above it, and short of it
    let arm = part_pose(&corpses, 0, Part::LeftForearm).position;
    for shot in [
        Shot::new(arm + Vec3::new(1.5, 0.0, -3.0), along, 6.0).unwrap(),
        Shot::new(arm + Vec3::new(0.0, 2.5, -3.0), along, 6.0).unwrap(),
        Shot::new(arm + Vec3::new(0.0, 0.0, -3.0), along, 1.0).unwrap(),
    ] {
        assert_eq!(corpses.trace(&shot), None);
    }
    // the wrong death (the place died again since): nothing pushed
    assert!(!corpses.push(0, DeathKey(8), Part::LeftForearm, arm, along));
    // the forearm, along +Z
    let mut pushed = Corpses::new();
    let mut alone = Corpses::new();
    for c in [&mut pushed, &mut alone] {
        for _ in 0..6 {
            frame(c, &d, None, &mut ground);
        }
    }
    let shot = shot_at(&pushed, Part::LeftForearm, along);
    let hit = pushed.trace(&shot).expect("hit");
    assert_eq!(hit.hit.part, Part::LeftForearm);
    pushed.push(
        0,
        DeathKey(7),
        hit.hit.part,
        hit.hit.point,
        crate::shot::impulse(along, None),
    );
    for _ in 0..10 {
        frame(&mut pushed, &d, None, &mut ground);
        frame(&mut alone, &d, None, &mut ground);
    }
    let gap = part_pose(&pushed, 0, Part::LeftForearm).position
        - part_pose(&alone, 0, Part::LeftForearm).position;
    assert!(
        gap.z > 0.05,
        "the shot forearm is ahead of the unshot one: {gap:?}"
    );
}

#[test]
fn the_same_shot_traced_again_pushes_once() {
    use crate::shot::{ShotKey, ShotLog, impulse};
    // what the arena does with each traced segment: the log decides, then
    // the push; a rollback traces the same bullet's segment again
    let shoot = |corpses: &mut Corpses, log: &mut ShotLog, shot: &Shot| {
        let Some(hit) = corpses.trace(shot) else {
            return false;
        };
        let key = ShotKey {
            shooter: 2,
            bullet: 41,
            segment: 0,
            place: hit.place as u8,
            death: hit.death.0,
        };
        log.first(key)
            && corpses.push(
                hit.place,
                hit.death,
                hit.hit.part,
                hit.hit.point,
                impulse(shot.direction, None),
            )
    };
    let mut ground = Ground::flat();
    let d = [dead(7)];
    let mut once = Corpses::new();
    let mut twice = Corpses::new();
    for c in [&mut once, &mut twice] {
        until_settled(c, &d, &mut ground);
    }
    let along = Vec3::new(1.0, 0.0, 0.0);
    let shot = shot_at(&once, Part::Torso, along);
    let (mut log_once, mut log_twice) = (ShotLog::new(), ShotLog::new());
    assert!(shoot(&mut once, &mut log_once, &shot));
    assert!(shoot(&mut twice, &mut log_twice, &shot));
    assert!(
        !shoot(&mut twice, &mut log_twice, &shot),
        "the re-simulated trace is not pushed"
    );
    for _ in 0..30 {
        frame(&mut once, &d, None, &mut ground);
        frame(&mut twice, &d, None, &mut ground);
    }
    for part in Part::ALL {
        let (a, b) = (part_pose(&once, 0, part), part_pose(&twice, 0, part));
        assert!((a.position - b.position).length() < 1e-6, "{part:?}");
    }
}

#[test]
#[ignore = "a measurement, not a check"]
fn cost_of_shots_at_twenty_ragdolls() {
    extern crate std;
    use std::println;
    use std::time::Instant;
    let mut corpses = Corpses::new();
    let mut ground = Ground::flat();
    let seen: std::vec::Vec<Seen> = (0..20).map(|p| dead(1000 + p)).collect();
    for _ in 0..4 {
        frame20(&mut corpses, &seen, &mut ground);
    }
    // 20 bodies in a row 3 m apart; shots from 50 m away in all directions,
    // a fifth of them through the row
    let shots: std::vec::Vec<Shot> = (0..4000)
        .map(|i| {
            let (s, c) = crate::math::sin_cos(i as f32 * 0.37);
            let target = if i % 5 == 0 {
                Vec3::new((i % 20) as f32 * 3.0, 0.5, 0.0)
            } else {
                Vec3::new(30.0, 1.0, 40.0)
            };
            let from = target - Vec3::new(s, 0.05, c) * 50.0;
            Shot::new(from, target - from, 60.0).unwrap()
        })
        .collect();
    for label in ["moving", "settled"] {
        let start = Instant::now();
        let hits = shots.iter().filter(|s| corpses.trace(s).is_some()).count();
        let t = start.elapsed().as_secs_f64() * 1e6;
        println!(
            "{label}: {} traces at 20 ragdolls, {hits} hits: {t:.0} us ({:.2} us a trace)",
            shots.len(),
            t / shots.len() as f64
        );
        for _ in 0..600 {
            frame20(&mut corpses, &seen, &mut ground);
        }
    }
    // heavy fire: every body hit every frame for 2 s (20 pushes a frame,
    // each body kept moving), the step's cost
    let mut worst = 0.0f64;
    let mut total = 0.0f64;
    let mut rays = 0usize;
    for f in 0..120 {
        for p in 0..20usize {
            let (s, c) = crate::math::sin_cos((f * 20 + p) as f32 * 0.9);
            let target =
                part_pose(&corpses, p, Part::ALL[(f + p) % crate::skeleton::PARTS]).position;
            let along = Vec3::new(s, -0.3, c).direction().unwrap();
            let shot = Shot::new(target - along * 2.0, along, 4.0).unwrap();
            if let Some(hit) = corpses.trace(&shot) {
                corpses.push(
                    hit.place,
                    hit.death,
                    hit.hit.part,
                    hit.hit.point,
                    crate::shot::impulse(along, None),
                );
            }
        }
        let asked = ground.queries;
        let start = Instant::now();
        frame20(&mut corpses, &seen, &mut ground);
        let t = start.elapsed().as_secs_f64() * 1e6;
        rays = rays.max(ground.queries - asked);
        worst = worst.max(t);
        total += t;
    }
    println!(
        "heavy fire (20 hits a frame, 2 s): step and draws worst {worst:.0} us, mean {:.0} us a frame, up to {rays} rays",
        total / 120.0
    );
    let mut frames = 0;
    while corpses
        .pool
        .iter()
        .any(|s| matches!(s, Slot::Ragdoll { .. }))
        && frames < 600
    {
        frame20(&mut corpses, &seen, &mut ground);
        frames += 1;
    }
    println!("then all settled again in {:.2} s", frames as f64 / 60.0);
    // the miss of every box (no ragdoll near): the broad phase alone
    let far: std::vec::Vec<Shot> = (0..4000)
        .map(|i| {
            Shot::new(
                Vec3::new(i as f32, 50.0, -100.0),
                Vec3::new(0.0, 0.0, 1.0),
                200.0,
            )
            .unwrap()
        })
        .collect();
    let start = Instant::now();
    let hits = far.iter().filter(|s| corpses.trace(s).is_some()).count();
    let t = start.elapsed().as_secs_f64() * 1e6;
    println!(
        "far: 4000 traces, {hits} hits: {t:.0} us ({:.3} us a trace)",
        t / 4000.0
    );
}

/// A frame of 20 places (`frame` draws 4).
fn frame20(corpses: &mut Corpses, places: &[Seen], ground: &mut Ground) {
    corpses.step(
        &Frame {
            dt: 1.0 / 60.0,
            up: UP,
            places,
            local: None,
        },
        ground,
    );
    for place in 0..places.len() {
        if let Corpse::Body { slot, .. } = corpses.place(place) {
            let mut skeleton = standing(Quat::IDENTITY, Vec3::new(place as f32 * 3.0, 0.0, 0.0));
            corpses.slot_mut(slot).draw(&mut skeleton);
        }
    }
}
