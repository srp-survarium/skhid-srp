//! PvE missions, lobby side (docs/notes/pve.md), with scripted clients: solo and squad Play with keys, commander
//! settings, ready / pays-for / locked slots, the refusals, the PvE queue with online matches off and with a recording
//! match server, and the top 100.

mod support;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use accounts::{public_id, PveRun, PveRunPlayer};
use game_data::pve::{KEY_FREE, KEY_PREMIUM};
use game_data::GameData;
use lobby_server::online::Left;
use lobby_server::{Ended, EndedPlayer, HostOutcome, HostPlayerStat, LobbyConfig, MinimalMatches, OnlineMatches, PveSetup, Seat, Ticket82};
use protocol::c2s::{Event53, Pve5F, Request, Squad49};
use protocol::s2c::pve::deny;
use protocol::s2c::*;
use protocol::Decode;
use support::*;

const MATCH_ID: u32 = 0x00C0_FFEE;

#[derive(Default)]
struct Fake {
    pve: Mutex<Vec<(PveSetup, Vec<(i64, u8)>)>>,
    ended: Mutex<Vec<Ended>>,
    fail: Mutex<Option<String>>,
    /// matches the lobby asked to end, and those of them it asked to end with the result screen on (DF code 2)
    ends: Mutex<Vec<u32>>,
    kept: Mutex<Vec<u32>>,
    /// players who left a running match (menu, disconnect)
    left: Mutex<Vec<Left>>,
    /// the world host's outcomes, and whether a world host is in use for the match
    outcomes: Mutex<Vec<HostOutcome>>,
    host: std::sync::atomic::AtomicBool,
}

/// The mission clock the fake match server reports (12 min 34 s).
const PLAYED: Duration = Duration::from_secs(754);

impl OnlineMatches for Fake {
    fn available(&self, _: u32, _: u32) -> bool {
        true
    }

    fn create(&self, _: &GameData, _: u32, _: u32, _: u32, _: u8, _: &[Seat]) -> Result<Ticket82, String> {
        Err("a PvP match in a PvE test".into())
    }

    fn create_pve(&self, _: &GameData, setup: &PveSetup, seats: &[Seat]) -> Result<Ticket82, String> {
        if let Some(e) = self.fail.lock().unwrap().clone() {
            return Err(e);
        }
        self.pve.lock().unwrap().push((setup.clone(), seats.iter().map(|s| (s.account, s.team)).collect()));
        Ok(Ticket82 { match_id: MATCH_ID, host: "match.example".into(), port: 1236, orders: (0..seats.len() as u32).map(|i| 500 + i).collect() })
    }

    fn take_ended(&self) -> Vec<Ended> {
        std::mem::take(&mut *self.ended.lock().unwrap())
    }

    fn end(&self, match_id: u32) -> bool {
        self.ends.lock().unwrap().push(match_id);
        true
    }

    fn end_after_result(&self, match_id: u32) -> bool {
        self.kept.lock().unwrap().push(match_id);
        self.end(match_id)
    }

    fn played(&self, _: u32) -> Option<Duration> {
        Some(PLAYED)
    }

    fn take_left(&self) -> Vec<Left> {
        std::mem::take(&mut *self.left.lock().unwrap())
    }

    fn take_outcomes(&self) -> Vec<HostOutcome> {
        std::mem::take(&mut *self.outcomes.lock().unwrap())
    }

    fn world_host(&self, _: u32) -> bool {
        self.host.load(std::sync::atomic::Ordering::SeqCst)
    }
}

struct World {
    _dir: tempfile::TempDir,
    e: Env,
    fake: Arc<Fake>,
}

fn world(online: bool) -> World {
    let dir = tempfile::tempdir().unwrap();
    let cfg = LobbyConfig {
        achievement_rewards: false,
        daily_quests: false,
        treasure_hunt: false,
        login_calendar: None,
        online_report_grace: Duration::from_secs(5),
        ..LobbyConfig::default()
    };
    let mut e = Env::with(&dir.path().join("pve.sqlite"), cfg, Arc::new(MinimalMatches));
    let fake = Arc::new(Fake::default());
    if online {
        e.server.lobby.online = Some(fake.clone());
    }
    World { _dir: dir, e, fake }
}

fn up(w: &mut World, name: &str) -> Client {
    let mut c = w.e.client(name);
    w.e.sign_in(&mut c);
    c
}

fn give(w: &World, c: &Client, dict: u16, n: u32) {
    w.e.store
        .update_profile(c.account, |p| {
            p.add_item(dict, n);
            Ok::<(), ()>(())
        })
        .unwrap();
}

fn set_gold(w: &World, c: &Client, gold: u32) {
    w.e.store
        .update_profile(c.account, |p| {
            p.gold = gold;
            Ok::<(), ()>(())
        })
        .unwrap();
}

fn keys(w: &World, c: &Client) -> (u32, u32) {
    let p = w.e.store.load_profile(c.account).unwrap();
    let n = |d: u16| p.items.iter().filter(|i| i.dict_id == d).map(|i| i.count).sum();
    (n(KEY_FREE), n(KEY_PREMIUM))
}

fn gold(w: &World, c: &Client) -> u32 {
    w.e.store.load_profile(c.account).unwrap().gold
}

/// Remove every access key of a player (a won run gives free daily keys).
fn drop_keys(w: &World, c: &Client) {
    w.e.store
        .update_profile(c.account, |p| {
            p.items.retain(|i| i.dict_id != KEY_FREE && i.dict_id != KEY_PREMIUM);
            Ok::<(), ()>(())
        })
        .unwrap();
}

fn play_solo(w: &mut World, c: &mut Client, difficulty: u32, rest: &mut [&mut Client]) {
    c.send(Request::Pve(Pve5F::Start { solo: true, map: 15, difficulty }));
    pump(w, c, rest);
}

fn pump(w: &mut World, c: &mut Client, rest: &mut [&mut Client]) {
    let mut all: Vec<&mut Client> = vec![c];
    all.extend(rest.iter_mut().map(|x| &mut **x));
    w.e.pump(&mut all);
}

fn pump_all(w: &mut World, all: &mut [&mut Client]) {
    w.e.pump(all);
}

fn denial(msgs: &[S2c]) -> Option<u8> {
    msgs.iter().find_map(|m| match m {
        S2c::Denied(Denied { request: 0x5F, body: DeniedBody::Pve { code } }) => Some(*code),
        _ => None,
    })
}

fn idle_count(msgs: &[S2c]) -> usize {
    msgs.iter().filter(|m| **m == S2c::QueueState(QueueState { state: 0 })).count()
}

fn searching(msgs: &[S2c]) -> Option<u32> {
    msgs.iter().find_map(|m| match m {
        S2c::Raw { id: 0x85, payload } if payload.first() == Some(&4) => PveSearching::from_payload(payload).ok().map(|s| s.order),
        _ => None,
    })
}

fn connect(msgs: &[S2c]) -> Option<ConnectToMatch> {
    msgs.iter().find_map(|m| match m {
        S2c::Raw { id: 0x82, payload } => ConnectToMatch::from_bytes(payload).ok(),
        _ => None,
    })
}

fn last_ae(msgs: &[S2c]) -> Option<Squad> {
    msgs.iter().rev().find_map(|m| if let S2c::Squad(s) = m { s.squad.clone() } else { None })
}

fn settings_push(msgs: &[S2c]) -> Option<PveSettingsChanged> {
    msgs.iter().find_map(|m| match m {
        S2c::Raw { id: 0xC2, payload } if payload.first() == Some(&0) => PveSettingsChanged::from_push(payload).ok(),
        _ => None,
    })
}

/// `who` invites `target` to a PvE squad (4D 17), the target accepts (4D 18), the inviter's client sends 49 02.
fn pve_join(w: &mut World, who: &mut Client, target: &mut Client, rest: &mut [&mut Client]) {
    let (pw, pt) = (public_id(who.account), public_id(target.account));
    let mut all: Vec<&mut Client> = vec![&mut *who, &mut *target];
    all.extend(rest.iter_mut().map(|c| &mut **c));
    all[0].send(Request::People { id: pt, kind: 0x17, text: String::new() });
    w.e.pump(&mut all);
    let inv = all[1].take();
    let m = inv.iter().find_map(|m| if let S2c::SystemMessage(s) = m { Some(s.clone()) } else { None }).expect("B1 invite");
    assert_eq!((m.kind, m.sender), (0x17, pw));
    all[1].send(Request::People { id: pw, kind: 0x18, text: String::new() });
    w.e.pump(&mut all);
    let acc = all[0].take();
    let m = acc.iter().find_map(|m| if let S2c::SystemMessage(s) = m { Some(s.clone()) } else { None }).expect("B1 accept");
    assert_eq!((m.kind, m.sender), (0x18, pt));
    all[0].send(Request::Squad(Squad49::Accepted { player: m.sender }));
    w.e.pump(&mut all);
}

#[test]
fn three_solo_players_fill_a_match_and_pay_keys() {
    game_data::require_tables!();
    let mut w = world(true);
    let (mut a, mut b, mut c) = (up(&mut w, "alpha"), up(&mut w, "bravo"), up(&mut w, "charlie"));
    give(&w, &a, KEY_FREE, 1);
    give(&w, &a, KEY_PREMIUM, 1);
    give(&w, &b, KEY_PREMIUM, 1);
    let gold_c = gold(&w, &c);
    let order = {
        play_solo(&mut w, &mut a, 1, &mut [&mut b, &mut c]);
        let m = a.take();
        assert_eq!(denial(&m), None);
        searching(&m).expect("85 04: the PvE search runs")
    };
    // nothing is charged while the search waits
    assert_eq!((keys(&w, &a), gold(&w, &a)), ((1, 1), gold(&w, &a)));
    assert!(w.fake.pve.lock().unwrap().is_empty());
    assert_eq!(w.e.server.registry.pve.len(), 1);
    // pressing Play again shows the same search
    play_solo(&mut w, &mut a, 1, &mut [&mut b, &mut c]);
    assert_eq!(searching(&a.take()), Some(order));

    play_solo(&mut w, &mut b, 1, &mut [&mut a, &mut c]);
    assert!(w.fake.pve.lock().unwrap().is_empty(), "two players are not a match");
    play_solo(&mut w, &mut c, 1, &mut [&mut a, &mut b]);

    // one match: difficulty 1, one team of 3 humans, a project of the difficulty
    let created = w.fake.pve.lock().unwrap().clone();
    assert_eq!(created.len(), 1);
    let (setup, seats) = &created[0];
    assert_eq!((setup.mission, setup.difficulty, setup.human_players), (15, 1, 3));
    assert!(setup.project.starts_with("pve_01_v2_maze"), "{}", setup.project);
    assert_eq!(seats.len(), 3);
    assert!(seats.iter().all(|s| s.1 == 0), "every human is on team 0");
    let (ma, mb, mc) = (a.take(), b.take(), c.take());
    for m in [&ma, &mb, &mc] {
        let t = connect(m).expect("82 for every player");
        assert_eq!((t.host.as_str(), t.match_id, t.side), ("match.example", MATCH_ID, 0));
        assert_eq!(idle_count(m), 0);
    }
    // charges: the free key goes first, then the premium key; without a key the ticket costs the gold price
    assert_eq!(keys(&w, &a), (0, 1));
    assert_eq!(keys(&w, &b), (0, 0));
    assert_eq!(gold(&w, &c), gold_c - 50);
    assert!(ma.iter().any(|m| matches!(m, S2c::ItemCounts(i) if i.entries.len() == 1 && i.entries[0].1 == 1)), "90 takes the key from the client");
    assert!(mc.iter().any(|m| matches!(m, S2c::Money(x) if x.gold == gold_c - 50)), "8B shows the gold");
    assert!(w.e.server.registry.pve.is_empty());
    assert_eq!(w.e.server.registry.online.len(), 1);

    // the match ends: the players are released (85 00), nobody is paid by the PvP path
    let silver_before = w.e.store.load_profile(a.account).unwrap().silver;
    let players = [&a, &b, &c].iter().map(|x| EndedPlayer { account: x.account, team: 0, entered: true, left_by_menu: false }).collect();
    w.fake.ended.lock().unwrap().push(Ended { match_id: MATCH_ID, duration: Duration::from_secs(900), players, stats: None, winner: None });
    w.e.wait(&mut [&mut a, &mut b, &mut c], 8);
    for x in [&mut a, &mut b, &mut c] {
        assert_eq!(idle_count(&x.take()), 1, "85 00 after the match");
    }
    assert_eq!(w.e.store.load_profile(a.account).unwrap().silver, silver_before);
    assert!(w.e.server.registry.online.is_empty());
}

