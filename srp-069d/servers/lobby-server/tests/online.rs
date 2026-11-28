//! Online matches through the lobby (online.rs) with a recording match server hook: queue group -> `82`, custom room
//! Start -> `82`, the clients' win / loss reports, rewards, refusals. The real match server behind the hook is tested in
//! `servers/srp/tests/online.rs`.

mod support;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use accounts::public_id;
use game_data::GameData;
use lobby_server::online::Left;
use lobby_server::{BotFill, Ended, EndedPlayer, LobbyConfig, MinimalMatches, OnlineMatches, Seat, Ticket82};
use protocol::c2s::{Event53, Request};
use protocol::s2c::*;
use support::*;

const MATCH_ID: u32 = 0xA1B2_C3D4;

#[derive(Default)]
struct Fake {
    created: Mutex<Vec<(u32, u32, u32, Vec<(i64, u8, Option<u32>)>)>>,
    ended: Mutex<Vec<Ended>>,
    fail: Mutex<Option<String>>,
    /// modes that have a world (None = every mode)
    modes: Mutex<Option<Vec<u32>>>,
    /// per create: (account, the per-match boosters the seat's profile carries into the options)
    boosters: Mutex<Vec<(i64, Vec<u16>)>>,
    /// the options match-type byte of every create
    types: Mutex<Vec<u8>>,
    /// the lobby's end requests
    ends: Mutex<Vec<u32>>,
    /// the lobby's close-after-the-result requests (PvP end screen)
    after: Mutex<Vec<u32>>,
    /// PvP end screen: the match server still runs the clients after the result / still has the match
    holds: Mutex<bool>,
    open: Mutex<bool>,
    /// players who left (the hub's per-player leave events)
    left: Mutex<Vec<Left>>,
    /// the hook drives bots
    bots: Mutex<bool>,
    /// every create_with_bots: (map, mode, seats (account, team), fill)
    bot_creates: Mutex<Vec<(u32, u32, Vec<(i64, u8)>, BotFill)>>,
    /// rejoin: players dropped with their slot kept, players back, whether a dropped slot may come back, declines
    dropped: Mutex<Vec<Left>>,
    rejoined: Mutex<Vec<(u32, i64)>>,
    allow_rejoin: Mutex<bool>,
    declined: Mutex<Vec<(u32, i64)>>,
    /// rejoin: every allow_rejoin call: (account, the login session handed over)
    rejoin_sessions: Mutex<Vec<(i64, Option<u32>)>>,
    /// serve every row of the data (a world host listing all projects; default: only the variation-0 rows)
    serve_all: Mutex<bool>,
}

impl OnlineMatches for Fake {
    fn available(&self, _: u32, mode: u32) -> bool {
        self.modes.lock().unwrap().as_ref().is_none_or(|m| m.contains(&mode))
    }

    fn serves(&self, data: &GameData, row: &game_data::ModeRow) -> bool {
        self.available(row.map, row.mode) && (*self.serve_all.lock().unwrap() || data.matches.resolve_map(row.map, row.mode, 0) == Some(row))
    }

    fn create(&self, _: &GameData, map: u32, mode: u32, weather: u32, match_type: u8, seats: &[Seat]) -> Result<Ticket82, String> {
        if let Some(e) = self.fail.lock().unwrap().clone() {
            return Err(e);
        }
        self.created.lock().unwrap().push((map, mode, weather, seats.iter().map(|s| (s.account, s.team, s.session)).collect()));
        self.types.lock().unwrap().push(match_type);
        for s in seats {
            let on = s.profile.boosters.iter().filter(|(_, b)| b.enabled).map(|(d, _)| *d).collect();
            self.boosters.lock().unwrap().push((s.account, on));
        }
        Ok(Ticket82 { match_id: MATCH_ID, host: "match.example".into(), port: 1236, orders: (0..seats.len() as u32).map(|i| 100 + i).collect() })
    }

    fn take_ended(&self) -> Vec<Ended> {
        std::mem::take(&mut *self.ended.lock().unwrap())
    }

    fn bots_available(&self) -> bool {
        *self.bots.lock().unwrap()
    }

    fn create_with_bots(&self, _: &GameData, map: u32, mode: u32, _: u32, _: u8, seats: &[Seat], fill: BotFill) -> Result<Ticket82, String> {
        self.bot_creates.lock().unwrap().push((map, mode, seats.iter().map(|s| (s.account, s.team)).collect(), fill));
        Ok(Ticket82 { match_id: MATCH_ID, host: "match.example".into(), port: 1236, orders: (0..seats.len() as u32).map(|i| 100 + i).collect() })
    }

    fn end(&self, match_id: u32) -> bool {
        self.ends.lock().unwrap().push(match_id);
        true
    }

    fn end_after_result(&self, match_id: u32) -> bool {
        self.after.lock().unwrap().push(match_id);
        true
    }

    fn holds_clients(&self, _: u32) -> bool {
        *self.holds.lock().unwrap()
    }

    fn has_match(&self, _: u32) -> bool {
        *self.open.lock().unwrap()
    }

    fn take_left(&self) -> Vec<Left> {
        std::mem::take(&mut *self.left.lock().unwrap())
    }

    fn take_dropped(&self) -> Vec<Left> {
        std::mem::take(&mut *self.dropped.lock().unwrap())
    }

    fn take_rejoined(&self) -> Vec<(u32, i64)> {
        std::mem::take(&mut *self.rejoined.lock().unwrap())
    }

    fn allow_rejoin(&self, _: u32, account: i64, session: Option<u32>) -> bool {
        self.rejoin_sessions.lock().unwrap().push((account, session));
        *self.allow_rejoin.lock().unwrap()
    }

    fn leave_dropped(&self, match_id: u32, account: i64) -> bool {
        self.declined.lock().unwrap().push((match_id, account));
        // the match server's answer: the slot leaves (a drop, not the menu)
        self.left.lock().unwrap().push(Left { match_id, account, by_menu: false });
        true
    }
}

struct World {
    _dir: tempfile::TempDir,
    e: Env,
    fake: Arc<Fake>,
    clients: Vec<Client>,
}

fn pump_all(e: &mut Env, clients: &mut [Client]) {
    for _ in 0..40 {
        let mut refs: Vec<&mut Client> = clients.iter_mut().collect();
        e.step(&mut refs, Duration::from_millis(30));
    }
}

fn setup(n: usize, queue: Option<(u32, u32, u32)>) -> World {
    setup_with(n, n as u32, queue)
}

/// `n` signed-in clients; a queue group of `min` players starts a match.
fn setup_with(n: usize, min: u32, queue: Option<(u32, u32, u32)>) -> World {
    setup_cfg(n, min, queue, |_| {})
}

fn setup_cfg(n: usize, min: u32, queue: Option<(u32, u32, u32)>, tune: impl FnOnce(&mut LobbyConfig)) -> World {
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = LobbyConfig {
        achievement_rewards: false,
        daily_quests: false,
        treasure_hunt: false,
        online_queue: queue,
        online_report_grace: Duration::from_secs(5),
        online_min_players: min,
        ..LobbyConfig::default()
    };
    tune(&mut cfg);
    let mut e = Env::with(&dir.path().join("o.sqlite"), cfg, Arc::new(MinimalMatches));
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
    World { _dir: dir, e, fake, clients }
}

/// The smallest team PvP row at variation 0 (the pinned `online_queue` value resolves variation-0 rows; the 10-player
/// Search and Destroy rows are variations 5..7; the gun race, all versus all, has its own test): (map, mode, weather)
/// and how many players the queue needs.
fn smallest_row() -> (u32, u32, u32, usize) {
    let gd = game_data::testing::tables();
    let r = gd.matches.modes().iter().filter(|r| r.min_players >= 2 && r.variation == 0 && r.mode != game_data::gunrace::MODE).min_by_key(|r| (r.min_players, r.map, r.mode)).unwrap().clone();
    (r.map, r.mode, r.weather, r.min_players as usize)
}

fn connect_msg(msgs: &[S2c]) -> Option<ConnectToMatch> {
    msgs.iter().find_map(|m| match m {
        S2c::Raw { id: 0x82, payload } => <ConnectToMatch as protocol::Decode>::from_bytes(payload).ok(),
        _ => None,
    })
}

fn queue_all(w: &mut World) {
    let all: Vec<(usize, u32)> = (0..w.clients.len()).map(|i| (i, 0x380)).collect();
    queue_masks(w, &all);
}

/// The listed clients press Play with their own `2F` game-mode mask, one after the other.
fn queue_masks(w: &mut World, who: &[(usize, u32)]) {
    let World { e, clients, .. } = w;
    for &(i, mask) in who {
        clients[i].send(Request::Queue { global: 0, arg: 0, mode: mask, flag: 0 });
        pump_all(e, clients);
    }
}

fn report(w: &mut World, who: usize, id: u16, value: u32) {
    let World { e, clients, .. } = w;
    clients[who].send(Request::Events(vec![Event53 { id, x: 0, value, time: 0 }]));
    pump_all(e, clients);
}

fn silver(w: &World, who: usize) -> u32 {
    w.e.store.load_profile(w.clients[who].account).unwrap().silver
}

#[test]
fn queue_group_gets_82_and_is_paid_by_the_reports() {
    game_data::require_tables!();
    let (map, mode, weather, n) = smallest_row();
    let mut w = setup(n, Some((map, mode, weather)));
    let before: Vec<u32> = (0..n).map(|i| silver(&w, i)).collect();
    queue_all(&mut w);

    // one match, every player has its own order id and a team; the two teams are even
    let created = w.fake.created.lock().unwrap().clone();
    assert_eq!(created.len(), 1);
    assert_eq!((created[0].0, created[0].1, created[0].2), (map, mode, weather));
    assert_eq!(*w.fake.types.lock().unwrap(), vec![protocol::s2c::online::MATCH_TYPE_RANDOM], "a random match: match type 1 (inactivity tracking)");
    let seats = &created[0].3;
    assert_eq!(seats.len(), n);
    for (i, s) in seats.iter().enumerate() {
        assert!(s.2.is_some(), "the login session travels to the match server (C9 carries it)");
        assert_eq!(s.2, w.clients.iter().find(|c| c.account == s.0).map(|c| c.session));
        assert!(i < n);
    }
    let teams = [seats.iter().filter(|s| s.1 == 0).count(), seats.iter().filter(|s| s.1 == 1).count()];
    assert!(teams[0].abs_diff(teams[1]) <= 1 && teams[0] + teams[1] == n, "{teams:?}");

    let mut sides = vec![];
    let mut team_of = std::collections::HashMap::new();
    for (i, c) in w.clients.iter_mut().enumerate() {
        let m = c.take();
        let t = connect_msg(&m).unwrap_or_else(|| panic!("client {i} got no 82: {m:?}"));
        assert_eq!((t.host.as_str(), t.port, t.match_id), ("match.example", 1236, MATCH_ID));
        let seat = seats.iter().position(|s| s.0 == c.account).unwrap();
        assert_eq!(t.order_id, 100 + seat as u32);
        assert_eq!(t.side, seats[seat].1);
        assert!(!m.contains(&S2c::QueueState(QueueState { state: 0 })), "not refused");
        sides.push(t.side);
        team_of.insert(i, t.side);
    }
    assert!(sides.contains(&0) && sides.contains(&1));
    assert_eq!(w.e.server.registry.online.len(), 1);
    assert!(w.e.server.registry.queue.is_empty(), "the tickets are used up");
    // chat context of the match (channels 5..7)
    assert!(w.clients.iter().all(|c| w.e.server.registry.match_context(c.account).is_some_and(|x| x.0 == MATCH_ID)));

    // team 0 won: its players report 5, team 1 reports 6 (the client's own events, value = match id)
    for i in 0..n {
        assert!(w.fake.ends.lock().unwrap().is_empty(), "no end before every player reported");
        let id = if team_of[&i] == 0 { 5 } else { 6 };
        report(&mut w, i, id, MATCH_ID);
    }
    // every seat reported: the lobby asks the match server to end the match (the clients' own end sends it nothing)
    assert_eq!(*w.fake.ends.lock().unwrap(), vec![MATCH_ID], "asked once");
    // the match server says it ended; everybody reported, so the lobby pays at once
    let players = w.clients.iter().map(|c| EndedPlayer { account: c.account, team: seats.iter().find(|s| s.0 == c.account).unwrap().1, entered: true, left_by_menu: false }).collect();
    w.fake.ended.lock().unwrap().push(Ended { match_id: MATCH_ID, duration: Duration::from_secs(600), players, stats: None, winner: None });
    let World { e, clients, .. } = &mut w;
    pump_all(e, clients);
    let accounts: Vec<i64> = w.clients.iter().map(|c| c.account).collect();
    for c in w.clients.iter_mut() {
        let m = c.take();
        let ids: Vec<u8> = m.iter().map(|x| x.id()).collect();
        assert!(ids.contains(&0x8B) && ids.contains(&0x8C) && ids.contains(&0x8E), "rewards pushed: {ids:02x?}");
        assert_eq!(ids.last(), Some(&0x85), "85 00 last");
        // the results screen: names, teams, the winner of the reports; no numbers invented (no statistics source)
        let at97 = ids.iter().position(|i| *i == 0x97).expect("97 results screen");
        assert!(at97 < ids.iter().position(|i| *i == 0x8B).unwrap(), "97 before the reward pushes: {ids:02x?}");
        let Some(S2c::MatchResult(r)) = m.iter().find(|x| x.id() == 0x97) else { panic!("97 not decoded: {m:?}") };
        assert_eq!((r.match_id, r.winner_team, r.team0_score, r.team1_score), (MATCH_ID, 0, 0, 0));
        assert_eq!(r.players.len(), n);
        for (i, acc) in accounts.iter().enumerate() {
            let p = r.players.iter().find(|p| p.id == public_id(*acc)).expect("every player is listed");
            assert_eq!(p.name, format!("player{i}"));
            assert_eq!(p.team, team_of[&i]);
            assert_eq!((p.score, p.kills, p.deaths), (0, 0, 0));
        }
    }
    for i in 0..n {
        let now = w.e.store.load_profile(w.clients[i].account).unwrap().silver;
        // no statistics: only the data's drop_after_combat silver roll of the online match level
        let gd = game_data::testing::tables();
        let row = gd.drops.money.iter().find(|r| r.match_level == lobby_server::online::ONLINE_MATCH_LEVEL).unwrap();
        let got = now - before[i];
        assert!((row.min..=row.max).contains(&got), "client {i} (team {}) is paid the silver roll: {got}", team_of[&i]);
    }
    assert!(w.e.server.registry.online.is_empty());
    assert!(w.e.server.registry.match_context(w.clients[0].account).is_none());
}

/// A player who leaves the running match (menu, closed game, no CD for 5 s) gets his lobby back at once: QueueState 0,
/// Play works again (the leaver's queue timer stops); the others stay in the match.
#[test]
fn a_player_who_leaves_is_released_at_once() {
    game_data::require_tables!();
    let (map, mode, weather, n) = smallest_row();
    let mut w = setup(n, Some((map, mode, weather)));
    queue_all(&mut w);
    for c in w.clients.iter_mut() {
        c.take();
    }
    let leaver = w.clients[0].account;
    w.fake.left.lock().unwrap().push(Left { match_id: MATCH_ID, account: leaver, by_menu: true });
    let World { e, clients, .. } = &mut w;
    pump_all(e, clients);
    let free = S2c::QueueState(QueueState { state: 0 });
    assert!(w.clients[0].take().contains(&free), "the leaver is released at once");
    assert!(!w.clients[1].take().contains(&free), "the others stay in the match");
    assert_eq!(w.e.server.registry.online.len(), 1, "the match goes on");
    assert!(w.e.server.registry.match_context(leaver).is_none(), "no match chat for the leaver");
}

#[test]
fn a_hero_title_shows_in_the_results_rows_and_its_bonus_in_its_own_row() {
    game_data::require_tables!();
    let (map, mode, weather, n) = smallest_row();
    let mut w = setup(n, Some((map, mode, weather)));
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
    let hero_acc = w.clients[0].account;
    let title = accounts::HeroTitle { account: hero_acc, group: 0, nomination: 2, place: 1, value: 1, since: now - 60, until: now + 86_400 };
    // titles live in their own table (read into the profile on load)
    w.e.store.hero_commit_slot(now - 60, &[title], 0).unwrap();
    queue_all(&mut w);
    let seats = w.fake.created.lock().unwrap()[0].3.clone();
    for c in w.clients.iter_mut() {
        c.take();
    }
    for i in 0..n {
        let team = seats.iter().find(|s| s.0 == w.clients[i].account).unwrap().1;
        report(&mut w, i, if team == 0 { 5 } else { 6 }, MATCH_ID);
    }
    let players = w.clients.iter().map(|c| EndedPlayer { account: c.account, team: seats.iter().find(|s| s.0 == c.account).unwrap().1, entered: true, left_by_menu: false }).collect();
    w.fake.ended.lock().unwrap().push(Ended { match_id: MATCH_ID, duration: Duration::from_secs(600), players, stats: None, winner: None });
    let World { e, clients, .. } = &mut w;
    pump_all(e, clients);
    let gd = game_data::testing::tables();
    for c in w.clients.iter_mut() {
        let m = c.take();
        let Some(S2c::MatchResult(r)) = m.iter().find(|x| x.id() == 0x97) else { panic!("97: {m:?}") };
        let row = r.players.iter().find(|p| p.id == public_id(hero_acc)).unwrap();
        assert_eq!(row.tail, [1, 1, 0, 0], "place 1, nomination 2 = index 1");
        assert!(r.players.iter().filter(|p| p.id != public_id(hero_acc)).all(|p| p.tail == [0, 255, 0, 0]), "no title: 0 / 255");
        let hero_row = r.full.score[9][0];
        if c.account == hero_acc {
            let b = gd.hero.bonus(0, 1).expect("group 0 place 1 bonus");
            assert_eq!((b.experience, b.silver, b.reputation), (0.25, 0.25, 0.25), "place 1 = +25 % (hero_of_the_day.options)");
            assert_eq!([2u8, 3].map(|p| gd.hero.bonus(0, p).unwrap().experience), [0.2, 0.15], "place 2 / 3 = +20 / +15 %");
            assert!(hero_row.money > 0 || hero_row.exp > 0, "the hero part has its own row: {hero_row:?}");
        } else {
            assert_eq!(hero_row, protocol::s2c::ScoreComponent::default());
        }
    }
}

/// A gold booster (853, experience +50 % "for one match") selected in the panel is paid when the online match starts
/// (5 gold, its 8A price) and its bonus is paid with that match: the 97 boosters block (3) of its owner only.
#[test]
fn a_gold_booster_is_paid_at_the_online_start_and_its_bonus_goes_with_that_match() {
    game_data::require_tables!();
    let (map, mode, weather, n) = smallest_row();
    let mut w = setup(n, Some((map, mode, weather)));
    let owner = w.clients[0].account;
    for c in &w.clients {
        w.e.store
            .update_profile(c.account, |p| {
                p.gold = 10;
                if c.account == owner {
                    p.boosters.entry(853).or_default().enabled = true;
                }
                Ok::<(), ()>(())
            })
            .unwrap();
    }
    let gold = |w: &World, i: usize| w.e.store.load_profile(w.clients[i].account).unwrap().gold;
    queue_all(&mut w);
    assert_eq!(w.fake.created.lock().unwrap().len(), 1);
    assert_eq!((gold(&w, 0), gold(&w, 1)), (5, 10), "5 gold at the start, nothing for the others");
    let seats = w.fake.created.lock().unwrap()[0].3.clone();
    for c in w.clients.iter_mut() {
        c.take();
    }
    for i in 0..n {
        let team = seats.iter().find(|s| s.0 == w.clients[i].account).unwrap().1;
        report(&mut w, i, if team == 0 { 5 } else { 6 }, MATCH_ID);
    }
    let players = w.clients.iter().map(|c| EndedPlayer { account: c.account, team: seats.iter().find(|s| s.0 == c.account).unwrap().1, entered: true, left_by_menu: false }).collect();
    w.fake.ended.lock().unwrap().push(Ended { match_id: MATCH_ID, duration: Duration::from_secs(600), players, stats: None, winner: None });
    let World { e, clients, .. } = &mut w;
    pump_all(e, clients);
    for c in w.clients.iter_mut() {
        let m = c.take();
        let Some(S2c::MatchResult(r)) = m.iter().find(|x| x.id() == 0x97) else { panic!("97: {m:?}") };
        let boosters_row = r.full.score[3].iter().fold(0u32, |a, b| a + u32::from(b.exp));
        if c.account == owner {
            assert!(boosters_row > 0, "the experience booster's share has its own row");
        } else {
            assert_eq!(boosters_row, 0);
        }
    }
    assert!(w.e.store.load_profile(owner).unwrap().boosters[&853].enabled, "the selection stays for the next match");
}

