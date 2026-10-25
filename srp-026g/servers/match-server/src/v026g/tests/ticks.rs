//! The per-tick protocol (`SRP_NETCODE=v069`, `game/ticks.rs`): every game
//! applies the same inputs at the same ticks, whatever comes late, out of
//! order, or not at all.

use super::*;
use survarium::v026g::match_msgs::{ServerMessage, rules::state};

/// Every input of every place, as each client's lists have it, over
/// `(from, to]`: the same for every client.
fn assert_same_inputs(bench: &Bench, from: u32, to: u32) {
    let places = bench.clients.len() as u8;
    for place in 0..places {
        let mut timelines = Vec::new();
        for (index, client) in bench.clients.iter().enumerate() {
            let per_tick = client.per_tick.as_ref().expect("a per-tick client");
            timelines.push((
                index,
                per_tick.timeline(index as u8, place, from, to),
            ));
        }
        let (first, reference) = &timelines[0];
        for (index, timeline) in &timelines[1..] {
            assert_eq!(
                timeline, reference,
                "place {place}'s inputs over ({from}, {to}] ms: client {index} against client \
                 {first}"
            );
        }
    }
}

/// No message of the stock protocol's relays went to anyone.
fn assert_no_relays(bench: &Bench) {
    for (place, client) in bench.clients.iter().enumerate() {
        let relays = client.got(|message| match message {
            ServerMessage::InputRelay { .. }
            | ServerMessage::InputsRelay { .. }
            | ServerMessage::ProfileInputRelay { .. }
            | ServerMessage::MoveInput { .. }
            | ServerMessage::DiscardInput { .. }
            | ServerMessage::MoveProfileInput { .. }
            | ServerMessage::DiscardProfileInput { .. }
            | ServerMessage::PlayerEntered { .. } => Some(message.id()),
            _ => None,
        });
        assert!(relays.is_empty(), "player {place} got {relays:02x?}");
    }
}

/// Each client makes an input every `every` ms for `ms`, at its own clock.
fn play(bench: &mut Bench, ms: u64, every: u64) {
    for n in 0..(ms / every) as u32 {
        for place in 0..bench.clients.len() {
            let client = &bench.clients[place];
            // A world that stands still makes no inputs.
            if client.per_tick.is_some()
                && client.model.is_synced()
                && client.frozen_at.is_none()
                && !client.closed
            {
                bench.clients[place].input(0, (place as u32) << 16 | n);
            }
        }
        bench.run(every);
    }
}

fn ticks_match(players: usize) -> Bench {
    let mut bench = Bench::ticks(players, MAP_BATTERIES, Settings::default());
    bench.join_all();
    bench
}

#[test]
fn a_match_of_the_per_tick_protocol_plays_on_updates_alone() {
    let mut bench = ticks_match(2);
    for client in &bench.clients {
        assert!(client.model.ticks.is_some(), "the 0xE4 trailer");
    }
    let from = bench.clock();
    play(&mut bench, 5_000, 50);
    bench.run(1_000);
    let to = bench.clock() - 600;
    bench.assert_no_faults();
    assert_no_relays(&bench);
    assert_same_inputs(&bench, from, to);
    for (place, client) in bench.clients.iter().enumerate() {
        let per_tick = client.per_tick.as_ref().unwrap();
        // An update a tick, each way.
        assert!(per_tick.sent >= 110, "player {place} sent {}", per_tick.sent);
        assert!(
            per_tick.updates >= 110,
            "player {place} took {}",
            per_tick.updates
        );
        // In time on a line of no delay: all confirmed, none waits.
        assert!(per_tick.confirmed >= 95, "{}", per_tick.confirmed);
        assert_eq!((per_tick.moved, per_tick.discarded), (0, 0));
        assert_eq!(per_tick.pending(), 0);
        assert_eq!(per_tick.unknown_outcomes, 0);
        assert_eq!(per_tick.inputs_past_history, 0);
        // The others' inputs: every one of them.
        let other = 1 - place as u8;
        assert!(per_tick.timeline(place as u8, other, from, to).len() >= 90);
    }
    assert_eq!(bench.game().rule_state(), state::COUNTDOWN);
}

