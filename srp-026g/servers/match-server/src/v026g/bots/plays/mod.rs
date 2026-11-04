//! The plays (`player_bots_behaviour.md` §3) and how they are chosen and
//! run (`player_bots_research.md` B1 points 3-4, B3 rules against
//! spaghetti, B4 "plays as data and functions").
//!
//! - One file per play, each with the same parts: `NUMBERS` (its tunable
//!   numbers and their defaults), `tuning()` (its rank band and default
//!   considerations), `candidates()` (scoring each target it could take),
//!   `State`, `start()` and `step()` (a small state machine) and its tests.
//! - [`PlayKind`] keys the tuning, the logs and the tests; [`Running`]
//!   holds the running play's state; the `match`es below dispatch, and a
//!   play added and not dispatched is a compile error.
//! - A play reads the match only through [`Ctx`] and never another play's
//!   state; what plays share goes through the team's knowledge or the
//!   commander. A step returns a [`Step`] with a reason; it never sets the
//!   bot's play itself.
//! - What a play wants of the body is an [`Intent`]: where to go, how
//!   fast, crouched or not, where to look, which keys to press. The combat
//!   reflex (`reflex.rs`) may override the look and the trigger.
//!
//! What is not a play: punishing a reload (5), turning to a sound (6),
//! breaking sight (7), the weapon matchup (11), spotting (19) and the
//! grenade before a peek (20) are the reflex's; personalities (14) are
//! tuning presets; self-balancing (23) is `signals::Skill`; answering a
//! theft and the event reactions (§4) raise the rank of the matching play
//! through the considerations and the commander.

pub mod ambush;
pub mod buddy;
pub mod collect;
pub mod cover_human;
pub mod crossfire;
pub mod cut;
pub mod escort;
pub mod flank;
pub mod guard_lead;
pub mod heal;
pub mod hold_exit;
pub mod hunt;
pub mod intercept;
pub mod late_denial;
pub mod pair_up;
pub mod raid;
pub mod retreat;
pub mod roam;
pub mod sneak;
pub mod station;
pub mod trade;

use mapdata::Vec3;
use serde::{Deserialize, Serialize};

use super::curve::{self, Explain, Input, Score};
use super::know::{Memory, TeamKnowledge, Track};
use super::rng::Rng;
use super::signals::{MatchSignals, Mods};
use super::tactics::Tactics;
use super::tuning::{Personality, PlayTuning, Tuning};
use super::view::{MatchView, Place, PlaceView};

/// The rank bands, highest first: a candidate of a higher band beats any
/// of a lower one (research B1 point 3); the commander counts an
/// objective its curves all but rule out as roam (`team::Bands`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Rank {
    Roam,
    Fight,
    Objective,
    Alarm,
}

/// The plays.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlayKind {
    /// Play 17: go forward by the ways humans go; the default.
    Roam,
    /// Pick up a battery lying about and bring it home (the battery
    /// modes' point; §4 "our carrier killed").
    Collect,
    /// Play 1.
    Ambush,
    /// Play 2, and the answer to a theft (§4).
    Intercept,
    /// Play 3.
    Escort,
    /// Play 4.
    Sneak,
    /// Play 8: an enemy holding an angle is flanked.
    Flank,
    /// Play 9.
    Crossfire,
    /// Play 10: a wounded bot falls back to resupply.
    Retreat,
    /// Play 12.
    CoverHuman,
    /// Play 13: never alone against the top player.
    PairUp,
    /// Play 15.
    HoldExit,
    /// Play 16.
    LateDenial,
    /// Play 18.
    GuardLead,
    /// Play 21: a partner's death is traded on the spot.
    Trade,
    /// Play 22.
    Heal,
    /// Team Deathmatch's endgame: the enemy cannot respawn; hunt the last
    /// of them.
    Hunt,
    /// Timed batteries: by a station when its battery comes.
    Station,
    /// Play 24: take a battery out of the enemy's container and bring it
    /// home, while their container is weakly held.
    Raid,
    /// Play 25: the follower of a bot pair keeps with its leader.
    Buddy,
    /// Play 26: hold a side angle on the enemy's best way where it comes
    /// into our half.
    Cut,
}

impl PlayKind {
    pub const ALL: [Self; 21] = [
        Self::Roam,
        Self::Collect,
        Self::Ambush,
        Self::Intercept,
        Self::Escort,
        Self::Sneak,
        Self::Flank,
        Self::Crossfire,
        Self::Retreat,
        Self::CoverHuman,
        Self::PairUp,
        Self::HoldExit,
        Self::LateDenial,
        Self::GuardLead,
        Self::Trade,
        Self::Heal,
        Self::Hunt,
        Self::Station,
        Self::Raid,
        Self::Buddy,
        Self::Cut,
    ];

