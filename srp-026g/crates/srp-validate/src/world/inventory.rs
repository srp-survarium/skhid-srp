//! A player state's inventory items and everything after them, to the
//! state's exact end (`base_player::deserialize` 0x2DB490 from the
//! inventory header on; the writer is `base_player::serialize` 0x2DAF60),
//! read into zero-copy views ([`Items`], [`ItemState`], [`AfterItems`]).
//!
//! The stream does not say which slots hold an item nor each item's shape:
//! `inventory::deserialize`'s loop 0x2FE7A9 walks the live kit's objects in
//! slot order, so the walk is given the kit's items as data ([`Inventory`]).
//! An uploaded world has no second reader (0x2D2A03..0x2D2A2C), so nothing
//! the game reads only from it is here. PROVEN unless marked, every layout
//! read to the exact end of the captured states.
//!
//! FIELD MAP
//!
//! Each view's reader takes its record's bytes once: it turns every field
//! with a limited set of values into its type (a bool from 0x00/0xFF, an
//! enum from its byte, an index bounded by its table, a [`Finite`] from an
//! f32) and refuses the world when it cannot. Its accessors return those
//! types and cannot fail. Rules that need the match (an owner's kit, an
//! item's slot) are checked over the parsed values by the walk.
//!
//! ```text
//! item header (inventory_item::deserialize 0x2FFF10)
//!   u16 m_amount (a stack) | f32 m_condition      [`Left`]
//!   u8  owner place, 0xFF none                     [`ItemOwner`]
//!   u8  owner kit (player_profile_id), < 3
//!   u8  m_slot_id, < 23, the item's own
//! medkit (0x33DCA0)          bool m_active; if active u32 u32 and an
//!                            f32 per influence    [`Medkit`]
//! oxygen tank (0x33E7F0)     bool, u32            [`OxygenTank`]
//! weapon (0x2E3490)          [`WeaponState`], its doc
//! after the items            [`AfterItems`], its doc
//! ```

use crate::reader::{Reader, Short};
use crate::{Finite, KITS, Reason};
use super::weapon::{JumpType, WeaponState, jump_type, optional_u32};
use weapon_pickup::{Kit, Place};

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
    pub measure: Measure,
    pub body: Body<'a>,
}

/// What an item's header saves of it, by the dictionary's `is_stack`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Measure {
    /// A stack's `u16` amount.
    Amount,
    /// Any other item's `f32` condition.
    Condition,
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
    pub chamber: Chamber,
    pub ammo: Ammo,
}

/// Whether a weapon saves its chambered flag.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Chamber {
    /// It has a `chamber_a_round` state.
    Saved,
    NotSaved,
}

/// A weapon's ammo list (`weapon_core+0x5B0`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ammo {
    /// Its kit's: this many entries, the kit's ammo slots holding a
    /// compatible stack (at most 8).
    Kit(u8),
    /// A picked gun's: the picker's game made it of its own slots, live
    /// state the stream does not say.
    Picked,
}

/// A weapon state's class (`states.<name>.state_type` of the weapon's
/// config) by the bytes it saves (the cook's registrations
/// 0x24B9FF..0x30CE4F).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GunState {
    /// 302, 310, 313, 320 (idle, preview): a flag.
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
            302 | 310 | 313 | 320 => Self::Flag,
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
/// (0x32B039, 0x32B5C9).
const MAX_AFFECTS: usize = 8;
const MAX_HITS: usize = 64;
/// Patch 78's carry snapshot and its version, as its writer has them: a
/// new version is read here as soon as the game writes it.
const CARRY_SNAPSHOT: usize = artefact_carry::carry::Carry::SNAPSHOT_SIZE;
const CARRY_VERSION: u8 = artefact_carry::carry::Carry::SNAPSHOT_VERSION;

/// A fault: where in the state and why.
pub(super) type Fault = (usize, Reason);

pub(super) fn short(s: Short) -> Fault {
    (s.at, Reason::TailUnread { at: s.at })
}

/// A list of up to `N` records read from the world, held in place (no
/// allocation): what a view hands out for a count the stream says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct List<T: Copy, const N: usize> {
    items: [Option<T>; N],
    len: usize,
}

impl<T: Copy, const N: usize> List<T, N> {
    pub(super) const fn new() -> Self {
        Self {
            items: [None; N],
            len: 0,
        }
    }

