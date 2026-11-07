//! NAV-FIRST: the bot map (walk grid) scored against where real bodies stood and walked in the game.
//!
//! `SRP_SCORE_POINTS`: a CSV of body positions (`project,body,time,fx,fy,fz`, optional `who`), the feet the client reported (a
//! recording's worlds: humans and bots alike, both are the client's own collision). `SRP_SCORE_MAP`: the project scored (rows of other
//! projects are skipped); `SRP_MAPDATA_DIR`: the map data; `SRP_BENCH_TUNING`: the tuning patch (the grid switches).
//!
//! Two scores, printed as one `SCORE` line:
//! - **floors**: of the standing positions (the body's height within 0.3 m of its fixes just before and after, so no jump or fall),
//!   the share with a grid floor within 0.5 m flat and 0.5 m in height. A miss is `missing` (no floor within 0.5 m flat at any
//!   height near) or `height` (a floor there, at another height).
//! - **steps**: of the moves between two standing fixes of one body 0.5-8 m apart flat and at most 1.1 s apart, the share the grid
//!   can walk: a way over its links (any kind) from the floor under the first to the floor under the second no longer than 1.5 times
//!   the straight line plus 1 m. A miss is `no_way` (none within that length).
//!
//! `SRP_SCORE_OUT`: a CSV of the misses (class, the two points) for sorting by cause. Ignored; it prints a summary.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use std::sync::Arc;

use mapdata::{NodeId, Vec3, WalkGrid, World};

use super::bench::bench_tuning;

/// A floor this far flat from the feet still holds them (the capsule's radius and the cell's half).
const FLOOR_FLAT_M: f32 = 0.5;
/// And this far in height (a step's rims and the feet's own noise).
const FLOOR_HEIGHT_M: f32 = 0.5;
/// A fix is standing when the fixes around it are within this of its height.
const STANDING_M: f32 = 0.3;
/// The fixes around a standing one are at most this far in time, ms.
const NEAR_MS: u32 = 1100;

struct Pos {
    body: String,
    time: u32,
    feet: Vec3,
}

/// A bounded Dijkstra entry: the way's length so far, the floor.
#[derive(PartialEq)]
struct Entry(f32, NodeId);

impl Eq for Entry {}

impl PartialOrd for Entry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Entry {
    fn cmp(&self, other: &Self) -> Ordering {
        other.0.total_cmp(&self.0).then_with(|| other.1.cmp(&self.1))
    }
}

/// The shortest way over the grid's links from `from` to `to`, if one is no longer than `limit`.
fn way_length(g: &WalkGrid, from: NodeId, to: NodeId, limit: f32) -> Option<f32> {
    let mut best: HashMap<NodeId, f32> = HashMap::from([(from, 0.0)]);
    let mut open = BinaryHeap::from([Entry(0.0, from)]);
    while let Some(Entry(d, node)) = open.pop() {
        if node == to {
            return Some(d);
        }
        if best.get(&node).is_some_and(|&b| d > b) {
            continue;
        }
        let at = g.position(node);
        for (next, _) in g.links(node) {
            let nd = d + (g.position(next) - at).length();
            if nd > limit || best.get(&next).is_some_and(|&b| nd >= b) {
                continue;
            }
            best.insert(next, nd);
            open.push(Entry(nd, next));
        }
    }
    None
}

/// The floor under the feet, if it holds them.
fn floor_under(g: &WalkGrid, feet: Vec3) -> Result<NodeId, &'static str> {
    match g.locate_within(feet, FLOOR_FLAT_M, FLOOR_HEIGHT_M) {
        Some(node) if (g.position(node).y - feet.y).abs() <= FLOOR_HEIGHT_M => Ok(node),
        Some(_) => Err("height"),
        // A floor near at another height (more than half a metre over the feet, or far under them).
        None => Err(if g.locate_within(feet, FLOOR_FLAT_M, 5.0).is_some() { "height" } else { "missing" }),
    }
}

