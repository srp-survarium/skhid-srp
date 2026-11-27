//! Item, preset, equipment profile and weapon skin flows: presets (58 / BB), 48 00..03, 45 11 / 45 12, selling a weapon with modules, unusable vouchers.

mod support;

use std::time::Duration;

use accounts::UID_BASE;
use lobby_server::LobbyConfig;
use protocol::c2s::{Account48, EquipEntry, Inventory45 as Q45, Presets58, Request, Shop46 as Q46};
use protocol::s2c::*;
use support::*;

fn env() -> (tempfile::TempDir, Env) {
    let dir = tempfile::tempdir().unwrap();
    let e = Env::new(&dir.path().join("srp.sqlite"));
    (dir, e)
}

fn presets_of(msgs: &[S2c]) -> Vec<Vec<(u32, String)>> {
    msgs.iter()
        .filter_map(|m| if let S2c::Presets(p) = m { Some(p.presets.iter().map(|x| (x.id, x.name.clone())).collect()) } else { None })
        .collect()
}

fn unequip_torso() -> Request {
    Request::Inventory(Q45::Equip {
        profile: 1,
        entries: vec![EquipEntry { item: UID_BASE + 2, dict: 31, from: 2, to: 100, count: 0, x: 1 }],
        c1: 1,
        c2: 1,
    })
}

#[test]
fn preset_create_load_rename_remove() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    e.sign_in(&mut c);
    let m = e.handshake(&mut c);
    assert_eq!(presets_of(&m), vec![vec![]], "BB with no presets after the handshake");

    c.send(Request::Presets(Presets58::Create { name: "Rush".into(), profile: 1 }));
    e.pump(&mut [&mut c]);
    let m = c.take();
    let l = presets_of(&m);
    assert_eq!(l.len(), 1);
    assert_eq!(l[0].len(), 1);
    assert_eq!(l[0][0].1, "Rush");
    let pid = l[0][0].0;
    let stored = e.store.presets(c.account).unwrap();
    assert_eq!(stored[0].slots.len(), 3, "torso, weapon and ammo slots");

    // take the torso off, then load the preset: the torso is back, 87 follows, then the list
    c.send(unequip_torso());
    e.pump(&mut [&mut c]);
    c.take();
    assert!(e.store.load_profile(c.account).unwrap().slots.get(&2).is_none());
    c.send(Request::Presets(Presets58::Load { profile: 1, preset: pid }));
    e.pump(&mut [&mut c]);
    let m = c.take();
    assert_eq!(m.iter().map(|x| x.id()).collect::<Vec<_>>(), [0x87, 0xBB]);
    let p = e.store.load_profile(c.account).unwrap();
    assert_eq!(p.slots.get(&2), Some(&(UID_BASE + 2)));
    assert_eq!(p.slot_rounds.get(&8), Some(&100));

    // rename, save over (torso off first), unknown profile ignored, remove
    c.send(Request::Presets(Presets58::Rename { preset: pid, name: "Camp".into() }));
    c.send(Request::Presets(Presets58::Create { name: "other".into(), profile: 7 }));
    e.pump(&mut [&mut c]);
    let m = c.take();
    assert_eq!(presets_of(&m), vec![vec![(pid, "Camp".to_string())], vec![(pid, "Camp".to_string())]]);
    c.send(unequip_torso());
    e.pump(&mut [&mut c]);
    c.take();
    c.send(Request::Presets(Presets58::Save { profile: 1, preset: pid }));
    c.send(Request::Presets(Presets58::Remove { preset: 999 }));
    c.send(Request::Presets(Presets58::Remove { preset: pid }));
    e.pump(&mut [&mut c]);
    let m = c.take();
    let l = presets_of(&m);
    assert_eq!(l.len(), 3);
    assert_eq!(l[2], vec![]);
    assert!(e.store.presets(c.account).unwrap().is_empty());
}

