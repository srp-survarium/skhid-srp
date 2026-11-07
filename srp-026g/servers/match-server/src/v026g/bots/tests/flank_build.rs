//! [A320 `threat.camp_flank`] The flank-route table's writer (`camp_flank.rs`), offline, from the A320 read's big camp
//! spots (`a320_flank_input.py`: spots of 20+ kills and the human wins at them) and each map's world:
//!
//! ```text
//! SRP_MAPDATA_DIR=<mapdata folder> SRP_FLANK_INPUT=<flank-input.json> SRP_WRITE_FLANK_ROUTES=<bot-flank-routes.json> \
//!   <test binary> write_flank_routes --ignored --nocapture
//! ```
//!
//! Per spot and the side it covers:
//! - the camper's map side is the spawn team whose spawns' middle is nearest the read's spawn centroid of his side;
//! - a floor is seen when the spot's standing or crouched eye sees a crouched flanker's eye over it (`view::still_eye`,
//!   the game's bullet origin: the head matrix `player+0x44C`, `weapon_core::instant_fire` 0x2E23A0; a flanker crouched
//!   through the exposed metres: fx-grid's review);
//! - starts: floors 40-90 m from the spot on the covered side's way from its spawns to the spot (a spawn nearer than that:
//!   the spawn itself), 15 m apart, at most [`STARTS`];
//! - firing floors: 8-25 m from the spot (the human flank wins fired from 8-13 m), whose crouched eye sees the spot's
//!   chest, 45 degrees or more off the victims' lane, spread by bearing, at most [`FIRES`];
//! - an A* route from each start to each firing floor, a seen floor costing [`SEEN_WEIGHT`] times its length (the
//!   grid's area cost, `SlicedSearch::step`); a route that jumps onto a seen floor is searched again with that floor
//!   dearer; the floors of the routes kept are made dearer for the next (diversity);
//! - each human win's path, its samples (5 s apart) joined by plain A* legs, as a human route;
//! - a route ends at the first floor of its last run of floors from which the spot is seen (where it fires from).
//!
//! The routes kept: the human ones and the [`KEEP`] best of the searched, by length plus [`SEEN_WEIGHT`] times the seen
//! metres, each written as feet points about 0.75 m apart and checked by reading it back onto the same grid.

use std::cell::RefCell;
use std::collections::HashMap;

use mapdata::{LinkKind, NodeId, Search, WalkGrid};
use serde::Deserialize;
use survarium::v026g::maps::RuleMode;

use super::*;
use crate::v026g::bots::camp_flank::{FlankTable, Origin, one_walk};
use crate::v026g::bots::rays::visible;
use crate::v026g::bots::view::still_eye;

/// A written route point about every this, m (the loader walks straight between two).
const POINT_GAP_M: f32 = 0.75;
/// A seen floor's cost per metre over a hidden one's.
const SEEN_WEIGHT: f32 = 20.0;
/// What a floor of a route already kept costs over its own, for the next search (diversity).
const KEPT_WEIGHT: f32 = 3.0;
/// The firing ring, m from the spot.
const FIRE_RING_M: (f32, f32) = (8.0, 25.0);
/// The starts' band, m from the spot.
const START_BAND_M: (f32, f32) = (40.0, 90.0);
/// Starts and firing floors per spot and side.
const STARTS: usize = 3;
const FIRES: usize = 4;
/// Searched routes kept per spot and side.
const KEEP: usize = 6;
/// A firing floor is this far off the victims' lane at least, degrees.
const OFF_LANE_DEG: f32 = 45.0;
/// Floors expanded per search at most.
const BUDGET: usize = 400_000;
/// A searched route's firing floor within this of a human win's is backed by it, m.
const NEAR_WIN_M: f32 = 5.0;

#[derive(Debug, Deserialize)]
struct Input {
    spots: Vec<InSpot>,
}

#[derive(Debug, Deserialize)]
struct InSpot {
    level: String,
    mode: u8,
    side: String,
    spot: usize,
    at: [f32; 3],
    lane_bearing_deg: f32,
    camper_spawn: [f32; 2],
    wins: Vec<InWin>,
}

#[derive(Debug, Deserialize)]
struct InWin {
    path: Vec<[f32; 3]>,
    fire: [f32; 3],
}

fn v3(p: [f32; 3]) -> Vec3 {
    Vec3::new(p[0], p[1], p[2])
}

