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
//! - **It lines up for a jump of its route** ([`Motor::jump_ahead`]): from
//!   [`LINE_UP_FROM_M`] before the take-off it walks to a run-up on the
//!   jump's own line, turns to the jump (standing, for a turn of more than
//!   [`LINE_UP_GO`]), and jumps walking along it from the take-off; it
//!   keeps to that line through the jump. A jump that did not get it up is
//!   tried again only from the run-up, [`JUMP_TRIES`] times in all; then
//!   it gives up on it ([`Walked::JumpFailed`]).
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
    /// The walk's keys are let go for a target more than this off the
    /// facing, radians (it turns on the spot) …
    pub stop_turn: f32,
    /// … and taken up again once the target is within this.
    pub go_turn: f32,
    /// The point steered at is this far ahead, metres, plus
    /// `look_ahead_per_speed` × speed.
    pub look_ahead_m: f32,
    /// How far under a walked leg's floor a body is still on the route, m
    /// (`path::OFF_WALK_BELOW_M`); 1.5 (`path::OFF_ROUTE_HEIGHT_M`) is the old rule.
    pub off_walk_below_m: f32,
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
    /// The lip guard ([`drive::lip_guard`]): the ticks a walk's keys count held when they are played forward
    /// to a stop on a lip's floors; 0 is off.
    pub lip_guard_ticks: u32,
    /// The look-ahead of a walk on a lip's floors, metres; 0 is off (the walk looks as far as anywhere).
    pub lip_look_ahead_m: f32,
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
            off_walk_below_m: super::path::OFF_WALK_BELOW_M,
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
            lip_guard_ticks: 2,
            lip_look_ahead_m: 0.8,
        }
    }
}

/// Where the body is, as the motor needs it.
#[derive(Clone, Copy, Debug, Default)]
pub struct Pose {
    pub feet: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    /// Ground speed, m/s.
    pub speed: f32,
    /// Hurt so it does not jump now ([`Hurt::may_jump`]): the motor
    /// presses no jump and counts no try at one.
    pub no_jump: bool,
}

/// What the world last showed of the body's hurts, as the jump needs it
/// (the sprint-jump RE, `FINDINGS.md` §1.8).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Hurt {
    /// A leg pool (`Damage::part_health` 6 or 7, 0.5 full) at 0: the game
    /// takes no jump of any kind then, for about 8 s (PROVEN).
    pub leg_broken: bool,
    /// The last bullet hit, the world's ms (`m_last_hit_time_ms`).
    pub last_hit_ms: Option<u32>,
}

/// After a hit the body is staggered this long, ms (`stagger_duration`,
/// PROVEN): slowed by up to half, so a jump of the route pressed then may
/// fall short of its landing [GUESS]; it is held until the stagger ends.
pub const HIT_JUMP_HOLD_MS: u32 = 500;

/// A leg pool at or under this is broken (the worlds show 0, and 0.01-0.03
/// just after it mends).
const LEG_BROKEN: f32 = 0.005;

impl Hurt {
    /// From the world's leg pools and last hit at `now`, ms (a hit time
    /// of 0 or later than `now` is none).
    pub fn of(legs: [Option<f32>; 2], last_hit_ms: u32, now: u32) -> Self {
        Self {
            leg_broken: legs.iter().flatten().any(|&pool| pool <= LEG_BROKEN),
            last_hit_ms: (last_hit_ms != 0 && last_hit_ms <= now).then_some(last_hit_ms),
        }
    }

    /// Whether the body may jump at `now`, ms: no broken leg, and not
    /// within [`HIT_JUMP_HOLD_MS`] of the last hit. Stamina is not in the
    /// world's state, so its gate (no jump under 10, nor from 0 until back
    /// over 20) is not known here.
    pub fn may_jump(&self, now: u32) -> bool {
        !self.leg_broken
            && self
                .last_hit_ms
                .is_none_or(|at| now >= at.saturating_add(HIT_JUMP_HOLD_MS))
    }
}

/// What the motor did with a route this tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Walked {
    OnTheWay,
    Arrived,
    /// Farther off the route than the caller allows.
    Off(f32),
    /// A jump of the route did not get the body up in [`JUMP_TRIES`]
    /// tries: the take-off.
    JumpFailed(Vec3),
}

/// A jump not yet up its ledge is pressed again no sooner than this,
/// seconds (the game's jump is over 0.8 s after the press,
/// `body::JUMP_AIR_MS`).
pub const JUMP_AGAIN_S: f32 = 1.0;
/// A jump of the route is lined up from this far before its take-off along
/// the route, metres ([`Motor::jump_ahead`]).
pub const LINE_UP_FROM_M: f32 = 2.5;
/// The run-up: the jump is walked into from this far behind the take-off on
/// the jump's line, metres (or half that, or none, where the way back is
/// not clear).
pub const RUN_UP_M: f32 = 0.8;
/// On the jump's line: within this of it to a side, metres …
const LINED_SIDE_M: f32 = 0.3;
/// … from this far behind the run-up …
const LINED_BEHIND_M: f32 = 0.5;
/// … to this far past the take-off (a body pressed to the ledge's face is
/// about a tenth of a metre past it: the face is about the landing's cell
/// centre, `player_bots_stuck.md` §8.9).
const LINED_PAST_M: f32 = 0.15;
/// The run-up is reached this near, metres.
const AT_RUN_UP_M: f32 = 0.3;
/// On the jump's line it walks only facing within this of the jump's way,
/// radians (else it stands and turns) …
pub const LINE_UP_GO: f32 = 0.45;
/// … and presses jump facing within this.
pub const JUMP_FACING: f32 = 0.2;
/// The game's jump rises from about 0.2 s after the press (the worlds,
/// `body::JUMP_AIR_MS`): it is pressed that long before the body walks
/// over the take-off at its speed …
const JUMP_LEAD_S: f32 = 0.2;
/// … and this much more, metres.
const PRESS_M: f32 = 0.15;
/// A body moving less than this a tick, m, is standing ([`Ahead::going`]).
const STANDING_M: f32 = 0.02;
/// Moving within this of the jump's way, radians, it goes the way
/// ([`Ahead::going`]).
const GOING_TURN: f32 = 0.6;
/// After a press the body is kept walking on the jump's line this long,
/// seconds: the jump's 0.8 s and a tick.
const CARRY_S: f32 = 0.85;
/// The feet within this under the landing's floor (and at most half the
/// jump's rise), within [`UP_NEAR_M`] of it across: up the ledge, metres.
pub const UP_M: f32 = 0.35;
const UP_NEAR_M: f32 = 1.5;
/// A jump is pressed at most this many times while it does not get the body
/// up; then the motor gives up on it ([`Walked::JumpFailed`]). In the
/// owner's matches a jump that did not get up at once got up at the next
/// press 29 times in 124, at the one after 15 times
/// (`player_bots_stuck.md` §8.9).
pub const JUMP_TRIES: u32 = 3;
/// The tries at a jump count on over a route planned again to it within
/// this, seconds.
pub(super) const TRY_KEPT_S: f32 = 10.0;
/// A world that shows the body at most this far from the take-off of the
/// jump it last pressed, on its floor, and not up, has it still under that
/// jump: the route is looked at from that jump's leg again, metres.
const UNDER_JUMP_M: f32 = 1.5;
/// A route's end higher than the feet by more than this is not arrived at
/// (the end up a ledge: it is jumped to), metres.
pub(super) const ARRIVE_BELOW_M: f32 = 0.5;
/// An exact spot's route end ([`Motor::exact_end`]) lower than the feet by
/// more than this is not arrived at (the bot is over it: on an overhang, a
/// box top, the top of a drop; it walks on by the route), metres. GUESS:
/// over a step or stair's rise the grid walks, under a storey. Collect's
/// Fetch→Take uses the same rule. Cologne CG-17: a bot on the overhang over
/// battery 2's stand was "there". Every route's end at first: the batt
/// bench's stuck rose +1.5 to +2.7 (about 2 SE) for it.
pub(super) const ARRIVE_ABOVE_M: f32 = 0.6;
/// A route whose last leg is a jump is arrived at only on its landing:
/// the end at most this higher than the feet, metres. GUESS: above a
/// floor's unevenness under the feet, well below the lowest jump (a
/// pocket's rim 0.23 m up, 0.56 m off, was "walked" with no press).
const ARRIVE_ON_LANDING_M: f32 = 0.15;
// A landing counts as reached only below the lowest rim a pocket's exit
// jumps to, or a low rim would be "walked" with no press.
const _: () = assert!(ARRIVE_ON_LANDING_M < mapdata::grid::POCKET_EXIT_LOW_M);
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

