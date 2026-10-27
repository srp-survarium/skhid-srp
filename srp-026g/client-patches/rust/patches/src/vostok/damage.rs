//! A player's damage model: its parts (the pools pain, body, head, ..),
//! found by name as the game finds them (find_body_part 0x2E58B0, engine row
//! engine_find_pool), and their health.

use core::ffi::CStr;
use core::marker::PhantomData;

use super::hook::Thread;
use super::object::{Object, pointer, read};
use crate::raw;

/// A part of a damage model, by the name the game's configs give it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PartName {
    /// The pain pool: emptied, the player falls of pain shock.
    Pain,
    Body,
    Head,
}

impl PartName {
    fn text(self) -> &'static CStr {
        match self {
            PartName::Pain => c"pain",
            PartName::Body => c"body",
            PartName::Head => c"head",
        }
    }
}

/// A player's damage model (base_player+2E0h).
#[derive(Clone, Copy)]
pub(in crate::vostok) struct DamageModel<'t> {
    model: Object,
    thread: PhantomData<&'t Thread>,
}

impl<'t> DamageModel<'t> {
    /// The player's damage model, or none.
    ///
    /// # Safety
    /// `player` is a live base_player.
    pub(in crate::vostok) unsafe fn of(player: Object) -> Option<DamageModel<'t>> {
        // SAFETY: the caller's; +2E0h is the player's damage model or 0.
        unsafe { pointer(raw::base_player::damage_model(player.ptr())) }
            .map(|model| DamageModel { model, thread: PhantomData })
    }

    /// The part named `name`, or none: the game's own look-up of its parts
    /// (the list +108h) by name.
    pub(in crate::vostok) fn part(self, name: PartName) -> Option<BodyPart<'t>> {
        let text = name.text().as_ptr().cast::<u8>();
        // SAFETY: a live damage model (of's contract, on the hook's thread);
        // the look-up takes its parts' address and the address of a name's
        // pointer, and only reads both.
        let part = unsafe { raw::engine_find_pool(raw::damage_model::parts(self.model.ptr()).cast(), &text) };
        Object::at(part as usize as u32).map(|part| BodyPart { part, thread: PhantomData })
    }
}

/// A part of a damage model.
pub struct BodyPart<'t> {
    part: Object,
    thread: PhantomData<&'t Thread>,
}

impl BodyPart<'_> {
    /// Whether it has health left: its bits, as an int, above 0 (a float
    /// above 0 for every health the game keeps, clamped at 0; -0.0 is
    /// none).
    pub fn has_health(&self) -> bool {
        // SAFETY: a part the look-up found, live for the hook; its health
        // read as its bits.
        unsafe { read(raw::body_part::health(self.part.ptr()).cast::<i32>()) > 0 }
    }
}
