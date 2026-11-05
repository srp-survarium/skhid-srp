//! The standing bot of Chemical Plant 4v4 batteries seed 4 (fx-grid, register #27c): from (25.1, 2.0, 2.9) to (72.4, 3.5, 54.4) the floors' search found no
//! way for 84 s, `crossing::wanted` said no 163 times, and the crossing was never searched. Which stage fails: the plain floor search, the regions' walk (zones shut /
//! open), `only_across_fixed`, the crossing search. A measurement on the real map, run with
//! `SRP_MAPDATA_DIR=<mapdata folder> cargo test -p match-server --release chemical_plant_neck -- --ignored --nocapture`.

use super::*;
use crate::v026g::bots::crossing;
use crate::v026g::bots::path;
use crate::v026g::bots::route_filter::RouteFilter;
use crate::v026g::bots::tactics::Tactics;

#[test]
#[ignore = "a measurement; needs SRP_MAPDATA_DIR"]
fn chemical_plant_neck_stage_by_stage() {
    let Some(dir) = std::env::var_os("SRP_MAPDATA_DIR").map(std::path::PathBuf::from) else {
        eprintln!("skipping: SRP_MAPDATA_DIR is not set");
        return;
    };
    let map = mapdata::MapData::read(&dir.join(mapdata::format::file_name("level_04/day_2"))).unwrap();
    let links = Tuning::default().off_mesh_links();
    let world = Arc::new(World::build_with_links(map, mapdata::CELL, mapdata::Body::default(), links));
    let mut tactics = Tactics::new(world.clone(), Tuning::default().hazard.margin_m.get());
    tactics.hazards.attach(crate::v026g::bots::anomalies::Anomalies::new(&world.map, 0x8000_0001));
    let grid = &world.grid;
    let (a, b) = (Vec3::new(25.1, 2.0, 2.9), Vec3::new(72.4, 3.5, 54.4));
    let (from, to) = (grid.locate(a, 1.5).expect("start floor"), grid.locate(b, 1.5).expect("goal floor"));
    println!("from {from} at {:?}, to {to} at {:?}", grid.position(from), grid.position(to));
    println!("zones: {:?}", tactics.hazards.zones.iter().map(|z| (z.zone, z.name.clone(), z.switches(), z.hit_per_s())).collect::<Vec<_>>());
    let side = Some(1);
    println!("start floor allowed {}, marked {}, fixed_at {}", tactics.allowed(from, 0, side, false), tactics.hazards.marked_at(from), tactics.hazards.fixed_at(from));
    // 1. the plain floor search
    let plain = tactics.route_nodes(from, to, 0, side, false, &RouteFilter::open());
    println!("1 plain floor search: {}", plain.as_ref().map_or("NO WAY".to_owned(), |r| format!("a route of {:.1} m", r.length())));
    // the same with every floor open: the way there is
    let open = path::plan_where(grid, from, to, |_| true);
    println!("  with every floor open: {}", open.as_ref().map_or("none".to_owned(), |r| format!("{:.1} m, {} corners", r.length(), r.points().len())));
    if let Some(open) = &open {
        let mut shut = 0;
        let mut fixed = 0;
        let mut first = Vec::new();
        for w in open.points() {
            if !tactics.allowed(w.node, 0, side, false) {
                shut += 1;
                if first.len() < 6 {
                    first.push((w.at.x, w.at.y, w.at.z));
                }
            }
            if tactics.hazards.fixed_at(w.node) {
                fixed += 1;
            }
        }
        println!("  its corners: {} shut to the planner, {} on a zone floor on all match; first shut: {first:?}", shut, fixed);
    }
    // 2. the regions' walk, zones shut / open
    let regions = tactics.regions();
    let (start, goal) = (regions.of(from), regions.of(to));
    let reach = |pass: &dyn Fn(NodeId) -> bool| {
        let mut seen = vec![false; regions.len()];
        seen[start as usize] = true;
        let mut queue = vec![start];
        while let Some(r) = queue.pop() {
            if r == goal {
                return true;
            }
            for &(n, _) in regions.out(r) {
                let middle = regions.middle(n);
                if !seen[n as usize] && (n == goal || (tactics.spawn_ok(middle, side, false) && pass(middle))) {
                    seen[n as usize] = true;
                    queue.push(n);
                }
            }
        }
        false
    };
    let shut_reach = reach(&|m| !tactics.hazards.fixed_at(m));
    let open_reach = reach(&|_| true);
    println!("2 regions ({} of them): start region {start}, goal region {goal}: reach with the zones shut {shut_reach}, with all open {open_reach}", regions.len());
    // 3. only_across_fixed and wanted
    let across = tactics.only_across_fixed(from, to, side, false, |m| crossing::crossable(&tactics, m, 1.0));
    println!("3 only_across_fixed {across}; wanted {}", crossing::wanted(&tactics, None, from, to, side, false, 1.0));
    // 4. the crossing search
    let mut floors = usize::MAX;
    let crossed = crossing::start(&tactics, from, to, 0, side, false, 1.0, &RouteFilter::open(), &mut floors);
    println!("4 crossing search: {}", match crossed { Ok(Some(r)) => format!("a route of {:.1} m", r.length()), Ok(None) => "NO WAY".to_owned(), Err(_) => "unfinished".to_owned() });
    println!("  floors expanded: {}", usize::MAX - floors);
}

