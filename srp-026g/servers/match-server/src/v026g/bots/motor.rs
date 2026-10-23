//! How a bot moves its body: the keys and the view's turn for each tick,
//! made to look like a player's hands.
//!
//! - **The view turns smoothly.** A player's mouse does not jump to a new
//!   heading: the view speeds up, turns and slows into it. The turn of a
//!   tick comes from a turning speed that eases toward what the remaining
//!   angle asks ([`Feel::turn_time_s`]), within a top speed and a top
//!   change of speed a tick, and never past the heading aimed at.
//! - **It walks while it turns**, as players do, and lets go of forward
//!   only for a turn of more than [`Feel::stop_turn`] (picking it up again
//!   once facing near the way).
//! - **It steers at a point ahead** on the route ([`super::path`]), so it
//!   rounds a corner in a curve; never a point it could not walk to in a
//!   line, or it would cut the corner into the wall (the look ahead is
//!   shortened till the line is clear).
//! - **It sprints on long straight stretches** only: after facing the way
//!   for a moment, and not into a sharp corner.
//! - **Its heading wanders a little** (a slow sway of a degree or two) and
//!   its view settles near level, so no two bots walk the same line.
//! - **Standing**, it looks about: a new look every second or two, eased
//!   into as any turn.
//! - **It hops when caught on rising ground**, as players jump over lips and
//!   ledges on the move: when a world puts the body well behind where its
//!   keys took the reckoning while the ground there climbs a little
//!   ([`Feel::hop_behind_m`]); at most once in a few seconds.
//!
//! The numbers are [`Feel`]'s: tuning, not game data.

use std::collections::VecDeque;
use std::f32::consts::PI;

use mapdata::Vec3;

use super::encode::Decision;
use super::kin::{TICK_S, wrap, yaw_of};
use super::path::Route;
use super::rng::Rng;
use crate::v026g::drive::key;

/// The look ahead is not shortened below this, metres.
const MIN_AHEAD_M: f32 = 0.5;

/// The motor's tuning.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Feel {
    /// The time to take most of a turn, seconds: the turning speed asked is
    /// the remaining angle over this.
    pub turn_time_s: f32,
    /// Top turning speed, radians a second.
    pub max_turn_speed: f32,
    /// Top change of the turning speed, radians a second per second.
    pub max_turn_accel: f32,
    /// Forward is let go for a turn larger than this, radians …
    pub stop_turn: f32,
    /// … and held again once the turn left is under this.
    pub go_turn: f32,
    /// The point steered at is this far ahead, metres, plus
    /// `look_ahead_per_speed` × speed.
    pub look_ahead_m: f32,
    pub look_ahead_per_speed: f32,
    /// Sprint only with this much straight route ahead, metres …
    pub sprint_from_m: f32,
    /// … and stop sprinting with this little left before a corner.
    pub sprint_until_m: f32,
    /// Facing the way within this for `sprint_after_s` before a sprint.
    pub sprint_facing: f32,
    pub sprint_after_s: f32,
    /// A corner sharper than this ends a straight stretch, radians.
    pub corner: f32,
    /// The heading's sway: amplitude (radians) and period (seconds).
    pub sway: f32,
    pub sway_period_s: f32,
    /// The route's end is reached this near, metres.
    pub arrive_m: f32,
    /// Standing, a new look every this many seconds (low, high) …
    pub look_every_s: (f32, f32),
    /// … turning by up to this, radians.
    pub look_turn: f32,
    /// The hop ([`Motor::hop_key`]), a jump while walking a route when
    /// the body was caught: a world put the body at least this far behind
    /// the reckoning along the way, metres …
    ///
    /// Players jump 0.98 times a minute (0.53-1.43), 94 % of them while
    /// moving, over lips and to free themselves (`player_bots_stuck.md`
    /// §6, the recordings). The hop's numbers are tuning, not measured:
    /// 1 m behind is §6's suggestion (a walk of 0.3 s; the stuck watch
    /// steps in at 1.5 s, `stuck.rs`), the rest a choice.
    pub hop_behind_m: f32,
    /// … and at least this share of the way the reckoning went since the
    /// world before: a body caught for a fifth of a world's period is, one
    /// a little slower than reckoned all along (up a slope, a few per cent)
    /// is not. Low, as the hop has to come at the first world that shows
    /// the catch: at the next the stuck watch takes over …
    pub hop_behind_share: f32,
    /// … while the ground climbs by this much (low, high), metres: the
    /// floors the reckoning walked about the body, or the route ahead;
    /// from a rough floor's unevenness to a step and a half (the grid
    /// walks up 0.6 m a cell) …
    pub hop_rise_m: (f32, f32),
    /// … within this, metres along the way.
    pub hop_look_m: f32,
    /// No hop while the heading is still this far from the way (a turn,
    /// a sprint round a corner: the reckoning is off there anyway),
    /// radians.
    pub hop_max_turn: f32,
    /// At most one hop in this, seconds: a few seconds, never spammy
    /// (a player's once a minute is mostly over things the bot's route
    /// jumps at its jump links) …
    pub hop_every_s: f32,
    /// … and none this soon after a jump of the route.
    pub hop_after_jump_s: f32,
}

