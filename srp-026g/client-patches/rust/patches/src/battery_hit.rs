//! Patch 143: a battery's carrier hits with it (docs/notes/battery-hit.md).
//!
//! The melee key while carrying strikes with the battery: an authored gather
//! and drive (the content's clip), the gun-stock hit's bullet of the
//! carrier's gun on the strike (that clip's end), then the follow-through
//! back to the carry. Three kinds in turn, the ram, the shove and the slam
//! (`Strike`), each a swing and a recovery state of the battery's own state
//! machine (`vostok::battery`), so every game runs, saves and loads them as
//! the game's own three; every game deals the hit.

#![forbid(unsafe_code)]

use crate::vostok::battery::{self, BatteryState, MadeBattery, PredicateBuffer, Strike, Times, UserSite};
use crate::vostok::{Context, Flow, NoExit, OrigBatteryStateFinalize, OrigTakeDeactivatable, battery_hit_on_wants_swing_entry,
                    battery_hit_swing_vtable_handle};

/// A strike's states' lengths, the authored clips' (scripts/battery-hit/swing.py,
/// 30 frames a second). The ram and the shove: 6 frames to the strike, 10.5
/// back to the carry. The slam lifts the battery first: 12 frames to the
/// strike, 9 back.
pub const fn times(strike: Strike) -> Times {
    match strike {
        Strike::Ram | Strike::Shove => Times { swing: 6.0 / 30.0, recover: 10.5 / 30.0 },
        Strike::Slam => Times { swing: 12.0 / 30.0, recover: 9.0 / 30.0 },
    }
}

/// The cook's callback once initialize_logic made the take, carry and put:
/// the strikes' states appended. A battery without them would play
/// otherwise than every other game's: a crash, not a skip.
pub fn on_logic(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let battery = MadeBattery::at(ctx).expect("the cook's battery and its results (patch 143)");
    battery.add_strikes(times, battery_hit_swing_vtable_handle(), battery_hit_on_wants_swing_entry())
        .expect("a battery's strikes (patch 143)");
    Flow::Continue
}

/// Carry -> swing: the carrier alive with the melee key down and not in
/// the air, the gun-stock hit's rules for a carrier (after carry -> put,
/// which a switch away takes first).
pub fn on_wants_swing(buffer: PredicateBuffer<'_>) -> bool {
    buffer.carrier().is_some_and(|carrier| carrier.melee_down() && !carrier.airborne())
}

/// A swing or a recovery left (by its machine, or the battery put away):
/// the native finalize, then, for a swing whose clip ended (the strike) and
/// whose carrier lives, its kind's strike. A carrier who cannot strike (no
/// gun, never in a match) is a crash, not a skip.
pub fn on_finalize(orig: &OrigBatteryStateFinalize, state: &mut BatteryState<'_>) {
    let struck = state.swing().filter(|_| state.ended());
    orig.call(state);
    if let (Some(strike), Some(carrier)) = (struck, state.carrier()) {
        carrier.strike(strike).expect("a carrier's strike (patch 143)");
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

/// The kill event's weapon icon: the battery's for a strike's kill.
pub fn on_kill_feed_icon(ctx: &mut Context<'_>) -> Flow<NoExit> {
    battery::kill_feed_icon(ctx);
    Flow::Continue
}
