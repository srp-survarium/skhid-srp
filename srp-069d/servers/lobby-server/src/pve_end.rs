//! The PvE end screen: the chests found in a mission (docs/notes/pve.md "End screen").
//!
//! Labels as in the notes: PROVEN = 0.69d exe (VA) / data / the in-match HUD movie, LIKELY = layout proven and meaning
//! inferred, CHOICE = a rule the server author picked because nothing proves it, OPEN = unknown, nothing invented.
//!
//! * **Screen** (PROVEN, `HUD.checkShowPVEContainer`): the in-match HUD shows the end screen only after BOTH `99` (the result)
//!   and the server push `83 45 0B` (reader 0x200193DF0 -> 0x2001ABD80) arrived, and only while the client is still in the
//!   match (0x1ABD97 / 0x1ABDB0). `99` goes (from the online path, not here) to every human player of the match at the
//!   mission end, with the stored run's match id (compared at 0x18EDA1), before `85 00`; [`store_run_chests`] pushes the
//!   `83 45 0B` at that same moment (through [`crate::pve::record_run_notify`]).
//! * **A chest per find** (PROVEN data: `chests_total 10`, `closed_chest_dict_id 1385`, content `pve_mission_1_chest_rank_NM`
//!   per equipment tier N and difficulty M): each player of a WON run gets one stored row per chest found. The first
//!   `free_chests_num` (3, PROVEN `pve_default_prices`) are opened at once, the others stay locked (PROVEN by the HUD
//!   `hasLockedChests` flag and the "open all" button priced `unlock_drop_price`; official 0.56 text: "you will receive only 3
//!   stashes, and will be able to unlock the rest"). A lost run gives no chests (LIKELY: "the more stashes you collect, the
//!   more rewards you get when the mission is a success", 0.50 notes), and neither does a player with score 0 (LIKELY: the HUD
//!   builds `afk = own score is 0` and the Flash text `reject_drop_for_inactivity`); both get an empty `83 45 0B` so the screen
//!   shows.
//! * **The first won mission of the day opens every chest found** (PROVEN, 0.69d client text on this very screen,
//!   `PVEMatchEndRewards.setData` -> `pveTokensInfo`: `st_coop_mission_passage` "The first team mission you complete daily
//!   rewards you with all the stashes you find. Subsequent missions will provide 3 stashes.", followed by
//!   `st_coop_mission_lobby_msg_all_stashes` "At the moment you will receive all stashes you find." when `hasLockedChests` is
//!   false, else `..._avaliable_stashes` "... for free only 3 stashes."). "Complete" = a won run; the day is the one the
//!   daily free keys count (`pve_daily_wins`, UTC day, CHOICE), so the run whose win is the player's first of that day.
//! * **Tier N = 1 always** (data + official text): `chest_drops.options` of resource volumes 00 / 22 / 26 has the 15 tables
//!   `pve_mission_1_chest_rank_11..53_drop`; the newest volumes that carry the file (60, 67) removed exactly the twelve of
//!   tiers 2..5 and keep 11..13; the mission's own difficulty previews are the tier-1 chests 1483 / 1468 / 1469; the 0.56
//!   notes: "only 3 difficulty levels in PvE regardless of equipment tier". So the chest is `rank_1M`, M = difficulty + 1;
//!   the equipment level does not pick it.
//! * **Roll** (PROVEN): the chest tables of `game_data::chests` (`items_count 1`, so one reward per chest), granted like an
//!   opened chest (`chests::grant`). `chests_premium_bonus_coef 1` multiplies nothing (OPEN meaning, identity value).
//! * **`5F 05` unlock** (PROVEN: payload-less request, `pve_unlock_rewards`): charged ONCE for all locked chests of the
//!   player's latest run, then all are rolled and granted. **One access key first, else `unlock_drop_price` (50) gold**
//!   (PROVEN): the options player record's `+0x2D0` = keys the player may spend in the match, the match entry's `+0x54` =
//!   keys spent, `+0x58` = gold spent against the budget `+0x2D4` (end-screen push 0x2001AC00F, paid respawn
//!   0x2003CF5A6..0x2003CF667, respawn prompt 0x20023DE35); the button sends the same payload-less `pve_unlock_rewards`
//!   for a key or gold, so the server decides what is charged.
//!
//!   The server mirrors that: a key when the player owns one (free 1617 first, then premium 1384: the entry-fee order of
//!   `pve::spendable_key`, CHOICE), else the gold. LIKELY: the options carry the player's keys at the match start in
//!   `+0x2D0`, and the keys / gold spent on respawns are taken at the match end ([`charge_respawns`]), so the client's
//!   view and the server's agree. Refused with no change: no run, no locked chests, not enough gold, or a chest that cannot
//!   be rolled. There is no refusal message in the client: nothing is sent (the client disables its own button without gold,
//!   HUD `unlockChestsAvailableMoney`).
//! * **`5F 06` reroll** (PROVEN: `pve_reroll_rewards`, payload-less; `reroll_start_price 50`, `reroll_step 25`, and
//!   `pve_reward_reroll_info [[50, 25]]` say the same): the price of the n-th reroll (n from 0) of a run is
//!   `start + step * n` (LIKELY: the names; the client never computes it, it shows the `cost` it is sent, HUD `rerollCost`).
//!   The count is per run and is kept in every chest row of the run. Rerolled = the opened chests only (CHOICE: a locked chest
//!   has no contents; after an unlock the newly opened chests are rerolled with the others). The previous roll's rewards are
//!   taken back and the new ones granted in ONE profile update (CHOICE: if a previous reward cannot be taken back in full, the
//!   item is equipped, the stack or money was spent, the reroll is refused and nothing is charged). Experience and reputation
//!   are taken back too, but the silver paid for a level reached is not.
//! * **Answers** (LIKELY, the client has no reply reader for `5F 05` / `5F 06`; the only code that changes the screen is
//!   another `83 45 0B`): a fresh `83 45 0B` and the `8B` money, plus the inventory change (`83 45 0F` item records, the
//!   client's own item-list update) for the granted and taken-back rows. A second `83 45 0B` redraws the open screen and
//!   the rewards panel opens the chest by itself (PROVEN, HUD movie `PVEManager.showPVEMatchEndScreen`,
//!   `auto_open_chest`).
//! * **Idempotent** (CHOICE): rows are keyed by (account, match id, chest index); a match id that is already stored does
//!   nothing again; `5F` only reads and changes the rows of the requesting account's latest run.
//! * **One transaction** (`Store::pve_change_chests`): the chest rows and the profile change commit together or not at all,
//!   and only when the stored rows are still the ones the change was computed from. A run end stored twice, or a second
//!   `5F 05` / `5F 06` racing the first, finds other rows and changes nothing.

