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
//!   link's landing, or the spot's centre), its take-off A (the link's start); the goal G: a floor on P's own top 1.5-4 m from
//!   P, furthest from A (from P when A is P), that the server's `off_lip` leaves where it is (`via` top); else one 4-10 m of
//!   grid way past P (`via` past); the starts: floors 10-30 m from P (12 bearings x 4 distances) whose route to G by the
//!   server's own planner (`path::plan`) passes A then P, the three most direct ones of different bearings. With no such
//!   start for any of those goals, G = P when `off_lip` leaves P where it is (`via` goal-at-spot). Every goal is one the bot's
//!   planner takes from that start: the test behind its "no way ... the goal's floor refused now" (`Tactics::allowed`: no zone
//!   on the floor, not the other side's spawn area for the side of the spawn nearest the start), with the map's zones taken
//!   as always on (the plan has no match clock: a switching anomaly's floor is no goal). With no routed start still, the
//!   column `why` says which: `no-floor-at-P`, `no-floor-at-A`, or `no-route-through-A-then-P` (`-` for a spot with starts);
//!   a spot with a floor at A gets ONE start on A's floor (`via` at-A), its goal the first of the goals above the planner
//!   reaches from A (else the first): it passes when the body gets off and reaches G with 0 errors. Without a floor at A (or
//!   at P) the spot's one row is `via` no-route. Made once and kept, so every commit walks the same starts; the run still
//!   reads a starts file without the `why` column (`spot-starts-v1.tsv`).
//!   ANY WAY (`SRP_SPOT_SUITE_ANY_WAY=<file: one spot a line, or S001,S002>`): the named spots are the ones no player crosses
//!   from A to P in the recordings (the human crossings list) and no planner route takes. For them the route need not pass
//!   A, then P: the goal is one past P (P's own floor, else the nearest floor within 4 m), the starts are the candidates on
//!   A's side of the spot (nearer A than the goal) the planner has a route from, by any way (`via` any-way). The spot passes
//!   when every start reaches the goal with 0 errors.
//! - `spot_suite_run`: `SRP_SPOT_SUITE_STARTS=<starts tsv>`, `SRP_SPOT_SUITE_OUT=<results tsv>`, optional
//!   `SRP_SPOT_SUITE_ONLY=S001,S017` (or a file of one spot a line: the per-PR gate on a spot list; each start keeps the
//!   seed it has in the full run, its row's place among its map's rows of the whole starts file, so a subset's rows are the
//!   full run's rows of those spots), `SRP_SPOT_SUITE_THREADS=<n>` (default 2; each start runs on a thread of its own, so no
//!   per-thread cache of the planner carries from one start to the next and the rows do not depend on the threads' timing),
//!   `SRP_SPOT_SUITE_SECONDS` (default 60),
//!   `SRP_SPOT_SUITE_NOTES=<notes tsv>` (default the results' path with `.notes.tsv` appended), `SRP_SPOT_SUITE_TRACE` (any
//!   value: the per-tick trace below). One line a start: whether the bot reached G, passed P, and its ERRORS, read from its
//!   own notes:
//!   - `stuck`: "is stuck at";
//!   - `jump`: "nav: route jump <outcome>: ..." (`nav_log::jump_line`) whose outcome word is a miss (`short`, `not down`,
//!     `gave up`: `nav_log::JumpOutcome::missed`), or a word the log does not write (`jump-unread` in the notes: a changed
//!     note fails loudly instead of passing);
//!   - `dead`: "dead link on map";
//!   - `fall`: "nav: fell ... (unplanned)";
//!   - `lip`: "nav: lip guard braked at" a planned take-off: within 2 m across of the take-off of a drop or jump link of the
//!     route's leg the bot is on or the next, or, at a drop spot (P 0.8 m or more under A), within 2 m of A. Any other brake is
//!     `lip-other` in the notes and no error;
//!   - `noway`: "no way" to its goal;
//!   - `died`: the body died;
//!   - `timeout`: not at G after the limit.
//!
//!   Every note of the bot goes to the notes TSV (`spot`, `start`, `ms`, `kind`: one of the above, `lip-other`, `jump-unread`
//!   or `other` for a note that is no error; `at_spot`: 1 when the note's point (its own, else the body's feet) is within
//!   2.5 m of A or P; `motor`: the route's leg and next link (of the route seen at the poll before when the note's own decision dropped
//!   it) and what the motor was doing with its link, `Motor::bench_link_text`;
//!   `text`: the full note). The results count the errors at the spot and on the way (`errors_at_spot`, `errors_on_way`).
//!
//! A spot passes when every start reaches G with 0 errors. A SETUP error of the suite, not of the spot (`setup` 1, left out of
//! the verdict): a start whose bot walked to another goal than G (the server's `off_lip` moved it: the `resolved` columns), or
//! whose first error is the planner refusing G's floor ("the goal's floor refused now").
//!
//! TRACE (`SRP_SPOT_SUITE_TRACE`, for a replay of a start on the game's rig): `<SRP_SPOT_SUITE_OUT>.trace.tsv`, one row per
//! stand-in tick of each start from its first tick to its end: `spot`, `start`, `ms`, the body's feet after the tick (map frame),
//! its held `keys` (hex), `jump_press` (1 on the tick a flight starts), `in_flight`, `on_ground` (no flight, the kept controller
//! not airborne), `sprint`, `crouch` (the keys), and the walk the client makes of the tick's keys: `walk_x`, `walk_z` (m over
//! the tick, map frame: the blended local velocity at the client's step cap turned by the moving yaw, what the game's own
//! controller gets as its walk), `yaw` (the body's), `sprint_state` (the client's). Unset, nothing of it is made.
//!
//! STANDIN-FIDELITY (a tag on a failing start, the start still fails): fix-move found the stand-in body wrong at some drops and
//! rims (a sloped rim kept as one flat lip cell, a fixed velocity through the fall where the game's keys keep steering, a snap
//! to a lip node up to 0.36 m away). A failing start of a drop spot (P lower than A by `DROP_M`) is tagged (`standin`: how the
//! bot pressed) when the bot pressed the drop at A (`press`: a route jump down from A; stuck at A with its route's next point at
//! P; or the body went over A's edge with no "unplanned" fall from A, a planned drop), the body did not come down on P (the
//! `over` and `land` columns: where it went over and came down), and no error came before the press.

use std::collections::{BinaryHeap, HashMap};
use std::io::Write;
use std::sync::Mutex;

use mapdata::{NodeId, WalkGrid};

use super::*;
use super::bench::{bench_client_sway, bench_equipment, bench_open_sides, bench_roster, bench_throws, bench_tuning, bench_wall_clearance};
use super::settings::bench_settings;
use crate::v026g::bots::nav_log::JumpOutcome;
use crate::v026g::bots::path::{DropTruth, Leap, Route, Waypoint};
use crate::v026g::bots::tactics::Tactics;

/// The starts: this far from P across the ground, m.
const START_MIN_M: f32 = 10.0;
const START_MAX_M: f32 = 30.0;
/// The goal: on P's own top (within this in height), this far past P across the ground, m, and off any lip (the server's
/// `path::off_lip` leaves it where it is: a goal on or near a lip is moved, possibly down off the top, fix-move's map 91 case).
const GOAL_TOP_Y_M: f32 = 0.3;
const GOAL_MIN_M: f32 = 1.5;
const GOAL_MAX_M: f32 = 4.0;
/// With no such floor: one this much grid way past P (any height), still off any lip.
const GOAL_FAR_MIN_M: f32 = 4.0;
const GOAL_FAR_MAX_M: f32 = 10.0;
/// The match time the plan asks the planner's floor test at, ms: its zones have no clock in the plan (always on), so any.
const PLAN_MS: u32 = 0;
/// The bot's resolved goal (its motor's, after every override) farther than this from G is a SETUP error, m.
const RESOLVED_M: f32 = 0.3;
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
/// A lip guard brake counts when within this across of a planned take-off (or of A at a drop spot), m.
const LIP_NEAR_M: f32 = 2.0;
/// [lip rule] The route's legs looked at for a planned link at a brake: the leg the bot is on and the next.
const LIP_LEGS: usize = 2;
/// A note is AT the spot when its point is within this of A or P (straight line), m; else on the way.
const AT_SPOT_M: f32 = 2.5;
/// [standin-fidelity] A spot whose landing P is this much lower than its take-off A is a drop (the client's step is 0.80 m).
const DROP_M: f32 = 0.8;
/// [standin-fidelity] The body is at the take-off within `LIP_NEAR_M` across the ground and this in height, m.
const EDGE_Y_M: f32 = 0.6;
/// [standin-fidelity] The body went over the edge when it was this much under A within this long after it was last at A, ms.
const OVER_MS: u32 = 3_000;
/// [standin-fidelity] A falling body came down when it sank less than this in one tick, m, or after `OVER_MS` of falling.
const STILL_M: f32 = 0.03;
/// [standin-fidelity] The body came down on the landing within this of P across the ground and this in height, m.
const LAND_M: f32 = 2.0;
const LAND_Y_M: f32 = 1.0;
/// [standin-fidelity] An error note this long before the press is the bot's own (on the approach), not the drop's, ms.
const NOTE_LAG_MS: u32 = 2_000;

