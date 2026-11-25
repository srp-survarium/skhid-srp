//! [deep walk-offs] Per-spot check of a drop's deep landing kept beside a way within the harmless fall, on the real maps, in a
//! 30 m box round the spot built as the server builds its grids (the tuning's body and links, the game's shapes):
//!
//! - School batteries, human crossing 13314: the 8.99 m walk-off (-3.30, 26.09, 106.97) -> (-5.55, 17.10, 106.72), lost when
//!   the way within the harmless fall went first (#2319), is back beside it.
//! - Rudnya batteries, human crossing 4837 (body physics S5c): the full-share Slow 0.6 jump onto the human's landing top is kept
//!   beside L1's link onto the top 1.25 m across (the gap links' per-metre key keeps a jump link to each landing top).
//!
//! ```text
//! SRP_MAPDATA_DIR=<dir> SRP_GAME_DATA=<server-game-data.db> cargo test -p match-server --release deep_landing_spots -- --ignored --nocapture
//! ```

use super::*;

/// Half the box's side round each spot, m.
const HALF_M: f32 = 15.0;
/// How near a link's end is to a spot's point to be that point's, m (the points are given to 0.01 m).
const NEAR_M: f32 = 0.4;

/// The grid of `project` in the box round `at`, with its build's counts; seeded at `seeds`.
fn boxed(dir: &std::path::Path, project: &str, at: Vec3, seeds: &[Vec3]) -> (mapdata::WalkGrid, mapdata::BuildStats) {
    let t = Tuning::default();
    let map = mapdata::MapData::read(&dir.join(mapdata::format::file_name(project))).expect("map");
    let full = map.play_area(mapdata::AREA_MARGIN);
    let cell = mapdata::CELL;
    let snap = |v: f32, o: f32| o + ((v - o) / cell).floor() * cell;
    let area = [snap(at.x - HALF_M, full[0]), snap(at.z - HALF_M, full[1]), snap(at.x + HALF_M, full[0]), snap(at.z + HALF_M, full[1])];
    let inside = |tr: &mapdata::Triangle| {
        let (lx, hx) = (tr.v.iter().map(|v| v.x).fold(f32::MAX, f32::min), tr.v.iter().map(|v| v.x).fold(f32::MIN, f32::max));
        let (lz, hz) = (tr.v.iter().map(|v| v.z).fold(f32::MAX, f32::min), tr.v.iter().map(|v| v.z).fold(f32::MIN, f32::max));
        hx >= area[0] - 1.0 && lx <= area[2] + 1.0 && hz >= area[1] - 1.0 && lz <= area[3] + 1.0
    };
    let walker: Vec<mapdata::Triangle> = map.walker_triangles().into_iter().filter(inside).collect();
    let shapes = mapdata::controller::Collision::build_exe(&map);
    mapdata::WalkGrid::build_kept_with_shapes(&walker, shapes.as_ref(), area, cell, t.grid_body(), seeds, seeds, t.off_mesh_links())
}

/// School batteries, human crossing 13314: the walk-off (-3.30, 26.09, 106.97) -> (-5.55, 17.10, 106.72), 8.99 m, is a link
/// again (an off-mesh GapDrop walk-off beside the way within the harmless fall), and the route prices it over that way.
#[test]
#[ignore = "a per-spot test on the real mapdata; needs SRP_MAPDATA_DIR and SRP_GAME_DATA"]
fn schools_deep_walk_off_of_crossing_13314_is_kept() {
    let Some(dir) = super::real_maps::dir() else {
        eprintln!("skipping: SRP_MAPDATA_DIR is not set");
        return;
    };
    let _ = survarium::v026g::game_data::init();
    let (from, to) = (Vec3::new(-3.30, 26.09, 106.97), Vec3::new(-5.55, 17.10, 106.72));
    let (grid, stats) = boxed(&dir, "level_02/day_2", from, &[from, to]);
    eprintln!("DEEPSPOT 13314 kept beside: {} {:?}", stats.drops_past_harmless_also.len(), stats.drops_walked);
    let a = grid.locate(from, NEAR_M).expect("the take-off's floor");
    let link = grid.off_links().iter().find(|l| l.from == a && (grid.position(l.to) - to).length() < NEAR_M);
    eprintln!("DEEPSPOT 13314 {:?} -> {link:?}", grid.position(a));
    let link = link.expect("the 8.99 m walk-off of crossing 13314");
    assert_eq!(link.kind, mapdata::LinkKind::GapDrop);
    assert!(grid.link_cost(a, link.to, link.kind) > (grid.position(a) - grid.position(link.to)).length(), "priced by the fall's damage");
}

/// [body physics S5c] Rudnya batteries, human crossing 4837: the full-share Slow 0.6 jump (-42.57, 26.45, 9.35) ->
/// (-43.07, 26.51, 9.35) is kept beside L1's Press onto the top 1.25 m across (-44.07, 26.55, 8.60): the gap links' per-metre key
/// keeps a jump link to each landing top, so the human's landing floor has a way in again.
#[test]
#[ignore = "a per-spot test on the real mapdata; needs SRP_MAPDATA_DIR and SRP_GAME_DATA"]
fn rudnyas_jump_of_crossing_4837_keeps_both_landing_tops() {
    let Some(dir) = super::real_maps::dir() else {
        eprintln!("skipping: SRP_MAPDATA_DIR is not set");
        return;
    };
    let _ = survarium::v026g::game_data::init();
    let (from, own, other) = (Vec3::new(-42.57, 26.45, 9.35), Vec3::new(-43.07, 26.51, 9.35), Vec3::new(-44.07, 26.55, 8.60));
    let (grid, _) = boxed(&dir, "level_01/day_2", from, &[from, own]);
    let a = grid.locate(from, NEAR_M).expect("the take-off's floor");
    let onto = |p: Vec3| grid.off_links().iter().find(|l| l.from == a && (grid.position(l.to) - p).length() < NEAR_M).copied();
    let (to_own, to_other) = (onto(own), onto(other));
    eprintln!("GAPTOP 4837 {:?}: own {to_own:?} other {to_other:?}", grid.position(a));
    let link = to_own.expect("the Slow 0.6 onto the human's landing top");
    assert_eq!(link.kind, mapdata::LinkKind::GapJump);
    assert!(link.share.mask().is_some_and(|m| m.lands(0)), "its own flights land: {link:?}");
    assert!(to_other.is_some(), "L1's link onto the other top is kept too");
}