impl Default for Feel {
    fn default() -> Self {
        Self {
            turn_time_s: 0.15,
            max_turn_speed: 5.0,
            max_turn_accel: 45.0,
            stop_turn: 1.4,
            go_turn: 0.7,
            look_ahead_m: 1.6,
            look_ahead_per_speed: 0.35,
            sprint_from_m: 10.0,
            sprint_until_m: 3.5,
            sprint_facing: 0.17,
            sprint_after_s: 0.3,
            corner: 0.6,
            sway: 0.025,
            sway_period_s: 3.7,
            arrive_m: 0.6,
            look_every_s: (0.8, 2.2),
            look_turn: 1.1,
            hop_behind_m: 1.0,
            hop_behind_share: 0.2,
            hop_rise_m: (0.25, 0.9),
            hop_look_m: 1.5,
            hop_max_turn: 0.35,
            hop_every_s: 4.0,
            hop_after_jump_s: 2.0,
        }
    }
}

/// Where the body is, as the motor needs it.
#[derive(Clone, Copy, Debug)]
pub struct Pose {
    pub feet: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    /// Ground speed, m/s.
    pub speed: f32,
}

/// What the motor did with a route this tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Walked {
    OnTheWay,
    Arrived,
    /// Farther off the route than the caller allows.
    Off(f32),
}

/// A jump not yet up its ledge is tried again after this long, seconds.
pub const JUMP_AGAIN_S: f32 = 1.0;
/// A low leg this near ahead, metres, is walked crouched already.
pub const LOW_AHEAD_M: f32 = 1.0;
/// The hop's trail keeps this many walking ticks (4 s: more than a
/// world's period while a bot walks) …
const HOP_TRAIL_TICKS: usize = 80;
/// … and is dropped after this many ticks of not walking a route (1 s).
const HOP_GAP_TICKS: u32 = 20;
/// A move of a tick longer than this, metres, is a world's correction,
/// not the keys' (a sprint moves 0.26 m a tick).
const HOP_STEP_M: f32 = 0.5;
/// The trail's floors this far to a side of the way, metres, are the
/// ground about the body.
const HOP_SIDE_M: f32 = 1.0;

/// One bot's hands.
#[derive(Clone, Debug)]
pub struct Motor {
    pub feel: Feel,
    /// The view's turning speed now (yaw, pitch), radians a second.
    rate: (f32, f32),
    walking: bool,
    sprinting: bool,
    /// Ticks faced the way well enough to sprint.
    facing_ticks: u32,
    /// The waypoint last jumped to and the ticks since
    /// ([`Motor::jump_key`]).
    jumped_at: Option<(usize, u32)>,
    /// The sway's phase and the time, seconds, for it.
    phase: f32,
    t: f32,
    /// Standing: the heading looked at, and ticks till the next look.
    look: Option<(f32, u32)>,
    /// The turn's time constant for this tick when not the feel's (the
    /// reflex aims faster or slower by skill).
    pub turn_time: Option<f32>,
    /// The ticks decided so far (each decision is a tick).
    clock: u32,
    /// The hop's memory ([`Motor::hop_key`]): the feet at the last walking
    /// ticks, with their tick, the newest last (the floors the reckoning
    /// walked: `body` keeps it on the walk grid) …
    trail: VecDeque<(u32, Vec3)>,
    /// … the way the keys took the body at the newest (unit, map x/z) …
    way: [f32; 2],
    /// … and the tick of the last hop.
    hopped: Option<u32>,
}

impl Motor {
    pub fn new(feel: Feel, rng: &mut Rng) -> Self {
        Self {
            feel,
            rate: (0.0, 0.0),
            walking: false,
            sprinting: false,
            facing_ticks: 0,
            jumped_at: None,
            phase: rng.range(0.0, 2.0 * PI),
            t: 0.0,
            look: None,
            turn_time: None,
            clock: 0,
            trail: VecDeque::new(),
            way: [0.0, 0.0],
            hopped: None,
        }
    }

    /// The tick's turn toward `yaw` and `pitch` from the pose: eased, within
    /// the top speed and change of speed, never past the aim.
    fn turn_toward(&mut self, pose: &Pose, yaw: f32, pitch: f32) -> (f32, f32) {
        let feel = &self.feel;
        let turn_time = self.turn_time.unwrap_or(feel.turn_time_s).max(0.02);
        let ease = |rate: &mut f32, left: f32| {
            let want = (left / turn_time).clamp(-feel.max_turn_speed, feel.max_turn_speed);
            let step = feel.max_turn_accel * TICK_S;
            *rate += (want - *rate).clamp(-step, step);
            let mut turn = *rate * TICK_S;
            // Not past the aim: a player slows into it.
            if turn * left <= 0.0 && left.abs() > 1e-4 {
                turn = 0.0;
                *rate = 0.0;
            } else if turn.abs() > left.abs() {
                turn = left;
                *rate = left / TICK_S;
            }
            turn
        };
        let yaw_left = wrap(yaw - pose.yaw);
        let pitch_left = pitch - pose.pitch;
        let (mut yaw_rate, mut pitch_rate) = self.rate;
        let turn = (
            ease(&mut yaw_rate, yaw_left),
            ease(&mut pitch_rate, pitch_left),
        );
        self.rate = (yaw_rate, pitch_rate);
        turn
    }

    fn sway(&mut self) -> f32 {
        self.t += TICK_S;
        self.feel.sway * (self.phase + self.t * 2.0 * PI / self.feel.sway_period_s).sin()
    }