/// The walk keys are the set nearest the way to go, and a set is held at
/// least this many ticks (100 ms) while the way stays within
/// [`KEY_FORCE_RAD`] of it: no chatter between two sets at a boundary. A
/// human holds a set 100 ms or more in 92 % of his holds (574 players in 12+
/// live matches), a one-tick hold is 1-2 %.
const KEY_HOLD_TICKS: u32 = 2;
/// A set is let go at once when the way to go is this far from it, radians
/// (more than a sector and a half: the facing swung round).
const KEY_FORCE_RAD: f32 = 3.0 * std::f32::consts::FRAC_PI_8;

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
    /// The route jump being made ([`Motor::jump_ahead`]).
    jump_try: Option<JumpTry>,
    /// Whether this tick's walk lines up for a jump of the route (the caller
    /// lets go of crouch then: a crouched body does not jump).
    lining_up: bool,
    /// The route jump this tick's decision pressed, if any.
    pressed: Option<RoutePress>,
    /// The sway's phase and the time, seconds, for it.
    phase: f32,
    t: f32,
    /// Standing: the heading looked at, and ticks till the next look.
    look: Option<(f32, u32)>,
    /// The turn's time constant for this tick when not the feel's (the
    /// reflex aims faster or slower by skill).
    pub turn_time: Option<f32>,
    /// The aim's turn rate this tick, yaw and pitch, radians a second (the
    /// reflex's, at a seen target: `CombatTuning::turn_feed`): a turn onto
    /// the aim follows it ([`Motor::aim_toward`]).
    pub track: Option<(f32, f32)>,
    /// This tick's route ends at an exact spot the play asked for (a goal
    /// within 0: Collect's walk to its stand): it is not arrived at from over
    /// it ([`ARRIVE_ABOVE_M`]). Cleared each tick before the walk sets it.
    pub exact_end: bool,
    /// This tick's walk looks no further ahead on the route than this, m (set by the caller each tick: on a
    /// lip's floors, where a line to a point further on cuts a corner over the rim).
    pub ahead_cap: Option<f32>,
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
    /// The jump this tick's decision pressed, if any.
    jump: Option<JumpBy>,
    /// The walk key set held ([`Motor::pick_keys`]).
    key_hold: Option<KeyHold>,
}

/// The walk key set held: its sector (45 degrees apart, 0 forward, positive
/// to the left), the motor's tick it was last used and for how many ticks it
/// has been held.
#[derive(Clone, Copy, Debug)]
struct KeyHold {
    sector: i32,
    at: u32,
    held: u32,
}

/// A jump of the route being made: its take-off and landing (the
/// waypoints), the presses made at it and the tick of the last, and
/// whether the next press waits for the run-up ([`RUN_UP_M`]).
#[derive(Clone, Copy, Debug)]
struct JumpTry {
    take_off: Vec3,
    landing: Vec3,
    tries: u32,
    pressed: Option<u32>,
    back: bool,
    /// Its outcome is in the bench's counts ([`counts`]).
    #[cfg(test)]
    counted: bool,
}

/// A press of a route jump, for the trace: the jump's take-off and landing,
/// which try, how far before the take-off along the jump's way the feet
/// were (negative past it), and how far from the way the heading was.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RoutePress {
    pub take_off: Vec3,
    pub landing: Vec3,
    pub tries: u32,
    pub before: f32,
    pub facing: f32,
}

/// What the jump ahead asks of this tick's walk ([`Motor::jump_ahead`]).
#[derive(Clone, Copy, Debug)]
struct Ahead {
    /// The point to walk toward.
    aim: Vec3,
    /// On the jump's line (or in the jump): walk only facing its way, and
    /// press near the take-off.
    lined: bool,
    /// The jump's way, unit (map x/z).
    way: [f32; 2],
    /// How far before the take-off along the way the feet are.
    before: f32,
    /// Not moving against the jump's way (since the last tick: standing,
    /// or within [`GOING_TURN`] of it).
    going: bool,
    /// Given up on: the take-off.
    failed: Option<Vec3>,
}

/// Why the motor pressed jump.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JumpBy {
    /// A jump link of the route ([`Motor::jump_key`]).
    Route,
    /// Caught on rising ground ([`Motor::hop_key`]).
    Hop,
}

impl Motor {
    pub fn new(feel: Feel, rng: &mut Rng) -> Self {
        Self {
            feel,
            rate: (0.0, 0.0),
            walking: false,
            sprinting: false,
            facing_ticks: 0,
            jump_try: None,
            lining_up: false,
            pressed: None,
            phase: rng.range(0.0, 2.0 * PI),
            t: 0.0,
            look: None,
            turn_time: None,
            track: None,
            exact_end: false,
            ahead_cap: None,
            clock: 0,
            trail: VecDeque::new(),
            way: [0.0, 0.0],
            hopped: None,
            jump: None,
            key_hold: None,
        }
    }

    /// The walk keys for a way to go `relative` to the facing (positive to
    /// the left): the set of the eight nearest the way, held at least
    /// [`KEY_HOLD_TICKS`] unless the way swung round. The body moves where
    /// the keys point and the view turns on its own: a player's keys and
    /// mouse. (A dither between the two nearest sets for a mean track was
    /// tried: its minor holds weave the body 0.25 m off the line and it fell
    /// off ledges it walks with the nearest set alone.)
    fn pick_keys(&mut self, relative: f32) -> u32 {
        use std::f32::consts::FRAC_PI_4;
        let relative = wrap(relative);
        let nearest = (relative / FRAC_PI_4).round() as i32;
        let held = self
            .key_hold
            .filter(|hold| self.clock.wrapping_sub(hold.at) <= 2);
        let (sector, ticks) = match held {
            Some(hold)
                if hold.sector == nearest
                    || (hold.held < KEY_HOLD_TICKS
                        && wrap(relative - hold.sector as f32 * FRAC_PI_4).abs()
                            <= KEY_FORCE_RAD) =>
            {
                (hold.sector, hold.held + 1)
            }
            _ => (nearest, 1),
        };
        #[cfg(test)]
        counts::key(
            held.is_some(),
            held.is_some_and(|hold| hold.sector != sector),
            held.map_or(0, |hold| hold.held),
        );
        self.key_hold = Some(KeyHold {
            sector,
            at: self.clock,
            held: ticks,
        });
        keys_toward(sector as f32 * FRAC_PI_4)
    }

    /// The jump this tick's decision pressed, and why.
    pub fn jumped(&self) -> Option<JumpBy> {
        self.jump
    }

    /// The route jump this tick's decision pressed, if any: where, which
    /// try, how lined up.
    pub fn route_press(&self) -> Option<RoutePress> {
        self.pressed
    }

