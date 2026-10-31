//! What holds a body stuck under the client's capsule rule (A191): each "is stuck at" note of a bench run, with the route's next
//! waypoint it carries, sorted by the geometry at the spot. A measurement, not a test of the bots: run it on the notes of the runs
//! with the capsule on and off.
//!
//! The classes, in the order they are tried:
//! - `beside`: a body (friend or enemy) within 1.2 m;
//! - `inside`: the spot is within the capsule's reach of a wall (a clearance under 0.36 m): a floor the grid has that the capsule does
//!   not fit;
//! - `line_blocked_waypoint_inside`: the same sweep meets a wall, and the next waypoint itself has no point the capsule fits within
//!   0.5 m (a floor the grid has that the capsule does not fit);
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
    LineBlockedWaypointInside,
    LineBlockedNoWay,
    LineBlockedWay,
    LineFree,
    NoWaypoint,
}

/// What the search for a way for the capsule found.
enum Way {
    Yes,
    No,
    /// The waypoint's surroundings have no point the capsule fits.
    GoalInside,
}

/// What a note with nothing in the sweep to the waypoint is about, by the route's next waypoint and the grid at the spot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum FreeKind {
    /// The next waypoint is 0.6-1.11 m higher: a lift or a standing jump.
    UpLow,
    /// The next waypoint is over 1.11 m higher: a jump up.
    UpHigh,
    /// The next waypoint is 0.5 m or more lower: a drop, the spot a lip floor or near a lip or a fall.
    DownLip,
    /// The same, the spot away from a lip.
    Down,
    /// The spot is a lip floor or near a lip or a fall, on the level.
    Lip,
    /// None of these, within 0.45 m of a wall: pressed along a wall (the slide's speed under the stuck watch's?).
    LevelWall,
    /// None of these, in the open.
    Level,
}

fn clearance(sections: &Sections, segs: &mut Vec<Seg>, at: (f32, f32), y: f32) -> f32 {
    sections.near(at, y, false, 1.0, segs);
    segs.iter().map(|s| nearest(s, at).1).fold(1.0f32, f32::min)
}

/// Whether the capsule can walk from `from` to `to` inside the box: a breadth-first search over the 0.1 m lattice of the points
/// with a floor within 1 m of `y` and a wall clearance of the capsule's radius.
fn way_for_the_capsule(world: &World, sections: &Sections, from: (f32, f32), to: (f32, f32), y: f32) -> Way {
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
    let goals: Vec<(i32, i32)> = near(goal, 5).filter(|&c| fit(c, &mut segs)).collect();
    if goals.is_empty() {
        return Way::GoalInside;
    }
    let mut seen = std::collections::HashSet::new();
    let mut queue = VecDeque::new();
    for c in near(start, 5) {
        if fit(c, &mut segs) && seen.insert(c) {
            queue.push_back(c);
        }
    }
    while let Some(c) = queue.pop_front() {
        if goals.contains(&c) {
            return Way::Yes;
        }
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)] {
            let next = (c.0 + dx, c.1 + dz);
            if !seen.contains(&next) && fit(next, &mut segs) {
                seen.insert(next);
                queue.push_back(next);
            }
        }
    }
    Way::No
}

