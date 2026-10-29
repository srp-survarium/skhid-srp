use super::*;

/// A bot that left is forgotten by the others.
#[test]
pub(crate) fn a_bot_that_left_is_forgotten() {
    use survarium::v026g::maps::mode;
    // A human of team 0 at x = 0; team 1's bots at 5, 40 and 30 m.
    let teams = [0, 1, 1, 1];
    let driven = [false, true, true, true];
    let mut bots = Bots::new(
        0x6A7E,
        mode::BATTERIES,
        0,
        &teams,
        &driven,
        &[None; 4],
        Arc::new(Tuning::default()),
        Trace::none(0),
    );
    for place in 0..4 {
        bots.entered(place, true, 100);
    }
    for (place, x) in [0.0, 5.0, 40.0, 30.0].into_iter().enumerate() {
        bots.fixed(place, standing(1_000, x, true), Some(1.0));
    }
    // Bot 3 leaves: it is forgotten, and no partner of anyone.
    bots.bots[1].as_mut().unwrap().partner = Some(3);
    bots.entered(3, false, 6_000);
    assert!(!bots.is_bot(3) && bots.is_bot(1));
    assert_eq!(bots.bots[1].as_ref().unwrap().partner, None);
    assert!(
        !bots
            .tracker()
            .view(
                &Clock {
                    clock: 6_000,
                    time_left_s: 0.0,
                    total_s: 0.0,
                    playing: true,
                },
                &bots.match_wide_tuning().kin,
                None
            )
            .places[3]
            .present
    );
    let (made, _) = bots.poll(Clock {
        clock: 9_000,
        time_left_s: 600.0,
        total_s: 900.0,
        playing: true,
    });
    assert!(made.iter().all(|made| made.place != 3));
}

/// The world's HUD mask reaches the teams' knowledge (`WorldFacts::marks`
/// through the tracker to `TeamKnowledge::hud_update`): team 1's dropped
/// battery that falls through level_01's floor and is put back 2 m away
/// stays known to team 1 (matches 0x1a09…/0x7298…), never to team 0.
#[test]
pub(crate) fn a_dropped_battery_put_back_after_a_fall_stays_on_its_teams_hud() {
    let teams = [0, 1];
    let mut bots = Bots::new(
        0xF1,
        survarium::v026g::maps::mode::BATTERIES,
        0,
        &teams,
        &[true, true],
        &[None; 2],
        Arc::new(Tuning::default()),
        Trace::none(0),
    );
    assert!(!bots.match_wide_tuning().cheats.batteries);
    for place in 0..2 {
        bots.entered(place, true, 100);
    }
    let lying = |time: u32, at: Vec3, mask: u32| WorldFacts {
        time,
        batteries: vec![BatteryState::Free(at)],
        marks: vec![mask],
        ..WorldFacts::default()
    };
    let known = |bots: &Bots, team: usize| bots.teams[team].knowledge.battery(0).copied();
    let dropped = Vec3::new(-43.58, 19.55, 3.80);
    let reset = Vec3::new(-41.6, 21.6, 3.5);
    bots.facts(lying(1_000, Vec3::new(-14.0, 21.7, -61.4), 0));
    assert!(known(&bots, 0).is_none() && known(&bots, 1).is_none());
    bots.facts(WorldFacts {
        time: 2_000,
        batteries: vec![BatteryState::Carried(None)],
        marks: vec![0],
        ..WorldFacts::default()
    });
    bots.facts(lying(3_000, dropped, 2));
    assert!(known(&bots, 1).is_some_and(|k| k.hud));
    bots.facts(lying(4_000, Vec3::new(dropped.x, -86.0, dropped.z), 2));
    bots.facts(lying(5_000, reset, 2));
    let kept = known(&bots, 1).expect("still on team 1's HUD");
    assert!(kept.hud && kept.time == 5_000);
    // Where the world has it: the worlds are in the map's frame.
    assert!((kept.at - reset).length() < 1e-3, "{kept:?}");
    assert!(known(&bots, 0).is_none());
}

