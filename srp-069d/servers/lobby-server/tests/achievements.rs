//! Achievements over the wire: B9 at login, `55 04`, progress and rewards after a finished local match.

mod support;

use std::sync::Arc;

use lobby_server::{LobbyConfig, MinimalMatches};
use protocol::c2s::{Event53, Play5B, Request};
use protocol::s2c::achievements::AchievementsB9;
use protocol::s2c::*;
use protocol::Decode;
use support::*;

fn env() -> (tempfile::TempDir, Env) {
    let dir = tempfile::tempdir().unwrap();
    let e = Env::with(&dir.path().join("a.sqlite"), LobbyConfig::default(), Arc::new(MinimalMatches));
    (dir, e)
}

fn b9s(msgs: &[S2c]) -> Vec<AchievementsB9> {
    msgs.iter()
        .filter_map(|m| match m {
            S2c::Raw { id: 0xB9, payload } => Some(AchievementsB9::from_bytes(payload).unwrap()),
            _ => None,
        })
        .collect()
}

#[test]
fn login_sends_every_achievement_and_gives_the_rewards_of_reached_ranks() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    let msgs = e.sign_in(&mut c);
    let b = b9s(&msgs);
    assert_eq!(b.len(), 1);
    assert_eq!(b[0].entries.len(), 79);
    assert!(b[0].entries.iter().all(|x| x.lo == x.hi), "no popups at login");
    let p = e.store.load_profile(c.account).unwrap();
    // the starter profile holds 200 000 points in every faction: rank 6, so the first states of 408..412 are reached
    let rank = |id: u16| b[0].entries.iter().find(|x| x.id == id).unwrap().value;
    assert_eq!(rank(409), game_data::levels::faction_level(200_000));
    assert!(p.items.iter().any(|i| i.dict_id == 237), "scavenger rank 1 reward (char_decal_2_1) is in the inventory");
    // a second sign-in changes nothing
    let items = p.items.len();
    let mut c2 = e.client_of("test");
    e.sign_in(&mut c2);
    assert_eq!(e.store.load_profile(c.account).unwrap().items.len(), items);
}

#[test]
fn window_answers_with_stored_progress_and_category_scores() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    e.sign_in(&mut c);
    c.send_raw(0x55, &[4]);
    e.pump(&mut [&mut c]);
    let m = c.take();
    let rs: Vec<&Reply> = m.iter().filter_map(|m| if let S2c::Reply(r) = m { Some(r) } else { None }).collect();
    assert_eq!(rs.len(), 2);
    let Reply::Achievements(Achievements55::Other { sub: 2, data }) = rs[0] else { panic!("{:?}", rs[0]) };
    assert_eq!(u32::from_le_bytes(data[..4].try_into().unwrap()), 79);
    let Reply::Achievements(Achievements55::List { entries }) = rs[1] else { panic!("{:?}", rs[1]) };
    assert_eq!(entries.iter().map(|e| e.0).collect::<Vec<_>>(), [2, 4, 6]);
    assert!(entries.iter().find(|e| e.0 == 4).unwrap().1 > 0, "rank states of the starter profile score");
}

#[test]
fn a_won_online_match_counts_and_a_local_bots_match_counts_nothing() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    e.sign_in(&mut c);
    e.handshake(&mut c);
    // a local training match with bots counts for nothing (CHOICE)
    for _ in 0..2 {
        c.send(Request::Play(Play5B::Play { map: 4, mode: 1, weather: 2 }));
        e.pump(&mut [&mut c]);
        c.take();
        e.wait(&mut [&mut c], 31);
        c.send(Request::Events(vec![Event53 { id: 5, x: 0, value: 0, time: 0 }]));
        e.pump(&mut [&mut c]);
        assert!(b9s(&c.take()).iter().all(|b| b.entries.iter().all(|x| x.id != 611)));
    }
    assert_eq!(value(&e, c.account, 611), 0);
    // five won online matches in mode 1 (tdm): the first state of "win tdm" (611) needs 5 (stats::record_match is the
    // entry point online.rs calls)
    let acc = c.account;
    let mut won = game_data::MatchFacts::local(4, 1, true, true, 1);
    won.match_type = Some(1);
    for _ in 0..5 {
        let mut cx = cx_of(&mut e);
        lobby_server::stats::record_match(&mut cx, acc, &won);
    }
    assert_eq!(value(&e, acc, 611), 5);
    // an online match left early (not finished) counts nothing
    let mut left = won.clone();
    left.finished = false;
    let mut cx = cx_of(&mut e);
    lobby_server::stats::record_match(&mut cx, acc, &left);
    assert_eq!(value(&e, acc, 611), 5);
}

