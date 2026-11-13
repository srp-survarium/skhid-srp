//! Spectators (Queue and spectate, `game/spectate.rs`): a game watching the
//! match from a room's queue, with no place.

use super::*;
use session::{LiveDelay, Player, WatchKind, Watcher};
use survarium::v026g::match_msgs::ServerMessage;
use survarium::v026g::match_msgs::rules::state;
use survarium::v026g::match_msgs::static_info::NO_LOCAL_PLACE;

pub(super) const SPECTATOR: u32 = 0x5_0001;
pub(super) const SPECTATOR_ACCOUNT: u32 = 900;

fn running() -> Bench {
    let mut bench = Bench::growing(2, MAP_BATTERIES, Settings::default());
    bench.join_all();
    bench.run_long(25_000);
    assert_eq!(bench.game().rule_state(), state::IN_PROCESS);
    bench
}

/// The lobby's side of a watch: a match of its own seats (players not of
/// the bench) and the session watching the bench's match as `kind` does.
fn watch_as(bench: &Bench, session: u32, kind: WatchKind) {
    let seated = |id: u32| Player {
        session_id: 0x7_0000 + 2 * (session & 0xff) + id,
        account_id: 700 + 2 * (session & 0xff) + id,
        profile_id: 1,
    };
    let matchmaking = bench.server.store().matchmaking();
    let lobby_match = matchmaking
        .seat_teams([&[seated(0)], &[seated(1)]], bench.now())
        .expect("a lobby match");
    assert!(matchmaking.watch(
        session,
        Watcher {
            match_id: lobby_match,
            ticket: bench.match_id,
            account_id: SPECTATOR_ACCOUNT,
            kind,
        },
    ));
}

/// A spectator's game connected and loaded: its client's index.
fn spectate(bench: &mut Bench, session: u32) -> usize {
    spectate_as(bench, session, WatchKind::Queued { delay: no_delay() })
}

/// Live watching with its delay configured away: the worlds and relays as
/// soon as the path takes them, so a test need not run through the delay.
fn no_delay() -> LiveDelay {
    LiveDelay::from_secs(0).unwrap()
}

/// [`spectate`], as `kind` watches (`watch_as`).
fn spectate_as(bench: &mut Bench, session: u32, kind: WatchKind) -> usize {
    watch_as(bench, session, kind);
    let at = bench.clients.len();
    let players = bench.game().roster_size();
    bench
        .clients
        .push(Client::growing(session, players, MAP_BATTERIES));
    bench
        .addrs
        .push(SocketAddr::from(([127, 0, 0, 1], 45_000 + at as u16)));
    bench.clients[at].connect();
    assert!(bench.until(1_000, |b| b.clients[at].model.options.is_some()));
    bench.clients[at].ready();
    at
}

#[test]
fn a_spectator_gets_the_roster_with_no_place_its_world_and_every_relay() {
    let mut bench = running();
    let syncs = [bench.clients[0].model.syncs, bench.clients[1].model.syncs];
    let at = spectate(&mut bench, SPECTATOR);
    assert_eq!(bench.clients[at].model.local_index, None);
    assert_eq!(
        bench.clients[at]
            .model
            .options
            .as_ref()
            .unwrap()
            .players
            .len(),
        2
    );
    assert!(
        bench.until(10_000, |b| b.game().spectator_watches(SPECTATOR)),
        "{:?}",
        bench.clients[at].faults
    );
    assert!(bench.clients[at].model.is_synced());
    assert_eq!(bench.clients[at].snapshots().len(), 1);
    // The players' inputs reach it as they reach the other player.
    for _ in 0..20 {
        bench.clients[0].input(50, 0x21);
        bench.run(50);
    }
    // It is as far behind as the match's own relaying lets it be.
    bench.run(1_500);
    let to_player = bench.clients[1].relays_of(0);
    let to_spectator = bench.clients[at].relays_of(0);
    assert!(to_player.len() >= 20);
    assert!(
        to_player
            .iter()
            .rev()
            .take(15)
            .all(|input| to_spectator.contains(input)),
        "{} of {} relays",
        to_spectator.len(),
        to_player.len()
    );
    // It is told of no place of its own, and nobody of it.
    assert!(!bench.clients[at].entered().contains(&NO_LOCAL_PLACE));
    for player in 0..2 {
        assert!(bench.clients[player].entered().iter().all(|&p| p < 2));
    }
    // The players were never started anew for it.
    assert_eq!(
        [bench.clients[0].model.syncs, bench.clients[1].model.syncs],
        syncs
    );
    assert_eq!(bench.game().roster_size(), 2);
    bench.assert_no_faults();
}

