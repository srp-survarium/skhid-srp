//! The columns, rows and lip bits a grid makes when it is built or read (`WalkGrid::hot_mismatches`) against the
//! functions they stand for, to the bit, for every floor of every world in `SRP_MAPDATA_DIR`; and what they cost in memory.
//!
//! ```text
//! SRP_MAPDATA_DIR=<mapdata-dump files> cargo test -p mapdata --release --test hot_floors -- --ignored --nocapture
//! ```

use mapdata::{Body, MapData, World};

#[test]
#[ignore = "needs SRP_MAPDATA_DIR"]
fn every_floor_of_every_world_has_the_places_and_lips_of_the_functions() {
    let Some(dir) = std::env::var_os("SRP_MAPDATA_DIR").map(std::path::PathBuf::from) else {
        eprintln!("skipping: SRP_MAPDATA_DIR is not set");
        return;
    };
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "mapdata"))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no map files in {}", dir.display());
    let (mut floors, mut bad, mut bytes, mut largest) = (0usize, 0usize, 0usize, (0usize, String::new()));
    for path in &files {
        let started = std::time::Instant::now();
        let world = World::build(MapData::read(path).unwrap(), mapdata::CELL, Body::default());
        let seconds = started.elapsed().as_secs_f32();
        let grid = &world.grid;
        // The same work as making them (every link's cost, by the function), plus the comparison: about what a load adds.
        let checked = std::time::Instant::now();
        let mismatches = grid.hot_mismatches();
        let making = checked.elapsed().as_secs_f32();
        eprintln!(
            "{}: {} floors, {:.1} MB, built in {seconds:.2} s, {mismatches} differ (making them, link costs included, ~{making:.2} s)",
            world.map.project,
            grid.len(),
            grid.memory_bytes() as f64 / 1e6
        );
        floors += grid.len();
        bad += mismatches;
        bytes += grid.memory_bytes();
        if grid.memory_bytes() > largest.0 {
            largest = (grid.memory_bytes(), world.map.project.clone());
        }
    }
    eprintln!(
        "{} worlds, {floors} floors, {bad} differ; grids {:.0} MB together, the largest {} {:.1} MB",
        files.len(),
        bytes as f64 / 1e6,
        largest.1,
        largest.0 as f64 / 1e6
    );
    assert_eq!(bad, 0);
}
