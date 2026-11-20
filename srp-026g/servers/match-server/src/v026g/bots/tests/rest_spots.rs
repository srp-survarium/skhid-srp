//! [map 91] A slow gap press whose rest point has no floor in the walk grid: Mamayev Kurgan's 1.5 m rise, the gap jump
//! (-22.60, 24.27, 57.88) -> (-21.85, 25.81, 57.88) the final build's matches waited at (`LOOPS.md` cause 1). The grid flew it as a
//! standing press 0.4 m behind the take-off (its window 0.4-0.6 m), where the walk grid holds no floor; 0.2-0.3 m behind it holds a
//! raised floor (0.26 m up), inside the motor's rest window. On the stand-in over the real mapdata and the server's game data, a
//! bot is sent across the link from up to 4 starts 3-8 m off whose planned route takes it. Asserted: the link is pressed, and every
//! press at it is the grid's (from rest, within the stored lead window). Reported, not asserted: where the bot comes down. The
//! stand-in lands presses the exe does not (rig_real_v3: from 0.11-0.13 m to the line's left, where the stored window (0, 0.15)
//! has no width, the exe body comes down short of the top at every lead tried), so its landing here is not the client's.
//! The motor's own press path: run with the press forecast off (`Feel::press_forecast`; `press_forecast_spots` runs it on).
//!
//! ```text
//! SRP_MAPDATA_DIR=<dir> SRP_GAME_DATA=<server-game-data.db> [SRP_BENCH_GRID_CACHE=<dir>] cargo test -p match-server --release rest_spots -- --ignored --nocapture
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
pub(super) struct Crossing {
    pub(super) presses: u32,
    /// The clock the feet were first on the landing (0.6 m across, 0.5 m in height), ms.
    pub(super) landed: Option<u32>,
    /// The ms the body was at rest (under 0.3 m/s) within 2 m of the take-off.
    pub(super) rest_ms: u32,
    /// The feet at the first press.
    pressed_at: Option<Vec3>,
    /// Every press.
    pub(super) presses_at: Vec<Press>,
    /// The bot's notes (ms, text), printed with `SRP_REST_TRAIL=1`.
    pub(super) notes: Vec<(u32, String)>,
    /// The feet at every press, and every 500 ms (`SRP_REST_TRAIL=1` prints them).
    pub(super) trail: Vec<(u32, Vec3, bool)>,
}

/// A press of the jump key on the way.
#[derive(Debug)]
pub(super) struct Press {
    pub(super) feet: Vec3,
    /// The body's speed over the stand-in's last step (a game tick), m/s.
    pub(super) speed: f32,
    /// The motor's route jump it pressed, its take-off and landing (`Motor::route_press`; None: another jump, the stuck watch's,
    /// an escape's).
    pub(super) link: Option<(Vec3, Vec3)>,
}

/// A bot on the stand-in walks to `goal` from `start` for at most `until` ms (as `drop_spots`' walk).
pub(super) fn cross(world: &Arc<World>, start: Vec3, take_off: Vec3, landing: Vec3, goal: Vec3, until: u32, tuning: Tuning) -> Crossing {
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
    let mut step_before = start;
    while clock < until {
        clock += 25;
        while game.tick + TICK_MS <= clock {
            step_before = game.bodies[0].feet;
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
                let feet = game.bodies[0].feet;
                let motor = bots.bots[0].as_ref().and_then(|bot| bot.motor.route_press()).map(|press| (press.take_off, press.landing));
                out.presses_at.push(Press { feet, speed: feet.distance_xz(step_before) / (TICK_MS as f32 / 1000.0), link: motor });
                out.trail.push((clock, game.bodies[0].feet, true));
            }
            let r = made.input.rotation;
            let turn = (half_value(u16::from_le_bytes([r[0], r[1]])), half_value(u16::from_le_bytes([r[2], r[3]])));
            let gun = Arms::fallback().primary.0;
            game.inputs[made.place].push_back((made.time, made.input.actions, turn, None, gun));
        }
        bots.nav_notes(0, &mut notes);
        out.notes.extend(notes.iter().map(|note| (note.time, note.text.clone())));
        let feet = game.bodies[0].feet;
        if clock % 500 == 0 {
            out.trail.push((clock, feet, false));
        }
        if clock % TICK_MS == 0 {
            let speed = feet.distance_xz(last) / (TICK_MS as f32 / 1000.0);
            last = feet;
            if feet.distance_xz(take_off) <= 2.0 && (feet.y - take_off.y).abs() <= 0.6 && speed < 0.3 {
                out.rest_ms += TICK_MS;
            }
        }
        // Run on 2.6 s past the landing, so the motor's line of a forecast press (written 2.5 s after it) is in the notes.
        if out.landed.is_none() && feet.distance_xz(landing) <= 0.6 && (feet.y - landing.y).abs() <= 0.5 && game.bodies[0].alive {
            out.landed = Some(clock);
        }
        if out.landed.is_some_and(|at| clock >= at + 2_600) {
            break;
        }
    }
    out
}

