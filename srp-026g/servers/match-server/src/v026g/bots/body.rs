//! Where a bot's body is now, though the server sees it only in the worlds
//! it is given: one every few seconds, of a tick a quarter of a second or
//! more behind the clock (`Game` asks two clients for one every
//! `refresh`, sooner while a bot needs one).
//!
//! The server knows every input the bot's body got, at its tick: it made
//! them. So from the last world that shows the body (a [`Fix`]) the body
//! is moved on tick by tick with those inputs by the movement model
//! ([`super::kin`]), kept on the walk grid where the map is known (a wall
//! stops it, a slope lifts it). A new world puts it back where the game
//! has it, and the inputs since are played again from there. How far the
//! model was off is kept ([`BodyTrack::last_error`]) for the logs and the
//! tuning.
//!
//! What the model does not know: other bodies in the way, being hit
//! (`stopping speed`), slides off steep ground, a jump's arc. Each is put
//! right by the next world.
//!
//! The pitch is the base pitch (`m_look_pitch`), which a world saves; the
//! view is that plus the weapon's kick ([`super::recoil`]: PROVEN,
//! 0x2E0F60). For a bot the kick is the bot's own prediction, kept with
//! each tick, and its turns as decided already carry the part of a
//! downward turn the kick took (0x2E0FB0), so the base pitch is the
//! world's. A human's kick is not known: 0.

use std::collections::VecDeque;

use mapdata::{LinkKind, NodeId, Vec3, WalkGrid};
use survarium::v026g::match_msgs::TICK_MS;

use super::kin::{Blend, Kinematics, TICK_S, to_world, wrap};

/// A world's word on the bot's body.
#[derive(Clone, Debug, PartialEq)]
pub struct Fix {
    /// The world's tick.
    pub time: u32,
    pub alive: bool,
    /// When this body was spawned: another value is another life.
    pub inserted: u32,
    /// The feet, map frame (alive only).
    pub feet: Option<Vec3>,
    /// The game's yaw: never wrapped.
    pub yaw: f32,
    pub pitch: f32,
    /// The keys the world has the body hold (`actions_mask`).
    pub keys: u32,
    /// `m_linear_horizontal_speed`, if it was read.
    pub speed: Option<f32>,
}

/// The body as the bot reckons it at a tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Estimate {
    pub time: u32,
    pub feet: Vec3,
    /// The ground velocity, map frame.
    pub velocity: [f32; 2],
    /// The velocity's blend, in the body's frame (`kin::Blend`).
    pub blend: Blend,
    /// In `(-π, π]`.
    pub yaw: f32,
    /// The base pitch (the world's).
    pub pitch: f32,
    /// The weapon's kick over it after the tick (a bot's own prediction).
    pub kick: f32,
    /// The keys held: they move the body from the next tick on (a world
    /// does not show its own tick's keys yet, `kin`).
    pub keys: u32,
    /// The grid floor it stands on, where the map is known.
    pub node: Option<NodeId>,
}

impl Estimate {
    /// The view's pitch: the base pitch and the kick (0x2E0F60).
    pub fn view_pitch(&self) -> f32 {
        (self.pitch + self.kick).clamp(-MAX_PITCH, MAX_PITCH)
    }
}

/// A decided tick: the keys held, the turn made to the base pitch, and the
/// kick after it.
#[derive(Clone, Copy, Debug)]
struct Tick {
    time: u32,
    keys: u32,
    turn: (f32, f32),
    kick: f32,
}

/// Ticks kept to be played again from a world: worlds come at most a few
/// seconds behind (a world older than this has no use).
const KEPT_MS: u32 = 20_000;
/// A floor this far from the feet is not the body's.
const LOCATE_M: f32 = 1.5;
/// A body is reckoned on this far past its last known tick at most, ms.
const PROJECT_MS: u32 = 3_000;
/// The pitch's limit, radians (LIKELY, `kin`).
const MAX_PITCH: f32 = 1.0;

