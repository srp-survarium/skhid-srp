use super::*;

/// A short run every build: two minutes of 4 against 4, fights and no
/// input out of the rules.
#[test]
pub(crate) fn a_short_soak_has_fights_and_keeps_the_input_rules() {
    let out = soak(4, 3, 0, 120, 3);
    assert!(
        out.kills[0] + out.kills[1] >= 2,
        "{:?} ({} hits of {} shots)",
        out.kills,
        out.hits,
        out.shots
    );
    assert!(out.least_moved_m > 20.0, "{}", out.least_moved_m);
    // Never through the world (`aimpoint`): before it, 48 of 416 shots
    // went into a wall short of the target or at one of whom nothing
    // showed; the few left are a tick's turn or step.
    // (With the sight refresh on, `sight.rs` runs the same limit: at the default grace, 250 ms, seed 3
    // gives 2 of 500 shots, 16 seeds 43 of 11,189 against 67 of 11,121 without it. At a grace of 150 ms
    // seed 3 gives 16 of 460, over this limit.)
    // The limit is over seeds 1 to 4 together: one encounter at a corner (a burst of seven shots at a target that ducked out of sight, within the
    // sight hold) is 12 of seed 3's 336 shots with the scopes' zoom and the aim's settling on (A182 #29), and the same limit over 16 seeds reads 0.41 %
    // of 10,152 shots with them against 0.57 % of 10,799 without, so a single seed is a chance of one encounter.
    let (mut walled, mut hidden, mut shots) = (out.walled, out.hidden, out.shots);
    for seed in [1, 2, 4] {
        let more = soak(4, 3, 0, 120, seed);
        (walled, hidden, shots) = (walled + more.walled, hidden + more.hidden, shots + more.shots);
    }
    assert!(
        walled + hidden <= shots / 33,
        "{walled} into a wall, {hidden} at a hidden target, of {shots} shots (seeds 1 to 4)"
    );
}

/// The short run with a tiny search budget: nearly every route search goes
/// on over several polls ([`Searching`]), its bot walking the way so far,
/// and the bots still move, fight and keep the input rules.
#[test]
pub(crate) fn a_short_soak_with_its_searches_in_slices_still_plays() {
    let before = super::path::searches();
    let mut tuning = Tuning::default();
    tuning.think.search_floors_per_poll = 300;
    let out = soak_tuned(4, 3, 0, None, 120, 3, tuning);
    let after = super::path::searches();
    assert!(
        after.sliced - before.sliced >= 50,
        "{} of {} searches sliced",
        after.sliced - before.sliced,
        after.searches - before.searches
    );
    assert!(
        out.kills[0] + out.kills[1] >= 2,
        "{:?} ({} hits of {} shots)",
        out.kills,
        out.hits,
        out.shots
    );
    assert!(out.least_moved_m > 20.0, "{}", out.least_moved_m);
    // Its bots are gone, and with them this thread's search arrays.
    assert_eq!(mapdata::search_memory_bytes(), 0);
}

