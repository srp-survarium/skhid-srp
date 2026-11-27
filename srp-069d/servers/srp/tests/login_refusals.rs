//! Login refusal codes (`0C`): wrong password, unknown account, attempt lockout, bans / suspensions set by the admin
//! command, already online, invalid version.

use std::net::SocketAddrV4;
use std::sync::Arc;
use std::time::{Duration, Instant};

use accounts::{HashCost, Store};
use login_server::{Attempt, Refusal};
use lobby_server::SessionTable;
use srp::{admin, AccountAuth, AlreadyOnline};

const LOBBY: SocketAddrV4 = SocketAddrV4::new(std::net::Ipv4Addr::new(127, 0, 0, 1), 1235);
const DIGEST: &str = "0123456789abcdef0123456789abcdef01234567";

fn setup(auto: bool) -> (tempfile::TempDir, Arc<Store>, Arc<SessionTable>, AccountAuth) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("a.sqlite")).unwrap();
    store.set_hash_cost(HashCost::FAST_FOR_TESTS);
    let store = Arc::new(store);
    let sessions = Arc::new(SessionTable::default());
    let auth = AccountAuth::new(store.clone(), sessions.clone(), LOBBY, auto);
    (dir, store, sessions, auth)
}

fn attempt<'a>(account: &'a str, digest: &'a str, version: &'a str) -> Attempt<'a> {
    Attempt { account, signin_type: 0, password_type: 2, digest, client_version: version, peer: None, steam_id: None }
}

#[test]
fn unknown_account_without_auto_register_is_code_4() {
    let (_d, _s, _ss, auth) = setup(false);
    let now = Instant::now();
    assert_eq!(auth.decide(&attempt("nobody", DIGEST, "0.69d0"), now, 0).unwrap_err(), Refusal::BAD_CREDENTIALS);
    assert_eq!(Refusal::BAD_CREDENTIALS.code, 4);
}

#[test]
fn five_failures_lock_the_name_for_the_window_and_success_resets() {
    let (_d, store, _ss, auth) = setup(true);
    store.create_account("pilot", DIGEST).unwrap();
    let t0 = Instant::now();
    let bad = "f".repeat(40);
    for i in 0..5 {
        let r = auth.decide(&attempt("pilot", &bad, "v"), t0 + Duration::from_secs(i), 0).unwrap_err();
        assert_eq!(r, Refusal::BAD_CREDENTIALS);
    }
    // locked: even the right password gets code 5
    let t = t0 + Duration::from_secs(10);
    assert_eq!(auth.decide(&attempt("pilot", DIGEST, "v"), t, 0).unwrap_err(), Refusal::ATTEMPT_INTERVAL);
    assert_eq!(auth.decide(&attempt("PILOT", DIGEST, "v"), t, 0).unwrap_err().code, 5);
    // another name is not affected
    assert!(auth.decide(&attempt("other", DIGEST, "v"), t, 0).is_ok());
    // the window ends 60 s after the last failure
    let t = t0 + Duration::from_secs(4 + 61);
    assert!(auth.decide(&attempt("pilot", DIGEST, "v"), t, 0).is_ok());
    // four failures and then a success: the count starts again
    for i in 0..4 {
        auth.decide(&attempt("pilot", &bad, "v"), t + Duration::from_secs(i), 0).unwrap_err();
    }
    assert!(auth.decide(&attempt("pilot", DIGEST, "v"), t + Duration::from_secs(5), 0).is_ok());
    for i in 0..4 {
        auth.decide(&attempt("pilot", &bad, "v"), t + Duration::from_secs(6 + i), 0).unwrap_err();
    }
    assert_eq!(auth.decide(&attempt("pilot", &bad, "v"), t + Duration::from_secs(11), 0).unwrap_err().code, 4);
}

#[test]
fn bans_and_suspensions_carry_reason_and_minutes_and_expire() {
    let (_d, store, _ss, auth) = setup(true);
    let id = store.create_account("cheater", DIGEST).unwrap();
    let t = Instant::now();
    let unix = 1_000_000;
    assert!(auth.decide(&attempt("cheater", DIGEST, "v"), t, unix).is_ok());
    let line = admin(&["ban".into(), "cheater".into(), "ban".into(), "13".into(), "90".into()], &store).unwrap();
    assert!(line.contains("cheater"));
    // set_ban uses the wall clock: ask at the real time
    let now_unix = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
    let r = auth.decide(&attempt("cheater", DIGEST, "v"), t, now_unix).unwrap_err();
    assert_eq!((r.code, r.reason), (2, 13));
    assert!((89..=90).contains(&r.value), "minutes {}", r.value);
    assert_eq!(r.payload()[..3], [0x0C, 2, 13]);
    // 91 minutes later it has run out
    assert!(auth.decide(&attempt("cheater", DIGEST, "v"), t, now_unix + 91 * 60).is_ok());
    // a suspension is code 3; a permanent one never runs out
    admin(&["ban".into(), "cheater".into(), "suspend".into(), "5".into(), "0".into()], &store).unwrap();
    let r = auth.decide(&attempt("cheater", DIGEST, "v"), t, now_unix + 1_000_000_000).unwrap_err();
    // the largest value the client shows correctly ((v + 1) * 60 in 32 bits; u32::MAX wrapped to "now")
    assert_eq!((r.code, r.reason, r.value), (3, 5, accounts::MAX_BAN_MINUTES));
    admin(&["unban".into(), "cheater".into()], &store).unwrap();
    assert!(auth.decide(&attempt("cheater", DIGEST, "v"), t, now_unix).is_ok());
    assert!(store.ban_of(id).unwrap().is_none());
    // bad admin input
    assert!(admin(&["ban".into(), "cheater".into(), "ban".into(), "16".into(), "1".into()], &store).is_err());
    assert!(admin(&["ban".into(), "ghost".into(), "ban".into(), "1".into(), "1".into()], &store).is_err());
}

