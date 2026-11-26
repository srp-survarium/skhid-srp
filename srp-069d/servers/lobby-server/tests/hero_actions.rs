//! Hero of the day (A6 / 48 2B -> A7 / 48 2C -> A9 / the 24 h status), the store action tabs (5C -> BC), the event
//! exchange (48 28), the favourite-weapon click (46 08) and the specifics (46 09 -> 91). Notes:
//! docs/notes/hero-of-the-day.md, docs/notes/actions.md.

mod support;

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use accounts::{AccountId, HeroResult};
use game_data::{GameData, MatchFacts};
use lobby_server::actions::{OfferGrant, StoreOffer, StoreTab};
use lobby_server::dispatch::Ctx;
use lobby_server::{stats, LobbyConfig, MinimalMatches};
use protocol::binconfig::{self, Value};
use protocol::c2s::{Account48, Request, Shop46 as Q46};
use protocol::s2c::hero::*;
use protocol::s2c::*;
use protocol::Decode;
use support::*;

fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64
}

fn env_with(cfg: LobbyConfig) -> (tempfile::TempDir, Env) {
    let dir = tempfile::tempdir().unwrap();
    let cfg = LobbyConfig { achievement_rewards: false, login_calendar: None, ..cfg };
    let e = Env::with(&dir.path().join("h.sqlite"), cfg, Arc::new(MinimalMatches));
    (dir, e)
}

fn raw(msgs: &[S2c], id: u8) -> Vec<Vec<u8>> {
    msgs.iter()
        .filter_map(|m| match m {
            S2c::Raw { id: i, payload } if *i == id => Some(payload.clone()),
            _ => None,
        })
        .collect()
}

fn a7(msgs: &[S2c]) -> Vec<HeroData7> {
    raw(msgs, 0xA7).iter().map(|p| HeroData7::from_bytes(p).unwrap()).collect()
}

fn a9(msgs: &[S2c]) -> Vec<HeroFriends> {
    raw(msgs, 0xA9).iter().map(|p| HeroFriends::from_bytes(p).unwrap()).collect()
}

fn bc(msgs: &[S2c]) -> Vec<StoreActions> {
    raw(msgs, 0xBC).iter().map(|p| StoreActions::from_bytes(p).unwrap()).collect()
}

fn notices(msgs: &[S2c]) -> Notices94 {
    msgs.iter().find_map(|m| if let S2c::Notices(n) = m { Some(n.clone()) } else { None }).expect("a 94")
}

fn data() -> GameData {
    game_data::testing::tables()
}

/// A finished ranked match of `account` in the hero results, `age` seconds ago.
fn played(e: &Env, account: AccountId, age: i64, score: u32, killstreak: u32, won: bool) {
    e.store.hero_add_result(&HeroResult { account, at: now() - age, group: 0, score, killstreak, won }).unwrap();
}

fn facts(match_type: Option<u8>, score: Option<u32>) -> MatchFacts {
    let mut f = MatchFacts::local(4, 1, true, true, 1);
    f.match_type = match_type;
    f.score = score;
    f.max_killstreak = Some(4);
    f
}

fn record(e: &mut Env, account: AccountId, f: &MatchFacts) {
    let lobby_server::Server { lobby, registry, .. } = &mut e.server;
    let mut cx = Ctx { lobby, registry, now: std::time::Instant::now(), fx: vec![], kick: vec![] };
    stats::record_match(&mut cx, account, f);
}

/// Wear a non-stack item of item level `level` in slot 0 (the equipment level is the highest worn item level).
fn wear_level(e: &Env, account: AccountId, level: u32) {
    let gd = game_data::testing::tables();
    let dict = gd.items.iter().filter(|i| i.item_level == level && !i.is_stackable).map(|i| i.dict_id).next().expect("an item of that level");
    e.store
        .update_profile(account, |p| {
            let uid = p.add_item(dict, 1);
            p.slots.insert(0, uid);
            Ok::<_, ()>(())
        })
        .unwrap();
    assert_eq!(lobby_server::items::equipment_level(&gd, &e.store.load_profile(account).unwrap(), now()), level as u8);
}

fn stored(e: &Env) -> Vec<HeroResult> {
    e.store.hero_results_between(0, now() + 10).unwrap()
}

// ---- hero of the day ----

#[test]
fn the_hero_entry_is_announced_at_login() {
    game_data::require_tables!();
    let (_d, mut e) = env_with(LobbyConfig::default());
    let mut c = e.client("test");
    let a6 = raw(&e.sign_in(&mut c), 0xA6);
    assert_eq!(a6.len(), 1);
    // show, 1 group: levels 3..5, characters up to level 100, +25 / +20 / +15 %
    assert_eq!(a6[0], [1, 1, 3, 5, 100, 3, 25, 20, 15]);
    // switched off: no entry
    let (_d2, mut e2) = env_with(LobbyConfig { hero_of_the_day: false, ..LobbyConfig::default() });
    let mut c2 = e2.client("test");
    assert!(raw(&e2.sign_in(&mut c2), 0xA6).is_empty());
}

