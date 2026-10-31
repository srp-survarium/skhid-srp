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