    /// Whether this tick's walk lines up for a jump of the route, or is in
    /// one ([`Motor::jump_ahead`]): no crouch then.
    pub fn lining_up(&self) -> bool {
        self.lining_up
    }

    /// A new tick's decision begins.
    fn next_tick(&mut self) {
        self.clock = self.clock.wrapping_add(1);
        self.jump = None;
        self.pressed = None;
        self.lining_up = false;
    }

    /// Seconds since the tick `at` of this motor's clock.
    fn since(&self, at: u32) -> f32 {
        self.clock.wrapping_sub(at) as f32 * TICK_S
    }

    /// The tick's turn toward `yaw` and `pitch` from the pose: eased, within
    /// the top speed and change of speed, never past the aim.
    fn turn_toward(&mut self, pose: &Pose, yaw: f32, pitch: f32) -> (f32, f32) {
        self.turn_fed(pose, yaw, pitch, (0.0, 0.0))
    }

    /// [`Self::turn_toward`] onto the aim, following its turn rate
    /// ([`Self::track`]).
    fn aim_toward(&mut self, pose: &Pose, yaw: f32, pitch: f32) -> (f32, f32) {
        let feed = self.track.unwrap_or((0.0, 0.0));
        self.turn_fed(pose, yaw, pitch, feed)
    }

