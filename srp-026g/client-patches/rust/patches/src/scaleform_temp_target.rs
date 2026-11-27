//! Patch 216: a Scaleform filter whose temporary render target could not be
//! made no longer leaves a null target on the HAL's render-target stack.
//!
//! The stock HAL pushes whatever CreateTempRenderTarget returned, 0 too, and
//! an uncached filter without a target is never popped again: the next
//! filter's PopRenderTarget reads the target below its own through the 0
//! (revision 73's crash at 0x2B3777, 2026-11-26, a player's game in Artifact
//! Hunt; client_patch.md section 3.113). The hooks are
//! `scaleform_temp_target.toml`'s, both at that call's return:
//!
//! - PushFilters: nothing pushed; the filter's content is drawn unfiltered
//!   into the target below this frame ([`FailedFilter::give_up`]);
//! - drawCachedFilter's cached pass: not drawn this frame, the primitive's
//!   cache let go so the next frame draws it from the start
//!   ([`FailedCachedPass::give_up`]).
//!
//! Each failure is logged with the size asked for, the 1st, 10th, 100th, ...
//! of the run: what Direct3D refused (a size past its limit, or no memory)
//! is not known from the one report. Nothing a match computes: the
//! interface's rendering only.

#![forbid(unsafe_code)]

use core::sync::atomic::{AtomicU32, Ordering};

use crate::vostok::log::{self, Piece};
use crate::vostok::scaleform_filter::{FailedCachedPass, FailedFilter, Size};
use crate::vostok::{
    Context, Flow, ScaleformTempTargetOnCachedFilterTargetExit, ScaleformTempTargetOnFilterTargetExit,
};

/// The run's failures (both sites, the render thread).
static FAILURES: AtomicU32 = AtomicU32::new(0);

/// PushFilters after CreateTempRenderTarget (0x2B3CFC).
pub fn on_filter_target(ctx: &mut Context<'_>) -> Flow<ScaleformTempTargetOnFilterTargetExit> {
    let Some(failed) = FailedFilter::at(ctx) else { return Flow::Continue };
    said("a filter's temporary target", failed.size(), "drawn without its filter");
    failed.give_up();
    Flow::Exit(ScaleformTempTargetOnFilterTargetExit::Unfiltered)
}

/// drawCachedFilter after its last pass's CreateTempRenderTarget (0x2B4863).
pub fn on_cached_filter_target(ctx: &mut Context<'_>) -> Flow<ScaleformTempTargetOnCachedFilterTargetExit> {
    let Some(failed) = FailedCachedPass::at(ctx) else { return Flow::Continue };
    said("a cached filter's last target", failed.size(), "not drawn this frame, its cache let go");
    failed.give_up();
    Flow::Exit(ScaleformTempTargetOnCachedFilterTargetExit::Skipped)
}

fn said(what: &str, size: Size, then: &str) {
    let n = FAILURES.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
    if !log::is_power_of_ten(n) {
        return;
    }
    log::line(&[Piece::Text("scaleform_temp_target: "), Piece::Text(what), Piece::Text(" ("),
                Piece::Dec(size.width), Piece::Text(" x "), Piece::Dec(size.height),
                Piece::Text(") was not made: "), Piece::Text(then), Piece::Text("; failure "), Piece::Dec(n)]);
}
