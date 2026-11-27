//! Opening chests: `46 06 u32 chest item` -> `83 46 06 u32 chest, u16 chest dict, u8 n, n x item record`
//! (docs/notes/items.md section 3; the rules of the roll are in `game_data::chests`).
//!
//! PROVEN client rules: it removes one unit of the chest itself (0x2001930D8), so the server takes the unit from the
//! stored stack and sends no `90`; it adds the reward records (same uid = added to that stack, new uid = new item,
//! 0x200193400) except the money ids 214, 215, 925, 951, 1138, 1139, 1356 (0x200192EE8, 0x20019335A), which the server
//! credits to the wallet and sends with uid 0 for the reward window only, then `8B`. Ids 1164 (reputation) and 1165
//! (experience) would become inventory rows: credited to the profile instead and not sent (CHOICE), then `8C` / `8E`.
//! A refusal answers `84 46` ([`refuse`]).

use accounts::{Currency, ItemRow, Profile};
use game_data::chests::{OpenError, Reward};
use game_data::craft::Roll;
use game_data::GameData;
use protocol::items::ItemRecord;
use protocol::s2c::*;

use crate::build;
use crate::dispatch::{update, Ctx};
use crate::peer::Peer;
use crate::shop::send_money;

pub const SILVER: u16 = 214;
pub const GOLD: u16 = 215;
pub const SPARE_PARTS: u16 = 925;
pub const REPUTATION: u16 = 1164;
pub const EXPERIENCE: u16 = 1165;

/// What a roll changed besides the inventory.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Granted {
    pub money: bool,
    pub level: bool,
    pub reputation: bool,
}

/// Put the rewards into the profile; returns the records of the 46 06 answer. A chest opening rolls its modules with
/// [`roll_module_mods`] and [`grant_with_mods`]; any other reward list that names a module (none in 0.69d data) gives it a
/// green roll ([`give_rarity`]). A chest in the list is opened first ([`open_chests`]): the player gets its contents,
/// never the chest, because the 0.69d lobby has no place to open a stored chest (see the module comment of `play.rs`).
pub fn grant(gd: &GameData, p: &mut Profile, rewards: &[Reward]) -> (Vec<ItemRecord>, Granted) {
    let opened = open_chests(gd, rewards, &mut crate::fresh_roll());
    grant_with_mods(gd, p, &opened.rewards, opened.module_mods)
}

/// A reward list with its chests opened (see [`open_chests`]).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct OpenedRewards {
    /// the list in order, every openable chest unit replaced by its rolled contents
    pub rewards: Vec<Reward>,
    /// the modifier lists of the modules that came out of the chests, in the order [`grant_with_mods`] creates them
    pub module_mods: Vec<Vec<(u16, u16)>>,
    /// (chest dict, contents) of every opened unit, for the log
    pub chests: Vec<(u16, Vec<Reward>)>,
}

/// How deep chests inside chests are opened (none in 0.69d data; the limit only stops a table that names itself).
const CHEST_DEPTH: u32 = 4;

/// Replace every chest unit of `rewards` by the roll of its table (`game_data::chests`, the same roll as `46 06` and the
/// match drops), with the modules' modifier lists from [`roll_module_mods`]. A chest the data cannot open (no table)
/// stays in the list as an item (logged). Each opened chest leaves an info log line.
pub fn open_chests(gd: &GameData, rewards: &[Reward], r: &mut dyn Roll) -> OpenedRewards {
    let mut out = OpenedRewards::default();
    open_into(gd, rewards, r, CHEST_DEPTH, &mut out);
    for (chest, contents) in &out.chests {
        log::info!("granted chest dict {chest} opened: {contents:?}");
    }
    out
}