/// The Vostok Radar Station carriers that stood (register #27c, the NOWAYGOAL lines): from three spots on the west side to their container (38.2, 0.4, 4.3), team 1. The
/// zones of the map, their kind and schedule (three seeds), and for each start and a few match times: the plain floor search, the floors' open route, the crossing search.
#[test]
#[ignore = "a measurement; needs SRP_MAPDATA_DIR"]
fn vostok_carriers_stage_by_stage() {
    let Some(dir) = std::env::var_os("SRP_MAPDATA_DIR").map(std::path::PathBuf::from) else {
        eprintln!("skipping: SRP_MAPDATA_DIR is not set");
        return;
    };
    let map = mapdata::MapData::read(&dir.join(mapdata::format::file_name("level_03/day_2"))).unwrap();
    println!("containers: {:?}", map.containers.iter().map(|c| (c.id, c.team, c.use_point())).collect::<Vec<_>>());
    let links = Tuning::default().off_mesh_links();
    let world = Arc::new(World::build_with_links(map, mapdata::CELL, mapdata::Body::default(), links));
    let grid = &world.grid;
    let goal = Vec3::new(38.2, 0.4, 4.3);
    let starts = [Vec3::new(-29.8, 6.4, -4.0), Vec3::new(-26.8, 5.9, -2.7), Vec3::new(-35.1, 6.3, -4.0)];
    let side = Some(1);
    for seed in [0x8000_0001u32, 0x8000_0abc, 0x8000_7777] {
        let mut tactics = Tactics::new(world.clone(), Tuning::default().hazard.margin_m.get());
        tactics.hazards.attach(crate::v026g::bots::anomalies::Anomalies::new(&world.map, seed));
        println!("== seed {seed:#x}");
        for z in tactics.hazards.zones.iter() {
            let spans = match &z.activity {
                crate::v026g::bots::hazards::Activity::Always => "always".to_owned(),
                crate::v026g::bots::hazards::Activity::Spans(s) => format!("{:?}", &s[..s.len().min(5)]),
            };
            println!("zone {} {} control {:?} switches {} hit {} at ({:.1},{:.1},{:.1}) activity {spans}", z.zone, z.name, z.control, z.switches(), z.hit_per_s(), z.centre.x, z.centre.y, z.centre.z);
        }
        let to = grid.locate(goal, 1.5).expect("goal floor");
        for a in starts {
            let from = grid.locate(a, 1.5).expect("start floor");
            for time in [0u32, 40_000, 80_000, 120_000] {
                let plain = tactics.route_nodes(from, to, time, side, false, &RouteFilter::open());
                let open = path::plan_where(grid, from, to, |n| tactics.spawn_ok(n, side, false));
                let mut floors = usize::MAX;
                let crossed = crossing::start(&tactics, from, to, time, side, false, 1.0, &RouteFilter::open(), &mut floors);
                let shut = open.as_ref().map_or(0, |r| r.points().iter().filter(|w| !tactics.allowed(w.node, time, side, false)).count());
                println!(
                    "from ({:.1},{:.1},{:.1}) t {time}: plain {}; open-but-spawns {}, its corners shut at t: {shut}; crossing {}; wanted {}",
                    a.x, a.y, a.z,
                    plain.as_ref().map_or("NO WAY".to_owned(), |r| format!("{:.0} m", r.length())),
                    open.as_ref().map_or("none".to_owned(), |r| format!("{:.0} m", r.length())),
                    match crossed { Ok(Some(r)) => format!("{:.0} m", r.length()), Ok(None) => "NO WAY".to_owned(), Err(_) => "unfinished".to_owned() },
                    crossing::wanted(&tactics, plain.as_ref(), from, to, side, false, 1.0),
                );
            }
        }
    }
}

