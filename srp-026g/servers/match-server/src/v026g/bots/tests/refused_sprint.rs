//! [refusal reach] A sprint link the client refuses, at a real spot: School's up sprint gap (19.70, 16.08, 64.47) ->
//! (20.70, 17.32, 64.72). A bot whose sprint is refused there (A386 `Walked::JumpRefused`) plans again with no sprint
//! (`drive::body_reach`): no link that needs one, so not the plain 1.25 m jump 2 m aside (17.70, 16.08, 64.47) ->
//! (17.20, 17.32, 64.97) either, and walks round to the top. On the stand-in over the real mapdata, the client's own steps.
//!
//! ```text
//! SRP_MAPDATA_DIR=<dir> [SRP_BENCH_GRID_CACHE=<dir>] cargo test -p match-server --release refused_sprint -- --ignored --nocapture
//! ```

use super::*;

use crate::v026g::bots::path;
use crate::v026g::bots::plays::{Go, Intent, Pace};
use crate::v026g::bots::view::Clock;

const PROJECT: &str = "level_02/day_2";
const TAKE_OFF: Vec3 = Vec3::new(19.70, 16.08, 64.47);
/// The sprint link's landing, the bot's goal.
const LANDING: Vec3 = Vec3::new(20.70, 17.32, 64.72);
/// The plain jump 2 m aside, which needs the sprint as well (1.25 m over the standing jump's 0.998).
const SPRINT_JUMP_FROM: Vec3 = Vec3::new(17.70, 16.08, 64.47);
/// The time over the no-sprint route's walk the bot may take (its turns, the refusal itself), s.
const MARGIN_S: f32 = 4.0;

/// The off-mesh link of `grid` from the floor at `take_off` to the one at `landing` (within 0.3 m flat, 0.3 m in height).
fn link_at(grid: &mapdata::grid::WalkGrid, take_off: Vec3, landing: Vec3) -> Option<mapdata::grid::gap::OffLink> {
    let at = |n: NodeId, p: Vec3| {
        let q = grid.position(n);
        q.distance_xz(p) <= 0.3 && (q.y - p.y).abs() <= 0.3
    };
    grid.off_links().iter().copied().find(|l| at(l.from, take_off) && at(l.to, landing))
}

/// What a run to the goal came to.
#[derive(Debug)]
struct Run {
    /// The clock the feet were first on the goal (0.6 m across, 0.5 m in height), ms.
    arrived: Option<u32>,
    /// Ticks the bot sent the sprint key.
    sprint_keys: u32,
    /// The ms the body was at rest (under 0.3 m/s) within 1 m of the plain jump's take-off.
    rest_at_sprint_jump_ms: u32,
}

/// A bot sent from `start` to `goal` with its sprint refused all the way: a hit kept within the stagger's window
/// (`drive::sprint_block` `SprintBlock::Staggered`, the client's `can_sprint`), which leaves its jumps and its walk as they are.
fn run(world: &Arc<World>, start: Vec3, goal: Vec3, until: u32) -> Run {
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
    let mut out = Run { arrived: None, sprint_keys: 0, rest_at_sprint_jump_ms: 0 };
    let mut last = start;
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
            bot.hurt.last_hit_ms = Some(clock);
        }
        let mut made = Vec::new();
        bots.ticks(0, &clock_now, &view, &mut made);
        for made in made {
            out.sprint_keys += u32::from(made.input.actions & key::SPRINT != 0);
            let r = made.input.rotation;
            let turn = (half_value(u16::from_le_bytes([r[0], r[1]])), half_value(u16::from_le_bytes([r[2], r[3]])));
            let gun = Arms::fallback().primary.0;
            game.inputs[made.place].push_back((made.time, made.input.actions, turn, None, gun));
        }
        let feet = game.bodies[0].feet;
        if clock % TICK_MS == 0 {
            let speed = feet.distance_xz(last) / (TICK_MS as f32 / 1000.0);
            last = feet;
            if feet.distance_xz(SPRINT_JUMP_FROM) <= 1.0 && (feet.y - SPRINT_JUMP_FROM.y).abs() <= 0.5 && speed < 0.3 {
                out.rest_at_sprint_jump_ms += TICK_MS;
            }
        }
        // The bot's own arrival (`exact_arrive_m`) is on its goal's floor; the feet within 0.6 m of the landing count.
        if feet.distance_xz(goal) <= 0.6 && (feet.y - goal.y).abs() <= 0.5 && game.bodies[0].alive {
            out.arrived = Some(clock);
            break;
        }
    }
    out
}

