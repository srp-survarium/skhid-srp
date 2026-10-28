//! A111 `roam.fresh_claims` at the start countdown: the commander picks the opening plays while `Clock::playing` is false, each
//! Roam draw seeing the picks before it, the bots stay at rest, the claims go with the runs. Without a countdown (a respawn, a
//! joiner) the same rule runs at the commander's pass in play.

use super::*;

use crate::v026g::bots::plays::{PlayKind, Rank, Running, default_tuning};
use crate::v026g::bots::routes::HumanRoute;

/// Six bots of team 0 (three pairs: the leaders draw routes, the followers follow) and one of team 1 on the field with two routes for side 0.
fn bots_on(fresh: bool, seed: u32) -> (Bots, Game, Arc<World>) {
    let world = soak_world();
    let mut tuning = Tuning::all_off();
    // The field's exits make the hold plays the best picks: Roam is put first so the picks are route draws.
    let roam = tuning.plays.entry(PlayKind::Roam).or_insert_with(|| default_tuning(PlayKind::Roam));
    roam.rank = Rank::Alarm;
    if fresh {
        roam.numbers.insert("fresh_claims".into(), 1.0);
    }
    let teams = [0u8, 0, 0, 0, 0, 0, 1];
    let mut bots = Bots::new(0xA111 + seed, survarium::v026g::maps::mode::TEAM_DEATHMATCH, 0, &teams, &[true; 7], &[None; 7], Arc::new(tuning), Trace::none(0));
    bots.map_ready(world.clone());
    let route = |reward: f32, points: &[(f32, f32)]| HumanRoute {
        team: 0,
        lives: 20,
        reward,
        fight: reward,
        points: points.iter().map(|&(x, z)| Vec3::new(x, 0.0, z)).collect(),
        ..Default::default()
    };
    bots.tactics.as_mut().unwrap().set_routes(vec![
        route(3.0, &[(20.0, 30.0), (35.0, 33.0), (50.0, 33.0)]),
        route(1.0, &[(20.0, 8.0), (30.0, 5.0), (52.0, 5.0)]),
    ]);
    let game = Game::new(world.clone(), &teams, u64::from(seed), None, None);
    for place in 0..teams.len() {
        bots.entered(place, true, 0);
    }
    (bots, game, world)
}

/// One poll at `clock`; the worlds are fed every second.
fn poll(bots: &mut Bots, game: &mut Game, clock: u32, playing: bool) -> Vec<String> {
    while game.tick + TICK_MS <= clock {
        game.step();
    }
    if clock.is_multiple_of(1_000)
        && clock > 0
        && let Some((_, fixes, health, facts)) = game.world_at(clock / TICK_MS * TICK_MS - 300)
    {
        for (place, (fix, health)) in fixes.into_iter().zip(health).enumerate() {
            bots.fixed(place, fix, Some(health));
        }
        bots.facts(facts);
    }
    let (_, notes) = bots.poll(Clock { clock, time_left_s: 600.0, total_s: 900.0, playing });
    notes.into_iter().map(|n| n.text).collect()
}

/// The side-0 routes the team-0 bots run along, by bot (a follower runs none).
fn routes_run(bots: &Bots) -> Vec<Option<usize>> {
    (0..6)
        .map(|p| bots.bots[p].as_ref().and_then(|b| b.running.as_ref()).and_then(Running::route_claim).map(|(route, _)| route))
        .collect()
}

fn claimed(bots: &Bots, now: u32) -> usize {
    (0..2)
        .map(|route| bots.teams[0].knowledge.route_claims_of_others(usize::MAX, 0, route, now, 600_000).count())
        .sum()
}

