//! A player state's inventory items and everything after them, to the
//! state's exact end (`base_player::deserialize` 0x2DB490 from the
//! inventory header on; the writer is `base_player::serialize` 0x2DAF60).
//!
//! The stream does not say which slots hold an item nor each item's shape:
//! `inventory::deserialize`'s loop 0x2FE7A9 walks the live kit's objects in
//! slot order, so the walk is given the kit's items as data ([`Inventory`]).
//! An uploaded world has no second reader (0x2D2A03..0x2D2A2C), so nothing
//! the game reads only from it is here. PROVEN unless marked, every layout
//! read to the exact end of the captured states.
//!
//! - An item (`inventory_item::deserialize` 0x2FFF10): a `u16` amount for a
//!   stack, else an `f32` condition; an owner place (0xFF none), then its
//!   kit and the item's slot. The owner is looked up with no bound,
//!   `[[core+0x18458] + place*0x30 + kit*4] + 0x10C`: a place of the match
//!   and a kit with a player object.
//! - Its body ([`Body`]): a medkit's active flag and, when active, two u32
//!   and a float for each of its influences (0x33DCA0); an oxygen tank's
//!   flag and u32 (0x33E7F0); a weapon's ([`Gun`]); anything else none.
//! - A weapon (`weapon_core::deserialize` 0x2E3490): its seed and tick, a
//!   load flag, the magazine, the fire queue, the current ammo id (an
//!   **unbounded** index into its ammo list, 0x2E3525), the chambered flag
//!   of a gun with that state; then, only for a gun with a user (one put in
//!   hand since its cook, live state the stream does not carry: both shapes
//!   are tried), the ammo switch (and the next ammo id, a second unbounded
//!   index), the recoil, the dispersion, the aim, the breath, the sprint
//!   and auto-reload bytes, the weapon's state machine (a state id past its
//!   states is a null vtable, `fsm::deserialize` 0x38CCF0, then that
//!   state's own bytes, [`GunState`]) and the portable object's
//!   (0x2988A0: a u32, a flag, its animation selector's state 0..6, the
//!   jump's with a sub-state and the dead one's with two bytes, and eight
//!   12-byte bone records).
//! - After the items: the jump type, the speed, the damage model (0x2E5480:
//!   a flag, four u32, then each body part's health, hit time, up to 8
//!   affects of a type under 9, up to 64 hits and a speed modifier,
//!   0x32AFE0), the usable-object data (0x32FD60), a u32, the character
//!   controller when the head says alive (0x7FB10, 67 bytes), the spotting
//!   times, the stagger, the recent hits (a u32 count of 8-byte records),
//!   the respawn flag, and patch 78's 32-byte carry snapshot (version 2).

use crate::reader::{Reader, Short};
use crate::{KITS, Reason, finite};

/// What a match's player states are walked against.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Inventory<'a> {
    /// By place, by kit: the kit's items as the game makes its objects
    /// (`inventory_cook::on_subresources_loaded` 0x2C75B0), in slot order.
    pub kits: &'a [[&'a [Item<'a>]; KITS]],
    /// The damage model's body parts: the entries of the character's
    /// `hit_params` (`gameplay/hit_params/human_hit_params.options`).
    pub body_parts: usize,
}

/// One object of a kit's inventory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Item<'a> {
    /// Its slot, 0..22.
    pub slot: u8,
    /// The dictionary's `is_stack`: a `u16` amount, else an `f32`
    /// condition.
    pub stack: bool,
    pub body: Body<'a>,
}

/// What an item saves after its header, by its class.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Body<'a> {
    /// Armour, ammunition, a booster, a grenade or trap set.
    None,
    /// `influences`: the floats an active one saves.
    Medkit { influences: u8 },
    OxygenTank,
    Weapon(Gun<'a>),
}

/// A weapon item's shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Gun<'a> {
    /// Its state machine's states in the cook's order (show, hide, idle,
    /// reload, fire, the chamber's if it has one, throw grenade, melee).
    pub states: &'a [GunState],
    /// It has a `chamber_a_round` state: the chambered flag is saved.
    pub chamber: bool,
    /// Its ammo list's length (`weapon_core+0x5B0`: the kit's ammo slots
    /// holding a compatible stack, at most 8). `None` for a picked gun,
    /// whose list the picker's game made of its own slots.
    pub ammo: Option<u8>,
}