#[test]
fn with_the_switch_off_a_game_that_announces_it_plays_the_stock_protocol() {
    let mut bench = Bench::growing(2, MAP_BATTERIES, Settings::default());
    for client in &mut bench.clients {
        client.announce_ticks = true;
    }
    bench.join_all();
    for client in &bench.clients {
        assert_eq!(client.model.ticks, None, "no 0xE4 trailer");
        assert!(client.per_tick.is_none());
    }
    // In play (entered at 400 ms): an input is relayed as ever.
    bench.run(1_000);
    bench.clients[0].input(0, 1);
    bench.run(1_000);
    assert_eq!(bench.clients[1].relays_of(0).len(), 1);
    assert!(
        bench.clients[1]
            .got(|message| matches!(message, ServerMessage::Update(_)).then_some(()))
            .is_empty()
    );
    bench.assert_no_faults();
}

#[test]
fn with_the_switch_on_a_game_that_does_not_announce_it_is_turned_away() {
    let settings = Settings {
        netcode: Netcode::Ticks {
            grid: sync_core::Grid::STOCK,
        },
        ..Settings::default()
    };
    let mut bench = Bench::growing(2, MAP_BATTERIES, settings);
    bench.clients[1].announce_ticks = true;
    bench.clients[0].connect();
    bench.clients[1].connect();
    bench.run(1_000);
    assert!(bench.clients[0].disconnected);
    assert!(bench.clients[0].model.options.is_none());
    assert!(!bench.clients[1].disconnected);
    assert!(bench.clients[1].model.ticks.is_some());
}

/// The body of a scripted client's datagram: its one message, id first,
/// if it is an update with inputs.
fn update_with_inputs(datagram: &[u8]) -> Option<&[u8]> {
    let body = datagram.get(3..)?;
    if body.first() != Some(&survarium::v026g::match_msgs::id::CLIENT_UPDATE) {
        return None;
    }
    let (inputs, profiles) = (*body.get(6)?, *body.get(7)?);
    if inputs == 0 && profiles == 0 {
        return None;
    }
    body.get(..8 + 9 * usize::from(inputs) + usize::from(profiles))
}

/// Holds every copy of `place`'s first update with inputs sent at or after
/// `after_ms` for `hold_ms` from its first send: they all arrive then.
fn hold_update(place: usize, after_ms: u64, hold_ms: u64) -> Jitter {
    let mut held: Option<(Vec<u8>, u64)> = None;
    Box::new(move |ms, from, up, datagram| {
        if from != place || !up {
            return 0;
        }
        let Some(body) = update_with_inputs(datagram) else {
            return 0;
        };
        match &held {
            Some((target, until)) if target == body => until.saturating_sub(ms),
            Some(_) => 0,
            None if ms >= after_ms => {
                held = Some((body.to_vec(), ms + hold_ms));
                hold_ms
            }
            None => 0,
        }
    })
}

#[test]
fn an_update_past_its_tick_is_moved_to_the_first_open_tick_for_every_game() {
    let mut bench = ticks_match(2);
    let from = bench.clock();
    play(&mut bench, 1_000, 50);
    // Player 1's next update with an input comes 350 ms late: its tick
    // closed at the deadline without it. Its later updates are empty and
    // in time (else they would make it stale).
    bench.jitter = Some(hold_update(1, bench.ms, 350));
    bench.clients[1].input(0, 0xABC);
    for n in 0..30 {
        bench.clients[0].input(0, n);
        bench.run(50);
    }
    bench.jitter = None;
    play(&mut bench, 1_000, 50);
    bench.run(1_000);
    let to = bench.clock() - 600;
    let owner = bench.clients[1].per_tick.as_ref().unwrap();
    assert_eq!(owner.moved, 1, "moved {}, discarded {}", owner.moved, owner.discarded);
    assert_eq!(owner.discarded, 0);
    assert_eq!(owner.answers_past_history, 0);
    assert_eq!(owner.pending(), 0);
    assert_same_inputs(&bench, from, to);
    // The others had it once, at the moved tick.
    let theirs = bench.clients[0].per_tick.as_ref().unwrap();
    assert_eq!(
        theirs.timeline(0, 1, from, to),
        owner.timeline(1, 1, from, to)
    );
    bench.assert_no_faults();
}