    /// The personality family whose weight the play's `Personality`
    /// consideration reads.
    pub fn family(self) -> Family {
        match self {
            Self::Ambush | Self::HoldExit | Self::GuardLead | Self::Crossfire | Self::Cut => {
                Family::Hold
            }
            Self::Intercept | Self::Trade | Self::LateDenial | Self::Hunt => Family::Push,
            Self::Sneak | Self::Flank | Self::Raid => Family::Sneak,
            Self::Escort | Self::CoverHuman | Self::PairUp | Self::Buddy => Family::Support,
            Self::Roam | Self::Collect | Self::Retreat | Self::Heal | Self::Station => Family::None,
        }
    }
}

impl PlayKind {
    /// A play whose point is a fight: to wait for an enemy, go at one or
    /// stand by a teammate against one. A dry bot (nothing to fire) takes
    /// none of them; it still collects, carries, escorts, raids, stations
    /// and roams (match 0x5402a730: 4 of the human's 19 kills were of dry
    /// bots).
    pub fn seeks_fight(self) -> bool {
        matches!(
            self,
            Self::Ambush
                | Self::Intercept
                | Self::Flank
                | Self::Crossfire
                | Self::PairUp
                | Self::HoldExit
                | Self::GuardLead
                | Self::Trade
                | Self::Hunt
                | Self::Cut
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    Hold,
    Push,
    Sneak,
    Support,
    None,
}

/// What a play is about.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TargetRef {
    None,
    Point(Vec3),
    Place(Place),
    Battery(usize),
    Container(u8),
    /// An exit of the enemy base, by index (`Tactics::exits`).
    Exit(usize),
    /// A battery spawner's spot, by index (`MapData::battery_spawners`):
    /// looked at for a battery, with the battery cheat off.
    Spawner(usize),
}

/// A play's claim that only one bot of a team may hold (research B1
/// "smart objects": "never two bots on one ambush" is slot ownership).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SlotKey {
    pub kind: PlayKind,
    pub key: u32,
}

/// A play a bot could take, scored.
#[derive(Clone, Debug, PartialEq)]
pub struct Candidate {
    pub kind: PlayKind,
    pub target: TargetRef,
    pub rank: Rank,
    pub score: Score,
    pub why: Explain,
    pub slot: Option<SlotKey>,
}

/// How fast a body goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pace {
    Walk,
    /// Sprint where the way allows.
    Run,
    /// Crouched.
    Sneak,
}

/// Where a body looks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Look {
    /// The way it goes.
    Path,
    /// At a point (a pickup point held, a door watched), moving or not:
    /// the view goes a metre above it, at a body's height.
    At(Vec3),
    /// At the point itself: an object to use, which the use's ray from the
    /// eye must hit (`base_player::detect_usable_object`, 2 m, PROVEN), as
    /// a battery lying on the floor.
    Use(Vec3),
    /// About, as a body standing guard.
    Around,
}

/// Where a body goes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Go {
    Stay,
    To { at: Vec3, within_m: f32 },
}

/// What a play wants of the body until its next step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Intent {
    pub go: Go,
    pub pace: Pace,
    pub crouch: bool,
    pub look: Look,
    /// Keys pressed for one tick, then let go (use, drop, spot, quick use).
    pub press: u32,
    /// Keys held while the intent lasts (a grenade until it is thrown).
    pub hold: u32,
    /// Whether the reflex may fire (a carrier cannot; a sneak holds fire
    /// till seen).
    pub fire: bool,
    /// The goal is in the enemy base (their container): their spawn area
    /// is open to the route (`Tactics::allowed`).
    pub objective: bool,
    /// The goal is a step or two off in a line the play vouches for (a
    /// take's stand round a battery, a step back out of a nook): walked to
    /// by keys each tick, no route (`Motor::step_to`).
    pub straight: bool,
    /// A take's stand this step gave up at (`collect`): the bot keeps it,
    /// so its next take of that battery stands elsewhere.
    pub failed_stand: Option<collect::FailedStand>,
}

impl Intent {
    pub fn stay() -> Self {
        Self {
            go: Go::Stay,
            pace: Pace::Walk,
            crouch: false,
            look: Look::Around,
            press: 0,
            hold: 0,
            fire: true,
            objective: false,
            straight: false,
            failed_stand: None,
        }
    }

    /// Bound for the enemy base (their container).
    pub fn into_base(self) -> Self {
        Self {
            objective: true,
            ..self
        }
    }

    pub fn to(at: Vec3, pace: Pace) -> Self {
        Self {
            go: Go::To { at, within_m: 1.0 },
            pace,
            crouch: pace == Pace::Sneak,
            look: Look::Path,
            ..Self::stay()
        }
    }

