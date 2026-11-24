//! The treasure hunt ("item search", the weekly drop-card roulette): `46 07`, `5A 02`, `5A 03` and the `AA` push.
//! Notes: docs/notes/items.md section 3a. Data: `game_data::hunt` (`weekly_drop_configs`).
//!
//! What the client does (PROVEN, exe + AS3):
//! * it shows `AA` sub 0 (state: the page's items, level, min / max level, time left, drop-card counts). The two counts
//!   are the stacks of dict 153 (paid / bonus drop card, `b`) and dict 1132 (free drop card, `c`); the client updates
//!   them itself from `83 46 00` records (0x19388E..0x1938A2) and the server sends sub 1 after every change;
//! * `46 07 u8` starts a search: 0 = pay with drop cards (a free card first, "Free stash maps are used before the bonus
//!   ones", `st_treasure_maps_info`), 1 = pay gold (`ActionBigStoreRenderer.onButtonClicked`, gold button = the
//!   `premium_money_to_open` price); the answer is `83 46 07 u8 index of the found item, u8 n, n records`
//!   (the roulette spins to that index), or `84 x 46` with a reason (`unblock_roulette`);
//! * `5A 03 i8 delta, u8 gold` (`RewardRerollWindow.changeRouletteLevel`): delta 0 = update the list (a new page of the
//!   same level), -1 / +1 = one level lower / higher, paid with one drop card (`MAP_ACTION_COST` = 1) or the gold price;
//! * `5A 02 u8` is sent when the hunt window opens and when the "new item" message of a found item is closed (not when
//!   every item is found): the server answers with the state.
//!
//! Rules (each marked): PROVEN = the data / client; CHOICE = nothing states it.
//! * Period: the hunt is weekly (`weekly_drop`), CHOICE Monday 00:00 UTC; a new period gives a new page and resets the
//!   free cards used (`d`); free cards held stay (the client text: "they won't disappear when Treasure Hunt resets").
//! * Free cards: a free card drops after a finished match with the chance of the client text (20 %, "active in battle,
//!   even if you lose"), only while `held + used < max_free_drop_cards_in_period` (the text: new ones won't drop while the
//!   two free slots are taken). `drops.options` also carries a 15-entry `prob` list for dict 1132 whose index meaning
//!   is not provable: OPEN, not used.
//! * Page: picked by `roulette_prob`, items resolved once per page (a `dict_id` list is a pool, uniform); each search
//!   finds one not-yet-found item with probability proportional to `prob`; the amount is uniform in `[min, max]`.
//! * Level: the highest level offered is the active faction's level capped at 5 (`reroll_window_max_level_reached`:
//!   "Maximum treasure tier is limited by your faction level"); the lowest is the first level of the data (1); a new
//!   hunt starts at the highest.

use std::collections::HashSet;

use accounts::{HuntEntry, HuntState, Profile};
use game_data::craft::Roll;
use game_data::chests::Reward;
use game_data::hunt::{HuntConfig, FREE_CARD, PAID_CARD};
use game_data::levels::faction_level;
use game_data::MatchFacts;
use protocol::items::ItemRecord;
use protocol::s2c::hunt::*;
use protocol::s2c::*;

use crate::achievements;
use crate::chests::Granted;
use crate::dispatch::{update, Ctx};
use crate::peer::Peer;
use crate::premium::unix_now;

/// Length of a hunt period: one week.
pub const PERIOD_SECS: i64 = 7 * 86_400;
/// Chance of a free drop card after a finished match (client text `st_treasure_maps_info`: "20% chance").
pub const FREE_CARD_CHANCE: f64 = 0.20;

/// Start (unix seconds, Monday 00:00 UTC) of the period `now` is in.
pub fn period_start(now: i64) -> i64 {
    // the unix epoch is a Thursday: Monday is 4 days later
    (now - 4 * 86_400).div_euclid(PERIOD_SECS) * PERIOD_SECS + 4 * 86_400
}

/// How a search or a level change is paid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pay {
    Cards,
    Gold,
}

