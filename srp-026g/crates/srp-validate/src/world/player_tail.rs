//! The player state's tail: everything after the head
//! (snapshot_rules.md section 7.4, world_validation.md section 10), as
//! `base_player::deserialize` 0x2DB490 reads it after the head's effect
//! count.
//!
//! The head ends at the player-effect count E (`PlayerState::head_len` =
//! 4M + 29; the crate's [`super::player_head`] reserves it). After it the
//! block reads, in order (PROVEN, 0x2DB490 call sites):
//! - the E player effects, each `game_effect_player`'s effect record
//!   (0x2DDDF0 -> 0x2DDE5D), the same 67-byte form the effects section's
//!   template has (world_validation.md section 8);
//! - the hit effects: a u32 count H, then H records, each a bool
//!   `is_exists` and, when set, a root emitter (a flag byte and a u32) and
//!   a u32, then two f32 (writer `hit_player_game_effect_presenter::
//!   serialize` 0x3402C0, element 0x14 bytes, 0x340347..0x3403CE);
//! - two active-object bytes, the current and the target: each 0xFF for
//!   none, else an index into the active-objects table at
//!   `[[player+0x120]+0x18B4C]`. The core's constructor makes that table
//!   with 255 entries (0x2CFFF4, end = begin + 0x3FC) and the reader uses a
//!   slot only after a null check (0x2DB671): a u8 is always an entry or
//!   none, and a null entry is guarded, so no value is a crash (PROVEN).
//!   The bytes are read, not bounded;
//! - the stamina: 29 bytes, an f32, four u32 raw times, a bool, two f32
//!   (`player_stamina::deserialize` 0x329400), no index;
//! - the inventory header (`inventory::deserialize` 0x2FE550): an f32
//!   `m_carried_weight`, a bool, a u8 carried-item id (0xFF for none, else
//!   an index into the same 255-entry table, used after a null check
//!   0x2FE5C8, then that battery's own state, below), a u8 artefact-slot
//!   count A and,
//!   when A is not 0, a u8 `slots_begin` and A u8 artefact ids (each 0xFF
//!   or an index into the artefacts table `[[inventory+0x108]+0x18940]`,
//!   0x2FE655; that table reserves 128 entries, 0x2CFFD7, and an id has no
//!   null check, so it is bounded), then the active and grenade slot bytes.
//!
//! When it is not 0xFF the carried item is a battery this player carries
//! (`victory_item_core::deserialize` 0x301660, [`battery`]), read before
//! the header goes on.
//!
//! After the inventory header come the kit's items in slot order, and
//! everything after them, which [`super::inventory`] reads to the state's
//! exact end against the match's kits ([`Context::inventory`](super::Context)):
//! the slots the header's artefacts took hold them, not the kit's; a pick
//! (the head's `weapon_pickup::Record`) puts its recipe's gun in its
//! weapon slot and may fill the picker's ammo slots.
//!
//! Without the kits (a tool) the walk stops at the inventory header.

use super::inventory::{self, Ammo, Body, Gun, Item, Measure, Owner, SLOTS, Slot, items_and_rest};
use super::AMMO_SLOTS;
use super::{Block, Context, PLAYER_FIELDS_AT};
use weapon_pickup::WeaponSlot;
use crate::reader::{Reader, Short};
use crate::{Part, Reason, Refusal, finite};

/// One player-effect record: the effects section's template form
/// (world_validation.md section 8: offsets 12..79 of the one-effect
/// template, 67 bytes). Only the leading flag is checked here; the effects
/// section's own template bounds the rest, and E is 0 in every observed
/// world.
const EFFECT_RECORD: usize = 67;
/// The inventory's slots, which bound `slots_begin + A`
/// (`inventory::deserialize` loop count 0x17, 0x2FE7A9).
const INVENTORY_SLOTS: usize = 23;
/// A player-effect or hit-effect count over this is refused: each record is
/// at least 9 bytes and a state is at most
/// [`PLAYER_STATE_CAP`](super::PLAYER_STATE_CAP) = 0x1000.
const MAX_RECORDS: u32 = (super::PLAYER_STATE_CAP / 9) as u32;
/// The most items a profile's table holds (`MAX_ITEMS`, static_info.md):
/// the item count a guessed walk tries up to.
const MAX_ITEMS: usize = 69;

/// A fault while walking the tail: where it is in the state and why.
type Fault = (usize, Reason);

