//! Patch 179: a shot ends the shooter's protection (the owner, 2026-11-22:
//! "when you start shooting your invulnerability should be removed";
//! docs/notes/shot-protection.md).
//!
//! - A gun's shot, by any player in any mode, ends what the spawn gave.
//! - A base's zone only keeps a protection going: it gives none to a life
//!   whose protection was ended, so a shot from inside a base ends it there
//!   too, until the next spawn.

#![forbid(unsafe_code)]

use crate::vostok::protection::Protection;
use crate::vostok::{Context, Flow, NoExit, ShotProtectionOnZoneProtectExit};

/// on_fire: a gun's shot (`weapon_stats.fired`, after patch 165's count).
pub fn on_fired(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(protection) = Protection::of_shooter(ctx) {
        protection.end();
    }
    Flow::Continue
}

/// A zone about to move an own-team player's protection on.
pub fn on_zone_protect(ctx: &mut Context<'_>) -> Flow<ShotProtectionOnZoneProtectExit> {
    match Protection::at_zone(ctx) {
        Some(protection) if protection.is_ended() => Flow::Exit(ShotProtectionOnZoneProtectExit::Skip),
        _ => Flow::Continue,
    }
}
