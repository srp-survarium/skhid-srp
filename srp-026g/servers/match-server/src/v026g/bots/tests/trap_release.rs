//! [A324] How often a trap the habit lets go of lies nowhere by the client's rule (`traps.release`): on every map of
//! `SRP_MAPDATA_DIR`, from walkable floor points (the walker mesh's triangles walkable by the grid's slope, every few) and eight
//! yaws, the habit's press look for both poses; the old rule (`plane`: the head ray on the flat plane at the feet' height) against
//! the client's (`client_landing` on the bots' bullet mesh, and on the walker mesh the client itself casts on). Prints a line a
//! map; asserts nothing beyond the run (`--ignored`). The bench run had 15 of 54 releases put no trap down (28 %).

use mapdata::{MapData, RayTree, Vec3};

use crate::v026g::bots::traps::{self, Why};
use crate::v026g::bots::tuning::Tuning;
use crate::v026g::bots::view::eye_height;

/// A walkable floor triangle of the grid's slope rule (cos 55 deg).
const WALK_Y: f32 = 0.573;
/// Every this many walkable triangles is a floor point.
const STRIDE: usize = 7;
/// At most this many floor points a map.
const MAX_POINTS: usize = 4_000;

#[derive(Default)]
struct Counts {
    looks: u32,
    /// The old rule puts a trap down (a release).
    plane: u32,
    /// Of them, the client places none (on the bullet mesh), by reason: no floor within 2 m, a slope.
    refused_floor: u32,
    refused_slope: u32,
    /// The client's verdict on the walker mesh differs from the bullet mesh's.
    meshes_differ: u32,
    /// Placed by both rules, more than 0.5 m apart.
    moved: u32,
}

#[test]
#[ignore]
fn a_trap_let_go_by_the_plane_rule_lies_nowhere_this_often() {
    let Some(dir) = std::env::var_os("SRP_MAPDATA_DIR") else {
        eprintln!("skipping: SRP_MAPDATA_DIR is not set");
        return;
    };
    let t = Tuning::default().traps;
    let run_m = traps::wanted_m(&t, t.press_landing_m.unwrap_or(1.0));
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == mapdata::format::EXTENSION))
        .collect();
    files.sort();
    let mut all = Counts::default();
    for path in files {
        let Ok(map) = MapData::read(&path) else {
            continue;
        };
        let walker = map.walker_triangles();
        let bullet = RayTree::from_corners(map.bullet_triangles().iter().map(|t| t.v));
        let walk_tree = RayTree::build(&walker);
        let floors: Vec<Vec3> = walker
            .iter()
            .filter(|t| {
                let n = (t.v[1] - t.v[0]).cross(t.v[2] - t.v[0]);
                n.length() > 1e-6 && (n.y / n.length()).abs() >= WALK_Y
            })
            .step_by(STRIDE)
            .take(MAX_POINTS)
            .map(|t| (t.v[0] + t.v[1] + t.v[2]) * (1.0 / 3.0))
            .collect();
        let mut c = Counts::default();
        for &feet in &floors {
            for crouched in [false, true] {
                let Some(look) = traps::press_look(crouched, run_m, 0.0) else {
                    continue;
                };
                let pitch = -look.pitch.0;
                let eye = feet + Vec3::new(0.0, eye_height(crouched, 0.0, pitch), 0.0);
                for k in 0..8 {
                    let yaw = k as f32 * std::f32::consts::FRAC_PI_4;
                    c.looks += 1;
                    let Some(plane) = traps::landing(eye, yaw, pitch, feet.y) else {
                        continue;
                    };
                    c.plane += 1;
                    let on_bullet = traps::client_landing(&bullet, eye, yaw, pitch, &[]);
                    let on_walker = traps::client_landing(&walk_tree, eye, yaw, pitch, &[]);
                    if on_bullet.is_ok() != on_walker.is_ok() {
                        c.meshes_differ += 1;
                    }
                    match on_bullet {
                        Err(Why::Slope) => c.refused_slope += 1,
                        Err(_) => c.refused_floor += 1,
                        Ok(at) if (at - plane).length() > 0.5 => c.moved += 1,
                        Ok(_) => {}
                    }
                }
            }
        }
        let pct = |n: u32| 100.0 * f64::from(n) / f64::from(c.plane.max(1));
        eprintln!(
            "trap release {}: floor points {}, looks {}, plane releases {}, client refuses {:.1} % (no floor {:.1} %, slope {:.1} %), placed 0.5 m+ off the plane point {:.1} %, bullet vs walker verdict differs {:.1} %",
            path.file_stem().map_or(String::new(), |s| s.to_string_lossy().into_owned()),
            floors.len(),
            c.looks,
            c.plane,
            pct(c.refused_floor + c.refused_slope),
            pct(c.refused_floor),
            pct(c.refused_slope),
            pct(c.moved),
            pct(c.meshes_differ),
        );
        all.looks += c.looks;
        all.plane += c.plane;
        all.refused_floor += c.refused_floor;
        all.refused_slope += c.refused_slope;
        all.meshes_differ += c.meshes_differ;
        all.moved += c.moved;
    }
    let pct = |n: u32| 100.0 * f64::from(n) / f64::from(all.plane.max(1));
    eprintln!(
        "trap release ALL: plane releases {}, client refuses {:.1} % (no floor {:.1} %, slope {:.1} %), moved 0.5 m+ {:.1} %, bullet vs walker differs {:.1} %",
        all.plane,
        pct(all.refused_floor + all.refused_slope),
        pct(all.refused_floor),
        pct(all.refused_slope),
        pct(all.moved),
        pct(all.meshes_differ),
    );
}
