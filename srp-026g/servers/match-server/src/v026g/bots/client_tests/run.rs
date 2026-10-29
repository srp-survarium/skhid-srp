//! The runner: one state machine over the steps of a match's group. It sees [`Frame`]s and answers with [`Action`]s
//! (words for the owner, log lines, result rows) and, for each scripted bot, what to do now ([`Runner::drive`]). It
//! does no I/O, so it is tested with a made-up owner.
//!
//! ```text
//! Waiting --(play on, owner in and alive, a step left)--> Armed --(start conditions, owner alive)--> Running --(end)--> Rest --> Waiting
//!    |                                                       |  \--(arm time out)--> row VOID never_started
//!    \--(no step left)--> Finished                           \--(no owner)--> back to the queue
//! ```

use std::collections::VecDeque;

use mapdata::Vec3;

use super::frame::{Body, Frame, Owner};
use super::step::{Cond, CustomMeter, End, Gait, Grip, Group, Measure, Progress, Script, Setup, Side, Span, Spot, Step, TryStart, Who};
use super::time::At;
use crate::v026g::bots::view::Place;

/// A chat line fades after about 7 s (GUESS: not timed on the client's screen): the instruction is told again this often while
/// the step waits for its start.
const RETELL: Span = Span::s(6);
/// After a step ends the next one waits this long (the owner reads the result).
const REST: Span = Span::s(4);
/// A scripted bot within this of a lane point has reached it, m.
const LEG_REACHED_M: f32 = 1.5;
/// A step is not run again after this many tries in a row were voided by the owner leaving.
const OWNER_GONE_LIMIT: u8 = 3;
/// A jump of the owner's reckoning over this in one poll is a respawn, not walking, m.
const RESPAWN_JUMP_M: f32 = 12.0;

/// A step with the tries left.
#[derive(Clone, Debug)]
pub struct Todo {
    pub step: Step,
    pub next_try: u8,
}

impl Todo {
    pub fn new(step: Step) -> Self {
        Self { step, next_try: 1 }
    }
}

/// What a row and the card say about the build they were made on: the commit of the source (`SRP_BUILD_SHA` at run time, or
/// baked in at build time; a server without one does not start), the hash of the tuning the bots ran on (`Tuning::hash`) and
/// where it came from (`LoadedTuning::data_line`: `tuning=defaults`, or the file and the digest of its bytes, which is what a
/// bench folder records), so a row can be matched to a bench run.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Provenance {
    pub sha: String,
    pub tuning: u32,
    pub tuning_source: String,
}

/// Why a try has no verdict.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VoidWhy {
    /// The owner did not reach the start in time.
    NeverStarted,
    /// The match lacks the bots a setup names.
    MissingBots(String),
    /// The owner left the match.
    OwnerGone,
    /// The match ended while the step ran.
    MatchEnded,
}

impl VoidWhy {
    pub fn text(&self) -> String {
        match self {
            VoidWhy::NeverStarted => "never_started".into(),
            VoidWhy::MissingBots(who) => format!("missing_bots({who})"),
            VoidWhy::OwnerGone => "owner_gone".into(),
            VoidWhy::MatchEnded => "match_ended".into(),
        }
    }
}

/// The verdict of a try.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Pass,
    /// The bars it missed.
    Fail(String),
    /// The step has no bar: the values are the result.
    Info,
    Void(VoidWhy),
}

impl Outcome {
    pub fn text(&self) -> String {
        match self {
            Outcome::Pass => "PASS".into(),
            Outcome::Fail(why) => format!("FAIL({why})"),
            Outcome::Info => "INFO".into(),
            Outcome::Void(why) => format!("VOID({})", why.text()),
        }
    }

    /// Whether the try is used up. A try the owner or the match cut short is run again, in this match or the next.
    pub fn counts(&self) -> bool {
        !matches!(self, Outcome::Void(VoidWhy::OwnerGone | VoidWhy::MatchEnded))
    }
}

/// One result row: a try.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub match_id: u32,
    pub provenance: Provenance,
    pub step: &'static str,
    pub package: &'static str,
    pub try_no: u8,
    pub tries: u8,
    pub outcome: Outcome,
    pub ended_by: &'static str,
    pub start: At,
    pub end: At,
    pub values: Vec<(&'static str, Option<f64>)>,
}

