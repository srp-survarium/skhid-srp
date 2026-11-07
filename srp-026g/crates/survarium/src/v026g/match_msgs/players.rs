//! The player blocks of a world snapshot: where each ends, and what a
//! player has left (`docs/notes/snapshot_rules.md` §7).
//!
//! `player_history_item::serialize` 0x33FDE0 (reader 0x33FEE0) writes, for
//! each set bit of the body's `active_clients_mask`:
//!
//! ```text
//! u16 L                  (u16)m_buffer.size()
//! L bytes                the player state                       [`PlayerState`]
//! u8  animation_tree != 0   (0x00 or 0xFF)
//! if set: the animation state, a bit stream with no length     [`animation_len`]
//! ```
//!
//! PROVEN in the exe. So the blocks can be walked from the front, and the
//! six sections start where the last block ends ([`walk`]). The animation
//! bit stream's length follows from its first 16 bits and the two tables
//! after its tree part; the same routines are in 0.21d, where the walk
//! lands on the sections of every snapshot kept (1222 of them).
//!
//! Of the state, the head is fixed (PROVEN): which of the player's objects
//! it is, client patch 85's pickup record ([`FIELDS_AT`]), one u32 for
//! every item of the profile (what is left in store of
//! a stack item, the condition of another), whether the player is alive.
//! The inventory further on has variable parts that were not all traced
//! (effects, weapons), so its items are found by their header's three id
//! bytes, in slot order [LIKELY: the roster place and the slot number in
//! them]. The walk reads every world of real 0.26g matches the server
//! saved (33,106 of them, 2026-10-12..14).

use std::collections::BTreeMap;

use super::super::dictionary::Dictionary;
use super::super::item_params::item_params;
use super::static_info::MatchProfile;

/// One player's block in a body.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerBlock<'a> {
    /// The roster place: the bit of the mask.
    pub place: u8,
    /// `player_serialized_state`, without its length.
    pub state: &'a [u8],
    /// The animation state's bytes, if the player has an animation tree.
    pub animation: Option<&'a [u8]>,
}

/// Reads bits most significant first, as the animation encoder writes
/// them (0x3A8980).
struct Bits<'a> {
    bytes: &'a [u8],
    /// In bits from the start.
    at: usize,
}

impl Bits<'_> {
    fn read(&mut self, bits: usize) -> Option<u32> {
        if bits > 32 || self.at + bits > self.bytes.len() * 8 {
            return None;
        }
        let mut value = 0u64;
        for _ in 0..bits {
            let byte = self.bytes[self.at / 8];
            value = value << 1 | u64::from(byte >> (7 - self.at % 8) & 1);
            self.at += 1;
        }
        Some(value as u32)
    }

    fn skip(&mut self, bits: usize) -> Option<()> {
        self.at = self.at.checked_add(bits)?;
        (self.at <= self.bytes.len() * 8).then_some(())
    }
}

/// The bits an index below `count` takes (0x3A961E, 0x396120).
fn width(count: u32) -> usize {
    (32 - count.leading_zeros()).max(1) as usize
}

/// How many bytes the animation state at the start of `bytes` takes
/// (`decode_data` 0x3A9900): 16 bits `T`, the tree part of `T` bits, the
/// times table (0x3A9580), the floats table (0x3A9720), padded to a byte.
/// `None`: the bytes end first.
pub fn animation_len(bytes: &[u8]) -> Option<usize> {
    let mut bits = Bits { bytes, at: 0 };
    let tree = bits.read(16)? as usize;
    bits.skip(tree)?;
    // Times: a count, then either each time or a table of the different
    // ones and an index for each.
    let count = bits.read(10)?;
    if count != 0 {
        let indexed = bits.read(1)? != 0;
        bits.skip(32)?;
        if count > 1 {
            let each = bits.read(5)? as usize;
            if indexed {
                let unique = bits.read(width(count))?;
                bits.skip(unique.saturating_sub(1) as usize * each)?;
                bits.skip(count as usize * width(unique))?;
            } else {
                bits.skip(count as usize * each)?;
            }
        }
    }
    // Floats: the same two forms, 32 bits each.
    let indexed = bits.read(1)? != 0;
    let count = bits.read(10)?;
    if indexed {
        let unique = bits.read(width(count))?;
        bits.skip(unique as usize * 32)?;
        bits.skip(count as usize * width(unique))?;
    } else {
        bits.skip(count as usize * 32)?;
    }
    Some(bits.at.div_ceil(8))
}

/// The player blocks of `body` and where they end (the first section's
/// prefix). `None`: the bytes end inside a block.
pub fn walk(body: &[u8]) -> Option<(Vec<PlayerBlock<'_>>, usize)> {
    let mask = u32::from_le_bytes(body.get(..4)?.try_into().ok()?);
    let mut at = 4;
    let mut blocks = Vec::new();
    for place in 0..32u8 {
        if mask >> place & 1 == 0 {
            continue;
        }
        let len = usize::from(u16::from_le_bytes(body.get(at..at + 2)?.try_into().ok()?));
        let state = body.get(at + 2..at + 2 + len)?;
        at += 2 + len;
        let animated = *body.get(at)? != 0;
        at += 1;
        let animation = if animated {
            let len = animation_len(&body[at..])?;
            let bytes = &body[at..at + len];
            at += len;
            Some(bytes)
        } else {
            None
        };
        blocks.push(PlayerBlock {
            place,
            state,
            animation,
        });
    }
    Some((blocks, at))
}