#[test]
fn an_update_with_an_input_no_game_makes_is_discarded_and_reaches_nobody() {
    let mut bench = ticks_match(2);
    let from = bench.clock();
    play(&mut bench, 1_000, 50);
    // Action bit 29: no game sets it (`sync_core::wire::UNUSED_ACTIONS`).
    bench.clients[1].input(0, 1 << 29);
    for n in 0..10 {
        bench.clients[0].input(0, n);
        bench.run(50);
    }
    play(&mut bench, 1_000, 50);
    bench.run(1_000);
    let to = bench.clock() - 600;
    let owner = bench.clients[1].per_tick.as_ref().unwrap();
    assert_eq!(owner.discarded, 1);
    assert_eq!(owner.moved, 0);
    assert_eq!(owner.pending(), 0);
    assert_same_inputs(&bench, from, to);
    bench.assert_no_faults();
}

#[test]
fn an_update_too_late_to_move_is_discarded_by_every_game() {
    let mut bench = ticks_match(2);
    let from = bench.clock();
    play(&mut bench, 1_000, 50);
    // 700 ms: past what its own history could follow.
    bench.jitter = Some(hold_update(1, bench.ms, 700));
    play(&mut bench, 1_500, 50);
    bench.jitter = None;
    play(&mut bench, 500, 50);
    bench.run(1_000);
    let to = bench.clock() - 600;
    let owner = bench.clients[1].per_tick.as_ref().unwrap();
    assert_eq!(owner.discarded, 1);
    assert_eq!(owner.moved, 0);
    assert_eq!(owner.pending(), 0);
    assert_same_inputs(&bench, from, to);
    bench.assert_no_faults();
}

#[test]
fn an_update_overtaken_by_later_ones_still_lands_at_its_tick_while_it_is_open() {
    let mut bench = ticks_match(2);
    let from = bench.clock();
    play(&mut bench, 1_000, 50);
    // 120 ms: the next two of its updates come before it; its tick waits.
    bench.jitter = Some(hold_update(1, bench.ms, 120));
    play(&mut bench, 1_000, 50);
    bench.jitter = None;
    bench.run(1_000);
    let to = bench.clock() - 600;
    let owner = bench.clients[1].per_tick.as_ref().unwrap();
    assert_eq!((owner.moved, owner.discarded), (0, 0));
    assert_eq!(owner.pending(), 0);
    assert_same_inputs(&bench, from, to);
    bench.assert_no_faults();
}

#[test]
fn an_update_overtaken_past_the_deadline_is_stale_and_discarded() {
    let mut bench = ticks_match(2);
    let from = bench.clock();
    play(&mut bench, 1_000, 50);
    // 300 ms: later updates with inputs landed at their ticks before it.
    bench.jitter = Some(hold_update(1, bench.ms, 300));
    play(&mut bench, 1_000, 50);
    bench.jitter = None;
    bench.run(1_000);
    let to = bench.clock() - 600;
    let owner = bench.clients[1].per_tick.as_ref().unwrap();
    assert_eq!((owner.moved, owner.discarded), (0, 1));
    assert_same_inputs(&bench, from, to);
    bench.assert_no_faults();
}

