//! The respawn rule's spawn (0x316DA0, slot +5Ch of every respawn rule; the
//! rule's tick 0x316820 calls it for every spawn, a match's first too, in
//! every game and in every replay of a tick after a rollback). It makes the
//! new kit current (0x316DE3), initialises that kit's player (vtable+1Ch:
//! the inventory from the profile, a weapon in hand; no weapon object is
//! made anew) and puts it into the world (vtable+20h). Two sites:
//! - `kit_state.spawn` (0x316E57, the load of the initialize's call): ESI
//!   the new kit's player, the spawn's [esp+10h] the kit current before and
//!   [esp+18h] the new kit (patch 20);
//! - `weapon_random.spawn` (0x316EA0, the load of the call that puts the
//!   player into the world): ESI the player, initialised, EBP the spawn's
//!   time (patch 18).
//!
//! The weapons of a player are the items of its inventory's 23 slots whose
//! methods' bind (+70h) is weapon_core's (0x2E30F0: only weapon_core's and
//! the client weapon's vtables hold it); the slot holds weapon+10h.

use core::marker::PhantomData;

use super::context::Context;
use super::hook::Thread;
use super::world::{read, write_floats};
use crate::raw;

/// The slots of an inventory.
const SLOTS: usize = 23;

/// The spawn at `kit_state.spawn`: the new kit current, its player not yet
/// initialised.
pub struct Spawning<'t> {
    player: Option<SpawnPlayer<'t>>,
    changes_kit: bool,
}

impl<'t> Spawning<'t> {
    pub fn at(ctx: &'t Context<'_>) -> Spawning<'t> {
        use raw::site::kit_state_spawn as site;
        // SAFETY: the site's ESI (the new kit's player, taken at 0x316E50 and
        // live through the spawn) and its frame's two kit bytes.
        unsafe {
            let player = site::player(ctx).map(|this| SpawnPlayer { this, _thread: PhantomData });
            let changes_kit = site::frame(ctx).is_some_and(|f| read(f.kit_before()) != read(f.kit()));
            Spawning { player, changes_kit }
        }
    }

    pub fn player(&self) -> Option<&SpawnPlayer<'t>> {
        self.player.as_ref()
    }

    /// Whether the spawn is into another kit than the one current before.
    pub fn changes_kit(&self) -> bool {
        self.changes_kit
    }
}

/// The spawn at `weapon_random.spawn`: its player initialised and not yet
/// in the world, at the spawn's time.
pub struct Spawned<'t> {
    player: Option<SpawnPlayer<'t>>,
    time: u32,
}

impl<'t> Spawned<'t> {
    pub fn at(ctx: &'t Context<'_>) -> Spawned<'t> {
        use raw::site::weapon_random_spawn as site;
        // SAFETY: the site's ESI (the spawn took it at 0x316E50 and writes
        // ESI nowhere after) and EBP, the spawn's time (0x316DC8).
        unsafe {
            Spawned { player: site::player(ctx).map(|this| SpawnPlayer { this, _thread: PhantomData }),
                      time: site::time(ctx) }
        }
    }

    pub fn player(&self) -> Option<&SpawnPlayer<'t>> {
        self.player.as_ref()
    }

    /// The time the spawn is at (ms).
    pub fn time(&self) -> u32 {
        self.time
    }
}

/// The spawn's player.
pub struct SpawnPlayer<'t> {
    this: raw::Player,
    _thread: PhantomData<&'t Thread>,
}

impl<'t> SpawnPlayer<'t> {
    /// Its place, the byte as the game holds it.
    pub fn place(&self) -> u8 {
        // SAFETY: the live player's plain byte.
        unsafe { read(self.this.place()) }
    }

    /// Its kit of the place, the byte as the game holds it.
    pub fn kit(&self) -> u8 {
        // SAFETY: as place.
        unsafe { read(self.this.kit()) }
    }

    /// The weapons of its inventory, by slot (0..23), as its slots hold
    /// them: none without an inventory.
    pub fn weapons(&self) -> impl Iterator<Item = (u32, SpawnWeapon<'_>)> + '_ {
        // SAFETY: the live player's inventory (or none) and its 23 slots.
        let slots = unsafe { self.this.inventory().map(|inventory| inventory.slots()) };
        (0..SLOTS).filter_map(move |slot| {
            // SAFETY: a slot of the live inventory: its item (or none), the
            // item's methods and their bind, and the weapon_core of an item
            // whose bind is weapon_core's (the item is weapon+10h).
            unsafe {
                let item = slots?.at(slot).item()?;
                let bind = read(item.methods()?.bind());
                (bind == raw::image::WEAPON_CORE_BIND)
                    .then(|| (slot as u32, SpawnWeapon { this: item.core_weapon(), _player: PhantomData }))
            }
        })
    }
}

/// A weapon of the spawn's player.
pub struct SpawnWeapon<'p> {
    this: raw::Weapon,
    _player: PhantomData<&'p ()>,
}

impl SpawnWeapon<'_> {
    /// Sets the state its spread draws from (minstd_rand's, +5BCh).
    pub fn set_random_state(&self, state: u32) {
        // SAFETY: a live weapon_core's plain field.
        unsafe { *self.this.random_state() = state }
    }

    /// Forgets the time of its last tick (the constructor's -1): its
    /// activation sets it again before it is ticked.
    pub fn forget_last_tick(&self) {
        // SAFETY: as set_random_state.
        unsafe { *self.this.last_tick() = u32::MAX }
    }

    /// The fields a weapon keeps through deaths and spawns and that neither
    /// the player's initialize nor the weapon's activation set again (or
    /// only for the weapon put in hand) get the constructor's values
    /// (0x2DF0D0): the ammunition switch's time, the ammunition kinds, the
    /// magazine, the fire mode, the aim, +5ECh, the chamber, the auto
    /// reload, the recoil and +60Ch.
    pub fn reset_as_constructed(&self) {
        let w = self.this;
        // SAFETY: as set_random_state; each field is the weapon_core's.
        unsafe {
            *w.ammo_switch_time() = u32::MAX;
            *w.ammo_selected() = 0;
            *w.next_ammo() = 0;
            *w.loaded() = 0;
            *w.fire_queue() = 0;
            *w.aimed() = 0;
            *w.state_5ec() = 6;
            *w.chamber() = 0;
            *w.auto_reload() = 0;
            write_floats::<4>(w.recoil(), [0.0; 4]);
            *w.state_60c() = u32::MAX;
        }
    }
}
