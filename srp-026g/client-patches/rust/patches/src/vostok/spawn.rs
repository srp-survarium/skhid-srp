//! The respawn rule's spawn (0x316DA0, slot +5Ch of every respawn rule; the
//! rule's tick 0x316820 calls it for every spawn, in every game and in every
//! replay of a tick after a rollback). It makes the new kit current
//! (0x316DE3), initialises that kit's player (vtable+1Ch: the inventory from
//! the profile, a weapon in hand) and puts it into the world (vtable+20h).
//! What the sites' registers hold is said here, once:
//! - `spawn.weapon_random` (0x316EA0, the load of the call that puts the
//!   player into the world): ESI the new kit's player, EBP the spawn's time.

use core::marker::PhantomData;

use super::context::{Context, Reg};
use super::hook::Thread;
use super::object::Object;
use super::player::Player;
use super::types::GameTime;

/// The spawn at `spawn.weapon_random`: its player is initialised and not yet
/// in the world.
pub struct Spawned<'t> {
    player: Option<Object>,
    time: GameTime,
    thread: PhantomData<&'t Thread>,
}

impl<'t> Spawned<'t> {
    /// The spawn at `spawn.weapon_random`.
    pub fn at(ctx: &'t Context<'_>) -> Spawned<'t> {
        Spawned { player: Object::at(ctx.reg(Reg::Esi)), time: GameTime(ctx.reg(Reg::Ebp)), thread: PhantomData }
    }

    /// The new kit's player (the spawn's ESI, never null there).
    pub fn player(&self) -> Option<Player<'t>> {
        // SAFETY: at the site ESI is the new kit's player (the spawn took it
        // at 0x316E50 and writes ESI nowhere after), live through the spawn.
        self.player.map(|this| unsafe { Player::new(this) })
    }

    /// The time the spawn is at (its third argument, EBP from 0x316DC8).
    pub fn time(&self) -> GameTime {
        self.time
    }
}
