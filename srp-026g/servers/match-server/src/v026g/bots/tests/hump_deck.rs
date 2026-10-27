//! Cologne's deck (-31.8, 35.6, 28.6, day_2): the route of a Collect walk crosses it in one 11 m leg between two lower
//! ends, over a floor up to 1.6 m higher than the line between them (`Feel::floor_height_clear`).

use super::*;

use crate::v026g::bots::path::{self, RouteHeight};

/// How many bodies on the route's floor near the deck have the point 1 m on clear at each height of that point (the
/// walk's test: the grid finds a floor under it, within 1.0 m over it, and the line to it is a walk). A measurement, run with
/// `SRP_MAPDATA_DIR=<mapdata folder> cargo test -p match-server --release deck_clear -- --ignored --nocapture`.
#[test]
#[ignore = "a measurement; needs SRP_MAPDATA_DIR"]
fn deck_clear() {
    let Some(dir) = std::env::var_os("SRP_MAPDATA_DIR").map(std::path::PathBuf::from) else {
        eprintln!("skipping: SRP_MAPDATA_DIR is not set");
        return;
    };
    let map = mapdata::MapData::read(&dir.join(mapdata::format::file_name("level_08/day_2"))).unwrap();
    let links = Tuning::default().grid.off_mesh_links();
    let world = Arc::new(World::build_with_links(map, mapdata::CELL, mapdata::Body::default(), links));
    let grid = &world.grid;
    let deck = Vec3::new(-31.8, 35.6, 28.6);
    let start = grid.locate(Vec3::new(-38.2, 35.6, 33.6), 1.0).unwrap();
    let goal = path::goal_floor(grid, Vec3::new(-14.5, 32.1, 21.7)).unwrap();
    let route = path::plan(grid, start, goal).unwrap();
    // The walk's clear(): `line_ends` (a floor within -2.5..1.0 m of the point's height) and a straight walk to it.
    let clear = |a: Vec3, b: Vec3| {
        let from = grid.locate(a, 1.0)?;
        let dy = |n: u32| grid.position(n).y - b.y;
        let to = grid
            .floors_in([b.x, b.z, b.x, b.z])
            .filter(|&n| (-2.5..=1.0).contains(&dy(n)))
            .min_by(|&m, &n| dy(m).abs().total_cmp(&dy(n).abs()))?;
        Some(path::straight_where(grid, from, to, |_| true))
    };
    let (mut near, mut by_chord, mut by_floor) = (0, 0, 0);
    let mut s = 0.0;
    while s + 1.0 < route.length() {
        let body = route.floor_point(s);
        if body.distance_xz(deck) < 5.0 {
            near += 1;
            by_chord += usize::from(clear(body, route.at(s + 1.0, RouteHeight::Chord)) == Some(true));
            by_floor += usize::from(clear(body, route.at(s + 1.0, RouteHeight::Floor)) == Some(true));
        }
        s += 0.5;
    }
    println!("deck: {near} bodies on the route's floor within 5 m, the point 1 m on clear at the chord's height {by_chord}, at the floor's {by_floor}");
    assert!(near > 0 && by_floor == near && by_chord < near, "{near} {by_chord} {by_floor}");
}