fn flat(a: Vec3, b: Vec3) -> f32 {
    ((a.x - b.x).powi(2) + (a.z - b.z).powi(2)).sqrt()
}

fn dist(a: Vec3, b: Vec3) -> f32 {
    (flat(a, b).powi(2) + (a.y - b.y).powi(2)).sqrt()
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
        let data = super::real_maps::game_data();
        eprintln!("game data: {}", data.origin);
        super::standin::start_bench_run();
    });
}

/// The map's world as the bench builds it (from the shared grid cache when `SRP_BENCH_GRID_CACHE` is set).
fn world_of(project: &str) -> Arc<World> {
    let dir = super::real_maps::dir().expect("SRP_MAPDATA_DIR");
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

/// The floors in the box around P, with their links inside it.
struct Local {
    nodes: Vec<NodeId>,
    index: HashMap<NodeId, usize>,
    fwd: Vec<Vec<usize>>,
}

impl Local {
    fn around(grid: &WalkGrid, p: Vec3) -> Self {
        let nodes: Vec<NodeId> = grid.floors_in([p.x - BOX_M, p.z - BOX_M, p.x + BOX_M, p.z + BOX_M]).collect();
        let index: HashMap<NodeId, usize> = nodes.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        let mut fwd = vec![Vec::new(); nodes.len()];
        for (i, &n) in nodes.iter().enumerate() {
            for (m, _) in grid.links(n) {
                if let Some(&j) = index.get(&m) {
                    fwd[i].push(j);
                }
            }
        }
        Self { nodes, index, fwd }
    }

    /// Dijkstra from `from` over the links, cost flat + rise, up to `limit_m`: (cost mm, the node before).
    fn dijkstra(&self, grid: &WalkGrid, from: usize, limit_m: f32) -> Vec<Option<(u32, usize)>> {
        let mut best: Vec<Option<(u32, usize)>> = vec![None; self.nodes.len()];
        best[from] = Some((0, from));
        let mut heap = BinaryHeap::from([(std::cmp::Reverse(0u32), from)]);
        let limit = (limit_m * 1000.0) as u32;
        while let Some((std::cmp::Reverse(d), i)) = heap.pop() {
            if best[i].is_some_and(|(b, _)| d > b) {
                continue;
            }
            let p = grid.position(self.nodes[i]);
            for &j in &self.fwd[i] {
                let q = grid.position(self.nodes[j]);
                let nd = d + cost(p, q);
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
    /// The planner's route from it to the goal, m; none for a start at A the planner finds no route from.
    way_m: Option<f32>,
}

/// How a spot's starts were found (the starts' `via`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Via {
    /// The goal on P's own top.
    Top,
    /// The goal 4-10 m of grid way past P.
    Past,
    /// The goal at P itself.
    GoalAtSpot,
    /// No routed start: the one start on A's floor.
    AtA,
    /// A spot no player crosses from A to P: starts on A's side, a goal past P, by any way ([`Rule::AnyWay`]).
    AnyWay,
}

impl Via {
    fn word(self) -> &'static str {
        match self {
            Self::Top => "top",
            Self::Past => "past",
            Self::GoalAtSpot => "goal-at-spot",
            Self::AtA => "at-A",
            Self::AnyWay => "any-way",
        }
    }
}

/// What a spot's starts must walk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Rule {
    /// The planner's route passes A, then P (the link the spot names).
    ThroughTheLink,
    /// Any way from A's side to a goal past P (`SRP_SPOT_SUITE_ANY_WAY`).
    AnyWay,
}

/// The `why` of an any-way spot's starts (the report's tag).
const ANY_WAY_WHY: &str = "no-human: any-way";

/// How near a floor an any-way spot's P may have when P itself has none, m.
const ANY_WAY_FLOOR_M: f32 = 4.0;

/// The spots of `SRP_SPOT_SUITE_ANY_WAY`: a file of one spot a line, or a comma list; none when unset.
fn any_way_spots() -> Vec<String> {
    spot_list("SRP_SPOT_SUITE_ANY_WAY").unwrap_or_default()
}

/// The spots the environment variable `name` lists: a file of one spot a line (`#` a comment), or a comma list; none when unset.
fn spot_list(name: &str) -> Option<Vec<String>> {
    let value = std::env::var(name).ok().filter(|v| !v.trim().is_empty())?;
    let text = std::fs::read_to_string(&value).unwrap_or_else(|_| value.replace(',', "\n"));
    Some(text.lines().map(str::trim).filter(|s| !s.is_empty() && !s.starts_with('#')).map(str::to_owned).collect())
}

/// An any-way spot's goals and starts ([`Rule::AnyWay`]): the goal past P (the furthest 4-10 m of grid way on, then its fork,
/// then one on P's top) that the planner takes and routes to from a start on A's side; up to three starts, by any way.
fn plan_any_way(grid: &WalkGrid, tactics: &Tactics, p: Vec3, a: Vec3) -> Option<Plan> {
    let local = Local::around(grid, p);
    let pn = floor_of(grid, p).or_else(|| grid.locate(p, ANY_WAY_FLOOR_M)).and_then(|n| local.index.get(&n).copied())?;
    let p_at = grid.position(local.nodes[pn]);
    let past = local.dijkstra(grid, pn, GOAL_FAR_MAX_M);
    let stays = |i: usize| {
        let (node, at) = (local.nodes[i], grid.position(local.nodes[i]));
        grid.locate(at, 1.0) == Some(node)
            && crate::v026g::bots::path::off_lip(grid, at).is_none()
            && tactics.hazards.allowed(node, PLAN_MS)
    };
    let furthest = |pick: &dyn Fn(usize, u32) -> bool| {
        past.iter()
            .enumerate()
            .filter_map(|(i, b)| b.filter(|&(d, _)| pick(i, d) && stays(i)).map(|_| i))
            .max_by(|&i, &j| flat(grid.position(local.nodes[i]), a).total_cmp(&flat(grid.position(local.nodes[j]), a)))
    };
    let far = furthest(&|_, d| (GOAL_FAR_MIN_M * 1000.0) as u32 <= d);
    let far2 = far.and_then(|f| {
        let first = grid.position(local.nodes[f]);
        furthest(&|i, d| (GOAL_FAR_MIN_M * 1000.0) as u32 <= d && flat(grid.position(local.nodes[i]), first) >= 4.0)
    });
    let on_top = furthest(&|i, _| {
        let q = grid.position(local.nodes[i]);
        (q.y - p_at.y).abs() <= GOAL_TOP_Y_M && (GOAL_MIN_M..=GOAL_MAX_M).contains(&flat(q, p_at))
    });
    let candidates = start_candidates(grid, p_at);
    for g in [far, far2, on_top].into_iter().flatten() {
        let goal = local.nodes[g];
        let goal_at = grid.position(goal);
        let mut found: Vec<(f32, f32, NodeId, f32)> = Vec::new();
        for &s in &candidates {
            let at = grid.position(s);
            // Before the spot: on A's side of it, nearer A than the goal.
            if flat(at, a) >= flat(at, goal_at) || !planner_takes(tactics, goal, at) {
                continue;
            }
            let Some(route) = crate::v026g::bots::path::plan(grid, s, goal) else { continue };
            let bearing = (at.z - p_at.z).atan2(at.x - p_at.x).to_degrees();
            found.push((route.length() / (flat(at, goal_at) + 1.0), bearing, s, route.length()));
        }
        if found.is_empty() {
            continue;
        }
        return Some(Plan { goal: goal_at, starts: choose_starts(grid, found), via: Via::AnyWay });
    }
    None
}

/// The candidate starts round P: the floor nearest P's height near each of 12 bearings x 4 distances from P, one a body can
/// be at (reached from a spawn and back).
fn start_candidates(grid: &WalkGrid, p_at: Vec3) -> Vec<NodeId> {
    let mut candidates: Vec<NodeId> = Vec::new();
    for k in 0..12 {
        let angle = k as f32 * std::f32::consts::TAU / 12.0;
        for d in [12.0, 18.0, 24.0, 30.0f32] {
            let c = Vec3::new(p_at.x + d * angle.cos(), p_at.y, p_at.z + d * angle.sin());
            if let Some(n) = grid
                .floors_in([c.x - 1.5, c.z - 1.5, c.x + 1.5, c.z + 1.5])
                .filter(|&n| grid.reached(n) && grid.returns(n) && (START_MIN_M..=START_MAX_M).contains(&flat(grid.position(n), p_at)))
                .min_by(|&x, &y| (grid.position(x).y - c.y).abs().total_cmp(&(grid.position(y).y - c.y).abs()))
                && !candidates.contains(&n)
            {
                candidates.push(n);
            }
        }
    }
    candidates
}

/// Up to three of `found` (directness, bearing, start, route m), the most direct first, of bearings apart and 3 m apart.
fn choose_starts(grid: &WalkGrid, mut found: Vec<(f32, f32, NodeId, f32)>) -> Vec<Start> {
    found.sort_by(|x, y| x.0.total_cmp(&y.0));
    let mut chosen: Vec<(f32, f32, NodeId, f32)> = Vec::new();
    for apart in [BEARING_APART_DEG, 15.0, 0.0] {
        for f in &found {
            if chosen.len() >= 3 {
                break;
            }
            let off = |c: &(f32, f32, NodeId, f32)| {
                let d = (c.1 - f.1).rem_euclid(360.0);
                d.min(360.0 - d)
            };
            let far = |c: &(f32, f32, NodeId, f32)| flat(grid.position(c.2), grid.position(f.2)) >= 3.0;
            if chosen.iter().all(|c| c.2 != f.2 && off(c) >= apart && far(c)) {
                chosen.push(*f);
            }
        }
    }
    chosen.iter().map(|c| Start { at: grid.position(c.2), way_m: Some(c.3) }).collect()
}

/// Why no start of a spot has the planner's route through it (the starts' `why`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NoRoute {
    /// P has no floor ([`floor_of`]).
    FloorAtP,
    /// P has one, A (a point of its own) has none.
    FloorAtA,
    /// Both have floors; no route of the planner from a candidate start passes A, then P, to a goal it takes.
    ThroughAThenP,
}

