//! A325: a REFERENCE body beside the server's reckoning on a real client match, built from the recorded keys by the client's own
//! rules only (nothing from the reckoning), to measure how close the bots' model of the client can get and where it parts.
//!
//! The drift lab's driver (`drift_replay.rs`): the recording's inputs drive one `BodyTrack` a place, each world fixes it. Per world
//! t of a life after its first, from the world before (b):
//! - the RECKONING: the track's own estimate at t (`reckoned_at`, the last column of the check rows);
//! - the REFERENCE: the client's walk tick (`mapdata::controller::tick`, ported from the exe and checked tick by tick in fx-grid's
//!   Unicorn harness) on the map's walker collision, from the client's feet at b, stepped every tick by the walk the keys give:
//!   - the clips: each key class's root speed (resources.db AK clips) x desired / default (default.player movement_speed), a new
//!     clip at weight 0 on its own tick, +1/6 a tick (0.3 s, 0x921804), Σ w_i v_i unnormalised, older clips keeping what the
//!     newer leave; the stack at b replayed from the keys of the 8 ticks before b (the client is mid-blend at most worlds);
//!   - the walk of tick k turned by the yaw BEFORE input k's turn (`apply_rotation` 0x2D8AD0 bakes the root motion so far with
//!     the old yaw; fx-grid, PROVEN);
//!   - the flat step capped at `max_speed` 0x2DC010 by the sprint state (`set_physics_controller_walk_vector` 0x2D9060; the
//!     sprint state's 0x336A10 / 0x336A50 set and clear the controller's +0x204 on the input's own tick); the same byte takes
//!     the forward step's capsule branch 0x86490 (`Controller::step_as` with the sprint; the impassable slide keeps the
//!     console's test);
//!   - the speed factor F: each world's own (the table's `speeds` = `KitSpeeds::in_hands`, the exe's formula on the kit in use and
//!     the weapon in hands; minus 0.05 carrying a battery, which also refuses the sprint) plus the body parts' modifiers; else the
//!     median of the player's last 5 steady-world reads of its recorded speed;
//!   - the legs (`damage_model+0x388`, fx-review A325f): both legs holding leg_damage = no walk, sprint or jump until it ends
//!     (FSM 0x2FBF00 / 0x2FBDF0); one = no sprint (0x2DBF58), no jump (0x2FC254);
//!   - the start state: the controller the previous interval left (vertical speed, air), moved to the client's feet at b, with
//!     the world's own controller block (contact normal, velocity, slide, ceiling; `bullet_character_controller::serialize`
//!     0x7F7F0) where the tables carry it; the jump flag is the jump's (below);
//!   - the jump (fx-team): the clip by `Clip::of` from the press, its root re-based on the body each tick, the take-off at the
//!     clip's jump event (the controller's jump flag), air control, `end_jump` at its last frame; the presses the stamina bar
//!     refuses start none.
//!
//! Env: `SRP_A304_WORLDS`, `SRP_A304_INPUTS`, `SRP_A304_MAP`, `SRP_MAPDATA_DIR`, `SRP_BENCH_TUNING` as the lab; `SRP_A325_REC` the
//! recording's id; `SRP_A325_OUT` the per-tick rows (and `<out>.check`, one line an interval); `SRP_A325_NO_JUMP=1` leaves the
//! jumps out; `SRP_A325_ROWS_NEAR_JUMP=1` keeps only the rows within 2.5 s after a press; `SRP_A325_WINDOWS` a rows file
//! `rec,place,t0,t1` (only those worlds). The optional columns read from the tables (fx-bench's tables-v6): `speeds`, `carrying`
//! in rec_worlds.csv; `normal_x/y/z`, `fall_x/y/z`, `slide_x/y/z`, `ceil_x/y/z`, `leg6`, `leg7`, `leg_damage_until`, `parts_speed`
//! in the rec_motion.csv beside it. Scripts and the acceptance table: private/pvb-343-check/traces-20261028/fx-smoke/a325.

