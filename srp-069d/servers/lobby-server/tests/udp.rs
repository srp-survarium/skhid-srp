//! The socket loop: a scripted client talks to `net::serve` over real UDP on the loopback interface.

mod support;

use std::net::UdpSocket;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use lobby_server::{net, LobbyConfig, MinimalMatches};
use protocol::c2s::Request;
use udp_transport::{Event, Role, Transport};

#[test]
fn sign_in_over_a_real_socket() {
    game_data::require_tables!();
    let dir = tempfile::tempdir().unwrap();
    let store = support::open_store(&dir.path().join("u.sqlite"));
    let id = store.create_account("udp", "pw").unwrap();
    let sessions = Arc::new(lobby_server::SessionTable::default());
    let session = sessions.grant(id);
    let mut server = support::build_server(&store, &sessions, LobbyConfig::default(), Arc::new(MinimalMatches));

    let sock = UdpSocket::bind("127.0.0.1:0").unwrap();
    let addr = sock.local_addr().unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let stop2 = stop.clone();
    let th = std::thread::spawn(move || net::serve(&mut server, &sock, &stop2).unwrap());

    let c = UdpSocket::bind("127.0.0.1:0").unwrap();
    c.set_read_timeout(Some(Duration::from_millis(20))).unwrap();
    let mut t = Transport::new(Role::Client, Instant::now());
    let (mid, pl) = Request::SignIn { session, a: 0xFF, b: 0xFF }.encode().unwrap();
    t.send(mid, &pl).unwrap();
    let mut ids = vec![];
    let mut buf = vec![0u8; 2048];
    let t0 = Instant::now();
    while t0.elapsed() < Duration::from_secs(5) && !ids.contains(&0xB8u8) {
        for d in t.poll(Instant::now()) {
            c.send_to(&d, addr).unwrap();
        }
        if let Ok((n, _)) = c.recv_from(&mut buf) {
            for ev in t.receive(&buf[..n], Instant::now()) {
                if let Event::Message { id, .. } = ev {
                    ids.push(id);
                }
            }
        }
    }
    stop.store(true, Ordering::Relaxed);
    th.join().unwrap();
    assert_eq!(ids.first(), Some(&0x80));
    assert!(ids.contains(&0x87) && ids.contains(&0x8A) && ids.last() == Some(&0xB8), "{ids:02x?}");
}
