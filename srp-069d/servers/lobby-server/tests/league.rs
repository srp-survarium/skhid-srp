//! Ranked leagues over the wire: 9F at login, the `4B` window requests, the rating update from the match facts, the
//! season end with its token reward, and the league shop. Rules and layouts: docs/notes/league.md.

mod support;

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use accounts::{AccountId, Currency};
use game_data::{LeagueSchedule, LeagueSeason, MatchFacts};
use lobby_server::dispatch::Ctx;
use lobby_server::{stats, Cross, LobbyConfig, MinimalMatches};
use protocol::c2s::{League4B, Request, Shop46 as Q46};
use protocol::s2c::*;
use support::*;

fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64
}

fn env_with(schedule: LeagueSchedule) -> (tempfile::TempDir, Env) {
    let dir = tempfile::tempdir().unwrap();
    let cfg = LobbyConfig { league: schedule, achievement_rewards: false, login_calendar: None, ..LobbyConfig::default() };
    let e = Env::with(&dir.path().join("l.sqlite"), cfg, Arc::new(MinimalMatches));
    (dir, e)
}

fn running() -> LeagueSchedule {
    LeagueSchedule { seasons: vec![LeagueSeason { number: 3, start: now() - 100, end: now() + 3600 }], tokens: vec![] }
}

/// A ranked result of `account` (win / loss and the place in his team).
fn ranked(e: &mut Env, account: AccountId, won: bool, place: u8) -> Vec<Cross> {
    let mut f = MatchFacts::local(4, 1, won, true, 1);
    f.match_type = Some(2);
    f.team_place = Some(place);
    let lobby_server::Server { lobby, registry, .. } = &mut e.server;
    let mut cx = Ctx { lobby, registry, now: std::time::Instant::now(), fx: vec![], kick: vec![] };
    stats::record_match(&mut cx, account, &f);
    // only the league messages (achievements and quests answer a match too)
    cx.fx.into_iter().filter(|c| matches!(c.msg, S2c::League(_))).collect()
}

fn nine_f(msgs: &[S2c]) -> LeagueInfo {
    msgs.iter()
        .find_map(|m| match m {
            S2c::League(League9F::Info(i)) => Some(i.clone()),
            _ => None,
        })
        .expect("9F info")
}

fn money(msgs: &[S2c]) -> Money {
    msgs.iter()
        .find_map(|m| match m {
            S2c::Money(m) => Some(m.clone()),
            _ => None,
        })
        .unwrap()
}

fn league_reply(msgs: Vec<S2c>) -> LeagueReply {
    msgs.into_iter()
        .find_map(|m| match m {
            S2c::Reply(Reply::League(r)) => Some(r),
            _ => None,
        })
        .expect("a 4B reply")
}

fn month_year_of(t: i64) -> (u8, u16) {
    lobby_server::league::month_year(t)
}

#[test]
fn without_configured_seasons_the_calendar_month_runs_and_ranked_is_open() {
    game_data::require_tables!();
    let (_d, mut e) = env_with(LeagueSchedule::default());
    let mut c = e.client("test");
    let msgs = e.sign_in(&mut c);
    let i = nine_f(&msgs);
    let month = LeagueSeason::month_at(now()).unwrap();
    assert_eq!((i.s, i.a, i.flag), (month.number, 0xFF, 1), "the monthly season runs: S = its number, ranked available");
    let st = i.state.expect("the player's state of the running season");
    assert_eq!((st.lv, st.w), (1, 0));
    assert_eq!(i64::from(st.id), (month.end - now()) / 60, "minutes left until the next 1st");
    // a ranked result counts in the monthly season
    ranked(&mut e, c.account, true, 0);
    let l = e.store.load_profile(c.account).unwrap().league;
    assert_eq!((l.season, l.league, l.points), (month.number, 1, 4));
}

