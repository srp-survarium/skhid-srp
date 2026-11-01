//! The match's state machine, in memory on a clock the test moves.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use std::collections::BTreeSet;

use survarium::v026g::match_msgs::client_model::Skip;
use survarium::v026g::match_msgs::players::{self, PlayerState};
use survarium::v026g::match_msgs::rules::state;
use survarium::v026g::match_msgs::snapshot::{Sections, event};
use survarium::v026g::match_msgs::{PlayerInput, ServerMessage, WireTime, id};
use udp_transport::wire::{self, low_level};
use udp_transport::{Direction, Table};
use world_frame::{Frame, PLAYER_STATE_CAP, Places, Section};

use super::client::{Client, HORIZON_MS, INITIAL_MS};
use super::{Bench, MAP_BATTERIES, MAP_TDM, session_of};
use crate::v026g::registry::static_id;
use crate::v026g::{DRAW, LateInputs, MatchEnd, Settings};

/// Every client's clock is the match clock's (the pings measured it and
/// 0xE7 brought it there).
fn settle(bench: &mut Bench) {
    bench.run(4_000);
    let clock = i64::from(bench.clock());
    for (place, client) in bench.clients.iter().enumerate() {
        if client.model.is_synced() && !client.closed {
            let off = clock - i64::from(client.clock());
            assert!(off.abs() <= 30, "player {place}'s clock is {off} ms behind");
        }
    }
}

fn set_times(client: &Client) -> Vec<u8> {
    client.got(|message| match message {
        ServerMessage::SetTime { generation, .. } => Some(*generation),
        _ => None,
    })
}

/// The generations of the syncs it was taken through.
fn generations(client: &Client) -> Vec<u8> {
    let mut generations = set_times(client);
    generations.dedup();
    generations
}

fn discards(client: &Client) -> Vec<WireTime> {
    let mut times = client.got(|message| match message {
        ServerMessage::DiscardInput { time } => Some(*time),
        _ => None,
    });
    times.dedup();
    times
}

#[test]
fn a_solo_match_runs_from_the_connect_to_the_time_limit() {
    let mut bench = Bench::new(1, MAP_BATTERIES, Settings::default());
    bench.clients[0].connect();
    assert!(bench.until(1_000, |bench| bench.clients[0].model.options.is_some()));
    {
        let client = &bench.clients[0];
        // The one port, then the options the client model read back whole.
        assert_eq!(
            client.taken[0],
            ServerMessage::Ports {
                min: 1236,
                max: 1236
            }
        );
        assert_eq!(client.model.local_index, Some(0));
        let options = client.model.options.as_ref().unwrap();
        assert_eq!(options.players.len(), 1);
        assert_eq!(options.players[0].name, "player0");
        assert_eq!(options.map_id, MAP_BATTERIES);
        assert_eq!(options.match_id, static_id(bench.match_id));
        assert_eq!(options.match_level, 3);
    }
    assert_eq!(bench.stages(), "l");

    // Loaded: its initial world goes up unasked, then 0xDA. The match
    // clock starts on that world, the client from it, as generation 0.
    bench.clients[0].ready();
    assert!(bench.until(1_000, |bench| bench.clients[0].model.is_synced()));
    assert!(bench.until(1_000, |bench| bench.stages() == "L"));
    // Its time alone first, then again with the world behind it.
    assert_eq!(set_times(&bench.clients[0]), [0, 0]);
    assert_eq!(bench.clients[0].snapshots(), [(0, INITIAL_MS)]);
    assert_eq!(bench.game().generation(0), Some(0));
    assert!(bench.until(500, |bench| bench.clients[0].entered() == [0]));
    // One player of a roster of one: the countdown starts with it.
    assert_eq!(bench.game().rule_state(), state::COUNTDOWN);
    settle(&mut bench);

    // Alone, its inputs go nowhere and none is turned down.
    for _ in 0..10 {
        bench.clients[0].input(0, 0x21);
        bench.run(100);
    }
    assert!(discards(&bench.clients[0]).is_empty());
    assert!(bench.clients[0].relays_of(0).is_empty());

    // 20 s of countdown, then play.
    bench.run_long(20_000);
    assert_eq!(bench.game().rule_state(), state::IN_PROCESS);
    // Worlds are asked for now and then, and the client's own reports
    // vouch for them: there is nobody else.
    assert!(bench.clients[0].upload_requests >= 3);
    let (trusted, standing) = bench.game().observed().expect("a trusted snapshot");
    assert!(trusted > 15_000, "{trusted}");
    assert_eq!(standing.map(|standing| standing.winner), Some(DRAW));
    assert!(bench.results.try_recv().is_err());

    // 900 s of play, then the end: the client is told it left.
    bench.run_long(895_000);
    assert!(!bench.clients[0].closed);
    assert!(bench.until(20_000, |bench| bench.clients[0].closed));
    let result = bench.results.try_recv().expect("the match's result");
    assert_eq!(result.match_id, bench.match_id);
    assert_eq!(result.end, MatchEnd::TimeLimit);
    assert_eq!(result.map_id, MAP_BATTERIES);
    assert_eq!(result.game_mode, 2);
    assert_eq!(result.winner, Some(DRAW));
    assert_eq!(result.team_points, [0, 0]);
    // Its own word: there was nobody else.
    assert_eq!(result.trusted_by, 1);
    let played = result.played_ms.expect("play began");
    assert!((900_000..903_500).contains(&played), "{played}");
    assert!(result.duration_ms > 920_000);
    assert!(result.trusted_snapshot_ms.unwrap() + 3_000 > result.duration_ms);
    let player = &result.players[0];
    assert_eq!(
        (player.place, player.session_id, player.account_id),
        (0, session_of(0), 100)
    );
    assert!(player.entered && player.present_at_end && !player.left);
    assert!(player.time_in_match_ms > 900_000);
    assert!(player.statistics.is_some());
    assert!(bench.results.try_recv().is_err(), "one result a match");

    // A moment later the match is gone and its seat free.
    bench.run_long(7_000);
    assert!(bench.server.games.is_empty());
    assert_eq!(bench.registry.seat(session_of(0)), None);
    assert_eq!(bench.registry.match_count(), 0);
    assert!(bench.results.try_recv().is_err());
    // Its resyncs, snapshots and generations never moved again.
    assert_eq!(bench.clients[0].model.syncs, 1);
    bench.assert_no_faults();
}

#[test]
fn inputs_are_relayed_both_ways_while_their_tick_is_open() {
    let mut bench = Bench::new(2, MAP_BATTERIES, Settings::default());
    bench.join_all();
    // Each saw both enter. The second started from the initial world too
    // (it was fresh still), so the first one's entry came to it after it.
    assert!(bench.until(500, |bench| bench.clients[0].entered() == [0, 1]));
    assert!(bench.until(50, |bench| bench.clients[1].entered() == [0, 1]));
    assert_eq!(bench.clients[1].snapshots(), [(0, INITIAL_MS)]);
    settle(&mut bench);

    let first = bench.clients[0].input(50, 0x8000_0021);
    bench.run(100);
    // Byte for byte, under its player's index; never back to its owner.
    assert_eq!(bench.clients[1].relays_of(0), [first]);
    assert!(bench.clients[0].relays_of(0).is_empty());
    let second = bench.clients[1].input(50, 0x40);
    bench.run(100);
    assert_eq!(bench.clients[0].relays_of(1), [second]);

    // A precise input (0xDD) goes out as the precise relay (0xEC).
    let clock = bench.clients[0].clock() + 73;
    let precise = PlayerInput {
        time: WireTime::precise(clock),
        rotation: [1, 2, 3, 4],
        actions: 0x20,
    };
    bench.clients[0].send_input(precise);
    bench.run(100);
    assert_eq!(bench.clients[1].relays_of(0), [first, precise]);
    assert!(
        bench.clients[1]
            .taken
            .iter()
            .any(|message| { message.id() == id::INPUT_RELAY_PRECISE })
    );

    // The client resends an input until it is acked, and the transport
    // hands every copy on: a copy is not relayed again.
    bench.clients[0].send_input(first);
    bench.clients[0].send_input(first);
    bench.run(100);
    assert_eq!(bench.clients[1].relays_of(0).len(), 2);

    // An input whose tick is final (250 ms) is not relayed: its owner is
    // told to drop it.
    let late = bench.clients[0].input(-400, 0x1);
    bench.run(100);
    assert_eq!(discards(&bench.clients[0]), [late.time]);
    assert_eq!(bench.clients[1].relays_of(0).len(), 2);
    assert!(discards(&bench.clients[1]).is_empty());
    bench.assert_no_faults();
}

#[test]
fn a_late_input_is_moved_when_the_setting_says_so() {
    let settings = Settings {
        late_inputs: LateInputs::Move,
        ..Settings::default()
    };
    let mut bench = Bench::new(2, MAP_BATTERIES, settings);
    bench.join_all();
    settle(&mut bench);
    let late = bench.clients[0].input(-400, 0x1);
    bench.run(100);
    // Its owner hears where it went; the other gets it there.
    let moves = bench.clients[0].got(|message| match message {
        ServerMessage::MoveInput { from, to } => Some((*from, *to)),
        _ => None,
    });
    assert!(!moves.is_empty());
    let (from, to) = moves[0];
    assert_eq!(from, late.time);
    let moved = i32::from(to.tick.wrapping_sub(from.tick) as i8);
    assert!((3..=6).contains(&moved), "moved {moved} ticks");
    assert!(discards(&bench.clients[0]).is_empty());
    assert_eq!(
        bench.clients[1].relays_of(0),
        [PlayerInput { time: to, ..late }]
    );
    bench.assert_no_faults();
}

#[test]
fn an_input_for_the_tick_a_moved_one_went_to_is_moved_on_and_a_stale_one_dropped() {
    let settings = Settings {
        late_inputs: LateInputs::Move,
        ..Settings::default()
    };
    let mut bench = Bench::new(2, MAP_BATTERIES, settings);
    bench.join_all();
    settle(&mut bench);
    // Late: moved. Then the player's own input for the very
    // tick it went to, in time: that one must go on to the next tick, for
    // the owner's node there holds both and a discard would take both.
    let late = bench.clients[0].input(-400, 0x1);
    bench.run(20);
    let moves = |client: &Client| {
        client.got(|message| match message {
            ServerMessage::MoveInput { from, to } => Some((*from, *to)),
            _ => None,
        })
    };
    let first = moves(&bench.clients[0]);
    assert_eq!(first.len(), 1);
    let (from, to) = first[0];
    assert_eq!(from, late.time);
    let own = PlayerInput {
        time: to,
        rotation: [9, 9, 9, 9],
        actions: 0x2,
    };
    bench.clients[0].send_input(own);
    bench.run(20);
    let next = WireTime {
        tick: to.tick.wrapping_add(1),
        ms: None,
    };
    assert_eq!(moves(&bench.clients[0]), [(from, to), (to, next)]);
    assert!(discards(&bench.clients[0]).is_empty());
    // The other client has both, in the order they were made.
    assert_eq!(
        bench.clients[1].relays_of(0),
        [
            PlayerInput { time: to, ..late },
            PlayerInput { time: next, ..own }
        ]
    );
    // A copy of either (the client resends until acked) changes nothing.
    bench.clients[0].send_input(late);
    bench.clients[0].send_input(own);
    bench.run(20);
    assert_eq!(moves(&bench.clients[0]).len(), 2);
    assert_eq!(bench.clients[1].relays_of(0).len(), 2);

    // An input older than one the others have already: stale, dropped.
    let newer = bench.clients[0].input(0, 0x4);
    bench.run(20);
    let stale = bench.clients[0].input(-420, 0x8);
    bench.run(20);
    assert_eq!(discards(&bench.clients[0]), [stale.time]);
    assert_eq!(moves(&bench.clients[0]).len(), 2);
    assert_eq!(bench.clients[1].relays_of(0).len(), 3);
    assert_eq!(bench.clients[1].relays_of(0)[2], newer);
    bench.assert_no_faults();
}

/// 0.26i's notes: "Fixed a bug which caused crashes of all clients in the
/// game associated with sending "bad" input of one of the clients". The
/// server is where one client's input becomes everybody's.
#[test]
fn an_input_with_a_rotation_no_client_makes_goes_to_nobody() {
    let mut bench = Bench::new(3, MAP_BATTERIES, Settings::default());
    bench.join_all();
    settle(&mut bench);
    let good = bench.clients[0].input(50, 0x21);
    bench.run(100);
    assert_eq!(bench.clients[1].relays_of(0), [good]);

    // Infinity, minus infinity, a NaN, and a number no mouse turns by
    // (65 504, the largest half), in either half.
    let bad_halves: [[u8; 4]; 5] = [
        [0x00, 0x7C, 0x00, 0x00],
        [0x00, 0x00, 0x00, 0xFC],
        [0x01, 0x7E, 0x00, 0x3C],
        [0x00, 0x3C, 0xFF, 0x7B],
        [0x00, 0x64, 0x00, 0x00],
    ];
    let mut refused = Vec::new();
    for (step, rotation) in bad_halves.into_iter().enumerate() {
        let clock = bench.clients[0].clock() + 50;
        let input = PlayerInput {
            time: WireTime::tick(clock),
            rotation,
            actions: 0x21,
        };
        bench.clients[0].send_input(input);
        // The client sends it again until it is acked.
        bench.clients[0].send_input(input);
        bench.run(100);
        refused.push(input.time);
        assert_eq!(bench.game().inputs_refused(0), step as u32 + 1);
    }
    // Its owner is told to drop each; nobody else hears of them.
    assert_eq!(discards(&bench.clients[0]), refused);
    for other in [1, 2] {
        assert_eq!(bench.clients[other].relays_of(0), [good]);
        assert!(discards(&bench.clients[other]).is_empty());
    }

    // The largest turn that is taken, and the player goes on as before.
    let clock = bench.clients[0].clock() + 50;
    let large = PlayerInput {
        time: WireTime::tick(clock),
        rotation: [0xFF, 0x63, 0xFF, 0xE3],
        actions: 0x21,
    };
    bench.clients[0].send_input(large);
    bench.run(100);
    let after = bench.clients[0].input(50, 0x20);
    bench.run(100);
    assert_eq!(bench.clients[1].relays_of(0), [good, large, after]);
    assert_eq!(bench.clients[2].relays_of(0), [good, large, after]);
    assert_eq!(bench.game().inputs_refused(0), 5);

    // Somebody who comes back gets what was relayed and nothing else.
    bench.clients[2].drop_link();
    bench.run(2_000);
    bench.restart(2);
    bench.clients[2].connect();
    assert!(bench.until(1_000, |bench| bench.clients[2].model.options.is_some()));
    bench.clients[2].ready();
    assert!(bench.until(10_000, |bench| bench.stages() == "LLL"));
    assert!(
        bench.clients[2]
            .relays_of(0)
            .iter()
            .all(PlayerInput::rotation_is_sane)
    );
    bench.assert_no_faults();
}

#[test]
fn a_late_input_is_dropped_when_the_setting_says_so() {
    let settings = Settings {
        late_inputs: LateInputs::Discard,
        ..Settings::default()
    };
    let mut bench = Bench::new(2, MAP_BATTERIES, settings);
    bench.join_all();
    settle(&mut bench);
    let late = bench.clients[0].input(-400, 0x1);
    bench.run(100);
    assert_eq!(discards(&bench.clients[0]), [late.time]);
    assert!(bench.clients[1].relays_of(0).is_empty());
    bench.assert_no_faults();
}

/// The default: an input past the server's horizon that every other game
/// can still take into its history goes to them at its own tick, and its
/// owner, who keeps it, is told nothing. (Dropped, the owner's drop order
/// came past its own history on the slow lines that made inputs late, and
/// it kept an input nobody else had: live, 2026-10-15.)
#[test]
fn a_late_input_the_others_can_still_take_is_relayed_at_its_tick() {
    let mut bench = Bench::new(2, MAP_BATTERIES, Settings::default());
    bench.join_all();
    settle(&mut bench);
    let late = bench.clients[0].input(-350, 0x1);
    bench.run(100);
    assert!(discards(&bench.clients[0]).is_empty());
    assert_eq!(bench.clients[1].relays_of(0), [late]);
    bench.assert_no_faults();
}

/// ... but not when one of them could not: its round trip and a margin
/// leave less than the input's lateness before its history passes the
/// tick. Then it is dropped, as before.
#[test]
fn a_late_input_a_far_game_could_not_take_is_dropped() {
    let mut bench = Bench::new(3, MAP_BATTERIES, Settings::default());
    // The third game: 150 ms each way, a 300 ms round trip: an input more
    // than 125 ms late reaches it past its history.
    bench.one_way = vec![0, 0, 150];
    bench.join_all();
    settle(&mut bench);
    bench.run(3_000);
    // 300 ms late: past the horizon, and past what the far game leaves.
    let late = bench.clients[0].input(-300, 0x1);
    bench.run(500);
    assert_eq!(discards(&bench.clients[0]), [late.time]);
    assert!(bench.clients[1].relays_of(0).is_empty());
    assert!(bench.clients[2].relays_of(0).is_empty());
    bench.assert_no_faults();
}

#[test]
fn a_profile_input_is_relayed_once() {
    let mut bench = Bench::new(2, MAP_BATTERIES, Settings::default());
    bench.join_all();
    settle(&mut bench);
    let tick = bench.clients[1].profile_input(2);
    bench.run(300);
    let mut relays = bench.clients[0].got(|message| match message {
        ServerMessage::ProfileInputRelay {
            tick,
            player,
            value,
        } => Some((*tick, *player, *value)),
        _ => None,
    });
    relays.dedup();
    assert_eq!(relays, [(tick, 1, 2)]);
    assert!(
        bench.clients[1]
            .taken
            .iter()
            .all(|message| { !matches!(message, ServerMessage::ProfileInputRelay { .. }) })
    );
    bench.assert_no_faults();
}

