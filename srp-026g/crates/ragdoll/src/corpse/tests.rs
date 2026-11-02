//! The corpse machine's tests: deaths, rollbacks, re-simulated deaths, the
//! pool and the local camera.

use super::*;
use crate::math::Quat;
use crate::ragdoll::tests::{Ground, standing};
use crate::shot::Shot;
use crate::skeleton::BONE_COUNT;

const UP: Vec3 = Vec3::Y;

#[test]
fn headwear_belongs_to_one_death_and_rollback_discards_it() {
    let mut corpses = Corpses::new();
    let mut ground = Ground::flat();
    let Seen::Dead(mut death) = dead(7) else {
        unreachable!()
    };
    death.headwear = Some(HeadwearShape);
    let places = [Seen::Dead(death)];
    for _ in 0..30 {
        frame(&mut corpses, &places, None, &mut ground);
    }
    let Corpse::Body { slot, .. } = corpses.place(0) else {
        panic!("missing body")
    };
    let pose = corpses.slot(slot).body().unwrap().headwear_pose().unwrap();
    frame(&mut corpses, &places, None, &mut ground);
    assert!(
        (corpses
            .slot(slot)
            .body()
            .unwrap()
            .headwear_pose()
            .unwrap()
            .position
            - pose.position)
            .length()
            < 0.2
    );
    frame(&mut corpses, &[Seen::Alive], None, &mut ground);
    assert!(matches!(corpses.slot(slot), Slot::Free));
    for _ in 0..3 {
        frame(&mut corpses, &places, None, &mut ground);
    }
    let Corpse::Body { slot, .. } = corpses.place(0) else {
        panic!("missing replayed body")
    };
    assert!(
        corpses
            .slot(slot)
            .body()
            .unwrap()
            .headwear_pose()
            .unwrap()
            .position
            .y
            > 1.5
    );
}