#[test]
fn a_player_who_reported_nothing_and_a_split_vote_pay_nothing() {
    game_data::require_tables!();
    let (map, mode, weather, n) = smallest_row();
    let mut w = setup(n, Some((map, mode, weather)));
    let before: Vec<u32> = (0..n).map(|i| silver(&w, i)).collect();
    queue_all(&mut w);
    for c in w.clients.iter_mut() {
        c.take();
    }
    let seats = w.fake.created.lock().unwrap()[0].3.clone();
    let team = |w: &World, i: usize| seats.iter().find(|s| s.0 == w.clients[i].account).unwrap().1;
    // everybody but client 0 reports; client 0 left early. Team 0 says 5, team 1 says 6.
    for i in 1..n {
        let id = if team(&w, i) == 0 { 5 } else { 6 };
        report(&mut w, i, id, MATCH_ID);
    }
    // a report with another match id is not a report
    report(&mut w, 0, 5, MATCH_ID + 1);
    let players: Vec<EndedPlayer> = w.clients.iter().map(|c| EndedPlayer { account: c.account, team: seats.iter().find(|s| s.0 == c.account).unwrap().1, entered: true, left_by_menu: false }).collect();
    w.fake.ended.lock().unwrap().push(Ended { match_id: MATCH_ID, duration: Duration::from_secs(90), players, stats: None, winner: None });
    {
        let World { e, clients, .. } = &mut w;
        pump_all(e, clients);
        // not everybody reported: the lobby waits for the grace time (5 s here)
        assert_eq!(e.server.registry.online.len(), 1, "waiting for client 0");
        for _ in 0..30 {
            let mut refs: Vec<&mut Client> = clients.iter_mut().collect();
            e.step(&mut refs, Duration::from_millis(250));
        }
    }
    assert!(w.e.server.registry.online.is_empty(), "paid after the grace time");
    // the leaver pays the leaving fine: 1000 x his equipment tier (game_data::fines), and is told so (84 2F text)
    let gd = game_data::testing::tables();
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
    let p0 = w.e.store.load_profile(w.clients[0].account).unwrap();
    let tier = lobby_server::items::equipment_level(&gd, &p0, now);
    assert_eq!(silver(&w, 0), before[0] - game_data::fines::leaving_fine(tier, 0), "the player who reported nothing pays the fine");
    assert_eq!((p0.penalty.count, p0.penalty.blocked_until <= now), (1, true), "the first leave of a day blocks nothing");
    let mut paid = 0;
    for i in 1..n {
        if silver(&w, i) > before[i] {
            paid += 1;
        }
    }
    assert_eq!(paid, n - 1);
    let m = w.clients[0].take();
    assert!(m.contains(&S2c::QueueState(QueueState { state: 0 })), "the leaver gets 85 00 so Play comes back");
    // no ban on the first leave: no text (the fine has no client key; 8B shows it)
    assert!(!m.iter().any(|x| matches!(x, S2c::Denied(Denied { request: 0x2F, .. }))), "{m:?}");

    // second match: both teams claim the win -> unknown -> nobody is paid
    for c in w.clients.iter_mut() {
        c.take();
    }
    let before: Vec<u32> = (0..n).map(|i| silver(&w, i)).collect();
    queue_all(&mut w);
    assert_eq!(w.fake.created.lock().unwrap().len(), 2);
    for i in 0..n {
        report(&mut w, i, 5, MATCH_ID);
    }
    let seats = w.fake.created.lock().unwrap()[1].3.clone();
    let players: Vec<EndedPlayer> = w.clients.iter().map(|c| EndedPlayer { account: c.account, team: seats.iter().find(|s| s.0 == c.account).unwrap().1, entered: true, left_by_menu: false }).collect();
    w.fake.ended.lock().unwrap().push(Ended { match_id: MATCH_ID, duration: Duration::from_secs(90), players, stats: None, winner: None });
    let World { e, clients, .. } = &mut w;
    pump_all(e, clients);
    assert!(w.e.server.registry.online.is_empty());
    for i in 0..n {
        assert_eq!(silver(&w, i), before[i], "no consensus on the winner: nobody paid");
    }
}

#[test]
fn a_menu_leaver_is_fined_and_unpaid_even_when_his_client_reports() {
    game_data::require_tables!();
    let (map, mode, weather, n) = smallest_row();
    let mut w = setup(n, Some((map, mode, weather)));
    let before: Vec<u32> = (0..n).map(|i| silver(&w, i)).collect();
    queue_all(&mut w);
    for c in w.clients.iter_mut() {
        c.take();
    }
    let seats = w.fake.created.lock().unwrap()[0].3.clone();
    let team = |w: &World, i: usize| seats.iter().find(|s| s.0 == w.clients[i].account).unwrap().1;
    // everybody reports a consistent result, client 0 included, but client 0 left with the menu (the match server got CE)
    for i in 0..n {
        let id = if team(&w, i) == 0 { 5 } else { 6 };
        report(&mut w, i, id, MATCH_ID);
    }
    let leaver = w.clients[0].account;
    let players: Vec<EndedPlayer> = w
        .clients
        .iter()
        .map(|c| EndedPlayer { account: c.account, team: seats.iter().find(|s| s.0 == c.account).unwrap().1, entered: true, left_by_menu: c.account == leaver })
        .collect();
    w.fake.ended.lock().unwrap().push(Ended { match_id: MATCH_ID, duration: Duration::from_secs(90), players, stats: None, winner: None });
    let World { e, clients, .. } = &mut w;
    pump_all(e, clients);
    assert!(w.e.server.registry.online.is_empty(), "all reports are in: finished at once");
    let gd = game_data::testing::tables();
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
    let p0 = w.e.store.load_profile(leaver).unwrap();
    let tier = lobby_server::items::equipment_level(&gd, &p0, now);
    assert_eq!(silver(&w, 0), before[0] - game_data::fines::leaving_fine(tier, 0), "fined, not paid");
    assert_eq!(p0.penalty.count, 1);
    for i in 1..n {
        assert!(silver(&w, i) > before[i], "the others are paid");
    }
}

#[test]
fn a_queue_block_refuses_the_search_with_the_time_left() {
    game_data::require_tables!();
    let (map, mode, weather, n) = smallest_row();
    let mut w = setup(n, Some((map, mode, weather)));
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
    let acc = w.clients[0].account;
    w.e.store.update_profile(acc, |p| Ok::<_, ()>(p.penalty = accounts::LeavePenalty { day: now / 86_400, count: 2, blocked_until: now + 170 })).unwrap();
    w.clients[0].send(Request::Queue { global: 0, arg: 0, mode: 0x380, flag: 0 });
    let World { e, clients, .. } = &mut w;
    pump_all(e, clients);
    let m = w.clients[0].take();
    // the client's own st_cant_play_leave with a countdown of b + 1 seconds (84 2F numeric code 0, a = 0)
    assert!(m.iter().any(|x| matches!(x, S2c::Denied(Denied { request: 0x2F, body: DeniedBody::Numeric { code: 0, a: 0, b } }) if (160..=169).contains(b))), "{m:?}");
    assert!(m.contains(&S2c::QueueState(QueueState { state: 0 })));
    assert!(w.e.server.registry.queue.ticket_of(acc).is_none());
}

#[test]
fn queue_refuses_as_before_when_online_is_off_or_the_server_fails() {
    game_data::require_tables!();
    let (map, mode, weather, n) = smallest_row();
    // the hook fails: the group is refused (85 00), nothing is left running
    let mut w = setup(n, Some((map, mode, weather)));
    *w.fake.fail.lock().unwrap() = Some("no room".into());
    queue_all(&mut w);
    for c in w.clients.iter_mut() {
        let m = c.take();
        assert!(connect_msg(&m).is_none());
        assert!(m.contains(&S2c::QueueState(QueueState { state: 0 })));
    }
    assert!(w.e.server.registry.online.is_empty());
    // no queue world configured and no world cached at all: refused
    let mut w = setup(n, None);
    *w.fake.modes.lock().unwrap() = Some(vec![]);
    queue_all(&mut w);
    assert!(w.fake.created.lock().unwrap().is_empty());
    for c in w.clients.iter_mut() {
        assert!(connect_msg(&c.take()).is_none());
    }
    // no match server attached: the same
    let mut w = setup(n, Some((map, mode, weather)));
    w.e.server.lobby.online = None;
    queue_all(&mut w);
    assert!(w.fake.created.lock().unwrap().is_empty());
}

#[test]
fn custom_room_start_sends_82_and_closes_the_room() {
    game_data::require_tables!();

    let mut w = setup(2, None);
    let (a, b) = (w.clients[0].account, w.clients[1].account);
    let send = |w: &mut World, who: usize, r: Custom62| {
        let World { e, clients, .. } = w;
        clients[who].send(Request::Other { id: 0x62, payload: r.encode() });
        pump_all(e, clients);
    };
    send(&mut w, 0, Custom62::Create { kind: 0 });
    send(&mut w, 0, Custom62::Invite { target: public_id(b) });
    send(&mut w, 1, Custom62::Answer { inviter: public_id(a), accept: 0 });
    // bob has not picked a team yet: he is in the reserve, which does not block; team B is empty
    for c in w.clients.iter_mut() {
        c.take();
    }
    send(&mut w, 0, Custom62::Start);
    let m = w.clients[0].take();
    assert!(m.iter().any(|x| matches!(x, S2c::Denied(d) if matches!(&d.body, DeniedBody::Text(t) if t == "st_custom_match_error_team_empty"))), "{m:?}");
    send(&mut w, 1, Custom62::MemberStatus { player: public_id(b), status: 1 });
    // bob has not said ready (62 22) yet: no match
    for c in w.clients.iter_mut() {
        c.take();
    }
    send(&mut w, 0, Custom62::Start);
    let m = w.clients[0].take();
    assert!(m.iter().any(|x| matches!(x, S2c::Denied(d) if matches!(&d.body, DeniedBody::Text(t) if t == "st_custom_match_error_not_all_members_ready"))), "{m:?}");
    send(&mut w, 1, Custom62::Ready { ready: 0xFF });
    // this match server drives no bots: the 98 offers none (max_ai 0 hides the client's bot settings) and 62 20 is refused
    w.clients[0].take();
    send(&mut w, 0, Custom62::AiCount { team: 0, count: 1 });
    let m = w.clients[0].take();
    assert!(m.iter().any(|x| matches!(x, S2c::Denied(d) if matches!(&d.body, DeniedBody::Text(t) if t == "st_cant_change_ai_count_custom_match_error"))), "{m:?}");
    assert_eq!(room98(&m).map(|r| (r.ai_a, r.max_ai)), Some((0, 0)));
    // a member cannot start it
    for c in w.clients.iter_mut() {
        c.take();
    }
    send(&mut w, 1, Custom62::Start);
    assert!(w.fake.created.lock().unwrap().is_empty());
    // the cost: one token per player without premium; alice pays hers in silver, bob his in gold (62 21)
    send(&mut w, 0, Custom62::Start);
    assert!(w.clients[0].take().iter().any(|x| matches!(x, S2c::Denied(d) if matches!(&d.body, DeniedBody::Text(t) if t == "st_cant_start_custom_match_error"))));
    send(&mut w, 0, Custom62::Tokens { gold: 0, silver: 1 });
    send(&mut w, 1, Custom62::Tokens { gold: 1, silver: 0 });
    for c in w.clients.iter_mut() {
        c.take();
    }
    let (sa, gb) = (w.e.store.load_profile(a).unwrap().silver, w.e.store.load_profile(b).unwrap().gold);
    // the host starts: both get 82 with their own side, the room is gone
    send(&mut w, 0, Custom62::Start);
    assert_eq!((w.e.store.load_profile(a).unwrap().silver, w.e.store.load_profile(b).unwrap().gold), (sa - 5000, gb - 20), "tokens charged at the start");
    let (ma, mb) = (w.clients[0].take(), w.clients[1].take());
    let (ta, tb) = (connect_msg(&ma).expect("host 82"), connect_msg(&mb).expect("member 82"));
    assert_eq!((ta.side, tb.side), (0, 1), "team A is side 0, team B side 1");
    // the room is over for both: A0 (the client resets its room record), no empty 98
    for m in [&ma, &mb] {
        assert!(m.iter().any(|x| matches!(x, S2c::Raw { id: 0xA0, payload } if payload.is_empty())), "{m:?}");
        assert!(room98(m).is_none());
    }
    assert_ne!(ta.order_id, tb.order_id);
    assert!(w.e.server.registry.rooms.is_empty());
    let created = w.fake.created.lock().unwrap().clone();
    assert_eq!(created.len(), 1);
    assert_eq!(created[0].3.iter().map(|s| (s.0, s.1)).collect::<Vec<_>>(), vec![(a, 0), (b, 1)]);

    // a custom TRAINING match pays nothing (OFFICIAL 0.65b): reports and the end change no money, xp or reputation
    let snap = |w: &World| -> Vec<(u32, u32, u32, u32)> {
        [a, b].iter().map(|id| {
            let p = w.e.store.load_profile(*id).unwrap();
            (p.silver, p.gold, p.parts, p.xp)
        }).collect()
    };
    let before = snap(&w);
    let rep_before: Vec<_> = [a, b].iter().map(|id| w.e.store.load_profile(*id).unwrap().fac_points).collect();
    report(&mut w, 0, 5, MATCH_ID);
    report(&mut w, 1, 6, MATCH_ID);
    let players = vec![EndedPlayer { account: a, team: 0, entered: true, left_by_menu: false }, EndedPlayer { account: b, team: 1, entered: true, left_by_menu: false }];
    w.fake.ended.lock().unwrap().push(Ended { match_id: MATCH_ID, duration: Duration::from_secs(600), players, stats: None, winner: None });
    let World { e, clients, .. } = &mut w;
    pump_all(e, clients);
    for c in w.clients.iter_mut() {
        let ids: Vec<u8> = c.take().iter().map(|x| x.id()).collect();
        assert_eq!(ids.last(), Some(&0x85), "85 00 ends the match");
        assert!(!ids.contains(&0x8B) && !ids.contains(&0x8C), "no reward pushes: {ids:02x?}");
        assert!(ids.contains(&0x97), "the results screen still shows who played and who won: {ids:02x?}");
    }
    assert_eq!(snap(&w), before, "money and xp unchanged");
    let rep_after: Vec<_> = [a, b].iter().map(|id| w.e.store.load_profile(*id).unwrap().fac_points).collect();
    assert_eq!(rep_after, rep_before, "reputation unchanged");
    assert!(w.e.server.registry.online.is_empty());
}

fn tdm_maps() -> Vec<u32> {
    let gd = game_data::testing::tables();
    let mut m: Vec<u32> = gd.matches.modes().iter().filter(|r| r.mode == 1).map(|r| r.map).collect();
    m.sort_unstable();
    m.dedup();
    m
}

#[test]
fn the_queue_picks_map_mode_and_weather_from_the_masks_and_rotates_the_maps() {
    game_data::require_tables!();
    let gd = game_data::testing::tables();
    let maps = tdm_maps();
    let mut w = setup_with(4, 2, None);
    // everybody ticked team deathmatch (bit 1; bits 7..9 have no check box and are always set)
    queue_masks(&mut w, &[(0, 0x382), (1, 0x382)]);
    queue_masks(&mut w, &[(2, 0x382), (3, 0x382)]);
    let created = w.fake.created.lock().unwrap().clone();
    assert_eq!(created.len(), 2, "two matches of two players");
    // the first map is drawn among the never played ones (a fresh queue); the second match rotates to another one
    assert_ne!(created[0].0, created[1].0, "the second match rotates to another map");
    for c in &created {
        assert_eq!(c.1, 1, "mode 1 is the one ticked");
        assert!(maps.contains(&c.0), "a tdm map");
        let lowest_variation = gd.matches.modes().iter().filter(|r| r.map == c.0 && r.mode == 1).map(|r| r.variation).min().unwrap();
        assert_eq!(c.2, lowest_variation, "weather = the row's lowest variation");
        assert!(gd.matches.resolve_map(c.0, c.1, c.2).is_some(), "a real row of the data");
    }
}

/// The `AC` sign-in push offers every check-box mode the match server can play (the client shows the others as "The
/// mode is unavailable"): with every (map, mode) served that is br, tdm, ctp, the gun race and
/// slaughter (the data's queue modes; ah has no check box and is never offered); with only tdm and ah served, tdm alone;
/// the test override offers its mode.
#[test]
fn the_play_gate_offers_every_mode_the_match_server_serves() {
    game_data::require_tables!();
    let gate = |m: &[S2c]| m.iter().find_map(|x| if let S2c::PlayGate(g) = x { Some(g.mode_mask) } else { None });
    let mut w = setup_with(0, 2, None);
    let mut c = w.e.client("gate1");
    assert_eq!(gate(&w.e.sign_in(&mut c)), Some(0b100_1111));
    *w.fake.modes.lock().unwrap() = Some(vec![1, 9]);
    let mut c = w.e.client("gate2");
    assert_eq!(gate(&w.e.sign_in(&mut c)), Some(0b10));
    let mut w = setup_with(0, 2, Some((5, 2, 0)));
    let mut c = w.e.client("gate3");
    assert_eq!(gate(&w.e.sign_in(&mut c)), Some(1 << 2));
}

/// Players who ticked several modes get the one played longest ago (equally old ones drawn); Rudnya battery (disabled in 0.69d, OFFICIAL
/// 2021-11-17) is never picked; nothing ticked (only the no-button bits: ah) waits instead of playing ah.
#[test]
fn the_queue_takes_the_ticked_modes_in_turn_and_never_plays_ah_or_rudnya_battery() {
    game_data::require_tables!();
    let mut w = setup_with(6, 2, None);
    // battery and tdm ticked: one of them first (both never played: drawn), then the other one; battery never on
    // Rudnya (out of the pool)
    queue_masks(&mut w, &[(0, 0x383), (1, 0x383)]);
    // the same masks again: tdm now (battery was played last)
    queue_masks(&mut w, &[(2, 0x383), (3, 0x383)]);
    let created: Vec<(u32, u32)> = w.fake.created.lock().unwrap().iter().map(|c| (c.0, c.1)).collect();
    // (the fake gives every match the same id, so the lobby refuses the second one and its tickets retry later: only
    // the first two creates are the picks of the two groups)
    let mut modes = [created[0].1, created[1].1];
    modes.sort_unstable();
    assert_eq!(modes, [0, 1], "both ticked modes in turn: {created:?}");
    // nothing ticked: ah is no queue mode, these two never form a match of their own
    let n = created.len();
    queue_masks(&mut w, &[(4, 0x380), (5, 0x380)]);
    let created = w.fake.created.lock().unwrap().clone();
    assert!(created[n..].iter().all(|c| c.1 == 1 && !c.3.iter().any(|s| s.0 == w.clients[4].account || s.0 == w.clients[5].account)), "{created:?}");
    assert!(created.iter().all(|c| c.1 != 9 && (c.0, c.1) != (1, 0)), "no ah, no Rudnya battery: {created:?}");
    for i in [4, 5] {
        assert!(w.e.server.registry.queue.ticket_of(w.clients[i].account).is_some(), "client {i} still searches");
    }
}