/// The tail reads up to the inventory header and every index it reaches is
/// within its bound. With the profiles M is the place's item count and the
/// walk is strict; without them (a tool, or the game checking a world with
/// no live match) M is any count the head and the tail both fit, as the
/// bullets are framed when the profiles are unknown.
pub fn player_tail(block: &Block, context: &Context) -> Result<(), Refusal> {
    let at = block.at;
    let fault = match context.profiles {
        Some(profiles) => match profiles.get(usize::from(block.place)) {
            // A place with no profile was refused by the head already.
            None => return Ok(()),
            // The kit in use is the head's `current_index`, the state's
            // first byte: the inventory serialized is that kit's.
            Some(profile) => walk(block.state, profile.items, Some(block.place), context).err(),
        },
        None => guessed(block.state, context).err(),
    };
    match fault {
        None => Ok(()),
        Some((offset, reason)) => Err(Refusal::new(
            Part::Player(block.place),
            at.saturating_add(offset),
            reason,
        )),
    }
}

/// The largest item count a guessed walk tries: the head must fit, and a
/// profile holds at most [`MAX_ITEMS`].
fn max_items(state_len: usize) -> usize {
    // head_len = 4M + 29 <= state_len
    (state_len.saturating_sub(PLAYER_FIELDS_AT.saturating_add(27)) / 4).min(MAX_ITEMS)
}

/// Without the profiles: the tail walks for some item count M, as the
/// bullets search when the match is guessed. The fault reported is the
/// furthest any M reached.
fn guessed(state: &[u8], context: &Context) -> Result<(), Fault> {
    let mut furthest: Option<Fault> = None;
    for items in 0..=max_items(state.len()) {
        // No profiles means no match, so no inventory to walk: the walk
        // stops at the inventory header, as the front-only check did.
        match walk(state, items, None, context) {
            Ok(()) => return Ok(()),
            Err(fault) => {
                if furthest
                    .as_ref()
                    .is_none_or(|(reached, _)| fault.0 > *reached)
                {
                    furthest = Some(fault);
                }
            }
        }
    }
    Err(furthest.unwrap_or((0, Reason::TailUnread { at: 0 })))
}

