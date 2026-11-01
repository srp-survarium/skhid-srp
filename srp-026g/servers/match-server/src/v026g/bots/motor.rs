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
use super::link_time;
use super::path::{DropTruth, Leap, Route, RouteHeight};
use super::rng::Rng;
use super::tuning::{Metres, Positive, Span};
use crate::v026g::drive::key;

/// The look ahead is not shortened below this, metres.
const MIN_AHEAD_M: f32 = 0.5;

/// With [`Feel::unclear_aim`], how far back of the progress along the route the walk looks for a point it walks to in a
/// line, metres (the doorway behind the leg it has been taken to be on).
const BACK_AIM_M: f32 = 3.0;

/// What the walk steers at ([`Motor::aim_point`]).
#[derive(Clone, Copy, Debug, PartialEq)]
enum Aim {
    /// A point the body walks to in a line.
    Clear(Vec3),
    /// The nearest the look-ahead came, not one (the rule is off).
    Unclear(Vec3),
    /// No point of the route is one.
    None,
}

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
    /// Which of the route's floors behind a body count for where it is along it (`path::PastDrop`).
    pub past_drop: super::path::PastDrop,
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
    pub hop_rise_m: Span,
    /// … within this, metres along the way.
    pub hop_look_m: f32,
    /// No hop while the heading is still this far from the way (a turn,
    /// a sprint round a corner: the reckoning is off there anyway),
    /// radians.
    pub hop_max_turn: f32,
    /// At most one hop in this, seconds: a few seconds, never spammy
    /// (a player's once a minute is mostly over things the bot's route
    /// jumps at its jump links) …
    pub hop_every_s: Positive,
    /// … and none this soon after a jump of the route.
    pub hop_after_jump_s: f32,
    /// The lip guard ([`drive::lip_guard`]): the ticks a walk's keys count held when they are played forward
    /// to a stop on a lip's floors; 0 is off (the default is 2).
    pub lip_guard_ticks: u32,
    /// The lip guard's three gaps closed (the default; `false`: as it was): a body that let go of its keys at a rim and still
    /// glides is asked too (the stop it makes is played, braking if it goes over); the guard also looks at the floors
    /// [`body::RECKONING_M`] about the reckoned feet, and holds a walk that would fall from the feet that far to the side; and a
    /// bot it gave up (after holding its keys too often) is watched again once it has moved 0.5 m or changed its keys, not after a
    /// timer, and is asked for the glide meanwhile ([`drive::LipWatch::GaveUp`]).
    pub lip_guard_full: bool,
    /// The look-ahead of a walk on a lip's floors, metres; 0 is off (the walk looks as far as anywhere; the default is 0.8).
    pub lip_look_ahead_m: Metres,
    /// A body above a drop of the route that is within its look-ahead aims at the drop's landing, not past it
    /// (`false` looks as far as anywhere). The route's distance is across the ground, so the look-ahead
    /// of a drop 0.35 m across (B8, Vector Laboratory) reaches the legs that come back under the way in and the body
    /// turns round on the upper floor instead of stepping off.
    pub drop_aim: bool,
    /// When no point of the route ahead is one the body walks to in a line (it is on the other side of a wall from the
    /// route's next legs): `false` (the default; the bench's trains switch it on) steers at the unclear point all the
    /// same, into the wall; `true` steers at the farthest route point it does walk to in a line, up to [`BACK_AIM_M`]
    /// back, and plans again from where it stands with none (`Motor::aim_point`).
    pub unclear_aim: bool,
    /// A body on floors a zone refuses tests the lines of its walk over the refused floors of its first stretch, the way
    /// out ([`Tactics::clear_line_leaving`]); `false`: over allowed floors only, so with `unclear_aim` it finds no clear
    /// point in the zone's band and stands there (A136: four `zone_runs` stood in the zone with the defaults).
    pub zone_lines: bool,
    /// The height of the route's points the walk asks whether the body walks to ([`Motor::aim_point`], the run-up,
    /// the hop's rise) and the threat detour's exposure: `false` on the line between each leg's ends
    /// ([`Route::point_at`]), which over a hump of the floor (Cologne's deck: a long leg between two lower ends) lies
    /// up to 1.6 m under the floor, so that the grid finds no floor under the point (`line_ends` takes floors up to
    /// 1.0 m over it) and the walk sees no clear point, cuts its look-ahead and, with `unclear_aim`, steers back;
    /// `true` on the route's floor ([`Route::floor_point`]).
    pub floor_height_clear: bool,
    /// The highest jump link a route of a body that cannot sprint (a battery carrier) takes, metres: the
    /// client's standing jump (`STANDING_JUMP_M`, 0.998, PROVEN) and no sprint jump with a battery (PROVEN, 0x337040);
    /// `None` (the default; the bench's `SRP_BENCH_CARRIER_JUMP=on` sets the standing jump) plans it over every jump
    /// link of the grid (up to `Body::max_jump`: 0.998 with the clients' numbers, 1.05 with the exe's).
    pub carrier_jump_m: Option<Positive>,
    /// A planned drop (a lip's height or more) that a sprint would carry past the harmless fall is walked off: the
    /// sprint is let go of [`DROP_WALK_AHEAD_M`] before it, so the body is not carried into the air at 5 m/s over the
    /// steps below (`GridTuning::honest_drops` keeps a Drop link only where the body that walks off at a walk lands
    /// within the harmless fall; [`Route::sprint_unsafe_drop`] asks the same at the sprint's speed). A drop that is
    /// harmless at a sprint is sprinted off. Off: the sprint as the route's straight stretch asks.
    pub drop_walk: bool,
    /// A sprint gap link ([`Leap::SprintGap`]) is run up to at a sprint, the run turning onto the jump's way, and
    /// pressed at speed; off, it is made as a walking gap jump.
    pub sprint_gaps: bool,
    /// A sprint-band ledge jump (a link the grid labels a sprint jump, [`Feel::band_from_grid`]) is pressed only with the sprint held and the speed
    /// at [`SPRINT_PRESS_SPEED_MS`] or more, at the take-off as before it (the client makes the sprint jump only at
    /// the walk cap, 3.5 m/s, with sprint and forward held; a walk jump tops at 0.998 m); a body that arrives
    /// slower backs off for another run-up, each a try, and a link with no clear run of the grid's
    /// `mapdata::grid::SPRINT_RUN_UP_M` is given up at once so the route is planned again. The lining up of a sprint
    /// ledge or a sprint gap link ([`Feel::sprint_gaps`]) keeps the sprint, which the driver let go of for every
    /// jump's lining up. Off: the speed is asked only short of the take-off, a run-up as short as half
    /// of [`SPRINT_RUN_UP_M`] is taken, and the sprint is let go of.
    pub band_jump_gate: bool,
    /// The sprint-band gate ([`Feel::band_jump_gate`]) covers every link the grid labels a sprint jump
    /// ([`Route::sprint_ledge`]: a rise over the grid's jump cap, 0.998 m with the clients' numbers), not only those over
    /// [`WALK_JUMP_TOP_M`]. The motor then keeps no number of its own for "what a walk jump reaches": the grid's label is
    /// the one. A link of 1.0-1.09 m was labelled a sprint jump by the grid and pressed at a walk by the motor; the
    /// client's walk jump is the `on_site_fwd` clip, whose root tops at 0.998 m (PROVEN, the jump study), and the
    /// stand-in's too (0 of 53 such presses got up). Off: the gate starts at [`WALK_JUMP_TOP_M`].
    pub band_from_grid: bool,
    /// A landing from a jump of the route whose carried run (the planar velocity) goes more than this, degrees,
    /// from both the route's next leg and the way to the goal, or into a wall within its coast, is braked: the
    /// walk's keys are the ones against the run until it is slow (the owner's rule, 2026-10-27: a landing run is
    /// cancelled when it is against the bot's direction and path, and a wall is no direction). 0 is off (the counters ([`landings`]) count the landings either way). 60 is a GUESS to tune on the bench.
    pub landing_cancel_deg: f32,
    /// The landing cancel covers a planned drop too: its window starts when the body walks off the edge
    /// ([`Route::left_drop_edge`]), and the run it lands with is braked by the same rule (the owner's rule,
    /// 2026-10-27: after a jump or a drop). Off: drop landings are counted, never braked.
    pub landing_cancel_drops: bool,
    /// What every reader of the route's drops takes a drop to be: the lip guard's planned landings, the drop aim, the
    /// landing watch, the drop walk ([`DropTruth`]). [`DropTruth::Height`] is a waypoint a lip's height
    /// under the one before it, so a stair or a ramp counts and a Drop link of 0.8-1.0 m does not;
    /// [`DropTruth::Links`] (the default) is the planner's Drop links.
    pub drop_truth: DropTruth,
    /// A body in a fight or a rush (an enemy known near, or bound for the enemy base, at a run: [`Pose::urgent`]) climbs
    /// a low top of its route by the sprint lift, not a jump (the owner's rule, 2026-10-26: "jump only out of fights"):
    /// it runs into the face at a sprint with no press, and the client carries it up (a rise up to [`LIFT_MAX_RISE_M`],
    /// a top [`LIFT_TOP_M`] deep, a run of [`LIFT_RUN_UP_M`]); the lift that has not got it up [`LIFT_WAIT_S`] after the
    /// face is given up for the jump. A calm body jumps as before. Off (the default): it jumps in a fight too. Off because
    /// a body in a fight is the one the client takes no sprint of (it aims, fires, is hit): in the bench 114 of 136 faces
    /// were refused for that, and of the 21 lifts that reached the face 2 got up, the whole face (0.8-1.2 m) taking as
    /// long and giving up as often with the lift as without it. On to measure it again (`SRP_BENCH_TUNING`).
    pub fight_lift: bool,
    /// A body that runs keeps its sprint into the walking jump up a ledge whose top is deep enough for the sprint jump
    /// ([`mapdata::grid::SPRINT_TOP_M`] past the face, `Route::ledge_top`) and presses it on the move, instead of letting go
    /// of the sprint to line up at a walk (humans keep it: the sprint is held at 58 % of their jump presses and kept after
    /// 88 % of those). The sprint jump carries the body about 3 m past the face (the 8f rows, PROVEN), which that top holds.
    /// Off: a walking jump is lined up at a walk, as before.
    pub sprint_through: bool,
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
            past_drop: super::path::PastDrop::Window,
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
            hop_rise_m: Span::of(0.25, 0.9),
            hop_look_m: 1.5,
            hop_max_turn: 0.35,
            hop_every_s: Positive::of(4.0),
            hop_after_jump_s: 2.0,
            lip_guard_ticks: 2,
            lip_guard_full: true,
            lip_look_ahead_m: Metres::of(0.8),
            drop_aim: true,
            unclear_aim: true,
            zone_lines: true,
            floor_height_clear: false,
            carrier_jump_m: Some(Positive::of(0.998)),
            drop_walk: true,
            sprint_gaps: true,
            band_jump_gate: true,
            band_from_grid: true,
            landing_cancel_deg: 60.0,
            landing_cancel_drops: true,
            drop_truth: DropTruth::Links,
            fight_lift: false,
            sprint_through: true,
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
    /// The planar velocity (x, z), m/s: the way the body is carried.
    pub velocity: [f32; 2],
    /// Hurt so it does not jump now ([`Hurt::may_jump`]): the motor
    /// presses no jump and counts no try at one.
    pub no_jump: bool,
    /// Why the client takes no sprint jump now, if it does not (`base_player::can_sprint`, the sprint-jump RE §1.4):
    /// the cases the bot knows of. None: nothing known against it.
    pub sprint_block: Option<SprintBlock>,
    /// In a fight or a rush ([`Feel::fight_lift`]): the body runs and an enemy is known near it or it is bound for the enemy
    /// base. Set by the driver; a calm body's low tops are jumped.
    pub urgent: bool,
    /// What the legs are asked to do, from the play's pace (`Pace`), for the sprint rules.
    pub gait: Gait,
}

/// The gait the driver asks of the motor, one state for the legs' pace (not a pair of flags).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Gait {
    /// Walking: the sprint only on a long straight stretch ([`Feel::sprint_from_m`]) the walk is not aimed at.
    #[default]
    Walk,
    /// The legs run (`Pace::Run`): the sprint is the body's pace where the way allows ([`Feel::sprint_through`]).
    Run,
    /// Out of a blast (`Pace::Escape`): [`Gait::Run`], and the sprint on any route: no long straight stretch to wait for, no
    /// facing time, and not let go at a corner.
    Escape,
}

impl Gait {
    /// The legs run or escape: the sprint is the pace where the way allows.
    pub fn runs(self) -> bool {
        matches!(self, Self::Run | Self::Escape)
    }
}

/// Why the client takes no sprint (so no sprint jump) now: the conditions of `base_player::can_sprint` and the
/// hits' stagger that the bot knows of (the sprint-jump RE §1.4 and §1.8, PROVEN in the code).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SprintBlock {
    /// The weapon is aimed (the aim transition must be at exactly 0), or the walk is aimed.
    Aiming,
    /// The weapon is not idle: it fires, or a grenade is thrown.
    Busy,
    /// A reload is on.
    Reloading,
    /// Hit within [`STAGGER_SPRINT_MS`]: no sprint, and the speed built up again takes a run.
    Staggered,
    /// The reckoned bar is in the low-stamina state.
    Tired,
    /// A battery is carried.
    Carrying,
}

/// After a hit no sprint jump is taken for this long, ms: the stagger's 0.5 s (no sprint, speed cut by up to half;
/// PROVEN, the sprint-jump RE `FINDINGS.md` 1.8) and the run to build the speed again, "about 1.25 s after the last
/// hit" (LIKELY, the same section).
pub const STAGGER_SPRINT_MS: u32 = 1_250;

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
/// The sprint jump counters' bands start here, metres ([`sprint_jumps`]): a little under the standing jump's cap, so that the
/// links of the capped band (the cap to 1.05 m) are counted apart.
const SPRINT_BAND_FROM_M: f32 = 0.95;
/// A sprint jump is lined up from this far before its take-off along the route, metres (the run is 2 m and the
/// take-off's lead), and walked into from this far behind the take-off on its line: a sprint from rest is at speed
/// by 2 m (the 8e rows).
pub const SPRINT_LINE_UP_FROM_M: f32 = 4.5;
pub const SPRINT_RUN_UP_M: f32 = 2.0;
/// The sprint is let go of this far before the edge of a planned drop, metres (`Feel::drop_walk`): 0.6 s of a sprint.
pub const DROP_WALK_AHEAD_M: f32 = 3.0;
/// A drop crossed at a sprint is watched this long for a hard fall (`stuck::HARD_FALL_M`), seconds.
const DROP_SPRINTED_WATCH_S: f32 = 4.0;
/// A release of the sprint before a planned drop is followed this long, seconds, for the sprint to come back.
const DROP_RELEASE_WATCH_S: f32 = 10.0;

/// A release of the sprint before a planned drop, from its first tick till the sprint is back (the trace of #671).
#[derive(Clone, Copy, Debug)]
struct Release {
    start: u32,
    /// How far along the route the drop's edge was at the start, metres.
    dist: f32,
    edge: Option<Edge>,
}

/// The edge of a release's drop, once crossed: the tick, the height there and at the tick before, and the tick the
/// body landed (the height steady again, 0.5 m or more down).
#[derive(Clone, Copy, Debug)]
struct Edge {
    at: u32,
    y: f32,
    last_y: f32,
    landed: Option<u32>,
}
/// A sprint gap link is lined up (the view turned onto the jump's way) over the last this much of the route to its
/// take-off, metres: a 90 degree turn at the view's top turn speed (5 rad/s) takes 0.31 s, 1.6 m of a sprint, and the
/// grid guarantees 1.5 m of run.
pub const SPRINT_GAP_TURN_M: f32 = 2.0;
/// A sprint jump is pressed at this ground speed at least, m/s (a sprint is 5.0-5.1 m/s). The client's own bar is
/// lower: the walk cap, 3.5 m/s times (1 + the movement modifiers), 3.32-3.70 (PROVEN, `get_move_direction` and the
/// speed test at 0x33705A, the sprint-jump RE §1.2); this is the margin above it for the speed the body reckons.
const SPRINT_PRESS_SPEED_MS: f32 = 4.0;
/// The highest rise a jump pressed without the sprint jump was taken to reach, metres, under `Feel::band_from_grid` off: its
/// apex 1.0 and a settle onto the top, +1.08-1.09. The 8f woodbox rows (PROVEN: a +1.00 box mounted 12 of 12, a +1.36 face
/// 0 of 12, from a stand or a walk) read +1.06-1.08 on the top in every row, but the press row itself reads +0.07
/// before the clip's rise has begun (the jump event is at frame 7), so the +0.07 is an offset of the rows' height, not a
/// settle (LIKELY): the walk jump's top is the clip's apex, [`mapdata::grid::STANDING_JUMP_M`] 0.998. With the switch on
/// the motor reads the grid's label instead of this number.
pub const WALK_JUMP_TOP_M: f32 = 1.09;
/// The highest rise the sprint lift carries a body up, metres: the owner's sprint-step limit (2026-10-26), a margin under
/// the client's ceiling (the J0 box +1.00 lifted 19 of 20, a +1.12 face failed about 1 in 4 at any run-in, PROVEN).
pub const LIFT_MAX_RISE_M: f32 = 1.11;
/// The least depth of a top the lift is tried up, metres: a 0.33 m rail head-on lifted 0 of 20 (PROVEN); 0.35 is the owner's
/// bar (LIKELY).
pub const LIFT_TOP_M: f32 = 0.35;
/// The least clear run before the face for a lift, metres: the box +1.00 lifted 19 of 20 from 1, 2, 3 and 4 m (PROVEN).
/// The run is walked into from [`SPRINT_RUN_UP_M`] behind the take-off where that is clear, else from this.
pub const LIFT_RUN_UP_M: f32 = 1.0;
/// A lift is at its face from this near the take-off along the jump, metres (the client holds 0.15-0.3 s there before it
/// lifts, PROVEN).
const LIFT_FACE_M: f32 = 0.5;
/// A lift that has stood at the face this long, seconds, and is not up is given up for the jump (the lift is up 1.2-1.5 s
/// after the start of the run and the hold at the face is 0.3 s, LIKELY: a second is a margin); so is one whose sprint the
/// client no longer takes (a shot, an aim).
const LIFT_WAIT_S: f32 = 1.0;
/// A lift at its face that the walk has not asked for in this long is left: it is counted left, and the jump is the way up.
const LIFT_RESUME_S: f32 = 0.5;
/// A sprint-band press needs the body's velocity within this of the jump's way, degrees, and its speed along that way
/// at the press speed ([`SPRINT_PRESS_SPEED_MS`]): a body moving at 45 degrees to the way carries 3.5 m/s of 5 into the
/// face. Quetoo's bot jumps only when its velocity points within 30 degrees of the target node
/// (<https://github.com/jdolan/quetoo/pull/1108>).
const BAND_PRESS_ALIGN_DEG: f32 = 30.0;

/// Whether a body moving with `velocity` (x, z) is carried along the jump's `way` (a unit vector) fast enough for a
/// sprint-band press: its speed along the way is the press speed or more, and within [`BAND_PRESS_ALIGN_DEG`] of the way.
fn press_aligned(velocity: [f32; 2], way: [f32; 2]) -> bool {
    let along = velocity[0] * way[0] + velocity[1] * way[1];
    along >= SPRINT_PRESS_SPEED_MS && along >= velocity[0].hypot(velocity[1]) * BAND_PRESS_ALIGN_DEG.to_radians().cos()
}
/// A jump of the route this far on or nearer is priced by the bar ([`Motor::sprint_by_the_bar`]), metres.
const STAMINA_LOOKAHEAD_M: f32 = 40.0;
/// A bar at this or under is empty for the sprint: let go before the lockout, units.
const STAMINA_TIRED_AT: f32 = 3.0;
/// The landing window after a route jump's press, seconds: from the first ground the jump can land on (a
/// 1 m jump is in the air about 0.6 s) to the end of the coast (the J0 rows: a sprint coasts 1.3 m, a walk 0.45 m).
const LANDING_FROM_S: f32 = 0.6;
const LANDING_TO_S: f32 = 1.6;
/// The same window after the take-off of a planned drop: a 1.4 m fall lands in 0.53 s, a 5 m fall in 1.0 s.
const LANDING_DROP_FROM_S: f32 = 0.5;
/// Only a body carried at this speed at least is braked, m/s.
const LANDING_MIN_SPEED_MS: f32 = 2.5;
/// A run's coast, seconds of its speed (5 m/s coasts 1.3 m).
const LANDING_COAST_S: f32 = 0.26;
/// The route's next leg is looked at this far on, metres.
const LANDING_LEG_M: f32 = 1.5;
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
/// A walking jump pressed farther before its take-off than the usual lead ([`Route::ledge_lead`]) is walked into from this
/// much farther back than the press, metres.
const LEAD_RUN_UP_M: f32 = 0.4;
/// A body moving less than this a tick, m, is standing ([`Ahead::going`]).
const STANDING_M: f32 = 0.02;
/// The arrivals at a sprint-band take-off, short of what the client asks, backed off from before it is given up.
const BAND_REFUSALS: u32 = 2;
/// A band press is got up (strictly) if the body stands on the landing's floor, within [`BAND_UP_NEAR_M`] of it, this long,
/// seconds, after the press at most, with no second press before.
const BAND_UP_S: f32 = 1.5;
/// ... on the landing's floor: its feet within this of the landing's height, metres.
const BAND_UP_FLOOR_M: f32 = 0.2;
/// ... and this near the landing, metres (the jump carries it on past the face).
const BAND_UP_NEAR_M: f32 = 4.5;
/// A sprint-band press held off by a block that passes (aiming, a reload, a stagger, the weapon busy) waits this long,
/// seconds, without spending a try; then the arrival counts as one backed off from.
const BAND_HOLD_S: f32 = 3.0;
/// A face given up and picked again within this, seconds, is counted ([`band_jumps::repicked`]).
const BAND_REPICK_S: f32 = 10.0;
/// A body this near a band jump's take-off along its way, metres, and under [`STILL_MS`], is standing at its face.
const BAND_FACE_M: f32 = 0.6;
/// A body slower than this, m/s, is standing.
const STILL_MS: f32 = 0.3;
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

