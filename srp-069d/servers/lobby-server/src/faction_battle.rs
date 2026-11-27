//! Faction battles: the status inside `8E`, the reward request `4A 05` and the battle reputation.
//! Research: docs/notes/progression.md section 5.
//!
//! PROVEN (exe + AS3):
//! * the status is the optional 40 bytes of `8E` (`u8 k != 0`, 10 x u32): `[time_left, mask, rep1, got1, .. rep4, got4]`
//!   -> `root.faction_battle_status_received(time_left, is_faction_active[4], reputations[4], received_awards_mask[4])`
//!   (0x20018AC70, 0x2001CAFC0). `WinterEventData.setBattleStatus`: the two factions whose `is_faction_active` entry is
//!   NOT true are "fighting"; a reward tier `k` of a faction is marked received when `2^k - 1 < received mask`, so the
//!   mask the server sends is `2^n - 1` for `n` tiers received; a tier is available when the faction's reputation reaches
//!   its `maxProgress`. The tiers are the client's own table (`faction_battle_drop`, `root.set_faction_battle_drop`);
//! * `4A 05 u8 faction (1..4)` (`EventFactionProgressLine.buttonGetReward_onClick`, sender 0x2001C7AB0 sends id + 1 of
//!   the AS faction index): answered `83 4A 05 u8 n, n x item record` -> `root.faction_battle_reward_received`
//!   (important messages "received: item xN"); the client does not add the items, the server pushes the inventory;
//! * the schedule is not in any game file: the window links to the website timetable. [`LobbyConfig::faction_battles`]
//!   is an input; without one `k = 0` (no status) and nothing here can run. NOT invented.
//!
//! CHOICE (nothing states it): a battle's reputation of a faction is the faction reputation the account's finished
//! matches paid while that faction was the active one and one of the two fighting factions; rewards are claimed
//! per tier, all reached tiers at once, once per battle; the unclaimed rewards of a finished battle stay claimable until
//! the faction's next battle row replaces them. `item_to_drop_after_win` (2086) has no stated rule: OPEN, not dropped.

use accounts::{BattleRow, Profile};
use game_data::chests::Reward;
use game_data::faction_battle::{Interval, Schedule, FACTIONS};
use game_data::{GameData, MatchFacts};
use protocol::s2c::hunt::{battle_status, faction_reward_reply};
use protocol::s2c::*;

use crate::dispatch::{update, Ctx};
use crate::peer::Peer;
use crate::premium::unix_now;

/// The 10 words of the status for `now`; None without a schedule or when no battle is running or ahead.
pub fn words(s: &Schedule, p: &Profile, now: i64) -> Option<[u32; 10]> {
    let (left, fighting, start) = match (s.current(now), s.next(now)) {
        (Some(i), _) => ((i.end - now), i.factions, Some(i.start)),
        (None, Some(n)) => ((n.start - now), [0, 0], None),
        (None, None) => return None,
    };
    let mut mask = 0u32;
    for f in FACTIONS {
        if !fighting.contains(&f) {
            mask |= 1 << f;
        }
    }
    let (mut rep, mut got) = ([0u32; 4], [0u32; 4]);
    for f in FACTIONS {
        if let (Some(row), Some(start)) = (p.battle.get(&f), start) {
            if row.start == start {
                rep[f as usize - 1] = row.reputation;
                got[f as usize - 1] = ((1u64 << row.received.min(31)) - 1) as u32;
            }
        }
    }
    Some(battle_status(left.clamp(0, u32::MAX as i64) as u32, mask, rep, got))
}

/// `8E` with the battle status (when a schedule runs).
pub fn factions_8e(cx: &Ctx, p: &Profile, now: i64) -> Factions8E {
    let mut m = crate::build::factions_8e(p);
    if let Some(s) = &cx.lobby.cfg.faction_battles {
        m.battle = words(s, p, now);
    }
    m
}

/// Add reputation to the faction's row of the battle that runs now.
pub fn add_reputation(s: &Schedule, p: &mut Profile, faction: u8, rep: u32, now: i64) -> bool {
    let Some(Interval { start, factions, .. }) = s.current(now).copied() else { return false };
    if rep == 0 || !factions.contains(&faction) {
        return false;
    }
    let row = p.battle.entry(faction).or_insert(BattleRow { start, reputation: 0, received: 0 });
    if row.start != start {
        *row = BattleRow { start, reputation: 0, received: 0 };
    }
    row.reputation = row.reputation.saturating_add(rep);
    true
}