#[test]
fn players_with_no_mode_in_common_keep_searching_and_match_those_who_share_one() {
    game_data::require_tables!();
    let mut w = setup_with(3, 2, None);
    // only team deathmatch (1) and capture points (2) have a world
    *w.fake.modes.lock().unwrap() = Some(vec![1, 2]);
    // client 0 ticked capture points, client 1 team deathmatch: they share only the no-button bits, which have no world
    queue_masks(&mut w, &[(0, 0x384), (1, 0x382)]);
    assert!(w.fake.created.lock().unwrap().is_empty());
    for c in w.clients.iter_mut() {
        let m = c.take();
        assert!(connect_msg(&m).is_none());
        assert!(!m.contains(&S2c::QueueState(QueueState { state: 0 })), "waiting, not refused: {m:?}");
    }
    assert_eq!(w.e.server.registry.queue.len(), 2);
    // client 2 also ticked team deathmatch: clients 1 and 2 play, client 0 still waits
    queue_masks(&mut w, &[(2, 0x382)]);
    let created = w.fake.created.lock().unwrap().clone();
    assert_eq!(created.len(), 1);
    assert_eq!(created[0].1, 1);
    let mut accounts: Vec<i64> = created[0].3.iter().map(|s| s.0).collect();
    accounts.sort_unstable();
    let mut expect = vec![w.clients[1].account, w.clients[2].account];
    expect.sort_unstable();
    assert_eq!(accounts, expect);
    assert!(connect_msg(&w.clients[0].take()).is_none());
    assert!(w.e.server.registry.queue.ticket_of(w.clients[0].account).is_some(), "client 0 is still in the queue");
}

#[test]
fn a_mask_without_a_playable_mode_waits_and_no_world_at_all_refuses() {
    game_data::require_tables!();
    // the event bit alone (paintball, mode 4) has no row in the data: the players wait
    let mut w = setup_with(2, 2, None);
    queue_masks(&mut w, &[(0, 0x10), (1, 0x10)]);
    assert!(w.fake.created.lock().unwrap().is_empty());
    for c in w.clients.iter_mut() {
        assert!(!c.take().contains(&S2c::QueueState(QueueState { state: 0 })));
    }
    assert_eq!(w.e.server.registry.queue.len(), 2);
    // a player cancels (31): his ticket leaves, the other keeps waiting
    let World { e, clients, .. } = &mut w;
    clients[0].send(Request::QueueCancel { order: 1 });
    pump_all(e, clients);
    assert_eq!(w.e.server.registry.queue.len(), 1);
}

#[test]
fn per_match_boosters_are_charged_when_the_online_match_starts() {
    game_data::require_tables!();
    let gd = game_data::testing::tables();
    let (map, mode, weather, _) = smallest_row();
    let cost0 = gd.vouchers.match_cost(lobby_server::online::ONLINE_MATCH_LEVEL, 0).unwrap();
    let set = |w: &World, i: usize, silver: u32| {
        w.e.store
            .update_profile(w.clients[i].account, |p| {
                p.silver = silver;
                p.boosters.entry(339).or_default().enabled = true;
                Ok::<(), ()>(())
            })
            .unwrap();
    };
    // the match server refuses: nothing is charged, the booster stays on
    let mut w = setup_with(2, 2, Some((map, mode, weather)));
    set(&w, 0, cost0 + 1000);
    *w.fake.fail.lock().unwrap() = Some("no room".into());
    queue_all(&mut w);
    assert_eq!(silver(&w, 0), cost0 + 1000, "a refused match costs nothing");
    assert!(w.e.store.load_profile(w.clients[0].account).unwrap().boosters[&339].enabled);

    // the match starts: client 0 pays the first booster slot, client 1 cannot pay and starts without the booster
    let mut w = setup_with(2, 2, Some((map, mode, weather)));
    set(&w, 0, cost0 + 1000);
    set(&w, 1, 10);
    queue_all(&mut w);
    assert_eq!(w.fake.created.lock().unwrap().len(), 1);
    let carried = w.fake.boosters.lock().unwrap().clone();
    let of = |i: usize| carried.iter().find(|c| c.0 == w.clients[i].account).unwrap().1.clone();
    assert_eq!(of(0), vec![339], "the options carry the paid booster");
    assert!(of(1).is_empty(), "an unaffordable booster is not in the options");
    assert_eq!(silver(&w, 0), 1000, "the data's silver cost of the first booster was taken");
    assert_eq!(silver(&w, 1), 10);
    assert!(!w.e.store.load_profile(w.clients[1].account).unwrap().boosters[&339].enabled, "switched off like a local match does");
    // the client is told: 82, the new money (8B) and the booster records (94)
    let ids0: Vec<u8> = w.clients[0].take().iter().map(|m| m.id()).collect();
    assert!(ids0.contains(&0x82) && ids0.contains(&0x8B) && ids0.contains(&0x94), "{ids0:02x?}");
    // stock is used before silver
    let mut w = setup_with(2, 2, Some((map, mode, weather)));
    set(&w, 0, 5);
    let acc = w.clients[0].account;
    w.e.store.update_profile(acc, |p| { p.add_item(339, 1); Ok::<(), ()>(()) }).unwrap();
    queue_all(&mut w);
    assert_eq!(silver(&w, 0), 5, "the stock unit paid");
    assert!(w.e.store.load_profile(acc).unwrap().items.iter().all(|i| i.dict_id != 339), "the stock row is gone");
    assert!(w.clients[0].take().iter().any(|m| matches!(m, S2c::Reply(Reply::Inventory(Inventory45::Used { .. })))));
}

#[test]
fn a_pinned_queue_world_waits_for_the_online_group_instead_of_refusing_it() {
    game_data::require_tables!();
    // the operator wants more players than the smallest PvP row: the searches must not be refused at the row's minimum
    // (the fallback for "no match server"), they wait for the online group
    let (map, mode, weather, n) = smallest_row();
    let mut w = setup_with(n, n as u32 + 2, Some((map, mode, weather)));
    queue_all(&mut w);
    assert!(w.fake.created.lock().unwrap().is_empty(), "not enough players for the online group yet");
    for (i, c) in w.clients.iter_mut().enumerate() {
        assert!(!c.take().contains(&S2c::QueueState(QueueState { state: 0 })), "client {i} was refused while a match server waits");
    }
    assert_eq!(w.e.server.registry.queue.len(), n, "every search goes on");
}

#[test]
fn a_rated_match_with_real_statistics_moves_the_league_rating() {
    game_data::require_tables!();
    // the ranked rating needs the place in the team by score (MatchFacts::team_place, league::record); an online match
    // whose server delivered real scores knows it
    let (map, mode, weather, n) = smallest_row();
    let mut w = setup(n, Some((map, mode, weather)));
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
    w.e.server.lobby.cfg.league =
        game_data::LeagueSchedule { seasons: vec![game_data::LeagueSeason { number: 1, start: now - 100, end: now + 3600 }], tokens: vec![] };
    {
        let World { e, clients, .. } = &mut w;
        for i in 0..clients.len() {
            clients[i].send(Request::Queue { global: 0, arg: 2, mode: 0x380, flag: 0 });
            pump_all(e, clients);
        }
    }
    let seats = w.fake.created.lock().unwrap()[0].3.clone();
    // a ranked match's options carry match type 3: no killcam, no hit / kill markers
    assert_eq!(*w.fake.types.lock().unwrap(), vec![protocol::s2c::online::MATCH_TYPE_RANKED]);
    for c in w.clients.iter_mut() {
        c.take();
    }
    let team_of = |a: i64| seats.iter().find(|s| s.0 == a).unwrap().1;
    for i in 0..n {
        let won = team_of(w.clients[i].account) == 0;
        report(&mut w, i, if won { 5 } else { 6 }, MATCH_ID);
    }
    let players = w.clients.iter().map(|c| EndedPlayer { account: c.account, team: team_of(c.account), entered: true, left_by_menu: false }).collect();
    let stats: Vec<lobby_server::EndedStat> =
        w.clients.iter().enumerate().map(|(i, c)| lobby_server::EndedStat { account: c.account, kills: i as u16, deaths: 1, score: 100 * i as u16, ..Default::default() }).collect();
    w.fake.ended.lock().unwrap().push(Ended { match_id: MATCH_ID, duration: Duration::from_secs(600), players, stats: Some((stats, [10, 5])), winner: None });
    let World { e, clients, .. } = &mut w;
    pump_all(e, clients);
    assert!(w.e.server.registry.online.is_empty());
    let gd = game_data::testing::tables();
    let need = gd.league.league(1).unwrap().points_for_levelup;
    for c in w.clients.iter_mut() {
        let st = w.e.store.load_profile(c.account).unwrap().league;
        assert_eq!((st.season, st.matches), (1, 1), "account {} (team {}): the ranked result counted", c.account, team_of(c.account));
        // the results screen's league row: a rating match (b3 = 3), league 1 from 0 points, the bar of league 1, the points
        let m = c.take();
        let Some(S2c::MatchResult(r)) = m.iter().find(|x| x.id() == 0x97) else { panic!("97: {m:?}") };
        assert_eq!(r.b3, 3, "rating match");
        assert_eq!((r.full.f928, r.full.stars_progress, u32::from(r.full.stars)), (0, 0, need));
        let d = i32::from(r.full.stars_delta);
        assert_eq!(st.points as i32, d.max(0), "the row shows what the rating applied");
    }
}

/// With a world host the match server's end carries the world's winner byte and counters; online clients send no
/// `53` reports, so the lobby pays from the end alone: the `97` shows the winner, each player's kills / deaths / score and
/// the team scores (`S+0xEBDC` / `+0xEBDE`), the place by score. Winner byte 2 is a draw (nobody won, `97` winner 2);
/// 3 (none) leaves the verdict to the reports.
#[test]
fn a_world_host_pvp_end_pays_without_reports_and_shows_its_numbers() {
    game_data::require_tables!();
    for (winner, want97) in [(1u8, 1u8), (2, 2)] {
        let (map, mode, weather, n) = smallest_row();
        let mut w = setup(n, Some((map, mode, weather)));
        queue_all(&mut w);
        let seats = w.fake.created.lock().unwrap()[0].3.clone();
        for c in w.clients.iter_mut() {
            c.take();
        }
        let team_of = |a: i64| seats.iter().find(|s| s.0 == a).unwrap().1;
        let players = w.clients.iter().map(|c| EndedPlayer { account: c.account, team: team_of(c.account), entered: true, left_by_menu: false }).collect();
        let stats: Vec<lobby_server::EndedStat> =
            w.clients.iter().enumerate().map(|(i, c)| lobby_server::EndedStat { account: c.account, kills: 3 + i as u16, deaths: 2, score: 50 + 10 * i as u16, ..Default::default() }).collect();
        w.fake.ended.lock().unwrap().push(Ended { match_id: MATCH_ID, duration: Duration::from_secs(600), players, stats: Some((stats, [12, 30])), winner: Some(winner) });
        let World { e, clients, .. } = &mut w;
        pump_all(e, clients);
        assert!(w.e.server.registry.online.is_empty(), "paid at once: the winner came with the end");
        for (i, c) in w.clients.iter_mut().enumerate() {
            let m = c.take();
            let Some(S2c::MatchResult(r)) = m.iter().find(|x| x.id() == 0x97) else { panic!("97: {m:?}") };
            assert_eq!((r.winner_team, r.team0_score, r.team1_score), (want97, 12, 30));
            let me = r.players.iter().find(|p| p.id == public_id(c.account)).unwrap();
            assert_eq!((me.kills, me.deaths, me.score), (3 + i as u16, 2, 50 + 10 * i as i16));
            assert!(m.iter().any(|x| x.id() == 0x8B), "paid without a report");
            assert_eq!(m.last().map(|x| x.id()), Some(0x85));
        }
        // PvP end screen: PVP_END_AFTER_RESULT after the 83 45 0A + 97 the lobby asks the match server to close the match
        // after its result (code 2 when it handed the result over in the match), once
        assert!(w.fake.after.lock().unwrap().is_empty(), "not before the margin");
        let World { e, clients, .. } = &mut w;
        for _ in 0..3 {
            pump_all(e, clients);
        }
        assert_eq!(*w.fake.after.lock().unwrap(), vec![MATCH_ID]);
        assert!(w.fake.ends.lock().unwrap().is_empty(), "no plain end");
    }
}

/// The after-match charge (charge.rs): "Match costs" = the equipment use of the player's
/// equipment level (default the patch 0.55 prices: level 3 = 2000 silver), one entry of dict 0xFFFF; the weapons'
/// per-item `match_participation_cost` is not charged; the pocket units of the world host's `Used` row (protocol 6) cost
/// the shop price in "Special items", rounds are free ("Ammunition" empty); the silver of the reward stays gross in the
/// `97` (`f14`) and the charge is taken after it (`8B`); a level-1 player without a `Used` row pays nothing.
#[test]
fn the_world_hosts_used_rows_are_charged_into_the_97_tail() {
    game_data::require_tables!();
    use lobby_server::charge::{MatchUse, StackUse, EQUIPMENT_USE_DICT};
    use protocol::s2c::{Spent, SPENT_AMMO, SPENT_POCKETS, SPENT_REPAIR};
    let (map, mode, weather, n) = smallest_row();
    let mut w = setup(n, Some((map, mode, weather)));
    // player 0 wears edge_torso_4 (117, item level 3) in the torso slot: equipment level 3
    let gd = game_data::testing::tables();
    w.e.store
        .update_profile(w.clients[0].account, |p| {
            let uid = p.add_item(117, gd.items.amount_field(117, 1));
            p.slots.insert(2, uid);
            p.silver += 10_000;
            Ok::<_, ()>(())
        })
        .unwrap();
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
    let level = |w: &World, i: usize| lobby_server::items::equipment_level(&gd, &w.e.store.load_profile(w.clients[i].account).unwrap(), now);
    assert_eq!(level(&w, 0), 3);
    assert!(level(&w, 1) <= 1, "the starter kit: level {}", level(&w, 1));
    queue_all(&mut w);
    let seats = w.fake.created.lock().unwrap()[0].3.clone();
    for c in w.clients.iter_mut() {
        c.take();
    }
    let before: Vec<u32> = (0..n).map(|i| silver(&w, i)).collect();
    let xp0 = w.e.store.load_profile(w.clients[0].account).unwrap().xp;
    let xp1 = w.e.store.load_profile(w.clients[1].account).unwrap().xp;
    let (_, medkit) = gd.shop.pocket_cost(760, 1);
    assert!(medkit > 0);
    let team_of = |a: i64| seats.iter().find(|s| s.0 == a).unwrap().1;
    let players = w.clients.iter().map(|c| EndedPlayer { account: c.account, team: team_of(c.account), entered: true, left_by_menu: false }).collect();
    // player 0: ak_74n (match_participation_cost 1000 in game.options, not charged) + akmn, 2 science medkits and 300
    // rounds used; player 1: no row (None)
    let used0 = MatchUse {
        profiles: 0b11,
        weapons: vec![306, 1201],
        stacks: vec![StackUse { uid: 100_028, dict: 760, used: 2 }, StackUse { uid: 100_014, dict: 81, used: 300 }],
    };
    let stats: Vec<lobby_server::EndedStat> = w
        .clients
        .iter()
        .enumerate()
        .map(|(i, c)| lobby_server::EndedStat { account: c.account, kills: 1, deaths: 1, score: 10, used: (i == 0).then(|| used0.clone()), ..Default::default() })
        .collect();
    let winner = team_of(w.clients[0].account);
    w.fake.ended.lock().unwrap().push(Ended { match_id: MATCH_ID, duration: Duration::from_secs(600), players, stats: Some((stats, [5, 2])), winner: Some(winner) });
    let World { e, clients, .. } = &mut w;
    pump_all(e, clients);
    let m0 = w.clients[0].take();
    let Some(S2c::MatchResult(r)) = m0.iter().find(|x| x.id() == 0x97) else { panic!("97: {m0:?}") };
    assert_eq!(r.spent[SPENT_REPAIR], vec![Spent { money: 2000, dict: EQUIPMENT_USE_DICT }], "level 3, no weapon entry");
    assert_eq!(r.spent[SPENT_POCKETS], vec![Spent { money: (2 * medkit) as u32, dict: 760 }]);
    assert!(r.spent[SPENT_AMMO].is_empty());
    let charged = 2000 + 2 * medkit;
    // f14 = the gross reward; the account ends with reward - charge (+ the silver of a character level reached, paid
    // apart: progress::add_xp); the last 8B shows it
    let level_silver = gd.levels.level_up_silver(xp0, w.e.store.load_profile(w.clients[0].account).unwrap().xp);
    assert_eq!(u64::from(silver(&w, 0)), u64::from(before[0]) + u64::from(r.full.f14) + u64::from(level_silver) - charged);
    let at97 = m0.iter().position(|x| x.id() == 0x97).unwrap();
    let last8b = m0.iter().rposition(|x| x.id() == 0x8B).unwrap();
    assert!(last8b > at97);
    // player 1: no Used row, nothing charged, empty lists
    let m1 = w.clients[1].take();
    let Some(S2c::MatchResult(r1)) = m1.iter().find(|x| x.id() == 0x97) else { panic!("97: {m1:?}") };
    assert!(r1.spent.iter().all(|l| l.is_empty()));
    let level_silver1 = gd.levels.level_up_silver(xp1, w.e.store.load_profile(w.clients[1].account).unwrap().xp);
    assert_eq!(silver(&w, 1), before[1] + r1.full.f14 + level_silver1);
}

