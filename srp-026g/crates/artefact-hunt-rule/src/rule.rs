//! The state machine: 0.46c2's rule 97 from the moment the match starts.
//!
//! Its eleven states and what becomes of them here:
//!
//! | 0.46c2 | here |
//! |---|---|
//! | 0 waiting for players, 1 match countdown, 9 finished (nobody came) | [`Stage::Idle`]: 0.26g's time rule does these |
//! | 2 waiting for the artefact | [`Round::Hunt`] with [`Artefact::Waiting`] |
//! | 3 spawned, 5 dropped | [`Artefact::Lying`] at a [`Spot`] |
//! | 4 picked | [`Artefact::Carried`] |
//! | 6 brought, 7 transition to a new round, 8 round countdown | [`Round::Restart`]: the shared [`Restart`] (Freeze Tag's too) |
//! | 10 finished successfully | [`Stage::Over`] |
//!
//! Sudden death is a [`Clock`] of the rounds that can have it: the hunt and
//! the countdown (its fallen), the fade (no fallen: everyone respawns at
//! its end). A delivery in sudden death ends the match, so the 3 s after a
//! delivery are always of the match time.

use crate::geometry::Position;
use crate::params::Params;
use crate::restart::{Lives, Restart, Spawning, Step};
use crate::roster::{Place, Roster, Sides, Team};
use crate::time::{Ms, Time};
use crate::way::Way;

/// The rule's own random numbers, exactly 0.46c2's (rule+0x1F8, 0x7B0DF5):
/// `s = s x 0x08088405 + 1`, a draw below `range` the high half of `s x range`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rng(u32);

impl Rng {
    pub const fn seeded(seed: u32) -> Rng {
        Rng(seed)
    }

    pub const fn state(self) -> u32 {
        self.0
    }

    /// The generator after a draw, and the draw: uniform in `0..range` (0
    /// for an empty range).
    pub const fn below(self, range: u32) -> (Rng, u32) {
        let next = self.0.wrapping_mul(0x0808_8405).wrapping_add(1);
        (Rng(next), ((next as u64 * range as u64) >> 32) as u32)
    }
}

/// The least time dead the game itself needs, from a death event's time
/// (the step before the death's, `rules_on_event` 0x2D1EA0): past the
/// death's step, whose respawn tick ran before the death (list 0 before the
/// players' list 2) and spawns only a time equal to its step's (0x3168BA).
/// Steps are at most 50 ms apart.
const AFTER_DEATH_STEP: Ms = Ms::short(51);

/// Where the artefact lies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Spot {
    /// At a spawner of the map (its index: below the map's count).
    Spawner(u8),
    /// Where a carrier let go of it.
    Dropped(Position),
}

impl Spot {
    /// The artefact's place. A spawner's index is checked where the spot is
    /// made (a draw, the wire's parse); one out of the map is the first.
    pub fn position(self, params: &Params) -> Position {
        match self {
            Spot::Spawner(index) => params.spawner(index).unwrap_or(params.spawners[0]),
            Spot::Dropped(at) => at,
        }
    }
}

/// Sudden death: no respawns, a team with nobody left loses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sudden {
    /// Its 180 s are counted from here (0.46c2 rule+0x228); it may be ahead
    /// of the round restart that turned it on.
    pub(crate) from: Time,
    /// Who died since the round's countdown, or since it began in a hunt:
    /// out for good.
    pub(crate) fallen: Roster,
}

impl Sudden {
    pub const fn from(&self) -> Time {
        self.from
    }

    pub const fn fallen(&self) -> Roster {
        self.fallen
    }
}

/// The match's clock in a round that may be in sudden death.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Clock {
    /// The match time runs.
    Regular,
    Sudden(Sudden),
}

/// The one artefact, in a round's hunt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Artefact {
    /// None in the world: one appears at `until`.
    Waiting {
        until: Time,
    },
    /// It lies in the world and is removed at `until`.
    Lying {
        spot: Spot,
        until: Time,
        way: Way,
    },
    Carried {
        by: Place,
        way: Way,
    },
}