/// A weapon state's class (`states.<name>.state_type` of the weapon's
/// config) by the bytes it saves (the cook's registrations
/// 0x24B9FF..0x30CE4F).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GunState {
    /// 302, 310, 313 (idle): a flag.
    Flag,
    /// 300, 301, 303, 304, 311, 314 (show, hide, reload, chamber): a flag
    /// and a byte.
    FlagByte,
    /// 305 (fire): 4 bytes.
    Fire,
    /// 312, 315 (a double-barrel's or pistol's fire): 5 bytes.
    FireTwice,
    /// 321 (melee): 3 bytes.
    Melee,
    /// 306 (a shotgun's reload): a flag, a sub-state under 3 and 2 bytes.
    ShotgunReload,
    /// 316 (throw grenade): a flag, a sub-state under 3, 2 bytes, the
    /// grenade's inventory slot (an **unbounded** index into the slots,
    /// 0x33A5AB) and a byte.
    ThrowGrenade,
}

impl GunState {
    /// The class of a `state_type`, if the game has one.
    pub fn of_type(state_type: u32) -> Option<Self> {
        Some(match state_type {
            302 | 310 | 313 => Self::Flag,
            300 | 301 | 303 | 304 | 311 | 314 => Self::FlagByte,
            305 => Self::Fire,
            312 | 315 => Self::FireTwice,
            321 => Self::Melee,
            306 => Self::ShotgunReload,
            316 => Self::ThrowGrenade,
            _ => return None,
        })
    }
}

/// The inventory's slots.
pub(super) const SLOTS: u8 = 23;
/// An ammo list holds at most this many (0x2E41D0).
const MAX_AMMO: u8 = 8;
/// A body part's affects and hits, inline buffers that assert past these
/// (0x32B039, 0x32B5C9), and the affect types (the damage model's nine
/// signals, 0x2E4DC4).
const MAX_AFFECTS: u8 = 8;
const MAX_HITS: u8 = 64;
const AFFECT_TYPES: u8 = 9;
/// The portable object's animation selector's states (0x2FAB70: stand,
/// run, sprint, jump, crouch, crouch walk, dead), the jump's sub-states
/// (0x336E60) with the bytes of each, and the selector's bone records
/// (0x330A20).
const PORTABLE_STATES: u8 = 7;
const PORTABLE_JUMP: u8 = 3;
const PORTABLE_DEAD: u8 = 6;
const JUMP_SUB_STATES: [usize; 5] = [7, 4, 3, 3, 2];
const BONES: usize = 8;
/// The breath's states (`initialize_logic` 0x31D450), which save nothing.
const BREATH_STATES: u8 = 3;
/// Patch 78's carry snapshot and its version.
const CARRY_SNAPSHOT: usize = 32;
const CARRY_VERSION: u8 = 2;
/// A weapon state machine's sub-state count (shotgun reload, throw).
const SUB_STATES: u8 = 3;

/// A fault: where in the state and why.
pub(super) type Fault = (usize, Reason);

pub(super) fn short(s: Short) -> Fault {
    (s.at, Reason::TailUnread { at: s.at })
}

/// What the walk needs of the state's place.
pub(super) struct Owner<'a> {
    /// The places of the match and, by place, whose kits have a player.
    pub places: usize,
    pub usable: &'a dyn Fn(u8, u8) -> bool,
}

/// An object the walk expects in a slot: one the state has (`optional`
/// false), or one it may have (an ammo slot of a picker: patch 85 moves the
/// borrowed gun's stacks into them, live state the stream does not say).
#[derive(Clone, Copy)]
pub(super) struct Slot<'a> {
    pub item: Item<'a>,
    pub optional: bool,
}

/// The items past the inventory header, then the rest of the state, to its
/// exact end. `items` are the kit's objects in slot order with the slots
/// the header's artefacts took left out; `alive` the head's `m_is_alive`.
pub(super) fn items_and_rest(
    reader: &Reader,
    items: &[Slot],
    alive: bool,
    inventory: &Inventory,
    owner: &Owner,
) -> Result<(), Fault> {
    let mut furthest = None;
    match walk(*reader, items, alive, inventory, owner, &mut furthest) {
        Some(()) => Ok(()),
        None => Err(furthest.unwrap_or((reader.at(), Reason::TailUnread { at: reader.at() }))),
    }
}

