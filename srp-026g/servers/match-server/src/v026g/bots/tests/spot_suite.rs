//! [spot suite] The problematic spots of the recordings walked on the exact port (`BRIEF-spot-suite-80pct-20261119.md`): the
//! owner's target is 0 errors on at least 80 % of them.
//!
//! The spots are `spots-v1.tsv` (the spots that failed in 2+ of 86 recordings, with the link the bots failed at there). Each is
//! walked by ONE bot of the bench (its role-0 kit, the bench's tuning, `Game`'s stand-in body with the client's walk tick) on the
//! map's real built grid, from up to 3 starts 10-30 m before the spot to a goal past it: the bot plays nothing, its intent is
//! pinned to the goal at a run (`Bot::bench_pin`); everything else (route, motor, link presses, lip guard, stuck recovery, dead
//! links) is the server's own. The other team's one place is dead from the start and never respawns.
//!
//! Two runs, both `#[ignore]`d and driven by the environment (with the bench's own: `SRP_MAPDATA_DIR`, `SRP_GAME_DATA`,
//! `SRP_BENCH_GRID_CACHE`, the preset's `SRP_BENCH_*`):
//! - `spot_suite_plan`: `SRP_SPOT_SUITE_SPOTS=<spots-v1.tsv>`, `SRP_SPOT_SUITE_STARTS=<out tsv>`: per spot, its point P (the
//!   link's landing, or the spot's centre), its take-off A (the link's start); the goal G: the grid floor 4-10 m of way past P,
//!   furthest from A (from P when A is P); the starts: grid floors 10-30 m from P (flat) whose shortest grid way to G passes A
//!   then P, the three most direct ones of different bearings. With no such floor, G = P; with none still, the spot has no
//!   grid way through it (`via` no-route). Made once and kept, so every commit walks the same starts.
//! - `spot_suite_run`: `SRP_SPOT_SUITE_STARTS=<starts tsv>`, `SRP_SPOT_SUITE_OUT=<results tsv>`, optional
//!   `SRP_SPOT_SUITE_ONLY=S001,S017`, `SRP_SPOT_SUITE_THREADS=<n>` (default 2), `SRP_SPOT_SUITE_SECONDS` (default 60).
//!   One line a start: whether the bot reached G, passed P, and its ERRORS, read from its own notes:
//!   - `stuck`: "is stuck at";
//!   - `jump`: "nav: route jump ... (<end>)" whose end is not "went on" (a link given up or failed);
//!   - `dead`: "dead link on map";
//!   - `fall`: "nav: fell ... (unplanned)";
//!   - `lip`: "nav: lip guard braked at" within 2 m of A or P (a brake at a planned take-off);
//!   - `noway`: "no way" to its goal;
//!   - `died`: the body died;
//!   - `timeout`: not at G after the limit.
//!   A spot passes when every start reaches G with 0 errors.

use std::collections::{BinaryHeap, HashMap};
use std::io::Write;
use std::sync::Mutex;

use mapdata::{NodeId, WalkGrid};

use super::*;
use super::bench::{bench_client_sway, bench_equipment, bench_open_sides, bench_roster, bench_throws, bench_tuning, bench_wall_clearance};
use super::settings::bench_settings;

/// The starts: this far from P across the ground, m.
const START_MIN_M: f32 = 10.0;
const START_MAX_M: f32 = 30.0;
/// The goal: this much grid way past P, m.
const GOAL_MIN_M: f32 = 4.0;
const GOAL_MAX_M: f32 = 10.0;
/// The floors looked at around P: half the side of the square, m.
const BOX_M: f32 = 45.0;
/// A way passes a point within this across the ground and this in height, m.
const PASS_M: f32 = 1.0;
const PASS_Y_M: f32 = 1.0;
/// Starts this many degrees apart in bearing from P at least (relaxed when too few).
const BEARING_APART_DEG: f32 = 50.0;
/// Reached: within this of G across the ground and in height, m.
const REACH_M: f32 = 1.2;
const REACH_Y_M: f32 = 1.0;
/// A lip guard brake counts when within this of A or P, m.
const LIP_NEAR_M: f32 = 2.0;