    pub fn looking(self, look: Look) -> Self {
        Self { look, ..self }
    }

    pub fn crouched(self) -> Self {
        Self {
            crouch: true,
            ..self
        }
    }

    pub fn pressing(self, keys: u32) -> Self {
        Self {
            press: self.press | keys,
            ..self
        }
    }

    pub fn within(self, within_m: f32) -> Self {
        match self.go {
            Go::To { at, .. } => Self {
                go: Go::To { at, within_m },
                ..self
            },
            Go::Stay => self,
        }
    }

    /// Walked to straight by keys ([`Intent::straight`]).
    pub fn straight(self) -> Self {
        Self {
            straight: true,
            ..self
        }
    }

    pub fn hold_fire(self) -> Self {
        Self {
            fire: false,
            ..self
        }
    }
}

/// What a step says.
#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    Continue(Intent),
    /// Finished as it should.
    Done(&'static str),
    /// Given up.
    Failed(&'static str),
}

/// A bot's facts the plays read besides the match.
#[derive(Clone, Debug)]
pub struct BotFacts<'a> {
    pub personality: &'a Personality,
    /// Its map side (spawn team), if known.
    pub side: Option<i32>,
    /// Its present play, target and when it started.
    pub current: Option<(PlayKind, TargetRef, u32)>,
    /// When it last ran each play.
    pub last_ran: &'a [(PlayKind, u32)],
    /// Its partner (play 21), if it has one.
    pub partner: Option<Place>,
    /// Whether the partner has it for partner too (a pair, play 25); a
    /// team's odd bot partners one way, for its trades only.
    pub paired: bool,
    /// Medkits it used this life (quick use 1 pressed).
    pub medkits_used: u8,
    /// Goals its searches proved no way to lately, and until when.
    pub no_way_to: &'a [(Vec3, u32)],
    /// Plays that failed at once at a target, and until when they are
    /// not taken there again ([`SET_ASIDE_MS`]).
    pub set_aside: &'a [(PlayKind, TargetRef, u32)],
    /// Stands its takes of batteries gave up at lately (`collect`).
    pub failed_stands: &'a [collect::FailedStand],
    /// Nothing left to fire ([`super::reflex::Reflex::dry`]): it takes no
    /// play that seeks a fight ([`PlayKind::seeks_fight`]).
    pub dry: bool,
}

/// A play that fails this soon after it starts ...
pub const QUICK_FAIL_MS: u32 = 2_000;
/// ... is not taken at that target again for this long: no play taken and
/// dropped every poll ("ends GuardLead: no spot with a sight line" four
/// times a second in the owner's match of 2026-10-16).
pub const SET_ASIDE_MS: u32 = 8_000;

/// What a play sees: the match, the bot, its knowledge, the team's, the
/// signals and modifiers, the map's tactics and the tuning.
pub struct Ctx<'a> {
    pub now: u32,
    pub view: &'a MatchView,
    pub me: &'a PlaceView,
    pub bot: BotFacts<'a>,
    pub memory: &'a Memory,
    pub team: &'a TeamKnowledge,
    pub sig: &'a MatchSignals,
    pub mods: &'a Mods,
    pub tactics: &'a Tactics,
    pub tuning: &'a Tuning,
}

