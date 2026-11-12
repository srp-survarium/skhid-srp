//! What the match is, as the bots get it: the `view` layer
//! (`docs/notes/player_bots_research.md` B1, B4 "snapshot in, commands
//! out"). [`Tracker`] keeps it up from what the match hands over: every
//! player's inputs as they are relayed, and the trusted worlds. A
//! [`MatchView`] is an owned copy of it at one moment; the layers above
//! read only that.
//!
//! - **Bodies:** every place's body is carried forward from its inputs
//!   between worlds (`body::BodyTrack`, as a bot's own), so a human's
//!   place is known to within a step or two, not five seconds old. Who may
//!   know it is the senses' business, not the view's.
//! - **Sounds:** from the inputs: shots (fire held), sprinting, a reload,
//!   a weapon switch, a use, a jump; the senses decide who hears them.
//! - **Facts from the worlds:** alive, health, the spawn time, kills,
//!   deaths and streaks, steals and deliveries, the batteries and the
//!   containers' counts, the teams' points. A death seen in the statistics
//!   is an event with its place (the kill feed).
//! - **Carriers:** a world says a battery is carried, not by whom; the
//!   carrier is the player who pressed use near it last, else the nearest.

use std::collections::VecDeque;

use mapdata::{Vec3, World};
use survarium::v026g::maps::RuleMode;

use super::body::{BodyTrack, Fix};
use super::kin::Kinematics;
use crate::v026g::drive::key;

/// A roster place.
pub type Place = usize;

/// One of a team mode's two teams, by the game's number.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Team {
    Zero,
    One,
}

impl Team {
    pub const BOTH: [Self; 2] = [Self::Zero, Self::One];

    /// A team mode's team as the match gives it: 0 or 1 (the registry
    /// gives a team mode's place no other, and counts any other as 1
    /// itself).
    pub fn of(number: u8) -> Self {
        if number == 0 { Self::Zero } else { Self::One }
    }

    /// The team it plays against.
    pub fn other(self) -> Self {
        match self {
            Self::Zero => Self::One,
            Self::One => Self::Zero,
        }
    }

    /// The game's number of it: its index in what is kept by team, and its
    /// bit in a HUD mask.
    pub fn index(self) -> usize {
        self as usize
    }
}

impl std::fmt::Display for Team {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.index().fmt(f)
    }
}

/// Whose side a place plays on. Two places are allies when their sides are
/// equal and enemies when not. Only a [`Team`] has the one side against it
/// ([`Team::other`]); a free-for-all's place has every other place.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Side {
    /// A team mode: its team's.
    Team(Team),
    /// A free-for-all: its own, by its place.
    Solo(Place),
}

impl Side {
    /// Its index in what a match keeps by side ([`Sides::count`]): a
    /// team's number, a free-for-all's place.
    pub fn index(self) -> usize {
        match self {
            Self::Team(team) => team.index(),
            Self::Solo(place) => place,
        }
    }
}

#[cfg(test)]
impl Side {
    /// A team mode's side by the game's number, as the tests name them.
    pub fn team(number: u8) -> Self {
        Self::Team(Team::of(number))
    }
}

impl PartialEq<Team> for Side {
    fn eq(&self, team: &Team) -> bool {
        *self == Self::Team(*team)
    }
}

impl PartialEq<Side> for Team {
    fn eq(&self, side: &Side) -> bool {
        side == self
    }
}

/// As the logs name it: `team 0`, or a free-for-all's `place 3`.
impl std::fmt::Display for Side {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Team(team) => write!(f, "team {team}"),
            Self::Solo(place) => write!(f, "place {place}"),
        }
    }
}

/// How a match's places are sided: the one reading of the teams the match
/// gives its places ([`Sides::side`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sides {
    /// Two teams.
    Teams,
    /// A free-for-all: every place on its own.
    Solo,
}

impl Sides {
    pub fn of(mode: RuleMode) -> Self {
        match mode {
            RuleMode::FreeForAll => Self::Solo,
            RuleMode::Batteries
            | RuleMode::TimedBatteries
            | RuleMode::TeamDeathmatch
            | RuleMode::Research => Self::Teams,
        }
    }

