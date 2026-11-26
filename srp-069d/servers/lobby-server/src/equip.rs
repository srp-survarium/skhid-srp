//! Equip 45 00 (`apply_profile_items`): moves between the inventory (slot 100) and the profile slots,
//! the ammo slider charge, and pocket buys from source 200.

use accounts::{Currency, Profile};
use game_data::pockets::QUICK_SLOTS;
use game_data::GameData;
use protocol::c2s::EquipEntry;
use protocol::s2c::*;

use crate::build;
use crate::dispatch::{update, Ctx};
use crate::peer::Peer;
use crate::shop::{currency, send_money};

/// Ammo slots of the profile table (primary / secondary weapon, two more cells).
pub const AMMO_SLOTS: [u8; 4] = [8, 9, 11, 12];

/// What an equip request changed.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct EquipOutcome {
    /// Uids of items that are new or whose count changed (the client gets them as `83 45 0F` records first).
    pub changed: Vec<u32>,
    /// Money was charged (ammo slider, pocket stack).
    pub spent: bool,
}

/// Apply the entries to the profile; refusals skip the entry, the others go on.
/// `opened` = the account's `specifics_opened` (pocket consumables opened with `46 09`). A pocket consumable that is
/// locked for the player ([`crate::actions::pocket_open`]) goes into a pocket slot 13..=17 only with units the player
/// owns, and never from the popup (source 200). The client's rule (PROVEN, lobby movie): only the popup tile tests
/// `opened` (a locked tile opens the gold dialog, never a placement); moves between pockets, the count stepper, drops
/// on a paper-doll slot and the preset load do not.
///
/// So a locked item is refused from source 200, and elsewhere it is placed when the owned stack covers every pocket
/// that holds it; units it would have to buy are refused (CHOICE: the only buy UI, the popup, refuses a locked item).
/// The refusal is the silent one of every invalid move: the entry is skipped before anything changes (its source slot
/// keeps the item) and the 87 that follows shows the real slots.
pub fn apply(gd: &GameData, p: &mut Profile, entries: &[EquipEntry], opened: &[u16]) -> EquipOutcome {
    let before: std::collections::BTreeMap<u32, u32> = p.items.iter().map(|i| (i.uid, i.count)).collect();
    let saved = p.clone();
    let mut spent = false;
    for e in entries {
        let (iid, did, src, dst, cnt) = (e.item, e.dict, e.from, e.to, e.count);
        // the item placed always has the entry's dict (found by uid with a matching dict, or by dict)
        if QUICK_SLOTS.contains(&dst) && gd.items.is_quick_item(did) && !crate::actions::pocket_open(gd, p, opened, did) {
            if let Err(why) = locked_owned_units(p, iid, did, src, dst, cnt) {
                log::info!("45 00: dict {did} is locked for this player (faction level not reached, not opened with 46 09): {why}: not placed in pocket {dst}");
                continue;
            }
        }
        if src <= 22 && p.slots.contains_key(&src) && (dst == 100 || dst <= 22) {
            let held = p.slots[&src];
            if dst == 100 || held == iid || p.item(iid).is_none() {
                p.slots.remove(&src);
                p.slot_rounds.remove(&src);
            }
        }
        if dst > 22 {
            continue;
        }
        let known = gd.items.contains(did);
        // The client names the item by uid and dict; ammo and pocket buys name it by its dict id (uid == dict, PROVEN),
        // which may equal the uid of another item, so a uid only counts when its dict matches. Source 200 (bought
        // straight from the shop) is always found by dict.
        let by_uid = if src == 200 { None } else { p.item(iid).filter(|i| i.dict_id == did).map(|i| i.uid) };
        let mut uid = by_uid.or_else(|| p.items.iter().find(|i| i.dict_id == did).map(|i| i.uid));
        if uid.is_none() && known && gd.items.is_ammo(did) && iid == did as u32 {
            // the loadout names standard ammo by its dict id without owning a stack: supplied, create the stack
            uid = Some(p.add_item(did, 0));
        }
        if uid.is_none() && known && gd.items.is_quick_item(did) && iid == did as u32 && QUICK_SLOTS.contains(&dst) {
            // the pocket popup buys straight into a pocket {dict, dict, from 200, to 13+i, count, 1}
            uid = Some(p.add_item(did, 0));
        }
        let Some(uid) = uid else {
            log::info!("45 00: item {iid} / dict {did} not owned");
            continue;
        };
        let quick = QUICK_SLOTS.contains(&dst);
        let item_dict = p.item(uid).unwrap().dict_id;
        // premium ammo boxes: slot 11 (primary weapon) / 12 (pistol) take only categories 37 / 38
        // (`profile_slots_restrictions`; the match's inventory cook pairs slot 7 with 11 and 10 with 12, 0x2002D9B8F)
        if matches!(dst, 11 | 12) && !gd.slot_allows(dst, item_dict) {
            log::info!("45 00: dict {item_dict} not allowed in ammo box slot {dst}");
            continue;
        }
        // ... and only the box category of the weapon in the paired slot (7 -> 11, 10 -> 12): the lobby movie's
        // `getItemAmmoBoostersCategory` (pistol category 17 -> 38, other weapons 37, no weapon -> nothing); its
        // selector offers only that category and `updateBoosterSlots` moves any other box back to the inventory
        if matches!(dst, 11 | 12) {
            let weapon = p.slots.get(&if dst == 11 { 7 } else { 10 }).and_then(|w| p.item(*w)).map(|w| w.dict_id);
            let want = weapon.and_then(|w| gd.items.ammo_booster_category(w));
            if want != Some(gd.items.category(item_dict)) {
                log::info!("45 00: box dict {item_dict} does not fit slot {dst} (weapon {weapon:?} takes box category {want:?})");
                continue;
            }
        }
        if quick && !gd.items.is_quick_item(item_dict) {
            log::info!("45 00: dict {item_dict} not allowed in quick slot {dst}");
            continue;
        }
        // one item in one slot (non-stack); ammo and pocket cells may repeat
        let others: Vec<u8> = p
            .slots
            .iter()
            .filter(|(k, v)| **v == uid && **k != dst && !AMMO_SLOTS.contains(k) && !QUICK_SLOTS.contains(k))
            .map(|(k, _)| *k)
            .collect();
        for k in others {
            p.slots.remove(&k);
        }
        let count_now = p.item(uid).unwrap().count;
        if gd.items.is_ammo(item_dict) && AMMO_SLOTS.contains(&dst) && cnt > 0 {
            // rounds of this stack loaded in the other ammo cells + this one against the stack
            let other: u32 = AMMO_SLOTS
                .iter()
                .filter(|k| **k != dst && p.slots.get(k) == Some(&uid))
                .map(|k| p.slot_rounds.get(k).copied().unwrap_or(0))
                .sum();
            let short = (other + cnt).saturating_sub(count_now);
            if short > 0 {
                let (cost, added) = gd.shop.ammo_top_up(item_dict, short);
                if p.spend(Currency::Silver, cost).is_err() {
                    log::info!("45 00: {short} rounds of dict {item_dict} cost {cost}, silver {}: not loaded", p.silver);
                    if count_now == 0 {
                        p.remove_item(uid);
                    }
                    continue;
                }
                spent |= cost > 0;
                p.item_mut(uid).unwrap().count += added;
                log::info!("45 00: supplied {added} rounds of dict {item_dict} for {cost} silver");
            }
        }
        let mut cnt = cnt;
        if quick {
            let other: u32 = QUICK_SLOTS
                .filter(|k| *k != dst && p.slots.get(k) == Some(&uid))
                .map(|k| p.slot_rounds.get(&k).copied().unwrap_or(1).max(1))
                .sum();
            let want = cnt.max(1);
            let count_now = p.item(uid).unwrap().count;
            let short = (other + want).saturating_sub(count_now);
            if short > 0 && !gd.shop.listed(item_dict) {
                // an unlisted item (no price; the popup builds no tile for it) is not sold, as the 46 00 buy refuses it:
                // only units the player owns go into the pocket (CHOICE; its pocket cost is 0, so a buy would be free)
                log::info!("45 00: {short} x dict {item_dict} missing and the item is not sold: not placed");
                if count_now == 0 {
                    p.remove_item(uid);
                }
                continue;
            }
            if short > 0 {
                let (wallet, cost) = gd.shop.pocket_cost(item_dict, short);
                if p.spend(currency(wallet), cost).is_err() {
                    log::info!("45 00: {short} x dict {item_dict} cost {cost}: not enough money");
                    if count_now == 0 {
                        p.remove_item(uid);
                    }
                    continue;
                }
                spent |= cost > 0;
                p.item_mut(uid).unwrap().count += short;
                log::info!("45 00: bought {short} x dict {item_dict} for {cost}");
            }
            cnt = want;
        }
        p.slots.insert(dst, uid);
        if cnt > 0 {
            p.slot_rounds.insert(dst, cnt);
        }
    }
    // the weapon slots as the lobby movie allows them for the learned skills (game_data::perks): checked on the result,
    // because one request may move a weapon out of one slot after putting another in; a refused result undoes the request
    if let Err(e) = weapon_slots_ok(gd, &saved, p) {
        log::info!("45 00 refused: {e}");
        *p = saved;
        return EquipOutcome::default();
    }
    let changed = p
        .items
        .iter()
        .filter(|i| before.get(&i.uid) != Some(&i.count))
        .map(|i| i.uid)
        .collect();
    EquipOutcome { changed, spent }
}

