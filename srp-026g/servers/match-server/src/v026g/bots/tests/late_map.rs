use super::*;

/// The map built cold, after the bots' first spawn (11-16 s on the owner's server, the spawns at 1.55 s): the bots
/// that spawned without it take their side when it comes, from where their first life began. Without the side a
/// bot's whole first life had no Cut, HoldExit or Crossfire (only Roam and Buddy).
#[test]
pub(crate) fn a_bot_spawned_before_the_map_takes_its_side_when_the_map_comes() {
    let world = soak_world();
    let teams = [0u8, 1];
    let mut bots = Bots::new(
        0x51DE,
        survarium::v026g::maps::mode::BATTERIES,
        0,
        &teams,
        &[true, true],
        &[None, None],
        Arc::new(Tuning::default()),
        Trace::none(0),
    );
    let mut game = Game::new(world.clone(), &teams, 7, None, None);
    bots.entered(0, true, 0);
    bots.entered(1, true, 0);
    // The first lives begin with no map.
    for _ in 0..(2_000 / TICK_MS) {
        game.step();
    }
    let (_, fixes, health, facts) = game.world_at(game.tick).expect("a world");
    for (place, (fix, health)) in fixes.into_iter().zip(health).enumerate() {
        bots.fixed(place, fix, Some(health));
    }
    bots.facts(facts);
    let first: Vec<Vec3> = (0..2)
        .map(|place| {
            let bot = bots.bots[place].as_ref().expect("a bot");
            assert_eq!(bot.side, None, "place {place}: no map, no side");
            bot.first_feet.expect("spawned")
        })
        .collect();
    let summary = bots.map_ready(world.clone());
    assert!(summary.contains("2 bots spawned before the map"), "{summary}");
    let tactics = bots.tactics().expect("the map");
    for (place, feet) in first.into_iter().enumerate() {
        let side = bots.bots[place].as_ref().expect("a bot").side;
        assert!(side.is_some(), "place {place}: a side once the map came");
        assert_eq!(side, tactics.side_at(feet), "place {place}: the side of its first spawn");
    }
    // The two teams spawn apart: the sides differ.
    assert_ne!(bots.bots[0].as_ref().unwrap().side, bots.bots[1].as_ref().unwrap().side);
}

/// The map ready before the spawns (the bench, a warm map): the side is read at the spawn as before, and the map's
/// coming changes nothing.
#[test]
pub(crate) fn a_map_ready_before_the_spawns_reads_no_side_late() {
    let world = soak_world();
    let teams = [0u8, 1];
    let mut bots = Bots::new(
        0x51DE,
        survarium::v026g::maps::mode::BATTERIES,
        0,
        &teams,
        &[true, true],
        &[None, None],
        Arc::new(Tuning::default()),
        Trace::none(0),
    );
    let summary = bots.map_ready(world.clone());
    assert!(!summary.contains("spawned before the map"), "{summary}");
}
