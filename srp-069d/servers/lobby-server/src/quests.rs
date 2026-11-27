//! Daily quests (push `AB`, `B1` quest messages, requests `4A 03` reroll and `4A 04` chest).
//!
//! Rules (all from `gameplay/quests/daily_quests.options` through `game_data::daily_quests`, see docs/notes/quests.md):
//! * a set of 3 distinct quests is assigned when the account has none or the set ended; it lives
//!   `lifetime_in_minutes` (1080) from the moment it is assigned (no calendar day boundary is involved);
//! * a quest with `need_item_of_category` is only given to an account that owns an item of that category (LIKELY
//!   meaning of the field), one with `min_faction_level` only above that level (no row has it);
//! * progress counts finished local matches (leavers get nothing): games, games of the active faction, games of a mode,
//!   each only as won when the quest says `must_win_to_complete`. Kill, score, killstreak, event and squad quests
//!   exist in the pool but stay at 0, because the server has no such facts (a local match reports only win / loss);
//! * completing a quest pays its reward row at once (experience, reputation of the active faction, silver; the row is
//!   chosen by the highest faction level, as the client does), completing all of them lets the player open the daily
//!   chest (`4A 04`): one weighted draw from the chest's drop table gives the reward;
//! * a reroll replaces an unfinished quest by another one not in the set: the first `free_reroll_count` per set are
//!   free, later ones cost `reroll_premium_cost` gold.

use accounts::{Currency, DailyState, Profile, QuestSlot};
use game_data::daily_quests::{Eligibility, Gain};
use game_data::levels::faction_level;
use game_data::{GameData, MatchFacts, Roll};
use protocol::s2c::quests::{AbQuest, DailyQuestsAb, QuestEvent, MAX_QUESTS};
use protocol::s2c::S2c;
use protocol::Encode;

use crate::build;
use crate::dispatch::{update, Ctx};
use crate::peer::Peer;
use crate::premium::unix_now;

/// Highest faction level of the account, capped at 5 (the client's reward row key, exe 0x2001883A0..0x20018840C).
pub fn reward_level(p: &Profile) -> u32 {
    (1..=4u8).map(|f| faction_level(p.points(f)).min(5)).max().unwrap_or(0)
}

fn owned_categories(gd: &GameData, p: &Profile) -> std::collections::BTreeSet<u16> {
    p.items.iter().map(|i| gd.items.category(i.dict_id)).collect()
}

fn pool(gd: &GameData, p: &Profile) -> Vec<u16> {
    let owned = owned_categories(gd, p);
    gd.daily_quests.eligible(&Eligibility { owned_categories: &owned, max_faction_level: reward_level(p) })
}

fn new_uid(d: &mut DailyState) -> u32 {
    d.next_uid = d.next_uid.max(1);
    d.next_uid += 1;
    d.next_uid - 1
}

fn new_msg(d: &mut DailyState) -> u32 {
    d.next_msg = d.next_msg.max(1);
    d.next_msg += 1;
    d.next_msg - 1
}

/// Assign a fresh set when there is none or it ended. Returns whether a new set was made.
pub fn ensure(gd: &GameData, p: &mut Profile, now: i64, r: &mut dyn Roll) -> bool {
    if !p.daily.slots.is_empty() && now < p.daily.expires {
        return false;
    }
    let mut avail = pool(gd, p);
    let mut slots = Vec::new();
    while slots.len() < MAX_QUESTS && !avail.is_empty() {
        let dict = avail.swap_remove(r.below(avail.len() as u32) as usize);
        let uid = new_uid(&mut p.daily);
        slots.push(QuestSlot { uid, dict, progress: 0, done: false });
    }
    p.daily.slots = slots;
    p.daily.issued = now;
    p.daily.expires = now + i64::from(gd.daily_quests.lifetime_secs);
    p.daily.free_used = 0;
    p.daily.chest_dict = 0;
    p.daily.chest_amount = 0;
    true
}

fn free_left(gd: &GameData, d: &DailyState) -> u8 {
    gd.daily_quests.free_rerolls.saturating_sub(d.free_used)
}

