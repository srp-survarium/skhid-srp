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
