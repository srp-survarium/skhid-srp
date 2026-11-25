//! Lobby tests with scripted clients (client-role transports, virtual clock): login pushes, shop, equip, workshop,
//! modules, factions, small answers, local match hook, several clients, restart.

mod support;

use std::sync::Arc;
use std::time::Duration;

use accounts::{Currency, UID_BASE};
use game_data::GameData;
use lobby_server::{LobbyConfig, MinimalMatches};
use protocol::c2s::{EquipEntry, Event53, Friends4E, Inventory45 as Q45, ModuleAction, Play5B, Pve5F, Quest4A, Request, Shop46 as Q46, Squad49, Workshop60};
use protocol::items::ItemRecord;
use protocol::s2c::*;
use support::*;

fn gd() -> GameData {
    game_data::testing::tables()
}

fn env() -> (tempfile::TempDir, Env) {
    let dir = tempfile::tempdir().unwrap();
    let e = Env::new(&dir.path().join("srp.sqlite"));
    (dir, e)
}

fn db(dir: &tempfile::TempDir) -> std::path::PathBuf {
    dir.path().join("srp.sqlite")
}

fn ids(msgs: &[S2c]) -> Vec<u8> {
    msgs.iter().map(|m| m.id()).collect()
}

fn replies(msgs: &[S2c]) -> Vec<&Reply> {
    msgs.iter().filter_map(|m| if let S2c::Reply(r) = m { Some(r) } else { None }).collect()
}

fn money(msgs: &[S2c]) -> Option<&Money> {
    msgs.iter().rev().find_map(|m| if let S2c::Money(x) = m { Some(x) } else { None })
}

fn account87(msgs: &[S2c]) -> Option<&Account87> {
    msgs.iter().rev().find_map(|m| if let S2c::Account(x) = m { Some(x) } else { None })
}

// ---- login ----

#[test]
fn login_pushes_in_the_proven_order() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    c.send(Request::SignIn { session: c.session, a: 0xFF, b: 0xFF });
    for _ in 0..300 {
        e.step(&mut [&mut c], Duration::from_millis(30));
    }
    // 80 first (alone until acked), then the login pushes: 91 first, then 8F, 8B, 9D, 89, 87 ...
    assert_eq!(
        c.ids(),
        [0x80, 0x91, 0x8F, 0x8B, 0x9D, 0x89, 0x87, 0xB0, 0x9A, 0x85, 0x9F, 0x8A, 0x96, 0x8E, 0x8D, 0x95, 0x94, 0x8C, 0x9C, 0x9E, 0xAC, 0xB8, 0xB9, 0xAB, 0xAA, 0xA6]
    );
    let msgs = c.take();
    let by_id = |id: u8| msgs.iter().find(|m| m.id() == id).unwrap();
    // the pocket consumables are gated like the client gates them: nothing bought yet
    assert_eq!(by_id(0x91), &S2c::SpecificsOpened(SpecificsOpened { mask: 0 }));
    assert_eq!(by_id(0x8F), &S2c::Name(NameMsg { flag: 0, name: "test".into() }));
    assert_eq!(by_id(0xAC), &S2c::PlayGate(PlayGate { value: 60, mode_mask: 0, waits: vec![] }));
    assert_eq!(by_id(0xB8), &S2c::EventIds(EventIds { tag: 0, ids: vec![5, 6, 0x3D, 0x3E] }));
    let S2c::ServicePrices(p) = by_id(0x8D) else { panic!() };
    assert_eq!(p.exchange_rate, 250);
    assert_eq!(p.sell_mult, [1.0; 4]);
}

/// The shop locks: the client handles the sign-in messages in arrival order; `set_prices` (8A)
/// makes the shop items and marks a lock only on items up to the best faction level, `setup_player_progress` (8E) locks
/// every made item its faction level does not open. So 8A must come before the 8E (and the 96 that also runs the
/// progress call), and the 8E carries the stored points: an account at faction level 1 (points < 5000) has level-2 items.
#[test]
fn sign_in_sends_the_shop_items_before_the_faction_levels_that_lock_them() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    e.store
        .update_profile(c.account, |p| {
            p.fac_points = (1..=4).map(|f| (f, if f == 1 { 1265 } else { 0 })).collect();
            Ok::<_, ()>(())
        })
        .unwrap();
    let msgs = e.sign_in(&mut c);
    let at = |id: u8| msgs.iter().position(|m| m.id() == id).unwrap_or_else(|| panic!("no {id:#04x}"));
    assert!(at(0x8A) < at(0x8E), "8A after 8E: the shop items miss the lock of their faction level: {:02x?}", ids(&msgs));
    assert!(at(0x8A) < at(0x96), "8A after 96: {:02x?}", ids(&msgs));
    let S2c::Factions(f) = &msgs[at(0x8E)] else { panic!() };
    assert_eq!(f.factions.iter().map(|r| (r.faction, r.points)).collect::<Vec<_>>(), [(0, 0), (1, 1265), (2, 0), (3, 0), (4, 0)]);
    // the client's 91 handler (0x2002470B8) and AB reader (0x200188398) take record f as faction f
    for (i, r) in f.factions.iter().enumerate() {
        assert_eq!(r.faction as usize, i, "8E record {i} must be faction {i}");
    }
    assert_eq!(game_data::levels::faction_level(1265), 1);
    // the 8A lists faction items above that level for sale (the ones the 8E must lock)
    let g = gd();
    let S2c::Prices(pr) = &msgs[at(0x8A)] else { panic!() };
    assert!(pr.records.iter().any(|r| r.e != 0 && g.items.get(r.dict).is_some_and(|it| it.faction == 1 && it.level_min1() >= 2)));
}

/// The reference vectors (crates/protocol/tests/vectors/s2c_pushes.json, default state, user name "test") decode to the
/// messages the Rust lobby sends for the starter profile, after the patches below for the rules the vectors predate.
#[test]
fn login_pushes_equal_the_reference_vectors() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    let msgs = e.sign_in(&mut c);
    let ref_msg = |name: &str| {
        let (id, mut pl) = vector("s2c_pushes.json", name);
        if id == 0xAC && pl.len() == 8 {
            pl.push(0); // the vector's AC lacks the server-row count the client always reads (0x20018AEC0)
        }
        let mut m = S2c::decode(id, &pl).unwrap();
        // the vector has skill points = level; the rule is level - 1 (progression.md 2)
        match &mut m {
            S2c::Account(a) => a.level.points -= 1,
            S2c::Level(l) => l.level.points -= 1,
            // the vector opens every pocket consumable (0x7FFFF); the server gates them like the client (nothing bought yet)
            S2c::SpecificsOpened(s) => s.mask = 0,
            // the vector has factions 1..4; the client reads record f as faction f, so the server adds record 0 (build::factions_8e)
            S2c::Factions(f) => f.factions.insert(0, FactionRec { faction: 0, unlocked: 0, points: 0 }),
            _ => {}
        }
        m
    };
    for (id, name) in [
        (0x91, "push_91_default"),
        (0x8F, "push_8F_default"),
        (0x8B, "push_8B_default"),
        (0x9D, "push_9D_default"),
        (0x89, "push_89_default"),
        (0x87, "push_87_default"),
        (0xB0, "push_B0_default"),
        (0x85, "push_85_default"),
        (0x96, "push_96_default"),
        (0x8E, "push_8E_default"),
        (0x94, "push_94_default"),
        (0x8C, "push_8C_default"),
        (0x9C, "push_9C_default"),
        (0x9E, "push_9E_default"),
        (0xAC, "push_AC_default"),
        (0xB8, "push_B8_default"),
    ] {
        let mut sent = msgs.iter().find(|m| m.id() == id).unwrap_or_else(|| panic!("no {id:#04x}")).clone();
        // the vector's seeded modules are grey (no pairs); the server stores them green (workshop.md section 7)
        let items = match &mut sent {
            S2c::Inventory(inv) => Some(&mut inv.items),
            S2c::Account(a) => Some(&mut a.body.items),
            _ => None,
        };
        for it in items.into_iter().flatten().filter(|i| gd().items.is_module(i.dict)) {
            assert_eq!(it.mods.first(), Some(&(312, 1)), "{name}: seeded module {} is green", it.dict);
            it.mods.clear();
        }
        if let S2c::Account(a) = &mut sent {
            // the vector has one profile "p"; the account lists all three (profiles.md): the first one must equal it
            // apart from the default tab name, the other two are locked buy slots
            assert_eq!(a.body.profiles.iter().map(|p| p.kind).collect::<Vec<_>>(), [0, 2, 2]);
            // the version is the content version (build::wire_version: a hash, never 0 / FFFFFFFF), not the vector's counter
            let want_v = match lift(ref_msg(name)) {
                S2c::Account(r) => r.profile_state[0].0,
                _ => unreachable!(),
            };
            for s in a.profile_state.iter_mut() {
                assert!(s.0 != 0 && s.0 != u32::MAX);
                s.0 = want_v;
            }
            a.body.profiles.truncate(1);
            a.body.profiles[0].name = "p".into();
            a.profile_state.truncate(1);
        }
        assert_eq!(&sent, &lift(ref_msg(name)), "{name}");
    }
    // 95: the vector push_95_default is empty; the default store carries the premium window's four offers and the
    // store tab of the two action cards (tests/store_offers.rs)
    let S2c::Shop(shop) = msgs.iter().find(|m| m.id() == 0x95).unwrap() else { panic!("95 not decoded") };
    assert_eq!(shop.elems.iter().map(|x| (x.f0, x.dict, x.fc)).collect::<Vec<_>>(), [(1, 1, 150), (2, 2, 300), (3, 3, 900), (4, 4, 2500)]);
    // the client shows tab names raw: the English text of the client's own key st_special_offer
    assert_eq!(shop.tabs, vec![ShopTab { id: 1, name: "Special offer".into(), flag: 1 }]);
    // 9F: the vector push_9F_default is the empty "no league" payload. The league window (league.rs,
    // tests/league.rs): the monthly season runs by default (S = its number, flag 1, the player's state), no final
    // notice, plus the level table from the game data.
    let S2c::League(League9F::Info(i)) = msgs.iter().find(|m| m.id() == 0x9F).unwrap() else { panic!() };
    assert_eq!((i.a, i.flag, i.state.is_some()), (0xFF, 1, true));
    assert_ne!(i.s, 0);
    assert_eq!(i.levels.len(), 10);
    // 8A: the reference push (game prices, so not in the repository: `push_8A_default.json` = {"id", "payload"} in the
    // test tables folder) holds 30 records, every one equal to ours (ours lists the whole dictionary)
    let S2c::Prices(got) = msgs.iter().find(|m| m.id() == 0x8A).unwrap() else { panic!() };
    let want = game_data::testing::reference("push_8A_default.json").map(|text| {
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        let pl = unhex(v["payload"].as_str().unwrap());
        let S2c::Prices(want) = S2c::decode(v["id"].as_u64().unwrap() as u8, &pl).unwrap() else { panic!() };
        want
    });
    if want.is_none() {
        eprintln!("skipped the 8A comparison: no push_8A_default.json in the test tables folder");
    }
    let want_records = want.map(|w| w.records).unwrap_or_default();
    assert!(want_records.is_empty() || want_records.len() >= 30);
    for r in &want_records {
        if [5u16, 66, 67, 219, 734, 736].contains(&r.dict) || gd().shop.is_choice(r.dict) {
            continue; // priced from the fan-wiki Items page or our own CHOICE (docs/notes/store.md section 5), not the vector's placeholder
        }
        let mut want = *r;
        // consumables without a real price are not for sale (e = 0); the vector lists them at a placeholder price
        if gd().items.is_consumable(r.dict) && !gd().shop.price(r.dict).unwrap().for_sale {
            want.e = 0;
        }
        // ammo is free (CHOICE): listed (the ammo window needs the shop row), price and sell value 0
        if gd().items.is_ammo(r.dict) {
            (want.a, want.b) = (0, 0);
        }
        assert_eq!(got.records.iter().find(|g| g.dict == r.dict), Some(&want), "8A dict {}", r.dict);
    }
    // 8D differs only in the exchange rate (vector 0, here 250: the exchange window divides by it) and the skill reset
    // prices (costs 0 and 1; the vector has 0)
    let (id, pl) = vector("s2c_pushes.json", "push_8D_default");
    let S2c::ServicePrices(mut want) = S2c::decode(id, &pl).unwrap() else { panic!() };
    want.exchange_rate = 250;
    want.costs[0] = 1000;
    want.costs[1] = 5;
    // slot 4: clan creation price (social area)
    want.costs[4] = lobby_server::clans::CREATION_COST_SILVER;
    // slot 3: the gold rename price = the rename voucher's price (CHOICE)
    want.costs[3] = 250;
    assert_eq!(msgs.iter().find(|m| m.id() == 0x8D).unwrap(), &S2c::ServicePrices(want));
}

#[test]
fn unknown_session_is_ignored_and_the_second_sign_in_replaces_the_first() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut bad = e.client("ghost");
    bad.session ^= 0x55AA55AA;
    bad.send(Request::SignIn { session: bad.session, a: 0xFF, b: 0xFF });
    e.pump(&mut [&mut bad]);
    assert!(bad.inbox.is_empty(), "no 80 for an unknown session");

    let mut a1 = e.client("dup");
    let mut a2 = e.client_of("dup");
    e.sign_in(&mut a1);
    assert_eq!(e.server.registry.len(), 1);
    let m = e.sign_in(&mut a2);
    assert!(ids(&m).contains(&0x87));
    assert_eq!(e.server.registry.addr_of(a2.account), Some(a2.addr));
    assert!(!e.server.peers.contains_key(&a1.addr), "the old connection is dropped");
}

#[test]
fn handshake_answers() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    e.sign_in(&mut c);
    let m = e.handshake(&mut c);
    // 83 5B 02 <size 0>, 9D A=1, 9B chances, 91 again, B0 05 friends (no welcome chat line: AF is not localized)
    assert_eq!(ids(&m), [0x83, 0x9D, 0x9B, 0x91, 0xBB, 0xAE, 0xB0]);
    assert_eq!(m[0], S2c::Reply(Reply::Raw { request: 0x5B, data: vec![2, 0, 0, 0, 0] }));
    assert_eq!(m[1], S2c::TutorialGate(TutorialGate { a: 1, flags: 0x4E2, g: 0, faction: 1, h: 0 }));
    assert_eq!(m[2], S2c::CraftChances(CraftChances { regular: 50.0, gold: 100.0 }));
    assert_eq!(m[3], S2c::SpecificsOpened(SpecificsOpened { mask: 0 }));
    assert_eq!(m[4], S2c::Presets(PresetsBB::default()));
    assert_eq!(m[5], S2c::Squad(SquadUpdate { squad: None }), "the squad state, also without a squad (the PvE window needs it)");
    assert_eq!(m[6], S2c::Messaging(Messaging::Friends(vec![])));
}

// ---- small answers ----

