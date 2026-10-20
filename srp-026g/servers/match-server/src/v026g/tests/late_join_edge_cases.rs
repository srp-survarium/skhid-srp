//! Independent lifecycle checks for the experimental growing roster.

use super::*;
use survarium::v026g::match_msgs::{ServerMessage, rules::state};

fn running() -> Bench {
    let mut bench = Bench::growing(2, MAP_BATTERIES, Settings::default());
    bench.join_all();
    bench.run_long(25_000);
    assert_eq!(bench.game().rule_state(), state::IN_PROCESS);
    assert!(bench.game().admitting());
    bench
}

fn reserve(bench: &mut Bench) -> usize {
    let place = bench.clients.len();
    bench
        .registry
        .join_running(
            bench.match_id,
            MatchPlayer {
                session_id: session_of(place),
                account_id: 100 + place as u32,
                profile: profile(&format!("edge-player-{place}"), (place % 2) as u8),
                bot: false,
                driven: false,
            },
        )
        .unwrap();
    bench
        .clients
        .push(Client::growing(session_of(place), place + 1, MAP_BATTERIES));
    bench
        .addrs
        .push(SocketAddr::from(([127, 0, 0, 1], 40_000 + place as u16)));
    place
}

fn load(bench: &mut Bench, place: usize) {
    bench.clients[place].connect();
    assert!(bench.until(1_000, |b| b.clients[place].model.options.is_some()));
    bench.clients[place].ready();
}

fn assert_existing_undisturbed(bench: &Bench) {
    assert_eq!(&bench.stages()[..2], "LL");
    for place in 0..2 {
        assert_eq!(bench.clients[place].model.syncs, 1);
        assert_eq!(bench.game().resyncs(place), 0);
    }
    bench.assert_no_faults();
}

#[test]
fn a_reserved_player_that_never_connects_expires_without_entering() {
    let mut bench = running();
    let place = reserve(&mut bench);
    bench.run_long(61_000);
    assert!(bench.registry.seat(session_of(place)).is_none());
    assert!(!bench.clients[0].entered().contains(&(place as u8)));
    // It failed by itself: its place is installed on, and the match takes
    // joiners again (until 2026-10-19 it took nobody after one failed).
    assert!(bench.game().admitting());
    assert_existing_undisturbed(&bench);
}

#[test]
fn voluntarily_leaving_before_entry_never_emits_a_phantom_departure() {
    for catching_up in [false, true] {
        let mut bench = running();
        let place = reserve(&mut bench);
        if catching_up {
            bench.clients[place].no_hashes = true;
            bench.join(place);
        } else {
            bench.clients[1].auto_prepare = false;
            load(&mut bench, place);
            bench.run(500);
        }
        bench.clients[place].leave();
        bench.run_long(61_000);
        assert!(bench.registry.seat(session_of(place)).is_none());
        for client in &bench.clients[..2] {
            assert!(!client.entered().contains(&(place as u8)));
            assert!(!client.left().contains(&(place as u8)));
        }
        assert_existing_undisturbed(&bench);
    }
}

#[test]
fn a_newcomer_can_reconnect_while_existing_peers_are_preparing() {
    let mut bench = running();
    bench.clients[1].auto_prepare = false;
    let place = reserve(&mut bench);
    load(&mut bench, place);
    assert!(bench.until(1_000, |b| !b.clients[1].prepares.is_empty()));
    bench.clients[place].drop_link();
    bench.run(100);
    assert!(bench.registry.seat(session_of(place)).is_some());
    bench.clients[place] = Client::growing(session_of(place), place + 1, MAP_BATTERIES);
    bench.addrs[place].set_port(43_000 + place as u16);
    load(&mut bench, place);
    bench.run(500);
    assert!(bench.clients[place].snapshots().is_empty());
    bench.clients[1].auto_prepare = true;
    assert!(bench.until(10_000, |b| {
        b.clients
            .iter()
            .all(|c| c.entered().contains(&(place as u8)))
    }));
    bench.run(2_000);
    for client in &bench.clients {
        assert_eq!(
            client
                .taken
                .iter()
                .filter(|m| matches!(m,
                    ServerMessage::PlayerEntered { player, .. } if *player == place as u8
                ))
                .count(),
            1
        );
    }
    assert_existing_undisturbed(&bench);
}

#[test]
fn existing_peer_timeout_releases_the_resource_barrier() {
    let mut bench = running();
    bench.clients[1].auto_prepare = false;
    let place = reserve(&mut bench);
    load(&mut bench, place);
    assert!(bench.until(1_000, |b| !b.clients[1].prepares.is_empty()));
    bench.clients[1].silent = true;
    assert!(bench.until(40_000, |b| b.clients[0].entered().contains(&(place as u8))));
    assert!(bench.clients[0].left().contains(&1));
    assert_eq!(bench.clients[0].model.syncs, 1);
    assert_eq!(bench.game().resyncs(0), 0);
    bench.assert_no_faults();
}

