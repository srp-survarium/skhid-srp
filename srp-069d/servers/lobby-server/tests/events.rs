//! The event (docs/notes/events.md): the `94` record 23, the event currencies in 8B, the `48 28` exchange, and what a finished
//! online match pays.

mod support;

use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use accounts::Currency;
use game_data::events::Schedule;
use game_data::GameData;
use lobby_server::{Ended, EndedPlayer, EndedStat, LobbyConfig, MinimalMatches, OnlineMatches, Seat, Ticket82};
use protocol::c2s::{Account48, Event53, Request};
use protocol::s2c::*;
use support::*;

fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64
}

fn running() -> Schedule {
    Schedule { start: now() - 1000, end: now() + 100_000, exchange_end: now() + 200_000, label: "Winter".into(), description: "Trade!".into() }
}

fn env_with(event: Option<Schedule>) -> (tempfile::TempDir, Env) {
    let dir = tempfile::tempdir().unwrap();
    let cfg = LobbyConfig { achievement_rewards: false, login_calendar: None, events: event.into_iter().collect(), ..LobbyConfig::default() };
    let e = Env::with(&dir.path().join("ev.sqlite"), cfg, Arc::new(MinimalMatches));
    (dir, e)
}

fn notices(msgs: &[S2c]) -> Notices94 {
    msgs.iter().find_map(|m| if let S2c::Notices(n) = m { Some(n.clone()) } else { None }).expect("a 94")
}

fn exchange(c: &mut Client, source: u16, target: u16) {
    c.send(Request::Account(Account48::ActionItemExchanged { a: source, b: target }));
}

fn acks(m: &[S2c]) -> Vec<u8> {
    m.iter().filter_map(|x| if let S2c::Reply(Reply::Account(protocol::s2c::Account48::Other { sub, .. })) = x { Some(*sub) } else { None }).collect()
}

fn money(m: &[S2c]) -> Vec<Money> {
    m.iter().filter_map(|x| if let S2c::Money(v) = x { Some(*v) } else { None }).collect()
}

#[test]
fn a_running_event_sends_record_23_and_no_event_sends_none() {
    game_data::require_tables!();
    let (_d, mut e) = env_with(Some(running()));
    let mut c = e.client("test");
    let m = e.sign_in(&mut c);
    let r: Vec<_> = notices(&m).notices.into_iter().filter(|n| n.kind == 23).collect();
    assert_eq!(r.len(), 1);
    // the countdown: the record's second u32 counts the seconds to the end
    assert!((99_000..=100_000).contains(&r[0].f38), "{}", r[0].f38);
    // the second body: LZMA2 of the exchange rows (the 0x17 reader 0x2001B8EF0 reads the decoded bytes): 3 rows, no second
    // list, only the exchange
    let body = protocol::lzma2::decompress(&r[0].data2).unwrap();
    assert_eq!((body[0], body.len(), &body[37..]), (3, 39, &[0u8, 1][..]), "{body:?}");
    assert!(!r[0].data1.is_empty() && r[0].data1.len() <= 0x800);
    assert!(String::from_utf8_lossy(&r[0].data1).contains("Winter"), "the configured label travels in the ui body");

    let (_d2, mut e2) = env_with(None);
    let mut c2 = e2.client("test");
    let m2 = e2.sign_in(&mut c2);
    assert!(notices(&m2).notices.iter().all(|n| n.kind != 23));

    // the window outlives the event, the drops do not
    let after = Schedule { start: now() - 2000, end: now() - 1000, exchange_end: now() + 1000, ..running() };
    let (_d3, mut e3) = env_with(Some(after));
    let mut c3 = e3.client("test");
    let m3 = e3.sign_in(&mut c3);
    let r3: Vec<_> = notices(&m3).notices.into_iter().filter(|n| n.kind == 23).collect();
    assert_eq!((r3.len(), r3[0].f38), (1, 0));
    let closed = Schedule { start: now() - 2000, end: now() - 1500, exchange_end: now() - 1000, ..running() };
    let (_d4, mut e4) = env_with(Some(closed));
    let mut c4 = e4.client("test");
    let m4 = e4.sign_in(&mut c4);
    assert!(notices(&m4).notices.iter().all(|n| n.kind != 23));
}

