//! The mouse look's steps (patch 110, docs/notes/mouse-pitch.md).
//!
//! `player_input_handler::on_mouse_move` (0x240200) turns the yaw by
//! x * pi/180 * s radians, s the yaw's step (the handler's fov factor +94h x
//! the option sensitivity x 0.1), and the look pitch by y * pi/180 * the
//! pitch's step * invert * 3/pi units. The stock pitch's step is the render
//! window's h / w x s (0x2402E9..0x2402FA, `mouse.pitch_step`); patch 46's
//! death camera turns by the same steps from its own code (`mouse.death_pitch`).

use super::context::Context;
use crate::raw;

/// on_mouse_move at its pitch step (`mouse.pitch_step`): the yaw's step the
/// game made and the handler's fov factor, read once; `set` writes the
/// pitch's step where the replaced lines leave it.
pub struct PitchStep<'a, 'c> {
    ctx: &'a mut Context<'c>,
    horizontal: f32,
    fov_factor: f32,
}

impl<'a, 'c> PitchStep<'a, 'c> {
    pub fn at(ctx: &'a mut Context<'c>) -> Option<PitchStep<'a, 'c>> {
        use raw::site::mouse_pitch_step as site;
        // SAFETY: the site's ESP is on_mouse_move's frame on the game's
        // thread; its [esp+18h] the handler (the function's ECX, alive for
        // the call), its [esp+28h] the float the game stored at 0x240289.
        let (horizontal, fov_factor) = unsafe {
            let frame = site::frame(ctx)?;
            let handler = frame.handler()?;
            (*frame.horizontal(), *handler.fov_factor())
        };
        Some(PitchStep { ctx, horizontal, fov_factor })
    }

    /// The yaw's step s: fov factor x option sensitivity x 0.1.
    pub fn horizontal(&self) -> f32 {
        self.horizontal
    }

    /// The handler's fov factor (both axes; 1 unaimed).
    pub fn fov_factor(&self) -> f32 {
        self.fov_factor
    }

    /// The pitch's step before the invert and 3/pi (XMM1), and the yaw's step
    /// in XMM2, as the replaced lines leave them; the caller skips them.
    pub fn set(self, pitch: f32) {
        self.ctx.set_xmm_f32(1, pitch);
        self.ctx.set_xmm_f32(2, self.horizontal);
    }
}

/// Patch 46's drag asking the pitch's sensitivity (`mouse.death_pitch`):
/// XMM1 holds the option sensitivity it loaded; `set` gives the pitch's.
pub struct DeathPitch<'a, 'c> {
    ctx: &'a mut Context<'c>,
}

impl<'a, 'c> DeathPitch<'a, 'c> {
    pub fn at(ctx: &'a mut Context<'c>) -> DeathPitch<'a, 'c> {
        DeathPitch { ctx }
    }

    /// The option sensitivity the drag loaded (0xA74AEC).
    pub fn sensitivity(&self) -> f32 {
        self.ctx.xmm_f32(1)
    }

    pub fn set(self, sensitivity: f32) {
        self.ctx.set_xmm_f32(1, sensitivity);
    }
}