#[test]
fn cancel_ends_the_search_without_a_charge() {
    game_data::require_tables!();
    let mut w = world(true);
    let (mut a, mut b, mut c) = (up(&mut w, "alpha"), up(&mut w, "bravo"), up(&mut w, "charlie"));
    give(&w, &a, KEY_FREE, 1);
    play_solo(&mut w, &mut a, 0, &mut [&mut b, &mut c]);
    let order = searching(&a.take()).unwrap();
    a.send(Request::QueueCancel { order });
    pump(&mut w, &mut a, &mut [&mut b, &mut c]);
    assert_eq!(idle_count(&a.take()), 1);
    assert!(w.e.server.registry.pve.is_empty());
    assert_eq!(keys(&w, &a), (1, 0));
    // the other two cannot make a match alone
    play_solo(&mut w, &mut b, 0, &mut [&mut a, &mut c]);
    play_solo(&mut w, &mut c, 0, &mut [&mut a, &mut b]);
    assert!(w.fake.pve.lock().unwrap().is_empty());
    // a cancel without a search is answered with 85 00
    a.send(Request::QueueCancel { order: 0 });
    pump(&mut w, &mut a, &mut []);
    assert_eq!(idle_count(&a.take()), 1);
}

#[test]
fn squad_commander_settings_ready_pays_for_and_play() {
    game_data::require_tables!();
    let mut w = world(true);
    let (mut a, mut b, mut c) = (up(&mut w, "alpha"), up(&mut w, "bravo"), up(&mut w, "charlie"));
    give(&w, &a, KEY_FREE, 1);
    give(&w, &b, KEY_PREMIUM, 1);
    set_gold(&w, &c, 0);
    let gold_b = gold(&w, &b);
    pve_join(&mut w, &mut a, &mut b, &mut [&mut c]);
    pve_join(&mut w, &mut a, &mut c, &mut [&mut b]);
    // the squad is of type 1 (the PvE window opens), alpha commands
    let sa = last_ae(&a.take()).unwrap();
    assert_eq!((sa.kind, sa.i_am_commander, sa.members.len()), (1, 1, 3));
    assert!(last_ae(&b.take()).is_some_and(|s| s.kind == 1 && s.i_am_commander == 0));
    c.take();

    // commander settings: map 15, difficulty 2 -> C2 00 to the members only
    a.send(Request::Pve(Pve5F::Settings { map: 15, difficulty: 2 }));
    pump_all(&mut w, &mut [&mut a, &mut b, &mut c]);
    assert_eq!(settings_push(&a.take()), None);
    assert_eq!(settings_push(&b.take()), Some(PveSettingsChanged { map: 15, difficulty: 2 }));
    assert_eq!(settings_push(&c.take()), Some(PveSettingsChanged { map: 15, difficulty: 2 }));
    // a member cannot set them, a bad mission is ignored
    b.send(Request::Pve(Pve5F::Settings { map: 15, difficulty: 0 }));
    a.send(Request::Pve(Pve5F::Settings { map: 99, difficulty: 0 }));
    pump_all(&mut w, &mut [&mut a, &mut b, &mut c]);
    assert!(settings_push(&a.take()).is_none() && settings_push(&b.take()).is_none() && settings_push(&c.take()).is_none());

    // Play with nobody ready: code 5, Play works again (85 00), nothing queued
    a.send(Request::Pve(Pve5F::Start { solo: false, map: 15, difficulty: 2 }));
    pump_all(&mut w, &mut [&mut a, &mut b, &mut c]);
    let m = a.take();
    assert_eq!(denial(&m), Some(deny::NOT_ALL_READY));
    assert_eq!(idle_count(&m), 1);
    assert!(w.e.server.registry.pve.is_empty());

    // bravo is ready and pays for charlie (slot 2 = bit 3); charlie is ready and tries to pay for himself (dropped)
    b.send(Request::Squad(Squad49::PveState { state: 1 | 8, zero: 0 }));
    c.send(Request::Squad(Squad49::PveState { state: 1 | 8, zero: 0 }));
    pump_all(&mut w, &mut [&mut a, &mut b, &mut c]);
    let ae = last_ae(&a.take()).unwrap();
    assert_eq!(ae.members.iter().map(|m| m.state).collect::<Vec<_>>(), vec![0, 9, 1]);
    b.take();
    c.take();
    // the commander cannot set his own ready bit (his Play is 5F 01)
    a.send(Request::Squad(Squad49::PveState { state: 1, zero: 0 }));
    pump_all(&mut w, &mut [&mut a, &mut b, &mut c]);
    assert_eq!(last_ae(&a.take()).unwrap().members[0].state, 0);

    // Play: 3 players fill the mission at once
    a.send(Request::Pve(Pve5F::Start { solo: false, map: 15, difficulty: 2 }));
    pump_all(&mut w, &mut [&mut a, &mut b, &mut c]);
    let created = w.fake.pve.lock().unwrap().clone();
    assert_eq!(created.len(), 1);
    assert_eq!((created[0].0.difficulty, created[0].0.human_players), (2, 3));
    assert!(created[0].0.project.starts_with("pve_01_v3_maze"));
    for x in [&mut a, &mut b, &mut c] {
        let m = x.take();
        assert!(connect(&m).is_some(), "82: {m:?}");
        assert_eq!(denial(&m), None);
    }
    // alpha: the free key. bravo: his own premium key and the sponsor price for charlie. charlie: nothing.
    assert_eq!(keys(&w, &a), (0, 0));
    assert_eq!(keys(&w, &b), (0, 0));
    assert_eq!(gold(&w, &b), gold_b - 50);
    assert_eq!((keys(&w, &c), gold(&w, &c)), ((0, 0), 0));
}

#[test]
fn a_locked_slot_lowers_the_size_and_costs_gold() {
    game_data::require_tables!();
    let mut w = world(true);
    let (mut a, mut b) = (up(&mut w, "alpha"), up(&mut w, "bravo"));
    give(&w, &a, KEY_PREMIUM, 1);
    give(&w, &b, KEY_FREE, 1);
    let gold_a = gold(&w, &a);
    pve_join(&mut w, &mut a, &mut b, &mut []);
    a.take();
    b.take();
    // two locks do not fit: only the empty seat player2 (bit 0x20) can be locked, player1 is bravo's seat
    a.send(Request::Squad(Squad49::PveState { state: 0x30, zero: 0 }));
    pump_all(&mut w, &mut [&mut a, &mut b]);
    assert_eq!(last_ae(&a.take()).unwrap().members[0].state, 0x20);
    b.take();
    // without the lock a squad of 2 would wait for a third player; with it the mission is full at 2
    b.send(Request::Squad(Squad49::PveState { state: 1, zero: 0 }));
    pump_all(&mut w, &mut [&mut a, &mut b]);
    a.send(Request::Pve(Pve5F::Start { solo: false, map: 15, difficulty: 0 }));
    pump_all(&mut w, &mut [&mut a, &mut b]);
    let created = w.fake.pve.lock().unwrap().clone();
    assert_eq!(created.len(), 1, "the squad of 2 starts alone");
    assert_eq!((created[0].0.difficulty, created[0].0.human_players), (0, 2));
    assert_eq!(keys(&w, &a), (0, 0));
    assert_eq!(keys(&w, &b), (0, 0));
    assert_eq!(gold(&w, &a), gold_a - 50, "one locked slot costs the block-slot price");
}

/// `who` invites `target` to a normal squad (4D 2), the target accepts (4D 3), the inviter's client sends 49 02.
fn pvp_join(w: &mut World, who: &mut Client, target: &mut Client) {
    let (pw, pt) = (public_id(who.account), public_id(target.account));
    who.send(Request::People { id: pt, kind: 2, text: String::new() });
    pump_all(w, &mut [&mut *who, &mut *target]);
    target.take();
    target.send(Request::People { id: pw, kind: 3, text: String::new() });
    pump_all(w, &mut [&mut *who, &mut *target]);
    who.take();
    who.send(Request::Squad(Squad49::Accepted { player: pt }));
    pump_all(w, &mut [&mut *who, &mut *target]);
}

fn queue_2f() -> Request {
    Request::Queue { global: u32::MAX, arg: 0, mode: 0x380, flag: 0 }
}

/// A squad made in the main lobby (type 0): the commander opens the PvE window (5F 00) and the squad becomes a PvE squad
/// (AE type 1 to both, C2 00 to the member); the commander's seat lock and the member's ready then work, and Play with
/// online matches off is refused with the no-match-server code.
#[test]
fn the_commanders_pve_window_turns_a_normal_squad_into_a_pve_squad() {
    game_data::require_tables!();
    let mut w = world(false);
    let (mut a, mut b) = (up(&mut w, "alpha"), up(&mut w, "bravo"));
    give(&w, &a, KEY_PREMIUM, 1);
    give(&w, &b, KEY_FREE, 1);
    pvp_join(&mut w, &mut a, &mut b);
    assert_eq!(last_ae(&a.take()).unwrap().kind, 0);
    assert_eq!(last_ae(&b.take()).unwrap().kind, 0);

    a.send(Request::Pve(Pve5F::Settings { map: 15, difficulty: 0 }));
    pump_all(&mut w, &mut [&mut a, &mut b]);
    let (ma, mb) = (a.take(), b.take());
    assert_eq!(last_ae(&ma).map(|s| (s.kind, s.i_am_commander)), Some((SQUAD_TYPE_PVE, 1)));
    assert_eq!(last_ae(&mb).map(|s| (s.kind, s.i_am_commander)), Some((SQUAD_TYPE_PVE, 0)));
    assert!(last_ae(&mb).unwrap().members.iter().all(|m| m.state == 0), "state bytes cleared");
    assert_eq!(settings_push(&mb), Some(PveSettingsChanged { map: 15, difficulty: 0 }));
    assert_eq!(settings_push(&ma), None);

    // the commander locks the empty seat, the member is ready
    a.send(Request::Squad(Squad49::PveState { state: 0x20, zero: 0xFF }));
    pump_all(&mut w, &mut [&mut a, &mut b]);
    assert_eq!(last_ae(&a.take()).unwrap().members[0].state, 0x20);
    b.take();
    b.send(Request::Squad(Squad49::PveState { state: 1, zero: 0 }));
    pump_all(&mut w, &mut [&mut a, &mut b]);
    assert_eq!(last_ae(&a.take()).unwrap().members.iter().map(|m| m.state).collect::<Vec<_>>(), vec![0x20, 1]);
    b.take();

    // Play: everything checks out up to the match server, which is off
    a.send(Request::Pve(Pve5F::Start { solo: false, map: 15, difficulty: 0 }));
    pump_all(&mut w, &mut [&mut a, &mut b]);
    let m = a.take();
    assert_eq!(denial(&m), Some(deny::NO_MATCH_SERVER), "{m:?}");
    assert_eq!(idle_count(&m), 1);
}