#[test]
fn only_finished_pvp_queue_matches_with_a_score_are_recorded() {
    game_data::require_tables!();
    let (_d, mut e) = env_with(LobbyConfig::default());
    let c = e.client("test");
    wear_level(&e, c.account, 4);
    record(&mut e, c.account, &facts(None, Some(900))); // local training
    let mut bots = facts(Some(1), Some(900));
    bots.local_bots = true;
    record(&mut e, c.account, &bots); // a local bots match counts for nothing
    record(&mut e, c.account, &facts(Some(2), None)); // ranked, no score observed
    let mut unfinished = facts(Some(2), Some(900));
    unfinished.finished = false;
    record(&mut e, c.account, &unfinished);
    let mut clan = facts(Some(2), Some(900));
    clan.clan_battle = true;
    record(&mut e, c.account, &clan);
    assert!(stored(&e).is_empty());
    let ok = facts(Some(2), Some(900));
    record(&mut e, c.account, &ok);
    let rows = stored(&e);
    assert_eq!(rows.len(), 1);
    assert_eq!((rows[0].account, rows[0].group, rows[0].score, rows[0].killstreak, rows[0].won), (c.account, 0, 900, 4, true));
    // a random queue match counts too (CHOICE: all PvP, not ranked only)
    let random = facts(Some(1), Some(700));
    record(&mut e, c.account, &random);
    assert_eq!(stored(&e).len(), 2);
    // the group is the player's OWN equipment level, not the match's highest: in the same match (match level 4) a
    // player wearing level 1 is in no competitive group (1 < 3) and stores nothing, the level-4 player stores his row
    let low_player = e.client("low");
    let same = facts(Some(1), Some(800));
    record(&mut e, low_player.account, &same);
    assert!(stored(&e).iter().all(|r| r.account != low_player.account), "own level 1: not counted");
    record(&mut e, c.account, &same);
    assert_eq!(stored(&e).len(), 3, "own level 4: counted");
    wear_level(&e, low_player.account, 3);
    record(&mut e, low_player.account, &same);
    assert_eq!(stored(&e).iter().filter(|r| r.account == low_player.account).count(), 1, "own level 3: counted");
    // an inactive player (the world's flag) stores no result (inactive = no reward, no fine)
    let mut idle = facts(Some(1), Some(900));
    idle.inactive = Some(true);
    record(&mut e, c.account, &idle);
    assert_eq!(stored(&e).len(), 4);
    // the same match with the player active is stored
    idle.inactive = Some(false);
    record(&mut e, c.account, &idle);
    assert_eq!(stored(&e).len(), 5);
}

