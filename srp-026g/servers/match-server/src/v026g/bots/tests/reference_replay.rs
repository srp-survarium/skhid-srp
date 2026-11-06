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
use mapdata::controller::speed::{self, Gait, Heading, Stance};
use mapdata::controller::tick::{Controller, Settings};
use mapdata::{Vec3, World};

use super::super::body::{BodyTrack, Fix};
use super::super::jump_arc::{Clip, END_FRAME, FPS as JUMP_FPS, PREPARE_MS, TAKEOFF_FRAME};
use super::super::kin::Local;
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
    // `SRP_A325_EXE_COLLISION=1`: the exe's collision rules (#1387, `mapdata::Body::controller` with exe_collision): its Bullet
    // caster 0xB9320 and the wall normal by rays 0x89B80 (fx-grid: the tail stalls p4/p7/p9 then match the exe within 1 mm).
    let exe_collision = var("SRP_A325_EXE_COLLISION").is_some();
    let (caster, live) = mapdata::Body { exe_collision, ..mapdata::Body::default() }.controller();
    let collision = Collision::build(&map.walker_triangles()).with_caster(caster);
    let world = Arc::new(World::build_with(
        map,
        mapdata::CELL,
        tuning.grid_body(),
        tuning.off_mesh_links(),
        tuning.needs_wall_clearance(),
        tuning.needs_open_sides(),
    ));
    let grid = &*world.grid;
    let s: Settings = live;
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
    // `SRP_A325_TURN_FIRST=1`: each tick's velocity turned by that tick's own turn first (the reckoning moves, then turns).
    let turn_first = var("SRP_A325_TURN_FIRST").is_some();
    let exe_order = var("SRP_A325_EXE_ORDER").is_some();
    let pos_lag1 = var("SRP_A325_POS_LAG1").is_some();
    let keys_mode = var("SRP_A325_KEYS").is_some();
    let level_from_world = var("SRP_A325_LEVEL_FROM_WORLD").is_some();
    // `SRP_A325_REPLAY_START=1`: the keys mode starts from the clip stack the keys before B leave (else one layer at full weight).
    let replay_start = var("SRP_A325_REPLAY_START").is_some();
    let yaw_before = var("SRP_A325_YAW_BEFORE").is_some();
    let speed_cap = var("SRP_A325_CAP").is_some();
    let mut last_level: HashMap<usize, Vec<f32>> = HashMap::new();
    // `SRP_A325_LEVEL_MEDIAN=1`: the level is the median of the player's last 5 steady-world reads, not the last one: a world
    // whose body was held by a wall or a player reads low (one read of 0.756 left a body walking 2.65 m/s for a whole life).
    let level_median = var("SRP_A325_LEVEL_MEDIAN").is_some();
    let level_now = |seen: &Vec<f32>, median: bool| -> f32 {
        if !median {
            return seen.last().copied().unwrap_or(1.0);
        }
        let mut last: Vec<f32> = seen.iter().rev().take(5).copied().collect();
        last.sort_by(f32::total_cmp);
        last.get(last.len() / 2).copied().unwrap_or(1.0)
    };
    // `SRP_A325_WORLD_BLEND=k`: the velocity kept in the world frame, each tick k of the way toward the intended (a turn does not
    // swing the momentum round with the yaw at once). A hypothesis test, not a client rule.
    let world_blend: Option<f32> = var("SRP_A325_WORLD_BLEND").and_then(|v| v.parse().ok());
    // `SRP_A325_JUMP=1` (keys mode, fx-team): the client's jump played through the controller's air branch (see `RefJump`).
    let jump_mode = keys_mode && var("SRP_A325_JUMP").is_some();
    // `SRP_A325_ROWS_NEAR_JUMP=1`: the per-tick rows only of intervals within 2.5 s after a press (the checks stay whole): a
    // small output for the jump rows.
    let rows_near_jump = var("SRP_A325_ROWS_NEAR_JUMP").is_some();
    let mut turn_of: HashMap<(usize, u32), f32> = HashMap::new();
    for &(t, p, k, turn) in &ticks {
        keys_of.entry(p).or_default().push((t, k));
        turn_of.insert((p, t), turn.0);
    }
    // A press: an input with JUMP whose place's input before had none (`base_player::jump` 0x2D7920 runs on the key's press).
    let presses_of: HashMap<usize, Vec<u32>> = keys_of
        .iter()
        .map(|(&p, v)| {
            let mut prev = 0u32;
            let mut out = Vec::new();
            for &(t, k) in v {
                if k & key::JUMP != 0 && prev & key::JUMP == 0 {
                    out.push(t);
                }
                prev = k;
            }
            (p, out)
        })
        .collect();
    let rotate = |v: [f32; 2], d: f32| {
        let ([a, b], [c, e]) = (super::super::kin::forward(0.0), super::super::kin::forward(d));
        let (cos, sin) = (a * c + b * e, a * e - b * c);
        [v[0] * cos - v[1] * sin, v[0] * sin + v[1] * cos]
    };
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
    let mut out = String::from("rec,place,world_t,tick,t,x,y,z,vx,vy,vz,ground,normal_y,branch,keys,ivx,ivz,turn,yaw\n");
    let mut checks = String::new();
    let (mut k, mut n, mut close, mut far) = (0usize, 0u32, 0u32, 0u32);
    for w in rows {
        while k < ticks.len() && ticks[k].0 <= w.time + WORLD_LATE_MS {
            let (time, place, keys, turn) = ticks[k];
            tracks.entry(place).or_default().decided(time, keys, turn, 0.0, &kin, Some(grid));
            k += 1;
        }
        let track = tracks.entry(w.place).or_default();
        let before = track.fix().filter(|f| f.alive && f.inserted == w.fix.inserted).map(|f| (f.time, f.feet, f.yaw, f.speed, f.keys));
        let trail = track.trail_ticks();
        let reckoned = track.reckoned_at(w.time);
        let feet = w.fix.feet;
        let new_life = track.fixed(w.fix, &kin, Some(grid));
        let (Some(feet), Some((t0, Some(from), yaw_b, speed_b, keys_b)), false) = (feet, before, new_life) else {
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
        let mut wv: Option<[f32; 2]> = None;
        // The yaw after each tick: the world's at b plus the inputs' turns (fx-zones: exact for bots), not wrapped.
        let mut yaw = yaw_b;
        // `land` only on the tick after an airborne one (the tick's `landed` note is set on most ground ticks).
        let mut was_air = false;
        // `SRP_A325_EXE_ORDER=1`: the exe's order inside a tick (fx-grid, tick list 2 at 0x2DA9A0, PROVEN): the tick's input
        // (keys, turn) is applied, the animations advance in the new yaw's frame, then the walk is set. So the move ending at tick
        // t is moved by the state AFTER tick t's input, which is the reckoning's next step (it moves t -> t + 50 with tick t's keys
        // and yaw); the trail keeps the ticks past the world (decided up to WORLD_LATE_MS), so the next one is there.
        let ahead: Vec<&(u32, Vec3, [f32; 2], bool)> = trail.iter().filter(|r| r.0 > t0).collect();
        // `SRP_A325_POS_LAG1=1`: the world's position one tick behind its yaw/keys (hypothesis: the world block written after
        // the tick's input, before physics moves the body): the move ending at t uses the previous step's velocity.
        let before_first = trail.iter().rev().find(|r| r.0 <= t0).map(|r| r.2);
        // `SRP_A325_KEYS=1`: the velocity from the recorded keys by the client's own rules, not the reckoning (see `key_clip`):
        // the exe's tick k (fx-grid, 0x2DA9A0) applies input k (yaw += its turn; a key change starts a new clip at weight 0),
        // then walks by R(yaw) x the clips' weighted root velocities, the newest ramping 1/6 a tick (0.3 s, 0x921804), Σ w_i v_i
        // without normalisation, each older clip keeping what the newer leave. World B's position is the body after tick B-1
        // (m_last_pos, 0x7BE7C) and its yaw is after input B, so the ticks run k = t0 .. t - 50 with input k, turn from t0 + 50.
        // The client is mid-blend at most worlds: its clip stack at B is the keys of the ticks before B played through the same
        // ramp from the keys 8 ticks before (an older clip is hidden by then, so the stack is exact).
        let k0 = t0.saturating_sub(50 * 8);
        let mut layers: Vec<(Local, u32, u32)> = vec![(key_clip(keys_in(k0) & CLIP_KEYS), 0, keys_in(k0) & CLIP_KEYS)];
        if replay_start {
            for k in (k0 + 50..t0).step_by(50) {
                let kk = keys_in(k) & CLIP_KEYS;
                if layers.last().is_some_and(|l| l.2 != kk) {
                    layers.push((key_clip(kk), k, kk));
                }
            }
        } else {
            let kb = keys_in(t0.saturating_sub(50)) & CLIP_KEYS;
            layers = vec![(key_clip(kb), 0, kb)];
        }
        // `SRP_A325_LEVEL_FROM_WORLD=1`: the speed factor's modifiers (skill, weapon, kit) measured from the client's own state:
        // world B's recorded speed (m_linear_horizontal_speed) over its clip's speed, when B's keys equal the input before
        // (a steady walk); else 1. A measurement of the client at B, not a model rule (the factor's own read: LIKELY, open).
        let level = if level_from_world {
            let clip = key_clip(keys_b & CLIP_KEYS);
            let clip_speed = clip[0].hypot(clip[1]);
            match speed_b {
                Some(v) if clip_speed > 0.5 && v > 0.5 && (k0 + 50..=t0).step_by(50).all(|k| keys_in(k) & CLIP_KEYS == keys_b & CLIP_KEYS) => {
                    let seen = last_level.entry(w.place).or_default();
                    seen.push((v / clip_speed).clamp(0.7, 1.3));
                    level_now(seen, level_median)
                }
                // Not steady at B: the player's last measured level (the modifiers change only with the kit).
                _ => last_level.get(&w.place).map_or(1.0, |seen| level_now(seen, level_median)),
            }
        } else {
            1.0
        };
        // `SRP_A325_YAW_BEFORE=1`: the walk of tick k turned by the yaw BEFORE input k's turn (the model's base transform from the
        // tick before), else after it (fx-grid's reading of 0x2D9060). The A/B of the two readings, until the exe read settles it.
        let turn_at = |k: u32| turn_of.get(&(w.place, k)).copied().unwrap_or(0.0);
        let mut kyaw = if yaw_before { yaw_b - turn_at(t0) } else { yaw_b };
        // The jump: one pressed before b and not over by b is under way at b (the client in the air there), its target from b's
        // feet on; a press at an input k of the interval starts one when the body may jump.
        let presses: &[u32] = presses_of.get(&w.place).map_or(&[], Vec::as_slice);
        let walk_speed = |k: u32| {
            let c = key_clip(keys_in(k) & CLIP_KEYS);
            c[0].hypot(c[1]) * level
        };
        let mut jump: Option<RefJump> = None;
        let mut iv = String::new();
        if jump_mode && let Some(&p) = presses.iter().rev().find(|&&p| p < t0) {
            let mut j = RefJump::new(p, keys_in(p), walk_speed(p.saturating_sub(50)), from);
            if j.frame(t0) < END_FRAME {
                if j.frame(t0) >= TAKEOFF_FRAME {
                    j.phase = RefPhase::Air;
                    body.jumping = true;
                }
                jump = Some(j);
            }
        }
        for (i, &&(t, _, v_own, _)) in ahead.iter().enumerate().take_while(|(_, r)| r.0 <= w.time) {
            let v_keys = keys_mode.then(|| {
                let k = t - 50;
                if yaw_before {
                    if k > t0 {
                        kyaw += turn_at(k - 50);
                    }
                } else if k > t0 {
                    kyaw += turn_at(k);
                }
                let kk = keys_in(k) & CLIP_KEYS;
                if layers.last().is_none_or(|l| l.2 != kk) {
                    layers.push((key_clip(kk), k, kk));
                }
                let mut sum = [0.0f32; 2];
                let mut left = 1.0f32;
                let mut keep = layers.len();
                for (j, &(clip, start, _)) in layers.iter().enumerate().rev() {
                    let w = if start == 0 { 1.0 } else { (((k - start) / 50) as f32 / 6.0).min(1.0) };
                    sum[0] += clip[0] * w * left;
                    sum[1] += clip[1] * w * left;
                    left *= 1.0 - w;
                    if left <= 0.0 {
                        keep = layers.len() - j;
                        break;
                    }
                }
                let drop = layers.len() - keep;
                layers.drain(..drop);
                let mut local = [sum[0] * level, sum[1] * level];
                // `SRP_A325_CAP=1`: `set_physics_controller_walk_vector` 0x2D9060 caps the flat step at `max_speed` 0x2DC010:
                // the sprint's forward speed 5.0 x F while the sprint state is on (its initialize 0x336A10 sets the controller's
                // +0x204, finalize 0x336A50 clears it), else the walk's 3.5 x F (mapdata::controller::speed::cap). So a released
                // sprint drops to the walk cap on its own tick while the clip blend still carries the sprint's speed.
                if speed_cap {
                    let gait = if sprint_input(keys_in(k)) && keys_in(k) & key::AIM == 0 { Gait::Sprint } else { Gait::Move { stance: Stance::Stand, heading: Heading::Forward, aiming: false } };
                    let cap = speed::cap(gait, level);
                    let len = local[0].hypot(local[1]);
                    if len > cap {
                        local = [local[0] * cap / len, local[1] * cap / len];
                    }
                }
                super::super::kin::to_world(local, kyaw)
            });
            let v = if let Some(v) = v_keys {
                v
            } else if exe_order {
                ahead.get(i + 1).map_or(v_own, |r| r.2)
            } else if pos_lag1 {
                if i == 0 { before_first.unwrap_or(v_own) } else { ahead[i - 1].2 }
            } else {
                v_own
            };
            let sub = (((t - last_t) as f32 / 1000.0) / TICK_S).round().max(1.0) as usize;
            for _ in 0..sub {
                tick_n += 1;
                // Keys mode: the crouch of the same input k as the walk (fx-review: the label tick is one ahead).
                let kk = if keys_mode { keys_in(t0 + 50 * (tick_n - 1)) } else { keys_in(t0 + 50 * tick_n) };
                body.want_crouch = kk & key::CROUCH != 0;
                let was = body.feet();
                let tick_turn = turn_of.get(&(w.place, t)).copied().unwrap_or(0.0);
                yaw += tick_turn;
                let v = if turn_first { rotate(v, tick_turn) } else { v };
                let v = match world_blend {
                    Some(k) => {
                        let cur = wv.unwrap_or(v);
                        let nv = [cur[0] + (v[0] - cur[0]) * k, cur[1] + (v[1] - cur[1]) * k];
                        wv = Some(nv);
                        nv
                    }
                    None => v,
                };
                body.walk = Vec3::new(v[0] * TICK_S, 0.0, v[1] * TICK_S);
                let mut ended = false;
                if jump_mode {
                    let k = t0 + 50 * (tick_n - 1);
                    if jump.is_none() && presses.contains(&k) && body.on_ground(&s) {
                        jump = Some(RefJump::new(k, keys_in(k), v[0].hypot(v[1]), body.feet()));
                    }
                    if let Some(j) = jump.as_mut() {
                        ended = j.tick(&mut body, k, kyaw, keys_in(k));
                    }
                }
                let notes = body.step(&collision, TICK_S, &s);
                if ended {
                    jump = None;
                }
                let now = body.feet();
                let moved = was.distance_xz(now);
                let asked = v[0].hypot(v[1]) * TICK_S;
                let ground = body.on_ground(&s);
                let branch = if notes.airborne || (!ground && body.velocity.y < -0.5) {
                    "air"
                } else if was_air {
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
                was_air = branch == "air";
                let normal = if notes.airborne { String::new() } else { format!("{:.3}", body.contact_normal.y) };
                iv.push_str(&format!(
                    "{rec_id},{},{t0},{tick_n},{},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{},{normal},{branch},0x{kk:x},{:.3},{:.3},{:.5},{:.5}\n",
                    w.place,
                    t0 + 50 * tick_n,
                    now.x,
                    now.y,
                    now.z,
                    (now.x - was.x) / TICK_S,
                    body.velocity.y,
                    (now.z - was.z) / TICK_S,
                    u8::from(ground),
                    v[0],
                    v[1],
                    tick_turn,
                    yaw
                ));
            }
            last_t = t;
        }
        let near_jump = presses.iter().any(|&p| p <= w.time && t0 < p + 2_500);
        if !rows_near_jump || near_jump {
            out.push_str(&iv);
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

/// Where the reference's jump is: on the floor before the take-off, or in the air (the controller's jump flag set).
#[derive(Clone, Copy, PartialEq, Eq)]
enum RefPhase {
    Floor,
    Air,
}

/// The client's jump in the reference (fx-team, A325 jump rows):
/// - the clip: `Clip::of` (jump codes, `player_logic_jump_state::initialize` 0x337040) by the keys at the press and the speed;
/// - the clip's time from the press: the sprint jump's prepare phase first (`PREPARE_MS`, LIKELY), then 30 frames a second times
///   its time scale (0x343C60, PROVEN);
/// - the walk vector each tick: the target (the mixer's object transform, 0x39F360) minus the body
///   (`set_physics_controller_walk_vector` 0x2D9060); the target moves by the clip's root delta of the tick, turned by the walk's
///   yaw (the old-yaw bake, `apply_rotation` 0x2D8AD0, as the keys mode); all three axes in the jump (0x2D91AC), the flat part
///   before;
/// - the take-off: the clip's `jump` event at frame 6 (`on_jump_event` 0x3434A0) sets the controller's jump flag (+0x2C70C), and
///   `player_step`'s air branch 0x7BF2C moves the body (no gravity; up, across with the wall slide, down);
/// - the air control: the keys' direction at `air_control_speed` 1 m/s, none for forward alone in a sprint jump (0x2D7990);
/// - the end: at the clip's last frame `jump_logic_state_landing::initialize` 0x3440E0 -> `end_jump` 0x7F050 (only a downward
///   vertical speed kept), then the walk branch with gravity.
/// Not here: the landing clips' root travel (the 'after the clip' rows), the press gates other than `on_ground` (stamina).
#[derive(Clone, Copy)]
struct RefJump {
    clip: Clip,
    press: u32,
    target: Vec3,
    phase: RefPhase,
}

impl RefJump {
    fn new(press: u32, keys: u32, speed: f32, feet: Vec3) -> Self {
        Self { clip: Clip::of(keys, speed), press, target: feet, phase: RefPhase::Floor }
    }

    /// The clip's frame at `time` (ms), clamped to its end.
    fn frame(&self, time: u32) -> f32 {
        let prepare = match self.clip {
            Clip::Sprint(_) => PREPARE_MS,
            Clip::Short(_) => 0.0,
        };
        ((time.saturating_sub(self.press) as f32 - prepare).max(0.0) / 1000.0 * JUMP_FPS * self.clip.time_scale()).min(END_FRAME)
    }

    /// Sets the body's walk for input tick `k` (yaw `yaw`, keys `keys`); true when the clip ends with this tick (`end_jump` done).
    fn tick(&mut self, body: &mut Controller, k: u32, yaw: f32, keys: u32) -> bool {
        let (f0, f1) = (self.frame(k), self.frame(k + 50));
        if f0 <= 0.0 {
            // The clip starts from where the body is (the prepare phase walks as the run).
            self.target = body.feet();
        }
        if f1 <= 0.0 {
            return false;
        }
        let (a, b) = (self.clip.root(f0), self.clip.root(f1));
        let d = super::super::kin::to_world([b[1] - a[1], -(b[2] - a[2])], yaw);
        self.target = self.target + Vec3::new(d[0], b[0] - a[0], d[1]);
        if f1 >= TAKEOFF_FRAME && self.phase == RefPhase::Floor {
            self.phase = RefPhase::Air;
            body.jumping = true;
        }
        let ended = f1 >= END_FRAME;
        if ended {
            body.end_jump();
        }
        let to = self.target - body.feet();
        if body.jumping {
            body.walk = to;
            let a = super::super::kin::to_world(self.clip.air_control(keys), yaw);
            body.air_control = Vec3::new(a[0], 0.0, a[1]);
        } else {
            body.walk = Vec3::new(to.x, 0.0, to.z);
            body.air_control = Vec3::ZERO;
        }
        ended
    }
}

/// The keys a movement clip depends on: the moves, aim, crouch, sprint.
const CLIP_KEYS: u32 = key::FORWARD | key::BACK | key::LEFT | key::RIGHT | key::AIM | key::CROUCH | key::SPRINT | key::FIRE;

/// `player_input::is_sprinting` 0x2F1220 (sprint_jump_re FINDINGS §1.3, PROVEN): sprint and forward, none of back, fire,
/// crouch (strafe allowed). `can_sprint` 0x2DBF40 refuses the aim key too (its hands vcall +0x60 = weapon_core::can_sprint 0x2E3A10 ->
/// is_going_to_aim 0x2FC2A0 reads keys +0x330 bit 0x80): callers test it. Weapon idle, aim_time, stamina, stagger, legs: not here.
fn sprint_input(keys: u32) -> bool {
    keys & (key::SPRINT | key::FORWARD) == key::SPRINT | key::FORWARD && keys & (key::BACK | key::FIRE | key::CROUCH) == 0
}

/// The client's movement clip for a key mask, as a body-frame velocity `[forward, left]` m/s, from the game data only:
/// - the direction (0x2D6510's mapping: F, F+R, R, B+R, B, B+L, L, F+L; F+B or none = standing);
/// - the clip's root speed along it (resources.db AK player clips, fx-zones nav-truth/fx-zones/clips/ak-root-motion.txt:
///   stand F and diagonals 3.50, B and diagonals 3.20, L/R 2.244; crouch 2.20, 1.95, 1.41; aimed = the same roots;
///   sprint 3.273 m / 0.6 s = 5.455, private/sprint-jump-re FINDINGS);
/// - times the clip's rate desired / default (gameplay/players/default.player movement_speed; forward diagonals take the fwd
///   entry, back diagonals back, L/R side; notes-walkvector §3): walk 3.5/3.5, 3/3.2, 2.5/2.25; aim 3/3.5, 2.5/3.2, 2/2.25;
///   crouch 2.5/2.2, 2/2, 2/1.4; crouch aim 2/2.2, 1.5/2, 1.5/1.4; sprint 5/5.5 (forward only, not aimed or crouched).
/// The speed factor's modifiers (skill, kit) are left at 0 (bots carry none in the 0.26g data: LIKELY).
fn key_clip(keys: u32) -> Local {
    let (f, b, l, r) = (keys & key::FORWARD != 0, keys & key::BACK != 0, keys & key::LEFT != 0, keys & key::RIGHT != 0);
    let fwd = f && !b;
    let back = b && !f;
    let side: f32 = match (l, r) {
        (true, false) => 1.0,
        (false, true) => -1.0,
        _ => 0.0,
    };
    if !fwd && !back && side == 0.0 {
        return [0.0, 0.0];
    }
    let (crouch, aim) = (keys & key::CROUCH != 0, keys & key::AIM != 0);
    if sprint_input(keys) && side == 0.0 && !aim {
        return [5.455 * 5.0 / 5.5, 0.0];
    }
    // (root speed, desired, default) by class: forward-ish, back-ish, pure side.
    let (root, desired, default) = match (crouch, aim, fwd, back) {
        (false, false, true, _) => (3.50, 3.5, 3.5),
        (false, false, _, true) => (3.20, 3.0, 3.2),
        (false, false, _, _) => (2.244, 2.5, 2.25),
        (false, true, true, _) => (3.50, 3.0, 3.5),
        (false, true, _, true) => (3.20, 2.5, 3.2),
        (false, true, _, _) => (2.244, 2.0, 2.25),
        (true, false, true, _) => (2.20, 2.5, 2.2),
        (true, false, _, true) => (1.95, 2.0, 2.0),
        (true, false, _, _) => (1.41, 2.0, 1.4),
        (true, true, true, _) => (2.20, 2.0, 2.2),
        (true, true, _, true) => (1.95, 1.5, 2.0),
        (true, true, _, _) => (1.41, 1.5, 1.4),
    };
    let speed = root * desired / default;
    let along: f32 = if fwd { 1.0 } else if back { -1.0 } else { 0.0 };
    let n = (along * along + side * side).sqrt();
    [along / n * speed, side / n * speed]
}
