//! Ranked leagues: the 9F push, the `4B` league window requests, the season end and the rating update.
//! Rules, layouts and what stays open: `docs/notes/league.md`. PROVEN / LIKELY / CHOICE as in `game_data::league`.
//!
//! The state of an account is `Profile::league` (schema v10). Ranked matches change it through
//! [`record`] (called from `stats::record_match` with the match facts); a season ends lazily for each account the next
//! time the server touches it ([`settle`]: at sign-in, at every league request and with the next ranked result), so no
//! background job is needed. Without configured seasons (`LobbyConfig::league` empty) a season runs every calendar
//! month (OFFICIAL rule; `game_data::league::Season::month`); configured seasons
//! (srp: `[[league.seasons]]` of the config file) replace it, and between them 9F says "no league" (S = 0).

use accounts::{account_of_public_id, public_id, AccountId, LadderRow, LeagueState as StoredLeague, Profile, SeasonRecord};
use game_data::league::{Position, TOKEN_ITEM};
use game_data::{LeagueSeason, MatchFacts};
use protocol::c2s::League4B;
use protocol::s2c::*;

use crate::dispatch::Ctx;
use crate::peer::{Cross, Peer};
use crate::premium::unix_now;

/// Rows of the top list sent to the client (CHOICE; the client takes at most 150).
pub const TOP_ROWS: usize = 100;
/// Reward group ids: the group a league pays when reached is the league number, the season-end group `END_BASE + league`.
pub const END_BASE: u32 = 100;
/// `match_type` of a ranked match in the facts (achievement `match_type_mask`: 2 = rating match).
pub const RANKED: u8 = 2;

/// The season running now.
pub fn current(cx: &Ctx) -> Option<LeagueSeason> {
    cx.lobby.cfg.league.current(unix_now())
}

/// Month (1..12) and year of a unix time (UTC).
pub fn month_year(t: i64) -> (u8, u16) {
    game_data::league::month_year(t)
}

/// Close the season an account's state still belongs to when it is over (not the running one): the history gets a
/// record, the tokens of the highest league reached are credited and the ladder position is cleared. An account that
/// never played ranked in that season only has its season cleared. Returns the new record.
pub fn settle(cx: &Ctx, account: AccountId) -> Option<SeasonRecord> {
    let now = unix_now();
    let cur = cx.lobby.cfg.league.current(now).map(|s| s.number);
    let sched = &cx.lobby.cfg.league;
    let rec = cx.lobby.store.update_profile(account, |p| {
        let st = &mut p.league;
        if st.season == 0 || Some(st.season) == cur {
            return Ok::<_, ()>(None);
        }
        let played = st.max_league > 0;
        let ended = sched.season(st.season).map(|s| s.end).unwrap_or(now);
        let mut out = None;
        if played {
            let tokens = sched.tokens_for(st.max_league);
            let r = SeasonRecord { season: st.season, league: st.league.max(1), max_league: st.max_league, tokens, closed_at: ended, notified: false };
            st.history.retain(|h| h.season != r.season);
            st.history.push(r);
            st.tokens = st.tokens.saturating_add(tokens);
            out = Some(r);
        }
        st.season = 0;
        st.league = 0;
        st.points = 0;
        st.max_league = 0;
        st.matches = 0;
        Ok(out)
    });
    match rec {
        Ok(r) => r,
        Err(e) => {
            log::error!("league settle of account {account} failed: {e:?}");
            None
        }
    }
}

/// The position of a state in the ladder (an account without a ranked match this season starts in league 1).
fn position(st: &StoredLeague) -> Position {
    Position { league: st.league.max(1), points: st.points }
}

/// The league row of a ranked match's results screen: what [`record`] is going to apply, from the same inputs and
/// before it runs (the 97 goes out first). `(f928 = league - 1, points before, points_for_levelup, signed points of the
/// match)`; None when [`record`] would leave the rating alone. A season the account's state does not belong to counts
/// as league 1 with 0 points (what `settle` leaves). A promotion / demotion shows the old league's bar (LIKELY).
pub fn results_row(cx: &Ctx, p: &Profile, f: &MatchFacts) -> Option<(u8, u16, u8, i8)> {
    if f.match_type != Some(RANKED) {
        return None;
    }
    let place = f.team_place?;
    let season = current(cx)?;
    let pos = if p.league.season == season.number { position(&p.league) } else { Position { league: 1, points: 0 } };
    let data = &cx.lobby.data.league;
    let o = data.apply(pos, f.won, !f.finished, place as usize)?;
    let stars = data.league(pos.league)?.points_for_levelup;
    Some((pos.league - 1, pos.points.min(0xFFFF) as u16, stars.min(255) as u8, o.delta.clamp(-128, 127) as i8))
}