    /// The side of roster place `place`, whose team the match gave as
    /// `team`. A free-for-all's place is its own side whatever that team
    /// (`maps::free_for_all_team`: 4 and up, another for each place).
    pub fn side(self, place: Place, team: u8) -> Side {
        match self {
            Self::Teams => Side::Team(Team::of(team)),
            Self::Solo => Side::Solo(place),
        }
    }

    /// The sides of a match of `places` places, each at its
    /// [`Side::index`].
    pub fn all(self, places: usize) -> Vec<Side> {
        match self {
            Self::Teams => Team::BOTH.map(Side::Team).to_vec(),
            Self::Solo => (0..places).map(Side::Solo).collect(),
        }
    }
}

/// The height of the eyes over the feet, standing and crouched, metres
/// (GUESS from the capsule: 1.8 m standing, 1.1 m crouched).
pub const EYE_STANDING: f32 = 1.6;
pub const EYE_CROUCHED: f32 = 0.95;
/// The chest, where a shot is aimed.
pub const CHEST: f32 = 1.3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Batteries,
    TimedBatteries,
    TeamDeathmatch,
}

impl Mode {
    pub fn of(mode: RuleMode) -> Self {
        match mode {
            RuleMode::TimedBatteries => Self::TimedBatteries,
            // A free-for-all plays as Team Deathmatch with no teammates.
            RuleMode::TeamDeathmatch | RuleMode::FreeForAll => Self::TeamDeathmatch,
            RuleMode::Batteries | RuleMode::Research => Self::Batteries,
        }
    }

    pub fn has_batteries(self) -> bool {
        self != Self::TeamDeathmatch
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoundKind {
    Shot,
    Sprint,
    Reload,
    Switch,
    Use,
    Jump,
}

/// A sound a player made.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sound {
    pub from: Place,
    pub kind: SoundKind,
    pub at: Vec3,
    pub time: u32,
}

/// A death seen in a world's statistics: who, where last reckoned, when.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Death {
    pub place: Place,
    pub at: Vec3,
    pub time: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BatteryState {
    /// Lying at a place (a spawner, or dropped).
    Free(Vec3),
    /// Carried; by whom, as far as can be told.
    Carried(Option<Place>),
    /// In a container, by its id.
    Home(u8),
}

/// One place of the match.
#[derive(Clone, Debug, PartialEq)]
pub struct PlaceView {
    pub place: Place,
    /// Its side: its team, or in a free-for-all its own.
    pub team: Side,
    pub bot: bool,
    /// Entered and not left.
    pub present: bool,
    pub alive: bool,
    pub feet: Vec3,
    pub yaw: f32,
    /// The view's pitch: the base pitch and, for a bot, its weapon's kick
    /// (`body::Estimate::view_pitch`).
    pub pitch: f32,
    pub velocity: [f32; 2],
    /// The keys held now.
    pub keys: u32,
    /// Part 0's health, `[0, 1]` (LIKELY the player's).
    pub health: f32,
    /// When this life began (`m_inserted_time_in_ms`).
    pub spawned: u32,
    pub kills: u32,
    pub deaths: u32,
    pub streak: u32,
    /// The number of a battery it carries.
    pub carrying: Option<usize>,
    /// The time of the world (or input) the body was last put right by.
    pub seen_at: u32,
}

impl PlaceView {
    pub fn crouched(&self) -> bool {
        self.keys & key::CROUCH != 0
    }

    pub fn eye(&self) -> Vec3 {
        let up = if self.crouched() {
            EYE_CROUCHED
        } else {
            EYE_STANDING
        };
        self.feet + Vec3::new(0.0, up, 0.0)
    }

    pub fn chest(&self) -> Vec3 {
        let up = if self.crouched() { CHEST * 0.6 } else { CHEST };
        self.feet + Vec3::new(0.0, up, 0.0)
    }

