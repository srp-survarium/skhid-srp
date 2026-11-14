//! [bench speed] The digests of every map's grid as the server builds it: the proof that a change to the grid's build (a faster
//! schedule, a shortcut) gives bit-identical grids, run on the code before and after it.

use std::path::PathBuf;
use std::time::Instant;

use mapdata::{MapData, WalkGrid};

use crate::v026g::bots::tuning::Tuning;

/// Every map file in `SRP_MAPDATA_DIR` (or the projects of `SRP_GRID_PROJECTS`, comma-separated): its grid as the server's map
/// store builds it with the default tuning (`bots::maps`: the grid body, the off-mesh links, the rim jumps flown on the game's
/// shapes), its floors, digest and build time printed and written to `SRP_DIGESTS_OUT` (`project,floors,grid,seconds`).
#[test]
#[ignore = "real maps; minutes a map"]
fn map_grid_digests() {
    let dir = PathBuf::from(std::env::var("SRP_MAPDATA_DIR").expect("SRP_MAPDATA_DIR"));
    let projects: Vec<String> = match std::env::var("SRP_GRID_PROJECTS") {
        Ok(list) => list
            .split(',')
            .filter(|p| !p.is_empty())
            .map(str::to_owned)
            .collect(),
        Err(_) => {
            let mut files: Vec<String> = std::fs::read_dir(&dir)
                .expect("SRP_MAPDATA_DIR")
                .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
                .filter_map(|name| name.strip_suffix(".mapdata").map(str::to_owned))
                .collect();
            files.sort();
            files
                .into_iter()
                .map(|stem| {
                    MapData::read(&dir.join(format!("{stem}.mapdata")))
                        .expect("map")
                        .project
                })
                .collect()
        }
    };
    let tuning = Tuning::default();
    let (body, links) = (tuning.grid_body(), tuning.off_mesh_links());
    let mut lines = vec!["project,floors,grid,seconds".to_owned()];
    for project in &projects {
        let map = MapData::read(&dir.join(mapdata::format::file_name(project))).expect("map");
        let started = Instant::now();
        let walker = map.walker_triangles();
        let spawns: Vec<mapdata::Vec3> = map.spawns.iter().map(|s| s.position).collect();
        let shapes = body
            .rim_jumps
            .then(|| mapdata::controller::Collision::build_exe(&map))
            .flatten();
        let area = map.play_area(mapdata::AREA_MARGIN);
        let grid = WalkGrid::build_kept_with_shapes(
            &walker,
            shapes.as_ref(),
            area,
            mapdata::CELL,
            body,
            &map.grid_seeds(),
            &spawns,
            links,
        )
        .0;
        let seconds = started.elapsed().as_secs_f32();
        println!(
            "grid {project}: {} floors, grid {:016x}, built in {seconds:.1} s",
            grid.len(),
            grid.digest()
        );
        lines.push(format!(
            "{project},{},{:016x},{seconds:.1}",
            grid.len(),
            grid.digest()
        ));
    }
    if let Some(out) = std::env::var_os("SRP_DIGESTS_OUT") {
        std::fs::write(out, lines.join("\n") + "\n").unwrap();
    }
}