/// The player blocks of a body whose six sections follow them exactly to
/// its end. `None` for any other bytes.
pub fn blocks(body: &[u8]) -> Option<Vec<PlayerBlock<'_>>> {
    let (blocks, mut at) = walk(body)?;
    for _ in 0..6 {
        let len = usize::from(u16::from_le_bytes(body.get(at..at + 2)?.try_into().ok()?));
        at += 2 + len;
    }
    (at == body.len()).then_some(blocks)
}

/// The fixed head of a player state (`game_world_core_client::serialize`
/// 0x3218E0, `player_profile::serialize_dynamic` 0x2F7CC0,
/// `base_player::serialize` 0x2DAF60; reader 0x2DB490). PROVEN.
#[derive(Clone, Debug, PartialEq)]
pub struct PlayerState {
    /// `current_index`: which of the place's up to three player objects
    /// this is [LIKELY the kit in use: every item's header carries the
    /// owner's kit index too].
    pub current_index: u8,
    pub target_index: u8,
    /// Client patch 85's record: the player's drop and its picked guns.
    pub pickup: weapon_pickup::Record,
    /// One for each item of the profile, in the static info's order:
    /// `amount_in_storage` of a stack item (what was not handed out to a
    /// slot), the `condition` of any other.
    pub items: Vec<u32>,
    /// `m_is_alive`.
    pub alive: bool,
    /// `m_has_to_die`.
    pub has_to_die: bool,
    pub look_pitch: f32,
    pub rotation_y: f32,
    pub actions_mask: u32,
    /// `serializable_effects_count`: the effects follow the head, and
    /// their layout was not traced.
    pub effects: u32,
}

/// Where the native fields start: after the two kit indices, client patch
/// 85 (`weapon_pickup`, written at base_player::serialize's start 0x2DAF70,
/// read at 0x2DB4CF) puts the player's pickup record
/// (`weapon_pickup::Record`: its drop and its picked guns).
pub const FIELDS_AT: usize = 2 + weapon_pickup::Record::BYTES;

impl PlayerState {
    /// The head's length for a profile of `items` items.
    pub fn head_len(items: usize) -> usize {
        FIELDS_AT + 4 * items + 27
    }

    /// Reads the head of the state of a player whose profile has `items`
    /// items. `None`: the state is shorter, a bool is no bool, or the
    /// pickup record is none.
    pub fn read(state: &[u8], items: usize) -> Option<Self> {
        let head = state.get(..Self::head_len(items))?;
        let pickup = weapon_pickup::Record::decode(head[2..FIELDS_AT].try_into().ok()?)?;
        let u32_at =
            |at: usize| u32::from_le_bytes([head[at], head[at + 1], head[at + 2], head[at + 3]]);
        let fixed = FIELDS_AT + 4 * items;
        // The bool writer (0x80FA0) writes 0x00 or 0xFF.
        let flag = |at: usize| match head[at] {
            0 => Some(false),
            0xFF => Some(true),
            _ => None,
        };
        Some(Self {
            current_index: head[0],
            target_index: head[1],
            pickup,
            items: (0..items).map(|item| u32_at(FIELDS_AT + 4 * item)).collect(),
            alive: flag(fixed)?,
            has_to_die: flag(fixed + 1)?,
            look_pitch: f32::from_bits(u32_at(fixed + 3)),
            rotation_y: f32::from_bits(u32_at(fixed + 7)),
            actions_mask: u32_at(fixed + 15),
            effects: u32_at(fixed + 23),
        })
    }
}

/// A magazine larger than this is not a magazine.
const MAGAZINE_MAX: u8 = 200;
/// A weapon's `m_ammo_in_magazine` is this far from its header's start
/// (`weapon_core::serialize` 0x2E3130): the 7 header bytes, a u32 seed, a
/// u32 time, a bool.
const MAGAZINE_AT: usize = 16;

/// An item of the profile as a snapshot has it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ItemLeft {
    pub dict_id: u16,
    /// The dictionary's `is_stack`.
    pub stack: bool,
    /// A stack item: `amount_in_storage`, what the player did not take
    /// into a slot. Any other: the profile's `condition`, in the static
    /// info's unit. PROVEN (the state's head).
    pub stored: u32,
    /// A stack item in a slot of the kit in use: its `m_amount`. `None`:
    /// the kit in use does not hold it, or the inventory was not found.
    /// [LIKELY: found by the header's id bytes]
    pub in_hand: Option<u16>,
    /// Another item in a slot of the kit in use: its `m_condition`, the
    /// inventory's own f32 (its unit against `stored` was not traced).
    pub condition: Option<f32>,
    /// A weapon (an item whose config `type` is `wpn_...`) in a slot of
    /// the kit in use: `m_ammo_in_magazine`. [LIKELY, as `in_hand`; a
    /// chambered round is not seen]
    pub magazine: Option<u8>,
    /// The slot of the kit in use it was found in.
    pub slot: Option<u8>,
}

