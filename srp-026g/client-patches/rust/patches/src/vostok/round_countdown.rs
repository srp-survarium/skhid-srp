//! A round about to start plays the match start's countdown, in every mode
//! with rounds (Artifact Hunt, patch 171; Freeze Tag, patch 160): the core's
//! copy of the time state is [`ROUND_COUNTDOWN`] then, which the client time
//! rule's camera test reads as the countdown's (the warmup camera, patch 31's
//! kits). The time rule's own state, start and end are untouched (its
//! countdown would restart the match clock).
use super::context::{Context, Reg};
use super::world::read;
use crate::raw;

/// The core's time state while a round is about to start.
const ROUND_COUNTDOWN: u32 = 4;
const IN_PROCESS: u32 = 3;
const COUNTDOWN: u32 = 2;

/// The core's copy of the time state as a mode with rounds reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Play {
    /// Waiting for players or the match start's countdown.
    Before,
    /// The time rule in process.
    InProcess,
    /// In process, a round about to start.
    RoundCountdown,
}

impl Play {
    /// # Safety
    /// `core` is a live core world.
    pub(super) unsafe fn of(core: raw::CoreWorld) -> Play {
        // SAFETY: the caller's live core; its copy of the time rule's state.
        match unsafe { read(core.time_state()) } {
            IN_PROCESS => Play::InProcess,
            ROUND_COUNTDOWN => Play::RoundCountdown,
            _ => Play::Before,
        }
    }

    pub fn in_play(self) -> bool {
        matches!(self, Play::InProcess | Play::RoundCountdown)
    }
}

/// The round's countdown on or off: written only in play (never before it),
/// by the mode's rule at its tick (and its load), in every game at the same
/// step. Its readers in these modes: patch 31's kit change (no switch in a 4),
/// the keeper's gift (a 4 is "not started": it waits), the batteries (none).
///
/// # Safety
/// `core` is a live core world, written on the simulation's thread.
pub(super) unsafe fn set(core: raw::CoreWorld, on: bool) {
    // SAFETY: the caller's live core; its copy of the time rule's state.
    unsafe {
        if Play::of(core).in_play() {
            core.time_state().write_volatile(if on { ROUND_COUNTDOWN } else { IN_PROCESS });
        }
    }
}

/// The client time rule's camera test (`time_rule.camera_state`, 0x267AE6:
/// its node's state against the network client's +1F1C60h): EDI its own
/// state, or the countdown's 2 while the core says a round is about to start,
/// so the native camera switch (0x27B1A0: the warmup camera; patch 31's kits
/// by +1F1C60h) plays the match start's countdown for a round, and switches
/// back to the player's camera as the round starts.
///
/// A mode may leave its own players their camera in the countdown: Freeze Tag's
/// who are not frozen move on through the pause (`freeze_tag::countdown_view`).
pub fn camera_state(ctx: &mut Context<'_>) {
    // SAFETY: the site's EDI, the live client time rule; its core (+128h,
    // 0x267ACC) and its own state (+148h, the instruction replaced); ESI the
    // network client (0x267AE0), its local player's place when it has one.
    let state = unsafe {
        let Some(rule) = raw::site::time_rule_camera_state::rule(ctx) else { return };
        match rule.timelimit_core() {
            Some(core) if Play::of(core) == Play::RoundCountdown && countdown_view(ctx) => COUNTDOWN,
            _ => read(rule.time_state()),
        }
    };
    ctx.set_reg(Reg::Edi, state);
}

/// Whether this game's local player sees the round's countdown; the local
/// player read only for a mode that asks (Freeze Tag).
fn countdown_view(ctx: &Context<'_>) -> bool {
    let thread = ctx.thread();
    if !crate::freeze_tag::active(thread) {
        return true;
    }
    // SAFETY: the site's ESI, the live network client; its local player's
    // place byte when it has one.
    let local = unsafe {
        raw::site::time_rule_camera_state::client(ctx)
            .and_then(|client| client.local_player())
            .and_then(|player| srp_validate::Place::new(read(player.place())))
    };
    crate::freeze_tag::countdown_view(thread, local)
}