// ---- lobby events (achievements.rs `Event`) ----

use accounts::{AccountId, HeroResult, PveRun, PveRunPlayer, QuestSlot, UID_BASE};
use lobby_server::dispatch::Ctx;
use protocol::c2s::{Inventory45 as Q45, Shop46 as Q46};

/// The B9 entry of achievement `id` in the messages (the last one when several were pushed).
fn entry(msgs: &[S2c], id: u16) -> Option<protocol::s2c::achievements::AchEntry> {
    b9s(msgs).iter().flat_map(|b| b.entries.iter()).filter(|x| x.id == id).last().copied()
}

fn value(e: &Env, a: AccountId, id: u16) -> u32 {
    e.store.load_profile(a).unwrap().ach.get(&id).map_or(0, |x| x.value)
}

fn cx_of(e: &mut Env) -> Ctx<'_> {
    let lobby_server::Server { lobby, registry, .. } = &mut e.server;
    Ctx { lobby, registry, now: std::time::Instant::now(), fx: vec![], kick: vec![] }
}

#[test]
fn an_upgrade_and_a_changed_modifier_push_their_achievements() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    e.sign_in(&mut c);
    e.store.update_profile(c.account, |p| Ok::<_, ()>((p.gold = 100_000, p.parts = 100_000))).unwrap();
    // 45 07 with gold, 45 08: the starter Mosin (rank 0) gets its first modifier: green, the first state of 420
    c.send(Request::Inventory(Q45::Craft { item: UID_BASE, pay: 0xFF }));
    e.pump(&mut [&mut c]);
    let m = c.take();
    let S2c::Reply(Reply::Inventory(Inventory45::Craft { result: 1, offer, .. })) = &m[0] else { panic!("{m:?}") };
    let pick = offer[0];
    c.send(Request::Inventory(Q45::CraftPick { item: UID_BASE, index: 0 }));
    e.pump(&mut [&mut c]);
    let m = c.take();
    assert!(matches!(m[0], S2c::Reply(Reply::Inventory(Inventory45::CraftDone { .. }))), "the answer comes first");
    let x = entry(&m, 420).expect("420 pushed");
    assert_eq!((x.value, x.lo, x.hi), (1, 0, 1));
    assert_eq!(value(&e, c.account, 404), 0);
    // 45 05 / 45 06: changing that modifier on the green item = the first state of 404
    c.send(Request::Inventory(Q45::Reroll { item: UID_BASE, group: pick.0, grade: pick.1, pay: 0 }));
    e.pump(&mut [&mut c]);
    let m = c.take();
    let S2c::Reply(Reply::Inventory(Inventory45::RerollOptions { options, .. })) = &m[0] else { panic!("{m:?}") };
    // the first row is free with spare parts; a row that keeps the pair would change nothing
    let row = options.iter().position(|o| *o != pick).expect("a different modifier is offered");
    let new = options[row];
    let gold = if row == 0 { 0 } else { game_data::testing::tables().craft.reroll_premium_price(107) };
    c.send(Request::Inventory(Q45::RerollPick { item: UID_BASE, old_group: pick.0, old_grade: pick.1, row: row as u32, gold }));
    e.pump(&mut [&mut c]);
    let x = entry(&c.take(), 404).expect("404 pushed");
    assert_eq!((x.value, x.lo, x.hi), (1, 0, 1));
    // a pick that keeps the modifier (no offer stored) changes nothing
    c.send(Request::Inventory(Q45::RerollPick { item: UID_BASE, old_group: new.0, old_grade: new.1, row: 0, gold: 0 }));
    e.pump(&mut [&mut c]);
    assert!(entry(&c.take(), 404).is_none());
    assert_eq!(value(&e, c.account, 404), 1);
}

