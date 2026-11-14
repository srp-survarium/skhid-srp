//! Patch 80: an anomaly's artefact taken into an empty artefact slot scores
//! 15, one taken with every slot full 5 (the owner, 2026-10-29: a player
//! whose slots are full hands one to a teammate first; docs/notes/
//! artefact-score.md).
//!
//! The game counts event 5 for every artefact a container gives
//! (use_execute 0x31C780, after the transfer): srp weighs it 5. Before the
//! transfer, this hook counts event 16 too when a slot is empty: srp weighs
//! it 10. The game counts 16 nowhere else in 0.26g.

#![forbid(unsafe_code)]

use crate::vostok::{ArtefactPickup, Context, Flow, NoExit, ScoreEvent, Slots};

/// What the score's extra 10 is counted in.
const INTO_A_FREE_SLOT: ScoreEvent = ScoreEvent::WINNING_TEAM_VI_BONUS;

/// use_execute before transfer_artefact (0x31C810).
pub fn on_transfer(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(pickup) = ArtefactPickup::at_transfer(ctx)
        && pickup.slots() == Slots::Free
    {
        pickup.count(INTO_A_FREE_SLOT);
    }
    Flow::Continue
}
