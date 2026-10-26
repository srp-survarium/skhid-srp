//! The dry run of the jump test's pool mode (`bots/jump_test.rs`, cargo feature `jump-test`) on the stand-in
//! game: the whole pool of one try file worked by every bot of a real map's match, with the stand-in giving
//! the worlds the owner's client gives (every body's row, asked by the test and delivered a few tens of
//! milliseconds later). What it checks, offline, before the owner's time is spent:
//!
//! - every rest position the plan was made from fits its start (0.3 m, 0.3 m, 10 degrees);
//! - no body of the other team within 1.5 m of a bot in a try;
//! - each run goes the planned way (within 10 degrees of the approach);
//! - no bot presses anything but the moving keys (and the suicide bit);
//! - the tries a minute, and the voids by reason.
//!
//! The stand-in has no body collisions, so a leak of the team lock shows as bodies near each other, as it
//! would in the game; it does not model the client's jump, so the outcomes of the jumps are not read.
//!
//! ```text
//! SRP_MAPDATA_DIR=<dir> SRP_DRY_TRIES=<try file> SRP_DRY_LEVEL=4 SRP_DRY_PER_TEAM=6 SRP_DRY_SECONDS=600 \
//!   SRP_DRY_OUT=<log dir> cargo test -p match-server --release --features jump-test jump_pool_dry_run -- --ignored --nocapture
//! ```

use std::collections::BTreeMap;
use std::path::PathBuf;

use super::*;
use crate::v026g::bots::jump_test::{JumpTest, Row};

/// A tick with a body of the other team near a bot in a try: the clock, the bot, the other, their feet, the distance.
pub(crate) type Crowd = (u32, Place, Place, [f32; 3], [f32; 3], f32);

/// What the run found.
#[derive(Debug, Default)]
pub(crate) struct DryReport {
    pub tries: u32,
    pub minutes: f32,
    pub voids: BTreeMap<String, u32>,
    pub refused: Vec<String>,
    pub suicides: u32,
    pub suicide_failed: u32,
    /// Tries given back to the pool by a bot whose planner had no way to the start.
    pub released: u32,
    /// Ticks (25 ms) a bot in a try had an enemy body within 1.5 m, and the first few.
    pub crowded_ticks: u32,
    pub crowded_first: Vec<Crowd>,
    /// By what the near enemy was doing, and what the bot in the try was.
    pub crowded_by: BTreeMap<String, u32>,
    pub forbidden_keys: u32,
    /// The worst rest fit over the tries: metres, metres in height, degrees.
    pub worst_rest: [f32; 3],
    /// The worst way off the plan, degrees.
    pub worst_dir: f32,
    pub dirs_read: u32,
    pub per_site: BTreeMap<String, u32>,
    pub requests: u32,
    /// The seed the stand-in's spawns were drawn with, and the bodies it could not put on the grid.
    pub seed: u32,
    pub off_grid: u32,
    /// Every 30 s: what each bot is doing, and where.
    pub roll: Vec<String>,
}

/// The rows the client's world would hold, from the stand-in's bodies at `tick`.
fn rows_of_game(game: &Game, tick: u32, before: &[Vec3]) -> Vec<Row> {
    game.bodies
        .iter()
        .enumerate()
        .map(|(place, body)| {
            let jumping = body.flight.is_some() || body.air_until > tick;
            let moved = body.feet - before[place];
            Row {
                time: tick,
                place,
                alive: body.alive,
                feet: body.feet,
                jumping,
                jump_type: if jumping { 5 } else { 0 },
                speed: body.step_m / TICK_S,
                velocity: [moved.x * 4.0, moved.y * 4.0, moved.z * 4.0],
                normal: [0.0, 1.0, 0.0],
                keys: body.keys,
                yaw: body.yaw,
                pitch: body.pitch,
                sprinting: body.keys & key::SPRINT != 0,
                crouch: body.keys & key::CROUCH != 0,
            }
        })
        .collect()
}