#[test]
fn a_decal_put_on_armour_reaches_customise() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    e.sign_in(&mut c);
    let g = game_data::testing::tables();
    let (torso, decal) = e
        .store
        .update_profile(c.account, |p| {
            let torso = p.items.iter().find(|i| g.items.category(i.dict_id) == 3).map(|i| i.uid).unwrap();
            Ok::<_, ()>((torso, p.add_item(718, 718)))
        })
        .unwrap();
    c.send(Request::Inventory(Q45::RelocateUpgrade { a: torso, b: decal, c: 1, d: 0 }));
    e.pump(&mut [&mut c]);
    let x = entry(&c.take(), 403).expect("403 pushed");
    assert_eq!((x.value, x.lo, x.hi), (1, 0, 1));
    // a refused attach (a weapon target) counts nothing; a second attach changes nothing
    c.send(Request::Inventory(Q45::RelocateUpgrade { a: UID_BASE, b: decal, c: 0, d: 0 }));
    c.send(Request::Inventory(Q45::RelocateUpgrade { a: torso, b: decal, c: 0, d: 0 }));
    e.pump(&mut [&mut c]);
    assert!(entry(&c.take(), 403).is_none());
}

#[test]
fn the_last_find_of_the_treasure_hunt_reaches_406() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    e.sign_in(&mut c);
    e.store.update_profile(c.account, |p| Ok::<_, ()>(p.gold = 1_000_000)).unwrap();
    let n = e.store.load_profile(c.account).unwrap().hunt.entries.len();
    assert!(n > 1, "a hunt page exists after the sign-in");
    for i in 0..n {
        c.send(Request::Shop(Q46::ItemSearch { a: 1 }));
        e.pump(&mut [&mut c]);
        let m = c.take();
        assert!(m.iter().any(|m| matches!(m, S2c::Reply(Reply::Shop(Shop46::Search { .. })))), "search {i} answered: {m:?}");
        match entry(&m, 406) {
            Some(x) => {
                assert_eq!(i, n - 1, "only the last find completes the hunt");
                assert_eq!((x.value, x.lo, x.hi), (1, 0, 1));
            }
            None => assert!(i < n - 1, "the last find pushes 406"),
        }
    }
    // the page is exhausted: a further search is refused and counts nothing
    c.send(Request::Shop(Q46::ItemSearch { a: 1 }));
    e.pump(&mut [&mut c]);
    assert!(entry(&c.take(), 406).is_none());
    assert_eq!(value(&e, c.account, 406), 1);
}

#[test]
fn completed_daily_quests_count_for_620() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    e.sign_in(&mut c);
    e.handshake(&mut c);
    let gd = game_data::testing::tables();
    let q = |name: &str| gd.daily_quests.quests.iter().find(|q| q.name == name).unwrap().index;
    let slots = vec![
        QuestSlot { uid: 101, dict: q("st_daily_quest_win_games"), progress: 0, done: false },
        QuestSlot { uid: 102, dict: q("st_daily_quest_play_games"), progress: 2, done: false },
        QuestSlot { uid: 103, dict: q("st_daily_quest_event_kill"), progress: 0, done: false },
    ];
    e.store.update_profile(c.account, |p| Ok::<_, ()>(p.daily.slots = slots)).unwrap();
    // a won online match (stats::record_match, the entry point online.rs calls; a local bots match counts nothing)
    let acc = c.account;
    let mut won = game_data::MatchFacts::local(4, 1, true, true, 1);
    won.match_type = Some(1);
    let msgs = {
        let mut cx = cx_of(&mut e);
        lobby_server::stats::record_match(&mut cx, acc, &won);
        cx.fx.into_iter().map(|f| f.msg).collect::<Vec<_>>()
    };
    // the win completes two quests (win 1 game, play 3 games); the kill quest has no fact
    let x = entry(&msgs, 620).expect("620 pushed");
    assert_eq!((x.value, x.lo, x.hi), (2, 0, 0));
    assert_eq!(value(&e, c.account, 620), 2);
}

