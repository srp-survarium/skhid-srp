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

use super::arc::{self, Flight, Grenade, Mesh};
use super::body::{BodyTrack, Fix};
use super::kin::Kinematics;
use super::live_grenades::{
    LiveGrenade, LAUNCH_MS, Phase, Pocketed, Pockets, SAME_THROW_MS, THROW_KEYS, ThrowCounts,
};
use crate::v026g::drive::key;

/// A roster place.
pub type Place = usize;

/// Where the game's bullet starts, over the feet, metres: the player's head
/// matrix (`player+0x44C`, the Head locator of its animation), which
/// `weapon_core::instant_fire` (0x2E23A0) copies as the bullet's transform
/// and `detect_usable_object` (0x2D84C0) takes the use ray from (PROVEN, the
/// exe). It is an animated bone, so the height depends on the pose. Live
/// bullets of 23 recorded matches (`fact-own-eye.md` 3, 2,591 of them, the
/// body's world position of the same tick):
///
/// - standing: 1.470 m level (sd 0.03), 1.460 + 0.160 sin(pitch) over the
///   2,053 rows (PROVEN); the same running, walking and still;
/// - crouched and moving: 1.14 + 0.22 sin(pitch) over the 385 rows under
///   1.3 m (PROVEN for the height, LIKELY for the pitch term: the sample is
///   small; the 0.30 of all 493 rows holds the 17 % of rows above 1.3 m,
///   the capsule flag flipping before the head follows);
/// - crouched and still: 1.04 m (n = 24, PROVEN for the height; the pitch
///   term is the moving one's, GUESS), 1.11 m at 0.1-0.5 m/s.
///
/// The old 1.6 and 0.95 m (GUESS from the capsule) were 0.13 m too high
/// standing and 0.1-0.19 m too low crouched.
const EYE_STANDING_M: f32 = 1.46;
const EYE_STANDING_PITCH_M: f32 = 0.16;
const EYE_CROUCHED_M: f32 = 1.14;
const EYE_CROUCHED_STILL_M: f32 = 1.04;
const EYE_CROUCHED_PITCH_M: f32 = 0.22;
/// Slower than this a crouched body counts as still, m/s (the data's
/// class: 24 rows under 0.1 m/s sit at 1.04, those at 0.1-0.5 m/s at 1.11).
const EYE_STILL_MPS: f32 = 0.1;

/// The heights before the measurement (GUESS from the capsule: 1.8 m
/// standing, 1.1 m crouched), kept for the off switch
/// (`CombatTuning::live_eye`).
const EYE_GUESSED_STANDING_M: f32 = 1.6;
const EYE_GUESSED_CROUCHED_M: f32 = 0.95;

thread_local! {
    static LIVE_EYE: std::cell::Cell<bool> = const { std::cell::Cell::new(true) };
}

/// Whether the eye is the game's ([`eye_height`], the stand ranges, the
/// look's re-look, the stand-in's shot order) or the old one (the off
/// switch of the fix, `CombatTuning::live_eye`). `Bots` sets it for its
/// thread at each call; on by default.
pub fn live_eye() -> bool {
    LIVE_EYE.with(|on| on.get())
}

/// Sets [`live_eye`] for this thread.
pub fn set_live_eye(on: bool) {
    LIVE_EYE.with(|cell| cell.set(on));
}

/// The height of the eyes (the bullet's origin) over the feet: `crouched`
/// or standing, at `speed` (m/s, ground) with the view at `pitch` (radians,
/// up is positive). The one place the crate gets an eye from: a body's own
/// ([`PlaceView::eye`]), a target's head in a sight test, a cover spot's.
pub fn eye_height(crouched: bool, speed: f32, pitch: f32) -> f32 {
    if !live_eye() {
        return if crouched {
            EYE_GUESSED_CROUCHED_M
        } else {
            EYE_GUESSED_STANDING_M
        };
    }
    let sin = pitch.sin();
    if !crouched {
        EYE_STANDING_M + EYE_STANDING_PITCH_M * sin
    } else if speed < EYE_STILL_MPS {
        EYE_CROUCHED_STILL_M + EYE_CROUCHED_PITCH_M * sin
    } else {
        EYE_CROUCHED_M + EYE_CROUCHED_PITCH_M * sin
    }
}

