//! The default store offers (CHOICE): the premium window's four premium-time offers (95 elements,
//! `46 02`), the actions-tab cards (14-day premium and the Winner's Contract, `5C 00` / `5C 01`) and the contract's `94`
//! record. Notes: docs/notes/store.md "Store offers and pictures", docs/notes/actions.md, docs/notes/requests-34-39-3a.md.
//! The contract's payout after a won online match is tested in tests/online.rs.

mod support;

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use accounts::ContractState;
use lobby_server::actions::{CONTRACT_CARD, PREMIUM_7_DAYS_CARD};
use lobby_server::{LobbyConfig, MinimalMatches};
use protocol::binconfig::{self, Value};
use protocol::c2s::{Request, Shop46 as Q46};
use protocol::s2c::hero::StoreActions;
use protocol::s2c::*;
use protocol::Decode;
use support::*;

fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64
}

fn env() -> (tempfile::TempDir, Env) {
    let dir = tempfile::tempdir().unwrap();
    let cfg = LobbyConfig { achievement_rewards: false, login_calendar: None, ..LobbyConfig::default() };
    let e = Env::with(&dir.path().join("s.sqlite"), cfg, Arc::new(MinimalMatches));
    (dir, e)
}

fn bc(msgs: &[S2c]) -> Vec<StoreActions> {
    msgs.iter()
        .filter_map(|m| match m {
            S2c::Raw { id: 0xBC, payload } => Some(StoreActions::from_bytes(payload).unwrap()),
            _ => None,
        })
        .collect()
}

fn notices(msgs: &[S2c]) -> Vec<Notices94> {
    msgs.iter().filter_map(|m| if let S2c::Notices(n) = m { Some(n.clone()) } else { None }).collect()
}

fn key<'a>(r: &'a [(String, Value)], k: &str) -> &'a Value {
    &r.iter().find(|(n, _)| n == k).unwrap_or_else(|| panic!("no key {k}")).1
}

fn table<'a>(r: &'a [(String, Value)], k: &str) -> &'a [(String, Value)] {
    let Value::Table(t) = key(r, k) else { panic!("{k} is not a named table") };
    t
}

/// The texts of the "premium bought" notices (B1 type 0x14).
fn bought(msgs: &[S2c]) -> Vec<String> {
    msgs.iter()
        .filter_map(|m| match m {
            S2c::SystemMessage(s) if s.kind == lobby_server::premium::KIND_PREMIUM_BOUGHT => Some(s.text.clone()),
            _ => None,
        })
        .collect()
}

fn premium_secs(n: &Notices94) -> u32 {
    n.notices.iter().find(|x| x.kind == 0).map_or(0, |x| x.f38)
}

/// `bundle.items` of a card as (dictId, amount); every entry carries exactly the two keys the exe reads.
fn bundle_items(card: &[(String, Value)]) -> Vec<(i32, i32)> {
    let b = table(card, "bundle");
    if b.is_empty() {
        return vec![];
    }
    let Value::List(items) = key(b, "items") else { panic!("bundle.items is an indexed table") };
    items
        .iter()
        .map(|v| {
            let Value::Table(t) = v else { panic!() };
            assert_eq!(t.len(), 2);
            let (Value::Int(d), Value::Int(n)) = (key(t, "dictId"), key(t, "amount")) else { panic!("ints") };
            (*d, *n)
        })
        .collect()
}

/// The card ids of a tab blob.
fn card_ids(blob: &[u8]) -> Vec<i32> {
    let r = binconfig::decode(blob).unwrap();
    let Value::List(items) = key(&r, "shop_items") else { panic!() };
    items.iter().map(|v| if let Value::Table(t) = v { if let Value::Int(i) = key(t, "id") { *i } else { panic!() } } else { panic!() }).collect()
}

/// The cards of the default tab this client is shown now (`5C 00`).
fn shown_cards(e: &mut Env, c: &mut Client) -> Vec<i32> {
    c.send(Request::ActionsData { a: 0, b: lobby_server::actions::DEFAULT_TAB, c: 2, d: 1 });
    e.pump(&mut [&mut *c]);
    let m = bc(&c.take());
    let [StoreActions::Tab { blob, .. }] = m.as_slice() else { panic!("one BC sub 0: {m:?}") };
    card_ids(blob)
}

