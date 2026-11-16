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
use mapdata::grid::{LineTop, TopFace as Face};

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
    /// The grid's line top (S3, `WalkGrid::line_top`) as a line, or none when it is not one ([`Self::new`]).
    pub fn of(top: &LineTop) -> Option<Self> {
        Self::new(top.segment, top.from, top.to, top.width, top.face)
    }

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

    /// The point on the line, extended past its ends, at `along`, at the top's height there (extended the same way).
    pub fn at(&self, along: f32) -> Vec3 {
        let t = along / self.len;
        Vec3::new(
            self.from.x + (self.to.x - self.from.x) * t,
            self.from.y + (self.to.y - self.from.y) * t,
            self.from.z + (self.to.z - self.from.z) * t,
        )
    }

    /// The point a body at `feet` going toward `to` (`forward`) or toward `from` steers at: on the line, `look_ahead` m on from
    /// the point nearest the feet; past the rest bar the look-ahead is cut to the distance off, so it turns back at 45 degrees
    /// at most. The line runs on past its piece (S3's piece is a cell long; the top runs on along its line), so the point is
    /// always ahead of the feet, never back at a piece's end.
    pub fn target(&self, feet: Vec3, forward: bool, look_ahead: f32) -> Vec3 {
        let place = self.place(feet);
        let off = place.across.abs();
        let ahead = if off > self.rest_bar() { look_ahead.min(off) } else { look_ahead };
        self.at(if forward { place.along + ahead } else { place.along - ahead })
    }

    /// Of the move keys in `keys` at view `yaw`, those that step along the line: each key's own way in the world within 45
    /// degrees of the line, either way (FORWARD the view's way, LEFT its left: a positive angle, `kin`; BACK behind, RIGHT its
    /// right). The other keys are kept.
    pub fn keys_along(&self, keys: u32, yaw: f32) -> u32 {
        use crate::v026g::drive::key;
        use std::f32::consts::{FRAC_1_SQRT_2, FRAC_PI_2, PI};
        let ways = [(key::FORWARD, 0.0), (key::LEFT, FRAC_PI_2), (key::BACK, PI), (key::RIGHT, -FRAC_PI_2)];
        let along = |angle: f32| {
            let [x, z] = super::kin::forward(yaw + angle);
            (x * self.dir.0 + z * self.dir.1).abs() > FRAC_1_SQRT_2
        };
        ways.iter().fold(keys, |kept, &(k, angle)| if kept & k != 0 && !along(angle) { kept & !k } else { kept })
    }

    /// The top's height under `p` (on the line extended).
    pub fn height_at(&self, p: Vec3) -> f32 {
        self.at(self.place(p).along).y
    }

    /// Whether going along `travel` (flat x, z) is going toward `to`.
    pub fn forward(&self, travel: (f32, f32)) -> bool {
        travel.0 * self.dir.0 + travel.1 * self.dir.1 >= 0.0
    }
}

/// Feet this far under the line's top have left it, m: the census's thin top has a drop of over 0.30 m beside it
/// (`railings-design.md` §3), so a body this far down is beside the top, not on it. A count's threshold only (decides nothing).
pub const BELOW_TOP_M: f32 = 0.30;

/// The motor's mode by the lines at the feet and at the route's next floor.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Precise {
    /// Neither is on a thin top.
    #[default]
    Off,
    /// On or onto a thin top, but the route leaves it (a drop, a jump or a climb off it, or onto another top): no sideways
    /// behaviour, the route's own steering.
    Guard,
    /// Along one top: steer on its line, toward `to` when `forward`.
    Steer { line: Line, forward: bool },
}

impl Precise {
    /// `at_feet`: the line of the floor at the feet, `feet` the feet; `next`: the line of the route's next floor, `next_at` that
    /// waypoint; `kept`: the line steered on last ([`Self::kept`]). Along one top when both lines are of one segment, or when a
    /// floor has none (S3 gives a line only to the floor whose cell holds the line's point: a neighbour on the same top may have
    /// none) and both the feet and the waypoint are within the moving bar of the other's line, or of the kept one when neither
    /// floor has a line. Guarded while the feet are within the moving bar of the kept line.
    pub fn of(at_feet: Option<Line>, feet: Vec3, next: Option<Line>, next_at: Vec3, kept: Option<Line>) -> Self {
        let travel = (next_at.x - feet.x, next_at.z - feet.z);
        let steer = |line: Line| Precise::Steer { line, forward: line.forward(travel) };
        let on = |line: &Line, p: Vec3| line.place(p).across.abs() <= line.moving_bar();
        match (at_feet, next) {
            (Some(here), Some(there)) if here.segment == there.segment => steer(there),
            (Some(line), None) | (None, Some(line)) if on(&line, feet) && on(&line, next_at) => steer(line),
            (None, None) => match kept {
                Some(line) if on(&line, feet) && on(&line, next_at) => steer(line),
                Some(line) if on(&line, feet) => Precise::Guard,
                _ => Precise::Off,
            },
            _ => Precise::Guard,
        }
    }

