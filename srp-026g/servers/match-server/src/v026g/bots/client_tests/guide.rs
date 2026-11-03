//! The way to a step's start spot, in the game chat. Nothing in the game shows a marker, so while a step waits for the owner at a
//! spot the runner tells him how far it is and which way, from his feet and his view: `marker: 23 m ahead-left, 2 m above you`,
//! and `you're at the marker` once he is within its reach.
//!
//! The direction is relative to where he looks (the yaw of his body in the [`Frame`], the bots' own map frame: [`kin::forward`] is
//! ahead, [`kin::to_world`]'s left is left), in eight words of 45 degrees each. The height is said when the spot's floor is more than
//! [`RISE_M`] above or below his feet.

use mapdata::Vec3;

use super::frame::{Body, Frame};
use super::step::Spot;
use crate::v026g::bots::kin;

/// A difference of height under this is not said, m (a step of the stairs is 0.8 m at most: the client's `startup.cfg`).
pub const RISE_M: f32 = 1.5;

/// Where the owner is to go for a step's start.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mark {
    /// A spot of the map, reached within `r_m` (flat). `landmark` is the way to it in plain words (the card and the first line).
    Spot { at: Spot, r_m: f32, landmark: &'static str },
    /// A spot whose floor height the step does not know: only the flat distance and the direction are said.
    Flat { x: f32, z: f32, r_m: f32, landmark: &'static str },
    /// The nearest exit of team 1's base ([`Frame::exits`]), reached within `r_m`; none to say until the map is built.
    EnemyExit { r_m: f32 },
}

impl Mark {
    /// What the chat calls it.
    pub fn name(self) -> &'static str {
        match self {
            Mark::Spot { .. } | Mark::Flat { .. } => "marker",
            Mark::EnemyExit { .. } => "enemy exit",
        }
    }

    pub fn landmark(self) -> Option<&'static str> {
        match self {
            Mark::Spot { landmark, .. } | Mark::Flat { landmark, .. } => Some(landmark).filter(|l| !l.is_empty()),
            Mark::EnemyExit { .. } => Some("where team 1's base opens toward your side; the chat counts down the nearest one"),
        }
    }

    /// The card's words for it.
    pub fn describe(self) -> String {
        match self {
            Mark::Spot { at, r_m, .. } => format!("({:.1}, {:.1}, {:.1}) within {r_m} m", at.0[0], at.0[1], at.0[2]),
            Mark::Flat { x, z, r_m, .. } => format!("({x:.1}, -, {z:.1}) within {r_m} m"),
            Mark::EnemyExit { r_m } => format!("the nearest exit of team 1's base within {r_m} m"),
        }
    }

    /// The point to go to for a body at `feet` (its height none when the mark has none), and the reach.
    fn target(self, frame: &Frame, feet: Vec3) -> Option<(Vec3, Option<f32>, f32)> {
        match self {
            Mark::Spot { at, r_m, .. } => Some((at.vec(), Some(at.0[1]), r_m)),
            Mark::Flat { x, z, r_m, .. } => Some((Vec3::new(x, feet.y, z), None, r_m)),
            Mark::EnemyExit { r_m } => frame.exits[1]
                .iter()
                .copied()
                .min_by(|a, b| a.distance_xz(feet).total_cmp(&b.distance_xz(feet)))
                .map(|exit| (exit, Some(exit.y), r_m)),
        }
    }
}

/// Which way a point lies from where a body looks: eight sectors of 45 degrees.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Way {
    Ahead,
    AheadLeft,
    Left,
    BehindLeft,
    Behind,
    BehindRight,
    Right,
    AheadRight,
}

