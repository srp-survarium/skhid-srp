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

impl Default for LineVertex {
    fn default() -> Self {
        LineVertex { at: [0.0; 3], color: LineColor(0) }
    }
}

/// The most vertices a draw_lines of ours takes: half the system
/// renderer's vertex stream (+74h, 100000h bytes of 10h-byte vertices,
/// made at 0x11C265; its lock 0x3D7640 starts over at 0 when a lock does
/// not fit and never checks a lock against the stream's size).
const CHUNK: usize = 32 * 1024;

/// Lines to draw, in pages of our own (kernel32's, never an engine arena),
/// made once and kept while the game runs; with the indices 0..CHUNK
/// every draw takes.
pub struct LineBuffer {
    vertices: &'static mut [LineVertex],
    indices: &'static [u16],
    len: usize,
}

impl LineBuffer {
    /// Room for `lines` lines; none when the address space has no hole for it.
    pub fn make(thread: &Thread, lines: usize) -> Option<LineBuffer> {
        let vertices = super::memory_profile::reserve::<LineVertex>(thread, 2 * lines)?.keep();
        let indices = super::memory_profile::reserve::<u16>(thread, CHUNK)?.keep();
        indices.iter_mut().enumerate().for_each(|(i, index)| *index = i as u16);
        Some(LineBuffer { vertices, indices, len: 0 })
    }

    /// Over buffers of the caller's (the tests').
    #[cfg(test)]
    pub fn over(vertices: &'static mut [LineVertex], indices: &'static [u16]) -> LineBuffer {
        LineBuffer { vertices, indices, len: 0 }
    }

    pub fn clear(&mut self) {
        self.len = 0;
    }

    pub fn count(&self) -> usize {
        self.len / 2
    }

    pub fn is_full(&self) -> bool {
        self.len + 2 > self.vertices.len()
    }

    /// A line from `a` to `b` (the game's frame); none once it is full.
    pub fn push(&mut self, a: [f32; 3], b: [f32; 3], color: LineColor) {
        if self.is_full() {
            return;
        }
        self.vertices[self.len] = LineVertex { at: a, color };
        self.vertices[self.len + 1] = LineVertex { at: b, color };
        self.len += 2;
    }

    pub fn vertices(&self) -> &[LineVertex] {
        &self.vertices[..self.len]
    }

    /// Each line's ends moved by `f`.
    pub fn move_points(&mut self, f: impl Fn([f32; 3]) -> [f32; 3]) {
        self.vertices[..self.len].iter_mut().for_each(|v| v.at = f(v.at));
    }

    /// The lines a draw at a time: at most CHUNK vertices and as many indices.
    fn chunks(&self) -> impl Iterator<Item = (&[LineVertex], &[u16])> {
        self.vertices().chunks(CHUNK).map(|chunk| (chunk, &self.indices[..chunk.len()]))
    }
}

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

    /// The buffer's lines, CHUNK vertices a call.
    pub fn draw(&self, lines: &LineBuffer) {
        for (chunk, indices) in lines.chunks() {
            let range = chunk.as_ptr_range();
            let indices = indices.as_ptr_range();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_draw_at_most_chunk_vertices() {
        let vertices = Box::leak(vec![LineVertex::default(); 2 * 40_000].into_boxed_slice());
        let indices = Box::leak((0..CHUNK).map(|i| i as u16).collect::<Vec<_>>().into_boxed_slice());
        let mut lines = LineBuffer::over(vertices, indices);
        for i in 0..40_000 {
            lines.push([i as f32; 3], [0.0; 3], LineColor::opaque(0, 255, 0));
        }
        assert!(lines.is_full());
        lines.push([1.0; 3], [2.0; 3], LineColor::opaque(0, 255, 0));
        assert_eq!(lines.count(), 40_000, "full: no more");
        let sizes: Vec<_> = lines.chunks().map(|(v, i)| (v.len(), i.len(), i.last().copied())).collect();
        assert_eq!(sizes, [(CHUNK, CHUNK, Some(CHUNK as u16 - 1)), (CHUNK, CHUNK, Some(CHUNK as u16 - 1)),
                           (80_000 - 2 * CHUNK, 80_000 - 2 * CHUNK, Some((80_000 - 2 * CHUNK) as u16 - 1))]);
    }
}