/// Walks the tail of a state whose profile has `items` items, from the
/// head's end to the inventory header and, when the kit's `inventory` is
/// known, on through the items to the first with a body. `Err` is the
/// furthest offset reached and why.
fn walk(state: &[u8], items: usize, place: Option<u8>, context: &Context) -> Result<(), Fault> {
    let fixed = items
        .checked_mul(4)
        .and_then(|m| m.checked_add(PLAYER_FIELDS_AT))
        .ok_or((0, Reason::TailUnread { at: 0 }))?;
    let head_len = fixed.saturating_add(27);
    // The head's bytes: three bools 0x00/0xFF, the pitch and the rotation
    // numbers. The head's own check made these with the profiles; here they
    // make the item count M sound when the profiles are unknown.
    let head = state
        .get(..head_len)
        .ok_or((head_len, Reason::TailUnread { at: head_len }))?;
    for k in 0..3 {
        let at = fixed.saturating_add(k);
        if !matches!(head.get(at), Some(0x00 | 0xFF)) {
            return Err((at, Reason::TailUnread { at }));
        }
    }
    for k in [3usize, 7] {
        let at = fixed.saturating_add(k);
        let bits = u32_at(head, at).ok_or((at, Reason::TailUnread { at }))?;
        if !finite(bits) {
            return Err((at, Reason::TailNotNumber { at }));
        }
    }
    let e_at = fixed.saturating_add(23);
    let e_count = u32_at(head, e_at).ok_or((e_at, Reason::TailUnread { at: e_at }))?;
    let mut reader = Reader::new(
        state
            .get(head_len..)
            .ok_or((head_len, Reason::TailUnread { at: head_len }))?,
        head_len,
    );

    // The E player effects: each the same 67-byte effect record as the
    // effects section's one effect, so [`super::effects::record_is_safe`]
    // bounds the two embedded table indices and the floats (the native reader
    // would read a raw pointer or an unbounded global-table entry otherwise).
    // E is 0 in every world seen.
    if e_count > MAX_RECORDS {
        return Err((
            head_len,
            Reason::TailCount {
                what: "player effects",
                count: e_count,
            },
        ));
    }
    for _ in 0..e_count {
        let start = reader.at();
        let record = reader.take(EFFECT_RECORD).map_err(short)?;
        if !super::effects::record_is_safe(record) {
            return Err((start, Reason::TailEffect { at: start }));
        }
    }

    // The hit effects: a u32 count, then that many records.
    let h_count = reader.u32().map_err(short)?;
    if h_count > MAX_RECORDS {
        return Err((
            reader.at().saturating_sub(4),
            Reason::TailCount {
                what: "hit effects",
                count: h_count,
            },
        ));
    }
    for _ in 0..h_count {
        let start = reader.at();
        match reader.bool().map_err(short)? {
            None => return Err((start, Reason::TailHitEffect { at: start })),
            Some(true) => {
                // A root emitter (a flag byte and a u32) and a u32, then two
                // f32. The flag is 0 (an index, as the effects template).
                if reader.u8().map_err(short)? != 0 {
                    let flag_at = start.saturating_add(1);
                    return Err((flag_at, Reason::TailHitEffect { at: flag_at }));
                }
                reader.skip(8).map_err(short)?;
                finite_f32(&mut reader)?;
                finite_f32(&mut reader)?;
            }
            Some(false) => {
                finite_f32(&mut reader)?;
                finite_f32(&mut reader)?;
            }
        }
    }

    // The two active-object bytes, the current and the target. Each is
    // 0xFF (none) or an index into the active-objects table, which the
    // core's constructor makes with 255 entries (0x2CFFF4, end = begin +
    // 0x3FC) and the reader uses only after a null check (0x2DB671). So a
    // u8 is always an entry or none, and a null entry is guarded: no u8 is
    // a crash. PROVEN. They are read, not bounded.
    reader.skip(2).map_err(short)?;

    // The stamina (`player_stamina::deserialize` 0x329400): an f32
    // (0x32941A, movss into +0x6C), four u32 raw times (0x329427..0x329454,
    // plain stores into +0x70 +0x74 +0x7C +0x80 -- not floats, so no number
    // check), a bool (0x32945A, +0x78), two f32 (0x3294BB, 0x32954E, the
    // list inserts). 29 bytes.
    finite_f32(&mut reader)?;
    reader.skip(16).map_err(short)?;
    let bool_at = reader.at();
    if reader.bool().map_err(short)?.is_none() {
        return Err((bool_at, Reason::TailStaminaBool));
    }
    finite_f32(&mut reader)?;
    finite_f32(&mut reader)?;

    // The inventory header: a weight, a bool, the carried item, the
    // artefact slots, the active and grenade slot bytes.
    finite_f32(&mut reader)?;
    let bool_at = reader.at();
    if reader.bool().map_err(short)?.is_none() {
        return Err((bool_at, Reason::TailInventoryBool));
    }
    let carried = reader.u8().map_err(short)?;
    if carried != 0xFF {
        // A battery carried: an id of the same 255-entry object table
        // (0x2FE5B6, used after a null check 0x2FE5C8), then the battery's
        // own state (`victory_item_core::deserialize` 0x301660), carried by
        // this player.
        battery(&mut reader, place)?;
    }
    let a_count = reader.u8().map_err(short)?;
    let mut artefact_slots = 0..0u8;
    if usize::from(a_count) > INVENTORY_SLOTS {
        return Err((
            reader.at().saturating_sub(1),
            Reason::TailArtefactCount { count: a_count },
        ));
    }
    if a_count != 0 {
        let begin_at = reader.at();
        let begin = reader.u8().map_err(short)?;
        if usize::from(begin).saturating_add(usize::from(a_count)) > INVENTORY_SLOTS {
            return Err((
                begin_at,
                Reason::TailSlotsBegin {
                    begin,
                    count: a_count,
                },
            ));
        }
        artefact_slots = begin..begin.saturating_add(a_count);
        for _ in 0..a_count {
            let id_at = reader.at();
            let id = reader.u8().map_err(short)?;
            if id != 0xFF {
                artefact_index(id, context, id_at)?;
            }
        }
    }
    // The active and grenade slot bytes end the header: stored with no
    // lookup (0x2FE788, 0x2FE79D), no bound.
    reader.skip(2).map_err(short)?;
    // Past the header, the kit's items in slot order and the rest of the
    // state, to its end, when the match's inventory is known; without it
    // the walk stops here.
    let (Some(place), Some(inventory)) = (place, context.inventory) else {
        return Ok(());
    };
    let kit = state.first().copied().unwrap_or(0);
    let Some(own) = inventory
        .kits
        .get(usize::from(place))
        .and_then(|kits| kits.get(usize::from(kit)))
    else {
        // A kit the head refused already.
        return Ok(());
    };
    let alive = head.get(fixed) == Some(&0xFF);
    let record = state
        .get(2..PLAYER_FIELDS_AT)
        .and_then(|bytes| bytes.try_into().ok())
        .and_then(weapon_pickup::Record::decode)
        .unwrap_or(weapon_pickup::Record::EMPTY);
    let mut slots: [Option<Slot>; SLOTS as usize] = [None; SLOTS as usize];
    for item in own.iter() {
        if let Some(slot) = slots.get_mut(usize::from(item.slot)) {
            *slot = Some(Slot::Held(*item));
        }
    }
    // A pick moves the borrowed gun's stacks into the picker's ammo slots
    // (patch 85): each may hold one or not.
    if record.picks.iter().any(Option::is_some) {
        for ammo in AMMO_SLOTS {
            if let Some(slot) = slots.get_mut(usize::from(ammo)) {
                let item = Item { slot: ammo, measure: Measure::Amount, body: Body::None };
                *slot = Some(Slot::Maybe(item));
            }
        }
    }
    // The slots the header's artefacts took hold them, not the kit's.
    for slot in artefact_slots {
        if let Some(slot) = slots.get_mut(usize::from(slot)) {
            *slot = None;
        }
    }
    // A picked gun is in its weapon slot, the own gun set aside.
    for weapon in [WeaponSlot::First, WeaponSlot::Second] {
        let Some(held) = record.held(weapon) else {
            continue;
        };
        let recipe = held.pick.recipe;
        let gun = inventory
            .kits
            .get(usize::from(recipe.place.get()))
            .and_then(|kits| kits.get(usize::from(recipe.kit.get())))
            .and_then(|items| items.iter().find(|item| item.slot == recipe.slot.slot()));
        let Some(Item { measure, body: Body::Weapon(gun), .. }) = gun.copied() else {
            return Err((2, Reason::PickupRecord));
        };
        if let Some(slot) = slots.get_mut(usize::from(weapon.slot())) {
            let item = Item {
                slot: weapon.slot(),
                measure,
                body: Body::Weapon(Gun { ammo: Ammo::Picked, ..gun }),
            };
            *slot = Some(Slot::Held(item));
        }
    }
    let usable = |place: u8, kit: u8| {
        context.profiles.is_some_and(|profiles| {
            profiles
                .get(usize::from(place))
                .is_some_and(|profile| profile.usable(kit))
        })
    };
    let owner = Owner {
        places: context.places,
        usable: &usable,
    };
    items_and_rest(&reader, &slots, alive, inventory, &owner)
}

