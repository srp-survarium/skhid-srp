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
//! `SRP_A304_TICKS` (with `SRP_A304_ROWS` and `SRP_A304_REC`) [A325]: the reckoning tick by tick over chosen intervals, for the
//! first-divergence classifier against the reference body (the client's movement code on the same keys). The world is the live one:
//! built through the server's map store (which drops the map's meshes, as live); `SRP_A304_BUILT=<dir>` [A325] adds the store's built
//! files, so a run with the same grid rules reads the grid back instead of building it. `SRP_A304_WORLD=meshes` (or `SRP_A304_SLOPE`,
//! which needs the walker triangles) builds the old test world that keeps the meshes instead: not the live world.

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
    let loading = std::time::Instant::now();
    // `SRP_A304_BUILT=<dir>` [A325, the fast case bench]: the world through the server's own map store and its built files (grid,
    // sight tree, tactics keyed by the map file and the grid rules, `maps::read_or_build`): a run whose grid rules are the last
    // run's reads the grid back in seconds; only a grid-rule change builds it again (and writes it for the next run).
    // The world is the LIVE one by default: the map store builds it as the server does (and drops the map's meshes after the grid and
    // the sight tree, as live: a rule that read the walker triangles would find none here either), with or without built files. Only
    // `SRP_A304_WORLD=meshes` (and `SRP_A304_SLOPE`, which needs the walker triangles) builds the old test world that keeps them.
    let meshes = var("SRP_A304_WORLD").as_deref() == Some("meshes") || var("SRP_A304_SLOPE").is_some();
    let world = match (meshes, var("SRP_A304_BUILT")) {
        (false, built) => {
            let maps = super::super::maps::Maps::configured_for(
                Some(std::path::PathBuf::from(&dir)),
                1,
                1,
                built.map(std::path::PathBuf::from),
                tuning.grid.clone(),
                tuning.needs_wall_clearance(),
                tuning.needs_open_sides(),
            );
            loop {
                match maps.get(&project) {
                    super::super::maps::MapState::Ready(base) => break base.world.clone(),
                    super::super::maps::MapState::Absent(why) => panic!("{project}: {why}"),
                    _ => std::thread::sleep(std::time::Duration::from_millis(20)),
                }
            }
        }
        (true, _) => {
            let file = std::path::Path::new(&dir).join(mapdata::format::file_name(&project));
            let map = mapdata::MapData::read(&file).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
            Arc::new(World::build_with(
                map,
                mapdata::CELL,
                tuning.grid_body(),
                tuning.off_mesh_links(),
                tuning.needs_wall_clearance(),
                tuning.needs_open_sides(),
            ))
        }
    };
    eprintln!("A325 world of {project} in {:.1} s", loading.elapsed().as_secs_f32());
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
    // `SRP_A304_HOLD=1` [A318]: each 'faster' world the grid held the reckoning in (the client walked 1 m+ more than the keys meant):
    // the floor it was held on against the floor under the client's feet, and the grid's links between them.
    let hold_probe = var("SRP_A304_HOLD").is_some();
    let mut hold_kinds: HashMap<&str, u32> = HashMap::new();
    // `SRP_A304_TICKS=<csv>` [A325, the first-divergence classifier]: the reckoning tick by tick over each interval of the windows of
    // `SRP_A304_ROWS` (lines `rec,place,t0,t1[,note]`, the rows of this recording `SRP_A304_REC`; every interval without it): the state
    // after each decided tick, joined with the reference body's (the client's movement code on the same keys) by (place, world_t, tick).
    let windows: Option<Vec<(usize, u32, u32)>> = var("SRP_A304_ROWS").map(|path| {
        let rec = var("SRP_A304_REC").unwrap_or_default();
        std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{path}: {e}"))
            .lines()
            .filter_map(|line| {
                let f: Vec<&str> = line.split(',').map(str::trim).collect();
                (f.len() >= 4 && f[0] == rec).then_some(())?;
                Some((f[1].parse().ok()?, f[2].parse().ok()?, f[3].parse().ok()?))
            })
            .collect()
    });
    let ticks_out = var("SRP_A304_TICKS");
    // Columns: rec (`SRP_A304_REC`), the feet and the velocity over the tick, the grid node within 0.5 m (-1 none), its floor height
    // (empty for none) and whether the grid held the step. The intended velocity is the reference's (`ivx,ivz`: the same kin both sides).
    let tick_rec = var("SRP_A304_REC").unwrap_or_default();
    let mut tick_rows = String::from("rec,place,world_t,tick,t,x,y,z,vx,vy,vz,node,floor_y,held\n");
    for w in rows {
        while k < ticks.len() && ticks[k].0 <= w.time + WORLD_LATE_MS {
            let (time, place, keys, turn) = ticks[k];
            tracks.entry(place).or_default().decided(time, keys, turn, 0.0, &kin, grid);
            k += 1;
        }
        let track = tracks.entry(w.place).or_default();
        let before = track.fix().filter(|f| f.alive && f.inserted == w.fix.inserted).and_then(|f| f.feet);
        let reckoned = track.reckoned_at(w.time);
        // [A325] The interval's ticks, before this world puts the track right.
        if let (Some(_), Some(start), Some(was)) = (&ticks_out, before, track.fix().map(|f| f.time)) {
            let wanted = windows.as_ref().is_none_or(|ws| ws.iter().any(|&(p, t0, t1)| p == w.place && (t0..=t1).contains(&w.time)));
            if wanted {
                let mut last = (was, start);
                for (i, (t, feet, held)) in track.trail_between(was, w.time).into_iter().enumerate() {
                    let dt = (t.saturating_sub(last.0)).max(1) as f32 / 1000.0;
                    let v = (feet - last.1) * (1.0 / dt);
                    let located = world.grid.locate(feet, 0.5);
                    let node = located.map_or(-1, i64::from);
                    let floor_y = located.map_or(String::new(), |n| format!("{:.3}", world.grid.position(n).y));
                    tick_rows.push_str(&format!(
                        "{tick_rec},{},{was},{},{t},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{node},{floor_y},{}\n",
                        w.place,
                        i + 1,
                        feet.x,
                        feet.y,
                        feet.z,
                        v.x,
                        v.y,
                        v.z,
                        u8::from(held)
                    ));
                    last = (t, feet);
                }
            }
        }
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
        if hold_probe && iv.held_ticks > 0 && flat >= 1.0 && track.last_meant().is_some_and(|m| from.distance_xz(feet) - m >= 1.0) {
            let g = &*world.grid;
            let held = g.locate(reckoned, 1.0);
            let at = g.locate(feet, 0.5).filter(|&node| (g.position(node).y - feet.y).abs() < 0.5);
            let kind = match (held, at) {
                (None, _) => "the reckoning held off the grid",
                (_, None) => "no grid floor under the client's feet (the grid lacks the client's floor)",
                (Some(h), Some(a)) if h == a => "the client's floor is the held floor",
                (Some(h), Some(a)) => match (way_within(g, h, a, 12, true), way_within(g, h, a, 12, false)) {
                    (true, _) => "linked by walk links within 12 (the reckoning did not follow)",
                    (false, true) => "linked within 12 only through a drop or a jump",
                    (false, false) => "no way within 12 links (the grid lacks the link)",
                },
            };
            let seen = hold_kinds.entry(kind).or_insert(0);
            *seen += 1;
            if *seen <= 3 {
                eprintln!(
                    "A318 hold sample: {kind}: client ({:.2}, {:.2}, {:.2}), held ({:.2}, {:.2}, {:.2})",
                    feet.x, feet.y, feet.z, reckoned.x, reckoned.y, reckoned.z
                );
            }
        }
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
    if let Some(path) = &ticks_out {
        std::fs::write(path, tick_rows).unwrap_or_else(|e| panic!("{path}: {e}"));
    }
    eprintln!(
        "A304 replay {project}: {n} worlds of bots compared, {over} drifting (1 m+), {:.1} %",
        100.0 * over as f64 / n.max(1) as f64
    );
    let mut holds: Vec<_> = hold_kinds.into_iter().collect();
    holds.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    for (kind, count) in holds {
        eprintln!("A318 hold drift: {count} {kind}");
    }
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

/// [A318] Whether the walk grid has a way from `from` to `to` within `steps` links (only walk links with `walk_only`).
fn way_within(grid: &mapdata::WalkGrid, from: NodeId, to: NodeId, steps: usize, walk_only: bool) -> bool {
    let mut seen = std::collections::HashSet::from([from]);
    let mut front = vec![from];
    for _ in 0..steps {
        let mut next = Vec::new();
        for node in front {
            for (n, kind) in grid.links(node) {
                if walk_only && kind != mapdata::LinkKind::Walk {
                    continue;
                }
                if n == to {
                    return true;
                }
                if seen.insert(n) {
                    next.push(n);
                }
            }
        }
        front = next;
    }
    false
}