    /// The line the next tick keeps ([`Self::of`]'s `kept`), after this mode with `kept` before it.
    pub fn kept(&self, kept: Option<Line>) -> Option<Line> {
        match *self {
            Precise::Steer { line, .. } => Some(line),
            Precise::Guard => kept,
            Precise::Off => None,
        }
    }

    /// The line steered on, if any.
    pub fn line(&self) -> Option<(Line, bool)> {
        match *self {
            Precise::Steer { line, forward } => Some((line, forward)),
            _ => None,
        }
    }

    /// No side step, strafe, sway, lane offset, apart step or sideways escape.
    pub fn holds_line(&self) -> bool {
        !matches!(self, Precise::Off)
    }
}

/// [S5] What the precise mode did since the last match end (decide nothing): one count a slot.
pub mod counts {
    use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

    #[derive(Clone, Copy, Debug)]
    pub enum Slot {
        /// A bot's walk tick steered on a top's line.
        Steer,
        /// A bot's walk tick on or onto a top, the route leaving it: guarded only.
        Guard,
        /// A side step or strafe held off.
        SideStep,
        /// A teammate's apart step held off.
        Apart,
        /// Escape keys taken off (those across the line when steering; every move key when guarded).
        Escape,
        /// The lip guard's sideways swap dropped.
        LipSwap,
        /// The lip guard's brake dropped off the rest bar.
        LipBrake,
        /// A stop off the rest bar walked on instead.
        WalkedOn,
        /// A walk tick steering on a line with the feet [`super::BELOW_TOP_M`] or more under the top: the body left the top
        /// (the line's failure, for the bench).
        BelowTop,
    }
    const NAMES: [&str; 9] = [
        "steer",
        "guard",
        "side_step_held",
        "apart_held",
        "escape_keys_cut",
        "lip_swap_dropped",
        "lip_brake_dropped",
        "walked_on_past_rest",
        "steering_below_top",
    ];
    static COUNTS: [AtomicU64; 9] = [const { AtomicU64::new(0) }; 9];

    pub fn add(slot: Slot) {
        COUNTS[slot as usize].fetch_add(1, Relaxed);
    }

    /// The match-end line's text after its prefix, and the counts zero again.
    pub fn line() -> String {
        let parts: Vec<String> = NAMES.iter().zip(&COUNTS).map(|(name, c)| format!("{name} {}", c.swap(0, Relaxed))).collect();
        format!("precise tops (S5) since the last match end: {}", parts.join(" "))
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
        // Going toward `from`.
        let t = line.target(Vec3::new(1.0, 1.3, 0.0), false, 0.8);
        assert!((t.x - 0.2).abs() < 1e-5, "{t:?}");
        // Past a piece's end the line runs on: the point is ahead of the feet, never back at the end.
        let near = |a: Vec3, b: Vec3| a.distance_xz(b) < 1e-5 && (a.y - b.y).abs() < 1e-5;
        assert!(near(line.target(Vec3::new(3.9, 1.6, 0.0), true, 0.8), Vec3::new(4.7, 1.67, 0.0)));
        assert!(near(line.target(Vec3::new(4.5, 1.6, 0.0), true, 0.8), Vec3::new(5.3, 1.73, 0.0)));
        assert!(near(line.target(Vec3::new(-1.0, 1.2, 0.0), false, 0.8), Vec3::new(-1.8, 1.02, 0.0)));
        assert!((line.height_at(Vec3::new(2.0, 0.0, 0.3)) - 1.4).abs() < 1e-5);
    }