use accounts::{AccountId, ChestRefusal, Currency, Profile, PveRewardRow, PveRun, UpdateError};
use game_data::chests::Reward;
use game_data::GameData;
use protocol::items::ItemRecord;
use protocol::s2c::*;

use crate::chests::{self, EXPERIENCE, GOLD, REPUTATION, SILVER, SPARE_PARTS};
use crate::dispatch::Ctx;
use crate::peer::{Cross, Peer};
use crate::build;

/// What a profile change touched, so the client can be told.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Touched {
    /// (uid, dict) of inventory rows that were added, changed or removed.
    pub items: Vec<(u32, u16)>,
    pub money: bool,
    pub level: bool,
    pub reputation: bool,
}

impl Touched {
    fn note(&mut self, uid: u32, dict: u16) {
        if !self.items.contains(&(uid, dict)) {
            self.items.push((uid, dict));
        }
    }
}

/// The chest (tier, dictionary id) of a run at `difficulty`: tier 1 at every equipment level (see the module docs), if
/// its table can be rolled.
pub fn rollable_chest(gd: &GameData, difficulty: u8) -> Option<(u8, u16)> {
    gd.pve.chest(1, difficulty).filter(|d| gd.chests.can_open(*d)).map(|d| (1, d))
}

fn currency_of(dict: u16) -> Option<Currency> {
    match dict {
        SILVER => Some(Currency::Silver),
        GOLD => Some(Currency::Gold),
        SPARE_PARTS => Some(Currency::Parts),
        d => game_data::events::slot_of(d).map(crate::events::currency),
    }
}