fn set(e: &Env, c: &Client, f: impl FnOnce(&mut accounts::Profile)) {
    e.store.update_profile(c.account, |p| { f(p); Ok::<_, ()>(()) }).unwrap();
}

#[test]
fn the_default_tab_blob_carries_every_mandatory_key_with_its_type() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    e.sign_in(&mut c);
    // language 1 = russian (5C 00 byte 6): the card texts are the client's own, in that language
    c.send(Request::ActionsData { a: 0, b: lobby_server::actions::DEFAULT_TAB, c: 1, d: 1 });
    e.pump(&mut [&mut c]);
    let m = bc(&c.take());
    let StoreActions::Tab { blob, more, .. } = &m[0] else { panic!("sub 0 expected") };
    assert_eq!(*more, None);
    let r = binconfig::decode(blob).unwrap();
    // root: currency_type is a STRING (the name of the real-money table in every cost)
    let Value::Str(cur) = key(&r, "currency_type") else { panic!("currency_type must be a string") };
    assert!(matches!(key(&r, "currency_sign"), Value::Str(_)));
    let Value::List(items) = key(&r, "shop_items") else { panic!("shop_items must be an indexed table") };
    assert_eq!(items.len(), 2);
    let cards: Vec<&[(String, Value)]> = items.iter().map(|v| if let Value::Table(t) = v { t.as_slice() } else { panic!() }).collect();
    for t in &cards {
        for k in ["id", "type", "ttl", "ttl_discount"] {
            assert!(matches!(key(t, k), Value::Int(_)), "{k} is an int");
        }
        for k in ["label", "description", "s_img", "m_img", "l_img"] {
            assert!(matches!(key(t, k), Value::Str(_)), "{k} is a string");
        }
        assert_eq!(key(t, "type"), &Value::Int(0));
        assert_eq!(key(t, "ttl"), &Value::Int(0), "no timer");
        let cost = table(t, "cost");
        for c in ["gold", cur.as_str()] {
            let p = table(cost, c);
            for k in ["price", "discount_price", "discount_value"] {
                assert!(matches!(key(p, k), Value::Int(_)), "cost.{c}.{k} is an int");
            }
            assert_eq!(key(p, "discount_value"), &Value::Int(0));
        }
        assert_eq!(key(table(cost, cur), "price"), &Value::Int(0), "no real-money price: the button stays hidden");
    }
    // the full view's "Contains" block lists bundle.items: dict 0 = premium hours (the client's Reward.PREMIUM entry,
    // shown as "7 days"), dict 214 = silver (the contract's daily reward)
    assert_eq!(bundle_items(cards[0]), vec![(0, 168)]);
    assert_eq!(bundle_items(cards[1]), vec![(214, 20_000)]);
    // item 0 (big slot): 7 days of premium with the crown; item 1 (small slot): the contract with image_10
    assert_eq!(key(cards[0], "id"), &Value::Int(PREMIUM_7_DAYS_CARD as i32));
    assert_eq!(key(cards[0], "label"), &Value::Str("Премиум аккаунт на 7 дней".into()));
    assert!(matches!(key(cards[0], "description"), Value::Str(s) if s.starts_with("Премиум аккаунт увеличивает")));
    assert_eq!(key(table(table(cards[0], "cost"), "gold"), "price"), &Value::Int(300));
    assert_eq!(key(cards[0], "l_img"), &Value::Str("673_l_1563455165.png".into()));
    assert_eq!(key(cards[0], "s_img"), &Value::Str("673_s_1563455165.png".into()));
    assert_eq!(key(cards[0], "m_img"), &Value::Str("673_m_1563455165.png".into()));
    assert_eq!(key(cards[1], "id"), &Value::Int(CONTRACT_CARD as i32));
    assert_eq!(key(cards[1], "label"), &Value::Str("Контракт с Барыгой".into()));
    assert_eq!(key(cards[1], "description"), &Value::Str("Выиграть 1 бой".into()));
    assert_eq!(key(table(table(cards[1], "cost"), "gold"), "price"), &Value::Int(100));
    for k in ["s_img", "m_img", "l_img"] {
        assert_eq!(key(cards[1], k), &Value::Str("image_10.png".into()));
    }
}