#[derive(Clone, Debug, Default)]
pub struct BodyTrack {
    ticks: VecDeque<Tick>,
    /// The reckoned feet at each decided tick, to compare with a world.
    trail: VecDeque<(u32, Vec3)>,
    fix: Option<Fix>,
    estimate: Option<Estimate>,
    /// The distance between the reckoned feet and a world's, at the
    /// world's tick, for the last world that had both.
    last_error: Option<f32>,
    /// This life's speeds over the nominal ones (`Kinematics::level_of`).
    level: Option<f32>,
    /// The level its profile gives (`bot_skills::profile_movement_level`:
    /// skills, the weight carried), taken until a world shows this life's.
    prior_level: Option<f32>,
    /// The lives the worlds showed, the present one counted.
    lives: u32,
    /// Where the body was last reckoned alive, and which life that was:
    /// kept past its death, as a world shows a dead body without feet.
    last_alive: Option<(u32, Vec3)>,
}

impl BodyTrack {
    /// The body now (at the last decided tick), if it is alive and was seen.
    pub fn estimate(&self) -> Option<&Estimate> {
        self.estimate.as_ref()
    }

    pub fn fix(&self) -> Option<&Fix> {
        self.fix.as_ref()
    }

    pub fn last_error(&self) -> Option<f32> {
        self.last_error
    }

    /// The speed level its profile gives, until a world shows the life's
    /// own (see `prior_level`).
    pub fn set_prior_level(&mut self, level: Option<f32>) {
        self.prior_level = level.filter(|level| level.is_finite() && *level > 0.0);
    }

    /// The speed level the body is moved at: this life's from a world, else
    /// its profile's, else the nominal 1.
    pub fn level(&self) -> f32 {
        self.level.or(self.prior_level).unwrap_or(1.0)
    }

    /// The body reckoned on to `time` with the keys it holds (a human's,
    /// between two of its inputs); at most [`PROJECT_MS`] past the last
    /// tick known, as its inputs may have stopped coming.
    pub fn at(&self, time: u32, kin: &Kinematics, grid: Option<&WalkGrid>) -> Option<Estimate> {
        let estimate = *self.estimate.as_ref()?;
        let until = time.min(estimate.time + PROJECT_MS);
        let ticks = until.saturating_sub(estimate.time) / TICK_MS;
        if ticks == 0 {
            return Some(estimate);
        }
        let tick = Tick {
            time: estimate.time + ticks * TICK_MS,
            keys: estimate.keys,
            turn: (0.0, 0.0),
            kick: estimate.kick,
        };
        Some(advance(estimate, tick, kin, self.level(), grid))
    }

    /// Where the body is, or was last reckoned alive, and in which of its
    /// lives (counted from 1 as the worlds showed them): its feet now while
    /// it lives, else where its last life was reckoned when a world first
    /// showed it dead (that world has no feet for it: the kill feed's spot
    /// of a death the world already shows). None for a body never placed.
    pub fn last_alive(&self) -> Option<(u32, Vec3)> {
        match self.estimate.as_ref() {
            Some(estimate) => Some((self.lives, estimate.feet)),
            None => self.last_alive,
        }
    }

    /// Whether the last world showed the body dead.
    pub fn dead(&self) -> bool {
        self.fix.as_ref().is_some_and(|fix| !fix.alive)
    }

    /// The bot decided the tick at `time`: its keys, the turn as sent (the
    /// pitch's with the part the kick took) and the kick after it.
    pub fn decided(
        &mut self,
        time: u32,
        keys: u32,
        turn: (f32, f32),
        kick: f32,
        kin: &Kinematics,
        grid: Option<&WalkGrid>,
    ) {
        let tick = Tick {
            time,
            keys,
            turn,
            kick,
        };
        self.ticks.push_back(tick);
        while self
            .ticks
            .front()
            .is_some_and(|tick| tick.time + KEPT_MS < time)
        {
            self.ticks.pop_front();
        }
        let level = self.level();
        if let Some(estimate) = self.estimate.as_mut() {
            *estimate = advance(*estimate, tick, kin, level, grid);
            self.trail.push_back((time, estimate.feet));
            while self
                .trail
                .front()
                .is_some_and(|(at, _)| at + KEPT_MS < time)
            {
                self.trail.pop_front();
            }
        }
    }