impl NoRoute {
    fn word(self) -> &'static str {
        match self {
            Self::FloorAtP => "no-floor-at-P",
            Self::FloorAtA => "no-floor-at-A",
            Self::ThroughAThenP => "no-route-through-A-then-P",
        }
    }
}

/// A spot's goal and starts.
struct Plan {
    goal: Vec3,
    starts: Vec<Start>,
    via: Via,
}

/// What the plan found for a spot.
enum Planned {
    /// Starts whose planner's route passes A, then P.
    Routed(Plan),
    /// None: why, and the start at A when A has a floor and a goal past P exists ([`Via::AtA`]).
    Unrouted { why: NoRoute, at_a: Option<Plan> },
}

/// Whether the bot's planner takes a goal on `goal` from a body that began its life at `from`: the test behind its "no way
/// ... the goal's floor refused now" (`drive::no_way_line`, `Tactics::allowed` with the bot's side, the side of the spawn
/// nearest where its first life began, and no objective: the pinned goal's intent has none).
fn planner_takes(tactics: &Tactics, goal: NodeId, from: Vec3) -> bool {
    tactics.allowed(goal, PLAN_MS, tactics.side_at(from), false)
}

/// The goal and starts of one spot.
fn plan_spot(grid: &WalkGrid, tactics: &Tactics, p: Vec3, a: Vec3) -> Planned {
    let local = Local::around(grid, p);
    let Some(pn) = floor_of(grid, p).and_then(|n| local.index.get(&n).copied()) else {
        return Planned::Unrouted { why: NoRoute::FloorAtP, at_a: None };
    };
    let p_at = grid.position(local.nodes[pn]);
    let a_distinct = flat(a, p_at) >= 0.5 || (a.y - p_at.y).abs() >= 0.5;
    let past = local.dijkstra(grid, pn, GOAL_FAR_MAX_M);
    let away_from = if a_distinct { a } else { p_at };
    // Where the server leaves a goal: its own floor (`locate`), not moved off a lip, and no zone on it (the planner's floor
    // test's part that is the same for every side, [`planner_takes`]).
    let stays = |i: usize| {
        let (node, at) = (local.nodes[i], grid.position(local.nodes[i]));
        grid.locate(at, 1.0) == Some(node)
            && crate::v026g::bots::path::off_lip(grid, at).is_none()
            && tactics.hazards.allowed(node, PLAN_MS)
    };
    let furthest = |pick: &dyn Fn(usize, u32) -> bool| {
        past.iter()
            .enumerate()
            .filter_map(|(i, b)| b.filter(|&(d, _)| pick(i, d) && stays(i)).map(|_| i))
            .max_by(|&i, &j| flat(grid.position(local.nodes[i]), away_from).total_cmp(&flat(grid.position(local.nodes[j]), away_from)))
    };
    let on_top = furthest(&|i, _| {
        let q = grid.position(local.nodes[i]);
        (q.y - p_at.y).abs() <= GOAL_TOP_Y_M && (GOAL_MIN_M..=GOAL_MAX_M).contains(&flat(q, p_at))
    });
    let far = furthest(&|_, d| (GOAL_FAR_MIN_M * 1000.0) as u32 <= d);
    // A second goal past P, away from the first (the way on may fork).
    let far2 = far.and_then(|f| {
        let first = grid.position(local.nodes[f]);
        furthest(&|i, d| (GOAL_FAR_MIN_M * 1000.0) as u32 <= d && flat(grid.position(local.nodes[i]), first) >= 4.0)
    });
    let at_spot = stays(pn).then_some(pn);
    let candidates = start_candidates(grid, p_at);
    let goals = [(on_top, Via::Top), (far, Via::Past), (far2, Via::Past), (at_spot, Via::GoalAtSpot)];
    for (g, via) in goals {
        let Some(g) = g else { continue };
        let goal = local.nodes[g];
        // (directness, bearing, start, route m): the server's own route (`path::plan`) passes A, then P, to a goal the bot's
        // planner takes from that start.
        let mut found: Vec<(f32, f32, NodeId, f32)> = Vec::new();
        for &s in &candidates {
            if !planner_takes(tactics, goal, grid.position(s)) {
                continue;
            }
            let Some(route) = crate::v026g::bots::path::plan(grid, s, goal) else { continue };
            let points: Vec<Vec3> = route.points().iter().map(|w| w.at).collect();
            let hit = |q: Vec3| (0..points.len()).find(|&i| passes(&points, i, q));
            let ok = match (a_distinct, hit(p_at)) {
                (_, None) => false,
                (false, Some(_)) => true,
                (true, Some(pi)) => hit(a).is_some_and(|ai| ai <= pi),
            };
            if ok {
                let at = grid.position(s);
                let bearing = (at.z - p_at.z).atan2(at.x - p_at.x).to_degrees();
                found.push((route.length() / (flat(at, grid.position(goal)) + 1.0), bearing, s, route.length()));
            }
        }
        if found.is_empty() {
            continue;
        }
        return Planned::Routed(Plan { goal: grid.position(goal), starts: choose_starts(grid, found), via });
    }
    // No routed start: the body placed on A's floor, its goal the first one past P (the spot itself only when A is a point of
    // its own) that the planner takes from there, one it has a route to first.
    let a_floor = if a_distinct { floor_of(grid, a) } else { Some(local.nodes[pn]) };
    let Some(a_floor) = a_floor else {
        return Planned::Unrouted { why: NoRoute::FloorAtA, at_a: None };
    };
    let a_at = grid.position(a_floor);
    let taken: Vec<NodeId> = goals
        .iter()
        .filter(|(_, via)| *via != Via::GoalAtSpot || a_distinct)
        .filter_map(|&(g, _)| g.map(|g| local.nodes[g]))
        .filter(|&g| planner_takes(tactics, g, a_at))
        .collect();
    let routes: Vec<(NodeId, Option<Route>)> =
        taken.iter().map(|&g| (g, crate::v026g::bots::path::plan(grid, a_floor, g))).collect();
    let pick = routes.iter().find(|(_, r)| r.is_some()).or(routes.first());
    let at_a = pick.map(|(g, route)| Plan {
        goal: grid.position(*g),
        starts: vec![Start { at: a_at, way_m: route.as_ref().map(Route::length) }],
        via: Via::AtA,
    });
    Planned::Unrouted { why: NoRoute::ThroughAThenP, at_a }
}

