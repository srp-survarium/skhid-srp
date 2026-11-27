//! Bear traps: the habit that puts one down (`docs/notes/bear_traps.md`).
//!
//! A bot that carries traps ([`Habit::carry`]) puts one down as a habit, as it
//! heals or throws a grenade: it is no play, takes over nothing, and the bot
//! goes on with what it was doing. At a think where its own plan leaves it free
//! (no urgent play, no enemy near, nothing hit it lately, a good spot a metre
//! off) it picks the floor point, looks down at it with the legs as the play
//! has them, presses the trap's quick-use key one tick and lets go the next
//! (the client places on the release, PROVEN), and the plan goes on. No goal,
//! route or assignment is changed.
//!
//! - **Where the trap lands** is set by the view alone: the head ray is 2.0 m
//!   (PROVEN) and a bot looks down at most 1.0 rad (`body::MAX_PITCH`). The
//!   eye is not 1.6 m: standing it is 1.46 + 0.16 sin(pitch), crouched and
//!   still 1.04 + 0.22 sin(pitch) (`view::eye_height`, PROVEN), so at the
//!   steepest look the ray meets the floor 0.85 m out standing and 0.55 m
//!   crouched, on the trigger's reach (0.6 m) of the placer's own feet.
//!   With `TrapTuning::press_landing_m` the habit looks down only so steeply
//!   that the ray lands that far out ([`press_look`]), from the eye the bot
//!   has at the press, and presses only when the predicted landing is that
//!   far from the feet and, off its own route ahead, by the lateral rule
//!   ([`Aim`]). The eight yaws round the bot are the candidates ([`choose`]).
//! - **The score** is enemy presence at the spot (the team's lanes of the
//!   enemy humans, `know::HumanLanes`) less the friend flow (a mate there now
//!   or in 2 and 4 s weighs; a mate within `friend_m` forbids it). Round it:
//!   level floor, open floor all round (never a doorway), not in a spawn
//!   area, off the bot's own route ahead by `lateral_m`, no own trap within
//!   `near_own_m`, and the team's cap of standing traps.
//! - **The placer's own steps:** the spot lies `lateral_m` off the route ahead
//!   (the placer was the victim in 4.2 % of catches, 3 s after placing:
//!   PROVEN), so it walks on without meeting it. A replan across it is the
//!   team field's to prevent (a later change); here it is counted.
//! - **The kit decides who carries:** a bot is told its traps at its spawn
//!   (`Bot::arm`: [`Habit::of_kit`] of the traps in its kit's pocket,
//!   `survarium::v026g::loadouts::bot_trap`); the old pockets hold none, so
//!   with `SRP_BOT_KIT` off the habit is idle as ever.

use std::sync::Arc;

use mapdata::{NodeId, Vec3, WalkGrid};

use super::kin;
use super::know::{HumanLanes, Track, LANE_CELL_M};
use super::plays::PlayKind;
use super::route_end::RouteEnd;
use super::route_filter::RouteFilter;
use super::think::Due;
use super::trap_know::TrapField;
use super::tuning::{LaneSource, PressClear, Stayer, TrapRelease, TrapTuning};
use super::view::PlaceView;
use super::{Bot, TeamState};

/// How far the view looks down, radians, positive (the habit's own sign: the bots' view pitch is negative looking down,
/// [`Down::of_view`] and [`Down::view`] are the only conversions).
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Down(pub f32);

impl Down {
    /// From the view's pitch (up positive, a look down is negative).
    pub fn of_view(pitch: f32) -> Self {
        Self(-pitch)
    }

    /// The view's pitch.
    pub fn view(self) -> f32 {
        -self.0
    }
}

/// The steepest look down (`body::MAX_PITCH`).
pub const STEEPEST: Down = Down(1.0);
/// The steepest look down, radians (`body::MAX_PITCH`).
pub const LOOK_DOWN: f32 = STEEPEST.0;
/// The press is made this near the pitch the look aims for, radians: at the
/// steepest look, 1.0 - 0.04 = 0.96, where the head ray met the floor within
/// 2.0 m from the (guessed) standing eye. Both sides: a view still
/// slewing past the aim is let go of a tick later, steeper, and lands nearer.
pub const PRESS_SLACK: f32 = 0.04;
/// The least of the head ray's 2.0 m left over, metres: a pose that changes
/// between the plan and the press (the crouched eye rises 0.1 m when the body
/// moves) must still meet the floor.
const RAY_MARGIN_M: f32 = 0.1;
/// The landing the look aims for lies this far past the least the tuning asks
/// of it, metres, so that the sway of a walking body does not undo it.
const LANDING_MARGIN_M: f32 = 0.1;
/// From the press tick to the tick that lets go of the key, seconds: the key is down one input and up the next
/// (50 ms ticks, the body two inputs on from the bot's reckoning).
const RELEASE_LEAD_S: f32 = 0.1;
/// The head ray that puts a trap down is this long, metres (PROVEN: 2.0 m
/// from the head along the view, `try_to_place_trap` 0x305EA0).
pub const PLACE_RAY_M: f32 = 2.0;
/// The metres round a trap's centre kept clear of the bot's own steps and of
/// a mate's: the sensor reaches 0.6 m from a foot point (PROVEN) and a body
/// has a margin.
pub const AVOID_M: f32 = 0.9;
/// How far from a trap's centre a foot point catches it, metres: 565 of 566
/// catches in the recordings had the foot within 0.6 m (PROVEN; the sensor
/// is 0.25 m and a foot reaches the rest).
pub const TRIGGER_M: f32 = 0.6;
/// The metres round a standing own trap the team's bots keep off: the
/// trigger reach and 0.15 m of the route's tolerance. Less than [`AVOID_M`]
/// (which the placer keeps from its own steps): a field of 0.9 m and half a
/// cell shut a floor 1.025 m off, which closed every corridor under 2 m.
pub const FIELD_M: f32 = TRIGGER_M + 0.15;
/// The yaws tried round the bot: eight, 45 degrees apart.
const YAWS: usize = 8;
/// A lane cell counts toward a spot this near, metres (the cell is 6 m).
const PRESENCE_M: f32 = 6.0;
/// A bot slower than this is standing, metres a second.
const STANDING_MS: f32 = 0.5;

/// What a bot carries: how many, and the key that puts one down (its quick
/// use bit: `key::QUICK[n]`, the trap's rank among the filled pockets,
/// boosters not counted).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TrapKit {
    pub count: u8,
    pub key: u32,
}

/// What the habit is doing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum State {
    Idle,
    /// Looking down along `yaw` for the floor point `spot`, since `since`; `last` is the last thing that kept the
    /// press from being made.
    Looking {
        yaw: f32,
        spot: Vec3,
        since: u32,
        last: Option<Why>,
    },
    /// The key is down this tick; let go the next, with the pose of the press (`crouched`) held.
    Down {
        yaw: f32,
        spot: Vec3,
        crouched: bool,
    },
    /// Done or given up; not again until `until`.
    Cooling {
        until: u32,
    },
}

/// Why a look has not pressed (yet), the last of them when it gives up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Why {
    /// The view is not on the yaw yet.
    Yaw,
    /// The view is not on the look's steepness (still turning down, or past it).
    Pitch,
    /// A trap let go of now would land nearer the feet than `press_landing_m`.
    TooNear,
    /// ... on the bot's own route ahead (the lateral rule).
    OnRoute,
    /// ... nowhere: the head ray ends in the air.
    NoFloor,
    /// No look of the pose reaches the floor (the spot is too far below, or the ray too short).
    NoLook,
    /// [A324] The head ray meets ground too steep for the client to place on (`TrapRelease::Client`).
    Slope,
    /// [A324] ... or a known trap's sensor (`TrapRelease::Client`).
    OnTrap,
}

impl Why {
    pub const ALL: [Self; 8] =
        [Self::Yaw, Self::Pitch, Self::TooNear, Self::OnRoute, Self::NoFloor, Self::NoLook, Self::Slope, Self::OnTrap];

    pub const fn index(self) -> usize {
        match self {
            Self::Yaw => 0,
            Self::Pitch => 1,
            Self::TooNear => 2,
            Self::OnRoute => 3,
            Self::NoFloor => 4,
            Self::NoLook => 5,
            Self::Slope => 6,
            Self::OnTrap => 7,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Yaw => "yaw",
            Self::Pitch => "pitch",
            Self::TooNear => "too near",
            Self::OnRoute => "on route",
            Self::NoFloor => "no floor",
            Self::NoLook => "no look",
            Self::Slope => "slope",
            Self::OnTrap => "on a trap",
        }
    }
}

/// Whether a trap let go of now would land where the tuning wants it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Press {
    Ready,
    Wait(Why),
}

/// How the habit looks and presses this tick, from the pose the bot has now.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Aim {
    /// `press_landing_m` off: the steepest look, a press as soon as the view is on it (the habit as it was).
    Steepest,
    /// The look the pose reaches: how steep, and whether a press now lands right.
    Look { pitch: Down, press: Press },
    /// No look of the pose reaches the floor: nothing to press with.
    NoLook,
}

/// One tick of the habit's state machine.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tick {
    pub now: u32,
    /// The view: yaw and pitch, radians (a look down is a negative pitch, as
    /// everywhere in the bots).
    pub yaw: f32,
    pub pitch: f32,
    /// Nothing calls the bot away: no enemy seen, the reflex aims at none.
    pub calm: bool,
    /// The pose the body will have in this tick's input: crouched or not (held from the press to the release).
    pub crouched: bool,
    /// How steeply to look, and whether a trap let go of now would land where it is wanted.
    pub aim: Aim,
}

/// How a look ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ended {
    /// The trap was let go of.
    Placed,
    /// An enemy or a shot called the bot away.
    CalledAway,
    /// The look time ran out; `Some` the last thing that kept the press back.
    GaveUp(Option<Why>),
}

/// What a tick asks of the body.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Out {
    /// Look down along this yaw.
    pub look_yaw: Option<f32>,
    /// The key is down this tick.
    pub key_down: bool,
    /// The trap was let go of: put down at this point.
    pub placed: Option<Vec3>,
    /// The pose to hold through the press and the release (the crouch key as it was at the press).
    pub hold_crouch: Option<bool>,
    /// The look ended this tick.
    pub ended: Option<Ended>,
}

/// The shortest turn from `a` to `b`, radians.
fn turn(a: f32, b: f32) -> f32 {
    let d = (b - a).rem_euclid(std::f32::consts::TAU);
    if d > std::f32::consts::PI {
        d - std::f32::consts::TAU
    } else {
        d
    }
}

/// What keeps the press back at this tick, if anything: the view not on the yaw or the look's steepness, or a trap let
/// go of now landing wrong.
fn kept_back(yaw: f32, tick: &Tick, t: &TrapTuning) -> Option<Why> {
    if turn(tick.yaw, yaw).abs() > t.tolerance_rad {
        return Some(Why::Yaw);
    }
    let down = Down::of_view(tick.pitch);
    match tick.aim {
        Aim::Steepest => ((down.0 - STEEPEST.0).abs() > PRESS_SLACK).then_some(Why::Pitch),
        Aim::NoLook => Some(Why::NoLook),
        Aim::Look { pitch, press } => {
            if (down.0 - pitch.0).abs() > PRESS_SLACK {
                Some(Why::Pitch)
            } else if let Press::Wait(why) = press {
                Some(why)
            } else {
                None
            }
        }
    }
}

/// The transition: the state and a tick give the new state and what the body
/// does. Total over the states.
pub fn step(state: State, tick: Tick, t: &TrapTuning) -> (State, Out) {
    let cooldown = |now: u32, scale: f32| State::Cooling {
        until: now + (t.cooldown_s * scale * 1000.0) as u32,
    };
    match state {
        State::Idle => (State::Idle, Out::default()),
        State::Looking { yaw, spot, since, last } => {
            if !tick.calm {
                // Called away: no harm done, soon again.
                return (
                    cooldown(tick.now, 0.25),
                    Out {
                        ended: Some(Ended::CalledAway),
                        ..Out::default()
                    },
                );
            }
            if tick.now.saturating_sub(since) > t.look_ms {
                return (
                    cooldown(tick.now, 0.5),
                    Out {
                        ended: Some(Ended::GaveUp(last)),
                        ..Out::default()
                    },
                );
            }
            match kept_back(yaw, &tick, t) {
                None => (
                    State::Down {
                        yaw,
                        spot,
                        crouched: tick.crouched,
                    },
                    Out {
                        look_yaw: Some(yaw),
                        key_down: true,
                        hold_crouch: t.press_landing_m.is_some().then_some(tick.crouched),
                        ..Out::default()
                    },
                ),
                why => (
                    State::Looking {
                        yaw,
                        spot,
                        since,
                        last: why,
                    },
                    Out {
                        look_yaw: Some(yaw),
                        ..Out::default()
                    },
                ),
            }
        }
        State::Down { yaw, spot, crouched } => (
            cooldown(tick.now, 1.0),
            Out {
                look_yaw: Some(yaw),
                key_down: false,
                placed: Some(spot),
                hold_crouch: t.press_landing_m.is_some().then_some(crouched),
                ended: Some(Ended::Placed),
            },
        ),
        State::Cooling { until } => {
            if tick.now >= until {
                (State::Idle, Out::default())
            } else {
                (state, Out::default())
            }
        }
    }
}

/// Why the habit does not place now (for the trace and the tests).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    Off,
    NoTrap,
    LifeSpent,
    Busy,
    Play(PlayKind),
    Carrying,
    Health,
    Enemy,
    Hit,
    Reloading,
    Jump,
    TeamCap,
    /// [fix-team 5 T8 `skill.tactics.traps`] Not a life its skill gives traps of its own.
    Skill,
}

/// What the gate reads: all the bot's own state at a think.
#[derive(Clone, Copy, Debug)]
pub struct Facts {
    pub now: u32,
    pub play: Option<PlayKind>,
    pub carrying: bool,
    pub health: f32,
    /// An enemy seen now, or sensed within `enemy_quiet_s`, or known within
    /// `enemy_near_m` (the caller reads its memory).
    pub enemy: bool,
    pub last_hit: Option<u32>,
    pub reloading: bool,
    /// A jump or crossing of the route within `jump_ahead_m`.
    pub jump_ahead: bool,
    /// Traps of the team counted standing.
    pub team_standing: usize,
}

/// Plays that call the bot to a task or a fight: the habit waits for
/// another (`design`: tasks first).
pub fn urgent(play: PlayKind) -> bool {
    matches!(
        play,
        PlayKind::Collect
            | PlayKind::Intercept
            | PlayKind::Escort
            | PlayKind::Raid
            | PlayKind::Station
            | PlayKind::Retreat
            | PlayKind::Heal
            | PlayKind::Hunt
            | PlayKind::Trade
            | PlayKind::LateDenial
    )
}

/// Whether a bot at a think may place one: `Ok`, or the first reason not.
pub fn gate(f: &Facts, t: &TrapTuning) -> Result<(), Refusal> {
    if let Some(play) = f.play.filter(|play| urgent(*play)) {
        return Err(Refusal::Play(play));
    }
    if f.carrying {
        return Err(Refusal::Carrying);
    }
    if f.health < t.min_health {
        return Err(Refusal::Health);
    }
    if f.enemy {
        return Err(Refusal::Enemy);
    }
    if f.last_hit
        .is_some_and(|hit| f.now < hit + (t.hit_quiet_s * 1000.0) as u32)
    {
        return Err(Refusal::Hit);
    }
    if f.reloading {
        return Err(Refusal::Reloading);
    }
    if f.jump_ahead {
        return Err(Refusal::Jump);
    }
    if f.team_standing >= usize::from(t.cap_team) {
        return Err(Refusal::TeamCap);
    }
    Ok(())
}

/// The habit of one bot.
#[derive(Clone, Debug, PartialEq)]
pub struct Habit {
    kit: Option<TrapKit>,
    /// Traps left in the pocket this life.
    left: u8,
    /// [Gap 11] When it last let go of one this life (the tick's time); `None`: not yet. A world made before
    /// `stock::PRESS_LAG_MS` after it is no word on that trap ([`Habit::read`]).
    counted: Option<u32>,
    placed_life: u8,
    state: State,
    /// Traps let go of in all its lives, and lives that ended while it was placing one (looking or pressing).
    placed_total: u32,
    died_placing: u32,
    /// [Gap 11] What the worlds' trap stacks did ([`PlaceCounts::world_read`]).
    world_read: u32,
    world_changed: u32,
    outcomes: LookOutcomes,
}

/// How the looks ended, over all the lives of a habit (or summed over habits): one outcome for every look, the last
/// thing that kept the press back for the ones that gave up.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LookOutcomes {
    pub placed: u32,
    pub called_away: u32,
    /// Gave up, by the last [`Why`] ([`Why::index`]).
    pub gave_up: [u32; Why::ALL.len()],
    /// Gave up with nothing to name (the look ran out before a tick of it was judged).
    pub gave_up_unjudged: u32,
}

impl LookOutcomes {
    pub fn record(&mut self, ended: Ended) {
        match ended {
            Ended::Placed => self.placed += 1,
            Ended::CalledAway => self.called_away += 1,
            Ended::GaveUp(Some(why)) => self.gave_up[why.index()] += 1,
            Ended::GaveUp(None) => self.gave_up_unjudged += 1,
        }
    }

    pub fn add(&mut self, other: &Self) {
        self.placed += other.placed;
        self.called_away += other.called_away;
        self.gave_up_unjudged += other.gave_up_unjudged;
        for (a, b) in self.gave_up.iter_mut().zip(&other.gave_up) {
            *a += b;
        }
    }

    pub fn looks(&self) -> u32 {
        self.placed + self.called_away + self.gave_up_unjudged + self.gave_up.iter().sum::<u32>()
    }

    /// One text line: `looks N: placed N, called away N, gave up N (yaw N, pitch N, ...)`.
    pub fn line(&self) -> String {
        let why = Why::ALL
            .iter()
            .map(|why| format!("{} {}", why.name(), self.gave_up[why.index()]))
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "looks {}: placed {}, called away {}, gave up {} ({why}, unjudged {})",
            self.looks(),
            self.placed,
            self.called_away,
            self.gave_up.iter().sum::<u32>() + self.gave_up_unjudged,
            self.gave_up_unjudged,
        )
    }
}

/// What a bot's habit did in all its lives (the bench's activity and harm counts).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlaceCounts {
    pub placed: u32,
    pub died_placing: u32,
    /// Bots that carry traps (their habit was told them).
    pub armed: u32,
    /// [Gap 11] Worlds whose trap stack the pocket took ([`Habit::read`]), and of them those that changed the count.
    pub world_read: u32,
    pub world_changed: u32,
}

impl PlaceCounts {
    pub fn add(&mut self, other: &PlaceCounts) {
        self.placed += other.placed;
        self.died_placing += other.died_placing;
        self.armed += other.armed;
        self.world_read += other.world_read;
        self.world_changed += other.world_changed;
    }
}

impl Default for Habit {
    fn default() -> Self {
        Self::none()
    }
}

impl Habit {
    /// A bot with no traps.
    pub fn none() -> Self {
        Self {
            kit: None,
            left: 0,
            counted: None,
            placed_life: 0,
            state: State::Idle,
            placed_total: 0,
            died_placing: 0,
            world_read: 0,
            world_changed: 0,
            outcomes: LookOutcomes::default(),
        }
    }

    /// A bot told it carries `kit`.
    pub fn carry(kit: TrapKit) -> Self {
        Self {
            kit: Some(kit),
            left: kit.count,
            counted: None,
            placed_life: 0,
            state: State::Idle,
            placed_total: 0,
            died_placing: 0,
            world_read: 0,
            world_changed: 0,
            outcomes: LookOutcomes::default(),
        }
    }

    /// A bot's habit from the traps its kit carries: `(count, quick-use bit)` of
    /// [`survarium::v026g::loadouts::bot_trap`]; none carried is [`Habit::none`].
    pub fn of_kit(trap: Option<(u32, u8)>) -> Self {
        trap.map_or_else(Self::none, |(count, bit)| {
            Self::carry(TrapKit {
                count: count.min(u32::from(u8::MAX)) as u8,
                key: 1 << bit,
            })
        })
    }

    /// How its looks ended so far.
    pub fn outcomes(&self) -> LookOutcomes {
        self.outcomes
    }

    /// A look ended ([`Out::ended`]).
    pub fn record(&mut self, ended: Ended) {
        self.outcomes.record(ended);
    }

    /// What it did so far.
    pub fn counts(&self) -> PlaceCounts {
        PlaceCounts {
            placed: self.placed_total,
            died_placing: self.died_placing,
            armed: u32::from(self.kit.is_some()),
            world_read: self.world_read,
            world_changed: self.world_changed,
        }
    }

    /// A new life: the pocket is full again, nothing placed.
    pub fn new_life(&mut self) {
        // A life that ended in the middle of placing one (looking down, or the key down): the harm count. Not one that ended in
        // the cooldown after a look (`cooldown_s`: 20 s after a press, 10 s after a give-up, 5 s after a call-away): the #2081
        // bench counted those as placing, 10 of 10 in its first match (the looks' own records: no death in a look or a press).
        if matches!(self.state, State::Looking { .. } | State::Down { .. }) {
            self.died_placing += 1;
        }
        self.left = self.kit.map_or(0, |kit| kit.count);
        self.counted = None;
        self.placed_life = 0;
        self.state = State::Idle;
    }

    /// Traps left in the pocket now.
    pub fn left(&self) -> u8 {
        self.left
    }

    /// [Gap 11] A world made at `time` shows `in_hand` traps in the pocket: the pocket holds them, unless one it let go
    /// of less than `stock::PRESS_LAG_MS` before (the world may not show it yet: the count stands). The client takes a
    /// trap only at a placement that succeeds (`booby_trap_set_core::action` 0x306720: the stack - 1 only when the
    /// placement 0x305EA0 returns true), so a release on a slope or on a trap's sensor is so undone. Returns whether
    /// the count changed; `None`: held, or no trap carried.
    pub fn read(&mut self, time: u32, in_hand: u16) -> Option<bool> {
        self.kit?;
        if self.counted.is_some_and(|at| time < at.saturating_add(super::stock::PRESS_LAG_MS)) {
            return None;
        }
        let left = in_hand.min(u16::from(u8::MAX)) as u8;
        let changed = left != self.left;
        self.left = left;
        self.world_read += 1;
        self.world_changed += u32::from(changed);
        Some(changed)
    }

    pub fn state(&self) -> State {
        self.state
    }

    pub fn kit(&self) -> Option<TrapKit> {
        self.kit
    }

    pub fn placed_this_life(&self) -> u8 {
        self.placed_life
    }

    /// Whether it may plan one at all now (the gate's cheap part: no kit,
    /// none left, the life's number placed, or busy).
    pub fn may_plan(&self, t: &TrapTuning) -> Result<(), Refusal> {
        if !t.enabled {
            return Err(Refusal::Off);
        }
        if self.kit.is_none() || self.left == 0 {
            return Err(Refusal::NoTrap);
        }
        if self.placed_life >= t.per_life {
            return Err(Refusal::LifeSpent);
        }
        if self.state != State::Idle {
            return Err(Refusal::Busy);
        }
        Ok(())
    }

    /// A spot chosen: look down along `yaw` for it.
    pub fn plan(&mut self, yaw: f32, spot: Vec3, now: u32) {
        self.state = State::Looking {
            yaw,
            spot,
            since: now,
            last: None,
        };
    }

    /// One tick: the keys and the look it asks of the body. A trap let go of
    /// counts at once.
    pub fn tick(&mut self, tick: Tick, t: &TrapTuning) -> Out {
        let (state, out) = step(self.state, tick, t);
        self.state = state;
        if out.placed.is_some() {
            self.left = self.left.saturating_sub(1);
            self.counted = Some(tick.now);
            self.placed_life += 1;
            self.placed_total += 1;
        }
        out
    }
}

/// What the habit needs of the world to pick a spot.
pub struct Search<'a> {
    pub t: &'a TrapTuning,
    /// How far ahead of the feet the candidates lie, metres: where the head ray meets a level floor at the look
    /// the bot will press with ([`press_look`]).
    pub reach_m: f32,
    /// With the landing rule: the pose the look is made in (crouched or not) and the distance it aims at, so that each
    /// candidate's own floor height gets its own look (a ramp or a stair moves the landing).
    pub press: Option<(bool, f32)>,
    /// The view's yaw now: the spot that turns it least wins a tie.
    pub view_yaw: f32,
    pub grid: &'a WalkGrid,
    pub now: u32,
    /// Where the bot's feet will be when it presses, and its yaw of walk.
    pub feet: Vec3,
    pub staying: bool,
    /// Points of the route ahead, one a metre, `route_ahead_m` of them.
    pub ahead: &'a [Vec3],
    /// The bot's mates: place, feet and velocity (ground, m/s).
    pub mates: &'a [(Vec3, [f32; 2])],
    pub lanes: &'a HumanLanes,
    /// Where the team's standing traps are.
    pub own: &'a [Vec3],
    /// Whether a point is in a spawn area.
    pub in_spawn: &'a dyn Fn(Vec3) -> bool,
    /// [A145] The spots of killed holders this bot's route passes.
    pub wish: Option<Wish<'a>>,
    /// [A309] With `TrapTuning::forward`: its side's home and the enemy's (the spawns' centres, [`homes_of`]) and the least share
    /// of the way between them a spot must lie past ([`forward_share`]).
    pub homes: Option<(Vec3, Vec3, f32)>,
}

/// [A309] The centres of the spawns of `side` and of the other sides, on the ground: what every player knows of a map. None when
/// either has no spawn.
pub fn homes_of(map: &mapdata::project::MapData, side: Option<i32>) -> Option<(Vec3, Vec3)> {
    let centre = |own: bool| {
        let points: Vec<Vec3> = map.spawns.iter().filter(|s| (Some(s.team) == side) == own).map(|s| s.position).collect();
        let n = points.len() as f32;
        (n > 0.0).then(|| Vec3::new(points.iter().map(|p| p.x).sum::<f32>() / n, 0.0, points.iter().map(|p| p.z).sum::<f32>() / n))
    };
    Some((centre(true)?, centre(false)?))
}

/// [bot traps] Why a held place gets no way-in point ([`way_in`]): the camper job's note says it, the bench counts it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NoWayIn {
    /// The place (or his home) on no floor (`path::goal_floor`).
    NoFloor,
    /// No way from his spawn even with every zone off ([`super::tactics::WayTo::Never`]): a shut pocket, a floor no planned link
    /// enters, one past a ledge only dropped from. No search asked.
    Never,
    /// A way only across zones on all match ([`super::tactics::WayTo::CrossingOnly`]): his place on or behind one. No search asked.
    BehindFixedZone,
    /// The regions give a way, the search with the zones that switch off found none.
    NoRoute,
    /// His way of `way_m` metres is shorter than `near_m`.
    TooShort { way_m: f32 },
    /// Every point on his way of `way_m` metres was in a spawn area, or in his sight (`in_spawn` of them in a spawn area).
    AllRefused { way_m: f32, points: usize, in_spawn: usize },
}

impl NoWayIn {
    /// The camper job's note's words.
    fn words(&self) -> String {
        match *self {
            Self::NoFloor => "held place on no floor".to_string(),
            Self::Never => "no planned way to his place from his spawn (a pocket, or a floor no link enters)".to_string(),
            Self::BehindFixedZone => "his place is on or behind a zone on all match".to_string(),
            Self::NoRoute => "no way to his place with the zones that switch off".to_string(),
            Self::TooShort { way_m } => format!("way of {way_m:.0} m, shorter than the nearest point"),
            Self::AllRefused { way_m, points, in_spawn } if in_spawn == points => {
                format!("way of {way_m:.0} m, every point in a spawn area")
            }
            Self::AllRefused { way_m, in_spawn: 0, .. } => format!("way of {way_m:.0} m, every point in his sight"),
            Self::AllRefused { way_m, points, in_spawn } => {
                format!("way of {way_m:.0} m, every point in his sight ({} of {points}) or a spawn area ({in_spawn})", points - in_spawn)
            }
        }
    }

    /// [`camper`]'s count of it (test builds).
    #[cfg(test)]
    fn count_kind(&self) -> usize {
        match *self {
            Self::NoFloor => camper::NO_FLOOR,
            Self::Never | Self::BehindFixedZone | Self::NoRoute => camper::NO_ROUTE,
            Self::TooShort { .. } => camper::TOO_SHORT,
            Self::AllRefused { points, in_spawn, .. } if in_spawn == points => camper::IN_SPAWN,
            Self::AllRefused { .. } => camper::ALL_SEEN,
        }
    }
}

/// [A318, fx-gear-2; A324; bot traps] The way back to a held `spot` from `home` (a floor of his side's spawns), as a body walks it
/// at a time not known yet ([`super::tactics::Tactics::route_all_match`]: the zones that switch off, those on all match shut).
/// Asked only where a way may be ([`super::tactics::Tactics::reach`] from home, side none as anyone walks it): a place no way
/// leads to is told at once, with no search.
pub fn approach_way(tactics: &super::tactics::Tactics, home: Vec3, spot: Vec3) -> Result<super::path::Route, NoWayIn> {
    use super::tactics::WayTo;
    let grid = &tactics.world.grid;
    let (Some(from), Some(to)) = (super::path::goal_floor(grid, home), super::path::goal_floor(grid, spot)) else {
        return Err(NoWayIn::NoFloor);
    };
    match tactics.reach(from, None, false).way(to) {
        WayTo::Never => return Err(NoWayIn::Never),
        WayTo::CrossingOnly => return Err(NoWayIn::BehindFixedZone),
        WayTo::Plain => {}
    }
    tactics.route_all_match(from, to, None).ok_or(NoWayIn::NoRoute)
}

/// On `route`, one point every `step_m` from `near_m` to `far_m` before its end, nearest first.
fn points_before_end(route: &super::path::Route, j: &super::tuning::CamperJob) -> Vec<Vec3> {
    let length = route.length();
    let mut points = Vec::new();
    let mut back = j.near_m;
    while back <= j.far_m && back < length {
        points.push(route.point_at(length - back));
        back += j.step_m;
    }
    points
}

/// [A318, fx-gear-2; A324] The way back to a held `spot` from `home` ([`approach_way`]), and on it one point every `step_m` from
/// `near_m` to `far_m` before the spot, nearest first. None when either end has no floor or no way joins them.
pub fn approach_points(tactics: &super::tactics::Tactics, home: Vec3, spot: Vec3, j: &super::tuning::CamperJob) -> Vec<Vec3> {
    approach_way(tactics, home, spot).map_or(Vec::new(), |route| points_before_end(&route, j))
}

/// [A324] The way-in point of a place `held`: the first point of his way there from `home`, `near_m`..`far_m` before it, out of
/// his sight both ways for a placer crouched there ([`out_of_his_sight`]: the `TrapSpot` play stays on it crouched), and in no
/// side's spawn area (his side's has no way for the walk, the team's own is no place for a trap). Else why there is none.
fn way_in(tactics: &super::tactics::Tactics, home: Vec3, held: Vec3, j: &super::tuning::CamperJob) -> Result<Vec3, NoWayIn> {
    let route = approach_way(tactics, home, held)?;
    let way_m = route.length();
    let points = points_before_end(&route, j);
    if points.is_empty() {
        return Err(NoWayIn::TooShort { way_m });
    }
    let mut in_spawn = 0;
    for &p in &points {
        if tactics.in_spawn_area(p, None) {
            in_spawn += 1;
        } else if out_of_his_sight(&tactics.world.sight, held, p) {
            return Ok(p);
        }
    }
    Err(NoWayIn::AllRefused { way_m, points: points.len(), in_spawn })
}

/// The eye over the feet of a body moving crouched (`view::eye_height`: any speed over the still class's).
fn crouched_moving_eye(feet: Vec3) -> Vec3 {
    feet + Vec3::new(0.0, super::view::eye_height(true, f32::MAX, 0.0), 0.0)
}

/// [A324] Whether a bot crouched at `feet` and a man standing on `held` cannot see each other, as a bot's sight looks (any of
/// the parts of `aimpoint::Part::SIGHT_ORDER`, `sense.rs`), both ways: no part of the crouched bot shows from his eye
/// (`lane_ahead::camper_eye`, a held spot's), and no part of him shows from the bot's crouched moving eye (`view::eye_height`:
/// 1.14 m, the higher of the crouched eyes; the still one 1.04 m). On the bullet mesh, without smoke.
pub fn out_of_his_sight(sight: &mapdata::RayTree, held: Vec3, feet: Vec3) -> bool {
    let his_eye = super::lane_ahead::camper_eye(held);
    !super::aimpoint::body_shows(sight, his_eye, feet, true) && !super::aimpoint::body_shows(sight, crouched_moving_eye(feet), held, false)
}

/// [A324] Whether crouching hides a bot at `feet` from a man on `held`: standing, some part of it shows from his eye; crouched,
/// they cannot see each other ([`out_of_his_sight`]).
pub fn hidden_by_crouching(sight: &mapdata::RayTree, held: Vec3, feet: Vec3) -> bool {
    super::aimpoint::body_shows(sight, super::lane_ahead::camper_eye(held), feet, false) && out_of_his_sight(sight, held, feet)
}

/// [#2081] A sent bot runs the job's play from its first thinks: two at `ThinkTuning::think_ms`'s upper 400 ms and one commander
/// tick (`ThinkTuning::commander_ms`, 500 ms), ms.
const LEFT_PLAY_GRACE_MS: u32 = 1_300;

/// [A324] The team's camper trap jobs at a commander tick (`TrapTuning::camper_job`): a trap on the way in of a place an enemy
/// holds: of 91 human traps at an enemy's killing hold in the live traps record, the 8 on his way in caught him 3 times, the 83
/// others never (small n; `docs/notes/bear_traps.md`, "Human traps at a holder's place"). The places:
/// - `sources`: the held spots its K2 jobs run on and the men it holds off from at a loose battery (`take.clear_first`), wanted
///   while that lasts;
///
/// The point is the first of his way from his side's nearest spawn, `near_m`..`far_m` before the place, that his eye there does
/// not see ([`way_in`]); the nearest free bot with a trap within `max_walk_m` is sent (`plays::trap_spot`); a bot not done in
/// `give_up_s` is let go. Every decision is a note.
#[allow(clippy::too_many_arguments)]
pub(super) fn camper_tick(
    bots: &[Option<Bot>],
    knowledge: &mut super::know::TeamKnowledge,
    view: &super::view::MatchView,
    tactics: &super::tactics::Tactics,
    t: &TrapTuning,
    team: u8,
    sources: &[super::threat::HeldLine],
    now: u32,
    notes: &mut Vec<super::Note>,
) {
    use super::know::{CamperHold, CamperTrap, CamperTrapKind, CamperTrapState, SeenRefusals};
    let Some(j) = t.camper_job else {
        knowledge.camper_traps.clear();
        knowledge.camper_no_way.clear();
        return;
    };
    let side = bots.iter().flatten().find(|b| b.team == team).and_then(|b| b.side);
    let mut said = Vec::new();
    let bot_of = |p: super::view::Place| bots.iter().flatten().find(|b| b.place == p);
    let alive = |p: super::view::Place| view.place(p).is_some_and(|b| b.alive);
    let xyz = |v: Vec3| format!("({:.1}, {:.1}, {:.1})", v.x, v.y, v.z);
    // The sent bots: who is done, who is let go.
    let give_up_ms = (j.give_up_s * 1000.0) as u32;
    knowledge.camper_traps.retain_mut(|c| {
        let CamperTrapState::Sent { placer, since, placed_before } = c.state else {
            return true;
        };
        if bot_of(placer).is_some_and(|b| b.features.traps.habit.placed_this_life() > placed_before) {
            #[cfg(test)]
            camper::count(team, camper::PLACED);
            #[cfg(test)]
            eprintln!("  camper life: team {team} spot {} {:?} placed t {now} placer {placer} walked {:.1}", c.spot, c.kind, now.saturating_sub(since) as f32 / 1000.0);
            said.push(format!(
                "team {team}: bot {placer} put its trap on {}'s way in at {} ({:?} of player {}); at the point its gate refused {} thinks for an enemy seen ({}); the place is trapped while its trap stands",
                c.spot,
                xyz(c.at),
                c.kind,
                c.shooter,
                c.seen.total(),
                c.seen.said()
            ));
            c.state = CamperTrapState::Trapped;
            return true;
        }
        let body = view.place(placer).filter(|b| b.alive && b.carrying.is_none());
        #[cfg(test)]
        if body.is_some_and(|b| b.feet.distance_xz(c.at) <= j.within_m + t.reach_m + 1.0) && camper::arrived(team, c.spot, since) {
            eprintln!("  camper life: team {team} spot {} {:?} arrived t {now} placer {placer} walked {:.1}", c.spot, c.kind, now.saturating_sub(since) as f32 / 1000.0);
        }
        // [#2081] A bot that left the job's play is let go: the job stayed sent to it while it ran another (12 of 26 sends of the
        // bench's m7: Retreat, Flank, Ambush, Trade, PairUp, none). Past its first thinks ([`LEFT_PLAY_GRACE_MS`]).
        let left_play = now.saturating_sub(since) > LEFT_PLAY_GRACE_MS
            && bot_of(placer).is_some_and(|b| b.assignment.as_ref().is_none_or(|a| a.kind != PlayKind::TrapSpot));
        let why = if body.is_none() {
            Some(if alive(placer) { "carrying" } else { "dead" })
        } else if now.saturating_sub(since) > give_up_ms {
            Some("its time is out")
        } else if left_play {
            Some("it left the job's play")
        } else {
            None
        };
        if let Some(why) = why {
            #[cfg(test)]
            {
                eprintln!(
                    "  camper life: team {team} spot {} {:?} let_go t {now} placer {placer} walked {:.1} why {} left {:.1} {}",
                    c.spot,
                    c.kind,
                    now.saturating_sub(since) as f32 / 1000.0,
                    why.replace(' ', "_"),
                    view.place(placer).map_or(-1.0, |b| b.feet.distance_xz(c.at)),
                    camper::walk_state(bot_of(placer), c.at)
                );
                camper::count(team, camper::GAVE_UP);
                let playing = bot_of(placer).and_then(|b| b.assignment.as_ref()).is_some_and(|a| a.kind == PlayKind::TrapSpot);
                let near = view.place(placer).is_some_and(|b| b.feet.distance_xz(c.at) <= j.within_m + t.reach_m + 1.0);
                if why == "dead" {
                    let at = view.deaths.iter().rev().find(|d| d.place == placer).map(|d| d.at);
                    camper::died(team, placer, now, c.kind, at.map(|at| {
                        let seen = super::aimpoint::body_shows(&tactics.world.sight, super::lane_ahead::camper_eye(c.held), at, false);
                        (at.distance_xz(c.at), at.distance_xz(c.held), seen)
                    }), c.shooter);
                }
                camper::count(
                    team,
                    if why == "dead" {
                        camper::GAVE_UP_DEAD
                    } else if !playing {
                        camper::GAVE_UP_NOT_PLAYING
                    } else if near {
                        camper::GAVE_UP_AT
                    } else {
                        camper::GAVE_UP_FAR
                    },
                );
            }
            said.push(format!(
                "team {team}: bot {placer} let go of {}'s way in at {}: {why}; at the point its gate refused {} thinks for an enemy seen ({})",
                c.spot,
                xyz(c.at),
                c.seen.total(),
                c.seen.said()
            ));
            c.state = CamperTrapState::Open;
        }
        true
    });
    // [bot traps T1] A place no longer held keeps its point for the team's memory of a camp place
    // (`threat::CAMP_KILLS_KEPT_MS`): no bot is sent to it, a mate whose walk passes it lays the trap (`think`), and a bot sent
    // while it was held is let go (its play ends at its next think: `camper_point` finds no point sent to it); walked on, those
    // walks cost 15 more deaths of sent bots in 96 bench matches for 6 more traps. What is over: a point that long past its hold.
    // A point with a standing trap of the team on it is trapped (nothing sent, not searched again); when that trap is gone it
    // wants another.
    let standing: Vec<Vec3> = knowledge.traps.standing().map(|k| k.at).collect();
    knowledge.camper_traps.retain_mut(|c| {
        let held = sources.iter().any(|l| l.id == c.spot);
        match (c.hold, held) {
            (CamperHold::Over { .. }, true) => c.hold = CamperHold::Held,
            (CamperHold::Held, false) => {
                c.hold = CamperHold::Over { since: now };
                if let CamperTrapState::Sent { placer, .. } = c.state {
                    c.state = CamperTrapState::Open;
                    said.push(format!("team {team}: {}'s way in at {}: its place is no longer held; bot {placer} is let go", c.spot, xyz(c.at)));
                }
                #[cfg(test)]
                camper::count(team, camper::KEPT);
                said.push(format!(
                    "team {team}: {}'s way in at {}: the place is no longer held; the point is kept {:.0} s for a mate's walk past it",
                    c.spot,
                    xyz(c.at),
                    super::threat::CAMP_KILLS_KEPT_MS as f32 / 1000.0
                ));
            }
            _ => {}
        }
        let over = matches!(c.hold, CamperHold::Over { since } if now.saturating_sub(since) > super::threat::CAMP_KILLS_KEPT_MS)
            .then_some("its place is not held again since it was kept");
        let trap_on = standing.iter().any(|s| s.distance_xz(c.at) <= j.within_m);
        match c.state {
            CamperTrapState::Open if over.is_none() && trap_on => {
                c.state = CamperTrapState::Trapped;
                said.push(format!("team {team}: {}'s way in at {}: a trap of the team stands on it; trapped while it stands", c.spot, xyz(c.at)));
            }
            CamperTrapState::Trapped if over.is_none() && !trap_on => {
                c.state = CamperTrapState::Open;
                #[cfg(test)]
                camper::count(team, camper::TRAP_GONE);
                said.push(format!("team {team}: {}'s way in at {}: its trap is gone; it wants another", c.spot, xyz(c.at)));
            }
            _ => {}
        }
        if let Some(why) = over {
            #[cfg(test)]
            camper::count(team, camper::DROPPED);
            #[cfg(test)]
            {
                let sent = match c.state {
                    CamperTrapState::Sent { placer, since, .. } => format!(
                        "sent placer {placer} walked {:.1} left {:.1} {}",
                        now.saturating_sub(since) as f32 / 1000.0,
                        view.place(placer).map_or(-1.0, |b| b.feet.distance_xz(c.at)),
                        camper::walk_state(bots.iter().flatten().find(|b| b.place == placer), c.at)
                    ),
                    CamperTrapState::Open => "open".to_string(),
                    CamperTrapState::Trapped => "trapped".to_string(),
                };
                eprintln!("  camper life: team {team} spot {} {:?} dropped t {now} {sent}", c.spot, c.kind);
            }
            said.push(format!("team {team}: {}'s way in at {} dropped: {why}", c.spot, xyz(c.at)));
        }
        over.is_none()
    });
    knowledge.camper_no_way.retain(|spot| sources.iter().any(|l| l.id == *spot));
    // The way in of each held place without a point, searched once while it is held.
    let homes: Vec<Vec3> = tactics.world.map.spawns.iter().filter(|s| Some(s.team) != side).map(|s| s.position).collect();
    let nearest_home = |at: Vec3| homes.iter().copied().min_by(|a, b| a.distance_xz(at).total_cmp(&b.distance_xz(at)));
    for line in sources {
        if knowledge.camper_traps.iter().any(|c| c.spot == line.id) || knowledge.camper_no_way.contains(&line.id) {
            continue;
        }
        let kind = if line.battery.is_some() { CamperTrapKind::Battery } else { CamperTrapKind::Job };
        match nearest_home(line.at).ok_or(NoWayIn::NoFloor).and_then(|home| way_in(tactics, home, line.at, &j)) {
            Ok(at) => {
                // [bot traps T1] One point a way in: a kept point within `near_own_m` (its place no longer held, no bot walking
                // to it) gives way to this one; a point of a place still held there, or one a bot walks to, keeps it.
                if let Some(other) = knowledge.camper_traps.iter().position(|c| c.at.distance_xz(at) <= t.near_own_m) {
                    let o = &knowledge.camper_traps[other];
                    if matches!(o.hold, CamperHold::Over { .. }) && !matches!(o.state, CamperTrapState::Sent { .. }) {
                        said.push(format!("team {team}: {}'s kept way in at {} gives way to {}'s", o.spot, xyz(o.at), line.id));
                        knowledge.camper_traps.remove(other);
                    } else {
                        said.push(format!(
                            "team {team}: {:?} {} held by player {} at {}: its way in at {} is {}'s point",
                            kind,
                            line.id,
                            line.shooter,
                            xyz(line.at),
                            xyz(at),
                            o.spot
                        ));
                        knowledge.camper_no_way.push(line.id);
                        continue;
                    }
                }
                knowledge.camper_traps.push(CamperTrap {
                    spot: line.id,
                    shooter: line.shooter,
                    held: line.at,
                    at,
                    kind,
                    state: CamperTrapState::Open,
                    hold: CamperHold::Held,
                    seen: SeenRefusals::default(),
                });
                #[cfg(test)]
                camper::count(team, if kind == CamperTrapKind::Battery { camper::NOTED_BATTERY } else { camper::NOTED_JOB });
                #[cfg(test)]
                eprintln!(
                    "  camper life: team {team} spot {} {:?} noted t {now} seen_age {}",
                    line.id,
                    kind,
                    line.seen.map_or("-".to_string(), |s| format!("{:.1}", now.saturating_sub(s) as f32 / 1000.0))
                );
                said.push(format!("team {team}: {:?} {} held by player {} at {}: his way in at {} wants a trap", kind, line.id, line.shooter, xyz(line.at), xyz(at)));
            }
            Err(why) => {
                knowledge.camper_no_way.push(line.id);
                #[cfg(test)]
                {
                    camper::count(team, camper::NO_WAY);
                    camper::count(team, why.count_kind());
                }
                said.push(format!("team {team}: {:?} {} held by player {} at {}: no way in: {}", kind, line.id, line.shooter, xyz(line.at), why.words()));
            }
        }
    }
    // Who goes: [bot traps T1] to an open point of a place still held only (a kept point is laid in passing, `think`).
    let mut taken: Vec<super::view::Place> = knowledge
        .camper_traps
        .iter()
        .filter_map(|c| match c.state {
            CamperTrapState::Sent { placer, .. } => Some(placer),
            CamperTrapState::Open | CamperTrapState::Trapped => None,
        })
        .collect();
    for c in knowledge.camper_traps.iter_mut().filter(|c| c.state == CamperTrapState::Open && c.hold == CamperHold::Held) {
        let pick = bots
            .iter()
            .flatten()
            .filter(|b| b.team == team && !taken.contains(&b.place))
            .filter(|b| b.features.traps.habit.may_plan(t).is_ok())
            .filter(|b| b.assignment.as_ref().is_none_or(|a| !urgent(a.kind) && !matches!(a.kind, PlayKind::Flank | PlayKind::Crossfire | PlayKind::Breach)))
            .filter_map(|b| {
                let body = view.place(b.place).filter(|p| p.alive && p.carrying.is_none())?;
                let d = body.feet.distance_xz(c.at);
                (d <= j.max_walk_m).then_some((b, d))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1));
        let Some((b, d)) = pick else {
            #[cfg(test)]
            camper::count(team, camper::NO_PLACER);
            continue;
        };
        c.state = CamperTrapState::Sent { placer: b.place, since: now, placed_before: b.features.traps.habit.placed_this_life() };
        c.seen = SeenRefusals::default();
        taken.push(b.place);
        #[cfg(test)]
        camper::count(team, camper::SENT);
        #[cfg(test)]
        {
            let seen = sources.iter().find(|l| l.id == c.spot).and_then(|l| l.seen);
            eprintln!(
                "  camper life: team {team} spot {} {:?} sent t {now} placer {} d {d:.1} seen_age {}",
                c.spot,
                c.kind,
                b.place,
                seen.map_or("-".to_string(), |s| format!("{:.1}", now.saturating_sub(s) as f32 / 1000.0))
            );
        }
        said.push(format!("team {team}: bot {} sent to trap {}'s way in at {} ({:?} of player {}), {d:.1} m off", b.place, c.spot, xyz(c.at), c.kind, c.shooter));
    }
    for text in said {
        notes.push(super::Note { place: usize::MAX, time: now, text });
    }
}

/// [A324] The camper trap point `place` is sent to, if any (`TrapTuning::camper_job`).
pub fn camper_point(knowledge: &super::know::TeamKnowledge, place: super::view::Place) -> Option<&super::know::CamperTrap> {
    knowledge.camper_traps.iter().find(|c| matches!(c.state, super::know::CamperTrapState::Sent { placer, .. } if placer == place))
}

/// [A324] The bench's counts of the camper trap jobs, by team: points noted (a K2 job's spot, a battery hold, a killed holder's
/// place), points kept for his return, dropped, places with no way in out of his sight, ticks with an open point and no free bot,
/// bots sent, let go (by why) and traps put down by a sent bot; the sent bots' thinks by outcome and the job gate's refusals by
/// reason, the "enemy seen" ones split by whom (its camper, another) and the bot's pose, with the nearest enemy's distances; and
/// the plans (placer, time, the point's holder) to match the catches by. Kept per thread (a match runs on one) for the tallies,
/// and summed over threads for the bench's line.
#[cfg(test)]
pub mod camper {
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicU64, Ordering};

    pub const NOTED_JOB: usize = 0;
    pub const NOTED_BATTERY: usize = 1;
    pub const DROPPED: usize = 2;
    pub const NO_WAY: usize = 3;
    pub const NO_PLACER: usize = 4;
    pub const SENT: usize = 5;
    pub const GAVE_UP: usize = 6;
    pub const PLACED: usize = 7;
    /// Let go: dead; its play was not `TrapSpot` then; it was farther from the point than it places from; it was at the point.
    pub const GAVE_UP_DEAD: usize = 8;
    pub const GAVE_UP_NOT_PLAYING: usize = 9;
    pub const GAVE_UP_FAR: usize = 10;
    pub const GAVE_UP_AT: usize = 11;
    /// Thinks of a sent bot: on its way; at the point refused by the gate; no look; no spot by the point; waited for it; a trap
    /// planned by the point; one planned elsewhere; a `TrapSpot` play whose job is over (nothing planned).
    pub const THINK_WALKING: usize = 12;
    pub const THINK_GATE: usize = 13;
    pub const THINK_NO_LOOK: usize = 14;
    pub const THINK_NO_SPOT: usize = 15;
    pub const THINK_WAITED: usize = 16;
    pub const THINK_PLANNED: usize = 17;
    pub const THINK_PLANNED_ELSE: usize = 18;
    pub const THINK_JOB_OVER: usize = 19;
    /// The gate's "enemy seen" refusals at the point: its camper seen (crouched, standing), only others seen (crouched, standing).
    pub const SEEN_CAMPER_CROUCHED: usize = 20;
    pub const SEEN_CAMPER_STANDING: usize = 21;
    pub const SEEN_OTHER_CROUCHED: usize = 22;
    pub const SEEN_OTHER_STANDING: usize = 23;
    /// A trapped point whose trap is gone (fired, disarmed, a promise unconfirmed): open again.
    pub const TRAP_GONE: usize = 24;
    /// [bot traps] The places with no way in (`NO_WAY`) by why (`NoWayIn`): no floor; no route (no way ever, behind a zone on all
    /// match, or none found); a way too short; every point in a spawn area; every point in his sight (or some in a spawn area).
    pub const NO_FLOOR: usize = 25;
    pub const NO_ROUTE: usize = 26;
    pub const TOO_SHORT: usize = 27;
    pub const IN_SPAWN: usize = 28;
    pub const ALL_SEEN: usize = 29;
    /// [bot traps T1] Points kept past their hold; thinks of a bot whose walk passes a kept or open point it may trap; traps
    /// planned on one so.
    pub const KEPT: usize = 30;
    pub const PASS_WISHED: usize = 31;
    pub const PASS_PLANNED: usize = 32;
    pub const N: usize = 33;
    pub const NAMES: [&str; N] = [
        "noted_job", "noted_battery", "dropped", "no_way", "no_placer_ticks", "sent", "gave_up", "placed",
        "gave_up_dead", "gave_up_not_playing", "gave_up_far", "gave_up_at", "think_walking", "think_gate", "think_no_look",
        "think_no_spot", "think_waited", "think_planned", "think_planned_else", "think_job_over", "seen_camper_crouched",
        "seen_camper_standing", "seen_other_crouched", "seen_other_standing", "trap_gone", "no_way_no_floor", "no_way_no_route",
        "no_way_too_short", "no_way_in_spawn", "no_way_all_seen", "kept", "pass_wished", "pass_planned",
    ];
    static TEAM: [[AtomicU64; N]; 2] = [const { [const { AtomicU64::new(0) }; N] }; 2];
    /// The gate's refusals of a sent bot at its point, in `gates::of` order from Play on.
    pub const GATE_NAMES: [&str; 8] = ["play", "carrying", "health", "enemy", "hit", "reloading", "team_cap", "other"];
    static GATE: [[AtomicU64; 8]; 2] = [const { [const { AtomicU64::new(0) }; 8] }; 2];
    /// The nearest enemy seen at an "enemy seen" refusal, metres: [its camper seen, only others].
    static SEEN_M: Mutex<[Vec<f32>; 2]> = Mutex::new([Vec::new(), Vec::new()]);
    /// The sent bots' deaths, `time:place:team:kind:to point/to his place/seen from him:holder`.
    static DEATHS: Mutex<Vec<String>> = Mutex::new(Vec::new());

    thread_local! {
        static MINE: std::cell::Cell<[[u64; N]; 2]> = const { std::cell::Cell::new([[0; N]; 2]) };
        static PLANS: std::cell::RefCell<Vec<(usize, u32, usize)>> = const { std::cell::RefCell::new(Vec::new()) };
    }

    pub fn count(team: u8, kind: usize) {
        TEAM[usize::from(team & 1)][kind].fetch_add(1, Ordering::Relaxed);
        MINE.with(|cell| {
            let mut all = cell.get();
            all[usize::from(team & 1)][kind] += 1;
            cell.set(all);
        });
    }

    pub fn gate(team: u8, k: usize) {
        GATE[usize::from(team & 1)][k.min(7)].fetch_add(1, Ordering::Relaxed);
    }

    /// An "enemy seen" refusal at the point: its camper among those seen or not, the bot's pose, the nearest seen (metres).
    pub fn seen(team: u8, camper: bool, crouched: bool, nearest_m: f32) {
        count(
            team,
            match (camper, crouched) {
                (true, true) => SEEN_CAMPER_CROUCHED,
                (true, false) => SEEN_CAMPER_STANDING,
                (false, true) => SEEN_OTHER_CROUCHED,
                (false, false) => SEEN_OTHER_STANDING,
            },
        );
        if let Ok(mut m) = SEEN_M.lock() {
            m[usize::from(!camper)].push(nearest_m);
        }
    }

    /// A sent bot died: its team, place, the time, its point's kind, where it died (metres to its point, to his place, and whether
    /// a body standing there shows from his eye), and the point's holder.
    pub fn died(team: u8, placer: usize, now: u32, kind: super::super::know::CamperTrapKind, at: Option<(f32, f32, bool)>, holder: usize) {
        let kind = match kind {
            super::super::know::CamperTrapKind::Job => "job",
            super::super::know::CamperTrapKind::Battery => "battery",
        };
        let at = at.map_or("-".to_string(), |(p, h, seen)| format!("{p:.1}/{h:.1}/{}", if seen { "seen" } else { "hidden" }));
        if let Ok(mut d) = DEATHS.lock() {
            d.push(format!("{now}:{placer}:{team}:{kind}:{at}:{holder}"));
        }
    }

    static ARRIVED: Mutex<Vec<(u8, u32, u32)>> = Mutex::new(Vec::new());

    /// [#2081 v6 measure] A sent bot's walk now: its play, its go (metres from the point), its route (its length, its goal's
    /// metres from the point), its hold and pace.
    pub fn walk_state(bot: Option<&super::Bot>, point: mapdata::Vec3) -> String {
        let Some(b) = bot else {
            return "bot -".to_string();
        };
        let go = match b.intent.go {
            crate::v026g::bots::plays::Go::Stay => "stay".to_string(),
            crate::v026g::bots::plays::Go::To { at, .. } => format!("to_point {:.1}", at.distance_xz(point)),
        };
        let route = b.route.as_ref().map_or("none".to_string(), |(r, goal)| format!("{:.1}m goal_point {:.1}", r.length(), goal.distance_xz(point)));
        format!("play {:?} go {go} route {route} hold {} pace {:?}", b.assignment.as_ref().map(|a| a.kind), b.intent.hold != 0, b.intent.pace)
    }

    /// [#2081 v6 measure] Whether this is the first tick the bot sent at `since` to `spot` is at its point.
    pub fn arrived(team: u8, spot: u32, since: u32) -> bool {
        ARRIVED.lock().is_ok_and(|mut a| {
            let new = !a.contains(&(team, spot, since));
            if new {
                a.push((team, spot, since));
            }
            new
        })
    }

    /// A trap planned by a sent bot `placer` at `now` on the way in of `holder`'s place.
    pub fn planned(placer: usize, now: u32, holder: usize) {
        PLANS.with(|p| p.borrow_mut().push((placer, now, holder)));
    }

    /// This thread's counts.
    pub fn counted() -> [[u64; N]; 2] {
        MINE.with(|cell| cell.get())
    }

    /// This thread's plans so far, from the `from`th on.
    pub fn plans(from: usize) -> Vec<(usize, u32, usize)> {
        PLANS.with(|p| p.borrow().get(from..).map(<[_]>::to_vec).unwrap_or_default())
    }

    pub fn line() -> String {
        let p50 = |v: &[f32]| {
            let mut v = v.to_vec();
            v.sort_by(f32::total_cmp);
            v.get(v.len() / 2).map_or("-".to_string(), |m| format!("{m:.1} m"))
        };
        let seen = SEEN_M.lock().map(|m| format!("; nearest enemy seen P50: with its camper {}, others only {}", p50(&m[0]), p50(&m[1]))).unwrap_or_default();
        let teams = (0..2)
            .map(|team| {
                let mut parts: Vec<String> = NAMES.iter().enumerate().map(|(k, n)| format!("{n} {}", TEAM[team][k].load(Ordering::Relaxed))).collect();
                parts.extend(GATE_NAMES.iter().enumerate().map(|(k, n)| format!("gate_{n} {}", GATE[team][k].load(Ordering::Relaxed))));
                format!("team {team}: {}", parts.join(", "))
            })
            .collect::<Vec<_>>()
            .join("; ");
        let deaths = DEATHS.lock().map(|d| format!("; sent bots' deaths [{}]", d.join(" "))).unwrap_or_default();
        format!("{teams}{seen}{deaths}")
    }
}

/// [#2081 v6 measure] The bench's record of each trap look (plan to its end, or to the placer's death): the nearest alive enemy
/// body at the plan and at the last think (the truth, any enemy, seen or not, as the live record measures humans), the bot's own
/// nearest track at the plan (metres, age, who), and the reflex look the trap look took the view over first (`look_over_heard`).
/// The deaths are joined with the stand-in game's kills by the bench (the killer).
/// [halt probe] Counts only: how long the view takes to come onto a look.
#[cfg(test)]
pub mod probe {
    use crate::v026g::bots::kin::TICK_S;
    use crate::v026g::bots::motor::Feel;

    /// Seconds for the view to close a yaw gap of `gap` radians to `tolerance`: the motor's own ease (`Motor::turn_fed`: a rate
    /// asked of `gap / turn_time_s`, within the speed cap of the bot's skill and `max_turn_accel`), tick by tick, from rest.
    pub fn turn_in_s(gap: f32, feel: &Feel, skill: f32, tolerance: f32) -> f32 {
        let turn_time = feel.turn_time_s.max(0.02);
        let cap = feel.max_turn_speed + skill.clamp(0.0, 1.0) * (feel.max_turn_speed_top - feel.max_turn_speed).max(0.0);
        let (mut left, mut rate, mut ticks) = (gap.abs(), 0.0f32, 0u32);
        while left > tolerance && ticks < 200 {
            let want = (left / turn_time).min(cap);
            let step = feel.max_turn_accel * TICK_S;
            rate += (want - rate).clamp(-step, step);
            left -= (rate * TICK_S).min(left);
            ticks += 1;
        }
        ticks as f32 * TICK_S
    }

    /// [halt look] The placer halts only while it looks or presses, calm, walking (not staying) and with no enemy known within
    /// 15 m; never with the switch off.
    #[test]
    fn a_placer_halts_for_its_look_only_where_the_pass_allows_it() {
        use super::{halts_for_look, State, TrapLegs, TrapTuning};
        use mapdata::Vec3;
        let on = TrapTuning::default();
        assert!(on.halt_look, "on by default");
        let looking = State::Looking { yaw: 0.0, spot: Vec3::ZERO, since: 0, last: None };
        let down = State::Down { yaw: 0.0, spot: Vec3::ZERO, crouched: false };
        assert!(halts_for_look(&on, looking, true, false, false));
        assert!(halts_for_look(&on, down, true, false, false), "through the press to the release");
        assert!(!halts_for_look(&on, State::Idle, true, false, false));
        assert!(!halts_for_look(&on, looking, false, false, false), "called away: not calm");
        assert!(!halts_for_look(&on, looking, true, true, false), "a bot that stays keeps its play's legs");
        assert!(!halts_for_look(&on, looking, true, false, true), "an enemy within 15 m");
        let off = TrapTuning { halt_look: false, ..TrapTuning::default() };
        assert!(!halts_for_look(&off, looking, true, false, false));
        assert!(TrapLegs::Halt.stay() && TrapLegs::AfterPress.stay() && !TrapLegs::Walk.stay());
    }

    #[test]
    fn a_wider_gap_takes_longer_and_none_takes_none() {
        let feel = Feel::default();
        assert_eq!(turn_in_s(0.01, &feel, 0.5, 0.07), 0.0);
        let (a, b) = (turn_in_s(0.5, &feel, 0.5, 0.07), turn_in_s(1.7, &feel, 0.5, 0.07));
        assert!(a > 0.0 && b > a, "{a} {b}");
    }
}

#[cfg(test)]
pub mod placing {
    use std::sync::Mutex;

    use super::OverHeard;

    #[derive(Clone, Copy, Debug, Default, PartialEq)]
    pub struct Ctx {
        pub place: usize,
        pub team: u8,
        pub planned: u32,
        pub last: u32,
        pub truth_plan_m: Option<f32>,
        pub truth_last_m: Option<f32>,
        pub track: Option<(f32, u32, usize)>,
        pub heard: Option<OverHeard>,
        /// [halt probe] (a) A candidate passes `choose` from the feet the bot will have at the press (feet + velocity times
        /// the view's predicted turn-in, [`super::probe::turn_in_s`]), and that turn-in, ms.
        pub predicted: Option<(bool, u32)>,
        /// [halt probe] (b) At the look's first tick, a press from the body halted there with the view on the look: `None` the
        /// press is ready (every landing rule passes), else the rule that keeps it back.
        pub halt: Option<Option<super::Why>>,
        /// [halt probe] When the view first came onto the look (yaw within the tolerance, pitch within the slack): the time a
        /// halt would hold the legs.
        pub turned: Option<u32>,
        /// [halt look] How long the legs were held for this look and its press, ms.
        pub halted_ms: u32,
    }

    impl Ctx {
        pub fn line(&self) -> String {
            let m = |v: Option<f32>| v.map_or("-".to_string(), |m| format!("{m:.1}"));
            let track = self.track.map_or("-".to_string(), |(m, age, who)| format!("{m:.1}/{:.1}/{who}", age as f32 / 1000.0));
            let heard = match self.heard {
                Some(OverHeard::Aim { m, age_ms }) => format!("aim {m:.1}/{:.1}", age_ms as f32 / 1000.0),
                Some(OverHeard::Glance { m }) => format!("glance {m:.1}"),
                None => "none".to_string(),
            };
            let predicted = self.predicted.map_or("-".to_string(), |(yes, ms)| format!("{} {ms}", if yes { "yes" } else { "no" }));
            let halt = match self.halt {
                None => "-".to_string(),
                Some(None) => "ready".to_string(),
                Some(Some(why)) => why.name().replace(' ', "_"),
            };
            let turned = self.turned.map_or("-".to_string(), |at| at.saturating_sub(self.planned).to_string());
            format!(
                "place {} team {} planned {} last {} look {:.1} s truth_plan {} truth_last {} track {track} over {heard} probe predicted {predicted} halt {halt} turned_ms {turned} halted_ms {}",
                self.place,
                self.team,
                self.planned,
                self.last,
                self.last.saturating_sub(self.planned) as f32 / 1000.0,
                m(self.truth_plan_m),
                m(self.truth_last_m),
                self.halted_ms
            )
        }
    }

    static DEATHS: Mutex<Vec<Ctx>> = Mutex::new(Vec::new());

    /// [halt look] The looks whose legs were held, by how they ended (placed, called away, gave up), and the time held, ms.
    static HALTS: [std::sync::atomic::AtomicU64; 4] = [const { std::sync::atomic::AtomicU64::new(0) }; 4];

    /// A look ended (`placed`, `called away`, `gave up`): its line at once.
    pub fn ended(c: &Ctx, how: &str) {
        eprintln!("  trap look end: {how} {}", c.line());
        if c.halted_ms > 0 {
            use std::sync::atomic::Ordering::Relaxed;
            let k = match how {
                "placed" => 0,
                "called_away" => 1,
                _ => 2,
            };
            HALTS[k].fetch_add(1, Relaxed);
            HALTS[3].fetch_add(u64::from(c.halted_ms), Relaxed);
        }
    }

    /// [halt look] The bench's line of the halted looks since the last call, set back to 0.
    pub fn halts_line() -> String {
        use std::sync::atomic::Ordering::Relaxed;
        let [placed, called, gave_up, ms] = [0, 1, 2, 3].map(|k| HALTS[k].swap(0, Relaxed));
        let looks = placed + called + gave_up;
        format!(
            "looks halted {looks}: placed {placed}, called away {called}, gave up {gave_up}; legs held {:.1} s ({:.2} s a look)",
            ms as f32 / 1000.0,
            if looks == 0 { 0.0 } else { ms as f32 / 1000.0 / looks as f32 }
        )
    }

    /// A life ended in the middle of a look or a press.
    pub fn died(c: Ctx) {
        if let Ok(mut d) = DEATHS.lock() {
            d.push(c);
        }
    }

    pub fn deaths() -> Vec<Ctx> {
        DEATHS.lock().map(|d| d.clone()).unwrap_or_default()
    }

    /// The nearest alive enemy body to `feet` in `view`, metres.
    pub fn truth(view: &super::super::view::MatchView, team: u8, feet: mapdata::Vec3) -> Option<f32> {
        view.places.iter().filter(|p| p.alive && p.team != team).map(|p| p.feet.distance_xz(feet)).min_by(f32::total_cmp)
    }
}

/// [A309] How far forward `at` lies on the way from `own` to `enemy`: `d_own / (d_own + d_enemy)` on the ground (0 at home, 1 at
/// the enemy's).
pub fn forward_share(own: Vec3, enemy: Vec3, at: Vec3) -> f32 {
    let (a, b) = (at.distance_xz(own), at.distance_xz(enemy));
    if a + b <= 1e-3 { 0.5 } else { a / (a + b) }
}

/// [A324 measure] Why trap looks are called away (`drive`'s calm false: a reflex look, fire, legs, a use or a target): the reflex's
/// seen target (by its distance), a threat look (`reflex::gate::THREAT`: a hit's shooter, a near shot), or another of its looks,
/// fire, legs or a use (a watch, a sound, a glance, a pre-aim); a `TrapSpot` bot's looks apart. Decisions are not touched.
#[cfg(test)]
pub mod called {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::{Bot, PlaceView};

    #[derive(Clone, Copy, Debug, PartialEq)]
    pub enum Why {
        Target(f32),
        Threat,
        Other(super::super::reflex::last_look::Kind),
    }

    impl Why {
        pub fn of(bot: &Bot, me: &PlaceView) -> Self {
            if let Some(who) = bot.reflex.target() {
                let at = bot.memory.tracks().iter().find(|t| t.who == who).map(|t| t.at);
                return Self::Target(at.map_or(f32::INFINITY, |at| at.distance_xz(me.feet)));
            }
            if bot.reflex.gates().bits & super::super::reflex::gate::THREAT != 0 {
                Self::Threat
            } else {
                Self::Other(super::super::reflex::last_look::get())
            }
        }
    }

    /// [habit, TrapSpot] x [target <15 m, 15-40, 40+ or unknown, threat, other, not judged, and of other: glance, aimed, look,
    /// legs or fire, none (a use)]
    static N: [[AtomicU64; 11]; 2] = [const { [const { AtomicU64::new(0) }; 11] }; 2];

    pub fn count(why: Option<Why>, job: bool) {
        use super::super::reflex::last_look::Kind;
        let k = match why {
            Some(Why::Target(m)) if m < 15.0 => 0,
            Some(Why::Target(m)) if m < 40.0 => 1,
            Some(Why::Target(_)) => 2,
            Some(Why::Threat) => 3,
            Some(Why::Other(_)) => 4,
            None => 5,
        };
        N[usize::from(job)][k].fetch_add(1, Ordering::Relaxed);
        if let Some(Why::Other(kind)) = why {
            let k = match kind {
                Kind::Glance => 6,
                Kind::Aimed => 7,
                Kind::Look => 8,
                Kind::LegsOrFire => 9,
                Kind::None => 10,
            };
            N[usize::from(job)][k].fetch_add(1, Ordering::Relaxed);
        }
    }

    /// [traps.look_over_heard] Looks that took the view over a reflex look: [a glance, a heard aim].
    static OVER: [AtomicU64; 2] = [const { AtomicU64::new(0) }; 2];

    pub fn over_heard(aim: bool) {
        OVER[usize::from(aim)].fetch_add(1, Ordering::Relaxed);
    }

    pub fn line() -> String {
        let over = format!(
            "; looks that took the view over a glance {}, over a heard aim {}",
            OVER[0].load(Ordering::Relaxed),
            OVER[1].load(Ordering::Relaxed)
        );
        (0..2)
            .map(|j| {
                let n = |k: usize| N[j][k].load(Ordering::Relaxed);
                format!(
                    "{}: target <15 m {}, 15-40 m {}, 40+ m {}, threat look {}, another look/fire/legs/use {}, not judged {} (other: glance {}, aimed {}, look {}, legs or fire {}, none {})",
                    if j == 0 { "habit" } else { "TrapSpot" },
                    n(0),
                    n(1),
                    n(2),
                    n(3),
                    n(4),
                    n(5),
                    n(6),
                    n(7),
                    n(8),
                    n(9),
                    n(10)
                )
            })
            .collect::<Vec<_>>()
            .join("; ")
            + &over
    }
}

/// [A145, A324] The bench's counts of the wished spot (a sent bot's camper job point): its thinks, waits and plans, and what refused the candidates by it.
#[derive(Clone, Copy, Debug)]
pub struct Wish<'a> {
    pub spots: &'a [Vec3],
    /// A candidate within this of one is wanted: it need not pass the lanes' `min_score`, it wins over any other, and a mate must
    /// be farther off than `friend_m` (now, in 2 s and in 4 s).
    pub within_m: f32,
    pub friend_m: f32,
}

impl Wish<'_> {
    fn wants(&self, at: Vec3) -> bool {
        self.spots.iter().any(|spot| spot.distance_xz(at) <= self.within_m)
    }
}

/// A spot picked: the yaw to look along, the floor point, its score.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Chosen {
    pub yaw: f32,
    pub spot: Vec3,
    pub score: f32,
    /// On the spot of a killed holder ([`Wish`]).
    pub wished: bool,
}

/// What a wished candidate is worth over every other (the scores are under 1).
const WISH_RANK: f32 = 2.0;

/// The unit vector of a yaw on the ground.
fn dir(yaw: f32) -> [f32; 2] {
    kin::forward(yaw)
}

/// Enemy presence at `at`: the lane counts of the enemy humans within
/// [`PRESENCE_M`], decayed, falling off linearly with the distance.
fn presence(lanes: &HumanLanes, at: Vec3, now: u32) -> f32 {
    lanes
        .humans
        .iter()
        .flat_map(|human| &human.cells)
        .map(|cell| {
            let near = 1.0 - cell.at.distance_xz(at) / PRESENCE_M;
            cell.count_at(now) * near.max(0.0)
        })
        .sum()
}

/// [A292 S1] The team's own senses of every enemy into the trap habit's lane map (`TrapLanes::Enemies`), by the rule
/// `TeamKnowledge::learn_humans_as` takes for the humans: its bots' own senses only (no callout), within a lane cell of error, the
/// latest place of each enemy at each poll, at most one a second into his lane (`HumanLanes::knew`).
pub(super) fn learn_enemy_lanes<'a>(lanes: &mut HumanLanes, team: u8, view: &super::view::MatchView, tracks: impl Iterator<Item = &'a Track>) {
    let enemy = |who: super::view::Place| view.place(who).is_some_and(|p| p.team & 1 != team & 1);
    let mut latest: Vec<(super::view::Place, Vec3, u32, f32)> = Vec::new();
    for t in tracks {
        if !t.source.own() || t.error_m > LANE_CELL_M || !enemy(t.who) {
            continue;
        }
        match latest.iter_mut().find(|l| l.0 == t.who) {
            Some(l) if (t.time, -t.error_m) > (l.2, -l.3) => *l = (t.who, t.at, t.time, t.error_m),
            Some(_) => {}
            None => latest.push((t.who, t.at, t.time, t.error_m)),
        }
    }
    latest.sort_by_key(|l| l.0);
    for (who, at, time, _) in latest {
        lanes.knew(who, at, time);
    }
}

/// The lanes the spot score reads (`TrapTuning::lanes`; [fix-team PvB traps] `facing` by whether the team faces an enemy human, the
/// humans' lanes before the commander's first tick).
fn lanes_of<'a>(t: &TrapTuning, state: &'a TeamState) -> &'a HumanLanes {
    match t.lanes.source(state.features.traps.faces_humans.unwrap_or(true)) {
        LaneSource::Humans => &state.knowledge.humans,
        LaneSource::Enemies => &state.features.traps.lanes,
    }
}

/// [A292] The enemy human whose lane cell lies nearest `at`, and how far (for the log only).
fn nearest_lane(lanes: &HumanLanes, at: Vec3) -> Option<(super::view::Place, f32)> {
    lanes
        .humans
        .iter()
        .flat_map(|human| human.cells.iter().map(move |cell| (human.who, cell.at.distance_xz(at))))
        .min_by(|a, b| a.1.total_cmp(&b.1))
}

/// A mate at `at` or where it will be in 2 and 4 s weighs on the spot:
/// 1 for each of the three that is within `friend_m`, 0 if none. The hard
/// rule is any one.
fn friend_flow(mates: &[(Vec3, [f32; 2])], at: Vec3, friend_m: f32) -> usize {
    mates
        .iter()
        .map(|(feet, v)| {
            [0.0_f32, 2.0, 4.0]
                .iter()
                .filter(|s| {
                    let p = Vec3::new(feet.x + v[0] * **s, feet.y, feet.z + v[1] * **s);
                    p.distance_xz(at) < friend_m
                })
                .count()
        })
        .sum()
}

/// The score of a spot: presence less friend flow, both in the units of
/// expected catches (`1 - exp(-x / scale)`).
pub fn score(presence: f32, friends: usize, t: &TrapTuning) -> f32 {
    let e = 1.0 - (-presence / t.presence_scale).exp();
    e - t.w_friend * friends as f32 * 0.5
}

/// Whether the floor at `spot` is level (every link of its floor within
/// 0.5 of rise per metre), a standing body fits, it is no lip, and floor
/// all round it is open (`open_floors` outside the trap's disc within 2 m,
/// as high as the spot to 0.4 m).
fn good_floor(grid: &WalkGrid, spot: Vec3, t: &TrapTuning) -> Option<Vec3> {
    let node = grid.locate(spot + Vec3::new(0.0, 0.5, 0.0), 0.6)?;
    if !grid.can_stand(node) || grid.is_lip(node) || grid.near_fall(node) {
        return None;
    }
    let at = grid.position(node);
    for (to, _) in grid.links(node) {
        let p = grid.position(to);
        let run = p.distance_xz(at).max(0.01);
        if (p.y - at.y).abs() / run > 0.5 {
            return None;
        }
    }
    let spot = Vec3::new(spot.x, at.y, spot.z);
    let area = [spot.x - 2.0, spot.z - 2.0, spot.x + 2.0, spot.z + 2.0];
    let open = grid
        .floors_in(area)
        .filter(|&n| {
            let p = grid.position(n);
            let d = p.distance_xz(spot);
            d > AVOID_M && d <= 2.0 && (p.y - spot.y).abs() < 0.4
        })
        .count() as u32;
    (open >= t.open_floors).then_some(spot)
}

/// The ground distance from `at` to the polyline through `points`; far if
/// there are none.
pub fn off_route_m(points: &[Vec3], at: Vec3) -> f32 {
    let mut best = f32::MAX;
    for pair in points.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let (dx, dz) = (b.x - a.x, b.z - a.z);
        let length2 = dx * dx + dz * dz;
        let along = if length2 > 1e-9 {
            (((at.x - a.x) * dx + (at.z - a.z) * dz) / length2).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let nearest = Vec3::new(a.x + along * dx, at.y, a.z + along * dz);
        best = best.min(nearest.distance_xz(at));
    }
    if points.len() == 1 {
        best = points[0].distance_xz(at);
    }
    best
}

/// The best spot of the eight round the bot, if one passes.
pub fn choose(s: &Search) -> Option<Chosen> {
    choose_counted(s, &mut [0; 9])
}

/// [`choose`], counting each refused candidate by [`tally::Candidate`] into `refused` (counts only: the pick is the same).
pub fn choose_counted(s: &Search, refused: &mut [u32; 9]) -> Option<Chosen> {
    use tally::Candidate;
    let mut refuse = |c: Candidate| refused[c.index()] += 1;
    let t = s.t;
    let mut best: Option<(Chosen, f32)> = None;
    for k in 0..YAWS {
        let yaw = std::f32::consts::TAU * k as f32 / YAWS as f32;
        let d = dir(yaw);
        let spot = Vec3::new(
            s.feet.x + d[0] * s.reach_m,
            s.feet.y,
            s.feet.z + d[1] * s.reach_m,
        );
        // [A145] A candidate on the spot of a killed holder is the one the carrier waited for.
        let wished = s.wish.is_some_and(|w| w.wants(spot));
        #[cfg(test)]
        if wished {
            defeated::count(defeated::CANDIDATES);
        }
        let Some(spot) = good_floor(s.grid, spot, t) else {
            refuse(Candidate::Floor);
            #[cfg(test)]
            {
                gates::count(gates::R_FLOOR);
                if wished {
                    defeated::count(defeated::R_FLOOR);
                }
            }
            continue;
        };
        // With the landing rule the candidate lies where the look for its own floor lands (a spot higher or lower than
        // the feet is reached by another steepness); none where no look reaches.
        let spot = match s.press {
            None => spot,
            Some((crouched, wanted)) => {
                let Some(look) = press_look(crouched, wanted, s.feet.y - spot.y) else {
                    refuse(Candidate::NoLook);
                    #[cfg(test)]
                    {
                        gates::count(gates::NO_LOOK);
                        if wished {
                            defeated::count(defeated::R_LOOK);
                        }
                    }
                    continue;
                };
                Vec3::new(s.feet.x + d[0] * look.run_m, spot.y, s.feet.z + d[1] * look.run_m)
            }
        };
        if (s.in_spawn)(spot) {
            refuse(Candidate::Spawn);
            #[cfg(test)]
            {
                gates::count(gates::R_SPAWN);
                if wished {
                    defeated::count(defeated::R_SPAWN);
                }
            }
            continue;
        }
        if s.own.iter().any(|own| own.distance_xz(spot) < t.near_own_m) {
            refuse(Candidate::Own);
            #[cfg(test)]
            {
                gates::count(gates::R_OWN);
                if wished {
                    defeated::count(defeated::R_OWN);
                }
            }
            continue;
        }
        // Off its own route ahead, unless it stands.
        if !s.staying && off_route_m(s.ahead, spot) < t.lateral_m.max(AVOID_M) - 1e-3 {
            refuse(Candidate::Route);
            #[cfg(test)]
            {
                gates::count(gates::R_ROUTE);
                if wished {
                    defeated::count(defeated::R_ROUTE);
                }
            }
            continue;
        }
        // [A309] Only forward enough on the way to the enemy's home: nearer its own, enemies rarely come.
        if let Some((own, enemy, least)) = s.homes
            && forward_share(own, enemy, spot) < least
        {
            refuse(Candidate::LowScore);
            continue;
        }
        // Where its own body will be: it keeps off the disc.
        if s.feet.distance_xz(spot) < AVOID_M - 0.2 {
            refuse(Candidate::Body);
            #[cfg(test)]
            {
                gates::count(gates::R_BODY);
                if wished {
                    defeated::count(defeated::R_BODY);
                }
            }
            continue;
        }
        // The mates are the ones who walk onto a killed holder's spot next: a wider guard there.
        let friend_m = match s.wish {
            Some(w) if wished => w.friend_m.max(t.friend_m),
            _ => t.friend_m,
        };
        let friends = friend_flow(s.mates, spot, friend_m);
        if friends > 0 {
            refuse(Candidate::Friend);
            #[cfg(test)]
            {
                gates::count(gates::R_FRIEND);
                if wished {
                    defeated::count(defeated::R_FRIEND);
                }
            }
            continue;
        }
        let present = presence(s.lanes, spot, s.now);
        let value = score(present, friends, t);
        if value < t.min_score && !wished {
            refuse(if present > 0.0 { Candidate::LowScore } else { Candidate::NoLane });
            #[cfg(test)]
            gates::count(gates::R_SCORE);
            continue;
        }
        // The better score wins; a look that turns the view less is worth a
        // little (a half turn costs 0.1).
        let rank = value - 0.1 * turn(s.view_yaw, yaw).abs() / std::f32::consts::PI + if wished { WISH_RANK } else { 0.0 };
        if best.is_none_or(|(_, r)| rank > r) {
            best = Some((
                Chosen {
                    yaw,
                    spot,
                    score: value,
                    wished,
                },
                rank,
            ));
        }
    }
    best.map(|(chosen, _)| chosen)
}

/// Where a trap let go of lands: the client puts it where the head ray of
/// the frame the key came up meets the floor (`floor_y`, the level of the
/// spot the habit planned), [`PLACE_RAY_M`] at most. The habit looks down
/// and presses while the bot goes on walking (about 0.6 s), so this is
/// not the planned spot; `None` where the ray is too short to reach the
/// floor (nothing is put down).
pub fn landing(eye: Vec3, yaw: f32, pitch: f32, floor_y: f32) -> Option<Vec3> {
    let down = -pitch.sin();
    let rise = eye.y - floor_y;
    if down <= 1e-3 || rise <= 0.0 {
        return None;
    }
    let length = rise / down;
    if length > PLACE_RAY_M {
        return None;
    }
    let run = length * pitch.cos();
    let d = dir(yaw);
    Some(Vec3::new(eye.x + d[0] * run, floor_y, eye.z + d[1] * run))
}

/// [A324] The client's ground for a trap: flat at least this (normal.y, cos 30 deg; `bear_traps.md`, PROVEN).
pub const PLACE_FLAT_Y: f32 = 0.866;
/// [A324] A trap is not placed on an armed trap's sensor: within [`TRIGGER_M`] on the ground and this high (PROVEN).
pub const SENSOR_TOP_M: f32 = 0.22;

/// [A324] Where a trap let go of from `eye` with the view `yaw`, `pitch` lies by the client's rule (`TrapRelease::Client`):
/// the head ray meets `mesh` within [`PLACE_RAY_M`], on ground flat to [`PLACE_FLAT_Y`], not on the sensor of a trap at
/// `traps`; else why the client places nothing. `mesh` is the map's walker mesh (`Tactics::trap_mesh`): the client's ray
/// meets the walker's objects, not the bullet-only ones (0x3064B6). The ray starts at the head transform the bullet leaves
/// along (`base_player+0x44C`: its place +0x47C, its forward +0x46C, 0x306401..0x306417), which `eye` and `pitch` model
/// (`view::eye_height`, `pitch::PitchScale::Client`).
pub fn client_landing(mesh: &mapdata::RayTree, eye: Vec3, yaw: f32, pitch: f32, traps: &[Vec3]) -> Result<Vec3, Why> {
    let d = dir(yaw);
    let ray = Vec3::new(d[0] * pitch.cos(), pitch.sin(), d[1] * pitch.cos());
    let hit = mesh.ray_hit(eye, ray, PLACE_RAY_M).ok_or(Why::NoFloor)?;
    if hit.normal.y < PLACE_FLAT_Y {
        return Err(Why::Slope);
    }
    let at = eye + ray * hit.t;
    if traps.iter().any(|trap| trap.distance_xz(at) <= TRIGGER_M && (at.y - trap.y).abs() <= SENSOR_TOP_M) {
        return Err(Why::OnTrap);
    }
    Ok(at)
}

/// [fix-team trap pose] How far ahead of the feet, along the view's yaw, the client's trap ray starts, metres, by the view's
/// pitch in the game's unit from -1 (the steepest look down) to 0 (level), a row every 0.1, standing and crouched. The ray
/// starts at the head transform the bullet leaves along (`base_player+0x47C`, the `Item` bone times the body, 0x3063E0 ->
/// 0x2DA4B0), not over the feet. The rows are the stock look clip posed by the pose port (`hbpose.py`, the rifle's
/// `on_site_idle` + `on_site_idle_look`, as `pitch.rs`'s curve): at the stop 0.159 m standing, where the worlds measure the
/// eye 0.135-0.17 m ahead of the feet at battery uses (`using_the_world.md`), PROVEN there; between, the clip's own pose.
const ITEM_AHEAD_STANDING_M: [f32; 11] = [0.159, 0.153, 0.143, 0.131, 0.114, 0.095, 0.072, 0.046, 0.017, -0.014, -0.048];
const ITEM_AHEAD_CROUCHED_M: [f32; 11] = [0.117, 0.108, 0.098, 0.086, 0.073, 0.057, 0.039, 0.020, -0.001, -0.025, -0.050];

/// [fix-team trap pose] [`ITEM_AHEAD_STANDING_M`] / [`ITEM_AHEAD_CROUCHED_M`] at the view `view_pitch` (an elevation,
/// radians, up positive; the client's unit by `PitchScale::Client`), straight between the rows; a look up as level.
pub(crate) fn item_ahead_m(crouched: bool, view_pitch: f32) -> f32 {
    let rows = if crouched { &ITEM_AHEAD_CROUCHED_M } else { &ITEM_AHEAD_STANDING_M };
    let units = super::pitch::Pitch::of_elevation(view_pitch, super::pitch::PitchScale::Client).units().clamp(-1.0, 0.0);
    let at = (units + 1.0) * 10.0;
    let i = (at.floor() as usize).min(rows.len() - 2);
    let f = at - i as f32;
    rows[i] + (rows[i + 1] - rows[i]) * f
}

/// [fix-team trap pose] Where the client's trap ray starts for a body at `feet` with the view `yaw`, `view_pitch` (an
/// elevation, up positive), crouched or standing, at `speed`: [`item_ahead_m`] ahead of the feet along the yaw, at the eye's
/// height (`view::eye_height`, measured within sd 0.03 m).
pub(crate) fn item_eye(feet: Vec3, yaw: f32, crouched: bool, speed: f32, view_pitch: f32) -> Vec3 {
    let d = dir(yaw);
    let ahead = item_ahead_m(crouched, view_pitch);
    feet + Vec3::new(d[0] * ahead, super::view::eye_height(crouched, speed, view_pitch), d[1] * ahead)
}

/// The eye a body at `feet` has with the view `yaw`, `view_pitch` (up positive), crouched or standing ([`item_eye`]), the
/// crouched one at the height of holding still (`view::eye_height`): the lower, so a trap lands at least as far as counted on.
fn eye_for(feet: Vec3, yaw: f32, crouched: bool, view_pitch: f32) -> Vec3 {
    item_eye(feet, yaw, crouched, 0.0, view_pitch)
}

/// [fix-team trap pose] How far the body's own reckoned feet may be off the client's at a press, metres, along the view: the
/// press holds only where the landing holds from the feet moved this far back and forth. The reckoning's error a walker has
/// about a second after a world is a median 0.20 m, p90 0.45 m (`fact-own-eye` cause 2, PROVEN live); 0.25 m is that median
/// and a margin, a design value.
pub const FEET_ERROR_M: f32 = 0.25;

/// The point to look at for a yaw: along it and down at `pitch`, as far as the head ray goes.
pub fn look_point(eye: Vec3, yaw: f32, pitch: f32) -> Vec3 {
    let d = dir(yaw);
    let (run, rise) = (pitch.cos() * PLACE_RAY_M, pitch.sin() * PLACE_RAY_M);
    Vec3::new(eye.x + d[0] * run, eye.y - rise, eye.z + d[1] * run)
}

/// How a pose looks down at the floor: the steepness, and where the head ray then meets the floor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PressLook {
    pub pitch: Down,
    /// Along the ground from the feet, metres.
    pub run_m: f32,
}

/// The steepest look from the pose (`crouched`) whose head ray meets a floor `drop_m` below the feet (negative: above)
/// at least `run_m` from them, leaving [`RAY_MARGIN_M`] of the ray's 2.0 m; none if no look of the body does (the ray is
/// then too short to meet the floor). The eye falls as the view goes down (`view::eye_height`), so the same pitch
/// lands nearer for a crouched body: 0.55 m crouched and still, 0.85 m standing, at 1.0 rad on a level floor.
pub fn press_look(crouched: bool, run_m: f32, drop_m: f32) -> Option<PressLook> {
    const STEP: f32 = 0.01;
    let mut down = STEEPEST.0;
    while down > 0.2 {
        let rise = super::view::eye_height(crouched, 0.0, -down) + drop_m;
        if rise < 0.05 {
            return None;
        }
        if rise / down.sin() > PLACE_RAY_M - RAY_MARGIN_M {
            return None;
        }
        let run = rise / down.tan();
        if run >= run_m {
            return Some(PressLook {
                pitch: Down(down),
                run_m: run,
            });
        }
        down -= STEP;
    }
    None
}

/// The distance the look aims the landing at: `reach_m` or the landing the tuning asks, with the margin.
pub(crate) fn wanted_m(t: &TrapTuning, min_m: f32) -> f32 {
    t.reach_m.max(min_m + LANDING_MARGIN_M)
}

/// Whether `t`'s landing rule can be met from both poses on a level floor: some look reaches the distance it aims at.
/// Always so with the rule off.
pub fn press_reachable(t: &TrapTuning) -> bool {
    t.press_landing_m.is_none_or(|min_m| {
        let wanted = wanted_m(t, min_m);
        [false, true].iter().all(|&crouched| press_look(crouched, wanted, 0.0).is_some())
    })
}

impl super::Bots {
    /// How the trap looks of each team's bots ended: `[team 0, team 1]`.
    pub fn trap_look_outcomes(&self) -> [LookOutcomes; 2] {
        let mut sums = [LookOutcomes::default(); 2];
        for bot in self.bots.iter().flatten() {
            sums[usize::from(bot.team).min(1)].add(&bot.features.traps.habit.outcomes());
        }
        sums
    }
}

/// Whether a trap let go of at `landing` (what [`landing`] predicts from the pose at the release) is where `t` wants
/// it: at least `press_landing_m` from the feet, and, unless the bot stays, off its own route ahead by the lateral rule
/// (`ahead` is read only then). Always `Ready` with `press_landing_m` off.
pub fn press_of(
    t: &TrapTuning,
    landing: Option<Vec3>,
    feet: Vec3,
    staying: bool,
    ahead: impl FnOnce() -> Vec<Vec3>,
) -> Press {
    let Some(min_m) = t.press_landing_m else {
        return Press::Ready;
    };
    let Some(at) = landing else {
        return Press::Wait(Why::NoFloor);
    };
    if at.distance_xz(feet) < min_m {
        return Press::Wait(Why::TooNear);
    }
    if !staying && off_route_m(&ahead(), at) < t.lateral_m.max(AVOID_M) - 1e-3 {
        return Press::Wait(Why::OnRoute);
    }
    Press::Ready
}

impl TeamState {
    /// The team's field again if its list changed; whether it did.
    /// [keep_off] With `keep_off` the field shuts its floors within `hard_m` ([`OWN_TRAP_CLEAR_M`]), else within [`FIELD_M`].
    /// [#2081] No narrower fallback field: a search's route is checked by the same clearance ([`own_trap_route`]).
    pub(super) fn refresh_traps(&mut self, grid: &WalkGrid, ring_m: f32, keep_off: Option<super::tuning::KeepOff>) -> bool {
        let version = self.knowledge.traps.version();
        if version == self.features.traps.field_version {
            return false;
        }
        self.features.traps.field = field_of(&self.knowledge.traps, grid, ring_m, keep_off);
        self.features.traps.field_version = version;
        true
    }
}

/// The field of the standing traps of `list` ([`TeamState::refresh_traps`]).
fn field_of(list: &super::trap_know::TrapList, grid: &WalkGrid, ring_m: f32, keep_off: Option<super::tuning::KeepOff>) -> Arc<TrapField> {
    match keep_off {
        Some(k) => {
            let mut field = TrapField::of_ringed_within(list, grid, ring_m, k.hard_m);
            field.shut_promised_within(list, grid, k.hard_m + PROMISE_MARGIN_M);
            Arc::new(field)
        }
        None => Arc::new(TrapField::of_ringed(list, grid, ring_m)),
    }
}

/// [G61] The fields of a bot's team's traps with the ones only it knows ([`BotState::mine`]), and the versions of the two lists
/// (the team's, its own) they were built for.
pub struct MineFields {
    versions: (u64, u64),
    field: Arc<TrapField>,
}

/// [G61] The trap fields `bot` keeps off: its team's ([`TeamState::refresh_traps`]), and with them the traps only it knows
/// ([`BotState::mine`]) while one of those stands, built once for each version of the two lists.
pub(super) fn take_fields(bot: &mut Bot, state: &TeamState, grid: &WalkGrid, ring_m: f32, keep_off: Option<super::tuning::KeepOff>) {
    let traps = &mut bot.features.traps;
    if traps.mine.standing_count() == 0 {
        traps.mine_fields = None;
        traps.field = state.features.traps.field.clone();
        return;
    }
    let versions = (state.knowledge.traps.version(), traps.mine.version());
    if traps.mine_fields.as_ref().is_none_or(|built| built.versions != versions) {
        let field = field_of(&state.knowledge.traps.with(&traps.mine), grid, ring_m, keep_off);
        traps.mine_fields = Some(MineFields { versions, field });
    }
    if let Some(built) = &traps.mine_fields {
        traps.field = built.field.clone();
    }
}

/// How far ahead a route is kept off the traps' discs, metres.
const KEEP_OFF_M: f32 = 15.0;

/// [#2081] The one clearance from the team's own known traps, metres: the searches' shut discs (`TrapTuning::keep_off`'s
/// `hard_m` default), their check of the route they return ([`own_trap_route`]), [`keep_off`]'s check of the route ahead and the
/// fight's side step ([`step_keeps_clear`]) all read it, through the field's own reach ([`TrapField::reach`]), so a route the search
/// gives is never one `keep_off` drops. The contact [`OWN_TRAP_CONTACT_M`] + the walk's tolerance [`WALK_P90_M`]; the motor's
/// guard ([`own_trap_guard`]) holds the rest. (1.14 m, the contact + the tight walk's P90, was tried and dropped: the bench's
/// refusals at 1.47 m, 16 plain matches, showed 2 ways of 18 cases that 1.14 m would have let through, while 1.14 m left the guard
/// less room than the bodies' own errors, 4 placer catches in 16 matches.)
pub const OWN_TRAP_CLEAR_M: f32 = OWN_TRAP_CONTACT_M + WALK_P90_M;

/// [#2081] The feet off the route of the bots walking one, P90, metres: 261,076 samples (farming-camper cell seed 5, tip dab73194;
/// median 0.25, p95 1.25).
pub const WALK_P90_M: f32 = 0.80;

/// [#2081] The feet off the route of a body walking it within [`TIGHT_ZONE_M`] of a standing own trap, P90 and P50, metres: the
/// bench's 1,735 thinks of the walk mover (no weave, side step, step apart or sprint) in 16 plain matches (m8, Rudnya TDM and
/// Batteries, 600 s; the matches' quantiles weighted by their samples; P95 0.58, P99 0.83, max 1.32).
pub const TIGHT_WALK_P90_M: f32 = 0.47;
const TIGHT_WALK_P50_M: f32 = 0.18;

/// The slack of the clearance test, metres: a leg between two open floors of the walk grid (`mapdata::CELL` 0.25 m apart, a
/// diagonal 0.354 m) passes at most 0.011 m nearer a disc of 1.47 m than its two ends.
const CLEAR_SLACK_M: f32 = 0.05;

/// A trap this far above or below a leg's ends is on another floor (the field's own rule for its shut floors).
const OTHER_FLOOR_M: f32 = 1.0;

/// [#2081] Where a body touches a trap's sensor, metres between the trap's point and the body's centre: the sensor cylinder's radius
/// 0.25 m (`booby_trap_core::on_enter` 0x304BD0, `docs/notes/thrown_and_placed.md`) + the body capsule's radius 0.38 m (`mapdata`
/// `grid/controlled.rs`: startup.cfg's stand size 0.76 x 1.8) + the controller's 0.04 m move margin (`mapdata` `controller/tick.rs`).
/// Which shape the sensor tests is not proven; the 0.6 m of a foot point seen live (565 of 566 samples) lies inside it.
pub const OWN_TRAP_CONTACT_M: f32 = 0.67;

/// [#2081] The nearest a way from inside a disc ever comes to the trap, metres: the contact and the tight walk's median drift.
const INSIDE_FLOOR_M: f32 = OWN_TRAP_CONTACT_M + TIGHT_WALK_P50_M;

/// [#2081] Whether the polyline `points`, walked from `from`, keeps `reach` from every disc of `discs` on its floor: never nearer
/// than `reach`; when `from` is already nearer (a body inside a disc, as a placer by its fresh trap), never more than a floor of
/// the walk grid (`mapdata::CELL` 0.25 m: the feet's own floor may lie that much nearer) nearer than `from`, and never under
/// [`INSIDE_FLOOR_M`]: it walks out or round, never in (the searches' floors inside a disc are opened only farther than the feet,
/// [`keep_off_open`]). The one rule of the search's check, keep_off and the side step.
pub fn keeps_clear(points: &[Vec3], discs: &[Vec3], from: Vec3, reach: f32) -> bool {
    route_keeps_clear(points, discs, from, None, reach)
}

/// [bot traps] Whether every leg of `points` that [`route_keeps_clear`] refuses (walked from `from` to `goal`, at `reach`) joins a
/// disc's floor to another: one end within [`OTHER_FLOOR_M`] of the trap's height, the other not (a climb or a drop beside it).
/// The search's field shuts a floor by its own height, so such a leg's far end is open to it while the leg is refused here; a
/// leg on the trap's own floor is one the field would have kept off. False when nothing is refused.
fn refused_legs_cross_floors(points: &[Vec3], discs: &[Vec3], from: Vec3, goal: Vec3, reach: f32) -> bool {
    let mut refused = false;
    for disc in discs {
        let need = clearance_leaving(from.distance_xz(*disc), reach).min(clearance_for(goal.distance_xz(*disc), reach));
        let near = |p: Vec3| (p.y - disc.y).abs() < OTHER_FLOOR_M;
        for leg in points.windows(2) {
            if (near(leg[0]) || near(leg[1])) && off_route_m(leg, *disc) < need {
                if near(leg[0]) && near(leg[1]) {
                    return false;
                }
                refused = true;
            }
        }
    }
    refused
}

/// [#2081] The nearest `points` may come to a disc, for an end `at` metres from it ([`keeps_clear`]'s rule).
fn clearance_for(at: f32, reach: f32) -> f32 {
    if at >= reach { reach - CLEAR_SLACK_M } else { (at - mapdata::CELL).max(INSIDE_FLOOR_M).min(reach) }
}

/// [#2081] [`clearance_for`] for the start of a way: never above the feet's own distance (less the slack), so a body nearer than the
/// inside floor already walks out (the bench's tdm s7: a bot 0.71 m from its trap refused every way out 464 times). A goal keeps
/// [`clearance_for`]: a way never ends nearer than the inside floor.
fn clearance_leaving(at: f32, reach: f32) -> f32 {
    clearance_for(at, reach).min(at - CLEAR_SLACK_M)
}

/// [#2081] [`keeps_clear`] for a route to `goal`: its end is held to the same rule as its start, so a goal inside a disc (a guard's
/// spot by the trap it put down there, 1.2-1.4 m off on the bench) is reached, never nearer than a floor short of it nor under
/// [`INSIDE_FLOOR_M`]. A step (no goal) keeps the start's rule alone: it never steps in.
pub fn route_keeps_clear(points: &[Vec3], discs: &[Vec3], from: Vec3, goal: Option<Vec3>, reach: f32) -> bool {
    discs.iter().all(|disc| {
        let need = clearance_leaving(from.distance_xz(*disc), reach);
        let need = goal.map_or(need, |g| need.min(clearance_for(g.distance_xz(*disc), reach)));
        let near = |p: Vec3| (p.y - disc.y).abs() < OTHER_FLOOR_M;
        match points {
            [] => true,
            [one] => !near(*one) || one.distance_xz(*disc) >= need,
            _ => points.windows(2).all(|leg| (!near(leg[0]) && !near(leg[1])) || off_route_m(leg, *disc) >= need),
        }
    })
}

/// The polyline of `route` from its progress at `feet`, `ahead` metres of it (all of it with `f32::MAX`): the point there, the
/// route's own waypoints after it, and the point `ahead` on. Its legs are the route's legs, each at its own floor's height (points
/// sampled along it would join two legs of different floors: a jump above a trap read as a pass on its floor).
fn route_points(route: &super::path::Route, feet: Vec3, ahead: f32) -> Vec<Vec3> {
    // A copy: the motor's own progress along the route is not touched.
    let mut copy = route.clone();
    let (s, _) = copy.progress(feet);
    let end = (s + ahead).min(route.length());
    let mut points = vec![route.point_at(s)];
    let mut along = 0.0;
    for pair in route.points().windows(2) {
        along += pair[0].at.distance_xz(pair[1].at);
        if along > s && along < end {
            points.push(pair[1].at);
        }
    }
    points.push(route.point_at(end));
    points
}

/// [#2081] How far past the clearance a step out of an own trap's disc goes, metres: a body's drift off its route, median 0.25 m
/// ([`OWN_TRAP_CLEAR_M`]'s walk sample), rounded up to the grid's floors (`mapdata::CELL` 0.25 m) and their slack.
const STEP_OUT_PAST_M: f32 = 0.3;

/// [#2081] The route a search gave `bot`, if it keeps the clearance from its team's own traps ([`keeps_clear`] over all of it);
/// else none, with a note: the search's way is never one [`keep_off`] would drop at the next think. A body inside a disc whose
/// way is refused steps straight out of it first ([`step_out`]), as a player steps off his own trap, and searches from there.
pub(super) fn own_trap_route(
    bot: &mut Bot,
    feet: Vec3,
    route: Option<super::path::Route>,
    now: u32,
    tactics: &super::tactics::Tactics,
    objective: bool,
) -> Option<super::path::Route> {
    own_trap_route_within(bot, feet, route, now, tactics, objective, None)
}

/// [#2081] [`own_trap_route`] for a route the contact-only fallback search gave (`within` [`FALLBACK_REACH_M`]): held to that reach.
pub(super) fn own_trap_route_within(
    bot: &mut Bot,
    feet: Vec3,
    route: Option<super::path::Route>,
    now: u32,
    tactics: &super::tactics::Tactics,
    objective: bool,
    within: Option<f32>,
) -> Option<super::path::Route> {
    let route = route?;
    let field = bot.features.traps.field.clone();
    let reach = within.unwrap_or(field.reach().max(FIELD_M));
    if field.is_empty() || route_keeps_clear(&route_points(&route, feet, f32::MAX), field.discs(), feet, Some(route.end()), reach) {
        return Some(route);
    }
    #[cfg(test)]
    keep_off_counts::count(keep_off_counts::REFUSED);
    #[cfg(test)]
    {
        let points = route_points(&route, feet, f32::MAX);
        let end = route.point_at(route.length());
        // The disc the rule refuses the route by (the first that fails it), not the nearest across the ground over all.
        let (disc, pass) = field
            .discs()
            .iter()
            .find(|d| !route_keeps_clear(&points, std::slice::from_ref(*d), feet, Some(route.end()), reach))
            .map(|d| (*d, off_route_m(&points, *d)))
            .unwrap_or((feet, f32::MAX));
        // The leg of the checked polyline the rule refuses (an end within `OTHER_FLOOR_M` of the trap's height, nearer than the
        // clearance it needs): its pass, its ends' heights against the trap's, and that clearance.
        let need = clearance_leaving(feet.distance_xz(disc), reach).min(clearance_for(route.end().distance_xz(disc), reach));
        let near = |p: Vec3| (p.y - disc.y).abs() < OTHER_FLOOR_M;
        let leg = points
            .windows(2)
            .find(|w| (near(w[0]) || near(w[1])) && off_route_m(w, disc) < need)
            .map_or("-".to_string(), |w| {
                format!(
                    "refused leg ({:.1}, {:.1}, {:.1})->({:.1}, {:.1}, {:.1}) passes {:.2} m, ends y {:+.2}/{:+.2} from the trap, needs {need:.2} m",
                    w[0].x, w[0].y, w[0].z, w[1].x, w[1].y, w[1].z,
                    off_route_m(w, disc),
                    w[0].y - disc.y,
                    w[1].y - disc.y
                )
            });
        eprintln!(
            "  own trap route refused: t {now} place {} play {:?} start {:.2} m end {:.2} m nearest pass {pass:.2} m length {:.1} at ({:.1}, {:.1}, {:.1}); {leg}",
            bot.place,
            bot.assignment.as_ref().map(|a| a.kind),
            feet.distance_xz(disc),
            end.distance_xz(disc),
            route.length(),
            disc.x,
            disc.y,
            disc.z
        );
    }
    // [bot traps] A route the search gave whose every leg nearer than the clearance joins the trap's floor to another
    // ([`refused_legs_cross_floors`]: the search's field shuts floors by their own height, so it let the far end's floors through
    // and a search again gives the same way), and that keeps the guard's contact ([`FALLBACK_REACH_M`]), is taken at that
    // contact, as the fallback search's way is (marked as the fallback's when it is set, `Bot::set_route`). The bench's tdm-m7-s2 at d2993ba6d: 4 bots refused the one climb
    // out of a low spot (1.21 m from a team trap) 285 times; batt-m7-s2: a drop to a battery 1.34 m from one, 31 times.
    let points = route_points(&route, feet, f32::MAX);
    if within.is_none()
        && refused_legs_cross_floors(&points, field.discs(), feet, route.end(), reach)
        && route_keeps_clear(&points, field.discs(), feet, Some(route.end()), FALLBACK_REACH_M)
    {
        let pass = field.discs().iter().map(|d| off_route_m(&points, *d)).fold(f32::INFINITY, f32::min);
        own_trap_note(
            bot,
            now,
            format!(
                "a route of {:.1} m taken at the guard's contact ({FALLBACK_REACH_M:.2} m): the search gave no way at {reach:.2} m; it passes an own trap {pass:.2} m off",
                route.length()
            ),
        );
        bot.features.traps.taken_at_contact = Some(now);
        return Some(route);
    }
    own_trap_note(bot, now, format!("a route of {:.1} m refused: it passes an own trap nearer than {reach:.2} m", route.length()));
    let out = step_out(&field, feet, |to| tactics.clear_step_for(feet, to, now, bot.side, objective), &tactics.world.grid);
    if out.is_some() {
        own_trap_note(bot, now, "a step out of an own trap's disc instead".to_string());
    }
    out
}

/// The ways a step out of a disc tries, degrees off straight away from the trap, in order.
const STEP_OUT_TURNS_DEG: [f32; 5] = [0.0, 30.0, -30.0, 60.0, -60.0];

/// [#2081] From inside the disc of the own trap of `field` nearest `feet`, a straight step out to `OWN_TRAP_CLEAR_M` +
/// [`STEP_OUT_PAST_M`] from it: straight away, else 30 or 60 degrees off that ([`STEP_OUT_TURNS_DEG`]), the first whose end is a
/// floor of the feet's level, whose ground is clear for a step (`clear`) and which keeps the clearance from every disc. A two-point
/// route; none outside every disc or with no way out.
fn step_out(field: &TrapField, feet: Vec3, clear: impl Fn(Vec3) -> bool, grid: &WalkGrid) -> Option<super::path::Route> {
    use super::path::{Route, Waypoint};
    let reach = field.reach().max(FIELD_M);
    let disc = field
        .discs()
        .iter()
        .filter(|d| d.distance_xz(feet) < reach && (d.y - feet.y).abs() < OTHER_FLOOR_M)
        .min_by(|a, b| a.distance_xz(feet).total_cmp(&b.distance_xz(feet)))?;
    let away = (feet.z - disc.z).atan2(feet.x - disc.x);
    let from = grid.locate(feet, 1.5)?;
    let point = |at: Vec3, node| Waypoint { at, node, jump: None, low: false };
    STEP_OUT_TURNS_DEG.iter().find_map(|turn| {
        let a = away + turn.to_radians();
        let want = Vec3::new(disc.x + a.cos() * (reach + STEP_OUT_PAST_M), feet.y, disc.z + a.sin() * (reach + STEP_OUT_PAST_M));
        let to = grid.locate(want, 0.6)?;
        let end = grid.position(to);
        ((end.y - feet.y).abs() < 1.0 && clear(end) && keeps_clear(&[feet, end], field.discs(), feet, reach))
            .then(|| Route::new(vec![point(feet, from), point(end, to)]))
    })
}

/// [#2081] Whether the step from `from` to `to` keeps the clearance from the team's own traps of `field` ([`keeps_clear`]): the
/// fight's side step and the step apart from a mate take no step onto one.
pub(super) fn step_keeps_clear(field: &TrapField, from: Vec3, to: Vec3) -> bool {
    field.is_empty() || keeps_clear(&[from, to], field.discs(), from, field.reach().max(FIELD_M))
}

/// [#2081] Within this of a team trap a body on a fallback route walks tight ([`walks_tight`]), metres: the zone the tight walk's
/// tolerance ([`TIGHT_WALK_P90_M`]) was measured in (the bench's feet off route by an own trap).
pub const TIGHT_ZONE_M: f32 = 3.0;

/// [#2081] Whether `bot` walks tight at `feet`: on a fallback route ([`on_fallback`]: a way that passes a team trap nearer than the
/// clearance, held only to the guard's contact) within [`TIGHT_ZONE_M`] of a team trap on its floor. Then no sprint, side step,
/// weave or step apart (`drive.rs`): the walk's tolerance there is the tight walk's ([`TIGHT_WALK_P90_M`] 0.47 m), not every
/// walk's 0.80 m the clearance is made of.
pub(super) fn walks_tight(bot: &Bot, feet: Vec3) -> bool {
    on_fallback(bot)
        && bot
            .features
            .traps
            .field
            .discs()
            .iter()
            .any(|d| d.distance_xz(feet) < TIGHT_ZONE_M && (d.y - feet.y).abs() < OTHER_FLOOR_M)
}


/// [#2081] What the own-trap guard made of a tick's keys ([`own_trap_guard`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OwnTrapGuard {
    /// No known trap within contact of the keys' path, or the path leaves it: the keys as they are.
    Clear,
    /// These keys instead keep out of contact.
    Swapped(u32),
    /// None does: the brake against the velocity (or no move key at rest).
    Braked(u32),
}

/// [#2081] The ticks the guard holds the keys before their stop: the lip guard's (`Feel::lip_guard_ticks`, 2).
pub const GUARD_HOLD_TICKS: u32 = 2;

/// [#2081] The feet over the next ticks with `keys` held for `hold` ticks, then let go: the body's velocity blends linearly from
/// `velocity` to the keys' target (`Kinematics::target_velocity` at speed level 1, the fastest a kit moves) over
/// `Kinematics::blend_ticks`, a new target starting a new blend from the velocity then, as the frozen blend does; `turn` once.
/// The ground plane only: no walls, no falls (the lip guard's), so a path may run through a wall: the safe side.
pub fn forecast(kin: &super::kin::Kinematics, feet: Vec3, velocity: [f32; 2], yaw: f32, turn: f32, keys: u32, hold: u32) -> Vec<Vec3> {
    const MOVE: u32 = super::key::FORWARD | super::key::BACK | super::key::LEFT | super::key::RIGHT | super::key::SPRINT;
    let blend = kin.blend_ticks.max(1);
    let mut at = feet;
    let mut points = vec![at];
    let (mut from, mut to, mut step) = (velocity, kin.target_velocity(keys, yaw + turn, 1.0), 0u32);
    for k in 0..blend + hold + 2 {
        if k == hold {
            let now = [from[0] + (to[0] - from[0]) * (step as f32 / blend as f32).min(1.0), from[1] + (to[1] - from[1]) * (step as f32 / blend as f32).min(1.0)];
            (from, to, step) = (now, kin.target_velocity(keys & !MOVE, yaw + turn, 1.0), 0);
        }
        step += 1;
        let t = (step as f32 / blend as f32).min(1.0);
        let v = [from[0] + (to[0] - from[0]) * t, from[1] + (to[1] - from[1]) * t];
        at = Vec3::new(at.x + v[0] * super::kin::TICK_S, at.y, at.z + v[1] * super::kin::TICK_S);
        points.push(at);
    }
    points
}

/// [#2081] A trap the own-trap guard keeps out of: where, and the nearest the body's forecast may come ([`OWN_TRAP_CONTACT_M`] and
/// the margins of [`guard_discs`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GuardDisc {
    pub at: Vec3,
    pub contact: f32,
}

impl From<Vec3> for GuardDisc {
    /// The bare contact.
    fn from(at: Vec3) -> Self {
        Self { at, contact: OWN_TRAP_CONTACT_M }
    }
}

/// [#2081] Whether the path `points` from `feet` comes within its contact of a trap of `discs` while nearer it than `feet` are (a
/// body inside contact may leave). Each leg is tested against the traps on its own floor: a fall's landing on a trap below counts
/// (the bench's tdm s3: a placer walked off a 1.5 m ledge onto its trap), as the route's check reads its legs.
pub fn path_touches(points: &[Vec3], discs: &[GuardDisc], feet: Vec3) -> bool {
    discs.iter().any(|disc| {
        let on = |p: Vec3| (p.y - disc.at.y).abs() < OTHER_FLOOR_M;
        let now = if on(feet) { feet.distance_xz(disc.at) } else { f32::INFINITY };
        let nearest = match points {
            [one] => if on(*one) { one.distance_xz(disc.at) } else { f32::INFINITY },
            _ => points.windows(2).filter(|leg| on(leg[0]) || on(leg[1])).map(|leg| off_route_m(leg, disc.at)).fold(f32::INFINITY, f32::min),
        };
        nearest < disc.contact && nearest < now - 0.01
    })
}

/// [#2081] The margin for the reckoning's own error on a bot's feet by the age of its last world, metres: the drift lab's
/// (`bots::tests::drift_replay::drift_on_a_real_match`, build 8d9d1f606: the server's reckoning of each bot's body from each world
/// against the client's feet at the next) flat error P90, 0.128 m at 250 ms, 0.153 m at 500 ms, 0.230 m at 1,000 ms (12,074 /
/// 17,611 / 16,415 intervals of firing bots), between the rows on a line, held past the ends.
pub fn reckon_margin_m(age_ms: u32) -> f32 {
    const ROWS: [(f32, f32); 3] = [(250.0, 0.128), (500.0, 0.153), (1_000.0, 0.230)];
    let age = age_ms as f32;
    if age <= ROWS[0].0 {
        return ROWS[0].1;
    }
    for w in ROWS.windows(2) {
        let ((a0, m0), (a1, m1)) = (w[0], w[1]);
        if age <= a1 {
            return m0 + (m1 - m0) * (age - a0) / (a1 - a0);
        }
    }
    ROWS[2].1
}

/// [#2081] The margin for a trap no world shows yet, metres: the team's promise is the press's own landing, and the trap lies off it
/// by the bench's promise -> world offsets ([`super::trap_know::landing_offsets`]): their P99, 51 offsets of 8 plain matches
/// (Rudnya TDM and Batteries, 600 s; P90 0.30-0.34 m, P95 0.37-0.42 m, and the P99 of 51 their largest, 0.71 m). The world showed
/// the trap 250-2,000 ms after the release (P90 about 1 s), the window this margin stands for. The team's field shuts its floors
/// that much farther round a promised trap too (`TrapField::shut_promised_within`).
pub const PROMISE_MARGIN_M: f32 = 0.71;

/// [#2081] The traps the guard keeps `bot` out of, with their contact: its team's field (each at the world's armed trap within the
/// promise match `match_m` as soon as a world shows it, [`BotState::world_armed`]; until then at the promise, with
/// [`PROMISE_MARGIN_M`]) and the traps it noticed (at the world's place), all with the reckoning's margin for the age of the bot's
/// last world `age_ms` ([`reckon_margin_m`]); not those in `skip`.
fn guard_discs(bot: &Bot, match_m: f32, age_ms: u32, skip: &[Vec3]) -> Vec<GuardDisc> {
    let reckon = reckon_margin_m(age_ms);
    let world = &bot.features.traps.world_armed;
    let team = bot.features.traps.field.discs().iter().map(|&d| {
        match world.iter().filter(|w| w.distance_xz(d) <= match_m && (w.y - d.y).abs() < OTHER_FLOOR_M).min_by(|a, b| a.distance_xz(d).total_cmp(&b.distance_xz(d))) {
            Some(&w) => GuardDisc { at: w, contact: OWN_TRAP_CONTACT_M + reckon },
            None => GuardDisc { at: d, contact: OWN_TRAP_CONTACT_M + reckon + PROMISE_MARGIN_M },
        }
    });
    let noticed = bot.features.traps.react.field().discs().iter().map(|&d| GuardDisc { at: d, contact: OWN_TRAP_CONTACT_M + reckon });
    team.chain(noticed).filter(|d| !skip.contains(&d.at)).collect()
}

/// [#2081] The own-trap guard at the motor's last stage (after the lip guard, `drive.rs`): the tick's keys `keys` (the view turns
/// `turn`) played forward by `path_of` (the body's stop forecast, `body::stop_path`; [`forecast`] on the ground plane in tests); keys whose path touches a trap of `discs` ([`path_touches`]) give way to the same without
/// sprint, without the side keys, without either, to no move key, or to the brake against the velocity, the first that does not
/// and that `allowed` lets stand (the lip guard's test: dodging a trap never steps a body off a ledge; the brake is that guard's own
/// last resort).
/// Every mover's keys pass it: the route walk, the fight's side step and weave, the step apart, unstick, a retreat's or a dodge's
/// steps, a sprint's carry past a route's end, a goal or hold point inside a disc. A jump (`jumping`: its press tick, its air time) is
/// left alone, its line-up is not: in the air over a trap is legal (the sensor is 0.2 m high), and its landing is a waypoint of the route, held to the
/// clearance by the route's check ([`own_trap_route`]). A push by a mate's body is not keys: not guarded.
#[allow(clippy::too_many_arguments)]
pub fn own_trap_guard(
    path_of: impl Fn(u32) -> Vec<Vec3>,
    allowed: impl Fn(u32) -> bool,
    feet: Vec3,
    velocity: [f32; 2],
    yaw: f32,
    turn: f32,
    keys: u32,
    discs: &[GuardDisc],
    jumping: bool,
) -> OwnTrapGuard {
    use super::key;
    const MOVE: u32 = key::FORWARD | key::BACK | key::LEFT | key::RIGHT | key::SPRINT;
    if jumping || discs.iter().all(|d| d.at.distance_xz(feet) > 8.0) {
        return OwnTrapGuard::Clear;
    }
    let touches = |k: u32| path_touches(&path_of(k), discs, feet);
    if !touches(keys) {
        return OwnTrapGuard::Clear;
    }
    let walk = keys & !key::SPRINT;
    let straight = keys & !(key::LEFT | key::RIGHT);
    if let Some(k) = [walk, straight, straight & !key::SPRINT, keys & !MOVE].into_iter().filter(|&k| k != keys).find(|&k| !touches(k) && allowed(k)) {
        return OwnTrapGuard::Swapped(k);
    }
    let speed = velocity[0].hypot(velocity[1]);
    let brake = if speed > super::drive::LIP_GLIDE_MPS {
        let relative = super::kin::wrap(super::kin::yaw_of([-velocity[0], -velocity[1]]) - (yaw + turn));
        (keys & !MOVE) | super::motor::keys_toward(relative)
    } else {
        keys & !MOVE
    };
    OwnTrapGuard::Braked(brake)
}

/// [#2081] [`own_trap_guard`] on the tick's decision: the team's known traps (its field, with those only it knows) and the traps of
/// others it noticed (`trap_react`, a human mate's among them); a note and a count when it changes the keys.
#[allow(clippy::too_many_arguments)]
pub(super) fn guard_own_traps(
    bot: &mut Bot,
    keys: u32,
    turn: f32,
    me: &PlaceView,
    tuning: &super::tuning::Tuning,
    grid: &WalkGrid,
    allowed: impl Fn(u32) -> bool,
    now: u32,
) -> Option<u32> {
    #[cfg(test)]
    {
        bot.features.traps.guard_seen = Some((now, me.feet));
    }
    let kin = &tuning.kin;
    // A noticed trap its route jumps is left to the jump's run-up (`trap_react::jump_due` presses it 0.3-1.1 m before the trap,
    // where a run's stop forecast already touches it).
    let run_up = super::trap_react::run_up(bot, me, now, &tuning.traps.react);
    let discs = guard_discs(bot, tuning.traps.promise_match.metres(), me.known_at.saturating_sub(me.seen_at), &run_up);
    if discs.is_empty() {
        bot.features.traps.guard_stall = None;
        return None;
    }
    // The press's own tick, or a body in the jump's air time (`body::JUMP_AIR_MS`, the press gate's); a line-up and a press held
    // for its gate walk on the ground and are guarded (the bench's batt s2: a line-up walked a placer onto its trap).
    let airborne = bot.jump_watch.as_ref().is_some_and(|watch| now < watch.at + super::body::JUMP_AIR_MS);
    let jumping = keys & super::key::JUMP != 0 || airborne;
    // The body's own stop forecast, as the lip guard's (its kept controller, at speed level 1: the fastest a kit moves).
    let at = super::body::Estimate { kept: bot.reckoned_kept, ..super::body::Estimate::of_view(me, grid.locate(me.feet, 1.5)) };
    let stop = super::body::Stop { kin, level: 1.0, grid, planned: &[], hold: GUARD_HOLD_TICKS, margin: super::body::RECKONING_M };
    let path_of = |k: u32| super::body::stop_path(&at, k, turn, &stop);
    let guard = own_trap_guard(path_of, allowed, me.feet, me.velocity, me.yaw, turn, keys, &discs, jumping);
    // A route the guard holds at rest (no move key, the body slower than the lip guard's glide) past the longest think gap is one
    // the body cannot walk: it ends, and the noticed traps it ran into are shut for the bot's searches (the team's are already).
    const MOVE: u32 = super::key::FORWARD | super::key::BACK | super::key::LEFT | super::key::RIGHT | super::key::SPRINT;
    let at_rest = matches!(guard, OwnTrapGuard::Swapped(k) | OwnTrapGuard::Braked(k) if k & MOVE == 0) && me.speed() < super::drive::LIP_GLIDE_MPS;
    let fallback = on_fallback(bot);
    #[cfg(test)]
    {
        if at_rest && bot.route.is_none() {
            own_trap_counts::bump(own_trap_counts::HELD_WITHOUT_ROUTE);
        }
        if fallback && !matches!(guard, OwnTrapGuard::Clear) {
            own_trap_counts::bump(own_trap_counts::GUARDED_ON_FALLBACK);
        }
        if fallback && at_rest {
            own_trap_counts::bump(own_trap_counts::HELD_ON_FALLBACK);
        }
    }
    let held = at_rest && bot.route.is_some();
    bot.features.traps.guard_stall = if held { Some(bot.features.traps.guard_stall.unwrap_or(now)) } else { None };
    if let Some(since) = bot.features.traps.guard_stall.filter(|&since| now.saturating_sub(since) >= tuning.think.think_ms.1) {
        let path = path_of(keys);
        let touched: Vec<Vec3> = discs.iter().filter(|d| path_touches(&path, &[**d], me.feet)).map(|d| d.at).collect();
        // A fallback route held by a team trap waits and goes on: the wide search would find no way again (a new loop), and the
        // guard lets the walk on once the body is clear of contact. A noticed trap still ends it (it is shut for the next search).
        let team = bot.features.traps.field.clone();
        if fallback && touched.iter().all(|t| team.discs().contains(t)) {
            bot.features.traps.guard_stall = Some(now);
            #[cfg(test)]
            own_trap_counts::bump(own_trap_counts::FALLBACK_WAITED);
            return match guard {
                OwnTrapGuard::Clear => None,
                OwnTrapGuard::Swapped(k) | OwnTrapGuard::Braked(k) => Some(k),
            };
        }
        let shut = bot.features.traps.react.block(&touched, grid);
        bot.features.traps.guard_stall = None;
        bot.end_route(super::route_end::RouteEnd::TrapAhead, now);
        own_trap_note(bot, now, format!("the guard held the route at rest {} ms: ended, {shut} noticed trap(s) shut", now - since));
    }
    match guard {
        OwnTrapGuard::Clear => None,
        OwnTrapGuard::Swapped(k) | OwnTrapGuard::Braked(k) => {
            own_trap_note(bot, now, format!("the guard changed keys {keys:#x} to {k:#x}: they ran within contact of an own trap"));
            Some(k)
        }
    }
}

/// [#2081] A note of a refusal or a re-route for an own trap, at most one a second a bot (a side step is asked every tick); the
/// bench counts every one.
pub(super) fn own_trap_note(bot: &mut Bot, now: u32, what: String) {
    #[cfg(test)]
    own_trap_counts::count(&what);
    if bot.features.traps.own_trap_noted.is_some_and(|t| now < t.saturating_add(1_000)) {
        return;
    }
    bot.features.traps.own_trap_noted = Some(now);
    bot.features.traps.notes.push((now, format!("{}own trap: {what}", tally::PREFIX)));
}

/// [#2081 FIX 2] The bench's feet off the route within 3 m of a standing own trap, by mover, at the thinks.
#[cfg(test)]
pub mod tight {
    use std::sync::Mutex;

    pub const WALK: usize = 0;
    pub const SPRINT: usize = 1;
    pub const WEAVE: usize = 2;
    pub const SIDE: usize = 3;
    pub const APART: usize = 4;
    pub const NAMES: [&str; 5] = ["walk", "sprint", "weave", "side step", "step apart"];
    /// Faster than the forward walk's 3.5 m/s (`kin.rs`) and short of the sprint's 4.9587: a sprint.
    pub const SPRINT_MPS: f32 = 4.2;
    static OFF: Mutex<[Vec<f32>; 5]> = Mutex::new([const { Vec::new() }; 5]);

    pub fn add(mover: usize, off: f32) {
        if let Ok(mut o) = OFF.lock() {
            o[mover].push(off);
        }
    }

    pub fn line() -> String {
        let Ok(o) = OFF.lock() else { return String::new() };
        NAMES
            .iter()
            .zip(o.iter())
            .map(|(n, v)| {
                let mut v = v.clone();
                v.sort_by(f32::total_cmp);
                let q = |p: f32| v.get(((v.len() as f32 * p) as usize).min(v.len().saturating_sub(1))).copied().unwrap_or(f32::NAN);
                format!("{n} n {} P50 {:.2} P90 {:.2} P95 {:.2} P99 {:.2} max {:.2}", v.len(), q(0.5), q(0.9), q(0.95), q(0.99), v.last().copied().unwrap_or(f32::NAN))
            })
            .collect::<Vec<_>>()
            .join("; ")
    }
}

/// [#2081] The bench's counts of the refusals and re-routes for own traps, by kind.
#[cfg(test)]
pub mod own_trap_counts {
    use std::sync::atomic::{AtomicU64, Ordering};

    pub const NAMES: [&str; 13] = [
        "route refused",
        "route dropped",
        "side step refused",
        "step apart refused",
        "keys guarded",
        "held route ended",
        "noticed route dropped",
        "held ticks without a route",
        "fallback routes",
        "keys guarded on a fallback",
        "held ticks on a fallback",
        "fallback holds waited",
        "tight ticks",
    ];
    pub const GUARDED_ON_FALLBACK: usize = 9;
    pub const HELD_ON_FALLBACK: usize = 10;
    pub const FALLBACK_WAITED: usize = 11;
    pub const TIGHT_TICKS: usize = 12;
    /// [`NAMES`]' indices counted by [`bump`] rather than by a note.
    pub const NOTICED_ROUTE_DROPPED: usize = 6;
    pub const HELD_WITHOUT_ROUTE: usize = 7;
    static COUNTS: [AtomicU64; 13] = [const { AtomicU64::new(0) }; 13];

    pub fn bump(k: usize) {
        COUNTS[k].fetch_add(1, Ordering::Relaxed);
    }

    pub fn count(what: &str) {
        let k = if what.starts_with("a route of") {
            0
        } else if what.starts_with("the route ahead") {
            1
        } else if what.starts_with("a side step") {
            2
        } else if what.starts_with("the guard held") {
            5
        } else if what.starts_with("a fallback route") {
            8
        } else if what.starts_with("the guard") {
            4
        } else {
            3
        };
        COUNTS[k].fetch_add(1, Ordering::Relaxed);
    }

    pub fn line() -> String {
        NAMES.iter().zip(&COUNTS).map(|(n, c)| format!("{n} {}", c.load(Ordering::Relaxed))).collect::<Vec<_>>().join(", ")
    }
}

/// A route that runs over a trap's disc within [`KEEP_OFF_M`] is dropped, to
/// be searched again round it (the searches shut the floors of the discs,
/// [`filter`]).
pub(super) fn keep_off(bot: &mut Bot, due: &Due) {
    if bot.features.traps.field.is_empty() {
        return;
    }
    let field = bot.features.traps.field.clone();
    // [#2081] A fallback route is held to the reach it was searched with ([`FALLBACK_REACH_M`]): keep_off never drops the way
    // the fallback found (the search / keep_off loop of the original catches).
    let fallback = on_fallback(bot);
    let reach = if fallback { FALLBACK_REACH_M } else { field.reach().max(FIELD_M) };
    if route_hits_within(bot, &field, due.me.feet, reach) {
        bot.end_route(RouteEnd::TrapAhead, due.now);
        own_trap_note(bot, due.now, format!("the route ahead dropped: it passes an own trap nearer than {reach:.2} m"));
        return;
    }
    if fallback {
        return;
    }
    // The ring: a route planned before the trap, or before this change of the traps, that runs within it is
    // searched again once (the search keeps the ring out as a preference, `soften`); not again for the same
    // change, so a corridor with no way round is not searched for ever.
    if field.has_ring() && bot.features.traps.ringed != field.stamp() && near_a_disc(&field, due.me.feet) {
        bot.features.traps.ringed = field.stamp();
        if route_within(bot, &field, due.me.feet, field.ring_m() - 0.05) {
            bot.end_route(RouteEnd::TrapAhead, due.now);
        }
    }
}

/// Whether a disc of `field` is within [`KEEP_OFF_M`] of `feet`.
fn near_a_disc(field: &TrapField, feet: Vec3) -> bool {
    field.discs().iter().any(|disc| disc.distance_xz(feet) < KEEP_OFF_M)
}

/// Whether the route of the bot, within [`KEEP_OFF_M`] ahead of `feet`, breaks the clearance from a disc of `field`
/// ([`keeps_clear`], the rule its search's route was checked by).
pub(super) fn route_hits(bot: &Bot, field: &TrapField, feet: Vec3) -> bool {
    route_hits_within(bot, field, feet, field.reach().max(FIELD_M))
}

/// [`route_hits`] at `reach`.
fn route_hits_within(bot: &Bot, field: &TrapField, feet: Vec3, reach: f32) -> bool {
    if !near_a_disc(field, feet) {
        return false;
    }
    let Some((route, _)) = bot.route.as_ref() else {
        return false;
    };
    !route_keeps_clear(&route_points(route, feet, KEEP_OFF_M), field.discs(), feet, Some(route.end()), reach)
}

/// [#2081] The reach of the fallback search's own-trap discs, metres: the guard's contact with its largest reckoning margin,
/// [`OWN_TRAP_CONTACT_M`] + the drift lab's 1,000 ms P90 0.230 m ([`reckon_margin_m`]). A search with no way at the clearance
/// searches once more with the team's own traps shut only this far (the bench's batt s4: a battery carrier 2.24 m from its own
/// trap, the only way out of where it stood within 1.47 m of it, stood 140 s); the motor's guard keeps the body off the trap.
pub const FALLBACK_REACH_M: f32 = OWN_TRAP_CONTACT_M + 0.230;

/// [#2081] The route the fallback search gave, marked by its end and length: the route check and keep_off hold it to
/// [`FALLBACK_REACH_M`]; any other route the bot takes is judged at the clearance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FallbackRoute {
    pub end: Vec3,
    pub length: f32,
}

/// [#2081] Whether the bot walks the route the fallback search gave.
pub(super) fn on_fallback(bot: &Bot) -> bool {
    match (bot.features.traps.fallback, bot.route.as_ref()) {
        (Some(m), Some((route, _))) => route.end().distance_xz(m.end) < 0.01 && (route.end().y - m.end.y).abs() < 0.01 && (route.length() - m.length).abs() < 0.01,
        _ => false,
    }
}

/// [#2081] The team's own traps' field at [`FALLBACK_REACH_M`] for the fallback search, built again only when the field changes.
pub(super) fn fallback_field(bot: &mut Bot, grid: &WalkGrid) -> Arc<TrapField> {
    let field = bot.features.traps.field.clone();
    if let Some((of, narrow)) = &bot.features.traps.fallback_field
        && Arc::ptr_eq(of, &field)
    {
        return narrow.clone();
    }
    let narrow = Arc::new(TrapField::round_within(field.discs().to_vec(), grid, FALLBACK_REACH_M));
    bot.features.traps.fallback_field = Some((field, narrow.clone()));
    narrow
}

/// [#2081] [`filter`] with `field`'s floors as the team's (the fallback search's narrow field).
pub(super) fn filter_within(bot: &Bot, field: &Arc<TrapField>, landings: Vec<super::stuck::Shut>, start: Option<NodeId>) -> RouteFilter {
    filter_of(bot, field, landings, start)
}

/// [#2081] Marks the route the bot just took as the fallback search's, with a note: its length, the nearest own trap from its start
/// and its goal.
pub(super) fn mark_fallback(bot: &mut Bot, now: u32) {
    let Some((route, goal)) = bot.route.as_ref() else {
        bot.features.traps.fallback = None;
        return;
    };
    let (end, length, start, goal) = (route.end(), route.length(), route.point_at(0.0), *goal);
    bot.features.traps.fallback = Some(FallbackRoute { end, length });
    let nearest = |p: Vec3| bot.features.traps.field.discs().iter().map(|d| d.distance_xz(p)).fold(f32::INFINITY, f32::min);
    let (from_start, from_goal) = (nearest(start), nearest(goal));
    own_trap_note(
        bot,
        now,
        format!("a fallback route of {length:.1} m: no way at the clearance; the nearest own trap {from_start:.2} m from its start, {from_goal:.2} m from its goal"),
    );
}

/// Whether the route of the bot runs within `reach` metres of a disc of `field` within [`KEEP_OFF_M`] ahead of
/// `feet`.
fn route_within(bot: &Bot, field: &TrapField, feet: Vec3, reach: f32) -> bool {
    if !near_a_disc(field, feet) {
        return false;
    }
    let Some((route, _)) = bot.route.as_ref() else {
        return false;
    };
    // A copy: the motor's own progress along the route is not touched.
    let mut route = route.clone();
    let (s, _) = route.progress(feet);
    let points: Vec<Vec3> = (0..=KEEP_OFF_M as usize)
        .map(|k| route.point_at((s + k as f32).min(route.length())))
        .collect();
    field.discs().iter().any(|disc| off_route_m(&points, *disc) < reach)
}

/// `filter` with the floors of the traps the bot noticed kept out as a
/// preference (soft): a way round them where that is not much longer, else
/// over them. Not the floor the body stands on (`start`).
pub(super) fn soften(bot: &Bot, filter: RouteFilter, start: Option<NodeId>) -> RouteFilter {
    let noticed = bot.features.traps.react.field().clone();
    let team = bot.features.traps.field.clone();
    if noticed.is_empty() && !team.has_ring() {
        return filter;
    }
    filter.avoiding_floors(move |node| Some(node) == start || (noticed.open(node) && team.ring_open(node)))
}

/// The hard part of a search's filter: the landings it gave up shut, and the
/// floors of the team's traps shut, but for the floor the body stands on
/// (`start`: a trap let go of a metre off must not shut the way out).
pub(super) fn filter(bot: &Bot, landings: Vec<super::stuck::Shut>, start: Option<NodeId>) -> RouteFilter {
    filter_of(bot, &bot.features.traps.field, landings, start)
}

/// [`filter`] with the team's traps of `field`.
fn filter_of(bot: &Bot, field: &Arc<TrapField>, landings: Vec<super::stuck::Shut>, start: Option<NodeId>) -> RouteFilter {
    let filter = RouteFilter::shutting_landings(landings);
    let fresh = bot.features.traps.fresh_shut.clone();
    // [#2081] The noticed traps whose way over the guard held at rest (`React::block`).
    let blocked = bot.features.traps.react.blocked_field().clone();
    if field.is_empty() && fresh.is_empty() && blocked.is_empty() {
        return filter;
    }
    let field = field.clone();
    let inside = bot.features.traps.keep_off_open.clone();
    filter.with_floors(move |node| {
        Some(node) == start
            || ((field.open(node) || inside.binary_search(&node).is_ok()) && fresh.binary_search(&node).is_err() && blocked.open(node))
    })
}

/// [keep_off] At a think, the floors of the wider field the bot may still take: for each trap within `hard_m` of its feet, the shut
/// floors farther from that trap than the feet (+0.1 m), so a placer by its own trap walks out of the disc and nobody inside it is cut
/// off; none without `keep_off`.
fn keep_off_open(bot: &mut Bot, due: &Due, t: &TrapTuning) {
    let Some(k) = t.keep_off else {
        if !bot.features.traps.keep_off_open.is_empty() {
            bot.features.traps.keep_off_open = Default::default();
        }
        return;
    };
    let feet = due.me.feet;
    let grid = &due.tactics.world.grid;
    let mut open: Vec<NodeId> = Vec::new();
    // [#2081] As far as a promised trap's wider shut floors (`PROMISE_MARGIN_M`): a placer by its fresh trap is never shut in.
    let reach = k.hard_m + PROMISE_MARGIN_M;
    for at in bot.features.traps.field.discs().iter().filter(|d| d.distance_xz(feet) < reach) {
        let mine = feet.distance_xz(*at);
        let area = [at.x - reach, at.z - reach, at.x + reach, at.z + reach];
        open.extend(grid.floors_in(area).filter(|&node| {
            let p = grid.position(node);
            let d = p.distance_xz(*at);
            d <= reach && d >= FIELD_M && d > mine + 0.1 && (p.y - at.y).abs() < 1.0
        }));
    }
    open.sort_unstable();
    open.dedup();
    bot.features.traps.keep_off_open = Arc::new(open);
}

/// [A309] At a think, the floors the placer of a fresh trap may not take (`TrapTuning::fresh_ring`): within `ring_m` of the trap and
/// nearer it than its feet are now, so every way it takes leads away from the trap; none once the time is over.
fn fresh_shut(bot: &mut Bot, due: &Due, ring_m: f32) {
    let Some((at, until)) = bot.features.traps.fresh else {
        return;
    };
    if due.now >= until {
        bot.features.traps.fresh = None;
        bot.features.traps.fresh_shut = Default::default();
        return;
    }
    let grid = &due.tactics.world.grid;
    let reach = ring_m.max(FIELD_M);
    let mine = due.me.feet.distance_xz(at);
    let area = [at.x - reach, at.z - reach, at.x + reach, at.z + reach];
    let mut shut: Vec<NodeId> = grid
        .floors_in(area)
        .filter(|&node| {
            let p = grid.position(node);
            let d = p.distance_xz(at);
            d <= reach && d < mine - 0.05 && (p.y - at.y).abs() < 1.0
        })
        .collect();
    shut.sort_unstable();
    bot.features.traps.fresh_shut = std::sync::Arc::new(shut);
}

/// [#2081] Whether a standing trap of the team is missing from `bot`'s field and the bot is its placer or sees its placer's body now
/// (`aimpoint::body_shows`, the sense's body parts from its eye).
fn knows_a_new_trap(bot: &Bot, due: &Due, state: &TeamState) -> bool {
    let mine = bot.features.traps.field.discs();
    state.knowledge.traps.standing().filter(|k| !mine.iter().any(|d| d.distance_xz(k.at) < 0.05)).any(|k| {
        k.by == bot.place
            || due
                .view
                .place(k.by)
                .is_some_and(|p| p.alive && super::aimpoint::body_shows(&due.tactics.world.sight, due.me.eye(), p.feet, p.crouched()))
    })
}

/// A think: whether this bot starts to put a trap down.
pub(super) fn think(bot: &mut Bot, due: &Due, state: &mut TeamState, notes: &mut Vec<super::Note>) {
    let t = &due.tuning.traps;
    let me = due.me;
    // [A292] The notes of its ticks since the last think (presses, looks ended), at their own times.
    for (time, text) in std::mem::take(&mut bot.features.traps.notes) {
        bot.note(notes, time, text);
    }
    // Its trap let go of in the ticks since: promised to the team at once,
    // the field built again, the bot keeping off it from this think on.
    if let Some((at, released)) = bot.features.traps.placed.take() {
        state.knowledge.traps.promise(bot.place, at, released);
        state.refresh_traps(&due.tactics.world.grid, t.ring_m, t.keep_off);
    }
    // [#2081] A trap the team's field has and this bot's does not: taken at this think when the bot is its placer or sees its
    // placer now (the game draws an armed trap for every team, 0x247EF0; a mate who watches the placer sees it put down); every
    // other bot takes the team's field at the commander tick.
    if knows_a_new_trap(bot, due, state) {
        take_fields(bot, state, &due.tactics.world.grid, t.ring_m, t.keep_off);
    }
    fresh_shut(bot, due, t.ring_m);
    keep_off_open(bot, due, t);
    #[cfg(test)]
    let pre = passes::route_near(bot, due, state);
    keep_off(bot, due);
    #[cfg(test)]
    passes::observe(bot, due, state, pre);
    #[cfg(test)]
    if !matches!(bot.features.traps.habit.state(), State::Idle)
        && let Some(c) = bot.features.traps.look_ctx.as_mut()
    {
        c.truth_last_m = placing::truth(due.view, me.team, me.feet);
    }
    if let Err(refusal) = bot.features.traps.habit.may_plan(t) {
        bot.features.traps.tally.may_not(refusal);
        return;
    }
    bot.features.traps.tally.think();
    // [A324] A bot the team sent to a camper's point (`camper_job`): no trap elsewhere on its way; at the point, the job's gate (an
    // enemy seen, not one known near: the camper himself is known there) and no chance roll.
    let job = t.camper_job.and_then(|j| camper_point(&state.knowledge, bot.place).map(|c| (c.at, j)));
    // [bot traps T1] A bot not sent whose walk passes an open camper point of the team (its place held, or kept past the hold):
    // the point is its wished spot. The trap there is the team's: no skill share and no chance roll; [D1] a task play does not
    // refuse it (only a trap on the point then), an enemy within `PASS_ENEMY_M` does.
    let passing = if job.is_none() { passed_points(bot, me, t, &state.knowledge) } else { Vec::new() };
    let task_waived = !passing.is_empty() && bot.assignment.as_ref().is_some_and(|a| urgent(a.kind));
    #[cfg(test)]
    if !passing.is_empty() {
        camper::count(me.team, camper::PASS_WISHED);
    }
    // [fix-team 5 T8 `skill.tactics.traps`] Traps on the enemy's lanes only on the lives its skill gives them; the camper job's
    // trap is the team's (not scaled).
    if job.is_none()
        && passing.is_empty()
        && let Some(tactics) = due.tuning.skill.tactics
        && !crate::v026g::bots::signals::life_uses(
            bot.life_key,
            crate::v026g::bots::tuning::Tactic::Traps,
            tactics.share(crate::v026g::bots::tuning::Tactic::Traps, bot.skill),
        )
    {
        bot.features.traps.tally.may_not(Refusal::Skill);
        return;
    }
    if let Some((at, j)) = job
        && me.feet.distance_xz(at) > j.within_m + t.reach_m + 1.0
    {
        #[cfg(test)]
        camper::count(me.team, camper::THINK_WALKING);
        return;
    }
    // A job's play whose job is over (placed, let go, its time out): no trap on the rest of its way.
    if job.is_none() && bot.assignment.as_ref().is_some_and(|a| a.kind == PlayKind::TrapSpot) {
        #[cfg(test)]
        camper::count(me.team, camper::THINK_JOB_OVER);
        return;
    }
    #[cfg(test)]
    let waiting = bot.features.traps.pass_wait.is_some();
    let facts = Facts {
        now: due.now,
        play: bot.assignment.as_ref().map(|a| a.kind).filter(|_| !task_waived),
        carrying: me.carrying.is_some(),
        health: super::pools::own_health(bot.features.stock.pools, me.health, due.tuning.heal.lethal_health),
        enemy: if job.is_some() {
            !bot.memory.all_seen(due.now, due.view.clock).is_empty()
        } else if !passing.is_empty() {
            enemy_within(bot, me.feet, due.now, due.view.clock, PASS_ENEMY_M)
        } else {
            enemy_near(bot, due)
        },
        last_hit: bot.fight_hit_ms,
        reloading: bot
            .reflex
            .reload_note()
            .is_some_and(|reload| due.now < reload.until),
        jump_ahead: false,
        team_standing: state.knowledge.traps.standing_count(),
    };
    // The cheap gate first; the route is read only once it passes.
    #[cfg(test)]
    gates::count(gates::THINKS);
    if let Err(refusal) = gate(&facts, t) {
        let near = (refusal == Refusal::Enemy).then(|| enemy_near_kind(bot, due)).flatten();
        bot.features.traps.tally.refused(refusal, near);
        // [A324] A sent bot's "enemy seen" at its point: whom it saw (its camper among them, or only others), from what pose,
        // the nearest how far; summed on its job for the job's note, and counted.
        if refusal == Refusal::Enemy
            && job.is_some()
            && let Some(point) = state.knowledge.camper_traps.iter_mut().find(
                |c| matches!(c.state, super::know::CamperTrapState::Sent { placer, .. } if placer == bot.place),
            )
        {
            let seen = bot.memory.all_seen(due.now, due.view.clock);
            let camper = seen.iter().any(|s| s.track().who == point.shooter);
            let nearest = seen.iter().map(|s| s.track().at.distance_xz(me.feet)).fold(f32::INFINITY, f32::min);
            point.seen.add(camper, me.crouched(), nearest);
            #[cfg(test)]
            camper::seen(me.team, camper, me.crouched(), nearest);
        }
        #[cfg(test)]
        {
            gates::count(gates::of(refusal));
            if waiting {
                onpass::count(me.team, onpass::LOST_GATE);
            }
            if job.is_some() {
                camper::count(me.team, camper::THINK_GATE);
                camper::gate(me.team, gates::of(refusal) - gates::of(Refusal::Play(PlayKind::Collect)));
            }
        }
        return;
    }
    if job.is_none() && passing.is_empty() && bot.rng.unit() >= t.chance {
        bot.features.traps.tally.chance();
        #[cfg(test)]
        {
            gates::count(gates::CHANCE);
            if waiting {
                onpass::count(me.team, onpass::LOST_CHANCE);
            }
        }
        return;
    }
    let (ahead, jump) = route_ahead(bot, me, t);
    if jump {
        bot.features.traps.tally.jump();
        #[cfg(test)]
        {
            gates::count(gates::JUMP);
            if waiting {
                onpass::count(me.team, onpass::LOST_JUMP);
            }
        }
        return;
    }
    let mates: Vec<(Vec3, [f32; 2])> = due
        .view
        .team(me.team)
        .filter(|p| p.place != me.place)
        .map(|p| (p.feet, p.velocity))
        .collect();
    let staying = is_staying(bot, me);
    // [A292 S4c] A bot that stays puts none down with `stayer: never`.
    if staying && t.stayer == Stayer::Never {
        bot.features.traps.tally.stayer();
        return;
    }
    let in_spawn = |at: Vec3| due.tactics.in_spawn_area(at, None);
    // Where the head ray will meet the floor at the look it presses with, from the eye of the pose it has now
    // (a crouched holder lands 0.3 m nearer than a walker): none, and no trap, where no look reaches that far.
    let crouched = bot.intent.crouch;
    let press = t.press_landing_m.map(|min_m| (crouched, wanted_m(t, min_m)));
    let reach_m = match press {
        None => t.reach_m,
        Some((crouched, wanted)) => match press_look(crouched, wanted, 0.0) {
            Some(look) => look.run_m,
            None => {
                bot.features.traps.tally.no_look();
                #[cfg(test)]
                {
                    gates::count(gates::NO_LOOK);
                    if job.is_some() {
                        camper::count(me.team, camper::THINK_NO_LOOK);
                    }
                }
                return;
            }
        },
    };
    let own: Vec<Vec3> = state
        .knowledge
        .traps
        .standing()
        .map(|trap| trap.at)
        .collect();
    // [A324] A sent bot's point is the one wished spot (the calm habit wishes none: a camper's place is the job's); [bot traps
    // T1] a passing bot's, the open points its walk passes.
    let job_spots: Vec<Vec3> = if job.is_some() { job.map(|(at, _)| at).into_iter().collect() } else { passing.clone() };
    let wish = job
        .map(|(_, j)| j)
        .or_else(|| t.camper_job.filter(|_| !passing.is_empty()))
        .map(|j| Wish { spots: &job_spots, within_m: j.within_m, friend_m: j.friend_m });
    let search = Search {
        t,
        reach_m,
        press,
        grid: &due.tactics.world.grid,
        now: due.now,
        view_yaw: me.yaw,
        feet: me.feet,
        staying,
        ahead: &ahead,
        mates: &mates,
        lanes: lanes_of(t, state),
        own: &own,
        in_spawn: &in_spawn,
        wish,
        homes: t.forward.zip(homes_of(&due.tactics.world.map, bot.side)).map(|(f, (own, enemy))| (own, enemy, f.min_share)),
    };
    let mut refused = [0; 9];
    let mut chosen = choose_counted(&search, &mut refused);
    // [A309] No spot far enough forward: the next best past the lower share, rather than none.
    if chosen.is_none()
        && let (Some(f), Some((own, enemy, _))) = (t.forward, search.homes)
        && f.fallback_share < f.min_share
    {
        chosen = choose_counted(&Search { homes: Some((own, enemy, f.fallback_share)), ..search }, &mut [0; 9]);
    }
    bot.features.traps.tally.searched(&refused, chosen.is_some());
    // [A145] A spot of a killed holder ahead on the route that no candidate here is on yet: wait for the body to reach it.
    if let Some(wish) = wish {
        #[cfg(test)]
        defeated::count_team(me.team, defeated::THINKS);
        if !chosen.is_some_and(|c| c.wished) && wish.spots.iter().any(|at| me.feet.distance_xz(*at) > wish.within_m + reach_m + 1.0) {
            #[cfg(test)]
            defeated::count_team(me.team, defeated::WAITED);
            bot.features.traps.tally.waited_wish();
            #[cfg(test)]
            if job.is_some() {
                camper::count(me.team, camper::THINK_WAITED);
            }
            return;
        }
    }
    let wished = chosen.is_some_and(|c| c.wished);
    #[cfg(test)]
    if waiting {
        if chosen.is_none() {
            onpass::count(me.team, onpass::LOST_NO_SPOT);
        }
        if bot.features.traps.pass_wait.is_some_and(|(_, since)| due.now.saturating_sub(since) >= PASS_WAIT_MS) {
            onpass::count(me.team, onpass::LOST_STALE);
        }
    }
    let lanes = lanes_of(t, state);
    let here = chosen.map(|c| presence(lanes, c.spot, due.now));
    // Place on the pass: a cell ahead on the route that is richer in enemy lanes than this spot is waited for.
    if let Some(pass) = t.on_the_pass.filter(|_| !staying && !wished) {
        let last = (pass.route_ahead_m as usize).min(ahead.len().saturating_sub(1));
        let cells: Vec<(usize, f32)> = (PASS_FIRST_M..=last)
            .step_by(PASS_STRIDE_M)
            .map(|k| (k, presence(lanes, ahead[k], due.now)))
            .collect();
        #[cfg(test)]
        onpass::count(me.team, onpass::WEIGHED);
        if let Pass::Wait(i) = on_the_pass(here, &cells.iter().map(|c| c.1).collect::<Vec<_>>(), pass.gain) {
            #[cfg(test)]
            {
                onpass::count(me.team, onpass::WAIT);
                if waiting {
                    onpass::count(me.team, onpass::WAITED_AGAIN);
                }
            }
            bot.features.traps.pass_wait = Some((ahead[cells[i].0], due.now));
            bot.features.traps.tally.waited_pass();
            return;
        }
    }
    #[cfg(test)]
    gates::count(if chosen.is_some() { gates::PLANNED } else { gates::NO_SPOT });
    #[cfg(test)]
    if job.is_some() {
        camper::count(
            me.team,
            match chosen {
                Some(c) if c.wished => camper::THINK_PLANNED,
                Some(_) => camper::THINK_PLANNED_ELSE,
                None => camper::THINK_NO_SPOT,
            },
        );
    }
    // [A324] A sent bot puts its trap only by its point; [bot traps T1] so does a passing bot on a task play.
    if let Some(chosen) = chosen.filter(|c| (job.is_none() && !task_waived) || c.wished) {
        #[cfg(test)]
        if chosen.wished {
            defeated::count_team(me.team, defeated::PLACED);
            let point = camper_point(&state.knowledge, bot.place).or_else(|| {
                state.knowledge.camper_traps.iter().filter(|c| passing.contains(&c.at)).min_by(|a, b| {
                    a.at.distance_xz(chosen.spot).total_cmp(&b.at.distance_xz(chosen.spot))
                })
            });
            if let Some(point) = point {
                camper::planned(bot.place, due.now, point.shooter);
            }
            if job.is_none() {
                camper::count(me.team, camper::PASS_PLANNED);
            }
        }
        #[cfg(test)]
        {
            let on_pass = bot
                .features
                .traps
                .pass_wait
                .is_some_and(|(at, since)| due.now.saturating_sub(since) < PASS_WAIT_MS && at.distance_xz(chosen.spot) < PASS_NEAR_M);
            onpass::placed(me.team, on_pass, here.unwrap_or(0.0));
        }
        let on_pass = bot.features.traps.pass_wait.is_some();
        bot.features.traps.pass_wait = None;
        bot.features.traps.habit.plan(chosen.yaw, chosen.spot, due.now);
        bot.features.traps.tally.planned();
        #[cfg(test)]
        {
            let truth = placing::truth(due.view, me.team, me.feet);
            let track = bot
                .memory
                .tracks()
                .iter()
                .map(|k| (k.at.distance_xz(me.feet), due.now.saturating_sub(k.time), k.who))
                .min_by(|a, b| a.0.total_cmp(&b.0));
            bot.features.traps.look_ctx = Some(placing::Ctx {
                place: bot.place,
                team: me.team,
                planned: due.now,
                last: due.now,
                truth_plan_m: truth,
                truth_last_m: truth,
                track,
                heard: None,
                // [halt probe] (a) The same choice from where the feet will be when the view has turned onto the chosen look.
                predicted: {
                    let s = probe::turn_in_s(turn(me.yaw, chosen.yaw).abs(), &due.tuning.feel, bot.skill, t.tolerance_rad);
                    let feet = Vec3::new(me.feet.x + me.velocity[0] * s, me.feet.y, me.feet.z + me.velocity[1] * s);
                    Some((choose(&Search { feet, ..search }).is_some(), (s * 1000.0) as u32))
                },
                halt: None,
                turned: None,
                halted_ms: 0,
            });
        }
        // [A309] The bench's record of the plan for its press line.
        #[cfg(test)]
        {
            let source = if chosen.wished {
                "camper"
            } else if on_pass {
                "pass"
            } else {
                "lane"
            };
            let homes: Vec<Vec3> = due.tactics.world.map.spawns.iter().filter(|s| Some(s.team) == bot.side).map(|s| s.position).collect();
            let (way, to_home) = if homes.is_empty() {
                ("unknown", 0.0)
            } else {
                let n = homes.len() as f32;
                let home = Vec3::new(homes.iter().map(|h| h.x).sum::<f32>() / n, me.feet.y, homes.iter().map(|h| h.z).sum::<f32>() / n);
                let (dx, dz) = (home.x - me.feet.x, home.z - me.feet.z);
                let d = dx.hypot(dz).max(1e-3);
                let v = me.velocity;
                let speed = v[0].hypot(v[1]);
                let way = if speed < 0.5 {
                    "still"
                } else {
                    match (v[0] * dx + v[1] * dz) / (speed * d) {
                        c if c > 0.5 => "home",
                        c if c < -0.5 => "out",
                        _ => "across",
                    }
                };
                (way, d)
            };
            let share = homes_of(&due.tactics.world.map, bot.side).map_or(-1.0, |(own, enemy)| forward_share(own, enemy, chosen.spot));
            bot.features.traps.planned_by = Some((source, way, to_home, share));
        }
        let play = bot.assignment.as_ref().map(|a| a.kind);
        let near = nearest_lane(lanes_of(t, state), chosen.spot);
        bot.note(
            notes,
            due.now,
            format!(
                "{}plans a trap at ({:.1}, {:.1}, {:.1}) in play {play:?}: score {:.2}, enemy lane presence {:.2}{}{}{}",
                tally::PREFIX,
                chosen.spot.x,
                chosen.spot.y,
                chosen.spot.z,
                chosen.score,
                here.unwrap_or(0.0),
                match near {
                    Some((who, m)) => format!(" (nearest lane cell of player {who}, {m:.1} m)"),
                    None => String::new(),
                },
                if chosen.wished { ", on its camper job's point" } else { "" },
                if on_pass { ", after a wait on its route" } else if staying { ", standing" } else { ", walking" },
            ),
        );
        if bot.tracing {
            bot.traced.push(serde_json::json!({
                "t": due.now, "bot": bot.place, "trap_plan": chosen.spot.to_array(),
                "score": chosen.score, "defeated": chosen.wished,
            }));
        }
    }
}

/// The first cell of the route ahead the pass rule looks at, and the metres between the cells it looks at.
const PASS_FIRST_M: usize = 3;
const PASS_STRIDE_M: usize = 2;
/// A wait for a richer cell is good for this long, ms, and the spot placed counts as on the pass within this of it, metres.
#[cfg(test)]
const PASS_WAIT_MS: u32 = 20_000;
#[cfg(test)]
const PASS_NEAR_M: f32 = 4.0;

/// What the pass rule says at a think.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pass {
    /// Place the spot at the feet (or none, if there is none).
    Here,
    /// Wait: the cell `.0` of the cells looked at is richer.
    Wait(usize),
}

/// The pass rule (`TrapTuning::on_the_pass`): `here` the lane presence of the spot at the feet (none: no spot), `ahead`
/// that of the cells along the route; wait for the richest cell ahead when it exceeds `here` by `gain` (and, with no spot
/// at the feet, when it is above `gain` itself).
pub fn on_the_pass(here: Option<f32>, ahead: &[f32], gain: f32) -> Pass {
    let Some((i, best)) = ahead
        .iter()
        .copied()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(&b.1))
    else {
        return Pass::Here;
    };
    if best > here.unwrap_or(0.0) + gain {
        Pass::Wait(i)
    } else {
        Pass::Here
    }
}

/// The bench's counts of the pass rule, by team (A169): thinks that looked along the route, waits, traps placed on the
/// pass and at the feet, and the lane presence of the cells they were placed at (hundredths, summed).
#[cfg(test)]
pub mod onpass {
    use std::sync::atomic::{AtomicU64, Ordering};

    pub const WEIGHED: usize = 0;
    pub const WAIT: usize = 1;
    pub const ON_PASS: usize = 2;
    pub const AT_FEET: usize = 3;
    pub const PRESENCE_ON_PASS: usize = 4;
    pub const PRESENCE_AT_FEET: usize = 5;
    /// A bot that was waiting at the next think: refused by the gate (a play, an enemy, a hit, ...), missed the chance,
    /// a jump ahead, no look, no spot there, or waited again.
    pub const LOST_GATE: usize = 6;
    pub const LOST_CHANCE: usize = 7;
    pub const LOST_JUMP: usize = 8;
    pub const LOST_NO_SPOT: usize = 9;
    pub const WAITED_AGAIN: usize = 10;
    pub const LOST_STALE: usize = 11;
    static COUNTS: [[AtomicU64; 12]; 2] = [const { [const { AtomicU64::new(0) }; 12] }; 2];

    pub fn count(team: u8, kind: usize) {
        COUNTS[usize::from(team & 1)][kind].fetch_add(1, Ordering::Relaxed);
    }

    pub fn placed(team: u8, on_pass: bool, presence: f32) {
        let (n, sum) = if on_pass { (ON_PASS, PRESENCE_ON_PASS) } else { (AT_FEET, PRESENCE_AT_FEET) };
        count(team, n);
        COUNTS[usize::from(team & 1)][sum].fetch_add((presence * 100.0) as u64, Ordering::Relaxed);
    }

    pub fn line() -> String {
        (0..2)
            .map(|team| {
                let c = |k: usize| COUNTS[team][k].load(Ordering::Relaxed);
                let mean = |sum: usize, n: usize| if c(n) == 0 { 0.0 } else { c(sum) as f64 / 100.0 / c(n) as f64 };
                format!(
                    "team {team}: weighed {}, waited {}, placed on the pass {} (lane presence {:.2}), at the feet {} (lane presence {:.2}); a waiting bot's next think: gate {}, chance {}, jump {}, no spot {}, waited again {}, stale {}",
                    c(WEIGHED),
                    c(WAIT),
                    c(ON_PASS),
                    mean(PRESENCE_ON_PASS, ON_PASS),
                    c(AT_FEET),
                    mean(PRESENCE_AT_FEET, AT_FEET),
                    c(LOST_GATE),
                    c(LOST_CHANCE),
                    c(LOST_JUMP),
                    c(LOST_NO_SPOT),
                    c(WAITED_AGAIN),
                    c(LOST_STALE)
                )
            })
            .collect::<Vec<_>>()
            .join("; ")
    }
}

/// The bench's counts of the traps on the spots of killed holders (A145): the records the winning team made (a held threat spot,
/// a camp), thinks with such a spot on the carrier's route, those that waited for the body to reach it, traps placed on one; and,
/// of the candidates within reach of a wished spot, what refused them.
#[cfg(test)]
pub mod defeated {
    use std::sync::atomic::{AtomicU64, Ordering};

    pub const THINKS: usize = 0;
    pub const WAITED: usize = 1;
    pub const PLACED: usize = 2;
    static TEAM: [[AtomicU64; 3]; 2] = [const { [const { AtomicU64::new(0) }; 3] }; 2];

    pub const CANDIDATES: usize = 0;
    pub const R_FLOOR: usize = 1;
    pub const R_LOOK: usize = 2;
    pub const R_SPAWN: usize = 3;
    pub const R_OWN: usize = 4;
    pub const R_ROUTE: usize = 5;
    pub const R_BODY: usize = 6;
    pub const R_FRIEND: usize = 7;
    static CANDIDATE: [AtomicU64; 8] = [const { AtomicU64::new(0) }; 8];

    pub fn count_team(team: u8, kind: usize) {
        TEAM[usize::from(team & 1)][kind].fetch_add(1, Ordering::Relaxed);
    }

    pub fn count(kind: usize) {
        CANDIDATE[kind].fetch_add(1, Ordering::Relaxed);
    }

    pub fn line() -> String {
        let team = (0..2)
            .map(|team| {
                let c = |k: usize| TEAM[team][k].load(Ordering::Relaxed);
                format!(
                    "team {team}: thinks with a wished spot {}, waited {}, trap planned on one {}",
                    c(THINKS),
                    c(WAITED),
                    c(PLACED)
                )
            })
            .collect::<Vec<_>>()
            .join("; ");
        let n = |k: usize| CANDIDATE[k].load(Ordering::Relaxed);
        format!(
            "{team}; candidates at a wished spot {}: refused by floor {}, no look {}, spawn {}, own trap {}, own route {}, own body {}, a mate near {}",
            n(CANDIDATES),
            n(R_FLOOR),
            n(R_LOOK),
            n(R_SPAWN),
            n(R_OWN),
            n(R_ROUTE),
            n(R_BODY),
            n(R_FRIEND)
        )
    }
}

/// The bench's count of how bodies come to the team's standing traps (A165): each time a body of the team comes within
/// 3 m of a standing trap of its team's, its class (the route it walks: none / clear of the discs / over one; whether its
/// field has this trap; whether the route's goal lies in the trap's disc), seen and, when the trap then went off under it,
/// caught. Decisions are not touched.
#[cfg(test)]
pub mod passes {
    use std::sync::atomic::{AtomicU64, Ordering};

    use mapdata::Vec3;

    use super::{Bot, Due, FIELD_M, TeamState, route_hits};

    /// route (none, clear, over a disc) x field has the trap x goal in the disc.
    pub const CLASSES: usize = 12;
    /// Of the placer (1) or of a mate (0).
    static SEEN: [[AtomicU64; CLASSES]; 2] = [const { [const { AtomicU64::new(0) }; CLASSES] }; 2];
    static CAUGHT: [[AtomicU64; CLASSES]; 2] = [const { [const { AtomicU64::new(0) }; CLASSES] }; 2];
    static UNSEEN: AtomicU64 = AtomicU64::new(0);
    /// The route's ground clearance from the disc's centre at the first sight (<0.8, <1.0, <1.3, <2.0, more, no
    /// route), mates only: seen and caught; and the body's distance from its route at the catch (<0.2, <0.4, <0.7,
    /// <1.2, more, no route).
    static CLEAR_SEEN: [AtomicU64; 6] = [const { AtomicU64::new(0) }; 6];
    static CLEAR_CAUGHT: [AtomicU64; 6] = [const { AtomicU64::new(0) }; 6];
    static DEVIATION: [AtomicU64; 6] = [const { AtomicU64::new(0) }; 6];

    fn bin(value: f32, edges: [f32; 4]) -> usize {
        edges.iter().position(|&e| value < e).unwrap_or(4)
    }

    fn clearance_bin(bot: &Bot, feet: Vec3, at: Vec3) -> usize {
        let Some((route, _)) = bot.route.as_ref() else {
            return 5;
        };
        let mut route = route.clone();
        let (s, _) = route.progress(feet);
        let points: Vec<Vec3> = (0..=15).map(|k| route.point_at((s + k as f32).min(route.length()))).collect();
        bin(super::off_route_m(&points, at), [0.8, 1.0, 1.3, 2.0])
    }

    pub fn class(route: usize, field_has: bool, goal_in: bool) -> usize {
        route * 4 + usize::from(field_has) * 2 + usize::from(goal_in)
    }

    /// [#2081 v6 measure] Before the keep-off check: whether it has a route, and the route ahead's nearest approach to the nearest
    /// standing own trap within 2 m of the feet.
    pub(in crate::v026g::bots) fn route_near(bot: &Bot, due: &Due, state: &TeamState) -> (bool, Option<f32>) {
        let feet = due.me.feet;
        let trap = state.knowledge.traps.standing().map(|k| k.at).filter(|at| at.distance_xz(feet) < 2.0).min_by(|a, b| a.distance_xz(feet).total_cmp(&b.distance_xz(feet)));
        let near = trap.and_then(|at| {
            let (route, _) = bot.route.as_ref()?;
            let mut route = route.clone();
            let (s, _) = route.progress(feet);
            let points: Vec<Vec3> = (0..=15).map(|k| route.point_at((s + k as f32).min(route.length()))).collect();
            Some(super::off_route_m(&points, at))
        });
        (bot.route.is_some(), near)
    }

    pub(in crate::v026g::bots) fn observe(bot: &mut Bot, due: &Due, state: &TeamState, pre: (bool, Option<f32>)) {
        let feet = due.me.feet;
        // [#2081 FIX 2] Within 3 m of a standing own trap, with a route: the feet off it, by mover.
        if let Some((route, _)) = bot.route.as_ref()
            && state.knowledge.traps.standing().any(|k| k.at.distance_xz(feet) < 3.0)
        {
            let mut route = route.clone();
            let off = route.progress(feet).1;
            if off.is_finite() {
                let mover = if bot.bench_keys_from == 2 {
                    super::tight::APART
                } else if bot.bench_stepped == 2 {
                    super::tight::WEAVE
                } else if bot.bench_stepped == 1 {
                    super::tight::SIDE
                } else if due.me.speed() > super::tight::SPRINT_MPS {
                    super::tight::SPRINT
                } else {
                    super::tight::WALK
                };
                super::tight::add(mover, off);
            }
        }
        // [#2081 v6 measure] What moved its legs while within 2 m of a standing own trap (the own-catch line's).
        if let Some(d) = state.knowledge.traps.standing().map(|k| k.at.distance_xz(feet)).filter(|&d| d < 2.0).min_by(f32::total_cmp) {
            let m = super::NearMove {
                t: due.now,
                d,
                keys_from: bot.bench_keys_from,
                apart: bot.bench_apart,
                stepped: bot.bench_stepped,
                route: bot.route.is_some(),
                stay: matches!(bot.intent.go, crate::v026g::bots::plays::Go::Stay),
                hold: bot.intent.hold != 0,
                speed: due.me.speed(),
                route_m: pre.1,
                ended: pre.0 && bot.route.is_none(),
            };
            let moves = &mut bot.features.traps.near_moves;
            if moves.len() == 6 {
                moves.remove(0);
            }
            moves.push(m);
        }
        let nearest = state
            .knowledge
            .traps
            .standing()
            .map(|trap| (trap, trap.at.distance_xz(feet)))
            .filter(|&(_, d)| d < 3.0)
            .min_by(|a, b| a.1.total_cmp(&b.1));
        let Some((trap, _)) = nearest else {
            if bot
                .features
                .traps
                .near
                .is_some_and(|(at, _, _)| at.distance_xz(feet) > 4.0)
            {
                bot.features.traps.near = None;
            }
            return;
        };
        if bot
            .features
            .traps
            .near
            .is_some_and(|(at, _, _)| at.distance_xz(trap.at) < 0.3)
        {
            return;
        }
        let field = bot.features.traps.field.clone();
        let field_has = field.discs().iter().any(|d| d.distance_xz(trap.at) < 0.3);
        let (route, goal_in) = match bot.route.as_ref() {
            None => (0, false),
            Some((_, goal)) => (
                if route_hits(bot, &field, feet) { 2 } else { 1 },
                goal.distance_xz(trap.at) < FIELD_M,
            ),
        };
        let class = class(route, field_has, goal_in);
        let clear = clearance_bin(bot, feet, trap.at);
        bot.features.traps.near = Some((trap.at, class, clear));
        SEEN[usize::from(trap.by == bot.place)][class].fetch_add(1, Ordering::Relaxed);
        if trap.by != bot.place {
            CLEAR_SEEN[clear].fetch_add(1, Ordering::Relaxed);
        }
    }

    /// A trap of the team at `at` went off under `victim`'s feet: its class when it had been seen.
    pub fn caught(bot: &Bot, at: Vec3, placer: bool, feet: Vec3) {
        match bot.features.traps.near {
            Some((trap, class, clear)) if trap.distance_xz(at) < 1.0 => {
                CAUGHT[usize::from(placer)][class].fetch_add(1, Ordering::Relaxed);
                if !placer {
                    CLEAR_CAUGHT[clear].fetch_add(1, Ordering::Relaxed);
                    let deviation = bot.route.as_ref().map_or(5, |(route, _)| {
                        let mut route = route.clone();
                        bin(route.progress(feet).1, [0.2, 0.4, 0.7, 1.2])
                    });
                    DEVIATION[deviation].fetch_add(1, Ordering::Relaxed);
                }
            }
            _ => {
                UNSEEN.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    pub fn line() -> String {
        let row6 = |counts: &[AtomicU64; 6]| {
            counts.iter().map(|c| c.load(Ordering::Relaxed).to_string()).collect::<Vec<_>>().join(",")
        };
        let row = |counts: &[AtomicU64; CLASSES]| {
            counts
                .iter()
                .map(|c| c.load(Ordering::Relaxed).to_string())
                .collect::<Vec<_>>()
                .join(",")
        };
        format!(
            "classes [route none/clear/over x field-has-trap no/yes x goal-in-disc no/yes]; mates seen [{}] caught [{}]; placer seen [{}] caught [{}]; caught unseen {}; mates' route clearance (<0.8,<1.0,<1.3,<2.0,more,no route) seen [{}] caught [{}]; body off its route at the catch (<0.2,<0.4,<0.7,<1.2,more,no route) [{}]",
            row(&SEEN[0]),
            row(&CAUGHT[0]),
            row(&SEEN[1]),
            row(&CAUGHT[1]),
            UNSEEN.load(Ordering::Relaxed),
            row6(&CLEAR_SEEN),
            row6(&CLEAR_CAUGHT),
            row6(&DEVIATION)
        )
    }
}

/// The bench's counts of what the habit's thinks came to (the places it was refused at, the chance missed, a jump
/// ahead, no spot, a spot planned), for the `trap habit` line.
#[cfg(test)]
pub mod gates {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::Refusal;

    pub const THINKS: usize = 0;
    pub const CHANCE: usize = 1;
    pub const JUMP: usize = 2;
    pub const NO_SPOT: usize = 3;
    pub const PLANNED: usize = 4;
    const FIRST_REFUSAL: usize = 5;
    /// The names of the counts, in order.
    pub const R_FLOOR: usize = 13;
    pub const R_SPAWN: usize = 14;
    pub const R_OWN: usize = 15;
    pub const R_ROUTE: usize = 16;
    pub const R_BODY: usize = 17;
    pub const R_FRIEND: usize = 18;
    pub const R_SCORE: usize = 19;
    /// No look of the body's pose reaches the floor far enough from the feet (`press_landing_m`).
    pub const NO_LOOK: usize = 20;
    /// A tick the look was on but a trap let go of would have landed too near the feet or on the route.
    pub const PRESS_WAIT: usize = 21;
    pub const NAMES: [&str; 22] = [
        "thinks", "chance", "jump", "no_spot", "planned", "play", "carrying", "health", "enemy", "hit", "reloading",
        "team_cap", "other", "cand_floor", "cand_spawn", "cand_own", "cand_route", "cand_body", "cand_friend",
        "cand_score", "no_look", "press_wait",
    ];

    static COUNTS: [AtomicU64; 22] = [const { AtomicU64::new(0) }; 22];

    pub fn of(refusal: Refusal) -> usize {
        FIRST_REFUSAL
            + match refusal {
                Refusal::Play(_) => 0,
                Refusal::Carrying => 1,
                Refusal::Health => 2,
                Refusal::Enemy => 3,
                Refusal::Hit => 4,
                Refusal::Reloading => 5,
                Refusal::TeamCap => 6,
                _ => 7,
            }
    }

    pub fn count(k: usize) {
        COUNTS[k].fetch_add(1, Ordering::Relaxed);
    }

    pub fn line() -> String {
        NAMES
            .iter()
            .zip(&COUNTS)
            .map(|(name, count)| format!("{name} {}", count.load(Ordering::Relaxed)))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// Whether an enemy is seen now, sensed lately, or known near.
/// [bot traps T1, D1] A passing bot lays the team's trap on a camper point only with no enemy within this, metres (seen or
/// known): the trap-job design's decision D1.
const PASS_ENEMY_M: f32 = 15.0;

/// [bot traps T1] Whether an enemy seen or known is within `m` of `feet`.
fn enemy_within(bot: &Bot, feet: Vec3, now: u32, clock: u32, m: f32) -> bool {
    bot.memory.all_seen(now, clock).iter().any(|s| s.track().at.distance_xz(feet) < m)
        || bot.memory.tracks().iter().any(|track| track.at.distance_xz(feet) < m)
}

/// [bot traps T1] The team's open camper points (no trap on them, no bot sent) that the bot's route ahead passes: within the
/// job's at-the-point distance (`within_m` + the trap's `reach_m` + 1 m, as a sent bot's) of a point of the route's next
/// `route_ahead_m`, a storey at most apart. A distance test of at most 13 route points against the team's few points; no search.
fn passed_points(bot: &Bot, me: &PlaceView, t: &TrapTuning, knowledge: &super::know::TeamKnowledge) -> Vec<Vec3> {
    use super::know::CamperTrapState;
    let Some(j) = t.camper_job else {
        return Vec::new();
    };
    let open: Vec<Vec3> = knowledge.camper_traps.iter().filter(|c| c.state == CamperTrapState::Open).map(|c| c.at).collect();
    if open.is_empty() {
        return Vec::new();
    }
    let (ahead, _) = route_ahead(bot, me, t);
    let near = j.within_m + t.reach_m + 1.0;
    open.into_iter().filter(|at| ahead.iter().any(|p| p.distance_xz(*at) <= near && (p.y - at.y).abs() < 2.0)).collect()
}

fn enemy_near(bot: &Bot, due: &Due) -> bool {
    enemy_near_at(bot, due.me.feet, due.now, due.view.clock, &due.tuning.traps)
}

/// [`enemy_near`] for a bot at `feet` at `now` (the view's clock `clock`): the gate's own check, at a think or a tick.
pub(super) fn enemy_near_at(bot: &Bot, feet: Vec3, now: u32, clock: u32, t: &TrapTuning) -> bool {
    let quiet = (t.enemy_quiet_s * 1000.0) as u32;
    !bot.memory.all_seen(now, clock).is_empty()
        || bot.memory.tracks().iter().any(|track| sensed_lately(t, track, feet, now, quiet) || track.at.distance_xz(feet) < t.enemy_near_m)
}

/// [`trap_calm`]'s one exception: the reflex's look the trap look took the view over.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OverHeard {
    /// A glance (`reflex::Override::glance`), at a place this far off, metres.
    Glance { m: f32 },
    /// The sights-up aim at an enemy only heard or told of (`Override::heard_aim`): this far off, sensed this long ago.
    Aim { m: f32, age_ms: u32 },
}

/// The facts of a tick the trap look's calm is judged on (`drive.rs`).
#[derive(Clone, Copy, Debug)]
pub struct CalmFacts<'a> {
    pub over: &'a super::reflex::Override,
    /// The reflex's target (a seen enemy it aims at or keeps).
    pub target: bool,
    /// A threat look this tick (`reflex::gate::THREAT`: a hit's shooter, a near shot).
    pub threat: bool,
    /// A use within its reach has the view.
    pub using: bool,
    /// The trap gate's own enemy-near check ([`enemy_near_at`]).
    pub enemy_near: bool,
    pub feet: Vec3,
    pub now: u32,
}

/// [traps.look_over_heard] Whether nothing calls the trap look away this tick, and the reflex look it took the view over if
/// any. Calm: no reflex look, no fire, no legs, no use, no target. With `look_over_heard` (ON) a glance, or the sights-up aim at
/// an enemy only heard or told of, gives way to the trap look as a glance gives way to a use (`drive.rs`), while the gate's
/// enemy-near check is clear (nothing seen now, nothing sensed in `enemy_quiet_s` within `sensed_near`, nothing known within
/// `enemy_near_m`); a seen target, a threat look, fire, legs or a use still win at once. Root cause (the plain bench, 8 matches,
/// 392 looks called away): the sights-up aim 294 (75 %), a glance 85 (22 %), a seen target 10, a threat look 1; the gate had
/// passed, so the plan and the reflex disagreed on "an enemy near".
pub fn trap_calm(f: CalmFacts, t: &TrapTuning) -> (bool, Option<OverHeard>) {
    let over = f.over;
    let others = !over.fire && over.legs.is_none() && !f.using && !f.target;
    let Some(look) = over.look_at else {
        return (others, None);
    };
    if !t.look_over_heard || f.threat || f.enemy_near || !others {
        return (false, None);
    }
    let heard = match over.heard_aim {
        Some((at, sensed)) => Some(OverHeard::Aim { m: at.distance_xz(f.feet), age_ms: f.now.saturating_sub(sensed) }),
        None if over.glance => Some(OverHeard::Glance { m: look.distance_xz(f.feet) }),
        None => None,
    };
    (heard.is_some(), heard)
}

/// The gate's "sensed lately": within `enemy_quiet_s`, and with `sensed_near` (A292 S2) only this near.
fn sensed_lately(t: &TrapTuning, track: &Track, feet: Vec3, now: u32, quiet: u32) -> bool {
    now < track.time + quiet && t.sensed_near.is_none_or(|n| track.at.distance_xz(feet) < n.within_m)
}

/// [A292] Which part of [`enemy_near`] holds, the first of seen / sensed lately / known near (for the counts only).
fn enemy_near_kind(bot: &Bot, due: &Due) -> Option<tally::EnemyNear> {
    let t = &due.tuning.traps;
    let quiet = (t.enemy_quiet_s * 1000.0) as u32;
    let tracks = bot.memory.tracks();
    if !bot.memory.all_seen(due.now, due.view.clock).is_empty() {
        Some(tally::EnemyNear::Seen)
    } else if tracks.iter().any(|track| sensed_lately(t, track, due.me.feet, due.now, quiet)) {
        Some(tally::EnemyNear::Sensed)
    } else if tracks.iter().any(|track| track.at.distance_xz(due.me.feet) < t.enemy_near_m) {
        Some(tally::EnemyNear::Known)
    } else {
        None
    }
}

/// The points of the route ahead, a metre apart, and whether a jump lies
/// within `jump_ahead_m`.
fn route_ahead(bot: &Bot, me: &PlaceView, t: &TrapTuning) -> (Vec<Vec3>, bool) {
    let Some((route, _)) = bot.route.as_ref() else {
        return (Vec::new(), false);
    };
    // A copy: the motor's own progress along the route is not touched.
    let mut route = route.clone();
    let (s, _) = route.progress(me.feet);
    let jump = route
        .next_jump(s)
        .is_some_and(|(_, distance)| distance < t.jump_ahead_m);
    let n = t.route_ahead_m.max(0.0) as usize;
    let points = (0..=n)
        .map(|k| route.point_at((s + k as f32).min(route.length())))
        .collect();
    (points, jump)
}

/// A tick of the body: what the habit asks of the view and the keys, if it
/// is doing anything. `calm` is false when an enemy is seen or the reflex
/// aims at one; `crouch` is the crouch the play asks of this tick's input.
#[allow(clippy::too_many_arguments)]
pub(super) fn drive(
    bot: &mut Bot,
    me: &PlaceView,
    now: u32,
    calm: bool,
    crouch: bool,
    t: &TrapTuning,
    mesh: Option<&mapdata::RayTree>,
    over_heard: Option<OverHeard>,
) -> Option<TrapDrive> {
    let state = bot.features.traps.habit.state();
    // [A292 S4a] From its press to the think that puts the trap in its field (and re-plans round it), the legs stay.
    let hold = held_after_press(bot, now, t);
    if state == State::Idle {
        return hold.then_some(TrapDrive {
            look: None,
            key: 0,
            crouch: None,
            legs: TrapLegs::AfterPress,
        });
    }
    // [halt look] A walking placer halts for its look and its press, where the trap in passing is allowed: calm, no enemy known
    // within PASS_ENEMY_M (one seen is not calm), and not a bot that stays (its legs are its play's).
    let halt = halts_for_look(
        t,
        state,
        calm,
        is_staying(bot, me),
        bot.memory.tracks().iter().any(|track| track.at.distance_xz(me.feet) < PASS_ENEMY_M),
    );
    #[cfg(test)]
    if halt && let Some(c) = bot.features.traps.look_ctx.as_mut() {
        c.halted_ms += (super::kin::TICK_S * 1000.0) as u32;
    }
    let kit = bot.features.traps.habit.kit()?;
    // [traps.look_over_heard] The look took the view over a heard aim or a glance: one note a look.
    if let (Some(heard), State::Looking { since, .. }) = (over_heard, state)
        && bot.features.traps.over_heard_noted != Some(since)
    {
        bot.features.traps.over_heard_noted = Some(since);
        #[cfg(test)]
        called::over_heard(matches!(heard, OverHeard::Aim { .. }));
        #[cfg(test)]
        if let Some(c) = bot.features.traps.look_ctx.as_mut()
            && c.heard.is_none()
        {
            c.heard = Some(heard);
        }
        let what = match heard {
            OverHeard::Aim { m, age_ms } => format!("a sights-up aim at an enemy heard or told of {m:.1} m off, {:.1} s ago", age_ms as f32 / 1000.0),
            OverHeard::Glance { m } => format!("a glance {m:.1} m off"),
        };
        bot.features.traps.notes.push((now, format!("{}trap look took the view over {what}, no enemy near", tally::PREFIX)));
    }
    // The pose the placing is counted on: held from the press to the release.
    let crouched = match state {
        State::Down { crouched, .. } => crouched,
        _ => crouch,
    };
    let aim = aim_of(bot, me, crouched, t, mesh);
    // [A324 measure] What keeps this tick from calm, for the count of a look called away.
    #[cfg(test)]
    let why_not_calm = (!calm).then(|| called::Why::of(bot, me));
    let out = bot.features.traps.habit.tick(
        Tick {
            now,
            yaw: me.yaw,
            pitch: me.pitch,
            calm,
            crouched,
            aim,
        },
        t,
    );
    #[cfg(test)]
    if let Some(c) = bot.features.traps.look_ctx.as_mut() {
        c.last = now;
    }
    // [halt probe] (b) Counts only: at the look's first tick, the press of the body halted where it is with the view on the look
    // (the same `aim_of`, its velocity 0); and the tick the view itself comes onto the look (the time a halt holds the legs).
    #[cfg(test)]
    if let State::Looking { yaw, .. } = state
        && let Aim::Look { pitch, .. } = aim
    {
        if bot.features.traps.look_ctx.is_some_and(|c| c.halt.is_none()) {
            let halted = PlaceView { velocity: [0.0, 0.0], yaw, pitch: pitch.view(), ..me.clone() };
            let press = match aim_of(bot, &halted, crouched, t, mesh) {
                Aim::Look { press: Press::Ready, .. } => None,
                Aim::Look { press: Press::Wait(why), .. } => Some(why),
                _ => Some(Why::NoLook),
            };
            if let Some(c) = bot.features.traps.look_ctx.as_mut() {
                c.halt = Some(press);
            }
        }
        let on = turn(me.yaw, yaw).abs() <= t.tolerance_rad && (Down::of_view(me.pitch).0 - pitch.0).abs() <= PRESS_SLACK;
        if on && let Some(c) = bot.features.traps.look_ctx.as_mut() && c.turned.is_none() {
            c.turned = Some(now);
        }
    }
    if let Some(ended) = out.ended {
        bot.features.traps.habit.record(ended);
        #[cfg(test)]
        if let Some(c) = bot.features.traps.look_ctx.take() {
            placing::ended(
                &c,
                match ended {
                    Ended::Placed => "placed",
                    Ended::CalledAway => "called_away",
                    Ended::GaveUp(_) => "gave_up",
                },
            );
        }
        #[cfg(test)]
        if matches!(ended, Ended::CalledAway) {
            called::count(why_not_calm, bot.assignment.as_ref().is_some_and(|a| a.kind == PlayKind::TrapSpot));
        }
        if bot.tracing && !matches!(ended, Ended::Placed) {
            bot.traced.push(serde_json::json!({
                "t": now, "bot": bot.place, "trap_look_ended": format!("{ended:?}"),
                "at": me.feet.to_array(),
            }));
        }
        // [A292] A look that ended without a trap: its note and count (a press is counted below, where its landing is known).
        let gave_up = match ended {
            Ended::Placed => None,
            Ended::CalledAway => Some("an enemy or a shot called it away".to_string()),
            Ended::GaveUp(Some(why)) => Some(format!("the look ran out ({})", why.name())),
            Ended::GaveUp(None) => Some("the look ran out (unjudged)".to_string()),
        };
        if let Some(why) = gave_up {
            bot.features.traps.tally.ended(ended, false);
            bot.features.traps.notes.push((
                now,
                format!("{}gave up a trap at ({:.1}, {:.1}, {:.1}): {why}", tally::PREFIX, me.feet.x, me.feet.y, me.feet.z),
            ));
        }
    }
    if let Some(planned) = out.placed {
        // The promise is where the head ray of this frame meets the floor,
        // not the spot planned before the look down and the steps taken
        // since (1.8 m off at 3 m/s); a ray too short puts nothing down.
        // With the landing rule the eye is the held pose's.
        // [fix-team trap pose] From the client's ray start, the `Item` bone ahead of the feet ([`item_eye`]).
        let eye = if t.press_landing_m.is_some() { item_eye(me.feet, me.yaw, crouched, me.speed(), me.pitch) } else { me.eye() };
        // [A324] By the client's rule: no promise where the client puts nothing, and the promise on the trap itself.
        let landed = match (t.release, mesh) {
            (TrapRelease::Client, Some(mesh)) => client_landing(mesh, eye, me.yaw, me.pitch, bot.features.traps.field.discs()).ok(),
            _ => landing(eye, me.yaw, me.pitch, planned.y),
        };
        bot.features.traps.placed = landed.map(|at| (at, now));
        bot.features.traps.pressed_at = landed.map(|_| now);
        // [A309] The placer keeps off its fresh trap for a while (`traps.fresh_ring`).
        if let (Some(at), Some(f)) = (landed, t.fresh_ring) {
            bot.features.traps.fresh = Some((at, now.saturating_add(f.for_ms)));
        }
        #[cfg(test)]
        if let Some(at) = landed {
            bot.features.traps.pressed = Some((at, me.velocity, now));
            // [A309] The bench's press line: where, what chose it, which way the placer walked.
            let (source, way, to_home, share) = bot.features.traps.planned_by.unwrap_or(("unknown", "unknown", 0.0, -1.0));
            eprintln!(
                "  trap press: place {} team {} at ({:.1}, {:.1}) t {} source {source} way {way} home {to_home:.0} m forward {share:.2} play {:?} speed {:.1} path1s {:.2} from_feet {:.2} route {}",
                bot.place,
                bot.team,
                at.x,
                at.z,
                now,
                bot.assignment.as_ref().map(|a| a.kind),
                me.speed(),
                // [A324] How near the landing lies to the body's own path over the next second (`traps.press_clear`).
                swept_m(me.feet, me.velocity, 1.0, at),
                at.distance_xz(me.feet),
                bot.route.is_some()
            );
        }
        // [A292] The press: its note and count.
        bot.features.traps.tally.ended(Ended::Placed, landed.is_some());
        let text = match landed {
            Some(at) => format!(
                "{}puts a trap down at ({:.1}, {:.1}, {:.1}), {:.1} m from its feet{} (planned at ({:.1}, {:.1}, {:.1}))",
                tally::PREFIX,
                at.x,
                at.y,
                at.z,
                at.distance_xz(me.feet),
                if crouched { ", crouched" } else { "" },
                planned.x,
                planned.y,
                planned.z,
            ),
            None => format!(
                "{}pressed its trap key but the head ray met no floor (planned at ({:.1}, {:.1}, {:.1}))",
                tally::PREFIX,
                planned.x,
                planned.y,
                planned.z
            ),
        };
        bot.features.traps.notes.push((now, text));
        if bot.tracing {
            bot.traced.push(serde_json::json!({
                "t": now, "bot": bot.place, "trap_promise": landed.map(Vec3::to_array),
                "planned": planned.to_array(),
                "crouched": crouched, "speed": me.speed(),
                "feet_to_landing": landed.map(|at| at.distance_xz(me.feet)),
            }));
        }
    }
    // The view: the aim as angles with the landing rule (the reflex's lead of the eye would put it 0.05 rad off), else
    // the point the old habit looked at.
    let look = out.look_yaw.map(|yaw| match aim {
        Aim::Look { pitch, .. } => LookAt::Angles {
            yaw,
            pitch: pitch.view(),
        },
        Aim::Steepest | Aim::NoLook => LookAt::Point(look_point(me.eye(), yaw, STEEPEST.0)),
    });
    let legs = if hold || (t.after_press.is_some() && out.placed.is_some()) {
        TrapLegs::AfterPress
    } else if halt && out.ended.is_none() {
        TrapLegs::Halt
    } else {
        TrapLegs::Walk
    };
    Some(TrapDrive {
        look,
        key: if out.key_down { kit.key } else { 0 },
        crouch: out.hold_crouch,
        legs,
    })
}

/// [A292 S4a] Whether the legs stay this tick: within `AfterPress::hold_ms` of its last press (`BotState::pressed_at`). The
/// think in between promises the trap to the team and re-plans the route round it; the reflex's own legs wait too.
fn held_after_press(bot: &Bot, now: u32, t: &TrapTuning) -> bool {
    t.after_press
        .zip(bot.features.traps.pressed_at)
        .is_some_and(|(a, at)| now.saturating_sub(at) <= a.hold_ms)
}

/// What the habit waits for this tick: the steepness of the look and whether a press would land right, from the
/// pose the bot has (it may have changed since the plan: the walk, a crouch). Worked out only while looking and
/// pressing; with `press_landing_m` off always the steepest look and no condition.
fn aim_of(bot: &Bot, me: &PlaceView, crouched: bool, t: &TrapTuning, mesh: Option<&mapdata::RayTree>) -> Aim {
    let Some(min_m) = t.press_landing_m else {
        return Aim::Steepest;
    };
    let wanted = wanted_m(t, min_m);
    match bot.features.traps.habit.state() {
        State::Looking { spot, .. } => {
            // The look for the spot's own floor: a ramp or a stair changes how far the ray reaches.
            let Some(look) = press_look(crouched, wanted, me.feet.y - spot.y) else {
                return Aim::NoLook;
            };
            let staying = is_staying(bot, me);
            // The release is two inputs on: the feet there, the view at most the slack steeper than now.
            let lead = Vec3::new(me.velocity[0] * RELEASE_LEAD_S, 0.0, me.velocity[1] * RELEASE_LEAD_S);
            let feet = me.feet + lead;
            let view = (me.pitch - PRESS_SLACK).max(-STEEPEST.0);
            let eye = eye_for(feet, me.yaw, crouched, view);
            // [A324] Where the client puts it, or why it puts nothing there (`traps.release`).
            let landed = match (t.release, mesh) {
                (TrapRelease::Client, Some(mesh)) => {
                    let discs = bot.features.traps.field.discs();
                    // [fix-team trap pose] It holds from the feet [`FEET_ERROR_M`] back and forth along the view too: the
                    // reckoned feet are that far off the client's (the landing promised is the middle one's).
                    let d = dir(me.yaw);
                    let off = |m: f32| eye_for(feet + Vec3::new(d[0] * m, 0.0, d[1] * m), me.yaw, crouched, view);
                    client_landing(mesh, eye, me.yaw, view, discs).and_then(|at| {
                        client_landing(mesh, off(-FEET_ERROR_M), me.yaw, view, discs)?;
                        client_landing(mesh, off(FEET_ERROR_M), me.yaw, view, discs)?;
                        Ok(at)
                    })
                }
                _ => landing(eye, me.yaw, view, spot.y).ok_or(Why::NoFloor),
            };
            let press = match landed {
                Ok(at) => press_of(t, Some(at), feet, staying, || route_ahead(bot, me, t).0),
                Err(why) => Press::Wait(why),
            };
            let landed = landed.ok();
            // [A292 S4a] Not where the body itself runs on to over the next moment.
            let press = match (press, t.after_press, landed) {
                (Press::Ready, Some(a), Some(at)) if swept_m(me.feet, me.velocity, a.sweep_s, at) < a.clear_m => Press::Wait(Why::OnRoute),
                (press, ..) => press,
            };
            // [A309] Nor while the body is slow: a slow placer stays by its trap (`traps.press_moving`).
            let press = match (press, t.press_moving) {
                (Press::Ready, Some(p)) if me.speed() < p.min_speed_ms => Press::Wait(Why::OnRoute),
                (press, _) => press,
            };
            // [A324] Nor on its own way: the client's trap is armed at once and takes its placer's foot (`traps.press_clear`).
            let press = match (press, landed) {
                (Press::Ready, Some(at))
                    if !clears_own_path(t.press_clear, me.feet, me.velocity, at, || {
                        if staying { Vec::new() } else { route_ahead(bot, me, t).0 }
                    }) =>
                {
                    Press::Wait(Why::OnRoute)
                }
                (press, _) => press,
            };
            Aim::Look {
                pitch: look.pitch,
                press,
            }
        }
        State::Down { spot, .. } => {
            // Held through the release: the same look.
            let pitch = press_look(crouched, wanted, me.feet.y - spot.y).map_or(STEEPEST, |look| look.pitch);
            Aim::Look {
                pitch,
                press: Press::Ready,
            }
        }
        _ => Aim::NoLook,
    }
}

/// [A324] Whether a trap at `at` lies clear of the body's own way at the press (`TrapTuning::press_clear`): the straight line of
/// its velocity over `sweep_s`, and the route it walks (`route`: its points ahead, a metre apart, from its progress; empty for a
/// bot that stays) for as far as it gets in that time, by `clear_m`. A walker turning with its route took its own trap 0.9 s after
/// a press 1.21 m clear of its straight line (the A324 by-side check). Always with the switch off.
fn clears_own_path(clear: Option<PressClear>, feet: Vec3, velocity: [f32; 2], at: Vec3, route: impl FnOnce() -> Vec<Vec3>) -> bool {
    let Some(c) = clear else {
        return true;
    };
    if swept_m(feet, velocity, c.sweep_s, at) < c.clear_m {
        return false;
    }
    let reach_m = velocity[0].hypot(velocity[1]) * c.sweep_s;
    if reach_m <= 0.0 {
        return true;
    }
    // The feet, then the route's points within the reach (and the one past it): the stretch the legs take next.
    let way: Vec<Vec3> = std::iter::once(feet).chain(route().into_iter().take(reach_m.ceil() as usize + 2)).collect();
    off_route_m(&way, at) >= c.clear_m
}

/// [A292 S4a] How near `at` comes to the body's own path over the next `sweep_s` (its feet moved by its velocity), metres on
/// the ground.
fn swept_m(feet: Vec3, velocity: [f32; 2], sweep_s: f32, at: Vec3) -> f32 {
    let end = Vec3::new(feet.x + velocity[0] * sweep_s, feet.y, feet.z + velocity[1] * sweep_s);
    off_route_m(&[feet, end], at)
}

/// Whether the bot stays where it is: its play holds it and it is slow. Its spot then need not lie off the route.
fn is_staying(bot: &Bot, me: &PlaceView) -> bool {
    matches!(bot.intent.go, super::plays::Go::Stay) && me.speed() < STANDING_MS
}

/// What the habit asks of a drive tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct TrapDrive {
    pub look: Option<LookAt>,
    pub key: u32,
    /// The crouch key as it was at the press, held to the release (with the landing rule).
    pub crouch: Option<bool>,
    /// What the legs do this tick.
    pub legs: TrapLegs,
}

/// What a trap's look and press ask of the legs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TrapLegs {
    /// The play's walk.
    Walk,
    /// [A292 S4a] They stay from the press until the next think re-plans the route round the trap (`TrapTuning::after_press`).
    AfterPress,
    /// [halt look] They stay for the look and its press, the route kept (`TrapTuning::halt_look`).
    Halt,
}

impl TrapLegs {
    /// The legs stay (no walk, the reflex's own legs wait too).
    pub fn stay(self) -> bool {
        !matches!(self, Self::Walk)
    }
}

/// [halt look] Whether a placer halts this tick (`TrapTuning::halt_look`): while it looks and presses, calm, not a bot that stays
/// (its legs are its play's), and no enemy known within [`PASS_ENEMY_M`] (the trap in passing's own rule).
fn halts_for_look(t: &TrapTuning, state: State, calm: bool, staying: bool, enemy_near: bool) -> bool {
    t.halt_look && matches!(state, State::Looking { .. } | State::Down { .. }) && calm && !staying && !enemy_near
}

/// Where the view is turned.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum LookAt {
    /// At a point (the habit as it was): the caller turns to it from its own eye.
    Point(Vec3),
    /// To these angles (the view's yaw and pitch): the aim itself.
    Angles { yaw: f32, pitch: f32 },
}

/// A bot's trap state, by feature ([`super::features`]): the habit, the point of the trap it let go of since the
/// last think, its notice of the traps of others (`trap_react.rs`) and the floors of its team's traps it keeps off
/// (`trap_know.rs`, shared at each commander tick).
#[derive(Default)]
pub struct BotState {
    pub habit: Habit,
    pub placed: Option<(mapdata::Vec3, u32)>,
    pub react: super::trap_react::React,
    pub field: std::sync::Arc<super::trap_know::TrapField>,
    /// [G61] The traps it put down that only it knows, its team not told: the bench's stand-in human's
    /// (`Bots::bench_placer_knows_trap`), whose team learns them as a live team learns a human mate's. A bot's own release goes to
    /// its team's list instead, so a bot's is empty. Followed by the worlds as the team's list is; kept across its lives.
    pub mine: super::trap_know::TrapList,
    /// [G61] The fields of its team's traps with [`Self::mine`] ([`take_fields`]); none while none of `mine` stands.
    pub mine_fields: Option<MineFields>,
    /// [keep_off] The floors of the wider field it may still take: within the disc of a trap it stands inside, farther from the
    /// trap than its feet were at its last think (sorted): so it walks out, never round.
    pub keep_off_open: std::sync::Arc<Vec<NodeId>>,
    /// The version of the team's traps its route was last looked at against the ring for (`keep_off`).
    pub ringed: u64,
    /// The richer cell ahead the habit is waiting to pass, and since when (`TrapTuning::on_the_pass`).
    pub pass_wait: Option<(mapdata::Vec3, u32)>,
    /// [A292] Its counts for the match-end log line, over all its lives ([`tally::Tally`]).
    pub tally: tally::Tally,
    /// [A292 S4a] When it last let go of a trap, ms: its legs stay `AfterPress::hold_ms` from then.
    pub pressed_at: Option<u32>,
    /// [A309] Its fresh trap and until when it keeps off it (`TrapTuning::fresh_ring`), and the floors its searches may not take:
    /// those within `ring_m` of the trap nearer it than its feet were at the last think.
    pub fresh: Option<(Vec3, u32)>,
    pub fresh_shut: std::sync::Arc<Vec<NodeId>>,
    /// [A292] Log notes of the ticks since its last think (a press, a look that ended) with their times: the think writes them.
    pub notes: Vec<(u32, String)>,
    /// [traps.look_over_heard] The look (its start) whose taking of the view over a heard aim or a glance was noted.
    pub over_heard_noted: Option<u32>,
    /// [#2081] When it last noted a refusal or a re-route for an own trap ([`own_trap_note`]).
    pub own_trap_noted: Option<u32>,
    /// The bench's: the standing own trap this body last came within 3 m of, and the class it came with
    /// ([`passes`]).
    #[cfg(test)]
    pub near: Option<(mapdata::Vec3, usize, usize)>,
    /// The bench's: its last press, where it landed, its ground velocity then and the time (for the catch lines).
    #[cfg(test)]
    pub pressed: Option<(mapdata::Vec3, [f32; 2], u32)>,
    /// [A309] The bench's: what chose its planned spot, which way it walked against its side's home (the spawns' centre) and how
    /// far home was, for the press line.
    #[cfg(test)]
    pub planned_by: Option<(&'static str, &'static str, f32, f32)>,
    /// [#2081 v6 measure] The bench's: its thinks within 2 m of a standing own trap (newest last, at most 6) and what moved its
    /// legs then, for the own-catch line.
    #[cfg(test)]
    pub near_moves: Vec<NearMove>,
    /// [#2081 v6 measure] The bench's: the feet the motor's own-trap guard last saw of itself (the bot's believed feet) and when,
    /// for the own-catch line's believed against true distance.
    #[cfg(test)]
    pub guard_seen: Option<(u32, Vec3)>,
    /// [#2081] Since when the motor's own-trap guard has held its route's keys with the body at rest ([`guard_own_traps`]).
    pub guard_stall: Option<u32>,
    /// [#2081] The armed traps the last world showed (`Bots::traps_from_world`): the guard takes a promised trap's real place from
    /// them at once, not at the next think ([`guard_discs`]).
    pub world_armed: std::sync::Arc<Vec<Vec3>>,
    /// [#2081] The route the contact-only fallback search gave ([`on_fallback`]), its goal while its search goes on, and the field
    /// at [`FALLBACK_REACH_M`] with the field it was built from.
    pub fallback: Option<FallbackRoute>,
    pub fallback_pending: Option<Vec3>,
    pub fallback_field: Option<(Arc<TrapField>, Arc<TrapField>)>,
    /// [bot traps] When the route check last took a search's route at the guard's contact ([`own_trap_route_within`]): the route set
    /// next is marked as the fallback's (`Bot::set_route` takes it, set or not).
    pub taken_at_contact: Option<u32>,
    /// [#2081 v6 measure] The bench's: the look in hand ([`placing`]).
    #[cfg(test)]
    pub look_ctx: Option<placing::Ctx>,
}

/// [#2081 v6 measure] One think of a bot within 2 m of a standing own trap: when, how near, and what moved its legs at its last
/// tick (`Bot::bench_keys_from`: 0 the walk or stand, 1 the fight's side step, 2 the step apart from a mate; `bench_apart`,
/// `bench_stepped` as the drive set them), whether it had a route, an order to stay, a hold, and its speed.
#[cfg(test)]
#[derive(Clone, Copy, Debug)]
pub struct NearMove {
    pub t: u32,
    pub d: f32,
    pub keys_from: u8,
    pub apart: u8,
    pub stepped: u8,
    pub route: bool,
    pub stay: bool,
    pub hold: bool,
    pub speed: f32,
    /// The route ahead's nearest approach to the trap (its next 15 m) before this think's keep-off check, and whether that check
    /// ended the route.
    pub route_m: Option<f32>,
    pub ended: bool,
}

#[cfg(test)]
impl NearMove {
    pub fn line(moves: &[NearMove], now: u32) -> String {
        moves
            .iter()
            .map(|m| {
                format!(
                    "-{:.1}s {:.2}m from{} ap{} st{} {}{}{} {:.1} pre {}{}",
                    now.saturating_sub(m.t) as f32 / 1000.0,
                    m.d,
                    m.keys_from,
                    m.apart,
                    m.stepped,
                    if m.route { "R" } else { "r" },
                    if m.stay { "S" } else { "g" },
                    if m.hold { "H" } else { "h" },
                    m.speed,
                    m.route_m.map_or("-".to_string(), |r| format!("{r:.2}")),
                    if m.ended { " ENDED" } else { "" }
                )
            })
            .collect::<Vec<_>>()
            .join(" | ")
    }
}

impl super::features::Feature for BotState {
    fn new_life(&mut self, _life: &super::features::Life) {
        #[cfg(test)]
        {
            if matches!(self.habit.state(), State::Looking { .. } | State::Down { .. })
                && let Some(c) = self.look_ctx
            {
                placing::died(c);
            }
            self.look_ctx = None;
            self.near_moves.clear();
        }
        self.habit.new_life();
        self.react.new_life();
        self.pass_wait = None;
        self.pressed_at = None;
        self.fresh = None;
        self.fresh_shut = Default::default();
        self.keep_off_open = Default::default();
        #[cfg(test)]
        {
            self.near = None;
        }
    }
}

/// [A292] What the habit did, live and per bot: where its thinks stopped, why its candidate spots were refused, and how
/// its looks ended. Only counts: no decision reads it. Summed by team into the match-end log line ([`super::Bots::trap_report`]).
pub mod tally {
    use super::{Ended, Refusal, Why};

    /// Every log note of the habit starts so ("trap: plans ...", "trap: puts ...", "trap: gave up ..."): made from what the bot
    /// already decided, they leave the bench's decision digest as it was (`tests/tallies/decisions.rs`, as `nav_log::PREFIX`).
    pub const PREFIX: &str = "trap: ";
    use crate::v026g::bots::plays::PlayKind;

    /// Why one of the eight candidate spots round the bot was refused ([`super::choose`]), in the order it is tested.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Candidate {
        /// Not level, no room to stand, a lip, a fall near, or not open all round (`good_floor`).
        Floor,
        /// No look of the pose lands there (the landing rule).
        NoLook,
        /// In a spawn area.
        Spawn,
        /// An own trap within `near_own_m`.
        Own,
        /// On its own route ahead (`lateral_m`).
        Route,
        /// On its own body.
        Body,
        /// A mate there now, in 2 s or in 4 s.
        Friend,
        /// No enemy human's lane cell within 6 m at all (presence 0).
        NoLane,
        /// Some lane presence, but the score under `min_score`.
        LowScore,
    }

    impl Candidate {
        pub const ALL: [Self; 9] = [
            Self::Floor,
            Self::NoLook,
            Self::Spawn,
            Self::Own,
            Self::Route,
            Self::Body,
            Self::Friend,
            Self::NoLane,
            Self::LowScore,
        ];

        pub const fn index(self) -> usize {
            match self {
                Self::Floor => 0,
                Self::NoLook => 1,
                Self::Spawn => 2,
                Self::Own => 3,
                Self::Route => 4,
                Self::Body => 5,
                Self::Friend => 6,
                Self::NoLane => 7,
                Self::LowScore => 8,
            }
        }

        pub const fn name(self) -> &'static str {
            match self {
                Self::Floor => "floor",
                Self::NoLook => "no look",
                Self::Spawn => "spawn area",
                Self::Own => "own trap near",
                Self::Route => "on its route",
                Self::Body => "its body",
                Self::Friend => "a mate",
                Self::NoLane => "no enemy lane",
                Self::LowScore => "score too low",
            }
        }
    }

    /// Which part of the enemy-near gate stopped a think ([`super::Refusal::Enemy`]), the first that holds.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum EnemyNear {
        /// An enemy seen now.
        Seen,
        /// An enemy sensed within `enemy_quiet_s`.
        Sensed,
        /// An enemy known within `enemy_near_m`.
        Known,
    }

    impl EnemyNear {
        pub const ALL: [Self; 3] = [Self::Seen, Self::Sensed, Self::Known];

        pub const fn index(self) -> usize {
            match self {
                Self::Seen => 0,
                Self::Sensed => 1,
                Self::Known => 2,
            }
        }

        pub const fn name(self) -> &'static str {
            match self {
                Self::Seen => "seen",
                Self::Sensed => "sensed lately",
                Self::Known => "known near",
            }
        }
    }

    /// The gate's refusals past the cheap part, by reason ([`super::gate`]).
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Gate {
        Play,
        Carrying,
        Health,
        Enemy,
        Hit,
        Reloading,
        Jump,
        TeamCap,
    }

    impl Gate {
        const ALL: [Self; 8] = [
            Self::Play,
            Self::Carrying,
            Self::Health,
            Self::Enemy,
            Self::Hit,
            Self::Reloading,
            Self::Jump,
            Self::TeamCap,
        ];

        const fn index(self) -> usize {
            match self {
                Self::Play => 0,
                Self::Carrying => 1,
                Self::Health => 2,
                Self::Enemy => 3,
                Self::Hit => 4,
                Self::Reloading => 5,
                Self::Jump => 6,
                Self::TeamCap => 7,
            }
        }

        const fn name(self) -> &'static str {
            match self {
                Self::Play => "an urgent play",
                Self::Carrying => "carrying",
                Self::Health => "health",
                Self::Enemy => "an enemy near",
                Self::Hit => "hit lately",
                Self::Reloading => "reloading",
                Self::Jump => "a jump ahead",
                Self::TeamCap => "the team's cap",
            }
        }

        /// The gate reason of a refusal; `None` for the cheap part's (no trap, life spent, busy), counted apart.
        fn of(refusal: Refusal) -> Option<Self> {
            match refusal {
                Refusal::Play(_) => Some(Self::Play),
                Refusal::Carrying => Some(Self::Carrying),
                Refusal::Health => Some(Self::Health),
                Refusal::Enemy => Some(Self::Enemy),
                Refusal::Hit => Some(Self::Hit),
                Refusal::Reloading => Some(Self::Reloading),
                Refusal::Jump => Some(Self::Jump),
                Refusal::TeamCap => Some(Self::TeamCap),
                Refusal::Off | Refusal::NoTrap | Refusal::LifeSpent | Refusal::Busy | Refusal::Skill => None,
            }
        }
    }

    /// One bot's counts (or a team's sum).
    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct Tally {
        /// Thinks of a bot with a trap left: the cheap part's refusals (`life spent`, `busy`) and the thinks that went on.
        spent: u32,
        busy: u32,
        /// [fix-team 5 T8] Thinks of a life its skill gives no traps of its own.
        skill: u32,
        thinks: u32,
        gate: [u32; 8],
        enemy: [u32; 3],
        /// The urgent plays that stopped a think, by play.
        plays: Vec<(PlayKind, u32)>,
        chance: u32,
        /// Thinks of a bot that stays, with `stayer: never`.
        stayer: u32,
        no_look: u32,
        waited_wish: u32,
        waited_pass: u32,
        searched: u32,
        no_spot: u32,
        /// Every refused candidate of the searches, by [`Candidate`].
        candidates: [u32; 9],
        planned: u32,
        pressed: u32,
        /// Presses whose head ray met no floor (nothing put down).
        pressed_air: u32,
        called_away: u32,
        /// Looks given up, by the last [`Why`] ([`Why::index`]), and with nothing to name.
        gave_up: [u32; Why::ALL.len()],
        gave_up_unjudged: u32,
    }

    impl Tally {
        /// A think stopped by the cheap part ([`super::Habit::may_plan`]): only `life spent` and `busy` are counted (a bot
        /// with no trap or with the habit off has nothing to count).
        pub fn may_not(&mut self, refusal: Refusal) {
            match refusal {
                Refusal::LifeSpent => self.spent += 1,
                Refusal::Busy => self.busy += 1,
                Refusal::Skill => self.skill += 1,
                _ => {}
            }
        }

        pub fn think(&mut self) {
            self.thinks += 1;
        }

        pub fn refused(&mut self, refusal: Refusal, enemy: Option<EnemyNear>) {
            let Some(gate) = Gate::of(refusal) else {
                return;
            };
            self.gate[gate.index()] += 1;
            if let (Gate::Enemy, Some(near)) = (gate, enemy) {
                self.enemy[near.index()] += 1;
            }
            if let Refusal::Play(play) = refusal {
                match self.plays.iter_mut().find(|(p, _)| *p == play) {
                    Some((_, n)) => *n += 1,
                    None => self.plays.push((play, 1)),
                }
            }
        }

        pub fn chance(&mut self) {
            self.chance += 1;
        }

        pub fn jump(&mut self) {
            self.gate[Gate::Jump.index()] += 1;
        }

        pub fn no_look(&mut self) {
            self.no_look += 1;
        }

        pub fn stayer(&mut self) {
            self.stayer += 1;
        }

        pub fn waited_wish(&mut self) {
            self.waited_wish += 1;
        }

        pub fn waited_pass(&mut self) {
            self.waited_pass += 1;
        }

        /// A search ran ([`super::choose`]): its refused candidates, and whether it found a spot.
        pub fn searched(&mut self, refused: &[u32; 9], found: bool) {
            self.searched += 1;
            for (sum, n) in self.candidates.iter_mut().zip(refused) {
                *sum += n;
            }
            if !found {
                self.no_spot += 1;
            }
        }

        pub fn planned(&mut self) {
            self.planned += 1;
        }

        /// A look ended: let go of (on a floor or in the air), called away, or given up.
        pub fn ended(&mut self, ended: Ended, landed: bool) {
            match ended {
                Ended::Placed if landed => self.pressed += 1,
                Ended::Placed => self.pressed_air += 1,
                Ended::CalledAway => self.called_away += 1,
                Ended::GaveUp(Some(why)) => self.gave_up[why.index()] += 1,
                Ended::GaveUp(None) => self.gave_up_unjudged += 1,
            }
        }

        pub fn add(&mut self, other: &Self) {
            self.spent += other.spent;
            self.busy += other.busy;
            self.skill += other.skill;
            self.thinks += other.thinks;
            for (a, b) in self.gate.iter_mut().zip(&other.gate) {
                *a += b;
            }
            for (a, b) in self.enemy.iter_mut().zip(&other.enemy) {
                *a += b;
            }
            for &(play, n) in &other.plays {
                match self.plays.iter_mut().find(|(p, _)| *p == play) {
                    Some((_, m)) => *m += n,
                    None => self.plays.push((play, n)),
                }
            }
            self.chance += other.chance;
            self.no_look += other.no_look;
            self.stayer += other.stayer;
            self.waited_wish += other.waited_wish;
            self.waited_pass += other.waited_pass;
            self.searched += other.searched;
            self.no_spot += other.no_spot;
            for (a, b) in self.candidates.iter_mut().zip(&other.candidates) {
                *a += b;
            }
            self.planned += other.planned;
            self.pressed += other.pressed;
            self.pressed_air += other.pressed_air;
            self.called_away += other.called_away;
            for (a, b) in self.gave_up.iter_mut().zip(&other.gave_up) {
                *a += b;
            }
            self.gave_up_unjudged += other.gave_up_unjudged;
        }

        /// The log text of the counts.
        pub fn line(&self) -> String {
            let gate = Gate::ALL
                .iter()
                .map(|g| format!("{} {}", g.name(), self.gate[g.index()]))
                .collect::<Vec<_>>()
                .join(", ");
            let enemy = EnemyNear::ALL
                .iter()
                .map(|e| format!("{} {}", e.name(), self.enemy[e.index()]))
                .collect::<Vec<_>>()
                .join(", ");
            let mut plays = self.plays.clone();
            plays.sort_by_key(|(p, n)| (std::cmp::Reverse(*n), format!("{p:?}")));
            let plays = plays.iter().map(|(p, n)| format!("{p:?} {n}")).collect::<Vec<_>>().join(", ");
            let candidates = Candidate::ALL
                .iter()
                .map(|c| format!("{} {}", c.name(), self.candidates[c.index()]))
                .collect::<Vec<_>>()
                .join(", ");
            let gave_up = Why::ALL
                .iter()
                .map(|w| format!("{} {}", w.name(), self.gave_up[w.index()]))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "life spent {}, busy {}, not by skill {}; thinks {}: refused by {gate} (the enemy: {enemy}; the plays: {plays}); \
                 the chance {}, a stayer {}, no look {}, waited for a wished spot {}, waited for the pass {}; searched {}, no spot {} \
                 (candidates refused: {candidates}); planned {}; pressed {} (in the air {}), called away {}, gave up {} \
                 ({gave_up}, unjudged {})",
                self.spent,
                self.busy,
                self.skill,
                self.thinks,
                self.chance,
                self.stayer,
                self.no_look,
                self.waited_wish,
                self.waited_pass,
                self.searched,
                self.no_spot,
                self.planned,
                self.pressed,
                self.pressed_air,
                self.called_away,
                self.gave_up.iter().sum::<u32>() + self.gave_up_unjudged,
                self.gave_up_unjudged,
            )
        }
    }
}

impl super::Bots {
    /// [A292] The trap habit's counts of both teams, as the match-end log line (`the bots' traps: ...`).
    pub fn trap_report(&self) -> String {
        let mut sums = [tally::Tally::default(), tally::Tally::default()];
        for bot in self.bots.iter().flatten() {
            sums[usize::from(bot.team.min(1))].add(&bot.features.traps.tally);
        }
        format!("team 0: {}; team 1: {}", sums[0].line(), sums[1].line())
    }
}

#[cfg(test)]
mod item_eye_tests {
    use super::*;

    /// [fix-team trap pose] The ray starts ahead of the feet as the posed clip has it: 0.159 m at the stop standing (the worlds
    /// measure 0.135-0.17 m), less as the view rises, about none level; straight up by `view::eye_height`.
    #[test]
    fn the_trap_ray_starts_ahead_of_the_feet_by_the_view() {
        let stop = super::super::pitch::Pitch::new(-1.0).elevation(super::super::pitch::PitchScale::Client);
        assert!((item_ahead_m(false, stop) - 0.159).abs() < 1e-3);
        assert!((item_ahead_m(true, stop) - 0.117).abs() < 1e-3);
        let mid = super::super::pitch::Pitch::new(-0.75).elevation(super::super::pitch::PitchScale::Client);
        assert!((item_ahead_m(false, mid) - 0.137).abs() < 1e-3, "{}", item_ahead_m(false, mid));
        assert!(item_ahead_m(false, 0.3).abs() < 0.05);
        let feet = Vec3::new(1.0, 2.0, 3.0);
        let eye = item_eye(feet, 0.0, false, 0.0, stop);
        let d = dir(0.0);
        assert!((eye.x - feet.x - d[0] * 0.159).abs() < 1e-3 && (eye.z - feet.z - d[1] * 0.159).abs() < 1e-3);
        assert!((eye.y - feet.y - super::super::view::eye_height(false, 0.0, stop)).abs() < 1e-6);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v026g::bots::plays::tests::Scenario;
    use crate::v026g::bots::tuning::Check;
    use crate::v026g::bots::view::Mode;

    fn t() -> TrapTuning {
        crate::v026g::bots::tuning::Tuning::all_off().traps
    }

    fn facts() -> Facts {
        Facts {
            now: 100_000,
            play: Some(PlayKind::Roam),
            carrying: false,
            health: 1.0,
            enemy: false,
            last_hit: None,
            reloading: false,
            jump_ahead: false,
            team_standing: 0,
        }
    }

    #[test]
    fn the_gate_refuses_each_thing_that_calls_the_bot_away() {
        assert_eq!(gate(&facts(), &t()), Ok(()));
        let refused = |change: fn(&mut Facts)| {
            let mut f = facts();
            change(&mut f);
            gate(&f, &t()).unwrap_err()
        };
        assert_eq!(
            refused(|f| f.play = Some(PlayKind::Collect)),
            Refusal::Play(PlayKind::Collect)
        );
        assert_eq!(
            refused(|f| f.play = Some(PlayKind::Retreat)),
            Refusal::Play(PlayKind::Retreat)
        );
        assert_eq!(refused(|f| f.carrying = true), Refusal::Carrying);
        assert_eq!(refused(|f| f.health = 0.4), Refusal::Health);
        assert_eq!(refused(|f| f.enemy = true), Refusal::Enemy);
        assert_eq!(refused(|f| f.last_hit = Some(95_000)), Refusal::Hit);
        assert_eq!(refused(|f| f.reloading = true), Refusal::Reloading);
        assert_eq!(refused(|f| f.jump_ahead = true), Refusal::Jump);
        assert_eq!(refused(|f| f.team_standing = 4), Refusal::TeamCap);
        // A hit long ago does not count; the plays that wait do not refuse.
        let mut f = facts();
        f.last_hit = Some(90_000);
        f.play = Some(PlayKind::Ambush);
        assert_eq!(gate(&f, &t()), Ok(()));
    }

    /// The habit of a kit: none carried is [`Habit::none`], else the stack and the quick-use bit's mask; the counts
    /// of what it did (a life that ended while placing is the harm).
    #[test]
    fn the_habit_comes_from_the_kit_and_counts_what_it_did() {
        assert_eq!(Habit::of_kit(None), Habit::none());
        let habit = Habit::of_kit(Some((2, 16)));
        assert_eq!(habit.kit(), Some(TrapKit { count: 2, key: 0x1_0000 }));
        assert_eq!(habit.counts(), PlaceCounts { armed: 1, ..PlaceCounts::default() });
        let mut habit = habit;
        habit.new_life();
        assert_eq!(habit.counts().died_placing, 0, "a life that began idle");
        habit.state = State::Looking { yaw: 0.0, spot: Vec3::ZERO, since: 0, last: None };
        habit.new_life();
        assert_eq!(habit.counts().died_placing, 1);
        assert_eq!(habit.state(), State::Idle);
        habit.state = State::Down { yaw: 0.0, spot: Vec3::ZERO, crouched: false };
        habit.new_life();
        assert_eq!(habit.counts().died_placing, 2, "the key down");
        // The cooldown after a look is not placing: a life that ends in it is not counted.
        habit.state = State::Cooling { until: 30_000 };
        habit.new_life();
        assert_eq!(habit.counts().died_placing, 2, "a life that ended in the cooldown");
        assert_eq!(habit.state(), State::Idle);
    }

    /// [Gap 11] The pocket holds what each world shows, unless the bot let go of a trap too recently for the world to
    /// show it: a release the client did not turn into a placement is undone at the next world past the lag.
    #[test]
    fn the_trap_pocket_holds_what_the_world_shows() {
        let lag = crate::v026g::bots::stock::PRESS_LAG_MS;
        assert_eq!(Habit::none().read(1_000, 2), None, "no trap carried");
        let mut habit = Habit::of_kit(Some((2, 16)));
        assert_eq!(habit.read(1_000, 2), Some(false));
        // Let go of one at 5 s: counted at once.
        habit.left -= 1;
        habit.counted = Some(5_000);
        assert_eq!(habit.read(5_100, 2), None, "a world too new for the release");
        assert_eq!(habit.left(), 1);
        assert_eq!(habit.read(5_000 + lag, 2), Some(true), "the client placed nothing");
        assert_eq!(habit.left(), 2);
        assert_eq!(habit.read(9_000, 0), Some(true));
        assert_eq!(habit.may_plan(&t()), Err(Refusal::NoTrap));
        assert_eq!((habit.counts().world_read, habit.counts().world_changed), (3, 2));
        habit.new_life();
        assert_eq!((habit.left(), habit.counted), (2, None));
    }

    #[test]
    fn a_bot_with_no_kit_or_switched_off_never_plans() {
        let mut off = t();
        off.enabled = false;
        assert_eq!(Habit::none().may_plan(&t()), Err(Refusal::NoTrap));
        let habit = Habit::carry(TrapKit {
            count: 2,
            key: 0x8000,
        });
        assert_eq!(habit.may_plan(&off), Err(Refusal::Off));
        assert_eq!(habit.may_plan(&t()), Ok(()));
    }

    fn tick(now: u32, yaw: f32, pitch: f32, calm: bool) -> Tick {
        Tick {
            now,
            yaw,
            pitch,
            calm,
            crouched: false,
            aim: Aim::Steepest,
        }
    }

    #[test]
    fn it_looks_down_presses_one_tick_lets_go_and_goes_on() {
        let mut habit = Habit::carry(TrapKit {
            count: 2,
            key: 0x8000,
        });
        habit.plan(1.0, Vec3::new(5.0, 0.0, 5.0), 1_000);
        // Turning: no key yet.
        let out = habit.tick(tick(1_050, 0.2, -0.3, true), &t());
        assert_eq!(
            (out.look_yaw, out.key_down, out.placed),
            (Some(1.0), false, None)
        );
        // On the yaw but not steep enough.
        let out = habit.tick(tick(1_100, 1.0, -0.5, true), &t());
        assert!(!out.key_down);
        // On it: the key goes down for this tick.
        let out = habit.tick(tick(1_150, 1.02, -1.0, true), &t());
        assert!(out.key_down && out.placed.is_none());
        assert!(matches!(habit.state(), State::Down { .. }));
        // The next tick lets go: the trap is put down and counted.
        let out = habit.tick(tick(1_200, 1.02, -1.0, true), &t());
        assert!(!out.key_down);
        assert_eq!(out.placed, Some(Vec3::new(5.0, 0.0, 5.0)));
        assert_eq!(habit.placed_this_life(), 1);
        // Cooling, then idle; one a life by default.
        assert!(matches!(habit.state(), State::Cooling { .. }));
        assert_eq!(habit.may_plan(&t()), Err(Refusal::LifeSpent));
        let later = 1_200 + 21_000;
        habit.tick(tick(later, 0.0, 0.0, true), &t());
        assert_eq!(habit.state(), State::Idle);
        // A new life: the pocket is full again.
        habit.new_life();
        assert_eq!(habit.may_plan(&t()), Ok(()));
    }

    #[test]
    fn an_enemy_or_a_slow_look_ends_it_without_a_trap() {
        let mut habit = Habit::carry(TrapKit {
            count: 2,
            key: 0x8000,
        });
        habit.plan(1.0, Vec3::ZERO, 1_000);
        let out = habit.tick(tick(1_100, 0.0, 0.0, false), &t());
        assert_eq!(out, Out { ended: Some(Ended::CalledAway), ..Out::default() });
        assert!(matches!(habit.state(), State::Cooling { .. }));
        assert_eq!(habit.placed_this_life(), 0);
        // Never found the spot within the look time.
        let mut habit = Habit::carry(TrapKit {
            count: 2,
            key: 0x8000,
        });
        habit.plan(1.0, Vec3::ZERO, 1_000);
        habit.tick(tick(1_000 + t().look_ms + 1, 0.0, 0.0, true), &t());
        assert!(matches!(habit.state(), State::Cooling { .. }));
        assert_eq!(habit.placed_this_life(), 0);
    }

    #[test]
    fn the_look_point_is_the_steepest_look_and_the_head_ray_reaches_the_floor() {
        let eye = Vec3::new(10.0, 1.6, 10.0);
        let p = look_point(eye, 0.0, LOOK_DOWN);
        let (dx, dz) = (p.x - eye.x, p.z - eye.z);
        let pitch = (eye.y - p.y).atan2(dx.hypot(dz));
        assert!((pitch - LOOK_DOWN).abs() < 1e-4);
        // The floor point at the 1.6 m eye is 1.0 m out, the ray 1.9 m < 2.0.
        let ray = 1.6 / LOOK_DOWN.sin();
        assert!(ray < 2.0 && ray > 1.8, "{ray}");
        assert!((1.6 / LOOK_DOWN.tan() - 1.0).abs() < 0.04);
    }

    /// An enemy human known at `at` for `seconds` (a place a second).
    fn lanes_at(at: Vec3, seconds: u32, now: u32) -> HumanLanes {
        let mut lanes = HumanLanes::default();
        for k in 0..seconds {
            lanes.knew(9, at, now - 1_000 * (seconds - k));
        }
        lanes
    }

    /// A search with nothing round the bot; a test sets what it tries.
    fn search<'a>(
        s: &'a Scenario,
        t: &'a TrapTuning,
        feet: Vec3,
        lanes: &'a HumanLanes,
        in_spawn: &'a dyn Fn(Vec3) -> bool,
    ) -> Search<'a> {
        Search {
            t,
            reach_m: t.reach_m,
            press: None,
            grid: &s.tactics.world.grid,
            now: 100_000,
            view_yaw: 0.0,
            feet,
            staying: false,
            ahead: &[],
            mates: &[],
            lanes,
            own: &[],
            in_spawn,
            wish: None,
            homes: None,
        }
    }

    #[test]
    fn the_spot_is_on_the_enemys_lane_off_the_bots_own_route_and_clear_of_mates() {
        let s = Scenario::field(Mode::TeamDeathmatch);
        let tuning = t();
        let feet = Vec3::new(30.0, 0.0, 10.0);
        let not_spawn = |_: Vec3| false;
        // The enemy's lane is 3 m to +x of the bot: the best yaw is the one
        // that puts the spot nearest it.
        let lanes = lanes_at(Vec3::new(33.0, 0.0, 10.0), 6, 100_000);
        let chosen = choose(&search(&s, &tuning, feet, &lanes, &not_spawn)).expect("a spot");
        let d = dir(chosen.yaw);
        assert!(d[0] > 0.9, "{chosen:?}");
        assert!((chosen.spot.distance_xz(feet) - 1.0).abs() < 1e-3);
        // Its route runs +x: the spot there lies on the route, so the best
        // is another yaw off it (a metre of lateral).
        let route: Vec<Vec3> = (0..=12)
            .map(|k| Vec3::new(30.0 + k as f32, 0.0, 10.0))
            .collect();
        let off = choose(&Search {
            ahead: &route,
            ..search(&s, &tuning, feet, &lanes, &not_spawn)
        })
        .expect("a spot");
        assert!(
            off_route_m(&route, off.spot) >= tuning.lateral_m - 1e-3,
            "{off:?}"
        );
        // A mate at the first choice forbids that yaw.
        let mates = [(chosen.spot + Vec3::new(0.5, 0.0, 0.0), [0.0, 0.0])];
        let by_mate = choose(&Search {
            mates: &mates,
            ..search(&s, &tuning, feet, &lanes, &not_spawn)
        })
        .expect("a spot");
        assert!(by_mate.spot.distance_xz(mates[0].0) >= tuning.friend_m);
        // A mate walking in at 2 s forbids it too.
        let walking = [(chosen.spot + Vec3::new(-4.0, 0.0, 0.0), [2.0, 0.0])];
        let by_walker = choose(&Search {
            mates: &walking,
            ..search(&s, &tuning, feet, &lanes, &not_spawn)
        });
        assert!(
            by_walker.is_none_or(|b| b.yaw != chosen.yaw),
            "{by_walker:?}"
        );
        // An own trap within 4 m of all eight forbids them all.
        let own = [feet + Vec3::new(1.5, 0.0, 0.0)];
        assert_eq!(
            choose(&Search {
                own: &own,
                ..search(&s, &tuning, feet, &lanes, &not_spawn)
            }),
            None
        );
        // A spawn area forbids it.
        let spawn = |_: Vec3| true;
        assert_eq!(choose(&search(&s, &tuning, feet, &lanes, &spawn)), None);
        // No enemy presence: a spot scoring under the least is not taken.
        let none = HumanLanes::default();
        assert_eq!(choose(&search(&s, &tuning, feet, &none, &not_spawn)), None);
    }

    /// [A145] A candidate on the spot of a killed holder is taken though no lane runs there, over a spot the lanes like, and not with
    /// a mate within the wider guard; the others keep their rules.
    #[test]
    fn a_killed_holders_spot_is_taken_without_lanes_and_with_no_mate_near() {
        let s = Scenario::field(Mode::TeamDeathmatch);
        let tuning = t();
        let feet = Vec3::new(30.0, 0.0, 10.0);
        let not_spawn = |_: Vec3| false;
        // The lanes like +x; the holder's spot is at -x (nothing runs there).
        let lanes = lanes_at(Vec3::new(33.0, 0.0, 10.0), 6, 100_000);
        let spot = [Vec3::new(28.5, 0.0, 10.0)];
        let wish = |friend_m: f32| Wish {
            spots: &spot,
            within_m: 1.0,
            friend_m,
        };
        let plain = choose(&search(&s, &tuning, feet, &lanes, &not_spawn)).expect("a spot");
        assert!(!plain.wished && dir(plain.yaw)[0] > 0.9, "{plain:?}");
        let wished = choose(&Search {
            wish: Some(wish(6.0)),
            ..search(&s, &tuning, feet, &lanes, &not_spawn)
        })
        .expect("the holder's spot");
        assert!(wished.wished && dir(wished.yaw)[0] < -0.9, "{wished:?}");
        // No lanes at all: only the wished spot passes the least score.
        let none = HumanLanes::default();
        let only = choose(&Search {
            wish: Some(wish(6.0)),
            ..search(&s, &tuning, feet, &none, &not_spawn)
        })
        .expect("the holder's spot");
        assert!(only.wished, "{only:?}");
        // A mate 4 m from the spot (outside the guard's 2.5 m, inside the wider 6 m): not there; the lanes' spot is taken instead.
        let mates = [(Vec3::new(24.5, 0.0, 10.0), [0.0, 0.0])];
        let guarded = choose(&Search {
            wish: Some(wish(6.0)),
            mates: &mates,
            ..search(&s, &tuning, feet, &lanes, &not_spawn)
        })
        .expect("a spot");
        assert!(!guarded.wished, "{guarded:?}");
        // The same mate does not stop the ordinary guard.
        let narrow = choose(&Search {
            wish: Some(wish(2.5)),
            mates: &mates,
            ..search(&s, &tuning, feet, &lanes, &not_spawn)
        })
        .expect("the holder's spot");
        assert!(narrow.wished, "{narrow:?}");
        // The spawn area and the team's own trap still forbid it.
        let spawn = |at: Vec3| at.x < 29.5;
        assert!(choose(&Search {
            wish: Some(wish(6.0)),
            ..search(&s, &tuning, feet, &none, &spawn)
        })
        .is_none());
        let own = [Vec3::new(28.5, 0.0, 10.0)];
        assert!(choose(&Search {
            wish: Some(wish(6.0)),
            own: &own,
            ..search(&s, &tuning, feet, &none, &not_spawn)
        })
        .is_none());
    }

    #[test]
    fn a_wall_makes_the_floor_beside_it_unfit() {
        // The field's wall across x = 40 (z 0 to 25): floor in the open is
        // fit, floor in the wall's lee, with too little open round it, is not,
        // and nor is the field's edge.
        let s = Scenario::field(Mode::TeamDeathmatch);
        let tuning = t();
        let grid = &s.tactics.world.grid;
        assert!(good_floor(grid, Vec3::new(30.0, 0.0, 10.0), &tuning).is_some());
        let mut narrow = tuning.clone();
        narrow.open_floors = 10_000;
        assert!(good_floor(grid, Vec3::new(30.0, 0.0, 10.0), &narrow).is_none());
        assert!(good_floor(grid, Vec3::new(30.0, 0.0, 500.0), &tuning).is_none());
    }

    #[test]
    fn a_route_over_a_trap_is_dropped_and_one_that_keeps_off_it_is_kept() {
        use crate::v026g::bots::path::{Route, Waypoint};
        use crate::v026g::bots::think::Due;
        use crate::v026g::bots::trap_know::TrapList;
        let mut s = Scenario::field(Mode::TeamDeathmatch);
        s.bot(0, 0, Vec3::new(15.0, 0.0, 10.0));
        let me = s.view.places[0].clone();
        let tuning = crate::v026g::bots::tuning::Tuning::all_off();
        let due = Due {
            now: 1_000,
            view: &s.view,
            me: &me,
            tactics: &s.tactics,
            tuning: &tuning,
        };
        let mut bot = Bot::new(1, 0, 0, None, &tuning);
        let waypoint = |x: f32, z: f32| Waypoint {
            at: Vec3::new(x, 0.0, z),
            node: 0,
            jump: None,
            low: false,
        };
        let mut list = TrapList::default();
        list.promise(0, Vec3::new(25.0, 0.0, 10.0), 500);
        bot.features.traps.field = Arc::new(TrapField::of(&list, &s.tactics.world.grid));
        // Over the disc: dropped.
        let goal = Vec3::new(35.0, 0.0, 10.0);
        bot.route = Some((
            Route::new(vec![waypoint(15.0, 10.0), waypoint(35.0, 10.0)]),
            goal,
        ));
        keep_off(&mut bot, &due);
        assert!(bot.route.is_none());
        // Three metres off it: kept.
        bot.route = Some((
            Route::new(vec![waypoint(15.0, 13.0), waypoint(35.0, 13.0)]),
            goal,
        ));
        keep_off(&mut bot, &due);
        assert!(bot.route.is_some());
        // No trap known: nothing is ever dropped.
        bot.features.traps.field = Arc::new(TrapField::default());
        bot.route = Some((
            Route::new(vec![waypoint(15.0, 10.0), waypoint(35.0, 10.0)]),
            goal,
        ));
        keep_off(&mut bot, &due);
        assert!(bot.route.is_some());
    }

    #[test]
    fn a_promised_trap_shuts_its_floors_wider_and_never_shuts_its_placer_in() {
        use crate::v026g::bots::think::Due;
        use crate::v026g::bots::trap_know::TrapList;
        let trap = Vec3::new(25.0, 0.0, 10.0);
        let mut s = Scenario::field(Mode::TeamDeathmatch);
        s.bot(0, 0, trap + Vec3::new(1.2, 0.0, 0.0));
        let grid = &s.tactics.world.grid;
        let tuning = crate::v026g::bots::tuning::Tuning::default();
        let keep = tuning.traps.keep_off.expect("keep_off is on by default");
        let mut list = TrapList::default();
        list.promise(0, trap, 500);
        let field = field_of(&list, grid, 0.0, Some(keep));
        let at = |d: f32| grid.locate(trap + Vec3::new(d, 0.0, 0.0), 1.5).unwrap();
        // Promised: shut out to the clearance + the promise's margin (2.18 m), open past it.
        assert!(!field.open(at(1.8)), "within the promise's wider disc");
        assert!(field.open(at(2.5)));
        // The placer 1.2 m off its fresh trap: the floors farther than its feet are opened out to that reach.
        let me = s.view.places[0].clone();
        let due = Due { now: 1_000, view: &s.view, me: &me, tactics: &s.tactics, tuning: &tuning };
        let mut bot = Bot::new(1, 0, 0, None, &tuning);
        bot.features.traps.field = field;
        keep_off_open(&mut bot, &due, &tuning.traps);
        assert!(bot.features.traps.keep_off_open.binary_search(&at(1.8)).is_ok(), "the placer is never shut in");
    }

    /// [#2081] The fallback search: the field's wall at x = 40 with one gap 2.5 m wide (z 25-27.5) and an own trap 1.0 m before its
    /// middle: every way through crosses the wall's line within 1.33 m of the trap and beyond 0.90 m, none at the clearance. The fallback (the trap shut only to the
    /// guard's contact) finds one; the route check takes it at the fallback's reach, not the clearance; keep_off keeps the marked
    /// route as the bot walks it (unmarked, the clearance drops it); and the guard still refuses a step into contact.
    #[test]
    fn a_way_past_an_own_trap_nearer_than_the_clearance_is_the_fallbacks_and_is_kept() {
        use crate::v026g::bots::think::Due;
        use crate::v026g::bots::trap_know::TrapList;
        let mut s = Scenario::field(Mode::TeamDeathmatch);
        let walls = crate::v026g::bots::body::tests::wall_x(40.0, 27.5, 40.0);
        s.tactics = crate::v026g::bots::tactics::Tactics::new(crate::v026g::bots::tactics::tests::field_with_solids(&walls, |_| {}), 1.0);
        let trap = Vec3::new(39.0, 0.0, 26.25);
        let feet = Vec3::new(30.0, 0.0, 30.0);
        let goal = Vec3::new(50.0, 0.0, 30.0);
        s.bot(0, 0, feet);
        let tuning = crate::v026g::bots::tuning::Tuning::default();
        let keep = tuning.traps.keep_off.expect("keep_off is on by default");
        let mut list = TrapList::default();
        list.promise(0, trap, 500);
        let grid = &s.tactics.world.grid;
        let (from, to) = (grid.locate(feet, 1.5).unwrap(), grid.locate(goal, 1.5).unwrap());
        let mut bot = Bot::new(1, 0, 0, None, &tuning);
        bot.features.traps.field = Arc::new(TrapField::of_ringed_within(&list, grid, 0.0, keep.hard_m));
        // At the clearance: no way.
        assert!(s.tactics.route_nodes(from, to, 0, Some(0), false, &filter(&bot, Vec::new(), Some(from))).is_none(), "no way at 1.47 m");
        // The fallback: a way, refused at the clearance and kept at the fallback's reach.
        let narrow = fallback_field(&mut bot, grid);
        let way = s
            .tactics
            .route_nodes(from, to, 0, Some(0), false, &filter_within(&bot, &narrow, Vec::new(), Some(from)))
            .expect("a way past the trap at the guard's contact");
        assert!(own_trap_route(&mut bot, feet, Some(way.clone()), 1_000, &s.tactics, false).is_none(), "the clearance refuses it");
        assert_eq!(bot.features.traps.taken_at_contact, None, "a leg on the trap's floor is no floor crossing");
        let kept = own_trap_route_within(&mut bot, feet, Some(way), 1_000, &s.tactics, false, Some(FALLBACK_REACH_M)).expect("kept at the fallback's reach");
        assert!(off_route_m(&route_points(&kept, feet, f32::MAX), trap) >= FALLBACK_REACH_M - CLEAR_SLACK_M);
        // keep_off: the marked route stands; the same route unmarked is dropped.
        let me = s.view.places[0].clone();
        let due = Due { now: 1_000, view: &s.view, me: &me, tactics: &s.tactics, tuning: &tuning };
        bot.route = Some((kept.clone(), goal));
        mark_fallback(&mut bot, 1_000);
        assert!(on_fallback(&bot));
        keep_off(&mut bot, &due);
        assert!(bot.route.is_some(), "keep_off keeps the fallback's route");
        bot.features.traps.fallback = None;
        keep_off(&mut bot, &due);
        assert!(bot.route.is_none(), "unmarked, the clearance drops it");
        // The guard: a body 0.9 m from the trap walking at it is still let go.
        let yaw = std::f32::consts::FRAC_PI_2; // facing -x
        let at = PlaceView { feet: trap + Vec3::new(0.9, 0.0, 0.0), yaw, velocity: [0.0, 0.0], keys: 0, seen_at: 0, known_at: 0, ..me.clone() };
        let keys = guard_own_traps(&mut bot, super::super::key::FORWARD, 0.0, &at, &tuning, grid, |_| true, 1_000);
        assert_eq!(keys.map(|k| k & super::super::key::FORWARD), Some(0), "the guard holds contact on the fallback's way");
        // On the fallback's route by the trap the body walks tight; off it, or far from the trap, it does not.
        bot.route = Some((kept.clone(), goal));
        mark_fallback(&mut bot, 1_000);
        assert!(walks_tight(&bot, trap + Vec3::new(1.5, 0.0, 0.0)), "tight within 3 m on the fallback's route");
        assert!(!walks_tight(&bot, trap + Vec3::new(5.0, 0.0, 0.0)), "not beyond 3 m");
        bot.features.traps.fallback = None;
        assert!(!walks_tight(&bot, trap + Vec3::new(1.5, 0.0, 0.0)), "not on a route at the clearance");
        // A guard hold at rest on the fallback's route by the team trap waits past the longest think gap: the route stands.
        mark_fallback(&mut bot, 1_000);
        for k in 0..20 {
            guard_own_traps(&mut bot, super::super::key::FORWARD, 0.0, &at, &tuning, grid, |_| true, 2_000 + 50 * k);
        }
        assert!(bot.route.is_some() && on_fallback(&bot), "held 1 s at rest, the fallback's route is not ended");
    }

    /// [bot traps] A route whose one leg nearer than the clearance climbs from the trap's floor to one 1.08 m up beside it (the
    /// bench's tdm-m7-s2 at d2993ba6d: the one climb out of a low spot, 1.21 m from a team trap, refused 285 times) is taken at the
    /// guard's contact and, once set, held there as the fallback's; the same leg on the trap's own floor is refused.
    #[test]
    fn a_climb_beside_an_own_trap_nearer_than_the_clearance_is_taken_at_its_contact() {
        use crate::v026g::bots::path::{Route, Waypoint};
        use crate::v026g::bots::trap_know::TrapList;
        let mut s = Scenario::field(Mode::TeamDeathmatch);
        let trap = Vec3::new(20.0, 0.0, 20.0);
        let feet = Vec3::new(21.5, 0.0, 14.0);
        s.bot(0, 0, feet);
        let tuning = crate::v026g::bots::tuning::Tuning::default();
        let keep = tuning.traps.keep_off.expect("keep_off is on by default");
        let mut list = TrapList::default();
        list.promise(0, trap, 500);
        let grid = &s.tactics.world.grid;
        let mut bot = Bot::new(1, 0, 0, None, &tuning);
        bot.features.traps.field = Arc::new(TrapField::of_ringed_within(&list, grid, 0.0, keep.hard_m));
        let waypoint = |x: f32, y: f32, z: f32| Waypoint { at: Vec3::new(x, y, z), node: 0, jump: None, low: false };
        // Up the trap's floor 1.5 m off it, then a climb of 1.08 m passing 1.21 m off, then on along the upper floor.
        let route = |top: f32| {
            Route::new(vec![
                waypoint(21.5, 0.0, 14.0),
                waypoint(21.5, 0.0, 20.6),
                waypoint(20.5, top, 21.1),
                waypoint(20.5, top, 28.0),
            ])
        };
        let goal = Vec3::new(20.5, 1.08, 28.0);
        let taken = own_trap_route(&mut bot, feet, Some(route(1.08)), 1_000, &s.tactics, false).expect("taken at the contact");
        assert_eq!(bot.features.traps.taken_at_contact, Some(1_000));
        assert!(bot.features.traps.notes.iter().any(|n| n.1.contains("taken at the guard's contact")), "{:?}", bot.features.traps.notes);
        bot.set_route(Some((taken, goal)), 1_000);
        assert!(on_fallback(&bot) && bot.features.traps.taken_at_contact.is_none(), "held as the fallback's, the mark spent");
        bot.route = None;
        bot.features.traps.fallback = None;
        assert!(own_trap_route(&mut bot, feet, Some(route(0.0)), 2_000, &s.tactics, false).is_none(), "on the trap's floor: refused");
        assert_eq!(bot.features.traps.taken_at_contact, None);
    }

    /// [#2081] A body standing 0.70 m from its own trap (nearer than the inside floor, 0.85 m) always finds a way out: the search
    /// from its floor to goals on four sides gives a way, and the own-trap check keeps it (the bench's tdm s7: every way out refused
    /// 464 times).
    #[test]
    fn a_body_by_its_own_trap_always_finds_a_way_out() {
        use crate::v026g::bots::think::Due;
        use crate::v026g::bots::trap_know::TrapList;
        let trap = Vec3::new(25.0, 0.0, 10.0);
        let feet = trap + Vec3::new(0.70, 0.0, 0.0);
        let mut s = Scenario::field(Mode::TeamDeathmatch);
        s.bot(0, 0, feet);
        let tuning = crate::v026g::bots::tuning::Tuning::default();
        let keep = tuning.traps.keep_off.expect("keep_off is on by default");
        let mut list = TrapList::default();
        list.promise(0, trap, 500);
        let me = s.view.places[0].clone();
        let due = Due { now: 1_000, view: &s.view, me: &me, tactics: &s.tactics, tuning: &tuning };
        let mut bot = Bot::new(1, 0, 0, None, &tuning);
        bot.features.traps.field = field_of(&list, &s.tactics.world.grid, 0.0, Some(keep));
        keep_off_open(&mut bot, &due, &tuning.traps);
        let grid = &s.tactics.world.grid;
        let start = grid.locate(feet, 1.5).unwrap();
        let filter = super::filter(&bot, Vec::new(), Some(start));
        for goal in [Vec3::new(35.0, 0.0, 10.0), Vec3::new(15.0, 0.0, 10.0), Vec3::new(25.0, 0.0, 16.0), Vec3::new(25.0, 0.0, 4.0)] {
            let to = grid.locate(goal, 1.5).unwrap();
            let way = s.tactics.route_nodes(start, to, 0, Some(0), false, &filter).unwrap_or_else(|| panic!("a way out to {goal:?}"));
            assert!(own_trap_route(&mut bot, feet, Some(way), 1_000, &s.tactics, false).is_some(), "kept to {goal:?}");
        }
    }

    #[test]
    fn a_route_within_the_ring_is_searched_again_once_for_each_change_of_the_traps() {
        use crate::v026g::bots::path::{Route, Waypoint};
        use crate::v026g::bots::think::Due;
        use crate::v026g::bots::trap_know::TrapList;
        let mut s = Scenario::field(Mode::TeamDeathmatch);
        s.bot(0, 0, Vec3::new(15.0, 0.0, 10.0));
        let me = s.view.places[0].clone();
        let tuning = crate::v026g::bots::tuning::Tuning::all_off();
        let due = Due {
            now: 1_000,
            view: &s.view,
            me: &me,
            tactics: &s.tactics,
            tuning: &tuning,
        };
        let mut bot = Bot::new(1, 0, 0, None, &tuning);
        let waypoint = |x: f32, z: f32| Waypoint {
            at: Vec3::new(x, 0.0, z),
            node: 0,
            jump: None,
            low: false,
        };
        let mut list = TrapList::default();
        list.promise(0, Vec3::new(25.0, 0.0, 10.0), 500);
        let goal = Vec3::new(35.0, 0.0, 10.0);
        // A route 1.0 m off the centre: clear of the hard field (0.75 m), within the ring of 1.3 m.
        let near = |bot: &mut Bot| {
            bot.route = Some((Route::new(vec![waypoint(15.0, 11.0), waypoint(35.0, 11.0)]), goal));
        };
        // Ring off: kept, as it was.
        bot.features.traps.field = Arc::new(TrapField::of_ringed(&list, &s.tactics.world.grid, 0.0));
        near(&mut bot);
        keep_off(&mut bot, &due);
        assert!(bot.route.is_some());
        // Ring on: dropped once ...
        bot.features.traps.field = Arc::new(TrapField::of_ringed(&list, &s.tactics.world.grid, 1.3));
        near(&mut bot);
        keep_off(&mut bot, &due);
        assert!(bot.route.is_none());
        // ... and a route of the same kind, planned after it (a corridor with no way round), is kept.
        near(&mut bot);
        keep_off(&mut bot, &due);
        assert!(bot.route.is_some(), "not searched again for the same change");
        // A change of the traps looks again.
        list.promise(1, Vec3::new(30.0, 0.0, 30.0), 600);
        bot.features.traps.field = Arc::new(TrapField::of_ringed(&list, &s.tactics.world.grid, 1.3));
        keep_off(&mut bot, &due);
        assert!(bot.route.is_none());
        // A route 2 m off is never looked at.
        bot.route = Some((Route::new(vec![waypoint(15.0, 12.0), waypoint(35.0, 12.0)]), goal));
        list.promise(2, Vec3::new(30.0, 0.0, 33.0), 700);
        bot.features.traps.field = Arc::new(TrapField::of_ringed(&list, &s.tactics.world.grid, 1.3));
        keep_off(&mut bot, &due);
        assert!(bot.route.is_some());
    }

    /// [#2081] One clearance: a route the search gives past an own trap keeps `OWN_TRAP_CLEAR_M` from it, and keep_off keeps that
    /// route at every think along it; a route nearer is refused by the search's check and dropped by keep_off alike; a body inside
    /// the disc may walk out (never nearer than its feet) and not in; the side step and the step apart take no step onto it.
    #[test]
    fn one_clearance_from_own_traps_holds_for_the_search_keep_off_and_the_side_step() {
        use crate::v026g::bots::path::{Route, Waypoint};
        use crate::v026g::bots::route_filter::RouteFilter;
        use crate::v026g::bots::think::Due;
        use crate::v026g::bots::trap_know::TrapList;
        let s = Scenario::field(Mode::TeamDeathmatch);
        let grid = &s.tactics.world.grid;
        let trap = Vec3::new(25.0, 0.0, 10.0);
        let mut list = TrapList::default();
        list.promise(0, trap, 500);
        let keep = crate::v026g::bots::tuning::KeepOff::default();
        assert_eq!(keep.hard_m, OWN_TRAP_CLEAR_M, "the tuning's default is the one constant");
        let field = Arc::new(TrapField::of_ringed_within(&list, grid, 0.0, keep.hard_m));
        assert_eq!(field.reach(), OWN_TRAP_CLEAR_M);
        // The search with the field's floors shut, from one side of the trap to the other: its way keeps the clearance.
        let shut = field.clone();
        let filter = RouteFilter::open().with_floors(move |n| shut.open(n));
        let (from, to) = (grid.locate(Vec3::new(15.0, 0.0, 10.0), 1.5).unwrap(), grid.locate(Vec3::new(35.0, 0.0, 10.0), 1.5).unwrap());
        let way = s.tactics.route_nodes(from, to, 0, Some(0), false, &filter).expect("a way round the trap");
        let feet = grid.position(from);
        let tuning = crate::v026g::bots::tuning::Tuning::all_off();
        let mut bot = Bot::new(1, 0, 0, None, &tuning);
        bot.features.traps.field = field.clone();
        let kept = own_trap_route(&mut bot, feet, Some(way.clone()), 1_000, &s.tactics, false).expect("the search's way is kept by its check");
        let points = route_points(&kept, feet, f32::MAX);
        assert!(off_route_m(&points, trap) >= OWN_TRAP_CLEAR_M - CLEAR_SLACK_M, "{}", off_route_m(&points, trap));
        // keep_off keeps it, from its start and from each point along it.
        let goal = grid.position(to);
        for k in 0..points.len() {
            let mut s2 = Scenario::field(Mode::TeamDeathmatch);
            s2.bot(0, 0, points[k]);
            let me = s2.view.places[0].clone();
            let due = Due { now: 1_000, view: &s2.view, me: &me, tactics: &s2.tactics, tuning: &tuning };
            bot.route = Some((kept.clone(), goal));
            keep_off(&mut bot, &due);
            assert!(bot.route.is_some(), "keep_off dropped the search's way at {:?}", points[k]);
        }
        // A way 1.0 m off the trap: refused by the search's check and dropped by keep_off.
        let waypoint = |x: f32, z: f32| Waypoint { at: Vec3::new(x, 0.0, z), node: 0, jump: None, low: false };
        let near = Route::new(vec![waypoint(15.0, 11.0), waypoint(35.0, 11.0)]);
        assert!(own_trap_route(&mut bot, Vec3::new(15.0, 0.0, 11.0), Some(near.clone()), 2_000, &s.tactics, false).is_none());
        // From inside the disc (1.04 m off, a guard by its own trap) a way over the trap is refused for a step straight out of the
        // disc, which keep_off keeps.
        let guard = trap + Vec3::new(-1.04, 0.0, 0.0);
        let over = Route::new(vec![waypoint(guard.x, guard.z), waypoint(35.0, 10.0)]);
        let out = own_trap_route(&mut bot, guard, Some(over), 3_000, &s.tactics, false).expect("a step out");
        let end = out.point_at(out.length());
        assert!(end.distance_xz(trap) >= OWN_TRAP_CLEAR_M, "{end:?}");
        assert!(end.x < guard.x, "away from the trap: {end:?}");
        assert!(keeps_clear(&route_points(&out, guard, f32::MAX), field.discs(), guard, OWN_TRAP_CLEAR_M));
        let mut s3 = Scenario::field(Mode::TeamDeathmatch);
        s3.bot(0, 0, Vec3::new(15.0, 0.0, 11.0));
        let me = s3.view.places[0].clone();
        let due = Due { now: 2_000, view: &s3.view, me: &me, tactics: &s3.tactics, tuning: &tuning };
        bot.route = Some((near, goal));
        keep_off(&mut bot, &due);
        assert!(bot.route.is_none(), "keep_off drops what the check refuses");
        // From inside the disc (1.1 m off, a placer by its own trap): out is clear, in is not.
        let inside = trap + Vec3::new(-1.1, 0.0, 0.0);
        assert!(keeps_clear(&[inside, trap + Vec3::new(-2.5, 0.0, 0.0)], field.discs(), inside, OWN_TRAP_CLEAR_M));
        assert!(!keeps_clear(&[inside, trap + Vec3::new(-0.5, 0.0, 0.3)], field.discs(), inside, OWN_TRAP_CLEAR_M));
        // From 1.36 m (the bench's stuck guard), round it at 1.26 m: clear; at 0.85 m (inside the clearance, under the contact and
        // drift floor): not.
        let at136 = trap + Vec3::new(-1.36, 0.0, 0.0);
        let arc = |r: f32| -> Vec<Vec3> {
            std::iter::once(at136)
                .chain((0..=12).map(|k| {
                    let a = std::f32::consts::PI * (1.0 - k as f32 / 12.0);
                    trap + Vec3::new(r * a.cos(), 0.0, r * a.sin())
                }))
                .collect()
        };
        assert!(keeps_clear(&arc(1.26), field.discs(), at136, OWN_TRAP_CLEAR_M));
        // [#2081] A body nearer than the inside floor (0.85 m) walks out never nearer than it stands (the bench's tdm s7: 0.71 m from
        // its trap, every way out refused 464 times); in, it never does.
        let at071 = trap + Vec3::new(0.71, 0.0, 0.0);
        assert!(keeps_clear(&[at071, trap + Vec3::new(0.69, 0.0, 0.3), trap + Vec3::new(0.2, 0.0, 3.0)], field.discs(), at071, OWN_TRAP_CLEAR_M));
        assert!(!keeps_clear(&[at071, trap + Vec3::new(0.5, 0.0, 0.0)], field.discs(), at071, OWN_TRAP_CLEAR_M));
        assert!(!keeps_clear(&arc(0.85), field.discs(), at136, OWN_TRAP_CLEAR_M));
        // A route whose leg over the trap's spot runs on the floor above (1.2 to 2.0 m up), its legs walked one by one: clear, though
        // its points a metre apart would join the floors.
        let w3 = |x: f32, y: f32, z: f32| Waypoint { at: Vec3::new(x, y, z), node: 0, jump: None, low: false };
        let above = Route::new(vec![w3(20.0, 0.0, 12.5), w3(21.0, 1.2, 12.0), w3(25.0, 2.0, 10.0), w3(30.0, 2.0, 10.0)]);
        let feet3 = Vec3::new(20.0, 0.0, 12.5);
        assert!(keeps_clear(&route_points(&above, feet3, f32::MAX), field.discs(), feet3, OWN_TRAP_CLEAR_M));
        // A route to a goal 1.3 m from the trap (a guard's spot by its own trap) is kept, and keep_off keeps it; a goal on the trap
        // (0.5 m) is not; a step toward that goal is still refused (it would step in).
        let start = Vec3::new(20.0, 0.0, 10.0);
        let to_guard = Route::new(vec![w3(20.0, 0.0, 10.0), w3(23.7, 0.0, 10.0)]);
        assert!(own_trap_route(&mut bot, start, Some(to_guard.clone()), 4_000, &s.tactics, false).is_some());
        let on_it = Route::new(vec![w3(20.0, 0.0, 10.0), w3(24.5, 0.0, 10.0)]);
        assert!(!route_keeps_clear(&route_points(&on_it, start, f32::MAX), field.discs(), start, Some(on_it.end()), OWN_TRAP_CLEAR_M));
        assert!(!step_keeps_clear(&field, trap + Vec3::new(-1.5, 0.0, 0.0), trap + Vec3::new(-0.9, 0.0, 0.0)));
        // A trap on another floor (2 m above) is no matter.
        assert!(keeps_clear(&[Vec3::new(15.0, 2.0, 10.0), Vec3::new(35.0, 2.0, 10.0)], field.discs(), Vec3::new(15.0, 2.0, 10.0), OWN_TRAP_CLEAR_M));
        // The side step (`STRAFE_CLEAR_M` long): onto the trap refused, away from it taken; with no trap, any.
        let by = trap + Vec3::new(-1.6, 0.0, 0.0);
        assert!(!step_keeps_clear(&field, by, by + Vec3::new(0.7, 0.0, 0.0)));
        assert!(step_keeps_clear(&field, by, by + Vec3::new(-0.7, 0.0, 0.0)));
        assert!(step_keeps_clear(&TrapField::default(), by, by + Vec3::new(0.7, 0.0, 0.0)));
    }

    /// [#2081] The own-trap guard, by mover: keys whose forecast path (the body's blend and a stop) comes within contact of a known
    /// own trap while nearing it give way, whatever moved them (the walk, the side step, a sprint, a step back, a glide with no key);
    /// a jump over it is legal, a landing on it is not; a body inside contact may leave; a trap on another floor is no matter.
    #[test]
    fn the_own_trap_guard_keeps_every_mover_out_of_contact() {
        use crate::v026g::drive::key;
        let kin = crate::v026g::bots::kin::Kinematics::default();
        // Facing +x (yaw 0 faces `kin::forward(0)`): the trap 1.2 m ahead along the facing.
        let yaw = 0.0;
        let [fx, fz] = crate::v026g::bots::kin::forward(yaw);
        let [lx, lz] = crate::v026g::bots::kin::forward(yaw + std::f32::consts::FRAC_PI_2);
        let feet = Vec3::ZERO;
        let ahead = |m: f32| Vec3::new(fx * m, 0.0, fz * m);
        let left = |m: f32| Vec3::new(lx * m, 0.0, lz * m);
        let still = [0.0, 0.0];
        let walking = [fx * 3.5, fz * 3.5];
        let guard = |velocity: [f32; 2], keys: u32, discs: &[Vec3], jumping: bool| {
            let discs: Vec<GuardDisc> = discs.iter().map(|&d| d.into()).collect();
            own_trap_guard(|k| forecast(&kin, feet, velocity, yaw, 0.0, k, GUARD_HOLD_TICKS), |_| true, feet, velocity, yaw, 0.0, keys, &discs, jumping)
        };
        // The walk on toward a trap ahead: refused (no move key from rest, the brake at a walk). From rest two ticks of a key and a
        // stop carry the body about 0.3 m, at a walk about 0.9 m: the guard stops it short.
        assert_eq!(guard(still, key::FORWARD, &[ahead(0.8)], false), OwnTrapGuard::Swapped(0));
        assert_eq!(guard(walking, key::FORWARD, &[ahead(1.4)], false), OwnTrapGuard::Swapped(0), "let go: the glide stops short");
        assert!(matches!(guard(walking, key::FORWARD, &[ahead(1.1)], false), OwnTrapGuard::Braked(k) if k & key::BACK != 0), "the glide too: the brake");
        // Walking with the trap well ahead: clear (it stops short this tick).
        assert_eq!(guard(still, key::FORWARD, &[ahead(4.0)], false), OwnTrapGuard::Clear);
        // The fight's side step onto it: the walk without the side key.
        assert_eq!(guard(still, key::LEFT, &[left(0.8)], false), OwnTrapGuard::Swapped(0));
        assert_eq!(guard(still, key::FORWARD | key::LEFT, &[left(0.8)], false), OwnTrapGuard::Swapped(key::FORWARD));
        // A sprint toward it: refused; the step back (a retreat or a dodge) onto one behind: refused.
        assert_ne!(guard([fx * 4.96, fz * 4.96], key::FORWARD | key::SPRINT, &[ahead(1.7)], false), OwnTrapGuard::Clear);
        assert_eq!(guard(still, key::BACK, &[ahead(-0.8)], false), OwnTrapGuard::Swapped(0));
        // A glide with no move key (a sprint's carry past a route's end) into it: the brake.
        assert!(matches!(guard([fx * 4.96, fz * 4.96], 0, &[ahead(0.9)], false), OwnTrapGuard::Braked(k) if k & key::BACK != 0));
        // A jump over it is legal (the guard leaves the press alone); a landing within contact is not: the route's jump leg that
        // ends there breaks the clearance.
        assert_eq!(guard(walking, key::FORWARD | key::JUMP, &[ahead(0.5)], true), OwnTrapGuard::Clear);
        let trap = ahead(2.0);
        assert!(keeps_clear(&[feet, ahead(4.0)], &[Vec3::new(trap.x, 2.0, trap.z)], feet, OWN_TRAP_CLEAR_M), "over it, above");
        assert!(!keeps_clear(&[feet, ahead(2.2)], &[trap], feet, OWN_TRAP_CLEAR_M), "a landing 0.2 m from it");
        // Inside contact, leaving it: clear; on another floor: clear; none near: clear.
        assert_eq!(guard(still, key::BACK, &[ahead(0.4)], false), OwnTrapGuard::Clear);
        assert_eq!(guard(still, key::FORWARD, &[Vec3::new(fx, 2.0, fz)], false), OwnTrapGuard::Clear);
        assert_eq!(guard(still, key::FORWARD, &[], false), OwnTrapGuard::Clear);
    }

    #[test]
    fn the_pass_rule_waits_for_a_richer_cell_ahead_and_places_here_otherwise() {
        // Nothing ahead to look at: here.
        assert_eq!(on_the_pass(Some(1.0), &[], 1.0), Pass::Here);
        // A cell ahead richer by more than the gain: wait for the richest one.
        assert_eq!(on_the_pass(Some(0.5), &[0.2, 2.0, 3.0, 1.0], 1.0), Pass::Wait(2));
        // By exactly the gain or less: here.
        assert_eq!(on_the_pass(Some(1.0), &[0.2, 2.0, 1.5], 1.0), Pass::Here);
        // No spot at the feet: a cell above the gain itself is waited for, a poor one is not.
        assert_eq!(on_the_pass(None, &[0.4, 1.5], 1.0), Pass::Wait(1));
        assert_eq!(on_the_pass(None, &[0.4, 0.9], 1.0), Pass::Here);
        // The first of equal cells is not preferred over a later richer one.
        assert_eq!(on_the_pass(Some(0.0), &[2.0, 2.0, 2.5], 1.0), Pass::Wait(2));
    }

    #[test]
    fn the_pass_rule_is_on_by_default_off_in_the_all_off_patch_validated_and_read_from_a_patch() {
        use crate::v026g::bots::tuning::{PassTuning, Tuning};
        assert_eq!(Tuning::default().traps.on_the_pass, Some(PassTuning::default()));
        assert_eq!(Tuning::all_off().traps.on_the_pass, None);
        assert_eq!(Tuning::default().traps.enemy_near_m, 20.0);
        assert_eq!(Tuning::all_off().traps.enemy_near_m, 50.0);
        let mut tuning = Tuning::all_off();
        tuning.traps.on_the_pass = Some(PassTuning::default());
        tuning.validate().unwrap();
        tuning.traps.on_the_pass = Some(PassTuning { route_ahead_m: 30.0, gain: 1.0 });
        assert!(tuning.validate().is_err(), "further than the route ahead the habit reads");
        tuning.traps.on_the_pass = Some(PassTuning { route_ahead_m: 12.0, gain: -1.0 });
        assert!(tuning.validate().is_err());
        let patched = Tuning::parse(r#"{"traps":{"on_the_pass":{"route_ahead_m":8.0,"gain":0.5}}}"#).unwrap();
        assert_eq!(patched.traps.on_the_pass, Some(PassTuning { route_ahead_m: 8.0, gain: 0.5 }));
    }

    #[test]
    fn the_ring_is_on_by_default_off_in_the_all_off_patch_and_validated() {
        assert_eq!(crate::v026g::bots::tuning::Tuning::default().traps.ring_m, 1.3);
        let mut tuning = crate::v026g::bots::tuning::Tuning::all_off();
        assert_eq!(tuning.traps.ring_m, 0.0);
        tuning.traps.ring_m = 1.3;
        tuning.validate().unwrap();
        tuning.traps.ring_m = 0.5;
        assert!(tuning.validate().is_err(), "inside the hard field");
        tuning.traps.ring_m = 9.0;
        assert!(tuning.validate().is_err(), "a ring this wide shuts the map");
    }

    #[test]
    fn the_defaults_are_valid_and_the_team_counts_what_stands() {
        crate::v026g::bots::tuning::Tuning::all_off()
            .validate()
            .unwrap();
        let mut bad = crate::v026g::bots::tuning::Tuning::all_off();
        bad.traps.reach_m = 2.0;
        assert_eq!(bad.validate().unwrap_err(), "traps.reach_m");
        // A landing no pose reaches (the standing eye reaches 1.3 m with the ray's 2.0 m) is refused, not planned for.
        let mut far = crate::v026g::bots::tuning::Tuning::default();
        far.traps.press_landing_m = Some(1.2);
        assert!(far.validate().is_ok());
        far.traps.press_landing_m = Some(1.5);
        assert_eq!(far.validate().unwrap_err(), "traps.press_landing_m");
        far.traps.press_landing_m = Some(0.5);
        assert_eq!(far.validate().unwrap_err(), "traps.press_landing_m");
    }

    /// [A324] The camper trap job is on by default and off in the all-off arm; the sends after a holder's death are gone (the
    /// old keys are no tuning's).
    #[test]
    fn the_camper_job_is_on_by_default_and_the_sends_after_a_death_are_gone() {
        use crate::v026g::bots::tuning::Tuning;
        assert!(Tuning::default().traps.camper_job.is_some());
        assert!(Tuning::all_off().traps.camper_job.is_none());
        assert!(Tuning::patched(serde_json::json!({"traps": {"at_defeated": {}}})).is_err());
        assert!(Tuning::patched(serde_json::json!({"traps": {"at_cleared": {}}})).is_err());
    }

    /// The eye of a pose at the pitch a look down has (`view::eye_height`, up positive).
    fn eye_at(feet: Vec3, crouched: bool, speed: f32, pitch_down: f32) -> Vec3 {
        feet + Vec3::new(0.0, crate::v026g::bots::view::eye_height(crouched, speed, -pitch_down), 0.0)
    }

    /// The steepest look lands 0.85 m out standing and 0.55 m crouched and still: on the trigger's 0.6 m of the
    /// placer's own foot. The look that [`press_look`] picks lands where it was asked, from either pose and from a
    /// floor higher or lower than the feet (a ramp, a stair), with the head ray inside its 2.0 m, and the landing
    /// [`landing`] predicts from the pose at the press is as far.
    #[test]
    fn the_press_look_lands_where_asked_from_a_standing_or_a_crouched_eye_on_a_level_floor_or_not() {
        let (feet, yaw) = (Vec3::new(10.0, 0.0, 4.0), kin::yaw_of([1.0, 0.0]));
        for (crouched, steepest_m) in [(false, 0.85), (true, 0.55)] {
            let eye = eye_at(feet, crouched, 0.0, LOOK_DOWN);
            let at = landing(eye, yaw, -LOOK_DOWN, 0.0).expect("lands");
            assert!(
                (at.distance_xz(feet) - steepest_m).abs() < 0.03 && at.distance_xz(feet) < TRIGGER_M + 0.3,
                "crouched {crouched}: the steepest look lands {} m out",
                at.distance_xz(feet)
            );
            // The floor 0.15 m above the feet, level, below (a standing eye reaches 0.15 m down, a crouched 0.3 m).
            for floor in [0.15, 0.0, if crouched { -0.3 } else { -0.15 }] {
                let look = press_look(crouched, 1.1, -floor).expect("a look reaches 1.1 m");
                assert!(look.pitch.0 <= LOOK_DOWN && look.run_m >= 1.1 && look.run_m < 1.25, "{look:?}");
                let eye = eye_at(feet, crouched, 0.0, look.pitch.0);
                let at = landing(eye, yaw, look.pitch.view(), floor).expect("lands");
                assert!((at.distance_xz(feet) - look.run_m).abs() < 1e-3, "{at:?} {look:?}");
                assert!((eye.y - floor) / look.pitch.0.sin() <= PLACE_RAY_M - RAY_MARGIN_M + 1e-3);
                // Within the press slack it lands about where asked, farther if flatter.
                let flatter = look.pitch.0 - PRESS_SLACK;
                let slack = landing(eye_at(feet, crouched, 0.0, flatter), yaw, -flatter, floor);
                assert!(slack.is_some_and(|at| at.distance_xz(feet) >= look.run_m - 1e-3), "{slack:?}");
            }
        }
        // A distance no look reaches: the head ray of 2.0 m runs out before the floor; a floor too far below.
        assert_eq!(press_look(true, 1.8, 0.0), None);
        assert_eq!(press_look(false, 3.0, 0.0), None);
        assert_eq!(press_look(true, 1.1, 1.2), None, "a stair 1.2 m down is not reached from a crouch");
        assert_eq!(press_look(false, 1.1, 0.3), None, "nor a floor 0.3 m down from a standing eye: the ray runs out");
    }

    /// The guarantee of the press: whatever the pose, the speed, the yaw and the floor, a press made where
    /// [`press_of`] says `Ready` lands at least `press_landing_m` from the feet at the release, which is two inputs on with
    /// the pose held and the view at most the slack steeper (the aim ends the slack; 3 m/s, eight yaws).
    #[test]
    fn every_ready_press_lands_a_metre_from_the_feet_at_the_release() {
        let t = TrapTuning::default();
        let min_m = t.press_landing_m.expect("on by default");
        let mut presses = 0;
        for crouched in [false, true] {
            for floor in [0.2, 0.0, if crouched { -0.3 } else { -0.15 }] {
                for k in 0..8 {
                    let yaw = std::f32::consts::TAU * k as f32 / 8.0;
                    let d = dir(yaw);
                    let feet = Vec3::new(5.0, 0.0, 5.0);
                    let look = press_look(crouched, wanted_m(&t, min_m), -floor).expect("a look");
                    // The view comes to the press anywhere in the band round the aim.
                    for step in 0..=8 {
                        let down = look.pitch.0 - PRESS_SLACK + PRESS_SLACK * 2.0 * step as f32 / 8.0;
                        // The bot's check at the press tick: the feet two inputs on at 3 m/s, the view a slack steeper.
                        let lead = Vec3::new(3.0 * d[0] * RELEASE_LEAD_S, 0.0, 3.0 * d[1] * RELEASE_LEAD_S);
                        let released = feet + lead;
                        let checked = (down + PRESS_SLACK).min(LOOK_DOWN);
                        let landed = landing(eye_for(released, yaw, crouched, -checked), yaw, -checked, floor);
                        if press_of(&t, landed, released, true, Vec::new) != Press::Ready {
                            continue;
                        }
                        presses += 1;
                        // What happens: the view is anywhere from `down` to `down + slack`.
                        for extra in [0.0, PRESS_SLACK / 2.0, PRESS_SLACK] {
                            let at_release = (down + extra).min(LOOK_DOWN);
                            let at = landing(eye_for(released, yaw, crouched, -at_release), yaw, -at_release, floor)
                                .expect("lands");
                            assert!(
                                at.distance_xz(released) >= min_m - 1e-3,
                                "crouched {crouched} floor {floor} yaw {k}: {} m at {at_release}",
                                at.distance_xz(released)
                            );
                        }
                    }
                }
            }
        }
        assert!(presses > 100, "{presses} presses");
    }

    /// The reasons the press waits, in the order they are judged, and the landing rule's three: too near the feet, on the
    /// route ahead (unless the bot stays), nowhere.
    #[test]
    fn a_press_waits_for_the_landing_and_the_steepness_and_says_why() {
        let mut t = TrapTuning::default();
        let feet = Vec3::ZERO;
        let route = || -> Vec<Vec3> { (0..=12).map(|k| Vec3::new(k as f32, 0.0, 0.0)).collect() };
        let far = Some(Vec3::new(0.0, 0.0, 1.2));
        assert_eq!(press_of(&t, far, feet, false, route), Press::Ready);
        assert_eq!(press_of(&t, Some(Vec3::new(0.0, 0.0, 0.7)), feet, false, route), Press::Wait(Why::TooNear));
        assert_eq!(press_of(&t, None, feet, false, route), Press::Wait(Why::NoFloor));
        assert_eq!(
            press_of(&t, Some(Vec3::new(3.0, 0.0, 0.4)), Vec3::new(3.0, 0.0, 1.6), false, route),
            Press::Wait(Why::OnRoute)
        );
        // A bot that stays has no route to keep off (the route is not even read); the feet are still kept clear.
        let never = || -> Vec<Vec3> { panic!("a bot that stays reads no route") };
        assert_eq!(press_of(&t, Some(Vec3::new(3.0, 0.0, 1.2)), Vec3::new(3.0, 0.0, 0.0), true, never), Press::Ready);
        assert_eq!(press_of(&t, Some(Vec3::new(0.0, 0.0, 0.5)), feet, true, never), Press::Wait(Why::TooNear));
        // Off: whatever the landing.
        t.press_landing_m = None;
        assert_eq!(press_of(&t, None, feet, false, route), Press::Ready);
        // The step: on the yaw and the look but a bad landing, no key and the reason kept; a good one, the key.
        let looking = State::Looking { yaw: 1.0, spot: Vec3::ZERO, since: 1_000, last: None };
        let aim = |press| Aim::Look { pitch: Down(0.7), press };
        let wait = Tick { aim: aim(Press::Wait(Why::TooNear)), ..tick(1_100, 1.0, -0.7, true) };
        let (state, out) = step(looking, wait, &t);
        assert!(!out.key_down && matches!(state, State::Looking { last: Some(Why::TooNear), .. }));
        let go = Tick { aim: aim(Press::Ready), ..wait };
        let (down, out) = step(looking, go, &t);
        assert!(out.key_down && matches!(down, State::Down { crouched: false, .. }));
        // A flatter look than the aim minus the slack is not on, nor one steeper than the aim plus it (still slewing).
        for pitch in [-0.6, -0.76] {
            let (state, out) = step(looking, Tick { pitch, ..go }, &t);
            assert!(!out.key_down && matches!(state, State::Looking { last: Some(Why::Pitch), .. }), "{pitch}");
        }
        // Not on the yaw yet.
        let (state, _) = step(looking, Tick { yaw: 2.0, ..go }, &t);
        assert!(matches!(state, State::Looking { last: Some(Why::Yaw), .. }));
        // With the landing rule the pose of the press is held through the release; without it nothing is held.
        let on = TrapTuning::default();
        let (_, out) = step(looking, Tick { crouched: true, ..go }, &on);
        assert_eq!(out.hold_crouch, Some(true));
        let (_, out) = step(State::Down { yaw: 1.0, spot: Vec3::ZERO, crouched: true }, go, &on);
        assert_eq!((out.hold_crouch, out.ended), (Some(true), Some(Ended::Placed)));
        let off = TrapTuning { press_landing_m: None, ..TrapTuning::default() };
        let (_, out) = step(looking, Tick { aim: Aim::Steepest, pitch: -1.0, ..go }, &off);
        assert!(out.key_down && out.hold_crouch.is_none());
        // Every look ends once, with the last thing that kept it back.
        let stuck = State::Looking { yaw: 1.0, spot: Vec3::ZERO, since: 1_000, last: Some(Why::OnRoute) };
        let late = Tick { now: 1_000 + t.look_ms + 1, ..wait };
        let (state, out) = step(stuck, late, &t);
        assert_eq!(out.ended, Some(Ended::GaveUp(Some(Why::OnRoute))));
        assert!(matches!(state, State::Cooling { .. }));
        let (_, out) = step(looking, Tick { calm: false, ..go }, &t);
        assert_eq!(out.ended, Some(Ended::CalledAway));
        let mut outcomes = LookOutcomes::default();
        for ended in [Ended::Placed, Ended::CalledAway, Ended::GaveUp(Some(Why::TooNear)), Ended::GaveUp(None)] {
            outcomes.record(ended);
        }
        assert_eq!(outcomes.looks(), 4);
        assert!(outcomes.line().starts_with("looks 4: placed 1, called away 1, gave up 2 ("), "{}", outcomes.line());
    }

    #[test]
    fn the_candidates_lie_where_the_look_reaches_for_their_own_floor() {
        let s = Scenario::field(Mode::TeamDeathmatch);
        let tuning = TrapTuning::default();
        let feet = Vec3::new(30.0, 0.0, 10.0);
        let not_spawn = |_: Vec3| false;
        let lanes = lanes_at(Vec3::new(33.0, 0.0, 10.0), 6, 100_000);
        let wanted = wanted_m(&tuning, 1.0);
        let look = press_look(true, wanted, 0.0).unwrap();
        let chosen = choose(&Search {
            reach_m: look.run_m,
            press: Some((true, wanted)),
            ..search(&s, &tuning, feet, &lanes, &not_spawn)
        })
        .expect("a spot");
        assert!((chosen.spot.distance_xz(feet) - look.run_m).abs() < 1e-3, "{chosen:?}");
    }

    /// [A309] The forward rule: a spot nearer home than `min_share` of the way to the enemy's home is refused, one past it taken;
    /// without the rule or without homes the spot is the same as before.
    #[test]
    fn a_spot_short_of_the_middle_is_refused_with_the_forward_rule() {
        let (own, enemy) = (Vec3::new(0.0, 0.0, 10.0), Vec3::new(100.0, 0.0, 10.0));
        assert!((forward_share(own, enemy, Vec3::new(25.0, 0.0, 10.0)) - 0.25).abs() < 1e-4);
        assert!((forward_share(own, enemy, Vec3::new(50.0, 0.0, 10.0)) - 0.5).abs() < 1e-4);
        let s = Scenario::field(Mode::TeamDeathmatch);
        let mut tuning = t();
        let not_spawn = |_: Vec3| false;
        // The enemy's lane by the bot; the bot at a third of the way: no spot with the rule, the same spot without it.
        let feet = Vec3::new(30.0, 0.0, 10.0);
        let lanes = lanes_at(Vec3::new(33.0, 0.0, 10.0), 6, 100_000);
        let free = choose(&search(&s, &tuning, feet, &lanes, &not_spawn)).expect("a spot");
        tuning.forward = Some(crate::v026g::bots::tuning::Forward { min_share: 0.5, fallback_share: 0.5 });
        assert_eq!(choose(&search(&s, &tuning, feet, &lanes, &not_spawn)), Some(free), "no homes: as before");
        let short = choose(&Search {
            homes: Some((Vec3::new(0.0, 0.0, 10.0), Vec3::new(100.0, 0.0, 10.0), 0.5)),
            ..search(&s, &tuning, feet, &lanes, &not_spawn)
        });
        assert!(short.is_none(), "{short:?}");
        // Past the middle: the spot is taken.
        let feet = Vec3::new(60.0, 0.0, 10.0);
        let lanes = lanes_at(Vec3::new(63.0, 0.0, 10.0), 6, 100_000);
        let past = choose(&Search {
            homes: Some((Vec3::new(0.0, 0.0, 10.0), Vec3::new(100.0, 0.0, 10.0), 0.5)),
            ..search(&s, &tuning, feet, &lanes, &not_spawn)
        });
        assert!(past.is_some_and(|c| forward_share(Vec3::new(0.0, 0.0, 10.0), Vec3::new(100.0, 0.0, 10.0), c.spot) >= 0.5), "{past:?}");
    }

    /// A292 S1: with `lanes: enemies` the trap map takes every enemy the team sensed itself, bot or human; a mate, a cheat or a
    /// report too vague for a cell does not count, and the humans' lanes are not touched.
    #[test]
    fn the_trap_lanes_take_every_sensed_enemy() {
        use crate::v026g::bots::know::{CheatKind, Source};
        let mut s = Scenario::field(Mode::TeamDeathmatch);
        s.bot(0, 0, Vec3::new(10.0, 0.0, 10.0));
        s.bot(1, 1, Vec3::new(20.0, 0.0, 10.0));
        s.human(2, 1, Vec3::new(30.0, 0.0, 10.0));
        s.bot(3, 0, Vec3::new(40.0, 0.0, 10.0));
        let track = |who, x: f32, source, error_m| Track {
            who,
            at: Vec3::new(x, 0.0, 10.0),
            error_m,
            time: 1_000,
            source,
            sight_since: None,
            notice_ms: 0,
            own: None,
            missed: None,
            held_until: 0,
        };
        let tracks = [
            track(1, 20.0, Source::Seen, 0.5),
            track(2, 30.0, Source::Heard, 2.0),
            track(3, 40.0, Source::Seen, 0.5),
            track(1, 90.0, Source::Cheat(CheatKind::Radar), 0.0),
            track(2, 70.0, Source::Heard, 20.0),
        ];
        let mut lanes = HumanLanes::default();
        learn_enemy_lanes(&mut lanes, 0, &s.view, tracks.iter());
        let mut who: Vec<_> = lanes.humans.iter().map(|h| h.who).collect();
        who.sort_unstable();
        assert_eq!(who, vec![1, 2], "{lanes:?}");
        assert!(lanes.humans.iter().all(|h| h.cells.len() == 1), "{lanes:?}");
        assert!(presence(&lanes, Vec3::new(21.0, 0.0, 10.0), 1_000) > 0.5);
        assert_eq!(presence(&lanes, Vec3::new(90.0, 0.0, 10.0), 1_000), 0.0);
        // [fix-team PvB traps] The default reads the humans' lanes for a team facing a human, the trap map for one facing bots
        // only; `humans` and `enemies` read theirs whatever it faces.
        use crate::v026g::bots::tuning::TrapLanes;
        let mut tuning = crate::v026g::bots::tuning::Tuning::default().traps;
        assert_eq!(tuning.lanes, TrapLanes::Facing);
        assert_eq!((tuning.lanes.source(true), tuning.lanes.source(false)), (LaneSource::Humans, LaneSource::Enemies));
        assert_eq!(TrapLanes::Humans.source(false), LaneSource::Humans);
        assert_eq!(TrapLanes::Enemies.source(true), LaneSource::Enemies);
        tuning.lanes = TrapLanes::Enemies;
        assert!(tuning.check().is_ok());
    }

    /// A292 S2: `sensed_near` keeps a far enemy sensed lately from the enemy gate; a near one, or the default, still stops it.
    #[test]
    fn sensed_near_lets_a_far_sound_pass_the_gate() {
        use crate::v026g::bots::know::Source;
        use crate::v026g::bots::tuning::SensedNear;
        let mut tuning = crate::v026g::bots::tuning::Tuning::default().traps;
        assert_eq!(tuning.sensed_near, Some(SensedNear::default()), "on by default (A292)");
        tuning.sensed_near = None;
        let quiet = (tuning.enemy_quiet_s * 1000.0) as u32;
        let at = |x: f32| Track {
            who: 1,
            at: Vec3::new(x, 0.0, 0.0),
            error_m: 5.0,
            time: 99_000,
            source: Source::Heard,
            sight_since: None,
            notice_ms: 0,
            own: None,
            missed: None,
            held_until: 0,
        };
        let lately = |tuning: &TrapTuning, x: f32| sensed_lately(tuning, &at(x), Vec3::ZERO, 100_000, quiet);
        assert!(lately(&tuning, 150.0), "the old gate: at any distance");
        assert!(!sensed_lately(&tuning, &at(150.0), Vec3::ZERO, 99_000 + quiet, quiet), "not lately");
        tuning.sensed_near = Some(SensedNear::default());
        assert!(!lately(&tuning, 150.0));
        assert!(lately(&tuning, 30.0));
        assert!(tuning.check().is_ok());
        tuning.sensed_near = Some(SensedNear { within_m: 0.0 });
        assert!(tuning.check().is_err(), "no range");
    }

    /// A292 S4a: with `after_press` the legs stay from the press until the next think takes the trap (`placed`), at most
    /// `hold_ms`; without it they never do.
    #[test]
    fn the_placer_stands_after_its_press_until_its_next_think() {
        use crate::v026g::bots::tuning::AfterPress;
        let full = crate::v026g::bots::tuning::Tuning::default();
        let mut bot = Bot::new(1, 0, 0, None, &full);
        let mut tuning = t();
        bot.features.traps.pressed_at = Some(10_000);
        assert!(!held_after_press(&bot, 10_100, &tuning), "off: no hold");
        tuning.after_press = Some(AfterPress::default());
        assert!(held_after_press(&bot, 10_100, &tuning));
        assert!(held_after_press(&bot, 11_400, &tuning), "past the next think: the reflex's legs wait too");
        assert!(!held_after_press(&bot, 10_000 + 1_501, &tuning), "the cap");
        bot.features.traps.pressed_at = None;
        assert!(!held_after_press(&bot, 10_100, &tuning), "never pressed");
        assert!(tuning.check().is_ok());
    }

    /// A324: the client's trap is armed at once and takes its placer's foot within 0.6 m, so the press waits while the landing
    /// lies on the body's own way over the next second (`press_clear`), as the walker caught 0.3 s after its press in the F2 run
    /// (2.3 m/s, its landing about 1.1 m ahead); a landing aside or behind, or a standing body, presses.
    #[test]
    fn the_press_waits_while_the_landing_lies_on_the_bodys_own_way() {
        let clear = t().press_clear;
        assert!(clear.is_some(), "on by default");
        let feet = Vec3::new(0.0, 0.0, 0.0);
        let walking = [2.3, 0.0];
        let none = Vec::new;
        assert!(!clears_own_path(clear, feet, walking, Vec3::new(1.1, 0.0, 0.1), none), "straight ahead");
        assert!(!clears_own_path(clear, feet, walking, Vec3::new(0.8, 0.0, 0.8), none), "45 deg ahead: 0.8 m off the way");
        assert!(clears_own_path(clear, feet, walking, Vec3::new(0.3, 0.0, 1.1), none), "aside");
        assert!(clears_own_path(clear, feet, walking, Vec3::new(-1.1, 0.0, 0.2), none), "behind");
        assert!(clears_own_path(clear, feet, [0.0, 0.0], Vec3::new(1.1, 0.0, 0.0), none), "standing");
        assert!(clears_own_path(None, feet, walking, Vec3::new(1.1, 0.0, 0.1), none), "off: the old press");
        // A route that turns: 1 m on along +x, then towards +z, where the trap lies aside of the straight line.
        let turning = || vec![Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 1.0), Vec3::new(1.0, 0.0, 2.0)];
        let aside = Vec3::new(0.3, 0.0, 1.2);
        assert!(clears_own_path(clear, feet, walking, aside, none), "clear of the straight line");
        assert!(!clears_own_path(clear, feet, walking, aside, turning), "on the route's turn");
        let far_on = || vec![Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0), Vec3::new(2.0, 0.0, 0.0), Vec3::new(3.0, 0.0, 0.0), Vec3::new(4.0, 0.0, 0.0), Vec3::new(5.0, 0.0, 0.0), Vec3::new(6.0, 0.0, 1.2)];
        assert!(clears_own_path(clear, feet, walking, Vec3::new(6.0, 0.0, 1.2), far_on), "past the second's reach");
        let mut tuning = t();
        assert!(tuning.check().is_ok());
        tuning.press_clear = Some(PressClear { clear_m: 0.5, sweep_s: 1.0 });
        assert!(tuning.check().is_err(), "inside the client's trigger");
    }

    /// A292 S4a: the swept path is the body's own, from its feet along its velocity for `sweep_s`.
    #[test]
    fn the_swept_path_is_the_feet_along_the_velocity() {
        let feet = Vec3::new(0.0, 0.0, 0.0);
        // Walking along x at 3 m/s: a trap 2 m ahead and 0.5 m aside lies 0.5 m off the next second's path.
        assert!((swept_m(feet, [3.0, 0.0], 1.0, Vec3::new(2.0, 0.0, 0.5)) - 0.5).abs() < 1e-4);
        // Behind the feet: its distance to the feet.
        assert!((swept_m(feet, [3.0, 0.0], 1.0, Vec3::new(-1.0, 0.0, 0.0)) - 1.0).abs() < 1e-4);
        // Standing: the feet alone.
        assert!((swept_m(feet, [0.0, 0.0], 1.0, Vec3::new(0.0, 0.0, 1.2)) - 1.2).abs() < 1e-4);
    }

    /// [A324] The client's rule for where a released trap lies: on flat ground the head ray's hit; on a slope steeper than
    /// 30 deg, on a known trap's sensor, or with no ground within 2.0 m, nothing (and why).
    #[test]
    fn a_trap_lies_where_the_client_puts_it_or_nowhere() {
        let quad = |y0: f32, y1: f32| {
            let (a, b, c, d) =
                (Vec3::new(-10.0, y0, -10.0), Vec3::new(10.0, y1, -10.0), Vec3::new(10.0, y1, 10.0), Vec3::new(-10.0, y0, 10.0));
            [[a, b, c], [a, c, d]]
        };
        let flat = mapdata::RayTree::from_corners(quad(0.0, 0.0));
        let eye = Vec3::new(0.0, 1.6, 0.0);
        // 1.0 rad down from 1.6 m: the ray meets the floor 1.90 m out along it (0.9 rad would need 2.04 m, past the reach).
        let pitch = -1.0;
        let at = client_landing(&flat, eye, 0.0, pitch, &[]).expect("flat ground within reach");
        assert!(at.y.abs() < 1e-3, "{at:?}");
        let run = 1.6 / pitch.tan().abs();
        let d = dir(0.0);
        assert!(at.distance_xz(Vec3::new(d[0] * run, 0.0, d[1] * run)) < 1e-3, "{at:?}");
        // The old plane guess lands at the same point on flat ground.
        let plane = landing(eye, 0.0, pitch, 0.0).unwrap();
        assert!(plane.distance_xz(at) < 1e-3);
        // A slope of 45 deg (rise 20 m over 20 m): refused; one of 20 deg: placed.
        assert_eq!(client_landing(&mapdata::RayTree::from_corners(quad(-10.0, 10.0)), eye, 0.0, -1.2, &[]), Err(Why::Slope));
        let gentle = 20.0 * 20f32.to_radians().tan();
        assert!(client_landing(&mapdata::RayTree::from_corners(quad(-gentle / 2.0, gentle / 2.0)), eye, 0.0, -1.2, &[]).is_ok());
        // On an armed trap's sensor: refused; 0.7 m off it: placed.
        assert_eq!(client_landing(&flat, eye, 0.0, pitch, &[at + Vec3::new(0.3, 0.0, 0.0)]), Err(Why::OnTrap));
        assert!(client_landing(&flat, eye, 0.0, pitch, &[at + Vec3::new(0.0, 0.0, 0.7)]).is_ok());
        // A look too flat for the 2.0 m ray to reach the floor: nothing.
        assert_eq!(client_landing(&flat, eye, 0.0, -0.3, &[]), Err(Why::NoFloor));
        assert_eq!(landing(eye, 0.0, -0.3, 0.0), None);
    }

    /// [A324, from fx-gear-2's A318] The way back to a spot: on an open field the way from home is about straight, so the points
    /// lie 6, 9, ... 18 m before the spot, on the side of home; a way shorter than `near_m` gives none.
    #[test]
    fn the_way_back_to_a_spot_is_the_stretch_before_it() {
        let s = Scenario::field(Mode::TeamDeathmatch);
        let j = crate::v026g::bots::tuning::CamperJob::default();
        let (home, spot) = (Vec3::new(10.0, 0.0, 10.0), Vec3::new(50.0, 0.0, 10.0));
        let points = approach_points(&s.tactics, home, spot, &j);
        assert_eq!(points.len(), ((j.far_m - j.near_m) / j.step_m) as usize + 1, "{points:?}");
        // The way is straight for its last 18 m (round the wall's end at z 25 before that): there each point lies its metres back.
        for (k, at) in points.iter().enumerate().filter(|&(k, _)| j.near_m + j.step_m * k as f32 <= 18.0) {
            let d = at.distance_xz(spot);
            let want = j.near_m + j.step_m * k as f32;
            assert!((d - want).abs() < 1.5, "point {k} {at:?}: {d} m from the spot, want about {want}");
            assert!(at.x < spot.x, "on home's side: {at:?}");
        }
        assert!(approach_points(&s.tactics, Vec3::new(46.0, 0.0, 10.0), spot, &j).is_empty());
    }

    /// [bot traps] A held place in a zone on all match, one on no floor: told why at once, with no route search; one with a way
    /// gives its way.
    #[test]
    fn a_held_place_with_no_way_is_told_why_with_no_search() {
        let world = crate::v026g::bots::tactics::tests::field_with(|map| {
            map.zones.push(mapdata::Zone {
                kind: mapdata::ZoneKind::Damage,
                name: "core".into(),
                team: -1,
                position: Vec3::new(20.0, 1.0, 33.0),
                protection: None,
                damage_type: "electric".into(),
                damage: None,
                shapes: vec![mapdata::Shape {
                    kind: mapdata::ShapeKind::Box,
                    center: Vec3::new(20.0, 1.0, 33.0),
                    rotation: mapdata::Rotation::IDENTITY,
                    half: Vec3::new(3.0, 1.0, 3.0),
                }],
            });
        });
        let tactics = crate::v026g::bots::tactics::Tactics::new(world, 1.0);
        let home = Vec3::new(75.0, 0.0, 10.0);
        let searches = super::super::path::searches().searches;
        assert_eq!(approach_way(&tactics, home, Vec3::new(20.0, 0.0, 33.0)).err(), Some(NoWayIn::BehindFixedZone));
        assert_eq!(approach_way(&tactics, home, Vec3::new(1_000.0, 0.0, 1_000.0)).err(), Some(NoWayIn::NoFloor));
        assert_eq!(super::super::path::searches().searches, searches, "no route search");
        assert!(approach_way(&tactics, home, Vec3::new(30.0, 0.0, 35.0)).is_ok());
        assert_eq!(NoWayIn::BehindFixedZone.words(), "his place is on or behind a zone on all match");
    }

    /// [A324] The camper job's points: a K2 job's spot and a battery hold each get their way in, out of his sight (here past
    /// the field's wall end, the way from the one spawn kept); [bot traps T1] when the holder dies the point is kept past its
    /// hold (`threat::CAMP_KILLS_KEPT_MS`) and then goes; a second noting within `near_own_m` of a held one makes no second
    /// point. Every step is a note.
    #[test]
    fn the_camper_job_traps_the_way_in_of_held_places_while_they_are_held() {
        use crate::v026g::bots::know::{CamperTrapKind, CamperTrapState, TeamKnowledge};
        use crate::v026g::bots::threat::HeldLine;
        let world = crate::v026g::bots::tactics::tests::field_with(|map| map.spawns.retain(|s| s.team == 0));
        let tactics = crate::v026g::bots::tactics::Tactics::new(world, 1.0);
        let mut s = Scenario::field(Mode::TeamDeathmatch);
        let held = Vec3::new(42.0, 0.0, 10.0);
        s.human(11, 1, held);
        s.human(12, 1, Vec3::new(60.0, 0.0, 30.0));
        let t = TrapTuning::default();
        let j = t.camper_job.unwrap();
        let line = HeldLine::for_test(7, 11, held, &[], Some(1_000));
        let mut battery = HeldLine::for_test(crate::v026g::bots::threat::BATTERY_LINE_ID + 12, 12, Vec3::new(60.0, 0.0, 30.0), &[], Some(1_000));
        battery.battery = Some(2);
        let mut k = TeamKnowledge::default();
        let mut notes = Vec::new();
        let tick = |k: &mut TeamKnowledge, t: &TrapTuning, view: &crate::v026g::bots::view::MatchView, sources: &[HeldLine], now: u32, notes: &mut Vec<super::super::Note>| {
            camper_tick(&[], k, view, &tactics, t, 0, sources, now, notes)
        };
        tick(&mut k, &t, &s.view, &[line.clone(), battery.clone()], 1_000, &mut notes);
        let point = *k.camper_traps.iter().find(|c| c.spot == 7).expect("the job's way in");
        assert_eq!(point.kind, CamperTrapKind::Job);
        assert!(out_of_his_sight(&tactics.world.sight, held, point.at), "{point:?}");
        let d = point.at.distance_xz(held);
        assert!(d >= j.near_m - 1.0, "{d} m from his place");
        assert!(notes.iter().any(|n| n.text.contains("his way in at")), "{notes:?}");
        // A trap of the team on the point: trapped, and tick after tick not searched again, nor sent, nor dropped.
        let before = camper::counted();
        k.traps.promise(3, point.at, 1_000);
        for now in [1_500, 2_000, 2_500, 3_000] {
            tick(&mut k, &t, &s.view, &[line.clone(), battery.clone()], now, &mut notes);
            assert_eq!(k.camper_traps.iter().find(|c| c.spot == 7).map(|c| c.state), Some(CamperTrapState::Trapped), "at {now}");
        }
        let after = camper::counted();
        for kind in [camper::NOTED_JOB, camper::NO_WAY, camper::SENT, camper::DROPPED] {
            assert_eq!(after[0][kind], before[0][kind], "{}", camper::NAMES[kind]);
        }
        assert_eq!(notes.iter().filter(|n| n.text.contains("trapped while it stands")).count(), 1, "{notes:?}");
        // The battery's man stands in the open field's east half, seen from all of his way from the west spawn: no way in.
        assert!(k.camper_no_way.contains(&battery.id) || k.camper_traps.iter().any(|c| c.kind == CamperTrapKind::Battery));
        // Another line held near his place finds the same way in: no second point.
        let near = HeldLine::for_test(8, 13, held + Vec3::new(0.5, 0.0, 0.0), &[], Some(1_000));
        tick(&mut k, &t, &s.view, &[line.clone(), battery.clone(), near.clone()], 3_200, &mut notes);
        assert!(k.camper_traps.iter().all(|c| c.spot != 8), "{:?}", k.camper_traps);
        assert!(k.camper_no_way.contains(&8));
        assert!(notes.iter().any(|n| n.text.contains("is 7's point")), "{notes:?}");
        // He dies: the point is kept past its hold, then goes.
        s.view.places.iter_mut().find(|p| p.place == 11).unwrap().alive = false;
        tick(&mut k, &t, &s.view, &[battery.clone()], 4_000, &mut notes);
        let kept = k.camper_traps.iter().find(|c| c.spot == 7).expect("kept past its hold");
        assert_eq!(kept.hold, crate::v026g::bots::know::CamperHold::Over { since: 4_000 });
        assert!(notes.iter().any(|n| n.text.contains("no longer held; the point is kept")), "{notes:?}");
        let gone = 4_001 + crate::v026g::bots::threat::CAMP_KILLS_KEPT_MS;
        tick(&mut k, &t, &s.view, &[battery.clone()], gone, &mut notes);
        assert!(k.camper_traps.iter().all(|c| c.spot != 7), "{:?}", k.camper_traps);
        assert!(notes.iter().any(|n| n.text.contains("dropped")), "{notes:?}");
        let at = gone + 1;
        // Off: nothing.
        tick(&mut k, &TrapTuning { camper_job: None, ..t.clone() }, &s.view, std::slice::from_ref(&line), at + 1, &mut notes);
        assert!(k.camper_traps.is_empty());
    }

    /// [#2081] A sent bot that runs another play than the job's past its first thinks is let go (the point is open again); within
    /// them it stays sent. A walk longer than the hold's typical remaining time is never sent (`max_walk_m` 22.75 m). [bot traps
    /// T1] At its place's hold end a sent bot is let go, and nobody is sent to the kept point.
    #[test]
    fn the_camper_job_lets_go_a_bot_that_left_its_play() {
        use crate::v026g::bots::know::{CamperTrap, CamperTrapKind, CamperTrapState, SeenRefusals, TeamKnowledge};
        use crate::v026g::bots::threat::HeldLine;
        let t = TrapTuning::default();
        assert_eq!(t.camper_job.unwrap().max_walk_m, 22.75);
        let mut s = Scenario::field(Mode::TeamDeathmatch);
        s.bot(0, 0, Vec3::new(15.0, 0.0, 10.0));
        let held = Vec3::new(42.0, 0.0, 10.0);
        s.human(11, 1, held);
        let line = HeldLine::for_test(7, 11, held, &[], Some(1_000));
        let tuning = crate::v026g::bots::tuning::Tuning::all_off();
        let bots = vec![Some(Bot::new(1, 0, 0, None, &tuning))];
        assert!(bots[0].as_ref().unwrap().assignment.is_none(), "on no play");
        let mut k = TeamKnowledge::default();
        k.camper_traps.push(CamperTrap {
            spot: 7,
            shooter: 11,
            held,
            at: Vec3::new(30.0, 0.0, 10.0),
            kind: CamperTrapKind::Job,
            state: CamperTrapState::Sent { placer: 0, since: 1_000, placed_before: 0 },
            hold: crate::v026g::bots::know::CamperHold::Held,
            seen: SeenRefusals::default(),
        });
        let mut notes = Vec::new();
        camper_tick(&bots, &mut k, &s.view, &s.tactics, &t, 0, std::slice::from_ref(&line), 1_000 + LEFT_PLAY_GRACE_MS, &mut notes);
        assert!(matches!(k.camper_traps[0].state, CamperTrapState::Sent { .. }), "within its first thinks");
        camper_tick(&bots, &mut k, &s.view, &s.tactics, &t, 0, std::slice::from_ref(&line), 1_001 + LEFT_PLAY_GRACE_MS, &mut notes);
        assert_eq!(k.camper_traps[0].state, CamperTrapState::Open);
        assert!(notes.iter().any(|n| n.text.contains("it left the job's play")), "{notes:?}");
        // [bot traps T1] Sent again; its place no longer held: let go (no point sent to it, its play ends), the point kept open
        // and sent to nobody.
        k.camper_traps[0].state = CamperTrapState::Sent { placer: 0, since: 2_000, placed_before: 0 };
        camper_tick(&bots, &mut k, &s.view, &s.tactics, &t, 0, &[], 2_100, &mut notes);
        assert_eq!(k.camper_traps[0].state, CamperTrapState::Open, "let go at the hold's end");
        assert!(camper_point(&k, 0).is_none());
        assert!(notes.iter().any(|n| n.text.contains("bot 0 is let go")), "{notes:?}");
        camper_tick(&bots, &mut k, &s.view, &s.tactics, &t, 0, &[], 2_200, &mut notes);
        assert_eq!(k.camper_traps[0].state, CamperTrapState::Open, "no bot is sent to a kept point");
    }

    /// [bot traps T1] A bot not sent whose route ahead passes an open camper point wishes it (`passed_points`); one whose route
    /// keeps off it does not; a trapped point is not wished.
    #[test]
    fn a_bot_whose_walk_passes_an_open_camper_point_wishes_it() {
        use crate::v026g::bots::know::{CamperHold, CamperTrap, CamperTrapKind, CamperTrapState, SeenRefusals};
        let t = TrapTuning::default();
        let mut s = Scenario::field(Mode::TeamDeathmatch);
        s.bot(0, 0, Vec3::new(10.0, 0.0, 30.0));
        let tuning = crate::v026g::bots::tuning::Tuning::all_off();
        let mut bot = Bot::new(1, 0, 0, None, &tuning);
        let me = s.view.places[0].clone();
        let route = s.tactics.route(me.feet, Vec3::new(35.0, 0.0, 30.0), 0, Some(0)).expect("a route");
        bot.route = Some((route, Vec3::new(35.0, 0.0, 30.0)));
        let mut state = crate::v026g::bots::know::TeamKnowledge::default();
        let point = |at: Vec3, state: CamperTrapState| CamperTrap {
            spot: 7,
            shooter: 11,
            held: Vec3::new(45.0, 0.0, 30.0),
            at,
            kind: CamperTrapKind::Battery,
            state,
            hold: CamperHold::Over { since: 0 },
            seen: SeenRefusals::default(),
        };
        let on = Vec3::new(15.0, 0.0, 30.0);
        state.camper_traps.push(point(on, CamperTrapState::Open));
        assert_eq!(passed_points(&bot, &me, &t, &state), vec![on]);
        state.camper_traps[0].state = CamperTrapState::Trapped;
        assert!(passed_points(&bot, &me, &t, &state).is_empty(), "trapped");
        state.camper_traps[0] = point(Vec3::new(15.0, 0.0, 5.0), CamperTrapState::Open);
        assert!(passed_points(&bot, &me, &t, &state).is_empty(), "off its route");
    }

    /// [traps.look_over_heard] The trap look survives a glance and a sights-up aim at an enemy only heard or told of while the
    /// gate's enemy-near check is clear; a seen target, a hit or near shot (a threat look), fire, legs, a use, an aim that is no
    /// heard aim (a partner's death spot) or an enemy near take the view back at once; the switch off gives the old calm.
    #[test]
    fn the_trap_look_survives_a_heard_aim_and_a_glance_and_a_seen_target_or_a_hit_takes_it_back() {
        use crate::v026g::bots::reflex::Override;
        let t = TrapTuning::default();
        assert!(t.look_over_heard);
        let feet = Vec3::ZERO;
        let heard = Override { look_at: Some(Vec3::new(30.0, 1.3, 0.0)), heard_aim: Some((Vec3::new(30.0, 0.0, 0.0), 1_000)), ..Override::default() };
        let glance = Override { look_at: Some(Vec3::new(0.0, 1.4, 25.0)), glance: true, ..Override::default() };
        let facts = |over: &Override| CalmFacts { over: Box::leak(Box::new(*over)), target: false, threat: false, using: false, enemy_near: false, feet, now: 4_000 };
        assert_eq!(trap_calm(facts(&heard), &t), (true, Some(OverHeard::Aim { m: 30.0, age_ms: 3_000 })));
        assert_eq!(trap_calm(facts(&glance), &t), (true, Some(OverHeard::Glance { m: 25.0 })));
        assert_eq!(trap_calm(facts(&Override::default()), &t), (true, None), "no look: calm as before");
        // What still wins at once, mid-look.
        for (why, f) in [
            ("a seen target", CalmFacts { target: true, ..facts(&heard) }),
            ("a threat look", CalmFacts { threat: true, ..facts(&glance) }),
            ("an enemy near", CalmFacts { enemy_near: true, ..facts(&heard) }),
            ("a use", CalmFacts { using: true, ..facts(&glance) }),
            ("fire", facts(&Override { fire: true, ..heard })),
            ("legs", facts(&Override { legs: Some(crate::v026g::bots::reflex::Legs { at: Vec3::ZERO, within_m: 1.0, run: false, crouch: false, why: crate::v026g::bots::reflex::LegsWhy::BreakSight }), ..glance })),
            ("a watch, no heard aim", facts(&Override { heard_aim: None, keys: crate::v026g::drive::key::AIM, ..heard })),
        ] {
            assert_eq!(trap_calm(f, &t), (false, None), "{why}");
        }
        let off = TrapTuning { look_over_heard: false, ..t.clone() };
        assert_eq!(trap_calm(facts(&heard), &off), (false, None));
        assert_eq!(trap_calm(facts(&glance), &off), (false, None));
        // A look not calm is called away at once (the habit's step).
        let looking = State::Looking { yaw: 0.0, spot: Vec3::ZERO, since: 0, last: None };
        let (_, out) = step(looking, Tick { now: 100, yaw: 0.0, pitch: 0.0, calm: false, crouched: false, aim: Aim::Steepest }, &t);
        assert_eq!(out.ended, Some(Ended::CalledAway));
    }

    /// A floor with a wall across x = 25 from z = 5 to 15, `high` metres tall, between a man on (30, 0, 10) and a point at
    /// (24, 0, 10).
    fn low_wall(high: f32) -> mapdata::RayTree {
        low_wall_to(high, 15.0)
    }

    /// [`low_wall`] ending at `z1` (from z = 5).
    fn low_wall_to(high: f32, z1: f32) -> mapdata::RayTree {
        use crate::v026g::bots::body::tests::floor;
        let walker = mapdata::WalkerBehaviour::BySlope;
        let (a, b, c, d) = (Vec3::new(25.0, 0.0, 5.0), Vec3::new(25.0, 0.0, z1), Vec3::new(25.0, high, z1), Vec3::new(25.0, high, 5.0));
        let mut triangles = floor(0.0, 0.0, 40.0, 20.0, 0.0).to_vec();
        triangles.push(mapdata::Triangle { v: [a, b, c], walker });
        triangles.push(mapdata::Triangle { v: [a, c, d], walker });
        mapdata::RayTree::build(&triangles)
    }

    /// [A324] The way-in point is out of his sight both ways, by the parts a bot's sight looks at: a 1.0 m wall 1 m in front
    /// of the point hides a 0.9 m chest from a held spot's eye 6 m off (the old test took the point) but not the crouched
    /// placer's upper body, nor him from its eye; a 1.3 m wall hides them from each other; a 5 cm kerb hides nothing. One
    /// part showing is enough: a 1.3 m wall that ends beside the line (here 0.1 m off his chest's line) does not hide him.
    #[test]
    fn a_way_in_point_is_out_of_his_sight_both_ways_by_the_sense_s_body_parts() {
        use crate::v026g::bots::aimpoint::body_shows;
        let (held, point) = (Vec3::new(30.0, 0.0, 10.0), Vec3::new(24.0, 0.0, 10.0));
        let eye = crate::v026g::bots::lane_ahead::camper_eye(held);
        let one = low_wall(1.0);
        assert!(!crate::v026g::bots::rays::visible(&one, eye, point + Vec3::new(0.0, 0.9, 0.0)), "the chest is hidden");
        assert!(body_shows(&one, eye, point, true), "the crouched body is not");
        assert!(!out_of_his_sight(&one, held, point));
        assert!(out_of_his_sight(&low_wall(1.3), held, point));
        assert!(!out_of_his_sight(&low_wall(0.05), held, point), "a 5 cm kerb hides nothing");
        assert!(!out_of_his_sight(&low_wall_to(1.3, 10.1), held, point), "an edge beside the line hides one part, not all");
    }

    /// [A324] The `TrapSpot` play crouches where that hides it: behind a 1.3 m wall (standing, its upper body shows over it;
    /// crouched, neither sees the other); not behind a 1.0 m one (crouched it is still seen), nor a 3 m one (hidden standing too).
    #[test]
    fn crouching_hides_the_placer_only_where_its_standing_eye_shows_and_its_crouched_one_not() {
        let (held, point) = (Vec3::new(30.0, 0.0, 10.0), Vec3::new(24.0, 0.0, 10.0));
        assert!(hidden_by_crouching(&low_wall(1.3), held, point));
        assert!(!hidden_by_crouching(&low_wall(1.0), held, point));
        assert!(!hidden_by_crouching(&low_wall(3.0), held, point));
    }
}

/// [keep_off] Bench counts (`tests::tallies::route`): the route searches made with `traps.keep_off` on, and the routes refused for
/// an own trap ([`own_trap_route`], #2081: the narrow fallback search is gone).
#[cfg(test)]
pub mod keep_off_counts {
    use std::sync::atomic::{AtomicU64, Ordering};

    pub const SEARCHES: usize = 0;
    pub const REFUSED: usize = 1;

    static COUNTS: [AtomicU64; 2] = [const { AtomicU64::new(0) }; 2];

    pub fn count(kind: usize) {
        COUNTS[kind].fetch_add(1, Ordering::Relaxed);
    }

    pub fn counts() -> [u64; 2] {
        std::array::from_fn(|k| COUNTS[k].load(Ordering::Relaxed))
    }
}