/// The tiers a faction row can still claim, and the new `received` count. Empty = nothing to claim.
pub fn claimable(gd: &GameData, p: &Profile, faction: u8) -> Vec<Reward> {
    let Some(row) = p.battle.get(&faction) else { return vec![] };
    let reached = gd.faction_battle.reached(faction, row.reputation);
    gd.faction_battle.tiers(faction)[(row.received as usize).min(reached)..reached].iter().map(|t| Reward { dict: t.dict_id, amount: t.amount }).collect()
}

/// Pay the claimable tiers of `faction`: marks them received and gives the items. Returns the records for the reply.
pub fn claim(gd: &GameData, p: &mut Profile, faction: u8) -> Option<(Vec<protocol::items::ItemRecord>, crate::chests::Granted)> {
    let rewards = claimable(gd, p, faction);
    if rewards.is_empty() {
        return None;
    }
    let reached = gd.faction_battle.reached(faction, p.battle[&faction].reputation);
    p.battle.get_mut(&faction)?.received = reached as u8;
    // a chest tier (rare_modules_chest_1..4) is opened: the player gets its contents (chests::grant)
    let opened = crate::chests::open_chests(gd, &rewards, &mut crate::fresh_roll());
    let (mut recs, granted) = crate::chests::grant_with_mods(gd, p, &opened.rewards, opened.module_mods);
    // the reply only lists what was received (the client does not add it): show the amount, not the stack
    let mut at = 0;
    for r in &opened.rewards {
        let n = crate::chests::record_count(gd, r);
        let shown = if n == 1 { r.amount } else { 1 };
        for rec in recs.iter_mut().skip(at).take(n) {
            rec.value = shown;
        }
        at += n;
    }
    Some((recs, granted))
}

/// Sign-in / `35 00`: the 8E with the battle status.
pub fn push_status(cx: &Ctx, peer: &mut Peer) {
    let Some(id) = peer.account else { return };
    if cx.lobby.cfg.faction_battles.is_none() {
        return;
    }
    if let Ok(p) = cx.lobby.store.load_profile(id) {
        peer.push(&S2c::Factions(factions_8e(cx, &p, unix_now())));
    }
}

/// `4A 05 u8 faction`.
pub fn claim_request(cx: &mut Ctx, peer: &mut Peer, faction: u8) {
    if cx.lobby.cfg.faction_battles.is_none() {
        log::info!("{}: 4A 05 faction battle rewards: no battle schedule is configured", peer.addr);
        return;
    }
    let gd = &cx.lobby.data;
    let res = update(cx, peer, |p| claim(gd, p, faction));
    match res {
        Some(Some((records, granted))) => {
            log::info!("{}: 4A 05 faction {faction}: {} rewards", peer.addr, records.len());
            peer.reply_raw(0x4A, &faction_reward_reply(&records));
            let Some(id) = peer.account else { return };
            if let Ok(p) = cx.lobby.store.load_profile(id) {
                peer.push(&S2c::Inventory(crate::build::inventory_89(gd, &p)));
                peer.push(&S2c::Money(crate::build::money_8b(&p)));
                if granted.level {
                    peer.push(&S2c::Level(crate::build::level_8c(gd, &p)));
                }
                peer.push(&S2c::Factions(factions_8e(cx, &p, unix_now())));
            }
        }
        Some(None) => {
            log::info!("{}: 4A 05 faction {faction}: nothing to receive", peer.addr);
            push_status(cx, peer);
        }
        None => {}
    }
}

