//! A life's protection (patch 179, docs/notes/shot-protection.md): the
//! damage model's m_invulnerability_time_in_ms (+32Ch), the time its player
//! is invulnerable until, -1 for none (get_invulnerability 0x2E5550).
//! - The spawn starts it: the time + its point's protection (0x316E90).
//! - A base's effect zone moves the end on each time it finds an own-team
//!   player inside (0x31ACA0: the time + the zone's duration, with a fade).
//! - base_player::initialize makes it -1 (damage_model::reset 0x2E5290); a
//!   spawn follows, so a life never has -1 of the game's own, save patch
//!   31's kit switch once the protection has run out.

use core::marker::PhantomData;

use super::context::Context;
use super::hook::Thread;
use super::world::read;
use crate::raw;

/// m_invulnerability_time_in_ms for no protection.
const NONE: u32 = u32::MAX;

/// A living player's protection, in its damage model.
pub struct Protection<'h> {
    model: raw::MedicalDamage,
    thread: PhantomData<&'h Thread>,
}

impl<'h> Protection<'h> {
    /// The shooter's, at `weapon_stats.fired` (base_player::on_fire, which
    /// instant_fire calls for every shot of a gun).
    pub fn of_shooter(ctx: &'h Context<'_>) -> Option<Protection<'h>> {
        // SAFETY: the context is this site's: on_fire's player (its +110h
        // interface in ESI), alive with the gun it fires.
        let model = unsafe { raw::site::weapon_stats_fired::user(ctx)?.player().damage_model()? };
        Some(Protection { model, thread: PhantomData })
    }

    /// The player's whose protection a zone is about to move on, at
    /// `effect_zone.protect`.
    pub fn at_zone(ctx: &'h Context<'_>) -> Option<Protection<'h>> {
        // SAFETY: the context is this site's: EAX the damage model of a
        // player the zone holds a reference of.
        let model = unsafe { raw::site::effect_zone_protect::model(ctx)? };
        Some(Protection { model, thread: PhantomData })
    }

    /// Whether the life has none: it was ended, not left to run out (a time
    /// that has passed stays as it is).
    pub fn is_ended(&self) -> bool {
        // SAFETY: the damage model of a living player.
        unsafe { read(self.model.invulnerable_until()) == NONE }
    }

    /// Ends it: no protection, and no fade of one, from this tick on.
    pub fn end(self) {
        // SAFETY: the damage model of a living player, written in the
        // simulation as the spawn writes it.
        unsafe { self.model.invulnerable_until().write_volatile(NONE) }
    }
}