/// What the runner asks for.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    /// A line for the owner.
    Tell(String),
    /// A log line (`CLIENT_TEST ...`).
    Log(String),
    Row(Row),
}

/// What a scripted bot's body does now.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Act {
    /// Stands where it is, crouched or not.
    Stand { crouch: bool },
    /// Stands where it is looking at `look`.
    Face { look: Vec3, crouch: bool },
    /// Goes to `at` and stays within `within_m`; crouches there if told.
    Go { at: Vec3, within_m: f32, gait: Gait, crouch: bool },
    /// Looks at `look` with the throw key held or let go.
    Throw { look: Vec3, held: bool },
}

/// What a scripted bot does now, and what of it the script leaves to the bot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drive {
    pub act: Act,
    pub grip: Grip,
}

/// Where a throw is.
#[derive(Clone, Copy, Debug, PartialEq)]
enum ThrowPhase {
    /// The step has not started: the bot stands.
    Waiting,
    Winding { since: At },
    Thrown,
}

/// A script with the state it carries: a variant for each, so a state is only where the script has it.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Motion {
    Stand { crouch: bool },
    Face { look: Vec3, crouch: bool },
    Hold { at: Vec3, within_m: f32, crouch: bool },
    Walk { lane: &'static [Spot], gait: Gait, looped: bool, leg: usize },
    Follow { within_m: f32, gait: Gait },
    Throw { at: Vec3, wind: Span, phase: ThrowPhase },
}

impl Motion {
    fn of(script: Script) -> Self {
        match script {
            Script::Idle => Motion::Stand { crouch: false },
            Script::Stand { crouch } => Motion::Stand { crouch },
            Script::Face { at, crouch } => Motion::Face { look: at.vec(), crouch },
            Script::Hold { at, within_m, crouch } => Motion::Hold { at: at.vec(), within_m, crouch },
            Script::Walk { lane, gait, looped } => Motion::Walk { lane, gait, looped, leg: 0 },
            Script::Follow { within_m, gait } => Motion::Follow { within_m, gait },
            Script::Throw { at, wind } => Motion::Throw { at: at.vec(), wind, phase: ThrowPhase::Waiting },
        }
    }
}

#[derive(Clone, Debug)]
struct Member {
    place: Place,
    motion: Motion,
    grip: Grip,
    act: Act,
}

impl Member {
    fn new(place: Place, setup: &Setup) -> Self {
        Self {
            place,
            motion: Motion::of(setup.script),
            grip: setup.grip,
            act: Act::Stand { crouch: false },
        }
    }

    /// The step starts: a throw begins to wind.
    fn begin(&mut self, clock: At) {
        if let Motion::Throw { phase: phase @ ThrowPhase::Waiting, .. } = &mut self.motion {
            *phase = ThrowPhase::Winding { since: clock };
        }
    }

    /// The next state and what the body does, from the bot's own body and the owner.
    fn advance(&mut self, me: Option<&Body>, owner: &Body, clock: At) {
        let feet = me.map(|b| b.feet);
        self.act = match &mut self.motion {
            Motion::Stand { crouch } => Act::Stand { crouch: *crouch },
            Motion::Face { look, crouch } => Act::Face { look: *look, crouch: *crouch },
            Motion::Hold { at, within_m, crouch } => Act::Go {
                at: *at,
                within_m: *within_m,
                gait: Gait::Run,
                crouch: *crouch,
            },
            Motion::Follow { within_m, gait } => Act::Go {
                at: owner.feet,
                within_m: *within_m,
                gait: *gait,
                crouch: false,
            },
            Motion::Walk { lane, gait, looped, leg } => {
                if lane.is_empty() {
                    Act::Stand { crouch: false }
                } else {
                    let reached = feet.is_some_and(|f| f.distance_xz(lane[*leg].vec()) <= LEG_REACHED_M);
                    if reached {
                        if *leg + 1 < lane.len() {
                            *leg += 1;
                        } else if *looped {
                            *leg = 0;
                        }
                    }
                    Act::Go {
                        at: lane[*leg].vec(),
                        within_m: LEG_REACHED_M,
                        gait: *gait,
                        crouch: false,
                    }
                }
            }
            Motion::Throw { at, wind, phase } => {
                let from = feet.unwrap_or(owner.feet);
                // A lob at about 45 degrees: the view rises with the distance.
                let look = *at + Vec3::new(0.0, from.distance_xz(*at), 0.0);
                match *phase {
                    ThrowPhase::Waiting => Act::Stand { crouch: false },
                    ThrowPhase::Winding { since } if clock - since >= *wind => {
                        *phase = ThrowPhase::Thrown;
                        Act::Throw { look, held: false }
                    }
                    ThrowPhase::Winding { .. } => Act::Throw { look, held: true },
                    ThrowPhase::Thrown => Act::Throw { look, held: false },
                }
            }
        };
    }