#[test]
fn kit_choices_before_play_are_counted_for_the_result() {
    // Patch 31: a game respawns a live player in a kit chosen before play
    // began, where it stands, with no death. The result says how many such
    // choices a player made: each may have been a life with a kit's
    // supplies of its own.
    let mut bench = Bench::new(2, MAP_BATTERIES, Settings::default());
    bench.join_all();
    settle(&mut bench);
    assert_eq!(bench.game().rule_state(), state::COUNTDOWN);
    bench.clients[1].profile_input(2);
    bench.run(300);
    bench.clients[1].profile_input(0);
    bench.run(300);
    bench.run_long(20_000);
    assert_eq!(bench.game().rule_state(), state::IN_PROCESS);
    // In play a choice is for the next life, as it always was.
    bench.clients[1].profile_input(1);
    bench.clients[0].profile_input(1);
    bench.run(300);
    bench.clients[0].leave();
    bench.clients[1].leave();
    bench.run(1_500);
    let result = bench.results.try_recv().expect("a result");
    assert!(result.played_ms.is_some());
    assert_eq!(
        result
            .players
            .iter()
            .map(|player| player.kit_changes)
            .collect::<Vec<_>>(),
        [0, 2]
    );
    bench.assert_no_faults();
}

#[test]
fn a_late_joiner_starts_from_a_fresh_snapshot_and_gets_what_came_since() {
    let mut bench = Bench::new(2, MAP_BATTERIES, Settings::default());
    bench.join(0);
    settle(&mut bench);
    bench.run(3_000);

    // The second player loads while the first plays and sends inputs.
    bench.clients[1].connect();
    assert!(bench.until(1_000, |bench| bench.clients[1].model.options.is_some()));
    bench.clients[1].ready();
    let mut sent = Vec::new();
    for _ in 0..40 {
        if bench.stages() == "LL" {
            break;
        }
        sent.push(bench.clients[0].input(50, 0x21));
        bench.run(50);
    }
    assert_eq!(bench.stages(), "LL");
    // Not from the initial world, 7 s old: from one the first player
    // uploaded when asked, at most 700 ms old, as generation 0.
    let snapshots = bench.clients[1].snapshots();
    assert_eq!(snapshots.len(), 1);
    let (generation, time) = snapshots[0];
    assert_eq!(generation, 0);
    assert!(time > 6_000, "{time}");
    // The inputs since that snapshot came with it, and none from before.
    let relays = bench.clients[1].relays_of(0);
    assert!(!relays.is_empty());
    assert!(relays.iter().all(|relay| sent.contains(relay)));
    assert!(
        bench.clients[1].skipped.is_empty(),
        "{:?}",
        bench.clients[1].skipped
    );
    // It entered for both; the first player is in its snapshot already.
    assert!(bench.until(500, |bench| bench.clients[0].entered() == [0, 1]));
    assert_eq!(bench.clients[1].entered(), [1]);
    // Its clock started behind and is brought up.
    settle(&mut bench);
    bench.assert_no_faults();
}

/// What becomes of player 1's clock when it is put `behind` ms behind the
/// match clock in a match of two. Its game makes a correction up at
/// `factor` of real time, and the server has `revision` as its patch
/// revision. `(how long after the first correction the clock was right,
/// the corrections it was sent, the most its clock was ever ahead)`, ms.
fn clock_brought_up(revision: u8, factor: f32, behind: u32) -> (u64, Vec<i16>, i64) {
    let mut bench = Bench::new(2, MAP_BATTERIES, Settings::default());
    bench.server.revisions.insert(session_of(1), revision);
    bench.clients[1].model.clock_factor = Some(factor);
    bench.join_all();
    let off = |bench: &Bench| i64::from(bench.clock()) - i64::from(bench.clients[1].clock());
    let offsets = |bench: &Bench| {
        bench.clients[1].got(|message| match message {
            ServerMessage::ClockOffset(offset) => Some(*offset),
            _ => None,
        })
    };
    // What its first world left it behind by is made up first.
    assert!(bench.until(60_000, |bench| {
        off(bench).abs() <= 10 && bench.clients[1].model.correction_left() == 0.0
    }));
    bench.run(3_000);
    let before = offsets(&bench).len();
    let clock = bench.clients[1].model.clock.unwrap();
    bench.clients[1].model.clock = Some(clock - behind);

    let (mut told_at, mut right_at, mut ahead) = (None, None, 0);
    let start = bench.ms;
    while bench.ms < start + 60_000 && right_at.is_none() {
        bench.step(5);
        ahead = ahead.max(-off(&bench));
        if told_at.is_none() && offsets(&bench).len() > before {
            told_at = Some(bench.ms);
        }
        if told_at.is_some() && off(&bench).abs() <= 10 {
            right_at = Some(bench.ms);
        }
    }
    // And it stays right: nothing more is sent, nothing is overshot.
    let sent = offsets(&bench).len();
    for _ in 0..2_000 {
        bench.step(5);
        ahead = ahead.max(-off(&bench));
    }
    assert_eq!(
        offsets(&bench).len(),
        sent,
        "a correction after the clock was right"
    );
    assert!(off(&bench).abs() <= 10);
    bench.assert_no_faults();
    let took = right_at.expect("its clock was brought up") - told_at.unwrap();
    (took, offsets(&bench)[before..].to_vec(), ahead)
}

#[test]
fn a_clock_is_brought_up_as_fast_as_the_games_build_does_it() {
    use survarium::v026g::match_msgs::clock::{FAST_FACTOR, FAST_FROM_REVISION, STOCK_FACTOR};
    // The original's 5 %: 400 ms take 8 s, in one correction.
    let (took, sent, ahead) = clock_brought_up(FAST_FROM_REVISION - 1, STOCK_FACTOR, 400);
    assert!((7_700..=8_100).contains(&took), "{took} ms");
    assert!(
        sent.len() == 1 && (390..=410).contains(&sent[0]),
        "{sent:?}"
    );
    assert!(ahead <= 10, "{ahead} ms ahead");
    // From revision 8 the game's 30 %: 1.3 s.
    let (took, sent, ahead) = clock_brought_up(FAST_FROM_REVISION, FAST_FACTOR, 400);
    assert!((1_200..=1_400).contains(&took), "{took} ms");
    assert!(
        sent.len() == 1 && (390..=410).contains(&sent[0]),
        "{sent:?}"
    );
    assert!(ahead <= 10, "{ahead} ms ahead");
}

/// Player 1's clock is brought up by 400 ms and, the moment it is right,
/// is 200 ms behind once more: how long until the server sends the next
/// correction, ms. Its game is of the fast factor; the server has
/// `revision` for it.
fn next_correction_after(revision: u8) -> u64 {
    use survarium::v026g::match_msgs::clock::FAST_FACTOR;
    let mut bench = Bench::new(2, MAP_BATTERIES, Settings::default());
    bench.server.revisions.insert(session_of(1), revision);
    bench.clients[1].model.clock_factor = Some(FAST_FACTOR);
    bench.join_all();
    let off = |bench: &Bench| i64::from(bench.clock()) - i64::from(bench.clients[1].clock());
    let offsets = |bench: &Bench| {
        bench.clients[1]
            .got(|message| matches!(message, ServerMessage::ClockOffset(_)).then_some(()))
            .len()
    };
    assert!(bench.until(60_000, |bench| {
        off(bench).abs() <= 10 && bench.clients[1].model.correction_left() == 0.0
    }));
    bench.run(3_000);
    let put_behind = |bench: &mut Bench, by: u32| {
        let clock = bench.clients[1].model.clock.unwrap();
        bench.clients[1].model.clock = Some(clock - by);
    };
    let before = offsets(&bench);
    put_behind(&mut bench, 400);
    assert!(bench.until(10_000, |bench| offsets(bench) == before + 1));
    assert!(bench.until(10_000, |bench| off(bench).abs() <= 10));
    put_behind(&mut bench, 200);
    let since = bench.ms;
    assert!(bench.until(30_000, |bench| offsets(bench) == before + 2));
    bench.ms - since
}

#[test]
fn a_fast_games_clock_is_looked_at_again_as_soon_as_it_is_done() {
    use survarium::v026g::match_msgs::clock::FAST_FROM_REVISION;
    // Known for what it is: the 1.3 s it takes, the second after, and the
    // two samples that a correction wants.
    let known = next_correction_after(FAST_FROM_REVISION);
    assert!(known <= 2_500, "{known} ms");
    // Taken for the original, its clock is left alone for the 8 s the
    // original would take: 200 ms behind for all that time, and a resync
    // it needed meanwhile would be put off as long.
    let unknown = next_correction_after(FAST_FROM_REVISION - 1);
    assert!(unknown >= 7_000, "{unknown} ms");
}

#[test]
fn a_game_whose_build_the_server_has_wrong_is_brought_up_all_the_same() {
    use survarium::v026g::match_msgs::clock::{FAST_FACTOR, FAST_FROM_REVISION, STOCK_FACTOR};
    // Taken for the original, and faster: done in its own 1.3 s; the
    // server only waits longer than it need before it looks again.
    let (took, sent, ahead) = clock_brought_up(0, FAST_FACTOR, 400);
    assert!((1_200..=1_400).contains(&took), "{took} ms");
    assert_eq!(sent.len(), 1, "{sent:?}");
    assert!(ahead <= 10, "{ahead} ms ahead");
    // Taken for the fast one, and the original: the server looks again
    // while the game is still at it, and what it sends then (the lag that
    // is left, in place of the correction that runs) changes nothing: the
    // game's own 8 s, and never a ms too far.
    let (took, sent, ahead) = clock_brought_up(FAST_FROM_REVISION, STOCK_FACTOR, 400);
    assert!((7_700..=8_300).contains(&took), "{took} ms");
    assert!(
        sent.len() >= 2 && sent.windows(2).all(|pair| pair[1] < pair[0]),
        "{sent:?}"
    );
    assert!(ahead <= 10, "{ahead} ms ahead");
}

#[test]
fn a_clock_correction_that_is_lost_goes_again_until_it_arrives() {
    let mut bench = Bench::new(2, MAP_BATTERIES, Settings::default());
    bench.join_all();
    settle(&mut bench);
    // Its clock falls 300 ms behind; every datagram with a 0xE7 in it is
    // lost for two seconds (a send-once message, live, 2026-10-14).
    let losing = Arc::new(AtomicBool::new(true));
    let lost = losing.clone();
    let dropped = Arc::new(AtomicUsize::new(0));
    let counted = dropped.clone();
    bench.clients[1].lose = Some(Box::new(move |datagram| {
        let drop = lost.load(Ordering::Relaxed)
            && wire::parse(Table::Match, Direction::ToClient, datagram, 1).is_ok_and(|parsed| {
                parsed
                    .messages
                    .iter()
                    .any(|message| message.id == id::CLOCK_OFFSET)
            });
        if drop {
            counted.fetch_add(1, Ordering::Relaxed);
        }
        drop
    }));
    let clock = bench.clients[1].model.clock.unwrap();
    bench.clients[1].model.clock = Some(clock - 300);
    bench.run(2_000);
    // It went, and again, and no newer one in its place.
    assert!(dropped.load(Ordering::Relaxed) >= 2, "{dropped:?}");
    losing.store(false, Ordering::Relaxed);
    bench.run(1_000);
    let offsets = bench.clients[1].got(|message| match message {
        ServerMessage::ClockOffset(offset) => Some(*offset),
        _ => None,
    });
    // Taken once (a copy has the same unique id), and the clock is right.
    assert_eq!(offsets.len(), 1, "{offsets:?}");
    assert!((290..=310).contains(&offsets[0]), "{offsets:?}");
    let off = i64::from(bench.clock()) - i64::from(bench.clients[1].clock());
    assert!(off.abs() <= 10, "{off} ms behind");
    bench.assert_no_faults();
}

#[test]
fn a_client_out_of_step_is_resynced_as_the_next_generation() {
    let mut bench = Bench::new(3, MAP_BATTERIES, Settings::default());
    bench.join_all();
    settle(&mut bench);
    bench.run(3_000);
    assert_eq!(bench.game().resyncs(2), 0);

    // The third client's world goes its own way.
    let from = bench.clients[2].clock();
    bench.clients[2].sim.salt_from = Some((from, 7));
    // Its reports disagree with the other two's: after they settled
    // (2.5 s) and stayed wrong for a second, it is resynced from a world
    // one of the others uploads.
    assert!(bench.until(10_000, |bench| bench.game().resyncs(2) == 1));
    assert!(bench.until(5_000, |bench| bench.clients[2].model.syncs == 2));
    assert!(bench.until(2_000, |bench| bench.stages() == "LLL"));
    assert_eq!(generations(&bench.clients[2]), [0, 1]);
    let snapshots = bench.clients[2].snapshots();
    assert_eq!(snapshots.len(), 2);
    assert_eq!(snapshots[1].0, 1);
    assert!(snapshots[1].1 > from);
    assert_eq!(bench.game().generation(2), Some(1));
    assert_eq!(bench.clients[2].model.generation.0, 1);
    assert!(bench.clients[0].upload_requests + bench.clients[1].upload_requests > 0);
    // It did not enter again, and nobody saw it leave.
    assert_eq!(bench.clients[0].entered(), [0, 1, 2]);
    assert!(bench.clients[0].left().is_empty());

    // In step again, it stays: no second resync, and the others never had
    // one.
    bench.run_long(25_000);
    assert_eq!(bench.game().resyncs(2), 1);
    assert_eq!(bench.clients[2].model.syncs, 2);
    assert_eq!(bench.clients[0].model.syncs, 1);
    assert_eq!(bench.clients[1].model.syncs, 1);
    settle(&mut bench);
    bench.assert_no_faults();
}

/// A game wrong from its first reports after the world it was resynced
/// with is resynced again at once, not after the 15 s between resyncs; a
/// third such world in a row waits them out (no world helps that game).
#[test]
fn a_resync_that_did_not_take_is_done_again_at_once_twice_at_most() {
    let mut bench = Bench::new(3, MAP_BATTERIES, Settings::default());
    bench.join_all();
    settle(&mut bench);
    bench.run(3_000);
    let from = bench.clients[2].clock();
    bench.clients[2].sim.salt_from = Some((from, 7));
    // Each time it has loaded a world, it goes wrong at once again.
    let mut at = Vec::new();
    for resyncs in 1..=4u32 {
        assert!(
            bench.until(30_000, |bench| bench.game().resyncs(2) == resyncs),
            "resync {resyncs}"
        );
        at.push(bench.ms);
        assert!(bench.until(5_000, |bench| bench.clients[2].model.syncs == resyncs + 1));
        let loaded = bench.clients[2].clock();
        bench.clients[2].sim.salt_from = Some((loaded, 7));
    }
    // The second and third came seconds after the one before; the fourth
    // after the interval.
    assert!(at[1] - at[0] < 6_000, "{at:?}");
    assert!(at[2] - at[1] < 6_000, "{at:?}");
    assert!(at[3] - at[2] >= 15_000, "{at:?}");
}

/// Live, 2026-10-15: a game agreed for one or two reports after its world
/// and was wrong 105 and 261 ms after it; those right reports took the
/// quick redo away, and it waited 12.7 and 14.3 s for its next resync. A
/// right report inside [`LOAD_WRONG_MS`] of the load no longer says the
/// world took.
#[test]
fn a_resync_wrong_soon_after_a_few_right_reports_is_done_again_at_once() {
    let mut bench = Bench::new(3, MAP_BATTERIES, Settings::default());
    bench.join_all();
    settle(&mut bench);
    bench.run(3_000);
    let from = bench.clients[2].clock();
    bench.clients[2].sim.salt_from = Some((from, 7));
    let mut at = Vec::new();
    for resyncs in 1..=2u32 {
        assert!(
            bench.until(30_000, |bench| bench.game().resyncs(2) == resyncs),
            "resync {resyncs}"
        );
        at.push(bench.ms);
        assert!(bench.until(5_000, |bench| bench.clients[2].model.syncs == resyncs + 1));
        // Right for a few ticks past its world, then wrong again.
        let loaded = bench.clients[2].clock();
        bench.clients[2].sim.salt_from = Some((loaded + 300, 7));
    }
    assert!(at[1] - at[0] < 6_000, "{at:?}");
}

/// When player 2 is resynced, `count` times: each world right for 3 s past
/// its load, then wrong again.
fn resyncs_after_agreed(settings: Settings, count: u32) -> Vec<u64> {
    let mut bench = Bench::new(3, MAP_BATTERIES, settings);
    bench.join_all();
    settle(&mut bench);
    bench.run(3_000);
    let from = bench.clients[2].clock();
    bench.clients[2].sim.salt_from = Some((from, 7));
    let mut at = Vec::new();
    for resyncs in 1..=count {
        assert!(
            bench.until(40_000, |bench| bench.game().resyncs(2) == resyncs),
            "resync {resyncs}"
        );
        at.push(bench.ms);
        assert!(bench.until(5_000, |bench| bench.clients[2].model.syncs == resyncs + 1));
        let loaded = bench.clients[2].clock();
        bench.clients[2].sim.salt_from = Some((loaded + 3_000, 7));
    }
    at
}