pub(crate) fn dry_run(map_dir: &std::path::Path, level: u8, per_team: usize, tries: &std::path::Path, out: &std::path::Path, limit_s: u32, seed: u32) -> DryReport {
    use survarium::v026g::maps::mode;
    let mode = mode::TEAM_DEATHMATCH;
    let project = format!("level_{level:02}/day_{mode}");
    let map = mapdata::MapData::read(&map_dir.join(mapdata::format::file_name(&project))).unwrap();
    let world = Arc::new(World::build(map, mapdata::CELL, mapdata::Body::default()));
    let mut report = DryReport::default();
    let teams: Vec<u8> = (0..2 * per_team).map(|p| u8::from(p >= per_team)).collect();
    let driven = vec![true; teams.len()];
    let roles: Vec<Option<&str>> = vec![None; teams.len()];
    let mut bots = Bots::new(0xD27 + seed, mode, level, &teams, &driven, &roles, Arc::new(bench_tuning()), Trace::none(0));
    bots.jump_test = Some(JumpTest::new(tries.to_path_buf(), out.to_path_buf(), "dry".into(), 120));
    bots.map_ready(world.clone());
    // The stand-in moves a body only from a floor of the grid: a seed whose spawns are all on one.
    let mut game = Game::new(world.clone(), &teams, u64::from(seed), Some(100_000), None);
    let mut drawn = seed;
    while game.bodies.iter().any(|b| b.node.is_none()) && drawn < seed + 40 {
        drawn += 1;
        game = Game::new(world.clone(), &teams, u64::from(drawn), Some(100_000), None);
    }
    report.seed = drawn;
    report.off_grid = game.bodies.iter().filter(|b| b.node.is_none()).count() as u32;
    for place in 0..teams.len() {
        bots.entered(place, true, 0);
    }
    let limit_ms = limit_s * 1000;
    let (mut clock, mut next_world) = (0, 1_000);
    let mut before: Vec<Vec3> = game.bodies.iter().map(|b| b.feet).collect();
    // Uploads on their way: (due, tick, rows).
    let mut flying: VecDeque<(u32, u32, Vec<Row>)> = VecDeque::new();
    while clock < limit_ms {
        clock += 25;
        while game.tick + TICK_MS <= clock {
            game.step();
        }
        if clock >= next_world {
            next_world += 1_000;
            if let Some((_, fixes, health, facts)) = game.world_at(clock / TICK_MS * TICK_MS - 300) {
                for (place, (fix, health)) in fixes.into_iter().zip(health).enumerate() {
                    bots.fixed(place, fix, Some(health));
                }
                bots.facts(facts);
            }
        }
        let (made, notes) = bots.poll(Clock { clock, time_left_s: (limit_ms - clock) as f32 / 1000.0, total_s: limit_s as f32, playing: true });
        if let Some(places) = std::env::var("SRP_DRY_NOTES").ok().map(|v| v.split(',').filter_map(|p| p.parse::<i64>().ok()).collect::<Vec<_>>()) {
            for note in notes.iter().filter(|n| places.contains(&(n.place as i64))) {
                eprintln!("{:>7} ms  bot {:>2}: {}", note.time, note.place as i64, note.text);
            }
        }
        for made in made {
            let allowed = key::FORWARD | key::BACK | key::LEFT | key::RIGHT | key::JUMP | key::CROUCH | key::SPRINT | key::SUICIDE;
            if made.input.actions & !allowed != 0 {
                report.forbidden_keys += 1;
            }
            let queue = &mut game.inputs[made.place];
            assert!(queue.back().is_none_or(|last| last.0 < made.time));
            let r = made.input.rotation;
            let turn = (half_value(u16::from_le_bytes([r[0], r[1]])), half_value(u16::from_le_bytes([r[2], r[3]])));
            queue.push_back((made.time, made.input.actions, turn, None, Arms::fallback().primary.0));
            // The stand-in has no suicide: the bit kills the body (a death, a ticket).
            if made.input.actions & key::SUICIDE != 0 && game.bodies[made.place].alive {
                let tick = game.tick;
                let body = &mut game.bodies[made.place];
                body.alive = false;
                body.dead_at = tick;
                game.stats[made.place].1 += 1;
            }
        }
        // The world requests of the test, answered 40 ms later with every body.
        if clock % 30_000 == 0
            && let Some(test) = bots.jump_test.as_ref()
        {
            let line: Vec<String> = game
                .bodies
                .iter()
                .enumerate()
                .map(|(p, b)| format!("{p}:{}{}@({:.0},{:.0})", test.phase_name(p), if b.alive { "" } else { "+dead" }, b.feet.x, b.feet.z))
                .collect();
            report.roll.push(format!("{:>4} s  {}", clock / 1000, line.join("  ")));
            if let Some(places) = std::env::var("SRP_DRY_NOTES").ok().map(|v| v.split(',').filter_map(|p| p.parse::<usize>().ok()).collect::<Vec<_>>()) {
                for p in places {
                    if let Some(bot) = bots.bots[p].as_ref() {
                        report.roll.push(format!("      bot {p}: intent {:?} pace {:?} no_way {:?}", bot.intent.go, bot.intent.pace, bot.no_way));
                    }
                }
            }
        }
        if let Some(test) = bots.jump_test.as_mut() {
            while flying.front().is_some_and(|f| f.0 <= clock) {
                let (_, tick, rows) = flying.pop_front().expect("one is due");
                test.take_rows(rows, tick, clock, 30_000);
            }
            if test.wants_world(clock) {
                report.requests += 1;
                let tick = clock / TICK_MS * TICK_MS;
                flying.push_back((clock + 40, tick, rows_of_game(&game, tick, &before)));
                before = game.bodies.iter().map(|b| b.feet).collect();
            }
            // Bodies of the other team near a bot in a try.
            for (place, team) in test.trying() {
                let me = &game.bodies[place];
                if !me.alive {
                    continue;
                }
                for (other, body) in game.bodies.iter().enumerate() {
                    if body.alive && body.team != team && body.feet.distance_xz(me.feet) < 1.5 && (body.feet.y - me.feet.y).abs() < 1.5 {
                        report.crowded_ticks += 1;
                        *report.crowded_by.entry(format!("try {} / enemy {}", test.phase_name(place), test.phase_name(other))).or_default() += 1;
                        if report.crowded_first.len() < 5 {
                            report.crowded_first.push((clock, place, other, me.feet.to_array(), body.feet.to_array(), body.feet.distance_xz(me.feet)));
                        }
                    }
                }
            }
        }
    }
    report.minutes = limit_s as f32 / 60.0;
    // The tries log.
    let log = out.join("jump-test-dry-tries.jsonl");
    let text = std::fs::read_to_string(&log).unwrap_or_default();
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        if let Some(why) = v.get("void").and_then(|x| x.as_str()) {
            *report.voids.entry(why.to_owned()).or_default() += 1;
        } else if let Some(site) = v.get("refused").and_then(|x| x.as_str()) {
            report.refused.push(format!("{site}: {}", v["why"]));
        } else if v.get("released").is_some() {
            report.released += 1;
        } else if v.get("suicide").is_some() {
            report.suicides += 1;
        } else if v.get("suicide_failed").is_some() {
            report.suicide_failed += 1;
        } else if v.get("outcome").is_some() {
            report.tries += 1;
            *report.per_site.entry(v["site"].as_str().unwrap_or("?").to_owned()).or_default() += 1;
            if let Some(off) = v.get("rest_off").and_then(|x| x.as_array()) {
                for (i, worst) in report.worst_rest.iter_mut().enumerate() {
                    *worst = worst.max(off[i].as_f64().unwrap_or(0.0).abs() as f32);
                }
            }
            if let Some(dir) = v.get("dir_off").and_then(|x| x.as_f64()) {
                report.dirs_read += 1;
                // `dir_off`: the angle between the way the body went from rest and the approach of the plan.
                report.worst_dir = report.worst_dir.max(dir as f32);
            }
        }
    }
    report
}

