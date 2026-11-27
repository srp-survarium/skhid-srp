//! Steam-mode sign-in through the Import panel against the real account store: the login server's state machine with
//! srp's `AccountAuth` (lockout, bans, auto-register, version, sessions) and the stored SteamID64.

use std::net::SocketAddrV4;
use std::sync::{Arc, OnceLock};

use accounts::{BanKind, HashCost, Store};
use lobby_server::SessionTable;
use login_server::digest::sha1_hex;
use login_server::{Attempt, Authenticator, Handshake, LoginKey, Outcome, Refusal};
use srp::AccountAuth;

const LOBBY: SocketAddrV4 = SocketAddrV4::new(std::net::Ipv4Addr::new(127, 0, 0, 1), 1235);
const STEAM_ID: u64 = 76561197960265728; // the SteamID64 base: account number 0, no real account

fn key() -> &'static LoginKey {
    static K: OnceLock<LoginKey> = OnceLock::new();
    K.get_or_init(|| LoginKey::generate_for_tests(1024))
}

fn setup(auto: bool) -> (tempfile::TempDir, Arc<Store>, AccountAuth) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("a.sqlite")).unwrap();
    store.set_hash_cost(HashCost::FAST_FOR_TESTS);
    let store = Arc::new(store);
    let auth = AccountAuth::new(store.clone(), Arc::new(SessionTable::default()), LOBBY, auto);
    (dir, store, auth)
}

fn str_field(out: &mut Vec<u8>, s: &[u8]) {
    out.push(s.len() as u8);
    out.extend(s);
}

/// `01 02 <SteamID64> str "0.69d0" str "r1" u32 token u32 key crc u8 FF u32 0B` (the client's method-2 sign-in).
fn steam_sign_in(version: &str) -> Vec<u8> {
    let mut p = vec![0x01, 0x02];
    p.extend(STEAM_ID.to_le_bytes());
    str_field(&mut p, version.as_bytes());
    str_field(&mut p, b"r1");
    p.extend(7u32.to_le_bytes());
    p.extend(0u32.to_le_bytes());
    p.push(0xFF);
    p.extend(0x0Bu32.to_le_bytes());
    p
}

fn import(email: &str, password: &str) -> Vec<u8> {
    let mut p = vec![0x04, 0, 0, 0, 0, 2];
    str_field(&mut p, email.as_bytes());
    str_field(&mut p, password.as_bytes());
    p
}

fn steam_session<'a>(auth: &AccountAuth, version: &str) -> Handshake<'a> {
    let mut hs = Handshake::new(key(), 1, 1);
    let r = hs.on_frame(&steam_sign_in(version), auth).unwrap();
    assert_eq!(r.payloads, vec![vec![0x10, 0x04]]);
    hs
}

fn normal<'a>(account: &'a str, digest: &'a str) -> Attempt<'a> {
    Attempt { account, signin_type: 1, password_type: 1, digest, client_version: "0.69d0", peer: None, steam_id: None }
}

#[test]
fn new_name_registers_through_the_panel_and_signs_in_normally_after() {
    let (_d, store, auth) = setup(true);
    let mut hs = steam_session(&auth, "0.69d0");
    let r = hs.on_frame(&import("newcomer", "hunter2"), &auth).unwrap();
    assert_eq!(r.payloads[0][0], 0x0B);
    assert!(matches!(r.outcome, Outcome::Granted { ref account, .. } if account == "newcomer"));
    let id = store.find_account("newcomer").unwrap().unwrap().id;
    assert_eq!(store.steam_id(id).unwrap().map(|(s, _)| s), Some(STEAM_ID));
    // the stored password is the normal client's digest: the normal sign-in with the same password works
    let digest = sha1_hex(b"hunter2");
    assert!(auth.authenticate(&normal("newcomer", &sha1_hex(b"Hunter2"))).is_err());
    assert!(auth.authenticate(&normal("newcomer", &digest)).is_ok());
}

#[test]
fn an_account_made_by_the_normal_path_signs_in_through_the_panel_and_its_steam_id_is_stored() {
    let (_d, store, auth) = setup(false);
    let id = store.create_account("pilot", &sha1_hex(b"s3cret pass")).unwrap();
    assert!(auth.authenticate(&normal("pilot", &sha1_hex(b"s3cret pass"))).is_ok());
    assert_eq!(store.steam_id(id).unwrap(), None, "the normal path stores no SteamID");
    let mut hs = steam_session(&auth, "0.69d0");
    // wrong password, then a wrong name: the panel again, nothing stored
    assert_eq!(hs.on_frame(&import("pilot", "s3cret"), &auth).unwrap().payloads, vec![vec![0x10, 0x04]]);
    assert_eq!(hs.on_frame(&import("nobody", "s3cret pass"), &auth).unwrap().payloads, vec![vec![0x10, 0x04]]);
    assert_eq!(store.steam_id(id).unwrap(), None);
    let r = hs.on_frame(&import("PILOT", "s3cret pass"), &auth).unwrap();
    assert_eq!(r.payloads[0][0], 0x0B);
    assert_eq!(store.steam_id(id).unwrap().map(|(s, _)| s), Some(STEAM_ID));
}

#[test]
fn ban_lockout_version_and_already_online_keep_their_0c_on_the_panel() {
    let (_d, store, auth) = setup(false);
    let id = store.create_account("pilot", &sha1_hex(b"pw")).unwrap();
    // banned: 0C 02 reason minutes, the connection goes on
    store.set_ban(id, BanKind::Banned, 13, 0).unwrap();
    let mut hs = steam_session(&auth, "0.69d0");
    let r = hs.on_frame(&import("pilot", "pw"), &auth).unwrap();
    assert_eq!(&r.payloads[0][..3], &[0x0C, 2, 13]);
    assert_eq!(r.outcome, Outcome::Continue);
    assert_eq!(store.steam_id(id).unwrap(), None, "no SteamID stored for a refused sign-in");
    store.clear_ban(id).unwrap();
    // unbanned on the same connection: granted
    assert_eq!(hs.on_frame(&import("pilot", "pw"), &auth).unwrap().payloads[0][0], 0x0B);
    // already online: 0C 07
    auth.sessions.mark_online(id);
    let mut hs = steam_session(&auth, "0.69d0");
    assert_eq!(hs.on_frame(&import("pilot", "pw"), &auth).unwrap().payloads, vec![Refusal::ALREADY_ONLINE.payload()]);
    auth.sessions.mark_offline(id);
    // five wrong passwords from this address: 0C 05 even for the right one
    let mut hs = steam_session(&auth, "0.69d0");
    for _ in 0..5 {
        assert_eq!(hs.on_frame(&import("pilot", "bad"), &auth).unwrap().payloads, vec![vec![0x10, 0x04]]);
    }
    assert_eq!(hs.on_frame(&import("pilot", "pw"), &auth).unwrap().payloads, vec![vec![0x0C, 5]]);
}

#[test]
fn the_version_check_uses_the_version_of_the_steam_sign_in() {
    let (_d, store, mut auth) = setup(false);
    store.create_account("pilot", &sha1_hex(b"pw")).unwrap();
    auth.required_version = Some("0.69d0".into());
    let mut hs = steam_session(&auth, "0.69c9");
    assert_eq!(hs.on_frame(&import("pilot", "pw"), &auth).unwrap().payloads, vec![vec![0x0C, 8]]);
    let mut hs = steam_session(&auth, "0.69d0");
    assert_eq!(hs.on_frame(&import("pilot", "pw"), &auth).unwrap().payloads[0][0], 0x0B);
}