#[test]
fn a_new_desync_after_a_resync_that_agreed_waits_the_short_interval() {
    let at = resyncs_after_agreed(Settings::default(), 2);
    let gap = at[1] - at[0];
    assert!((5_000..8_000).contains(&gap), "{at:?}");
    // The setting at 15 s is the old rule.
    let mut settings = Settings::default();
    settings.timing.resync_after_agreed = Duration::from_secs(15);
    let at = resyncs_after_agreed(settings, 2);
    assert!(at[1] - at[0] >= 15_000, "{at:?}");
}

#[test]
fn a_game_resynced_four_times_in_a_minute_waits_the_whole_interval() {
    let at = resyncs_after_agreed(Settings::default(), 5);
    let gaps: Vec<u64> = at.windows(2).map(|pair| pair[1] - pair[0]).collect();
    assert!(gaps[..3].iter().all(|gap| *gap < 8_000), "{gaps:?}");
    assert!(gaps[3] >= 15_000, "{gaps:?}");
}

#[test]
fn a_snapshot_that_does_not_arrive_goes_again_as_the_next_generation() {
    let mut bench = Bench::new(1, MAP_BATTERIES, Settings::default());
    bench.clients[0].connect();
    assert!(bench.until(1_000, |bench| bench.clients[0].model.options.is_some()));
    // Every datagram with a 0xE8 in it is lost for a while (the 0xE5
    // travels with the first).
    let losing = Arc::new(AtomicBool::new(true));
    let lost = losing.clone();
    bench.clients[0].lose = Some(Box::new(move |datagram| {
        lost.load(Ordering::Relaxed)
            && wire::parse(Table::Match, Direction::ToClient, datagram, 1).is_ok_and(|parsed| {
                parsed
                    .messages
                    .iter()
                    .any(|message| message.id == id::SNAPSHOT)
            })
    }));
    bench.clients[0].ready();
    // A game's first world is kept on its way for four seconds, not the
    // client table's 1.75: its world stands still until it has one.
    bench.run(3_700);
    // Its time came and was answered (and came once more ahead of the
    // world, in a datagram of its own when the world's first part fills
    // one); the world did not.
    assert_eq!(bench.stages(), "s");
    let times = set_times(&bench.clients[0]);
    assert!(!times.is_empty() && times.iter().all(|generation| *generation == 0));
    assert!(!bench.clients[0].model.is_synced());
    // Then the transport gives the snapshot up; the sync goes again from
    // its 0xE5, one generation on.
    assert!(bench.until(1_000, |bench| bench.game().generation(0) == Some(1)));
    losing.store(false, Ordering::Relaxed);
    assert!(bench.until(3_000, |bench| bench.stages() == "L"));
    assert_eq!(bench.game().generation(0), Some(1));
    assert_eq!(bench.clients[0].model.generation.0, 1);
    assert_eq!(bench.clients[0].snapshots().len(), 1);
    assert_eq!(bench.clients[0].snapshots()[0].0, 1);
    assert_eq!(bench.clients[0].model.syncs, 1);
    assert!(bench.until(50, |bench| bench.clients[0].entered() == [0]));
    bench.assert_no_faults();
}

/// The snapshots a client took: whether each came compressed.
fn packed(client: &Client) -> Vec<bool> {
    client.got(|message| match message {
        ServerMessage::Snapshot { packed, .. } => Some(*packed),
        _ => None,
    })
}

#[test]
fn the_world_goes_out_compressed_unless_the_setting_says_no() {
    for compress in [true, false] {
        let settings = Settings {
            compress_state: compress,
            ..Settings::default()
        };
        let mut bench = Bench::new(2, MAP_BATTERIES, settings);
        // A world of the size live ones have: 14 KiB.
        for client in &mut bench.clients {
            client.sim.ballast = 13_000;
        }
        bench.join_all();
        settle(&mut bench);
        for client in &bench.clients {
            assert_eq!(packed(client), [compress]);
            assert_eq!(client.model.syncs, 1);
        }
        // The worlds the two loaded are the same bytes: their hashes agree.
        bench.run_long(10_000);
        assert_eq!(bench.game().resyncs(0) + bench.game().resyncs(1), 0);
        bench.assert_no_faults();
    }
}

/// A line that carries one full datagram in each of the bench's 5 ms
/// steps and drops the others; the small ones all pass. Live, 2026-10-13:
/// of a world's 28 datagrams sent in one millisecond, one or two reached
/// such a player. [`thin_step`] is a step on it.
fn thin_line(bench: &mut Bench, place: usize) -> Arc<AtomicUsize> {
    let left = Arc::new(AtomicUsize::new(1));
    let line = left.clone();
    bench.clients[place].lose = Some(Box::new(move |datagram| {
        // The allowance is taken by the first full datagram of the step.
        datagram.len() >= 400 && line.swap(0, Ordering::Relaxed) == 0
    }));
    left
}

fn thin_step(bench: &mut Bench, line: &AtomicUsize) {
    line.store(1, Ordering::Relaxed);
    bench.step(5);
}

#[test]
fn a_first_world_gets_through_a_line_that_carries_a_little_at_a_time() {
    // Uncompressed, so that the world is the 29 datagrams it was live.
    let settings = Settings {
        compress_state: false,
        ..Settings::default()
    };
    let mut bench = Bench::new(2, MAP_BATTERIES, settings);
    for client in &mut bench.clients {
        client.sim.ballast = 13_000;
    }
    bench.join(0);
    settle(&mut bench);

    bench.clients[1].slow_acks = true;
    bench.clients[1].connect();
    assert!(bench.until(1_000, |bench| bench.clients[1].model.options.is_some()));
    let line = thin_line(&mut bench, 1);
    bench.clients[1].ready();
    let started = bench.ms;
    while bench.stages() != "LL" && bench.ms < started + 4_000 {
        thin_step(&mut bench, &line);
    }
    // In, from the one world it was sent: the parts came a few at a time,
    // and the ones the line dropped came again while the rest was kept.
    assert_eq!(bench.stages(), "LL", "after {} ms", bench.ms - started);
    assert_eq!(bench.game().generation(1), Some(0));
    assert_eq!(bench.clients[1].snapshots().len(), 1);
    assert!(bench.until(50, |bench| bench.clients[1].entered() == [1]));
    for _ in 0..2_000 {
        thin_step(&mut bench, &line);
    }
    assert_eq!(bench.game().resyncs(1), 0);
    bench.assert_no_faults();
}

#[test]
fn a_game_waiting_for_its_first_world_gets_the_others_inputs_many_in_a_datagram() {
    let mut bench = Bench::new(4, MAP_BATTERIES, Settings::default());
    for place in 0..3 {
        bench.join(place);
    }
    settle(&mut bench);

    // The fourth player's world does not reach it for two seconds: its
    // game waits, acking as a waiting game does.
    bench.clients[3].slow_acks = true;
    bench.clients[3].connect();
    assert!(bench.until(1_000, |bench| bench.clients[3].model.options.is_some()));
    let losing = Arc::new(AtomicBool::new(true));
    let (datagrams, relays) = (Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0)));
    let (lost, with_relays, relayed) = (losing.clone(), datagrams.clone(), relays.clone());
    bench.clients[3].lose = Some(Box::new(move |datagram| {
        let Ok(parsed) = wire::parse(Table::Match, Direction::ToClient, datagram, 1) else {
            return false;
        };
        let has = |id: u8| {
            parsed
                .messages
                .iter()
                .filter(|message| message.id == id)
                .count()
        };
        if has(id::INPUT_RELAY) > 0 {
            with_relays.fetch_add(1, Ordering::Relaxed);
            relayed.fetch_add(has(id::INPUT_RELAY), Ordering::Relaxed);
        }
        lost.load(Ordering::Relaxed) && has(id::SNAPSHOT) > 0
    }));
    bench.clients[3].ready();
    assert!(bench.until(1_000, |bench| bench.stages() == "LLLs"));

    // The three who play make an input each tick.
    let mut made = BTreeSet::new();
    for tick in 0..40u32 {
        for (place, client) in bench.clients.iter_mut().take(3).enumerate() {
            let input = client.input(50, 0x21 + (tick & 1));
            made.insert((place as u8, input.time.tick));
        }
        bench.run(50);
    }
    assert_eq!(bench.stages(), "LLLs");
    // Every one of them reached the waiting game, in far fewer datagrams
    // than there were inputs: three came in each 50 ms, and so three went
    // in one datagram (and again together while not acked).
    let got: BTreeSet<_> = (0..3u8)
        .flat_map(|place| {
            bench.clients[3]
                .relays_of(place)
                .into_iter()
                .map(move |input| (place, input.time.tick))
        })
        .collect();
    assert_eq!(got, made);
    let (datagrams, relays) = (
        datagrams.load(Ordering::Relaxed),
        relays.load(Ordering::Relaxed),
    );
    assert!(relays >= 120, "{relays} relays");
    assert!(
        datagrams <= 90 && relays >= 2 * datagrams,
        "{relays} relays in {datagrams} datagrams"
    );

    // Its world comes after all, the same one: it is in, and in step.
    losing.store(false, Ordering::Relaxed);
    assert!(bench.until(1_500, |bench| bench.stages() == "LLLL"));
    assert_eq!(bench.game().generation(3), Some(0));
    assert_eq!(bench.clients[3].snapshots().len(), 1);
    settle(&mut bench);
    bench.run_long(10_000);
    assert_eq!(bench.game().resyncs(3), 0);
    bench.assert_no_faults();
}

#[test]
fn a_world_goes_out_only_once_its_time_was_answered() {
    let mut bench = Bench::new(1, MAP_BATTERIES, Settings::default());
    bench.clients[0].connect();
    assert!(bench.until(1_000, |bench| bench.clients[0].model.options.is_some()));
    // The client's answers to 0xE5 do not come: the server cannot know
    // that the 0xE5 itself (sent once) arrived, and sends it again, alone.
    bench.clients[0].mute_time_replies = true;
    bench.clients[0].ready();
    bench.run(400);
    assert_eq!(bench.stages(), "t");
    assert_eq!(set_times(&bench.clients[0]), [0, 0, 0]);
    assert!(bench.clients[0].snapshots().is_empty());
    // One is answered: 0xE5 once more with the world behind it, all of the
    // same generation.
    bench.clients[0].mute_time_replies = false;
    assert!(bench.until(1_000, |bench| bench.stages() == "L"));
    assert_eq!(set_times(&bench.clients[0]), [0, 0, 0, 0, 0]);
    assert_eq!(bench.clients[0].snapshots(), [(0, INITIAL_MS)]);
    assert_eq!(bench.clients[0].model.syncs, 1);
    bench.assert_no_faults();

    // A client that never answers gets its world all the same, after five.
    let mut bench = Bench::new(1, MAP_BATTERIES, Settings::default());
    bench.clients[0].connect();
    assert!(bench.until(1_000, |bench| bench.clients[0].model.options.is_some()));
    bench.clients[0].mute_time_replies = true;
    bench.clients[0].ready();
    assert!(bench.until(2_000, |bench| bench.stages() == "L"));
    assert_eq!(set_times(&bench.clients[0]), [0; 6]);
    assert_eq!(bench.clients[0].model.syncs, 1);
    assert_eq!(bench.game().generation(0), Some(0));
    bench.run(3_000);
    assert_eq!(bench.clients[0].model.syncs, 1);
    bench.assert_no_faults();
}

#[test]
fn an_upload_nobody_asked_for_and_a_forged_initial_world_are_not_used() {
    let mut bench = Bench::new(2, MAP_BATTERIES, Settings::default());
    // The first client's "initial" world already has kills in it.
    bench.clients[0].sim.kills_from = Some((0, 3));
    bench.clients[0].connect();
    assert!(bench.until(1_000, |bench| bench.clients[0].model.options.is_some()));
    bench.clients[0].ready();
    bench.run(1_000);
    assert_eq!(bench.stages(), "w-");
    assert!(!bench.game().has_clock());
    // An honest client's starts the match, for both.
    bench.clients[0].sim.kills_from = None;
    bench.join(1);
    assert!(bench.until(2_000, |bench| bench.stages() == "LL"));
    settle(&mut bench);
    let before = bench.game().observed();

    // A world sent unasked, with a won match in it, changes nothing.
    let time = bench.clock() / 50 * 50 - 500;
    let mut forged = bench.clients[0].sim.clone();
    forged.points_from = Some((0, [10, 0]));
    let requests = bench.clients[0].upload_requests;
    bench.clients[0].upload(time, &forged.world(time));
    bench.run(300);
    if bench.clients[0].upload_requests == requests {
        assert_eq!(bench.game().observed(), before);
    }
    bench.run(5_000);
    let (_, standing) = bench.game().observed().unwrap();
    assert_eq!(standing.unwrap().points, [0, 0]);
    assert!(bench.results.try_recv().is_err());
    bench.assert_no_faults();
}

#[test]
fn a_player_who_leaves_is_seen_to_go_and_an_empty_match_is_over() {
    let mut bench = Bench::new(2, MAP_BATTERIES, Settings::default());
    bench.join_all();
    settle(&mut bench);
    // 0xE0, empty: the other sees it go; nothing is sent back to the
    // leaver about itself.
    bench.clients[1].leave();
    assert!(bench.until(500, |bench| bench.clients[0].left() == [1]));
    assert_eq!(bench.stages(), "Lg");
    assert!(!bench.clients[1].told_to_leave);
    assert!(bench.clients[0].left() == [1] && !bench.clients[0].closed);
    // The match goes on for the one left.
    bench.run(3_000);
    assert!(bench.results.try_recv().is_err());
    let relayed = bench.clients[1].relays_of(0).len();
    bench.clients[0].input(50, 1);
    bench.run(200);
    assert_eq!(bench.clients[1].relays_of(0).len(), relayed);

    // The last one leaves: the match is over at once, with a result.
    bench.clients[0].leave();
    bench.run(1_500);
    let result = bench.results.try_recv().expect("a result");
    assert_eq!(result.end, MatchEnd::Deserted);
    assert!(
        result
            .players
            .iter()
            .all(|player| player.entered && player.left)
    );
    assert!(result.players.iter().all(|player| !player.present_at_end));
    assert!(result.players[0].time_in_match_ms > result.players[1].time_in_match_ms);
    assert_eq!(result.played_ms, None, "the countdown was still running");
    assert_eq!(bench.registry.match_count(), 0);
    assert_eq!(bench.registry.seat(session_of(0)), None);
    bench.assert_no_faults();
}

#[test]
fn a_client_that_falls_silent_times_out() {
    let mut bench = Bench::new(2, MAP_BATTERIES, Settings::default());
    bench.join_all();
    settle(&mut bench);
    bench.clients[1].silent = true;
    bench.run_long(9_000);
    assert_eq!(bench.stages(), "LL");
    assert!(bench.clients[0].left().is_empty());
    // 10 s without a datagram.
    assert!(bench.until(2_000, |bench| bench.clients[0].left() == [1]));
    assert_eq!(&bench.stages()[..1], "L");
    // The one left plays on, and is never blamed for the other's silence.
    bench.run_long(10_000);
    assert_eq!(bench.clients[0].model.syncs, 1);
    assert!(!bench.clients[0].closed);
    bench.assert_no_faults();
}

#[test]
fn a_player_back_in_the_match_enters_for_the_others_first() {
    let mut bench = Bench::new(2, MAP_BATTERIES, Settings::default());
    bench.join_all();
    settle(&mut bench);
    bench.clients[1].drop_link();
    assert!(bench.until(500, |bench| bench.clients[0].left() == [1]));
    bench.run(2_000);

    // Its game starts anew, from another port, with the same session.
    bench.restart(1);
    bench.clients[1].connect();
    assert!(bench.until(1_000, |bench| bench.clients[1].model.options.is_some()));
    bench.clients[1].ready();
    // The other sees it enter again before it has a world to start from.
    let entries = |client: &Client| {
        let mut ticks = client.got(|message| match message {
            ServerMessage::PlayerEntered { tick, player: 1 } => Some(*tick),
            _ => None,
        });
        ticks.dedup();
        ticks.len()
    };
    assert!(bench.until(500, |bench| entries(&bench.clients[0]) == 2));
    assert_ne!(bench.stages(), "LL");
    // Then it starts from a snapshot taken after that entry, which has its
    // player as the other has it: no entry of its own is sent to it.
    assert!(bench.until(10_000, |bench| bench.stages() == "LL"));
    assert!(bench.clients[1].model.is_synced());
    assert_eq!(entries(&bench.clients[1]), 0);
    assert_eq!(entries(&bench.clients[0]), 2);
    // Its world is the other's: nobody is resynced afterwards.
    bench.run_long(15_000);
    assert_eq!(bench.clients[0].model.syncs, 1);
    assert_eq!(bench.clients[1].model.syncs, 1);
    settle(&mut bench);
    bench.assert_no_faults();
}