impl Pay {
    pub fn from_byte(b: u8) -> Pay {
        if b == 0 { Pay::Cards } else { Pay::Gold }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    NotEnoughCards,
    NotEnoughMoney,
    AllOpened,
    /// level out of range, level change not allowed, no hunt data
    Failed,
}

impl Refused {
    pub fn denied(self) -> SearchDenied {
        match self {
            Refused::NotEnoughCards => SearchDenied::NotEnoughCards,
            Refused::NotEnoughMoney => SearchDenied::NotEnoughMoney,
            Refused::AllOpened => SearchDenied::AllOpened,
            Refused::Failed => SearchDenied::Failed,
        }
    }
}

/// Item stacks (uid, units) a payment took, for the `90` message.
pub type Taken = Vec<(u32, u32)>;

fn stack(p: &Profile, dict: u16) -> Option<(u32, u32)> {
    p.items.iter().find(|i| i.dict_id == dict && i.count > 0).map(|i| (i.uid, i.count))
}

/// Drop cards held: (paid / bonus dict 153, free dict 1132).
pub fn card_counts(p: &Profile) -> (u32, u32) {
    (stack(p, PAID_CARD).map_or(0, |s| s.1), stack(p, FREE_CARD).map_or(0, |s| s.1))
}

/// The counts of the `AA` message.
pub fn counts(p: &Profile) -> HuntCounts {
    let (b, c) = card_counts(p);
    HuntCounts { b, c, d: p.hunt.free_used }
}

/// Highest roulette level of the account.
pub fn max_level(cfg: &HuntConfig, p: &Profile) -> u8 {
    let pts = p.fac_points.get(&p.faction).copied().unwrap_or(0);
    let by_faction = faction_level(pts).clamp(1, 5) as u8;
    by_faction.min(cfg.top_level()).max(cfg.min_level())
}

/// Weapon and armour dictionary ids the player owns (a skinned weapon counts as itself and as its base weapon).
pub fn owned_gear(gd: &game_data::GameData, p: &Profile) -> HashSet<u16> {
    p.items
        .iter()
        .flat_map(|i| [i.dict_id, p.real_dict(i)])
        .filter(|d| gd.items.is_weapon(*d) || gd.items.is_armour(*d))
        .collect()
}

/// A new page: picked by `roulette_prob`, every pool resolved to one id. A weapon or armour piece the player owns is
/// never drawn while its pool has another id ([`HuntConfig::resolve_page_skipping`]; all owned: the plain draw, CHOICE).
fn new_page(gd: &game_data::GameData, cfg: &HuntConfig, p: &mut Profile, r: &mut dyn Roll) -> bool {
    let owned = owned_gear(gd, p);
    let h = &mut p.hunt;
    let Some(page) = cfg.pick_page(h.level, r) else {
        h.entries.clear();
        return false;
    };
    h.page = page;
    let (ids, avoided) = cfg.resolve_page_skipping(h.level, page, &|d| owned.contains(&d), r);
    if avoided > 0 {
        log::info!("hunt page level {} page {page}: {avoided} entries drew another pool id instead of owned gear", h.level);
    }
    h.entries = ids.into_iter().map(|dict| HuntEntry { dict, found: false }).collect();
    true
}

/// Bring the hunt of the profile up to date: make it (first time), start the new period, keep the level inside the
/// limits. Returns whether anything changed.
pub fn ensure(gd: &game_data::GameData, cfg: &HuntConfig, p: &mut Profile, now: i64, r: &mut dyn Roll) -> bool {
    let start = period_start(now);
    let (min, max) = (cfg.min_level(), max_level(cfg, p));
    let before = p.hunt.clone();
    let h = &mut p.hunt;
    let renew = if h.period == 0 {
        h.period = start;
        h.level = max;
        h.free_used = 0;
        true
    } else if h.period != start {
        h.period = start;
        h.free_used = 0;
        h.level = h.level.clamp(min, max);
        true
    } else if h.level < min || h.level > max || h.entries.is_empty() {
        h.level = h.level.clamp(min, max);
        true
    } else {
        false
    };
    if renew {
        new_page(gd, cfg, p, r);
    }
    p.hunt != before
}

/// The modifier list of page entry `idx` when it is a module: the pairs the hunt window shows (`AA` sub 0) and the found
/// module gets ([`search`]). Drawn from the blue table (`Source::Rare`: the page entries of the modules carry
/// `rare_level` 1 = table value 1, and the 0.51b notes put "high quality modules (rare)" into the treasure hunt;
/// LIKELY, docs/notes/workshop.md section 7) with a generator seeded by the page (period, level, page, the entry's
/// position and every entry's dict, which differ per account), so the shown and the given pairs agree without storing
/// them. Includes the type's override pairs, at most 4 (the AA reader's buffer, 0x200181080). Empty for non-modules.
pub fn entry_mods(gd: &game_data::GameData, h: &HuntState, idx: usize) -> Vec<(u16, u16)> {
    let Some(e) = h.entries.get(idx) else { return vec![] };
    if !gd.items.is_module(e.dict) || gd.items.is_stackable(e.dict) {
        return vec![];
    }
    let mut x: u64 = 0xcbf2_9ce4_8422_2325;
    let mut mix = |v: u64| {
        for b in v.to_le_bytes() {
            x = (x ^ b as u64).wrapping_mul(0x0100_0000_01b3);
        }
    };
    mix(h.period as u64);
    mix(((h.level as u64) << 16) | ((h.page as u64) << 8));
    mix(idx as u64);
    for e in &h.entries {
        mix(e.dict as u64);
    }
    let mut r = game_data::SplitMix(x);
    match gd.module_drops.roll(e.dict, game_data::module_drops::Source::Rare, &mut r) {
        Some(l) => gd.modules.instance_mods(e.dict, &l),
        None => vec![],
    }
}

/// The `AA` sub 0 message for the profile (the hunt must exist: call [`ensure`] first).
pub fn state(gd: &game_data::GameData, cfg: &HuntConfig, name: &str, p: &Profile, now: i64) -> WeeklyDrop {
    let h = &p.hunt;
    if h.period == 0 || h.entries.is_empty() {
        return WeeklyDrop::State { name: name.into(), a: 0, counts: HuntCounts::default(), page: None };
    }
    let left = (h.period + PERIOD_SECS - now).clamp(1, u32::MAX as i64) as u32;
    WeeklyDrop::State {
        name: name.into(),
        // non-zero = a hunt exists (LIKELY an id; the exe only tests it); the week number keeps it stable per period
        a: (h.period / PERIOD_SECS) as u32 + 1,
        counts: counts(p),
        page: Some(HuntPage {
            time_left: left,
            level: h.level,
            page: h.page,
            max_level: max_level(cfg, p),
            min_level: cfg.min_level(),
            items: h.entries.iter().enumerate().map(|(i, e)| HuntItem { dict: e.dict, not_found: !e.found, mods: entry_mods(gd, h, i) }).collect(),
        }),
    }
}

/// The `AA` sub 1 message (counts only).
pub fn counts_msg(name: &str, p: &Profile) -> WeeklyDrop {
    WeeklyDrop::Counts { name: name.into(), a: (p.hunt.period / PERIOD_SECS) as u32 + 1, counts: counts(p) }
}

/// Take `n` drop cards, free ones first. Nothing is taken when there are fewer than `n`.
fn spend_cards(p: &mut Profile, n: u32) -> Result<Taken, Refused> {
    let (paid, free) = card_counts(p);
    if paid + free < n {
        return Err(Refused::NotEnoughCards);
    }
    let from_free = free.min(n);
    let mut taken = Taken::new();
    if let Some((uid, _)) = stack(p, FREE_CARD).filter(|_| from_free > 0) {
        p.consume(uid, from_free);
        p.hunt.free_used = p.hunt.free_used.saturating_add(from_free.min(255) as u8);
        taken.push((uid, from_free));
    }
    let rest = n - from_free;
    if let Some((uid, _)) = stack(p, PAID_CARD).filter(|_| rest > 0) {
        p.consume(uid, rest);
        taken.push((uid, rest));
    }
    Ok(taken)
}

fn pay(cfg: &HuntConfig, p: &mut Profile, how: Pay, cards: u32) -> Result<Taken, Refused> {
    match how {
        Pay::Cards => spend_cards(p, cards),
        Pay::Gold => p.spend(accounts::Currency::Gold, cfg.premium_money_to_open as u64).map(|_| Taken::new()).map_err(|_| Refused::NotEnoughMoney),
    }
}

/// What one search gave.
#[derive(Debug)]
pub struct Found {
    /// index of the found item in the page (the roulette stops there)
    pub index: u8,
    pub rewards: Vec<Reward>,
    pub records: Vec<ItemRecord>,
    pub granted: Granted,
    pub taken: Taken,
    pub gold: bool,
}

/// `46 07`: pay, pick an item not found yet, give it.
pub fn search(gd: &game_data::GameData, cfg: &HuntConfig, p: &mut Profile, how: Pay, now: i64, r: &mut dyn Roll) -> Result<Found, Refused> {
    ensure(gd, cfg, p, now, r);
    if p.hunt.entries.iter().all(|e| e.found) {
        return Err(Refused::AllOpened);
    }
    let found: Vec<bool> = p.hunt.entries.iter().map(|e| e.found).collect();
    let (level, page) = (p.hunt.level, p.hunt.page);
    let idx = cfg.pick_item(level, page, &found, r).ok_or(Refused::AllOpened)?;
    let taken = pay(cfg, p, how, cfg.drop_cards_to_open)?;
    let module_mods = entry_mods(gd, &p.hunt, idx);
    p.hunt.entries[idx].found = true;
    let amount = cfg.amount(level, page, idx, r).max(1);
    let rewards = vec![Reward { dict: p.hunt.entries[idx].dict, amount }];
    // a module gets the pairs the window showed (the records add its type's override pairs only when missing)
    let mods = if module_mods.is_empty() { vec![] } else { vec![module_mods; amount as usize] };
    let (records, granted) = crate::chests::grant_with_mods(gd, p, &rewards, mods);
    let hidden = cfg.page(level, page).and_then(|pg| pg.items.get(idx)).map_or(0, |it| it.item_quality as usize);
    let records = with_hidden_mods(gd, p, records, rewards[0].dict, hidden, r);
    Ok(Found { index: idx as u8, rewards, records, granted, taken, gold: how == Pay::Gold })
}

/// The hidden modifiers of a found item: a page entry's `item_quality` is, for a non-module item, the number of
/// "unknown" modifiers the hunt window promises (`unknown_mod_count`, UI builder 0x20024E9C3, PROVEN; in the data 3
/// for 74 weapons, 0 elsewhere). Each new unit of `dict` gets that many, rolled by the workshop rules
/// ([`game_data::CraftData::roll_mods`], CHOICE), stored with the item and sent in its record.
/// Modules show the `AA` pairs instead (they get them in [`search`], see [`entry_mods`]) and stacks carry no modifiers.
fn with_hidden_mods(gd: &game_data::GameData, p: &mut Profile, records: Vec<ItemRecord>, dict: u16, n: usize, r: &mut dyn Roll) -> Vec<ItemRecord> {
    if n == 0 || gd.items.is_module(dict) || gd.items.is_stackable(dict) {
        return records;
    }
    records
        .into_iter()
        .map(|rec| {
            if rec.uid == 0 || rec.dict != dict {
                return rec;
            }
            let mods = gd.craft.roll_mods(dict, n, r);
            log::debug!("hunt item {} (dict {dict}): hidden modifiers {mods:?}", rec.uid);
            p.mods.insert(rec.uid, mods);
            match p.item(rec.uid).copied() {
                Some(row) => crate::build::record(gd, p, &row),
                None => rec,
            }
        })
        .collect()
}

/// `5A 03`: `delta` 0 = a new page of the same level, +-1 = the next level; paid with one card (`MAP_ACTION_COST`) or
/// the gold price. `can_change_drop_level` gates only +-1: the client keeps Refresh visible and enabled whatever the flag
/// (PROVEN, RewardRerollWindow.showChangeDropLevel hides only the level buttons, enableButtons 1033-1036 sets
/// `buttonRefresh.enabled = true`).
pub fn change_level(gd: &game_data::GameData, cfg: &HuntConfig, p: &mut Profile, delta: i8, how: Pay, now: i64, r: &mut dyn Roll) -> Result<Taken, Refused> {
    ensure(gd, cfg, p, now, r);
    if delta != 0 && !cfg.can_change_drop_level {
        return Err(Refused::Failed);
    }
    let (min, max) = (cfg.min_level() as i16, max_level(cfg, p) as i16);
    let to = p.hunt.level as i16 + delta.clamp(-1, 1) as i16;
    if to < min || to > max {
        return Err(Refused::Failed);
    }
    let taken = pay(cfg, p, how, 1)?;
    p.hunt.level = to as u8;
    new_page(gd, cfg, p, r);
    Ok(taken)
}

/// After a finished match: maybe a free drop card. Returns whether one was added.
pub fn free_card_drop(cfg: &HuntConfig, p: &mut Profile, r: &mut dyn Roll) -> bool {
    let (_, held) = card_counts(p);
    let used = p.hunt.free_used as u32;
    if held + used >= cfg.max_free_drop_cards_in_period as u32 || held >= cfg.max_free_drop_cards as u32 {
        return false;
    }
    if r.unit() >= FREE_CARD_CHANCE {
        return false;
    }
    match stack(p, FREE_CARD) {
        Some((uid, _)) => p.item_mut(uid).unwrap().count += 1,
        None => {
            p.add_item(FREE_CARD, 1);
        }
    }
    true
}

// ---- glue (messages to the peer)

fn cfg<'a>(cx: &Ctx<'a>) -> Option<&'a HuntConfig> {
    if !cx.lobby.cfg.treasure_hunt {
        return None;
    }
    cx.lobby.data.hunt.default_config()
}