/// The reward level is each player's OWN equipment level (OFFICIAL 0.69b: only the Guns Race rewards do not depend on
/// the equipment tier): in one match a player in level-5 armour gets the
/// level-5 factors and drop roll (silver 500..1000) and a level-1 player the level-1 ones (100..200); each 97 carries
/// the receiver's own level (header byte and R +0x2E of his profile); the level-5 player pays the level-5 repairs.
#[test]
fn each_player_is_paid_at_his_own_equipment_level() {
    game_data::require_tables!();
    use protocol::s2c::{Spent, SPENT_REPAIR};
    let (map, mode, weather, n) = smallest_row();
    let mut w = setup(n, Some((map, mode, weather)));
    let gd = game_data::testing::tables();
    // player 0: edge_torso_9 (196, item level 5) in the torso slot, silver for the repairs; the others the starter kit
    w.e.store
        .update_profile(w.clients[0].account, |p| {
            let uid = p.add_item(196, gd.items.amount_field(196, 1));
            p.slots.insert(2, uid);
            p.silver += 20_000;
            Ok::<_, ()>(())
        })
        .unwrap();
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
    let level = |w: &World, i: usize| lobby_server::items::equipment_level(&gd, &w.e.store.load_profile(w.clients[i].account).unwrap(), now);
    assert_eq!(level(&w, 0), 5);
    assert!(level(&w, 1) <= 1);
    queue_all(&mut w);
    let seats = w.fake.created.lock().unwrap()[0].3.clone();
    for c in w.clients.iter_mut() {
        c.take();
    }
    for i in 0..n {
        let team = seats.iter().find(|s| s.0 == w.clients[i].account).unwrap().1;
        report(&mut w, i, if team == 0 { 5 } else { 6 }, MATCH_ID);
    }
    // no event counts (no world data): the silver is the drop_after_combat roll of each player's own level
    let players = w.clients.iter().map(|c| EndedPlayer { account: c.account, team: seats.iter().find(|s| s.0 == c.account).unwrap().1, entered: true, left_by_menu: false }).collect();
    w.fake.ended.lock().unwrap().push(Ended { match_id: MATCH_ID, duration: Duration::from_secs(900), players, stats: None, winner: None });
    let World { e, clients, .. } = &mut w;
    pump_all(e, clients);
    let r_of = |m: &[S2c]| match m.iter().find(|x| x.id() == 0x97) {
        Some(S2c::MatchResult(r)) => r.clone(),
        _ => panic!("97: {m:?}"),
    };
    let r0 = r_of(&w.clients[0].take());
    let r1 = r_of(&w.clients[1].take());
    let p = 0usize; // the current equipment profile of both
    assert_eq!((r0.match_level, r0.full.bytes[p]), (5, 5), "player 0: his own level");
    assert_eq!((r1.match_level, r1.full.bytes[p]), (1, 1), "player 1: his own level, not the match's highest");
    // the drop silver roll of the level (the worn items may add their silver percent on top)
    let rows_silver = |r: &MatchResult| r.full.awards.iter().map(|a| a.extras[p].0).sum::<u32>();
    assert!((500..=1000).contains(&rows_silver(&r0)), "level 5 roll: {}", rows_silver(&r0));
    assert!((100..=200).contains(&rows_silver(&r1)), "level 1 roll: {}", rows_silver(&r1));
    // the repairs follow the same number
    assert_eq!(r0.spent[SPENT_REPAIR], vec![Spent { money: 8000, dict: lobby_server::charge::EQUIPMENT_USE_DICT }]);
    assert!(r1.spent[SPENT_REPAIR].is_empty());
}

/// The events' silver of an online match uses the setting `match_silver_factors` at the player's reward level (default
/// level 5 = 14 instead of the data's 4); experience follows the data (x1.45 at level 5), the drop roll is not scaled.
#[test]
fn the_events_silver_uses_the_silver_factor_setting_of_the_players_level() {
    game_data::require_tables!();
    use lobby_server::match_data::{RosterSlot, WorldData};
    let (map, mode, weather, n) = smallest_row();
    let mut w = setup(n, Some((map, mode, weather)));
    let gd = game_data::testing::tables();
    // player 0 in edge_torso_9 (item level 5)
    w.e.store
        .update_profile(w.clients[0].account, |p| {
            let uid = p.add_item(196, gd.items.amount_field(196, 1));
            p.slots.insert(2, uid);
            Ok::<_, ()>(())
        })
        .unwrap();
    queue_all(&mut w);
    let seats = w.fake.created.lock().unwrap()[0].3.clone();
    for c in w.clients.iter_mut() {
        c.take();
    }
    let team_of = |a: i64| seats.iter().find(|s| s.0 == a).unwrap().1;
    let roster = w.clients.iter().enumerate().map(|(i, c)| RosterSlot { slot: i as u8, account: Some(c.account), team: team_of(c.account), entered: true }).collect();
    let mut d0 = world_host_proto::PlayerDetail { slot: 0, ..Default::default() };
    (d0.events[0], d0.events[15]) = (4, 2); // 4 kills (tdm 45 / 9 exp), 2 headshots (15 / 3 exp)
    let world = Arc::new(WorldData { roster, details: vec![d0], kills: None, bots: vec![] });
    let players = w.clients.iter().map(|c| EndedPlayer { account: c.account, team: team_of(c.account), entered: true, left_by_menu: false }).collect();
    let stats: Vec<lobby_server::EndedStat> =
        w.clients.iter().map(|c| lobby_server::EndedStat { account: c.account, kills: 0, deaths: 0, score: 0, world: Some(world.clone()), used: None }).collect();
    let winner = team_of(w.clients[0].account);
    w.fake.ended.lock().unwrap().push(Ended { match_id: MATCH_ID, duration: Duration::from_secs(900), players, stats: Some((stats, [5, 2])), winner: Some(winner) });
    let World { e, clients, .. } = &mut w;
    pump_all(e, clients);
    let m0 = w.clients[0].take();
    let Some(S2c::MatchResult(r)) = m0.iter().find(|x| x.id() == 0x97) else { panic!("97: {m0:?}") };
    assert_eq!(r.match_level, 5);
    let rows_silver: u32 = r.full.awards.iter().map(|a| a.extras[0].0).sum();
    let rows_exp: u32 = r.full.awards.iter().map(|a| u32::from(a.extras[0].1)).sum();
    // the same events through the formula of this mode: silver at factor 14 (the data has 4), experience at the data's
    // level-5 factor; the level-5 drop roll 500..1000 is not scaled
    let mut f = game_data::MatchFacts::local(map, mode, true, true, 1);
    f.stat_events = Some([(0u16, 4u32), (15, 2)].into_iter().collect());
    f.first_game = true; // a new account: played_first_game (500 experience) pays with this match
    let (with, wp) = gd.match_reward_parts_with(&f, 5, Some(14.0), &mut game_data::SplitMix(1));
    let (data, dp) = gd.match_reward_parts(&f, 5, &mut game_data::SplitMix(1));
    let events = with.silver - wp.drop_silver;
    assert!(events > data.silver - dp.drop_silver && events > 0);
    assert!(rows_silver >= events + 500 && rows_silver <= events + 1000, "rows silver {rows_silver}, events {events}");
    assert_eq!(rows_exp, with.exp, "experience: the data's level-5 factor");
    assert_eq!(with.exp, data.exp);
}

/// Select the silver booster 339 (`use_variable_costs`: priced by `booster_use_costs[row][0]`) with no stock, 10 000 silver,
/// and optionally one armour item in the torso slot (its item level sets the equipment level).
fn booster_kit(w: &World, i: usize, torso: Option<u16>) {
    let gd = game_data::testing::tables();
    w.e.store
        .update_profile(w.clients[i].account, |p| {
            p.items.retain(|x| x.dict_id != 339);
            p.boosters.entry(339).or_default().enabled = true;
            p.silver = 10_000;
            if let Some(d) = torso {
                let uid = p.add_item(d, gd.items.amount_field(d, 1));
                p.slots.insert(2, uid);
            }
            Ok::<_, ()>(())
        })
        .unwrap();
}

/// The booster start charge follows the client's own price (0x2001BF460): row = the player's booster level (his
/// equipment level, or a squad mate's when higher), the level itself as the row: a solo level-3 player pays
/// `booster_use_costs[3][0]` = 750, a level-1 player row 1 = 250.
#[test]
fn the_booster_start_charge_uses_the_players_own_level_as_the_row() {
    game_data::require_tables!();
    let (map, mode, weather, n) = smallest_row();
    let mut w = setup(n, Some((map, mode, weather)));
    let gd = game_data::testing::tables();
    booster_kit(&w, 0, Some(117)); // edge_torso_4, item level 3
    booster_kit(&w, 1, None); // the starter kit, level 1
    queue_all(&mut w);
    assert_eq!(w.fake.created.lock().unwrap().len(), 1);
    assert_eq!(10_000 - silver(&w, 0), gd.vouchers.match_cost(3, 0).unwrap());
    assert_eq!(gd.vouchers.match_cost(3, 0), Some(750));
    assert_eq!(10_000 - silver(&w, 1), 250, "level 1 = row 1");
}

/// A squad member's higher level raises the booster row of the whole squad (the client's squad-member levels in
/// 0x2001BF460): a level-1 commander with a level-5 mate pays row 5 = 1500.
#[test]
fn a_squad_mates_higher_level_sets_the_booster_row() {
    game_data::require_tables!();
    let (map, mode, weather, n) = smallest_row();
    let mut w = setup(n, Some((map, mode, weather)));
    booster_kit(&w, 0, None); // level 1
    booster_kit(&w, 1, Some(196)); // edge_torso_9, item level 5
    let (a0, a1) = (w.clients[0].account, w.clients[1].account);
    {
        let sq = &mut w.e.server.registry.squads;
        sq.record_invite(a0, a1, 2);
        sq.accept_invite(a0, a1);
        sq.join(a0, a1).unwrap();
        sq.set_ready(a1, true);
    }
    // the commander queues the squad, everyone else on his own
    let rest: Vec<(usize, u32)> = std::iter::once((0, 0x380)).chain((2..n).map(|i| (i, 0x380))).collect();
    queue_masks(&mut w, &rest);
    assert_eq!(w.fake.created.lock().unwrap().len(), 1, "the match started");
    assert_eq!(10_000 - silver(&w, 0), 1500, "row 5 from the squad mate");
    assert_eq!(10_000 - silver(&w, 1), 1500);
}

/// Premium ammo boxes: the world host lists one use per spawned profile whose slot 11 / 12
/// held a box; after the match the stack loses it (`83 45 03 <uid>` per box, no money, nothing in the 97 lists); a stack
/// that runs out leaves slot 11 and the client gets the 87.
#[test]
fn used_ammo_boxes_leave_the_stack_after_the_match() {
    game_data::require_tables!();
    use lobby_server::charge::{MatchUse, StackUse};
    let (map, mode, weather, n) = smallest_row();
    let mut w = setup(n, Some((map, mode, weather)));
    // player 0: 2 boxes in slot 11, player 1: 1 box in slot 11
    let mut boxes = vec![];
    for (i, count) in [(0usize, 2u32), (1, 1)] {
        let uid = w.e.store.update_profile(w.clients[i].account, |p| {
            let uid = p.add_item(1281, count);
            p.slots.insert(11, uid);
            p.slot_rounds.insert(11, 1);
            Ok::<_, ()>(uid)
        }).unwrap();
        boxes.push(uid);
    }
    queue_all(&mut w);
    let seats = w.fake.created.lock().unwrap()[0].3.clone();
    for c in w.clients.iter_mut() {
        c.take();
    }
    let before: Vec<u32> = (0..n).map(|i| silver(&w, i)).collect();
    let team_of = |a: i64| seats.iter().find(|s| s.0 == a).unwrap().1;
    let players = w.clients.iter().map(|c| EndedPlayer { account: c.account, team: team_of(c.account), entered: true, left_by_menu: false }).collect();
    let stats: Vec<lobby_server::EndedStat> = w
        .clients
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let used = (i < 2).then(|| MatchUse { profiles: 1, weapons: vec![1201], stacks: vec![StackUse { uid: boxes[i], dict: 1281, used: 1 }] });
            lobby_server::EndedStat { account: c.account, kills: 1, deaths: 1, score: 10, used, ..Default::default() }
        })
        .collect();
    let winner = team_of(w.clients[0].account);
    w.fake.ended.lock().unwrap().push(Ended { match_id: MATCH_ID, duration: Duration::from_secs(600), players, stats: Some((stats, [5, 2])), winner: Some(winner) });
    let World { e, clients, .. } = &mut w;
    pump_all(e, clients);
    for i in 0..2 {
        let m = w.clients[i].take();
        let used: Vec<u32> = m.iter().filter_map(|x| if let S2c::Reply(Reply::Inventory(Inventory45::Used { item })) = x { Some(*item) } else { None }).collect();
        assert_eq!(used, vec![boxes[i]], "player {i}: one 83 45 03");
        let Some(S2c::MatchResult(r)) = m.iter().find(|x| x.id() == 0x97) else { panic!("97: {m:?}") };
        assert!(r.spent.iter().all(|l| l.is_empty()), "a box costs no money after the match");
        assert_eq!(silver(&w, i), before[i] + r.full.f14);
        let p = w.e.store.load_profile(w.clients[i].account).unwrap();
        if i == 0 {
            assert_eq!((p.item(boxes[0]).unwrap().count, p.slots.get(&11)), (1, Some(&boxes[0])));
        } else {
            assert!(p.item(boxes[1]).is_none() && p.slots.get(&11).is_none(), "the last box: stack and slot gone");
            let a = m.iter().rev().find_map(|x| if let S2c::Account(a) = x { Some(a) } else { None }).expect("87 after the last box");
            assert!(!a.body.profiles[0].slots.contains_key(&11));
        }
    }
}

/// One premium ammo box per box slot per PvP match: a player who spawned with three profiles (the world host lists the
/// boxes of each: two primary-weapon box stacks and a pistol box) loses one primary box and one pistol box; a player who
/// left early with the menu loses his boxes too.
#[test]
fn a_pvp_match_takes_one_ammo_box_also_from_a_leaver() {
    game_data::require_tables!();
    use lobby_server::charge::{MatchUse, StackUse};
    let (map, mode, weather, n) = smallest_row();
    let mut w = setup(n, Some((map, mode, weather)));
    let mut boxes = vec![];
    for i in 0..2usize {
        let uids = w
            .e
            .store
            .update_profile(w.clients[i].account, |p| {
                // two primary-weapon box stacks (category 37) and a pistol box (1293, category 38)
                let a = p.add_item(1281, 5);
                let b = p.add_item(1280, 5);
                let c = p.add_item(1293, 5);
                p.slots.insert(11, a);
                p.slots.insert(12, c);
                Ok::<_, ()>((a, b, c))
            })
            .unwrap();
        boxes.push(uids);
    }
    queue_all(&mut w);
    let seats = w.fake.created.lock().unwrap()[0].3.clone();
    for c in w.clients.iter_mut() {
        c.take();
    }
    let team_of = |a: i64| seats.iter().find(|s| s.0 == a).unwrap().1;
    // player 1 left with the menu
    let players = w.clients.iter().enumerate().map(|(i, c)| EndedPlayer { account: c.account, team: team_of(c.account), entered: true, left_by_menu: i == 1 }).collect();
    let stats: Vec<lobby_server::EndedStat> = w
        .clients
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let used = (i < 2).then(|| MatchUse {
                profiles: 0b111,
                weapons: vec![1201],
                stacks: vec![
                    StackUse { uid: boxes[i].0, dict: 1281, used: 2 },
                    StackUse { uid: boxes[i].1, dict: 1280, used: 1 },
                    StackUse { uid: boxes[i].2, dict: 1293, used: 3 },
                ],
            });
            lobby_server::EndedStat { account: c.account, kills: 1, deaths: 1, score: 10, used, ..Default::default() }
        })
        .collect();
    let winner = team_of(w.clients[0].account);
    w.fake.ended.lock().unwrap().push(Ended { match_id: MATCH_ID, duration: Duration::from_secs(600), players, stats: Some((stats, [5, 2])), winner: Some(winner) });
    let World { e, clients, .. } = &mut w;
    pump_all(e, clients);
    for i in 0..2 {
        let m = w.clients[i].take();
        let used: Vec<u32> = m.iter().filter_map(|x| if let S2c::Reply(Reply::Inventory(Inventory45::Used { item })) = x { Some(*item) } else { None }).collect();
        assert_eq!(used, vec![boxes[i].0, boxes[i].2], "player {i}: one 83 45 03 per box slot");
        let p = w.e.store.load_profile(w.clients[i].account).unwrap();
        let counts = [boxes[i].0, boxes[i].1, boxes[i].2].map(|u| p.item(u).unwrap().count);
        assert_eq!(counts, [4, 5, 4], "player {i}");
    }
}

/// PvP end screen: while the match server still runs the clients after their result, the `85 00` waits (an
/// `85 00` with the match world loaded unloads it: no victory splash); it goes out once the match server let go of the
/// match. The 97 and the pay go out at once.
#[test]
fn the_end_screens_85_00_waits_until_the_match_server_let_go() {
    game_data::require_tables!();
    let (map, mode, weather, n) = smallest_row();
    let mut w = setup(n, Some((map, mode, weather)));
    queue_all(&mut w);
    let seats = w.fake.created.lock().unwrap()[0].3.clone();
    for c in w.clients.iter_mut() {
        c.take();
    }
    *w.fake.holds.lock().unwrap() = true;
    *w.fake.open.lock().unwrap() = true;
    let team_of = |a: i64| seats.iter().find(|s| s.0 == a).unwrap().1;
    let players = w.clients.iter().map(|c| EndedPlayer { account: c.account, team: team_of(c.account), entered: true, left_by_menu: false }).collect();
    w.fake.ended.lock().unwrap().push(Ended { match_id: MATCH_ID, duration: Duration::from_secs(600), players, stats: None, winner: Some(1) });
    let World { e, clients, .. } = &mut w;
    pump_all(e, clients);
    for c in w.clients.iter_mut() {
        let ids: Vec<u8> = c.take().iter().map(|x| x.id()).collect();
        assert!(ids.contains(&0x97) && ids.contains(&0x8B), "the result at once: {ids:02x?}");
        assert!(!ids.contains(&0x85), "no 85 00 while the match server holds the clients: {ids:02x?}");
    }
    // the close after the result goes out; the match server still drains the connections: still no 85 00
    let World { e, clients, .. } = &mut w;
    for _ in 0..3 {
        pump_all(e, clients);
    }
    assert_eq!(*w.fake.after.lock().unwrap(), vec![MATCH_ID]);
    assert!(w.clients.iter_mut().all(|c| !c.take().iter().any(|x| x.id() == 0x85)));
    // the match server let go of the match: 85 00
    *w.fake.open.lock().unwrap() = false;
    let World { e, clients, .. } = &mut w;
    pump_all(e, clients);
    for c in w.clients.iter_mut() {
        let ids: Vec<u8> = c.take().iter().map(|x| x.id()).collect();
        assert!(ids.contains(&0x85), "85 00 after the end screen: {ids:02x?}");
    }
}

