//! The generic one-byte requests 34, 39 and 3A (docs/notes/requests-34-39-3a.md).

mod support;

use protocol::c2s::Request;
use protocol::s2c::*;
use support::*;

fn env() -> (tempfile::TempDir, Env) {
    let dir = tempfile::tempdir().unwrap();
    let e = Env::new(&dir.path().join("srp.sqlite"));
    (dir, e)
}

#[test]
fn ask_new_contract_info_repeats_the_94_state() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    let login = e.sign_in(&mut c);
    let at_login = login.iter().find_map(|m| if let S2c::Notices(n) = m { Some(n.clone()) } else { None }).expect("94 at login");
    c.send(Request::Generic { id: 0x39, a: 0 });
    e.pump(&mut [&mut c]);
    let got = c.take();
    let again: Vec<_> = got.iter().filter_map(|m| if let S2c::Notices(n) = m { Some(n.clone()) } else { None }).collect();
    assert_eq!(again, vec![at_login], "exactly one 94, the same state");
    assert_eq!(got.len(), 1);
    assert!(again[0].tail.is_none(), "no Winner's Contract bought: no type 0x16 record (tests/store_offers.rs has one)");
}

#[test]
fn faction_mask_request_answers_with_the_96_mask() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    e.sign_in(&mut c);
    c.send(Request::Generic { id: 0x3A, a: 0 });
    e.pump(&mut [&mut c]);
    let got = c.take();
    assert_eq!(got.len(), 1);
    assert!(matches!(got[0], S2c::FactionMask(FactionMask { mask: 0x0F })), "{:?}", got[0]);
}

#[test]
fn important_message_request_expects_no_answer() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    e.sign_in(&mut c);
    c.send(Request::Generic { id: 0x34, a: 0 });
    e.pump(&mut [&mut c]);
    assert!(c.take().is_empty());
}