#[test]
fn what_a_spectator_sends_changes_nothing_for_the_players() {
    let mut bench = running();
    let at = spectate(&mut bench, SPECTATOR);
    assert!(bench.until(10_000, |b| b.game().spectator_watches(SPECTATOR)));
    let relays = bench.clients[1].relays_of(0).len();
    // Inputs, kit choices, wrong hash reports and an upload of its own.
    for _ in 0..20 {
        bench.clients[at].input(50, 0x41);
        bench.clients[at].profile_input(2);
        let clock = bench.clients[at].clock();
        bench.clients[at].report(clock / 50 * 50, 0xdead_beef);
        bench.run(50);
    }
    let clock = bench.clients[at].clock();
    bench.clients[at].upload(clock, b"not a world");
    bench.run(3_000);
    for player in 0..2 {
        assert!(
            bench.clients[player].relays_of(2).is_empty(),
            "player {player} got a spectator's input"
        );
        assert!(bench.clients[player].relays_of(u8::MAX).is_empty());
    }
    // Late inputs too, which a player's would be told to drop or move.
    for _ in 0..10 {
        bench.clients[at].input(-1_000, 0x41);
        bench.run(50);
    }
    bench.run(1_000);
    assert_eq!(bench.clients[1].relays_of(0).len(), relays);
    assert_eq!(bench.game().resyncs(0), 0);
    assert_eq!(bench.game().resyncs(1), 0);
    assert_eq!(bench.stages(), "LL");
    // Never an answer to an input of its own (0xEF..0xF5): its game would
    // index its player table at the place it does not have.
    let answers = bench.clients[at].got(|message| {
        matches!(
            message,
            ServerMessage::MoveInput { .. }
                | ServerMessage::DiscardInput { .. }
                | ServerMessage::MoveProfileInput { .. }
                | ServerMessage::DiscardProfileInput { .. }
        )
        .then_some(())
    });
    assert!(answers.is_empty(), "{} answers", answers.len());
    bench.assert_no_faults();
}

#[test]
fn a_spectator_never_holds_up_a_late_joiner() {
    let mut bench = running();
    let at = spectate(&mut bench, SPECTATOR);
    assert!(bench.until(10_000, |b| b.game().spectator_watches(SPECTATOR)));
    // Its game installs nothing more.
    bench.clients[at].auto_prepare = false;
    let place = bench.game().roster_size();
    bench
        .registry
        .join_running(
            bench.match_id,
            MatchPlayer {
                session_id: session_of(place),
                account_id: 100 + place as u32,
                profile: profile("newcomer", 0),
                bot: false,
                driven: false,
            },
        )
        .unwrap();
    // Its place on the bench is the roster's next: 2.
    bench
        .clients
        .insert(2, Client::growing(session_of(2), 3, MAP_BATTERIES));
    bench
        .addrs
        .insert(2, SocketAddr::from(([127, 0, 0, 1], 40_002)));
    bench.join(2);
    assert!(
        bench.until(15_000, |b| b.clients[0].entered().contains(&2)),
        "{}",
        bench.stages()
    );
    // The spectator, without the place, is sent nothing of it.
    let spectator = &bench.clients[at + 1];
    assert!(spectator.relays_of(2).is_empty());
    assert!(!spectator.entered().contains(&2));
    bench.assert_no_faults();
}

#[test]
fn a_watch_that_ends_lets_the_spectator_go_and_its_own_leave_is_told() {
    let mut bench = running();
    let at = spectate(&mut bench, SPECTATOR);
    assert!(bench.until(10_000, |b| b.game().spectator_watches(SPECTATOR)));
    // The lobby ends the watch (its place came): an 0xEA naming no place,
    // then its link closes; the lobby is told nothing.
    bench.server.store().matchmaking().stop_watching(SPECTATOR);
    assert!(bench.until(1_000, |b| b.clients[at].told_to_leave));
    // Then silence, longer than the 5 s that end a game's match link.
    bench.run(300);
    let heard = bench.clients[at].datagrams_received;
    bench.run_long(6_000);
    assert_eq!(bench.clients[at].datagrams_received, heard);
    assert!(!bench.clients[at].disconnected);
    assert!(
        bench
            .server
            .store()
            .matchmaking()
            .take_watchers_left()
            .is_empty()
    );
    assert!(!bench.game().spectator_watches(SPECTATOR));

    // Another one leaves by its own word (its menu's Leave match): told.
    const SECOND: u32 = SPECTATOR + 1;
    let second = spectate(&mut bench, SECOND);
    assert!(bench.until(10_000, |b| b.game().spectator_watches(SECOND)));
    bench.clients[second].leave();
    bench.run(500);
    assert_eq!(
        bench
            .server
            .store()
            .matchmaking()
            .take_watchers_left()
            .into_iter()
            .map(|(_, account)| account)
            .collect::<Vec<_>>(),
        [SPECTATOR_ACCOUNT]
    );
    assert!(bench.server.store().matchmaking().watched(SECOND).is_none());
    assert_eq!(bench.stages(), "LL");
}