#[test]
fn the_wallet_is_in_8b() {
    game_data::require_tables!();
    let (_d, mut e) = env_with(Some(running()));
    let mut c = e.client("test");
    e.store.update_profile(c.account, |p| {
        p.credit(Currency::Event1, 11);
        p.credit(Currency::Event2, 22);
        p.credit(Currency::Event3, 33);
        Ok::<_, ()>(())
    }).unwrap();
    let m = e.sign_in(&mut c);
    let v = money(&m).pop().expect("an 8B");
    assert_eq!((v.event1, v.event2, v.event3), (11, 22, 33));
}

fn denied_texts(m: &[S2c]) -> Vec<String> {
    m.iter()
        .filter_map(|x| match x {
            S2c::Denied(Denied { request: 0x48, body: DeniedBody::Text(t) }) => Some(t.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn an_affordable_exchange_charges_grants_and_acks() {
    game_data::require_tables!();
    let (_d, mut e) = env_with(Some(running()));
    let mut c = e.client("test");
    e.sign_in(&mut c);
    e.store.update_profile(c.account, |p| Ok::<_, ()>(p.credit(Currency::Event1, 800))).unwrap();
    // 951 x 750 -> 2202 (a character decal)
    exchange(&mut c, 951, 2202);
    e.pump(&mut [&mut c]);
    let m = c.take();
    assert_eq!(acks(&m), vec![3], "the request lock is cleared");
    let bought = m.iter().find_map(|x| if let S2c::Reply(Reply::Shop(Shop46::Buy { flag, items })) = x { Some((*flag, items.clone())) } else { None }).expect("the received-item reply");
    assert_eq!((bought.0, bought.1.len(), bought.1[0].dict), (1, 1, 2202));
    assert_eq!(money(&m).pop().unwrap().event1, 50, "the new wallet is pushed");
    let p = e.store.load_profile(c.account).unwrap();
    assert_eq!(p.event, [50, 0, 0]);
    assert!(p.items.iter().any(|i| i.dict_id == 2202));

    // the decal is owned: a second exchange is refused with the client's text, nothing is charged, the lock is cleared
    e.store.update_profile(c.account, |p| Ok::<_, ()>(p.credit(Currency::Event1, 800))).unwrap();
    exchange(&mut c, 951, 2202);
    e.pump(&mut [&mut c]);
    let m = c.take();
    assert_eq!((acks(&m), m.len()), (vec![3], 2));
    assert_eq!(denied_texts(&m), vec!["st_item_bought".to_string()]);
    assert_eq!(e.store.load_profile(c.account).unwrap().event, [850, 0, 0]);
}

#[test]
fn a_too_poor_or_unlisted_or_closed_exchange_changes_nothing_but_the_lock() {
    game_data::require_tables!();
    let (_d, mut e) = env_with(Some(running()));
    let mut c = e.client("test");
    e.sign_in(&mut c);
    e.store.update_profile(c.account, |p| Ok::<_, ()>(p.credit(Currency::Event3, 2499))).unwrap();
    let before = e.store.load_profile(c.account).unwrap();
    exchange(&mut c, 1139, 2200); // 2500 needed
    exchange(&mut c, 1139, 2202); // not a row
    e.pump(&mut [&mut c]);
    let m = c.take();
    assert_eq!((acks(&m), m.len()), (vec![3, 3], 3));
    assert_eq!(denied_texts(&m), vec!["st_not_enought_items_for_exchange".to_string()], "only the poor one gets a text");
    assert_eq!(e.store.load_profile(c.account).unwrap(), before);

    // no event configured: even a full wallet exchanges nothing (no window can show)
    let (_d2, mut e2) = env_with(None);
    let mut c2 = e2.client("test");
    e2.sign_in(&mut c2);
    e2.store.update_profile(c2.account, |p| Ok::<_, ()>(p.credit(Currency::Event3, 9000))).unwrap();
    exchange(&mut c2, 1139, 2200);
    e2.pump(&mut [&mut c2]);
    assert_eq!(acks(&c2.take()), vec![3]);
    assert_eq!(e2.store.load_profile(c2.account).unwrap().event, [0, 0, 9000]);
}

// ---- an online match pays event currency by result and place ----

const MATCH_ID: u32 = 0xE7E7_0001;

#[derive(Default)]
struct Fake {
    seats: Mutex<Vec<(i64, u8)>>,
    ended: Mutex<Vec<Ended>>,
}

impl OnlineMatches for Fake {
    fn available(&self, _: u32, _: u32) -> bool {
        true
    }

    fn create(&self, _: &GameData, _: u32, _: u32, _: u32, _: u8, seats: &[Seat]) -> Result<Ticket82, String> {
        *self.seats.lock().unwrap() = seats.iter().map(|s| (s.account, s.team)).collect();
        Ok(Ticket82 { match_id: MATCH_ID, host: "match.example".into(), port: 1236, orders: (0..seats.len() as u32).map(|i| 100 + i).collect() })
    }

    fn take_ended(&self) -> Vec<Ended> {
        std::mem::take(&mut *self.ended.lock().unwrap())
    }
}

fn pump_all(e: &mut Env, clients: &mut [Client]) {
    for _ in 0..40 {
        let mut refs: Vec<&mut Client> = clients.iter_mut().collect();
        e.step(&mut refs, Duration::from_millis(30));
    }
}

fn online_world(event: Option<Schedule>) -> (tempfile::TempDir, Env, Arc<Fake>, Vec<Client>, u32, u32, u32) {
    let gd = game_data::testing::tables();
    let row = gd.matches.modes().iter().filter(|r| r.min_players >= 2 && r.mode != 3 && r.mode != 9 && r.variation == 0).min_by_key(|r| (r.min_players, r.map, r.mode)).unwrap().clone();
    let n = row.min_players as usize;
    let dir = tempfile::tempdir().unwrap();
    let cfg = LobbyConfig {
        achievement_rewards: false,
        daily_quests: false,
        treasure_hunt: false,
        login_calendar: None,
        online_queue: Some((row.map, row.mode, row.weather)),
        online_report_grace: Duration::from_secs(5),
        online_min_players: n as u32,
        events: event.into_iter().collect(),
        ..LobbyConfig::default()
    };
    let mut e = Env::with(&dir.path().join("eo.sqlite"), cfg, Arc::new(MinimalMatches));
    let fake = Arc::new(Fake::default());
    e.server.lobby.online = Some(fake.clone());
    let mut clients: Vec<Client> = vec![];
    for i in 0..n {
        let mut c = e.client(&format!("player{i}"));
        c.send(Request::SignIn { session: c.session, a: 0xFF, b: 0xFF });
        for _ in 0..300 {
            let mut refs: Vec<&mut Client> = clients.iter_mut().collect();
            refs.push(&mut c);
            e.step(&mut refs, Duration::from_millis(30));
            if c.inbox.iter().any(|(id, _)| *id == 0xB8) {
                break;
            }
        }
        clients.push(c);
        pump_all(&mut e, &mut clients);
    }
    for c in clients.iter_mut() {
        c.take();
    }
    (dir, e, fake, clients, row.mode, n as u32, MATCH_ID)
}

/// Queue everybody, report team 0 as the winner, end the match with scores descending by client index.
fn play(e: &mut Env, fake: &Fake, clients: &mut Vec<Client>) -> Vec<(u8, u16)> {
    for i in 0..clients.len() {
        clients[i].send(Request::Queue { global: 0, arg: 0, mode: 0x380, flag: 0 });
        pump_all(e, clients);
    }
    let seats = fake.seats.lock().unwrap().clone();
    assert_eq!(seats.len(), clients.len());
    let team_of = |acc: i64| seats.iter().find(|s| s.0 == acc).unwrap().1;
    for i in 0..clients.len() {
        let id = if team_of(clients[i].account) == 0 { 5 } else { 6 };
        clients[i].send(Request::Events(vec![Event53 { id, x: 0, value: MATCH_ID, time: 0 }]));
        pump_all(e, clients);
    }
    let n = clients.len();
    let players = clients.iter().map(|c| EndedPlayer { account: c.account, team: team_of(c.account), entered: true, left_by_menu: false }).collect();
    // client i scores (2000 - 10 i): inside a team the lower index is the better place
    let stats: Vec<EndedStat> = clients.iter().enumerate().map(|(i, c)| EndedStat { account: c.account, kills: 1, deaths: 1, score: (2000 - 10 * i) as u16, ..Default::default() }).collect();
    fake.ended.lock().unwrap().push(Ended { match_id: MATCH_ID, duration: Duration::from_secs(600), players, stats: Some((stats, [1, 0])), winner: None });
    pump_all(e, clients);
    let mut out = vec![];
    for i in 0..n {
        let t = team_of(clients[i].account);
        let place = (0..i).filter(|&j| team_of(clients[j].account) == t).count() as u8;
        out.push((t, place as u16));
    }
    out
}

#[test]
fn an_online_match_during_the_event_pays_by_result_and_place() {
    game_data::require_tables!();
    let (_d, mut e, fake, mut clients, mode, n, _) = online_world(Some(running()));
    let t = play(&mut e, &fake, &mut clients);
    let gd = game_data::testing::tables();
    let mut paid_any = false;
    for i in 0..n as usize {
        let (team, place) = t[i];
        let want = gd.events.match_pay(mode, team == 0, place as u8, false);
        let got = e.store.load_profile(clients[i].account).unwrap().event;
        assert_eq!(got, want, "client {i}: team {team} place {place}");
        paid_any |= got.iter().any(|&v| v > 0);
        // the pushes carry it: 8B with the wallet, and the victory screen's drop list (83 45 0A) before the 97
        let m = clients[i].take();
        if want.iter().any(|&v| v > 0) {
            assert_eq!(money(&m).last().map(|v| [v.event1, v.event2, v.event3]), Some(want));
            let at = m.iter().position(|x| matches!(x, S2c::Reply(Reply::Inventory(Inventory45::MatchDrops { .. })))).expect("83 45 0A");
            let at97 = m.iter().position(|x| x.id() == 0x97).expect("97");
            assert!(at < at97, "the drops come before the results screen");
            let S2c::Reply(Reply::Inventory(Inventory45::MatchDrops { flag: 0, items })) = &m[at] else { panic!() };
            for (slot, &w) in want.iter().enumerate().filter(|(_, w)| **w > 0) {
                let d = items.iter().find(|d| d.item.dict == game_data::events::CURRENCIES[slot]).expect("the currency is listed");
                assert_eq!((d.amount, d.bonus, d.item.uid, d.item.bound), (w, 0, 0, 0), "no premium, no hat: no bonus");
            }
            assert!(!m.iter().any(|x| matches!(x, S2c::Reply(Reply::Shop(Shop46::Buy { flag: 1, .. })))), "no second popup");
        }
    }
    assert!(paid_any, "winners and losers of the first places are paid");
}

/// A won match drops a `random_module_chest` (prob 1.0) that the 0.69d lobby could never show (no inventory tab lists a
/// superCategory 5 item): the victory panel (83 45 0A) lists the chest, then one `83 46 06` per unit opens it (the client
/// takes the unit out again, adds the module, shows its chest notification) before the 8B; the profile keeps the module,
/// not the chest.
#[test]
fn a_won_online_match_opens_its_dropped_chests() {
    game_data::require_tables!();
    let (_d, mut e, fake, mut clients, _, n, _) = online_world(None);
    let gd = game_data::testing::tables();
    let t = play(&mut e, &fake, &mut clients);
    let mut winners = 0;
    for i in 0..n as usize {
        let m = clients[i].take();
        let p = e.store.load_profile(clients[i].account).unwrap();
        let at97 = m.iter().position(|x| x.id() == 0x97).expect("97");
        let Some(S2c::Reply(Reply::Inventory(Inventory45::MatchDrops { flag: 0, items }))) =
            m.iter().find(|x| matches!(x, S2c::Reply(Reply::Inventory(Inventory45::MatchDrops { .. }))))
        else {
            assert_ne!(t[i].0, 0, "client {i}: a winner has drops");
            continue;
        };
        let opened: Vec<(usize, u32, u16, &Vec<protocol::items::ItemRecord>)> = m
            .iter()
            .enumerate()
            .filter_map(|(k, x)| match x {
                S2c::Reply(Reply::Shop(Shop46::Chest { item, chest_dict, rewards })) => Some((k, *item, *chest_dict, rewards)),
                _ => None,
            })
            .collect();
        let chests: Vec<_> = items.iter().filter(|d| gd.chests.can_open(d.item.dict)).collect();
        if t[i].0 == 0 {
            winners += 1;
            assert!(chests.iter().any(|d| d.item.dict == 1872 && d.amount >= 1), "client {i}: the module chest is listed: {items:?}");
        }
        for d in &chests {
            let units: Vec<_> = opened.iter().filter(|o| o.2 == d.item.dict).collect();
            assert_eq!(units.len() as u32, d.amount, "client {i}: one 83 46 06 per unit of dict {}", d.item.dict);
            for (k, item, _, rewards) in units {
                assert_eq!(*item, d.item.uid, "the listed chest stack");
                assert!(*k > at97, "after the results screen's drop list");
                let at8b = m.iter().rposition(|x| matches!(x, S2c::Money(_))).expect("8B");
                assert!(*k < at8b, "before the wallet push");
                assert!(!rewards.is_empty());
                for r in rewards.iter().filter(|r| r.uid != 0) {
                    assert_eq!(p.item(r.uid).map(|x| x.dict_id), Some(r.dict), "the content is stored");
                }
            }
            assert!(p.item(d.item.uid).is_none(), "client {i}: no chest of dict {} stays in the inventory", d.item.dict);
        }
        // the module chest's content is one weapon module with a rarity (green / blue / orange) and its drop pairs,
        // stored and sent in the record
        for (_, _, dict, rewards) in opened.iter().filter(|o| o.2 == 1872) {
            assert_eq!(rewards.len(), 1, "{dict}: one module");
            assert!(gd.items.is_module(rewards[0].dict), "{:?}", rewards[0]);
            let r = game_data::merge::rarity(&rewards[0].mods);
            assert!((1..=3).contains(&r), "{:?}", rewards[0]);
            assert_eq!(p.mods.get(&rewards[0].uid).map(|m| game_data::merge::rarity(m)), Some(r));
            assert!(rewards[0].mods.len() >= 2, "rarity + drop pairs: {:?}", rewards[0]);
        }
    }
    assert!(winners > 0);
}

#[test]
fn the_first_pvp_win_of_the_event_gives_the_win_item_once() {
    game_data::require_tables!();
    let (_d, mut e, fake, mut clients, _, n, _) = online_world(Some(running()));
    let gd = game_data::testing::tables();
    let hat = gd.events.item_to_drop_after_win;
    let owned = |e: &Env, acc: i64| e.store.load_profile(acc).unwrap().items.iter().filter(|i| i.dict_id == hat).count() as u32; // a hat is one row each (not a stack)
    let t = play(&mut e, &fake, &mut clients);
    for i in 0..n as usize {
        let won = t[i].0 == 0;
        assert_eq!(owned(&e, clients[i].account), u32::from(won), "client {i}: the winners get one");
    }
    // a second win in the same event gives nothing more
    let t = play(&mut e, &fake, &mut clients);
    for i in 0..n as usize {
        let won = t[i].0 == 0;
        assert!(owned(&e, clients[i].account) <= 1, "client {i}: once per event");
        if won {
            assert_eq!(owned(&e, clients[i].account), 1);
        }
    }
}

#[test]
fn ranked_search_is_refused_while_the_event_runs() {
    game_data::require_tables!();
    let (_d, mut e, _fake, mut clients, _, _, _) = online_world(Some(running()));
    clients[0].send(Request::Queue { global: 0, arg: 2, mode: 0x380, flag: 0 });
    pump_all(&mut e, &mut clients);
    let m = clients[0].take();
    assert!(
        m.iter().any(|x| matches!(x, S2c::Denied(Denied { request: 0x2F, body: DeniedBody::Text(t) }) if t == lobby_server::queue::RANKED_OFF_DURING_EVENT)),
        "{m:?}"
    );
    assert!(m.contains(&S2c::QueueState(QueueState { state: 0 })), "the search stops (85 00)");
    assert!(e.server.registry.queue.ticket_of(clients[0].account).is_none());
}

#[test]
fn an_online_match_without_an_event_pays_no_event_currency() {
    game_data::require_tables!();
    let (_d, mut e, fake, mut clients, _, n, _) = online_world(None);
    play(&mut e, &fake, &mut clients);
    for i in 0..n as usize {
        assert_eq!(e.store.load_profile(clients[i].account).unwrap().event, [0, 0, 0]);
    }
    // and after the event ended nothing is paid either
    let over = Schedule { start: now() - 2000, end: now() - 1000, exchange_end: now() + 1000, ..running() };
    let (_d2, mut e2, fake2, mut clients2, _, n2, _) = online_world(Some(over));
    play(&mut e2, &fake2, &mut clients2);
    for i in 0..n2 as usize {
        assert_eq!(e2.store.load_profile(clients2[i].account).unwrap().event, [0, 0, 0]);
    }
}
