use std::collections::BTreeMap;
use std::sync::Arc;

use accounts::{Account, Currency, DecalRow, Error, FriendState, HashCost, ItemRow, Profile, Store, UpdateError, UID_BASE};

fn mem() -> Store {
    let s = Store::open_in_memory().unwrap();
    s.set_hash_cost(HashCost::FAST_FOR_TESTS);
    s
}

fn file_store(path: &std::path::Path) -> Store {
    let s = Store::open(path).unwrap();
    s.set_hash_cost(HashCost::FAST_FOR_TESTS);
    s
}

/// a profile that uses every table
fn busy_profile() -> Profile {
    let mut p = Profile::starter();
    let m1 = p.add_item(1772, 1772);
    let m2 = p.add_item(1774, 1774);
    p.add_item(66, 5); // a stack
    p.slots.insert(13, 6);
    p.slot_rounds.insert(13, 3);
    p.slot_rounds.insert(20, 7); // rounds without a slot item
    p.mods.insert(UID_BASE, vec![(201, 1), (203, 0), (205, 2)]);
    p.attach.insert(UID_BASE, BTreeMap::from([("scope_dovetail".to_string(), m1), ("muzzle".to_string(), m2)]));
    p.bound.insert(m1, 107);
    p.faction = 3;
    p.decals.insert(UID_BASE + 2, vec![DecalRow { slot: 1, decal_uid: m2, dict: 718, x_bits: 1.5f64.to_bits() }]);
    p.claims.insert("tutorial_completed".to_string());
    p.fac_points.insert(3, 123_456);
    p.xp = 99_999;
    p.skills = BTreeMap::from([(112, 5), (513, 2), (554, 1)]);
    p.version = 17;
    p.silver = 12;
    p.gold = 0;
    p.parts = u32::MAX;
    p
}

#[test]
fn accounts_and_login() {
    let s = mem();
    let id = s.create_account("Scav_One", "pw1").unwrap();
    assert_eq!(s.verify_login("Scav_One", "pw1").unwrap(), id);
    assert_eq!(s.verify_login("scav_one", "pw1").unwrap(), id, "names are case-insensitive");
    assert!(matches!(s.verify_login("Scav_One", "pw2"), Err(Error::BadCredentials)));
    assert!(matches!(s.verify_login("nobody", "pw1"), Err(Error::BadCredentials)));
    assert!(matches!(s.create_account("SCAV_ONE", "x"), Err(Error::NameTaken)));
    for bad in ["", " lead", "trail ", "tab\there", &"x".repeat(33)] {
        assert!(matches!(s.create_account(bad, "x"), Err(Error::InvalidName(_))), "{bad:?}");
    }
    assert!(matches!(s.create_account("ok", ""), Err(Error::InvalidPassword(_))));
    assert!(matches!(s.create_account("ok", &"p".repeat(257)), Err(Error::InvalidPassword(_))));
    let id2 = s.create_account("second", "pw").unwrap();
    assert_ne!(id, id2);
    assert_eq!(s.account_count().unwrap(), 2);
    assert_eq!(s.account(id).unwrap().unwrap().name, "Scav_One");
    assert_eq!(s.find_account("SECOND").unwrap().unwrap().id, id2);
    assert!(s.find_account("third").unwrap().is_none());
    s.change_password(id, "new").unwrap();
    assert!(s.verify_login("Scav_One", "pw1").is_err());
    assert_eq!(s.verify_login("Scav_One", "new").unwrap(), id);
    assert!(matches!(s.change_password(999, "x"), Err(Error::NoSuchAccount)));
}

#[test]
fn password_is_hashed_in_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.sqlite");
    {
        let s = file_store(&path);
        s.create_account("hashed", "topsecret").unwrap();
    }
    let c = rusqlite::Connection::open(&path).unwrap();
    let h: String = c.query_row("SELECT pass_hash FROM accounts", [], |r| r.get(0)).unwrap();
    assert!(h.starts_with("$argon2id$"), "{h}");
    assert!(!h.contains("topsecret"));
}

#[test]
fn profile_round_trip() {
    let s = mem();
    let id = s.create_account("p", "pw").unwrap();
    assert_eq!(s.load_profile(id).unwrap(), Profile::starter());
    let p = busy_profile();
    s.save_profile(id, &p).unwrap();
    assert_eq!(s.load_profile(id).unwrap(), p);
    // saving a smaller profile removes the old rows
    let mut q = Profile::starter();
    q.silver = 1;
    s.save_profile(id, &q).unwrap();
    assert_eq!(s.load_profile(id).unwrap(), q);
    assert!(matches!(s.load_profile(777), Err(Error::NoSuchAccount)));
    assert!(matches!(s.save_profile(777, &q), Err(Error::NoSuchAccount)));
}

#[test]
fn item_order_is_kept() {
    let s = mem();
    let id = s.create_account("o", "pw").unwrap();
    let mut p = Profile::starter();
    p.items = vec![
        ItemRow { uid: 30, dict_id: 5, count: 1 },
        ItemRow { uid: 4, dict_id: 6, count: 2 },
        ItemRow { uid: 17, dict_id: 7, count: 3 },
    ];
    p.slots.clear();
    p.slot_rounds.clear();
    p.next_uid = 31;
    s.save_profile(id, &p).unwrap();
    let got: Vec<u32> = s.load_profile(id).unwrap().items.iter().map(|i| i.uid).collect();
    assert_eq!(got, vec![30, 4, 17]);
}

#[test]
fn atomic_updates() {
    let s = mem();
    let id = s.create_account("a", "pw").unwrap();
    // success: saved
    let uid = s.update_profile(id, |p| -> Result<u32, ()> { p.spend(Currency::Silver, 500).map_err(|_| ())?; Ok(p.add_item(65, 2)) }).unwrap();
    let p = s.load_profile(id).unwrap();
    assert_eq!(p.silver, 1_000_000 - 500);
    assert_eq!(p.item(uid).unwrap().dict_id, 65);
    // rejected: nothing saved, even the part of the change that ran before the error
    let r = s.update_profile(id, |p| {
        p.spend(Currency::Gold, 1)?;
        p.spend(Currency::Silver, 5_000_000)
    });
    match r {
        Err(UpdateError::Rejected(e)) => assert_eq!((e.currency, e.need), (Currency::Silver, 5_000_000)),
        other => panic!("{other:?}"),
    }
    assert_eq!(s.load_profile(id).unwrap(), p);
    // unknown account
    let r = s.update_profile::<(), ()>(4242, |_| Ok(()));
    assert!(matches!(r, Err(UpdateError::Store(Error::NoSuchAccount))));
}

#[test]
fn concurrent_updates_do_not_lose_changes() {
    let dir = tempfile::tempdir().unwrap();
    let s = Arc::new(file_store(&dir.path().join("c.sqlite")));
    let id = s.create_account("c", "pw").unwrap();
    s.update_profile(id, |p| -> Result<(), ()> { p.silver = 0; Ok(()) }).unwrap();
    let threads: Vec<_> = (0..6)
        .map(|_| {
            let s = s.clone();
            std::thread::spawn(move || {
                for _ in 0..25 {
                    s.update_profile(id, |p| -> Result<(), ()> { p.credit(Currency::Silver, 3); p.add_item(1, 1); Ok(()) }).unwrap();
                }
            })
        })
        .collect();
    for t in threads {
        t.join().unwrap();
    }
    let p = s.load_profile(id).unwrap();
    assert_eq!(p.silver, 6 * 25 * 3);
    assert_eq!(p.items.len(), 3 + 6 * 25);
    let uids: std::collections::BTreeSet<u32> = p.items.iter().map(|i| i.uid).collect();
    assert_eq!(uids.len(), p.items.len(), "uids stay unique");
}

