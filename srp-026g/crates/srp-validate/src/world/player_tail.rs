//! A player's state (`base_player::deserialize` 0x2DB490, writer
//! `base_player::serialize` 0x2DAF60; snapshot_rules.md sections 7.3 and
//! 7.4, world_validation.md section 10), read into zero-copy views: the
//! head ([`Head`]), the state to the inventory header ([`PlayerState`]),
//! a carried battery ([`CarriedBattery`]); the items and everything after
//! them are [`super::inventory`]'s, a weapon's and the portable object
//! [`super::weapon`]'s.
//!
//! Each view's reader takes its record's bytes once and turns every field
//! with a limited set of values into its type, refusing the world when it
//! cannot; its accessors return those types and cannot fail. The rules
//! that need the match ([`player_head`], [`player_tail`]) are checked over
//! the parsed values.
//!
//! The names are the 0.26g writers' own: each serialize passes the member's
//! C++ expression to its assert (e.g. `m_is_alive` 0xA1071C), checked
//! against the 0.21d PDB's members. PROVEN unless marked.
//!
//! FIELD MAP
//!
//! ```text
//! head ([`Head`], `PlayerState` in the server; 4M + 27 bytes past the
//! pickup record)
//!   u8   current_index        the kit in use, < 3 (game_world_core_client::serialize 0x3218E0)
//!   u8   target_index         the kit of the next spawn, < 3
//!   29   weapon_pickup::Record (client patch 85)
//!   M*4  each profile item's amount_in_storage (a stack) or condition
//!        (player_profile::serialize_dynamic 0x2F7CC0)
//!   bool m_is_alive, bool m_has_to_die, bool m_need_to_select_animations
//!   f32  m_look_pitch, f32 m_rotation_y
//!   4    the player input's math::half2(rotation), u32 actions_mask
//!   u32  the game effect player's m_current_time_in_ms
//!   u32  serializable_effects_count E
//! E effects ([`Effect`], 67 bytes each)
//! u32 m_effects.size() <= 3, the hit effects ([`HitEffect`],
//!   hit_player_game_effect_presenter::serialize 0x3402C0)
//! u8 m_current_active_object, u8 m_target_active_object: 0xFF none, else
//!   an entry of the 255-entry table at [[player+0x120]+0x18B4C]
//!   (0x2CFFF4; used after a null check 0x2DB671, so any byte is safe)
//! the stamina ([`Stamina`], player_stamina::serialize 0x329310, 29 bytes)
//! the inventory header ([`InventoryHeader`], inventory::serialize 0x2FE360)
//!   f32  m_carried_weight, bool m_need_to_recalculate_weight
//!   u8   m_carried_item's id: 0xFF none, else a battery of the same
//!        255-entry table (used after a null check 0x2FE5C8), then its own
//!        state ([`CarriedBattery`])
//!   u8   m_artefact_slots_count A; when A: u8 slots_begin (a quick slot
//!        13..18, A at most 19 - slots_begin: setup_from_profile
//!        0x2FD1C3..0x2FD2A5) and A artefact ids, each 0xFF or an index of
//!        the artefacts table [[inventory+0x108]+0x18940] (0x2FE655, no
//!        null check: bounded)
//!   u8   m_active_slot, u8 m_grenade_slot: a slot 0..22 or 23 none (every
//!        setter, inventory::action 0x2FCB18, grenade_set_core::action
//!        0x309DC0; stored with no lookup, 0x2FE788, 0x2FE79D)
//! the items and the rest ([`super::inventory`])
//! ```
//!
//! After the inventory header come the kit's items in slot order, and
//! everything after them, which [`super::inventory`] reads to the state's
//! exact end against the match's kits ([`Context::inventory`](super::Context)):
//! the slots the header's artefacts took hold them, not the kit's; a pick
//! (the head's `weapon_pickup::Record`) puts its recipe's gun in its
//! weapon slot and may fill the picker's ammo slots.
//!
//! Without the kits (a tool, the game's own check) the walk stops at the
//! inventory header.

use super::AMMO_SLOTS;
use super::effects::{Effect, EffectFault};
use super::inventory::{
    self, Ammo, Body, Fault, Gun, Item, ItemsAndRest, Measure, Owner, SLOTS, Slot, SlotId, flag,
    float, floats, short, state_id, u8, u32,
};
use super::weapon::{Portable, optional_u8, optional_u32};
use super::{Block, Context, PLAYER_FIELDS_AT, PLAYER_STATE_CAP};
use crate::reader::Reader;
use crate::{Finite, Part, Reason, Refusal};
use weapon_pickup::{Kit, Place, WeaponSlot};

/// The inventory's slots, which bound `slots_begin + A`
/// (`inventory::deserialize` loop count 0x17, 0x2FE7A9).
const INVENTORY_SLOTS: usize = 23;
/// The quick slots artefacts take (setup_from_profile 0x2FD1C3..0x2FD2A5):
/// `slots_begin` is one of them, `slots_begin + A` at most their end.
const QUICK_SLOTS: core::ops::Range<u8> = 13..19;
/// The most player effects a view holds: as many as a state has room for
/// (the writer has no cap; each is 67 bytes).
pub const MAX_PLAYER_EFFECTS: usize = PLAYER_STATE_CAP / Effect::LEN;
/// The hit presenter's effects: a `buffer_vector` over 3 entries (its
/// constructor 0x2D6D1F), which the reader gathers into a 3-entry buffer
/// whose push asserts past it (0x3406B0).
pub const MAX_HIT_EFFECTS: usize = 3;
/// The most items a profile's table holds (`MAX_ITEMS`, static_info.md):
/// the item count a guessed walk tries up to.
const MAX_ITEMS: usize = 69;
/// The head's bytes past the items: three bools, two f32, the input's 8
/// bytes, the effect time and the effect count.
const HEAD_AFTER_ITEMS: usize = 27;

/// A field outside its domain.
fn outside(at: usize, field: &'static str, value: u32) -> Fault {
    (at, Reason::TailField { at, field, value })
}

/// What every head starts with, whatever its item count: the current and
/// target kits and the pickup record (decoded once for all the counts a
/// guessed walk tries).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Front(Kit, Kit, weapon_pickup::Record);

/// The head of a player's state, borrowed from the world.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Head<'a> {
    current_kit: Kit,
    target_kit: Kit,
    pickup: weapon_pickup::Record,
    stored: &'a [[u8; 4]],
    alive: bool,
    has_to_die: bool,
    need_to_select_animations: bool,
    look_pitch: Finite,
    rotation_y: Finite,
    input_rotation: [u16; 2],
    actions_mask: u32,
    effects_time_ms: u32,
    effect_count: u32,
}