    /// Adds `item`; `false` when the list is full.
    pub(super) fn push(&mut self, item: T) -> bool {
        match self.items.get_mut(self.len) {
            Some(slot) => {
                *slot = Some(item);
                self.len = self.len.saturating_add(1);
                true
            }
            None => false,
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn get(&self, index: usize) -> Option<T> {
        self.items.get(index).copied().flatten()
    }

    pub fn iter(&self) -> impl Iterator<Item = T> + '_ {
        self.items.iter().copied().flatten()
    }
}

/// A bool the game writes as 0x00 or 0xFF (the bool writer 0x80FA0).
pub(super) fn flag(reader: &mut Reader) -> Result<bool, Fault> {
    let at = reader.at();
    reader
        .bool()
        .map_err(short)?
        .ok_or((at, Reason::TailBool { at }))
}

/// An f32 that is a number.
pub(super) fn float(reader: &mut Reader) -> Result<Finite, Fault> {
    let at = reader.at();
    Finite::of_bits(reader.f32_bits().map_err(short)?).ok_or((at, Reason::TailNotNumber { at }))
}

/// `N` f32 that are numbers.
pub(super) fn floats<const N: usize>(reader: &mut Reader) -> Result<[Finite; N], Fault> {
    let mut out = [Finite::ZERO; N];
    for value in &mut out {
        *value = float(reader)?;
    }
    Ok(out)
}

pub(super) fn u8(reader: &mut Reader) -> Result<u8, Fault> {
    reader.u8().map_err(short)
}

pub(super) fn u32(reader: &mut Reader) -> Result<u32, Fault> {
    reader.u32().map_err(short)
}


/// A state machine's state id: one past its states is a null state, called
/// through (`fsm::deserialize` 0x38CCF0 -> 0x38CD26).
pub(super) fn state_id(reader: &mut Reader, states: u8) -> Result<u8, Fault> {
    let at = reader.at();
    let state = u8(reader)?;
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

/// An inventory slot, `0..23`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SlotId(u8);

impl SlotId {
    pub const fn new(slot: u8) -> Option<SlotId> {
        if slot < SLOTS { Some(SlotId(slot)) } else { None }
    }

    pub const fn get(self) -> u8 {
        self.0
    }
}

/// What the walk needs of the state's place.
pub(super) struct Owner<'a> {
    /// The places of the match and, by place, whose kits have a player.
    pub places: usize,
    pub usable: &'a dyn Fn(u8, u8) -> bool,
}

/// An object the walk expects in a slot.
#[derive(Clone, Copy)]
pub(super) enum Slot<'a> {
    /// One the state has.
    Held(Item<'a>),
    /// One it may have: an ammo slot of a picker (patch 85 moves the
    /// borrowed gun's stacks into them, live state the stream does not say).
    Maybe(Item<'a>),
}

/// What an item's header saves of how much is left of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Left {
    /// A stack's `m_amount`.
    Amount(u16),
    /// Any other item's `m_condition`.
    Condition(Finite),
}

/// An owned item's owner and slot: the player object it belongs to,
/// `[[core+0x18458] + place*0x30 + kit*4] + 0x10C` (looked up with no
/// bound, 0x2FFF72), and the slot it is in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ItemOwner {
    place: Place,
    kit: Kit,
    slot: SlotId,
}

impl ItemOwner {
    /// The owner's roster place (`player_id`).
    pub fn place(&self) -> Place {
        self.place
    }

    /// The owner's kit (`player_profile_id`).
    pub fn kit(&self) -> Kit {
        self.kit
    }

    /// The slot the item is in (`m_slot_id`).
    pub fn slot(&self) -> SlotId {
        self.slot
    }
}

/// A medkit's state (`medkit::deserialize` 0x33DCA0, writer 0x33DBD0).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Medkit<'a> {
    active: Option<MedkitUse<'a>>,
}

impl<'a> Medkit<'a> {
    /// Its use in progress, when `m_active`.
    pub fn active(&self) -> Option<MedkitUse<'a>> {
        self.active
    }
}

/// An active medkit's use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MedkitUse<'a> {
    activity_time_ms: u32,
    delay_ms: u32,
    applied_influences: &'a [u8],
}

impl<'a> MedkitUse<'a> {
    /// `m_activity_time_ms`.
    pub fn activity_time_ms(&self) -> u32 {
        self.activity_time_ms
    }

    /// `m_delay_ms`.
    pub fn delay_ms(&self) -> u32 {
        self.delay_ms
    }

    /// `m_applied_influence[i]`, one per `data.influences` entry, each a
    /// number (read again from the bytes the parse held to that).
    pub fn applied_influences(&self) -> impl Iterator<Item = Finite> + 'a {
        let mut reader = Reader::new(self.applied_influences, 0);
        core::iter::from_fn(move || float(&mut reader).ok())
    }
}

/// An oxygen tank's state (`oxygen_tank::deserialize` 0x33E7F0, writer
/// 0x33E790).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OxygenTank {
    active: bool,
    amount_ms: u32,
}

impl OxygenTank {
    /// `m_active`.
    pub fn active(&self) -> bool {
        self.active
    }

    /// `m_amount_ms`, what is left.
    pub fn amount_ms(&self) -> u32 {
        self.amount_ms
    }
}

/// An `m_active` of a medkit or an oxygen tank: active only with an owner,
/// whose inventory its reader takes with no null check (0x33DCFE,
/// 0x33E860).
fn active(reader: &mut Reader, owned: bool) -> Result<bool, Fault> {
    let at = reader.at();
    let active = flag(reader)?;
    if active && !owned {
        return Err((
            at,
            Reason::TailField {
                at,
                field: "an unowned item's m_active",
                value: 0xFF,
            },
        ));
    }
    Ok(active)
}

