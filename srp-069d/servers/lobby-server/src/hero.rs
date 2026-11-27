//! Hero of the day: the lobby entry (`A6`), the window (`48 2B` -> `A7`, `48 2C` -> `A9`), the 24 h status (94 record
//! kind 0x19) and its bonus on the match rewards. Rules, layouts and what stays open: `docs/notes/hero-of-the-day.md`.
//!
//! * All PvP queue matches, random and ranked (CHOICE after OFFICIAL 0.41 "by simply playing the game"; the
//!   0.69d client text `st_general_hero_rules` says "ranked matches", but ranked is closed during events and the window
//!   cannot show a ranking per match type): [`record`] stores a row for every finished random or ranked match
//!   (`MatchFacts.match_type` 1 / 2) with a known score.
//! * Every `minutes_recalc_period` (30) the ranking is calculated lazily ([`tick`], called with the next request, no
//!   background job): the best three of each (group, nomination) over the last `minutes_operating_period` (1440 = 24 h)
//!   get a title that lasts as long. A player holding several titles gets only the best (`st_general_hero_rule_3`).
//! * The status is shown as `root.set_hero_of_the_day_level` (94 record kind 0x19) and pays +25 / +20 / +15 % on the
//!   experience, silver and reputation of a match ([`bonus_percent`], added to premium and boosters).

use accounts::{public_id, AccountId, HeroResult, HeroTitle, Profile};
use game_data::hero::{self, Played};
use game_data::{GameData, MatchFacts};
use protocol::s2c::hero::*;
use protocol::s2c::{Notice94, S2c};

use crate::dispatch::Ctx;
use crate::peer::{Cross, Peer};
use crate::premium::unix_now;
use crate::league::RANKED;

/// `MatchFacts.match_type` of a random queue match (`MatchKind::match_type`: 1 random, 2 rated).
const RANDOM: u8 = 1;

/// 94 record kind of the hero status (jump table 0x1C5EB4 / bytes 0x1C5EC8: kind 0x19 -> 0x2001C5930, PROVEN).
pub const KIND: u8 = 0x19;
/// The window draws at most two competitive groups (A7 carries two).
const GROUPS_SHOWN: usize = 2;

fn enabled(cx: &Ctx) -> bool {
    cx.lobby.cfg.hero_of_the_day
}

/// `A6`: the competitive groups and the bonus of each place (the "Heroes of the Day" entry and its rules page).
pub fn config_push(cx: &Ctx) -> Option<S2c> {
    if !enabled(cx) {
        return None;
    }
    let groups = cx
        .lobby
        .data
        .hero
        .groups
        .iter()
        .take(9)
        .map(|g| HeroGroupInfo {
            min_level: g.min_level,
            max_level: g.max_level,
            max_player_level: g.max_player_level,
            bonus: g.bonuses.iter().take(3).map(|b| b.percent().min(255) as u8).collect(),
        })
        .collect();
    Some(HeroConfig(Some(groups)).push())
}

/// Result of a PvP queue match (random or ranked) for the nominations. A match of another kind (local, custom training,
/// clan battle, PvE), not finished or without a known score is not counted; a group the player is not admitted to
/// (`max_player_level`) is not counted. The competitive group is the one of the player's OWN equipment level
/// ([`crate::items::equipment_level`]: the number the client keys its hero tab on, 0x2001CAF10, and the one the repairs
/// charge uses), not the match's highest.
pub fn record(cx: &mut Ctx, account: AccountId, f: &MatchFacts) {
    if !enabled(cx) || f.clan_battle || f.local_bots || !f.finished || !matches!(f.match_type, Some(RANDOM) | Some(RANKED)) {
        return;
    }
    let Some(score) = f.score else {
        log::info!("hero: match of account {account} (match type {:?}): no score known, not counted", f.match_type);
        return;
    };
    let gd = &cx.lobby.data;
    let Ok(p) = cx.lobby.store.load_profile(account) else { return };
    let own = crate::items::equipment_level(gd, &p, unix_now());
    let Some(group) = gd.hero.group_of(Some(own)) else {
        // PROVEN data: hero_of_the_day.options has one competitive group, equipment levels 3..5 (the ranked tiers)
        let ranges: Vec<String> = gd.hero.groups.iter().map(|g| format!("{}..{}", g.min_level, g.max_level)).collect();
        log::info!(
            "hero: match of account {account} (match type {:?}): own equipment level {own} is in no competitive group ({}): not counted",
            f.match_type,
            ranges.join(", ")
        );
        return;
    };
    let level = gd.levels.level_of(p.xp);
    if !gd.hero.admits(group, level) {
        log::info!("hero: account {account} (level {level}) is above the group {group} limit");
        return;
    }
    let r = HeroResult { account, at: unix_now(), group, score, killstreak: f.max_killstreak.unwrap_or(0), won: f.won };
    match cx.lobby.store.hero_add_result(&r) {
        Ok(()) => log::info!(
            "hero: account {account} result stored: group {group}, score {score}, killstreak {} ({}), won {}",
            r.killstreak,
            if f.max_killstreak.is_some() { "world" } else { "unknown" },
            r.won
        ),
        Err(e) => log::error!("hero: result of account {account} not stored: {e}"),
    }
}