/// A member of a normal squad: his PvE ready / lock (49 05) and his settings (5F 00) are refused and the squad stays
/// normal.
#[test]
fn a_members_pve_requests_leave_a_normal_squad_normal() {
    game_data::require_tables!();
    let mut w = world(false);
    let (mut a, mut b) = (up(&mut w, "alpha"), up(&mut w, "bravo"));
    pvp_join(&mut w, &mut a, &mut b);
    a.take();
    b.take();
    b.send(Request::Squad(Squad49::PveState { state: 1, zero: 0 }));
    pump_all(&mut w, &mut [&mut a, &mut b]);
    let s = last_ae(&b.take()).expect("the sender gets his AE");
    assert_eq!(s.kind, 0);
    assert!(s.members.iter().all(|m| m.state == 0), "{s:?}");
    b.send(Request::Squad(Squad49::PveState { state: 0x20, zero: 0xFF }));
    pump_all(&mut w, &mut [&mut a, &mut b]);
    b.send(Request::Pve(Pve5F::Settings { map: 15, difficulty: 0 }));
    pump_all(&mut w, &mut [&mut a, &mut b]);
    b.take();
    assert_eq!(w.e.server.registry.squads.squad_of(a.account).map(|s| s.kind), Some(0));
    assert!(settings_push(&a.take()).is_none());
}

/// A lock by the commander of a normal squad converts it too (a lock that came before the window's 5F 00).
#[test]
fn a_commanders_seat_lock_turns_a_normal_squad_into_a_pve_squad() {
    game_data::require_tables!();
    let mut w = world(false);
    let (mut a, mut b) = (up(&mut w, "alpha"), up(&mut w, "bravo"));
    pvp_join(&mut w, &mut a, &mut b);
    a.take();
    b.take();
    a.send(Request::Squad(Squad49::PveState { state: 0x20, zero: 0xFF }));
    pump_all(&mut w, &mut [&mut a, &mut b]);
    let s = last_ae(&b.take()).unwrap();
    assert_eq!((s.kind, s.members[0].state), (SQUAD_TYPE_PVE, 0x20));
}

/// The commander's PvP Play (2F) turns a PvE squad back into a normal squad: AE type 0 to both and 85 (the member is
/// not ready); the member's 2F is his ready; the commander's next 2F queues the squad for PvP.
#[test]
fn the_commanders_pvp_play_turns_a_pve_squad_back() {
    game_data::require_tables!();
    let mut w = world(true);
    let (mut a, mut b) = (up(&mut w, "alpha"), up(&mut w, "bravo"));
    pve_join(&mut w, &mut a, &mut b, &mut []);
    a.take();
    b.take();
    a.send(queue_2f());
    pump_all(&mut w, &mut [&mut a, &mut b]);
    let (ma, mb) = (a.take(), b.take());
    assert_eq!(last_ae(&ma).map(|s| s.kind), Some(0), "{ma:?}");
    assert_eq!(last_ae(&mb).map(|s| s.kind), Some(0), "{mb:?}");
    assert!(ma.iter().any(|m| m.id() == 0x85), "85 to the commander: {ma:?}");
    assert!(w.e.server.registry.queue.ticket_of(a.account).is_none(), "the member was not ready");
    // the member's 2F: ready
    b.send(queue_2f());
    pump_all(&mut w, &mut [&mut a, &mut b]);
    assert_eq!(last_ae(&a.take()).unwrap().members[1].state, 1);
    b.take();
    // the commander's 2F: the squad's PvP search
    a.send(queue_2f());
    pump_all(&mut w, &mut [&mut a, &mut b]);
    assert!(w.e.server.registry.queue.ticket_of(a.account).is_some(), "the squad searches PvP");
    assert!(w.e.server.registry.queue.ticket_of(b.account).is_some());
}

/// A member's 2F in a PvE squad does not convert it (85 00 as before).
#[test]
fn a_members_pvp_play_keeps_the_pve_squad() {
    game_data::require_tables!();
    let mut w = world(true);
    let (mut a, mut b) = (up(&mut w, "alpha"), up(&mut w, "bravo"));
    pve_join(&mut w, &mut a, &mut b, &mut []);
    a.take();
    b.take();
    b.send(queue_2f());
    pump_all(&mut w, &mut [&mut a, &mut b]);
    assert_eq!(idle_count(&b.take()), 1);
    assert_eq!(w.e.server.registry.squads.squad_of(a.account).map(|s| s.kind), Some(SQUAD_TYPE_PVE));
}

/// Two-member PvE squad: a lock bit only for an empty seat; 0x10 is bravo's seat (no lock), 0x30 keeps 0x20.
#[test]
fn a_lock_bit_only_for_an_empty_seat() {
    game_data::require_tables!();
    let mut w = world(true);
    let (mut a, mut b) = (up(&mut w, "alpha"), up(&mut w, "bravo"));
    pve_join(&mut w, &mut a, &mut b, &mut []);
    a.take();
    b.take();
    a.send(Request::Squad(Squad49::PveState { state: 0x10, zero: 0xFF }));
    pump_all(&mut w, &mut [&mut a, &mut b]);
    assert_eq!(last_ae(&a.take()).unwrap().members[0].state, 0);
    a.send(Request::Squad(Squad49::PveState { state: 0x30, zero: 0xFF }));
    pump_all(&mut w, &mut [&mut a, &mut b]);
    assert_eq!(last_ae(&a.take()).unwrap().members[0].state, 0x20);
}

fn result99(msgs: &[S2c]) -> Option<PveResult> {
    msgs.iter().find_map(|m| match m {
        S2c::Raw { id: 0x99, payload } => PveResult::from_bytes(payload).ok(),
        _ => None,
    })
}

/// The end of an online mission: the player's "won" report (53 event 5, value = match id) ->
/// `99` (the match id, success, the match server's mission time, his own row) and the drops `83 45 0B` at once, the run
/// stored (a won run gives the daily key); the match server is asked to end the match only PVE_END_AFTER_RESULT (3 s)
/// later, with the result screen on (DF code 2); after its end the `85 00` waits for the player's `5F 04` (he left the
/// screen) and nothing is sent twice.
#[test]
fn an_online_mission_end_sends_99_and_the_drops_before_the_match_ends() {
    game_data::require_tables!();
    let mut w = world(true);
    let mut a = up(&mut w, "alpha");
    give(&w, &a, KEY_PREMIUM, 1);
    a.send(Request::Squad(Squad49::PveState { state: 0x30, zero: 0xFF }));
    pump_all(&mut w, &mut [&mut a]);
    a.send(Request::Pve(Pve5F::Start { solo: false, map: 15, difficulty: 0 }));
    pump_all(&mut w, &mut [&mut a]);
    assert!(connect(&a.take()).is_some());
    assert_eq!(keys(&w, &a), (0, 0));

    a.send(Request::Events(vec![Event53 { id: 5, x: 0, value: MATCH_ID, time: 0 }]));
    pump_all(&mut w, &mut [&mut a]);
    let m = a.take();
    let r = result99(&m).expect("99");
    assert_eq!((r.match_id, r.success, r.time_secs, r.chests_total, r.me), (MATCH_ID, true, PLAYED.as_secs() as u32, 10, 0));
    assert_eq!(r.players.len(), 1);
    assert_eq!((r.players[0].account, r.players[0].name.as_str()), (public_id(a.account), "alpha"));
    assert_eq!(r.chests_found, 0, "chests found are not reported by any client (OPEN)");
    assert!(drops(&m).is_some(), "83 45 0B: the end screen shows after 99 and the drops");
    assert_eq!(idle_count(&m), 0, "still in the match");
    assert!(w.fake.ends.lock().unwrap().is_empty(), "the match is not ended before the result went out");
    assert_eq!(rows(&w, a.account, i64::from(MATCH_ID)).len(), 0, "no chest without a reported chest");
    assert_eq!(w.e.store.pve_result_ids_of(a.account).unwrap().len(), 1, "the run is stored");
    assert_eq!(keys(&w, &a).0, 1, "the daily key of a won mission");

    w.e.wait(&mut [&mut a], 4);
    assert_eq!(*w.fake.kept.lock().unwrap(), vec![MATCH_ID], "then the match server ends the match, the result screen on");
    assert_eq!(*w.fake.ends.lock().unwrap(), vec![MATCH_ID]);
    assert!(result99(&a.take()).is_none(), "the result goes out once");
    let players = vec![EndedPlayer { account: a.account, team: 0, entered: true, left_by_menu: false }];
    w.fake.ended.lock().unwrap().push(Ended { match_id: MATCH_ID, duration: PLAYED, players, stats: None, winner: None });
    w.e.wait(&mut [&mut a], 8);
    assert_eq!(idle_count(&a.take()), 0, "no 85 00 while the result screen is open");
    a.send(Request::Pve(Pve5F::CloseStats));
    pump_all(&mut w, &mut [&mut a]);
    let m = a.take();
    assert_eq!(idle_count(&m), 1, "85 00 after the player left the screen");
    assert!(result99(&m).is_none() && drops(&m).is_none());
    assert!(w.e.server.registry.online.is_empty());
}

/// A mission the match server ended on the script's failure (pre-start cancel / 45-minute end / everybody left: winner
/// 1, match_server::PveEndRule): the client's own failure screen (CHOICE): `99` with success 0, then
/// `83 45 0B` (no chest), then `85 00`, all at once (no report needed). The failed run is stored (no daily key, never on
/// the top 100); the key stays spent, no leaver penalty. A "won" report cannot turn it into a success.
#[test]
fn a_mission_failed_by_the_script_sends_99_failure_then_releases_the_player() {
    game_data::require_tables!();
    let mut w = world(true);
    let mut a = up(&mut w, "alpha");
    give(&w, &a, KEY_PREMIUM, 1);
    a.send(Request::Squad(Squad49::PveState { state: 0x30, zero: 0xFF }));
    pump_all(&mut w, &mut [&mut a]);
    a.send(Request::Pve(Pve5F::Start { solo: false, map: 15, difficulty: 0 }));
    pump_all(&mut w, &mut [&mut a]);
    assert!(connect(&a.take()).is_some());
    assert_eq!(keys(&w, &a), (0, 0));
    let silver = w.e.store.load_profile(a.account).unwrap().silver;
    let players = vec![EndedPlayer { account: a.account, team: 0, entered: true, left_by_menu: false }];
    w.fake.ended.lock().unwrap().push(Ended { match_id: MATCH_ID, duration: PLAYED, players, stats: None, winner: Some(1) });
    a.send(Request::Events(vec![Event53 { id: 5, x: 0, value: MATCH_ID, time: 0 }]));
    pump_all(&mut w, &mut [&mut a]);
    w.e.wait(&mut [&mut a], 2);
    let m = a.take();
    assert_eq!(idle_count(&m), 1, "85 00 at once: a winner from the match server needs no reports");
    let r = result99(&m).expect("99 for a failure");
    assert_eq!((r.match_id, r.success, r.time_secs, r.chests_found, r.chests_total), (MATCH_ID, false, PLAYED.as_secs() as u32, 0, 10));
    let (items, _, _) = drops(&m).expect("83 45 0B");
    assert!(items.is_empty(), "no chest for a failed mission");
    assert!(seq(&m) == ["99", "83 45 0B", "85 00"], "99, the drops, then 85 00: {:?}", seq(&m));
    assert_eq!(w.e.store.pve_result_ids_of(a.account).unwrap().len(), 1, "the failed run is stored");
    assert_eq!(keys(&w, &a), (0, 0), "the key stays spent, no daily key");
    assert_eq!(w.e.store.load_profile(a.account).unwrap().silver, silver, "no leaver fine in PvE");
    assert!(w.fake.ends.lock().unwrap().is_empty(), "the match server ended it itself");
    assert!(w.e.server.registry.online.is_empty());
}

