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
//! - **Carriers:** a world says a battery is carried, not by whom
//!   ([`Carry`]). The carrier is the player whose use's ray passed nearest
//!   it lately (for a battery taken from a container, one of the other
//!   team), else the nearest; a player believed to carry another battery
//!   is none (a player holds one, `m_carried_item`). The others near stay
//!   candidates: when the believed carrier's uses at his own container put
//!   nothing in, or he died and the battery stayed carried, the next one
//!   has it. (Audit of the owner's matches, #2: a bot at its container
//!   pressing to put its battery in won a teammate's take beside it, the
//!   latest use, and tried nine times to put in a battery the teammate
//!   walked off with unaware.)

use std::collections::VecDeque;

use mapdata::{Vec3, World};

use super::body::{BodyTrack, Fix};
use super::kin::Kinematics;
use crate::v026g::drive::key;

/// A roster place.
pub type Place = usize;

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
    pub fn from_id(mode: u8) -> Self {
        use survarium::v026g::maps::mode;
        match mode {
            mode::TIMED_BATTERIES => Self::TimedBatteries,
            mode::TEAM_DEATHMATCH => Self::TeamDeathmatch,
            _ => Self::Batteries,
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
    pub team: u8,
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
    /// The time (ms) `feet` and `velocity` are for: a bot's last decided
    /// tick (up to [`super::encode::LEAD_TICKS`] past the clock), a human's
    /// reckoning to the clock. A bot's input for a tick runs then, so what
    /// it aims at is led from this to that tick (`reflex`, `lead_target`).
    pub known_at: u32,
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
    pub team: u8,
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
    /// Batteries home, or tickets in Team Deathmatch, by team.
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
    /// Whether each team may still respawn.
    pub can_respawn: [bool; 2],
    /// Timed batteries: when the next battery appears (match ms).
    pub next_battery: Option<u32>,
}

impl MatchView {
    pub fn place(&self, place: Place) -> Option<&PlaceView> {
        self.places.get(place)
    }

    /// Our points minus theirs, for `team` (tickets count a tenth).
    pub fn score_gap(&self, team: u8) -> f32 {
        let (ours, theirs) = (
            self.points[usize::from(team & 1)],
            self.points[usize::from(!team & 1)],
        );
        let gap = ours as f32 - theirs as f32;
        if self.mode == Mode::TeamDeathmatch {
            gap / 10.0
        } else {
            gap
        }
    }

    /// The living players of `team` (or not of it).
    pub fn team(&self, team: u8) -> impl Iterator<Item = &PlaceView> {
        self.places
            .iter()
            .filter(move |p| p.present && p.alive && p.team == team)
    }

    pub fn enemies_of(&self, team: u8) -> impl Iterator<Item = &PlaceView> {
        self.places
            .iter()
            .filter(move |p| p.present && p.alive && p.team != team)
    }

    pub fn container_of(&self, team: u8) -> Option<&ContainerView> {
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
/// The carriers' numbers. Each is a GUESS unless it says otherwise;
/// measured on the battery bench (`real_bot_cost`) and the whole-match
/// soaks, which check the belief against the stand-in's truth (`carry:`
/// lines), and on the audit's match 0x4da3b51d.
///
/// A use this near a battery (feet, metres), this recently (ms), makes the
/// user a candidate for its carrier.
const CARRIER_USE_M: f32 = 4.0;
const CARRIER_USE_MS: u32 = 6_000;
/// The uses kept of each place.
const USES_KEPT: usize = 4;
/// The client's use: a ray from the eye this long
/// (`base_player::detect_usable_object`, PROVEN).
const USE_RAY_M: f32 = 2.0;
/// A lying battery's middle over its place (its box is about a quarter of
/// a metre high), and a container's use box's over its place
/// (`plays::Look::use_box`), metres.
const BATTERY_MIDDLE_M: f32 = 0.12;
const USE_BOX_UP_M: f32 = 1.0;
/// A candidate's cost, metres: how far its use's ray passed the battery,
/// and this share of how far its feet were from it (the ray tells most:
/// a take passes within a fifth of a metre, a put-in at a container a
/// metre or more off a battery lying beside it)...
const CARRIER_FEET_SHARE: f32 = 0.25;
/// ... this more for a use pressed this long before the last world that
/// showed the battery lying (a take is instant on the key)...
const CARRIER_STALE_MS: u32 = 1_000;
const CARRIER_STALE_COST: f32 = 1.0;
/// ... and for a player who pressed no use within this of it, this more
/// than his distance (the old rule's nearest within 6 m).
const CARRIER_NEAR_M: f32 = 6.0;
const CARRIER_UNPRESSED_COST: f32 = 3.0;
/// The other candidates kept, likeliest first.
const CARRIER_OTHERS: usize = 4;
/// The believed carrier's uses at his own container (feet within this of
/// it, metres): after this many, each followed by a world this long after
/// it still showing the battery carried, he does not carry it, if another
/// candidate may (a put-in is a use on the key; the PutIn play presses
/// two a play, and is taken again).
const PUT_IN_NEAR_M: f32 = 2.5;
const PUT_IN_FAILS: usize = 3;
const PUT_IN_SEEN_MS: u32 = 1_500;
/// A believed carrier died, and the battery stayed carried this long
/// after: another candidate has it (the drop shows in the world after the
/// death's, about a second on).
const DEAD_CARRY_MS: u32 = 2_500;
/// A battery that fell this far from its believed carrier was not his
/// (a note), metres.
const FELL_OFF_M: f32 = 4.0;

/// A use pressed: when, where the feet were, and its ray (the eye and the
/// view's way).
#[derive(Clone, Copy, Debug, PartialEq)]
struct UsePress {
    time: u32,
    feet: Vec3,
    eye: Vec3,
    dir: Vec3,
}

impl UsePress {
    /// How far the use's ray passes `point`, metres.
    fn miss(&self, point: Vec3) -> f32 {
        let along = (point - self.eye).dot(self.dir).clamp(0.0, USE_RAY_M);
        (self.eye + self.dir * along - point).length()
    }
}

/// What the view believes of a carried battery's carrier.
#[derive(Clone, Debug, Default, PartialEq)]
struct Carry {
    carrier: Option<Place>,
    /// Who else may have it, likeliest first.
    others: Vec<Place>,
    /// Who it is not with: their put-ins failed, or they died and it
    /// stayed carried.
    not_with: Vec<Place>,
    /// The believed carrier's uses at his own container, ms.
    put_ins: Vec<u32>,
    /// Its believed carrier died, at the world's time.
    died: Option<u32>,
}

/// A candidate for a carrier: its cost, and its use's ray's miss and its
/// distance.
#[derive(Clone, Copy, Debug)]
struct Candidate {
    place: Place,
    cost: f32,
    miss: Option<f32>,
    distance: f32,
}

#[derive(Clone, Debug, Default)]
struct PlaceState {
    team: u8,
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
    /// The last few uses pressed.
    uses: VecDeque<UsePress>,
    last_shot: Option<u32>,
    /// The body's life (`BodyTrack::last_alive`) whose death the kill feed
    /// has a spot for: one spot a life.
    death_spotted: Option<u32>,
}

/// The view's keeper.
pub struct Tracker {
    places: Vec<PlaceState>,
    mode: Mode,
    /// Container id → (team, place on the map).
    containers: Vec<(u8, Vec3)>,
    batteries: Vec<BatteryState>,
    marks: Vec<u32>,
    carries: Vec<Carry>,
    /// The time of the last world that said where the batteries are.
    batteries_time: u32,
    /// What the carriers' belief came to, for the log ([`Tracker::take_notes`]).
    notes: Vec<String>,
    container_counts: Vec<usize>,
    points: [u32; 2],
    can_respawn: [bool; 2],
    next_battery: Option<u32>,
    sounds: VecDeque<Sound>,
    deaths: VecDeque<Death>,
    world_time: u32,
    /// The bench only: by place, a bot seen as a human is, its body known
    /// this many ms before the clock (his ping and the input's way) and
    /// reckoned on from there to the clock ([`BodyTrack::seen_late`]). The
    /// bench's bots are otherwise known to their decided tick, which no
    /// live human is.
    #[cfg(test)]
    pub seen_late: Option<Vec<u32>>,
}

impl Tracker {
    /// `teams[place]`, `bots[place]`; `containers[id]` = (team, place).
    pub fn new(mode: u8, teams: &[u8], bots: &[bool], containers: Vec<(u8, Vec3)>) -> Self {
        Self {
            places: teams
                .iter()
                .zip(bots)
                .map(|(&team, &bot)| PlaceState {
                    team,
                    bot,
                    health: 1.0,
                    ..PlaceState::default()
                })
                .collect(),
            mode: Mode::from_id(mode),
            container_counts: vec![0; containers.len()],
            containers,
            batteries: Vec::new(),
            marks: Vec::new(),
            carries: Vec::new(),
            batteries_time: 0,
            notes: Vec::new(),
            points: [0, 0],
            can_respawn: [true, true],
            next_battery: None,
            sounds: VecDeque::new(),
            deaths: VecDeque::new(),
            world_time: 0,
            #[cfg(test)]
            seen_late: None,
        }
    }

    /// The map's containers: by id, (team, place).
    pub fn set_containers(&mut self, containers: Vec<(u8, Vec3)>) {
        self.container_counts.resize(containers.len(), 0);
        self.containers = containers;
    }

    /// A place added (a late joiner).
    pub fn push_place(&mut self, team: u8) {
        self.places.push(PlaceState {
            team,
            health: 1.0,
            ..PlaceState::default()
        });
    }

    pub fn entered(&mut self, place: Place, entered: bool) {
        if let Some(state) = self.places.get_mut(place) {
            state.present = entered;
        }
    }

    pub fn body(&self, place: Place) -> Option<&BodyTrack> {
        self.places.get(place).map(|state| &state.body)
    }

    /// The speed level each place's profile gives (by place), taken for a
    /// body until a world shows its own (`BodyTrack::set_prior_level`).
    pub fn set_prior_levels(&mut self, levels: &[Option<f32>]) {
        for (state, &level) in self.places.iter_mut().zip(levels) {
            state.body.set_prior_level(level);
        }
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
        let Some(estimate) = state.body.estimate().copied() else {
            return;
        };
        let at = estimate.feet;
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
        }
        if pressed & key::JUMP != 0 {
            heard(SoundKind::Jump);
        }
        if pressed & key::USE != 0 {
            let up = if keys & key::CROUCH != 0 {
                EYE_CROUCHED
            } else {
                EYE_STANDING
            };
            let [fx, fz] = super::kin::forward(estimate.yaw);
            let (sin, cos) = estimate.view_pitch().sin_cos();
            self.used(
                place,
                UsePress {
                    time,
                    feet: at,
                    eye: at + Vec3::new(0.0, up, 0.0),
                    dir: Vec3::new(fx * cos, sin, fz * cos),
                },
            );
        }
    }

    /// A use of `place`: kept, and if he is believed to carry a battery
    /// and stands at his own container, a put-in of it.
    fn used(&mut self, place: Place, press: UsePress) {
        let Some(state) = self.places.get_mut(place) else {
            return;
        };
        if state.uses.len() >= USES_KEPT {
            state.uses.pop_front();
        }
        state.uses.push_back(press);
        let team = state.team;
        let home = self
            .containers
            .iter()
            .any(|&(t, at)| t == team && at.distance_xz(press.feet) <= PUT_IN_NEAR_M);
        if !home {
            return;
        }
        for (carry, battery) in self.carries.iter_mut().zip(&self.batteries) {
            if carry.carrier == Some(place) && matches!(battery, BatteryState::Carried(_)) {
                carry.put_ins.push(press.time);
            }
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
        self.carriers_seen(&facts.batteries, facts.time);
        self.batteries = facts.batteries;
        self.batteries_time = facts.time;
        self.marks = facts.marks;
        new_deaths
    }

    /// Who is believed to carry `battery` (a soak checks it against its
    /// stand-in's truth).
    pub fn carrier(&self, battery: usize) -> Option<Place> {
        self.carries.get(battery).and_then(|carry| carry.carrier)
    }

    /// What the carriers' belief came to since the last call: lines for
    /// the log.
    pub fn take_notes(&mut self) -> Vec<String> {
        std::mem::take(&mut self.notes)
    }

    /// The carriers, from a world's batteries (`self.batteries` is still
    /// the world before's).
    fn carriers_seen(&mut self, batteries: &[BatteryState], time: u32) {
        self.carries.resize_with(batteries.len(), Carry::default);
        // Batteries no longer carried; and their carriers' deaths.
        for (battery, state) in batteries.iter().enumerate() {
            let before = self.batteries.get(battery).copied();
            if !matches!(state, BatteryState::Carried(_)) {
                if let (BatteryState::Free(at), Some(BatteryState::Carried(_))) = (state, before)
                    && let Some(carrier) = self.carries[battery].carrier
                    && let Some(feet) = self.feet(carrier)
                    && feet.distance_xz(*at) > FELL_OFF_M
                {
                    self.notes.push(format!(
                        "battery {battery} fell {:.1} m from player {carrier}, believed its carrier",
                        feet.distance_xz(*at)
                    ));
                }
                self.carries[battery] = Carry::default();
                continue;
            }
            let carry = &mut self.carries[battery];
            if let Some(carrier) = carry.carrier
                && !self.places[carrier].alive
            {
                // A carrier who died has dropped it (the next world says
                // where), unless it was not his.
                carry.carrier = None;
                carry.not_with.push(carrier);
                carry.died = Some(time);
            }
        }
        // Batteries picked up since the world before.
        let mut picked = Vec::new();
        for (battery, state) in batteries.iter().enumerate() {
            if !matches!(state, BatteryState::Carried(_)) {
                continue;
            }
            let (point, thieves_of) = match self.batteries.get(battery).copied() {
                Some(BatteryState::Free(at)) => (at + Vec3::new(0.0, BATTERY_MIDDLE_M, 0.0), None),
                Some(BatteryState::Home(id)) => match self.containers.get(usize::from(id)) {
                    Some(&(team, at)) => (at + Vec3::new(0.0, USE_BOX_UP_M, 0.0), Some(team)),
                    None => continue,
                },
                _ => continue,
            };
            let mut candidates = self.candidates(point, thieves_of, time);
            let carry = &mut self.carries[battery];
            *carry = Carry::default();
            if candidates.is_empty() {
                self.notes.push(format!(
                    "battery {battery} picked up by nobody known near it"
                ));
                continue;
            }
            let best = candidates.remove(0);
            carry.carrier = Some(best.place);
            picked.push((battery, best.place));
            carry.others = candidates
                .iter()
                .map(|c| c.place)
                .take(CARRIER_OTHERS)
                .collect();
            let others: Vec<String> = candidates
                .iter()
                .take(CARRIER_OTHERS)
                .map(|c| format!("{} ({:.1})", c.place, c.cost))
                .collect();
            self.notes.push(format!(
                "battery {battery} picked up by {}: {}, {:.1} m away; the others near: {}",
                best.place,
                best.miss
                    .map_or("pressed no use".to_string(), |miss| format!(
                        "its use's ray {miss:.2} m off it"
                    )),
                best.distance,
                if others.is_empty() {
                    "none".to_string()
                } else {
                    others.join(", ")
                },
            ));
        }
        // One who picked up a battery did not have another (the game
        // gives a battery only to a player carrying nothing:
        // `victory_item_core::can_use` 0x300710): he is no other candidate
        // for any other carry, even once he has put his own in. In
        // 0x1b8ee975 one world showed batteries 6 and 8 picked up by bots
        // 6 and 4 side by side; bot 4 put 8 in, bot 6's put-ins failed, and
        // battery 6 was believed bot 4's for 35 s while bot 6 carried it.
        for &(battery, place) in &picked {
            for (other, carry) in self.carries.iter_mut().enumerate() {
                if other != battery {
                    carry.others.retain(|&p| p != place);
                }
            }
        }
        // Beliefs the batteries' staying carried rule out.
        for (battery, state) in batteries.iter().enumerate() {
            if !matches!(state, BatteryState::Carried(_)) {
                continue;
            }
            let carry = &self.carries[battery];
            let why = match (carry.carrier, carry.died) {
                (Some(_), _)
                    if carry
                        .put_ins
                        .iter()
                        .filter(|&&t| t + PUT_IN_SEEN_MS <= time)
                        .count()
                        >= PUT_IN_FAILS =>
                {
                    format!(
                        "{} uses at his container put nothing in",
                        carry.put_ins.len()
                    )
                }
                (None, Some(died)) if died + DEAD_CARRY_MS <= time => {
                    "he died and it stayed carried".to_string()
                }
                _ => continue,
            };
            let was = carry.carrier.or(carry.not_with.last().copied());
            let next = carry.others.iter().copied().find(|&place| {
                !carry.not_with.contains(&place)
                    && Some(place) != carry.carrier
                    && self.places.get(place).is_some_and(|s| s.alive)
                    && !self.carries.iter().any(|c| c.carrier == Some(place))
            });
            let carry = &mut self.carries[battery];
            carry.put_ins.clear();
            carry.died = None;
            match (next, carry.carrier) {
                (Some(next), Some(was)) => {
                    carry.not_with.push(was);
                    carry.carrier = Some(next);
                }
                (Some(next), None) => carry.carrier = Some(next),
                // No one else: the belief stays.
                (None, _) => {}
            }
            self.notes.push(format!(
                "player {} no longer believed to carry battery {battery} ({why}); now {}",
                was.map_or("nobody".to_string(), |p| p.to_string()),
                match (next, carry.carrier) {
                    (Some(next), _) => format!("player {next}"),
                    (None, Some(still)) => format!("still player {still}, no one else near it"),
                    (None, None) => "nobody known".to_string(),
                }
            ));
        }
    }

    /// The feet of `place` as last reckoned.
    fn feet(&self, place: Place) -> Option<Vec3> {
        Some(self.places.get(place)?.body.estimate()?.feet)
    }

    /// Who may have picked up a battery at `point` by the world at `time`,
    /// likeliest first: the living, near it, not believed to carry another
    /// battery, and for one taken from a container of `thieves_of`'s, not
    /// of that team.
    fn candidates(&self, point: Vec3, thieves_of: Option<u8>, time: u32) -> Vec<Candidate> {
        let lying_seen = self.batteries_time;
        let mut candidates: Vec<Candidate> = self
            .places
            .iter()
            .enumerate()
            .filter(|(place, state)| {
                state.alive
                    && thieves_of != Some(state.team)
                    && !self.carries.iter().any(|c| c.carrier == Some(*place))
            })
            .filter_map(|(place, state)| {
                let used = state
                    .uses
                    .iter()
                    .filter(|u| {
                        u.time + CARRIER_USE_MS >= time && u.feet.distance_xz(point) < CARRIER_USE_M
                    })
                    .map(|u| {
                        let miss = u.miss(point);
                        let distance = u.feet.distance_xz(point);
                        let stale = if u.time + CARRIER_STALE_MS < lying_seen {
                            CARRIER_STALE_COST
                        } else {
                            0.0
                        };
                        Candidate {
                            place,
                            cost: miss + CARRIER_FEET_SHARE * distance + stale,
                            miss: Some(miss),
                            distance,
                        }
                    })
                    .min_by(|a, b| a.cost.total_cmp(&b.cost));
                used.or_else(|| {
                    let distance = state.body.estimate()?.feet.distance_xz(point);
                    (distance < CARRIER_NEAR_M).then_some(Candidate {
                        place,
                        cost: CARRIER_UNPRESSED_COST + distance,
                        miss: None,
                        distance,
                    })
                })
            })
            .collect();
        candidates.sort_by(|a, b| a.cost.total_cmp(&b.cost));
        candidates
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
                    let own = state.body.estimate().copied();
                    // The bench's bots seen as humans are (`seen_late`).
                    #[cfg(test)]
                    let own = match self.seen_late.as_ref().and_then(|late| late.get(place)) {
                        Some(&late) => own.map(|e| state.body.seen_late(e, clock.clock, late)),
                        None => own,
                    };
                    own
                } else {
                    state
                        .body
                        .at(clock.clock, kin, world.map(|world| &*world.grid))
                };
                let carrying = self
                    .carries
                    .iter()
                    .position(|carry| carry.carrier == Some(place));
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
                    known_at: estimate.map_or(clock.clock, |e| e.time),
                }
            })
            .collect();
        let batteries = self
            .batteries
            .iter()
            .enumerate()
            .map(|(battery, state)| match state {
                BatteryState::Carried(_) => BatteryState::Carried(self.carrier(battery)),
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
        let mut tracker = Tracker::new(2, &[0, 1], &[true, false], vec![]);
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
            2,
            &[0, 1],
            &[false, false],
            vec![(0, Vec3::ZERO), (1, Vec3::new(50.0, 0.0, 0.0))],
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
        let mut tracker = Tracker::new(2, &[0, 0, 1], &[true, true, false], vec![]);
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

    /// The audit's match 0x4da3b51d at 206 s, two bots of team 1 at
    /// their container (57, 44): one at it, pressing use at its use box;
    /// one a metre beside it, crouched, pressing use at a battery lying
    /// there, which a world later is carried.
    struct AtTheContainer {
        kin: Kinematics,
        tracker: Tracker,
    }

    const CONTAINER: Vec3 = Vec3::new(57.0, 0.0, 44.0);
    /// At the container, facing it (-z).
    const AT_IT: Vec3 = Vec3::new(57.0, 0.0, 45.2);
    /// Beside it, facing the battery (+z) a metre on.
    const BESIDE: Vec3 = Vec3::new(58.5, 0.0, 45.0);
    const BATTERY: Vec3 = Vec3::new(58.5, 0.0, 46.0);

    impl AtTheContainer {
        /// Places 0 and 1 the two bots, 2 an enemy far off.
        fn new() -> Self {
            let kin = Kinematics::default();
            let mut tracker = Tracker::new(
                2,
                &[1, 1, 0],
                &[true, true, true],
                vec![(0, Vec3::new(12.0, 0.0, 30.0)), (1, CONTAINER)],
            );
            for place in 0..3 {
                tracker.entered(place, true);
            }
            let mut at = Self { kin, tracker };
            at.stand(
                0,
                1_000,
                AT_IT,
                std::f32::consts::PI,
                (1.0 - 1.6f32).atan2(1.2),
            );
            at.stand(1, 1_000, BESIDE, 0.0, (0.12 - 0.95f32).atan2(1.0));
            at.stand(2, 1_000, Vec3::new(12.0, 0.0, 30.0), 0.0, 0.0);
            at.world(1_000, vec![BatteryState::Free(BATTERY)]);
            at
        }

        fn stand(&mut self, place: Place, time: u32, feet: Vec3, yaw: f32, pitch: f32) {
            let fix = Fix {
                yaw,
                pitch,
                ..fix(time, feet)
            };
            self.tracker.fixed(place, fix, None, &self.kin, None);
        }

        fn press(&mut self, place: Place, time: u32, keys: u32) {
            let kin = &self.kin;
            self.tracker
                .input(place, time, keys | key::USE, (0.0, 0.0), 0.0, kin, None);
            self.tracker
                .input(place, time + 50, keys, (0.0, 0.0), 0.0, kin, None);
        }

        fn world(&mut self, time: u32, batteries: Vec<BatteryState>) {
            self.tracker.facts(WorldFacts {
                time,
                batteries,
                ..WorldFacts::default()
            });
        }
    }

    /// The bot beside the battery whose use's ray passed through it takes
    /// it, not the one at the container who pressed later (the old rule:
    /// the latest use within 4 m).
    #[test]
    fn of_two_bots_at_their_container_the_one_whose_use_met_the_battery_carries_it() {
        let mut at = AtTheContainer::new();
        at.press(1, 1_100, key::CROUCH);
        at.press(0, 1_600, 0);
        at.world(2_000, vec![BatteryState::Carried(None)]);
        assert_eq!(at.tracker.carrier(0), Some(1));
        let notes = at.tracker.take_notes();
        assert!(
            notes
                .iter()
                .any(|n| n.starts_with("battery 0 picked up by 1: its use's ray")),
            "{notes:?}"
        );
        let view = at.tracker.view(
            &Clock {
                clock: 2_000,
                time_left_s: 100.0,
                total_s: 900.0,
                playing: true,
            },
            &at.kin,
            None,
        );
        assert_eq!(view.place(1).unwrap().carrying, Some(0));
        assert_eq!(view.place(0).unwrap().carrying, None);
    }

    /// A player carries one battery: the bot at the container already
    /// carrying one is not the taker of a second one beside it, however
    /// its use went.
    #[test]
    fn a_player_believed_to_carry_a_battery_takes_no_second_one() {
        let mut at = AtTheContainer::new();
        // Place 0 brought battery 1 and puts it in: its uses at the
        // container are as near the lying battery as its teammate's.
        at.world(
            1_200,
            vec![BatteryState::Free(BATTERY), BatteryState::Free(AT_IT)],
        );
        at.press(0, 1_300, 0);
        at.world(
            1_400,
            vec![BatteryState::Free(BATTERY), BatteryState::Carried(None)],
        );
        assert_eq!(at.tracker.carrier(1), Some(0));
        // Place 1 presses facing away: its ray misses the battery by more
        // than place 0's, which turned to it.
        at.stand(1, 1_500, BESIDE, std::f32::consts::PI, 0.3);
        at.press(1, 1_600, key::CROUCH);
        let to = BATTERY - AT_IT;
        at.stand(
            0,
            1_650,
            AT_IT,
            (-to.x).atan2(to.z),
            (0.12 - 1.6f32).atan2(to.length()),
        );
        at.press(0, 1_700, 0);
        at.world(
            2_000,
            vec![BatteryState::Carried(None), BatteryState::Carried(None)],
        );
        assert_eq!(at.tracker.carrier(0), Some(1));
        assert_eq!(at.tracker.carrier(1), Some(0));
    }

    /// Two batteries picked up in one world side by side (0x1b8ee975,
    /// 607 s): the one who put his in is no candidate for the other's
    /// carry when the other's put-ins fail (the game gives a battery only
    /// to a player carrying nothing), so the belief stays with its taker.
    #[test]
    fn one_who_picked_up_another_battery_is_not_believed_to_carry_this_one() {
        let mut at = AtTheContainer::new();
        at.world(
            1_200,
            vec![BatteryState::Free(BATTERY), BatteryState::Free(AT_IT)],
        );
        at.press(1, 1_300, key::CROUCH);
        at.press(0, 1_300, 0);
        at.world(
            1_400,
            vec![BatteryState::Carried(None), BatteryState::Carried(None)],
        );
        assert_eq!(at.tracker.carrier(0), Some(1));
        assert_eq!(at.tracker.carrier(1), Some(0));
        // Place 0 puts battery 1 in; place 1's uses there put nothing in.
        at.world(
            2_000,
            vec![BatteryState::Carried(None), BatteryState::Home(1)],
        );
        at.tracker.take_notes();
        for time in [2_400, 3_600, 4_800] {
            at.press(1, time, key::CROUCH);
            at.world(
                time + 1_000,
                vec![BatteryState::Carried(None), BatteryState::Home(1)],
            );
        }
        at.world(
            6_400,
            vec![BatteryState::Carried(None), BatteryState::Home(1)],
        );
        assert_eq!(at.tracker.carrier(0), Some(1));
        let notes = at.tracker.take_notes();
        assert!(
            notes
                .iter()
                .any(|n| n.ends_with("now still player 1, no one else near it")),
            "{notes:?}"
        );
    }

    /// The belief went to the wrong teammate (the taker's use was not
    /// relayed): its uses at its own container put nothing in, so the
    /// other has it; and a believed carrier who died while the battery
    /// stayed carried did not carry it.
    #[test]
    fn a_belief_moves_on_when_put_ins_fail_or_its_carrier_dies_with_it_still_carried() {
        let mut at = AtTheContainer::new();
        at.press(0, 1_600, 0);
        at.world(2_000, vec![BatteryState::Carried(None)]);
        assert_eq!(at.tracker.carrier(0), Some(0), "the only use near it");
        at.tracker.take_notes();
        for (i, time) in [2_400, 3_600, 4_800].into_iter().enumerate() {
            at.press(0, time, 0);
            at.world(time + 1_000, vec![BatteryState::Carried(None)]);
            assert_eq!(at.tracker.carrier(0), Some(0), "use {i} not yet judged");
        }
        at.world(6_400, vec![BatteryState::Carried(None)]);
        assert_eq!(at.tracker.carrier(0), Some(1));
        let notes = at.tracker.take_notes();
        assert!(
            notes.iter().any(|n| n
                == "player 0 no longer believed to carry battery 0 (3 uses at his container \
                    put nothing in); now player 1"),
            "{notes:?}"
        );
        // Place 1 dies; the battery stays carried: place 0 has it after
        // all (the only other one near, ruled out before, is not taken
        // again), so the belief is nobody.
        let mut dead = fix(7_000, BESIDE);
        dead.alive = false;
        dead.feet = None;
        at.tracker.fixed(1, dead, None, &at.kin, None);
        at.world(7_000, vec![BatteryState::Carried(None)]);
        assert_eq!(at.tracker.carrier(0), None);
        at.world(10_000, vec![BatteryState::Carried(None)]);
        assert_eq!(at.tracker.carrier(0), None);
    }
}
