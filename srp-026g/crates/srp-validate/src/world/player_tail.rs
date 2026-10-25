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
//!   0x2FE5C8, then that battery's own state, which the exe does not trace
//!   to an end -- the walk stops there), a u8 artefact-slot count A and,
//!   when A is not 0, a u8 `slots_begin` and A u8 artefact ids (each 0xFF
//!   or an index into the artefacts table `[[inventory+0x108]+0x18940]`,
//!   0x2FE655; that table reserves 128 entries, 0x2CFFD7, and an id has no
//!   null check, so it is bounded), then the active and grenade slot bytes.
//!
//! After the inventory header come the inventory's items, by ascending
//! slot (`inventory::deserialize` loop 0x2FE7A9, 23 slots). The loop walks
//! the live kit's slot array, not the stream: which slots hold an item and
//! each item's shape (a `u16 m_amount` for a stack, else an `f32
//! m_condition`; a weapon's body or a header-only item) are the kit's, not
//! the state's. So the walk is given the kit's items as data
//! ([`Context::inventory`](super::Context)), the same way the artefacts
//! table size is. Each item's header has an owner lookup
//! `[core+0x18458]+(profile_id+player_id*12)*4` then `[entry+0x10c]` with
//! no null check (0x2FFF72), and a weapon a `m_current_ammo_id` indexing
//! its ammo list with no bound (0x2E350A) — both bounded here against the
//! match. The walk reads each item's header, bounds it, and stops at the
//! first item with a body: a weapon's body deserializes an FSM whose state
//! id picks that state's own virtual reader with no length marker
//! (`fsm::deserialize` 0x38CCF0, 0x38CD26; PROVEN), and a carried battery,
//! an oxygen tank's config loop and the query-driven grenade/medkit/trap
//! readers are the same, so no item body has a static length. Nor do the
//! readers after the items — an animation state (a bit stream), the damage
//! model with each body part's health (0x2E5480), the character controller
//! (0x7FB10), the spotting and the recent hits. So the state cannot be read
//! to its exact end from static RE; the walk bounds the reachable
//! forge-a-crash indexes (every item header reached and the first weapon's
//! ammo id) and stops. The frame's exact block length and
//! [`PLAYER_STATE_CAP`](super::PLAYER_STATE_CAP) bound the rest.
//!
//! Without the kit (a tool, or the server, which does not build the
//! inventory yet) the walk stops at the inventory header, as before.

use super::{Block, Context, Item};
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
            Some(profile) => {
                let kit = usize::from(block.state.first().copied().unwrap_or(0));
                walk(
                    block.state,
                    profile.items,
                    kit_items(context, block.place, kit),
                    context,
                )
                .err()
            }
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

/// The serialized items of `place`'s `kit`, when the match's inventory is
/// known; an empty slice otherwise (the walk then stops at the inventory
/// header).
fn kit_items<'a>(context: &Context<'a>, place: u8, kit: usize) -> &'a [Item] {
    context
        .inventory
        .and_then(|inventory| inventory.get(usize::from(place)))
        .and_then(|kits| kits.get(kit))
        .copied()
        .unwrap_or(&[])
}

/// The largest item count a guessed walk tries: the head must fit, and a
/// profile holds at most [`MAX_ITEMS`].
fn max_items(state_len: usize) -> usize {
    // head_len = 4M + 29 <= state_len
    (state_len.saturating_sub(29) / 4).min(MAX_ITEMS)
}