/// Count a finished match for the ranked ladder. Only a ranked match with an observed team place counts; a local
/// training match, a random match and a match whose places were not observed leave the rating alone (nothing is
/// guessed). Returns the messages for the account's peer.
pub fn record(cx: &mut Ctx, account: AccountId, f: &MatchFacts) -> Vec<S2c> {
    if f.match_type != Some(RANKED) {
        return vec![];
    }
    let Some(place) = f.team_place else {
        log::info!("ranked match of account {account}: team place not observed, rating unchanged");
        return vec![];
    };
    settle(cx, account);
    let Some(season) = current(cx) else {
        log::info!("ranked match of account {account} outside a season: rating unchanged");
        return vec![];
    };
    let data = &cx.lobby.data.league;
    let out = cx.lobby.store.update_profile(account, |p| {
        let st = &mut p.league;
        let o = data.apply(position(st), f.won, !f.finished, place as usize).ok_or(())?;
        st.season = season.number;
        st.league = o.after.league;
        st.points = o.after.points;
        st.max_league = st.max_league.max(o.after.league);
        st.matches += 1;
        Ok::<_, ()>(o)
    });
    let Ok(o) = out else {
        log::error!("ranked result of account {account} not stored");
        return vec![];
    };
    log::info!("ranked match of account {account}: {:?} -> {:?} ({:+} points)", o.before, o.after, o.delta);
    let mut msgs = Vec::new();
    if o.after.league != o.before.league {
        // 9F type 3: "new league" (up) or "league down" message; the client answers with 4B 02 and gets the 9F info back
        msgs.push(S2c::League(League9F::Notice(LeagueNotice {
            state: o.after.league,
            league_up: u8::from(o.promoted()),
            show_rewards: 0,
            entries: vec![],
        })));
    }
    msgs
}

fn entries_of(rows: &[LadderRow]) -> Vec<LeagueEntry> {
    rows.iter()
        .take(255)
        .map(|r| LeagueEntry { a: r.points.min(0xFFFF) as u16, b: r.league, c: r.max_league, value: f64::from_bits(public_id(r.account)) })
        .collect()
}

/// Friends and clan members who play the running season (the two lists of the league window), the account itself excluded.
fn social_lists(cx: &Ctx, account: AccountId, season: u16) -> (Vec<LeagueEntry>, Vec<LeagueEntry>) {
    let store = &cx.lobby.store;
    let friends: Vec<AccountId> = store
        .friends(account)
        .unwrap_or_default()
        .into_iter()
        .filter(|f| f.state == accounts::FriendState::Friend)
        .map(|f| f.id)
        .collect();
    let clan: Vec<AccountId> = store
        .clan_of(account)
        .ok()
        .flatten()
        .map(|c| c.members.iter().map(|m| m.id).filter(|id| *id != account).collect())
        .unwrap_or_default();
    let rows = |ids: &[AccountId]| store.league_rows_of(ids, season).unwrap_or_default();
    (entries_of(&rows(&friends)), entries_of(&rows(&clan)))
}

fn history_entries(h: &[SeasonRecord]) -> Vec<SeasonEntry> {
    let skip = h.len().saturating_sub(64);
    h[skip..]
        .iter()
        .map(|r| {
            // the month the season was played: its last second (a monthly season ends at the next 1st 00:00)
            let (month, year) = month_year(r.closed_at - 1);
            SeasonEntry { a: month, b: year, c: r.max_league }
        })
        .collect()
}

/// The player's part of 9F / `83 4B 01`.
fn player_state(cx: &Ctx, account: AccountId, p: &Profile, season: LeagueSeason, with_history: bool) -> LeagueState {
    let pos = position(&p.league);
    let (friends, clan) = social_lists(cx, account, season.number);
    let minutes = ((season.end - unix_now()).max(0) / 60).min(u32::MAX as i64) as u32;
    LeagueState {
        // PROVEN (0x20027EA40): id = minutes left of the season, w = points (the client shows `w * 0.25` stars),
        // lv = current league, lv2 = highest league reached
        id: minutes,
        w: pos.points.min(0xFFFF) as u16,
        lv: pos.league,
        lv2: p.league.max_league.max(pos.league),
        history: if with_history { history_entries(&p.league.history) } else { vec![] },
        friends,
        clan,
        pairs: [(0, 0); 5],
    }
}