#[test]
#[ignore = "NAV-FIRST: needs body positions and the map data (SRP_SCORE_POINTS, SRP_SCORE_MAP, SRP_MAPDATA_DIR)"]
fn map_score_on_bodies() {
    let var = |name: &str| std::env::var(name).ok().filter(|v| !v.is_empty());
    let (Some(points), Some(project), Some(dir)) = (var("SRP_SCORE_POINTS"), var("SRP_SCORE_MAP"), var("SRP_MAPDATA_DIR")) else {
        eprintln!("skipping: SRP_SCORE_POINTS, SRP_SCORE_MAP and SRP_MAPDATA_DIR are needed");
        return;
    };
    let tuning = bench_tuning();
    let file = std::path::Path::new(&dir).join(mapdata::format::file_name(&project));
    let map = mapdata::MapData::read(&file).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
    let world = Arc::new(World::build_with(
        map,
        mapdata::CELL,
        tuning.grid_body(),
        tuning.off_mesh_links(),
        tuning.needs_wall_clearance(),
        tuning.needs_open_sides(),
    ));
    let g = &*world.grid;
    // The positions of this map, by body and time.
    let text = std::fs::read_to_string(&points).unwrap_or_else(|e| panic!("{points}: {e}"));
    let mut lines = text.lines();
    let head: Vec<&str> = lines.next().expect("a header").split(',').collect();
    let col = |name: &str| head.iter().position(|h| *h == name).unwrap_or_else(|| panic!("{points}: no column {name}"));
    let (c_project, c_body, c_time, c_x, c_y, c_z) = (col("project"), col("body"), col("time"), col("fx"), col("fy"), col("fz"));
    let mut bodies: HashMap<String, Vec<Pos>> = HashMap::new();
    for line in lines {
        let f: Vec<&str> = line.split(',').collect();
        if f.get(c_project) != Some(&project.as_str()) {
            continue;
        }
        let num = |c: usize| f.get(c).and_then(|v| v.parse::<f32>().ok()).filter(|v| v.is_finite());
        let (Some(x), Some(y), Some(z), Some(time)) = (num(c_x), num(c_y), num(c_z), f.get(c_time).and_then(|t| t.parse().ok())) else {
            continue;
        };
        let body = f[c_body].to_owned();
        bodies.entry(body.clone()).or_default().push(Pos { body, time, feet: Vec3::new(x, y, z) });
    }
    let mut out = String::from("class,body,time,x,y,z,x2,y2,z2\n");
    let (mut standing, mut on_floor, mut steps, mut walkable) = (0u32, 0u32, 0u32, 0u32);
    let mut classes: HashMap<&str, u32> = HashMap::new();
    for track in bodies.values_mut() {
        track.sort_by_key(|p| p.time);
        // Standing: the fixes before and after (when near in time) are level with it.
        let level = |i: usize, j: usize| {
            let (a, b) = (&track[i], &track[j]);
            a.time.abs_diff(b.time) > NEAR_MS || (a.feet.y - b.feet.y).abs() <= STANDING_M
        };
        let is_standing: Vec<bool> =
            (0..track.len()).map(|i| (i == 0 || level(i, i - 1)) && (i + 1 == track.len() || level(i, i + 1))).collect();
        let floors: Vec<Option<NodeId>> = track
            .iter()
            .zip(&is_standing)
            .map(|(p, &s)| {
                if !s {
                    return None;
                }
                standing += 1;
                match floor_under(g, p.feet) {
                    Ok(node) => {
                        on_floor += 1;
                        Some(node)
                    }
                    Err(class) => {
                        *classes.entry(class).or_insert(0) += 1;
                        out.push_str(&format!("{class},{},{},{:.2},{:.2},{:.2},,,\n", p.body, p.time, p.feet.x, p.feet.y, p.feet.z));
                        None
                    }
                }
            })
            .collect();
        for i in 1..track.len() {
            let (a, b) = (&track[i - 1], &track[i]);
            let flat = a.feet.distance_xz(b.feet);
            if !(is_standing[i - 1] && is_standing[i]) || b.time - a.time > NEAR_MS || !(0.5..=8.0).contains(&flat) {
                continue;
            }
            // Both ends on a floor: a step whose end has none is already a floor miss.
            let (Some(from), Some(to)) = (floors[i - 1], floors[i]) else { continue };
            steps += 1;
            if way_length(g, from, to, 1.5 * (a.feet - b.feet).length() + 1.0).is_some() {
                walkable += 1;
            } else {
                *classes.entry("no_way").or_insert(0) += 1;
                out.push_str(&format!(
                    "no_way,{},{},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2}\n",
                    a.body, a.time, a.feet.x, a.feet.y, a.feet.z, b.feet.x, b.feet.y, b.feet.z
                ));
            }
        }
    }
    let pct = |n: u32, d: u32| 100.0 * f64::from(n) / f64::from(d.max(1));
    let mut by_class: Vec<_> = classes.into_iter().collect();
    by_class.sort();
    eprintln!(
        "SCORE {project} bodies {} standing {standing} floors {on_floor} ({:.2} %) steps {steps} walkable {walkable} ({:.2} %) misses {by_class:?}",
        bodies.len(),
        pct(on_floor, standing),
        pct(walkable, steps),
    );
    if let Some(path) = var("SRP_SCORE_OUT") {
        std::fs::write(&path, out).unwrap_or_else(|e| panic!("{path}: {e}"));
    }
}