/// Where a round is (0.46c2's states 2 to 8).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Round {
    /// States 2 to 5: the artefact's life.
    Hunt { clock: Clock, artefact: Artefact },
    /// States 6 to 8, after a delivery: what follows its 3 s is [`After`],
    /// decided by the delivery (their deadline less `time_before_new_round`);
    /// the fade carries sudden death's start when it is on (it turns on with
    /// the fade), the countdown the next round's clock.
    Restart(Restart<Option<Time>, Clock>),
}

/// What follows the 3 s after a delivery: one test, at the delivery.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum After {
    /// A team has the goal, or the scores differ with less than
    /// `time_needed_to_continue_match` left.
    End,
    /// A new round.
    NewRound,
    /// Equal scores with less than `time_needed_to_continue_match` left: a
    /// new round in sudden death, its time counted from `from` (0.46c2:
    /// the delivery + `time_before_new_round` + `round_countdown_time`,
    /// 0x78519B).
    SuddenDeath { from: Time },
}

/// The match in play.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Play {
    /// The match time and the respawn waves are counted from here.
    pub(crate) start: Time,
    pub(crate) round: Round,
}

impl Play {
    pub const fn start(&self) -> Time {
        self.start
    }

    pub const fn round(&self) -> &Round {
        &self.round
    }
}

/// Why the match ended: 0.46c2's two end texts (0x785EF0), and its sudden
/// death's point for a team with nobody left told as such (2026-11-21's test:
/// the owner's leaving gave the other team a "time out" win).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    /// Decided by deliveries before the time ran out.
    Collected = 0,
    /// The match time ran out, sudden death's did, or a delivery won it.
    TimeOut = 1,
    /// Sudden death's point for the team that still had somebody in.
    Wiped = 2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Over {
    pub(crate) at: Time,
    pub(crate) reason: Reason,
}

impl Over {
    pub const fn at(&self) -> Time {
        self.at
    }

    pub const fn reason(&self) -> Reason {
        self.reason
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    /// The time rule waits for players or counts the match down.
    Idle,
    Play(Play),
    Over(Over),
}

/// The result (the statistics' leading team: 0, 1, 2 a draw).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Winner(Team),
    Draw,
}

impl Outcome {
    /// The statistics' byte.
    pub const fn byte(self) -> u8 {
        match self {
            Outcome::Winner(team) => team as u8,
            Outcome::Draw => 2,
        }
    }
}

/// The rule's whole state: what a world saves and loads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rule {
    pub(crate) rng: Rng,
    /// Each at most the goal.
    pub(crate) score: [u8; 2],
    pub(crate) lives: Lives,
    pub(crate) sides: Sides,
    pub(crate) stage: Stage,
}

/// What happened in the world. Each carries the world's time.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    /// The time rule's countdown ended: `now` is the match's start.
    Start { now: Time },
    /// A step of the simulation: the rule's own tick. The step's player
    /// events (a take, a drop, a death) follow it and carry the time of the
    /// step before.
    Tick { now: Time },
    /// A player entered the match.
    Entered { place: Place, team: Team, now: Time },
    /// A player left the match; `ground` is the ground under him while he
    /// was in the world.
    Left {
        place: Place,
        now: Time,
        ground: Option<Position>,
    },
    /// A player died (by anybody or nothing); `ground` is the ground under him.
    Died {
        place: Place,
        now: Time,
        ground: Position,
    },
    /// The use key with the lying artefact in reach and in sight.
    Take { place: Place, now: Time },
    /// The Drop key; `ground` is the ground under the player.
    Drop {
        place: Place,
        now: Time,
        ground: Position,
    },
    /// The use key at one's own team's base; `ground` is the ground under
    /// the player.
    Deliver {
        place: Place,
        now: Time,
        ground: Position,
    },
    /// The game switches `place` into its chosen kit where it stands at the
    /// next step, in a round's countdown (patch 31's switch): a life for
    /// supplies, counted.
    KitSwitched { place: Place },
}

/// What the game does for a transition: a score event for the statistics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// `put_victory_artefact_in_container`: the deliverer.
    Put { place: Place },
    /// `bring_victory_artefact`: a carrier's share of the way, hundredths.
    Bring { place: Place, hundredths: u16 },
}

/// The most actions of one transition: a delivery's put and a share for
/// every place.
pub const MAX_ACTIONS: usize = crate::roster::PLAYERS + 1;