fn pve_run(match_id: u32, players: &[AccountId], success: bool, chests: u8) -> PveRun {
    PveRun {
        match_id,
        mission: 15,
        difficulty: 0,
        success,
        time_secs: 900,
        chests_found: chests,
        chests_total: 10,
        started_at: 1000,
        players: players.iter().enumerate().map(|(i, a)| PveRunPlayer { account: *a, score: 100, kills: 1, deaths: 0, sort: i as u8, premium: false }).collect(),
    }
}

#[test]
fn a_won_pve_mission_counts_once_per_run() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut a = e.client("alpha");
    let mut b = e.client("bravo");
    e.sign_in(&mut a);
    e.sign_in(&mut b);
    let (aa, ba) = (a.account, b.account);
    let now = 20_000 * 86_400;
    let mut cx = cx_of(&mut e);
    lobby_server::pve::record_run_notify(&mut cx, &pve_run(1, &[aa, ba], true, 10), now).unwrap();
    let fx = std::mem::take(&mut cx.fx);
    let to_a: Vec<S2c> = fx.iter().filter(|x| x.to == aa).map(|x| x.msg.clone()).collect();
    let x = entry(&to_a, 630).expect("630 pushed to alpha");
    assert_eq!((x.value, x.lo, x.hi), (1, 0, 1));
    assert_eq!(entry(&to_a, 629).unwrap().value, 1);
    // the same match id again is a second run (a PvE match id is its maze's world id; the lobby sends one result per
    // match): it counts; a lost mission and a win with 2 caches change nothing; 3 caches count for 629 only
    lobby_server::pve::record_run_notify(&mut cx, &pve_run(1, &[aa, ba], true, 10), now).unwrap();
    lobby_server::pve::record_run_notify(&mut cx, &pve_run(2, &[aa], false, 10), now).unwrap();
    lobby_server::pve::record_run_notify(&mut cx, &pve_run(3, &[aa], true, 2), now).unwrap();
    lobby_server::pve::record_run_notify(&mut cx, &pve_run(4, &[aa], true, 3), now).unwrap();
    drop(cx);
    assert_eq!((value(&e, aa, 629), value(&e, aa, 630)), (3, 2));
    assert_eq!((value(&e, ba, 629), value(&e, ba, 630)), (2, 2));
}

#[test]
fn hero_titles_reach_the_hero_achievements() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let c = e.client("test");
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
    // a ranked result an hour ago: alone in the window, first place in all three nominations at the next calculation
    e.store.hero_add_result(&HeroResult { account: c.account, at: t - 3600, group: 0, score: 400, killstreak: 3, won: true }).unwrap();
    let mut cx = cx_of(&mut e);
    lobby_server::hero::tick(&mut cx);
    let fx = std::mem::take(&mut cx.fx);
    drop(cx);
    let msgs: Vec<S2c> = fx.into_iter().filter(|x| x.to == c.account).map(|x| x.msg).collect();
    for id in [623, 624, 625] {
        let x = entry(&msgs, id).unwrap_or_else(|| panic!("{id} pushed"));
        assert_eq!((x.value, x.state, x.lo, x.hi), (3, 3, 0, 3), "first place = all three states of {id}");
    }
    // the slot is done: a second tick changes nothing
    let mut cx = cx_of(&mut e);
    lobby_server::hero::tick(&mut cx);
    assert!(cx.fx.is_empty());
}