#[test]
fn a_finished_month_is_in_the_history_after_the_rollover() {
    game_data::require_tables!();
    let (_d, mut e) = env_with(LeagueSchedule::default());
    let mut c = e.client("test");
    // the account played the previous calendar month (league 3 at most), the monthly season has rolled over since
    let this = LeagueSeason::month_at(now()).unwrap();
    let prev = LeagueSeason::month_at(this.start - 1).unwrap();
    assert_eq!(prev.number + 1, this.number);
    e.store
        .update_profile(c.account, |p| {
            p.league.season = prev.number;
            p.league.league = 3;
            p.league.max_league = 3;
            p.league.matches = 5;
            Ok::<_, ()>(())
        })
        .unwrap();
    let msgs = e.sign_in(&mut c);
    assert_eq!(money(&msgs).league, 80, "the token table: 80 for league 3");
    let p = e.store.load_profile(c.account).unwrap();
    assert_eq!(p.league.history.len(), 1);
    let h = p.league.history[0];
    assert_eq!((h.season, h.max_league, h.closed_at), (prev.number, 3, prev.end), "closed at the next 1st");
    // the history window (4B 04) shows that month: {month, year, highest league}
    e.handshake(&mut c);
    c.take();
    c.send(Request::League(League4B::History { public_id: accounts::public_id(c.account) }));
    e.pump(&mut [&mut c]);
    let LeagueReply::History { entries, .. } = league_reply(c.take()) else { panic!() };
    let (m, y) = month_year_of(prev.start);
    assert_eq!(entries.len(), 1);
    assert_eq!((entries[0].a, entries[0].b, entries[0].c), (m, y, 3));
}

#[test]
fn between_configured_seasons_ranked_is_off_but_the_level_table_is_sent_and_results_are_ignored() {
    game_data::require_tables!();
    // configured seasons (srp: [[league.seasons]]) replace the monthly seasons: a schedule whose only season is over
    let sched = LeagueSchedule { seasons: vec![LeagueSeason { number: 1, start: now() - 7200, end: now() - 3600 }], tokens: vec![] };
    let (_d, mut e) = env_with(sched);
    let mut c = e.client("test");
    let msgs = e.sign_in(&mut c);
    let i = nine_f(&msgs);
    assert_eq!((i.s, i.a, i.flag), (0, 0xFF, 0), "no season: S = 0, no season-final notice");
    assert!(i.state.is_none());
    assert_eq!(i.levels.len(), 10);
    assert_eq!(i.levels[0], (1, 101, 1), "league 1: reach group 1, season-end group 101");
    assert_eq!(i.x, 3, "minimum equipment tier of the data");
    let end10 = i.rewards.iter().find(|g| g.id == 110).unwrap();
    assert_eq!(end10.items, vec![(1356, 1000, 0)], "season end pays league tokens (dict 1356)");
    assert_eq!(money(&msgs).league, 0);
    // a ranked result outside a season changes nothing
    let fx = ranked(&mut e, c.account, true, 0);
    assert!(fx.is_empty());
    assert_eq!(e.store.load_profile(c.account).unwrap().league, Default::default());
}

#[test]
fn ranked_results_move_points_and_promote_and_the_window_shows_them() {
    game_data::require_tables!();
    let (_d, mut e) = env_with(running());
    let mut c = e.client("test");
    let msgs = e.sign_in(&mut c);
    let i = nine_f(&msgs);
    assert_eq!((i.s, i.flag), (3, 1));
    let st = i.state.unwrap();
    assert_eq!((st.w, st.lv, st.lv2), (0, 1, 1), "an account without a ranked match starts in league 1");
    assert!((59..=60).contains(&st.id), "minutes left: {}", st.id);
    // winning 3 times in first place: 4 + 4 + 4 = 12 points = league 1 done -> league 2 with 0 points
    for _ in 0..2 {
        assert!(ranked(&mut e, c.account, true, 0).is_empty());
    }
    let fx = ranked(&mut e, c.account, true, 0);
    let l = e.store.load_profile(c.account).unwrap().league;
    assert_eq!((l.season, l.league, l.points, l.max_league, l.matches), (3, 2, 0, 2, 3));
    assert_eq!(fx.len(), 1);
    let S2c::League(League9F::Notice(n)) = &fx[0].msg else { panic!("{:?}", fx[0].msg) };
    assert_eq!((n.state, n.league_up), (2, 1));
    // a loss in league 2 in the last place costs nothing (league 2 has no loss points); a win in 6th place adds 2
    ranked(&mut e, c.account, false, 7);
    ranked(&mut e, c.account, true, 5);
    // the window: 4B 01
    c.send(Request::League(League4B::Opened));
    e.pump(&mut [&mut c]);
    let LeagueReply::Window { s2, state } = league_reply(c.take()) else { panic!() };
    let st = state.unwrap();
    assert_eq!((s2, st.w, st.lv, st.lv2), (3, 2, 2, 2));
}