/// 9F type 0 for an account: the reward groups and the level table (from the data), the minimum equipment tier,
/// a pending season-final notice and, while a season runs, the account's own state.
pub fn info(cx: &Ctx, account: AccountId, p: &Profile) -> LeagueInfo {
    let gd = &cx.lobby.data;
    let sched = &cx.lobby.cfg.league;
    let k = gd.league.count();
    let mut rewards = Vec::new();
    let mut levels = Vec::new();
    for l in 1..=k {
        // reaching a league pays nothing (no data names such a reward): an empty group the level table can point at
        rewards.push(RewardGroup { id: l as u32, items: vec![] });
        rewards.push(RewardGroup { id: END_BASE + l as u32, items: vec![(TOKEN_ITEM, sched.tokens_for(l), 0)] });
        levels.push((l as u32, END_BASE + l as u32, l));
    }
    let pending = p.league.history.iter().find(|h| !h.notified);
    let (a, b) = pending.map(|h| (h.max_league, h.max_league)).unwrap_or((0xFF, 0));
    let season = current(cx);
    let mut i = LeagueInfo {
        rewards,
        levels,
        x: gd.league.min_equip_level(),
        a,
        b,
        val: 0,
        s: season.map(|s| s.number).unwrap_or(0),
        // LIKELY: L+0x7A394C is copied into a match-start structure (0x264290); 1 = ranked is available
        flag: u8::from(season.is_some()),
        state: None,
    };
    if let Some(s) = season {
        i.state = Some(player_state(cx, account, p, s, true));
    }
    i
}

/// The 9F of the login pushes. A season-final notice it carries is marked delivered.
pub fn login_push(cx: &Ctx, account: AccountId, p: &Profile) -> S2c {
    let i = info(cx, account, p);
    if i.a != 0xFF {
        let _ = cx.lobby.store.update_profile(account, |p| {
            for h in p.league.history.iter_mut().filter(|h| !h.notified) {
                h.notified = true;
            }
            Ok::<_, ()>(())
        });
    }
    S2c::League(League9F::Info(i))
}

/// Settle a finished season before the login pushes are built (the tokens must be in 8B).
pub fn sync_on_login(cx: &Ctx, account: AccountId) {
    settle(cx, account);
}

fn name_and_clan(cx: &Ctx, id: AccountId) -> (String, String) {
    let name = cx.lobby.store.account(id).ok().flatten().map(|a| a.name).unwrap_or_default();
    (name, cx.lobby.store.clan_abbr_of(id))
}

/// `4B 01 / 02 / 04 / 05`.
pub fn handle(cx: &mut Ctx, peer: &mut Peer, req: &League4B) {
    let Some(me) = peer.account else { return };
    settle(cx, me);
    let Ok(p) = cx.lobby.store.load_profile(me) else { return };
    match req {
        League4B::Opened => {
            let season = current(cx);
            let s2 = season.map(|s| s.number).unwrap_or(0);
            let state = season.map(|s| player_state(cx, me, &p, s, false));
            peer.reply(Reply::League(LeagueReply::Window { s2, state }));
        }
        League4B::Ack => {
            // end of a 9F type 3 notice (LIKELY: the client expects the 9F info again)
            peer.push(&S2c::League(League9F::Info(info(cx, me, &p))));
        }
        League4B::History { public_id } => {
            // the "Performance History" window (panel_league.gfx `LeagueHistoryWindow`): the asked player's finished
            // seasons. An empty list is the client's own "Cannot load the history." for the viewer's own id and "This
            // player did not participate in ranking matches." for another id (docs/notes/league.md section 5).
            let who = account_of_public_id(*public_id);
            let entries = who.and_then(|id| cx.lobby.store.league_history(id).ok()).map(|h| history_entries(&h)).unwrap_or_default();
            log::info!(
                "{}: 4B 04 season history of public id {public_id} (account {who:?}{}): {} finished seasons",
                peer.addr,
                if who == Some(me) { ", own" } else { "" },
                entries.len()
            );
            peer.reply(Reply::League(LeagueReply::History { public_id: *public_id, entries }));
        }
        League4B::TopPlayers => {
            let Some(s) = current(cx) else {
                // no season running: an empty list (the client shows its "no data" state)
                peer.reply(Reply::League(LeagueReply::Top { rows: vec![], me: (0, String::new(), String::new()), tail: 0 }));
                return;
            };
            let gd = &cx.lobby.data;
            let ladder = cx.lobby.store.league_ladder(s.number, TOP_ROWS).unwrap_or_default();
            let pts = |r: &LadderRow| gd.league.total_points(Position { league: r.league, points: r.points }).min(0xFFFF) as u16;
            let rows = ladder.iter().map(|r| (pts(r), r.name.clone(), cx.lobby.store.clan_abbr_of(r.account))).collect();
            let (name, abbr) = name_and_clan(cx, me);
            let place = cx.lobby.store.league_place(me, s.number).ok().flatten().unwrap_or(0);
            let mine = gd.league.total_points(position(&p.league)).min(0xFFFF) as u16;
            peer.reply(Reply::League(LeagueReply::Top { rows, me: (mine, name, abbr), tail: place }));
        }
    }
}

/// Push the 9F info to an online account (e.g. after a season-end settlement while it is connected).
pub fn push_info(cx: &mut Ctx, account: AccountId) {
    if let Ok(p) = cx.lobby.store.load_profile(account) {
        let msg = S2c::League(League9F::Info(info(cx, account, &p)));
        cx.fx.push(Cross { to: account, msg });
    }
}