/// Roll one chest: its single reward (`items_count 1`; a table that gave more keeps the first, logged).
fn roll(cx: &Ctx, chest: u16) -> Option<Reward> {
    let gd = &cx.lobby.data;
    match cx.lobby.roll(|r| gd.chests.open(chest, r)) {
        Ok(v) => {
            if v.len() > 1 {
                log::warn!("PvE chest {chest} rolled {} rewards, only the first is kept", v.len());
            }
            v.into_iter().next()
        }
        Err(e) => {
            log::warn!("PvE chest {chest} cannot be rolled: {e}");
            None
        }
    }
}

/// Grant one reward (the way an opened chest does).
fn give(gd: &GameData, p: &mut Profile, r: &Reward, t: &mut Touched) {
    let (recs, g) = chests::grant(gd, p, &[*r]);
    for rec in recs.iter().filter(|x| x.uid != 0) {
        t.note(rec.uid, rec.dict);
    }
    t.money |= g.money;
    t.level |= g.level;
    t.reputation |= g.reputation;
}

/// Take a granted reward back. None = it cannot be taken back in full (the profile may be half changed: the caller's store
/// transaction then writes nothing).
fn take_back(gd: &GameData, p: &mut Profile, r: &Reward, t: &mut Touched) -> Option<()> {
    if let Some(cur) = currency_of(r.dict) {
        p.spend(cur, r.amount as u64).ok()?;
        t.money = true;
        return Some(());
    }
    match r.dict {
        REPUTATION => {
            let f = p.faction;
            let v = p.fac_points.get_mut(&f)?;
            *v = v.checked_sub(r.amount)?;
            t.reputation = true;
        }
        EXPERIENCE => {
            p.xp = p.xp.checked_sub(r.amount)?;
            t.level = true;
        }
        dict if gd.items.is_stackable(dict) => {
            let uid = p.items.iter().find(|i| i.dict_id == dict && i.count >= r.amount).map(|i| i.uid)?;
            p.consume(uid, r.amount)?;
            t.note(uid, dict);
        }
        dict => {
            // single items: newest first, never one that is equipped
            let worn: Vec<u32> = p.slots.values().copied().collect();
            let mut uids: Vec<u32> = p.items.iter().filter(|i| i.dict_id == dict && !worn.contains(&i.uid)).map(|i| i.uid).collect();
            uids.sort_unstable_by(|a, b| b.cmp(a));
            if uids.len() < r.amount as usize {
                return None;
            }
            for uid in uids.into_iter().take(r.amount as usize) {
                p.remove_item(uid)?;
                t.note(uid, dict);
            }
        }
    }
    Some(())
}

/// The messages that bring the client's lobby state in line after `t` (item list change, money, level, reputation).
fn sync(gd: &GameData, p: &Profile, t: &Touched) -> Vec<S2c> {
    let mut out = Vec::new();
    let recs: Vec<ItemRecord> = t
        .items
        .iter()
        .map(|&(uid, dict)| match p.item(uid) {
            Some(row) => build::record(gd, p, row),
            // removed: record count 0 (PROVEN reader 0x1910FD)
            None => ItemRecord { uid, dict, value: 0, ..Default::default() },
        })
        .collect();
    for chunk in recs.chunks(100) {
        out.push(S2c::Reply(Reply::Inventory(Inventory45::Records { x: 0, items: chunk.to_vec() })));
    }
    if t.money {
        out.push(S2c::Money(build::money_8b(p)));
    }
    if t.level {
        out.push(S2c::Level(build::level_8c(gd, p)));
    }
    if t.reputation {
        out.push(S2c::Factions(build::factions_8e(p)));
    }
    out
}

