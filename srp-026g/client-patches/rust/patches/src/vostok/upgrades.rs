//! An item's upgrade records, where patch 68 (the weapon kit,
//! `docs/notes/weapon-kit.md`) meets them:
//!
//! - [`CookQuery`]: item_config_cook::translate_query after it copied its
//!   query (`weapon_kit.query_records`, 0x2F1776), the frame's [ebp+40h]
//!   the item's records, ESP patch 68's room below the frame
//!   (`weapon_kit.query_room` made it);
//! - [`FlashUpgrade`]: the lobby's upgrades to Flash after a record's object
//!   has its `param` (`weapon_kit.flash_upgrade`, 0x2882E3);
//! - [`AttachedPairs`]: setup_inventory_data after it pushed a record's
//!   {item id, host id} pair (`weapon_kit.attached_pair`, 0x2885CE);
//! - [`PreviewApply`]: patch 48's apply of its queue to the window's
//!   weapon (the entry `weapon_kit.preview_apply`, a generated one patch
//!   48's NASM calls through the dword the build writes,
//!   `UPGRADE_PREVIEW_KIT_APPLY`).
//!
//! Records are read once into `weapon_kit::Record`s; a list of more than
//! three (no item of the game's holds one) is not read.

use core::ffi::CStr;

use weapon_kit::{Expanded, Logical, RECORDS, Record, Wire};

use super::context::Context;
use super::hook::Thread;
use super::ui::GFX_UINT;
use crate::raw;

/// Patch 48's "taken off" (its queue's slot).
pub const TAKE_OFF: u32 = 0xFF;
/// Patch 48's queue holds at most this many (its MAX_OPS).
const MAX_CHANGES: usize = 5;

/// An item's upgrade records as read: at most [`RECORDS`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Records {
    records: [Record; RECORDS],
    len: usize,
}

impl Records {
    pub fn as_slice(&self) -> &[Record] {
        self.records.get(..self.len).unwrap_or(&[])
    }
}

/// The records of `list` (None when it is broken or holds more than three).
///
/// # Safety
/// `list` is a live buffer_vector of upgrade records.
unsafe fn read(list: raw::UpgradeRecords) -> Option<Records> {
    // SAFETY: the caller's.
    let (begin, end) = unsafe { (list.begin(), list.end()) };
    let mut out = Records::default();
    let (begin, end) = match (begin, end) {
        (None, None) => return Some(out),
        (Some(begin), Some(end)) => (begin, end),
        _ => return None,
    };
    out.len = end.index_from(begin)?;
    if out.len > RECORDS {
        return None;
    }
    for (i, place) in out.records.iter_mut().take(out.len).enumerate() {
        // SAFETY: begin..end are the vector's records, `i` before end.
        *place = unsafe { record_at(begin.at(i)) };
    }
    Some(out)
}

/// # Safety
/// `record` is a live upgrade record.
unsafe fn record_at(record: raw::UpgradeRecord) -> Record {
    // SAFETY: the caller's.
    unsafe {
        Record {
            id: *record.id(),
            dict: *record.dict(),
            value: u64::from(*record.value_high()) << 32 | u64::from(*record.value_low()),
            slot: *record.slot(),
        }
    }
}

/// # Safety
/// `place` is an upgrade record's room, ours or the item's.
unsafe fn write(place: raw::UpgradeRecord, record: &Record) {
    // SAFETY: the caller's.
    unsafe {
        *place.id() = record.id;
        *place.dict() = record.dict;
        *place.value_low() = record.value as u32;
        *place.value_high() = (record.value >> 32) as u32;
        *place.slot() = record.slot;
    }
}

/// translate_query at `weapon_kit.query_records`: the item's records, the
/// items dictionary, and patch 68's room on the cook's stack.
pub struct CookQuery<'t> {
    room: raw::WeaponKitRoom,
    frame: raw::TranslateQueryFrame,
    _thread: &'t Thread,
}