/// What a player has left, from its block of a snapshot.
#[derive(Clone, Debug, PartialEq)]
pub struct Supplies {
    /// `m_is_alive` when the snapshot was made.
    pub alive: bool,
    /// The kit in use [LIKELY: `current_index`].
    pub kit: u8,
    /// Whether the kit's items were found in the inventory: with `false`
    /// every `in_hand`, `condition` and `magazine` is `None` and says
    /// nothing (a dead player's inventory, or a layout read wrong). For a
    /// player holding a picked gun (client patch 85), the own stacks the
    /// pick set aside are out of the inventory: read as the kit gives them.
    pub inventory_read: bool,
    /// By profile item, in the static info's order.
    pub items: Vec<ItemLeft>,
    /// [A334] The kit slot whose weapon is in hands: `inventory::serialize`'s `m_active_slot` (inventory+0x170), 2 bytes before the
    /// first slot item's record (fx-review A334b, from the exe's write order; LIKELY, not read back on real worlds). Set at the
    /// selection (0x2FCB18), so in a swap it already names the new slot. `None` unless the inventory was read and the byte names a
    /// slot of the kit in use that holds a weapon.
    pub hands_slot: Option<u8>,
    /// [A334] The client's own carried weight `w` (`m_carried_weight`, loaded rounds included), kg: 9 bytes before the first slot
    /// item's record when nothing is carried and no artefact is held (LIKELY, as `hands_slot`). `None` otherwise.
    pub carried_weight: Option<f32>,
    /// [A334] Whether the player carries an item (a battery): the carried object's id 4 bytes before the first slot item's record,
    /// 0xFF for none (LIKELY, as `hands_slot`). `None` unless the inventory was read.
    pub carrying: Option<bool>,
}

impl Supplies {
    /// Reads the block of the player at roster place `place` whose
    /// profile the static info gave as `profile`. `None`: the state is not
    /// one of that profile (too short for its items).
    pub fn read(
        block: &PlayerBlock,
        profile: &MatchProfile,
        dictionary: &Dictionary,
    ) -> Option<Self> {
        let head = PlayerState::read(block.state, profile.items.len())?;
        let mut items: Vec<ItemLeft> = profile
            .items
            .iter()
            .zip(&head.items)
            .map(|(item, stored)| ItemLeft {
                dict_id: item.dict_id,
                stack: dictionary.is_stack(item.dict_id),
                stored: *stored,
                in_hand: None,
                condition: None,
                magazine: None,
                slot: None,
            })
            .collect();
        let mut inventory_read = false;
        // [A334] The first slot item's record: the inventory's own fields just before it.
        let mut first: Option<usize> = None;
        let kit = profile.kits.get(usize::from(head.current_index));
        if let Some(kit) = kit {
            let state = block.state;
            let mut at = PlayerState::head_len(items.len());
            // Items come by ascending slot (0x2FE50A..0x2FE53B), each with
            // `player_id, player_profile_id, m_slot_id` after its amount
            // (u16, a stack) or condition (f32).
            for (slot, held) in kit.slots.iter().enumerate() {
                let Some((index, count)) = *held else {
                    continue;
                };
                let Some(item) = items.get_mut(usize::from(index)) else {
                    continue;
                };
                let ids = [block.place, head.current_index, slot as u8];
                let value = if item.stack { 2 } else { 4 };
                let found = (at..state.len().saturating_sub(value + 2)).find(|start| {
                    state[start + value..start + value + 3] == ids
                        && if item.stack {
                            u16::from_le_bytes([state[*start], state[start + 1]]) <= count
                        } else {
                            let condition = f32::from_le_bytes([
                                state[*start],
                                state[start + 1],
                                state[start + 2],
                                state[start + 3],
                            ]);
                            condition.is_finite() && condition >= 0.0
                        }
                });
                let Some(start) = found else {
                    continue;
                };
                inventory_read = true;
                first.get_or_insert(start);
                at = start + value + 3;
                item.slot = Some(slot as u8);
                if item.stack {
                    item.in_hand = Some(u16::from_le_bytes([state[start], state[start + 1]]));
                } else {
                    item.condition = Some(f32::from_le_bytes([
                        state[start],
                        state[start + 1],
                        state[start + 2],
                        state[start + 3],
                    ]));
                    let is_weapon = item_params(item.dict_id)
                        .is_some_and(|params| params.kind.starts_with("wpn"));
                    if is_weapon {
                        item.magazine = state
                            .get(start + MAGAZINE_AT)
                            .copied()
                            .filter(|rounds| *rounds <= MAGAZINE_MAX);
                    }
                }
            }
            // A pick (patch 85) sets own stacks aside, out of the inventory
            // (its read misses them; a borrowed item's header names its
            // donor's place, so none is read as the own): they are neither
            // charged nor refunded (the owner, 2026-11-02), so a stack of
            // the kit not found is taken as the kit gives it.
            if head.pickup.picks.iter().any(Option::is_some) {
                for (slot, held) in kit.slots.iter().enumerate() {
                    let Some((index, count)) = *held else { continue };
                    if let Some(item) = items.get_mut(usize::from(index)).filter(|item| item.stack && item.in_hand.is_none()) {
                        item.slot = Some(slot as u8);
                        item.in_hand = Some(count);
                        inventory_read = true;
                    }
                }
            }
        }
        let state = block.state;
        let byte = |back: usize| first.and_then(|at| at.checked_sub(back)).and_then(|at| state.get(at).copied());
        let hands_slot = byte(2).filter(|&slot| {
            kit.and_then(|kit| kit.slots.get(usize::from(slot)).copied().flatten())
                .and_then(|(index, _)| profile.items.get(usize::from(index)))
                .is_some_and(|item| item_params(item.dict_id).is_some_and(|params| params.kind.starts_with("wpn")))
        });
        let carrying = byte(4).map(|id| id != 0xFF);
        let carried_weight = match (byte(4), byte(3)) {
            (Some(0xFF), Some(0)) => first
                .and_then(|at| at.checked_sub(9))
                .and_then(|at| state.get(at..at + 4))
                .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                .filter(|w| w.is_finite() && (0.0..200.0).contains(w)),
            _ => None,
        };
        Some(Self {
            alive: head.alive,
            kit: head.current_index,
            inventory_read,
            items,
            hands_slot,
            carried_weight,
            carrying,
        })
    }