/// School's refused sprint gap: from starts 3-8 m off on the take-off's level whose planned route takes the sprint link, a
/// bot with no sprint sends no sprint key, does not stand at the plain sprint jump aside, and reaches the sprint link's
/// landing within the walk of its no-sprint way (to the take-off, then the no-sprint route from there) and [`MARGIN_S`].
#[test]
#[ignore = "a per-spot test on the real mapdata; needs SRP_MAPDATA_DIR"]
fn a_bot_refused_the_sprint_at_schools_gap_walks_round_with_no_sprint_jump() {
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
    let sprint = link_at(grid, TAKE_OFF, LANDING).expect("the sprint link");
    assert!(matches!(sprint.press, mapdata::grid::gap::Takeoff::Sprint(_)), "{:?}", sprint.press);
    let (a, b) = (grid.position(sprint.from), grid.position(sprint.to));
    // The no-sprint way from the take-off, as the bot plans it after the refusal (a carrier's reach).
    let no_sprint = mapdata::Reach::no_sprint(t.feel.carrier_jump_m.map(f32::from));
    let walk_round = path::plan_where_reaching(grid, sprint.from, sprint.to, |_| true, no_sprint).expect("a no-sprint way up");
    eprintln!("REFUSED sprint link {:?} walk {:?}; no-sprint way {:.2} m", sprint.press, sprint.walk, walk_round.length());
    let mut starts: Vec<Vec3> = Vec::new();
    for n in grid.floors_in([a.x - 8.0, a.z - 8.0, a.x + 8.0, a.z + 8.0]).step_by(3) {
        let p = grid.position(n);
        let d = p.distance_xz(a);
        if !(3.0..=8.0).contains(&d) || (p.y - a.y).abs() > 0.5 || starts.iter().any(|s| s.distance_xz(p) < 2.5) {
            continue;
        }
        let Some(route) = path::plan(grid, n, sprint.to) else { continue };
        let pts = route.points();
        if pts.windows(2).any(|w| w[1].jump.is_some() && w[0].at.distance_xz(a) < 0.3 && w[1].at.distance_xz(b) < 0.3) {
            starts.push(p);
        }
        if starts.len() >= 3 {
            break;
        }
    }
    assert!(!starts.is_empty(), "no start within 3-8 m whose route takes the sprint link");
    let runs: Vec<(Vec3, Run)> = starts.into_iter().map(|start| (start, run(&world, start, b, 40_000))).collect();
    for (start, r) in &runs {
        let to_take_off = path::plan(grid, grid.locate(*start, 1.0).unwrap(), sprint.from).map_or(f32::NAN, |r| r.length());
        let allowed_ms = ((to_take_off + walk_round.length()) / mapdata::grid::controlled::WALK_MPS + MARGIN_S) * 1000.0;
        eprintln!("REFUSED from {start:?} ({:.1} m): {r:?}, allowed {allowed_ms:.0} ms", start.distance_xz(a));
        assert_eq!(r.sprint_keys, 0, "from {start:?}: the sprint is refused all the way");
        assert!(r.rest_at_sprint_jump_ms <= 500, "from {start:?}: stood {} ms at the plain sprint jump", r.rest_at_sprint_jump_ms);
        let arrived = r.arrived.unwrap_or_else(|| panic!("from {start:?}: never reached the landing {b:?}"));
        assert!(arrived as f32 <= allowed_ms, "from {start:?}: reached the landing at {arrived} ms, over {allowed_ms:.0} ms");
    }
}
