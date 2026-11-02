//! How a player's body moves for the keys it holds: the bot's model of
//! the game's movement, used to keep track of every body between two
//! worlds ([`super::body`]) and to plan a bot's steps ([`super::motor`]).
//!
//! Read in the 0.26g client's code and checked tick by tick on a recorded
//! match (the private `KINEMATICS-CODE.md` of the bot research; summed up
//! in `docs/notes/player_bots.md` §4):
//! - **Movement is the animation's** (`set_physics_controller_walk_vector`
//!   0x2D9060): each tick the body moves at the movement class's desired
//!   speed × the body's factor `f = 1 + Σ movement_speed_modifier`
//!   (0x2D5AE0; weapons, mods, a hurt leg, the battery). PROVEN.
//! - **Desired speeds**, m/s (forward / side / back): walk 3.5 / 2.5 /
//!   3.0, aiming 3.0 / 2.0 / 2.5, crouched 2.5 / 2.0 / 2.0, crouched and
//!   aiming 2.0 / 1.5 / 1.5; sprint 5.0 (the config's 5.5 is the sprint
//!   animation's own speed). Forward diagonals go at the forward speed,
//!   back diagonals at the back speed; F+B and L+R cancel. PROVEN.
//! - **Sprint** needs SPRINT and FORWARD with none of BACK, FIRE, CROUCH
//!   (0x2F1220); aiming blocks it (LIKELY); strafing does not. PROVEN.
//! - **A change of speed** is a 0.3 s animation blend (0x921804): linear
//!   over 6 ticks from the present velocity to the new one, held in the
//!   body's frame, so a turn turns it at once. Blend PROVEN, the linear
//!   shape LIKELY (the best fit on the recording).
//! - **Ways:** forward at yaw `a` is `(-sin a, cos a)` in the map frame; a
//!   positive yaw turns left; LEFT goes to yaw + 90°. PROVEN.
//! - **Order:** a world of tick T has T's turn and keys; its position is
//!   from before T's move, which uses T's keys and the yaw after T's turn
//!   (`body.rs` keeps that order). PROVEN. The yaw is never wrapped by the
//!   game; the pitch is clamped at ±1. PROVEN.
//!
//! On the recording this model, with the body's level read from a world,
//! lands a median 0.33 m (p90 1.55 m) from the next world ~5 s on, walls
//! not counted. Every world puts the body back where the game has it.

use std::f32::consts::{PI, TAU};

use serde::{Deserialize, Serialize};

use super::pitch::PitchScale;
use crate::v026g::drive::key;

/// The game's ticks, seconds.
pub const TICK_S: f32 = 0.05;

/// The desired speeds of a movement class: forward, side, back (m/s).
pub type Speeds = [f32; 3];

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Kinematics {
    pub walk: Speeds,
    pub aim: Speeds,
    pub crouch: Speeds,
    pub crouch_aim: Speeds,
    pub sprint: f32,
    /// Ticks of the blend to a new velocity.
    pub blend_ticks: u32,
    /// Which jump links the reckoned body (`body.rs`) takes up after a jump press ([`JumpSnap`]).
    pub jump_snap: JumpSnap,
    /// Where the reckoned body falls off a ledge ([`Edge`]).
    pub edge: Edge,
    /// Where the reckoned body comes down off a ledge whose drop is an off-mesh walk-off ([`WalkOffLanding`]).
    pub walk_off_landing: WalkOffLanding,
    /// Where the stop forecast's margin falls off a ledge ([`MarginEdge`]).
    pub margin_edge: MarginEdge,
    /// Which floor a world's feet are put on ([`FixFloor`]).
    pub fix_floor: FixFloor,
    /// How a view's pitch unit relates to its elevation ([`PitchScale`]): the unit is 60.0 degrees up and 67.48 down
    /// (`pitch.rs`), not a radian (`radian`: the bots as they were, which aimed 3.5 degrees low at a target 15 to 30 degrees below the eye
    /// and shot 16 % there against 29 % with the map, `player_bots_weapons.md`). The world's pitch, the pitch turns the bots send and the geometry from the
    /// view's pitch (the eye, the throws, the rays) all follow it, so it is one setting for the whole match, not a team's.
    pub pitch_scale: PitchScale,
}

/// Which jump links the reckoned body takes up after a jump press, until a world shows where the body is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JumpSnap {
    /// Any jump link: the body is on its landing 0.8 s after the press whatever the rise, so a jump the clip
    /// cannot make is believed made until the next world.
    Any,
    /// Only a link whose rise the jump pressed could make: the plain jump's 1.09 m, or with forward and sprint held at
    /// the walk cap or more the sprint jump's 1.28 m (the client's choice by the keys and the speed at the press, PROVEN,
    /// the sprint-jump RE 1.1).
    #[default]
    ByKeys,
    /// None: the body stays on the take-off side until a world shows it on the landing floor (a landing counts only
    /// with walkable ground under the body near the target).
    Never,
}

