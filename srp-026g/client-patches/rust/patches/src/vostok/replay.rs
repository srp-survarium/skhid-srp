//! The replay viewer (README "24 to 26"): what a replay's seek walks, and a
//! client weapon's delayed sounds and fx history.
//!
//! At `replay.seek` (sites_026g.tsv; the seek 0x1EE2E0 on a user seek
//! alone, after the clock went to the target) ESI is the game, EDI the
//! target time. What those registers hold is said here, once.

use core::ffi::c_void;
use core::marker::PhantomData;

use super::context::{Context, Reg};
use super::hook::Thread;
use super::object::{Object, pointer, read, vtable, write};
use crate::raw;

/// A place of the world's array (game_world+18650h), in bytes.
const PLACE_SIZE: u32 = 0x30;
/// A place's kits, a player each.
const KITS: usize = 3;
/// An inventory's item slots.
const SLOTS: usize = 23;

/// The game a replay's seek runs in.
pub struct ReplaySeek<'t> {
    game: Option<Object>,
    thread: PhantomData<&'t Thread>,
}

impl<'t> ReplaySeek<'t> {
    /// The seek at the site: its game (ESI).
    pub fn at(ctx: &'t Context<'_>) -> ReplaySeek<'t> {
        ReplaySeek { game: Object::at(ctx.reg(Reg::Esi)), thread: PhantomData }
    }

    /// Every client weapon in the inventories of every place's three kits:
    /// place by place, kit 0 to 2, slot 0 to 22, an item whose vtable is a
    /// client weapon's item's; a place's kit without a player, or a player
    /// without an inventory, has none. Read as the game holds them while `f`
    /// runs (the places' end once, at the start).
    pub fn for_each_client_weapon(&self, mut f: impl FnMut(ClientWeapon<'t>)) {
        // SAFETY: the seek's game, whose +4D64h is its world.
        let Some(world) = self.game.and_then(|game| unsafe { pointer(raw::game::world(game.ptr())) }) else {
            return;
        };
        // SAFETY: the world's places' array, its end and its first.
        let (end, mut place) =
            unsafe { (read(raw::game_world::places_end(world.ptr())), read(raw::game_world::places_begin(world.ptr()))) };
        while place < end {
            for kit in 0..KITS {
                // SAFETY: a place of the array (below its end): its kits'
                // players at +0, +4, +8, a player's inventory at +10Ch.
                let inventory = unsafe {
                    Object::at(place)
                        .and_then(|at| pointer(raw::place::players(at.ptr()).add(kit)))
                        .and_then(|player| pointer(raw::base_player::inventory(player.ptr())))
                };
                let Some(inventory) = inventory else { continue };
                for slot in 0..SLOTS {
                    // SAFETY: an inventory's 23 slots, each an item or 0;
                    // an item's first dword its vtable.
                    let item = unsafe { pointer(raw::inventory::slots(inventory.ptr()).add(slot)) };
                    let Some(item) = item else { continue };
                    // SAFETY: as above.
                    if unsafe { vtable(item) } == raw::image::CLIENT_WEAPON_ITEM_VTABLE {
                        f(ClientWeapon { item, thread: PhantomData });
                    }
                }
            }
            place = place.wrapping_add(PLACE_SIZE);
        }
    }
}

/// A client weapon, by its item (weapon+10h) as an inventory slot holds it.
pub struct ClientWeapon<'t> {
    item: Object,
    thread: PhantomData<&'t Thread>,
}

impl ClientWeapon<'_> {
    /// Its delayed sounds go, as ~weapon (0x2438BC) lets them go: their
    /// functors destroyed (0x245C50 on begin..&end, which plays none), then
    /// the queue empty (end = begin).
    pub fn drop_delayed_sounds(&self) {
        let at = self.item.ptr();
        // SAFETY: a client weapon's item (its vtable): its queue's begin and
        // end, and the game's own destroy of that range.
        unsafe {
            let begin = read(raw::client_weapon_item::delayed_sounds_begin(at));
            let end = raw::client_weapon_item::delayed_sounds_end(at);
            raw::engine_delayed_sounds_destroy(begin as usize as *mut c_void, end.cast());
            write(end, read(raw::client_weapon_item::delayed_sounds_begin(at)));
        }
    }

    /// Its fx history is empty: the write index is the oldest.
    pub fn empty_fx_history(&self) {
        let at = self.item.ptr();
        // SAFETY: a client weapon's item: the fx history's two indices.
        unsafe {
            write(raw::client_weapon_item::fx_history_write(at), read(raw::client_weapon_item::fx_history_oldest(at)));
        }
    }
}