#[test]
fn small_answers() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    let other = e.client("other");
    e.sign_in(&mut c);
    // 48 04 name valid
    let mut nick = vec![4u8, 4];
    nick.extend(b"Nick");
    nick.extend(3u32.to_le_bytes());
    c.send_raw(0x48, &nick);
    // 48 06 comment
    let mut com = vec![6u8];
    com.extend(accounts::public_id(other.account).to_le_bytes());
    com.extend([1, 0, 0, 0, 0, 0]);
    c.send_raw(0x48, &com);
    // 55 04, 4B 01
    c.send_raw(0x55, &[4]);
    c.send_raw(0x4B, &[1]);
    // 62 23 and 48 28 flag clearers
    c.send_raw(0x62, &[0x23]);
    c.send_raw(0x48, &[0x28, 0, 0, 0, 0]);
    e.pump(&mut [&mut c]);
    let m = c.take();
    let want = [
        (0x48u8, "04044e69636b01"),
        (0x48, "0601"),
        (0x55, "02"),
        (0x55, "0303"),
        (0x4B, "010000"),
        (0x62, "ff"),
        (0x48, "03"),
    ];
    let got: Vec<(u8, String)> = m
        .iter()
        // 62 23 (custom match start) is also refused with a text (84 62), see clans_rooms.rs
        .filter(|x| !matches!(x, S2c::Denied(d) if d.request == 0x62))
        .map(|x| match x {
            S2c::Reply(Reply::Raw { request, data }) => (*request, data.iter().map(|b| format!("{b:02x}")).collect()),
            S2c::Reply(Reply::Account(Account48::Nickname { name, valid, .. })) => {
                (0x48, format!("04{:02x}{}{:02x}", name.len(), name.bytes().map(|b| format!("{b:02x}")).collect::<String>(), valid))
            }
            S2c::Reply(Reply::Account(Account48::Comment { kind })) => (0x48, format!("06{kind:02x}")),
            S2c::Reply(Reply::Account(Account48::Other { sub, .. })) => (0x48, format!("{sub:02x}")),
            S2c::Reply(Reply::Achievements(Achievements55::List { entries })) => (0x55, format!("03{:02x}", entries.len())),
            S2c::Reply(Reply::Achievements(Achievements55::Other { sub, .. })) => (0x55, format!("{sub:02x}")),
            S2c::Reply(Reply::League(_)) => (0x4B, "010000".into()),
            other => panic!("unexpected {other:?}"),
        })
        .collect();
    let want: Vec<(u8, String)> = want.iter().map(|(a, b)| (*a, b.to_string())).collect();
    assert_eq!(got, want);
}

#[test]
fn fixed_replies_match_the_reference_vectors() {
    use lobby_server::dispatch::fixed_reply;
    // 4B 01 / 04 / 05 are real league answers (league.rs, tests/league.rs), not fixed replies; 48 06 is stored and
    // answered by comments.rs (tests/gaps.rs), its answer keeps the vector's bytes
    {
        use protocol::Encode;
        let (id, pl) = vector("s2c_replies.json", "REPLIES_48_06");
        assert_eq!(id, 0x83);
        assert_eq!(pl[0], 0x48);
        assert_eq!(fixed_reply(0x48, &[6, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0]), None);
        assert_eq!(Reply::Account(Account48::Comment { kind: 1 }).to_bytes().unwrap(), pl, "REPLIES_48_06");
    }
    // 48 04 is answered by rename::validate (tests/onboarding.rs); its "valid" reply keeps the vector's bytes
    {
        use protocol::Encode;
        let (_, pl) = vector("s2c_replies.json", "REPLIES_48_04");
        let real = Reply::Account(Account48::Nickname { name: "Nick".into(), valid: 1, reason: 0, suggestions: vec![] });
        assert_eq!(real.to_bytes().unwrap(), pl, "REPLIES_48_04");
        assert_eq!(fixed_reply(0x48, &[4, 4, b'N', b'i', b'c', b'k', 3, 0, 0, 0]), None);
    }
    // 5B 03 is answered by play::shooting_range ( tests/lobby.rs shooting_range_is_a_local_match_that_pays_nothing)
    assert_eq!(fixed_reply(0x5B, &[3]), None);
    assert_eq!(fixed_reply(0x5B, &[1]), None);
    assert_eq!(fixed_reply(0x2F, &[0]), None);
}

#[test]
fn queue_requests_and_squad_actions() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    e.sign_in(&mut c);
    c.send(Request::Queue { global: 0, arg: 0, mode: 1, flag: 0 });
    c.send(Request::QueueCancel { order: 0 });
    c.send(Request::Pve(Pve5F::Top100));
    c.send(Request::Squad(Squad49::Leave));
    e.pump(&mut [&mut c]);
    let m = c.take();
    // 2F: a real search (83 2F with the order id and no estimate yet: one arrival gives no rate, 85 01), 31: out of the
    // queue again (85 00), the PvE top 100 is answered with C2 03 (an empty list, nothing was ever won)
    assert_eq!(m[0], S2c::Reply(Reply::Queue { a: 1, eta: 0 }));
    assert_eq!(m[1], S2c::Raw { id: 0x85, payload: vec![1, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0] }, "order 1, place 1 (0 = match found), 1 waiting");
    assert_eq!(
        m[2..],
        [
            S2c::QueueState(QueueState { state: 0 }),
            S2c::Raw { id: 0xC2, payload: vec![3, 0, 0xFF, 0xFF, 0xFF, 0xFF] },
            S2c::Squad(SquadUpdate { squad: None }),
        ]
    );
}

#[test]
fn faction_change_pushes_8e_and_is_saved() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    e.sign_in(&mut c);
    c.send(Request::Quest(Quest4A::SetFaction { faction: 3 }));
    c.send(Request::Quest(Quest4A::SetFaction { faction: 9 }));
    e.pump(&mut [&mut c]);
    let m = c.take();
    assert_eq!(m.len(), 1, "an invalid faction is refused without an answer");
    let S2c::Factions(f) = &m[0] else { panic!("{m:?}") };
    assert_eq!(f.active, 3);
    assert_eq!(f.factions.len(), 5, "faction-0 record + the four factions");
    assert_eq!(e.store.load_profile(c.account).unwrap().faction, 3);
}

// ---- shop ----

fn buy_req(dict: u16, count: u32) -> Request {
    Request::Shop(Q46::Buy { dict, count, slot: 100, profile: 1, flag: 0 })
}

#[test]
fn buy_replies_equal_the_reference_vectors_and_charge() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    e.sign_in(&mut c);
    let data = gd();
    let before = e.store.load_profile(c.account).unwrap();
    for (req_name, rep_name, dict) in [("buy_dict3", "buy_dict3#0", 3u16)] {
        let (id, pl) = vector("c2s_handlers.json", req_name);
        c.send_raw(id, &pl);
        e.pump(&mut [&mut c]);
        let m = c.take();
        let (rid, rpl) = vector("s2c_replies.json", rep_name);
        // the vector has no default modules; a weapon record carries them (uid 0xFFFFFFFF)
        let mut want = lift(S2c::decode(rid, &rpl).unwrap());
        if let S2c::Reply(Reply::Shop(Shop46::Buy { items, .. })) = &mut want {
            for it in items.iter_mut() {
                for (slot, md) in data.modules.default_modules(it.dict) {
                    it.modules.push(protocol::items::ModuleElem { dict: *md, uid: u32::MAX, f24: 0, slot: slot.clone(), mods: vec![] });
                }
                // written parents first (the slot set's order)
                let keys: Vec<(&str, u16)> = it.modules.iter().map(|e| (e.slot.as_str(), e.dict)).collect();
                let order = data.modules.record_order(it.dict, &keys);
                it.modules = order.into_iter().map(|i| it.modules[i].clone()).collect();
            }
        }
        assert!(!data.modules.default_modules(dict).is_empty(), "dict {dict} has default modules");
        assert_eq!(m[0], want, "{rep_name}");
        assert!(matches!(m[1], S2c::Money(_)));
        let price = data.shop.price(dict).unwrap().a;
        let _ = price;
    }
    let after = e.store.load_profile(c.account).unwrap();
    assert_eq!(after.items.len(), before.items.len() + 1);
    let spent = before.silver - after.silver;
    // dict 5 (RGD-5): the Items-page price 30, not the vector's placeholder
    assert_eq!(data.shop.buy_total(&data.items, 5, 3), Some(90));
    assert_eq!(spent as u64, data.shop.buy_total(&data.items, 3, 1).unwrap());
}

#[test]
fn buy_refused_without_money_or_price_and_stacks_merge() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    e.sign_in(&mut c);
    let data = gd();
    // unknown dict: refused as `00 00 00` + 8B + the `84 46` text, nothing charged
    c.send(buy_req(60000, 1));
    e.pump(&mut [&mut c]);
    let m = c.take();
    assert_eq!(m[0], S2c::Reply(Reply::Shop(Shop46::Buy { flag: 0, items: vec![] })));
    assert!(matches!(m[1], S2c::Money(_)));
    assert_eq!(m[2], buy_refusal("st_cannot_buy_item"));
    // no silver left: refused
    e.store.update_profile(c.account, |p| { p.silver = 0; Ok::<_, ()>(()) }).unwrap();
    c.send(buy_req(3, 1));
    e.pump(&mut [&mut c]);
    let m = c.take();
    assert_eq!(m[0], S2c::Reply(Reply::Shop(Shop46::Buy { flag: 0, items: vec![] })));
    assert_eq!(m[2], buy_refusal("st_not_enough_money"));
    // ammo is not sold (free, CHOICE): the Mosin's ammo (dict 104, in the profile) is refused
    e.store.update_profile(c.account, |p| { p.silver = 1_000_000; Ok::<_, ()>(()) }).unwrap();
    c.send(buy_req(104, 30));
    e.pump(&mut [&mut c]);
    assert_eq!(c.take()[0], S2c::Reply(Reply::Shop(Shop46::Buy { flag: 0, items: vec![] })));
    let p = e.store.load_profile(c.account).unwrap();
    assert_eq!((p.item(UID_BASE + 1).unwrap().count, p.silver), (200, 1_000_000));
    // a stackable that is sold (a pocket consumable): bought twice, the second buy goes into the same uid
    let dict = pocket_item(&data);
    assert!(data.items.is_stackable(dict));
    c.send(buy_req(dict, 2));
    e.pump(&mut [&mut c]);
    let m = c.take();
    let S2c::Reply(Reply::Shop(Shop46::Buy { items, .. })) = &m[0] else { panic!("{m:?}") };
    assert_eq!(items.len(), 1);
    let uid = items[0].uid;
    c.send(buy_req(dict, 3));
    e.pump(&mut [&mut c]);
    let m = c.take();
    let S2c::Reply(Reply::Shop(Shop46::Buy { items, .. })) = &m[0] else { panic!("{m:?}") };
    assert_eq!((items[0].uid, items[0].dict, items[0].value), (uid, dict, 3));
    let p = e.store.load_profile(c.account).unwrap();
    assert_eq!(p.item(uid).unwrap().count, 5);
}

fn buy_refusal(text: &str) -> S2c {
    S2c::Denied(Denied { request: 0x46, body: DeniedBody::Text(text.into()) })
}

/// A locked faction tile opens the client's trade dialog at the silver price (8A `c` = 0, no gold unlock price). The buy
/// is refused by the client's own rule (level in the item's faction >= item level): window unlocked, 8B, then the
/// faction's `st_item_sell_precondition_faction_<f>` text. At the level it is bought.
#[test]
fn a_locked_faction_item_is_refused_with_the_factions_text_and_bought_at_the_level() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    e.sign_in(&mut c);
    let data = gd();
    // 1218 = SPAS-12: faction 3, item level 3, silver, listed
    let it = data.items.get(1218).unwrap();
    assert_eq!((it.faction, it.item_level), (3, 3));
    assert_eq!(data.shop.price(1218).map(|p| (p.kind, p.c, p.for_sale)), Some((0, 0, true)));
    e.store
        .update_profile(c.account, |p| {
            p.silver = 1_000_000;
            p.fac_points.clear();
            p.add_points(1, 80_000);
            Ok::<_, ()>(())
        })
        .unwrap();
    let before = e.store.load_profile(c.account).unwrap();
    c.send(buy_req(1218, 1));
    e.pump(&mut [&mut c]);
    let m = c.take();
    assert_eq!(m.len(), 3, "{m:?}");
    assert_eq!(m[0], S2c::Reply(Reply::Shop(Shop46::Buy { flag: 0, items: vec![] })));
    assert!(matches!(m[1], S2c::Money(_)));
    assert_eq!(m[2], buy_refusal("st_item_sell_precondition_faction_3"), "faction 1 level 4 does not open faction 3");
    let p = e.store.load_profile(c.account).unwrap();
    assert_eq!((p.silver, p.items.len()), (before.silver, before.items.len()));
    // faction 3 at level 3 (20 000 points): bought for the silver price
    e.store.update_profile(c.account, |p| { p.add_points(3, 20_000); Ok::<_, ()>(()) }).unwrap();
    c.send(buy_req(1218, 1));
    e.pump(&mut [&mut c]);
    let m = c.take();
    let S2c::Reply(Reply::Shop(Shop46::Buy { items, .. })) = &m[0] else { panic!("{m:?}") };
    assert_eq!(items[0].dict, 1218);
    assert!(!m.iter().any(|x| matches!(x, S2c::Denied(_))));
    let p = e.store.load_profile(c.account).unwrap();
    assert_eq!(u64::from(before.silver - p.silver), data.shop.buy_total(&data.items, 1218, 1).unwrap());
}

#[test]
fn sell_pays_the_sell_base_and_removes_the_item() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    e.sign_in(&mut c);
    // the vector sells uid 3 (its torso); ours is UID_BASE + 2
    c.send(Request::Shop(Q46::Sell { item: UID_BASE + 2, amount: 1 }));
    c.send(Request::Shop(Q46::Sell { item: 777, amount: 1 }));
    e.pump(&mut [&mut c]);
    let m = c.take();
    let (rid, rpl) = vector("s2c_replies.json", "sell#0");
    assert_eq!(m[0], lift(S2c::decode(rid, &rpl).unwrap()));
    assert_eq!(m.len(), 2, "an item that is not owned gets no answer: {m:?}");
    let p = e.store.load_profile(c.account).unwrap();
    assert!(p.item(UID_BASE + 2).is_none());
    assert!(p.slots.get(&2).is_none(), "the slot of the sold torso is cleared");
    let data = gd();
    let (_, gain) = data.shop.sell_gain(&data.items, 31, 1, 1);
    assert_eq!(p.silver as u64, 1_000_000 + gain);
}

#[test]
fn exchange_gold_for_silver() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    e.sign_in(&mut c);
    c.send(Request::Shop(Q46::Exchange { amount: 10 }));
    c.send(Request::Shop(Q46::Exchange { amount: 1_000_000 }));
    e.pump(&mut [&mut c]);
    let m = c.take();
    assert_eq!(ids(&m), [0x8B, 0x8B]);
    let p = e.store.load_profile(c.account).unwrap();
    assert_eq!(p.gold, 0, "the second exchange is clamped to the gold held");
    assert_eq!(p.silver, 1_000_000 + 10_000 * 250);
}

// ---- equip ----

fn entry(item: u32, dict: u16, from: u8, to: u8, count: u32) -> EquipEntry {
    EquipEntry { item, dict, from, to, count, x: 1 }
}

fn equip_req(entries: Vec<EquipEntry>) -> Request {
    Request::Inventory(Q45::Equip { profile: 1, entries, c1: 1, c2: 1 })
}

#[test]
fn equip_unequip_bumps_the_version_and_pushes_87() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    e.sign_in(&mut c);
    c.send(equip_req(vec![entry(3, 31, 2, 100, 0)]));
    e.pump(&mut [&mut c]);
    let m = c.take();
    // plain ack, then 87 (no item changed: no 45 0F)
    assert_eq!(m[0], S2c::Reply(Reply::Inventory(Inventory45::Other { sub: 0, data: vec![] })));
    assert_eq!(ids(&m), [0x83, 0x87]);
    let a = account87(&m).unwrap();
    // the version is the content version: one value for the three entries, new after every change
    let v1 = a.profile_state[0].0;
    assert_eq!(a.profile_state, [(v1, 0); 3]);
    assert!(!a.body.profiles[0].slots.contains_key(&2));
    assert!(a.body.profiles[0].slots.contains_key(&7));
    // and back: the same slots as at sign-in, but the stored counter moved, so the version differs from v1
    c.send(equip_req(vec![entry(3, 31, 100, 2, 0)]));
    e.pump(&mut [&mut c]);
    let m = c.take();
    let a = account87(&m).unwrap();
    let v2 = a.profile_state[0].0;
    assert_ne!(v2, v1, "a change gives a new version");
    assert_eq!(a.profile_state, [(v2, 0); 3]);
    assert!(a.body.profiles[0].slots.contains_key(&2));
}

