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

/// The bench gives the tracker every body's pockets from its profile (`bench_throws`), as the live server does: each
/// is a bot's pockets, and the frags it can throw are the frags its first kit holds, so the bots' frag rolls go by
/// them (`aim.combat.grenade_kit_cap`) and not by an assumed frag.
#[test]
fn the_bench_reads_every_bodys_pockets_from_its_profile() {
    use crate::v026g::bots::live_grenades::Throws;
    use survarium::v026g::loadouts::{BotEquipment, BotTier, RusherKits, bot_equipment_of, bot_kits_kit, grenades_in};
    let teams = [0u8, 0, 0, 0, 1, 1, 1, 1];
    let equipment = BotEquipment::of(BotTier::Mid, RusherKits::default());
    let tunings = tunings(Tuning::default(), Tuning::default());
    let roster = bench_roster(&teams, equipment, &tunings);
    let throws = crate::v026g::bots::tests::bench::bench_throws(&roster, &tunings);
    assert_eq!(throws.len(), teams.len());
    for (place, (throws, profile)) in throws.iter().zip(&roster.profiles).enumerate() {
        assert!(matches!(throws, Throws::Bot(_)), "place {place}: {throws:?}");
        let (equipment, role) = bot_equipment_of(profile).expect("a bot");
        let first = bot_kits_kit(equipment, role, tunings.of(teams[place]).kit.pockets())[0];
        assert_eq!(throws.frags_left(), Some(grenades_in(first) as u16), "place {place} {role:?}");
    }
    let mut bots = facing_bots(Tuning::default(), Tuning::default(), 0x5E10);
    bots.set_throws(crate::v026g::bots::tests::bench::bench_throws(&bench_roster(&[0, 0, 1, 1], equipment, &tunings), &tunings));
    assert!((0..4).all(|place| bots.tracker.frags_left(place).is_some()));
    // The bench's own poll tells each bot's grenade logic what its kit holds, as `Bots::poll` does (it did not:
    // the bench's bots rolled frags their kit did not hold).
    let mut times = crate::v026g::bots::tests::bench::Times::default();
    crate::v026g::bots::tests::bench::timed_poll(&mut bots, Clock { clock: 1_100, time_left_s: 600.0, total_s: 900.0, playing: true }, &mut times);
    for place in 0..4 {
        let bot = bots.bots[place].as_ref().unwrap();
        assert_eq!(bot.reflex.frags_left(), bots.tracker.frags_left(place), "place {place}");
    }
}

/// The kits of a bench team are built with its own tuning's pocket store (`kit.pocket_store`): team 1 on the old store has
/// profiles whose stacks are stored at the count, the stand-in's drugs and the bots' prior follow them, and team 0 keeps the
/// store of 100.
#[test]
fn the_kits_of_team_1_are_built_with_team_1s_pocket_store() {
    use crate::v026g::bots::stock::{Drug, Pantry};
    use survarium::v026g::loadouts::{BotEquipment, BotTier, RusherKits};
    let mut team1 = Tuning::default();
    team1.kit.pocket_store = false;
    let teams = [0u8, 0, 1, 1];
    let equipment = BotEquipment::of(BotTier::Mid, RusherKits::default());
    let roster = bench_roster(&teams, equipment, &tunings(Tuning::default(), team1));
    for (place, &team) in teams.iter().enumerate() {
        let pantry = Pantry::of_profile(&roster.profiles[place]);
        let per_life = team == 0;
        // The medkits of the kit: 2 in the slot, 100 behind it (team 0) or 2 (team 1).
        assert_eq!(pantry.total(Drug::Medkit), if per_life { 100 } else { 2 }, "place {place}");
        let heal = crate::v026g::bots::tests::standin_heal::Heal::of_profile(&roster.profiles[place]);
        assert_eq!(heal.store_of(Drug::Medkit), if per_life { 100 } else { 2 }, "place {place}");
        // The frag the stand-in carries (the support's) has the same store.
        for kits in &roster.grenades[place] {
            assert!(kits.stored == 0 || kits.stored == if per_life { 100 } else { kits.count }, "place {place} {kits:?}");
        }
    }
}