/// The `AB` message of the account's current state.
pub fn ab(gd: &GameData, p: &Profile, now: i64) -> DailyQuestsAb {
    let free = free_left(gd, &p.daily);
    DailyQuestsAb {
        expires_in: (p.daily.expires - now).clamp(0, i64::from(u32::MAX)) as u32,
        reroll_allowed: free > 0 || p.gold >= gd.daily_quests.reroll_cost,
        free_rerolls: free,
        quests: p
            .daily
            .slots
            .iter()
            .map(|s| AbQuest { uid: s.uid, dict: u32::from(s.dict), param: gd.daily_quests.quest(s.dict).map_or(0, |q| q.parameter), progress: s.progress })
            .collect(),
        received_dict: p.daily.chest_dict,
        received_amount: p.daily.chest_amount.min(0xFFFF) as u16,
    }
}

pub fn ab_push(gd: &GameData, p: &Profile, now: i64) -> S2c {
    let payload = ab(gd, p, now).to_bytes().unwrap_or_else(|e| {
        log::error!("AB not encodable: {e}");
        vec![0; 11]
    });
    S2c::Raw { id: protocol::ids::s2c::DAILY_QUESTS, payload }
}

/// Result of a finished match for the quests.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct MatchChange {
    /// `B1` messages (type 0x0E) with their unique ids
    pub events: Vec<(QuestEvent, u32)>,
    /// quests completed by this match (their rewards are paid)
    pub completed: usize,
}

/// Count a finished match. Pays the reward of every quest it completes.
pub fn on_match(gd: &GameData, p: &mut Profile, f: &MatchFacts) -> MatchChange {
    let mut out = MatchChange::default();
    let row = gd.daily_quests.reward_row(reward_level(p)).clone();
    for i in 0..p.daily.slots.len() {
        let s = p.daily.slots[i];
        let Some(def) = gd.daily_quests.quest(s.dict) else { continue };
        if s.done {
            continue;
        }
        // None = a fact this quest needs is absent: nothing moves
        let Some(gain) = def.gain(f) else { continue };
        let progress = match gain {
            Gain::Add(n) => s.progress.saturating_add(n),
            Gain::Max(n) => s.progress.max(n),
        }
        .min(def.value);
        if progress == s.progress {
            continue;
        }
        let done = progress >= def.value;
        p.daily.slots[i].progress = progress;
        p.daily.slots[i].done = done;
        let id = new_msg(&mut p.daily);
        if done {
            crate::progress::add_xp(gd, p, row.exp);
            let fac = p.faction;
            p.add_points(fac, row.rep);
            p.credit(Currency::Silver, u64::from(row.silver));
            out.completed += 1;
            out.events.push((QuestEvent::Completed { uid: s.uid, progress }, id));
        } else {
            out.events.push((QuestEvent::Progress { uid: s.uid, progress }, id));
        }
    }
    out
}

/// Why a request was refused.
#[derive(Debug, PartialEq, Eq)]
pub enum Refused {
    NoSuchQuest,
    Finished,
    NoReplacement,
    Gold,
    ChestNotReady,
    NoChestData,
}

/// `4A 03`: replace quest `uid`.
pub fn reroll(gd: &GameData, p: &mut Profile, uid: u32, r: &mut dyn Roll) -> Result<(), Refused> {
    let Some(i) = p.daily.slots.iter().position(|s| s.uid == uid) else { return Err(Refused::NoSuchQuest) };
    if p.daily.slots[i].done {
        return Err(Refused::Finished);
    }
    let have: Vec<u16> = p.daily.slots.iter().map(|s| s.dict).collect();
    let mut avail: Vec<u16> = pool(gd, p).into_iter().filter(|d| !have.contains(d)).collect();
    if avail.is_empty() {
        return Err(Refused::NoReplacement);
    }
    if free_left(gd, &p.daily) > 0 {
        p.daily.free_used += 1;
    } else if p.spend(Currency::Gold, u64::from(gd.daily_quests.reroll_cost)).is_err() {
        return Err(Refused::Gold);
    }
    let dict = avail.swap_remove(r.below(avail.len() as u32) as usize);
    let uid = new_uid(&mut p.daily);
    p.daily.slots[i] = QuestSlot { uid, dict, progress: 0, done: false };
    Ok(())
}