fn flat(a: Vec3, b: Vec3) -> f32 {
    ((a.x - b.x).powi(2) + (a.z - b.z).powi(2)).sqrt()
}

fn near(a: Vec3, b: Vec3) -> bool {
    flat(a, b) <= PASS_M && (a.y - b.y).abs() <= PASS_Y_M
}

fn cost(p: Vec3, q: Vec3) -> u32 {
    ((flat(p, q) + (q.y - p.y).abs()) * 1000.0) as u32
}

fn tsv_rows(path: &str) -> (Vec<String>, Vec<HashMap<String, String>>) {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let mut lines = text.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty());
    let head: Vec<String> = lines.next().unwrap_or_else(|| panic!("{path}: empty")).split('\t').map(str::to_owned).collect();
    let rows = lines
        .map(|l| head.iter().cloned().zip(l.split('\t').map(str::to_owned)).collect())
        .collect();
    (head, rows)
}

fn v3(row: &HashMap<String, String>, x: &str, y: &str, z: &str) -> Option<Vec3> {
    let n = |k: &str| row.get(k).and_then(|v| v.parse::<f32>().ok()).filter(|v| v.is_finite());
    Some(Vec3::new(n(x)?, n(y)?, n(z)?))
}

fn rule_mode(project: &str) -> (u8, survarium::v026g::maps::RuleMode) {
    let level: u8 = project[6..8].parse().unwrap_or_else(|e| panic!("{project}: {e}"));
    let day: u8 = project.rsplit('_').next().and_then(|d| d.parse().ok()).unwrap_or_else(|| panic!("{project}: day"));
    (level, survarium::v026g::maps::RuleMode::of_config(day).unwrap_or_else(|| panic!("{project}: mode {day}")))
}

/// As the bench's setup: the game data (`SRP_GAME_DATA`, the server's file, every table checked) and the bench run's units.
fn setup() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        if bench_settings().var_os("SRP_GAME_DATA").is_some_and(|p| !p.is_empty()) {
            let data = survarium::v026g::game_data::init().unwrap_or_else(|cause| panic!("SRP_GAME_DATA: {cause}"));
            eprintln!("game data: {}", data.origin);
        } else {
            eprintln!("game data: THE BUILT-IN TEST ARCHIVE (set SRP_GAME_DATA to the server's file)");
        }
        super::standin::start_bench_run();
    });
}

/// The map's world as the bench builds it (from the shared grid cache when `SRP_BENCH_GRID_CACHE` is set).
fn world_of(project: &str) -> Arc<World> {
    let dir = std::path::PathBuf::from(bench_settings().var_os("SRP_MAPDATA_DIR").expect("SRP_MAPDATA_DIR"));
    let file = dir.join(mapdata::format::file_name(project));
    let bytes = std::fs::read(&file).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
    let map_file = {
        let mut digest = mapdata::digest::Digest::new();
        digest.bytes(&bytes);
        digest.finish()
    };
    let map = mapdata::MapData::from_bytes(&bytes).unwrap();
    let links = bench_tuning().off_mesh_links();
    let extras = mapdata::GridExtras {
        wall_clearance: bench_wall_clearance(),
        open_sides: bench_open_sides(),
        pull_hold: bench_tuning().grid.wall_clear_pull.then_some(crate::v026g::bots::tuning::PULL_HOLD_M),
        pull_walk: bench_tuning().grid.pull_walk,
        walker_shapes: bench_tuning().grid.walker_shapes,
    };
    Arc::new(super::grid_cache::world(map, map_file, mapdata::CELL, bench_tuning().grid_body(), links, extras))
}

/// The floor of `p` (within PASS_M / PASS_Y_M, the nearest), else the grid's nearest within 2 m.
fn floor_of(grid: &WalkGrid, p: Vec3) -> Option<NodeId> {
    grid.floors_in([p.x - PASS_M, p.z - PASS_M, p.x + PASS_M, p.z + PASS_M])
        .filter(|&n| (grid.position(n).y - p.y).abs() <= PASS_Y_M)
        .min_by(|&a, &b| {
            let (pa, pb) = (grid.position(a), grid.position(b));
            (flat(pa, p) + (pa.y - p.y).abs()).total_cmp(&(flat(pb, p) + (pb.y - p.y).abs()))
        })
        .or_else(|| grid.locate(p, 2.0))
}