/// The weapon slots follow the lobby movie (game_data::perks): at most one two-handed weapon in slots 7 and 10 (no 0.69d
/// skill carries the "Terminator" modifier), a rifle may sit in the secondary slot next to a pistol, and a refused result
/// undoes the whole request (the 87 shows the stored kit again).
#[test]
fn equip_keeps_one_two_handed_weapon_in_the_weapon_slots() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    e.sign_in(&mut c);
    let data = gd();
    let by = |n: &str| data.items.by_cfg_name(n).unwrap();
    let (ak, svd, glock) = (by("ak_74m"), by("svd"), by("glock_17"));
    let mut uids = (0, 0, 0);
    e.store
        .update_profile(c.account, |p| {
            uids = (p.add_item(ak, 1), p.add_item(svd, 1), p.add_item(glock, 1));
            p.slots.insert(7, uids.0);
            p.slots.remove(&10);
            Ok::<_, ()>(())
        })
        .unwrap();
    let (ak_u, svd_u, glock_u) = uids;
    // a second rifle into slot 10 next to the rifle in slot 7: refused, nothing changes
    c.send(equip_req(vec![entry(svd_u, svd, 100, 10, 0)]));
    e.pump(&mut [&mut c]);
    let a = account87(&c.take()).unwrap().clone();
    assert!(!a.body.profiles[0].slots.contains_key(&10), "{:?}", a.body.profiles[0].slots);
    let p = e.store.load_profile(c.account).unwrap();
    assert_eq!((p.slots.get(&7), p.slots.get(&10)), (Some(&ak_u), None));
    // the pistol into 7 and the rifle into 10 in one request: one two-handed weapon, accepted
    c.send(equip_req(vec![entry(glock_u, glock, 100, 7, 0), entry(svd_u, svd, 100, 10, 0)]));
    e.pump(&mut [&mut c]);
    c.take();
    let p = e.store.load_profile(c.account).unwrap();
    assert_eq!((p.slots.get(&7), p.slots.get(&10)), (Some(&glock_u), Some(&svd_u)));
    // the rifle back into 7 while the other rifle stays in 10: refused, the pistol stays
    c.send(equip_req(vec![entry(ak_u, ak, 100, 7, 0)]));
    e.pump(&mut [&mut c]);
    c.take();
    let p = e.store.load_profile(c.account).unwrap();
    assert_eq!((p.slots.get(&7), p.slots.get(&10)), (Some(&glock_u), Some(&svd_u)));
    // swapping the two weapons between the slots keeps one two-handed weapon: accepted
    c.send(equip_req(vec![entry(svd_u, svd, 10, 7, 0), entry(glock_u, glock, 7, 10, 0)]));
    e.pump(&mut [&mut c]);
    c.take();
    let p = e.store.load_profile(c.account).unwrap();
    assert_eq!((p.slots.get(&7), p.slots.get(&10)), (Some(&svd_u), Some(&glock_u)));
}

#[test]
fn equip_ammo_slider_supplies_the_missing_rounds_free() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    e.sign_in(&mut c);
    let data = gd();
    // ammo is free (CHOICE): supplied by its dict id (uid == dict, PROVEN with the real client), the stack is created in whole
    // clips and nothing is charged; 7.62x39 (81, the wiki had a pack price for it)
    let pd = 81u16;
    assert!(data.items.is_ammo(pd));
    let before = e.store.load_profile(c.account).unwrap();
    c.send(equip_req(vec![entry(pd as u32, pd, 100, 8, 250)]));
    e.pump(&mut [&mut c]);
    let m = c.take();
    let (cost, added) = data.shop.ammo_top_up(pd, 250);
    assert_eq!(cost, 0);
    let p = e.store.load_profile(c.account).unwrap();
    let uid = p.slots[&8];
    assert_eq!(p.item(uid).unwrap().count, added);
    assert_eq!(p.silver, before.silver);
    assert_eq!(p.slot_rounds[&8], 250);
    // the new stack goes out as a record before the 87; no money changed: no 8B
    assert_eq!(ids(&m), [0x83, 0x83, 0x87]);
    let S2c::Reply(Reply::Inventory(Inventory45::Records { items, .. })) = &m[1] else { panic!("{m:?}") };
    assert_eq!((items[0].uid, items[0].dict, items[0].value), (uid, pd, added));
    // within the stack: free
    let silver = p.silver;
    c.send(equip_req(vec![entry(uid, pd, 8, 8, 100)]));
    e.pump(&mut [&mut c]);
    c.take();
    let p = e.store.load_profile(c.account).unwrap();
    assert_eq!((p.silver, p.slot_rounds[&8]), (silver, 100));
    // no silver at all: rounds beyond the stack still load (free)
    e.store.update_profile(c.account, |p| { p.silver = 0; Ok::<_, ()>(()) }).unwrap();
    c.send(equip_req(vec![entry(uid, pd, 8, 8, added + 500)]));
    e.pump(&mut [&mut c]);
    c.take();
    let p = e.store.load_profile(c.account).unwrap();
    assert_eq!((p.silver, p.slot_rounds[&8]), (0, added + 500));
    assert!(p.item(uid).unwrap().count >= added + 500);
    // the shop does not sell ammo, and selling it pays nothing
    assert_eq!(data.shop.buy_total(&data.items, pd, 30), None);
    assert_eq!(data.shop.sell_gain(&data.items, pd, 300, 300).1, 0);
}

/// Premium ammo boxes (categories 37 / 38): listed in 8A at the game.options gold price
/// (3 / 7 / 10 primary, 2 / 3 / 5 pistol), bought per box with gold (46 00 from the selector's trade dialog) into one
/// stack, and equipped into slot 11 (one box); slot 11 refuses anything else.
#[test]
fn premium_ammo_boxes_are_sold_for_gold_and_go_into_slot_11() {
    game_data::require_tables!();
    let data = gd();
    for (dict, gold) in [(1276u16, 3u32), (1278, 10), (1287, 10), (1288, 2), (1299, 5)] {
        let p = data.shop.price(dict).unwrap();
        assert_eq!((p.a, p.b, p.kind, p.for_sale), (gold, 0, 1, true), "8A box {dict}");
    }
    let (_d, mut e) = env();
    let mut c = e.client("test");
    e.sign_in(&mut c);
    c.take();
    e.store.update_profile(c.account, |p| { p.gold = 100; Ok::<_, ()>(()) }).unwrap();
    c.send(buy_req(1281, 3));
    e.pump(&mut [&mut c]);
    let m = c.take();
    let S2c::Reply(Reply::Shop(Shop46::Buy { items, .. })) = &m[0] else { panic!("{m:?}") };
    let uid = items[0].uid;
    assert_eq!((items[0].dict, items[0].value), (1281, 3));
    let p = e.store.load_profile(c.account).unwrap();
    assert_eq!((p.gold, p.item(uid).unwrap().count), (70, 3));
    // one more: the same stack
    c.send(buy_req(1281, 1));
    e.pump(&mut [&mut c]);
    c.take();
    let p = e.store.load_profile(c.account).unwrap();
    assert_eq!((p.gold, p.item(uid).unwrap().count), (60, 4));
    // equip one box into slot 11 (the selector sends {uid, dict, 100, 11, 1})
    c.send(equip_req(vec![entry(uid, 1281, 100, 11, 1)]));
    e.pump(&mut [&mut c]);
    let m = c.take();
    let p = e.store.load_profile(c.account).unwrap();
    assert_eq!((p.slots.get(&11), p.slot_rounds.get(&11)), (Some(&uid), Some(&1)));
    assert!(account87(&m).unwrap().body.profiles[0].slots.contains_key(&11));
    // a non-box item is refused in slot 11 (the Mosin ammo, uid UID_BASE + 1)
    c.send(equip_req(vec![entry(UID_BASE + 1, 104, 100, 12, 30)]));
    e.pump(&mut [&mut c]);
    c.take();
    assert!(e.store.load_profile(c.account).unwrap().slots.get(&12).is_none());
}

/// The box slots follow the lobby movie's selector (`AmmoBoostersSelector`): slot 11 takes the
/// box category of the weapon in slot 7, slot 12 that of slot 10 (`getItemAmmoBoostersCategory`: pistol 38, other
/// weapons 37, no weapon none); picking the selector's default entry sends `{uid, dict, 11, 100, 1}` and frees the slot
/// without touching the stack.
#[test]
fn ammo_box_slots_take_the_box_category_of_their_weapon() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    e.sign_in(&mut c);
    c.take();
    let (main_box, pistol_box) = e
        .store
        .update_profile(c.account, |p| {
            let a = p.add_item(1281, 2);
            let b = p.add_item(1288, 2);
            Ok::<_, ()>((a, b))
        })
        .unwrap();
    let slots = |e: &Env, c: &Client| {
        let p = e.store.load_profile(c.account).unwrap();
        (p.slots.get(&11).copied(), p.slots.get(&12).copied())
    };
    // the Mosin in slot 7 takes a primary box only; slot 10 is empty, so slot 12 takes nothing
    for (uid, dict, slot) in [(pistol_box, 1288, 11), (main_box, 1281, 12), (pistol_box, 1288, 12)] {
        c.send(equip_req(vec![entry(uid, dict, 100, slot, 1)]));
        e.pump(&mut [&mut c]);
        c.take();
        assert_eq!(slots(&e, &c), (None, None), "box {dict} -> slot {slot}");
    }
    c.send(equip_req(vec![entry(main_box, 1281, 100, 11, 1)]));
    e.pump(&mut [&mut c]);
    c.take();
    assert_eq!(slots(&e, &c), (Some(main_box), None));
    // a pistol (TT-33, category 17) in slot 10: slot 12 takes the pistol box, not the primary one
    let tt = e
        .store
        .update_profile(c.account, |p| {
            let u = p.add_item(18, 18);
            Ok::<_, ()>(u)
        })
        .unwrap();
    c.send(equip_req(vec![entry(tt, 18, 100, 10, 0), entry(main_box, 1281, 100, 12, 1)]));
    e.pump(&mut [&mut c]);
    c.take();
    assert_eq!(slots(&e, &c), (Some(main_box), None));
    c.send(equip_req(vec![entry(pistol_box, 1288, 100, 12, 1)]));
    e.pump(&mut [&mut c]);
    let m = c.take();
    assert_eq!(slots(&e, &c), (Some(main_box), Some(pistol_box)));
    let s = &account87(&m).unwrap().body.profiles[0].slots;
    assert!(s.contains_key(&11) && s.contains_key(&12));
    // the selector's default entry: back to the inventory, the stack keeps both boxes
    c.send(equip_req(vec![entry(main_box, 1281, 11, 100, 1)]));
    e.pump(&mut [&mut c]);
    c.take();
    let p = e.store.load_profile(c.account).unwrap();
    assert_eq!((p.slots.get(&11), p.slot_rounds.get(&11), p.item(main_box).unwrap().count), (None, None, 2));
}

fn pocket_item(d: &GameData) -> u16 {
    d.items
        .iter()
        .find(|i| d.items.is_consumable(i.dict_id) && d.shop.price(i.dict_id).is_some_and(|p| p.for_sale && p.a > 0 && p.kind == 0))
        .unwrap()
        .dict_id
}

#[test]
fn a_pocket_buy_creates_and_charges_the_stack() {
    game_data::require_tables!();
    let d = gd();
    let dict = pocket_item(&d);
    // the stack is created and charged, 45 0F record first, then 87 with the pocket slot, then 8B
    let dir = tempfile::tempdir().unwrap();
    let cfg = LobbyConfig { achievement_rewards: false, ..LobbyConfig::default() };
    let mut e = Env::with(&dir.path().join("p.sqlite"), cfg, Arc::new(MinimalMatches));
    let mut c = e.client("test");
    e.sign_in(&mut c);
    c.send(equip_req(vec![entry(dict as u32, dict, 200, 13, 2)]));
    e.pump(&mut [&mut c]);
    let m = c.take();
    assert_eq!(ids(&m), [0x83, 0x83, 0x87, 0x8B]);
    let p = e.store.load_profile(c.account).unwrap();
    let uid = p.slots[&13];
    assert_eq!(p.item(uid).unwrap().count, 2);
    assert_eq!(p.slot_rounds[&13], 2);
    let (_, cost) = d.shop.pocket_cost(dict, 2);
    assert_eq!(1_000_000 - p.silver as u64, cost);
    let a = account87(&m).unwrap();
    assert!(a.body.profiles[0].slots.contains_key(&13));
}

// ---- workshop ----

#[test]
fn workshop_open_pushes_b3_with_every_slot() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    e.sign_in(&mut c);
    c.send(Request::Workshop(Workshop60::Opened { dict: 107 }));
    e.pump(&mut [&mut c]);
    let m = c.take();
    assert_eq!(m[0], S2c::Reply(Reply::Raw { request: 0x60, data: vec![0] }));
    let S2c::Workshop(w) = &m[1] else { panic!("{m:?}") };
    let d = gd();
    let want: Vec<(String, u8)> = d.modules.weapon_slot_ids(107).iter().map(|s| (s.clone(), 0)).collect();
    assert!(!want.is_empty());
    assert_eq!((w.dict, &w.slots), (107, &want));
}

/// A slot of the weapon itself (no mount needed) that takes the module, if any.
fn weapon_slot_for(d: &GameData, weapon: u16, module: u16) -> Option<String> {
    let set = d.modules.slot_set(weapon, |s| d.modules.default_in(weapon, s));
    d.modules.attach_slots(module, weapon).find(|s| set.get(s).is_some_and(|i| i.accept && i.parent.is_none())).map(str::to_string)
}

#[test]
fn modules_attach_detach_buy_and_persist() {
    game_data::require_tables!();
    let (_dir, mut e) = env();
    let mut c = e.client("test");
    e.sign_in(&mut c);
    let d = gd();
    let p = e.store.load_profile(c.account).unwrap();
    // a seeded module of the Mosin (107, uid UID_BASE) that goes into one of the rifle's own slots
    let (module, slot) = p
        .items
        .iter()
        .filter(|i| d.items.is_module(i.dict_id) && p.owner_of(i.uid).is_none() && d.modules.is_candidate(i.dict_id, 107))
        .find_map(|i| weapon_slot_for(&d, 107, i.dict_id).map(|s| (*i, s)))
        .unwrap();
    let act = |kind: u8, uid: u32, dict: u16, slot: Option<&str>, cost: u16| ModuleAction {
        kind,
        f24: 0,
        module_uid: uid,
        module_dict: dict,
        slot: slot.map(|s| s.to_string()),
        cost,
        f2d: 0,
        bound: 0,
    };
    // attach
    c.send(Request::Workshop(Workshop60::Save { weapon: UID_BASE, weapon_dict: 107, actions: vec![act(0, module.uid, module.dict_id, Some(&slot), 0)] }));
    e.pump(&mut [&mut c]);
    let m = c.take();
    assert_eq!(ids(&m), [0x83, 0x8B, 0x83, 0x87], "45 0F, 8B, 83 60 01, 87");
    let S2c::Reply(Reply::Inventory(Inventory45::Records { items, .. })) = &m[0] else { panic!() };
    let weapon = items.iter().find(|r| r.uid == UID_BASE).unwrap();
    assert_eq!(weapon.modules.len(), 1);
    assert_eq!((weapon.modules[0].uid, weapon.modules[0].slot.as_str()), (module.uid, slot.as_str()));
    assert_eq!(items.iter().find(|r| r.uid == module.uid).unwrap().attached_to, UID_BASE);
    assert_eq!(m[2], S2c::Reply(Reply::WorkshopDone { uid: UID_BASE }));
    let name = e.store.account(c.account).unwrap().unwrap().name;
    let p_now = e.store.load_profile(c.account).unwrap();
    assert_eq!(account87(&m).unwrap().profile_state[0].0, lobby_server::build::wire_version(&gd(), &p_now, &name), "content version");
    assert_eq!(e.store.load_profile(c.account).unwrap().attach[&UID_BASE][&slot], module.uid);
    // a slot the weapon does not have is refused (nothing attached)
    c.send(Request::Workshop(Workshop60::Save { weapon: UID_BASE, weapon_dict: 107, actions: vec![act(0, module.uid, module.dict_id, Some("no_such_slot"), 0)] }));
    e.pump(&mut [&mut c]);
    c.take();
    assert_eq!(e.store.load_profile(c.account).unwrap().attach[&UID_BASE].len(), 1);
    // detach
    c.send(Request::Workshop(Workshop60::Save { weapon: UID_BASE, weapon_dict: 107, actions: vec![act(1, module.uid, module.dict_id, None, 0)] }));
    e.pump(&mut [&mut c]);
    c.take();
    assert!(e.store.load_profile(c.account).unwrap().attach.is_empty());
    // buy with spare parts at the server's price (the client's cost field says 0), then attach in the same save
    let (nm, nslot) = d
        .items
        .iter()
        .filter(|i| d.items.is_module(i.dict_id) && d.shop.price(i.dict_id).is_some_and(|p| p.a > 0))
        .filter(|i| d.modules.is_candidate(i.dict_id, 107))
        .find_map(|i| weapon_slot_for(&d, 107, i.dict_id).map(|s| (i.dict_id, s)))
        .unwrap();
    let price = d.shop.price(nm).unwrap().a;
    let parts = e.store.load_profile(c.account).unwrap().parts;
    c.send(Request::Workshop(Workshop60::Save {
        weapon: UID_BASE,
        weapon_dict: 107,
        actions: vec![act(4, 9999, nm, None, 0), act(0, 9999, nm, Some(&nslot), 0)],
    }));
    e.pump(&mut [&mut c]);
    let m = c.take();
    let q = e.store.load_profile(c.account).unwrap();
    assert_eq!(q.parts, parts - price);
    let uid = q.attach[&UID_BASE][&nslot];
    assert_eq!(q.item(uid).unwrap().dict_id, nm);
    assert!(p.item(uid).is_none(), "the bought module, not an owned one");
    assert!(matches!(money(&m), Some(x) if x.spare_parts == parts - price));
}

