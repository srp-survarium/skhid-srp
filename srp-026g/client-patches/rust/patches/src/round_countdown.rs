//! The rounds of the modes played in rounds (Artifact Hunt, patch 171; Freeze Tag, patch 160): one
//! path for both, as their rules share one restart (`artefact_hunt_rule::Restart`). A round about to
//! start plays the match start's countdown (`vostok::round_countdown`), and the respawn rule's tick
//! does what the restart says.
#![forbid(unsafe_code)]
use artefact_hunt_rule::{PLAYERS, Spawning};

use crate::vostok::artefact_hunt::RespawnTick;
use crate::vostok::{Context, Flow, RoundCountdownOnCameraStateExit};

/// The client time rule's camera test: the countdown's state while the core
/// says a round is about to start (any mode whose rule makes it so).
pub fn on_camera_state(ctx: &mut Context<'_>) -> Flow<RoundCountdownOnCameraStateExit> {
    crate::vostok::round_countdown::camera_state(ctx);
    Flow::Exit(RoundCountdownOnCameraStateExit::State)
}

/// The respawn rule's tick of a match played in rounds, before its rule's step of the step: in a
/// round's countdown (`countdown`) patch 31's kit switch where the player stands, each a life the
/// rule counts (`switched`); the places its rule voids (`void`, a mask) lose their native time;
/// then the places due as the restart says (`spawning`). An entering place is never voided or held
/// (freeze-tag.md, "No place out of the world").
pub fn respawn_tick(
    tick: &RespawnTick<'_>,
    countdown: bool,
    spawning: Spawning,
    void: u32,
    mut switched: impl FnMut(u8),
) {
    let places = 0..PLAYERS as u8;
    if countdown {
        places.clone().filter(|place| tick.switch_kit(*place)).for_each(&mut switched);
    }
    places.filter(|place| (void >> place) & 1 != 0).for_each(|place| tick.void(place));
    match spawning {
        Spawning::Native => {}
        Spawning::Held => tick.hold_due(),
        Spawning::Everyone => tick.everyone_now(),
    }
}