/// Live, 2026-10-15: a player back in was put 306 ms behind the match clock
/// (its snapshot + `SYNC_LEAD_MS`) while the other games had its player
/// since its entry; its inputs of the first second came 250-400 ms late,
/// 9 were dropped at the edge of its history and it was out of step 2 s
/// after its load. Now its game goes at the match clock, as a resynced
/// one's, and its inputs reach the other in time.
#[test]
fn a_player_back_in_the_match_starts_at_the_match_clock() {
    let mut bench = Bench::new(2, MAP_BATTERIES, Settings::default());
    // The other's uploads come late: the world the player back in starts
    // from is older than the lead.
    bench.one_way = vec![150, 0];
    bench.join_all();
    settle(&mut bench);
    bench.clients[1].drop_link();
    assert!(bench.until(500, |bench| bench.clients[0].left() == [1]));
    bench.run(2_000);
    bench.restart(1);
    bench.clients[1].connect();
    assert!(bench.until(1_000, |bench| bench.clients[1].model.options.is_some()));
    bench.clients[1].ready();
    assert!(bench.until(5_000, |bench| !set_times(&bench.clients[1]).is_empty()));
    let arrived = bench.clock();
    let client = &bench.clients[1];
    let at = first(client, |message| {
        matches!(message, ServerMessage::SetTime { .. })
    })
    .unwrap();
    let ServerMessage::SetTime { match_time, .. } = client.taken[at] else {
        unreachable!()
    };
    assert!(match_time + 20 >= arrived, "{match_time} {arrived}");
    assert!(bench.until(10_000, |bench| bench.stages() == "LL"));
    // An input it makes at once on its own clock is the other's, and is
    // never dropped.
    let input = bench.clients[1].input(0, 0x5);
    bench.run(1_000);
    assert!(bench.clients[0].relays_of(1).contains(&input));
    assert!(discards(&bench.clients[1]).is_empty());
    bench.run_long(15_000);
    assert_eq!(bench.clients[0].model.syncs, 1);
    assert_eq!(bench.clients[1].model.syncs, 1);
    bench.assert_no_faults();
}

/// A game on a slow line that starts from a world older than another
/// player's entry gets that entry before its world is loaded, never past
/// its horizon. Live, 2026-10-15: it came once the load was proven, 600 ms
/// and more past its tick, the game refused it ("too old", 0x2D15C2) and
/// was out of step 3 s into the match (two of two big starts).
#[test]
fn a_slow_starter_gets_the_entries_since_its_world_in_time() {
    let bench = slow_start(300);
    let (_, world) = bench.clients[2].snapshots()[0];
    // The entry came with its first world, which was not done again.
    assert_eq!(generations(&bench.clients[2]), [0]);
    assert!(
        bench
            .game()
            .events()
            .iter()
            .any(|(at, who, entered)| *who == 1 && *entered && *at > world)
    );
    assert!(bench.clients[2].entered().contains(&1));
}

/// So slow a line that its 0xE5 goes unanswered (the world goes all the
/// same, and its entries cannot go ahead of it, `Peer::time_answered`):
/// the entry would reach it past its horizon, and its start is done again
/// from a world that has the player.
#[test]
fn a_starter_too_slow_for_its_entries_starts_again_from_a_world_with_them() {
    let bench = slow_start(600);
    assert_eq!(generations(&bench.clients[2]), [0, 1]);
    let (_, world) = bench.clients[2].snapshots()[1];
    assert!(
        bench
            .game()
            .events()
            .iter()
            .any(|(at, who, entered)| *who == 1 && *entered && *at < world)
    );
}

/// Player 2, `one_way` ms each way, is timed for its first world when
/// player 1 enters, after that world's tick; 15 s on, no entry was refused
/// and it was never resynced.
fn slow_start(one_way: u64) -> Bench {
    let mut bench = Bench::new(3, MAP_BATTERIES, Settings::default());
    bench.one_way = vec![0, 0, one_way];
    bench.join(0);
    for place in [1, 2] {
        bench.clients[place].connect();
        assert!(bench.until(5_000, |bench| bench.clients[place].model.options.is_some()));
    }
    bench.clients[2].ready();
    // Timed for its world while the next player gets in, after that
    // world's tick.
    assert!(bench.until(10_000, |bench| bench.stages().as_bytes().get(2)
        == Some(&b't')));
    bench.clients[1].ready();
    assert!(bench.until(10_000, |bench| bench.stages() == "LLL"));
    bench.run_long(15_000);
    let refused: Vec<_> = bench.clients[2]
        .skipped
        .iter()
        .filter(|(id, _)| *id == id::PLAYER_ENTERED)
        .collect();
    assert!(refused.is_empty(), "{refused:?}");
    assert_eq!(bench.game().resyncs(2), 0);
    bench.assert_no_faults();
    bench
}

/// A Team Deathmatch of three in play, with tickets gone on both sides.
fn deathmatch_under_way(tickets: [u16; 2]) -> Bench {
    use survarium::v026g::match_msgs::snapshot::Tdm;
    let mut bench = Bench::new(3, MAP_TDM, Settings::default());
    bench.join_all();
    settle(&mut bench);
    bench.run_long(25_000);
    assert_eq!(bench.game().rule_state(), state::IN_PROCESS);
    let at = bench.clock() / 50 * 50 + 100;
    for client in &mut bench.clients {
        client.sim.tdm_from = Some((
            at,
            Tdm {
                tickets,
                alive_players_mask: 0b111,
                active_players_mask: 0b111,
            },
        ));
    }
    bench.run_long(6_000);
    bench
}

/// The player's game is started anew and comes back to the match; its
/// world has the tickets the others' have.
fn come_back(bench: &mut Bench, place: usize) {
    let tdm = bench.clients[0].sim.tdm_from;
    bench.clients[place].drop_link();
    bench.run(2_000);
    bench.restart(place);
    bench.clients[place].sim.tdm_from = tdm;
    bench.clients[place].connect();
    assert!(bench.until(1_000, |bench| {
        bench.clients[place].model.options.is_some()
    }));
    bench.clients[place].ready();
}

/// Seen live on 2026-10-12, 4 times of 4: a player back in a Team
/// Deathmatch was out of step from its first tick (its game has the
/// tickets of the start in its statistics: the client's own defect, which
/// the test client has too), was resynced 7 s later and was right from
/// then on. It is started once more as soon as it has its first world.
#[test]
fn a_player_back_in_a_team_deathmatch_is_started_twice_and_is_in_step_at_once() {
    let mut bench = deathmatch_under_way([88, 78]);
    come_back(&mut bench, 2);
    assert!(bench.until(10_000, |bench| bench.clients[2].model.syncs == 1));
    let first_at = bench.ms;
    // The second world follows the first within a second: no wait for its
    // reports to be wrong, nor for its clock.
    assert!(bench.until(1_000, |bench| bench.clients[2].model.syncs == 2));
    assert!(bench.ms - first_at <= 1_000);
    assert!(bench.until(2_000, |bench| bench.stages() == "LLL"));
    assert_eq!(bench.game().reloads(2), 1);
    assert_eq!(generations(&bench.clients[2]), [0, 1]);
    let snapshots = bench.clients[2].snapshots();
    assert_eq!(snapshots.len(), 2);
    assert!(snapshots[1].1 >= snapshots[0].1);
    // It entered once for the others, before its first world, and no
    // entry of its own was sent to it.
    let entries = |client: &Client| {
        let mut ticks = client.got(|message| match message {
            ServerMessage::PlayerEntered { tick, player: 2 } => Some(*tick),
            _ => None,
        });
        ticks.dedup();
        ticks.len()
    };
    assert_eq!(entries(&bench.clients[0]), 2);
    assert_eq!(entries(&bench.clients[2]), 0);
    assert_eq!(bench.clients[0].left(), [2]);

    // In step from there: its reports agree, nobody is resynced, and the
    // others were never started again.
    bench.run_long(30_000);
    assert_eq!(bench.game().resyncs(2), 0);
    assert_eq!(bench.game().reloads(2), 1);
    assert_eq!(bench.clients[2].model.syncs, 2);
    assert_eq!(bench.clients[0].model.syncs, 1);
    assert_eq!(bench.clients[1].model.syncs, 1);
    assert_eq!((bench.game().resyncs(0), bench.game().resyncs(1)), (0, 0));
    settle(&mut bench);
    bench.assert_no_faults();
}

/// With the tickets still the ones of the start the first world is right
/// as it is: one start, as in the other modes.
#[test]
fn a_player_back_in_a_team_deathmatch_nobody_died_in_is_started_once() {
    let mut bench = deathmatch_under_way([100, 100]);
    come_back(&mut bench, 2);
    assert!(bench.until(10_000, |bench| bench.stages() == "LLL"));
    bench.run_long(30_000);
    assert_eq!(bench.game().reloads(2), 0);
    assert_eq!(bench.game().resyncs(2), 0);
    assert_eq!(bench.clients[2].model.syncs, 1);
    settle(&mut bench);
    bench.assert_no_faults();
}

/// A player who is in a Team Deathmatch for the first time when it is in
/// play already has the same wrong statistics. It is not started twice (its
/// entry comes after its first world, and a second one would have to wait
/// for a world newer than that): the resync that follows wrong reports puts
/// it right, as it did for everyone until 2026-10-12.
#[test]
fn a_first_entry_into_a_team_deathmatch_in_play_is_put_right_by_a_resync() {
    use survarium::v026g::match_msgs::snapshot::Tdm;
    let mut bench = Bench::new(3, MAP_TDM, Settings::default());
    bench.join(0);
    bench.join(1);
    settle(&mut bench);
    bench.run_long(30_000);
    assert_eq!(bench.game().rule_state(), state::IN_PROCESS);
    let at = bench.clock() / 50 * 50 + 100;
    let tdm = Some((
        at,
        Tdm {
            tickets: [91, 64],
            alive_players_mask: 0b011,
            active_players_mask: 0b011,
        },
    ));
    for client in &mut bench.clients {
        client.sim.tdm_from = tdm;
    }
    bench.run_long(6_000);
    bench.join(2);
    assert_eq!(bench.game().reloads(2), 0);
    assert!(bench.until(30_000, |bench| bench.game().resyncs(2) == 1));
    assert!(bench.until(5_000, |bench| bench.stages() == "LLL"));
    bench.run_long(30_000);
    assert_eq!(bench.game().resyncs(2), 1);
    assert_eq!(bench.clients[2].model.syncs, 2);
    bench.assert_no_faults();
}

/// The second start needs somebody to be in step with. The only other
/// player leaves just as the first world is loaded: the player back in is
/// not left waiting for a world nobody can give.
#[test]
fn a_player_back_in_a_team_deathmatch_is_not_left_waiting_when_the_rest_go() {
    use survarium::v026g::match_msgs::snapshot::Tdm;
    for leave_after_ms in [0, 20, 60, 150, 400] {
        let mut bench = Bench::new(2, MAP_TDM, Settings::default());
        bench.join_all();
        settle(&mut bench);
        bench.run_long(25_000);
        let at = bench.clock() / 50 * 50 + 100;
        let tdm = Some((
            at,
            Tdm {
                tickets: [70, 95],
                alive_players_mask: 0b11,
                active_players_mask: 0b11,
            },
        ));
        for client in &mut bench.clients {
            client.sim.tdm_from = tdm;
        }
        bench.run_long(6_000);
        bench.clients[1].drop_link();
        bench.run(2_000);
        bench.restart(1);
        bench.clients[1].sim.tdm_from = tdm;
        bench.clients[1].connect();
        assert!(bench.until(1_000, |bench| bench.clients[1].model.options.is_some()));
        bench.clients[1].ready();
        assert!(bench.until(10_000, |bench| bench.clients[1].model.syncs == 1));
        bench.run(leave_after_ms);
        bench.clients[0].leave();
        // Whatever stage its second start was at: it plays on.
        assert!(
            bench.until(8_000, |bench| bench.stages() == "gL"),
            "left {leave_after_ms} ms after the first world: {}",
            bench.stages()
        );
        assert!(bench.clients[1].model.is_synced());
        assert!(!bench.clients[1].closed);
        bench.run_long(5_000);
        assert_eq!(bench.stages(), "gL");
        bench.assert_no_faults();
    }
}

/// The games of a match of three all stop in one tick (live on
/// 2026-10-12, three times: a spawn that ends every client). The players
/// of two close theirs; the third stays open, its link alive and its world
/// still. `frozen_reports`: that one goes on sending the hash of the tick
/// it stopped at, as a game whose window is not active does.
fn every_game_stops(frozen_reports: bool) -> Bench {
    let mut bench = Bench::new(3, MAP_BATTERIES, Settings::default());
    bench.join_all();
    settle(&mut bench);
    bench.run_long(25_000);
    assert_eq!(bench.game().rule_state(), state::IN_PROCESS);
    let stopped_at = bench.clients[0].horizon().expect("a world");
    bench.clients[0].frozen_at = Some(stopped_at);
    bench.clients[1].silent = true;
    bench.clients[2].silent = true;
    // The two closed games are timed out; the open one is still "live".
    for _ in 0..120 {
        if frozen_reports {
            bench.clients[0].report(stopped_at, 0x1234_5678);
        }
        bench.run(100);
    }
    assert_eq!(&bench.stages()[..1], "L", "{}", bench.stages());
    assert!(bench.results.try_recv().is_err());
    // One of the two starts its game again and presses Play: back to its
    // place, loaded, and waiting for a world.
    bench.restart(1);
    bench.clients[1].connect();
    assert!(bench.until(1_000, |bench| bench.clients[1].model.options.is_some()));
    bench.clients[1].ready();
    bench.run(1_000);
    assert_eq!(&bench.stages()[..2], "Lw", "{}", bench.stages());
    for _ in 0..300 {
        if frozen_reports {
            bench.clients[0].report(stopped_at, 0x1234_5678);
        }
        bench.run(100);
    }
    bench
}

/// Live on 2026-10-12 (match 0x1f793b8): every game of a match stopped,
/// the players came back one after another and each sat on the "awaiting
/// other players" screen for as long as it stayed, for nine and a half
/// minutes, because the only world there was to start from was a dead
/// game's and the match was not over while one of them was connected. A
/// match that nobody simulates and that a player has waited 20 s to get
/// into is over: the player is told (0xEA with its own index, which ends
/// the match on a game with no clock too) and can play another.
#[test]
fn a_match_whose_games_all_stopped_is_over_for_a_player_who_comes_back() {
    for frozen_reports in [false, true] {
        let bench = every_game_stops(frozen_reports);
        let result = bench
            .results
            .try_recv()
            .unwrap_or_else(|_| panic!("the match goes on: {}", bench.stages()));
        assert_eq!(result.end, MatchEnd::Deserted);
        // The player that waited never had a time: it is told with the
        // tick 0, which its game refuses and closes the match on.
        let waiting = &bench.clients[1];
        assert!(waiting.told_to_leave, "reports: {frozen_reports}");
        assert!(
            waiting
                .skipped
                .contains(&(id::PLAYER_LEFT, Skip::OlderThanResync))
        );
        // The game that stood still is told too.
        assert!(bench.clients[0].told_to_leave);
        bench.assert_no_faults();
    }
}

/// A player waits for a world that no client can give (the one that plays
/// has no upload), while that client plays on: the match is its, and goes
/// on. Its hash reports show that its game simulates.
#[test]
fn a_player_waiting_for_a_world_does_not_end_a_match_that_is_played() {
    let mut bench = Bench::new(2, MAP_BATTERIES, Settings::default());
    bench.join_all();
    settle(&mut bench);
    bench.run_long(25_000);
    bench.clients[1].drop_link();
    bench.run(2_000);
    // From here the client that plays answers no upload request.
    bench.clients[0].model.patched = false;
    bench.restart(1);
    bench.clients[1].connect();
    assert!(bench.until(1_000, |bench| bench.clients[1].model.options.is_some()));
    bench.clients[1].ready();
    bench.run_long(60_000);
    assert_eq!(bench.stages(), "Lw");
    assert!(bench.results.try_recv().is_err());
    assert!(!bench.clients[0].told_to_leave);
    assert!(!bench.clients[1].told_to_leave);
}

/// Seen live on 2026-10-12: a player left a match of two by the menu,
/// pressed Play and was sent back to the match it had left, three times.
/// 0xE0 is the player's own word; a link that broke says nothing.
#[test]
fn a_player_who_left_by_its_own_word_has_given_its_place_up() {
    let mut bench = Bench::new(3, MAP_BATTERIES, Settings::default());
    bench.join_all();
    settle(&mut bench);

    // A link that broke: the place is kept for the player.
    bench.clients[2].drop_link();
    assert!(bench.until(500, |bench| bench.clients[0].left() == [2]));
    assert_eq!(
        bench.registry.seat(session_of(2)).map(|seat| seat.place),
        Some(2)
    );

    // 0xE0: the others see it go as they saw the other, and its session
    // has no place any more. The match goes on for the one who plays.
    bench.clients[1].leave();
    assert!(bench.until(500, |bench| bench.clients[0].left() == [2, 1]));
    assert_eq!(bench.registry.seat(session_of(1)), None);
    assert_eq!(
        bench.registry.seat(session_of(0)).map(|seat| seat.place),
        Some(0)
    );
    assert!(bench.registry.is_open(bench.match_id));

    // Its session can be put in another match at once.
    let mut another = super::setup(1, MAP_BATTERIES);
    another.players[0].session_id = session_of(1);
    let second = bench.registry.create(another).expect("another match");
    assert_ne!(second.match_id, bench.match_id);
    bench.registry.end(second.match_id);

    // A game that comes for the place it gave up is turned away; the one
    // whose link broke is taken back.
    bench.run(2_000);
    bench.restart(1);
    bench.clients[1].connect();
    assert!(bench.until(1_000, |bench| bench.clients[1].disconnected));
    assert!(bench.clients[1].model.options.is_none());
    bench.restart(2);
    bench.clients[2].connect();
    assert!(bench.until(1_000, |bench| bench.clients[2].model.options.is_some()));
    bench.clients[2].ready();
    assert!(bench.until(10_000, |bench| bench.clients[2].model.is_synced()));

    // The result has the place as the account's that left.
    bench.clients[0].leave();
    bench.clients[2].leave();
    assert!(bench.until(5_000, |bench| bench.registry.match_count() == 0));
    let result = bench.results.try_recv().expect("the match's result");
    assert_eq!(result.players[1].account_id, 101);
    assert!(result.players[1].left && !result.players[1].present_at_end);
}