impl<'a> Head<'a> {
    /// `current_index`: the kit in use, whose inventory the state saves.
    pub fn current_kit(&self) -> Kit {
        self.current_kit
    }

    /// `target_index`: the kit of the next spawn.
    pub fn target_kit(&self) -> Kit {
        self.target_kit
    }

    /// Client patch 85's weapon-pickup record.
    pub fn pickup(&self) -> weapon_pickup::Record {
        self.pickup
    }

    /// For each item of the place's profile, `items[i].amount_in_storage`
    /// (a stack) or `items[i].condition`, as the game saves them (both
    /// u32).
    pub fn stored(&self) -> impl ExactSizeIterator<Item = u32> + 'a {
        self.stored.iter().map(|word| u32::from_le_bytes(*word))
    }

    /// `m_is_alive`.
    pub fn alive(&self) -> bool {
        self.alive
    }

    /// `m_has_to_die`.
    pub fn has_to_die(&self) -> bool {
        self.has_to_die
    }

    /// `m_need_to_select_animations`.
    pub fn need_to_select_animations(&self) -> bool {
        self.need_to_select_animations
    }

    /// `m_look_pitch` (LIKELY in [-1, 1]: the input path clamps it,
    /// 0x2E1006; not held here).
    pub fn look_pitch(&self) -> Finite {
        self.look_pitch
    }

    /// `m_rotation_y`.
    pub fn rotation_y(&self) -> Finite {
        self.rotation_y
    }

    /// The last input's `math::half2(rotation)`: two IEEE halves, the yaw
    /// and pitch turn (player_input+0x32C, as the input carried them).
    pub fn input_rotation(&self) -> [u16; 2] {
        self.input_rotation
    }

    /// The last input's `actions_mask` (player+0x330).
    pub fn actions_mask(&self) -> u32 {
        self.actions_mask
    }

    /// The game effect player's `m_current_time_in_ms` (0x2DDAD0).
    pub fn effects_time_ms(&self) -> u32 {
        self.effects_time_ms
    }

    /// `serializable_effects_count` E, the player effects after the head.
    pub fn effect_count(&self) -> u32 {
        self.effect_count
    }

    /// The head's length for a profile of `items` items.
    fn len(items: usize) -> Option<usize> {
        items
            .checked_mul(4)?
            .checked_add(PLAYER_FIELDS_AT)?
            .checked_add(HEAD_AFTER_ITEMS)
    }

    /// The two kit bytes and the pickup record, which every head starts
    /// with whatever its item count.
    fn front(state: &[u8]) -> Result<Front, Fault> {
        let (current, target) = match state.get(..2) {
            Some(&[current, target]) => (current, target),
            _ => return Err((0, Reason::StateUnder2)),
        };
        let (Some(current), Some(target)) = (Kit::new(current), Kit::new(target)) else {
            return Err((0, Reason::KitByteOver));
        };
        let pickup = state
            .get(2..PLAYER_FIELDS_AT)
            .and_then(|bytes| bytes.try_into().ok())
            .and_then(weapon_pickup::Record::decode)
            .ok_or((2, Reason::PickupRecord))?;
        Ok(Front(current, target, pickup))
    }

    /// The head of `state` for a profile of `items` items, its offsets the
    /// state's; `Err` with [`Reason::NoHead`] when the state is shorter.
    fn read(
        reader: &mut Reader<'a>,
        state: &'a [u8],
        front: Front,
        items: usize,
    ) -> Result<Self, Fault> {
        let no_head = (
            0,
            Reason::NoHead {
                items,
                len: state.len(),
            },
        );
        let Front(current_kit, target_kit, pickup) = front;
        let len = Self::len(items).ok_or(no_head)?;
        if state.len() < len {
            return Err(no_head);
        }
        reader.skip(PLAYER_FIELDS_AT).map_err(|_| no_head)?;
        let stored = reader
            .take(items.saturating_mul(4))
            .map_err(|_| no_head)?
            .as_chunks::<4>()
            .0;
        let (Some(alive), Some(has_to_die)) = (
            reader.bool().map_err(|_| no_head)?,
            reader.bool().map_err(|_| no_head)?,
        ) else {
            return Err(no_head);
        };
        let select_at = reader.at();
        let need_to_select_animations = reader
            .bool()
            .map_err(|_| no_head)?
            .ok_or((select_at, Reason::SelectAnimations))?;
        let look_at = reader.at();
        let [look_pitch, rotation_y] =
            floats(reader).map_err(|_| (look_at, Reason::LookNotNumber))?;
        let input_rotation = [
            reader.u16().map_err(|_| no_head)?,
            reader.u16().map_err(|_| no_head)?,
        ];
        let actions_mask = u32(reader).map_err(|_| no_head)?;
        let effects_time_ms = u32(reader).map_err(|_| no_head)?;
        let effect_count = u32(reader).map_err(|_| no_head)?;
        Ok(Self {
            current_kit,
            target_kit,
            pickup,
            stored,
            alive,
            has_to_die,
            need_to_select_animations,
            look_pitch,
            rotation_y,
            input_rotation,
            actions_mask,
            effects_time_ms,
            effect_count,
        })
    }
}

/// An existing hit effect's root emitter (`game_effect_ptr_wrapper`): its
/// registry index (the pointer flag `is_client_effect` 0, as the effects
/// template's), held to 0 as the effects template's is, and
/// `m_single_effect_index` (stored, not an index the loader follows).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HitEmitter {
    registry_index: u32,
    single_effect_index: Option<u32>,
}

impl HitEmitter {
    /// `m_emitter->m_registry_index`.
    pub fn registry_index(&self) -> u32 {
        self.registry_index
    }

    /// `m_single_effect_index`, `None` for 0xFFFFFFFF.
    pub fn single_effect_index(&self) -> Option<u32> {
        self.single_effect_index
    }
}

/// A hit effect (`m_effects[i]` of `hit_player_game_effect_presenter`,
/// element 0x14 bytes, 0x340347..0x3403CE).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HitEffect {
    emitter: Option<HitEmitter>,
    last_hit_time: Finite,
    time: Finite,
}

impl HitEffect {
    /// Its emitter, when `is_exists`.
    pub fn emitter(&self) -> Option<HitEmitter> {
        self.emitter
    }

    /// `last_hit_time` (elem+0x10). LIKELY in [0, `time`]: the reader's
    /// loop 0x3405C0 steps it by 0.1 to `time`, one hit each; not held.
    pub fn last_hit_time(&self) -> Finite {
        self.last_hit_time
    }

    /// `time` (elem+0xC).
    pub fn time(&self) -> Finite {
        self.time
    }