    pub fn speed(&self) -> f32 {
        self.velocity[0].hypot(self.velocity[1])
    }
}

/// A container and its count.
#[derive(Clone, Debug, PartialEq)]
pub struct ContainerView {
    pub id: u8,
    /// The team it belongs to.
    pub team: Team,
    pub at: Vec3,
    pub batteries: usize,
}

/// The match at one moment.
#[derive(Clone, Debug, PartialEq)]
pub struct MatchView {
    pub clock: u32,
    pub mode: Mode,
    /// Seconds left in play; the whole play time.
    pub time_left_s: f32,
    pub total_s: f32,
    /// Batteries home, or tickets in Team Deathmatch, by team
    /// ([`Team::index`]); a free-for-all's rules have neither.
    pub points: [u32; 2],
    pub places: Vec<PlaceView>,
    pub batteries: Vec<BatteryState>,
    /// By battery number: the HUD masks that came with the batteries
    /// (`WorldFacts::marks`); empty when that world gave none.
    pub marks: Vec<u32>,
    pub containers: Vec<ContainerView>,
    /// The sounds of the last few seconds.
    pub sounds: Vec<Sound>,
    /// The deaths of the last half minute.
    pub deaths: Vec<Death>,
    /// The time of the newest world.
    pub world_time: u32,
    /// Whether each team may still respawn ([`Self::can_respawn`]).
    pub can_respawn: [bool; 2],
    /// Timed batteries: when the next battery appears (match ms).
    pub next_battery: Option<u32>,
}

impl MatchView {
    pub fn place(&self, place: Place) -> Option<&PlaceView> {
        self.places.get(place)
    }

    /// Our points minus theirs, for `side` (tickets count a tenth). A
    /// free-for-all's place stands by its kills, counted as tickets are:
    /// its own minus the most any other place has.
    pub fn score_gap(&self, side: Side) -> f32 {
        match side {
            Side::Team(team) => {
                let (ours, theirs) = (self.points[team.index()], self.points[team.other().index()]);
                let gap = ours as f32 - theirs as f32;
                if self.mode == Mode::TeamDeathmatch {
                    gap / 10.0
                } else {
                    gap
                }
            }
            Side::Solo(place) => {
                let kills = |p: &PlaceView| p.kills as f32;
                let ours = self.place(place).map_or(0.0, kills);
                let theirs = self
                    .places
                    .iter()
                    .filter(|p| p.present && p.place != place)
                    .map(kills)
                    .fold(0.0, f32::max);
                (ours - theirs) / 10.0
            }
        }
    }

    /// Whether `team` may still respawn (Team Deathmatch clears it when
    /// the team's tickets are out).
    pub fn can_respawn(&self, team: Team) -> bool {
        self.can_respawn[team.index()]
    }

    /// The living players of `team` (or not of it).
    pub fn team(&self, team: Side) -> impl Iterator<Item = &PlaceView> {
        self.places
            .iter()
            .filter(move |p| p.present && p.alive && p.team == team)
    }

    pub fn enemies_of(&self, team: Side) -> impl Iterator<Item = &PlaceView> {
        self.places
            .iter()
            .filter(move |p| p.present && p.alive && p.team != team)
    }

    pub fn container_of(&self, team: Side) -> Option<&ContainerView> {
        self.containers.iter().find(|c| c.team == team)
    }
}

/// What the match hands over each poll besides inputs and worlds.
#[derive(Clone, Debug, PartialEq)]
pub struct Clock {
    pub clock: u32,
    pub time_left_s: f32,
    pub total_s: f32,
    /// Play has begun: the start countdown is over. Before it the game
    /// holds the players where they stand, and the bots rest (the owner's
    /// word, 2026-10-16: the bots ran off while he waited the countdown).
    pub playing: bool,
}

/// What a trusted world says beyond the bodies (from the decoded
/// statistics and rules).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WorldFacts {
    pub time: u32,
    /// By place: (kills, deaths, streak).
    pub stats: Vec<(u32, u32, u32)>,
    pub points: Option<[u32; 2]>,
    /// By battery number.
    pub batteries: Vec<BatteryState>,
    /// By battery number: a lying battery's `m_spotted_mask`, the client's
    /// HUD (team `t`'s HUD shows it when bit `1 << t` is set); 0 for one
    /// carried or home. Empty when the world did not say.
    pub marks: Vec<u32>,
    /// By container id: the batteries in it.
    pub container_counts: Vec<usize>,
    /// Whether each team may still respawn (Team Deathmatch clears a
    /// team's when its tickets are out).
    pub can_respawn: Option<[bool; 2]>,
    /// Timed batteries: when the next battery appears (match ms).
    pub next_battery: Option<u32>,
}