impl Ctx<'_> {
    /// The play's tuning.
    pub fn play(&self, kind: PlayKind) -> PlayTuning {
        self.tuning
            .plays
            .get(&kind)
            .cloned()
            .unwrap_or_else(|| default_tuning(kind))
    }

    /// A play's number (its tuning's, else its default).
    pub fn num(&self, kind: PlayKind, name: &str) -> f32 {
        self.tuning.number(kind, name)
    }

    /// The point a target stands for, if it has one.
    pub fn point_of(&self, target: TargetRef) -> Option<Vec3> {
        match target {
            TargetRef::None => None,
            TargetRef::Point(at) => Some(at),
            TargetRef::Place(place) => self.known_place(place),
            TargetRef::Battery(battery) => self.battery_at(battery),
            TargetRef::Container(id) => self
                .view
                .containers
                .iter()
                .find(|c| c.id == id)
                .map(|c| c.at),
            TargetRef::Exit(index) => self
                .bot
                .side
                .and_then(|side| self.enemy_side(side))
                .and_then(|enemy| self.tactics.exits(enemy).get(index).copied()),
            TargetRef::Spawner(index) => self
                .tactics
                .world
                .map
                .battery_spawners
                .get(index)
                .map(|s| s.item),
        }
    }

    /// The other map side.
    pub fn enemy_side(&self, side: i32) -> Option<i32> {
        self.tactics
            .world
            .map
            .spawns
            .iter()
            .map(|spawn| spawn.team)
            .find(|team| *team != side)
    }

    /// Where a place is as far as this bot may know: a teammate's place is
    /// known (the HUD shows it); an enemy's only through the memory.
    pub fn known_place(&self, place: Place) -> Option<Vec3> {
        let p = self.view.place(place)?;
        if p.team == self.me.team {
            return p.alive.then_some(p.feet);
        }
        self.memory
            .tracks()
            .iter()
            .find(|t| t.who == place)
            .map(|t| t.at)
    }

    /// Where a battery lies as far as the bot may know: with the battery
    /// cheat on (`CheatTuning::batteries`) where every lying battery is;
    /// with it off where its team believes it lies (its HUD, or seen by a
    /// bot of the team: maybe taken since, until a bot sees the spot).
    pub fn battery_lying(&self, battery: usize) -> Option<Vec3> {
        use super::view::BatteryState;
        if self.tuning.cheats.batteries {
            match self.view.batteries.get(battery)? {
                BatteryState::Free(at) => Some(*at),
                _ => None,
            }
        } else {
            self.team.battery(battery).map(|known| known.at)
        }
    }

    /// Where a battery is as far as the bot may know: lying
    /// ([`Self::battery_lying`]), carried (where the carrier is known: a
    /// teammate, or an enemy sensed) or home (in its container; the HUD
    /// shows the counts).
    pub fn battery_at(&self, battery: usize) -> Option<Vec3> {
        use super::view::BatteryState;
        let cheat = self.tuning.cheats.batteries;
        if !cheat && let Some(at) = self.battery_lying(battery) {
            return Some(at);
        }
        match self.view.batteries.get(battery)? {
            BatteryState::Free(at) => cheat.then_some(*at),
            BatteryState::Carried(Some(carrier)) => self.known_place(*carrier),
            BatteryState::Carried(None) => None,
            BatteryState::Home(id) => self.point_of(TargetRef::Container(*id)),
        }
    }

    /// With the battery cheat off: the batteries the team cannot place
    /// ([`batteries_unaccounted`]).
    pub fn batteries_unaccounted(&self) -> usize {
        batteries_unaccounted(self.view, self.me.team, self.team)
    }

    /// With the battery cheat off: whether battery spawner `index`'s spot
    /// is worth a look ([`spawner_worth_checking`]).
    pub fn spawner_worth_checking(&self, index: usize) -> bool {
        spawner_worth_checking(self.view, self.team, self.tactics, self.tuning, index)
    }

    /// Whether a way to `at` may be open now: the floor there (as the
    /// motor finds it) is one a walk from a spawn gets to, and it is
    /// entered from a floor this bot may walk on, or from where it stands.
    /// Else the route search would answer no way, and the bot would stand
    /// still (a battery inside a zone the bots keep out of: level_02's
    /// fruit cloud and stalactites; one on a shelf's top past the jump),
    /// or a floor that zones on all match close round (a pocket of toxic
    /// ground: the search's answer, made here for the whole match,
    /// `Tactics::past_fixed_zones`).
    pub fn reachable(&self, at: Vec3, objective: bool) -> bool {
        let grid = &self.tactics.world.grid;
        let Some(goal) = grid.locate(at, 3.0) else {
            return false;
        };
        if !self.tactics.walkable(goal) {
            return false;
        }
        let start = grid.locate(self.me.feet, 1.5);
        if start == Some(goal) {
            return true;
        }
        let past = |b| self.tactics.past_fixed_zones(b);
        (past(goal) || grid.entered_from(goal, past))
            && grid.entered_from(goal, |b| {
                Some(b) == start || self.tactics.allowed(b, self.now, self.bot.side, objective)
            })
    }

    /// Whether no way leads to a battery lying at `at` all match, for any
    /// side: no floor to take it from, or that floor is past zones on all
    /// match ([`Tactics::past_fixed_zones`]: level_01's swamp). Whatever
    /// lies there stays there.
    pub fn shut_all_match(&self, at: Vec3) -> bool {
        let Some(stand) = self.tactics.stand_to_take(at) else {
            return true;
        };
        let grid = &self.tactics.world.grid;
        let Some(goal) = grid.locate(stand, 3.0) else {
            return true;
        };
        let past = |b| self.tactics.past_fixed_zones(b);
        !self.tactics.walkable(goal) || !(past(goal) || grid.entered_from(goal, past))
    }

    /// Whether this bot's search to a goal at `at` (the motor's, within
    /// its replan distance) proved no way lately (`Bot::no_way_to`).
    pub fn no_way_lately(&self, at: Vec3) -> bool {
        self.bot
            .no_way_to
            .iter()
            .any(|&(goal, until)| self.now < until && goal.distance_xz(at) <= super::REPLAN_MOVED_M)
    }

    /// Known enemies within `m` of `at` (by the memory, whatever the
    /// source).
    pub fn enemies_near(&self, at: Vec3, m: f32) -> usize {
        self.memory
            .tracks()
            .iter()
            .filter(|t| t.at.distance_xz(at) < m)
            .count()
    }

    /// Teammates (alive) within `m` of `at`, the bot itself not counted.
    pub fn friends_near(&self, at: Vec3, m: f32) -> usize {
        self.view
            .team(self.me.team)
            .filter(|p| p.place != self.me.place && p.feet.distance_xz(at) < m)
            .count()
    }

    /// Known enemy tracks, nearest first.
    pub fn threats(&self) -> Vec<&Track> {
        let mut tracks: Vec<&Track> = self.memory.tracks().iter().collect();
        tracks.sort_by(|a, b| {
            a.at.distance_xz(self.me.feet)
                .total_cmp(&b.at.distance_xz(self.me.feet))
        });
        tracks
    }

    /// The walk to `at`, metres: the ground distance with a detour factor
    /// (cheap; a route is planned only for the play taken).
    pub fn path_dist(&self, at: Vec3) -> f32 {
        self.me.feet.distance_xz(at) * 1.25
    }

    /// A consideration's input for a candidate of `kind` at `target`.
    pub fn input(&self, input: Input, kind: PlayKind, target: TargetRef) -> f32 {
        if let Some(value) = self.sig.input(input).or_else(|| self.mods.input(input)) {
            return value;
        }
        let point = self.point_of(target);
        let since =
            |at: Option<u32>| at.map_or(1e6, |at| self.now.saturating_sub(at) as f32 / 1000.0);
        match input {
            // A target with no place (roam ahead) is near.
            Input::PathDist => point.map_or(0.0, |at| self.path_dist(at)),
            Input::Health => self.me.health,
            Input::EnemiesNearTarget => point.map_or(0.0, |at| self.enemies_near(at, 25.0) as f32),
            Input::FriendsNearTarget => point.map_or(0.0, |at| self.friends_near(at, 25.0) as f32),
            Input::Personality => {
                let p = self.bot.personality;
                match kind.family() {
                    Family::Hold => p.hold,
                    Family::Push => p.push,
                    Family::Sneak => p.sneak,
                    Family::Support => p.support,
                    Family::None => 1.0,
                }
            }
            Input::SinceLast => since(
                self.bot
                    .last_ran
                    .iter()
                    .find(|(k, _)| *k == kind)
                    .map(|(_, at)| *at),
            ),
            Input::SinceStarted => match self.bot.current {
                Some((k, _, started)) if k == kind => since(Some(started)),
                _ => 0.0,
            },
            Input::Current => match self.bot.current {
                Some((k, t, _)) if k == kind && t == target => 1.0,
                _ => 0.0,
            },
            _ => 0.0,
        }
    }

    /// A candidate of `kind` at `target`, scored by its tuning's
    /// considerations; `None` at 0.
    pub fn candidate(
        &self,
        kind: PlayKind,
        target: TargetRef,
        slot: Option<u32>,
    ) -> Option<Candidate> {
        if self
            .bot
            .set_aside
            .iter()
            .any(|&(k, t, until)| k == kind && t == target && self.now < until)
            || (self.bot.dry && kind.seeks_fight())
        {
            return None;
        }
        let play = self.play(kind);
        let (score, why) = curve::score(&play.considerations, |input| {
            self.input(input, kind, target)
        });
        (!score.is_zero()).then(|| Candidate {
            kind,
            target,
            rank: play.rank,
            score,
            why,
            slot: slot.map(|key| SlotKey { kind, key }),
        })
    }
}

