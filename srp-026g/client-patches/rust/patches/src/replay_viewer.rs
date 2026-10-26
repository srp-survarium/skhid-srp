//! The replay viewer (README "24 to 26", client_patch.md section 3.24;
//! docs/notes/replay_sur.md). Nothing a match computes.
//!
//! - 25, a full sound queue: a weapon's delayed sounds' add pushes without
//!   looking, and push_back asserts at the capacity (64; seen in a replay
//!   about 50 s after a seek). With the queue full the new sound is dropped
//!   by the game's own exit for a sound past its chain's cap.
//! - 26, the seek: after a user seek the old timeline's delayed weapon
//!   sounds and the re-simulation's burst would play late, by the seek's
//!   distance, and fill the queues; every client weapon's delayed sounds
//!   go and its fx history is emptied, as ~weapon does.

#![forbid(unsafe_code)]

use crate::vostok::replay::{ReplaySeek, SoundQueue};
use crate::vostok::{Context, Flow, NoExit, Reg, ReplayViewerOnSoundAddedExit};

/// `replay.seek`: a user seek, the clock at the target; then the seek's
/// own `lea` (Continue).
pub fn on_seek(ctx: &mut Context<'_>) -> Flow<NoExit> {
    ReplaySeek::at(ctx).for_each_client_weapon(|weapon| {
        weapon.drop_delayed_sounds();
        weapon.empty_fx_history();
    });
    Flow::Continue
}

/// `weapon_sounds.added`: a full queue drops the new sound (ECX its end, as
/// the instructions it skips would load it); else the add goes on.
pub fn on_sound_added(ctx: &mut Context<'_>) -> Flow<ReplayViewerOnSoundAddedExit> {
    let Some(queue) = SoundQueue::at_added(ctx) else { return Flow::Continue };
    if !queue.full() {
        return Flow::Continue;
    }
    let end = queue.end();
    ctx.set_reg(Reg::Ecx, end);
    Flow::Exit(ReplayViewerOnSoundAddedExit::Drop)
}