/// The floors in the box around P, with their links inside it (forward and reverse).
struct Local {
    nodes: Vec<NodeId>,
    index: HashMap<NodeId, usize>,
    fwd: Vec<Vec<usize>>,
    rev: Vec<Vec<usize>>,
}

impl Local {
    fn around(grid: &WalkGrid, p: Vec3) -> Self {
        let nodes: Vec<NodeId> = grid.floors_in([p.x - BOX_M, p.z - BOX_M, p.x + BOX_M, p.z + BOX_M]).collect();
        let index: HashMap<NodeId, usize> = nodes.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        let mut fwd = vec![Vec::new(); nodes.len()];
        let mut rev = vec![Vec::new(); nodes.len()];
        for (i, &n) in nodes.iter().enumerate() {
            for (m, _) in grid.links(n) {
                if let Some(&j) = index.get(&m) {
                    fwd[i].push(j);
                    rev[j].push(i);
                }
            }
        }
        Self { nodes, index, fwd, rev }
    }

    /// Dijkstra from `from` over `fwd` (or `rev`), cost flat + rise, up to `limit_m`: (cost mm, the next node toward `from`).
    fn dijkstra(&self, grid: &WalkGrid, from: usize, reverse: bool, limit_m: f32) -> Vec<Option<(u32, usize)>> {
        let mut best: Vec<Option<(u32, usize)>> = vec![None; self.nodes.len()];
        best[from] = Some((0, from));
        let mut heap = BinaryHeap::from([(std::cmp::Reverse(0u32), from)]);
        let limit = (limit_m * 1000.0) as u32;
        while let Some((std::cmp::Reverse(d), i)) = heap.pop() {
            if best[i].is_some_and(|(b, _)| d > b) {
                continue;
            }
            let p = grid.position(self.nodes[i]);
            let next = if reverse { &self.rev[i] } else { &self.fwd[i] };
            for &j in next {
                let q = grid.position(self.nodes[j]);
                let nd = d + if reverse { cost(q, p) } else { cost(p, q) };
                if nd <= limit && best[j].is_none_or(|(b, _)| nd < b) {
                    best[j] = Some((nd, i));
                    heap.push((std::cmp::Reverse(nd), j));
                }
            }
        }
        best
    }
}

struct Start {
    at: Vec3,
    way_m: f32,
}

