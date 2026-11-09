//! The character's look (patch 123, docs/notes/face-choice.md):
//! profile_skin_visual_cook::on_configs_loaded (0x204560) fills a
//! skeleton_combined_cook_data with a part record per model of the kit's
//! armour (record 1 on, each tagged by its slot), then the head's record 0,
//! names the combined skin by the kit (the resources' cache key) and asks for
//! it. At `skin_cook.head` (0x205662) the head is still to be made (no item's
//! `remove_head_model` took record 0); its handler makes it and goes on at
//! the join `made` (0x2056F0), the native head's end.
//!
//! The cook runs for the lobby's characters and a match's players alike, the
//! profile its query's (a lobby profile or a match place: the player's name
//! at +8h either way).

use core::ffi::CStr;

use super::config::{ConfigError, ConfigTable, ConfigValue};
use super::context::Context;
use super::hook::Thread;
use super::ui::c_string;
use super::world::read;
use crate::raw;

/// A player's name: 64 bytes with its zero.
const NAME_LIMIT: usize = 64;
/// The cook data's records (0x127C30 constructs 20; on_configs_loaded never
/// checks its count against them).
const PARTS: usize = 20;
/// A part record's tag ([`raw::SkeletonCombinedPart::tag`]): the head's, and
/// 1 + the slot table's index (0xA79870) of the helmet's (profile slot 0, the
/// table's fifth) and the mask's (slot 1, its sixth).
const HEAD: u32 = 0;
const HELMET: u32 = 5;
const MASK: u32 = 6;

/// What the kit's models cover of the head, by its records.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cover {
    /// A mask's model (a mask wins over a helmet, as 0.69's cook).
    Mask,
    /// A helmet's model, no mask's.
    Helmet,
    Bare,
}

impl Cover {
    /// The cover of these records' tags.
    pub fn of_tags(tags: impl Iterator<Item = u32>) -> Cover {
        tags.fold(Cover::Bare, |cover, tag| match (cover, tag) {
            (_, MASK) | (Cover::Mask, _) => Cover::Mask,
            (_, HELMET) => Cover::Helmet,
            (cover, _) => cover,
        })
    }
}

/// A model as a part's config gives it (an item's `data`, a head's
/// variant, an entry of `additional_models`): its path and part, and the
/// optional material.
#[derive(Clone, Copy, Debug)]
pub struct Model<'c> {
    pub base_model: &'c CStr,
    pub part_name: &'c CStr,
    pub material: Option<&'c CStr>,
}

impl<'c> Model<'c> {
    /// The model of `table`: `base_model` and `part_name` texts, `material`
    /// one when there.
    pub fn of(table: &ConfigTable<'c>) -> Result<Model<'c>, ConfigError> {
        Ok(Model {
            base_model: table.required(c"base_model")?.text()?,
            part_name: table.required(c"part_name")?.text()?,
            material: table.get(c"material")?.map(ConfigValue::text).transpose()?,
        })
    }
}

/// The cook data holds 20 records: no room for another model.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Full;

/// The skin's name has no room for more.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NameFull;

/// The head to make, at `skin_cook.head`.
pub struct HeadCook<'t> {
    frame: raw::SkinCookFrame,
    data: raw::SkeletonCombinedCookData,
    profile: raw::PlayerProfile,
    thread: &'t Thread,
}