#[test]
fn the_window_ranks_the_nominations_and_the_status_pays() {
    game_data::require_tables!();
    let (_d, mut e) = env_with(LobbyConfig::default());
    let mut me = e.client("me");
    let mut a = e.client("alpha");
    let b = e.client("beta");
    let g = e.client("gamma");
    // alpha: best score and two wins in a row; beta: longest kill streak; gamma: one win; me: nothing
    played(&e, a.account, 7200, 900, 2, true);
    played(&e, b.account, 7100, 300, 9, false);
    played(&e, g.account, 7000, 100, 0, true);
    played(&e, a.account, 6700, 500, 1, true);
    // alpha is online when the ranking is calculated: he gets his status pushed (a 94 with the hero record)
    e.sign_in(&mut a);
    let msgs = e.sign_in(&mut me);
    assert_eq!(raw(&msgs, 0xA6).len(), 1);
    assert!(notices(&msgs).notices.iter().all(|n| n.kind != 0x19), "no title, no hero record");

    // 48 2B: nothing cached, own values wanted
    me.send(Request::Account(Account48::HeroOfTheDay { a: 0, b: 0xFF }));
    e.pump(&mut [&mut me, &mut a]);
    let w = a7(&me.take());
    assert_eq!(w.len(), 1);
    assert_eq!(w[0].groups, 1);
    let rk = w[0].ranking.clone().expect("the ranking");
    assert_ne!(rk.id, 0);
    let who = |n: usize, p: usize| (rk.places[0][n][p].score, rk.places[0][n][p].name.clone());
    // [group][nomination][place]: nomination 0 = best player (score), 1 = killstreak, 2 = winstreak
    assert_eq!([who(0, 0), who(0, 1), who(0, 2)], [(900, "alpha".into()), (300, "beta".into()), (100, "gamma".into())]);
    assert_eq!([who(1, 0), who(1, 1), who(1, 2)], [(9, "beta".into()), (2, "alpha".into()), (0, String::new())]);
    assert_eq!([who(2, 0), who(2, 1), who(2, 2)], [(2, "alpha".into()), (1, "gamma".into()), (0, String::new())]);
    // the second group does not exist: empty
    assert_eq!(rk.places[1], <[[HeroPlace; 3]; 3]>::default());
    assert_eq!(w[0].own.clone().unwrap().values, [[0; 3]; 2], "the player has no result of his own");
    let own = w[0].own.clone().unwrap();
    assert_eq!((own.a, own.b), (0xFF, 0), "no title: the client's own 'none' word 0x00FF");

    // the cached id is current: no ranking again; without the own flag nothing comes back
    me.send(Request::Account(Account48::HeroOfTheDay { a: rk.id, b: 0 }));
    e.pump(&mut [&mut me, &mut a]);
    assert!(a7(&me.take()).is_empty());
    me.send(Request::Account(Account48::HeroOfTheDay { a: rk.id, b: 0xFF }));
    e.pump(&mut [&mut me, &mut a]);
    let w = a7(&me.take());
    assert_eq!(w.len(), 1);
    assert!(w[0].ranking.is_none() && w[0].own.is_some());

    let a_pushed = a.take();
    // alpha asks for his own values: best score 900, kill streak 2, 2 wins in a row; chat bytes = nomination index 0
    // (best player), place 1
    a.send(Request::Account(Account48::HeroOfTheDay { a: rk.id, b: 0xFF }));
    e.pump(&mut [&mut me, &mut a]);
    let own = a7(&a.take()).remove(0).own.unwrap();
    assert_eq!(own.values[0], [900, 2, 2]);
    assert_eq!((own.a, own.b), (0, 1));
    // alpha holds place 1 (score), 2 (killstreak) and 1 (winstreak): his status is the best, +25 %
    // (the hero row of the bonus; the worn faction kit adds its own reputation row)
    let gd = data();
    let alpha = e.store.load_profile(a.account).unwrap();
    assert_eq!(alpha.hero.len(), 3);
    assert_eq!(lobby_server::boosters::reward_percent_parts(&gd, &alpha, now(), &[])[lobby_server::boosters::BONUS_HERO], [25, 25, 25]);
    let beta = e.store.load_profile(b.account).unwrap();
    assert_eq!(lobby_server::boosters::reward_percent_parts(&gd, &beta, now(), &[])[lobby_server::boosters::BONUS_HERO], [25, 25, 25], "beta is first in the kill streak");
    let gamma = e.store.load_profile(g.account).unwrap();
    assert_eq!(lobby_server::boosters::reward_percent_parts(&gd, &gamma, now(), &[])[lobby_server::boosters::BONUS_HERO], [20, 20, 20], "gamma is third in the best-player ranking, second in winstreak: place 2");
    // the status ends after the operating period (24 h)
    assert_eq!(lobby_server::boosters::reward_percent_parts(&gd, &alpha, now() + 86_400 + 1_800, &[])[lobby_server::boosters::BONUS_HERO], [0, 0, 0]);
    // the online holder got his 94 with the record: kind 0x19, f3c = nomination index 0 (best player), f40 = 141 << 16 | grade 4
    let pushed: Vec<_> = a_pushed.into_iter().filter_map(|m| if let S2c::Notices(n) = m { Some(n) } else { None }).collect();
    let rec = pushed.iter().flat_map(|n| n.notices.iter()).find(|n| n.kind == 0x19).expect("the hero record");
    assert_eq!((rec.f3c, rec.f40), (0, (141 << 16) | 4));
    // and a later sign-in carries it too
    let mut a2 = e.client_of("alpha");
    let msgs = e.sign_in(&mut a2);
    assert!(notices(&msgs).notices.iter().any(|n| n.kind == 0x19 && n.f40 == (141 << 16) | 4));
    let mut g2 = e.client_of("gamma");
    let rec = notices(&e.sign_in(&mut g2)).notices.into_iter().find(|n| n.kind == 0x19).unwrap();
    assert_eq!(rec.f40, (141 << 16) | 3, "place 2 = grade 3 = +20 %");
}

#[test]
fn the_ranking_is_calculated_once_per_slot_and_titles_end() {
    game_data::require_tables!();
    let (_d, mut e) = env_with(LobbyConfig::default());
    let c = e.client("test");
    let gd = data();
    let t = now();
    played(&e, c.account, 600, 400, 3, true);
    // the slot after the match: the match is in the window
    let later = t + 1801;
    assert_eq!(lobby_server::hero::recalc(&e.store, &gd, later), vec![c.account, c.account, c.account]);
    assert!(lobby_server::hero::recalc(&e.store, &gd, later).is_empty(), "the slot is done");
    let titles = e.store.hero_titles_of(c.account, later).unwrap();
    assert_eq!(titles.len(), 3);
    assert!(titles.iter().all(|x| x.place == 1 && x.until == x.since + 86_400));
    // the next slot a day later: the result left the window, the old titles ended, nothing new
    let next_day = later + 86_400 + 1_800;
    assert!(lobby_server::hero::recalc(&e.store, &gd, next_day).is_empty());
    assert!(e.store.hero_titles_of(c.account, next_day).unwrap().is_empty());
    assert!(stored(&e).is_empty(), "results older than the window are deleted");
}