    /// The decision of one tick that walks the route from `pose`. `off_m`:
    /// how far off the route the caller lets the body be before it plans
    /// again; `clear(point)`: whether the body walks to `point` in a line.
    pub fn walk(
        &mut self,
        route: &mut Route,
        pose: &Pose,
        off_m: f32,
        clear: impl Fn(Vec3) -> bool,
    ) -> (Decision, Walked) {
        self.clock = self.clock.wrapping_add(1);
        self.look = None;
        let (s, off) = route.progress(pose.feet);
        let left = route.length() - s;
        if left <= self.feel.arrive_m || pose.feet.distance_xz(route.end()) <= self.feel.arrive_m {
            self.walking = false;
            self.sprinting = false;
            let turn = self.turn_toward(pose, pose.yaw, 0.0);
            return (Decision { keys: 0, turn }, Walked::Arrived);
        }
        if off > off_m {
            return (Decision::default(), Walked::Off(off));
        }
        let mut ahead = self.feel.look_ahead_m + self.feel.look_ahead_per_speed * pose.speed;
        let mut target = route.point_at(s + ahead);
        while ahead > MIN_AHEAD_M && !clear(target) {
            ahead *= 0.6;
            target = route.point_at(s + ahead);
        }
        let (dx, dz) = (target.x - pose.feet.x, target.z - pose.feet.z);
        let sway = self.sway();
        let heading = if dx.abs() + dz.abs() < 1e-3 {
            pose.yaw
        } else {
            yaw_of([dx, dz]) + sway
        };
        let turn = self.turn_toward(pose, heading, 0.0);
        let still = wrap(heading - (pose.yaw + turn.0)).abs();

        // Walk while turning; let go only for a big turn.
        if self.walking && still > self.feel.stop_turn {
            self.walking = false;
        } else if !self.walking && still < self.feel.go_turn {
            self.walking = true;
        }
        // Sprint on a long straight stretch, facing the way.
        let (straight, _) = route.straight_ahead(s, self.feel.corner);
        if self.walking && still < self.feel.sprint_facing {
            self.facing_ticks += 1;
        } else {
            self.facing_ticks = 0;
        }
        let settled = self.facing_ticks as f32 * TICK_S >= self.feel.sprint_after_s;
        if !self.sprinting && settled && straight >= self.feel.sprint_from_m {
            self.sprinting = true;
        } else if self.sprinting
            && (!self.walking
                || straight < self.feel.sprint_until_m
                || still > 2.0 * self.feel.sprint_facing)
        {
            self.sprinting = false;
        }
        // Crouched where a standing body does not fit (a low passage):
        // no sprint there.
        let low = route.low_ahead(s, LOW_AHEAD_M);
        if low {
            self.sprinting = false;
        }
        let mut keys = 0;
        if self.walking {
            keys |= key::FORWARD;
            if self.sprinting {
                keys |= key::SPRINT;
            }
            if low {
                keys |= key::CROUCH;
            }
        }
        keys |= self.jump_key(route, s);
        let may = self.walking && still <= self.feel.hop_max_turn;
        keys |= self.hop_key(route, s, pose.feet, unit(dx, dz), may);
        (Decision { keys, turn }, Walked::OnTheWay)
    }

    /// The decision of one tick that walks the route while the view looks
    /// at `aim` (yaw, pitch): the legs pick the key of the eight ways
    /// nearest the way to go, as a player strafes while aiming; no sprint.
    pub fn walk_aiming(
        &mut self,
        route: &mut Route,
        pose: &Pose,
        off_m: f32,
        clear: impl Fn(Vec3) -> bool,
        aim: (f32, f32),
    ) -> (Decision, Walked) {
        self.clock = self.clock.wrapping_add(1);
        self.look = None;
        self.sprinting = false;
        self.facing_ticks = 0;
        let (s, off) = route.progress(pose.feet);
        let turn = self.turn_toward(pose, aim.0, aim.1);
        if route.length() - s <= self.feel.arrive_m
            || pose.feet.distance_xz(route.end()) <= self.feel.arrive_m
        {
            self.walking = false;
            return (Decision { keys: 0, turn }, Walked::Arrived);
        }
        if off > off_m {
            return (Decision { keys: 0, turn }, Walked::Off(off));
        }
        let mut ahead = self.feel.look_ahead_m;
        let mut target = route.point_at(s + ahead);
        while ahead > MIN_AHEAD_M && !clear(target) {
            ahead *= 0.6;
            target = route.point_at(s + ahead);
        }
        let (dx, dz) = (target.x - pose.feet.x, target.z - pose.feet.z);
        self.walking = true;
        let mut keys = keys_toward(wrap(yaw_of([dx, dz]) - (pose.yaw + turn.0)));
        if route.low_ahead(s, LOW_AHEAD_M) {
            keys |= key::CROUCH;
        }
        // Up a ledge while aiming too (the walk aiming had no jump: a body
        // walked into the ledge's wall for as long as it aimed).
        keys |= self.jump_key(route, s);
        keys |= self.hop_key(route, s, pose.feet, unit(dx, dz), true);
        (Decision { keys, turn }, Walked::OnTheWay)
    }

