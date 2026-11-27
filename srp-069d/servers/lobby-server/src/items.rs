//! Item use, chests, item search and the active profile (docs/notes/items.md).
//!
//! * `45 03` item use: the request is decoded and checked; effects of the usable items are not built yet (see
//!   [`use_item`]), so nothing is consumed without its effect.
//! * [`consume`]: the real count-down (profile rules of `Profile::consume`, the client's own `90` / `83 45 03`
//!   handling mirrored by the push), for chests and any rule that uses stack units up.
//! * `46 06` chest: `crate::chests`. `46 07` item search and `5A xx` (the treasure hunt): `crate::hunt`.
//! * `48 02` active profile: `crate::profiles::changed`.

use accounts::Consumed;
use protocol::s2c::*;

use crate::build;
use crate::dispatch::{update, Ctx};
use crate::peer::Peer;

/// Dictionary category of vouchers (`Category.VOUCHER` in the lobby movie): the only items the client offers
/// "use" for (`ItemData.usable` = stack and category 27 and voucher type 0, 3 or 4, ItemFactory.as line ~1006).
pub const VOUCHER_CATEGORY: u16 = 27;

/// Slots the client's equipment level ignores: the quick slots (pockets) 13..=17 and 21..=22 (loop of 0x3330C0).
pub(crate) fn level_ignores_slot(slot: u8) -> bool {
    (13..=17).contains(&slot) || (21..=22).contains(&slot)
}

/// The equipment level of a player (1..=5, 0 = nothing worn): the client's own number (`root.set_equipment_level`,
/// 0x1C7440 over 0x3330C0, PROVEN structure): the highest dictionary level byte of the worn items, over the profiles that can
/// be played; pocket slots do not count, and neither does a stack with no rounds / units in it. It is the `items_level` of the
/// AE squad record and what the client's PvE equipment recommendation and the ranked-match check compare (LIKELY that the byte is `item_level`: the
/// league text says "minimal tier 3" and the data's `item_level` runs 1..5).
pub fn equipment_level(gd: &game_data::GameData, p: &accounts::Profile, now: i64) -> u8 {
    let mut best = 0u32;
    for idx in 0..accounts::equip::PROFILES as u8 {
        if !p.profile_playable(idx, now) {
            continue;
        }
        let rounds = if idx == p.equip.current { &p.slot_rounds } else { &p.equip.profiles[idx as usize].rounds };
        for (&slot, &uid) in p.profile_slots(idx) {
            if level_ignores_slot(slot) {
                continue;
            }
            let Some(item) = p.item(uid).and_then(|r| gd.items.get(r.dict_id)) else { continue };
            if item.is_stackable && rounds.get(&slot) == Some(&0) {
                continue;
            }
            best = best.max(item.item_level);
        }
    }
    best.min(255) as u8
}

/// Use up `amount` units of stack `uid` and tell the client: `90` (`count - amount`, the record is erased at 0) and,
/// when the item left the inventory or a placed count was capped, a fresh `87` so the slot table follows.
/// Returns what happened; None when the account does not own the item.
pub fn consume(cx: &mut Ctx, peer: &mut Peer, uid: u32, amount: u32) -> Option<Consumed> {
    let gd = &cx.lobby.data;
    let name = peer.name.clone();
    let (done, acct) = update(cx, peer, |p| {
        let before = p.slots.clone();
        let rounds = p.slot_rounds.clone();
        let done = p.consume(uid, amount);
        let changed = before != p.slots || rounds != p.slot_rounds;
        (done, done.filter(|_| changed).map(|_| build::account_87(gd, p, &name)))
    })?;
    let done = done?;
    // the 90 amount is a u8: larger amounts are sent in steps
    let mut left = done.taken;
    let mut entries = Vec::new();
    while left > 0 {
        let step = left.min(255);
        entries.push((uid, step as u8));
        left -= step;
    }
    for chunk in entries.chunks(255) {
        peer.push(&S2c::ItemCounts(ItemCounts90 { entries: chunk.to_vec() }));
    }
    if let Some(a) = acct {
        peer.push(&S2c::Account(a));
    }
    Some(done)
}

/// `45 03 u32 item`: use one unit. Checked against the account's inventory and the client's own "usable" rule
/// (vouchers). OPEN: the effects (premium access time, boosters, account rename, repair, drop cards) need
/// subsystems the server does not have yet, so a use is logged and refused without touching the stack; the
/// client's use button does nothing visible then (no flag waits for an answer: `45 03` is not a locking request).
pub fn use_item(cx: &mut Ctx, peer: &mut Peer, item: u32) {
    if item == 0 {
        return;
    }
    let Some(id) = peer.account else { return };
    let row = match cx.lobby.store.load_profile(id) {
        Ok(p) => p.item(item).copied(),
        Err(e) => {
            log::error!("{}: loading profile of {id}: {e}", peer.addr);
            return;
        }
    };
    let Some(row) = row else {
        log::info!("{}: 45 03: item {item} not owned", peer.addr);
        return;
    };
    let cat = cx.lobby.data.items.category(row.dict_id);
    if cat != VOUCHER_CATEGORY || row.count == 0 {
        log::info!("{}: 45 03: item {item} (dict {}, category {cat}) is not usable", peer.addr, row.dict_id);
        return;
    }
    log::info!(
        "{}: 45 03: voucher {item} (dict {}): its effect is not implemented, the stack stays untouched",
        peer.addr,
        row.dict_id
    );
}

