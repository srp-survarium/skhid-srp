//! Publish native medical-effect timing without changing simulation state.
#![forbid(unsafe_code)]
use crate::vostok::{Context, Flow, NoExit, QuickSlotActivity};
pub fn on_update(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(slot) = QuickSlotActivity::at_update(ctx) {
        slot.publish();
    }
    Flow::Continue
}