    /// [`Self::turn_toward`] with the aim turning at `feed` (radians a
    /// second): the ease asks the rate to close the gap and that rate too.
    /// The aim is where the target is at this tick, so a turn of the whole
    /// gap at one tick needs none of it: at a time constant of `turn_time`
    /// the part of the rate the ease leaves behind is `1 - tick / turn_time`
    /// of it, and that is what is added (none at a tick's time constant).
    fn turn_fed(&mut self, pose: &Pose, yaw: f32, pitch: f32, feed: (f32, f32)) -> (f32, f32) {
        let feel = &self.feel;
        let turn_time = self.turn_time.unwrap_or(feel.turn_time_s).max(0.02);
        let share = (1.0 - TICK_S / turn_time).max(0.0);
        let ease = |rate: &mut f32, left: f32, feed: f32| {
            let want = (left / turn_time + feed * share)
                .clamp(-feel.max_turn_speed, feel.max_turn_speed);
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
            ease(&mut yaw_rate, yaw_left, feed.0),
            ease(&mut pitch_rate, pitch_left, feed.1),
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
        self.next_tick();
        self.look = None;
        self.under_jump(route, pose.feet);
        route.set_below_walk(self.feel.off_walk_below_m);
        let (s, off) = route.progress(pose.feet);
        if self.arrived(route, s, pose.feet) {
            self.walking = false;
            self.sprinting = false;
            let turn = self.turn_toward(pose, pose.yaw, 0.0);
            return (Decision { keys: 0, turn }, Walked::Arrived);
        }
        if off > off_m {
            return (Decision::default(), Walked::Off(off));
        }
        let jump = self.jump_ahead(route, pose, s, &clear);
        let target = match jump {
            Some(jump) => jump.aim,
            None => {
                let mut ahead =
                    self.feel.look_ahead_m + self.feel.look_ahead_per_speed * pose.speed;
                ahead = ahead.min(self.ahead_cap.unwrap_or(f32::INFINITY));
                let mut target = route.point_at(s + ahead);
                while ahead > MIN_AHEAD_M && !clear(target) {
                    #[cfg(test)]
                    counts::cut();
                    ahead *= 0.6;
                    target = route.point_at(s + ahead);
                }
                target
            }
        };
        let (dx, dz) = (target.x - pose.feet.x, target.z - pose.feet.z);
        // No sway lining up: the jump's line is narrow.
        let sway = if jump.is_some() { 0.0 } else { self.sway() };
        let heading = if dx.abs() + dz.abs() < 1e-3 {
            pose.yaw
        } else {
            yaw_of([dx, dz]) + sway
        };
        let turn = self.turn_toward(pose, heading, 0.0);
        let relative = wrap(heading - (pose.yaw + turn.0));
        let still = relative.abs();

        // Walk while turning, the keys pointing where the way goes; let go
        // only for a way behind it (it turns on the spot), and on a jump's
        // line for a turn off its way: it stands and turns to it first.
        let lined = jump.is_some_and(|jump| jump.lined);
        if lined {
            self.walking = still < LINE_UP_GO;
        } else if self.walking && still > self.feel.stop_turn {
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
        // no sprint there. Lining up for a jump: neither (a crouched body
        // does not jump).
        let low = jump.is_none() && route.low_ahead(s, LOW_AHEAD_M);
        if low || jump.is_some() {
            self.sprinting = false;
        }
        let mut keys = 0;
        if self.walking {
            // A jump's line is walked facing its way, forward only; any
            // other walk by the key set nearest the way to go.
            keys |= if lined {
                self.key_hold = None;
                key::FORWARD
            } else {
                self.pick_keys(relative)
            };
            if self.sprinting {
                keys |= key::SPRINT;
            }
            if low {
                keys |= key::CROUCH;
            }
        } else {
            self.key_hold = None;
        }
        if let Some(jump) = jump {
            if let Some(take_off) = jump.failed {
                return (Decision { keys: 0, turn }, Walked::JumpFailed(take_off));
            }
            keys |= self.jump_key(&jump, still, pose);
        }
        // A hop goes the way the keys do: only with forward held.
        let may = keys & key::FORWARD != 0 && keys & key::BACK == 0 && !pose.no_jump;
        keys |= self.hop_key(route, s, pose.feet, unit(dx, dz), may);
        (Decision { keys, turn }, Walked::OnTheWay)
    }

    /// Whether the route is walked to its end: within [`Feel::arrive_m`] of
    /// it, and not under it ([`ARRIVE_BELOW_M`]: an end up a ledge is jumped
    /// to; [`ARRIVE_ON_LANDING_M`] when the last leg is a jump: on its
    /// landing) nor, for an exact spot's end ([`Motor::exact_end`]), over it
    /// ([`ARRIVE_ABOVE_M`]).
    fn arrived(&self, route: &Route, s: f32, feet: Vec3) -> bool {
        let end = route.end();
        let below = if route.points().last().is_some_and(|p| p.jump) {
            ARRIVE_ON_LANDING_M
        } else {
            ARRIVE_BELOW_M
        };
        (route.length() - s <= self.feel.arrive_m || feet.distance_xz(end) <= self.feel.arrive_m)
            && end.y - feet.y <= below
            && (!self.exact_end || feet.y - end.y <= ARRIVE_ABOVE_M)
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
        self.next_tick();
        self.look = None;
        self.sprinting = false;
        self.facing_ticks = 0;
        self.under_jump(route, pose.feet);
        route.set_below_walk(self.feel.off_walk_below_m);
        let (s, off) = route.progress(pose.feet);
        if self.arrived(route, s, pose.feet) {
            self.walking = false;
            let turn = self.aim_toward(pose, aim.0, aim.1);
            return (Decision { keys: 0, turn }, Walked::Arrived);
        }
        if off > off_m {
            let turn = self.aim_toward(pose, aim.0, aim.1);
            return (Decision { keys: 0, turn }, Walked::Off(off));
        }
        let jump = self.jump_ahead(route, pose, s, &clear);
        // On a jump's line the view turns to the jump's way: a jump goes
        // the way the body faces and its keys push (the jump branch of the
        // client's `bullet_character_controller::player_step`, 0x7BF80..
        // 0x7C13B), and a player looks where he jumps.
        let lined = jump.filter(|jump| jump.lined);
        let turn = match lined {
            Some(jump) => self.turn_toward(pose, yaw_of(jump.way), aim.1),
            None => self.aim_toward(pose, aim.0, aim.1),
        };
        let target = match jump {
            Some(jump) => jump.aim,
            None => {
                let mut ahead = self.feel.look_ahead_m.min(self.ahead_cap.unwrap_or(f32::INFINITY));
                let mut target = route.point_at(s + ahead);
                while ahead > MIN_AHEAD_M && !clear(target) {
                    #[cfg(test)]
                    counts::cut();
                    ahead *= 0.6;
                    target = route.point_at(s + ahead);
                }
                target
            }
        };
        let (dx, dz) = (target.x - pose.feet.x, target.z - pose.feet.z);
        let relative = wrap(yaw_of([dx, dz]) - (pose.yaw + turn.0));
        self.walking = true;
        let mut keys;
        if let Some(jump) = lined {
            let still = wrap(yaw_of(jump.way) - (pose.yaw + turn.0)).abs();
            self.walking = still < LINE_UP_GO;
            self.key_hold = None;
            keys = if self.walking { key::FORWARD } else { 0 };
        } else {
            // The set nearest the way to go, and the other nearest in
            // proportion ([`Motor::pick_keys`]): no steady bias.
            keys = self.pick_keys(relative);
            if jump.is_none() && route.low_ahead(s, LOW_AHEAD_M) {
                keys |= key::CROUCH;
            }
        }
        if let Some(jump) = jump {
            if let Some(take_off) = jump.failed {
                return (Decision { keys: 0, turn }, Walked::JumpFailed(take_off));
            }
            let still = wrap(yaw_of(jump.way) - (pose.yaw + turn.0)).abs();
            keys |= self.jump_key(&jump, still, pose);
        }
        // A hop goes the way the keys do: only with forward held.
        let may = keys & key::FORWARD != 0 && keys & key::BACK == 0 && !pose.no_jump;
        keys |= self.hop_key(route, s, pose.feet, unit(dx, dz), may);
        (Decision { keys, turn }, Walked::OnTheWay)
    }

    /// The route's next jump, if its take-off is within [`LINE_UP_FROM_M`]
    /// along the route and the body is not up it yet: where to walk to line
    /// up for it. A player lines up first and jumps from the edge: a jump
    /// goes the way the body faces and its keys push, so a jump pressed
    /// short of the take-off while steering past it flew on along the
    /// approach where the jump's way turned from it (Cologne's battery 6,
    /// `player_bots_stuck.md` §8.8-8.9).
    ///
    /// - Off the jump's line, or after a try that did not get up, it walks
    ///   to the run-up ([`RUN_UP_M`] behind the take-off on the line, or
    ///   half that, or the take-off itself, whichever it walks to straight).
    /// - On the line ([`LINED_SIDE_M`]), and in the jump ([`CARRY_S`]), it
    ///   walks along the jump's way, past the landing: the caller walks
    ///   only facing it ([`LINE_UP_GO`]) and presses at the take-off
    ///   ([`Motor::jump_key`]).
    /// - [`JUMP_TRIES`] presses that did not get it up: given up on.
    fn jump_ahead(
        &mut self,
        route: &Route,
        pose: &Pose,
        s: f32,
        clear: &impl Fn(Vec3) -> bool,
    ) -> Option<Ahead> {
        let (k, before_route) = route.next_jump(s)?;
        if before_route > LINE_UP_FROM_M {
            return None;
        }
        let points = route.points();
        let (take_off, landing) = (points[k - 1].at, points[k].at);
        let way = unit(landing.x - take_off.x, landing.z - take_off.z)?;
        let feet = pose.feet;
        if is_up(feet, take_off, landing) {
            #[cfg(test)]
            if let Some(t) = self.jump_try.as_mut()
                && t.tries > 0
                && !t.counted
                && (t.take_off - take_off).length() < 0.3
                && (t.landing - landing).length() < 0.3
            {
                t.counted = true;
                counts::up(t.tries);
            }
            return None;
        }
        // The tries at this jump, kept over a route planned again to it.
        let same = |t: &JumpTry| {
            (t.take_off - take_off).length() < 0.3
                && (t.landing - landing).length() < 0.3
                && t.pressed.is_none_or(|at| self.since(at) < TRY_KEPT_S)
        };
        let mut jump = match self.jump_try {
            Some(t) if same(&t) => t,
            _ => JumpTry {
                take_off,
                landing,
                tries: 0,
                pressed: None,
                back: false,
                #[cfg(test)]
                counted: false,
            },
        };
        let in_the_air = jump.pressed.is_some_and(|at| self.since(at) < CARRY_S);
        self.lining_up = true;
        if jump.tries >= JUMP_TRIES && !in_the_air {
            #[cfg(test)]
            if !jump.counted {
                jump.counted = true;
                counts::gave_up();
            }
            self.jump_try = Some(jump);
            return Some(Ahead {
                aim: take_off,
                lined: false,
                way,
                before: 0.0,
                going: false,
                failed: Some(take_off),
            });
        }
        let at = |m: f32| Vec3::new(take_off.x - way[0] * m, take_off.y, take_off.z - way[1] * m);
        let run_up = [RUN_UP_M, 0.5 * RUN_UP_M]
            .into_iter()
            .find(|&m| clear(at(m)))
            .unwrap_or(0.0);
        let (dx, dz) = (feet.x - take_off.x, feet.z - take_off.z);
        let along = dx * way[0] + dz * way[1];
        let side = (dx * way[1] - dz * way[0]).abs();
        let on_line =
            side <= LINED_SIDE_M && along <= LINED_PAST_M && along >= -(run_up + LINED_BEHIND_M);
        if jump.back && (at(run_up).distance_xz(feet) <= AT_RUN_UP_M || (run_up == 0.0 && on_line))
        {
            jump.back = false;
        }
        let lined = in_the_air || (on_line && !jump.back);
        let aim = if lined {
            // Along the jump's way, past the landing.
            Vec3::new(
                take_off.x + way[0] * 3.0,
                landing.y,
                take_off.z + way[1] * 3.0,
            )
        } else if clear(at(run_up)) {
            at(run_up)
        } else {
            // The run-up not in a line yet (round a corner): the route, up
            // to the take-off.
            let mut ahead = self.feel.look_ahead_m.min(before_route.max(0.0));
            let mut target = route.point_at(s + ahead);
            while ahead > MIN_AHEAD_M && !clear(target) {
                #[cfg(test)]
                counts::cut();
                ahead *= 0.6;
                target = route.point_at(s + ahead);
            }
            target
        };
        // The way the feet went since the last tick (the hop's trail): a
        // body facing the way but still moving off it (back from the
        // run-up, the walk's blend) is not pressed for.
        let going = self
            .trail
            .back()
            .filter(|&&(at, _)| self.clock.wrapping_sub(at) <= 1)
            .is_none_or(|&(_, before)| {
                let (mx, mz) = (feet.x - before.x, feet.z - before.z);
                let moved = mx.hypot(mz);
                moved < STANDING_M || mx * way[0] + mz * way[1] >= GOING_TURN.cos() * moved
            });
        self.jump_try = Some(jump);
        Some(Ahead {
            aim,
            lined,
            way,
            before: -along,
            going,
            failed: None,
        })
    }

    /// The jump of this tick up the route's next ledge ([`Self::jump_ahead`]):
    /// on the jump's line, walking and facing its way within
    /// [`JUMP_FACING`] (`still`) and going it ([`Ahead::going`]), as the
    /// body walks over the take-off
    /// ([`JUMP_LEAD_S`] ahead at its `speed`), and not in a jump or within
    /// [`JUMP_AGAIN_S`] of the last press, nor hurt so it does not jump
    /// ([`Pose::no_jump`]: no press, so no try is spent on a jump the game
    /// would not take). Each press counts a try; the next waits for the
    /// run-up.
    fn jump_key(&mut self, ahead: &Ahead, still: f32, pose: &Pose) -> u32 {
        let Some(mut jump) = self.jump_try else {
            return 0;
        };
        if pose.no_jump {
            return 0;
        }
        let speed = pose.speed;
        let rested = jump
            .pressed
            .is_none_or(|at| self.since(at) >= JUMP_AGAIN_S.max(CARRY_S));
        let near = ahead.before <= JUMP_LEAD_S * speed + PRESS_M;
        if !(ahead.lined && ahead.going && self.walking && still <= JUMP_FACING && near && rested) {
            return 0;
        }
        jump.tries += 1;
        jump.pressed = Some(self.clock);
        jump.back = true;
        self.jump_try = Some(jump);
        self.pressed = Some(RoutePress {
            take_off: jump.take_off,
            landing: jump.landing,
            tries: jump.tries,
            before: ahead.before,
            facing: still,
        });
        self.jump = Some(JumpBy::Route);
        key::JUMP
    }

    /// A world showed the body still under the jump it last pressed (on the
    /// take-off's floor, near it, not up) while the reckoning had taken it
    /// up and the route on past it (`body::JUMP_AIR_MS`): the route is
    /// looked at from that jump's leg again, so the jump is lined up again
    /// instead of walked on from under it.
    fn under_jump(&self, route: &mut Route, feet: Vec3) {
        let Some(jump) = self.jump_try else {
            return;
        };
        if jump.pressed.is_none_or(|at| self.since(at) >= TRY_KEPT_S)
            || is_up(feet, jump.take_off, jump.landing)
            || feet.distance_xz(jump.take_off) > UNDER_JUMP_M
            || (feet.y - jump.take_off.y).abs() > UP_M
        {
            return;
        }
        let leg = route
            .points()
            .iter()
            .position(|point| point.jump && (point.at - jump.landing).length() < 0.3);
        if let Some(k) = leg.filter(|&k| k >= 1 && route.segment() >= k) {
            route.back_to(k - 1);
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
    /// `may`: walking the way, not turning hard, not hurt so it does not
    /// jump. None by a low floor (where
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
            .jump_try
            .and_then(|jump| jump.pressed)
            .is_some_and(|at| self.since(at) < feel.hop_after_jump_s);
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
            self.jump = Some(JumpBy::Hop);
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
        self.next_tick();
        self.walking = false;
        self.sprinting = false;
        self.facing_ticks = 0;
        self.look = None;
        let turn = self.aim_toward(pose, aim.0, aim.1);
        Decision { keys: 0, turn }
    }

    /// The decision of one tick that steps straight to `to`, a metre or
    /// two off, no route (a take's stand round a battery, or a step back
    /// out of a nook the walk grid has no line from): the legs pick the key
    /// of the eight ways nearest the way to go while the view turns to
    /// `aim`, else keeps its yaw; no sprint.
    pub fn step_to(&mut self, pose: &Pose, to: Vec3, aim: Option<(f32, f32)>) -> Decision {
        self.next_tick();
        // Moved by keys off any route: the hop's trail is not the way.
        self.trail.clear();
        self.sprinting = false;
        self.facing_ticks = 0;
        self.look = None;
        let turn = match aim {
            Some((yaw, pitch)) => self.aim_toward(pose, yaw, pitch),
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
        self.next_tick();
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
        self.next_tick();
        self.trail.clear();
        self.rate = (0.0, 0.0);
        self.walking = false;
        self.sprinting = false;
        self.facing_ticks = 0;
        self.look = None;
        Decision::default()
    }
}

/// Whether feet are up a jump from `take_off` to `landing`: within
/// [`UP_NEAR_M`] of the landing across, and under its floor by at most
/// [`UP_M`] and half the jump's rise (0.1 m for a jump that does not
/// rise), and past the jump's middle along its way (a jump over a gap is
/// made only across it).
pub fn is_up(feet: Vec3, take_off: Vec3, landing: Vec3) -> bool {
    let rise = landing.y - take_off.y;
    let (dx, dz) = (landing.x - take_off.x, landing.z - take_off.z);
    let past = (feet.x - take_off.x) * dx + (feet.z - take_off.z) * dz >= 0.5 * (dx * dx + dz * dz);
    past && feet.distance_xz(landing) <= UP_NEAR_M
        && feet.y >= landing.y - UP_M.min((0.5 * rise).max(0.1))
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

/// The bench's counts (all matches of the run): the route jumps got up at
/// the first press and later, those given up ([`JUMP_TRIES`]), the cuts of
/// a look-ahead to a point a line is not clear to, and the fight's side
/// steps asked and refused (`Tactics::clear_step_for`).
#[cfg(test)]
pub mod counts {
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTS: [AtomicU64; 8] = [const { AtomicU64::new(0) }; 8];

    fn add(k: usize) {
        COUNTS[k].fetch_add(1, Ordering::Relaxed);
    }

    pub fn up(presses: u32) {
        add(usize::from(presses > 1));
    }

    pub fn gave_up() {
        add(2);
    }

    pub fn cut() {
        add(3);
    }

    pub fn side_step(clear: bool) {
        add(if clear { 4 } else { 5 });
    }

    /// The lip guard: a tick's keys changed to ones that stop on the floor, and one with none that does.
    pub fn lip_guard(changed: bool) {
        add(if changed { 6 } else { 7 });
    }

    static KEYS: [AtomicU64; 4] = [const { AtomicU64::new(0) }; 4];

    /// One tick of the walk keys: whether the plan goes on from a tick
    /// before, whether the key set changed, and how long the set before
    /// was held.
    pub fn key(continued: bool, changed: bool, held: u32) {
        KEYS[0].fetch_add(1, Ordering::Relaxed);
        if continued && changed {
            KEYS[1].fetch_add(1, Ordering::Relaxed);
            KEYS[2].fetch_add(1, Ordering::Relaxed);
            if held <= 1 {
                KEYS[3].fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// (walk key ticks, set changes, holds ended, of them one tick long).
    pub fn keys() -> [u64; 4] {
        std::array::from_fn(|k| KEYS[k].load(Ordering::Relaxed))
    }

    /// (up at the first press, up later, given up, look-ahead cuts, side
    /// steps clear, side steps refused, lip guard changed, lip guard none).
    pub fn counts() -> [u64; 8] {
        std::array::from_fn(|k| COUNTS[k].load(Ordering::Relaxed))
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

    /// The keys of `ticks` ticks for a way to go `relative` to the facing,
    /// as the walk asks for them: the sets (as key bits).
    fn keys_for(motor: &mut Motor, relative: f32, ticks: usize) -> Vec<u32> {
        (0..ticks)
            .map(|_| {
                motor.next_tick();
                motor.pick_keys(relative)
            })
            .collect()
    }

    /// The walk keys point where the way goes: the nearest of the eight
    /// sets (the way is within 22.5 degrees of it), held and not changed
    /// for a steady way, so a set changes far less than the humans' 2.4 a
    /// second.
    #[test]
    fn steady_ways_are_followed_by_the_nearest_set() {
        let kin = Kinematics::default();
        for degrees in [0.0_f32, 3.0, 10.0, 17.0, 22.0, 34.0, 50.0, 80.0, -25.0] {
            let relative = degrees.to_radians();
            let mut rng = Rng::new(3);
            let mut motor = Motor::new(Feel::default(), &mut rng);
            let sets = keys_for(&mut motor, relative, 320);
            assert!(sets.windows(2).all(|w| w[0] == w[1]), "{degrees} deg: {sets:?}");
            let [x, z] = kin.target_velocity(sets[0], 0.0, 1.0);
            assert!(
                wrap(yaw_of([x, z]) - relative).abs() <= 24.0_f32.to_radians(),
                "{degrees} deg: the keys went {} deg",
                yaw_of([x, z]).to_degrees()
            );
        }
        // Straight ahead is forward alone, with no change at all.
        let mut rng = Rng::new(3);
        let mut motor = Motor::new(Feel::default(), &mut rng);
        assert!(keys_for(&mut motor, 0.0, 100).iter().all(|&k| k == key::FORWARD));
    }

    /// A way wobbling over the boundary between two sets does not make the
    /// keys chatter: a set is held two ticks or more (a human holds 100 ms
    /// or more in 92 % of his holds), and the set changes at most as often
    /// as the humans' 2.4 a second while the way is steady.
    #[test]
    fn a_way_at_a_boundary_does_not_chatter() {
        let mut rng = Rng::new(3);
        let mut motor = Motor::new(Feel::default(), &mut rng);
        let boundary = std::f32::consts::FRAC_PI_8;
        let sets: Vec<u32> = (0..400)
            .map(|k| {
                motor.next_tick();
                // 2 degrees either side of the boundary, every tick.
                let wobble = if k % 2 == 0 { 0.035 } else { -0.035 };
                motor.pick_keys(boundary + wobble)
            })
            .collect();
        let mut holds = Vec::new();
        let mut run = 1;
        for w in sets.windows(2) {
            if w[0] == w[1] {
                run += 1;
            } else {
                holds.push(run);
                run = 1;
            }
        }
        assert!(
            holds.iter().skip(1).all(|&hold| hold >= KEY_HOLD_TICKS),
            "holds {holds:?}"
        );
    }

    /// A way that swings round (the facing turned a long way) changes the
    /// set at once, not after the two ticks' hold.
    #[test]
    fn a_swing_changes_the_set_at_once() {
        let mut rng = Rng::new(3);
        let mut motor = Motor::new(Feel::default(), &mut rng);
        assert_eq!(keys_for(&mut motor, 0.0, 3), [key::FORWARD; 3]);
        let after = keys_for(&mut motor, 1.57, 1)[0];
        assert_eq!(after, key::LEFT, "a quarter turn to the left at once");
        let after = keys_for(&mut motor, -1.57, 1)[0];
        assert_eq!(after, key::RIGHT);
    }

    fn facing_x(feet: Vec3) -> Pose {
        Pose {
            feet,
            yaw: yaw_of([1.0, 0.0]),
            pitch: 0.0,
            speed: 0.0,
            ..Default::default()
        }
    }

    /// A ledge 0.9 m high whose face is `face` m along the jump's way from
    /// `take_off`, as the client has one: a body walks into its face and
    /// stops, and gets up only by a jump pressed within `reach` m of the
    /// face whose move crosses it along the jump's way (within 0.4 rad:
    /// the client's jump goes the way the body faces and its keys push) in
    /// [`body::JUMP_AIR_MS`](crate::v026g::bots::body::JUMP_AIR_MS); none
    /// if not `gets_up`.
    struct Ledge {
        take_off: Vec3,
        way: [f32; 2],
        face: f32,
        reach: f32,
        gets_up: bool,
    }

    /// What a body walked by the motor at a [`Ledge`] did.
    struct Climb {
        /// The pose at each press of the jump key, and the tick.
        presses: Vec<(usize, Pose)>,
        decisions: Vec<Decision>,
        poses: Vec<Pose>,
        walked: Walked,
        up: bool,
        /// The tick it got up.
        up_at: Option<usize>,
    }

    /// Walks a body that does what the keys and turns say (the movement
    /// model) by the motor along `route`, up to `ledge`; aiming at the way
    /// it faces at the start if `aiming`. Until it arrives, gives up or
    /// `ticks` pass.
    fn climb_ledge(
        route: &mut Route,
        ledge: &Ledge,
        start: Vec3,
        yaw: f32,
        ticks: usize,
        aiming: bool,
    ) -> Climb {
        climb_ledge_hurt(route, ledge, start, yaw, ticks, aiming, 0)
    }

    /// [`climb_ledge`], hurt so it does not jump for the first `hurt_ticks`.
    fn climb_ledge_hurt(
        route: &mut Route,
        ledge: &Ledge,
        start: Vec3,
        yaw: f32,
        ticks: usize,
        aiming: bool,
        hurt_ticks: usize,
    ) -> Climb {
        let kin = Kinematics::default();
        let mut rng = Rng::new(17);
        let mut motor = Motor::new(Feel::default(), &mut rng);
        let mut pose = Pose {
            feet: start,
            yaw,
            pitch: 0.0,
            speed: 0.0,
            ..Default::default()
        };
        let mut blend = crate::v026g::bots::kin::Blend::default();
        let air_ticks = (crate::v026g::bots::body::JUMP_AIR_MS as f32 / 1000.0 / TICK_S) as usize;
        let landing_y = ledge.take_off.y + 0.9;
        let along = |at: Vec3| {
            (at.x - ledge.take_off.x) * ledge.way[0] + (at.z - ledge.take_off.z) * ledge.way[1]
        };
        let mut climb = Climb {
            presses: Vec::new(),
            decisions: Vec::new(),
            poses: Vec::new(),
            walked: Walked::OnTheWay,
            up: false,
            up_at: None,
        };
        let mut air: Option<usize> = None;
        for tick in 0..ticks {
            pose.no_jump = tick < hurt_ticks;
            let (decision, walked) = if aiming {
                motor.walk_aiming(route, &pose, 5.0, |_| true, (yaw, 0.0))
            } else {
                motor.walk(route, &pose, 5.0, |_| true)
            };
            assert!(!matches!(walked, Walked::Off(_)), "{tick}: {pose:?}");
            assert_eq!(motor.jumped().is_some(), decision.keys & key::JUMP != 0);
            climb.decisions.push(decision);
            climb.poses.push(pose);
            climb.walked = walked;
            if walked != Walked::OnTheWay {
                break;
            }
            if decision.keys & key::JUMP != 0 {
                assert_eq!(motor.jumped(), Some(JumpBy::Route));
                climb.presses.push((tick, pose));
                if ledge.face - along(pose.feet) <= ledge.reach {
                    air = Some(tick);
                }
            }
            pose.yaw = wrap(pose.yaw + decision.turn.0);
            pose.pitch += decision.turn.1;
            let local = blend.tick(kin.target_local(decision.keys, 1.0), kin.blend_ticks);
            let velocity = crate::v026g::bots::kin::to_world(local, pose.yaw);
            let mut to = Vec3::new(
                pose.feet.x + velocity[0] * TICK_S,
                pose.feet.y,
                pose.feet.z + velocity[1] * TICK_S,
            );
            if !climb.up && along(to) > ledge.face {
                let across = unit(velocity[0], velocity[1])
                    .is_some_and(|v| v[0] * ledge.way[0] + v[1] * ledge.way[1] >= 0.4_f32.cos());
                if ledge.gets_up && across && air.is_some_and(|at| tick - at <= air_ticks) {
                    climb.up = true;
                    climb.up_at = Some(tick);
                    to.y = landing_y;
                } else {
                    // Against the face.
                    let back = along(to) - ledge.face;
                    to.x -= ledge.way[0] * back;
                    to.z -= ledge.way[1] * back;
                }
            }
            pose.speed = (to - pose.feet).length() / TICK_S;
            pose.feet = to;
        }
        climb
    }

    /// A route along x to a take-off at x 4.6, up a 0.9 m ledge to x 5,
    /// and on along x up there; with the take-off at `(x, z)` and the
    /// jump's way turned to +z if `turned`.
    fn up_a_ledge(turned: bool) -> (Route, Ledge) {
        let p = |x: f32, y: f32, z: f32, jump| Waypoint {
            at: Vec3::new(x, y, z),
            node: 0,
            jump,
            low: false,
        };
        let (points, way) = if turned {
            (
                vec![
                    p(0.0, 0.0, 0.0, false),
                    p(5.0, 0.0, 0.0, false),
                    p(5.0, 0.9, 0.4, true),
                    p(5.0, 0.9, 6.0, false),
                ],
                [0.0, 1.0],
            )
        } else {
            (
                vec![
                    p(0.0, 0.0, 0.0, false),
                    p(4.6, 0.0, 0.0, false),
                    p(5.0, 0.9, 0.0, true),
                    p(10.0, 0.9, 0.0, false),
                ],
                [1.0, 0.0],
            )
        };
        let take_off = points[1].at;
        let ledge = Ledge {
            take_off,
            way,
            face: 0.2,
            reach: 1.2,
            gets_up: true,
        };
        (Route::new(points), ledge)
    }

    /// A route whose last leg is a jump 0.5 m off and 0.3 m up (a pocket's
    /// rim, `escape.rs`) is not walked until the body is up on it: the
    /// motor presses the jump and gets up. The same end by a walk (no jump)
    /// is arrived at as before.
    #[test]
    fn a_near_low_jump_at_a_routes_end_is_pressed() {
        let p = |x: f32, y: f32, jump| Waypoint {
            at: Vec3::new(x, y, 0.0),
            node: 0,
            jump,
            low: false,
        };
        let ledge = Ledge {
            take_off: Vec3::ZERO,
            way: [1.0, 0.0],
            face: 0.25,
            reach: 1.2,
            gets_up: true,
        };
        let mut route = Route::new(vec![p(0.0, 0.0, false), p(0.5, 0.3, true)]);
        let climb = climb_ledge(
            &mut route,
            &ledge,
            Vec3::ZERO,
            yaw_of([1.0, 0.0]),
            300,
            false,
        );
        assert!(!climb.presses.is_empty(), "{:?}", climb.poses.last());
        assert!(climb.up, "{:?}", climb.poses.last());
        assert_eq!(climb.walked, Walked::Arrived);
        let mut walk = Route::new(vec![p(0.0, 0.0, false), p(0.5, 0.3, false)]);
        let mut rng = Rng::new(17);
        let mut motor = Motor::new(Feel::default(), &mut rng);
        let (decision, walked) = motor.walk(&mut walk, &facing_x(Vec3::ZERO), 5.0, |_| true);
        assert_eq!(walked, Walked::Arrived);
        assert_eq!(decision.keys & key::JUMP, 0);
    }

    /// A cap on the look-ahead (on a lip's floors) keeps the walk to the leg it is on: a corner 0.5 m on is
    /// turned at, not cut by a heading to a point past it.
    #[test]
    fn a_capped_look_ahead_turns_at_the_corner_not_across_it() {
        let p = |x, z| Waypoint { at: Vec3::new(x, 0.0, z), node: 0, jump: false, low: false };
        let corner = || Route::new(vec![p(0.0, 0.0), p(3.0, 0.0), p(3.0, 4.0)]);
        let pose = Pose { feet: Vec3::new(2.5, 0.0, 0.0), yaw: yaw_of([1.0, 0.0]), pitch: 0.0, speed: 3.0, ..Default::default() };
        let heading = |cap: Option<f32>| {
            let mut rng = Rng::new(3);
            let mut motor = Motor::new(Feel { sway: 0.0, ..Feel::default() }, &mut rng);
            motor.ahead_cap = cap;
            let (decision, walked) = motor.walk(&mut corner(), &pose, 5.0, |_| true);
            assert_eq!(walked, Walked::OnTheWay);
            decision.turn.0.abs()
        };
        // Uncapped it heads for a point round the corner (a turn to the side); capped at 0.5 m the point is
        // the corner's, straight ahead.
        assert!(heading(Some(0.5)) < heading(None) * 0.5, "{} {}", heading(Some(0.5)), heading(None));
    }

    /// Hurt so it does not jump (a broken leg: about 8 s) as it comes to a
    /// jump of its route, the body presses nothing and spends no try; once
    /// it may jump it makes the jump at the first try, walking or aiming.
    #[test]
    fn a_hurt_body_presses_no_jump_and_spends_no_try() {
        let hurt_ticks = (8.0 / TICK_S) as usize;
        for aiming in [false, true] {
            let (mut route, ledge) = up_a_ledge(false);
            let climb = climb_ledge_hurt(
                &mut route,
                &ledge,
                Vec3::ZERO,
                yaw_of([1.0, 0.0]),
                hurt_ticks + 300,
                aiming,
                hurt_ticks,
            );
            let label = format!("aiming {aiming}");
            assert!(
                climb.presses.iter().all(|&(tick, _)| tick >= hurt_ticks),
                "{label}: {:?}",
                climb.presses
            );
            assert!(climb.up, "{label}: {:?}", climb.poses.last());
            assert_eq!(climb.presses.len(), 1, "{label}: {:?}", climb.presses);
            assert_eq!(climb.walked, Walked::Arrived, "{label}");
        }
    }

    /// A broken leg, or a hit within the stagger, holds the jump; a hit
    /// time of 0 or after the world's is none.
    #[test]
    fn a_broken_leg_or_a_fresh_hit_holds_the_jump() {
        let now = 10_000;
        assert!(Hurt::default().may_jump(now));
        assert!(Hurt::of([Some(0.5), Some(0.2)], 0, now).may_jump(now));
        assert!(!Hurt::of([Some(0.5), Some(0.0)], 0, now).may_jump(now));
        assert!(!Hurt::of([Some(0.0), None], 0, now).may_jump(now));
        assert!(Hurt::of([None, None], 0, now).may_jump(now));
        let hit = Hurt::of([Some(0.5); 2], now - 200, now);
        assert!(!hit.may_jump(now));
        assert!(hit.may_jump(now - 200 + HIT_JUMP_HOLD_MS));
        assert_eq!(Hurt::of([Some(0.5); 2], now + 50, now).last_hit_ms, None);
    }

    /// A jump whose way turns from the approach (Cologne's battery 6) is
    /// lined up: the body walks to the run-up, stands and turns to the
    /// jump's way, and presses at the take-off facing it; the jump gets it
    /// up at the first try, walking or aiming. No sprint lining up.
    #[test]
    fn a_turned_jump_is_lined_up_and_made_at_the_first_try() {
        for aiming in [false, true] {
            for turned in [false, true] {
                let (mut route, ledge) = up_a_ledge(turned);
                let climb = climb_ledge(
                    &mut route,
                    &ledge,
                    Vec3::ZERO,
                    yaw_of([1.0, 0.0]),
                    300,
                    aiming,
                );
                let label = format!("aiming {aiming}, turned {turned}");
                assert!(climb.up, "{label}: {:?}", climb.poses.last());
                assert_eq!(
                    climb.presses.len(),
                    1,
                    "{label}: {:?}, up at {:?}",
                    climb.presses,
                    climb.up_at
                );
                assert_eq!(climb.walked, Walked::Arrived, "{label}");
                let (_, at) = climb.presses[0];
                let way = yaw_of(ledge.way);
                assert!(
                    wrap(at.yaw - way).abs() <= JUMP_FACING + 1e-3,
                    "{label}: {at:?}"
                );
                let (dx, dz) = (at.feet.x - ledge.take_off.x, at.feet.z - ledge.take_off.z);
                let side = (dx * ledge.way[1] - dz * ledge.way[0]).abs();
                assert!(
                    side <= 0.3 && at.feet.distance_xz(ledge.take_off) <= 0.9,
                    "{label}: {at:?}"
                );
                // No sprint from 2.5 m before the take-off.
                for (pose, decision) in climb.poses.iter().zip(&climb.decisions) {
                    if pose.feet.distance_xz(ledge.take_off) <= LINE_UP_FROM_M {
                        assert_eq!(decision.keys & key::SPRINT, 0, "{label}: {pose:?}");
                    }
                }
            }
        }
    }

    /// A jump that does not get the body up is tried again only from the
    /// run-up, [`JUMP_TRIES`] times in all, each at the take-off; then it
    /// is given up on ([`Walked::JumpFailed`]). Walking or aiming. Held
    /// short of the take-off (a wall before it), it is never pressed.
    #[test]
    fn a_jump_that_did_not_get_up_is_tried_from_the_run_up_then_given_up() {
        for aiming in [false, true] {
            let (mut route, mut ledge) = up_a_ledge(false);
            ledge.gets_up = false;
            let climb = climb_ledge(
                &mut route,
                &ledge,
                Vec3::ZERO,
                yaw_of([1.0, 0.0]),
                800,
                aiming,
            );
            assert!(!climb.up);
            assert_eq!(
                climb.walked,
                Walked::JumpFailed(ledge.take_off),
                "aiming {aiming}"
            );
            let ticks: Vec<usize> = climb.presses.iter().map(|&(tick, _)| tick).collect();
            assert_eq!(
                ticks.len(),
                JUMP_TRIES as usize,
                "aiming {aiming}: {:?} then {:?}",
                climb.presses,
                &climb.poses[climb.poses.len().saturating_sub(5)..]
            );
            for &(_, at) in &climb.presses {
                assert!(
                    at.feet.x >= ledge.take_off.x - 0.9,
                    "aiming {aiming}: {at:?}"
                );
            }
            // Back at the run-up between two tries.
            for pair in ticks.windows(2) {
                let back = climb.poses[pair[0]..pair[1]]
                    .iter()
                    .any(|pose| pose.feet.x <= ledge.take_off.x - RUN_UP_M + AT_RUN_UP_M + 1e-3);
                assert!(
                    back,
                    "aiming {aiming}: {pair:?} {:?}",
                    climb.poses[pair[0]..pair[1]]
                        .iter()
                        .map(|pose| (pose.feet.x, pose.yaw))
                        .collect::<Vec<_>>()
                );
            }
            // A wall at x 3: never at the take-off, never pressed.
            let (mut route, mut ledge) = up_a_ledge(false);
            ledge.take_off.x = 3.0;
            ledge.face = 0.0;
            ledge.gets_up = false;
            let held = climb_ledge(
                &mut route,
                &ledge,
                Vec3::ZERO,
                yaw_of([1.0, 0.0]),
                200,
                aiming,
            );
            assert!(held.presses.is_empty(), "aiming {aiming}");
        }
    }

    /// A route's end up a ledge is not arrived at from under it; up it, it
    /// is.
    #[test]
    fn an_end_up_a_ledge_is_arrived_at_only_up_it() {
        let mut rng = Rng::new(19);
        let p = |x: f32, y: f32, jump| Waypoint {
            at: Vec3::new(x, y, 0.0),
            node: 0,
            jump,
            low: false,
        };
        let route = || {
            Route::new(vec![
                p(0.0, 0.0, false),
                p(4.6, 0.0, false),
                p(5.0, 0.9, true),
            ])
        };
        let mut motor = Motor::new(Feel::default(), &mut rng);
        let (_, walked) = motor.walk(
            &mut route(),
            &facing_x(Vec3::new(4.6, 0.0, 0.0)),
            5.0,
            |_| true,
        );
        assert_eq!(walked, Walked::OnTheWay);
        let (_, walked) = motor.walk(
            &mut route(),
            &facing_x(Vec3::new(5.0, 0.9, 0.0)),
            5.0,
            |_| true,
        );
        assert_eq!(walked, Walked::Arrived);
    }

    /// A world that shows the body still under the jump it pressed, after
    /// the reckoning took it up and the route on, has the route looked at
    /// from the jump's leg again: it is not off the route, and lines up.
    #[test]
    fn under_a_jump_that_did_not_get_up_the_route_is_taken_back() {
        let mut rng = Rng::new(23);
        let (mut route, _) = up_a_ledge(false);
        let mut motor = Motor::new(Feel::default(), &mut rng);
        // Pressed at the take-off.
        let mut pressed = false;
        for _ in 0..40 {
            let (decision, _) =
                motor.walk(&mut route, &facing_x(Vec3::new(4.6, 0.0, 0.0)), 5.0, |_| {
                    true
                });
            pressed |= decision.keys & key::JUMP != 0;
        }
        assert!(pressed);
        // Reckoned up and on.
        motor.walk(&mut route, &facing_x(Vec3::new(6.5, 0.9, 0.0)), 5.0, |_| {
            true
        });
        assert!(route.segment() >= 2);
        // A world: under it.
        let (_, walked) = motor.walk(&mut route, &facing_x(Vec3::new(4.7, 0.0, 0.0)), 5.0, |_| {
            true
        });
        assert_eq!(walked, Walked::OnTheWay);
        assert_eq!(route.segment(), 1);
        assert!(motor.lining_up());
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
            ..Default::default()
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
        // Keys change at the humans' rate or below (2.4 set changes a second
        // while moving), and the sprint does not flicker.
        let changes = decisions
            .windows(2)
            .filter(|w| w[0].keys != w[1].keys)
            .count();
        let seconds = decisions.len() as f32 * TICK_S;
        assert!(changes as f32 <= 2.4 * seconds, "{changes} in {seconds} s");
        let sprints = decisions
            .windows(2)
            .filter(|w| (w[0].keys ^ w[1].keys) & key::SPRINT != 0)
            .count();
        assert!(sprints <= 4, "{sprints}");
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
            ..Default::default()
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
                ..Default::default()
            };
            let (decision, walked) = if aiming {
                motor.walk_aiming(route, &pose, 5.0, |_| true, (pose.yaw, 0.0))
            } else {
                motor.walk(route, &pose, 5.0, |_| true)
            };
            assert_eq!(walked, Walked::OnTheWay, "{tick}: {pose:?}");
            if decision.keys & key::JUMP != 0 {
                assert!(motor.jumped().is_some());
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
            // The route up the ramp (a route of flat floor under it is off it).
            let mut ramp = straight().points().to_vec();
            for point in ramp.iter_mut() {
                point.at.y = point.at.x * 0.18;
            }
            let mut ramp = Route::new(ramp);
            let jumps = reckoned_to(
                &mut motor,
                &mut ramp,
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
            // 5.8 to 6.2, up 0.9 m from the lip), which the reckoning walks
            // to: the jumps the route makes, and no more (whatever the time
            // since its last). 3 s: two tries, not given up yet.
            let high = |x: f32, y: f32, jump| Waypoint {
                at: Vec3::new(x, y, 0.0),
                ..point(x, jump, false)
            };
            let ledge = || {
                Route::new(vec![
                    point(0.0, false, false),
                    high(5.8, 0.4, false),
                    high(6.2, 1.3, true),
                    high(30.0, 1.3, false),
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
                60,
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
                60,
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
