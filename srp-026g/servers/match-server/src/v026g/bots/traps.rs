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
//!   (PROVEN) and a bot looks down at most 1.0 rad (`body::MAX_PITCH`), so
//!   from the eye's 1.6 m it hits the floor 1.0 m out along the yaw. The
//!   eight yaws round the bot are the candidates ([`choose`]).
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

/// The steepest look down, radians (`body::MAX_PITCH`).
pub const LOOK_DOWN: f32 = 1.0;
/// The view is steep enough to press at this pitch: the head ray meets the
/// floor within 2.0 m from the standing eye only so steep (1.6 / sin 0.96 =
/// 1.98).
pub const PRESS_PITCH: f32 = 0.96;
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
    /// Looking down along `yaw` for the floor point `spot`, since `since`.
    Looking {
        yaw: f32,
        spot: Vec3,
        since: u32,
    },
    /// The key is down this tick; let go the next.
    Down {
        yaw: f32,
        spot: Vec3,
    },
    /// Done or given up; not again until `until`.
    Cooling {
        until: u32,
    },
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

/// The transition: the state and a tick give the new state and what the body
/// does. Total over the states.
pub fn step(state: State, tick: Tick, t: &TrapTuning) -> (State, Out) {
    let cooldown = |now: u32, scale: f32| State::Cooling {
        until: now + (t.cooldown_s * scale * 1000.0) as u32,
    };
    match state {
        State::Idle => (State::Idle, Out::default()),
        State::Looking { yaw, spot, since } => {
            if !tick.calm {
                // Called away: no harm done, soon again.
                return (cooldown(tick.now, 0.25), Out::default());
            }
            if tick.now.saturating_sub(since) > t.look_ms {
                return (cooldown(tick.now, 0.5), Out::default());
            }
            let on = turn(tick.yaw, yaw).abs() <= t.tolerance_rad && -tick.pitch >= PRESS_PITCH;
            if on {
                (
                    State::Down { yaw, spot },
                    Out {
                        look_yaw: Some(yaw),
                        key_down: true,
                        placed: None,
                    },
                )
            } else {
                (
                    state,
                    Out {
                        look_yaw: Some(yaw),
                        ..Out::default()
                    },
                )
            }
        }
        State::Down { yaw, spot } => (
            cooldown(tick.now, 1.0),
            Out {
                look_yaw: Some(yaw),
                key_down: false,
                placed: Some(spot),
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
}

/// What a bot's habit did in all its lives (the bench's activity and harm counts).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlaceCounts {
    pub placed: u32,
    pub died_placing: u32,
}

impl PlaceCounts {
    pub fn add(&mut self, other: &PlaceCounts) {
        self.placed += other.placed;
        self.died_placing += other.died_placing;
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

    /// What it did so far.
    pub fn counts(&self) -> PlaceCounts {
        PlaceCounts {
            placed: self.placed_total,
            died_placing: self.died_placing,
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
}

/// A spot picked: the yaw to look along, the floor point, its score.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Chosen {
    pub yaw: f32,
    pub spot: Vec3,
    pub score: f32,
}

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
            s.feet.x + d[0] * t.reach_m,
            s.feet.y,
            s.feet.z + d[1] * t.reach_m,
        );
        let Some(spot) = good_floor(s.grid, spot, t) else {
            continue;
        };
        if (s.in_spawn)(spot) {
            continue;
        }
        if s.own.iter().any(|own| own.distance_xz(spot) < t.near_own_m) {
            continue;
        }
        // Off its own route ahead, unless it stands.
        if !s.staying && off_route_m(s.ahead, spot) < t.lateral_m.max(AVOID_M) - 1e-3 {
            continue;
        }
        // Where its own body will be: it keeps off the disc.
        if s.feet.distance_xz(spot) < AVOID_M - 0.2 {
            continue;
        }
        let friends = friend_flow(s.mates, spot, t.friend_m);
        if friends > 0 {
            continue;
        }
        let value = score(presence(s.lanes, spot, s.now), friends, t);
        if value < t.min_score {
            continue;
        }
        // The better score wins; a look that turns the view less is worth a
        // little (a half turn costs 0.1).
        let rank = value - 0.1 * turn(s.view_yaw, yaw).abs() / std::f32::consts::PI;
        if best.is_none_or(|(_, r)| rank > r) {
            best = Some((
                Chosen {
                    yaw,
                    spot,
                    score: value,
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

/// The point to look at for a yaw: along it and steeply down, as far as the
/// head ray goes.
pub fn look_point(eye: Vec3, yaw: f32) -> Vec3 {
    let d = dir(yaw);
    let (run, rise) = (LOOK_DOWN.cos() * 2.0, LOOK_DOWN.sin() * 2.0);
    Vec3::new(eye.x + d[0] * run, eye.y - rise, eye.z + d[1] * run)
}

impl TeamState {
    /// The team's field again if its list changed; whether it did.
    pub(super) fn refresh_traps(&mut self, grid: &WalkGrid) -> bool {
        let version = self.knowledge.traps.version();
        if version == self.features.traps.field_version {
            return false;
        }
        self.features.traps.field = Arc::new(TrapField::of(&self.knowledge.traps, grid));
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
    }
}

/// Whether the route of the bot runs over a disc of `field` within
/// [`KEEP_OFF_M`] ahead of `feet`.
pub(super) fn route_hits(bot: &Bot, field: &TrapField, feet: Vec3) -> bool {
    if !field
        .discs()
        .iter()
        .any(|disc| disc.distance_xz(feet) < KEEP_OFF_M)
    {
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
    field.hits(&points)
}

/// `filter` with the floors of the traps the bot noticed kept out as a
/// preference (soft): a way round them where that is not much longer, else
/// over them. Not the floor the body stands on (`start`).
pub(super) fn soften(bot: &Bot, filter: RouteFilter, start: Option<NodeId>) -> RouteFilter {
    if bot.features.traps.react.field().is_empty() {
        return filter;
    }
    let field = bot.features.traps.react.field().clone();
    filter.avoiding_floors(move |node| Some(node) == start || field.open(node))
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
        state.refresh_traps(&due.tactics.world.grid);
        bot.features.traps.field = state.features.traps.field.clone();
    }
    keep_off(bot, due);
    if bot.features.traps.habit.may_plan(t).is_err() {
        return;
    }
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
    if gate(&facts, t).is_err() || bot.rng.unit() >= t.chance {
        return;
    }
    let (ahead, jump) = route_ahead(bot, me, t);
    if jump {
        return;
    }
    let mates: Vec<(Vec3, [f32; 2])> = due
        .view
        .team(me.team)
        .filter(|p| p.place != me.place)
        .map(|p| (p.feet, p.velocity))
        .collect();
    let staying = matches!(bot.intent.go, super::plays::Go::Stay)
        && me.velocity[0].hypot(me.velocity[1]) < STANDING_MS;
    let in_spawn = |at: Vec3| due.tactics.in_spawn_area(at, None);
    let own: Vec<Vec3> = state
        .knowledge
        .traps
        .standing()
        .map(|trap| trap.at)
        .collect();
    let search = Search {
        t,
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
    };
    if let Some(chosen) = choose(&search) {
        bot.features.traps.habit.plan(chosen.yaw, chosen.spot, due.now);
        if bot.tracing {
            bot.traced.push(serde_json::json!({
                "t": due.now, "bot": bot.place, "trap_plan": chosen.spot.to_array(),
                "score": chosen.score,
            }));
        }
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
/// aims at one.
pub(super) fn drive(
    bot: &mut Bot,
    me: &PlaceView,
    now: u32,
    calm: bool,
    t: &TrapTuning,
) -> Option<TrapDrive> {
    if bot.features.traps.habit.state() == State::Idle {
        return None;
    }
    let kit = bot.features.traps.habit.kit()?;
    let out = bot.features.traps.habit.tick(
        Tick {
            now,
            yaw: me.yaw,
            pitch: me.pitch,
            calm,
        },
        t,
    );
    if let Some(planned) = out.placed {
        // The promise is where the head ray of this frame meets the floor,
        // not the spot planned before the look down and the steps taken
        // since (1.8 m off at 3 m/s); a ray too short puts nothing down.
        let landed = landing(me.eye(), me.yaw, me.pitch, planned.y);
        bot.features.traps.placed = landed.map(|at| (at, now));
        if bot.tracing {
            bot.traced.push(serde_json::json!({
                "t": now, "bot": bot.place, "trap_promise": landed.map(Vec3::to_array),
                "planned": planned.to_array(),
            }));
        }
    }
    Some(TrapDrive {
        look: out.look_yaw.map(|yaw| look_point(me.eye(), yaw)),
        key: if out.key_down { kit.key } else { 0 },
    })
}

/// What the habit asks of a drive tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct TrapDrive {
    pub look: Option<Vec3>,
    pub key: u32,
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
}

impl super::features::Feature for BotState {
    fn new_life(&mut self, _life: &super::features::Life) {
        self.habit.new_life();
        self.react.new_life();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v026g::bots::plays::tests::Scenario;
    use crate::v026g::bots::view::Mode;

    fn t() -> TrapTuning {
        TrapTuning::default()
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
        assert_eq!(habit.counts(), PlaceCounts::default());
        let mut habit = habit;
        habit.new_life();
        assert_eq!(habit.counts().died_placing, 0, "a life that began idle");
        habit.state = State::Looking { yaw: 0.0, spot: Vec3::ZERO, since: 0 };
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
        assert_eq!(out, Out::default());
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
        let p = look_point(eye, 0.0);
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
        let tuning = crate::v026g::bots::tuning::Tuning::default();
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
    fn the_defaults_are_valid_and_the_team_counts_what_stands() {
        crate::v026g::bots::tuning::Tuning::default()
            .validate()
            .unwrap();
        let mut bad = crate::v026g::bots::tuning::Tuning::default();
        bad.traps.reach_m = 2.0;
        assert_eq!(bad.validate().unwrap_err(), "traps.reach_m");
    }
}
