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
use super::know::HumanLanes;
use super::plays::PlayKind;
use super::route_end::RouteEnd;
use super::route_filter::RouteFilter;
use super::think::Due;
use super::trap_know::TrapField;
use super::tuning::TrapTuning;
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
}

impl Why {
    pub const ALL: [Self; 6] = [Self::Yaw, Self::Pitch, Self::TooNear, Self::OnRoute, Self::NoFloor, Self::NoLook];

    pub const fn index(self) -> usize {
        match self {
            Self::Yaw => 0,
            Self::Pitch => 1,
            Self::TooNear => 2,
            Self::OnRoute => 3,
            Self::NoFloor => 4,
            Self::NoLook => 5,
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
    placed_life: u8,
    state: State,
    /// Traps let go of in all its lives, and lives that ended while it was placing one (looking or pressing).
    placed_total: u32,
    died_placing: u32,
    outcomes: LookOutcomes,
}

/// How the looks ended, over all the lives of a habit (or summed over habits): one outcome for every look, the last
/// thing that kept the press back for the ones that gave up.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LookOutcomes {
    pub placed: u32,
    pub called_away: u32,
    /// Gave up, by the last [`Why`] ([`Why::index`]).
    pub gave_up: [u32; 6],
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
}

impl PlaceCounts {
    pub fn add(&mut self, other: &PlaceCounts) {
        self.placed += other.placed;
        self.died_placing += other.died_placing;
        self.armed += other.armed;
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
            placed_life: 0,
            state: State::Idle,
            placed_total: 0,
            died_placing: 0,
            outcomes: LookOutcomes::default(),
        }
    }

    /// A bot told it carries `kit`.
    pub fn carry(kit: TrapKit) -> Self {
        Self {
            kit: Some(kit),
            left: kit.count,
            placed_life: 0,
            state: State::Idle,
            placed_total: 0,
            died_placing: 0,
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
        }
    }