impl Way {
    /// The way to `to` from `from` for a body looking at `yaw` (map frame); ahead when the two are one point.
    pub fn of(from: Vec3, yaw: f32, to: Vec3) -> Self {
        let [dx, dz] = [to.x - from.x, to.z - from.z];
        let [fx, fz] = kin::forward(yaw);
        // The left of the view is the left of `kin::to_world`: a quarter turn to the positive side.
        let [lx, lz] = [-fz, fx];
        let ahead = dx * fx + dz * fz;
        let left = dx * lx + dz * lz;
        let degrees = left.atan2(ahead).to_degrees();
        const WAYS: [Way; 8] =
            [Way::Ahead, Way::AheadLeft, Way::Left, Way::BehindLeft, Way::Behind, Way::BehindRight, Way::Right, Way::AheadRight];
        let sector = ((degrees + 22.5).rem_euclid(360.0) / 45.0) as usize;
        WAYS[sector.min(7)]
    }

    pub fn word(self) -> &'static str {
        match self {
            Way::Ahead => "ahead",
            Way::AheadLeft => "ahead-left",
            Way::Left => "to your left",
            Way::BehindLeft => "behind-left",
            Way::Behind => "behind you",
            Way::BehindRight => "behind-right",
            Way::Right => "to your right",
            Way::AheadRight => "ahead-right",
        }
    }
}

/// Whether the owner is within the mark's reach: a line that says so is told less often than one that guides.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reach {
    There,
    Away,
}

/// The guidance line for the owner's body: none while the mark has no point yet (an exit before the map is built).
pub fn line(mark: Mark, frame: &Frame, owner: &Body) -> Option<(Reach, String)> {
    let (to, floor, r_m) = mark.target(frame, owner.feet)?;
    let flat = owner.feet.distance_xz(to);
    let rise = floor.map(|y| y - owner.feet.y).filter(|r| r.abs() > RISE_M);
    let height = rise.map(|r| format!("{} m {} you", metres(r.abs()), if r > 0.0 { "above" } else { "below" }));
    let name = mark.name();
    Some(if flat <= r_m {
        match height {
            None => (Reach::There, format!("you're at the {name}")),
            Some(height) => (Reach::Away, format!("{name}: right here, {height}")),
        }
    } else {
        let way = Way::of(owner.feet, owner.yaw, to).word();
        let mut text = format!("{name}: {} m {way}", metres(flat));
        if let Some(height) = height {
            text.push_str(", ");
            text.push_str(&height);
        }
        (Reach::Away, text)
    })
}