/// Keeps the furthest fault of the shapes tried.
fn keep(furthest: &mut Option<Fault>, fault: Fault) {
    if furthest.as_ref().is_none_or(|(at, _)| fault.0 >= *at) {
        *furthest = Some(fault);
    }
}

/// Every shape the items can have (a weapon with a user or without, a
/// picked gun's two ammo-list layouts): `Some` when one reads the state to
/// its exact end.
fn walk(
    reader: Reader,
    items: &[Slot],
    alive: bool,
    inventory: &Inventory,
    owner: &Owner,
    furthest: &mut Option<Fault>,
) -> Option<()> {
    let Some((&Slot { item, optional }, rest)) = items.split_first() else {
        return match after_items(reader, alive, inventory.body_parts) {
            Ok(()) => Some(()),
            Err(fault) => {
                keep(furthest, fault);
                None
            }
        };
    };
    if optional && walk(reader, rest, alive, inventory, owner, furthest).is_some() {
        return Some(());
    }
    let item = &item;
    let mut reader = reader;
    if let Err(fault) = header(&mut reader, item, owner) {
        keep(furthest, fault);
        return None;
    }
    let shapes: &[Shape] = match item.body {
        Body::Weapon(Gun { ammo: Some(_), .. }) => &[Shape::User, Shape::NoUser],
        Body::Weapon(Gun { ammo: None, .. }) => {
            &[Shape::PickedShortList, Shape::PickedLongList, Shape::NoUser]
        }
        _ => &[Shape::NoUser],
    };
    for &shape in shapes {
        let mut tried = reader;
        match body(&mut tried, item.body, shape) {
            Ok(()) => {
                if walk(tried, rest, alive, inventory, owner, furthest).is_some() {
                    return Some(());
                }
            }
            Err(fault) => keep(furthest, fault),
        }
    }
    None
}

/// The two shapes of a weapon (with a user or not), and a picked gun's
/// ammo-list layouts (under two entries, or two and more).
#[derive(Clone, Copy)]
enum Shape {
    User,
    NoUser,
    PickedShortList,
    PickedLongList,
}

fn header(reader: &mut Reader, item: &Item, owner: &Owner) -> Result<(), Fault> {
    if item.stack {
        reader.skip(2).map_err(short)?;
    } else {
        float(reader)?;
    }
    let at = reader.at();
    let place = reader.u8().map_err(short)?;
    if place != 0xFF {
        let kit = reader.u8().map_err(short)?;
        if usize::from(place) >= owner.places || !(owner.usable)(place, kit) {
            return Err((
                at,
                Reason::TailItemOwner {
                    player_id: place,
                    profile_id: kit,
                },
            ));
        }
        // The slot it is in: stored with no lookup, always its own.
        let at = reader.at();
        let slot = reader.u8().map_err(short)?;
        if slot != item.slot {
            return Err((at, Reason::TailItemSlot { slot, wanted: item.slot }));
        }
    }
    Ok(())
}

fn body(reader: &mut Reader, body: Body, shape: Shape) -> Result<(), Fault> {
    match body {
        Body::None => Ok(()),
        Body::Medkit { influences } => {
            if flag(reader)? {
                reader.skip(8).map_err(short)?;
                floats(reader, usize::from(influences))?;
            }
            Ok(())
        }
        Body::OxygenTank => {
            flag(reader)?;
            reader.skip(4).map_err(short)
        }
        Body::Weapon(gun) => weapon(reader, gun, shape),
    }
}