/// Whether the route's leg from point `i` to the next (or point `i` itself, for the last) passes `q`: within PASS_M across
/// the ground of the leg, at the leg's height there within PASS_Y_M.
fn passes(points: &[Vec3], i: usize, q: Vec3) -> bool {
    let a = points[i];
    let Some(&b) = points.get(i + 1) else { return near(a, q) };
    let (dx, dz) = (b.x - a.x, b.z - a.z);
    let len2 = dx * dx + dz * dz;
    let t = if len2 < 1e-6 { 0.0 } else { (((q.x - a.x) * dx + (q.z - a.z) * dz) / len2).clamp(0.0, 1.0) };
    let on = Vec3::new(a.x + t * dx, a.y + t * (b.y - a.y), a.z + t * dz);
    // A link's leg (a jump, a drop) changes height at its end: either end's height counts.
    flat(on, q) <= PASS_M && [on.y, a.y, b.y].iter().any(|y| (y - q.y).abs() <= PASS_Y_M)
}

#[test]
#[ignore]
fn spot_suite_plan() {
    let (Ok(spots), Ok(out)) = (std::env::var("SRP_SPOT_SUITE_SPOTS"), std::env::var("SRP_SPOT_SUITE_STARTS")) else {
        eprintln!("skipping: SRP_SPOT_SUITE_SPOTS / SRP_SPOT_SUITE_STARTS not set");
        return;
    };
    setup();
    let any_way = any_way_spots();
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
        "spot\tproject\tstart\tvia\tlink_kind\tsx\tsy\tsz\tgx\tgy\tgz\tpx\tpy\tpz\tax\tay\taz\tway_m\twhy".to_owned(),
    ];
    for (project, list) in by_project {
        let world = world_of(&project);
        let grid = &world.grid;
        // The bot's own tactics' floor test (as `Bots::map_ready` builds them; no anomaly clock attached: every zone on).
        let tactics = Tactics::new(world.clone(), super::hazards::Marking::of(&bench_tuning().hazard));
        for row in list {
            let centre = v3(row, "x", "y", "z").unwrap();
            let p = v3(row, "to_x", "to_y", "to_z").unwrap_or(centre);
            let a = v3(row, "from_x", "from_y", "from_z").unwrap_or(p);
            let kind = &row["link_kind"];
            let spot = &row["spot"];
            let fmt = |v: Vec3| format!("{:.2}\t{:.2}\t{:.2}", v.x, v.y, v.z);
            let push = |lines: &mut Vec<String>, plan: &Plan, why: &str| {
                for (k, s) in plan.starts.iter().enumerate() {
                    let way = s.way_m.map_or("-".to_owned(), |m| format!("{m:.1}"));
                    lines.push(format!(
                        "{spot}\t{project}\t{k}\t{}\t{kind}\t{}\t{}\t{}\t{}\t{way}\t{why}",
                        plan.via.word(),
                        fmt(s.at),
                        fmt(plan.goal),
                        fmt(p),
                        fmt(a)
                    ));
                }
            };
            let rule = if any_way.contains(spot) { Rule::AnyWay } else { Rule::ThroughTheLink };
            if rule == Rule::AnyWay {
                let said = match plan_any_way(grid, &tactics, p, a) {
                    Some(plan) => {
                        push(&mut lines, &plan, ANY_WAY_WHY);
                        plan.via.word()
                    }
                    None => {
                        lines.push(format!(
                            "{spot}\t{project}\t-\tno-route\t{kind}\tnan\tnan\tnan\tnan\tnan\tnan\t{}\t{}\t-\t{ANY_WAY_WHY}",
                            fmt(p),
                            fmt(a)
                        ));
                        "no-route (any way: no start)"
                    }
                };
                eprintln!("{spot} {project} planned: {said}");
                continue;
            }
            let said = match plan_spot(grid, &tactics, p, a) {
                Planned::Routed(plan) => {
                    push(&mut lines, &plan, "-");
                    plan.via.word()
                }
                Planned::Unrouted { why, at_a: Some(plan) } => {
                    push(&mut lines, &plan, why.word());
                    why.word()
                }
                Planned::Unrouted { why, at_a: None } => {
                    lines.push(format!(
                        "{spot}\t{project}\t-\tno-route\t{kind}\tnan\tnan\tnan\tnan\tnan\tnan\t{}\t{}\t-\t{}",
                        fmt(p),
                        fmt(a),
                        why.word()
                    ));
                    why.word()
                }
            };
            eprintln!("{spot} {project} planned: {said}");
        }
    }
    std::fs::write(&out, lines.join("\n") + "\n").unwrap();
    eprintln!("wrote {} lines to {out}", lines.len() - 1);
}

/// A route jump's outcome that is an error ([`NoteKind::Jump`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum JumpMiss {
    /// A missed jump (`JumpOutcome::missed`: short, not down, gave up).
    Missed(JumpOutcome),
    /// An outcome word the log does not write (the note's format changed): an error, so the change shows.
    Unread,
}

/// Why a "no way" note found none.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NoWayWhy {
    /// The search found no way.
    Search,
    /// The planner refuses the goal's floor ("the goal's floor refused now"): the suite's goal, a SETUP error when first.
    GoalRefused,
}

/// What a note of the bot is, for the results' counts and the notes' `kind`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NoteKind {
    Stuck,
    Jump(JumpMiss),
    Dead,
    Fall,
    /// A lip-guard brake at a planned take-off ([`lip_at_take_off`]).
    Lip,
    /// A lip-guard brake anywhere else: no error.
    LipOther,
    NoWay(NoWayWhy),
    Died,
    Timeout,
    /// Any other note: no error.
    Other,
}

impl NoteKind {
    fn word(self) -> &'static str {
        match self {
            Self::Stuck => "stuck",
            Self::Jump(JumpMiss::Missed(_)) => "jump",
            Self::Jump(JumpMiss::Unread) => "jump-unread",
            Self::Dead => "dead",
            Self::Fall => "fall",
            Self::Lip => "lip",
            Self::LipOther => "lip-other",
            Self::NoWay(_) => "noway",
            Self::Died => "died",
            Self::Timeout => "timeout",
            Self::Other => "other",
        }
    }

    fn error(self) -> bool {
        !matches!(self, Self::LipOther | Self::Other)
    }
}

/// One note of the bot (or its death, or the time running out), as the notes TSV has it.
#[derive(Clone, Debug)]
struct NoteRow {
    ms: u32,
    kind: NoteKind,
    /// Its point within `AT_SPOT_M` of A or P ([`at_spot`]).
    at_spot: bool,
    /// The route's leg and the motor's link state when it came ([`At::motor`]).
    motor: String,
    text: String,
}

/// The bot when a note came: its body's feet, the take-offs of the drop and jump links of its route's leg and the next
/// ([`link_take_offs`]), and its leg's and motor's state ([`leg_text`], `Motor::bench_link_text`).
struct At {
    feet: Vec3,
    take_offs: Vec<Vec3>,
    motor: String,
}

#[derive(Default, Debug)]
struct Outcome {
    reached_ms: Option<u32>,
    passed: bool,
    died: bool,
    /// Every note, in order, with the death and the time out.
    notes: Vec<NoteRow>,
    end: Vec3,
    /// The goal the bot's motor walked to (after `off_lip` and every override), at its first motor tick.
    resolved: Option<Vec3>,
    /// [standin-fidelity] The bot's own press of the spot's drop: the clock and how it shows (`press_word`).
    press: Option<(u32, Press)>,
    /// [standin-fidelity] A "nav: fell ... (unplanned)" from the take-off: the model did not go over, the body did.
    unplanned_at_edge: bool,
    /// [standin-fidelity] What the stand-in body did at the take-off.
    edge: Edge,
}

/// [standin-fidelity] How the bot showed it pressed the spot's drop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Press {
    /// A route jump down from the take-off ("nav: route jump <word>: from A ..., rise under -`DROP_M`").
    Jump(JumpWord),
    /// Stuck at the take-off with the route's next point at the landing.
    StuckAtEdge,
    /// The body went over the take-off's edge and no "unplanned" fall came from there (a planned drop: the model went too).
    WentOver,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum JumpWord {
    Down,
    NotDown,
    GaveUp,
    Other,
}

