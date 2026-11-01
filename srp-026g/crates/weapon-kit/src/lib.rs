//! A 0.26g weapon's upgrades in the client's three upgrade records: the
//! weapon kit (`docs/notes/weapon-kit.md`). One encoding, shared by two
//! builds:
//! - srp's servers, which keep a weapon's upgrades in six logical slots and
//!   send them packed (`survarium::v026g::weapon_kit`, the lobby's inventory
//!   and a match's static info);
//! - the game itself, in the exe's arena (patch 68,
//!   `client-patches/rust/patches/src/weapon_kit.rs`), which expands the kit
//!   into the records its item config cook applies, and packs the lobby's
//!   preview of a change (patch 48) the same way.
//!
//! The client keeps at most [`RECORDS`] upgrade records an item (the item
//! record's inline array; a fourth is the fatal assert at 0x1F1655). A
//! weapon holds up to six things, one a [`Slot`]: two decals, a camouflage,
//! a scope, a muzzle device and a grip. On the wire: the decals as records
//! of their own, then one **kit** record in record slot [`KIT_RECORD_SLOT`]
//! for the rest. The kit is the first of scope, camouflage, muzzle, grip the
//! weapon holds (its item id and dict id, as that one's own record would
//! be); its value names the others by dict id:
//!
//! ```text
//! bits  0..15  the camouflage's dict id (0: none, or it is the kit's own)
//! bits 16..31  the muzzle device's
//! bits 32..47  the grip's
//! bits 48..55  the kit's own logical slot (2, 3, 4 or 5)
//! bits 56..63  KIT_MAGIC
//! ```
//!
//! A record is a kit only in record slot 3 with [`KIT_MAGIC`] on top; any
//! other is a plain record. A kit value that names its own slot, or an own
//! slot outside 2..=5, is [`Malformed`], as is a set of records that puts two
//! things in one slot or names a dict id another record holds (the client's
//! config would hit its duplicate-name assert; two records of one dict id,
//! the same decal twice, are the stock game's). The client refuses a malformed kit by
//! building the item from its records as they are (the stock way).
//!
//! `no_std`, no `alloc`, no `unsafe`, and no read can panic: a panic in the
//! game is a crash.

#![no_std]
#![forbid(unsafe_code)]
#![cfg_attr(
    not(test),
    deny(
        clippy::indexing_slicing,
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::arithmetic_side_effects
    )
)]

#[cfg(test)]
extern crate std;

#[cfg(test)]
mod tests;

use core::num::NonZeroU16;

/// The upgrade records the client keeps an item (the item record's inline
/// array, +6Ch..+B4h of its 0xB4 bytes; the reader's assert at 0x1F1655).
pub const RECORDS: usize = 3;
/// The value's top byte that makes a record of [`KIT_RECORD_SLOT`] a kit.
pub const KIT_MAGIC: u8 = 0xA7;
/// The record slot the kit goes in (the scope's).
pub const KIT_RECORD_SLOT: u8 = 3;
/// The records an expanded kit gives the item config cook at most: the
/// records and one for each slot the kit names.
pub const EXPANDED: usize = RECORDS + KitSlot::ALL.len() - 1;

/// A weapon's logical upgrade slot (a record's `slot` byte).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Slot {
    Decal0 = 0,
    Decal1 = 1,
    Camo = 2,
    Scope = 3,
    Muzzle = 4,
    Grip = 5,
}

impl Slot {
    /// Every slot, in order.
    pub const ALL: [Slot; 6] = [
        Slot::Decal0,
        Slot::Decal1,
        Slot::Camo,
        Slot::Scope,
        Slot::Muzzle,
        Slot::Grip,
    ];

    /// The slot a record's byte names (None: no weapon slot).
    pub fn from_byte(byte: u8) -> Option<Slot> {
        Slot::ALL.into_iter().find(|slot| *slot as u8 == byte)
    }

    /// The slot's byte, as a record holds it.
    pub fn byte(self) -> u8 {
        self as u8
    }
}

/// A slot a kit covers: the kit's own is one of them, the value names the
/// others.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KitSlot {
    Camo,
    Scope,
    Muzzle,
    Grip,
}

impl KitSlot {
    /// The order the kit's own is chosen in: the first the weapon holds.
    pub const ALL: [KitSlot; 4] = [
        KitSlot::Scope,
        KitSlot::Camo,
        KitSlot::Muzzle,
        KitSlot::Grip,
    ];
    /// The slots the value names, each with the bit its dict id begins at.
    const NAMED: [(KitSlot, u32); 3] = [
        (KitSlot::Camo, 0),
        (KitSlot::Muzzle, 16),
        (KitSlot::Grip, 32),
    ];

