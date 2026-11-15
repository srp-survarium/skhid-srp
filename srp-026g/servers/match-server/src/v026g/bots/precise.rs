//! [S5] The motor's precise mode on a thin top (the railings design, Part D): while a bot's route runs along a held thin top (a
//! rail, a wall top: the builder's line floors, S3's `LineTop`), it steers on the top's centre line and does nothing that moves
//! it sideways off.
//!
//! The client's rules it keeps (the railings record, PROVEN):
//! - **F2:** a body at rest stays on a top while its axis is within half the top's width + 0.27 m of the line on a top whose faces
//!   hold by slope, + 0.38 m on walk faces; past that it slides off.
//! - **F2, F3f:** a walking or sprinting body is not slid; it leaves only when its axis is about half the width + 0.38 m off the
//!   line (the ground is searched 0.05 m under the feet, `step_down` 0x7E1B0), at any speed.
//!
//! So the bot steers on the line itself (not node to node), and two things follow from the bars:
//! - past the rest bar it turns back toward the line at most at 45 degrees (its look-ahead along the line is cut to its distance
//!   off), so a stop there never finds it off the top;
//! - it never stops (lets the walk keys go) while past the rest bar ([`Line::may_stop`]).
//!
//! Its own position error is under these bars on the ground: the reckoning's error across the move is 38 cm or more at 0.7 % of
//! 31,438 ground worlds (#2103, walking straight 0.4 %), and the drift lab's own-position read on real matches agrees.
//!
//! This module is the geometry and the decision only: the driver hands it the line at the feet and at the route's next floor, as
//! the grid's `line_top` gives them, and gates the sideways behaviours on [`Precise`].

use mapdata::Vec3;

/// How a top's faces hold a body at rest (S3's `TopFace`): by their slope, or as walk faces. Dont-walk tops are no floors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Face {
    BySlope,
    Walk,
}

/// F2: past half the width + this, a body at rest on a top whose faces hold by slope slides off, m.
pub const REST_BY_SLOPE_M: f32 = 0.27;
/// F2, F3f: past half the width + this, a body leaves a top while moving (and at rest on walk faces), m.
pub const MOVING_M: f32 = 0.38;

/// A held thin top's centre line through a floor (S3's `LineTop`), read once at the boundary: its ends at the top's height in
/// the census's walk order, its width, its faces, and the census segment it belongs to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Line {
    pub segment: u32,
    from: Vec3,
    /// The unit direction from `from` to `to`, flat (x, z), and the flat length.
    dir: (f32, f32),
    len: f32,
    to: Vec3,
    width: f32,
    face: Face,
}

/// Where a point is against a line: `along` from its start (under 0 before it, over its length past it), and `across` (signed:
/// positive to the left of its direction, flat), m.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Place {
    pub along: f32,
    pub across: f32,
}

impl Line {
    /// The line, or none when it is not one (a zero or non-finite width, ends under 1 cm apart flat).
    pub fn new(segment: u32, from: Vec3, to: Vec3, width: f32, face: Face) -> Option<Self> {
        let (dx, dz) = (to.x - from.x, to.z - from.z);
        let len = dx.hypot(dz);
        (width.is_finite() && width > 0.0 && len.is_finite() && len >= 0.01).then(|| Self {
            segment,
            from,
            dir: (dx / len, dz / len),
            len,
            to,
            width,
            face,
        })
    }

    pub fn place(&self, p: Vec3) -> Place {
        let (px, pz) = (p.x - self.from.x, p.z - self.from.z);
        let (ux, uz) = self.dir;
        Place { along: px * ux + pz * uz, across: ux * pz - uz * px }
    }

    /// F2: how far off the line a body at rest stays on, m.
    pub fn rest_bar(&self) -> f32 {
        self.width / 2.0
            + match self.face {
                Face::BySlope => REST_BY_SLOPE_M,
                Face::Walk => MOVING_M,
            }
    }