impl JumpWord {
    fn of(outcome: Option<JumpOutcome>) -> Self {
        match outcome {
            Some(JumpOutcome::Down) => Self::Down,
            Some(JumpOutcome::NotDown) => Self::NotDown,
            Some(JumpOutcome::GaveUp) => Self::GaveUp,
            Some(JumpOutcome::Up | JumpOutcome::Short) | None => Self::Other,
        }
    }
}

impl Press {
    fn word(self) -> &'static str {
        match self {
            Self::Jump(JumpWord::Down) => "jump-down",
            Self::Jump(JumpWord::NotDown) => "jump-not-down",
            Self::Jump(JumpWord::GaveUp) => "jump-gave-up",
            Self::Jump(JumpWord::Other) => "jump-other",
            Self::StuckAtEdge => "stuck-at-edge",
            Self::WentOver => "went-over",
        }
    }
}

/// [standin-fidelity] The stand-in body at the spot's take-off, tick by tick.
#[derive(Clone, Copy, Debug, Default)]
enum Edge {
    /// Never at the take-off yet.
    #[default]
    Top,
    /// At the take-off last at `tick`, there at `at`.
    AtTop { at: Vec3, tick: u32 },
    /// Over the edge (under A by `DROP_M` within `OVER_MS` of the last tick at A), still sinking.
    Falling { over: Vec3, since: u32, prev: Vec3 },
    /// Came down (the first drop only).
    Landed { over: Vec3, since: u32, landing: Vec3 },
}

impl Edge {
    fn step(self, feet: Vec3, tick: u32, a: Vec3) -> Self {
        let at_a = flat(feet, a) <= LIP_NEAR_M && (feet.y - a.y).abs() <= EDGE_Y_M;
        match self {
            Self::Top | Self::AtTop { .. } if at_a => Self::AtTop { at: feet, tick },
            Self::AtTop { at, tick: last } if feet.y < a.y - DROP_M && tick.saturating_sub(last) <= OVER_MS => {
                Self::Falling { over: at, since: tick, prev: feet }
            }
            Self::Falling { over, since, prev } if prev.y - feet.y < STILL_M || tick.saturating_sub(since) > OVER_MS => {
                Self::Landed { over, since, landing: feet }
            }
            Self::Falling { over, since, .. } => Self::Falling { over, since, prev: feet },
            other => other,
        }
    }

    /// Where the body went over the edge, and its tick.
    fn over(self) -> Option<(Vec3, u32)> {
        match self {
            Self::Falling { over, since, .. } | Self::Landed { over, since, .. } => Some((over, since)),
            Self::Top | Self::AtTop { .. } => None,
        }
    }

    fn landing(self) -> Option<Vec3> {
        match self {
            Self::Landed { landing, .. } => Some(landing),
            Self::Falling { prev, .. } => Some(prev),
            Self::Top | Self::AtTop { .. } => None,
        }
    }
}

/// The numbers inside the first "(x, y, z)" or "[x, y, z]" after `key`.
fn point_after(text: &str, key: &str, close: char) -> Option<Vec3> {
    let rest = text.split(key).nth(1)?;
    let n: Vec<f32> = rest.split(close).next()?.split(',').filter_map(|v| v.trim().parse().ok()).collect();
    (n.len() == 3).then(|| Vec3::new(n[0], n[1], n[2]))
}

/// The outcome of a route jump's note from its text after "nav: route jump " (`nav_log::jump_line`: "<word>: from (..) to
/// (..), rise R m, try N, after MS ms (then)"): the word before its first ':', as the log writes it; none for another word.
fn jump_outcome(rest: &str) -> Option<JumpOutcome> {
    let word = rest.split(':').next()?;
    [JumpOutcome::Up, JumpOutcome::Short, JumpOutcome::Down, JumpOutcome::NotDown, JumpOutcome::GaveUp]
        .into_iter()
        .find(|outcome| outcome.word() == word)
}

/// [lip rule] Whether a lip-guard brake at `at` is at a planned take-off: within `LIP_NEAR_M` across of the take-off of a drop
/// or jump link of the route's leg the bot is on or the next (`take_offs`, [`link_take_offs`]), or, at a drop spot (P
/// `DROP_M` or more under A), within it of A.
fn lip_at_take_off(at: Vec3, take_offs: &[Vec3], a: Vec3, p: Vec3) -> bool {
    take_offs.iter().any(|&t| flat(t, at) <= LIP_NEAR_M) || (p.y <= a.y - DROP_M && flat(at, a) <= LIP_NEAR_M)
}

/// Whether a note's point is at the spot: within `AT_SPOT_M` of A or P.
fn at_spot(point: Vec3, a: Vec3, p: Vec3) -> bool {
    dist(point, a) <= AT_SPOT_M || dist(point, p) <= AT_SPOT_M
}

/// The point a note is about: where it was stuck, braked, jumped, fell or searched from; none for a note with no such point.
fn note_point(text: &str) -> Option<Vec3> {
    point_after(text, "is stuck at [", ']')
        .or_else(|| point_after(text, "lip guard braked at (", ')'))
        .or_else(|| point_after(text, "from (", ')'))
}

/// What a leg of a route is ([`leg_kind`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LegKind {
    Walk,
    /// Walked crouched (`Waypoint::low`).
    Low,
    /// A drop of the planner's path (`Route::drop_landings`, `DropTruth::Links`).
    Drop,
    Ledge,
    SprintLedge,
    Gap,
    SprintGap,
}

impl LegKind {
    fn word(self) -> &'static str {
        match self {
            Self::Walk => "walk",
            Self::Low => "low",
            Self::Drop => "drop",
            Self::Ledge => "ledge",
            Self::SprintLedge => "sprint-ledge",
            Self::Gap => "gap",
            Self::SprintGap => "sprint-gap",
        }
    }

    fn link(self) -> bool {
        !matches!(self, Self::Walk | Self::Low)
    }
}

/// The leg into waypoint `k` (`k` at least 1), `drops` the route's drop landings ahead.
fn leg_kind(points: &[Waypoint], k: usize, drops: &[Vec3]) -> LegKind {
    match points[k].jump {
        Some(Leap::Ledge) => LegKind::Ledge,
        Some(Leap::SprintLedge) => LegKind::SprintLedge,
        Some(Leap::Gap(_)) => LegKind::Gap,
        Some(Leap::SprintGap(_)) => LegKind::SprintGap,
        None if drops.contains(&points[k].at) => LegKind::Drop,
        None if points[k].low => LegKind::Low,
        None => LegKind::Walk,
    }
}

/// The route's drop landings from the leg the body was last found on to its end.
fn drops_ahead(route: &Route) -> Vec<Vec3> {
    route.drop_landings(route.walked(), route.length(), DropTruth::Links)
}

/// [lip rule] The take-offs of the drop and jump links of the route's leg the body was last found on and the next.
fn link_take_offs(route: &Route) -> Vec<Vec3> {
    let points = route.points();
    let drops = drops_ahead(route);
    (route.segment() + 1..points.len())
        .take(LIP_LEGS)
        .filter(|&k| leg_kind(points, k, &drops).link())
        .map(|k| points[k - 1].at)
        .collect()
}

/// The notes' `motor` first part: the leg the body is on, and the route's next link and its take-off.
fn leg_text(route: &Route) -> String {
    let points = route.points();
    let drops = drops_ahead(route);
    let k = route.segment() + 1;
    if k >= points.len() {
        return "leg end".to_owned();
    }
    let next = (k..points.len()).find(|&j| leg_kind(points, j, &drops).link()).map_or("none".to_owned(), |j| {
        let t = points[j - 1].at;
        format!("{} from ({:.1}, {:.1}, {:.1})", leg_kind(points, j, &drops).word(), t.x, t.y, t.z)
    });
    let kind = leg_kind(points, k, &drops);
    // [debug trace] A walk leg names its rise over its length, and "climb step" where it rises over the client's step
    // (`mapdata::Body::client().step`): a ledge the grid made a walk shows in the notes.
    let rise = match kind {
        LegKind::Walk | LegKind::Low => {
            let (a, b) = (points[k - 1].at, points[k].at);
            let up = b.y - a.y;
            let climb = if up > mapdata::Body::client().step { ", climb step" } else { "" };
            format!(" ({up:+.2} m over {:.1} m{climb})", a.distance_xz(b))
        }
        _ => String::new(),
    };
    format!("leg {}{rise}, next link {next}", kind.word())
}

/// The bot's route as last seen at a poll ([`RouteSeen::look`]): a note often comes out of the decision that dropped the
/// route (a stuck back-off, a jump's watch ending), so the leg it was on is the one seen before.
#[derive(Default)]
enum RouteSeen {
    /// No route seen yet.
    #[default]
    Never,
    /// At the motor's tick `ms`: its leg text ([`leg_text`]) and its links' take-offs ([`link_take_offs`]).
    At { ms: u32, leg: String, take_offs: Vec<Vec3> },
}