/// Without the profiles: the tail walks for some item count M, as the
/// bullets search when the match is guessed. The fault reported is the
/// furthest any M reached.
fn guessed(state: &[u8], context: &Context) -> Result<(), Fault> {
    let mut furthest: Option<Fault> = None;
    for items in 0..=max_items(state.len()) {
        // No profiles means no match, so no inventory to walk: the walk
        // stops at the inventory header, as the front-only check did.
        match walk(state, items, &[], context) {
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
fn walk(state: &[u8], items: usize, inventory: &[Item], context: &Context) -> Result<(), Fault> {
    let fixed = items
        .checked_mul(4)
        .and_then(|m| m.checked_add(2))
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

    // The E player effects: each the template's 67-byte effect record. E is
    // 0 in every world seen; a record is length-walked and its leading
    // root-emitter flag checked (the effects section's template bounds its
    // own records).
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
        if record.first() != Some(&0) {
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
        // A battery carried: an index into the same 255-entry object table
        // (0x2FE5B6, used after a null check 0x2FE5C8, so safe for any u8),
        // then that battery's own state, which the exe does not trace. The
        // walk stops here (the items follow and are not walkable either).
        return Ok(());
    }
    let a_count = reader.u8().map_err(short)?;
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
    // Past the header, the items by ascending slot. With the kit's items
    // known the walk bounds each item's owner lookup and a weapon's ammo id
    // and stops at the first item body; without them it stops here.
    walk_items(&mut reader, inventory, context)
}

/// Walks the inventory's items past the header, in slot order, as the kit's
/// objects deserialize them (`inventory::deserialize` loop 0x2FE7A9). Each
/// item's header is bounded -- its owner lookup, and a weapon's
/// `m_current_ammo_id`. The walk stops at the first item with a body (a
/// weapon's FSM, 0x38CCF0, and the readers after the items -- the damage
/// model 0x2E5480, the character controller 0x7FB10, the spotting and the
/// recent hits -- have no static length), or after the last header-only
/// item (those same readers follow it). `Ok` means every item reached is
/// bounded.
fn walk_items(reader: &mut Reader, items: &[Item], context: &Context) -> Result<(), Fault> {
    for item in items {
        // A `u16 m_amount` when the dictionary entry is a stack, else an
        // `f32 m_condition` (0x2FFF16): read, not indexed.
        reader.skip(if item.stack { 2 } else { 4 }).map_err(short)?;
        let owner_at = reader.at();
        let player_id = reader.u8().map_err(short)?;
        if player_id != 0xFF {
            let profile_id = reader.u8().map_err(short)?;
            item_owner(player_id, profile_id, context, owner_at)?;
            // The owner is a real object (bounded), so the slot id follows
            // (0x2FFFA0); 0xFF had no owner and no slot id.
            reader.u8().map_err(short)?;
        }
        if let Some(count) = item.ammo {
            // A weapon (`weapon_core::deserialize` 0x2E3490): the seed and
            // tick time (u32 each), three bytes, then `m_current_ammo_id` at
            // h+18, which indexes the ammo list of `count` with no bound.
            reader.skip(11).map_err(short)?;
            let ammo_at = reader.at();
            let current = reader.u8().map_err(short)?;
            if count > 0 && current > count {
                return Err((
                    ammo_at,
                    Reason::TailWeaponAmmo {
                        ammo: current,
                        count,
                    },
                ));
            }
            // The weapon's FSM and the rest of its body have no static
            // length: the walk stops, its header bounded.
            return Ok(());
        }
        if item.body {
            // A grenade, a medkit, an oxygen tank, a battery: the header is
            // bounded, the body is not walkable.
            return Ok(());
        }
    }
    Ok(())
}

/// Bounds an inventory item's owner lookup `[core+0x18458]+(profile_id+
/// player_id*12)*4`, then `[entry+0x10c]` with no null check (0x2FFF72):
/// `player_id` must be a place of the match and `profile_id` a usable kit
/// of it, so the table entry is a real player object (the writer only ever
/// sends such an owner, 0x2FFE4F).
fn item_owner(player_id: u8, profile_id: u8, context: &Context, at: usize) -> Result<(), Fault> {
    let ok = context.profiles.is_some_and(|profiles| {
        usize::from(player_id) < context.places
            && profiles
                .get(usize::from(player_id))
                .is_some_and(|profile| profile.usable(profile_id))
    });
    if ok {
        Ok(())
    } else {
        Err((
            at,
            Reason::TailItemOwner {
                player_id,
                profile_id,
            },
        ))
    }
}

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
    use crate::world::Profile;
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
    const HIT_AT: usize = 33; // head_len for M = 1 (4 + 29)
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
    fn a_carried_item_that_is_not_0xff_is_a_battery_and_stops_the_walk() {
        // Not 0xFF: a battery carried, whose own state the walk does not
        // read; the walk stops (any index is safe, the table is
        // null-checked).
        let mut state = valid_state();
        state[CARRIED_AT] = 3;
        assert_eq!(with_profile(&state), Ok(()));
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

    // The item walk, past the inventory header. `valid_state` ends at the
    // header; the items follow, in ascending slot order.

    /// A header-only item (armour, boosters, ...): an f32 condition, then the
    /// owner (place, kit) and the slot id (5 bytes, no body).
    fn header_item(slot: u8, place: u8, kit: u8) -> Vec<u8> {
        let mut h = Vec::new();
        h.extend(1.0f32.to_le_bytes()); // m_condition (not a stack)
        h.push(place); // player_id
        h.push(kit); // player_profile_id
        h.push(slot); // m_slot_id
        h
    }

    /// A weapon item's bytes as far as the walk reads them: the header, then
    /// h+7..18 and `m_current_ammo_id`.
    fn weapon_item(slot: u8, place: u8, kit: u8, current_ammo: u8) -> Vec<u8> {
        let mut w = header_item(slot, place, kit);
        w.extend(0u32.to_le_bytes()); // seed
        w.extend(0u32.to_le_bytes()); // m_last_tick_time_in_ms
        w.push(0x00); // m_load_ammo_on_next_activate
        w.push(10); // m_ammo_in_magazine
        w.push(0x00); // m_fire_queue_type
        w.push(current_ammo); // m_current_ammo_id
        w
    }

    /// The tail checked with the match's inventory known: `kit0` is the
    /// items of kit 0 (the state's `current_index`), kits 1 and 2 empty.
    fn with_inventory(state: &[u8], kit0: &[Item]) -> Result<(), Refusal> {
        let profiles = [profile()];
        let kits: [&[Item]; crate::KITS] = [kit0, &[], &[]];
        let inventory = [kits];
        let context = Context {
            inventory: Some(&inventory),
            ..context(Some(&profiles))
        };
        player_tail(&block(state), &context)
    }

    #[test]
    fn a_weapon_item_reads_its_header_and_bounds_the_ammo_id() {
        let mut state = valid_state();
        state.extend(weapon_item(7, 0, 0, 1)); // ammo id 1, of 2
        let items = [Item {
            slot: 7,
            stack: false,
            body: true,
            ammo: Some(2),
        }];
        assert_eq!(with_inventory(&state, &items), Ok(()));
    }

    #[test]
    fn a_weapon_ammo_id_past_the_list_goes_to_nobody() {
        let mut state = valid_state();
        state.extend(weapon_item(7, 0, 0, 5)); // 5, over the 2 ammo
        let items = [Item {
            slot: 7,
            stack: false,
            body: true,
            ammo: Some(2),
        }];
        let refusal = with_inventory(&state, &items).expect_err("refused");
        assert!(
            matches!(refusal.reason, Reason::TailWeaponAmmo { ammo: 5, count: 2 }),
            "{refusal}"
        );
    }

    #[test]
    fn an_item_owner_not_in_the_match_goes_to_nobody() {
        let mut state = valid_state();
        state.extend(weapon_item(7, 5, 0, 0)); // owner place 5, the match has 1
        let items = [Item {
            slot: 7,
            stack: false,
            body: true,
            ammo: Some(2),
        }];
        let refusal = with_inventory(&state, &items).expect_err("refused");
        assert!(
            matches!(refusal.reason, Reason::TailItemOwner { player_id: 5, .. }),
            "{refusal}"
        );
    }

    #[test]
    fn header_only_items_before_a_weapon_are_bounded_then_the_weapon_stops() {
        let mut state = valid_state();
        state.extend(header_item(0, 0, 0)); // armour, slot 0
        state.extend(header_item(3, 0, 0)); // another, slot 3
        state.extend(weapon_item(7, 0, 0, 0));
        let items = [
            Item {
                slot: 0,
                stack: false,
                body: false,
                ammo: None,
            },
            Item {
                slot: 3,
                stack: false,
                body: false,
                ammo: None,
            },
            Item {
                slot: 7,
                stack: false,
                body: true,
                ammo: Some(2),
            },
        ];
        assert_eq!(with_inventory(&state, &items), Ok(()));
        // A header-only item with a bad owner is caught before the weapon.
        let mut state = valid_state();
        state.extend(header_item(0, 9, 0)); // owner place 9, past the match
        state.extend(weapon_item(7, 0, 0, 0));
        let items = [
            Item {
                slot: 0,
                stack: false,
                body: false,
                ammo: None,
            },
            Item {
                slot: 7,
                stack: false,
                body: true,
                ammo: Some(2),
            },
        ];
        let refusal = with_inventory(&state, &items).expect_err("refused");
        assert!(
            matches!(refusal.reason, Reason::TailItemOwner { player_id: 9, .. }),
            "{refusal}"
        );
    }

    #[test]
    fn an_item_that_runs_past_the_state_goes_to_nobody() {
        let mut state = valid_state();
        state.extend(1.0f32.to_le_bytes()); // a weapon's condition, then nothing
        let items = [Item {
            slot: 7,
            stack: false,
            body: true,
            ammo: Some(2),
        }];
        let refusal = with_inventory(&state, &items).expect_err("refused");
        assert!(
            matches!(refusal.reason, Reason::TailUnread { .. }),
            "{refusal}"
        );
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