/// The exits off Vostok Radar Station's antenna plateau that the players' worlds (Oct 12-14, 19 matches, 5 s) show, and whether the bots' grid and planner have them.
/// For each: the path from the carriers' start to the plateau side of the exit with the zones shut at t = 0 s and ignoring zones, and the path from there to the
/// low ground. Then a map of the plateau at feet 5.4-6.9 from the start: `R` reached with the zones shut, `o` only ignoring zones, `x` a floor neither reaches,
/// `Z` a floor a zone marks, `.` no floor.
#[test]
#[ignore = "a measurement; needs SRP_MAPDATA_DIR"]
fn vostok_exits_by_stage() {
    let Some(dir) = std::env::var_os("SRP_MAPDATA_DIR").map(std::path::PathBuf::from) else {
        eprintln!("skipping: SRP_MAPDATA_DIR is not set");
        return;
    };
    let map = mapdata::MapData::read(&dir.join(mapdata::format::file_name("level_03/day_2"))).unwrap();
    let links = Tuning::default().off_mesh_links();
    let world = Arc::new(World::build_with_links(map, mapdata::CELL, mapdata::Body::default(), links));
    let grid = &world.grid;
    let mut tactics = Tactics::new(world.clone(), Tuning::default().hazard.margin_m.get());
    tactics.hazards.attach(crate::v026g::bots::anomalies::Anomalies::new(&world.map, 0x8000_0001));
    for z in tactics.hazards.zones.iter().filter(|z| z.switches()) {
        println!("zone {} {} shapes {:?}", z.zone, z.name, z.shapes.iter().map(|s| (s.kind, s.center, s.half)).collect::<Vec<_>>());
    }
    let start = grid.locate(Vec3::new(-29.8, 6.4, -4.0), 1.5).expect("start floor");
    let shut = |n: NodeId| tactics.allowed(n, 0, Some(1), false);
    let open = |n: NodeId| tactics.spawn_ok(n, Some(1), false);
    let any = |_: NodeId| true;
    let exits: [(&str, [f32; 3], [f32; 3]); 8] = [
        ("north-west", [-42.4, 5.6, -33.2], [-52.7, 2.4, -45.1]),
        ("north-mid", [-36.1, 5.4, -33.0], [-39.2, 2.0, -49.1]),
        ("north-mid2", [-34.5, 5.5, -36.0], [-40.2, 1.9, -46.7]),
        ("north-east", [-14.1, 5.4, -33.1], [-5.1, 1.9, -50.4]),
        ("north-east2", [-7.3, 5.4, -35.3], [-15.8, 0.5, -53.1]),
        ("east1", [-8.2, 5.9, -7.6], [12.0, 0.6, -6.0]),
        ("east2", [-6.9, 5.6, -29.0], [12.4, 0.6, -23.1]),
        ("east3", [-6.5, 5.4, -1.4], [14.9, 0.5, 2.5]),
    ];
    let len = |p: Option<mapdata::grid::Path>| p.map_or("none".to_owned(), |p| format!("{} floors", p.nodes.len()));
    for (name, a, b) in exits {
        let (a, b) = (Vec3::new(a[0], a[1], a[2]), Vec3::new(b[0], b[1], b[2]));
        let (na, nb) = (grid.locate(a, 2.0), grid.locate(b, 2.0));
        let (Some(na), Some(nb)) = (na, nb) else {
            println!("{name}: floor not found (deck {na:?}, low {nb:?})");
            continue;
        };
        println!(
            "{name}: deck ({:.1},{:.1},{:.1}) low ({:.1},{:.1},{:.1}); start->deck zones shut {}, spawn-only {}; deck->low {}; start->low shut {}, spawn-only {}",
            grid.position(na).x, grid.position(na).y, grid.position(na).z, grid.position(nb).x, grid.position(nb).y, grid.position(nb).z,
            len(grid.path_where(start, na, shut)), len(grid.path_where(start, na, open)),
            len(grid.path_where(na, nb, any)),
            len(grid.path_where(start, nb, shut)), len(grid.path_where(start, nb, open)),
        );
    }
    // What shuts the way: the floors of the spawn-only path from the start that the zones shut at t = 0 s, with the zones that mark them.
    let spots: [(&str, [f32; 3]); 6] = [
        ("north of the pocket (-34,5.9,-14)", [-34.0, 5.9, -14.0]),
        ("north (-31,5.9,-14)", [-31.0, 5.9, -14.0]),
        ("west (-44,5.5,-8)", [-44.0, 5.5, -8.0]),
        ("south (-28,5.3,5)", [-28.0, 5.3, 5.0]),
        ("east1 deck (-8.3,5.9,-7.5)", [-8.3, 5.9, -7.5]),
        ("north-mid deck (-36,5.4,-33)", [-36.0, 5.4, -33.0]),
    ];
    for (name, at) in spots {
        let Some(node) = grid.locate(Vec3::new(at[0], at[1], at[2]), 2.0) else {
            println!("{name}: no floor");
            continue;
        };
        let Some(path) = grid.path_where(start, node, open) else {
            println!("{name}: no spawn-only path either");
            continue;
        };
        let mut cut: Vec<String> = Vec::new();
        for &n in &path.nodes {
            if !shut(n) {
                let at = grid.position(n);
                let zones: Vec<String> = tactics.hazards.zones_at(n).map(|z| format!("{}:{}", z.zone, z.activity.active(0))).collect();
                cut.push(format!("({:.1},{:.1},{:.1}) {:?}", at.x, at.y, at.z, zones));
            }
        }
        println!("{name}: spawn-only path {} floors, {} of them shut at t=0: {}", path.nodes.len(), cut.len(), cut.iter().take(6).cloned().collect::<Vec<_>>().join("; "));
    }
    println!("pocket's north edge, x from -40 to -24 across, z from -14 (top) to -2, 0.5 m cells; Z marked by a zone (the digit its first zone), R reached shut, o spawn-only, x floor neither, . none");
    for zi in 0..25 {
        let z = -14.0 + 0.5 * zi as f32;
        let mut row = String::new();
        for xi in 0..33 {
            let x = -40.0 + 0.5 * xi as f32;
            let node = [5.9f32, 5.5, 6.2, 5.2].iter().find_map(|&y| grid.locate(Vec3::new(x, y, z), 0.3));
            row.push(match node {
                None => '.',
                Some(n) if tactics.hazards.marked_at(n) => char::from_digit(tactics.hazards.zone_of(n).map_or(0, |zone| zone.zone as u32 % 10), 10).unwrap_or('Z'),
                Some(n) if grid.path_where(start, n, shut).is_some() => 'R',
                Some(n) if grid.path_where(start, n, open).is_some() => 'o',
                Some(_) => 'x',
            });
        }
        println!("{z:6.1} {row}");
    }
    println!("plateau map, x from -50 to -4 across, z from -40 (top) to 10 (bottom), 2 m cells");
    for zi in 0..25 {
        let z = -40.0 + 2.0 * zi as f32;
        let mut row = String::new();
        for xi in 0..24 {
            let x = -50.0 + 2.0 * xi as f32;
            let node = [6.0f32, 5.6, 6.5, 6.9].iter().find_map(|&y| grid.locate(Vec3::new(x, y, z), 0.7));
            row.push(match node {
                None => '.',
                Some(n) if tactics.hazards.marked_at(n) => 'Z',
                Some(n) if grid.path_where(start, n, shut).is_some() => 'R',
                Some(n) if grid.path_where(start, n, open).is_some() => 'o',
                Some(_) => 'x',
            });
        }
        println!("{z:6.1} {row}");
    }
}