/// Sign-in: make / roll over the hunt before the login pushes.
pub fn sync_on_login(cx: &Ctx, id: accounts::AccountId) {
    let Some(c) = cfg(cx) else { return };
    let now = unix_now();
    let gd = &cx.lobby.data;
    let res = cx.lobby.store.update_profile(id, |p| Ok::<_, ()>(cx.lobby.roll(|r| ensure(gd, c, p, now, r))));
    if let Err(e) = res {
        log::error!("treasure hunt of account {id}: {e:?}");
    }
}

/// The `AA` of the login pushes.
pub fn login_push(cx: &Ctx, p: &Profile) -> Option<S2c> {
    let c = cfg(cx)?;
    Some(state(&cx.lobby.data, c, game_data::hunt::DEFAULT_CONFIG, p, unix_now()).push())
}

fn push_taken(peer: &mut Peer, taken: &Taken) {
    let mut entries = Vec::new();
    for &(uid, mut left) in taken {
        while left > 0 {
            let step = left.min(255);
            entries.push((uid, step as u8));
            left -= step;
        }
    }
    for chunk in entries.chunks(255) {
        peer.push(&S2c::ItemCounts(ItemCounts90 { entries: chunk.to_vec() }));
    }
}

fn push_state(cx: &Ctx, peer: &mut Peer) {
    let (Some(c), Some(id)) = (cfg(cx), peer.account) else { return };
    let now = unix_now();
    let gd = &cx.lobby.data;
    let res = cx.lobby.store.update_profile(id, |p| Ok::<_, ()>(cx.lobby.roll(|r| ensure(gd, c, p, now, r))));
    if let Err(e) = res {
        log::error!("{}: treasure hunt of account {id}: {e:?}", peer.addr);
        return;
    }
    if let Ok(p) = cx.lobby.store.load_profile(id) {
        peer.push(&state(&cx.lobby.data, c, game_data::hunt::DEFAULT_CONFIG, &p, now).push());
    }
}