    pub fn slot(self) -> Slot {
        match self {
            KitSlot::Camo => Slot::Camo,
            KitSlot::Scope => Slot::Scope,
            KitSlot::Muzzle => Slot::Muzzle,
            KitSlot::Grip => Slot::Grip,
        }
    }

    fn of(slot: Slot) -> Option<KitSlot> {
        KitSlot::ALL.into_iter().find(|kit| kit.slot() == slot)
    }
}

/// An item's dict id a kit names (never 0: 0 names nothing).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DictId(NonZeroU16);

impl DictId {
    /// A dict id (None for 0).
    pub fn new(id: u16) -> Option<DictId> {
        NonZeroU16::new(id).map(DictId)
    }

    pub fn get(self) -> u16 {
        self.0.get()
    }
}

/// An upgrade record as the client holds it (0x18 bytes: u32 the upgrade's
/// item id, u32 its dict id, u64 its value, u8 its slot) and srp sends it
/// (`lobby::Upgrade`, a match's static info).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Record {
    pub id: u32,
    pub dict: u32,
    pub value: u64,
    pub slot: u8,
}

/// Why records are not a weapon's: what the client refuses (and srp never
/// sends).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Malformed {
    /// A kit's own slot byte is not 2..=5.
    OwnSlot(u8),
    /// A kit's value names a dict id for its own slot.
    NamesItsOwn(KitSlot),
    /// More than [`RECORDS`] records.
    TooMany,
    /// More than one kit.
    TwoKits,
    /// Two things in one logical slot.
    SlotTwice(Slot),
    /// A kit names a dict id another record holds (the config's duplicate
    /// modification).
    DictTwice(u32),
    /// A plain record's slot is no weapon slot.
    NoSlot(u8),
}

/// A kit record's value: the kit's own slot and the dict ids it names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Kit {
    own: KitSlot,
    /// By [`KitSlot::NAMED`]: camouflage, muzzle, grip; never the own's.
    named: [Option<DictId>; 3],
}

impl Kit {
    /// A kit whose own is `own`, naming nothing yet.
    fn new(own: KitSlot) -> Kit {
        Kit {
            own,
            named: [None; 3],
        }
    }

    /// The kit's own slot.
    pub fn own(&self) -> KitSlot {
        self.own
    }

    /// This kit naming `dict` for `slot`; refused for the own's slot and
    /// the scope (the scope is never named: it is always the own when held).
    fn naming(mut self, slot: KitSlot, dict: DictId) -> Result<Kit, Malformed> {
        if slot == self.own {
            return Err(Malformed::NamesItsOwn(slot));
        }
        let at = KitSlot::NAMED.iter().position(|(named, _)| *named == slot);
        match at.and_then(|at| self.named.get_mut(at)) {
            Some(place) => *place = Some(dict),
            None => return Err(Malformed::NamesItsOwn(slot)),
        }
        Ok(self)
    }

    /// The slots it names and their dict ids, in the value's order.
    pub fn named(&self) -> impl Iterator<Item = (KitSlot, DictId)> + '_ {
        KitSlot::NAMED
            .iter()
            .zip(self.named.iter())
            .filter_map(|((slot, _), dict)| dict.map(|dict| (*slot, dict)))
    }

    /// The record's value.
    pub fn value(&self) -> u64 {
        let mut value = u64::from(KIT_MAGIC) << 56 | u64::from(self.own.slot().byte()) << 48;
        for ((_, shift), dict) in KitSlot::NAMED.iter().zip(self.named.iter()) {
            if let Some(dict) = dict {
                value |= u64::from(dict.get()) << shift;
            }
        }
        value
    }

    /// The kit a record holds: None for a plain record (not in record slot
    /// 3, or no [`KIT_MAGIC`]).
    pub fn of(record: &Record) -> Result<Option<Kit>, Malformed> {
        if record.slot != KIT_RECORD_SLOT || (record.value >> 56) as u8 != KIT_MAGIC {
            return Ok(None);
        }
        let byte = (record.value >> 48) as u8;
        let own = Slot::from_byte(byte)
            .and_then(KitSlot::of)
            .ok_or(Malformed::OwnSlot(byte))?;
        let mut kit = Kit::new(own);
        for (slot, shift) in KitSlot::NAMED {
            if let Some(dict) = DictId::new((record.value >> shift) as u16) {
                kit = kit.naming(slot, dict)?;
            }
        }
        Ok(Some(kit))
    }
}