/// World host protocol 3: the world's own data (event row, best streak, badges, inactive flag) drives the achievements and
/// the badge awards, and the `97` shows the receiver's badges and the medal holders; an inactive player gets no reward
/// and no drops, the screen says why (`83 45 0A` flag 1 before the `97`, `has_inactive_fine`).
#[test]
fn the_worlds_rows_count_achievements_pay_badges_and_withhold_an_inactive_reward() {
    game_data::require_tables!();
    use lobby_server::match_data::{RosterSlot, WorldData};
    let (map, mode, weather, n) = smallest_row();
    let mut w = setup(n, Some((map, mode, weather)));
    queue_all(&mut w);
    let seats = w.fake.created.lock().unwrap()[0].3.clone();
    // random queue matches carry match type 1 (the world's inactivity tracking)
    assert_eq!(*w.fake.types.lock().unwrap(), vec![protocol::s2c::online::MATCH_TYPE_RANDOM]);
    for c in w.clients.iter_mut() {
        c.take();
    }
    let before: Vec<u32> = (0..n).map(|i| silver(&w, i)).collect();
    let team_of = |a: i64| seats.iter().find(|s| s.0 == a).unwrap().1;
    let roster = w.clients.iter().enumerate().map(|(i, c)| RosterSlot { slot: i as u8, account: Some(c.account), team: team_of(c.account), entered: true }).collect();
    let mut d0 = world_host_proto::PlayerDetail { slot: 0, best_streak: 4, ..Default::default() };
    d0.events[0] = 4; // kills
    d0.events[15] = 2; // headshots
    d0.badges[5] = [1, 0, 0]; // kill badge state 1
    d0.badges[23] = [1, 0, 0]; // best player
    let d1 = world_host_proto::PlayerDetail { slot: 1, inactive: true, ..Default::default() };
    let world = Arc::new(WorldData { roster, details: vec![d0, d1], kills: None, bots: vec![] });
    let players = w.clients.iter().map(|c| EndedPlayer { account: c.account, team: team_of(c.account), entered: true, left_by_menu: false }).collect();
    let stats: Vec<lobby_server::EndedStat> = w
        .clients
        .iter()
        .enumerate()
        .map(|(i, c)| lobby_server::EndedStat { account: c.account, kills: if i == 0 { 4 } else { 0 }, deaths: 1, score: 40, world: Some(world.clone()), used: None })
        .collect();
    let winner = team_of(w.clients[0].account);
    w.fake.ended.lock().unwrap().push(Ended { match_id: MATCH_ID, duration: Duration::from_secs(900), players, stats: Some((stats, [5, 2])), winner: Some(winner) });
    let World { e, clients, .. } = &mut w;
    pump_all(e, clients);
    let gd = game_data::testing::tables();
    // player 0: the rows count (kills 201, headshots 203, killstreak 214), the 97 shows his badges and his medal
    let p0 = w.e.store.load_profile(w.clients[0].account).unwrap();
    let ach = |id: u16| p0.ach.get(&id).map_or(0, |x| x.value);
    assert_eq!((ach(201), ach(203), ach(214)), (4, 2, 4));
    let name0 = w.e.store.account(w.clients[0].account).unwrap().unwrap().name;
    let m0 = w.clients[0].take();
    let Some(S2c::MatchResult(r)) = m0.iter().find(|x| x.id() == 0x97) else { panic!("97: {m0:?}") };
    assert_eq!(r.full.kinds, vec![(5, [1, 0, 0]), (23, [1, 0, 0])]);
    assert_eq!((r.full.counts[0], r.full.bytes[0]), (900, r.match_level));
    assert_eq!(r.texts[0], name0, "the best player medal in the header");
    assert!(r.texts[1..].iter().all(|t| t.is_empty()));
    assert_eq!(r.full.flag92e, 0);
    // the badge award is in the totals (f14 = every credited silver) and not in the game-mode component
    let mut counts = [[0u8; 3]; 27];
    counts[5] = [1, 0, 0];
    counts[23] = [1, 0, 0];
    let badge = gd.badges.reward(&counts, &gd.match_level_factors, u32::from(r.match_level), mode);
    assert!(badge.silver > 0);
    assert_eq!(r.full.f14, silver(&w, 0) - before[0]);
    // the window's total: the three reward rows + the bonus blocks but the premium one + the client's own badge row =
    // every credited silver; the undrawn game-mode block stays 0
    let rows: u32 = r.full.awards.iter().map(|a| a.extras[0].0).sum();
    let blocks: u32 = r.full.score.iter().enumerate().filter(|(k, _)| *k != 2).map(|(_, b)| b[0].money).sum();
    assert_eq!(r.full.score[10][0], Default::default());
    assert_eq!(rows + blocks + badge.silver, r.full.f14);
    // no premium: the premium row (block 2) shows what premium would have added, +50 % of the base on all three;
    // it is not in the pay above
    let base = |f: fn(&Award) -> u32| r.full.awards.iter().map(f).sum::<u32>();
    let would = [base(|a| u32::from(a.extras[0].1)), base(|a| a.extras[0].0), base(|a| u32::from(a.extras[0].2))].map(|v| v / 2);
    let prem = r.full.score[2][0];
    assert!(would.iter().all(|v| *v > 0), "a won match pays experience, silver and reputation: {would:?}");
    assert_eq!([u32::from(prem.exp), prem.money, u32::from(prem.rep)], would);
    // the kills row shows the 4 kills (the headshots pay in the third row)
    assert_eq!((r.full.awards[0].flag, r.full.awards[0].values[0]), (1, 4.0));
    // player 1 was inactive: no silver, no drops, the screen's flag and the refused-drop panel first
    assert_eq!(silver(&w, 1), before[1], "no reward");
    let m1 = w.clients[1].take();
    let at = m1.iter().position(|x| matches!(x, S2c::Reply(Reply::Inventory(Inventory45::MatchDrops { flag: 1, .. })))).expect("83 45 0A flag 1");
    let at97 = m1.iter().position(|x| x.id() == 0x97).expect("97");
    assert!(at < at97);
    let S2c::MatchResult(r1) = &m1[at97] else { panic!() };
    assert_eq!((r1.full.flag92e, r1.full.f14), (1, 0));
    assert!(r1.full.kinds.is_empty());
}

/// The result window: a bot-filled match's AI players are rows of the `97` (ids 1.., their
/// team and the host's counters, the name in the language the client gave in `32`), the personal tab's three reward rows
/// carry the client keys and the match's base reward by `ui_category` at the current profile, the bonus blocks the
/// premium share, the undrawn game-mode block is 0, and rows + blocks = the silver / experience / reputation paid.
#[test]
fn the_97_lists_the_bots_and_its_reward_rows_add_up_to_the_pay() {
    game_data::require_tables!();
    use lobby_server::match_data::{BotRow, RosterSlot, WorldData};
    let (map, mode, weather, n) = smallest_row();
    let mut w = setup(n, Some((map, mode, weather)));
    let gd = game_data::testing::tables();
    // player 0: ukrainian client (32 after the 80), premium, current equipment profile 1
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
    w.e.store
        .update_profile(w.clients[0].account, |p| {
            p.premium_until = now + 86_400;
            p.equip.current = 1;
            // profile 3 is bought but switched off: not playable in a match
            p.equip.profiles[2].owned = true;
            p.equip.profiles[2].enabled = false;
            Ok::<_, ()>(())
        })
        .unwrap();
    {
        let World { e, clients, .. } = &mut w;
        clients[0].send(Request::Info32 { a: 5, b: 0 });
        pump_all(e, clients);
    }
    queue_all(&mut w);
    let seats = w.fake.created.lock().unwrap()[0].3.clone();
    for c in w.clients.iter_mut() {
        c.take();
    }
    let before = w.e.store.load_profile(w.clients[0].account).unwrap();
    let team_of = |a: i64| seats.iter().find(|s| s.0 == a).unwrap().1;
    let humans = w.clients.len() as u8;
    let mut roster: Vec<RosterSlot> =
        w.clients.iter().enumerate().map(|(i, c)| RosterSlot { slot: i as u8, account: Some(c.account), team: team_of(c.account), entered: true }).collect();
    roster.push(RosterSlot { slot: humans, account: None, team: 0, entered: true });
    roster.push(RosterSlot { slot: humans + 1, account: None, team: 1, entered: true });
    // player 0: 5 kills, 2 assists, 3 deaths, 1 headshot
    let mut d0 = world_host_proto::PlayerDetail { slot: 0, ..Default::default() };
    (d0.events[0], d0.events[1], d0.events[2], d0.events[15]) = (5, 2, 3, 1);
    let bots = vec![
        BotRow { slot: humans, team: 0, names: Some((23, 17)), ai: Some(3), kills: 7, deaths: 2, score: 105 },
        BotRow { slot: humans + 1, team: 1, names: Some((0, 0)), ai: None, kills: 1, deaths: 9, score: 10 },
    ];
    let world = Arc::new(WorldData { roster, details: vec![d0], kills: None, bots });
    let players = w.clients.iter().map(|c| EndedPlayer { account: c.account, team: team_of(c.account), entered: true, left_by_menu: false }).collect();
    let stats: Vec<lobby_server::EndedStat> = w
        .clients
        .iter()
        .enumerate()
        .map(|(i, c)| lobby_server::EndedStat { account: c.account, kills: if i == 0 { 5 } else { 0 }, deaths: 3, score: 75, world: Some(world.clone()), used: None })
        .collect();
    let winner = team_of(w.clients[0].account);
    w.fake.ended.lock().unwrap().push(Ended { match_id: MATCH_ID, duration: Duration::from_secs(900), players, stats: Some((stats, [5, 2])), winner: Some(winner) });
    let World { e, clients, .. } = &mut w;
    pump_all(e, clients);
    let after = w.e.store.load_profile(w.clients[0].account).unwrap();
    let m0 = w.clients[0].take();
    let Some(S2c::MatchResult(r)) = m0.iter().find(|x| x.id() == 0x97) else { panic!("97: {m0:?}") };
    // the rows: the humans, then the two bots with ids 1, 2, their teams, counters and ukrainian names
    assert_eq!(r.players.len(), w.clients.len() + 2);
    let b: Vec<_> = r.players[w.clients.len()..].iter().map(|p| (p.id, p.team, p.kills, p.deaths, p.score)).collect();
    assert_eq!(b, vec![(1, 0, 7, 2, 105), (2, 1, 1, 9, 10)]);
    assert_eq!(r.players[w.clients.len()].name, gd.bot_names.name(23, 17, 5).unwrap());
    assert_eq!(r.players[w.clients.len()].name, "Вася Топаз", "st_bot_1st_name_24 + st_bot_2nd_name_19 in ukrainian");
    assert!(r.players.iter().all(|p| p.id >= 2 * r.players.len() as u64 + 2 || p.id <= 2), "humans keep their public ids");
    assert_eq!(r.players[w.clients.len()].flag, 0, "the best score of the table: place 0");
    // the personal tab at profile 1: the three rows with the client keys; kills / assists shown, the third without amount
    let p = 1usize;
    assert_eq!(r.full.awards.iter().map(|a| a.name.as_str()).collect::<Vec<_>>(), game_data::result_texts::ROW_LABELS.to_vec());
    assert_eq!(r.full.awards.iter().map(|a| a.flag).collect::<Vec<_>>(), vec![1, 1, 0]);
    assert_eq!((r.full.awards[0].values[p], r.full.awards[1].values[p]), (5.0, 2.0));
    assert!(r.full.awards.iter().all(|a| a.values[0] == 0.0), "nothing at the other profiles");
    assert_eq!((r.full.counts[p], r.full.bytes[p]), (900, r.match_level));
    // premium: block 2 holds the premium share; the game-mode block stays 0
    assert!(r.full.score[2][p].money > 0);
    assert_eq!(r.full.score[10][p], Default::default());
    // rows + blocks = what was paid (no badges in this match)
    let rows = |f: fn(&Award) -> u32| r.full.awards.iter().map(f).sum::<u32>();
    let blocks = |f: fn(&ScoreComponent) -> u32| r.full.score.iter().map(|b| f(&b[p])).sum::<u32>();
    // (the silver of a level reached is paid on top with its own notice, progress::add_xp)
    let level_silver = gd.levels.level_up_silver(before.xp, after.xp);
    let paid = (after.silver - before.silver - level_silver, after.xp - before.xp, after.points(after.faction) - before.points(before.faction));
    // (`extras[1]`: profile 1; a fn pointer cannot capture `p`)
    assert_eq!(rows(|a| a.extras[1].0) + blocks(|c| c.money), paid.0);
    assert_eq!(rows(|a| u32::from(a.extras[1].1)) + blocks(|c| u32::from(c.exp)), paid.1);
    assert_eq!(rows(|a| u32::from(a.extras[1].2)) + blocks(|c| u32::from(c.rep)), paid.2);
    assert_eq!((r.full.f14, u32::from(r.full.exp_delta), u32::from(r.full.rep_delta)), paid, "the gross amounts");
    // the kills row is the kill events' share: 5 tdm-or-base kills x money, level factor, before premium
    assert!(r.full.awards[0].extras[p].0 > r.full.awards[1].extras[p].0);

    // the equipment view of a clicked row (38 00 -> 93)
    let me = w.clients[0].account;
    fn ask(w: &mut World, player: u64, b: u32) -> Option<protocol::s2c::PlayerDetails93> {
        use protocol::Decode;
        let World { e, clients, .. } = w;
        clients[0].send(Request::VictoryData { a: 0, player, b });
        pump_all(e, clients);
        clients[0].take().into_iter().find_map(|m| match m {
            S2c::Raw { id: 0x93, payload } => Some(protocol::s2c::PlayerDetails93::from_bytes(&payload).unwrap()),
            _ => None,
        })
    }
    // himself: the three profiles with the match options' rule: 1 and 2 playable (state 0 with their slots), 3 switched
    // off (state 2, no slots: the window shows it locked)
    let d = ask(&mut w, public_id(me), MATCH_ID).expect("93 for himself");
    assert_eq!(d.id, public_id(me));
    assert_eq!(d.body.profiles.iter().map(|q| q.kind).collect::<Vec<_>>(), vec![0, 0, 2]);
    assert!(!d.body.profiles[1].slots.is_empty(), "the current profile's equipment");
    assert!(d.body.profiles[2].slots.is_empty());
    // a bot row: its AI player's match kit, one profile
    let d = ask(&mut w, 1, MATCH_ID).expect("93 for bot 1");
    let ai = &gd.matches.ai_players()[3];
    let kit = gd.matches.kit(&gd.items, &ai.profiles[0]);
    assert_eq!((d.id, d.body.name.as_str()), (1, "Вася Топаз"));
    assert_eq!(d.body.profiles.len(), 1);
    assert_eq!(d.body.profiles[0].kind, 0);
    assert_eq!(d.body.profiles[0].slots.len(), kit.slots.len());
    for (slot, uid) in &kit.slots {
        let (idx, _) = d.body.profiles[0].slots[slot];
        let dict = kit.items.iter().find(|i| i.0 == *uid).unwrap().1;
        assert_eq!(d.body.items[usize::from(idx)].dict, dict, "slot {slot}");
    }
    // a bot without a known AI player, another match's id, an id past the rows: no answer
    assert!(ask(&mut w, 2, MATCH_ID).is_none());
    assert!(ask(&mut w, 1, MATCH_ID + 1).is_none());
    assert!(ask(&mut w, 3, MATCH_ID).is_none());
}

/// An inactive player (the world's flag B+0x9F0) earns no achievements, no daily-quest progress
/// and no Winner's Contract day from the match, even on the winning team with kills in his event row; an active player
/// of the same match earns them.
#[test]
fn an_inactive_winner_earns_no_achievements_quests_or_contract_day() {
    game_data::require_tables!();
    use lobby_server::match_data::{RosterSlot, WorldData};
    let (map, mode, weather, n) = smallest_row();
    let mut w = setup(n, Some((map, mode, weather)));
    w.e.server.lobby.cfg.daily_quests = true;
    let gd = game_data::testing::tables();
    // a daily quest every finished match moves ("play N games", no win needed)
    let q = gd.daily_quests.quests.iter().position(|q| q.kind == 0 && !q.must_win && q.value > 1 && q.need_item_of_category == 255).expect("a games quest") as u16;
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
    for c in &w.clients {
        w.e.store
            .update_profile(c.account, |p| {
                p.daily = accounts::DailyState {
                    issued: now - 60,
                    expires: now + 86_000,
                    next_uid: 2,
                    next_msg: 1,
                    slots: vec![accounts::QuestSlot { uid: 1, dict: q, progress: 0, done: false }],
                    ..Default::default()
                };
                p.contract = accounts::ContractState { started: now - 60, won_mask: 0 };
                Ok::<_, ()>(())
            })
            .unwrap();
    }
    queue_all(&mut w);
    let seats = w.fake.created.lock().unwrap()[0].3.clone();
    for c in w.clients.iter_mut() {
        c.take();
    }
    let team_of = |a: i64| seats.iter().find(|s| s.0 == a).unwrap().1;
    let roster = w.clients.iter().enumerate().map(|(i, c)| RosterSlot { slot: i as u8, account: Some(c.account), team: team_of(c.account), entered: true }).collect();
    // both have 4 kills and a 4 streak in the world's rows; player 1 was inactive
    let mut d0 = world_host_proto::PlayerDetail { slot: 0, best_streak: 4, ..Default::default() };
    d0.events[0] = 4;
    let mut d1 = world_host_proto::PlayerDetail { slot: 1, best_streak: 4, inactive: true, ..Default::default() };
    d1.events[0] = 4;
    let world = Arc::new(WorldData { roster, details: vec![d0, d1], kills: None, bots: vec![] });
    let players = w.clients.iter().map(|c| EndedPlayer { account: c.account, team: team_of(c.account), entered: true, left_by_menu: false }).collect();
    let stats: Vec<lobby_server::EndedStat> =
        w.clients.iter().map(|c| lobby_server::EndedStat { account: c.account, kills: 4, deaths: 1, score: 40, world: Some(world.clone()), used: None }).collect();
    // the inactive player's team wins
    let winner = team_of(w.clients[1].account);
    w.fake.ended.lock().unwrap().push(Ended { match_id: MATCH_ID, duration: Duration::from_secs(900), players, stats: Some((stats, [5, 2])), winner: Some(winner) });
    let World { e, clients, .. } = &mut w;
    pump_all(e, clients);
    assert!(w.e.server.registry.online.is_empty());
    let p0 = w.e.store.load_profile(w.clients[0].account).unwrap();
    let p1 = w.e.store.load_profile(w.clients[1].account).unwrap();
    let ach = |p: &accounts::Profile, id: u16| p.ach.get(&id).map_or(0, |x| x.value);
    // the active player: kills 201 and streak 214 counted, the quest moved, the contract day only if his team won
    assert_eq!((ach(&p0, 201), ach(&p0, 214)), (4, 4));
    assert_eq!(p0.daily.slots[0].progress, 1);
    let won0 = team_of(w.clients[0].account) == winner;
    assert_eq!(p0.contract.won_mask, u8::from(won0));
    // the inactive winner: nothing
    assert_eq!((ach(&p1, 201), ach(&p1, 214)), (0, 0), "no achievements");
    assert_eq!(p1.daily.slots[0].progress, 0, "no daily quest progress");
    assert_eq!(p1.contract.won_mask, 0, "no Winner's Contract day");
    let m1 = w.clients[1].take();
    assert!(contract_notices(&m1).is_empty());
    let Some(S2c::MatchResult(r1)) = m1.iter().find(|x| x.id() == 0x97) else { panic!("97: {m1:?}") };
    assert_eq!(r1.full.flag92e, 1);
}

