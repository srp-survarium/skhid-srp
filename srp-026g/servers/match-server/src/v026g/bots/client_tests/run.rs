//! The runner: one state machine over the steps of a match's group. It sees [`Frame`]s and answers with [`Action`]s
//! (words for the owner, log lines, result rows) and, for each scripted bot, what to do now ([`Runner::drive`]). It
//! does no I/O, so it is tested with a made-up owner.
//!
//! ```text
//! Waiting --(play on, owner in, a step left)--> Armed --(start conditions)--> Running --(end)--> Rest --> Waiting
//!    |                                            |  \--(arm time out)--> row VOID never_started
//!    \--(no step left)--> Finished                \--(no owner)--> back to the queue
//! ```

use std::collections::VecDeque;

use mapdata::Vec3;

use super::frame::{Body, Frame};
use super::step::{Cond, End, Gait, Group, Measure, Scratch, Script, Step, Who};
use crate::v026g::bots::view::Place;

/// The owner reads the instruction this long before the start can happen, ms.
const SETTLE_MS: u32 = 1_500;
/// A chat line fades after about 7 s: the instruction is told again this often while the step waits for its start, ms.
const RETELL_MS: u32 = 6_000;
/// After a step ends the next one waits this long, ms (the owner reads the result).
const REST_MS: u32 = 4_000;
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
    pub step: &'static str,
    pub package: &'static str,
    pub try_no: u8,
    pub tries: u8,
    pub outcome: Outcome,
    pub ended_by: &'static str,
    pub start_ms: u32,
    pub end_ms: u32,
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

/// What a scripted bot does now: the body's wish, as the bots' intent is made from it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Drive {
    /// Stands, does not fire.
    Idle,
    /// Stands where it is; fires on sight.
    Stand { crouch: bool },
    /// Goes to `at` and stays within `within_m`; crouches there if told.
    Go { at: Vec3, within_m: f32, gait: Gait, crouch: bool },
    /// Looks at `look` with the throw key held or let go.
    Throw { look: Vec3, held: bool },
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum ScriptState {
    Fixed,
    Walk { leg: usize },
    Throw { since: Option<u32>, thrown: bool },
}

#[derive(Clone, Debug)]
struct Member {
    place: Place,
    script: Script,
    state: ScriptState,
    drive: Drive,
}

impl Member {
    fn new(place: Place, script: Script) -> Self {
        let state = match script {
            Script::Walk { .. } => ScriptState::Walk { leg: 0 },
            Script::Throw { .. } => ScriptState::Throw { since: None, thrown: false },
            _ => ScriptState::Fixed,
        };
        Self {
            place,
            script,
            state,
            drive: Drive::Stand { crouch: false },
        }
    }

    /// The next state and what the body does, from the bot's own body and the owner.
    fn advance(&mut self, me: Option<&Body>, owner: &Body, clock: u32) {
        let feet = me.map(|b| b.feet);
        self.drive = match (self.script, self.state) {
            (Script::Idle, _) => Drive::Idle,
            (Script::Stand { crouch }, _) => Drive::Stand { crouch },
            (Script::Hold { at, within_m, crouch }, _) => Drive::Go { at: at.vec(), within_m, gait: Gait::Run, crouch },
            (Script::Follow { within_m, gait }, _) => Drive::Go {
                at: owner.feet,
                within_m,
                gait,
                crouch: false,
            },
            (Script::Walk { lane, gait, looped }, ScriptState::Walk { leg }) if !lane.is_empty() => {
                let reached = feet.is_some_and(|f| f.distance_xz(lane[leg].vec()) <= LEG_REACHED_M);
                let leg = match (reached, leg + 1 < lane.len(), looped) {
                    (false, ..) => leg,
                    (true, true, _) => leg + 1,
                    (true, false, true) => 0,
                    (true, false, false) => leg,
                };
                self.state = ScriptState::Walk { leg };
                Drive::Go {
                    at: lane[leg].vec(),
                    within_m: LEG_REACHED_M,
                    gait,
                    crouch: false,
                }
            }
            (Script::Walk { .. }, _) => Drive::Stand { crouch: false },
            (Script::Throw { at, wind_ms }, ScriptState::Throw { since, thrown }) => {
                let from = feet.unwrap_or(owner.feet);
                // A lob at about 45 degrees: the view rises with the distance.
                let look = at.vec() + Vec3::new(0.0, from.distance_xz(at.vec()), 0.0);
                let since = since.unwrap_or(clock);
                let thrown = thrown || clock.saturating_sub(since) >= wind_ms;
                self.state = ScriptState::Throw { since: Some(since), thrown };
                if thrown {
                    Drive::Throw { look, held: false }
                } else {
                    Drive::Throw { look, held: true }
                }
            }
            (Script::Throw { .. }, _) => Drive::Stand { crouch: false },
        };
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
            let mut places: Vec<Place> = frame.side(side).map(|b| b.place).collect();
            places.sort_unstable();
            let chosen: Vec<Place> = match nth {
                Some(n) => places.get(n).copied().into_iter().collect(),
                None => places,
            };
            if chosen.is_empty() || nth.is_some_and(|n| n >= frame.side(side).count()) {
                return Err(format!("{} bot{}", side.name(), nth.map_or(String::new(), |n| format!(" {n}"))));
            }
            for place in chosen {
                // A later setup for the same bot replaces the earlier.
                members.retain(|m| m.place != place);
                members.push(Member::new(place, setup.script));
            }
        }
        Ok(Self(members))
    }

    fn advance(&mut self, frame: &Frame, owner: &Body) {
        for member in &mut self.0 {
            let me = frame.bots.iter().find(|b| b.place == member.place);
            member.advance(me, owner, frame.clock);
        }
    }

    fn in_place(&self, frame: &Frame) -> bool {
        self.0.iter().all(|member| match member.script {
            Script::Hold { at, within_m, .. } => frame
                .bots
                .iter()
                .find(|b| b.place == member.place)
                .is_some_and(|b| b.alive && b.feet.distance_xz(at.vec()) <= within_m + 1.0),
            _ => true,
        })
    }

    fn drive(&self, place: Place) -> Option<Drive> {
        self.0.iter().find(|m| m.place == place).map(|m| m.drive)
    }
}

