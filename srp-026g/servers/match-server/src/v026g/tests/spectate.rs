//! Spectators (Queue and spectate, `game/spectate.rs`): a game watching the
//! match from a room's queue, with no place.

use super::*;
use session::{Player, WatchKind, Watcher};
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
    spectate_as(bench, session, WatchKind::Queued)
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
    bench.run(500);
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
            kind: WatchKind::Queued,
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

#[test]
fn a_spectator_is_told_only_the_kits_a_world_shows_and_a_late_one_gets_them() {
    let mut bench = running();
    in_the_world(&mut bench, 0);
    bench.run_long(8_000);
    // Comes long after the start: the kept worlds are gone, the match's record is not.
    bench.run_long(90_000);
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
    // A pick the player never spawns with: told nothing more.
    bench.clients[0].profile_input(1);
    bench.run_long(10_000);
    assert_eq!(kits_of_place(&bench.clients[at], 0), Some(vec![0]), "a pick is no use of the kit");
    assert_eq!(kits_told(&bench.clients[at]).len(), told, "nothing more told");
    bench.assert_no_faults();
}

#[test]
fn a_delayed_spectator_is_told_a_kit_only_when_its_clock_reaches_it() {
    let mut bench = running();
    let delay = session::LiveDelay::from_secs(10).unwrap();
    let at = spectate_as(&mut bench, SPECTATOR, WatchKind::Live { delay, view: None });
    assert!(bench.until(8_000, |b| b.clients[at].model.is_synced()), "{:?}", bench.clients[at].faults);
    bench.run(200);
    in_the_world(&mut bench, 0);
    bench.run_long(6_000);
    assert_eq!(kits_of_place(&bench.clients[at], 0), None, "its clock has not reached that world yet");
    bench.run_long(14_000);
    assert_eq!(kits_of_place(&bench.clients[at], 0), Some(vec![0]), "{:?}", kits_told(&bench.clients[at]));
    // Its clock is 10 s behind the match: what it was told is what a world at or before it showed.
    assert!(kits_told(&bench.clients[at]).iter().all(|(_, kits)| kits == &vec![0u8]), "{:?}", kits_told(&bench.clients[at]));
    bench.assert_no_faults();
}