// AKMN (1201), the dovetail picatinny mounts 1703 / 1704 (100 spare parts, detach price 50) and the NcSTAR collimator
// 1772 (200 spare parts, detach price 100), docs/notes/workshop.md
const AKMN: u16 = 1201;
const MOUNT: u16 = 1703;
const MOUNT2: u16 = 1704;
const SCOPE: u16 = 1772;

fn wact(kind: u8, uid: u32, dict: u16, slot: Option<&str>) -> ModuleAction {
    ModuleAction { kind, f24: 0, module_uid: uid, module_dict: dict, slot: slot.map(|s| s.to_string()), cost: 0, f2d: 0, bound: 0 }
}

fn wsave(c: &mut Client, e: &mut Env, weapon: u32, actions: Vec<ModuleAction>) -> Vec<S2c> {
    c.send(Request::Workshop(Workshop60::Save { weapon, weapon_dict: AKMN, actions }));
    e.pump(&mut [&mut *c]);
    c.take()
}

fn wrecords(m: &[S2c]) -> &[ItemRecord] {
    m.iter().find_map(|x| if let S2c::Reply(Reply::Inventory(Inventory45::Records { items, .. })) = x { Some(items.as_slice()) } else { None }).unwrap()
}

/// A signed-in client whose profile owns an AKMN and the listed items; returns (AKMN uid, item uids).
fn with_akmn(e: &mut Env, c: &mut Client, extra: &[u16]) -> (u32, Vec<u32>) {
    let d = gd();
    let r = e
        .store
        .update_profile(c.account, |p| {
            let w = p.add_item(AKMN, d.items.amount_field(AKMN, 1));
            Ok::<_, ()>((w, extra.iter().map(|&x| p.add_item(x, d.items.amount_field(x, 1))).collect()))
        })
        .unwrap();
    c.take();
    r
}

/// The 40-round magazine (dict 1991) bought for gold and attached in one save. The module gets the
/// list the client itself predicts for a bought module (premium (311, 7) (407, 7), rarity gold (312, 4), capacity
/// (520, 7) = 40; 0x2002DFD30), in its own record and in the AKMN's n4 element; both survive a reload (89).
#[test]
fn a_bought_module_carries_its_modifier_pairs_in_both_records() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    e.sign_in(&mut c);
    let (w, _) = with_akmn(&mut e, &mut c, &[]);
    e.store.update_profile(c.account, |p| Ok::<_, ()>(p.gold = 100)).unwrap();
    let want = vec![(311, 7), (407, 7), (312, 4), (520, 7)];
    let m = wsave(&mut c, &mut e, w, vec![wact(3, 900, 1991, None), wact(0, 900, 1991, Some("mag"))]);
    let recs = wrecords(&m);
    let weapon = recs.iter().find(|r| r.uid == w).expect("weapon record");
    let elem = weapon.modules.iter().find(|x| x.slot == "mag").unwrap();
    assert_eq!((elem.dict, &elem.mods), (1991, &want));
    let module = recs.iter().find(|r| r.uid == elem.uid).expect("module record");
    assert_eq!((module.attached_to, &module.mods), (w, &want));
    let p = e.store.load_profile(c.account).unwrap();
    assert_eq!(p.mods.get(&elem.uid), Some(&want));
    let inv = lobby_server::build::inventory_89(&gd(), &p);
    assert_eq!(inv.items.iter().find(|r| r.uid == w).unwrap().modules.iter().find(|x| x.slot == "mag").unwrap().mods, want);
}

/// "Create" for spare parts (action 4, 0.60 notes: "Rarity of the module will be determined randomly"): the 40-round
/// magazine gets a rolled rarity (green / blue / orange, never the gold of a gold buy) with its drop grades (311, 407)
/// inside a cell of that rarity, plus its capacity pair; the records carry the same list.
#[test]
fn a_created_module_gets_a_random_rarity() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    e.sign_in(&mut c);
    let (w, _) = with_akmn(&mut e, &mut c, &[]);
    e.store.update_profile(c.account, |p| Ok::<_, ()>(p.parts = 100_000)).unwrap();
    let mut seen = std::collections::BTreeSet::new();
    for seed in 1..=12u64 {
        e.server.lobby.seed(seed);
        let m = wsave(&mut c, &mut e, w, vec![wact(4, 900, 1991, None), wact(0, 900, 1991, Some("mag"))]);
        let recs = wrecords(&m);
        let weapon = recs.iter().find(|r| r.uid == w).expect("weapon record");
        let elem = weapon.modules.iter().find(|x| x.slot == "mag").unwrap();
        let module = recs.iter().find(|r| r.uid == elem.uid).expect("module record");
        assert_eq!(module.mods, elem.mods);
        let r = rarity(&module.mods);
        assert!((1..=3).contains(&r), "{:?}", module.mods);
        assert!(module.mods.contains(&(520, 7)), "{:?}", module.mods);
        assert_drop_cells(1991, &module.mods, r);
        seen.insert(r);
    }
    assert!(seen.len() >= 2, "random: {seen:?}");
}

#[test]
fn workshop_mount_and_scope_chain_on_the_akmn() {
    game_data::require_tables!();
    let (_dir, mut e) = env();
    let mut c = e.client("test");
    e.sign_in(&mut c);
    let d = gd();
    let (w, u) = with_akmn(&mut e, &mut c, &[SCOPE]);
    let scope = u[0];
    // the scope alone: picatinny_scope_medium exists only while a dovetail mount sits in scope_dovetail
    wsave(&mut c, &mut e, w, vec![wact(6, scope, SCOPE, None), wact(0, scope, SCOPE, Some("picatinny_scope_medium"))]);
    let p = e.store.load_profile(c.account).unwrap();
    assert!(!p.attach.contains_key(&w));
    assert_eq!(p.bound.get(&scope), Some(&AKMN), "action 6 binds to this weapon");
    // the client's chain: buy the mount (spare parts), bind, attach it to scope_dovetail, then the scope in the added slot
    let parts = p.parts;
    let m = wsave(
        &mut c,
        &mut e,
        w,
        vec![wact(4, 7, MOUNT, None), wact(6, 7, MOUNT, None), wact(0, 7, MOUNT, Some("scope_dovetail")), wact(0, scope, SCOPE, Some("picatinny_scope_medium"))],
    );
    let p = e.store.load_profile(c.account).unwrap();
    let mount = p.attach[&w]["scope_dovetail"];
    assert_eq!(p.item(mount).unwrap().dict_id, MOUNT);
    assert_eq!(p.attach[&w]["picatinny_scope_medium"], scope);
    assert_eq!(p.parts, parts - d.shop.price(MOUNT).unwrap().a);
    assert_eq!(p.bound.get(&mount), Some(&AKMN));
    let rec = wrecords(&m).iter().find(|r| r.uid == w).unwrap();
    assert!(rec.modules.iter().any(|x| x.uid == scope && x.slot == "picatinny_scope_medium"), "{rec:?}");
    assert!(rec.modules.iter().any(|x| x.uid == mount && x.slot == "scope_dovetail"));
    assert_eq!(rec.modules.iter().filter(|x| x.uid == u32::MAX).count(), d.modules.default_modules(AKMN).len(), "defaults stay");
    // removing the mount sends the scope to the inventory (its slot is gone)
    let m = wsave(&mut c, &mut e, w, vec![wact(1, mount, MOUNT, None)]);
    let p = e.store.load_profile(c.account).unwrap();
    assert!(!p.attach.contains_key(&w), "{:?}", p.attach);
    assert!(p.owner_of(scope).is_none() && p.item(scope).is_some() && p.item(mount).is_some());
    assert!(wrecords(&m).iter().any(|r| r.uid == scope && r.attached_to == 0));
    // a detach with f24 = 1 destroys the module: record count 0
    wsave(&mut c, &mut e, w, vec![wact(0, mount, MOUNT, Some("scope_dovetail"))]);
    assert_eq!(e.store.load_profile(c.account).unwrap().attach[&w]["scope_dovetail"], mount);
    let m = wsave(&mut c, &mut e, w, vec![ModuleAction { f24: 1, ..wact(1, mount, MOUNT, None) }]);
    let p = e.store.load_profile(c.account).unwrap();
    assert!(p.item(mount).is_none() && !p.attach.contains_key(&w));
    assert!(wrecords(&m).iter().any(|r| r.uid == mount && r.value == 0));
}

#[test]
fn workshop_removed_default_stays_removed() {
    game_data::require_tables!();
    let (dir, mut e) = env();
    let mut c = e.client("test");
    e.sign_in(&mut c);
    let d = gd();
    let (w, _) = with_akmn(&mut e, &mut c, &[]);
    let forend = d.modules.default_in(AKMN, "forend").unwrap();
    let m = wsave(&mut c, &mut e, w, vec![wact(1, u32::MAX, forend, None)]);
    let rec = wrecords(&m).iter().find(|r| r.uid == w).unwrap();
    assert!(!rec.modules.iter().any(|x| x.slot == "forend"), "{rec:?}");
    assert_eq!(rec.modules.len(), d.modules.default_modules(AKMN).len() - 1);
    // after a restart the default is still gone (stored as an attachment row with module uid 0)
    e.restart(&db(&dir), LobbyConfig { achievement_rewards: false, login_calendar: None, ..LobbyConfig::default() }, Arc::new(MinimalMatches));
    let p = e.store.load_profile(c.account).unwrap();
    assert!(p.default_removed(w, "forend") && !p.attach.contains_key(&w));
    let rec = lobby_server::build::record(&d, &p, p.item(w).unwrap());
    assert!(!rec.modules.iter().any(|x| x.slot == "forend"));
    // ATTACH uid 0xFFFFFFFF puts the default back
    let mut c = e.client_of("test");
    e.sign_in(&mut c);
    c.take();
    let m = wsave(&mut c, &mut e, w, vec![wact(0, u32::MAX, forend, Some("forend"))]);
    let rec = wrecords(&m).iter().find(|r| r.uid == w).unwrap();
    assert!(rec.modules.iter().any(|x| x.slot == "forend" && x.uid == u32::MAX && x.dict == forend));
    assert!(!e.store.load_profile(c.account).unwrap().default_removed(w, "forend"));
    // a default of another slot is refused
    let mag = d.modules.default_in(AKMN, "mag").unwrap();
    wsave(&mut c, &mut e, w, vec![wact(1, u32::MAX, forend, None), wact(0, u32::MAX, mag, Some("forend"))]);
    let p = e.store.load_profile(c.account).unwrap();
    assert!(p.default_removed(w, "forend") && !p.default_removed(w, "mag"));
}

#[test]
fn workshop_rebind_charges_gold_and_gold_buys_cost_a_twentieth() {
    game_data::require_tables!();
    let (_dir, mut e) = env();
    let mut c = e.client("test");
    e.sign_in(&mut c);
    let d = gd();
    let (w, u) = with_akmn(&mut e, &mut c, &[SCOPE]);
    let scope = u[0];
    let acct = c.account;
    let set = |e: &Env, f: &dyn Fn(&mut accounts::Profile)| e.store.update_profile(acct, |p| Ok::<_, ()>(f(p))).unwrap();
    set(&e, &|p| {
        p.bound.insert(scope, 107);
    });
    // action 6 on a module bound to another weapon: refused (that is action 7)
    wsave(&mut c, &mut e, w, vec![wact(6, scope, SCOPE, None)]);
    assert_eq!(e.store.load_profile(c.account).unwrap().bound[&scope], 107);
    // action 7: detach_price_in_gold of the game data (100 for 1772), not the client's cost field
    let rebind = ModuleAction { f24: 107, cost: 1, ..wact(7, scope, SCOPE, None) };
    let gold = e.store.load_profile(c.account).unwrap().gold;
    assert_eq!(d.modules.detach_price(SCOPE), 100);
    wsave(&mut c, &mut e, w, vec![rebind.clone()]);
    let p = e.store.load_profile(c.account).unwrap();
    assert_eq!((p.gold, p.bound[&scope]), (gold - 100, AKMN));
    // gold short: refused, nothing changes
    set(&e, &|p| {
        p.bound.insert(scope, 107);
        p.gold = 50;
    });
    wsave(&mut c, &mut e, w, vec![rebind]);
    let p = e.store.load_profile(c.account).unwrap();
    assert_eq!((p.gold, p.bound[&scope]), (50, 107));
    // gold buy (action 3) at 8A c = a / 20 (CHOICE): 100 spare parts = 5 gold, 200 = 10
    assert_eq!((d.shop.price(MOUNT2).unwrap().c, d.shop.price(SCOPE).unwrap().c), (5, 10));
    set(&e, &|p| p.gold = 1000);
    wsave(&mut c, &mut e, w, vec![wact(3, 11, MOUNT2, None)]);
    let p = e.store.load_profile(c.account).unwrap();
    assert_eq!(p.gold, 995);
    assert!(p_has(&p, MOUNT2));
    // a module with no spare-parts price is not sold for gold either
    let free = d.items.iter().find(|i| d.items.is_module(i.dict_id) && i.cost_int() == 0).unwrap().dict_id;
    wsave(&mut c, &mut e, w, vec![wact(3, 12, free, None)]);
    assert_eq!(e.store.load_profile(c.account).unwrap().gold, 995);
    // inventory full (weapons + armour + modules >= 89 max_items): a spare-parts buy is refused, a gold buy is not
    set(&e, &|p| {
        while p.items.iter().filter(|i| d.items.is_weapon(i.dict_id) || d.items.is_armour(i.dict_id) || d.items.is_module(i.dict_id)).count()
            < lobby_server::build::MAX_ITEMS as usize
        {
            p.add_item(SCOPE, d.items.amount_field(SCOPE, 1));
        }
    });
    let before = e.store.load_profile(c.account).unwrap();
    wsave(&mut c, &mut e, w, vec![wact(4, 13, MOUNT, None)]);
    let p = e.store.load_profile(c.account).unwrap();
    assert_eq!((p.parts, p.items.len()), (before.parts, before.items.len()));
    wsave(&mut c, &mut e, w, vec![wact(3, 14, MOUNT, None)]);
    let p = e.store.load_profile(c.account).unwrap();
    assert_eq!((p.gold, p.items.len()), (before.gold - 5, before.items.len() + 1));
}

