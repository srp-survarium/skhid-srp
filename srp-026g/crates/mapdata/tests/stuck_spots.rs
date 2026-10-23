//! The spots where the live server's bots stood caught
//! (`docs/notes/player_bots_stuck.md`), on the game's own maps: each was a
//! way the walk grid had that a body does not get along. They run only
//! when `SRP_MAPDATA_DIR` names the `mapdata-dump` files (and skip with a
//! note otherwise):
//!
//! ```text
//! SRP_MAPDATA_DIR=<dir of .mapdata files> cargo test -p mapdata --release --test stuck_spots
//! ```

use mapdata::{Body, LinkKind, MapData, NodeId, Vec3, WalkGrid};

fn grid(project: &str) -> Option<WalkGrid> {
    let Some(dir) = std::env::var_os("SRP_MAPDATA_DIR") else {
        eprintln!("skipping: SRP_MAPDATA_DIR is not set");
        return None;
    };
    let path = std::path::Path::new(&dir).join(mapdata::format::file_name(project));
    let map = match MapData::read(&path) {
        Ok(map) => map,
        Err(error) => {
            eprintln!("skipping: {} ({error})", path.display());
            return None;
        }
    };
    Some(WalkGrid::build_kept(
        &map.walker_triangles(),
        map.play_area(mapdata::AREA_MARGIN),
        mapdata::CELL,
        Body::default(),
        &map.grid_seeds(),
    ))
}

/// The floors in an x/z box between two heights.
fn floors(grid: &WalkGrid, area: [f32; 4], y: (f32, f32)) -> Vec<Vec3> {
    grid.floors_in(area)
        .map(|n| grid.position(n))
        .filter(|p| {
            p.x > area[0]
                && p.x < area[2]
                && p.z > area[1]
                && p.z < area[3]
                && p.y > y.0
                && p.y < y.1
        })
        .collect()
}

/// Cologne (level_08), the hall with the rails: a workbench
/// (`props/furniture/workbench_02`, its walker box x -50.64..-49.14,
/// z 42.53..43.58, 0.92 m high) stands 0.64 m off a half-wall (its face at
/// z 41.89). The grid had a way between them, beside the bench, for it is
/// jumped on; the body (0.76 m) does not pass. The left team's bots on
/// their way out of their base stood caught there for minutes in 68
/// matches (player report of 2026-10-23 with screenshots).
#[test]
fn cologne_no_way_between_the_workbench_and_the_half_wall() {
    for project in ["level_08/day_4", "level_08/day_2"] {
        let Some(grid) = grid(project) else {
            return;
        };
        // The bench is turned 1°: its box's corners are a few
        // centimetres off the bounds above.
        let r = Body::default().radius - 0.03;
        let ground = (31.6, 32.6);
        // Beside its side to the wall, and its end to the west.
        let side = floors(&grid, [-50.64, 42.53 - r, -49.14, 42.53], ground);
        let end = floors(&grid, [-50.64 - r, 42.53, -50.64, 43.58], ground);
        assert!(
            side.is_empty() && end.is_empty(),
            "{project}: {side:?} {end:?}"
        );
        // From where the bots stood, no straight walk on east past it.
        let caught = grid
            .locate(Vec3::new(-50.966, 32.068, 42.307), 0.3)
            .unwrap();
        let east: Vec<NodeId> = grid
            .floors_in([-48.9, 42.3, -48.7, 42.45])
            .filter(|&n| (grid.position(n).y - 32.0).abs() < 0.5)
            .collect();
        for to in east {
            assert!(!grid.walks_straight(caught, to, 5.0, |_| true), "{project}");
        }
        // The bench is still jumped on, from the open side.
        let top = grid.locate(Vec3::new(-49.9, 32.93, 43.0), 0.3).unwrap();
        let open = grid.locate(Vec3::new(-49.9, 32.0, 44.5), 0.5).unwrap();
        assert!(grid.distances_from(&[open]).reaches(top), "{project}");
    }
}

/// level_02 (TDM and batteries): an anomalous tree whose walker collision
/// the archive lacks. Bodies stood stuck in it for minutes, two at once;
/// its bullet mesh now stands in, and the spot is no floor.
#[test]
fn level_02_the_anomalous_tree_is_in_the_way() {
    for project in ["level_02/day_4", "level_02/day_2"] {
        let Some(grid) = grid(project) else {
            return;
        };
        let near = floors(&grid, [18.6, 118.5, 19.05, 118.9], (15.7, 16.7));
        assert!(near.is_empty(), "{project}: {near:?}");
    }
}

/// level_02: a platform 1.24-1.49 m over a rubble slope. The grid had a
/// jump up (to 1.3 m); bots jumped at its edge and stayed below (2,200
/// stuck lines in 40 matches). No jump goes up there now.
#[test]
fn level_02_no_jump_up_the_high_platform() {
    for project in ["level_02/day_4", "level_02/day_2"] {
        let Some(grid) = grid(project) else {
            return;
        };
        for n in grid.floors_in([20.0, 63.5, 27.5, 64.7]) {
            let p = grid.position(n);
            if !(15.5..16.6).contains(&p.y) {
                continue;
            }
            for (to, kind) in grid.links(n) {
                let q = grid.position(to);
                assert!(
                    !(kind == LinkKind::Jump && q.y > 17.0),
                    "{project}: a jump from {p:?} to {q:?}"
                );
            }
        }
    }
}

/// level_07: a wall 1 m high and a cell thick, ground both sides. The grid
/// had a jump onto its top; bots jumped at it for minutes. No jump east
/// lands on it now.
#[test]
fn level_07_no_jump_onto_the_thin_wall() {
    for project in ["level_07/day_4", "level_07/day_2"] {
        let Some(grid) = grid(project) else {
            return;
        };
        for n in grid.floors_in([-68.6, -12.6, -68.0, -11.4]) {
            let p = grid.position(n);
            if (p.y - 24.85).abs() > 0.3 {
                continue;
            }
            for (to, kind) in grid.links(n) {
                let q = grid.position(to);
                assert!(
                    !(kind == LinkKind::Jump && (q.z - p.z).abs() < 0.1),
                    "{project}: a jump from {p:?} to {q:?}"
                );
            }
        }
    }
}