/// The order of the end messages a player got: `99`, `83 45 0B` (the drops), `85 00`.
fn seq(msgs: &[S2c]) -> Vec<&'static str> {
    msgs.iter()
        .filter_map(|m| match m {
            S2c::Raw { id: 0x99, .. } => Some("99"),
            S2c::Reply(Reply::Inventory(Inventory45::PveDrops { .. })) => Some("83 45 0B"),
            _ if *m == S2c::QueueState(QueueState { state: 0 }) => Some("85 00"),
            _ => None,
        })
        .collect()
}

/// A solo PvE mission running on the fake match server, with a world host in use for it.
fn host_mission(w: &mut World) -> Client {
    let mut a = up(w, "alpha");
    give(w, &a, KEY_PREMIUM, 1);
    a.send(Request::Squad(Squad49::PveState { state: 0x30, zero: 0xFF }));
    pump_all(w, &mut [&mut a]);
    a.send(Request::Pve(Pve5F::Start { solo: false, map: 15, difficulty: 0 }));
    pump_all(w, &mut [&mut a]);
    assert!(connect(&a.take()).is_some());
    w.fake.host.store(true, std::sync::atomic::Ordering::SeqCst);
    a
}

/// The world host's outcome of that mission: 40 kills, 1 death, score 590, 2 caches opened, 20 min 34.567 s.
fn host_outcome(a: &Client, success: bool) -> HostOutcome {
    HostOutcome {
        match_id: MATCH_ID,
        state: 5,
        winner_team: if success { 0 } else { 1 },
        success,
        mission_ms: 1_234_567,
        players: vec![HostPlayerStat { account: a.account, kills: 40, deaths: 1, score: 590, chests: 2, keys_spent: 0, gold_spent: 0 }],
    }
}

/// The order of the respawn charge and the end messages: `99`, the used keys' inventory records (`83 45 00`) and / or the
/// `8B` money, then `83 45 0B`.
fn charge_seq(msgs: &[S2c]) -> Vec<&'static str> {
    msgs.iter()
        .filter_map(|m| match m {
            S2c::Raw { id: 0x99, .. } => Some("99"),
            S2c::Reply(Reply::Inventory(Inventory45::PveDrops { .. })) => Some("83 45 0B"),
            S2c::Reply(Reply::Inventory(Inventory45::Records { .. })) => Some("records"),
            S2c::Money(_) => Some("8B"),
            _ => None,
        })
        .collect()
}

/// Paid respawns: the world host reports what the world charged each
/// human (`keys_spent` = entry `+0x54`, `gold_spent` = `+0x58`); the lobby charges it at the result, for a won and for a
/// failed mission, before the run's `83 45 0B` (so a `5F 05` cannot spend a key the player already used): keys free
/// first (1617), then bought (1384); gold through `8B`.
#[test]
fn the_world_hosts_paid_respawns_are_charged_at_the_result() {
    game_data::require_tables!();
    for (success, keys_spent, gold_spent) in [(true, 2u16, 0u32), (false, 0, 50), (true, 1, 50)] {
        let mut w = world(true);
        let mut a = host_mission(&mut w);
        // the keys he still holds after the entry fee: one free, two bought; and 300 gold
        give(&w, &a, KEY_FREE, 1);
        give(&w, &a, KEY_PREMIUM, 2);
        set_gold(&w, &a, 300);
        let mut o = host_outcome(&a, success);
        // no cache found: no chest reward can change the gold or the keys here
        (o.players[0].keys_spent, o.players[0].gold_spent, o.players[0].chests) = (keys_spent, gold_spent, 0);
        w.fake.outcomes.lock().unwrap().push(o);
        pump_all(&mut w, &mut [&mut a]);
        let m = a.take();
        let mut want_keys = match keys_spent {
            0 => (1, 2),
            1 => (0, 2),
            _ => (0, 1),
        };
        // a won mission then gives the day's free key (record_run_notify, after the charge)
        want_keys.0 += u32::from(success);
        assert_eq!(keys(&w, &a), want_keys, "free key first (success {success}, keys {keys_spent})");
        assert_eq!(gold(&w, &a), 300 - gold_spent, "gold (success {success})");
        let seq = charge_seq(&m);
        let drops_at = seq.iter().position(|x| *x == "83 45 0B").expect("83 45 0B");
        if keys_spent > 0 {
            assert!(seq[..drops_at].contains(&"records"), "the used keys' records before 83 45 0B: {seq:?}");
        }
        if gold_spent > 0 {
            assert!(seq[..drops_at].contains(&"8B"), "8B before 83 45 0B: {seq:?}");
        }
        assert_eq!(seq[0], "99", "{seq:?}");
    }
}

/// No world host outcome (the report path or the match server's own failure end): no spends are known, nothing is
/// charged (and srp gives no budget without a host: tests in srp/tests/online.rs).
#[test]
fn without_a_host_outcome_nothing_is_charged_for_respawns() {
    game_data::require_tables!();
    let mut w = world(true);
    let mut a = up(&mut w, "alpha");
    give(&w, &a, KEY_PREMIUM, 2);
    a.send(Request::Squad(Squad49::PveState { state: 0x30, zero: 0xFF }));
    pump_all(&mut w, &mut [&mut a]);
    a.send(Request::Pve(Pve5F::Start { solo: false, map: 15, difficulty: 0 }));
    pump_all(&mut w, &mut [&mut a]);
    set_gold(&w, &a, 300);
    a.take();
    let players = vec![EndedPlayer { account: a.account, team: 0, entered: true, left_by_menu: false }];
    w.fake.ended.lock().unwrap().push(Ended { match_id: MATCH_ID, duration: PLAYED, players, stats: None, winner: Some(1) });
    pump_all(&mut w, &mut [&mut a]);
    assert!(result99(&a.take()).is_some());
    assert_eq!((keys(&w, &a), gold(&w, &a)), ((0, 1), 300), "the fee's key only, no respawn charge");
}

/// The world host ends a mission with the extraction: `99` success 1 at once with the world's mission time,
/// counters and chests (the humans' `pve_open_chest` events), `83 45 0B` with the found chests, the run stored with them
/// (daily key). A client report that disagrees is only logged. 3 s after the `99` the lobby has the match server close
/// the match with the result screen on (DF code 2: the pointer; with the match connection kept open the client stays in
/// its in-match controls); the `85 00` waits for the player's `5F 04`, nothing twice.
#[test]
fn a_mission_the_world_host_ended_sends_99_from_the_world_and_waits_for_5f_04() {
    game_data::require_tables!();
    let mut w = world(true);
    let mut a = host_mission(&mut w);
    w.fake.outcomes.lock().unwrap().push(host_outcome(&a, true));
    pump_all(&mut w, &mut [&mut a]);
    let m = a.take();
    let r = result99(&m).expect("99 from the host's outcome, no report needed");
    assert_eq!((r.match_id, r.success, r.time_secs, r.chests_found, r.chests_total, r.me), (MATCH_ID, true, 1234, 2, 10, 0));
    assert_eq!((r.players[0].score, r.players[0].kills, r.players[0].deaths), (590, 40, 1));
    assert_eq!(seq(&m), ["99", "83 45 0B"], "{m:?}");
    let (items, _, _) = drops(&m).unwrap();
    assert_eq!(items.len(), 2, "the two caches the world counted");
    assert_eq!(rows(&w, a.account, i64::from(MATCH_ID)).len(), 2);
    assert_eq!(keys(&w, &a).0, 1, "the daily key of a won mission");
    let (run, _) = w.e.store.pve_result(i64::from(MATCH_ID)).unwrap().unwrap();
    assert_eq!((run.success, run.time_secs, run.chests_found, run.players[0].score, run.players[0].kills), (true, 1234, 2, 590, 40));

    // a client report that says otherwise is a logged disagreement, nothing more
    a.send(Request::Events(vec![Event53 { id: 6, x: 0, value: MATCH_ID, time: 0 }]));
    pump_all(&mut w, &mut [&mut a]);
    assert!(result99(&a.take()).is_none());
    assert!(w.fake.ends.lock().unwrap().is_empty(), "the 99 goes first");
    // 3 s after the 99: the match server closes the match with the result screen on
    w.e.wait(&mut [&mut a], 4);
    assert_eq!(*w.fake.kept.lock().unwrap(), vec![MATCH_ID]);
    assert_eq!(*w.fake.ends.lock().unwrap(), vec![MATCH_ID]);
    let players = vec![EndedPlayer { account: a.account, team: 0, entered: true, left_by_menu: false }];
    w.fake.ended.lock().unwrap().push(Ended { match_id: MATCH_ID, duration: PLAYED, players, stats: None, winner: Some(0) });
    w.e.wait(&mut [&mut a], 10);
    assert!(seq(&a.take()).is_empty(), "no 85 00 while the result screen is open");
    // the player leaves the screen ("Return to lobby" sends 5F 04): 85 00
    a.send(Request::Pve(Pve5F::CloseStats));
    pump_all(&mut w, &mut [&mut a]);
    let m = a.take();
    assert_eq!(seq(&m), ["85 00"], "85 00, no second result: {m:?}");
    assert!(w.e.server.registry.online.is_empty());
}

/// A failure the world host reports (every human dead past the resurrection window): `99` with success 0 (CHOICE),
/// `83 45 0B` without chests, no daily key; the match closes 3 s later with the result screen on;
/// without a `5F 04` the `85 00` comes at PVE_RESULT_SCREEN_CAP (120 s after the `99`). The match server's hand-over of
/// its own failure end (winner 1, no counters) takes this path too.
#[test]
fn a_failure_from_the_world_host_shows_99_failure_until_the_120_s_cap() {
    game_data::require_tables!();
    let mut w = world(true);
    let mut a = host_mission(&mut w);
    w.fake.outcomes.lock().unwrap().push(host_outcome(&a, false));
    pump_all(&mut w, &mut [&mut a]);
    let m = a.take();
    let r = result99(&m).expect("99");
    assert_eq!((r.success, r.time_secs, r.chests_found), (false, 1234, 2), "the world's numbers on the failure screen");
    assert!(drops(&m).unwrap().0.is_empty(), "no chest for a failure");
    assert!(rows(&w, a.account, i64::from(MATCH_ID)).is_empty());
    assert_eq!(keys(&w, &a), (0, 0));
    w.e.wait(&mut [&mut a], 4);
    assert_eq!(*w.fake.kept.lock().unwrap(), vec![MATCH_ID], "closed 3 s after the 99, the result screen on");
    let players = vec![EndedPlayer { account: a.account, team: 0, entered: true, left_by_menu: false }];
    w.fake.ended.lock().unwrap().push(Ended { match_id: MATCH_ID, duration: PLAYED, players, stats: None, winner: Some(1) });
    w.e.wait(&mut [&mut a], 110);
    assert!(seq(&a.take()).is_empty(), "the 85 00 waits for the screen up to 120 s");
    w.e.wait(&mut [&mut a], 7);
    assert_eq!(seq(&a.take()), ["85 00"]);
    assert_eq!(w.fake.ends.lock().unwrap().len(), 1);
}