impl RouteSeen {
    /// The bot's route after a poll at `ms`, when it has one.
    fn look(&mut self, bot: &Bot, ms: u32) {
        if let Some((route, _)) = bot.route.as_ref() {
            *self = Self::At { ms, leg: leg_text(route), take_offs: link_take_offs(route) };
        }
    }
}

/// The bot and its body now, for a note at `ms` ([`At`]): the leg of its route now, else of the route last seen (`seen`).
fn at_of(bots: &Bots, game: &Game, seen: &RouteSeen, ms: u32) -> At {
    let feet = game.bodies[0].feet;
    let Some(bot) = bots.bots[0].as_ref() else {
        return At { feet, take_offs: Vec::new(), motor: "no bot".to_owned() };
    };
    let (leg, take_offs) = match (bot.route.as_ref(), seen) {
        (Some((route, _)), _) => (leg_text(route), link_take_offs(route)),
        (None, RouteSeen::At { ms: at, leg, take_offs }) => {
            (format!("no route now; {} ms before: {leg}", ms.saturating_sub(*at)), take_offs.clone())
        }
        (None, RouteSeen::Never) => ("no route".to_owned(), Vec::new()),
    };
    At { feet, take_offs, motor: format!("{leg}; {}", bot.motor.bench_link_text()) }
}

/// A note's text in one TSV cell.
fn cell(text: &str) -> String {
    text.replace(['\t', '\n', '\r'], " ")
}

impl Outcome {
    fn count(&self, pick: impl Fn(NoteKind) -> bool) -> usize {
        self.notes.iter().filter(|n| pick(n.kind)).count()
    }

    fn errors(&self) -> usize {
        self.count(NoteKind::error)
    }

    fn first_error(&self) -> Option<&NoteRow> {
        self.notes.iter().find(|n| n.kind.error())
    }

    fn timed_out(&self) -> bool {
        self.count(|k| k == NoteKind::Timeout) > 0
    }

    /// [standin-fidelity] The press, when this failed start of a drop spot is the stand-in's: the bot pressed the drop
    /// (`Press`), the body did not come down on the landing, and no error came before the press (the approach was the bot's).
    fn standin(&self, leg: &Leg, failed: bool) -> Option<Press> {
        if !failed || leg.p.y >= leg.a.y - DROP_M {
            return None;
        }
        let (pressed_ms, press) =
            self.press.or_else(|| self.edge.over().filter(|_| !self.unplanned_at_edge).map(|(_, since)| (since, Press::WentOver)))?;
        let landed = self.edge.landing().is_some_and(|l| flat(l, leg.p) <= LAND_M && (l.y - leg.p.y).abs() <= LAND_Y_M);
        let before = self.first_error().is_some_and(|f| f.ms + NOTE_LAG_MS < pressed_ms);
        (!landed && !before).then_some(press)
    }

    /// A note of the bot at `clock`: its kind, the standin-fidelity press, and its row.
    fn note(&mut self, text: &str, clock: u32, leg: &Leg, at: At) {
        let (a, p) = (leg.a, leg.p);
        let at_a = |q: Vec3| flat(q, a) <= LIP_NEAR_M && (q.y - a.y).abs() <= EDGE_Y_M;
        let kind = if text.contains("is stuck at") {
            let next = text.split(" next ").nth(1).and_then(|rest| {
                let n: Vec<f32> = rest.split(' ').next()?.split(',').filter_map(|v| v.trim().parse().ok()).collect();
                (n.len() == 3).then(|| Vec3::new(n[0], n[1], n[2]))
            });
            if point_after(text, "is stuck at [", ']').is_some_and(at_a) && next.is_some_and(|n| flat(n, p) <= LAND_M && n.y < a.y - DROP_M) {
                self.press.get_or_insert((clock, Press::StuckAtEdge));
            }
            NoteKind::Stuck
        } else if let Some(rest) = text.split("nav: route jump ").nth(1) {
            let outcome = jump_outcome(rest);
            let rise = rest.split("rise ").nth(1).and_then(|r| r.split(' ').next()).and_then(|r| r.parse::<f32>().ok());
            if point_after(rest, "from (", ')').is_some_and(at_a) && rise.is_some_and(|r| r < -DROP_M) {
                self.press.get_or_insert((clock, Press::Jump(JumpWord::of(outcome))));
            }
            match outcome {
                Some(outcome) if !outcome.missed() => NoteKind::Other,
                Some(outcome) => NoteKind::Jump(JumpMiss::Missed(outcome)),
                None => NoteKind::Jump(JumpMiss::Unread),
            }
        } else if text.contains("dead link on map") {
            NoteKind::Dead
        } else if text.contains("nav: fell ") && text.contains("(unplanned") {
            self.unplanned_at_edge |= point_after(text, "from (", ')').is_some_and(at_a);
            NoteKind::Fall
        } else if text.contains("nav: lip guard braked at (") {
            match point_after(text, "nav: lip guard braked at (", ')') {
                Some(brake) if lip_at_take_off(brake, &at.take_offs, a, p) => NoteKind::Lip,
                _ => NoteKind::LipOther,
            }
        } else if text.contains("no way") && (text.contains("no way from") || text.contains("no way to its goal")) {
            NoteKind::NoWay(if text.contains("the goal's floor refused now") { NoWayWhy::GoalRefused } else { NoWayWhy::Search })
        } else {
            NoteKind::Other
        };
        self.push(clock, kind, note_point(text).unwrap_or(at.feet), leg, at.motor, text.to_owned());
    }

    fn push(&mut self, ms: u32, kind: NoteKind, point: Vec3, leg: &Leg, motor: String, text: String) {
        self.notes.push(NoteRow { ms, kind, at_spot: at_spot(point, leg.a, leg.p), motor, text });
    }
}

/// One start of a spot: where the bot starts, its goal, the take-off and the spot's point.
#[derive(Clone, Copy)]
struct Leg {
    start: Vec3,
    goal: Vec3,
    a: Vec3,
    p: Vec3,
}

/// [trace] The per-tick rows of one start (`SRP_SPOT_SUITE_TRACE`): its `spot` and `start` cells, and the rows so far.
struct TickTrace {
    head: String,
    rows: String,
}

impl TickTrace {
    /// The stand-in body after a tick (`flew`: in a flight before it).
    fn tick(&mut self, body: &super::standin::Body, tick: u32, flew: bool) {
        use std::fmt::Write as _;
        let flying = body.flight.is_some();
        let on_ground = !flying && !body.kept.as_ref().is_some_and(|kept| kept.airborne());
        let f = body.feet;
        let _ = writeln!(
            self.rows,
            "{}\t{tick}\t{:.4}\t{:.4}\t{:.4}\t{:x}\t{}\t{}\t{}\t{}\t{}\t{:.5}\t{:.5}\t{:.4}\t{}",
            self.head,
            f.x,
            f.y,
            f.z,
            body.keys,
            u8::from(flying && !flew),
            u8::from(flying),
            u8::from(on_ground),
            u8::from(body.keys & key::SPRINT != 0),
            u8::from(body.keys & key::CROUCH != 0),
            body.client_walk.0[0],
            body.client_walk.0[1],
            body.yaw,
            u8::from(body.client_walk.1)
        );
    }
}