/// Where the reckoned body (and the lip guard's forecast) falls off a ledge.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Edge {
    /// At the grid's cell boundary, 0.125 m past the last floor cell's centre (the lattice's picture, the bots' reckoning
    /// before the client edge).
    Lattice,
    /// As the client's capsule: held until [`super::body::SUPPORT_M`] past the centre of the floor it stands on, so a
    /// body that stays up at a ledge is not reckoned fallen (`body::slide_supported`, LIKELY, the live samples at lips).
    #[default]
    Client,
}

/// [POCKETS (b)] Where the reckoned body ([`Edge::Client`]) comes down when it goes off a ledge whose drop is an off-mesh walk-off
/// (`GapDrop`, `gap::Takeoff::WalkOff`): a flight kept only where bodies at every one of `gap::WALK_OFF_SPEEDS` (3.0-5.1 m/s, PROVEN
/// J0 g2a) come down on its landing, which lies cells out from the edge.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WalkOffLanding {
    /// On the link's landing at any speed. A body that slips off slowly falls almost straight down, onto another floor: Vector
    /// Laboratory's 20.6 m lip, whose walk-off lands 2.5 m out on the 17.80 floor, put two fighting bots there at 0.03-0.12 m/s
    /// while their bodies fell onto the 16.22 ledge under the edge; believing they stood 2 m away, they walked off the ledge into
    /// the shut pit (`fx-smoke-pockets.md` §1, from their FALL lines in the bench's stand-in game: the walker mesh has a 0.97 m
    /// parapet along that edge and a plate over the ledge, so the client's body does neither, A247).
    #[default]
    Link,
    /// On the link's landing only at the slowest of `gap::WALK_OFF_SPEEDS` or more; slower, on the highest floor under the body
    /// (within its radius and half a cell, lower than a step) open above the edge (A247: no slab, plate or overhang between
    /// them, `WalkGrid::open_to`), else held at the edge until a world shows where it is.
    BySpeed,
}

/// Where the stop forecast's margin (`body::stays_up`: the same stop from [`super::body::RECKONING_M`] further on along the body's
/// way, for a body not exactly where it is reckoned) falls off a ledge.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MarginEdge {
    /// At the grid's cell boundary ([`Edge::Lattice`]) whatever [`Kinematics::edge`] is: with [`Edge::Client`] the forecast moves
    /// the body up to [`super::body::SUPPORT_M`] past its floor's centre, past the cell's boundary, and the margin from there falls at
    /// once, whichever way the body goes. A body walking along a lip on its last floor is held by the full lip guard's sides
    /// (`Feel::lip_guard_full`: the same walk [`super::body::RECKONING_M`] to the side) every tick, and by its own margin once it is
    /// reckoned past the boundary: of the 3,705 ticks the guard held a walk in the bench's stuck episodes at lips, 2,031 (55 %) were
    /// held for a side only, the rest for the walk itself; with `edge = lattice` and no sides, 680, all for the walk (A205, 16 runs).
    #[default]
    Lattice,
    /// As the body itself ([`Kinematics::edge`]): the margin is the forecast's body a little further on, so it falls where that
    /// body would.
    Body,
}

/// Which floor of the walk grid the reckoning puts a world's feet on (`body::BodyTrack::fixed`), the floor it moves the body on from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FixFloor {
    /// The nearest floor from 1 m over the feet to 2.5 m under them (`WalkGrid::locate`): a world that shows the body in the air
    /// in front of a wall puts it on the floor over the wall's top when that is nearer than its own. School, the r1 stuck spot
    /// (A196, live): a jump against the facade, shown 0.68 m up, put the reckoned body on the top behind it, 0.59 m over the feet;
    /// with [`Edge::Client`] it walked on there for a second (2-3 m off) while the body fell back.
    Nearest,
    /// The nearest floor at most [`FIX_ABOVE_M`] over the feet, else the nearest: feet are on their floor or over it.
    Under,
    /// [`Self::Under`] while the body's own jump is in the air (its jump key held at a decided tick within
    /// [`super::body::JUMP_AIR_MS`] before the world's), else [`Self::Nearest`]. A world shows a body between floors in a jump
    /// (School r1: rising at the facade, its own floor under it) and in a fall (Tarakanovsky Fort (-54, 8, 12): walked off a
    /// ledge, 0.5 m under the top it left and over a lower top 0.6-0.8 m aside that it falls past); only the jump's floor is
    /// under it. Over every differing fix of the rev 38+ live recordings (14,097, A204), the mean error of the reckoning at the
    /// next world is 0.638 m with this rule, 1.380 with [`Self::Nearest`] and 0.944 with [`Self::Under`], and no map is worse
    /// than with [`Self::Nearest`] (Under alone was worse on four: Tarakanovsky Fort 1.644 against 0.691). The default: its by-side
    /// bench could not reach the case (47 such fixes in 256 matches: the stand-in's bodies stay on the grid's floors) and showed no
    /// cost (score +0.3 %, z +0.6, no guard flagged; #944).
    #[default]
    UnderInJump,
}

