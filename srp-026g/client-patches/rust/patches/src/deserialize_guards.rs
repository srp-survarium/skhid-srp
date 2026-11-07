//! Bounds at the three native reads that the snapshot format cannot frame.
//! Valid indices execute the original instructions; invalid ones take the
//! loader's own absent-state/object path.
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