    /// A new life: the pocket is full again, nothing placed.
    pub fn new_life(&mut self) {
        // A life that ended in the middle of placing one (looking down, or the key down): the harm count.
        if !matches!(self.state, State::Idle) {
            self.died_placing += 1;
        }
        self.left = self.kit.map_or(0, |kit| kit.count);
        self.placed_life = 0;
        self.state = State::Idle;
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
}

/// [A145] The spots of killed holders (`know::Defeated`) a search wants a trap on (`TrapTuning::at_defeated`).
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
            #[cfg(test)]
            {
                gates::count(gates::R_ROUTE);
                if wished {
                    defeated::count(defeated::R_ROUTE);
                }
            }
            continue;
        }
        // Where its own body will be: it keeps off the disc.
        if s.feet.distance_xz(spot) < AVOID_M - 0.2 {
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
            #[cfg(test)]
            {
                gates::count(gates::R_FRIEND);
                if wished {
                    defeated::count(defeated::R_FRIEND);
                }
            }
            continue;
        }
        let value = score(presence(s.lanes, spot, s.now), friends, t);
        if value < t.min_score && !wished {
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

/// The eye a body at `feet` has with the view at `view_pitch` (up positive), crouched or standing, the crouched one at
/// the height of holding still (`view::eye_height`): the lower, so a trap lands at least as far as counted on.
fn eye_for(feet: Vec3, crouched: bool, view_pitch: f32) -> Vec3 {
    feet + Vec3::new(0.0, super::view::eye_height(crouched, 0.0, view_pitch), 0.0)
}

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
fn wanted_m(t: &TrapTuning, min_m: f32) -> f32 {
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
    pub(super) fn refresh_traps(&mut self, grid: &WalkGrid, ring_m: f32) -> bool {
        let version = self.knowledge.traps.version();
        if version == self.features.traps.field_version {
            return false;
        }
        self.features.traps.field = Arc::new(TrapField::of_ringed(&self.knowledge.traps, grid, ring_m));
        self.features.traps.field_version = version;
        true
    }
}

/// How far ahead a route is kept off the traps' discs, metres.
const KEEP_OFF_M: f32 = 15.0;

/// A route that runs over a trap's disc within [`KEEP_OFF_M`] is dropped, to
/// be searched again round it (the searches shut the floors of the discs,
/// [`filter`]).
pub(super) fn keep_off(bot: &mut Bot, due: &Due) {
    if bot.features.traps.field.is_empty() {
        return;
    }
    let field = bot.features.traps.field.clone();
    if route_hits(bot, &field, due.me.feet) {
        bot.end_route(RouteEnd::TrapAhead, due.now);
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

/// Whether the route of the bot runs over a disc of `field` within
/// [`KEEP_OFF_M`] ahead of `feet`.
pub(super) fn route_hits(bot: &Bot, field: &TrapField, feet: Vec3) -> bool {
    route_within(bot, field, feet, FIELD_M - 0.05)
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
pub(super) fn filter(bot: &Bot, landings: Vec<Vec3>, start: Option<NodeId>) -> RouteFilter {
    let filter = RouteFilter::shutting_landings(landings);
    if bot.features.traps.field.is_empty() {
        return filter;
    }
    let field = bot.features.traps.field.clone();
    filter.with_floors(move |node| Some(node) == start || field.open(node))
}

/// A think: whether this bot starts to put a trap down.
pub(super) fn think(bot: &mut Bot, due: &Due, state: &mut TeamState) {
    let t = &due.tuning.traps;
    let me = due.me;
    // Its trap let go of in the ticks since: promised to the team at once,
    // the field built again, the bot keeping off it from this think on.
    if let Some((at, released)) = bot.features.traps.placed.take() {
        state.knowledge.traps.promise(bot.place, at, released);
        state.refresh_traps(&due.tactics.world.grid, t.ring_m);
        bot.features.traps.field = state.features.traps.field.clone();
    }
    keep_off(bot, due);
    #[cfg(test)]
    passes::observe(bot, due, state);
    if bot.features.traps.habit.may_plan(t).is_err() {
        return;
    }
    #[cfg(test)]
    let waiting = bot.features.traps.pass_wait.is_some();
    let facts = Facts {
        now: due.now,
        play: bot.assignment.as_ref().map(|a| a.kind),
        carrying: me.carrying.is_some(),
        health: me.health,
        enemy: enemy_near(bot, due),
        last_hit: bot.hurt.last_hit_ms,
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
    if let Err(_refusal) = gate(&facts, t) {
        #[cfg(test)]
        {
            gates::count(gates::of(_refusal));
            if waiting {
                onpass::count(me.team, onpass::LOST_GATE);
            }
        }
        return;
    }
    if bot.rng.unit() >= t.chance {
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
                #[cfg(test)]
                gates::count(gates::NO_LOOK);
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
    // [A145] The spots of killed holders this route passes, that no trap of the team stands on.
    let wished_spots: Vec<Vec3> = match t.at_defeated {
        Some(d) => {
            let keep_ms = (d.keep_s * 1000.0) as u32;
            let route = &ahead[..(d.ahead_m as usize).min(ahead.len())];
            state
                .knowledge
                .defeated
                .iter()
                .filter(|w| due.now.saturating_sub(w.time) <= keep_ms)
                .map(|w| w.at)
                .filter(|at| !own.iter().any(|own| own.distance_xz(*at) <= d.within_m))
                .filter(|at| {
                    if staying {
                        me.feet.distance_xz(*at) <= d.within_m + reach_m
                    } else {
                        off_route_m(route, *at) <= d.within_m
                    }
                })
                .collect()
        }
        None => Vec::new(),
    };
    let wish = t
        .at_defeated
        .filter(|_| !wished_spots.is_empty())
        .map(|d| Wish {
            spots: &wished_spots,
            within_m: d.within_m,
            friend_m: d.friend_m,
        });
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
        lanes: &state.knowledge.humans,
        own: &own,
        in_spawn: &in_spawn,
        wish,
    };
    let chosen = choose(&search);
    // [A145] A spot of a killed holder ahead on the route that no candidate here is on yet: wait for the body to reach it.
    if let Some(wish) = wish {
        #[cfg(test)]
        defeated::count_team(me.team, defeated::THINKS);
        if !chosen.is_some_and(|c| c.wished) && wish.spots.iter().any(|at| me.feet.distance_xz(*at) > wish.within_m + reach_m + 1.0) {
            #[cfg(test)]
            defeated::count_team(me.team, defeated::WAITED);
            return;
        }
    }
    let at_defeated = chosen.is_some_and(|c| c.wished);
    #[cfg(test)]
    if waiting {
        if chosen.is_none() {
            onpass::count(me.team, onpass::LOST_NO_SPOT);
        }
        if bot.features.traps.pass_wait.is_some_and(|(_, since)| due.now.saturating_sub(since) >= PASS_WAIT_MS) {
            onpass::count(me.team, onpass::LOST_STALE);
        }
    }
    let lanes = &state.knowledge.humans;
    let here = chosen.map(|c| presence(lanes, c.spot, due.now));
    // Place on the pass: a cell ahead on the route that is richer in enemy lanes than this spot is waited for.
    if let Some(pass) = t.on_the_pass.filter(|_| !staying && !at_defeated) {
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
            return;
        }
    }
    #[cfg(test)]
    gates::count(if chosen.is_some() { gates::PLANNED } else { gates::NO_SPOT });
    if let Some(chosen) = chosen {
        #[cfg(test)]
        if chosen.wished {
            defeated::count_team(me.team, defeated::PLACED);
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
        bot.features.traps.pass_wait = None;
        bot.features.traps.habit.plan(chosen.yaw, chosen.spot, due.now);
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

    pub const NOTED_SPOT: usize = 0;
    pub const NOTED_CAMP: usize = 1;
    pub const THINKS: usize = 2;
    pub const WAITED: usize = 3;
    pub const PLACED: usize = 4;
    static TEAM: [[AtomicU64; 5]; 2] = [const { [const { AtomicU64::new(0) }; 5] }; 2];

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
                    "team {team}: holders killed (spot {}, camp {}), thinks with such a spot on the route {}, waited {}, trap placed on one {}",
                    c(NOTED_SPOT),
                    c(NOTED_CAMP),
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

    pub(in crate::v026g::bots) fn observe(bot: &mut Bot, due: &Due, state: &TeamState) {
        let feet = due.me.feet;
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
fn enemy_near(bot: &Bot, due: &Due) -> bool {
    let t = &due.tuning.traps;
    let quiet = (t.enemy_quiet_s * 1000.0) as u32;
    !bot.memory.all_seen(due.now, due.view.clock).is_empty()
        || bot.memory.tracks().iter().any(|track| {
            due.now < track.time + quiet || track.at.distance_xz(due.me.feet) < t.enemy_near_m
        })
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
pub(super) fn drive(
    bot: &mut Bot,
    me: &PlaceView,
    now: u32,
    calm: bool,
    crouch: bool,
    t: &TrapTuning,
) -> Option<TrapDrive> {
    let state = bot.features.traps.habit.state();
    if state == State::Idle {
        return None;
    }
    let kit = bot.features.traps.habit.kit()?;
    // The pose the placing is counted on: held from the press to the release.
    let crouched = match state {
        State::Down { crouched, .. } => crouched,
        _ => crouch,
    };
    let aim = aim_of(bot, me, crouched, t);
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
    if let Some(ended) = out.ended {
        bot.features.traps.habit.record(ended);
        if bot.tracing && !matches!(ended, Ended::Placed) {
            bot.traced.push(serde_json::json!({
                "t": now, "bot": bot.place, "trap_look_ended": format!("{ended:?}"),
                "at": me.feet.to_array(),
            }));
        }
    }
    if let Some(planned) = out.placed {
        // The promise is where the head ray of this frame meets the floor,
        // not the spot planned before the look down and the steps taken
        // since (1.8 m off at 3 m/s); a ray too short puts nothing down.
        // With the landing rule the eye is the held pose's.
        let eye = if t.press_landing_m.is_some() {
            me.feet + Vec3::new(0.0, super::view::eye_height(crouched, me.speed(), me.pitch), 0.0)
        } else {
            me.eye()
        };
        let landed = landing(eye, me.yaw, me.pitch, planned.y);
        bot.features.traps.placed = landed.map(|at| (at, now));
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
    Some(TrapDrive {
        look,
        key: if out.key_down { kit.key } else { 0 },
        crouch: out.hold_crouch,
    })
}

/// What the habit waits for this tick: the steepness of the look and whether a press would land right, from the
/// pose the bot has (it may have changed since the plan: the walk, a crouch). Worked out only while looking and
/// pressing; with `press_landing_m` off always the steepest look and no condition.
fn aim_of(bot: &Bot, me: &PlaceView, crouched: bool, t: &TrapTuning) -> Aim {
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
            let landed = landing(eye_for(feet, crouched, view), me.yaw, view, spot.y);
            Aim::Look {
                pitch: look.pitch,
                press: press_of(t, landed, feet, staying, || route_ahead(bot, me, t).0),
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
    /// The version of the team's traps its route was last looked at against the ring for (`keep_off`).
    pub ringed: u64,
    /// The richer cell ahead the habit is waiting to pass, and since when (`TrapTuning::on_the_pass`).
    pub pass_wait: Option<(mapdata::Vec3, u32)>,
    /// The bench's: the standing own trap this body last came within 3 m of, and the class it came with
    /// ([`passes`]).
    #[cfg(test)]
    pub near: Option<(mapdata::Vec3, usize, usize)>,
}

impl super::features::Feature for BotState {
    fn new_life(&mut self, _life: &super::features::Life) {
        self.habit.new_life();
        self.react.new_life();
        self.pass_wait = None;
        #[cfg(test)]
        {
            self.near = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v026g::bots::plays::tests::Scenario;
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

    /// [A145] The rule for killed holders' spots is off by default (benched inert), validated and read from a patch.
    #[test]
    fn the_defeated_rule_is_off_by_default_validated_and_read_from_a_patch() {
        use crate::v026g::bots::tuning::{DefeatedTuning, Tuning};
        assert_eq!(Tuning::default().traps.at_defeated, None);
        assert_eq!(Tuning::all_off().traps.at_defeated, None);
        assert_eq!(DefeatedTuning::default().min_kills, 2);
        let mut bad = Tuning::default();
        assert!(bad.validate().is_ok());
        bad.traps.at_defeated = Some(DefeatedTuning { within_m: 0.5, ..DefeatedTuning::default() });
        assert_eq!(bad.validate().unwrap_err(), "traps.at_defeated");
        bad.traps.at_defeated = Some(DefeatedTuning { friend_m: 1.0, ..DefeatedTuning::default() });
        assert_eq!(bad.validate().unwrap_err(), "traps.at_defeated", "a guard under the ordinary one");
        bad.traps.at_defeated = Some(DefeatedTuning { ahead_m: 40.0, ..DefeatedTuning::default() });
        assert_eq!(bad.validate().unwrap_err(), "traps.at_defeated", "farther than the route is read");
        let patched = Tuning::patched(serde_json::json!({"traps": {"at_defeated": {"keep_s": 60.0}}})).unwrap();
        assert_eq!(patched.traps.at_defeated.map(|d| (d.keep_s, d.within_m)), Some((60.0, 3.0)));
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
                        let landed = landing(eye_for(released, crouched, -checked), yaw, -checked, floor);
                        if press_of(&t, landed, released, true, Vec::new) != Press::Ready {
                            continue;
                        }
                        presses += 1;
                        // What happens: the view is anywhere from `down` to `down + slack`.
                        for extra in [0.0, PRESS_SLACK / 2.0, PRESS_SLACK] {
                            let at_release = (down + extra).min(LOOK_DOWN);
                            let at = landing(eye_for(released, crouched, -at_release), yaw, -at_release, floor)
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
}
