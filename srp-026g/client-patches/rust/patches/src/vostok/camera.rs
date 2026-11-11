//! player::update_camera 0x1F3480 at `camera.eye` (sites_026g.tsv): the
//! camera's player, EAX the eye transform the view is given (the player's
//! +44Ch in the first person).

use core::cell::UnsafeCell;

use super::context::{Context, Reg};
use super::world::{Player, floats};
use crate::raw;

/// The camera's player, and whether the view is given its own eye (the
/// first person).
pub fn eye_player<'t>(ctx: &'t Context<'_>) -> Option<(Player<'t>, bool)> {
    // SAFETY: the context is this site's: the camera's player, alive while
    // update_camera runs on its thread.
    let player = Player::of(ctx, unsafe { raw::site::camera_eye::player(ctx) })?;
    Some((player, ctx.reg(Reg::Eax) == player.eye_address()))
}

/// The eye the view is given instead, a row-vector 4x4: kept in [`EYE`],
/// which the view reads after the hook (until the next frame's).
pub fn look_from(ctx: &mut Context<'_>, eye: [f32; 16]) {
    // SAFETY: update_camera runs on the game's one render thread (the
    // Context's): the only writer, and the view reads it on that thread.
    unsafe { *EYE.0.get() = eye };
    ctx.set_reg(Reg::Eax, EYE.0.get() as usize as u32);
}

struct Eye(UnsafeCell<[f32; 16]>);

// SAFETY: as look_from: one thread writes and reads it.
unsafe impl Sync for Eye {}

/// The eye given to the view (not 0: the arena's data, beside the crate's
/// other statics).
static EYE: Eye = Eye(UnsafeCell::new([1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0]));

/// What the camera showed at a frame: the camera's player's place (its
/// simulated transform's, +40Ch, the world coordinates the map projects
/// use) and the view's transform (its handler's +48h, a row-vector 4x4 as
/// update_inverted_view writes it: the rows right, up, forward and the
/// place). In first person (the handler's modes 0 and 4) the view is the
/// eyes' (+44Ch, or patch 76's ragdoll eye); otherwise its third-person step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraView {
    pub body: [f32; 3],
    pub eye: [f32; 3],
    pub forward: [f32; 3],
}

/// update_camera after the view's update (`camera.viewed`): the view just
/// written for the camera's player.
pub fn viewed(ctx: &Context<'_>) -> Option<CameraView> {
    // SAFETY: the context is this site's: EBX the camera's player (update_
    // camera returned earlier for any other), alive while update_camera runs
    // on its thread; its handler is the one whose view the call before the
    // site wrote (+48h, 16 floats), its transform 16 floats too.
    unsafe {
        let player = raw::site::camera_viewed::player(ctx)?;
        let handler = player.camera_handler()?;
        let body: [f32; 16] = floats(player.transform());
        let view: [f32; 16] = floats(handler.view());
        Some(CameraView {
            body: [body[12], body[13], body[14]],
            eye: [view[12], view[13], view[14]],
            forward: [view[8], view[9], view[10]],
        })
    }
}