#[test]
fn friends_and_clan_rankings_are_a9() {
    game_data::require_tables!();
    let (_d, mut e) = env_with(LobbyConfig::default());
    let mut me = e.client("me");
    let a = e.client("alpha");
    let b = e.client("beta");
    e.store.friend_request(me.account, a.account).unwrap();
    e.store.friend_accept(a.account, me.account).unwrap();
    played(&e, a.account, 3000, 900, 2, true);
    played(&e, b.account, 3000, 999, 5, true); // not a friend
    played(&e, me.account, 2900, 200, 1, false);
    e.sign_in(&mut me);
    me.send(Request::Account(Account48::DayHeroData { a: 1, b: 0 }));
    e.pump(&mut [&mut me]);
    let m = a9(&me.take());
    assert_eq!(m.len(), 1);
    assert_eq!(m[0].kind, 1);
    let (rows, id) = m[0].rows.clone().expect("the list");
    assert_ne!(id, 0);
    let pid = |acc: AccountId| f64::from_bits(accounts::public_id(acc));
    // best player: alpha 900, me 200 (beta is not a friend); killstreak: alpha 2, me 1; winstreak: alpha 1, me 0 (a loss)
    assert_eq!(rows[0], [(pid(a.account), 900), (pid(me.account), 200), (0.0, 0)]);
    assert_eq!(rows[1], [(pid(a.account), 2), (pid(me.account), 1), (0.0, 0)]);
    assert_eq!(rows[2], [(pid(a.account), 1), (0.0, 0), (0.0, 0)]);
    // the cached id is current: flag 0
    me.send(Request::Account(Account48::DayHeroData { a: 1, b: id }));
    e.pump(&mut [&mut me]);
    assert_eq!(a9(&me.take()), vec![HeroFriends { kind: 1, rows: None }]);
    // clan (none): only me
    me.send(Request::Account(Account48::DayHeroData { a: 2, b: 0 }));
    e.pump(&mut [&mut me]);
    let m = a9(&me.take());
    assert_eq!(m[0].kind, 2);
    assert_eq!(m[0].rows.clone().unwrap().0[0][0], (pid(me.account), 200));
    // another kind is ignored
    me.send(Request::Account(Account48::DayHeroData { a: 3, b: 0 }));
    e.pump(&mut [&mut me]);
    assert!(a9(&me.take()).is_empty());
}

// ---- store action tabs ----

fn offer(id: u32, tab: u32, gold: u32, items: Vec<(u16, u32)>) -> StoreOffer {
    StoreOffer {
        id,
        tab,
        label: format!("offer {id}"),
        description: "d".into(),
        gold,
        items,
        ttl_secs: Some(3600),
        images: ["s.png".into(), "m.png".into(), "l.png".into()],
        grant: OfferGrant::Items,
    }
}

fn root(blob: &[u8]) -> Vec<(String, Value)> {
    binconfig::decode(blob).unwrap()
}

fn key<'a>(r: &'a [(String, Value)], k: &str) -> &'a Value {
    &r.iter().find(|(n, _)| n == k).unwrap_or_else(|| panic!("no key {k}")).1
}

#[test]
fn without_tabs_95_is_the_smallest_and_a_tab_request_gets_an_empty_config() {
    game_data::require_tables!();
    let (_d, mut e) = env_with(LobbyConfig { store_tabs: vec![], store_offers: vec![], premium_offers: vec![], ..LobbyConfig::default() });
    let mut c = e.client("test");
    let msgs = e.sign_in(&mut c);
    let shop = msgs.iter().find_map(|m| if let S2c::Shop(s) = m { Some(s.clone()) } else { None }).unwrap();
    assert_eq!(shop, Shop95::default());
    c.send(Request::ActionsData { a: 0, b: 5, c: 0, d: 1 });
    e.pump(&mut [&mut c]);
    let m = bc(&c.take());
    assert_eq!(m.len(), 1);
    let StoreActions::Tab { tab, blob, more } = &m[0] else { panic!("sub 0 expected") };
    assert_eq!((*tab, *more), (5, None));
    let r = root(blob);
    assert_eq!(key(&r, "shop_items"), &Value::List(vec![]));
    assert_eq!(key(&r, "currency_type"), &Value::Str("usd".into()));
    // a buy of an offer that does not exist is answered with result 0
    c.send(Request::ActionBuy { id: 9, mode: 1 });
    e.pump(&mut [&mut c]);
    assert_eq!(bc(&c.take()), vec![StoreActions::Bought { data_id: 9, chest_item: u32::MAX, ok: false }]);
}

