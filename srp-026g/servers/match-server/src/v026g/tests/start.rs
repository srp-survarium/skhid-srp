//! A room's match starts with the players who are in, or is called off
//! (`game/start.rs`, `docs/notes/match.md` section 34).

use super::*;
use crate::v026g::MatchEnd;
use survarium::v026g::match_msgs::rules::state;

/// The rule's state in a client's own simulation now.
fn client_state(bench: &Bench, place: usize) -> u8 {
    bench.clients[place].sim.rule_at(bench.clock()).state
}

/// Every game took every leave: none came before the game played the
/// entry (a stand-in's leave waits for that).
fn assert_no_dropped_leaves(bench: &Bench) {
    for (place, client) in bench.clients.iter().enumerate() {
        assert!(
            client.dropped_leaves.is_empty(),
            "player {place}: {:?}",
            client.dropped_leaves
        );
    }
}

/// Whether the place is in the entered mask of the client's simulation.
fn client_has(bench: &Bench, place: usize, player: usize) -> bool {
    bench.clients[place].sim.rule_at(bench.clock()).mask & (1 << player) != 0
}

/// The live room 5209 (2026-10-20): made 8 v 8 from 1 v 1, a match of five
/// places (team 0: places 0, 1, 2; team 1: 3 and 4), so the clients wait
/// for 4 in. Places 3, 4, 1 came; 4 left by its own word; 2 came; 1 left by
/// its own word; then 0 came and left. No countdown ever started, and the
/// two who stayed waited in "Awaiting other players". Now, once the
/// places that can still come cannot make 4, two places given up stand in
/// for one tick, the countdown starts, and the two who are there play.
#[test]
fn the_live_room_5209_starts_with_the_two_players_who_stayed() {
    let mut bench = Bench::room(&[0, 0, 0, 1, 1], MAP_BATTERIES, Settings::default(), 2);
    bench.join(3);
    bench.join(4);
    bench.join(1);
    bench.run(1_000);
    bench.clients[4].leave();
    bench.run(1_000);
    bench.join(2);
    bench.run(1_000);
    assert_eq!(bench.game().rule_state(), state::WAITING_FOR_PLAYERS);
    bench.clients[1].leave();
    // 2 in, place 0 still to come: 3 of the 4 needed at most.
    assert!(
        bench.until(2_000, |b| b.game().rule_state() == state::COUNTDOWN),
        "no countdown: {:?}",
        bench.game().events()
    );
    // Every game counts down too, from the same tick.
    assert!(bench.until(1_000, |b| {
        [2, 3]
            .iter()
            .all(|&place| client_state(b, place) == state::COUNTDOWN)
    }));
    // The two places given up stood in, and leave again once both games
    // played their entry.
    let stand_ins = |b: &Bench| {
        let events = b.game().events();
        let entry = events
            .iter()
            .rev()
            .find(|(_, who, entered)| *who == 4 && *entered)
            .map(|(at, _, _)| *at);
        (
            entry,
            events
                .iter()
                .filter(|(at, who, entered)| {
                    entry.is_some_and(|entry| *at > entry) && [1, 4].contains(who) && !entered
                })
                .count(),
        )
    };
    assert!(
        bench.until(3_000, |b| stand_ins(b).1 == 2),
        "{:?}",
        bench.game().events()
    );
    let (entry, _) = stand_ins(&bench);
    assert!(bench.game().events().contains(&(entry.unwrap(), 1, true)));
    bench.run(1_000);
    for place in [2, 3] {
        assert!(!client_has(&bench, place, 1) && !client_has(&bench, place, 4));
        assert!(client_has(&bench, place, 2) && client_has(&bench, place, 3));
    }
    // Place 0 comes during the countdown, and leaves.
    bench.join(0);
    bench.run(1_000);
    bench.clients[0].leave();
    bench.run_long(25_000);
    assert_eq!(bench.game().rule_state(), state::IN_PROCESS);
    for place in [2, 3] {
        assert_eq!(client_state(&bench, place), state::IN_PROCESS);
        assert_eq!(bench.game().resyncs(place), 0, "player {place}");
    }
    bench.assert_no_faults();
    assert_no_dropped_leaves(&bench);
}