#[test]
fn the_end_of_the_match_lets_its_spectators_go_and_counts_none_of_them() {
    let mut bench = running();
    let at = spectate(&mut bench, SPECTATOR);
    assert!(bench.until(10_000, |b| b.game().spectator_watches(SPECTATOR)));
    bench.clients[0].leave();
    bench.clients[1].leave();
    let result = (0..400)
        .find_map(|_| {
            bench.run(50);
            bench.results.try_recv().ok()
        })
        .expect("the match ends");
    assert_eq!(result.players.len(), 2);
    assert!(bench.until(5_000, |b| b.clients[at].told_to_leave));
    // Its match is not closed before its spectators are let go.
    bench.run_long(8_000);
    assert!(!bench.server.games.contains_key(&bench.match_id));
    assert!(
        bench
            .server
            .store()
            .matchmaking()
            .take_watchers_left()
            .is_empty()
    );
}

#[test]
fn a_spectator_of_no_match_here_is_turned_away() {
    let mut bench = running();
    let matchmaking = bench.server.store().matchmaking();
    let seated = Player {
        session_id: 0x7_0100,
        account_id: 790,
        profile_id: 1,
    };
    let other = Player {
        session_id: 0x7_0101,
        account_id: 791,
        profile_id: 1,
    };
    let lobby_match = matchmaking
        .seat_teams([&[seated], &[other]], bench.now())
        .unwrap();
    assert!(matchmaking.watch(
        SPECTATOR,
        Watcher {
            match_id: lobby_match,
            ticket: bench.match_id + 1,
            account_id: SPECTATOR_ACCOUNT,
            kind: WatchKind::Queued { delay: no_delay() },
        },
    ));
    let at = bench.clients.len();
    bench
        .clients
        .push(Client::growing(SPECTATOR, 2, MAP_BATTERIES));
    bench
        .addrs
        .push(SocketAddr::from(([127, 0, 0, 1], 45_000 + at as u16)));
    bench.clients[at].connect();
    assert!(bench.until(2_000, |b| b.clients[at].disconnected));
    assert!(bench.clients[at].model.options.is_none());
}

#[test]
fn a_spectator_before_the_start_holds_nothing_up_and_watches_from_the_first_world() {
    let mut bench = Bench::growing(2, MAP_BATTERIES, Settings::default());
    bench.join(0);
    let at = spectate(&mut bench, SPECTATOR);
    bench.join(1);
    bench.run_long(25_000);
    assert_eq!(bench.game().rule_state(), state::IN_PROCESS);
    assert_eq!(bench.stages(), "LL");
    assert!(
        bench.until(10_000, |b| b.game().spectator_watches(SPECTATOR)),
        "{:?}",
        bench.clients[at].faults
    );
    bench.assert_no_faults();
}

/// A spectator that watches, its client's index.
fn watching(bench: &mut Bench) -> usize {
    let at = spectate(bench, SPECTATOR);
    assert!(bench.until(10_000, |b| b.game().spectator_watches(SPECTATOR)));
    at
}

#[test]
fn a_spectator_out_of_step_alone_starts_again_from_a_world() {
    let mut bench = running();
    let at = watching(&mut bench);
    let syncs = [bench.clients[0].model.syncs, bench.clients[1].model.syncs];
    let generation = bench.game().spectator_generation(SPECTATOR);
    let worlds = bench.clients[at].snapshots().len();
    // Its game reports other worlds than the players' from now on.
    bench.clients[at].no_hashes = true;
    for _ in 0..40 {
        let clock = bench.clients[at].clock();
        bench.clients[at].report(clock / 50 * 50, 0xdead_beef);
        bench.run(50);
    }
    assert!(
        bench.until(5_000, |b| b.game().spectator_generation(SPECTATOR)
            != generation),
        "the spectator was not started again"
    );
    assert!(bench.until(10_000, |b| b.game().spectator_watches(SPECTATOR)));
    assert!(bench.clients[at].snapshots().len() > worlds);
    // The players: nothing new.
    assert_eq!(
        [bench.clients[0].model.syncs, bench.clients[1].model.syncs],
        syncs
    );
    assert_eq!(bench.game().resyncs(0), 0);
    assert_eq!(bench.game().resyncs(1), 0);
    assert_eq!(bench.stages(), "LL");
    // And its next starts are as rare as a player's.
    bench.run_long(3_000);
    let restarted = bench.clients[at].snapshots().len();
    for _ in 0..40 {
        let clock = bench.clients[at].clock();
        bench.clients[at].report(clock / 50 * 50, 0xdead_beef);
        bench.run(50);
    }
    assert!(bench.clients[at].snapshots().len() <= restarted + 1);
    bench.assert_no_faults();
}