/// Through the start countdown the game holds the players where they
/// stand: the bots rest (their inputs keep the clock going, with no key
/// and no turn) and go only when play begins.
#[test]
pub(crate) fn the_bots_rest_through_the_start_countdown() {
    let world = soak_world();
    let teams: Vec<u8> = (0..4).map(|p| u8::from(p >= 2)).collect();
    let mut bots = Bots::new(
        0x50AC,
        survarium::v026g::maps::mode::BATTERIES,
        0,
        &teams,
        &[true; 4],
        &[None; 4],
        Arc::new(Tuning::default()),
        Trace::none(0),
    );
    bots.map_ready(world.clone());
    let mut game = Game::new(world, &teams, 1, None, None);
    (0..teams.len()).for_each(|place| bots.entered(place, true, 0));
    let start: Vec<Vec3> = game.bodies.iter().map(|body| body.feet).collect();
    let play_from = 15_000;
    let (mut clock, mut next_world) = (0, 1_000);
    let mut going = 0;
    while clock < play_from + 10_000 {
        clock += 25;
        while game.tick + TICK_MS <= clock {
            game.step();
        }
        if clock >= next_world {
            next_world += 1_000;
            if let Some((_, fixes, health, facts)) = game.world_at(clock / TICK_MS * TICK_MS - 300)
            {
                for (place, (fix, health)) in fixes.into_iter().zip(health).enumerate() {
                    bots.fixed(place, fix, Some(health));
                }
                bots.facts(facts);
            }
        }
        let playing = clock >= play_from;
        if clock == play_from {
            for (body, start) in game.bodies.iter().zip(&start) {
                assert!(
                    body.feet.distance_xz(*start) < 0.1,
                    "{:?} from {start:?}",
                    body.feet
                );
            }
        }
        let (made, _) = bots.poll(Clock {
            clock,
            time_left_s: 600.0,
            total_s: 600.0,
            playing,
        });
        for made in made {
            let moves =
                made.input.actions & (key::FORWARD | key::BACK | key::LEFT | key::RIGHT) != 0;
            if playing {
                going += usize::from(moves);
            } else {
                assert_eq!(made.input.actions, 0, "{made:?}");
                assert!(made.input.rotation.iter().all(|&b| b == 0), "{made:?}");
            }
            let r = made.input.rotation;
            let turn = (
                half_value(u16::from_le_bytes([r[0], r[1]])),
                half_value(u16::from_le_bytes([r[2], r[3]])),
            );
            game.inputs[made.place].push_back((
                made.time,
                made.input.actions,
                turn,
                None,
                Arms::fallback().primary.0,
            ));
        }
    }
    assert!(going > 100, "{going} inputs that move after the start");
}

/// The whole match (minutes of play: `cargo test --release -- --ignored`).
#[test]
#[ignore = "a whole match; run in release"]
pub(crate) fn four_bots_a_side_fight_and_bring_batteries_to_a_win() {
    let out = soak(4, 3, 0, 900, 1);
    eprintln!("{}", out.decisions.line());
    let total = out.kills[0] + out.kills[1];
    assert!(
        total >= 4,
        "they fought: {:?} kills, {} hits of {} shots",
        out.kills,
        out.hits,
        out.shots
    );
    assert!(
        out.delivered[0] + out.delivered[1] >= 1,
        "batteries brought: {:?}",
        out.delivered
    );
    assert!(
        out.least_moved_m > 30.0,
        "every bot moved: {}",
        out.least_moved_m
    );
    assert!(out.winner.is_some(), "a winner in {} s: {out:?}", out.end_s);
}

#[test]
#[ignore = "a whole match; run in release"]
pub(crate) fn a_team_deathmatch_of_bots_is_fought_to_the_last_ticket() {
    // 25 tickets a side: the endgame (no respawns, the hunt) comes.
    let out = soak(4, 0, 25, 900, 2);
    eprintln!("{}", out.decisions.line());
    assert!(
        out.winner.is_some(),
        "a winner in {} s: {:?}",
        out.end_s,
        out.kills
    );
    assert!(out.end_s < 899.0, "ended by the tickets: {} s", out.end_s);
    assert!(
        out.kills[0] > 0 && out.kills[1] > 0,
        "{:?} ({} hits, {} shots)",
        out.kills,
        out.hits,
        out.shots
    );
    assert!(out.least_moved_m > 20.0, "{}", out.least_moved_m);
}

/// No shot through a teammate (`friends`): Team Deathmatch matches of 6 a
/// side with the check off and on, the same seeds. Off, a bot fires
/// through a teammate as it did live (2026-10-20..23: 7,250 team kills to
/// 27,088 kills); on, far fewer team hits, and the enemy still dies about
/// as often.
/// A take waits for the fight while an enemy sees the taker, unless he
/// reloads or a teammate covers ([`Ctx::exposed_to`]): the battery soaks
/// (4 a side to 3, seeds 1-8, the battery cheat off and on) with the wait
/// off (`exposed_m` 0) and on. Prints the batteries taken and brought,
/// the carriers killed within [`TAKEN_KILLED_MS`] of their take, and the
/// wins; batteries are still brought with it on.
///
/// [`Ctx::exposed_to`]: super::plays::Ctx::exposed_to
#[test]
#[ignore = "whole matches; run in release"]
pub(crate) fn a_take_waits_for_the_fight_unless_covered() {
    let mut totals = Vec::new();
    for on in [false, true] {
        let (mut taken, mut killed, mut brought, mut won, mut runs) = (0, 0, 0, 0, 0);
        let mut draws = Vec::new();
        for cheat in [false, true] {
            for seed in 1..=16 {
                let mut tuning = Tuning::default();
                tuning.cheats.batteries = cheat;
                if !on {
                    let mut play = super::plays::default_tuning(PlayKind::Collect);
                    play.numbers.insert("exposed_m".into(), 0.0);
                    tuning.plays.insert(PlayKind::Collect, play);
                }
                let out = soak_tuned(4, 3, 0, None, 900, seed, tuning);
                taken += out.taken;
                killed += out.taken_killed;
                brought += out.delivered[0] + out.delivered[1];
                won += u32::from(out.delivered.iter().any(|&d| d >= 3));
                if out.winner.is_none() {
                    draws.push((cheat, seed));
                }
                runs += 1;
            }
        }
        eprintln!(
            "take waits {on}: taken {taken}, killed within 3 s of a take {killed}, brought {brought}, won by batteries {won} of {runs}; draws (cheat, seed) {draws:?}"
        );
        totals.push((taken, killed, brought));
    }
    let (_, _, brought_on) = totals[1];
    let (_, _, brought_off) = totals[0];
    assert!(
        brought_on * 10 >= brought_off * 8,
        "batteries still brought: {brought_on} against {brought_off}"
    );
}

