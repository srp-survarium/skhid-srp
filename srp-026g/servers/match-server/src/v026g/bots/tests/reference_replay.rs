//! PROBE (A325, fx-smoke, local only): a REFERENCE body beside the server's reckoning on a real client match, and the first tick
//! where they part.
//!
//! The drift lab's driver (`drift_replay.rs`): the recording's inputs drive one `BodyTrack` a bot place, each world fixes it.
//! Per world t of a bot life after its first, from the world before (b, the client's feet there = the start of both):
//! - the RECKONING: the track's own ticks b -> t (feet, the ground velocity each tick moved by, held);
//! - the REFERENCE: the client's walk tick (`mapdata::controller::tick`, ported from the exe, Unicorn-checked) standing at the
//!   client's feet at b, stepped every tick with the reckoning's own velocity on the map's walker collision. Same intent, real
//!   collision instead of the grid.
//! So the drift (reckoning vs client at t) splits: reckoning vs reference = the GRID / TERRAIN part (same velocity); reference vs
//! client = the VELOCITY / KIN part (and any port error, which fx-grid's Unicorn checks decide).
//!
//! Per drifting world (1 m+ as the lab), the class:
//! - the reference ends within 0.5 m of the client: the first tick the reckoning is 0.25 m+ from the reference names the terrain
//!   rule: `grid held` (the grid stopped the reckoning, the reference moved), `collision the grid lacks` (the reference stopped,
//!   the reckoning went on), `climb missed` (the reference rose, the reckoning did not), `fall` / `step down` (the reference fell
//!   or stepped down, the reckoning stayed up), `steep ground` (the reference on a contact steeper than the walk limit), `floor
//!   missing` (no grid floor at the reference's feet), else `path`;
//! - the reference also ends 0.5 m+ off: `kin speed` (the client went farther or shorter along the same way) or `kin turn` (the
//!   client's way 20 deg+ from the reference's), unless a jump key was pressed (`jump`, the air branch is not referenced here).
//! `SRP_A304_WORLDS`, `SRP_A304_INPUTS`, `SRP_A304_MAP`, `SRP_MAPDATA_DIR`, `SRP_BENCH_TUNING` as the lab; `SRP_A325_OUT` the rows.

use std::collections::HashMap;
use std::sync::Arc;

use mapdata::controller::Collision;
use mapdata::controller::tick::{Controller, Settings};
use mapdata::{Vec3, World};

use super::super::body::{BodyTrack, Fix};
use super::bench::bench_tuning;
use crate::v026g::drive::key;

const WORLD_LATE_MS: u32 = 300;
const TICK_S: f32 = 0.05;

fn read_csv(path: &str) -> Vec<HashMap<String, String>> {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let mut lines = text.lines();
    let head: Vec<String> = lines.next().expect("a header").split(',').map(str::to_owned).collect();
    lines.map(|line| head.iter().cloned().zip(line.split(',').map(str::to_owned)).collect()).collect()
}

fn hex(s: &str) -> u32 {
    u32::from_str_radix(s.trim_start_matches("0x"), 16).unwrap_or(0)
}