#[test]
fn a_player_who_signed_in_again_comes_back_under_its_new_session() {
    let mut bench = Bench::new(2, MAP_BATTERIES, Settings::default());
    bench.join_all();
    settle(&mut bench);
    bench.clients[1].drop_link();
    assert!(bench.until(500, |bench| bench.clients[0].left() == [1]));
    bench.run(2_000);

    // Its game starts anew and signs in again: another session. Until the
    // lobby says whose place it is, the match server turns it away.
    const NEW_SESSION: u32 = 0x2001;
    bench.restart(1);
    bench.clients[1].session = NEW_SESSION;
    bench.clients[1].connect();
    assert!(bench.until(1_000, |bench| bench.clients[1].disconnected));
    assert!(bench.clients[1].model.options.is_none());

    // The lobby gives the account's place (account 101, place 1) to the
    // new session: the client is taken, and gets back into the match.
    let seat = bench.registry.reseat(bench.match_id, 101, NEW_SESSION);
    assert_eq!(seat.map(|seat| seat.place), Some(1));
    assert_eq!(bench.registry.seat(session_of(1)), None);
    bench.restart(1);
    bench.clients[1].session = NEW_SESSION;
    bench.clients[1].connect();
    assert!(bench.until(1_000, |bench| bench.clients[1].model.options.is_some()));
    assert_eq!(bench.clients[1].model.local_index, Some(1));
    bench.clients[1].ready();
    assert!(bench.until(10_000, |bench| bench.stages() == "LL"));
    assert!(bench.clients[1].model.is_synced());
    assert!(bench.game().sessions().contains(&NEW_SESSION));
    assert!(!bench.game().sessions().contains(&session_of(1)));
    bench.run_long(8_000);
    assert_eq!(bench.clients[0].model.syncs, 1);
    settle(&mut bench);

    // The result names the session the player last came with.
    bench.clients[0].leave();
    bench.clients[1].leave();
    assert!(bench.until(5_000, |bench| bench.registry.match_count() == 0));
    let result = bench.results.try_recv().expect("the match's result");
    assert_eq!(result.players[0].session_id, session_of(0));
    assert_eq!(result.players[1].session_id, NEW_SESSION);
    assert_eq!(result.players[1].account_id, 101);
    bench.assert_no_faults();
}

#[test]
fn a_team_with_every_battery_home_ends_the_match_three_seconds_later() {
    let mut bench = Bench::new(2, MAP_BATTERIES, Settings::default());
    bench.join_all();
    settle(&mut bench);
    // The countdown, and enough play for ten batteries to be possible.
    bench.run_long(130_000);
    assert_eq!(bench.game().rule_state(), state::IN_PROCESS);
    assert!(bench.results.try_recv().is_err());

    // Team 0 brings the tenth battery home; player 0 has 5 kills.
    let at = bench.clock() / 50 * 50 + 100;
    for client in &mut bench.clients {
        client.sim.points_from = Some((at, [10, 3]));
        client.sim.kills_from = Some((at, 5));
    }
    // The next world asked for shows it, both clients vouch for it, and
    // 3 s after that world's time the match is over for both.
    assert!(bench.until(12_000, |bench| {
        bench.clients.iter().all(|client| client.closed)
    }));
    let result = bench.results.try_recv().expect("a result");
    assert_eq!(result.end, MatchEnd::ModeRule);
    assert_eq!(result.winner, Some(0));
    assert_eq!(result.team_points, [10, 3]);
    assert_eq!(result.trusted_by, 2);
    let ended_after = result.duration_ms - at;
    assert!((3_000..9_500).contains(&ended_after), "{ended_after}");
    assert!(result.played_ms.unwrap() > 100_000);
    assert_eq!((result.players[0].kills, result.players[0].deaths), (5, 0));
    assert_eq!((result.players[1].kills, result.players[1].deaths), (0, 5));
    assert!(result.players[0].score > 0);
    assert!(result.players.iter().all(|player| player.present_at_end));
    assert_eq!(result.players[0].team, 0);
    assert_eq!(result.players[1].team, 1);
    // Each was told with its own index.
    assert_eq!(bench.clients[0].left(), [0]);
    assert_eq!(bench.clients[1].left(), [1]);
    bench.assert_no_faults();
}

#[test]
fn a_players_battery_events_are_paid_at_most_the_maps_battery_count_per_kind() {
    // Map 61 has 10 batteries (docs/notes/battery_credit.md).
    let mut bench = Bench::new(2, MAP_BATTERIES, Settings::default());
    bench.join_all();
    settle(&mut bench);
    bench.run_long(130_000);
    // Player 0 put a battery in 233 times; player 1 stole three and a half
    // batteries' worth and put 4 in: under the cap.
    let at = bench.clock() / 50 * 50 + 100;
    let events = vec![
        (at, 0, event::PUT_VICTORY_ITEM_IN_CONTAINER, 233),
        (at, 1, event::STEAL_VICTORY_ITEM, 350),
        (at, 1, event::PUT_VICTORY_ITEM_IN_CONTAINER, 4),
    ];
    for client in &mut bench.clients {
        client.sim.points_from = Some((at, [10, 3]));
        client.sim.events_from.clone_from(&events);
    }
    assert!(bench.until(12_000, |bench| {
        bench.clients.iter().all(|client| client.closed)
    }));
    let result = bench.results.try_recv().expect("a result");
    let paid = |place: usize, event: usize| {
        result.players[place]
            .statistics
            .as_ref()
            .expect("statistics")
            .total(event)
    };
    assert_eq!(paid(0, event::PUT_VICTORY_ITEM_IN_CONTAINER), 10);
    // 10 put-ins at 5 each, not 233.
    assert_eq!(result.players[0].score, 50);
    assert_eq!(paid(1, event::STEAL_VICTORY_ITEM), 350);
    assert_eq!(paid(1, event::PUT_VICTORY_ITEM_IN_CONTAINER), 4);
    bench.assert_no_faults();
}

#[test]
fn results_one_client_claims_alone_do_not_count() {
    let mut bench = Bench::new(2, MAP_BATTERIES, Settings::default());
    bench.join_all();
    settle(&mut bench);
    bench.run_long(130_000);
    // One client alone says its team has every battery home. The two
    // disagree from then on, with no majority; nothing either says alone
    // is believed, and the match does not end by it.
    let at = bench.clock() / 50 * 50 + 100;
    bench.clients[0].sim.points_from = Some((at, [10, 0]));
    bench.run_long(12_000);
    assert!(bench.results.try_recv().is_err());
    let (_, standing) = bench.game().observed().expect("the last agreed world");
    assert_eq!(standing.unwrap().points, [0, 0]);
    assert!(!bench.clients[0].closed && !bench.clients[1].closed);
    bench.assert_no_faults();
}

#[test]
fn a_match_nobody_comes_to_is_given_up() {
    let mut bench = Bench::new(2, MAP_TDM, Settings::default());
    bench.run_long(119_000);
    assert!(bench.results.try_recv().is_err());
    assert_eq!(bench.registry.match_count(), 1);
    bench.run_long(3_000);
    let result = bench.results.try_recv().expect("a result");
    assert_eq!(result.end, MatchEnd::NoShow);
    assert_eq!(result.game_mode, 4);
    assert_eq!(result.winner, None);
    assert_eq!(result.trusted_by, 0);
    assert_eq!(result.players.len(), 2);
    assert!(result.players.iter().all(|player| !player.entered));
    assert_eq!(bench.registry.match_count(), 0);
    // Its players can be put in another match.
    assert!(bench.registry.create(super::setup(2, MAP_TDM)).is_ok());
}

#[test]
fn a_client_with_no_seat_and_one_that_never_connects_are_told_to_go() {
    let mut bench = Bench::new(1, MAP_BATTERIES, Settings::default());
    // A session the registry does not know.
    bench.clients[0] = Client::new(0xDEAD, true, 1, MAP_BATTERIES);
    bench.clients[0].connect();
    assert!(bench.until(1_000, |bench| bench.clients[0].disconnected));
    assert!(bench.clients[0].taken.is_empty());
    assert!(bench.server.games.is_empty());

    // An address that sends without ever connecting (its match is gone):
    // answered for three seconds, then told to disconnect.
    bench.restart(0);
    for _ in 0..29 {
        bench.clients[0].send(low_level::CONTINUOUS_FLOW, &[], false);
        bench.run(100);
    }
    assert!(!bench.clients[0].disconnected);
    for _ in 0..3 {
        bench.clients[0].send(low_level::CONTINUOUS_FLOW, &[], false);
        bench.run(100);
    }
    assert!(bench.until(1_000, |bench| bench.clients[0].disconnected));
    bench.assert_no_faults();
}

/// Seen live on 2026-10-12 (10:28:22 UTC): a client's first datagram, the
/// one with its 0xD8, was lost on the way. Its keep-alives of the next
/// 300 ms were three datagrams "without connecting", it was told to
/// disconnect, and its 0xD8 of 500 ms never came: the player got in 32 s
/// later through a new sign-in. The client sends 0xD8 again every 500 ms
/// for as long as it takes, so the server waits for it.
#[test]
fn a_client_whose_first_datagram_was_lost_still_connects() {
    for lost in 1..=4 {
        let mut bench = Bench::new(1, MAP_BATTERIES, Settings::default());
        // Each lost 0xD8 is followed by the keep-alives of its 500 ms.
        for _ in 0..lost {
            for _ in 0..5 {
                bench.clients[0].send(low_level::CONTINUOUS_FLOW, &[], false);
                bench.run(100);
            }
        }
        assert!(!bench.clients[0].disconnected, "after {lost} lost");
        assert!(bench.server.games.is_empty());
        // The next 0xD8 arrives.
        bench.clients[0].connect();
        assert!(
            bench.until(1_000, |bench| bench.clients[0].model.options.is_some()),
            "after {lost} lost"
        );
        assert!(!bench.clients[0].disconnected);
        bench.clients[0].ready();
        assert!(bench.until(10_000, |bench| bench.stages() == "L"));
        bench.assert_no_faults();
    }
}

#[test]
fn a_team_deathmatch_ends_when_a_team_has_no_tickets_and_nobody_alive() {
    use survarium::v026g::match_msgs::snapshot::Tdm;
    let mut bench = Bench::new(2, MAP_TDM, Settings::default());
    bench.join_all();
    settle(&mut bench);
    bench.run_long(25_000);
    assert_eq!(bench.game().rule_state(), state::IN_PROCESS);
    // Tickets left are the points: nothing is over.
    let (_, standing) = bench.game().observed().expect("a trusted snapshot");
    assert_eq!(standing.unwrap().points, [100, 100]);
    assert!(bench.results.try_recv().is_err());

    // Team 1 (player 1) has no tickets and nobody alive.
    let at = bench.clock() / 50 * 50 + 100;
    for client in &mut bench.clients {
        client.sim.tdm_from = Some((
            at,
            Tdm {
                tickets: [37, 0],
                alive_players_mask: 0b01,
                active_players_mask: 0b11,
            },
        ));
    }
    assert!(bench.until(12_000, |bench| {
        bench.clients.iter().all(|client| client.closed)
    }));
    let result = bench.results.try_recv().expect("a result");
    assert_eq!(result.end, MatchEnd::ModeRule);
    assert_eq!(result.game_mode, 4);
    assert_eq!(result.winner, Some(0));
    assert_eq!(result.team_points, [37, 0]);
    bench.assert_no_faults();
}

#[test]
fn with_no_majority_the_client_that_never_got_a_relay_is_the_one_resynced() {
    let mut bench = Bench::new(2, MAP_BATTERIES, Settings::default());
    bench.join_all();
    settle(&mut bench);
    bench.run(3_000);

    // The first player's link drops every datagram with a relay in it for
    // a moment: the second player's input never reaches it (a relay goes
    // until its game's history has passed its tick, some 450 ms), and its
    // world goes on without that input.
    let losing = Arc::new(AtomicBool::new(true));
    let lost = losing.clone();
    bench.clients[0].lose = Some(Box::new(move |datagram| {
        lost.load(Ordering::Relaxed)
            && wire::parse(Table::Match, Direction::ToClient, datagram, 1).is_ok_and(|parsed| {
                parsed
                    .messages
                    .iter()
                    .any(|message| message.id == id::INPUT_RELAY)
            })
    }));
    let from = bench.clients[0].clock();
    bench.clients[1].input(50, 0x21);
    bench.run(600);
    losing.store(false, Ordering::Relaxed);
    assert!(bench.clients[0].relays_of(1).is_empty());
    bench.clients[0].sim.salt_from = Some((from, 3));

    // Two clients, two hashes, no majority; the one that joined last would
    // be the suspect. But the server knows who missed something.
    assert!(bench.until(12_000, |bench| bench.game().resyncs(0) == 1));
    assert!(bench.until(5_000, |bench| bench.clients[0].model.syncs == 2));
    assert_eq!(generations(&bench.clients[0]), [0, 1]);
    bench.run_long(20_000);
    assert_eq!(bench.game().resyncs(1), 0);
    assert_eq!(bench.clients[1].model.syncs, 1);
    assert_eq!(bench.clients[0].model.syncs, 2);
    bench.assert_no_faults();
}

/// A relay a client missed long before a disagreement says nothing about
/// it: had it mattered, the client would have been wrong from that tick.
/// (On a long line some relay is always unacked; the tie went against
/// such a client whoever was wrong, live 2026-10-15.)
#[test]
fn with_no_majority_a_relay_missed_long_before_names_nobody() {
    let mut bench = Bench::new(2, MAP_BATTERIES, Settings::default());
    bench.join_all();
    settle(&mut bench);
    bench.run(3_000);

    let losing = Arc::new(AtomicBool::new(true));
    let lost = losing.clone();
    bench.clients[0].lose = Some(Box::new(move |datagram| {
        lost.load(Ordering::Relaxed)
            && wire::parse(Table::Match, Direction::ToClient, datagram, 1).is_ok_and(|parsed| {
                parsed
                    .messages
                    .iter()
                    .any(|message| message.id == id::INPUT_RELAY)
            })
    }));
    bench.clients[1].input(50, 0x21);
    bench.run(600);
    losing.store(false, Ordering::Relaxed);
    assert!(bench.clients[0].relays_of(1).is_empty());

    // Its world did not change for it (the model's hash is its salt's), and
    // seconds later the other one goes wrong: that one is resynced.
    bench.run_long(5_000);
    assert_eq!(bench.game().resyncs(0), 0);
    let from = bench.clients[1].clock();
    bench.clients[1].sim.salt_from = Some((from, 3));
    assert!(bench.until(12_000, |bench| bench.game().resyncs(1) == 1));
    bench.run_long(10_000);
    assert_eq!(bench.game().resyncs(0), 0);
    bench.assert_no_faults();
}

#[test]
fn a_roster_of_three_waits_for_its_second_player_before_the_countdown() {
    let mut bench = Bench::new(3, MAP_BATTERIES, Settings::default());
    bench.join(0);
    bench.run_long(5_000);
    // floor(0.8 × 3) = 2 must be in.
    assert_eq!(bench.game().rule_state(), state::WAITING_FOR_PLAYERS);
    bench.join(1);
    bench.run(500);
    assert_eq!(bench.game().rule_state(), state::COUNTDOWN);
    // Whoever leaves, the countdown goes on and play begins.
    bench.clients[1].leave();
    bench.run_long(21_000);
    assert_eq!(bench.game().rule_state(), state::IN_PROCESS);
    bench.assert_no_faults();
}

/// Worlds no game could load: the bytes given, and a real world but for
/// one thing (world_frame_check.md).
fn unloadable_worlds(world: &[u8]) -> Vec<Vec<u8>> {
    let real = Sections::walk(world).expect("a world that walks");
    let block = players::block_bytes(&[0xAB; 33], None);
    let big_state = players::block_bytes(&[0xAB; PLAYER_STATE_CAP + 1], None);
    let over = Section::ALL.map(|section| vec![0; section.cap() + 1]);
    let [bullets, statistics, objects, physics, rules, effects] = &over;
    let worlds = [
        // A place past the roster of two.
        Sections {
            active_clients_mask: 1 << 2,
            players: &block,
            ..real
        },
        // A place past the 20 slots of any game.
        Sections {
            active_clients_mask: 1 << 31,
            players: &block,
            ..real
        },
        Sections {
            active_clients_mask: 1,
            players: &big_state,
            ..real
        },
        Sections { bullets, ..real },
        Sections { statistics, ..real },
        Sections { objects, ..real },
        Sections { physics, ..real },
        Sections { rules, ..real },
        Sections { effects, ..real },
    ];
    let mut bodies = vec![vec![0xEE; 777]];
    bodies.extend(worlds.iter().map(|world| world.to_body().unwrap()));
    // A mask that names a player whose block is not there: the reader
    // would take a section's length for the block's.
    let mut torn = world.to_vec();
    torn[..4].copy_from_slice(&1u32.to_le_bytes());
    bodies.push(torn);
    bodies
}