    fn read(reader: &mut Reader) -> Result<Self, Fault> {
        let start = reader.at();
        let emitter = match reader.bool().map_err(short)? {
            None => return Err((start, Reason::TailHitEffect { at: start })),
            Some(true) => {
                // `is_client_effect`: 0 (an index, as the effects template);
                // 0xFF would be a raw address.
                if u8(reader)? != 0 {
                    let flag_at = start.saturating_add(1);
                    return Err((flag_at, Reason::TailHitEffect { at: flag_at }));
                }
                // The root emitter's registry index names an entry of an
                // unbounded global table (0xAA2E48) that the native hit-effect
                // loader reads and ref-counts through with NO bound:
                // `hit_player_game_effect_presenter::deserialize` 0x340410,
                // when `is_exists`, calls `game_effect_ptr_wrapper` 0x2E5C30,
                // which with the flag clear does `[0xAA2E48 + index*4]` (an
                // out-of-bounds read for any index past the table) and then a
                // refcount `lock xadd [ptr+0xD0]` through the loaded pointer (a
                // write through a bogus pointer). This is the same unbounded
                // read the effects section and the player E-effect records
                // force to 0 (`world::Effect::read` RootIndex,
                // `effects::record_is_safe`); the hit-effect emitter was the
                // one copy of it left open. Hold it to 0 too, as the effects
                // template is: a non-zero index is refused, not loaded.
                let index_at = reader.at();
                let registry_index = u32(reader)?;
                if registry_index != 0 {
                    return Err((index_at, Reason::TailHitEffect { at: index_at }));
                }
                Some(HitEmitter {
                    registry_index,
                    single_effect_index: optional_u32(reader)?,
                })
            }
            Some(false) => None,
        };
        Ok(Self {
            emitter,
            last_hit_time: float(reader)?,
            time: float(reader)?,
        })
    }
}

/// The stamina (`player_stamina::deserialize` 0x329400, writer 0x329310).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stamina {
    value: Finite,
    current_time_ms: u32,
    last_spending_time_ms: u32,
    last_boost_time_ms: u32,
    previous_boost_time_ms: u32,
    is_low_stamina: bool,
    spending_speed_modifier: Finite,
    movement_speed_modifier: Finite,
}

impl Stamina {
    /// `m_value` (LIKELY in [0, the config's max]; not held).
    pub fn value(&self) -> Finite {
        self.value
    }

    /// `m_current_time_in_ms`.
    pub fn current_time_ms(&self) -> u32 {
        self.current_time_ms
    }

    /// `m_last_spending_time_in_ms`.
    pub fn last_spending_time_ms(&self) -> u32 {
        self.last_spending_time_ms
    }

    /// `m_last_boost_time_in_ms`.
    pub fn last_boost_time_ms(&self) -> u32 {
        self.last_boost_time_ms
    }

    /// `m_previous_boost_time_in_ms`.
    pub fn previous_boost_time_ms(&self) -> u32 {
        self.previous_boost_time_ms
    }

    /// `m_is_low_stamina`.
    pub fn is_low_stamina(&self) -> bool {
        self.is_low_stamina
    }

    /// `m_spending_speed_modifier.modifier_value()`.
    pub fn spending_speed_modifier(&self) -> Finite {
        self.spending_speed_modifier
    }

    /// `m_movement_speed_modifier.modifier_value()`.
    pub fn movement_speed_modifier(&self) -> Finite {
        self.movement_speed_modifier
    }

    fn read(reader: &mut Reader) -> Result<Self, Fault> {
        let value = float(reader)?;
        let current_time_ms = u32(reader)?;
        let last_spending_time_ms = u32(reader)?;
        let last_boost_time_ms = u32(reader)?;
        let previous_boost_time_ms = u32(reader)?;
        let at = reader.at();
        let is_low_stamina = reader
            .bool()
            .map_err(short)?
            .ok_or((at, Reason::TailStaminaBool))?;
        Ok(Self {
            value,
            current_time_ms,
            last_spending_time_ms,
            last_boost_time_ms,
            previous_boost_time_ms,
            is_low_stamina,
            spending_speed_modifier: float(reader)?,
            movement_speed_modifier: float(reader)?,
        })
    }
}

/// The view a battery's take or drop waits for the animation of
/// (`m_index_of_animation_to_wait`: 0 first person, 2 third person, 0xFF
/// none, the constructor's).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BatteryAnimation {
    FirstPerson,
    ThirdPerson,
}

/// A battery's state machine's state (`victory_item_core`'s three:
/// `victory_item_core_take_state`, `_carry_state`, `_drop_state`; an id
/// past them crashes the reader), with its base state's fields and, for
/// take and drop, the animation it waits for
/// (`victory_item_core_animation_end_aware_state` 0x344870).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BatteryState {
    Take {
        motion: BatteryMotion,
        animation: Option<BatteryAnimation>,
    },
    Carry {
        motion: BatteryMotion,
    },
    Drop {
        motion: BatteryMotion,
        animation: Option<BatteryAnimation>,
    },
}

impl BatteryState {
    pub fn motion(&self) -> BatteryMotion {
        match *self {
            BatteryState::Take { motion, .. }
            | BatteryState::Carry { motion }
            | BatteryState::Drop { motion, .. } => motion,
        }
    }
}

/// A battery state's base (`victory_item_core_base_state` 0x344D70).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BatteryMotion {
    animation_has_been_ended: bool,
    last_angular_speed: [Finite; 3],
    last_linear_speed: [Finite; 3],
    last_position: [Finite; 16],
    last_rotation: [Finite; 16],
    last_time_ms: u32,
}

impl BatteryMotion {
    /// `m_animation_has_been_ended`.
    pub fn animation_has_been_ended(&self) -> bool {
        self.animation_has_been_ended
    }

    /// `m_last_angular_speed`.
    pub fn last_angular_speed(&self) -> [Finite; 3] {
        self.last_angular_speed
    }

    /// `m_last_linear_speed`.
    pub fn last_linear_speed(&self) -> [Finite; 3] {
        self.last_linear_speed
    }

    /// `m_last_position`: the item's transform at the state's start.
    pub fn last_position(&self) -> [Finite; 16] {
        self.last_position
    }

    /// `m_last_rotation`.
    pub fn last_rotation(&self) -> [Finite; 16] {
        self.last_rotation
    }

    /// `m_last_time`.
    pub fn last_time_ms(&self) -> u32 {
        self.last_time_ms
    }

    fn read(reader: &mut Reader) -> Result<Self, Fault> {
        Ok(Self {
            animation_has_been_ended: flag(reader)?,
            last_angular_speed: floats(reader)?,
            last_linear_speed: floats(reader)?,
            last_position: floats(reader)?,
            last_rotation: floats(reader)?,
            last_time_ms: u32(reader)?,
        })
    }

