//! A character's visual as profile_skin_visual_cook::on_configs_loaded
//! (0x204560) builds it, at `skin_visual.parts_done` (0x2056F0: its parts and
//! the head done, before the combined skin is queried by the visual's name at
//! 0x20579F): the cook data's part records (skeleton_combined_cook_data, 20
//! records of C0Ch at +234h, +F324h in use), the visual's name (the
//! resource's cache key) and the profile the visual is of.
//!
//! The profile is read once into a [`Wearer`]; each record's kind and texts
//! into a [`PartKeys`]. A record is added only into the cook data's own room
//! (at most [`RECORDS`]), its texts by the game's buffer_string::assign.

use core::ffi::CStr;
use core::marker::PhantomData;

use super::context::Context;
use super::hook::Thread;
use crate::raw;

/// The part records a cook data has room for (0x127C30: 14h of them).
pub const RECORDS: usize = 20;
/// A record's text room, and the visual name's: 104h bytes, the zero included.
pub const TEXT_ROOM: usize = 0x104;

/// srp's Blackout band, in the profile's protection field (every value from
/// 10 up protects nothing, so the match computes the same): 11 blue (team 0),
/// 12 red (team 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Band {
    Blue,
    Red,
}

impl Band {
    pub const fn from_protection(value: u32) -> Option<Band> {
        match value {
            11 => Some(Band::Blue),
            12 => Some(Band::Red),
            _ => None,
        }
    }
}

/// Whom a visual is of, as its profile says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wearer {
    /// The game's own player (the profile's is_local byte; in first person
    /// what is on his arms would be at the camera).
    Own,
    /// Another player, with srp's band or none (the lobby's and other
    /// matches' profiles say 10).
    Player { band: Option<Band> },
}

/// A record's part (its +C08h): 0 the head, else 1 + the slot table's index
/// (0xA79870: gloves, torso, boots, pants, helmet, mask, back).
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PartKind {
    Head = 0,
    Gloves = 1,
    Torso = 2,
    Boots = 3,
    Pants = 4,
    Helmet = 5,
    Mask = 6,
    Back = 7,
}

impl PartKind {
    pub const fn from_raw(raw: u32) -> Option<PartKind> {
        Some(match raw {
            0 => PartKind::Head,
            1 => PartKind::Gloves,
            2 => PartKind::Torso,
            3 => PartKind::Boots,
            4 => PartKind::Pants,
            5 => PartKind::Helmet,
            6 => PartKind::Mask,
            7 => PartKind::Back,
            _ => return None,
        })
    }
}

/// A record as the cook made it: its part and its model's texts (no zero).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PartKeys<'v> {
    pub kind: PartKind,
    pub base_model: &'v [u8],
    pub part_name: &'v [u8],
}

/// A record to add: its model, surface and material, of a part.
pub struct NewPart<'a> {
    pub kind: PartKind,
    pub base_model: &'a CStr,
    pub part_name: &'a CStr,
    pub material: &'a CStr,
}

/// The cook data has no room for another record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Full;

/// The visual being cooked, at `skin_visual.parts_done`.
pub struct SkinVisual<'t> {
    data: raw::SkeletonCombinedCookData,
    name: raw::BufferString,
    profile: raw::PlayerProfile,
    _thread: PhantomData<&'t Thread>,
}

/// A buffer_string's text: begin..end, inside its room of at most
/// [`TEXT_ROOM`] bytes (else none: not a string here).
///
/// # Safety
/// `string` is alive for `'v`, its text not written meanwhile.
unsafe fn text<'v>(string: raw::BufferString) -> Option<&'v [u8]> {
    // SAFETY: the caller's: a live buffer_string.
    let (begin, end, limit) = unsafe { (string.begin(), string.end(), string.limit()) };
    let (b, e, l) = (begin as usize, end as usize, limit as usize);
    if begin.is_null() || e < b || l < e || l - b > TEXT_ROOM {
        return None;
    }
    // SAFETY: begin..end lies in the string's own room, alive for 'v.
    Some(unsafe { core::slice::from_raw_parts(begin, e - b) })
}