    #[test]
    fn the_escape_keeps_only_the_keys_along_the_line_by_their_way_in_the_world() {
        use crate::v026g::drive::key;
        let line = rail(1, Face::BySlope); // along +x
        let moves = key::FORWARD | key::BACK | key::LEFT | key::RIGHT;
        // The view along +x: FORWARD / BACK step along the line, LEFT / RIGHT across.
        let yaw_x = super::super::kin::yaw_of([1.0, 0.0]);
        assert_eq!(line.keys_along(moves | key::JUMP, yaw_x), key::FORWARD | key::BACK | key::JUMP);
        // The view across it (+z): LEFT / RIGHT step along the line, FORWARD / BACK across (the case a key's name gets wrong).
        let yaw_z = super::super::kin::yaw_of([0.0, 1.0]);
        assert_eq!(line.keys_along(moves, yaw_z), key::LEFT | key::RIGHT);
        // The view at 30 degrees to the line: FORWARD is within 45 degrees of it, RIGHT (60 degrees) is not.
        let yaw_30 = super::super::kin::yaw_of([30f32.to_radians().cos(), 30f32.to_radians().sin()]);
        assert_eq!(line.keys_along(key::FORWARD | key::RIGHT, yaw_30), key::FORWARD);
    }

    #[test]
    fn the_mode_steers_only_along_one_top() {
        let here = rail(7, Face::BySlope);
        let along = rail(7, Face::BySlope);
        let other = rail(8, Face::BySlope);
        let (feet, ahead, behind) = (Vec3::new(1.0, 1.3, 0.05), Vec3::new(2.0, 1.4, 0.0), Vec3::new(0.0, 1.2, 0.0));
        assert_eq!(Precise::of(Some(here), feet, Some(along), ahead, None), Precise::Steer { line: along, forward: true });
        assert_eq!(Precise::of(Some(here), feet, Some(along), behind, None), Precise::Steer { line: along, forward: false });
        // One floor of the top with no line (S3 marks only the floor whose cell holds the line's point): still along it while the
        // feet and the waypoint are within the moving bar (0.43 m here).
        assert_eq!(Precise::of(None, feet, Some(along), ahead, None), Precise::Steer { line: along, forward: true });
        assert_eq!(Precise::of(Some(here), feet, None, ahead, None), Precise::Steer { line: here, forward: true });
        // Onto a top from beside it, off one (the waypoint is off the line: a drop, a jump), or onto another: guarded.
        let beside = Vec3::new(1.0, 0.2, 1.5);
        assert_eq!(Precise::of(None, beside, Some(along), ahead, None), Precise::Guard);
        assert_eq!(Precise::of(Some(here), feet, None, beside, None), Precise::Guard);
        assert_eq!(Precise::of(Some(here), feet, Some(other), ahead, None), Precise::Guard);
        assert_eq!(Precise::of(None, feet, None, ahead, None), Precise::Off);
        // Two floors with no line in a row: the kept line steers on while both are within its moving bar, guards while the feet
        // are, and lets go past it.
        let (far_feet, far_ahead) = (Vec3::new(5.0, 1.7, 0.1), Vec3::new(6.0, 1.8, 0.0));
        assert_eq!(Precise::of(None, far_feet, None, far_ahead, Some(here)), Precise::Steer { line: here, forward: true });
        assert_eq!(Precise::of(None, far_feet, None, beside, Some(here)), Precise::Guard);
        assert_eq!(Precise::of(None, beside, None, far_ahead, Some(here)), Precise::Off);
        // What the next tick keeps.
        assert_eq!(Precise::Steer { line: along, forward: true }.kept(None), Some(along));
        assert_eq!(Precise::Guard.kept(Some(here)), Some(here));
        assert_eq!(Precise::Off.kept(Some(here)), None);
        assert!(Precise::Guard.holds_line() && !Precise::Off.holds_line());
        assert_eq!(Precise::Guard.line(), None);
    }

    #[test]
    fn a_grid_line_top_is_read_as_its_line() {
        let top = LineTop { segment: 3, from: Vec3::new(0.0, 1.0, 0.0), to: Vec3::new(0.25, 1.0, 0.0), width: 0.08, face: Face::Walk };
        let line = Line::of(&top).unwrap();
        assert_eq!(line.segment, 3);
        assert!((line.rest_bar() - (0.04 + MOVING_M)).abs() < 1e-6);
        assert_eq!(Line::of(&LineTop { width: f32::NAN, ..top }), None);
    }
}
