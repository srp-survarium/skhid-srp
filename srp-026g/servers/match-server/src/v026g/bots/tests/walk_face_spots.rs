//! [walk-face chain] Vector Laboratory day_2's box top beside the walker pillar (#2255): the grid made a gap press up the
//! pillar's side, (8.01, 11.59, -17.04) -> (8.01, 32.16, -16.04), 20.6 m, which no client jump makes; a bot waited at it. With
//! the walk-face chain the link is gone: a bot sent from the box top to the pillar's top plans a way the client makes (no link
//! rises over the sprint jump's reach) and reaches it. On the stand-in over the real mapdata, the client's own steps.
//!
//! ```text
//! SRP_MAPDATA_DIR=<dir> [SRP_BENCH_GRID_CACHE=<dir>] cargo test -p match-server --release walk_face_spots -- --ignored --nocapture
//! ```

use super::*;

use crate::v026g::bots::path;
use crate::v026g::bots::plays::{Go, Intent, Pace};
use crate::v026g::bots::view::Clock;

const PROJECT: &str = "level_07/day_2";
const BOX_TOP: Vec3 = Vec3::new(8.01, 11.59, -17.04);
const PILLAR_TOP: Vec3 = Vec3::new(8.01, 32.16, -16.04);
/// The most a route's link may rise: over it no client jump goes (the sprint climb's 1.15 m with a jump's reach, well under).
const RISE_MAX_M: f32 = 3.0;
/// The time over the route's walk the bot may take (its turns, presses and their run-ups), s.
const MARGIN_S: f32 = 10.0;
/// How near the goal across, and in height, the feet count as there, m.
const THERE_M: (f32, f32) = (1.2, 0.3);

/// A bot sent from `start` to `goal`: the clock it was there, or none within `until` ms; and its jump presses.
fn run(world: &Arc<World>, start: Vec3, goal: Vec3, until: u32) -> (Option<u32>, u32) {
    let tuning = Tuning::default();
    let teams = [0u8, 1];
    let mut bots = Bots::new(0x1005, survarium::v026g::maps::RuleMode::TeamDeathmatch, 0, &teams, &[true, false], &[None, None], Arc::new(tuning), Trace::none(0));
    bots.map_ready(world.clone());
    let mut game = Game::new(world.clone(), &teams, 3, None, None);
    game.bodies[0].node = world.grid.locate(start, 1.0);
    game.bodies[0].feet = start;
    game.bodies[1].alive = false;
    bots.entered(0, true, 0);
    let (mut clock, mut next_world, mut presses) = (0, 1_000, 0);
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
        if clock % 2_000 == 0 {
            eprintln!("WALKFACE t {clock} feet ({:.2}, {:.2}, {:.2})", feet.x, feet.y, feet.z);
        }
        if feet.distance_xz(goal) <= THERE_M.0 && (feet.y - goal.y).abs() <= THERE_M.1 && game.bodies[0].alive {
            return (Some(clock), presses);
        }
    }
    (None, presses)
}

/// The box top's way to the pillar's top: no link of the grid from the box top's floors rises over [`RISE_MAX_M`], the
/// planned route has none, and a bot reaches the pillar's top within the route's walk and [`MARGIN_S`].
#[test]
#[ignore = "a per-spot test on the real mapdata; needs SRP_MAPDATA_DIR"]
fn a_bot_on_vector_laboratorys_box_top_reaches_the_pillar_top_with_no_link_up_its_side() {
    let Some(dir) = bench_settings().var_os("SRP_MAPDATA_DIR").map(std::path::PathBuf::from) else {
        eprintln!("skipping: SRP_MAPDATA_DIR is not set");
        return;
    };
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
    let from = grid.locate(BOX_TOP, 0.5).expect("the box top's floor");
    let to = grid.locate(PILLAR_TOP, 0.5).expect("the pillar top's floor");
    let near_box: Vec<NodeId> = grid.floors_in([BOX_TOP.x - 1.5, BOX_TOP.z - 1.5, BOX_TOP.x + 1.5, BOX_TOP.z + 1.5]).filter(|&n| (grid.position(n).y - BOX_TOP.y).abs() <= 0.5).collect();
    let high: Vec<_> = grid.off_links().iter().filter(|l| near_box.contains(&l.from) && grid.position(l.to).y - grid.position(l.from).y > RISE_MAX_M).collect();
    assert!(high.is_empty(), "links up from the box top: {high:?}");
    let route = path::plan(grid, from, to).expect("a way to the pillar's top");
    let pts = route.points();
    eprintln!("WALKFACE route {:.1} m, {} points: {:?}", route.length(), pts.len(), pts.iter().map(|p| (p.at, p.jump.is_some())).collect::<Vec<_>>());
    assert!(pts.windows(2).all(|w| w[1].at.y - w[0].at.y <= RISE_MAX_M), "a step of the route rises over {RISE_MAX_M} m");
    let allowed_ms = (route.length() / mapdata::grid::controlled::WALK_MPS + MARGIN_S) * 1000.0;
    let (there, presses) = run(&world, grid.position(from), grid.position(to), allowed_ms as u32 + 20_000);
    eprintln!("WALKFACE there {there:?} ms, presses {presses}, allowed {allowed_ms:.0} ms");
    let there = there.expect("never reached the pillar's top");
    assert!(there as f32 <= allowed_ms, "reached the pillar's top at {there} ms, over {allowed_ms:.0} ms");
}
