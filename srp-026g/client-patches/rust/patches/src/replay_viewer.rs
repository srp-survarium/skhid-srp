//! The replay viewer (README "24 to 26", client_patch.md section 3.24;
//! docs/notes/replay_sur.md). Nothing a match computes.
//!
//! - 26, the seek: after a user seek the old timeline's delayed weapon
//!   sounds and the re-simulation's burst would play late, by the seek's
//!   distance, and fill the queues; every client weapon's delayed sounds
//!   go and its fx history is emptied, as ~weapon does.

#![forbid(unsafe_code)]

use crate::vostok::replay::ReplaySeek;
use crate::vostok::{Context, Flow, NoExit};

/// `replay.seek`: a user seek, the clock at the target; then the seek's
/// own `lea` (Continue).
pub fn on_seek(ctx: &mut Context<'_>) -> Flow<NoExit> {
    ReplaySeek::at(ctx).for_each_client_weapon(|weapon| {
        weapon.drop_delayed_sounds();
        weapon.empty_fx_history();
    });
    Flow::Continue
}