/// The bearing from `a` to `b`, degrees, 0 = +z, 90 = +x (the read's).
fn bearing(a: Vec3, b: Vec3) -> f32 {
    (b.x - a.x).atan2(b.z - a.z).to_degrees().rem_euclid(360.0)
}

fn off_deg(a: f32, b: f32) -> f32 {
    let d = (a - b).rem_euclid(360.0);
    d.min(360.0 - d)
}

/// One spot's sight: whether each floor is seen from its eyes (asked lazily, kept).
struct SpotSight<'w> {
    world: &'w World,
    eyes: [Vec3; 2],
    /// [B3g4] Where he may be: his standing and crouched eye over his spot's floor and the floors 1 m round it (he holds
    /// within his spot, crouches, peeks): a firing floor must see enough of them, not one point (Darken et al., visibility
    /// as a fraction; a single ray left 17 of 21 arrivals with no line to him).
    targets: Vec<Vec3>,
    seen: RefCell<HashMap<NodeId, bool>>,
    fires: RefCell<HashMap<NodeId, bool>>,
}

/// [B3g4] A firing floor sees at least this share of his sample points.
const FIRE_SHARE: f32 = 0.6;
/// [B3g4] The route stops this many floors inside the stretch that fires on him (0.25 m apart: about 1 m past his cover's
/// edge), not on its first floor.
const FIRE_INSET: usize = 4;
/// [B3g4] His sample points lie on the floors this far round his spot's, m.
const TARGET_RING_M: f32 = 1.0;

impl SpotSight<'_> {
    fn seen(&self, n: NodeId) -> bool {
        *self.seen.borrow_mut().entry(n).or_insert_with(|| {
            // A crouched flanker's eye there (the exe's head matrix, `view::eye_height`; 1.04 m still).
            let top = still_eye(self.world.grid.position(n), true);
            self.eyes.iter().any(|&eye| (top - eye).length() <= 120.0 && visible(&self.world.sight, eye, top))
        })
    }

    /// Whether a flanker crouched on `n` sees [`FIRE_SHARE`] of his sample points (asked lazily, kept).
    fn fires_from(&self, n: NodeId) -> bool {
        *self.fires.borrow_mut().entry(n).or_insert_with(|| {
            let eye = still_eye(self.world.grid.position(n), true);
            let seen = self.targets.iter().filter(|&&t| visible(&self.world.sight, eye, t)).count();
            seen as f32 >= FIRE_SHARE * self.targets.len() as f32
        })
    }
}

/// A route found: its floors, length and seen metres.
#[derive(Clone, Debug)]
struct Found {
    floors: Vec<NodeId>,
    length_m: f32,
    seen_m: f32,
}

impl Found {
    fn score(&self) -> f32 {
        self.length_m + SEEN_WEIGHT * self.seen_m
    }
}

/// The weighted A* from `from` to `to` (`weight` at least 1 a floor).
fn search(grid: &WalkGrid, from: NodeId, to: NodeId, weight: impl Fn(NodeId) -> f32) -> Option<Vec<NodeId>> {
    let mut s = grid.start_search(from, to, BUDGET).ok()?;
    let mut iters = usize::MAX;
    match s.step(grid, |_| true, weight, &mut iters)? {
        Search::Found(path) => Some(path.nodes),
        Search::NoWay | Search::TooFar => None,
    }
}

/// The route cut at the first floor of its last run of floors that fire on the spot; its length and seen metres to there.
fn ending_at_fire(grid: &WalkGrid, sight: &SpotSight, mut floors: Vec<NodeId>) -> Option<Found> {
    let last = floors.iter().rposition(|&n| sight.fires_from(n))?;
    let first = (0..=last).rev().take_while(|&i| sight.fires_from(floors[i])).last()?;
    floors.truncate((first + FIRE_INSET).min(last) + 1);
    let (mut length_m, mut seen_m) = (0.0, 0.0);
    for w in floors.windows(2) {
        let d = (grid.position(w[1]) - grid.position(w[0])).length();
        length_m += d;
        if sight.seen(w[1]) {
            seen_m += d;
        }
    }
    Some(Found { floors, length_m, seen_m })
}

/// The index of the first link of `floors` that leaps onto a seen floor (jump links inside the spot's sight are refused).
fn seen_leap(grid: &WalkGrid, sight: &SpotSight, floors: &[NodeId]) -> Option<usize> {
    floors.windows(2).position(|w| {
        let kind = grid.links(w[0]).find(|&(n, _)| n == w[1]).map(|(_, k)| k);
        kind.is_some_and(LinkKind::is_leap) && sight.seen(w[1])
    })
}

