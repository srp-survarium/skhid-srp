use super::*;

/// A match of `per_team` bots a side to `win` batteries (Team Deathmatch
/// when `win` is 0: the most kills when time is up), for at most
/// `limit_s`.
pub(crate) fn soak(per_team: usize, win: u32, tdm_tickets: u32, limit_s: u32, seed: u64) -> Outcome {
    soak_mode(per_team, win, tdm_tickets, None, limit_s, seed)
}

/// [`soak`], with timed batteries (one every `timed_s`) when given.
pub(crate) fn soak_mode(
    per_team: usize,
    win: u32,
    tdm_tickets: u32,
    timed_s: Option<u32>,
    limit_s: u32,
    seed: u64,
) -> Outcome {
    soak_tuned(
        per_team,
        win,
        tdm_tickets,
        timed_s,
        limit_s,
        seed,
        bench_tuning(),
    )
}

/// [`soak_mode`] with the bots' `tuning`.
pub(crate) fn soak_tuned(
    per_team: usize,
    win: u32,
    tdm_tickets: u32,
    timed_s: Option<u32>,
    limit_s: u32,
    seed: u64,
    tuning: Tuning,
) -> Outcome {
    soak_volume(per_team, win, tdm_tickets, timed_s, limit_s, seed, tuning, HitVolume::Disc)
}

/// [`soak_tuned`] with the stand-in's shots tested against `volume` (the soaks keep the old disc; the bench sets
/// its own, [`HitVolume::bench`]).
#[allow(clippy::too_many_arguments)]
pub(crate) fn soak_volume(
    per_team: usize,
    win: u32,
    tdm_tickets: u32,
    timed_s: Option<u32>,
    limit_s: u32,
    seed: u64,
    tuning: Tuning,
    volume: HitVolume,
) -> Outcome {
    let world = soak_world();
    let teams: Vec<u8> = (0..2 * per_team).map(|p| u8::from(p >= per_team)).collect();
    let driven = vec![true; teams.len()];
    let roles: Vec<Option<&str>> = vec![None; teams.len()];
    let mode = if timed_s.is_some() {
        survarium::v026g::maps::mode::TIMED_BATTERIES
    } else if win > 0 {
        survarium::v026g::maps::mode::BATTERIES
    } else {
        survarium::v026g::maps::mode::TEAM_DEATHMATCH
    };
    // `SRP_SOAK_TRACE=<dir>`: the decision trace of each run there.
    let trace = std::env::var_os("SRP_SOAK_TRACE").map_or(Trace::none(0), |dir| {
        Trace::open(std::path::Path::new(&dir), seed as u32, 0, u64::MAX)
    });
    let cheat = if tuning.cheats.batteries { "-cheat" } else { "" };
    let mut bots = Bots::new(
        0x50AC + seed as u32,
        mode,
        0,
        &teams,
        &driven,
        &roles,
        Arc::new(tuning),
        trace,
    );
    bots.map_ready(world.clone());
    let tickets = (win == 0).then_some(tdm_tickets);
    let mut game = Game::new(world, &teams, seed, tickets, timed_s.map(|s| s * 1000));
    game.volume = volume;
    for place in 0..teams.len() {
        bots.entered(place, true, 0);
    }
    let mut out = Outcome {
        carry: CarryTally::new(format!("soak{per_team}-s{seed}{cheat}")),
        ..Outcome::default()
    };
    let rays_before = super::aimpoint::rays_cast();
    let limit_ms = limit_s * 1000;
    let mut raids = RaidTally::default();
    let (mut clock, mut next_world) = (0, 1_000);
    while clock < limit_ms {
        clock += 25;
        while game.tick + TICK_MS <= clock {
            let before = game.stats.clone();
            game.step();
            out.carry.deaths(&before, &game, &bots);
        }
        let thieves = std::mem::take(&mut game.thieves);
        raids.bodies(&game, &thieves, clock);
        if clock >= next_world {
            next_world += 1_000;
            if let Some((_, fixes, health, facts)) = game.world_at(clock / TICK_MS * TICK_MS - 300)
            {
                for (place, (fix, health)) in fixes.into_iter().zip(health).enumerate() {
                    let notes = bots.fixed(place, fix, Some(health));
                    notes.iter().for_each(|note| out.decisions.note(note));
                    notes.iter().for_each(|note| out.carry.note(note));
                    out.notes.extend(notes);
                }
                let notes = bots.facts(facts);
                notes.iter().for_each(|note| out.decisions.note(note));
                notes.iter().for_each(|note| out.carry.note(note));
                out.notes.extend(notes);
                out.carry.world(&game, &bots);
            }
        }
        let (made, notes) = bots.poll(Clock {
            clock,
            time_left_s: (limit_ms - clock) as f32 / 1000.0,
            total_s: limit_s as f32,
            playing: true,
        });
        notes.iter().for_each(|note| out.decisions.note(note));
        notes.iter().for_each(|note| out.carry.note(note));
        raids.polled(&bots, &notes, &made);
        out.notes.extend(notes);
        for made in made {
            out.decisions.made(&made);
            // Every input keeps the rules: on the grid, ahead, in order,
            // no forbidden bit, a turn under π.
            assert!(made.time > clock && made.time <= clock + 100);
            assert_eq!(made.input.actions & key::NEVER, 0);
            assert!(made.input.rotation_is_sane());
            let queue = &mut game.inputs[made.place];
            assert!(queue.back().is_none_or(|last| last.0 < made.time));
            let r = made.input.rotation;
            let turn = (
                half_value(u16::from_le_bytes([r[0], r[1]])),
                half_value(u16::from_le_bytes([r[2], r[3]])),
            );
            assert!(turn.0.abs() < std::f32::consts::PI && turn.1.abs() < std::f32::consts::PI);
            let bot = bots.bots[made.place].as_ref();
            let target = bot.and_then(|bot| bot.reflex.target());
            let gun = bot.map_or(Arms::fallback().primary.0, |bot| bot.reflex.held_gun());
            queue.push_back((made.time, made.input.actions, turn, target, gun));
        }
        if win > 0
            && let Some(team) = (0..2u8).find(|&t| game.points[usize::from(t)] >= win)
        {
            out.winner = Some(team);
            break;
        }
        // Team Deathmatch: a team out of tickets with no one alive loses.
        if let Some(tickets) = game.tickets
            && let Some(loser) = (0..2u8).find(|&t| {
                tickets[usize::from(t)] == 0 && !game.bodies.iter().any(|b| b.team == t && b.alive)
            })
        {
            out.winner = Some(1 - loser);
            break;
        }
    }
    out.end_s = clock as f32 / 1000.0;
    out.raids = raids.finish();
    for (place, &team) in teams.iter().enumerate() {
        out.kills[usize::from(team)] += game.stats[place].0;
    }
    // Time up: the mode's leader wins (batteries home, else kills).
    if out.winner.is_none() {
        let (a, b) = if win > 0 {
            (game.points[0], game.points[1])
        } else {
            (out.kills[0], out.kills[1])
        };
        out.winner = match a.cmp(&b) {
            std::cmp::Ordering::Greater => Some(0),
            std::cmp::Ordering::Less => Some(1),
            std::cmp::Ordering::Equal => None,
        };
    }
    out.delivered = game.delivered;
    out.stolen = game.stolen;
    out.taken = game.taken;
    out.taken_killed = game.taken_killed;
    out.shots = game.shots;
    out.hits = game.hits;
    out.team_hits = game.team_hits;
    out.team_kills = game.team_kills;
    out.walled = game.walled;
    out.hidden = game.hidden;
    out.rays_per_bot_s = (super::aimpoint::rays_cast() - rays_before - game.metric_rays) as f32
        / teams.len() as f32
        / out.end_s.max(1.0);
    out.least_moved_m = game
        .bodies
        .iter()
        .map(|b| b.moved)
        .fold(f32::INFINITY, f32::min);
    out.stuck_notes = out
        .notes
        .iter()
        .filter(|n| n.text.contains("stuck"))
        .count();
    if std::env::var_os("SRP_BOT_NOTES").is_some() {
        for note in &out.notes {
            eprintln!(
                "{:>7} ms  bot {:>2}: {}",
                note.time, note.place as i64, note.text
            );
        }
        eprintln!(
            "winner {:?} at {} s; kills {:?}; delivered {:?}; stolen {}; hits {} of {} shots, {} into a wall short of the target, {} at a hidden target; {:.1} sight and aim rays per bot a second; least moved {:.0} m; stuck notes {}",
            out.winner,
            out.end_s,
            out.kills,
            out.delivered,
            out.stolen,
            out.hits,
            out.shots,
            out.walled,
            out.hidden,
            out.rays_per_bot_s,
            out.least_moved_m,
            out.stuck_notes
        );
    }
    out
}