#[test]
fn a_stalled_place_holds_nobody() {
    let mut bench = ticks_match(3);
    let from = bench.clock();
    play(&mut bench, 1_000, 50);
    // Player 2's world stands still: it sends no update.
    let stalled = bench.clients[2].clock();
    bench.clients[2].frozen_at = Some(stalled);
    let taken = bench.clients[0].per_tick.as_ref().unwrap().taken_at.len();
    play(&mut bench, 3_000, 50);
    // After the first deadline, the others' ticks close as their own
    // updates come: no deeper than a few ticks.
    let depths: Vec<u32> = bench.clients[0].per_tick.as_ref().unwrap().taken_at[taken + 10..]
        .iter()
        .map(|(tick, clock)| clock.saturating_sub(*tick))
        .collect();
    assert!(depths.len() > 40);
    assert!(
        depths.iter().all(|depth| *depth <= 60),
        "the others waited for the stalled game: {depths:?}"
    );
    // It comes back: its updates are late, moved or dropped, and every
    // game agrees.
    bench.clients[2].frozen_at = None;
    play(&mut bench, 2_000, 50);
    bench.run(1_000);
    let to = bench.clock() - 600;
    assert_same_inputs(&bench, from, to);
    bench.assert_no_faults();
}

#[test]
fn roster_changes_mid_match_reach_every_game_at_one_tick() {
    let mut bench = ticks_match(2);
    play(&mut bench, 1_000, 50);
    // A third player comes, plays, and leaves.
    let place = bench.clients.len();
    let player = MatchPlayer {
        session_id: session_of(place),
        account_id: 100 + place as u32,
        profile: profile("newcomer", 0),
        bot: false,
        driven: false,
    };
    bench
        .registry
        .join_running(bench.match_id, player)
        .expect("room for a third");
    let map_id = bench.clients[0].sim.map_id;
    bench
        .clients
        .push(Client::ticks(session_of(place), place + 1, map_id));
    bench
        .addrs
        .push(SocketAddr::from(([127, 0, 0, 1], 40_000 + place as u16)));
    bench.join(place);
    assert!(bench.until(5_000, |bench| {
        bench.clients.iter().all(|client| client.sim_events().contains(&(2, true)))
    }));
    let from = bench.clock();
    play(&mut bench, 2_000, 50);
    bench.clients[place].leave();
    bench.run(2_000);
    let to = bench.clock() - 600;
    let events: Vec<_> = bench.clients[..2]
        .iter()
        .map(|client| client.sim_event_times(2))
        .collect();
    assert_eq!(events[0], events[1]);
    assert!(
        events[0].iter().any(|(_, entered)| !entered),
        "the leave: {events:?}"
    );
    assert_eq!(
        bench.clients[2].sim_event_times(2)[0],
        events[0][0],
        "its own entry, at the others' tick"
    );
    bench.clients.truncate(2);
    assert_same_inputs(&bench, from, to);
    bench.assert_no_faults();
}

#[test]
fn a_resynced_game_gets_the_updates_since_its_world_and_agrees_again() {
    let mut bench = ticks_match(3);
    play(&mut bench, 4_000, 50);
    let from = bench.clients[2].clock();
    bench.clients[2].sim.salt_from = Some((from, 7));
    assert!(bench.until(10_000, |bench| bench.game().resyncs(2) == 1));
    // It plays on all the while.
    assert!(bench.until(10_000, |bench| {
        bench.clients[2].model.syncs == 2 && bench.stages() == "LLL"
    }));
    let loaded = bench.clients[2].model.last_resync.unwrap();
    play(&mut bench, 3_000, 50);
    bench.run(1_000);
    let to = bench.clock() - 600;
    assert_same_inputs(&bench, loaded, to);
    assert_eq!(bench.game().resyncs(2), 1);
    bench.assert_no_faults();
}

#[test]
fn the_servers_own_bots_play_on_updates() {
    let mut bench = Bench::ticks_vs_bots(3, MAP_BATTERIES, Settings::default(), &[1, 2]);
    bench.join(0);
    let from = bench.clock();
    play(&mut bench, 5_000, 50);
    bench.run(1_000);
    let to = bench.clock() - 600;
    let client = bench.clients[0].per_tick.as_ref().unwrap();
    let bots: usize = (1..3)
        .map(|bot| client.timeline(0, bot, from, to).len())
        .sum();
    assert!(bots > 0, "the bots' inputs reach the player");
    assert_eq!(client.inputs_past_history, 0);
    assert_eq!(client.pending(), 0);
    bench.assert_no_faults();
}
