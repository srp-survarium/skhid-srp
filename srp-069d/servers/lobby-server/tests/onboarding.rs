//! New-player onboarding (docs/notes/onboarding.md): the 9D tutorial gate from stored flags, the free first nickname
//! of the nickname screen (48 04 validation, 48 05 without a voucher), the faction step (4A 00) and the tutorial
//! (4A 02 skip). Login names stay private: only nicknames are compared and searched.
mod support;

use accounts::Profile;
use game_data::GameData;
use protocol::c2s::{Account48 as Q48, Quest4A, Request};
use protocol::s2c::*;
use support::*;

fn env() -> (tempfile::TempDir, Env) {
    let dir = tempfile::tempdir().unwrap();
    let e = Env::new(&dir.path().join("srp.sqlite"));
    (dir, e)
}

/// A brand-new account (the data's new-player kit, onboarding flags 0) and its client.
fn newcomer(e: &mut Env, login: &str) -> Client {
    let gd = game_data::testing::tables();
    e.store.create_account_with_profile(login, "pw", &lobby_server::progress::new_player(&gd, 1, &mut game_data::SplitMix(1))).unwrap();
    e.client_of(login)
}

fn gate(m: &[S2c]) -> Vec<TutorialGate> {
    m.iter().filter_map(|x| if let S2c::TutorialGate(g) = x { Some(g.clone()) } else { None }).collect()
}

fn nickname_reply(m: &[S2c]) -> Option<Account48> {
    m.iter().find_map(|x| match x {
        S2c::Reply(Reply::Account(a @ (Account48::Nickname { .. } | Account48::NicknameChanged { .. }))) => Some(a.clone()),
        _ => None,
    })
}

#[test]
fn a_new_player_gets_the_nickname_screen_then_faction_then_the_full_lobby() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = newcomer(&mut e, "secret-login");
    // sign-in: F = 0 (nickname screen first), no faction yet
    let m = e.sign_in(&mut c);
    assert_eq!(gate(&m), vec![TutorialGate { a: 0, flags: 0, g: 0, faction: 0, h: 0 }]);
    // handshake: A = 1 and bit 10 -> the client picks the nickname step (bit 7 clear)
    let m = e.handshake(&mut c);
    assert_eq!(gate(&m), vec![TutorialGate { a: 1, flags: 0x400, g: 0, faction: 0, h: 0 }]);
    assert!(!e.store.has_nickname(c.account).unwrap(), "no nickname before the screen");
    assert!(e.store.search_accounts("secret", 10, None).unwrap().is_empty(), "the login name is never searchable");

    // validation: free -> valid
    c.send(Request::Account(Q48::NicknameCheck { name: "Stalker".into(), x: 3 }));
    e.pump(&mut [&mut c]);
    assert_eq!(nickname_reply(&c.take()), Some(Account48::Nickname { name: "Stalker".into(), valid: 1, reason: 0, suggestions: vec![] }));

    // the first nickname is free: 48 05 with the client's "no voucher" uid, face 6
    c.send(Request::Account(Q48::NicknameChange { name: "Stalker".into(), a: 6, b: 0xFFFF_FFFF }));
    e.pump(&mut [&mut c]);
    let m = c.take();
    assert_eq!(nickname_reply(&m), Some(Account48::NicknameChanged { ok: 1, uid: 0, face: 6 }), "uid 0: no item is decremented");
    assert!(m.contains(&S2c::Name(NameMsg { flag: 0, name: "Stalker".into() })));
    let p = e.store.load_profile(c.account).unwrap();
    assert_eq!((p.onboarding, p.face), (Profile::NICKNAME_CHOSEN, 6));
    assert_eq!(e.store.find_account("secret-login").unwrap().unwrap().name, "Stalker", "sign-in keeps the login name");
    assert_eq!(e.store.search_accounts("Stalk", 10, None).unwrap().len(), 1, "the nickname is searchable");

    // a second free choice is not possible: without a voucher it is refused
    c.send(Request::Account(Q48::NicknameChange { name: "Other".into(), a: 1, b: 0xFFFF_FFFF }));
    e.pump(&mut [&mut c]);
    assert_eq!(nickname_reply(&c.take()), Some(Account48::NicknameChanged { ok: 0, uid: 0, face: 0 }));

    // the gold option (uid 0) costs the rename voucher's price, the same number 8D costs[3] shows
    let gd = game_data::testing::tables();
    let price = lobby_server::rename::gold_price(&gd);
    assert_eq!(price, 250);
    assert_eq!(lobby_server::build::service_prices_8d(&gd).costs[3], price);
    e.store.update_profile(c.account, |p| Ok::<_, ()>(p.gold = price - 1)).unwrap();
    c.send(Request::Account(Q48::NicknameChange { name: "Other".into(), a: 2, b: 0 }));
    e.pump(&mut [&mut c]);
    assert_eq!(nickname_reply(&c.take()), Some(Account48::NicknameChanged { ok: 0, uid: 0, face: 0 }), "not enough gold");
    e.store.update_profile(c.account, |p| Ok::<_, ()>(p.gold = price + 7)).unwrap();
    // a refused name gives the gold back
    c.send(Request::Account(Q48::NicknameChange { name: " bad".into(), a: 2, b: 0 }));
    e.pump(&mut [&mut c]);
    assert_eq!(nickname_reply(&c.take()), Some(Account48::NicknameChanged { ok: 0, uid: 0, face: 0 }));
    assert_eq!(e.store.load_profile(c.account).unwrap().gold, price + 7);
    c.send(Request::Account(Q48::NicknameChange { name: "Other".into(), a: 2, b: 0 }));
    e.pump(&mut [&mut c]);
    let m = c.take();
    assert_eq!(nickname_reply(&m), Some(Account48::NicknameChanged { ok: 1, uid: 0, face: 2 }));
    assert!(m.iter().any(|x| matches!(x, S2c::Money(_))), "8B after the payment");
    assert!(m.contains(&S2c::Name(NameMsg { flag: 0, name: "Other".into() })));
    let p = e.store.load_profile(c.account).unwrap();
    assert_eq!((p.gold, p.face), (7, 2));
    // back to the first nickname for the rest of the test
    e.store.rename_account(c.account, "Stalker").unwrap();
    e.store.update_profile(c.account, |p| Ok::<_, ()>(p.face = 6)).unwrap();

    // faction step, then the tutorial is skipped
    c.send(Request::Quest(Quest4A::SetFaction { faction: 3 }));
    c.send(Request::Quest(Quest4A::SkipTutorial));
    e.pump(&mut [&mut c]);
    c.take();
    let p = e.store.load_profile(c.account).unwrap();
    assert_eq!(p.onboarding, Profile::NICKNAME_CHOSEN | Profile::FACTION_CHOSEN | Profile::TUTORIAL_DONE);

    // the next sign-in goes to the full lobby with the chosen faction and face
    let mut c2 = e.client_of("secret-login");
    let m = e.sign_in(&mut c2);
    assert_eq!(gate(&m), vec![TutorialGate { a: 0, flags: 0xE0, g: 0, faction: 3, h: 0 }]);
    let body = m.iter().find_map(|x| if let S2c::Account(a) = x { Some(a.body.clone()) } else { None }).expect("87");
    assert_eq!((body.name.as_str(), body.b2f6), ("Stalker", 6));
}

