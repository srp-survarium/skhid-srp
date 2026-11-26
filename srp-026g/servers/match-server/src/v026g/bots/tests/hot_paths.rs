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

/// Every map of the bench: levels 1-8, batteries (day 2) and Team Deathmatch (day 4).
fn all_maps() -> Vec<String> {
    (1..=8).flat_map(|level| [2, 4].map(|day| format!("level_{level:02}/day_{day}"))).collect()
}

/// [CPU hot paths, the drift guard] On every map: the index's size, and every floor's lookups and every link's price by
/// the runtime copies (`mapdata::grid::hot`) against the originals, for each reach the bots search with; a difference fails.
#[test]
#[ignore = "a check and a measurement on all 16 real maps; needs SRP_MAPDATA_DIR"]
fn hot_paths_index_on_all_maps() {
    let Some(dir) = super::real_maps::dir() else {
        eprintln!("skipping: SRP_MAPDATA_DIR is not set");
        return;
    };
    let mut largest = 0;
    for project in all_maps() {
        let world = world_of(&dir, &project);
        let grid = &world.grid;
        let started = std::time::Instant::now();
        let hot = HotIndex::for_grid(grid);
        let build_ms = started.elapsed().as_secs_f64() * 1e3;
        largest = largest.max(hot.hot_bytes());
        let (floors, links) = hot.hot_check_grid(grid).unwrap_or_else(|why| panic!("{project}: {why}"));
        let mut priced = 0u64;
        for u in 0..grid.len() as NodeId {
            for (v, kind) in grid.links(u) {
                for reach in reaches() {
                    let (a, b) = (grid.link_cost_reaching(u, v, kind, reach), grid.link_cost_hot(&hot, u, v, kind, reach));
                    assert_eq!(a.to_bits(), b.to_bits(), "{project}: {u} -> {v} {kind:?} {reach:?}");
                    priced += 1;
                }
            }
        }
        let _ = mapdata::take_falls_closed();
        eprintln!(
            "HOT index {project}: floors {floors}, {:.2} MB ({} bytes), built in {build_ms:.1} ms; lookups equal on {floors} floors \
             and {links} links; {priced} prices equal ({} reaches)",
            hot.hot_bytes() as f64 / 1e6,
            hot.hot_bytes(),
            reaches().len()
        );
    }
    eprintln!("HOT index: the largest {:.2} MB", largest as f64 / 1e6);
    assert!(largest < 5_000_000, "the index is over 5 MB a map: {largest} bytes");
}

/// [CPU hot paths, the copy guard] The runtime copies in `mapdata::grid::hot` (and `RouteSearch::step_hot`) each follow an
/// original inside the grid key, which they cannot call (`copy-of:` in their docs). Each original's tokens (no comments,
/// no docs: `mapdata-srchash`'s units) are held to the digest they had when their copy was last checked against them: an
/// original changed without its copy fails here. Then make the change in the copy too, run `hot_paths_index_on_all_maps`
/// and `hot_paths_route_search_on_real_maps`, and put the new digest printed below in [`HOT_COPIES`].
const HOT_COPIES: &[(&str, &str, &str)] = &[
    ("WalkGrid::link_cost_hot", "WalkGrid::link_cost_reaching", "ca5771ab36318403b4663a22489b4862"),
    ("WalkGrid::links_back_hot", "WalkGrid::links_back", "bcef72e4b97322224a89b629d0d79bc3"),
    ("HotIndex::hot_off_link", "WalkGrid::off_link", "10687070e348a2917b1c0dd212af720a"),
    ("SlicedSearch::step_hot", "SlicedSearch::step", "0baa0f061f25ae8d4f1d44405f2d942f"),
    ("RouteSearch::step_hot", "RouteSearch::step", "438b3865ba552fad4fd3062c32a2070e"),
    ("HotIndex::hot_links", "WalkGrid::links", "593c33755eb61fba569fe7b8796c28b3"),
];