/// Review of 2026-11-13: a delayed spectator's reports are of ticks the
/// delay back, and the players' majorities were kept 30 s: at the default
/// delay none was judged, and one that loaded wrong stayed wrong.
#[test]
fn a_spectator_out_of_step_at_the_default_delay_starts_again_from_a_world() {
    let mut bench = running();
    let delay = LiveDelay::DEFAULT;
    bench.run_long(delay.as_secs() * 1_000 + 5_000);
    let at = spectate_as(&mut bench, SPECTATOR, WatchKind::Live { delay, view: None });
    assert!(bench.until(10_000, |b| b.game().spectator_watches(SPECTATOR)));
    let generation = bench.game().spectator_generation(SPECTATOR);
    let behind = bench.clients[0].clock() - bench.clients[at].clock();
    assert!(behind >= delay.as_secs() as u32 * 1_000 - 1_000, "{behind}");
    bench.clients[at].no_hashes = true;
    for _ in 0..40 {
        let clock = bench.clients[at].clock();
        bench.clients[at].report(clock / 50 * 50, 0xdead_beef);
        bench.run(50);
    }
    assert!(
        bench.until(5_000, |b| b.game().spectator_generation(SPECTATOR)
            != generation),
        "the spectator was not started again"
    );
    assert!(bench.until(10_000, |b| b.game().spectator_watches(SPECTATOR)));
    assert_eq!(bench.game().resyncs(0), 0);
    assert_eq!(bench.game().resyncs(1), 0);
    bench.assert_no_faults();
}

#[test]
fn a_spectator_in_step_is_left_alone() {
    let mut bench = running();
    let at = watching(&mut bench);
    let worlds = bench.clients[at].snapshots().len();
    let generation = bench.game().spectator_generation(SPECTATOR);
    bench.run_long(10_000);
    assert_eq!(bench.clients[at].snapshots().len(), worlds);
    assert_eq!(bench.game().spectator_generation(SPECTATOR), generation);
    bench.assert_no_faults();
}

#[test]
fn a_spectators_reports_never_vouch_for_a_world() {
    // One player reports, the other does not: the spectator's reports of
    // the same ticks are no second voucher. The worlds the match trusts are
    // those of the same match without it.
    let trusted = |spectator: bool| {
        let mut bench = running();
        if spectator {
            watching(&mut bench);
        } else {
            bench.run_long(1_000);
        }
        bench.clients[1].no_hashes = true;
        let mut seen = Vec::new();
        for _ in 0..40 {
            bench.run_long(250);
            seen.push((
                bench.game().observed().map(|(time, _)| time),
                bench.game().trusted_by(),
            ));
        }
        assert_eq!(bench.game().resyncs(0), 0);
        seen
    };
    assert_eq!(trusted(true), trusted(false));
}

/// Review of 2026-11-05: a newcomer that failed left its place with no
/// barrier, and every spectator waited the whole match, asking the players
/// for a world each second.
#[test]
fn a_place_whose_newcomer_failed_keeps_no_spectator_waiting() {
    let mut bench = running();
    let place = bench.game().roster_size();
    bench
        .registry
        .join_running(
            bench.match_id,
            MatchPlayer {
                session_id: session_of(place),
                account_id: 100 + place as u32,
                profile: profile("never-came", 0),
                bot: false,
                driven: false,
            },
        )
        .unwrap();
    // Its game never connects: let go after 20 s.
    bench.run_long(21_000);
    assert!(bench.registry.seat(session_of(place)).is_none());
    let at = spectate(&mut bench, SPECTATOR);
    assert!(
        bench.until(10_000, |b| b.game().spectator_watches(SPECTATOR)),
        "{:?}",
        bench.clients[at].faults
    );
}

/// The longest delay `LiveDelay` allows still finds a world to start from: a match keeps 60 s.
#[test]
fn a_live_windows_spectator_at_the_longest_delay_still_starts() {
    let mut bench = running();
    let delay = session::LiveDelay::from_secs(u64::from(session::LiveDelay::MAX_SECONDS)).unwrap();
    bench.until(62_000, |_| false);
    let at = spectate_as(&mut bench, SPECTATOR, WatchKind::Live { delay, view: None });
    assert!(
        bench.until(8_000, |b| b.clients[at].model.is_synced()),
        "{:?}",
        bench.clients[at].faults
    );
}