#[test]
fn tabs_and_offers_are_a_server_setting_and_a_buy_pays_gold_and_grants_the_bundle() {
    game_data::require_tables!();
    let cfg = LobbyConfig {
        store_tabs: vec![StoreTab { id: 3, name: "tab_bundles".into(), default_gold: true }],
        store_offers: vec![offer(11, 3, 300, vec![(214, 500), (5, 2)]), offer(12, 4, 100, vec![(214, 1)]), offer(13, 3, 99_999_999, vec![(214, 1)])],
        ..LobbyConfig::default()
    };
    let (_d, mut e) = env_with(cfg);
    let mut c = e.client("test");
    let msgs = e.sign_in(&mut c);
    let shop = msgs.iter().find_map(|m| if let S2c::Shop(s) = m { Some(s.clone()) } else { None }).unwrap();
    assert_eq!(shop.tabs, vec![ShopTab { id: 3, name: "tab_bundles".into(), flag: 1 }]);

    // the tab carries its own offers only
    c.send(Request::ActionsData { a: 0, b: 3, c: 0, d: 1 });
    e.pump(&mut [&mut c]);
    let m = bc(&c.take());
    let StoreActions::Tab { blob, .. } = &m[0] else { panic!() };
    let r = root(blob);
    let Value::List(items) = key(&r, "shop_items") else { panic!() };
    assert_eq!(items.len(), 2);
    let Value::Table(first) = &items[0] else { panic!() };
    assert_eq!(key(first, "id"), &Value::Int(11));
    assert_eq!(key(first, "label"), &Value::Str("offer 11".into()));
    let Value::Table(cost) = key(first, "cost") else { panic!() };
    let Value::Table(gold) = key(cost, "gold") else { panic!() };
    assert_eq!(key(gold, "price"), &Value::Int(300));
    // the bundle items carry `dictId` (the key the client reads)
    let Value::Table(bundle) = key(first, "bundle") else { panic!() };
    let Value::List(bi) = key(bundle, "items") else { panic!() };
    let Value::Table(b0) = &bi[0] else { panic!() };
    assert_eq!((key(b0, "dictId"), key(b0, "amount")), (&Value::Int(214), &Value::Int(500)));

    // buy offer 11: 300 gold, 500 silver and 2 grenades
    let before = e.store.load_profile(c.account).unwrap();
    let grenades = before.items.iter().filter(|i| i.dict_id == 5).map(|i| i.count).sum::<u32>();
    c.send(Request::ActionBuy { id: 11, mode: 1 });
    e.pump(&mut [&mut c]);
    let m = c.take();
    assert_eq!(bc(&m), vec![StoreActions::Bought { data_id: 11, chest_item: u32::MAX, ok: true }]);
    let after = e.store.load_profile(c.account).unwrap();
    assert_eq!((after.gold, after.silver), (before.gold - 300, before.silver + 500));
    assert_eq!(after.items.iter().filter(|i| i.dict_id == 5).map(|i| i.count).sum::<u32>(), grenades + 2);
    assert!(m.iter().any(|x| matches!(x, S2c::Money(_))));

    // not enough gold: result 0 and nothing changes
    c.send(Request::ActionBuy { id: 13, mode: 1 });
    // the other payment mode is not served
    c.send(Request::ActionBuy { id: 11, mode: 2 });
    e.pump(&mut [&mut c]);
    let m = bc(&c.take());
    assert_eq!(m, vec![StoreActions::Bought { data_id: 13, chest_item: u32::MAX, ok: false }, StoreActions::Bought { data_id: 11, chest_item: u32::MAX, ok: false }]);
    assert_eq!(e.store.load_profile(c.account).unwrap().gold, after.gold);
}

// ---- 48 28, 46 08, 46 09 ----

#[test]
fn the_event_exchange_clears_the_lock_and_the_favourite_click_needs_no_answer() {
    game_data::require_tables!();
    let (_d, mut e) = env_with(LobbyConfig::default());
    let mut c = e.client("test");
    e.sign_in(&mut c);
    let before = e.store.load_profile(c.account).unwrap();
    // 1139 -> 2200 is a row of event_exchange_action_items.options; the account holds no event currency: refused, lock cleared
    c.send_raw(0x48, &[0x28, 0x73, 0x04, 0x98, 0x08]);
    c.send(Request::Shop(Q46::FavoriteDiscount { a: 3, b: 1 }));
    e.pump(&mut [&mut c]);
    let m = c.take();
    let subs: Vec<u8> = m.iter().filter_map(|x| if let S2c::Reply(Reply::Account(protocol::s2c::Account48::Other { sub, .. })) = x { Some(*sub) } else { None }).collect();
    assert_eq!(subs, vec![3]);
    assert_eq!(m.len(), 1, "46 08 is not answered");
    assert_eq!(e.store.load_profile(c.account).unwrap(), before);
}