impl<'t> SkinVisual<'t> {
    /// At `skin_visual.parts_done`.
    pub fn at_parts_done(ctx: &'t Context<'_>) -> Option<SkinVisual<'t>> {
        use raw::site::skin_visual_parts_done as site;
        // SAFETY: the context is this site's: on_configs_loaded's own frame
        // (ESP: the cook data it made, its name's buffer_string) and its
        // arguments (EBP: the profile it was asked for, read by the function
        // itself at 0x205196), alive while it runs on the game's thread.
        let (data, name, profile) = unsafe {
            let frame = site::frame(ctx)?;
            (frame.cook_data()?, frame.name(), site::args(ctx)?.profile()?)
        };
        Some(SkinVisual { data, name, profile, _thread: PhantomData })
    }

    /// Whom the visual is of: its profile's is_local byte and protection.
    pub fn wearer(&self) -> Wearer {
        // SAFETY: the profile is alive for the hook (on_configs_loaded reads it).
        let (local, protection) = unsafe { (*self.profile.is_local(), *self.profile.protection()) };
        if local != 0 {
            return Wearer::Own;
        }
        Wearer::Player { band: Band::from_protection(protection) }
    }

    /// The records in use (at most [`RECORDS`]).
    pub fn count(&self) -> usize {
        // SAFETY: the cook data is alive for the hook.
        usize::from(unsafe { *self.data.count() }).min(RECORDS)
    }

    fn record(&self, index: usize) -> Option<raw::SkeletonCombinedPart> {
        // SAFETY: `index` is below RECORDS: one of the cook data's 20 records.
        (index < RECORDS).then(|| unsafe { self.data.records().at(index) })
    }

    /// Record `index` (below [`count`](Self::count)): its part and texts;
    /// none for a kind or a text that is not the game's.
    pub fn part(&self, index: usize) -> Option<PartKeys<'_>> {
        if index >= self.count() {
            return None;
        }
        let record = self.record(index)?;
        // SAFETY: a record of the live cook data; its texts are not written
        // while `self` is borrowed (add_part takes it mutably).
        unsafe {
            Some(PartKeys {
                kind: PartKind::from_raw(*record.kind())?,
                base_model: text(record.base_model())?,
                part_name: text(record.part_name())?,
            })
        }
    }

    /// One more record after those in use: `part`'s model, surface,
    /// material and kind, never baked (no camouflage, no decals).
    pub fn add_part(&mut self, part: &NewPart<'_>) -> Result<(), Full> {
        let index = self.count();
        let record = self.record(index).ok_or(Full)?;
        // SAFETY: a constructed record of the live cook data past those in
        // use (the cook data is made for this visual, 0x2045AB); its strings
        // are buffer_strings over their own rooms, which assign keeps to;
        // the count stays below RECORDS.
        unsafe {
            raw::engine_string_assign(Some(record.base_model()), part.base_model.as_ptr().cast());
            raw::engine_string_assign(Some(record.part_name()), part.part_name.as_ptr().cast());
            raw::engine_string_assign(Some(record.material()), part.material.as_ptr().cast());
            *record.kind() = part.kind as u32;
            *record.bakes() = 0;
            *self.data.count() = (index + 1) as u8;
        }
        Ok(())
    }

    /// `mark` at the end of the visual's name (its cache key): after the
    /// name, or, when the room is short, in place of its last bytes, so the
    /// whole mark always ends it. Nothing when the room is shorter than the
    /// mark.
    pub fn mark_name(&mut self, mark: &[u8]) {
        // SAFETY: the frame's name, a live buffer_string over its own room.
        let Some(len) = (unsafe { text(self.name) }).map(<[u8]>::len) else { return };
        // SAFETY: as above.
        let (begin, limit) = unsafe { (self.name.begin(), self.name.limit()) };
        // The text and its zero stay below limit (the room's last byte is not ours).
        let room = (limit as usize - begin as usize).saturating_sub(1);
        let Some(at) = room.checked_sub(mark.len()).map(|most| len.min(most)) else { return };
        // SAFETY: begin + at + mark.len() < limit: inside the room.
        unsafe {
            for (i, &byte) in mark.iter().enumerate() {
                *begin.add(at + i) = byte;
            }
            let end = begin.add(at + mark.len());
            *end = 0;
            self.name.set_end(end);
        }
    }
}