/// The Live window's spectator of a public match watches it 10 s behind:
/// its game's clock and world are that far back, its relays come as its
/// clock reaches them, and the players see nothing of it. The place its
/// watcher asked to see is told its game once, when it watches.
#[test]
fn a_live_windows_spectator_watches_behind_the_match() {
    let mut bench = running();
    let delay = session::LiveDelay::from_secs(10).unwrap();
    let view = Some(1);
    let at = spectate_as(&mut bench, SPECTATOR, WatchKind::Live { delay, view });
    assert!(
        bench.until(8_000, |b| b.clients[at].model.is_synced()),
        "{:?}",
        bench.clients[at].faults
    );
    bench.run(200);
    assert_eq!(
        views_told(&bench.clients[at]),
        [replay_control::ServerNote::view(1).to_vec()]
    );
    let behind = |bench: &Bench| i64::from(bench.clients[0].clock()) - i64::from(bench.clients[at].clock());
    assert!((9_000..=11_000).contains(&behind(&bench)), "{}", behind(&bench));
    let world = bench.clients[at].snapshots()[0].1;
    assert!(bench.clients[0].clock() - world >= 10_000, "{world}");
    // Player 0's inputs: at once to player 1, 10 s later to it.
    for _ in 0..20 {
        bench.clients[0].input(50, 0x21);
        bench.run(50);
    }
    bench.run(500);
    let sent = bench.clients[1].relays_of(0);
    assert!(sent.len() >= 20);
    let late = |bench: &Bench| {
        let got = bench.clients[at].relays_of(0);
        sent.iter().rev().take(15).filter(|input| got.contains(input)).count()
    };
    assert_eq!(late(&bench), 0, "not yet");
    bench.run_long(10_000);
    assert_eq!(late(&bench), 15);
    assert!((9_000..=11_000).contains(&behind(&bench)), "{}", behind(&bench));
    assert_eq!(bench.clients[at].snapshots().len(), 1, "never started again");
    assert_eq!(views_told(&bench.clients[at]).len(), 1, "told once");
    bench.assert_no_faults();
}

/// Review of 2026-11-09: a delayed spectator was sent every leave after its world up to the
/// present, so a leave inside the delay reached it at once and again when its clock got there.
#[test]
fn a_delayed_spectator_gets_a_leave_once_when_its_clock_reaches_it() {
    let mut bench = Bench::growing(3, MAP_BATTERIES, Settings::default());
    bench.join_all();
    bench.run_long(25_000);
    assert_eq!(bench.game().rule_state(), state::IN_PROCESS);
    let delay = session::LiveDelay::from_secs(10).unwrap();
    bench.clients[2].leave();
    bench.run(500);
    let at = spectate_as(&mut bench, SPECTATOR, WatchKind::Live { delay, view: None });
    assert!(
        bench.until(8_000, |b| b.clients[at].model.is_synced()),
        "{:?}",
        bench.clients[at].faults
    );
    bench.run(200);
    // Taken or refused for its time, it would have come.
    let leaves_heard = |client: &Client| {
        client.left().len()
            + client
                .skipped
                .iter()
                .filter(|(message, _)| *message == survarium::v026g::match_msgs::id::PLAYER_LEFT)
                .count()
    };
    assert_eq!(leaves_heard(&bench.clients[at]), 0, "a leave from its future");
    bench.run_long(11_000);
    assert_eq!(bench.clients[at].left(), [2], "when its clock got there");
    assert_eq!(leaves_heard(&bench.clients[at]), 1, "once");
    bench.assert_no_faults();
}

/// Review of 2026-11-09: a client that skips the way back and connects again within each 7 s of
/// silence kept a match everyone had left open, and with it its results and pay.
#[test]
fn a_spectator_coming_back_holds_no_abandoned_match_open() {
    let mut bench = running();
    let at = watching(&mut bench);
    bench.clients[0].leave();
    bench.clients[1].leave();
    assert!(bench.until(5_000, |b| b.clients[at].told_to_leave));
    for round in 0..4 {
        bench.clients.push(Client::growing(SPECTATOR, 2, MAP_BATTERIES));
        bench
            .addrs
            .push(SocketAddr::from(([127, 0, 0, 1], 46_000 + round)));
        bench.clients.last_mut().unwrap().connect();
        bench.run_long(5_000);
    }
    assert!(
        !bench.server.games.contains_key(&bench.match_id),
        "the match is still open"
    );
    assert!(bench.clients.last().unwrap().disconnected, "turned away");
}

/// The owner's rule of 2026-11-09: Queue and spectate runs behind the match by the delay it was
/// given (the Live delay), as a Live window's watcher does.
#[test]
fn queue_and_spectate_watches_behind_the_match() {
    let mut bench = running();
    let delay = LiveDelay::from_secs(10).unwrap();
    let at = spectate_as(&mut bench, SPECTATOR, WatchKind::Queued { delay });
    assert!(
        bench.until(8_000, |b| b.clients[at].model.is_synced()),
        "{:?}",
        bench.clients[at].faults
    );
    bench.run(200);
    let behind = i64::from(bench.clients[0].clock()) - i64::from(bench.clients[at].clock());
    assert!((9_000..=11_000).contains(&behind), "{behind}");
    bench.assert_no_faults();
}

/// Review of 2026-11-09: a spectator waiting for a fresh world asked the players for one every
/// second, and one that ignores each 0xE5 waits for ever: it got them uploading a world a second.
#[test]
fn a_stuck_spectator_asks_the_players_for_nothing() {
    let requests = |stuck: bool| {
        let mut bench = running();
        if stuck {
            let at = spectate(&mut bench, SPECTATOR);
            bench.clients[at].mute_time_replies = true;
        }
        let before = bench.clients[0].upload_requests;
        bench.run_long(15_000);
        bench.clients[0].upload_requests - before
    };
    assert_eq!(requests(true), requests(false));
}

