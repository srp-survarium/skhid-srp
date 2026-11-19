//! [drop give-ups] The planned walk-offs the lip guard braked (fix-builder's diagnostic match of 2026-11-19, level_02 day 2:
//! 7 of 20 drops given up): a bot sent down a drop of the
//! spot from starts on its top whose planned route takes one (the edge strip's walk-offs lie side by side, a route takes the one it
//! pulls straight to), on the stand-in over the real mapdata. Per start the guard must not brake it on the drop's approach and leg
//! (on its route's line). Where it comes down is reported, not asserted: the exe makes the strip's drop on the port's own approach
//! (forward held through the fall, 0.09 m from its landing), but the stand-in does not model this rim (the client's capsule rides
//! its slope, which the grid holds as one flat lip cell, and a walk-off falls on the walk branch, the keys' walk moving the body),
//! so its landing here is not the client's. A regression test on the real map data, ignored without it.
//!
//! ```text
//! SRP_MAPDATA_DIR=<dir> [SRP_BENCH_GRID_CACHE=<dir>] cargo test -p match-server --release drop_spots -- --ignored --nocapture
//! ```

use super::*;

use crate::v026g::bots::path;
use crate::v026g::bots::plays::{Go, Intent, Pace};
use crate::v026g::bots::view::Clock;

/// A spot: its project, a take-off and landing of it (map frame), and its name in the report. The exe makes the walk the port plays
/// here (over the bar onto the strip, turned at the take-off (11.20, 19.40, 38.47) to its landing (10.70, 16.36, 37.97), forward
/// held through the fall, as the bots' walk-off holds it): down 0.09 m from that landing (`strip_turn_exe_v1` on the game's
/// objects, 2026-11-19: the port's own feet steps, then the client's 6-tick blend); a walk-off the exe does not make (the roof at (36.49, 21.76, 113.97): a walk is
/// held on its top) is a link fault, not this test's. A route's drop is the spot's when it
/// leaves within `take_off_m` of the take-off for a floor within `landing_m` of the landing.
struct Spot {
    name: &'static str,
    project: &'static str,
    take_off: Vec3,
    landing: Vec3,
    /// How near the take-off a route's drop leaves to be the spot's, m.
    take_off_m: f32,
    /// How near the landing its landing lies, m.
    landing_m: f32,
    /// The least distance of a start from the take-off, m (the most is 8 m).
    start_m: f32,
}

const SPOTS: [Spot; 2] = [
    // The strip as the match gave its drops up: any of its walk-offs side by side 0.25 m apart, the one a route pulls straight to.
    Spot {
        name: "level_02 edge strip walk-off 3.2 m down, 1.77 m across",
        project: "level_02/day_2",
        take_off: Vec3::new(11.20, 19.40, 37.97),
        landing: Vec3::new(9.95, 16.18, 39.22),
        take_off_m: 1.3,
        landing_m: 1.6,
        start_m: 3.0,
    },
    // The match's own link of the strip, exactly (a gap drop). No route planned from the strip's top 3-8 m off takes it (they take
    // the strip's neighbouring walk-offs), and the plan from the link's take-off node does: its starts begin 0.5 m off. The exe makes
    // it as the port walks it from the bar's top 0.7 m off (down the step to the take-off (11.20, 19.40, 37.72), turned to the landing
    // (9.95, 16.20, 38.97) over the client's 6-tick blend, forward held through the fall): down 0.06 m from that landing
    // (`bar_exact_exe_v1` on the game's objects, 2026-11-19).
    Spot {
        name: "level_02 edge strip, the match's link (11.20, 19.40, 37.97) -> (9.95, 16.18, 39.22)",
        project: "level_02/day_2",
        take_off: Vec3::new(11.20, 19.40, 37.97),
        landing: Vec3::new(9.95, 16.18, 39.22),
        take_off_m: 0.3,
        landing_m: 0.3,
        start_m: 0.5,
    },
];

