//! Level-up, friend-level and faction rank-up notices (B1 types 0xA, 9 and 4; docs/notes/notices.md) and the client's
//! follow-up requests 51, 34, 3A, 35.

mod support;

use accounts::Store;
use game_data::levels::faction_level;
use game_data::GameData;
use lobby_server::notices;
use protocol::c2s::Request;
use protocol::s2c::*;
use support::*;

fn env() -> (tempfile::TempDir, Env) {
    let dir = tempfile::tempdir().unwrap();
    let e = Env::new(&dir.path().join("srp.sqlite"));
    (dir, e)
}

fn kinds(m: &[S2c]) -> Vec<(u8, String)> {
    m.iter()
        .filter_map(|x| if let S2c::SystemMessage(s) = x { Some((s.kind, s.text.clone())) } else { None })
        .collect()
}

fn give(store: &Store, account: accounts::AccountId, xp: u32, points: [u32; 4]) {
    let gd = game_data::testing::tables();
    store
        .update_profile(account, |p| {
            lobby_server::progress::add_xp(&gd, p, xp);
            for (i, n) in points.iter().enumerate() {
                p.add_points(i as u8 + 1, *n);
            }
            Ok::<_, ()>(())
        })
        .unwrap();
}

/// Parse `[a][b]...` into integers (what the client's 0x1C4750 does, one bracket pair per call).
fn ints(text: &str) -> Vec<u32> {
    text.split('[').skip(1).map(|t| t.split(']').next().unwrap().parse().unwrap()).collect()
}

#[test]
fn level_up_sends_type_0a_with_the_levels_and_the_silver() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    e.sign_in(&mut c);
    let gd = game_data::testing::tables();
    let p0 = e.store.load_profile(c.account).unwrap();
    let (l0, silver0) = (gd.levels.level_of(p0.xp), p0.silver);
    // enough for exactly three more levels
    let target = gd.levels.xp_table()[(l0 + 2) as usize - 1];
    let gain = target - p0.xp;
    give(&e.store, c.account, gain, [0; 4]);
    e.pump(&mut [&mut c]);
    let m = c.take();
    let k = kinds(&m);
    assert_eq!(k.len(), 1, "{m:?}");
    assert_eq!(k[0].0, notices::KIND_LEVEL_UP);
    let v = ints(&k[0].1);
    let p1 = e.store.load_profile(c.account).unwrap();
    let silver = p1.silver - silver0;
    assert!(silver > 0, "the data pays silver for a level");
    assert_eq!(v, [3, l0 + 1, p0.xp, p1.xp, 2, 0, silver, 0, 0], "{}", k[0].1);
    assert_eq!(gd.levels.level_of(p1.xp), l0 + 3);
    // the same message id never repeats
    let id = |m: &[S2c]| m.iter().find_map(|x| if let S2c::SystemMessage(s) = x { Some(s.message_id) } else { None }).unwrap();
    let first = id(&m);
    give(&e.store, c.account, gd.levels.xp_table()[(l0 + 3) as usize - 1] - p1.xp, [0; 4]);
    e.pump(&mut [&mut c]);
    assert_ne!(id(&c.take()), first);
}

#[test]
fn no_notice_without_a_new_level_or_rank() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    e.sign_in(&mut c);
    give(&e.store, c.account, 1, [1, 0, 0, 0]);
    e.pump(&mut [&mut c]);
    assert!(kinds(&c.take()).is_empty());
}

#[test]
fn faction_rank_up_sends_a_fresh_8e_then_one_ru_per_level() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    e.sign_in(&mut c);
    let p0 = e.store.load_profile(c.account).unwrap();
    let before = faction_level(p0.points(2));
    // 20 000 points: the levels with thresholds 5000 and 20 000 (faction 2 starts at 200 000 in the test starter,
    // so use faction 2 only when it is below; the starter's points are the base)
    give(&e.store, c.account, 0, [0, 300_000, 0, 0]);
    e.pump(&mut [&mut c]);
    let m = c.take();
    let after = faction_level(e.store.load_profile(c.account).unwrap().points(2));
    assert!(after > before);
    let ids: Vec<u8> = m.iter().map(|x| x.id()).collect();
    assert_eq!(ids[0], 0x8E, "fresh points first: {ids:x?}");
    let k = kinds(&m);
    assert_eq!(k.len() as u32, after - before);
    for (i, (kind, text)) in k.iter().enumerate() {
        assert_eq!(*kind, notices::KIND_FACTION_LEVEL);
        assert!(text.starts_with("#ru[2:"));
        assert_eq!(text, &format!("#ru[2:{}][0][0]", before + 1 + i as u32));
    }
}

#[test]
fn online_friends_see_the_level_and_offline_ones_nothing() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut a = e.client("alpha");
    let mut b = e.client("bravo");
    let z = e.client("zulu");
    e.sign_in(&mut a);
    e.sign_in(&mut b);
    e.store.friend_request(a.account, b.account).unwrap();
    e.store.friend_request(b.account, a.account).unwrap();
    e.store.friend_request(a.account, z.account).unwrap();
    e.store.friend_request(z.account, a.account).unwrap(); // zulu is a friend too, but never signs in
    let gd = game_data::testing::tables();
    let p0 = e.store.load_profile(a.account).unwrap();
    let l0 = gd.levels.level_of(p0.xp);
    give(&e.store, a.account, gd.levels.xp_table()[l0 as usize - 1] - p0.xp, [0; 4]);
    e.pump(&mut [&mut a, &mut b]);
    let ma = kinds(&a.take());
    let mb = c_messages(&mut b);
    assert_eq!(ma.len(), 1);
    assert_eq!(ma[0].0, notices::KIND_LEVEL_UP);
    assert_eq!(mb.len(), 1, "{mb:?}");
    assert_eq!((mb[0].kind, mb[0].name.as_str(), mb[0].text.as_str()), (notices::KIND_FRIEND_LEVEL, "alpha", format!("[{}][0][0]", l0 + 1).as_str()));
    assert_eq!(mb[0].sender, accounts::public_id(a.account));
    // the message layout survives the wire
    use protocol::{Decode, Encode};
    let bytes = mb[0].to_bytes().unwrap();
    assert_eq!(SystemMessage::from_bytes(&bytes).unwrap(), mb[0]);
}

fn c_messages(c: &mut Client) -> Vec<SystemMessage> {
    c.take().into_iter().filter_map(|x| if let S2c::SystemMessage(s) = x { Some(s) } else { None }).collect()
}

#[test]
fn follow_up_requests_are_consistent() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    e.sign_in(&mut c);
    // 51 (read) and 34 after a type 9 / 0xA message: no answer
    c.send(Request::ReadMessage { id: 0x1000_0001, a: 0 });
    c.send(Request::Generic { id: 0x34, a: 0 });
    e.pump(&mut [&mut c]);
    assert!(c.take().is_empty());
    // 3A (faction not in the mask) answers the 96 mask; 35 answers the 8E battle status only when a battle schedule
    // is configured (faction_battle::push_status), so nothing more here
    c.send(Request::Generic { id: 0x3A, a: 0 });
    c.send(Request::Generic { id: 0x35, a: 0 });
    e.pump(&mut [&mut c]);
    let ids: Vec<u8> = c.take().iter().map(|m| m.id()).collect();
    assert_eq!(ids, [0x96]);
}