/// Calculate the ranking of the slot `now` lies in when it was not calculated yet (idempotent) and give the new
/// title holders who are online their status (a 94 with the hero record).
pub fn tick(cx: &mut Ctx) {
    if !enabled(cx) {
        return;
    }
    let now = unix_now();
    let titles = recalc_titles(&cx.lobby.store, &cx.lobby.data, now);
    let rw = cx.lobby.cfg.achievement_rewards;
    let ach = title_achievements(&cx.lobby.store, &cx.lobby.data, rw, &titles);
    let mut holders: Vec<AccountId> = titles.iter().map(|t| t.account).collect();
    holders.sort_unstable();
    holders.dedup();
    for a in holders {
        if let Ok(p) = cx.lobby.store.load_profile(a) {
            let m = crate::login_calendar::with_calendar(cx, &p, crate::premium::notices(&cx.lobby.data, &p, now));
            cx.fx.push(Cross { to: a, msg: S2c::Notices(m) });
        }
    }
    for (to, changes, before) in &ach {
        for msg in crate::achievements::changes_messages(cx, *to, changes, *before) {
            cx.fx.push(Cross { to: *to, msg });
        }
    }
}

/// Achievements 623..625 (`hero_of_the_day_*`, `nomination` 1..3): each new title moves the achievement of its
/// nomination to the place reached (third place = state 1, second = 2, first = 3; the state texts
/// `st_achievement_desc_hero_of_the_day_*_1..3`). Returns the changes per account with the item count before them.
pub fn title_achievements(store: &accounts::Store, gd: &GameData, rewards: bool, titles: &[HeroTitle]) -> Vec<(AccountId, Vec<crate::achievements::Change>, usize)> {
    let mut out: Vec<(AccountId, Vec<crate::achievements::Change>, usize)> = Vec::new();
    for t in titles {
        let e = crate::achievements::Event::HeroTitle { nomination: t.nomination, place: t.place };
        let Some((ch, before)) = crate::achievements::event_for(store, gd, rewards, t.account, e) else { continue };
        match out.iter_mut().find(|(a, _, _)| *a == t.account) {
            // one push per account: the first item count, every change
            Some((_, all, _)) => all.extend(ch),
            None => out.push((t.account, ch, before)),
        }
    }
    out
}

/// [`tick`] with explicit store, data and time (the tests move the clock). Returns the accounts that hold a title of the
/// slot that was just calculated (empty when the slot was already done).
pub fn recalc(store: &accounts::Store, gd: &GameData, now: i64) -> Vec<AccountId> {
    recalc_titles(store, gd, now).into_iter().map(|t| t.account).collect()
}