#[test]
#[ignore = "whole matches; run in release"]
pub(crate) fn bots_hold_fire_while_a_teammate_is_in_the_line_of_fire() {
    let mut totals = Vec::new();
    for on in [false, true] {
        let mut tuning = bench_tuning();
        tuning.aim.friends.on = on;
        let (mut kills, mut team_kills, mut hits, mut team_hits, mut shots) = (0, 0, 0, 0, 0);
        for seed in 1..=6 {
            let out = soak_tuned(6, 0, 60, None, 600, seed, tuning.clone());
            kills += out.kills[0] + out.kills[1];
            team_kills += out.team_kills;
            hits += out.hits;
            team_hits += out.team_hits;
            shots += out.shots;
        }
        eprintln!(
            "friends.on {on}: kills {kills}, team kills {team_kills}; hits {hits}, team hits {team_hits} of {shots} shots"
        );
        totals.push((kills, team_kills, hits, team_hits));
    }
    let [
        (kills_off, _, _, team_hits_off),
        (kills_on, _, _, team_hits_on),
    ] = totals[..]
    else {
        unreachable!()
    };
    assert!(
        team_hits_off >= 20,
        "the harness sees friendly fire with the check off: {team_hits_off}"
    );
    assert!(
        team_hits_on * 4 <= team_hits_off,
        "a quarter of the team hits at most: {team_hits_on} against {team_hits_off}"
    );
    assert!(
        kills_on * 10 >= kills_off * 8,
        "the enemy dies about as often: {kills_on} kills against {kills_off}"
    );
}

