//! player::update_camera 0x1F3480 at `camera.eye` (sites_026g.tsv): the
//! camera's player, EAX the eye transform the view is given (the player's
//! +44Ch in the first person); and at `camera.orbited`, after the view's
//! update (update_inverted_view 0x2422B0) wrote the camera's transform, the
//! orbit's pivot and eye.

use core::cell::UnsafeCell;

use ragdoll::math::Vec3;

use super::context::{Context, Reg};
use super::world::{Player, floats, write_floats};
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

/// The input handler's modes whose camera is the eyes' (update_camera's
/// `test ecx,ecx` / `cmp ecx,4` at 0x1F34A3, update_inverted_view's at
/// 0x2422CE): 0 the first person, 4 a spectator. Every other mode (3 the
/// warmup before the start) orbits a pivot.
const EYES_MODES: [u32; 2] = [0, 4];

/// The orbit camera at `camera.orbited`: update_inverted_view has turned the
/// view about the pivot (the player's place raised 1.4 m along its up) and
/// put the eye `distance` (+354h) out along the view's back and a fifth of
/// that to its right. Nothing of it tests the level; camera_director::apply
/// (0x1F365D) gives the view on after the site.
pub struct Orbit<'t> {
    player: Player<'t>,
    view: raw::PlayerInputHandler,
    pivot: Vec3,
}

impl<'t> Orbit<'t> {
    /// The orbit of this frame's camera; None in the eyes' modes (where the
    /// frame's pivot was not written).
    pub fn at(ctx: &'t Context<'_>) -> Option<Orbit<'t>> {
        // SAFETY: the context is this site's: EBX the camera's player, alive
        // while update_camera runs on the game's thread; its input handler
        // is what update_camera gave the view's update (0x1F3643).
        let player = Player::of(ctx, unsafe { raw::site::camera_orbited::player(ctx) })?;
        // SAFETY: as above.
        let view = unsafe { player.this.input_handler() }?;
        // SAFETY: the handler's own field, read by update_camera this frame.
        if EYES_MODES.contains(&unsafe { *view.mode() }) {
            return None;
        }
        // SAFETY: the site's frame, update_camera's own; in the orbit modes
        // its pivot was written before the view's update (0x1F3609).
        let pivot: [f32; 16] = unsafe { floats(raw::site::camera_orbited::frame(ctx)?.pivot()) };
        Some(Orbit { player, view, pivot: place(&pivot) })
    }

    /// The camera's player.
    pub fn player(&self) -> &Player<'t> {
        &self.player
    }

    /// The point the camera turns about.
    pub fn pivot(&self) -> Vec3 {
        self.pivot
    }

    /// The camera's transform, as the view's update wrote it.
    fn view(&self) -> [f32; 16] {
        // SAFETY: the handler's own run of 16 f32, written by the view's
        // update before the site; read and written on the game's thread.
        unsafe { floats(self.view.view()) }
    }

    /// Where the eye is.
    pub fn eye(&self) -> Vec3 {
        place(&self.view())
    }

    /// Puts the eye at `eye`, the view's turn kept; the view the director
    /// gives on is this one.
    pub fn set_eye(&mut self, eye: Vec3) {
        let mut v = self.view();
        v[12..15].copy_from_slice(&[eye.x, eye.y, eye.z]);
        // SAFETY: as view.
        unsafe { write_floats(self.view.view(), v) }
    }
}

/// A row-vector 4x4's place (its row 3).
fn place(m: &[f32; 16]) -> Vec3 {
    Vec3::new(m[12], m[13], m[14])
}