/// Give one chest reward: silver (214), spare parts (925) and reputation (1164) are wallet entries, everything else an
/// inventory item (stacks grow). Returns whether the inventory changed.
pub fn grant(gd: &GameData, p: &mut Profile, dict: u16, amount: u32) -> bool {
    match dict {
        214 => p.credit(Currency::Silver, u64::from(amount)),
        925 => p.credit(Currency::Parts, u64::from(amount)),
        1164 => {
            let f = p.faction;
            p.add_points(f, amount);
        }
        _ => {
            if gd.items.is_stackable(dict) {
                match p.items.iter_mut().find(|i| i.dict_id == dict) {
                    Some(row) => row.count += amount,
                    None => {
                        p.add_item(dict, amount);
                    }
                }
            } else {
                for _ in 0..amount.max(1) {
                    let uid = p.add_item(dict, gd.items.amount_field(dict, 100));
                    // a module (none in the quest chests' 0.69d data) is green, never grey
                    crate::chests::give_rarity(gd, p, uid, &mut crate::fresh_roll());
                }
            }
            return true;
        }
    }
    false
}

/// `4A 04`: open the daily chest. Returns (item, amount, inventory changed).
pub fn open_chest(gd: &GameData, p: &mut Profile, r: &mut dyn Roll) -> Result<(u16, u32, bool), Refused> {
    let d = &p.daily;
    if d.slots.is_empty() || d.slots.iter().any(|s| !s.done) || d.chest_dict != 0 {
        return Err(Refused::ChestNotReady);
    }
    let chest = gd.daily_quests.reward_row(reward_level(p)).all_completed;
    let (item, amount) = gd.daily_quests.draw_chest(chest, r).ok_or(Refused::NoChestData)?;
    let items = grant(gd, p, item, amount);
    p.daily.chest_dict = item;
    p.daily.chest_amount = amount;
    Ok((item, amount, items))
}

// ---- glue (messages to the peer)

/// 8B, 8C, 8E (and 89 when items came) of the profile.
pub fn wallet_pushes(gd: &GameData, p: &Profile, items_changed: bool) -> Vec<S2c> {
    let mut v = vec![S2c::Money(build::money_8b(p)), S2c::Level(build::level_8c(gd, p)), S2c::Factions(build::factions_8e(p))];
    if items_changed {
        // LIKELY: 89 replaces the client's inventory (same assumption as achievements::push_changes)
        v.push(S2c::Inventory(build::inventory_89(gd, p)));
    }
    v
}

fn push_wallet(cx: &Ctx, peer: &mut Peer, items_changed: bool) {
    let Some(id) = peer.account else { return };
    let Ok(p) = cx.lobby.store.load_profile(id) else { return };
    for m in wallet_pushes(&cx.lobby.data, &p, items_changed) {
        peer.push(&m);
    }
}

/// Sign-in: assign the set if needed (before the login pushes).
pub fn sync_on_login(cx: &Ctx, id: accounts::AccountId) {
    if !cx.lobby.cfg.daily_quests {
        return;
    }
    let gd = &cx.lobby.data;
    let now = unix_now();
    let res = cx.lobby.store.update_profile(id, |p| Ok::<_, ()>(cx.lobby.roll(|r| ensure(gd, p, now, r))));
    if let Err(e) = res {
        log::error!("daily quests of account {id}: {e:?}");
    }
}

/// The `AB` of the login pushes (the set exists after `sync_on_login`).
pub fn login_push(cx: &Ctx, p: &Profile) -> S2c {
    ab_push(&cx.lobby.data, p, unix_now())
}

fn push_ab(cx: &Ctx, peer: &mut Peer) {
    let Some(id) = peer.account else { return };
    if let Ok(p) = cx.lobby.store.load_profile(id) {
        peer.push(&ab_push(&cx.lobby.data, &p, unix_now()));
        peer.quest_expires = p.daily.expires;
    }
}