    fn drive(&self) -> Drive {
        Drive { act: self.act, grip: self.grip }
    }
}

/// Who plays which script in this try.
#[derive(Clone, Debug, Default)]
struct Crew(Vec<Member>);

impl Crew {
    /// Resolves the setup against the bots in the match, or names the first side that lacks a bot.
    fn pick(step: &Step, frame: &Frame) -> Result<Self, String> {
        let mut members: Vec<Member> = Vec::new();
        for setup in &step.setup {
            let (side, nth) = match setup.who {
                Who::Nth(side, n) => (side, Some(usize::from(n))),
                Who::All(side) => (side, None),
            };
            let mut places: Vec<Place> = frame.side(side).map(|b| b.body.place).collect();
            places.sort_unstable();
            let chosen: Vec<Place> = match nth {
                Some(n) => places.get(n).copied().into_iter().collect(),
                None => places,
            };
            if chosen.is_empty() {
                return Err(format!("{} bot{}", side.name(), nth.map_or(String::new(), |n| format!(" {n}"))));
            }
            for place in chosen {
                // A later setup for the same bot replaces the earlier.
                members.retain(|m| m.place != place);
                members.push(Member::new(place, setup));
            }
        }
        Ok(Self(members))
    }

    fn advance(&mut self, frame: &Frame, owner: &Body) {
        for member in &mut self.0 {
            let me = frame.bot(member.place).map(|b| &b.body);
            member.advance(me, owner, frame.clock);
        }
    }

    fn begin(&mut self, clock: At) {
        for member in &mut self.0 {
            member.begin(clock);
        }
    }

    fn in_place(&self, frame: &Frame) -> bool {
        self.0.iter().all(|member| match member.motion {
            Motion::Hold { at, within_m, .. } => frame
                .bot(member.place)
                .is_some_and(|b| b.body.alive && b.body.feet.distance_xz(at) <= within_m + 1.0),
            _ => true,
        })
    }

    fn drive(&self, place: Place) -> Option<Drive> {
        self.0.iter().find(|m| m.place == place).map(Member::drive)
    }
}

/// One measure under way: its names and the state it keeps.
#[derive(Debug)]
struct Meter {
    /// The names its values go under, the single place they are written (`Measure::all_names`).
    names: Vec<&'static str>,
    kind: Kind,
}

/// What a measure keeps: a variant for each, with the state it keeps.
#[derive(Debug)]
enum Kind {
    Elapsed,
    OwnerKills,
    OwnerDeaths,
    FirstKill(Option<Span>),
    DiedAt(Option<Span>),
    MinHealth(Option<f64>),
    Travel { total: f64, last: Option<Vec3> },
    SideKills { side: Side, total: u32, seen: Vec<(Place, u32)> },
    Nearest { side: Side, low: Option<f64> },
    Notes { contains: &'static str, count: u32 },
    Seconds { side: Side, at: Vec3, r_m: f32, total: Span, last: At },
    /// A package's own measure and how many names it has.
    Custom { meter: Box<dyn CustomMeter>, count: usize },
}

#[derive(Clone, Copy, Debug)]
struct Base {
    started: At,
    kills: u32,
    deaths: u32,
}

impl Meter {
    fn new(measure: Measure, base: Base) -> Self {
        Self {
            names: measure.all_names(),
            kind: Kind::new(measure, base),
        }
    }

    fn sample(&mut self, frame: &Frame, owner: &Owner, base: Base) {
        self.kind.sample(frame, owner, base);
    }