/// The eye of a body at `feet` that holds still and looks level, standing
/// or `crouched`: a cover spot, a stand, a track (nothing is known of its
/// pose).
pub fn still_eye(feet: Vec3, crouched: bool) -> Vec3 {
    feet + Vec3::new(0.0, eye_height(crouched, 0.0, 0.0), 0.0)
}
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

    /// Team Deathmatch: tickets and a fight to the last body, no batteries.
    pub fn is_deathmatch(self) -> bool {
        self == Self::TeamDeathmatch
    }

    /// The key of a play's weight for this mode (`weight_tdm`,
    /// `weight_batteries`, `weight_timed`), for the plays that weigh the
    /// timed batteries apart from the plain ones (Cut, HoldExit, Crossfire,
    /// Flank).
    pub fn weight_key(self) -> &'static str {
        match self {
            Self::TeamDeathmatch => "weight_tdm",
            Self::Batteries => "weight_batteries",
            Self::TimedBatteries => "weight_timed",
        }
    }

    /// Timed batteries: the batteries come on a clock to charging stations
    /// (Station), and are not lying about to be found.
    pub fn has_stations(self) -> bool {
        self == Self::TimedBatteries
    }

    /// Whether a spawner's spot seen empty is worth looking at again after
    /// a time (`spawner_recheck_ms`): only where the batteries come on a
    /// clock, not where they once spawned.
    pub fn rechecks_spawners(self) -> bool {
        self == Self::TimedBatteries
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
    /// A bear trap went off under a body (`Tracker::snap`): the sound is the
    /// body's, heard at the loudness of its steps.
    Trap,
}

/// A sound a player made.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sound {
    pub from: Place,
    pub kind: SoundKind,
    pub at: Vec3,
    pub time: u32,
    /// A shot's reach by the weapon that fired it, metres
    /// ([`super::shot_reach`]), where the shooter's weapon 1 is known.
    pub reach_m: Option<f32>,
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
    /// For a player: how late his inputs reach the server, ms (a running
    /// mean over his last accepted inputs, `Tracker::lateness`); 0 for a
    /// bot and until his first input. Not the world's age (`seen_at`).
    pub late_ms: u32,
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
        let up = eye_height(self.crouched(), self.speed(), self.pitch);
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
    /// The fragmentation grenades thrown and not yet gone off, from the
    /// throw keys the server saw ([`super::live_grenades`]).
    pub grenades: Vec<LiveGrenade>,
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
        if self.mode.is_deathmatch() {
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
/// candidate whose own use met the battery may (a put-in is a use on the
/// key; the PutIn play presses two a play, and is taken again).
/// The wait covers the put (`states[2].put_time` 0.6 s; the battery goes
/// in at its end) and the world's lag: at 1.5 s, in the owner's Cologne
/// match of 2026-10-24 the belief moved off the real carrier 1 s before
/// each of 3 put-ins went in, once to a bot 8 m off that had pressed
/// nothing (GUESS, 0.6 s + 2 s + slack).
const PUT_IN_NEAR_M: f32 = 2.5;
const PUT_IN_FAILS: usize = 3;
const PUT_IN_SEEN_MS: u32 = 3_000;
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
    /// Who else may have it, likeliest first, and whether their own use
    /// met it (else only near it).
    others: Vec<(Place, bool)>,
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
    /// A running mean of how late his inputs came, ms.
    late: Option<f32>,
    alive: bool,
    health: f32,
    spawned: u32,
    stats: (u32, u32, u32),
    /// The last few uses pressed.
    uses: VecDeque<UsePress>,
    /// A human's pockets (its profile's first kit), for the throw keys; a
    /// server bot's throw key is always a frag.
    pockets: Option<Pockets>,
    /// The press of its last throw ([`SAME_THROW_MS`]).
    last_throw: Option<u32>,
    last_shot: Option<u32>,
    /// The body's life (`BodyTrack::last_alive`) whose death the kill feed
    /// has a spot for: one spot a life.
    death_spotted: Option<u32>,
}