/// Sounds are kept this long, ms.
const SOUNDS_KEPT_MS: u32 = 3_000;
/// Deaths are kept this long, ms.
const DEATHS_KEPT_MS: u32 = 30_000;
/// A use this near a battery, this recently, makes the user its carrier.
const CARRIER_USE_M: f32 = 4.0;
const CARRIER_USE_MS: u32 = 6_000;

#[derive(Clone, Debug)]
struct PlaceState {
    team: Side,
    bot: bool,
    present: bool,
    body: BodyTrack,
    keys: u32,
    /// The last input's tick.
    last_input: Option<u32>,
    alive: bool,
    health: f32,
    spawned: u32,
    stats: (u32, u32, u32),
    last_use: Option<(u32, Vec3)>,
    last_shot: Option<u32>,
    /// The body's life (`BodyTrack::last_alive`) whose death the kill feed
    /// has a spot for: one spot a life.
    death_spotted: Option<u32>,
}

impl PlaceState {
    /// A place nobody entered yet.
    fn new(team: Side, bot: bool) -> Self {
        Self {
            team,
            bot,
            present: false,
            body: BodyTrack::default(),
            keys: 0,
            last_input: None,
            alive: false,
            health: 1.0,
            spawned: 0,
            stats: (0, 0, 0),
            last_use: None,
            last_shot: None,
            death_spotted: None,
        }
    }
}

/// The view's keeper.
pub struct Tracker {
    places: Vec<PlaceState>,
    mode: Mode,
    sides: Sides,
    /// Container id → (team, place on the map).
    containers: Vec<(Team, Vec3)>,
    batteries: Vec<BatteryState>,
    marks: Vec<u32>,
    carriers: Vec<Option<Place>>,
    container_counts: Vec<usize>,
    points: [u32; 2],
    can_respawn: [bool; 2],
    next_battery: Option<u32>,
    sounds: VecDeque<Sound>,
    deaths: VecDeque<Death>,
    world_time: u32,
}

impl Tracker {
    /// `teams[place]` as the match gives them (read by [`Sides::side`]),
    /// `bots[place]`; `containers[id]` = (team, place).
    pub fn new(mode: RuleMode, teams: &[u8], bots: &[bool], containers: Vec<(Team, Vec3)>) -> Self {
        let sides = Sides::of(mode);
        Self {
            places: teams
                .iter()
                .zip(bots)
                .enumerate()
                .map(|(place, (&team, &bot))| PlaceState::new(sides.side(place, team), bot))
                .collect(),
            mode: Mode::of(mode),
            sides,
            container_counts: vec![0; containers.len()],
            containers,
            batteries: Vec::new(),
            marks: Vec::new(),
            carriers: Vec::new(),
            points: [0, 0],
            can_respawn: [true, true],
            next_battery: None,
            sounds: VecDeque::new(),
            deaths: VecDeque::new(),
            world_time: 0,
        }
    }

    /// The map's containers: by id, (team, place).
    pub fn set_containers(&mut self, containers: Vec<(Team, Vec3)>) {
        self.container_counts.resize(containers.len(), 0);
        self.containers = containers;
    }

    /// How the match's places are sided.
    pub fn sides(&self) -> Sides {
        self.sides
    }

    /// The side of `place`.
    pub fn side(&self, place: Place) -> Option<Side> {
        self.places.get(place).map(|state| state.team)
    }

    /// A place added (a late joiner, never a bot), whose team the match
    /// gave as `team`. Returns its side.
    pub fn push_place(&mut self, team: u8) -> Side {
        let side = self.sides.side(self.places.len(), team);
        self.places.push(PlaceState::new(side, false));
        side
    }

    pub fn entered(&mut self, place: Place, entered: bool) {
        if let Some(state) = self.places.get_mut(place) {
            state.present = entered;
        }
    }

    pub fn body(&self, place: Place) -> Option<&BodyTrack> {
        self.places.get(place).map(|state| &state.body)
    }