/// With the battery cheat off: the batteries team `team` cannot place, of
/// the mode's count (on the HUD): neither in a container (the HUD's
/// counts), nor carried by a teammate, nor believed lying somewhere.
pub fn batteries_unaccounted(view: &MatchView, team: u8, known: &TeamKnowledge) -> usize {
    use super::view::BatteryState;
    let home: usize = view.containers.iter().map(|c| c.batteries).sum();
    let mut placed = 0;
    for (battery, state) in view.batteries.iter().enumerate() {
        match *state {
            // Counted by its container already.
            BatteryState::Home(_) => {}
            BatteryState::Carried(Some(carrier))
                if view.place(carrier).is_some_and(|p| p.team == team) =>
            {
                placed += 1;
            }
            _ if known.battery(battery).is_some() => placed += 1,
            _ => {}
        }
    }
    view.batteries
        .len()
        .saturating_sub(home)
        .saturating_sub(placed)
}

/// With the battery cheat off: whether battery spawner `index`'s spot is
/// worth a look for a team that knows `known`. In the battery mode the
/// batteries lie at spawners from the start and never come again, so a
/// spot once seen is done; in timed batteries one comes now and then by a
/// station, so a spot is looked at again after `spawner_recheck_ms`. A
/// spot where the team believes a battery lies needs no look.
pub fn spawner_worth_checking(
    view: &MatchView,
    known: &TeamKnowledge,
    tactics: &Tactics,
    tuning: &Tuning,
    index: usize,
) -> bool {
    use super::view::Mode;
    let Some(spawner) = tactics.world.map.battery_spawners.get(index) else {
        return false;
    };
    let fresh = match (view.mode, known.spawner_seen(index)) {
        (_, None) => true,
        (Mode::TimedBatteries, Some(seen)) => view.clock >= seen + tuning.sense.spawner_recheck_ms,
        _ => false,
    };
    fresh
        && !known
            .batteries
            .iter()
            .flatten()
            .any(|k| k.at.distance_xz(spawner.item) <= 2.0)
}