/// The actions of a transition, in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Actions {
    list: [Option<Action>; MAX_ACTIONS],
    len: usize,
}

impl Actions {
    const NONE: Actions = Actions {
        list: [None; MAX_ACTIONS],
        len: 0,
    };

    fn push(&mut self, action: Action) {
        if let Some(slot) = self.list.get_mut(self.len) {
            *slot = Some(action);
            self.len += 1;
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = Action> + '_ {
        self.list[..self.len].iter().flatten().copied()
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

/// The phase for the hud: 0.46c2's state ids.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Pregame = 0,
    Waiting = 2,
    Spawned = 3,
    Carried = 4,
    Dropped = 5,
    Brought = 6,
    Fade = 7,
    Countdown = 8,
    Finished = 10,
}

/// Sudden death as the hud shows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SuddenShown {
    Off = 0,
    /// It turns on when the delivery's 3 s end.
    Armed = 1,
    On = 2,
}

/// The artefact as the hud and the world's view show it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArtefactView {
    Nowhere,
    Lying { spot: Spot, at: Position },
    Carried { by: Place, team: Team },
}

/// What the hud shows of the rule at a moment (the viewer's own part is the
/// game's).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hud {
    pub phase: Phase,
    pub score: [u8; 2],
    pub goal: u8,
    /// The clock shown: the match time left, in sudden death its own
    /// ([`Hud::clock_seconds`] its whole seconds).
    pub clock_ms: u32,
    pub clock_total_ms: u32,
    pub sudden_death: SuddenShown,
    /// What is left of the phase and its whole length: a lying artefact's
    /// life, the 3 s after a delivery, the fade, the countdown; 0 for the
    /// others.
    pub phase_ms: u32,
    pub phase_total_ms: u32,
    /// In [`Phase::Brought`], what follows it; else `None`.
    pub after: Option<After>,
    /// By team: who is still in while sudden death is on (0 otherwise), and
    /// who is in the match.
    pub alive: [u8; 2],
    pub total: [u8; 2],
    pub artefact: ArtefactView,
    /// The next respawn wave, while one comes: a round's hunt out of sudden
    /// death; `None` before the start, in a round's restart (respawns
    /// held), in sudden death and once over.
    pub wave: Option<Wave>,
}

/// The next respawn wave as the hud shows it: the ms to it and a wave's
/// whole length. Everyone's, both teams' alike: a death respawns in it
/// ([`Rule::respawn_at`]). The waves are the clock's multiples of
/// `respawn_time`, so [`Hud::clock_seconds`] less [`Wave::seconds`] is such a
/// multiple at every moment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Wave {
    pub left_ms: u32,
    pub total_ms: u32,
}

/// The whole seconds a time left is shown in: rounded up, as the game's own
/// countdowns (the respawn line, 0x316C20) and the movie's wave number
/// (`ArtefactHuntWave`): a countdown shows 0 only once it has run out.
pub const fn seconds_shown(ms: u32) -> u32 {
    ms.div_ceil(1000)
}

impl Hud {
    /// The clock's whole seconds, what the game shows of it (slot 17's
    /// hud+4): rounded up as the wave's, so that a wave comes as the clock
    /// turns to a multiple of `respawn_time` (the owner, 2026-11-27: "the
    /// general timer and wave timer should be similar", 14:53 beside 14,
    /// the clock rounded down).
    pub const fn clock_seconds(&self) -> u32 {
        seconds_shown(self.clock_ms)
    }
}

impl Wave {
    /// The wave's whole seconds left, as the movie shows them.
    pub const fn seconds(&self) -> u32 {
        seconds_shown(self.left_ms)
    }
}

impl Rule {
    /// A match's rule before its start. `seed` is the match's (0.46c2
    /// seeds the rule from the match's static data, 0x783ADE).
    pub const fn new(seed: u32) -> Rule {
        Rule {
            rng: Rng::seeded(seed),
            score: [0; 2],
            lives: Lives::NONE,
            sides: Sides::EMPTY,
            stage: Stage::Idle,
        }
    }

    pub const fn stage(&self) -> &Stage {
        &self.stage
    }

    pub const fn score(&self) -> [u8; 2] {
        self.score
    }

    /// How many rounds started after the match's first: each one a life of
    /// every player in the match then (the fade's end respawns everyone).
    pub const fn round_starts(&self) -> u16 {
        self.lives.round_starts()
    }

