//! Upper levels: whether a floor has another floor a storey or more under it (a second floor, a walkway, a roof over a
//! room), for the counts of how much of their time bodies spend on them: the players' (the routes study's positions,
//! `routes::tests::how_reachable_the_real_positions_are`) and the bots' (the bench's `upper:` line), by one rule.

use mapdata::{NodeId, WalkGrid};

/// A floor this far or more under another is a storey under it, m.
pub(crate) const STOREY_M: f32 = 2.0;
/// ... when it lies within this of it on the ground plane, m (a corpus spot's reach, `routes::tests::SPOT_XZ_M`).
const NEAR_M: f32 = 0.6;

/// How far under `node` the nearest floor a storey or more under it lies; `None`: `node` is the lowest floor there.
pub(crate) fn storey_under(grid: &WalkGrid, node: NodeId) -> Option<f32> {
    let at = grid.position(node);
    grid.floors_in([at.x - NEAR_M, at.z - NEAR_M, at.x + NEAR_M, at.z + NEAR_M])
        .map(|other| grid.position(other))
        .filter(|under| under.distance_xz(at) <= NEAR_M && at.y - under.y >= STOREY_M)
        .map(|under| at.y - under.y)
        .min_by(f32::total_cmp)
}

/// The walkway 3 m over the field is an upper level; the field under it and beside it is not.
#[test]
fn a_walkway_over_the_field_is_an_upper_level_and_the_field_is_not() {
    use mapdata::Vec3;
    let world = super::walkway::walkway_world();
    let grid = &world.grid;
    let on = |x: f32, y: f32, z: f32| grid.locate(Vec3::new(x, y, z), 0.3).unwrap_or_else(|| panic!("no floor at {x} {y} {z}"));
    let under = storey_under(grid, on(20.0, 3.0, 32.5)).expect("the walkway has the field under it");
    assert!((under - 3.0).abs() < 0.2, "{under}");
    assert_eq!(storey_under(grid, on(20.0, 0.0, 32.5)), None, "the field under the walkway");
    assert_eq!(storey_under(grid, on(50.0, 0.0, 20.0)), None, "the open field");
}
