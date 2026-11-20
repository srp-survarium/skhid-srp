//! [loops] The final build's loop spots (the 3 local matches of 2026-11-19, `LOOPS.md` cause 1: slow gap presses waited at
//! forever): at each spot, a bot sent across the spot's link from starts whose planned route takes it, on the stand-in over
//! the real mapdata, the client's own steps and blend. Per start: whether it pressed, whether it came down on the link's
//! landing, how long it took, and the stuck notes on the way. A lab, not a gate.
//!
//! ```text
//! SRP_MAPDATA_DIR=<dir> [SRP_BENCH_GRID_CACHE=<dir>] cargo test -p match-server --release loops_spots -- --ignored --nocapture
//! ```

use super::*;

use crate::v026g::bots::path;
use crate::v026g::bots::plays::{Go, Intent, Pace};
use crate::v026g::bots::view::Clock;

/// A spot: its project, the take-off and landing the log named (map frame), and its name in the report.
struct Spot {
    name: &'static str,
    project: &'static str,
    take_off: Vec3,
    landing: Vec3,
}

const SPOTS: [Spot; 4] = [
    Spot { name: "map 97 top spot", project: "level_07/day_2", take_off: Vec3::new(65.01, 25.01, -31.04), landing: Vec3::new(64.01, 24.09, -30.79) },
    Spot { name: "map 97 x = 58 ledge", project: "level_07/day_2", take_off: Vec3::new(58.01, 25.01, -10.54), landing: Vec3::new(57.26, 26.1, -10.54) },
    Spot { name: "map 103 rise 1.8 m", project: "level_08/day_2", take_off: Vec3::new(5.5, 32.6, 54.6), landing: Vec3::new(5.7, 34.4, 54.1) },
    Spot { name: "map 91 rise 1.5 m", project: "level_06/day_2", take_off: Vec3::new(-22.6, 24.3, 57.9), landing: Vec3::new(-21.9, 25.8, 57.9) },
];

/// What one run across the link came to.
#[derive(Debug, Default)]
struct Crossing {
    presses: u32,
    /// The clock the feet were first on the landing (0.6 m across, 0.5 m in height), ms.
    landed: Option<u32>,
    stuck: usize,
    /// The ms the body was at rest (under 0.3 m/s) within 2 m of the take-off.
    rest_ms: u32,
    /// The least distance of the feet to the take-off, flat, m.
    nearest_m: f32,
}

/// The off-mesh link (gap or rim jump) of `grid` from a floor within `reach` of `take_off` to one within `reach` of `landing`.
fn find_link(grid: &mapdata::grid::WalkGrid, take_off: Vec3, landing: Vec3, reach: f32) -> Option<mapdata::grid::gap::OffLink> {
    let near = |n: NodeId, p: Vec3| {
        let q = grid.position(n);
        q.distance_xz(p) <= reach && (q.y - p.y).abs() <= 0.5
    };
    grid.off_links().iter().copied().filter(|l| near(l.from, take_off) && near(l.to, landing)).min_by(|a, b| {
        let d = |l: &mapdata::grid::gap::OffLink| grid.position(l.from).distance_xz(take_off) + grid.position(l.to).distance_xz(landing);
        d(a).total_cmp(&d(b))
    })
}

/// A walk to `goal` from `start` for `until` ms, as `door_walk`'s.
fn cross(world: &Arc<World>, start: Vec3, take_off: Vec3, landing: Vec3, until: u32) -> Crossing {
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
    let mut out = Crossing { nearest_m: f32::MAX, ..Crossing::default() };
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
            bot.intent = Intent { go: Go::To { at: landing, within_m: 0.0 }, pace: Pace::Run, fire: false, exact_arrive_m: Some(0.3), ..Intent::stay() };
        }
        let mut made = Vec::new();
        bots.ticks(0, &clock_now, &view, &mut made);
        for made in made {
            if made.input.actions & key::JUMP != 0 {
                out.presses += 1;
            }
            let r = made.input.rotation;
            let turn = (half_value(u16::from_le_bytes([r[0], r[1]])), half_value(u16::from_le_bytes([r[2], r[3]])));
            let gun = Arms::fallback().primary.0;
            game.inputs[made.place].push_back((made.time, made.input.actions, turn, None, gun));
        }
        out.stuck += notes.iter().filter(|n| n.text.contains("is stuck at")).count();
        let feet = game.bodies[0].feet;
        if clock % TICK_MS == 0 {
            let speed = feet.distance_xz(last) / (TICK_MS as f32 / 1000.0);
            last = feet;
            if feet.distance_xz(take_off) <= 2.0 && (feet.y - take_off.y).abs() <= 0.6 && speed < 0.3 {
                out.rest_ms += TICK_MS;
            }
        }
        out.nearest_m = out.nearest_m.min(feet.distance_xz(take_off));
        if out.landed.is_none() && feet.distance_xz(landing) <= 0.6 && (feet.y - landing.y).abs() <= 0.5 && game.bodies[0].alive {
            out.landed = Some(clock);
            break;
        }
    }
    out
}

