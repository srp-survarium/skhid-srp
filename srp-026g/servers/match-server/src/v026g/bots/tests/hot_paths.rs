//! [CPU hot paths] The route search's load-time lookups (`mapdata::grid::HotIndex`) on the bench's real maps: the hot
//! copy against the plain search on every link and on searches from random floors, and what each costs. A measurement and
//! a check, decision-neutral.
//!
//! ```text
//! SRP_MAPDATA_DIR=<dir> SRP_GAME_DATA=<server-game-data.db> [SRP_BENCH_GRID_CACHE=<dir>] [SRP_HOT_MAPS=level_06/day_4,..] \
//!   cargo test -p match-server --release hot_paths -- --ignored --nocapture --test-threads=1
//! ```

use super::*;

use mapdata::grid::HotIndex;
use mapdata::regions::Regions;

/// The proof cells' maps (batt m2, tdm m6, batt m6), unless `SRP_HOT_MAPS` names others.
fn maps() -> Vec<String> {
    match std::env::var("SRP_HOT_MAPS") {
        Ok(list) if !list.is_empty() => list.split(',').map(str::to_owned).collect(),
        _ => ["level_02/day_2", "level_06/day_4", "level_06/day_2"].map(str::to_owned).to_vec(),
    }
}

fn world_of(dir: &std::path::Path, project: &str) -> Arc<World> {
    let t = Tuning::default();
    let bytes = std::fs::read(dir.join(mapdata::format::file_name(project))).unwrap();
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
    Arc::new(super::grid_cache::world(map, map_file, mapdata::CELL, t.grid_body(), t.off_mesh_links(), extras))
}

/// A xorshift: the same pairs every run.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

/// The reaches the bots search with: the grid's own body and a carrier's, and each with falls that kill.
fn reaches() -> Vec<mapdata::Reach> {
    let jumps = crate::v026g::bots::jump_arc::jump_ends();
    let mut all = vec![mapdata::Reach::FULL, mapdata::Reach::no_sprint(Some(mapdata::grid::STANDING_JUMP_M))];
    for r in all.clone() {
        for pain in [0.1, 0.6] {
            let body = mapdata::FallBody { pain, fall_scale: 1.0, jumps, second: mapdata::SecondLanding::Adds };
            all.push(mapdata::Reach { fall: mapdata::FallRoom::Survives(body), ..r });
        }
    }
    all
}

#[test]
#[ignore = "a measurement and check on real maps; needs SRP_MAPDATA_DIR"]
fn hot_paths_route_search_on_real_maps() {
    let Some(dir) = super::real_maps::dir() else {
        eprintln!("skipping: SRP_MAPDATA_DIR is not set");
        return;
    };
    for project in maps() {
        let world = world_of(&dir, &project);
        let grid = &world.grid;
        let regions = Regions::build(grid, None);
        let started = std::time::Instant::now();
        let hot = HotIndex::for_grid(grid);
        let build_ms = started.elapsed().as_secs_f64() * 1e3;
        eprintln!(
            "HOT {project}: floors {}, index {:.2} MB ({} bytes), built in {build_ms:.1} ms",
            grid.len(),
            hot.hot_bytes() as f64 / 1e6,
            hot.hot_bytes()
        );
        // Every link, priced by both, for every reach: equal to the bit.
        let mut links = 0u64;
        for u in 0..grid.len() as NodeId {
            for (v, kind) in grid.links(u) {
                for reach in reaches() {
                    let (a, b) = (grid.link_cost_reaching(u, v, kind, reach), grid.link_cost_hot(&hot, u, v, kind, reach));
                    assert_eq!(a.to_bits(), b.to_bits(), "{project}: {u} -> {v} {kind:?} {reach:?}");
                }
                links += 1;
            }
        }
        let _ = mapdata::take_falls_closed();
        eprintln!("HOT {project}: {links} links priced equal by both, for {} reaches", reaches().len());
        // Searches from random floors to random floors within 60 m, by the regions' route search as the bots run it,
        // in slices of 200 floors: the same answers, stages and floors, then the time of each.
        let n = grid.len() as u64;
        let mut rng = Rng(0x9e37_79b9_7f4a_7c15 ^ n);
        let mut pairs = Vec::new();
        while pairs.len() < 600 {
            let (a, b) = ((rng.next() % n) as NodeId, (rng.next() % n) as NodeId);
            if grid.position(a).distance_xz(grid.position(b)) < 60.0 && grid.may_reach(a, b) {
                pairs.push((a, b, reaches()[(rng.next() % 2) as usize]));
            }
        }
        let run = |hot_step: bool| {
            let mut out = Vec::new();
            let started = std::time::Instant::now();
            for &(a, b, reach) in &pairs {
                let Ok(mut search) = regions.start_route_search_reaching(grid, a, b, |_| true, |_| true, 200_000, reach) else {
                    out.push(None);
                    continue;
                };
                let mut slices = 0u32;
                let found = loop {
                    let mut iters = 200;
                    let step = if hot_step {
                        search.step_hot(&regions, grid, &hot, |_| true, |_| true, |_| 1.0, &mut iters)
                    } else {
                        search.step(&regions, grid, |_| true, |_| true, |_| 1.0, &mut iters)
                    };
                    slices += 1;
                    if let Some(found) = step {
                        break found;
                    }
                };
                out.push(Some((found, search.expanded(), slices)));
            }
            (out, started.elapsed().as_secs_f64())
        };
        // Warm both, then time each twice, alternating.
        let (plain, _) = run(false);
        let (fast, _) = run(true);
        assert_eq!(plain, fast, "{project}: the searches' answers");
        let mut times = [0.0f64; 2];
        for _ in 0..2 {
            times[0] += run(false).1;
            times[1] += run(true).1;
        }
        let expanded: usize = plain.iter().flatten().map(|(_, e, _)| e).sum();
        eprintln!(
            "HOT {project}: {} searches, {expanded} floors expanded, plain {:.1} ms, hot {:.1} ms ({:.1} % less), equal answers",
            pairs.len(),
            times[0] * 1e3 / 2.0,
            times[1] * 1e3 / 2.0,
            100.0 * (1.0 - times[1] / times[0])
        );
    }
}
