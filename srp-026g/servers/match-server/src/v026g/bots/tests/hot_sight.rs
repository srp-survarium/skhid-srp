//! [CPU hot paths] Sight's visibility test (`RayTree::visible`, by `hit_any_hot`) against the walk it replaced
//! (`RayTree::visible_by_walk`) on the bench's real maps: the same answer for every line, and what each costs. A
//! measurement and a check, decision-neutral.
//!
//! ```text
//! SRP_MAPDATA_DIR=<dir> SRP_GAME_DATA=<server-game-data.db> [SRP_BENCH_GRID_CACHE=<dir>] [SRP_HOT_MAPS=level_06/day_4,..] \
//!   cargo test -p match-server --release hot_sight -- --ignored --nocapture --test-threads=1
//! ```

use super::*;

/// The proof cells' maps (batt m2, tdm m6, batt m6), unless `SRP_HOT_MAPS` names others.
fn maps() -> Vec<String> {
    match std::env::var("SRP_HOT_MAPS") {
        Ok(list) if !list.is_empty() => list.split(',').map(str::to_owned).collect(),
        _ => ["level_02/day_2", "level_06/day_4", "level_06/day_2"].map(str::to_owned).to_vec(),
    }
}

#[test]
#[ignore = "a measurement and check on real maps; needs SRP_MAPDATA_DIR"]
fn hot_sight_visible_on_real_maps() {
    let Some(dir) = super::real_maps::dir() else {
        eprintln!("skipping: SRP_MAPDATA_DIR is not set");
        return;
    };
    for project in maps() {
        let t = Tuning::default();
        let bytes = std::fs::read(dir.join(mapdata::format::file_name(&project))).unwrap();
        let map_file = {
            let mut digest = mapdata::digest::Digest::new();
            digest.bytes(&bytes);
            digest.finish()
        };
        let map = mapdata::MapData::from_bytes(&bytes).unwrap();
        let extras = mapdata::GridExtras {
            wall_clearance: t.needs_wall_clearance(),
            open_sides: t.needs_open_sides(),
            pull_hold: t.grid.wall_clear_pull.then_some(crate::v026g::bots::tuning::PULL_HOLD_M),
            pull_walk: t.grid.pull_walk,
            walker_shapes: t.grid.walker_shapes,
        };
        let world = super::grid_cache::world(map, map_file, mapdata::CELL, t.grid_body(), t.off_mesh_links(), extras);
        let (grid, sight) = (&world.grid, &world.sight);
        // Eye-height lines between random floors 2-80 m apart, as the bots' sight tests are.
        let n = grid.len() as u64;
        let mut rng = 0x9e37_79b9_7f4a_7c15u64 ^ n;
        let mut next = || {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            rng
        };
        let mut lines = Vec::new();
        while lines.len() < 100_000 {
            let (a, b) = (grid.position((next() % n) as NodeId), grid.position((next() % n) as NodeId));
            let d = a.distance_xz(b);
            if (2.0..80.0).contains(&d) {
                lines.push((a + Vec3::new(0.0, 1.6, 0.0), b + Vec3::new(0.0, 1.2, 0.0)));
            }
        }
        let run = |hot: bool| {
            let started = std::time::Instant::now();
            let seen: Vec<bool> =
                lines.iter().map(|&(a, b)| if hot { sight.visible(a, b) } else { sight.visible_by_walk(a, b) }).collect();
            (seen, started.elapsed().as_secs_f64())
        };
        let (plain, _) = run(false);
        let (fast, _) = run(true);
        assert!(plain == fast, "{project}: the answers");
        let mut times = [0.0f64; 2];
        for _ in 0..3 {
            times[0] += run(false).1;
            times[1] += run(true).1;
        }
        eprintln!(
            "HOT sight {project}: {} lines, {} seen, plain {:.1} ms, hot {:.1} ms ({:.1} % less), equal answers",
            lines.len(),
            plain.iter().filter(|&&s| s).count(),
            times[0] * 1e3 / 3.0,
            times[1] * 1e3 / 3.0,
            100.0 * (1.0 - times[1] / times[0])
        );
    }
}

/// [CPU hot paths, the copy guard] `RayTree::hit_any_hot` follows `RayTree::walk`, which is inside the grid and sight keys and
/// which it cannot call (`copy-of:` in its doc). The original's tokens (no comments, no docs: `mapdata-srchash`'s units) are
/// held to the digest they had when the copy was last checked against them: the original changed without its copy fails
/// here. Then make the change in the copy too, run `hot_sight_visible_on_real_maps`, and put the new digest printed below in
/// [`SIGHT_COPIES`].
const SIGHT_COPIES: &[(&str, &str, &str)] = &[("RayTree::hit_any_hot", "RayTree::walk", "6de85458f3aec073be03e558f6eac2e2")];

#[test]
fn hot_sight_copies_follow_their_originals() {
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = mapdata_srchash::mapdata_source(&workspace).expect("mapdata's source parses");
    let mut changed = Vec::new();
    for &(copy, original, want) in SIGHT_COPIES {
        let units: Vec<_> = source.units.iter().filter(|u| u.names.iter().any(|n| n == original)).collect();
        assert_eq!(units.len(), 1, "{original}: {} items of that name in mapdata", units.len());
        let copies = source.units.iter().filter(|u| u.names.iter().any(|n| n == copy)).count();
        assert_eq!(copies, 1, "{copy}: {copies} items of that name in mapdata");
        let mut hash = mapdata_srchash::Hash128::default();
        hash.str(&units[0].text);
        let got = format!("{:032x}", hash.finish());
        if got != want {
            changed.push(format!("(\"{copy}\", \"{original}\", \"{got}\"),"));
        }
    }
    assert!(
        changed.is_empty(),
        "originals changed since their runtime copies were checked against them: change each copy too, run          hot_sight_visible_on_real_maps, then put these rows in SIGHT_COPIES:
  {}",
        changed.join("
  ")
    );
}
