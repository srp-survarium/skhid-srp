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
/// `RoomRules::match_start_wait_ms` after the first entry: it stands in, and the match
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

/// A private room needs one player (its owner starts it; the owner's of 2026-10-26):
/// its match of one place starts 1 v 0 (the clients' number for one place
/// is 0, so its entry starts the countdown), with no stand-in, and plays.
#[test]
fn a_room_match_of_one_player_plays_1_v_0() {
    let mut bench = Bench::room(&[0], MAP_BATTERIES, Settings::default(), 1);
    bench.join(0);
    assert!(bench.until(2_000, |b| b.game().rule_state() == state::COUNTDOWN));
    bench.run_long(25_000);
    assert_eq!(bench.game().rule_state(), state::IN_PROCESS);
    assert_eq!(client_state(&bench, 0), state::IN_PROCESS);
    assert!(bench.results.try_recv().is_err(), "not called off");
    assert!(bench.game().events().iter().all(|(_, who, _)| *who == 0));
    bench.assert_no_faults();
}

/// With a minimum of one no side needs a player: a match whose other side
/// never comes starts with the one who is in once the start wait is over
/// (stand-ins for the places that did not come), where a minimum of two
/// calls it off (`a_room_match_its_minimum_cannot_play_is_called_off`).
#[test]
fn a_room_match_of_a_minimum_of_one_starts_with_a_side_empty() {
    let mut bench = Bench::room(&[0, 1, 1], MAP_BATTERIES, Settings::default(), 1);
    bench.join(0);
    bench.run_long(20_000);
    assert_eq!(bench.game().rule_state(), state::WAITING_FOR_PLAYERS);
    assert!(bench.until(15_000, |b| b.game().rule_state() == state::COUNTDOWN));
    assert!(bench.results.try_recv().is_err(), "not called off");
    bench.run_long(25_000);
    assert_eq!(bench.game().rule_state(), state::IN_PROCESS);
    assert_eq!(client_state(&bench, 0), state::IN_PROCESS);
    assert!(!client_has(&bench, 0, 1) && !client_has(&bench, 0, 2));
    bench.assert_no_faults();
    assert_no_dropped_leaves(&bench);
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

/// The tick of the place's stand-in entry: its first entry 30 s or more
/// after the first (`RoomRules::match_start_wait_ms`; a stand-in's, in these tests).
fn stand_in_entry(bench: &Bench, place: usize) -> Option<u32> {
    let first = bench.game().events().iter().map(|(at, _, _)| *at).min()?;
    bench
        .game()
        .events()
        .iter()
        .find(|(at, who, entered)| *who == place && *entered && *at >= first + 30_000)
        .map(|(at, _, _)| *at)
}

/// The place's roster changes from `at` on: entered or left.
fn changes_from(bench: &Bench, place: usize, at: u32) -> Vec<bool> {
    bench
        .game()
        .events()
        .iter()
        .filter(|(each, who, _)| *who == place && *each >= at)
        .map(|(_, _, entered)| *entered)
        .collect()
}

/// Every game of a room's match lost its link before the countdown (a
/// crash, a time-out: no 0xE0), every place having come by then: the match
/// is called off, as when its players left by their own word, not
/// deserted (its results paid and its room kept waiting after a match).
#[test]
fn a_room_match_whose_games_all_lost_their_link_before_its_countdown_is_called_off() {
    let mut bench = Bench::room(&[0, 1, 0, 1], MAP_BATTERIES, Settings::default(), 2);
    bench.join(0);
    bench.join(1);
    bench.clients[1].drop_link();
    bench.run(500);
    bench.join(2);
    bench.clients[2].drop_link();
    bench.run(500);
    bench.join(3);
    assert_eq!(bench.game().rule_state(), state::WAITING_FOR_PLAYERS);
    bench.clients[0].drop_link();
    bench.clients[3].drop_link();
    bench.run_long(10_000);
    let result = bench.results.try_recv().expect("over");
    assert_eq!(result.end, MatchEnd::NotMade);
    assert_eq!(result.played_ms, None);
}

/// The games of a room's match all went while they loaded: the match is
/// called off, not "nobody came".
#[test]
fn a_room_match_whose_games_all_went_while_loading_is_called_off() {
    let mut bench = Bench::room(&[0, 1], MAP_BATTERIES, Settings::default(), 2);
    for place in [0, 1] {
        bench.clients[place].connect();
        assert!(bench.until(1_000, |b| b.clients[place].model.options.is_some()));
    }
    bench.clients[0].drop_link();
    bench.clients[1].drop_link();
    bench.run_long(70_000);
    let result = bench.results.try_recv().expect("over");
    assert_eq!(result.end, MatchEnd::NotMade);
}

/// A game whose world stands still from before the stand-ins' entry (its
/// window not active) has not played it when the others have: it is
/// resynced before the stand-ins leave, for it would drop their leave and
/// keep a stand-in in its world alone.
#[test]
fn a_game_behind_the_stand_ins_entry_is_resynced_before_their_leave() {
    let mut bench = Bench::room(&[0, 1, 0, 1], MAP_BATTERIES, Settings::default(), 2);
    bench.join(0);
    bench.join(1);
    assert!(bench.until(36_000, |b| stand_in_entry(b, 2).is_some()));
    let entry = stand_in_entry(&bench, 2).unwrap();
    assert!(bench.clients[1].clock() < entry);
    let at = bench.clients[1].clock();
    bench.clients[1].frozen_at = Some(at);
    assert!(
        bench.until(4_000, |b| b.game().resyncs(1) >= 1),
        "{:?}",
        bench.game().events()
    );
    // Its window active again, it loads the others' world.
    bench.clients[1].frozen_at = None;
    assert!(bench.until(3_000, |b| changes_from(b, 2, entry) == [true, false]));
    assert!(bench.until(10_000, |b| {
        b.stages().starts_with("LL") && b.clients[1].model.is_synced()
    }));
    bench.run_long(5_000);
    for place in [0, 1] {
        assert!(!client_has(&bench, place, 2), "player {place}");
        assert!(client_has(&bench, place, 0) && client_has(&bench, place, 1));
        assert_ne!(client_state(&bench, place), state::WAITING_FOR_PLAYERS);
    }
    assert_eq!(bench.game().resyncs(0), 0);
    bench.assert_no_faults();
    assert_no_dropped_leaves(&bench);
}

/// A place that stood in and whose game connects after the stand-in's
/// leave is a new arrival (from a world, then its entry), not a player back
/// in a match it left (its entry first, `enter_early`).
#[test]
fn a_place_that_stood_in_comes_as_a_new_player() {
    let mut bench = Bench::room(&[0, 1, 0, 1], MAP_BATTERIES, Settings::default(), 2);
    bench.join(0);
    bench.join(1);
    assert!(bench.until(36_000, |b| stand_in_entry(b, 2).is_some()));
    let entry = stand_in_entry(&bench, 2).unwrap();
    assert!(bench.until(3_000, |b| changes_from(b, 2, entry) == [true, false]));
    bench.join(2);
    bench.run_long(3_000);
    let entries: Vec<u32> = bench
        .game()
        .events()
        .iter()
        .filter(|(_, who, entered)| *who == 2 && *entered)
        .map(|(at, _, _)| *at)
        .collect();
    assert_eq!(entries.len(), 2, "{:?}", bench.game().events());
    let (_, world) = bench.clients[2].snapshots()[0];
    assert!(
        entries[1] > world,
        "entered at {} ms, its world of {world} ms",
        entries[1]
    );
    for place in [0, 1, 2] {
        assert!(client_has(&bench, place, 2), "player {place}");
    }
    bench.assert_no_faults();
    assert_no_dropped_leaves(&bench);
}

/// A player whose game was lost, its place standing in, comes back while
/// the stand-in is in: its game waits for the stand-in's leave and then
/// enters, once; the leave does not take it out.
#[test]
fn a_player_whose_place_stands_in_comes_back_after_the_stand_in() {
    let mut bench = Bench::room(&[0, 1, 0, 1, 0], MAP_BATTERIES, Settings::default(), 2);
    bench.join(0);
    bench.join(1);
    bench.join(2);
    bench.run(1_000);
    bench.clients[2].drop_link();
    assert!(bench.until(36_000, |b| stand_in_entry(b, 2).is_some()));
    let entry = stand_in_entry(&bench, 2).unwrap();
    bench.restart(2);
    bench.clients[2].connect();
    assert!(bench.until(1_000, |b| b.clients[2].model.options.is_some()));
    assert_eq!(changes_from(&bench, 2, entry), [true], "the stand-in is in");
    bench.clients[2].ready();
    assert!(
        bench.until(30_000, |b| {
            b.stages().as_bytes().get(2) == Some(&b'L') && b.clients[2].model.is_synced()
        }),
        "{}",
        bench.stages()
    );
    bench.run_long(3_000);
    assert_eq!(
        changes_from(&bench, 2, entry),
        [true, false, true],
        "{:?}",
        bench.game().events()
    );
    for place in [0, 1, 2] {
        assert!(client_has(&bench, place, 2), "player {place}");
    }
    bench.assert_no_faults();
    assert_no_dropped_leaves(&bench);
}

/// A place taken into the match (a member of the room come late, its game
/// not connected yet) may still come until its admission is over: the
/// match is not called off for want of its side meanwhile.
#[test]
fn a_place_taken_into_the_match_counts_as_one_that_may_come() {
    let mut bench = Bench::room(&[0, 1, 0, 1], MAP_BATTERIES, Settings::default(), 2);
    bench.join(0);
    bench.run_long(20_000);
    // The match takes no joiner before its countdown; opened here, so that
    // the place's admission is still running at the start wait's end.
    bench.registry.set_admitting(
        bench.match_id,
        4,
        Ok(Instant::now() + Duration::from_secs(600)),
    );
    bench
        .registry
        .join_running(
            bench.match_id,
            MatchPlayer {
                session_id: session_of(4),
                account_id: 104,
                profile: profile("late", 1),
                bot: false,
                driven: false,
            },
        )
        .unwrap();
    bench.run_long(15_000);
    assert!(
        bench.results.try_recv().is_err(),
        "called off with a place of team 1 to come"
    );
}
