//! Daily quests over the wire: AB at login, progress and rewards after a finished local match, reroll, daily chest.

mod support;

use std::sync::Arc;

use accounts::QuestSlot;
use lobby_server::{LobbyConfig, MinimalMatches};
use protocol::c2s::{Event53, Play5B, Quest4A, Request};
use protocol::s2c::quests::{DailyQuestsAb, QuestEvent};
use protocol::s2c::*;
use protocol::Decode;
use support::*;

fn env() -> (tempfile::TempDir, Env) {
    let dir = tempfile::tempdir().unwrap();
    let e = Env::with(&dir.path().join("q.sqlite"), LobbyConfig { achievement_rewards: false, ..LobbyConfig::default() }, Arc::new(MinimalMatches));
    (dir, e)
}

fn abs(msgs: &[S2c]) -> Vec<DailyQuestsAb> {
    msgs.iter()
        .filter_map(|m| match m {
            S2c::Raw { id: 0xAB, payload } => Some(DailyQuestsAb::from_bytes(payload).unwrap()),
            _ => None,
        })
        .collect()
}

fn events(msgs: &[S2c]) -> Vec<QuestEvent> {
    msgs.iter()
        .filter_map(|m| match m {
            S2c::SystemMessage(s) if s.kind == 0x0E => QuestEvent::parse(&s.text),
            _ => None,
        })
        .collect()
}

fn dict_of(name: &str) -> u16 {
    game_data::testing::tables().daily_quests.quests.iter().find(|q| q.name == name).unwrap().index
}

/// A local training match with bots (counts for nothing, CHOICE).
fn play_local(e: &mut Env, c: &mut Client, won: bool) -> Vec<S2c> {
    c.send(Request::Play(Play5B::Play { map: 4, mode: 1, weather: 2 }));
    e.pump(&mut [c]);
    c.take();
    e.wait(&mut [c], 31);
    c.send(Request::Events(vec![Event53 { id: if won { 5 } else { 6 }, x: 0, value: 0, time: 0 }]));
    e.pump(&mut [c]);
    c.take()
}

/// A finished online random match: its facts go through `stats::record_match` (the entry point online.rs calls); the
/// messages for the player are returned.
fn play_match(e: &mut Env, c: &mut Client, won: bool) -> Vec<S2c> {
    let mut f = game_data::MatchFacts::local(4, 1, won, true, 1);
    f.match_type = Some(1);
    let lobby_server::Server { lobby, registry, .. } = &mut e.server;
    let mut cx = lobby_server::dispatch::Ctx { lobby, registry, now: std::time::Instant::now(), fx: vec![], kick: vec![] };
    lobby_server::stats::record_match(&mut cx, c.account, &f);
    cx.fx.into_iter().filter(|x| x.to == c.account).map(|x| x.msg).collect()
}

#[test]
fn a_local_bots_match_moves_no_quest() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    e.sign_in(&mut c);
    e.handshake(&mut c);
    let before = e.store.load_profile(c.account).unwrap().daily;
    let m = play_local(&mut e, &mut c, true);
    assert!(abs(&m).is_empty() && events(&m).is_empty(), "{m:?}");
    assert_eq!(e.store.load_profile(c.account).unwrap().daily, before);
}

#[test]
fn login_carries_three_quests_and_the_set_is_kept() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    let msgs = e.sign_in(&mut c);
    let ab = abs(&msgs);
    assert_eq!(ab.len(), 1);
    assert_eq!(ab[0].quests.len(), 3);
    assert!(ab[0].expires_in > 17 * 3600 && ab[0].expires_in <= 18 * 3600);
    assert_eq!((ab[0].free_rerolls, ab[0].reroll_allowed, ab[0].received_dict), (1, true, 0));
    let mut c2 = e.client_of("test");
    let again = abs(&e.sign_in(&mut c2));
    assert_eq!(again[0].quests, ab[0].quests, "a second sign-in keeps the set");
}