    /// A take's or a drop's animation: 0, 2 or 0xFF (PROVEN, its stores).
    fn animation(reader: &mut Reader) -> Result<Option<BatteryAnimation>, Fault> {
        let at = reader.at();
        Ok(match u8(reader)? {
            0 => Some(BatteryAnimation::FirstPerson),
            2 => Some(BatteryAnimation::ThirdPerson),
            0xFF => None,
            value => {
                return Err(outside(
                    at,
                    "a battery state's m_index_of_animation_to_wait",
                    value.into(),
                ));
            }
        })
    }
}

/// A battery mode's two containers (`maps::ProjectObjects`: one of each
/// team).
const BATTERY_CONTAINERS: u8 = 2;

/// A carried battery (`victory_item_core::deserialize` 0x301660, writer
/// 0x301470): its object id, `m_spotted_mask`, its user, its container,
/// its state machine's state, then its portable object.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CarriedBattery {
    at: usize,
    id: u8,
    spotted_mask: u32,
    user: Place,
    container: Option<u8>,
    state: BatteryState,
    portable: Portable,
}

impl CarriedBattery {
    /// Where its record (past the id) starts in the state.
    pub fn at(&self) -> usize {
        self.at
    }

    /// Its entry of the active-objects table.
    pub fn id(&self) -> u8 {
        self.id
    }

    /// `m_spotted_mask`: bit 0 spotted by team 0, bit 1 by the other
    /// (LIKELY, 0x301C40; not held).
    pub fn spotted_mask(&self) -> u32 {
        self.spotted_mask
    }

    /// `m_user->id`: the player who carries it (0xFF, no user, is the free
    /// battery's layout, never a carried one's).
    pub fn user(&self) -> Place {
        self.user
    }

    /// Its container (an index of the gather rule's `m_containers`), `None`
    /// for 0xFF.
    pub fn container(&self) -> Option<u8> {
        self.container
    }

    pub fn state(&self) -> BatteryState {
        self.state
    }

    pub fn portable(&self) -> Portable {
        self.portable
    }

    fn read(reader: &mut Reader, id: u8) -> Result<Self, Fault> {
        let record_at = reader.at();
        let spotted_mask = u32(reader)?;
        let at = reader.at();
        let user = u8(reader)?;
        let user = Place::new(user).ok_or((at, Reason::TailBattery { at }))?;
        // The container: 0xFF, or an **unbounded** index into the gather
        // rule's containers (0x3016AE).
        let at = reader.at();
        let container = match u8(reader)? {
            0xFF => None,
            index if index < BATTERY_CONTAINERS => Some(index),
            _ => return Err((at, Reason::TailBattery { at })),
        };
        let state = match state_id(reader, 3)? {
            0 => BatteryState::Take {
                motion: BatteryMotion::read(reader)?,
                animation: BatteryMotion::animation(reader)?,
            },
            1 => BatteryState::Carry {
                motion: BatteryMotion::read(reader)?,
            },
            _ => BatteryState::Drop {
                motion: BatteryMotion::read(reader)?,
                animation: BatteryMotion::animation(reader)?,
            },
        };
        Ok(Self {
            at: record_at,
            id,
            spotted_mask,
            user,
            container,
            state,
            portable: Portable::read(reader)?,
        })
    }
}

/// The artefact slots of the inventory header: where they begin and each
/// slot's artefact id, borrowed from the world.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArtefactSlots<'a> {
    begin: SlotId,
    ids: &'a [u8],
}

impl<'a> ArtefactSlots<'a> {
    /// `slots_begin`: the first slot they take, a quick slot 13..18.
    pub fn begin(&self) -> SlotId {
        self.begin
    }

    /// Each slot's artefact, an index of the artefacts table; `None` for
    /// `artefact_base::invalid_id` 0xFF.
    pub fn ids(&self) -> impl ExactSizeIterator<Item = Option<u8>> + 'a {
        self.ids.iter().map(|&id| (id != 0xFF).then_some(id))
    }

    /// The slots they take.
    pub fn slots(&self) -> core::ops::Range<u8> {
        let begin = self.begin.get();
        begin..begin.saturating_add(u8::try_from(self.ids.len()).unwrap_or(u8::MAX))
    }
}

/// An inventory slot byte that is 23 for none (`m_active_slot`,
/// `m_grenade_slot`).
fn slot_or_none(reader: &mut Reader, field: &'static str) -> Result<Option<SlotId>, Fault> {
    let at = reader.at();
    match u8(reader)? {
        SLOTS => Ok(None),
        slot => SlotId::new(slot)
            .map(Some)
            .ok_or(outside(at, field, slot.into())),
    }
}

/// A carried battery as the header holds it: its id and user (what the
/// match's rule needs) and its bytes, read again by
/// [`InventoryHeader::carried`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Carried<'a> {
    id: u8,
    user: Place,
    at: usize,
    bytes: &'a [u8],
}

/// A carried battery read and dropped out of line (its frame holds the
/// view), its user and its bytes kept.
#[inline(never)]
fn carried<'a>(reader: &mut Reader<'a>, id: u8) -> Result<Carried<'a>, Fault> {
    let from = *reader;
    let battery = CarriedBattery::read(reader, id)?;
    Ok(Carried {
        id,
        user: battery.user,
        at: from.at(),
        bytes: reader.since(&from),
    })
}

/// The inventory header (`inventory::deserialize` 0x2FE550), borrowed from
/// the world.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InventoryHeader<'a> {
    carried_weight: Finite,
    need_to_recalculate_weight: bool,
    carried: Option<Carried<'a>>,
    artefact_slots: Option<ArtefactSlots<'a>>,
    active_slot: Option<SlotId>,
    grenade_slot: Option<SlotId>,
}

impl<'a> InventoryHeader<'a> {
    /// `m_carried_weight`.
    pub fn carried_weight(&self) -> Finite {
        self.carried_weight
    }

    /// `m_need_to_recalculate_weight`.
    pub fn need_to_recalculate_weight(&self) -> bool {
        self.need_to_recalculate_weight
    }

    /// The battery the player carries (`m_carried_item`), if any, read
    /// again from its bytes.
    pub fn carried(&self) -> Option<CarriedBattery> {
        let carried = self.carried?;
        CarriedBattery::read(&mut Reader::new(carried.bytes, carried.at), carried.id).ok()
    }