/// What a logical slot holds: an item of the account (its record), or a
/// dict id a kit names (the client knows no item id for it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Held {
    Item { id: u32, dict: u32, value: u64 },
    Named(DictId),
}

impl Held {
    /// The dict id it holds.
    pub fn dict(&self) -> u32 {
        match *self {
            Held::Item { dict, .. } => dict,
            Held::Named(dict) => u32::from(dict.get()),
        }
    }

    /// It is the upgrade `id` of dict id `dict`: its item, or a kit's name
    /// of that dict id.
    pub fn is(&self, id: u32, dict: u32) -> bool {
        match *self {
            Held::Item { id: own, .. } => own == id,
            Held::Named(named) => u32::from(named.get()) == dict,
        }
    }

    /// Its value (a decal's place; 0 for a name).
    pub fn value(&self) -> u64 {
        match *self {
            Held::Item { value, .. } => value,
            Held::Named(_) => 0,
        }
    }
}

/// A weapon's upgrades by logical slot, one a slot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Logical([Option<Held>; 6]);

/// Records packed for the client: at most [`RECORDS`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Wire {
    records: [Record; RECORDS],
    len: usize,
}

impl Wire {
    pub fn as_slice(&self) -> &[Record] {
        self.records.get(..self.len).unwrap_or(&[])
    }

    fn push(&mut self, record: Record) {
        if let Some(place) = self.records.get_mut(self.len) {
            *place = record;
            self.len = self.len.saturating_add(1);
        }
    }
}

impl Logical {
    pub fn new() -> Logical {
        Logical::default()
    }

    /// What `slot` holds.
    pub fn get(&self, slot: Slot) -> Option<Held> {
        self.0.get(usize::from(slot.byte())).copied().flatten()
    }

    /// `slot` holds `held` (or nothing).
    pub fn set(&mut self, slot: Slot, held: Option<Held>) {
        if let Some(place) = self.0.get_mut(usize::from(slot.byte())) {
            *place = held;
        }
    }

    /// Each slot that holds something, in slot order.
    pub fn iter(&self) -> impl Iterator<Item = (Slot, Held)> + '_ {
        Slot::ALL
            .into_iter()
            .filter_map(|slot| self.get(slot).map(|held| (slot, held)))
    }

    /// The upgrade `id` of `dict` goes into `slot`, as the server attaches
    /// one (`session::v026g::items_rules::attach_upgrade_in`): already there
    /// (that slot, that dict id), nothing changes; else it comes off where it
    /// was (a decal keeps its place, its value) and what `slot` held comes
    /// off.
    pub fn put(&mut self, slot: Slot, id: u32, dict: u32) {
        if self
            .get(slot)
            .is_some_and(|held| held.is(id, dict) && held.dict() == dict)
        {
            return;
        }
        let value = self
            .iter()
            .find(|(_, held)| held.is(id, dict) && held.dict() == dict)
            .map_or(0, |(_, held)| held.value());
        self.take_off(id, dict);
        self.set(slot, Some(Held::Item { id, dict, value }));
    }

    /// The upgrade `id` of `dict` comes off wherever it is.
    pub fn take_off(&mut self, id: u32, dict: u32) {
        for slot in Slot::ALL {
            if self.get(slot).is_some_and(|held| held.is(id, dict)) {
                self.set(slot, None);
            }
        }
    }

    /// The records the client is sent: the decals in slot order, then the
    /// kit (the first of [`KitSlot::ALL`] held is its own: its item id and
    /// dict id; it names the others). At most [`RECORDS`].
    pub fn pack(&self) -> Wire {
        let mut wire = Wire::default();
        for slot in [Slot::Decal0, Slot::Decal1] {
            if let Some(held) = self.get(slot) {
                wire.push(record_of(held, slot.byte()));
            }
        }
        let own = KitSlot::ALL
            .into_iter()
            .find_map(|kit| self.get(kit.slot()).map(|held| (kit, held)));
        if let Some((own, held)) = own {
            let mut kit = Kit::new(own);
            for (slot, _) in KitSlot::NAMED {
                let named = self
                    .get(slot.slot())
                    .and_then(|held| u16::try_from(held.dict()).ok());
                if let (true, Some(dict)) = (slot != own, named.and_then(DictId::new)) {
                    // `slot` is not the own: naming it cannot fail.
                    kit = kit.naming(slot, dict).unwrap_or(kit);
                }
            }
            wire.push(Record {
                value: kit.value(),
                ..record_of(held, KIT_RECORD_SLOT)
            });
        }
        wire
    }

    /// Records as the client holds them, by logical slot: a kit's own in its
    /// own slot (value 0), each dict id it names [`Held::Named`]; a plain
    /// record in its slot.
    pub fn unpack(records: &[Record]) -> Result<Logical, Malformed> {
        if records.len() > RECORDS {
            return Err(Malformed::TooMany);
        }
        let mut logical = Logical::new();
        let mut kits = 0u8;
        let put = |logical: &mut Logical, slot: Slot, held: Held| {
            if logical.get(slot).is_some() {
                return Err(Malformed::SlotTwice(slot));
            }
            // A name beside a record of its dict id: the config would get
            // that modification twice (two `camouflage`s, its duplicate-name
            // assert). Two records of one dict id (the same decal twice) are
            // the stock game's, as on armour.
            let named = |held: &Held| matches!(held, Held::Named(_));
            if logical
                .iter()
                .any(|(_, other)| other.dict() == held.dict() && (named(&held) || named(&other)))
            {
                return Err(Malformed::DictTwice(held.dict()));
            }
            logical.set(slot, Some(held));
            Ok(())
        };
        for record in records {
            let item = |value| Held::Item {
                id: record.id,
                dict: record.dict,
                value,
            };
            match Kit::of(record)? {
                None => {
                    let slot =
                        Slot::from_byte(record.slot).ok_or(Malformed::NoSlot(record.slot))?;
                    put(&mut logical, slot, item(record.value))?;
                }
                Some(kit) => {
                    kits = kits.saturating_add(1);
                    if kits > 1 {
                        return Err(Malformed::TwoKits);
                    }
                    put(&mut logical, kit.own().slot(), item(0))?;
                    for (slot, dict) in kit.named() {
                        put(&mut logical, slot.slot(), Held::Named(dict))?;
                    }
                }
            }
        }
        Ok(logical)
    }
}

