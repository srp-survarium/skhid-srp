//! The round countdown of the modes with rounds (Artifact Hunt, patch 171;
//! Freeze Tag, patch 160): a round about to start plays the match start's
//! countdown (`vostok::round_countdown`).
#![forbid(unsafe_code)]
use crate::vostok::{Context, Flow, RoundCountdownOnCameraStateExit};

/// The client time rule's camera test: the countdown's state while the core
/// says a round is about to start (any mode whose rule makes it so).
pub fn on_camera_state(ctx: &mut Context<'_>) -> Flow<RoundCountdownOnCameraStateExit> {
    crate::vostok::round_countdown::camera_state(ctx);
    Flow::Exit(RoundCountdownOnCameraStateExit::State)
}
