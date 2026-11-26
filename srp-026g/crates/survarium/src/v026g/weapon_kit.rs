//! A weapon's upgrades on the wire: the weapon kit
//! (`docs/notes/weapon-kit.md`; client patch 68). The encoding is the
//! `weapon-kit` crate's, which the client's arena builds too: the client
//! reads exactly what srp packs.
//!
//! The client keeps at most three upgrade records an item
//! ([`super::lobby::limits::UPGRADES`]). srp keeps a weapon's upgrades in
//! seven logical slots, one upgrade a slot (the account's records):
//!
//! | slot | what |
//! |---|---|
//! | 0, 1 | a decal ([`super::modifications::WEAPON_DECAL_SLOTS`]) |
//! | 2 | the camouflage ([`super::modifications::WEAPON_CAMO_SLOT`]) |
//! | 3 | the scope ([`super::scopes::SCOPE_SLOT`]) |
//! | 4 | a muzzle device ([`MUZZLE_SLOT`]; no item yet) |
//! | 5 | a grip ([`GRIP_SLOT`]) |
//! | 6 | a second sight ([`SECOND_SIGHT_SLOT`]): an optic on the offset rail at the gun's side, its record's value its side (0 left, 1 right) |
//!
//! and sends them packed ([`pack`]): the decals as they are, then one kit
//! record in slot 3 that holds the first of scope, camouflage, muzzle, grip
//! and second sight and names the others by dict id (the wide kit when a
//! second sight is held). Every copy of the item (the
//! lobby's inventory and kits, a match's static info, a late join's, a
//! replay's) carries those records as they are.

pub use weapon_kit::{KIT_MAGIC, KIT_MAGIC_WIDE, KIT_RECORD_SLOT as KIT_SLOT, Malformed, Side};
use weapon_kit::{Held, Logical, Record, Slot};

use super::lobby::Upgrade;

/// A muzzle device's logical slot (a flash hider, a brake).
pub const MUZZLE_SLOT: u8 = Slot::Muzzle as u8;
/// A grip's logical slot.
pub const GRIP_SLOT: u8 = Slot::Grip as u8;
/// A second sight's logical slot.
pub const SECOND_SIGHT_SLOT: u8 = Slot::SecondSight as u8;
/// The logical slots of a weapon, decals first.
pub const WEAPON_SLOTS: [u8; 7] = [0, 1, 2, 3, MUZZLE_SLOT, GRIP_SLOT, SECOND_SIGHT_SLOT];

/// A weapon's records (logical slots, the account's) as the client is sent
/// them: at most three. A record of no weapon slot, or a second one of a
/// slot, is not sent (the account never holds one: attach replaces).
pub fn pack(logical: &[Upgrade]) -> Vec<Upgrade> {
    let mut slots = Logical::new();
    for held in logical {
        if let Some(slot) = Slot::from_byte(held.param)
            && slots.get(slot).is_none()
        {
            slots.set(
                slot,
                Some(Held::Item {
                    id: held.id,
                    dict: held.a,
                    value: held.value,
                }),
            );
        }
    }
    slots.pack().as_slice().iter().map(upgrade_of).collect()
}

/// Records as the client was sent them, by logical slot: the kit's own in
/// its own slot (value 0, a second sight's its side), each dict id it names
/// a record of item id 0 (a second sight's value its side).
/// What the client would refuse is refused.
pub fn unpack(wire: &[Upgrade]) -> Result<Vec<Upgrade>, Malformed> {
    let records: Vec<Record> = wire.iter().map(record_of).collect();
    let logical = Logical::unpack(&records)?;
    Ok(logical
        .iter()
        .map(|(slot, held)| {
            let (id, value) = match held {
                Held::Item { id, value, .. } => (id, value),
                Held::Named(_, value) => (0, value),
            };
            Upgrade {
                id,
                a: held.dict(),
                value,
                param: slot.byte(),
            }
        })
        .collect())
}

/// `record` is a kit (slot 3, [`KIT_MAGIC`] or [`KIT_MAGIC_WIDE`] on top,
/// well formed).
pub fn is_kit(record: &Upgrade) -> bool {
    matches!(weapon_kit::Kit::of(&record_of(record)), Ok(Some(_)))
}

fn record_of(upgrade: &Upgrade) -> Record {
    Record {
        id: upgrade.id,
        dict: upgrade.a,
        value: upgrade.value,
        slot: upgrade.param,
    }
}

fn upgrade_of(record: &Record) -> Upgrade {
    Upgrade {
        id: record.id,
        a: record.dict,
        value: record.value,
        param: record.slot,
    }
}

#[cfg(test)]
mod tests;