/// The goal and starts of one spot: (G, starts, via).
fn plan_spot(grid: &WalkGrid, p: Vec3, a: Vec3) -> Option<(Vec3, Vec<Start>, &'static str)> {
    let local = Local::around(grid, p);
    let pn = floor_of(grid, p).and_then(|n| local.index.get(&n).copied())?;
    let p_at = grid.position(local.nodes[pn]);
    let a_distinct = flat(a, p_at) >= 0.5 || (a.y - p_at.y).abs() >= 0.5;
    let past = local.dijkstra(grid, pn, false, GOAL_MAX_M);
    let away_from = if a_distinct { a } else { p_at };
    let goal = past
        .iter()
        .enumerate()
        .filter_map(|(i, b)| b.filter(|(d, _)| (GOAL_MIN_M * 1000.0) as u32 <= *d).map(|_| i))
        .max_by(|&i, &j| flat(grid.position(local.nodes[i]), away_from).total_cmp(&flat(grid.position(local.nodes[j]), away_from)));
    for (g, via) in [(goal, "link"), (Some(pn), "goal-at-spot")] {
        let Some(g) = g else { continue };
        let to_goal = local.dijkstra(grid, g, true, 3.0 * START_MAX_M);
        let mut found: Vec<(f32, f32, usize, f32)> = Vec::new(); // (directness, bearing, node, way m)
        for (i, b) in to_goal.iter().enumerate() {
            let Some((d, _)) = *b else { continue };
            let s = grid.position(local.nodes[i]);
            let r = flat(s, p_at);
            if !(START_MIN_M..=START_MAX_M).contains(&r) || !grid.reached(local.nodes[i]) || !grid.returns(local.nodes[i]) {
                continue;
            }
            // Follow the way to G: A then P (P alone when A is P).
            let (mut k, mut seen_a, mut seen_p) = (i, !a_distinct, false);
            let mut hops = 0;
            while let Some((_, next)) = to_goal[k] {
                let q = grid.position(local.nodes[k]);
                seen_a |= near(q, a);
                seen_p |= seen_a && near(q, p_at);
                if k == g || hops > 100_000 {
                    break;
                }
                k = next;
                hops += 1;
            }
            if seen_p {
                let way_m = d as f32 / 1000.0;
                let bearing = (s.z - p_at.z).atan2(s.x - p_at.x).to_degrees();
                found.push((way_m / (flat(s, grid.position(local.nodes[g])) + 1.0), bearing, i, way_m));
            }
        }
        if found.is_empty() {
            continue;
        }
        found.sort_by(|x, y| x.0.total_cmp(&y.0));
        let mut chosen: Vec<(f32, f32, usize, f32)> = Vec::new();
        for apart in [BEARING_APART_DEG, 15.0, 0.0] {
            for f in &found {
                if chosen.len() >= 3 {
                    break;
                }
                let off = |c: &(f32, f32, usize, f32)| {
                    let d = (c.1 - f.1).rem_euclid(360.0);
                    d.min(360.0 - d)
                };
                let far = |c: &(f32, f32, usize, f32)| flat(grid.position(local.nodes[c.2]), grid.position(local.nodes[f.2])) >= 3.0;
                if chosen.iter().all(|c| c.2 != f.2 && off(c) >= apart && far(c)) {
                    chosen.push(*f);
                }
            }
        }
        let starts = chosen.iter().map(|c| Start { at: grid.position(local.nodes[c.2]), way_m: c.3 }).collect();
        return Some((grid.position(local.nodes[g]), starts, via));
    }
    None
}

#[test]
#[ignore]
fn spot_suite_plan() {
    let (Ok(spots), Ok(out)) = (std::env::var("SRP_SPOT_SUITE_SPOTS"), std::env::var("SRP_SPOT_SUITE_STARTS")) else {
        eprintln!("skipping: SRP_SPOT_SUITE_SPOTS / SRP_SPOT_SUITE_STARTS not set");
        return;
    };
    setup();
    let (_, rows) = tsv_rows(&spots);
    let mut by_project: Vec<(String, Vec<&HashMap<String, String>>)> = Vec::new();
    for row in &rows {
        let project = row["project"].clone();
        match by_project.iter_mut().find(|(p, _)| *p == project) {
            Some((_, list)) => list.push(row),
            None => by_project.push((project, vec![row])),
        }
    }
    let mut lines = vec![
        "spot\tproject\tstart\tvia\tlink_kind\tsx\tsy\tsz\tgx\tgy\tgz\tpx\tpy\tpz\tax\tay\taz\tway_m".to_owned(),
    ];
    for (project, list) in by_project {
        let world = world_of(&project);
        let grid = &world.grid;
        for row in list {
            let centre = v3(row, "x", "y", "z").unwrap();
            let p = v3(row, "to_x", "to_y", "to_z").unwrap_or(centre);
            let a = v3(row, "from_x", "from_y", "from_z").unwrap_or(p);
            let kind = &row["link_kind"];
            let spot = &row["spot"];
            let fmt = |v: Vec3| format!("{:.2}\t{:.2}\t{:.2}", v.x, v.y, v.z);
            match plan_spot(grid, p, a) {
                Some((g, starts, via)) if !starts.is_empty() => {
                    for (k, s) in starts.iter().enumerate() {
                        lines.push(format!("{spot}\t{project}\t{k}\t{via}\t{kind}\t{}\t{}\t{}\t{}\t{:.1}", fmt(s.at), fmt(g), fmt(p), fmt(a), s.way_m));
                    }
                }
                _ => lines.push(format!("{spot}\t{project}\t-\tno-route\t{kind}\tnan\tnan\tnan\tnan\tnan\tnan\t{}\t{}\tnan")),
            }
            eprintln!("{spot} {project} planned");
        }
    }
    std::fs::write(&out, lines.join("\n") + "\n").unwrap();
    eprintln!("wrote {} lines to {out}", lines.len() - 1);
}