    pub const fn lives(&self) -> &Lives {
        &self.lives
    }

    pub const fn sides(&self) -> &Sides {
        &self.sides
    }

    pub const fn rng(&self) -> Rng {
        self.rng
    }

    /// The standing: the higher score, equal a draw (0.46c2 0x785770; its
    /// third case, equal scores with a team wiped, never stands here: the
    /// wiped team's point is given at once).
    pub fn leader(&self) -> Outcome {
        match self.score[0].cmp(&self.score[1]) {
            core::cmp::Ordering::Greater => Outcome::Winner(Team::Zero),
            core::cmp::Ordering::Less => Outcome::Winner(Team::One),
            core::cmp::Ordering::Equal => Outcome::Draw,
        }
    }

    /// The match's result, once it is over.
    pub fn result(&self) -> Option<(Outcome, Over)> {
        match self.stage {
            Stage::Over(over) => Some((self.leader(), over)),
            _ => None,
        }
    }

    /// The game's `rule_completed`: true from the match's end on.
    pub fn completed(&self, now: Time) -> bool {
        matches!(self.stage, Stage::Over(over) if now.reached(over.at))
    }

    fn round(&self) -> Option<&Round> {
        match &self.stage {
            Stage::Play(play) => Some(&play.round),
            _ => None,
        }
    }

    /// The round's restart, while one is on.
    fn restart(&self) -> Option<Restart<Option<Time>, Clock>> {
        match self.round() {
            Some(Round::Restart(restart)) => Some(*restart),
            _ => None,
        }
    }

    /// Who carries the artefact: slowed, no sprint, glowing, marked for both teams.
    pub fn carrier(&self) -> Option<Place> {
        match self.round() {
            Some(Round::Hunt {
                artefact: Artefact::Carried { by, .. },
                ..
            }) => Some(*by),
            _ => None,
        }
    }

    /// Whether sudden death is on.
    pub fn sudden_on(&self) -> bool {
        matches!(
            self.round(),
            Some(
                Round::Hunt {
                    clock: Clock::Sudden(_),
                    ..
                } | Round::Restart(
                    Restart::Countdown {
                        next: Clock::Sudden(_),
                        ..
                    } | Restart::Fade { next: Some(_), .. }
                )
            )
        )
    }

    /// Who is out for good in sudden death.
    pub fn fallen(&self) -> Roster {
        match self.round() {
            Some(
                Round::Hunt {
                    clock: Clock::Sudden(sudden),
                    ..
                }
                | Round::Restart(Restart::Countdown {
                    next: Clock::Sudden(sudden),
                    ..
                }),
            ) => sudden.fallen,
            _ => Roster::EMPTY,
        }
    }

    /// Who of `team` is still in: in the match and not fallen.
    pub fn standing(&self, team: Team) -> Roster {
        self.sides.of(team).minus(self.fallen())
    }

    /// Whether a death is given a respawn time: not in sudden death (the
    /// game's two "this team may respawn" flags are this, for both teams).
    pub fn respawns(&self) -> bool {
        !self.sudden_on()
    }

    /// Whether every player's input is held: the round's countdown (0.46c2
    /// flag 4 of state 8, 0x7B0BA0).
    pub fn frozen(&self) -> bool {
        self.restart().is_some_and(|restart| restart.frozen())
    }

    /// Whether a round is about to start: its fade and its countdown, which
    /// the game plays as the match start's own countdown (the warmup camera,
    /// patch 31's kit choice).
    pub fn round_countdown(&self) -> bool {
        self.restart().is_some_and(|restart| restart.round_countdown())
    }

    /// `place`'s kit switches in a round's countdown.
    pub fn kit_switches(&self, place: Place) -> u16 {
        self.lives.kit_switches(place.index())
    }

    /// What the respawn rule does at `now`, a step whose rule tick is still
    /// to come ([`Restart::spawning`]).
    pub fn spawning(&self, now: Time) -> Spawning {
        self.restart()
            .map_or(Spawning::Native, |restart| restart.spawning(now))
    }

