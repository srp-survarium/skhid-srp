//! A route point goes on the side of a wall the humans walked (`GridTuning::snap_routes_walked`,
//! `Tactics::set_routes_with`).

use mapdata::{MapData, RayTree, Spawn, Triangle, WalkGrid, WalkerBehaviour};

use super::*;
use crate::v026g::bots::body::tests::floor;
use crate::v026g::bots::routes::HumanRoute;

/// One open floor, 80 x 40 m, with a thin wall 3.8 m tall along z = 17.65 from x = 0 to x = 60: the way from one side
/// to the other is round its east end.
fn walled() -> Arc<World> {
    let mut tris: Vec<Triangle> = Vec::new();
    tris.extend(floor(0.0, 0.0, 80.0, 40.0, 0.0));
    let walker = WalkerBehaviour::BySlope;
    let (a, b) = ((0.0, 17.65), (60.0, 17.65));
    let (p, q) = (Vec3::new(a.0, 0.0, a.1), Vec3::new(b.0, 0.0, b.1));
    let (r, s) = (Vec3::new(b.0, 3.8, b.1), Vec3::new(a.0, 3.8, a.1));
    tris.push(Triangle { v: [p, q, r], walker });
    tris.push(Triangle { v: [p, r, s], walker });
    let spawns = [Vec3::new(5.0, 0.0, 10.0), Vec3::new(75.0, 0.0, 10.0)];
    let grid = WalkGrid::build_kept(&tris, [0.0, 0.0, 80.0, 40.0], 0.25, mapdata::Body::default(), &spawns, &spawns);
    let spawn = |team: i32, at: Vec3| Spawn {
        position: at,
        rotation: Vec3::ZERO,
        team,
        priority: 0,
        point_id: 0,
        layer: "default".into(),
    };
    let map = MapData {
        spawns: vec![spawn(0, spawns[0]), spawn(1, spawns[1])],
        ..MapData::default()
    };
    let sight = RayTree::build(&tris);
    Arc::new(World::assemble(map, grid.into(), sight.into()))
}

fn route(points: &[(f32, f32)]) -> HumanRoute {
    HumanRoute {
        team: 0,
        lives: 1,
        points: points.iter().map(|&(x, z)| Vec3::new(x, 0.0, z)).collect(),
        ..Default::default()
    }
}

/// The points a table of one route comes out with, and how many moved.
fn snapped(points: &[(f32, f32)], walked: bool) -> (Vec<Vec3>, usize) {
    let mut tactics = Tactics::new(walled(), 1.0);
    let moved = tactics.set_routes_with(vec![route(points)], walked);
    (tactics.routes_of(0).next().expect("a route").points.clone(), moved)
}

/// A point 0.15 m past a wall, a metre from the floor on the side the route comes from: the nearest floor is on the
/// far side, a 100 m walk round the wall, and the walked side is 12 m from the point before it (Tarakanovsky Fort
/// route 88's point 2).
#[test]
fn a_point_just_past_a_wall_goes_on_the_side_the_route_walks() {
    let (nearest, moved) = snapped(&[(10.0, 5.0), (10.0, 17.8)], false);
    assert_eq!(moved, 0);
    assert!(nearest[1].z > 17.65, "{nearest:?}: the nearest floor is on the far side");
    let (walked, moved) = snapped(&[(10.0, 5.0), (10.0, 17.8)], true);
    assert_eq!(moved, 1);
    assert!(walked[1].z < 17.65, "{walked:?}");
    assert_eq!(walked[0], nearest[0]);
}

/// A point that is a few metres past the wall (its own floor is not within a metre), and a route with no wall in the
/// way, are left where the nearest floor is.
#[test]
fn a_point_clear_of_the_wall_and_a_route_with_no_detour_stay_put() {
    let far = [(10.0, 5.0), (10.0, 20.0)];
    let (nearest, _) = snapped(&far, false);
    let (walked, moved) = snapped(&far, true);
    assert_eq!((moved, &walked), (0, &nearest));
    let open = [(10.0, 5.0), (30.0, 10.0), (70.0, 30.0)];
    let (nearest, _) = snapped(&open, false);
    let (walked, moved) = snapped(&open, true);
    assert_eq!((moved, &walked), (0, &nearest));
}

/// The first point has no point before it: it stays the nearest floor.
#[test]
fn the_first_point_is_not_moved() {
    let (nearest, _) = snapped(&[(10.0, 17.8), (10.0, 5.0)], false);
    let (walked, moved) = snapped(&[(10.0, 17.8), (10.0, 5.0)], true);
    assert_eq!((moved, walked[0]), (0, nearest[0]));
}