#[test]
fn history_and_top_players_are_real_queries() {
    game_data::require_tables!();
    let (_d, mut e) = env_with(running());
    let mut a = e.client("alpha");
    let mut b = e.client("bravo");
    e.sign_in(&mut a);
    e.sign_in(&mut b);
    for _ in 0..3 {
        ranked(&mut e, a.account, true, 0);
    }
    ranked(&mut e, b.account, true, 1);
    // top players: alpha (league 2) before bravo (league 1, 4 points)
    b.send(Request::League(League4B::TopPlayers));
    e.pump(&mut [&mut a, &mut b]);
    let LeagueReply::Top { rows, me, tail } = league_reply(b.take()) else { panic!() };
    assert_eq!(rows.iter().map(|r| r.1.as_str()).collect::<Vec<_>>(), ["alpha", "bravo"]);
    assert_eq!(rows[0].0, 12, "total points climbed: league 1 (12) + 0");
    assert_eq!((me.0, me.1.as_str(), tail), (4, "bravo", 2));
    // history of alpha: none yet (the season runs)
    let id = accounts::public_id(a.account);
    b.send(Request::League(League4B::History { public_id: id }));
    e.pump(&mut [&mut a, &mut b]);
    let LeagueReply::History { public_id, entries } = league_reply(b.take()) else { panic!() };
    assert_eq!(public_id, id, "the request's public id is echoed");
    assert!(entries.is_empty());
}

#[test]
fn a_season_end_pays_tokens_by_the_highest_league_once_and_the_final_notice_is_sent_once() {
    game_data::require_tables!();
    // season 2 ended an hour ago, season 3 runs
    let sched = LeagueSchedule {
        seasons: vec![LeagueSeason { number: 2, start: now() - 7200, end: now() - 3600 }, LeagueSeason { number: 3, start: now() - 3600, end: now() + 3600 }],
        tokens: vec![],
    };
    let (_d, mut e) = env_with(sched);
    let mut c = e.client("test");
    e.store
        .update_profile(c.account, |p| {
            p.league.season = 2;
            p.league.league = 3;
            p.league.points = 5;
            p.league.max_league = 4;
            p.league.matches = 9;
            Ok::<_, ()>(())
        })
        .unwrap();
    let msgs = e.sign_in(&mut c);
    assert_eq!(money(&msgs).league, 140, "the token table: 140 for the highest league (4)");
    let i = nine_f(&msgs);
    assert_eq!((i.a, i.b, i.val), (4, 4, 0), "season-final notice for league 4");
    assert_eq!(i.s, 3);
    let st = i.state.unwrap();
    assert_eq!(st.history.len(), 1);
    assert_eq!((st.history[0].c, st.history[0].b), (4, month_year_of(now() - 3600).1));
    assert_eq!((st.lv, st.w), (1, 0), "the new season starts at the bottom");
    let p = e.store.load_profile(c.account).unwrap();
    assert_eq!((p.league.tokens, p.league.season, p.league.matches), (140, 0, 0));
    // a second sign-in: tokens are not paid again and the notice is gone
    let mut c2 = e.client_of("test");
    let msgs = e.sign_in(&mut c2);
    assert_eq!(money(&msgs).league, 140);
    assert_eq!(nine_f(&msgs).a, 0xFF);
}

#[test]
fn month_year_is_the_calendar() {
    assert_eq!(month_year_of(0), (1, 1970));
    assert_eq!(month_year_of(1_709_164_800), (2, 2024)); // 2024-02-29
    assert_eq!(month_year_of(1_709_251_200), (3, 2024)); // 2024-03-01
    assert_eq!(month_year_of(1_767_225_599), (12, 2025));
}

#[test]
fn league_items_are_bought_with_tokens() {
    game_data::require_tables!();
    let (_d, mut e) = env_with(LeagueSchedule::default());
    let mut c = e.client("test");
    e.sign_in(&mut c);
    e.handshake(&mut c);
    c.take();
    let buy = |dict| Request::Shop(Q46::Buy { dict, count: 1, slot: 100, profile: 1, flag: 0 });
    // no tokens: refused (an empty record list)
    c.send(buy(2087));
    e.pump(&mut [&mut c]);
    let m = c.take();
    assert!(m.contains(&S2c::Reply(Reply::Shop(Shop46::Buy { flag: 0, items: vec![] }))), "{m:?}");
    e.store.update_profile(c.account, |p| Ok::<_, ()>(p.credit(Currency::League, 3000))).unwrap();
    let silver = e.store.load_profile(c.account).unwrap().silver;
    c.send(buy(2087));
    e.pump(&mut [&mut c]);
    let m = c.take();
    let money = money(&m);
    assert_eq!(money.league, 750, "Saiga-12K Legend costs 2250 tokens");
    assert_eq!(money.silver, silver, "no silver is charged");
    let p = e.store.load_profile(c.account).unwrap();
    assert!(p.items.iter().any(|i| i.dict_id == 2087));
}
