//! Bounds at the native reads that the snapshot format cannot frame.
//! Valid indices execute the original instructions; invalid ones take the
//! loader's own absent-state/object path, or (a death animation) the set's
//! first entry.
#![forbid(unsafe_code)]
use crate::vostok::{Context, Flow, Reg, NoExit, DeserializeGuardsOnFsmExit, DeserializeGuardsOnAnomalyExit};
use crate::vostok::deserialize;

pub fn on_fsm(ctx: &mut Context<'_>) -> Flow<DeserializeGuardsOnFsmExit> {
    if deserialize::fsm_has_state(ctx) { Flow::Continue }
    else { Flow::Exit(DeserializeGuardsOnFsmExit::Done) }
}

pub fn on_anomaly(ctx: &mut Context<'_>) -> Flow<DeserializeGuardsOnAnomalyExit> {
    if deserialize::anomaly_has_index(ctx) { Flow::Continue }
    else { Flow::Exit(DeserializeGuardsOnAnomalyExit::NoState) }
}

pub fn on_artefact(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if deserialize::artefact_has_index(ctx) { Flow::Continue }
    else {
        ctx.set_reg(Reg::Eax, 0);
        Flow::Skip
    }
}

/// A dead selector's `m_animation_index` comes from the world as any byte;
/// the game's pick keeps it below the set's death animations. One past them
/// becomes the first, so the lists are never indexed outside their entries.
pub fn on_death_animation(ctx: &mut Context<'_>) -> Flow<NoExit> {
    deserialize::keep_death_animation_in_set(ctx);
    Flow::Continue
}