/// What an item saves after its header, read.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ItemBody<'a> {
    None,
    Medkit(Medkit<'a>),
    OxygenTank(OxygenTank),
    Weapon(WeaponState),
}

/// One item of a player's state, borrowed from the world.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ItemState<'a> {
    at: usize,
    bytes: &'a [u8],
    left: Left,
    owner: Option<ItemOwner>,
    body: ItemBody<'a>,
}

impl<'a> ItemState<'a> {
    /// Where it starts in the message.
    pub fn at(&self) -> usize {
        self.at
    }

    /// Its bytes in the world.
    pub fn bytes(&self) -> &'a [u8] {
        self.bytes
    }

    pub fn left(&self) -> Left {
        self.left
    }

    /// Its owner, `None` for an item no player owns (place 0xFF).
    pub fn owner(&self) -> Option<ItemOwner> {
        self.owner
    }

    pub fn body(&self) -> ItemBody<'a> {
        self.body
    }

    /// The header (`inventory_item::deserialize` 0x2FFF10): an amount or a
    /// condition by `measure`, the owner's place (0xFF none, then nothing
    /// more), kit and the slot.
    fn header(reader: &mut Reader, measure: Measure) -> Result<(Left, Option<ItemOwner>), Fault> {
        let left = match measure {
            Measure::Amount => Left::Amount(reader.u16().map_err(short)?),
            Measure::Condition => Left::Condition(float(reader)?),
        };
        let at = reader.at();
        let place = u8(reader)?;
        if place == 0xFF {
            return Ok((left, None));
        }
        let kit = u8(reader)?;
        let (Some(place), Some(kit)) = (Place::new(place), Kit::new(kit)) else {
            return Err((
                at,
                Reason::TailItemOwner {
                    player_id: place,
                    profile_id: kit,
                },
            ));
        };
        let at = reader.at();
        let slot = u8(reader)?;
        let slot = SlotId::new(slot).ok_or((
            at,
            Reason::TailField {
                at,
                field: "an item's m_slot_id",
                value: slot.into(),
            },
        ))?;
        Ok((left, Some(ItemOwner { place, kit, slot })))
    }

    /// The body of `body` in the weapon shape `shape`, after a header with
    /// an owner or (`owned` false) without.
    fn read_body(
        reader: &mut Reader<'a>,
        body: Body<'_>,
        shape: Shape,
        owned: bool,
    ) -> Result<ItemBody<'a>, Fault> {
        Ok(match body {
            Body::None => ItemBody::None,
            Body::Medkit { influences } => {
                let active = if active(reader, owned)? {
                    let activity_time_ms = u32(reader)?;
                    let delay_ms = u32(reader)?;
                    let from = *reader;
                    for _ in 0..influences {
                        float(reader)?;
                    }
                    Some(MedkitUse {
                        activity_time_ms,
                        delay_ms,
                        applied_influences: reader.since(&from),
                    })
                } else {
                    None
                };
                ItemBody::Medkit(Medkit { active })
            }
            Body::OxygenTank => ItemBody::OxygenTank(OxygenTank {
                active: active(reader, owned)?,
                amount_ms: u32(reader)?,
            }),
            Body::Weapon(gun) => ItemBody::Weapon(WeaponState::read(reader, gun, shape)?),
        })
    }

    /// The item the walk chose, read again from its bytes (the parse read
    /// it whole, so it reads).
    fn of(chosen: &Chosen<'a, '_>) -> Option<Self> {
        let mut reader = Reader::new(chosen.bytes, chosen.at);
        let (left, owner) = Self::header(&mut reader, chosen.item.measure).ok()?;
        let body = Self::read_body(&mut reader, chosen.item.body, chosen.shape, owner.is_some()).ok()?;
        Some(Self {
            at: chosen.at,
            bytes: chosen.bytes,
            left,
            owner,
            body,
        })
    }

    /// The rules the match sets on a read header at `at`: its owner is a
    /// player object of the match, and the item is in its own slot.
    fn check(
        at: usize,
        left: Left,
        own: Option<ItemOwner>,
        item: &Item,
        owner: &Owner,
    ) -> Result<(), Fault> {
        let Some(own) = own else {
            return Ok(());
        };
        let owner_at = at.saturating_add(match left {
            Left::Amount(_) => 2,
            Left::Condition(_) => 4,
        });
        let (place, kit) = (own.place.get(), own.kit.get());
        if usize::from(place) >= owner.places || !(owner.usable)(place, kit) {
            return Err((
                owner_at,
                Reason::TailItemOwner {
                    player_id: place,
                    profile_id: kit,
                },
            ));
        }
        let slot = own.slot.get();
        if slot != item.slot {
            return Err((
                owner_at.saturating_add(2),
                Reason::TailItemSlot {
                    slot,
                    wanted: item.slot,
                },
            ));
        }
        Ok(())
    }
}

/// The two shapes of a weapon: with a user, and the length of the ammo list
/// it reads with (a picked gun's is under two entries or two and more), or
/// without.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Shape {
    User { list: u8 },
    NoUser,
}

