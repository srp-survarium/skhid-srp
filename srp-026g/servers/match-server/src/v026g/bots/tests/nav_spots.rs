//! [A313, NAV FIRST] The nav regression spots: each row of `data/nav-spots.tsv` is a walk real bodies make (humans' passes, a
//! client's), checked twice on the game's own maps:
//! - the client's own walk tick (`mapdata::controller`, ported from the exe and proven in Unicorn) gets the body there or not, as
//!   the row says: a row it gets wrong is the port's or the export's;
//! - the default grid (the server's own body and links, `Tuning::default()`) has a way there or not, as the row says: today's
//!   state, so a nav change that opens or closes a spot flips its row in the same PR, and anything else that moves is a regression.
//!
//! Every nav PR runs it (no bench). They run only when `SRP_MAPDATA_DIR` names the `mapdata-dump` files (and skip with a note
//! otherwise):
//!
//! ```text
//! SRP_MAPDATA_DIR=<dir of .mapdata files> cargo test -p match-server --release --lib nav_spots -- --nocapture
//! ```

use std::collections::{BinaryHeap, HashMap};

use mapdata::controller::Collision;
use mapdata::controller::tick::{Controller, Settings};
use mapdata::{MapData, NodeId, Vec3, WalkGrid};

use crate::v026g::bots::tuning::Tuning;

/// The walk tick, s (the client's 30 updates a second), and the forward walk speed, m/s (walk forward 3.5).
const DT: f32 = 1.0 / 30.0;
const WALK_MPS: f32 = 3.5;
/// Reached: this near the target across the ground, m, and this near its height.
const NEAR_M: f32 = 0.4;
const NEAR_Y_M: f32 = 0.4;
/// A grid way counts when no longer than this times the straight line plus this, m (every link kind; a link's length on the
/// ground plus its rise): the Vector strip's cut made 31-35 m of 2-5 m.
const WAY_FACTOR: f32 = 2.0;
const WAY_PLUS_M: f32 = 4.0;
/// The grid's floor for a point: within this across the ground and this in height.
const FLOOR_M: f32 = 0.5;
const FLOOR_Y_M: f32 = 0.5;

struct Row {
    project: String,
    from: Vec3,
    to: Vec3,
    walks: bool,
    /// None: printed, not asserted.
    way: Option<bool>,
    spot: String,
}

fn rows() -> Vec<Row> {
    include_str!("data/nav-spots.tsv")
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            assert_eq!(f.len(), 10, "{l}: 10 columns");
            let n = |i: usize| f[i].parse::<f32>().unwrap_or_else(|e| panic!("{l}: column {i}: {e}"));
            Row {
                project: f[0].to_owned(),
                from: Vec3::new(n(1), n(2), n(3)),
                to: Vec3::new(n(4), n(5), n(6)),
                walks: match f[7] {
                    "walk" => true,
                    "no" => false,
                    other => panic!("{l}: controller is walk or no, not {other}"),
                },
                way: match f[8] {
                    "way" => Some(true),
                    "none" => Some(false),
                    "?" => None,
                    other => panic!("{l}: grid is way, none or ?, not {other}"),
                },
                spot: f[9].to_owned(),
            }
        })
        .collect()
}

fn flat(a: Vec3, b: Vec3) -> f32 {
    ((a.x - b.x).powi(2) + (a.z - b.z).powi(2)).sqrt()
}

/// The walk from `from` toward `to`, steered at it every tick: whether the feet come within reach of it, and where they ended.
fn walk(world: &Collision, from: Vec3, to: Vec3) -> (bool, Vec3) {
    let s = Settings::LIVE;
    let mut body = Controller::standing_at(from);
    let ticks = ((flat(from, to) / 3.0 + 2.0) * 2.0 / DT) as usize;
    let near = |feet: Vec3| flat(feet, to) <= NEAR_M && (feet.y - to.y).abs() <= NEAR_Y_M;
    for _ in 0..ticks {
        let feet = body.feet();
        if near(feet) {
            return (true, feet);
        }
        let d = flat(feet, to).max(1e-3);
        let step = (WALK_MPS * DT).min(d);
        body.walk = Vec3::new((to.x - feet.x) / d * step, 0.0, (to.z - feet.z) / d * step);
        body.step(world, DT, &s);
    }
    (near(body.feet()), body.feet())
}