#[test]
fn the_premium_window_lists_bundles_1_to_4_with_prices_and_one_sub_record_each() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    let msgs = e.sign_in(&mut c);
    let shop = msgs.iter().find_map(|m| if let S2c::Shop(s) = m { Some(s.clone()) } else { None }).unwrap();
    let want = [(1, 1, 150, 72), (2, 2, 300, 168), (3, 3, 900, 720), (4, 4, 2500, 2160)];
    assert_eq!(shop.elems.len(), 4);
    for (el, (id, bundle, gold, hours)) in shop.elems.iter().zip(want) {
        assert_eq!((el.f0, el.dict, el.f9, el.fc, el.f488), (id, bundle, 0, gold, 1));
        assert_eq!(el.subs.len(), 1);
        let s = el.subs[0];
        assert_eq!((s.vals[0], s.vals[1], s.vals[5], s.vals[7], s.b, s.c), (0, 50, hours, 24, 1, 1));
        // the client's "saved" percentage: full = trunc(s5 / s7 * s1), trunc((full - price) / full * 100)
        let full = (s.vals[5] as f32 / s.vals[7] as f32 * s.vals[1] as f32) as u32;
        let pct = ((full - gold) as f32 / full as f32 * 100.0) as u32;
        assert_eq!(pct, [0, 14, 40, 44][id as usize - 1]);
    }
    // a premium voucher item id is never a +4 (the client asserts on an unknown bundle id)
    assert!(shop.elems.iter().all(|el| ![213, 980, 975, 1021].contains(&el.dict)));
}

#[test]
fn buying_a_premium_window_offer_takes_gold_and_extends_premium() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    e.sign_in(&mut c);
    let before = e.store.load_profile(c.account).unwrap();
    assert_eq!(before.premium_left(now()), 0);
    // offer 2 = a week, 300 gold (lobby: match id 0, f32 0.0)
    c.send(Request::Shop(Q46::BuyOffer { offer: 2, a: 0, b: 0 }));
    e.pump(&mut [&mut c]);
    let m = c.take();
    let after = e.store.load_profile(c.account).unwrap();
    assert_eq!(after.gold, before.gold - 300);
    let left = after.premium_left(now());
    assert!((168 * 3600 - 5..=168 * 3600).contains(&left), "{left}");
    assert!(m.iter().any(|x| matches!(x, S2c::Money(_))), "8B");
    let n = notices(&m);
    assert_eq!(n.len(), 1, "one 94");
    assert!(premium_secs(&n[0]) >= 168 * 3600 - 5);
    // the client's own "Days of Premium: %d!" notice (B1 0x14, sub 1, n = days), after 8B / 94
    assert_eq!(bought(&m), vec!["[1][7]".to_string()]);
    let at = |f: &dyn Fn(&S2c) -> bool| m.iter().position(f).unwrap();
    assert!(at(&|x| matches!(x, S2c::SystemMessage(_))) > at(&|x| matches!(x, S2c::Notices(_))));
    // the victory screen's variant (with a match id) adds to the running time
    c.send(Request::Shop(Q46::BuyOffer { offer: 1, a: 0x1234, b: 0 }));
    e.pump(&mut [&mut c]);
    c.take();
    let p = e.store.load_profile(c.account).unwrap();
    assert_eq!(p.premium_until, after.premium_until + 72 * 3600);
    assert_eq!(p.gold, after.gold - 150);
    assert_eq!(lobby_server::premium::bought_text(720), "[1][30]");
    assert_eq!(lobby_server::premium::bought_text(2160), "[1][90]");
    // an unknown offer (a voucher item id) and too little gold change nothing and send nothing
    c.send(Request::Shop(Q46::BuyOffer { offer: 213, a: 0, b: 0 }));
    e.pump(&mut [&mut c]);
    assert!(c.take().is_empty());
    set(&e, &c, |p| p.gold = 2499);
    let p = e.store.load_profile(c.account).unwrap();
    c.send(Request::Shop(Q46::BuyOffer { offer: 4, a: 0, b: 0 }));
    e.pump(&mut [&mut c]);
    assert!(c.take().is_empty());
    assert_eq!(e.store.load_profile(c.account).unwrap(), p);
}

