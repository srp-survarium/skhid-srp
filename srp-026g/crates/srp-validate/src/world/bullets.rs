//! Current bullet records: a 16-byte header and N fixed 69-byte records.
//! Patch 85 always saves/loads the final quiet byte and clamps stale ammo
//! ordinals after weapon pickups. Original profile ammo cannot bound a bullet
//! fired by a picked-up gun. Validate framing, owners, slots and finite floats.
//!
//! The owner (`bullet::bullet`, the loader 0x32BBC0): its place goes to
//! `game_world::get_player` (0x1F7660, the engine's slot +24h, 0x32BD4E),
//! which takes the network client's slot +4Ch (0x26D660):
//! `[[core+18458h] + place*30h + 4*[..+0Ch]]`, the place's player object of
//! its current kit, with no bound on the place and no look at the world's
//! mask. A null player is kept at +8, but 0x32BDA1 reads its inventory
//! (`[player+10Ch]`) with no check, and 0x2E2750 the slot's item as a gun
//! (`[weapon+5B0h]`) with none: the reader needs a player object at the place
//! and a gun in the slot of its current kit. A place's objects last the
//! match: a game makes one for each usable kit of every roster place (the
//! current kit the lowest of them, 0x321820), the roster only grows, and a
//! player's leaving clears his bit of the events' masks alone
//! (`on_player_left_match` 0x2D2040). So a bullet whose shooter left with it
//! in flight, which has no block in the world, is read with the objects every
//! game still has of him.
use super::{Body, Context, Frame, WEAPON_SLOTS};
use crate::reader::Reader;
use crate::{KITS, MAX_PLAYERS, Part, Reason, Refusal, finite};

const HEADER: usize = 16;
const RECORD: usize = 69;

pub fn bullets(frame: &Frame<'_>, context: &Context<'_>) -> Result<(), Refusal> {
    let section = frame.bullets();
    let unread = |count| {
        Refusal::new(
            Part::Bullets,
            section.at,
            Reason::BulletsUnread {
                len: section.bytes.len(),
                bullets: count,
            },
        )
    };
    let mut reader = Reader::new(section.bytes, section.at);
    reader.skip(12).map_err(|_| unread(0))?;
    let count = reader.u32().map_err(|_| unread(0))?;
    let wanted = (count as usize)
        .checked_mul(RECORD)
        .and_then(|bytes| bytes.checked_add(HEADER));
    let Some(wanted) = wanted.filter(|&end| end <= section.bytes.len()) else {
        return Err(unread(count));
    };
    // Shipped breakable lamps append LMP1, the authored count, and its bitset.
    // Maps without lamp state have no suffix. The native lamp loader checks
    // the count against its project; here bound the wire value and its length.
    let suffix = section.bytes.get(wanted..).unwrap_or_default();
    if !suffix.is_empty() {
        let bad = || {
            Refusal::new(
                Part::Bullets,
                section.at.saturating_add(wanted),
                Reason::LampHistory,
            )
        };
        let mut lamps = Reader::new(suffix, 0);
        if lamps.array::<4>().map_err(|_| bad())? != *b"LMP1" {
            return Err(bad());
        }
        let count = lamps.u32().map_err(|_| bad())?;
        if !(1..=256).contains(&count)
            || lamps.rest().len() != (count as usize).div_ceil(32).saturating_mul(4)
        {
            return Err(bad());
        }
    }
    for number in 0..count as usize {
        let at = reader.at();
        let bytes = reader.array::<RECORD>().map_err(|_| unread(count))?;
        let [.., owner, second, slot, _ammo, _material, _quiet] = bytes;
        let reason = if second != owner {
            Some(Reason::BulletSecond {
                bullet: number,
                second,
                owner,
            })
        } else if !WEAPON_SLOTS.contains(&slot) {
            Some(Reason::BulletSlot {
                bullet: number,
                slot,
            })
        } else if frame.has_block(owner) {
            None
        } else {
            left(number, owner, slot, context)
        };
        if let Some(reason) = reason {
            return Err(Refusal::new(Part::Bullets, at, reason));
        }
        // Bytes 4..16 are the segment's origin (+28h, the loader 0x32BC19):
        // any f32. The game writes NaN there for a shooter whose bones went
        // NaN (rec026g_20261106-211841_03439962, place 12). The origin is
        // only computed with and compared: the ray test of a NaN segment
        // meets nothing, and update_bullet_position (0x32EECB..0x32EF0C)
        // takes a NaN or infinite position as outside the level and removes
        // the bullet at its first step.
        for offset in [16, 20, 24, 28, 32, 40, 44, 48, 52, 58] {
            let number_bits = bytes.get(offset..).and_then(<[u8]>::first_chunk::<4>);
            if !number_bits.is_some_and(|value| finite(u32::from_le_bytes(*value))) {
                return Err(Refusal::new(
                    Part::Bullets,
                    at,
                    Reason::BulletNotNumber { bullet: number },
                ));
            }
        }
    }
    Ok(())
}

