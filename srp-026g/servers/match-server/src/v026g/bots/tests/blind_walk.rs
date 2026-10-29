//! A body with no world for a long walk (J0 0146be4a bot 3: the last world it was given was taken mid-fall at 28.65 s; the bot sprinted on
//! for 72 s). What its reckoning does with that world and a long run of the same keys.

use super::*;

use crate::v026g::bots::plays::{Go, Intent, Pace};
use crate::v026g::bots::view::Clock;

use crate::v026g::bots::body::BodyTrack;
use crate::v026g::bots::kin::Kinematics;

/// The estimate of a body given the fix of 28.65 s (feet (-50.24, 4.17, 31.32) above the floor of Chemical Plant, falling at 4.9 m/s, sprint and
/// forward held, yaw -1.37) and then 72 s of the same keys, printed every 5 s. A measurement, run with
/// `SRP_MAPDATA_DIR=<mapdata folder> cargo test -p match-server --release blind_walk -- --ignored --nocapture`.
#[test]
#[ignore = "a measurement; needs SRP_MAPDATA_DIR"]
fn blind_walk_after_a_mid_air_world() {
    let Some(dir) = std::env::var_os("SRP_MAPDATA_DIR").map(std::path::PathBuf::from) else {
        eprintln!("skipping: SRP_MAPDATA_DIR is not set");
        return;
    };
    let map = mapdata::MapData::read(&dir.join(mapdata::format::file_name("level_04/day_4"))).unwrap();
    let links = Tuning::default().off_mesh_links();
    let world = Arc::new(World::build_with_links(map, mapdata::CELL, mapdata::Body::default(), links));
    let kin = Kinematics::default();
    let mut track = BodyTrack::default();
    let feet = Vec3::new(-50.24, 4.17, 31.32);
    let fix = Fix { time: 28_650, alive: true, inserted: 1, feet: Some(feet), yaw: -1.37, pitch: 0.0, keys: 513, speed: Some(5.08) };
    track.fixed(fix, &kin, Some(&world.grid));
    let goal = Vec3::new(-38.38, 1.96, 33.67);
    let at = track.estimate().copied().unwrap();
    println!("fix: node {:?}, floor under {:?}", at.node, world.grid.locate(feet, 3.0).map(|n| world.grid.position(n)));
    for t in (28_675..=100_750).step_by(25) {
        track.decided(t, 513, (0.0, 0.0), 0.0, &kin, Some(&world.grid));
        if t % 5_000 == 0 {
            let e = track.estimate().unwrap();
            println!("t {t}: ({:.2}, {:.2}, {:.2}) node {:?}, {:.1} m from the goal", e.feet.x, e.feet.y, e.feet.z, e.node, e.feet.distance_xz(goal));
        }
    }
}

/// A walk across the soak field, the worlds given for the first 3 s only; the body's place after `until` ms and the distance to the goal.
fn walk_with_no_worlds_after_3s(blind_stop_ms: Option<u32>, until: u32) -> (Vec3, f32) {
    let world = soak_world();
    let start = Vec3::new(5.0, 0.0, 10.0);
    let goal = Vec3::new(50.0, 0.0, 35.0);
    let mut tuning = Tuning::default();
    tuning.unstick.blind_stop_ms = blind_stop_ms;
    let teams = [0u8, 1];
    let mut bots = Bots::new(0xB11D, survarium::v026g::maps::mode::TEAM_DEATHMATCH, 0, &teams, &[true, false], &[None, None], Arc::new(tuning), Trace::none(0));
    bots.map_ready(world.clone());
    let mut game = Game::new(world.clone(), &teams, 3, None, None);
    game.bodies[0].node = world.grid.locate(start, 1.0);
    game.bodies[0].feet = start;
    bots.entered(0, true, 0);
    let (mut clock, mut next_world) = (0, 1_000);
    while clock < until {
        clock += 25;
        while game.tick + TICK_MS <= clock {
            game.step();
        }
        if clock >= next_world {
            next_world += 1_000;
            if clock <= 3_000
                && let Some((_, fixes, health, facts)) = game.world_at(clock / TICK_MS * TICK_MS - 300)
            {
                for (place, (fix, health)) in fixes.into_iter().zip(health).enumerate() {
                    bots.fixed(place, fix, Some(health));
                }
                bots.facts(facts);
            }
        }
        let clock_now = Clock { clock, time_left_s: 600.0, total_s: 900.0, playing: true };
        let view = bots.tracker.view(&clock_now, &bots.match_wide_tuning().kin, Some(&world));
        bots.features.searching.left = SEARCHES_PER_POLL;
        bots.features.searching.floors = bots.match_wide_tuning().think.search_floors_per_poll as usize;
        bots.go_on_searching();
        if let Some(bot) = bots.bots[0].as_mut() {
            bot.intent = Intent { go: Go::To { at: goal, within_m: 0.0 }, pace: Pace::Run, fire: false, ..Intent::stay() };
        }
        let mut made = Vec::new();
        bots.ticks(0, &clock_now, &view, &mut made);
        if clock % 2_000 == 0 {
            let b = &game.bodies[0];
            eprintln!(
                "blind {blind_stop_ms:?} t {clock}: ({:.1}, {:.1}) route {} estimate {:?} made {}",
                b.feet.x,
                b.feet.z,
                bots.bots[0].as_ref().is_some_and(|bot| bot.route.is_some()),
                bots.tracker.body(0).and_then(|t| t.estimate()).map(|e| (e.feet.x, e.feet.z)),
                made.len()
            );
        }
        for made in made {
            let r = made.input.rotation;
            let turn = (half_value(u16::from_le_bytes([r[0], r[1]])), half_value(u16::from_le_bytes([r[2], r[3]])));
            let gun = Arms::fallback().primary.0;
            game.inputs[made.place].push_back((made.time, made.input.actions, turn, None, gun));
        }
    }
    (game.bodies[0].feet, game.bodies[0].feet.distance_xz(goal))
}

/// With no world for 4 s the walk stands (tuned), until a world comes; untuned it walks on to the goal on its reckoning.
#[test]
fn a_bot_with_no_world_for_the_tuned_time_stands_still() {
    let (_, far) = walk_with_no_worlds_after_3s(None, 25_000);
    assert!(far < 3.0, "untuned, the walk goes on to the goal: {far} m off");
    let (at, left) = walk_with_no_worlds_after_3s(Some(4_000), 25_000);
    assert!(left > 8.0, "tuned, it stands where its world ran out: {left} m from the goal");
    assert!(at.distance_xz(Vec3::new(5.0, 0.0, 10.0)) > 10.0, "it did walk at first: {at:?}");
}
