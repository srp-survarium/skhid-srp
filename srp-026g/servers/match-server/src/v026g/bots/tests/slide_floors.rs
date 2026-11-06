//! [A316/A318] `grid.slide_floors` on the game's maps: what the pass reads and cuts on each map file, and the floors kept with and
//! without it. A probe (prints, asserts nothing); runs only when `SRP_MAPDATA_DIR` names the `mapdata-dump` files:
//!
//! ```text
//! SRP_MAPDATA_DIR=<dir of .mapdata files> cargo test -p match-server --release slide_floors_on_the_maps -- --ignored --nocapture
//! ```

use mapdata::{MapData, Vec3, WalkGrid};

use crate::v026g::bots::tuning::GridTuning;

#[test]
#[ignore = "a probe of the map files: prints the slide pass's numbers per map"]
fn slide_floors_on_the_maps() {
    let Some(dir) = std::env::var_os("SRP_MAPDATA_DIR") else {
        eprintln!("skipping: SRP_MAPDATA_DIR is not set");
        return;
    };
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .expect("the map dir")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "mapdata"))
        .collect();
    files.sort();
    for path in files {
        let Ok(map) = MapData::read(&path) else {
            continue;
        };
        let walker = map.walker_triangles();
        let spawns: Vec<Vec3> = map.spawns.iter().map(|s| s.position).collect();
        let build = |slide_floors: bool| {
            let tuning = GridTuning { slide_floors, ..GridTuning::default() };
            WalkGrid::build_kept_with_links(
                &walker,
                map.play_area(mapdata::AREA_MARGIN),
                mapdata::CELL,
                tuning.body(),
                &map.grid_seeds(),
                &spawns,
                tuning.off_mesh_links(),
            )
        };
        let (off, _) = build(false);
        let (on, stats) = build(true);
        println!(
            "{}: floors kept off {} on {} ({:+}); slide pass {:?}",
            path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
            off.len(),
            on.len(),
            on.len() as i64 - off.len() as i64,
            stats.slide
        );
    }
}