#[test]
fn restart_keeps_independent_inventories() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("two.sqlite");
    let (a, b);
    {
        let s = file_store(&path);
        a = s.create_account("alice", "pa").unwrap();
        b = s.create_account("bob", "pb").unwrap();
        s.update_profile(a, |p| -> Result<(), ()> { p.remove_item(UID_BASE + 1); p.add_item(1419, 1419); p.silver = 111; Ok(()) }).unwrap();
        s.update_profile(b, |p| -> Result<(), ()> { p.spend(Currency::Gold, 9_000).unwrap(); p.set_faction(4).unwrap(); p.add_xp(10); Ok(()) }).unwrap();
        s.friend_request(a, b).unwrap();
    }
    let s = file_store(&path);
    assert_eq!(s.verify_login("alice", "pa").unwrap(), a);
    assert_eq!(s.verify_login("bob", "pb").unwrap(), b);
    let pa = s.load_profile(a).unwrap();
    let pb = s.load_profile(b).unwrap();
    assert_eq!(pa.silver, 111);
    assert!(pa.item(UID_BASE + 1).is_none() && pa.items.iter().any(|i| i.dict_id == 1419));
    assert!(!pa.slots.contains_key(&8), "ammo slot went with the item");
    assert_eq!((pb.silver, pb.gold, pb.faction, pb.xp), (1_000_000, 1_000, 4, 2180));
    assert!(pb.item(UID_BASE + 1).is_some() && pb.items.len() == 3, "bob's inventory is untouched by alice's changes");
    assert_eq!(s.friends(b).unwrap()[0].state, FriendState::RequestReceived);
}

#[test]
fn schema_version_is_checked() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("v.sqlite");
    drop(file_store(&path));
    drop(file_store(&path)); // reopening an existing database is fine
    {
        let c = rusqlite::Connection::open(&path).unwrap();
        c.pragma_update(None, "user_version", 99).unwrap();
    }
    assert!(matches!(Store::open(&path), Err(Error::SchemaTooNew(99))));
}

#[test]
fn profile_rules() {
    let mut p = busy_profile();
    // spend leaves the wallet alone when short
    assert!(p.spend(Currency::Gold, 1).is_err());
    assert_eq!(p.gold, 0);
    p.credit(Currency::Parts, 5); // saturates
    assert_eq!(p.parts, u32::MAX);
    assert_eq!(p.spend(Currency::Silver, 12), Ok(()));
    assert_eq!(p.silver, 0);
    // remove_item cleans every reference
    let module = *p.attach[&UID_BASE].get("muzzle").unwrap();
    let before = p.next_uid;
    p.remove_item(module).unwrap();
    assert!(!p.attach[&UID_BASE].contains_key("muzzle"));
    assert!(p.owner_of(module).is_none());
    p.remove_item(UID_BASE).unwrap(); // the weapon: slot 7, rounds, mods and the whole attach entry go
    assert!(!p.slots.contains_key(&7) && !p.mods.contains_key(&UID_BASE) && !p.attach.contains_key(&UID_BASE));
    assert!(p.remove_item(UID_BASE).is_none());
    // uids are not reused after a removal
    let u = p.add_item(5, 1);
    assert!(u >= before);
    assert!(p.set_faction(0).is_err() && p.set_faction(5).is_err());
    assert_eq!(p.bump_version(), 18);
}

#[test]
fn friends_flow() {
    let s = mem();
    let a = s.create_account("alice", "x").unwrap();
    let b = s.create_account("bob", "x").unwrap();
    let c = s.create_account("carol", "x").unwrap();
    assert!(matches!(s.friend_request(a, a), Err(Error::SelfTarget)));
    assert!(matches!(s.friend_request(a, 9999), Err(Error::NoSuchAccount)));
    assert_eq!(s.friend_request(a, b).unwrap(), FriendState::RequestSent);
    assert!(matches!(s.friend_request(a, b), Err(Error::AlreadyRequested)));
    assert_eq!(s.friends(a).unwrap()[0].state, FriendState::RequestSent);
    assert_eq!(s.friends(b).unwrap()[0].state, FriendState::RequestReceived);
    assert!(!s.are_friends(a, b).unwrap());
    assert!(matches!(s.friend_accept(a, b), Err(Error::NoRequest)), "the asker cannot accept");
    s.friend_accept(b, a).unwrap();
    assert!(s.are_friends(a, b).unwrap() && s.are_friends(b, a).unwrap());
    assert!(matches!(s.friend_request(b, a), Err(Error::AlreadyFriends)));
    // both asked each other: friends at once
    assert_eq!(s.friend_request(a, c).unwrap(), FriendState::RequestSent);
    assert_eq!(s.friend_request(c, a).unwrap(), FriendState::Friend);
    let names: Vec<String> = s.friends(a).unwrap().into_iter().map(|f| f.name).collect();
    assert_eq!(names, vec!["bob", "carol"]);
    assert!(s.friends(a).unwrap().iter().all(|f| f.state == FriendState::Friend));
    // remove from either side
    assert!(s.friend_remove(b, a).unwrap());
    assert!(!s.friend_remove(b, a).unwrap());
    assert_eq!(s.friends(a).unwrap().len(), 1);
    // decline a request
    s.friend_request(b, a).unwrap();
    assert!(s.friend_remove(a, b).unwrap());
    assert!(s.friends(a).unwrap().iter().all(|f| f.id != b));
    // ignore
    assert!(matches!(s.ignore_add(a, a), Err(Error::SelfTarget)));
    assert!(matches!(s.ignore_add(a, 9999), Err(Error::NoSuchAccount)));
    s.ignore_add(a, b).unwrap();
    s.ignore_add(a, b).unwrap(); // idempotent
    assert!(s.is_ignored(a, b).unwrap() && !s.is_ignored(b, a).unwrap());
    let ig: Vec<Account> = s.ignored(a).unwrap();
    assert_eq!(ig.len(), 1);
    assert_eq!(ig[0].name, "bob");
    assert!(s.ignore_remove(a, b).unwrap());
    assert!(!s.ignore_remove(a, b).unwrap());
    assert!(s.ignored(a).unwrap().is_empty());
}

#[test]
fn search() {
    let s = mem();
    let a = s.create_account("Scav_Alpha", "x").unwrap();
    s.create_account("Scav_Bravo", "x").unwrap();
    s.create_account("scavenger", "x").unwrap();
    s.create_account("Zed", "x").unwrap();
    s.create_account("100%_pure", "x").unwrap();
    let names = |v: Vec<Account>| v.into_iter().map(|a| a.name).collect::<Vec<_>>();
    assert_eq!(names(s.search_accounts("scav", 10, None).unwrap()), vec!["Scav_Alpha", "Scav_Bravo", "scavenger"]);
    assert_eq!(names(s.search_accounts("SCAV", 2, None).unwrap()).len(), 2);
    assert_eq!(names(s.search_accounts("scav", 10, Some(a)).unwrap()), vec!["Scav_Bravo", "scavenger"]);
    assert!(s.search_accounts("", 10, None).unwrap().is_empty());
    assert!(s.search_accounts("nope", 10, None).unwrap().is_empty());
    // wildcards are literal
    assert_eq!(names(s.search_accounts("%", 10, None).unwrap()), vec!["100%_pure"]);
    assert_eq!(names(s.search_accounts("_", 10, None).unwrap()), vec!["100%_pure", "Scav_Alpha", "Scav_Bravo"]);
}

