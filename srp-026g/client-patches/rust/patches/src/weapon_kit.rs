//! Patch 68: a weapon's upgrades in at most three records, the weapon kit
//! (`docs/notes/weapon-kit.md`). srp sends a weapon's decals as records of
//! their own and one kit record (record slot 3) that holds the first of its
//! scope, camouflage, muzzle device and grip and names the others by dict id
//! (the `weapon-kit` crate, srp's own encoding). Here the client reads it:
//!
//! - `on_query` (translate_query, `weapon_kit.query_records`): the item
//!   config cook applies the records and each dict id the kit names, as
//!   records of their own (the kit's own valued 0, as its record was before
//!   the kit), from patch 68's room on the cook's stack; the item's records
//!   stay as they are. No kit, or a malformed one: the stock way;
//! - `on_flash_upgrade` (the lobby's upgrades to Flash): a kit record's
//!   object gets `kit_own` (its own slot) and `kit_camo`, `kit_muzzle`,
//!   `kit_grip` (the dict ids it names, 0 none), which the lobby movie's
//!   UpgradeWindow.kitUpgrades makes slots of;
//! - `on_attached_pair` (setup_inventory_data): each dict id a kit names
//!   marks the inventory's item of that dict id (the lowest item id) as on
//!   the kit's weapon, as a record of its own would (its is_attached_to);
//! - `on_preview_apply` (patch 48's queue of a window's changes, the entry
//!   `weapon_kit.preview_apply`): applied on the logical slots as srp applies
//!   them, packed again.
//!
//! What a match computes does not change: the cook applies the same
//! modifications with the same values as from the records srp sent before
//! the kit (`docs/notes/weapon-kit.md`, "The simulation").

#![forbid(unsafe_code)]

use weapon_kit::{Kit, KitSlot, Logical, Slot};

use crate::vostok::{AttachedPairs, Context, CookQuery, Flow, FlashUpgrade, NoExit, PreviewApply, PreviewChange};

/// translate_query after it copied its query (0x2F1776).
pub fn on_query(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(mut query) = CookQuery::at_records(ctx) {
        query.apply_kit();
    }
    Flow::Continue
}

/// The members a kit record's object gets: its own slot, then the dict id
/// it names for each of the camouflage, the muzzle device and the grip.
pub fn kit_members(kit: &Kit) -> [(&'static core::ffi::CStr, u32); 4] {
    let named = |slot| kit.named().find(|(named, _)| *named == slot).map_or(0, |(_, dict)| u32::from(dict.get()));
    [
        (c"kit_own", u32::from(kit.own().slot().byte())),
        (c"kit_camo", named(KitSlot::Camo)),
        (c"kit_muzzle", named(KitSlot::Muzzle)),
        (c"kit_grip", named(KitSlot::Grip)),
    ]
}

/// The lobby's upgrades to Flash, after a record's object has its `param`
/// (0x2882E3).
pub fn on_flash_upgrade(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(upgrade) = FlashUpgrade::at(ctx) else { return Flow::Continue };
    let Some(record) = upgrade.record() else { return Flow::Continue };
    if let Ok(Some(kit)) = Kit::of(&record) {
        for (name, value) in kit_members(&kit) {
            upgrade.set_uint(name, value);
        }
    }
    Flow::Continue
}

/// setup_inventory_data after a record's {item id, host} pair (0x2885CE).
pub fn on_attached_pair(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(mut pairs) = AttachedPairs::at(ctx) else { return Flow::Continue };
    let Some(record) = pairs.record() else { return Flow::Continue };
    if let Ok(Some(kit)) = Kit::of(&record) {
        for (_, dict) in kit.named() {
            if let Some(item) = pairs.item_of(dict.get()) {
                pairs.push(item);
            }
        }
    }
    Flow::Continue
}

/// Patch 48's apply (the entry `weapon_kit.preview_apply`): its queue applied
/// to the window's weapon's record, each change as srp applies it
/// (`items_rules::attach_upgrade_in`, `detach_upgrade`) on the logical slots,
/// the result packed as srp packs it. Records that are no weapon's
/// (malformed) stay as they are.
pub fn on_preview_apply(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(mut preview) = PreviewApply::at(ctx) else { return Flow::Continue };
    let Some(records) = preview.records() else { return Flow::Continue };
    if let Ok(mut logical) = Logical::unpack(records.as_slice()) {
        apply(&mut logical, preview.changes());
        preview.set(&logical.pack());
    }
    Flow::Continue
}

fn apply(logical: &mut Logical, changes: impl Iterator<Item = PreviewChange>) {
    for change in changes {
        match change {
            PreviewChange::TakeOff { upgrade, dict } => logical.take_off(upgrade, dict),
            PreviewChange::Put { upgrade, dict, slot } => {
                if let Some(slot) = Slot::from_byte(slot) {
                    logical.put(slot, upgrade, dict);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use weapon_kit::Record;

    const MAGIC: u64 = 0xA7 << 56;

    fn record(id: u32, dict: u32, value: u64, slot: u8) -> Record {
        Record { id, dict, value, slot }
    }

    #[test]
    fn a_camouflage_dropped_on_a_scoped_weapon_with_two_decals() {
        let sent = [record(1, 601, 0x11, 0), record(2, 602, 0x22, 1), record(3, 362, MAGIC | 3 << 48, 3)];
        let mut logical = Logical::unpack(&sent).unwrap();
        apply(&mut logical, [PreviewChange::Put { upgrade: 4, dict: 900, slot: 2 }].into_iter());
        assert_eq!(
            logical.pack().as_slice(),
            [record(1, 601, 0x11, 0), record(2, 602, 0x22, 1), record(3, 362, MAGIC | 3 << 48 | 900, 3)]
        );
        // taken off again; a slot of no weapon is left out
        apply(
            &mut logical,
            [PreviewChange::TakeOff { upgrade: 4, dict: 900 }, PreviewChange::Put { upgrade: 5, dict: 1, slot: 9 }]
                .into_iter(),
        );
        assert_eq!(logical.pack().as_slice(), sent);
    }

    #[test]
    fn the_movie_gets_the_kits_fields() {
        let kit = Kit::of(&record(3, 362, MAGIC | 3 << 48 | 2301 << 32 | 900, 3)).unwrap().unwrap();
        assert_eq!(
            kit_members(&kit).map(|(name, value)| (name.to_str().unwrap(), value)),
            [("kit_own", 3), ("kit_camo", 900), ("kit_muzzle", 0), ("kit_grip", 2301)]
        );
    }
}