/// Premium ammo boxes in a PvE mission: the world applies the slot-11 box there too, so a
/// failed mission also takes one box per spawned profile from the world host's Used, at the match end (`83 45 03`).
#[test]
fn a_failed_world_host_mission_takes_the_used_ammo_box() {
    game_data::require_tables!();
    use lobby_server::charge::{MatchUse, StackUse};
    use lobby_server::EndedStat;
    let mut w = world(true);
    let mut a = host_mission(&mut w);
    let uid = w.e.store.update_profile(a.account, |p| {
        let uid = p.add_item(1281, 2);
        p.slots.insert(11, uid);
        p.slot_rounds.insert(11, 1);
        Ok::<_, ()>(uid)
    }).unwrap();
    w.fake.outcomes.lock().unwrap().push(host_outcome(&a, false));
    pump_all(&mut w, &mut [&mut a]);
    assert!(result99(&a.take()).is_some());
    w.e.wait(&mut [&mut a], 4);
    let used = MatchUse { profiles: 1, weapons: vec![1201], stacks: vec![StackUse { uid, dict: 1281, used: 1 }] };
    let stats = vec![EndedStat { account: a.account, kills: 40, deaths: 1, score: 590, used: Some(used), ..Default::default() }];
    let players = vec![EndedPlayer { account: a.account, team: 0, entered: true, left_by_menu: false }];
    w.fake.ended.lock().unwrap().push(Ended { match_id: MATCH_ID, duration: PLAYED, players, stats: Some((stats, [0, 0])), winner: Some(1) });
    a.send(Request::Pve(Pve5F::CloseStats));
    pump_all(&mut w, &mut [&mut a]);
    w.e.wait(&mut [&mut a], 2);
    let m = a.take();
    let sent: Vec<u32> = m.iter().filter_map(|x| if let S2c::Reply(Reply::Inventory(Inventory45::Used { item })) = x { Some(*item) } else { None }).collect();
    assert_eq!(sent, vec![uid], "one 83 45 03 for the box");
    let p = w.e.store.load_profile(a.account).unwrap();
    assert_eq!((p.item(uid).unwrap().count, p.slots.get(&11)), (1, Some(&uid)));
    assert!(w.e.server.registry.online.is_empty());
}

/// The match server's hand-over of its own failure end (pre-start cancel or 45-minute end, match_server
/// `MatchConfig::pve_handover`): an outcome with winner 1 and no player rows. `99` success 0 with the mission time and
/// zero counters, `83 45 0B` without chests, the close with the result screen on 3 s later.
#[test]
fn the_match_servers_handed_over_failure_shows_the_failure_screen() {
    game_data::require_tables!();
    let mut w = world(true);
    let mut a = host_mission(&mut w);
    w.fake.host.store(false, std::sync::atomic::Ordering::SeqCst);
    w.fake.outcomes.lock().unwrap().push(HostOutcome { match_id: MATCH_ID, state: 4, winner_team: 1, success: false, mission_ms: 420_000, players: Vec::new() });
    pump_all(&mut w, &mut [&mut a]);
    let m = a.take();
    let r = result99(&m).expect("99");
    assert_eq!((r.success, r.time_secs, r.chests_found), (false, 420, 0));
    assert_eq!((r.players[0].score, r.players[0].kills, r.players[0].deaths), (0, 0, 0), "no counters invented");
    assert_eq!(seq(&m), ["99", "83 45 0B"]);
    w.e.wait(&mut [&mut a], 4);
    assert_eq!(*w.fake.kept.lock().unwrap(), vec![MATCH_ID]);
    let players = vec![EndedPlayer { account: a.account, team: 0, entered: true, left_by_menu: false }];
    w.fake.ended.lock().unwrap().push(Ended { match_id: MATCH_ID, duration: PLAYED, players, stats: None, winner: Some(1) });
    a.send(Request::Pve(Pve5F::CloseStats));
    pump_all(&mut w, &mut [&mut a]);
    assert_eq!(seq(&a.take()), ["85 00"]);
    assert_eq!(keys(&w, &a), (0, 0), "no daily key for a failure");
}

/// With a world host in use, the clients' reports wait for its outcome (they come first: the host reads a final tick);
/// the outcome then makes the result. When no outcome comes within HOST_OUTCOME_WAIT, the reports make it (the report
/// path: the client's time, no counters, the 2 s close).
#[test]
fn reports_wait_for_the_world_host_then_fall_back() {
    game_data::require_tables!();
    let mut w = world(true);
    let mut a = host_mission(&mut w);
    a.send(Request::Events(vec![Event53 { id: 5, x: 0, value: MATCH_ID, time: 0 }]));
    pump_all(&mut w, &mut [&mut a]);
    w.e.wait(&mut [&mut a], 3);
    assert!(result99(&a.take()).is_none(), "the report waits for the host");
    w.fake.outcomes.lock().unwrap().push(host_outcome(&a, true));
    pump_all(&mut w, &mut [&mut a]);
    let r = result99(&a.take()).expect("99 from the host");
    assert_eq!((r.success, r.time_secs, r.chests_found), (true, 1234, 2));

    let mut w = world(true);
    let mut a = host_mission(&mut w);
    a.send(Request::Events(vec![Event53 { id: 5, x: 0, value: MATCH_ID, time: 0 }]));
    pump_all(&mut w, &mut [&mut a]);
    w.e.wait(&mut [&mut a], 8);
    assert!(result99(&a.take()).is_none());
    w.e.wait(&mut [&mut a], 3);
    let r = result99(&a.take()).expect("the reports after HOST_OUTCOME_WAIT");
    assert_eq!((r.success, r.time_secs, r.chests_found), (true, PLAYED.as_secs() as u32, 0));
    w.e.wait(&mut [&mut a], 4);
    assert_eq!(*w.fake.kept.lock().unwrap(), vec![MATCH_ID], "the reports' 3 s close, the result screen on");
    // a late host outcome only cross-checks
    w.fake.outcomes.lock().unwrap().push(host_outcome(&a, true));
    pump_all(&mut w, &mut [&mut a]);
    assert!(result99(&a.take()).is_none());
}

/// The menu leave of an online PvE mission (seen with the real client): the search started as a running search (`85 04`
/// second word not 0), and the player's one-man PvE squad (his seat locks) ends with the match, so he gets `AE` with squad
/// id 0 and `85 00`. With the squad kept, the client's PvE preview has no Cancel (`!PlayerInfo.isInSquad`) and hides the
/// Play / mode panel until a re-login.
/// The match end that follows sends him nothing again.
#[test]
fn a_menu_leave_of_a_pve_mission_ends_the_one_man_pve_squad() {
    game_data::require_tables!();
    let mut w = world(true);
    let mut a = up(&mut w, "alpha");
    give(&w, &a, KEY_PREMIUM, 1);
    a.send(Request::Squad(Squad49::PveState { state: 0x30, zero: 0xFF }));
    pump_all(&mut w, &mut [&mut a]);
    assert!(w.e.server.registry.squads.solo_pve(a.account), "the seat locks made a one-man PvE squad");
    a.send(Request::Pve(Pve5F::Start { solo: false, map: 15, difficulty: 0 }));
    pump_all(&mut w, &mut [&mut a]);
    let m = a.take();
    assert!(connect(&m).is_some());
    // 85 04 with a running search (second word not 0): the client turns its game search on, so the 85 00 at the end turns
    // it off again and gives the mode window back (word 0 = "match found" skips that and the lobby is stuck)
    let search = m.iter().find_map(|x| match x {
        S2c::Raw { id: 0x85, payload } if payload.first() == Some(&4) => Some(u32::from_le_bytes(payload[5..9].try_into().unwrap())),
        _ => None,
    });
    assert_eq!(search, Some(PveSearching::SEARCH_RUNNING), "{m:?}");

    w.fake.left.lock().unwrap().push(Left { match_id: MATCH_ID, account: a.account, by_menu: true });
    w.e.wait(&mut [&mut a], 2);
    let m = a.take();
    assert_eq!(last_squad_msg(&m), Some(None), "AE with squad id 0: {m:?}");
    assert_eq!(idle_count(&m), 1, "85 00: {m:?}");
    assert!(w.e.server.registry.squads.squad_of(a.account).is_none(), "the one-man PvE squad is gone");

    let players = vec![EndedPlayer { account: a.account, team: 0, entered: true, left_by_menu: true }];
    w.fake.ended.lock().unwrap().push(Ended { match_id: MATCH_ID, duration: PLAYED, players, stats: None, winner: Some(1) });
    w.e.wait(&mut [&mut a], 2);
    let m = a.take();
    assert_eq!(idle_count(&m), 0, "released once: {m:?}");
    assert_eq!(last_squad_msg(&m), None, "no second AE: {m:?}");
    assert!(w.e.server.registry.online.is_empty());
}

/// The AE of the last squad message (None = squad id 0), unlike `last_ae`, which skips AEs with no squad.
fn last_squad_msg(msgs: &[S2c]) -> Option<Option<Squad>> {
    msgs.iter().rev().find_map(|m| if let S2c::Squad(s) = m { Some(s.squad.clone()) } else { None })
}

/// A player in no squad locks seats in the PvE window: a one-man PvE squad carries the locks in his record (the client
/// draws locks only from it); with both locked the mission wants him alone; clearing the locks dissolves the squad.
#[test]
fn seat_locks_without_a_squad_make_a_one_man_pve_squad() {
    game_data::require_tables!();
    let mut w = world(true);
    let mut a = up(&mut w, "alpha");
    give(&w, &a, KEY_PREMIUM, 1);
    a.take();
    a.send(Request::Squad(Squad49::PveState { state: 0x10, zero: 0xFF }));
    pump_all(&mut w, &mut [&mut a]);
    let s = last_squad_msg(&a.take()).expect("an AE").expect("a squad");
    assert_ne!(s.id, 0);
    assert_eq!((s.kind, s.i_am_commander, s.members.len()), (SQUAD_TYPE_PVE, 1, 1));
    assert_eq!(s.members[0].state, 0x10);
    // the second seat too
    a.send(Request::Squad(Squad49::PveState { state: 0x30, zero: 0xFF }));
    pump_all(&mut w, &mut [&mut a]);
    assert_eq!(last_squad_msg(&a.take()).unwrap().unwrap().members[0].state, 0x30);
    // both unlocked: the squad is gone (AE squad id 0)
    a.send(Request::Squad(Squad49::PveState { state: 0, zero: 0xFF }));
    pump_all(&mut w, &mut [&mut a]);
    assert_eq!(last_squad_msg(&a.take()), Some(None));
    // locked again, then Play: 1 player + 2 locks = the mission's 3, started for him alone
    a.send(Request::Squad(Squad49::PveState { state: 0x30, zero: 0xFF }));
    pump_all(&mut w, &mut [&mut a]);
    a.take();
    let gold_a = gold(&w, &a);
    a.send(Request::Pve(Pve5F::Start { solo: false, map: 15, difficulty: 0 }));
    pump_all(&mut w, &mut [&mut a]);
    let m = a.take();
    assert_eq!(denial(&m), None, "{m:?}");
    let created = w.fake.pve.lock().unwrap().clone();
    assert_eq!(created.len(), 1);
    assert_eq!(created[0].0.human_players, 1);
    assert_eq!(keys(&w, &a), (0, 0), "his key");
    assert_eq!(gold(&w, &a), gold_a - 100, "two locked seats at the block-slot price");
}