/// The running play's state.
#[derive(Clone, Debug, PartialEq)]
pub enum Running {
    Roam(roam::State),
    Collect(collect::State),
    Ambush(ambush::State),
    Intercept(intercept::State),
    Escort(escort::State),
    Sneak(sneak::State),
    Flank(flank::State),
    Crossfire(crossfire::State),
    Retreat(retreat::State),
    CoverHuman(cover_human::State),
    PairUp(pair_up::State),
    HoldExit(hold_exit::State),
    LateDenial(late_denial::State),
    GuardLead(guard_lead::State),
    Trade(trade::State),
    Heal(heal::State),
    Hunt(hunt::State),
    Station(station::State),
    Raid(raid::State),
    Buddy(buddy::State),
    Cut(cut::State),
}

impl Running {
    pub fn kind(&self) -> PlayKind {
        match self {
            Self::Roam(_) => PlayKind::Roam,
            Self::Collect(_) => PlayKind::Collect,
            Self::Ambush(_) => PlayKind::Ambush,
            Self::Intercept(_) => PlayKind::Intercept,
            Self::Escort(_) => PlayKind::Escort,
            Self::Sneak(_) => PlayKind::Sneak,
            Self::Flank(_) => PlayKind::Flank,
            Self::Crossfire(_) => PlayKind::Crossfire,
            Self::Retreat(_) => PlayKind::Retreat,
            Self::CoverHuman(_) => PlayKind::CoverHuman,
            Self::PairUp(_) => PlayKind::PairUp,
            Self::HoldExit(_) => PlayKind::HoldExit,
            Self::LateDenial(_) => PlayKind::LateDenial,
            Self::GuardLead(_) => PlayKind::GuardLead,
            Self::Trade(_) => PlayKind::Trade,
            Self::Heal(_) => PlayKind::Heal,
            Self::Hunt(_) => PlayKind::Hunt,
            Self::Station(_) => PlayKind::Station,
            Self::Raid(_) => PlayKind::Raid,
            Self::Buddy(_) => PlayKind::Buddy,
            Self::Cut(_) => PlayKind::Cut,
        }
    }

    /// The state's step name, for the trace.
    pub fn step_name(&self) -> String {
        let text = format!("{self:?}");
        text.split(['(', '{', ' '])
            .nth(1)
            .unwrap_or("")
            .trim()
            .to_owned()
    }
}

/// Every play's candidates for the bot.
pub fn candidates(cx: &Ctx, out: &mut Vec<Candidate>) {
    roam::candidates(cx, out);
    collect::candidates(cx, out);
    ambush::candidates(cx, out);
    intercept::candidates(cx, out);
    escort::candidates(cx, out);
    sneak::candidates(cx, out);
    flank::candidates(cx, out);
    crossfire::candidates(cx, out);
    retreat::candidates(cx, out);
    cover_human::candidates(cx, out);
    pair_up::candidates(cx, out);
    hold_exit::candidates(cx, out);
    late_denial::candidates(cx, out);
    guard_lead::candidates(cx, out);
    trade::candidates(cx, out);
    heal::candidates(cx, out);
    hunt::candidates(cx, out);
    station::candidates(cx, out);
    raid::candidates(cx, out);
    buddy::candidates(cx, out);
    cut::candidates(cx, out);
}