/// 45 05 on the starter Mosin's modifier `old`: the three rows and every answer.
fn reroll(e: &mut Env, c: &mut Client, old: (u16, u16), pay: u8) -> ([(u16, u16); 3], Vec<S2c>) {
    c.send(Request::Inventory(Q45::Reroll { item: UID_BASE, group: old.0, grade: old.1, pay }));
    e.pump(&mut [&mut *c]);
    let m = c.take();
    let S2c::Reply(Reply::Inventory(Inventory45::RerollOptions { options, .. })) = &m[0] else { panic!("{m:?}") };
    (*options, m)
}

/// 45 06 on the starter Mosin: row `row`, the client's shown gold `gold`; returns the answers.
fn reroll_pick(e: &mut Env, c: &mut Client, old: (u16, u16), row: u32, gold: u32) -> Vec<S2c> {
    c.send(Request::Inventory(Q45::RerollPick { item: UID_BASE, old_group: old.0, old_grade: old.1, row, gold }));
    e.pump(&mut [&mut *c]);
    c.take()
}

fn p_has(p: &accounts::Profile, dict: u16) -> bool {
    p.items.iter().any(|i| i.dict_id == dict)
}

#[test]
fn craft_pick_and_reroll() {
    game_data::require_tables!();
    let (_dir, mut e) = env();
    let mut c = e.client("test");
    e.sign_in(&mut c);
    let d = gd();
    let quote = d.craft.craft_quote(107, 0, true);
    assert!(quote.allowed, "the starter Mosin can be upgraded: {quote:?}");
    let gold0 = e.store.load_profile(c.account).unwrap().gold;
    // upgrade with gold: offer sorted, price charged, 8B
    c.send(Request::Inventory(Q45::Craft { item: UID_BASE, pay: 0xFF }));
    e.pump(&mut [&mut c]);
    let m = c.take();
    let S2c::Reply(Reply::Inventory(Inventory45::Craft { result, item, offer })) = &m[0] else { panic!("{m:?}") };
    assert_eq!((*result, *item), (1, UID_BASE));
    assert!(!offer.is_empty() && offer.len() <= 3);
    let mut sorted = offer.clone();
    sorted.sort_unstable();
    assert_eq!(&sorted, offer);
    assert_eq!(money(&m).unwrap().gold, gold0 - quote.price);
    // pick index 1 (clamped to the offer)
    c.send(Request::Inventory(Q45::CraftPick { item: UID_BASE, index: 1 }));
    e.pump(&mut [&mut c]);
    let m = c.take();
    let pick = offer[1.min(offer.len() - 1)];
    assert_eq!(m[0], S2c::Reply(Reply::Inventory(Inventory45::CraftDone { item: UID_BASE, old: (0, 0), new: pick })));
    assert_eq!(e.store.load_profile(c.account).unwrap().mods[&UID_BASE], vec![pick]);
    // a pick without an offer changes nothing; the refusal 83 45 07 result 2 unlocks the window
    c.send(Request::Inventory(Q45::CraftPick { item: UID_BASE, index: 0 }));
    e.pump(&mut [&mut c]);
    assert_eq!(c.take(), vec![S2c::Reply(Reply::Inventory(Inventory45::Craft { result: 2, item: UID_BASE, offer: vec![] }))]);
    assert_eq!(e.store.load_profile(c.account).unwrap().mods[&UID_BASE], vec![pick]);

    // reroll that modifier with spare parts: three rows, 8B; row 0 is free and replaces the modifier
    let parts0 = e.store.load_profile(c.account).unwrap().parts;
    let (options, m) = reroll(&mut e, &mut c, pick, 0);
    assert_eq!(money(&m).unwrap().spare_parts, parts0 - d.craft.reroll_price(107, false));
    let gold1 = e.store.load_profile(c.account).unwrap().gold;
    let new = options[0];
    let m = reroll_pick(&mut e, &mut c, pick, 0, 0);
    assert_eq!(m[0], S2c::Reply(Reply::Inventory(Inventory45::RerollDone { item: UID_BASE, old: pick, new })));
    assert!(money(&m).is_none(), "row 0 costs nothing: {m:?}");
    let p = e.store.load_profile(c.account).unwrap();
    assert_eq!((p.mods[&UID_BASE].clone(), p.gold), (vec![new], gold1));
    // spare parts again, row 2 = a "premium" row: our own price is charged (not the client's number), 8B
    let premium = d.craft.reroll_premium_price(107);
    assert!(premium > 0, "the Mosin has a premium reroll price");
    e.store.update_profile(c.account, |p| Ok::<_, ()>(p.gold = 1_000)).unwrap();
    let (options, _) = reroll(&mut e, &mut c, new, 0);
    let m = reroll_pick(&mut e, &mut c, new, 2, 1);
    let third = options[2];
    assert_eq!(m[0], S2c::Reply(Reply::Inventory(Inventory45::RerollDone { item: UID_BASE, old: new, new: third })));
    assert_eq!(money(&m).unwrap().gold, 1_000 - premium);
    assert_eq!(e.store.load_profile(c.account).unwrap().mods[&UID_BASE], vec![third]);
    // a premium row without enough gold keeps the modifier and charges nothing
    e.store.update_profile(c.account, |p| Ok::<_, ()>(p.gold = premium - 1)).unwrap();
    reroll(&mut e, &mut c, third, 0);
    let m = reroll_pick(&mut e, &mut c, third, 1, premium);
    assert_eq!(m[0], S2c::Reply(Reply::Inventory(Inventory45::RerollDone { item: UID_BASE, old: third, new: third })));
    let p = e.store.load_profile(c.account).unwrap();
    assert_eq!((p.mods[&UID_BASE].clone(), p.gold), (vec![third], premium - 1));
    // a gold reroll: every row is free
    e.store.update_profile(c.account, |p| Ok::<_, ()>(p.gold = 10_000)).unwrap();
    let (options, _) = reroll(&mut e, &mut c, third, 0xFF);
    let gold2 = e.store.load_profile(c.account).unwrap().gold;
    assert_eq!(gold2, 10_000 - d.craft.reroll_price(107, true));
    let m = reroll_pick(&mut e, &mut c, third, 1, 0);
    let fourth = options[1];
    assert_eq!(m[0], S2c::Reply(Reply::Inventory(Inventory45::RerollDone { item: UID_BASE, old: third, new: fourth })));
    assert_eq!(e.store.load_profile(c.account).unwrap().gold, gold2);
    // a row outside the list and a pick without an offer keep the modifier and still answer
    reroll(&mut e, &mut c, fourth, 0);
    let m = reroll_pick(&mut e, &mut c, fourth, 3, 0);
    assert_eq!(m[0], S2c::Reply(Reply::Inventory(Inventory45::RerollDone { item: UID_BASE, old: fourth, new: fourth })));
    let m = reroll_pick(&mut e, &mut c, fourth, 0, 0);
    assert_eq!(m[0], S2c::Reply(Reply::Inventory(Inventory45::RerollDone { item: UID_BASE, old: fourth, new: fourth })));
    assert_eq!(e.store.load_profile(c.account).unwrap().mods[&UID_BASE], vec![fourth]);
    // module upgrade with an unknown donor: refused
    c.send(Request::Inventory(Q45::ModuleUpgrade { items: [UID_BASE + 4, UID_BASE + 5, UID_BASE + 6, 999_999], pay: 0 }));
    e.pump(&mut [&mut c]);
    assert_eq!(c.take()[0], S2c::Reply(Reply::Inventory(Inventory45::ModuleUpgrade { result: 2, uid: UID_BASE + 4, mods: vec![] })));
}

#[test]
fn craft_refused_without_money_charges_nothing() {
    game_data::require_tables!();
    let (_dir, mut e) = env();
    let mut c = e.client("test");
    e.sign_in(&mut c);
    e.store.update_profile(c.account, |p| { p.parts = 0; Ok::<_, ()>(()) }).unwrap();
    c.send(Request::Inventory(Q45::Craft { item: UID_BASE, pay: 0 }));
    c.send(Request::Inventory(Q45::Craft { item: 424242, pay: 0 }));
    e.pump(&mut [&mut c]);
    let m = c.take();
    let rs = replies(&m);
    assert_eq!(rs[0], &Reply::Inventory(Inventory45::Craft { result: 2, item: UID_BASE, offer: vec![] }));
    assert_eq!(rs[1], &Reply::Inventory(Inventory45::Craft { result: 2, item: 424242, offer: vec![] }));
    // nothing stored for the weapon; the only stored lists are the seeded modules' green rolls
    let p = e.store.load_profile(c.account).unwrap();
    assert!(!p.mods.contains_key(&UID_BASE));
    assert!(p.mods.keys().all(|u| gd().items.is_module(p.item(*u).unwrap().dict_id)));
}

// ---- local matches ----

/// A training match with bots pays nothing, so it charges no per-match booster (CHOICE): no silver, no gold, no
/// stock taken, no `8B` / `45 03` at the start, the selection stays for the next online match.
#[test]
fn a_local_training_match_charges_no_boosters() {
    game_data::require_tables!();
    let dir = tempfile::tempdir().unwrap();
    let rec = Arc::new(Recorder::default());
    let cfg = LobbyConfig { achievement_rewards: false, daily_quests: false, treasure_hunt: false, ..LobbyConfig::default() };
    let mut e = Env::with(&dir.path().join("b.sqlite"), cfg, rec.clone());
    let mut c = e.client("test");
    e.store
        .update_profile(c.account, |p| {
            p.silver = 100_000;
            p.gold = 100;
            p.boosters.entry(339).or_default().enabled = true;
            p.boosters.entry(853).or_default().enabled = true;
            Ok::<(), ()>(())
        })
        .unwrap();
    e.sign_in(&mut c);
    e.handshake(&mut c);
    let p0 = e.store.load_profile(c.account).unwrap();
    c.send(Request::Play(Play5B::Play { map: 4, mode: 1, weather: 2 }));
    e.pump(&mut [&mut c]);
    let m = c.take();
    assert_eq!(ids(&m), [0x85, 0x83], "85 02 and the options only: no money or stock change at the start");
    let p1 = e.store.load_profile(c.account).unwrap();
    assert_eq!((p1.silver, p1.gold), (p0.silver, p0.gold), "nothing charged");
    assert_eq!(p1.boosters, p0.boosters, "the selection and the stock stay as they were");
    assert!(p1.boosters[&339].enabled && p1.boosters[&853].enabled);
}

#[test]
fn local_match_hook_training_tutorial_and_results() {
    game_data::require_tables!();
    let dir = tempfile::tempdir().unwrap();
    let rec = Arc::new(Recorder::default());
    let cfg = LobbyConfig { achievement_rewards: false, daily_quests: false, treasure_hunt: false, ..LobbyConfig::default() };
    let mut e = Env::with(&dir.path().join("m.sqlite"), cfg, rec.clone());
    let mut c = e.client("test");
    e.sign_in(&mut c);
    e.handshake(&mut c);
    // Play: 85 02, then the options of the trait
    c.send(Request::Play(Play5B::Play { map: 4, mode: 1, weather: 2 }));
    e.pump(&mut [&mut c]);
    let m = c.take();
    assert_eq!(m[0], protocol::s2c::local_match_state(), "85 02 -1 -1 3: a local match");
    assert_eq!(m[1], S2c::Reply(Reply::Play(PlayBlob { sub: 1, blob: vec![1, 2, 3, 4] })));
    // a 53 flush too early (before the match could have run) is not the end of the match
    c.send(Request::Events(vec![Event53 { id: 5, x: 0, value: 0, time: 0 }]));
    e.pump(&mut [&mut c]);
    assert!(c.take().is_empty());
    e.wait(&mut [&mut c], 31);
    let p0 = e.store.load_profile(c.account).unwrap();
    c.send(Request::Events(vec![Event53 { id: 5, x: 0, value: 0, time: 0 }]));
    e.pump(&mut [&mut c]);
    let m = c.take();
    // no drop popup (a local match drops nothing) and no B9: a local bots match counts for no achievement (CHOICE)
    assert_eq!(ids(&m), [0x97, 0x8B, 0x8C, 0x8E, 0x85], "no drop popup: a local match drops nothing");
    assert_eq!(m[0], S2c::Raw { id: 0x97, payload: vec![7, 7] });
    assert_eq!(m[4], S2c::QueueState(QueueState { state: 0 }));
    let p1 = e.store.load_profile(c.account).unwrap();
    assert_eq!((p1.silver, p1.xp, p1.points(1)), (p0.silver + 1000, p0.xp + 500, p0.points(1) + 50));
    // no spare parts or chests: a local match drops nothing
    assert_eq!(p1.parts, p0.parts);
    assert!(!p1.items.iter().any(|i| i.dict_id == 1872));
    // a second flush: no match, nothing happens
    c.send(Request::Events(vec![Event53 { id: 5, x: 0, value: 0, time: 0 }]));
    e.pump(&mut [&mut c]);
    assert!(c.take().is_empty());
    // a later 5B 02 starts the tutorial
    c.send(Request::Play(Play5B::Load));
    e.pump(&mut [&mut c]);
    let m = c.take();
    assert_eq!(m[0], protocol::s2c::local_match_state(), "85 02 -1 -1 3: a local match");
    assert_eq!(m[1], S2c::Reply(Reply::Play(PlayBlob { sub: 2, blob: vec![9; 5] })));
    e.wait(&mut [&mut c], 31);
    c.send(Request::Events(vec![Event53 { id: 6, x: 0, value: 0, time: 0 }]));
    e.pump(&mut [&mut c]);
    c.take();
    let calls = rec.calls.lock().unwrap().clone();
    assert_eq!(
        calls,
        [
            "training test 4 1 2 items 12",
            "finished test 4 1 won true fin true [5]",
            "tutorial test",
            "finished test 0 1 won false fin true [6]",
        ]
    );
}

/// 5B 03 (test a weapon in the shooting range): `85 02`, `83 5B 03 <blob of the hook>`; the next 53 flush after the
/// minimum time ends it with `85 00` only: no 97, no money, no experience, no drops.
#[test]
fn shooting_range_is_a_local_match_that_pays_nothing() {
    game_data::require_tables!();
    let dir = tempfile::tempdir().unwrap();
    let rec = Arc::new(Recorder::default());
    let cfg = LobbyConfig { achievement_rewards: false, daily_quests: false, treasure_hunt: false, ..LobbyConfig::default() };
    let mut e = Env::with(&dir.path().join("r.sqlite"), cfg, rec.clone());
    let mut c = e.client("test");
    e.sign_in(&mut c);
    e.handshake(&mut c);
    let p0 = e.store.load_profile(c.account).unwrap();
    let weapon = ItemRecord { uid: UID_BASE, dict: 107, value: 107, ..Default::default() };
    c.send(Request::Play(Play5B::ShootingRange { weapon, tail: 1 }));
    e.pump(&mut [&mut c]);
    let m = c.take();
    assert_eq!(m[0], protocol::s2c::local_match_state(), "85 02 -1 -1 3: a local match");
    assert_eq!(m[1], S2c::Reply(Reply::Play(PlayBlob { sub: 3, blob: vec![3; 6] })));
    assert_eq!(m.len(), 2, "{m:?}");
    // an early flush does not end it
    c.send(Request::Events(vec![Event53 { id: 5, x: 0, value: 0, time: 0 }]));
    e.pump(&mut [&mut c]);
    assert!(c.take().is_empty());
    e.wait(&mut [&mut c], 31);
    c.send(Request::Events(vec![Event53 { id: 5, x: 0, value: 0, time: 0 }]));
    e.pump(&mut [&mut c]);
    let m = c.take();
    assert_eq!(m, [S2c::QueueState(QueueState { state: 0 })]);
    let p1 = e.store.load_profile(c.account).unwrap();
    assert_eq!((p1.silver, p1.gold, p1.parts, p1.xp, p1.items.len(), p1.points(1)), (p0.silver, p0.gold, p0.parts, p0.xp, p0.items.len(), p0.points(1)), "the range changes nothing");
    assert_eq!(rec.calls.lock().unwrap().clone(), ["range test dict 107 uid 100001 profile 1"]);
    // a second flush: no range any more
    c.send(Request::Events(vec![Event53 { id: 5, x: 0, value: 0, time: 0 }]));
    e.pump(&mut [&mut c]);
    assert!(c.take().is_empty());
}