/// The display records of the opened chests' rewards (the client only shows them: the uid is the chest index + 1).
pub fn drop_records(gd: &GameData, rows: &[PveRewardRow]) -> Vec<ItemRecord> {
    let mut out = Vec::new();
    for r in rows.iter().filter(|r| r.opened && r.reward_dict != 0) {
        let pseudo = currency_of(r.reward_dict).is_some() || r.reward_dict == REPUTATION || r.reward_dict == EXPERIENCE;
        if pseudo || gd.items.is_stackable(r.reward_dict) {
            out.push(ItemRecord { uid: r.idx as u32 + 1, dict: r.reward_dict, value: r.reward_amount, ..Default::default() });
        } else {
            for _ in 0..r.reward_amount.max(1) {
                out.push(ItemRecord { uid: r.idx as u32 + 1, dict: r.reward_dict, value: gd.items.amount_field(r.reward_dict, 100), ..Default::default() });
            }
        }
    }
    out
}

/// Rerolls done on the run (the same number is kept in every row).
fn rerolls_done(rows: &[PveRewardRow]) -> u32 {
    rows.iter().map(|r| r.rerolls).max().unwrap_or(0)
}

/// `83 45 0B` for a run's rows: the opened rewards, `cost` = the price of the next reroll (0 when nothing is opened: there
/// is nothing to reroll, CHOICE), `flag` = chests are still locked.
pub fn drops_message(gd: &GameData, rows: &[PveRewardRow]) -> S2c {
    let items = drop_records(gd, rows);
    let cost = if rows.iter().any(|r| r.opened) { gd.pve.reroll_cost(rerolls_done(rows)) } else { 0 };
    S2c::Reply(Reply::Inventory(Inventory45::PveDrops { items, cost, flag: rows.iter().any(|r| !r.opened) }))
}

/// The paid respawns of one player at the end of a mission: what the world charged against his options budget, as the world host read it (`keys_spent` = world entry `+0x54`,
/// `gold_spent` = `+0x58`). Keys: the free key first, then the bought one; gold through `8B`; the inventory records of
/// the used keys. Won and failed missions alike, before the run's `83 45 0B` (so a `5F 05` cannot spend a key the player
/// already used). The budget was his holdings at the start, so a shortfall can only come from a spend after the start
/// (the lobby is not locked while he plays): what is missing is logged and not charged (CHOICE).
pub fn charge_respawns(cx: &mut Ctx, match_id: u32, to: AccountId, keys: u32, gold: u32) {
    if keys == 0 && gold == 0 {
        return;
    }
    let gd = cx.lobby.data.clone();
    let done = crate::dispatch::update_account(cx, to, |p| {
        let mut t = Touched::default();
        let mut used = Vec::new();
        for _ in 0..keys {
            let Some((uid, dict)) = crate::pve::spendable_key(p) else { break };
            if p.consume(uid, 1).is_none() {
                break;
            }
            t.note(uid, dict);
            used.push(dict);
        }
        let g = gold.min(p.gold);
        if g > 0 && p.spend(Currency::Gold, u64::from(g)).is_ok() {
            t.money = true;
        }
        let msgs = sync(&gd, p, &t);
        (used, g, msgs)
    });
    let Some((used, g, msgs)) = done else { return };
    if used.len() as u32 != keys || g != gold {
        log::warn!("PvE match {match_id:#x}: account {to} paid respawns: the world charged {keys} key(s) and {gold} gold, the account had {} key(s) and {g} gold left", used.len());
    }
    log::info!("PvE match {match_id:#x}: account {to} pays the respawns: keys {used:?}, {g} gold");
    for msg in msgs {
        cx.fx.push(Cross { to, msg });
    }
}

