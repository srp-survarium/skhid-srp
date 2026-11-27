//! Login -> lobby handover: a golden sign-in (real-client capture, account name test@example.com, test key) goes through
//! `AccountAuth`; the session in the `0B` answer signs a scripted client in at the lobby, which then sees its own account.

use std::net::SocketAddrV4;
use std::sync::Arc;
use std::time::{Duration, Instant};

use accounts::{HashCost, Store};
use lobby_server::{Lobby, LobbyConfig, MinimalMatches, Server, SessionTable};
use login_server::frame::{frame, FrameReader};
use login_server::handshake::parse_plaintext;
use login_server::keys::encrypt_with_public_der;
use login_server::{Handshake, LoginKey, Outcome, Refusal};
use protocol::c2s::Request;
use protocol::s2c::S2c;
use serde_json::Value;
use srp::{AccountAuth, Config};
use udp_transport::{Event, Role, Transport};

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap()).collect()
}

struct Golden {
    sign_in: Vec<u8>,
    plaintext: Vec<u8>,
}

fn golden() -> Golden {
    let path = format!("{}/../../tests/golden/captured_session_2_login.jsonl", env!("CARGO_MANIFEST_DIR"));
    let recs: Vec<Value> = std::fs::read_to_string(path).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    let get = |dir: &str, kind: &str| unhex(recs.iter().find(|r| r["dir"] == dir && r["kind"] == kind).unwrap()["hex"].as_str().unwrap());
    let mut r = FrameReader::default();
    r.push(&get("c2s", "raw"));
    Golden { sign_in: r.next_frame().unwrap().unwrap(), plaintext: get("note", "rsa_plain") }
}

const CHALLENGE: u32 = 0xC0FFEE01;
const LOBBY: SocketAddrV4 = SocketAddrV4::new(std::net::Ipv4Addr::new(127, 0, 0, 1), 1235);

fn login(key: &LoginKey, auth: &AccountAuth, plaintext: &[u8]) -> Outcome {
    let g = golden();
    let mut hs = Handshake::new(key, 2, CHALLENGE);
    hs.on_frame(&g.sign_in, auth).unwrap();
    let block = encrypt_with_public_der(&key.public_key(), plaintext);
    let mut p = vec![0x02];
    p.extend((block.len() as u32).to_le_bytes());
    p.extend(block);
    hs.on_frame(&p, auth).unwrap().outcome
}

fn setup() -> (tempfile::TempDir, Arc<Store>, Arc<SessionTable>) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("srp.sqlite")).unwrap();
    store.set_hash_cost(HashCost::FAST_FOR_TESTS);
    (dir, Arc::new(store), Arc::new(SessionTable::default()))
}

#[test]
fn first_sign_in_registers_the_account_and_the_session_opens_the_lobby() {
    game_data::require_tables!();
    let (_dir, store, sessions) = setup();
    let key = LoginKey::generate_for_tests(1024);
    let auth = AccountAuth::new(store.clone(), sessions.clone(), LOBBY, true);
    let g = golden();
    let Outcome::Granted { account, granted } = login(&key, &auth, &g.plaintext) else { panic!("not granted") };
    assert_eq!(account, "test@example.com");
    assert_eq!(granted.lobby, LOBBY);
    let id = store.find_account("test@example.com").unwrap().unwrap().id;
    assert_eq!(sessions.redeem(granted.session), Some(id));

    // the lobby: 2E with that session; the 87 carries the account name
    let lobby = Lobby::new(LobbyConfig::default(), store.clone(), Arc::new(game_data::testing::tables()), sessions.clone(), Arc::new(MinimalMatches));
    let mut server = Server::new(lobby);
    let addr = "127.0.0.1:41000".parse().unwrap();
    let mut now = Instant::now();
    let mut t = Transport::new(Role::Client, now);
    let (mid, pl) = Request::SignIn { session: granted.session, a: 0xFF, b: 0xFF }.encode().unwrap();
    t.send(mid, &pl).unwrap();
    let mut got = vec![];
    for _ in 0..300 {
        now += Duration::from_millis(30);
        for d in t.poll(now) {
            server.receive(addr, &d, now);
        }
        for (_, d) in server.poll(now) {
            for ev in t.receive(&d, now) {
                if let Event::Message { id, payload, .. } = ev {
                    got.push((id, payload));
                }
            }
        }
    }
    assert_eq!(got.first().map(|m| m.0), Some(0x80));
    let (i, pl) = got.iter().find(|m| m.0 == 0x87).expect("87");
    let _ = i;
    let S2c::Account(a) = S2c::decode(0x87, pl).unwrap() else { panic!() };
    assert_eq!(a.body.name, "test@example.com");

    // the lobby connection ends (a second sign-in of an online account would be refused with 0C 07)
    sessions.mark_offline(id);
    // the same digest again: the same account; another digest: refused
    let Outcome::Granted { granted: g2, .. } = login(&key, &auth, &g.plaintext) else { panic!() };
    assert_eq!(sessions.redeem(g2.session), Some(id));
    assert_ne!(g2.session, granted.session);
    assert_eq!(store.account_count().unwrap(), 1);
    let digest = parse_plaintext(&g.plaintext).unwrap().digest;
    assert_eq!(digest.len(), 40);
    store.change_password(id, &"f".repeat(40)).unwrap();
    assert_eq!(
        login(&key, &auth, &g.plaintext),
        Outcome::Refused { account: "test@example.com".into(), reason: Refusal::BAD_CREDENTIALS }
    );
}

#[test]
fn unknown_accounts_are_refused_when_auto_register_is_off() {
    let (_dir, store, sessions) = setup();
    let key = LoginKey::generate_for_tests(1024);
    let auth = AccountAuth::new(store.clone(), sessions, LOBBY, false);
    assert!(matches!(login(&key, &auth, &golden().plaintext), Outcome::Refused { .. }));
    assert_eq!(store.account_count().unwrap(), 0);
    let _ = frame(&[1]);
}

#[test]
fn default_config_is_the_documented_one() {
    let c = Config::default();
    assert_eq!(c.login_addr.to_string(), "127.0.0.1:65000");
    assert_eq!(c.lobby_bind.to_string(), "127.0.0.1:1235");
    assert_eq!(c.lobby_public, LOBBY);
    assert!(c.auto_register);
    assert_eq!(c.online.bind.to_string(), "127.0.0.1:1236");
    assert_eq!(c.key_path.to_string_lossy(), "login_key.pem");
}