    /// A world showed the body. An older world than the last is not used.
    /// Returns whether it starts a new life (or the first one).
    pub fn fixed(&mut self, fix: Fix, kin: &Kinematics, grid: Option<&WalkGrid>) -> bool {
        if self.fix.as_ref().is_some_and(|last| last.time >= fix.time) {
            return false;
        }
        let new_life = fix.alive
            && self
                .fix
                .as_ref()
                .is_none_or(|last| !last.alive || last.inserted != fix.inserted);
        let feet = match (fix.alive, fix.feet) {
            (true, Some(feet)) => feet,
            _ => {
                // Where it fell, as near as is known: the reckoning at the
                // world's tick (it was dead by then), else the last world's
                // feet, else the reckoning now.
                if let Some(estimate) = self.estimate.take() {
                    let feet = self
                        .trail
                        .iter()
                        .rev()
                        .find(|(at, _)| *at <= fix.time)
                        .map(|(_, feet)| *feet)
                        .or_else(|| self.fix.as_ref().and_then(|last| last.feet))
                        .unwrap_or(estimate.feet);
                    self.last_alive = Some((self.lives, feet));
                }
                self.trail.clear();
                self.level = None;
                self.fix = Some(fix);
                return false;
            }
        };
        if new_life {
            self.level = None;
            self.lives += 1;
        }
        if let Some(level) = fix.speed.and_then(|speed| kin.level_of(fix.keys, speed)) {
            self.level = Some(level);
        }
        let level = self.level();
        self.last_error = (!new_life)
            .then(|| {
                self.trail
                    .iter()
                    .find(|(at, _)| *at == fix.time)
                    .map(|(_, reckoned)| reckoned.distance_xz(feet))
            })
            .flatten();
        let node = grid.and_then(|grid| grid.locate(feet, LOCATE_M));
        // The world has its own tick's keys (PROVEN, `kin`); its velocity
        // is its speed along the way the keys before were taking it.
        let held = fix.keys;
        let before = self
            .ticks
            .iter()
            .rev()
            .find(|tick| tick.time < fix.time)
            .map_or(fix.keys, |tick| tick.keys);
        let mut local = kin.target_local(before, level);
        if let Some(speed) = fix.speed.filter(|speed| speed.is_finite()) {
            let now = local[0].hypot(local[1]);
            local = if now > 1e-3 {
                [local[0] / now * speed, local[1] / now * speed]
            } else {
                [speed, 0.0]
            };
        }
        let mut estimate = Estimate {
            time: fix.time,
            feet,
            velocity: to_world(local, fix.yaw),
            blend: Blend::at(local),
            yaw: wrap(fix.yaw),
            pitch: fix.pitch,
            // The world has no kick; the bot's own for that tick.
            kick: self
                .ticks
                .iter()
                .find(|tick| tick.time == fix.time)
                .map_or(0.0, |tick| tick.kick),
            keys: held,
            node,
        };
        self.trail.clear();
        // The ticks after the world's, again from where the world has it.
        for tick in self.ticks.iter().filter(|tick| tick.time > fix.time) {
            estimate = advance(estimate, *tick, kin, level, grid);
            self.trail.push_back((tick.time, estimate.feet));
        }
        // A world newer than every decided tick: the body is as it shows.
        self.estimate = Some(estimate);
        self.fix = Some(fix);
        new_life
    }
}

/// The body at the next decided tick: moved by the keys it held (a tick
/// missed between two decided ones, a stall, held the same keys), then the
/// tick's turn and keys taken.
fn advance(
    mut at: Estimate,
    tick: Tick,
    kin: &Kinematics,
    level: f32,
    grid: Option<&WalkGrid>,
) -> Estimate {
    let steps = (tick.time.saturating_sub(at.time) / TICK_MS).max(1);
    let target = kin.target_local(at.keys, level);
    for _ in 0..steps {
        let local = at.blend.tick(target, kin.blend_ticks);
        at.velocity = to_world(local, at.yaw);
        let to = Vec3::new(
            at.feet.x + at.velocity[0] * TICK_S,
            at.feet.y,
            at.feet.z + at.velocity[1] * TICK_S,
        );
        match (grid, at.node) {
            (Some(grid), Some(node)) => {
                let (node, feet, stopped) = slide(grid, node, at.feet, to);
                at.node = Some(node);
                at.feet = feet;
                if stopped {
                    at.velocity = [0.0, 0.0];
                }
            }
            _ => at.feet = to,
        }
    }
    at.yaw = wrap(at.yaw + tick.turn.0);
    at.pitch = (at.pitch + tick.turn.1).clamp(-MAX_PITCH, MAX_PITCH);
    at.kick = tick.kick;
    at.keys = tick.keys;
    at.time = tick.time;
    at
}