#[test]
fn default_match_hook_is_the_minimal_path() {
    game_data::require_tables!();
    let (_dir, mut e) = env();
    let mut c = e.client("test");
    e.sign_in(&mut c);
    e.handshake(&mut c);
    c.send(Request::Play(Play5B::Play { map: 1, mode: 1, weather: 0 }));
    e.pump(&mut [&mut c]);
    let m = c.take();
    assert_eq!(m[0], protocol::s2c::local_match_state(), "85 02 -1 -1 3: a local match");
    assert_eq!(m[1], S2c::Reply(Reply::Play(PlayBlob { sub: 1, blob: PlayBlob::minimal_blob() })));
    e.wait(&mut [&mut c], 31);
    c.send(Request::Events(vec![Event53 { id: 6, x: 0, value: 0, time: 0 }]));
    e.pump(&mut [&mut c]);
    let m = c.take();
    // no results screen and no rewards (the bots mode awards nothing): only 85 00
    assert_eq!(ids(&m), [0x85]);
}

// ---- several clients, restart ----

#[test]
fn two_clients_change_their_inventories_independently_and_restart_keeps_both() {
    game_data::require_tables!();
    let dir = tempfile::tempdir().unwrap();
    let path = db(&dir);
    let mut e = Env::new(&path);
    let mut a = e.client("alice");
    let mut b = e.client("bob");
    let ma = e.sign_in(&mut a);
    let mb = e.sign_in(&mut b);
    assert_eq!(account87(&ma).unwrap().body.name, "alice");
    assert_eq!(account87(&mb).unwrap().body.name, "bob");
    assert_eq!(e.server.registry.len(), 2);

    // alice buys an AK, sells her torso and swaps armour; bob exchanges gold and unequips the weapon
    a.send(buy_req(3, 1));
    a.send(Request::Shop(Q46::Sell { item: UID_BASE + 2, amount: 1 }));
    b.send(Request::Shop(Q46::Exchange { amount: 100 }));
    b.send(equip_req(vec![entry(1, 107, 7, 100, 0)]));
    e.pump(&mut [&mut a, &mut b]);
    let ra = a.take();
    let rb = b.take();
    assert!(replies(&ra).iter().any(|r| matches!(r, Reply::Shop(Shop46::Buy { items, .. }) if items.len() == 1)));
    assert!(replies(&rb).iter().all(|r| !matches!(r, Reply::Shop(_))), "bob got no shop answers of alice");
    let pa = e.store.load_profile(a.account).unwrap();
    let pb = e.store.load_profile(b.account).unwrap();
    assert!(pa.items.iter().any(|i| i.dict_id == 3) && pa.item(3).is_none() && pa.gold == 10_000);
    assert_eq!(pb.items.len(), 12);
    assert_eq!((pb.gold, pb.silver), (9_900, 1_000_000 + 100 * 250));
    assert!(pb.slots.get(&7).is_none() && pa.slots.get(&7).is_some());

    // restart: a new server over the same database; both sign in again and see their own state
    e.restart(&path, LobbyConfig::default(), Arc::new(MinimalMatches));
    let mut a2 = e.client_of("alice");
    let mut b2 = e.client_of("bob");
    let ma = e.sign_in(&mut a2);
    let mb = e.sign_in(&mut b2);
    let (Some(xa), Some(xb)) = (account87(&ma), account87(&mb)) else { panic!() };
    assert_eq!(xa.body.items.len(), pa.items.len());
    assert!(xa.body.items.iter().any(|r| r.dict == 3));
    assert!(xa.body.items.iter().all(|r| r.uid != 3));
    assert_eq!(xb.body.items.len(), 12);
    assert!(!xb.body.profiles[0].slots.contains_key(&7));
    assert_eq!(money(&ma).unwrap().silver, pa.silver);
    assert_eq!(money(&mb).unwrap().gold, 9_900);
    // the module seed ran once: no second set of modules after the restart
    assert_eq!(xb.body.items.len(), 12);
}

#[test]
fn login_session_table_is_the_handover() {
    game_data::require_tables!();
    let (_d, e) = env();
    let id = e.store.create_account("someone", "pw").unwrap();
    let s = e.sessions.grant(id);
    assert_eq!(e.sessions.redeem(s), Some(id));
    assert_eq!(e.sessions.redeem(s ^ 1), None);
    assert_eq!(e.sessions.redeem(0), None);
}

/// The client's lobby reconnect after a server restart (PROVEN with the real client): the client keeps the session of its sign-in
/// and sends the same 2E on a new UDP connection. The restarted server must still know it: the account signs in with
/// the full login sequence, no new sign-in at the login server.
#[test]
fn session_survives_a_server_restart() {
    game_data::require_tables!();
    let (dir, mut e) = env();
    let mut c = e.client("roamer");
    let first = e.sign_in(&mut c);
    assert!(first.iter().any(|m| matches!(m, S2c::SignedIn(_))));
    e.restart(&db(&dir), LobbyConfig::default(), Arc::new(MinimalMatches));
    assert!(e.sessions.is_empty(), "nothing in memory after the restart");
    let mut again = e.reconnect(&c);
    let msgs = e.sign_in(&mut again);
    let s = msgs.iter().find_map(|m| if let S2c::SignedIn(s) = m { Some(*s) } else { None }).expect("80 after the restart");
    assert_eq!(s.account_id, accounts::public_id(c.account));
    // the whole login sequence follows (the client goes through its loading state again)
    assert!(ids(&msgs).contains(&0xB8) && ids(&msgs).contains(&0x87), "{:x?}", ids(&msgs));
    assert!(e.server.registry.is_online(c.account));
    assert_eq!(e.server.registry.session_of(c.account), Some(c.session));
    // a session the server never granted: refused with FD (the client closes the connection and runs its own
    // reconnect; after six drops it opens the login menu), nothing else
    let mut forged = e.reconnect(&c);
    forged.session ^= 1;
    assert!(e.sign_in(&mut forged).is_empty());
    assert!(forged.closed, "FD reached the client");
}

/// A session outlives its sign-in, so a ban given since then refuses its 2E (as the login server refuses a fresh
/// sign-in of a banned account): FD, not silence.
#[test]
fn reconnect_of_a_banned_account_is_refused() {
    game_data::require_tables!();
    let (dir, mut e) = env();
    let mut c = e.client("outlaw");
    e.sign_in(&mut c);
    e.store.set_ban(c.account, accounts::BanKind::Suspended, 3, 60).unwrap();
    e.restart(&db(&dir), LobbyConfig::default(), Arc::new(MinimalMatches));
    let mut again = e.reconnect(&c);
    assert!(e.sign_in(&mut again).is_empty());
    assert!(again.closed);
    assert!(!e.server.registry.is_online(c.account));
    e.store.clear_ban(c.account).unwrap();
    let mut later = e.reconnect(&c);
    assert!(e.sign_in(&mut later).iter().any(|m| matches!(m, S2c::SignedIn(_))));
    assert!(!later.closed);
}

/// An expired session (time to live after its last use, e.g. 1 h idle) is refused like an unknown one (an unanswered 2E
/// leaves the client hanging until the game is restarted).
#[test]
fn expired_session_is_refused_with_fd() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let c = e.client("sleeper");
    // every session is past its time to live
    e.sessions = Arc::new(lobby_server::SessionTable::persistent_with_ttl(e.store.clone(), Duration::ZERO));
    e.server = build_server(&e.store, &e.sessions, LobbyConfig::default(), Arc::new(MinimalMatches));
    let mut again = e.reconnect(&c);
    assert!(e.sign_in(&mut again).is_empty());
    assert!(again.closed);
    assert_eq!(again.low, vec![0xFD]);
    // the refused connection handles nothing more: a second 2E (or any request) gets no answer, only another FD
    again.send(Request::SignIn { session: c.session, a: 0xFF, b: 0xFF });
    e.pump(&mut [&mut again]);
    assert!(again.take().is_empty());
    assert!(again.low.len() >= 2 && again.low.iter().all(|t| *t == 0xFD), "{:x?}", again.low);
    // it is dropped once it goes quiet
    e.step(&mut [], Duration::from_secs(11));
    assert!(!e.server.peers.contains_key(&again.addr));
}

/// The client's FA (it ends the connection itself, e.g. back to the login screen) gets the FB it waits for, before the
/// peer is dropped: without the FB the client repeats its FA and never gets back to the login screen.
#[test]
fn client_disconnect_gets_its_fb() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("leaver");
    e.sign_in(&mut c);
    assert!(e.server.registry.is_online(c.account));
    c.t.initiate_disconnect();
    e.pump(&mut [&mut c]);
    assert!(c.closed, "FB reached the client");
    assert_eq!(c.low, vec![0xFB]);
    assert!(!e.server.registry.is_online(c.account));
    assert!(!e.server.peers.contains_key(&c.addr));
    // a refused connection's FA gets its FB too
    let mut forged = e.reconnect(&c);
    forged.session ^= 1;
    e.sign_in(&mut forged);
    assert!(forged.closed && forged.low.contains(&0xFD));
    forged.t.initiate_disconnect();
    e.pump(&mut [&mut forged]);
    assert_eq!(forged.low.last(), Some(&0xFB), "{:x?}", forged.low);
    assert!(!e.server.peers.contains_key(&forged.addr));
}

/// A connection that sends no 2E within 15 s (e.g. the old connection of a client after a quick server restart:
/// keepalives acked, requests unanswered) is closed with FD, so the client runs its own reconnect with the 2E.
#[test]
fn connection_without_sign_in_is_closed_after_15_s() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("silent");
    c.send(Request::Skills { skills: vec![(112, 1)] });
    e.wait(&mut [&mut c], 14);
    assert!(!c.closed);
    e.wait(&mut [&mut c], 2);
    assert!(c.closed);
    assert!(c.take().is_empty());
    // its reconnect (new connection, same 2E) signs in
    let mut again = e.reconnect(&c);
    assert!(e.sign_in(&mut again).iter().any(|m| matches!(m, S2c::SignedIn(_))));
}

// ---- social basics ----

#[test]
fn friends_ignore_and_search_with_two_accounts() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut a = e.client("alice");
    let mut b = e.client("bob");
    e.sign_in(&mut a);
    e.sign_in(&mut b);
    let id_a = accounts::public_id(a.account);
    let id_b = accounts::public_id(b.account);
    let friends = |m: &[S2c]| -> Vec<(u64, u8)> {
        match m.iter().rev().find(|x| matches!(x, S2c::Messaging(Messaging::Friends(_)))) {
            Some(S2c::Messaging(Messaging::Friends(v))) => v.iter().map(|p| (p.id, p.status)).collect(),
            _ => panic!("no friend list in {m:?}"),
        }
    };
    // alice asks bob: her own list stays empty (request sent); bob gets the client's own Requests entry (B1 type 0),
    // not a status-4 row
    a.send(Request::People { id: id_b, kind: 0, text: String::new() });
    e.pump(&mut [&mut a, &mut b]);
    assert_eq!(friends(&a.take()), []);
    let mb = b.take();
    assert_eq!(friends(&mb), []);
    assert!(mb.iter().any(|x| matches!(x, S2c::SystemMessage(s) if s.kind == 0 && s.sender == id_a)), "{mb:?}");
    // bob accepts (asks back): friends, both online
    b.send(Request::People { id: id_a, kind: 0, text: String::new() });
    e.pump(&mut [&mut a, &mut b]);
    assert_eq!(friends(&a.take()), [(id_b, 0)]);
    assert_eq!(friends(&b.take()), [(id_a, 0)]);
    // search
    a.send(Request::Friends(Friends4E::Find { query: "bo".into() }));
    a.send(Request::Friends(Friends4E::Find { query: String::new() }));
    e.pump(&mut [&mut a, &mut b]);
    let m = a.take();
    let S2c::Messaging(Messaging::Found(rows)) = &m[0] else { panic!("{m:?}") };
    assert_eq!((rows.len(), rows[0].id, rows[0].name.as_str()), (1, id_b, "bob"));
    assert_eq!(m[1], S2c::Messaging(Messaging::Found(vec![])));
    // ignore and un-ignore
    a.send(Request::Friends(Friends4E::AddIgnore { id: id_b }));
    e.pump(&mut [&mut a, &mut b]);
    assert_eq!(a.take()[0], S2c::Messaging(Messaging::Ignored(vec![(id_b, "bob".into())])));
    a.send(Request::Friends(Friends4E::RemoveIgnored { id: id_b }));
    a.send(Request::Friends(Friends4E::Remove { id: id_b }));
    e.pump(&mut [&mut a, &mut b]);
    let m = a.take();
    assert_eq!(m[0], S2c::Messaging(Messaging::Ignored(vec![])));
    assert_eq!(friends(&m), []);
    assert_eq!(friends(&b.take()), []);
}

#[test]
fn the_senders_own_chat_line_is_never_echoed() {
    game_data::require_tables!();
    // the client prints its own line itself (PROVEN 0x2643E0)
    let (_d, mut e) = env();
    let mut c = e.client("test");
    e.sign_in(&mut c);
    c.send(Request::Chat { channel: 1, target: 0, text: "hello".into() });
    e.pump(&mut [&mut c]);
    assert!(c.take().is_empty());
}

#[test]
fn requests_before_sign_in_are_ignored() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    c.send(buy_req(3, 1));
    e.pump(&mut [&mut c]);
    assert!(c.inbox.is_empty());
    assert_eq!(e.store.load_profile(c.account).unwrap().silver, 1_000_000);
    let _ = Currency::Silver;
}


// ---- module merge (45 10), premium vouchers, shop prices ----

/// Give the profile `n` modules of one dict with rarity `r` (group 312 grade); returns their uids.
fn give_modules(e: &Env, id: accounts::AccountId, n: usize, rarity: Option<u16>) -> Vec<u32> {
    let d = gd();
    let dict = d.items.iter().find(|i| d.items.is_module(i.dict_id) && d.shop.price(i.dict_id).is_some()).unwrap().dict_id;
    e.store
        .update_profile(id, |p| {
            let mut v = vec![];
            for _ in 0..n {
                let u = p.add_item(dict, dict as u32);
                if let Some(r) = rarity {
                    p.mods.insert(u, vec![(312, r)]);
                }
                v.push(u);
            }
            Ok::<_, ()>(v)
        })
        .unwrap()
}

fn merge(c: &mut Client, e: &mut Env, items: [u32; 4], pay: u8) -> Vec<S2c> {
    c.send(Request::Inventory(Q45::ModuleUpgrade { items, pay }));
    e.pump(&mut [&mut *c]);
    c.take()
}

/// Seed the lobby generator so that the next craft roll at `chance` percent succeeds (or misses).
fn seed_roll(e: &mut Env, success: bool, chance: f32) {
    use game_data::craft::{CraftData, SplitMix};
    let seed = (1u64..).find(|s| CraftData::roll_success(chance, &mut SplitMix(*s)) == success).unwrap();
    e.server.lobby.seed(seed);
}

fn rarity(m: &[(u16, u16)]) -> u16 {
    game_data::merge::rarity(m)
}

