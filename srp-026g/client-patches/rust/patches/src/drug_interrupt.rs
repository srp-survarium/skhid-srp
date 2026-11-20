//! Patch 84: a bullet that hits a player stops the healing drug he is using
//! (the owner, 2026-10-31: "if a bullet hits a person while (use/active),
//! the healing stops"; docs/notes/drug-interrupt.md).
//!
//! - A ranged hit that took health: the game's own word for a hit (hit_impl
//!   goes on only then); one armour stopped whole took none and stops
//!   nothing. Whoever fired it: a teammate's too (0.26g deals team damage);
//!   a player's own bullets never hit him (try_pierce 0x32D9DD).
//! - Every drug in use that heals, in its use time (it never starts) or its
//!   active time (it stops; what it healed stays). The unit stays used.
//! - The painkiller heals nothing (its only effect is less pain from
//!   bullets): it goes on.

#![forbid(unsafe_code)]

use crate::vostok::drug::{AppliedHit, Effect, HitType};
use crate::vostok::{Context, Flow, NoExit};

/// hit_impl once the hit took health (`player.hit_applied`).
pub fn on_hit_applied(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(hit) = AppliedHit::at_applied(ctx)
        && hit.kind() == HitType::Ranged
    {
        for drug in hit.drugs_in_use().filter(|drug| drug.effect() == Effect::Heals) {
            drug.stop(hit.time());
        }
    }
    Flow::Continue
}