#[test]
fn nickname_validation_refuses_taken_and_malformed_names_and_suggests_free_ones() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    // an existing player (past onboarding: nickname = its login name)
    let old = e.client("Ghost");
    let mut c = newcomer(&mut e, "newbie");
    e.sign_in(&mut c);

    c.send(Request::Account(Q48::NicknameCheck { name: "ghost".into(), x: 3 }));
    e.pump(&mut [&mut c]);
    let Some(Account48::Nickname { valid, reason, suggestions, .. }) = nickname_reply(&c.take()) else { panic!() };
    assert_eq!(valid, 0, "taken, case-insensitive");
    assert_eq!(reason, 15, "st_already_exist_clan_string");
    assert_eq!(suggestions, vec!["ghost1".to_string(), "ghost2".into(), "ghost3".into()]);

    c.send(Request::Account(Q48::NicknameCheck { name: " spaced".into(), x: 3 }));
    e.pump(&mut [&mut c]);
    let Some(Account48::Nickname { valid, reason, .. }) = nickname_reply(&c.take()) else { panic!() };
    assert_eq!(valid, 0, "leading space");
    assert_eq!(reason, 23, "st_space_symbol_clan_string");

    for (name, want, why) in [("", 21u8, "empty"), (&"x".repeat(33), 22, "long"), ("ab", 14, "control character")] {
        c.send(Request::Account(Q48::NicknameCheck { name: name.to_string(), x: 3 }));
        e.pump(&mut [&mut c]);
        let Some(Account48::Nickname { valid, reason, .. }) = nickname_reply(&c.take()) else { panic!() };
        assert_eq!((valid, reason), (0, want), "{why}");
    }

    // taking a nickname that another account uses is refused even without the check
    c.send(Request::Account(Q48::NicknameChange { name: "GHOST".into(), a: 1, b: 0xFFFF_FFFF }));
    e.pump(&mut [&mut c]);
    assert_eq!(nickname_reply(&c.take()), Some(Account48::NicknameChanged { ok: 0, uid: 0, face: 0 }));
    assert!(!e.store.has_nickname(c.account).unwrap());

    // a nickname equal to another account's private LOGIN name (not its nickname) is free
    let gd = game_data::testing::tables();
    let other = e.store.create_account_with_profile("hidden-login", "pw", &lobby_server::progress::new_player(&gd, 1, &mut game_data::SplitMix(1))).unwrap();
    e.store.rename_account(other, "Visible").unwrap();
    c.send(Request::Account(Q48::NicknameCheck { name: "hidden-login".into(), x: 3 }));
    e.pump(&mut [&mut c]);
    let Some(Account48::Nickname { valid, .. }) = nickname_reply(&c.take()) else { panic!() };
    assert_eq!(valid, 1, "login names are not compared: a check never reveals one");
    drop(old);
}