    /// What is left of each stack item, by dict id: what is in store,
    /// and, for a player alive with its inventory read, what is in its
    /// slots and its weapons' magazines. A magazine's rounds are counted
    /// to the ammunition in the slot after the weapon's, or the one after
    /// that [LIKELY: a weapon's two ammunition slots follow it, as the
    /// lobby lays a kit out; which of the two is loaded is not read]. A
    /// dead player has what is in store: it takes its slots' counts from
    /// there when it is back (0x2FCC4A).
    pub fn left(&self) -> BTreeMap<u16, u32> {
        let mut left = BTreeMap::new();
        for item in self.items.iter().filter(|item| item.stack) {
            let in_hand = if self.alive {
                u32::from(item.in_hand.unwrap_or(0))
            } else {
                0
            };
            *left.entry(item.dict_id).or_insert(0) += item.stored + in_hand;
        }
        if self.alive {
            for weapon in &self.items {
                let Some(rounds) = weapon.magazine else {
                    continue;
                };
                let Some(slot) = weapon.slot else {
                    continue;
                };
                let ammo = [slot + 1, slot + 2].into_iter().find_map(|slot| {
                    self.items
                        .iter()
                        .find(|item| item.stack && item.slot == Some(slot))
                });
                if let Some(ammo) = ammo {
                    *left.entry(ammo.dict_id).or_insert(0) += u32::from(rounds);
                }
            }
        }
        left
    }
}

/// For tests and tools: writes bits most significant first.
#[derive(Default)]
pub struct BitWriter {
    bytes: Vec<u8>,
    bits: usize,
}

impl BitWriter {
    pub fn put(&mut self, value: u32, bits: usize) {
        for bit in (0..bits).rev() {
            if self.bits.is_multiple_of(8) {
                self.bytes.push(0);
            }
            let last = self.bytes.len() - 1;
            self.bytes[last] |= ((value >> bit & 1) as u8) << (7 - self.bits % 8);
            self.bits += 1;
        }
    }

    pub fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

/// An animation state with a tree part of `tree` bits, `times` and
/// `floats`, each written plainly (not indexed): what [`animation_len`]
/// measures, for tests and tools.
pub fn animation_bytes(tree: &[bool], times: &[u32], floats: &[f32]) -> Vec<u8> {
    let mut out = BitWriter::default();
    out.put(tree.len() as u32, 16);
    for bit in tree {
        out.put(u32::from(*bit), 1);
    }
    out.put(times.len() as u32, 10);
    if !times.is_empty() {
        out.put(0, 1);
        out.put(times[0], 32);
        if times.len() > 1 {
            out.put(20, 5);
            for time in times {
                out.put(time.wrapping_sub(times[0]) & 0xF_FFFF, 20);
            }
        }
    }
    out.put(0, 1);
    out.put(floats.len() as u32, 10);
    for float in floats {
        out.put(float.to_bits(), 32);
    }
    out.finish()
}

/// A player state for tests and tools: the head, `inventory` after it.
pub fn state_bytes(head: &PlayerState, inventory: &[u8]) -> Vec<u8> {
    let flag = |set: bool| if set { 0xFF } else { 0 };
    let mut out = vec![head.current_index, head.target_index];
    out.extend(head.pickup.encode());
    for item in &head.items {
        out.extend(item.to_le_bytes());
    }
    out.extend([flag(head.alive), flag(head.has_to_die), 0]);
    out.extend(head.look_pitch.to_le_bytes());
    out.extend(head.rotation_y.to_le_bytes());
    out.extend([0; 4]);
    out.extend(head.actions_mask.to_le_bytes());
    out.extend([0; 4]);
    out.extend(head.effects.to_le_bytes());
    out.extend_from_slice(inventory);
    out
}

/// A block's bytes: the length, the state, the flag, the animation.
pub fn block_bytes(state: &[u8], animation: Option<&[u8]>) -> Vec<u8> {
    let mut out = (state.len() as u16).to_le_bytes().to_vec();
    out.extend_from_slice(state);
    out.push(if animation.is_some() { 0xFF } else { 0 });
    out.extend_from_slice(animation.unwrap_or(&[]));
    out
}

#[cfg(test)]
mod tests {
    use super::super::static_info::{MatchItem, MatchKit};
    use super::*;

    #[test]
    fn an_animation_states_length_comes_from_its_own_bits() {
        // No tree, no times, no floats: 16 + 10 + 1 + 10 bits, 5 bytes.
        assert_eq!(animation_bytes(&[], &[], &[]), [0, 0, 0, 0, 0]);
        assert_eq!(animation_len(&[0, 0, 0, 0, 0, 0xAA]), Some(5));
        // A tree of 3 bits, one time (its base only), two floats.
        let bytes = animation_bytes(&[true, false, true], &[70_000], &[1.0, -2.5]);
        // 16 + 3 + 10 + 1 + 32 + 1 + 10 + 64 = 137 bits.
        assert_eq!(bytes.len(), 18);
        assert_eq!(bytes[..2], [0, 3]);
        assert_eq!(bytes[2] >> 5, 0b101);
        let mut longer = bytes.clone();
        longer.extend([0xEE; 9]);
        assert_eq!(animation_len(&longer), Some(18));
        // Three times: a 5-bit width, then each.
        let bytes = animation_bytes(&[], &[100, 150, 200], &[]);
        // 16 + 10 + 1 + 32 + 5 + 60 + 1 + 10 = 135 bits.
        assert_eq!(bytes.len(), 17);
        assert_eq!(animation_len(&bytes), Some(17));
        // Cut short: no length.
        assert_eq!(animation_len(&bytes[..16]), None);
        assert_eq!(animation_len(&[]), None);
    }

