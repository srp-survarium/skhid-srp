use super::*;
use crate::v026g::bots::tests::settings::bench_settings;

/// A deterministic made-up match of six bodies, long enough that every topic of the counts has
/// something to count (presses and landings, reloads, fire and hits, kills and held lines, a
/// grenade, takes and stuck notes), run through `step` and `finish_topics`. The counts and the
/// owner-test text of each topic are its file in `golden/` (`core.txt` for what the run shares): a
/// change to how the counts are worked out (not to what they count) shows in the file of its
/// topic only, and a new topic is a new file. A file is the topic's text, then its keys, one a line
/// and sorted. `SRP_WRITE_GOLDEN=<dir> cargo test ... the_counts_of_a_made_up_match` writes the
/// files to <dir> (the folder `golden/` of this one, to keep them) and does not compare.
fn made_up_match() -> Vec<(&'static str, String)> {
    let teams = [0u8, 0, 0, 1, 1, 1];
    let (mut game, mut tally) = made_up(&teams);
    let mut seed = 0x2545_F491_4F6C_DD1Du64;
    let mut next = move |n: u64| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed % n
    };
    let plays = ["Roam", "Collect", "Crossfire", "Cut", "Heal", "Trade"];
    let steps = ["Going", "Hold", "Follow", "Wait", "Fetch)", "Search", "Take", "Behind"];
    let all_keys = [
        key::FORWARD,
        key::BACK,
        key::LEFT,
        key::RIGHT,
        key::SPRINT,
        key::AIM,
        key::FIRE,
        key::JUMP,
        key::CROUCH,
    ];
    let mut feet: Vec<(f32, f32)> = (0..teams.len())
        .map(|p| (10.0 + 12.0 * p as f32, 8.0 + 4.0 * (p % 3) as f32))
        .collect();
    for k in 0..2400usize {
        game.tick = (k as u32 + 1) * TICK_MS;
        let tick = game.tick;
        let mut nows = Vec::new();
        for place in 0..teams.len() {
            // A body that mostly walks, stops now and then, jumps, fires.
            let mode = (k / 40 + place) % 7;
            let (dx, dz) = match mode {
                0 | 1 => (0.05, 0.0),
                2 => (-0.04, 0.03),
                3 => (0.0, 0.0),
                4 => (0.0, -0.05),
                5 => (0.02, 0.02),
                _ => (0.0, 0.0),
            };
            let room = if teams[place] == 0 { 76.0 } else { 40.0 };
            feet[place].0 = (feet[place].0 + dx).clamp(4.0, room);
            feet[place].1 = (feet[place].1 + dz).clamp(4.0, 36.0);
            let mut keys = 0;
            for (i, bit) in all_keys.iter().enumerate() {
                let on = match i {
                    0 => dx > 0.0 || dz != 0.0,
                    1..=3 => next(9) == 0,
                    4 => dx > 0.04,
                    5 | 6 => mode == 3 && next(3) == 0,
                    7 => next(150) == 0,
                    _ => next(40) == 0,
                };
                if on {
                    keys |= bit;
                }
            }
            // Windows with something to count: a calm body standing for 20 s (idle), two
            // teammates side by side for 15 s and in one another for 5 (close pair, one body),
            // a drop of 3 m, and two bodies at the grenade's landing as it goes off.
            let (mut y, mut x, mut z) = (0.0, feet[place].0, feet[place].1);
            if place == 2 && (400..800).contains(&k) {
                (x, z, keys) = (76.0, 36.0, 0);
            }
            if place == 0 && (900..1200).contains(&k) {
                (x, z) = (20.0, 10.0);
            }
            if place == 1 && (900..1200).contains(&k) {
                (x, z) = (if (1000..1100).contains(&k) { 20.2 } else { 21.5 }, 10.0);
            }
            if place == 3 && (1300..1320).contains(&k) {
                y = 3.0;
            }
            if [2, 4].contains(&place) && (1270..1300).contains(&k) {
                (x, z) = (30.0, 20.0);
            }
            put(&mut game, place, x, z, keys);
            game.bodies[place].feet.y = y;
            if place == 2 && (400..800).contains(&k) {
                game.bodies[place].target = None;
            }
            let body = &mut game.bodies[place];
            // Any number will do for the made-up yaw; this one is near pi by chance.
            #[allow(clippy::approx_constant)]
            let turn = 3.14;
            body.yaw = (next(628) as f32) / 100.0 - turn;
            body.step_m = (dx * dx + dz * dz).sqrt();
            body.carrying = (k / 300 % 4 == place % 4).then_some(0);
            body.alive = !(k % 500 > 480 && place % 2 == 0);
            body.target = (next(5) != 0 && !(place == 2 && (400..800).contains(&k))).then(|| (place + 3) % teams.len());
            if (place == 2 && (400..800).contains(&k)) || (place < 2 && (900..1200).contains(&k)) {
                body.step_m = 0.0;
            }
            body.air_until = if next(120) < 3 { tick + 3 * TICK_MS } else { 0 };
            let mut now = BotNow {
                dry: next(300) < 10,
                play: plays[(k / 200 + place) % plays.len()].to_owned(),
                goal_m: (next(4) != 0).then(|| next(60) as f32),
                ..BotNow::default()
            };
            if (k / 40) % 5 == 0 && k % 40 == 0 || k % (1000 / TICK_MS as usize) == 0 {
                now.step = Some(steps[next(steps.len() as u64) as usize].to_owned());
            }
            if next(400) < 8 {
                let at = tick.saturating_sub(next(200) as u32);
                now.reload = Some((at, at + 1000 + next(800) as u32));
            }
            if keys & key::JUMP != 0 {
                let low = Vec3::new(feet[place].0, 0.0, feet[place].1);
                now.jump = Some((
                    ["route", "hop", "unstick"][next(3) as usize],
                    (next(2) == 0).then(|| (low, Vec3::new(low.x + 1.0, 0.4 + next(120) as f32 / 100.0, low.z))),
                ));
            }
            nows.push(Some(now));
        }
        // Hits, kills and landings the game would have logged this tick.
        game.hit_log.clear();
        if next(25) == 0 {
            let (shooter, victim) = (next(3) as usize, 3 + next(3) as usize);
            game.hit_log.push((tick, shooter, victim));
            game.hit_log.push((tick, victim, shooter));
        }
        if next(120) == 0 {
            let (shooter, victim) = (next(6) as usize, next(6) as usize);
            game.kill_log.push((tick, shooter, victim));
        }
        if next(90) == 0 {
            let place = next(6) as usize;
            game.landings.push(Landing {
                place,
                at: tick.saturating_sub(300),
                to: Vec3::new(feet[place].0, next(150) as f32 / 100.0, feet[place].1),
                sprint: next(2) == 0,
            });
        }
        game.team_kills = (k / 700) as u32;
        game.team_hits = (k / 300) as u32;
        // The bots' notes this tick.
        let texts: &[&str] = &[
            "takes Roam Battery(0) (score 0.50: x)",
            "takes Trade Place(4) (score 0.50: x)",
            "takes Flank Place(1) (score 0.50: x)",
            "takes Crossfire Place(2) (score 0.50: x)",
            "is stuck at [1.0, 0.0, 2.0]: BackOff",
            "ends Collect: no way to its goal",
        ];
        if next(60) == 0 {
            let note = Note {
                place: next(6) as usize,
                time: tick,
                text: texts[next(texts.len() as u64) as usize].to_owned(),
            };
            tally.note(&note, &game);
        }
        if k == 1200 {
            let note = Note {
                place: 1,
                time: tick,
                text: format!("throws a grenade at (30.0, 0.5, 20.0), blast at {}", tick + 4000),
            };
            tally.note(&note, &game);
        }
        // The shots the stand-in would have fired at a target this tick: lines at made-up offsets (from `k` alone,
        // so that no draw above moves), all bands, classes and stances, some hits, some with no line.
        if k % 7 == 0 {
            use crate::v026g::bots::tests::tallies::fight_tally::ShotFact;
            use crate::v026g::bots::tests::tallies::shot_offset::{DistBand, Offset, ShotClass, ShotOffset, Stance};
            let n = (k / 7) as u32;
            let distance = 4.0 + (n * 13 % 95) as f32;
            let (lateral, vertical) = ((n * 7 % 31) as f32 / 10.0 - 1.55, (n * 11 % 29) as f32 / 10.0 - 1.45);
            let line = (!n.is_multiple_of(23)).then(|| Offset { miss: lateral.hypot(vertical), lateral, vertical });
            game.shot_facts.push(ShotFact {
                shooter: n as usize % 3,
                tick,
                hit: (n.is_multiple_of(3)).then_some(3),
                target: Some((3, distance, 0.0)),
                moving: false,
                aimed: true,
                crouched: false,
                aim_state: Default::default(),
                cone: 0.0,
                offset: Some(ShotOffset {
                    band: DistBand::of(distance),
                    class: ShotClass::ALL[n as usize % 7],
                    stance: if n.is_multiple_of(5) { Stance::Crouched } else { Stance::Standing },
                    line,
                }),
            });
        }
        tally.step_with(&mut game, &nows);
        game.shot_facts.clear();
    }
    tally.run.camper = Some(3);
    let Topics { core, parts } = tally.finish_topics(&game);
    let file = |text: Option<&str>, json: serde_json::Map<String, serde_json::Value>| {
        let json = serde_json::to_string_pretty(&serde_json::Value::Object(json)).unwrap();
        match text {
            Some(text) => format!("{text}\n{json}\n"),
            None => format!("{json}\n"),
        }
    };
    let mut files = vec![("core", file(None, core))];
    files.extend(parts.into_iter().map(|(topic, part)| (topic, file(Some(&part.text), part.json))));
    files
}