/// What the walk decided of a slot's item: its bytes, the kit's object
/// they are and the shape they read in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Chosen<'a, 'k> {
    at: usize,
    bytes: &'a [u8],
    item: Item<'k>,
    shape: Shape,
}

/// The items past the inventory header and the rest of the state, read to
/// its exact end, borrowed from the world: each slot's item as the walk
/// found it (where it is, of which kit object, in which shape), handed out
/// by reading it again, so that a check holds one item at a time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ItemsAndRest<'a, 'k> {
    items: [Option<Chosen<'a, 'k>>; SLOTS as usize],
    rest: AfterItems<'a>,
}

impl<'a, 'k> ItemsAndRest<'a, 'k> {
    /// The items, in slot order.
    pub fn items(&self) -> impl Iterator<Item = ItemState<'a>> + '_ {
        self.items.iter().flatten().filter_map(ItemState::of)
    }

    /// The items with the slot each was walked in, in slot order.
    pub fn by_slot(&self) -> impl Iterator<Item = (u8, ItemState<'a>)> + '_ {
        (0..SLOTS).zip(self.items.iter()).filter_map(|(slot, chosen)| {
            chosen.as_ref().and_then(ItemState::of).map(|item| (slot, item))
        })
    }

    pub fn rest(&self) -> &AfterItems<'a> {
        &self.rest
    }
}

/// The items past the inventory header, then the rest of the state, to its
/// exact end. `items` are the kit's slots in order, each with the object the
/// walk expects there (the slots the header's artefacts took have none);
/// `alive` the head's `m_is_alive`.
pub(super) fn items_and_rest<'a, 'k>(
    reader: &Reader<'a>,
    items: &[Option<Slot<'k>>],
    alive: bool,
    inventory: &Inventory,
    owner: &Owner,
) -> Result<ItemsAndRest<'a, 'k>, Fault> {
    let mut walk = Walk {
        alive,
        body_parts: inventory.body_parts,
        owner,
        furthest: None,
        chosen: [None; SLOTS as usize],
        rest: None,
    };
    match (walk.walk(*reader, items), walk.rest) {
        (true, Some(rest)) => Ok(ItemsAndRest {
            items: walk.chosen,
            rest,
        }),
        _ => Err(walk
            .furthest
            .unwrap_or((reader.at(), Reason::TailUnread { at: reader.at() }))),
    }
}

/// The backtracking over the shapes the stream does not say. Its frames,
/// one for each item, hold no read record: a body is read out of line and
/// only its end kept.
struct Walk<'w, 'a, 'k> {
    alive: bool,
    body_parts: usize,
    owner: &'w Owner<'w>,
    /// The furthest fault of the shapes tried.
    furthest: Option<Fault>,
    /// By slot, the item of the shapes being tried.
    chosen: [Option<Chosen<'a, 'k>>; SLOTS as usize],
    /// The rest of the state, once a shape read to its end.
    rest: Option<AfterItems<'a>>,
}

/// Where a body of `body` in `shape` ends, or why it does not read: read
/// and dropped out of line, so that no walk frame holds it.
#[inline(never)]
fn body_end<'a>(
    mut reader: Reader<'a>,
    body: Body<'_>,
    shape: Shape,
    owned: bool,
) -> Result<Reader<'a>, Fault> {
    ItemState::read_body(&mut reader, body, shape, owned).map(|_| reader)
}

impl<'a, 'k> Walk<'_, 'a, 'k> {
    /// The rest of the state after the last item, kept when it reads to the
    /// end. Out of line, so that the walk's own frames do not hold it.
    #[inline(never)]
    fn finish(&mut self, reader: Reader<'a>) -> bool {
        match AfterItems::read(reader, self.alive, self.body_parts) {
            Ok(after) => {
                self.rest = Some(after);
                true
            }
            Err(fault) => {
                self.keep(fault);
                false
            }
        }
    }

    fn keep(&mut self, fault: Fault) {
        if self.furthest.as_ref().is_none_or(|(at, _)| fault.0 >= *at) {
            self.furthest = Some(fault);
        }
    }

    /// Sets what the walk holds of `slot`'s item.
    fn choose(&mut self, slot: u8, chosen: Option<Chosen<'a, 'k>>) {
        if let Some(entry) = self.chosen.get_mut(usize::from(slot)) {
            *entry = chosen;
        }
    }

    /// Every shape the items can have (a weapon with a user or without, a
    /// picked gun's two ammo-list layouts): `true` when one reads the state
    /// to its exact end, the rest kept.
    fn walk(&mut self, reader: Reader<'a>, items: &[Option<Slot<'k>>]) -> bool {
        let Some((&slot, rest)) = items.split_first() else {
            return self.finish(reader);
        };
        let item = match slot {
            None => return self.walk(reader, rest),
            Some(Slot::Held(item)) => item,
            Some(Slot::Maybe(item)) => {
                self.choose(item.slot, None);
                if self.walk(reader, rest) {
                    return true;
                }
                item
            }
        };
        let shapes = match item.body {
            Body::Weapon(Gun {
                ammo: Ammo::Kit(list),
                ..
            }) => [Some(Shape::User { list }), Some(Shape::NoUser), None],
            Body::Weapon(Gun {
                ammo: Ammo::Picked, ..
            }) => [
                Some(Shape::User { list: 1 }),
                Some(Shape::User { list: MAX_AMMO }),
                Some(Shape::NoUser),
            ],
            _ => [Some(Shape::NoUser), None, None],
        };
        let at = reader.at();
        let mut after_header = reader;
        let header = ItemState::header(&mut after_header, item.measure).and_then(|(left, own)| {
            ItemState::check(at, left, own, &item, self.owner).map(|()| own.is_some())
        });
        let owned = match header {
            Ok(owned) => owned,
            Err(fault) => {
                self.keep(fault);
                return false;
            }
        };
        for shape in shapes.into_iter().flatten() {
            match body_end(after_header, item.body, shape, owned) {
                Ok(tried) => {
                    let chosen = Chosen {
                        at,
                        bytes: tried.since(&reader),
                        item,
                        shape,
                    };
                    self.choose(item.slot, Some(chosen));
                    if self.walk(tried, rest) {
                        return true;
                    }
                }
                Err(fault) => self.keep(fault),
            }
        }
        false
    }
}