/// A player's lateness is the mean of about this many of his last inputs
/// (a second or so of play).
const LATE_MEAN_INPUTS: f32 = 16.0;

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
    grenades: Vec<LiveGrenade>,
    throws: ThrowCounts,
    sounds: VecDeque<Sound>,
    /// By place, the reach of a shot of its weapon 1 ([`super::shot_reach`]),
    /// where known.
    shot_reach: Vec<Option<f32>>,
    deaths: VecDeque<Death>,
    world_time: u32,
    /// The bench only: by place, a bot seen as a human is, its body known
    /// this many ms before the clock (his ping and the input's way) and
    /// reckoned on from there to the clock ([`BodyTrack::seen_late`]). The
    /// bench's bots are otherwise known to their decided tick, which no
    /// live human is.
    #[cfg(test)]
    pub seen_late: Option<Vec<u32>>,
    /// The bench only (`SRP_BENCH_PING_JITTER`): each place's `seen_late`
    /// is off by a random walk within this many ms either way, one step a
    /// tick of at most [`JITTER_STEP_MS`] (the live lateness changes by
    /// 14 ms a tick at the 99th percentile), from the seed. Spikes of
    /// 150 ms are the live 90th percentile.
    #[cfg(test)]
    pub seen_jitter: (u32, u32),
    /// The walks: by place, the tick last drawn for and the offset there.
    #[cfg(test)]
    jitter_walks: std::cell::RefCell<Vec<(u32, i32)>>,
    /// The bench only: by place, the lateness above which the relay
    /// discards an input (`Game::late_relay_reach`): a body late by more is
    /// shown as it is, with no error, for the input never came (nor to
    /// the game's other clients).
    #[cfg(test)]
    pub seen_reach: Vec<u32>,
}