/// Mission end (through `pve::record_run_notify` after the run was stored for the first time): the chest rows of
/// every player, the free chests opened and granted, and the `83 45 0B` pushed to each player (with the inventory / money
/// updates of the grant before it).
pub fn store_run_chests(cx: &mut Ctx, run: &PveRun, run_key: i64, now: i64) {
    let gd = cx.lobby.data.clone();
    let day = now.div_euclid(86_400);
    let found = match gd.pve.mission(run.mission) {
        Some(m) => (run.chests_found as u32).min(m.chests_total) as u8,
        None => run.chests_found,
    };
    let chest = rollable_chest(&gd, run.difficulty);
    for pl in &run.players {
        let to: AccountId = pl.account;
        let eligible = run.success && pl.score > 0 && found > 0;
        let (Some((tier, dict)), true) = (chest, eligible) else {
            cx.fx.push(Cross { to, msg: drops_message(&gd, &[]) });
            continue;
        };
        let mut rows: Vec<PveRewardRow> = (0..found)
            .map(|i| PveRewardRow { idx: i, chest_dict: dict, tier, difficulty: run.difficulty, opened: false, reward_dict: 0, reward_amount: 0, rerolls: 0, paid: false })
            .collect();
        let mut rolls = Vec::new();
        for r in rows.iter_mut().take(free_chests(cx, &gd, to, day)) {
            if let Some(w) = roll(cx, dict) {
                (r.opened, r.reward_dict, r.reward_amount) = (true, w.dict, w.amount);
                rolls.push(w);
            }
        }
        let stored = cx.lobby.store.pve_change_chests(to, run_key, &[], &rows, |p| {
            let mut t = Touched::default();
            for w in &rolls {
                give(&gd, p, w, &mut t);
            }
            Ok::<_, ()>(t)
        });
        let t = match stored {
            Ok(t) => t,
            Err(UpdateError::Rejected(ChestRefusal::Stale)) => {
                log::info!("PvE match {:#x}: chests of account {to} were stored before (run {run_key:#x})", run.match_id);
                continue;
            }
            Err(e) => {
                log::error!("PvE chests of account {to} not stored: {e:?}");
                continue;
            }
        };
        log::info!("PvE match {:#x} (run {run_key:#x}): account {to} found {found} chests (tier {tier}), {} opened free: {rolls:?}", run.match_id, rolls.len());
        let Ok(p) = cx.lobby.store.load_profile(to) else { continue };
        for msg in sync(&gd, &p, &t) {
            cx.fx.push(Cross { to, msg });
        }
        cx.fx.push(Cross { to, msg: drops_message(&gd, &rows) });
    }
}

/// How many chests of a won run open free: all on the player's first won mission of `day` (its win was already counted by
/// `pve::win_key`), else `free_chests_num`.
fn free_chests(cx: &Ctx, gd: &GameData, account: AccountId, day: i64) -> usize {
    match cx.lobby.store.pve_wins_on(account, day) {
        Ok(1) => usize::MAX,
        Ok(_) => gd.pve.prices.free_chests_num as usize,
        Err(e) => {
            log::error!("PvE wins of account {account} not read: {e}");
            gd.pve.prices.free_chests_num as usize
        }
    }
}

/// The player's latest run with chests: (run key, rows). None when his latest run has no chest rows. Every run has its
/// own key (`accounts::pve_run_key`), so this is the run of the screen the player just saw, never an older run of the
/// same maze (same wire id).
fn latest(cx: &Ctx, me: AccountId) -> Option<(i64, Vec<PveRewardRow>)> {
    let store = &cx.lobby.store;
    let run = store.pve_latest_run(me).ok().flatten()?;
    let rows = store.pve_rewards(me, run).ok()?;
    (!rows.is_empty()).then_some((run, rows))
}