    #[test]
    fn indexed_tables_are_measured_too() {
        // Times: 4 of them, indexed, 2 different ones of 7 bits; floats: 3,
        // indexed, 1 different one.
        let mut out = BitWriter::default();
        out.put(5, 16);
        out.put(0b10110, 5);
        out.put(4, 10); // count
        out.put(1, 1); // indexed
        out.put(1_000, 32); // base
        out.put(7, 5); // bits each
        out.put(2, 3); // different ones: width(4) = 3 bits
        out.put(0x55, 7); // the second one (the first is the base)
        for index in [0, 1, 1, 0] {
            out.put(index, 2); // width(2) = 2 bits
        }
        out.put(1, 1); // indexed
        out.put(3, 10);
        out.put(1, 2); // different ones: width(3) = 2 bits
        out.put(1.5f32.to_bits(), 32);
        for _ in 0..3 {
            out.put(0, 1); // width(1) = 1 bit
        }
        let bits = 16 + 5 + 10 + 1 + 32 + 5 + 3 + 7 + 8 + 1 + 10 + 2 + 32 + 3;
        let bytes = out.finish();
        assert_eq!(bytes.len(), usize::div_ceil(bits, 8));
        let mut longer = bytes.clone();
        longer.extend([0xFF; 4]);
        assert_eq!(animation_len(&longer), Some(bytes.len()));
    }