/// `5A 02`: the hunt window opened / the found item was acknowledged: the current state.
pub fn opened(cx: &mut Ctx, peer: &mut Peer) {
    push_state(cx, peer);
}

/// `46 07 u8`.
pub fn search_request(cx: &mut Ctx, peer: &mut Peer, how: u8) {
    let Some(c) = cfg(cx) else {
        log::info!("{}: 46 07: treasure hunt is off", peer.addr);
        return;
    };
    let gd = &cx.lobby.data;
    let now = unix_now();
    let how = Pay::from_byte(how);
    let rw = cx.lobby.cfg.achievement_rewards;
    let res = update(cx, peer, |p| {
        let before = p.clone();
        let out = cx.lobby.roll(|r| search(gd, c, p, how, now, r));
        // a refused search keeps the profile as it was (ensure may have made the hunt: that is fine, keep it)
        if out.is_err() {
            let hunt = p.hunt.clone();
            *p = before;
            p.hunt = hunt;
        }
        // achievement 406 ("Receive all rewards in the Treasure Hunt"): this find was the last of the page; a find
        // that leaves items open still updates the state figures (the item may have paid experience / reputation)
        out.map(|f| {
            let ach = if p.hunt.entries.iter().all(|e| e.found) {
                achievements::event_in(gd, rw, p, achievements::Event::HuntCompleted)
            } else {
                let before = p.items.len();
                (achievements::refresh_state(gd, rw, p), before)
            };
            (f, ach)
        })
    });
    match res {
        Some(Ok((f, (changes, items_before)))) => {
            log::info!("{}: 46 07 search ({how:?}): item {} -> {:?}", peer.addr, f.index, f.rewards);
            peer.reply(Reply::Shop(Shop46::Search { a: f.index, items: f.records.clone() }));
            push_taken(peer, &f.taken);
            if f.gold || f.granted.money {
                crate::shop::send_money(cx, peer);
            }
            if let Some(id) = peer.account {
                if let Ok(p) = cx.lobby.store.load_profile(id) {
                    if f.granted.level {
                        peer.push(&S2c::Level(crate::build::level_8c(&cx.lobby.data, &p)));
                    }
                    if f.granted.reputation {
                        peer.push(&S2c::Factions(crate::build::factions_8e(&p)));
                    }
                    peer.push(&counts_msg(game_data::hunt::DEFAULT_CONFIG, &p).push());
                }
            }
            achievements::push_changes(cx, peer, &changes, items_before);
        }
        Some(Err(why)) => {
            log::info!("{}: 46 07 search ({how:?}) refused: {why:?}", peer.addr);
            peer.push(&why.denied().message());
            push_state(cx, peer);
        }
        None => {}
    }
}

