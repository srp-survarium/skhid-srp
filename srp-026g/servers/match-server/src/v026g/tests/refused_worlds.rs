//! A refused world is never relayed, and every game that waits for one gets
//! a defined end when none can be had (`game/refused_worlds.rs`).

use super::late_join::reserve;
use super::*;
use crate::v026g::registry::MatchEnd;
use crate::v026g::world_validation::{Roster, check};
use session::matchmaking_policy::Closed;
use survarium::v026g::match_msgs::ServerMessage;

/// Every world the server sent any game passes the server's check: no
/// refused world was relayed.
fn assert_every_world_sent_passes(bench: &Bench) {
    let formed = bench.registry.formed(bench.match_id).unwrap();
    let mut sent = 0;
    for (place, client) in bench.clients.iter().enumerate() {
        let worlds = client.got(|message| match message {
            ServerMessage::Snapshot { time, body, .. } => Some((*time, body.clone())),
            _ => None,
        });
        for (time, body) in worlds {
            assert!(
                check(&body, time, srp_validate::MAX_PLAYERS, Roster::of(&formed)).is_ok(),
                "player {place} was sent a world the check refuses, at {time} ms"
            );
            sent += 1;
        }
    }
    assert!(sent > 0, "no world was sent");
}

/// The client connects and loads (its initial world goes up as it does),
/// without waiting to get in.
fn load(bench: &mut Bench, place: usize) {
    bench.clients[place].connect();
    assert!(
        bench.until(1_000, |bench| bench.clients[place].model.options.is_some()),
        "player {place} got no static info"
    );
    bench.clients[place].ready();
}

/// A stock game (no uploads: never a source) of a growing match.
fn stock(place: usize, players: usize, map_id: u8) -> Client {
    let mut client = Client::new(session_of(place), false, players, map_id);
    client.model.growing_roster = true;
    client.sim.snapshot_players = 20;
    client
}

/// One human and the server's bot, with a check wrong for every world the
/// human's game writes: its initial world is refused, no other game can
/// give one, and the match is not made. Its game is told the end (0xEA of
/// its own place), and nothing of the match counts.
#[test]
fn a_match_whose_only_initial_world_is_refused_is_not_made() {
    let mut bench = Bench::growing_with_driven(2, MAP_TDM, Settings::default(), &[1]);
    bench.clients[0].sim.stray_objects_from = Some((0, 7));
    load(&mut bench, 0);
    assert!(bench.until(2_000, |bench| bench.clients[0].told_to_leave));
    assert_eq!(bench.game().uploads_refused(0), 1);
    assert!(!bench.game().has_clock());
    let result = bench.results.try_recv().expect("the match's result");
    assert_eq!(result.end, MatchEnd::NotMade);
    assert!(bench.clients[0].snapshots().is_empty());
    bench.assert_no_faults();
}

/// Two games whose worlds the check refuses (forged or broken, or a check
/// wrong for them) and a late joiner: each is asked in turn, each world is
/// refused, and the joiner, with no source left to ask, fails its admission
/// (the lobby hears it: its seat is freed) and is removed. The match takes
/// no joiner until a world passes again.
#[test]
fn a_late_joiner_whose_every_source_is_refused_is_sent_back() {
    let mut bench = Bench::growing(2, MAP_BATTERIES, Settings::default());
    bench.join_all();
    bench.run_long(10_000);
    for client in &mut bench.clients {
        client.forged_uploads = Some(7);
    }
    let place = reserve(&mut bench);
    load(&mut bench, place);
    assert!(
        bench.until(30_000, |bench| bench.clients[place].disconnected),
        "{}",
        bench.stages()
    );
    assert!(bench.clients[place].snapshots().is_empty());
    assert!(bench.game().uploads_refused(0) >= 1);
    assert!(bench.game().uploads_refused(1) >= 1);
    assert!(bench.registry.seat(session_of(place)).is_none());
    assert!(bench.stages().starts_with("LL"), "{}", bench.stages());
    assert_eq!(bench.game().admission(bench.now()), Err(Closed::NoWorld));
    assert_every_world_sent_passes(&bench);
    bench.assert_no_faults();
}

