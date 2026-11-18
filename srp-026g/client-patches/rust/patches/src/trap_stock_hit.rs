//! Patch 151: a gun-stock hit disarms a trap (the owner, 2026-11-15;
//! docs/notes/trap-stock-hit.md).
//!
//! - A trap defuses on a hit of its set's defuse_by_hit type alone, ranged:
//!   a bullet.
//! - A weapon's melee is a bullet too, of the blunt type (melee/data), so it
//!   reaches the trap's hit and is dropped there.
//! - A blunt hit goes on where the trap's own type does: the game's defuse.

#![forbid(unsafe_code)]

use crate::vostok::drug::HitType;
use crate::vostok::{trap, Context, Flow, TrapStockHitOnHitExit};

/// booby_trap_core::hit's entry (`trap.hit`), before its test of the type.
pub fn on_hit(ctx: &mut Context<'_>) -> Flow<TrapStockHitOnHitExit> {
    match trap::hit_kind(ctx) {
        Some(HitType::Blunt) => Flow::Exit(TrapStockHitOnHitExit::Defuse),
        _ => Flow::Continue,
    }
}
