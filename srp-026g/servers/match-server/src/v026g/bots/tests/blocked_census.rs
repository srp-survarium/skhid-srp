//! [blocked walk-offs] The census of the built grids' walk-offs down which no client way goes
//! (`mapdata::WalkGrid::walk_offs_with_no_client_way`): what the Drop links' landing pass (`mapdata::Body::drop_lands`) now
//! takes away. A measurement on real files, the grids read with the server's grid body (`GridTuning::default().body()`) and
//! walked on the game's shapes, as the build walks them:
//!
//! ```text
//! ROOM_MAPDATA=<dir of <project>.mapdata> ROOM_BUILT=<dir of built grids> ROOM_NAMES=<tsv: project, grid file>
//!   cargo test -p match-server --release --lib blocked_census -- --ignored --nocapture
//! ```
//!
//! Prints a line per project and one per link (`BLOCKEDLINK`: project, kind, whether a walk line could not move (the old rule
//! kept those), take-off, landing).

use crate::v026g::bots::tuning::GridTuning;
use mapdata::controller::Collision;
use mapdata::{LinkKind, WalkGrid};

#[test]
#[ignore = "a census of real built grids; needs ROOM_MAPDATA, ROOM_BUILT, ROOM_NAMES"]
fn blocked_census() {
    let var = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("set {k}"));
    let (maps, built) = (std::path::PathBuf::from(var("ROOM_MAPDATA")), std::path::PathBuf::from(var("ROOM_BUILT")));
    let names = std::fs::read_to_string(var("ROOM_NAMES")).unwrap();
    let tuning = GridTuning::default();
    let (mut drops, mut walk_offs) = (0usize, 0usize);
    for line in names.lines().filter(|l| !l.trim().is_empty()) {
        let cols: Vec<&str> = line.split('\t').collect();
        let (project, grid_file) = (cols[0], cols[1]);
        let key = u64::from_str_radix(grid_file.trim_start_matches("grid-").trim_end_matches(".bin"), 16).unwrap();
        let file = maps.join(format!("{}.mapdata", project.replace('/', "_")));
        let map = mapdata::MapData::from_bytes(&std::fs::read(&file).unwrap()).unwrap();
        let grid = WalkGrid::read(&built.join(grid_file), key, tuning.body()).unwrap();
        let (caster, _) = grid.body().controller();
        let world = Collision::build_exe(&map).unwrap_or_else(|| Collision::build(&map.walker_triangles()).with_caster(caster));
        let links = grid.walk_offs_with_no_client_way(&world);
        let d = links.iter().filter(|l| l.2 == LinkKind::Drop).count();
        println!("BLOCKED {project}: {} walk-offs with no client way: {d} Drop links, {} walk-off GapDrops", links.len(), links.len() - d);
        for (from, to, kind, held) in &links {
            println!(
                "BLOCKEDLINK\t{project}\t{kind:?}\t{held}\t{:.2}\t{:.2}\t{:.2}\t{:.2}\t{:.2}\t{:.2}",
                from.x, from.y, from.z, to.x, to.y, to.z
            );
        }
        drops += d;
        walk_offs += links.len() - d;
    }
    println!("BLOCKED all: {drops} Drop links, {walk_offs} walk-off GapDrops");
}
