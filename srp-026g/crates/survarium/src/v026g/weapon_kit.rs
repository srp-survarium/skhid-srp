//! A weapon's attachments on the wire: the weapon kit
//! (`docs/notes/weapon-kit.md`; client patch 68).
//!
//! The client keeps at most three upgrade records an item
//! ([`super::lobby::limits::UPGRADES`]: the item record's inline array, a
//! fourth is a fatal assert). Stock 0.26g gave weapons none; srp's weapons
//! hold up to six things, each in a logical slot of its own:
//!
//! | slot | what |
//! |---|---|
//! | 0, 1 | a decal ([`super::modifications::WEAPON_DECAL_SLOTS`]) |
//! | 2 | the camouflage ([`super::modifications::WEAPON_CAMO_SLOT`]) |
//! | 3 | the scope ([`super::scopes::SCOPE_SLOT`]) |
//! | 4 | a muzzle device ([`MUZZLE_SLOT`]; no item yet) |
//! | 5 | a grip ([`GRIP_SLOT`]; no item yet) |
//!
//! The server keeps them so (the account's records, one a slot) and sends
//! them packed ([`pack`]): the decals as they are, then one **kit** record
//! in slot 3 for the rest. The kit is the first of scope, camouflage,
//! muzzle, grip the weapon wears, its item id and record dict id as that
//! one's own record would have them; its value names the others by dict id:
//!
//! ```text
//! bits  0..15  the camouflage's dict id (0: none, or the kit's own)
//! bits 16..31  the muzzle device's
//! bits 32..47  the grip's
//! bits 48..55  the kit's own logical slot (2, 3, 4 or 5)
//! bits 56..63  KIT_MAGIC
//! ```
//!
//! Patch 68 has the client's item config cook apply the modification of each
//! dict id a kit names as if it were a record of its own
//! (`weapon_kit_026g.c`, `weapon_kit_expand`); its preview of a change
//! (patch 48) and this module pack the same way. Every copy of the item
//! (the lobby's inventory and kits, a match's static info, a late join's, a
//! replay's) carries the records as they are, so nothing else changes.
//! What a record names must be in the items dictionary with a
//! modification: the client reads the row it did not find.

use super::lobby::Upgrade;

/// The value's top byte that makes a slot-3 record a kit.
pub const KIT_MAGIC: u8 = 0xA7;
/// The record slot the kit is sent in (the scope's).
pub const KIT_SLOT: u8 = 3;
/// A muzzle device's logical slot (silencer, flash hider, brake).
pub const MUZZLE_SLOT: u8 = 4;
/// A grip's logical slot.
pub const GRIP_SLOT: u8 = 5;
/// The logical slots of a weapon, decals first.
pub const WEAPON_SLOTS: [u8; 6] = [0, 1, 2, 3, MUZZLE_SLOT, GRIP_SLOT];
/// The kit's own: the first of these the weapon wears.
pub const KIT_ORDER: [u8; 4] = [3, 2, MUZZLE_SLOT, GRIP_SLOT];
/// The logical slots the kit's value names, and the bit each begins at.
pub const FIELDS: [(u8, u32); 3] = [(2, 0), (MUZZLE_SLOT, 16), (GRIP_SLOT, 32)];

/// `record` is a kit (slot 3, [`KIT_MAGIC`], its own slot 2..=5).
pub fn is_kit(record: &Upgrade) -> bool {
    let own = own_slot(record.value);
    record.param == KIT_SLOT && (record.value >> 56) as u8 == KIT_MAGIC && (2..=5).contains(&own)
}

fn own_slot(value: u64) -> u8 {
    (value >> 48) as u8
}

/// A weapon's records (logical slots) as the client is sent them: the
/// records of slots 0 and 1 in that order, then the kit; a record of
/// another slot after them while there is room. At most three.
pub fn pack(logical: &[Upgrade]) -> Vec<Upgrade> {
    let room = super::lobby::limits::UPGRADES;
    let mut wire: Vec<Upgrade> = Vec::with_capacity(room);
    for slot in [0, 1] {
        if let Some(held) = logical.iter().find(|held| held.param == slot) {
            wire.push(*held);
        }
    }
    let own = KIT_ORDER
        .iter()
        .find_map(|&slot| logical.iter().find(|held| held.param == slot));
    if let Some(own) = own {
        let mut value = u64::from(KIT_MAGIC) << 56 | u64::from(own.param) << 48;
        for (slot, shift) in FIELDS {
            if slot == own.param {
                continue;
            }
            if let Some(held) = logical.iter().find(|held| held.param == slot) {
                value |= u64::from(held.a & 0xFFFF) << shift;
            }
        }
        wire.push(Upgrade {
            id: own.id,
            a: own.a,
            value,
            param: KIT_SLOT,
        });
    }
    wire.extend(
        logical
            .iter()
            .filter(|held| held.param > GRIP_SLOT)
            .copied(),
    );
    wire.truncate(room);
    wire
}

/// Records as the client was sent them, in logical slots: a kit's own in
/// its own slot (value 0), each dict id it names as a record of item id 0.
pub fn unpack(wire: &[Upgrade]) -> Vec<Upgrade> {
    let mut logical = Vec::new();
    for record in wire {
        if !is_kit(record) {
            logical.push(*record);
            continue;
        }
        let own = own_slot(record.value);
        logical.push(Upgrade {
            value: 0,
            param: own,
            ..*record
        });
        for (slot, shift) in FIELDS {
            let dict_id = (record.value >> shift) as u16;
            if dict_id != 0 && slot != own {
                logical.push(Upgrade {
                    id: 0,
                    a: dict_id.into(),
                    value: 0,
                    param: slot,
                });
            }
        }
    }
    logical
}

#[cfg(test)]
mod tests;
