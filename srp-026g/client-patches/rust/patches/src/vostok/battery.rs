//! A battery: survarium::victory_item_core (and the game's victory_item over
//! it, the same fields and the same is_ready_to_be_deactivated 0x301270 in
//! both vtables), as patch 80 reads it.

use super::hook::{Handle, Thread};
use super::world::{Object, Player, pointer};
use crate::raw;

/// A battery a hook was given (`victory_item.is_ready`'s ECX). A [`Handle`]:
/// the generated shim makes it; a patch receives it.
pub struct VictoryItem<'t> {
    this: Object,
    thread: &'t Thread,
}

impl<'t> Handle<'t> for VictoryItem<'t> {
    unsafe fn from_raw(thread: &'t Thread, raw: u32) -> Option<Self> {
        Object::at(raw).map(|this| VictoryItem { this, thread })
    }
}

impl<'t> VictoryItem<'t> {
    pub(in crate::vostok) fn ptr(&self) -> *mut core::ffi::c_void {
        self.this.ptr()
    }

    /// The player carrying it (+8, its user), if any.
    pub fn carrier(&self) -> Option<Player<'t>> {
        // SAFETY: a battery's user field: a player or 0 (set_user 0x300B00,
        // deactivate 0x300F40 makes it 0 last).
        let user = unsafe { pointer(raw::victory_item_core::user(self.this.ptr()).cast())? };
        Some(Player::of(user, self.thread))
    }
}