/// The map 91 link on the real map: its world, ends, press way and stored window, the goal past it and up to 4 starts whose
/// planned route takes it.
pub(super) struct Map91 {
    pub(super) world: Arc<World>,
    pub(super) a: Vec3,
    pub(super) b: Vec3,
    pub(super) way: (f32, f32),
    pub(super) window: Option<mapdata::grid::gap::TakeoffWindow>,
    pub(super) goal: Vec3,
    pub(super) goal_node: NodeId,
    pub(super) starts: Vec<Vec3>,
}

/// [`Map91`] from `SRP_MAPDATA_DIR`; none (said so) when it is not set.
pub(super) fn map91() -> Option<Map91> {
    let Some(dir) = super::real_maps::dir() else {
        eprintln!("skipping: SRP_MAPDATA_DIR is not set");
        return None;
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
    let window = link.window();
    Some(Map91 { world, a, b, way, window, goal, goal_node, starts })
}

#[test]
#[ignore = "needs SRP_MAPDATA_DIR"]
fn a_slow_press_with_no_grid_floor_at_its_rest_is_made() {
    let Some(Map91 { world, a, b, way, window, goal, goal_node, starts }) = map91() else {
        return;
    };
    let grid = &world.grid;
    // The stored window's leads (m behind the take-off along the press's way) and the turn of the way to its right.
    let lead_window = window.expect("a flown press").lead_m;
    let right = (way.1, -way.0);
    let (mut pressed, mut landed, mut off_rule) = (0, 0, Vec::new());
    let n_starts = starts.len();
    for start in starts {
        let mut off = Tuning::default();
        off.feel.press_forecast = false;
        let c = cross(&world, start, a, b, goal, 40_000, off);
        eprintln!(
            "REST from ({:.2}, {:.2}, {:.2}) ({:.1} m): landed {:?} ms, presses {}, first press at {:?}, rest {} ms near the take-off",
            start.x, start.y, start.z, start.distance_xz(a), c.landed, c.presses, c.pressed_at, c.rest_ms
        );
        if std::env::var_os("SRP_REST_TRAIL").is_some() {
            if let Some(route) = grid.locate(start, 1.0).and_then(|n| path::plan(grid, n, goal_node)) {
                for w in route.points().iter().filter(|w| w.jump.is_some()) {
                    eprintln!("REST   route jump to ({:.2}, {:.2}, {:.2}) {:?}", w.at.x, w.at.y, w.at.z, w.jump);
                }
            }
            for (t, p, press) in &c.trail {
                eprintln!("REST   {t} ms ({:.2}, {:.2}, {:.2}){}", p.x, p.y, p.z, if *press { " PRESS" } else { "" });
            }
            for (t, text) in &c.notes {
                eprintln!("REST   note {t} ms: {text}");
            }
        }
        landed += usize::from(c.landed.is_some());
        let mut at_link = false;
        for &Press { feet: p, speed, link } in &c.presses_at {
            // The motor's press of this link (its take-off and landing within 0.3 m of the link's).
            let motor = link.is_some_and(|(from, to)| (from - a).length() <= 0.3 && (to - b).length() <= 0.3);
            let (dx, dz) = (a.x - p.x, a.z - p.z);
            let behind = dx * way.0 + dz * way.1;
            if p.distance_xz(a) > 1.0 || (p.y - a.y).abs() > 0.6 {
                continue;
            }
            at_link |= motor;
            let to_right = -(dx * right.0 + dz * right.1);
            eprintln!(
                "REST   press at the link {behind:.3} m behind, {to_right:+.3} m to the right, {speed:.2} m/s, {}",
                match link {
                    _ if motor => "the motor's press of the link".to_string(),
                    Some((from, to)) => format!("the motor's press of another link ({:.2}, {:.2}, {:.2}) -> ({:.2}, {:.2}, {:.2})", from.x, from.y, from.z, to.x, to.y, to.z),
                    None => "not the motor's (another jump)".to_string(),
                }
            );
            // The motor's press of a flown stand press: from rest (under 0.3 m/s over the last step) and within the stored lead
            // window, the motor's lead gate's 0.2 m on its earlier side (`lead_off`, F3). Another jump there is reported.
            let in_leads = behind >= lead_window.0 - 0.2 - 0.01 && behind <= lead_window.1 + 0.01;
            if motor && (speed >= 0.3 || !in_leads) {
                off_rule.push((start, behind, to_right, speed));
            }
        }
        pressed += usize::from(at_link);
    }
    eprintln!("REST pressed at the link from {pressed} of {n_starts} starts, came down on the landing (the stand-in's) from {landed}");
    assert!(pressed > 0, "no start pressed the link");
    assert!(off_rule.is_empty(), "presses at the link not from rest within the stored leads: {off_rule:?}");
}
