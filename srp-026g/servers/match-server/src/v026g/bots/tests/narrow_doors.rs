//! [A313] `grid.narrow_doors` on the game's maps: the doorways the client's own body walks through both ways and the default grid
//! closes (the all-maps census of 2026-11-05: 438 openings of 0.6-1.1 m the client walks, 6 closed; the 4 here are doors the
//! erosion rounds closed). With the switch each has a walk way through it no longer than [`WAY_M`]; without it none. The census's
//! other two are not this pass's: level_04 (59.4, 22.1) is 0.75 m at the body's height, narrower than the body by the exact
//! pass's rule, and Cologne (level_08, 25.0, 21.4) a passage about 1.5 m wide the rounds took for another reason. They run only when
//! `SRP_MAPDATA_DIR` names the `mapdata-dump` files (and skip with a note otherwise):
//!
//! ```text
//! SRP_MAPDATA_DIR=<dir of .mapdata files> cargo test -p match-server --release narrow_doors
//! ```

use std::collections::{BinaryHeap, HashMap};

use mapdata::{LinkKind, MapData, NodeId, Vec3, WalkGrid};

use crate::v026g::bots::tuning::GridTuning;

/// A walk way through the door counts when this short, metres (walk links only), and reaches within this of the far floor.
const WAY_M: f32 = 4.0;
const NEAR_M: f32 = 0.3;
const LEVEL_M: f32 = 0.35;

/// The doors: the project, and a floor on each side the client's body walks between (straight, both ways).
const DOORS: [(&str, [f32; 3], [f32; 3]); 4] = [
    ("level_01/day_4", [43.68, 18.98, 44.10], [43.68, 18.91, 45.60]),
    ("level_02/day_4", [-28.55, 15.64, 92.47], [-28.30, 15.68, 93.97]),
    ("level_04/day_4", [110.12, 7.14, 84.88], [111.88, 7.14, 85.12]),
    // Vector Laboratory's strip: the doorway between two posts 0.90 m apart (fx-smoke's controller spots, fx-review's 42 passes).
    ("level_07/day_4", [10.01, 20.76, -7.29], [11.26, 20.79, -6.04]),
];

fn flat(a: Vec3, b: Vec3) -> f32 {
    ((a.x - b.x).powi(2) + (a.z - b.z).powi(2)).sqrt()
}

/// Whether the walk links join the floor nearest `a` to a floor within NEAR_M of `b` by a way no longer than WAY_M.
fn joined(grid: &WalkGrid, a: Vec3, b: Vec3) -> bool {
    let Some(start) = grid
        .floors_in([a.x - NEAR_M, a.z - NEAR_M, a.x + NEAR_M, a.z + NEAR_M])
        .filter(|&n| (grid.position(n).y - a.y).abs() <= LEVEL_M)
        .min_by(|&x, &y| flat(grid.position(x), a).total_cmp(&flat(grid.position(y), a)))
    else {
        return false;
    };
    let mut dist: HashMap<NodeId, u32> = HashMap::from([(start, 0)]);
    let mut heap = BinaryHeap::from([(std::cmp::Reverse(0u32), start)]);
    while let Some((std::cmp::Reverse(d), n)) = heap.pop() {
        if d > dist[&n] {
            continue;
        }
        let p = grid.position(n);
        if flat(p, b) <= NEAR_M && (p.y - b.y).abs() <= LEVEL_M {
            return true;
        }
        for (m, kind) in grid.links(n) {
            let nd = d + (flat(p, grid.position(m)) * 1000.0) as u32;
            if kind == LinkKind::Walk && nd as f32 <= WAY_M * 1000.0 && dist.get(&m).is_none_or(|&o| nd < o) {
                dist.insert(m, nd);
                heap.push((std::cmp::Reverse(nd), m));
            }
        }
    }
    false
}

#[test]
fn narrow_doors_open_the_doorways_the_client_walks_through() {
    let Some(dir) = std::env::var_os("SRP_MAPDATA_DIR") else {
        eprintln!("skipping: SRP_MAPDATA_DIR is not set");
        return;
    };
    let mut maps: HashMap<&str, Option<(WalkGrid, WalkGrid)>> = HashMap::new();
    let mut wrong = Vec::new();
    for (project, a, b) in DOORS {
        let grids = maps.entry(project).or_insert_with(|| {
            let path = std::path::Path::new(&dir).join(mapdata::format::file_name(project));
            let map = MapData::read(&path).ok()?;
            let walker = map.walker_triangles();
            let spawns: Vec<Vec3> = map.spawns.iter().map(|s| s.position).collect();
            let build = |narrow_doors: bool| {
                // narrow_doors alone: grid.wall_give_back (ON) gives some of these doorways' floors back too.
                let tuning = GridTuning { narrow_doors, wall_give_back: false, ..GridTuning::default() };
                WalkGrid::build_kept_with_links(
                    &walker,
                    map.play_area(mapdata::AREA_MARGIN),
                    mapdata::CELL,
                    tuning.body(),
                    &map.grid_seeds(),
                    &spawns,
                    tuning.off_mesh_links(),
                )
                .0
            };
            Some((build(false), build(true)))
        });
        let Some((off, on)) = grids else {
            eprintln!("skipping {project}: no map file");
            continue;
        };
        let (a, b) = (Vec3::new(a[0], a[1], a[2]), Vec3::new(b[0], b[1], b[2]));
        let (was, now) = (joined(off, a, b) || joined(off, b, a), joined(on, a, b) && joined(on, b, a));
        println!("{project} {a:?} <-> {b:?}: off {was}, on {now}");
        if was || !now {
            wrong.push(format!("{project} {a:?} <-> {b:?}: off {was} (want false), on {now} (want true)"));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}