#[test]
fn hot_copies_follow_their_originals() {
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = mapdata_srchash::mapdata_source(&workspace).expect("mapdata's source parses");
    let mut changed = Vec::new();
    for &(copy, original, want) in HOT_COPIES {
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
        "originals changed since their runtime copies were checked against them: change each copy too, run the real-map \
         checks (hot_paths_index_on_all_maps, hot_paths_route_search_on_real_maps), then put these rows in HOT_COPIES:\n  {}",
        changed.join("\n  ")
    );
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
        let hot = HotIndex::for_grid(grid);
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
                let mut queue = mapdata::grid::HotOpen::hot_open_empty();
                let found = loop {
                    let mut iters = 200;
                    let step = if hot_step {
                        search.step_hot(&regions, grid, &hot, &mut queue, |_| true, |_| true, |_| 1.0, &mut iters)
                    } else {
                        search.step(&regions, grid, |_| true, |_| true, |_| 1.0, &mut iters)
                    };
                    slices += 1;
                    // The radix open list's invariants after every slice of a real search (the side keys below the buckets').
                    queue.hot_open_check().unwrap_or_else(|why| panic!("the open list after slice {slices}: {why}"));
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

/// [CPU hot paths, the key guard] The runtime-only items (every item of `crates/mapdata/src/grid/hot.rs` (the index, the copies, the open list) and `RouteSearch::step_hot`) are reached by none of the grid's, the sight tree's or
/// the memo sites' roots (`mapdata-srchash`'s closures, the ones `SRC_GRID`, `SRC_SIGHT` and the memo digests are taken
/// over), so no edit of theirs names the built grids anew. A new item named like a reached one, or a trait impl whose method
/// name the builder calls (`impl Default`: `default`), is reached and fails here; give it a name of its own.
#[test]
fn hot_items_are_outside_the_grid_and_sight_keys() {
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = mapdata_srchash::mapdata_source(&workspace).expect("mapdata's source parses");
    let file: Option<&str> = Some("crates/mapdata/src/grid/hot.rs");
    let names: &[&str] = &["RouteSearch::step_hot", "RouteSearch::step_bounded"];
    let ours = |u: &mapdata_srchash::Unit| file.is_some_and(|f| u.file == f) || u.names.iter().any(|n| names.contains(&n.as_str()));
    assert!(source.units.iter().any(ours), "no runtime-only items found: the test looks in the wrong place");
    let mut sets: Vec<(&str, Vec<&str>)> =
        vec![("grid", mapdata_srchash::GRID_ROOTS.to_vec()), ("sight", mapdata_srchash::SIGHT_ROOTS.to_vec())];
    for &(site, roots) in mapdata_srchash::MEMO_SITES {
        sets.push((site, roots.to_vec()));
    }
    let mut reached = Vec::new();
    for (key, roots) in sets {
        for i in source.closure(&roots).unwrap_or_else(|why| panic!("{key}: {why}")) {
            let unit = &source.units[i];
            if ours(unit) {
                let chain = source.why(&roots, &unit.names[0]).ok().flatten().unwrap_or_default().join(" ");
                reached.push(format!("{key}: {} ({chain})", unit.names.join(" / ")));
            }
        }
    }
    assert!(reached.is_empty(), "runtime-only items inside a key (rename them):\n  {}", reached.join("\n  "));
}

/// The spot plays' walk-up search (`Tactics::walk_up_from`) by the base's lookups against the plain one on the real maps:
/// the same sets (and so the same cap cut), and what each costs.
#[test]
#[ignore = "a measurement and check on real maps; needs SRP_MAPDATA_DIR"]
fn hot_paths_walk_up_on_real_maps() {
    let Some(dir) = super::real_maps::dir() else {
        eprintln!("skipping: SRP_MAPDATA_DIR is not set");
        return;
    };
    for project in maps() {
        let world = world_of(&dir, &project);
        let t = Tactics::new(world.clone(), Tuning::default().hazard.margin_m.get());
        let grid = &world.grid;
        let n = grid.len() as u64;
        let mut rng = Rng(0x51_7cc1_b727_220a ^ n);
        let points: Vec<(Vec3, f32)> = (0..200)
            .map(|i| (grid.position((rng.next() % n) as NodeId), [20.0, 32.0, 45.0, 65.0][i % 4]))
            .collect();
        let run = |hot: bool| {
            let started = std::time::Instant::now();
            let sets: Vec<_> = points.iter().map(|&(p, r)| t.walk_up_both(p, r, hot)).collect();
            (sets, started.elapsed().as_secs_f64())
        };
        let (plain, _) = run(false);
        let (fast, _) = run(true);
        assert!(plain == fast, "{project}: the walk-up sets");
        let mut times = [0.0f64; 2];
        for _ in 0..2 {
            times[0] += run(false).1;
            times[1] += run(true).1;
        }
        let floors: usize = plain.iter().map(|(_, c)| c).sum();
        eprintln!(
            "HOT walk-up {project}: {} sets, {floors} floors, plain {:.1} ms, hot {:.1} ms ({:.1} % less), equal sets",
            points.len(),
            times[0] * 1e3 / 2.0,
            times[1] * 1e3 / 2.0,
            100.0 * (1.0 - times[1] / times[0])
        );
    }
}