#[test]
fn public_ids() {
    let id = 42;
    let p = accounts::public_id(id);
    assert_eq!(p, 0x5000_0000_0000_002A);
    assert_eq!(accounts::account_of_public_id(p), Some(id));
    assert_eq!(accounts::account_of_public_id(0x1234), None);
}

// ---- importer

/// A state file of the earlier reference implementation (its own format, 44 items with slots, rounds, money, one
/// installed module and its binding; no xp key).
const SAMPLE: &str = r#"{"items": [[1, 107, 107], [2, 104, 200], [3, 31, 31], [4, 55, 55], [5, 53, 250], [6, 753, 753],
  [7, 745, 745], [8, 746, 746], [9, 892, 892], [10, 43, 43], [11, 37, 37], [12, 1208, 1208], [13, 1172, 50], [14, 25, 25],
  [15, 1419, 1419], [16, 1420, 1420], [17, 1422, 1422], [18, 1392, 1392], [19, 1414, 1414], [20, 1432, 1432],
  [21, 1436, 1436], [22, 1339, 1339], [23, 1768, 1768], [24, 1766, 1766], [25, 1767, 1767], [26, 1769, 1769],
  [27, 1670, 1670], [28, 1395, 1395], [29, 1391, 1391], [30, 1398, 1398], [31, 1394, 1394], [32, 1772, 1772],
  [33, 1395, 1395], [34, 1391, 1391], [35, 1398, 1398], [36, 1394, 1394], [37, 1772, 1772], [38, 1393, 1393],
  [39, 1400, 1400], [40, 1399, 1399], [41, 1774, 1774], [42, 1772, 1772], [43, 66, 1], [44, 65, 1]],
 "slots": {"2": 3, "7": 4, "0": 9, "1": 10, "3": 6, "4": 8, "10": 12, "5": 14, "6": 11, "8": 5, "9": 13},
 "slot_rounds": {"7": 1, "0": 1, "1": 1, "3": 1, "2": 1, "4": 1, "10": 1, "5": 1, "6": 1, "8": 250, "9": 50},
 "money": [989115, 9990, 10000], "mods": {}, "faction": 2, "attach": {"4": {"picatinny_scope_medium": 42}}, "bound": {"42": 55}}"#;

#[test]
fn import_a_sample_state() {
    let s = mem();
    let id = s.create_account("imp", "pw").unwrap();
    let rep = s.import_state(id, SAMPLE).unwrap();
    assert_eq!((rep.items, rep.slots, rep.dropped), (44, 11, 0));
    let p = s.load_profile(id).unwrap();
    let v: serde_json::Value = serde_json::from_str(SAMPLE).unwrap();
    // items: same order, same fields
    for (row, want) in p.items.iter().zip(v["items"].as_array().unwrap()) {
        assert_eq!((row.uid as u64, row.dict_id as u64, row.count as u64), (want[0].as_u64().unwrap() + 100_000, want[1].as_u64().unwrap(), want[2].as_u64().unwrap()));
    }
    assert_eq!(p.items.len(), v["items"].as_array().unwrap().len());
    assert_eq!(p.slots.get(&7), Some(&100_004));
    assert_eq!(p.slots.get(&8), Some(&100_005));
    assert_eq!(p.slot_rounds.get(&8), Some(&250));
    assert_eq!(p.slot_rounds.get(&9), Some(&50));
    assert_eq!((p.silver, p.gold, p.parts), (989_115, 9_990, 10_000));
    assert_eq!(p.faction, 2);
    assert_eq!(p.xp, 2170, "no xp key: the default");
    assert_eq!(p.fac_points, (1..=4).map(|f| (f, 200_000)).collect::<BTreeMap<u8, u32>>(), "defaults");
    assert_eq!(p.attach[&100_004]["picatinny_scope_medium"], 100_042);
    assert_eq!(p.bound[&100_042], 55);
    assert_eq!(p.owner_of(100_042), Some(100_004));
    assert_eq!(p.next_uid, 100_045);
    assert!(p.mods.is_empty());
    // survives a restart of the store: loading again gives the same
    assert_eq!(s.load_profile(id).unwrap(), p);
    let id2 = s.create_account_from_state("imp2", "pw", SAMPLE).unwrap().0;
    assert_eq!(s.load_profile(id2).unwrap(), p);
}

#[test]
fn import_defaults_padding_and_dangling() {
    let json = r#"{"items": [[1,107,107],[2,104,200]],
                   "slots": {"7": 1, "8": 2, "2": 99},
                   "slot_rounds": {"8": 100, "2": 1, "30": 5},
                   "money": [500],
                   "xp": 7000, "faction": 3,
                   "fac_points": {"3": 5000},
                   "mods": {"1": [[201, 1]], "77": [[202, 0]]},
                   "attach": {"1": {"scope": 2}, "1000": {"x": 1}},
                   "bound": {"2": 107, "88": 1}}"#;
    let (p, rep) = accounts_import(json);
    // dropped: slot 2 -> missing uid 99, its rounds, mods of 77, attach of 1000, bound of 88; slot_rounds 30 stays? it has no slot
    assert_eq!(p.items.len(), 2);
    assert_eq!(p.slots, BTreeMap::from([(7, UID_BASE), (8, UID_BASE + 1)]));
    assert_eq!(p.slot_rounds, BTreeMap::from([(8, 100)]), "rounds of empty slots are dropped too");
    assert_eq!((p.silver, p.gold, p.parts), (500, 10_000, 10_000), "money padded from the defaults");
    assert_eq!((p.xp, p.faction), (7000, 3));
    assert_eq!(p.fac_points[&3], 5000);
    assert_eq!(p.fac_points[&1], 200_000);
    assert_eq!(p.mods.len(), 1);
    assert_eq!(p.attach.len(), 1);
    assert_eq!(p.bound, BTreeMap::from([(UID_BASE + 1, 107)]));
    assert!(rep.dropped >= 5, "{rep:?}");
    assert_eq!(p.next_uid, UID_BASE + 2);
}

fn accounts_import(json: &str) -> (Profile, accounts::ImportReport) {
    let s = mem();
    let id = s.create_account("t", "pw").unwrap();
    let rep = s.import_state(id, json).unwrap();
    (s.load_profile(id).unwrap(), rep)
}

#[test]
fn import_errors() {
    let s = mem();
    let id = s.create_account("t", "pw").unwrap();
    for bad in [
        "not json",
        r#"{"items": [[1,2,3],[1,4,5]]}"#,
        r#"{"slots": {"x": 1}}"#,
        r#"{"faction": 9}"#,
        r#"{"slots": {"300": 1}}"#,
    ] {
        assert!(matches!(s.import_state(id, bad), Err(Error::Import(_))), "{bad}");
    }
    // a failed import leaves the old profile
    assert_eq!(s.load_profile(id).unwrap(), Profile::starter());
    assert!(matches!(s.import_state(999, "{}"), Err(Error::NoSuchAccount)));
    assert!(matches!(s.import_state_file(id, std::path::Path::new("/no/such/file.json")), Err(Error::Import(_))));
    // an empty object: the defaults
    let rep = s.import_state(id, "{}").unwrap();
    assert_eq!(rep.items, 3);
    assert_eq!(s.load_profile(id).unwrap(), Profile::starter());
}