    /// When a player dead at `death` (his death event's time) comes back
    /// while the match is played and deaths respawn: at the wave the hud
    /// showed at his death's step (the first wave after that step: a moment
    /// the clock shows a multiple of `respawn_time`), everyone's alike: the respawn
    /// modifier (the radio) has no effect in waves (the owner, 2026-11-23).
    /// 0.46c2 (0x785598..0x7856C4) takes the wave at or after `death +
    /// time_needed_to_respawn` (7 s), so a death in the 7 s before a wave
    /// missed it (the owner, 2026-11-23), and moves it by the modifier. The
    /// game puts the time on its simulation grid (the next step at or after
    /// it). `None` before the start and after the end (the respawn rule's
    /// own time stands) and in sudden death (nobody comes back). A time that
    /// falls in a round's restart is held by [`Rule::spawning`].
    pub fn respawn_at(&self, params: &Params, death: Time) -> Option<Time> {
        let Stage::Play(play) = self.stage else {
            return None;
        };
        if !self.respawns() {
            return None;
        }
        let least = death.after(AFTER_DEATH_STEP).elapsed(play.start);
        Some(Time::from_ms(play.start.ms().wrapping_add(wave_from(params, least))))
    }

    /// What follows the 3 s that end at `until` (its delivery `until` less
    /// `time_before_new_round`), from the match's `start`.
    pub fn after(&self, params: &Params, start: Time, until: Time) -> After {
        after(params, start, until, self.score)
    }

    /// The artefact for the world's view and the hud.
    pub fn artefact(&self, params: &Params) -> ArtefactView {
        match self.round() {
            Some(Round::Hunt {
                artefact: Artefact::Lying { spot, .. },
                ..
            }) => ArtefactView::Lying {
                spot: *spot,
                at: spot.position(params),
            },
            Some(Round::Hunt {
                artefact: Artefact::Carried { by, .. },
                ..
            }) => match self.sides.team_of(*by) {
                Some(team) => ArtefactView::Carried { by: *by, team },
                None => ArtefactView::Nowhere,
            },
            _ => ArtefactView::Nowhere,
        }
    }

    /// What the hud shows at `now`.
    pub fn hud(&self, params: &Params, now: Time) -> Hud {
        let mut hud = Hud {
            phase: Phase::Pregame,
            score: self.score,
            goal: params.goal,
            clock_ms: 0,
            clock_total_ms: params.match_time.get(),
            sudden_death: SuddenShown::Off,
            phase_ms: 0,
            phase_total_ms: 0,
            after: None,
            alive: [0; 2],
            total: [
                self.sides.of(Team::Zero).len(),
                self.sides.of(Team::One).len(),
            ],
            artefact: self.artefact(params),
            wave: None,
        };
        let play = match self.stage {
            Stage::Idle => return hud,
            Stage::Over(_) => {
                hud.phase = Phase::Finished;
                return hud;
            }
            Stage::Play(play) => play,
        };
        hud.clock_ms = params
            .match_time
            .get()
            .saturating_sub(now.elapsed(play.start));
        let sudden_from = match play.round {
            Round::Hunt {
                clock: Clock::Sudden(sudden),
                ..
            }
            | Round::Restart(Restart::Countdown {
                next: Clock::Sudden(sudden),
                ..
            }) => Some(sudden.from),
            Round::Restart(Restart::Fade { next, .. }) => next,
            _ => None,
        };
        if let Some(from) = sudden_from {
            hud.sudden_death = SuddenShown::On;
            hud.clock_total_ms = params.sudden_death.get();
            hud.clock_ms = params.sudden_death.get().saturating_sub(now.elapsed(from));
            hud.alive = [
                self.standing(Team::Zero).len(),
                self.standing(Team::One).len(),
            ];
        }
        let (phase, timer) = match play.round {
            // When the artefact appears is not shown.
            Round::Hunt {
                artefact: Artefact::Waiting { .. },
                ..
            } => (Phase::Waiting, None),
            Round::Hunt {
                artefact:
                    Artefact::Lying {
                        spot: Spot::Spawner(_),
                        until,
                        ..
                    },
                ..
            } => (Phase::Spawned, Some((until, params.not_picked_ttl))),
            Round::Hunt {
                artefact:
                    Artefact::Lying {
                        spot: Spot::Dropped(_),
                        until,
                        ..
                    },
                ..
            } => (Phase::Dropped, Some((until, params.dropped_ttl))),
            Round::Hunt {
                artefact: Artefact::Carried { .. },
                ..
            } => (Phase::Carried, None),
            Round::Restart(restart) => {
                let timer = Some((restart.until(), restart.length(params.restart)));
                match restart {
                    Restart::Decided { until } => {
                        let after = after(params, play.start, until, self.score);
                        if let After::SuddenDeath { .. } = after {
                            hud.sudden_death = SuddenShown::Armed;
                        }
                        hud.after = Some(after);
                        (Phase::Brought, timer)
                    }
                    Restart::Fade { .. } => (Phase::Fade, timer),
                    Restart::Countdown { .. } => (Phase::Countdown, timer),
                }
            }
        };
        hud.phase = phase;
        if let Round::Hunt {
            clock: Clock::Regular,
            ..
        } = play.round
        {
            // The wave of this very step has come (the respawn rule's tick
            // is before the hud's): the next one is after it.
            let passed = now.elapsed(play.start);
            hud.wave = Some(Wave {
                left_ms: wave_from(params, passed + 1) - passed,
                total_ms: params.respawn_wave.get(),
            });
        }
        if let Some((until, total)) = timer {
            hud.phase_ms = now.left(until);
            hud.phase_total_ms = total.get();
        }
        hud
    }