/// The dry run of one try file on a real map (needs `SRP_MAPDATA_DIR`, `SRP_DRY_TRIES`).
#[test]
#[ignore = "a measurement; needs SRP_MAPDATA_DIR and SRP_DRY_TRIES"]
fn jump_pool_dry_run() {
    let (Some(dir), Some(tries)) = (std::env::var_os("SRP_MAPDATA_DIR"), std::env::var_os("SRP_DRY_TRIES")) else {
        eprintln!("skipping: SRP_MAPDATA_DIR and SRP_DRY_TRIES are not set");
        return;
    };
    let get = |name: &str, default: u32| std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default);
    let out = std::env::var_os("SRP_DRY_OUT").map_or_else(|| std::env::temp_dir().join("jump-dry"), PathBuf::from);
    std::fs::create_dir_all(&out).unwrap();
    for name in ["tries", "worlds", "bodies"] {
        let _ = std::fs::remove_file(out.join(format!("jump-test-dry-{name}.jsonl")));
    }
    let report = dry_run(
        &PathBuf::from(dir),
        get("SRP_DRY_LEVEL", 4) as u8,
        get("SRP_DRY_PER_TEAM", 6) as usize,
        &PathBuf::from(tries),
        &out,
        get("SRP_DRY_SECONDS", 600),
        get("SRP_DRY_SEED", 1),
    );
    eprintln!("{report:#?}");
    eprintln!(
        "tries {} in {:.1} min = {:.1} a minute; voids {:?}; released {}; refused {}; suicides {} (failed {}); bodies of the other team near a bot in a try: {} ticks; forbidden keys {}; \
         worst rest fit {:?} (m, m, degrees); worst run direction off the plan {:.1} degrees over {} runs; world requests {}",
        report.tries,
        report.minutes,
        report.tries as f32 / report.minutes,
        report.voids,
        report.released,
        report.refused.len(),
        report.suicides,
        report.suicide_failed,
        report.crowded_ticks,
        report.forbidden_keys,
        report.worst_rest,
        report.worst_dir,
        report.dirs_read,
        report.requests
    );
    assert_eq!(report.forbidden_keys, 0, "a bot of the test pressed a key outside the moving keys");
    assert_eq!(report.crowded_ticks, 0, "bodies of the other team came near a bot in a try: {:?}", report.crowded_first);
    assert!(report.worst_rest[0] <= 0.3 && report.worst_rest[1] <= 0.3 && report.worst_rest[2] <= 10.0 + 0.01, "a rest did not fit its start");
}
