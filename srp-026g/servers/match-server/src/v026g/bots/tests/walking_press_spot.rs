//! [slow walk press] A sprint link's walking press played where the client refuses the sprint at the take-off, at a real spot:
//! School's up sprint gap (17.95, 15.72, 92.22) -> (18.45, 17.07, 92.72), whose walking press is a slow press (Slow(4) at 0.50
//! m, share 5 of 14: the press at its own lead from the take-off floor's own point lands in the port and in the exe on
//! rig_real_v3, and 4 mm off that point it falls back in both). The route is FORCED through the link: the bot plans its way
//! unhurt (the sprint link is its way up), and is hit from [`HIT_M`] before the take-off on, so the client gives no sprint
//! there (`drive::sprint_block` `SprintBlock::Staggered`) and the motor plays the link's walking press (A386,
//! `sprint.walk.map_or(flown, Leap::Gap)`). After a refusal the planner takes no sprint link (#2265), so a bot with no sprint
//! from the start walks round and never plays this press. On the stand-in over the real mapdata, the client's own steps.
//!
//! ```text
//! SRP_MAPDATA_DIR=<dir> [SRP_BENCH_GRID_CACHE=<dir>] cargo test -p match-server --release walking_press_spot -- --ignored --nocapture
//! ```

use super::*;

use crate::v026g::bots::path;
use crate::v026g::bots::plays::{Go, Intent, Pace};
use crate::v026g::bots::view::Clock;

const PROJECT: &str = "level_02/day_2";
const TAKE_OFF: Vec3 = Vec3::new(17.95, 15.72, 92.22);
/// The sprint link's landing, the bot's goal.
const LANDING: Vec3 = Vec3::new(18.45, 17.07, 92.72);
/// From this near the take-off across, the bot is hit every tick (the stagger's window holds).
const HIT_M: f32 = 1.5;
/// How near the landing across the feet count as on its top, m (the top's floors round the landing).
const ON_TOP_M: f32 = 1.2;
/// After the first press with no sprint key, this long for the feet to come onto the top: the press landed, ms.
const LANDS_WITHIN_MS: u32 = 2_000;

/// The off-mesh link of `grid` from the floor at `take_off` to the one at `landing` (within 0.3 m flat, 0.3 m in height).
fn link_at(grid: &mapdata::grid::WalkGrid, take_off: Vec3, landing: Vec3) -> Option<mapdata::grid::gap::OffLink> {
    let at = |n: NodeId, p: Vec3| {
        let q = grid.position(n);
        q.distance_xz(p) <= 0.3 && (q.y - p.y).abs() <= 0.3
    };
    grid.off_links().iter().copied().find(|l| at(l.from, take_off) && at(l.to, landing))
}

/// What a run came to.
#[derive(Debug)]
struct Run {
    /// The clock of the first jump press with no sprint key within [`HIT_M`] of the take-off, ms.
    walk_press: Option<u32>,
    /// The clock the feet were first on the landing's top, ms.
    arrived: Option<u32>,
    /// Ticks the bot sent the sprint key while hit.
    sprint_keys_hit: u32,
}