#[test]
#[ignore = "a lab; needs SRP_MAPDATA_DIR"]
fn loops_spots() {
    let Some(dir) = bench_settings().var_os("SRP_MAPDATA_DIR").map(std::path::PathBuf::from) else {
        eprintln!("skipping: SRP_MAPDATA_DIR is not set");
        return;
    };
    let t = Tuning::default();
    let mut worlds: Vec<(&str, Arc<World>)> = Vec::new();
    for spot in &SPOTS {
        let world = match worlds.iter().find(|(p, _)| *p == spot.project) {
            Some((_, w)) => w.clone(),
            None => {
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
                let w = Arc::new(super::grid_cache::world(map, map_file, mapdata::CELL, t.grid_body(), t.off_mesh_links(), extras));
                worlds.push((spot.project, w.clone()));
                w
            }
        };
        let grid = &world.grid;
        // The log named the take-off; the landing only by its rise where it named no point: any link from within 0.6 m of the
        // take-off rising as named (within 0.35 m), the nearest.
        let by_rise = || {
            let rise = spot.landing.y - spot.take_off.y;
            grid.off_links()
                .iter()
                .copied()
                .filter(|l| {
                    let (p, q) = (grid.position(l.from), grid.position(l.to));
                    p.distance_xz(spot.take_off) <= 0.6 && (p.y - spot.take_off.y).abs() <= 0.5 && ((q.y - p.y) - rise).abs() <= 0.35
                })
                .min_by(|x, y| grid.position(x.from).distance_xz(spot.take_off).total_cmp(&grid.position(y.from).distance_xz(spot.take_off)))
        };
        let Some(link) = find_link(grid, spot.take_off, spot.landing, 1.0).or_else(|| find_link(grid, spot.take_off, spot.landing, 2.0)).or_else(by_rise) else {
            eprintln!("LOOPS {}: no off-mesh link near {:?} -> {:?} on this grid", spot.name, spot.take_off, spot.landing);
            continue;
        };
        let (a, b) = (grid.position(link.from), grid.position(link.to));
        let way = link.press_way(a, b);
        eprintln!(
            "LOOPS {}: link {:?} -> {:?} kind {:?} press {:?} share {:?} way {:?} (line {:?}) window {:?} aim {:?} room at take-off {:.2}",
            spot.name, a, b, link.kind, link.press, link.share, way, link.way, link.window(), link.aim, grid.headroom(link.from)
        );
        // Starts on the take-off's top 3-8 m off whose planned route takes this link: up to 4, spread round it.
        let mut starts: Vec<Vec3> = Vec::new();
        for n in grid.floors_in([a.x - 8.0, a.z - 8.0, a.x + 8.0, a.z + 8.0]).step_by(3) {
            let p = grid.position(n);
            let d = p.distance_xz(a);
            if !(3.0..=8.0).contains(&d) || (p.y - a.y).abs() > 1.0 || starts.iter().any(|s| s.distance_xz(p) < 2.5) {
                continue;
            }
            let Some(route) = path::plan(grid, n, link.to) else { continue };
            let pts = route.points();
            let takes = pts.windows(2).any(|w| w[1].jump.is_some() && w[0].at.distance_xz(a) < 0.3 && w[1].at.distance_xz(b) < 0.3);
            if takes {
                starts.push(p);
            }
            if starts.len() >= 4 {
                break;
            }
        }
        if starts.is_empty() {
            eprintln!("LOOPS {}: no start within 3-8 m whose route takes the link", spot.name);
        }
        for start in starts {
            let c = cross(&world, start, a, b, 40_000);
            eprintln!(
                "LOOPS {}: from {:?} ({:.1} m): landed {:?} ms, presses {}, rest {} ms near the take-off, stuck notes {}, nearest {:.2} m",
                spot.name, start, start.distance_xz(a), c.landed, c.presses, c.rest_ms, c.stuck, c.nearest_m
            );
        }
    }
}