/// What one run down the drop came to.
#[derive(Debug, Default)]
struct Run {
    /// The clock the feet were first on the landing (0.6 m across, 0.5 m in height), ms.
    landed: Option<u32>,
    /// The lip guard's verdicts that swapped or braked the keys with the feet it judged (the bot's own, `Bot::bench_guard_at`) on the
    /// drop's top within 2 m of its take-off, on the line ([`body::RECKONING_M`]) of a route that takes this drop.
    braked: u32,
    /// The same verdicts with those feet off that line: reported, not asserted (a body the stand-in put back on a lip node while
    /// the bot's reckoning of it falls beside the line, its world fix a second apart).
    off_line: u32,
    /// The body's falls of a lip's height or more ([`mapdata::grid::LIP_DROP_M`]) that came to rest off the drop's landing
    /// ([`body::PLANNED_LANDING_M`]): reported, compared before and after a change of the guard.
    unplanned: u32,
    stuck: usize,
}

fn run(world: &Arc<World>, start: Vec3, take_off: Vec3, landing: Vec3, until: u32) -> Run {
    let tuning = Tuning::default();
    let teams = [0u8, 1];
    let mut bots = Bots::new(0x1005, survarium::v026g::maps::RuleMode::TeamDeathmatch, 0, &teams, &[true, false], &[None, None], Arc::new(tuning), Trace::none(0));
    bots.map_ready(world.clone());
    let mut game = Game::new(world.clone(), &teams, 3, None, None);
    game.bodies[0].node = world.grid.locate(start, 1.0);
    game.bodies[0].feet = start;
    game.bodies[1].alive = false;
    bots.entered(0, true, 0);
    let (mut clock, mut next_world) = (0, 1_000);
    let mut out = Run::default();
    // The height the body last rested or walked at (a tick that moved it under 1 cm up or down).
    let (mut rest_y, mut last_y) = (start.y, start.y);
    while clock < until {
        clock += 25;
        while game.tick + TICK_MS <= clock {
            game.step();
        }
        let mut notes = Vec::new();
        if clock >= next_world {
            next_world += 1_000;
            if let Some((_, fixes, health, facts)) = game.world_at(clock / TICK_MS * TICK_MS - 300) {
                for (place, (fix, health)) in fixes.into_iter().zip(health).enumerate() {
                    notes.extend(bots.fixed(place, fix, Some(health)));
                }
                bots.facts(facts);
            }
        }
        let clock_now = Clock { clock, time_left_s: 600.0, total_s: 900.0, playing: true };
        let view = bots.tracker.view(&clock_now, &bots.match_wide_tuning().kin, Some(world));
        bots.features.searching.new_poll(
            bots.match_wide_tuning().think.search_queue,
            bots.match_wide_tuning().think.search_floors_per_poll as usize,
            crate::v026g::bots::search::living(&bots.bots, &view),
        );
        bots.go_on_searching();
        if let Some(bot) = bots.bots[0].as_mut() {
            bot.intent = Intent { go: Go::To { at: landing, within_m: 0.0 }, pace: Pace::Run, fire: false, exact_arrive_m: Some(0.3), ..Intent::stay() };
        }
        let mut made = Vec::new();
        bots.ticks(0, &clock_now, &view, &mut made);
        for made in made {
            let r = made.input.rotation;
            let turn = (half_value(u16::from_le_bytes([r[0], r[1]])), half_value(u16::from_le_bytes([r[2], r[3]])));
            let gun = Arms::fallback().primary.0;
            game.inputs[made.place].push_back((made.time, made.input.actions, turn, None, gun));
        }
        out.stuck += notes.iter().filter(|n| n.text.contains("is stuck at")).count();
        // The guard's verdict of this tick, judged on the bot's own feet (its reckoning, not the stand-in's body), taken once: the
        // bot drives every other tick and an unstick replay skips the guard, so it is cleared once read.
        if let Some(bot) = bots.bots[0].as_mut() {
            let (guarded, judged) = (matches!(bot.bench_guard, 2 | 3), bot.bench_guard_at.1);
            bot.bench_guard = 0;
            if guarded && judged.distance_xz(take_off) <= 2.0 && (judged.y - take_off.y).abs() <= 0.6 {
                // `progress` moves the route's cursor: asked only here, of the feet the bot itself asked it of this tick.
                let on_the_drop = bot.route.as_mut().is_some_and(|(route, _)| {
                    route.points().windows(2).any(|w| w[0].at == take_off && w[1].at == landing) && route.progress(judged).1 <= body::RECKONING_M
                });
                if on_the_drop {
                    out.braked += 1;
                } else {
                    out.off_line += 1;
                }
            }
        }
        let feet = game.bodies[0].feet;
        if (feet.y - last_y).abs() < 0.01 {
            if rest_y - feet.y >= mapdata::grid::LIP_DROP_M && feet.distance_xz(landing) > body::PLANNED_LANDING_M {
                out.unplanned += 1;
            }
            rest_y = feet.y;
        }
        last_y = feet.y;
        if feet.distance_xz(landing) <= 0.6 && (feet.y - landing.y).abs() <= 0.5 && game.bodies[0].alive {
            out.landed = Some(clock);
            break;
        }
    }
    out
}