#[test]
fn the_7_day_card_adds_168_hours_of_premium() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    e.sign_in(&mut c);
    let before = e.store.load_profile(c.account).unwrap();
    c.send(Request::ActionBuy { id: PREMIUM_7_DAYS_CARD, mode: 1 });
    e.pump(&mut [&mut c]);
    let m = c.take();
    assert_eq!(bc(&m), vec![StoreActions::Bought { data_id: PREMIUM_7_DAYS_CARD, chest_item: u32::MAX, ok: true }]);
    let after = e.store.load_profile(c.account).unwrap();
    assert_eq!(after.gold, before.gold - 300);
    assert!((168 * 3600 - 5..=168 * 3600).contains(&after.premium_left(now())));
    assert!(m.iter().any(|x| matches!(x, S2c::Money(_))));
    assert!(premium_secs(&notices(&m)[0]) >= 168 * 3600 - 5);
    assert_eq!(bought(&m), vec!["[1][7]".to_string()], "the 7-day card: 7 days of premium");
    // too little gold: result 0, nothing changes
    set(&e, &c, |p| p.gold = 299);
    let p = e.store.load_profile(c.account).unwrap();
    c.send(Request::ActionBuy { id: PREMIUM_7_DAYS_CARD, mode: 1 });
    e.pump(&mut [&mut c]);
    assert_eq!(bc(&c.take()), vec![StoreActions::Bought { data_id: PREMIUM_7_DAYS_CARD, chest_item: u32::MAX, ok: false }]);
    assert_eq!(e.store.load_profile(c.account).unwrap(), p);
}

/// The contract record and tail of a 94.
fn contract_of(n: &Notices94) -> Option<Tail94> {
    let has = n.notices.iter().any(|x| x.kind == 0x16);
    assert_eq!(has, n.tail.is_some(), "the tail goes with the 0x16 record");
    // the client skips a default-case record with both buffers empty before it reads the type (0x2001C5A5A): the
    // contract would never show
    assert!(n.notices.iter().filter(|x| x.kind == 0x16).all(|x| !x.data1.is_empty() || !x.data2.is_empty()), "an empty 0x16 record is skipped");
    // ... and the tail blob must be a raw LZMA2 stream: a plain config crashes the client at sign-in
    if let Some(t) = &n.tail {
        let body = protocol::lzma2::decompress(&t.text).expect("the contract blob is LZMA2");
        assert!(body.len() <= 0x800 && binconfig::decode(&body).is_ok());
    }
    n.tail.clone()
}