/// A golden file as its text and its keys: the keys parsed, so that key order or the way a float is
/// written never makes a difference (the text is compared as it is).
fn read_golden(file: &str) -> (String, serde_json::Value) {
    let file = file.replace("\r\n", "\n");
    let at = file.lines().position(|line| line.starts_with('{')).expect("a golden file has its keys");
    let lines: Vec<&str> = file.lines().collect();
    (lines[..at].join("\n"), serde_json::from_str(&lines[at..].join("\n")).expect("the keys are JSON"))
}

#[test]
fn the_counts_of_a_made_up_match_are_what_they_were() {
    let got = made_up_match();
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/v026g/bots/tests/tallies/owner/golden");
    if let Some(out) = bench_settings().var_os("SRP_WRITE_GOLDEN") {
        let out = std::path::Path::new(&out);
        std::fs::create_dir_all(out).unwrap();
        for (topic, file) in &got {
            std::fs::write(out.join(format!("{topic}.txt")), file).unwrap();
        }
        return;
    }
    let mut have: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    have.sort();
    let mut topics: Vec<String> = got.iter().map(|(topic, _)| format!("{topic}.txt")).collect();
    topics.sort();
    assert_eq!(have, topics, "a topic without a file, or a file without a topic (golden/)");
    for (topic, file) in &got {
        let want = std::fs::read_to_string(dir.join(format!("{topic}.txt"))).unwrap();
        let (got_text, got_keys) = read_golden(file);
        let (want_text, want_keys) = read_golden(&want);
        assert!(
            got_text == want_text && got_keys == want_keys,
            "the counts of `{topic}` changed:\n--- got\n{file}\n--- want\n{want}"
        );
    }
}