/// The floors spread along `floors`, `gap` m apart at least, that `keep` lets through, at most `n`.
fn spread(grid: &WalkGrid, floors: impl Iterator<Item = NodeId>, gap: f32, n: usize, keep: impl Fn(NodeId) -> bool) -> Vec<NodeId> {
    let mut out: Vec<NodeId> = Vec::new();
    for f in floors.filter(|&f| keep(f)) {
        if out.iter().all(|&o| (grid.position(f) - grid.position(o)).length() >= gap) {
            out.push(f);
            if out.len() == n {
                break;
            }
        }
    }
    out
}

/// The floors of a route that are written: one about every [`POINT_GAP_M`], both ends of each link that is not a step on
/// the level, and the last; a floor is kept too wherever leaving it out would leave two written floors in a row that are
/// not one walk apart for the loader ([`one_walk`]).
fn written(grid: &WalkGrid, floors: &[NodeId]) -> Vec<NodeId> {
    let Some(&first) = floors.first() else { return Vec::new() };
    let level = |a: NodeId, b: NodeId| grid.links(a).any(|(n, k)| n == b && k == LinkKind::Walk);
    let mut out = vec![first];
    for i in 1..floors.len() {
        let (prev, here) = (*out.last().unwrap(), floors[i]);
        let last = i + 1 == floors.len();
        let far = grid.position(here).distance_xz(grid.position(prev)) >= POINT_GAP_M;
        let leap = !level(floors[i - 1], here) || floors.get(i + 1).is_some_and(|&next| !level(here, next));
        let broken = floors.get(i + 1).is_some_and(|&next| !one_walk(grid, prev, next));
        if last || far || leap || broken {
            out.push(here);
        }
    }
    out
}

/// The feet points of floors (the loader puts each back on its floor).
fn points(grid: &WalkGrid, floors: &[NodeId]) -> Vec<[f32; 3]> {
    floors
        .iter()
        .map(|&n| {
            let p = grid.position(n);
            [(p.x * 100.0).round() / 100.0, (p.y * 100.0).round() / 100.0, (p.z * 100.0).round() / 100.0]
        })
        .collect()
}