    /// The artefact slots, `None` when `m_artefact_slots_count` is 0.
    pub fn artefact_slots(&self) -> Option<ArtefactSlots<'a>> {
        self.artefact_slots
    }

    /// `m_active_slot`, `None` for 23.
    pub fn active_slot(&self) -> Option<SlotId> {
        self.active_slot
    }

    /// `m_grenade_slot`, `None` for 23.
    pub fn grenade_slot(&self) -> Option<SlotId> {
        self.grenade_slot
    }

    fn read(reader: &mut Reader<'a>, artefacts: Option<usize>) -> Result<Self, Fault> {
        let carried_weight = float(reader)?;
        let at = reader.at();
        let need_to_recalculate_weight = reader
            .bool()
            .map_err(short)?
            .ok_or((at, Reason::TailInventoryBool))?;
        let carried = match u8(reader)? {
            0xFF => None,
            id => Some(carried(reader, id)?),
        };
        let count_at = reader.at();
        let count = u8(reader)?;
        if usize::from(count) > INVENTORY_SLOTS {
            return Err((count_at, Reason::TailArtefactCount { count }));
        }
        let artefact_slots = if count == 0 {
            None
        } else {
            let begin_at = reader.at();
            let begin = u8(reader)?;
            let bad_begin = (begin_at, Reason::TailSlotsBegin { begin, count });
            if !QUICK_SLOTS.contains(&begin) || begin.saturating_add(count) > QUICK_SLOTS.end {
                return Err(bad_begin);
            }
            let begin = SlotId::new(begin).ok_or(bad_begin)?;
            let from = *reader;
            for _ in 0..count {
                let at = reader.at();
                let id = optional_u8(reader)?;
                if let (Some(index), Some(bound)) = (id, artefacts)
                    && usize::from(index) >= bound
                {
                    return Err((at, Reason::TailArtefact { index, bound }));
                }
            }
            Some(ArtefactSlots {
                begin,
                ids: reader.since(&from),
            })
        };
        Ok(Self {
            carried_weight,
            need_to_recalculate_weight,
            carried,
            artefact_slots,
            active_slot: slot_or_none(reader, "m_active_slot")?,
            grenade_slot: slot_or_none(reader, "m_grenade_slot")?,
        })
    }
}

/// A player's state from its start to the inventory header, borrowed from
/// the world, and where its items start. Its records are held as bytes and
/// handed out by reading them again, so that a check holds one at a time
/// (the game's check runs on the world loader's stack).
#[derive(Clone, Copy, Debug)]
pub struct PlayerState<'a> {
    head: Head<'a>,
    effects: &'a [u8],
    effects_at: usize,
    hit_effects: &'a [u8],
    hit_effects_at: usize,
    hit_effect_count: usize,
    current_active_object: Option<u8>,
    target_active_object: Option<u8>,
    stamina: Stamina,
    inventory: InventoryHeader<'a>,
    items: Reader<'a>,
}