/// Achievements 415-417 ("Play a match with items having at least N random modifiers") count at the end of an online
/// match, from the loadout the match was started with: the profiles a player spawned with (the world host's `Used` row),
/// else the options' start profile. Wearing the items counts nothing, a modifier added after the start is not part of the
/// match, an inactive player counts nothing.
#[test]
fn item_modifier_achievements_count_the_loadout_of_a_finished_active_online_match() {
    game_data::require_tables!();
    use lobby_server::charge::MatchUse;
    use lobby_server::match_data::{RosterSlot, WorldData};
    let (map, mode, weather, n) = smallest_row();
    let mut w = setup(n, Some((map, mode, weather)));
    let worn_of = |p: &accounts::Profile| *p.slots.values().next().expect("the starter wears something");
    let add_mod = |w: &World, account: i64, group: u16| {
        w.e.store
            .update_profile(account, |p| {
                let worn = worn_of(p);
                p.mods.entry(worn).or_default().push((group, 1));
                Ok::<_, ()>(())
            })
            .unwrap();
    };
    // everybody wears an item with two random modifiers
    for c in &w.clients {
        add_mod(&w, c.account, 10);
        add_mod(&w, c.account, 20);
    }
    queue_all(&mut w);
    // a third modifier after the start is not part of the match's loadout
    for c in &w.clients {
        add_mod(&w, c.account, 30);
    }
    let seats = w.fake.created.lock().unwrap()[0].3.clone();
    for c in w.clients.iter_mut() {
        c.take();
    }
    let ach = |p: &accounts::Profile, id: u16| p.ach.get(&id).map_or(0, |x| x.value);
    for c in &w.clients {
        let p = w.e.store.load_profile(c.account).unwrap();
        assert_eq!((ach(&p, 415), ach(&p, 416), ach(&p, 417)), (0, 0, 0), "wearing the items counts nothing");
    }
    let team_of = |a: i64| seats.iter().find(|s| s.0 == a).unwrap().1;
    let roster = w.clients.iter().enumerate().map(|(i, c)| RosterSlot { slot: i as u8, account: Some(c.account), team: team_of(c.account), entered: true }).collect();
    // player 0 active (spawned with options profile 0), player 1 inactive; the others have no Used row
    let d0 = world_host_proto::PlayerDetail { slot: 0, ..Default::default() };
    let d1 = world_host_proto::PlayerDetail { slot: 1, inactive: true, ..Default::default() };
    let world = Arc::new(WorldData { roster, details: vec![d0, d1], kills: None, bots: vec![] });
    let players = w.clients.iter().map(|c| EndedPlayer { account: c.account, team: team_of(c.account), entered: true, left_by_menu: false }).collect();
    let stats: Vec<lobby_server::EndedStat> = w
        .clients
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let used = (i < 2).then(|| MatchUse { profiles: 1, weapons: vec![], stacks: vec![] });
            lobby_server::EndedStat { account: c.account, kills: 0, deaths: 0, score: 10, world: Some(world.clone()), used }
        })
        .collect();
    w.fake.ended.lock().unwrap().push(Ended { match_id: MATCH_ID, duration: Duration::from_secs(900), players, stats: Some((stats, [5, 2])), winner: Some(0) });
    let World { e, clients, .. } = &mut w;
    pump_all(e, clients);
    assert!(w.e.server.registry.online.is_empty());
    let figures = |i: usize| {
        let p = w.e.store.load_profile(w.clients[i].account).unwrap();
        (ach(&p, 415), ach(&p, 416), ach(&p, 417))
    };
    assert_eq!(figures(0), (2, 2, 2), "the spawned profile's item had two modifiers at the start: 415 and 416 reached");
    let gd = game_data::testing::tables();
    let p0 = w.e.store.load_profile(w.clients[0].account).unwrap();
    let reached = |id: u16| gd.achievements.get(id).unwrap().reached(ach(&p0, id));
    assert_eq!((reached(415), reached(416), reached(417)), (1, 1, 0));
    assert_eq!(figures(1), (0, 0, 0), "an inactive player counts nothing");
    for i in 2..n {
        assert_eq!(figures(i), (2, 2, 2), "client {i}: no Used row, the start profile");
    }
}

/// With the match server's winner (the world host's), a player who left before the end is not
/// paid, whether by the menu or by a disconnect / silence; only the menu leave is penalised (a disconnect can be a crash).
/// The players still in the match at the end are paid without reports.
#[test]
fn with_the_servers_winner_early_leavers_are_unpaid_and_only_menu_leavers_fined() {
    game_data::require_tables!();
    let (map, mode, weather, n) = smallest_row();
    let mut w = setup(n, Some((map, mode, weather)));
    // everybody wears an item with one random modifier (achievement 415)
    for c in &w.clients {
        w.e.store
            .update_profile(c.account, |p| {
                let worn = *p.slots.values().next().expect("the starter wears something");
                p.mods.insert(worn, vec![(10, 1)]);
                Ok::<_, ()>(())
            })
            .unwrap();
    }
    queue_all(&mut w);
    let seats = w.fake.created.lock().unwrap()[0].3.clone();
    for c in w.clients.iter_mut() {
        c.take();
    }
    let team_of = |a: i64| seats.iter().find(|s| s.0 == a).unwrap().1;
    let before: Vec<u32> = (0..n).map(|i| silver(&w, i)).collect();
    // client 0 drops (disconnect / silence), client 1 leaves with the menu
    let (a0, a1) = (w.clients[0].account, w.clients[1].account);
    w.fake.left.lock().unwrap().push(Left { match_id: MATCH_ID, account: a0, by_menu: false });
    w.fake.left.lock().unwrap().push(Left { match_id: MATCH_ID, account: a1, by_menu: true });
    {
        let World { e, clients, .. } = &mut w;
        pump_all(e, clients);
    }
    for c in w.clients.iter_mut() {
        c.take();
    }
    let players = w
        .clients
        .iter()
        .map(|c| EndedPlayer { account: c.account, team: team_of(c.account), entered: true, left_by_menu: c.account == a1 })
        .collect();
    w.fake.ended.lock().unwrap().push(Ended { match_id: MATCH_ID, duration: Duration::from_secs(600), players, stats: None, winner: Some(0) });
    let World { e, clients, .. } = &mut w;
    pump_all(e, clients);
    assert!(w.e.server.registry.online.is_empty());
    let p0 = w.e.store.load_profile(a0).unwrap();
    assert_eq!((p0.silver, p0.penalty.count), (before[0], 0), "a disconnect before the end: not paid, not fined");
    let p1 = w.e.store.load_profile(a1).unwrap();
    assert_eq!(p1.penalty.count, 1, "a menu leave is fined");
    assert!(p1.silver <= before[1], "a menu leaver is not paid");
    for i in 2..n {
        assert!(w.clients[i].take().iter().any(|x| x.id() == 0x97), "client {i} stayed to the end: paid without a report");
    }
    for i in 0..2 {
        assert!(!w.clients[i].take().iter().any(|x| x.id() == 0x97), "client {i} left early: no results screen");
    }
    // achievements 415-417: a leaver's match counts nothing (everybody wore the same item)
    for (i, c) in w.clients.iter().enumerate() {
        let p = w.e.store.load_profile(c.account).unwrap();
        let v = p.ach.get(&415).map_or(0, |x| x.value);
        assert_eq!(v, if i < 2 { 0 } else { 1 }, "client {i}");
    }
}

#[test]
fn a_grenade_in_a_pocket_does_not_raise_the_online_match_level() {
    game_data::require_tables!();
    // the match level is the client's own equipment level (items::equipment_level): pockets do not count
    let (map, mode, weather, n) = smallest_row();
    let mut w = setup(n, Some((map, mode, weather)));
    let gd = game_data::testing::tables();
    // 736: a quick item (grenade) of item level 4
    assert!(gd.items.is_quick_item(736) && gd.items.get(736).unwrap().item_level == 4);
    w.e.store
        .update_profile(w.clients[0].account, |p| {
            let uid = p.add_item(736, 1);
            p.slots.insert(13, uid);
            p.slot_rounds.insert(13, 1);
            Ok::<_, ()>(())
        })
        .unwrap();
    let before: Vec<u32> = (0..n).map(|i| silver(&w, i)).collect();
    queue_all(&mut w);
    let seats = w.fake.created.lock().unwrap()[0].3.clone();
    for c in w.clients.iter_mut() {
        c.take();
    }
    for i in 0..n {
        let team = seats.iter().find(|s| s.0 == w.clients[i].account).unwrap().1;
        report(&mut w, i, if team == 0 { 5 } else { 6 }, MATCH_ID);
    }
    let players = w.clients.iter().map(|c| EndedPlayer { account: c.account, team: seats.iter().find(|s| s.0 == c.account).unwrap().1, entered: true, left_by_menu: false }).collect();
    w.fake.ended.lock().unwrap().push(Ended { match_id: MATCH_ID, duration: Duration::from_secs(600), players, stats: None, winner: None });
    let World { e, clients, .. } = &mut w;
    pump_all(e, clients);
    assert!(w.e.server.registry.online.is_empty());
    let row = gd.drops.money.iter().find(|r| r.match_level == lobby_server::online::ONLINE_MATCH_LEVEL).unwrap();
    for i in 0..n {
        let got = silver(&w, i) - before[i];
        assert!((row.min..=row.max).contains(&got), "client {i}: the silver roll of match level 1, not of the pocket grenade's level: {got}");
    }
}

/// The booster cost row of an online match is the one of the match's equipment level, like its reward.
#[test]
fn the_booster_cost_row_follows_the_match_equipment_level() {
    game_data::require_tables!();
    let gd = game_data::testing::tables();
    let (map, mode, weather, _) = smallest_row();
    let strong = gd.items.iter().find(|i| i.item_level >= 3 && i.section == "weapons" && !i.is_stackable).expect("a level 3+ weapon");
    let level = strong.item_level.clamp(1, 5);
    let cost = gd.vouchers.match_cost(level, 0).unwrap();
    assert!(cost > gd.vouchers.match_cost(lobby_server::online::ONLINE_MATCH_LEVEL, 0).unwrap(), "the rows differ");
    let mut w = setup_with(2, 2, Some((map, mode, weather)));
    let dict = strong.dict_id;
    for i in 0..2 {
        w.e.store
            .update_profile(w.clients[i].account, |p| {
                let uid = p.add_item(dict, 100);
                p.slots.insert(5, uid);
                p.silver = cost + 1000;
                p.boosters.entry(339).or_default().enabled = true;
                Ok::<(), ()>(())
            })
            .unwrap();
    }
    queue_all(&mut w);
    assert_eq!(w.fake.created.lock().unwrap().len(), 1);
    assert_eq!(silver(&w, 0), 1000, "the cost row of the match level was taken");
}

/// One queue match where team 0 wins (everybody reports); returns the team of every client and what each received.
fn play_round(w: &mut World) -> (Vec<u8>, Vec<Vec<S2c>>) {
    queue_all(w);
    let seats = w.fake.created.lock().unwrap().last().unwrap().3.clone();
    for c in w.clients.iter_mut() {
        c.take();
    }
    let teams: Vec<u8> = w.clients.iter().map(|c| seats.iter().find(|s| s.0 == c.account).unwrap().1).collect();
    for (i, t) in teams.clone().into_iter().enumerate() {
        report(w, i, if t == 0 { 5 } else { 6 }, MATCH_ID);
    }
    let players = w.clients.iter().zip(&teams).map(|(c, t)| EndedPlayer { account: c.account, team: *t, entered: true, left_by_menu: false }).collect();
    w.fake.ended.lock().unwrap().push(Ended { match_id: MATCH_ID, duration: Duration::from_secs(600), players, stats: None, winner: None });
    let World { e, clients, .. } = w;
    pump_all(e, clients);
    assert!(w.e.server.registry.online.is_empty());
    let got = w.clients.iter_mut().map(|c| c.take()).collect();
    (teams, got)
}

fn contract_notices(m: &[S2c]) -> Vec<SystemMessage> {
    m.iter().filter_map(|x| if let S2c::SystemMessage(s) = x { (s.kind == 0x16).then(|| s.clone()) } else { None }).collect()
}

fn set_contract(w: &World, started: i64, won_mask: u8) {
    for c in &w.clients {
        w.e.store.update_profile(c.account, |p| { p.contract = accounts::ContractState { started, won_mask }; Ok::<_, ()>(()) }).unwrap();
    }
}

fn mask(w: &World, i: usize) -> u8 {
    w.e.store.load_profile(w.clients[i].account).unwrap().contract.won_mask
}

/// The Winner's Contract (contract.rs): the first won online match of a contract day pays 20 000 silver once and marks
/// the day; a loss, a second win the same day or an expired contract pays nothing.
#[test]
fn the_winners_contract_pays_the_first_win_of_each_day() {
    game_data::require_tables!();
    let (map, mode, weather, n) = smallest_row();
    let mut w = setup(n, Some((map, mode, weather)));
    let now = || std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
    let gd = game_data::testing::tables();
    let row = gd.drops.money.iter().find(|r| r.match_level == lobby_server::online::ONLINE_MATCH_LEVEL).unwrap();
    let (lo, hi) = (row.min, row.max);

    // day 1: every player holds a contract bought a minute ago
    set_contract(&w, now() - 60, 0);
    let before: Vec<u32> = (0..n).map(|i| silver(&w, i)).collect();
    let (teams, got) = play_round(&mut w);
    assert!(teams.contains(&0) && teams.contains(&1));
    for i in 0..n {
        let delta = silver(&w, i) - before[i];
        let notes = contract_notices(&got[i]);
        if teams[i] == 0 {
            assert_eq!(mask(&w, i), 0b001, "client {i}: day 1 marked");
            assert!((lo + 20_000..=hi + 20_000).contains(&delta), "client {i}: roll + 20 000: {delta}");
            assert_eq!(notes.len(), 1, "client {i}: one B1 0x16");
            assert_eq!(notes[0].text, "[20000][0]");
            // 8B, then the 94 with the day won (before the notice, which opens the contract window), then the notice
            let ids: Vec<u8> = got[i].iter().map(|x| x.id()).collect();
            let at_b1 = got[i].iter().position(|x| matches!(x, S2c::SystemMessage(s) if s.kind == 0x16)).unwrap();
            let at_94 = got[i][..at_b1].iter().rposition(|x| x.id() == 0x94).expect("a 94 before the notice");
            let at_8b = got[i][..at_94].iter().rposition(|x| x.id() == 0x8B).expect("an 8B before the 94");
            assert!(at_8b < at_94, "{ids:02x?}");
            let S2c::Notices(n94) = &got[i][at_94] else { panic!() };
            let t = n94.tail.clone().expect("contract tail");
            assert_eq!(t.bytes, vec![1, 0, 0]);
            assert_ne!(t.b, u32::MAX, "today's task is done: the countdown to day 2 runs");
        } else {
            assert_eq!(mask(&w, i), 0, "client {i}: a loss marks nothing");
            assert!((lo..=hi).contains(&delta), "client {i}: the roll only: {delta}");
            assert!(notes.is_empty());
        }
    }

    // the same day again: a day already paid pays nothing; a first win of the day (a loser of round 1 now winning) pays
    let before: Vec<u32> = (0..n).map(|i| silver(&w, i)).collect();
    let masks: Vec<u8> = (0..n).map(|i| mask(&w, i)).collect();
    let (teams, got) = play_round(&mut w);
    for i in 0..n {
        let delta = silver(&w, i) - before[i];
        let first_win = teams[i] == 0 && masks[i] == 0;
        assert_eq!(contract_notices(&got[i]).len(), usize::from(first_win), "client {i}");
        assert_eq!(mask(&w, i), if first_win { 0b001 } else { masks[i] }, "client {i}");
        assert_eq!(delta >= 20_000, first_win, "client {i}: {delta}");
    }

    // day 2 (the purchase moved one day back, day 1 won): a win pays day 2
    set_contract(&w, now() - 86_400 - 60, 0b001);
    let (teams, got) = play_round(&mut w);
    for i in 0..n {
        let won = teams[i] == 0;
        assert_eq!(mask(&w, i), if won { 0b011 } else { 0b001 }, "client {i}");
        assert_eq!(contract_notices(&got[i]).len(), usize::from(won));
    }

    // after the 3 days nothing is paid and no 94 carries the contract any more
    set_contract(&w, now() - 3 * 86_400 - 60, 0b011);
    let before: Vec<u32> = (0..n).map(|i| silver(&w, i)).collect();
    let (_, got) = play_round(&mut w);
    for i in 0..n {
        assert!(contract_notices(&got[i]).is_empty(), "client {i}: the contract is over");
        assert!(silver(&w, i) - before[i] <= hi);
        assert_eq!(mask(&w, i), 0b011);
        assert!(got[i].iter().all(|x| !matches!(x, S2c::Notices(n) if n.tail.is_some())));
    }
}

/// The stock count of `dict` in an account's inventory.
fn stock(w: &World, who: usize, dict: u16) -> u32 {
    w.e.store.load_profile(w.clients[who].account).unwrap().items.iter().filter(|i| i.dict_id == dict).map(|i| i.count).sum()
}

/// A PvP match that ends before anybody entered (the world host gave no start world: `NoWorld`, or nobody loaded):
/// what its start took comes back. Client 0 paid a per-match booster in silver, client 1 a booster from stock and one
/// service-booster unit; both get it back with their inventory (89), money (8B) and booster records (94). A match that
/// somebody entered gives nothing back.
#[test]
fn a_pvp_match_nobody_entered_gives_back_the_boosters_and_service_units() {
    game_data::require_tables!();
    let gd = game_data::testing::tables();
    let (map, mode, weather, _) = smallest_row();
    let cost0 = gd.vouchers.match_cost(lobby_server::online::ONLINE_MATCH_LEVEL, 0).unwrap();
    let service = gd.vouchers.service_boosters().next().expect("a service booster").dict_id;
    for entered in [false, true] {
        let mut w = setup_with(2, 2, Some((map, mode, weather)));
        let (a0, a1) = (w.clients[0].account, w.clients[1].account);
        w.e.store
            .update_profile(a0, |p| {
                p.silver = cost0 + 1000;
                p.boosters.entry(339).or_default().enabled = true;
                Ok::<(), ()>(())
            })
            .unwrap();
        w.e.store
            .update_profile(a1, |p| {
                p.silver = 5;
                p.boosters.entry(339).or_default().enabled = true;
                p.add_item(339, 1);
                p.add_item(service, 2);
                Ok::<(), ()>(())
            })
            .unwrap();
        let before1 = stock(&w, 1, service);
        queue_all(&mut w);
        assert_eq!(w.fake.created.lock().unwrap().len(), 1);
        assert_eq!(silver(&w, 0), 1000, "client 0 paid the booster in silver");
        assert_eq!((stock(&w, 1, 339), stock(&w, 1, service), silver(&w, 1)), (0, before1 - 1, 5), "client 1 paid from stock and used a service unit");
        for c in w.clients.iter_mut() {
            c.take();
        }
        let players = vec![EndedPlayer { account: a0, team: 0, entered, left_by_menu: false }, EndedPlayer { account: a1, team: 1, entered, left_by_menu: false }];
        w.fake.ended.lock().unwrap().push(Ended { match_id: MATCH_ID, duration: Duration::from_secs(if entered { 600 } else { 0 }), players, stats: None, winner: None });
        let World { e, clients, .. } = &mut w;
        pump_all(e, clients);
        w.e.wait(&mut w.clients.iter_mut().collect::<Vec<_>>(), 8);
        if entered {
            assert_eq!(silver(&w, 0), 1000, "somebody entered: nothing comes back");
            assert_eq!(stock(&w, 1, 339), 0);
            continue;
        }
        assert_eq!(silver(&w, 0), cost0 + 1000, "client 0's silver is back");
        assert_eq!((stock(&w, 1, 339), stock(&w, 1, service), silver(&w, 1)), (1, before1, 5), "client 1's booster and service unit are back");
        for i in 0..2 {
            let ids: Vec<u8> = w.clients[i].take().iter().map(|m| m.id()).collect();
            assert!(ids.contains(&0x89) && ids.contains(&0x8B) && ids.contains(&0x94), "client {i} sees it: {ids:02x?}");
        }
        assert!(w.e.server.registry.online.is_empty());
    }
}

/// The last 98 of a batch.
fn room98(m: &[S2c]) -> Option<Room98> {
    m.iter().rev().find_map(|x| match x {
        S2c::Raw { id: 0x98, payload } => <Room98 as protocol::Decode>::from_bytes(payload).ok(),
        _ => None,
    })
}

