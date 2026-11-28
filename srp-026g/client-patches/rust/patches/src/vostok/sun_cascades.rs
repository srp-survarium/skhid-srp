//! The sun's cascaded shadow maps as stage_shadow_direct::execute (0x172E30;
//! renderer-026g.md 4.5) holds them once it has placed the cascades for the
//! frame (site `shadow_direct.cascades_placed`), for patch 222
//! (sun_shadow_cut). The render thread's.
//!
//! With r_use_csm_scrolling (on in the game) a cascade keeps its static
//! casters' depth from frame to frame: the cached map is scrolled to where
//! the cascade moved and only the casters new to it are drawn again. The
//! stage rebuilds every cascade whole only while its `cache_reset` is set:
//! at its start and when the scene or the sun changed, for 100 frames.

use super::context::Context;
use super::world::{floats, read};
use crate::raw;

/// A place in the world, metres (x, y up, z).
pub type Point = [f32; 3];

/// The stage, its cascades placed around this frame's eye.
pub struct SunCascades {
    stage: raw::StageShadowDirect,
    /// The eye the frame draws from, the cascades' centre.
    pub eye: Point,
}

impl SunCascades {
    pub fn at(ctx: &Context<'_>) -> Option<SunCascades> {
        // SAFETY: the site's EBP is the stage execute runs (the renderer's
        // slot 0, alive with the renderer), and its +4 the renderer context
        // its constructor stored, whose eye set_v wrote for this frame.
        unsafe {
            let stage = raw::site::shadow_direct_cascades_placed::stage(ctx)?;
            let eye = floats(stage.context()?.eye());
            Some(SunCascades { stage, eye })
        }
    }

    /// Whether the stage rebuilds its cascades whole this frame already.
    pub fn rebuilding(&self) -> bool {
        // SAFETY: a byte of the live stage, read before execute reads it.
        unsafe { read(self.stage.cache_reset()) != 0 }
    }

    /// Every cascade rebuilt whole this frame, as at the stage's start, and
    /// cached again from the next: the stage's own reset, set with its count
    /// past the 100 frames so that it ends after this frame's flush
    /// (0x1730E2..0x1730ED).
    pub fn rebuild_this_frame(self) {
        // SAFETY: two bytes of the live stage that execute reads next
        // (0x172FCA, 0x1730E2) on this thread; nothing else writes them.
        unsafe {
            self.stage.cache_reset().write(1);
            self.stage.cache_reset_frames().write(101);
        }
    }
}
