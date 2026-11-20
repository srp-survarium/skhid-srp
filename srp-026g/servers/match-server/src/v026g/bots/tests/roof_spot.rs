//! [marked walk] School day_2's roof: the crouched walk (36.70, 21.48, 113.97) -> (36.45, 21.76, 113.97) under the beam is one the
//! client's tick does not walk (the walk check fails it on all three lines; the exe's crouched body stalls at x 36.68), kept and
//! marked as the west roof's only walking way. Bots planned their way west over it and stalled. Priced as a link no press makes,
//! a bot on the east roof sent to the west roof goes the way players go: the standing press onto the beam (36.45, 22.45, 112.97)
//! and the walk down. On the stand-in over the real mapdata, the client's own steps, from three starts on the east roof (the
//! first the start of the match's given-up leg).
//!
//! ```text
//! SRP_MAPDATA_DIR=<dir> SRP_GAME_DATA=<server-game-data.db> [SRP_BENCH_GRID_CACHE=<dir>] cargo test -p match-server --release roof_spot -- --ignored --nocapture
//! ```

use super::*;

use crate::v026g::bots::path;
use crate::v026g::bots::plays::{Go, Intent, Pace};
use crate::v026g::bots::view::Clock;

const PROJECT: &str = "level_02/day_2";
/// The crouched walk the client does not walk: its foot and its top.
const RISER: (Vec3, Vec3) = (Vec3::new(36.70, 21.48, 113.97), Vec3::new(36.45, 21.76, 113.97));
/// The west roof's floor the bots' drop leaves from (its Drop to (35.45, 17.02, 114.22)).
const WEST: Vec3 = Vec3::new(35.95, 21.76, 113.72);
/// The starts on the east roof: the match's given-up leg's start (`DIAG drop given up`, take-off (37.13, 21.23, 112.92)), and
/// two floors beside the riser's foot.
const STARTS: [Vec3; 3] = [Vec3::new(37.13, 21.23, 112.92), Vec3::new(36.95, 21.61, 113.97), Vec3::new(37.20, 21.58, 113.72)];
/// The time over the route's walk the bot may take (its turns, the press and its run-up), s.
const MARGIN_S: f32 = 10.0;
/// How near the goal across, and in height, the feet count as there, m.
const THERE_M: (f32, f32) = (1.2, 0.3);
/// The server's game data, loaded once from `SRP_GAME_DATA` (required: the grid body's fastest walk cap comes from the bots'
/// kits in it, so a grid built without it is not the server's).
fn game_data() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        assert!(bench_settings().var_os("SRP_GAME_DATA").is_some_and(|p| !p.is_empty()), "set SRP_GAME_DATA to the server's file");
        let data = survarium::v026g::game_data::init().unwrap_or_else(|cause| panic!("SRP_GAME_DATA: {cause}"));
        eprintln!("game data: {}", data.origin);
    });
}

/// Under this height the body is off the roof (the roof's floors are 21.2-22.9 m; the ground beside 20.6 m and lower).
const OFF_ROOF_Y: f32 = 21.0;