/// One walk: the lone bot from the leg's start to its goal through A and P; each stand-in tick into `trace` when given.
fn walk(world: &Arc<World>, project: &str, leg: &Leg, limit_ms: u32, seed: u32, mut trace: Option<&mut TickTrace>) -> Outcome {
    let Leg { start, goal, a, p } = *leg;
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
        super::Trace::none(0),
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
    let mut seen = RouteSeen::default();
    while clock < end_at {
        clock += 5;
        while game.tick + TICK_MS <= clock {
            for bot in bots.bots.iter().flatten() {
                game.kits[bot.place] = bot.reflex.arms();
            }
            let flew = game.bodies[0].flight.is_some();
            game.step();
            let body = &game.bodies[0];
            if let Some(trace) = trace.as_deref_mut() {
                trace.tick(body, game.tick, flew);
            }
            if !body.alive && !out.died {
                out.died = true;
                let at = at_of(&bots, &game, &seen, clock);
                out.push(clock, NoteKind::Died, at.feet, leg, at.motor, "died".to_owned());
                end_at = clock;
            }
            let body = &game.bodies[0];
            out.passed |= near(body.feet, p);
            out.edge = out.edge.step(body.feet, game.tick, a);
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
                let mut notes = Vec::new();
                for (place, (fix, health)) in fixes.into_iter().zip(health).enumerate() {
                    notes.extend(bots.fixed(place, fix, Some(health)));
                }
                notes.extend(bots.facts(facts));
                for note in notes {
                    if note.place == 0 {
                        out.note(&note.text, clock, leg, at_of(&bots, &game, &seen, clock));
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
                out.note(&note.text, clock, leg, at_of(&bots, &game, &seen, clock));
            }
        }
        if let Some(bot) = bots.bots[0].as_ref() {
            seen.look(bot, clock);
        }
        if out.resolved.is_none()
            && let Some(Some(crate::v026g::bots::plays::Go::To { at, .. })) = bots.bots[0].as_ref().map(|bot| bot.bench_go)
        {
            out.resolved = Some(at);
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
    if out.reached_ms.is_none() && !out.died {
        let at = at_of(&bots, &game, &seen, clock);
        out.push(clock, NoteKind::Timeout, out.end, leg, at.motor, format!("not at the goal after {limit_ms} ms"));
    }
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
    let only: Option<Vec<String>> = spot_list("SRP_SPOT_SUITE_ONLY");
    let threads: usize = std::env::var("SRP_SPOT_SUITE_THREADS").ok().and_then(|v| v.parse().ok()).unwrap_or(2).max(1);
    let limit_ms: u32 = std::env::var("SRP_SPOT_SUITE_SECONDS").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(60) * 1000;
    let notes_path = std::env::var("SRP_SPOT_SUITE_NOTES").unwrap_or_else(|_| format!("{out_path}.notes.tsv"));
    let (_, rows) = tsv_rows(&starts);
    // Each start's seed is its row's place among its map's rows of the WHOLE starts file, taken before `SRP_SPOT_SUITE_ONLY`
    // keeps a subset: a subset run walks each of its starts exactly as the full run does (the per-PR gate on a spot list).
    let mut seen: HashMap<String, u32> = HashMap::new();
    let seeds: Vec<u32> = rows
        .iter()
        .map(|r| {
            let n = seen.entry(r["project"].clone()).or_insert(0);
            *n += 1;
            *n - 1
        })
        .collect();
    let rows: Vec<_> = rows
        .into_iter()
        .zip(seeds)
        .filter(|(r, _)| only.as_ref().is_none_or(|o| o.contains(&r["spot"])))
        .collect();
    let mut projects: Vec<String> = rows.iter().map(|(r, _)| r["project"].clone()).collect();
    projects.sort();
    projects.dedup();
    let out = Mutex::new(std::fs::File::create(&out_path).unwrap_or_else(|e| panic!("{out_path}: {e}")));
    writeln!(
        out.lock().unwrap(),
        "spot\tproject\tstart\tvia\tlink_kind\treached\treach_ms\tpassed\terrors\tstuck\tjump\tdead\tfall\tlip\tnoway\tdied\ttimeout\tend_x\tend_y\tend_z\tsetup\tresolved_x\tresolved_y\tresolved_z\tpress\tover_x\tover_y\tover_z\tland_x\tland_y\tland_z\tstandin\terrors_at_spot\terrors_on_way\twhy\tfirst_error"
    )
    .unwrap();
    let notes_out = Mutex::new(std::fs::File::create(&notes_path).unwrap_or_else(|e| panic!("{notes_path}: {e}")));
    writeln!(notes_out.lock().unwrap(), "spot\tstart\tms\tkind\tat_spot\tmotor\ttext").unwrap();
    let trace_out = std::env::var_os("SRP_SPOT_SUITE_TRACE").map(|_| {
        let path = format!("{out_path}.trace.tsv");
        let mut file = std::fs::File::create(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
        writeln!(file, "spot\tstart\tms\tfeet_x\tfeet_y\tfeet_z\tkeys\tjump_press\tin_flight\ton_ground\tsprint\tcrouch\twalk_x\twalk_z\tyaw\tsprint_state").unwrap();
        Mutex::new(file)
    });
    for project in projects {
        let list: Vec<(&HashMap<String, String>, u32)> = rows.iter().filter(|(r, _)| r["project"] == project).map(|(r, s)| (r, *s)).collect();
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
                    let Some(&(row, seed)) = list.get(i) else { break };
                    let head = format!("{}\t{}\t{}\t{}\t{}", row["spot"], row["project"], row["start"], row["via"], row["link_kind"]);
                    // A starts file before the `why` column (spot-starts-v1.tsv): none said.
                    let why = row.get("why").map_or("-", String::as_str);
                    let line = match (v3(row, "sx", "sy", "sz"), v3(row, "gx", "gy", "gz"), v3(row, "px", "py", "pz")) {
                        (Some(s), Some(g), Some(p)) => {
                            let a = v3(row, "ax", "ay", "az").unwrap_or(p);
                            let leg = Leg { start: s, goal: g, a, p };
                            let mut trace = trace_out.as_ref().map(|_| TickTrace { head: format!("{}\t{}", row["spot"], row["start"]), rows: String::new() });
                            // Each start on a thread of its own: the planner keeps per-thread caches whose filling decides what a
                            // poll's budget walks (mapdata's PULL_WALKS: a cached pull-walk answer is free, a new one spends
                            // the poll's walk ticks), so on a shared worker a start's routes would depend on the starts that
                            // ran there before it, and the run's rows on the workers' timing.
                            let o = std::thread::scope(|own| {
                                own.spawn(|| walk(&world, &project, &leg, limit_ms, seed, trace.as_mut())).join().expect("a start's walk")
                            });
                            if let (Some(file), Some(trace)) = (trace_out.as_ref(), trace) {
                                file.lock().unwrap().write_all(trace.rows.as_bytes()).unwrap();
                            }
                            {
                                let mut notes = notes_out.lock().unwrap();
                                for n in &o.notes {
                                    writeln!(
                                        notes,
                                        "{}\t{}\t{}\t{}\t{}\t{}\t{}",
                                        row["spot"],
                                        row["start"],
                                        n.ms,
                                        n.kind.word(),
                                        u8::from(n.at_spot),
                                        cell(&n.motor),
                                        cell(&n.text)
                                    )
                                    .unwrap();
                                }
                            }
                            let timeout = o.timed_out();
                            let first = o.first_error();
                            // SETUP: the bot's own goal is not G (moved off a lip, or never walked to), or its planner refused G's floor first.
                            let setup = o.resolved.is_none_or(|r| flat(r, g) > RESOLVED_M || (r.y - g.y).abs() > RESOLVED_M)
                                || first.is_some_and(|f| f.kind == NoteKind::NoWay(NoWayWhy::GoalRefused));
                            let nan = Vec3::new(f32::NAN, f32::NAN, f32::NAN);
                            let r = o.resolved.unwrap_or(nan);
                            let errors = o.errors();
                            let failed = o.reached_ms.is_none() || errors > 0;
                            let standin = o.standin(&leg, failed);
                            let over = o.edge.over().map_or(nan, |(at, _)| at);
                            let land = o.edge.landing().unwrap_or(nan);
                            let at_spot = o.notes.iter().filter(|n| n.kind.error() && n.at_spot).count();
                            let first_error = first.map_or("-".to_owned(), |f| match f.kind {
                                NoteKind::Timeout => "timeout".to_owned(),
                                _ => format!("{} {}", f.ms, cell(&f.text)),
                            });
                            format!(
                                "{head}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{:.2}\t{:.2}\t{:.2}\t{}\t{:.2}\t{:.2}\t{:.2}\t{}\t{:.2}\t{:.2}\t{:.2}\t{:.2}\t{:.2}\t{:.2}\t{}\t{}\t{}\t{why}\t{first_error}",
                                u8::from(o.reached_ms.is_some()),
                                o.reached_ms.map_or("-".to_owned(), |t| t.to_string()),
                                u8::from(o.passed),
                                errors,
                                o.count(|k| k == NoteKind::Stuck),
                                o.count(|k| matches!(k, NoteKind::Jump(_))),
                                o.count(|k| k == NoteKind::Dead),
                                o.count(|k| k == NoteKind::Fall),
                                o.count(|k| k == NoteKind::Lip),
                                o.count(|k| matches!(k, NoteKind::NoWay(_))),
                                u8::from(o.died),
                                u8::from(timeout),
                                o.end.x,
                                o.end.y,
                                o.end.z,
                                u8::from(setup),
                                r.x,
                                r.y,
                                r.z,
                                o.press.map_or("-", |(_, press)| press.word()),
                                over.x,
                                over.y,
                                over.z,
                                land.x,
                                land.y,
                                land.z,
                                standin.map_or("-", Press::word),
                                at_spot,
                                errors - at_spot,
                            )
                        }
                        _ => format!("{head}\t0\t-\t0\t1\t0\t0\t0\t0\t0\t0\t0\t0\tnan\tnan\tnan\t0\tnan\tnan\tnan\t-\tnan\tnan\tnan\tnan\tnan\tnan\t-\t0\t0\t{why}\tno grid way through the spot"),
                    };
                    writeln!(out.lock().unwrap(), "{line}").unwrap();
                });
            }
        });
        eprintln!("{project}: {} starts in {:.0} s", list.len(), started.elapsed().as_secs_f32());
    }
    // [stand-in jump] How the stand-in flew the run's jumps (`standin_jump::controller_jumps`).
    let [flown, clip, landed, fell] = super::standin_jump::controller_jumps::counts();
    eprintln!("stand-in jumps: {flown} flown by the client's flight ({landed} landed, {fell} fell on), {clip} on the clip (side or back presses)");
    // [stand-in fidelity] The stand-in's falls that came down on the walker's ground the grid had no floor for.
    let [walker, over] = super::standin_jump::walker_landings::counts();
    eprintln!("stand-in falls: {walker} came down on the client's walker ground over the grid's floor ({:.2} m over it in all)", over as f32 / 100.0);
}