/// What each team's avoidance made of the throw keys of its first bots, with `throws` given to every place: both teams' first
/// bots press the throw key, a mate of each stands where it comes to rest. Team 0's tuning reads the bots' pockets
/// (`avoid.bot_pockets`), team 1's does not.
fn grenade_probe(throws: impl Fn() -> crate::v026g::bots::live_grenades::Throws) -> [[crate::v026g::bots::grenade_threat::AvoidStats; 4]; 2] {
    use crate::v026g::bots::live_grenades::Phase;
    use crate::v026g::drive::key;
    let mut team1 = Tuning::default();
    team1.avoid.bot_pockets = false;
    let (mut bots, mut now) = after_one_poll(Tuning::default(), team1);
    bots.set_throws((0..4).map(|_| throws()).collect());
    let fix = |time: u32, feet: Vec3, keys: u32| Fix {
        time,
        alive: true,
        inserted: 100,
        feet: Some(feet),
        yaw: 0.0,
        pitch: 0.0,
        keys,
        speed: Some(0.0),
    };
    for thrower in [0usize, 2] {
        bots.tracker.input(thrower, now, key::GRENADE, (0.0, 0.0), 0.0, &crate::v026g::bots::kin::Kinematics::default(), None);
    }
    assert_eq!(bots.tracker.live_grenades().len(), 2, "both throw keys are listed");
    let mut placed = [false; 2];
    while now < 8_000 {
        now += 100;
        bots.poll(Clock { clock: now, time_left_s: 600.0, total_s: 900.0, playing: true });
        for (i, (thrower, mate)) in [(0usize, 1usize), (2, 3)].into_iter().enumerate() {
            let flying = bots.tracker.live_grenades().iter().find(|g| g.by == thrower).map(|g| g.phase);
            if let (false, Some(Phase::Flying { rest, .. })) = (placed[i], flying) {
                bots.fixed(mate, fix(now - 100, rest, 0), Some(1.0));
                placed[i] = true;
            }
        }
    }
    assert_eq!(placed, [true, true], "both grenades flew");
    bots.grenade_counts()
}

/// The avoidance of a grenade the live list holds from a bot's key with its pockets read and empty is each team's own switch
/// (`avoid.bot_pockets`): the team whose tuning reads the bots' pockets (team 0) refuses it, the team whose tuning does not
/// (team 1) knows of it, as its mates are told from the launch.
#[test]
fn an_assumed_grenade_is_refused_by_the_team_whose_tuning_reads_the_bots_pockets() {
    use crate::v026g::bots::live_grenades::{Pocketed, Pockets, Throws, Whose};
    let medkit = || Throws::Bot(Pockets::new([Some((Pocketed::Other, 1)), None, None, None, None, None]).into());
    let counts = grenade_probe(medkit);
    let assumed = Whose::Assumed.index();
    let noticed = |team: usize| counts[team].iter().map(|a| a.noticed.iter().sum::<u32>()).sum::<u32>();
    assert_eq!(noticed(0), 0, "team 0 reads the bots' pockets: it knows of none");
    assert!(counts[0][assumed].refused >= 1, "and it counts what it refused: {counts:?}");
    assert!(noticed(1) >= 1, "team 1 knows of its own: {counts:?}");
    assert_eq!(counts[1][assumed].refused, 0);
    assert!(counts[1][assumed].noticed_mate >= 1, "a teammate's, told from the launch");
}

/// A throw key of a bot whose pockets were never read (the bench's bots, which do not call `set_throws`) may throw a frag: every
/// team dodges it, the team that reads the bots' pockets too, and none is counted as refused. The bench is then as the tip.
#[test]
fn a_grenade_of_a_bot_with_unread_pockets_is_dodged_by_every_team() {
    use crate::v026g::bots::live_grenades::{Throws, Whose};
    let counts = grenade_probe(|| Throws::GrenadeKeyIsFrag);
    let unread = Whose::Unread.index();
    for team in 0..2 {
        assert!(counts[team][unread].noticed_mate >= 1, "team {team} knows of its mate's: {counts:?}");
        assert_eq!(counts[team][unread].refused, 0, "team {team} refuses none");
        assert_eq!(counts[team][Whose::Assumed.index()].refused, 0);
    }
}

