//! One surface's motion vectors as `stage_postprocess::render_models`
//! (0x1B18B0, accumulate_motion_vectors' draw; renderer-026g.md 10.1) is
//! about to upload its previous transform (site `motion_vectors.previous`),
//! for patch 215 (first_person_motion). The render thread's.
//!
//! The previous transform is the surface's world matrix of the frame before
//! times the view matrix of the frame before (row vectors); a static
//! surface's shader (`motion_vectors_accumulation.ps`) projects its
//! model-space position through it with this frame's projection and writes
//! the pixel's position less that, with the object flag.

use super::context::Context;
use super::occlusion::Drawn;
use super::world::{floats, write_floats};
use crate::raw;

/// A row-vector 4x4 matrix, row after row.
pub type Matrix = [f32; 16];

/// A surface the pass draws motion vectors for, before its previous
/// transform goes to the shader.
pub struct MotionDraw {
    frame: raw::MotionVectorDrawFrame,
    pub drawn: Drawn,
    /// Its world matrix this frame.
    pub world: Matrix,
    /// The view matrix this frame draws with.
    pub view: Matrix,
}

impl MotionDraw {
    pub fn at(ctx: &Context<'_>) -> Option<MotionDraw> {
        // SAFETY: the site's EBP is the surface render_models draws, alive
        // for the pass, and its ESP render_models' frame: its matrix of this
        // frame (+28h) and the previous transform it made just before
        // (+0F0h, 0x1B1A15) and reads just after (0x1B1A1D), its stage
        // (+138h), whose renderer context holds the frame's view matrix.
        unsafe {
            let surface = raw::site::motion_vectors_previous::surface(ctx)?;
            let frame = raw::site::motion_vectors_previous::frame(ctx)?;
            let context = frame.stage()?.context()?;
            Some(MotionDraw {
                frame,
                drawn: Drawn::of_surface(surface),
                world: floats(frame.current()),
                view: floats(context.view()),
            })
        }
    }

    /// The previous transform the shader takes.
    pub fn set_previous(self, previous: Matrix) {
        // SAFETY: 16 f32 of render_models' own frame, which it reads next
        // (0x1B1A1D); nothing else holds them.
        unsafe { write_floats(self.frame.previous(), previous) }
    }
}