/// `5A 03 i8 delta, u8 gold`.
pub fn level_request(cx: &mut Ctx, peer: &mut Peer, delta: u8, gold: u8) {
    let Some(c) = cfg(cx) else { return };
    let now = unix_now();
    let how = Pay::from_byte(gold);
    let res = update(cx, peer, |p| {
        let before = p.clone();
        let out = cx.lobby.roll(|r| change_level(&cx.lobby.data, c, p, delta as i8, how, now, r));
        if out.is_err() {
            let hunt = p.hunt.clone();
            *p = before;
            p.hunt = hunt;
        }
        out
    });
    match res {
        Some(Ok(taken)) => {
            log::info!("{}: 5A 03 level change ({}, {how:?})", peer.addr, delta as i8);
            push_taken(peer, &taken);
            if how == Pay::Gold {
                crate::shop::send_money(cx, peer);
            }
            push_state(cx, peer);
        }
        Some(Err(why)) => {
            log::info!("{}: 5A 03 level change ({}, {how:?}) refused: {why:?}", peer.addr, delta as i8);
            push_state(cx, peer);
        }
        None => {}
    }
}

/// A finished match of `account` (called by `stats::record_match`): maybe a free drop card.
pub fn record(cx: &Ctx, account: accounts::AccountId, f: &MatchFacts) -> Vec<S2c> {
    let Some(c) = cfg(cx) else { return vec![] };
    if !f.finished || f.map == 0 || f.local_bots {
        return vec![]; // leavers get nothing; map 0 is the tutorial; a local bots match pays no item
    }
    let now = unix_now();
    let gd = &cx.lobby.data;
    let res = cx.lobby.store.update_profile(account, |p| Ok::<_, ()>(cx.lobby.roll(|r| after_match(gd, c, p, now, r))));
    match res {
        Ok(true) => match cx.lobby.store.load_profile(account) {
            Ok(p) => {
                log::info!("account {account}: a free drop card dropped");
                vec![S2c::Inventory(crate::build::inventory_89(&cx.lobby.data, &p)), counts_msg(game_data::hunt::DEFAULT_CONFIG, &p).push()]
            }
            Err(_) => vec![],
        },
        _ => vec![],
    }
}