/// `held` as a record of record slot `slot` (a name has no item id).
fn record_of(held: Held, slot: u8) -> Record {
    match held {
        Held::Item { id, dict, value } => Record {
            id,
            dict,
            value,
            slot,
        },
        Held::Named(dict) => Record {
            id: 0,
            dict: u32::from(dict.get()),
            value: 0,
            slot,
        },
    }
}

/// The records the item config cook applies for an item's `records`: None
/// when they hold no kit (the cook takes the item's own, as the stock does);
/// else the records with the kit's own record valued 0 (as the thing's own
/// record was before the kit) and, after them, one `{0, dict id, 0, slot}`
/// for each dict id the kit names that `known` (the items dictionary) has: a
/// dict id the game data lacks would be read from a null entry (0x2F18E3).
/// Malformed records are refused: the cook takes them as they are.
pub fn expand(
    records: &[Record],
    known: impl Fn(u16) -> bool,
) -> Result<Option<Expanded>, Malformed> {
    if records.len() > RECORDS {
        return Err(Malformed::TooMany);
    }
    let mut kit = None;
    for record in records {
        if let Some(found) = Kit::of(record)?
            && kit.replace(found).is_some()
        {
            return Err(Malformed::TwoKits);
        }
    }
    let Some(kit) = kit else { return Ok(None) };
    // The whole set is a weapon's (no slot twice, no dict id twice).
    Logical::unpack(records)?;
    let mut out = Expanded::default();
    for record in records {
        let own = Kit::of(record).ok().flatten().is_some();
        out.push(if own {
            Record {
                value: 0,
                ..*record
            }
        } else {
            *record
        });
    }
    for (slot, dict) in kit.named() {
        if known(dict.get()) {
            out.push(record_of(Held::Named(dict), slot.slot().byte()));
        }
    }
    Ok(Some(out))
}

/// The records an expanded kit gives the item config cook: at most
/// [`EXPANDED`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Expanded {
    records: [Record; EXPANDED],
    len: usize,
}

impl Expanded {
    pub fn as_slice(&self) -> &[Record] {
        self.records.get(..self.len).unwrap_or(&[])
    }

    fn push(&mut self, record: Record) {
        if let Some(place) = self.records.get_mut(self.len) {
            *place = record;
            self.len = self.len.saturating_add(1);
        }
    }
}