#[derive(Default, Debug)]
struct Outcome {
    reached_ms: Option<u32>,
    passed: bool,
    stuck: u32,
    jump: u32,
    dead: u32,
    fall: u32,
    lip: u32,
    noway: u32,
    died: bool,
    first: Option<String>,
    end: Vec3,
}

impl Outcome {
    fn errors(&self, timeout: bool) -> u32 {
        self.stuck + self.jump + self.dead + self.fall + self.lip + self.noway + u32::from(self.died) + u32::from(timeout)
    }

    fn note(&mut self, text: &str, clock: u32, a: Vec3, p: Vec3) {
        let error = if text.contains("is stuck at") {
            self.stuck += 1;
            true
        } else if let Some(rest) = text.split("nav: route jump ").nth(1) {
            let end = rest.rsplit('(').next().unwrap_or("");
            let bad = !end.starts_with("went on");
            self.jump += u32::from(bad);
            bad
        } else if text.contains("dead link on map") {
            self.dead += 1;
            true
        } else if text.contains("nav: fell ") && text.contains("(unplanned") {
            self.fall += 1;
            true
        } else if let Some(rest) = text.split("nav: lip guard braked at (").nth(1) {
            let n: Vec<f32> = rest.split(')').next().unwrap_or("").split(',').filter_map(|v| v.trim().parse().ok()).collect();
            let at = (n.len() == 3).then(|| Vec3::new(n[0], n[1], n[2]));
            let bad = at.is_some_and(|at| flat(at, a) <= LIP_NEAR_M || flat(at, p) <= LIP_NEAR_M);
            self.lip += u32::from(bad);
            bad
        } else if text.contains("no way") && (text.contains("no way from") || text.contains("no way to its goal")) {
            self.noway += 1;
            true
        } else {
            false
        };
        if error && self.first.is_none() {
            self.first = Some(format!("{clock} {}", text.chars().take(240).collect::<String>().replace('\t', " ")));
        }
    }
}