/// A play's start.
pub fn start(kind: PlayKind, target: TargetRef, cx: &Ctx, rng: &mut Rng) -> Running {
    match kind {
        PlayKind::Roam => Running::Roam(roam::start(target, cx, rng)),
        PlayKind::Collect => Running::Collect(collect::start(target, cx, rng)),
        PlayKind::Ambush => Running::Ambush(ambush::start(target, cx, rng)),
        PlayKind::Intercept => Running::Intercept(intercept::start(target, cx, rng)),
        PlayKind::Escort => Running::Escort(escort::start(target, cx, rng)),
        PlayKind::Sneak => Running::Sneak(sneak::start(target, cx, rng)),
        PlayKind::Flank => Running::Flank(flank::start(target, cx, rng)),
        PlayKind::Crossfire => Running::Crossfire(crossfire::start(target, cx, rng)),
        PlayKind::Retreat => Running::Retreat(retreat::start(target, cx, rng)),
        PlayKind::CoverHuman => Running::CoverHuman(cover_human::start(target, cx, rng)),
        PlayKind::PairUp => Running::PairUp(pair_up::start(target, cx, rng)),
        PlayKind::HoldExit => Running::HoldExit(hold_exit::start(target, cx, rng)),
        PlayKind::LateDenial => Running::LateDenial(late_denial::start(target, cx, rng)),
        PlayKind::GuardLead => Running::GuardLead(guard_lead::start(target, cx, rng)),
        PlayKind::Trade => Running::Trade(trade::start(target, cx, rng)),
        PlayKind::Heal => Running::Heal(heal::start(target, cx, rng)),
        PlayKind::Hunt => Running::Hunt(hunt::start(target, cx, rng)),
        PlayKind::Station => Running::Station(station::start(target, cx, rng)),
        PlayKind::Raid => Running::Raid(raid::start(target, cx, rng)),
        PlayKind::Buddy => Running::Buddy(buddy::start(target, cx, rng)),
        PlayKind::Cut => Running::Cut(cut::start(target, cx, rng)),
    }
}

/// A step of the running play.
pub fn step(running: &mut Running, target: TargetRef, cx: &Ctx, rng: &mut Rng) -> Step {
    match running {
        Running::Roam(s) => roam::step(s, target, cx, rng),
        Running::Collect(s) => collect::step(s, target, cx, rng),
        Running::Ambush(s) => ambush::step(s, target, cx, rng),
        Running::Intercept(s) => intercept::step(s, target, cx, rng),
        Running::Escort(s) => escort::step(s, target, cx, rng),
        Running::Sneak(s) => sneak::step(s, target, cx, rng),
        Running::Flank(s) => flank::step(s, target, cx, rng),
        Running::Crossfire(s) => crossfire::step(s, target, cx, rng),
        Running::Retreat(s) => retreat::step(s, target, cx, rng),
        Running::CoverHuman(s) => cover_human::step(s, target, cx, rng),
        Running::PairUp(s) => pair_up::step(s, target, cx, rng),
        Running::HoldExit(s) => hold_exit::step(s, target, cx, rng),
        Running::LateDenial(s) => late_denial::step(s, target, cx, rng),
        Running::GuardLead(s) => guard_lead::step(s, target, cx, rng),
        Running::Trade(s) => trade::step(s, target, cx, rng),
        Running::Heal(s) => heal::step(s, target, cx, rng),
        Running::Hunt(s) => hunt::step(s, target, cx, rng),
        Running::Station(s) => station::step(s, target, cx, rng),
        Running::Raid(s) => raid::step(s, target, cx, rng),
        Running::Buddy(s) => buddy::step(s, target, cx, rng),
        Running::Cut(s) => cut::step(s, target, cx, rng),
    }
}

/// A play's default tuning.
pub fn default_tuning(kind: PlayKind) -> PlayTuning {
    match kind {
        PlayKind::Roam => roam::tuning(),
        PlayKind::Collect => collect::tuning(),
        PlayKind::Ambush => ambush::tuning(),
        PlayKind::Intercept => intercept::tuning(),
        PlayKind::Escort => escort::tuning(),
        PlayKind::Sneak => sneak::tuning(),
        PlayKind::Flank => flank::tuning(),
        PlayKind::Crossfire => crossfire::tuning(),
        PlayKind::Retreat => retreat::tuning(),
        PlayKind::CoverHuman => cover_human::tuning(),
        PlayKind::PairUp => pair_up::tuning(),
        PlayKind::HoldExit => hold_exit::tuning(),
        PlayKind::LateDenial => late_denial::tuning(),
        PlayKind::GuardLead => guard_lead::tuning(),
        PlayKind::Trade => trade::tuning(),
        PlayKind::Heal => heal::tuning(),
        PlayKind::Hunt => hunt::tuning(),
        PlayKind::Station => station::tuning(),
        PlayKind::Raid => raid::tuning(),
        PlayKind::Buddy => buddy::tuning(),
        PlayKind::Cut => cut::tuning(),
    }
}

