//! What holds a body stuck under the client's capsule rule (A191): each "is stuck at" note of a bench run, with the route's next
//! waypoint it carries, sorted by the geometry at the spot. A measurement, not a test of the bots: run it on the notes of the runs
//! with the capsule on and off.
//!
//! The classes, in the order they are tried:
//! - `beside`: a body (friend or enemy) within 1.2 m;
//! - `inside`: the spot is within the capsule's reach of a wall (a clearance under 0.36 m): a floor the grid has that the capsule does
//!   not fit;
//! - `line_blocked_no_way`: the capsule swept from the spot toward the next waypoint meets a wall within 3 m of it, and there is no
//!   way for the capsule (clearance 0.38 m or more at every 0.1 m step of a floor) from the spot to the waypoint inside a 8 m square:
//!   the route goes through a gap narrower than the capsule;
//! - `line_blocked_way`: the same sweep meets a wall, but a way for the capsule exists: the body steers into a wall it could go round
//!   (a corner cut by a straight line, a door jamb);
//! - `line_free`: nothing in the sweep: the cause is not the wall (a ledge, a jump, a lift, the recovery itself);
//! - `no_waypoint`: the note names none.
//!
//! Each note is also counted `humans` when live players have stood within 1 m of the spot (5 or more samples of the 12+ human matches
//! at the same height, `SRP_HUMAN_DIR`): a place players use.
//!
//! `SRP_MAPDATA_DIR=<maps> SRP_STUCK_NOTES=<tsv> cargo test -p match-server --release stuck_causes -- --ignored --nocapture`, the tsv of
//! `arm<TAB>project<TAB>x<TAB>y<TAB>z<TAB>nx<TAB>ny<TAB>nz<TAB>beside<TAB>heading_off` per note (`nan` for no waypoint).

use std::collections::{HashMap, VecDeque};

use super::*;
use super::standin_capsule::{RADIUS_M, Seg, Sections, nearest, sweep};

/// How near a wall the spot of a body "inside" is, m.
const INSIDE_M: f32 = 0.36;
/// The sweep toward the next waypoint, m.
const LOOK_M: f32 = 3.0;
/// The square the way is looked for in, half its side, m, and its step.
const BOX_M: f32 = 4.0;
const STEP_M: f32 = 0.1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum Cause {
    Beside,
    Inside,
    LineBlockedNoWay,
    LineBlockedWay,
    LineFree,
    NoWaypoint,
}

fn clearance(sections: &Sections, segs: &mut Vec<Seg>, at: (f32, f32), y: f32) -> f32 {
    sections.near(at, y, 1.0, segs);
    segs.iter().map(|s| nearest(s, at).1).fold(1.0f32, f32::min)
}

/// Whether the capsule can walk from `from` to `to` inside the box: a breadth-first search over the 0.1 m lattice of the points
/// with a floor within 1 m of `y` and a wall clearance of the capsule's radius.
fn way_for_the_capsule(world: &World, sections: &Sections, from: (f32, f32), to: (f32, f32), y: f32) -> bool {
    let grid = &world.grid;
    let n = (2.0 * BOX_M / STEP_M) as i32 + 1;
    let cell = |p: (f32, f32)| (((p.0 - (from.0 - BOX_M)) / STEP_M).round() as i32, ((p.1 - (from.1 - BOX_M)) / STEP_M).round() as i32);
    let point = |c: (i32, i32)| (from.0 - BOX_M + c.0 as f32 * STEP_M, from.1 - BOX_M + c.1 as f32 * STEP_M);
    let mut segs = Vec::new();
    let mut fits: HashMap<(i32, i32), bool> = HashMap::new();
    let mut fit = |c: (i32, i32), segs: &mut Vec<Seg>| -> bool {
        if c.0 < 0 || c.1 < 0 || c.0 >= n || c.1 >= n {
            return false;
        }
        *fits.entry(c).or_insert_with(|| {
            let (px, pz) = point(c);
            grid.floors_in([px, pz, px, pz]).any(|f| (grid.position(f).y - y).abs() <= 1.0) && clearance(sections, segs, (px, pz), y) >= RADIUS_M - 0.01
        })
    };
    // The spot itself may be inside a wall's reach: start from the fitting cells within 0.5 m of it, end within 0.5 m of the waypoint.
    let (start, goal) = (cell(from), cell(to));
    let near = |c: (i32, i32), r: i32| (-r..=r).flat_map(move |dx| (-r..=r).map(move |dz| (c.0 + dx, c.1 + dz)));
    let goals: Vec<(i32, i32)> = near(goal, 5).collect();
    let mut seen = std::collections::HashSet::new();
    let mut queue = VecDeque::new();
    for c in near(start, 5) {
        if fit(c, &mut segs) && seen.insert(c) {
            queue.push_back(c);
        }
    }
    while let Some(c) = queue.pop_front() {
        if goals.contains(&c) {
            return true;
        }
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)] {
            let next = (c.0 + dx, c.1 + dz);
            if !seen.contains(&next) && fit(next, &mut segs) {
                seen.insert(next);
                queue.push_back(next);
            }
        }
    }
    false
}