/// The records the room holds ([`weapon_kit::EXPANDED`]).
const ROOM: usize = weapon_kit::EXPANDED;

impl<'t> CookQuery<'t> {
    /// None also when 0x2F2060 did not copy the query (AL 0): the frame's
    /// [ebp+40h] is not the item's then.
    pub fn at_records(ctx: &'t Context<'_>) -> Option<CookQuery<'t>> {
        use raw::site::weapon_kit_query_records as site;
        // SAFETY: the context is this site's: AL 0x2F2060's answer,
        // translate_query's frame (EBP), and ESP patch 68's room, which
        // weapon_kit.query_room took on the cook's stack just before; both
        // alive while the cook runs on this thread.
        let (room, frame) = unsafe {
            if site::copied(ctx) & 0xFF == 0 {
                return None;
            }
            (site::room(ctx)?, site::frame(ctx)?)
        };
        Some(CookQuery { room, frame, _thread: ctx.thread() })
    }

    /// The item's records (the query's), None when not readable.
    fn records(&self) -> Option<Records> {
        // SAFETY: the frame's [ebp+40h], the item record's own vector, alive
        // for the cook (0x2F2060 copied it from the query).
        unsafe { read(self.frame.upgrades()?) }
    }

    /// The items dictionary has an entry of `dict` (a record of a dict id
    /// it lacks is read from a null entry, 0x2F18E3).
    fn knows(&self, dict: u32) -> bool {
        // SAFETY: the frame's dictionary, the game's, alive for the cook;
        // item_by_id only looks the id up (0x1F0090).
        u16::try_from(dict).is_ok_and(|_| unsafe {
            raw::engine_item_by_id(self.frame.dictionary(), dict).is_some()
        })
    }

    /// The cook applies the item's records with what their kit names
    /// (`weapon_kit::expand`, each name the dictionary has): written into
    /// the room, [ebp+40h] pointed at it (the cook reads it at 0x2F1799 and
    /// 0x2F18D0 alone); the item's own records stay as they are. Whether it
    /// did: not for records without a kit, a malformed one, or a list with
    /// a dict id the dictionary lacks (the stock way then, as the original).
    pub fn apply_kit(&mut self) -> bool {
        let Some(records) = self.records() else { return false };
        let Ok(Some(expanded)) = weapon_kit::expand(records.as_slice(), |dict| self.knows(u32::from(dict))) else {
            return false;
        };
        self.give(&expanded)
    }

    /// `expanded` into the room, when the dictionary has every dict id of
    /// it (the room holds [`ROOM`], every expansion fits).
    fn give(&mut self, expanded: &Expanded) -> bool {
        let records = expanded.as_slice();
        if records.len() > ROOM || !records.iter().all(|record| self.knows(record.dict)) {
            return false;
        }
        // SAFETY: the room is ours (ROOM records and their vector), on the
        // cook's stack below its frame until it returns; the frame's
        // [ebp+40h] is its own local.
        unsafe {
            let first = self.room.records();
            for (i, record) in records.iter().enumerate() {
                write(first.at(i), record);
            }
            let list = self.room.list();
            list.set_begin(Some(first));
            list.set_end(Some(first.at(records.len())));
            list.set_capacity(Some(first.at(records.len())));
            self.frame.set_upgrades(Some(list));
        }
        true
    }
}

/// The lobby's upgrades to Flash at `weapon_kit.flash_upgrade`: an item's
/// record, and the object made for it.
pub struct FlashUpgrade<'t> {
    item: raw::ItemRecord,
    frame: raw::UpgradesFlashFrame,
    _thread: &'t Thread,
}