/// Every vertical move of the players' worlds on Vostok Radar Station (`SRP_VERTICAL_TSV`: variant, match, place, t0 ms, dt ms, from x y z (feet), to x y z) against
/// the bots' grid: the path between the two floors ignoring hazards, its length along the ground, the drops and the jumps on it. A human move whose path is longer than
/// he can walk in the time (6 m/s), or has no path, took a shortcut the grid lacks or the planner prices out. One `ROW` line each.
#[test]
#[ignore = "a measurement; needs SRP_MAPDATA_DIR and SRP_VERTICAL_TSV"]
fn vostok_human_shortcuts() {
    let (Some(dir), Some(tsv)) = (std::env::var_os("SRP_MAPDATA_DIR"), std::env::var_os("SRP_VERTICAL_TSV")) else {
        eprintln!("skipping: SRP_MAPDATA_DIR or SRP_VERTICAL_TSV is not set");
        return;
    };
    let map = mapdata::MapData::read(&std::path::PathBuf::from(dir).join(mapdata::format::file_name("level_03/day_2"))).unwrap();
    let links = Tuning::default().off_mesh_links();
    let world = Arc::new(World::build_with_links(map, mapdata::CELL, mapdata::Body::default(), links));
    let grid = &world.grid;
    let any = |_: NodeId| true;
    for line in std::fs::read_to_string(tsv).unwrap().lines() {
        let f: Vec<&str> = line.split(char::from(9)).collect();
        let n = |i: usize| f[i].parse::<f32>().unwrap();
        let (a, b) = (Vec3::new(n(5), n(6), n(7)), Vec3::new(n(8), n(9), n(10)));
        let dt = n(4) / 1000.0;
        let (Some(na), Some(nb)) = (grid.locate(a, 1.5), grid.locate(b, 1.5)) else {
            println!("ROW	{}	{}	{}	{}	{:.1}	{:.1}	{:.1}	{:.1}	{:.1}	{:.1}	nofloor", f[0], f[1], f[2], f[3], a.x, a.y, a.z, b.x, b.y, b.z);
            continue;
        };
        let walked = match grid.path_where(na, nb, any) {
            None => "nopath".to_owned(),
            Some(path) => {
                let (mut len, mut drop, mut big, mut jumps) = (0.0f32, 0.0f32, 0.0f32, 0u32);
                for w in path.nodes.windows(2) {
                    let (p, q) = (grid.position(w[0]), grid.position(w[1]));
                    let dxz = p.distance_xz(q);
                    len += dxz;
                    if dxz > 0.6 {
                        jumps += 1;
                    }
                    let dy = p.y - q.y;
                    if dy > 0.8 {
                        drop += dy;
                        big = big.max(dy);
                    }
                }
                format!("{len:.1}	{drop:.1}	{big:.1}	{jumps}")
            }
        };
        println!(
            "ROW	{}	{}	{}	{}	{:.1}	{:.1}	{:.1}	{:.1}	{:.1}	{:.1}	{dt:.1}	{walked}",
            f[0], f[1], f[2], f[3], a.x, a.y, a.z, b.x, b.y, b.z
        );
    }
}

