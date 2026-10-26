//! Patch 80: a carried battery drops at its carrier's death, from the hand,
//! with the hand's velocity (`docs/notes/battery-drop.md`).
//!
//! The game drops a dead carrier's battery only once the item switch finds
//! it ready to be deactivated, which the dead carrier's hands' expression
//! and the battery's state make it about the death clip's end later. Then
//! the battery is put at the last hand pose the carried state tracked, with
//! the hand's velocity, and falls in the simulation's own physics world.
//! Two sites (`battery_drop.toml`):
//!
//! - `on_is_ready` (a splice of is_ready_to_be_deactivated, 0x301270): ready
//!   at once when the carrier is dead, so the item switch drops it on the
//!   death's tick; otherwise the game's own answer;
//! - a write of nops over the carried states' gate (0x344B89): execute
//!   tracks the hand's pose (the simulation's "Weapon" bone) and velocity
//!   every tick, so the pose and velocity at the drop are the last tick's.
//!   finalize still gives them to the battery only for a dead carrier (its
//!   own test, the state's vt+28h), so a living carrier's drop and a put
//!   into a container are unchanged.
//!
//! Everything here is the match simulation's (the carried state is
//! serialized with the world): every game computes it the same way, and
//! older replays are refused (simulation_changes.tsv).

#![forbid(unsafe_code)]

use crate::vostok::{OrigVictoryItemIsReadyForBatteryDropOnIsReady, VictoryItem};

/// The battery's is_ready_to_be_deactivated: true for a dead carrier, else
/// the game's own answer (`is_ready`, the trampoline).
pub fn on_is_ready(is_ready: &OrigVictoryItemIsReadyForBatteryDropOnIsReady, battery: &mut VictoryItem) -> bool {
    if battery.carrier().is_some_and(|carrier| !carrier.alive()) {
        return true;
    }
    is_ready.call(battery)
}