/// Can the `45 00` entry put a locked pocket consumable into pocket `dst` with owned units only ([`apply`])? The stack
/// and the units are found as `apply` finds them: by uid with the entry's dict, else the first stack of the dict; the
/// units are those of every other pocket holding the stack (the source pocket of a move is vacated first) plus this one.
fn locked_owned_units(p: &Profile, iid: u32, did: u16, src: u8, dst: u8, cnt: u32) -> Result<(), String> {
    if src == 200 {
        return Err("a popup buy (source 200)".into());
    }
    let item = p.item(iid).filter(|i| i.dict_id == did).or_else(|| p.items.iter().find(|i| i.dict_id == did));
    let Some(item) = item.filter(|i| i.count > 0) else { return Err("no owned units".into()) };
    // the source pocket of this move is emptied before the item is placed (same test as `apply`)
    let vacated = src <= 22 && p.slots.get(&src) == Some(&item.uid) && (item.uid == iid || p.item(iid).is_none());
    let other: u32 = QUICK_SLOTS
        .filter(|k| *k != dst && !(vacated && *k == src) && p.slots.get(k) == Some(&item.uid))
        .map(|k| p.slot_rounds.get(&k).copied().unwrap_or(1).max(1))
        .sum();
    let need = other + cnt.max(1);
    if need > item.count {
        return Err(format!("{} owned, {need} needed (units of a locked item are not bought)", item.count));
    }
    Ok(())
}