/// How far over a world's feet [`FixFloor::Under`] takes a floor, metres: a floor's height is its cell centre's, which on the
/// steepest walkable slope (55 degrees, `impassable_slope` 0x7DE10, PROVEN) lies up to 0.25 m over feet at the cell's edge (half
/// the diagonal of a 0.25 m cell, 0.18 m, times tan 55); a little over that.
pub const FIX_ABOVE_M: f32 = 0.3;

impl Default for Kinematics {
    fn default() -> Self {
        Self {
            walk: [3.5, 2.5, 3.0],
            aim: [3.0, 2.0, 2.5],
            crouch: [2.5, 2.0, 2.0],
            crouch_aim: [2.0, 1.5, 1.5],
            sprint: 5.0,
            blend_ticks: 6,
            jump_snap: JumpSnap::ByKeys,
            edge: Edge::Client,
            walk_off_landing: WalkOffLanding::Link,
            margin_edge: MarginEdge::Lattice,
            fix_floor: FixFloor::UnderInJump,
            pitch_scale: PitchScale::Client,
        }
    }
}

/// The ground direction a body faces at `yaw` (map frame, x and z).
pub fn forward(yaw: f32) -> [f32; 2] {
    let (s, c) = yaw.sin_cos();
    [-s, c]
}

/// The yaw that faces the ground direction `(x, z)`; the inverse of
/// [`forward`], in `(-π, π]`.
pub fn yaw_of(direction: [f32; 2]) -> f32 {
    (-direction[0]).atan2(direction[1])
}

/// `angle` brought into `(-π, π]`.
pub fn wrap(angle: f32) -> f32 {
    let a = (angle + PI).rem_euclid(TAU) - PI;
    if a <= -PI { a + TAU } else { a }
}

/// A velocity in the body's frame: (ahead, to the left), m/s.
pub type Local = [f32; 2];

/// The world velocity of `local` for a body at `yaw`.
pub fn to_world(local: Local, yaw: f32) -> [f32; 2] {
    let [fx, fz] = forward(yaw);
    // Left of facing: a quarter turn to the positive side.
    let [lx, lz] = [-fz, fx];
    [local[0] * fx + local[1] * lx, local[0] * fz + local[1] * lz]
}

/// The blend to a new velocity, in the body's frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Blend {
    pub from: Local,
    pub to: Local,
    /// Ticks into the blend.
    pub step: u32,
}

impl Blend {
    pub fn now(&self, ticks: u32) -> Local {
        let t = if ticks == 0 {
            1.0
        } else {
            (self.step as f32 / ticks as f32).min(1.0)
        };
        [
            self.from[0] + (self.to[0] - self.from[0]) * t,
            self.from[1] + (self.to[1] - self.from[1]) * t,
        ]
    }

    /// One tick on toward `target`: a new target starts a new blend from
    /// where it is.
    pub fn tick(&mut self, target: Local, ticks: u32) -> Local {
        if target != self.to {
            *self = Self {
                from: self.now(ticks),
                to: target,
                step: 0,
            };
        }
        self.step = self.step.saturating_add(1).min(ticks.max(1));
        self.now(ticks)
    }

    /// Already at `local`, no blend going.
    pub fn at(local: Local) -> Self {
        Self {
            from: local,
            to: local,
            step: u32::MAX,
        }
    }
}

impl Kinematics {
    /// Whether the keys sprint (SPRINT and FORWARD, none of BACK, FIRE,
    /// CROUCH, AIM).
    pub fn sprints(keys: u32) -> bool {
        keys & key::SPRINT != 0
            && keys & key::FORWARD != 0
            && keys & (key::BACK | key::FIRE | key::CROUCH | key::AIM) == 0
    }

    /// The velocity the keys ask for, in the body's frame, for a body at
    /// `level` × the desired speeds.
    pub fn target_local(&self, keys: u32, level: f32) -> Local {
        let held = |bit| f32::from(u8::from(keys & bit != 0));
        let ahead = held(key::FORWARD) - held(key::BACK);
        let left = held(key::LEFT) - held(key::RIGHT);
        if ahead == 0.0 && left == 0.0 {
            return [0.0, 0.0];
        }
        let class = match (keys & key::CROUCH != 0, keys & key::AIM != 0) {
            (false, false) => self.walk,
            (false, true) => self.aim,
            (true, false) => self.crouch,
            (true, true) => self.crouch_aim,
        };
        let speed = if Self::sprints(keys) {
            self.sprint
        } else if ahead > 0.0 {
            class[0]
        } else if ahead < 0.0 {
            class[2]
        } else {
            class[1]
        };
        let n = ahead.hypot(left);
        [ahead / n * speed * level, left / n * speed * level]
    }

