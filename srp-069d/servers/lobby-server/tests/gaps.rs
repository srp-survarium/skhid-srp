//! Lobby gaps: the game-server list (9E / 57 / 56) and complaints / compliments (48 06). Notes: server-list.md, social.md.

mod support;

use std::sync::Arc;

use accounts::public_id;
use lobby_server::servers::GameServer;
use lobby_server::{LobbyConfig, MinimalMatches};
use protocol::c2s::{Account48, Request};
use protocol::s2c::Account48 as R48;
use protocol::s2c::*;
use support::*;

fn cfg() -> LobbyConfig {
    LobbyConfig { achievement_rewards: false, login_calendar: None, ..LobbyConfig::default() }
}

#[test]
fn the_server_list_is_sent_at_login_answered_on_57_and_ping_results_are_kept() {
    game_data::require_tables!();
    let dir = tempfile::tempdir().unwrap();
    let mut c = LobbyConfig { game_servers: GameServer::parse_list("eu.example|Europe|10|20;us.example|America|30|40").unwrap(), ..cfg() };
    c.daily_quests = false;
    let mut e = Env::with(&dir.path().join("srp.sqlite"), c, Arc::new(MinimalMatches));
    let mut a = e.client("alpha");
    let login = e.sign_in(&mut a);
    let list = login.iter().find_map(|m| match m {
        S2c::ServerInfo(i) => Some(i.clone()),
        _ => None,
    });
    let list = list.expect("9E at login");
    assert_eq!(list.servers.len(), 2);
    assert_eq!((list.servers[1].name.as_str(), list.servers[1].address.as_str(), list.servers[1].a, list.servers[1].c), ("us.example", "America", 30, 1));
    // 57: the client's choice; the list comes again
    a.send(Request::ServerList { a: 0b10, b: 0 });
    e.pump(&mut [&mut a]);
    let got = a.take();
    assert!(matches!(&got[..], [S2c::ServerInfo(i)] if *i == list), "{got:?}");
    // 56: nothing comes back
    a.send(Request::Retry56 { pings: vec![(0, 41), (1, 180)] });
    e.pump(&mut [&mut a]);
    assert!(a.take().is_empty());
}

#[test]
fn without_a_configured_server_the_list_stays_empty_and_57_gets_no_answer() {
    game_data::require_tables!();
    let dir = tempfile::tempdir().unwrap();
    let mut e = Env::new(&dir.path().join("srp.sqlite"));
    let mut a = e.client("alpha");
    let login = e.sign_in(&mut a);
    assert!(login.iter().any(|m| matches!(m, S2c::ServerInfo(i) if i.servers.is_empty())));
    a.send(Request::ServerList { a: u32::MAX, b: 0 });
    e.pump(&mut [&mut a]);
    assert!(a.take().is_empty());
}

#[test]
fn comments_are_stored_answered_and_limited() {
    game_data::require_tables!();
    let dir = tempfile::tempdir().unwrap();
    let mut e = Env::new(&dir.path().join("srp.sqlite"));
    let mut a = e.client("alpha");
    let b = e.client("bravo");
    e.sign_in(&mut a);
    let target = public_id(b.account);
    // a complaint: answered with kind 1, stored
    a.send(Request::Account(Account48::Comment { player: target, kind: 1, a: 0, b: 9 }));
    e.pump(&mut [&mut a]);
    let got = a.take();
    assert!(matches!(&got[..], [S2c::Reply(Reply::Account(R48::Comment { kind: 1 }))]), "{got:?}");
    // the same player twice a day: refused with the client's own text (84 x 48 01 <key>)
    a.send(Request::Account(Account48::Comment { player: target, kind: 1, a: 0, b: 9 }));
    e.pump(&mut [&mut a]);
    assert_eq!(a.take(), vec![refused("st_claims_count_per_user_owerflow")]);
    // a compliment is a different thing
    a.send(Request::Account(Account48::Comment { player: target, kind: 0, a: 0, b: 9 }));
    e.pump(&mut [&mut a]);
    assert!(matches!(&a.take()[..], [S2c::Reply(Reply::Account(R48::Comment { kind: 0 }))]));
    // yourself and strangers are refused
    a.send(Request::Account(Account48::Comment { player: public_id(a.account), kind: 0, a: 0, b: 0 }));
    a.send(Request::Account(Account48::Comment { player: 12345, kind: 1, a: 0, b: 0 }));
    e.pump(&mut [&mut a]);
    assert!(a.take().is_empty());
    let rows = e.store.list_comments(Some(b.account), 10).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!((rows[1].kind, rows[1].b, rows[1].from), (1, 9, a.account));
}

#[test]
fn the_daily_maximum_is_a_setting() {
    game_data::require_tables!();
    let dir = tempfile::tempdir().unwrap();
    let mut e = Env::with(&dir.path().join("srp.sqlite"), LobbyConfig { claims_per_day: Some(1), ..cfg() }, Arc::new(MinimalMatches));
    let mut a = e.client("alpha");
    let b = e.client("bravo");
    let c = e.client("charlie");
    e.sign_in(&mut a);
    a.send(Request::Account(Account48::Comment { player: public_id(b.account), kind: 1, a: 0, b: 0 }));
    a.send(Request::Account(Account48::Comment { player: public_id(c.account), kind: 1, a: 0, b: 0 }));
    e.pump(&mut [&mut a]);
    let got = a.take();
    assert_eq!(got.len(), 2);
    assert_eq!(got[1], refused("st_claims_count_per_day_owerflow"), "the second complaint of the day is over the maximum");
}

#[test]
fn spare_parts_percent_follows_the_worn_items_group_147() {
    game_data::require_tables!();
    let gd = game_data::testing::tables();
    let mut p = accounts::Profile::starter();
    assert_eq!(lobby_server::boosters::parts_percent(&gd, &p), 0);
    // the starter's scavenger kit: +2 % reputation with the active faction (125, 3), nothing else
    let kit = lobby_server::boosters::worn_bonus(&gd, &p);
    let uid = *p.slots.values().next().unwrap();
    // a workshop roll (147, 1) = +10 % spare parts, (147, 7) = +5 %
    p.mods.insert(uid, vec![(147, 1), (147, 7)]);
    assert_eq!(lobby_server::boosters::parts_percent(&gd, &p), 15);
    // it is not an experience / silver / reputation bonus
    assert_eq!(lobby_server::boosters::worn_bonus(&gd, &p), kit);
    assert!(kit[0] == 0.0 && kit[1] == 0.0 && (kit[2] - 0.02).abs() < 1e-9, "{kit:?}");
}

#[test]
fn equipment_level_is_the_highest_worn_item_level_without_pockets() {
    game_data::require_tables!();
    let gd = game_data::testing::tables();
    let mut p = accounts::Profile::starter();
    let base = lobby_server::items::equipment_level(&gd, &p, 0);
    assert!((1..=5).contains(&base), "{base}");
    let top = gd.items.iter().filter(|i| i.item_level == 5 && !i.is_stackable).map(|i| i.dict_id).next().expect("a level 5 item");
    let uid = p.add_item(top, 1);
    p.slots.insert(13, uid);
    assert_eq!(lobby_server::items::equipment_level(&gd, &p, 0), base, "a pocket slot does not count");
    p.slots.remove(&13);
    p.slots.insert(0, uid);
    assert_eq!(lobby_server::items::equipment_level(&gd, &p, 0), 5);
}

fn refused(key: &str) -> S2c {
    S2c::Denied(protocol::s2c::Denied { request: 0x48, body: protocol::s2c::DeniedBody::Text(key.into()) })
}