fn dead(key: u32) -> Seen {
    Seen::Dead(Death::new(
        DeathKey(key),
        Cause::Unknown,
        None,
        Some((Vec3::ZERO, UP)),
    ))
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

/// 464f: a corpse whose player holds no item (a battery carrier's, once the
/// simulation drops the battery: mt_draw then makes no skeleton) is still
/// drawn, every bone from the ragdoll whatever the skeleton given, and it
/// settles; a dying body (the clip not captured) is left to the game.
#[test]
fn a_corpse_holding_no_item_is_still_drawn_and_settles() {
    let mut corpses = Corpses::new();
    let mut ground = Ground::flat();
    let d = [dead(7)];
    let step = |corpses: &mut Corpses, ground: &mut Ground| {
        corpses.step(
            &Frame {
                dt: 1.0 / 60.0,
                up: UP,
                places: &d,
                local: None,
            },
            ground,
        );
    };
    // the death: two draws of the clip (with the item), the ragdoll's start
    for _ in 0..2 {
        frame(&mut corpses, &d, None, &mut ground);
    }
    let Corpse::Body { slot, .. } = corpses.place(0) else {
        panic!("no body")
    };
    assert_eq!(stage(&corpses, 0), "dying");
    let mut empty = [Pose::IDENTITY; BONE_COUNT];
    assert_eq!(
        corpses.slot(slot).draw_alone(&mut empty),
        Drawn::Game,
        "a dying body is the game's"
    );
    step(&mut corpses, &mut ground);
    assert_eq!(stage(&corpses, 0), "ragdoll");
    // the item gone: no skeleton from the game, the ragdoll's own drawn
    let mut frames = 0;
    let mut first = None;
    while stage(&corpses, 0) != "settled" {
        let mut skeleton = [Pose::IDENTITY; BONE_COUNT];
        assert_eq!(corpses.slot(slot).draw_alone(&mut skeleton), Drawn::Ragdoll);
        // the same skeleton as a draw over the game's would give
        let mut over = standing(Quat::IDENTITY, Vec3::new(5.0, 0.0, 0.0));
        let mut copy = *corpses.slot(slot);
        copy.draw(&mut over);
        assert_eq!(skeleton, over, "every bone is the ragdoll's");
        first.get_or_insert(skeleton);
        step(&mut corpses, &mut ground);
        frames += 1;
        assert!(frames < 1200, "never settled");
    }
    let mut skeleton = [Pose::IDENTITY; BONE_COUNT];
    assert_eq!(corpses.slot(slot).draw_alone(&mut skeleton), Drawn::Ragdoll);
    let fell = first.unwrap()[crate::skeleton::HEAD.index()].position.y
        - skeleton[crate::skeleton::HEAD.index()].position.y;
    assert!(
        fell > 0.8,
        "it fell ({fell} m) and settled in {frames} frames"
    );
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
        seen[p] = Seen::Dead(Death::new(
            DeathKey(1000 + p as u32),
            cause,
            None,
            Some((Vec3::ZERO, UP)),
        ));
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
    let j = crate::shot::impulse(along, 0.02, 800.0);
    assert!(
        corpses
            .push(0, DeathKey(7), hit.hit.part, hit.hit.point, j)
            .is_some()
    );
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
    assert!(
        corpses
            .push(0, DeathKey(8), Part::LeftForearm, arm, along)
            .is_none()
    );
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
        crate::shot::impulse(along, 0.02, 800.0),
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
            place: hit.place as u8,
            death: hit.death.0,
        };
        log.first(key)
            && corpses
                .push(
                    hit.place,
                    hit.death,
                    hit.hit.part,
                    hit.hit.point,
                    impulse(shot.direction, 0.02, 800.0),
                )
                .is_some()
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
                    crate::shot::impulse(along, 0.02, 800.0),
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

/// The owner's report on 464b (2026-10-26): "on the ground the bodies often
/// start to dance". On a floor, a slope, stairs, against a wall and on a
/// battery, with the frame time the game has (it varies every frame): the
/// body comes to rest and is frozen, shows not one bone moved for 60 s
/// with no shot, and after a burst of shots settles again.
#[test]
fn a_lying_body_comes_to_rest_and_stays_still_on_every_surface() {
    extern crate std;
    use crate::ragdoll::scenes::scenes;
    let frame_times = [1.0 / 30.0, 1.0 / 144.0, 1.0 / 60.0, 1.0 / 45.0, 1.0 / 75.0];
    for mut scene in scenes() {
        let mut corpses = Corpses::new();
        // shot in the back: it falls forward, onto the stairs, the wall, the
        // battery (the scenes have them ahead of the body, +z)
        let mut death = Death::new(DeathKey(7), Cause::Unknown, None, Some((Vec3::ZERO, UP)));
        death.hits[0] = crate::shot::Hit {
            point: Vec3::new(0.0, 1.3, -0.12),
            impulse: crate::shot::impulse(Vec3::Z, 0.02, 800.0),
        };
        death.hit_count = 1;
        let d = [Seen::Dead(death)];
        let mut n = 0usize;
        let mut t = 0.0f32;
        let step = |corpses: &mut Corpses,
                    scene: &mut crate::ragdoll::scenes::Scene,
                    n: &mut usize,
                    t: &mut f32| {
            let dt = frame_times[*n % frame_times.len()];
            *n += 1;
            *t += dt;
            corpses.step(
                &Frame {
                    dt,
                    up: UP,
                    places: &d,
                    local: None,
                },
                scene,
            );
            let Corpse::Body { slot, .. } = corpses.place(0) else {
                panic!("no body")
            };
            let mut skeleton = standing(Quat::IDENTITY, Vec3::ZERO);
            corpses.slot_mut(slot).draw(&mut skeleton);
            skeleton
        };
        let mut last = step(&mut corpses, &mut scene, &mut n, &mut t);
        // to rest: settled within 4 s of the death, and no dance on the way:
        // a bone that moves back (against the last frame's move, both over
        // 5 mm) four times within 0.4 s shakes (the old dance did it frame
        // after frame for seconds); a fall or a tumble down stairs bounces
        // a limb once or twice
        let mut previous_moves = [Vec3::ZERO; BONE_COUNT];
        let mut reversals: [[f32; 4]; BONE_COUNT] = [[-9.0; 4]; BONE_COUNT];
        let mut shakes = 0;
        while stage(&corpses, 0) != "settled" {
            // (down stairs it slides and tumbles to the bottom first; the 5 s cap)
            let most = if scene.name == "stairs" { 5.2 } else { 4.0 };
            assert!(t < most, "{}: still moving at {t:.2} s", scene.name);
            let now = step(&mut corpses, &mut scene, &mut n, &mut t);
            for b in 0..BONE_COUNT {
                let m = now[b].position - last[b].position;
                let p = previous_moves[b];
                if m.length() > 0.005 && p.length() > 0.005 && m.dot(p) < 0.0 {
                    reversals[b] = [reversals[b][1], reversals[b][2], reversals[b][3], t];
                    if t - reversals[b][0] < 0.4 {
                        shakes += 1;
                    }
                }
                previous_moves[b] = m;
            }
            last = now;
        }
        let settled_at = t;
        // (down the stairs the body slides from step to step, its arms
        // knocked about by the edges: a tumble, it ends at rest below)
        if scene.name != "stairs" {
            assert_eq!(shakes, 0, "{}: bones shook", scene.name);
        }
        // 60 s with no shot: not one bone moves
        let frozen = last;
        for _ in 0..(60 * 60) {
            let now = step(&mut corpses, &mut scene, &mut n, &mut t);
            assert_eq!(now, frozen, "{}: a settled body moved", scene.name);
        }
        // a burst: five rifle rounds at the torso, 0.1 s apart; then rest again
        let mut log = crate::shot::ShotLog::new();
        for k in 0..5u32 {
            let target = part_pose(&corpses, 0, Part::Torso).position;
            let along = Vec3::new(0.3, -0.5, 1.0).direction().unwrap();
            let shot = Shot::new(target - along * 2.0, along, 4.0).unwrap();
            if let Some(hit) = corpses.trace(&shot) {
                let key = crate::shot::ShotKey {
                    shooter: 1,
                    bullet: k,
                    place: 0,
                    death: 7,
                };
                if log.first(key) {
                    corpses.push(
                        hit.place,
                        hit.death,
                        hit.hit.part,
                        hit.hit.point,
                        crate::shot::impulse(along, 0.02, 800.0),
                    );
                }
            }
            for _ in 0..6 {
                step(&mut corpses, &mut scene, &mut n, &mut t);
            }
        }
        let burst_end = t;
        while stage(&corpses, 0) != "settled" {
            assert!(
                t - burst_end < 4.0,
                "{}: still moving {:.2} s after the burst",
                scene.name,
                t - burst_end
            );
            step(&mut corpses, &mut scene, &mut n, &mut t);
        }
        let frozen = step(&mut corpses, &mut scene, &mut n, &mut t);
        for _ in 0..(10 * 60) {
            assert_eq!(
                step(&mut corpses, &mut scene, &mut n, &mut t),
                frozen,
                "{}: moved after resettling",
                scene.name
            );
        }
        std::println!(
            "{}: at rest {settled_at:.2} s after the death ({shakes} shakes), {:.2} s after the burst",
            scene.name,
            t - burst_end - 10.0
        );
    }
}

fn armed(key: u32) -> Seen {
    Seen::Dead(Death::new(DeathKey(key), Cause::Unknown, Some(GunShape::RIFLE), Some((Vec3::ZERO, UP))))
}

/// The `Weapon` bone the last draw of a place's ragdoll gave.
fn drawn_weapon(corpses: &Corpses, place: usize) -> Pose {
    let Corpse::Body { slot, .. } = corpses.place(place) else { panic!("no body") };
    let mut skeleton = standing(Quat::IDENTITY, Vec3::new(place as f32 * 3.0, 0.0, 0.0));
    corpses.slot(slot).draw_alone(&mut skeleton);
    skeleton[crate::skeleton::WEAPON.index()]
}

#[test]
fn a_respawn_while_the_gun_falls_leaves_it_falling_until_it_rests() {
    let mut corpses = Corpses::new();
    let mut ground = Ground::flat();
    let d = [armed(7)];
    // the ragdoll starts (two draws of the clip), one frame of its fall
    for _ in 0..4 {
        frame(&mut corpses, &d, None, &mut ground);
    }
    assert_eq!(stage(&corpses, 0), "ragdoll");
    let last = drawn_weapon(&corpses, 0);
    assert!(last.position.y > 0.5, "the gun still in the air: {:?}", last.position);
    // the respawn: the body goes, the gun goes on from where it was drawn
    frame(&mut corpses, &[Seen::Alive], None, &mut ground);
    assert_eq!(stage(&corpses, 0), "alive");
    let Lone::Moving { death, .. } = corpses.lone(0) else { panic!("{:?}", corpses.lone(0)) };
    assert_eq!(death, DeathKey(7));
    let (_, handed) = corpses.lone_gun(0).unwrap();
    assert!((handed.position - last.position).length() < 0.1, "no jump: {:?} {:?}", handed.position, last.position);
    let mut frames = 0;
    while matches!(corpses.lone(0), Lone::Moving { .. }) {
        frame(&mut corpses, &[Seen::Alive], None, &mut ground);
        frames += 1;
        assert!(frames < 400, "never rested");
    }
    let Lone::Resting { gun, .. } = corpses.lone(0) else { panic!() };
    assert!(gun.centre().y < 0.1, "at rest on the ground: {:?}", gun.centre());
    let rest = corpses.lone_gun(0).unwrap().1.position;
    for _ in 0..30 {
        frame(&mut corpses, &[Seen::Alive], None, &mut ground);
    }
    assert_eq!(corpses.lone_gun(0).unwrap().1.position, rest, "kept where it lies");
    // the place's next death ends it
    frame(&mut corpses, &[armed(9)], None, &mut ground);
    assert_eq!(corpses.lone(0), Lone::None);
}

#[test]
fn a_respawn_after_the_ragdoll_settled_leaves_the_gun_at_rest() {
    let mut corpses = Corpses::new();
    let mut ground = Ground::flat();
    let d = [armed(7)];
    for _ in 0..3 {
        frame(&mut corpses, &d, None, &mut ground);
    }
    until_settled(&mut corpses, &d, &mut ground);
    let last = drawn_weapon(&corpses, 0);
    frame(&mut corpses, &[Seen::Alive], None, &mut ground);
    let Lone::Resting { .. } = corpses.lone(0) else { panic!("{:?}", corpses.lone(0)) };
    assert!((corpses.lone_gun(0).unwrap().1.position - last.position).length() < 1e-5);
    // a rollback that undoes the respawn brings the body back: its ragdoll
    // starts again and the lone gun goes
    frame(&mut corpses, &d, None, &mut ground);
    assert_eq!(corpses.lone(0), Lone::None);
    assert_eq!(stage(&corpses, 0), "dying");
}

#[test]
fn a_body_that_had_no_gun_or_no_ragdoll_leaves_none() {
    let mut corpses = Corpses::new();
    let mut ground = Ground::flat();
    for _ in 0..4 {
        frame(&mut corpses, &[dead(7)], None, &mut ground);
    }
    frame(&mut corpses, &[Seen::Alive], None, &mut ground);
    assert_eq!(corpses.lone(0), Lone::None, "no gun dropped");
    // a respawn before the ragdoll started (still dying)
    frame(&mut corpses, &[armed(8)], None, &mut ground);
    frame(&mut corpses, &[Seen::Alive], None, &mut ground);
    assert_eq!(corpses.lone(0), Lone::None);
}