fn dict_736_account(e: &mut Env, gold: u32) -> Client {
    let c = e.client("test");
    // no faction points: the faction level does not open anything
    e.store
        .update_profile(c.account, |p| {
            p.fac_points.clear();
            p.gold = gold;
            Ok::<_, ()>(())
        })
        .unwrap();
    c
}

#[test]
fn a_gated_server_sends_the_bought_specifics_and_46_09_pays_gold() {
    game_data::require_tables!();
    let (_d, mut e) = env_with(LobbyConfig::default());
    let mut c = dict_736_account(&mut e, 1000);
    let msgs = e.sign_in(&mut c);
    let mask = |m: &[S2c]| m.iter().filter_map(|x| if let S2c::SpecificsOpened(s) = x { Some(s.mask) } else { None }).collect::<Vec<_>>();
    assert_eq!(mask(&msgs), vec![0], "nothing is open yet");
    // MON-50 (dict 736, level 4): price = row 0 of buy_specific_cost at column min(4 - 2, 2) = 200 gold
    assert_eq!(lobby_server::actions::specific_price(&data(), 736), Some(200));
    c.send(Request::Shop(Q46::BuySpecific { dict: 736 }));
    e.pump(&mut [&mut c]);
    let m = c.take();
    // entry 9 of the client's table of 19
    assert_eq!(mask(&m), vec![1 << 9]);
    assert_eq!(e.store.load_profile(c.account).unwrap().gold, 800);
    // a second purchase, an item outside the table and too little gold change nothing
    c.send(Request::Shop(Q46::BuySpecific { dict: 736 }));
    c.send(Request::Shop(Q46::BuySpecific { dict: 3 }));
    e.pump(&mut [&mut c]);
    assert!(c.take().is_empty());
    e.store.update_profile(c.account, |p| { p.gold = 10; Ok::<_, ()>(()) }).unwrap();
    c.send(Request::Shop(Q46::BuySpecific { dict: 734 }));
    e.pump(&mut [&mut c]);
    assert!(c.take().is_empty());
    assert_eq!(e.store.load_profile(c.account).unwrap().gold, 10);
    // the next login carries the bought bit
    let mut c2 = e.client_of("test");
    assert_eq!(mask(&e.sign_in(&mut c2)), vec![1 << 9]);
}

/// The pocket consumables priced by our own CHOICE (docs/notes/store.md section 5) reach the client listed (8A byte e = 1:
/// the pocket popup builds a tile only for those) with their silver price, open with gold through 46 09 like the other
/// specifics, and the after-match charge takes their price per unit used. The flash grenade (2072) is not one of the 19:
/// level 1, so the client opens it by faction level; 46 09 refuses it.
#[test]
fn choice_priced_pockets_are_listed_unlock_with_gold_and_are_charged_at_their_price() {
    game_data::require_tables!();
    let (_d, mut e) = env_with(LobbyConfig::default());
    let mut c = dict_736_account(&mut e, 1000);
    let msgs = e.sign_in(&mut c);
    let S2c::Prices(pr) = msgs.iter().find(|m| m.id() == 0x8A).unwrap() else { panic!() };
    let want = [(733u16, 40u32), (1089, 50), (1090, 100), (1091, 50), (1101, 75), (1106, 100), (1133, 250), (1196, 40), (2072, 40)];
    for (d, silver) in want {
        let r = pr.records.iter().find(|r| r.dict == d).unwrap_or_else(|| panic!("no 8A record for {d}"));
        assert_eq!((r.a, r.e), (silver, 1), "8A record of {d}");
    }
    // C-4 (level 4): 200 gold, entry 17 of the client's table of 19
    assert_eq!(lobby_server::actions::specific_price(&data(), 1133), Some(200));
    let mask = |m: &[S2c]| m.iter().filter_map(|x| if let S2c::SpecificsOpened(s) = x { Some(s.mask) } else { None }).collect::<Vec<_>>();
    c.send(Request::Shop(Q46::BuySpecific { dict: 1133 }));
    e.pump(&mut [&mut c]);
    assert_eq!(mask(&c.take()), vec![1 << 17]);
    assert_eq!(e.store.load_profile(c.account).unwrap().gold, 800);
    c.send(Request::Shop(Q46::BuySpecific { dict: 2072 }));
    e.pump(&mut [&mut c]);
    assert!(c.take().is_empty(), "2072 is outside the client's table of 19");
    assert_eq!(e.store.load_profile(c.account).unwrap().gold, 800);
    // after the match: 2 x C-4 and 3 x Black Market medkit used -> "Special items" 500 + 120 silver
    let used = lobby_server::charge::MatchUse {
        profiles: 1,
        weapons: vec![],
        stacks: vec![
            lobby_server::charge::StackUse { uid: 1, dict: 1133, used: 2 },
            lobby_server::charge::StackUse { uid: 2, dict: 1196, used: 3 },
        ],
    };
    let ch = lobby_server::charge::price(&data(), 0, Some(&used));
    assert_eq!(ch.spent[SPENT_POCKETS], vec![Spent { money: 500, dict: 1133 }, Spent { money: 120, dict: 1196 }]);
}

