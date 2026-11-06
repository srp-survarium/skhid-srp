//! A304: the server's own reckoning replayed on a real client match, to measure and test position drift against the client itself
//! (the bench's stand-in body is not the client).
//!
//! A recording's tables (`srp-rec-table`: `rec_worlds.csv`, every uploaded world's player blocks with feet, yaw, keys and speed;
//! `rec_inputs.csv`, the inputs relayed for every place, the server's bots among them) drive one [`BodyTrack`] a bot place, as
//! `Bots` does: each input is the bot's decided tick ([`BodyTrack::decided`]), each world a fix ([`BodyTrack::fixed`]) taken once the
//! ticks up to [`WORLD_LATE_MS`] past it are in (the bots get a world that late). Before each fix the reckoning's feet at its tick
//! ([`BodyTrack::reckoned_at`]) are compared with the world's: the drift the server logs as `nav: drift` (A294), here for every world.
//!
//! `SRP_A304_WORLDS`, `SRP_A304_INPUTS`: the tables; `SRP_A304_MAP`: the project (`level_08/day_2`); `SRP_MAPDATA_DIR`: the map
//! data; `SRP_BENCH_TUNING`: the tuning patch (the server's file); `SRP_A304_HUMANS`: the human places (default `0`);
//! `SRP_A304_OUT`: the CSV written, one row a world of a bot life after its first. Ignored; it prints a summary.

use std::collections::HashMap;
use std::sync::Arc;

use mapdata::{NodeId, Vec3, World};

use super::super::body::{BodyTrack, Fix};
use super::bench::bench_tuning;

/// A world reaches the bots this long after its tick, ms (the bench's own delay; the live one is 200-400).
const WORLD_LATE_MS: u32 = 300;

struct WorldRow {
    time: u32,
    place: usize,
    fix: Fix,
}

fn read_csv(path: &str) -> Vec<HashMap<String, String>> {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let mut lines = text.lines();
    let head: Vec<String> = lines.next().expect("a header").split(',').map(str::to_owned).collect();
    lines
        .map(|line| head.iter().cloned().zip(line.split(',').map(str::to_owned)).collect())
        .collect()
}

fn hex(s: &str) -> u32 {
    u32::from_str_radix(s.trim_start_matches("0x"), 16).unwrap_or(0)
}