    /// An input of `place` for the tick `time` (a human's as relayed, a
    /// bot's as made, its pitch turn with the part its kick took and the
    /// kick after it: `body::BodyTrack::decided`): its body moves on, and
    /// its sounds are heard.
    #[allow(clippy::too_many_arguments)]
    pub fn input(
        &mut self,
        place: Place,
        time: u32,
        keys: u32,
        turn: (f32, f32),
        kick: f32,
        kin: &Kinematics,
        world: Option<&World>,
    ) {
        let Some(state) = self.places.get_mut(place) else {
            return;
        };
        if state.last_input.is_some_and(|last| time <= last) {
            return;
        }
        state.last_input = Some(time);
        let pressed = keys & !state.keys;
        state.keys = keys;
        state
            .body
            .decided(time, keys, turn, kick, kin, world.map(|world| &*world.grid));
        let at = state.body.estimate().map(|e| e.feet);
        let Some(at) = at else {
            return;
        };
        let mut heard = |kind| {
            self.sounds.push_back(Sound {
                from: place,
                kind,
                at,
                time,
            });
        };
        if pressed & key::FIRE != 0 {
            heard(SoundKind::Shot);
            state.last_shot = Some(time);
        }
        if pressed & key::RELOAD != 0 {
            heard(SoundKind::Reload);
        }
        if pressed & (key::WEAPON_1 | key::WEAPON_2) != 0 {
            heard(SoundKind::Switch);
        }
        if pressed & key::USE != 0 {
            heard(SoundKind::Use);
            state.last_use = Some((time, at));
        }
        if pressed & key::JUMP != 0 {
            heard(SoundKind::Jump);
        }
    }

    /// The sounds of keys held: fire held is a shot every 150 ms, a sprint
    /// is heard while it lasts (a sound a poll, at most every 500 ms).
    pub fn held_sounds(&mut self, clock: u32) {
        for (place, state) in self.places.iter_mut().enumerate() {
            let Some(at) = state.body.estimate().map(|e| e.feet) else {
                continue;
            };
            if state.keys & key::FIRE != 0 && state.last_shot.is_none_or(|last| clock >= last + 150)
            {
                state.last_shot = Some(clock);
                self.sounds.push_back(Sound {
                    from: place,
                    kind: SoundKind::Shot,
                    at,
                    time: clock,
                });
            }
            let sprinting = state.keys & key::SPRINT != 0 && state.keys & key::FORWARD != 0;
            let heard_lately = self
                .sounds
                .iter()
                .rev()
                .take_while(|sound| sound.time + 500 > clock)
                .any(|sound| sound.from == place && sound.kind == SoundKind::Sprint);
            if sprinting && !heard_lately {
                self.sounds.push_back(Sound {
                    from: place,
                    kind: SoundKind::Sprint,
                    at,
                    time: clock,
                });
            }
        }
        while self
            .sounds
            .front()
            .is_some_and(|sound| sound.time + SOUNDS_KEPT_MS < clock)
        {
            self.sounds.pop_front();
        }
        while self
            .deaths
            .front()
            .is_some_and(|death| death.time + DEATHS_KEPT_MS < clock)
        {
            self.deaths.pop_front();
        }
    }

    /// A trusted world's word on one place's body. Returns whether it
    /// starts a new life.
    pub fn fixed(
        &mut self,
        place: Place,
        fix: Fix,
        health: Option<f32>,
        kin: &Kinematics,
        world: Option<&World>,
    ) -> bool {
        let Some(state) = self.places.get_mut(place) else {
            return false;
        };
        state.alive = fix.alive;
        state.spawned = fix.inserted;
        if let Some(health) = health {
            state.health = health;
        }
        state.body.fixed(fix, kin, world.map(|world| &*world.grid))
    }