/// A move from `from` (on `node`) toward `to` over the grid, in steps of
/// at most half a cell: each step goes to a floor linked to the one it is
/// on, or along one axis only (a body slides along a wall), or not at all.
/// Returns the floor, the feet, and whether the move was stopped before
/// it got anywhere.
pub fn slide(grid: &WalkGrid, mut node: NodeId, from: Vec3, to: Vec3) -> (NodeId, Vec3, bool) {
    let half = grid.cell_size() * 0.5;
    let (dx, dz) = (to.x - from.x, to.z - from.z);
    let steps = ((dx.abs().max(dz.abs()) / half).ceil() as usize).max(1);
    let (sx, sz) = (dx / steps as f32, dz / steps as f32);
    let mut at = from;
    for _ in 0..steps {
        let tries = [(sx, sz), (sx, 0.0), (0.0, sz)];
        let moved = tries.iter().find_map(|&(mx, mz)| {
            if mx == 0.0 && mz == 0.0 {
                return None;
            }
            let point = Vec3::new(at.x + mx, at.y, at.z + mz);
            floor_under(grid, node, point).map(|next| (next, point))
        });
        let Some((next, point)) = moved else {
            break;
        };
        node = next;
        at = Vec3::new(point.x, grid.position(next).y, point.z);
    }
    (node, at, at == from)
}

