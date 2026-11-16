//! renderer::render 0x13D010 at `render.debug_lines` (sites_026g.tsv): the
//! frame's depth-tested debug lines, on the render thread, right after the
//! scene's on_draw_scene(true) and before the scene's own queued lines. A
//! box drawn here goes through system_renderer::draw_aabb 0x11DD60, as the
//! render stages' own debug boxes do (0x14C209, 0x195ADA): its twelve edges
//! with draw_lines' technique 0, against the scene's depth. draw_aabb draws
//! nothing while the system renderer's effects are not made
//! (is_effects_ready 0x11C900).

use super::context::Context;
use super::geometry::Aabb;
use super::hook::Thread;
use crate::raw;

/// A line's colour as draw_lines' vertices hold it: one byte each of red,
/// green, blue and alpha, red the lowest (LIKELY R8G8B8A8, the game's
/// formats; the stages' red 0xFF0000FF).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LineColor(u32);

impl LineColor {
    pub const fn opaque(red: u8, green: u8, blue: u8) -> LineColor {
        LineColor(u32::from_le_bytes([red, green, blue, u8::MAX]))
    }

    pub fn packed(self) -> u32 {
        self.0
    }
}

/// The frame's depth-tested debug lines, while the site's code runs.
pub struct DebugLines<'t> {
    renderer: raw::SystemRenderer,
    _thread: &'t Thread,
}

impl<'t> DebugLines<'t> {
    /// At `render.debug_lines`; none before the system renderer exists.
    pub fn at(ctx: &'t Context<'_>) -> Option<DebugLines<'t>> {
        // SAFETY: the system renderer's global, a pointer the game sets once.
        let renderer = unsafe { raw::image::system_renderer_global().renderer() }?;
        Some(DebugLines { renderer, _thread: ctx.thread() })
    }

    pub fn draw_box(&self, aabb: Aabb, color: LineColor) {
        let corners = aabb.corners();
        let packed = color.packed();
        // SAFETY: the render thread, inside renderer::render where the game
        // draws its own debug lines; draw_aabb reads our six floats and the
        // colour before it returns.
        unsafe { raw::engine_draw_aabb(Some(self.renderer), corners.as_ptr(), &packed) };
    }
}
