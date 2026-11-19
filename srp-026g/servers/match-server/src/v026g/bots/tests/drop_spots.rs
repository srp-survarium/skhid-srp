//! [drop give-ups] The planned walk-offs the lip guard braked (fix-builder's diagnostic match of 2026-11-19, level_02 day 2:
//! 7 of 20 drops given up; fix-team's roof take-off, 51 bot-seconds there in 4 runs and no drop): a bot sent down a drop of the
//! spot from starts on its top whose planned route takes one (the edge strip's walk-offs lie side by side, a route takes the one it
//! pulls straight to), on the stand-in over the real mapdata. Per start it must come down on its route's landing, and the guard
//! must not brake it on the drop's top. A regression test on the real map data, ignored without it.
//!
//! ```text
//! SRP_MAPDATA_DIR=<dir> [SRP_BENCH_GRID_CACHE=<dir>] cargo test -p match-server --release drop_spots -- --ignored --nocapture
//! ```

use super::*;

use crate::v026g::bots::path;
use crate::v026g::bots::plays::{Go, Intent, Pace};
use crate::v026g::bots::view::Clock;

/// A spot: its project, a take-off and landing of it (map frame), and its name in the report. A route's drop is the spot's when it
/// leaves within [`SPOT_TAKE_OFF_M`] of the take-off for a floor within [`SPOT_LANDING_M`] of the landing.
struct Spot {
    name: &'static str,
    project: &'static str,
    take_off: Vec3,
    landing: Vec3,
}

const SPOTS: [Spot; 2] = [
    Spot {
        name: "level_02 edge strip walk-off 3.2 m down, 1.77 m across",
        project: "level_02/day_2",
        take_off: Vec3::new(11.20, 19.40, 37.97),
        landing: Vec3::new(9.95, 16.18, 39.22),
    },
    Spot {
        name: "level_02 roof take-off on a 0.3 m top, 4.7 m down",
        project: "level_02/day_2",
        take_off: Vec3::new(36.49, 21.76, 113.97),
        landing: Vec3::new(35.45, 17.02, 114.22),
    },
];

/// How near the spot's take-off a route's drop leaves to be the spot's, m: the strip's walk-offs side by side 0.25 m apart.
const SPOT_TAKE_OFF_M: f32 = 1.3;
/// How near the spot's landing its landing lies, m.
const SPOT_LANDING_M: f32 = 1.6;

/// What one run down the drop came to.
#[derive(Debug, Default)]
struct Run {
    /// The clock the feet were first on the landing (0.6 m across, 0.5 m in height), ms.
    landed: Option<u32>,
    /// The ticks the lip guard swapped or braked the keys with the feet on the drop's top within 2 m of its take-off.
    braked: u32,
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
        let feet = game.bodies[0].feet;
        if let Some(bot) = bots.bots[0].as_ref()
            && matches!(bot.bench_guard, 2 | 3)
            && feet.distance_xz(take_off) <= 2.0
            && (feet.y - take_off.y).abs() <= 0.6
        {
            out.braked += 1;
        }
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
                    && p.distance_xz(spot.take_off) <= SPOT_TAKE_OFF_M
                    && (p.y - spot.take_off.y).abs() <= 0.3
                    && q.distance_xz(spot.landing) <= SPOT_LANDING_M
                    && (q.y - spot.landing.y).abs() <= 0.5)
                    .then_some((p, q))
            })
        };
        // Starts on the drop's top 3-8 m off whose planned route takes a drop of the spot: up to 3, spread round it.
        let a = spot.take_off;
        let mut starts: Vec<(Vec3, Vec3, Vec3)> = Vec::new();
        for n in grid.floors_in([a.x - 8.0, a.z - 8.0, a.x + 8.0, a.z + 8.0]) {
            let p = grid.position(n);
            let d = p.distance_xz(a);
            if !(3.0..=8.0).contains(&d) || (p.y - a.y).abs() > 0.5 || starts.iter().any(|s| s.0.distance_xz(p) < 2.5) {
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
            assert!(r.landed.is_some(), "{}: from {start:?} the bot never came down on the landing: {r:?}", spot.name);
            assert_eq!(r.braked, 0, "{}: from {start:?} the lip guard braked the route's own walk-off: {r:?}", spot.name);
        }
    }
}
