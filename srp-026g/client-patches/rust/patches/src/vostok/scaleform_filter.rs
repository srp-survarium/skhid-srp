//! Scaleform's D3D1x HAL when a filter's temporary render target could not
//! be made (patch 216; client_patch.md section 3.113).
//!
//! An uncached filter draws its content into a temporary target
//! (HAL::PushFilters 0x2B3A50: CreateTempRenderTarget, then PushRenderTarget),
//! then the filter of that target into the one below (PopFilters 0x527AC0 ->
//! drawUncachedFilter 0x2B3E40, whose PopRenderTarget takes it off the HAL's
//! render-target stack). CreateTempRenderTarget returns 0 when Direct3D makes
//! no texture or view for it, and the stock code pushes the 0 anyway;
//! drawUncachedFilter then returns at its start (a primitive without a target)
//! and never pops it. The stack keeps a null entry, and the next filter's
//! PopRenderTarget reads the target below its own through it: the crash at
//! 0x2B3777 (revision 73, 2026-11-26). A primitive cached one pass from its
//! target (drawCachedFilter 0x2B44E0) pushes a 0 the same way, and its own
//! PopRenderTarget reads it at once (0x2B3655).
//!
//! The two sites here are those calls' returns, on the render thread:
//! - [`FailedFilter`] (`scaleform.filter_target`): the filter is not drawn;
//!   its content is drawn into the target below as it is, unfiltered, this
//!   frame. Nothing is pushed; the filter stack gets the primitive with no
//!   target, which PopFilters and drawUncachedFilter take off without a pop
//!   of the render-target stack. What PushFilters changed before the call is
//!   undone as the stock filter's end would: the masks' state set again, and
//!   HS_DrawingFilter cleared when no other filter is being drawn.
//! - [`FailedCachedPass`] (`scaleform.cached_filter_target`): the cached pass
//!   is not drawn (the primitive is missing this frame) and the primitive's
//!   cache is let go, so the next frame draws it from the start, as an
//!   uncached filter.

use core::marker::PhantomData;

use super::{Context, Thread};
use crate::raw;

/// HALState's HS_DrawingFilter: a filter's content is being drawn into its
/// temporary target (PushFilters 0x2B3C5A sets it, PopFilters 0x527C74
/// clears it when its filter stack is empty).
const DRAWING_FILTER: u32 = 0x80;

/// FilterPrimitive's cache state "none" (SetCacheResults' first argument:
/// drawUncachedFilter's uncached results, 0x2B4312).
const UNCACHED: u32 = 0;

/// The size a temporary target was asked for, in pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

/// Which depth-stencil state the masks are given again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MaskTest {
    /// The target has a stencil (stencil_mask_state).
    Stencil,
    /// It has a depth buffer and no stencil (depth_mask_state).
    Depth,
}

/// What drawUncachedFilter sets after its PopRenderTarget (0x2B4326..0x2B4360):
/// nothing while no mask is drawn, else the stencil's state if the target has a
/// stencil, else the depth buffer's if it has one, else nothing.
pub fn mask_test(mask_depth: u32, stencil: bool, depth: bool) -> Option<MaskTest> {
    match (mask_depth, stencil, depth) {
        (0, _, _) => None,
        (_, true, _) => Some(MaskTest::Stencil),
        (_, false, true) => Some(MaskTest::Depth),
        (_, false, false) => None,
    }
}

/// HALState once a filter that drew nothing is given up: HS_DrawingFilter
/// cleared when no other filter is being drawn (`filters` the filter stack's
/// size before this one goes on it), else as it is.
pub fn state_without_filter(state: u32, filters: u32) -> u32 {
    if filters == 0 { state & !DRAWING_FILTER } else { state }
}

/// The HAL at one of the sites, on the render thread.
#[derive(Clone, Copy)]
struct Hal<'t> {
    raw: raw::D3d1xHal,
    _thread: PhantomData<&'t Thread>,
}