    /// The values at the end, each under its name.
    fn values(&self, base: Base, end: &Frame, owner: &Owner) -> Vec<(&'static str, Option<f64>)> {
        self.names.iter().copied().zip(self.kind.values(base, end, owner)).collect()
    }
}

impl Kind {
    fn new(measure: Measure, base: Base) -> Self {
        match measure {
            Measure::ElapsedMs => Kind::Elapsed,
            Measure::OwnerKills => Kind::OwnerKills,
            Measure::OwnerDeaths => Kind::OwnerDeaths,
            Measure::FirstKillMs => Kind::FirstKill(None),
            Measure::DiedAtMs => Kind::DiedAt(None),
            Measure::MinHealth => Kind::MinHealth(None),
            Measure::TravelM => Kind::Travel { total: 0.0, last: None },
            Measure::SideKills(side) => Kind::SideKills { side, total: 0, seen: Vec::new() },
            Measure::NearestBotM(side) => Kind::Nearest { side, low: None },
            Measure::Notes { contains, .. } => Kind::Notes { contains, count: 0 },
            Measure::SecondsInArea { side, at, r_m, .. } => Kind::Seconds {
                side,
                at: at.vec(),
                r_m,
                total: Span(0),
                last: base.started,
            },
            Measure::Custom { names, make } => Kind::Custom {
                meter: make(&TryStart {
                    started: base.started,
                    kills: base.kills,
                    deaths: base.deaths,
                }),
                count: names.len(),
            },
        }
    }

    fn sample(&mut self, frame: &Frame, owner: &Owner, base: Base) {
        let body = &owner.body;
        let since = frame.clock - base.started;
        match self {
            Kind::Elapsed | Kind::OwnerKills | Kind::OwnerDeaths => {}
            Kind::FirstKill(first) => {
                if first.is_none() && body.kills > base.kills {
                    *first = Some(since);
                }
            }
            Kind::DiedAt(first) => {
                if first.is_none() && (body.deaths > base.deaths || !body.alive) {
                    *first = Some(since);
                }
            }
            Kind::MinHealth(low) => {
                if body.alive {
                    let h = f64::from(body.health);
                    *low = Some(low.map_or(h, |l| l.min(h)));
                }
            }
            Kind::Nearest { side, low } => {
                if body.alive {
                    for bot in frame.side(*side).filter(|b| b.body.alive) {
                        let d = f64::from(bot.body.feet.distance_xz(body.feet));
                        *low = Some(low.map_or(d, |l| l.min(d)));
                    }
                }
            }
            Kind::Travel { total, last } => {
                if body.alive {
                    if let Some(prev) = last {
                        let d = prev.distance_xz(body.feet);
                        if d < RESPAWN_JUMP_M {
                            *total += f64::from(d);
                        }
                    }
                    *last = Some(body.feet);
                } else {
                    *last = None;
                }
            }
            Kind::SideKills { side, total, seen } => {
                for bot in frame.side(*side) {
                    match seen.iter_mut().find(|(p, _)| *p == bot.body.place) {
                        Some((_, kills)) => {
                            *total += bot.body.kills.saturating_sub(*kills);
                            *kills = bot.body.kills;
                        }
                        None => seen.push((bot.body.place, bot.body.kills)),
                    }
                }
            }
            Kind::Notes { contains, count, .. } => {
                *count += frame.heard.iter().filter(|h| h.text.contains(*contains)).count() as u32;
            }
            Kind::Seconds { side, at, r_m, total, last, .. } => {
                if frame.side(*side).any(|b| b.body.alive && b.body.feet.distance_xz(*at) <= *r_m) {
                    *total = *total + (frame.clock - *last);
                }
                *last = frame.clock;
            }
            Kind::Custom { meter, .. } => meter.sample(frame),
        }
    }

