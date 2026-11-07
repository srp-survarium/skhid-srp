//! [A318] Where the client's walk tick holds a body walking up to a route jump's take-off (a grid `Jump` link: up a ledge higher
//! than a step), against the motor's press distance (`motor::PRESS_M` past `JUMP_LEAD_S` at the body's speed). A body the client
//! holds at the ledge's face stands still, so it presses only within `PRESS_M` of the take-off; under `kin.client_step = client`
//! the reckoned body is held where the client holds it. A lab: per map, every Jump link's held distance before the take-off.
//!
//! ```text
//! SRP_MAPDATA_DIR=<dir> cargo test -p match-server --release press_reach -- --ignored --nocapture
//! ```

use mapdata::{grid::LinkKind, Vec3};

use super::bench_settings;

/// The motor's press reach for a body standing still (`motor::PRESS_M`).
const PRESS_M: f32 = 0.15;
/// The walk toward the take-off starts this far back along the jump's way, m.
const START_BACK_M: f32 = 1.5;
/// A walk tick's move at the walk speed (`grid::controlled::WALK_MPS` 3.5 m/s, one 0.05 s tick).
const TICK_MOVE_M: f32 = 3.5 * 0.05;

#[test]
#[ignore = "a lab; needs SRP_MAPDATA_DIR"]
fn press_reach() {
    let Some(dir) = bench_settings().var_os("SRP_MAPDATA_DIR").map(std::path::PathBuf::from) else {
        eprintln!("skipping: SRP_MAPDATA_DIR is not set");
        return;
    };
    let tuning = super::bench::bench_tuning();
    for project in ["level_01/day_2", "level_03/day_2", "level_04/day_2", "level_05/day_2", "level_06/day_2", "level_07/day_2", "level_08/day_2"] {
        let file = dir.join(mapdata::format::file_name(project));
        let Ok(map) = mapdata::MapData::read(&file) else {
            eprintln!("press_reach {project}: no map file");
            continue;
        };
        let world = mapdata::World::build_with(map, mapdata::CELL, tuning.grid_body(), tuning.off_mesh_links(), false, None);
        let grid = &world.grid;
        let (mut links, mut held_out, mut reached, mut no_start) = (0usize, 0usize, 0usize, 0usize);
        let mut befores = Vec::new();
        for from in 0..grid.len() as mapdata::grid::NodeId {
            for (to, kind) in grid.links(from) {
                if kind != LinkKind::Jump {
                    continue;
                }
                links += 1;
                let (take_off, landing) = (grid.position(from), grid.position(to));
                let (dx, dz) = (landing.x - take_off.x, landing.z - take_off.z);
                let l = dx.hypot(dz);
                if l < 1e-3 {
                    continue;
                }
                let way = (dx / l, dz / l);
                let start = Vec3::new(take_off.x - way.0 * START_BACK_M, take_off.y, take_off.z - way.1 * START_BACK_M);
                let Some(node) = grid.locate(start, 0.3) else {
                    no_start += 1;
                    continue;
                };
                let mut feet = Vec3::new(start.x, grid.position(node).y, start.z);
                let by = Vec3::new(way.0 * TICK_MOVE_M, 0.0, way.1 * TICK_MOVE_M);
                let mut before = f32::INFINITY;
                for _ in 0..40 {
                    let Some((from_feet, end)) = grid.client_step(feet, by) else { break };
                    let next = Vec3::new(feet.x + end.x - from_feet.x, end.y, feet.z + end.z - from_feet.z);
                    let moved = next.distance_xz(feet);
                    feet = next;
                    before = -((feet.x - take_off.x) * way.0 + (feet.z - take_off.z) * way.1);
                    if moved < 1e-3 || before <= 0.0 || feet.y > take_off.y + 0.3 {
                        break;
                    }
                }
                if before <= PRESS_M {
                    reached += 1;
                } else {
                    held_out += 1;
                }
                befores.push(before);
            }
        }
        befores.sort_by(f32::total_cmp);
        let q = |p: f32| befores.get(((befores.len() as f32 - 1.0) * p) as usize).copied().unwrap_or(f32::NAN);
        eprintln!(
            "press_reach {project}: jump links {links}, no start {no_start}, reached {reached}, held short of the press {held_out} ({:.1} %); before m p25 {:.3} p50 {:.3} p75 {:.3} p90 {:.3}",
            100.0 * held_out as f32 / (reached + held_out).max(1) as f32,
            q(0.25),
            q(0.5),
            q(0.75),
            q(0.9)
        );
    }
}