/// An ammo index (`m_current_ammo_id`, `m_next_ammo_id`) of a list of
/// `count`: under it (a gun with none does not use it, any byte).
pub(super) fn ammo_id(reader: &mut Reader, count: u8) -> Result<u8, Fault> {
    let at = reader.at();
    let ammo = u8(reader)?;
    if count > 0 && ammo >= count {
        return Err((at, Reason::TailWeaponAmmo { ammo, count }));
    }
    Ok(ammo)
}

/// The most entries a weapon's ammo list holds when the stream does not
/// say (a picked gun's).
pub(super) const fn ammo_bound(ammo: Ammo) -> u8 {
    match ammo {
        Ammo::Kit(list) => list,
        Ammo::Picked => MAX_AMMO,
    }
}

/// `hit_affects_type_enum` (0.21d PDB): what a body part's affect is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AffectKind {
    Death,
    Bleeding,
    Concussion,
    HandDamage,
    LegDamage,
    CriticalPoisoning,
    Poisoning,
    RadiationSickness,
    Blindness,
}

impl AffectKind {
    /// The kind of a byte, if the enum has it (0..=8).
    pub const fn of(byte: u8) -> Option<AffectKind> {
        Some(match byte {
            0 => AffectKind::Death,
            1 => AffectKind::Bleeding,
            2 => AffectKind::Concussion,
            3 => AffectKind::HandDamage,
            4 => AffectKind::LegDamage,
            5 => AffectKind::CriticalPoisoning,
            6 => AffectKind::Poisoning,
            7 => AffectKind::RadiationSickness,
            8 => AffectKind::Blindness,
            _ => return None,
        })
    }
}

/// A body part's affect (`serialize_affect_functor` 0x32B720): its type
/// (`first`) and when it ends (`second`: its start + the type's duration).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Affect {
    kind: AffectKind,
    end_time_ms: u32,
}

impl Affect {
    pub fn kind(&self) -> AffectKind {
        self.kind
    }

    pub fn end_time_ms(&self) -> u32 {
        self.end_time_ms
    }
}

/// A hit a body part sums (its `hit_id` and the `damage` so far).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PartHit {
    hit_id: u32,
    damage: Finite,
}

impl PartHit {
    /// `hit_id`.
    pub fn hit_id(&self) -> u32 {
        self.hit_id
    }

    /// `damage`.
    pub fn damage(&self) -> Finite {
        self.damage
    }

    fn read(reader: &mut Reader) -> Result<Self, Fault> {
        Ok(Self {
            hit_id: u32(reader)?,
            damage: float(reader)?,
        })
    }
}

/// A body part's state (`body_part_parameters::deserialize` 0x32AFE0,
/// writer 0x32AE00), its hits borrowed from the world.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BodyPart<'a> {
    health: Finite,
    last_hit_time_ms: u32,
    affects: List<Affect, MAX_AFFECTS>,
    hits: &'a [u8],
    movement_speed_modifier: Finite,
}

impl<'a> BodyPart<'a> {
    /// `m_health`.
    pub fn health(&self) -> Finite {
        self.health
    }

    /// `m_last_hit_time` (0 never hit, LIKELY).
    pub fn last_hit_time_ms(&self) -> u32 {
        self.last_hit_time_ms
    }

    /// `m_affects`: at most 8 (the reader asserts past them, 0x32B039).
    pub fn affects(&self) -> &List<Affect, MAX_AFFECTS> {
        &self.affects
    }

