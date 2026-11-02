//! What the floor is where the bench's bots stood on a "flight" (A240b, `tallies/route/flights.rs`): the walk grid's floors round
//! the place and the walker faces under it (a stair is `walk` faces, `WalkerBehaviour::Walk`; ground is `by slope`). A
//! measurement, run with
//! `SRP_MAPDATA_DIR=<mapdata folder> SRP_FLIGHT_LOOK="level_01/day_2:62.2,25.2,-7.4;..." cargo test -p match-server --release
//! flight_look -- --ignored --nocapture`.

use super::*;

/// The floors within this of the place, across, are printed, metres.
const LOOK_M: f32 = 1.5;
/// … every this, metres.
const LOOK_STEP_M: f32 = 0.25;

#[test]
#[ignore = "a measurement; needs SRP_MAPDATA_DIR and SRP_FLIGHT_LOOK"]
fn flight_look() {
    let (Some(dir), Some(look)) = (std::env::var_os("SRP_MAPDATA_DIR").map(std::path::PathBuf::from), std::env::var("SRP_FLIGHT_LOOK").ok()) else {
        eprintln!("skipping: SRP_MAPDATA_DIR or SRP_FLIGHT_LOOK is not set");
        return;
    };
    let mut worlds: std::collections::HashMap<String, (Arc<World>, Vec<mapdata::Triangle>)> = std::collections::HashMap::new();
    for item in look.split(';').filter(|s| !s.is_empty()) {
        let (project, at) = item.split_once(':').unwrap();
        let p: Vec<f32> = at.split(',').map(|v| v.parse().unwrap()).collect();
        let at = Vec3::new(p[0], p[1], p[2]);
        let (world, walker) = worlds.entry(project.to_owned()).or_insert_with(|| {
            let map = mapdata::MapData::read(&dir.join(mapdata::format::file_name(project))).unwrap();
            let walker = map.walker_triangles();
            let links = Tuning::default().off_mesh_links();
            (Arc::new(World::build_with_links(map, mapdata::CELL, mapdata::Body::default(), links)), walker)
        });
        let grid = &world.grid;
        let Some(node) = grid.locate(at, 1.0) else {
            println!("{project} {at:?}: no floor");
            continue;
        };
        let y0 = grid.position(node).y;
        println!("== {project} ({:.1}, {:.1}, {:.1}) floor y {y0:.2}: heights round it, dm over its floor (. no floor within 1.5 m in height)", at.x, at.y, at.z);
        let n = (LOOK_M / LOOK_STEP_M) as i32;
        for k in -n..=n {
            let row: Vec<String> = (-n..=n)
                .map(|i| {
                    let q = Vec3::new(at.x + i as f32 * LOOK_STEP_M, y0, at.z + k as f32 * LOOK_STEP_M);
                    match grid.locate(q, 0.2).map(|f| grid.position(f)).filter(|f| (f.y - y0).abs() <= 1.5) {
                        Some(f) => format!("{:>4}", ((f.y - y0) * 10.0).round() as i32),
                        None => "   .".to_owned(),
                    }
                })
                .collect();
            println!("  z {:+.2} {}", k as f32 * LOOK_STEP_M, row.join(""));
        }
        // The walker faces a vertical line through the place meets within 1.5 m of its floor.
        let mut faces = Vec::new();
        for t in walker.iter() {
            let [a, b, c] = t.v;
            let cross = |p: Vec3, q: Vec3, r: Vec3| (q.x - p.x) * (r.z - p.z) - (q.z - p.z) * (r.x - p.x);
            let (d1, d2, d3) = (cross(a, b, at), cross(b, c, at), cross(c, a, at));
            if (d1 < 0.0 || d2 < 0.0 || d3 < 0.0) && (d1 > 0.0 || d2 > 0.0 || d3 > 0.0) {
                continue;
            }
            let normal = (b - a).cross(c - a);
            let len = normal.length();
            if len < 1e-6 || normal.y.abs() < 1e-6 {
                continue;
            }
            let y = a.y - (normal.x * (at.x - a.x) + normal.z * (at.z - a.z)) / normal.y;
            if (y - y0).abs() > 1.5 {
                continue;
            }
            let slope = (normal.y.abs() / len).acos().to_degrees();
            faces.push(format!("{:?} y {:.2} slope {:.0} deg", t.walker, y, slope));
        }
        println!("  faces: {}", if faces.is_empty() { "-".to_owned() } else { faces.join("; ") });
    }
}