/// A carried battery (0x301660, writer 0x301470): its spotted mask, its
/// user (this player: with another the game takes the free battery's
/// layout, live state the stream does not say), its container (0xFF, or an
/// **unbounded** index into the gather rule's two containers, 0x3016AE),
/// its state machine's state (take, carry, drop, 0x301920; past them a
/// null state) with that state's last motion (`victory_item_core_base_state`
/// 0x344D70: a flag, two speeds, two transforms, a time; take and drop one
/// byte more, 0x344870), then its portable object, a weapon's class.
fn battery(reader: &mut Reader, place: Option<u8>) -> Result<(), Fault> {
    reader.skip(4).map_err(short)?;
    let at = reader.at();
    let user = reader.u8().map_err(short)?;
    if place.is_some_and(|place| user != place) {
        return Err((at, Reason::TailBattery { at }));
    }
    let at = reader.at();
    let container = reader.u8().map_err(short)?;
    if container != 0xFF && container >= BATTERY_CONTAINERS {
        return Err((at, Reason::TailBattery { at }));
    }
    let state = inventory::state_id(reader, BATTERY_STATES)?;
    let at = reader.at();
    if reader.bool().map_err(short)?.is_none() {
        return Err((at, Reason::TailBool { at }));
    }
    for _ in 0..(3 + 3 + 16 + 16) {
        finite_f32(reader)?;
    }
    reader.skip(4).map_err(short)?;
    if state != BATTERY_CARRY {
        // The animation it waits for: compared, not indexed.
        reader.skip(1).map_err(short)?;
    }
    inventory::portable(reader)
}

/// A battery mode's two containers (`maps::ProjectObjects`: one of each
/// team), the battery's states and the one of them that saves no
/// animation.
const BATTERY_CONTAINERS: u8 = 2;
const BATTERY_STATES: u8 = 3;
const BATTERY_CARRY: u8 = 1;

/// A u32 at `at` of `bytes`.
fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    bytes
        .get(at..)
        .and_then(<[u8]>::first_chunk::<4>)
        .map(|b| u32::from_le_bytes(*b))
}

/// Turns a short read into a tail fault at where it ran out.
fn short(s: Short) -> Fault {
    (s.at, Reason::TailUnread { at: s.at })
}