    #[test]
    fn the_blocks_are_walked_from_the_front() {
        let animation = animation_bytes(&[true; 11], &[5, 6], &[0.25]);
        let mut body = 0b101u32.to_le_bytes().to_vec();
        body.extend(block_bytes(&[1, 2, 3], Some(&animation)));
        body.extend(block_bytes(&[9; 40], None));
        let players_end = body.len();
        for section in [&[7u8; 3][..], &[], &[1], &[], &[2, 2], &[]] {
            body.extend((section.len() as u16).to_le_bytes());
            body.extend_from_slice(section);
        }
        let (walked, end) = walk(&body).unwrap();
        assert_eq!(end, players_end);
        assert_eq!(
            walked,
            [
                PlayerBlock {
                    place: 0,
                    state: &[1, 2, 3],
                    animation: Some(&animation[..]),
                },
                PlayerBlock {
                    place: 2,
                    state: &[9; 40],
                    animation: None,
                },
            ]
        );
        assert_eq!(blocks(&body).unwrap(), walked);
        // A byte more or less after the sections: not a body.
        assert_eq!(blocks(&body[..body.len() - 1]), None);
        body.push(0);
        assert_eq!(blocks(&body), None);
        // No players: the sections follow the mask.
        assert_eq!(
            blocks(&[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
            Some(Vec::new())
        );
    }

    /// A profile with one kit: an AK-74M (3) in slot 7 with 5.45x39 (7) in
    /// slot 8, RGD-5 grenades (5) in slot 19.
    fn profile() -> MatchProfile {
        let item = |dict_id: u16, condition_or_stack: u32| MatchItem {
            instance_id: u32::from(dict_id),
            dict_id,
            condition_or_stack,
            ..MatchItem::default()
        };
        let mut kit = MatchKit::empty(1, "");
        kit.slots[7] = Some((0, 1));
        kit.slots[8] = Some((1, 120));
        kit.slots[19] = Some((2, 2));
        MatchProfile {
            team: 0,
            name: "tester".to_owned(),
            clan_tag: String::new(),
            protection: 10,
            kits: vec![kit],
            items: vec![item(3, 1_000_000), item(7, 300), item(5, 6)],
            squad: 0,
        }
    }

    fn head(alive: bool) -> PlayerState {
        PlayerState {
            current_index: 0,
            target_index: 0,
            pickup: weapon_pickup::Record::EMPTY,
            // The condition, 180 rounds and 4 grenades in store.
            items: vec![990_000, 180, 4],
            alive,
            has_to_die: false,
            look_pitch: 0.5,
            rotation_y: -1.0,
            actions_mask: 0x21,
            effects: 0,
        }
    }

    /// The inventory of place 4: some bytes, the weapon with 17 rounds in
    /// its magazine, 95 rounds, 1 grenade.
    fn inventory() -> Vec<u8> {
        let mut bytes = vec![0xA5; 40];
        // The weapon: f32 condition, player, kit, slot; seed, time, a bool,
        // the magazine.
        bytes.extend(0.99f32.to_le_bytes());
        bytes.extend([4, 0, 7]);
        bytes.extend([0x11; 8]);
        bytes.push(0);
        bytes.push(17);
        bytes.extend([0x22; 30]);
        // The ammunition and the grenades: u16 amount, player, kit, slot.
        bytes.extend([95, 0, 4, 0, 8]);
        bytes.extend([1, 0, 4, 0, 19]);
        bytes.extend([0x33; 12]);
        bytes
    }

    #[test]
    fn the_head_of_a_state_is_read_byte_for_byte() {
        let state = state_bytes(&head(true), &[]);
        assert_eq!(state.len(), PlayerState::head_len(3));
        assert_eq!(
            state,
            [
                0, 0, // current and target index
                8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // patch 85's record (version 8), empty
                0, 0, 0, 0, 0, 0, 0, 0, // no pick, no own wear
                0, // no drop's recipe
                0x30, 0x1B, 0x0F, 0x00, // the weapon's condition, 990 000
                180, 0, 0, 0, // rounds in store
                4, 0, 0, 0, // grenades in store
                0xFF, 0, 0, // alive, has to die, select animations
                0, 0, 0, 0x3F, // look pitch 0.5
                0, 0, 0x80, 0xBF, // rotation -1
                0, 0, 0, 0, // the input
                0x21, 0, 0, 0, // actions
                0, 0, 0, 0, // the effect player's time
                0, 0, 0, 0, // effects
            ]
        );
        assert_eq!(PlayerState::read(&state, 3), Some(head(true)));
        // Too short for the profile's items, or a bool that is none.
        assert_eq!(PlayerState::read(&state, 4), None);
        let mut wrong = state.clone();
        wrong[FIELDS_AT + 12] = 1;
        assert_eq!(PlayerState::read(&wrong, 3), None);
        // A pickup record that does not decode.
        let mut wrong = state.clone();
        wrong[2] = 0;
        assert_eq!(PlayerState::read(&wrong, 3), None);
    }

    /// [A334] The inventory's own fields before the first slot item's record: the active slot 2 bytes before it, the carried
    /// object's id 4 before (0xFF none), the artefacts 3 before, and the carried weight 9 before when neither is held.
    #[test]
    fn the_hands_slot_the_carrier_and_the_carried_weight_are_read() {
        let dictionary = Dictionary::shared();
        let mut inventory = inventory();
        inventory[40 - 2] = 7;
        inventory[40 - 4] = 0xFF;
        inventory[40 - 3] = 0;
        inventory[40 - 9..40 - 5].copy_from_slice(&23.5f32.to_le_bytes());
        let state = state_bytes(&head(true), &inventory);
        let block = PlayerBlock { place: 4, state: &state, animation: None };
        let supplies = Supplies::read(&block, &profile(), dictionary).unwrap();
        assert_eq!(supplies.hands_slot, Some(7), "the weapon in slot 7");
        assert_eq!(supplies.carrying, Some(false));
        assert_eq!(supplies.carried_weight, Some(23.5));
        // A slot that holds no weapon (the ammunition's 8), a carried item: no hands slot, no weight, carrying.
        inventory[40 - 2] = 8;
        inventory[40 - 4] = 3;
        let state = state_bytes(&head(true), &inventory);
        let block = PlayerBlock { place: 4, state: &state, animation: None };
        let supplies = Supplies::read(&block, &profile(), dictionary).unwrap();
        assert_eq!((supplies.hands_slot, supplies.carrying, supplies.carried_weight), (None, Some(true), None));
    }

    #[test]
    fn what_a_player_has_left_is_its_store_its_slots_and_its_magazine() {
        let dictionary = Dictionary::shared();
        let state = state_bytes(&head(true), &inventory());
        let block = PlayerBlock {
            place: 4,
            state: &state,
            animation: None,
        };
        let supplies = Supplies::read(&block, &profile(), dictionary).unwrap();
        assert!(supplies.alive && supplies.inventory_read);
        assert_eq!(supplies.kit, 0);
        assert_eq!(
            supplies.items,
            [
                ItemLeft {
                    dict_id: 3,
                    stack: false,
                    stored: 990_000,
                    in_hand: None,
                    condition: Some(0.99),
                    magazine: Some(17),
                    slot: Some(7),
                },
                ItemLeft {
                    dict_id: 7,
                    stack: true,
                    stored: 180,
                    in_hand: Some(95),
                    condition: None,
                    magazine: None,
                    slot: Some(8),
                },
                ItemLeft {
                    dict_id: 5,
                    stack: true,
                    stored: 4,
                    in_hand: Some(1),
                    condition: None,
                    magazine: None,
                    slot: Some(19),
                },
            ]
        );
        // 180 + 95 + 17 rounds, 4 + 1 grenades.
        assert_eq!(supplies.left(), BTreeMap::from([(7, 292), (5, 5)]));

        // Dead: what is in store is what is left.
        let state = state_bytes(&head(false), &inventory());
        let block = PlayerBlock {
            place: 4,
            state: &state,
            animation: None,
        };
        let supplies = Supplies::read(&block, &profile(), dictionary).unwrap();
        assert!(!supplies.alive);
        assert_eq!(supplies.left(), BTreeMap::from([(7, 180), (5, 4)]));

        // Another place's block: the headers are not this player's.
        let block = PlayerBlock {
            place: 5,
            state: &state,
            animation: None,
        };
        let supplies = Supplies::read(&block, &profile(), dictionary).unwrap();
        assert!(!supplies.inventory_read);
        assert!(supplies.items.iter().all(|item| item.in_hand.is_none()));
        // Holding a borrowed gun (patch 85): the kit's items found are read
        // as found (here none set aside); a stack set aside is not (below,
        // `a_borrowing_players_set_aside_ammunition_is_not_charged`).
        let recipe = weapon_pickup::Recipe {
            place: weapon_pickup::Place::new(2).unwrap(),
            kit: weapon_pickup::Kit::new(1).unwrap(),
            slot: weapon_pickup::WeaponSlot::Second,
        };
        let pick = weapon_pickup::Pick { taken: weapon_pickup::Ms(5_000), recipe };
        let held = weapon_pickup::Held { pick, own: weapon_pickup::Wear::of(0.9) };
        let pickup = weapon_pickup::Record { picks: [Some(held), None], ..weapon_pickup::Record::EMPTY };
        let state = state_bytes(&PlayerState { pickup, ..head(true) }, &inventory());
        let block = PlayerBlock {
            place: 4,
            state: &state,
            animation: None,
        };
        let supplies = Supplies::read(&block, &profile(), dictionary).unwrap();
        assert!(supplies.alive && supplies.inventory_read);
        let own = |dict_id: u16| supplies.items.iter().find(|item| item.dict_id == dict_id).unwrap().in_hand;
        assert_eq!((own(7), own(5)), (Some(95), Some(1)));
        // A state too short for the profile is not read at all.
        let block = PlayerBlock {
            place: 4,
            state: &state[..20],
            animation: None,
        };
        assert_eq!(Supplies::read(&block, &profile(), dictionary), None);
    }

    /// The owner's case (2026-11-01): the AK kit of place 4 took a PKP
    /// (patch 85), fired 300 rounds with it, and the match ends with it in
    /// hand. Its inventory holds the borrowed PKP and its ammunition, whose
    /// headers name their donor's place (2); its own AK and its 5.45 are set
    /// aside, neither charged nor refunded (2026-11-02): the 5.45 is taken as
    /// the kit gives it (120), nothing of the 300 rounds is its.
    #[test]
    fn a_borrowing_players_set_aside_ammunition_is_not_charged() {
        let dictionary = Dictionary::shared();
        let mut inventory = vec![0xA5; 40];
        // The borrowed PKP: f32 condition, donor 2, kit 1, slot 7, then 5 in its magazine.
        inventory.extend(0.8f32.to_le_bytes());
        inventory.extend([2, 1, 7]);
        inventory.extend([0x11; 8]);
        inventory.push(0);
        inventory.push(5);
        inventory.extend([0x22; 30]);
        // Its 7N13 in slot 8 (its donor's header), the own grenade in slot 19.
        inventory.extend([100, 0, 2, 1, 8]);
        inventory.extend([1, 0, 4, 0, 19]);
        inventory.extend([0x33; 12]);
        let recipe = weapon_pickup::Recipe {
            place: weapon_pickup::Place::new(2).unwrap(),
            kit: weapon_pickup::Kit::new(1).unwrap(),
            slot: weapon_pickup::WeaponSlot::First,
        };
        let pick = weapon_pickup::Pick { taken: weapon_pickup::Ms(5_000), recipe };
        let pickup = weapon_pickup::Record {
            picks: [Some(weapon_pickup::Held { pick, own: weapon_pickup::Wear::of(0.9) }), None],
            ..weapon_pickup::Record::EMPTY
        };
        let state = state_bytes(&PlayerState { pickup, ..head(true) }, &inventory);
        let block = PlayerBlock {
            place: 4,
            state: &state,
            animation: None,
        };
        let supplies = Supplies::read(&block, &profile(), dictionary).unwrap();
        assert!(supplies.alive && supplies.inventory_read);
        let own = |dict_id: u16| *supplies.items.iter().find(|item| item.dict_id == dict_id).unwrap();
        assert_eq!((own(7).slot, own(7).in_hand), (Some(8), Some(120)), "the set-aside 5.45 as the kit gives it");
        assert_eq!(own(5).in_hand, Some(1));
        // 180 in store + the kit's 120; 4 + 1 grenades.
        assert_eq!(supplies.left(), BTreeMap::from([(7, 300), (5, 5)]));
    }

    #[test]
    fn an_amount_over_the_slots_count_is_no_header() {
        let dictionary = Dictionary::shared();
        // 200 "rounds" before the id bytes of slot 8, which holds 120 at
        // most: skipped; the real header comes later.
        let mut inventory = vec![200, 0, 4, 0, 8];
        inventory.extend([60, 0, 4, 0, 8]);
        let state = state_bytes(&head(true), &inventory);
        let block = PlayerBlock {
            place: 4,
            state: &state,
            animation: None,
        };
        let supplies = Supplies::read(&block, &profile(), dictionary).unwrap();
        assert_eq!(supplies.items[1].in_hand, Some(60));
        assert_eq!(supplies.items[0].magazine, None);
    }
}

/// The client patch revision from which a state ends with client patch 78's
/// carry snapshot (`motion`'s `native_state`).
pub const CARRY_REVISION: u16 = 38;
/// The client patch revision from which a state has client patch 85's pickup
/// record after the two kit indices: version 4 (48 bytes) in revision 41.
pub const PICKUP_REVISION: u16 = 41;
/// The client patch revision from which the pickup record is today's
/// (`weapon_pickup::Record`, version 8, 29 bytes).
pub const PICKUP_V8_REVISION: u16 = 42;
/// The bytes of revision 41's pickup record (version 4: 20 bytes and two
/// parked kits of 2 + 3 x 4).
const PICKUP_V4_BYTES: usize = 48;

/// A state that a client of patch revision `revision` wrote, in the layout
/// this server reads ([`PlayerState::read`], `Motion::read`): for recordings,
/// saved worlds and fixtures made before (A218, the owner's of 2026-11-01:
/// read them too).
/// - Before revision 41 the state has no pickup record: the empty one goes
///   after the two kit indices.
/// - Revision 41's record (version 4) is replaced by the empty one: its
///   picks are not read.
/// - Before revision 38 there is no carry snapshot either: a new carry's
///   goes at the end.
///
/// From revision 42 on the state is as it is. `None`: a state too short for
/// its kit indices, or for revision 41's record.
pub fn state_at_revision(state: &[u8], revision: u16) -> Option<std::borrow::Cow<'_, [u8]>> {
    use artefact_carry::carry::Carry;
    use std::borrow::Cow;
    if revision >= PICKUP_V8_REVISION {
        return Some(Cow::Borrowed(state));
    }
    let skip = if revision >= PICKUP_REVISION { PICKUP_V4_BYTES } else { 0 };
    let rest = state.get(2 + skip..)?;
    let mut out = Vec::with_capacity(state.len() + weapon_pickup::Record::BYTES + Carry::SNAPSHOT_SIZE);
    out.extend_from_slice(&state[..2]);
    out.extend(weapon_pickup::Record::EMPTY.encode());
    out.extend_from_slice(rest);
    if revision < CARRY_REVISION {
        out.extend(Carry::NEW.snapshot());
    }
    Some(Cow::Owned(out))
}

#[cfg(test)]
mod revision_tests {
    use super::super::motion::Motion;
    use super::*;