#[test]
fn an_item_open_by_faction_level_is_not_sold_and_nothing_bought_is_masked_open() {
    game_data::require_tables!();
    let (_d, mut e) = env_with(LobbyConfig::default());
    let mut c = e.client("test");
    let msgs = e.sign_in(&mut c);
    assert!(msgs.iter().any(|x| matches!(x, S2c::SpecificsOpened(s) if s.mask == 0)));
    // the starter profile has 200000 points in every faction: dict 736 is open by level, the purchase is refused
    let gold = e.store.load_profile(c.account).unwrap().gold;
    c.send(Request::Shop(Q46::BuySpecific { dict: 736 }));
    e.pump(&mut [&mut c]);
    assert!(c.take().is_empty());
    assert_eq!(e.store.load_profile(c.account).unwrap().gold, gold);
}

fn pocket_entry(item: u32, dict: u16, from: u8, to: u8, count: u32) -> Request {
    let entries = vec![protocol::c2s::EquipEntry { item, dict, from, to, count, x: 1 }];
    Request::Inventory(protocol::c2s::Inventory45::Equip { profile: 1, entries, c1: 1, c2: 1 })
}

/// The pocket gating on the server (docs/notes/actions.md section 6), the client's rule: a pocket consumable that is
/// locked for the player (faction level below the item's level and not opened with 46 09) is never placed from the popup
/// (source 200: the client's locked tile cannot be selected or dragged), and elsewhere only with units the player owns;
/// units it would have to buy are refused. The refusal is silent (ack + 87 with the real slots, nothing charged). An
/// item opened with gold or by rank is bought as before; an item outside the client's table of 19 opens by faction
/// level only.
#[test]
fn locked_pocket_items_take_owned_units_only_and_open_ones_are_bought() {
    game_data::require_tables!();
    let (_d, mut e) = env_with(LobbyConfig::default());
    let mut c = dict_736_account(&mut e, 1000);
    // owned stacks: Fringe Settlers medkit 760 (FS level 2) x3, Snowball 1884 (RA level 2, outside the 19, unlisted) x2
    let (medkit, snow) = e
        .store
        .update_profile(c.account, |p| Ok::<_, ()>((p.add_item(760, 3), p.add_item(1884, 2))))
        .unwrap();
    e.sign_in(&mut c);
    let silver = e.store.load_profile(c.account).unwrap().silver;
    let ids = |m: &[S2c]| m.iter().map(|x| x.id()).collect::<Vec<_>>();
    // 1. popup buy of MON-50 (736, BM level 4, 0 points, not bought): refused, no stack created, nothing charged
    c.send(pocket_entry(736, 736, 200, 13, 1));
    e.pump(&mut [&mut c]);
    assert_eq!(ids(&c.take()), [0x83, 0x87], "ack + 87, no 45 0F record, no 8B");
    let p = e.store.load_profile(c.account).unwrap();
    assert!(!p.slots.contains_key(&13) && !p.items.iter().any(|i| i.dict_id == 736) && p.silver == silver);
    // 2. the owned locked stack from the inventory: its 3 units go in, nothing is bought
    c.send(pocket_entry(medkit, 760, 100, 14, 3));
    e.pump(&mut [&mut c]);
    assert_eq!(ids(&c.take()), [0x83, 0x87], "no 45 0F record (the stack did not grow), no 8B");
    let p = e.store.load_profile(c.account).unwrap();
    assert_eq!((p.slots.get(&14), p.slot_rounds.get(&14), p.item(medkit).unwrap().count), (Some(&medkit), Some(&3), 3));
    // 3. one more unit in another pocket would be a buy (4 needed, 3 owned): refused
    c.send(pocket_entry(medkit, 760, 100, 15, 1));
    e.pump(&mut [&mut c]);
    c.take();
    let p = e.store.load_profile(c.account).unwrap();
    assert!(!p.slots.contains_key(&15));
    assert_eq!((p.item(medkit).unwrap().count, p.silver), (3, silver));
    // 4. the popup (source 200) never places a locked item, owned or not
    c.send(pocket_entry(760, 760, 200, 15, 1));
    e.pump(&mut [&mut c]);
    c.take();
    let p = e.store.load_profile(c.account).unwrap();
    assert!(!p.slots.contains_key(&15));
    assert_eq!((p.item(medkit).unwrap().count, p.silver), (3, silver));
    // 5. outside the 19 (Snowball, RA level 1 -> locked, and unlisted): owned units only (3 asked -> refused; 2 -> free)
    c.send(pocket_entry(snow, 1884, 100, 15, 3));
    e.pump(&mut [&mut c]);
    c.take();
    assert!(!e.store.load_profile(c.account).unwrap().slots.contains_key(&15), "a locked item is not bought");
    c.send(pocket_entry(snow, 1884, 100, 15, 2));
    e.pump(&mut [&mut c]);
    c.take();
    let p = e.store.load_profile(c.account).unwrap();
    assert_eq!((p.slots.get(&15), p.slot_rounds.get(&15), p.item(snow).unwrap().count), (Some(&snow), Some(&2), 2));
    assert_eq!(p.silver, silver);
    // 6. opened with gold (46 09, 200 gold): the popup buy goes in at the silver price
    c.send(Request::Shop(Q46::BuySpecific { dict: 736 }));
    e.pump(&mut [&mut c]);
    c.take();
    c.send(pocket_entry(736, 736, 200, 13, 1));
    e.pump(&mut [&mut c]);
    assert_eq!(ids(&c.take()), [0x83, 0x83, 0x87, 0x8B]);
    let p = e.store.load_profile(c.account).unwrap();
    assert_eq!(p.item(p.slots[&13]).map(|i| (i.dict_id, i.count)), Some((736, 1)));
    assert_eq!((p.gold, silver - p.silver), (800, 200));
    // 7. opened by rank (Fringe Settlers level 2, 5 000 points): the popup now buys a 4th medkit into pocket 16
    e.store
        .update_profile(c.account, |p| {
            p.add_points(4, 5_000);
            Ok::<_, ()>(())
        })
        .unwrap();
    c.send(pocket_entry(760, 760, 200, 16, 1));
    e.pump(&mut [&mut c]);
    // record + ack + 87 + 8B (the rank-up notices for the points written straight into the store follow)
    assert!(ids(&c.take()).starts_with(&[0x83, 0x83, 0x87, 0x8B]));
    let medkit_price = data().shop.pocket_cost(760, 1).1 as u32;
    let p = e.store.load_profile(c.account).unwrap();
    assert_eq!((p.slots.get(&16), p.item(medkit).unwrap().count), (Some(&medkit), 4));
    assert_eq!(silver - p.silver, 200 + medkit_price);
    // 8. Flash grenade (2072, outside the 19, level 1): open at 0 points, bought at its price (pocket 17)
    c.send(pocket_entry(2072, 2072, 200, 17, 1));
    e.pump(&mut [&mut c]);
    c.take();
    let p = e.store.load_profile(c.account).unwrap();
    assert_eq!(p.item(p.slots[&17]).map(|i| i.dict_id), Some(2072));
    assert_eq!(silver - p.silver, 200 + medkit_price + data().shop.pocket_cost(2072, 1).1 as u32);
}

