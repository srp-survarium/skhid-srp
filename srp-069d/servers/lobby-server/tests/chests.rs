//! Chest opening (46 06) and player details (38 00 -> 93).

mod support;

use accounts::public_id;
use protocol::c2s::{Request, Shop46 as Q46};
use protocol::s2c::*;
use protocol::Decode;
use support::*;

fn env() -> (tempfile::TempDir, Env) {
    let dir = tempfile::tempdir().unwrap();
    let e = Env::new(&dir.path().join("srp.sqlite"));
    (dir, e)
}

#[test]
fn opening_a_chest_rolls_the_table_and_consumes_one() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    e.sign_in(&mut c);
    e.handshake(&mut c);
    // two random module chests (dict 1872)
    let uid = e.store.update_profile(c.account, |p| Ok::<_, ()>(p.add_item(1872, 2))).unwrap();
    c.send(Request::Shop(Q46::OpenChest { item: uid }));
    e.pump(&mut [&mut c]);
    let m = c.take();
    let Some(S2c::Reply(Reply::Shop(Shop46::Chest { item, chest_dict, rewards }))) = m.first() else { panic!("{m:?}") };
    assert_eq!((*item, *chest_dict), (uid, 1872));
    assert_eq!(rewards.len(), 1);
    let p = e.store.load_profile(c.account).unwrap();
    assert_eq!(p.item(uid).unwrap().count, 1, "one unit taken, no 90 sent");
    assert!(!m.iter().any(|x| matches!(x, S2c::ItemCounts(_))));
    // the reward is in the inventory under the record's uid
    let r = &rewards[0];
    assert_eq!(p.item(r.uid).map(|i| i.dict_id), Some(r.dict));
    // an unopenable chest (equip chest, item levels unmapped) changes nothing; the answer is 84 46 (unlocks the window)
    let eq = e.store.update_profile(c.account, |p| Ok::<_, ()>(p.add_item(1115, 1))).unwrap();
    c.send(Request::Shop(Q46::OpenChest { item: eq }));
    e.pump(&mut [&mut c]);
    let m = c.take();
    assert!(
        matches!(m.as_slice(), [S2c::Denied(Denied { request: 0x46, body: DeniedBody::Text(t) })] if t == lobby_server::chests::REFUSED_TEXT),
        "{m:?}"
    );
    assert_eq!(e.store.load_profile(c.account).unwrap().item(eq).unwrap().count, 1);
}

/// Modules out of chests carry a rarity (group 312) and their drop modifier pairs in the record and in the profile: the
/// PvP win chest (1872) rolls the general table (green / blue / orange, roughly 71 / 24 / 5 % for two-modifier modules,
/// 42 / 34 / 24 % for one-modifier ones), a rare-module chest (1957) gives ten blue modules. Record bytes: the pair list
/// starts at offset 16 (`u8 n`, then `u16 group, u16 grade`) with (312, rarity).
#[test]
fn chest_modules_get_a_rarity_and_drop_pairs() {
    game_data::require_tables!();
    use protocol::Encode;
    let (_d, mut e) = env();
    let mut c = e.client("test");
    e.sign_in(&mut c);
    e.handshake(&mut c);
    e.server.lobby.seed(17);
    let gd = game_data::testing::tables();
    const N: u32 = 200;
    let uid = e.store.update_profile(c.account, |p| Ok::<_, ()>(p.add_item(1872, N))).unwrap();
    let mut n = [0u32; 6];
    for _ in 0..N {
        c.send(Request::Shop(Q46::OpenChest { item: uid }));
        e.pump(&mut [&mut c]);
        let m = c.take();
        let Some(S2c::Reply(Reply::Shop(Shop46::Chest { rewards, .. }))) = m.first() else { panic!("{m:?}") };
        let r = &rewards[0];
        assert!(gd.items.is_module(r.dict));
        let rar = game_data::merge::rarity(&r.mods);
        n[rar as usize] += 1;
        let b = r.to_bytes().unwrap();
        assert_eq!(b[16] as usize, r.mods.len());
        assert_eq!(&b[17..21], &[0x38, 0x01, rar as u8, 0], "{r:?}");
        let (first, second) = gd.module_drops.drop_groups(r.dict);
        assert_eq!(r.mods.iter().filter(|p| Some(p.0) == first || Some(p.0) == second).count(), first.iter().count() + second.iter().count(), "{r:?}");
        assert_eq!(e.store.load_profile(c.account).unwrap().mods.get(&r.uid).map(|m| game_data::merge::rarity(m)), Some(rar));
    }
    assert_eq!(n[0] + n[4] + n[5], 0, "{n:?}");
    assert!(n[1] > n[2] && n[2] > n[3] && n[3] > 0, "green > blue > orange: {n:?}");
    // the rare-module chest: its ten fixed modules, all blue
    let rare = e.store.update_profile(c.account, |p| Ok::<_, ()>(p.add_item(1957, 1))).unwrap();
    c.send(Request::Shop(Q46::OpenChest { item: rare }));
    e.pump(&mut [&mut c]);
    let m = c.take();
    let Some(S2c::Reply(Reply::Shop(Shop46::Chest { rewards, .. }))) = m.first() else { panic!("{m:?}") };
    assert_eq!(rewards.len(), 10);
    assert!(rewards.iter().all(|r| game_data::merge::rarity(&r.mods) == 2), "{rewards:?}");
}

#[test]
fn money_rewards_are_credited_and_listed() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    e.sign_in(&mut c);
    e.handshake(&mut c);
    // seven_days_chest_rank_2: voucher 213 x1 and 861 x3 (fixed pack)
    let uid = e.store.update_profile(c.account, |p| Ok::<_, ()>(p.add_item(1301, 1))).unwrap();
    c.send(Request::Shop(Q46::OpenChest { item: uid }));
    e.pump(&mut [&mut c]);
    let m = c.take();
    let Some(S2c::Reply(Reply::Shop(Shop46::Chest { rewards, .. }))) = m.first() else { panic!("{m:?}") };
    let got: Vec<(u16, u32)> = rewards.iter().map(|r| (r.dict, r.value)).collect();
    assert_eq!(got, vec![(213, 1), (861, 3)]);
    assert!(e.store.load_profile(c.account).unwrap().item(uid).is_none());
}

#[test]
fn player_details_for_self_carry_the_real_body() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    e.sign_in(&mut c);
    e.handshake(&mut c);
    c.send(Request::VictoryData { a: 0, player: public_id(c.account), b: 0 });
    e.pump(&mut [&mut c]);
    let m = c.take();
    let Some(S2c::Raw { id: 0x93, payload }) = m.first() else { panic!("{m:?}") };
    let d = PlayerDetails93::from_bytes(payload).unwrap();
    assert_eq!(d.id, public_id(c.account));
    let p = e.store.load_profile(c.account).unwrap();
    assert_eq!(d.body.items.len(), p.items.len());
    // somebody else's id: no answer
    c.send(Request::VictoryData { a: 0, player: public_id(c.account + 99), b: 0 });
    e.pump(&mut [&mut c]);
    assert!(c.take().is_empty());
}
