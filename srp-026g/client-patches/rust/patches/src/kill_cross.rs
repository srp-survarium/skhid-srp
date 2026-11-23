//! Patch 191: a kill first simulated in a re-simulation keeps its red kill
//! marker (docs/notes/kill-marker.md).
//!
//! - The game walks the world events after the last kill-cross time and
//!   gives each to on_kill_cross_event, which draws the marker for a kill by
//!   the drawn player and then wrote the event's time, of any type, as the
//!   last. A world load drops the events after its time and the
//!   re-simulation makes them again; the last time stays.
//! - Only a kill's time is written now: a hit or a battery's user walked
//!   before the load no longer hides a kill the re-simulation makes at or
//!   before its time. A kill walked is not walked again (its time is the
//!   last).

#![forbid(unsafe_code)]

use crate::vostok::world_event::{self, Kind};
use crate::vostok::{Context, Flow, KillCrossOnWalkedExit};

/// on_kill_cross_event done with an event (`kill_cross.walked`).
pub fn on_walked(ctx: &mut Context<'_>) -> Flow<KillCrossOnWalkedExit> {
    match world_event::walked(ctx) {
        Some(Kind::Kill) => Flow::Continue,
        _ => Flow::Exit(KillCrossOnWalkedExit::Kept),
    }
}