/// The most a bench jitter walk moves in a tick, ms.
#[cfg(test)]
pub const JITTER_STEP_MS: i32 = 14;

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
            grenades: Vec::new(),
            throws: ThrowCounts::default(),
            sounds: VecDeque::new(),
            shot_reach: Vec::new(),
            deaths: VecDeque::new(),
            world_time: 0,
            #[cfg(test)]
            seen_late: None,
            #[cfg(test)]
            seen_jitter: (0, 0),
            #[cfg(test)]
            jitter_walks: Default::default(),
            #[cfg(test)]
            seen_reach: Vec::new(),
        }
    }

    /// The map's containers: by id, (team, place).
    pub fn set_containers(&mut self, containers: Vec<(u8, Vec3)>) {
        self.container_counts.resize(containers.len(), 0);
        self.containers = containers;
    }

    /// The jitter of a place's ping at `clock`, ms: a random walk within
    /// `seen_jitter`, a step at most [`JITTER_STEP_MS`] each tick, the same
    /// for a place and a seed, advanced to the tick of `clock` (asking again
    /// for it gives the same).
    #[cfg(test)]
    fn jitter(&self, place: usize, clock: u32) -> i32 {
        let (ms, seed) = self.seen_jitter;
        if ms == 0 {
            return 0;
        }
        let bound = ms as i32;
        let tick = clock / 50;
        let mut walks = self.jitter_walks.borrow_mut();
        if walks.len() <= place {
            walks.resize(place + 1, (tick, 0));
        }
        let (mut at, mut offset) = walks[place];
        while at < tick {
            at += 1;
            let mut z = u64::from(seed) << 32 | u64::from(at);
            z = z.wrapping_add((place as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15));
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^= z >> 31;
            let step = (z % (2 * JITTER_STEP_MS as u64 + 1)) as i32 - JITTER_STEP_MS;
            offset = (offset + step).clamp(-bound, bound);
        }
        walks[place] = (at, offset);
        offset
    }

    /// A player's input reached the server `late_ms` after its tick: his
    /// running mean of that (each input weighs 1/[`LATE_MEAN_INPUTS`]),
    /// which the view gives as [`PlaceView::late_ms`]. Called for an input
    /// the server accepted, not one it discarded for being later than
    /// the relay's reach.
    pub fn lateness(&mut self, place: Place, late_ms: u32) {
        if let Some(state) = self.places.get_mut(place) {
            let late = late_ms as f32;
            state.late = Some(state.late.map_or(late, |mean| {
                mean + (late - mean) / LATE_MEAN_INPUTS
            }));
        }
    }

    /// TEST ONLY: the tick of the last input taken for `place`.
    #[cfg(test)]
    pub fn last_input(&self, place: Place) -> Option<u32> {
        self.places.get(place).and_then(|state| state.last_input)
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

    /// Every place's shot reach, metres ([`super::shot_reach`]), by place.
    pub fn set_shot_reach(&mut self, reach: Vec<Option<f32>>) {
        self.shot_reach = reach;
    }

    /// The speed level each place's profile gives (by place), taken for a
    /// body until a world shows its own (`BodyTrack::set_prior_level`).
    pub fn set_prior_levels(&mut self, levels: &[Option<f32>]) {
        for (state, &level) in self.places.iter_mut().zip(levels) {
            state.body.set_prior_level(level);
        }
    }

    /// Each place's pockets by place (a human's: its profile's first kit),
    /// for the throw keys; `None` for a server bot and for a profile that
    /// did not read.
    pub fn set_pockets(&mut self, pockets: Vec<Option<Pockets>>) {
        for (state, pockets) in self.places.iter_mut().zip(pockets) {
            state.pockets = pockets;
        }
    }

    /// The fragmentation grenades in the match now.
    pub fn live_grenades(&self) -> &[LiveGrenade] {
        &self.grenades
    }

    /// What the throw keys came to so far.
    pub fn throw_counts(&self) -> ThrowCounts {
        self.throws
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
        let reach_m = self.shot_reach.get(place).copied().flatten();
        let mut heard = |kind| {
            self.sounds.push_back(Sound {
                from: place,
                kind,
                at,
                time,
                reach_m: (kind == SoundKind::Shot).then_some(reach_m).flatten(),
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
        if pressed & THROW_KEYS != 0
            && state.last_throw.is_none_or(|last| time >= last + SAME_THROW_MS)
        {
            let thrown = if state.bot {
                (pressed & key::GRENADE != 0).then_some((Pocketed::Frag, *Grenade::bots()))
            } else if let Some(pockets) = state.pockets.as_mut() {
                pockets.throw_grenade(pressed)
            } else {
                self.throws.no_pockets += 1;
                None
            };
            match thrown {
                Some((Pocketed::Frag, grenade)) => {
                    self.throws.frag += 1;
                    state.last_throw = Some(time);
                    self.grenades.push(LiveGrenade {
                        by: place,
                        team: state.team,
                        press: time,
                        phase: Phase::Pressed,
                        grenade,
                    });
                }
                Some((Pocketed::Smoke, _)) => {
                    self.throws.smoke += 1;
                    state.last_throw = Some(time);
                }
                Some((Pocketed::Other, _)) | None => {}
            }
        }
        if pressed & key::USE != 0 {
            let pitch = estimate.view_pitch();
            let speed = estimate.velocity[0].hypot(estimate.velocity[1]);
            let up = eye_height(keys & key::CROUCH != 0, speed, pitch);
            let [fx, fz] = super::kin::forward(estimate.yaw);
            let (sin, cos) = pitch.sin_cos();
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

    /// The grenades due to leave the hand at `clock` get their landing: the
    /// arc model from the thrower's body as reckoned now, over the map's
    /// bullet mesh (`arc::Flight::landing`); entries past their blast go.
    /// With no map a grenade stays pressed.
    pub fn launch_grenades(&mut self, clock: u32, world: Option<&World>) {
        let grenades = std::mem::take(&mut self.grenades);
        let mut kept = Vec::with_capacity(grenades.len());
        for mut grenade in grenades {
            if !grenade.live_at(clock) {
                continue;
            }
            if grenade.phase == Phase::Pressed
                && clock >= grenade.press + LAUNCH_MS
                && let Some(world) = world
            {
                match self.launch_of(&grenade, world) {
                    Launch::Dead => {
                        self.throws.died_throwing += 1;
                        continue;
                    }
                    Launch::NoLanding => {
                        self.throws.unplaced += 1;
                        continue;
                    }
                    Launch::Lands(landing) => {
                        let launched = grenade.press + LAUNCH_MS;
                        grenade.phase = Phase::Flying {
                            launched,
                            lands: launched + (landing.contact.t * 1000.0) as u32,
                            rest: landing.rest,
                        };
                        self.throws.launched += 1;
                    }
                }
            }
            kept.push(grenade);
        }
        self.grenades = kept;
    }

    /// Where the grenade `grenade` let go lands, from its thrower's body now.
    fn launch_of(&self, grenade: &LiveGrenade, world: &World) -> Launch {
        let Some(state) = self.places.get(grenade.by).filter(|state| state.alive) else {
            return Launch::Dead;
        };
        let Some(e) = state.body.estimate().copied() else {
            return Launch::Dead;
        };
        let pitch = e.view_pitch();
        let speed = e.velocity[0].hypot(e.velocity[1]);
        let up = eye_height(e.keys & key::CROUCH != 0, speed, pitch);
        let eye = e.feet + Vec3::new(0.0, up, 0.0);
        match Flight::thrown_by(grenade.grenade, eye, e.yaw, pitch).landing(&Mesh(&world.sight)) {
            Some(landing) => Launch::Lands(landing),
            None => Launch::NoLanding,
        }
    }

    /// The sounds of keys held: fire held is a shot every 150 ms, a sprint
    /// is heard while it lasts (a sound a poll, at most every 500 ms).
    /// Sounds the held keys make. `sprint_body_mps` (`SenseTuning::
    /// sprint_body_mps`): `None`, SPRINT and FORWARD held make the sprint
    /// sound; `Some(v)`, only a body that also sprints by the client's key
    /// rule ([`Kinematics::sprints`]) and moves at `v` m/s or more.
    /// The alive body nearest `at` within `within` metres, if any: whose
    /// foot a trap that went off there was under.
    pub fn nearest_body(&self, at: Vec3, within: f32) -> Option<Place> {
        self.places
            .iter()
            .enumerate()
            .filter(|(_, state)| state.present && state.alive)
            .filter_map(|(place, state)| {
                let feet = state.body.estimate()?.feet;
                let d = feet.distance_xz(at);
                (d <= within && (feet.y - at.y).abs() < 2.0).then_some((place, d))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(place, _)| place)
    }

    /// A trap went off under body `victim` at `at`: a sound of its own, heard
    /// by the enemies of the body like its steps.
    pub fn snap(&mut self, victim: Place, at: Vec3, time: u32) {
        self.sounds.push_back(Sound {
            from: victim,
            kind: SoundKind::Trap,
            at,
            time,
            // A trap's snap has no weapon reach.
            reach_m: None,
        });
    }

    pub fn held_sounds(&mut self, clock: u32, sprint_body_mps: Option<f32>) {
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
                    reach_m: self.shot_reach.get(place).copied().flatten(),
                });
            }
            let sprinting = match sprint_body_mps {
                None => state.keys & key::SPRINT != 0 && state.keys & key::FORWARD != 0,
                Some(least) => {
                    let [vx, vz] = state.body.estimate().map_or([0.0; 2], |e| e.velocity);
                    Kinematics::sprints(state.keys) && vx.hypot(vz) >= least
                }
            };
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
                    reach_m: None,
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
        if state.spawned != fix.inserted
            && let Some(pockets) = state.pockets.as_mut()
        {
            pockets.refill();
        }
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
                .map(|c| (c.place, c.miss.is_some()))
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
                    carry.others.retain(|&(p, _)| p != place);
                }
            }
        }
        // Beliefs the batteries' staying carried rule out.
        for (battery, state) in batteries.iter().enumerate() {
            if !matches!(state, BatteryState::Carried(_)) {
                continue;
            }
            let carry = &self.carries[battery];
            // Failed put-ins tell only that it is not his: another takes the
            // belief only if his own use met it. A death with it still
            // carried tells someone has it: the likeliest.
            let (why, pressed_only) = match (carry.carrier, carry.died) {
                (Some(_), _)
                    if carry
                        .put_ins
                        .iter()
                        .filter(|&&t| t + PUT_IN_SEEN_MS <= time)
                        .count()
                        >= PUT_IN_FAILS =>
                {
                    (
                        format!(
                            "{} uses at his container put nothing in",
                            carry.put_ins.len()
                        ),
                        true,
                    )
                }
                (None, Some(died)) if died + DEAD_CARRY_MS <= time => {
                    ("he died and it stayed carried".to_string(), false)
                }
                _ => continue,
            };
            let was = carry.carrier.or(carry.not_with.last().copied());
            let next = carry.others.iter().copied().find_map(|(place, pressed)| {
                (!carry.not_with.contains(&place)
                    && (pressed || !pressed_only)
                    && Some(place) != carry.carrier
                    && self.places.get(place).is_some_and(|s| s.alive)
                    && !self.carries.iter().any(|c| c.carrier == Some(place)))
                .then_some(place)
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
                    (None, Some(still)) => format!(
                        "still player {still}, no one else {}",
                        if pressed_only {
                            "whose use met it"
                        } else {
                            "near it"
                        }
                    ),
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
                        Some(&late) => {
                            let late = late.saturating_add_signed(self.jitter(place, clock.clock));
                            // Later than the relay takes: never came.
                            let late = if self.seen_reach.get(place).is_some_and(|&r| late > r) {
                                0
                            } else {
                                late
                            };
                            own.map(|e| state.body.seen_late(e, clock.clock, late))
                        }
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
                    late_ms: state.late.map_or(0, |late| late.round() as u32),
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
            grenades: self.grenades.clone(),
        }
    }
}

/// What a grenade's launch came to.
enum Launch {
    /// The thrower is dead or not placed: nothing was thrown.
    Dead,
    /// The throw touches no ground within the flight's time.
    NoLanding,
    Lands(arc::Landing),
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

    /// A human's body at place 1 that holds `keys` for `ms` ms from 1.05 s
    /// on, then `held_sounds` at its end with `by_body`: the sprint sounds
    /// made in all.
    fn sprint_sounds(keys: u32, ms: u32, by_body: Option<f32>) -> usize {
        let kin = Kinematics::default();
        let mut tracker = Tracker::new(2, &[0, 1], &[true, false], vec![]);
        tracker.entered(1, true);
        assert!(tracker.fixed(1, fix(1_000, Vec3::new(5.0, 0.0, 5.0)), Some(0.8), &kin, None));
        let mut now = 1_000;
        for _ in 0..=ms / 50 {
            now += 50;
            tracker.input(1, now, keys, (0.0, 0.0), 0.0, &kin, None);
        }
        tracker.held_sounds(now, by_body);
        tracker
            .sounds
            .iter()
            .filter(|s| s.from == 1 && s.kind == SoundKind::Sprint)
            .count()
    }

    #[test]
    fn the_sprint_sound_follows_the_keys_by_default_and_the_body_when_asked() {
        let sprint = key::SPRINT | key::FORWARD;
        // Off (`None`): the keys make it, a body that has not moved yet too.
        assert_eq!(sprint_sounds(sprint, 0, None), 1);
        // On: the key rule and the speed (5.0 m/s sprinting after the blend).
        assert_eq!(sprint_sounds(sprint, 0, Some(4.6)), 0, "not moving yet");
        assert_eq!(sprint_sounds(sprint, 800, Some(4.6)), 1, "sprinting");
        // A body that walks (FORWARD alone, 3.5 m/s) or holds FIRE makes none.
        assert_eq!(sprint_sounds(key::FORWARD, 800, Some(4.6)), 0);
        assert_eq!(sprint_sounds(sprint | key::FIRE, 800, Some(4.6)), 0);
        assert_eq!(sprint_sounds(sprint | key::CROUCH, 800, Some(4.6)), 0);
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
        tracker.held_sounds(1_100, None);
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
            8_000,
            vec![BatteryState::Carried(None), BatteryState::Home(1)],
        );
        assert_eq!(at.tracker.carrier(0), Some(1));
        let notes = at.tracker.take_notes();
        assert!(
            notes
                .iter()
                .any(|n| n.ends_with("now still player 1, no one else whose use met it")),
            "{notes:?}"
        );
    }

    /// The belief went to the wrong teammate: place 1's use met the battery
    /// a while before the world showed it lying (stale), place 0's later
    /// one too, from nearer. Place 0's uses at its own container put
    /// nothing in, judged after the put and the world's lag, so place 1 has
    /// it; and a believed carrier who died while the battery stayed carried
    /// did not carry it.
    #[test]
    fn a_belief_moves_on_when_put_ins_fail_or_its_carrier_dies_with_it_still_carried() {
        let mut at = AtTheContainer::new();
        at.press(1, 1_050, key::CROUCH);
        at.world(2_100, vec![BatteryState::Free(BATTERY)]);
        // Place 0 turns to the battery, standing at the container.
        at.stand(
            0,
            2_150,
            AT_IT,
            (-1.5f32).atan2(0.8),
            (0.12 - 1.6f32).atan2(1.7),
        );
        at.press(0, 2_200, 0);
        at.world(2_500, vec![BatteryState::Carried(None)]);
        assert_eq!(at.tracker.carrier(0), Some(0), "the fresher, nearer use");
        at.tracker.take_notes();
        for (i, time) in [2_700, 3_900, 5_100].into_iter().enumerate() {
            at.press(0, time, 0);
            at.world(time + 1_000, vec![BatteryState::Carried(None)]);
            assert_eq!(at.tracker.carrier(0), Some(0), "use {i} not yet judged");
        }
        // 2 s after the third use: the put (0.6 s) may still show.
        at.world(7_100, vec![BatteryState::Carried(None)]);
        assert_eq!(
            at.tracker.carrier(0),
            Some(0),
            "the third use not judged yet"
        );
        at.world(8_200, vec![BatteryState::Carried(None)]);
        assert_eq!(at.tracker.carrier(0), Some(1));
        let notes = at.tracker.take_notes();
        assert!(
            notes.iter().any(|n| n
                == "player 0 no longer believed to carry battery 0 (3 uses at his container put nothing in); now player 1"),
            "{notes:?}"
        );
        // Place 1 dies; the battery stays carried: place 0 has it after
        // all (the only other one near, ruled out before, is not taken
        // again), so the belief is nobody.
        let mut dead = fix(9_000, BESIDE);
        dead.alive = false;
        dead.feet = None;
        at.tracker.fixed(1, dead, None, &at.kin, None);
        at.world(9_000, vec![BatteryState::Carried(None)]);
        assert_eq!(at.tracker.carrier(0), None);
        at.world(12_000, vec![BatteryState::Carried(None)]);
        assert_eq!(at.tracker.carrier(0), None);
    }

    /// Cologne 2026-10-24: failed put-ins move the belief only to one whose
    /// own use met the battery; a teammate who was only near it is not
    /// believed the carrier (bot 4, 8 m off, had pressed nothing).
    #[test]
    fn failed_put_ins_move_no_belief_to_a_teammate_who_pressed_nothing() {
        let mut at = AtTheContainer::new();
        at.press(0, 1_600, 0);
        at.world(2_000, vec![BatteryState::Carried(None)]);
        assert_eq!(at.tracker.carrier(0), Some(0), "the only use near it");
        at.tracker.take_notes();
        for time in [2_400, 3_600, 4_800] {
            at.press(0, time, 0);
            at.world(time + 1_000, vec![BatteryState::Carried(None)]);
        }
        at.world(8_000, vec![BatteryState::Carried(None)]);
        assert_eq!(at.tracker.carrier(0), Some(0));
        let notes = at.tracker.take_notes();
        assert!(
            notes
                .iter()
                .any(|n| n.ends_with("now still player 0, no one else whose use met it")),
            "{notes:?}"
        );
    }

    /// The bench's ping jitter is a bounded random walk: within its bound
    /// either way (a spike of 150 ms is the live 90th percentile), a step of
    /// at most the live 99th percentile of the lateness's change a tick, the
    /// same for a place and a seed however it is asked, different from place
    /// to place and seed to seed, and none when it is 0.
    #[test]
    fn the_bench_jitter_is_a_seeded_bounded_walk_with_small_steps() {
        let tracker = |seed| {
            let mut tracker = Tracker::new(2, &[0, 1], &[true, true], vec![]);
            tracker.seen_jitter = (150, seed);
            tracker
        };
        let walk = |tracker: &Tracker, place| -> Vec<i32> {
            (0..2_000).map(|k| tracker.jitter(place, 1_000 + k * 50)).collect()
        };
        let mut off = Tracker::new(2, &[0, 1], &[true, true], vec![]);
        assert_eq!(off.jitter(0, 1_000), 0);
        off.seen_jitter = (0, 7);
        assert_eq!(off.jitter(0, 2_000), 0);
        let one = tracker(7);
        let draws = walk(&one, 0);
        assert!(draws.iter().all(|d| d.abs() <= 150));
        assert!(draws.windows(2).all(|w| (w[1] - w[0]).abs() <= JITTER_STEP_MS));
        // It wanders: both sides of the bound are reached in 100 s.
        assert!(draws.iter().any(|&d| d > 100) && draws.iter().any(|&d| d < -100));
        // The same tick asked again, or within it, gives the same.
        assert_eq!(one.jitter(0, 1_000 + 1_999 * 50), draws[1_999]);
        assert_eq!(one.jitter(0, 1_000 + 1_999 * 50 + 49), draws[1_999]);
        assert_eq!(walk(&tracker(7), 0), draws);
        assert_ne!(walk(&one, 1), draws);
        assert_ne!(walk(&tracker(8), 0), draws);
    }

    /// A player's lateness is the running mean of his accepted inputs'; a
    /// bot has none.
    #[test]
    fn a_players_lateness_is_the_running_mean_of_his_inputs() {
        let kin = Kinematics::default();
        let mut tracker = Tracker::new(2, &[0, 1], &[true, false], vec![]);
        tracker.entered(1, true);
        tracker.fixed(1, fix(1_000, Vec3::ZERO), None, &kin, None);
        let clock = Clock {
            clock: 1_100,
            time_left_s: 600.0,
            total_s: 900.0,
            playing: true,
        };
        assert_eq!(tracker.view(&clock, &kin, None).place(1).unwrap().late_ms, 0);
        for _ in 0..200 {
            tracker.lateness(1, 100);
        }
        let late = |tracker: &Tracker| tracker.view(&clock, &kin, None).place(1).unwrap().late_ms;
        assert_eq!(late(&tracker), 100);
        // A spike moves the mean by a sixteenth of it; a run of them, all.
        tracker.lateness(1, 420);
        assert_eq!(late(&tracker), 120);
        for _ in 0..200 {
            tracker.lateness(1, 20);
        }
        assert_eq!(late(&tracker), 20);
        assert_eq!(tracker.view(&clock, &kin, None).place(0).unwrap().late_ms, 0);
    }
}