use std::collections::HashMap;

use mapdata::controller::Collision;
use mapdata::controller::speed::{self, Gait, Heading, Stance};
use mapdata::controller::tick::{Controller, Settings};
use mapdata::{Vec3, World};

use super::super::body::{BodyTrack, Fix};
use super::super::jump_arc::{Clip, PREPARE_MS, TAKEOFF_FRAME};
use super::super::kin::Local;
use super::bench::bench_tuning;
use crate::v026g::drive::key;

const WORLD_LATE_MS: u32 = 300;
const TICK_MS: u32 = 50;
const TICK_S: f32 = 0.05;

fn read_csv(path: &str) -> Vec<HashMap<String, String>> {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let mut lines = text.lines();
    let head: Vec<String> = lines.next().expect("a header").split(',').map(str::to_owned).collect();
    lines.map(|line| head.iter().cloned().zip(line.split(',').map(str::to_owned)).collect()).collect()
}

/// A table that may be missing (an older table set): its rows, or none.
fn read_csv_if(path: &std::path::Path) -> Vec<HashMap<String, String>> {
    if path.is_file() { read_csv(&path.to_string_lossy()) } else { Vec::new() }
}

fn hex(s: &str) -> u32 {
    u32::from_str_radix(s.trim_start_matches("0x"), 16).unwrap_or(0)
}

fn at(r: &HashMap<String, String>) -> Option<(u32, usize)> {
    Some((r.get("time")?.parse().ok()?, r.get("place")?.parse().ok()?))
}

/// A world's controller block (the start state): contact normal, velocity, slide, ceiling.
#[derive(Clone, Copy)]
struct Block {
    normal: Vec3,
    velocity: Vec3,
    slide: Vec3,
    ceiling: Vec3,
}

/// A world's body parts, as far as the walk goes: the parts' speed modifier sum, and the legs holding leg_damage until when.
#[derive(Clone, Copy)]
enum Legs {
    Sound,
    /// One leg at 0: no sprint, no jump.
    One { until: u32 },
    /// Both legs at 0: no walk at all.
    Both { until: u32 },
}

#[derive(Clone, Copy)]
struct Parts {
    speed: f32,
    legs: Legs,
}

/// The speed factor a world gives: the exe's F for the hands (one value) or the kit's candidates, and a battery carried.
struct WorldF {
    speeds: Vec<f32>,
    carrying: bool,
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
    // The controller rules the server's grid uses (the exe's caster and wall normal with `grid.exe_collision`).
    let (caster, s): (_, Settings) = tuning.grid_body().controller();
    let collision = Collision::build(&map.walker_triangles()).with_caster(caster);
    let world = World::build_with(
        map,
        mapdata::CELL,
        tuning.grid_body(),
        tuning.off_mesh_links(),
        tuning.needs_wall_clearance(),
        tuning.needs_open_sides(),
    );
    let grid = &*world.grid;
    let mut ticks: Vec<(u32, usize, u32, (f32, f32))> = read_csv(&inputs)
        .iter()
        .filter_map(|r| {
            let place: usize = r.get("place")?.parse().ok()?;
            (place != 0).then_some(())?;
            Some((r.get("time")?.parse().ok()?, place, hex(r.get("actions")?), (r.get("rot_a")?.parse().ok()?, r.get("rot_b")?.parse().ok()?)))
        })
        .collect();
    ticks.sort_by_key(|t| (t.0, t.1));
    let jump_mode = var("SRP_A325_NO_JUMP").is_none();
    let rows_near_jump = var("SRP_A325_ROWS_NEAR_JUMP").is_some();