#[test]
fn skills_roundtrip_and_replace() {
    let s = mem();
    let id = s.create_account("sk", "pw").unwrap();
    assert!(s.load_profile(id).unwrap().skills.is_empty());
    s.update_profile(id, |p| {
        p.skills.insert(112, 3);
        p.skills.insert(543, 3);
        Ok::<_, ()>(())
    })
    .unwrap();
    let p = s.load_profile(id).unwrap();
    assert_eq!(p.skills, BTreeMap::from([(112, 3), (543, 3)]));
    assert_eq!(p.skill_points_spent(), 6);
    // a smaller set replaces the old one, zero points are not stored
    s.update_profile(id, |p| {
        p.skills = BTreeMap::from([(543, 1), (112, 0)]);
        Ok::<_, ()>(())
    })
    .unwrap();
    assert_eq!(s.load_profile(id).unwrap().skills, BTreeMap::from([(543, 1)]));
    // importing a state file with skills
    s.import_state(id, r#"{"skills": {"112": 2, "513": 5}}"#).unwrap();
    assert_eq!(s.load_profile(id).unwrap().skills, BTreeMap::from([(112, 2), (513, 5)]));
    assert!(matches!(s.import_state(id, r#"{"skills": {"70000": 1}}"#), Err(Error::Import(_))));
}

/// A version 1 database (no `skills` table) is migrated on open: data kept, skills empty and storable.
#[test]
fn v1_database_is_migrated() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("v1.sqlite");
    let id = {
        let s = file_store(&path);
        let id = s.create_account("old", "pw").unwrap();
        s.update_profile(id, |p| {
            p.silver = 777;
            Ok::<_, ()>(())
        })
        .unwrap();
        id
    };
    {
        // turn it back into a v1 file
        let c = rusqlite::Connection::open(&path).unwrap();
        c.execute_batch(
            "ALTER TABLE profiles DROP COLUMN onboarding; ALTER TABLE profiles DROP COLUMN face; DROP TABLE league_history; DROP TABLE league; DROP TABLE hunt_entries; DROP TABLE hunt; DROP TABLE battle_rows; DROP TABLE login_calendar; DROP TABLE boosters; ALTER TABLE accounts DROP COLUMN nick; DROP TABLE daily_quest_slots; DROP TABLE daily_quests; DROP TABLE premium; DROP TABLE skills; DROP TABLE achievements; DROP TABLE clan_reservations; DROP TABLE clan_invites; DROP TABLE clan_members; DROP TABLE clan_ranks; DROP TABLE clans; DROP TABLE preset_slots; DROP TABLE presets;
             ALTER TABLE accounts DROP COLUMN ban_kind; ALTER TABLE accounts DROP COLUMN ban_reason; ALTER TABLE accounts DROP COLUMN ban_until;
             PRAGMA user_version = 1;",
        )
        .unwrap();
    }
    let s = file_store(&path);
    let v: i64 = rusqlite::Connection::open(&path).unwrap().query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
    assert_eq!(v, 23);
    assert!(s.ban_of(id).unwrap().is_none());
    let mut p = s.load_profile(id).unwrap();
    assert_eq!(p.silver, 777);
    assert!(p.skills.is_empty());
    p.skills.insert(112, 1);
    s.save_profile(id, &p).unwrap();
    assert_eq!(s.load_profile(id).unwrap().skills.len(), 1);
    drop(s);
    assert_eq!(file_store(&path).load_profile(id).unwrap().skills.len(), 1);
}

#[test]
fn achievement_progress_roundtrip_and_v2_migration() {
    use accounts::AchProgress;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("v2.sqlite");
    let id = {
        let s = file_store(&path);
        let id = s.create_account("ach", "pw").unwrap();
        s.update_profile(id, |p| {
            p.ach.insert(611, AchProgress { value: 7, run: 0 });
            p.ach.insert(616, AchProgress { value: 3, run: 2 });
            Ok::<_, ()>(())
        })
        .unwrap();
        assert_eq!(s.load_profile(id).unwrap().ach.get(&616), Some(&AchProgress { value: 3, run: 2 }));
        id
    };
    {
        // a v2 file has no achievements table
        let c = rusqlite::Connection::open(&path).unwrap();
        c.execute_batch(
            "ALTER TABLE profiles DROP COLUMN onboarding; ALTER TABLE profiles DROP COLUMN face; DROP TABLE league_history; DROP TABLE league; DROP TABLE hunt_entries; DROP TABLE hunt; DROP TABLE battle_rows; DROP TABLE login_calendar; DROP TABLE boosters; ALTER TABLE accounts DROP COLUMN nick; DROP TABLE daily_quest_slots; DROP TABLE daily_quests; DROP TABLE premium; DROP TABLE achievements; DROP TABLE clan_reservations; DROP TABLE clan_invites; DROP TABLE clan_members; DROP TABLE clan_ranks; DROP TABLE clans; DROP TABLE preset_slots; DROP TABLE presets;
             ALTER TABLE accounts DROP COLUMN ban_kind; ALTER TABLE accounts DROP COLUMN ban_reason; ALTER TABLE accounts DROP COLUMN ban_until;
             PRAGMA user_version = 2;",
        )
        .unwrap();
    }
    let s = file_store(&path);
    let mut p = s.load_profile(id).unwrap();
    assert!(p.ach.is_empty());
    p.ach.insert(201, AchProgress { value: 1, run: 0 });
    s.save_profile(id, &p).unwrap();
    assert_eq!(s.load_profile(id).unwrap().ach.len(), 1);
}

/// A version 2 database with the old small uids is renumbered on open: every reference follows
/// (items, slots, modifiers, installed modules, bound modules, next_uid), other accounts are untouched,
/// and the result equals the profile as the new numbering would have stored it.
#[test]
fn v4_database_uids_are_renumbered() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("v2.sqlite");
    let (a, b, want_a, want_b) = {
        let s = file_store(&path);
        let a = s.create_account("old", "pw").unwrap();
        let b = s.create_account("fresh", "pw").unwrap();
        let pa = busy_profile();
        s.save_profile(a, &pa).unwrap();
        let mut pb = busy_profile();
        pb.silver = 5;
        s.save_profile(b, &pb).unwrap();
        (a, b, pa, pb)
    };
    {
        // turn account `a` back into the old numbering (uids 1, 2, 3 ...) and the file into version 2
        let c = rusqlite::Connection::open(&path).unwrap();
        c.execute_batch("PRAGMA foreign_keys=OFF;").unwrap();
        let d = 100_000;
        c.execute_batch(&format!(
            "UPDATE items SET uid = -(uid - {d}) WHERE account_id = {a};
             UPDATE items SET uid = -uid WHERE uid < 0;
             UPDATE slots SET uid = uid - {d} WHERE uid IS NOT NULL AND account_id = {a};
             UPDATE item_mods SET uid = -(uid - {d}) WHERE account_id = {a};
             UPDATE item_mods SET uid = -uid WHERE uid < 0;
             UPDATE attachments SET weapon_uid = -(weapon_uid - {d}), module_uid = module_uid - {d} WHERE account_id = {a};
             UPDATE attachments SET weapon_uid = -weapon_uid WHERE weapon_uid < 0;
             UPDATE bound SET module_uid = module_uid - {d} WHERE account_id = {a};
             UPDATE profiles SET next_uid = next_uid - {d} WHERE account_id = {a};
             ALTER TABLE profiles DROP COLUMN onboarding; ALTER TABLE profiles DROP COLUMN face; DROP TABLE league_history; DROP TABLE league; DROP TABLE hunt_entries; DROP TABLE hunt; DROP TABLE battle_rows; DROP TABLE login_calendar; DROP TABLE boosters; ALTER TABLE accounts DROP COLUMN nick; DROP TABLE daily_quest_slots; DROP TABLE daily_quests; DROP TABLE premium; DROP TABLE preset_slots; DROP TABLE presets; PRAGMA user_version = 4;"
        ))
        .unwrap();
        let min: i64 = c.query_row("SELECT MIN(uid) FROM items WHERE account_id = ?1", [a], |r| r.get(0)).unwrap();
        assert_eq!(min, 1, "the fixture really has old uids");
    }
    let s = file_store(&path);
    let v: i64 = rusqlite::Connection::open(&path).unwrap().query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
    assert_eq!(v, 23);
    assert_eq!(s.load_profile(a).unwrap(), want_a);
    assert_eq!(s.load_profile(b).unwrap(), want_b);
    // new items continue above everything
    let mut p = s.load_profile(a).unwrap();
    let max = p.items.iter().map(|i| i.uid).max().unwrap();
    assert!(p.add_item(5, 1) > max);
    // a second open changes nothing
    drop(s);
    assert_eq!(file_store(&path).load_profile(b).unwrap(), want_b);
}

#[test]
fn imported_uids_are_lifted() {
    let s = mem();
    let id = s.create_account("imp", "pw").unwrap();
    s.import_state(id, r#"{"items": [[1, 107, 107], [2, 104, 200]], "slots": {"7": 1, "8": 2}, "attach": {"1": {"scope": 2}}}"#).unwrap();
    let p = s.load_profile(id).unwrap();
    assert_eq!(p.items.iter().map(|i| i.uid).collect::<Vec<_>>(), vec![UID_BASE + 0, UID_BASE + 1]);
    assert_eq!(p.slots.get(&7), Some(&UID_BASE));
    assert_eq!(p.attach[&UID_BASE]["scope"], UID_BASE + 1);
    assert_eq!(p.next_uid, UID_BASE + 2);
}

#[test]
fn consume_counts_down_and_cleans_up() {
    let mut p = Profile::starter();
    let stack = p.add_item(66, 5);
    p.slots.insert(13, stack);
    p.slot_rounds.insert(13, 5);
    p.slots.insert(14, stack);
    p.slot_rounds.insert(14, 2);
    assert_eq!(p.consume(stack, 0), None);
    assert_eq!(p.consume(1, 1), None, "no such item");
    assert_eq!(p.consume(stack, 2), Some(accounts::Consumed { taken: 2, left: 3 }));
    assert_eq!(p.slot_rounds[&13], 3, "units placed are capped to the stack");
    assert_eq!(p.slot_rounds[&14], 2);
    assert_eq!(p.consume(stack, 9), Some(accounts::Consumed { taken: 3, left: 0 }), "never more than held");
    assert!(p.item(stack).is_none());
    assert!(!p.slots.contains_key(&13) && !p.slots.contains_key(&14) && !p.slot_rounds.contains_key(&13) && !p.slot_rounds.contains_key(&14));
}

#[test]
fn presets_are_stored_per_account() {
    use accounts::{PresetSlot, MAX_PRESETS, MAX_PRESETS_PREMIUM};
    let s = mem();
    let a = s.create_account("pa", "pw").unwrap();
    let b = s.create_account("pb", "pw").unwrap();
    let slots = [PresetSlot { slot: 2, uid: UID_BASE + 2, rounds: None }, PresetSlot { slot: 8, uid: UID_BASE + 1, rounds: Some(60) }];
    let id1 = s.create_preset(a, " Rush ", &slots, false).unwrap();
    let id2 = s.create_preset(a, "Camp", &[], false).unwrap();
    assert_ne!(id1, id2);
    assert!(s.presets(b).unwrap().is_empty());
    let l = s.presets(a).unwrap();
    assert_eq!((l[0].name.as_str(), l[0].slots.clone()), ("Rush", slots.to_vec()));
    assert!(s.rename_preset(a, id2, "Camper").unwrap());
    assert!(s.save_preset(a, id2, &slots[..1]).unwrap());
    assert!(!s.save_preset(b, id2, &slots[..1]).unwrap(), "another account's preset id");
    assert_eq!(s.presets(a).unwrap()[1].slots.len(), 1);
    assert!(s.create_preset(a, "", &[], false).is_err());
    assert!(s.remove_preset(a, id1).unwrap());
    assert_eq!(s.presets(a).unwrap().len(), 1);
    for i in 0..MAX_PRESETS {
        let _ = s.create_preset(a, &format!("p{i}"), &[], false);
    }
    assert_eq!(s.presets(a).unwrap().len(), MAX_PRESETS);
    assert!(s.create_preset(a, "one more", &[], false).is_err());
    // premium raises the cap to 100 (the client's maxPresetsWithPremium)
    for i in MAX_PRESETS..MAX_PRESETS_PREMIUM {
        s.create_preset(a, &format!("q{i}"), &[], true).unwrap();
    }
    assert_eq!(s.presets(a).unwrap().len(), MAX_PRESETS_PREMIUM);
    assert!(s.create_preset(a, "one more", &[], true).is_err());
    // after premium the extra presets stay; only new ones are refused
    assert!(s.create_preset(a, "again", &[], false).is_err());
    assert_eq!(s.presets(a).unwrap().len(), MAX_PRESETS_PREMIUM);
}

#[test]
fn nicknames_are_at_most_30_characters_login_names_32() {
    let s = mem();
    let a = s.create_account(&"l".repeat(32), "pw").unwrap();
    assert!(s.check_nickname(a, &"n".repeat(30)).is_ok());
    assert!(matches!(s.check_nickname(a, &"n".repeat(31)), Err(accounts::Error::InvalidName("longer than 30 characters"))));
    assert!(s.rename_account(a, &"n".repeat(31)).is_err());
    s.rename_account(a, &"n".repeat(30)).unwrap();
}

#[test]
fn premium_roundtrip_and_v5_migration() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("v5.sqlite");
    let id = {
        let s = file_store(&path);
        let id = s.create_account("prem", "pw").unwrap();
        assert_eq!(s.load_profile(id).unwrap().premium_until, 0);
        s.update_profile(id, |p| {
            p.extend_premium(1_000, 86_400);
            Ok::<_, ()>(())
        })
        .unwrap();
        let p = s.load_profile(id).unwrap();
        assert_eq!(p.premium_until, 87_400);
        assert_eq!(p.premium_left(1_400), 86_000);
        assert_eq!(p.premium_left(90_000), 0);
        // extending a running premium adds to the expiry
        s.update_profile(id, |p| {
            p.extend_premium(2_000, 3_600);
            Ok::<_, ()>(())
        })
        .unwrap();
        assert_eq!(s.load_profile(id).unwrap().premium_until, 91_000);
        // an expired value cleared to 0 leaves no row
        s.update_profile(id, |p| {
            p.premium_until = 0;
            Ok::<_, ()>(())
        })
        .unwrap();
        assert_eq!(s.load_profile(id).unwrap().premium_until, 0);
        s.update_profile(id, |p| {
            p.premium_until = 555;
            Ok::<_, ()>(())
        })
        .unwrap();
        id
    };
    {
        // a v5 file has no premium table
        let c = rusqlite::Connection::open(&path).unwrap();
        c.execute_batch("ALTER TABLE profiles DROP COLUMN onboarding; ALTER TABLE profiles DROP COLUMN face; DROP TABLE league_history; DROP TABLE league; DROP TABLE hunt_entries; DROP TABLE hunt; DROP TABLE battle_rows; DROP TABLE login_calendar; DROP TABLE boosters; ALTER TABLE accounts DROP COLUMN nick; DROP TABLE daily_quest_slots; DROP TABLE daily_quests; DROP TABLE premium; PRAGMA user_version = 5;").unwrap();
    }
    let s = file_store(&path);
    let mut p = s.load_profile(id).unwrap();
    assert_eq!(p.premium_until, 0);
    p.premium_until = 777;
    s.save_profile(id, &p).unwrap();
    assert_eq!(s.load_profile(id).unwrap().premium_until, 777);
}

#[test]
fn daily_quests_roundtrip_and_v7_migration() {
    use accounts::{DailyState, QuestSlot};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("v7.sqlite");
    let want = DailyState {
        issued: 1_000,
        expires: 65_800,
        free_used: 1,
        chest_dict: 213,
        chest_amount: 1,
        next_uid: 9,
        next_msg: 4,
        slots: vec![
            QuestSlot { uid: 5, dict: 0, progress: 1, done: true },
            QuestSlot { uid: 6, dict: 13, progress: 0, done: false },
            QuestSlot { uid: 8, dict: 28, progress: 2, done: false },
        ],
    };
    let id = {
        let s = file_store(&path);
        let id = s.create_account("dq", "pw").unwrap();
        assert_eq!(s.load_profile(id).unwrap().daily, DailyState::default());
        s.update_profile(id, |p| {
            p.daily = want.clone();
            Ok::<_, ()>(())
        })
        .unwrap();
        assert_eq!(s.load_profile(id).unwrap().daily, want);
        // shrinking the set leaves no stale slot row
        s.update_profile(id, |p| {
            p.daily.slots.truncate(1);
            Ok::<_, ()>(())
        })
        .unwrap();
        assert_eq!(s.load_profile(id).unwrap().daily.slots.len(), 1);
        id
    };
    {
        // a v6 file has no daily quest tables
        let c = rusqlite::Connection::open(&path).unwrap();
        c.execute_batch("ALTER TABLE profiles DROP COLUMN onboarding; ALTER TABLE profiles DROP COLUMN face; DROP TABLE league_history; DROP TABLE league; DROP TABLE hunt_entries; DROP TABLE hunt; DROP TABLE battle_rows; DROP TABLE login_calendar; DROP TABLE boosters; ALTER TABLE accounts DROP COLUMN nick; DROP TABLE daily_quest_slots; DROP TABLE daily_quests; PRAGMA user_version = 6;").unwrap();
    }
    let s = file_store(&path);
    let v: i64 = rusqlite::Connection::open(&path).unwrap().query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
    assert_eq!(v, 23);
    let mut p = s.load_profile(id).unwrap();
    assert_eq!(p.daily, DailyState::default());
    p.daily = want.clone();
    s.save_profile(id, &p).unwrap();
    assert_eq!(s.load_profile(id).unwrap().daily, want);
}

#[test]
fn periodic_state_roundtrip_and_v8_migration() {
    use accounts::{BattleRow, CalendarState, HuntEntry, HuntState};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("v8.sqlite");
    let hunt = HuntState {
        period: 1_699_833_600,
        level: 4,
        page: 6,
        free_used: 2,
        entries: vec![HuntEntry { dict: 952, found: true }, HuntEntry { dict: 1221, found: false }, HuntEntry { dict: 214, found: false }],
    };
    let cal = CalendarState { last_day: 19_675, day: 2, received_mask: 0b111 };
    let id = {
        let s = file_store(&path);
        let id = s.create_account("per", "pw").unwrap();
        let p = s.load_profile(id).unwrap();
        assert_eq!(p.hunt, HuntState::default());
        assert!(p.battle.is_empty());
        assert_eq!(p.calendar, CalendarState::default());
        s.update_profile(id, |p| {
            p.hunt = hunt.clone();
            p.battle.insert(3, BattleRow { start: 1_000, reputation: 41_000, received: 3 });
            p.battle.insert(1, BattleRow { start: 1_000, reputation: 5, received: 0 });
            p.calendar = cal;
            Ok::<_, ()>(())
        })
        .unwrap();
        let p = s.load_profile(id).unwrap();
        assert_eq!(p.hunt, hunt);
        assert_eq!(p.battle[&3], BattleRow { start: 1_000, reputation: 41_000, received: 3 });
        assert_eq!(p.battle.len(), 2);
        assert_eq!(p.calendar, cal);
        // a shorter page leaves no stale entry row; a full 64-bit mask survives the signed column
        s.update_profile(id, |p| {
            p.hunt.entries.truncate(1);
            p.calendar.received_mask = u64::MAX;
            Ok::<_, ()>(())
        })
        .unwrap();
        let p = s.load_profile(id).unwrap();
        assert_eq!(p.hunt.entries.len(), 1);
        assert_eq!(p.calendar.received_mask, u64::MAX);
        id
    };
    {
        // a v7 file has none of the periodic tables
        let c = rusqlite::Connection::open(&path).unwrap();
        c.execute_batch("ALTER TABLE profiles DROP COLUMN onboarding; ALTER TABLE profiles DROP COLUMN face; DROP TABLE league_history; DROP TABLE league; DROP TABLE hunt_entries; DROP TABLE hunt; DROP TABLE battle_rows; DROP TABLE login_calendar; DROP TABLE boosters; ALTER TABLE accounts DROP COLUMN nick; PRAGMA user_version = 7;").unwrap();
    }
    let s = file_store(&path);
    let v: i64 = rusqlite::Connection::open(&path).unwrap().query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
    assert_eq!(v, 23);
    let mut p = s.load_profile(id).unwrap();
    assert_eq!(p.hunt, HuntState::default());
    assert!(p.battle.is_empty());
    assert_eq!(p.calendar, CalendarState::default());
    p.hunt = hunt.clone();
    p.calendar = cal;
    s.save_profile(id, &p).unwrap();
    assert_eq!(s.load_profile(id).unwrap().hunt, hunt);
    assert_eq!(s.load_profile(id).unwrap().calendar, cal);
}

#[test]
fn boosters_roundtrip_and_v8_migration() {
    use accounts::BoosterState;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("v8.sqlite");
    let id = {
        let s = file_store(&path);
        let id = s.create_account("boost", "pw").unwrap();
        assert!(s.load_profile(id).unwrap().boosters.is_empty());
        s.update_profile(id, |p| {
            p.boosters.insert(853, BoosterState { enabled: true, until: 90_000, autobuy: false });
            p.boosters.insert(340, BoosterState { enabled: true, until: 0, autobuy: true });
            Ok::<_, ()>(())
        })
        .unwrap();
        let p = s.load_profile(id).unwrap();
        assert_eq!(p.boosters.len(), 2);
        assert_eq!(p.boosters[&340], BoosterState { enabled: true, until: 0, autobuy: true });
        assert!(p.booster_running(853, 89_999) && !p.booster_running(853, 90_000) && !p.booster_running(340, 0));
        id
    };
    {
        // a v8 file has no boosters table and no nickname column
        let c = rusqlite::Connection::open(&path).unwrap();
        c.execute_batch("ALTER TABLE profiles DROP COLUMN onboarding; ALTER TABLE profiles DROP COLUMN face; DROP TABLE league_history; DROP TABLE league; DROP TABLE boosters; ALTER TABLE accounts DROP COLUMN nick; PRAGMA user_version = 8;").unwrap();
    }
    let s = file_store(&path);
    let mut p = s.load_profile(id).unwrap();
    assert!(p.boosters.is_empty());
    p.boosters.insert(861, BoosterState { enabled: true, until: 5, autobuy: false });
    s.save_profile(id, &p).unwrap();
    assert_eq!(s.load_profile(id).unwrap().boosters.len(), 1);
}

#[test]
fn rename_account_rules() {
    let s = mem();
    let a = s.create_account("Alpha", "pw").unwrap();
    let b = s.create_account("Beta", "pw").unwrap();
    assert!(matches!(s.rename_account(a, "beta"), Err(accounts::Error::NameTaken)));
    assert!(s.rename_account(a, " bad").is_err());
    s.rename_account(a, "ALPHA").unwrap(); // own name in another case
    s.rename_account(a, "Gamma").unwrap();
    // only the nickname changed: the login name still works, the nickname is what is shown
    assert_eq!(s.account(a).unwrap().unwrap().name, "Gamma");
    assert_eq!(s.find_account("Alpha").unwrap().unwrap().id, a);
    assert!(s.find_account("Gamma").unwrap().is_none());
    assert!(s.verify_login("Alpha", "pw").is_ok());
    assert!(s.verify_login("Gamma", "pw").is_err());
    assert_eq!(s.search_accounts("gam", 5, None).unwrap().len(), 1);
    assert!(matches!(s.create_account("gamma", "pw"), Err(accounts::Error::NameTaken)));
    assert!(matches!(s.rename_account(b, "GAMMA"), Err(accounts::Error::NameTaken)));
    assert!(matches!(s.rename_account(999, "Nobody"), Err(accounts::Error::NoSuchAccount)));
}

#[test]
fn league_state_round_trips_and_a_v9_file_migrates() {
    use accounts::{LeagueState, SeasonRecord};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("l.sqlite");
    let (a, b, c);
    {
        let s = file_store(&path);
        a = s.create_account("alpha", "pw").unwrap();
        b = s.create_account("bravo", "pw").unwrap();
        c = s.create_account("charlie", "pw").unwrap();
        let st = |league, points| LeagueState { season: 4, league, points, max_league: league, matches: 3, tokens: 0, history: vec![] };
        s.update_profile(a, |p| Ok::<_, ()>(p.league = LeagueState { tokens: 500, history: vec![SeasonRecord { season: 3, league: 5, max_league: 6, tokens: 900, closed_at: 1_700_000_000, notified: true }], ..st(2, 7) })).unwrap();
        s.update_profile(b, |p| Ok::<_, ()>(p.league = st(2, 9))).unwrap();
        s.update_profile(c, |p| Ok::<_, ()>(p.league = st(1, 11))).unwrap();
        let l = s.load_profile(a).unwrap().league;
        assert_eq!((l.season, l.league, l.points, l.tokens, l.history.len()), (4, 2, 7, 500, 1));
        assert!(l.history[0].notified && l.history[0].max_league == 6);
        // ladder: league, then points
        let rows = s.league_ladder(4, 10).unwrap();
        assert_eq!(rows.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(), ["bravo", "alpha", "charlie"]);
        assert!(s.league_ladder(3, 10).unwrap().is_empty(), "another season");
        assert_eq!(s.league_ladder(4, 2).unwrap().len(), 2);
        assert_eq!((s.league_place(a, 4).unwrap(), s.league_place(c, 4).unwrap(), s.league_place(a, 5).unwrap()), (Some(2), Some(3), None));
        assert_eq!(s.league_rows_of(&[a, c], 4).unwrap().len(), 2);
        assert_eq!(s.league_history(a).unwrap().len(), 1);
        // Currency::League is the token balance
        s.update_profile(a, |p| p.spend(Currency::League, 200)).unwrap();
        assert_eq!(s.load_profile(a).unwrap().league.tokens, 300);
        assert!(s.update_profile(a, |p| p.spend(Currency::League, 301)).is_err());
        // an empty state writes no row; deleting the account drops its rows
        s.update_profile(c, |p| Ok::<_, ()>(p.league = LeagueState::default())).unwrap();
        assert_eq!(s.league_ladder(4, 10).unwrap().len(), 2);
    }
    {
        // a v9 file has neither league table
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch("ALTER TABLE profiles DROP COLUMN onboarding; ALTER TABLE profiles DROP COLUMN face; DROP TABLE league_history; DROP TABLE league; PRAGMA user_version = 9;").unwrap();
    }
    let s = file_store(&path);
    assert_eq!(s.load_profile(a).unwrap().league, LeagueState::default());
    s.update_profile(a, |p| Ok::<_, ()>(p.league.tokens = 5)).unwrap();
    assert_eq!(s.load_profile(a).unwrap().league.tokens, 5);
    let v: i64 = rusqlite::Connection::open(&path).unwrap().query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
    assert_eq!(v, 23);
}

#[test]
fn onboarding_and_face_round_trip_and_a_v10_file_migrates_past_onboarding() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("o.sqlite");
    let (old, fresh);
    {
        let s = file_store(&path);
        old = s.create_account("veteran", "pw").unwrap();
        // a new player's profile (onboarding 0) gets no nickname until it chooses one
        let mut p = accounts::Profile::starter();
        p.onboarding = 0;
        fresh = s.create_account_with_profile("newcomer", "pw", &p).unwrap();
        assert!(s.has_nickname(old).unwrap() && !s.has_nickname(fresh).unwrap());
        s.update_profile(fresh, |p| Ok::<_, ()>((p.onboarding, p.face) = (0x80, 9))).unwrap();
        let p = s.load_profile(fresh).unwrap();
        assert_eq!((p.onboarding, p.face), (0x80, 9));
    }
    {
        // a v10 file: no onboarding / face columns, nicknames only from the rename voucher
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch("ALTER TABLE profiles DROP COLUMN onboarding; ALTER TABLE profiles DROP COLUMN face; UPDATE accounts SET nick = NULL; PRAGMA user_version = 10;").unwrap();
    }
    let s = file_store(&path);
    // every account of a v10 file played in the full lobby: past onboarding, its login name is its nickname
    for id in [old, fresh] {
        let p = s.load_profile(id).unwrap();
        assert_eq!((p.onboarding, p.face), (accounts::Profile::PAST_ONBOARDING, 0));
        assert!(s.has_nickname(id).unwrap());
    }
    assert_eq!(s.search_accounts("newcom", 10, None).unwrap().len(), 1);
    let v: i64 = rusqlite::Connection::open(&path).unwrap().query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
    assert_eq!(v, 23);
}

#[test]
fn equipment_profiles_and_skins_round_trip_and_switch() {
    let s = mem();
    let id = s.create_account("eq", "pw12345678").unwrap();
    let mut p = Profile::starter();
    for e in p.equip.profiles.iter_mut() {
        e.owned = true;
    }
    p.equip.profiles[1].name = "Sniper".into();
    p.equip.profiles[2].enabled = false;
    // profile 0 is current and wears the starter set; profile 1 holds the Mosin in slot 7 and 30 rounds in slot 8
    p.equip.profiles[1].slots.insert(7, UID_BASE);
    p.equip.profiles[1].rounds.insert(8, 30);
    p.skins.insert(1240);
    s.save_profile(id, &p).unwrap();
    let mut q = s.load_profile(id).unwrap();
    assert_eq!(q.equip, p.equip);
    assert_eq!(q.skins, p.skins);
    assert_eq!(q.slots, p.slots);
    // switching stashes the worn items and brings the target's out; it survives a save
    assert!(q.switch_profile(1));
    assert_eq!(q.slots, BTreeMap::from([(7, UID_BASE)]));
    assert_eq!(q.equip.profiles[0].slots, p.slots);
    s.save_profile(id, &q).unwrap();
    let r = s.load_profile(id).unwrap();
    assert_eq!((r.equip.current, &r.slots), (1, &q.slots));
    assert_eq!(r.equip.profiles[0].slots, p.slots);
    // removing an item takes it out of every stash
    let mut r = r;
    r.remove_item(UID_BASE + 2);
    assert!(!r.equip.profiles[0].slots.values().any(|u| *u == UID_BASE + 2));
    // a default table writes nothing and loads back as default
    let id2 = s.create_account("eq2", "pw12345678").unwrap();
    s.save_profile(id2, &Profile::starter()).unwrap();
    assert_eq!(s.load_profile(id2).unwrap().equip, accounts::equip::Equip::default());
}

#[test]
fn a_player_without_a_nickname_is_shown_under_a_neutral_name_never_the_login_name() {
    let s = mem();
    let mut fresh = Profile::starter();
    fresh.onboarding = 0;
    let a = s.create_account_with_profile("secret_login_a", "pw", &fresh).unwrap();
    let b = s.create_account("visible_b", "pw").unwrap();
    let neutral = format!("Player{a}");
    assert_eq!(s.account(a).unwrap().unwrap().name, neutral);
    assert_eq!(s.find_account("secret_login_a").unwrap().unwrap().name, neutral);
    s.friend_request(b, a).unwrap();
    s.friend_accept(a, b).unwrap();
    assert!(s.friends(b).unwrap().iter().any(|f| f.id == a && f.name == neutral));
    assert!(s.friends(a).unwrap().iter().all(|f| f.name != "secret_login_a"));
    // after the nickname is chosen the nickname shows
    s.rename_account(a, "Chosen").unwrap();
    assert_eq!(s.account(a).unwrap().unwrap().name, "Chosen");
}

#[test]
fn consuming_a_shared_stack_caps_the_rounds_of_every_profile() {
    let mut p = Profile::starter();
    let ammo = *p.slots.get(&8).expect("the starter has ammo in slot 8");
    let count = p.item(ammo).unwrap().count;
    assert!(count > 10);
    // profile 1 (not current) holds the same stack with all its rounds
    p.equip.profiles[1].slots.insert(8, ammo);
    p.equip.profiles[1].rounds.insert(8, count);
    p.consume(ammo, count - 10).unwrap();
    assert_eq!(p.slot_rounds[&8], 10);
    assert_eq!(p.equip.profiles[1].rounds[&8], 10, "the other profile cannot load more than the stack holds");
}

#[test]
fn the_clan_registration_takes_the_silver_and_marks_it_paid_in_one_step() {
    let s = mem();
    let a = s.create_account("founder", "pw").unwrap();
    // nothing reserved: nothing is taken
    let silver = s.load_profile(a).unwrap().silver;
    assert!(s.pay_clan_registration(a, 100).is_err());
    assert_eq!(s.load_profile(a).unwrap().silver, silver);
    s.reserve_clan(a, "Wolves", "WLF").unwrap();
    // too poor: silver and the paid mark stay as they were
    assert!(s.pay_clan_registration(a, u64::from(silver) + 1).is_err());
    assert_eq!(s.load_profile(a).unwrap().silver, silver);
    assert_eq!(s.clan_reservation(a).unwrap().unwrap().2, false);
    s.pay_clan_registration(a, 100).unwrap();
    assert_eq!(s.load_profile(a).unwrap().silver, silver - 100);
    assert_eq!(s.clan_reservation(a).unwrap().unwrap().2, true);
    // a second payment takes nothing
    assert!(s.pay_clan_registration(a, 100).is_err());
    assert_eq!(s.load_profile(a).unwrap().silver, silver - 100);
}

#[test]
fn a_claim_keeps_the_time_it_was_first_made_across_saves() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("c.sqlite");
    let s = file_store(&path);
    let id = s.create_account("claimer", "pw").unwrap();
    s.update_profile(id, |p| Ok::<_, ()>(p.claim("first"))).unwrap();
    let c = rusqlite::Connection::open(&path).unwrap();
    c.execute("UPDATE claims SET claimed_at = 100 WHERE account_id = ?1", [id]).unwrap();
    // an unrelated save and a second claim: the first keeps 100, the new one gets the current time
    s.update_profile(id, |p| {
        p.silver += 1;
        Ok::<_, ()>(p.claim("second"))
    })
    .unwrap();
    let at = |name: &str| -> i64 { c.query_row("SELECT claimed_at FROM claims WHERE account_id = ?1 AND name = ?2", rusqlite::params![id, name], |r| r.get(0)).unwrap() };
    assert_eq!(at("first"), 100);
    assert!(at("second") > 100);
}