impl<'t> FlashUpgrade<'t> {
    pub fn at(ctx: &'t Context<'_>) -> Option<FlashUpgrade<'t>> {
        use raw::site::weapon_kit_flash_upgrade as site;
        // SAFETY: the context is this site's: the item record (ESI) and the
        // function's stack (ESP), alive while it runs on the lobby's thread.
        let (item, frame) = unsafe { (site::item(ctx)?, site::frame(ctx)?) };
        Some(FlashUpgrade { item, frame, _thread: ctx.thread() })
    }

    /// The record the object is made of.
    pub fn record(&self) -> Option<Record> {
        // SAFETY: the item's live records; the index the loop's.
        unsafe {
            let index = usize::from(*self.frame.index());
            read(self.item.upgrades())?.as_slice().get(index).copied()
        }
    }

    /// The object's member `name` is the uint `value` (SetMember of the AS3
    /// object interface, as the site's own `id` and `param`); false when the
    /// object is no AS3 movie's (nothing set) or the movie refused.
    pub fn set_uint(&self, name: &CStr, value: u32) -> bool {
        let mut room = raw::GfxValueStorage::zeroed();
        let given = room.handle();
        // SAFETY: the object is the site's live GFx value; its interface is
        // called only when its vtable is the AS3 interface's, whose SetMember
        // 0x7C6A20 is that slot; the name a C string; the value ours, a uint.
        unsafe {
            let object = self.frame.object();
            let Some(interface) = object.interface() else { return false };
            if *interface.vtable() != raw::image::AS3_OBJECT_INTERFACE {
                return false;
            }
            *given.kind() = GFX_UINT;
            *given.value() = value;
            let display = u32::from(*object.kind() & 0x8F == 0x0A);
            raw::engine_gfx_set_member(Some(interface), object.object(), name.as_ptr().cast(), Some(given), display)
                != 0
        }
    }
}

/// setup_inventory_data at `weapon_kit.attached_pair`: the host item, the
/// record whose pair was just pushed, the inventory and the pairs.
pub struct AttachedPairs<'t> {
    host: raw::ItemRecord,
    frame: raw::SetupInventoryFrame,
    _thread: &'t Thread,
}

impl<'t> AttachedPairs<'t> {
    pub fn at(ctx: &'t Context<'_>) -> Option<AttachedPairs<'t>> {
        use raw::site::weapon_kit_attached_pair as site;
        // SAFETY: the context is this site's: the host item (EBX) and the
        // function's frame (EBP), alive while it runs on the lobby's thread.
        let (host, frame) = unsafe { (site::host(ctx)?, site::frame(ctx)?) };
        Some(AttachedPairs { host, frame, _thread: ctx.thread() })
    }

    /// The record whose pair was just pushed.
    pub fn record(&self) -> Option<Record> {
        // SAFETY: the host's live records; the frame's index of the loop.
        unsafe {
            let index = usize::from(*self.frame.index());
            read(self.host.upgrades())?.as_slice().get(index).copied()
        }
    }

    /// The inventory's item of `dict` with the lowest item id (as the lobby
    /// movie's InventoryList.getInventoryItemOf), None when it holds none.
    pub fn item_of(&self, dict: u16) -> Option<u32> {
        // SAFETY: the inventory's vector and its end, the frame's, alive for
        // the function; begin..end are its item records.
        unsafe {
            let begin = self.frame.inventory()?.begin()?;
            let count = self.frame.inventory_end()?.index_from(begin)?;
            (0..count)
                .map(|i| begin.at(i))
                .filter(|item| *item.dict() == dict && *item.id() != 0)
                .map(|item| *item.id())
                .min()
        }
    }

    /// One more pair, {`item`, the host}, when the pairs have room (the
    /// game's push asserts without); whether it went in.
    pub fn push(&mut self, item: u32) -> bool {
        // SAFETY: the frame's pairs vector (its alloca, ten pairs an
        // inventory item); a pair is written only before its capacity end.
        unsafe {
            let pairs = self.frame.pairs();
            let (Some(end), Some(capacity)) = (pairs.end(), pairs.capacity()) else { return false };
            if capacity.index_from(end).is_none_or(|room| room == 0) {
                return false;
            }
            *end.item() = item;
            *end.host() = *self.frame.host();
            pairs.set_end(Some(end.at(1)));
            true
        }
    }
}