impl<'a> PlayerState<'a> {
    pub fn head(&self) -> &Head<'a> {
        &self.head
    }

    /// The player effects, read again from their bytes.
    pub fn effects(&self) -> impl Iterator<Item = Effect> + 'a {
        let mut reader = Reader::new(self.effects, self.effects_at);
        core::iter::from_fn(move || Effect::read(&mut reader).ok())
    }

    /// The hit presenter's `m_effects`, read again from their bytes.
    pub fn hit_effects(&self) -> impl Iterator<Item = HitEffect> + 'a {
        let mut reader = Reader::new(self.hit_effects, self.hit_effects_at);
        let mut left = self.hit_effect_count;
        core::iter::from_fn(move || {
            left = left.checked_sub(1)?;
            HitEffect::read(&mut reader).ok()
        })
    }

    /// `m_current_active_object`'s id, `None` for 0xFF.
    pub fn current_active_object(&self) -> Option<u8> {
        self.current_active_object
    }

    /// `m_target_active_object`'s id, `None` for 0xFF.
    pub fn target_active_object(&self) -> Option<u8> {
        self.target_active_object
    }

    pub fn stamina(&self) -> Stamina {
        self.stamina
    }

    pub fn inventory(&self) -> &InventoryHeader<'a> {
        &self.inventory
    }

    /// The state of a place whose profile has `items` items, read to its
    /// inventory header; `artefacts` bounds the artefact-slot ids.
    pub fn read(state: &'a [u8], items: usize, artefacts: Option<usize>) -> Result<Self, Fault> {
        Self::read_with(state, Head::front(state)?, items, artefacts)
    }

    /// [`PlayerState::read`] with the head's front already read.
    fn read_with(
        state: &'a [u8],
        front: Front,
        items: usize,
        artefacts: Option<usize>,
    ) -> Result<Self, Fault> {
        let mut reader = Reader::new(state, 0);
        let head = Head::read(&mut reader, state, front, items)?;
        let head_len = reader.at();
        // The E player effects, each an effect in the one form a game writes
        // (`effects::Effect`, as the effects section's). E is 0 in every world
        // seen.
        let count = head.effect_count;
        if usize::try_from(count).map_or(true, |n| n > MAX_PLAYER_EFFECTS) {
            return Err((
                head_len,
                Reason::TailCount {
                    what: "player effects",
                    count,
                },
            ));
        }
        let effects = reader;
        for _ in 0..count {
            effect(&mut reader)?;
        }
        let effects = reader.since(&effects);
        let count_at = reader.at();
        let count = u32(&mut reader)?;
        let hit_effect_count = usize::try_from(count)
            .ok()
            .filter(|&n| n <= MAX_HIT_EFFECTS)
            .ok_or((
                count_at,
                Reason::TailCount {
                    what: "hit effects",
                    count,
                },
            ))?;
        let hit_effects = reader;
        for _ in 0..hit_effect_count {
            HitEffect::read(&mut reader)?;
        }
        let hit_effects_at = hit_effects.at();
        let hit_effects = reader.since(&hit_effects);
        let current_active_object = optional_u8(&mut reader)?;
        let target_active_object = optional_u8(&mut reader)?;
        let stamina = Stamina::read(&mut reader)?;
        let inventory = InventoryHeader::read(&mut reader, artefacts)?;
        Ok(Self {
            head,
            effects,
            effects_at: head_len,
            hit_effects,
            hit_effects_at,
            hit_effect_count,
            current_active_object,
            target_active_object,
            stamina,
            inventory,
            items: reader,
        })
    }

    /// The items past the inventory header and the rest of the state, read
    /// to its exact end against the match's kits: the place's own kit's
    /// items, the slots the header's artefacts took empty, a picked gun in
    /// its weapon slot. `None` without the kits. Out of line: the game's
    /// check, which has no kits, never takes the stack this needs.
    #[inline(never)]
    pub fn items_and_rest<'k>(
        &self,
        place: u8,
        context: &Context<'k>,
    ) -> Result<Option<ItemsAndRest<'a, 'k>>, Fault> {
        let Some(inventory) = context.inventory else {
            return Ok(None);
        };
        let Some(own) = inventory
            .kits
            .get(usize::from(place))
            .and_then(|kits| kits.get(usize::from(self.head.current_kit.get())))
        else {
            // A place the head refused already.
            return Ok(None);
        };
        let record = self.head.pickup;
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
                    let item = Item {
                        slot: ammo,
                        measure: Measure::Amount,
                        body: Body::None,
                    };
                    *slot = Some(Slot::Maybe(item));
                }
            }
        }
        // The slots the header's artefacts took hold them, not the kit's.
        if let Some(artefacts) = self.inventory.artefact_slots {
            for slot in artefacts.slots() {
                if let Some(slot) = slots.get_mut(usize::from(slot)) {
                    *slot = None;
                }
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
            let Some(Item {
                measure,
                body: Body::Weapon(gun),
                ..
            }) = gun.copied()
            else {
                return Err((2, Reason::PickupRecord));
            };
            if let Some(slot) = slots.get_mut(usize::from(weapon.slot())) {
                let item = Item {
                    slot: weapon.slot(),
                    measure,
                    body: Body::Weapon(Gun {
                        ammo: Ammo::Picked,
                        ..gun
                    }),
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
        inventory::items_and_rest(&self.items, &slots, self.head.alive, inventory, &owner).map(Some)
    }
}

/// A player effect read and dropped out of line (an [`Effect`] is a large
/// value): the reader past it, or why it is no effect.
#[inline(never)]
fn effect(reader: &mut Reader) -> Result<(), Fault> {
    Effect::read(reader).map(drop).map_err(|fault| match fault {
        EffectFault::Short(cut) => short(cut),
        fault => (fault.at(), Reason::TailEffect(fault)),
    })
}

/// The head of a player's state (snapshot_rules.md section 7.3), as world
/// apply 0x2D2770 uses it:
/// - `current_index` picks the place's player object with no bound
///   (`[entry + kit * 4]`, then a call through its vtable +0x3C): it must
///   be a kit of the place that has an object, one the match was sent as
///   usable; the object table has three entries;
/// - `target_index` is the next kit, used at the next spawn with the same
///   lookup: the same bound;
/// - the three bools are 0x00 or 0xFF (what the bool writer 0x80FA0
///   writes);
/// - the look pitch and the rotation are numbers.
///
/// Without the profiles (a tool reading kept worlds) the item count is not
/// known: the kit bytes are held to the table's three entries and the
/// pickup record read; [`player_tail`] guesses the rest.
// Out of line: its frame is its own, not the caller's.
#[inline(never)]
pub fn player_head(block: &Block, context: &Context) -> Result<(), Refusal> {
    let refuse = |(offset, reason): Fault| {
        Refusal::new(
            Part::Player(block.place),
            block.at.saturating_add(offset),
            reason,
        )
    };
    let Some(profiles) = context.profiles else {
        return Head::front(block.state).map(drop).map_err(refuse);
    };
    let Some(profile) = profiles.get(usize::from(block.place)) else {
        return Err(refuse((0, Reason::NoProfile)));
    };
    let mut reader = Reader::new(block.state, 0);
    let head = Head::front(block.state)
        .and_then(|front| Head::read(&mut reader, block.state, front, profile.items))
        .map_err(refuse)?;
    if !profile.usable(head.current_kit.get()) {
        return Err(refuse((0, Reason::CurrentKit(head.current_kit.get()))));
    }
    if !profile.usable(head.target_kit.get()) {
        return Err(refuse((1, Reason::NextKit(head.target_kit.get()))));
    }
    Ok(())
}

/// The tail reads up to the inventory header and every index it reaches is
/// within its bound. With the profiles M is the place's item count and the
/// walk is strict; without them (a tool, or the game checking a world with
/// no live match) M is any count the head and the tail both fit, as the
/// bullets are framed when the profiles are unknown.
// Out of line, as `player_head`: its frame is its own, not the caller's
// (the game's check runs on the world loader's 32 KiB of stack).
#[inline(never)]
pub fn player_tail(block: &Block, context: &Context) -> Result<(), Refusal> {
    let fault = match context.profiles {
        Some(profiles) => match profiles.get(usize::from(block.place)) {
            // A place with no profile was refused by the head already.
            None => return Ok(()),
            Some(profile) => strict(block, profile.items, context).err(),
        },
        None => guessed(block.state, context).err(),
    };
    match fault {
        None => Ok(()),
        Some((offset, reason)) => Err(Refusal::new(
            Part::Player(block.place),
            block.at.saturating_add(offset),
            reason,
        )),
    }
}

/// [`player_tail`] keeping what it read, for a reader that has the match's
/// profiles (the server): each place whose state reads is handed to `keep`
/// with its state to the inventory header and, with the match's kits, its
/// items and the rest of the state to its exact end. The same reads and the
/// same refusals as [`player_tail`]; not for the game's check, whose stack
/// has no room for the items (it has no kits).
pub fn player_tail_kept<'a>(
    block: &Block<'a>,
    context: &Context,
    keep: &mut dyn for<'k> FnMut(u8, &PlayerState<'a>, Option<&ItemsAndRest<'a, 'k>>),
) -> Result<(), Refusal> {
    let Some(profiles) = context.profiles else {
        return player_tail(block, context);
    };
    // A place with no profile was refused by the head already.
    let Some(profile) = profiles.get(usize::from(block.place)) else {
        return Ok(());
    };
    let refuse = |(offset, reason): Fault| {
        Refusal::new(
            Part::Player(block.place),
            block.at.saturating_add(offset),
            reason,
        )
    };
    let state = PlayerState::read(block.state, profile.items, context.artefacts).map_err(refuse)?;
    own_battery(&state, block.place).map_err(refuse)?;
    let items = state.items_and_rest(block.place, context).map_err(refuse)?;
    keep(block.place, &state, items.as_ref());
    Ok(())
}

/// The state of a place with a profile of `items` items, to its exact end
/// when the kits are known; its carried battery is its own.
fn strict(block: &Block, items: usize, context: &Context) -> Result<(), Fault> {
    let state = PlayerState::read(block.state, items, context.artefacts)?;
    // [`own_battery`] repeats this rule for [`player_tail_kept`]: a change goes in both.
    if let Some(battery) = state.inventory.carried
        && battery.user.get() != block.place
    {
        // With another user the game takes the free battery's layout, live
        // state the stream does not say.
        let at = battery.at.saturating_add(4);
        return Err((at, Reason::TailBattery { at }));
    }
    if context.inventory.is_some() {
        check_items(&state, block.place, context)?;
    }
    Ok(())
}