/// [`recalc`] returning the titles of the slot that was just calculated (empty when the slot was already done).
pub fn recalc_titles(store: &accounts::Store, gd: &GameData, now: i64) -> Vec<HeroTitle> {
    let h = &gd.hero;
    let slot = h.slot_of(now);
    match store.hero_slot() {
        Ok(Some(s)) if s >= slot => return vec![],
        Ok(_) => {}
        Err(e) => {
            log::error!("hero: slot not readable: {e}");
            return vec![];
        }
    }
    let rows = match store.hero_results_between(slot - h.operating_secs, slot) {
        Ok(r) => r,
        Err(e) => {
            log::error!("hero: results not readable: {e}");
            return vec![];
        }
    };
    let played: Vec<Played> =
        rows.iter().map(|r| Played { account: r.account, at: r.at, group: r.group, score: r.score, killstreak: r.killstreak, won: r.won }).collect();
    let mut titles = Vec::new();
    for g in 0..h.groups.len() as u8 {
        for n in hero::NOMINATIONS {
            for (i, p) in hero::rank(&played, g, n).into_iter().enumerate() {
                titles.push(HeroTitle { account: p.account, group: g, nomination: n, place: i as u8 + 1, value: p.value, since: slot, until: slot + h.operating_secs });
            }
        }
    }
    log::info!("hero: slot {slot}: {} results, {} titles", rows.len(), titles.len());
    if let Err(e) = store.hero_commit_slot(slot, &titles, slot - h.operating_secs) {
        log::error!("hero: slot {slot} not stored: {e}");
        return vec![];
    }
    titles
}

/// The best living title of a profile (lowest place, then the lowest nomination).
pub fn best_title(p: &Profile, now: i64) -> Option<HeroTitle> {
    p.hero.iter().copied().filter(|t| t.until > now).min_by_key(|t| (t.place, t.nomination))
}

/// The title as a match shows it: `(hero_place 1..=3, nomination 0..=2)` of the best living title (the match options'
/// scoreboard table and the 97 player rows; nominations are 1-based here, 0-based in the client). None = no title.
pub fn match_bytes(p: &Profile, now: i64) -> Option<(u8, u8)> {
    best_title(p, now).map(|t| (t.place, t.nomination.saturating_sub(1)))
}

/// Bonus percentages `[experience, silver, reputation]` of the player's status.
pub fn bonus_percent(gd: &GameData, p: &Profile, now: i64) -> [u32; 3] {
    let Some(t) = best_title(p, now) else { return [0; 3] };
    let Some(b) = gd.hero.bonus(t.group, t.place) else { return [0; 3] };
    [(b.experience * 100.0).round() as u32, (b.silver * 100.0).round() as u32, (b.reputation * 100.0).round() as u32]
}

/// The 94 record of the status (kind 0x19): `f3c` = nomination index 0..2 (PROVEN: 0x2001C5951 / 0x2001C5DAE pass it
/// as the 3rd argument of `root.set_hero_of_the_day_level(enabled, bonus, nomination)`, booster icon
/// `hero_<nomination>_<place - 1>`; our nominations are 1..3, LIKELY in the window's order), `f40` = `group << 16 |
/// grade` of the experience modifier (the client looks the percent up in `modifications`).
pub fn notice(gd: &GameData, p: &Profile, now: i64) -> Option<Notice94> {
    let t = best_title(p, now)?;
    let b = gd.hero.bonus(t.group, t.place)?;
    let nomination = u32::from(t.nomination.saturating_sub(1));
    Some(Notice94 { f3c: nomination, f40: (u32::from(b.exp_group) << 16) | u32::from(b.grade), kind: KIND, ..Default::default() })
}

fn name_and_clan(cx: &Ctx, id: AccountId) -> (String, String) {
    let name = cx.lobby.store.account(id).ok().flatten().map(|a| a.name).unwrap_or_default();
    (name, cx.lobby.store.clan_abbr_of(id))
}

