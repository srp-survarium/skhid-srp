//! A body at rest keeps its centre on the floor (the owner's rule, 2026-10-26: "how about bots standing not in the air?").
//!
//! [`path::off_lip`] moves the goal of a walk that lies on a lip. This watches the body itself, wherever its rest comes
//! from (a landing, a fight stand's end, a wait, a stuck BackOff's end, a hold): a body that stands still for
//! [`STILL_MS`] on a lip, or within the body's radius of one, steps inward to the nearest floor that is neither
//! ([`path::off_lip`]), once. Not while the reflex aims at something (a shot from cover is the play's choice).
//!
//! The J0 rows show the symptom: a rest on a flat deck with the contact normal 0.58-0.80 is the capsule hanging over
//! the rim by 0.23-0.31 m (sin(theta) = d / 0.38 with n.y = cos(theta)), 90-600 s at a time (A84 items 13 and 14).
//! The rests on a lip are counted whether the step is on or off ([`stops`]), so the bench shows the drop.

use mapdata::{Vec3, WalkGrid};

use super::path;

/// A body is at rest after standing still this long, ms.
pub const STILL_MS: u32 = 500;
/// Still means under this ground speed, m/s.
pub const STILL_SPEED_MS: f32 = 0.3;
/// The inward step is given up after this long, ms.
pub const STEP_MS: u32 = 2_000;
/// The step is done within this of its target, metres.
pub const STEP_ARRIVE_M: f32 = 0.25;

/// What the body is doing as to its rest.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum RestWatch {
    /// Walking, fighting, or not yet still.
    #[default]
    Moving,
    /// Still since `since`; `last` is the tick before (for the seconds counted); `counted`: this rest is in the
    /// count of rests on a lip.
    Still { since: u32, last: u32, counted: bool },
    /// Walking inward to `to`, given up at `until`.
    Stepping { to: Vec3, until: u32 },
}

impl RestWatch {
    /// One tick. `resting`: the play wants no walk and the reflex aims at nothing; `speed`: the body's ground speed;
    /// `act`: the switch (`think.rest_off_lip`): off, the rests are only counted. Some(target): the point to step to
    /// now (the caller makes it the goal).
    pub fn tick(&mut self, time: u32, resting: bool, speed: f32, feet: Vec3, grid: &WalkGrid, act: bool) -> Option<Vec3> {
        match *self {
            Self::Stepping { to, until } if resting && time < until && feet.distance_xz(to) > STEP_ARRIVE_M => Some(to),
            Self::Stepping { .. } => {
                *self = Self::Moving;
                None
            }
            _ if !resting || speed > STILL_SPEED_MS => {
                *self = Self::Moving;
                None
            }
            Self::Moving => {
                *self = Self::Still { since: time, last: time, counted: false };
                None
            }
            Self::Still { since, last, counted } => {
                if time < since + STILL_MS {
                    return None;
                }
                let on_lip = grid.locate(feet, 1.0).is_some_and(|node| grid.is_lip(node) || grid.near_lip(node));
                if !on_lip {
                    *self = Self::Still { since, last: time, counted };
                    return None;
                }
                stops::seconds(time.saturating_sub(last));
                if !counted {
                    stops::rest();
                }
                *self = Self::Still { since, last: time, counted: true };
                let to = path::off_lip(grid, feet).filter(|_| act)?;
                *self = Self::Stepping { to, until: time + STEP_MS };
                path::rests::count();
                Some(to)
            }
        }
    }
}

/// The bench's counts of the rests on a lip (still 0.5 s or more on a lip or within the body's radius of one): the
/// rests, and the milliseconds spent in them.
pub mod stops {
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTS: [AtomicU64; 2] = [const { AtomicU64::new(0) }; 2];

    pub fn rest() {
        COUNTS[0].fetch_add(1, Ordering::Relaxed);
    }

    pub fn seconds(ms: u32) {
        COUNTS[1].fetch_add(u64::from(ms), Ordering::Relaxed);
    }

    pub fn counts() -> [u64; 2] {
        std::array::from_fn(|k| COUNTS[k].load(Ordering::Relaxed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v026g::bots::body::tests::floor;

    fn deck() -> WalkGrid {
        // A 20 m deck at height 5 over a ground 5 m lower: the edge along x = 10.
        let mut tris = floor(0.0, 0.0, 10.0, 10.0, 5.0).to_vec();
        tris.extend(floor(10.0, 0.0, 20.0, 10.0, 0.0));
        WalkGrid::build(&tris, [0.0, 0.0, 20.0, 10.0], 0.25, mapdata::Body::default())
    }

    /// A body still for half a second on a lip steps inward once (switch on); well inside the deck, or walking, or
    /// aiming, it does not; with the switch off the rest is counted and nothing is done.
    #[test]
    fn a_body_still_on_a_lip_steps_inward_once() {
        let grid = deck();
        let on_edge = Vec3::new(9.9, 5.0, 5.0);
        let run = |act: bool, resting: bool, speed: f32, feet: Vec3| {
            let mut watch = RestWatch::default();
            let mut steps = Vec::new();
            for tick in 0..60u32 {
                let time = 1_000 + tick * 50;
                steps.extend(watch.tick(time, resting, speed, feet, &grid, act));
            }
            steps
        };
        let before = stops::counts();
        let steps = run(true, true, 0.0, on_edge);
        assert!(!steps.is_empty(), "a step");
        let first = steps[0];
        assert!(first.x < on_edge.x - 0.4, "{first:?}");
        // The same target each tick until the step is done or given up.
        assert!(steps.iter().all(|&at| at == first), "{steps:?}");
        assert!(run(true, true, 0.0, Vec3::new(5.0, 5.0, 5.0)).is_empty(), "well inside");
        assert!(run(true, true, 0.0, Vec3::new(15.0, 0.0, 5.0)).is_empty(), "on the ground below");
        assert!(run(true, true, 2.0, on_edge).is_empty(), "walking");
        assert!(run(true, false, 0.0, on_edge).is_empty(), "aiming or walking to a goal");
        assert!(run(false, true, 0.0, on_edge).is_empty(), "switch off: only counted");
        let after = stops::counts();
        assert!(after[0] >= before[0] + 2, "{before:?} {after:?}");
        assert!(after[1] >= before[1] + 1_000, "{before:?} {after:?}");
    }
}