/// A training room with bots (the world host drives them): the 98 offers up to the team limit, the Start creates the
/// match on the room's row with the room's AI players per team, the reserve stays out with its own text. A row the match
/// server does not serve is refused with "The mode is unavailable".
#[test]
fn a_custom_room_starts_with_its_bots_and_leaves_the_reserve_out() {
    game_data::require_tables!();
    let mut w = setup(2, None);
    *w.fake.bots.lock().unwrap() = true;
    let (a, b) = (w.clients[0].account, w.clients[1].account);
    let send = |w: &mut World, who: usize, r: Custom62| {
        let World { e, clients, .. } = w;
        clients[who].send(Request::Other { id: 0x62, payload: r.encode() });
        pump_all(e, clients);
    };
    send(&mut w, 0, Custom62::Create { kind: 0 });
    send(&mut w, 0, Custom62::Invite { target: public_id(b) });
    send(&mut w, 1, Custom62::Answer { inviter: public_id(a), accept: 0 });
    let room = room98(&w.clients[0].take()).expect("98");
    let gd = game_data::testing::tables();
    let row = gd.matches.modes().iter().find(|r| (r.map, r.mode, r.variation) == (room.map.into(), room.mode.into(), room.weather.into())).unwrap().clone();
    let cap = ((if row.max_players > 0 { row.max_players } else { row.min_players }) / 2).clamp(1, 9) as u8;
    let size = if row.max_players > 0 { row.max_players } else { row.min_players };
    let bots = cap.min(lobby_server::online::bot_team_cap(LobbyConfig::default().bots_per_team, size).0 as u8);
    assert_eq!((room.max_ai, room.max_players), (bots, cap), "bots offered up to the team limit");
    // the host ticks "bots ignore players" (62 1E, the same row): stored and shown, and it goes with the bots at the Start
    send(&mut w, 0, Custom62::Parameters { map: room.map, weather: room.weather, mode: room.mode, region: 0, voice: 0, ai_ignore: 1 });
    assert_eq!(room98(&w.clients[0].take()).map(|r| r.ai_ignore), Some(1));
    // over the limit: refused; alice alone on team A, three bots on team B; bob stays in the reserve
    send(&mut w, 0, Custom62::AiCount { team: 1, count: bots + 1 });
    let m = w.clients[0].take();
    assert!(m.iter().any(|x| matches!(x, S2c::Denied(d) if matches!(&d.body, DeniedBody::Text(t) if t == "st_cant_change_ai_count_custom_match_error"))), "{m:?}");
    send(&mut w, 0, Custom62::AiCount { team: 1, count: 3 });
    assert_eq!(room98(&w.clients[0].take()).map(|r| (r.ai_a, r.ai_b)), Some((0, 3)));
    // a row the match server does not serve: "The mode is unavailable"
    *w.fake.modes.lock().unwrap() = Some(vec![]);
    send(&mut w, 0, Custom62::Tokens { gold: 0, silver: 1 });
    send(&mut w, 1, Custom62::Tokens { gold: 1, silver: 0 });
    w.clients[0].take();
    send(&mut w, 0, Custom62::Start);
    let m = w.clients[0].take();
    assert!(m.iter().any(|x| matches!(x, S2c::Denied(d) if matches!(&d.body, DeniedBody::Text(t) if t == "st_locked_by_server"))), "{m:?}");
    assert!(w.fake.bot_creates.lock().unwrap().is_empty());
    *w.fake.modes.lock().unwrap() = None;
    send(&mut w, 0, Custom62::Start);
    let creates = w.fake.bot_creates.lock().unwrap().clone();
    assert_eq!(creates.len(), 1, "one match with bots");
    let (map, mode, seats, fill) = creates[0].clone();
    assert_eq!((map, mode), (row.map, row.mode));
    assert_eq!(seats, vec![(a, 0)], "the reserve does not play");
    assert_eq!(fill.need, [0, 3]);
    assert!(fill.ignore_players, "the room's 'bots ignore players' reaches the match server");
    let (ma, mb) = (w.clients[0].take(), w.clients[1].take());
    assert!(connect_msg(&ma).is_some() && connect_msg(&mb).is_none());
    assert!(ma.iter().any(|x| matches!(x, S2c::Raw { id: 0xA0, .. })));
    let reason = mb.iter().find_map(|x| match x {
        S2c::Raw { id: 0xA1, payload } => protocol::wire::Reader::new(payload).str().ok(),
        _ => None,
    });
    assert_eq!(reason.as_deref(), Some("st_custom_match_match_started_without_you"));
    assert!(w.e.server.registry.rooms.is_empty());
}

/// A custom clan / training room's tokens are charged when the match exists; a match nobody entered gives them back.
#[test]
fn a_custom_match_nobody_entered_gives_back_the_tokens() {
    game_data::require_tables!();
    let mut w = setup(2, None);
    let (a, b) = (w.clients[0].account, w.clients[1].account);
    let send = |w: &mut World, who: usize, r: Custom62| {
        let World { e, clients, .. } = w;
        clients[who].send(Request::Other { id: 0x62, payload: r.encode() });
        pump_all(e, clients);
    };
    send(&mut w, 0, Custom62::Create { kind: 0 });
    send(&mut w, 0, Custom62::Invite { target: public_id(b) });
    send(&mut w, 1, Custom62::Answer { inviter: public_id(a), accept: 0 });
    send(&mut w, 1, Custom62::MemberStatus { player: public_id(b), status: 1 });
    send(&mut w, 1, Custom62::Ready { ready: 0xFF });
    send(&mut w, 0, Custom62::Tokens { gold: 0, silver: 1 });
    send(&mut w, 1, Custom62::Tokens { gold: 1, silver: 0 });
    let money = |w: &World| -> Vec<(u32, u32)> { [a, b].iter().map(|id| w.e.store.load_profile(*id).map(|p| (p.silver, p.gold)).unwrap()).collect() };
    let before = money(&w);
    send(&mut w, 0, Custom62::Start);
    assert_eq!(w.fake.created.lock().unwrap().len(), 1, "the match started");
    assert_ne!(money(&w), before, "the tokens were charged");
    for c in w.clients.iter_mut() {
        c.take();
    }
    let players = vec![EndedPlayer { account: a, team: 0, entered: false, left_by_menu: false }, EndedPlayer { account: b, team: 1, entered: false, left_by_menu: false }];
    w.fake.ended.lock().unwrap().push(Ended { match_id: MATCH_ID, duration: Duration::ZERO, players, stats: None, winner: None });
    let World { e, clients, .. } = &mut w;
    pump_all(e, clients);
    w.e.wait(&mut w.clients.iter_mut().collect::<Vec<_>>(), 8);
    assert_eq!(money(&w), before, "nobody entered: the tokens are back");
    for i in 0..2 {
        let ids: Vec<u8> = w.clients[i].take().iter().map(|m| m.id()).collect();
        assert!(ids.contains(&0x8B), "client {i} sees the money: {ids:02x?}");
    }
}

/// The tdm row (map, mode, weather) and its players per team.
fn tdm_row() -> (u32, u32, u32, usize) {
    let gd = game_data::testing::tables();
    let r = gd.matches.modes().iter().filter(|r| r.project.ends_with("_tdm") && r.min_players >= 12).min_by_key(|r| (r.map, r.variation)).expect("a tdm row of 12 or more").clone();
    let size = if r.max_players > 0 { r.max_players } else { r.min_players };
    (r.map, r.mode, r.variation, lobby_server::online::bot_team_cap(LobbyConfig::default().bots_per_team, size).0)
}

/// The humans' average equipped match-making level as the lobby computes it (online.rs `bot_level`), from the store.
fn expected_bot_level(w: &World, who: &[usize]) -> u32 {
    let gd = game_data::testing::tables();
    let per: Vec<f64> = who
        .iter()
        .filter_map(|&i| {
            let p = w.e.store.load_profile(w.clients[i].account).unwrap();
            let lv: Vec<u32> = p
                .profile_slots(p.equip.current)
                .iter()
                .filter(|(s, _)| !((13..=17).contains(*s) || (21..=22).contains(*s)))
                .filter_map(|(_, u)| p.item(*u).and_then(|r| gd.items.get(r.dict_id)))
                .map(|it| it.match_making_level)
                .filter(|l| *l > 0)
                .collect();
            (!lv.is_empty()).then(|| lv.iter().sum::<u32>() as f64 / lv.len() as f64)
        })
        .collect();
    if per.is_empty() { 1 } else { ((per.iter().sum::<f64>() / per.len() as f64).round() as u32).clamp(1, 5) }
}

/// A lone searcher whose wait passed gets a match with AI players: he is on team 0,
/// bots fill team 0 to 6 and team 1 to 6, picked at the humans' average equipped level; he gets his `82` and the
/// ticket is used up. Before the wait, or when the match server drives no bots, he keeps searching.
#[test]
fn a_lone_searcher_gets_ai_players_after_the_wait() {
    game_data::require_tables!();
    let (map, mode, weather, cap) = tdm_row();
    // the wait has passed at once (0 s): the next lobby tick fills the match
    let mut w = setup_cfg(1, 2, Some((map, mode, weather)), |c| c.online_bot_wait = Some(Duration::ZERO));
    *w.fake.bots.lock().unwrap() = true;
    queue_all(&mut w);
    let bc = w.fake.bot_creates.lock().unwrap().clone();
    assert_eq!(bc.len(), 1, "one bot-filled match");
    let (m, md, seats, fill) = &bc[0];
    assert_eq!((*m, *md), (map, mode));
    assert_eq!(seats, &vec![(w.clients[0].account, 0u8)]);
    assert_eq!(fill.need, [cap - 1, cap], "bots fill both teams to {cap}");
    assert_eq!(fill.level, expected_bot_level(&w, &[0]));
    assert!(!fill.ignore_players, "queue bots fight");
    assert!(w.fake.created.lock().unwrap().is_empty(), "no human-only match");
    let msgs = w.clients[0].take();
    let t = connect_msg(&msgs).expect("82");
    assert_eq!((t.match_id, t.side, t.order_id), (MATCH_ID, 0, 100));
    assert!(w.e.server.registry.queue.is_empty(), "the ticket is used up");

    // the wait has not passed: still searching, nothing created
    let mut w = setup_cfg(1, 2, Some((map, mode, weather)), |c| c.online_bot_wait = Some(Duration::from_secs(3600)));
    *w.fake.bots.lock().unwrap() = true;
    queue_all(&mut w);
    assert!(w.fake.bot_creates.lock().unwrap().is_empty());
    assert_eq!(w.e.server.registry.queue.len(), 1);
    // no bot-driving match server: still searching
    let mut w = setup_cfg(1, 2, Some((map, mode, weather)), |c| c.online_bot_wait = Some(Duration::ZERO));
    queue_all(&mut w);
    assert!(w.fake.bot_creates.lock().unwrap().is_empty() && w.fake.created.lock().unwrap().is_empty());
    assert_eq!(w.e.server.registry.queue.len(), 1);
    // switched off (None): still searching
    let mut w = setup_cfg(1, 2, Some((map, mode, weather)), |c| c.online_bot_wait = None);
    *w.fake.bots.lock().unwrap() = true;
    queue_all(&mut w);
    assert!(w.fake.bot_creates.lock().unwrap().is_empty());
}

/// Three humans who waited are spread over both teams (2 and 1) before the bots fill each team to 6.
#[test]
fn waiting_humans_are_spread_evenly_before_the_bots() {
    game_data::require_tables!();
    let (map, mode, weather, cap) = tdm_row();
    let mut w = setup_cfg(3, 8, Some((map, mode, weather)), |c| c.online_bot_wait = Some(Duration::from_secs(3600)));
    *w.fake.bots.lock().unwrap() = true;
    queue_all(&mut w);
    assert!(w.fake.bot_creates.lock().unwrap().is_empty(), "nobody waited long enough yet");
    assert_eq!(w.e.server.registry.queue.len(), 3);
    // the wait passes for all three at once
    w.e.server.lobby.cfg.online_bot_wait = Some(Duration::ZERO);
    let World { e, clients, .. } = &mut w;
    pump_all(e, clients);
    let bc = w.fake.bot_creates.lock().unwrap().clone();
    assert_eq!(bc.len(), 1, "{bc:?}");
    let (_, _, seats, fill) = &bc[0];
    let t0 = seats.iter().filter(|s| s.1 == 0).count();
    assert_eq!((seats.len(), t0), (3, 2), "two on team 0, one on team 1: {seats:?}");
    assert_eq!(fill.need, [cap - 2, cap - 1]);
    assert_eq!(fill.level, expected_bot_level(&w, &[0, 1, 2]));
    for c in w.clients.iter_mut() {
        assert!(connect_msg(&c.take()).is_some(), "every human gets 82");
    }
}

/// Ranked stays humans-only. A ranked search whose wait passed gets no bots and keeps
/// searching; an unrated search queued after it still gets its bot-filled match (the ranked ticket does not block it).
#[test]
fn bot_fill_never_takes_a_ranked_search() {
    game_data::require_tables!();
    let (map, mode, weather, cap) = tdm_row();
    let mut w = setup_cfg(2, 4, Some((map, mode, weather)), |c| c.online_bot_wait = Some(Duration::from_secs(3600)));
    *w.fake.bots.lock().unwrap() = true;
    {
        let World { e, clients, .. } = &mut w;
        // client 0 ranked (2F arg 2), client 1 random
        clients[0].send(Request::Queue { global: 0, arg: 2, mode: 0x380, flag: 0 });
        pump_all(e, clients);
        clients[1].send(Request::Queue { global: 0, arg: 0, mode: 0x380, flag: 0 });
        pump_all(e, clients);
    }
    assert_eq!(w.e.server.registry.queue.len(), 2);
    w.e.server.lobby.cfg.online_bot_wait = Some(Duration::ZERO);
    let World { e, clients, .. } = &mut w;
    pump_all(e, clients);
    let bc = w.fake.bot_creates.lock().unwrap().clone();
    assert_eq!(bc.len(), 1, "{bc:?}");
    assert_eq!(bc[0].2, vec![(w.clients[1].account, 0u8)], "only the random searcher");
    assert_eq!(bc[0].3.need, [cap - 1, cap]);
    assert!(w.fake.created.lock().unwrap().is_empty());
    assert_eq!(w.e.server.registry.queue.len(), 1, "the ranked search keeps waiting");
    assert!(w.e.server.registry.queue.ticket_of(w.clients[0].account).is_some_and(|t| t.rated));
    assert!(connect_msg(&w.clients[0].take()).is_none(), "no 82 for the ranked searcher");
    assert_eq!(*w.fake.types.lock().unwrap(), Vec::<u8>::new());
}

fn pump(w: &mut World) {
    let World { e, clients, .. } = w;
    pump_all(e, clients);
}

/// The data's four Search and Destroy layouts (map, variation).
const SAD_LAYOUTS: [(u32, u32); 4] = [(2, 7), (3, 5), (3, 6), (12, 5)];

/// Search and Destroy from the queue: with a match server that serves the data's S&D rows, two players who tick only the
/// S&D box (bit 5) get an S&D match on one of its four layouts; its world's win at 5 rounds to 0 (the team values) counts
/// 628 and 627 for the winner (achievement mode mask bit 5 = S&D) and nothing for the loser.
#[test]
fn a_search_and_destroy_match_from_the_queue_counts_627_and_628() {
    game_data::require_tables!();
    let mut w = setup(2, None);
    *w.fake.serve_all.lock().unwrap() = true;
    queue_masks(&mut w, &[(0, 0x3A0), (1, 0x3A0)]);
    let created = w.fake.created.lock().unwrap().clone();
    assert_eq!(created.len(), 1, "one match");
    let (map, mode, variation, seats) = created[0].clone();
    assert_eq!(mode, 5);
    assert!(SAD_LAYOUTS.contains(&(map, variation)), "({map}, {variation})");
    let team_of = |a: i64| seats.iter().find(|s| s.0 == a).unwrap().1;
    let winner = team_of(w.clients[0].account);
    let players = w.clients.iter().map(|c| EndedPlayer { account: c.account, team: team_of(c.account), entered: true, left_by_menu: false }).collect();
    let stats: Vec<lobby_server::EndedStat> =
        w.clients.iter().map(|c| lobby_server::EndedStat { account: c.account, kills: 0, deaths: 0, score: 0, world: None, used: None }).collect();
    let mut rounds = [0u32; 2];
    rounds[usize::from(winner)] = 5;
    w.fake.ended.lock().unwrap().push(Ended { match_id: MATCH_ID, duration: Duration::from_secs(700), players, stats: Some((stats, rounds)), winner: Some(winner) });
    pump(&mut w);
    let ach = |i: usize, id: u16| w.e.store.load_profile(w.clients[i].account).unwrap().ach.get(&id).map_or(0, |x| x.value);
    assert_eq!((ach(0, 628), ach(0, 627)), (1, 1), "an S&D win, 5 - 0");
    assert_eq!((ach(1, 628), ach(1, 627)), (0, 0), "the loser");
}

/// Search and Destroy gets no AI players (online::NO_BOT_MODES: its projects have no patrol graphs): a search with only
/// the S&D box keeps waiting after the bot wait, one that also ticks TDM gets its bot-filled match on TDM.
#[test]
fn bot_fill_skips_search_and_destroy() {
    game_data::require_tables!();
    let mut w = setup_cfg(1, 4, None, |c| c.online_bot_wait = Some(Duration::ZERO));
    *w.fake.serve_all.lock().unwrap() = true;
    *w.fake.bots.lock().unwrap() = true;
    queue_masks(&mut w, &[(0, 0x3A0)]);
    pump(&mut w);
    assert!(w.fake.bot_creates.lock().unwrap().is_empty() && w.fake.created.lock().unwrap().is_empty());
    assert_eq!(w.e.server.registry.queue.len(), 1, "an S&D-only search keeps waiting for players");
    let mut w = setup_cfg(1, 4, None, |c| c.online_bot_wait = Some(Duration::ZERO));
    *w.fake.serve_all.lock().unwrap() = true;
    *w.fake.bots.lock().unwrap() = true;
    queue_masks(&mut w, &[(0, 0x3A2)]);
    pump(&mut w);
    let bc = w.fake.bot_creates.lock().unwrap().clone();
    assert_eq!(bc.len(), 1, "{bc:?}");
    assert_eq!(bc[0].1, 1, "the bot fill plays TDM, not S&D");
}

/// Ranked plays the slot's mode; when the match server serves no ranked row of it the slot falls through to the next
/// rotation mode instead of waiting for the next slot. Time-free: the test takes whatever slot the clock is in (the
/// queue unit tests pin the S&D slot itself).
#[test]
fn ranked_plays_the_slot_mode_or_falls_through_to_the_next() {
    game_data::require_tables!();
    use lobby_server::queue::{ranked_order, RANKED_SLOT_SECS};
    let rank = |w: &mut World| {
        let World { e, clients, .. } = w;
        for i in 0..clients.len() {
            clients[i].send(Request::Queue { global: 0, arg: 2, mode: 0x382, flag: 0 });
            pump_all(e, clients);
        }
    };
    let now = || std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
    // not within 20 s of a slot change (the setup and the presses take a few seconds)
    while now().rem_euclid(RANKED_SLOT_SECS) > RANKED_SLOT_SECS - 20 {
        std::thread::sleep(Duration::from_secs(1));
    }
    let order = ranked_order(now());
    let mut w = setup(2, None);
    *w.fake.serve_all.lock().unwrap() = true;
    rank(&mut w);
    let created = w.fake.created.lock().unwrap().clone();
    assert_eq!(created.len(), 1);
    assert_eq!(created[0].1, order[0], "the slot's mode");
    if order[0] == 5 {
        assert!([(2, 7), (3, 5), (12, 5)].contains(&(created[0].0, created[0].2)), "a ranked S&D layout: {:?}", created[0]);
    }
    // the slot's mode not served: the next mode of the rotation
    let mut w = setup(2, None);
    *w.fake.serve_all.lock().unwrap() = true;
    *w.fake.modes.lock().unwrap() = Some(order[1..].to_vec());
    rank(&mut w);
    let created = w.fake.created.lock().unwrap().clone();
    assert_eq!(created.len(), 1, "no row of the slot's mode: falls through, no silent wait");
    assert_eq!(created[0].1, order[1], "the next slot's mode");
}