    /// The rest of a trusted world: statistics (deaths become events),
    /// batteries and containers (carriers worked out). Returns the deaths
    /// it put in the kill feed.
    pub fn facts(&mut self, facts: WorldFacts) -> Vec<Death> {
        let mut new_deaths = Vec::new();
        self.world_time = self.world_time.max(facts.time);
        for (place, stats) in facts.stats.iter().enumerate() {
            let Some(state) = self.places.get_mut(place) else {
                continue;
            };
            if stats.1 > state.stats.1 {
                // Where the body was last reckoned alive: the world that
                // shows the death has the body dead, without feet, and its
                // fixes come first. A death where the body was never placed
                // says nothing of a spot (it was the map's origin, and
                // trades ran there), nor does a second one in a life no
                // world showed (the spot is the life before's).
                if let Some((life, at)) = state.body.last_alive()
                    && state.death_spotted != Some(life)
                {
                    state.death_spotted = Some(life);
                    let death = Death {
                        place,
                        at,
                        time: facts.time,
                    };
                    self.deaths.push_back(death);
                    new_deaths.push(death);
                }
                state.alive = false;
            }
            state.stats = *stats;
        }
        if let Some(points) = facts.points {
            self.points = points;
        }
        if let Some(can_respawn) = facts.can_respawn {
            self.can_respawn = can_respawn;
        }
        if facts.next_battery.is_some() {
            self.next_battery = facts.next_battery;
        }
        if !facts.container_counts.is_empty() {
            self.container_counts = facts.container_counts;
        }
        if facts.batteries.is_empty() {
            return new_deaths;
        }
        self.carriers.resize(facts.batteries.len(), None);
        for (battery, state) in facts.batteries.iter().enumerate() {
            let before = self.batteries.get(battery).copied();
            self.carriers[battery] = match state {
                BatteryState::Carried(_) => match before {
                    Some(BatteryState::Carried(_)) => self.carriers[battery],
                    Some(BatteryState::Free(at)) => self.carrier_near(at, facts.time),
                    Some(BatteryState::Home(id)) => {
                        let at = self.containers.get(usize::from(id)).map(|c| c.1);
                        at.and_then(|at| self.carrier_near(at, facts.time))
                    }
                    None => None,
                },
                _ => None,
            };
            // A carrier who died has dropped it (the next world says where).
            if let Some(carrier) = self.carriers[battery]
                && !self.places[carrier].alive
            {
                self.carriers[battery] = None;
            }
        }
        self.batteries = facts.batteries;
        self.marks = facts.marks;
        new_deaths
    }