fn open_into(gd: &GameData, rewards: &[Reward], r: &mut dyn Roll, depth: u32, out: &mut OpenedRewards) {
    for rw in rewards {
        if !gd.chests.is_chest(rw.dict) || depth == 0 {
            push_plain(gd, *rw, &[], out);
            continue;
        }
        for unit in 0..rw.amount {
            match gd.chests.open(rw.dict, r) {
                Ok(contents) => {
                    let mods = roll_module_mods(gd, rw.dict, &contents, r);
                    out.chests.push((rw.dict, contents.clone()));
                    let mut mods = mods.into_iter();
                    for c in &contents {
                        if gd.chests.is_chest(c.dict) {
                            open_into(gd, std::slice::from_ref(c), r, depth - 1, out);
                        } else {
                            let n = module_units(gd, c);
                            let m: Vec<_> = mods.by_ref().take(n).collect();
                            push_plain(gd, *c, &m, out);
                        }
                    }
                }
                Err(e) => {
                    log::warn!("granted chest dict {} cannot be opened ({e}): kept as an item", rw.dict);
                    push_plain(gd, Reward { dict: rw.dict, amount: rw.amount - unit }, &[], out);
                    break;
                }
            }
        }
    }
}

/// How many records [`grant_with_mods`] returns for one reward: one for money, event currency and a stack, none for
/// reputation and experience, one per unit for other items.
pub fn record_count(gd: &GameData, r: &Reward) -> usize {
    match r.dict {
        SILVER | GOLD | SPARE_PARTS => 1,
        d if game_data::events::slot_of(d).is_some() => 1,
        REPUTATION | EXPERIENCE => 0,
        d if gd.items.is_stackable(d) => 1,
        _ => r.amount as usize,
    }
}

/// Module units a reward creates (one modifier list each in [`grant_with_mods`]).
fn module_units(gd: &GameData, rw: &Reward) -> usize {
    if gd.items.is_module(rw.dict) && !gd.items.is_stackable(rw.dict) {
        rw.amount as usize
    } else {
        0
    }
}

/// Append a reward that is not opened, with the modifier lists of its module units (empty = a green roll at grant time).
fn push_plain(gd: &GameData, rw: Reward, mods: &[Vec<(u16, u16)>], out: &mut OpenedRewards) {
    let n = module_units(gd, &rw);
    for i in 0..n {
        out.module_mods.push(mods.get(i).cloned().unwrap_or_default());
    }
    out.rewards.push(rw);
}

/// A new module unit `uid` that got no rolled modifier list gets a green one: `(312, 1)` plus drop grades from the green
/// cells of the general table (`ModuleDrops::green`; LIKELY: the 0.69d client has no state for an owned module
/// without a rarity pair, green "Factory-made" is the default quality). Every path that creates a module without its own
/// table (the new-player kit, the module seed, reward lists, quest and achievement items, match drops, a shop buy) calls
/// this, so no module is created grey. Anything else (not a module, a stack, a module that has a list) is left alone.
pub fn give_rarity(gd: &GameData, p: &mut Profile, uid: u32, r: &mut dyn Roll) {
    let Some(dict) = p.item(uid).map(|i| i.dict_id) else { return };
    if !gd.items.is_module(dict) || gd.items.is_stackable(dict) || p.mods.get(&uid).is_some_and(|m| !m.is_empty()) {
        return;
    }
    p.mods.insert(uid, gd.module_drops.green(dict, r));
}

/// The stored modifier list of every module unit in `rewards` (in the order [`grant_with_mods`] creates them), rolled
/// from the weight table of chest `chest` (`game_data::module_drops::chest_source`: the rare-module chests blue, the
/// others the general table; docs/notes/workshop.md section 7). Rolled before the transaction, like the chest itself.
pub fn roll_module_mods(gd: &GameData, chest: u16, rewards: &[Reward], r: &mut dyn Roll) -> Vec<Vec<(u16, u16)>> {
    let source = game_data::module_drops::chest_source(gd.chests.name(chest).unwrap_or(""));
    let mut out = vec![];
    for rw in rewards.iter().filter(|rw| gd.items.is_module(rw.dict) && !gd.items.is_stackable(rw.dict)) {
        for _ in 0..rw.amount {
            out.push(gd.module_drops.roll(rw.dict, source, r).unwrap_or_default());
        }
    }
    out
}

