//! The srp LocalMatches: Training and tutorial options equal crates/local-match, rewards persist.

#[path = "../../lobby-server/tests/support/mod.rs"]
mod support;

use std::sync::Arc;

use lobby_server::LobbyConfig;
use local_match::TrainingParams;
use protocol::c2s::{Event53, Play5B, Request};
use protocol::s2c::*;
use srp::matches::{human_loadout, Matches};
use support::*;

#[test]
fn training_options_equal_local_match_and_rewards_persist() {
    game_data::require_tables!();
    let dir = tempfile::tempdir().unwrap();
    let m = Arc::new(Matches::with_fixed_id(4242));
    let mut e = Env::with(&dir.path().join("m.sqlite"), LobbyConfig { daily_quests: false, treasure_hunt: false, ..LobbyConfig::default() }, m);
    let mut c = e.client("test");
    e.sign_in(&mut c);
    e.handshake(&mut c);
    let p0 = e.store.load_profile(c.account).unwrap();
    let gd = game_data::testing::tables();
    let ctx = lobby_server::MatchCtx { account_id: c.account, account_name: "test", profile: &p0, data: &gd };
    let (want, _) = local_match::training_options(&gd, &TrainingParams::new(1, 1, 0, 4242), &human_loadout(&ctx)).unwrap();

    c.send(Request::Play(Play5B::Play { map: 1, mode: 1, weather: 0 }));
    e.pump(&mut [&mut c]);
    let msgs = c.take();
    assert_eq!(msgs[0], protocol::s2c::local_match_state());
    assert_eq!(msgs[1], S2c::Reply(Reply::Play(PlayBlob { sub: 1, blob: want })));

    e.wait(&mut [&mut c], 31);
    c.send(Request::Events(vec![Event53 { id: 5, x: 0, value: 0, time: 0 }]));
    e.pump(&mut [&mut c]);
    let msgs = c.take();
    let ids: Vec<u8> = msgs.iter().map(|m| m.id()).collect();
    assert_eq!(ids, [0x97, 0x85], "the results screen, no rewards (no 83 / 8B / 8C / 8E), and a local bots match counts nothing (no B9)");
    let p1 = e.store.load_profile(c.account).unwrap();
    // the bots mode awards nothing (client hint `st_match_settings_bots_hint`): no silver, experience or reputation, and the
    // `played_first_game` bonus stays unspent for the first real match
    assert_eq!(p1.silver, p0.silver);
    assert_eq!((p1.xp, p1.points(1)), (p0.xp, p0.points(1)));
    assert!(!p1.claimed(accounts::CLAIM_FIRST_GAME));
}

#[test]
fn tutorial_options_equal_local_match() {
    game_data::require_tables!();
    let dir = tempfile::tempdir().unwrap();
    let m = Arc::new(Matches::with_fixed_id(7));
    let mut e = Env::with(&dir.path().join("t.sqlite"), LobbyConfig { daily_quests: false, treasure_hunt: false, ..LobbyConfig::default() }, m);
    let mut c = e.client("test");
    e.sign_in(&mut c);
    e.handshake(&mut c);
    let p0 = e.store.load_profile(c.account).unwrap();
    let gd = game_data::testing::tables();
    let ctx = lobby_server::MatchCtx { account_id: c.account, account_name: "test", profile: &p0, data: &gd };
    let want = local_match::tutorial_options(&gd, &human_loadout(&ctx), 7).unwrap();
    c.send(Request::Play(Play5B::Load));
    e.pump(&mut [&mut c]);
    let msgs = c.take();
    assert_eq!(msgs[1], S2c::Reply(Reply::Play(PlayBlob { sub: 2, blob: want })));
}