    /// The player who pressed use near `at` lately, else the nearest one.
    fn carrier_near(&self, at: Vec3, time: u32) -> Option<Place> {
        let used = self
            .places
            .iter()
            .enumerate()
            .filter_map(|(place, state)| {
                let (when, from) = state.last_use?;
                (when + CARRIER_USE_MS >= time && from.distance_xz(at) < CARRIER_USE_M)
                    .then_some((place, when))
            })
            .max_by_key(|(_, when)| *when)
            .map(|(place, _)| place);
        used.or_else(|| {
            self.places
                .iter()
                .enumerate()
                .filter(|(_, state)| state.alive)
                .filter_map(|(place, state)| {
                    let feet = state.body.estimate()?.feet;
                    Some((place, feet.distance_xz(at)))
                })
                .filter(|(_, d)| *d < 6.0)
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(place, _)| place)
        })
    }

    /// The match as it is at `clock`. A human's body is reckoned on from
    /// its last input to the clock; a bot's is where its last decided tick
    /// put it.
    pub fn view(&self, clock: &Clock, kin: &Kinematics, world: Option<&World>) -> MatchView {
        let places = self
            .places
            .iter()
            .enumerate()
            .map(|(place, state)| {
                let estimate = if state.bot {
                    state.body.estimate().copied()
                } else {
                    state
                        .body
                        .at(clock.clock, kin, world.map(|world| &*world.grid))
                };
                let carrying = self
                    .carriers
                    .iter()
                    .position(|carrier| *carrier == Some(place));
                PlaceView {
                    place,
                    team: state.team,
                    bot: state.bot,
                    present: state.present,
                    alive: state.alive && estimate.is_some(),
                    feet: estimate.map_or(Vec3::ZERO, |e| e.feet),
                    yaw: estimate.map_or(0.0, |e| e.yaw),
                    pitch: estimate.map_or(0.0, |e| e.view_pitch()),
                    velocity: estimate.map_or([0.0, 0.0], |e| e.velocity),
                    keys: state.keys,
                    health: state.health,
                    spawned: state.spawned,
                    kills: state.stats.0,
                    deaths: state.stats.1,
                    streak: state.stats.2,
                    carrying,
                    seen_at: state.body.fix().map_or(0, |fix| fix.time),
                }
            })
            .collect();
        let batteries = self
            .batteries
            .iter()
            .enumerate()
            .map(|(battery, state)| match state {
                BatteryState::Carried(_) => BatteryState::Carried(self.carriers[battery]),
                other => *other,
            })
            .collect();
        let containers = self
            .containers
            .iter()
            .enumerate()
            .map(|(id, &(team, at))| ContainerView {
                id: id as u8,
                team,
                at,
                batteries: self.container_counts.get(id).copied().unwrap_or(0),
            })
            .collect();
        MatchView {
            clock: clock.clock,
            mode: self.mode,
            time_left_s: clock.time_left_s,
            total_s: clock.total_s,
            points: self.points,
            places,
            batteries,
            marks: self.marks.clone(),
            containers,
            sounds: self.sounds.iter().copied().collect(),
            deaths: self.deaths.iter().copied().collect(),
            world_time: self.world_time,
            can_respawn: self.can_respawn,
            next_battery: self.next_battery,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fix(time: u32, feet: Vec3) -> Fix {
        Fix {
            time,
            alive: true,
            inserted: 10,
            feet: Some(feet),
            yaw: 0.0,
            pitch: 0.0,
            keys: 0,
            speed: Some(0.0),
        }
    }

    #[test]
    fn a_humans_body_is_reckoned_to_the_clock_and_its_keys_are_heard() {
        let kin = Kinematics::default();
        let mut tracker = Tracker::new(RuleMode::Batteries, &[0, 1], &[true, false], vec![]);
        tracker.entered(1, true);
        assert!(tracker.fixed(
            1,
            fix(1_000, Vec3::new(5.0, 0.0, 5.0)),
            Some(0.8),
            &kin,
            None
        ));
        tracker.input(
            1,
            1_050,
            key::FORWARD | key::FIRE,
            (0.0, 0.0),
            0.0,
            &kin,
            None,
        );
        tracker.held_sounds(1_100);
        let clock = Clock {
            clock: 3_000,
            time_left_s: 600.0,
            total_s: 900.0,
            playing: true,
        };
        let view = tracker.view(&clock, &kin, None);
        let human = view.place(1).unwrap();
        assert!(human.alive && human.present);
        // Walking +z from 1.1 s to 3 s: about 6 m on.
        assert!(human.feet.z > 9.0, "{human:?}");
        assert_eq!(human.health, 0.8);
        assert!(
            view.sounds
                .iter()
                .any(|s| s.from == 1 && s.kind == SoundKind::Shot)
        );
        // The bot's place, never seen, is not alive in the view.
        assert!(!view.place(0).unwrap().alive);
    }

    #[test]
    fn a_death_in_the_statistics_is_an_event_and_the_user_near_a_battery_carries_it() {
        let kin = Kinematics::default();
        let mut tracker = Tracker::new(
            RuleMode::Batteries,
            &[0, 1],
            &[false, false],
            vec![
                (Team::Zero, Vec3::ZERO),
                (Team::One, Vec3::new(50.0, 0.0, 0.0)),
            ],
        );
        for place in 0..2 {
            tracker.entered(place, true);
        }
        tracker.fixed(0, fix(1_000, Vec3::new(10.0, 0.0, 10.0)), None, &kin, None);
        tracker.fixed(1, fix(1_000, Vec3::new(30.0, 0.0, 30.0)), None, &kin, None);
        let battery = Vec3::new(30.5, 0.0, 30.0);
        tracker.facts(WorldFacts {
            time: 1_000,
            batteries: vec![BatteryState::Free(battery)],
            stats: vec![(0, 0, 0), (0, 0, 0)],
            ..WorldFacts::default()
        });
        tracker.input(1, 1_100, key::USE, (0.0, 0.0), 0.0, &kin, None);
        tracker.facts(WorldFacts {
            time: 2_000,
            batteries: vec![BatteryState::Carried(None)],
            stats: vec![(1, 0, 1), (0, 1, 0)],
            ..WorldFacts::default()
        });
        let view = tracker.view(
            &Clock {
                clock: 2_000,
                time_left_s: 100.0,
                total_s: 900.0,
                playing: true,
            },
            &kin,
            None,
        );
        assert_eq!(
            view.batteries[0],
            BatteryState::Carried(None),
            "the carrier died"
        );
        assert_eq!(view.deaths.len(), 1);
        assert_eq!(view.deaths[0].place, 1);
        assert_eq!(view.place(0).unwrap().kills, 1);
    }

    /// The client's world shows a dead body without feet, and its fixes
    /// come before its statistics (`game::driven`): the death is still in
    /// the kill feed, at the spot the body was last reckoned alive. One
    /// spot a life; a body never placed gives none.
    #[test]
    fn a_death_the_world_shows_without_feet_is_in_the_feed_where_the_body_fell() {
        let kin = Kinematics::default();
        let mut tracker = Tracker::new(RuleMode::Batteries, &[0, 0, 1], &[true, true, false], vec![]);
        for place in 0..3 {
            tracker.entered(place, true);
        }
        let dead = |time: u32| Fix {
            alive: false,
            feet: None,
            ..fix(time, Vec3::ZERO)
        };
        let fell = Vec3::new(12.0, 0.0, 7.0);
        tracker.fixed(0, fix(1_000, fell), None, &kin, None);
        tracker.fixed(1, fix(1_000, Vec3::new(20.0, 0.0, 7.0)), None, &kin, None);
        let stats = |deaths: [u32; 3]| deaths.map(|d| (0, d, 0)).to_vec();
        tracker.facts(WorldFacts {
            time: 1_000,
            stats: stats([0, 0, 0]),
            ..WorldFacts::default()
        });
        // Place 0 dies; place 2 (never placed) too.
        tracker.fixed(0, dead(2_000), None, &kin, None);
        tracker.fixed(2, dead(2_000), None, &kin, None);
        let new = tracker.facts(WorldFacts {
            time: 2_000,
            stats: stats([1, 0, 1]),
            ..WorldFacts::default()
        });
        let death = Death {
            place: 0,
            at: fell,
            time: 2_000,
        };
        assert_eq!(new, vec![death]);
        let clock = Clock {
            clock: 2_000,
            time_left_s: 100.0,
            total_s: 900.0,
            playing: true,
        };
        let view = tracker.view(&clock, &kin, None);
        assert_eq!(view.deaths, vec![death]);
        assert!(!view.place(0).unwrap().alive);
        // A second death in a life no world showed: no spot (the last one
        // is the life before's).
        tracker.fixed(0, dead(3_000), None, &kin, None);
        let new = tracker.facts(WorldFacts {
            time: 3_000,
            stats: stats([2, 0, 1]),
            ..WorldFacts::default()
        });
        assert!(new.is_empty());
        // A life a world showed, then its death: its own spot.
        let there = Vec3::new(30.0, 0.0, 2.0);
        let respawned = Fix {
            inserted: 3_500,
            ..fix(4_000, there)
        };
        tracker.fixed(0, respawned, None, &kin, None);
        tracker.fixed(0, dead(5_000), None, &kin, None);
        let new = tracker.facts(WorldFacts {
            time: 5_000,
            stats: stats([3, 0, 1]),
            ..WorldFacts::default()
        });
        assert_eq!(new.len(), 1);
        assert_eq!(new[0].at, there);
        // Statistics ahead of the body (the world still has it alive):
        // the death is where it stands, and the dead world after it adds
        // none.
        tracker.fixed(1, fix(6_000, Vec3::new(21.0, 0.0, 7.0)), None, &kin, None);
        let new = tracker.facts(WorldFacts {
            time: 6_000,
            stats: stats([3, 1, 1]),
            ..WorldFacts::default()
        });
        assert_eq!(new.len(), 1);
        assert_eq!(new[0].at, Vec3::new(21.0, 0.0, 7.0));
        tracker.fixed(1, dead(7_000), None, &kin, None);
        let new = tracker.facts(WorldFacts {
            time: 7_000,
            stats: stats([3, 1, 1]),
            ..WorldFacts::default()
        });
        assert!(new.is_empty());
    }
}