/// Reads an f32 and refuses a non-number.
fn finite_f32(reader: &mut Reader) -> Result<(), Fault> {
    let at = reader.at();
    let bits = reader.f32_bits().map_err(short)?;
    if finite(bits) {
        Ok(())
    } else {
        Err((at, Reason::TailNotNumber { at }))
    }
}

/// Bounds an artefact id (the artefact table at
/// `[[inventory+0x108]+0x18940]`).
fn artefact_index(index: u8, context: &Context, at: usize) -> Result<(), Fault> {
    if let Some(bound) = context.artefacts
        && usize::from(index) >= bound
    {
        return Err((at, Reason::TailArtefact { index, bound }));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::inventory::{Chamber, GunState};
    use crate::world::{Inventory, Profile};
    use std::vec::Vec;

    const ARTEFACTS: usize = 4;

    fn profile() -> Profile {
        Profile {
            usable_kits: 0b1,
            items: 1,
            ..Profile::default()
        }
    }

    fn context<'a>(profiles: Option<&'a [Profile]>) -> Context<'a> {
        Context {
            time: 0,
            places: 1,
            game_mode: crate::mode::BATTERIES,
            batteries: 0,
            respawn_points: None,
            profiles,
            artefacts: Some(ARTEFACTS),
            objects: None,
            inventory: None,
        }
    }

    /// A valid state: a head of one item, then the tail with no hit effect,
    /// no active object, a zero stamina block and an empty inventory header.
    /// The offsets of the bytes the tests poke are its fields.
    const HIT_AT: usize = 33 + weapon_pickup::Record::BYTES; // head_len for M = 1 (4 + 29)
    const ACTIVE_AT: usize = HIT_AT + 4;
    const STAMINA_AT: usize = ACTIVE_AT + 2;
    const STAMINA_BOOL_AT: usize = STAMINA_AT + 4 + 16;
    const WEIGHT_AT: usize = STAMINA_AT + 29;
    const CARRIED_AT: usize = WEIGHT_AT + 5;
    const A_AT: usize = CARRIED_AT + 1;

    fn valid_state() -> Vec<u8> {
        let mut s = Vec::new();
        // The head (M = 1): the two kit bytes, one item, three bools, the
        // pitch and the rotation, the raw input and actions, the effect
        // time, the effect count.
        s.extend([0, 0]);
        s.extend(weapon_pickup::Record::EMPTY.encode());
        s.extend(0u32.to_le_bytes());
        s.extend([0xFF, 0x00, 0x00]);
        s.extend(0.0f32.to_le_bytes());
        s.extend(0.0f32.to_le_bytes());
        s.extend([0; 4]);
        s.extend([0; 4]);
        s.extend([0; 4]);
        s.extend(0u32.to_le_bytes()); // E = 0
        assert_eq!(s.len(), HIT_AT);
        // The tail: no hit effect, no active object, a zero stamina, an
        // empty inventory header.
        s.extend(0u32.to_le_bytes()); // H = 0
        s.extend([0xFF, 0xFF]); // current and target active object: none
        assert_eq!(s.len(), STAMINA_AT);
        s.extend([0u8; 29]); // stamina
        assert_eq!(s.len(), WEIGHT_AT);
        s.extend(0.0f32.to_le_bytes()); // m_carried_weight
        s.push(0x00); // a bool
        assert_eq!(s.len(), CARRIED_AT);
        s.push(0xFF); // carried item: none
        s.push(0x00); // artefact slots A = 0
        s.extend([0, 0]); // the active and grenade slot bytes
        s
    }

    fn block(state: &[u8]) -> Block<'_> {
        Block {
            place: 0,
            at: 0,
            state,
        }
    }

    fn with_profile(state: &[u8]) -> Result<(), Refusal> {
        let profiles = [profile()];
        player_tail(&block(state), &context(Some(&profiles)))
    }

    #[test]
    fn a_real_player_tail_reads_to_the_inventory_header() {
        assert_eq!(with_profile(&valid_state()), Ok(()));
        // And the game checking a world with no live match guesses its item
        // count.
        assert_eq!(player_tail(&block(&valid_state()), &context(None)), Ok(()));
    }

    #[test]
    fn active_object_bytes_are_any_value_the_table_is_null_checked() {
        // The active-objects table has 255 entries and the reader
        // null-checks a slot, so any u8 is safe.
        for value in [0u8, 1, 200, 254, 0xFF] {
            let mut state = valid_state();
            state[ACTIVE_AT] = value;
            state[ACTIVE_AT + 1] = value;
            assert_eq!(with_profile(&state), Ok(()), "active {value}");
        }
    }

    #[test]
    fn a_carried_battery_reads_its_state_and_the_header_goes_on() {
        // Place 0 carries battery 3: its mask, its user (place 0), no
        // container, its state machine in carry (157 bytes), its portable
        // object (selector state 1: nothing), then A and the slot bytes.
        let battery = |user: u8, container: u8, state: u8| {
            let mut state_bytes = valid_state();
            let tail = state_bytes.split_off(CARRIED_AT);
            state_bytes.push(3);
            state_bytes.extend(1u32.to_le_bytes());
            state_bytes.extend([user, container, state, 0x00]);
            state_bytes.extend([0; 152]);
            if state != 1 {
                state_bytes.push(0);
            }
            state_bytes.extend([0; 4]);
            state_bytes.extend([0; 4]);
            state_bytes.extend([0x00, 1]);
            state_bytes.extend([0; 96]);
            state_bytes.extend(&tail[1..]);
            state_bytes
        };
        assert_eq!(with_profile(&battery(0, 0xFF, 1)), Ok(()));
        assert_eq!(with_profile(&battery(0, 1, 2)), Ok(()));
        let reason = |state: &[u8]| with_profile(state).expect_err("refused").reason;
        // Another user, a container past the two, a state past the three.
        assert!(matches!(reason(&battery(1, 0xFF, 1)), Reason::TailBattery { .. }));
        assert!(matches!(reason(&battery(0, 2, 1)), Reason::TailBattery { .. }));
        assert!(matches!(reason(&battery(0, 0xFF, 3)), Reason::TailGunState { state: 3, .. }));
    }

    #[test]
    fn a_stamina_or_inventory_bool_that_is_no_bool_goes_to_nobody() {
        let mut state = valid_state();
        state[STAMINA_BOOL_AT] = 0x01;
        let refusal = with_profile(&state).expect_err("refused");
        assert!(
            matches!(refusal.reason, Reason::TailStaminaBool),
            "{refusal}"
        );

        let mut state = valid_state();
        state[WEIGHT_AT + 4] = 0x01; // the inventory bool
        let refusal = with_profile(&state).expect_err("refused");
        assert!(
            matches!(refusal.reason, Reason::TailInventoryBool),
            "{refusal}"
        );
    }

    #[test]
    fn a_stamina_float_that_is_no_number_goes_to_nobody() {
        let mut state = valid_state();
        state[STAMINA_AT..STAMINA_AT + 4].copy_from_slice(&f32::NAN.to_le_bytes());
        let refusal = with_profile(&state).expect_err("refused");
        assert!(
            matches!(refusal.reason, Reason::TailNotNumber { .. }),
            "{refusal}"
        );
        // But the four u32 after it are raw times, not numbers, and pass.
        let mut state = valid_state();
        state[STAMINA_AT + 4..STAMINA_AT + 8].copy_from_slice(&0x7F80_0001u32.to_le_bytes());
        assert_eq!(with_profile(&state), Ok(()));
    }

    #[test]
    fn an_artefact_count_over_the_slots_goes_to_nobody() {
        let mut state = valid_state();
        state[A_AT] = 24;
        let refusal = with_profile(&state).expect_err("refused");
        assert!(
            matches!(refusal.reason, Reason::TailArtefactCount { .. }),
            "{refusal}"
        );
    }

    #[test]
    fn a_hit_effect_with_a_raw_root_emitter_goes_to_nobody() {
        // H = 1, one record present with a raw (non-index) root emitter.
        let mut state = valid_state();
        state.truncate(HIT_AT);
        state.extend(1u32.to_le_bytes()); // H = 1
        state.push(0xFF); // is_exists
        state.push(0x01); // the root emitter's flag: raw, not an index
        state.extend([0u8; 32]); // room, so the fault is the flag not a short read
        let refusal = with_profile(&state).expect_err("refused");
        assert!(
            matches!(refusal.reason, Reason::TailHitEffect { .. }),
            "{refusal}"
        );
    }

    #[test]
    fn a_tail_that_runs_short_goes_to_nobody() {
        let mut state = valid_state();
        state.truncate(STAMINA_AT + 3); // stops inside the stamina
        let refusal = with_profile(&state).expect_err("refused");
        assert!(
            matches!(refusal.reason, Reason::TailUnread { .. }),
            "{refusal}"
        );
    }

    #[test]
    fn a_hit_effect_round_reads_its_records() {
        // H = 1, one record, not present (a bool and two f32).
        let mut state = valid_state();
        let tail: Vec<u8> = state.split_off(HIT_AT);
        state.extend(1u32.to_le_bytes()); // H = 1
        state.push(0x00); // is_exists = false
        state.extend(0.0f32.to_le_bytes());
        state.extend(0.0f32.to_le_bytes());
        state.extend(&tail[4..]); // the active bytes onward
        assert_eq!(with_profile(&state), Ok(()));
    }

    // The item walk, past the inventory header, and the rest of the state.
    // `valid_state` ends at the header; the items follow in slot order,
    // then everything after them ([`rest`]).

    /// The body parts of the tests' damage model.
    const BODY_PARTS: usize = 2;
    const SHOW_FIRE: [GunState; 3] = [GunState::FlagByte, GunState::Flag, GunState::ThrowGrenade];

    /// An item's header: an f32 condition (not a stack), the owner (place,
    /// kit) and the slot.
    fn header_item(slot: u8, place: u8, kit: u8) -> Vec<u8> {
        let mut h = Vec::new();
        h.extend(1.0f32.to_le_bytes());
        h.extend([place, kit, slot]);
        h
    }

    /// A weapon that was never in a hand (no user): the header, the seed,
    /// the tick, the load flag, the magazine, the fire queue, the ammo id.
    fn weapon_item(slot: u8, place: u8, kit: u8, current_ammo: u8) -> Vec<u8> {
        let mut w = header_item(slot, place, kit);
        w.extend([0; 8]);
        w.extend([0x00, 10, 0x00, current_ammo]);
        w
    }

    /// The same weapon in hand (a user), in its state `state`, with
    /// `body` that state's bytes.
    fn weapon_in_hand(slot: u8, current_ammo: u8, state: u8, body: &[u8]) -> Vec<u8> {
        let mut w = weapon_item(slot, 0, 0, current_ammo);
        w.extend(u32::MAX.to_le_bytes()); // no ammo switch (a list of 2)
        w.extend([0; 4]);
        w.extend(u32::MAX.to_le_bytes()); // the recoil, no more
        w.extend([0; 40]); // two transitions
        w.extend([0; 4]); // a float
        w.push(0x00); // not aimed
        w.extend([0; 20]); // its transition
        w.extend([0, 0x00]); // sprint, auto-reload
        w.push(state);
        w.extend(body);
        w.extend([0; 4]);
        w.push(0x00);
        w.push(0); // the portable object's state
        w.extend([0; 96]);
        w
    }

    /// Everything after the items of a living player.
    fn rest() -> Vec<u8> {
        let mut r = std::vec![0];
        r.extend(0f32.to_le_bytes());
        r.push(0x00);
        r.extend([0; 16]);
        for _ in 0..BODY_PARTS {
            r.extend(1f32.to_le_bytes());
            r.extend([0; 4]);
            r.extend([0, 0]); // no affect, no hit
            r.extend(1f32.to_le_bytes());
        }
        r.push(0x00); // no usable-object data
        r.extend([0; 4]);
        r.extend([0; 12]); // the controller: position, six flags, four vectors, sprint
        r.extend([0; 6]);
        r.extend([0; 48]);
        r.push(0x00);
        r.extend([0; 8]);
        r.extend(1f32.to_le_bytes());
        r.extend(0u32.to_le_bytes());
        r.push(0x00);
        let mut carry = [0; 32];
        carry[0] = 2;
        r.extend(carry);
        r
    }

    fn gun(ammo: u8) -> Body<'static> {
        Body::Weapon(Gun {
            states: &SHOW_FIRE,
            chamber: Chamber::NotSaved,
            ammo: Ammo::Kit(ammo),
        })
    }

    fn item(slot: u8, body: Body<'static>) -> Item<'static> {
        Item { slot, measure: Measure::Condition, body }
    }

    /// The state checked with the match's inventory known: `kit0` is the
    /// items of kit 0 (the state's `current_index`), kits 1 and 2 empty.
    fn with_inventory(state: &[u8], kit0: &[Item]) -> Result<(), Refusal> {
        let profiles = [profile()];
        let kits: [[&[Item]; crate::KITS]; 1] = [[kit0, &[], &[]]];
        let inventory = Inventory {
            kits: &kits,
            body_parts: BODY_PARTS,
        };
        let context = Context {
            inventory: Some(&inventory),
            ..context(Some(&profiles))
        };
        player_tail(&block(state), &context)
    }

    fn reason(state: &[u8], kit0: &[Item]) -> Reason {
        with_inventory(state, kit0).expect_err("refused").reason
    }

    #[test]
    fn a_state_reads_to_its_end_through_its_items() {
        let mut state = valid_state();
        state.extend(header_item(0, 0, 0)); // armour
        state.extend(weapon_item(7, 0, 0, 1)); // a gun never in hand
        state.extend(weapon_in_hand(10, 0, 1, &[0x00])); // a gun idle in hand
        state.extend(rest());
        let items = [item(0, Body::None), item(7, gun(2)), item(10, gun(2))];
        assert_eq!(with_inventory(&state, &items), Ok(()));
        // A byte more, or a byte less: no state of the game.
        let mut long = state.clone();
        long.push(0);
        assert!(matches!(reason(&long, &items), Reason::TailUnread { .. }));
        let short = &state[..state.len() - 1];
        assert!(matches!(reason(short, &items), Reason::TailUnread { .. }));
    }

    #[test]
    fn a_weapons_indices_are_bounded() {
        let at = |ammo, state, body: &[u8]| {
            let mut s = valid_state();
            s.extend(weapon_in_hand(7, ammo, state, body));
            s.extend(rest());
            with_inventory(&s, &[item(7, gun(2))])
        };
        assert_eq!(at(0, 2, &[0x00, 0, 0, 0, 22, 0]), Ok(()));
        // The ammo id is read in both shapes: its refusal is the one.
        assert!(matches!(
            at(2, 1, &[0x00]).map_err(|refusal| refusal.reason),
            Err(Reason::TailWeaponAmmo { ammo: 2, count: 2 })
        ));
        // A state past the gun's three, a throw of the grenade of slot 23:
        // no shape reads them (the refusal is the shape that read furthest).
        assert!(at(0, 3, &[0x00]).is_err());
        assert!(at(0, 2, &[0x00, 0, 0, 0, 23, 0]).is_err());
    }

    #[test]
    fn an_item_owner_not_in_the_match_goes_to_nobody() {
        let mut state = valid_state();
        state.extend(weapon_item(7, 5, 0, 0)); // owner place 5, the match has 1
        state.extend(rest());
        assert!(matches!(
            reason(&state, &[item(7, gun(2))]),
            Reason::TailItemOwner { player_id: 5, .. }
        ));
    }

    #[test]
    fn a_medkit_saves_its_influences_when_active() {
        let medkit = [Item {
            slot: 13,
            measure: Measure::Amount,
            body: Body::Medkit { influences: 2 },
        }];
        let state = |active: bool| {
            let mut s = valid_state();
            s.extend([0, 0, 0, 0, 13]); // amount, owner, slot
            if active {
                s.push(0xFF);
                s.extend([0; 8]);
                s.extend([0; 8]);
            } else {
                s.push(0x00);
            }
            s.extend(rest());
            s
        };
        assert_eq!(with_inventory(&state(true), &medkit), Ok(()));
        assert_eq!(with_inventory(&state(false), &medkit), Ok(()));
        let mut nan = state(true);
        let at = valid_state().len() + 6 + 8;
        nan[at..at + 4].copy_from_slice(&f32::NAN.to_le_bytes());
        assert!(matches!(reason(&nan, &medkit), Reason::TailNotNumber { .. }));
    }

    #[test]
    fn the_rest_of_the_state_is_bounded() {
        let mut state = valid_state();
        let rest_at = state.len();
        state.extend(rest());
        assert_eq!(with_inventory(&state, &[]), Ok(()));
        // A body part affect of no type, too many hits, a carry snapshot of
        // another version.
        let part = rest_at + 1 + 4 + 1 + 16;
        let mut affect = state.clone();
        affect[part + 8] = 1;
        affect.splice(part + 9..part + 9, [9, 0, 0, 0, 0]);
        assert!(matches!(reason(&affect, &[]), Reason::TailAffect { kind: 9 }));
        let mut hits = state.clone();
        hits[part + 9] = 65;
        assert!(matches!(reason(&hits, &[]), Reason::TailCount { what: "hits", .. }));
        let mut carry = state.clone();
        let carry_at = carry.len() - 32;
        carry[carry_at] = 1;
        assert!(matches!(reason(&carry, &[]), Reason::TailCarry));
    }

    #[test]
    fn an_item_that_runs_past_the_state_goes_to_nobody() {
        let mut state = valid_state();
        state.extend(1.0f32.to_le_bytes()); // a weapon's condition, then nothing
        assert!(matches!(
            reason(&state, &[item(7, gun(2))]),
            Reason::TailUnread { .. }
        ));
    }

    #[test]
    fn without_the_inventory_the_walk_stops_at_the_header() {
        // The server leaves the inventory `None`: the items are not walked,
        // so bytes after the header (a weapon here) are not read.
        let mut state = valid_state();
        state.extend(weapon_item(7, 9, 0, 200)); // a bad owner and ammo, ignored
        assert_eq!(with_profile(&state), Ok(()));
    }
}