    /// The values at the end, one for each name.
    /// The values at the end, one for each name of the measure, in order.
    fn values(&self, base: Base, end: &Frame, owner: &Owner) -> Vec<Option<f64>> {
        let body = &owner.body;
        let ms = |span: Option<Span>| span.map(|s| f64::from(s.as_ms()));
        match self {
            Kind::Elapsed => vec![Some(f64::from((end.clock - base.started).as_ms()))],
            Kind::OwnerKills => vec![Some(f64::from(body.kills.saturating_sub(base.kills)))],
            Kind::OwnerDeaths => vec![Some(f64::from(body.deaths.saturating_sub(base.deaths)))],
            Kind::FirstKill(first) => vec![ms(*first)],
            Kind::DiedAt(first) => vec![ms(*first)],
            Kind::MinHealth(low) => vec![*low],
            Kind::Travel { total, .. } => vec![Some(*total)],
            Kind::SideKills { total, .. } => vec![Some(f64::from(*total))],
            Kind::Nearest { low, .. } => vec![*low],
            Kind::Notes { count, .. } => vec![Some(f64::from(*count))],
            Kind::Seconds { total, .. } => vec![Some(total.as_s())],
            Kind::Custom { meter, count } => (0..*count).map(|n| meter.value(n)).collect(),
        }
    }
}

/// A try under way.
#[derive(Debug)]
struct Running {
    todo: Todo,
    crew: Crew,
    base: Base,
    meters: Vec<Meter>,
}

#[derive(Clone, Debug)]
struct Armed {
    todo: Todo,
    crew: Crew,
    since: At,
    /// When the instruction was last told.
    told: At,
}

#[derive(Debug, Default)]
enum Phase {
    #[default]
    Waiting,
    Armed(Armed),
    Running(Running),
    /// The result is on the owner's screen; the next step waits.
    Rest {
        until: At,
    },
    /// No step is left for this group.
    Finished,
}

/// The runner of one match.
#[derive(Debug)]
pub struct Runner {
    match_id: u32,
    provenance: Provenance,
    played: Group,
    queue: VecDeque<Todo>,
    /// The group after this one, to tell the owner (none: the plan ends here).
    next_group: Option<Group>,
    phase: Phase,
    /// The last poll's clock, for a row written when the match ends.
    last: At,
    /// Tries in a row voided by the owner leaving, per step id.
    gone: Vec<(&'static str, u8)>,
}

impl Runner {
    pub fn new(match_id: u32, provenance: Provenance, played: Group, todo: Vec<Todo>, next_group: Option<Group>) -> Self {
        Self {
            match_id,
            provenance,
            played,
            queue: todo.into(),
            next_group,
            phase: Phase::Waiting,
            last: At(0),
            gone: Vec::new(),
        }
    }

    pub fn is_finished(&self) -> bool {
        matches!(self.phase, Phase::Finished)
    }

    /// What the bot at `place` is scripted to do now, if anything.
    pub fn drive(&self, place: Place) -> Option<Drive> {
        match &self.phase {
            Phase::Armed(armed) => armed.crew.drive(place),
            Phase::Running(running) => running.crew.drive(place),
            Phase::Waiting | Phase::Rest { .. } | Phase::Finished => None,
        }
    }

    /// The match is over: a try under way is void (and not used up).
    pub fn finish(&mut self) -> Vec<Action> {
        let mut acts = Vec::new();
        if let Phase::Running(running) = std::mem::take(&mut self.phase) {
            let row = self.row(&running, Outcome::Void(VoidWhy::MatchEnded), "match_end", self.last);
            acts.push(Action::Log(end_line(&row)));
            acts.push(Action::Row(row));
        }
        self.phase = Phase::Finished;
        acts
    }

    pub fn tick(&mut self, frame: &Frame) -> Vec<Action> {
        let mut acts = Vec::new();
        self.last = frame.clock;
        self.phase = match std::mem::take(&mut self.phase) {
            Phase::Waiting => self.waiting(frame, &mut acts),
            Phase::Rest { until } if frame.clock >= until => self.waiting(frame, &mut acts),
            Phase::Rest { until } => Phase::Rest { until },
            Phase::Armed(armed) => self.armed(armed, frame, &mut acts),
            Phase::Running(running) => self.running(running, frame, &mut acts),
            Phase::Finished => Phase::Finished,
        };
        acts
    }