    // The tables' optional columns.
    let motion = std::path::Path::new(&worlds).with_file_name("rec_motion.csv");
    let mut block_of: HashMap<(u32, usize), Block> = HashMap::new();
    let mut parts_of: HashMap<(u32, usize), Parts> = HashMap::new();
    for r in read_csv_if(&motion) {
        let Some(key) = at(&r) else { continue };
        let num = |k: &str| r.get(k).and_then(|v| v.parse::<f32>().ok());
        let v3 = |a: &str, b: &str, c: &str| Some(Vec3::new(num(a)?, num(b)?, num(c)?));
        if let (Some(normal), Some(velocity), Some(slide), Some(ceiling)) = (
            v3("normal_x", "normal_y", "normal_z"),
            v3("fall_x", "fall_y", "fall_z"),
            v3("slide_x", "slide_y", "slide_z"),
            v3("ceil_x", "ceil_y", "ceil_z"),
        ) {
            block_of.insert(key, Block { normal, velocity, slide, ceiling });
        }
        if let (Some(l6), Some(l7)) = (num("leg6"), num("leg7")) {
            let until = r.get("leg_damage_until").and_then(|v| v.parse::<u32>().ok()).unwrap_or(0);
            let legs = match (l6 <= 0.0, l7 <= 0.0) {
                (true, true) => Legs::Both { until },
                (true, false) | (false, true) => Legs::One { until },
                (false, false) => Legs::Sound,
            };
            parts_of.insert(key, Parts { speed: num("parts_speed").unwrap_or(0.0), legs });
        }
    }
    let mut f_of: HashMap<(u32, usize), WorldF> = HashMap::new();

