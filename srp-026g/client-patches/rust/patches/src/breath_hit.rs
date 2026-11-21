//! Patch 176: a hit that takes health breaks the victim's breath hold (the
//! owner, 2026-11-21: "when player gets hit they lose their breath holding";
//! docs/notes/breath-hit.md).
//!
//! - Any hit hit_impl goes on with (it took health past armour), of any type
//!   and by anyone: the rule patch 84 uses, for every hit type.
//! - The hold ends as running out of breath does: short breathing, and no new
//!   hold until it has passed.

#![forbid(unsafe_code)]

use crate::vostok::drug::AppliedHit;
use crate::vostok::{Context, Flow, NoExit};

/// hit_impl once the hit took health (`player.hit_applied`).
pub fn on_hit_applied(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(hit) = AppliedHit::at_applied(ctx)
        && let Some(breath) = hit.held_breath()
    {
        breath.spend();
    }
    Flow::Continue
}
