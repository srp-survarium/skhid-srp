//! A player (survarium::base_player) and the weapons of its inventory: the
//! place and kit it plays, its inventory's 23 slots (an item each, or none)
//! and, of those, the items that are weapons, with the weapon_core fields
//! the patches set.

use core::marker::PhantomData;

use super::hook::Thread;
use super::object::{Object, pointer, read, vtable, write};
use crate::raw;

/// The slots of an inventory (+10Ch..+164h).
pub const INVENTORY_SLOTS: u32 = 23;

/// A player the game handed a hook (a register at a site says which).
#[derive(Clone, Copy)]
pub struct Player<'t> {
    this: Object,
    thread: PhantomData<&'t Thread>,
}

impl<'t> Player<'t> {
    /// # Safety
    /// `this` is a live survarium::base_player, for the hook's thread.
    pub(in crate::vostok) unsafe fn new(this: Object) -> Player<'t> {
        Player { this, thread: PhantomData }
    }

    /// Its place, the byte as the game holds it.
    pub fn place(&self) -> u8 {
        // SAFETY: a live player (new's contract).
        unsafe { read(raw::base_player::place(self.this.ptr())) }
    }

    /// Its kit of the place, the byte as the game holds it.
    pub fn kit(&self) -> u8 {
        // SAFETY: as place.
        unsafe { read(raw::base_player::kit(self.this.ptr())) }
    }

    /// The weapons of its inventory, by slot (0..23), as the game's slots
    /// hold them now: none without an inventory. The inventory pointer is
    /// read once; each slot when the walk comes to it.
    pub fn weapons(&self) -> Weapons<'t> {
        // SAFETY: as place; the inventory is the player's or null.
        let inventory = unsafe { pointer(raw::base_player::inventory(self.this.ptr())) };
        Weapons { inventory, slot: 0, thread: PhantomData }
    }
}

/// The walk of an inventory's weapons ([`Player::weapons`]).
pub struct Weapons<'t> {
    inventory: Option<Object>,
    slot: u32,
    thread: PhantomData<&'t Thread>,
}

impl<'t> Iterator for Weapons<'t> {
    /// (the slot, its weapon)
    type Item = (u32, Weapon<'t>);

    fn next(&mut self) -> Option<(u32, Weapon<'t>)> {
        let inventory = self.inventory?;
        while self.slot < INVENTORY_SLOTS {
            let slot = self.slot;
            self.slot += 1;
            // SAFETY: a live inventory's slot `slot` (< 23) of its item
            // pointers; an item has a vtable whose +70h is its bind.
            let weapon = unsafe {
                let Some(item) = pointer(raw::inventory::slots(inventory.ptr()).add(slot as usize)) else {
                    continue;
                };
                let Some(table) = Object::at(vtable(item)) else {
                    continue;
                };
                if read(raw::item_vtable::bind(table.ptr())) != raw::image::WEAPON_CORE_BIND {
                    continue;
                }
                Object::at(raw::weapon_item::core(item.ptr()) as usize as u32)
            };
            if let Some(core) = weapon {
                return Some((slot, Weapon { core, thread: PhantomData }));
            }
        }
        None
    }
}

/// A weapon of an inventory: the weapon_core whose item a slot holds (the
/// item's bind is weapon_core's).
pub struct Weapon<'t> {
    core: Object,
    thread: PhantomData<&'t Thread>,
}

impl Weapon<'_> {
    /// Sets the state its spread draws from (minstd_rand's).
    pub fn set_random_state(&self, state: u32) {
        // SAFETY: a live weapon_core (Weapons found it by its item's bind).
        unsafe { write(raw::weapon_core::random_state(self.core.ptr()), state) }
    }

    /// Forgets the time of its last tick (the constructor's -1): its
    /// activation sets it again before it is ticked.
    pub fn forget_last_tick(&self) {
        // SAFETY: as set_random_state.
        unsafe { write(raw::weapon_core::last_tick_time(self.core.ptr()), u32::MAX) }
    }

    /// Gives the fields a weapon keeps through deaths and spawns and that
    /// neither the player's initialize nor the weapon's activation set
    /// again (or only for the weapon put in hand) the constructor's values
    /// (0x2DF0D0): the ammunition switch's time, the ammunition kind, the
    /// magazine, the fire mode, the aim, +5ECh, the chamber, the auto
    /// reload, the recoil and +60Ch.
    pub fn reset_as_constructed(&self) {
        let core = self.core.ptr();
        // SAFETY: as set_random_state; each field is the weapon_core's.
        unsafe {
            write(raw::weapon_core::ammo_switch_time(core), u32::MAX);
            write(raw::weapon_core::current_ammo(core), 0);
            write(raw::weapon_core::next_ammo(core), 0);
            write(raw::weapon_core::ammo_in_magazine(core), 0);
            write(raw::weapon_core::fire_queue_type(core), 0);
            write(raw::weapon_core::aimed(core), 0);
            write(raw::weapon_core::unnamed_5ec(core), 6);
            write(raw::weapon_core::round_chambered(core), 0);
            write(raw::weapon_core::need_to_auto_reload(core), 0);
            let recoil = raw::weapon_core::recoil(core);
            for i in 0..4 {
                write(recoil.add(i), 0.0);
            }
            write(raw::weapon_core::unnamed_60c(core), u32::MAX);
        }
    }
}