#[test]
#[ignore = "needs SRP_MAPDATA_DIR"]
fn drop_spots() {
    let Some(dir) = bench_settings().var_os("SRP_MAPDATA_DIR").map(std::path::PathBuf::from) else {
        eprintln!("skipping: SRP_MAPDATA_DIR is not set");
        return;
    };
    let t = Tuning::default();
    for spot in &SPOTS {
        let bytes = std::fs::read(dir.join(mapdata::format::file_name(spot.project))).unwrap();
        let map_file = {
            let mut digest = mapdata::digest::Digest::new();
            digest.bytes(&bytes);
            digest.finish()
        };
        let map = mapdata::MapData::from_bytes(&bytes).unwrap();
        let extras = mapdata::GridExtras {
            wall_clearance: t.needs_wall_clearance(),
            open_sides: t.needs_open_sides(),
            pull_hold: t.grid.wall_clear_pull.then_some(crate::v026g::bots::tuning::PULL_HOLD_M),
            pull_walk: t.grid.pull_walk,
            walker_shapes: t.grid.walker_shapes,
        };
        let world = Arc::new(super::grid_cache::world(map, map_file, mapdata::CELL, t.grid_body(), t.off_mesh_links(), extras));
        let grid = &world.grid;
        let Some(to) = grid.locate(spot.landing, 1.0) else {
            panic!("{}: no floor at the landing on this grid", spot.name);
        };
        // The route's drop of the spot: a leg reached by no jump going a lip's height or more down, from near the take-off to
        // near the landing.
        let spots_drop = |route: &path::Route| {
            route.points().windows(2).find_map(|w| {
                let (p, q) = (w[0].at, w[1].at);
                (w[1].jump.is_none()
                    && p.y - q.y >= mapdata::grid::LIP_DROP_M
                    && p.distance_xz(spot.take_off) <= spot.take_off_m
                    && (p.y - spot.take_off.y).abs() <= 0.3
                    && q.distance_xz(spot.landing) <= spot.landing_m
                    && (q.y - spot.landing.y).abs() <= 0.5)
                    .then_some((p, q))
            })
        };
        // Starts on the drop's top `start_m`-8 m off whose planned route takes a drop of the spot: up to 3, spread round it.
        let a = spot.take_off;
        let mut starts: Vec<(Vec3, Vec3, Vec3)> = Vec::new();
        for n in grid.floors_in([a.x - 8.0, a.z - 8.0, a.x + 8.0, a.z + 8.0]) {
            let p = grid.position(n);
            let d = p.distance_xz(a);
            if !(spot.start_m..=8.0).contains(&d) || (p.y - a.y).abs() > 0.5 || starts.iter().any(|s| s.0.distance_xz(p) < 2.5) {
                continue;
            }
            let Some(route) = path::plan(grid, n, to) else { continue };
            if let Some((take_off, landing)) = spots_drop(&route) {
                starts.push((p, take_off, landing));
            }
            if starts.len() >= 3 {
                break;
            }
        }
        assert!(!starts.is_empty(), "{}: no start whose route takes a drop of the spot", spot.name);
        for (start, take_off, landing) in starts {
            let r = run(&world, start, take_off, landing, 20_000);
            eprintln!("DROPS {}: from {start:?} ({:.1} m) down {take_off:?} -> {landing:?}: {r:?}", spot.name, start.distance_xz(take_off));
            assert_eq!(r.braked, 0, "{}: from {start:?} the lip guard braked the route's own walk-off: {r:?}", spot.name);
        }
    }
}