/// One walk: the lone bot from `start` to `goal` through A and P.
fn walk(world: &Arc<World>, project: &str, start: Vec3, goal: Vec3, a: Vec3, p: Vec3, limit_ms: u32, seed: u32) -> Outcome {
    let (level, mode) = rule_mode(project);
    let teams = [0u8, 1];
    let driven = [true, false];
    let tunings = super::tuning::TeamTunings::new(Arc::new(bench_tuning()));
    let roster = bench_roster(&teams, bench_equipment(), &tunings);
    let throws = bench_throws(&roster, &tunings);
    let mut bots = Bots::new_with(
        0x5907 + seed,
        crate::v026g::bots::objective::Mode::of_rules(mode).unwrap(),
        level,
        &teams,
        &driven,
        &roster.roles,
        tunings,
        Trace::none(0),
    );
    bots = bots.with_tiers(&roster.equipments).with_client_sway(bench_client_sway());
    bots.set_pantries(roster.profiles.iter().map(super::stock::Pantry::of_profile).collect());
    bots.set_throws(throws);
    bots.map_ready(world.clone());
    let mut game = Game::new(world.clone(), &teams, u64::from(seed), None, None);
    game.gates = super::bench_spread::ClientGates {
        sprint_ms: bench_settings().number("SRP_BENCH_SPRINT_GATE"),
        jump_ms: bench_settings().number("SRP_BENCH_JUMP_GATE"),
    };
    game.set_grenade_kits(&roster.grenades);
    game.set_heal_kits(&roster.profiles);
    game.set_radio_kits(&roster.profiles);
    if super::standin_level::enabled() {
        let levels = super::standin_level::levels(&roster.upgrades, None);
        game.set_speed_levels(&levels);
        bots.set_speed_levels(&levels);
        let computed = bots.tunings.match_wide().kin.level_source == super::kin::LevelSource::Computed;
        for (place, profile) in roster.profiles.iter().enumerate() {
            if computed && let Some(candidates) = survarium::v026g::bot_skills::profile_movement_candidates(profile, 0) {
                bots.set_speed_candidates(place, candidates);
            }
        }
    }
    // The other team's place: dead from the start, no tickets to come back with.
    game.tickets = Some([u32::MAX / 2, 0]);
    game.bodies[1].alive = false;
    game.placed_start = Some(PlacedStart { place: 0, at: start, walk_ms: 0 });
    game.spawn(0);
    game.bodies[0].yaw = (goal.x - start.x).atan2(goal.z - start.z);
    bots.entered(0, true, 0);
    if let Some(bot) = bots.bots[0].as_mut() {
        bot.bench_pin = Some(goal);
    }
    let world_ms = super::settings::bench_world_ms(bench_settings());
    let refresh = crate::v026g::Timing::default().refresh;
    let (mut clock, mut next_world) = (0u32, world_ms);
    let mut out = Outcome::default();
    let mut end_at = limit_ms;
    while clock < end_at {
        clock += 5;
        while game.tick + TICK_MS <= clock {
            for bot in bots.bots.iter().flatten() {
                game.kits[bot.place] = bot.reflex.arms();
            }
            game.step();
            let body = &game.bodies[0];
            if !body.alive {
                out.died = true;
                out.first.get_or_insert_with(|| format!("{clock} died"));
                end_at = clock;
            }
            out.passed |= near(body.feet, p);
            if out.reached_ms.is_none() && flat(body.feet, goal) <= REACH_M && (body.feet.y - goal.y).abs() <= REACH_Y_M {
                out.reached_ms = Some(game.tick);
                // The notes of the last moments still come in.
                end_at = end_at.min(clock + 1_000);
            }
        }
        if clock >= next_world {
            let period = crate::v026g::game::world_period(&bots, refresh, std::time::Duration::from_millis(u64::from(world_ms)));
            next_world = clock + u32::try_from(period.as_millis()).unwrap_or(u32::MAX);
            let world_tick = (clock / TICK_MS * TICK_MS).saturating_sub(300);
            bots.received_damage(game.received_at(world_tick));
            for (place, hurt) in game.hurts_at(world_tick).unwrap_or_default().into_iter().enumerate() {
                bots.hurt(place, hurt);
                bots.hit_kind(place, world_tick);
            }
            for (place, hits) in game.hits_at(world_tick).unwrap_or_default().into_iter().enumerate() {
                bots.hits(place, &hits);
            }
            for (place, saved) in game.staminas_at(world_tick).unwrap_or_default().into_iter().enumerate() {
                if let Some(saved) = saved {
                    bots.stamina(place, saved);
                }
            }
            if let Some((_, fixes, health, facts)) = game.world_at(world_tick) {
                for (place, (fix, health)) in fixes.into_iter().zip(health).enumerate() {
                    for note in bots.fixed(place, fix, Some(health)) {
                        if note.place == 0 {
                            out.note(&note.text, clock, a, p);
                        }
                    }
                }
                for note in bots.facts(facts) {
                    if note.place == 0 {
                        out.note(&note.text, clock, a, p);
                    }
                }
            }
        }
        let (made, notes) = bots.poll(Clock {
            clock,
            time_left_s: (limit_ms.saturating_sub(clock)) as f32 / 1000.0,
            total_s: limit_ms as f32 / 1000.0,
            playing: true,
        });
        for note in notes {
            if note.place == 0 {
                out.note(&note.text, clock, a, p);
            }
        }
        for made in made {
            let r = made.input.rotation;
            let turn = (half_value(u16::from_le_bytes([r[0], r[1]])), half_value(u16::from_le_bytes([r[2], r[3]])));
            let bot = bots.bots[made.place].as_ref();
            let target = bot.and_then(|bot| bot.reflex.target());
            let gun = bot.map_or(Arms::fallback().primary.0, |bot| bot.reflex.held_gun());
            game.inputs[made.place].push_back((made.time, made.input.actions, turn, target, gun));
        }
    }
    out.end = game.bodies[0].feet;
    out
}

