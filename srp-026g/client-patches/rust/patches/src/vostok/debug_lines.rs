//! renderer::render 0x13D010 at `render.debug_lines` (sites_026g.tsv): the
//! frame's depth-tested debug lines, on the render thread, right after the
//! scene's on_draw_scene(true) and before the scene's own queued lines,
//! which the game draws next with system_renderer::draw_lines 0x11CD10,
//! technique 0: lines of ours go through it the same way, and filled
//! triangles through draw_triangles 0x11E370 (technique 0 too, then its own
//! wireframe pass, technique 4). Both draw nothing while the system
//! renderer's effects are not made (is_effects_ready 0x11C900).
//!
//! The system renderer's colored effect (effect_system_colored::compile
//! 0x16B2C0): technique 0 alpha-blends (SRC_ALPHA, INV_SRC_ALPHA, 0x16B35D),
//! with the states' defaults (state_descriptor::reset 0x3E2DC0: solid fill,
//! back faces culled; depth tested and written); technique 4 the same
//! blend, wireframe fill (0x16B608).

use super::context::Context;
use super::hook::Thread;
use crate::raw;

/// A vertex's colour as draw_lines' and draw_triangles' vertices hold it:
/// one byte each of red, green, blue and alpha, red the lowest (LIKELY
/// R8G8B8A8, the game's formats; the stages' red 0xFF0000FF).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LineColor(u32);

impl LineColor {
    pub const fn opaque(red: u8, green: u8, blue: u8) -> LineColor {
        LineColor(u32::from_le_bytes([red, green, blue, u8::MAX]))
    }

    /// This colour times `shade` (0 to 1), its alpha `alpha`.
    pub fn shaded(self, shade: f32, alpha: u8) -> LineColor {
        let [r, g, b, _] = self.0.to_le_bytes().map(|c| (f32::from(c) * shade.clamp(0.0, 1.0)) as u8);
        LineColor(u32::from_le_bytes([r, g, b, alpha]))
    }

    pub fn packed(self) -> u32 {
        self.0
    }
}

/// A vertex as draw_lines and draw_triangles take them (10h bytes: the
/// point in the game's frame, its colour).
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

/// The most vertices a draw of ours takes: half the system renderer's
/// vertex stream (+74h, 100000h bytes of 10h-byte vertices, made at
/// 0x11C265; its lock 0x3D7640 starts over at 0 when a lock does not fit
/// and never checks a lock against the stream's size), whole triangles.
const CHUNK: usize = 32 * 1024;
const TRIANGLE_CHUNK: usize = CHUNK / 3 * 3;

/// Lines and filled triangles to draw, in pages of our own (kernel32's,
/// never an engine arena), made once and kept while the game runs: the
/// lines' vertices from the room's start, the triangles' from its end, full
/// when they meet. With the indices every draw takes: 0..CHUNK for the
/// lines, each triangle both ways round for the fill (both its faces seen
/// with back faces culled).
pub struct LineBuffer {
    vertices: &'static mut [LineVertex],
    line_indices: &'static [u16],
    fill_indices: &'static [u16],
    /// Line vertices from the start.
    lines: usize,
    /// Triangle vertices at the end.
    filled: usize,
    /// Lines and triangles refused since the last clear: the room was full.
    refused: usize,
}

impl LineBuffer {
    /// Two buffers of room for `vertices` vertices each, sharing their
    /// indices; none (nothing kept) when the address space has no hole for
    /// all of it.
    pub fn make_pair(thread: &Thread, vertices: usize) -> Option<(LineBuffer, LineBuffer)> {
        use super::memory_profile::reserve;
        let (first, second) = (reserve::<LineVertex>(thread, vertices)?, reserve::<LineVertex>(thread, vertices)?);
        let (lines, fill) = (reserve::<u16>(thread, CHUNK)?, reserve::<u16>(thread, 2 * TRIANGLE_CHUNK)?);
        let (line_indices, fill_indices) = (lines.keep(), fill.keep());
        fill_indices_into(line_indices, fill_indices);
        let (line_indices, fill_indices): (&'static [u16], &'static [u16]) = (line_indices, fill_indices);
        let buffer = |vertices| LineBuffer { vertices, line_indices, fill_indices, lines: 0, filled: 0, refused: 0 };
        Some((buffer(first.keep()), buffer(second.keep())))
    }