/// Review of 2026-11-09: a spectator that never acks grew its send queue without limit (each
/// relay kept for ever) and every poll of the match thread walked it.
#[test]
fn a_spectator_that_never_acks_piles_up_nothing() {
    let mut bench = running();
    let at = spectate(&mut bench, SPECTATOR);
    bench.clients[at].mute_acks = true;
    let mut most = 0;
    for _ in 0..1_200 {
        bench.clients[0].input(50, 0x21);
        bench.clients[1].input(50, 0x22);
        bench.run(50);
        most = most.max(bench.game().spectator_backlog(SPECTATOR).unwrap_or(0));
    }
    // The relays of about one wait for its world: 40 a second for 15 s.
    assert!(most < 1_000, "{most} messages queued");
    bench.assert_no_faults();
}

/// Review of 2026-11-09: a spectator's turn walked all the match's relays on every poll, and a
/// poll follows every datagram the server takes. Once a tick is as often as anything comes due.
#[test]
fn a_spectator_walks_the_matchs_relays_once_a_tick() {
    let mut bench = running();
    let at = watching(&mut bench);
    let before = bench.game().spectator_list_walks(SPECTATOR).unwrap();
    bench.run(5_000);
    let walks = bench.game().spectator_list_walks(SPECTATOR).unwrap() - before;
    assert!((90..=101).contains(&walks), "{walks} walks in 5 s");
    assert!(bench.clients[at].model.is_synced());
    bench.assert_no_faults();
}

/// Every place stands in every game's next worlds with `kit` as its current one.
fn in_the_world(bench: &mut Bench, kit: u8) {
    for client in &mut bench.clients {
        for state in &mut client.sim.player_states {
            *state = Some(survarium::v026g::match_msgs::players::PlayerState {
                current_index: kit,
                target_index: kit,
                pickup: weapon_pickup::Record::EMPTY,
                items: vec![1],
                alive: true,
                has_to_die: false,
                look_pitch: 0.0,
                rotation_y: 0.0,
                actions_mask: 0,
                effects: 0,
            });
        }
    }
}

/// A spectator's view notes taken, in order.
fn views_told(client: &Client) -> Vec<Vec<u8>> {
    client
        .replay_notes
        .iter()
        .filter(|payload| matches!(replay_control::ServerNote::parse(payload), Some(replay_control::ServerNote::View(_))))
        .cloned()
        .collect()
}

/// A spectator's kits notes taken: each place's kit numbers.
fn kits_told(client: &Client) -> Vec<(u8, Vec<u8>)> {
    client
        .replay_notes
        .iter()
        .filter_map(|payload| match replay_control::ServerNote::parse(payload)? {
            replay_control::ServerNote::Kits(note) => Some((note.place, note.kits().collect())),
            _ => None,
        })
        .collect()
}

/// `kits_told`, the last list of `place`.
fn kits_of_place(client: &Client, place: u8) -> Option<Vec<u8>> {
    kits_told(client).into_iter().rev().find(|(of, _)| *of == place).map(|(_, kits)| kits)
}

/// Every playable kit of each place, as a replay's viewer is told: with no player in the world
/// yet, and once.
#[test]
fn a_spectator_is_told_every_playable_kit_once_it_watches() {
    let mut bench = running();
    let at = spectate(&mut bench, SPECTATOR);
    assert!(
        bench.until(8_000, |b| b.clients[at].model.is_synced()),
        "{:?}",
        bench.clients[at].faults
    );
    bench.run(500);
    assert_eq!(kits_of_place(&bench.clients[at], 0), Some(vec![0]), "{:?}", kits_told(&bench.clients[at]));
    assert_eq!(kits_of_place(&bench.clients[at], 1), Some(vec![0]));
    let told = kits_told(&bench.clients[at]).len();
    in_the_world(&mut bench, 0);
    bench.clients[0].profile_input(1);
    bench.run_long(10_000);
    assert_eq!(kits_told(&bench.clients[at]).len(), told, "nothing more told");
    bench.assert_no_faults();
}

/// A delayed spectator too: its clock keeps nothing of the kits from it.
#[test]
fn a_delayed_spectator_is_told_the_kits_as_soon_as_it_watches() {
    let mut bench = running();
    let delay = session::LiveDelay::from_secs(10).unwrap();
    let at = spectate_as(&mut bench, SPECTATOR, WatchKind::Live { delay, view: None });
    assert!(bench.until(8_000, |b| b.clients[at].model.is_synced()), "{:?}", bench.clients[at].faults);
    bench.run(500);
    assert_eq!(kits_of_place(&bench.clients[at], 0), Some(vec![0]), "{:?}", kits_told(&bench.clients[at]));
    bench.assert_no_faults();
}