/// A change patch 48 queued for the window's weapon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreviewChange {
    /// The upgrade goes into the slot.
    Put { upgrade: u32, dict: u32, slot: u8 },
    /// The upgrade comes off.
    TakeOff { upgrade: u32, dict: u32 },
}

impl PreviewChange {
    fn dict(&self) -> u32 {
        match *self {
            PreviewChange::Put { dict, .. } | PreviewChange::TakeOff { dict, .. } => dict,
        }
    }
}

/// Patch 48's apply at `weapon_kit.preview_apply` (upgrade_preview_026g.asm
/// `apply`, through its dword +23h, which the build writes with this
/// entry): the window's weapon's record, which patch 48 builds the lobby's
/// character from, and the changes it queued, read once.
pub struct PreviewApply<'t> {
    item: raw::ItemRecord,
    changes: [Option<PreviewChange>; MAX_CHANGES],
    _thread: &'t Thread,
}

impl<'t> PreviewApply<'t> {
    pub fn at(ctx: &'t Context<'_>) -> Option<PreviewApply<'t>> {
        // SAFETY: the context is this entry's: its arguments by the site's
        // ESP, patch 48's scratch record and its state's queue of `count`
        // changes (at most five, 0Ch each), alive for the call on the lobby's
        // thread.
        let (item, changes) = unsafe {
            let frame = raw::site::weapon_kit_preview_apply::frame(ctx)?;
            let item = frame.item()?;
            let count = (*frame.count() as usize).min(MAX_CHANGES);
            let mut changes = [None; MAX_CHANGES];
            if let Some(first) = frame.changes() {
                for (i, place) in changes.iter_mut().take(count).enumerate() {
                    let change = first.at(i);
                    let (upgrade, dict, slot) = (*change.upgrade(), *change.dict(), *change.slot());
                    *place = Some(match slot {
                        TAKE_OFF => PreviewChange::TakeOff { upgrade, dict },
                        slot => PreviewChange::Put { upgrade, dict, slot: u8::try_from(slot).unwrap_or(u8::MAX) },
                    });
                }
            }
            (item, changes)
        };
        Some(PreviewApply { item, changes, _thread: ctx.thread() })
    }

    /// The record's upgrade records.
    pub fn records(&self) -> Option<Records> {
        // SAFETY: patch 48's live scratch record.
        unsafe { read(self.item.upgrades()) }
    }

    /// The queued changes, oldest first.
    pub fn changes(&self) -> impl Iterator<Item = PreviewChange> + '_ {
        self.changes.iter().flatten().copied()
    }

    /// The record holds `wire` (packed by `weapon_kit::Logical::pack`, at
    /// most three). Only dict ids the record held (its records and what its
    /// kit names) or a queued change named
    /// go in (the game's own, which patch 48 found: a dict id the items
    /// dictionary lacks would be read from a null entry by the cook,
    /// 0x2F18E3); else nothing is written. Whether it was.
    pub fn set(&mut self, wire: &Wire) -> bool {
        let records = wire.as_slice();
        let held = self.records().and_then(|held| Logical::unpack(held.as_slice()).ok());
        let given = |dict: u32| {
            held.is_some_and(|held| held.iter().any(|(_, held)| held.dict() == dict))
                || self.changes().any(|change| change.dict() == dict)
        };
        if !records.iter().all(|record| given(record.dict)) {
            return false;
        }
        // SAFETY: the record's own vector and inline room: written up to its
        // capacity end only.
        unsafe {
            let list = self.item.upgrades();
            let Some(begin) = list.begin() else { return false };
            let room = list.capacity().and_then(|end| end.index_from(begin)).unwrap_or(0).min(RECORDS);
            if records.len() > room {
                return false;
            }
            for (i, record) in records.iter().enumerate() {
                write(begin.at(i), record);
            }
            list.set_end(Some(begin.at(records.len())));
        }
        true
    }
}
