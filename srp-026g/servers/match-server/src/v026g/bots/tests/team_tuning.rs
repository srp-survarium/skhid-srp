//! A one-sided run (`SRP_BENCH_ONE_SIDE`, the head-to-head benches) gives team 1 another tuning. Every decision of a team reads
//! its own team's ([`TeamTunings::of`]): a read of team 0's for team 1's bots would mix the rules of the two sides without a
//! sound. These tests give team 1 a setting that team 0 does not have, at each place that reads a tuning, and look for it
//! in team 1's results only. The plays and the reflex take the `&Tuning` that the think and the commander hand them
//! (`Ctx::tuning`, `Reflex::think_with`), so the think and the commander are their inputs here.

use super::*;

use crate::v026g::bots::tuning::TeamTunings;

fn tunings(team0: Tuning, team1: Tuning) -> TeamTunings {
    let mut tunings = TeamTunings::new(Arc::new(team0));
    tunings.give_team1(Arc::new(team1));
    tunings
}

/// Two bots a side on the soak field, in play, each reckoned alive by the bots' own tracker, with one poll made.
fn after_one_poll(team0: Tuning, team1: Tuning) -> (Bots, u32) {
    let teams = [0u8, 0, 1, 1];
    let mut bots = Bots::new_with(
        0x7EA3,
        survarium::v026g::maps::mode::TEAM_DEATHMATCH,
        0,
        &teams,
        &[true; 4],
        &[None; 4],
        tunings(team0, team1),
        Trace::none(0),
    );
    bots.map_ready(soak_world());
    let spots = [(10.0, 30.0), (14.0, 30.0), (70.0, 30.0), (72.0, 34.0)];
    for (place, (x, z)) in spots.into_iter().enumerate() {
        bots.entered(place, true, 0);
        let fix = Fix {
            time: 1_000,
            alive: true,
            inserted: 100,
            feet: Some(Vec3::new(x, 0.0, z)),
            yaw: 0.0,
            pitch: 0.0,
            keys: 0,
            speed: Some(0.0),
        };
        bots.fixed(place, fix, Some(1.0));
    }
    let now = 1_100;
    bots.poll(Clock { clock: now, time_left_s: 600.0, total_s: 900.0, playing: true });
    (bots, now)
}

/// The commander's pass, the bots' think and the drive each run on their own team's tuning.
#[test]
fn the_commander_the_think_and_the_drive_use_their_teams_tuning() {
    let mut team0 = Tuning::default();
    team0.think.commander_ms = 2_000;
    team0.think.think_ms = (100, 100);
    team0.think.stamina = false;
    let mut team1 = team0.clone();
    team1.think.commander_ms = 9_000;
    team1.think.think_ms = (700, 700);
    team1.think.stamina = true;
    let (bots, now) = after_one_poll(team0, team1);
    // The commander (`commander.rs`): when its team's next pass is.
    assert_eq!(bots.teams[0].next_commander, now + 2_000);
    assert_eq!(bots.teams[1].next_commander, now + 9_000);
    // The think (`think::schedule`): when each bot's next think is.
    for place in [0, 1] {
        assert_eq!(bots.bots[place].as_ref().unwrap().next_think, now + 100, "team 0 bot {place}");
    }
    for place in [2, 3] {
        assert_eq!(bots.bots[place].as_ref().unwrap().next_think, now + 700, "team 1 bot {place}");
    }
    // The drive (`drive_tick`, from `START_AFTER_MS` past the bot's entry on): the stamina gauge is the team's switch.
    let mut bots = bots;
    for clock in (now + 100..=2_900).step_by(100) {
        bots.poll(Clock { clock, time_left_s: 600.0, total_s: 900.0, playing: true });
    }
    for place in [0, 1] {
        assert!(bots.bots[place].as_ref().unwrap().motor.gauge.is_none(), "team 0 bot {place} has no stamina");
    }
    for place in [2, 3] {
        assert!(bots.bots[place].as_ref().unwrap().motor.gauge.is_some(), "team 1 bot {place} has stamina");
    }
}

