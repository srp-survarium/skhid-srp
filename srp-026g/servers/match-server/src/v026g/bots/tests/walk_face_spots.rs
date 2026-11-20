//! [walk-face chain] Vector Laboratory day_2's box top beside the walker pillar (#2255): the grid made a gap press up the
//! pillar's side, (8.01, 11.59, -17.04) -> (8.01, 32.16, -16.04), 20.6 m, which no client jump makes; a bot waited at it. With
//! the walk-face chain the link is gone: no link from the box top rises over the sprint jump's reach, and, no client way leading
//! up the pillar, the planner plans no way to its top, where no human of the recordings ever stood (no crossing of theirs on
//! Vector Laboratory day_2 ends over 30 m: routes, crossings_v2). On the real mapdata, the server's game data.
//!
//! ```text
//! SRP_MAPDATA_DIR=<dir> SRP_GAME_DATA=<server-game-data.db> [SRP_BENCH_GRID_CACHE=<dir>] cargo test -p match-server --release walk_face_spots -- --ignored --nocapture
//! ```

use super::*;

use crate::v026g::bots::path;

const PROJECT: &str = "level_07/day_2";
const BOX_TOP: Vec3 = Vec3::new(8.01, 11.59, -17.04);
const PILLAR_TOP: Vec3 = Vec3::new(8.01, 32.16, -16.04);
/// The most a link may rise: over it no client jump goes (the sprint climb's 1.15 m with a jump's reach, well under).
const RISE_MAX_M: f32 = 3.0;

/// The box top beside the pillar: no link of the grid from the box top's floors rises over [`RISE_MAX_M`], and no way is
/// planned from it to the pillar's top.
#[test]
#[ignore = "a per-spot test on the real mapdata; needs SRP_MAPDATA_DIR and SRP_GAME_DATA"]
fn vector_laboratorys_box_top_has_no_link_up_the_pillars_side_and_no_way_to_its_top() {
    let Some(dir) = super::real_maps::dir() else {
        eprintln!("skipping: SRP_MAPDATA_DIR is not set");
        return;
    };
    let t = Tuning::default();
    let bytes = std::fs::read(dir.join(mapdata::format::file_name(PROJECT))).unwrap();
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
    let grid = &world.grid;
    let from = grid.locate(BOX_TOP, 0.5).expect("the box top's floor");
    let to = grid.locate(PILLAR_TOP, 0.5).expect("the pillar top's floor");
    let near_box: Vec<NodeId> = grid
        .floors_in([BOX_TOP.x - 1.5, BOX_TOP.z - 1.5, BOX_TOP.x + 1.5, BOX_TOP.z + 1.5])
        .filter(|&n| (grid.position(n).y - BOX_TOP.y).abs() <= 0.5)
        .collect();
    let high: Vec<_> = grid.off_links().iter().filter(|l| near_box.contains(&l.from) && grid.position(l.to).y - grid.position(l.from).y > RISE_MAX_M).collect();
    eprintln!("WALKFACE links from the box top's {} floors rising over {RISE_MAX_M} m: {}", near_box.len(), high.len());
    assert!(high.is_empty(), "links up from the box top: {high:?}");
    assert!(path::plan(grid, from, to).is_none(), "a way planned to the pillar's top");
    eprintln!("WALKFACE there none: no way planned to the pillar's top");
}