/// The hunt part of a finished match: bring the hunt up to date with the lobby's real dice (a player online over the
/// weekly reset gets a randomly drawn new page here, like at sign-in), then maybe a free drop card.
pub fn after_match(gd: &game_data::GameData, cfg: &HuntConfig, p: &mut Profile, now: i64, r: &mut dyn Roll) -> bool {
    if ensure(gd, cfg, p, now, r) {
        log::info!("hunt: new page after a match (period {}, level {}, page {})", p.hunt.period, p.hunt.level, p.hunt.page);
    }
    free_card_drop(cfg, p, r)
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_data::{GameData, SplitMix};

    fn gd() -> GameData {
        game_data::testing::tables()
    }

    const T0: i64 = 1_700_000_000;

    fn profile() -> Profile {
        let mut p = Profile::starter();
        p.faction = 1;
        p
    }

    #[test]
    fn periods_start_on_monday_utc() {
        // 2023-11-14 22:13:20 UTC is a Tuesday; its Monday 00:00 UTC is 2023-11-13 = 1699833600
        assert_eq!(period_start(T0), 1_699_833_600);
        assert_eq!(period_start(1_699_833_600), 1_699_833_600);
        assert_eq!(period_start(1_699_833_599), 1_699_833_600 - PERIOD_SECS);
        assert_eq!(period_start(0), -3 * 86_400);
    }

    #[test]
    fn a_new_hunt_starts_at_the_highest_level_of_the_faction() {
        game_data::require_tables!();
        let g = gd();
        let c = g.hunt.default_config().unwrap();
        let mut p = profile();
        assert!(ensure(&g, c, &mut p, T0, &mut SplitMix(1)));
        // the starter has 200 000 points = faction level 5
        assert_eq!(p.hunt.level, 5);
        assert_eq!(p.hunt.period, period_start(T0));
        assert_eq!(p.hunt.entries.len(), c.page(5, p.hunt.page).unwrap().items.len());
        assert!(p.hunt.entries.iter().all(|e| !e.found));
        assert!(!ensure(&g, c, &mut p, T0 + 5, &mut SplitMix(2)), "same period: nothing changes");
        // a low faction caps the level
        let mut low = profile();
        low.fac_points.insert(1, 100);
        ensure(&g, c, &mut low, T0, &mut SplitMix(1));
        assert_eq!(low.hunt.level, 1);
        assert_eq!(max_level(c, &low), 1);
    }

    #[test]
    fn a_search_pays_a_free_card_first_and_finds_one_item() {
        game_data::require_tables!();
        let g = gd();
        let c = g.hunt.default_config().unwrap();
        let mut p = profile();
        p.add_item(FREE_CARD, 1);
        p.add_item(PAID_CARD, 3);
        let mut r = SplitMix(9);
        ensure(&g, c, &mut p, T0, &mut r);
        let gold = p.gold;
        let f = search(&g, c, &mut p, Pay::Cards, T0, &mut r).unwrap();
        assert_eq!(card_counts(&p), (3, 0), "the free card went first");
        assert_eq!(p.hunt.free_used, 1);
        assert_eq!(p.gold, gold);
        assert!(p.hunt.entries[f.index as usize].found);
        assert_eq!(p.hunt.entries.iter().filter(|e| e.found).count(), 1);
        assert_eq!(f.rewards.len(), 1);
        assert_eq!(f.rewards[0].dict, p.hunt.entries[f.index as usize].dict);
        // the second search takes a paid card
        search(&g, c, &mut p, Pay::Cards, T0, &mut r).unwrap();
        assert_eq!(card_counts(&p), (2, 0));
        assert_eq!(p.hunt.entries.iter().filter(|e| e.found).count(), 2);
    }

    #[test]
    fn a_quality_3_weapon_comes_with_3_hidden_modifiers_of_different_types() {
        game_data::require_tables!();
        let g = gd();
        let c = g.hunt.default_config().unwrap();
        let (level, page, idx) = (0..=5u8)
            .flat_map(|l| (0..c.page_count(l) as u8).map(move |pg| (l, pg)))
            .find_map(|(l, pg)| c.page(l, pg).unwrap().items.iter().position(|it| it.item_quality == 3).map(|i| (l, pg, i)))
            .expect("a page entry with item_quality 3");
        let mut p = profile();
        let mut r = SplitMix(5);
        ensure(&g, c, &mut p, T0, &mut r);
        p.hunt.level = level;
        p.hunt.page = page;
        // only that entry is still hidden, so the search must find it
        p.hunt.entries =
            c.resolve_page(level, page, &mut r).into_iter().enumerate().map(|(i, dict)| HuntEntry { dict, found: i != idx }).collect();
        let f = search(&g, c, &mut p, Pay::Gold, T0, &mut r).unwrap();
        assert_eq!(f.index as usize, idx);
        let rec = f.records.iter().find(|x| x.uid != 0).expect("the weapon record");
        assert_eq!(rec.mods.len(), 3, "{rec:?}");
        assert_eq!(p.mods.get(&rec.uid), Some(&rec.mods), "stored with the item");
        let allowed = g.craft.candidates(rec.dict, &[], None);
        let types: std::collections::BTreeSet<u32> = rec
            .mods
            .iter()
            .map(|m| *allowed.iter().find(|(_, gs)| gs.contains(&(m.0 as u32))).expect("an allowed group").0)
            .collect();
        assert_eq!(types.len(), 3, "one modifier per slot type");
    }

    /// A module entry of the page: `AA` shows its pairs (blue rarity + drop grades + override), the search gives the
    /// module exactly those pairs; the same page shows the same pairs again; non-module entries carry none.
    #[test]
    fn a_hunt_module_shows_and_gets_the_same_blue_pairs() {
        game_data::require_tables!();
        let g = gd();
        let c = g.hunt.default_config().unwrap();
        let (level, page, idx) = (0..=5u8)
            .flat_map(|l| (0..c.page_count(l) as u8).map(move |pg| (l, pg)))
            .find_map(|(l, pg)| c.page(l, pg).unwrap().items.iter().position(|it| it.ids.iter().all(|d| g.items.is_module(*d))).map(|i| (l, pg, i)))
            .expect("a page entry of modules");
        let mut p = profile();
        let mut r = SplitMix(5);
        ensure(&g, c, &mut p, T0, &mut r);
        p.hunt.level = level;
        p.hunt.page = page;
        p.hunt.entries =
            c.resolve_page(level, page, &mut r).into_iter().enumerate().map(|(i, dict)| HuntEntry { dict, found: i != idx }).collect();
        let shown = match state(&g, c, "weekly_drop", &p, T0) {
            WeeklyDrop::State { page: Some(pg), .. } => pg.items,
            other => panic!("{other:?}"),
        };
        let m = shown[idx].mods.clone();
        assert_eq!(game_data::merge::rarity(&m), 2, "{m:?}");
        assert!(m.len() >= 2 && m.len() <= HUNT_ITEM_MODS, "{m:?}");
        assert!(shown.iter().enumerate().all(|(i, it)| i == idx || it.mods.is_empty() || g.items.is_module(it.dict)));
        assert_eq!(entry_mods(&g, &p.hunt, idx), m, "stable for the page");
        let f = search(&g, c, &mut p, Pay::Gold, T0, &mut r).unwrap();
        assert_eq!(f.index as usize, idx);
        let rec = f.records.iter().find(|x| x.uid != 0).expect("the module record");
        assert_eq!(rec.mods, m);
        assert_eq!(p.mods.get(&rec.uid), Some(&m));
    }

    #[test]
    fn no_cards_means_a_refusal_and_gold_pays_the_price() {
        game_data::require_tables!();
        let g = gd();
        let c = g.hunt.default_config().unwrap();
        let mut p = profile();
        let mut r = SplitMix(3);
        ensure(&g, c, &mut p, T0, &mut r);
        let silver = p.silver;
        assert_eq!(search(&g, c, &mut p, Pay::Cards, T0, &mut r).unwrap_err(), Refused::NotEnoughCards);
        assert_eq!(p.hunt.entries.iter().filter(|e| e.found).count(), 0, "a refused search finds nothing");
        let gold = p.gold;
        let f = search(&g, c, &mut p, Pay::Gold, T0, &mut r).unwrap();
        assert_eq!(p.gold, gold - c.premium_money_to_open + if f.rewards[0].dict == 215 { f.rewards[0].amount } else { 0 });
        assert!(f.gold);
        let _ = silver;
        let mut poor = profile();
        poor.gold = 99;
        ensure(&g, c, &mut poor, T0, &mut r);
        assert_eq!(search(&g, c, &mut poor, Pay::Gold, T0, &mut r).unwrap_err(), Refused::NotEnoughMoney);
        assert_eq!(poor.gold, 99);
    }

    #[test]
    fn every_item_can_be_found_once_then_the_list_is_exhausted() {
        game_data::require_tables!();
        let g = gd();
        let c = g.hunt.default_config().unwrap();
        let mut p = profile();
        p.add_item(PAID_CARD, 100);
        let mut r = SplitMix(5);
        ensure(&g, c, &mut p, T0, &mut r);
        let n = p.hunt.entries.len();
        let mut seen = std::collections::BTreeSet::new();
        for _ in 0..n {
            let f = search(&g, c, &mut p, Pay::Cards, T0, &mut r).unwrap();
            assert!(seen.insert(f.index), "an item is found once");
        }
        assert_eq!(seen.len(), n);
        assert_eq!(card_counts(&p).0, 100 - n as u32);
        assert_eq!(search(&g, c, &mut p, Pay::Cards, T0, &mut r).unwrap_err(), Refused::AllOpened);
        assert_eq!(card_counts(&p).0, 100 - n as u32, "an exhausted list costs nothing");
    }

    #[test]
    fn levels_change_inside_the_limits_and_cost_one_card() {
        game_data::require_tables!();
        let g = gd();
        let c = g.hunt.default_config().unwrap();
        let mut p = profile();
        p.add_item(PAID_CARD, 5);
        let mut r = SplitMix(7);
        ensure(&g, c, &mut p, T0, &mut r);
        assert_eq!(p.hunt.level, 5);
        assert_eq!(change_level(&g, c, &mut p, 1, Pay::Cards, T0, &mut r).unwrap_err(), Refused::Failed, "already at the top");
        assert_eq!(card_counts(&p).0, 5);
        change_level(&g, c, &mut p, -1, Pay::Cards, T0, &mut r).unwrap();
        assert_eq!((p.hunt.level, card_counts(&p).0), (4, 4));
        let page = p.hunt.page;
        change_level(&g, c, &mut p, 0, Pay::Cards, T0, &mut r).unwrap();
        assert_eq!(p.hunt.level, 4);
        assert_eq!(card_counts(&p).0, 3);
        assert!((page as usize) < c.page_count(4) && (p.hunt.page as usize) < c.page_count(4));
        assert!(p.hunt.entries.iter().all(|e| !e.found), "a new page has nothing found");
        // gold pays when there are no cards
        let mut q = profile();
        ensure(&g, c, &mut q, T0, &mut r);
        let gold = q.gold;
        change_level(&g, c, &mut q, -1, Pay::Gold, T0, &mut r).unwrap();
        assert_eq!(q.gold, gold - c.premium_money_to_open);
        assert_eq!(change_level(&g, c, &mut profile(), 0, Pay::Cards, T0, &mut r).unwrap_err(), Refused::NotEnoughCards);
        // with the level change off, Refresh (delta 0) still works and +-1 is refused
        let off = HuntConfig { can_change_drop_level: false, ..c.clone() };
        let mut s = profile();
        s.add_item(PAID_CARD, 2);
        assert_eq!(change_level(&g, &off, &mut s, -1, Pay::Cards, T0, &mut r).unwrap_err(), Refused::Failed);
        let level = s.hunt.level;
        change_level(&g, &off, &mut s, 0, Pay::Cards, T0, &mut r).unwrap();
        assert_eq!(s.hunt.level, level, "Refresh keeps the level");
    }

    #[test]
    fn a_new_week_resets_the_period_and_keeps_the_free_cards() {
        game_data::require_tables!();
        let g = gd();
        let c = g.hunt.default_config().unwrap();
        let mut p = profile();
        p.add_item(FREE_CARD, 2);
        let mut r = SplitMix(11);
        ensure(&g, c, &mut p, T0, &mut r);
        search(&g, c, &mut p, Pay::Cards, T0, &mut r).unwrap();
        assert_eq!((p.hunt.free_used, card_counts(&p).1), (1, 1));
        assert!(ensure(&g, c, &mut p, T0 + PERIOD_SECS, &mut r));
        assert_eq!(p.hunt.period, period_start(T0 + PERIOD_SECS));
        assert_eq!(p.hunt.free_used, 0);
        assert_eq!(card_counts(&p).1, 1, "the held free card stays");
        assert!(p.hunt.entries.iter().all(|e| !e.found));
    }

    /// A player online over the weekly reset whose first finished match comes before he opens the hunt: the new page is
    /// drawn with the real dice (different seeds give different pages / ids), not always page 0 and the first ids.
    #[test]
    fn a_match_after_the_weekly_reset_draws_the_new_page_with_real_dice() {
        game_data::require_tables!();
        let g = gd();
        let c = g.hunt.default_config().unwrap();
        let mut seen = std::collections::BTreeSet::new();
        for seed in 0..20 {
            let mut p = profile();
            ensure(&g, c, &mut p, T0, &mut SplitMix(1));
            after_match(&g, c, &mut p, T0 + PERIOD_SECS, &mut SplitMix(100 + seed));
            assert_eq!(p.hunt.period, period_start(T0 + PERIOD_SECS));
            assert!(p.hunt.entries.iter().all(|e| !e.found));
            seen.insert((p.hunt.page, p.hunt.entries.iter().map(|e| e.dict).collect::<Vec<_>>()));
        }
        assert!(seen.len() > 1, "the new week's page depends on the roll: {seen:?}");
    }

    /// A pool entry never offers a weapon or armour piece the player owns while the pool has one he does not own; when he
    /// owns them all, the plain draw stays.
    #[test]
    fn owned_weapons_and_armour_are_not_drawn_while_the_pool_has_others() {
        game_data::require_tables!();
        let g = gd();
        let c = g.hunt.default_config().unwrap();
        let gear = |d: u16| g.items.is_weapon(d) || g.items.is_armour(d);
        let starter = owned_gear(&g, &profile());
        let (level, page, idx, free) = (0..=5u8)
            .flat_map(|l| (0..c.page_count(l) as u8).map(move |pg| (l, pg)))
            .find_map(|(l, pg)| {
                c.page(l, pg).unwrap().items.iter().enumerate().find_map(|(i, it)| {
                    let free = it.ids.iter().copied().find(|d| !starter.contains(d))?;
                    (it.ids.len() >= 2 && it.ids.iter().all(|d| gear(*d))).then_some((l, pg, i, free))
                })
            })
            .expect("a pool of weapons or armour");
        let ids = c.page(level, page).unwrap().items[idx].ids.clone();
        // a config whose level has only that page
        let mut one = c.clone();
        one.levels[level as usize] = vec![c.page(level, page).unwrap().clone()];
        let mut p = profile();
        for d in ids.iter().filter(|d| **d != free) {
            p.add_item(*d, 100);
        }
        for seed in 0..20 {
            p.hunt.level = level;
            assert!(new_page(&g, &one, &mut p, &mut SplitMix(seed)));
            assert_eq!(p.hunt.entries[idx].dict, free, "seed {seed}: the only unowned id of the pool");
        }
        // every id owned: the plain draw
        p.add_item(free, 100);
        p.hunt.level = level;
        new_page(&g, &one, &mut p, &mut SplitMix(3));
        assert!(ids.contains(&p.hunt.entries[idx].dict));
    }

    #[test]
    fn free_cards_drop_with_the_limits_of_the_period() {
        game_data::require_tables!();
        let g = gd();
        let c = g.hunt.default_config().unwrap();
        let mut p = profile();
        ensure(&g, c, &mut p, T0, &mut SplitMix(1));
        // a roll below the chance gives a card, above it none
        struct Fixed(f64);
        impl Roll for Fixed {
            fn below(&mut self, _n: u32) -> u32 {
                0
            }
            fn unit(&mut self) -> f64 {
                self.0
            }
        }
        assert!(!free_card_drop(c, &mut p, &mut Fixed(0.5)));
        assert!(free_card_drop(c, &mut p, &mut Fixed(0.1)));
        assert!(free_card_drop(c, &mut p, &mut Fixed(0.1)));
        assert_eq!(card_counts(&p).1, 2);
        assert!(!free_card_drop(c, &mut p, &mut Fixed(0.0)), "two free cards held: no third");
        // both spent in this period: held 0 + used 2 = the period limit
        p.hunt.free_used = 2;
        let uid = stack(&p, FREE_CARD).unwrap().0;
        p.consume(uid, 2);
        assert!(!free_card_drop(c, &mut p, &mut Fixed(0.0)), "the period limit is reached");
    }

    #[test]
    fn the_state_message_lists_the_page_and_the_counts() {
        game_data::require_tables!();
        let g = gd();
        let c = g.hunt.default_config().unwrap();
        let mut p = profile();
        p.add_item(PAID_CARD, 4);
        p.add_item(FREE_CARD, 1);
        ensure(&g, c, &mut p, T0, &mut SplitMix(4));
        match state(&g, c, "weekly_drop", &p, T0) {
            WeeklyDrop::State { a, counts, page: Some(pg), .. } => {
                assert!(a != 0);
                assert_eq!((counts.b, counts.c, counts.d), (4, 1, 0));
                assert_eq!((pg.level, pg.max_level, pg.min_level), (5, 5, 1));
                assert_eq!(pg.items.len(), p.hunt.entries.len());
                assert!(pg.items.iter().all(|i| i.not_found));
                assert_eq!(pg.time_left as i64, p.hunt.period + PERIOD_SECS - T0);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn every_dictionary_id_of_the_data_exists() {
        game_data::require_tables!();
        let g = gd();
        for name in ["weekly_drop", "weekly_drop_new_year_2018"] {
            let c = g.hunt.config(name).unwrap();
            for level in &c.levels {
                for page in level {
                    let sum: f64 = page.items.iter().map(|i| i.prob).sum();
                    assert!(sum > 0.0 && sum <= 1.0001, "{name}: page probabilities {sum}");
                    for it in &page.items {
                        for id in &it.ids {
                            assert!(g.items.get(*id).is_some() || matches!(id, 214 | 215 | 925), "{name}: dict {id} unknown");
                        }
                    }
                }
                if !level.is_empty() {
                    let total: f64 = level.iter().map(|p| p.roulette_prob).sum();
                    assert!((total - 1.0).abs() < 0.01, "{name}: roulette_prob sums to {total}");
                }
            }
        }
    }
}