/// The bots of a team are made of its tuning: the skill a tier starts at, and the motor's feel (`Bots::new_with`).
#[test]
fn the_bots_of_team_1_are_made_of_team_1s_tuning() {
    let mut team1 = Tuning::default();
    team1.feel.lip_guard_ticks += 3;
    let lip = team1.feel.lip_guard_ticks;
    let teams = [0u8, 1];
    let bots = Bots::new_with(
        0x7EA4,
        survarium::v026g::maps::mode::TEAM_DEATHMATCH,
        0,
        &teams,
        &[true; 2],
        &[None; 2],
        tunings(Tuning::default(), team1),
        Trace::none(0),
    );
    assert_ne!(bots.bots[0].as_ref().unwrap().motor.feel.lip_guard_ticks, lip);
    assert_eq!(bots.bots[1].as_ref().unwrap().motor.feel.lip_guard_ticks, lip);
}

/// The facts of a world (`facts.rs`): a team whose battery cheat is on has no HUD of the batteries. Team 1's dropped battery is
/// on team 1's HUD (as in `a_dropped_battery_put_back_after_a_fall_stays_on_its_teams_hud`) when team 1's cheat is off, and not
/// when it is on, whatever team 0's is.
#[test]
fn the_facts_read_the_battery_cheat_of_the_team_they_are_for() {
    let hud_of_team_1 = |cheat0: bool, cheat1: bool| {
        let (mut team0, mut team1) = (Tuning::default(), Tuning::default());
        team0.cheats.batteries = cheat0;
        team1.cheats.batteries = cheat1;
        let mut bots = Bots::new_with(
            0x7EA5,
            survarium::v026g::maps::mode::BATTERIES,
            0,
            &[0, 1],
            &[true, true],
            &[None; 2],
            tunings(team0, team1),
            Trace::none(0),
        );
        for place in 0..2 {
            bots.entered(place, true, 100);
        }
        let lying = |time: u32, at: Vec3, mask: u32| WorldFacts {
            time,
            batteries: vec![BatteryState::Free(at)],
            marks: vec![mask],
            ..WorldFacts::default()
        };
        bots.facts(lying(1_000, Vec3::new(-14.0, 21.7, -61.4), 0));
        bots.facts(WorldFacts { time: 2_000, batteries: vec![BatteryState::Carried(None)], marks: vec![0], ..WorldFacts::default() });
        bots.facts(lying(3_000, Vec3::new(-43.58, 19.55, 3.80), 2));
        bots.teams[1].knowledge.battery(0).is_some_and(|k| k.hud)
    };
    assert!(hud_of_team_1(true, false), "team 1's cheat is off: its HUD shows the battery");
    assert!(!hud_of_team_1(false, true), "team 1's cheat is on: no HUD, whatever team 0's is");
}

/// The cover rule (`cover.pierce`) is a team's: team 1 tests cover as bullets (a crate is no cover from them) when team 1's
/// tuning says so, whatever team 0's is, and the other way round. The rule reaches the plays and the reflex through
/// `Bots::think`'s one `tuning_of(team)` (`Due::tuning`, `Situation::cover`), the same place the probes above exercise; no
/// play of the soak field asks a cover question, so the rule is read where a think takes it from.
#[test]
fn the_cover_rule_is_the_rule_of_the_team_that_asks() {
    let rules = |team0_pierce: bool, team1_pierce: bool| {
        let (mut team0, mut team1) = (Tuning::default(), Tuning::default());
        if !team0_pierce {
            team0.cover.pierce = None;
        }
        if !team1_pierce {
            team1.cover.pierce = None;
        }
        let (bots, _) = after_one_poll(team0, team1);
        [0u8, 1].map(|team| bots.tuning_of(team).cover.rule().is_some())
    };
    assert_eq!(rules(false, true), [false, true], "team 1 pierce-aware, team 0 solid");
    assert_eq!(rules(true, false), [true, false], "team 1 solid, team 0 pierce-aware");
}