    /// The world velocity the keys ask for at `yaw`.
    pub fn target_velocity(&self, keys: u32, yaw: f32, level: f32) -> [f32; 2] {
        to_world(self.target_local(keys, level), yaw)
    }

    /// The body's speed level from a world: its speed over the desired one
    /// of its keys, if it moved steadily under keys that have one.
    pub fn level_of(&self, keys: u32, speed: f32) -> Option<f32> {
        let [a, l] = self.target_local(keys, 1.0);
        let nominal = a.hypot(l);
        if nominal < 0.5 {
            return None;
        }
        let level = speed / nominal;
        (0.85..=1.15).contains(&level).then_some(level)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yaw_and_direction_are_inverse_and_angles_wrap() {
        for yaw in [-3.0_f32, -1.0, 0.0, 0.5, 2.0, 3.1] {
            assert!((yaw_of(forward(yaw)) - yaw).abs() < 1e-5, "{yaw}");
        }
        assert!((wrap(3.0 * PI) - PI).abs() < 1e-5);
        assert!((wrap(-PI) - PI).abs() < 1e-5);
        assert!((wrap(0.25 - TAU) - 0.25).abs() < 1e-5);
        let (a, b) = (forward(0.4), forward(0.4 + 8.0 * PI));
        assert!((a[0] - b[0]).abs() < 1e-4 && (a[1] - b[1]).abs() < 1e-4);
    }

    #[test]
    fn keys_give_the_desired_speeds_of_the_code() {
        let k = Kinematics::default();
        let speed = |keys| {
            let [a, l] = k.target_local(keys, 1.0);
            a.hypot(l)
        };
        let close = |a: f32, b: f32| (a - b).abs() < 1e-4;
        assert!(close(speed(key::FORWARD), 3.5));
        assert!(close(speed(key::LEFT), 2.5));
        assert!(close(speed(key::BACK), 3.0));
        assert!(
            close(speed(key::FORWARD | key::LEFT), 3.5),
            "forward diagonals"
        );
        assert!(close(speed(key::BACK | key::RIGHT), 3.0), "back diagonals");
        assert!(close(speed(key::FORWARD | key::SPRINT), 5.0));
        assert!(
            close(speed(key::FORWARD | key::RIGHT | key::SPRINT), 5.0),
            "a diagonal sprints"
        );
        assert!(
            close(speed(key::FORWARD | key::SPRINT | key::FIRE), 3.5),
            "no sprint firing"
        );
        assert!(close(speed(key::FORWARD | key::CROUCH | key::SPRINT), 2.5));
        assert!(close(speed(key::FORWARD | key::AIM), 3.0));
        assert!(close(speed(key::BACK | key::CROUCH | key::AIM), 1.5));
        assert_eq!(speed(key::FORWARD | key::BACK), 0.0);
        assert_eq!(speed(key::LEFT | key::RIGHT), 0.0);
        // At yaw 0 forward is +z, LEFT is -x (a positive yaw turns left).
        let at = |keys| k.target_velocity(keys, 0.0, 1.0);
        assert!(close(at(key::FORWARD)[1], 3.5));
        assert!(close(at(key::LEFT)[0], -2.5));
        assert!(close(at(key::RIGHT)[0], 2.5));
        assert!(close(k.target_velocity(key::FORWARD, 0.0, 0.95)[1], 3.325));
    }

    #[test]
    fn a_new_velocity_is_blended_in_over_six_ticks() {
        let k = Kinematics::default();
        let mut blend = Blend::default();
        let target = k.target_local(key::FORWARD, 1.0);
        let speeds: Vec<f32> = (0..8)
            .map(|_| blend.tick(target, k.blend_ticks)[0])
            .collect();
        let close = |a: f32, b: f32| (a - b).abs() < 1e-4;
        assert!(
            close(speeds[0], 3.5 / 6.0) && close(speeds[5], 3.5) && close(speeds[7], 3.5),
            "{speeds:?}"
        );
        // Two ticks into a stop, a sprint: blended from where it was.
        blend.tick([0.0, 0.0], 6);
        blend.tick([0.0, 0.0], 6);
        let mid = blend.now(6)[0];
        assert!(close(mid, 3.5 * 4.0 / 6.0));
        let next = blend.tick([5.0, 0.0], 6)[0];
        assert!(close(next, mid + (5.0 - mid) / 6.0));
        assert!((k.level_of(key::FORWARD, 3.675).unwrap() - 1.05).abs() < 1e-4);
        assert_eq!(k.level_of(key::FORWARD, 1.0), None);
    }
}