#[test]
fn a_world_no_game_could_load_goes_to_nobody() {
    let mut bench = Bench::new(2, MAP_BATTERIES, Settings::default());
    bench.clients[0].connect();
    assert!(bench.until(1_000, |bench| bench.clients[0].model.options.is_some()));
    let world = bench.clients[0].sim.world(INITIAL_MS);
    // At its caps a world goes, as a real one does.
    let real = Sections::walk(&world).unwrap();
    let full = vec![0; Section::Bullets.cap()];
    let at_cap = Sections {
        bullets: &full,
        ..real
    }
    .to_body()
    .unwrap();
    assert!(Frame::parse(&at_cap, Places::of(2)).is_ok());
    let bodies = unloadable_worlds(&world);
    for (count, body) in bodies.iter().enumerate() {
        assert!(Frame::parse(body, Places::of(2)).is_err(), "world {count}");
        // One item time is one upload for the reassembler.
        bench.clients[0].upload(INITIAL_MS + 50 * count as u32, body);
        bench.run(100);
        assert_eq!(bench.game().uploads_refused(0), count as u32 + 1);
    }
    assert!(bench.clients[0].model.send_ready());
    bench.clients[0].send(id::READY, &[], true);
    bench.run(1_000);
    assert!(!bench.game().has_clock());
    assert_eq!(bench.stages().as_bytes()[0], b'w');
    // The next game's world starts the match, and the first game from it.
    bench.join(1);
    assert!(bench.until(5_000, |bench| bench.stages() == "LL"));
    assert_eq!(bench.game().uploads_refused(1), 0);
    bench.assert_no_faults();
}

#[test]
fn the_initial_world_is_not_checked_when_the_setting_says_so() {
    let settings = Settings {
        check_initial_world: false,
        ..Settings::default()
    };
    let mut bench = Bench::new(1, MAP_BATTERIES, settings);
    bench.clients[0].sim.kills_from = Some((0, 1));
    bench.join(0);
    assert!(bench.game().has_clock());
    bench.assert_no_faults();
}

#[test]
fn clients_on_opposite_hash_phases_are_noticed_and_one_report_is_never_trusted() {
    let mut bench = Bench::new(2, MAP_BATTERIES, Settings::default());
    bench.join_all();
    settle(&mut bench);
    // Both report every tick (the patched 50 ms interval): worlds are
    // trusted, nothing is starved.
    bench.run_long(12_000);
    assert!(!bench.game().hash_starved());
    assert_eq!(bench.game().hash_starved_warnings(), 0);
    let (trusted, _) = bench.game().observed().expect("a trusted snapshot");
    assert!(trusted + 8_000 > bench.clock(), "{trusted}");

    // Stock clients, 100 ms, on opposite phases: player 0 reports the
    // ticks at whole 100 ms, player 1 the ones between (the horizon is a
    // whole number of 100 ms behind), and no tick has two hash reports.
    assert_eq!(HORIZON_MS % 100, 0);
    let switched = bench.clock();
    let even = (switched / 100 + 3) * 100;
    for (place, client) in bench.clients.iter_mut().enumerate() {
        client.hash_interval_ms = 100;
        client.hash_last_sent = even + 50 * place as u32;
    }
    let uploads = [bench.clients[0].uploads, bench.clients[1].uploads];
    bench.run_long(4_000);
    assert!(!bench.game().hash_starved(), "not said before 5 s");
    bench.run_long(8_000);
    assert!(bench.game().hash_starved());
    assert_eq!(bench.game().hash_starved_warnings(), 1);
    // Said again every 30 s while it lasts; nobody is resynced over it.
    bench.run_long(30_000);
    assert!(bench.game().hash_starved());
    assert_eq!(bench.game().hash_starved_warnings(), 2);
    assert_eq!((bench.game().resyncs(0), bench.game().resyncs(1)), (0, 0));
    // The two-voucher rule stands: a world trusted in that time is one
    // whose tick the client that did not upload it sent a hash for. An
    // upload on its uploader's own ticks has one voucher and is not.
    let uploaded = [
        bench.clients[0].uploads > uploads[0],
        bench.clients[1].uploads > uploads[1],
    ];
    assert!(uploaded[0] || uploaded[1], "worlds were asked for");
    let (trusted, _) = bench.game().observed().expect("the last agreed world");
    if trusted > switched + 1_000 {
        let on_ticks_of = (trusted % 100 / 50) as usize;
        assert!(
            uploaded[1 - on_ticks_of],
            "the world at {trusted} ms is on player {on_ticks_of}'s ticks and only it uploaded"
        );
    }

    // The same clients reporting every tick: over at once, and worlds
    // are trusted as before.
    for client in &mut bench.clients {
        client.hash_interval_ms = 50;
    }
    bench.run_long(10_000);
    assert!(!bench.game().hash_starved());
    assert_eq!(bench.game().hash_starved_warnings(), 2);
    let (trusted, _) = bench.game().observed().unwrap();
    assert!(trusted + 8_000 > bench.clock(), "{trusted}");
    bench.assert_no_faults();
}

#[test]
fn on_opposite_hash_phases_an_upload_on_its_uploaders_own_tick_is_not_trusted() {
    // The unlucky half of a starved match, every time: each client hashes
    // every second tick, never the other's, and uploads a tick it hashed.
    let mut bench = Bench::new(2, MAP_BATTERIES, Settings::default());
    bench.join_all();
    settle(&mut bench);
    bench.run_long(6_000);
    let switched = bench.clock();
    let even = (switched / 100 + 3) * 100;
    for (place, client) in bench.clients.iter_mut().enumerate() {
        client.hash_interval_ms = 100;
        client.hash_last_sent = even + 50 * place as u32;
        client.uploads_hashed_tick = true;
    }
    let uploads = bench.clients[0].uploads + bench.clients[1].uploads;
    bench.run_long(20_000);
    assert!(bench.game().hash_starved());
    assert_eq!(bench.game().hash_starved_warnings(), 1);
    assert!(bench.clients[0].uploads + bench.clients[1].uploads >= uploads + 3);
    // No world from after the switch is trusted: each has one voucher.
    let (trusted, _) = bench.game().observed().expect("the last agreed world");
    assert!(trusted <= switched + 1_000, "{trusted} after {switched}");
    assert_eq!((bench.game().resyncs(0), bench.game().resyncs(1)), (0, 0));
    assert!(bench.results.try_recv().is_err());
    bench.assert_no_faults();
}

/// Stock clients (100 ms between hashes) on opposite phases, from the
/// match clock `switched` on.
fn opposite_phases(bench: &mut Bench) -> u32 {
    assert_eq!(HORIZON_MS % 100, 0);
    let switched = bench.clock();
    let even = (switched / 100 + 3) * 100;
    for (place, client) in bench.clients.iter_mut().enumerate() {
        client.hash_interval_ms = 100;
        client.hash_last_sent = even + 50 * place as u32;
    }
    switched
}

/// The times of the worlds trusted while `ms` pass, in 5 ms steps.
fn trusted_during(bench: &mut Bench, ms: u64) -> BTreeSet<u32> {
    let mut trusted = BTreeSet::new();
    for _ in 0..ms / 5 {
        bench.step(5);
        if let Some((time, _)) = bench.game().observed() {
            trusted.insert(time);
        }
    }
    trusted
}

#[test]
fn on_opposite_hash_phases_two_clients_are_asked_for_the_world_of_one_tick() {
    let mut bench = Bench::new(2, MAP_BATTERIES, Settings::default());
    bench.join_all();
    settle(&mut bench);
    // Player 1's clock is 30 ms ahead of player 0's and stays so: asked at
    // the same moment, the two are in different ticks more often than not.
    let match_id = bench.match_id;
    bench
        .server
        .games
        .get_mut(&match_id)
        .unwrap()
        .set_clock_correction(false);
    bench.clients[1].model.advance(30);
    let switched = opposite_phases(&mut bench);
    bench.run(9_000);
    assert!(bench.game().hash_starved());
    // The first rounds tell where each client's ticks lie; from then on
    // both are asked for one tick, each at its own moment, every 3 s.
    let before = trusted_during(&mut bench, 9_000);
    let uploads = [bench.clients[0].uploads, bench.clients[1].uploads];
    let trusted = trusted_during(&mut bench, 30_000);
    let new: Vec<u32> = trusted.difference(&before).copied().collect();
    assert!(new.len() >= 9, "{new:?}");
    assert!(new.iter().all(|time| *time > switched));
    assert_eq!(bench.game().trusted_by(), 2);
    for place in 0..2 {
        assert!(bench.clients[place].uploads >= uploads[place] + 9);
        // What the server knows of each client's ticks is true: asked at
        // match clock c, the client uploads the tick of c + offset.
        let (low, high) = bench.game().upload_offset(place).expect("bounds");
        let truth = i64::from(bench.clients[place].clock())
            - i64::from(bench.clock())
            - i64::from(HORIZON_MS);
        assert!(
            low <= truth && truth < high,
            "{place}: {truth} in {low}..{high}"
        );
    }
    // Still starved of hashes, and said so; nobody resynced over it.
    assert!(bench.game().hash_starved());
    assert_eq!((bench.game().resyncs(0), bench.game().resyncs(1)), (0, 0));

    // A client out of step is found by the paired worlds and resynced.
    let at = bench.clock() / 50 * 50 + 100;
    bench.clients[1].sim.salt_from = Some((at, 0x5A));
    assert!(bench.until(20_000, |bench| {
        bench.game().resyncs(0) + bench.game().resyncs(1) == 1
    }));
    bench.assert_no_faults();
}

#[test]
fn on_opposite_hash_phases_one_patched_client_is_asked_for_the_others_ticks() {
    let mut bench = Bench::new(2, MAP_BATTERIES, Settings::default());
    // Player 1's exe has no upload patch: only player 0 can give worlds.
    bench.clients[1] = Client::new(session_of(1), false, 2, MAP_BATTERIES);
    bench.join_all();
    settle(&mut bench);
    let match_id = bench.match_id;
    bench
        .server
        .games
        .get_mut(&match_id)
        .unwrap()
        .set_clock_correction(false);
    bench.clients[0].model.advance(20);
    let switched = opposite_phases(&mut bench);
    bench.run(9_000);
    assert!(bench.game().hash_starved());
    let before = trusted_during(&mut bench, 9_000);
    let trusted = trusted_during(&mut bench, 30_000);
    let new: Vec<u32> = trusted.difference(&before).copied().collect();
    // Every world asked for now falls on a tick player 1 sends a hash
    // for (the ones ending in 50), and is trusted by the two of them.
    assert!(new.len() >= 9, "{new:?}");
    assert!(
        new.iter().all(|time| *time > switched && time % 100 == 50),
        "{new:?}"
    );
    assert_eq!(bench.game().trusted_by(), 2);
    assert_eq!(bench.clients[1].uploads, 0);
    bench.assert_no_faults();
}

#[test]
fn what_each_player_has_left_comes_with_the_result() {
    let mut bench = Bench::new(2, MAP_BATTERIES, Settings::default());
    // Both simulations have both players: player 0 alive with its pistol
    // worn a little, player 1 dead.
    let state = |alive: bool, condition: u32| PlayerState {
        current_index: 0,
        target_index: 0,
        pickup: weapon_pickup::Record::EMPTY,
        items: vec![condition],
        alive,
        has_to_die: false,
        look_pitch: 0.0,
        rotation_y: 1.5,
        actions_mask: 0,
        effects: 0,
    };
    for client in &mut bench.clients {
        client.sim.player_states = vec![Some(state(true, 990_000)), Some(state(false, 1_000_000))];
    }
    bench.join_all();
    settle(&mut bench);
    bench.run_long(130_000);
    let at = bench.clock() / 50 * 50 + 100;
    for client in &mut bench.clients {
        client.sim.points_from = Some((at, [10, 3]));
    }
    assert!(bench.until(12_000, |bench| {
        bench.clients.iter().all(|client| client.closed)
    }));
    let result = bench.results.try_recv().expect("a result");
    assert_eq!(result.end, MatchEnd::ModeRule);
    assert_eq!(result.team_points, [10, 3]);
    let supplies = result.players[0]
        .supplies
        .as_ref()
        .expect("player 0's block");
    assert!(supplies.alive);
    assert_eq!(supplies.kit, 0);
    assert_eq!(supplies.items.len(), 1);
    assert_eq!(
        (supplies.items[0].stack, supplies.items[0].stored),
        (false, 990_000)
    );
    let supplies = result.players[1]
        .supplies
        .as_ref()
        .expect("player 1's block");
    assert!(!supplies.alive);
    assert_eq!(supplies.items[0].stored, 1_000_000);
    bench.assert_no_faults();
}

/// The index in what the client took of the first message `pick` names.
fn first(client: &Client, pick: impl Fn(&ServerMessage) -> bool) -> Option<usize> {
    client.taken.iter().position(pick)
}

#[test]
fn the_inputs_since_a_snapshot_go_between_the_time_and_the_world() {
    let mut bench = Bench::new(2, MAP_BATTERIES, Settings::default());
    bench.join(0);
    settle(&mut bench);
    bench.run(3_000);
    bench.clients[1].connect();
    assert!(bench.until(1_000, |bench| bench.clients[1].model.options.is_some()));
    bench.clients[1].ready();
    for _ in 0..40 {
        if bench.stages() == "LL" {
            break;
        }
        bench.clients[0].input(50, 0x21);
        bench.run(50);
    }
    assert_eq!(bench.stages(), "LL");
    let joiner = &bench.clients[1];
    let time = first(joiner, |message| {
        matches!(message, ServerMessage::SetTime { .. })
    })
    .unwrap();
    let relay = first(joiner, |message| {
        matches!(message, ServerMessage::InputRelay { .. })
    })
    .unwrap();
    let world = first(joiner, |message| {
        matches!(message, ServerMessage::Snapshot { .. })
    })
    .unwrap();
    assert!(time < relay && relay < world, "{time} {relay} {world}");
    // Its own entry is for a tick about 300 ms ahead of the clock.
    let entered = joiner
        .got(|message| match message {
            ServerMessage::PlayerEntered { tick, player: 1 } => Some(*tick),
            _ => None,
        })
        .pop()
        .unwrap();
    let ahead = entered.wrapping_sub((bench.clock() / 50) as u8) as i8;
    assert!((3..=7).contains(&ahead), "{ahead} ticks ahead");
    bench.assert_no_faults();
}

#[test]
fn a_resynced_client_gets_the_others_inputs_ahead_of_its_world_and_never_its_own() {
    for world_only in [false, true] {
        let settings = Settings {
            resync_world_only: world_only,
            ..Settings::default()
        };
        let mut bench = Bench::new(3, MAP_BATTERIES, settings);
        bench.join_all();
        settle(&mut bench);
        bench.run(3_000);
        let from = bench.clients[2].clock();
        bench.clients[2].sim.salt_from = Some((from, 7));
        // Everybody keeps sending inputs while the third is found out and
        // restored.
        let taken_before = bench.clients[2].taken.len();
        let mut resynced = false;
        for _ in 0..400 {
            for client in &mut bench.clients {
                client.input(20, 0x21);
            }
            bench.run(50);
            if bench.game().resyncs(2) == 1 && bench.stages() == "LLL" {
                resynced = true;
                break;
            }
        }
        assert!(resynced, "{}", bench.stages());
        let client = &bench.clients[2];
        // Never one of its own inputs back.
        assert!(client.relays_of(2).is_empty());
        let since = &client.taken[taken_before..];
        let world = since
            .iter()
            .position(|message| matches!(message, ServerMessage::Snapshot { generation: 1, .. }))
            .expect("the world of the next generation");
        let (world_time, set_times) = match &since[world] {
            ServerMessage::Snapshot { time, .. } => (
                *time,
                since
                    .iter()
                    .filter(|message| {
                        matches!(message, ServerMessage::SetTime { generation: 1, .. })
                    })
                    .count(),
            ),
            _ => unreachable!(),
        };
        // The time goes with it, unless the setting says the world alone.
        assert_eq!(set_times == 0, world_only, "{set_times}");
        // The others' inputs after the world's time were with the client
        // before the world was.
        let ahead = since[..world]
            .iter()
            .filter(|message| match message {
                ServerMessage::InputRelay { input, .. } => input
                    .time
                    .resolve(bench.clock())
                    .is_some_and(|at| at > world_time),
                _ => false,
            })
            .count();
        assert!(ahead > 0);
        assert_eq!(client.model.generation.0, 1);
        bench.assert_no_faults();
    }
}

#[test]
fn a_client_that_restores_do_not_help_is_put_out_when_the_setting_says_so() {
    let settings = Settings {
        restore_limit: 2,
        ..Settings::default()
    };
    let mut bench = Bench::new(3, MAP_BATTERIES, settings);
    bench.join_all();
    settle(&mut bench);
    bench.run(3_000);
    // The third client's world goes its own way again after every load.
    let gone = (0..3_000).any(|_| {
        if bench.clients[2].sim.salt_from.is_none() && bench.clients[2].model.is_synced() {
            let from = bench.clients[2].clock();
            bench.clients[2].sim.salt_from = Some((from, 9));
        }
        bench.step(25);
        bench.stages().ends_with('g') || bench.stages().ends_with('-')
    });
    assert!(gone, "{}", bench.stages());
    // Two restores, then out; the others saw it leave and play on.
    assert_eq!(bench.game().resyncs(0) + bench.game().resyncs(1), 0);
    assert!(bench.clients[2].disconnected);
    assert!(bench.until(1_000, |bench| bench.clients[0].left() == [2]));
    assert!(bench.stages().starts_with("LL"));

    // With no limit (the default) it is restored again and again.
    let mut bench = Bench::new(3, MAP_BATTERIES, Settings::default());
    bench.join_all();
    settle(&mut bench);
    for _ in 0..3_000 {
        if bench.clients[2].sim.salt_from.is_none() && bench.clients[2].model.is_synced() {
            let from = bench.clients[2].clock();
            bench.clients[2].sim.salt_from = Some((from, 9));
        }
        bench.step(25);
    }
    assert!(bench.game().resyncs(2) >= 3, "{}", bench.game().resyncs(2));
    assert_eq!(bench.stages().len(), 3);
    assert!(!bench.clients[2].disconnected);
}