/// Two bots a side, 30 m apart across x = 35 on the soak field, each facing the other team, in play.
fn facing_bots(team0: Tuning, team1: Tuning, seed: u32) -> Bots {
    use crate::v026g::bots::kin::yaw_of;
    let teams = [0u8, 0, 1, 1];
    let mut bots = Bots::new_with(
        seed,
        survarium::v026g::maps::mode::TEAM_DEATHMATCH,
        0,
        &teams,
        &[true; 4],
        &[None; 4],
        tunings(team0, team1),
        Trace::none(0),
    );
    bots.map_ready(soak_world());
    let spots = [(20.0, 28.0), (20.0, 32.0), (50.0, 28.0), (50.0, 32.0)];
    for (place, (x, z)) in spots.into_iter().enumerate() {
        bots.entered(place, true, 0);
        let yaw = yaw_of([if teams[place] == 0 { 1.0 } else { -1.0 }, 0.0]);
        let fix = Fix {
            time: 1_000,
            alive: true,
            inserted: 100,
            feet: Some(Vec3::new(x, 0.0, z)),
            yaw,
            pitch: 0.0,
            keys: 0,
            speed: Some(0.0),
        };
        bots.fixed(place, fix, Some(1.0));
    }
    bots
}

/// A cloud across the middle (35, 30) poured at `pours`: it hides from `pours` + 0.7 s.
fn across_middle(pours: u32) -> crate::v026g::bots::smoke::Cloud {
    use crate::v026g::bots::smoke::{Cloud, MatchMs, SmokeGrenade};
    Cloud {
        by: 2,
        team: 1,
        placed: crate::v026g::bots::smoke::Placed::Arc,
        centre: Vec3::new(35.0, 0.0, 30.0),
        thrown: MatchMs(pours.saturating_sub(4_000)),
        pours: MatchMs(pours),
        shape: SmokeGrenade::RDG_M.cloud,
    }
}

/// Smoke (`smoke.sight`): team 0 reads the clouds, team 1 does not. A cloud between the teams, two bots a side facing
/// each other 30 m apart on the soak field, polled through the real think for 3 s: team 0's bots never see team 1's,
/// team 1's see team 0's through it.
#[test]
fn a_cloud_blocks_the_sight_of_the_team_whose_tuning_reads_smoke_only() {
    let team0 = Tuning::default();
    let mut team1 = Tuning::default();
    team1.smoke.sight = false;
    let mut bots = facing_bots(team0, team1, 0x5E0C);
    bots.tracker.add_cloud(across_middle(0));
    for clock in (1_100..=4_000).step_by(100) {
        bots.poll(Clock { clock, time_left_s: 600.0, total_s: 900.0, playing: true });
    }
    let seen = |place: usize| {
        let bot = bots.bots[place].as_ref().unwrap();
        bot.memory.all_seen(4_000, 4_000).iter().map(|t| t.who()).collect::<Vec<_>>()
    };
    for place in [0, 1] {
        assert!(seen(place).is_empty(), "team 0 bot {place} saw {:?} through the smoke", seen(place));
    }
    for place in [2, 3] {
        assert!(!seen(place).is_empty(), "team 1 bot {place} (smoke not read) saw nobody");
    }
}

/// The smoke counts are wired (`smoke::tally`, read by the `smoke` owner topic): both teams read smoke; they see each
/// other and fire for 2 s in the open, then a cloud pours between them. Each team's bots lose their live sights to it
/// (a look), the aim's target (its look), sights they never begin (the other enemy), and spray at the last point.
#[test]
fn the_smoke_counts_fire_when_a_cloud_comes_between_two_fighting_teams() {
    use crate::v026g::bots::smoke::tally::{self, Event};
    let before = tally::counts();
    let mut bots = facing_bots(Tuning::default(), Tuning::default(), 0x5E0D);
    for clock in (1_100..=3_000).step_by(25) {
        bots.poll(Clock { clock, time_left_s: 600.0, total_s: 900.0, playing: true });
    }
    bots.tracker.add_cloud(across_middle(2_400));
    for clock in (3_025..=5_000).step_by(25) {
        bots.poll(Clock { clock, time_left_s: 600.0, total_s: 900.0, playing: true });
    }
    let after = tally::counts();
    let got = |team: usize, e: Event| after[team][e as usize] - before[team][e as usize];
    for team in [0, 1] {
        let counts: Vec<(Event, u64)> = [Event::SightLost, Event::NotSeen, Event::AimLost, Event::Spray]
            .into_iter()
            .map(|e| (e, got(team, e)))
            .collect();
        println!("team {team}: {counts:?}");
        assert!(counts.iter().all(|&(_, n)| n > 0), "team {team}: {counts:?}");
    }
}