/// The frag cap's wiring through `Bots::poll`: each poll hands every bot the frags its kit holds (`Throws::frags_left`), none for
/// a kit nobody read, and the cap itself is the team's own tuning (`aim.combat.grenade_kit_cap`).
#[test]
fn the_frags_a_kit_holds_reach_the_reflex_of_each_bot_and_the_cap_is_a_team_switch() {
    use crate::v026g::bots::live_grenades::{Pocketed, Pockets, Throws};
    let mut team1 = Tuning::default();
    team1.aim.combat.grenade_kit_cap = false;
    let (mut bots, now) = after_one_poll(Tuning::default(), team1);
    let medkit = || Throws::Bot(Pockets::new([Some((Pocketed::Other, 1)), None, None, None, None, None]).into());
    let frags = || Throws::Bot(Pockets::new([None, Some((Pocketed::Frag, 2)), None, None, None, None]).into());
    bots.set_throws(vec![medkit(), frags(), medkit(), Throws::GrenadeKeyIsFrag]);
    bots.poll(Clock { clock: now + 100, time_left_s: 600.0, total_s: 900.0, playing: true });
    let held: Vec<_> = (0..4).map(|place| bots.bots[place].as_ref().unwrap().reflex.frags_left()).collect();
    assert_eq!(held, [Some(0), Some(2), Some(0), None]);
    assert!(bots.tunings.of(0).aim.combat.grenade_kit_cap && !bots.tunings.of(1).aim.combat.grenade_kit_cap);
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

/// A battery lies in a zone so wide that no straight walk crosses to it, which only the motor's own route over the zone
/// offers (`route_crossing` `total`); how many bots of each team go for it, when team 0's and team 1's tunings are the
/// arguments (the HUD shows the lying battery, the way out is not priced: a walk of 26 m through the zone would be).
fn collectors_of_a_wide_zone(damage: mapdata::Damage, mut team0: Tuning, mut team1: Tuning) -> [usize; 2] {
    use mapdata::{Rotation, Shape, ShapeKind, Zone, ZoneKind};
    let at = Vec3::new(25.0, 0.0, 32.0);
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
            damage: Some(damage),
            shapes: vec![Shape {
                kind: ShapeKind::Sphere,
                center: Vec3::new(25.0, 1.0, 32.0),
                rotation: Rotation::IDENTITY,
                half: Vec3::new(26.0, 26.0, 26.0),
            }],
        });
    });
    team0.cheats.batteries = false;
    team1.cheats.batteries = false;
    team0.hazard.price_way_out = false;
    team1.hazard.price_way_out = false;
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
    for (place, team) in teams.iter().enumerate() {
        let collects = bots.bots[place]
            .as_ref()
            .and_then(|bot| bot.assignment.as_ref())
            .is_some_and(|a| a.kind == PlayKind::Collect);
        count[usize::from(*team)] += usize::from(collects);
    }
    count
}

/// The plays' reads of the hazard switches (`Ctx::tuning`, `plays/collect.rs`): a battery lies in a zone so wide that no straight
/// walk crosses to it, which only the motor's own route over the zone offers (`route_crossing` `total`). Team 0's switch is on,
/// team 1's off, so only team 0's bots go for the battery: the same situation, two answers by tuning. Swapped, the answers
/// swap.
#[test]
fn the_crossing_rule_is_read_from_the_tuning_of_the_bots_team() {
    use crate::v026g::bots::tuning::RouteCrossing;
    let goes_for_it = |route0: RouteCrossing, route1: RouteCrossing| -> [usize; 2] {
        let (mut team0, mut team1) = (Tuning::default(), Tuning::default());
        team0.hazard.route_crossing = route0;
        team1.hazard.route_crossing = route1;
        let damage = mapdata::Damage { max_hit: 0.01, interval_ms: 1000, parts: 2, ..mapdata::Damage::default() };
        collectors_of_a_wide_zone(damage, team0, team1)
    };
    let [t0, t1] = goes_for_it(RouteCrossing::Total, RouteCrossing::Off);
    assert!(t0 > 0 && t1 == 0, "team 0 (routes) {t0} collect, team 1 (off) {t1}");
    let [t0, t1] = goes_for_it(RouteCrossing::Off, RouteCrossing::Total);
    assert!(t0 == 0 && t1 > 0, "swapped: team 0 (off) {t0} collect, team 1 (routes) {t1}");
}