/// Rejoin: a player the match server drops with his slot kept is not released (no `85 00`:
/// his client keeps its in-match state and its own "Lost connection to server. Reconnecting ..." text) and gets his
/// seat's `82` again; once back he is paid as a player who stayed.
#[test]
fn a_dropped_player_keeps_his_seat_gets_his_82_again_and_is_paid_when_back() {
    game_data::require_tables!();
    let (map, mode, weather, n) = smallest_row();
    let mut w = setup(n, Some((map, mode, weather)));
    queue_all(&mut w);
    let first = connect_msg(&w.clients[0].take()).expect("the start's 82");
    for c in w.clients.iter_mut() {
        c.take();
    }
    let a0 = w.clients[0].account;
    let before = silver(&w, 0);
    *w.fake.allow_rejoin.lock().unwrap() = true;
    w.fake.dropped.lock().unwrap().push(Left { match_id: MATCH_ID, account: a0, by_menu: false });
    pump(&mut w);
    let got = w.clients[0].take();
    assert_eq!(connect_msg(&got), Some(first), "the same 82 again: same server, match, side and order id");
    assert!(!got.iter().any(|m| matches!(m, S2c::QueueState(q) if q.state == 0)), "no 85 00: still in the match");
    assert_eq!(w.e.server.registry.online.match_of(a0), Some(MATCH_ID));
    // back in the match, then the end with the server's winner: paid like everybody who stayed
    w.fake.rejoined.lock().unwrap().push((MATCH_ID, a0));
    pump(&mut w);
    let seats = w.fake.created.lock().unwrap()[0].3.clone();
    let players = seats.iter().map(|s| EndedPlayer { account: s.0, team: s.1, entered: true, left_by_menu: false }).collect();
    w.fake.ended.lock().unwrap().push(Ended { match_id: MATCH_ID, duration: Duration::from_secs(600), players, stats: None, winner: Some(0) });
    pump(&mut w);
    assert!(w.clients[0].take().iter().any(|x| x.id() == 0x97));
    assert!(silver(&w, 0) > before, "paid");
}

/// Rejoin, the parts `a_dropped_player_signing_in_again_gets_his_82_after_his_first_request` does not check: after a new
/// sign-in the match server gets the new login session with the rejoin offer (the client's C9 carries it; the match server
/// refuses a C9 whose session differs, relay `attach`), and once the client is back in the match a later sign-in or lobby
/// reconnect brings no `82` (a second `82` would make the client close its loaded match and connect again, 0x20018E276).
#[test]
fn a_rejoin_after_a_new_sign_in_hands_the_new_session_over_and_ends_once_back() {
    game_data::require_tables!();
    let (map, mode, weather, n) = smallest_row();
    let mut w = setup(n, Some((map, mode, weather)));
    queue_all(&mut w);
    let first = connect_msg(&w.clients[0].take()).expect("the start's 82");
    for c in w.clients.iter_mut() {
        c.take();
    }
    let a0 = w.clients[0].account;
    let old_session = w.clients[0].session;
    *w.fake.allow_rejoin.lock().unwrap() = true;
    w.fake.dropped.lock().unwrap().push(Left { match_id: MATCH_ID, account: a0, by_menu: false });
    pump(&mut w);
    assert_eq!(*w.fake.rejoin_sessions.lock().unwrap(), vec![(a0, None)], "the offer at the drop: the session is unchanged");
    let mut c = w.e.client_of("player0");
    let new_session = c.session;
    assert_ne!(new_session, old_session);
    w.e.sign_in(&mut c);
    assert_eq!(connect_msg(&w.e.handshake(&mut c)), Some(first), "the seat's 82 after the new sign-in");
    assert_eq!(w.fake.rejoin_sessions.lock().unwrap().last(), Some(&(a0, Some(new_session))), "the new session to the match server");
    // back in the match (the match server's rejoined event): no 82 any more
    w.fake.rejoined.lock().unwrap().push((MATCH_ID, a0));
    w.e.pump(&mut [&mut c]);
    let mut r = w.e.reconnect(&c);
    w.e.sign_in(&mut r);
    assert!(connect_msg(&w.e.handshake(&mut r)).is_none(), "back in the match: no 82 after a lobby reconnect");
    assert_eq!(w.e.server.registry.online.match_of(a0), Some(MATCH_ID));
}

/// Room traffic in a batch: a 98 (room state), A0 (ended) or A1 (aborted) would reset or reopen the client's room window.
fn room_traffic(m: &[S2c]) -> Vec<u8> {
    m.iter().filter_map(|x| match x {
        S2c::Raw { id, .. } if matches!(id, 0x98 | 0xA0 | 0xA1) => Some(*id),
        _ => None,
    }).collect()
}

/// Rejoin of a match started from a custom room (`62 23` -> `start_room`): the same path as a queue match. The room is
/// closed at the Start (A0 after the 82, once); a member who drops keeps his seat and gets the seat's 82 again, at the
/// drop and after a new sign-in with the new session; nothing of the closed room reaches him again (no 98 / A0 / A1 that
/// would touch his room window while he loads the match), a new room is refused while his seat is kept, and once back he
/// is in the match like before.
#[test]
fn a_match_started_from_a_custom_room_keeps_the_seat_for_a_rejoin_and_the_closed_room_stays_out() {
    game_data::require_tables!();
    let mut w = setup(2, None);
    let (a, b) = (w.clients[0].account, w.clients[1].account);
    let send = |w: &mut World, who: usize, r: Custom62| {
        let World { e, clients, .. } = w;
        clients[who].send(Request::Other { id: 0x62, payload: r.encode() });
        pump_all(e, clients);
    };
    send(&mut w, 0, Custom62::Create { kind: 0 });
    send(&mut w, 0, Custom62::Invite { target: public_id(b) });
    send(&mut w, 1, Custom62::Answer { inviter: public_id(a), accept: 0 });
    send(&mut w, 1, Custom62::MemberStatus { player: public_id(b), status: 1 });
    send(&mut w, 1, Custom62::Ready { ready: 0xFF });
    send(&mut w, 0, Custom62::Tokens { gold: 0, silver: 1 });
    send(&mut w, 1, Custom62::Tokens { gold: 0, silver: 1 });
    for c in w.clients.iter_mut() {
        c.take();
    }
    send(&mut w, 0, Custom62::Start);
    let mb = w.clients[1].take();
    let first = connect_msg(&mb).expect("bob's 82 at the start");
    assert_eq!(room_traffic(&mb), vec![0xA0], "the room ends once, after the 82");
    w.clients[0].take();
    assert!(w.e.server.registry.rooms.is_empty());
    assert_eq!(w.e.server.registry.online.match_of(b), Some(MATCH_ID));

    // bob drops (silence, crash): the seat is kept, the same 82 again, nothing of the room
    *w.fake.allow_rejoin.lock().unwrap() = true;
    w.fake.dropped.lock().unwrap().push(Left { match_id: MATCH_ID, account: b, by_menu: false });
    pump(&mut w);
    let got = w.clients[1].take();
    assert_eq!(connect_msg(&got), Some(first.clone()), "the same 82: same server, match, side and order id");
    assert!(!got.iter().any(|m| matches!(m, S2c::QueueState(q) if q.state == 0)), "no 85 00: still in the match");
    assert_eq!(room_traffic(&got), Vec::<u8>::new());
    assert!(room_traffic(&w.clients[0].take()).is_empty(), "the host sees nothing of the old room either");

    // the game crashed: a new sign-in with a new session gets the 82 with the session handed over, no room traffic
    let old_session = w.clients[1].session;
    let mut c = w.e.client_of("player1");
    assert_ne!(c.session, old_session);
    w.e.sign_in(&mut c);
    let hs = w.e.handshake(&mut c);
    assert_eq!(connect_msg(&hs), Some(first), "the seat's 82 after the new sign-in");
    assert_eq!(room_traffic(&hs), Vec::<u8>::new());
    assert_eq!(w.fake.rejoin_sessions.lock().unwrap().last(), Some(&(b, Some(c.session))), "the new session to the match server");
    // while his seat is kept he is in a match: no new room (the game's own text)
    c.send(Request::Other { id: 0x62, payload: Custom62::Create { kind: 0 }.encode() });
    w.e.pump(&mut [&mut c]);
    let m = c.take();
    assert!(m.iter().any(|x| matches!(x, S2c::Denied(d) if matches!(&d.body, DeniedBody::Text(t) if t == "st_custom_match_error_player_already_in_match"))), "{m:?}");
    assert!(w.e.server.registry.rooms.is_empty());

    // back in the match: no 82 and no room traffic any more, still in the same match
    w.fake.rejoined.lock().unwrap().push((MATCH_ID, b));
    w.e.pump(&mut [&mut c]);
    let m = c.take();
    assert!(connect_msg(&m).is_none());
    assert_eq!(room_traffic(&m), Vec::<u8>::new());
    assert_eq!(w.e.server.registry.online.match_of(b), Some(MATCH_ID));
}

/// Rejoin: a dropped player who never comes back is paid nothing with the server's winner and not fined (a drop can be a
/// crash), but gets the results screen (every reward 0) and `85 00` at the end. A player who declines the rejoin (31)
/// leaves for good and is released at once.
#[test]
fn a_dropped_player_who_does_not_come_back_gets_the_results_unpaid_and_a_decline_releases() {
    game_data::require_tables!();
    let (map, mode, weather, n) = smallest_row();
    let mut w = setup(n, Some((map, mode, weather)));
    queue_all(&mut w);
    for c in w.clients.iter_mut() {
        c.take();
    }
    let (a0, a1) = (w.clients[0].account, w.clients[1].account);
    let before = silver(&w, 0);
    *w.fake.allow_rejoin.lock().unwrap() = true;
    w.fake.dropped.lock().unwrap().push(Left { match_id: MATCH_ID, account: a0, by_menu: false });
    w.fake.dropped.lock().unwrap().push(Left { match_id: MATCH_ID, account: a1, by_menu: false });
    pump(&mut w);
    // client 1 declines with 31: the slot leaves, he is free at once
    w.clients[1].take();
    w.clients[1].send(Request::QueueCancel { order: 0 });
    pump(&mut w);
    assert_eq!(*w.fake.declined.lock().unwrap(), vec![(MATCH_ID, a1)]);
    assert!(w.clients[1].take().iter().any(|m| matches!(m, S2c::QueueState(q) if q.state == 0)), "released");
    assert_eq!(w.e.server.registry.online.match_of(a1), None);
    let seats = w.fake.created.lock().unwrap()[0].3.clone();
    let players = seats.iter().map(|s| EndedPlayer { account: s.0, team: s.1, entered: true, left_by_menu: false }).collect();
    w.fake.ended.lock().unwrap().push(Ended { match_id: MATCH_ID, duration: Duration::from_secs(600), players, stats: None, winner: Some(0) });
    for c in w.clients.iter_mut() {
        c.take();
    }
    pump(&mut w);
    let got = w.clients[0].take();
    assert!(got.iter().any(|x| x.id() == 0x97), "the results screen");
    assert!(got.iter().any(|m| matches!(m, S2c::QueueState(q) if q.state == 0)), "85 00 at the end");
    let p0 = w.e.store.load_profile(a0).unwrap();
    assert_eq!((p0.silver, p0.penalty.count), (before, 0), "not paid, not fined");
    assert!(!w.clients[1].take().iter().any(|x| x.id() == 0x97), "the decliner left: no results screen");
}

/// Rejoin: when the match server will not keep the slot after all, the dropped player is released like a leaver.
#[test]
fn a_drop_without_a_rejoin_releases() {
    game_data::require_tables!();
    let (map, mode, weather, n) = smallest_row();
    let mut w = setup(n, Some((map, mode, weather)));
    queue_all(&mut w);
    for c in w.clients.iter_mut() {
        c.take();
    }
    let a0 = w.clients[0].account;
    w.fake.dropped.lock().unwrap().push(Left { match_id: MATCH_ID, account: a0, by_menu: false });
    pump(&mut w);
    let got = w.clients[0].take();
    assert!(connect_msg(&got).is_none());
    assert!(got.iter().any(|m| matches!(m, S2c::QueueState(q) if q.state == 0)));
    assert_eq!(w.e.server.registry.online.match_of(a0), None);
}

/// The leaver's fine shows the client's own notice: B1 type 0x19, text `[0][silver]` (`st_leave_match_fine_received`,
/// 0x2001C458B).
#[test]
fn a_menu_leavers_fine_comes_with_the_clients_own_notice() {
    game_data::require_tables!();
    let (map, mode, weather, n) = smallest_row();
    let mut w = setup(n, Some((map, mode, weather)));
    queue_all(&mut w);
    for c in w.clients.iter_mut() {
        c.take();
    }
    let a1 = w.clients[1].account;
    w.e.store.update_profile(a1, |p| { p.silver = 1_000_000; Ok::<_, ()>(()) }).unwrap();
    w.fake.left.lock().unwrap().push(Left { match_id: MATCH_ID, account: a1, by_menu: true });
    pump(&mut w);
    let seats = w.fake.created.lock().unwrap()[0].3.clone();
    let players = seats.iter().map(|s| EndedPlayer { account: s.0, team: s.1, entered: true, left_by_menu: s.0 == a1 }).collect();
    w.fake.ended.lock().unwrap().push(Ended { match_id: MATCH_ID, duration: Duration::from_secs(600), players, stats: None, winner: Some(0) });
    for c in w.clients.iter_mut() {
        c.take();
    }
    pump(&mut w);
    let fine = 1_000_000 - silver(&w, 1);
    assert!(fine > 0, "fined");
    let notes: Vec<SystemMessage> = w.clients[1].take().into_iter().filter_map(|x| if let S2c::SystemMessage(s) = x { (s.kind == 0x19).then_some(s) } else { None }).collect();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].text, format!("[0][{fine}]"));
}

/// Rejoin after the game was closed or crashed: the old lobby connection goes silent (the 82 sent at the drop is lost),
/// the client's own lobby reconnect (same session) and later a fresh sign-in (new session) are accepted as usual
/// (lobby-refuse: FD only for a bad 2E or none within 15 s), the loading messages carry no 82 (the lobby screen is not up
/// yet), and the seat's 82 comes with the answer to the first request afterwards (here the lobby-load 5B 02).
#[test]
fn a_dropped_player_signing_in_again_gets_his_82_after_his_first_request() {
    game_data::require_tables!();
    let (map, mode, weather, n) = smallest_row();
    let mut w = setup(n, Some((map, mode, weather)));
    queue_all(&mut w);
    let first = connect_msg(&w.clients[0].take()).expect("the start's 82");
    for c in w.clients.iter_mut() {
        c.take();
    }
    *w.fake.allow_rejoin.lock().unwrap() = true;
    let old = w.clients.remove(0);
    let a0 = old.account;
    w.fake.dropped.lock().unwrap().push(Left { match_id: MATCH_ID, account: a0, by_menu: false });
    {
        let World { e, clients, .. } = &mut w;
        let mut refs: Vec<&mut Client> = clients.iter_mut().collect();
        e.wait(&mut refs, 20);
    }
    for (label, mut c) in [("reconnect (same session)", w.e.reconnect(&old)), ("fresh sign-in (new session)", w.e.client_of("player0"))] {
        let pushes = w.e.sign_in(&mut c);
        assert!(pushes.iter().any(|m| m.id() == 0x80), "{label}: signed in, not refused");
        assert!(connect_msg(&pushes).is_none(), "{label}: no 82 with the loading messages");
        let got = w.e.handshake(&mut c);
        assert_eq!(connect_msg(&got), Some(first.clone()), "{label}: the seat's 82 after the first request");
        let again = w.e.handshake(&mut c);
        assert!(connect_msg(&again).is_none(), "{label}: once per sign-in");
        assert_eq!(w.e.server.registry.online.match_of(a0), Some(MATCH_ID));
    }
}

/// Arms Race (gun race, mode 3; docs/notes/gun-race.md): all versus all. The match is created with every seat on its
/// own team (its record index); the match server's winner byte is arena.lua's `player_id` of the race winner: that
/// seat wins (97 winner byte = his team), the others lose, places by score over the whole table; 9 (the 10-minute end)
/// is a played match nobody won; the relay's "no result" (the pre-game cancel) pays nobody. Nothing is charged after
/// the match (the race kits are the match's). The play gate keeps bit 3 off until the gun race is open.
#[test]
fn a_gun_race_is_all_versus_all_and_pays_the_race_winner() {
    game_data::require_tables!();
    let gd = game_data::testing::tables();
    let row = gd.gunrace.row.clone();
    for (winner, want) in [(1u8, Some(1u8)), (game_data::gunrace::NO_WINNER, Some(9)), (match_server_no_result(), None)] {
        let mut w = setup(3, Some((row.map, row.mode, row.variation)));
        queue_all(&mut w);
        let seats = w.fake.created.lock().unwrap()[0].3.clone();
        assert_eq!(seats.iter().map(|s| s.1).collect::<Vec<_>>(), vec![0, 1, 2], "each seat its own team");
        for c in w.clients.iter_mut() {
            c.take();
        }
        let team_of = |a: i64| seats.iter().find(|s| s.0 == a).unwrap().1;
        let players = w.clients.iter().map(|c| EndedPlayer { account: c.account, team: team_of(c.account), entered: true, left_by_menu: false }).collect();
        // score = 10 x race progress (rank progress +10, rank loss -10): the winner (team 1) leads
        let stats: Vec<lobby_server::EndedStat> = w
            .clients
            .iter()
            .map(|c| {
                let t = team_of(c.account);
                lobby_server::EndedStat { account: c.account, kills: [5, 27, 12][usize::from(t)], deaths: 4, score: [50, 270, 120][usize::from(t)], ..Default::default() }
            })
            .collect();
        let before: Vec<u32> = (0..3).map(|i| silver(&w, i)).collect();
        w.fake.ended.lock().unwrap().push(Ended { match_id: MATCH_ID, duration: Duration::from_secs(400), players, stats: Some((stats, [0, 0])), winner: Some(winner) });
        let World { e, clients, .. } = &mut w;
        pump_all(e, clients);
        let inbox: Vec<Vec<S2c>> = w.clients.iter_mut().map(|c| c.take()).collect();
        for (i, m) in inbox.iter().enumerate() {
            let r97 = m.iter().find_map(|x| if let S2c::MatchResult(r) = x { Some(r.clone()) } else { None });
            match want {
                None => {
                    assert!(r97.is_none(), "no result: nothing paid ({winner:#x})");
                    assert!(!m.iter().any(|x| x.id() == 0x8B));
                }
                Some(wb) => {
                    let r = r97.expect("97");
                    assert_eq!((r.winner_team, r.mode), (wb, 3));
                    let mut by_team: Vec<(u8, u8)> = r.players.iter().map(|p| (p.team, p.flag)).collect();
                    by_team.sort();
                    assert_eq!(by_team.iter().map(|x| x.0).collect::<Vec<_>>(), vec![0, 1, 2]);
                    let place = |t: u8| by_team.iter().find(|x| x.0 == t).unwrap().1;
                    assert!(place(1) < place(2) && place(2) < place(0), "places by score over the whole table: {by_team:?}");
                    assert!(r.spent.iter().all(|l| l.is_empty()), "no after-match charge in the gun race");
                    assert!(silver(&w, i) >= before[i], "paid, nothing taken");
                }
            }
        }
    }
    // the play gate: every mode served, the gun race still locked
    let gate = |m: &[S2c]| m.iter().find_map(|x| if let S2c::PlayGate(g) = x { Some(g.mode_mask) } else { None });
    let mut w = setup_with(0, 2, None);
    let mut c = w.e.client("gate_gun");
    let mask = gate(&w.e.sign_in(&mut c)).unwrap();
    assert_eq!(mask & (1 << 3) != 0, lobby_server::queue::GUNRACE_OPEN);
}

/// The relay's gun race "no result" byte (match_server::GUNRACE_NO_RESULT; the lobby does not depend on the match server).
fn match_server_no_result() -> u8 {
    0xFE
}