#[test]
fn a_newcomer_still_catching_up_is_not_present_in_match_results() {
    let mut bench = running();
    bench.run_long(110_000);
    let place = reserve(&mut bench);
    bench.clients[place].no_hashes = true;
    bench.join(place);
    assert!(!bench.clients[0].entered().contains(&(place as u8)));
    let at = bench.clock() / 50 * 50 + 100;
    for client in &mut bench.clients {
        client.sim.points_from = Some((at, [10, 3]));
    }
    assert!(bench.until(12_000, |b| b.clients[..2].iter().all(|c| c.closed)));
    let result = bench.results.try_recv().unwrap();
    assert!(!result.players[place].entered);
    assert_eq!(result.players[place].time_in_match_ms, 0);
    assert!(
        !result.players[place].present_at_end,
        "a snapshot recipient without verified catch-up never entered the simulation"
    );
    bench.assert_no_faults();
}

#[test]
fn losing_all_existing_players_does_not_make_an_unverified_newcomer_a_witness() {
    let mut bench = running();
    let place = reserve(&mut bench);
    bench.clients[place].no_hashes = true;
    bench.join(place);
    bench.clients[place].sim.salt_from = Some((0, 9));
    for client in &mut bench.clients[..2] {
        client.leave();
    }
    bench.run(500);
    bench.clients[place].no_hashes = false;
    assert!(bench.until(130_000, |b| b.registry.match_count() == 0));
    assert!(!bench.clients[place].entered().contains(&(place as u8)));
    let result = bench.results.try_recv().unwrap();
    assert_eq!(result.end, crate::v026g::MatchEnd::Deserted);
    assert!(!result.players[place].entered);
    assert!(!result.players[place].present_at_end);
    bench.assert_no_faults();
}

#[test]
fn a_growing_roster_prepares_and_catches_up_over_real_udp() {
    let _guard = one_at_a_time();
    let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
    let server = socket.local_addr().unwrap();
    let registry = Arc::new(MatchRegistry::new("127.0.0.1", server.port()));
    let ticket = registry.create_growing(setup(2, MAP_BATTERIES)).unwrap();
    let (sink, results) = channel();
    let stop = Arc::new(AtomicBool::new(false));
    let thread = {
        let registry = registry.clone();
        let stop = stop.clone();
        std::thread::spawn(move || {
            serve(
                &socket,
                server.port(),
                registry,
                store(),
                Box::new(sink),
                quick(),
                &stop,
            )
        })
    };
    let client = |place, players| {
        let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
        socket.set_nonblocking(true).unwrap();
        (
            Client::growing(session_of(place), players, MAP_BATTERIES),
            socket,
        )
    };
    let mut net = Net {
        stop,
        thread: Some(thread),
        server,
        registry,
        results,
        clients: vec![client(0, 2), client(1, 2)],
        match_id: ticket.match_id,
        start: Instant::now(),
    };
    let soon = Duration::from_secs(8);
    net.join(0);
    net.join(1);
    // Exercise the actual countdown and admission observation in the server loop.
    net.run(Duration::from_secs(22));
    net.registry
        .join_running(
            net.match_id,
            MatchPlayer {
                session_id: session_of(2),
                account_id: 102,
                profile: profile("udp-newcomer", 0),
                bot: false,
                driven: false,
            },
        )
        .unwrap();
    net.clients.push(client(2, 3));
    net.client(1).auto_prepare = false;
    net.client(2).connect();
    assert!(net.until(soon, |n| n.clients[2].0.model.options.is_some()));
    net.client(2).ready();
    assert!(net.until(soon, |n| !n.clients[1].0.prepares.is_empty()));
    net.run(Duration::from_millis(1_200));
    assert!(net.clients[2].0.snapshots().is_empty());
    let input = net.client(0).input(100, 0x21);
    assert!(net.until(soon, |n| n.clients[1].0.relays_of(0).contains(&input)));
    net.client(1).auto_prepare = true;
    assert!(net.until(soon, |n| {
        n.clients.iter().all(|(c, _)| c.entered().contains(&2))
    }));
    let input = net.client(2).input(100, 0x40);
    assert!(net.until(soon, |n| {
        n.clients[..2]
            .iter()
            .all(|(c, _)| c.relays_of(2).contains(&input))
    }));
    for (client, _) in &net.clients[..2] {
        assert_eq!(client.model.syncs, 1);
    }
    net.assert_no_faults();
}