/// Why a bullet whose owner has no block cannot be loaded, if it cannot: the
/// owner must be a place of the match (below the world's places and, when the
/// server has the roster's kits, one of them) whose every usable kit has a
/// gun in the slot. Which kit is current for a place with no block differs
/// between games (a newcomer's is the lowest usable, an older game's the last
/// it saw), so each must have one. Without the kits (the game's own check)
/// the place bound alone.
fn left(bullet: usize, owner: u8, slot: u8, context: &Context<'_>) -> Option<Reason> {
    let place = usize::from(owner);
    let no_place = Some(Reason::BulletOwner { bullet, owner });
    if place >= context.places.min(MAX_PLAYERS) {
        return no_place;
    }
    // Without the kits, no reason: the place bound alone.
    let inventory = context.inventory?;
    let (Some(kits), Some(profile)) = (
        inventory.kits.get(place),
        context.profiles.and_then(|profiles| profiles.get(place)),
    ) else {
        return no_place;
    };
    let gun = |kit: u8| {
        kits.get(usize::from(kit)).is_some_and(|items| {
            items
                .iter()
                .any(|item| item.slot == slot && matches!(item.body, Body::Weapon(_)))
        })
    };
    let mut usable = (0..KITS as u8).filter(|&kit| profile.usable(kit)).peekable();
    if usable.peek().is_none() {
        return no_place;
    }
    usable
        .find(|&kit| !gun(kit))
        .map(|kit| Reason::BulletNoWeapon { bullet, slot, kit })
}