impl Hal<'_> {
    /// The masks' depth-stencil state set again, as drawUncachedFilter does
    /// after its PopRenderTarget.
    fn mask_again(self) {
        // SAFETY: the HAL is alive while it draws (the site's EBX), on the
        // render thread, which alone draws with it; its flags and states
        // are what drawUncachedFilter reads at 0x2B4326..0x2B4356.
        let (depth, stencil, has_depth, context) = unsafe {
            (*self.raw.mask_stack_top(), *self.raw.stencil_available(), *self.raw.depth_available(),
             self.raw.context())
        };
        let Some(test) = mask_test(depth, stencil != 0, has_depth != 0) else { return };
        // SAFETY: as above, the state drawUncachedFilter passes.
        let state = unsafe {
            match test {
                MaskTest::Stencil => self.raw.stencil_mask_state(),
                MaskTest::Depth => self.raw.depth_mask_state(),
            }
        };
        // SAFETY: the HAL's immediate context on the render thread, which
        // owns it; OMSetDepthStencilState with the state and reference the
        // stock code gives it at 0x2B4360.
        unsafe { raw::engine_context_om_set_depth_stencil_state(context, state, depth) }
    }

    /// HS_DrawingFilter cleared when no other filter is being drawn.
    fn leave_filter(self) {
        // SAFETY: as mask_again; the flags and the filter stack's size are
        // the HAL's own, changed only on this thread.
        unsafe {
            let state = self.raw.state();
            *state = state_without_filter(*state, *self.raw.filters());
        }
    }
}

/// PushFilters whose temporary target was not made (`scaleform.filter_target`
/// with EAX 0).
pub struct FailedFilter<'t> {
    hal: Hal<'t>,
    size: Size,
}

impl<'t> FailedFilter<'t> {
    /// None when the target was made (the stock code goes on).
    pub fn at(ctx: &'t Context<'_>) -> Option<FailedFilter<'t>> {
        use raw::site::scaleform_filter_target as site;
        // SAFETY: the context is this site's: EAX CreateTempRenderTarget's
        // result, EBX the HAL whose PushFilters runs.
        if unsafe { site::target(ctx) } != 0 {
            return None;
        }
        // SAFETY: as above; the site's ESP+30h/+34h the size PushFilters
        // passed (its own frame, as the call left it).
        let (hal, size) = unsafe {
            let frame = site::frame(ctx)?;
            (site::hal(ctx)?, Size { width: *frame.width(), height: *frame.height() })
        };
        Some(FailedFilter { hal: Hal { raw: hal, _thread: PhantomData }, size })
    }

    pub fn size(&self) -> Size {
        self.size
    }

    /// The filter given up: the masks and HS_DrawingFilter as its end would
    /// leave them. The caller then exits to the filter stack's push.
    pub fn give_up(self) {
        self.hal.mask_again();
        self.hal.leave_filter();
    }
}

/// drawCachedFilter whose last pass's target was not made
/// (`scaleform.cached_filter_target` with EAX 0).
pub struct FailedCachedPass<'t> {
    primitive: raw::FilterPrimitive,
    size: Size,
    _thread: PhantomData<&'t Thread>,
}

impl<'t> FailedCachedPass<'t> {
    /// None when the target was made (the stock code goes on).
    pub fn at(ctx: &'t Context<'_>) -> Option<FailedCachedPass<'t>> {
        use raw::site::scaleform_cached_filter_target as site;
        // SAFETY: the context is this site's: EAX CreateTempRenderTarget's
        // result, EDI the primitive drawCachedFilter draws.
        if unsafe { site::target(ctx) } != 0 {
            return None;
        }
        // SAFETY: as above; ESP+2Ch/+30h the size drawCachedFilter passed.
        let (primitive, size) = unsafe {
            let frame = site::frame(ctx)?;
            (site::primitive(ctx)?, Size { width: *frame.width(), height: *frame.height() })
        };
        Some(FailedCachedPass { primitive, size, _thread: PhantomData })
    }

    pub fn size(&self) -> Size {
        self.size
    }

    /// The pass given up and the primitive's cache let go: drawn from the
    /// start next frame. The caller then exits to drawCachedFilter's end.
    pub fn give_up(self) {
        // SAFETY: SetCacheResults on the primitive drawCachedFilter draws,
        // on the render thread, as drawUncachedFilter calls it for an
        // uncached result (0x2B4317: the state 0, no results).
        unsafe { raw::engine_filter_set_cache_results(Some(self.primitive), UNCACHED, core::ptr::null(), 0) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_masks_are_set_again_as_draw_uncached_filter_does() {
        assert_eq!(mask_test(0, true, true), None);
        assert_eq!(mask_test(2, true, true), Some(MaskTest::Stencil));
        assert_eq!(mask_test(1, false, true), Some(MaskTest::Depth));
        assert_eq!(mask_test(1, false, false), None);
    }

    #[test]
    fn drawing_filter_is_cleared_only_with_no_other_filter() {
        assert_eq!(state_without_filter(0x2BF, 0), 0x23F);
        assert_eq!(state_without_filter(0x2BF, 1), 0x2BF);
        assert_eq!(state_without_filter(0x23F, 0), 0x23F);
    }
}