    /// The transition: the state after `event` and what the game does for
    /// it. Total: an event that means nothing in a state changes nothing.
    pub fn step(self, params: &Params, event: Event) -> (Rule, Actions) {
        let mut actions = Actions::NONE;
        let rule = self.apply(params, event, &mut actions);
        (rule, actions)
    }

    fn apply(mut self, params: &Params, event: Event, actions: &mut Actions) -> Rule {
        match event {
            Event::Start { now } => {
                if self.stage == Stage::Idle {
                    let artefact = self.wait(params, now);
                    self.stage = Stage::Play(Play {
                        start: now,
                        round: Round::Hunt {
                            clock: Clock::Regular,
                            artefact,
                        },
                    });
                }
            }
            Event::Tick { now } => {
                if let Stage::Play(play) = self.stage {
                    self.stage = self.tick(params, play, now);
                    self.wiped();
                }
            }
            Event::Entered { place, team, now } => {
                if self.sides.team_of(place) != Some(team) {
                    // Another player has the place now: nothing of the last
                    // one's is his.
                    self.vacate(params, place, now, None);
                    self.sides = self.sides.entered(place, team);
                    self.wiped();
                }
            }
            Event::Left { place, now, ground } => {
                self.vacate(params, place, now, ground);
                self.wiped();
            }
            Event::Died { place, now, ground } => {
                if self.sides.team_of(place).is_some() {
                    self.release(params, place, now, Some(ground));
                    if let Some(sudden) = self.sudden_mut() {
                        sudden.fallen = sudden.fallen.with(place);
                    }
                    self.wiped();
                }
            }
            Event::Take { place, .. } => self.take(params, place),
            Event::Drop { place, now, ground } => self.release(params, place, now, Some(ground)),
            Event::Deliver { place, now, ground } => {
                self.deliver(params, place, now, ground, actions)
            }
            Event::KitSwitched { place } => {
                if let Some(restart) = self.restart() {
                    self.lives.kit_switched(&restart, place.index());
                }
            }
        }
        self
    }

    /// Sudden death's fallen, in a round that counts them.
    fn sudden_mut(&mut self) -> Option<&mut Sudden> {
        match &mut self.stage {
            Stage::Play(Play {
                round:
                    Round::Hunt {
                        clock: Clock::Sudden(sudden),
                        ..
                    }
                    | Round::Restart(Restart::Countdown {
                        next: Clock::Sudden(sudden),
                        ..
                    }),
                ..
            }) => Some(sudden),
            _ => None,
        }
    }

    /// A wait for the next artefact from `now` (0.46c2 0x7B0DC0).
    fn wait(&mut self, params: &Params, now: Time) -> Artefact {
        let (rng, draw) = self.rng.below(params.wait_range);
        self.rng = rng;
        // The wait is at most a day: a duration.
        Artefact::Waiting {
            until: now
                .after(params.wait_least)
                .after(Ms::new(draw).unwrap_or(Ms::MAX)),
        }
    }