/// The module's drop modifier grades lie in a cell of rarity `r` (`modules_rarity`; one-modifier modules: the
/// client's column).
fn assert_drop_cells(dict: u16, mods: &[(u16, u16)], r: u16) {
    let d = gd();
    let md = &d.module_drops;
    let (a, b) = md.drop_groups(dict);
    let grade = |g: Option<u16>| g.and_then(|g| mods.iter().find(|p| p.0 == g)).map(|p| p.1 as usize);
    let v = game_data::module_drops::rarity_value(r).unwrap();
    match (grade(a), grade(b)) {
        (Some(i), Some(j)) => assert_eq!(md.cell_value(i, j), v, "{dict} {mods:?}"),
        (Some(i), None) => {
            let (lo, hi, _, _) = md.client_bounds(v, true).unwrap();
            assert!((lo..=hi).contains(&i), "{dict} {mods:?}");
        }
        _ => assert!(a.is_none(), "{dict}: drop pairs missing in {mods:?}"),
    }
}

fn refused(m: &[S2c]) -> bool {
    matches!(&m[0], S2c::Reply(Reply::Inventory(Inventory45::ModuleUpgrade { result: 2, .. })))
}

#[test]
fn module_merge_consumes_donors_charges_gold_and_raises_the_rarity() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    e.sign_in(&mut c);
    let u = give_modules(&e, c.account, 4, None);
    let gold0 = e.store.load_profile(c.account).unwrap().gold;
    let m = merge(&mut c, &mut e, [u[0], u[1], u[2], u[3]], 1);
    // the answer first, then the records (module with the new rarity, donors with count 0), then 8B
    let S2c::Reply(Reply::Inventory(Inventory45::ModuleUpgrade { result, uid, mods })) = &m[0] else { panic!("{m:?}") };
    assert_eq!((*result, *uid, rarity(mods)), (1, u[0], 1));
    let S2c::Reply(Reply::Inventory(Inventory45::Records { items, .. })) = &m[1] else { panic!("{m:?}") };
    assert_eq!(items[0].uid, u[0]);
    // the reply's list is the record's: rarity 312 = green, the drop grades redrawn inside the green cells
    assert_eq!(&items[0].mods, mods);
    assert_drop_cells(items[0].dict, &items[0].mods, 1);
    assert!(items[1..].iter().all(|r| r.value == 0) && items.len() == 4, "{items:?}");
    // a grey module's gold button shows 0 (`premium_to_craft[rarity - 1]` reads index -1) and costs 0
    let p = e.store.load_profile(c.account).unwrap();
    assert_eq!(p.gold, gold0);
    assert_eq!(money(&m).unwrap().gold, gold0);
    assert!(p.item(u[0]).is_some() && u[1..].iter().all(|x| p.item(*x).is_none()));
    assert_eq!(rarity(&p.mods[&u[0]]), 1);
    // green -> blue costs 10 gold (`premium_to_craft[0]`); a duplicate or a donor of another rarity is refused
    // without a charge
    let more = give_modules(&e, c.account, 3, Some(1));
    assert!(refused(&merge(&mut c, &mut e, [u[0], more[0], more[1], u[0]], 1)));
    let grey = give_modules(&e, c.account, 1, None)[0];
    assert!(refused(&merge(&mut c, &mut e, [u[0], more[0], more[1], grey], 1)));
    assert_eq!(e.store.load_profile(c.account).unwrap().gold, gold0);
    let m = merge(&mut c, &mut e, [u[0], more[0], more[1], more[2]], 1);
    assert!(matches!(&m[0], S2c::Reply(Reply::Inventory(Inventory45::ModuleUpgrade { result: 1, .. }))), "{m:?}");
    let p = e.store.load_profile(c.account).unwrap();
    assert_eq!(p.gold, gold0 - 10);
    assert_eq!(money(&m).unwrap().gold, gold0 - 10);
    assert_eq!(rarity(&p.mods[&u[0]]), 2);
    assert_drop_cells(p.item(u[0]).unwrap().dict_id, &p.mods[&u[0]], 2);
    // blue -> orange costs 25, orange -> legendary 50
    let blue = give_modules(&e, c.account, 3, Some(2));
    merge(&mut c, &mut e, [u[0], blue[0], blue[1], blue[2]], 1);
    assert_eq!(e.store.load_profile(c.account).unwrap().gold, gold0 - 10 - 25);
    let orange = give_modules(&e, c.account, 3, Some(3));
    merge(&mut c, &mut e, [u[0], orange[0], orange[1], orange[2]], 1);
    let p = e.store.load_profile(c.account).unwrap();
    assert_eq!(p.gold, gold0 - 10 - 25 - 50);
    assert_eq!(rarity(&p.mods[&u[0]]), 5);
    // legendary = the cell (10, 10): both drop modifiers at their best grade
    assert_drop_cells(p.item(u[0]).unwrap().dict_id, &p.mods[&u[0]], 5);
}

#[test]
fn module_merge_roll_follows_the_wiki_chances() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    e.sign_in(&mut c);
    let u = give_modules(&e, c.account, 8, Some(1));
    let gold0 = e.store.load_profile(c.account).unwrap().gold;
    // free button, 50 %: a missed roll answers result 0 (neutral, no modifier list), consumes the donors, the module keeps its rarity
    seed_roll(&mut e, false, 50.0);
    let m = merge(&mut c, &mut e, [u[0], u[1], u[2], u[3]], 0);
    let S2c::Reply(Reply::Inventory(Inventory45::ModuleUpgrade { result, uid, .. })) = &m[0] else { panic!("{m:?}") };
    assert_eq!((*result, *uid), (0, u[0]));
    let S2c::Reply(Reply::Inventory(Inventory45::Records { items, .. })) = &m[1] else { panic!("{m:?}") };
    assert_eq!(items.len(), 4, "{items:?}");
    assert!(items[1..].iter().all(|r| r.value == 0));
    let p = e.store.load_profile(c.account).unwrap();
    assert_eq!(p.mods[&u[0]], vec![(312, 1)]);
    assert!(p.item(u[0]).is_some() && u[1..4].iter().all(|x| p.item(*x).is_none()));
    assert_eq!(p.gold, gold0, "the free button costs no gold");
    // free button, success
    seed_roll(&mut e, true, 50.0);
    let m = merge(&mut c, &mut e, [u[4], u[5], u[6], u[7]], 0);
    assert!(matches!(&m[0], S2c::Reply(Reply::Inventory(Inventory45::ModuleUpgrade { result: 1, .. }))), "{m:?}");
    assert_eq!(rarity(&e.store.load_profile(c.account).unwrap().mods[&u[4]]), 2);
    // gold button: 100 %, whatever the generator says
    for seed in [1u64, 2, 3, 4, 5] {
        e.server.lobby.seed(seed);
        let ms = give_modules(&e, c.account, 4, Some(1));
        let m = merge(&mut c, &mut e, [ms[0], ms[1], ms[2], ms[3]], 1);
        assert!(matches!(&m[0], S2c::Reply(Reply::Inventory(Inventory45::ModuleUpgrade { result: 1, .. }))), "seed {seed}: {m:?}");
    }
}

/// A gold (bought) module merges for 50 gold (`premium_to_craft[min(4 - 1, 2)]`) into a legendary one whose drop grades
/// are both 10 (the Mastercraft cell); its capacity pair stays.
#[test]
fn a_gold_module_merges_into_legendary_with_the_best_grades() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    e.sign_in(&mut c);
    let d = gd();
    let bought = d.modules.bought_mods(1991).unwrap();
    let u: Vec<u32> = e
        .store
        .update_profile(c.account, |p| {
            p.gold = 100;
            Ok::<_, ()>((0..4).map(|_| {
                let u = p.add_item(1991, d.items.amount_field(1991, 1));
                p.mods.insert(u, bought.clone());
                u
            }).collect())
        })
        .unwrap();
    let m = merge(&mut c, &mut e, [u[0], u[1], u[2], u[3]], 1);
    let S2c::Reply(Reply::Inventory(Inventory45::ModuleUpgrade { result: 1, mods, .. })) = &m[0] else { panic!("{m:?}") };
    assert_eq!(mods, &vec![(312, 5), (311, 10), (407, 10), (520, 7)]);
    let p = e.store.load_profile(c.account).unwrap();
    assert_eq!(p.gold, 50);
    assert_eq!(p.mods[&u[0]], vec![(312, 5), (311, 10), (407, 10), (520, 7)]);
}

#[test]
fn module_merge_free_button_and_refusals_charge_nothing() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    e.sign_in(&mut c);
    let u = give_modules(&e, c.account, 4, Some(3));
    e.store.update_profile(c.account, |p| { p.gold = 0; Ok::<_, ()>(()) }).unwrap();
    let n0 = e.store.load_profile(c.account).unwrap().items.len();
    // no gold: the gold button is refused, the free one (pay 0) costs nothing
    assert!(refused(&merge(&mut c, &mut e, [u[0], u[1], u[2], u[3]], 1)));
    assert_eq!(e.store.load_profile(c.account).unwrap().items.len(), n0);
    seed_roll(&mut e, true, 50.0);
    let m = merge(&mut c, &mut e, [u[0], u[1], u[2], u[3]], 0);
    // orange (3) merges into legendary (5)
    assert!(matches!(&m[0], S2c::Reply(Reply::Inventory(Inventory45::ModuleUpgrade { result: 1, .. }))), "{m:?}");
    assert_eq!(rarity(&e.store.load_profile(c.account).unwrap().mods[&u[0]]), 5);
    // legendary cannot be merged; duplicates and unknown uids are refused
    let more = give_modules(&e, c.account, 3, Some(5));
    for items in [[u[0], more[0], more[1], more[2]], [more[0], more[0], more[1], more[2]], [u[0], 999_999, 1, 2]] {
        assert!(refused(&merge(&mut c, &mut e, items, 0)));
    }
}

#[test]
fn installed_donor_is_detached_and_the_weapon_record_refreshed() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    e.sign_in(&mut c);
    let d = gd();
    let u = give_modules(&e, c.account, 4, None);
    let weapon = e.store.load_profile(c.account).unwrap().items.iter().find(|i| d.items.is_weapon(i.dict_id)).unwrap().uid;
    e.store.update_profile(c.account, |p| { p.attach.entry(weapon).or_default().insert("slot_x".into(), u[3]); Ok::<_, ()>(()) }).unwrap();
    let m = merge(&mut c, &mut e, [u[0], u[1], u[2], u[3]], 1);
    let S2c::Reply(Reply::Inventory(Inventory45::Records { items, .. })) = &m[1] else { panic!("{m:?}") };
    assert!(items.iter().any(|r| r.uid == weapon && r.modules.is_empty()), "{items:?}");
    assert!(account87(&m).is_some(), "the lobby character is rebuilt");
    assert!(e.store.load_profile(c.account).unwrap().owner_of(u[3]).is_none());
}

/// The default config carries the server's 7-day login calendar of boosters and vouchers: the 94 at sign-in holds the 0x1B
/// record (star on day 7 only), and on 7 consecutive days `5E` (whatever id it carries: the real client sent 2373604912
/// and 2371704368 for our record id 1) puts that day's item into the inventory, sends its record with the received-item
/// popup (`83 46 00` flag 1), and the next 94 marks the day received; a second claim on a day changes nothing.
#[test]
fn default_login_calendar_grants_each_days_item() {
    game_data::require_tables!();
    let dir = tempfile::tempdir().unwrap();
    let cfg = LobbyConfig { achievement_rewards: false, ..LobbyConfig::default() };
    let days = cfg.login_calendar.clone().unwrap();
    assert_eq!(days.iter().map(|d| (d.reward, d.amount)).collect::<Vec<_>>(), [(340, 1), (341, 2), (342, 2), (1132, 1), (861, 1), (853, 1), (857, 3)]);
    let mut e = Env::with(&dir.path().join("cal.sqlite"), cfg, Arc::new(lobby_server::MinimalMatches));
    let mut c = e.client("test");
    let msgs = e.sign_in(&mut c);
    let calendar = |m: &[S2c]| -> protocol::s2c::quests::BonusBody {
        let n = m.iter().find_map(|x| if let S2c::Notices(n) = x { Some(n) } else { None }).expect("a 94");
        let cal = n.notices.iter().find(|x| x.kind == 0x1B).expect("the calendar record");
        <protocol::s2c::quests::BonusBody as protocol::Decode>::from_bytes(&protocol::lzma2::decompress(&cal.data2).unwrap()).unwrap()
    };
    // the client draws the star for award_quality 1 only (BonusDayRenderer.setData): day 7, the big reward
    assert_eq!(calendar(&msgs).days.iter().map(|d| d.quality).collect::<Vec<_>>(), [0, 0, 0, 0, 0, 0, 1]);
    let acct = c.account;
    let count = |e: &Env, dict: u16| e.store.load_profile(acct).unwrap().items.iter().filter(|i| i.dict_id == dict).map(|i| i.count).sum::<u32>();
    let today = lobby_server::login_calendar::day_of(lobby_server::premium::unix_now());
    let ids = [2_373_604_912u32, 2_371_704_368, 1, 7, 0, 99, u32::MAX];
    for (i, d) in days.iter().enumerate() {
        if i > 0 {
            // day i of the streak: yesterday paid day i - 1
            e.store
                .update_profile(acct, |p| {
                    p.calendar = accounts::CalendarState { last_day: today - 1, day: i as u8 - 1, received_mask: (1u64 << i) - 1 };
                    Ok::<(), ()>(())
                })
                .unwrap();
        }
        let before = count(&e, d.reward);
        let silver = e.store.load_profile(acct).unwrap().silver;
        c.take();
        c.send(Request::DailyReward { a: ids[i] });
        e.pump(&mut [&mut c]);
        assert_eq!(count(&e, d.reward), before + d.amount, "day {} dict {}", i + 1, d.reward);
        assert_eq!(e.store.load_profile(acct).unwrap().silver, silver, "an item day pays no silver");
        let got = c.take();
        let popup: Vec<_> = got
            .iter()
            .filter_map(|m| if let S2c::Reply(Reply::Shop(Shop46::Buy { flag: 1, items })) = m { Some(items.clone()) } else { None })
            .flatten()
            .collect();
        assert!(popup.iter().any(|r| r.dict == d.reward && r.value == d.amount && r.uid != 0), "day {}: {popup:?}", i + 1);
        let st = calendar(&got).state.unwrap();
        assert_eq!(st.current_day as usize, i);
        assert!(st.received_mask >> i & 1 == 1, "day {} received: the dialog does not open again", i + 1);
        // a second collection on the same day changes nothing
        c.send(Request::DailyReward { a: 1 });
        e.pump(&mut [&mut c]);
        assert_eq!(count(&e, d.reward), before + d.amount);
    }
}

/// The lobby's booster panel: a gold slot (853, "for one match", 5 gold in "Match costs") is
/// selected with 45 0C without paying, and 45 0D on the selected slot unselects it. The client changes no slot itself,
/// so every request, refused or not, is answered with the 94 records as they are now (`root.set_active_boosters`).
#[test]
fn booster_panel_selects_and_unselects_a_gold_booster_without_paying() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    e.sign_in(&mut c);
    let acc = c.account;
    e.store.update_profile(acc, |p| Ok::<_, ()>(p.gold = 50)).unwrap();
    let boosters = |m: &[S2c]| -> Option<Vec<u32>> {
        let S2c::Notices(n) = m.iter().find(|x| matches!(x, S2c::Notices(_)))? else { unreachable!() };
        Some(n.notices.iter().filter(|r| r.kind == lobby_server::boosters::KIND_BOOSTER).map(|r| r.f40).collect())
    };
    let mut ask = |e: &mut Env, r: Q45| {
        c.take();
        c.send(Request::Inventory(r));
        e.pump(&mut [&mut c]);
        c.take()
    };
    let m = ask(&mut e, Q45::BoosterEnable { dict: 853 });
    assert_eq!(boosters(&m), Some(vec![853]), "{m:?}");
    assert!(m.iter().all(|x| x.id() != 0x8B), "nothing is paid on select");
    assert_eq!(e.store.load_profile(acc).unwrap().gold, 50);
    // the selected gold slot is clicked again
    let m = ask(&mut e, Q45::BoosterDisable { dict: 853 });
    assert_eq!(boosters(&m), Some(vec![]), "{m:?}");
    // a refused request still gets the current records, so the slots match the server
    let m = ask(&mut e, Q45::BoosterDisable { dict: 853 });
    assert_eq!(boosters(&m), Some(vec![]));
    // replace within the gold slots, and a silver booster next to them
    ask(&mut e, Q45::BoosterEnable { dict: 857 });
    let m = ask(&mut e, Q45::BoosterReplace { old: 857, new: 861 });
    assert_eq!(boosters(&m), Some(vec![861]));
    let m = ask(&mut e, Q45::BoosterEnable { dict: 339 });
    assert_eq!(boosters(&m), Some(vec![339, 861]));
    let p = e.store.load_profile(acc).unwrap();
    assert_eq!((p.gold, lobby_server::boosters::selected(&e.server.lobby.data, &p)), (50, vec![339, 861]));
}