/// Called before each request of a signed-in peer: when the set ended, assign a new one and push it.
pub fn tick(cx: &mut Ctx, peer: &mut Peer) {
    if !cx.lobby.cfg.daily_quests {
        return;
    }
    let now = unix_now();
    if peer.quest_expires == 0 || now < peer.quest_expires {
        return;
    }
    let gd = &cx.lobby.data;
    let new = update(cx, peer, |p| cx.lobby.roll(|r| ensure(gd, p, now, r)));
    if new == Some(true) {
        log::info!("{}: the daily quests ended, a new set was assigned", peer.addr);
        push_ab(cx, peer);
    } else {
        // nothing assigned (store failure): try again after a minute
        peer.quest_expires = now + 60;
    }
}

/// Count a finished match for an account (called by `stats::record_match`): assigns a set when needed, counts, and
/// returns the messages for the player (quest messages, wallet pushes, the new `AB` after an assignment).
pub fn record(cx: &Ctx, account: accounts::AccountId, f: &MatchFacts) -> Vec<S2c> {
    if !cx.lobby.cfg.daily_quests || !f.finished || f.map == 0 {
        return vec![]; // leavers get nothing; map 0 is the tutorial
    }
    let gd = &cx.lobby.data;
    let now = unix_now();
    let rw = cx.lobby.cfg.achievement_rewards;
    let res = cx.lobby.store.update_profile(account, |p| {
        let fresh = cx.lobby.roll(|r| ensure(gd, p, now, r));
        let ch = on_match(gd, p, f);
        // achievement 620 ("Complete N daily quests"): one per completed quest; the rewards paid may move the level /
        // rank figures too
        let ach = (ch.completed > 0)
            .then(|| crate::achievements::event_in(gd, rw, p, crate::achievements::Event::DailyQuests(ch.completed as u32)));
        Ok::<_, ()>((fresh, ch, ach))
    });
    let (fresh, ch, ach) = match res {
        Ok(v) => v,
        Err(e) => {
            log::error!("daily quests of account {account}: {e:?}");
            return vec![];
        }
    };
    let mut out = Vec::new();
    let p = cx.lobby.store.load_profile(account).ok();
    if fresh {
        if let Some(p) = &p {
            out.push(ab_push(gd, p, now));
        }
    }
    for (e, id) in &ch.events {
        out.push(S2c::SystemMessage(e.message(*id)));
    }
    if ch.completed > 0 {
        if let Some(p) = &p {
            out.extend(wallet_pushes(gd, p, false));
        }
    }
    if let Some((changes, before)) = ach {
        out.extend(crate::achievements::changes_messages(cx, account, &changes, before));
    }
    log::info!("account {account}: daily quests after the match: {} changed, {} completed", ch.events.len(), ch.completed);
    out
}

/// `4A 03 u32 quest id`.
pub fn reroll_request(cx: &mut Ctx, peer: &mut Peer, uid: u32) {
    if !cx.lobby.cfg.daily_quests {
        return;
    }
    let gd = &cx.lobby.data;
    let res = update(cx, peer, |p| cx.lobby.roll(|r| reroll(gd, p, uid, r)));
    match res {
        Some(Ok(())) => {
            log::info!("{}: 4A 03 quest {uid} rerolled", peer.addr);
            push_ab(cx, peer);
            push_wallet(cx, peer, false);
        }
        Some(Err(e)) => {
            log::info!("{}: 4A 03 quest {uid} refused: {e:?}", peer.addr);
            push_ab(cx, peer); // the unchanged state brings the client back in line
        }
        None => {}
    }
}