fn weapon(reader: &mut Reader, gun: Gun, shape: Shape) -> Result<(), Fault> {
    // The seed, the tick, the load flag, the magazine, the fire queue.
    reader.skip(8).map_err(short)?;
    flag(reader)?;
    reader.skip(2).map_err(short)?;
    let list = match shape {
        Shape::PickedShortList => Some(1),
        Shape::PickedLongList => Some(MAX_AMMO),
        _ => gun.ammo,
    };
    // A picked gun's list is the picker's: its ids bounded by the most a
    // list holds.
    let bound = gun.ammo.unwrap_or(MAX_AMMO);
    ammo_id(reader, bound)?;
    if gun.chamber {
        flag(reader)?;
    }
    if matches!(shape, Shape::NoUser) {
        return Ok(());
    }
    if list.unwrap_or(0) >= 2 && reader.u32().map_err(short)? != u32::MAX {
        ammo_id(reader, bound)?;
    }
    // The recoil: a u32, and when its second is set 48 more bytes.
    reader.skip(4).map_err(short)?;
    if reader.u32().map_err(short)? != u32::MAX {
        reader.skip(48).map_err(short)?;
    }
    // The dispersion: two transitions and a float; the aim and its
    // transition.
    transition(reader)?;
    transition(reader)?;
    float(reader)?;
    let aimed = flag(reader)?;
    transition(reader)?;
    if aimed {
        // The breath's vibration, then its state machine's state, whose
        // states save nothing (0x31AD50).
        reader.skip(32).map_err(short)?;
        state_id(reader, BREATH_STATES)?;
    }
    // The sprint animation, the auto-reload flag.
    reader.skip(1).map_err(short)?;
    flag(reader)?;
    let at = reader.at();
    let state = reader.u8().map_err(short)?;
    let Some(&class) = gun.states.get(usize::from(state)) else {
        return Err((
            at,
            Reason::TailGunState {
                state,
                states: gun.states.len(),
            },
        ));
    };
    gun_state(reader, class)?;
    portable(reader)
}

/// A portable object (`weapon_controller_core::deserialize` 0x2FF970, a
/// weapon's and a battery's): its animation selector's u32, a
/// flag, the selector's state (`weapon_user_animations_selector`, its jump
/// with its own sub-state, 0x337510; the dead one's flag and byte,
/// 0x338430) and the bones. Its legs' IK only with `ik_on_legs`, which no
/// game sets.
pub(super) fn portable(reader: &mut Reader) -> Result<(), Fault> {
    reader.skip(4).map_err(short)?;
    flag(reader)?;
    match state_id(reader, PORTABLE_STATES)? {
        PORTABLE_JUMP => {
            reader.skip(1).map_err(short)?;
            flag(reader)?;
            float(reader)?;
            let sub = state_id(reader, JUMP_SUB_STATES.len() as u8)?;
            let bytes = JUMP_SUB_STATES.get(usize::from(sub)).copied().unwrap_or(0);
            reader.skip(bytes).map_err(short)?;
        }
        PORTABLE_DEAD => {
            flag(reader)?;
            reader.skip(1).map_err(short)?;
        }
        _ => {}
    }
    reader.skip(BONES.saturating_mul(12)).map_err(short)
}

fn gun_state(reader: &mut Reader, class: GunState) -> Result<(), Fault> {
    match class {
        GunState::Flag => flag(reader).map(drop),
        GunState::FlagByte => {
            flag(reader)?;
            reader.skip(1).map_err(short)
        }
        GunState::Fire => reader.skip(4).map_err(short),
        GunState::FireTwice => reader.skip(5).map_err(short),
        GunState::Melee => reader.skip(3).map_err(short),
        GunState::ShotgunReload => {
            flag(reader)?;
            sub_state(reader)?;
            reader.skip(2).map_err(short)
        }
        GunState::ThrowGrenade => {
            flag(reader)?;
            sub_state(reader)?;
            reader.skip(2).map_err(short)?;
            let at = reader.at();
            let slot = reader.u8().map_err(short)?;
            if slot >= SLOTS {
                return Err((at, Reason::TailGrenadeSlot { slot }));
            }
            reader.skip(1).map_err(short)
        }
    }
}

fn sub_state(reader: &mut Reader) -> Result<(), Fault> {
    state_id(reader, SUB_STATES).map(drop)
}

/// A state machine's state id: one past its states is a null state, called
/// through (`fsm::deserialize` 0x38CCF0 -> 0x38CD26).
pub(super) fn state_id(reader: &mut Reader, states: u8) -> Result<u8, Fault> {
    let at = reader.at();
    let state = reader.u8().map_err(short)?;
    if state >= states {
        return Err((
            at,
            Reason::TailGunState {
                state,
                states: usize::from(states),
            },
        ));
    }
    Ok(state)
}