/// Whole metres, at least 1.
fn metres(m: f32) -> u32 {
    m.round().max(1.0) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owner_at(feet: Vec3, yaw: f32) -> Body {
        Body {
            place: 0,
            team: 0,
            alive: true,
            feet,
            health: 1.0,
            kills: 0,
            deaths: 0,
            crouched: false,
            keys: 0,
            yaw,
            pitch: 0.0,
            speed: 0.0,
        }
    }

    fn frame() -> Frame {
        Frame {
            clock: super::super::time::At(0),
            playing: true,
            owner: None,
            bots: Vec::new(),
            batteries: Vec::new(),
            heard: Vec::new(),
            sight: None,
            exits: [Vec::new(), Vec::new()],
            counters: Default::default(),
        }
    }

    /// The point `local` (ahead, left) metres from `from` for a body at `yaw`, by the motor's own frame.
    fn local(from: Vec3, yaw: f32, ahead: f32, left: f32) -> Vec3 {
        let [x, z] = kin::to_world([ahead, left], yaw);
        Vec3::new(from.x + x, from.y, from.z + z)
    }

    #[test]
    fn the_eight_ways_follow_the_view_at_every_yaw() {
        let from = Vec3::new(10.0, 2.0, -16.0);
        let cases = [
            (10.0, 0.0, Way::Ahead),
            (10.0, 10.0, Way::AheadLeft),
            (0.0, 10.0, Way::Left),
            (-10.0, 10.0, Way::BehindLeft),
            (-10.0, 0.0, Way::Behind),
            (-10.0, -10.0, Way::BehindRight),
            (0.0, -10.0, Way::Right),
            (10.0, -10.0, Way::AheadRight),
            // 20 degrees off ahead is still ahead; 25 is ahead-left.
            (10.0, 3.6, Way::Ahead),
            (10.0, 4.7, Way::AheadLeft),
        ];
        for yaw in [0.0_f32, 0.7, 1.6, -2.3, 3.1] {
            for (ahead, left, want) in cases {
                assert_eq!(Way::of(from, yaw, local(from, yaw, ahead, left)), want, "yaw {yaw}, ahead {ahead}, left {left}");
            }
        }
    }

    #[test]
    fn ahead_is_where_the_motor_walks_forward() {
        // A body at yaw 0 walks toward +z (`kin::forward`); its left is -x.
        let from = Vec3::ZERO;
        assert_eq!(Way::of(from, 0.0, Vec3::new(0.0, 0.0, 5.0)), Way::Ahead);
        assert_eq!(Way::of(from, 0.0, Vec3::new(-5.0, 0.0, 0.0)), Way::Left);
        assert_eq!(Way::of(from, 0.0, Vec3::new(5.0, 0.0, 0.0)), Way::Right);
        assert_eq!(Way::of(from, 0.0, Vec3::new(0.0, 0.0, -5.0)), Way::Behind);
    }

    #[test]
    fn the_line_says_distance_way_and_height() {
        let spot = Spot::at(0.0, 5.0, 23.0);
        let mark = Mark::Spot { at: spot, r_m: 3.0, landmark: "" };
        let (reach, text) = line(mark, &frame(), &owner_at(Vec3::new(0.0, 2.6, 0.0), 0.0)).unwrap();
        assert_eq!((reach, text.as_str()), (Reach::Away, "marker: 23 m ahead, 2 m above you"));
        let (_, text) = line(mark, &frame(), &owner_at(Vec3::new(0.0, 9.0, 0.0), std::f32::consts::PI)).unwrap();
        assert_eq!(text, "marker: 23 m behind you, 4 m below you");
        // Within a step of the floor the height is not said.
        let (_, text) = line(mark, &frame(), &owner_at(Vec3::new(10.0, 4.0, 23.0), 0.0)).unwrap();
        assert_eq!(text, "marker: 10 m to your left");
    }

    #[test]
    fn at_the_spot_it_says_so_unless_on_another_floor() {
        let mark = Mark::Spot { at: Spot::at(1.0, 13.8, 1.0), r_m: 1.0, landmark: "" };
        assert_eq!(line(mark, &frame(), &owner_at(Vec3::new(1.5, 13.8, 1.0), 2.0)), Some((Reach::There, "you're at the marker".to_string())));
        assert_eq!(
            line(mark, &frame(), &owner_at(Vec3::new(1.5, 17.0, 1.0), 2.0)),
            Some((Reach::Away, "marker: right here, 3 m below you".to_string()))
        );
    }

    #[test]
    fn a_flat_mark_never_speaks_of_height() {
        let mark = Mark::Flat { x: 0.0, z: -8.0, r_m: 5.0, landmark: "" };
        assert_eq!(line(mark, &frame(), &owner_at(Vec3::new(0.0, 60.0, 0.0), 0.0)).unwrap().1, "marker: 8 m behind you");
        assert_eq!(line(mark, &frame(), &owner_at(Vec3::new(0.0, 60.0, -4.0), 0.0)).unwrap().0, Reach::There);
    }

    #[test]
    fn the_enemy_exit_is_the_nearest_once_the_map_is_built() {
        let mark = Mark::EnemyExit { r_m: 5.0 };
        let owner = owner_at(Vec3::ZERO, 0.0);
        assert_eq!(line(mark, &frame(), &owner), None);
        let mut built = frame();
        built.exits[1] = vec![Vec3::new(0.0, 0.0, 40.0), Vec3::new(-12.0, 0.0, 0.0)];
        assert_eq!(line(mark, &built, &owner).unwrap().1, "enemy exit: 12 m to your left");
    }
}