/// One forging game among honest ones: its worlds are refused and relayed
/// to nobody, the honest games are asked instead, and a late joiner gets an
/// honest world and plays.
#[test]
fn one_forging_game_among_honest_ones_relays_nothing_forged() {
    let mut bench = Bench::growing(3, MAP_BATTERIES, Settings::default());
    bench.join_all();
    bench.run_long(10_000);
    bench.clients[0].forged_uploads = Some(7);
    bench.run_long(15_000);
    let place = reserve(&mut bench);
    bench.join(place);
    assert!(bench.game().uploads_refused(0) >= 1);
    assert_eq!(bench.game().uploads_refused(1), 0);
    assert_eq!(bench.game().uploads_refused(2), 0);
    assert_eq!(bench.clients[place].snapshots().len(), 1);
    assert_eq!(bench.stages(), "LLLL");
    assert_every_world_sent_passes(&bench);
    bench.assert_no_faults();
}

/// A game whose uploads are refused again and again while another's pass
/// (four players, two of them stock games: both uploaders are asked each
/// time): resynced from the other's world at its third refusal in a row,
/// removed at the third after that. Nothing it forged was relayed.
#[test]
fn a_game_refused_again_and_again_is_resynced_then_removed() {
    let mut bench = Bench::growing(4, MAP_BATTERIES, Settings::default());
    bench.clients[2] = stock(2, 4, MAP_BATTERIES);
    bench.clients[3] = stock(3, 4, MAP_BATTERIES);
    bench.join_all();
    bench.run_long(10_000);
    assert_eq!(bench.game().resyncs(0), 0);
    bench.clients[0].forged_uploads = Some(7);
    assert!(
        bench.until(60_000, |bench| bench.game().resyncs(0) == 1),
        "{}",
        bench.stages()
    );
    assert_eq!(bench.game().uploads_refused(0), 3);
    assert!(!bench.clients[0].disconnected);
    assert!(
        bench.until(60_000, |bench| bench.clients[0].disconnected),
        "{}",
        bench.stages()
    );
    assert_eq!(bench.game().uploads_refused(0), 6);
    assert_eq!(bench.game().uploads_refused(1), 0);
    assert!(bench.stages().starts_with("gLLL"), "{}", bench.stages());
    assert_every_world_sent_passes(&bench);
}

/// A player whose game came back into the match (a restart) waits for a
/// world; the one game it could take one from gives worlds the check
/// refuses: it is removed, its seat kept as for a lost link.
#[test]
fn a_player_back_in_the_match_whose_every_source_is_refused_is_removed() {
    let mut bench = Bench::growing(2, MAP_BATTERIES, Settings::default());
    bench.join_all();
    bench.run_long(10_000);
    bench.clients[0].forged_uploads = Some(7);
    bench.restart(1);
    load(&mut bench, 1);
    assert!(
        bench.until(30_000, |bench| bench.clients[1].disconnected),
        "{}",
        bench.stages()
    );
    assert!(bench.clients[1].snapshots().is_empty());
    assert_eq!(bench.game().uploads_refused(0), 1);
    assert!(bench.registry.seat(session_of(1)).is_some());
    assert_eq!(bench.stages(), "Lg");
    assert_every_world_sent_passes(&bench);
}

