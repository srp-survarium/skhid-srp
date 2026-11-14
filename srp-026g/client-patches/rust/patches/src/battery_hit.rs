//! Patch 143: a battery's carrier hits with it (docs/notes/battery-hit.md).
//!
//! The melee key while carrying swings the battery: the put's clip (both
//! hands push it forward) played as a thrust, then straight back to the
//! carry; when the clip ends, the gun-stock hit's bullet of the carrier's
//! gun. The swing is a fourth state of the battery's own state machine
//! (`vostok::battery`), so every game runs, saves and loads it as the
//! game's own three; every game deals the hit.

#![forbid(unsafe_code)]

use crate::vostok::battery::{BatteryState, MadeBattery, PredicateBuffer};
use crate::vostok::{Context, Flow, NoExit, OrigBatteryStateFinalize, battery_hit_on_wants_swing_entry,
                    battery_hit_swing_vtable_handle};

/// The swing's length, s: the put's clip (0.6 s as the put) as a thrust.
pub const SWING_S: f32 = 0.3;

/// The cook's callback once initialize_logic made the take, carry and put:
/// the swing appended. A battery without it would play otherwise than every
/// other game's: a crash, not a skip.
pub fn on_logic(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(battery) = MadeBattery::at(ctx) {
        battery.add_swing(SWING_S, battery_hit_swing_vtable_handle(), battery_hit_on_wants_swing_entry())
            .expect("a battery's swing (patch 143)");
    }
    Flow::Continue
}

/// Carry -> swing: the carrier alive with the melee key down (after
/// carry -> put, which a switch away takes first).
pub fn on_wants_swing(buffer: PredicateBuffer<'_>) -> bool {
    buffer.carrier().is_some_and(|carrier| carrier.melee_down())
}

/// The swing left: the native finalize, then, when its clip ended (not a
/// death mid-swing), the strike. A carrier without a gun (never in a kit)
/// strikes nothing, in every game alike.
pub fn on_finalize(orig: &OrigBatteryStateFinalize, state: &mut BatteryState<'_>) {
    let ended = state.ended();
    orig.call(state);
    if ended && let Some(carrier) = state.carrier() {
        let _ = carrier.strike();
    }
}