    /// F2, F3f: how far off the line a moving body stays on, m.
    pub fn moving_bar(&self) -> f32 {
        self.width / 2.0 + MOVING_M
    }

    /// Whether a body at `feet` may stop here: within the rest bar.
    pub fn may_stop(&self, feet: Vec3) -> bool {
        self.place(feet).across.abs() <= self.rest_bar()
    }

    /// The point on the line at `along` (clamped to its ends), at the top's height there.
    fn at(&self, along: f32) -> Vec3 {
        let t = along.clamp(0.0, self.len) / self.len;
        Vec3::new(
            self.from.x + (self.to.x - self.from.x) * t,
            self.from.y + (self.to.y - self.from.y) * t,
            self.from.z + (self.to.z - self.from.z) * t,
        )
    }

    /// The point a body at `feet` going toward `to` (`forward`) or toward `from` steers at: on the line, `look_ahead` m on from
    /// the point nearest the feet; past the rest bar the look-ahead is cut to the distance off, so it turns back at 45 degrees
    /// at most. Clamped to the line's ends (the next floor's line takes over there).
    pub fn target(&self, feet: Vec3, forward: bool, look_ahead: f32) -> Vec3 {
        let place = self.place(feet);
        let off = place.across.abs();
        let ahead = if off > self.rest_bar() { look_ahead.min(off) } else { look_ahead };
        let along = place.along.clamp(0.0, self.len);
        self.at(if forward { along + ahead } else { along - ahead })
    }

    /// Whether going along `travel` (flat x, z) is going toward `to`.
    pub fn forward(&self, travel: (f32, f32)) -> bool {
        travel.0 * self.dir.0 + travel.1 * self.dir.1 >= 0.0
    }
}

/// The motor's mode by the lines at the feet and at the route's next floor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Precise {
    /// Neither is on a thin top.
    Off,
    /// On or onto a thin top, but the route leaves it (a drop, a jump or a climb off it, or onto another top): no sideways
    /// behaviour, the route's own steering.
    Guard,
    /// Along one top: steer on its line, toward `to` when `forward`.
    Steer { line: Line, forward: bool },
}

impl Precise {
    /// `at_feet`: the line of the floor at the feet; `next`: of the route's next floor; `travel`: the route's way (flat).
    pub fn of(at_feet: Option<Line>, next: Option<Line>, travel: (f32, f32)) -> Self {
        match (at_feet, next) {
            (Some(here), Some(there)) if here.segment == there.segment => {
                Precise::Steer { line: there, forward: there.forward(travel) }
            }
            (None, None) => Precise::Off,
            _ => Precise::Guard,
        }
    }