/// The hero achievements count only titles, and titles only come from the matches the ranking counts: random / ranked,
/// equipment level 3..5 (the data's only competitive group), active players. The window (`55 04` -> `83 55 02`) shows
/// the stored states, the category total adds the state scores 10 + 20 + 30 (the data's states give no item).
#[test]
fn hero_achievements_follow_the_ranking_filter_and_reach_the_window() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    let idle = e.client("idle");
    let low = e.client("low");
    let pvp = |score: u32, inactive: bool| {
        let mut f = game_data::MatchFacts::local(4, 1, true, true, 1);
        f.match_type = Some(1);
        f.score = Some(score);
        f.max_killstreak = Some(5);
        f.inactive = Some(inactive);
        f
    };
    // own equipment levels (the hero group's key): test and idle wear a level-4 item, low keeps the starter kit
    let gd0 = game_data::testing::tables();
    let lvl4 = gd0.items.iter().filter(|i| i.item_level == 4 && !i.is_stackable).map(|i| i.dict_id).next().unwrap();
    for a in [c.account, idle.account] {
        e.store
            .update_profile(a, |p| {
                let uid = p.add_item(lvl4, 1);
                p.slots.insert(0, uid);
                Ok::<_, ()>(())
            })
            .unwrap();
    }
    for (acc, f) in [(idle.account, pvp(990, true)), (low.account, pvp(980, false)), (c.account, pvp(400, false))] {
        let mut cx = cx_of(&mut e);
        lobby_server::stats::record_match(&mut cx, acc, &f);
    }
    // the next 30-minute slot ranks the stored results: only "test" holds titles (first place in all three)
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
    let gd = e.server.lobby.data.clone();
    let titles = lobby_server::hero::recalc_titles(&e.store, &gd, t + 1800);
    assert_eq!(titles.iter().map(|x| (x.account, x.nomination, x.place)).collect::<Vec<_>>(), [(c.account, 1, 1), (c.account, 2, 1), (c.account, 3, 1)]);
    let items_before = e.store.load_profile(c.account).unwrap().items.len();
    let ach = lobby_server::hero::title_achievements(&e.store, &gd, true, &titles);
    assert_eq!(ach.len(), 1, "one account changed");
    assert_eq!(ach[0].1.iter().map(|x| (x.id, x.value)).collect::<Vec<_>>(), [(623, 3), (624, 3), (625, 3)]);
    for a in [idle.account, low.account] {
        assert_eq!([623, 624, 625].map(|id| value(&e, a, id)), [0; 3]);
    }
    assert_eq!(e.store.load_profile(c.account).unwrap().items.len(), items_before, "no reward item (the states' rewards are 0)");
    // the window: 83 55 02 = u32 n, n x {u16 id, u8 state, u32 value}
    e.sign_in(&mut c);
    c.send_raw(0x55, &[4]);
    e.pump(&mut [&mut c]);
    let m = c.take();
    let rs: Vec<&Reply> = m.iter().filter_map(|m| if let S2c::Reply(r) = m { Some(r) } else { None }).collect();
    let Reply::Achievements(Achievements55::Other { sub: 2, data }) = rs[0] else { panic!("{:?}", rs[0]) };
    let n = u32::from_le_bytes(data[..4].try_into().unwrap()) as usize;
    let rows: Vec<(u16, u8, u32)> = (0..n)
        .map(|i| {
            let o = 4 + 7 * i;
            (u16::from_le_bytes([data[o], data[o + 1]]), data[o + 2], u32::from_le_bytes(data[o + 3..o + 7].try_into().unwrap()))
        })
        .collect();
    for id in [623u16, 624, 625] {
        assert_eq!(rows.iter().find(|r| r.0 == id).copied(), Some((id, 3, 3)), "{id} in 83 55 02");
    }
    let Reply::Achievements(Achievements55::List { entries }) = rs[1] else { panic!("{:?}", rs[1]) };
    let cat6 = entries.iter().find(|x| x.0 == 6).unwrap().1;
    assert!(cat6 >= 3 * 60, "category 6 holds the three hero achievements' 10 + 20 + 30: {cat6}");
}