    /// The jump of this tick up the route's next ledge: just before it,
    /// walking into it; again every [`JUMP_AGAIN_S`] while the body is still
    /// short of it (a jump that did not get it up: it hit the edge, or it
    /// came too early).
    fn jump_key(&mut self, route: &Route, s: f32) -> u32 {
        if let Some((_, ticks)) = self.jumped_at.as_mut() {
            *ticks = ticks.saturating_add(1);
        }
        let Some((k, before)) = route.next_jump(s) else {
            return 0;
        };
        let again = match self.jumped_at {
            Some((at, ticks)) => at != k || ticks as f32 * TICK_S >= JUMP_AGAIN_S,
            None => true,
        };
        if self.walking && before < 0.8 && again {
            self.jumped_at = Some((k, 0));
            key::JUMP
        } else {
            0
        }
    }

    /// The hop of this tick: a jump when a world has just put the body
    /// well behind where its keys took the reckoning ([`Self::stalled`])
    /// while the ground about it climbs a little ([`Self::rises`]): a lip,
    /// a step the grid walks up and the body caught on. Players jump over
    /// such things on the move; a bot's route jumps only at its jump
    /// links, and the stuck watch (`stuck.rs`) steps in only once the body
    /// has not moved for 1.5 s.
    ///
    /// The signal is the reckoned feet going back along the way from one
    /// tick to the next: the keys never move them back, only a world does
    /// (`body`: the world's feet and the inputs since played again), and by
    /// as much as the body was short of the reckoning (`last_error`, along
    /// the way). It is read from the poses the motor is given, and the
    /// ground from the floors the reckoning walked (the route's corners are
    /// far apart: a lip between two is not in its heights).
    ///
    /// `may`: walking the way, not turning hard. None by a low floor (where
    /// it walks crouched), nor by a jump of the route or soon after one,
    /// nor within [`Feel::hop_every_s`] of the last. Both looked for from
    /// [`Feel::hop_look_m`] behind: a world may put the body back behind a
    /// corner the route has it past ([`Route::progress`] never goes back).
    /// Keeps the trail.
    fn hop_key(
        &mut self,
        route: &Route,
        s: f32,
        feet: Vec3,
        way: Option<[f32; 2]>,
        may: bool,
    ) -> u32 {
        let feel = &self.feel;
        let fresh = self
            .trail
            .back()
            .is_some_and(|&(at, _)| self.clock.wrapping_sub(at) <= HOP_GAP_TICKS);
        if !fresh {
            self.trail.clear();
        }
        let cooled = self
            .hopped
            .is_none_or(|at| self.clock.wrapping_sub(at) as f32 * TICK_S >= feel.hop_every_s);
        let jumped = self
            .jumped_at
            .is_some_and(|(_, ticks)| (ticks as f32 * TICK_S) < feel.hop_after_jump_s);
        let from = (s - feel.hop_look_m).max(0.0);
        let low = route.low_ahead(from, feel.hop_look_m + LOW_AHEAD_M);
        let jump_near = route
            .next_jump(from)
            .is_some_and(|(_, before)| before <= 2.0 * feel.hop_look_m);
        let hop = may
            && cooled
            && !jumped
            && !low
            && !jump_near
            && self.stalled(feet)
            && self.rises(route, s, feet);
        self.trail.push_back((self.clock, feet));
        while self.trail.len() > HOP_TRAIL_TICKS {
            self.trail.pop_front();
        }
        if let Some(way) = way {
            self.way = way;
        }
        if hop {
            self.hopped = Some(self.clock);
            key::JUMP
        } else {
            0
        }
    }

    /// Whether the feet now are well behind the last tick's along the way
    /// the keys took them: by [`Feel::hop_behind_m`], and by
    /// [`Feel::hop_behind_share`] of the way the reckoning went since the
    /// correction before (a tick's move longer than a step is one).
    fn stalled(&self, feet: Vec3) -> bool {
        let Some(&(_, before)) = self.trail.back() else {
            return false;
        };
        let behind = (before.x - feet.x) * self.way[0] + (before.z - feet.z) * self.way[1];
        if behind < self.feel.hop_behind_m {
            return false;
        }
        let mut meant = 0.0;
        let mut later = before;
        for &(_, at) in self.trail.iter().rev().skip(1) {
            let step = at.distance_xz(later);
            if step > HOP_STEP_M {
                break;
            }
            meant += step;
            later = at;
        }
        behind >= self.feel.hop_behind_share * meant
    }

    /// Whether the ground about the feet climbs along the way by
    /// [`Feel::hop_rise_m`] within [`Feel::hop_look_m`]: the floors the
    /// reckoning walked near the feet (behind and ahead: the inputs played
    /// again after a world may have taken the reckoning up the lip
    /// already), or the route's own heights ahead.
    fn rises(&self, route: &Route, s: f32, feet: Vec3) -> bool {
        let (low, high) = self.feel.hop_rise_m;
        let look = self.feel.hop_look_m;
        let within = |rise: f32| rise >= low && rise <= high;
        let base = route.point_at(s).y;
        let route_rise = (1..=3)
            .map(|k| route.point_at(s + look * k as f32 / 3.0).y - base)
            .fold(f32::NEG_INFINITY, f32::max);
        if within(route_rise) {
            return true;
        }
        let [wx, wz] = self.way;
        let mut ground: Vec<(f32, f32)> = self
            .trail
            .iter()
            .map(|&(_, at)| at)
            .chain([feet])
            .filter_map(|at| {
                let (dx, dz) = (at.x - feet.x, at.z - feet.z);
                let along = dx * wx + dz * wz;
                let side = (dx * wz - dz * wx).abs();
                (along.abs() <= look && side <= HOP_SIDE_M).then_some((along, at.y))
            })
            .collect();
        ground.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut rise = f32::NEG_INFINITY;
        for (k, &(along, y)) in ground.iter().enumerate() {
            for &(earlier, below) in &ground[..k] {
                if along - earlier <= look {
                    rise = rise.max(y - below);
                }
            }
        }
        within(rise)
    }