/// The price of a zone's hit by its place in the zone (`zone_intensity`) is a team's: a zone that falls off from its centre to
/// nothing at its rim, whose hit at the full price is more than the bots can bear on the way to a battery at its centre, is
/// crossed for it by the team whose tuning says so and refused by the other, the same situation; swapped, the answers swap.
#[test]
fn the_zone_intensity_rule_is_read_from_the_tuning_of_the_bots_team() {
    let goes_for_it = |intensity0: bool, intensity1: bool| -> [usize; 2] {
        let (mut team0, mut team1) = (Tuning::default(), Tuning::default());
        team0.hazard.zone_intensity = intensity0;
        team1.hazard.zone_intensity = intensity1;
        let damage = mapdata::Damage {
            // 0.028 a second at the centre: the walk in, the walk out and the take cost 25 times that at the full hit (0.7 of
            // the health, over the half a crossing may take), 14 times that at the intensity (0.4, borne).
            max_hit: 0.014,
            interval_ms: 1000,
            parts: 2,
            hit_curve: vec![(0.0, 1.0), (1.0, 0.0)],
            ..mapdata::Damage::default()
        };
        collectors_of_a_wide_zone(damage, team0, team1)
    };
    let [t0, t1] = goes_for_it(true, false);
    assert!(t0 > 0 && t1 == 0, "team 0 (intensity) {t0} collect, team 1 (full hit) {t1}");
    let [t0, t1] = goes_for_it(false, true);
    assert!(t0 == 0 && t1 > 0, "swapped: team 0 (full hit) {t0} collect, team 1 (intensity) {t1}");
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
    // The spot cost is a team's too: team 1 weighs its hiding spots, team 0 only filters them.
    let (mut team0, mut team1) = (Tuning::default(), Tuning::default());
    team0.cover.spot_cost = None;
    team1.cover.spot_cost = crate::v026g::bots::tuning::SpotCost::try_from(1.0).ok();
    let (bots, _) = after_one_poll(team0, team1);
    let costs = [0u8, 1].map(|team| bots.tuning_of(team).cover.rule().and_then(|rule| rule.spot_cost).map(|c| c.get()));
    assert_eq!(costs, [None, Some(1.0)], "each team's own spot cost");
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

/// The spacing switches are a team's too (A151's diagnosis arms): team 1 plays without Buddy and with Seen-only callouts, team
/// 0 as today. The switches reach the plays and the callouts through `Bots::think`'s one `tuning_of(team)` (`Due::tuning`),
/// the place the probes above exercise; each switch's own effect is in the tests of its file.
#[test]
fn the_spacing_switches_are_the_switches_of_the_team_that_asks() {
    let (team0, mut team1) = (Tuning::default(), Tuning::default());
    team1.spacing.buddy = false;
    team1.spacing.callouts = crate::v026g::bots::tuning::Callouts::SeenOnly { steer_age_ms: 8_000 };
    let (bots, _) = after_one_poll(team0, team1);
    assert_eq!([0u8, 1].map(|team| bots.tuning_of(team).spacing.buddy), [true, false]);
    assert_eq!(
        [0u8, 1].map(|team| bots.tuning_of(team).spacing.callouts.steer_age_ms()),
        [None, Some(8_000)]
    );
}

/// The edge watch (`smoke.react_watch`) through the real poll, one team's switch on: a cloud of each team between them
/// from the start, nobody sees anybody. Only the team whose tuning has it watches, and only the other team's cloud
/// (its own is told after `told_ms`); its bots look at the edge on the cloud's floor (a note a cloud).
#[test]
fn only_the_team_whose_tuning_reads_react_watch_watches_a_clouds_near_edge() {
    use crate::v026g::bots::smoke::tally::{self, Event};
    let run = |on: u8| {
        let before = tally::counts();
        let (mut team0, mut team1) = (Tuning::default(), Tuning::default());
        if on == 0 {
            team0.smoke.react_watch = true;
        } else {
            team1.smoke.react_watch = true;
        }
        let mut bots = facing_bots(team0, team1, 0x5E0F);
        // Team 1's cloud (thrown by place 2) across the middle; team 0's (place 0) beside it.
        bots.tracker.add_cloud(across_middle(0));
        bots.tracker.add_cloud(crate::v026g::bots::smoke::Cloud {
            by: 0,
            team: 0,
            centre: Vec3::new(35.0, 0.0, 42.0),
            ..across_middle(0)
        });
        let mut notes = Vec::new();
        for clock in (1_100..=6_000).step_by(100) {
            let (_, polled) = bots.poll(Clock { clock, time_left_s: 600.0, total_s: 900.0, playing: true });
            notes.extend(polled);
        }
        let after = tally::counts();
        let watched = [0, 1].map(|team| after[team][Event::EdgeWatch as usize] - before[team][Event::EdgeWatch as usize]);
        let edges: Vec<String> = notes.iter().filter(|n| n.text.contains("watches the edge")).map(|n| n.text.clone()).collect();
        (watched, edges)
    };
    let (watched, edges) = run(0);
    assert!(watched[0] > 0 && watched[1] == 0, "{watched:?}");
    assert_eq!(edges.len(), 2, "one note per bot of team 0: {edges:?}");
    for edge in &edges {
        assert!(edge.contains("player 2's smoke"), "only the other team's cloud: {edge}");
        // On the cloud's floor (y 0): the glance looks a chest's height over it.
        assert!(edge.contains(", 0.0, "), "{edge}");
    }
    let (watched, edges) = run(1);
    assert!(watched[1] > 0 && watched[0] == 0, "{watched:?}");
    assert!(edges.iter().all(|edge| edge.contains("player 0's smoke")), "{edges:?}");
}

/// The kit smoke (`kit.smoke`) of a bench team is built with its own tuning: team 1 on carries a `rdg-m` in its profiles,
/// team 0's kits are those without the smoke.
#[test]
fn only_the_kits_of_the_team_whose_tuning_has_kit_smoke_carry_a_smoke() {
    use survarium::v026g::dictionary::Dictionary;
    use survarium::v026g::loadouts::{BotEquipment, BotPocketKit, BotTier, RusherKits, bot_kits_kit, bot_equipment_of};
    let mut team1 = Tuning::default();
    team1.kit.smoke = survarium::v026g::loadouts::KitSmoke::RadioPlace;
    let teams = [0u8, 0, 1, 1];
    let equipment = BotEquipment::of(BotTier::Mid, RusherKits::default());
    let roster = bench_roster(&teams, equipment, &tunings(Tuning::default(), team1));
    let dictionary = Dictionary::shared();
    let mut carried = 0;
    for (place, &team) in teams.iter().enumerate() {
        let profile = &roster.profiles[place];
        let smokes = profile
            .items
            .iter()
            .filter(|item| dictionary.item(item.dict_id).is_some_and(|row| row.name == "rdg-m"))
            .count();
        if team == 0 {
            assert_eq!(smokes, 0, "place {place}");
            let (equipment, role) = bot_equipment_of(profile).expect("a bot");
            let plain = bot_kits_kit(equipment, role, BotPocketKit::default());
            let ids: Vec<Vec<u16>> = plain.iter().map(|kit| kit.items.iter().map(|item| item.dict_id).collect()).collect();
            let held: Vec<Vec<u16>> = profile
                .kits
                .iter()
                .map(|kit| kit.slots.iter().flatten().map(|&(index, _)| profile.items[usize::from(index)].dict_id).collect())
                .collect();
            for (want, have) in ids.iter().zip(&held) {
                let (mut want, mut have) = (want.clone(), have.clone());
                want.sort_unstable();
                have.sort_unstable();
                assert_eq!(want, have, "place {place}: the kits without the smoke");
            }
        } else {
            carried += smokes;
        }
    }
    assert!(carried > 0, "team 1's kits carry a smoke");
}

/// The team jobs on held spots (K2, `commander.rs` `threat_tick`) open by their own team's `threat.job_kills`: one enemy's two
/// kills from one spot are a hold to team 1 (2, the default) and not to team 0 (3).
#[test]
fn the_team_jobs_open_by_their_teams_job_kills() {
    let mut team0 = Tuning::default();
    team0.threat.job_kills = 3;
    let (mut bots, now) = after_one_poll(team0, Tuning::default());
    // Each team's two deaths to one enemy on a spot south of it, out of its bots' sight of it (42 m and more off) and its
    // lanes (the bots stand 40 degrees and more off them).
    let spot = Vec3::new(40.0, 0.0, 0.0);
    for (team, shooter) in [(0usize, 2), (1, 0)] {
        let rules = crate::v026g::bots::threat::SpotRules::of(&bots.tuning_of(team as u8).threat);
        for (i, victim) in [Vec3::new(40.0, 0.0, 20.0), Vec3::new(43.0, 0.0, 20.0)].into_iter().enumerate() {
            bots.teams[team].features.threat.spots.died(victim, shooter, spot, now, now + i as u32, rules);
        }
    }
    for clock in (now + 100..=now + 1_500).step_by(100) {
        bots.poll(Clock { clock, time_left_s: 600.0, total_s: 900.0, playing: true });
    }
    assert!(bots.teams[0].features.threat.jobs.is_empty(), "team 0: two kills are no hold by its job_kills 3");
    assert_eq!(bots.teams[1].features.threat.jobs.len(), 1, "team 1: a hold by its job_kills 2");
}