/// Match 0x5402a730 (2026-10-16): the client's world has a dead body
/// without feet, and `game::driven` hands the bots its bodies before its
/// statistics, so the death the statistics show came with no spot: no
/// kill feed entry, no danger spot, play 21 never a candidate. Now the
/// death is where the body was last reckoned alive, and a team's third
/// bot has a partner too (one way: it trades, it follows no one).
#[test]
pub(crate) fn a_partners_death_the_world_shows_without_feet_is_traded_by_both_teammates() {
    let world = soak_world();
    let teams = [0, 0, 0, 1, 1, 1];
    let mut bots = Bots::new(
        0x5402,
        survarium::v026g::maps::mode::BATTERIES,
        0,
        &teams,
        &[true; 6],
        &[None; 6],
        Arc::new(Tuning::default()),
        Trace::none(0),
    );
    bots.map_ready(world);
    for place in 0..6 {
        bots.entered(place, true, 0);
    }
    let spots = [
        Vec3::new(10.0, 0.0, 30.0),
        Vec3::new(14.0, 0.0, 30.0),
        Vec3::new(15.0, 0.0, 34.0),
        Vec3::new(70.0, 0.0, 30.0),
        Vec3::new(72.0, 0.0, 34.0),
        Vec3::new(70.0, 0.0, 38.0),
    ];
    let body = |time: u32, place: usize, alive: bool| Fix {
        time,
        alive,
        inserted: 100,
        feet: alive.then_some(spots[place]),
        yaw: 0.0,
        pitch: 0.0,
        keys: 0,
        speed: Some(0.0),
    };
    let clock = |clock: u32| Clock {
        clock,
        time_left_s: 600.0,
        total_s: 900.0,
        playing: true,
    };
    for place in 0..6 {
        bots.fixed(place, body(1_000, place, true), Some(1.0));
    }
    bots.facts(WorldFacts {
        time: 1_000,
        stats: vec![(0, 0, 0); 6],
        ..WorldFacts::default()
    });
    bots.poll(clock(1_100));
    let partner = |bots: &Bots, place: usize| {
        let bot = bots.bots[place].as_ref().unwrap();
        (bot.partner, bot.paired)
    };
    assert_eq!(partner(&bots, 0), (Some(1), true));
    assert_eq!(partner(&bots, 1), (Some(0), true));
    // The third: its nearest living teammate, one way.
    assert_eq!(partner(&bots, 2), (Some(1), false));
    assert_eq!(partner(&bots, 5), (Some(4), false));

    // Bot 1 dies: the world shows it dead, without feet, then the
    // statistics.
    for place in 0..6 {
        bots.fixed(place, body(2_000, place, place != 1), Some(1.0));
    }
    let mut stats = vec![(0, 0, 0); 6];
    stats[1].1 = 1;
    stats[3].0 = 1;
    bots.facts(WorldFacts {
        time: 2_000,
        stats,
        ..WorldFacts::default()
    });
    let view = bots.tracker().view(&clock(2_000), &bots.match_wide_tuning().kin, None);
    assert_eq!(view.deaths.len(), 1, "{:?}", view.deaths);
    assert_eq!(view.deaths[0].place, 1);
    assert!(
        view.deaths[0].at.distance_xz(spots[1]) < 1.0,
        "{:?}",
        view.deaths
    );
    let danger = &bots.teams[0].knowledge.danger;
    assert_eq!(danger.len(), 1, "{danger:?}");
    assert_eq!(danger[0].victim, 1);
    assert!(danger[0].at.distance_xz(spots[1]) < 1.0, "{danger:?}");
    // Both its teammates trade it: its pair and the third.
    for now in (2_100..=3_000).step_by(100) {
        bots.poll(clock(now));
    }
    // The third keeps its partner while he is down.
    assert_eq!(partner(&bots, 2), (Some(1), false));
    for place in [0, 2] {
        let bot = bots.bots[place].as_ref().unwrap();
        assert_eq!(
            bot.assignment.as_ref().map(|a| a.kind),
            Some(plays::PlayKind::Trade),
            "bot {place}"
        );
    }
}