#[test]
fn a_won_match_advances_and_pays_and_the_chest_follows_the_third_quest() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    e.sign_in(&mut c);
    e.handshake(&mut c);
    let slots = vec![
        QuestSlot { uid: 101, dict: dict_of("st_daily_quest_win_games"), progress: 0, done: false },
        QuestSlot { uid: 102, dict: dict_of("st_daily_quest_play_games"), progress: 2, done: false },
        QuestSlot { uid: 103, dict: dict_of("st_daily_quest_event_kill"), progress: 0, done: true },
    ];
    e.store.update_profile(c.account, |p| Ok::<_, ()>(p.daily.slots = slots)).unwrap();
    let before = e.store.load_profile(c.account).unwrap();
    let m = play_match(&mut e, &mut c, true);
    let ev = events(&m);
    assert_eq!(ev, vec![QuestEvent::Completed { uid: 101, progress: 1 }, QuestEvent::Completed { uid: 102, progress: 3 }]);
    let after = e.store.load_profile(c.account).unwrap();
    let row = game_data::testing::tables().daily_quests.reward_row(5).clone();
    assert!(after.silver - before.silver > 2 * row.silver, "match reward plus two quest rewards");
    // the rest is the match's drop_after_combat silver roll (100..200 at match level 1, data) plus level-up silver
    assert!(after.silver - before.silver - 2 * row.silver >= 100);
    assert!(after.daily.slots.iter().all(|s| s.done));
    // the chest is open for business: one answer, the item is in the account, the AB shows it opened
    c.send(Request::Quest(Quest4A::ChestOpened { a: 0 }));
    e.pump(&mut [&mut c]);
    let m = c.take();
    let reply = m.iter().find_map(|m| if let S2c::Reply(Reply::Raw { request: 0x4A, data }) = m { Some(data.clone()) } else { None });
    let data = reply.expect("83 4A 04");
    assert_eq!(data[0], 4);
    let (item, amount) = (u16::from_le_bytes([data[1], data[2]]), u32::from_le_bytes([data[3], data[4], data[5], data[6]]));
    let ab = abs(&m);
    assert_eq!((ab[0].received_dict, u32::from(ab[0].received_amount)), (item, amount));
    // a second opening is refused: no new answer
    c.send(Request::Quest(Quest4A::ChestOpened { a: 0 }));
    e.pump(&mut [&mut c]);
    assert!(!c.take().iter().any(|m| matches!(m, S2c::Reply(Reply::Raw { request: 0x4A, .. }))));
}

#[test]
fn reroll_replaces_the_quest_for_free_once_and_a_loss_counts_as_played() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    let msgs = e.sign_in(&mut c);
    e.handshake(&mut c);
    let first = abs(&msgs).remove(0);
    let uid = first.quests[0].uid;
    c.send(Request::Quest(Quest4A::DailyReroll { quest: uid }));
    e.pump(&mut [&mut c]);
    let ab = abs(&c.take()).remove(0);
    assert_eq!(ab.free_rerolls, 0);
    assert!(ab.quests.iter().all(|q| q.uid != uid));
    assert_eq!(ab.quests[1], first.quests[1]);
    // an unknown quest id changes nothing
    c.send(Request::Quest(Quest4A::DailyReroll { quest: 424242 }));
    e.pump(&mut [&mut c]);
    assert_eq!(abs(&c.take()).remove(0).quests, ab.quests);
    // a lost match counts for "play" quests only
    let slots = vec![
        QuestSlot { uid: 201, dict: dict_of("st_daily_quest_win_games"), progress: 0, done: false },
        QuestSlot { uid: 202, dict: dict_of("st_daily_quest_play_games"), progress: 0, done: false },
        QuestSlot { uid: 203, dict: dict_of("st_daily_quest_win_games_squad"), progress: 0, done: false },
    ];
    e.store.update_profile(c.account, |p| Ok::<_, ()>(p.daily.slots = slots)).unwrap();
    let m = play_match(&mut e, &mut c, false);
    assert_eq!(events(&m), vec![QuestEvent::Progress { uid: 202, progress: 1 }]);
}
