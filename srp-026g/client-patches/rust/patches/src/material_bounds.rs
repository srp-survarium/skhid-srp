//! Patch 193: game_material_manager::material_pair (0x306D50) takes an id
//! past its 128 material slots as the default material's, the answer it
//! gives an id whose slot is empty, instead of reading past its tables and
//! handing on whatever is there (a rev71 crash in step_manager::on_step was
//! one: a foot material of 0x6860, patch 101). Every caller of the pair, the
//! game's (steps, bullets, grenades, traps) and ours, goes through it. The
//! log says so at the 1st, 10th, 100th, .. of a run, with the id.

#![forbid(unsafe_code)]

use crate::vostok::log::{is_power_of_ten, line, Piece::*};
use crate::vostok::material::PairCall;
use crate::vostok::{Context, Flow, GameCell, MaterialBoundsOnMaterialPairExit};

/// Calls with an id past the table, this run.
static OUTSIDE: GameCell<u32> = GameCell::new(0);

pub fn on_material_pair(ctx: &mut Context<'_>) -> Flow<MaterialBoundsOnMaterialPairExit> {
    let Some(call) = PairCall::at_entry(ctx) else {
        return Flow::Continue;
    };
    let Some(id) = call.outside() else {
        return Flow::Continue;
    };
    let count = OUTSIDE.get(ctx.thread()).saturating_add(1);
    OUTSIDE.set(ctx.thread(), count);
    if is_power_of_ten(count) {
        line(&[
            Text("material_bounds: material id "),
            Hex(u32::from(id)),
            Text(" is past the 128 materials, taken as the default; "),
            Dec(count),
            Text(" this run"),
        ]);
    }
    call.answer_bounded(ctx);
    Flow::Exit(MaterialBoundsOnMaterialPairExit::Done)
}