/// Review of 2026-11-13: a delayed spectator installs a late joiner's place when the players do,
/// a delay before its clock reaches that time, and the players' majorities of the ticks before
/// were of worlds without the place: its reports of them are no sign of a world out of step.
#[test]
fn a_late_join_starts_no_delayed_spectator_again() {
    let mut bench = running();
    let delay = session::LiveDelay::from_secs(10).unwrap();
    bench.run_long(12_000);
    let at = spectate_as(&mut bench, SPECTATOR, WatchKind::Live { delay, view: None });
    assert!(bench.until(8_000, |b| b.game().spectator_watches(SPECTATOR)));
    bench.run_long(2_000);
    let generation = bench.game().spectator_generation(SPECTATOR);
    let place = bench.game().roster_size();
    bench
        .registry
        .join_running(bench.match_id, super::late_join::grenadier(place))
        .unwrap();
    bench
        .clients
        .insert(2, Client::growing(session_of(2), 3, MAP_BATTERIES));
    bench
        .addrs
        .insert(2, SocketAddr::from(([127, 0, 0, 1], 40_002)));
    bench.join(2);
    let at = at + 1;
    assert!(bench.clients[at].prepares_received > 0);
    bench.run_long(15_000);
    assert_eq!(
        bench.game().spectator_generation(SPECTATOR),
        generation,
        "started again"
    );
    assert!(bench.game().spectator_watches(SPECTATOR));
    bench.assert_no_faults();
}

/// Review of 2026-11-13: a place's install was asked again every second, each kept until acked:
/// a spectator's game that acks nothing piled them up. One goes at a time.
#[test]
fn a_spectator_that_never_installs_piles_up_no_installs() {
    let mut bench = running();
    let at = watching(&mut bench);
    bench.clients[at].auto_prepare = false;
    bench.clients[at].mute_acks = true;
    let place = bench.game().roster_size();
    bench
        .registry
        .join_running(bench.match_id, super::late_join::player(place))
        .unwrap();
    bench
        .clients
        .insert(2, Client::growing(session_of(2), 3, MAP_BATTERIES));
    bench
        .addrs
        .insert(2, SocketAddr::from(([127, 0, 0, 1], 40_002)));
    bench.join(2);
    bench.run_long(2_000);
    let before = bench.game().spectator_backlog(SPECTATOR).unwrap();
    bench.run_long(20_000);
    let after = bench.game().spectator_backlog(SPECTATOR).unwrap();
    assert!(after <= before + 2, "{before} then {after} messages queued");
}

/// Review of 2026-11-13: a spectator let go swallowed what came from its address for its 7 s
/// of silence, a new connection of its game too (another watch; a late join into its room's
/// match): the game drops a link silent for 5 s.
#[test]
fn a_spectator_let_go_that_connects_again_is_heard_at_once() {
    let mut bench = running();
    let at = watching(&mut bench);
    let matchmaking = bench.server.store().matchmaking();
    let watcher = matchmaking.stop_watching(SPECTATOR).unwrap();
    assert!(bench.until(1_000, |b| b.clients[at].told_to_leave));
    bench.run(300);
    // Its game watches again, from the same address.
    assert!(bench.server.store().matchmaking().watch(SPECTATOR, watcher));
    let players = bench.game().roster_size();
    bench.clients[at] = Client::growing(SPECTATOR, players, MAP_BATTERIES);
    bench.clients[at].connect();
    assert!(bench.until(1_000, |b| b.clients[at].model.options.is_some()));
    bench.clients[at].ready();
    assert!(bench.until(10_000, |b| b.game().spectator_watches(SPECTATOR)));
    bench.assert_no_faults();
}