#[test]
fn an_account_past_onboarding_keeps_the_full_lobby_flags() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("veteran");
    let m = e.sign_in(&mut c);
    assert_eq!(gate(&m), vec![TutorialGate { a: 0, flags: Profile::PAST_ONBOARDING, g: 0, faction: 1, h: 0 }]);
    assert!(e.store.has_nickname(c.account).unwrap(), "the starter account kept its login name as nickname");
}

#[test]
fn skipping_the_tutorial_on_the_nickname_screen_does_not_skip_the_nickname() {
    game_data::require_tables!();
    // the nickname screen offers skip_tutorial (4A 02, docs/notes/onboarding.md); with bit 5 alone the client would open
    // the full lobby at the next sign-in and the account would play under its private login name
    let (_d, mut e) = env();
    let mut c = newcomer(&mut e, "private-login");
    e.sign_in(&mut c);
    e.handshake(&mut c);
    c.send(Request::Quest(Quest4A::SkipTutorial));
    c.send(Request::Quest(Quest4A::SetFaction { faction: 2 }));
    e.pump(&mut [&mut c]);
    c.take();
    let p = e.store.load_profile(c.account).unwrap();
    assert_eq!(p.onboarding, Profile::FACTION_CHOSEN | Profile::TUTORIAL_DONE, "the flags are stored as they came");

    let mut c2 = e.client_of("private-login");
    let m = e.sign_in(&mut c2);
    let g = gate(&m);
    assert_eq!(g.len(), 1);
    assert_eq!(g[0].flags & 0x22, 0, "no full-lobby bit while no nickname is chosen: {:#x}", g[0].flags);
    assert_eq!(g[0].flags & 0x80, 0, "bit 7 clear: the client opens the nickname screen");

    // once the nickname is chosen, the stored tutorial flag counts again
    c2.send(Request::Account(Q48::NicknameChange { name: "Public".into(), a: 1, b: 0xFFFF_FFFF }));
    e.pump(&mut [&mut c2]);
    c2.take();
    let mut c3 = e.client_of("private-login");
    let m = e.sign_in(&mut c3);
    assert_eq!(gate(&m), vec![TutorialGate { a: 0, flags: 0xE0, g: 0, faction: 2, h: 0 }]);
}

#[test]
fn a_skipped_tutorial_without_a_faction_gets_the_forced_faction_pick() {
    game_data::require_tables!();
    // nickname chosen, 4A 02 (skip), then the player quits before 4A 00: bit 5 alone would open the full lobby with no
    // faction; bit 1 makes the client force the faction pick there (step 3, docs/notes/onboarding.md)
    let (_d, mut e) = env();
    let mut c = newcomer(&mut e, "quitter");
    e.sign_in(&mut c);
    e.handshake(&mut c);
    c.send(Request::Account(Q48::NicknameChange { name: "Quitter".into(), a: 1, b: 0xFFFF_FFFF }));
    c.send(Request::Quest(Quest4A::SkipTutorial));
    e.pump(&mut [&mut c]);
    c.take();
    let mut c2 = e.client_of("quitter");
    let m = e.sign_in(&mut c2);
    assert_eq!(gate(&m), vec![TutorialGate { a: 0, flags: 0xA2, g: 0, faction: 0, h: 0 }]);
    assert_eq!(e.store.load_profile(c2.account).unwrap().onboarding & 0x02, 0, "bit 1 is sent, not stored");
    // after the pick the bit is gone
    c2.send(Request::Quest(Quest4A::SetFaction { faction: 1 }));
    e.pump(&mut [&mut c2]);
    let mut c3 = e.client_of("quitter");
    let m = e.sign_in(&mut c3);
    assert_eq!(gate(&m), vec![TutorialGate { a: 0, flags: 0xE0, g: 0, faction: 1, h: 0 }]);
}