/// The lane a walk is in this tick (A175, `spacing.lane`): how far to the left the steering point moves (negative: the right), whether the body itself stands where a shift is safe
/// (or why not), and the test of a shifted point against the route's own (the same floor, off a lip and a fall; or why not).
pub struct LaneAim<'a> {
    pub offset_m: f32,
    pub body: Result<(), super::apart::Unsafe>,
    pub safe: &'a dyn Fn(Vec3, Vec3) -> Result<(), super::apart::Unsafe>,
}

/// A lane is kept off a gate, a jump or a drop within this far past the look-ahead, m, and off the route's last stretch of this length, m: the route's own line is the one that lines
/// up for a jump, a gate or an arrival.
const LANE_ROUTE_M: f32 = 4.0;
const LANE_END_M: f32 = 6.0;
/// The steering point is this far from the feet at least for a side to be known, m.
const LANE_MIN_AIM_M: f32 = 0.5;

/// One bot's hands.
#[derive(Clone, Debug)]
pub struct Motor {
    pub feel: Feel,
    /// Walk ticks in a row whose look-ahead was cut to a drop's landing ([`Feel::drop_aim`]).
    drop_aim_run: u32,
    /// The view's turning speed now (yaw, pitch), radians a second.
    rate: (f32, f32),
    walking: bool,
    sprinting: bool,
    /// Ticks faced the way well enough to sprint.
    facing_ticks: u32,
    /// The route jump being made ([`Motor::jump_ahead`]).
    jump_try: Option<JumpTry>,
    /// Whether this tick's walk lines up for a jump of the route, and for which kind (the caller lets go of crouch
    /// then: a crouched body does not jump).
    lining: Lining,
    /// The route jump this tick's decision pressed, if any.
    pressed: Option<RoutePress>,
    /// The last sprint-band jump given up: a pick of it again soon is counted.
    band_gave_up: Option<GaveUp>,
    /// The sprint-band press being followed for the strict got-up count ([`band_jumps::Slot::StrictUp`]).
    band_watch: Option<BandWatch>,
    /// The landing being watched: from the last press of a route jump or the last take-off of a planned drop.
    last_press: Option<LandingWatch>,
    /// The route's progress at the last walk, for the edge of a drop crossed since ([`Route::left_drop_edge`]).
    last_s: Option<f32>,
    /// A planned drop's edge crossed at a sprint (`Feel::drop_walk` on, so one safe at a sprint): the tick of the
    /// motor's clock and the feet's height there, till a fall of [`DROP_SPRINTED_WATCH_S`] has shown whether it was a
    /// hard one.
    sprinted_drop: Option<(u32, f32)>,
    /// A release of the sprint before a planned drop that is on (the trace of #671).
    release: Option<Release>,
    /// The sway's phase and the time, seconds, for it.
    phase: f32,
    t: f32,
    /// Standing: the heading looked at, and ticks till the next look.
    look: Option<(f32, u32)>,
    /// What this tick's lane did to the steering point ([`LaneAim`], A175): the bench counts it by reason.
    pub lane_use: super::apart::LaneUse,
    /// The turn's time constant for this tick when not the feel's (the
    /// reflex aims faster or slower by skill).
    pub turn_time: Option<f32>,
    /// The aim's turn rate this tick, yaw and pitch, radians a second (the
    /// reflex's, at a seen target: `CombatTuning::turn_feed`): a turn onto
    /// the aim follows it ([`Motor::aim_toward`]).
    pub track: Option<(f32, f32)>,
    /// This tick's route ends at an exact spot the play asked for (a goal
    /// within 0: Collect's walk to its stand): it is not arrived at from over
    /// it ([`ARRIVE_ABOVE_M`]), and it is walked to within this many metres
    /// (the play's own, else [`Feel::arrive_m`]). Cleared each tick before the
    /// walk sets it.
    pub exact_end: Option<f32>,
    /// This tick's walk looks no further ahead on the route than this, m (set by the caller each tick: on a
    /// lip's floors, where a line to a point further on cuts a corner over the rim).
    pub ahead_cap: Option<f32>,
    /// The bar as the bot reckons it ([`stamina`]), when `think.stamina` is on: the sprint is let go of at an empty
    /// bar and for a jump the bar could not pay.
    pub gauge: Option<super::stamina::Gauge>,
    /// Tired: the sprint was let go of at the empty bar and is not taken up again before the bar is back over
    /// [`stamina::CLEAR_ABOVE`](super::stamina::CLEAR_ABOVE).
    tired: bool,
    /// The bar this life's sprint is let go of at and taken up again over (`pace.reserve`, drawn at each spawn, [`Motor::reserve_for_life`]);
    /// none: [`STAMINA_TIRED_AT`] and [`stamina::CLEAR_ABOVE`](super::stamina::CLEAR_ABOVE).
    reserve: Option<(f32, f32)>,
    /// The sprint was let go of at the reserve this tick (the pace counts' cause of a burst's end).
    released_by_bar: bool,
    /// The take-off of the sprint segment that was short of the bar when first seen ([`stamina::counts`]).
    short_for: Option<Vec3>,
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
    /// This tick's walk aims at a point (`walk_aiming`): the sprint is let go of and the view is the aim's off the jump's line.
    aimed_walk: bool,
    /// The link being timed ([`link_time`]), and what the last tick's walk was doing with the body in its approach.
    links: super::link_time::Watch,
    link_why: link_time::Why,
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
    /// A sprint jump's outcome is counted ([`sprint_jumps`]).
    sprint_counted: bool,
    /// A sprint-band jump's own state ([`BandTry`]).
    band: BandTry,
    /// The sprint lift up this face ([`Feel::fight_lift`]).
    lift: LiftTry,
    /// What kept the lift from a face of its band is in the counts ([`lifts::refused`]).
    lift_noted: bool,
    /// The motor's tick the lift was last at this face: a lift not at it for [`TRY_KEPT_S`] is a new one.
    lift_seen: Option<u32>,
    /// What refused the lift at the face after it had reached it (the walk aims, the bar is spent...): the jump is the way
    /// up from then, and a lift not taken up again within [`LIFT_RESUME_S`] is counted left for this ([`lifts::left`]).
    lift_refused_at_face: Option<lifts::Refused>,
}

/// The sprint lift up a face ([`Feel::fight_lift`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum LiftTry {
    /// Not lifting: a jump is pressed as it always was.
    #[default]
    Open,
    /// Run into the face at a sprint with no press; `at_face` is the motor's tick it first stood at the face.
    Lifting { at_face: Option<u32> },
    /// The lift did not get up in [`LIFT_WAIT_S`]: the jump is pressed.
    GaveUp,
}

/// What a lining up is for ([`Motor::lining`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Lining {
    /// No jump of the route ahead.
    #[default]
    No,
    /// A walking jump: no crouch, no sprint.
    Walk,
    /// A sprint jump (a sprint ledge or a sprint gap link) under [`Feel::band_jump_gate`]: it is run up to at a sprint.
    Sprint,
}

/// Where a sprint-band ledge jump stands as to the speed the client asks of it ([`Feel::band_jump_gate`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum BandTry {
    /// Walked up to; no arrival short of the speed yet.
    #[default]
    Approaching,
    /// Backed off from `refused` arrivals short of the speed (or blocked), the last at the motor's tick `at`.
    BackedOff { refused: u32, at: u32 },
    /// Held off the face since the motor's tick `since`, by a block that passes (aiming, a reload, a stagger, the
    /// weapon busy): no try is spent for it until [`BAND_HOLD_S`] is up; `refused` arrivals were backed off from before.
    Held { since: u32, refused: u32 },
}

impl BandTry {
    fn refused(self) -> u32 {
        match self {
            Self::Approaching => 0,
            Self::BackedOff { refused, .. } | Self::Held { refused, .. } => refused,
        }
    }

    /// The tick of the last arrival backed off from or held off at its start.
    fn last(self) -> Option<u32> {
        match self {
            Self::Approaching => None,
            Self::BackedOff { at, .. } => Some(at),
            Self::Held { since, .. } => Some(since),
        }
    }
}

/// A sprint-band press followed for [`BAND_UP_S`]: the tick, the landing it is for and the take-off.
#[derive(Clone, Copy, Debug)]
struct BandWatch {
    at: u32,
    landing: Vec3,
    /// The feet and the take-off at the press, the speed and the sprint key then (the trace of #673).
    #[cfg(test)]
    from: Vec3,
    #[cfg(test)]
    take_off: Vec3,
    #[cfg(test)]
    speed: f32,
    #[cfg(test)]
    sprint: bool,
}

/// A sprint-band jump given up: its take-off and the tick.
#[derive(Clone, Copy, Debug)]
struct GaveUp {
    take_off: Vec3,
    at: u32,
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
    /// How it is pressed.
    leap: Leap,
    /// How the face is approached.
    approach: Approach,
    /// The jump's rise, metres; over [`WALK_JUMP_TOP_M`] only the sprint jump gets up ([`Feel::band_jump_gate`]).
    rise: f32,
    /// A walking jump up a short top is pressed this far before the take-off ([`Route::ledge_lead`]); 0 is the usual lead.
    lead_m: f32,
}

/// How the body approaches the face of a jump link: one of these, never a mixture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Approach {
    /// A walking jump: lined up at a walk and pressed near the take-off.
    Walk,
    /// A sprint jump of a ledge (the grid's label): walked into with sprint held, pressed at speed.
    Sprint,
    /// A sprint gap link: the run goes on while it turns onto the jump's way ([`SPRINT_GAP_TURN_M`]), no stopping to
    /// line up, and `before` is the route's distance to the take-off.
    SprintGap,
    /// The sprint lift: run into the face at a sprint, no press ([`Feel::fight_lift`]).
    Lift,
    /// A walking jump kept at a sprint ([`Feel::sprint_through`]).
    Through,
}

impl Approach {
    /// The sprint is held into the face.
    fn sprints(self) -> bool {
        self != Approach::Walk
    }

    /// The sprint gap link's run through the turn.
    fn runs_through(self) -> bool {
        self == Approach::SprintGap
    }
}

/// Why the motor pressed jump.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JumpBy {
    /// A jump link of the route ([`Motor::jump_key`]).
    Route,
    /// Caught on rising ground ([`Motor::hop_key`]).
    Hop,
}

/// The aim past a drop's landing when the look-ahead is cut to it, metres (the target is just past the landing's point).
const DROP_AIM_PAST_M: f32 = 0.05;
/// A run of cut ticks longer than this, ticks (2 s), counts as the body staying above the drop.
#[cfg(test)]
const DROP_AIM_LONG_TICKS: u32 = 40;