#[test]
fn the_contract_card_starts_one_contract_and_the_94_carries_it_until_the_third_day_ends() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    let login = e.sign_in(&mut c);
    assert!(contract_of(&notices(&login)[0]).is_none(), "no contract before the purchase");
    assert_eq!(shown_cards(&mut e, &mut c), vec![PREMIUM_7_DAYS_CARD as i32, CONTRACT_CARD as i32]);
    let before = e.store.load_profile(c.account).unwrap();

    c.send(Request::ActionBuy { id: CONTRACT_CARD, mode: 1 });
    e.pump(&mut [&mut c]);
    let m = c.take();
    // BC sub 1 result 1, then the tab again without the contract card (the client cannot show a card as bought: it goes)
    let m_bc = bc(&m);
    assert_eq!(m_bc[0], StoreActions::Bought { data_id: CONTRACT_CARD, chest_item: u32::MAX, ok: true });
    let [_, StoreActions::Tab { tab, blob, more: None }] = m_bc.as_slice() else { panic!("the tab follows: {m_bc:?}") };
    assert_eq!((*tab, card_ids(blob)), (lobby_server::actions::DEFAULT_TAB, vec![PREMIUM_7_DAYS_CARD as i32]));
    assert_eq!(shown_cards(&mut e, &mut c), vec![PREMIUM_7_DAYS_CARD as i32], "hidden while the contract runs");
    let after = e.store.load_profile(c.account).unwrap();
    assert_eq!(after.gold, before.gold - 100);
    assert!((now() - after.contract.started).abs() <= 5);
    assert_eq!(after.contract.won_mask, 0);
    assert!(m.iter().any(|x| matches!(x, S2c::Money(_))));
    let t = contract_of(&notices(&m)[0]).expect("the 94 carries the contract");
    assert_eq!(t.bytes, vec![0, 0, 0], "C = 3 days, none won");
    assert!((3 * 24 * 60 - 1..=3 * 24 * 60).contains(&t.a), "minutes left: {}", t.a);
    assert_eq!(t.b, u32::MAX, "challenge available");
    // the tail blob is a raw LZMA2 stream of the config (the client decodes it with its 8A decoder, 0x2001C5F25)
    let mut blob = binconfig::decode(&protocol::lzma2::decompress(&t.text).unwrap()).unwrap();
    blob.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(blob, vec![("actionType".into(), Value::Int(1)), ("amount".into(), Value::Int(20_000)), ("dictId".into(), Value::Int(214))]);
    // the encoded 94 round-trips (the tail layout the client reads)
    let n = notices(&m)[0].clone();
    assert_eq!(Notices94::from_bytes(&protocol::Encode::to_bytes(&n).unwrap()).unwrap(), n);

    // a second purchase while it runs (a store view drawn before the purchase): result 0, no gold taken, no tab
    c.send(Request::ActionBuy { id: CONTRACT_CARD, mode: 1 });
    e.pump(&mut [&mut c]);
    assert_eq!(bc(&c.take()), vec![StoreActions::Bought { data_id: CONTRACT_CARD, chest_item: u32::MAX, ok: false }]);
    assert_eq!(e.store.load_profile(c.account).unwrap().gold, after.gold);

    // day 2 after a won day 1: the statuses and the minutes until day 3 (39 00 asks for the current 94)
    set(&e, &c, |p| p.contract = ContractState { started: now() - 86_400 - 3600, won_mask: 0b011 });
    c.send(Request::Generic { id: 0x39, a: 0 });
    e.pump(&mut [&mut c]);
    let t = contract_of(&notices(&c.take())[0]).unwrap();
    assert_eq!(t.bytes, vec![1, 1, 0]);
    assert!((23 * 60 - 1..=23 * 60).contains(&t.b), "minutes to the next day: {}", t.b);
    assert!((47 * 60 - 1..=47 * 60).contains(&t.a));

    // after the 3 days the record is no longer sent, and a new contract may be bought
    set(&e, &c, |p| p.contract = ContractState { started: now() - 3 * 86_400 - 1, won_mask: 0b001 });
    c.send(Request::Generic { id: 0x39, a: 0 });
    e.pump(&mut [&mut c]);
    assert!(contract_of(&notices(&c.take())[0]).is_none());
    assert_eq!(shown_cards(&mut e, &mut c), vec![PREMIUM_7_DAYS_CARD as i32, CONTRACT_CARD as i32], "back after the end");
    set(&e, &c, |p| p.gold = 99);
    c.send(Request::ActionBuy { id: CONTRACT_CARD, mode: 1 });
    e.pump(&mut [&mut c]);
    assert_eq!(bc(&c.take()), vec![StoreActions::Bought { data_id: CONTRACT_CARD, chest_item: u32::MAX, ok: false }], "gold short");
    set(&e, &c, |p| p.gold = 100);
    c.send(Request::ActionBuy { id: CONTRACT_CARD, mode: 1 });
    e.pump(&mut [&mut c]);
    assert_eq!(bc(&c.take())[0], StoreActions::Bought { data_id: CONTRACT_CARD, chest_item: u32::MAX, ok: true });
    let p = e.store.load_profile(c.account).unwrap();
    assert_eq!((p.gold, p.contract.won_mask), (0, 0));
    // the stored contract survives a new sign-in
    let mut c2 = e.client_of("test");
    let t = contract_of(&notices(&e.sign_in(&mut c2))[0]).expect("94 at login");
    assert_eq!(t.bytes, vec![0, 0, 0]);
}
