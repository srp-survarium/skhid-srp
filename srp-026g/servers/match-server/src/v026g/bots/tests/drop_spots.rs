//! [drop give-ups] The planned walk-offs the lip guard braked (fix-builder's diagnostic match of 2026-11-19, level_02 day 2:
//! 7 of 20 drops given up): a bot sent down the drop from starts on its top whose planned route takes it, on the stand-in over the
//! real mapdata. Per start it must come down on the drop's landing, the guard must not brake it on the drop's top, and its
//! route must not end there. A regression test on the real map data, ignored without it.
//!
//! ```text
//! SRP_MAPDATA_DIR=<dir> [SRP_BENCH_GRID_CACHE=<dir>] cargo test -p match-server --release drop_spots -- --ignored --nocapture
//! ```

use super::*;

use crate::v026g::bots::path;
use crate::v026g::bots::plays::{Go, Intent, Pace};
use crate::v026g::bots::view::Clock;

/// A drop: its project, the take-off and landing (map frame), and its name in the report.
struct Spot {
    name: &'static str,
    project: &'static str,
    take_off: Vec3,
    landing: Vec3,
}

const SPOTS: [Spot; 1] = [Spot {
    name: "level_02 walk-off 3.2 m down, 1.77 m across",
    project: "level_02/day_2",
    take_off: Vec3::new(11.20, 19.40, 37.97),
    landing: Vec3::new(9.95, 16.18, 39.22),
}];

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
        let near = |n: NodeId, p: Vec3| grid.position(n).distance_xz(p) <= 0.15 && (grid.position(n).y - p.y).abs() <= 0.2;
        let (Some(from), Some(to)) = (grid.locate(spot.take_off, 0.5).filter(|&n| near(n, spot.take_off)), grid.locate(spot.landing, 0.5).filter(|&n| near(n, spot.landing)))
        else {
            panic!("{}: no floors at the drop's ends on this grid", spot.name);
        };
        let (a, b) = (grid.position(from), grid.position(to));
        // Starts on the drop's top 3-8 m off whose planned route takes this drop: up to 3, spread round it.
        let mut starts: Vec<Vec3> = Vec::new();
        for n in grid.floors_in([a.x - 8.0, a.z - 8.0, a.x + 8.0, a.z + 8.0]).step_by(3) {
            let p = grid.position(n);
            let d = p.distance_xz(a);
            if !(3.0..=8.0).contains(&d) || (p.y - a.y).abs() > 0.5 || starts.iter().any(|s| s.distance_xz(p) < 2.5) {
                continue;
            }
            let Some(route) = path::plan(grid, n, to) else { continue };
            if route.points().windows(2).any(|w| w[0].at.distance_xz(a) < 0.05 && w[1].at.distance_xz(b) < 0.05) {
                starts.push(p);
            }
            if starts.len() >= 3 {
                break;
            }
        }
        assert!(!starts.is_empty(), "{}: no start whose route takes the drop", spot.name);
        for start in starts {
            let r = run(&world, start, a, b, 20_000);
            eprintln!("DROPS {}: from {start:?} ({:.1} m): {r:?}", spot.name, start.distance_xz(a));
            assert!(r.landed.is_some(), "{}: from {start:?} the bot never came down on the landing: {r:?}", spot.name);
            assert_eq!(r.braked, 0, "{}: from {start:?} the lip guard braked the route's own walk-off: {r:?}", spot.name);
        }
    }
}
