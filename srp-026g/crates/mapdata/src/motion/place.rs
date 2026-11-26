//! [body physics S6b] Where a layer puts a body before the shared model moves it: the client's controller made at a point.

use crate::controller::tick::Controller;
use crate::controller::Collision;
use crate::grid::controlled::{body_at, Stance, WalkStart};
use crate::Vec3;

/// A body of `stance` put on `feet` as the walk check puts one (`start`'s placing; [`body_at`]).
pub fn body(world: &Collision, start: WalkStart, stance: Stance, feet: Vec3) -> Controller {
    body_at(world, start, stance, feet)
}

/// A standing body with its feet at `feet`, where it is put ([`Controller::standing_at`]).
pub fn standing(feet: Vec3) -> Controller {
    Controller::standing_at(feet)
}
