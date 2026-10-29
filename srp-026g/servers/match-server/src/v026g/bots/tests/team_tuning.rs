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

/// The plays' reads of the hazard switches (`Ctx::tuning`, `plays/collect.rs`): a battery lies in a zone so wide that no straight
/// walk crosses to it, which only the motor's own route over the zone offers (`route_crossing` `total`). Team 0's switch is on,
/// team 1's off, so only team 0's bots go for the battery: the same situation, two answers by tuning. Swapped, the answers
/// swap.
#[test]
fn the_crossing_rule_is_read_from_the_tuning_of_the_bots_team() {
    use mapdata::{Damage, Rotation, Shape, ShapeKind, Zone, ZoneKind};
    use crate::v026g::bots::tuning::RouteCrossing;
    let at = Vec3::new(25.0, 0.0, 32.0);
    let goes_for_it = |route0: RouteCrossing, route1: RouteCrossing| -> [usize; 2] {
        let world = super::tactics::tests::field_with(|map| {
            for (id, team, x) in [(0, 0, 12.0), (1, 1, 68.0)] {
                let position = Vec3::new(x, 0.0, 30.0);
                map.containers.push(mapdata::Container {
                    position,
                    team,
                    id,
                    use_shapes: vec![Shape {
                        kind: ShapeKind::Box,
                        center: position + Vec3::new(0.0, 1.0, 0.0),
                        rotation: Rotation::IDENTITY,
                        half: Vec3::new(1.0, 0.85, 0.65),
                    }],
                });
            }
            map.battery_spawners = vec![mapdata::BatterySpawner { position: at, item: at, layer: "default".into() }];
            map.zones.push(Zone {
                kind: ZoneKind::Damage,
                name: "toxic".into(),
                team: -1,
                position: Vec3::new(25.0, 0.0, 32.0),
                damage_type: "toxic".into(),
                damage: Some(Damage { max_hit: 0.01, interval_ms: 1000, parts: 2, ..Damage::default() }),
                shapes: vec![Shape {
                    kind: ShapeKind::Sphere,
                    center: Vec3::new(25.0, 1.0, 32.0),
                    rotation: Rotation::IDENTITY,
                    half: Vec3::new(26.0, 26.0, 26.0),
                }],
            });
        });
        let (mut team0, mut team1) = (Tuning::default(), Tuning::default());
        // The HUD shows the lying battery (the cheat off).
        team0.cheats.batteries = false;
        team1.cheats.batteries = false;
        team0.hazard.route_crossing = route0;
        team1.hazard.route_crossing = route1;
        let teams = [0u8, 0, 1, 1];
        let mut bots = Bots::new_with(
            0x7EA6,
            survarium::v026g::maps::mode::BATTERIES,
            0,
            &teams,
            &[true; 4],
            &[None; 4],
            tunings(team0, team1),
            Trace::none(0),
        );
        bots.map_ready(world);
        let spots = [(10.0, 10.0), (10.0, 14.0), (10.0, 26.0), (10.0, 30.0)];
        for place in 0..4 {
            bots.entered(place, true, 0);
        }
        let mut count = [0usize; 2];
        for clock in (100..=20_000u32).step_by(100) {
            if clock % 1_000 == 0 {
                for (place, (x, z)) in spots.into_iter().enumerate() {
                    let fix = Fix {
                        time: clock,
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
                bots.facts(WorldFacts { time: clock, batteries: vec![BatteryState::Free(at)], marks: vec![0], ..WorldFacts::default() });
            }
            bots.poll(Clock { clock, time_left_s: 600.0, total_s: 900.0, playing: true });
        }
        for team in 0..2 {
            eprintln!("team {team} knows battery 0: {:?}", bots.teams[team].knowledge.battery(0));
        }
        if let Some(t) = bots.tactics.as_ref() {
            let stand = t.stand_to_take(at);
            let grid = &t.world.grid;
            eprintln!(
                "stand {stand:?} zones at stand {:?} past fixed {:?}",
                stand.and_then(|s| grid.locate(s, 3.0)).map(|n| t.hazards.zones_at(n).count()),
                stand.and_then(|s| grid.locate(s, 3.0)).map(|n| t.past_fixed_zones(n)),
            );
        }
        for (place, team) in teams.iter().enumerate() {
            let collects = bots.bots[place]
                .as_ref()
                .and_then(|bot| bot.assignment.as_ref())
                .is_some_and(|a| a.kind == PlayKind::Collect);
            count[usize::from(*team)] += usize::from(collects);
            eprintln!(
                "place {place} team {team}: {:?}",
                bots.bots[place].as_ref().and_then(|bot| bot.assignment.as_ref()).map(|a| (a.kind, a.target, a.reason))
            );
        }
        count
    };
    let [t0, t1] = goes_for_it(RouteCrossing::Total, RouteCrossing::Off);
    assert!(t0 > 0 && t1 == 0, "team 0 (routes) {t0} collect, team 1 (off) {t1}");
    let [t0, t1] = goes_for_it(RouteCrossing::Off, RouteCrossing::Total);
    assert!(t0 == 0 && t1 > 0, "swapped: team 0 (off) {t0} collect, team 1 (routes) {t1}");
}