    /// The hits: at most 64 (the writer drops the oldest, 0x32A1A8), read
    /// again from their bytes.
    pub fn hits(&self) -> impl Iterator<Item = PartHit> + 'a {
        let mut reader = Reader::new(self.hits, 0);
        core::iter::from_fn(move || PartHit::read(&mut reader).ok())
    }

    /// `m_movement_speed_modifier.modifier_value()`.
    pub fn movement_speed_modifier(&self) -> Finite {
        self.movement_speed_modifier
    }

    fn read(reader: &mut Reader<'a>) -> Result<Self, Fault> {
        let health = float(reader)?;
        let last_hit_time_ms = u32(reader)?;
        let at = reader.at();
        let count = u8(reader)?;
        let mut affects = List::new();
        for _ in 0..count {
            let kind_at = reader.at();
            let kind = u8(reader)?;
            let kind = AffectKind::of(kind).ok_or((kind_at, Reason::TailAffect { kind }))?;
            let affect = Affect {
                kind,
                end_time_ms: u32(reader)?,
            };
            if !affects.push(affect) {
                return Err((
                    at,
                    Reason::TailCount {
                        what: "affects",
                        count: count.into(),
                    },
                ));
            }
        }
        let at = reader.at();
        let count = u8(reader)?;
        if usize::from(count) > MAX_HITS {
            return Err((
                at,
                Reason::TailCount {
                    what: "hits",
                    count: count.into(),
                },
            ));
        }
        let from = *reader;
        for _ in 0..count {
            PartHit::read(reader)?;
        }
        Ok(Self {
            health,
            last_hit_time_ms,
            affects,
            hits: reader.since(&from),
            movement_speed_modifier: float(reader)?,
        })
    }
}

/// The damage model (`damage_model::deserialize` 0x2E5480, writer
/// 0x2E53B0): its damage protector's two fields, three of its own, then
/// the body parts, borrowed from the world.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DamageModel<'a> {
    have_second_chance: bool,
    last_hit_time_ms: u32,
    last_tick_time_ms: Option<u32>,
    invulnerability_time_ms: Option<u32>,
    invulnerability_attenuation: u32,
    body_parts: &'a [u8],
    body_parts_at: usize,
    body_part_count: usize,
}

impl<'a> DamageModel<'a> {
    /// The protector's `m_have_second_chance`.
    pub fn have_second_chance(&self) -> bool {
        self.have_second_chance
    }

    /// The protector's `m_last_hit_time_ms`.
    pub fn last_hit_time_ms(&self) -> u32 {
        self.last_hit_time_ms
    }

    /// `m_last_tick_time_in_ms`, `None` for 0xFFFFFFFF (its constructor's).
    pub fn last_tick_time_ms(&self) -> Option<u32> {
        self.last_tick_time_ms
    }

    /// `m_invulnerability_time_in_ms`, `None` for 0xFFFFFFFF.
    pub fn invulnerability_time_ms(&self) -> Option<u32> {
        self.invulnerability_time_ms
    }

    /// `m_invulnerability_attenuation` (a u32; its range is not traced).
    pub fn invulnerability_attenuation(&self) -> u32 {
        self.invulnerability_attenuation
    }

    /// How many body parts it has (the character's `hit_params`).
    pub fn body_part_count(&self) -> usize {
        self.body_part_count
    }

    /// `m_body_parts`, in the list's order, each read again from its bytes.
    pub fn body_parts(&self) -> impl Iterator<Item = BodyPart<'a>> + 'a {
        let mut reader = Reader::new(self.body_parts, self.body_parts_at);
        let mut left = self.body_part_count;
        core::iter::from_fn(move || {
            left = left.checked_sub(1)?;
            BodyPart::read(&mut reader).ok()
        })
    }
}

/// The usable-object user data (`usable_object_user_data::serialize`
/// 0x32FCC0), when `serialize_user_data`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Usable {
    start_using_time_ms: u32,
    head_offset: [Finite; 3],
    use_point: [Finite; 3],
    current_progress: Finite,
}

impl Usable {
    /// `start_using_time_ms`.
    pub fn start_using_time_ms(&self) -> u32 {
        self.start_using_time_ms
    }

    /// `head_offset`.
    pub fn head_offset(&self) -> [Finite; 3] {
        self.head_offset
    }

    /// `use_point`.
    pub fn use_point(&self) -> [Finite; 3] {
        self.use_point
    }

    /// `current_progress`.
    pub fn current_progress(&self) -> Finite {
        self.current_progress
    }
}

/// The character controller (`bullet_character_controller::serialize`
/// 0x7F7F0, 67 bytes), saved for a living player.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Controller {
    last_pos: [Finite; 3],
    jumping: bool,
    input_is_in_crouch: bool,
    logic_is_in_crouch: bool,
    capsule_is_in_crouch: bool,
    can_stand: bool,
    colliding_with_player: bool,
    fall_and_slide_velocity: [Finite; 3],
    supporting_surface_normal: [Finite; 3],
    blocking_ceiling_normal: [Finite; 3],
    blocking_ceiling_slide_velocity: [Finite; 3],
    is_sprinting: bool,
}

impl Controller {
    /// `m_last_pos` (+50h, stored raw at 0x7FB31..0x7FB65): where the
    /// controller's last step began, player_step's copy of the position at
    /// its start (0x7BE84). A number: exe patch 117 keeps the position
    /// finite (a revision-50 game could write NaN here, `snapshot_rules.md`
    /// 7.2.1).
    pub fn last_pos(&self) -> [Finite; 3] {
        self.last_pos
    }