/// The weapons of slots 7 / 10 after a request, checked only when the request changed them (a stored kit is not
/// re-judged by an unrelated move).
fn weapon_slots_ok(gd: &GameData, before: &Profile, p: &Profile) -> Result<(), game_data::perks::WeaponSlotError> {
    use game_data::perks::{PRIMARY_SLOT, SECONDARY_SLOT};
    let dict = |q: &Profile, s: u8| q.slots.get(&s).and_then(|u| q.item(*u)).map(|i| i.dict_id);
    let (pri, sec) = (dict(p, PRIMARY_SLOT), dict(p, SECONDARY_SLOT));
    if pri == dict(before, PRIMARY_SLOT) && sec == dict(before, SECONDARY_SLOT) {
        return Ok(());
    }
    gd.check_weapon_slots(&p.skills, pri, sec)
}

/// 45 00 (the plain ack `83 45 00 00` that clears the client's equip lock is a fixed reply, `dispatch::fixed_reply`):
/// new or grown
/// stacks as `83 45 0F` records (so the 87 never names an item the client does not have), then 87 with a bumped
/// profile version (the 3D character rebuilds only then), then 8B if money was spent.
pub fn equip(cx: &mut Ctx, peer: &mut Peer, profile: u32, entries: &[EquipEntry]) {
    let gd = &cx.lobby.data;
    let name = peer.name.clone();
    let now = crate::premium::unix_now();
    let opened = peer.account.map(|a| cx.lobby.store.specifics_opened(a).unwrap_or_default()).unwrap_or_default();
    let out = update(cx, peer, |p| {
        // the request names the profile being edited (`u32 profile id`); the client announces a tab change with 48 02, so
        // this is normally the current one. When it is another available profile, that one is edited (CHOICE).
        p.fix_current_profile(now);
        if let Some(i) = crate::profiles::index_of(profile).filter(|i| p.profile_available(*i, now)) {
            if i != p.equip.current {
                p.switch_profile(i);
            }
        }
        let o = apply(gd, p, entries, &opened);
        p.bump_version();
        (build::records(gd, p, &o.changed), build::account_87(gd, p, &name), o.spent)
    });
    let Some((records, acct, spent)) = out else { return };
    if !records.is_empty() {
        peer.reply(Reply::Inventory(Inventory45::Records { x: 0, items: records }));
    }
    peer.push(&S2c::Account(acct));
    if spent {
        send_money(cx, peer);
    }
    // 415-417 count the random modifiers of the worn items
    crate::achievements::after_skills(cx, peer);
}