#[test]
fn preset_skips_items_that_are_gone() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    e.sign_in(&mut c);
    c.send(Request::Presets(Presets58::Create { name: "A".into(), profile: 1 }));
    e.pump(&mut [&mut c]);
    c.take();
    // sell the torso (removes it), then load: the torso slot stays empty, the rest is set
    c.send(Request::Shop(Q46::Sell { item: UID_BASE + 2, amount: 1 }));
    e.pump(&mut [&mut c]);
    c.take();
    let pid = e.store.presets(c.account).unwrap()[0].id;
    c.send(Request::Presets(Presets58::Load { profile: 1, preset: pid }));
    e.pump(&mut [&mut c]);
    c.take();
    let p = e.store.load_profile(c.account).unwrap();
    assert!(p.slots.get(&2).is_none());
    assert_eq!(p.slots.get(&7), Some(&UID_BASE));
}


#[test]
fn selling_a_weapon_frees_its_modules() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    e.sign_in(&mut c);
    let module_uid = e
        .store
        .update_profile(c.account, |p| {
            let m = p.add_item(1772, 1772);
            p.attach.entry(UID_BASE).or_default().insert("scope_dovetail".into(), m);
            Ok::<_, ()>(m)
        })
        .unwrap();
    c.send(Request::Shop(Q46::Sell { item: UID_BASE, amount: 1 }));
    e.pump(&mut [&mut c]);
    let m = c.take();
    let S2c::Reply(Reply::Shop(Shop46::Sell { item, .. })) = &m[0] else { panic!("{m:?}") };
    assert_eq!(*item, UID_BASE);
    let Some(S2c::Reply(Reply::Inventory(Inventory45::Records { items, .. }))) = m.get(1) else { panic!("{m:?}") };
    assert_eq!((items[0].uid, items[0].attached_to), (module_uid, 0));
    let p = e.store.load_profile(c.account).unwrap();
    assert!(p.item(module_uid).is_some() && p.owner_of(module_uid).is_none() && p.attach.is_empty());
}

#[test]
fn using_a_non_voucher_changes_nothing() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    e.sign_in(&mut c);
    c.send(Request::Inventory(Q45::Use { item: UID_BASE + 2 }));
    c.send(Request::Inventory(Q45::Use { item: 424242 }));
    e.pump(&mut [&mut c]);
    assert!(c.take().is_empty());
    assert_eq!(e.store.load_profile(c.account).unwrap().items.len(), 12);
    let _ = (Duration::ZERO, LobbyConfig::default());
}

#[test]
fn profile_changed_is_answered_with_the_current_profile_id() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    e.sign_in(&mut c);
    // an old-style account owns profile 0 only (wire id 1): a locked profile cannot be selected
    c.send(Request::Account(Account48::ProfileChanged { profile: 1 }));
    c.send(Request::Account(Account48::ProfileChanged { profile: 2 }));
    c.send(Request::Account(Account48::ProfileChanged { profile: 9 }));
    e.pump(&mut [&mut c]);
    let m = c.take();
    assert_eq!(m, vec![S2c::Reply(Reply::Account(protocol::s2c::Account48::ProfileSet { profile: 1 })); 3]);
}

fn account_87(m: &[S2c]) -> &Account87 {
    m.iter().rev().find_map(|x| if let S2c::Account(a) = x { Some(a) } else { None }).expect("an 87")
}

#[test]
fn the_87_lists_three_profiles_and_locked_ones_are_buy_slots() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    let m = e.sign_in(&mut c);
    let a = account_87(&m);
    assert_eq!(a.body.profiles.iter().map(|p| (p.id, p.kind)).collect::<Vec<_>>(), vec![(1, 0), (2, 2), (3, 2)]);
    assert_eq!(a.current_profile, 1);
    assert_eq!(a.profile_state.len(), 3);
    assert!(a.body.profiles[1].slots.is_empty());
}