    /// Sudden death's wiped team (0.46c2 0x785F80): with equal scores a
    /// team with nobody left gives the other team 1, team 0 tested first.
    fn wiped(&mut self) {
        if !self.sudden_on() || self.score[0] != self.score[1] {
            return;
        }
        if self.standing(Team::Zero).is_empty() {
            self.score[1] = self.score[1].saturating_add(1);
        } else if self.standing(Team::One).is_empty() {
            self.score[0] = self.score[0].saturating_add(1);
        }
    }

    fn take(&mut self, params: &Params, place: Place) {
        let Some(team) = self.sides.team_of(place) else {
            return;
        };
        if !self.standing(team).has(place) {
            return;
        }
        if let Stage::Play(Play {
            round: Round::Hunt { artefact, .. },
            ..
        }) = &mut self.stage
            && let Artefact::Lying { spot, way, .. } = *artefact
        {
            let way = way.touched(params, place, team, spot.position(params));
            *artefact = Artefact::Carried { by: place, way };
        }
    }

    /// The carrier `place` lets go of the artefact (the Drop key, his
    /// death, his leaving): it lies a little over `ground` for
    /// `vi_dropped_ttl`; with no ground it is gone and another is waited for.
    fn release(&mut self, params: &Params, place: Place, now: Time, ground: Option<Position>) {
        if self.carrier() != Some(place) {
            return;
        }
        let Stage::Play(Play {
            round:
                Round::Hunt {
                    artefact: Artefact::Carried { way, .. },
                    ..
                },
            ..
        }) = self.stage
        else {
            return;
        };
        let next = match (ground, self.sides.team_of(place)) {
            (Some(ground), Some(team)) => {
                let at = ground.raised(params.drop_lift);
                Artefact::Lying {
                    spot: Spot::Dropped(at),
                    until: now.after(params.dropped_ttl),
                    way: way.touched(params, place, team, at),
                }
            }
            _ => self.wait(params, now),
        };
        if let Stage::Play(Play {
            round: Round::Hunt { artefact, .. },
            ..
        }) = &mut self.stage
        {
            *artefact = next;
        }
    }

    /// `place` is out of the match: what it carried is let go, its part of
    /// the way is forgotten.
    fn vacate(&mut self, params: &Params, place: Place, now: Time, ground: Option<Position>) {
        self.release(params, place, now, ground);
        if let Some(sudden) = self.sudden_mut() {
            sudden.fallen = sudden.fallen.without(place);
        }
        if let Stage::Play(Play {
            round:
                Round::Hunt {
                    artefact: Artefact::Lying { way, .. } | Artefact::Carried { way, .. },
                    ..
                },
            ..
        }) = &mut self.stage
        {
            *way = way.forgetting(place);
        }
        self.sides = self.sides.left(place);
    }

    fn deliver(
        &mut self,
        params: &Params,
        place: Place,
        now: Time,
        ground: Position,
        actions: &mut Actions,
    ) {
        let Stage::Play(Play {
            start,
            round:
                Round::Hunt {
                    clock,
                    artefact: Artefact::Carried { by, way },
                },
        }) = self.stage
        else {
            return;
        };
        let Some(team) = self.sides.team_of(place) else {
            return;
        };
        if by != place {
            return;
        }
        let way = way.touched(params, place, team, ground);
        actions.push(Action::Put { place });
        for (place, hundredths) in way.shares(&self.sides, team) {
            actions.push(Action::Bring { place, hundredths });
        }
        let scored = &mut self.score[team.index()];
        *scored = scored.saturating_add(1).min(params.goal);
        self.stage = match clock {
            // A delivery wins sudden death: the scores differ, and the
            // rule's first test in state 4 is "match over" (0x785E10).
            Clock::Sudden(_) => Stage::Over(Over {
                at: now,
                reason: Reason::TimeOut,
            }),
            Clock::Regular => Stage::Play(Play {
                start,
                round: Round::Restart(Restart::decided(now, params.restart)),
            }),
        };
    }