impl Motor {
    pub fn new(feel: Feel, rng: &mut Rng) -> Self {
        Self {
            feel,
            rate: (0.0, 0.0),
            walking: false,
            sprinting: false,
            facing_ticks: 0,
            jump_try: None,
            lining: Lining::No,
            pressed: None,
            band_gave_up: None,
            band_watch: None,
            last_press: None,
            last_s: None,
            sprinted_drop: None,
            release: None,
            phase: rng.range(0.0, 2.0 * PI),
            t: 0.0,
            look: None,
            lane_use: super::apart::LaneUse::Idle,
            turn_time: None,
            track: None,
            exact_end: None,
            drop_aim_run: 0,
            ahead_cap: None,
            gauge: None,
            tired: false,
            reserve: None,
            released_by_bar: false,
            short_for: None,
            clock: 0,
            trail: VecDeque::new(),
            way: [0.0, 0.0],
            hopped: None,
            jump: None,
            aimed_walk: false,
            links: super::link_time::Watch::default(),
            link_why: link_time::Why::Unknown,
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

    /// Whether the lift has reached its face and is held there ([`Feel::fight_lift`]).
    pub fn lift_at_face(&self) -> bool {
        self.jump_try.is_some_and(|t| {
            matches!(t.lift, LiftTry::Lifting { at_face: Some(_) }) && t.lift_seen == Some(self.clock)
        })
    }

    /// The route walked ended for this reason ([`link_time::Watch::route_ended`]).
    pub fn link_route_ended(&mut self, why: usize) {
        self.links.route_ended(why);
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
        self.lining != Lining::No
    }

    /// Whether a sprint-labelled link of `rise` is in the sprint band: over the walk jump's top, or ([`Feel::band_from_grid`])
    /// whatever the rise, the grid's label having said it is over the jump cap.
    fn over_walk_top(&self, rise: f32) -> bool {
        self.feel.band_from_grid || rise > WALK_JUMP_TOP_M
    }

    /// A new tick's decision begins.
    fn next_tick(&mut self) {
        self.clock = self.clock.wrapping_add(1);
        self.jump = None;
        self.pressed = None;
        self.lining = Lining::No;
        self.aimed_walk = false;
    }

    /// Whether this tick's lining up for a jump keeps the sprint (a sprint ledge under [`Feel::band_jump_gate`]): the
    /// driver lets go of it for a walking jump's lining up, but a sprint jump is run up to at a sprint.
    pub fn keeps_sprint(&self) -> bool {
        self.lining == Lining::Sprint
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

    /// With [`Feel::drop_aim`], the look-ahead cut to the landing of a drop within it that the body (`feet`) is still
    /// above: the target is the landing, and the walk goes off the edge. A run of ticks it applies to is counted
    /// (`drop_aim`), and a long one (the body stays above the drop) is the harm.
    fn drop_cap(&mut self, route: &Route, s: f32, feet: Vec3, ahead: f32) -> f32 {
        if !self.feel.drop_aim {
            return ahead;
        }
        let cap = route
            .next_drop(s, ahead, self.feel.drop_truth)
            .filter(|&(_, landing)| feet.y - landing.y >= self.feel.drop_truth.above_m())
            .map(|(along, _)| (along - s).max(0.0) + DROP_AIM_PAST_M);
        match cap {
            Some(cap) if cap < ahead => {
                self.drop_aim_run += 1;
                #[cfg(test)]
                drop_aim::tick(self.drop_aim_run);
                cap
            }
            _ => {
                self.drop_aim_run = 0;
                ahead
            }
        }
    }

    /// The point of the route the walk steers at: `ahead` metres on from `s`, nearer by 0.6 while the body does not walk
    /// to it in a line (`clear`), down to [`MIN_AHEAD_M`]. Where even that is not clear the body is on the wrong side of
    /// something (a wall between it and the route's next legs): with [`Feel::unclear_aim`] off it steers at the unclear
    /// point all the same (into the wall); on, at the farthest route point it does walk to in a line, within
    /// [`BACK_AIM_M`] back of `s`, and with none the route is lost to it ([`Aim::None`]: off the route, planned again from
    /// where it stands).
    fn aim_point(&self, route: &Route, s: f32, ahead: f32, clear: &impl Fn(Vec3) -> bool) -> Aim {
        let (target, found) = self.look_ahead(route, s, ahead, clear, self.heights());
        if found {
            return Aim::Clear(target);
        }
        #[cfg(test)]
        counts::unclear();
        if !self.feel.unclear_aim {
            return Aim::Unclear(target);
        }
        // The heights of the line between the legs' ends can be what makes the points unclear (a hump of the floor
        // the leg crosses): the look-ahead again at the floor's.
        if self.heights() == RouteHeight::Chord {
            let (target, found) = self.look_ahead(route, s, ahead, clear, RouteHeight::Floor);
            if found {
                #[cfg(test)]
                counts::unclear_floor();
                return Aim::Clear(target);
            }
        }
        // The unclear point is no wall when the route itself leaves the ground within reach (a jump, a drop): a walk in a line
        // to the far side of a ledge does not exist (a 1.3-2 m drop: `clear` is right), and the route goes over it. The body
        // steers on as with the rule off (the drop's own aim and the lip guard act at the edge); stepping back along the
        // route would be away from the edge it is to take.
        if route.link_ahead(s, ahead.max(self.feel.look_ahead_m) + MIN_AHEAD_M, mapdata::Body::client().step) {
            #[cfg(test)]
            counts::unclear_link();
            return Aim::Unclear(target);
        }
        let mut back = MIN_AHEAD_M;
        while back > -BACK_AIM_M {
            back -= 0.5;
            let point = route.at((s + back).max(0.0), RouteHeight::Floor);
            if clear(point) {
                #[cfg(test)]
                counts::unclear_back();
                return Aim::Clear(point);
            }
        }
        #[cfg(test)]
        counts::unclear_off();
        Aim::None
    }

    /// How high the route's points are taken for the walk's tests of the ground ([`Feel::floor_height_clear`]).
    pub(super) fn heights(&self) -> RouteHeight {
        if self.feel.floor_height_clear {
            RouteHeight::Floor
        } else {
            RouteHeight::Chord
        }
    }

    /// The point of the route `ahead` metres on from `s`, nearer by 0.6 while the body does not walk to it in a
    /// line, down to [`MIN_AHEAD_M`]; whether the one it came to is one.
    fn look_ahead(&self, route: &Route, s: f32, mut ahead: f32, clear: &impl Fn(Vec3) -> bool, height: RouteHeight) -> (Vec3, bool) {
        let mut target = route.at(s + ahead, height);
        while ahead > MIN_AHEAD_M && !clear(target) {
            #[cfg(test)]
            {
                counts::cut();
                if height == RouteHeight::Chord && clear(route.at(s + ahead, RouteHeight::Floor)) {
                    counts::cut_false();
                }
            }
            ahead *= 0.6;
            target = route.at(s + ahead, height);
        }
        (target, clear(target))
    }

    /// The steering point `target` moved over to the lane, when the route is plain ahead of the body and the shifted point is one the body walks to in a line and a floor the route
    /// chose (`lane.safe`); else `target` itself, with the reason in [`Self::lane_use`].
    #[allow(clippy::too_many_arguments)]
    fn laned(&mut self, route: &Route, s: f32, ahead: f32, feet: Vec3, target: Vec3, lane: LaneAim<'_>, clear: &impl Fn(Vec3) -> bool) -> Vec3 {
        use super::apart::LaneUse;
        let reach = ahead.max(self.feel.look_ahead_m) + LANE_ROUTE_M;
        if route.length() - s < LANE_END_M
            || route.gate_within(s, reach).is_some()
            || route.link_ahead(s, reach, mapdata::Body::client().step)
        {
            self.lane_use = LaneUse::RouteBusy;
            return target;
        }
        let (dx, dz) = (target.x - feet.x, target.z - feet.z);
        let len = dx.hypot(dz);
        if len < LANE_MIN_AIM_M {
            self.lane_use = LaneUse::RouteBusy;
            return target;
        }
        // The left of the way is a quarter turn: (-dz, dx).
        let shifted = Vec3::new(target.x - dz / len * lane.offset_m, target.y, target.z + dx / len * lane.offset_m);
        if let Err(why) = lane.body.and_then(|()| (lane.safe)(shifted, target)) {
            self.lane_use = LaneUse::UnsafeFloor(why);
            return target;
        }
        if !clear(shifted) {
            self.lane_use = LaneUse::NoRoom;
            return target;
        }
        self.lane_use = LaneUse::Applied;
        shifted
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
        self.walk_in_lane(route, pose, off_m, clear, None)
    }

    /// [`Self::walk`] on a lane (A175, `spacing.lane`): the point the walk steers at is moved sideways by `lane.offset_m` where that is safe, else it is the route's own.
    pub fn walk_in_lane(
        &mut self,
        route: &mut Route,
        pose: &Pose,
        off_m: f32,
        clear: impl Fn(Vec3) -> bool,
        lane: Option<LaneAim<'_>>,
    ) -> (Decision, Walked) {
        self.next_tick();
        self.look = None;
        self.lane_use = super::apart::LaneUse::Idle;
        self.under_jump(route, pose.feet);
        self.follow_band_press(pose.feet);
        self.follow_lift(pose.feet);
        self.links.tick(self.clock, pose.feet, pose.speed, self.link_why);
        route.set_below_walk(self.feel.off_walk_below_m);
        route.set_past_drop(self.feel.past_drop);
        let (s, off) = route.progress(pose.feet);
        if self.last_s.replace(s).is_some_and(|before| route.left_drop_edge(before, s, self.feel.drop_truth)) {
            self.last_press = Some(LandingWatch { at: self.clock, counted: false, from: LandingFrom::Drop });
            if self.feel.drop_walk && self.sprinting {
                self.sprinted_drop = Some((self.clock, pose.feet.y));
            }
            if let Some(release) = self.release.as_mut()
                && release.edge.is_none()
            {
                release.edge = Some(Edge { at: self.clock, y: pose.feet.y, last_y: pose.feet.y, landed: None });
                drop_walks::edge_after(self.clock.wrapping_sub(release.start) as f32 * TICK_S, release.dist);
            }
        }
        if let Some(edge) = self.release.as_mut().and_then(|release| release.edge.as_mut()) {
            if edge.landed.is_none() && edge.y - pose.feet.y >= 0.5 && (edge.last_y - pose.feet.y).abs() < 0.03 {
                edge.landed = Some(self.clock);
            }
            edge.last_y = pose.feet.y;
        }
        if let Some((at, y)) = self.sprinted_drop {
            if y - pose.feet.y >= super::stuck::HARD_FALL_M {
                self.sprinted_drop = None;
                drop_walks::hard_fall_sprinted();
            } else if self.since(at) > DROP_SPRINTED_WATCH_S {
                self.sprinted_drop = None;
            }
        }
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
                ahead = self.drop_cap(route, s, pose.feet, ahead);
                // A door gate on the way is steered at, not past (`Route::door_centred`).
                if let Some(gate) = route.gate_within(s, ahead) {
                    ahead = (gate - s).max(MIN_AHEAD_M);
                }
                match self.aim_point(route, s, ahead, &clear) {
                    Aim::Clear(target) => match lane {
                        Some(lane) => self.laned(route, s, ahead, pose.feet, target, lane, &clear),
                        None => target,
                    },
                    Aim::Unclear(target) => target,
                    // No point of the route is one the body walks to in a line: it is off it.
                    Aim::None => return (Decision::default(), Walked::Off(off_m.max(off) + 0.01)),
                }
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
        let run_through = jump.is_some_and(|jump| jump.approach.runs_through());
        if lined && run_through {
            // A sprint gap link: the run goes on while the view turns onto the jump's way.
            self.walking = true;
        } else if lined {
            self.walking = still < LINE_UP_GO;
        } else if self.walking && still > self.feel.stop_turn {
            self.walking = false;
        } else if !self.walking && still < self.feel.go_turn {
            self.walking = true;
        }
        self.link_why = if jump.is_none() {
            link_time::Why::Unknown
        } else if matches!(pose.sprint_block, Some(SprintBlock::Aiming | SprintBlock::Busy | SprintBlock::Reloading | SprintBlock::Staggered)) {
            link_time::Why::Fight
        } else if !lined {
            link_time::Why::Approach
        } else if !self.walking {
            link_time::Why::Turning
        } else {
            link_time::Why::Lined
        };
        // Sprint on a long straight stretch, facing the way.
        let (straight, _) = route.straight_ahead(s, self.feel.corner);
        if self.walking && still < self.feel.sprint_facing {
            self.facing_ticks += 1;
        } else {
            self.facing_ticks = 0;
        }
        // Out of a blast the sprint needs neither the facing time nor a long straight (the walks are 2-10 m), only the way
        // faced as for any sprint.
        let escaping = pose.gait == Gait::Escape;
        let settled = if escaping {
            self.walking && still < self.feel.sprint_facing
        } else {
            self.facing_ticks as f32 * TICK_S >= self.feel.sprint_after_s
        };
        if !self.sprinting && settled && (escaping || straight >= self.feel.sprint_from_m) {
            self.sprinting = true;
        } else if self.sprinting
            && (!self.walking
                || (straight < self.feel.sprint_until_m && !escaping)
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
        // The reckoned bar: let go of the sprint at the empty bar, and short of the bar for a jump ahead.
        self.sprint_by_the_bar(route, s);
        // A planned drop is a link too: timed from the lining up to it.
        if let Some(&landing) = route.drop_landings(s, DROP_WALK_AHEAD_M, self.feel.drop_truth).first() {
            let link = link_time::Link { kind: link_time::Kind::Drop, take_off: pose.feet, landing, top: None };
            if self.links.idle() {
                self.links.begin(self.clock, link, pose.feet, pose.speed);
            }
        }
        // A planned drop just ahead is walked off, not sprinted into the air, unless its flight at a sprint comes down
        // within the harmless fall as well.
        if self.feel.drop_walk && self.sprinting && !route.drop_landings(s, DROP_WALK_AHEAD_M, self.feel.drop_truth).is_empty() {
            let record = route.drop_record(s, DROP_WALK_AHEAD_M, self.feel.drop_truth);
            match record {
                // A drop a sprint would not land within the harmless fall, or one nothing is known of.
                super::path::DropRecord::Unsafe | super::path::DropRecord::Unknown => {
                    self.sprinting = false;
                    drop_walks::released(record);
                    if self.release.is_none() {
                        let dist = route.next_drop_edge_along(s, DROP_WALK_AHEAD_M, self.feel.drop_truth).map_or(0.0, |edge| (edge - s).max(0.0));
                        self.release = Some(Release { start: self.clock, dist, edge: None });
                        drop_walks::episode(record);
                    }
                }
                // A drop the sprint lands as well.
                super::path::DropRecord::Safe => drop_walks::skipped_as_safe(),
                // A descent by walk links only (a ramp, stairs): no fall, the sprint goes on.
                super::path::DropRecord::Walk | super::path::DropRecord::None => drop_walks::skipped_as_descent(),
            }
        }
        if let Some(release) = self.release {
            match release.edge {
                None if self.sprinting => {
                    drop_walks::unresolved(false);
                    self.release = None;
                }
                Some(edge) if self.sprinting => {
                    let from = |at: u32| self.clock.wrapping_sub(at) as f32 * TICK_S;
                    drop_walks::resumed(from(edge.at), edge.landed.map(from));
                    self.release = None;
                }
                _ if self.since(release.start) > DROP_RELEASE_WATCH_S => {
                    drop_walks::unresolved(release.edge.is_some());
                    self.release = None;
                }
                _ => {}
            }
        }
        // A sprint jump is walked into at a sprint, on its line and facing its way, unless the bar is spent: a tired body
        // walks (the keys held through a lockout lengthen it, and the bar comes back walking).
        if let Some(j) = jump
            && j.approach.sprints()
            && ((j.lined && self.walking && still < LINE_UP_GO) || (j.approach.runs_through() && self.walking))
            && !pose.no_jump
        {
            if self.tired {
                super::stamina::counts::sprint_jump_kept_walking();
            } else {
                self.sprinting = true;
            }
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
            keys |= self.jump_key(&jump, still, pose, false);
        }
        // A landing run against the way is braked, not walked out.
        let mut braked = false;
        if let Some(brake) = self.landing_brake(route, pose, s, &clear) {
            keys = (keys & !(key::FORWARD | key::BACK | key::LEFT | key::RIGHT | key::SPRINT)) | brake;
            braked = true;
        }
        // A lift at its face that the motor itself asks no sprint or forward of: the bench's cause (`lifts::motor_off`).
        if self.lift_at_face() && keys & (key::SPRINT | key::FORWARD) != (key::SPRINT | key::FORWARD) {
            lifts::motor_off(if braked {
                lifts::MotorOff::Brake
            } else if !self.walking {
                lifts::MotorOff::Standing
            } else if pose.no_jump {
                lifts::MotorOff::NoJump
            } else if self.tired {
                lifts::MotorOff::Tired
            } else if keys & key::SPRINT == 0 {
                lifts::MotorOff::NotSprinting
            } else {
                lifts::MotorOff::NoForward
            });
        }
        // A hop goes the way the keys do: only with forward held.
        let may = keys & key::FORWARD != 0 && keys & key::BACK == 0 && !pose.no_jump;
        keys |= self.hop_key(route, s, pose.feet, unit(dx, dz), may);
        (Decision { keys, turn }, Walked::OnTheWay)
    }

    /// This life's bar reserve, drawn once at the spawn from the tuning's spans (none without them).
    pub fn reserve_for_life(&mut self, reserve: Option<&super::tuning::Reserve>, rng: &mut Rng) {
        self.reserve = reserve.map(|r| {
            let (stop_lo, stop_hi) = r.stop.get();
            let (gap_lo, gap_hi) = r.gap.get();
            let stop = rng.range(stop_lo, stop_hi);
            (stop, stop + rng.range(gap_lo, gap_hi))
        });
    }

    /// Whether the sprint was let go of this tick at the life's reserve (the bar), not at the map.
    pub fn released_by_bar(&self) -> bool {
        self.released_by_bar
    }

    /// The sprint by the reckoned bar ([`Motor::gauge`]): let go of at an empty bar (a lockout lasts as long as the
    /// sprint keys are held) and not taken up again before the bar is back over [`stamina::CLEAR_ABOVE`]; and let go of
    /// when a jump of the route within [`STAMINA_LOOKAHEAD_M`] would find the bar short of what the sprint there, its
    /// run-up and the jump cost ([`stamina::Reckoning::covers`]): walking, the bar comes back.
    fn sprint_by_the_bar(&mut self, route: &Route, s: f32) {
        self.released_by_bar = false;
        let Some(gauge) = self.gauge else {
            self.tired = false;
            self.short_for = None;
            return;
        };
        let (stop, start) = self.reserve.unwrap_or((STAMINA_TIRED_AT, super::stamina::CLEAR_ABOVE));
        if gauge.low || gauge.value <= stop {
            self.tired = true;
        } else if gauge.value >= start {
            self.tired = false;
        }
        if !self.sprinting {
            return;
        }
        if self.tired {
            self.sprinting = false;
            self.released_by_bar = true;
            super::stamina::counts::released_low();
            return;
        }
        let Some((k, before)) = route.next_jump(s) else {
            self.short_for = None;
            return;
        };
        if before > STAMINA_LOOKAHEAD_M {
            return;
        }
        let points = route.points();
        let take_off = points[k - 1].at;
        let sprint_jump = route.sprint_ledge(k);
        // The run to the take-off is sprinted too: a sprint-band ledge's run-up, and a sprint gap link's own run.
        let gap = self.feel.sprint_gaps && matches!(points[k].jump, Some(Leap::SprintGap(_)));
        let run_up = if sprint_jump {
            SPRINT_RUN_UP_M
        } else if gap {
            mapdata::grid::gap::SPRINT_RUN_UP_M
        } else {
            0.0
        };
        let reckoning = super::stamina::Reckoning::of(gauge);
        if !reckoning.covers(before.max(0.0), run_up) {
            self.sprinting = false;
            super::stamina::counts::saved((TICK_S * 1000.0) as u32);
            if self.short_for.is_none_or(|at| at.distance_xz(take_off) > 0.3) {
                self.short_for = Some(take_off);
                super::stamina::counts::short_segment();
                if gap && reckoning.covers(before.max(0.0), 0.0) {
                    super::stamina::counts::gap_run_short();
                }
            }
        }
    }

    /// The keys against a landing run ([`Feel::landing_cancel_deg`]): in the window after a route jump's press
    /// ([`LANDING_FROM_S`] .. [`LANDING_TO_S`]), the body carried at [`LANDING_MIN_SPEED_MS`] or more, whose run
    /// goes into a wall within its coast ([`LANDING_COAST_S`]) or away from both the route's next leg
    /// ([`LANDING_LEG_M`] on) and the goal. Counted once per jump ([`landings`]), braked only when the feel
    /// says so. None when there is nothing to cancel.
    fn landing_brake(
        &mut self,
        route: &Route,
        pose: &Pose,
        s: f32,
        clear: &impl Fn(Vec3) -> bool,
    ) -> Option<u32> {
        let LandingWatch { at, counted, from } = self.last_press?;
        let t = self.since(at);
        let first = match from {
            LandingFrom::Jump => LANDING_FROM_S,
            LandingFrom::Drop => LANDING_DROP_FROM_S,
        };
        if !(first..LANDING_TO_S).contains(&t) {
            return None;
        }
        let [vx, vz] = pose.velocity;
        let speed = vx.hypot(vz);
        if speed < LANDING_MIN_SPEED_MS {
            return None;
        }
        let carried = yaw_of([vx, vz]);
        let off = |to: Vec3| wrap(yaw_of([to.x - pose.feet.x, to.z - pose.feet.z]) - carried).abs();
        let against = off(route.point_at(s + LANDING_LEG_M)).min(off(route.end()));
        let coast = LANDING_COAST_S * speed;
        let blocked = !clear(pose.feet + Vec3::new(vx / speed * coast, 0.0, vz / speed * coast));
        let limit = self.feel.landing_cancel_deg.to_radians();
        let may = match from {
            LandingFrom::Jump => true,
            LandingFrom::Drop => self.feel.landing_cancel_drops,
        };
        let cancel = may && limit > 0.0 && (blocked || against > limit);
        if !counted {
            self.last_press = Some(LandingWatch { at, counted: true, from });
            landings::seen(from, against, blocked, cancel);
        }
        cancel.then(|| keys_toward(wrap(carried + PI - pose.yaw)))
    }

    /// Whether the route is walked to its end: within [`Feel::arrive_m`] of
    /// it (an exact spot's own radius, [`Motor::exact_end`]), and not under it ([`ARRIVE_BELOW_M`]: an end up a ledge is jumped
    /// to; [`ARRIVE_ON_LANDING_M`] when the last leg is a jump: on its
    /// landing) nor, for an exact spot's end ([`Motor::exact_end`]), over it
    /// ([`ARRIVE_ABOVE_M`]).
    fn arrived(&self, route: &Route, s: f32, feet: Vec3) -> bool {
        let end = route.end();
        let below = if route.points().last().is_some_and(|p| p.jump.is_some()) {
            ARRIVE_ON_LANDING_M
        } else {
            ARRIVE_BELOW_M
        };
        let radius = self.exact_end.unwrap_or(self.feel.arrive_m);
        (route.length() - s <= radius || feet.distance_xz(end) <= radius)
            && end.y - feet.y <= below
            && (self.exact_end.is_none() || feet.y - end.y <= ARRIVE_ABOVE_M)
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
        self.aimed_walk = true;
        self.look = None;
        self.sprinting = false;
        self.facing_ticks = 0;
        self.under_jump(route, pose.feet);
        self.follow_band_press(pose.feet);
        self.follow_lift(pose.feet);
        self.links.tick(self.clock, pose.feet, pose.speed, self.link_why);
        route.set_below_walk(self.feel.off_walk_below_m);
        route.set_past_drop(self.feel.past_drop);
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
                let ahead = self.feel.look_ahead_m.min(self.ahead_cap.unwrap_or(f32::INFINITY));
                match self.aim_point(route, s, ahead, &clear) {
                    Aim::Clear(target) | Aim::Unclear(target) => target,
                    Aim::None => {
                        let turn = self.aim_toward(pose, aim.0, aim.1);
                        return (Decision { keys: 0, turn }, Walked::Off(off_m.max(off) + 0.01));
                    }
                }
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
            keys |= self.jump_key(&jump, still, pose, true);
        }
        // A landing run against the way is braked, not walked out.
        if let Some(brake) = self.landing_brake(route, pose, s, &clear) {
            keys = (keys & !(key::FORWARD | key::BACK | key::LEFT | key::RIGHT | key::SPRINT)) | brake;
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
        let Some((k, before_route)) = route.next_jump(s) else {
            self.links.lost(link_time::Lost::NoJump);
            return None;
        };
        let points = route.points();
        let (take_off, landing) = (points[k - 1].at, points[k].at);
        let leap = points[k].jump.unwrap_or(Leap::Ledge);
        let sprint_ledge = route.sprint_ledge(k);
        let sprint_gap = self.feel.sprint_gaps && matches!(leap, Leap::SprintGap(_));
        let sprint = sprint_ledge || sprint_gap;
        let band = self.feel.band_jump_gate && sprint_ledge && self.over_walk_top(landing.y - take_off.y);
        // A body in a fight or a rush climbs a low top by the sprint lift ([`Feel::fight_lift`]); the face, the run and
        // the bar are asked below.
        let lift_open = self.feel.fight_lift
            && pose.urgent
            && !pose.no_jump
            && matches!(leap, Leap::Ledge | Leap::SprintLedge)
            && landing.y - take_off.y <= LIFT_MAX_RISE_M;
        if before_route > if sprint || lift_open { SPRINT_LINE_UP_FROM_M } else { LINE_UP_FROM_M } {
            self.links.lost(link_time::Lost::Far);
            return None;
        }
        let way = unit(landing.x - take_off.x, landing.z - take_off.z)?;
        let feet = pose.feet;
        if is_up(feet, take_off, landing) {
            if sprint_ledge
                && let Some(t) = self.jump_try.as_mut()
                && t.tries > 0
                && !t.sprint_counted
                && (t.take_off - take_off).length() < 0.3
                && (t.landing - landing).length() < 0.3
            {
                t.sprint_counted = true;
                sprint_jumps::up(landing.y - take_off.y);
                band_jumps::face(take_off, landing.y - take_off.y, band_jumps::Event::Up);
            }
            if sprint_gap
                && let Some(t) = self.jump_try.as_mut()
                && t.tries > 0
                && !t.sprint_counted
                && (t.take_off - take_off).length() < 0.3
                && (t.landing - landing).length() < 0.3
            {
                t.sprint_counted = true;
                sprint_gaps::across();
            }
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
                && t.band.last().is_none_or(|at| self.since(at) < TRY_KEPT_S)
                && t.lift_seen.is_none_or(|at| self.since(at) < TRY_KEPT_S)
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
                sprint_counted: false,
                band: BandTry::Approaching,
                lift: LiftTry::Open,
                lift_noted: false,
                lift_seen: None,
                lift_refused_at_face: None,
            },
        };
        let in_the_air = jump.pressed.is_some_and(|at| self.since(at) < CARRY_S);
        self.lining = if self.feel.band_jump_gate && (sprint_ledge || sprint_gap) { Lining::Sprint } else { Lining::Walk };
        if band && jump.tries == 0 && jump.band == BandTry::Approaching
            && let Some(gave_up) = self.band_gave_up
            && (gave_up.take_off - take_off).length() < 0.3
            && self.since(gave_up.at) < BAND_REPICK_S
        {
            band_jumps::repicked();
            self.band_gave_up = None;
        }
        if jump.tries >= JUMP_TRIES && !in_the_air {
            if sprint_ledge && !jump.sprint_counted {
                jump.sprint_counted = true;
                sprint_jumps::gave_up(landing.y - take_off.y);
                band_jumps::face(take_off, landing.y - take_off.y, band_jumps::Event::GaveUp);
                if band {
                    self.band_gave_up = Some(GaveUp { take_off, at: self.clock });
                }
            }
            if sprint_gap && !jump.sprint_counted {
                jump.sprint_counted = true;
                sprint_gaps::gave_up();
            }
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
                leap,
                approach: if sprint { Approach::Sprint } else { Approach::Walk },
                rise: landing.y - take_off.y,
                lead_m: 0.0,
            });
        }
        let lead_m = route.ledge_lead(k).unwrap_or(0.0);
        let at = |m: f32| Vec3::new(take_off.x - way[0] * m, take_off.y, take_off.z - way[1] * m);
        // The sprint lift: up this face, with the top deep enough, a clear run before it and a body the client lets sprint.
        let mut lifting = lift_open && jump.lift != LiftTry::GaveUp;
        if lifting {
            let why = if !route.ledge_top(k).is_some_and(|depth| depth >= LIFT_TOP_M) {
                Some(lifts::Refused::TopShort)
            } else if pose.sprint_block.is_some() || self.aimed_walk || self.tired {
                // The client takes no sprint now (aiming, firing, a reload, a hit, the bar, a battery), or the walk aims (it
                // lets go of the sprint) or the bar is spent for the motor: the lift needs the sprint held against the face
                // for the client's hold (the bench: 6 % of the ticks a lift stood at its face held it), so the jump is the
                // way up. A fight's reflex fires most ticks.
                Some(lifts::Refused::Blocked)
            } else if !matches!(jump.lift, LiftTry::Lifting { at_face: Some(_) }) && !clear(at(LIFT_RUN_UP_M)) {
                // Asked before the face only: `clear` is the body's line to the point, and from the face the run is run (the
                // bench: half the lifts left at their face were left for this).
                Some(lifts::Refused::NoRunUp)
            } else {
                None
            };
            if let Some(why) = why {
                lifting = false;
                if matches!(jump.lift, LiftTry::Lifting { at_face: Some(_) }) {
                    jump.lift_refused_at_face = Some(why);
                } else if !jump.lift_noted {
                    jump.lift_noted = true;
                    lifts::refused(why);
                }
            } else {
                jump.lift_refused_at_face = None;
            }
        }
        // A running body keeps its sprint into a walking jump up a deep top: the sprint jump carries it up and on.
        let through = self.feel.sprint_through
            && !lifting
            && pose.gait.runs()
            && !pose.no_jump
            && pose.sprint_block.is_none()
            && leap == Leap::Ledge
            && route.ledge_top(k).is_some_and(|depth| depth >= mapdata::grid::SPRINT_TOP_M)
            && clear(at(SPRINT_RUN_UP_M));
        let sprint = sprint || lifting || through;
        if lifting || through {
            self.lining = Lining::Sprint;
        }
        // A gap link is pressed at its own point: the run-up starts
        // [`mapdata::grid::gap::GAP_RUN_UP_M`] behind it, or the jump is
        // walked into from wherever the line is clear (it presses there).
        let run_up = match leap {
            // A lift is run up to from the sprint jump's run where it is clear, else from the least the lift needs.
            Leap::Ledge | Leap::SprintLedge if lifting || through => {
                [SPRINT_RUN_UP_M, LIFT_RUN_UP_M].into_iter().find(|&m| clear(at(m))).unwrap_or(LIFT_RUN_UP_M)
            }
            // A band jump runs up over the grid's run (the floor it proved straight behind the take-off), not less. The
            // point of the run may be out of sight from where the body is (it arrives along the route, round a corner):
            // then the grid's run is taken and the route is followed to the take-off; only a body that has backed off
            // from the take-off and still has no clear run behind it gives the jump up, so the route is planned again.
            Leap::Ledge | Leap::SprintLedge if band => {
                let found = [SPRINT_RUN_UP_M, mapdata::grid::SPRINT_RUN_UP_M].into_iter().find(|&m| clear(at(m)));
                if found.is_none() && jump.band != BandTry::Approaching && jump.tries < JUMP_TRIES {
                    jump.tries = JUMP_TRIES;
                    band_jumps::no_run_up();
                    #[cfg(test)]
                    eprintln!(
                        "band-no-run-up take-off ({:.2}, {:.2}, {:.2}) landing ({:.2}, {:.2}, {:.2}) rise {:.2}; feet ({:.2}, {:.2}, {:.2}); the points {} m and {} m behind it: {:?} {:?}",
                        take_off.x, take_off.y, take_off.z, landing.x, landing.y, landing.z, landing.y - take_off.y,
                        feet.x, feet.y, feet.z,
                        SPRINT_RUN_UP_M, mapdata::grid::SPRINT_RUN_UP_M, at(SPRINT_RUN_UP_M), at(mapdata::grid::SPRINT_RUN_UP_M)
                    );
                }
                found.unwrap_or(mapdata::grid::SPRINT_RUN_UP_M)
            }
            Leap::Ledge | Leap::SprintLedge => {
                let base = if sprint { SPRINT_RUN_UP_M } else { RUN_UP_M };
                // A press farther before the take-off is walked into from farther back.
                (lead_m > 0.0)
                    .then_some(lead_m + LEAD_RUN_UP_M)
                    .into_iter()
                    .chain([base, 0.5 * base])
                    .find(|&m| clear(at(m)))
                    .unwrap_or(0.0)
            }
            Leap::Gap(press) | Leap::SprintGap(press) => {
                let m = press.metres() + mapdata::grid::gap::GAP_RUN_UP_M;
                if clear(at(m)) { m } else { press.metres() }
            }
        };
        let (dx, dz) = (feet.x - take_off.x, feet.z - take_off.z);
        let along = dx * way[0] + dz * way[1];
        let side = (dx * way[1] - dz * way[0]).abs();
        if lifting {
            // The lift stands at the face while the client holds it, then goes up; one that is not up in time is a jump.
            let mut at_face = match jump.lift {
                LiftTry::Lifting { at_face } => at_face,
                _ => None,
            };
            if at_face.is_none() && -along <= LIFT_FACE_M {
                at_face = Some(self.clock);
                lifts::tried(landing.y - take_off.y);
            }
            jump.lift_seen = Some(self.clock);
            if at_face.is_some_and(|at| self.since(at) > LIFT_WAIT_S || pose.sprint_block.is_some()) {
                jump.lift = LiftTry::GaveUp;
                lifts::failed(landing.y - take_off.y, pose.sprint_block.is_some());
                lifting = false;
            } else {
                jump.lift = LiftTry::Lifting { at_face };
            }
        }
        let on_line =
            side <= LINED_SIDE_M && along <= LINED_PAST_M && along >= -(run_up + LINED_BEHIND_M);
        if jump.back && !in_the_air {
            self.links.backing();
        }
        if jump.back && (at(run_up).distance_xz(feet) <= AT_RUN_UP_M || (run_up == 0.0 && on_line))
        {
            jump.back = false;
        }
        // A sprint gap link is lined up only over its last metres: the run before it follows the route and may turn onto
        // the jump's way (the run along a rail, the jump across it).
        // A lift that has reached its face is on the jump's line until it is up: the climb carries the body past the
        // take-off, which is no reason to turn it back to the run-up.
        let climbing = lifting && matches!(jump.lift, LiftTry::Lifting { at_face: Some(_) });
        let lined = if sprint_gap {
            in_the_air || (before_route <= SPRINT_GAP_TURN_M && !jump.back)
        } else {
            in_the_air || climbing || (on_line && !jump.back)
        };
        let aim = if lined {
            // Along the jump's way, past the landing (a gap link's lands
            // up to 2 m on).
            let past = 3.0f32.max(take_off.distance_xz(landing) + 1.0);
            Vec3::new(
                take_off.x + way[0] * past,
                landing.y,
                take_off.z + way[1] * past,
            )
        } else if !sprint_gap && clear(at(run_up)) {
            at(run_up)
        } else {
            // The run-up not in a line yet (round a corner): the route, up
            // to the take-off.
            let ahead = self.feel.look_ahead_m.min(before_route.max(0.0));
            self.look_ahead(route, s, ahead, clear, self.heights()).0
        };
        // The way the feet went since the last tick (the hop's trail): a
        // body facing the way but still moving off it (back from the
        // run-up, the walk's blend) is not pressed for.
        let going = sprint_gap
            || self
            .trail
            .back()
            .filter(|&&(at, _)| self.clock.wrapping_sub(at) <= 1)
            .is_none_or(|&(_, before)| {
                let (mx, mz) = (feet.x - before.x, feet.z - before.z);
                let moved = mx.hypot(mz);
                moved < STANDING_M || mx * way[0] + mz * way[1] >= GOING_TURN.cos() * moved
            });
        self.jump_try = Some(jump);
        let kind = match leap {
            Leap::Ledge | Leap::SprintLedge if lifting => link_time::Kind::Lift,
            Leap::Ledge if through => link_time::Kind::Through,
            Leap::Ledge => link_time::Kind::Walking,
            Leap::SprintLedge => link_time::Kind::SprintBand,
            Leap::SprintGap(_) if self.feel.sprint_gaps => link_time::Kind::SprintGap,
            Leap::Gap(_) | Leap::SprintGap(_) => link_time::Kind::Gap,
        };
        let top = route.ledge_top(k);
        self.links.begin(self.clock, link_time::Link { kind, take_off, landing, top }, feet, pose.speed);
        Some(Ahead {
            aim,
            lined,
            way,
            before: if sprint_gap { before_route } else { -along },
            going,
            failed: None,
            leap,
            approach: if lifting {
                Approach::Lift
            } else if through {
                Approach::Through
            } else if sprint_gap {
                Approach::SprintGap
            } else if sprint {
                Approach::Sprint
            } else {
                Approach::Walk
            },
            rise: landing.y - take_off.y,
            lead_m,
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
    fn jump_key(&mut self, ahead: &Ahead, still: f32, pose: &Pose, aimed: bool) -> u32 {
        let Some(mut jump) = self.jump_try else {
            return 0;
        };
        if pose.no_jump {
            return 0;
        }
        // The sprint lift is no press: the sprint into the face carries the body up.
        if ahead.approach == Approach::Lift {
            return 0;
        }
        // A sprint jump is pressed at speed with sprint held (or, past its take-off, as it is: the try counts).
        let band = self.feel.band_jump_gate && ahead.approach.sprints() && !ahead.approach.runs_through() && self.over_walk_top(ahead.rise);
        let block = pose.sprint_block.or(aimed.then_some(SprintBlock::Aiming));
        let aligned = !band || press_aligned(pose.velocity, ahead.way);
        let sprinted = self.sprinting && pose.speed >= SPRINT_PRESS_SPEED_MS && (!band || (block.is_none() && aligned));
        if ahead.approach.sprints() && !sprinted && (ahead.before > 0.0 || band) {
            if band && ahead.before.abs() <= BAND_FACE_M && pose.speed < STILL_MS {
                band_jumps::face_tick();
            }
            if !band || ahead.before > 0.0 {
                return 0;
            }
        }
        let speed = pose.speed;
        let rested = jump
            .pressed
            .is_none_or(|at| self.since(at) >= JUMP_AGAIN_S.max(CARRY_S));
        // A jump link as it walks over the take-off; a gap link at its own
        // point (where the grid found the client's jump lands it).
        let near = match ahead.leap {
            Leap::Ledge | Leap::SprintLedge => ahead.before <= (JUMP_LEAD_S * speed + PRESS_M).max(ahead.lead_m),
            Leap::Gap(press) | Leap::SprintGap(press) => ahead.before <= press.metres(),
        };
        if !(ahead.lined && ahead.going && self.walking && still <= JUMP_FACING && near && rested) {
            return 0;
        }
        if band && !sprinted {
            // At the take-off short of the speed the client asks of a sprint jump: a walk jump would not get up. Back
            // off for another run-up. A block that passes (aiming, a reload, a stagger, busy hands) is waited out
            // for [`BAND_HOLD_S`] without a try; anything else, and a wait that is over, spends one, so it is no loop
            // (`JUMP_TRIES`).
            let passes = matches!(
                block,
                Some(SprintBlock::Aiming | SprintBlock::Reloading | SprintBlock::Staggered | SprintBlock::Busy)
            );
            let waited = match jump.band {
                BandTry::Held { since, .. } => self.since(since),
                _ => 0.0,
            };
            if passes && waited <= BAND_HOLD_S {
                if !matches!(jump.band, BandTry::Held { .. }) {
                    jump.band = BandTry::Held { since: self.clock, refused: jump.band.refused() };
                    band_jumps::held_off();
                }
            } else {
                if passes {
                    band_jumps::hold_over();
                }
                jump.tries += 1;
                let refused = jump.band.refused() + 1;
                jump.band = BandTry::BackedOff { refused, at: self.clock };
                if refused >= BAND_REFUSALS {
                    // Backed off enough: the next arrival gives it up, the route is planned again.
                    jump.tries = JUMP_TRIES;
                }
            }
            jump.back = true;
            self.jump_try = Some(jump);
            self.links.backed_off();
            band_jumps::refused(block, self.sprinting, self.sprinting && pose.speed >= SPRINT_PRESS_SPEED_MS && !aligned);
            band_jumps::face(jump.take_off, ahead.rise, band_jumps::Event::Refused);
            return 0;
        }
        if let BandTry::Held { refused, .. } = jump.band {
            band_jumps::pressed_after_hold();
            jump.band = if refused == 0 { BandTry::Approaching } else { BandTry::BackedOff { refused, at: self.clock } };
        }
        jump.tries += 1;
        jump.pressed = Some(self.clock);
        jump.back = true;
        self.jump_try = Some(jump);
        self.links.pressed(self.clock, pose.feet, pose.speed);
        self.pressed = Some(RoutePress {
            take_off: jump.take_off,
            landing: jump.landing,
            tries: jump.tries,
            before: ahead.before,
            facing: still,
        });
        self.jump = Some(JumpBy::Route);
        self.last_press = Some(LandingWatch { at: self.clock, counted: false, from: LandingFrom::Jump });
        if matches!(ahead.leap, Leap::Ledge | Leap::SprintLedge) && ahead.rise <= LIFT_MAX_RISE_M {
            lifts::jumped(ahead.rise, pose.urgent);
        }
        if ahead.approach == Approach::Sprint {
            sprint_jumps::pressed(jump.landing.y - jump.take_off.y);
            if self.over_walk_top(ahead.rise) {
                band_jumps::pressed(speed, self.sprinting);
                band_jumps::face(jump.take_off, ahead.rise, band_jumps::Event::Press);
                // A press still followed when the next comes did not get up on its own.
                let watch = BandWatch {
                    at: self.clock,
                    landing: jump.landing,
                    #[cfg(test)]
                    from: pose.feet,
                    #[cfg(test)]
                    take_off: jump.take_off,
                    #[cfg(test)]
                    speed,
                    #[cfg(test)]
                    sprint: self.sprinting,
                };
                if let Some(_old) = self.band_watch.replace(watch) {
                    band_jumps::strict(false);
                    #[cfg(test)]
                    band_jumps::trace("second press", self.since(_old.at), &_old, pose.feet);
                }
            }
        }
        if ahead.approach.runs_through() {
            sprint_gaps::pressed(speed);
            sprint_gaps::sprint_held(self.sprinting);
        }
        key::JUMP
    }

    /// A world showed the body still under the jump it last pressed (on the
    /// take-off's floor, near it, not up) while the reckoning had taken it
    /// up and the route on past it (`body::JUMP_AIR_MS`): the route is
    /// looked at from that jump's leg again, so the jump is lined up again
    /// instead of walked on from under it.
    /// The strict got-up count of the sprint-band press followed ([`BandWatch`]): on the landing's floor, near it, within
    /// [`BAND_UP_S`] of the press; else not up.
    /// A lift that has reached its face and finds the body up the ledge is a lift got up ([`lifts::up`]), whatever the
    /// route's progress is by then.
    fn follow_lift(&mut self, feet: Vec3) {
        let clock = self.clock;
        let since = |at: u32| clock.wrapping_sub(at) as f32 * TICK_S;
        if let Some(t) = self.jump_try.as_mut()
            && let LiftTry::Lifting { at_face: Some(_) } = t.lift
        {
            if is_up(feet, t.take_off, t.landing) {
                t.lift = LiftTry::Open;
                lifts::up(t.landing.y - t.take_off.y);
            } else if t.lift_seen.is_some_and(|at| since(at) > LIFT_RESUME_S) {
                // The walk has not asked for the lift at this face for a while (a refusal that stayed, or the route went
                // on to another leg): its outcome is counted and the jump is the way up.
                t.lift = LiftTry::GaveUp;
                lifts::left(t.landing.y - t.take_off.y, t.lift_refused_at_face);
            }
        }
    }

    fn follow_band_press(&mut self, feet: Vec3) {
        let Some(watch) = self.band_watch else {
            return;
        };
        if feet.distance_xz(watch.landing) <= BAND_UP_NEAR_M && (feet.y - watch.landing.y).abs() <= BAND_UP_FLOOR_M {
            band_jumps::strict(true);
            #[cfg(test)]
            band_jumps::trace("up", self.since(watch.at), &watch, feet);
            self.band_watch = None;
        } else if self.since(watch.at) > BAND_UP_S {
            band_jumps::strict(false);
            #[cfg(test)]
            band_jumps::trace("not up", self.since(watch.at), &watch, feet);
            self.band_watch = None;
        }
    }

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
            .position(|point| point.jump.is_some() && (point.at - jump.landing).length() < 0.3);
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
            .is_none_or(|at| self.clock.wrapping_sub(at) as f32 * TICK_S >= feel.hop_every_s.get());
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
        let (low, high) = self.feel.hop_rise_m.get();
        let look = self.feel.hop_look_m;
        let within = |rise: f32| rise >= low && rise <= high;
        let height = self.heights();
        let base = route.at(s, height).y;
        let route_rise = (1..=3)
            .map(|k| route.at(s + look * k as f32 / 3.0, height).y - base)
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

/// What a watched landing started from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LandingFrom {
    /// The press of a route jump.
    Jump,
    /// The take-off of a planned drop ([`Route::left_drop_edge`]).
    Drop,
}

/// The landing being watched ([`Motor::landing_brake`]): when it started, whether it is counted yet, and from what.
#[derive(Clone, Copy, Debug)]
struct LandingWatch {
    at: u32,
    counted: bool,
    from: LandingFrom,
}

/// The landings of route jumps and of planned drops by how far the carried run is from the way (the smaller of its
/// angle to the route's next leg and to the goal): 0-30, 30-60, 60-90, over 90 degrees, and those with a wall within
/// the coast; and how many of them were braked ([`Feel::landing_cancel_deg`], [`Feel::landing_cancel_drops`]).
pub mod landings {
    use super::LandingFrom;
    use std::sync::atomic::{AtomicU64, Ordering};

    pub const BANDS: usize = 4;
    /// By jump then drop: the bands, then the walls, of the landings seen; the same of those braked.
    static COUNTS: [[[AtomicU64; BANDS + 1]; 2]; 2] =
        [const { [const { [const { AtomicU64::new(0) }; BANDS + 1] }; 2] }; 2];

    /// A landing run at `against` radians from the way, into a wall or not, braked or not.
    pub(super) fn seen(from: LandingFrom, against: f32, wall: bool, braked: bool) {
        let band = if wall { BANDS } else { ((against.to_degrees() / 30.0) as usize).min(BANDS - 1) };
        let counts = &COUNTS[from as usize];
        counts[0][band].fetch_add(1, Ordering::Relaxed);
        if braked {
            counts[1][band].fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Seen and braked, by band (the last is the walls), of the route jumps.
    pub fn counts() -> [[u64; BANDS + 1]; 2] {
        counts_of(LandingFrom::Jump)
    }

    /// The same of the planned drops.
    pub fn drop_counts() -> [[u64; BANDS + 1]; 2] {
        counts_of(LandingFrom::Drop)
    }

    fn counts_of(from: LandingFrom) -> [[u64; BANDS + 1]; 2] {
        std::array::from_fn(|k| std::array::from_fn(|b| COUNTS[from as usize][k][b].load(Ordering::Relaxed)))
    }
}

/// The sprint at planned drops (`Feel::drop_walk`): the ticks it was let go of before one (by what the planner knows of
/// it: unsafe at a sprint, no record by the way the route was made, a descent by walk links only), the ticks the
/// release was skipped as the drop is safe at a sprint, the hard falls (`stuck::HARD_FALL_M` or more) within
/// [`DROP_SPRINTED_WATCH_S`] of a drop's edge crossed at a sprint (the harm); and the releases as episodes (the trace of
/// #671): the seconds and metres from the first tick to the edge, and the seconds from the edge, and from the landing,
/// to the sprint back, in bins.
pub mod drop_walks {
    use std::sync::atomic::{AtomicU64, Ordering};

    use crate::v026g::bots::path::DropRecord;

    /// Released ticks by kind (unsafe 0, unknown 1), skipped as safe 6, skipped as a descent by walk links 5, hard falls 7,
    /// no edge 14 and unresolved 15; the episodes by kind are apart.
    const HEAD: usize = 16;
    /// The bins: start to edge 0.1 s x 60, start to edge 0.25 m x 24, edge to sprint 0.1 s x 100, landing to sprint 0.1 s x 100.
    const T_EDGE: usize = 60;
    const D_EDGE: usize = 24;
    const T_SPRINT: usize = 100;
    const LEN: usize = HEAD + T_EDGE + D_EDGE + 2 * T_SPRINT;
    static COUNTS: [AtomicU64; LEN] = [const { AtomicU64::new(0) }; LEN];

    fn add(k: usize) {
        COUNTS[k].fetch_add(1, Ordering::Relaxed);
    }

    fn kind(record: DropRecord) -> usize {
        match record {
            DropRecord::Unsafe => 0,
            _ => 1,
        }
    }

    /// A tick the sprint was let go of.
    pub fn released(record: DropRecord) {
        add(kind(record));
    }

    /// A tick the release was skipped as the drops ahead are descents by walk links only.
    pub fn skipped_as_descent() {
        add(5);
    }

    pub fn skipped_as_safe() {
        add(6);
    }

    pub fn hard_fall_sprinted() {
        add(7);
    }

    /// A release begins.
    pub fn episode(record: DropRecord) {
        EPISODES[kind(record)].fetch_add(1, Ordering::Relaxed);
    }

    static EPISODES: [AtomicU64; 2] = [const { AtomicU64::new(0) }; 2];

    /// The edge crossed `seconds` after the release began, the drop's edge `metres` along then.
    pub fn edge_after(seconds: f32, metres: f32) {
        add(HEAD + ((seconds / 0.1) as usize).min(T_EDGE - 1));
        add(HEAD + T_EDGE + ((metres / 0.25) as usize).min(D_EDGE - 1));
    }

    /// The sprint is back `from_edge` seconds after the edge, and `from_landing` after the landing.
    pub fn resumed(from_edge: f32, from_landing: Option<f32>) {
        add(HEAD + T_EDGE + D_EDGE + ((from_edge / 0.1) as usize).min(T_SPRINT - 1));
        if let Some(seconds) = from_landing {
            add(HEAD + T_EDGE + D_EDGE + T_SPRINT + ((seconds / 0.1) as usize).min(T_SPRINT - 1));
        }
    }

    /// A release ended without the sprint back: before the edge (the route changed) or after it (10 s).
    pub fn unresolved(after_edge: bool) {
        add(if after_edge { 15 } else { 14 });
    }

    /// The counts: the head, then the four histograms.
    pub fn counts() -> Vec<u64> {
        COUNTS.iter().map(|c| c.load(Ordering::Relaxed)).collect()
    }

    /// The episodes by kind (unsafe, unknown).
    pub fn episodes() -> Vec<u64> {
        EPISODES.iter().map(|c| c.load(Ordering::Relaxed)).collect()
    }

    /// The `q` quantile of the bins `d` (each `width` wide, the last open), in the bins' unit.
    pub fn quantile(d: &[u64], width: f32, q: f32) -> f32 {
        let total: u64 = d.iter().sum();
        if total == 0 {
            return 0.0;
        }
        let mut seen = 0;
        for (k, &n) in d.iter().enumerate() {
            seen += n;
            if seen as f32 >= q * total as f32 {
                return (k as f32 + 0.5) * width;
            }
        }
        d.len() as f32 * width
    }

    /// The layout for the tally: the four histograms' slices of [`counts`].
    pub const SLICES: [(usize, usize); 4] = [
        (HEAD, T_EDGE),
        (HEAD + T_EDGE, D_EDGE),
        (HEAD + T_EDGE + D_EDGE, T_SPRINT),
        (HEAD + T_EDGE + D_EDGE + T_SPRINT, T_SPRINT),
    ];
}

/// The sprint gap links of routes ([`Leap::SprintGap`], `Feel::sprint_gaps`): presses by the speed at the press (under
/// 3.5, 3.5-4.5, 4.5 and over m/s: the speed after the turn is a GUESS to read off this), and those given up.
pub mod sprint_gaps {
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTS: [AtomicU64; 7] = [const { AtomicU64::new(0) }; 7];

    /// A press with the sprint key held (4) or not (5): the lining up lets go of it with `band_jump_gate` off.
    pub fn sprint_held(held: bool) {
        COUNTS[if held { 4 } else { 5 }].fetch_add(1, Ordering::Relaxed);
    }

    /// A gap got across: the body is at the landing after a press.
    pub fn across() {
        COUNTS[6].fetch_add(1, Ordering::Relaxed);
    }

    pub fn pressed(speed: f32) {
        let band = if speed < 3.5 { 0 } else if speed < 4.5 { 1 } else { 2 };
        COUNTS[band].fetch_add(1, Ordering::Relaxed);
    }

    pub fn gave_up() {
        COUNTS[3].fetch_add(1, Ordering::Relaxed);
    }

    /// Pressed under 3.5 m/s, 3.5-4.5, 4.5 and over, given up, pressed with the sprint key held and not, got across.
    pub fn counts() -> [u64; 7] {
        std::array::from_fn(|k| COUNTS[k].load(Ordering::Relaxed))
    }
}

/// The sprint-band ledge jumps (a rise over [`WALK_JUMP_TOP_M`], `Feel::band_jump_gate`): the presses by the ground speed
/// at the press and with the sprint key not held; the arrivals backed off from (refused) by reason, the links with no
/// clear run given up, the ticks the body stood at a face (within [`BAND_FACE_M`], under [`STILL_MS`]) with no press,
/// and the faces picked again after giving up. The jumps given up are in [`sprint_jumps`].
pub mod band_jumps {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::SprintBlock;

    /// The counters, by name.
    #[derive(Clone, Copy, Debug)]
    pub enum Slot {
        PressUnder35,
        Press35To40,
        Press40To45,
        Press45Up,
        PressNoSprint,
        RefusedSlow,
        RefusedNoKey,
        RefusedAiming,
        RefusedBusy,
        RefusedReloading,
        RefusedStaggered,
        RefusedTired,
        RefusedCarrying,
        RefusedDirection,
        NoRunUp,
        FaceStill,
        Repicked,
        StrictUp,
        StrictNotUp,
        HeldOff,
        HoldOver,
        PressedAfterHold,
    }

    pub const COUNT: usize = Slot::PressedAfterHold as usize + 1;
    static COUNTS: [AtomicU64; COUNT] = [const { AtomicU64::new(0) }; COUNT];

    fn add(slot: Slot) {
        COUNTS[slot as usize].fetch_add(1, Ordering::Relaxed);
    }

    pub fn pressed(speed: f32, sprint_held: bool) {
        add(if !sprint_held {
            Slot::PressNoSprint
        } else if speed < 3.5 {
            Slot::PressUnder35
        } else if speed < 4.0 {
            Slot::Press35To40
        } else if speed < 4.5 {
            Slot::Press40To45
        } else {
            Slot::Press45Up
        });
    }

    /// An arrival backed off from, by the reason: a `block`, else the sprint key not held, else the speed short of the
    /// press speed, or (`crooked`) the speed enough but the run not along the jump's way.
    pub fn refused(block: Option<SprintBlock>, sprint_held: bool, crooked: bool) {
        add(match block {
            Some(SprintBlock::Aiming) => Slot::RefusedAiming,
            Some(SprintBlock::Busy) => Slot::RefusedBusy,
            Some(SprintBlock::Reloading) => Slot::RefusedReloading,
            Some(SprintBlock::Staggered) => Slot::RefusedStaggered,
            Some(SprintBlock::Tired) => Slot::RefusedTired,
            Some(SprintBlock::Carrying) => Slot::RefusedCarrying,
            None if !sprint_held => Slot::RefusedNoKey,
            None if crooked => Slot::RefusedDirection,
            None => Slot::RefusedSlow,
        });
    }

    pub fn no_run_up() {
        add(Slot::NoRunUp);
    }

    pub fn face_tick() {
        add(Slot::FaceStill);
    }

    pub fn repicked() {
        add(Slot::Repicked);
    }

    /// A hold off the face begun (a block that passes), one that ran out ([`super::BAND_HOLD_S`]: the try counts), a press
    /// made after a hold.
    pub fn held_off() {
        add(Slot::HeldOff);
    }

    pub fn hold_over() {
        add(Slot::HoldOver);
    }

    pub fn pressed_after_hold() {
        add(Slot::PressedAfterHold);
    }

    /// A band press followed to its end: on the landing's floor within 1.5 s with no second press (up), or not.
    pub fn strict(up: bool) {
        add(if up { Slot::StrictUp } else { Slot::StrictNotUp });
    }

    /// The counts, by [`Slot`].
    pub fn counts() -> [u64; COUNT] {
        std::array::from_fn(|k| COUNTS[k].load(Ordering::Relaxed))
    }

    /// The first of each kind of ending of a band press, to stderr (the trace of #673): when, where from, where to, the
    /// speed and the sprint key at the press, and where the body is.
    #[cfg(test)]
    pub(super) fn trace(what: &str, seconds: f32, watch: &super::BandWatch, now: super::Vec3) {
        static SEEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        if SEEN.fetch_add(1, Ordering::Relaxed) < 60 {
            let (f, t, l) = (watch.from, watch.take_off, watch.landing);
            eprintln!(
                "band-press {what} after {seconds:.2} s: press at ({:.2}, {:.2}, {:.2}) speed {:.2} sprint {}; take-off ({:.2}, {:.2}, {:.2}); landing ({:.2}, {:.2}, {:.2}) rise {:.2}; now ({:.2}, {:.2}, {:.2})",
                f.x, f.y, f.z, watch.speed, watch.sprint, t.x, t.y, t.z, l.x, l.y, l.z, l.y - t.y, now.x, now.y, now.z
            );
        }
    }

    /// What happened at a face.
    #[derive(Clone, Copy, Debug)]
    pub enum Event {
        Press,
        Refused,
        Up,
        GaveUp,
    }

    /// Per face (the take-off's metre bin: x, z, y in 2 m), the presses, the arrivals backed off from, the times it was
    /// got up and given up; the faces of a rise over [`super::WALK_JUMP_TOP_M`] only.
    static FACES: std::sync::Mutex<std::collections::BTreeMap<(i32, i32, i32), [u32; 4]>> =
        std::sync::Mutex::new(std::collections::BTreeMap::new());

    pub fn face(take_off: super::Vec3, rise: f32, event: Event) {
        if rise <= super::WALK_JUMP_TOP_M {
            return;
        }
        if let Ok(mut faces) = FACES.lock() {
            let key = (take_off.x.floor() as i32, take_off.z.floor() as i32, (take_off.y / 2.0).floor() as i32);
            faces.entry(key).or_default()[event as usize] += 1;
        }
    }

    /// The `n` faces with the most presses and arrivals backed off from: ((x, z, y/2), [presses, refused, up, gave up]).
    pub fn top_faces(n: usize) -> Vec<((i32, i32, i32), [u32; 4])> {
        let mut faces: Vec<_> = FACES.lock().map(|f| f.iter().map(|(&k, &v)| (k, v)).collect()).unwrap_or_default();
        faces.sort_by(|a, b| (b.1[0] + b.1[1]).cmp(&(a.1[0] + a.1[1])).then(a.0.cmp(&b.0)));
        faces.truncate(n);
        faces
    }
}

/// The sprint lift ([`Feel::fight_lift`]) and the jumps pressed up the same tops (a rise of [`LIFT_MAX_RISE_M`] or less),
/// by band of rise: up to 0.80 m (the client's step), 0.80-0.95, 0.95-1.05, 1.05-1.11. Lifts tried (the body stood at the
/// face), got up, given up for the jump after [`LIFT_WAIT_S`]; jumps pressed with the body calm and in a fight or a rush;
/// and the faces a body in a fight or a rush did not lift, by reason.
pub mod lifts {
    use std::sync::atomic::{AtomicU64, Ordering};

    pub const BANDS: usize = 4;
    static COUNTS: [[AtomicU64; BANDS]; 6] = [const { [const { AtomicU64::new(0) }; BANDS] }; 6];
    static REFUSED: [AtomicU64; 3] = [const { AtomicU64::new(0) }; 3];

    /// Why a body in a fight or a rush did not lift a face of its route.
    #[derive(Clone, Copy, Debug)]
    pub enum Refused {
        /// The top is shallower than [`super::LIFT_TOP_M`], or was not measured.
        TopShort,
        /// The client lets the body take no sprint (tired, a battery, staggered).
        Blocked,
        /// Less than [`super::LIFT_RUN_UP_M`] of clear run before the face.
        NoRunUp,
    }

    fn band(rise: f32) -> usize {
        if rise <= 0.8 {
            0
        } else if rise <= 0.95 {
            1
        } else if rise <= 1.05 {
            2
        } else {
            3
        }
    }

    fn add(kind: usize, rise: f32) {
        COUNTS[kind][band(rise)].fetch_add(1, Ordering::Relaxed);
    }

    pub fn tried(rise: f32) {
        add(0, rise);
    }

    pub fn up(rise: f32) {
        add(1, rise);
    }

    /// What became of the sprint and forward keys of a lift at its face (the ticks), by the first thing that took them.
    #[derive(Clone, Copy, Debug)]
    pub enum Keys {
        /// The walk aims (a fight's target): the body faces the target, not the jump.
        AimedWalk,
        /// The motor itself asked for no sprint or no forward.
        MotorOff,
        /// The reflex's side step.
        SideStep,
        /// The lip guard.
        LipGuard,
        /// The play's pace is not a run, or it is crouched or sneaking.
        PaceOrStance,
        /// The reflex fires this tick.
        Fire,
        /// The reflex's aim, grenade, reload or crouch key.
        OverrideKeys,
        /// The lining up of a walking jump.
        Lining,
        /// Held to the client.
        Held,
    }

    /// Why the motor itself asked a lift at its face for no sprint or no forward (the ticks).
    #[derive(Clone, Copy, Debug)]
    pub enum MotorOff {
        /// The landing brake of an earlier jump.
        Brake,
        /// The walk stands (the view is not within the line-up turn of the jump's way, or no walk).
        Standing,
        /// The body may not jump now (hurt, or the bar under the jump's cost): no sprint is taken up.
        NoJump,
        /// The reckoned bar is spent (the motor's own tired state).
        Tired,
        /// Walking but no sprint: the lining up did not take it up.
        NotSprinting,
        /// Sprint held, no forward.
        NoForward,
    }

    static MOTOR_OFF: [AtomicU64; 6] = [const { AtomicU64::new(0) }; 6];

    pub fn motor_off(why: MotorOff) {
        MOTOR_OFF[why as usize].fetch_add(1, Ordering::Relaxed);
    }

    /// The ticks of a lift at its face the motor asked no sprint or forward of, by [`MotorOff`].
    pub fn motor_off_counts() -> [u64; 6] {
        std::array::from_fn(|k| MOTOR_OFF[k].load(Ordering::Relaxed))
    }

    static KEYS: [AtomicU64; 9] = [const { AtomicU64::new(0) }; 9];

    pub fn keys(stage: Keys) {
        KEYS[stage as usize].fetch_add(1, Ordering::Relaxed);
    }

    /// The ticks of a lift at its face by [`Keys`].
    pub fn key_counts() -> [u64; 9] {
        std::array::from_fn(|k| KEYS[k].load(Ordering::Relaxed))
    }

    /// A lift given up for the jump; `blocked`: the client took no sprint at the face (a shot, an aim), not the time.
    pub fn failed(rise: f32, blocked: bool) {
        add(2, rise);
        if blocked {
            add(5, rise);
        }
    }

    /// A jump pressed up a face a lift could climb, `urgent`: the body in a fight or a rush.
    pub fn jumped(rise: f32, urgent: bool) {
        add(if urgent { 4 } else { 3 }, rise);
    }

    pub fn refused(why: Refused) {
        REFUSED[why as usize].fetch_add(1, Ordering::Relaxed);
    }

    static LEFT: [[AtomicU64; BANDS]; 4] = [const { [const { AtomicU64::new(0) }; BANDS] }; 4];

    /// A lift that reached its face and was left for the jump with no outcome of its own: by what refused it there (top
    /// short, sprint blocked or the walk aims or the bar is spent, no run), or none (the route went on to another leg).
    pub fn left(rise: f32, why: Option<Refused>) {
        LEFT[why.map_or(3, |w| w as usize)][band(rise)].fetch_add(1, Ordering::Relaxed);
    }

    /// Lifts left, by [`left`]'s cause and band.
    pub fn left_counts() -> [[u64; BANDS]; 4] {
        std::array::from_fn(|k| std::array::from_fn(|b| LEFT[k][b].load(Ordering::Relaxed)))
    }

    /// Tried, up, failed, jumps calm, jumps in a fight or a rush: by band.
    pub fn counts() -> [[u64; BANDS]; 6] {
        std::array::from_fn(|k| std::array::from_fn(|b| COUNTS[k][b].load(Ordering::Relaxed)))
    }

    /// Faces not lifted: top short, sprint blocked, no run.
    pub fn refusals() -> [u64; 3] {
        std::array::from_fn(|k| REFUSED[k].load(Ordering::Relaxed))
    }
}

/// The sprint jumps of routes by 0.05 m band of rise from [`SPRINT_BAND_FROM_M`]: pressed, got up, given up, for the
/// bench's failed-jump counter.
pub mod sprint_jumps {
    use std::sync::atomic::{AtomicU64, Ordering};

    /// The bands: 0.95-1.00, 1.00-1.05 (the capped band), 1.05-1.10, 1.10-1.15, 1.15-1.20, 1.20-1.25, 1.25-1.30, 1.30 and over.
    pub const BANDS: usize = 8;
    static COUNTS: [[AtomicU64; BANDS]; 3] = [const { [const { AtomicU64::new(0) }; BANDS] }; 3];

    fn band(rise: f32) -> usize {
        (((rise - super::SPRINT_BAND_FROM_M) / 0.05).floor().max(0.0) as usize).min(BANDS - 1)
    }

    pub fn pressed(rise: f32) {
        COUNTS[0][band(rise)].fetch_add(1, Ordering::Relaxed);
    }

    pub fn up(rise: f32) {
        COUNTS[1][band(rise)].fetch_add(1, Ordering::Relaxed);
    }

    pub fn gave_up(rise: f32) {
        COUNTS[2][band(rise)].fetch_add(1, Ordering::Relaxed);
    }

    /// Pressed, up and given up, by band.
    pub fn counts() -> [[u64; BANDS]; 3] {
        std::array::from_fn(|k| std::array::from_fn(|b| COUNTS[k][b].load(Ordering::Relaxed)))
    }
}

/// The bench's counts of the walk ticks whose look-ahead was cut to a drop's landing ([`Feel::drop_aim`]), and of
/// the ticks of a run that went on past [`DROP_AIM_LONG_TICKS`] (the body stays above the drop it aims off).
#[cfg(test)]
pub mod drop_aim {
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTS: [AtomicU64; 2] = [const { AtomicU64::new(0) }; 2];

    pub fn tick(run: u32) {
        COUNTS[0].fetch_add(1, Ordering::Relaxed);
        if run > super::DROP_AIM_LONG_TICKS {
            COUNTS[1].fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn counts() -> [u64; 2] {
        std::array::from_fn(|k| COUNTS[k].load(Ordering::Relaxed))
    }
}

/// The bench's counts (all matches of the run): the route jumps got up at
/// the first press and later, those given up ([`JUMP_TRIES`]), the cuts of
/// a look-ahead to a point a line is not clear to, and the fight's side
/// steps asked and refused (`Tactics::clear_step_for`).
#[cfg(test)]
pub mod counts {
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTS: [AtomicU64; 14] = [const { AtomicU64::new(0) }; 14];

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

    /// A walk tick whose look-ahead ended on a point the body does not walk to in a line.
    pub fn unclear() {
        add(8);
    }

    /// Of those, ticks that steered at a clear route point back of the progress instead.
    pub fn unclear_back() {
        add(9);
    }

    /// Of those, ticks with no clear route point at all: reported off the route.
    pub fn unclear_off() {
        add(10);
    }

    /// An ahead cut by a point that is not clear at the line between the legs' ends and is at the floor's height:
    /// the false clear tests of a hump ([`Feel::floor_height_clear`] off).
    pub fn cut_false() {
        add(11);
    }

    /// Of the unclear look-aheads (rule on), those with a planned jump or drop within reach: steered at, not back.
    pub fn unclear_link() {
        add(13);
    }

    /// Of the unclear look-aheads (rule on), those clear at the floor's heights: steered at, not back.
    pub fn unclear_floor() {
        add(12);
    }

    static LIP_GAPS: [AtomicU64; 8] = [const { AtomicU64::new(0) }; 8];

    /// The lip guard's gaps ([`Feel::lip_guard_full`]): 0 asked for a glide (no move key, speed), 1 of those a swap or brake, 2 asked
    /// only because of the floors about the reckoned feet, 3 of those a swap or brake, 4 a walk the shifted feet would have fallen from
    /// (held, the plain stop was safe), 5 given up (entered `GaveUp`), 6 watched again after moving or changing keys, 7 ticks
    /// asked for a glide only while given up.
    pub fn lip_gap(k: usize) {
        LIP_GAPS[k].fetch_add(1, Ordering::Relaxed);
    }

    pub fn lip_gaps() -> [u64; 8] {
        std::array::from_fn(|k| LIP_GAPS[k].load(Ordering::Relaxed))
    }

    static SLIDES: [AtomicU64; 9] = [const { AtomicU64::new(0) }; 9];

    /// A refused side step of the reflex's own, asked the other side too: whether that side is clear (the
    /// step could slide to it) or both are refused (the bot stands), standing to aim or weaving a walk,
    /// from a floor in a lip's band or not.
    pub fn slide_asked(other_clear: bool, weave: bool, in_band: bool) {
        SLIDES[usize::from(!other_clear) * 4 + usize::from(weave) * 2 + usize::from(in_band)]
            .fetch_add(1, Ordering::Relaxed);
    }

    /// The step was taken to the other side (`clear_step_slides_along`).
    pub fn slid() {
        SLIDES[8].fetch_add(1, Ordering::Relaxed);
    }

    /// (other side clear: still out of the band, still in it, weave out, weave in; both refused: the same
    /// four; slid).
    pub fn slides() -> [u64; 9] {
        std::array::from_fn(|k| SLIDES[k].load(Ordering::Relaxed))
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
    /// steps clear, side steps refused, lip guard changed, lip guard none, look-ahead ended unclear, of them steered back
    /// to a clear point, of them off the route, ahead cuts by a false clear at a hump, unclear look-aheads clear at the floor's
    /// heights).
    pub fn counts() -> [u64; 14] {
        std::array::from_fn(|k| COUNTS[k].load(Ordering::Relaxed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v026g::bots::kin::Kinematics;

    /// The label the planner gives a jump link up to height `y` from a take-off at 0: a sprint jump over the exe body's top.
    fn ledge_to(y: f32) -> Leap {
        if y > mapdata::Body::default().max_jump { Leap::SprintLedge } else { Leap::Ledge }
    }

    /// The feel with the owner's options off (the bots as they were): what a test of an option's effect starts from.
    fn feel_off() -> Feel {
        crate::v026g::bots::tuning::Tuning::all_off().feel
    }

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
                    jump: None,
                    low: false,
                })
                .collect(),
        )
    }

    /// A route along x with waypoint 1 (at x = 5) up a ledge, and the leg
    /// to waypoint 2 low (from x = 5 to 10).
    fn ledge_then_low() -> Route {
        let p = |x: f32, jump: bool, low| Waypoint {
            at: Vec3::new(x, 0.0, 0.0),
            node: 0,
            jump: jump.then_some(Leap::Ledge),
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
        /// The client lifts a body that runs into the face at a sprint (not from a stand) and is held there with the
        /// sprint and forward keys for 0.3 s: up with no jump.
        lifts: bool,
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
        climb_ledge_feel(route, ledge, start, yaw, ticks, aiming, hurt_ticks, Feel::default())
    }

    /// [`climb_ledge_hurt`] with the motor's `feel`.
    #[allow(clippy::too_many_arguments)]
    fn climb_ledge_feel(
        route: &mut Route,
        ledge: &Ledge,
        start: Vec3,
        yaw: f32,
        ticks: usize,
        aiming: bool,
        hurt_ticks: usize,
        feel: Feel,
    ) -> Climb {
        climb_ledge_urgent(route, ledge, start, yaw, ticks, aiming, hurt_ticks, feel, false)
    }

    /// [`climb_ledge_feel`] with the pose's `urgent` flag (a fight or a rush).
    #[allow(clippy::too_many_arguments)]
    fn climb_ledge_urgent(
        route: &mut Route,
        ledge: &Ledge,
        start: Vec3,
        yaw: f32,
        ticks: usize,
        aiming: bool,
        hurt_ticks: usize,
        feel: Feel,
        urgent: bool,
    ) -> Climb {
        let kin = Kinematics::default();
        let mut rng = Rng::new(17);
        let mut motor = Motor::new(feel, &mut rng);
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
        // The ticks held against the face with the sprint and forward keys, and whether the body came to it at a sprint.
        let (mut held, mut came_fast) = (0usize, false);
        for tick in 0..ticks {
            pose.no_jump = tick < hurt_ticks;
            pose.urgent = urgent;
            pose.gait = Gait::Run;
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
                    if held == 0 {
                        came_fast = pose.speed >= 3.5;
                    }
                    let pushing = decision.keys & key::SPRINT != 0 && decision.keys & key::FORWARD != 0;
                    held = if pushing { held + 1 } else { 0 };
                    if ledge.lifts && came_fast && held as f32 * TICK_S >= 0.3 {
                        climb.up = true;
                        climb.up_at = Some(tick);
                        to.y = landing_y;
                        to.x += ledge.way[0] * 0.3;
                        to.z += ledge.way[1] * 0.3;
                    }
                }
            }
            pose.speed = (to - pose.feet).length() / TICK_S;
            pose.velocity = [(to.x - pose.feet.x) / TICK_S, (to.z - pose.feet.z) / TICK_S];
            pose.feet = to;
        }
        climb
    }

    /// A route along x to a take-off at x 4.6, up a 0.9 m ledge to x 5,
    /// and on along x up there; with the take-off at `(x, z)` and the
    /// jump's way turned to +z if `turned`.
    fn up_a_ledge(turned: bool) -> (Route, Ledge) {
        let p = |x: f32, y: f32, z: f32, jump: bool| Waypoint {
            at: Vec3::new(x, y, z),
            node: 0,
            jump: jump.then_some(ledge_to(y)),
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
            lifts: false,
        };
        (Route::new(points), ledge)
    }

    /// A route whose last leg is a jump 0.5 m off and 0.3 m up (a pocket's
    /// rim, `escape.rs`) is not walked until the body is up on it: the
    /// motor presses the jump and gets up. The same end by a walk (no jump)
    /// is arrived at as before.
    #[test]
    fn a_near_low_jump_at_a_routes_end_is_pressed() {
        let p = |x: f32, y: f32, jump: bool| Waypoint {
            at: Vec3::new(x, y, 0.0),
            node: 0,
            jump: jump.then_some(ledge_to(y)),
            low: false,
        };
        let ledge = Ledge {
            take_off: Vec3::ZERO,
            way: [1.0, 0.0],
            face: 0.25,
            reach: 1.2,
            gets_up: true,
            lifts: false,
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
        let p = |x, z| Waypoint { at: Vec3::new(x, 0.0, z), node: 0, jump: None, low: false };
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

    /// A door gate on the way (`Route::door_centred`) is steered at, not past: with the corner a gate the walk heads for it
    /// from 2 m, where the look-ahead would take it to a point round the corner; once the body is at the gate the aim goes
    /// on to the leg after it.
    #[test]
    fn the_walk_steers_at_a_door_gate_not_past_it() {
        let p = |x, z| Waypoint { at: Vec3::new(x, 0.0, z), node: 0, jump: None, low: false };
        let corner = |gate: bool| {
            let route = Route::new(vec![p(0.0, 0.0), p(3.0, 0.0), p(3.0, 4.0)]);
            if gate { route.with_gate(Vec3::new(3.0, 0.0, 0.0)) } else { route }
        };
        let heading = |gate: bool, x: f32| {
            let pose = Pose { feet: Vec3::new(x, 0.0, 0.0), yaw: yaw_of([1.0, 0.0]), pitch: 0.0, speed: 3.0, ..Default::default() };
            let mut rng = Rng::new(3);
            let mut motor = Motor::new(Feel { sway: 0.0, ..Feel::default() }, &mut rng);
            let (decision, walked) = motor.walk(&mut corner(gate), &pose, 5.0, |_| true);
            assert_eq!(walked, Walked::OnTheWay);
            decision.turn.0.abs()
        };
        // 2 m short of the corner the look-ahead (2.65 m) reaches round it; with the gate the aim is the corner, dead ahead.
        assert!(heading(true, 1.0) < heading(false, 1.0) * 0.5, "{} {}", heading(true, 1.0), heading(false, 1.0));
        // At the gate it is reached: the aim is as without one.
        assert_eq!(heading(true, 2.9), heading(false, 2.9));
    }

    /// A lane (A175) moves the steering point sideways on a plain stretch of the route, and keeps the route's own point where the shifted one is no room, is not the route's floor, or the
    /// route has its end, a gate or a jump near; each with its reason.
    #[test]
    fn a_lane_moves_the_steering_point_over_only_where_the_route_is_plain_and_the_ground_is_the_routes() {
        use super::super::apart::{LaneUse, Unsafe};
        let p = |x, z| Waypoint { at: Vec3::new(x, 0.0, z), node: 0, jump: None, low: false };
        let line = || Route::new(vec![p(0.0, 0.0), p(30.0, 0.0)]);
        let pose = Pose { feet: Vec3::new(1.0, 0.0, 0.0), yaw: yaw_of([1.0, 0.0]), pitch: 0.0, speed: 3.0, ..Default::default() };
        let walk = |route: &mut Route, pose: &Pose, lane: Option<(f32, bool, bool)>, clear: bool| {
            // `clear` false: the points off the route's line (across it by more than half a metre) are no room.
            let mut rng = Rng::new(3);
            let mut motor = Motor::new(Feel { sway: 0.0, ..Feel::default() }, &mut rng);
            let safe_floor = lane.is_some_and(|(_, _, safe)| safe);
            let safe = move |_: Vec3, _: Vec3| if safe_floor { Ok(()) } else { Err(Unsafe::OtherFloor) };
            let aim = lane.map(|(offset_m, body_ok, _)| LaneAim { offset_m, body: if body_ok { Ok(()) } else { Err(Unsafe::BodyAtEdge) }, safe: &safe });
            let (decision, walked) = motor.walk_in_lane(route, pose, 5.0, |p: Vec3| clear || p.z.abs() < 0.5, aim);
            assert_eq!(walked, Walked::OnTheWay);
            (decision.turn.0, motor.lane_use)
        };
        // Without a lane the point is dead ahead (no turn); with one it is 0.9 m to the left of the route's point, 2.6 m on: a turn to the left, and the other way for the right.
        let (straight, idle) = walk(&mut line(), &pose, None, true);
        assert_eq!((straight.abs() < 1e-4, idle), (true, LaneUse::Idle));
        let (left, used) = walk(&mut line(), &pose, Some((0.9, true, true)), true);
        let (right, _) = walk(&mut line(), &pose, Some((-0.9, true, true)), true);
        assert_eq!(used, LaneUse::Applied);
        assert!(left.abs() > 0.05 && right.abs() > 0.05 && left * right < 0.0, "{left} {right}");
        // The shifted point is not one the body walks to in a line: the route's own point, counted.
        assert_eq!(walk(&mut line(), &pose, Some((0.9, true, true)), false), (straight, LaneUse::NoRoom));
        // Not the route's floor, or the body itself at a lip: the route's own point.
        assert_eq!(walk(&mut line(), &pose, Some((0.9, true, false)), true), (straight, LaneUse::UnsafeFloor(Unsafe::OtherFloor)));
        assert_eq!(walk(&mut line(), &pose, Some((0.9, false, true)), true), (straight, LaneUse::UnsafeFloor(Unsafe::BodyAtEdge)));
        // The route's last stretch (6 m): its own line.
        let near_end = Pose { feet: Vec3::new(26.0, 0.0, 0.0), yaw: yaw_of([1.0, 0.0]), pitch: 0.0, speed: 3.0, ..Default::default() };
        assert_eq!(walk(&mut line(), &near_end, Some((0.9, true, true)), true).1, LaneUse::RouteBusy);
        // A gate within reach: its own line.
        let gated = || Route::new(vec![p(0.0, 0.0), p(5.0, 0.0), p(30.0, 0.0)]).with_gate(Vec3::new(5.0, 0.0, 0.0));
        assert_eq!(walk(&mut gated(), &pose, Some((0.9, true, true)), true).1, LaneUse::RouteBusy);
    }

    /// B8 (Vector Laboratory, match 223ad08c bot 3, 138-200 s): a Drop of 3.58 m over 0.35 m across whose later legs come
    /// back under the way in (the route is a hairpin on one line). The route's distance is across the ground, so at the
    /// take-off the look-ahead (about 2.5 m, even 0.8 m under the lip cap) reaches the lower legs, horizontally the way the
    /// bot came from: it turns 115-173 degrees and walks back along the upper floor instead of off the edge.
    /// `Feel::drop_aim` cuts the look-ahead to the landing.
    #[test]
    fn at_a_hairpin_drops_take_off_the_walk_aims_at_the_landing_only_with_drop_aim() {
        let p = |x, y, z| Waypoint { at: Vec3::new(x, y, z), node: 0, jump: None, low: false };
        let hairpin = || {
            Route::new(vec![
                p(10.01, 21.0, -22.04),
                p(6.51, 21.03, -22.54),
                p(6.01, 21.07, -23.04),
                p(5.76, 17.49, -23.29),
                p(6.0, 17.47, -22.8),
                p(6.51, 17.45, -22.29),
                p(8.76, 17.45, -21.54),
            ])
        };
        let landing = Vec3::new(5.76, 17.49, -23.29);
        // The body at the take-off, walking.
        let feet = Vec3::new(6.01, 21.07, -23.04);
        let yaw = yaw_of([landing.x - feet.x, landing.z - feet.z]);
        let pose = Pose { feet, yaw: yaw + std::f32::consts::PI, pitch: 0.0, speed: 2.5, ..Default::default() };
        let turn = |feel: Feel, cap: Option<f32>, pose: &Pose| {
            let mut rng = Rng::new(3);
            let mut motor = Motor::new(Feel { sway: 0.0, ..feel }, &mut rng);
            motor.ahead_cap = cap;
            let (decision, walked) = motor.walk(&mut hairpin(), pose, 5.0, |_| true);
            assert_eq!(walked, Walked::OnTheWay);
            decision.turn.0
        };
        // Facing the landing already: with drop_aim the turn needed is nil; without, the aim lies behind and the
        // turn is the half circle.
        let facing = Pose { yaw, ..pose };
        let off = turn(feel_off(), None, &facing).abs();
        let capped = turn(feel_off(), Some(0.8), &facing).abs();
        let on = turn(Feel { drop_aim: true, ..feel_off() }, None, &facing).abs();
        println!("B8 turn when facing the landing: off {off:.3}, lip cap {capped:.3}, drop_aim {on:.3}");
        assert!(off > 0.05, "without the rule it turns away from the edge: {off}");
        assert!(capped > 0.05, "the lip cap alone still aims past the landing: {capped}");
        assert!(on < off * 0.3, "with drop_aim it stays on the landing: {on} {off}");
        // The rule changes nothing where there is no drop (a straight route).
        let straight = || Route::new(vec![p(0.0, 0.0, 0.0), p(5.0, 0.0, 0.0), p(10.0, 0.0, 0.0)]);
        let pose = Pose { feet: Vec3::new(1.0, 0.0, 0.0), yaw: yaw_of([1.0, 0.0]), speed: 3.0, ..Default::default() };
        let run = |drop_aim| {
            let mut rng = Rng::new(3);
            let mut motor = Motor::new(Feel { sway: 0.0, drop_aim, ..feel_off() }, &mut rng);
            motor.walk(&mut straight(), &pose, 5.0, |_| true).0
        };
        assert_eq!(run(false), run(true));
    }

    /// A doorway in a thin wall (School's spawn room exit, x 24.04-25.91 in the wall strip z 182.62-183.07): the route goes
    /// through it and turns west along the wall's far side. A body on the near side, level with the far leg, has no
    /// route point ahead it walks to in a line: steering at the unclear one walks it into the wall; with
    /// `Feel::unclear_aim` it steers back to the doorway's near point.
    fn doorway() -> (Route, impl Fn(Vec3) -> bool + Copy) {
        let p = |x, z| Waypoint { at: Vec3::new(x, 0.0, z), node: 0, jump: None, low: false };
        let route = Route::new(vec![
            p(22.45, 179.72),
            p(24.45, 182.72),
            p(24.45, 182.97),
            p(23.95, 183.47),
            p(19.2, 183.72),
            p(16.2, 180.47),
        ]);
        // A line from `a` to `b` crosses the wall strip unless it does so in the doorway.
        let crosses = |a: Vec3, b: Vec3| {
            let (lo, hi) = (182.62, 183.07);
            if (a.z < lo) == (b.z < lo) && (a.z > hi) == (b.z > hi) && (a.z < lo || a.z > hi) {
                return false;
            }
            if (a.z - b.z).abs() < 1e-4 {
                return a.z > lo && a.z < hi;
            }
            [lo, hi].iter().any(|&z| {
                let t = (z - a.z) / (b.z - a.z);
                (0.0..=1.0).contains(&t) && {
                    let x = a.x + (b.x - a.x) * t;
                    !(24.04..=25.91).contains(&x)
                }
            })
        };
        (route, move |point: Vec3| !crosses(Vec3::new(21.5, 0.0, 182.3), point))
    }

    #[test]
    fn with_no_clear_point_ahead_the_walk_steers_back_to_the_doorway_not_into_the_wall() {
        let (mut route, clear) = doorway();
        let feet = Vec3::new(21.5, 0.0, 182.3);
        let (s, off) = route.progress(feet);
        assert!(off < 2.0, "{off}");
        let mut rng = Rng::new(3);
        let motor = Motor::new(Feel { sway: 0.0, ..feel_off() }, &mut rng);
        // The body is on the far leg's nearest segment: past the doorway by the progress.
        assert!(s > 5.0, "{s}");
        // Off (the default): the point it steers at is one the wall is across.
        match motor.aim_point(&route, s, 2.4, &clear) {
            Aim::Unclear(point) => assert!(!clear(point), "{point:?}"),
            other => panic!("{other:?}"),
        }
        // On: a clear point of the route, back of the progress.
        let on = Motor::new(Feel { sway: 0.0, unclear_aim: true, ..feel_off() }, &mut rng);
        match on.aim_point(&route, s, 2.4, &clear) {
            Aim::Clear(point) => assert!(clear(point) && point.z < 183.0, "{point:?}"),
            Aim::None => {}
            other => panic!("{other:?}"),
        }
    }

    /// Cologne's deck: one long leg between two low ends over a floor 1.6 m higher in the middle. The line between the
    /// leg's ends lies under the floor, and the grid finds no floor under a point more than a metre under one
    /// (`line_ends`): the test of a point at that height says "not clear" where the body walks.
    fn hump() -> (Route, impl Fn(Vec3) -> bool + Copy) {
        let p = |x, y| Waypoint { at: Vec3::new(x, y, 0.0), node: 0, jump: None, low: false };
        let route = Route::over_floors(vec![p(0.0, 0.0), p(20.0, 0.0)], vec![(0.0, 0.0), (8.0, 1.6), (12.0, 1.6), (20.0, 0.0)]);
        let floor = |x: f32| (x.min(20.0 - x) * 0.2).clamp(0.0, 1.6);
        (route, move |point: Vec3| floor(point.x) - point.y <= 1.0)
    }

    #[test]
    fn a_hump_in_a_long_leg_is_not_a_wall_with_the_floor_heights() {
        let (route, clear) = hump();
        let motor = |feel: Feel| Motor::new(Feel { sway: 0.0, ..feel }, &mut Rng::new(3));
        let s = 9.0;
        // As it was: the point at the chord's height is not clear, off and with the rule on its retry finds the floor's.
        assert!(matches!(motor(feel_off()).aim_point(&route, s, 3.0, &clear), Aim::Unclear(_)));
        assert!(matches!(motor(Feel { unclear_aim: true, ..feel_off() }).aim_point(&route, s, 3.0, &clear), Aim::Clear(_)));
        // The floor heights: clear at once, at the look-ahead's full length, with the rule off too.
        match motor(Feel { floor_height_clear: true, ..feel_off() }).aim_point(&route, s, 3.0, &clear) {
            Aim::Clear(point) => assert!((point.x - 12.0).abs() < 1e-3 && (point.y - 1.6).abs() < 1e-3, "{point:?}"),
            other => panic!("{other:?}"),
        }
        // Off the hump nothing differs: the same point either way.
        let flat = |feel| motor(feel).aim_point(&route, 2.0, 1.0, &|_| true);
        assert_ne!(flat(Feel { floor_height_clear: true, ..feel_off() }), Aim::None);
    }

    /// The Mamayev Kurgan spawn exits (smoke 2, `exit_on_grid`): the platform's lip cell has only a Drop link toward the
    /// route's next points, so `clear` is false for them and right to be; the route itself goes over the ledge. The
    /// unclear look-ahead steers on (not back along the route, not off it) while a planned jump or drop is within reach,
    /// and keeps its answers where the unclear point is a wall.
    #[test]
    fn an_unclear_look_ahead_is_no_wall_at_a_planned_drop() {
        let p = |x, y, z| Waypoint { at: Vec3::new(x, y, z), node: 0, jump: None, low: false };
        let aim = |feel: Feel, route: &Route, s: f32, clear: &dyn Fn(Vec3) -> bool| {
            Motor::new(Feel { sway: 0.0, ..feel }, &mut Rng::new(3)).aim_point(route, s, 0.8, &clear)
        };
        let ua = || Feel { unclear_aim: true, ..Feel::default() };
        // Team 1's exit: the platform (y 29.14) to the next floor 0.95 m down, then on down; a body on the platform walks
        // in a line to the platform's points only.
        let team1 = Route::new(vec![
            p(98.6, 29.88, 136.1),
            p(97.9, 29.14, 136.1),
            p(97.4, 28.19, 136.6),
            p(97.1, 28.08, 136.9),
            p(96.9, 27.97, 136.9),
            p(96.4, 27.83, 137.4),
        ]);
        let platform = |q: Vec3| q.y > 28.9;
        assert!(team1.link_ahead(0.9, 1.3, 0.8));
        assert!(matches!(aim(ua(), &team1, 0.9, &platform), Aim::Unclear(_)), "steers on at the edge");
        // Team 0's exit: a 3.9 m drop in the second leg after the lip.
        let team0 = Route::new(vec![p(-25.1, 39.0, 142.6), p(-22.6, 37.4, 143.4), p(-22.1, 33.5, 143.4), p(-20.1, 33.8, 145.4)]);
        let top = |q: Vec3| q.y > 36.0;
        assert!(matches!(aim(ua(), &team0, 2.5, &top), Aim::Unclear(_)));
        // The rule off answers the same.
        assert_eq!(aim(Feel::default(), &team1, 0.9, &platform), aim(ua(), &team1, 0.9, &platform));
        // No drop within reach: the walls of before. The same heights without the drop (a flat way, a wall at x > 97.5):
        // steered back to the clear point behind, or off the route when none is.
        let flat = Route::new(vec![p(98.6, 29.1, 136.1), p(97.9, 29.1, 136.1), p(97.4, 29.1, 136.6), p(97.1, 29.1, 136.9)]);
        let wall = |q: Vec3| q.x > 97.6;
        assert!(!flat.link_ahead(0.9, 1.3, 0.8));
        assert!(matches!(aim(ua(), &flat, 0.9, &wall), Aim::Clear(_)), "steered back");
        assert_eq!(aim(ua(), &flat, 0.9, &|_| false), Aim::None);
        // A drop further than the look-ahead reaches is not one in reach.
        let far = Route::new(vec![p(0.0, 5.0, 0.0), p(20.0, 5.0, 0.0), p(21.0, 3.0, 0.0)]);
        assert!(!far.link_ahead(0.0, 2.0, 0.8));
        assert!(far.link_ahead(0.0, 20.5, 0.8));
        // A jump is a planned link as well.
        let leap = Waypoint { jump: Some(Leap::Ledge), ..p(2.0, 6.0, 0.0) };
        let hop = Route::new(vec![p(0.0, 5.0, 0.0), leap]);
        assert!(hop.link_ahead(0.0, 2.5, 0.8));
    }

    /// The walk reports the route lost (off it) when no route point back of the progress is clear either, so the drive
    /// plans again from where the body stands; and the rule changes nothing where the look-ahead is clear.
    #[test]
    fn a_clear_look_ahead_is_the_same_with_the_rule_on_and_off() {
        let p = |x, z| Waypoint { at: Vec3::new(x, 0.0, z), node: 0, jump: None, low: false };
        let pose = Pose { feet: Vec3::new(1.0, 0.0, 0.0), yaw: yaw_of([1.0, 0.0]), pitch: 0.0, speed: 3.0, ..Default::default() };
        let decide = |unclear_aim: bool| {
            let mut rng = Rng::new(3);
            let mut motor = Motor::new(Feel { sway: 0.0, unclear_aim, ..Feel::default() }, &mut rng);
            let mut route = Route::new(vec![p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0)]);
            let (decision, walked) = motor.walk(&mut route, &pose, 5.0, |_| true);
            (decision.keys, decision.turn.0.to_bits(), walked)
        };
        assert_eq!(decide(false), decide(true));
        // Nothing clear at all, the rule on: off the route; off: it walks on.
        let blocked = |unclear_aim: bool| {
            let mut rng = Rng::new(3);
            let mut motor = Motor::new(Feel { sway: 0.0, unclear_aim, ..Feel::default() }, &mut rng);
            let mut route = Route::new(vec![p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0)]);
            motor.walk(&mut route, &pose, 5.0, |_| false).1
        };
        assert_eq!(blocked(false), Walked::OnTheWay);
        assert!(matches!(blocked(true), Walked::Off(_)));
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

    /// A landing run carried across the route's way and the goal is braked by the keys against it once the feel
    /// says so (`landing_cancel_deg`); one along the way is not; one into a wall (nothing clear within its coast) is;
    /// off, the walk's keys are the route's as always. Counted either way.
    #[test]
    fn a_landing_run_against_the_way_is_braked_only_when_the_feel_says_so() {
        let p = |x: f32| Waypoint { at: Vec3::new(x, 0.0, 0.0), node: 0, jump: None, low: false };
        let keys_of = |deg: f32, velocity: [f32; 2], open: bool| {
            let mut rng = Rng::new(3);
            let mut motor = Motor::new(Feel { sway: 0.0, landing_cancel_deg: deg, ..feel_off() }, &mut rng);
            let at = motor.clock.wrapping_sub((0.8 / TICK_S) as u32);
            motor.last_press = Some(LandingWatch { at, counted: false, from: LandingFrom::Jump });
            let mut route = Route::new(vec![p(0.0), p(10.0)]);
            let pose = Pose {
                feet: Vec3::new(3.0, 0.0, 0.0),
                yaw: yaw_of([1.0, 0.0]),
                speed: velocity[0].hypot(velocity[1]),
                velocity,
                ..Default::default()
            };
            motor.walk(&mut route, &pose, 5.0, |_| open).0.keys
        };
        let before = landings::counts();
        // Across the way (z+ while the route goes x+): braked with the keys toward z-, no forward.
        let keys = keys_of(60.0, [0.0, 4.0], true);
        assert_eq!(keys & key::FORWARD, 0, "{keys:#x}");
        assert_ne!(keys & (key::LEFT | key::RIGHT), 0, "{keys:#x}");
        // Off: the route's keys.
        assert_ne!(keys_of(0.0, [0.0, 4.0], true) & key::FORWARD, 0);
        // Along the way: forward as ever.
        assert_ne!(keys_of(60.0, [4.0, 0.0], true) & key::FORWARD, 0);
        // Along the way but into a wall: braked, back.
        let keys = keys_of(60.0, [4.0, 0.0], false);
        assert_ne!(keys & key::BACK, 0, "{keys:#x}");
        assert_eq!(keys & key::FORWARD, 0, "{keys:#x}");
        // Too slow to be a run: no brake.
        assert_ne!(keys_of(60.0, [0.0, 1.0], true) & key::FORWARD, 0);
        let after = landings::counts();
        assert!(after[0].iter().sum::<u64>() >= before[0].iter().sum::<u64>() + 4);
        assert!(after[1].iter().sum::<u64>() >= before[1].iter().sum::<u64>() + 2);
    }

    /// The landing of a planned drop is watched from the take-off, counted either way, and braked like a jump's only
    /// with `landing_cancel_drops`: a run carried across the way (z while the route goes x) is braked, off the
    /// route's keys stay.
    #[test]
    fn a_drop_landing_run_against_the_way_is_braked_only_with_the_drop_switch() {
        let p = |x: f32, y: f32| Waypoint { at: Vec3::new(x, y, 0.0), node: 0, jump: None, low: false };
        let keys_of = |drops: bool| {
            let mut rng = Rng::new(3);
            let feel = Feel { sway: 0.0, landing_cancel_deg: 60.0, landing_cancel_drops: drops, ..Feel::default() };
            let mut motor = Motor::new(feel, &mut rng);
            let mut route = Route::new(vec![p(0.0, 0.0), p(5.0, 0.0), p(5.4, -2.0), p(12.0, -2.0)]);
            let at = |x: f32, y: f32, velocity: [f32; 2]| Pose {
                feet: Vec3::new(x, y, 0.0),
                yaw: yaw_of([1.0, 0.0]),
                speed: velocity[0].hypot(velocity[1]),
                velocity,
                ..Default::default()
            };
            // On the floor before the edge, then over it: the watch starts.
            motor.walk(&mut route, &at(4.9, 0.0, [2.0, 0.0]), 5.0, |_| true);
            motor.walk(&mut route, &at(5.1, 0.0, [2.0, 0.0]), 5.0, |_| true);
            assert!(motor.last_press.is_some_and(|w| w.from == LandingFrom::Drop));
            let mut keys = 0;
            for _ in 0..(0.8 / TICK_S) as u32 {
                keys = motor.walk(&mut route, &at(5.3, -1.5, [0.0, 4.0]), 5.0, |_| true).0.keys;
            }
            keys
        };
        let before = landings::drop_counts();
        let braked = keys_of(true);
        assert_eq!(braked & key::FORWARD, 0, "{braked:#x}");
        assert_ne!(braked & (key::LEFT | key::RIGHT), 0, "{braked:#x}");
        assert_ne!(keys_of(false) & key::FORWARD, 0);
        let after = landings::drop_counts();
        assert!(after[0].iter().sum::<u64>() >= before[0].iter().sum::<u64>() + 2);
        assert!(after[1].iter().sum::<u64>() > before[1].iter().sum::<u64>());
    }

    /// A jump link of 1.2 m (a sprint jump, only the clients' grid makes
    /// one) is lined up from further back, walked into at a sprint, and
    /// pressed only at speed; the press is counted.
    #[test]
    fn a_sprint_jump_is_walked_into_at_a_sprint_and_pressed_at_speed() {
        let p = |x: f32, y: f32, jump: bool| Waypoint {
            at: Vec3::new(x, y, 0.0),
            node: 0,
            jump: jump.then_some(ledge_to(y)),
            low: false,
        };
        let mut route = Route::new(vec![
            p(0.0, 0.0, false),
            p(8.0, 0.0, false),
            p(8.4, 1.2, true),
            p(14.0, 1.2, false),
        ]);
        let ledge = Ledge {
            take_off: Vec3::new(8.0, 0.0, 0.0),
            way: [1.0, 0.0],
            face: 0.2,
            reach: 1.2,
            gets_up: true,
            lifts: false,
        };
        let before = sprint_jumps::counts();
        let climb = climb_ledge(&mut route, &ledge, Vec3::ZERO, yaw_of([1.0, 0.0]), 400, false);
        assert!(climb.up, "{:?}", climb.poses.last());
        assert_eq!(climb.presses.len(), 1, "{:?}", climb.presses);
        let (_, at) = climb.presses[0];
        assert!(at.speed >= SPRINT_PRESS_SPEED_MS, "{at:?}");
        let sprinting = climb
            .poses
            .iter()
            .zip(&climb.decisions)
            .filter(|(pose, d)| {
                pose.feet.distance_xz(ledge.take_off) <= SPRINT_RUN_UP_M && d.keys & key::SPRINT != 0
            })
            .count();
        assert!(sprinting > 0, "sprint held on the run-up");
        let after = sprint_jumps::counts();
        assert!(after[0].iter().sum::<u64>() > before[0].iter().sum::<u64>(), "press counted");
    }

    /// The lift tests' route: a ledge 1.0 m high (the client's step is 0.8, the lift's ceiling 1.11), 0.6 m deep.
    fn lift_route() -> (Route, Ledge) {
        let p = |x: f32, y: f32, jump: bool| Waypoint {
            at: Vec3::new(x, y, 0.0),
            node: 0,
            jump: jump.then_some(ledge_to(y)),
            low: false,
        };
        let route = Route::new(vec![p(0.0, 0.0, false), p(8.0, 0.0, false), p(8.4, 1.0, true), p(14.0, 1.0, false)])
            .with_ledge_top(Vec3::new(8.4, 1.0, 0.0), 0.6);
        let ledge = Ledge { take_off: Vec3::new(8.0, 0.0, 0.0), way: [1.0, 0.0], face: 0.2, reach: 1.2, gets_up: true, lifts: true };
        (route, ledge)
    }

    /// A body in a fight runs at the face of a low top with the sprint held and presses no jump: the client lifts it
    /// (`Feel::fight_lift`). The lift is counted tried and up, by its band.
    #[test]
    fn a_body_in_a_fight_lifts_a_low_top_with_no_jump() {
        let (mut route, ledge) = lift_route();
        let before = lifts::counts();
        let climb = climb_ledge_urgent(&mut route, &ledge, Vec3::ZERO, yaw_of([1.0, 0.0]), 400, false, 0, Feel { fight_lift: true, ..Feel::default() }, true);
        assert!(climb.presses.is_empty(), "no press: {:?}", climb.presses);
        assert!(climb.up, "{:?}", climb.poses.last());
        let after = lifts::counts();
        let band = 2; // 0.95-1.05
        assert!(after[0][band] > before[0][band], "tried");
        assert!(after[1][band] > before[1][band], "up");
    }

    /// A trace (not a check): a running body on a long straight to a 1.0 m ledge, speed and keys by tick, `--nocapture`.
    #[test]
    #[ignore = "a trace for reading, not a check"]
    fn trace_a_walking_jump_from_a_sprint() {
        let p = |x: f32, y: f32, jump: bool| Waypoint { at: Vec3::new(x, y, 0.0), node: 0, jump: jump.then_some(ledge_to(y)), low: false };
        let mut route = Route::new(vec![p(0.0, 0.0, false), p(20.0, 0.0, false), p(20.4, 1.0, true), p(40.0, 1.0, false)])
            .with_ledge_top(Vec3::new(20.4, 1.0, 0.0), 1.0);
        let ledge = Ledge { take_off: Vec3::new(20.0, 0.0, 0.0), way: [1.0, 0.0], face: 0.2, reach: 1.6, gets_up: true, lifts: false };
        let feel = Feel { sprint_through: false, fight_lift: false, ..Feel::default() };
        let climb = climb_ledge_urgent(&mut route, &ledge, Vec3::ZERO, yaw_of([1.0, 0.0]), 700, false, 0, feel, false);
        for (t, (pose, d)) in climb.poses.iter().zip(&climb.decisions).enumerate() {
            if pose.feet.x > 12.0 {
                eprintln!("t {t:3} x {:6.2} y {:4.2} speed {:4.2} sprint {} jump {} fwd {}", pose.feet.x, pose.feet.y, pose.speed,
                    u8::from(d.keys & key::SPRINT != 0), u8::from(d.keys & key::JUMP != 0), u8::from(d.keys & key::FORWARD != 0));
            }
        }
    }

    /// A running body keeps its sprint into a walking jump up a deep top and presses it on the move, at speed, 0.6 m or more
    /// before the face; a shallow top, or the switch off, is lined up at a walk as before.
    #[test]
    fn a_running_body_jumps_a_deep_top_at_a_sprint() {
        let (route, ledge) = lift_route();
        let pressed_at = |top: f32, through: bool| {
            let mut route = Route::new(route.points().to_vec()).with_ledge_top(Vec3::new(8.4, 1.0, 0.0), top);
            let feel = Feel { sprint_through: through, fight_lift: false, ..Feel::default() };
            let mut ledge = Ledge { lifts: false, ..ledge_copy(&ledge) };
            ledge.reach = 1.6;
            let climb = climb_ledge_urgent(&mut route, &ledge, Vec3::ZERO, yaw_of([1.0, 0.0]), 500, false, 0, feel, false);
            assert!(climb.up, "top {top} through {through}: {:?}", climb.poses.last());
            climb.presses.first().map(|&(_, at)| (at.speed, ledge.take_off.x - at.feet.x))
        };
        let (speed, before) = pressed_at(3.5, true).expect("pressed");
        assert!(speed >= SPRINT_PRESS_SPEED_MS && before >= 0.6, "speed {speed} before {before}");
        let (slow, _) = pressed_at(3.5, false).expect("pressed");
        assert!(slow < SPRINT_PRESS_SPEED_MS, "walked into: {slow}");
        let (shallow, _) = pressed_at(1.0, true).expect("pressed");
        assert!(shallow < SPRINT_PRESS_SPEED_MS, "a shallow top is walked into: {shallow}");
    }

    fn ledge_copy(l: &Ledge) -> Ledge {
        Ledge { take_off: l.take_off, way: l.way, face: l.face, reach: l.reach, gets_up: l.gets_up, lifts: l.lifts }
    }

    /// A calm body jumps the same top as it always has, pressed before the face, and tries no lift.
    #[test]
    fn a_calm_body_jumps_a_low_top() {
        let (mut route, ledge) = lift_route();
        let before = lifts::counts();
        let climb = climb_ledge_urgent(&mut route, &ledge, Vec3::ZERO, yaw_of([1.0, 0.0]), 400, false, 0, Feel::default(), false);
        assert_eq!(climb.presses.len(), 1, "{:?}", climb.presses);
        let (_, at) = climb.presses[0];
        assert!(ledge.take_off.x - at.feet.x >= 0.6, "pressed at least 0.6 m before the face: {at:?}");
        assert!(climb.up);
        let after = lifts::counts();
        assert!(after[3][2] > before[3][2], "a calm jump");
    }

    /// With the switch off a body in a fight jumps too.
    #[test]
    fn a_fight_lift_switched_off_is_a_jump() {
        let (mut route, ledge) = lift_route();
        let feel = Feel { fight_lift: false, ..Feel::default() };
        let climb = climb_ledge_urgent(&mut route, &ledge, Vec3::ZERO, yaw_of([1.0, 0.0]), 400, false, 0, feel, true);
        assert_eq!(climb.presses.len(), 1, "{:?}", climb.presses);
        assert!(climb.up);
    }

    /// A top the planner did not find deep enough or did not measure, a body the client lets take no sprint: jumped, not
    /// lifted, and the refusal counted.
    #[test]
    fn a_fight_does_not_lift_what_the_client_does_not() {
        let jumped = |route: &mut Route, ledge: &Ledge, block: Option<SprintBlock>| {
            let mut rng = Rng::new(17);
            let mut motor = Motor::new(Feel { sway: 0.0, fight_lift: true, ..Feel::default() }, &mut rng);
            let mut pose = Pose { feet: Vec3::ZERO, yaw: yaw_of([1.0, 0.0]), urgent: true, sprint_block: block, ..Default::default() };
            let mut blend = crate::v026g::bots::kin::Blend::default();
            let kin = Kinematics::default();
            let mut pressed = false;
            for _ in 0..300 {
                let (decision, _) = motor.walk(route, &pose, 5.0, |_| true);
                pressed |= decision.keys & key::JUMP != 0;
                pose.yaw = wrap(pose.yaw + decision.turn.0);
                let local = blend.tick(kin.target_local(decision.keys, 1.0), kin.blend_ticks);
                let velocity = crate::v026g::bots::kin::to_world(local, pose.yaw);
                let x = (pose.feet.x + velocity[0] * TICK_S).min(ledge.take_off.x + ledge.face);
                pose.speed = (x - pose.feet.x).abs() / TICK_S;
                pose.velocity = velocity;
                pose.feet.x = x;
                if pressed {
                    break;
                }
            }
            pressed
        };
        let (route, ledge) = lift_route();
        let before = lifts::refusals();
        let mut shallow = Route::new(route.points().to_vec()).with_ledge_top(Vec3::new(8.4, 1.0, 0.0), 0.2);
        assert!(jumped(&mut shallow, &ledge, None), "a shallow top is jumped");
        let mut unknown = Route::new(route.points().to_vec());
        assert!(jumped(&mut unknown, &ledge, None), "an unmeasured top is jumped");
        let mut carried = route.clone();
        assert!(jumped(&mut carried, &ledge, Some(SprintBlock::Carrying)), "a carrier jumps");
        let after = lifts::refusals();
        assert!(after[0] >= before[0] + 2, "top short counted: {before:?} {after:?}");
        assert!(after[1] > before[1], "blocked counted: {before:?} {after:?}");
    }

    /// A body with no clear run before the face does not lift: the jump is pressed from where it stands.
    #[test]
    fn a_fight_with_no_run_before_the_face_jumps() {
        let (mut route, ledge) = lift_route();
        let mut rng = Rng::new(17);
        let mut motor = Motor::new(Feel { sway: 0.0, fight_lift: true, ..Feel::default() }, &mut rng);
        motor.walking = true;
        let pose = Pose { feet: ledge.take_off, yaw: yaw_of([1.0, 0.0]), speed: 1.0, urgent: true, ..Default::default() };
        let before = lifts::refusals();
        let mut pressed = false;
        for _ in 0..60 {
            let (decision, _) = motor.walk(&mut route, &pose, 5.0, |p| p.x >= ledge.take_off.x - 0.5);
            pressed |= decision.keys & key::JUMP != 0;
        }
        assert!(pressed, "a body with no run jumps");
        assert!(lifts::refusals()[2] > before[2], "no run counted");
    }

    /// A lift that has stood at the face for [`LIFT_WAIT_S`] and is not up is given up for the jump, counted failed.
    #[test]
    fn a_lift_that_does_not_get_up_is_a_jump_after_a_second() {
        let (mut route, mut ledge) = lift_route();
        ledge.lifts = false; // the client does not lift here
        let before = lifts::counts();
        let climb = climb_ledge_urgent(&mut route, &ledge, Vec3::ZERO, yaw_of([1.0, 0.0]), 600, false, 0, Feel { fight_lift: true, ..Feel::default() }, true);
        assert_eq!(climb.presses.len(), 1, "the jump is pressed after the lift: {:?}", climb.presses);
        assert!(climb.up, "and gets up: {:?}", climb.poses.last());
        let after = lifts::counts();
        assert!(after[2][2] > before[2][2], "failed");
        assert!(after[4][2] > before[4][2], "the jump pressed in a fight");
    }

    /// A lift that reached its face and is not asked for again (the walk aims, the route goes on) is left after
    /// [`LIFT_RESUME_S`]: counted left with its cause, the jump is the way up, and it is no longer a lift at its face.
    #[test]
    fn a_lift_not_asked_for_again_is_left_with_its_cause() {
        let mut rng = Rng::new(3);
        let mut motor = Motor::new(Feel::default(), &mut rng);
        let (take_off, landing) = (Vec3::new(8.0, 0.0, 0.0), Vec3::new(8.4, 1.0, 0.0));
        motor.jump_try = Some(JumpTry {
            take_off,
            landing,
            tries: 0,
            pressed: None,
            back: false,
            counted: false,
            sprint_counted: false,
            band: BandTry::Approaching,
            lift: LiftTry::Lifting { at_face: Some(0) },
            lift_noted: false,
            lift_seen: Some(0),
            lift_refused_at_face: Some(lifts::Refused::Blocked),
        });
        let before = lifts::left_counts();
        let band = 2; // 0.95-1.05
        motor.follow_lift(take_off);
        assert_eq!(lifts::left_counts(), before, "just seen: not left yet");
        motor.clock = (LIFT_RESUME_S / TICK_S) as u32 + 2;
        assert!(!motor.lift_at_face(), "not seen this tick: no lift at its face");
        motor.follow_lift(take_off);
        let after = lifts::left_counts();
        assert_eq!(after[1][band], before[1][band] + 1, "left for a blocked sprint");
        assert_eq!(motor.jump_try.map(|t| t.lift), Some(LiftTry::GaveUp));
        motor.follow_lift(take_off);
        assert_eq!(lifts::left_counts(), after, "counted once");
    }

    /// A walking jump up a ledge with a short top is pressed farther before its take-off, when the route knows
    /// ([`Route::ledge_lead`]): the clip's root then ends nearer the face and the body stays on the top.
    #[test]
    fn a_ledge_with_a_short_top_is_pressed_farther_before_it() {
        let p = |x: f32, y: f32, jump: bool| Waypoint {
            at: Vec3::new(x, y, 0.0),
            node: 0,
            jump: jump.then_some(ledge_to(y)),
            low: false,
        };
        let ledge = Ledge { take_off: Vec3::new(8.0, 0.0, 0.0), way: [1.0, 0.0], face: 0.2, reach: 1.2, gets_up: true, lifts: false };
        let press_at = |lead: Option<f32>| {
            let mut route = Route::new(vec![p(0.0, 0.0, false), p(8.0, 0.0, false), p(8.4, 0.9, true), p(14.0, 0.9, false)]);
            if let Some(lead) = lead {
                route = route.with_ledge_lead(Vec3::new(8.4, 0.9, 0.0), lead);
            }
            let climb = climb_ledge(&mut route, &ledge, Vec3::ZERO, yaw_of([1.0, 0.0]), 400, false);
            climb.presses.first().map(|(_, at)| at.feet.distance_xz(ledge.take_off))
        };
        let usual = press_at(None).expect("the usual press");
        let early = press_at(Some(1.4)).expect("the early press");
        assert!(early > usual + 0.3 && early > 1.0, "usual {usual:.2} m before the take-off, early {early:.2} m");
    }

    /// A sprint-band ledge jump (1.2 m, over the walk jump's 1.09) of a body that starts at the take-off, at rest: without
    /// `band_jump_gate` the old fallback presses it there at a walk (the client makes a walk jump of that, which
    /// does not get up); with it the body backs off for a run-up and presses only at speed with the sprint held.
    #[test]
    fn a_band_jump_is_not_pressed_at_a_walk_with_the_gate() {
        let p = |x: f32, y: f32, jump: bool| Waypoint {
            at: Vec3::new(x, y, 0.0),
            node: 0,
            jump: jump.then_some(ledge_to(y)),
            low: false,
        };
        let ledge = Ledge {
            take_off: Vec3::new(8.0, 0.0, 0.0),
            way: [1.0, 0.0],
            face: 0.2,
            reach: 1.2,
            gets_up: true,
            lifts: false,
        };
        let presses = |gate: bool| {
            let mut route = Route::new(vec![p(0.0, 0.0, false), p(8.0, 0.0, false), p(8.4, 1.2, true), p(14.0, 1.2, false)]);
            let feel = Feel { band_jump_gate: gate, ..Feel::default() };
            climb_ledge_feel(&mut route, &ledge, Vec3::new(8.0, 0.0, 0.0), yaw_of([1.0, 0.0]), 600, false, 0, feel)
        };
        let before = band_jumps::counts();
        let off = presses(false);
        assert!(off.presses.first().is_some_and(|(_, at)| at.speed < SPRINT_PRESS_SPEED_MS), "{:?}", off.presses);
        let on = presses(true);
        assert!(!on.presses.is_empty(), "it runs up and jumps: {:?}", on.poses.last());
        assert!(on.presses.iter().all(|(_, at)| at.speed >= SPRINT_PRESS_SPEED_MS), "{:?}", on.presses);
        let after = band_jumps::counts();
        let refusals = band_jumps::Slot::RefusedSlow as usize..=band_jumps::Slot::RefusedDirection as usize;
        assert!(after[refusals.clone()].iter().sum::<u64>() > before[refusals].iter().sum::<u64>(), "the slow arrival was refused");
        assert!(on.up, "{:?}", on.poses.last());
    }

    /// A link the grid labels a sprint jump but of 1.07 m (over the grid's cap, under the walk jump's 1.09): without
    /// `band_from_grid` the gate leaves it to the old fallback, pressed at the take-off at a walk (0 of 53 such presses got up in
    /// the stand-in); with it the link is in the band, run up to and pressed only at speed with the sprint held.
    #[test]
    fn a_link_the_grid_calls_a_sprint_jump_is_in_the_band_whatever_its_rise() {
        let p = |x: f32, y: f32, jump: bool| Waypoint {
            at: Vec3::new(x, y, 0.0),
            node: 0,
            jump: jump.then_some(ledge_to(y)),
            low: false,
        };
        let ledge = Ledge { take_off: Vec3::new(8.0, 0.0, 0.0), way: [1.0, 0.0], face: 0.2, reach: 1.2, gets_up: true, lifts: false };
        let presses = |from_grid: bool| {
            let mut route = Route::new(vec![p(0.0, 0.0, false), p(8.0, 0.0, false), p(8.4, 1.07, true), p(14.0, 1.07, false)]);
            assert_eq!(route.points()[2].jump, Some(Leap::SprintLedge));
            let feel = Feel { band_jump_gate: true, band_from_grid: from_grid, ..Feel::default() };
            climb_ledge_feel(&mut route, &ledge, Vec3::new(8.0, 0.0, 0.0), yaw_of([1.0, 0.0]), 600, false, 0, feel)
        };
        let old = presses(false);
        assert!(old.presses.first().is_some_and(|(_, at)| at.speed < SPRINT_PRESS_SPEED_MS), "{:?}", old.presses);
        let new = presses(true);
        assert!(!new.presses.is_empty(), "it runs up and jumps: {:?}", new.poses.last());
        assert!(new.presses.iter().all(|(_, at)| at.speed >= SPRINT_PRESS_SPEED_MS), "{:?}", new.presses);
    }

    /// At a band jump's take-off at speed with the sprint held, a block the client's `can_sprint` says (aiming here) holds
    /// the press, backs the body off (a try spent) and is counted by its reason; after `BAND_REFUSALS` the face is
    /// given up so the route is planned again; the lining up keeps the sprint under the switch only.
    #[test]
    fn a_band_jump_waits_for_what_can_sprint_asks_and_is_given_up_after_the_refusals() {
        let p = |x: f32, y: f32, jump: bool| Waypoint {
            at: Vec3::new(x, y, 0.0),
            node: 0,
            jump: jump.then_some(ledge_to(y)),
            low: false,
        };
        let route = || Route::new(vec![p(0.0, 0.0, false), p(8.0, 0.0, false), p(8.4, 1.2, true), p(14.0, 1.2, false)]);
        let run = |gate: bool, block: Option<SprintBlock>| {
            let mut rng = Rng::new(3);
            let mut motor = Motor::new(Feel { band_jump_gate: gate, sway: 0.0, ..Feel::default() }, &mut rng);
            let mut route = route();
            let mut keys = Vec::new();
            let mut walked = Walked::OnTheWay;
            for _ in 0..3 {
                motor.walking = true;
                motor.sprinting = true;
                motor.facing_ticks = 100;
                let pose = Pose {
                    feet: Vec3::new(8.0, 0.0, 0.0),
                    yaw: yaw_of([1.0, 0.0]),
                    speed: 5.0,
                    velocity: [5.0, 0.0],
                    sprint_block: block,
                    ..Default::default()
                };
                for _ in 0..30 {
                    let (decision, w) = motor.walk(&mut route, &pose, 5.0, |_| true);
                    keys.push(decision.keys);
                    walked = w;
                }
            }
            (keys, walked, motor.keeps_sprint())
        };
        let before = band_jumps::counts();
        let (open, _, _) = run(true, None);
        assert!(open.iter().any(|k| k & key::JUMP != 0), "at speed, no block: it jumps");
        let (blocked, _, _) = run(true, Some(SprintBlock::Aiming));
        assert!(blocked.iter().all(|k| k & key::JUMP == 0), "aiming: no sprint jump");
        let after = band_jumps::counts();
        let aiming = band_jumps::Slot::RefusedAiming as usize;
        assert!(after[aiming] > before[aiming], "counted as aiming: {after:?}");
        let (_, _, keeps) = run(true, None);
        assert!(keeps, "the sprint is kept for the lining up");
        assert!(!run(false, None).2, "off: as before");
    }

    /// A sprint gap link's lining up keeps the sprint under `band_jump_gate` (and only then).
    #[test]
    fn a_sprint_gap_keeps_the_sprint_through_the_lining_up_only_with_the_gate() {
        let p = |x: f32, y: f32, z: f32, jump: bool| Waypoint {
            at: Vec3::new(x, y, z),
            node: 0,
            jump: jump.then_some(Leap::SprintGap(mapdata::PressLead::from_metres(0.5))),
            low: false,
        };
        let keeps = |gate: bool| {
            let mut rng = Rng::new(3);
            let mut motor = Motor::new(Feel { sprint_gaps: true, band_jump_gate: gate, sway: 0.0, ..Feel::default() }, &mut rng);
            let mut route =
                Route::new(vec![p(0.0, 0.0, 20.0, false), p(0.0, 0.0, 0.0, false), p(3.0, -1.7, 0.0, true), p(6.0, -1.7, 0.0, false)]);
            motor.walking = true;
            motor.sprinting = true;
            let pose = Pose { feet: Vec3::new(0.0, 0.0, 2.0), yaw: yaw_of([0.0, -1.0]), speed: 5.0, ..Default::default() };
            motor.walk(&mut route, &pose, 5.0, |_| true);
            (motor.lining_up(), motor.keeps_sprint())
        };
        assert_eq!(keeps(true), (true, true));
        assert_eq!(keeps(false), (true, false));
    }

    /// With the reckoned bar the sprint is let go of at the empty bar and kept let go of until the bar is over 20,
    /// and for a sprint jump ahead that the bar could not pay (the sprint to it, its run-up, the jump, a margin);
    /// without the gauge, or with the bar full, it goes on.
    #[test]
    fn the_sprint_is_let_go_of_by_the_reckoned_bar() {
        use crate::v026g::bots::stamina::Gauge;
        let p = |x: f32, y: f32, jump: bool| Waypoint {
            at: Vec3::new(x, y, 0.0),
            node: 0,
            jump: jump.then_some(ledge_to(y)),
            low: false,
        };
        let route = Route::new(vec![p(0.0, 0.0, false), p(20.0, 0.0, false), p(20.4, 1.2, true), p(40.0, 1.2, false)]);
        let sprinting_after = |gauge: Option<Gauge>, tired: bool| {
            let mut rng = Rng::new(3);
            let mut motor = Motor::new(Feel::default(), &mut rng);
            motor.sprinting = true;
            motor.tired = tired;
            motor.gauge = gauge;
            motor.sprint_by_the_bar(&route, 0.0);
            motor.sprinting
        };
        let bar = |value: f32, low: bool| Some(Gauge { value, low });
        assert!(sprinting_after(None, false), "no gauge: as before");
        assert!(sprinting_after(bar(100.0, false), false), "a full bar pays the 20 m, the run-up and the jump");
        assert!(!sprinting_after(bar(30.0, false), false), "30 - 22 is under the jump and the margin");
        assert!(!sprinting_after(bar(2.0, false), false), "empty for the sprint");
        assert!(!sprinting_after(bar(0.0, true), false), "locked out");
        assert!(!sprinting_after(bar(10.0, false), true), "tired until the bar is over 20");
        // The jump 20 m on is a sprint jump (1.2 m): with a walk jump (0.5 m) it wants no run-up.
        let walk = Route::new(vec![p(0.0, 0.0, false), p(20.0, 0.0, false), p(20.4, 0.5, true), p(40.0, 0.5, false)]);
        let mut rng = Rng::new(3);
        let mut motor = Motor::new(Feel::default(), &mut rng);
        motor.sprinting = true;
        motor.gauge = bar(40.0, false);
        motor.sprint_by_the_bar(&walk, 0.0);
        assert!(motor.sprinting, "40 - 20 pays a walk jump and the margin");
    }

    /// The life's reserve (`pace.reserve`) moves the lockout's 3 and 20: the sprint is let go of at the drawn stop, not taken up
    /// again before the bar is over stop + gap, and the draw is a life's own (two lives, two numbers; the same seed, the same).
    #[test]
    fn the_sprint_is_let_go_of_at_the_lifes_reserve_and_taken_up_over_its_gap() {
        use crate::v026g::bots::stamina::Gauge;
        use crate::v026g::bots::tuning::Reserve;
        use crate::v026g::bots::tuning::Span;
        let route = Route::new(vec![
            Waypoint { at: Vec3::new(0.0, 0.0, 0.0), node: 0, jump: None, low: false },
            Waypoint { at: Vec3::new(60.0, 0.0, 0.0), node: 0, jump: None, low: false },
        ]);
        let bar = |value: f32| Some(Gauge { value, low: false });
        let after = |reserve: Option<(f32, f32)>, value: f32, tired: bool| {
            let mut rng = Rng::new(3);
            let mut motor = Motor::new(Feel::default(), &mut rng);
            motor.sprinting = true;
            motor.tired = tired;
            motor.reserve = reserve;
            motor.gauge = bar(value);
            motor.sprint_by_the_bar(&route, 0.0);
            (motor.sprinting, motor.released_by_bar())
        };
        let reserve = Some((20.0, 45.0));
        assert_eq!(after(None, 15.0, false), (true, false), "the old lockout: 15 is over the 3");
        assert_eq!(after(reserve, 15.0, false), (false, true), "under the stop of 20");
        assert_eq!(after(reserve, 30.0, true), (false, true), "tired until the bar is over 45");
        assert_eq!(after(reserve, 50.0, true), (true, false), "back over the start");
        assert_eq!(after(reserve, 30.0, false), (true, false), "between: sprinting goes on");
        // The draw: inside the spans, repeatable, and a life's own.
        let tuning = Reserve { stop: Span::of(10.0, 25.0), gap: Span::of(15.0, 30.0) };
        let draw = |seed: u64| {
            let mut rng = Rng::new(seed);
            let mut motor = Motor::new(Feel::default(), &mut Rng::new(1));
            motor.reserve_for_life(Some(&tuning), &mut rng);
            motor.reserve.expect("drawn")
        };
        let (stop, start) = draw(5);
        assert!((10.0..25.0).contains(&stop) && (stop + 15.0..stop + 30.0).contains(&start), "{stop} {start}");
        assert_eq!(draw(5), (stop, start));
        assert_ne!(draw(6), (stop, start));
        let mut none = Motor::new(Feel::default(), &mut Rng::new(1));
        none.reserve_for_life(None, &mut Rng::new(5));
        assert_eq!(none.reserve, None);
    }

    /// A sprint-band press is aligned when the body's speed along the jump's way is the press speed or more, within 30
    /// degrees of it: 5 m/s straight on is, 5 m/s at 45 degrees (3.5 along) is not, and 4.2 m/s straight on is.
    #[test]
    fn a_band_press_needs_the_run_along_the_jumps_way() {
        let way = [1.0, 0.0];
        assert!(press_aligned([5.0, 0.0], way));
        assert!(press_aligned([4.2, 0.0], way));
        let d45 = 5.0 * std::f32::consts::FRAC_1_SQRT_2;
        assert!(!press_aligned([d45, d45], way), "3.5 m/s of 5 along the way");
        let d25 = 25.0_f32.to_radians();
        assert!(press_aligned([5.0 * d25.cos(), 5.0 * d25.sin()], way), "25 degrees, 4.5 along");
        let d35 = 35.0_f32.to_radians();
        assert!(!press_aligned([6.0 * d35.cos(), 6.0 * d35.sin()], way), "35 degrees is over the 30");
        assert!(!press_aligned([3.9, 0.0], way), "too slow");
    }

    /// A tired body does not take the sprint up to walk into a sprint jump (it walks, the bar comes back); a rested one
    /// does. A sprint gap link's own run is priced like a ledge's run-up: a bar that pays the way to the gap but not
    /// its run lets the sprint go.
    #[test]
    fn a_tired_body_walks_into_a_sprint_jump_and_a_gap_run_is_priced() {
        use crate::v026g::bots::stamina::Gauge;
        let ledge = |tired_bar: f32| {
            let p = |x: f32, y: f32, jump: bool| Waypoint {
                at: Vec3::new(x, y, 0.0),
                node: 0,
                jump: jump.then_some(ledge_to(y)),
                low: false,
            };
            let mut rng = Rng::new(3);
            let mut motor = Motor::new(Feel { sway: 0.0, ..Feel::default() }, &mut rng);
            motor.gauge = Some(Gauge { value: tired_bar, low: false });
            motor.walking = true;
            let mut route = Route::new(vec![p(0.0, 0.0, false), p(8.0, 0.0, false), p(8.4, 1.2, true), p(14.0, 1.2, false)]);
            let pose = Pose { feet: Vec3::new(6.0, 0.0, 0.0), yaw: yaw_of([1.0, 0.0]), speed: 4.0, ..Default::default() };
            motor.walk(&mut route, &pose, 5.0, |_| true);
            motor.sprinting
        };
        assert!(ledge(100.0), "a full bar: the sprint is taken up for the sprint jump");
        assert!(!ledge(2.0), "a spent bar: it walks");
        let gap = |bar: f32| {
            let p = |x: f32, jump: bool| Waypoint {
                at: Vec3::new(x, if jump { -1.7 } else { 0.0 }, 0.0),
                node: 0,
                jump: jump.then_some(Leap::SprintGap(mapdata::PressLead::from_metres(0.5))),
                low: false,
            };
            let route = Route::new(vec![p(-20.0, false), p(0.0, false), p(3.0, true), p(6.0, false)]);
            let mut rng = Rng::new(3);
            let mut motor = Motor::new(Feel { sprint_gaps: true, sway: 0.0, ..Feel::default() }, &mut rng);
            motor.sprinting = true;
            motor.gauge = Some(Gauge { value: bar, low: false });
            motor.sprint_by_the_bar(&route, 19.5);
            motor.sprinting
        };
        // 0.5 m to the take-off: the way costs 0.5, the gap's 1.5 m run 1.5 more, the jump and its margin 15.
        assert!(gap(18.0), "pays the way, the run and the jump");
        assert!(!gap(16.0), "pays the way and the jump, not the run");
    }

    /// With `drop_walk` the sprint is let go of 3 m before the edge of a planned drop, so the body is walked off it;
    /// far from the edge, or with the switch off, the sprint of a long straight stretch goes on.
    #[test]
    fn a_planned_drop_is_walked_off_not_sprinted_into_the_air() {
        let p = |x: f32, y: f32| Waypoint { at: Vec3::new(x, y, 0.0), node: 0, jump: None, low: false };
        let route = || Route::new(vec![p(0.0, 0.0), p(20.0, 0.0), p(20.5, -3.0), p(60.0, -3.0)]);
        let keys_at = |on: bool, x: f32| {
            let mut rng = Rng::new(3);
            let mut motor = Motor::new(Feel { drop_walk: on, sway: 0.0, ..Feel::default() }, &mut rng);
            motor.walking = true;
            motor.sprinting = true;
            motor.facing_ticks = 100;
            let pose = Pose { feet: Vec3::new(x, 0.0, 0.0), yaw: yaw_of([1.0, 0.0]), speed: 5.0, ..Default::default() };
            motor.walk(&mut route(), &pose, 5.0, |_| true).0.keys
        };
        assert_ne!(keys_at(true, 5.0) & key::SPRINT, 0, "far from the edge the sprint goes on");
        assert_eq!(keys_at(true, 18.0) & key::SPRINT, 0, "3 m from the edge: walked off");
        assert_ne!(keys_at(false, 18.0) & key::SPRINT, 0, "off: as before");
    }

    /// A planned drop the planner found safe at a sprint is sprinted off (the release is skipped, counted), one it did
    /// not is walked off as before; a descent by walk links only (a ramp, stairs: a planner's route with no drop
    /// recorded) is no drop, the sprint goes on.
    #[test]
    fn a_drop_safe_at_a_sprint_is_sprinted_off() {
        let p = |x: f32, y: f32| Waypoint { at: Vec3::new(x, y, 0.0), node: 0, jump: None, low: false };
        let points = vec![p(0.0, 0.0), p(20.0, 0.0), p(20.5, -3.0), p(60.0, -3.0)];
        let keys_at = |safe: bool| {
            let mut route = Route::new(points.clone());
            if safe {
                route = route.with_sprint_safe(vec![Vec3::new(20.5, -3.0, 0.0)]);
            }
            let mut rng = Rng::new(3);
            let mut motor = Motor::new(Feel { drop_walk: true, sway: 0.0, ..Feel::default() }, &mut rng);
            motor.walking = true;
            motor.sprinting = true;
            motor.facing_ticks = 100;
            let pose = Pose { feet: Vec3::new(18.0, 0.0, 0.0), yaw: yaw_of([1.0, 0.0]), speed: 5.0, ..Default::default() };
            motor.walk(&mut route, &pose, 5.0, |_| true).0.keys
        };
        assert_ne!(keys_at(true) & key::SPRINT, 0, "safe at a sprint: the sprint goes on");
        assert_eq!(keys_at(false) & key::SPRINT, 0, "not known to be safe: walked off");
    }

    /// A descent by walk links only (a planner's route with no drop recorded: stairs, a ramp, 3 m down in 20 m) is no drop:
    /// with `drop_walk` the sprint goes on along it; a route nothing is known of (made by hand) is released as before.
    #[test]
    fn a_planned_descent_by_walk_links_keeps_the_sprint_with_drop_walk() {
        let p = |x: f32, y: f32| Waypoint { at: Vec3::new(x, y, 0.0), node: 0, jump: None, low: false };
        let keys_at = |planned: bool| {
            let mut route = Route::new(vec![p(0.0, 0.0), p(20.0, -3.0), p(60.0, -3.0)]);
            if planned {
                route = route.planned_by_walks();
            }
            let mut rng = Rng::new(3);
            let mut motor = Motor::new(Feel { drop_walk: true, sway: 0.0, ..Feel::default() }, &mut rng);
            motor.walking = true;
            motor.sprinting = true;
            motor.facing_ticks = 100;
            let pose = Pose { feet: Vec3::new(8.0, -1.2, 0.0), yaw: yaw_of([1.0, 0.0]), speed: 5.0, ..Default::default() };
            motor.walk(&mut route, &pose, 5.0, |_| true).0.keys
        };
        assert_ne!(keys_at(true) & key::SPRINT, 0, "a descent by walk links: sprinted");
        assert_eq!(keys_at(false) & key::SPRINT, 0, "nothing known of the route: released as before");
    }

    /// A sprint gap link (`Leap::SprintGap`, `Feel::sprint_gaps`) is run up to at a sprint along the route, the run
    /// turning onto the jump's way over its last 2 m with no stop to line up, and pressed at the link's lead, at speed,
    /// facing the way; off, it is a walking gap jump (no sprint held).
    #[test]
    fn a_sprint_gap_link_is_run_up_to_at_a_sprint_and_pressed_at_speed() {
        let p = |x: f32, y: f32, z: f32, jump: bool| Waypoint {
            at: Vec3::new(x, y, z),
            node: 0,
            jump: jump.then_some(Leap::SprintGap(mapdata::PressLead::from_metres(0.5))),
            low: false,
        };
        // A run along -z (a rail) to the take-off at the origin, the jump across it along +x.
        let route = || {
            Route::new(vec![p(0.0, 0.0, 20.0, false), p(0.0, 0.0, 0.0, false), p(3.0, -1.7, 0.0, true), p(6.0, -1.7, 0.0, false)])
        };
        let step = |on: bool, feet: Vec3, way: [f32; 2], speed: f32| {
            let mut rng = Rng::new(3);
            let mut motor = Motor::new(Feel { sprint_gaps: on, sway: 0.0, ..Feel::default() }, &mut rng);
            motor.walking = true;
            let pose = Pose { feet, yaw: yaw_of(way), speed, ..Default::default() };
            motor.walk(&mut route(), &pose, 5.0, |_| true).0.keys
        };
        let run = step(true, Vec3::new(0.0, 0.0, 2.0), [0.0, -1.0], 5.0);
        assert_eq!(run & (key::FORWARD | key::SPRINT), key::FORWARD | key::SPRINT, "the sprint goes on into the turn: {run:#x}");
        assert_eq!(run & key::JUMP, 0, "not yet");
        // 0.4 m before the take-off, the view turned onto the way, 4.6 m/s: the press.
        let press = step(true, Vec3::new(0.0, 0.0, 0.4), [1.0, 0.0], 4.6);
        assert_ne!(press & key::JUMP, 0, "pressed: {press:#x}");
        // Too slow after the turn (a standing start): not pressed short of the take-off.
        assert_eq!(step(true, Vec3::new(0.0, 0.0, 0.4), [1.0, 0.0], 2.0) & key::JUMP, 0);
        // Off: no sprint held on the run.
        assert_eq!(step(false, Vec3::new(0.0, 0.0, 2.0), [0.0, -1.0], 5.0) & key::SPRINT, 0);
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
        let p = |x: f32, y: f32, jump: bool| Waypoint {
            at: Vec3::new(x, y, 0.0),
            node: 0,
            jump: jump.then_some(ledge_to(y)),
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

    /// A walk to an exact spot that asks for 0.25 m does not end at 0.45 m (where every other walk ends, within 0.6 m): the
    /// body walks on until it is inside.
    #[test]
    fn a_walk_to_an_exact_spot_goes_on_to_the_radius_the_play_asked_for() {
        let mut rng = Rng::new(7);
        let p = |x: f32| Waypoint {
            at: Vec3::new(x, 0.0, 0.0),
            node: 0,
            jump: None,
            low: false,
        };
        let route = || Route::new(vec![p(0.0), p(5.0)]);
        let at = |x: f32| facing_x(Vec3::new(x, 0.0, 0.0));
        let mut motor = Motor::new(Feel::default(), &mut rng);
        // An ordinary walk ends within Feel::arrive_m (0.6 m) of its end.
        assert_eq!(motor.walk(&mut route(), &at(4.55), 5.0, |_| true).1, Walked::Arrived);
        // An exact spot asked within 0.25 m does not end at 0.45 m, and ends at 0.2 m.
        motor.exact_end = Some(0.25);
        assert_eq!(motor.walk(&mut route(), &at(4.55), 5.0, |_| true).1, Walked::OnTheWay);
        assert_eq!(motor.walk(&mut route(), &at(4.8), 5.0, |_| true).1, Walked::Arrived);
        // Without a radius of its own it ends where every walk ends.
        motor.exact_end = Some(motor.feel.arrive_m);
        assert_eq!(motor.walk(&mut route(), &at(4.55), 5.0, |_| true).1, Walked::Arrived);
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

    /// Out of a blast (register #30) the sprint is taken on a 6 m route, facing the way, with no 10 m straight stretch and no
    /// facing time to wait for; the same route at a walk, or a run (`Gait::Run`), is not sprinted before its end.
    #[test]
    fn an_escape_sprints_on_a_short_route_and_a_walk_does_not() {
        let sprinted = |gait: Gait| {
            let mut route = route(&[(0.0, 0.0), (6.0, 0.0)]);
            let (decisions, _) = walk_as(&mut route, Vec3::new(0.0, 0.0, 0.0), yaw_of([1.0, 0.0]), 80, gait);
            decisions.iter().filter(|d| d.keys & key::SPRINT != 0).count()
        };
        assert!(sprinted(Gait::Escape) >= 5, "an escape sprints: {}", sprinted(Gait::Escape));
        assert_eq!(sprinted(Gait::Walk), 0);
        assert_eq!(sprinted(Gait::Run), 0, "a run needs the long straight too");
    }

    /// An escape is the legs running: the sprint into a walking jump is kept as for `Gait::Run`.
    #[test]
    fn an_escape_and_a_run_both_run() {
        assert!(Gait::Escape.runs() && Gait::Run.runs() && !Gait::Walk.runs());
    }

    /// Walks a body that does what the keys and turns say, with the
    /// movement model, until it arrives; returns the decisions.
    fn walk(route: &mut Route, start: Vec3, yaw: f32, ticks: usize) -> (Vec<Decision>, Pose) {
        walk_as(route, start, yaw, ticks, Gait::Walk)
    }

    /// [`walk`] with the legs asked to `gait`.
    fn walk_as(route: &mut Route, start: Vec3, yaw: f32, ticks: usize, gait: Gait) -> (Vec<Decision>, Pose) {
        let kin = Kinematics::default();
        let mut rng = Rng::new(7);
        let mut motor = Motor::new(Feel::default(), &mut rng);
        let mut pose = Pose {
            feet: start,
            yaw,
            pitch: 0.0,
            speed: 0.0,
            gait,
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
            pose.velocity = velocity;
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
        let every = (feel.hop_every_s.get() / TICK_S) as usize;
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
        let point = |x: f32, jump: bool, low| Waypoint {
            at: Vec3::new(x, 0.0, 0.0),
            node: 0,
            jump: jump.then_some(Leap::Ledge),
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