/// The human's learned skills (profile) become the modifier block of his player record: the options carry the expected
/// modifier bytes for the same skills, and none without skills.
#[test]
fn learned_skills_go_into_the_match_options() {
    game_data::require_tables!();
    let Some(text) = game_data::testing::reference("skills_expected_069d.json") else {
        eprintln!("skipped learned_skills_go_into_the_match_options: no skills_expected_069d.json in the test tables folder");
        return;
    };
    let sets: Vec<serde_json::Value> = serde_json::from_str(&text).unwrap();
    let aim = sets.iter().find(|s| s["name"] == "aim").unwrap();
    let want_mask: Vec<u8> = {
        let h = aim["mask_hex"].as_str().unwrap();
        (0..h.len() / 2).map(|i| u8::from_str_radix(&h[2 * i..2 * i + 2], 16).unwrap()).collect()
    };
    let dir = tempfile::tempdir().unwrap();
    let m = Arc::new(Matches::with_fixed_id(4242));
    let mut e = Env::with(&dir.path().join("sk.sqlite"), LobbyConfig { daily_quests: false, treasure_hunt: false, ..LobbyConfig::default() }, m);
    let mut c = e.client("test");
    // level 6: five skill points (one per level gained)
    e.store.update_profile(c.account, |p| {
        p.xp = 2790;
        Ok::<_, ()>(())
    })
    .unwrap();
    e.sign_in(&mut c);
    e.handshake(&mut c);

    let blob_of = |e: &mut Env, c: &mut Client| {
        c.send(Request::Play(Play5B::Play { map: 1, mode: 1, weather: 0 }));
        e.pump(&mut [&mut *c]);
        let msgs = c.take();
        let S2c::Reply(Reply::Play(b)) = &msgs[1] else { panic!("{msgs:?}") };
        b.blob.clone()
    };
    let none = blob_of(&mut e, &mut c);
    assert!(!contains(&none, &want_mask));

    // learn in the lobby (47 00), then start a match
    c.send(Request::Skills { skills: vec![(112, 3), (113, 2)] });
    e.pump(&mut [&mut c]);
    c.take();
    // leave the first local match: the same account may start another one only after it ended
    e.wait(&mut [&mut c], 31);
    c.send(Request::Events(vec![Event53 { id: 6, x: 0, value: 0, time: 0 }]));
    e.pump(&mut [&mut c]);
    c.take();
    let with = blob_of(&mut e, &mut c);
    assert!(contains(&with, &want_mask), "options carry the mask and floats of the learned skills");
    assert_ne!(none, with);

    // the same options built directly from the profile
    let p = e.store.load_profile(c.account).unwrap();
    assert_eq!(p.skills.len(), 2);
    let gd = game_data::testing::tables();
    let ctx = lobby_server::MatchCtx { account_id: c.account, account_name: "test", profile: &p, data: &gd };
    let h = human_loadout(&ctx);
    assert_eq!(h.skills.get("skill_112"), Some(&3));
    assert_eq!(h.skills.get("skill_113"), Some(&2));
    let (want, _) = local_match::training_options(&gd, &TrainingParams::new(1, 1, 0, 4242), &h).unwrap();
    assert_eq!(with, want);
}

/// 5B 03: the range options are the account's kit with the tested weapon in its slot; nothing is saved.
#[test]
fn shooting_range_kit_has_the_tested_weapon_and_fitting_ammo() {
    game_data::require_tables!();
    use accounts::Profile;
    use srp::matches::range_loadout;
    let gd = game_data::testing::tables();
    let p = Profile::starter(); // Mosin (107) in slot 7 with its ammo 104 in slot 8
    let ctx = lobby_server::MatchCtx { account_id: 1, account_name: "test", profile: &p, data: &gd };
    let rec = |dict: u16, uid: u32| protocol::items::ItemRecord { uid, dict, value: u32::from(dict), ..Default::default() };
    // the owned primary weapon: the kit is the account's own
    let h = range_loadout(&ctx, &rec(107, accounts::UID_BASE));
    assert_eq!(h.slots, human_loadout(&ctx).slots);
    assert_eq!(h.items.len(), p.items.len());
    // a pistol the account does not own: a temporary item in slot 10 and the first compatible ammo in slot 9
    let h = range_loadout(&ctx, &rec(18, 0));
    let uid = h.slots[&10];
    assert_eq!(h.items.iter().find(|i| i.0 == uid).unwrap().1, 18);
    assert_eq!(h.slots[&7], accounts::UID_BASE, "the primary weapon stays");
    let ammo = h.items.iter().find(|i| i.0 == h.slots[&9]).unwrap().1;
    assert!(gd.modules.compatible_ammo(18).contains(&ammo), "ammo {ammo}");
    assert!(h.slot_rounds[&9] > 0);
    assert!(p.items.iter().all(|i| i.uid != uid), "nothing is saved to the profile");
    // a rifle replaces slot 7, its loaded ammo is swapped when it does not fit
    let h = range_loadout(&ctx, &rec(1201, 0));
    assert_eq!(h.items.iter().find(|i| i.0 == h.slots[&7]).unwrap().1, 1201);
    let ammo = h.items.iter().find(|i| i.0 == h.slots[&8]).unwrap().1;
    assert!(gd.modules.compatible_ammo(1201).contains(&ammo), "ammo {ammo}");
    // a record that is no weapon leaves the kit alone
    let h = range_loadout(&ctx, &rec(31, 0));
    assert_eq!(h.slots, human_loadout(&ctx).slots);
}

#[test]
fn shooting_range_blob_names_the_range_level() {
    game_data::require_tables!();
    let dir = tempfile::tempdir().unwrap();
    let m = Arc::new(Matches::with_fixed_id(99));
    let mut e = Env::with(&dir.path().join("sr.sqlite"), LobbyConfig { daily_quests: false, treasure_hunt: false, ..LobbyConfig::default() }, m);
    let mut c = e.client("test");
    e.sign_in(&mut c);
    e.handshake(&mut c);
    let weapon = protocol::items::ItemRecord { uid: accounts::UID_BASE, dict: 107, value: 107, ..Default::default() };
    c.send(Request::Play(Play5B::ShootingRange { weapon, tail: 0 }));
    e.pump(&mut [&mut c]);
    let msgs = c.take();
    assert_eq!(msgs[0], protocol::s2c::local_match_state());
    let S2c::Reply(Reply::Play(PlayBlob { sub: 3, blob })) = &msgs[1] else { panic!("{:?}", msgs[1]) };
    assert!(contains(blob, b"\x0eshooting_range\x13shooting_range/test"));
    assert_eq!(blob[5], 9, "the human and the range's eight bots");
}

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}