#[test]
#[ignore = "A304: needs a recording's tables and the map data (SRP_A304_*, SRP_MAPDATA_DIR)"]
fn drift_on_a_real_match() {
    let var = |name: &str| std::env::var(name).ok().filter(|v| !v.is_empty());
    let (Some(worlds), Some(inputs), Some(project), Some(dir)) =
        (var("SRP_A304_WORLDS"), var("SRP_A304_INPUTS"), var("SRP_A304_MAP"), var("SRP_MAPDATA_DIR"))
    else {
        eprintln!("skipping: SRP_A304_WORLDS, SRP_A304_INPUTS, SRP_A304_MAP and SRP_MAPDATA_DIR are needed");
        return;
    };
    let humans: Vec<usize> = var("SRP_A304_HUMANS").unwrap_or_else(|| "0".into()).split(',').filter_map(|p| p.parse().ok()).collect();
    let tuning = bench_tuning();
    let kin = tuning.kin.clone();
    let file = std::path::Path::new(&dir).join(mapdata::format::file_name(&project));
    let map = mapdata::MapData::read(&file).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
    let world = Arc::new(World::build_with(
        map,
        mapdata::CELL,
        tuning.grid_body(),
        tuning.off_mesh_links(),
        tuning.needs_wall_clearance(),
        tuning.needs_open_sides(),
    ));
    // `SRP_A304_RAMP_AT=x,z;x,z`: the ramp marks (`grid.ramp_faces`) of the floors within 2 m of each point, then nothing else.
    if let Some(points) = var("SRP_A304_RAMP_AT") {
        let g = &*world.grid;
        for point in points.split(';') {
            let Some((x, z)) = point.split_once(',').and_then(|(x, z)| Some((x.parse::<f32>().ok()?, z.parse::<f32>().ok()?))) else {
                continue;
            };
            let near: Vec<(f32, Option<f32>)> = (0..g.len() as NodeId)
                .map(|node| (g.position(node), node))
                .filter(|(p, _)| (p.x - x).hypot(p.z - z) <= 2.0)
                .map(|(p, node)| (p.y, g.ramp_slope_deg(node)))
                .collect();
            let marked: Vec<f32> = near.iter().filter_map(|&(_, d)| d).collect();
            let (low, high) = near.iter().fold((f32::MAX, f32::MIN), |(lo, hi), &(y, _)| (lo.min(y), hi.max(y)));
            eprintln!(
                "A304 ramp at ({x}, {z}) {project}: {} floors within 2 m (y {low:.2}-{high:.2}), {} marked {marked:?}",
                near.len(),
                marked.len()
            );
        }
        return;
    }
    // `SRP_A304_FREE=1`: the reckoning with no walk grid (free space), to weigh what the grid adds.
    let grid: Option<&mapdata::WalkGrid> = var("SRP_A304_FREE").is_none().then_some(&*world.grid);
    // The inputs, by time; the worlds, by the time they reach the bots.
    let mut ticks: Vec<(u32, usize, u32, (f32, f32))> = read_csv(&inputs)
        .iter()
        .filter_map(|r| {
            let place: usize = r.get("place")?.parse().ok()?;
            (!humans.contains(&place)).then_some(())?;
            Some((r.get("time")?.parse().ok()?, place, hex(r.get("actions")?), (r.get("rot_a")?.parse().ok()?, r.get("rot_b")?.parse().ok()?)))
        })
        .collect();
    ticks.sort_by_key(|t| (t.0, t.1));
    let mut rows: Vec<WorldRow> = read_csv(&worlds)
        .iter()
        .filter_map(|r| {
            (r.get("upload")? == "0").then_some(())?;
            let place: usize = r.get("place")?.parse().ok()?;
            (!humans.contains(&place)).then_some(())?;
            let num = |k: &str| r.get(k).and_then(|v| v.parse::<f32>().ok()).filter(|v| v.is_finite());
            let alive = r.get("alive")? == "1";
            let feet = (alive).then(|| Some(Vec3::new(num("fx")?, num("fy")?, num("fz")?))).flatten();
            Some(WorldRow {
                time: r.get("time")?.parse().ok()?,
                place,
                fix: Fix {
                    time: r.get("time")?.parse().ok()?,
                    alive,
                    inserted: r.get("inserted")?.parse().ok()?,
                    feet,
                    yaw: num("yaw").unwrap_or(0.0),
                    pitch: num("pitch").unwrap_or(0.0),
                    keys: hex(r.get("keys")?),
                    speed: num("speed"),
                },
            })
        })
        .collect();
    rows.sort_by_key(|w| (w.time, w.place));
    let mut tracks: HashMap<usize, BodyTrack> = HashMap::new();
    let mut out = String::from("place,time,gap,flat,height,meant,moved,ticks,walk,held,rx,ry,rz,wx,wy,wz\n");
    let (mut k, mut n, mut over) = (0usize, 0u32, 0u32);
    let mut held_kinds: HashMap<(&str, &str), u32> = HashMap::new();
    // `SRP_A304_SLOPE=1` [A318]: each drifting world's ground under the client's feet, at it and at the world before, by the client's
    // slide rule (`standin_slide::Ground::slides`): what of the drift a slide in the reckoning could take.
    let grounds = var("SRP_A304_SLOPE").map(|_| {
        let triangles = world.map.walker_triangles();
        let all = Arc::new(mapdata::RayTree::from_corners(triangles.iter().map(|t| t.v)));
        super::standin_slide::Grounds::new(&triangles, all)
    });
    let mut slope_kinds: HashMap<&str, u32> = HashMap::new();
    for w in rows {
        while k < ticks.len() && ticks[k].0 <= w.time + WORLD_LATE_MS {
            let (time, place, keys, turn) = ticks[k];
            tracks.entry(place).or_default().decided(time, keys, turn, 0.0, &kin, grid);
            k += 1;
        }
        let track = tracks.entry(w.place).or_default();
        let before = track.fix().filter(|f| f.alive && f.inserted == w.fix.inserted).and_then(|f| f.feet);
        let reckoned = track.reckoned_at(w.time);
        let feet = w.fix.feet;
        let speed = w.fix.speed;
        let new_life = track.fixed(w.fix, &kin, grid);
        let (Some(feet), Some(reckoned), Some(from), false) = (feet, reckoned, before, new_life) else {
            continue;
        };
        let flat = reckoned.distance_xz(feet);
        let height = reckoned.y - feet.y;
        n += 1;
        over += u32::from(flat >= 1.0 || height.abs() >= 1.0);
        if let (Some(g), true) = (&grounds, flat >= 1.0 || height.abs() >= 1.0) {
            use super::standin_slide::Ground;
            let still = speed.is_some_and(|s| s < 0.1);
            let (at, was) = (g.under(feet), g.under(from));
            let kind = if [at, was].iter().flatten().any(|ground| ground.slides(false)) {
                "the client's ground slides a body moving or not (by_slope over 55 deg, or dont_walk)"
            } else if [at, was].iter().flatten().any(|ground| ground.slides(true)) {
                if still { "still on by_slope ground of 45-55 deg (slides a body standing)" } else { "moving on by_slope ground of 45-55 deg (holds a moving body)" }
            } else if [at, was].iter().flatten().any(|ground| matches!(ground, Ground::Walk(deg) | Ground::Slope(deg) if *deg >= 30.0)) {
                "ground of 30 deg or more the client holds (walk face, or by_slope under 45)"
            } else if at.is_none() {
                "no face under the client's feet"
            } else {
                "ground under 30 deg"
            };
            // Whether the walk grid has a floor where the client stands (a floor the client cannot hold is a grid matter).
            let floored = world.grid.locate(feet, 0.5).is_some_and(|node| (world.grid.position(node).y - feet.y).abs() < 0.5);
            let kind: &str = match (kind.starts_with("the client's ground slides") || kind.starts_with("still on"), floored) {
                (true, true) => if kind.starts_with("still") { "still slide, the grid has a floor there" } else { "slide, the grid has a floor there" },
                (true, false) => if kind.starts_with("still") { "still slide, no grid floor there" } else { "slide, no grid floor there" },
                _ => kind,
            };
            let seen = slope_kinds.entry(kind).or_insert(0);
            *seen += 1;
            if kind.ends_with("the grid has a floor there") && *seen <= 4 {
                eprintln!("A318 slope sample: {kind} at ({:.2}, {:.2}, {:.2}) ground {at:?} before {was:?}", feet.x, feet.y, feet.z);
            }
        }
        let iv = track.last_interval().unwrap_or_default();
        // What the grid has where a held reckoning stopped and where the client's feet are (A304: what kind of hold).
        if iv.held_ticks > 0 && flat >= 1.0 {
            let g = &*world.grid;
            let floor = |p: Vec3| g.locate(p, 2.0).map(|node| (node, g.position(node).distance_xz(p), (g.position(node).y - p.y).abs()));
            let at_world = match floor(feet) {
                None => "no floor within 2 m of the client's feet",
                Some((_, _, dy)) if dy > 0.6 => "the nearest floor to the client's feet is another level",
                Some((_, d, _)) if d > 0.4 => "the client's feet 0.4-2 m off any floor (eroded or missing)",
                Some(_) => "a floor under the client's feet",
            };
            let at_held = match floor(reckoned) {
                Some((node, _, _)) if g.is_lip(node) || g.near_lip(node) => "held at a lip",
                Some((node, _, _)) if g.near_fall(node) => "held by a fall's edge",
                Some(_) => "held on a plain floor (a wall or an edge)",
                None => "held off the grid",
            };
            *held_kinds.entry((at_held, at_world)).or_insert(0u32) += 1;
        }
        out.push_str(&format!(
            "{},{},{},{flat:.3},{height:.3},{:.3},{:.3},{},{},{},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2}\n",
            w.place,
            w.time,
            track.gap_of(w.time).unwrap_or(0),
            track.last_meant().unwrap_or(f32::NAN),
            from.distance_xz(feet),
            iv.ticks,
            iv.walk_ticks,
            iv.held_ticks,
            reckoned.x,
            reckoned.y,
            reckoned.z,
            feet.x,
            feet.y,
            feet.z,
        ));
    }
    if let Some(path) = var("SRP_A304_OUT") {
        std::fs::write(&path, out).unwrap_or_else(|e| panic!("{path}: {e}"));
    }
    eprintln!(
        "A304 replay {project}: {n} worlds of bots compared, {over} drifting (1 m+), {:.1} %",
        100.0 * over as f64 / n.max(1) as f64
    );
    let mut slopes: Vec<_> = slope_kinds.into_iter().collect();
    slopes.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    for (kind, count) in slopes {
        eprintln!("A318 slope drift: {count} {kind}");
    }
    let mut kinds: Vec<_> = held_kinds.into_iter().collect();
    kinds.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    for ((held, world), count) in kinds {
        eprintln!("A304 held drift: {count} {held}; {world}");
    }
}