/// The live alive samples of a map (x, feet, z) that were on the ground, from `SRP_HUMAN_DIR/<level>_<weather>_<n>.tsv`.
fn humans_of(project: &str) -> Vec<[f32; 3]> {
    let Ok(dir) = std::env::var("SRP_HUMAN_DIR") else { return Vec::new() };
    let path = std::path::Path::new(&dir).join(format!("{}.tsv", project.replace('/', "_")));
    let Ok(text) = std::fs::read_to_string(path) else { return Vec::new() };
    text.lines()
        .skip(1)
        .filter_map(|line| {
            let c: Vec<&str> = line.split('\t').collect();
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
    type Row = (String, [f32; 3], Option<[f32; 3]>, bool, String);
    let mut by_project: HashMap<String, Vec<Row>> = HashMap::new();
    for line in std::fs::read_to_string(&notes).unwrap().lines() {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 10 {
            continue;
        }
        let num = |i: usize| f[i].parse::<f32>().unwrap_or(f32::NAN);
        let next = (!num(5).is_nan()).then(|| [num(5), num(6), num(7)]);
        by_project.entry(f[1].to_string()).or_default().push((f[0].to_string(), [num(2), num(3), num(4)], next, f[8] == "1", f.get(11).copied().unwrap_or("?").to_string()));
    }
    let mut totals: HashMap<(String, Cause), u32> = HashMap::new();
    let mut human_totals: HashMap<(String, Cause), u32> = HashMap::new();
    let mut free_totals: HashMap<(String, FreeKind), u32> = HashMap::new();
    let mut level_recovery: HashMap<(String, String), u32> = HashMap::new();
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
        let mut free_per: HashMap<(String, FreeKind), u32> = HashMap::new();
        for (arm, at, next, beside, recovery) in rows {
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
                    sections.near(here, at[1], false, LOOK_M + 1.0, &mut segs);
                    if sweep(&segs, here, (dx * k, dz * k), RADIUS_M).is_some() {
                        match way_for_the_capsule(&world, &sections, here, (next[0], next[2]), at[1]) {
                            Way::Yes => Cause::LineBlockedWay,
                            Way::No => Cause::LineBlockedNoWay,
                            Way::GoalInside => Cause::LineBlockedWaypointInside,
                        }
                    } else {
                        Cause::LineFree
                    }
                }
            } else {
                Cause::NoWaypoint
            };
            if cause == Cause::LineFree {
                let rise = next.map_or(0.0, |n| n[1] - at[1]);
                let node = world.grid.locate(mapdata::Vec3::new(at[0], at[1], at[2]), 1.0);
                let kind = if rise > 1.11 {
                    FreeKind::UpHigh
                } else if rise > 0.6 {
                    FreeKind::UpLow
                } else if rise < -0.5 {
                    if node.is_some_and(|n| world.grid.is_lip(n) || world.grid.near_lip(n) || world.grid.near_fall(n)) {
                        FreeKind::DownLip
                    } else {
                        FreeKind::Down
                    }
                } else if node.is_some_and(|n| world.grid.is_lip(n) || world.grid.near_lip(n) || world.grid.near_fall(n)) {
                    FreeKind::Lip
                } else if clear < 0.45 {
                    FreeKind::LevelWall
                } else {
                    FreeKind::Level
                };
                *free_per.entry((arm.clone(), kind)).or_default() += 1;
                *free_totals.entry((arm.clone(), kind)).or_default() += 1;
                if matches!(kind, FreeKind::Level | FreeKind::LevelWall) {
                    *level_recovery.entry((arm.clone(), recovery.clone())).or_default() += 1;
                }
            }
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
        let mut fkeys: Vec<_> = free_per.keys().cloned().collect();
        fkeys.sort();
        println!("    nothing in the sweep: {}", fkeys.iter().map(|k| format!("{} {:?} {}", k.0, k.1, free_per[k])).collect::<Vec<_>>().join(", "));
    }
    let mut keys: Vec<_> = totals.keys().cloned().collect();
    keys.sort();
    let mut fkeys: Vec<_> = free_totals.keys().cloned().collect();
    fkeys.sort();
    println!("FREE ALL: {}", fkeys.iter().map(|k| format!("{} {:?} {}", k.0, k.1, free_totals[k])).collect::<Vec<_>>().join(", "));
    let mut rkeys: Vec<_> = level_recovery.keys().cloned().collect();
    rkeys.sort();
    println!("FREE LEVEL RECOVERY: {}", rkeys.iter().map(|k| format!("{} {} {}", k.0, k.1, level_recovery[k])).collect::<Vec<_>>().join(", "));
    println!("ALL: {}", keys.iter().map(|k| format!("{} {:?} {} (humans use the spot {})", k.0, k.1, totals[k], human_totals.get(k).copied().unwrap_or(0))).collect::<Vec<_>>().join(", "));
}

/// How many of a map's floors the capsule does not fit, and how many live players stood on such floors (`SRP_PROJECTS="level_04/day_2;.."`,
/// `SRP_MAPDATA_DIR`, `SRP_HUMAN_DIR`): the floors by their wall clearance (under 0.30, 0.30-0.36, 0.36-0.38, 0.38 and over), the
/// live ground samples by the clearance of the floor under them (the nearest within 0.5 m). A measurement.
#[test]
#[ignore = "a measurement; needs SRP_MAPDATA_DIR and SRP_PROJECTS"]
fn grid_floor_clearance() {
    let (Some(maps), Ok(projects)) = (std::env::var_os("SRP_MAPDATA_DIR"), std::env::var("SRP_PROJECTS")) else {
        eprintln!("skipping: SRP_MAPDATA_DIR or SRP_PROJECTS is not set");
        return;
    };
    let band = |c: f32| usize::from(c >= 0.30) + usize::from(c >= 0.36) + usize::from(c >= RADIUS_M);
    for project in projects.split(';').filter(|p| !p.trim().is_empty()) {
        let map = mapdata::MapData::read(&std::path::Path::new(&maps).join(mapdata::format::file_name(project))).unwrap();
        let sections = Sections::new(&map.walker_triangles(), mapdata::Body::client().min_ground_normal_y);
        let links = Tuning::default().grid.off_mesh_links();
        let world = World::build_with_links(map, mapdata::CELL, mapdata::Body::default(), links);
        let grid = &world.grid;
        let mut segs = Vec::new();
        let (mut floors, mut sample_bands) = ([0u32; 4], [0u32; 4]);
        let mut clear_of: Vec<Option<f32>> = Vec::new();
        for f in 0..grid.len() as u32 {
            let p = grid.position(f);
            let c = clearance(&sections, &mut segs, (p.x, p.z), p.y);
            floors[band(c)] += 1;
            clear_of.push(Some(c));
        }
        for h in humans_of(project) {
            let near = grid.floors_in([h[0] - 0.5, h[2] - 0.5, h[0] + 0.5, h[2] + 0.5]).filter(|&f| (grid.position(f).y - h[1]).abs() <= 1.0).min_by(|&a, &b| grid.position(a).distance_xz(Vec3::new(h[0], 0.0, h[2])).total_cmp(&grid.position(b).distance_xz(Vec3::new(h[0], 0.0, h[2]))));
            if let Some(c) = near.and_then(|f| clear_of[f as usize]) {
                sample_bands[band(c)] += 1;
            }
        }
        let share = |a: [u32; 4]| {
            let t: u32 = a.iter().sum();
            a.map(|n| format!("{:.2} %", 100.0 * f64::from(n) / f64::from(t.max(1)))).join(" / ")
        };
        println!("{project}: floors {} (<0.30 / 0.30-0.36 / 0.36-0.38 / >=0.38): {}; live samples on them: {}", grid.len(), share(floors), share(sample_bands));
    }
}

