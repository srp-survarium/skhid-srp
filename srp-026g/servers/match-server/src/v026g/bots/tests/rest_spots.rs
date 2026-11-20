//! [map 91] A slow gap press whose rest point has no floor in the walk grid: Mamayev Kurgan's 1.5 m rise, the gap jump
//! (-22.60, 24.27, 57.88) -> (-21.85, 25.81, 57.88) the final build's matches waited at (`LOOPS.md` cause 1). The grid flew it as a
//! standing press 0.4 m behind the take-off (its window 0.4-0.6 m), where the walk grid holds no floor; 0.2-0.3 m behind it holds a
//! raised floor (0.26 m up), inside the motor's rest window. On the stand-in over the real mapdata, a bot sent across the link from
//! every start 3-8 m off whose planned route takes it must come down on its landing.
//!
//! ```text
//! SRP_MAPDATA_DIR=<dir> [SRP_BENCH_GRID_CACHE=<dir>] cargo test -p match-server --release rest_spots -- --ignored --nocapture
//! ```

use super::*;

use crate::v026g::bots::path;
use crate::v026g::bots::plays::{Go, Intent, Pace};
use crate::v026g::bots::view::Clock;

const PROJECT: &str = "level_06/day_2";
/// The link's ends as the grid holds them (map frame).
const TAKE_OFF: Vec3 = Vec3::new(-22.601, 24.266, 57.875);
const LANDING: Vec3 = Vec3::new(-21.851, 25.805, 57.875);

/// What one run across the link came to.
#[derive(Debug, Default)]
struct Crossing {
    presses: u32,
    /// The clock the feet were first on the landing (0.6 m across, 0.5 m in height), ms.
    landed: Option<u32>,
    /// The ms the body was at rest (under 0.3 m/s) within 2 m of the take-off.
    rest_ms: u32,
    /// The feet at the first press.
    pressed_at: Option<Vec3>,
}

/// A bot on the stand-in walks to `goal` from `start` for at most `until` ms (as `drop_spots`' walk).
fn cross(world: &Arc<World>, start: Vec3, take_off: Vec3, landing: Vec3, goal: Vec3, until: u32) -> Crossing {
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
    let mut out = Crossing::default();
    let mut last = start;
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
            bot.intent = Intent { go: Go::To { at: goal, within_m: 0.0 }, pace: Pace::Run, fire: false, exact_arrive_m: Some(0.3), ..Intent::stay() };
        }
        let mut made = Vec::new();
        bots.ticks(0, &clock_now, &view, &mut made);
        for made in made {
            if made.input.actions & key::JUMP != 0 {
                out.presses += 1;
                out.pressed_at.get_or_insert(game.bodies[0].feet);
            }
            let r = made.input.rotation;
            let turn = (half_value(u16::from_le_bytes([r[0], r[1]])), half_value(u16::from_le_bytes([r[2], r[3]])));
            let gun = Arms::fallback().primary.0;
            game.inputs[made.place].push_back((made.time, made.input.actions, turn, None, gun));
        }
        bots.nav_notes(0, &mut notes);
        let feet = game.bodies[0].feet;
        if clock % TICK_MS == 0 {
            let speed = feet.distance_xz(last) / (TICK_MS as f32 / 1000.0);
            last = feet;
            if feet.distance_xz(take_off) <= 2.0 && (feet.y - take_off.y).abs() <= 0.6 && speed < 0.3 {
                out.rest_ms += TICK_MS;
            }
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
fn a_slow_press_with_no_grid_floor_at_its_rest_is_made() {
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
    let near = |n: NodeId, p: Vec3| {
        let q = grid.position(n);
        q.distance_xz(p) <= 0.3 && (q.y - p.y).abs() <= 0.3
    };
    let link = grid
        .off_links()
        .iter()
        .copied()
        .find(|l| near(l.from, TAKE_OFF) && near(l.to, LANDING))
        .expect("the match's link is on this grid");
    let (a, b) = (grid.position(link.from), grid.position(link.to));
    let way = link.press_way(a, b);
    eprintln!("REST link {a:?} -> {b:?} kind {:?} press {:?} window {:?} way {way:?}", link.kind, link.press, link.window());
    // What the walk grid holds behind the take-off along the press's way: its floors, each with whether the take-off has a
    // straight walk to it.
    for k in 0..=10 {
        let m = k as f32 * 0.1;
        let p = Vec3::new(a.x - way.0 * m, a.y, a.z - way.1 * m);
        let floors: Vec<String> = grid
            .floors_in([p.x, p.z, p.x, p.z])
            .map(|n| {
                let q = grid.position(n);
                format!("({:.2}, {:.2}, {:.2}) straight {}", q.x, q.y, q.z, path::straight_where(grid, link.from, n, |_| true))
            })
            .collect();
        eprintln!("REST behind {m:.1} m at ({:.2}, {:.2}): {}", p.x, p.z, if floors.is_empty() { "no floor".to_string() } else { floors.join(" | ") });
    }
    // The goal past the link: the nearest floor 3-15 m from the landing, off any lip, that the take-off's own route reaches over
    // the link (a rest on a lip's floor is moved off the lip, `path::off_lip`, down off the top too).
    let takes = |route: &path::Route| route.points().windows(2).any(|w| w[1].jump.is_some() && w[0].at.distance_xz(a) < 0.3 && w[1].at.distance_xz(b) < 0.3);
    let (goal_node, goal) = grid
        .floors_in([b.x - 15.0, b.z - 15.0, b.x + 15.0, b.z + 15.0])
        .step_by(2)
        .map(|n| (n, grid.position(n)))
        .filter(|&(n, p)| (3.0..=15.0).contains(&p.distance_xz(b)) && !grid.is_lip(n) && !grid.near_lip(n))
        .filter(|&(n, _)| path::plan(grid, link.from, n).is_some_and(|r| takes(&r)))
        .min_by(|x, y| x.1.distance_xz(b).total_cmp(&y.1.distance_xz(b)))
        .expect("a floor past the link the take-off's route reaches over it");
    // Starts on the take-off's floor 3-8 m off whose planned route to the goal takes the link: up to 4, 2.5 m apart.
    let mut starts: Vec<Vec3> = Vec::new();
    for n in grid.floors_in([a.x - 8.0, a.z - 8.0, a.x + 8.0, a.z + 8.0]).step_by(3) {
        let p = grid.position(n);
        let d = p.distance_xz(a);
        if !(3.0..=8.0).contains(&d) || (p.y - a.y).abs() > 1.0 || starts.iter().any(|s| s.distance_xz(p) < 2.5) {
            continue;
        }
        if path::plan(grid, n, goal_node).is_some_and(|r| takes(&r)) {
            starts.push(p);
        }
        if starts.len() >= 4 {
            break;
        }
    }
    assert!(!starts.is_empty(), "no start 3-8 m off whose route takes the link");
    let mut missed = Vec::new();
    for start in starts {
        let c = cross(&world, start, a, b, goal, 40_000);
        eprintln!(
            "REST from ({:.2}, {:.2}, {:.2}) ({:.1} m): landed {:?} ms, presses {}, first press at {:?}, rest {} ms near the take-off",
            start.x, start.y, start.z, start.distance_xz(a), c.landed, c.presses, c.pressed_at, c.rest_ms
        );
        if c.landed.is_none() {
            missed.push(start);
        }
    }
    assert!(missed.is_empty(), "the bot did not come down on the landing from {missed:?}");
}