#[test]
fn already_online_is_refused_or_kicks() {
    let (_d, store, sessions, mut auth) = setup(true);
    let id = store.create_account("twin", DIGEST).unwrap();
    let t = Instant::now();
    assert!(auth.decide(&attempt("twin", DIGEST, "v"), t, 0).is_ok());
    sessions.mark_online(id);
    assert_eq!(auth.decide(&attempt("twin", DIGEST, "v"), t, 0).unwrap_err(), Refusal::ALREADY_ONLINE);
    auth.already_online = AlreadyOnline::Kick;
    assert!(auth.decide(&attempt("twin", DIGEST, "v"), t, 0).is_ok());
    auth.already_online = AlreadyOnline::Refuse;
    sessions.mark_offline(id);
    assert!(auth.decide(&attempt("twin", DIGEST, "v"), t, 0).is_ok());
}

#[test]
fn wrong_client_version_is_code_8() {
    let (_d, _s, _ss, mut auth) = setup(true);
    auth.required_version = Some("0.69d0".into());
    let t = Instant::now();
    assert_eq!(auth.decide(&attempt("a", DIGEST, "0.69c9"), t, 0).unwrap_err(), Refusal::INVALID_VERSION);
    assert!(auth.decide(&attempt("a", DIGEST, "0.69d0"), t, 0).is_ok());
}

#[test]
fn start_money_switch_gives_new_accounts_test_money_and_default_gives_none() {
    game_data::require_tables!();
    let gd = Arc::new(game_data::testing::tables());
    let (_d, store, _s, mut auth) = setup(true);
    auth.data = Some(gd.clone());
    let real = auth.register("plain", DIGEST).unwrap();
    let p = store.load_profile(real).unwrap();
    assert_eq!((p.silver, p.gold, p.parts), (0, 0, 0), "the real 0.69d start has no money");
    assert!(!p.items.is_empty(), "the data's new-player kit");
    // the kit's five modules are stored green (rarity pair (312, 1)), never grey
    let modules: Vec<_> = p.items.iter().filter(|i| gd.items.is_module(i.dict_id)).collect();
    assert_eq!(modules.len(), 5);
    assert!(modules.iter().all(|i| p.mods.get(&i.uid).is_some_and(|m| m[0] == (312, 1))), "{:?}", p.mods);

    auth.start_money = Some(srp::StartMoney::parse("1000000, 10000,1000").unwrap());
    let test = auth.register("rich", DIGEST).unwrap();
    let q = store.load_profile(test).unwrap();
    assert_eq!((q.silver, q.gold, q.parts), (1_000_000, 10_000, 1_000));
    assert_eq!(q.items.len(), p.items.len(), "same kit, only the money differs");

    assert!(srp::StartMoney::parse("1,2").is_err());
    assert!(srp::StartMoney::parse("a,2,3").is_err());
}

#[test]
fn the_lockout_is_per_name_and_address_so_a_stranger_cannot_lock_a_player_out() {
    use std::net::{IpAddr, Ipv4Addr};
    let (_d, store, _ss, auth) = setup(true);
    store.create_account("pilot", DIGEST).unwrap();
    let stranger = Some(IpAddr::V4(Ipv4Addr::new(192, 0, 2, 7)));
    let owner = Some(IpAddr::V4(Ipv4Addr::new(192, 0, 2, 8)));
    let from = |digest, peer| Attempt { account: "pilot", signin_type: 0, password_type: 2, digest, client_version: "v", peer, steam_id: None };
    let t0 = Instant::now();
    let bad = "f".repeat(40);
    for i in 0..5 {
        assert_eq!(auth.decide(&from(&bad, stranger), t0 + Duration::from_secs(i), 0).unwrap_err(), Refusal::BAD_CREDENTIALS);
    }
    let t = t0 + Duration::from_secs(10);
    // the stranger's address is locked, even with the right password
    assert_eq!(auth.decide(&from(DIGEST, stranger), t, 0).unwrap_err(), Refusal::ATTEMPT_INTERVAL);
    // the player signs in from their own address
    assert!(auth.decide(&from(DIGEST, owner), t, 0).is_ok());
    // ... and that success does not clear the stranger's lock
    assert_eq!(auth.decide(&from(DIGEST, stranger), t + Duration::from_secs(1), 0).unwrap_err(), Refusal::ATTEMPT_INTERVAL);
    assert!(auth.locked_out("PILOT", stranger, t + Duration::from_secs(2)));
    assert!(!auth.locked_out("pilot", owner, t + Duration::from_secs(2)));
}