/// [`strict`]'s rule that a carried battery's user is the place itself, for [`player_tail_kept`] (repeated so that
/// the game's path stays as it is: the exe's arena links it).
fn own_battery(state: &PlayerState, place: u8) -> Result<(), Fault> {
    if let Some(battery) = state.inventory.carried
        && battery.user.get() != place
    {
        let at = battery.at.saturating_add(4);
        return Err((at, Reason::TailBattery { at }));
    }
    Ok(())
}

/// The items and the rest of `state`, read and dropped. Out of line: the
/// game's check, which has no kits, never comes here.
#[inline(never)]
fn check_items(state: &PlayerState, place: u8, context: &Context) -> Result<(), Fault> {
    state.items_and_rest(place, context).map(drop)
}

/// The largest item count a guessed walk tries: the head must fit, and a
/// profile holds at most [`MAX_ITEMS`].
fn max_items(state_len: usize) -> usize {
    // head_len = 4M + 29 + 27 <= state_len
    (state_len.saturating_sub(PLAYER_FIELDS_AT.saturating_add(HEAD_AFTER_ITEMS)) / 4).min(MAX_ITEMS)
}

/// Without the profiles: the tail walks for some item count M, as the
/// bullets search when the match is guessed. No profiles means no match,
/// so no inventory to walk: the walk stops at the inventory header. The
/// fault reported is the furthest any M reached.
fn guessed(state: &[u8], context: &Context) -> Result<(), Fault> {
    let front = Head::front(state)?;
    let mut furthest: Option<Fault> = None;
    for items in 0..=max_items(state.len()) {
        match PlayerState::read_with(state, front, items, context.artefacts) {
            Ok(_) => return Ok(()),
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
        s.extend([7, 23]); // the active slot, no grenade slot
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
    fn every_field_of_the_state_reads_by_name() {
        let state = valid_state();
        let read = PlayerState::read(&state, 1, Some(ARTEFACTS)).unwrap();
        let head = read.head();
        assert_eq!((head.current_kit().get(), head.target_kit().get()), (0, 0));
        assert_eq!(head.pickup(), weapon_pickup::Record::EMPTY);
        assert_eq!(head.stored().collect::<Vec<_>>(), [0]);
        assert!(head.alive() && !head.has_to_die() && !head.need_to_select_animations());
        assert_eq!(
            (head.look_pitch(), head.rotation_y()),
            (Finite::ZERO, Finite::ZERO)
        );
        assert_eq!(
            (
                head.actions_mask(),
                head.effects_time_ms(),
                head.effect_count()
            ),
            (0, 0, 0)
        );
        assert!(read.effects().next().is_none() && read.hit_effects().next().is_none());
        assert_eq!(
            (read.current_active_object(), read.target_active_object()),
            (None, None)
        );
        assert!(!read.stamina().is_low_stamina());
        let inventory = read.inventory();
        assert_eq!(inventory.carried_weight(), Finite::ZERO);
        assert_eq!(
            (inventory.carried(), inventory.artefact_slots()),
            (None, None)
        );
        assert_eq!(
            (inventory.active_slot(), inventory.grenade_slot()),
            (SlotId::new(7), None)
        );
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
        let state = battery(0, 1, 2);
        let read = PlayerState::read(&state, 1, None).unwrap();
        let carried = read.inventory().carried().unwrap();
        assert_eq!(
            (carried.id(), carried.spotted_mask(), carried.user().get()),
            (3, 1, 0)
        );
        assert_eq!(carried.container(), Some(1));
        assert!(matches!(
            carried.state(),
            BatteryState::Drop {
                animation: Some(BatteryAnimation::FirstPerson),
                ..
            }
        ));
        let reason = |state: &[u8]| with_profile(state).expect_err("refused").reason;
        // Another user, a container past the two, a state past the three.
        assert!(matches!(
            reason(&battery(1, 0xFF, 1)),
            Reason::TailBattery { .. }
        ));
        assert!(matches!(
            reason(&battery(0, 2, 1)),
            Reason::TailBattery { .. }
        ));
        assert!(matches!(
            reason(&battery(0, 0xFF, 3)),
            Reason::TailGunState { state: 3, .. }
        ));
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
    fn a_hit_effect_with_a_nonzero_emitter_index_goes_to_nobody() {
        // H = 1, one record present whose root emitter is the index form
        // (flag 0) but names a non-zero registry entry: the native loader
        // (0x340410 -> 0x2E5C30) would read `[0xAA2E48 + index*4]` and
        // ref-count through it with no bound. Must be refused like the
        // effects template's root emitter, not loaded.
        let mut state = valid_state();
        state.truncate(HIT_AT);
        state.extend(1u32.to_le_bytes()); // H = 1
        state.push(0xFF); // is_exists
        state.push(0x00); // is_client_effect: the index form
        state.extend(0x0000_0001u32.to_le_bytes()); // registry_index: not 0
        state.extend([0u8; 32]); // room, so the fault is the index not a short read
        let refusal = with_profile(&state).expect_err("refused");
        assert!(
            matches!(refusal.reason, Reason::TailHitEffect { .. }),
            "{refusal}"
        );
    }

    #[test]
    fn a_hit_effect_with_a_zero_emitter_index_is_read() {
        // The same record with registry_index 0 (the effects template's
        // value) is the valid index form and must still be accepted.
        let mut state = valid_state();
        let tail: Vec<u8> = state.split_off(HIT_AT);
        state.extend(1u32.to_le_bytes()); // H = 1
        state.push(0xFF); // is_exists
        state.push(0x00); // is_client_effect: the index form
        state.extend(0u32.to_le_bytes()); // registry_index = 0 (valid)
        state.extend(0xFFFF_FFFFu32.to_le_bytes()); // single_effect_index: none
        state.extend(0.0f32.to_le_bytes()); // last_hit_time
        state.extend(0.0f32.to_le_bytes()); // time
        state.extend(&tail[4..]); // the active bytes onward
        assert_eq!(with_profile(&state), Ok(()));
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
        w.extend(1u32.to_le_bytes()); // m_random_generator
        w.extend(u32::MAX.to_le_bytes()); // never ticked
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
        Item {
            slot,
            measure: Measure::Condition,
            body,
        }
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
    fn the_speed_and_the_last_position_are_numbers() {
        // Place 12's state in rec026g_20261106-211841_03439962 from
        // 540 200 ms (a revision-50 game): its position NaN, the speed and
        // the controller's last position NaN too. Exe patch 117 keeps the
        // position finite, so none of these is a state a game writes.
        let items = [item(0, Body::None)];
        let mut state = valid_state();
        state.extend(header_item(0, 0, 0));
        let speed = state.len() + 1;
        let health = speed + 4 + 1 + 16;
        let last_pos = health + BODY_PARTS * 14 + 1 + 4;
        state.extend(rest());
        assert_eq!(with_inventory(&state, &items), Ok(()));
        for bits in [(-f32::NAN).to_bits(), f32::NAN.to_bits(), f32::INFINITY.to_bits()] {
            for at in [speed, last_pos, last_pos + 4, last_pos + 8] {
                let mut bad = state.clone();
                bad[at..at + 4].copy_from_slice(&bits.to_le_bytes());
                assert!(
                    matches!(reason(&bad, &items), Reason::TailNotNumber { .. }),
                    "{bits:#x} at {at}"
                );
            }
        }
        // The damage model's floats too.
        state[health..health + 4].copy_from_slice(&f32::NAN.to_bits().to_le_bytes());
        assert!(matches!(
            reason(&state, &items),
            Reason::TailNotNumber { .. }
        ));
    }

    #[test]
    fn the_items_and_the_rest_read_by_name() {
        use crate::world::inventory::{ItemBody, Left};
        let mut state = valid_state();
        state.extend(header_item(0, 0, 0));
        state.extend(weapon_item(7, 0, 0, 1));
        state.extend(weapon_in_hand(10, 0, 1, &[0x00]));
        state.extend(rest());
        let items = [item(0, Body::None), item(7, gun(2)), item(10, gun(2))];
        let profiles = [profile()];
        let kits: [[&[Item]; crate::KITS]; 1] = [[&items, &[], &[]]];
        let inventory = Inventory {
            kits: &kits,
            body_parts: BODY_PARTS,
        };
        let context = Context {
            inventory: Some(&inventory),
            ..context(Some(&profiles))
        };
        let read = PlayerState::read(&state, 1, None).unwrap();
        let read = read.items_and_rest(0, &context).unwrap().unwrap();
        let mut items = read.items();
        let armour = items.next().unwrap();
        assert_eq!(
            armour.left(),
            Left::Condition(Finite::of_bits(1f32.to_bits()).unwrap())
        );
        assert_eq!(armour.owner().map(|own| own.slot().get()), Some(0));
        assert_eq!(armour.body(), ItemBody::None);
        let ItemBody::Weapon(stowed) = items.next().unwrap().body() else {
            panic!("a weapon");
        };
        assert_eq!(
            (stowed.ammo_in_magazine(), stowed.current_ammo_id()),
            (10, 1)
        );
        assert_eq!(stowed.in_hand(), None);
        let ItemBody::Weapon(held) = items.next().unwrap().body() else {
            panic!("a weapon");
        };
        let in_hand = held.in_hand().unwrap();
        assert_eq!(in_hand.state_index(), 1);
        assert_eq!(in_hand.state().class(), GunState::Flag);
        assert!(!in_hand.aimed());
        assert!(items.next().is_none());
        let rest = read.rest();
        assert_eq!(rest.damage_model().body_parts().count(), BODY_PARTS);
        assert!(rest.controller().is_some());
        assert_eq!(rest.carry()[0], 2);
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
        assert!(matches!(
            reason(&nan, &medkit),
            Reason::TailNotNumber { .. }
        ));
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
        assert!(matches!(
            reason(&affect, &[]),
            Reason::TailAffect { kind: 9 }
        ));
        let mut hits = state.clone();
        hits[part + 9] = 65;
        assert!(matches!(
            reason(&hits, &[]),
            Reason::TailCount { what: "hits", .. }
        ));
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

    #[test]
    fn a_field_takes_only_the_values_its_writer_saves() {
        let field = |state: &[u8]| match with_profile(state).map_err(|r| r.reason) {
            Err(Reason::TailField { field, .. }) => Some(field),
            _ => None,
        };
        // The active and grenade slots: a slot, or 23 for none.
        let len = valid_state().len();
        let mut state = valid_state();
        state[len - 2] = 24;
        assert_eq!(field(&state), Some("m_active_slot"));
        state[len - 2] = 23;
        assert_eq!(with_profile(&state), Ok(()));
        // At most the presenter's three hit effects.
        let mut state = valid_state();
        state[HIT_AT] = 4;
        assert!(matches!(
            with_profile(&state).map_err(|r| r.reason),
            Err(Reason::TailCount {
                what: "hit effects",
                count: 4
            })
        ));
        // Artefact slots begin at a quick slot and end by slot 18.
        let artefacts = |begin: u8, count: u8| {
            let mut state = valid_state();
            let tail = state.split_off(A_AT + 1);
            state[A_AT] = count;
            state.push(begin);
            state.extend(std::iter::repeat_n(0xFF, usize::from(count)));
            state.extend(tail);
            with_profile(&state).map_err(|r| r.reason)
        };
        assert_eq!(artefacts(13, 6), Ok(()));
        assert!(matches!(
            artefacts(12, 1),
            Err(Reason::TailSlotsBegin { .. })
        ));
        assert!(matches!(
            artefacts(18, 2),
            Err(Reason::TailSlotsBegin { .. })
        ));
    }

    #[test]
    fn an_item_takes_only_the_values_its_writer_saves() {
        let in_hand = |patch: &dyn Fn(&mut Vec<u8>)| {
            let mut weapon = weapon_in_hand(7, 0, 1, &[0x00]);
            patch(&mut weapon);
            let mut state = valid_state();
            state.extend(weapon);
            state.extend(rest());
            with_inventory(&state, &[item(7, gun(2))])
        };
        assert_eq!(in_hand(&|_| ()), Ok(()));
        // The header (7 bytes), the seed: never 0.
        assert!(in_hand(&|w| w[7..11].copy_from_slice(&[0; 4])).is_err());
        // The sprint animation is 0, 2, 4 or 6 (none): 1 is no shape's.
        let sprint = 7 + 12 + 4 + 8 + 40 + 4 + 1 + 20;
        assert!(in_hand(&|w| w[sprint] = 1).is_err());
        assert_eq!(in_hand(&|w| w[sprint] = 6), Ok(()));
        // An active medkit has an owner: its reader takes the owner's
        // inventory with no null check.
        let medkit = [Item {
            slot: 13,
            measure: Measure::Amount,
            body: Body::Medkit { influences: 0 },
        }];
        let mut state = valid_state();
        state.extend([0, 0, 0xFF, 0xFF]); // amount, no owner, active
        state.extend([0; 8]);
        state.extend(rest());
        assert!(matches!(
            reason(&state, &medkit),
            Reason::TailField {
                field: "an unowned item's m_active",
                ..
            }
        ));
    }
}