    fn waiting(&mut self, frame: &Frame, acts: &mut Vec<Action>) -> Phase {
        // A step is armed with the owner in the match and on his feet: a dead owner is told nothing he cannot act on.
        let Some(owner) = frame.owner.as_ref().filter(|o| frame.playing && o.body.alive) else {
            return Phase::Waiting;
        };
        let Some(todo) = self.queue.pop_front() else {
            let done = self.played.describe();
            acts.push(Action::Log(format!("CLIENT_TEST group {done} DONE")));
            acts.push(Action::Tell(match self.next_group {
                Some(next) => format!(
                    "That is all for {done}. Next: {}. Leave this match from the in-game menu and start Player vs Bots there.",
                    next.describe()
                ),
                None => "That is all: every step of the plan is run.".to_string(),
            }));
            return Phase::Finished;
        };
        match Crew::pick(&todo.step, frame) {
            Ok(mut crew) => {
                crew.advance(frame, &owner.body);
                acts.push(Action::Tell(format!("[{}/{}] {}", todo.step.id, todo.next_try, todo.step.instruction)));
                acts.push(Action::Log(format!(
                    "CLIENT_TEST step {} ARMED try {}/{}: {}",
                    todo.step.id, todo.next_try, todo.step.tries, todo.step.instruction
                )));
                Phase::Armed(Armed {
                    todo,
                    crew,
                    since: frame.clock,
                    told: frame.clock,
                })
            }
            Err(lacking) => {
                let row = Row {
                    match_id: self.match_id,
                    provenance: self.provenance.clone(),
                    step: todo.step.id,
                    package: todo.step.package,
                    try_no: todo.next_try,
                    tries: todo.step.tries,
                    outcome: Outcome::Void(VoidWhy::MissingBots(lacking.replace(' ', "_"))),
                    ended_by: "setup",
                    start: frame.clock,
                    end: frame.clock,
                    values: Vec::new(),
                };
                acts.push(Action::Log(end_line(&row)));
                acts.push(Action::Row(row));
                self.used_up(todo);
                Phase::Waiting
            }
        }
    }

    fn armed(&mut self, mut armed: Armed, frame: &Frame, acts: &mut Vec<Action>) -> Phase {
        let Some(owner) = frame.owner.as_ref() else {
            // The owner left before it began: the try is not used.
            self.queue.push_front(armed.todo);
            return Phase::Waiting;
        };
        armed.crew.advance(frame, &owner.body);
        let waited = frame.clock - armed.since;
        if waited >= armed.todo.step.arm {
            let step = &armed.todo.step;
            let row = Row {
                match_id: self.match_id,
                provenance: self.provenance.clone(),
                step: step.id,
                package: step.package,
                try_no: armed.todo.next_try,
                tries: step.tries,
                outcome: Outcome::Void(VoidWhy::NeverStarted),
                ended_by: "arm_time",
                start: frame.clock,
                end: frame.clock,
                values: Vec::new(),
            };
            acts.push(Action::Tell(format!("[{}] not started in time; on to the next.", step.id)));
            acts.push(Action::Log(end_line(&row)));
            acts.push(Action::Row(row));
            self.used_up(armed.todo);
            return Phase::Rest { until: frame.clock + REST };
        }
        // A step starts with the owner on his feet and after he had time to read it.
        let begins = owner.body.alive && waited >= armed.todo.step.settle && armed.todo.step.start.iter().all(|cond| holds(cond, &armed, frame, owner));
        if !begins {
            if frame.clock - armed.told >= RETELL {
                armed.told = frame.clock;
                acts.push(Action::Tell(format!("[{}] {}", armed.todo.step.id, armed.todo.step.instruction)));
            }
            return Phase::Armed(armed);
        }
        let base = Base {
            started: frame.clock,
            kills: owner.body.kills,
            deaths: owner.body.deaths,
        };
        let meters = armed.todo.step.measures.iter().map(|m| Meter::new(*m, base)).collect();
        armed.crew.begin(frame.clock);
        acts.push(Action::Log(format!(
            "CLIENT_TEST step {} START try {}/{}",
            armed.todo.step.id, armed.todo.next_try, armed.todo.step.tries
        )));
        Phase::Running(Running {
            todo: armed.todo,
            crew: armed.crew,
            base,
            meters,
        })
    }