/// [drop give-ups] The other way of the strip's walk-offs: a lip the exe keeps a body on stays ground in the stand-in (a check on
/// any change of the stand-in's walk off a top; a landing rule that let the strip's turn down did let these fall). A body walks straight along the level_02
/// edge strip's rim (+z, forward held, yaw 0) from z 37.10 for 24 ticks at x 11.20 .. 10.95; the exe keeps it on ground every tick at
/// every x (`strip_rim_exe_v1` on the game's objects, 2026-11-19: 0 air ticks, lowest y 19.28 .. 18.93, the rim sloping down), and
/// so must the stand-in: never down to the 16.4 m floor under the strip.
#[test]
#[ignore = "needs SRP_MAPDATA_DIR"]
fn strip_rim_walks_stay_on_the_top() {
    let Some(dir) = bench_settings().var_os("SRP_MAPDATA_DIR").map(std::path::PathBuf::from) else {
        eprintln!("skipping: SRP_MAPDATA_DIR is not set");
        return;
    };
    let t = Tuning::default();
    let project = "level_02/day_2";
    let bytes = std::fs::read(dir.join(mapdata::format::file_name(project))).unwrap();
    let map_file = {
        let mut digest = mapdata::digest::Digest::new();
        digest.bytes(&bytes);
        digest.finish()
    };
    let map = mapdata::MapData::from_bytes(&bytes).unwrap();
    let extras = mapdata::GridExtras {
        wall_clearance: t.needs_wall_clearance(),
        open_sides: t.needs_open_sides(),
        pull_hold: t.grid.wall_clear_pull.then_some(crate::v026g::bots::tuning::PULL_HOLD_M),
        pull_walk: t.grid.pull_walk,
        walker_shapes: t.grid.walker_shapes,
    };
    let world = Arc::new(super::grid_cache::world(map, map_file, mapdata::CELL, t.grid_body(), t.off_mesh_links(), extras));
    let gun = Arms::fallback().primary.0;
    let mut fell = Vec::new();
    for x in [11.20f32, 11.15, 11.10, 11.05, 11.00, 10.95] {
        let start = Vec3::new(x, 19.40, 37.10);
        let mut game = Game::new(world.clone(), &[0u8, 1], 3, None, None);
        game.bodies[0].node = world.grid.locate(start, 1.0);
        game.bodies[0].feet = start;
        // Facing +z (`kin::forward`); the inputs' turn is a change of it.
        game.bodies[0].yaw = 0.0;
        game.bodies[1].alive = false;
        let mut lowest = start.y;
        for _ in 0..24 {
            let at = game.tick + TICK_MS;
            game.inputs[0].push_back((at, key::FORWARD, (0.0, 0.0), None, gun));
            game.step();
            lowest = lowest.min(game.bodies[0].feet.y);
        }
        let end = game.bodies[0].feet;
        eprintln!("RIM x {x:.2}: lowest y {lowest:.2}, end ({:.2}, {:.2}, {:.2})", end.x, end.y, end.z);
        if lowest < 18.0 {
            fell.push(x);
        }
    }
    assert!(fell.is_empty(), "the stand-in let bodies fall off the rim the exe keeps them on, at x {fell:?}");
}