/// A picture of a place the capsule holds bodies at (`SRP_SPOTS="<level_NN/weather_n> <x> <z> <y>;..."`, `SRP_MAPDATA_DIR`, and
/// `SRP_HUMAN_DIR` for the live samples): 5 m square at 0.1 m: `.` where a floor is and the capsule fits (0.38 m from every wall), `x`
/// where a floor is and it does not, a blank where there is no floor; the live players' samples within 1.5 m of it and how near
/// the walls they stood. A measurement.
#[test]
#[ignore = "a measurement; needs SRP_MAPDATA_DIR and SRP_SPOTS"]
fn capsule_spot_report() {
    let (Some(dir), Ok(spots)) = (std::env::var_os("SRP_MAPDATA_DIR").map(std::path::PathBuf::from), std::env::var("SRP_SPOTS")) else {
        eprintln!("skipping: SRP_MAPDATA_DIR or SRP_SPOTS is not set");
        return;
    };
    for spot in spots.split(';').filter(|s| !s.trim().is_empty()) {
        let f: Vec<&str> = spot.split_whitespace().collect();
        let (project, x, z, y) = (f[0], f[1].parse::<f32>().unwrap(), f[2].parse::<f32>().unwrap(), f[3].parse::<f32>().unwrap());
        let map = mapdata::MapData::read(&dir.join(mapdata::format::file_name(project))).unwrap();
        let sections = Sections::new(&map.walker_triangles(), mapdata::Body::client().min_ground_normal_y);
        let links = Tuning::default().grid.off_mesh_links();
        let world = Arc::new(World::build_with_links(map, mapdata::CELL, mapdata::Body::default(), links));
        let grid = &world.grid;
        let clearance = |px: f32, pz: f32, py: f32| {
            let mut segs = Vec::new();
            sections.near((px, pz), py, false, 1.0, &mut segs);
            segs.iter().map(|s| nearest(s, (px, pz)).1).fold(1.0f32, f32::min)
        };
        println!("{project} spot ({x}, {y}, {z}): rows are z {:.1}..{:.1} (down), columns x {:.1}..{:.1}", z - 2.5, z + 2.5, x - 2.5, x + 2.5);
        for k in 0..=50 {
            let pz = z - 2.5 + k as f32 * 0.1;
            let mut row = String::new();
            for i in 0..=50 {
                let px = x - 2.5 + i as f32 * 0.1;
                let floor = grid.floors_in([px, pz, px, pz]).map(|n| grid.position(n).y).find(|fy| (fy - y).abs() <= 2.0);
                row.push(match floor {
                    None => ' ',
                    Some(fy) if clearance(px, pz, fy) >= RADIUS_M => '.',
                    Some(_) => 'x',
                });
            }
            if !row.trim().is_empty() {
                println!("{pz:7.1} {row}");
            }
        }
        if let Ok(humans) = std::env::var("SRP_HUMAN_DIR") {
            let stem = project.replace('/', "_");
            let path = std::path::Path::new(&humans).join(format!("{stem}.tsv"));
            let (mut n, mut moving, mut least) = (0, 0, f32::MAX);
            if let Ok(text) = std::fs::read_to_string(&path) {
                for line in text.lines().skip(1) {
                    let c: Vec<&str> = line.split('\t').collect();
                    if c.len() < 10 {
                        continue;
                    }
                    let (Ok(sx), Ok(sy), Ok(sz), Ok(speed)) = (c[3].parse::<f32>(), c[4].parse::<f32>(), c[5].parse::<f32>(), c[9].parse::<f32>()) else { continue };
                    if (sx - x).hypot(sz - z) <= 1.5 && (sy - y).abs() <= 1.5 {
                        n += 1;
                        moving += usize::from(speed > 1.0);
                        least = least.min(clearance(sx, sz, sy));
                    }
                }
            }
            println!("  live samples within 1.5 m: {n} ({moving} over 1 m/s), the least wall clearance among them {least:.2} m");
        }
    }
}