/// The sign-in answer (80) carries the player's own public id, the key the client compares with the squad records'
/// account ids (`is_local_player`): his own record in the one-man squad is "me", not a second player (seen with the
/// real client).
#[test]
fn sign_in_carries_the_own_id_of_the_squad_record() {
    game_data::require_tables!();
    let mut w = world(true);
    let mut a = w.e.client("alpha");
    let msgs = w.e.sign_in(&mut a);
    let s = msgs.iter().find_map(|m| if let S2c::SignedIn(s) = m { Some(*s) } else { None }).expect("80");
    assert_eq!(s.account_id, public_id(a.account));
    a.send(Request::Squad(Squad49::PveState { state: 0x30, zero: 0xFF }));
    pump_all(&mut w, &mut [&mut a]);
    let sq = last_squad_msg(&a.take()).unwrap().unwrap();
    assert_eq!(sq.members[0].account_id, s.account_id);
}

/// With online matches off, the one-man squad's Play is refused with the no-match-server code (the client's "All game
/// servers are busy"), nothing is charged and the squad stays.
#[test]
fn a_one_man_squad_play_without_online_matches_is_refused_cleanly() {
    game_data::require_tables!();
    let mut w = world(false);
    let mut a = up(&mut w, "alpha");
    give(&w, &a, KEY_PREMIUM, 1);
    a.send(Request::Squad(Squad49::PveState { state: 0x30, zero: 0xFF }));
    pump_all(&mut w, &mut [&mut a]);
    a.take();
    let gold_a = gold(&w, &a);
    a.send(Request::Pve(Pve5F::Start { solo: false, map: 15, difficulty: 0 }));
    pump_all(&mut w, &mut [&mut a]);
    assert_eq!(denial(&a.take()), Some(deny::NO_MATCH_SERVER));
    assert_eq!((keys(&w, &a), gold(&w, &a)), ((0, 1), gold_a));
}

/// A PvP Play (2F) from a one-man PvE squad leaves it (AE squad id 0) and is answered as for a single player.
#[test]
fn pvp_play_leaves_a_one_man_pve_squad() {
    game_data::require_tables!();
    let mut w = world(false);
    let mut a = up(&mut w, "alpha");
    a.send(Request::Squad(Squad49::PveState { state: 0x10, zero: 0xFF }));
    pump_all(&mut w, &mut [&mut a]);
    assert!(last_squad_msg(&a.take()).unwrap().is_some());
    a.send(Request::Queue { global: u32::MAX, arg: 0, mode: 0x380, flag: 0 });
    pump_all(&mut w, &mut [&mut a]);
    let m = a.take();
    assert_eq!(last_squad_msg(&m), Some(None), "{m:?}");
    // the single player's queue answer: 83 2F and the 85 queue state
    assert!(m.iter().any(|x| matches!(x, S2c::Reply(Reply::Queue { .. }))), "the queue answer: {m:?}");
    assert!(m.iter().any(|x| x.id() == 0x85), "85: {m:?}");
}

#[test]
fn an_open_squad_waits_and_leaving_ends_its_search() {
    game_data::require_tables!();
    let mut w = world(true);
    let (mut a, mut b) = (up(&mut w, "alpha"), up(&mut w, "bravo"));
    give(&w, &a, KEY_FREE, 1);
    give(&w, &b, KEY_FREE, 1);
    pve_join(&mut w, &mut a, &mut b, &mut []);
    b.send(Request::Squad(Squad49::PveState { state: 1, zero: 0 }));
    pump_all(&mut w, &mut [&mut a, &mut b]);
    a.take();
    b.take();
    a.send(Request::Pve(Pve5F::Start { solo: false, map: 15, difficulty: 1 }));
    pump_all(&mut w, &mut [&mut a, &mut b]);
    let (ma, mb) = (a.take(), b.take());
    assert!(searching(&ma).is_some() && searching(&mb).is_some(), "both members see the search");
    assert!(w.fake.pve.lock().unwrap().is_empty());
    assert_eq!(w.e.server.registry.pve.len(), 1);
    // bravo leaves the squad (49 04): the search ends for the commander, who is alone now
    b.send(Request::Squad(Squad49::Leave));
    pump_all(&mut w, &mut [&mut a, &mut b]);
    assert!(w.e.server.registry.pve.is_empty());
    assert_eq!(idle_count(&a.take()), 1);
    assert_eq!(keys(&w, &a), (1, 0));

    // a squad member can cancel the squad's search with 31 as well
    pve_join(&mut w, &mut a, &mut b, &mut []);
    b.send(Request::Squad(Squad49::PveState { state: 1, zero: 0 }));
    pump_all(&mut w, &mut [&mut a, &mut b]);
    a.send(Request::Pve(Pve5F::Start { solo: false, map: 15, difficulty: 1 }));
    pump_all(&mut w, &mut [&mut a, &mut b]);
    let order = searching(&b.take()).unwrap();
    b.send(Request::QueueCancel { order });
    pump_all(&mut w, &mut [&mut a, &mut b]);
    assert!(w.e.server.registry.pve.is_empty());
    assert!(idle_count(&a.take()) >= 1);
}

#[test]
fn no_key_and_no_gold_is_refused_with_money_code_and_nothing_queued() {
    game_data::require_tables!();
    let mut w = world(true);
    let mut a = up(&mut w, "alpha");
    set_gold(&w, &a, 49);
    play_solo(&mut w, &mut a, 0, &mut []);
    let m = a.take();
    assert_eq!(denial(&m), Some(deny::NOT_ENOUGH_MONEY));
    assert_eq!(idle_count(&m), 1);
    assert!(w.e.server.registry.pve.is_empty());
    assert_eq!(gold(&w, &a), 49);
    // a key makes the same press work
    give(&w, &a, KEY_PREMIUM, 1);
    play_solo(&mut w, &mut a, 0, &mut []);
    let m = a.take();
    assert_eq!(denial(&m), None);
    assert!(searching(&m).is_some());
    // a bad difficulty is the generic refusal
    let mut b = up(&mut w, "bravo");
    b.send(Request::Pve(Pve5F::Start { solo: true, map: 15, difficulty: 3 }));
    pump(&mut w, &mut b, &mut []);
    assert_eq!(denial(&b.take()), Some(deny::GENERIC));
}

#[test]
fn with_online_matches_off_the_queue_refuses_cleanly_and_charges_nothing() {
    game_data::require_tables!();
    let mut w = world(false);
    let mut a = up(&mut w, "alpha");
    give(&w, &a, KEY_FREE, 2);
    let before = w.e.store.load_profile(a.account).unwrap();
    play_solo(&mut w, &mut a, 0, &mut []);
    let m = a.take();
    assert_eq!(denial(&m), Some(deny::NO_MATCH_SERVER), "st_not_enough_match_servers");
    assert_eq!(idle_count(&m), 1, "a proper 85 00, Play is usable again");
    assert!(searching(&m).is_none());
    assert!(w.e.server.registry.pve.is_empty());
    assert_eq!(w.e.store.load_profile(a.account).unwrap(), before, "no charge");
    // a squad Play is refused the same way
    let mut b = up(&mut w, "bravo");
    pve_join(&mut w, &mut a, &mut b, &mut []);
    b.send(Request::Squad(Squad49::PveState { state: 1, zero: 0 }));
    pump_all(&mut w, &mut [&mut a, &mut b]);
    a.take();
    a.send(Request::Pve(Pve5F::Start { solo: false, map: 15, difficulty: 0 }));
    pump_all(&mut w, &mut [&mut a, &mut b]);
    assert_eq!(denial(&a.take()), Some(deny::NO_MATCH_SERVER));
    assert_eq!(keys(&w, &a), (2, 0));
    // the top 100 and the closing of the stats screen need no match server
    a.send(Request::Pve(Pve5F::Top100));
    a.send(Request::Pve(Pve5F::CloseStats));
    pump(&mut w, &mut a, &mut []);
    assert!(a.take().iter().any(|m| matches!(m, S2c::Raw { id: 0xC2, .. })));
}

#[test]
fn a_match_server_that_cannot_create_the_match_gives_everything_back() {
    game_data::require_tables!();
    let mut w = world(true);
    let (mut a, mut b, mut c) = (up(&mut w, "alpha"), up(&mut w, "bravo"), up(&mut w, "charlie"));
    give(&w, &a, KEY_FREE, 1);
    give(&w, &b, KEY_PREMIUM, 1);
    let (gold_b, gold_c) = (gold(&w, &b), gold(&w, &c));
    *w.fake.fail.lock().unwrap() = Some("no world".into());
    play_solo(&mut w, &mut a, 0, &mut [&mut b, &mut c]);
    play_solo(&mut w, &mut b, 0, &mut [&mut a, &mut c]);
    a.take();
    b.take();
    play_solo(&mut w, &mut c, 0, &mut [&mut a, &mut b]);
    for x in [&mut a, &mut b, &mut c] {
        let m = x.take();
        assert_eq!(denial(&m), Some(deny::NO_MATCH_SERVER));
        assert_eq!(idle_count(&m), 1);
        assert!(connect(&m).is_none());
    }
    assert!(w.e.server.registry.pve.is_empty());
    assert_eq!(keys(&w, &a), (1, 0));
    assert_eq!(keys(&w, &b), (0, 1));
    assert_eq!((gold(&w, &b), gold(&w, &c)), (gold_b, gold_c));
    // the server recovers: the same three can search again
    *w.fake.fail.lock().unwrap() = None;
    play_solo(&mut w, &mut a, 0, &mut [&mut b, &mut c]);
    play_solo(&mut w, &mut b, 0, &mut [&mut a, &mut c]);
    play_solo(&mut w, &mut c, 0, &mut [&mut a, &mut b]);
    assert_eq!(w.fake.pve.lock().unwrap().len(), 1);
}

fn run(match_id: u32, players: &[accounts::AccountId], success: bool, chests: u8, time: u32) -> PveRun {
    PveRun {
        match_id,
        mission: 15,
        difficulty: 0,
        success,
        time_secs: time,
        chests_found: chests,
        chests_total: 10,
        started_at: 1000,
        players: players.iter().enumerate().map(|(i, a)| PveRunPlayer { account: *a, score: 100 + i as u16, kills: 1, deaths: 0, sort: i as u8, premium: false }).collect(),
    }
}

fn top(msgs: &[S2c]) -> PveTop100 {
    msgs.iter()
        .find_map(|m| match m {
            S2c::Raw { id: 0xC2, payload } if payload.first() == Some(&3) => Some(PveTop100::from_push(payload).unwrap()),
            _ => None,
        })
        .expect("C2 03")
}