/// [smoke review S1] `aim.combat.aim_last_known` over many soaks: the fallback point's use (aims at a target with no
/// part seen, and how far the remembered place is from his true chest) and the outcome, paired by seed, with the disc
/// and with the hit boxes. `cargo test -p match-server --release aim_last_known_soaks -- --ignored --nocapture`.
#[test]
#[ignore]
fn aim_last_known_soaks() {
    for volume in [HitVolume::Disc, HitVolume::ON] {
        let mut sums = [[0u64; 4]; 2];
        for seed in 1..=12u64 {
            for (arm, on) in [false, true].into_iter().enumerate() {
                let mut tuning = bench_tuning();
                tuning.aim.combat.aim_last_known = on;
                let before = super::aimpoint::fallback_aims();
                let out = soak_volume(4, 0, 12, None, 120, seed, tuning, volume);
                let after = super::aimpoint::fallback_aims();
                let (n, metres) = (after.0 - before.0, after.1 - before.1);
                sums[arm][0] += u64::from(out.shots);
                sums[arm][1] += u64::from(out.hits);
                sums[arm][2] += n;
                sums[arm][3] += (metres * 10.0) as u64;
                println!(
                    "{volume:?} seed {seed} on {on}: shots {} hits {} kills {:?} walled {} hidden {}; fallback aims {n}, mean {:.1} m off his chest",
                    out.shots, out.hits, out.kills, out.walled, out.hidden, if n > 0 { metres / n as f64 } else { 0.0 }
                );
            }
        }
        for (arm, s) in sums.iter().enumerate() {
            println!(
                "{volume:?} on {}: shots {} hits {} rate {:.3}; fallback aims {} mean {:.1} m",
                arm == 1, s[0], s[1], s[1] as f64 / s[0].max(1) as f64, s[2], s[3] as f64 / 10.0 / s[2].max(1) as f64
            );
        }
    }
}