/// Send the new state of a run to its player: the grant's updates, then the screen.
fn answer(cx: &Ctx, peer: &mut Peer, me: AccountId, rows: &[PveRewardRow], t: &Touched) {
    let gd = &cx.lobby.data;
    if let Ok(p) = cx.lobby.store.load_profile(me) {
        for msg in sync(gd, &p, t) {
            peer.push(&msg);
        }
    }
    peer.push(&drops_message(gd, rows));
}

/// Log a refused `5F 05` / `5F 06` (the client has no refusal message, so nothing is sent).
fn refused(peer: &Peer, what: &str, price: u32, e: UpdateError<ChestRefusal<&str>>) {
    match e {
        UpdateError::Rejected(ChestRefusal::Rejected("gold")) => log::info!("{}: {what} refused: {price} gold needed", peer.addr),
        UpdateError::Rejected(ChestRefusal::Rejected(why)) => log::info!("{}: {what} refused: {why}", peer.addr),
        UpdateError::Rejected(ChestRefusal::Stale) => log::info!("{}: {what} refused: the chests changed meanwhile", peer.addr),
        UpdateError::Store(e) => log::error!("{}: {what} not saved: {e}", peer.addr),
    }
}

/// `5F 05`: open all locked chests of the latest run for `unlock_drop_price` gold.
pub fn unlock(cx: &mut Ctx, peer: &mut Peer) {
    let Some(me) = peer.account else { return };
    let gd = cx.lobby.data.clone();
    let Some((run, mut rows)) = latest(cx, me) else {
        log::info!("{}: 5F 05 unlock refused: no run with chests", peer.addr);
        return;
    };
    let locked: Vec<usize> = (0..rows.len()).filter(|&i| !rows[i].opened).collect();
    if locked.is_empty() {
        log::info!("{}: 5F 05 unlock refused: no locked chests in run {run:#x}", peer.addr);
        return;
    }
    let mut rolls = Vec::new();
    for &i in &locked {
        match roll(cx, rows[i].chest_dict) {
            Some(w) => rolls.push(w),
            None => return log::info!("{}: 5F 05 unlock refused: chest {} cannot be rolled", peer.addr, rows[i].chest_dict),
        }
    }
    let price = gd.pve.prices.unlock_drop_price;
    let before = rows.clone();
    for (&i, w) in locked.iter().zip(&rolls) {
        (rows[i].opened, rows[i].paid, rows[i].reward_dict, rows[i].reward_amount) = (true, true, w.dict, w.amount);
    }
    let done = cx.lobby.store.pve_change_chests(me, run, &before, &rows, |p| {
        let mut t = Touched::default();
        let paid = match crate::pve::spendable_key(p) {
            Some((uid, dict)) => {
                p.consume(uid, 1).ok_or("key")?;
                t.note(uid, dict);
                format!("key {dict}")
            }
            None => {
                p.spend(Currency::Gold, price as u64).map_err(|_| "no key and not enough gold")?;
                t.money = true;
                format!("{price} gold")
            }
        };
        for w in &rolls {
            give(&gd, p, w, &mut t);
        }
        Ok((t, paid))
    });
    let (t, paid) = match done {
        Ok(v) => v,
        Err(e) => return refused(peer, "5F 05 unlock", price, e),
    };
    log::info!("{}: 5F 05 unlocked {} chests of run {run:#x} for {paid}: {rolls:?}", peer.addr, locked.len());
    answer(cx, peer, me, &rows, &t);
}