/// A finished match of `account` (called by `stats::record_match`): its reputation counts for the battle.
pub fn record(cx: &Ctx, account: accounts::AccountId, f: &MatchFacts) -> Vec<S2c> {
    let Some(s) = &cx.lobby.cfg.faction_battles else { return vec![] };
    let (true, Some(rep)) = (f.finished && f.map != 0, f.reputation) else { return vec![] };
    let now = unix_now();
    let res = cx.lobby.store.update_profile(account, |p| Ok::<_, ()>(add_reputation(s, p, f.faction, rep, now)));
    match res {
        Ok(true) => match cx.lobby.store.load_profile(account) {
            Ok(p) => {
                log::info!("account {account}: faction {} battle reputation +{rep}", f.faction);
                vec![S2c::Factions(factions_8e(cx, &p, now))]
            }
            Err(_) => vec![],
        },
        _ => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gd() -> GameData {
        game_data::testing::tables()
    }

    fn sched() -> Schedule {
        Schedule {
            intervals: vec![
                Interval { start: 1_000, end: 2_000, factions: [1, 3] },
                Interval { start: 3_000, end: 4_000, factions: [2, 4] },
            ],
        }
    }

    #[test]
    fn the_status_names_the_fighters_by_their_cleared_bit() {
        let mut p = Profile::starter();
        // before the first battle: nobody fights (every bit set), the time is the wait for the next one
        let w = words(&sched(), &p, 400).unwrap();
        assert_eq!((w[0], w[1]), (600, 0b11110));
        // during battle 1: factions 1 and 3 fight (bits cleared)
        assert!(add_reputation(&sched(), &mut p, 1, 12_000, 1_500));
        let w = words(&sched(), &p, 1_500).unwrap();
        assert_eq!((w[0], w[1]), (500, 0b10100));
        assert_eq!((w[2], w[3]), (12_000, 0), "faction 1: reputation, nothing received");
        assert_eq!(w[4], 0);
        // an outsider's reputation does not count, nor does a pause between battles
        assert!(!add_reputation(&sched(), &mut p, 2, 500, 1_500));
        assert!(!add_reputation(&sched(), &mut p, 1, 500, 2_500));
        // no battle left: no status
        assert!(words(&sched(), &p, 5_000).is_none());
        assert!(words(&Schedule::default(), &p, 0).is_none());
    }

    #[test]
    fn a_new_battle_replaces_the_row_and_tiers_pay_once() {
        game_data::require_tables!();
        let g = gd();
        let mut p = Profile::starter();
        let spare = p.parts;
        add_reputation(&sched(), &mut p, 1, 10_000, 1_100);
        assert_eq!(claimable(&g, &p, 1), vec![Reward { dict: 925, amount: 500 }]);
        let (recs, granted) = claim(&g, &mut p, 1).unwrap();
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].dict, 925);
        assert_eq!(recs[0].value, 500, "the reply lists the amount");
        assert!(granted.money);
        assert_eq!(p.parts, spare + 500);
        assert!(claim(&g, &mut p, 1).is_none(), "once per tier");
        // the received mask the client reads: 2^1 - 1
        let w = words(&sched(), &p, 1_200).unwrap();
        assert_eq!((w[2], w[3]), (10_000, 1));
        // more reputation: tiers 2..3 (voucher at 20000, item at 40000) in one claim
        add_reputation(&sched(), &mut p, 1, 31_000, 1_300);
        let r = claimable(&g, &p, 1);
        assert_eq!(r.iter().map(|x| x.dict).collect::<Vec<_>>(), vec![975, 2085]);
        let items = p.items.len();
        let (recs, _) = claim(&g, &mut p, 1).unwrap();
        assert_eq!(recs.len(), 2);
        assert!(p.items.len() > items);
        assert_eq!(words(&sched(), &p, 1_400).unwrap()[3], 0b111);
        // faction 3 never fought: nothing
        assert!(claim(&g, &mut p, 3).is_none());
        // the next battle of faction 2 starts a fresh row; faction 1's old row is untouched
        add_reputation(&sched(), &mut p, 2, 5_000, 3_100);
        assert_eq!(p.battle[&2].start, 3_000);
        assert_eq!(p.battle[&1].start, 1_000);
        let w = words(&sched(), &p, 3_100).unwrap();
        assert_eq!((w[4], w[5]), (5_000, 0));
        assert_eq!(w[2], 0, "faction 1's row belongs to the old battle");
    }

    /// The chest tier (rare_modules_chest_1..4) is opened at once: its contents are stored and listed, not the chest.
    #[test]
    fn a_chest_tier_gives_its_contents() {
        game_data::require_tables!();
        let g = gd();
        let mut p = Profile::starter();
        add_reputation(&sched(), &mut p, 1, 10_000_000, 1_100);
        let tiers = claimable(&g, &p, 1);
        let chest = tiers.iter().map(|t| t.dict).find(|d| g.chests.is_chest(*d)).expect("a chest tier");
        let (recs, _) = claim(&g, &mut p, 1).unwrap();
        assert!(p.items.iter().all(|i| i.dict_id != chest), "no chest stack is stored");
        assert!(recs.iter().all(|r| r.dict != chest), "{recs:?}");
        assert!(recs.len() >= tiers.len(), "the contents are listed: {recs:?}");
    }
}