/// `4A 04 u8`: the chest was opened by the player.
pub fn chest_request(cx: &mut Ctx, peer: &mut Peer) {
    if !cx.lobby.cfg.daily_quests {
        return;
    }
    let gd = &cx.lobby.data;
    let res = update(cx, peer, |p| cx.lobby.roll(|r| open_chest(gd, p, r)));
    match res {
        Some(Ok((item, amount, items))) => {
            log::info!("{}: 4A 04 daily chest: item {item} x{amount}", peer.addr);
            peer.reply_raw(0x4A, &protocol::s2c::quests::chest_reply(item, amount));
            push_wallet(cx, peer, items);
            push_ab(cx, peer);
        }
        Some(Err(e)) => log::info!("{}: 4A 04 refused: {e:?}", peer.addr),
        None => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_data::SplitMix;

    fn gd() -> GameData {
        game_data::testing::tables()
    }

    fn made(seed: u64) -> (GameData, Profile) {
        let gd = gd();
        let mut p = Profile::starter();
        ensure(&gd, &mut p, 1000, &mut SplitMix(seed));
        (gd, p)
    }

    #[test]
    fn a_set_has_three_distinct_quests_and_lasts_18_hours() {
        game_data::require_tables!();
        let (_, p) = made(1);
        let d: std::collections::BTreeSet<u16> = p.daily.slots.iter().map(|s| s.dict).collect();
        assert_eq!(p.daily.slots.len(), 3);
        assert_eq!(d.len(), 3);
        assert_eq!(p.daily.expires, 1000 + 18 * 3600);
        let uids: std::collections::BTreeSet<u32> = p.daily.slots.iter().map(|s| s.uid).collect();
        assert_eq!(uids.len(), 3);
    }

    #[test]
    fn the_set_is_kept_until_it_ends() {
        game_data::require_tables!();
        let (gd, mut p) = made(2);
        let before = p.daily.clone();
        assert!(!ensure(&gd, &mut p, 1000 + 18 * 3600 - 1, &mut SplitMix(9)));
        assert_eq!(p.daily, before);
        assert!(ensure(&gd, &mut p, 1000 + 18 * 3600, &mut SplitMix(9)));
        assert_eq!(p.daily.issued, 1000 + 18 * 3600);
        assert!(p.daily.slots.iter().all(|s| s.uid > before.slots[2].uid));
    }

    fn force(p: &mut Profile, names: [&str; 3]) {
        let gd = gd();
        for (i, n) in names.iter().enumerate() {
            let q = gd.daily_quests.quests.iter().find(|q| q.name == *n).unwrap();
            p.daily.slots[i].dict = q.index;
            p.daily.slots[i].progress = 0;
            p.daily.slots[i].done = false;
        }
    }

    #[test]
    fn matches_advance_and_complete_quests_and_pay_the_row() {
        game_data::require_tables!();
        let (gd, mut p) = made(3);
        force(&mut p, ["st_daily_quest_win_games", "st_daily_quest_play_games", "st_daily_quest_event_kill"]);
        let (silver, xp) = (p.silver, p.xp);
        let loss = MatchFacts::local(4, 1, false, true, 1);
        let ch = on_match(&gd, &mut p, &loss);
        // a lost match: play_games 1 / 3, win_games untouched, kills untrackable
        assert_eq!(ch.events.len(), 1);
        assert!(matches!(ch.events[0].0, QuestEvent::Progress { progress: 1, .. }));
        assert_eq!(p.daily.slots[0].progress, 0);
        assert_eq!(p.daily.slots[2].progress, 0);
        let win = MatchFacts::local(4, 1, true, true, 1);
        let ch = on_match(&gd, &mut p, &win);
        assert_eq!(ch.completed, 1); // win_games done (value 1)
        let ch2 = on_match(&gd, &mut p, &win);
        assert_eq!(ch2.completed, 1); // play_games reaches 3
        let row = gd.daily_quests.reward_row(reward_level(&p));
        // the quest experience may cross levels; each level reached pays its silver (progress::add_xp)
        let up = gd.levels.level_up_silver(xp, p.xp);
        assert_eq!(p.silver, silver + 2 * row.silver + up);
        assert_eq!(p.xp, xp + 2 * row.exp);
        // a done quest does not count again
        assert!(on_match(&gd, &mut p, &win).events.is_empty());
        // message ids are unique
        let ids: Vec<u32> = ch.events.iter().chain(&ch2.events).map(|e| e.1).collect();
        assert_ne!(ids[0], ids[1]);
    }

    #[test]
    fn kill_score_and_streak_quests_complete_from_full_facts_and_the_chest_follows() {
        game_data::require_tables!();
        use game_data::KillFact;
        let (gd, mut p) = made(8);
        force(&mut p, ["st_daily_quest_kill_with_weapon_class_sniper", "st_daily_quest_match_score", "st_daily_quest_killstreak_length"]);
        let sniper = |flags| KillFact { weapon_category: Some(13), kill_flags: Some(flags), ..KillFact::default() };
        // the local facts do nothing
        assert!(on_match(&gd, &mut p, &MatchFacts::local(4, 1, true, true, 1)).events.is_empty());
        let mut f = MatchFacts::local(4, 1, false, true, 1);
        f.kills = Some(vec![sniper(0), sniper(8)]);
        f.score = Some(60);
        f.max_killstreak = Some(2);
        let ch = on_match(&gd, &mut p, &f);
        // 2 of 3 sniper kills, score 60 of 100, streak 2 of 3: three progress messages, nothing completed
        assert_eq!((ch.events.len(), ch.completed), (3, 0));
        assert_eq!(p.daily.slots.iter().map(|s| s.progress).collect::<Vec<_>>(), vec![2, 60, 2]);
        // the best score counts, a lower one later does not move it
        f.score = Some(40);
        f.kills = Some(vec![sniper(0)]);
        f.max_killstreak = Some(3);
        let ch = on_match(&gd, &mut p, &f);
        assert_eq!(ch.completed, 2, "sniper 3 / 3 and streak 3 / 3");
        assert_eq!(p.daily.slots[1].progress, 60);
        f.score = Some(150);
        assert_eq!(on_match(&gd, &mut p, &f).completed, 1);
        assert!(p.daily.slots.iter().all(|s| s.done && s.progress == gd.daily_quests.quest(s.dict).unwrap().value));
        let (item, amount, _) = open_chest(&gd, &mut p, &mut SplitMix(5)).unwrap();
        assert!(amount > 0 && item > 0);
    }

    #[test]
    fn event_kill_quests_count_kills_melee_headshots_and_artefacts() {
        game_data::require_tables!();
        use game_data::KillFact;
        let (gd, mut p) = made(9);
        force(&mut p, ["st_daily_quest_event_kill", "st_daily_quest_event_melee_kill", "st_daily_quest_event_headshot_kill"]);
        let k = |flags| KillFact { kill_flags: Some(flags), ..KillFact::default() };
        let mut f = MatchFacts::local(4, 1, true, true, 1);
        f.kills = Some((0..10).map(|i| k(if i < 3 { 2 } else if i < 8 { 8 } else { 0 })).collect());
        let ch = on_match(&gd, &mut p, &f);
        assert_eq!(ch.completed, 3, "10 kills, 3 melee, 5 headshots reach all three goals");
        let q = gd.daily_quests.quests.iter().find(|q| q.name == "st_daily_quest_event_artefact").unwrap().index;
        p.daily.slots[0] = QuestSlot { uid: 77, dict: q, progress: 0, done: false };
        f.artefacts = Some(3);
        assert_eq!(on_match(&gd, &mut p, &f).completed, 1);
    }

    #[test]
    fn faction_quests_follow_the_active_faction() {
        game_data::require_tables!();
        let (gd, mut p) = made(4);
        force(&mut p, ["st_daily_quest_win_games_faction_edge", "st_daily_quest_play_games_faction_edge", "st_daily_quest_win_games_squad"]);
        let ch = on_match(&gd, &mut p, &MatchFacts::local(4, 1, true, true, 3));
        assert!(ch.events.is_empty());
        let ch = on_match(&gd, &mut p, &MatchFacts::local(4, 1, true, true, 4));
        assert_eq!(ch.completed, 2);
        assert_eq!(p.daily.slots[2].progress, 0, "squad matches do not exist locally");
    }

    #[test]
    fn reroll_is_free_once_then_costs_gold_and_never_touches_a_finished_quest() {
        game_data::require_tables!();
        let (gd, mut p) = made(5);
        let gold = p.gold;
        let uid = p.daily.slots[0].uid;
        reroll(&gd, &mut p, uid, &mut SplitMix(1)).unwrap();
        assert_eq!((p.gold, p.daily.free_used), (gold, 1));
        assert_ne!(p.daily.slots[0].uid, uid);
        assert_eq!(reroll(&gd, &mut p, uid, &mut SplitMix(1)), Err(Refused::NoSuchQuest));
        let uid = p.daily.slots[1].uid;
        reroll(&gd, &mut p, uid, &mut SplitMix(2)).unwrap();
        assert_eq!(p.gold, gold - 30);
        p.gold = 29;
        let uid = p.daily.slots[2].uid;
        assert_eq!(reroll(&gd, &mut p, uid, &mut SplitMix(3)), Err(Refused::Gold));
        assert!(!ab(&gd, &p, 1000).reroll_allowed);
        p.daily.slots[2].done = true;
        p.gold = 1000;
        assert_eq!(reroll(&gd, &mut p, uid, &mut SplitMix(3)), Err(Refused::Finished));
        let dicts: std::collections::BTreeSet<u16> = p.daily.slots.iter().map(|s| s.dict).collect();
        assert_eq!(dicts.len(), 3, "a replacement is never a duplicate");
    }

    #[test]
    fn the_chest_opens_once_after_all_quests() {
        game_data::require_tables!();
        let (gd, mut p) = made(6);
        assert_eq!(open_chest(&gd, &mut p, &mut SplitMix(1)).unwrap_err(), Refused::ChestNotReady);
        for s in &mut p.daily.slots {
            s.done = true;
        }
        let (item, amount, _) = open_chest(&gd, &mut p, &mut SplitMix(1)).unwrap();
        assert_eq!((p.daily.chest_dict, p.daily.chest_amount), (item, amount));
        assert_eq!(open_chest(&gd, &mut p, &mut SplitMix(2)).unwrap_err(), Refused::ChestNotReady);
        let m = ab(&gd, &p, 1000);
        assert_eq!((m.received_dict, u32::from(m.received_amount)), (item, amount));
    }

    #[test]
    fn grant_routes_currencies_and_items() {
        game_data::require_tables!();
        let gd = gd();
        let mut p = Profile::starter();
        let (s, parts, pts) = (p.silver, p.parts, p.points(p.faction));
        assert!(!grant(&gd, &mut p, 214, 5000));
        assert!(!grant(&gd, &mut p, 925, 30));
        assert!(!grant(&gd, &mut p, 1164, 1500));
        assert_eq!((p.silver, p.parts, p.points(p.faction)), (s + 5000, parts + 30, pts + 1500));
        // the premium voucher is a stackable item
        assert!(grant(&gd, &mut p, 213, 1));
        assert!(grant(&gd, &mut p, 213, 1));
        assert_eq!(p.items.iter().filter(|i| i.dict_id == 213).map(|i| i.count).sum::<u32>(), 2);
        assert_eq!(p.items.iter().filter(|i| i.dict_id == 213).count(), 1);
    }

    #[test]
    fn the_account_state_survives_the_store() {
        game_data::require_tables!();
        let store = accounts::Store::open_in_memory().unwrap();
        store.set_hash_cost(accounts::HashCost::FAST_FOR_TESTS);
        let id = store.create_account("q", "secret").unwrap();
        let gd = gd();
        store.update_profile(id, |p| { ensure(&gd, p, 5, &mut SplitMix(1)); Ok::<_, ()>(()) }).unwrap();
        store.update_profile(id, |p| Ok::<_, ()>(p.daily.slots[1].progress = 2)).unwrap();
        let p = store.load_profile(id).unwrap();
        assert_eq!(p.daily.slots.len(), 3);
        assert_eq!((p.daily.issued, p.daily.slots[1].progress), (5, 2));
    }
}