    /// The decision of one tick standing and looking at `aim`.
    pub fn aim_still(&mut self, pose: &Pose, aim: (f32, f32)) -> Decision {
        self.clock = self.clock.wrapping_add(1);
        self.walking = false;
        self.sprinting = false;
        self.facing_ticks = 0;
        self.look = None;
        let turn = self.turn_toward(pose, aim.0, aim.1);
        Decision { keys: 0, turn }
    }

    /// The decision of one tick that steps straight to `to`, a metre or
    /// two off, no route (a take's stand round a battery, or a step back
    /// out of a nook the walk grid has no line from): the legs pick the key
    /// of the eight ways nearest the way to go while the view turns to
    /// `aim`, else keeps its yaw; no sprint.
    pub fn step_to(&mut self, pose: &Pose, to: Vec3, aim: Option<(f32, f32)>) -> Decision {
        self.clock = self.clock.wrapping_add(1);
        // Moved by keys off any route: the hop's trail is not the way.
        self.trail.clear();
        self.sprinting = false;
        self.facing_ticks = 0;
        self.look = None;
        let turn = match aim {
            Some((yaw, pitch)) => self.turn_toward(pose, yaw, pitch),
            None => self.turn_toward(pose, pose.yaw, pose.pitch),
        };
        let (dx, dz) = (to.x - pose.feet.x, to.z - pose.feet.z);
        if dx.abs() + dz.abs() < 1e-3 {
            self.walking = false;
            return Decision { keys: 0, turn };
        }
        self.walking = true;
        let keys = keys_toward(wrap(yaw_of([dx, dz]) - (pose.yaw + turn.0)));
        Decision { keys, turn }
    }

    /// The decision of one tick standing still and looking about.
    pub fn stand(&mut self, pose: &Pose, rng: &mut Rng) -> Decision {
        self.clock = self.clock.wrapping_add(1);
        self.walking = false;
        self.sprinting = false;
        self.facing_ticks = 0;
        let (low, high) = self.feel.look_every_s;
        let (aim, ticks) = match self.look {
            Some((aim, ticks)) if ticks > 0 => (aim, ticks - 1),
            _ => (
                wrap(pose.yaw + rng.range(-self.feel.look_turn, self.feel.look_turn)),
                (rng.range(low, high) / TICK_S) as u32,
            ),
        };
        self.look = Some((aim, ticks));
        let turn = self.turn_toward(pose, aim, 0.0);
        Decision { keys: 0, turn }
    }

    /// Hands off: no keys, no turn (dead, or waiting for a world).
    pub fn rest(&mut self) -> Decision {
        self.clock = self.clock.wrapping_add(1);
        self.trail.clear();
        self.rate = (0.0, 0.0);
        self.walking = false;
        self.sprinting = false;
        self.facing_ticks = 0;
        self.look = None;
        Decision::default()
    }
}

/// The unit vector of (`dx`, `dz`), if it is not (near) zero.
fn unit(dx: f32, dz: f32) -> Option<[f32; 2]> {
    let length = dx.hypot(dz);
    (length > 1e-3).then(|| [dx / length, dz / length])
}