/// [standin-fidelity] A drop the bot pressed (a route jump down from A) whose body came down 3.4 m off the landing is tagged; one
/// that came down on it, or a spot that is no drop, is not.
#[test]
fn standin_tag_of_a_pressed_drop_landed_off() {
    let (a, p) = (Vec3::new(0.0, 10.0, 0.0), Vec3::new(3.0, 6.0, 0.0));
    let leg = Leg { start: Vec3::new(-20.0, 10.0, 0.0), goal: Vec3::new(5.0, 6.0, 0.0), a, p };
    let walked = |landing: Vec3| {
        let mut o = Outcome::default();
        for (tick, feet) in [(4_000, Vec3::new(0.2, 10.0, 0.0)), (4_050, Vec3::new(1.0, 8.5, 0.0)), (4_100, landing), (4_150, landing)] {
            o.edge = o.edge.step(feet, tick, a);
        }
        o.note(
            "nav: route jump not down: from (0.2, 10.0, 0.0) to (3.0, 6.0, 0.0), rise -4.00 m, try 1, after 3000 ms (went on)",
            5_000,
            &leg,
            At { feet: landing, take_offs: Vec::new(), motor: String::new() },
        );
        o
    };
    let off = walked(Vec3::new(1.5, 6.0, -3.0));
    assert_eq!(off.edge.over().map(|(_, tick)| tick), Some(4_050));
    assert_eq!(off.standin(&leg, true), Some(Press::Jump(JumpWord::NotDown)));
    assert_eq!(off.standin(&leg, false), None);
    assert_eq!(walked(Vec3::new(3.0, 6.0, 0.5)).standin(&leg, true), None);
    assert_eq!(off.standin(&Leg { p: Vec3::new(3.0, 9.5, 0.0), ..leg }, true), None);
    // The jump "not down" is a miss: an error at the spot (its take-off is A).
    assert_eq!(off.notes.len(), 1);
    assert_eq!(off.notes[0].kind, NoteKind::Jump(JumpMiss::Missed(JumpOutcome::NotDown)));
    assert!(off.notes[0].at_spot);
}

/// The outcome of a route jump is its note's word before the first ':', whatever its tail says ("(went on)", "(flown ...)"); a
/// word the log does not write is read as none (an error), and a jump up or down is no error.
#[test]
fn jump_outcome_is_read_from_the_word() {
    let line = |word: &str, tail: &str| {
        format!("{word}: from (0.2, 10.0, 0.0) to (3.0, 6.0, 0.0), rise -4.00 m, try 1, after 3000 ms ({tail})")
    };
    for outcome in [JumpOutcome::Up, JumpOutcome::Short, JumpOutcome::Down, JumpOutcome::NotDown, JumpOutcome::GaveUp] {
        assert_eq!(jump_outcome(&line(outcome.word(), "went on")), Some(outcome));
        assert_eq!(jump_outcome(&line(outcome.word(), "flown 0.40 m short (lead 0.1)")), Some(outcome));
    }
    assert_eq!(jump_outcome(&line("landed", "went on")), None);
    // As the run counts them: "up" and "down" are no error, the others are.
    let (a, p) = (Vec3::new(50.0, 0.0, 0.0), Vec3::new(52.0, 0.0, 0.0));
    let leg = Leg { start: Vec3::ZERO, goal: Vec3::new(60.0, 0.0, 0.0), a, p };
    let mut o = Outcome::default();
    for word in ["up", "down", "short", "not down", "gave up", "landed"] {
        o.note(&format!("nav: route jump {}", line(word, "went on")), 1_000, &leg, At { feet: Vec3::ZERO, take_offs: Vec::new(), motor: String::new() });
    }
    let kinds: Vec<&str> = o.notes.iter().map(|n| n.kind.word()).collect();
    assert_eq!(kinds, ["other", "other", "jump", "jump", "jump", "jump-unread"]);
    assert_eq!(o.errors(), 4);
    // Far from A and P: on the way.
    assert!(o.notes.iter().all(|n| !n.at_spot));
}

/// A lip brake is an error only at a planned take-off: near a link's take-off of the route's legs, or near A at a drop spot;
/// a brake near P, or near A of a level spot, is not.
#[test]
fn lip_brake_counts_only_at_a_planned_take_off() {
    let (a, level_p, drop_p) = (Vec3::new(0.0, 10.0, 0.0), Vec3::new(3.0, 10.0, 0.0), Vec3::new(3.0, 8.0, 0.0));
    let take_off = Vec3::new(20.0, 10.0, 0.0);
    let brake = |x: f32| Vec3::new(x, 10.0, 0.5);
    // At a link's take-off of the route, wherever the spot is.
    assert!(lip_at_take_off(brake(19.0), &[take_off], a, level_p));
    assert!(!lip_at_take_off(brake(17.5), &[take_off], a, level_p));
    // Near A: only at a drop spot.
    assert!(lip_at_take_off(brake(1.0), &[], a, drop_p));
    assert!(!lip_at_take_off(brake(1.0), &[], a, level_p));
    // Near P, no link there: not at a take-off.
    assert!(!lip_at_take_off(brake(3.5), &[], a, drop_p));
    // As the run counts them: `lip` fails the start, `lip-other` does not.
    let leg = Leg { start: Vec3::new(-20.0, 10.0, 0.0), goal: Vec3::new(5.0, 8.0, 0.0), a, p: level_p };
    let mut o = Outcome::default();
    let at = |take_offs: Vec<Vec3>| At { feet: brake(1.0), take_offs, motor: "leg walk".to_owned() };
    o.note("nav: lip guard braked at (1.00, 10.00, 0.50): no kept controller", 2_000, &leg, at(Vec::new()));
    o.note("nav: lip guard braked at (1.00, 10.00, 0.50): no kept controller", 2_500, &leg, at(vec![Vec3::new(1.5, 10.0, 0.0)]));
    let kinds: Vec<&str> = o.notes.iter().map(|n| n.kind.word()).collect();
    assert_eq!(kinds, ["lip-other", "lip"]);
    assert_eq!(o.errors(), 1);
}

/// A note is at the spot within 2.5 m of A or P in a straight line (height counts), else on the way.
#[test]
fn at_spot_is_within_two_and_a_half_metres_of_a_or_p() {
    let (a, p) = (Vec3::new(0.0, 10.0, 0.0), Vec3::new(4.0, 6.0, 0.0));
    assert!(at_spot(Vec3::new(2.0, 10.0, 1.0), a, p));
    assert!(at_spot(Vec3::new(4.0, 7.5, 1.5), a, p));
    assert!(!at_spot(Vec3::new(0.0, 7.0, 0.0), a, p));
    assert!(!at_spot(Vec3::new(10.0, 6.0, 0.0), a, p));
    // The note's own point first, else the body's feet.
    assert_eq!(note_point("player 1 is stuck at [1.0, 2.0, 3.0]: x"), Some(Vec3::new(1.0, 2.0, 3.0)));
    assert_eq!(note_point("nav: lip guard braked at (4.0, 5.0, 6.0): y"), Some(Vec3::new(4.0, 5.0, 6.0)));
    assert_eq!(note_point("some note"), None);
}