/// What one measure keeps during a try.
#[derive(Clone, Debug)]
enum Acc {
    /// Read from the last frame at the end.
    Derived,
    First(Option<u32>),
    Low(Option<f64>),
    Travel { total: f64, last: Option<Vec3> },
    Kills { total: u32, seen: Vec<(Place, u32)> },
    Count(u32),
    Seconds { ms: u32, last: u32 },
    Scratch(Scratch),
}

#[derive(Clone, Copy, Debug)]
struct Base {
    started: u32,
    kills: u32,
    deaths: u32,
}

#[derive(Clone, Debug)]
struct Meter {
    measure: Measure,
    acc: Acc,
}

impl Meter {
    fn new(measure: Measure, base: Base) -> Self {
        let acc = match measure {
            Measure::ElapsedMs | Measure::OwnerKills | Measure::OwnerDeaths => Acc::Derived,
            Measure::FirstKillMs | Measure::DiedAtMs => Acc::First(None),
            Measure::MinHealth | Measure::NearestBotM(_) => Acc::Low(None),
            Measure::TravelM => Acc::Travel { total: 0.0, last: None },
            Measure::SideKills(_) => Acc::Kills { total: 0, seen: Vec::new() },
            Measure::Notes { .. } => Acc::Count(0),
            Measure::SecondsInArea { .. } => Acc::Seconds { ms: 0, last: base.started },
            Measure::Custom { .. } => Acc::Scratch(Scratch {
                numbers: Vec::new(),
                started: base.started,
                base: (base.kills, base.deaths),
            }),
        };
        Self { measure, acc }
    }

    fn sample(&mut self, frame: &Frame, owner: &Body, base: Base) {
        let since = frame.clock.saturating_sub(base.started);
        match (&self.measure, &mut self.acc) {
            (Measure::FirstKillMs, Acc::First(first)) => {
                if first.is_none() && owner.kills > base.kills {
                    *first = Some(since);
                }
            }
            (Measure::DiedAtMs, Acc::First(first)) => {
                if first.is_none() && (owner.deaths > base.deaths || !owner.alive) {
                    *first = Some(since);
                }
            }
            (Measure::MinHealth, Acc::Low(low)) => {
                if owner.alive {
                    let h = f64::from(owner.health);
                    *low = Some(low.map_or(h, |l| l.min(h)));
                }
            }
            (Measure::NearestBotM(side), Acc::Low(low)) => {
                if owner.alive {
                    for bot in frame.side(*side).filter(|b| b.alive) {
                        let d = f64::from(bot.feet.distance_xz(owner.feet));
                        *low = Some(low.map_or(d, |l| l.min(d)));
                    }
                }
            }
            (Measure::TravelM, Acc::Travel { total, last }) => {
                if owner.alive {
                    if let Some(prev) = last {
                        let d = prev.distance_xz(owner.feet);
                        if d < RESPAWN_JUMP_M {
                            *total += f64::from(d);
                        }
                    }
                    *last = Some(owner.feet);
                } else {
                    *last = None;
                }
            }
            (Measure::SideKills(side), Acc::Kills { total, seen }) => {
                for bot in frame.side(*side) {
                    match seen.iter_mut().find(|(p, _)| *p == bot.place) {
                        Some((_, kills)) => {
                            *total += bot.kills.saturating_sub(*kills);
                            *kills = bot.kills;
                        }
                        None => seen.push((bot.place, bot.kills)),
                    }
                }
            }
            (Measure::Notes { contains, .. }, Acc::Count(count)) => {
                *count += frame.heard.iter().filter(|h| h.text.contains(contains)).count() as u32;
            }
            (Measure::SecondsInArea { side, at, r_m, .. }, Acc::Seconds { ms, last }) => {
                let inside = frame.side(*side).any(|b| b.alive && b.feet.distance_xz(at.vec()) <= *r_m);
                if inside {
                    *ms += frame.clock.saturating_sub(*last);
                }
                *last = frame.clock;
            }
            (Measure::Custom { sample, .. }, Acc::Scratch(scratch)) => sample(frame, scratch),
            _ => {}
        }
    }

