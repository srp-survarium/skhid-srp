//! Patch 143: a battery's carrier hits with it (docs/notes/battery-hit.md).
//!
//! The melee key while carrying swings the battery: an authored wind-up and
//! strike (the content's battery_hit clip), the gun-stock hit's bullet of
//! the carrier's gun on the impact (that clip's end), then the follow-through
//! back to the carry (battery_hit_recover). Two states of the battery's own
//! state machine (`vostok::battery`), so every game runs, saves and loads
//! them as the game's own three; every game deals the hit.

#![forbid(unsafe_code)]

use crate::vostok::battery::{self, BatteryState, MadeBattery, PredicateBuffer, UserSite};
use crate::vostok::{Context, Flow, NoExit, OrigBatteryStateFinalize, OrigTakeDeactivatable, battery_hit_on_wants_swing_entry,
                    battery_hit_swing_vtable_handle};

/// The swing's length, s: wind-up and strike, its end the impact (the
/// authored clip's 6 frames at 30 fps, scripts/battery-hit/swing.py IMPACT).
pub const SWING_S: f32 = 6.0 / 30.0;

/// The recovery's length, s: held out, then back to the carry idle (its
/// 10.5 frames, swing.py END - IMPACT): 7 ticks of 50 ms, so it ends on a
/// tick as the swing does.
pub const RECOVER_S: f32 = 10.5 / 30.0;

/// The cook's callback once initialize_logic made the take, carry and put:
/// the swing and the recovery appended. A battery without them would play
/// otherwise than every other game's: a crash, not a skip.
pub fn on_logic(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let battery = MadeBattery::at(ctx).expect("the cook's battery and its results (patch 143)");
    battery.add_swing(SWING_S, RECOVER_S, battery_hit_swing_vtable_handle(), battery_hit_on_wants_swing_entry())
        .expect("a battery's swing (patch 143)");
    Flow::Continue
}

/// Carry -> swing: the carrier alive with the melee key down and not in
/// the air, the gun-stock hit's rules for a carrier (after carry -> put,
/// which a switch away takes first).
pub fn on_wants_swing(buffer: PredicateBuffer<'_>) -> bool {
    buffer.carrier().is_some_and(|carrier| carrier.melee_down() && !carrier.airborne())
}

/// The swing or the recovery left (by its machine, or the battery put
/// away): the native finalize, then, for the swing whose clip ended (the
/// impact) and whose carrier lives, the strike. A carrier who cannot strike
/// (no gun, never in a match) is a crash, not a skip.
pub fn on_finalize(orig: &OrigBatteryStateFinalize, state: &mut BatteryState<'_>) {
    let ended = state.ended() && state.is_swing();
    orig.call(state);
    if ended && let Some(carrier) = state.carrier() {
        carrier.strike().expect("a carrier's strike (patch 143)");
    }
}

/// May the battery be put away during the swing: as from the carry
/// (0x338E90), when its carrier is dead; a living carrier leaves through
/// swing -> put once the clip ended.
pub fn on_deactivatable(_: &OrigTakeDeactivatable, state: &mut BatteryState<'_>) -> bool {
    state.carrier().is_none()
}

/// The entries' filter of the gun reads below: a null user or posture.
pub const NONE: [u32; 1] = [0];

/// player_stance read no user: the gun's player.
pub fn on_stance_user(ctx: &mut Context<'_>) -> Flow<NoExit> {
    battery::substitute_player(ctx, UserSite::Stance);
    Flow::Continue
}

/// player_stance read no posture: the gun's player's held battery's.
pub fn on_stance_posture(ctx: &mut Context<'_>) -> Flow<NoExit> {
    battery::substitute_posture(ctx);
    Flow::Continue
}

/// The melee damage read no user: the gun's player.
pub fn on_melee_damage_user(ctx: &mut Context<'_>) -> Flow<NoExit> {
    battery::substitute_player(ctx, UserSite::MeleeDamage);
    Flow::Continue
}

/// The melee minimum damage read no user: the gun's player.
pub fn on_melee_falloff_user(ctx: &mut Context<'_>) -> Flow<NoExit> {
    battery::substitute_player(ctx, UserSite::MeleeFalloff);
    Flow::Continue
}
