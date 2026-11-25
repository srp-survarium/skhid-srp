//! The retail weapon projection belongs to the first-person camera for this
//! frame. Send the lens with its scene draw so the render thread may lag.
#![forbid(unsafe_code)]

use crate::vostok::{Context, Flow, Lock, NoExit, trig, weapon_view as engine};
use engine::ViewId;

#[derive(Clone, Copy)]
pub(crate) struct Focal(f32);

/// Multipliers of the retail camera's 65-degree vertical FOV.
#[derive(Clone, Copy)]
pub(crate) struct Profile {
    hip: f32,
    aimed: f32,
}

impl Profile {
    pub(crate) fn parse([hip, aimed]: [f32; 2]) -> Option<Self> {
        (hip > 0.0 && hip <= 1.0 && aimed > 0.0 && aimed <= 1.0).then_some(Self { hip, aimed })
    }

    fn focal_at(self, progress: f32) -> Option<Focal> {
        let factor = self.hip + (self.aimed - self.hip) * progress.clamp(0.0, 1.0);
        trig::focal_length(f64::from(factor) * 32.5_f64.to_radians()).map(Focal)
    }
}

impl Focal {
    pub(crate) fn decode(value: f32) -> Option<Self> {
        (value.is_finite() && value > 0.0).then_some(Self(value))
    }

    pub(crate) fn value(self) -> f32 { self.0 }

    pub(crate) fn diagonal(self, world_x: f32, world_y: f32) -> Option<[f32; 2]> {
        let x = self.0 * (world_x / world_y);
        (x.is_finite() && x > 0.0).then_some([x, self.0])
    }
}

static CAMERA: Lock<Option<(ViewId, Focal)>> = Lock::new(None);
static DRAW: Lock<Option<Focal>> = Lock::new(None);

pub fn on_view_frame(ctx: &mut Context<'_>) -> Flow<NoExit> {
    CAMERA.with(ctx.thread(), |camera| *camera = None);
    Flow::Continue
}

pub fn on_view_camera(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let sample = engine::camera(ctx).and_then(|(view, factors, progress)| Some((view, factors.focal_at(progress)?)));
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
    fn interpolates_angles_before_projecting_and_preserves_aspect() {
        for (progress, angle) in [(0.0, 65.0_f32), (0.5, 48.75), (1.0, 32.5)] {
            let focal = Profile::parse([1.0, 0.5]).unwrap().focal_at(progress).unwrap();
            let expected = 1.0 / (angle.to_radians() / 2.0).tan();
            assert!((focal.value() - expected).abs() < 0.000001);
            for aspect in [4.0/3.0, 16.0/9.0, 21.0/9.0] {
                let [x, y] = focal.diagonal(2.0/aspect, 2.0).unwrap();
                assert!((x * aspect - y).abs() < 0.000001);
            }
        }
        assert!(Profile::parse([0.75, 0.6]).unwrap().focal_at(0.0).unwrap().value() > Profile::parse([1.0, 0.5]).unwrap().focal_at(0.0).unwrap().value());
        assert!(Profile::parse([0.0, 0.5]).is_none());
        assert!(Profile::parse([1.0, 0.5]).unwrap().focal_at(f32::NAN).is_none());
        assert!(Focal::decode(0.0).is_none());
    }
}