    fn value(&self, base: Base, end: &Frame, owner: &Body) -> Option<f64> {
        match (&self.measure, &self.acc) {
            (Measure::ElapsedMs, _) => Some(f64::from(end.clock.saturating_sub(base.started))),
            (Measure::OwnerKills, _) => Some(f64::from(owner.kills.saturating_sub(base.kills))),
            (Measure::OwnerDeaths, _) => Some(f64::from(owner.deaths.saturating_sub(base.deaths))),
            (_, Acc::First(first)) => first.map(f64::from),
            (_, Acc::Low(low)) => *low,
            (_, Acc::Travel { total, .. }) => Some(*total),
            (_, Acc::Kills { total, .. }) => Some(f64::from(*total)),
            (_, Acc::Count(count)) => Some(f64::from(*count)),
            (_, Acc::Seconds { ms, .. }) => Some(f64::from(*ms) / 1_000.0),
            (Measure::Custom { value, .. }, Acc::Scratch(scratch)) => value(scratch),
            _ => None,
        }
    }
}

/// A try under way.
#[derive(Clone, Debug)]
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
    since: u32,
    /// When the instruction was last told.
    told: u32,
}

#[derive(Debug, Default)]
enum Phase {
    #[default]
    Waiting,
    Armed(Armed),
    Running(Running),
    /// The result is on the owner's screen; the next step waits.
    Rest {
        until: u32,
    },
    /// No step is left for this group.
    Finished,
}

/// The runner of one match.
#[derive(Debug)]
pub struct Runner {
    match_id: u32,
    played: Group,
    queue: VecDeque<Todo>,
    /// The group after this one, to tell the owner (none: the plan ends here).
    next_group: Option<Group>,
    phase: Phase,
    /// Tries in a row voided by the owner leaving, per step id.
    gone: Vec<(&'static str, u8)>,
}

impl Runner {
    pub fn new(match_id: u32, played: Group, todo: Vec<Todo>, next_group: Option<Group>) -> Self {
        Self {
            match_id,
            played,
            queue: todo.into(),
            next_group,
            phase: Phase::Waiting,
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
            _ => None,
        }
    }

    /// The match is over: a try under way is void (and not used up).
    pub fn finish(&mut self) -> Vec<Action> {
        let mut acts = Vec::new();
        if let Phase::Running(running) = std::mem::take(&mut self.phase) {
            let row = self.row(&running, Outcome::Void(VoidWhy::MatchEnded), "match_end", running.base.started);
            acts.push(Action::Log(end_line(&row)));
            acts.push(Action::Row(row));
        }
        self.phase = Phase::Finished;
        acts
    }