#[test]
fn buying_switching_renaming_and_disabling_profiles() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    e.sign_in(&mut c);
    let silver0 = e.store.load_profile(c.account).unwrap().silver;
    // buy the second profile: 200 000 silver, 87 + 8B
    c.send(Request::Account(Account48::BuyPreset { a: 1 }));
    e.pump(&mut [&mut c]);
    let m = c.take();
    assert_eq!(account_87(&m).body.profiles[1].kind, 0);
    let p = e.store.load_profile(c.account).unwrap();
    assert!(p.equip.profiles[1].owned);
    assert_eq!(p.silver, silver0 - 200_000);
    // buying it again, or a bad index, charges nothing
    c.send(Request::Account(Account48::BuyPreset { a: 1 }));
    c.send(Request::Account(Account48::BuyPreset { a: 7 }));
    e.pump(&mut [&mut c]);
    c.take();
    assert_eq!(e.store.load_profile(c.account).unwrap().silver, silver0 - 200_000);
    // the third one costs 500 000
    c.send(Request::Account(Account48::BuyPreset { a: 2 }));
    e.pump(&mut [&mut c]);
    c.take();
    assert_eq!(e.store.load_profile(c.account).unwrap().silver, silver0 - 700_000);

    // select profile id 2: its (empty) equipment comes out, the torso of profile 0 goes back to its table
    c.send(Request::Account(Account48::ProfileChanged { profile: 2 }));
    e.pump(&mut [&mut c]);
    let m = c.take();
    assert_eq!(m.last(), Some(&S2c::Reply(Reply::Account(protocol::s2c::Account48::ProfileSet { profile: 2 }))));
    let a = account_87(&m);
    assert_eq!(a.current_profile, 2);
    assert!(a.body.profiles[1].slots.is_empty() && !a.body.profiles[0].slots.is_empty());
    let p = e.store.load_profile(c.account).unwrap();
    assert!(p.slots.is_empty());
    assert_eq!(p.profile_slots(0).get(&2), Some(&(UID_BASE + 2)));
    // equip the torso in profile 2 (the request names it): profile 0 keeps its own
    c.send(Request::Inventory(Q45::Equip {
        profile: 2,
        entries: vec![EquipEntry { item: UID_BASE + 2, dict: 31, from: 100, to: 2, count: 0, x: 1 }],
        c1: 1,
        c2: 1,
    }));
    e.pump(&mut [&mut c]);
    c.take();
    let p = e.store.load_profile(c.account).unwrap();
    assert_eq!(p.slots.get(&2), Some(&(UID_BASE + 2)));
    assert_eq!(p.equip.profiles[0].slots.get(&2), Some(&(UID_BASE + 2)));

    // rename survives a reload; a bad name is refused
    c.send(Request::Account(Account48::ProfileRename { profile: 2, name: "  Sniper ".into() }));
    c.send(Request::Account(Account48::ProfileRename { profile: 2, name: "   ".into() }));
    e.pump(&mut [&mut c]);
    let m = c.take();
    assert_eq!(account_87(&m).body.profiles[1].name, "Sniper");
    assert_eq!(e.store.load_profile(c.account).unwrap().equip.profiles[1].name, "Sniper");

    // disabling the current profile makes another enabled one current; the last enabled one cannot be disabled
    c.send(Request::Account(Account48::EnableProfile { profile: 2, enable: 0 }));
    e.pump(&mut [&mut c]);
    let m = c.take();
    assert_eq!(account_87(&m).current_profile, 1);
    assert_eq!(account_87(&m).body.profiles[1].flag, 0);
    c.send(Request::Account(Account48::EnableProfile { profile: 3, enable: 0 }));
    c.send(Request::Account(Account48::EnableProfile { profile: 1, enable: 0 }));
    e.pump(&mut [&mut c]);
    c.take();
    let p = e.store.load_profile(c.account).unwrap();
    assert_eq!(p.equip.profiles.iter().filter(|x| x.enabled).count(), 1, "one profile always stays on");
}