    fn today() -> Vec<u8> {
        let head = PlayerState {
            current_index: 1,
            target_index: 1,
            pickup: weapon_pickup::Record::EMPTY,
            items: vec![990_000, 180, 4],
            alive: true,
            has_to_die: false,
            look_pitch: 0.25,
            rotation_y: -2.0,
            actions_mask: 0xa,
            effects: 0,
        };
        state_bytes(&head, &[0xA5; 40])
    }

    #[test]
    fn a_state_of_revision_42_or_later_is_read_as_it_is() {
        let state = today();
        for revision in [42, 48, 60] {
            assert_eq!(state_at_revision(&state, revision).unwrap().as_ref(), &state[..]);
        }
    }

    #[test]
    fn a_state_before_revision_41_gets_the_empty_pickup_record() {
        let state = today();
        let mut old = state[..2].to_vec();
        old.extend_from_slice(&state[FIELDS_AT..]);
        for revision in [38, 40] {
            let now = state_at_revision(&old, revision).unwrap();
            assert_eq!(now.as_ref(), &state[..], "{revision}");
            assert_eq!(PlayerState::read(&now, 3).unwrap().items, vec![990_000, 180, 4]);
        }
        assert_eq!(PlayerState::read(&old, 3), None, "main's reader refuses it as it is");
    }