#[test]
fn the_commander_picks_the_opening_plays_in_the_countdown_and_the_bots_rest() {
    let (mut bots, mut game, _) = bots_on(true, 1);
    let mut notes = Vec::new();
    for clock in (0..=3_000).step_by(25) {
        notes.extend(poll(&mut bots, &mut game, clock, false));
    }
    let drawn = notes.iter().filter(|n| n.contains("(opening pass in the countdown)")).count();
    assert!(drawn >= 3, "the log says which path drew the routes: {notes:?}");
    let runs = routes_run(&bots);
    let leaders = runs.iter().flatten().count();
    assert!(leaders >= 3, "the three leaders run routes: {runs:?}");
    assert_eq!(claimed(&bots, 3_000), leaders, "each pick is claimed");
    for place in 0..6 {
        let intent = &bots.bots[place].as_ref().unwrap().intent;
        assert_eq!(format!("{intent:?}"), format!("{:?}", crate::v026g::bots::plays::Intent::stay()), "bot {place} rests before go");
    }
    // Off: no pick before go.
    let (mut bots, mut game, _) = bots_on(false, 1);
    for clock in (0..=3_000).step_by(25) {
        poll(&mut bots, &mut game, clock, false);
    }
    assert_eq!(routes_run(&bots).iter().flatten().count(), 0);
    assert_eq!(claimed(&bots, 3_000), 0);
}

#[test]
fn the_opening_picks_spread_over_the_routes_each_seeing_the_picks_before_it() {
    let all_one = |fresh: bool| {
        (0..40)
            .filter(|&seed| {
                let (mut bots, mut game, _) = bots_on(fresh, seed);
                for clock in (0..=3_000).step_by(25) {
                    poll(&mut bots, &mut game, clock, false);
                }
                // Play begins: the commander's pass of play (the whole pass for the switch off).
                poll(&mut bots, &mut game, 3_025, true);
                let runs = routes_run(&bots);
                let routes: Vec<usize> = runs.into_iter().flatten().collect();
                routes.len() >= 3 && routes.iter().all(|r| *r == routes[0])
            })
            .count()
    };
    let (off, on) = (all_one(false), all_one(true));
    assert!(on + 3 < off, "all four on one route in {off} of 40 openings off, {on} on");
}

#[test]
fn the_claims_go_with_the_runs() {
    let (mut bots, mut game, _) = bots_on(true, 3);
    for clock in (0..=3_000).step_by(25) {
        poll(&mut bots, &mut game, clock, false);
    }
    let leaders = routes_run(&bots).iter().flatten().count();
    assert_eq!(claimed(&bots, 3_000), leaders);
    // Play begins; a run ended by the bot (a new play, an arrival, a replan all end the run the same way).
    poll(&mut bots, &mut game, 3_025, true);
    let mut notes = Vec::new();
    let ender = routes_run(&bots).iter().position(Option::is_some).unwrap();
    bots.bots[ender].as_mut().unwrap().end_play(3_050, "test", &mut notes);
    poll(&mut bots, &mut game, 3_050, true);
    let held: Vec<bool> = (0..6)
        .map(|p| bots.teams[0].knowledge.route_claims_of_others(usize::MAX - 1, 0, 0, 3_050, 600_000).any(|c| c.who == p)
            || bots.teams[0].knowledge.route_claims_of_others(usize::MAX - 1, 0, 1, 3_050, 600_000).any(|c| c.who == p))
        .collect();
    assert!(!held[ender], "the bot that ended its run holds no route: {held:?}");
}

#[test]
fn without_a_countdown_the_same_rule_runs_at_the_commanders_pass_in_play() {
    let (mut bots, mut game, _) = bots_on(true, 2);
    let mut notes = Vec::new();
    for clock in (25..=1_500).step_by(25) {
        notes.extend(poll(&mut bots, &mut game, clock, true));
    }
    let leaders = routes_run(&bots).iter().flatten().count();
    assert!(leaders >= 3, "{leaders}");
    assert_eq!(claimed(&bots, 1_500), leaders);
    assert!(notes.iter().any(|n| n.contains("(fresh claim in play)")), "{notes:?}");
}
