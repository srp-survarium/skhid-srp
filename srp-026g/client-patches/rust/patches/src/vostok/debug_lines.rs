//! renderer::render 0x13D010 at `render.debug_lines` (sites_026g.tsv): the
//! frame's depth-tested debug lines, on the render thread, right after the
//! scene's on_draw_scene(true) and before the scene's own queued lines,
//! which the game draws next with system_renderer::draw_lines 0x11CD10,
//! technique 0: lines of ours go through it the same way. draw_lines draws
//! nothing while the system renderer's effects are not made
//! (is_effects_ready 0x11C900).

use super::context::Context;
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

/// One end of a line, as draw_lines takes them (10h bytes: the point in the
/// game's frame, its colour).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineVertex {
    pub at: [f32; 3],
    pub color: LineColor,
}

impl LineVertex {
    pub const NONE: LineVertex = LineVertex { at: [0.0; 3], color: LineColor(0) };
}

/// The most vertices a draw_lines of ours takes: a line list's indices
/// 0..CHUNK, the same for every chunk.
const CHUNK: usize = 1024;

static INDICES: [u16; CHUNK] = {
    let mut out = [0u16; CHUNK];
    let mut i = 0;
    while i < CHUNK {
        out[i] = i as u16;
        i += 1;
    }
    out
};

/// draw_lines' technique: 0, as the depth-tested pass's own lines (0x13DA08).
const DEPTH_TESTED: u32 = 0;

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

    /// `vertices` as a line list, each pair a line, CHUNK vertices a call.
    pub fn draw(&self, vertices: &[LineVertex]) {
        for chunk in vertices.chunks(CHUNK) {
            let count = chunk.len() & !1;
            let range = chunk[..count].as_ptr_range();
            let indices = INDICES[..count].as_ptr_range();
            // SAFETY: the render thread, inside renderer::render where the
            // game draws its own debug lines; draw_lines copies our vertices
            // and indices into its streams before it returns.
            unsafe {
                raw::engine_draw_lines(range.end.cast(), Some(self.renderer), range.start.cast(), indices.start,
                                       indices.end, DEPTH_TESTED)
            };
        }
    }
}
