//! The nav problem spots (NAV FIRST, A313) walked by the client's own controller (`mapdata::controller`, ported from the exe and
//! proven in Unicorn), on the game's maps: each row of `data/controller-spots.tsv` is a walk from one feet position toward
//! another and what real bodies do there. A row the controller gets wrong is a gap in the port or the export; a row it gets
//! right while the walk grid gets it wrong is the builder's to fix. They run only when `SRP_MAPDATA_DIR` names the
//! `mapdata-dump` files (and skip with a note otherwise):
//!
//! ```text
//! SRP_MAPDATA_DIR=<dir of .mapdata files> cargo test -p mapdata --release --test controller_spots -- --nocapture
//! ```

use mapdata::controller::Collision;
use mapdata::controller::tick::{Controller, Settings};
use mapdata::{MapData, Vec3};

/// The walk tick, s (the client's 30 updates a second).
const DT: f32 = 1.0 / 30.0;
/// The forward walk speed, m/s (`notes-speed.md`: walk forward 3.5 at a speed factor of 1).
const WALK_MPS: f32 = 3.5;
/// Reached: this near the target across the ground, m, and this near its height.
const NEAR_M: f32 = 0.4;
const NEAR_Y_M: f32 = 0.4;

struct Row {
    project: String,
    from: Vec3,
    to: Vec3,
    walk: bool,
    spot: String,
}

fn rows() -> Vec<Row> {
    include_str!("data/controller-spots.tsv")
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            let n = |i: usize| f[i].parse::<f32>().unwrap_or_else(|e| panic!("{l}: column {i}: {e}"));
            Row {
                project: f[0].to_owned(),
                from: Vec3::new(n(1), n(2), n(3)),
                to: Vec3::new(n(4), n(5), n(6)),
                walk: match f[7] {
                    "walk" => true,
                    "no" => false,
                    other => panic!("{l}: expect is walk or no, not {other}"),
                },
                spot: f[8].to_owned(),
            }
        })
        .collect()
}

/// The walk from `from` toward `to`, steered at it every tick: whether the feet come within reach of it, and where they ended.
fn walk(world: &Collision, from: Vec3, to: Vec3) -> (bool, Vec3) {
    let s = Settings::LIVE;
    let mut body = Controller::standing_at(from);
    let flat = |a: Vec3, b: Vec3| ((a.x - b.x).powi(2) + (a.z - b.z).powi(2)).sqrt();
    let ticks = ((flat(from, to) / 3.0 + 2.0) * 2.0 / DT) as usize;
    for _ in 0..ticks {
        let feet = body.feet();
        if flat(feet, to) <= NEAR_M && (feet.y - to.y).abs() <= NEAR_Y_M {
            return (true, feet);
        }
        let d = flat(feet, to).max(1e-3);
        let step = (WALK_MPS * DT).min(d);
        body.walk = Vec3::new((to.x - feet.x) / d * step, 0.0, (to.z - feet.z) / d * step);
        body.step(world, DT, &s);
    }
    let feet = body.feet();
    (flat(feet, to) <= NEAR_M && (feet.y - to.y).abs() <= NEAR_Y_M, feet)
}

#[test]
fn the_controller_walks_the_nav_problem_spots_as_real_bodies_do() {
    let Some(dir) = std::env::var_os("SRP_MAPDATA_DIR") else {
        eprintln!("skipping: SRP_MAPDATA_DIR is not set");
        return;
    };
    let mut worlds = std::collections::HashMap::new();
    let mut wrong = Vec::new();
    for row in rows() {
        let world = worlds.entry(row.project.clone()).or_insert_with(|| {
            let path = std::path::Path::new(&dir).join(mapdata::format::file_name(&row.project));
            MapData::read(&path).ok().map(|map| Collision::build(&map.walker_triangles()))
        });
        let Some(world) = world else {
            eprintln!("skipping {}: no map file", row.project);
            continue;
        };
        let (reached, end) = walk(world, row.from, row.to);
        let line = format!(
            "{} ({:.2}, {:.2}, {:.2}) -> ({:.2}, {:.2}, {:.2}): expect {}, {} (ended at ({:.2}, {:.2}, {:.2})); {}",
            row.project,
            row.from.x,
            row.from.y,
            row.from.z,
            row.to.x,
            row.to.y,
            row.to.z,
            if row.walk { "walk" } else { "no" },
            if reached { "reached" } else { "not reached" },
            end.x,
            end.y,
            end.z,
            row.spot
        );
        println!("{line}");
        if reached != row.walk {
            wrong.push(line);
        }
    }
    assert!(wrong.is_empty(), "the controller differs from the real bodies at:\n{}", wrong.join("\n"));
}