#[test]
fn clans_get_the_four_standard_ranks_and_old_default_names_migrate() {
    use accounts::clans::{DEFAULT_RANKS, JOIN_RANK};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ranks.sqlite");
    let (a, cid) = {
        let s = file_store(&path);
        let a = s.create_account("chief", "password1").unwrap();
        s.reserve_clan(a, "Iron Wolves", "IW").unwrap();
        s.mark_clan_registration_paid(a).unwrap();
        let cid = s.create_clan(a).unwrap();
        let c = s.load_clan(cid).unwrap().unwrap();
        // the client's own standard names (wire default = 1): Commander / Deputy commander / Sergeant / Soldier
        let names: Vec<(&str, bool)> = c.ranks.iter().map(|r| (r.name.as_str(), r.standard)).collect();
        assert_eq!(names, vec![("commander", true), ("assistant", true), ("warlord", true), ("soldier", true)]);
        assert_eq!(names.len(), DEFAULT_RANKS.len());
        assert_eq!(c.ranks[c.default_rank() as usize].name, JOIN_RANK);
        // a custom rank is not standard and can be removed; a standard one cannot
        s.clan_create_rank(a, "Scout").unwrap();
        assert!(!s.load_clan(cid).unwrap().unwrap().ranks[4].standard);
        assert!(s.clan_remove_rank(a, 3).is_err(), "soldier is standard");
        s.clan_remove_rank(a, 4).unwrap();
        (a, cid)
    };
    // an old database (v20): the three old default names, only Member marked
    {
        let c = rusqlite::Connection::open(&path).unwrap();
        c.execute_batch(&format!(
            "DELETE FROM clan_ranks WHERE clan_id = {cid};
             INSERT INTO clan_ranks (clan_id, idx, name, perms, is_default) VALUES ({cid}, 0, 'Commander', 31, 0), ({cid}, 1, 'Officer', 26, 0), ({cid}, 2, 'Member', 0, 1), ({cid}, 3, 'Scout', 0, 0);
             PRAGMA user_version = 20;"
        ))
        .unwrap();
    }
    let s = file_store(&path);
    let c = s.clan_of(a).unwrap().unwrap();
    let names: Vec<(&str, bool)> = c.ranks.iter().map(|r| (r.name.as_str(), r.standard)).collect();
    assert_eq!(names, vec![("commander", true), ("assistant", true), ("soldier", true), ("Scout", false)]);
    assert_eq!(c.default_rank(), 2, "new members still join at soldier");
}