/// A busy 5 ms of the match: player 0 makes two precise inputs a tick, as a firing player
/// does, player 1 one.
fn busy(bench: &mut Bench) {
    use survarium::v026g::match_msgs::{PlayerInput, WireTime};
    if bench.ms.is_multiple_of(50) {
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
}

/// The bench busy until a spectator `delay_ms` behind the match would have the newest world the
/// match keeps (every 5 s, at 4 550 ms and on) `age_ms` behind its clock.
fn busy_until_world_age(bench: &mut Bench, delay_ms: u32, age_ms: u32) {
    let end = bench.ms + 5_000;
    while bench.ms < end || (bench.clock() + 5_000 - delay_ms - 4_550) % 5_000 != age_ms {
        busy(bench);
    }
}

/// Player 0's inputs the game had when the world it took came: those after the 0xE5 that went
/// with that world.
fn inputs_with_world(client: &Client) -> usize {
    let taken = &client.taken;
    let world = taken
        .iter()
        .rposition(|message| matches!(message, ServerMessage::Snapshot { .. }))
        .expect("a world");
    let ServerMessage::Snapshot { generation, .. } = taken[world] else {
        unreachable!()
    };
    let timed = taken[..world]
        .iter()
        .rposition(|message| {
            matches!(message, ServerMessage::SetTime { generation: g, .. } if *g == generation)
        })
        .expect("its 0xE5");
    let mut inputs = Vec::new();
    for message in &taken[timed..world] {
        if let ServerMessage::InputRelay { player: 0, input } = message
            && !inputs.contains(input)
        {
            inputs.push(*input);
        }
    }
    inputs.len()
}

/// Review of 2026-11-13: a delayed spectator took a world up to 1.5 s behind its clock, with the
/// inputs since: a player making two inputs a tick put more of them after it than its game keeps
/// (27), and it played wrong from its first tick. It waits for a newer world.
#[test]
fn a_delayed_spectators_world_comes_with_no_more_inputs_than_its_game_keeps() {
    let mut bench = running();
    let delay = LiveDelay::from_secs(3).unwrap();
    busy_until_world_age(&mut bench, 3_000, 900);
    let at = spectate_as(&mut bench, SPECTATOR, WatchKind::Live { delay, view: None });
    let end = bench.ms + 15_000;
    while bench.ms < end && !bench.game().spectator_watches(SPECTATOR) {
        busy(&mut bench);
    }
    assert!(bench.game().spectator_watches(SPECTATOR));
    let inputs = inputs_with_world(&bench.clients[at]);
    assert!(inputs <= 26, "{inputs} inputs of player 0 after its world");
    bench.assert_no_faults();
}

/// The same for the inputs it is fed while its world is on its way: a world held up is given
/// up once they are past the ring, and it starts again from a newer one.
#[test]
fn a_delayed_spectators_world_held_up_past_the_ring_is_given_up() {
    let mut bench = running();
    let delay = LiveDelay::from_secs(3).unwrap();
    busy_until_world_age(&mut bench, 3_000, 250);
    let at = spectate_as(&mut bench, SPECTATOR, WatchKind::Live { delay, view: None });
    // Its first world's datagrams are lost for a second.
    let held_until = bench.ms + 1_000;
    let clock = Arc::new(std::sync::atomic::AtomicU64::new(bench.ms));
    let now = Arc::clone(&clock);
    bench.clients[at].lose = Some(Box::new(move |datagram| {
        now.load(Ordering::Relaxed) < held_until
            && udp_transport::wire::parse(
                udp_transport::Table::Match,
                udp_transport::Direction::ToClient,
                datagram,
                1,
            )
            .is_ok_and(|parsed| {
                parsed
                    .messages
                    .iter()
                    .any(|message| message.id == survarium::v026g::match_msgs::id::SNAPSHOT)
            })
    }));
    let end = bench.ms + 15_000;
    while bench.ms < end && !bench.game().spectator_watches(SPECTATOR) {
        busy(&mut bench);
        clock.store(bench.ms, Ordering::Relaxed);
    }
    assert!(bench.game().spectator_watches(SPECTATOR));
    let mut generations = bench.clients[at].got(|message| match message {
        ServerMessage::SetTime { generation, .. } => Some(*generation),
        _ => None,
    });
    generations.dedup();
    assert!(generations.len() >= 2, "started again: {generations:?}");
    let inputs = inputs_with_world(&bench.clients[at]);
    assert!(inputs <= 26, "{inputs} inputs of player 0 after its world");
}

/// Review of 2026-11-13: a spectator's game that connected again from
/// another port was its session's spectator anew, with no starts behind it:
/// each round (0xD8, probe, 0xDA, its answer to the 0xE5) packed a world on
/// the match thread, as fast as it went round, and against a live match. Its
/// session's starts go on from its last: one a second at most.
#[test]
fn a_spectator_connecting_again_and_again_starts_from_a_world_once_a_second() {
    let mut bench = running();
    let first = watching(&mut bench);
    let players = bench.game().roster_size();
    let started = bench.ms;
    let mut games = vec![first];
    for _ in 0..15 {
        bench.clients[*games.last().unwrap()].silent = true;
        let at = bench.clients.len();
        bench
            .clients
            .push(Client::growing(SPECTATOR, players, MAP_BATTERIES));
        bench
            .addrs
            .push(SocketAddr::from(([127, 0, 0, 1], 45_000 + at as u16)));
        bench.clients[at].connect();
        assert!(bench.until(1_000, |b| b.clients[at].model.options.is_some()));
        bench.clients[at].ready();
        games.push(at);
        bench.run(200);
    }
    let lasted = bench.ms - started;
    let worlds: usize = games
        .iter()
        .skip(1)
        .map(|&at| bench.clients[at].snapshots().len())
        .sum();
    assert!(
        worlds as u64 <= lasted / 1_000 + 1,
        "{worlds} worlds in {lasted} ms"
    );
    assert!(bench.until(3_000, |b| b.game().spectator_watches(SPECTATOR)));
    bench.assert_no_faults();
}