    /// `m_jumping`.
    pub fn jumping(&self) -> bool {
        self.jumping
    }

    /// `m_input_is_in_crouch`.
    pub fn input_is_in_crouch(&self) -> bool {
        self.input_is_in_crouch
    }

    /// `m_logic_is_in_crouch`.
    pub fn logic_is_in_crouch(&self) -> bool {
        self.logic_is_in_crouch
    }

    /// `m_capsule_is_in_crouch`.
    pub fn capsule_is_in_crouch(&self) -> bool {
        self.capsule_is_in_crouch
    }

    /// `m_can_stand`.
    pub fn can_stand(&self) -> bool {
        self.can_stand
    }

    /// `m_colliding_with_player`.
    pub fn colliding_with_player(&self) -> bool {
        self.colliding_with_player
    }

    /// `m_fall_and_slide_velocity`.
    pub fn fall_and_slide_velocity(&self) -> [Finite; 3] {
        self.fall_and_slide_velocity
    }

    /// `m_supporting_surface_normal`.
    pub fn supporting_surface_normal(&self) -> [Finite; 3] {
        self.supporting_surface_normal
    }

    /// `m_blocking_ceiling_normal`.
    pub fn blocking_ceiling_normal(&self) -> [Finite; 3] {
        self.blocking_ceiling_normal
    }

    /// `m_blocking_ceiling_slide_velocity`.
    pub fn blocking_ceiling_slide_velocity(&self) -> [Finite; 3] {
        self.blocking_ceiling_slide_velocity
    }

    /// `m_is_sprinting`.
    pub fn is_sprinting(&self) -> bool {
        self.is_sprinting
    }

    fn read(reader: &mut Reader) -> Result<Self, Fault> {
        Ok(Self {
            last_pos: floats(reader)?,
            jumping: flag(reader)?,
            input_is_in_crouch: flag(reader)?,
            logic_is_in_crouch: flag(reader)?,
            capsule_is_in_crouch: flag(reader)?,
            can_stand: flag(reader)?,
            colliding_with_player: flag(reader)?,
            fall_and_slide_velocity: floats(reader)?,
            supporting_surface_normal: floats(reader)?,
            blocking_ceiling_normal: floats(reader)?,
            blocking_ceiling_slide_velocity: floats(reader)?,
            is_sprinting: flag(reader)?,
        })
    }
}

/// One of the player's recent hits (`m_recent_hits`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecentHit {
    timestamp_ms: u32,
    stopping_speed: Finite,
}

impl RecentHit {
    /// `timestamp`.
    pub fn timestamp_ms(&self) -> u32 {
        self.timestamp_ms
    }

    /// `stopping_speed`.
    pub fn stopping_speed(&self) -> Finite {
        self.stopping_speed
    }

    fn read(reader: &mut Reader) -> Result<Self, Fault> {
        Ok(Self {
            timestamp_ms: u32(reader)?,
            stopping_speed: float(reader)?,
        })
    }
}

/// Everything after the items, to the state's exact end
/// (`base_player::serialize` 0x2DAF60 after `inventory::serialize`),
/// borrowed from the world: its body parts and its recent hits are handed
/// out by reading them again.
///
/// ```text
/// u8    m_jump_type (jump_type_enum, 0..=12)    [`JumpType`]
/// f32   m_linear_horizontal_speed               a number
/// the damage model                               [`DamageModel`]
///   bool m_have_second_chance, u32 m_last_hit_time_ms (its protector),
///   u32 m_last_tick_time_in_ms, u32 m_invulnerability_time_in_ms (both
///   0xFFFFFFFF none), u32 m_invulnerability_attenuation, the body parts
///   ([`BodyPart`]: f32 m_health, u32 m_last_hit_time, u8 the affects <= 8
///   each u8 hit_affects_type_enum and its u32 end, u8 the hits <= 64 each
///   u32 hit_id and f32 damage, f32 m_movement_speed_modifier)
/// bool  serialize_user_data, then the usable data [`Usable`]
/// u32   m_inserted_time_in_ms (0xFFFFFFFF none)
/// the character controller, a living player's    [`Controller`]
/// u32   m_spotted_time_left_in_ms, u32 m_time_left_to_try_spot_in_ms
/// f32   m_stagger_speed_modifier.modifier_value()
/// u32   m_recent_hits.size(), each u32 timestamp, f32 stopping_speed
/// bool  m_respawn_disabled
/// 32    patch 78's carry snapshot, version 2
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AfterItems<'a> {
    jump_type: JumpType,
    linear_horizontal_speed: Finite,
    damage_model: DamageModel<'a>,
    usable: Option<Usable>,
    inserted_time_ms: Option<u32>,
    controller: Option<Controller>,
    spotted_time_left_ms: u32,
    time_left_to_try_spot_ms: u32,
    stagger_speed_modifier: Finite,
    recent_hits: &'a [u8],
    respawn_disabled: bool,
    carry: &'a [u8; CARRY_SNAPSHOT],
}

impl<'a> AfterItems<'a> {
    /// `m_jump_type`.
    pub fn jump_type(&self) -> JumpType {
        self.jump_type
    }