/// `48 2B u32 cached id, u8 need own` -> `A7`. The ranking part is sent when the client's cached id is not the current
/// one (the id is the start of the calculated slot); the player's own values when the client asks (`0xFF`).
pub fn window(cx: &mut Ctx, peer: &mut Peer, cached: u32, need_own: u8) {
    let Some(me) = peer.account else { return };
    if !enabled(cx) {
        return;
    }
    let now = unix_now();
    tick(cx);
    let h = &cx.lobby.data.hero;
    let store = &cx.lobby.store;
    let slot = h.slot_of(now);
    let groups = h.groups.len().min(GROUPS_SHOWN) as u8;
    let id = (slot.max(1) as u64 & 0xFFFF_FFFF) as u32;
    let ranking = if cached == id {
        None
    } else {
        let mut rk = HeroRanking { id, ..Default::default() };
        for t in store.hero_current_ranking(now).unwrap_or_default() {
            let (g, n, p) = (t.group as usize, (t.nomination as usize).saturating_sub(1), (t.place as usize).saturating_sub(1));
            if g >= GROUPS_SHOWN || n >= 3 || p >= 3 {
                continue;
            }
            let (name, abbr) = name_and_clan(cx, t.account);
            rk.places[g][n][p] = HeroPlace { score: t.value, name, abbr };
        }
        Some(rk)
    };
    let own = (need_own != 0).then(|| {
        let rows = store.hero_results_between(now - h.operating_secs, now).unwrap_or_default();
        let played: Vec<Played> =
            rows.iter().map(|r| Played { account: r.account, at: r.at, group: r.group, score: r.score, killstreak: r.killstreak, won: r.won }).collect();
        let mut o = HeroOwn::default();
        for g in 0..GROUPS_SHOWN {
            for n in hero::NOMINATIONS {
                // my value = what the ranking function gives me when I am the only player
                let mine: Vec<Played> = played.iter().filter(|r| r.account == me).copied().collect();
                o.values[g][n as usize - 1] = hero::rank(&mine, g as u8, n).first().map_or(0, |p| p.value);
            }
        }
        // the two bytes the client copies into its own chat lines as `nomination` and `hero_place` (PROVEN: A7 reader
        // 0x200188B5E / 0x200188B73, initial word 0x00FF = no title; `LobbyChat` shows `hero_chat_<nomination>_<place - 1>`
        // only when place != 0 and nomination < 254): nomination 0..2 (0xFF = none), place 1..3 (0 = none)
        o.a = 0xFF;
        o.b = 0;
        if let Some(t) = store.hero_titles_of(me, now).unwrap_or_default().into_iter().min_by_key(|t| (t.place, t.nomination)) {
            o.a = t.nomination.saturating_sub(1);
            o.b = t.place;
        }
        o
    });
    if ranking.is_none() && own.is_none() {
        // nothing new: the client keeps what it shows; nothing is waiting for an answer
        log::debug!("{}: 48 2B: client cache {cached} is current", peer.addr);
        return;
    }
    log::info!(
        "{}: 48 2B -> A7: ranking {} (filled places {}), own values {:?}",
        peer.addr,
        ranking.as_ref().map_or("cached".to_string(), |r| format!("id {}", r.id)),
        ranking.as_ref().map_or(0, |r| r.places.iter().flatten().flatten().filter(|p| p.score > 0).count()),
        own.as_ref().map(|o| o.values)
    );
    peer.push(&HeroData7 { groups, ranking, own }.push());
}

/// `48 2C u8 kind, u32 cached id` -> `A9` (kind 1 friends, 2 clan): the same three nominations over the people of the
/// list, from the results of the operating period (group 0, the only group of the data). Not rewarded (client text
/// `st_hero_friends_clan_info`).
pub fn friends(cx: &mut Ctx, peer: &mut Peer, kind: u8, cached: u32) {
    let Some(me) = peer.account else { return };
    if !enabled(cx) || !(1..=2).contains(&kind) {
        return;
    }
    let store = &cx.lobby.store;
    let people: Vec<AccountId> = if kind == 1 {
        store.friends(me).unwrap_or_default().into_iter().filter(|f| f.state == accounts::FriendState::Friend).map(|f| f.id).collect()
    } else {
        store.clan_of(me).ok().flatten().map(|c| c.members.iter().map(|m| m.id).collect()).unwrap_or_default()
    };
    let now = unix_now();
    let h = &cx.lobby.data.hero;
    let rows = store.hero_results_between(now - h.operating_secs, now).unwrap_or_default();
    // the list ranks the player too (he sees his own place among friends)
    let played: Vec<Played> = rows
        .iter()
        .filter(|r| r.account == me || people.contains(&r.account))
        .map(|r| Played { account: r.account, at: r.at, group: r.group, score: r.score, killstreak: r.killstreak, won: r.won })
        .collect();
    let id = played.iter().map(|p| p.at).max().unwrap_or(1).max(1) as u32;
    if cached == id {
        peer.push(&HeroFriends { kind, rows: None }.push());
        return;
    }
    let mut table = [[(0.0, 0u32); 3]; 3];
    for n in hero::NOMINATIONS {
        for (i, p) in hero::rank(&played, 0, n).into_iter().enumerate() {
            table[n as usize - 1][i] = (f64::from_bits(public_id(p.account)), p.value);
        }
    }
    peer.push(&HeroFriends { kind, rows: Some((table, id)) }.push());
}