#[test]
#[ignore]
fn spot_suite_run() {
    let (Ok(starts), Ok(out_path)) = (std::env::var("SRP_SPOT_SUITE_STARTS"), std::env::var("SRP_SPOT_SUITE_OUT")) else {
        eprintln!("skipping: SRP_SPOT_SUITE_STARTS / SRP_SPOT_SUITE_OUT not set");
        return;
    };
    setup();
    let only: Option<Vec<String>> = std::env::var("SRP_SPOT_SUITE_ONLY").ok().map(|s| s.split(',').map(|x| x.trim().to_owned()).collect());
    let threads: usize = std::env::var("SRP_SPOT_SUITE_THREADS").ok().and_then(|v| v.parse().ok()).unwrap_or(2).max(1);
    let limit_ms: u32 = std::env::var("SRP_SPOT_SUITE_SECONDS").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(60) * 1000;
    let (_, rows) = tsv_rows(&starts);
    let rows: Vec<_> = rows.into_iter().filter(|r| only.as_ref().is_none_or(|o| o.contains(&r["spot"]))).collect();
    let mut projects: Vec<String> = rows.iter().map(|r| r["project"].clone()).collect();
    projects.sort();
    projects.dedup();
    let out = Mutex::new(std::fs::File::create(&out_path).unwrap_or_else(|e| panic!("{out_path}: {e}")));
    writeln!(
        out.lock().unwrap(),
        "spot\tproject\tstart\tvia\tlink_kind\treached\treach_ms\tpassed\terrors\tstuck\tjump\tdead\tfall\tlip\tnoway\tdied\ttimeout\tend_x\tend_y\tend_z\tfirst_error"
    )
    .unwrap();
    for project in projects {
        let list: Vec<&HashMap<String, String>> = rows.iter().filter(|r| r["project"] == project).collect();
        let world = world_of(&project);
        let next = Mutex::new(0usize);
        let started = std::time::Instant::now();
        std::thread::scope(|scope| {
            for _ in 0..threads {
                scope.spawn(|| loop {
                    let i = {
                        let mut n = next.lock().unwrap();
                        let i = *n;
                        *n += 1;
                        i
                    };
                    let Some(row) = list.get(i) else { break };
                    let head = format!("{}\t{}\t{}\t{}\t{}", row["spot"], row["project"], row["start"], row["via"], row["link_kind"]);
                    let line = match (v3(row, "sx", "sy", "sz"), v3(row, "gx", "gy", "gz"), v3(row, "px", "py", "pz")) {
                        (Some(s), Some(g), Some(p)) => {
                            let a = v3(row, "ax", "ay", "az").unwrap_or(p);
                            let o = walk(&world, &project, s, g, a, p, limit_ms, i as u32);
                            let timeout = o.reached_ms.is_none() && !o.died;
                            format!(
                                "{head}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{:.2}\t{:.2}\t{:.2}\t{}",
                                u8::from(o.reached_ms.is_some()),
                                o.reached_ms.map_or("-".to_owned(), |t| t.to_string()),
                                u8::from(o.passed),
                                o.errors(timeout),
                                o.stuck,
                                o.jump,
                                o.dead,
                                o.fall,
                                o.lip,
                                o.noway,
                                u8::from(o.died),
                                u8::from(timeout),
                                o.end.x,
                                o.end.y,
                                o.end.z,
                                o.first.clone().unwrap_or_else(|| if timeout { "timeout".into() } else { "-".into() })
                            )
                        }
                        _ => format!("{head}\t0\t-\t0\t1\t0\t0\t0\t0\t0\t0\t0\t0\tnan\tnan\tnan\tno grid way through the spot"),
                    };
                    writeln!(out.lock().unwrap(), "{line}").unwrap();
                });
            }
        });
        eprintln!("{project}: {} starts in {:.0} s", list.len(), started.elapsed().as_secs_f32());
    }
}
