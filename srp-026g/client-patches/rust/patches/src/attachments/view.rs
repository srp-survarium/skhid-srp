//! The first-person projection as 0.69d0's camera makes it
//! (`first_person_view_camera_controller::update_camera` 0x2002C2D70 of its
//! exe; `docs/notes/attachments-scopes.md`, "Why the EOTech looks small on
//! the A-545"): the world at the view's FOV and the sight's zoom, and
//! everything held, the gun, its parts and its optic with its reticle, at a
//! projection of its own, 65 degrees times the gun item's factor, from its
//! hip one to its aimed one by the aim; anything that is no gun at 65
//! degrees. The camera samples that lens each frame; the scene's draw
//! command carries it to the render thread, whose foreground pass takes it
//! in place of the world's.
//!
//! One exception: a gun with an optic of its own (0.26g's PSO-1, Leupold,
//! Giperon, EOTech and Walther of the SVD, VSS, VSK-94, Rem 700, SV-98,
//! VSSK, HK416, MSBS, Kriss Vector and P90) is drawn at the world's
//! projection, as before: 0.26g made those models and their eye relief
//! (the gun's `aim_offset`) for it, and 0.69d0 has none of them.
#![forbid(unsafe_code)]

use crate::vostok::{Context, Flow, Lock, NoExit, trig, weapon_view as engine};
use engine::{HeldGun, Optic, ViewId};

/// A projection's vertical focal length, 1 / tan(fov / 2).
#[derive(Clone, Copy)]
pub(crate) struct Focal(f32);

/// A gun item's two factors of 0.69d0's 65 degrees: at the hip, fully aimed.
#[derive(Clone, Copy)]
pub(crate) struct Profile {
    hip: f32,
    aimed: f32,
}

impl Profile {
    /// What 0.69d0 holds anything with that gives no factors: 1 and 1
    /// (update_camera's 1.0, 0x2002C2DC6).
    const NONE: Profile = Profile { hip: 1.0, aimed: 1.0 };

    pub(crate) fn parse([hip, aimed]: [f32; 2]) -> Option<Self> {
        (hip > 0.0 && hip <= 1.0 && aimed > 0.0 && aimed <= 1.0).then_some(Self { hip, aimed })
    }

    /// The factors interpolated by the aim, then the angle (0.69d0's order).
    fn focal_at(self, progress: f32) -> Option<Focal> {
        let factor = self.hip + (self.aimed - self.hip) * progress.clamp(0.0, 1.0);
        trig::focal_length(f64::from(factor) * 32.5_f64.to_radians()).map(Focal)
    }
}

impl Focal {
    /// The command's field: 0 for no lens.
    pub(crate) fn decode(value: f32) -> Option<Self> {
        (value > 0.0).then_some(Self(value))
    }

    pub(crate) fn value(self) -> f32 {
        self.0
    }

    /// The two scale terms in place of the world projection's, its aspect kept.
    pub(crate) fn diagonal(self, world_x: f32, world_y: f32) -> [f32; 2] {
        [self.0 * (world_x / world_y), self.0]
    }
}

/// The lens of the gun update_camera holds.
fn lens(gun: HeldGun) -> Option<Focal> {
    match gun.optic {
        Optic::Own => None,
        Optic::None | Optic::Fitted => gun.profile.unwrap_or(Profile::NONE).focal_at(gun.progress),
    }
}

/// This frame's lens and the view it is for: written by the camera on the
/// game's thread, taken by the scene's draw command on it.
static CAMERA: Lock<Option<(ViewId, Focal)>> = Lock::new(None);
/// The lens of the draw command the render thread runs.
static DRAW: Lock<Option<Focal>> = Lock::new(None);

/// `game.tick`: a frame without a first-person camera has no lens.
pub fn on_view_frame(ctx: &mut Context<'_>) -> Flow<NoExit> {
    CAMERA.with(ctx.thread(), |camera| *camera = None);
    Flow::Continue
}

/// `weapon_view.camera`: a gun in first-person hands.
pub fn on_view_camera(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let sample = engine::gun_camera(ctx).and_then(|(view, gun)| Some((view, lens(super::sights::lowered(ctx, gun))?)));
    CAMERA.with(ctx.thread(), |camera| *camera = sample);
    Flow::Continue
}

/// `weapon_view.no_gun`: anything else in first-person hands, or nothing.
pub fn on_view_no_gun(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let sample = engine::bare_camera(ctx).and_then(|view| Some((view, Profile::NONE.focal_at(0.0)?)));
    CAMERA.with(ctx.thread(), |camera| *camera = sample);
    Flow::Continue
}

pub fn on_view_submit(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let sample = CAMERA.with(ctx.thread(), |camera| *camera);
    engine::submit(ctx, sample);
    Flow::Continue
}

pub fn on_view_begin(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let focal = engine::queued(ctx);
    DRAW.with(ctx.thread(), |draw| *draw = focal);
    Flow::Continue
}

pub fn on_view_end(ctx: &mut Context<'_>) -> Flow<NoExit> {
    DRAW.with(ctx.thread(), |draw| *draw = None);
    Flow::Continue
}

pub fn on_foreground(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(focal) = DRAW.with(ctx.thread(), |draw| *draw) {
        engine::project(ctx, focal);
    }
    Flow::Continue
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_factors_interpolate_before_the_angle_and_the_aspect_stays() {
        for (progress, angle) in [(0.0, 65.0_f32), (0.5, 48.75), (1.0, 32.5)] {
            let focal = Profile::parse([1.0, 0.5]).unwrap().focal_at(progress).unwrap();
            assert!((focal.value() - 1.0 / (angle.to_radians() / 2.0).tan()).abs() < 1e-6);
            for aspect in [4.0 / 3.0, 16.0 / 9.0, 21.0 / 9.0] {
                let [x, y] = focal.diagonal(2.0 / aspect, 2.0);
                assert!((x * aspect - y).abs() < 1e-6);
            }
        }
    }

    #[test]
    fn a_gun_with_its_own_optic_keeps_the_world_projection() {
        let gun = |optic| HeldGun { profile: Profile::parse([1.0, 0.5]), progress: 1.0, optic };
        let aimed = 1.0 / 16.25_f32.to_radians().tan();
        for optic in [Optic::None, Optic::Fitted] {
            assert!(lens(gun(optic)).is_some_and(|focal| (focal.value() - aimed).abs() < 1e-5));
        }
        assert!(lens(gun(Optic::Own)).is_none());
        let unknown = HeldGun { profile: None, ..gun(Optic::None) };
        assert!(lens(unknown).is_some_and(|focal| (focal.value() - 1.0 / 32.5_f32.to_radians().tan()).abs() < 1e-5));
    }
}