/// A play's numbers and their defaults.
pub fn numbers(kind: PlayKind) -> &'static [(&'static str, f32)] {
    match kind {
        PlayKind::Roam => roam::NUMBERS,
        PlayKind::Collect => collect::NUMBERS,
        PlayKind::Ambush => ambush::NUMBERS,
        PlayKind::Intercept => intercept::NUMBERS,
        PlayKind::Escort => escort::NUMBERS,
        PlayKind::Sneak => sneak::NUMBERS,
        PlayKind::Flank => flank::NUMBERS,
        PlayKind::Crossfire => crossfire::NUMBERS,
        PlayKind::Retreat => retreat::NUMBERS,
        PlayKind::CoverHuman => cover_human::NUMBERS,
        PlayKind::PairUp => pair_up::NUMBERS,
        PlayKind::HoldExit => hold_exit::NUMBERS,
        PlayKind::LateDenial => late_denial::NUMBERS,
        PlayKind::GuardLead => guard_lead::NUMBERS,
        PlayKind::Trade => trade::NUMBERS,
        PlayKind::Heal => heal::NUMBERS,
        PlayKind::Hunt => hunt::NUMBERS,
        PlayKind::Station => station::NUMBERS,
        PlayKind::Raid => raid::NUMBERS,
        PlayKind::Buddy => buddy::NUMBERS,
        PlayKind::Cut => cut::NUMBERS,
    }
}

/// Shared considerations (research B3 rule 3): the catalogue every play's
/// tuning draws from.
pub mod considerations {
    use crate::v026g::bots::curve::{Consideration, Curve, Input};

    /// Nearer is better, out to `far` metres.
    pub fn near(far: f32) -> Consideration {
        Consideration::new(Input::PathDist, 0.0, far, Curve::Poly { k: 0.7 }).inverted()
    }

    /// Healthy enough: 0 under `low`.
    pub fn health_above(low: f32) -> Consideration {
        Consideration::new(Input::Health, low, low + 0.01, Curve::Step { at: 0.5 })
    }

    /// The personality's weight for the play's family (0.5 to 2 → 0.25 to 1).
    pub fn personality() -> Consideration {
        Consideration::new(Input::Personality, 0.0, 2.0, Curve::Linear)
    }

    /// Not again within `s` seconds of the last run.
    pub fn cooldown(s: f32) -> Consideration {
        Consideration::new(Input::SinceLast, 0.0, s, Curve::Step { at: 1.0 })
    }

    /// Kept a while once taken (commitment), up to `s` seconds of runtime,
    /// then less and less.
    pub fn runtime(s: f32) -> Consideration {
        Consideration::new(
            Input::SinceStarted,
            0.0,
            s,
            Curve::Piecewise {
                points: vec![(0.0, 1.0), (0.8, 1.0), (1.0, 0.05)],
            },
        )
    }

    /// A play that pushes into the enemy (a raid, a sneak, a flank): more
    /// as the team's aggression rises ([`Input::Aggression`]: behind on
    /// points, and the more so the less time is left), 0.7 at an even
    /// match. The owner's word after the first matches against the real
    /// game (2026-10-16): the team that loses should be more aggressive.
    pub fn pushing() -> Consideration {
        Consideration::new(
            Input::Aggression,
            0.0,
            1.0,
            Curve::Piecewise {
                points: vec![(0.0, 0.4), (0.5, 0.7), (1.0, 1.0)],
            },
        )
    }

    /// A play that holds a spot and waits (a crossfire, an exit, a cut, an
    /// ambush): less as the team's aggression rises, 0.85 at an even
    /// match.
    pub fn holding() -> Consideration {
        Consideration::new(
            Input::Aggression,
            0.0,
            1.0,
            Curve::Piecewise {
                points: vec![(0.0, 1.0), (0.5, 0.85), (1.0, 0.45)],
            },
        )
    }

    /// A play that holds a spot and waits, for a team behind late in the
    /// match ([`Input::BehindLate`]): less and less, and none when far
    /// behind with little time left, so its bots push for points (a raid,
    /// late denial, a fetch, else forward: the hold plays are of the fight
    /// band, which beats any play of the roam band whatever its score).
    pub fn not_behind_late() -> Consideration {
        Consideration::new(
            Input::BehindLate,
            0.0,
            1.0,
            Curve::Piecewise {
                points: vec![(0.0, 1.0), (0.5, 0.6), (0.8, 0.0), (1.0, 0.0)],
            },
        )
    }

    /// A team modifier, from `lo` (at 0) to 1.
    pub fn modifier(input: Input, lo: f32) -> Consideration {
        Consideration::new(
            input,
            0.0,
            1.0,
            Curve::Piecewise {
                points: vec![(0.0, lo), (1.0, 1.0)],
            },
        )
    }
}

#[cfg(test)]
pub mod tests;
