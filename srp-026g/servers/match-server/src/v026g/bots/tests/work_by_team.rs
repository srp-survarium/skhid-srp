use super::*;
use crate::v026g::bots::bench_work::{self, Work};

/// The work counters of a real match of bots (two a side on the soak field, 20 s): each team's bots and its commander do work that
/// is counted for the team, the shared rest is what no team asked for. A made-up match cannot show this: the commanders and the
/// pending searches are the bots' own.
#[test]
pub(crate) fn a_teams_commander_and_searches_are_counted_for_the_team() {
    let world = soak_world();
    let teams = [0u8, 0, 1, 1];
    let mut bots = Bots::new(
        0x7A6F,
        survarium::v026g::maps::mode::BATTERIES,
        0,
        &teams,
        &[true; 4],
        &[None; 4],
        Arc::new(Tuning::default()),
        Trace::none(0),
    );
    bots.map_ready(world.clone());
    let mut game = Game::new(world.clone(), &teams, 7, None, None);
    for place in 0..teams.len() {
        bots.entered(place, true, 0);
    }
    let before = bench_work::totals();
    let (mut clock, mut next_world) = (0, 1_000);
    while clock < 20_000 {
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
        let (made, _) = bots.poll(Clock { clock, time_left_s: 600.0, total_s: 900.0, playing: true });
        for made in made {
            let r = made.input.rotation;
            let turn = (half_value(u16::from_le_bytes([r[0], r[1]])), half_value(u16::from_le_bytes([r[2], r[3]])));
            let gun = Arms::fallback().primary.0;
            game.inputs[made.place].push_back((made.time, made.input.actions, turn, None, gun));
        }
    }
    let after = bench_work::totals();
    let delta = |row: usize, work: Work| after[row][work as usize] - before[row][work as usize];
    for team in 0..2 {
        assert!(delta(team, Work::Plays) > 0, "team {team}'s commander scored plays: {:?}", (0..3).map(|r| delta(r, Work::Plays)).collect::<Vec<_>>());
        assert!(delta(team, Work::Rays) > 0, "team {team}'s bots cast rays");
    }
    assert_eq!(delta(2, Work::Plays), 0, "no play is scored for no team");
}