    /// One step of the simulation (0.46c2's tick 0x785130 and its state
    /// machine 0x844F70): at most one round transition.
    fn tick(&mut self, params: &Params, play: Play, now: Time) -> Stage {
        let Play { start, round } = play;
        let over = |reason| Stage::Over(Over { at: now, reason });
        let timed_out = now.elapsed(start) >= params.match_time.get();
        let play = |round| Stage::Play(Play { start, round });
        match round {
            Round::Hunt { clock, artefact } => {
                let equal = self.score[0] == self.score[1];
                // "Match over" (0x785E10), tested in the hunt's states only.
                let clock = match clock {
                    // The match time ran out: the higher score wins at once,
                    // equal scores go to sudden death (0x78517D).
                    Clock::Regular if !timed_out => Clock::Regular,
                    Clock::Regular if equal => Clock::Sudden(Sudden {
                        from: now,
                        fallen: Roster::EMPTY,
                    }),
                    Clock::Regular => return over(Reason::TimeOut),
                    Clock::Sudden(sudden) => {
                        let spent = now
                            .since(sudden.from)
                            .is_some_and(|passed| passed >= params.sudden_death.get());
                        // Unequal in sudden death: a team was wiped (a delivery
                        // there ends the match at once).
                        if !equal {
                            return over(Reason::Wiped);
                        }
                        if spent {
                            return over(Reason::TimeOut);
                        }
                        clock
                    }
                };
                let artefact = match artefact {
                    Artefact::Waiting { until } if now.reached(until) => {
                        // A spawner, each as likely (0x7B0E90).
                        let (rng, index) = self.rng.below(u32::from(params.spawner_count));
                        self.rng = rng;
                        let spot = Spot::Spawner(index as u8);
                        Artefact::Lying {
                            spot,
                            until: now.after(params.not_picked_ttl),
                            way: Way::begin(params, spot.position(params)),
                        }
                    }
                    // Nobody took it in its time: removed (0x7981EB).
                    Artefact::Lying { until, .. } if now.reached(until) => self.wait(params, now),
                    other => other,
                };
                play(Round::Hunt { clock, artefact })
            }
            Round::Restart(restart) => {
                // Everyone respawned at the fade's end: nobody has fallen yet.
                let clock = |sudden: Option<Time>| match sudden {
                    Some(from) => Clock::Sudden(Sudden {
                        from,
                        fallen: Roster::EMPTY,
                    }),
                    None => Clock::Regular,
                };
                match restart.step(now, params.restart, &mut self.lives, clock) {
                    Step::Stays(restart) | Step::Started(restart) => play(Round::Restart(restart)),
                    Step::Decided { until } => match after(params, start, until, self.score) {
                        After::End => over(if timed_out {
                            Reason::TimeOut
                        } else {
                            Reason::Collected
                        }),
                        After::NewRound => play(Round::Restart(Restart::fade(now, None))),
                        // The transition state turns it on (0x7B1004).
                        After::SuddenDeath { from } => {
                            play(Round::Restart(Restart::fade(now, Some(from))))
                        }
                    },
                    Step::Next(clock) => {
                        let artefact = self.wait(params, now);
                        play(Round::Hunt { clock, artefact })
                    }
                }
            }
        }
    }
}

/// The one time test of a delivery made at `until` less
/// `time_before_new_round` with the scores after it.
/// The first wave at or after `passed` ms from the match's start (not the
/// start itself), in ms from the start. The waves are the moments the
/// clock shows a multiple of `respawn_time` (`match_time` less a wave is one),
/// both teams' the same: the multiples of `respawn_time` from the start when
/// `match_time` is one (every map's, 900 s), else shifted by what is left.
fn wave_from(params: &Params, passed: u32) -> u32 {
    let wave = params.respawn_wave.get();
    let shift = (wave - params.match_time.get() % wave) % wave;
    (passed.saturating_add(shift).saturating_sub(1) / wave + 1) * wave - shift
}

fn after(params: &Params, start: Time, until: Time, score: [u8; 2]) -> After {
    let delivered = Time::from_ms(until.ms().wrapping_sub(params.restart.decided().get()));
    // Less match time left than `time_needed_to_continue_match`.
    let late = delivered.elapsed(start) > params.rounds_until.get();
    if score[0] >= params.goal || score[1] >= params.goal || (late && score[0] != score[1]) {
        After::End
    } else if late {
        After::SuddenDeath {
            from: until.after(params.restart.countdown()),
        }
    } else {
        After::NewRound
    }
}