#[cfg(test)]
mod tests {
    use super::super::{Ammo, Chamber, Gun, Inventory, Item, Measure, Profile};
    use super::*;
    fn body(records: &[u8], count: u32) -> std::vec::Vec<u8> {
        let mut body = 1u32.to_le_bytes().to_vec();
        body.extend([2, 0, 0, 0, 0]);
        body.extend(((HEADER + records.len()) as u16).to_le_bytes());
        body.extend([0; 12]);
        body.extend(count.to_le_bytes());
        body.extend(records);
        body.extend([0; 10]);
        body
    }
    fn context<'a>(
        places: usize,
        profiles: Option<&'a [Profile]>,
        inventory: Option<&'a Inventory<'a>>,
    ) -> Context<'a> {
        Context {
            time: 0,
            places,
            game_mode: 4,
            batteries: 0,
            respawn_points: None,
            profiles,
            artefacts: None,
            objects: None,
            inventory,
        }
    }
    fn checked(records: &[u8], count: u32, context: &Context<'_>) -> Result<(), Refusal> {
        bullets(&super::super::frame(&body(records, count), 1).unwrap(), context)
    }
    fn verdict(records: &[u8], count: u32) -> Result<(), Refusal> {
        checked(records, count, &context(1, None, None))
    }
    #[test]
    fn quiet_byte_is_always_present_and_old_ammo_ordinals_are_allowed() {
        let mut record = [0u8; RECORD];
        record[65] = 7;
        record[66] = 255; // native loading clamps this after a gun swap
        assert_eq!(verdict(&record, 1), Ok(()));
        assert!(verdict(&record[..68], 1).is_err());
        let pair = [record, record].concat();
        assert_eq!(verdict(&pair, 2), Ok(()));
        for length in 0..pair.len() {
            assert!(verdict(&pair[..length], 2).is_err());
        }
    }
    #[test]
    fn owner_slot_and_numbers_are_bounded() {
        let mut valid = [0u8; RECORD];
        valid[65] = 7;
        for (offset, value) in [(63, 20), (64, 1), (65, 23)] {
            let mut bad = valid;
            bad[offset] = value;
            assert!(verdict(&bad, 1).is_err());
        }
        // The origin may be NaN (a NaN shooter's); the velocity may not.
        let mut origin = valid;
        for at in [4, 8, 12] {
            origin[at..at + 4].copy_from_slice(&0xFFC0_0000u32.to_le_bytes());
        }
        assert_eq!(verdict(&origin, 1), Ok(()));
        valid[16..20].copy_from_slice(&f32::NAN.to_bits().to_le_bytes());
        assert!(verdict(&valid, 1).is_err());
        assert!(verdict(&[], u32::MAX).is_err());
    }

    /// A bullet of place 2, which has no block (the world's mask is place 0
    /// alone): its shooter left with it in flight
    /// (rec026g_20261106-205639_4900abd9, 149 550 ms: place 12's).
    #[test]
    fn a_bullet_of_a_place_that_left_needs_its_objects() {
        let record = |slot: u8| {
            let mut record = [0u8; RECORD];
            (record[63], record[64], record[65]) = (2, 2, slot);
            record
        };
        let reason = |result: Result<(), Refusal>| result.expect_err("refused").reason;
        // The game's own check: the places alone.
        assert_eq!(checked(&record(7), 1, &context(3, None, None)), Ok(()));
        assert!(matches!(
            reason(checked(&record(7), 1, &context(2, None, None))),
            Reason::BulletOwner { owner: 2, .. }
        ));
        // The server: every usable kit of the place has a gun in the slot.
        let gun = Body::Weapon(Gun {
            states: &[],
            chamber: Chamber::NotSaved,
            ammo: Ammo::Kit(1),
        });
        let item = |slot, body| Item {
            slot,
            measure: Measure::Condition,
            body,
        };
        let rifle = [item(0, Body::None), item(7, gun)];
        let none = [item(0, Body::None), item(7, Body::None)];
        let kits: [[&[Item]; KITS]; 3] = [[&[]; KITS], [&[]; KITS], [&rifle, &none, &rifle]];
        let inventory = Inventory {
            kits: &kits,
            body_parts: 1,
        };
        let usable = |kits| Profile {
            usable_kits: kits,
            ..Profile::default()
        };
        let server = |kits: u8, record: &[u8]| {
            let profiles = [usable(1), usable(1), usable(kits)];
            checked(record, 1, &context(3, Some(&profiles), Some(&inventory)))
        };
        assert_eq!(server(0b101, &record(7)), Ok(()));
        assert!(matches!(
            reason(server(0b111, &record(7))),
            Reason::BulletNoWeapon { slot: 7, kit: 1, .. }
        ));
        assert!(matches!(
            reason(server(0b001, &record(10))),
            Reason::BulletNoWeapon { slot: 10, kit: 0, .. }
        ));
        assert!(matches!(
            reason(server(0, &record(7))),
            Reason::BulletOwner { owner: 2, .. }
        ));
        // A place past the roster the server has.
        let profiles = [usable(1), usable(1)];
        assert!(matches!(
            reason(checked(&record(7), 1, &context(3, Some(&profiles), Some(&inventory)))),
            Reason::BulletOwner { owner: 2, .. }
        ));
    }

    #[test]
    fn lamp_history_suffix_has_a_bounded_exact_length() {
        for count in [1u32, 32, 33, 256] {
            let mut suffix = b"LMP1".to_vec();
            suffix.extend(count.to_le_bytes());
            suffix.extend(std::vec![0xff; count.div_ceil(32) as usize * 4]);
            assert_eq!(verdict(&suffix, 0), Ok(()));
            for len in 1..suffix.len() {
                assert!(verdict(&suffix[..len], 0).is_err());
            }
            for wrong in [0u32, 257, u32::MAX] {
                suffix[4..8].copy_from_slice(&wrong.to_le_bytes());
                assert!(verdict(&suffix, 0).is_err());
            }
        }
    }
}