    /// Over buffers of the caller's (the tests').
    #[cfg(test)]
    pub fn over(vertices: &'static mut [LineVertex]) -> LineBuffer {
        let line_indices = Box::leak(vec![0u16; CHUNK].into_boxed_slice());
        let fill_indices = Box::leak(vec![0u16; 2 * TRIANGLE_CHUNK].into_boxed_slice());
        fill_indices_into(line_indices, fill_indices);
        LineBuffer { vertices, line_indices, fill_indices, lines: 0, filled: 0, refused: 0 }
    }

    pub fn clear(&mut self) {
        self.lines = 0;
        self.filled = 0;
        self.refused = 0;
    }

    pub fn count(&self) -> usize {
        self.lines / 2
    }

    pub fn triangles(&self) -> usize {
        self.filled / 3
    }

    /// The vertices taken, lines' and triangles'.
    pub fn used(&self) -> usize {
        self.lines + self.filled
    }

    fn fits(&self, vertices: usize) -> bool {
        self.lines + self.filled + vertices <= self.vertices.len()
    }

    /// No room for another line.
    pub fn is_full(&self) -> bool {
        !self.fits(2)
    }

    /// A line or a triangle was refused since the last clear (not merely full).
    pub fn is_cut(&self) -> bool {
        self.refused > 0
    }

    /// A line from `a` to `b` (the game's frame); refused once it is full.
    pub fn push(&mut self, a: [f32; 3], b: [f32; 3], color: LineColor) {
        if !self.fits(2) {
            self.refused += 1;
            return;
        }
        self.vertices[self.lines] = LineVertex { at: a, color };
        self.vertices[self.lines + 1] = LineVertex { at: b, color };
        self.lines += 2;
    }

    /// A filled triangle (the game's frame); refused once it is full.
    pub fn push_triangle(&mut self, corners: [[f32; 3]; 3], color: LineColor) {
        if !self.fits(3) {
            self.refused += 1;
            return;
        }
        let end = self.vertices.len() - self.filled;
        for (vertex, at) in self.vertices[end - 3..end].iter_mut().zip(corners) {
            *vertex = LineVertex { at, color };
        }
        self.filled += 3;
    }

    pub fn vertices(&self) -> &[LineVertex] {
        &self.vertices[..self.lines]
    }

    fn triangle_vertices(&self) -> &[LineVertex] {
        &self.vertices[self.vertices.len() - self.filled..]
    }

    /// Each line's ends moved by `line`, each triangle's corners by `fill`.
    pub fn move_points(&mut self, line: impl Fn([f32; 3]) -> [f32; 3], fill: impl Fn([f32; 3]) -> [f32; 3]) {
        let (len, lines) = (self.vertices.len(), self.lines);
        self.vertices[..lines].iter_mut().for_each(|v| v.at = line(v.at));
        self.vertices[len - self.filled..].iter_mut().for_each(|v| v.at = fill(v.at));
    }

    /// The lines a draw at a time: at most CHUNK vertices and as many indices.
    fn line_chunks(&self) -> impl Iterator<Item = (&[LineVertex], &[u16])> {
        self.vertices().chunks(CHUNK).map(|chunk| (chunk, &self.line_indices[..chunk.len()]))
    }

    /// The triangles a draw at a time: whole triangles, each twice in the indices.
    fn triangle_chunks(&self) -> impl Iterator<Item = (&[LineVertex], &[u16])> {
        self.triangle_vertices().chunks(TRIANGLE_CHUNK).map(|chunk| (chunk, &self.fill_indices[..2 * chunk.len()]))
    }
}

/// 0..CHUNK; and each triangle i as (3i, 3i+1, 3i+2) then (3i, 3i+2, 3i+1).
fn fill_indices_into(lines: &mut [u16], fill: &mut [u16]) {
    lines.iter_mut().enumerate().for_each(|(i, index)| *index = i as u16);
    for (i, six) in fill.chunks_exact_mut(6).enumerate() {
        let first = (3 * i) as u16;
        six.copy_from_slice(&[first, first + 1, first + 2, first, first + 2, first + 1]);
    }
}

