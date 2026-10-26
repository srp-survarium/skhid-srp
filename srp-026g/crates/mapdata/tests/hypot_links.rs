//! `Vec3::distance_xz` against `f32::hypot`, bit for bit, on the game's own maps: every Walk, Drop and Jump link
//! of every world in `SRP_MAPDATA_DIR`, and the pairs the search's heuristic computes (each floor against goals
//! anywhere). Where `hypotf` is glibc's (2.35 or later) none may differ; on another libm the count is only printed.
//!
//! ```text
//! SRP_MAPDATA_DIR=<mapdata-dump files> cargo test -p mapdata --release --test hypot_links -- --ignored --nocapture
//! ```
//!
//! With `SRP_HYPOT_DUMP=<file>` every 256th difference (as the two floats' bits, little-endian, x then z) is also
//! written, for `private/pvb-343-check/profile/hypot_dump_check.c`: the same inputs under another libm (the live
//! image's glibc 2.36), which a Rust test binary built here cannot be run under.

use std::io::Write;

use mapdata::{Body, MapData, Vec3, World};

/// The new and the old distance of `a` and `b`, as bits; every 256th difference goes to `dump`.
fn bits(a: Vec3, b: Vec3, dump: &mut Option<(std::io::BufWriter<std::fs::File>, u64)>) -> (u32, u32) {
    let (dx, dz) = (a.x - b.x, a.z - b.z);
    if let Some((file, count)) = dump {
        *count += 1;
        if *count % 256 == 0 {
            file.write_all(&dx.to_bits().to_le_bytes()).unwrap();
            file.write_all(&dz.to_bits().to_le_bytes()).unwrap();
        }
    }
    (a.distance_xz(b).to_bits(), dx.hypot(dz).to_bits())
}

#[test]
#[ignore = "needs SRP_MAPDATA_DIR"]
fn distance_xz_is_hypot_on_every_link_and_heuristic_pair_of_every_world() {
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
    let (mut checked, mut differ) = (0u64, 0u64);
    let mut dump = std::env::var_os("SRP_HYPOT_DUMP")
        .map(|path| (std::io::BufWriter::new(std::fs::File::create(path).unwrap()), 0u64));
    for path in &files {
        let world = World::build(MapData::read(path).unwrap(), mapdata::CELL, Body::default());
        let grid = &world.grid;
        let (mut links, mut pairs, mut bad) = (0u64, 0u64, 0u64);
        for node in 0..grid.len() as u32 {
            let from = grid.position(node);
            for (to, _kind) in grid.links(node) {
                let (new, old) = bits(grid.position(to), from, &mut dump);
                links += 1;
                bad += u64::from(new != old);
            }
        }
        // The heuristic: a floor against a goal. Goals are floors and points between them, as a bot's are.
        let n = grid.len() as u64;
        let mut s = 0x9E37_79B9_7F4A_7C15u64;
        let mut next = move || {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            s
        };
        for _ in 0..64 {
            let goal = grid.position((next() % n) as u32);
            let goal = Vec3::new(
                goal.x + (next() % 1000) as f32 * 0.001 - 0.5,
                goal.y,
                goal.z + (next() % 1000) as f32 * 0.001 - 0.5,
            );
            for node in 0..grid.len() as u32 {
                let (new, old) = bits(grid.position(node), goal, &mut dump);
                pairs += 1;
                bad += u64::from(new != old);
            }
        }
        eprintln!("{}: {} floors, {links} links, {pairs} heuristic pairs, {bad} differ", world.map.project, grid.len());
        checked += links + pairs;
        differ += bad;
    }
    if let Some((mut file, _)) = dump {
        file.flush().unwrap();
    }
    eprintln!("{} worlds, {checked} distances checked, {differ} differ", files.len());
    if cfg!(all(target_os = "linux", target_env = "gnu")) {
        assert_eq!(differ, 0);
    }
}