/// Datagrams to the client at `place` with any of `ids` in them are lost
/// while the flag returned is set.
fn hold(bench: &mut Bench, place: usize, ids: &'static [u8]) -> Arc<AtomicBool> {
    let holding = Arc::new(AtomicBool::new(false));
    let held = holding.clone();
    bench.clients[place].lose = Some(Box::new(move |datagram| {
        held.load(Ordering::Relaxed)
            && wire::parse(Table::Match, Direction::ToClient, datagram, 1).is_ok_and(|parsed| {
                parsed
                    .messages
                    .iter()
                    .any(|message| ids.contains(&message.id))
            })
    }));
    holding
}

/// 5 ms pass; every 50 ms the first two clients make an input.
fn step_playing(bench: &mut Bench) {
    if bench.ms.is_multiple_of(50) {
        for client in &mut bench.clients[..2] {
            client.input(20, 0x21);
        }
    }
    bench.step(5);
}

/// Steps until `done`, at most `limit_ms`, the first two clients playing.
fn until_playing(bench: &mut Bench, limit_ms: u64, done: impl Fn(&Bench) -> bool) -> bool {
    let end = bench.ms + limit_ms;
    while bench.ms < end && !done(bench) {
        step_playing(bench);
    }
    done(bench)
}

/// Three clients in play; the third goes its own way and is resynced. The
/// first two make an input every 50 ms, the third too while it plays (if
/// `own_ahead` is given: that far ahead of its clock). Steps until the
/// server has sent the third its replay, which `holding` holds from its
/// first 0xE5 on. Returns the third's inputs and the match clock when its
/// first 0xE5 went.
fn resync_third(
    bench: &mut Bench,
    holding: &AtomicBool,
    own_ahead: Option<i32>,
) -> (Vec<PlayerInput>, u32) {
    bench.join_all();
    settle(bench);
    bench.run(3_000);
    let from = bench.clients[2].clock();
    bench.clients[2].sim.salt_from = Some((from, 7));
    let mut made = Vec::new();
    let mut resync_at = None;
    while bench.ms < 60_000 {
        let (resyncing, replayed) = bench.game().resync_state(2);
        if resyncing {
            resync_at.get_or_insert(bench.clock());
            holding.store(true, Ordering::Relaxed);
        }
        if replayed {
            return (made, resync_at.unwrap());
        }
        if let Some(ahead) = own_ahead
            && !resyncing
            && bench.ms.is_multiple_of(50)
        {
            made.push(bench.clients[2].input(ahead, 0x21));
        }
        step_playing(bench);
    }
    panic!("player 2 was not resynced: {}", bench.stages());
}

/// An input of the third client for `time` (precise, so that no other of
/// its inputs has that time).
fn own_input(bench: &mut Bench, time: u32) -> PlayerInput {
    let input = PlayerInput {
        time: WireTime::precise(time),
        rotation: [0x00, 0x3C, 0x00, 0xC0],
        actions: 0x23,
    };
    bench.clients[2].send_input(input);
    input
}

/// The answers to its own inputs a client took: dropped or moved.
fn answers(messages: &[ServerMessage]) -> Vec<&ServerMessage> {
    messages
        .iter()
        .filter(|message| {
            matches!(
                message,
                ServerMessage::DiscardInput { .. }
                    | ServerMessage::MoveInput { .. }
                    | ServerMessage::DiscardProfileInput { .. }
                    | ServerMessage::MoveProfileInput { .. }
            )
        })
        .collect()
}

/// Where in what the client took the first 0xE5 of `generation` and the
/// world of that generation are.
fn sync_span(client: &Client, generation: u8) -> (usize, usize) {
    let time = first(client, |message| {
        matches!(message, ServerMessage::SetTime { generation: g, .. } if *g == generation)
    })
    .expect("its 0xE5");
    let world = first(client, |message| {
        matches!(message, ServerMessage::Snapshot { generation: g, .. } if *g == generation)
    })
    .expect("its world");
    (time, world)
}

/// The replay of the third client's resync is held (`hold`), and the others
/// give no world for a moment: the newest snapshot there is, which is the
/// one its world will be.
fn hold_replay(bench: &mut Bench, holding: &AtomicBool) -> u32 {
    holding.store(true, Ordering::Relaxed);
    for client in &mut bench.clients[..2] {
        client.no_uploads = true;
    }
    // An upload asked for already may be on its way.
    until_playing(bench, 100, |_| false);
    assert_eq!(bench.stages(), "LLt");
    bench.game().newest_snapshot_time().expect("a snapshot")
}

/// The replay goes through, and the others give worlds again.
fn release(bench: &mut Bench, holding: &AtomicBool) {
    holding.store(false, Ordering::Relaxed);
    for client in &mut bench.clients[..2] {
        client.no_uploads = false;
    }
}

#[test]
fn a_resynced_clients_inputs_are_relayed_throughout_and_their_answers_wait_for_its_world() {
    // Late inputs dropped, as `discard` does: the answer is what is looked
    // at (relayed, a late one needs none).
    let settings = Settings {
        late_inputs: LateInputs::Discard,
        ..Settings::default()
    };
    let mut bench = Bench::new(3, MAP_BATTERIES, settings);
    let holding = hold(&mut bench, 2, &[id::INPUT_RELAY]);
    // It plays on while it waits for a world: its inputs go to the others.
    let (made, _) = resync_third(&mut bench, &holding, Some(20));
    assert!(!made.is_empty());
    let newest = hold_replay(&mut bench, &holding);
    // Inputs it made before its 0xE5 come now: one in time, one too late,
    // for a tick past every world there is but still in its history once
    // it has loaded one (500 ms).
    let clock = bench.clock();
    let in_time = own_input(&mut bench, clock - 100 + 7);
    let late_time = bench.clock() - 255 + 3;
    assert!(late_time > newest, "{late_time} {newest}");
    let late = own_input(&mut bench, late_time);
    bench.step(5);
    assert_eq!(bench.stages(), "LLt");
    release(&mut bench, &holding);
    assert!(until_playing(&mut bench, 2_000, |bench| bench.stages() == "LLL"));
    bench.run(500);

    let client = &bench.clients[2];
    let (time, world) = sync_span(client, 1);
    // Its clock was put past the newest of its own inputs.
    let ServerMessage::SetTime { match_time, .. } = client.taken[time] else {
        unreachable!()
    };
    let newest_own = made
        .iter()
        .map(|input| input.time.resolve(match_time).unwrap())
        .max()
        .unwrap();
    assert!(match_time >= newest_own + 50, "{match_time} {newest_own}");
    // Nothing that answers an input of its own while it stood still ...
    assert!(answers(&client.taken[time..world]).is_empty());
    // ... and right after its world, the drop of the late one, past the
    // world's time.
    let ServerMessage::Snapshot {
        time: world_time, ..
    } = client.taken[world]
    else {
        unreachable!()
    };
    assert_eq!(world_time, newest);
    assert_eq!(
        answers(&client.taken[world..]),
        [&ServerMessage::DiscardInput { time: late.time }]
    );
    // Every other input of its own reached the others, and none came back.
    let relayed = bench.clients[0].relays_of(2);
    for input in made.iter().chain([&in_time]) {
        assert!(relayed.contains(input), "{input:?}");
    }
    assert!(!relayed.contains(&late));
    assert!(client.relays_of(2).is_empty());
    assert_eq!(bench.game().generation(2), Some(1));
    assert_eq!(bench.game().resyncs(2), 1);
    bench.assert_no_faults();
}

/// Live, 2026-10-15: only agreed worlds were kept, so an out-of-step game's
/// world could never be compared with the others'. A player found out of
/// step is asked for its world ahead of its resync's 0xE5; it is kept to
/// look at, and nobody starts from it. Twice a player at most.
#[test]
fn a_player_out_of_step_is_asked_for_its_world_ahead_of_its_resync() {
    let mut bench = Bench::new(3, MAP_BATTERIES, Settings::default());
    bench.join_all();
    settle(&mut bench);
    bench.run(3_000);
    let asked_before = bench.clients[2].upload_requests;
    let from = bench.clients[2].clock();
    bench.clients[2].sim.salt_from = Some((from, 7));
    assert!(until_playing(&mut bench, 20_000, |bench| {
        bench.game().resyncs(2) == 1
    }));
    let is_set_time =
        |message: &ServerMessage| matches!(message, ServerMessage::SetTime { generation: 1, .. });
    assert!(until_playing(&mut bench, 2_000, |bench| {
        first(&bench.clients[2], is_set_time).is_some()
    }));
    // The request went ahead of the 0xE5 of its resync.
    let client = &bench.clients[2];
    let set_time = first(client, is_set_time).unwrap();
    assert!(
        client.taken[..set_time]
            .iter()
            .rev()
            .take(20)
            .any(|message| matches!(message, ServerMessage::UploadRequest)),
        "{:?}",
        &client.taken[set_time.saturating_sub(5)..=set_time]
    );
    assert!(client.upload_requests > asked_before);
    assert!(until_playing(&mut bench, 5_000, |bench| bench.stages() == "LLL"));
    assert_eq!(bench.game().wrong_worlds(2), (1, false));
    // Its world was nobody's start: the others never went again, and it
    // is right from its resync on.
    bench.clients[2].sim.salt_from = None;
    bench.run_long(15_000);
    assert_eq!(bench.game().resyncs(2), 1);
    assert_eq!(bench.game().resyncs(0) + bench.game().resyncs(1), 0);
    bench.assert_no_faults();
}

#[test]
fn an_answer_a_resynced_game_could_no_longer_take_has_it_started_from_a_newer_world() {
    let mut bench = Bench::new(3, MAP_BATTERIES, Settings::default());
    let holding = hold(&mut bench, 2, &[id::INPUT_RELAY]);
    resync_third(&mut bench, &holding, None);
    let newest = hold_replay(&mut bench, &holding);
    // A late input past the world, which comes when the game's history
    // will not reach back to it once it has its world (500 ms).
    let late_time = newest + 3;
    assert!(until_playing(&mut bench, 2_000, |bench| bench.clock() >= late_time + 480));
    let late = own_input(&mut bench, late_time);
    bench.step(5);
    release(&mut bench, &holding);
    // Its world comes, then a second one, from past the late input.
    assert!(until_playing(&mut bench, 5_000, |bench| {
        bench.stages() == "LLL" && bench.game().generation(2) == Some(2)
    }));
    let client = &bench.clients[2];
    let snapshots = client.snapshots();
    assert_eq!(snapshots.len(), 3, "{snapshots:?}");
    assert_eq!(snapshots[1], (1, newest));
    assert!(snapshots[2].1 >= late_time, "{snapshots:?}");
    // It never had to be told to drop it: the next world is past it.
    assert!(answers(&client.taken).is_empty());
    assert!(!bench.clients[0].relays_of(2).contains(&late));
    bench.assert_no_faults();
}

#[test]
fn a_resynced_clients_world_waits_for_its_replay_and_is_never_old() {
    let mut bench = Bench::new(3, MAP_BATTERIES, Settings::default());
    let holding = hold(&mut bench, 2, &[id::INPUT_RELAY]);
    let (_, resync_at) = resync_third(&mut bench, &holding, None);
    // It made no input: its clock is put at its own as the 0xE5 arrives
    // (known from the 0xE5 it last took; the match clock here, and half a
    // round trip of a few ms), never behind it.
    let time = first(&bench.clients[2], |message| {
        matches!(message, ServerMessage::SetTime { generation: 1, .. })
    })
    .unwrap();
    let ServerMessage::SetTime { match_time, .. } = bench.clients[2].taken[time] else {
        unreachable!()
    };
    assert!(
        (resync_at..=resync_at + 15).contains(&match_time),
        "{match_time} {resync_at}"
    );
    let newest = hold_replay(&mut bench, &holding);
    // While its replay is not acked, the world waits, as long as its
    // snapshot stays young enough to be loaded in time (and a second at
    // most): then it goes without it, never older.
    assert!(until_playing(&mut bench, 2_000, |bench| bench.stages() == "LLs"));
    let waited_to = bench.clock();
    assert!(
        (newest + 800..=newest + 1_000).contains(&waited_to),
        "{newest} {waited_to}"
    );
    assert_eq!(bench.game().sync_time(2), Some(newest));
    assert_eq!(bench.game().generation(2), Some(1));
    // The replay up to the world's time is in the world: no more of it
    // goes. The rest arrives once the line lets it, and the game is in.
    release(&mut bench, &holding);
    assert!(until_playing(&mut bench, 3_000, |bench| bench.stages() == "LLL"));
    let snapshots = bench.clients[2].snapshots();
    assert_eq!(snapshots.len(), 2, "{snapshots:?}");
    assert_eq!(snapshots[1], (1, newest));
    assert_eq!(generations(&bench.clients[2]), [0, 1]);
    bench.assert_no_faults();
}

#[test]
fn a_roster_change_to_a_resynced_client_goes_again_until_acked() {
    let mut bench = Bench::new(3, MAP_BATTERIES, Settings::default());
    let holding = hold(&mut bench, 2, &[id::INPUT_RELAY, id::PLAYER_LEFT]);
    resync_third(&mut bench, &holding, None);
    // The second player leaves while the third stands still: that roster
    // change is sent to it at once, and its copies are lost for a while.
    let left_at = bench.clock();
    bench.clients[1].leave();
    bench.run(600);
    let left = |bench: &Bench| {
        bench.clients[2]
            .got(|message| match message {
                ServerMessage::PlayerLeft { player: 1, .. } => Some(()),
                _ => None,
            })
            .len()
    };
    assert_eq!(left(&bench), 0);
    holding.store(false, Ordering::Relaxed);
    // Kept until acked: the world (older than the leave) went without
    // it, in the same generation, and it arrives all the same.
    assert!(bench.until(5_000, |bench| {
        bench.stages().ends_with('L') && left(bench) >= 1
    }));
    let snapshots = bench.clients[2].snapshots();
    assert_eq!(snapshots.len(), 2, "{snapshots:?}");
    assert_eq!(snapshots[1].0, 1);
    assert!(snapshots[1].1 < left_at, "{snapshots:?} {left_at}");
    assert!(bench.clients[2].skipped.is_empty());
    bench.assert_no_faults();
}

#[test]
fn a_resync_world_not_loaded_in_time_is_sent_again_from_a_fresh_snapshot_three_times_at_most() {
    let mut bench = Bench::new(3, MAP_BATTERIES, Settings::default());
    let holding = hold(&mut bench, 2, &[id::SNAPSHOT]);
    resync_third(&mut bench, &holding, None);
    // Its world does not arrive: 1.2 s after the snapshot's time (and the
    // way back of an ack) it is done again from a fresh one, as the next
    // generation, three times.
    for redone in 1..=3u32 {
        assert!(until_playing(&mut bench, 3_000, |bench| bench.stages() == "LLs"));
        let time = bench.game().sync_time(2).unwrap();
        let generation = bench.game().generation(2).unwrap();
        assert!(until_playing(&mut bench, 3_000, |bench| {
            bench.game().slow_loads(2) == redone
        }));
        assert!(bench.clock() > time + 1_200, "{} {time}", bench.clock());
        assert!(until_playing(&mut bench, 3_000, |bench| {
            bench.game().generation(2) == Some(generation + 1)
        }));
    }
    // Then a slower load is taken: the fourth world is not given up at
    // 1.2 s.
    assert!(until_playing(&mut bench, 3_000, |bench| bench.stages() == "LLs"));
    let time = bench.game().sync_time(2).unwrap();
    assert!(until_playing(&mut bench, 3_000, |bench| bench.clock() > time + 1_400));
    assert_eq!(bench.stages(), "LLs");
    // (Past the three: the slow load is taken.)
    assert_eq!(bench.game().slow_loads(2), 4);
    holding.store(false, Ordering::Relaxed);
    assert!(until_playing(&mut bench, 3_000, |bench| bench.stages() == "LLL"));
    assert_eq!(bench.game().slow_loads(2), 0);
    let snapshots = bench.clients[2].snapshots();
    assert_eq!(snapshots.len(), 2, "{snapshots:?}");
    assert_eq!(snapshots[1].0, 4);
    bench.assert_no_faults();
}

/// A player making two inputs a tick (precise inputs) fills a game's ring
/// of 27 in 0.7 s: a world not loaded by then is done again, before the
/// time rule's 1.2 s (live, 2026-10-15: 32 inputs in 1.1 s, the load wrong
/// from its first tick).
#[test]
fn a_resync_world_is_done_again_once_a_player_has_more_inputs_after_it_than_a_game_keeps() {
    let mut bench = Bench::new(3, MAP_BATTERIES, Settings::default());
    let holding = hold(&mut bench, 2, &[id::SNAPSHOT]);
    resync_third(&mut bench, &holding, None);
    assert!(until_playing(&mut bench, 3_000, |bench| bench.stages() == "LLs"));
    let time = bench.game().sync_time(2).unwrap();
    let generation = bench.game().generation(2).unwrap();
    let busy = |bench: &mut Bench| {
        if bench.ms.is_multiple_of(50) {
            // Two precise inputs in one tick, as a firing player's.
            let tick = bench.clients[0].clock() / 50 * 50 + 50;
            for at in [tick + 10, tick + 35] {
                bench.clients[0].send_input(PlayerInput {
                    time: WireTime::precise(at),
                    rotation: [0x00, 0x3C, 0x00, 0xC0],
                    actions: 0x21,
                });
            }
            bench.clients[1].input(20, 0x21);
        }
        bench.step(5);
    };
    let end = bench.ms + 3_000;
    while bench.ms < end && bench.game().slow_loads(2) == 0 {
        busy(&mut bench);
    }
    assert_eq!(bench.game().slow_loads(2), 1);
    assert!(bench.clock() < time + 1_200, "{} {time}", bench.clock());
    let end = bench.ms + 3_000;
    while bench.ms < end && bench.game().generation(2) != Some(generation + 1) {
        busy(&mut bench);
    }
    assert_eq!(bench.game().generation(2), Some(generation + 1));
    holding.store(false, Ordering::Relaxed);
    assert!(until_playing(&mut bench, 3_000, |bench| bench.stages() == "LLL"));
    bench.assert_no_faults();
}