#[test]
#[ignore = "A325: needs a recording's tables and the map data"]
fn reference_replay() {
    let var = |name: &str| std::env::var(name).ok().filter(|v| !v.is_empty());
    let (Some(worlds), Some(inputs), Some(project), Some(dir)) =
        (var("SRP_A304_WORLDS"), var("SRP_A304_INPUTS"), var("SRP_A304_MAP"), var("SRP_MAPDATA_DIR"))
    else {
        eprintln!("skipping: SRP_A304_WORLDS, SRP_A304_INPUTS, SRP_A304_MAP and SRP_MAPDATA_DIR are needed");
        return;
    };
    let tuning = bench_tuning();
    let kin = tuning.kin.clone();
    let file = std::path::Path::new(&dir).join(mapdata::format::file_name(&project));
    let map = mapdata::MapData::read(&file).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
    let collision = Collision::build(&map.walker_triangles());
    let world = Arc::new(World::build_with(
        map,
        mapdata::CELL,
        tuning.grid_body(),
        tuning.off_mesh_links(),
        tuning.needs_wall_clearance(),
        tuning.needs_open_sides(),
    ));
    let grid = &*world.grid;
    let s = Settings::LIVE;
    let mut ticks: Vec<(u32, usize, u32, (f32, f32))> = read_csv(&inputs)
        .iter()
        .filter_map(|r| {
            let place: usize = r.get("place")?.parse().ok()?;
            (place != 0).then_some(())?;
            Some((r.get("time")?.parse().ok()?, place, hex(r.get("actions")?), (r.get("rot_a")?.parse().ok()?, r.get("rot_b")?.parse().ok()?)))
        })
        .collect();
    ticks.sort_by_key(|t| (t.0, t.1));
    let mut keys_of: HashMap<usize, Vec<(u32, u32)>> = HashMap::new();
    for &(t, p, k, _) in &ticks {
        keys_of.entry(p).or_default().push((t, k));
    }
    struct Row {
        time: u32,
        place: usize,
        fix: Fix,
    }
    let mut rows: Vec<Row> = read_csv(&worlds)
        .iter()
        .filter_map(|r| {
            (r.get("upload")? == "0").then_some(())?;
            let place: usize = r.get("place")?.parse().ok()?;
            (place != 0).then_some(())?;
            let num = |k: &str| r.get(k).and_then(|v| v.parse::<f32>().ok()).filter(|v| v.is_finite());
            let alive = r.get("alive")? == "1";
            let feet = alive.then(|| Some(Vec3::new(num("fx")?, num("fy")?, num("fz")?))).flatten();
            Some(Row {
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
    // `SRP_A325_WINDOWS`: fx-bench's rows file `rec,place,t0,t1[,note]`: only the worlds of those places in [t0, t1].
    let rec_id = var("SRP_A325_REC").unwrap_or_default();
    let windows: Option<Vec<(usize, u32, u32)>> = var("SRP_A325_WINDOWS").map(|path| {
        std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{path}: {e}"))
            .lines()
            .filter_map(|l| {
                let f: Vec<&str> = l.split(',').collect();
                (f.len() >= 4 && f[0] == rec_id).then_some(())?;
                Some((f[1].parse().ok()?, f[2].parse().ok()?, f[3].parse().ok()?))
            })
            .collect()
    });
    let wanted = |place: usize, t: u32| windows.as_ref().is_none_or(|w| w.iter().any(|&(p, a, b)| p == place && a <= t && t <= b));
    let mut tracks: HashMap<usize, BodyTrack> = HashMap::new();
    let mut out = String::from("rec,place,world_t,tick,t,x,y,z,vx,vy,vz,ground,normal_y,branch,keys\n");
    let mut checks = String::new();
    let (mut k, mut n, mut close, mut far) = (0usize, 0u32, 0u32, 0u32);
    for w in rows {
        while k < ticks.len() && ticks[k].0 <= w.time + WORLD_LATE_MS {
            let (time, place, keys, turn) = ticks[k];
            tracks.entry(place).or_default().decided(time, keys, turn, 0.0, &kin, Some(grid));
            k += 1;
        }
        let track = tracks.entry(w.place).or_default();
        let before = track.fix().filter(|f| f.alive && f.inserted == w.fix.inserted).map(|f| (f.time, f.feet));
        let trail = track.trail_ticks();
        let reckoned = track.reckoned_at(w.time);
        let feet = w.fix.feet;
        let new_life = track.fixed(w.fix, &kin, Some(grid));
        let (Some(feet), Some((t0, Some(from))), false) = (feet, before, new_life) else {
            continue;
        };
        if !wanted(w.place, w.time) {
            continue;
        }
        n += 1;
        let keys_in = |t: u32| keys_of.get(&w.place).and_then(|v| v.iter().rev().find(|&&(tt, _)| tt <= t).map(|&(_, k)| k)).unwrap_or(0);
        let jumped = keys_of.get(&w.place).is_some_and(|v| v.iter().any(|&(t, k)| t > t0 && t <= w.time && k & key::JUMP != 0));
        let mut body = Controller::standing_at(from);
        let mut last_t = t0;
        let mut tick_n = 0u32;
        for &(t, _, v, _) in trail.iter().filter(|r| r.0 > t0 && r.0 <= w.time) {
            let sub = (((t - last_t) as f32 / 1000.0) / TICK_S).round().max(1.0) as usize;
            for _ in 0..sub {
                tick_n += 1;
                let kk = keys_in(t0 + 50 * tick_n);
                body.want_crouch = kk & key::CROUCH != 0;
                let was = body.feet();
                body.walk = Vec3::new(v[0] * TICK_S, 0.0, v[1] * TICK_S);
                let notes = body.step(&collision, TICK_S, &s);
                let now = body.feet();
                let moved = was.distance_xz(now);
                let asked = v[0].hypot(v[1]) * TICK_S;
                let ground = body.on_ground(&s);
                let branch = if notes.airborne || (!ground && body.velocity.y < -0.5) {
                    "air"
                } else if notes.landed {
                    "land"
                } else if notes.side_slide || notes.down_slide {
                    "slide"
                } else if asked > 1e-3 && moved < 0.1 * asked {
                    "blocked"
                } else if now.y - was.y > 0.05 {
                    "step_up"
                } else if was.y - now.y > 0.05 {
                    "step_down"
                } else {
                    "walk"
                };
                let normal = if notes.airborne { String::new() } else { format!("{:.3}", body.contact_normal.y) };
                out.push_str(&format!(
                    "{rec_id},{},{t0},{tick_n},{},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{},{normal},{branch},0x{kk:x}\n",
                    w.place,
                    t0 + 50 * tick_n,
                    now.x,
                    now.y,
                    now.z,
                    (now.x - was.x) / TICK_S,
                    body.velocity.y,
                    (now.z - was.z) / TICK_S,
                    u8::from(ground)
                ));
            }
            last_t = t;
        }
        let refe = body.feet();
        let dist = (refe - feet).length();
        let (a, b) = (Vec3::new(refe.x - from.x, 0.0, refe.z - from.z), Vec3::new(feet.x - from.x, 0.0, feet.z - from.z));
        let (la, lb) = (a.length(), b.length());
        let ang = if la > 0.2 && lb > 0.2 { (a.dot(b) / (la * lb)).clamp(-1.0, 1.0).acos().to_degrees() } else { 0.0 };
        close += u32::from(dist < 0.25);
        far += u32::from(dist >= 0.5);
        checks.push_str(&format!(
            "CHECK,{rec_id},{},{t0},{dist:.3},{:.3},{ang:.1},{},{:.3},{}\n",
            w.place,
            la - lb,
            if jumped { "jump" } else { "" },
            reckoned.map_or(f32::NAN, |r| (r - feet).length()),
            w.time
        ));
    }
    if let Some(path) = var("SRP_A325_OUT") {
        std::fs::write(&path, out).unwrap();
        std::fs::write(format!("{path}.check"), &checks).unwrap();
    }
    eprintln!(
        "A325 {project}: {n} intervals, reference within 0.25 m of the client {close} ({:.1} %), 0.5 m+ off {far} ({:.1} %)",
        100.0 * close as f64 / n.max(1) as f64,
        100.0 * far as f64 / n.max(1) as f64
    );
}