    #[test]
    fn revision_41s_record_is_replaced_by_the_empty_one() {
        let state = today();
        let mut old = state[..2].to_vec();
        let mut v4 = [0u8; PICKUP_V4_BYTES];
        v4[0] = 4;
        old.extend_from_slice(&v4);
        old.extend_from_slice(&state[FIELDS_AT..]);
        assert_eq!(state_at_revision(&old, 41).unwrap().as_ref(), &state[..]);
        assert_eq!(state_at_revision(&[1, 1, 4], 41), None, "too short for its record");
    }

    #[test]
    fn a_state_before_revision_38_gets_a_carry_snapshot_too() {
        use artefact_carry::carry::Carry;
        let state = today();
        let mut old = state[..2].to_vec();
        old.extend_from_slice(&state[FIELDS_AT..]);
        let now = state_at_revision(&old, 34).unwrap();
        assert_eq!(&now[..state.len()], &state[..]);
        assert_eq!(&now[state.len()..], &Carry::NEW.snapshot()[..]);
    }

    /// A world a live client uploaded with client patch 78 (rev 38): School,
    /// day 4, match 4758f64b at 203,850 ms, 20 places, 1 human and 19 server
    /// bots, all alive (A154, e28746ead). Read through [`state_at_revision`],
    /// every body of it reads under today's layout, as it did then.
    #[test]
    fn every_body_of_a_live_rev_38_world_reads_at_its_revision() {
        const WORLD: &[u8] =
            include_bytes!("../../../../../resources/worlds-026g/school_rev38_4758f64b_203850.bin");
        const TIME_MS: u32 = 203_850;
        let (blocks, _) = walk(WORLD).expect("the world's player blocks");
        assert_eq!(blocks.len(), 20);
        let bodies: Vec<(u8, Motion)> = blocks
            .iter()
            .filter_map(|block| {
                let state = state_at_revision(block.state, 38)?;
                Motion::find(&state, TIME_MS).map(|(_, motion)| (block.place, motion))
            })
            .filter(|(_, motion)| motion.body.is_some())
            .collect();
        assert!(bodies.len() * 100 >= blocks.len() * 99, "{} of {} blocks read with a body", bodies.len(), blocks.len());
        // Without it, today's reader finds none.
        assert!(blocks.iter().all(|block| Motion::find(block.state, TIME_MS).is_none()));
        // Place 1, as the recording and the snapshot table read it: feet
        // (48.022, 15.606, 117.538), yaw -2.1051, keys 0xa.
        let (_, one) = bodies.iter().find(|(place, _)| *place == 1).expect("place 1");
        let feet = one.body.as_ref().unwrap().feet();
        assert!(
            (feet[0] - 48.022).abs() < 0.01 && (feet[1] - 15.606).abs() < 0.01 && (feet[2] - 117.538).abs() < 0.01,
            "{feet:?}"
        );
        assert!((one.rotation_y + 2.1051).abs() < 1e-3 && one.actions_mask == 0xa && one.alive);
    }
}