/// [`grant`], with the stored modifier lists of the new modules (one per module unit, in reward order; see
/// [`roll_module_mods`]). The records carry them (rarity pair 312 = the colour, the drop pairs = the stat lines).
pub fn grant_with_mods(gd: &GameData, p: &mut Profile, rewards: &[Reward], module_mods: Vec<Vec<(u16, u16)>>) -> (Vec<ItemRecord>, Granted) {
    let mut module_mods = module_mods.into_iter();
    let mut recs = Vec::new();
    let mut g = Granted::default();
    for r in rewards {
        match r.dict {
            SILVER | GOLD | SPARE_PARTS => {
                let cur = match r.dict {
                    SILVER => Currency::Silver,
                    GOLD => Currency::Gold,
                    _ => Currency::Parts,
                };
                p.credit(cur, r.amount as u64);
                g.money = true;
                recs.push(ItemRecord { uid: 0, dict: r.dict, value: r.amount, ..Default::default() });
            }
            d if game_data::events::slot_of(d).is_some() => {
                // event currency: a wallet value like the money (the client skips these ids when it adds records)
                let slot = game_data::events::slot_of(d).unwrap_or(0);
                p.credit(crate::events::currency(slot), r.amount as u64);
                g.money = true;
                recs.push(ItemRecord { uid: 0, dict: r.dict, value: r.amount, ..Default::default() });
            }
            REPUTATION => {
                let f = p.faction;
                p.add_points(f, r.amount);
                g.reputation = true;
            }
            EXPERIENCE => {
                crate::progress::add_xp(gd, p, r.amount);
                g.level = true;
            }
            dict => {
                if gd.items.is_stackable(dict) {
                    let uid = match p.items.iter().find(|i| i.dict_id == dict).map(|i| i.uid) {
                        Some(uid) => {
                            p.item_mut(uid).unwrap().count += r.amount;
                            uid
                        }
                        None => p.add_item(dict, r.amount),
                    };
                    let row: ItemRow = *p.item(uid).unwrap();
                    recs.push(build::record_value(gd, p, &row, r.amount));
                } else {
                    for _ in 0..r.amount {
                        let uid = p.add_item(dict, gd.items.amount_field(dict, 100));
                        if gd.items.is_module(dict) {
                            match module_mods.next().filter(|m| !m.is_empty()) {
                                Some(m) => {
                                    p.mods.insert(uid, m);
                                }
                                None => give_rarity(gd, p, uid, &mut crate::fresh_roll()),
                            }
                        }
                        let row: ItemRow = *p.item(uid).unwrap();
                        recs.push(build::record(gd, p, &row));
                    }
                }
            }
        }
    }
    (recs, g)
}

/// Text of a refused `46 06` (localisation `st_generating_chest_drop_error`: "Error generating items").
pub const REFUSED_TEXT: &str = "st_generating_chest_drop_error";

/// A refused `46 06`: logged, then `84 46` with a text. The client locks UI element 2 before it sends `46 06`
/// (builder 0x20024B110) and only an answer unlocks it: `83 46 06`, or the `84` display code of request 0x46, which
/// shows the text and unlocks elements 1 / 2 / 4 (0x200176D62..0x200176D86, PROVEN). Without an answer the chest window
/// stays locked.
fn refuse(peer: &mut Peer, item: u32, why: &str) {
    log::info!("{}: 46 06 chest {item}: {why}", peer.addr);
    peer.push(&S2c::Denied(Denied { request: 0x46, body: DeniedBody::Text(REFUSED_TEXT.into()) }));
}

