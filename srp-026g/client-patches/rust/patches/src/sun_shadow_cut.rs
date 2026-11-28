//! Patch 221: after a camera cut the sun's shadows are drawn again whole
//! (renderer-026g.md 4.5).
//!
//! The game keeps each sun cascade's static casters from frame to frame
//! (r_use_csm_scrolling): the cached depth is scrolled to where the cascade
//! moved, and only a caster the cascade did not hold the frame before is
//! drawn again. That holds while the eye moves a little a frame. After a
//! jump of tens of metres (a respawn, a spectator's switch, the death
//! camera, a capture's next pose) a caster both places' cascades hold is
//! not drawn again, and its shadow can be gone until it leaves the cascade:
//! School (v0.69)'s ground at terrain_06 drawn in sun after the eye came
//! from the gym 42 m away, under a shade it has when the eye starts there
//! or walks there.
//!
//! When the eye moved more than `CUT` since the frame before, the stage
//! rebuilds every cascade whole that frame, with its own reset (the one it
//! runs at its start and when the sun changes), and caches again from the
//! next. A walk, a sprint or a fall moves the eye centimetres a frame.

#![forbid(unsafe_code)]

use crate::vostok::sun_cascades::{Point, SunCascades};
use crate::vostok::{Context, Flow, Lock, NoExit};

/// The most the eye moves between two frames without a cut, metres: a
/// sprint at 10 frames a second is under 1 m.
const CUT: f32 = 2.0;

/// Where the eye was the frame before.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Eye {
    Unseen,
    At(Point),
}

impl Eye {
    /// The eye this frame: whether the camera was cut to it, and where it is.
    fn moved_to(self, now: Point) -> (bool, Eye) {
        let cut = match self {
            Eye::Unseen => false,
            Eye::At(before) => {
                let d = [now[0] - before[0], now[1] - before[1], now[2] - before[2]];
                // A NaN eye is no cut: the comparison is false.
                d[0] * d[0] + d[1] * d[1] + d[2] * d[2] > CUT * CUT
            }
        };
        (cut, Eye::At(now))
    }
}

static EYE: Lock<Eye> = Lock::new(Eye::Unseen);

/// `shadow_direct.cascades_placed`: a cut rebuilds the cascades whole.
pub fn on_cascades_placed(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(cascades) = SunCascades::at(ctx) else {
        return Flow::Continue;
    };
    let cut = EYE.with(ctx.thread(), |eye| {
        let (cut, now) = eye.moved_to(cascades.eye);
        *eye = now;
        cut
    });
    if cut && !cascades.rebuilding() {
        cascades.rebuild_this_frame();
    }
    Flow::Continue
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_frame_is_no_cut() {
        assert_eq!(Eye::Unseen.moved_to([1.0, 2.0, 3.0]), (false, Eye::At([1.0, 2.0, 3.0])));
    }

    #[test]
    fn a_walk_is_no_cut() {
        let mut eye = Eye::Unseen;
        for step in 0..200 {
            let (cut, now) = eye.moved_to([0.15 * step as f32, 18.0, 0.0]);
            assert!(!cut, "step {step}");
            eye = now;
        }
    }

    #[test]
    fn a_jump_is_a_cut_once() {
        let (_, eye) = Eye::Unseen.moved_to([-11.88, 17.92, 123.48]);
        let (cut, eye) = eye.moved_to([-54.2, 18.25, 110.86]);
        assert!(cut);
        assert!(!eye.moved_to([-54.2, 18.25, 110.86]).0);
    }

    #[test]
    fn a_fall_is_no_cut_but_a_drop_of_metres_is() {
        let (_, eye) = Eye::Unseen.moved_to([0.0, 20.0, 0.0]);
        assert!(!eye.moved_to([0.0, 19.0, 0.0]).0);
        assert!(eye.moved_to([0.0, 17.5, 0.0]).0);
    }

    #[test]
    fn a_nan_eye_is_no_cut() {
        let (_, eye) = Eye::Unseen.moved_to([0.0, 0.0, 0.0]);
        assert!(!eye.moved_to([f32::NAN, 0.0, 0.0]).0);
    }
}