#[test]
fn skins_unlock_by_owning_the_variant_and_change_the_weapon_dict() {
    game_data::require_tables!();
    use protocol::c2s::Workshop60;
    let (_d, mut e) = env();
    let mut c = e.client("test");
    e.sign_in(&mut c);
    // a plain SR-2M (1225); the skin variant 1240 is not owned yet
    let id = c.account;
    let weapon = e.store.update_profile(id, |p| Ok::<_, ()>(p.add_item(1225, 1225))).unwrap();
    let b3 = |m: &[S2c]| m.iter().find_map(|x| if let S2c::Workshop(w) = x { Some(w.unlocked.clone()) } else { None }).unwrap();
    c.send(Request::Workshop(Workshop60::Opened { dict: 1225 }));
    e.pump(&mut [&mut c]);
    assert_eq!(b3(&c.take()), vec![1225]);
    // locked skin: the change is refused, a preview is answered and stores nothing
    c.send(Request::Inventory(Q45::Skin { item: weapon, skin: 1240 }));
    e.pump(&mut [&mut c]);
    assert!(c.take().is_empty());
    assert_eq!(e.store.load_profile(c.account).unwrap().item(weapon).unwrap().dict_id, 1225);
    c.send(Request::Inventory(Q45::SkinPreview { item: weapon, skin: 1240 }));
    e.pump(&mut [&mut c]);
    let m = c.take();
    let S2c::Reply(Reply::Inventory(Inventory45::Skin { sub: 0x12, item })) = &m[0] else { panic!("{m:?}") };
    assert_eq!((item.uid, item.dict), (weapon, 1240));
    assert_eq!(e.store.load_profile(c.account).unwrap().item(weapon).unwrap().dict_id, 1225, "a preview stores nothing");
    // a skin of another weapon is refused even for a preview
    c.send(Request::Inventory(Q45::SkinPreview { item: weapon, skin: 1203 }));
    e.pump(&mut [&mut c]);
    assert!(c.take().is_empty());

    // owning the variant unlocks the skin (stored), the workshop lists it, and the change works
    e.store.update_profile(id, |p| Ok::<_, ()>(p.add_item(1240, 1225))).unwrap();
    c.send(Request::Workshop(Workshop60::Opened { dict: 1225 }));
    e.pump(&mut [&mut c]);
    assert_eq!(b3(&c.take()), vec![1225, 1240]);
    assert!(e.store.load_profile(c.account).unwrap().skins.contains(&1240));
    c.send(Request::Inventory(Q45::Skin { item: weapon, skin: 1240 }));
    e.pump(&mut [&mut c]);
    let m = c.take();
    let S2c::Reply(Reply::Inventory(Inventory45::Skin { sub: 0x11, item })) = &m[0] else { panic!("{m:?}") };
    assert_eq!((item.uid, item.dict, item.value), (weapon, 1240, 1225));
    assert!(m.iter().any(|x| matches!(x, S2c::Account(_))), "87 follows so the character rebuilds");
    assert_eq!(e.store.load_profile(c.account).unwrap().item(weapon).unwrap().dict_id, 1240);
    // taking it off again: the plain weapon id is always offered
    c.send(Request::Inventory(Q45::Skin { item: weapon, skin: 1225 }));
    e.pump(&mut [&mut c]);
    c.take();
    assert_eq!(e.store.load_profile(c.account).unwrap().item(weapon).unwrap().dict_id, 1225);
}

#[test]
fn a_skin_is_cosmetic_the_bonus_and_the_sell_price_follow_the_weapon_underneath() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    e.sign_in(&mut c);
    let id = c.account;
    let gd = game_data::testing::tables();
    let (weapon, variant) = e
        .store
        .update_profile(id, |p| {
            let w = p.add_item(1225, 1225);
            let v = p.add_item(1240, 1225);
            p.slots.insert(5, w);
            Ok::<_, ()>((w, v))
        })
        .unwrap();
    let plain = lobby_server::boosters::worn_bonus(&gd, &e.store.load_profile(id).unwrap());
    c.send(Request::Inventory(Q45::Skin { item: weapon, skin: 1240 }));
    e.pump(&mut [&mut c]);
    c.take();
    let p = e.store.load_profile(id).unwrap();
    assert_eq!(p.item(weapon).unwrap().dict_id, 1240);
    assert_eq!(lobby_server::boosters::worn_bonus(&gd, &p), plain, "the skin carries no reward bonus");
    // the skin survives a reload and the real weapon is remembered
    assert_eq!(p.real_dict(p.item(weapon).unwrap()), 1225);
    // buy-sell farming: sell the variant back, the unlock stays, the base weapon still has no bonus
    c.send(Request::Shop(Q46::Sell { item: variant, amount: 1 }));
    e.pump(&mut [&mut c]);
    c.take();
    let p = e.store.load_profile(id).unwrap();
    assert!(p.skins.contains(&1240));
    assert_eq!(lobby_server::boosters::worn_bonus(&gd, &p), plain);
    // selling the skinned base weapon pays the base weapon's price
    let before = p.silver;
    c.send(Request::Shop(Q46::Sell { item: weapon, amount: 1 }));
    e.pump(&mut [&mut c]);
    c.take();
    let p = e.store.load_profile(id).unwrap();
    assert_eq!(u64::from(p.silver - before), gd.shop.sell_gain(&gd.items, 1225, 1, 1).1);
    assert!(p.skin_base.is_empty());
}