/// One spot's routes (human ones first), as the table's rows, and what was made and left out.
fn routes_of(world: &World, spot: &InSpot, camper_side: i32) -> (Vec<serde_json::Value>, String) {
    let grid = &world.grid;
    let at = v3(spot.at);
    let Some(feet_node) = grid.locate(at, 2.0) else {
        return (Vec::new(), "no floor at the spot".into());
    };
    let feet = grid.position(feet_node);
    let sight = SpotSight {
        world,
        eyes: [still_eye(feet, false), still_eye(feet, true)],
        // What a flanker must see to fire on him: his standing eye (the exe's head matrix, `view::still_eye`).
        targets: [(0.0, 0.0), (TARGET_RING_M, 0.0), (-TARGET_RING_M, 0.0), (0.0, TARGET_RING_M), (0.0, -TARGET_RING_M)]
            .iter()
            .filter_map(|&(dx, dz)| grid.locate(feet + Vec3::new(dx, 0.0, dz), 0.3).map(|n| grid.position(n)))
            .filter(|p| (p.y - feet.y).abs() <= 0.5)
            .flat_map(|p| [still_eye(p, false), still_eye(p, true)])
            .collect(),
        seen: RefCell::new(HashMap::new()),
        fires: RefCell::new(HashMap::new()),
    };
    let weight = |n: NodeId, extra: &HashMap<NodeId, f32>| {
        (if sight.seen(n) { SEEN_WEIGHT } else { 1.0 }) * extra.get(&n).copied().unwrap_or(1.0)
    };
    // The covered side's ways in: from each of its spawns to the spot, plain.
    let covered: Vec<NodeId> = world
        .map
        .spawns
        .iter()
        .filter(|s| s.team != camper_side)
        .filter_map(|s| grid.locate(s.position, 2.0))
        .collect();
    let ways: Vec<Vec<NodeId>> = spread(grid, covered.into_iter(), 10.0, 4, |_| true)
        .into_iter()
        .filter_map(|s| grid.path(s, feet_node).map(|p| p.nodes))
        .collect();
    let in_band = |n: NodeId| {
        let d = grid.position(n).distance_xz(feet);
        (START_BAND_M.0..=START_BAND_M.1).contains(&d)
    };
    // A way whose spawn is nearer the spot than the band (the spots at spawn exits and in the spawn, 8-51 m off: Cologne's
    // biggest, 1,376 kills, 22 m) starts at its spawn: the way out by another exit.
    let near_start = |w: &Vec<NodeId>| {
        let first = *w.first()?;
        (!w.iter().any(|&n| in_band(n))).then_some(first)
    };
    let banded = ways.iter().flat_map(|w| w.iter().copied().filter(|&n| in_band(n)));
    let starts = spread(grid, ways.iter().filter_map(near_start).chain(banded), 15.0, STARTS, |_| true);
    let ring = grid.floors_in([feet.x - FIRE_RING_M.1, feet.z - FIRE_RING_M.1, feet.x + FIRE_RING_M.1, feet.z + FIRE_RING_M.1]);
    let mut fire_floors: Vec<NodeId> = ring
        .filter(|&n| grid.can_stand(n) && !grid.in_shut_pocket(n) && grid.may_reach(n, feet_node))
        .filter(|&n| {
            let p = grid.position(n);
            let d = p.distance_xz(feet);
            (FIRE_RING_M.0..=FIRE_RING_M.1).contains(&d)
                && (p.y - feet.y).abs() <= 6.0
                && off_deg(bearing(feet, p), spot.lane_bearing_deg) >= OFF_LANE_DEG
        })
        .collect();
    // Spread by bearing: the floors in order round the spot, then every k-th that fires on it.
    fire_floors.sort_by(|&a, &b| bearing(feet, grid.position(a)).total_cmp(&bearing(feet, grid.position(b))));
    let step = (fire_floors.len() / (FIRES * 8)).max(1);
    let candidates: Vec<NodeId> = fire_floors.iter().copied().step_by(step).filter(|&n| sight.fires_from(n)).collect();
    let mut fires: Vec<NodeId> = Vec::new();
    for &c in &candidates {
        let b = bearing(feet, grid.position(c));
        if fires.iter().all(|&f| off_deg(bearing(feet, grid.position(f)), b) >= 360.0 / (FIRES as f32 * 2.0)) {
            fires.push(c);
        }
        if fires.len() == FIRES {
            break;
        }
    }
    // The human wins.
    let mut rows = Vec::new();
    let mut wins_fire: Vec<Vec3> = Vec::new();
    let mut human = 0;
    for win in &spot.wins {
        let snapped: Option<Vec<NodeId>> = win.path.iter().map(|&p| grid.locate(v3(p), 2.0)).collect();
        let Some(snapped) = snapped else { continue };
        let mut floors: Vec<NodeId> = vec![snapped[0]];
        let joined = snapped.windows(2).all(|w| match grid.path(w[0], w[1]) {
            Some(p) => {
                floors.extend_from_slice(&p.nodes[1..]);
                true
            }
            None => false,
        });
        if !joined {
            continue;
        }
        wins_fire.push(v3(win.fire));
        if let Some(found) = ending_at_fire(grid, &sight, floors) {
            human += 1;
            rows.push(row(grid, Origin::Human, &found));
        }
    }
    // The searched routes.
    let mut extra: HashMap<NodeId, f32> = HashMap::new();
    let mut found: Vec<Found> = Vec::new();
    let mut refused_leaps = 0;
    for &s in &starts {
        for &f in &fires {
            let mut tries = 0;
            let route = loop {
                let Some(floors) = search(grid, s, f, |n| weight(n, &extra)) else { break None };
                match seen_leap(grid, &sight, &floors) {
                    Some(i) if tries < 3 => {
                        *extra.entry(floors[i + 1]).or_insert(1.0) *= 100.0;
                        refused_leaps += 1;
                        tries += 1;
                    }
                    Some(_) => break None,
                    None => break ending_at_fire(grid, &sight, floors),
                }
            };
            if let Some(route) = route {
                for &n in &route.floors {
                    *extra.entry(n).or_insert(1.0) *= KEPT_WEIGHT;
                }
                found.push(route);
            }
        }
    }
    found.sort_by(|a, b| a.score().total_cmp(&b.score()));
    for r in found.iter().take(KEEP) {
        let fire_at = grid.position(*r.floors.last().unwrap());
        let origin = if wins_fire.iter().any(|&w| (fire_at - w).length() <= NEAR_WIN_M) { Origin::AstarNearWin } else { Origin::Astar };
        rows.push(row(grid, origin, r));
    }
    let note = format!(
        "starts {}, firing floors {} of {} in the ring, searched {} (kept {}), human {human} of {} wins, jump refusals {refused_leaps}",
        starts.len(),
        fires.len(),
        candidates.len(),
        found.len(),
        found.len().min(KEEP),
        spot.wins.len()
    );
    (rows, note)
}