/// An ammo id of a list of `count` (a gun with none does not use it).
fn ammo_id(reader: &mut Reader, count: u8) -> Result<(), Fault> {
    let at = reader.at();
    let ammo = reader.u8().map_err(short)?;
    if count > 0 && ammo >= count {
        return Err((at, Reason::TailWeaponAmmo { ammo, count }));
    }
    Ok(())
}

/// A transition helper (0x32FF70): four floats and a u32.
fn transition(reader: &mut Reader) -> Result<(), Fault> {
    floats(reader, 4)?;
    reader.skip(4).map_err(short)
}

/// Everything after the items, to the state's exact end.
fn after_items(mut reader: Reader, alive: bool, body_parts: usize) -> Result<(), Fault> {
    let reader = &mut reader;
    // The jump type, the horizontal speed.
    reader.skip(1).map_err(short)?;
    float(reader)?;
    // The damage model: the second chance, four times, the body parts.
    flag(reader)?;
    reader.skip(16).map_err(short)?;
    for _ in 0..body_parts {
        float(reader)?;
        reader.skip(4).map_err(short)?;
        let at = reader.at();
        let affects = reader.u8().map_err(short)?;
        if affects > MAX_AFFECTS {
            return Err((
                at,
                Reason::TailCount {
                    what: "affects",
                    count: u32::from(affects),
                },
            ));
        }
        for _ in 0..affects {
            let at = reader.at();
            let kind = reader.u8().map_err(short)?;
            if kind >= AFFECT_TYPES {
                return Err((at, Reason::TailAffect { kind }));
            }
            reader.skip(4).map_err(short)?;
        }
        let at = reader.at();
        let hits = reader.u8().map_err(short)?;
        if hits > MAX_HITS {
            return Err((
                at,
                Reason::TailCount {
                    what: "hits",
                    count: u32::from(hits),
                },
            ));
        }
        for _ in 0..hits {
            reader.skip(4).map_err(short)?;
            float(reader)?;
        }
        float(reader)?;
    }
    // The usable-object data: when set, a time, a head offset, a use point
    // and a progress.
    if flag(reader)? {
        reader.skip(4).map_err(short)?;
        floats(reader, 7)?;
    }
    // The time it was inserted.
    reader.skip(4).map_err(short)?;
    if alive {
        // The character controller: its last position, six flags, four
        // vectors, the sprint flag.
        floats(reader, 3)?;
        for _ in 0..6 {
            flag(reader)?;
        }
        floats(reader, 12)?;
        flag(reader)?;
    }
    // The spotting times, the stagger.
    reader.skip(8).map_err(short)?;
    float(reader)?;
    // The recent hits.
    let at = reader.at();
    let hits = reader.u32().map_err(short)?;
    let room = reader.rest().len() / 8;
    if hits as usize > room {
        return Err((
            at,
            Reason::TailCount {
                what: "recent hits",
                count: hits,
            },
        ));
    }
    for _ in 0..hits {
        reader.skip(4).map_err(short)?;
        float(reader)?;
    }
    flag(reader)?;
    let at = reader.at();
    let carry = reader.take(CARRY_SNAPSHOT).map_err(short)?;
    if carry.first() != Some(&CARRY_VERSION) {
        return Err((at, Reason::TailCarry));
    }
    if reader.is_done() {
        Ok(())
    } else {
        let at = reader.at();
        Err((at, Reason::TailUnread { at }))
    }
}

/// A bool the game writes as 0x00 or 0xFF.
fn flag(reader: &mut Reader) -> Result<bool, Fault> {
    let at = reader.at();
    reader
        .bool()
        .map_err(short)?
        .ok_or((at, Reason::TailBool { at }))
}

fn float(reader: &mut Reader) -> Result<(), Fault> {
    let at = reader.at();
    if finite(reader.f32_bits().map_err(short)?) {
        Ok(())
    } else {
        Err((at, Reason::TailNotNumber { at }))
    }
}

fn floats(reader: &mut Reader, count: usize) -> Result<(), Fault> {
    for _ in 0..count {
        float(reader)?;
    }
    Ok(())
}