/// The raids (play 24) of whole battery matches, the battery cheat off
/// and on, `SRP_RAID_SEEDS` seeds each (default 8): how many are taken,
/// how long they run, how near their container they come, the uses, the
/// steals, and the matches' deliveries and winners. A measurement: it
/// asserts nothing.
#[test]
#[ignore = "whole matches; run in release"]
pub(crate) fn raids_in_whole_battery_matches() {
    let seeds: u64 = std::env::var("SRP_RAID_SEEDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(8);
    for cheat in [false, true] {
        let mut tuning = bench_tuning();
        tuning.cheats.batteries = cheat;
        let mut runs: Vec<RaidRun> = Vec::new();
        let (mut stolen, mut delivered, mut wins, mut decided) = (0, 0, [0u32; 2], 0);
        for seed in 1..=seeds {
            let out = soak_tuned(4, 3, 0, None, 900, seed, tuning.clone());
            stolen += out.stolen;
            delivered += out.delivered[0] + out.delivered[1];
            if let Some(team) = out.winner {
                wins[usize::from(team)] += 1;
                decided += u32::from(out.end_s < 899.0);
            }
            eprintln!(
                "  cheat {cheat} seed {seed}: winner {:?} at {:.0} s, delivered {:?}, stolen {}, raids {}, raid steals {}; kills {:?}; {}",
                out.winner,
                out.end_s,
                out.delivered,
                out.stolen,
                out.raids.len(),
                out.raids.iter().filter(|r| r.stole).count(),
                out.kills,
                out.carry.line(out.taken)
            );
            runs.extend(out.raids);
        }
        eprintln!(
            "cheat {cheat}: {}; all steals {stolen}, delivered {delivered}, wins {wins:?} ({decided} by the points)",
            raid_line(&runs)
        );
    }
}

/// Timed batteries: they come by the stations over time; bots wait there
/// and bring them home.
#[test]
#[ignore = "a whole match; run in release"]
pub(crate) fn timed_batteries_are_waited_for_at_the_stations_and_brought_home() {
    let out = soak_mode(4, 4, 0, Some(20), 900, 4);
    eprintln!("{}", out.decisions.line());
    assert!(
        out.delivered[0] + out.delivered[1] >= 2,
        "{:?}",
        out.delivered
    );
    assert!(
        out.notes.iter().any(|n| n.text.contains("takes Station")),
        "no station waits"
    );
    assert!(out.winner.is_some());
}

/// With the battery cheat off: the bots look for the batteries at the
/// spawners, find them and bring them home.
#[test]
pub(crate) fn a_short_soak_without_the_battery_cheat_finds_and_brings_batteries() {
    let mut tuning = Tuning::default();
    tuning.cheats.batteries = false;
    let out = soak_tuned(4, 3, 0, None, 150, 3, tuning);
    assert!(
        out.notes
            .iter()
            .any(|n| n.text.contains("takes Collect Spawner")),
        "no spawner looked at"
    );
    assert!(
        out.notes
            .iter()
            .any(|n| n.text.contains("takes Collect Battery")),
        "no battery fetched"
    );
    assert!(
        out.delivered[0] + out.delivered[1] >= 1,
        "batteries brought: {:?}",
        out.delivered
    );
}

/// [`four_bots_a_side_fight_and_bring_batteries_to_a_win`] and
/// [`timed_batteries_are_waited_for_at_the_stations_and_brought_home`]
/// with the battery cheat on (off by default).
#[test]
#[ignore = "whole matches; run in release"]
pub(crate) fn with_the_battery_cheat_whole_matches_are_won_by_batteries() {
    let mut tuning = Tuning::default();
    tuning.cheats.batteries = true;
    // Seed 2: a match of these may end level (1 of 32 with the rule of
    // `a_take_waits_for_the_fight_unless_covered` and without it); seed 1
    // did with it, 1-1 after 900 s.
    let out = soak_tuned(4, 3, 0, None, 900, 2, tuning.clone());
    eprintln!("{}", out.decisions.line());
    assert!(
        out.delivered[0] + out.delivered[1] >= 1,
        "batteries brought: {:?}",
        out.delivered
    );
    assert!(out.winner.is_some(), "a winner in {} s: {out:?}", out.end_s);
    let out = soak_tuned(4, 4, 0, Some(20), 900, 4, tuning);
    eprintln!("{}", out.decisions.line());
    assert!(
        out.delivered[0] + out.delivered[1] >= 2,
        "{:?}",
        out.delivered
    );
    assert!(out.winner.is_some());
}

/// The near margin of the line of fire (`friends::FriendlyFireTuning::
/// near_m`) off and on, `SRP_NEAR_SEEDS` seeds (default 8): whole TDM
/// (6 against 6) and battery (4 against 4) matches, kills and team hits
/// per seed. A measurement: it asserts nothing (friends.rs has the numbers).
#[test]
#[ignore = "whole matches; run in release"]
pub(crate) fn friends_near_margin_off_and_on() {
    let seeds: u64 = std::env::var("SRP_NEAR_SEEDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(8);
    for seed in 1..=seeds {
        for near in [0.0, bench_tuning().aim.friends.near_m] {
            let mut tuning = bench_tuning();
            tuning.aim.friends.near_m = near;
            let tdm = soak_tuned(6, 0, 60, None, 600, seed, tuning.clone());
            let batteries = soak_tuned(4, 3, 0, None, 600, seed, tuning);
            for (mode, out) in [("tdm", tdm), ("batteries", batteries)] {
                eprintln!(
                    "{mode} seed {seed} near_m {near}: kills {}, team kills {}; hits {}, team hits {} of {} shots; winner {:?}",
                    out.kills[0] + out.kills[1],
                    out.team_kills,
                    out.hits,
                    out.team_hits,
                    out.shots,
                    out.winner
                );
            }
        }
    }
}