    fn running(&mut self, mut run: Running, frame: &Frame, acts: &mut Vec<Action>) -> Phase {
        let Some(owner) = frame.owner.as_ref() else {
            let row = self.row(&run, Outcome::Void(VoidWhy::OwnerGone), "owner_gone", frame.clock);
            acts.push(Action::Log(end_line(&row)));
            acts.push(Action::Row(row));
            let strikes = match self.gone.iter_mut().find(|(id, _)| *id == run.todo.step.id) {
                Some((_, n)) => {
                    *n += 1;
                    *n
                }
                None => {
                    self.gone.push((run.todo.step.id, 1));
                    1
                }
            };
            if strikes < OWNER_GONE_LIMIT {
                self.queue.push_front(run.todo);
            }
            return Phase::Waiting;
        };
        run.crew.advance(frame, &owner.body);
        for meter in &mut run.meters {
            meter.sample(frame, owner, run.base);
        }
        let step = &run.todo.step;
        let body = &owner.body;
        let progress = Progress {
            since_start: frame.clock - run.base.started,
            owner_kills: body.kills.saturating_sub(run.base.kills),
            owner_deaths: body.deaths.saturating_sub(run.base.deaths),
        };
        let scripted_died = |who: Who| match who {
            Who::All(side) => frame.side(side).any(|b| !b.body.alive && run.crew.0.iter().any(|m| m.place == b.body.place)),
            Who::Nth(side, n) => {
                let mut places: Vec<Place> = frame.side(side).map(|b| b.body.place).collect();
                places.sort_unstable();
                places.get(usize::from(n)).is_some_and(|p| frame.bot(*p).is_some_and(|b| !b.body.alive))
            }
        };
        let ended_by = step
            .end
            .iter()
            .find_map(|end| match *end {
                End::OwnerDied => (progress.owner_deaths > 0 || !body.alive).then_some("owner_died"),
                End::OwnerKills(n) => (progress.owner_kills >= n).then_some("owner_kills"),
                End::OwnerLeft { at, r_m } => (body.alive && body.feet.distance_xz(at.vec()) > r_m).then_some("owner_left"),
                End::ScriptedDied(who) => scripted_died(who).then_some("scripted_died"),
                End::Custom(when) => (when.holds)(frame, &progress).then_some(when.reason),
            })
            .or((progress.since_start >= step.budget).then_some("budget"));
        let Some(ended_by) = ended_by else {
            return Phase::Running(run);
        };
        let values: Vec<(&'static str, Option<f64>)> = run.meters.iter().flat_map(|m| m.values(run.base, frame, owner)).collect();
        let outcome = judge(step, &values);
        let mut row = self.row(&run, outcome, ended_by, frame.clock);
        row.values = values;
        acts.push(Action::Tell(format!("[{}] done: {}", row.step, row.outcome.text())));
        acts.push(Action::Log(end_line(&row)));
        acts.push(Action::Row(row));
        self.used_up(run.todo);
        Phase::Rest { until: frame.clock + REST }
    }

    /// A try that counted: the next try of the step goes next, or the next step.
    fn used_up(&mut self, todo: Todo) {
        if todo.next_try < todo.step.tries {
            self.queue.push_front(Todo {
                next_try: todo.next_try + 1,
                ..todo
            });
        }
    }

    fn row(&self, run: &Running, outcome: Outcome, ended_by: &'static str, end: At) -> Row {
        let step = &run.todo.step;
        Row {
            match_id: self.match_id,
            provenance: self.provenance.clone(),
            step: step.id,
            package: step.package,
            try_no: run.todo.next_try,
            tries: step.tries,
            outcome,
            ended_by,
            start: run.base.started,
            end,
            values: Vec::new(),
        }
    }
}

/// The verdict of a try: its bars against its values.
fn judge(step: &Step, values: &[(&'static str, Option<f64>)]) -> Outcome {
    if step.bars.is_empty() {
        return Outcome::Info;
    }
    let value_of = |name: &str| values.iter().find(|(n, _)| *n == name).and_then(|(_, v)| *v);
    let missed: Vec<String> = step
        .bars
        .iter()
        .filter_map(|bar| match value_of(bar.measure) {
            Some(v) if bar.test.holds(v) => None,
            Some(v) => Some(format!("{} {} got {}", bar.measure, bar.test.describe(), round(v))),
            None => Some(format!("{} {} got none", bar.measure, bar.test.describe())),
        })
        .collect();
    if missed.is_empty() {
        Outcome::Pass
    } else {
        Outcome::Fail(missed.join("; "))
    }
}

fn round(v: f64) -> String {
    format!("{v:.2}").trim_end_matches('0').trim_end_matches('.').to_string()
}

/// The log line of an ended try.
pub fn end_line(row: &Row) -> String {
    format!("CLIENT_TEST step {} END {} (try {}/{}, by {})", row.step, row.outcome.text(), row.try_no, row.tries, row.ended_by)
}

fn holds(cond: &Cond, armed: &Armed, frame: &Frame, owner: &Owner) -> bool {
    match *cond {
        Cond::OwnerNear { at, r_m } => owner.body.alive && owner.body.feet.distance_xz(at.vec()) <= r_m,
        Cond::BotsInPlace => armed.crew.in_place(frame),
        Cond::After(span) => frame.clock - armed.since >= span,
        Cond::Custom(wait) => (wait.holds)(frame, frame.clock - armed.since),
    }
}
