//! player::update_camera 0x1F3480 at `camera.eye` (sites_026g.tsv): the
//! camera's player, EAX the eye transform the view is given (the player's
//! +44Ch in the first person). And the camera's angle (patch 22's uro_fov).

use core::cell::UnsafeCell;

use super::context::{Context, Reg};
use super::hook::Thread;
use super::world::{Player, floats, read};
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

/// The transform the view is given at this site (EAX, after an earlier
/// hook's [`look_from`]): a row-vector 4x4, its rows right, up, forward and
/// the place; what the screen shows this frame.
pub fn view_transform(ctx: &Context<'_>) -> Option<[f32; 16]> {
    // SAFETY: the context is this site's: EAX the eye transform update_camera
    // gives the view (the player's +44Ch, its third person's copy on the
    // stack, or EYE), 16 f32 alive while it runs on its thread.
    unsafe { raw::site::camera_eye::eye(ctx).map(|eye| floats(eye.value())) }
}

/// The camera's vertical angle unaimed, in degrees: uro_fov, the Settings'
/// FOV (patch 22's projection: an aim narrows it).
pub fn field_of_view(_: &Thread) -> f32 {
    // SAFETY: the console variable's value, in the image for the whole run;
    // only the game's thread (the Thread's) sets it.
    unsafe { read(raw::image::uro_fov().value()) }
}
