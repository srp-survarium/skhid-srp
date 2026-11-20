//! A client is its address and the route it came by (`udp_transport::relay`):
//! datagrams that name a client's address by another route are another
//! peer's, and every client is answered by its own route.

use std::net::SocketAddr;

use udp_transport::relay::{Peer, Relays};

use super::{Bench, MAP_BATTERIES};
use crate::v026g::Settings;

/// `peer`'s address as the tests' relay names it.
fn through_relay(peer: Peer) -> Peer {
    let SocketAddr::V4(address) = peer.address() else {
        unreachable!("the bench's clients are on 127.0.0.1")
    };
    let relay = Relays::parse("10.77.0.2=203.0.113.8")
        .unwrap()
        .relay("10.77.0.2".parse().unwrap())
        .unwrap();
    Peer::relayed(address, relay)
}

/// Both players' inputs still go both ways.
fn inputs_flow(bench: &mut Bench) {
    let first = bench.clients[0].input(50, 0x8000_0021);
    bench.run(100);
    assert_eq!(bench.clients[1].relays_of(0), [first]);
    let second = bench.clients[1].input(50, 0x40);
    bench.run(100);
    assert_eq!(bench.clients[0].relays_of(1), [second]);
}

#[test]
fn datagrams_naming_a_players_address_by_another_route_leave_the_player_alone() {
    let mut bench = Bench::new(2, MAP_BATTERIES, Settings::default());
    bench.join_all();
    bench.run(2_000);
    let claimed = through_relay(bench.addrs[0]);
    assert_ne!(claimed, bench.addrs[0]);
    for datagram in [&[][..], &[0, 0, 0, 0][..], &[0xE3, 0xD4, 4, 0xD4, 4, 1, 2, 3][..]] {
        for _ in 0..50 {
            bench.server.receive(claimed, datagram, bench.now());
        }
    }
    bench.run(500);
    assert_eq!(bench.stages().as_bytes()[0], b'L', "{}", bench.stages());
    assert_eq!(bench.game().peer_index(bench.addrs[0]), Some(0));
    assert_eq!(bench.game().peer_index(claimed), None);
    inputs_flow(&mut bench);
    bench.assert_no_faults();
}

#[test]
fn a_relayed_player_plays_and_is_answered_through_the_relay() {
    let mut bench = Bench::new(2, MAP_BATTERIES, Settings::default());
    bench.addrs[1] = through_relay(bench.addrs[1]);
    bench.join_all();
    bench.run(2_000);
    assert_eq!(bench.game().peer_index(bench.addrs[1]), Some(1));
    // Nothing for it went to its address directly.
    let direct = Peer::direct(bench.addrs[1].address());
    assert_eq!(bench.game().peer_index(direct), None);
    assert!(bench.server.poll(bench.now()).iter().all(|(to, _)| *to != direct));
    inputs_flow(&mut bench);
    bench.assert_no_faults();
}

#[test]
fn two_players_of_one_address_one_direct_one_relayed_both_play() {
    let mut bench = Bench::new(2, MAP_BATTERIES, Settings::default());
    // A port-preserving NAT: the second player's address is the first's.
    bench.addrs[1] = through_relay(bench.addrs[0]);
    assert_eq!(bench.addrs[0].address(), bench.addrs[1].address());
    bench.join_all();
    bench.run(2_000);
    assert_eq!(bench.game().peer_index(bench.addrs[0]), Some(0));
    assert_eq!(bench.game().peer_index(bench.addrs[1]), Some(1));
    inputs_flow(&mut bench);
    bench.assert_no_faults();
}