/// `46 06`. A chest the account does not own, an item that is no chest, or a chest the data cannot open changes
/// nothing (the stack stays) and gets `84 46` ([`refuse`]).
pub fn open(cx: &mut Ctx, peer: &mut Peer, item: u32) {
    let Some(id) = peer.account else { return };
    let gd = &cx.lobby.data;
    let row = match cx.lobby.store.load_profile(id) {
        Ok(p) => p.item(item).copied(),
        Err(e) => {
            log::error!("{}: loading profile of {id}: {e}", peer.addr);
            return refuse(peer, item, "profile not loaded");
        }
    };
    let Some(row) = row.filter(|r| r.count > 0) else {
        return refuse(peer, item, "not owned");
    };
    let rewards = match cx.lobby.roll(|r| gd.chests.open(row.dict_id, r)) {
        Ok(v) => v,
        Err(OpenError::NotAChest) => {
            return refuse(peer, item, &format!("dict {} is not a chest", row.dict_id));
        }
        Err(e) => {
            return refuse(peer, item, &format!("dict {}: cannot be opened: {e}", row.dict_id));
        }
    };
    let module_mods = cx.lobby.roll(|r| roll_module_mods(gd, row.dict_id, &rewards, r));
    let done = update(cx, peer, |p| {
        // the client takes the unit itself when it reads the answer: no 90
        p.consume(item, 1)?;
        Some(grant_with_mods(gd, p, &rewards, module_mods.clone()))
    });
    let Some(Some((records, granted))) = done else {
        return refuse(peer, item, "gone before the roll was applied");
    };
    log::info!("{}: 46 06 opened chest {item} (dict {}): {:?}", peer.addr, row.dict_id, rewards);
    peer.reply(Reply::Shop(Shop46::Chest { item, chest_dict: row.dict_id, rewards: records }));
    if granted.money {
        send_money(cx, peer);
    }
    if let Ok(p) = cx.lobby.store.load_profile(id) {
        if granted.level {
            peer.push(&S2c::Level(build::level_8c(gd, &p)));
        }
        if granted.reputation {
            peer.push(&S2c::Factions(build::factions_8e(&p)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A reward list that names a module (none in 0.69d data, e.g. a store bundle or a quest) gives a green module, one
    /// list per unit; a chest's own rolled list is kept as it is.
    #[test]
    fn a_granted_module_is_green() {
        game_data::require_tables!();
        let gd = game_data::testing::tables();
        let mut p = Profile::starter();
        let (recs, _) = grant(&gd, &mut p, &[Reward { dict: 2002, amount: 2 }]);
        assert_eq!(recs.len(), 2);
        for rec in &recs {
            let m = &p.mods[&rec.uid];
            assert_eq!(m[0], (312, 1), "{m:?}");
            assert_eq!((m[1].0, m[2].0), (325, 411));
            assert_eq!(gd.module_drops.cell_value(m[1].1 as usize, m[2].1 as usize), 0, "green cell: {m:?}");
        }
        let rolled = vec![(312, 3), (325, 6), (411, 6)];
        let (recs, _) = grant_with_mods(&gd, &mut p, &[Reward { dict: 2002, amount: 1 }], vec![rolled.clone()]);
        assert_eq!(p.mods[&recs[0].uid], rolled);
        // not a module: nothing stored
        let (recs, _) = grant(&gd, &mut p, &[Reward { dict: 107, amount: 1 }]);
        assert!(!p.mods.contains_key(&recs[0].uid));
    }

    /// A chest given by a reward list (faction battle tier, store offer, calendar day) is opened: the profile gets the
    /// contents (blue modules from a rare-module chest), never the chest stack.
    #[test]
    fn a_granted_chest_is_opened() {
        game_data::require_tables!();
        let gd = game_data::testing::tables();
        let chest = (2033..=2036).find(|d| gd.chests.can_open(*d)).expect("a rare_modules_chest opens");
        let mut p = Profile::starter();
        let before = p.items.len();
        let (recs, _) = grant(&gd, &mut p, &[Reward { dict: chest, amount: 2 }]);
        assert!(p.items.iter().all(|i| i.dict_id != chest), "no chest stack is stored");
        assert!(p.items.len() > before, "the contents were added");
        assert!(!recs.is_empty() && recs.iter().all(|r| r.dict != chest), "{recs:?}");
        // the modules of a rare-module chest carry the chest's own (blue) roll, not the green default
        for rec in recs.iter().filter(|r| gd.items.is_module(r.dict) && !gd.items.is_stackable(r.dict)) {
            assert_eq!(p.mods[&rec.uid][0], (312, 2), "{:?}", p.mods[&rec.uid]);
        }
        // the expanded list: two units opened, no chest left
        let o = open_chests(&gd, &[Reward { dict: chest, amount: 2 }, Reward { dict: 107, amount: 1 }], &mut crate::fresh_roll());
        assert_eq!(o.chests.len(), 2);
        assert!(o.rewards.iter().all(|r| !gd.chests.is_chest(r.dict)));
        assert_eq!(o.rewards.last(), Some(&Reward { dict: 107, amount: 1 }));
        let units: usize = o.rewards.iter().map(|r| module_units(&gd, r)).sum();
        assert_eq!(o.module_mods.len(), units);
    }
}