    pub fn tick(&mut self, frame: &Frame) -> Vec<Action> {
        let mut acts = Vec::new();
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
        let Some(owner) = frame.owner.as_ref().filter(|_| frame.playing) else {
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
                crew.advance(frame, owner);
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
                    step: todo.step.id,
                    package: todo.step.package,
                    try_no: todo.next_try,
                    tries: todo.step.tries,
                    outcome: Outcome::Void(VoidWhy::MissingBots(lacking.replace(' ', "_"))),
                    ended_by: "setup",
                    start_ms: frame.clock,
                    end_ms: frame.clock,
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
        armed.crew.advance(frame, owner);
        let waited = frame.clock.saturating_sub(armed.since);
        if waited >= armed.todo.step.arm_ms {
            let step = &armed.todo.step;
            let row = Row {
                match_id: self.match_id,
                step: step.id,
                package: step.package,
                try_no: armed.todo.next_try,
                tries: step.tries,
                outcome: Outcome::Void(VoidWhy::NeverStarted),
                ended_by: "arm_time",
                start_ms: frame.clock,
                end_ms: frame.clock,
                values: Vec::new(),
            };
            acts.push(Action::Tell(format!("[{}] not started in time; on to the next.", step.id)));
            acts.push(Action::Log(end_line(&row)));
            acts.push(Action::Row(row));
            self.used_up(armed.todo);
            return Phase::Rest { until: frame.clock + REST_MS };
        }
        let begins = waited >= SETTLE_MS && armed.todo.step.start.iter().all(|cond| holds(cond, &armed, frame, owner));
        if !begins {
            if frame.clock.saturating_sub(armed.told) >= RETELL_MS {
                armed.told = frame.clock;
                acts.push(Action::Tell(format!("[{}] {}", armed.todo.step.id, armed.todo.step.instruction)));
            }
            return Phase::Armed(armed);
        }
        let base = Base {
            started: frame.clock,
            kills: owner.kills,
            deaths: owner.deaths,
        };
        let meters = armed.todo.step.measures.iter().map(|m| Meter::new(*m, base)).collect();
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
        run.crew.advance(frame, owner);
        for meter in &mut run.meters {
            meter.sample(frame, owner, run.base);
        }
        let step = &run.todo.step;
        let scripted_died = |who: Who| match who {
            Who::All(side) => frame.side(side).any(|b| !b.alive && run.crew.0.iter().any(|m| m.place == b.place)),
            Who::Nth(side, n) => {
                let mut places: Vec<Place> = frame.side(side).map(|b| b.place).collect();
                places.sort_unstable();
                places
                    .get(usize::from(n))
                    .is_some_and(|p| frame.bots.iter().any(|b| b.place == *p && !b.alive))
            }
        };
        let since = frame.clock.saturating_sub(run.base.started);
        let ended_by = step
            .end
            .iter()
            .find_map(|end| match *end {
                End::OwnerDied => (owner.deaths > run.base.deaths || !owner.alive).then_some("owner_died"),
                End::OwnerKills(n) => (owner.kills.saturating_sub(run.base.kills) >= n).then_some("owner_kills"),
                End::OwnerLeft { at, r_m } => (owner.alive && owner.feet.distance_xz(at.vec()) > r_m).then_some("owner_left"),
                End::ScriptedDied(who) => scripted_died(who).then_some("scripted_died"),
            })
            .or((since >= step.budget_ms).then_some("budget"));
        let Some(ended_by) = ended_by else {
            return Phase::Running(run);
        };
        let outcome = self.judge(&run, frame, owner);
        let row = self.row_at(&run, frame, owner, outcome, ended_by);
        acts.push(Action::Tell(format!("[{}] done: {}", row.step, row.outcome.text())));
        acts.push(Action::Log(end_line(&row)));
        acts.push(Action::Row(row));
        self.used_up(run.todo);
        Phase::Rest { until: frame.clock + REST_MS }
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

    fn judge(&self, run: &Running, frame: &Frame, owner: &Body) -> Outcome {
        let step = &run.todo.step;
        if step.bars.is_empty() {
            return Outcome::Info;
        }
        let value_of = |name: &str| {
            run.meters
                .iter()
                .find(|m| m.measure.name() == name)
                .and_then(|m| m.value(run.base, frame, owner))
        };
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

    fn row_at(&self, run: &Running, frame: &Frame, owner: &Body, outcome: Outcome, ended_by: &'static str) -> Row {
        let mut row = self.row(run, outcome, ended_by, frame.clock);
        row.values = run.meters.iter().map(|m| (m.measure.name(), m.value(run.base, frame, owner))).collect();
        row
    }

    fn row(&self, run: &Running, outcome: Outcome, ended_by: &'static str, end_ms: u32) -> Row {
        let step = &run.todo.step;
        Row {
            match_id: self.match_id,
            step: step.id,
            package: step.package,
            try_no: run.todo.next_try,
            tries: step.tries,
            outcome,
            ended_by,
            start_ms: run.base.started,
            end_ms,
            values: Vec::new(),
        }
    }
}

fn round(v: f64) -> String {
    format!("{:.2}", v).trim_end_matches('0').trim_end_matches('.').to_string()
}

/// The log line of an ended try.
pub fn end_line(row: &Row) -> String {
    format!("CLIENT_TEST step {} END {} (try {}/{}, by {})", row.step, row.outcome.text(), row.try_no, row.tries, row.ended_by)
}

fn holds(cond: &Cond, armed: &Armed, frame: &Frame, owner: &Body) -> bool {
    match *cond {
        Cond::OwnerNear { at, r_m } => owner.alive && owner.feet.distance_xz(at.vec()) <= r_m,
        Cond::BotsInPlace => armed.crew.in_place(frame),
        Cond::After { ms } => frame.clock.saturating_sub(armed.since) >= ms,
    }
}