    /// `m_linear_horizontal_speed` (stored at 0x2DB844): the step's move
    /// length over its milliseconds (0x2D9507..0x2D9545; never 0 ms,
    /// `tick_events` skips those at 0x2D22DE). A number: it is NaN only for
    /// a NaN position, which exe patch 117 keeps from happening (a
    /// revision-50 game could write NaN here, `snapshot_rules.md` 7.2.1).
    pub fn linear_horizontal_speed(&self) -> Finite {
        self.linear_horizontal_speed
    }

    pub fn damage_model(&self) -> &DamageModel<'a> {
        &self.damage_model
    }

    /// The usable-object user data, when `serialize_user_data`.
    pub fn usable(&self) -> Option<Usable> {
        self.usable
    }

    /// `m_inserted_time_in_ms`, `None` before the player is inserted.
    pub fn inserted_time_ms(&self) -> Option<u32> {
        self.inserted_time_ms
    }

    /// The controller, saved for a living player only.
    pub fn controller(&self) -> Option<Controller> {
        self.controller
    }

    /// `m_spotted_time_left_in_ms`.
    pub fn spotted_time_left_ms(&self) -> u32 {
        self.spotted_time_left_ms
    }

    /// `m_time_left_to_try_spot_in_ms`.
    pub fn time_left_to_try_spot_ms(&self) -> u32 {
        self.time_left_to_try_spot_ms
    }

    /// `m_stagger_speed_modifier.modifier_value()`.
    pub fn stagger_speed_modifier(&self) -> Finite {
        self.stagger_speed_modifier
    }

    /// `m_recent_hits`, as many as the state has room for (the writer has
    /// no cap), read again from their bytes.
    pub fn recent_hits(&self) -> impl Iterator<Item = RecentHit> + 'a {
        let mut reader = Reader::new(self.recent_hits, 0);
        core::iter::from_fn(move || RecentHit::read(&mut reader).ok())
    }

    /// `m_respawn_disabled`.
    pub fn respawn_disabled(&self) -> bool {
        self.respawn_disabled
    }

    /// Patch 78's carry snapshot (`artefact_carry::carry::Carry::snapshot`),
    /// version 2.
    pub fn carry(&self) -> &'a [u8; CARRY_SNAPSHOT] {
        self.carry
    }

    fn read(mut reader: Reader<'a>, alive: bool, body_parts: usize) -> Result<Self, Fault> {
        let reader = &mut reader;
        let jump_type = jump_type(reader, "m_jump_type")?;
        let linear_horizontal_speed = float(reader)?;
        let have_second_chance = flag(reader)?;
        let last_hit_time_ms = u32(reader)?;
        let last_tick_time_ms = optional_u32(reader)?;
        let invulnerability_time_ms = optional_u32(reader)?;
        let invulnerability_attenuation = u32(reader)?;
        let parts = *reader;
        for _ in 0..body_parts {
            BodyPart::read(reader)?;
        }
        let damage_model = DamageModel {
            have_second_chance,
            last_hit_time_ms,
            last_tick_time_ms,
            invulnerability_time_ms,
            invulnerability_attenuation,
            body_parts: reader.since(&parts),
            body_parts_at: parts.at(),
            body_part_count: body_parts,
        };
        let usable = if flag(reader)? {
            Some(Usable {
                start_using_time_ms: u32(reader)?,
                head_offset: floats(reader)?,
                use_point: floats(reader)?,
                current_progress: float(reader)?,
            })
        } else {
            None
        };
        let inserted_time_ms = optional_u32(reader)?;
        let controller = if alive {
            Some(Controller::read(reader)?)
        } else {
            None
        };
        let spotted_time_left_ms = u32(reader)?;
        let time_left_to_try_spot_ms = u32(reader)?;
        let stagger_speed_modifier = float(reader)?;
        let at = reader.at();
        let count = u32(reader)?;
        let room = reader.rest().len() / 8;
        if usize::try_from(count).map_or(true, |count| count > room) {
            return Err((
                at,
                Reason::TailCount {
                    what: "recent hits",
                    count,
                },
            ));
        }
        let hits = *reader;
        for _ in 0..count {
            RecentHit::read(reader)?;
        }
        let recent_hits = reader.since(&hits);
        let respawn_disabled = flag(reader)?;
        let at = reader.at();
        let carry: &'a [u8; CARRY_SNAPSHOT] = reader
            .take(CARRY_SNAPSHOT)
            .map_err(short)?
            .try_into()
            .map_err(|_| (at, Reason::TailUnread { at }))?;
        if carry.first() != Some(&CARRY_VERSION) {
            return Err((at, Reason::TailCarry));
        }
        if !reader.is_done() {
            let at = reader.at();
            return Err((at, Reason::TailUnread { at }));
        }
        Ok(Self {
            jump_type,
            linear_horizontal_speed,
            damage_model,
            usable,
            inserted_time_ms,
            controller,
            spotted_time_left_ms,
            time_left_to_try_spot_ms,
            stagger_speed_modifier,
            recent_hits,
            respawn_disabled,
            carry,
        })
    }
}