fn row(grid: &WalkGrid, origin: Origin, r: &Found) -> serde_json::Value {
    let kept = written(grid, &r.floors);
    let origin = match origin {
        Origin::Human => "human",
        Origin::AstarNearWin => "astar_near_win",
        Origin::Astar => "astar",
    };
    serde_json::json!({
        "origin": origin,
        "points": points(grid, &kept),
        "fire": kept.len() - 1,
        "exposed_m": (r.seen_m * 10.0).round() / 10.0,
    })
}

/// The camper's map side: the spawn team whose spawns' middle is nearest the read's spawn centroid of his side.
fn camper_side(world: &World, centroid: [f32; 2]) -> Option<i32> {
    let mut by: HashMap<i32, (f32, f32, f32)> = HashMap::new();
    for s in &world.map.spawns {
        let e = by.entry(s.team).or_default();
        *e = (e.0 + s.position.x, e.1 + s.position.z, e.2 + 1.0);
    }
    by.into_iter()
        .map(|(team, (x, z, n))| (team, (x / n - centroid[0]).hypot(z / n - centroid[1])))
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(team, _)| team)
}

/// Writes the table (see the module's note). A tool, not a check.
#[test]
#[ignore = "a tool; needs SRP_MAPDATA_DIR, SRP_FLANK_INPUT, SRP_WRITE_FLANK_ROUTES"]
fn write_flank_routes() {
    let s = bench_settings();
    let (Some(dir), Some(input), Some(out)) =
        (s.var_os("SRP_MAPDATA_DIR"), s.var_os("SRP_FLANK_INPUT"), s.var_os("SRP_WRITE_FLANK_ROUTES"))
    else {
        eprintln!("skipping: SRP_MAPDATA_DIR, SRP_FLANK_INPUT and SRP_WRITE_FLANK_ROUTES are needed");
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    let input: Input = serde_json::from_str(&std::fs::read_to_string(&input).unwrap()).unwrap();
    let only: Option<Vec<String>> = s.var("SRP_FLANK_LEVELS").ok().map(|v| v.split(',').map(str::to_owned).collect());
    let mut groups: Vec<(String, u8)> = input.spots.iter().map(|p| (p.level.clone(), p.mode)).collect();
    groups.sort();
    groups.dedup();
    let tuning = bench_tuning();
    let mut spots_out = Vec::new();
    for (level, mode) in groups {
        if only.as_ref().is_some_and(|o| !o.contains(&level)) {
            continue;
        }
        let file = dir.join(mapdata::format::file_name(&format!("{level}/day_{mode}")));
        let Ok(map) = mapdata::MapData::read(&file) else {
            println!("{level} mode {mode}: no map file {}", file.display());
            continue;
        };
        let started = std::time::Instant::now();
        let world = World::build_with(
            map,
            mapdata::CELL,
            tuning.grid_body(),
            tuning.off_mesh_links(),
            tuning.needs_wall_clearance(),
            tuning.needs_open_sides(),
        );
        for spot in input.spots.iter().filter(|p| p.level == level && p.mode == mode) {
            let Some(side) = camper_side(&world, spot.camper_spawn) else { continue };
            let t = std::time::Instant::now();
            let (routes, note) = routes_of(&world, spot, side);
            println!(
                "{level} mode {mode} {}#{}: camper side {side}, {} routes; {note} ({:.1} s)",
                spot.side,
                spot.spot,
                routes.len(),
                t.elapsed().as_secs_f32()
            );
            if routes.is_empty() {
                continue;
            }
            spots_out.push(serde_json::json!({
                "level": level, "mode": mode, "camper_side": side, "at": spot.at,
                "match_radius_m": 5.0, "follow_radius_m": 10.0, "routes": routes,
            }));
        }
        // Read back onto the same grid: every route written must fit.
        let text = serde_json::json!({ "format": 1, "spots": spots_out.clone() }).to_string();
        let table = FlankTable::parse(&text).unwrap();
        let level_n: u8 = level[6..].parse().unwrap();
        let rows = table.for_map(level_n, RuleMode::of_config(mode).unwrap());
        let refused: usize = rows.iter().map(|r| r.on_grid(&world.grid).1.len()).sum();
        println!("{level} mode {mode}: read back {} spots, {refused} routes refused ({:.0} s)", rows.len(), started.elapsed().as_secs_f32());
        assert_eq!(refused, 0, "a route written does not fit the grid it was made on");
    }
    let text = serde_json::json!({ "format": 1, "spots": spots_out }).to_string();
    std::fs::write(&out, text).unwrap();
    println!("wrote {} spots to {}", spots_out.len(), std::path::Path::new(&out).display());
}

/// [A320 B3f8] Why the driver finds no way to some route floors (`NOWAYGOAL ... play=Flank`, all on level_05, the same
/// few goals from many places): each written route's floors against the rules the driver's search keeps
/// (`Tactics::allowed_from`: the hazard zones, the other side's spawn area for the flanker's side) and that the builder
/// does not, and the goals the bench logs named. A probe, not a check.
#[test]
#[ignore = "a probe; needs SRP_MAPDATA_DIR, SRP_FLANK_ROUTES"]
fn probe_flank_route_floors() {
    use crate::v026g::bots::tactics::Tactics;
    let s = bench_settings();
    let (Some(dir), Some(table)) = (s.var_os("SRP_MAPDATA_DIR"), s.var_os("SRP_FLANK_ROUTES")) else {
        eprintln!("skipping: SRP_MAPDATA_DIR and SRP_FLANK_ROUTES are needed");
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    let table = FlankTable::parse(&std::fs::read_to_string(&table).unwrap()).unwrap();
    let tuning = bench_tuning();
    // The goals the B3f/B3g logs named (level_05 mode 2).
    let goals = [
        Vec3::new(-62.6, 8.0, 12.7),
        Vec3::new(-61.9, 7.3, 16.4),
        Vec3::new(-71.4, 5.8, 17.9),
        Vec3::new(-55.1, 8.8, 13.4),
        Vec3::new(-72.4, 6.3, 8.7),
        Vec3::new(-6.9, 4.2, -10.1),
    ];
    for (level, mode) in [("level_02", 2u8), ("level_05", 2), ("level_08", 2)] {
        let file = dir.join(mapdata::format::file_name(&format!("{level}/day_{mode}")));
        let Ok(map) = mapdata::MapData::read(&file) else {
            println!("{level} mode {mode}: no map file");
            continue;
        };
        let world = World::build_with(
            map,
            mapdata::CELL,
            tuning.grid_body(),
            tuning.off_mesh_links(),
            tuning.needs_wall_clearance(),
            tuning.needs_open_sides(),
        );
        let tactics = Tactics::new(std::sync::Arc::new(world), tuning.hazard.margin_m.get());
        let grid = &tactics.world.grid;
        let level_n: u8 = level[6..].parse().unwrap();
        for row in table.for_map(level_n, RuleMode::of_config(mode).unwrap()) {
            let (spot, _) = row.on_grid(grid);
            // The flanker's map side: the camper's other one.
            let side = Some(1 - spot.camper_side);
            for (k, r) in spot.routes.iter().enumerate() {
                let floors = &r.floors[..=r.fire];
                let spawn = floors.iter().filter(|&&n| !tactics.spawn_ok(n, side, false)).count();
                let zoned = floors.iter().filter(|&&n| tactics.hazards.marked_at(n)).count();
                let unwalkable = floors.iter().filter(|&&n| !tactics.walkable(n)).count();
                let first_bad = floors.iter().position(|&n| !tactics.spawn_ok(n, side, false) || tactics.hazards.marked_at(n));
                println!(
                    "{level} spot at ({:.1},{:.1},{:.1}) side {} route {k} {:?}: {} floors to fire; other side's spawn area {spawn}, zoned {zoned}, unwalkable {unwalkable}; first bad floor {:?}",
                    spot.at.x, spot.at.y, spot.at.z, spot.camper_side, r.origin, floors.len(), first_bad
                );
            }
        }
        if level == "level_05" {
            for g in goals {
                match grid.locate(g, 1.0) {
                    Some(n) => println!(
                        "{level} goal ({:.1},{:.1},{:.1}): spawn_ok side 0 {} side 1 {}, zoned {}, walkable {}, past fixed zones {}",
                        g.x, g.y, g.z,
                        tactics.spawn_ok(n, Some(0), false),
                        tactics.spawn_ok(n, Some(1), false),
                        tactics.hazards.marked_at(n),
                        tactics.walkable(n),
                        tactics.past_fixed_zones(n)
                    ),
                    None => println!("{level} goal ({:.1},{:.1},{:.1}): no floor within 1 m", g.x, g.y, g.z),
                }
            }
        }
    }
}
