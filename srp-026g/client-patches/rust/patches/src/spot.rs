//! Patch 38: a spot that hits a battery's body spots the battery
//! (client_patch.md section 3.35; the rules: docs/notes/spotting.md).
//!
//! A spot walks its ray's hits. A spottable hit with no player behind it
//! asks its owner for a geometry; a battery's rigid body has the owner
//! victory_item_core+58h, whose slots answer none, so the original took the
//! spottable bit out and went on. Here a battery's body asks the battery
//! itself, as the use box's path asks its geometry's first spottable: can
//! it be spotted, then spot it, and the walk ends spotted; else not
//! spotted, the next hit. Any other owner goes on as the original.

#![forbid(unsafe_code)]

use crate::vostok::spotting::SpotHit;
use crate::vostok::{Context, Flow, SpotOnHitOwnerExit};

/// `spot.hit_owner`: a spottable hit with no player behind it.
pub fn on_hit_owner(ctx: &mut Context<'_>) -> Flow<SpotOnHitOwnerExit> {
    let Some(battery) = SpotHit::at(ctx).battery() else {
        return Flow::Continue;
    };
    if !battery.can_be_spotted() {
        return Flow::Exit(SpotOnHitOwnerExit::NotSpotted);
    }
    battery.set_spotted();
    Flow::Exit(SpotOnHitOwnerExit::Spotted)
}