#[test]
fn top_100_comes_from_the_stored_runs() {
    game_data::require_tables!();
    let mut w = world(false);
    let (mut a, mut b, mut c, mut d) = (up(&mut w, "alpha"), up(&mut w, "bravo"), up(&mut w, "charlie"), up(&mut w, "delta"));
    a.send(Request::Pve(Pve5F::Top100));
    pump(&mut w, &mut a, &mut []);
    assert_eq!(top(&a.take()), PveTop100::default(), "nobody passed the mission yet");

    let gd = game_data::testing::tables();
    let now = 20_000 * 86_400;
    // alpha + bravo: two wins (the slower one with the same chests does not count); charlie alone; delta only failed
    let given = lobby_server::pve::record_run(&w.e.store, &gd, &run(1, &[a.account, b.account], true, 7, 1200), now).unwrap();
    assert_eq!(given, vec![a.account, b.account], "the first wins of the day give a free key");
    lobby_server::pve::record_run(&w.e.store, &gd, &run(2, &[b.account, a.account], true, 7, 1000), now).unwrap();
    lobby_server::pve::record_run(&w.e.store, &gd, &run(3, &[c.account], true, 3, 500), now).unwrap();
    let given = lobby_server::pve::record_run(&w.e.store, &gd, &run(4, &[d.account], false, 10, 5), now).unwrap();
    assert!(given.is_empty());
    assert_eq!(keys(&w, &a).0, 2, "two free keys for two wins");
    // a third win of the day gives none (the same squad, slower: its ladder entry stays the 1000 s run)
    assert!(lobby_server::pve::record_run(&w.e.store, &gd, &run(2, &[a.account, b.account], true, 7, 1300), now).unwrap().is_empty());
    assert_eq!(keys(&w, &a).0, 2);

    for x in [&mut a, &mut b, &mut c, &mut d] {
        x.send(Request::Pve(Pve5F::Top100));
    }
    pump_all(&mut w, &mut [&mut a, &mut b, &mut c, &mut d]);
    let (ta, tb, tc, td) = (top(&a.take()), top(&b.take()), top(&c.take()), top(&d.take()));
    // more chests first, then the shorter time: alpha + bravo (7 chests, 1000 s), then charlie
    assert_eq!(ta.entries.len(), 2);
    let e0 = &ta.entries[0];
    assert_eq!((e0.play_time, e0.players.iter().map(|p| p.name.as_str()).collect::<Vec<_>>()), (1000, vec!["bravo", "alpha"]));
    // the byte under the list's "chests_num" column is the run's chests found, not the run score (100 / 101 here; the
    // real client showed 255 for a score of 590 and 5 chests)
    assert_eq!(e0.players.iter().map(|p| p.score).collect::<Vec<_>>(), vec![7, 7]);
    assert_eq!(ta.entries[1].players[0].name, "charlie");
    assert_eq!(ta.entries[1].players[0].score, 3);
    assert_eq!(ta.entries[1].play_time, 500);
    assert_eq!(ta.own.as_ref().map(|o| o.0), Some(0));
    assert_eq!(tb.own.as_ref().map(|o| o.0), Some(0));
    assert_eq!(tc.own.as_ref().map(|o| o.0), Some(1));
    assert_eq!(tc.own.as_ref().unwrap().1.play_time, 500);
    assert_eq!(td.own, None, "a failed run is no entry");
    assert_eq!(td.entries, ta.entries);
}