#[test]
fn the_lost_parts_of_a_resync_world_go_again_soon() {
    // Uncompressed and big: a world of many parts.
    let settings = Settings {
        compress_state: false,
        ..Settings::default()
    };
    let mut bench = Bench::new(3, MAP_BATTERIES, settings);
    for client in &mut bench.clients {
        client.sim.ballast = 13_000;
    }
    // It acks as a game was seen to: 13 seqs deep, so that a part lost
    // early in a burst is not seen skipped by the acks of later ones.
    bench.clients[2].slow_acks = true;
    let holding = hold(&mut bench, 2, &[id::SNAPSHOT]);
    resync_third(&mut bench, &holding, None);
    // The first 60 ms of parts of its world are lost on their way.
    assert!(until_playing(&mut bench, 3_000, |bench| bench.stages() == "LLs"));
    let sent = bench.ms;
    until_playing(&mut bench, 60, |_| false);
    holding.store(false, Ordering::Relaxed);
    // They come again 120 ms on (the table's own pace is 500 ms).
    assert!(until_playing(&mut bench, 1_000, |bench| bench.stages() == "LLL"));
    let took = bench.ms - sent;
    assert!(took < 400, "loaded {took} ms after it went");
    assert_eq!(bench.clients[2].snapshots().len(), 2);
    bench.assert_no_faults();
}

#[test]
fn a_world_whose_parts_go_unacked_is_taken_as_loaded_on_the_games_right_hash() {
    let mut bench = Bench::new(3, MAP_BATTERIES, Settings::default());
    let holding = hold(&mut bench, 2, &[]);
    resync_third(&mut bench, &holding, None);
    assert!(until_playing(&mut bench, 3_000, |bench| bench.stages() == "LLs"));
    let generation = bench.game().generation(2);
    // From its world's first parts on, its acks name nothing (a game's
    // acks name its newest few seqs only, live, 2026-10-14) ...
    bench.clients[2].mute_acks = true;
    // ... but its game loads the world and reports the world's hashes:
    // that is the proof, before the world's 1.75 s and before 1.2 s past
    // its snapshot.
    assert!(until_playing(&mut bench, 1_000, |bench| bench.stages() == "LLL"));
    assert_eq!(bench.game().generation(2), generation);
    bench.clients[2].mute_acks = false;
    bench.run(3_000);
    assert_eq!(bench.clients[2].snapshots().len(), 2);
    assert_eq!(bench.game().resyncs(2), 1);
    assert_eq!(bench.game().generation(2), generation);
    bench.assert_no_faults();
}

#[test]
fn a_tie_does_not_clear_a_disagreement_that_stands() {
    let mut bench = Bench::new(3, MAP_BATTERIES, Settings::default());
    bench.join_all();
    settle(&mut bench);
    // Player 1's inputs come late: in a tie its side counts for less.
    for _ in 0..60 {
        bench.clients[1].input(-300, 0x21);
        bench.run(20);
    }
    bench.run(3_000);
    // Player 2's world goes its own way; the three report it for a second.
    let from = bench.clients[2].clock();
    bench.clients[2].sim.salt_from = Some((from, 7));
    bench.run(1_000);
    // Then player 0 reports no more: every tick has two reports, 1 against
    // 2, which the links decide for 2. Taken as "agrees again", that wiped
    // a standing disagreement (live, 2026-10-14); it stands, and player 2
    // is resynced.
    bench.clients[0].no_hashes = true;
    assert!(bench.until(10_000, |bench| bench.game().resyncs(2) == 1));
    bench.assert_no_faults();
}

#[test]
fn a_relay_that_reached_a_game_past_its_history_keeps_its_resync_past_it() {
    let mut bench = Bench::new(3, MAP_BATTERIES, Settings::default());
    bench.join_all();
    settle(&mut bench);
    // Player 2's clock runs 600 ms ahead: the others' inputs reach its
    // game more than 500 ms past their ticks, and the game moves them.
    let match_id = bench.match_id;
    bench
        .server
        .games
        .get_mut(&match_id)
        .unwrap()
        .set_clock_correction(false);
    bench.clients[2].model.advance(600);
    bench.run(2_000);
    until_playing(&mut bench, 1_000, |_| false);
    let floor = bench.game().resync_floor(2);
    assert!(floor > bench.clock() + 300, "{floor} {}", bench.clock());
    // Its resync starts from no snapshot before the last moved tick.
    let from = bench.clients[2].clock();
    bench.clients[2].sim.salt_from = Some((from, 7));
    assert!(bench.until(20_000, |bench| {
        bench.game().resyncs(2) == 1 && bench.stages() == "LLL"
    }));
    let snapshots = bench.clients[2].snapshots();
    assert_eq!(snapshots.len(), 2, "{snapshots:?}");
    assert!(snapshots[1].1 >= floor, "{snapshots:?} {floor}");
    bench.assert_no_faults();
}

/// The third client, on a line of `round_trip` ms both ways, goes its own
/// way and is resynced while the first two play and give worlds as they
/// are asked. Returns the generations of the 0xE5s of its resync.
fn far_resync(round_trip: u64) -> BTreeSet<u8> {
    let mut bench = Bench::new(3, MAP_BATTERIES, Settings::default());
    bench.join_all();
    settle(&mut bench);
    bench.one_way = vec![0, 0, round_trip / 2];
    // Its round trip is measured, and its clock put right.
    until_playing(&mut bench, 12_000, |_| false);
    let before = set_times(&bench.clients[2]).len();
    let from = bench.clients[2].clock();
    bench.clients[2].sim.salt_from = Some((from, 7));
    assert!(until_playing(&mut bench, 20_000, |bench| {
        bench.game().resyncs(2) == 1
    }));
    let begun = bench.ms;
    assert!(
        until_playing(&mut bench, 10_000, |bench| {
            bench.stages() == "LLL" && bench.clients[2].model.syncs == 2
        }),
        "{}",
        bench.stages()
    );
    // In some round trips and a wait for a fresh world, never 20 s.
    assert!(bench.ms - begun < 5_000, "{} ms", bench.ms - begun);
    let generations = set_times(&bench.clients[2])[before..]
        .iter()
        .copied()
        .collect();
    // And it stays in step.
    until_playing(&mut bench, 5_000, |_| false);
    assert_eq!(bench.game().resyncs(2), 1);
    assert_eq!(bench.game().slow_loads(2), 0);
    assert_eq!(bench.stages(), "LLL");
    bench.assert_no_faults();
    generations
}

#[test]
fn a_resync_on_a_line_of_300_ms_takes_one_generation() {
    let generations = far_resync(300);
    assert_eq!(generations.len(), 1, "{generations:?}");
}

#[test]
fn a_resync_on_a_line_of_500_ms_takes_one_generation() {
    let generations = far_resync(500);
    assert_eq!(generations.len(), 1, "{generations:?}");
}

#[test]
fn a_clock_correction_still_running_is_ended_ahead_of_every_0xe5() {
    use std::sync::Mutex;
    use survarium::v026g::match_msgs::clock::{FAST_FROM_REVISION, STOCK_FACTOR};
    let mut bench = Bench::new(3, MAP_BATTERIES, Settings::default());
    bench
        .server
        .revisions
        .insert(session_of(2), FAST_FROM_REVISION - 1);
    bench.clients[2].model.clock_factor = Some(STOCK_FACTOR);
    // The messages (id, payload) of each datagram to the third client
    // with a 0xE5.
    type Datagram = Vec<(u8, Vec<u8>)>;
    let seen: Arc<Mutex<Vec<Datagram>>> = Arc::default();
    let record = seen.clone();
    bench.clients[2].lose = Some(Box::new(move |datagram| {
        if let Ok(parsed) = wire::parse(Table::Match, Direction::ToClient, datagram, 1)
            && parsed
                .messages
                .iter()
                .any(|message| message.id == id::SET_TIME)
        {
            record.lock().unwrap().push(
                parsed
                    .messages
                    .iter()
                    .map(|message| (message.id, message.payload.to_vec()))
                    .collect(),
            );
        }
        false
    }));
    bench.join_all();
    let off = |bench: &Bench| i64::from(bench.clock()) - i64::from(bench.clients[2].clock());
    assert!(bench.until(60_000, |bench| {
        off(bench).abs() <= 10 && bench.clients[2].model.correction_left() == 0.0
    }));
    bench.run(3_000);
    seen.lock().unwrap().clear();
    let offsets = |bench: &Bench| {
        bench.clients[2].got(|message| match message {
            ServerMessage::ClockOffset(offset) => Some(*offset),
            _ => None,
        })
    };
    let before = offsets(&bench).len();
    // Its clock falls 1.2 s behind: a correction of 24 s at 5 % ...
    let clock = bench.clients[2].model.clock.unwrap();
    bench.clients[2].model.clock = Some(clock - 1_200);
    assert!(until_playing(&mut bench, 5_000, |bench| {
        offsets(bench).len() > before
    }));
    assert_ne!(offsets(&bench)[before], 0);
    // ... during which its world goes its own way: it is resynced once the
    // resync was put off for as long as it may be (20 s).
    let from = bench.clients[2].clock();
    bench.clients[2].sim.salt_from = Some((from, 7));
    assert!(until_playing(&mut bench, 30_000, |bench| bench
        .game()
        .resyncs(2)
        == 1));
    assert!(bench.clients[2].model.correction_left() != 0.0);
    assert!(until_playing(&mut bench, 10_000, |bench| {
        bench.stages() == "LLL" && bench.clients[2].model.syncs == 2
    }));
    // Every 0xE5 until the game answered one went behind a 0xE7 of offset
    // 0, in its datagram (the last, with the world, needs none: the game
    // took one) ...
    let seen = seen.lock().unwrap().clone();
    assert!(seen.len() >= 2, "{seen:?}");
    let stop = (id::CLOCK_OFFSET, vec![0, 0]);
    let mut stops = 0;
    for (n, messages) in seen.iter().enumerate() {
        let at = messages
            .iter()
            .position(|(id, _)| *id == id::SET_TIME)
            .unwrap();
        if at > 0 && messages[at - 1] == stop {
            stops += 1;
        } else {
            assert!(n > 0 && n + 1 == seen.len(), "{n}: {messages:?}");
        }
    }
    assert!(stops >= 1);
    // ... and no 0xE7 of offset 0 went by itself.
    let sent = offsets(&bench)
        .into_iter()
        .filter(|offset| *offset == 0)
        .count();
    assert_eq!(sent, stops);
    bench.assert_no_faults();
}

/// What the registry says of the bench's one match now.
fn live(bench: &Bench) -> crate::v026g::LiveMatch {
    let mut all = bench.registry.live_matches(bench.now());
    assert_eq!(all.len(), 1);
    all.remove(0)
}

/// The registry is what a page outside the match server reads: it follows a
/// match from the moment it is made to its end, a second behind at most.
#[test]
fn the_registry_says_how_a_match_stands_while_it_runs() {
    use crate::v026g::{LiveMatch, LivePhase, LiveStanding};
    use survarium::v026g::match_msgs::snapshot::Tdm;
    let mut bench = Bench::new(3, MAP_TDM, Settings::default());
    // Made, and no client has come: the whole roster is expected, by team
    // (the places are of teams 0, 1, 0).
    assert_eq!(
        live(&bench),
        LiveMatch {
            match_id: bench.match_id,
            map_id: MAP_TDM,
            phase: LivePhase::Gathering,
            players: [2, 1],
            present: 0b111,
            kills: [0, 0],
            played: None,
            time_left: None,
            standing: None,
        }
    );
    // One of three is in: the countdown waits for the second.
    bench.join(0);
    bench.run(1_100);
    let waiting = live(&bench);
    assert_eq!(waiting.phase, LivePhase::Gathering);
    assert_eq!(waiting.players, [2, 1]);
    assert_eq!((waiting.played, waiting.time_left), (None, None));

    // The countdown: the time limit is known from here, 20 s and 15 min on.
    bench.join(1);
    bench.join(2);
    assert_eq!(bench.game().rule_state(), state::COUNTDOWN);
    bench.run(1_100);
    let countdown = live(&bench);
    assert_eq!(countdown.phase, LivePhase::Countdown);
    assert_eq!(countdown.played, None);
    let left = countdown.time_left.expect("the time limit");
    assert!(
        left > Duration::from_secs(900) && left <= Duration::from_secs(920),
        "{left:?}"
    );

    // In play: the two times add up to the match's 15 minutes, and the
    // standing is the one the clients agree on.
    settle(&mut bench);
    bench.run_long(25_000);
    assert_eq!(bench.game().rule_state(), state::IN_PROCESS);
    let playing = live(&bench);
    assert_eq!(playing.phase, LivePhase::Playing);
    assert_eq!(playing.players, [2, 1]);
    let played = playing.played.expect("play began");
    assert!(played < Duration::from_secs(15), "{played:?}");
    assert_eq!(
        played + playing.time_left.unwrap(),
        Duration::from_secs(900)
    );
    let standing = playing.standing.expect("a trusted world");
    assert_eq!((standing.points, standing.winner), ([100, 100], DRAW));
    assert!(standing.age < Duration::from_secs(10), "{:?}", standing.age);

    // Tickets go: team 0 leads.
    let at = bench.clock() / 50 * 50 + 100;
    for client in &mut bench.clients {
        client.sim.tdm_from = Some((
            at,
            Tdm {
                tickets: [91, 64],
                alive_players_mask: 0b111,
                active_players_mask: 0b111,
            },
        ));
    }
    bench.run_long(8_000);
    let standing = live(&bench).standing.unwrap();
    assert_eq!((standing.points, standing.winner), ([91, 64], 0));
    // Between two words of the match loop the times go on by themselves.
    let before = live(&bench);
    bench.ms += 400;
    let after = live(&bench);
    bench.ms -= 400;
    let step = Duration::from_millis(400);
    assert_eq!(after.played, before.played.map(|played| played + step));
    assert_eq!(after.time_left, before.time_left.map(|left| left - step));
    assert_eq!(
        after.standing,
        before.standing.map(|standing| LiveStanding {
            age: standing.age + step,
            ..standing
        })
    );

    // A player who leaves is no longer counted in its team.
    bench.clients[1].leave();
    bench.run(1_500);
    assert_eq!(live(&bench).players, [2, 0]);
    assert_eq!(live(&bench).phase, LivePhase::Playing);

    // The end: the match has its result and lingers, then it is gone.
    bench.clients[0].leave();
    bench.clients[2].leave();
    bench.run(1_500);
    assert!(bench.results.try_recv().is_ok());
    assert!(bench.registry.live_matches(bench.now()).is_empty());
    bench.assert_no_faults();
}

/// A match that ends by its own rule is said to be over for the seconds it
/// lingers, so that nobody shows it as still played.
#[test]
fn the_registry_says_a_match_with_its_result_is_over() {
    use crate::v026g::LivePhase;
    use survarium::v026g::match_msgs::snapshot::Tdm;
    let mut bench = Bench::new(2, MAP_TDM, Settings::default());
    bench.join_all();
    settle(&mut bench);
    bench.run_long(25_000);
    assert_eq!(live(&bench).phase, LivePhase::Playing);
    let at = bench.clock() / 50 * 50 + 100;
    for client in &mut bench.clients {
        client.sim.tdm_from = Some((
            at,
            Tdm {
                tickets: [37, 0],
                alive_players_mask: 0b01,
                active_players_mask: 0b11,
            },
        ));
    }
    assert!(bench.until(12_000, |bench| bench.results.try_recv().is_ok()));
    bench.run(1_100);
    let over = live(&bench);
    assert_eq!(over.phase, LivePhase::Over);
    let standing = over.standing.unwrap();
    assert_eq!((standing.points, standing.winner), ([37, 0], 0));
    bench.run_long(8_000);
    assert!(bench.registry.live_matches(bench.now()).is_empty());
}

#[test]
fn repeated_kill_payment_limit_preserves_the_match_result_statistics() {
    let mut bench = Bench::new(2, MAP_BATTERIES, Settings::default());
    bench.join_all();
    settle(&mut bench);
    bench.run_long(130_000);
    let at = bench.clock() / 50 * 50 + 100;
    for client in &mut bench.clients {
        client.sim.points_from = Some((at, [10, 3]));
        client.sim.kills_from = Some((at, 23));
    }
    assert!(bench.until(12_000, |bench| {
        bench.clients.iter().all(|client| client.closed)
    }));
    let result = bench.results.try_recv().expect("a result");
    let player = &result.players[0];
    assert_eq!(player.kills, 23);
    assert_eq!(player.statistics.as_ref().unwrap().total(event::KILL), 23);
    assert_eq!(
        player.pay_statistics.as_ref().unwrap().total(event::KILL),
        10
    );
    assert_eq!(player.score, 230);
    bench.assert_no_faults();
}