/// The grid's floor nearest `p` (within FLOOR_M across, FLOOR_Y_M in height).
fn floor_at(grid: &WalkGrid, p: Vec3) -> Option<NodeId> {
    grid.floors_in([p.x - FLOOR_M, p.z - FLOOR_M, p.x + FLOOR_M, p.z + FLOOR_M])
        .filter(|&n| (grid.position(n).y - p.y).abs() <= FLOOR_Y_M)
        .min_by(|&a, &b| flat(grid.position(a), p).total_cmp(&flat(grid.position(b), p)))
}

/// The shortest grid way from `a`'s floor to `b`'s, metres (every link kind: the ground length plus the rise), if any within
/// `limit`.
fn way(grid: &WalkGrid, a: Vec3, b: Vec3, limit: f32) -> Option<f32> {
    let (start, goal) = (floor_at(grid, a)?, floor_at(grid, b)?);
    let mut dist: HashMap<NodeId, u32> = HashMap::from([(start, 0)]);
    let mut heap = BinaryHeap::from([(std::cmp::Reverse(0u32), start)]);
    while let Some((std::cmp::Reverse(d), n)) = heap.pop() {
        if n == goal {
            return Some(d as f32 / 1000.0);
        }
        if d > dist[&n] {
            continue;
        }
        let p = grid.position(n);
        for (m, _) in grid.links(n) {
            let q = grid.position(m);
            let nd = d + ((flat(p, q) + (q.y - p.y).abs()) * 1000.0) as u32;
            if nd as f32 <= limit * 1000.0 && dist.get(&m).is_none_or(|&o| nd < o) {
                dist.insert(m, nd);
                heap.push((std::cmp::Reverse(nd), m));
            }
        }
    }
    None
}

#[test]
fn the_nav_spots_are_walked_by_the_client_and_held_by_the_grid_as_their_rows_say() {
    let Some(dir) = std::env::var_os("SRP_MAPDATA_DIR") else {
        eprintln!("skipping: SRP_MAPDATA_DIR is not set");
        return;
    };
    let tuning = Tuning::default();
    let mut maps: HashMap<String, Option<(Collision, WalkGrid)>> = HashMap::new();
    let mut wrong = Vec::new();
    for row in rows() {
        let world = maps.entry(row.project.clone()).or_insert_with(|| {
            let path = std::path::Path::new(&dir).join(mapdata::format::file_name(&row.project));
            let map = MapData::read(&path).ok()?;
            let walker = map.walker_triangles();
            let spawns: Vec<Vec3> = map.spawns.iter().map(|s| s.position).collect();
            let grid = WalkGrid::build_kept_with_links(
                &walker,
                map.play_area(mapdata::AREA_MARGIN),
                mapdata::CELL,
                tuning.grid_body(),
                &map.grid_seeds(),
                &spawns,
                tuning.off_mesh_links(),
            )
            .0;
            Some((Collision::build(&walker), grid))
        });
        let Some((collision, grid)) = world else {
            eprintln!("skipping {}: no map file", row.project);
            continue;
        };
        let (reached, end) = walk(collision, row.from, row.to);
        let straight = flat(row.from, row.to);
        let found = way(grid, row.from, row.to, straight * WAY_FACTOR + WAY_PLUS_M);
        let line = format!(
            "{} ({:.2}, {:.2}, {:.2}) -> ({:.2}, {:.2}, {:.2}), {:.1} m: controller {} (want {}, ended at ({:.2}, {:.2}, {:.2})); grid {} (want {}); {}",
            row.project,
            row.from.x,
            row.from.y,
            row.from.z,
            row.to.x,
            row.to.y,
            row.to.z,
            straight,
            if reached { "walk" } else { "no" },
            if row.walks { "walk" } else { "no" },
            end.x,
            end.y,
            end.z,
            found.map_or("none".to_owned(), |m| format!("way {m:.1} m")),
            row.way.map_or("?", |w| if w { "way" } else { "none" }),
            row.spot
        );
        println!("{line}");
        if reached != row.walks || row.way.is_some_and(|w| w != found.is_some()) {
            wrong.push(line);
        }
    }
    assert!(wrong.is_empty(), "the nav spots moved:\n{}", wrong.join("\n"));
}