/// One busy rule (busy.rs): a PvE searcher cannot create a custom room, and a room member can neither search for PvE nor
/// for PvP.
#[test]
fn rooms_and_searches_exclude_each_other() {
    game_data::require_tables!();
    let mut w = world(true);
    let mut a = up(&mut w, "alice");
    let mut b = up(&mut w, "bob");
    // alice searches PvE (the third seat is missing, so she keeps waiting)
    play_solo(&mut w, &mut a, 0, &mut [&mut b]);
    assert!(searching(&a.take()).is_some());
    a.send(Request::Other { id: 0x62, payload: Custom62::Create { kind: 0 }.encode() });
    pump(&mut w, &mut a, &mut [&mut b]);
    let texts: Vec<String> = a
        .take()
        .iter()
        .filter_map(|x| match x {
            S2c::Denied(Denied { body: DeniedBody::Text(t), .. }) => Some(t.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(texts, ["st_custom_match_error_player_already_in_match"]);
    // bob is in a room: PvE search and PvP queue are refused
    b.send(Request::Other { id: 0x62, payload: Custom62::Create { kind: 0 }.encode() });
    pump(&mut w, &mut b, &mut [&mut a]);
    b.take();
    play_solo(&mut w, &mut b, 0, &mut [&mut a]);
    let m = b.take();
    assert_eq!(denial(&m), Some(deny::GENERIC));
    assert!(searching(&m).is_none());
    b.send(Request::Queue { global: 0, arg: 0, mode: 0x380, flag: 0 });
    pump(&mut w, &mut b, &mut [&mut a]);
    let m = b.take();
    assert!(m.contains(&S2c::QueueState(QueueState { state: 0 })), "{m:?}");
    assert!(!m.iter().any(|x| matches!(x, S2c::Reply(Reply::Queue { .. }))), "{m:?}");
}

// ---- the end screen (docs/notes/pve.md "End screen"): chests stored at the mission end, `5F 05` unlock, `5F 06` reroll

const DAY: i64 = 20_000;

fn end_of(w: &mut World, run: &PveRun) -> Vec<lobby_server::Cross> {
    end_on(w, run, DAY)
}

fn end_on(w: &mut World, run: &PveRun, day: i64) -> Vec<lobby_server::Cross> {
    let now = w.e.now;
    let srv = &mut w.e.server;
    let mut cx = lobby_server::dispatch::Ctx { lobby: &srv.lobby, registry: &mut srv.registry, now, fx: vec![], kick: vec![] };
    lobby_server::pve::record_run_notify(&mut cx, run, day * 86_400 + 3600).unwrap();
    cx.fx
}

/// The players already won a mission on [`DAY`], so the next won run is not their first of the day (that one opens every
/// chest, see `the_first_won_mission_of_the_day_opens_every_chest`).
fn first_win_used(w: &World, accounts: &[accounts::AccountId]) {
    for &a in accounts {
        w.e.store.pve_note_win(a, DAY).unwrap();
    }
}

fn drops(msgs: &[S2c]) -> Option<(Vec<protocol::items::ItemRecord>, u32, bool)> {
    msgs.iter().rev().find_map(|m| match m {
        S2c::Reply(Reply::Inventory(Inventory45::PveDrops { items, cost, flag })) => Some((items.clone(), *cost, *flag)),
        _ => None,
    })
}

fn fx_of(fx: &[lobby_server::Cross], a: accounts::AccountId) -> Vec<S2c> {
    fx.iter().filter(|c| c.to == a).map(|c| c.msg.clone()).collect()
}

/// The chest rows of a run, by its run key (the first run of a wire id: the wire id itself).
fn rows(w: &World, a: accounts::AccountId, run: i64) -> Vec<accounts::PveRewardRow> {
    w.e.store.pve_rewards(a, run).unwrap()
}

fn gold_in(rows: &[accounts::PveRewardRow]) -> u32 {
    rows.iter().filter(|r| r.opened && r.reward_dict == 215).map(|r| r.reward_amount).sum()
}

#[test]
fn a_won_run_opens_three_chests_and_keeps_the_rest_locked() {
    game_data::require_tables!();
    let mut w = world(false);
    let a = up(&mut w, "alpha");
    first_win_used(&w, &[a.account]);
    let before = w.e.store.load_profile(a.account).unwrap();
    let fx = end_of(&mut w, &run(0x51, &[a.account], true, 5, 900));
    let r = rows(&w, a.account, 0x51);
    assert_eq!(r.len(), 5);
    assert_eq!(r.iter().map(|x| x.opened).collect::<Vec<_>>(), [true, true, true, false, false], "free_chests_num 3");
    assert!(r.iter().all(|x| x.chest_dict == 1483 && x.tier == 1 && x.difficulty == 0 && x.rerolls == 0 && !x.paid));
    assert!(r[..3].iter().all(|x| x.reward_dict != 0 && x.reward_amount > 0) && r[3..].iter().all(|x| x.reward_dict == 0));
    let msgs = fx_of(&fx, a.account);
    let (items, cost, flag) = drops(&msgs).expect("83 45 0B");
    assert_eq!((items.len(), cost, flag), (3, 50, true), "three rewards, first reroll 50 gold, chests still locked");
    assert_eq!(items.iter().map(|i| i.dict).collect::<Vec<_>>(), r[..3].iter().map(|x| x.reward_dict).collect::<Vec<_>>());
    assert!(matches!(msgs.last(), Some(S2c::Reply(Reply::Inventory(Inventory45::PveDrops { .. })))), "the screen message comes last");
    // the three rewards are in the profile
    assert_ne!(w.e.store.load_profile(a.account).unwrap(), before);
    // the same wire match id again (a PvE match id is the maze's captured world id): a new run under its own key with
    // its own chests and its own 83 45 0B; the first run's rows stay as they were
    let fx = end_of(&mut w, &run(0x51, &[a.account], true, 4, 100));
    assert_eq!(rows(&w, a.account, 0x51), r);
    let r2 = rows(&w, a.account, accounts::pve_run_key(0x51, 1));
    assert_eq!(r2.len(), 4);
    assert_eq!(drops(&fx_of(&fx, a.account)).map(|d| d.0.len()), Some(3));
    assert_eq!(w.e.store.pve_latest_run(a.account).unwrap(), Some(accounts::pve_run_key(0x51, 1)));
}

/// A match id seen twice (stored runs keep their own keys even when a wire id repeats; the match server gives every
/// match its own id). A won mission, then a failed one with the same id: both are stored (two runs), both get `99` and `83 45 0B` (the
/// client's end screen needs the drops message), and `5F 05` after the second acts on the second run. A match server
/// that returns an id the lobby still holds (alpha's result screen) is refused (it would take over the lobby's entry of
/// the old one) and the key is given back.
#[test]
fn two_missions_with_the_same_match_id_are_two_runs_with_their_own_end_screens() {
    game_data::require_tables!();
    let mut w = world(true);
    let mut a = host_mission(&mut w);
    // not his first win of the day: 3 of the 5 chests open free, 2 stay locked
    let today = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64 / 86_400;
    w.e.store.pve_note_win(a.account, today).unwrap();
    let mut o = host_outcome(&a, true);
    o.players[0].chests = 5;
    w.fake.outcomes.lock().unwrap().push(o);
    pump_all(&mut w, &mut [&mut a]);
    let m = a.take();
    assert_eq!(seq(&m), ["99", "83 45 0B"], "{m:?}");
    let first = i64::from(MATCH_ID);
    assert_eq!(rows(&w, a.account, first).iter().filter(|r| !r.opened).count(), 2);

    // the (fake) match server hands bravo alpha's id while alpha's result screen is still up: refused, the key comes back
    let mut b = up(&mut w, "bravo");
    give(&w, &b, KEY_PREMIUM, 1);
    b.send(Request::Squad(Squad49::PveState { state: 0x30, zero: 0xFF }));
    pump_all(&mut w, &mut [&mut a, &mut b]);
    b.take();
    b.send(Request::Pve(Pve5F::Start { solo: false, map: 15, difficulty: 0 }));
    pump_all(&mut w, &mut [&mut a, &mut b]);
    let mb = b.take();
    assert!(connect(&mb).is_none(), "{mb:?}");
    assert_eq!(denial(&mb), Some(deny::NO_MATCH_SERVER));
    assert_eq!(keys(&w, &b), (0, 1), "the key is given back");
    assert_eq!(w.fake.pve.lock().unwrap().len(), 2, "the match server was asked; its answer (an id the lobby holds) was refused");

    // alpha's match ends and he leaves the screen: the world is free again
    w.e.wait(&mut [&mut a, &mut b], 4);
    let players = vec![EndedPlayer { account: a.account, team: 0, entered: true, left_by_menu: false }];
    w.fake.ended.lock().unwrap().push(Ended { match_id: MATCH_ID, duration: PLAYED, players, stats: None, winner: None });
    w.e.wait(&mut [&mut a], 8);
    a.send(Request::Pve(Pve5F::CloseStats));
    pump_all(&mut w, &mut [&mut a]);
    assert_eq!(idle_count(&a.take()), 1);
    assert!(w.e.server.registry.online.is_empty());

    // the second mission of the same world (same match id), failed by the script: 99 success 0 and 83 45 0B again
    w.fake.host.store(false, std::sync::atomic::Ordering::SeqCst);
    give(&w, &a, KEY_PREMIUM, 1);
    a.send(Request::Squad(Squad49::PveState { state: 0x30, zero: 0xFF }));
    pump_all(&mut w, &mut [&mut a]);
    a.send(Request::Pve(Pve5F::Start { solo: false, map: 15, difficulty: 0 }));
    pump_all(&mut w, &mut [&mut a]);
    assert!(connect(&a.take()).is_some());
    let players = vec![EndedPlayer { account: a.account, team: 0, entered: true, left_by_menu: false }];
    w.fake.ended.lock().unwrap().push(Ended { match_id: MATCH_ID, duration: PLAYED, players, stats: None, winner: Some(1) });
    pump_all(&mut w, &mut [&mut a]);
    w.e.wait(&mut [&mut a], 2);
    let m = a.take();
    assert_eq!(seq(&m), ["99", "83 45 0B", "85 00"], "{m:?}");
    let r = result99(&m).unwrap();
    assert_eq!((r.match_id, r.success), (MATCH_ID, false), "the 99 keeps the wire id");
    assert_eq!(w.e.store.pve_result_ids_of(a.account).unwrap().len(), 2, "two runs stored");
    let second = accounts::pve_run_key(MATCH_ID, 1);
    let (run2, _) = w.e.store.pve_result(second).unwrap().unwrap();
    assert_eq!((run2.match_id, run2.success), (MATCH_ID, false));
    assert_eq!(w.e.store.pve_latest_run(a.account).unwrap(), Some(second));
    // 5F 05 now acts on the failed run (no chests): refused; the first run's 2 locked chests stay locked, no gold taken
    set_gold(&w, &a, 500);
    let before = rows(&w, a.account, first);
    a.send(Request::Pve(Pve5F::UnlockRewards));
    pump_all(&mut w, &mut [&mut a]);
    assert!(a.take().is_empty());
    assert_eq!(rows(&w, a.account, first), before);
    assert_eq!(gold(&w, &a), 500);
}

#[test]
fn a_lost_run_and_an_inactive_player_get_an_empty_screen_and_no_chests() {
    game_data::require_tables!();
    let mut w = world(false);
    let (a, b) = (up(&mut w, "alpha"), up(&mut w, "bravo"));
    let fx = end_of(&mut w, &run(0x52, &[a.account], false, 6, 900));
    assert_eq!(drops(&fx_of(&fx, a.account)), Some((vec![], 0, false)));
    assert!(rows(&w, a.account, 0x52).is_empty());
    // a won run: bravo scored 0 (afk) and gets none
    first_win_used(&w, &[a.account, b.account]);
    let mut r = run(0x53, &[a.account, b.account], true, 4, 900);
    r.players[1].score = 0;
    let fx = end_of(&mut w, &r);
    assert_eq!(rows(&w, a.account, 0x53).len(), 4);
    assert!(rows(&w, b.account, 0x53).is_empty());
    assert_eq!(drops(&fx_of(&fx, b.account)), Some((vec![], 0, false)));
    assert_eq!(drops(&fx_of(&fx, a.account)).unwrap().0.len(), 3);
    // found 2 chests (fewer than the free count): both open, nothing locked
    let fx = end_of(&mut w, &run(0x54, &[b.account], true, 2, 900));
    let (items, _, flag) = drops(&fx_of(&fx, b.account)).unwrap();
    assert_eq!((items.len(), flag), (2, false));
}

#[test]
fn unlock_opens_all_locked_chests_for_one_price_and_only_once() {
    game_data::require_tables!();
    let mut w = world(false);
    let (mut a, mut b) = (up(&mut w, "alpha"), up(&mut w, "bravo"));
    first_win_used(&w, &[a.account, b.account]);
    end_of(&mut w, &run(0x61, &[a.account, b.account], true, 6, 900));
    let (ra, rb) = (rows(&w, a.account, 0x61), rows(&w, b.account, 0x61));
    // the gold path: alpha has no access key
    drop_keys(&w, &a);
    // no key and no gold: refused, nothing changes and nothing is sent
    set_gold(&w, &a, 49);
    let profile = w.e.store.load_profile(a.account).unwrap();
    a.send(Request::Pve(Pve5F::UnlockRewards));
    pump(&mut w, &mut a, &mut [&mut b]);
    assert!(a.take().is_empty());
    assert_eq!((rows(&w, a.account, 0x61), w.e.store.load_profile(a.account).unwrap()), (ra.clone(), profile));
    // 50 gold: one price for the three locked chests
    set_gold(&w, &a, 50);
    a.send(Request::Pve(Pve5F::UnlockRewards));
    pump(&mut w, &mut a, &mut [&mut b]);
    let msgs = a.take();
    let after = rows(&w, a.account, 0x61);
    assert!(after.iter().all(|r| r.opened && r.reward_dict != 0), "all six open");
    assert_eq!(after[..3], ra[..3], "the free chests keep their rewards");
    assert!(after[3..].iter().all(|r| r.paid) && after[..3].iter().all(|r| !r.paid));
    assert_eq!(gold(&w, &a), gold_in(&after[3..]), "50 gold paid once for three chests (a gold reward may come back)");
    let (items, cost, flag) = drops(&msgs).expect("83 45 0B");
    assert_eq!((items.len(), cost, flag), (6, 50, false));
    assert!(msgs.iter().any(|m| matches!(m, S2c::Money(_))), "8B");
    // pressed again: nothing locked, nothing charged
    let g = gold(&w, &a);
    a.send(Request::Pve(Pve5F::UnlockRewards));
    pump(&mut w, &mut a, &mut [&mut b]);
    assert!(a.take().is_empty());
    assert_eq!(gold(&w, &a), g);
    // bravo's rows were not touched by alpha
    assert_eq!(rows(&w, b.account, 0x61), rb);
}

#[test]
fn reroll_costs_start_plus_step_per_reroll_and_swaps_the_rewards() {
    game_data::require_tables!();
    let mut w = world(false);
    let mut a = up(&mut w, "alpha");
    end_of(&mut w, &run(0x71, &[a.account], true, 3, 900));
    let mut last = rows(&w, a.account, 0x71);
    // not enough gold: refused
    set_gold(&w, &a, 49);
    a.send(Request::Pve(Pve5F::RerollRewards));
    pump(&mut w, &mut a, &mut []);
    assert!(a.take().is_empty());
    assert_eq!(rows(&w, a.account, 0x71), last);
    // three rerolls: 50, 75, 100 gold; the screen always shows the price of the next one
    set_gold(&w, &a, 1000);
    for (n, (paid, next)) in [(50u32, 75u32), (75, 100), (100, 125)].into_iter().enumerate() {
        let g = gold(&w, &a);
        a.send(Request::Pve(Pve5F::RerollRewards));
        pump(&mut w, &mut a, &mut []);
        let msgs = a.take();
        let now = rows(&w, a.account, 0x71);
        assert!(now.iter().all(|r| r.opened && r.rerolls == n as u32 + 1 && r.reward_dict != 0));
        assert_eq!(gold(&w, &a) + paid + gold_in(&last), g + gold_in(&now), "reroll {n}: {paid} gold, old gold reward back, new one in");
        let (items, cost, flag) = drops(&msgs).expect("83 45 0B");
        assert_eq!((items.len(), cost, flag), (3, next, false));
        assert_eq!(items.iter().map(|i| i.dict).collect::<Vec<_>>(), now.iter().map(|r| r.reward_dict).collect::<Vec<_>>());
        assert!(msgs.iter().any(|m| matches!(m, S2c::Money(_))));
        last = now;
    }
}

#[test]
fn the_end_screen_belongs_to_the_players_latest_run_and_own_rows() {
    game_data::require_tables!();
    let mut w = world(false);
    let (mut a, mut b) = (up(&mut w, "alpha"), up(&mut w, "bravo"));
    // without any run both requests are ignored
    for r in [Pve5F::UnlockRewards, Pve5F::RerollRewards] {
        a.send(Request::Pve(r));
    }
    pump(&mut w, &mut a, &mut [&mut b]);
    assert!(a.take().is_empty());
    set_gold(&w, &a, 500);
    set_gold(&w, &b, 500);
    first_win_used(&w, &[a.account]);
    end_of(&mut w, &run(0x81, &[a.account], true, 5, 900));
    let rb = rows(&w, b.account, 0x81);
    assert!(rb.is_empty(), "bravo was not in the run");
    // bravo cannot touch alpha's chests
    b.send(Request::Pve(Pve5F::UnlockRewards));
    b.send(Request::Pve(Pve5F::RerollRewards));
    pump(&mut w, &mut a, &mut [&mut b]);
    assert!(b.take().is_empty());
    assert_eq!(rows(&w, a.account, 0x81).iter().filter(|r| r.opened).count(), 3);
    // alpha loses the next run: the screen of the latest run has no chests, so alpha's old chests are not reachable
    end_of(&mut w, &run(0x82, &[a.account], false, 5, 900));
    let g = gold(&w, &a);
    a.send(Request::Pve(Pve5F::UnlockRewards));
    a.send(Request::Pve(Pve5F::RerollRewards));
    pump(&mut w, &mut a, &mut []);
    assert!(a.take().is_empty());
    assert_eq!(gold(&w, &a), g);
    assert_eq!(rows(&w, a.account, 0x81).iter().filter(|r| r.opened).count(), 3);
}

#[test]
fn the_first_won_mission_of_the_day_opens_every_chest() {
    game_data::require_tables!();
    let mut w = world(false);
    let a = up(&mut w, "alpha");
    // a lost run is not "completed": the first win is still to come
    end_of(&mut w, &run(0x91, &[a.account], false, 5, 900));
    // the first won run of the day: all five chests open, nothing locked
    let fx = end_of(&mut w, &run(0x92, &[a.account], true, 5, 900));
    assert!(rows(&w, a.account, 0x92).iter().all(|r| r.opened && r.reward_dict != 0 && !r.paid));
    let (items, cost, flag) = drops(&fx_of(&fx, a.account)).expect("83 45 0B");
    assert_eq!((items.len(), cost, flag), (5, 50, false));
    // the second one that day: three free, two locked
    end_of(&mut w, &run(0x93, &[a.account], true, 5, 900));
    assert_eq!(rows(&w, a.account, 0x93).iter().filter(|r| r.opened).count(), 3);
    // the next day starts over
    end_on(&mut w, &run(0x94, &[a.account], true, 5, 900), DAY + 1);
    assert_eq!(rows(&w, a.account, 0x94).iter().filter(|r| r.opened).count(), 5);
}

#[test]
fn unlock_spends_one_access_key_before_gold() {
    game_data::require_tables!();
    let mut w = world(false);
    let mut a = up(&mut w, "alpha");
    first_win_used(&w, &[a.account]);
    end_of(&mut w, &run(0xA1, &[a.account], true, 6, 900));
    // one free and one premium key; 50 gold on the account
    drop_keys(&w, &a);
    w.e.store
        .update_profile(a.account, |p| {
            p.add_item(KEY_PREMIUM, 1);
            p.add_item(KEY_FREE, 1);
            Ok::<(), ()>(())
        })
        .unwrap();
    set_gold(&w, &a, 50);
    a.send(Request::Pve(Pve5F::UnlockRewards));
    pump(&mut w, &mut a, &mut []);
    let msgs = a.take();
    let r = rows(&w, a.account, 0xA1);
    assert!(r.iter().all(|x| x.opened && x.reward_dict != 0), "all six open");
    assert_eq!(keys(&w, &a), (0, 1), "the free key is spent first");
    assert_eq!(gold(&w, &a), 50 + gold_in(&r[3..]), "no gold paid (a gold reward may come in)");
    assert_eq!(drops(&msgs).map(|d| (d.0.len(), d.2)), Some((6, false)));
    assert!(
        msgs.iter().any(|m| matches!(m, S2c::Reply(Reply::Inventory(Inventory45::Records { items, .. })) if items.iter().any(|i| i.dict == KEY_FREE && i.value == 0))),
        "the key stack change is sent: {msgs:?}"
    );
}
