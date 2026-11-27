//! Smoke test over real loopback UDP sockets: `MatchServer` (driver thread) + two scripted clients in real time.

mod common;

use common::*;
use match_server::{HubConfig, MatchServer, MatchSpec, PlayerSpec, WorldStore};
use std::net::{SocketAddr, UdpSocket};
use std::sync::mpsc;
use std::time::{Duration, Instant};

#[test]
fn two_clients_over_loopback_udp() {
    let mut worlds = WorldStore::new();
    worlds.insert(test_world());
    let (tx, rx) = mpsc::channel();
    let server = MatchServer::start(
        "127.0.0.1:0".parse().unwrap(),
        HubConfig::default(),
        worlds,
        move |e| {
            let _ = tx.send(e);
        },
    )
    .unwrap();
    let ticket = server
        .create_match(&MatchSpec {
            end_rule: None,
            pve_end: None,
            ai_ignore_players: false,
            roster: None,
            project: None,
            match_id: Some(SEED),
            map: MAP,
            mode: MODE,
            options: test_options(),
            players: (0..2u64)
                .map(|i| PlayerSpec { account_id: i, name: format!("p{i}"), team: i as u8, session: None, bot: false })
                .collect(),
            level_checksum: None,
        })
        .unwrap();
    let dest: SocketAddr = format!("127.0.0.1:{}", ticket.port).parse().unwrap();
    assert_eq!(dest, server.local_addr().to_string().parse::<SocketAddr>().unwrap());

    let t0 = Instant::now();
    let mut clients: Vec<(UdpSocket, Client)> = ticket
        .players
        .iter()
        .map(|p| {
            let s = UdpSocket::bind("127.0.0.1:0").unwrap();
            s.set_nonblocking(true).unwrap();
            let mut c = Client::new(u64::from(p.slot), p.order_id, 1, Instant::now());
            c.connect();
            (s, c)
        })
        .collect();
    let mut buf = [0u8; 2048];
    let mut pump = |clients: &mut Vec<(UdpSocket, Client)>| {
        let now = Instant::now();
        for (s, c) in clients.iter_mut() {
            for d in c.poll(now) {
                let _ = s.send_to(&d, dest);
            }
            while let Ok((n, _)) = s.recv_from(&mut buf) {
                c.receive(&buf[..n], Instant::now());
            }
        }
        std::thread::sleep(Duration::from_millis(2));
    };
    while t0.elapsed() < Duration::from_millis(2500) {
        pump(&mut clients);
    }
    let (a, b) = (&clients[0].1, &clients[1].1);
    assert!(a.de.is_some() && b.de.is_some());
    assert_eq!(a.dd.as_ref().unwrap().level_checksum, CHECKSUM);
    assert!(a.entered && b.entered);
    // real timing: the other's inputs arrive in order, nothing out of sequence
    for (viewer, sender, p) in [(a, b, 1u8), (b, a, 0u8)] {
        let seen = viewer.seen(p);
        assert!(seen.len() >= 15, "only {} inputs of player {p}", seen.len());
        assert_eq!(&seen[..], &sender.sent[..seen.len()]);
    }
    for w in a.dfs.windows(2) {
        assert_eq!(w[1].time, w[0].time + TICK);
    }
    let st = server.stats(SEED).unwrap();
    assert_eq!((st.phase, st.entered), ("running", 2));

    // leave: the callback fires with AllLeft
    clients[0].1.leave();
    clients[1].1.leave();
    let t1 = Instant::now();
    while t1.elapsed() < Duration::from_millis(400) {
        pump(&mut clients);
    }
    let end = rx.recv_timeout(Duration::from_secs(5)).expect("match end callback");
    assert_eq!(end.match_id, SEED);
    assert_eq!(end.reason, match_server::EndReason::AllLeft);
    server.stop();
}