/// A locked item that already sits in a pocket moves to another pocket with its owned units (the client's pocket-to-
/// pocket drag, `SlotsSelector.exchangeSlotData`, has no `opened` test); a count above the owned units is refused, and
/// taking it out always works.
#[test]
fn a_locked_item_already_in_a_pocket_moves_with_its_owned_units() {
    game_data::require_tables!();
    let (_d, mut e) = env_with(LobbyConfig::default());
    let mut c = dict_736_account(&mut e, 0);
    let medkit = e
        .store
        .update_profile(c.account, |p| {
            let u = p.add_item(760, 3);
            p.slots.insert(13, u);
            p.slot_rounds.insert(13, 3);
            Ok::<_, ()>(u)
        })
        .unwrap();
    e.sign_in(&mut c);
    let silver = e.store.load_profile(c.account).unwrap().silver;
    c.send(pocket_entry(medkit, 760, 13, 14, 3));
    e.pump(&mut [&mut c]);
    c.take();
    let p = e.store.load_profile(c.account).unwrap();
    assert_eq!((p.slots.get(&13), p.slots.get(&14), p.slot_rounds.get(&14)), (None, Some(&medkit), Some(&3)));
    // 4 units in that pocket would buy one: refused, the pocket keeps its 3
    c.send(pocket_entry(medkit, 760, 14, 14, 4));
    e.pump(&mut [&mut c]);
    c.take();
    let p = e.store.load_profile(c.account).unwrap();
    assert_eq!((p.slot_rounds.get(&14), p.item(medkit).unwrap().count, p.silver), (Some(&3), 3, silver));
    // taking it out is always allowed
    c.send(pocket_entry(medkit, 760, 14, 100, 0));
    e.pump(&mut [&mut c]);
    c.take();
    assert!(!e.store.load_profile(c.account).unwrap().slots.contains_key(&14));
}