impl<'t> HeadCook<'t> {
    pub fn at(ctx: &'t Context<'_>) -> Option<HeadCook<'t>> {
        use raw::site::skin_cook_head as site;
        // SAFETY: the context is this site's: on_configs_loaded's frame (by
        // ESP and EBP) while it runs, its cook data and profile alive for it.
        let (frame, profile) = unsafe { (site::frame(ctx)?, site::args(ctx)?.profile()?) };
        // SAFETY: the frame's slot, written at 0x2045B4 (0 when the
        // allocation failed, which this refuses).
        let data = unsafe { frame.cook_data()? };
        Some(HeadCook { frame, data, profile, thread: ctx.thread() })
    }

    /// The player's name, the profile's.
    pub fn player_name(&self) -> Option<&'t CStr> {
        // SAFETY: the query's profile, alive while the cook runs; its name a
        // C string in its 64 bytes.
        unsafe { c_string(self.profile.name(), NAME_LIMIT) }
    }

    /// gameplay/bodyparts/default's `body_parts`, as the cook read it.
    pub fn body_parts(&self) -> Option<ConfigTable<'t>> {
        let _ = self.thread;
        // SAFETY: the frame's copy of the node; its children are the config
        // resource's, alive and immutable while the cook runs.
        unsafe { ConfigValue::read(self.frame.body_parts()) }.ok()?.table().ok()
    }

    fn count(&self) -> usize {
        // SAFETY: the cook data's own count, for the hook.
        usize::from(unsafe { read(self.data.part_count()) })
    }

    /// What the records made so far (from 1) cover of the head.
    pub fn cover(&self) -> Cover {
        let parts = self.count().min(PARTS);
        // SAFETY: records 1 to the count, made; each its own tag.
        Cover::of_tags((1..parts).map(|i| unsafe { read(self.data.parts().at(i).tag()) }))
    }

    /// Record `index` made as `model`, tagged the head's.
    fn write(&mut self, index: usize, model: &Model<'_>) {
        // SAFETY: a record of the 20, the cook data the hook's; each
        // buffer_string its record's own room.
        unsafe {
            let part = self.data.parts().at(index);
            assign(part.base_model(), model.base_model.to_bytes());
            assign(part.part_name(), model.part_name.to_bytes());
            if let Some(material) = model.material {
                assign(part.material(), material.to_bytes());
            }
            *part.tag() = HEAD;
        }
    }

    /// The head's record (0) made as `model`, as the native head's
    /// (0x205659..0x2056E6).
    pub fn make_head(&mut self, model: &Model<'_>) {
        self.write(0, model);
    }

    /// Another record of the head (its hair), as an item's additional model
    /// (0x205520..0x205607).
    pub fn add_head_part(&mut self, model: &Model<'_>) -> Result<(), Full> {
        let index = self.count();
        if index >= PARTS {
            return Err(Full);
        }
        self.write(index, model);
        // SAFETY: the cook data's count, below the 20.
        unsafe { *self.data.part_count() = (index + 1) as u8 };
        Ok(())
    }

    /// `text` added to the skin's name (its cache key), as the slots' `0_`
    /// (0x204A7A), when its room takes it all.
    pub fn name_skin(&mut self, text: &[u8]) -> Result<(), NameFull> {
        // SAFETY: the frame's name buffer, alive while the cook runs.
        unsafe { append(self.frame.skin_name(), text) }
    }
}

/// `text` into `string`, as buffer_string::assign (0x129BB0): from its
/// start, while its room lasts, then its zero (always inside the room: the
/// native's of a full one is at its capacity, past it).
///
/// # Safety
/// `string` is a live buffer_string over its own room.
unsafe fn assign(string: raw::BufferString, text: &[u8]) {
    // SAFETY: the caller's.
    unsafe {
        let (begin, capacity) = (string.begin(), string.capacity());
        let room = (capacity as usize).saturating_sub(begin as usize);
        let len = text.len().min(room.saturating_sub(1));
        core::ptr::copy_nonoverlapping(text.as_ptr(), begin, len);
        *begin.add(len) = 0;
        string.set_end(begin.add(len));
    }
}

/// `text` at `string`'s end, its zero after, when the room takes it all.
///
/// # Safety
/// `string` is a live buffer_string over its own room.
unsafe fn append(string: raw::BufferString, text: &[u8]) -> Result<(), NameFull> {
    // SAFETY: the caller's.
    unsafe {
        let (end, capacity) = (string.end(), string.capacity());
        if (capacity as usize).saturating_sub(end as usize) <= text.len() {
            return Err(NameFull);
        }
        core::ptr::copy_nonoverlapping(text.as_ptr(), end, text.len());
        *end.add(text.len()) = 0;
        string.set_end(end.add(text.len()));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mask_wins_over_a_helmet() {
        assert_eq!(Cover::of_tags([2, 5, 6, 1].into_iter()), Cover::Mask);
        assert_eq!(Cover::of_tags([6, 5].into_iter()), Cover::Mask);
        assert_eq!(Cover::of_tags([2, 5, 7].into_iter()), Cover::Helmet);
        assert_eq!(Cover::of_tags([1, 2, 3, 4, 7].into_iter()), Cover::Bare);
        assert_eq!(Cover::of_tags(core::iter::empty()), Cover::Bare);
    }
}