/// The floor whose cell holds `point`: `node` itself or one it walks or
/// drops to.
fn floor_under(grid: &WalkGrid, node: NodeId, point: Vec3) -> Option<NodeId> {
    let half = grid.cell_size() * 0.5 + 1e-4;
    let holds = |n: NodeId| {
        let p = grid.position(n);
        (p.x - point.x).abs() <= half && (p.z - point.z).abs() <= half
    };
    if holds(node) {
        return Some(node);
    }
    grid.links(node)
        .filter(|(_, kind)| *kind != LinkKind::Jump)
        .map(|(next, _)| next)
        .find(|&next| holds(next))
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::v026g::drive::key;
    use mapdata::{Body, Triangle, WalkerBehaviour};

    /// A flat floor from (x0, z0) to (x1, z1) at height y (as the mapdata
    /// crate's tests make one).
    pub fn floor(x0: f32, z0: f32, x1: f32, z1: f32, y: f32) -> [Triangle; 2] {
        let walker = WalkerBehaviour::BySlope;
        let (a, b, c, d) = (
            Vec3::new(x0, y, z0),
            Vec3::new(x1, y, z0),
            Vec3::new(x1, y, z1),
            Vec3::new(x0, y, z1),
        );
        [
            Triangle {
                v: [a, b, c],
                walker,
            },
            Triangle {
                v: [a, c, d],
                walker,
            },
        ]
    }

    /// A wall across x at `x` from z0 to z1, 3 m high.
    pub fn wall_x(x: f32, z0: f32, z1: f32) -> [Triangle; 2] {
        let walker = WalkerBehaviour::BySlope;
        let (a, b, c, d) = (
            Vec3::new(x, 0.0, z0),
            Vec3::new(x, 0.0, z1),
            Vec3::new(x, 3.0, z1),
            Vec3::new(x, 3.0, z0),
        );
        [
            Triangle {
                v: [a, b, c],
                walker,
            },
            Triangle {
                v: [a, c, d],
                walker,
            },
        ]
    }

    /// A 40 x 40 m floor with a wall along x = 20 from z = 0 to z = 30:
    /// the way round is past z = 30.
    pub fn walled() -> WalkGrid {
        let mut triangles: Vec<Triangle> = floor(0.0, 0.0, 40.0, 40.0, 0.0).to_vec();
        triangles.extend(wall_x(20.0, 0.0, 30.0));
        WalkGrid::build(&triangles, [0.0, 0.0, 40.0, 40.0], 0.25, Body::default())
    }

    fn fix(time: u32, feet: Vec3, yaw: f32) -> Fix {
        Fix {
            time,
            alive: true,
            inserted: 100,
            feet: Some(feet),
            yaw,
            pitch: 0.0,
            keys: 0,
            speed: None,
        }
    }

    #[test]
    fn the_body_is_moved_on_by_its_inputs_and_put_back_by_a_world() {
        // Speeds reached at once, to count the metres.
        let kin = Kinematics {
            blend_ticks: 1,
            ..Kinematics::default()
        };
        let mut track = BodyTrack::default();
        assert!(track.estimate().is_none());
        // Inputs before any world are kept for when one comes.
        for time in (1_050..=2_000).step_by(50) {
            track.decided(time, key::FORWARD, (0.0, 0.0), 0.0, &kin, None);
        }
        assert!(track.fixed(fix(1_000, Vec3::new(5.0, 0.0, 5.0), 0.0), &kin, None));
        let at = *track.estimate().unwrap();
        assert_eq!(at.time, 2_000);
        // Forward at yaw 0 is +z; the keys of a tick move the body from the
        // next one: 19 ticks of walking.
        assert!(
            (at.feet.z - (5.0 + kin.walk[0] * 0.95)).abs() < 1e-3,
            "{at:?}"
        );
        assert!((at.feet.x - 5.0).abs() < 1e-3);
        // A world of a later tick: the body is put where it shows, the
        // ticks after it played again, and how far off the reckoning was is
        // kept.
        track.decided(2_050, key::FORWARD, (0.0, 0.0), 0.0, &kin, None);
        // The world has its tick's keys: forward.
        let walking = Fix {
            keys: key::FORWARD,
            ..fix(1_500, Vec3::new(5.0, 0.0, 6.0), 0.0)
        };
        assert!(!track.fixed(walking, &kin, None));
        let error = track.last_error().unwrap();
        assert!(
            (error - (kin.walk[0] * 0.45 - 1.0).abs()).abs() < 1e-3,
            "{error}"
        );
        let at = *track.estimate().unwrap();
        assert!(
            (at.feet.z - (6.0 + kin.walk[0] * 0.55)).abs() < 1e-3,
            "{at:?}"
        );
        // An older world is not used.
        assert!(!track.fixed(fix(1_400, Vec3::new(0.0, 0.0, 0.0), 0.0), &kin, None));
        // A turn is applied at its tick: a quarter turn, then forward goes
        // to -x.
        track.decided(
            2_100,
            0,
            (std::f32::consts::FRAC_PI_2, 0.0),
            0.0,
            &kin,
            None,
        );
        let before = track.estimate().unwrap().feet;
        track.decided(2_150, key::FORWARD, (0.0, 0.0), 0.0, &kin, None);
        assert_eq!(
            track.estimate().unwrap().feet,
            before,
            "the keys move it a tick later"
        );
        track.decided(2_200, key::FORWARD, (0.0, 0.0), 0.0, &kin, None);
        let after = track.estimate().unwrap().feet;
        assert!(after.x < before.x - 0.1 && (after.z - before.z).abs() < 1e-3);
        // Dead: nothing reckoned; alive again with a new spawn time is a
        // new life.
        track.fixed(
            Fix {
                alive: false,
                feet: None,
                ..fix(2_000, Vec3::ZERO, 0.0)
            },
            &kin,
            None,
        );
        assert!(track.dead() && track.estimate().is_none());
        assert!(track.fixed(
            Fix {
                inserted: 3_000,
                ..fix(3_200, Vec3::new(1.0, 0.0, 1.0), 0.0)
            },
            &kin,
            None
        ));
    }

    /// A body moves at its profile's speed level (its skills, its weight)
    /// until a world shows the life's own; a new life starts from the
    /// profile's again.
    #[test]
    fn a_body_moves_at_its_profiles_level_until_a_world_shows_its_own() {
        // Speeds reached at once, to count the metres.
        let kin = Kinematics {
            blend_ticks: 1,
            ..Kinematics::default()
        };
        let walk = |track: &mut BodyTrack, from: u32, to: u32| {
            for time in (from..=to).step_by(50) {
                track.decided(time, key::FORWARD, (0.0, 0.0), 0.0, &kin, None);
            }
        };
        let mut track = BodyTrack::default();
        assert_eq!(track.level(), 1.0);
        track.set_prior_level(Some(0.9));
        assert_eq!(track.level(), 0.9);
        track.fixed(fix(0, Vec3::ZERO, 0.0), &kin, None);
        walk(&mut track, 50, 1_000);
        // 19 ticks of walking at 0.9 of the walk.
        let z = track.estimate().unwrap().feet.z;
        assert!((z - kin.walk[0] * 0.9 * 0.95).abs() < 1e-3, "{z}");
        // A world shows it walking at 1.1: that is the life's level.
        let walking = Fix {
            keys: key::FORWARD,
            speed: Some(kin.walk[0] * 1.1),
            ..fix(1_000, Vec3::ZERO, 0.0)
        };
        track.fixed(walking, &kin, None);
        assert!((track.level() - 1.1).abs() < 1e-5);
        // A new life: the profile's again.
        track.fixed(
            Fix {
                inserted: 5_000,
                ..fix(5_000, Vec3::ZERO, 0.0)
            },
            &kin,
            None,
        );
        assert_eq!(track.level(), 0.9);
        // Nonsense is not taken.
        track.set_prior_level(Some(f32::NAN));
        assert_eq!(track.level(), 1.0);
    }

    #[test]
    fn a_wall_stops_the_reckoned_body_and_it_slides_along() {
        let grid = walled();
        // Speeds reached at once, to count the metres.
        let kin = Kinematics {
            blend_ticks: 1,
            ..Kinematics::default()
        };
        let mut track = BodyTrack::default();
        // At x = 17, facing +x (yaw -π/2), walking into the wall for 2 s.
        let yaw = -std::f32::consts::FRAC_PI_2;
        assert!((crate::v026g::bots::kin::forward(yaw)[0] - 1.0).abs() < 1e-5);
        track.fixed(fix(0, Vec3::new(17.0, 0.0, 10.0), yaw), &kin, Some(&grid));
        assert!(track.estimate().unwrap().node.is_some());
        for time in (50..=2_000).step_by(50) {
            track.decided(time, key::FORWARD, (0.0, 0.0), 0.0, &kin, Some(&grid));
        }
        let at = track.estimate().unwrap();
        // The body's radius keeps it short of the wall.
        assert!(at.feet.x < 19.8 && at.feet.x > 19.0, "{at:?}");
        assert!((at.feet.z - 10.0).abs() < 0.2);
        // Diagonally into it: it slides along the wall in z.
        let yaw = -std::f32::consts::FRAC_PI_4;
        let mut track = BodyTrack::default();
        track.fixed(fix(0, Vec3::new(18.5, 0.0, 10.0), yaw), &kin, Some(&grid));
        for time in (50..=2_000).step_by(50) {
            track.decided(time, key::FORWARD, (0.0, 0.0), 0.0, &kin, Some(&grid));
        }
        let at = track.estimate().unwrap();
        assert!(at.feet.x < 19.8 && at.feet.z > 13.0, "{at:?}");
    }

    #[test]
    fn the_view_is_the_base_pitch_and_the_kick_of_the_last_tick() {
        let kin = Kinematics::default();
        let mut track = BodyTrack::default();
        track.fixed(fix(0, Vec3::ZERO, 0.0), &kin, None);
        // Up 0.02 with no kick; then the kick at 0.03 and a pull of 0.01
        // all taken off it (the base turn 0).
        track.decided(50, 0, (0.0, 0.02), 0.0, &kin, None);
        track.decided(100, key::FIRE, (0.0, 0.0), 0.03, &kin, None);
        track.decided(150, key::FIRE, (0.0, 0.0), 0.02, &kin, None);
        let at = track.estimate().unwrap();
        assert!((at.pitch - 0.02).abs() < 1e-6 && (at.view_pitch() - 0.04).abs() < 1e-6);
        // A world of tick 100: its base pitch, the bot's kick of that tick,
        // the ticks after played again.
        let world = Fix {
            pitch: 0.025,
            ..fix(100, Vec3::ZERO, 0.0)
        };
        track.fixed(world, &kin, None);
        let at = track.estimate().unwrap();
        assert!((at.view_pitch() - 0.045).abs() < 1e-6, "{at:?}");
        // The view is clamped as the client clamps it.
        track.decided(200, 0, (0.0, 0.97), 0.1, &kin, None);
        assert_eq!(track.estimate().unwrap().view_pitch(), 1.0);
    }
}