/// A bot sent from `start` to `goal`, hit every tick once within [`HIT_M`] of `take_off`.
fn run(world: &Arc<World>, start: Vec3, take_off: Vec3, goal: Vec3, until: u32) -> Run {
    let tuning = Tuning::default();
    let teams = [0u8, 1];
    let mut bots = Bots::new(0x1005, survarium::v026g::maps::RuleMode::TeamDeathmatch, 0, &teams, &[true, false], &[None, None], Arc::new(tuning), Trace::none(0));
    bots.map_ready(world.clone());
    let mut game = Game::new(world.clone(), &teams, 3, None, None);
    game.bodies[0].node = world.grid.locate(start, 1.0);
    game.bodies[0].feet = start;
    game.bodies[1].alive = false;
    bots.entered(0, true, 0);
    let (mut clock, mut next_world, mut hit) = (0, 1_000, false);
    let mut out = Run { walk_press: None, arrived: None, sprint_keys_hit: 0 };
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
        let feet = game.bodies[0].feet;
        hit |= feet.distance_xz(take_off) <= HIT_M && (feet.y - take_off.y).abs() <= 0.5;
        if let Some(bot) = bots.bots[0].as_mut() {
            bot.intent = Intent { go: Go::To { at: goal, within_m: 0.0 }, pace: Pace::Run, fire: false, exact_arrive_m: Some(0.3), ..Intent::stay() };
            if hit {
                bot.hurt.last_hit_ms = Some(clock);
            }
        }
        let mut made = Vec::new();
        bots.ticks(0, &clock_now, &view, &mut made);
        for made in made {
            let a = made.input.actions;
            if hit {
                out.sprint_keys_hit += u32::from(a & key::SPRINT != 0);
                if out.walk_press.is_none() && a & key::JUMP != 0 && a & key::SPRINT == 0 && feet.distance_xz(take_off) <= HIT_M {
                    out.walk_press = Some(clock);
                }
            }
            let r = made.input.rotation;
            let turn = (half_value(u16::from_le_bytes([r[0], r[1]])), half_value(u16::from_le_bytes([r[2], r[3]])));
            let gun = Arms::fallback().primary.0;
            game.inputs[made.place].push_back((made.time, made.input.actions, turn, None, gun));
        }
        let feet = game.bodies[0].feet;
        if clock % 500 == 0 {
            eprintln!("WALKPRESS t {clock} feet ({:.2}, {:.2}, {:.2}) hit {hit}", feet.x, feet.y, feet.z);
        }
        if feet.distance_xz(goal) <= ON_TOP_M && (feet.y - goal.y).abs() <= 0.3 && game.bodies[0].alive {
            out.arrived = Some(clock);
            break;
        }
    }
    out
}

/// School's sprint gap with its slow walking press: from up to 3 starts 3-8 m off on the take-off's level whose planned route
/// takes the sprint link, a bot hit at the take-off sends no sprint key, presses its jump there with no sprint (the walking
/// press), and is on the landing's top within [`LANDS_WITHIN_MS`] of that press.
#[test]
#[ignore = "a per-spot test on the real mapdata; needs SRP_MAPDATA_DIR"]
fn a_bot_refused_the_sprint_at_schools_gap_takes_it_by_its_slow_walking_press() {
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
    assert!(matches!(sprint.walk, Some((mapdata::grid::gap::WalkingPress::Slow { .. }, _))), "a slow walking press: {:?}", sprint.walk);
    let (a, b) = (grid.position(sprint.from), grid.position(sprint.to));
    eprintln!("WALKPRESS link {:?} walk {:?} take-off ({:.4}, {:.4}, {:.4})", sprint.press, sprint.walk, a.x, a.y, a.z);
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
    let runs: Vec<(Vec3, Run)> = starts.into_iter().map(|start| (start, run(&world, start, a, b, 30_000))).collect();
    let landed = runs.iter().filter(|(_, r)| matches!((r.walk_press, r.arrived), (Some(p), Some(t)) if t <= p + LANDS_WITHIN_MS)).count();
    for (start, r) in &runs {
        eprintln!("WALKPRESS from ({:.2}, {:.2}, {:.2}) ({:.1} m): {r:?}", start.x, start.y, start.z, start.distance_xz(a));
    }
    eprintln!("WALKPRESS landed {landed} of {}", runs.len());
    for (start, r) in &runs {
        assert_eq!(r.sprint_keys_hit, 0, "from {start:?}: the sprint is refused while hit");
        let press = r.walk_press.unwrap_or_else(|| panic!("from {start:?}: no jump press with no sprint at the take-off"));
        let arrived = r.arrived.unwrap_or_else(|| panic!("from {start:?}: never reached the landing {b:?}"));
        assert!(arrived <= press + LANDS_WITHIN_MS, "from {start:?}: on the top at {arrived} ms, the walking press at {press} ms");
    }
}