/// draw_lines' technique: 0, as the depth-tested pass's own lines (0x13DA08);
/// draw_triangles' override byte 0: its technique from the system renderer's
/// color write (+150h, 1 from its constructor 0x11C342) and grid mode (+140h,
/// 0 from 0x11C33B), so 0, the blended one.
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

    /// The buffer's filled triangles, then its lines over them, CHUNK
    /// vertices a call.
    pub fn draw(&self, lines: &LineBuffer) {
        // SAFETY: the render thread, inside renderer::render where the game
        // draws its own debug lines; draw_triangles and draw_lines copy our
        // vertices and indices into their streams before they return.
        unsafe {
            for (chunk, indices) in lines.triangle_chunks() {
                let (range, indices) = (chunk.as_ptr_range(), indices.as_ptr_range());
                raw::engine_draw_triangles(Some(self.renderer), range.start.cast(), range.end.cast(), indices.start,
                                           indices.end, DEPTH_TESTED);
            }
            for (chunk, indices) in lines.line_chunks() {
                let (range, indices) = (chunk.as_ptr_range(), indices.as_ptr_range());
                raw::engine_draw_lines(range.end.cast(), Some(self.renderer), range.start.cast(), indices.start,
                                       indices.end, DEPTH_TESTED);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GREEN: LineColor = LineColor::opaque(0, 255, 0);

    #[test]
    fn a_draw_at_most_chunk_vertices() {
        let vertices = Box::leak(vec![LineVertex::default(); 2 * 40_000].into_boxed_slice());
        let mut lines = LineBuffer::over(vertices);
        for i in 0..40_000 {
            lines.push([i as f32; 3], [0.0; 3], GREEN);
        }
        assert!(lines.is_full() && !lines.is_cut(), "full, nothing refused: it fits exactly");
        lines.push([1.0; 3], [2.0; 3], GREEN);
        assert_eq!(lines.count(), 40_000, "full: no more");
        assert!(lines.is_cut(), "a line refused: cut");
        let sizes: Vec<_> = lines.line_chunks().map(|(v, i)| (v.len(), i.len(), i.last().copied())).collect();
        assert_eq!(sizes, [(CHUNK, CHUNK, Some(CHUNK as u16 - 1)), (CHUNK, CHUNK, Some(CHUNK as u16 - 1)),
                           (80_000 - 2 * CHUNK, 80_000 - 2 * CHUNK, Some((80_000 - 2 * CHUNK) as u16 - 1))]);
    }

    #[test]
    fn lines_and_triangles_share_the_room() {
        let vertices = Box::leak(vec![LineVertex::default(); 3 * 30_000].into_boxed_slice());
        let mut lines = LineBuffer::over(vertices);
        lines.push([1.0; 3], [2.0; 3], GREEN);
        for i in 0..29_999 {
            lines.push_triangle([[i as f32; 3], [0.0; 3], [1.0; 3]], GREEN);
        }
        assert_eq!((lines.count(), lines.triangles()), (1, 29_999));
        assert!(!lines.is_cut() && lines.is_full(), "one vertex left: no line fits, nothing refused yet");
        lines.push_triangle([[0.0; 3]; 3], GREEN);
        assert!(lines.is_cut());
        assert_eq!(lines.triangle_vertices()[0].at, [29_998.0; 3], "the last triangle nearest the lines");
        let chunks: Vec<_> = lines.triangle_chunks().map(|(v, i)| (v.len(), i.len())).collect();
        let last = 3 * 29_999 - 2 * TRIANGLE_CHUNK;
        let whole = (TRIANGLE_CHUNK, 2 * TRIANGLE_CHUNK);
        assert_eq!(chunks, [whole, whole, (last, 2 * last)]);
        let (_, indices) = lines.triangle_chunks().next().unwrap();
        assert_eq!(indices[..12], [0, 1, 2, 0, 2, 1, 3, 4, 5, 3, 5, 4], "each triangle both ways round");
    }

    #[test]
    fn shaded_colours() {
        assert_eq!(GREEN.shaded(0.5, 77).packed().to_le_bytes(), [0, 127, 0, 77]);
        assert_eq!(GREEN.shaded(2.0, 0).packed().to_le_bytes(), [0, 255, 0, 0]);
    }
}
