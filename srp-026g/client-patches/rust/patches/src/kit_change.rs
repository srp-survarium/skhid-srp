//! Patch 31: a kit chosen before the match starts is played at once
//! (client_patch.md section 3.29; the rules:
//! docs/notes/kit-change-before-start.md).
//!
//! The respawn rule's tick, at every tick of every game and every replay of
//! a tick, first: while the time rule waits or counts down, each place whose
//! next kit is another than the kit in use, whose player is alive and in
//! the world, and which has no statistics yet (no event of any kind, no
//! damage taken from a player), is respawned in the next kit where it
//! stands, facing as it faces, with what is left of its invulnerability, by
//! the rule's own two calls as its tick makes them for a place it respawns.
//! No death is counted, no respawn time set. It ticks before the places
//! (which take the tick's kit choices) and the time rule after both, so a
//! choice is played at the next tick, and a switch is of a tick before play
//! or of the one whose time rule begins it.
//!
//! The client's view of such a spawn keeps the warmup's camera and input
//! before play, as the time rule's change does. The hud shows the kits to a
//! player who becomes alive before play and keeps them shown before play,
//! hiding them in play.

#![forbid(unsafe_code)]

use crate::vostok::kit_choice::{HudState, KitView, PLACES, RespawnTick};
use crate::vostok::{Context, Flow, KitChangeOnAliveExit, NoExit};

/// `respawn_rule.tick`: each place's kit choice, before play.
pub fn on_tick(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(tick) = RespawnTick::at(ctx) else { return Flow::Continue };
    if !tick.before_play() {
        return Flow::Continue;
    }
    let time = tick.time().0;
    for index in 0..PLACES {
        let Some(place) = tick.place(index) else { continue };
        let (next, kit) = (place.next_kit(), place.kit());
        if next == kit || place.player(next).is_none() {
            continue;
        }
        let Some(player) = place.player(kit) else { continue };
        if !player.alive_in_world() || !tick.clean(&place) {
            continue;
        }
        let Some(until) = player.invulnerable_until() else { continue };
        // what is left of it; none (any more): -1 - time, which the spawn
        // adds to the time, so it writes -1 (not invulnerable)
        let protection = match until {
            u32::MAX => !time,
            until if until > time => until - time,
            _ => !time,
        };
        tick.respawn(&place, &player, protection);
    }
    Flow::Continue
}

/// `kit_view.camera`: the player camera in play, the warmup's before.
pub fn on_kit_view(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let view = KitView::at(ctx);
    if view.in_play() {
        return Flow::Continue;
    }
    view.warmup_camera();
    Flow::Skip
}

/// `hud.alive`: the original call, then the kits shown before play (the
/// original hid them).
pub fn on_alive(ctx: &mut Context<'_>) -> Flow<KitChangeOnAliveExit> {
    let hud = HudState::at(ctx);
    hud.show_stamina();
    hud.kits_until_play();
    Flow::Exit(KitChangeOnAliveExit::Shown)
}

/// `hud.update`: a live local player's kits shown before play, hidden in
/// play.
pub fn on_update(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let hud = HudState::at(ctx);
    if hud.player_alive() {
        hud.kits_until_play();
    }
    Flow::Continue
}