    let mut keys_of: HashMap<usize, Vec<(u32, u32)>> = HashMap::new();
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
    struct Row {
        time: u32,
        place: usize,
        fix: Fix,
    }
    let world_rows = read_csv(&worlds);
    for r in &world_rows {
        let Some(key) = at(r) else { continue };
        let speeds: Vec<f32> = r.get("speeds").map(|v| v.split(';').filter_map(|x| x.parse::<f32>().ok()).collect()).unwrap_or_default();
        if !speeds.is_empty() {
            f_of.insert(key, WorldF { speeds, carrying: r.get("carrying").is_some_and(|v| v == "1") });
        }
    }
    let mut rows: Vec<Row> = world_rows
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
                    fall: None,
                    controller: None,
                },
            })
        })
        .collect();
    rows.sort_by_key(|w| (w.time, w.place));
    // The presses the client's stamina lets go (`player_stamina` 0x329790 / spend 0x329A20 / sprint 0x329A70, the jump's gate
    // 0x2FC230: `stamina::Reckoning`): a bar a place from each life's first world, ticked on its inputs; a press the bar refuses
    // starts no jump.
    let mut spawns: HashMap<usize, Vec<u32>> = HashMap::new();
    let mut life_of: HashMap<usize, u32> = HashMap::new();
    for w in &rows {
        if w.fix.alive && life_of.get(&w.place) != Some(&w.fix.inserted) {
            life_of.insert(w.place, w.fix.inserted);
            spawns.entry(w.place).or_default().push(w.time);
        }
    }
    let presses_of: HashMap<usize, Vec<u32>> = presses_of
        .into_iter()
        .map(|(place, presses)| {
            let lives = spawns.get(&place).cloned().unwrap_or_default();
            let mut bar = super::super::stamina::Reckoning::default();
            let mut next_life = 0usize;
            let mut kept = Vec::new();
            for &(t, k) in keys_of.get(&place).map_or(&[][..], Vec::as_slice) {
                while next_life < lives.len() && lives[next_life] <= t {
                    bar.spawn(lives[next_life]);
                    next_life += 1;
                }
                if presses.contains(&t) {
                    let mut probe = bar.clone();
                    probe.tick(t, k & !key::JUMP, false, false);
                    if probe.may_jump() {
                        kept.push(t);
                    }
                }
                bar.tick(t, k, false, false);
            }
            (place, kept)
        })
        .collect();
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
    let mut last_level: HashMap<usize, Vec<f32>> = HashMap::new();
    let mut carried: HashMap<usize, (u32, Controller)> = HashMap::new();
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
        let parts = parts_of.get(&(t0, w.place)).copied();
        let legs = parts.map_or(Legs::Sound, |p| p.legs);
        let carrying = f_of.get(&(t0, w.place)).is_some_and(|f| f.carrying);
        let one_leg = matches!(legs, Legs::One { until } if until > t0);
        // A battery carrier never sprints (fx-zones #1424 carrier_sprint); one leg holding leg_damage: no sprint, no jump.
        let mask = if carrying { !key::SPRINT } else { u32::MAX } & if one_leg { !(key::SPRINT | key::JUMP) } else { u32::MAX };
        let keys_in = |t: u32| keys_of.get(&w.place).and_then(|v| v.iter().rev().find(|&&(tt, _)| tt <= t).map(|&(_, k)| k)).unwrap_or(0) & mask;
        let jumped = keys_of.get(&w.place).is_some_and(|v| v.iter().any(|&(t, k)| t > t0 && t <= w.time && k & key::JUMP != 0));
        // The start state: the previous interval's controller (the jump flag is the jump's), else standing; the world's own block on top.
        let mut body = match carried.get(&w.place) {
            Some(&(end, prev)) if end == t0 => {
                let mut b = prev;
                let shift = from - b.feet();
                b.origin = b.origin + shift;
                b.pos = b.pos + shift;
                b.jumping = false;
                b
            }
            _ => Controller::standing_at(from),
        };
        if let Some(block) = block_of.get(&(t0, w.place)) {
            if block.normal.length() > 0.5 {
                body.contact_normal = block.normal * (1.0 / block.normal.length());
            }
            body.velocity = block.velocity;
            body.slide = block.slide;
            body.ceiling_normal = block.ceiling;
        }
        // The clip stack at b: the keys of the 8 ticks before b played through the ramp.
        let k0 = t0.saturating_sub(TICK_MS * 8);
        let mut layers: Vec<(Local, u32, u32)> = vec![(key_clip(keys_in(k0) & CLIP_KEYS), 0, keys_in(k0) & CLIP_KEYS)];
        for k in (k0 + TICK_MS..t0).step_by(TICK_MS as usize) {
            let kk = keys_in(k) & CLIP_KEYS;
            if layers.last().is_some_and(|l| l.2 != kk) {
                layers.push((key_clip(kk), k, kk));
            }
        }
        // The level: the world's F (+ the parts), else the median of the player's last 5 steady-world speed reads.
        let clip = key_clip(keys_b & CLIP_KEYS);
        let clip_speed = clip[0].hypot(clip[1]);
        let measured = match speed_b {
            Some(v) if clip_speed > 0.5 && v > 0.5 && (k0 + TICK_MS..=t0).step_by(TICK_MS as usize).all(|k| keys_in(k) & CLIP_KEYS == keys_b & CLIP_KEYS) => {
                let seen = last_level.entry(w.place).or_default();
                seen.push((v / clip_speed).clamp(0.7, 1.3));
                median_of_last(seen)
            }
            _ => last_level.get(&w.place).map_or(1.0, |seen| median_of_last(seen)),
        };
        let legs_stop_until = match legs {
            Legs::Both { until } if until > t0 => until,
            _ => 0,
        };
        let level = match f_of.get(&(t0, w.place)) {
            Some(f) => {
                let battery = if f.carrying { 0.05 } else { 0.0 };
                let f = f.speeds.iter().map(|v| (v - battery).max(0.0)).min_by(|a, b| (a - measured).abs().total_cmp(&(b - measured).abs()));
                f.map_or(measured, |f| (f + parts.map_or(0.0, |p| p.speed)).max(0.0))
            }
            None => measured,
        };
        let turn_at = |k: u32| turn_of.get(&(w.place, k)).copied().unwrap_or(0.0);
        let mut kyaw = yaw_b - turn_at(t0);
        let presses: &[u32] = presses_of.get(&w.place).map_or(&[], Vec::as_slice);
        let walk_speed = |k: u32| {
            let c = key_clip(keys_in(k) & CLIP_KEYS);
            c[0].hypot(c[1]) * level
        };
        let cap_of = |kk: u32| {
            let gait = if sprint_input(kk) && kk & key::AIM == 0 { Gait::Sprint } else { Gait::Move { stance: Stance::Stand, heading: Heading::Forward, aiming: false } };
            speed::cap(gait, level)
        };
        // A jump pressed before b and not over by b is under way at b.
        let mut jump: Option<RefJump> = None;
        if jump_mode && let Some(&p) = presses.iter().rev().find(|&&p| p < t0) {
            let mut j = RefJump::new(p, keys_in(p), walk_speed(p.saturating_sub(TICK_MS)), from);
            if j.frame(t0) < j.clip.air_end_frame() - 1e-3 {
                if j.frame(t0) >= TAKEOFF_FRAME {
                    j.phase = RefPhase::Air;
                    body.jumping = true;
                }
                jump = Some(j);
            }
        }
        let mut iv = String::new();
        let mut yaw = yaw_b;
        let mut was_air = false;
        for (tick_n, t) in (t0 + TICK_MS..=w.time).step_by(TICK_MS as usize).enumerate() {
            let tick_n = tick_n as u32 + 1;
            // Tick k = t - 50 with input k: its keys, the yaw before its turn.
            let k = t - TICK_MS;
            if k > t0 {
                kyaw += turn_at(k - TICK_MS);
            }
            let kk = keys_in(k) & CLIP_KEYS;
            if layers.last().is_none_or(|l| l.2 != kk) {
                layers.push((key_clip(kk), k, kk));
            }
            let mut sum = [0.0f32; 2];
            let mut left = 1.0f32;
            let mut keep = layers.len();
            for (j, &(clip, start, _)) in layers.iter().enumerate().rev() {
                let wgt = if start == 0 { 1.0 } else { (((k - start) / TICK_MS) as f32 / 6.0).min(1.0) };
                sum[0] += clip[0] * wgt * left;
                sum[1] += clip[1] * wgt * left;
                left *= 1.0 - wgt;
                if left <= 0.0 {
                    keep = layers.len() - j;
                    break;
                }
            }
            layers.drain(..layers.len() - keep);
            let mut local = [sum[0] * level, sum[1] * level];
            let cap = cap_of(keys_in(k));
            let len = local[0].hypot(local[1]);
            if len > cap {
                local = [local[0] * cap / len, local[1] * cap / len];
            }
            if legs_stop_until > k {
                local = [0.0, 0.0];
            }
            let v = super::super::kin::to_world(local, kyaw);
            let keys = keys_in(k);
            // [A364 P2] The crouch key through the game's logic (`Controller::crouch_key`, PROVEN on 27,790 traced steps): held,
            // no stand test and no ceiling push; released, the body stands on the SECOND step after (the stand test first) and
            // stays down under a ceiling. Setting `want_crouch` from the key stood it a step early and pushed it out from under
            // ceilings the game keeps it under.
            body.crouch_key(keys & key::CROUCH != 0);
            let was = body.feet();
            let tick_turn = turn_at(t);
            yaw += tick_turn;
            body.walk = Vec3::new(v[0] * TICK_S, 0.0, v[1] * TICK_S);
            let mut ended = false;
            if jump_mode {
                if jump.is_none() && !one_leg && legs_stop_until <= k && presses.contains(&k) && body.on_ground(&s) {
                    jump = Some(RefJump::new(k, keys, v[0].hypot(v[1]), body.feet()));
                }
                if let Some(j) = jump.as_mut() {
                    ended = j.tick(&mut body, k, kyaw, keys, cap_of(keys));
                }
            }
            let sprinting = sprint_input(keys) && keys & key::AIM == 0;
            let notes = body.step_as(&collision, TICK_S, &s, sprinting);
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
                "{rec_id},{},{t0},{tick_n},{t},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{},{normal},{branch},0x{keys:x},{:.6},{:.6},{:.5},{:.5}\n",
                w.place,
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
        let near_jump = presses.iter().any(|&p| p <= w.time && t0 < p + 2_500);
        if !rows_near_jump || near_jump {
            out.push_str(&iv);
        }
        carried.insert(w.place, (w.time, body));
        let refe = body.feet();
        let dist = (refe - feet).length();
        let (a, b) = (Vec3::new(refe.x - from.x, 0.0, refe.z - from.z), Vec3::new(feet.x - from.x, 0.0, feet.z - from.z));
        let (la, lb) = (a.length(), b.length());
        let ang = if la > 0.2 && lb > 0.2 { (a.dot(b) / (la * lb)).clamp(-1.0, 1.0).acos().to_degrees() } else { 0.0 };
        close += u32::from(dist < 0.25);
        far += u32::from(dist >= 0.5);
        checks.push_str(&format!(
            "CHECK,{rec_id},{},{t0},{dist:.3},{:.3},{ang:.1},{},{:.3},{},{level:.4}\n",
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

/// The median of the last 5 reads (a world whose body was held by a wall reads low; one read must not set a life's level).
fn median_of_last(seen: &[f32]) -> f32 {
    let mut last: Vec<f32> = seen.iter().rev().take(5).copied().collect();
    last.sort_by(f32::total_cmp);
    last.get(last.len() / 2).copied().unwrap_or(1.0)
}

/// Where the reference's jump is: on the floor before the take-off, or in the air (the controller's jump flag set).
#[derive(Clone, Copy, PartialEq, Eq)]
enum RefPhase {
    Floor,
    Air,
}

/// The client's jump in the reference (fx-team, A325 jump rows):
/// - the clip: `Clip::of` (jump codes, `player_logic_jump_state::initialize` 0x337040) by the keys at the press and the speed;
/// - the clip's time from the press: the sprint jump's prepare phase first (`PREPARE_MS`, a fixed 75 ms: not the client's rule,
///   whose prepare lasts 2AC / (A + BC) < 2C = 80 ms by the running clip's phase at the press, 0x3375A0 with C = startup.cfg
///   `jump_prepare_interval_length` 0.040, counted 52-81 ms from the press to the take-off clip's start, rig
///   `notes-jump-state.md` 0; kept until changed, register G13), then 30 frames a second times its time scale (0x343C60,
///   PROVEN);
/// - the walk vector each tick: the tick's root delta, the root re-based on the body each tick (0x39F450, fx-review A325g), turned
///   by the walk's yaw (the old-yaw bake, `apply_rotation` 0x2D8AD0); all three axes in the jump (0x2D91AC), the flat part before;
/// - the take-off: the clip's `jump` event at frame 6 (`on_jump_event` 0x3434A0) sets the controller's jump flag (+0x2C70C), and
///   `player_step`'s air branch 0x7BF2C moves the body;
/// - the air control: the keys' direction at `air_control_speed` 1 m/s, none for forward alone in a sprint jump (0x2D7990);
/// - the end: at the clip's last frame `end_jump` 0x7F050 (only a downward vertical speed kept), then the walk with gravity.
///
/// Not here: the landing clips' root travel.
#[derive(Clone, Copy)]
struct RefJump {
    clip: Clip,
    /// [A364] The share of the clip's travel the jump moves by (`Clip::carry` of the press's speed).
    carry: f32,
    press: u32,
    target: Vec3,
    phase: RefPhase,
}

impl RefJump {
    fn new(press: u32, keys: u32, speed: f32, feet: Vec3) -> Self {
        let clip = Clip::of(keys, speed);
        Self { clip, carry: clip.carry(speed), press, target: feet, phase: RefPhase::Floor }
    }

    /// The clip's frame at `time` (ms), clamped to the air phase's last step ([A364 P6] `Clip::air_end_frame`).
    fn frame(&self, time: u32) -> f32 {
        let prepare = match self.clip {
            Clip::Sprint(_) => PREPARE_MS,
            Clip::Short(_) => 0.0,
        };
        // [A364 P6] The clip's time scale on the floor, 1.5 frames a step in the air.
        self.clip.frame_after(time.saturating_sub(self.press) as f32 - prepare).min(self.clip.air_end_frame())
    }

    /// Sets the body's walk for input tick `k` (yaw `yaw`, keys `keys`); true when the clip ends with this tick (`end_jump` done).
    /// `cap` (m/s): the walk vector's flat cap (0x2D9060), skipped while the controller's jump flag is set (fx-review A328).
    fn tick(&mut self, body: &mut Controller, k: u32, yaw: f32, keys: u32, cap: f32) -> bool {
        let (f0, f1) = (self.frame(k), self.frame(k + TICK_MS));
        if f1 <= 0.0 {
            return false;
        }
        self.target = body.feet();
        let (a, b) = (self.clip.root(f0), self.clip.root(f1));
        // [A364] A short jump travels its direction clip x the carry (0x337280), a sprint jump its clip whole.
        let k = self.carry;
        let d = super::super::kin::to_world([(b[1] - a[1]) * k, -(b[2] - a[2]) * k], yaw);
        self.target = self.target + Vec3::new(d[0], b[0] - a[0], d[1]);
        if f1 >= TAKEOFF_FRAME && self.phase == RefPhase::Floor {
            self.phase = RefPhase::Air;
            body.jumping = true;
        }
        // [A364 P6] The air phase ends with its twelfth step, short of the clip's end.
        let ended = f1 >= self.clip.air_end_frame() - 1e-3;
        if ended {
            body.end_jump();
        }
        let to = self.target - body.feet();
        if body.jumping {
            body.walk = to;
            let a = super::super::kin::to_world(self.clip.air_control(keys), yaw);
            body.air_control = Vec3::new(a[0], 0.0, a[1]);
        } else {
            let (mut x, mut z) = (to.x, to.z);
            let (len, most) = (x.hypot(z), cap * TICK_S);
            if len > most {
                (x, z) = (x * most / len, z * most / len);
            }
            body.walk = Vec3::new(x, 0.0, z);
            body.air_control = Vec3::ZERO;
        }
        ended
    }
}

/// The keys a movement clip depends on: the moves, aim, crouch, sprint (and fire, which refuses the sprint).
const CLIP_KEYS: u32 = key::FORWARD | key::BACK | key::LEFT | key::RIGHT | key::AIM | key::CROUCH | key::SPRINT | key::FIRE;

/// `player_input::is_sprinting` 0x2F1220: sprint and forward, none of back, fire, crouch (strafe allowed). `can_sprint` 0x2DBF40
/// also refuses the aim key (weapon_core::can_sprint 0x2E3A10 -> is_going_to_aim 0x2FC2A0): callers test it. Weapon idle,
/// aim_time, stamina, stagger: not here.
fn sprint_input(keys: u32) -> bool {
    keys & (key::SPRINT | key::FORWARD) == key::SPRINT | key::FORWARD && keys & (key::BACK | key::FIRE | key::CROUCH) == 0
}

/// The client's movement clip for a key mask, as a body-frame velocity `[forward, left]` m/s at F = 1, from the game data:
/// - the direction (0x2D6510's mapping: F, F+R, R, B+R, B, B+L, L, F+L; F+B or none = standing);
/// - the clip's root speed along it (resources.db AK player clips: stand F and diagonals 3.50, B and diagonals 3.20, L/R 2.244;
///   crouch 2.20, 1.95, 1.41; aimed = the same roots; sprint 3.273 m / 0.6 s = 5.455);
/// - times the clip's rate desired / default (gameplay/players/default.player movement_speed): walk 3.5/3.5, 3/3.2, 2.5/2.25;
///   aim 3/3.5, 2.5/3.2, 2/2.25; crouch 2.5/2.2, 2/2, 2/1.4; crouch aim 2/2.2, 1.5/2, 1.5/1.4; sprint 5/5.5 (forward only).
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