/// Four players and, from 10 s on, a world every game has that the check
/// refuses (as rev50's server bot NaN in every game's simulation,
/// rec026g_20261106-211841_03439962: 174 uploads over 185 s refused, the
/// games' hashes agreeing; or `forged` every game's uploads, as an upload
/// fault every game shares). Nothing is relayed, and no game's refusal is
/// its own: nobody is resynced or removed for them, and the match goes on.
/// A joiner gets no world (`Closed::NoWorld`), and a player back in the
/// match, with no world to take, is removed alone.
fn a_world_every_game_shares(forged: bool) {
    let mut bench = Bench::growing(4, MAP_BATTERIES, Settings::default());
    bench.join_all();
    bench.run_long(10_000);
    let from = bench.clock();
    for client in &mut bench.clients {
        if forged {
            client.forged_uploads = Some(7);
        } else {
            client.sim.stray_objects_from = Some((from, 7));
        }
    }
    bench.run_long(185_000);
    for place in 0..4 {
        assert!(bench.game().uploads_refused(place) >= 1, "{place}");
        assert_eq!(bench.game().resyncs(place), 0, "{place}");
        assert!(!bench.clients[place].disconnected, "{place}");
    }
    assert_eq!(bench.stages(), "LLLL");
    assert!(bench.results.try_recv().is_err(), "the match goes on");
    assert_eq!(bench.game().admission(bench.now()), Err(Closed::NoWorld));
    bench.restart(3);
    load(&mut bench, 3);
    assert!(
        bench.until(30_000, |bench| bench.clients[3].disconnected),
        "{}",
        bench.stages()
    );
    assert!(bench.clients[3].snapshots().is_empty());
    assert!(bench.stages().starts_with("LLL"), "{}", bench.stages());
    assert_every_world_sent_passes(&bench);
}

#[test]
fn a_world_every_game_simulates_refused_removes_nobody() {
    a_world_every_game_shares(false);
}

#[test]
fn uploads_every_game_forges_refused_remove_nobody() {
    a_world_every_game_shares(true);
}

/// A's initial world is refused; B is an honest game whose 0xDA reaches
/// the server before its first chunk (the order on the wire is not kept, a
/// first chunk can be lost and sent again). The match is not called off
/// for B looking like a stock game: B's world comes, and the match starts
/// on it.
#[test]
fn an_honest_game_ready_before_its_first_chunk_still_gives_the_first_world() {
    let mut bench = Bench::growing(2, MAP_BATTERIES, Settings::default());
    bench.clients[0].sim.stray_objects_from = Some((0, 7));
    load(&mut bench, 0);
    assert!(bench.until(2_000, |bench| bench.game().uploads_refused(0) == 1));
    bench.clients[1].connect();
    assert!(bench.until(1_000, |bench| bench.clients[1].model.options.is_some()));
    bench.clients[1].model.patched = false;
    bench.clients[1].ready();
    bench.clients[1].model.patched = true;
    bench.step(5);
    assert!(bench.results.try_recv().is_err(), "the match was not made");
    let initial_ms = bench.clients[1].initial_ms;
    let body = bench.clients[1].sim.world(initial_ms);
    bench.clients[1].upload(initial_ms, &body);
    bench.run_long(2_000);
    assert!(bench.game().has_clock());
    assert!(!bench.clients[1].told_to_leave);
}

/// A's initial world is refused and the other place's game never
/// connects: past the server's bound for games that do not come
/// (`Timing::unstarted`), that place cannot give a world either, and the
/// match is not made; before it, it is waited for.
fn a_refused_first_world_and_a_place_that_never_connects(mut bench: Bench) {
    let unstarted = crate::v026g::Timing::default().unstarted;
    bench.clients[0].sim.stray_objects_from = Some((0, 7));
    load(&mut bench, 0);
    assert!(bench.until(2_000, |bench| bench.game().uploads_refused(0) == 1));
    bench.run_long(u64::try_from(unstarted.as_millis()).unwrap() - 5_000);
    assert!(
        bench.results.try_recv().is_err(),
        "not made before the bound"
    );
    bench.run_long(10_000);
    let result = bench.results.try_recv().expect("the match's result");
    assert_eq!(result.end, MatchEnd::NotMade);
    assert!(bench.clients[0].told_to_leave);
}

#[test]
fn a_refused_first_world_and_a_place_that_never_connects_is_not_made() {
    a_refused_first_world_and_a_place_that_never_connects(Bench::growing(
        2,
        MAP_BATTERIES,
        Settings::default(),
    ));
}

#[test]
fn a_room_with_a_refused_first_world_and_a_place_that_never_connects_is_not_made() {
    a_refused_first_world_and_a_place_that_never_connects(Bench::room(
        &[0, 1],
        MAP_BATTERIES,
        Settings::default(),
        2,
    ));
}