/// The live alive samples of a map (x, feet, z) that were on the ground, from `SRP_HUMAN_DIR/<level>_<weather>_<n>.tsv`.
fn humans_of(project: &str) -> Vec<[f32; 3]> {
    let Ok(dir) = std::env::var("SRP_HUMAN_DIR") else { return Vec::new() };
    let path = std::path::Path::new(&dir).join(format!("{}.tsv", project.replace('/', "_")));
    let Ok(text) = std::fs::read_to_string(path) else { return Vec::new() };
    text.lines()
        .skip(1)
        .filter_map(|line| {
            let c: Vec<&str> = line.split('	').collect();
            (c.len() >= 10 && c[8] == "0").then(|| Some([c[3].parse().ok()?, c[4].parse().ok()?, c[5].parse().ok()?])).flatten()
        })
        .collect()
}

#[test]
#[ignore = "a measurement; needs SRP_MAPDATA_DIR and SRP_STUCK_NOTES"]
fn stuck_causes() {
    let (Some(maps), Ok(notes)) = (std::env::var_os("SRP_MAPDATA_DIR"), std::env::var("SRP_STUCK_NOTES")) else {
        eprintln!("skipping: SRP_MAPDATA_DIR or SRP_STUCK_NOTES is not set");
        return;
    };
    type Row = (String, [f32; 3], Option<[f32; 3]>, bool);
    let mut by_project: HashMap<String, Vec<Row>> = HashMap::new();
    for line in std::fs::read_to_string(&notes).unwrap().lines() {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 10 {
            continue;
        }
        let num = |i: usize| f[i].parse::<f32>().unwrap_or(f32::NAN);
        let next = (!num(5).is_nan()).then(|| [num(5), num(6), num(7)]);
        by_project.entry(f[1].to_string()).or_default().push((f[0].to_string(), [num(2), num(3), num(4)], next, f[8] == "1"));
    }
    let mut totals: HashMap<(String, Cause), u32> = HashMap::new();
    let mut human_totals: HashMap<(String, Cause), u32> = HashMap::new();
    let mut projects: Vec<_> = by_project.into_iter().collect();
    projects.sort_by(|a, b| a.0.cmp(&b.0));
    for (project, rows) in projects {
        let map = mapdata::MapData::read(&std::path::Path::new(&maps).join(mapdata::format::file_name(&project))).unwrap();
        let sections = Sections::new(&map.walker_triangles(), mapdata::Body::client().min_ground_normal_y);
        let links = Tuning::default().grid.off_mesh_links();
        let world = World::build_with_links(map, mapdata::CELL, mapdata::Body::default(), links);
        let mut segs = Vec::new();
        let mut per: HashMap<(String, Cause), u32> = HashMap::new();
        let humans = humans_of(&project);
        for (arm, at, next, beside) in rows {
            let here = (at[0], at[2]);
            let clear = clearance(&sections, &mut segs, here, at[1]);
            let cause = if beside {
                Cause::Beside
            } else if clear < INSIDE_M {
                Cause::Inside
            } else if let Some(next) = next {
                let (dx, dz) = (next[0] - here.0, next[2] - here.1);
                let len = dx.hypot(dz);
                if len < 1e-3 {
                    Cause::LineFree
                } else {
                    let k = (LOOK_M / len).min(1.0);
                    sections.near(here, at[1], LOOK_M + 1.0, &mut segs);
                    if sweep(&segs, here, (dx * k, dz * k), RADIUS_M).is_some() {
                        if way_for_the_capsule(&world, &sections, here, (next[0], next[2]), at[1]) { Cause::LineBlockedWay } else { Cause::LineBlockedNoWay }
                    } else {
                        Cause::LineFree
                    }
                }
            } else {
                Cause::NoWaypoint
            };
            let used = humans.iter().filter(|h| (h[0] - at[0]).hypot(h[2] - at[2]) <= 1.0 && (h[1] - at[1]).abs() <= 1.5).count() >= 5;
            if used {
                *human_totals.entry((arm.clone(), cause)).or_default() += 1;
            }
            *per.entry((arm.clone(), cause)).or_default() += 1;
            *totals.entry((arm, cause)).or_default() += 1;
        }
        let mut keys: Vec<_> = per.keys().cloned().collect();
        keys.sort();
        println!("{project}: {}", keys.iter().map(|k| format!("{} {:?} {}", k.0, k.1, per[k])).collect::<Vec<_>>().join(", "));
    }
    let mut keys: Vec<_> = totals.keys().cloned().collect();
    keys.sort();
    println!("ALL: {}", keys.iter().map(|k| format!("{} {:?} {} (humans use the spot {})", k.0, k.1, totals[k], human_totals.get(k).copied().unwrap_or(0))).collect::<Vec<_>>().join(", "));
}