    /// No side step, strafe, sway, lane offset, apart step or sideways escape.
    pub fn holds_line(&self) -> bool {
        !matches!(self, Precise::Off)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rail(segment: u32, face: Face) -> Line {
        // A 0.10 m rail 1.2 m up, 4 m long along +x then rising 0.4 m.
        Line::new(segment, Vec3::new(0.0, 1.2, 0.0), Vec3::new(4.0, 1.6, 0.0), 0.10, face).unwrap()
    }

    #[test]
    fn a_line_is_read_once_and_refuses_what_is_not_one() {
        let (a, b) = (Vec3::new(0.0, 1.0, 0.0), Vec3::new(2.0, 1.0, 0.0));
        assert!(Line::new(1, a, b, 0.1, Face::BySlope).is_some());
        assert_eq!(Line::new(1, a, b, 0.0, Face::BySlope), None);
        assert_eq!(Line::new(1, a, b, f32::NAN, Face::BySlope), None);
        // Ends one above the other: no direction flat.
        assert_eq!(Line::new(1, a, Vec3::new(0.0, 2.0, 0.005), 0.1, Face::BySlope), None);
    }

    #[test]
    fn a_place_is_along_and_across_the_line_flat() {
        let line = Line::new(1, Vec3::new(1.0, 2.0, 1.0), Vec3::new(4.0, 2.0, 5.0), 0.1, Face::BySlope).unwrap();
        // Along (0.6, 0.8): 2.5 m along, then 0.5 m to the left (-0.8, 0.6); height is not counted.
        let p = Vec3::new(1.0 + 0.6 * 2.5 - 0.8 * 0.5, 7.0, 1.0 + 0.8 * 2.5 + 0.6 * 0.5);
        let place = line.place(p);
        assert!((place.along - 2.5).abs() < 1e-5 && (place.across - 0.5).abs() < 1e-5, "{place:?}");
        let right = line.place(Vec3::new(1.0 + 0.8 * 0.3, 2.0, 1.0 - 0.6 * 0.3));
        assert!((right.across + 0.3).abs() < 1e-5, "{right:?}");
    }

    #[test]
    fn the_bars_are_the_clients() {
        let slope = rail(1, Face::BySlope);
        let walk = rail(1, Face::Walk);
        assert!((slope.rest_bar() - 0.32).abs() < 1e-6);
        assert!((slope.moving_bar() - 0.43).abs() < 1e-6);
        assert!((walk.rest_bar() - 0.43).abs() < 1e-6);
        assert!(slope.may_stop(Vec3::new(2.0, 1.4, 0.31)));
        assert!(!slope.may_stop(Vec3::new(2.0, 1.4, 0.34)));
        assert!(walk.may_stop(Vec3::new(2.0, 1.4, -0.42)));
    }

    #[test]
    fn the_target_is_on_the_line_ahead_and_turns_back_at_45_degrees_past_the_rest_bar() {
        let line = rail(1, Face::BySlope);
        // 0.2 m off (inside the rest bar 0.32): the full look-ahead, on the line, at its height.
        let t = line.target(Vec3::new(1.0, 1.3, 0.2), true, 0.8);
        assert!((t.x - 1.8).abs() < 1e-5 && t.z.abs() < 1e-6 && (t.y - 1.38).abs() < 1e-5, "{t:?}");
        // 0.5 m off: the look-ahead is cut to 0.5 m, a 45-degree turn back.
        let t = line.target(Vec3::new(1.0, 1.3, -0.5), true, 0.8);
        assert!((t.x - 1.5).abs() < 1e-5 && t.z.abs() < 1e-6, "{t:?}");
        // Going toward `from`, and clamped at the ends.
        let t = line.target(Vec3::new(1.0, 1.3, 0.0), false, 0.8);
        assert!((t.x - 0.2).abs() < 1e-5, "{t:?}");
        let near = |a: Vec3, b: Vec3| a.distance_xz(b) < 1e-5 && (a.y - b.y).abs() < 1e-5;
        assert!(near(line.target(Vec3::new(3.9, 1.6, 0.0), true, 0.8), Vec3::new(4.0, 1.6, 0.0)));
        assert!(near(line.target(Vec3::new(-1.0, 1.2, 0.0), false, 0.8), Vec3::new(0.0, 1.2, 0.0)));
    }

    #[test]
    fn the_mode_steers_only_along_one_top() {
        let here = rail(7, Face::BySlope);
        let along = rail(7, Face::BySlope);
        let other = rail(8, Face::BySlope);
        assert_eq!(Precise::of(Some(here), Some(along), (1.0, 0.0)), Precise::Steer { line: along, forward: true });
        assert_eq!(Precise::of(Some(here), Some(along), (-1.0, 0.1)), Precise::Steer { line: along, forward: false });
        // Onto a top, off one, or from one onto another: guarded, not steered.
        assert_eq!(Precise::of(None, Some(along), (1.0, 0.0)), Precise::Guard);
        assert_eq!(Precise::of(Some(here), None, (1.0, 0.0)), Precise::Guard);
        assert_eq!(Precise::of(Some(here), Some(other), (1.0, 0.0)), Precise::Guard);
        assert_eq!(Precise::of(None, None, (1.0, 0.0)), Precise::Off);
        assert!(Precise::Guard.holds_line() && !Precise::Off.holds_line());
    }
}