#[test]
fn premium_voucher_extends_the_account_and_push_94_shows_it() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    e.sign_in(&mut c);
    let v = e.store.update_profile(c.account, |p| Ok::<_, ()>(p.add_item(213, 2))).unwrap(); // premium_access_voucher_1d x2
    c.take();
    c.send(Request::Inventory(Q45::Use { item: v }));
    e.pump(&mut [&mut c]);
    let m = c.take();
    assert_eq!(m[0], S2c::Reply(Reply::Inventory(Inventory45::Used { item: v })));
    let S2c::Notices(n) = m.iter().find(|x| matches!(x, S2c::Notices(_))).unwrap() else { unreachable!() };
    assert_eq!(n.notices.len(), 1);
    assert_eq!(n.notices[0].kind, 0);
    assert!((86_300..=86_400).contains(&n.notices[0].f38), "{}", n.notices[0].f38);
    assert_eq!(e.store.load_profile(c.account).unwrap().item(v).unwrap().count, 1);
    // the second voucher adds to the running premium; the last one removes the row
    c.send(Request::Inventory(Q45::Use { item: v }));
    e.pump(&mut [&mut c]);
    c.take();
    let p = e.store.load_profile(c.account).unwrap();
    assert!(p.item(v).is_none());
    assert!(p.premium_left(lobby_server::premium::unix_now()) > 2 * 86_300);
    // a non-voucher item is not touched by this module
    let other = p.items[0].uid;
    c.send(Request::Inventory(Q45::Use { item: other }));
    e.pump(&mut [&mut c]);
    assert!(c.take().iter().all(|x| !matches!(x, S2c::Reply(Reply::Inventory(Inventory45::Used { .. })))));
}

#[test]
fn prices_come_from_the_dictionary_or_the_item_is_not_sold() {
    game_data::require_tables!();
    let d = gd();
    let p = d.shop.price(213).unwrap(); // premium_access_voucher_1d: 50 gold
    assert_eq!((p.a, p.kind, p.for_sale), (50, 1, true));
    let p = d.shop.price(1021).unwrap(); // 7d: 200 gold
    assert_eq!((p.a, p.kind), (200, 1));
    let p = d.shop.price(339).unwrap(); // weight_booster: 500 silver
    assert_eq!((p.a, p.kind), (500, 0));
    assert!(!d.shop.price(1088).unwrap().for_sale); // new_player_booster: cost 0
    // modules: spare parts price = dictionary cost, gold price = a / 20 (CHOICE), cost 0 = not sold
    for it in d.items.iter().filter(|i| d.items.is_module(i.dict_id)) {
        let p = d.shop.price(it.dict_id).unwrap();
        assert_eq!(p.c, it.cost_int() / 20, "module {} gold price", it.dict_id);
        assert_eq!(p.for_sale, it.cost_int() > 0);
    }
    // fan-wiki Items page (May 2022) fills items without a faction-shop price: RGD-5 30, bandages 20, smoke grenade 50,
    // PMN-3 150, MON-50 200, Scavengers medkit 50; a faction-shop price wins (Anesthetic 25, Antitoxin 20, Military medkit 40)
    for (dict, silver) in [(5u16, 30u32), (66, 20), (67, 50), (219, 50), (734, 150), (736, 200), (65, 25), (82, 20), (759, 40)] {
        let p = d.shop.price(dict).unwrap();
        assert_eq!((p.a, p.kind, p.for_sale), (silver, 0, true), "dict {dict}");
        assert_eq!(d.shop.buy_total(&d.items, dict, 2), Some(silver as u64 * 2), "dict {dict}");
    }
    // RGO has no 0.69d or wiki price (wiki '?'): our own CHOICE price 40 (docs/notes/store.md section 5); the event
    // specials Nut and Snowball stay unsold
    assert_eq!(d.shop.buy_total(&d.items, 733, 1), Some(40));
    assert!(d.shop.is_choice(733) && !d.shop.is_choice(5));
    assert!(d.shop.buy_total(&d.items, 1355, 1).is_none() && d.shop.buy_total(&d.items, 1884, 1).is_none());
}

#[test]
fn module_gold_buy_costs_a_twentieth_and_zero_cost_modules_are_not_for_sale() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    e.sign_in(&mut c);
    let d = gd();
    let weapon = *e.store.load_profile(c.account).unwrap().items.iter().find(|i| d.items.is_weapon(i.dict_id) && !d.modules.weapon_slot_ids(i.dict_id).is_empty()).unwrap();
    let slot = d.modules.weapon_slot_ids(weapon.dict_id)[0].clone();
    let priced = d.items.iter().find(|i| d.items.is_module(i.dict_id) && i.cost_int() > 0).unwrap().dict_id;
    let free = d.items.iter().find(|i| d.items.is_module(i.dict_id) && i.cost_int() == 0).unwrap().dict_id;
    let p0 = e.store.load_profile(c.account).unwrap();
    let act = |kind: u8, dict: u16| ModuleAction { kind, f24: 0, module_uid: 900, module_dict: dict, slot: Some(slot.clone()), cost: 1, f2d: 1, bound: 0 };
    c.send(Request::Workshop(Workshop60::Save { weapon: weapon.uid, weapon_dict: weapon.dict_id, actions: vec![act(3, priced), act(4, free), act(3, free)] }));
    e.pump(&mut [&mut c]);
    c.take();
    // gold price = spare-parts price / 20 (CHOICE), not the client's cost field; cost 0 = not sold
    let p1 = e.store.load_profile(c.account).unwrap();
    let a = d.items.get(priced).unwrap().cost_int();
    assert_eq!((p1.items.len(), p1.gold, p1.parts), (p0.items.len() + 1, p0.gold - a / 20, p0.parts));
    assert!(p1.items.iter().any(|i| i.dict_id == priced && p0.item(i.uid).is_none()));
}

// ---- one-time awards, first game, decals ----

#[test]
fn tutorial_awards_are_paid_once_from_4a_01() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    e.sign_in(&mut c);
    let p0 = e.store.load_profile(c.account).unwrap();
    c.take();
    c.send(Request::Quest(Quest4A::Sub01 { a: 1 }));
    e.pump(&mut [&mut c]);
    let m = c.take();
    assert!(ids(&m).contains(&0x8B), "{:?}", ids(&m));
    let p1 = e.store.load_profile(c.account).unwrap();
    let g = gd();
    // tutorial_completed: 2500 silver (type 2 payment 0), 1000 experience (type 0), 200 spare parts (type 2 payment 4)
    let level_silver = g.levels.level_up_silver(p0.xp, p0.xp + 1000);
    assert_eq!((p1.xp, p1.parts), (p0.xp + 1000, p0.parts + 200));
    assert_eq!(p1.silver, p0.silver + 2500 + level_silver);
    assert!(p1.claimed(accounts::CLAIM_TUTORIAL));
    // a second 4A 01 pays nothing
    c.send(Request::Quest(Quest4A::Sub01 { a: 1 }));
    e.pump(&mut [&mut c]);
    c.take();
    assert_eq!(e.store.load_profile(c.account).unwrap(), p1);
}

#[test]
fn a_local_bots_match_pays_nothing_and_keeps_the_first_game_bonus() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    e.sign_in(&mut c);
    e.handshake(&mut c);
    let (xp0, silver0) = {
        let p = e.store.load_profile(c.account).unwrap();
        (p.xp, p.silver)
    };
    for round in 0..2 {
        c.send(Request::Play(Play5B::Play { map: 1, mode: 1, weather: 0 }));
        e.pump(&mut [&mut c]);
        c.take();
        e.wait(&mut [&mut c], 31);
        c.send(Request::Events(vec![Event53 { id: 6, x: 0, value: 0, time: 0 }]));
        e.pump(&mut [&mut c]);
        c.take();
        let p = e.store.load_profile(c.account).unwrap();
        // "Rewards in this mode are not awarded" (client hint st_match_settings_bots_hint)
        assert_eq!((p.xp, p.silver), (xp0, silver0), "round {round}");
        assert!(!p.claimed(accounts::CLAIM_FIRST_GAME), "the bonus is kept for a real match");
    }
}

#[test]
fn the_tutorial_match_pays_its_awards_and_keeps_the_first_game() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    e.sign_in(&mut c);
    e.handshake(&mut c);
    c.send(Request::Play(Play5B::Load));
    e.pump(&mut [&mut c]);
    c.take();
    e.wait(&mut [&mut c], 31);
    c.send(Request::Events(vec![Event53 { id: 6, x: 0, value: 0, time: 0 }]));
    e.pump(&mut [&mut c]);
    c.take();
    let p = e.store.load_profile(c.account).unwrap();
    assert!(p.claimed(accounts::CLAIM_GAME_WORLD_TUTORIAL));
    assert!(!p.claimed(accounts::CLAIM_FIRST_GAME), "the tutorial is not the account's first game");
    // game_world_tutorial_completed row 1: type 3 item 1132 (drop_card_free) x1
    assert!(p.items.iter().any(|i| i.dict_id == 1132));
}

#[test]
fn decals_attach_save_and_drop_with_stored_state() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    e.sign_in(&mut c);
    let g = gd();
    let (torso, decal) = e
        .store
        .update_profile(c.account, |p| {
            let torso = p.items.iter().find(|i| g.items.category(i.dict_id) == 3).map(|i| i.uid).unwrap();
            Ok::<_, ()>((torso, p.add_item(718, 718)))
        })
        .unwrap();
    c.take();
    let bits = 2.5f64.to_bits();
    c.send(Request::Inventory(Q45::RelocateUpgrade { a: torso, b: decal, c: 1, d: bits }));
    e.pump(&mut [&mut c]);
    let m = c.take();
    assert_eq!(replies(&m), vec![&Reply::Inventory(Inventory45::Camo { item: torso, id: decal, x: 1, value: 2.5 })]);
    let p = e.store.load_profile(c.account).unwrap();
    assert_eq!(p.decals[&torso].len(), 1);
    assert_eq!((p.decals[&torso][0].dict, p.decals[&torso][0].x_bits), (718, bits));
    // the editor save changes the stored placement; no answer
    c.send(Request::Inventory(Q45::Decal { a: torso, b: decal, c: 7.0f64.to_bits(), d: 1 }));
    e.pump(&mut [&mut c]);
    assert!(replies(&c.take()).is_empty());
    let p = e.store.load_profile(c.account).unwrap();
    assert_eq!(p.decals[&torso][0].x_bits, 7.0f64.to_bits());
    // the item records carry the entry
    let row = p.items.iter().find(|i| i.uid == torso).copied().unwrap();
    let rec = lobby_server::build::record(&g, &p, &row);
    assert_eq!(rec.decals.len(), 1);
    assert_eq!((rec.decals[0].id, rec.decals[0].param, rec.decals[0].y), (decal, 718, 1));
    // removing
    c.send(Request::Inventory(Q45::DropUpgrade { a: torso, b: decal, c: 1 }));
    e.pump(&mut [&mut c]);
    assert_eq!(replies(&c.take()), vec![&Reply::Inventory(Inventory45::CamoDrop { item: torso, id: decal, slot: 1 })]);
    assert!(e.store.load_profile(c.account).unwrap().decals.is_empty());
}

#[test]
fn a_decal_is_bought_at_the_dictionary_price_and_a_worn_one_raises_the_bonus() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    e.sign_in(&mut c);
    let g = gd();
    e.store
        .update_profile(c.account, |p| {
            p.gold = 100;
            Ok::<_, ()>(())
        })
        .unwrap();
    c.take();
    c.send(buy_req(718, 1));
    e.pump(&mut [&mut c]);
    c.take();
    let p = e.store.load_profile(c.account).unwrap();
    assert_eq!(p.gold, 50, "char_decal_5_29 costs 50 gold");
    let decal = p.items.iter().find(|i| i.dict_id == 718).unwrap().uid;
    let torso = p.slots.values().copied().find(|u| g.items.category(p.item(*u).unwrap().dict_id) == 3).expect("the kit torso is worn");
    let before = lobby_server::boosters::worn_bonus(&g, &p);
    c.send(Request::Inventory(Q45::RelocateUpgrade { a: torso, b: decal, c: 0, d: 0 }));
    e.pump(&mut [&mut c]);
    c.take();
    let after = lobby_server::boosters::worn_bonus(&g, &e.store.load_profile(c.account).unwrap());
    assert!(after[0] > before[0], "{after:?} vs {before:?}");
    // selling the decal frees the piece and re-sends its record
    c.send(Request::Shop(Q46::Sell { item: decal, amount: 1 }));
    e.pump(&mut [&mut c]);
    let m = c.take();
    let recs = m
        .iter()
        .filter_map(|x| if let S2c::Reply(Reply::Inventory(Inventory45::Records { items, .. })) = x { Some(items) } else { None })
        .next()
        .unwrap();
    assert!(recs.iter().any(|r| r.uid == torso && r.decals.is_empty()));
    assert!(e.store.load_profile(c.account).unwrap().decals.is_empty());
}

/// A local bots match pays nothing, so it cannot drop a free treasure-hunt card either.
#[test]
fn a_local_bots_match_drops_no_hunt_card() {
    game_data::require_tables!();
    let dir = tempfile::tempdir().unwrap();
    let rec = Arc::new(Recorder::default());
    let cfg = LobbyConfig { achievement_rewards: false, daily_quests: false, treasure_hunt: true, ..LobbyConfig::default() };
    let mut e = Env::with(&dir.path().join("h.sqlite"), cfg, rec.clone());
    let mut c = e.client("test");
    e.sign_in(&mut c);
    e.handshake(&mut c);
    let cards = |e: &Env, c: &support::Client| e.store.load_profile(c.account).unwrap().items.iter().filter(|i| i.dict_id == game_data::hunt::FREE_CARD).count();
    let before = cards(&e, &c);
    for _ in 0..40 {
        c.send(Request::Play(Play5B::Play { map: 4, mode: 1, weather: 2 }));
        e.pump(&mut [&mut c]);
        c.take();
        e.wait(&mut [&mut c], 31);
        c.send(Request::Events(vec![Event53 { id: 5, x: 0, value: 0, time: 0 }]));
        e.pump(&mut [&mut c]);
        c.take();
    }
    assert_eq!(cards(&e, &c), before);
}

/// A premium-only profile that is current when the premium is over is not used for the next match.
#[test]
fn an_expired_premium_profile_is_not_used_to_start_a_match() {
    game_data::require_tables!();
    let dir = tempfile::tempdir().unwrap();
    let rec = Arc::new(Recorder::default());
    let cfg = LobbyConfig { achievement_rewards: false, daily_quests: false, treasure_hunt: false, ..LobbyConfig::default() };
    let mut e = Env::with(&dir.path().join("x.sqlite"), cfg, rec.clone());
    let mut c = e.client("test");
    e.sign_in(&mut c);
    e.handshake(&mut c);
    e.store
        .update_profile(c.account, |p| {
            p.equip.current = 1;
            Ok::<_, ()>(())
        })
        .unwrap();
    c.send(Request::Play(Play5B::Play { map: 4, mode: 1, weather: 2 }));
    e.pump(&mut [&mut c]);
    c.take();
    assert_eq!(e.store.load_profile(c.account).unwrap().equip.current, 0);
}