/// How a bot sent from `start` to `goal` did: the clock it was there (None within `until` ms), its lowest feet before, its
/// presses, and the nearest its feet came to the riser's top across.
fn run(world: &Arc<World>, start: Vec3, goal: Vec3, until: u32) -> (Option<u32>, f32, u32, f32) {
    let tuning = Tuning::default();
    let teams = [0u8, 1];
    let mut bots = Bots::new(0x1005, survarium::v026g::maps::RuleMode::TeamDeathmatch, 0, &teams, &[true, false], &[None, None], Arc::new(tuning), Trace::none(0));
    bots.map_ready(world.clone());
    let mut game = Game::new(world.clone(), &teams, 3, None, None);
    game.bodies[0].node = world.grid.locate(start, 1.0);
    game.bodies[0].feet = start;
    game.bodies[1].alive = false;
    bots.entered(0, true, 0);
    let (mut clock, mut next_world, mut presses, mut lowest, mut nearest) = (0, 1_000, 0, f32::MAX, f32::MAX);
    while clock < until {
        clock += 25;
        while game.tick + TICK_MS <= clock {
            game.step();
        }
        if clock >= next_world {
            next_world += 1_000;
            if let Some((_, fixes, health, facts)) = game.world_at(clock / TICK_MS * TICK_MS - 300) {
                for (place, (fix, health)) in fixes.into_iter().zip(health).enumerate() {
                    let _ = bots.fixed(place, fix, Some(health));
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
            bot.intent = Intent { go: Go::To { at: goal, within_m: 0.0 }, pace: Pace::Run, fire: false, exact_arrive_m: Some(0.3), ..Intent::stay() };
        }
        let mut made = Vec::new();
        bots.ticks(0, &clock_now, &view, &mut made);
        for made in made {
            presses += u32::from(made.input.actions & key::JUMP != 0);
            let r = made.input.rotation;
            let turn = (half_value(u16::from_le_bytes([r[0], r[1]])), half_value(u16::from_le_bytes([r[2], r[3]])));
            let gun = Arms::fallback().primary.0;
            game.inputs[made.place].push_back((made.time, made.input.actions, turn, None, gun));
        }
        let feet = game.bodies[0].feet;
        lowest = lowest.min(feet.y);
        nearest = nearest.min(feet.distance_xz(RISER.1));
        if clock % 1_000 == 0 {
            eprintln!("ROOFSPOT t {clock} feet ({:.2}, {:.2}, {:.2})", feet.x, feet.y, feet.z);
        }
        if feet.distance_xz(goal) <= THERE_M.0 && (feet.y - goal.y).abs() <= THERE_M.1 && game.bodies[0].alive {
            return (Some(clock), lowest, presses, nearest);
        }
    }
    (None, lowest, presses, nearest)
}

/// The crouched walk under the beam is marked, the planned way west does not take it and presses (onto the beam), and a bot
/// from each start reaches the west roof within the route's walk and [`MARGIN_S`], never under [`OFF_ROOF_Y`] on the way.
#[test]
#[ignore = "a per-spot test on the real mapdata; needs SRP_MAPDATA_DIR"]
fn a_bot_on_schools_east_roof_goes_west_over_the_beam_not_under_it() {
    let Some(dir) = bench_settings().var_os("SRP_MAPDATA_DIR").map(std::path::PathBuf::from) else {
        eprintln!("skipping: SRP_MAPDATA_DIR is not set");
        return;
    };
    game_data();
    let t = Tuning::default();
    let bytes = std::fs::read(dir.join(mapdata::format::file_name(PROJECT))).unwrap();
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
    let floor = |p: Vec3| {
        grid.floors_in([p.x - 0.1, p.z - 0.1, p.x + 0.1, p.z + 0.1]).find(|&f| (grid.position(f).y - p.y).abs() < 0.05).expect("the floor")
    };
    let (foot, top) = (floor(RISER.0), floor(RISER.1));
    assert!(grid.walk_marked(foot, top), "the crouched walk under the beam is marked");
    let west = floor(WEST);
    for (i, &start) in STARTS.iter().enumerate() {
        let from = grid.locate(start, 0.5).expect("the start's floor");
        let route = path::plan(grid, from, west).expect("a way west");
        let pts = route.points();
        eprintln!("ROOFSPOT {i} route {:.1} m: {:?}", route.length(), pts.iter().map(|p| (p.at, p.jump.is_some())).collect::<Vec<_>>());
        let under = pts.windows(2).any(|w| w[0].at.distance_xz(RISER.0) < 0.1 && w[1].at.distance_xz(RISER.1) < 0.1);
        assert!(!under, "{i}: the route walks under the beam");
        assert!(pts.iter().any(|p| p.jump.is_some()), "{i}: the route presses (onto the beam)");
        let allowed_ms = (route.length() / mapdata::grid::controlled::WALK_MPS + MARGIN_S) * 1000.0;
        let (there, lowest, presses, nearest) = run(&world, grid.position(from), grid.position(west), allowed_ms as u32 + 20_000);
        eprintln!("ROOFSPOT {i} there {there:?} ms, lowest {lowest:.2}, presses {presses}, nearest the riser's top {nearest:.2} m, allowed {allowed_ms:.0} ms");
        let there = there.unwrap_or_else(|| panic!("{i}: never reached the west roof"));
        assert!(there as f32 <= allowed_ms, "{i}: reached the west roof at {there} ms, over {allowed_ms:.0} ms");
        assert!(lowest >= OFF_ROOF_Y, "{i}: off the roof on the way ({lowest:.2})");
    }
}