/// The players' moves on and off Vostok Radar Station's plateau (`SRP_MOVES_TSV`, as the vertical moves' file) against what the carriers' start reaches with the zones
/// shut at `SRP_AT_MS` (0): for each move whose first end the start reaches and second it does not, the zones that mark the floors of the open path between them, and
/// whether the bots' grid has the path at all. A move with no zone floor on its open path is cut by something else.
#[test]
#[ignore = "a measurement; needs SRP_MAPDATA_DIR and SRP_MOVES_TSV"]
fn vostok_pocket_exits() {
    let (Some(dir), Some(tsv)) = (std::env::var_os("SRP_MAPDATA_DIR"), std::env::var_os("SRP_MOVES_TSV")) else {
        eprintln!("skipping: SRP_MAPDATA_DIR or SRP_MOVES_TSV is not set");
        return;
    };
    let at_ms: u32 = std::env::var("SRP_AT_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
    let map = mapdata::MapData::read(&std::path::PathBuf::from(dir).join(mapdata::format::file_name("level_03/day_2"))).unwrap();
    let links = Tuning::default().off_mesh_links();
    let world = Arc::new(World::build_with_links(map, mapdata::CELL, mapdata::Body::default(), links));
    let grid = &world.grid;
    let mut tactics = Tactics::new(world.clone(), Tuning::default().hazard.margin_m.get());
    tactics.hazards.attach(crate::v026g::bots::anomalies::Anomalies::new(&world.map, 0x8000_0001));
    let start = grid.locate(Vec3::new(-29.8, 6.4, -4.0), 1.5).expect("start floor");
    let shut = |n: NodeId| tactics.allowed(n, at_ms, Some(1), false);
    let open = |n: NodeId| tactics.spawn_ok(n, Some(1), false);
    let mut reached: std::collections::HashMap<NodeId, bool> = std::collections::HashMap::new();
    let mut reach = |n: NodeId| *reached.entry(n).or_insert_with(|| grid.path_where(start, n, shut).is_some());
    let (mut moves, mut out_of, mut into, mut inside) = (0u32, 0u32, 0u32, 0u32);
    for line in std::fs::read_to_string(tsv).unwrap().lines() {
        let f: Vec<&str> = line.split(char::from(9)).collect();
        let n = |i: usize| f[i].parse::<f32>().unwrap();
        let (a, b) = (Vec3::new(n(5), n(6), n(7)), Vec3::new(n(8), n(9), n(10)));
        let (Some(na), Some(nb)) = (grid.locate(a, 1.0), grid.locate(b, 1.0)) else { continue };
        moves += 1;
        let (ra, rb) = (reach(na), reach(nb));
        match (ra, rb) {
            (true, true) => inside += 1,
            (false, true) => {
                into += 1;
                let desc = match grid.path_where(na, nb, open) {
                    None => "no path in the grid".to_owned(),
                    Some(p) => {
                        let (mut len, mut cut) = (0.0f32, 0u32);
                        for w in p.nodes.windows(2) {
                            len += grid.position(w[0]).distance_xz(grid.position(w[1]));
                        }
                        let mut zones: std::collections::BTreeSet<usize> = std::collections::BTreeSet::new();
                        for &x in &p.nodes {
                            if !shut(x) {
                                cut += 1;
                                zones.extend(tactics.hazards.zones_at(x).map(|z| z.zone));
                            }
                        }
                        format!("grid path {len:.1} m, {cut} shut floors, zones {zones:?}, human line {:.1} m", a.distance_xz(b))
                    }
                };
                println!("IN	{}	{}	p{}	t{}	({:.1},{:.1},{:.1})	({:.1},{:.1},{:.1})	dt {}	{desc}", f[0], f[1], f[2], f[3], a.x, a.y, a.z, b.x, b.y, b.z, f[4]);
            }
            (false, false) => {}
            (true, false) => {
                out_of += 1;
                let path = grid.path_where(na, nb, open);
                let desc = match path {
                    None => "no path in the grid".to_owned(),
                    Some(p) => {
                        let mut zones: std::collections::BTreeMap<usize, u32> = std::collections::BTreeMap::new();
                        let mut cut = 0u32;
                        for &x in &p.nodes {
                            if !shut(x) {
                                cut += 1;
                                for z in tactics.hazards.zones_at(x) {
                                    *zones.entry(z.zone).or_default() += 1;
                                }
                            }
                        }
                        format!("{} floors, {cut} shut, zones {zones:?}", p.nodes.len())
                    }
                };
                println!("OUT	{}	{}	p{}	t{}	({:.1},{:.1},{:.1})	({:.1},{:.1},{:.1})	dt {}	{desc}", f[0], f[1], f[2], f[3], a.x, a.y, a.z, b.x, b.y, b.z, f[4]);
            }
        }
    }
    println!("SUMMARY at {at_ms} ms: moves {moves}, inside the reached set {inside}, into it from outside {into}, out of it {out_of}");
}

/// Did the players cross Vostok Radar Station's zone 14 while it was on? The moves of `SRP_MOVES_TSV` (full match ids) whose ends are on different sides of the set the
/// carriers' start reaches with the zones shut (at 0 s), and the state of zone 14 in that match at the move's two ends: the cores run from the match's static seed
/// (`id | 0x8000_0000`) on the world clock, exact until an artefact is picked up (`anomalies.rs`). One `CROSS` line each.
#[test]
#[ignore = "a measurement; needs SRP_MAPDATA_DIR and SRP_MOVES_TSV"]
fn vostok_gate_crossings() {
    let (Some(dir), Some(tsv)) = (std::env::var_os("SRP_MAPDATA_DIR"), std::env::var_os("SRP_MOVES_TSV")) else {
        eprintln!("skipping: SRP_MAPDATA_DIR or SRP_MOVES_TSV is not set");
        return;
    };
    let map = mapdata::MapData::read(&std::path::PathBuf::from(dir).join(mapdata::format::file_name("level_03/day_2"))).unwrap();
    let links = Tuning::default().off_mesh_links();
    let world = Arc::new(World::build_with_links(map, mapdata::CELL, mapdata::Body::default(), links));
    let grid = &world.grid;
    let mut tactics = Tactics::new(world.clone(), Tuning::default().hazard.margin_m.get());
    tactics.hazards.attach(crate::v026g::bots::anomalies::Anomalies::new(&world.map, 0x8000_0001));
    let start = grid.locate(Vec3::new(-29.8, 6.4, -4.0), 1.5).expect("start floor");
    let shut = |n: NodeId| tactics.allowed(n, 0, Some(1), false);
    let mut reached: std::collections::HashMap<NodeId, bool> = std::collections::HashMap::new();
    let mut reach = |n: NodeId| *reached.entry(n).or_insert_with(|| grid.path_where(start, n, shut).is_some());
    let zone14 = tactics.hazards.zones.iter().find(|z| z.name.ends_with("coridor_1/elctro_damage_zone(1)")).map(|z| z.zone).expect("zone 14");
    // (id, t0, dt, variant, place, a, b)
    let mut crossings: Vec<(u32, u32, u32, String, String, Vec3, Vec3)> = Vec::new();
    for line in std::fs::read_to_string(tsv).unwrap().lines() {
        let f: Vec<&str> = line.split(char::from(9)).collect();
        let n = |i: usize| f[i].parse::<f32>().unwrap();
        let (a, b) = (Vec3::new(n(5), n(6), n(7)), Vec3::new(n(8), n(9), n(10)));
        let (Some(na), Some(nb)) = (grid.locate(a, 1.0), grid.locate(b, 1.0)) else { continue };
        if reach(na) != reach(nb) {
            let id = u32::from_str_radix(f[1], 16).unwrap() | 0x8000_0000;
            crossings.push((id, f[3].parse().unwrap(), f[4].parse().unwrap(), f[0].to_owned(), f[2].to_owned(), a, b));
        }
    }
    // The floors zone 14 marks: how near each crossing's straight line (5 s apart, so only a guide) came to them.
    let marked14: Vec<Vec3> = (0..grid.len() as NodeId)
        .filter(|&n| tactics.hazards.zones_at(n).any(|z| z.zone == zone14))
        .map(|n| grid.position(n))
        .collect();
    let miss = |a: Vec3, b: Vec3| {
        marked14
            .iter()
            .map(|p| {
                let (dx, dz) = (b.x - a.x, b.z - a.z);
                let len2 = (dx * dx + dz * dz).max(1e-6);
                let t = (((p.x - a.x) * dx + (p.z - a.z) * dz) / len2).clamp(0.0, 1.0);
                ((a.x + t * dx - p.x).powi(2) + (a.z + t * dz - p.z).powi(2)).sqrt()
            })
            .fold(f32::INFINITY, f32::min)
    };
    crossings.sort_by_key(|c| (c.0, c.1));
    let mut i = 0;
    while i < crossings.len() {
        let id = crossings[i].0;
        let mut anomalies = crate::v026g::bots::anomalies::Anomalies::new(&world.map, id);
        while i < crossings.len() && crossings[i].0 == id {
            let (_, t0, dt, variant, place, a, b) = &crossings[i];
            anomalies.advance(*t0);
            let on0 = anomalies.zone_on_now(zone14);
            let switched_from = anomalies.next_switch(zone14);
            anomalies.advance(t0 + dt);
            let on1 = anomalies.zone_on_now(zone14);
            println!(
                "CROSS	{variant}	{id:08x}	p{place}	t0 {t0}	dt {dt}	({:.1},{:.1},{:.1})	({:.1},{:.1},{:.1})	on at t0 {on0}	on at t1 {on1}	next switch from t0 {switched_from:?}	miss {:.1}",
                a.x, a.y, a.z, b.x, b.y, b.z, miss(*a, *b)
            );
            i += 1;
        }
    }
}

/// Where the players stood in the band north of the carriers' pocket (`SRP_POINTS_TSV`: variant, match, place, time, x, feet, z, speed, jumping) against the grid: the
/// floor nearest (within 2 m), how far it is, the zones that mark it, and whether the start reaches it with the zones shut / ignoring them. One `PT` line each.
#[test]
#[ignore = "a measurement; needs SRP_MAPDATA_DIR and SRP_POINTS_TSV"]
fn vostok_band_points() {
    let (Some(dir), Some(tsv)) = (std::env::var_os("SRP_MAPDATA_DIR"), std::env::var_os("SRP_POINTS_TSV")) else {
        eprintln!("skipping: SRP_MAPDATA_DIR or SRP_POINTS_TSV is not set");
        return;
    };
    let map = mapdata::MapData::read(&std::path::PathBuf::from(dir).join(mapdata::format::file_name("level_03/day_2"))).unwrap();
    let links = Tuning::default().off_mesh_links();
    let world = Arc::new(World::build_with_links(map, mapdata::CELL, mapdata::Body::default(), links));
    let grid = &world.grid;
    let mut tactics = Tactics::new(world.clone(), Tuning::default().hazard.margin_m.get());
    tactics.hazards.attach(crate::v026g::bots::anomalies::Anomalies::new(&world.map, 0x8000_0001));
    let start = grid.locate(Vec3::new(-29.8, 6.4, -4.0), 1.5).expect("start floor");
    let shut = |n: NodeId| tactics.allowed(n, 0, Some(1), false);
    let open = |n: NodeId| tactics.spawn_ok(n, Some(1), false);
    for line in std::fs::read_to_string(tsv).unwrap().lines() {
        let f: Vec<&str> = line.split(char::from(9)).collect();
        let n = |i: usize| f[i].parse::<f32>().unwrap();
        let p = Vec3::new(n(4), n(5), n(6));
        let found = [0.3f32, 0.6, 1.0, 2.0].iter().find_map(|&d| grid.locate(p, d).map(|node| (node, d)));
        match found {
            None => println!("PT	{}	{}	({:.1},{:.1},{:.1})	speed {} jump {}	NO FLOOR within 2 m", f[0], f[3], p.x, p.y, p.z, f[7], f[8]),
            Some((node, d)) => {
                let at = grid.position(node);
                let zones: Vec<usize> = tactics.hazards.zones_at(node).map(|z| z.zone).collect();
                println!(
                    "PT	{}	{}	({:.1},{:.1},{:.1})	speed {} jump {}	floor ({:.1},{:.1},{:.1}) within {d} m	zones {zones:?}	reached shut {}	open {}",
                    f[0], f[3], p.x, p.y, p.z, f[7], f[8], at.x, at.y, at.z,
                    grid.path_where(start, node, shut).is_some(), grid.path_where(start, node, open).is_some()
                );
            }
        }
    }
}

/// The seam between the floors the carriers' start reaches with the zones shut and the unmarked floors beside them (Vostok Radar Station, t = 0 s): a flood over the
/// grid's links from the start through the floors a zone leaves open, then every unreached, unmarked floor in the pocket's north band that lies within 1.3 m
/// (ground) of a reached one, with the links between the two (kind, either way) and whether each can be stood on. One `SEAM` line each.
#[test]
#[ignore = "a measurement; needs SRP_MAPDATA_DIR"]
fn vostok_pocket_seam() {
    let Some(dir) = std::env::var_os("SRP_MAPDATA_DIR").map(std::path::PathBuf::from) else {
        eprintln!("skipping: SRP_MAPDATA_DIR is not set");
        return;
    };
    let map = mapdata::MapData::read(&dir.join(mapdata::format::file_name("level_03/day_2"))).unwrap();
    let links = Tuning::default().off_mesh_links();
    let world = Arc::new(World::build_with_links(map, mapdata::CELL, mapdata::Body::default(), links));
    let grid = &world.grid;
    let mut tactics = Tactics::new(world.clone(), Tuning::default().hazard.margin_m.get());
    tactics.hazards.attach(crate::v026g::bots::anomalies::Anomalies::new(&world.map, 0x8000_0001));
    let start = grid.locate(Vec3::new(-29.8, 6.4, -4.0), 1.5).expect("start floor");
    let shut = |n: NodeId| tactics.allowed(n, 0, Some(1), false);
    let mut seen = vec![false; grid.len()];
    let mut queue = vec![start];
    seen[start as usize] = true;
    while let Some(u) = queue.pop() {
        for (v, _) in grid.links(u) {
            if !seen[v as usize] && shut(v) {
                seen[v as usize] = true;
                queue.push(v);
            }
        }
    }
    println!("SEAMINFO reached {} floors", seen.iter().filter(|&&b| b).count());
    // The box searched (`SRP_SEAM_BOX=x0,x1,z0,z1,max_gap_m`), by default the pocket's north band and 1.3 m.
    let b: Vec<f32> = std::env::var("SRP_SEAM_BOX").ok().map(|v| v.split(',').filter_map(|x| x.parse().ok()).collect()).filter(|v: &Vec<f32>| v.len() == 5).unwrap_or_else(|| vec![-42.0, -24.0, -16.0, -5.0, 1.3]);
    let in_band = |p: Vec3| (b[0]..=b[1]).contains(&p.x) && (b[2]..=b[3]).contains(&p.z) && (5.0..=6.9).contains(&p.y);
    let kind = |a: NodeId, b: NodeId| grid.links(a).find(|&(v, _)| v == b).map(|(_, k)| format!("{k:?}"));
    let mut pairs: Vec<(f32, NodeId, NodeId)> = Vec::new();
    for u in 0..grid.len() as NodeId {
        let pu = grid.position(u);
        if seen[u as usize] || !in_band(pu) || !shut(u) {
            continue;
        }
        for r in 0..grid.len() as NodeId {
            if !seen[r as usize] {
                continue;
            }
            let pr = grid.position(r);
            if pr.distance_xz(pu) <= b[4] && (pr.y - pu.y).abs() <= 1.5 && in_band(pr) {
                pairs.push((pr.distance_xz(pu), r, u));
            }
        }
    }
    pairs.sort_by(|a, b| a.0.total_cmp(&b.0));
    println!("SEAMINFO {} pairs", pairs.len());
    for (d, r, u) in pairs.iter().take(60) {
        let (pr, pu) = (grid.position(*r), grid.position(*u));
        println!(
            "SEAM	reached ({:.2},{:.2},{:.2}) unreached ({:.2},{:.2},{:.2})	dist {d:.2} dy {:.2}	link r->u {:?} u->r {:?}	can_stand r {} u {}",
            pr.x, pr.y, pr.z, pu.x, pu.y, pu.z, pu.y - pr.y, kind(*r, *u), kind(*u, *r), grid.can_stand(*r), grid.can_stand(*u)
        );
    }
}

/// The collision triangles of the seam in the pocket's north band (Vostok Radar Station): every walker triangle whose ground footprint meets x -36..-30, z -11.6..-8.8
/// and whose heights are within 4.5 .. 8.5, its corners, its steepness and its walker behaviour; then the same for the bullet mesh. One `TRI` line each.
#[test]
#[ignore = "a measurement; needs SRP_MAPDATA_DIR"]
fn vostok_seam_triangles() {
    let Some(dir) = std::env::var_os("SRP_MAPDATA_DIR").map(std::path::PathBuf::from) else {
        eprintln!("skipping: SRP_MAPDATA_DIR is not set");
        return;
    };
    let map = mapdata::MapData::read(&dir.join(mapdata::format::file_name("level_03/day_2"))).unwrap();
    for (kind, tris) in [("walker", map.walker_triangles()), ("bullet", map.bullet_triangles())] {
        for t in tris {
            let (xs, zs, ys) = (t.v.map(|v| v.x), t.v.map(|v| v.z), t.v.map(|v| v.y));
            let (min, max) = (|a: [f32; 3]| a.into_iter().fold(f32::INFINITY, f32::min), |a: [f32; 3]| a.into_iter().fold(f32::NEG_INFINITY, f32::max));
            let b: Vec<f32> = std::env::var("SRP_TRI_BOX").ok().map(|v| v.split(',').filter_map(|x| x.parse().ok()).collect()).filter(|v: &Vec<f32>| v.len() == 4).unwrap_or_else(|| vec![-36.0, -30.0, -11.6, -8.8]);
            if max(xs) < b[0] || min(xs) > b[1] || max(zs) < b[2] || min(zs) > b[3] || max(ys) < 4.5 || min(ys) > 8.5 {
                continue;
            }
            let (a, b, c) = (t.v[0], t.v[1], t.v[2]);
            let (u, v) = ((b.x - a.x, b.y - a.y, b.z - a.z), (c.x - a.x, c.y - a.y, c.z - a.z));
            let n = (u.1 * v.2 - u.2 * v.1, u.2 * v.0 - u.0 * v.2, u.0 * v.1 - u.1 * v.0);
            let len = (n.0 * n.0 + n.1 * n.1 + n.2 * n.2).sqrt().max(1e-9);
            println!(
                "TRI	{kind}	({:.2},{:.2},{:.2}) ({:.2},{:.2},{:.2}) ({:.2},{:.2},{:.2})	up {:.2}	{:?}",
                a.x, a.y, a.z, b.x, b.y, b.z, c.x, c.y, c.z, (n.1 / len).abs(), t.walker
            );
        }
    }
}

/// What Vostok Radar Station's electric zones hit with, from the map file's `damage_zone` config (raw, before armour; health is 1.0).
#[test]
#[ignore = "a measurement; needs SRP_MAPDATA_DIR"]
fn vostok_antenna_damage() {
    let Some(dir) = std::env::var_os("SRP_MAPDATA_DIR").map(std::path::PathBuf::from) else {
        eprintln!("skipping: SRP_MAPDATA_DIR is not set");
        return;
    };
    let map = mapdata::MapData::read(&dir.join(mapdata::format::file_name("level_03/day_2"))).unwrap();
    for (i, z) in map.zones.iter().enumerate() {
        if let Some(d) = &z.damage {
            println!(
                "ZONE	{i}	{}	type {}	max_hit {}	parts {} {:?}	interval_ms {}	continuous {}	piercing {}	rate/s {:.3}	curve {:?}",
                z.name, z.damage_type, d.max_hit, d.parts, d.part_names, d.interval_ms, d.continuous, d.armor_piercing, d.rate_per_s(), d.hit_curve
            );
        }
    }
}