/// A place whose game never connects is not waited for past
/// `START_WAIT_MS` after the first entry: it stands in, and the match
/// starts with who is in. The result counts the stand-in nowhere.
#[test]
fn a_place_whose_game_never_comes_stands_in_after_the_start_wait() {
    let mut bench = Bench::room(&[0, 1, 0, 1], MAP_BATTERIES, Settings::default(), 2);
    bench.join(0);
    bench.join(1);
    // 2 of the 3 needed; places 2 and 3 may still come.
    bench.run_long(25_000);
    assert_eq!(bench.game().rule_state(), state::WAITING_FOR_PLAYERS);
    assert!(bench.until(8_000, |b| b.game().rule_state() == state::COUNTDOWN));
    assert!(bench.until(3_000, |b| b.clients[0].left().contains(&2)));
    bench.run_long(25_000);
    assert_eq!(bench.game().rule_state(), state::IN_PROCESS);
    for place in [0, 1] {
        assert_eq!(client_state(&bench, place), state::IN_PROCESS);
        assert!(!client_has(&bench, place, 2));
        assert_eq!(bench.game().resyncs(place), 0);
    }
    bench.assert_no_faults();
    assert_no_dropped_leaves(&bench);
    // Everyone leaves: the result, with the stand-in as a place nobody
    // came to.
    bench.clients[0].leave();
    bench.clients[1].leave();
    bench.run_long(20_000);
    let result = bench.results.try_recv().expect("the match is over");
    assert!(result.played_ms.is_some());
    let stand_in = &result.players[2];
    assert!(!stand_in.entered && !stand_in.left && stand_in.time_in_match_ms == 0);
    assert!(result.players[0].entered && result.players[1].entered);
}

/// Fewer than the room's minimum can play: the match is called off before
/// its countdown, its games are told it is over, and nothing of it counts.
#[test]
fn a_room_match_its_minimum_cannot_play_is_called_off() {
    let mut bench = Bench::room(&[0, 1, 0, 1], MAP_BATTERIES, Settings::default(), 2);
    bench.join(0);
    bench.join(1);
    bench.run(1_000);
    // Team 1: place 1 gives up, place 3 never comes.
    bench.clients[1].leave();
    bench.run_long(20_000);
    assert!(bench.results.try_recv().is_err(), "place 3 may still come");
    bench.run_long(15_000);
    let result = bench.results.try_recv().expect("called off");
    assert_eq!(result.end, MatchEnd::NotMade);
    assert_eq!(result.played_ms, None);
    // Its game is told the end (0xEA of its own place).
    assert!(bench.until(1_000, |b| b.clients[0].closed));
}

/// Every player who came left by its own word before the countdown: the
/// match is called off, at the latest when the start wait is over for the
/// places that never came.
#[test]
fn a_room_match_everyone_left_before_its_countdown_is_called_off() {
    let mut bench = Bench::room(&[0, 1, 0, 1], MAP_BATTERIES, Settings::default(), 2);
    bench.join(0);
    bench.join(1);
    bench.run(1_000);
    bench.clients[0].leave();
    bench.clients[1].leave();
    assert!(bench.until(35_000, |b| b.results.try_recv().is_ok_and(
        |result| result.end == MatchEnd::NotMade && result.played_ms.is_none()
    )));
}

/// A match with no room (a clan match) keeps the clients' rule alone.
#[test]
fn a_match_of_no_room_waits_for_its_players_as_before() {
    let mut bench = Bench::growing(4, MAP_BATTERIES, Settings::default());
    bench.join(0);
    bench.join(1);
    bench.run_long(60_000);
    assert_eq!(bench.game().rule_state(), state::WAITING_FOR_PLAYERS);
    assert!(bench.game().events().iter().all(|(_, who, _)| *who < 2));
}