/// The keys of the eight ways nearest `relative` (the way to go less the
/// view's yaw; a positive angle is to the left, `kin`).
pub fn keys_toward(relative: f32) -> u32 {
    use std::f32::consts::FRAC_PI_4;
    let sector = (wrap(relative) / FRAC_PI_4).round() as i32;
    match sector.rem_euclid(8) {
        0 => key::FORWARD,
        1 => key::FORWARD | key::LEFT,
        2 => key::LEFT,
        3 => key::BACK | key::LEFT,
        4 => key::BACK,
        5 => key::BACK | key::RIGHT,
        6 => key::RIGHT,
        _ => key::FORWARD | key::RIGHT,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v026g::bots::kin::Kinematics;

    #[test]
    fn the_eight_ways_match_the_movement_model() {
        let kin = Kinematics::default();
        for k in -8..8 {
            let relative = k as f32 * 0.4;
            let keys = keys_toward(relative);
            let [x, z] = kin.target_velocity(keys, 0.0, 1.0);
            // The way the keys go is within 22.5 degrees of the asked one.
            let went = yaw_of([x, z]);
            assert!(
                wrap(went - relative).abs() <= std::f32::consts::FRAC_PI_8 + 1e-3,
                "{relative} -> {keys:#x}"
            );
        }
    }
    use crate::v026g::bots::path::Waypoint;

    fn route(points: &[(f32, f32)]) -> Route {
        Route::new(
            points
                .iter()
                .map(|&(x, z)| Waypoint {
                    at: Vec3::new(x, 0.0, z),
                    node: 0,
                    jump: false,
                    low: false,
                })
                .collect(),
        )
    }

    /// A route along x with waypoint 1 (at x = 5) up a ledge, and the leg
    /// to waypoint 2 low (from x = 5 to 10).
    fn ledge_then_low() -> Route {
        let p = |x: f32, jump, low| Waypoint {
            at: Vec3::new(x, 0.0, 0.0),
            node: 0,
            jump,
            low,
        };
        Route::new(vec![
            p(0.0, false, false),
            p(5.0, true, false),
            p(10.0, false, true),
            p(20.0, false, false),
        ])
    }

    fn facing_x(feet: Vec3) -> Pose {
        Pose {
            feet,
            yaw: yaw_of([1.0, 0.0]),
            pitch: 0.0,
            speed: 0.0,
        }
    }

    /// A body kept short of a ledge (it hits the edge) jumps again every
    /// [`JUMP_AGAIN_S`], walking or aiming; once up, it does not.
    #[test]
    fn a_jump_that_did_not_get_up_is_tried_again_also_aiming() {
        let mut rng = Rng::new(3);
        for aiming in [false, true] {
            let mut motor = Motor::new(Feel::default(), &mut rng);
            let mut route = ledge_then_low();
            let pose = facing_x(Vec3::new(4.5, 0.0, 0.0));
            let mut jumps = Vec::new();
            for tick in 0..100 {
                let (decision, _) = if aiming {
                    motor.walk_aiming(&mut route, &pose, 5.0, |_| true, (pose.yaw, 0.0))
                } else {
                    motor.walk(&mut route, &pose, 5.0, |_| true)
                };
                if decision.keys & key::JUMP != 0 {
                    jumps.push(tick);
                }
            }
            // 5 s held short of it: a jump at once and every second.
            let again = (JUMP_AGAIN_S / TICK_S) as usize;
            assert_eq!(jumps.len(), 100 / again, "{aiming}: {jumps:?}");
            assert!(jumps.windows(2).all(|w| w[1] - w[0] == again), "{jumps:?}");
            // Up on the ledge: no more jumps.
            let up = facing_x(Vec3::new(5.5, 0.0, 0.0));
            for _ in 0..40 {
                let (decision, _) = motor.walk(&mut route, &up, 5.0, |_| true);
                assert_eq!(decision.keys & key::JUMP, 0);
            }
        }
    }

    /// A low leg is walked crouched, from a metre before it, and not
    /// sprinted; past it the body stands again.
    #[test]
    fn a_low_leg_is_walked_crouched() {
        let mut rng = Rng::new(5);
        let mut motor = Motor::new(Feel::default(), &mut rng);
        let mut route = ledge_then_low();
        let crouched = |motor: &mut Motor, route: &mut Route, x: f32| {
            let (decision, _) = motor.walk(route, &facing_x(Vec3::new(x, 0.0, 0.0)), 5.0, |_| true);
            decision.keys & key::CROUCH != 0
        };
        assert!(!crouched(&mut motor, &mut route, 2.0));
        assert!(crouched(&mut motor, &mut route, 4.5));
        assert!(crouched(&mut motor, &mut route, 7.0));
        let (decision, _) =
            motor.walk(&mut route, &facing_x(Vec3::new(7.0, 0.0, 0.0)), 5.0, |_| {
                true
            });
        assert_eq!(decision.keys & key::SPRINT, 0);
        assert!(!crouched(&mut motor, &mut route, 12.0));
        // Aiming too (a route of its own: a route keeps where the body was).
        let mut route = ledge_then_low();
        let (decision, _) = motor.walk_aiming(
            &mut route,
            &facing_x(Vec3::new(7.0, 0.0, 0.0)),
            5.0,
            |_| true,
            (0.0, 0.0),
        );
        assert_ne!(decision.keys & key::CROUCH, 0, "aiming too");
    }

    /// Walks a body that does what the keys and turns say, with the
    /// movement model, until it arrives; returns the decisions.
    fn walk(route: &mut Route, start: Vec3, yaw: f32, ticks: usize) -> (Vec<Decision>, Pose) {
        let kin = Kinematics::default();
        let mut rng = Rng::new(7);
        let mut motor = Motor::new(Feel::default(), &mut rng);
        let mut pose = Pose {
            feet: start,
            yaw,
            pitch: 0.0,
            speed: 0.0,
        };
        let mut blend = crate::v026g::bots::kin::Blend::default();
        let mut decisions = Vec::new();
        for _ in 0..ticks {
            let (decision, walked) = motor.walk(route, &pose, 5.0, |_| true);
            assert!(!matches!(walked, Walked::Off(_)), "{pose:?}");
            decisions.push(decision);
            if walked == Walked::Arrived {
                break;
            }
            pose.yaw = wrap(pose.yaw + decision.turn.0);
            pose.pitch += decision.turn.1;
            let local = blend.tick(kin.target_local(decision.keys, 1.0), kin.blend_ticks);
            let velocity = crate::v026g::bots::kin::to_world(local, pose.yaw);
            pose.feet = Vec3::new(
                pose.feet.x + velocity[0] * TICK_S,
                0.0,
                pose.feet.z + velocity[1] * TICK_S,
            );
            pose.speed = velocity[0].hypot(velocity[1]);
        }
        (decisions, pose)
    }

    #[test]
    fn a_body_walks_a_route_round_a_corner_smoothly_and_sprints_on_the_straight() {
        let feel = Feel::default();
        // 30 m along +z, a right angle, 20 m along +x; starting faced away.
        let mut r = route(&[(0.0, 0.0), (0.0, 30.0), (20.0, 30.0)]);
        let (decisions, pose) = walk(&mut r, Vec3::ZERO, PI * 0.9, 600);
        assert!(pose.feet.distance_xz(Vec3::new(20.0, 0.0, 30.0)) <= feel.arrive_m + 0.3);
        // Smooth: the turning speed changes by at most the top change a
        // tick, and never passes the top speed.
        let rates: Vec<f32> = decisions.iter().map(|d| d.turn.0 / TICK_S).collect();
        for pair in rates.windows(2) {
            assert!(
                (pair[1] - pair[0]).abs() <= feel.max_turn_accel * TICK_S + 1e-3
                    || pair[1] == 0.0
                    || pair[1].abs() < pair[0].abs(),
                "{pair:?}"
            );
        }
        assert!(rates.iter().all(|r| r.abs() <= feel.max_turn_speed + 1e-3));
        // It sprinted on the 30 m stretch, and not into the corner.
        let sprint = |d: &Decision| d.keys & key::SPRINT != 0;
        assert!(decisions.iter().any(sprint));
        // The about-turn at the start: forward let go, then taken up.
        assert_eq!(decisions[0].keys & key::FORWARD, 0);
        assert!(decisions.iter().any(|d| d.keys & key::FORWARD != 0));
        // 50 m at 3.3 to 5.25 m/s plus the turn: well under 20 s.
        assert!(decisions.len() < 400, "{}", decisions.len());
        // Keys change seldom: no flicker of forward or sprint.
        let changes = decisions
            .windows(2)
            .filter(|w| w[0].keys != w[1].keys)
            .count();
        assert!(changes <= 8, "{changes}");
    }

    #[test]
    fn standing_it_looks_about_in_eased_turns() {
        let mut rng = Rng::new(3);
        let mut motor = Motor::new(Feel::default(), &mut rng);
        let mut pose = Pose {
            feet: Vec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
            speed: 0.0,
        };
        let mut turned = 0.0_f32;
        for _ in 0..200 {
            let decision = motor.stand(&pose, &mut rng);
            assert_eq!(decision.keys, 0);
            assert!(decision.turn.0.abs() <= Feel::default().max_turn_speed * TICK_S + 1e-4);
            pose.yaw = wrap(pose.yaw + decision.turn.0);
            turned += decision.turn.0.abs();
        }
        assert!(turned > 1.0, "it looked about: {turned}");
    }

    /// The floor's height at `x`: a lip 0.4 m high at x = 5.
    fn lip(x: f32) -> f32 {
        if x >= 5.0 { 0.4 } else { 0.0 }
    }

    /// A body reckoned walking along x at a walk's 3.3 m/s from x = 2 for
    /// `ticks`, on floors as high as `ground` says; every `period` ticks a
    /// world puts the reckoning where `world` (the reckoned x, the tick)
    /// has it. The reckoning does not pass `reach` (the foot of a jump: it
    /// walks the grid and does not jump). Returns the ticks with a jump.
    fn reckoned_to(
        motor: &mut Motor,
        route: &mut Route,
        ground: impl Fn(f32) -> f32,
        mut world: impl FnMut(f32, usize) -> f32,
        (period, reach): (usize, f32),
        ticks: usize,
        aiming: bool,
    ) -> Vec<usize> {
        let step = 3.3 * TICK_S;
        let mut x = 2.0_f32;
        let mut jumps = Vec::new();
        for tick in 0..ticks {
            if tick > 0 && tick % period == 0 {
                x = world(x, tick);
            }
            let pose = Pose {
                feet: Vec3::new(x, ground(x), 0.0),
                yaw: yaw_of([1.0, 0.0]),
                pitch: 0.0,
                speed: 3.3,
            };
            let (decision, walked) = if aiming {
                motor.walk_aiming(route, &pose, 5.0, |_| true, (pose.yaw, 0.0))
            } else {
                motor.walk(route, &pose, 5.0, |_| true)
            };
            assert_eq!(walked, Walked::OnTheWay, "{tick}: {pose:?}");
            if decision.keys & key::JUMP != 0 {
                jumps.push(tick);
            }
            x = (x + step).min(reach);
        }
        jumps
    }

    fn reckoned(
        motor: &mut Motor,
        route: &mut Route,
        ground: impl Fn(f32) -> f32,
        world: impl FnMut(f32, usize) -> f32,
        ticks: usize,
        aiming: bool,
    ) -> Vec<usize> {
        reckoned_to(
            motor,
            route,
            ground,
            world,
            (30, f32::INFINITY),
            ticks,
            aiming,
        )
    }

    /// Caught at x = 4.6, before the lip: a world puts the reckoning back
    /// there (plus `replay`: the inputs since the world's tick played again).
    fn caught(replay: f32) -> impl FnMut(f32, usize) -> f32 {
        move |x, _| x.min(4.6 + replay)
    }

    /// A body caught at the foot of a lip the grid walks up (the route's
    /// corners far apart: flat in its heights) hops at the first world
    /// that shows it, walking or aiming, whether the inputs played again
    /// took the reckoning up the lip already or not; once.
    #[test]
    fn a_body_caught_at_a_lip_hops_once() {
        let mut rng = Rng::new(11);
        for aiming in [false, true] {
            for replay in [0.0, 0.8] {
                let mut motor = Motor::new(Feel::default(), &mut rng);
                let mut route = route(&[(0.0, 0.0), (30.0, 0.0)]);
                let jumps = reckoned(&mut motor, &mut route, lip, caught(replay), 100, aiming);
                assert_eq!(jumps, [30], "aiming {aiming}, replay {replay}");
            }
        }
    }

    /// Caught for 12 s: a hop at most every [`Feel::hop_every_s`].
    #[test]
    fn the_hop_cools_down() {
        let mut rng = Rng::new(12);
        let feel = Feel::default();
        let mut motor = Motor::new(feel.clone(), &mut rng);
        let mut route = route(&[(0.0, 0.0), (30.0, 0.0)]);
        let jumps = reckoned(&mut motor, &mut route, lip, caught(0.0), 240, false);
        // A world every 1.5 s: the first after the cool-down hops.
        assert_eq!(jumps, [30, 120, 210]);
        let every = (feel.hop_every_s / TICK_S) as usize;
        assert!(jumps.windows(2).all(|w| w[1] - w[0] >= every), "{jumps:?}");
    }

    /// On flat ground no hop: walking (the movement model, round a corner,
    /// sprinting), caught (by another body, a wall: a jump does not help),
    /// or a little slower than reckoned up a ramp.
    #[test]
    fn flat_walking_never_hops() {
        let mut r = route(&[(0.0, 0.0), (0.0, 30.0), (20.0, 30.0)]);
        let (decisions, _) = walk(&mut r, Vec3::ZERO, PI * 0.9, 600);
        assert!(decisions.iter().all(|d| d.keys & key::JUMP == 0));
        let mut rng = Rng::new(13);
        for aiming in [false, true] {
            let straight = || route(&[(0.0, 0.0), (30.0, 0.0)]);
            let mut motor = Motor::new(Feel::default(), &mut rng);
            let jumps = reckoned(
                &mut motor,
                &mut straight(),
                |_| 0.0,
                caught(0.0),
                240,
                aiming,
            );
            assert!(
                jumps.is_empty(),
                "caught on flat ground, aiming {aiming}: {jumps:?}"
            );
            // Up a 10° ramp at 85 % of the reckoned speed, a world every
            // 3 s: 1.3 m behind at each, 15 % of the way meant.
            let mut motor = Motor::new(Feel::default(), &mut rng);
            let slower = |_, tick: usize| 2.0 + 0.85 * 3.3 * TICK_S * tick as f32;
            let jumps = reckoned_to(
                &mut motor,
                &mut straight(),
                |x| x * 0.18,
                slower,
                (60, f32::INFINITY),
                150,
                aiming,
            );
            assert!(
                jumps.is_empty(),
                "slower up a ramp, aiming {aiming}: {jumps:?}"
            );
        }
    }

    /// No hop crouched under a low floor, nor near a jump of the route
    /// (its own jumps only, as without the hop), nor within
    /// [`Feel::hop_after_jump_s`] of one.
    #[test]
    fn no_hop_under_a_low_floor_or_by_a_planned_jump() {
        let mut rng = Rng::new(14);
        let point = |x: f32, jump, low| Waypoint {
            at: Vec3::new(x, 0.0, 0.0),
            node: 0,
            jump,
            low,
        };
        for aiming in [false, true] {
            // The lip under a low floor (x 4-8).
            let mut motor = Motor::new(Feel::default(), &mut rng);
            let mut route = Route::new(vec![
                point(0.0, false, false),
                point(4.0, false, false),
                point(8.0, false, true),
                point(30.0, false, false),
            ]);
            let jumps = reckoned(&mut motor, &mut route, lip, caught(0.0), 240, aiming);
            assert!(jumps.is_empty(), "low, aiming {aiming}: {jumps:?}");
            // Caught at the lip 1.2 m short of the route's own jump (at x
            // 5.8 to 6.2), which the reckoning walks to: the jumps the route
            // makes, and no more (whatever the time since its last).
            let ledge = || {
                Route::new(vec![
                    point(0.0, false, false),
                    point(5.8, false, false),
                    point(6.2, true, false),
                    point(30.0, false, false),
                ])
            };
            let feel = Feel {
                hop_after_jump_s: 0.0,
                ..Feel::default()
            };
            let mut motor = Motor::new(feel, &mut rng);
            let jumps = reckoned_to(
                &mut motor,
                &mut ledge(),
                lip,
                caught(0.0),
                (30, 5.8),
                240,
                aiming,
            );
            let no_hop = Feel {
                hop_behind_m: f32::INFINITY,
                ..Feel::default()
            };
            let mut motor = Motor::new(no_hop, &mut rng);
            let planned = reckoned_to(
                &mut motor,
                &mut ledge(),
                lip,
                caught(0.0),
                (30, 5.8),
                240,
                aiming,
            );
            assert!(!planned.is_empty());
            assert_eq!(jumps, planned, "by a planned jump, aiming {aiming}");
            // A jump of the route at x 2.6 to 3 (made at once), then caught
            // at the lip 1.5 s later: not then, at the next world.
            let mut motor = Motor::new(Feel::default(), &mut rng);
            let mut route = Route::new(vec![
                point(0.0, false, false),
                point(2.6, false, false),
                point(3.0, true, false),
                point(30.0, false, false),
            ]);
            let jumps = reckoned(&mut motor, &mut route, lip, caught(0.0), 100, aiming);
            assert_eq!(jumps, [0, 60], "after a planned jump, aiming {aiming}");
        }
    }
}