/// `5F 06`: reroll the opened chests of the latest run for `start + step * rerolls done` gold.
pub fn reroll(cx: &mut Ctx, peer: &mut Peer) {
    let Some(me) = peer.account else { return };
    let gd = cx.lobby.data.clone();
    let Some((run, mut rows)) = latest(cx, me) else {
        log::info!("{}: 5F 06 reroll refused: no run with chests", peer.addr);
        return;
    };
    let opened: Vec<usize> = (0..rows.len()).filter(|&i| rows[i].opened && rows[i].reward_dict != 0).collect();
    if opened.is_empty() {
        log::info!("{}: 5F 06 reroll refused: no opened chests in run {run:#x}", peer.addr);
        return;
    }
    let n = rerolls_done(&rows);
    let price = gd.pve.reroll_cost(n);
    let mut rolls = Vec::new();
    for &i in &opened {
        match roll(cx, rows[i].chest_dict) {
            Some(w) => rolls.push(w),
            None => return log::info!("{}: 5F 06 reroll refused: chest {} cannot be rolled", peer.addr, rows[i].chest_dict),
        }
    }
    let old: Vec<Reward> = opened.iter().map(|&i| Reward { dict: rows[i].reward_dict, amount: rows[i].reward_amount }).collect();
    let before = rows.clone();
    for (&i, w) in opened.iter().zip(&rolls) {
        (rows[i].reward_dict, rows[i].reward_amount) = (w.dict, w.amount);
    }
    for r in rows.iter_mut() {
        r.rerolls = n + 1;
    }
    let done = cx.lobby.store.pve_change_chests(me, run, &before, &rows, |p| {
        p.spend(Currency::Gold, price as u64).map_err(|_| "gold")?;
        let mut t = Touched { money: true, ..Default::default() };
        for w in &old {
            take_back(&gd, p, w, &mut t).ok_or("the old rewards are gone")?;
        }
        for w in &rolls {
            give(&gd, p, w, &mut t);
        }
        Ok(t)
    });
    let t = match done {
        Ok(t) => t,
        Err(e) => return refused(peer, "5F 06 reroll", price, e),
    };
    log::info!("{}: 5F 06 reroll {} of run {run:#x} for {price} gold: {old:?} -> {rolls:?}", peer.addr, n + 1);
    answer(cx, peer, me, &rows, &t);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gd() -> GameData {
        game_data::testing::tables()
    }

    fn row(idx: u8, opened: bool, dict: u16, amount: u32, rerolls: u32) -> PveRewardRow {
        PveRewardRow { idx, chest_dict: 1483, tier: 1, difficulty: 0, opened, reward_dict: dict, reward_amount: amount, rerolls, paid: false }
    }

    #[test]
    fn the_chest_is_the_tier_1_chest_of_the_difficulty() {
        game_data::require_tables!();
        let g = gd();
        // 0.69d data keeps only the tier-1 tables (pve_mission_1_chest_rank_11..13), see the module docs
        assert_eq!(rollable_chest(&g, 0), Some((1, 1483)));
        assert_eq!(rollable_chest(&g, 1), Some((1, 1468)));
        assert_eq!(rollable_chest(&g, 2), Some((1, 1469)));
        assert_eq!(rollable_chest(&g, 3), None, "no such difficulty");
    }

    #[test]
    fn money_experience_and_stacks_are_taken_back_exactly() {
        game_data::require_tables!();
        let g = gd();
        let mut p = Profile::starter();
        let mut t = Touched::default();
        let (silver, parts, xp) = (p.silver, p.parts, p.xp);
        give(&g, &mut p, &Reward { dict: SILVER, amount: 120 }, &mut t);
        give(&g, &mut p, &Reward { dict: SPARE_PARTS, amount: 7 }, &mut t);
        assert_eq!((p.silver, p.parts), (silver + 120, parts + 7));
        assert!(take_back(&g, &mut p, &Reward { dict: SILVER, amount: 120 }, &mut t).is_some());
        assert!(take_back(&g, &mut p, &Reward { dict: SPARE_PARTS, amount: 7 }, &mut t).is_some());
        assert_eq!((p.silver, p.parts), (silver, parts));
        // spent in the meantime: refused
        give(&g, &mut p, &Reward { dict: SILVER, amount: 50 }, &mut t);
        p.silver = 10;
        assert!(take_back(&g, &mut p, &Reward { dict: SILVER, amount: 50 }, &mut t).is_none());
        // experience
        give(&g, &mut p, &Reward { dict: EXPERIENCE, amount: 30 }, &mut t);
        assert_eq!(p.xp, xp + 30);
        assert!(t.level);
        assert!(take_back(&g, &mut p, &Reward { dict: EXPERIENCE, amount: 30 }, &mut t).is_some());
        assert_eq!(p.xp, xp);
        // a stack: the same row loses the units; a smaller stack than the reward refuses
        let stack = g.items.iter().find(|i| i.is_stackable && !matches!(i.dict_id, SILVER | GOLD | SPARE_PARTS | REPUTATION | EXPERIENCE) && game_data::events::slot_of(i.dict_id).is_none()).unwrap().dict_id;
        let mut p = Profile::starter();
        let mut t = Touched::default();
        let before = p.items.iter().filter(|i| i.dict_id == stack).map(|i| i.count).sum::<u32>();
        give(&g, &mut p, &Reward { dict: stack, amount: 5 }, &mut t);
        assert_eq!(p.items.iter().filter(|i| i.dict_id == stack).map(|i| i.count).sum::<u32>(), before + 5);
        assert!(take_back(&g, &mut p, &Reward { dict: stack, amount: 5 }, &mut t).is_some());
        assert_eq!(p.items.iter().filter(|i| i.dict_id == stack).map(|i| i.count).sum::<u32>(), before);
        assert!(take_back(&g, &mut p, &Reward { dict: stack, amount: before + 1 }, &mut t).is_none(), "not enough units");
        assert!(!t.items.is_empty());
    }

    #[test]
    fn a_single_item_is_taken_back_unless_it_is_equipped() {
        game_data::require_tables!();
        let g = gd();
        let dict = g.items.iter().find(|i| !i.is_stackable && !g.items.is_module(i.dict_id) && game_data::events::slot_of(i.dict_id).is_none() && !matches!(i.dict_id, SILVER | GOLD | SPARE_PARTS | REPUTATION | EXPERIENCE)).unwrap().dict_id;
        let mut p = Profile::starter();
        let n = p.items.len();
        let mut t = Touched::default();
        give(&g, &mut p, &Reward { dict, amount: 1 }, &mut t);
        assert_eq!(p.items.len(), n + 1);
        let uid = t.items.iter().find(|x| x.1 == dict).unwrap().0;
        // worn: refused, nothing removed
        p.slots.insert(9, uid);
        assert!(take_back(&g, &mut p, &Reward { dict, amount: 1 }, &mut t).is_none());
        assert_eq!(p.items.len(), n + 1);
        p.slots.remove(&9);
        assert!(take_back(&g, &mut p, &Reward { dict, amount: 1 }, &mut t).is_some());
        assert_eq!(p.items.len(), n);
        // the client is told: the row is gone = a record with count 0
        let msgs = sync(&g, &p, &t);
        match &msgs[0] {
            S2c::Reply(Reply::Inventory(Inventory45::Records { items, .. })) => assert!(items.iter().any(|r| r.uid == uid && r.dict == dict && r.value == 0)),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn the_screen_message_prices_the_next_reroll_and_flags_locked_chests() {
        game_data::require_tables!();
        let g = gd();
        let rows = [row(0, true, SILVER, 100, 2), row(1, true, 925, 4, 2), row(2, false, 0, 0, 2)];
        match drops_message(&g, &rows) {
            S2c::Reply(Reply::Inventory(Inventory45::PveDrops { items, cost, flag })) => {
                assert_eq!((items.len(), cost, flag), (2, 100, true), "50 + 25 * 2 done, a chest is locked");
                assert_eq!((items[0].dict, items[0].value, items[1].dict, items[1].value), (SILVER, 100, 925, 4));
            }
            other => panic!("{other:?}"),
        }
        // nothing opened (a lost run): empty, no price, no locked chests
        match drops_message(&g, &[]) {
            S2c::Reply(Reply::Inventory(Inventory45::PveDrops { items, cost, flag })) => assert_eq!((items.len(), cost, flag), (0, 0, false)),
            other => panic!("{other:?}"),
        }
    }
}
