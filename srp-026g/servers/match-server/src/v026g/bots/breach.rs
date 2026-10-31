//! The Breach job on a held spot (K2, `design-p1c.md` §1, items 13, 14, 16, 18): when a held spot's Flank failed or found no
//! flank spot, two crossers gather out of his lanes and out of his sight, wait for each other, go on one tick to two cover
//! points picked before the go that see his spot from two bearings out of his lanes, and the second keeps the spot in view to
//! trade the first; a pinner, when one is free, holds him from Crossfire's spot meanwhile.
//!
//! The live humans' numbers it takes (`fx-gear-breach-human-note.md`, 478 ended campers in 402 recordings of 12+ players): two
//! comers is the common group push; half go within 5 s of each other; 84 % from two or more 45-degree sectors, the nearest pair a
//! median 12 m apart (P25 7 m); the camper dies a median 8 s after his last kill, and a pusher he kills is traded a median 4 s later.
//!
//! One state a phase ([`Phase`]) and one transition function ([`Breach::step`]) from what the team sees this tick ([`Facts`]);
//! the team's jobs by [`tick`]; what a try cost is the team's spot memory's (`concede.rs`, [`Failed`]).

use mapdata::Vec3;

use super::concede::{Abort, Failed};
use super::rng::Rng;
use super::tactics::{SpotQuery, Tactics};
use super::threat::{HeldLine, Holder, Job, Mate, Role};
use super::view::Place;

/// The crossers of a Breach: two (humans' group push; with the pinner, the three bots a spot at most, §4).
pub const CROSSERS: usize = 2;

/// A team runs this many Breaches at once at most (three bots each).
pub const BREACHES_MAX: usize = 1;

/// A spot whose Flank failed may get a Breach within this, ms; then it is no longer due (the try is stale).
pub const DUE_MS: u32 = 20_000;

/// A crosser within this of its gather point is there, m, and within this of its cover point.
pub const AT_GATHER_M: f32 = 2.0;
pub const AT_COVER_M: f32 = 1.5;

/// A Breach's numbers, parsed once from `ThreatTuning::breach` (`tuning::Breach::rules`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BreachRules {
    pub gather_ms: u32,
    pub go_ms: u32,
    pub hold_ms: u32,
    pub trade_ms: u32,
    /// The cover points' distance apart, m: at least, at most.
    pub apart_m: (f32, f32),
    pub bearing_min_deg: f32,
    /// The cover points' distance from his spot, and a gather point's from its cover point, m.
    pub cover_m: (f32, f32),
    pub gather_m: (f32, f32),
}

/// A crosser: its bot, where it gathers (out of his lanes and his sight) and the cover point it goes to (out of his lanes, his
/// spot in sight), both picked when the job opened.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Crosser {
    pub holder: Holder,
    pub gather: Vec3,
    pub cover: Vec3,
}

/// How a Breach ended well, or at least not as a failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Done {
    /// Its shooter died: by one of the job's bots, or by someone else.
    ShooterDied { by_job: bool },
    /// The spot is no longer held for another reason (faded, cleared, unsensed): nothing to breach.
    Unheld,
}

/// A Breach's phase: what each phase needs and nothing else.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Phase {
    /// The crossers walk to their gather points and wait there, until `until`.
    Gather { until: u32 },
    /// The go tick was `at`: the crossers walk to their cover points.
    Go { at: u32 },
    /// At the cover points since `since`; the first crosser death and when, if any.
    Hold { since: u32, first_death: Option<u32> },
    Done { at: u32, done: Done },
    Failed { at: u32, why: Failed },
}

impl Phase {
    pub fn ended(&self) -> bool {
        matches!(self, Phase::Done { .. } | Phase::Failed { .. })
    }

    pub fn name(&self) -> &'static str {
        match self {
            Phase::Gather { .. } => "gather",
            Phase::Go { .. } => "go",
            Phase::Hold { .. } => "hold",
            Phase::Done { .. } => "done",
            Phase::Failed { .. } => "failed",
        }
    }
}

/// A crosser's bot this tick, as the team sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CrosserNow {
    /// Dead: its role is lost, a death.
    Down,
    /// Alive, but no longer free (a carrier, dry) or it did not take the play: its role is lost, no death.
    Gone,
    /// On its way, or at neither point.
    Moving,
    AtGather,
    AtCover,
}

impl CrosserNow {
    fn lost(self) -> bool {
        matches!(self, CrosserNow::Down | CrosserNow::Gone)
    }
}

/// The pinner's bot this tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PinnerNow {
    Down,
    Gone,
    Up,
}

/// The spot's shooter as the team sees it this tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shooter {
    /// The spot is held.
    Holds,
    /// The kill feed shows him dead since the job opened; `by_job` when the killer is one of its bots.
    Died { by_job: bool },
    /// The spot is not held any more for another reason.
    Unheld,
}

/// What [`Breach::step`] reads: the team's view of the job this tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Facts {
    pub crossers: [CrosserNow; CROSSERS],
    /// The pinner, if the job has one.
    pub pinner: Option<PinnerNow>,
    pub shooter: Shooter,
    /// The team's objective state is worse than when the job opened (§16).
    pub objective_worse: bool,
}

/// One change, for the notes and the counts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Step {
    /// Both crossers gathered after `waited_ms`: the go.
    Went { waited_ms: u32 },
    /// The crossers came to their cover points (or the go's time ran) after `took_ms`.
    Holding { took_ms: u32 },
    /// A crosser went down at `at` (the first).
    FirstDeath { at: u32 },
    /// The pinner was lost (`down`: dead); the job goes on without it.
    PinnerLost { down: bool },
    Ended(Phase),
}

/// The Breach job on spot `spot` (its shooter's place `shooter`).
#[derive(Clone, Debug, PartialEq)]
pub struct Breach {
    pub spot: u32,
    pub shooter: Place,
    /// Where his spot was when the job opened.
    pub at: Vec3,
    /// Its lanes cover an objective area: objective work, outside the team's cap (§4, §13).
    pub objective: bool,
    pub since: u32,
    /// The pinner, while it has one.
    pub pinner: Option<Holder>,
    pub crossers: [Crosser; CROSSERS],
    /// The team's points (batteries home) and the enemy's when it opened, in the battery modes (§16).
    pub points_at_open: Option<[u32; 2]>,
    pub phase: Phase,
}

impl Breach {
    /// A new job: the crossers start for their gather points.
    #[allow(clippy::too_many_arguments)]
    pub fn open(
        line: &HeldLine,
        objective: bool,
        pinner: Option<Holder>,
        crossers: [Crosser; CROSSERS],
        points_at_open: Option<[u32; 2]>,
        now: u32,
        rules: &BreachRules,
    ) -> Self {
        Self {
            spot: line.id,
            shooter: line.shooter,
            at: line.at,
            objective,
            since: now,
            pinner,
            crossers,
            points_at_open,
            phase: Phase::Gather { until: now + rules.gather_ms },
        }
    }

    /// The job's role of `place`, if it has one.
    pub fn role_of(&self, place: Place) -> Option<BreachRole> {
        if self.pinner.is_some_and(|p| p.place == place) {
            return Some(BreachRole::Pinner);
        }
        self.crossers.iter().position(|c| c.holder.place == place).map(BreachRole::Crosser)
    }

    /// The bots it holds.
    pub fn places(&self) -> impl Iterator<Item = Place> + '_ {
        self.pinner.iter().map(|p| p.place).chain(self.crossers.iter().map(|c| c.holder.place))
    }

    /// Where its try came from: the middle of its cover points (the spot memory's bearing, `concede::bearing`).
    pub fn from(&self) -> Vec3 {
        let [a, b] = [self.crossers[0].cover, self.crossers[1].cover];
        Vec3::new((a.x + b.x) * 0.5, (a.y + b.y) * 0.5, (a.z + b.z) * 0.5)
    }

    /// The transition: the phase from what the team sees at `now`, and what changed. An ended job stays ended.
    pub fn step(&mut self, facts: &Facts, now: u32, rules: &BreachRules) -> Vec<Step> {
        let mut steps = Vec::new();
        if !self.phase.ended()
            && let Some(p) = facts.pinner.filter(|p| *p != PinnerNow::Up)
            && self.pinner.is_some()
        {
            self.pinner = None;
            steps.push(Step::PinnerLost { down: p == PinnerNow::Down });
        }
        let next = self.next(facts, now, rules, &mut steps);
        if next != self.phase {
            if next.ended() {
                steps.push(Step::Ended(next));
            }
            self.phase = next;
        }
        steps
    }

    fn next(&self, facts: &Facts, now: u32, rules: &BreachRules, steps: &mut Vec<Step>) -> Phase {
        if self.phase.ended() {
            return self.phase;
        }
        match facts.shooter {
            Shooter::Died { by_job } => return Phase::Done { at: now, done: Done::ShooterDied { by_job } },
            Shooter::Unheld => return Phase::Done { at: now, done: Done::Unheld },
            Shooter::Holds => {}
        }
        if facts.objective_worse {
            return Phase::Failed { at: now, why: Failed::Aborted(Abort::ObjectiveWorse) };
        }
        let down = facts.crossers.iter().filter(|c| **c == CrosserNow::Down).count();
        let lost = facts.crossers.iter().filter(|c| c.lost()).count();
        match self.phase {
            Phase::Gather { until } => {
                if down > 0 {
                    Phase::Failed { at: now, why: Failed::HolderDown { role: Role::Crosser } }
                } else if lost > 0 {
                    Phase::Failed { at: now, why: Failed::Aborted(Abort::NoCrossers) }
                } else if facts.crossers.iter().all(|c| *c == CrosserNow::AtGather) {
                    steps.push(Step::Went { waited_ms: now.saturating_sub(self.since) });
                    Phase::Go { at: now }
                } else if now >= until {
                    Phase::Failed { at: now, why: Failed::GatherTimedOut }
                } else {
                    self.phase
                }
            }
            Phase::Go { at } => {
                let first_death = (down > 0).then_some(now);
                if let Some(at) = first_death {
                    steps.push(Step::FirstDeath { at });
                }
                if lost == CROSSERS {
                    Phase::Failed { at: now, why: Self::all_lost(down) }
                } else if down > 0 || facts.crossers.iter().all(|c| *c == CrosserNow::AtCover || c.lost()) || now >= at + rules.go_ms {
                    steps.push(Step::Holding { took_ms: now.saturating_sub(at) });
                    Phase::Hold { since: now, first_death }
                } else {
                    self.phase
                }
            }
            Phase::Hold { since, first_death } => {
                let first_death = first_death.or_else(|| {
                    (down > 0).then(|| {
                        steps.push(Step::FirstDeath { at: now });
                        now
                    })
                });
                if lost == CROSSERS {
                    Phase::Failed { at: now, why: Self::all_lost(down) }
                } else if first_death.is_some_and(|d| now >= d + rules.trade_ms) {
                    Phase::Failed { at: now, why: Failed::Untraded }
                } else if now >= since + rules.hold_ms {
                    Phase::Failed { at: now, why: Failed::Time }
                } else {
                    Phase::Hold { since, first_death }
                }
            }
            ended => ended,
        }
    }

    /// Both crossers lost after the go: a death among them is a holder down, else no crossers are left.
    fn all_lost(down: usize) -> Failed {
        if down > 0 { Failed::HolderDown { role: Role::Crosser } } else { Failed::Aborted(Abort::NoCrossers) }
    }
}

/// A role of a Breach.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BreachRole {
    /// Holds him from Crossfire's spot, out of his lanes.
    Pinner,
    /// Crosser 0 or 1.
    Crosser(usize),
}

impl BreachRole {
    /// The team's role kind (the spot memory's and the mates' `playing`).
    pub fn role(self) -> Role {
        match self {
            BreachRole::Pinner => Role::Pinner,
            BreachRole::Crosser(_) => Role::Crosser,
        }
    }
}

/// This bot's Breach and its role among the team's `breaches`.
pub fn role_in(breaches: &[Breach], place: Place) -> Option<(&Breach, BreachRole)> {
    breaches.iter().find_map(|b| b.role_of(place).map(|role| (b, role)))
}

/// A crosser's two points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plan {
    pub gather: Vec3,
    pub cover: Vec3,
}

/// Why no Breach could be planned on a spot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoPlan {
    /// No point sees his spot from the cover band out of every held lane.
    NoCover,
    /// Cover points, but no two the rules' distance and bearings apart.
    NoPair,
    /// A pair, but one of them has no gather point out of his sight and out of the lanes.
    NoGather,
}

/// The cover and gather points of a Breach on `line` for crossers standing at `feet` (§1, item 18): cover points in the
/// cover band round his spot that see it (crouched eye to his chest), out of every held line `lanes` (his own included), fine
/// goals out of the spawn areas; two of them [`BreachRules::apart_m`] apart and [`BreachRules::bearing_min_deg`] apart in
/// bearing from his spot, the pair with the shortest walk for the crossers; a gather point for each in the gather band round
/// its cover point, out of his sight (no bullet line from his spot) and out of the lanes, the nearest the crosser.
#[allow(clippy::too_many_arguments)]
pub fn plan(
    tactics: &Tactics,
    line: &HeldLine,
    lanes: &[HeldLine],
    feet: [Vec3; CROSSERS],
    side: Option<i32>,
    now: u32,
    rules: &BreachRules,
    rng: &mut Rng,
) -> Result<[Plan; CROSSERS], NoPlan> {
    let out_of_lanes = |p: Vec3| !lanes.iter().any(|l| l.covers(p));
    let fine = |p: Vec3| tactics.fine_goal(p, now, side) && !tactics.in_spawn_area(p, None);
    let cover_query = SpotQuery {
        sight_to: Some(line.at),
        side,
        samples: 64,
        ..SpotQuery::ring(line.at, rules.cover_m.0, rules.cover_m.1)
    };
    let covers: Vec<Vec3> = tactics
        .spots(&cover_query, now, 16, rng)
        .into_iter()
        .map(|s| s.at)
        .filter(|&p| out_of_lanes(p) && fine(p))
        .collect();
    if covers.is_empty() {
        return Err(NoPlan::NoCover);
    }
    // The pairs the rules allow, by the crossers' walk (each to the nearer of the two in the order that walks less).
    let bearing = |p: Vec3| (p.z - line.at.z).atan2(p.x - line.at.x).to_degrees();
    let apart_deg = |a: f32, b: f32| {
        let d = (a - b).abs() % 360.0;
        if d > 180.0 { 360.0 - d } else { d }
    };
    let mut pairs: Vec<(f32, [Vec3; CROSSERS])> = Vec::new();
    for (i, &a) in covers.iter().enumerate() {
        for &b in &covers[i + 1..] {
            let d = a.distance_xz(b);
            if d < rules.apart_m.0 || d > rules.apart_m.1 || apart_deg(bearing(a), bearing(b)) < rules.bearing_min_deg {
                continue;
            }
            let straight = feet[0].distance_xz(a) + feet[1].distance_xz(b);
            let crossed = feet[0].distance_xz(b) + feet[1].distance_xz(a);
            pairs.push(if straight <= crossed { (straight, [a, b]) } else { (crossed, [b, a]) });
        }
    }
    pairs.sort_by(|x, y| x.0.total_cmp(&y.0));
    if pairs.is_empty() {
        return Err(NoPlan::NoPair);
    }
    // The best pair whose cover points both have a gather point.
    for (_, cover) in pairs.into_iter().take(4) {
        let mut plans = [Plan { gather: cover[0], cover: cover[0] }; CROSSERS];
        let mut all = true;
        for k in 0..CROSSERS {
            let query = SpotQuery {
                unseen_by: Some(line.at),
                side,
                samples: 32,
                ..SpotQuery::ring(cover[k], rules.gather_m.0, rules.gather_m.1)
            };
            let gather = tactics
                .spots(&query, now, 8, rng)
                .into_iter()
                .map(|s| s.at)
                .filter(|&p| out_of_lanes(p) && fine(p) && p.distance_xz(line.at) > cover[k].distance_xz(line.at))
                .min_by(|a, b| a.distance_xz(feet[k]).total_cmp(&b.distance_xz(feet[k])));
            match gather {
                Some(gather) => plans[k] = Plan { gather, cover: cover[k] },
                None => {
                    all = false;
                    break;
                }
            }
        }
        if all {
            return Ok(plans);
        }
    }
    Err(NoPlan::NoGather)
}

/// A spot whose Flank failed (its flanker down, its time run, nobody to hold it, or a flanker that did not take the play: no
/// flank spot), due a Breach until `until`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Due {
    pub spot: u32,
    pub until: u32,
}

/// An enemy the team killed lately, as the kill feed showed: the victim and the team's killer if one was told.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Kill {
    pub victim: Place,
    pub killer: Option<Place>,
    pub time: u32,
}

/// The team as [`tick`] reads it.
pub struct Team<'a> {
    /// The team's living bots that may take a role (`Mate::free`), with the job play each runs.
    pub mates: &'a [Mate],
    /// Its players, bots and humans (the cap's count, `threat::JOB_SHARE`).
    pub size: usize,
    pub objective_areas: &'a [Vec3],
    /// The Flank jobs (K2): their spots, shooters and bots are not the Breach's.
    pub flank_jobs: &'a [Job],
    /// The enemies it killed lately (the kill feed).
    pub kills: &'a [Kill],
    /// Its points and the enemy's in the battery modes ([`Breach::points_at_open`]).
    pub points: Option<[u32; 2]>,
}

/// What a tick of the team's Breaches did, for the notes and the counts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    /// A Breach opened on `spot`, with a pinner or not.
    Opened { spot: u32, pinner: bool, objective: bool },
    /// A due spot got none: no plan (and why), too few free crossers, the team's cap, or the spot memory refused it.
    NotOpened { spot: u32, why: NotOpened },
    /// A phase change of the Breach on `spot` (his spot `at` when it opened, its try from `from`) from phase `was`, its first
    /// crosser death so far, and its bots.
    Stepped { spot: u32, shooter: Place, at: Vec3, from: Vec3, was: Phase, step: Step, life_ms: u32, first_death: Option<u32>, bots: [Option<Place>; 3] },
}

impl Event {
    /// Its note line.
    pub fn note(&self) -> String {
        match *self {
            Event::Opened { spot, pinner, objective } => format!(
                "breach on threat spot {spot} opened ({}{})",
                if pinner { "two crossers and a pinner" } else { "two crossers, no pinner free" },
                if objective { ", objective work" } else { "" }
            ),
            Event::NotOpened { spot, why } => format!("breach on threat spot {spot} not opened: {why:?}"),
            Event::Stepped { spot, step, life_ms, .. } => {
                let what = match step {
                    Step::Went { waited_ms } => format!("the crossers go after {:.1} s gathering", waited_ms as f32 / 1000.0),
                    Step::Holding { took_ms } => format!("the crossers hold after {:.1} s going", took_ms as f32 / 1000.0),
                    Step::FirstDeath { .. } => "a crosser went down".to_string(),
                    Step::PinnerLost { down } => format!("the pinner was lost ({})", if down { "down" } else { "not free" }),
                    Step::Ended(Phase::Done { done, .. }) => format!("ends: {done:?}"),
                    Step::Ended(Phase::Failed { why, .. }) => format!("fails: {why:?}"),
                    Step::Ended(phase) => format!("ends in {}", phase.name()),
                };
                format!("breach on threat spot {spot}: {what} ({:.1} s in)", life_ms as f32 / 1000.0)
            }
        }
    }
}

/// Why a due spot got no Breach.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotOpened {
    Plan(NoPlan),
    NoCrossers,
    Cap,
    Conceded,
    LikeTheLast,
}

/// What finds a Breach's points on a held spot for crossers at their feet ([`plan`] with the map, or a test's stand-in).
pub type Planner<'a> = dyn FnMut(&HeldLine, [Vec3; CROSSERS]) -> Result<[Plan; CROSSERS], NoPlan> + 'a;

/// Whether the spot memory lets a Breach be tried on a held spot from a point (its cover points' middle).
pub type MayTry<'a> = dyn Fn(&HeldLine, Vec3) -> Result<(), NotOpened> + 'a;

/// Holding a role: alive, free, and its play taken within this of the role, ms (`threat::JOB_START_MS`).
pub const START_MS: u32 = super::threat::JOB_START_MS;

/// The team's Breaches at `now`: each running one stepped by what the team sees (its bots in `team.mates`, the kill feed,
/// the held lines `held`, the objective points), ended ones dropped; then a due spot (`due`, the Flank's failures) that is
/// held, has no job of either kind and whose shooter has none, gets one when two free crossers stand out of every held line
/// within `threat::JOB_NEAR_M`, the team's cap allows its bots (off the objective; a spot covering an objective area is
/// objective work, §13, and goes first), and `may_try` (the spot memory) lets it. `plan` finds its points.
#[allow(clippy::too_many_arguments)]
pub fn tick(
    breaches: &mut Vec<Breach>,
    due: &mut Vec<Due>,
    holds: &[HeldLine],
    lanes: &[HeldLine],
    team: &Team,
    may_try: &MayTry,
    plan: &mut Planner,
    now: u32,
    rules: &BreachRules,
) -> Vec<Event> {
    let mut events = Vec::new();
    let mate = |place: Place| team.mates.iter().find(|m| m.place == place);
    for b in breaches.iter_mut() {
        let spot = b.spot;
        let crosser = |c: &Crosser| match mate(c.holder.place) {
            None => CrosserNow::Down,
            Some(m) if !m.free => CrosserNow::Gone,
            Some(m) if m.playing != Some((Role::Crosser, spot)) && now >= c.holder.since + START_MS => CrosserNow::Gone,
            Some(m) if m.feet.distance_xz(c.cover) <= AT_COVER_M => CrosserNow::AtCover,
            Some(m) if m.feet.distance_xz(c.gather) <= AT_GATHER_M => CrosserNow::AtGather,
            Some(_) => CrosserNow::Moving,
        };
        let crossers = [crosser(&b.crossers[0]), crosser(&b.crossers[1])];
        let pinner = b.pinner.map(|p| match mate(p.place) {
            None => PinnerNow::Down,
            Some(m) if !m.free => PinnerNow::Gone,
            Some(m) if m.playing != Some((Role::Pinner, spot)) && now >= p.since + START_MS => PinnerNow::Gone,
            Some(_) => PinnerNow::Up,
        });
        let shooter = match team.kills.iter().find(|k| k.victim == b.shooter && k.time >= b.since) {
            Some(k) => Shooter::Died { by_job: k.killer.is_some_and(|q| b.places().any(|p| p == q)) },
            None if lanes.iter().any(|l| l.id == spot) => Shooter::Holds,
            None => Shooter::Unheld,
        };
        let objective_worse = match (b.points_at_open, team.points) {
            (Some([ours, theirs]), Some([ours_now, theirs_now])) => theirs_now > theirs || ours_now < ours,
            _ => false,
        };
        let facts = Facts { crossers, pinner, shooter, objective_worse };
        let (shooter, at, from, was, life_ms) = (b.shooter, b.at, b.from(), b.phase, now.saturating_sub(b.since));
        let bots = [Some(b.crossers[0].holder.place), Some(b.crossers[1].holder.place), b.pinner.map(|p| p.place)];
        let mut first_death = match b.phase {
            Phase::Hold { first_death, .. } => first_death,
            _ => None,
        };
        for step in b.step(&facts, now, rules) {
            if let Step::FirstDeath { at } = step {
                first_death = Some(at);
            }
            events.push(Event::Stepped { spot, shooter, at, from, was, step, life_ms, first_death, bots });
        }
    }
    breaches.retain(|b| !b.phase.ended());
    // Due spots: still held and fresh.
    due.retain(|d| now < d.until && holds.iter().any(|l| l.id == d.spot));
    if breaches.len() >= BREACHES_MAX || due.is_empty() {
        return events;
    }
    let share = team.size.div_ceil(super::threat::JOB_SHARE);
    let off_objective_roles: usize = team
        .flank_jobs
        .iter()
        .filter(|j| !j.objective)
        .map(|j| usize::from(j.flanker.is_some()) + usize::from(j.pinner.is_some()))
        .sum::<usize>()
        + breaches.iter().filter(|b| !b.objective).map(|b| b.places().count()).sum::<usize>();
    // Objective spots first (§13), then the oldest due.
    let mut order: Vec<(bool, Due)> = due
        .iter()
        .filter_map(|d| holds.iter().find(|l| l.id == d.spot).map(|l| (team.objective_areas.iter().any(|&a| l.covers(a)), *d)))
        .collect();
    order.sort_by_key(|(objective, d)| (!*objective, d.until));
    let mut spent = Vec::new();
    for (objective, d) in order {
        if breaches.len() >= BREACHES_MAX {
            break;
        }
        let Some(line) = holds.iter().find(|l| l.id == d.spot) else {
            continue;
        };
        // A Flank job on this spot is taken over (its bots are free for the Breach, its pinner first); one on another spot of
        // the same shooter keeps him (one job a shooter).
        if team.flank_jobs.iter().any(|j| j.spot != line.id && j.shooter == line.shooter) || breaches.iter().any(|b| b.shooter == line.shooter) {
            continue;
        }
        let replaced = team.flank_jobs.iter().find(|j| j.spot == line.id);
        let freed = replaced.filter(|j| !j.objective).map_or(0, |j| usize::from(j.flanker.is_some()) + usize::from(j.pinner.is_some()));
        let busy = |place: Place| {
            team.flank_jobs.iter().any(|j| j.spot != line.id && j.role_of(place).is_some()) || breaches.iter().any(|b| b.role_of(place).is_some())
        };
        spent.push(d.spot);
        let mut free: Vec<&Mate> = team
            .mates
            .iter()
            .filter(|m| m.free && !busy(m.place))
            .filter(|m| m.feet.distance_xz(line.at) <= super::threat::JOB_NEAR_M)
            .filter(|m| !lanes.iter().any(|l| l.covers(m.feet)))
            .collect();
        // The Flank's pinner keeps the pin: last in the order, so it is not taken as a crosser.
        let pinning = replaced.and_then(|j| j.pinner).map(|h| h.place);
        free.sort_by(|a, b| (Some(a.place) == pinning).cmp(&(Some(b.place) == pinning)).then(a.feet.distance_xz(line.at).total_cmp(&b.feet.distance_xz(line.at))));
        if free.len() < CROSSERS {
            events.push(Event::NotOpened { spot: d.spot, why: NotOpened::NoCrossers });
            continue;
        }
        let with_pinner = free.len() > CROSSERS;
        let roles = CROSSERS + usize::from(with_pinner);
        let taken = off_objective_roles.saturating_sub(freed);
        if !objective && taken + CROSSERS > share {
            events.push(Event::NotOpened { spot: d.spot, why: NotOpened::Cap });
            continue;
        }
        // The pinner only inside the cap (the crossers first).
        let with_pinner = with_pinner && (objective || taken + roles <= share);
        let feet = [free[0].feet, free[1].feet];
        let plans = match plan(line, feet) {
            Ok(plans) => plans,
            Err(why) => {
                events.push(Event::NotOpened { spot: d.spot, why: NotOpened::Plan(why) });
                continue;
            }
        };
        let crossers = [0, 1].map(|k| Crosser { holder: Holder { place: free[k].place, since: now }, gather: plans[k].gather, cover: plans[k].cover });
        let from = {
            let [a, b] = [plans[0].cover, plans[1].cover];
            Vec3::new((a.x + b.x) * 0.5, (a.y + b.y) * 0.5, (a.z + b.z) * 0.5)
        };
        if let Err(why) = may_try(line, from) {
            events.push(Event::NotOpened { spot: d.spot, why });
            continue;
        }
        // The Flank's pinner if it is free here, else the next nearest.
        let pinner = with_pinner.then(|| {
            let place = pinning.filter(|p| free[CROSSERS..].iter().any(|m| m.place == *p)).unwrap_or(free[CROSSERS].place);
            Holder { place, since: now }
        });
        breaches.push(Breach::open(line, objective, pinner, crossers, team.points, now, rules));
        events.push(Event::Opened { spot: d.spot, pinner: pinner.is_some(), objective });
    }
    due.retain(|d| !spent.contains(&d.spot));
    events
}

/// The bench's counts of the Breach, by team (`tests/tallies/owner/breach.rs`).
#[cfg(test)]
pub mod tally {
    use super::{Event, NoPlan, NotOpened, Step};
    use crate::v026g::bots::concede::{Abort, Failed};

    pub const NAMES: [&str; 26] = [
        "due",
        "opened",
        "opened_with_pinner",
        "opened_objective",
        "no_cover",
        "no_pair",
        "no_gather",
        "no_crossers",
        "cap",
        "conceded",
        "like_the_last",
        "went",
        "gather_wait_ms",
        "crossers_at_go",
        "holding",
        "first_death",
        "traded",
        "trade_ms",
        "shooter_killed_by_job",
        "shooter_died_other",
        "unheld",
        "failed_holder_down",
        "failed_untraded",
        "failed_gather_timeout",
        "failed_aborted",
        "failed_time",
    ];
    pub const PINNER_LOST: usize = 26;
    pub const CROSSER_DOWNS: usize = 27;
    /// Thinks an objective player waited out of a held lane for the team's Breach (§13, §14).
    pub const OBJECTIVE_WAITS: usize = 28;
    /// Frags a Breach's bots let go at a held spot while in the job (A170's held-spot rule: the Breach has no throw of its own).
    pub const FRAGS_AT_SPOT: usize = 29;
    pub const ALL: usize = 30;

    // A match runs on one thread: its counts are the thread's (the topic reads the difference from its start).
    thread_local! {
        static COUNTS: std::cell::Cell<[[u64; ALL]; 2]> = const { std::cell::Cell::new([[0; ALL]; 2]) };
    }

    pub fn add(team: u8, i: usize, n: u64) {
        COUNTS.with(|cell| {
            let mut all = cell.get();
            all[usize::from(team & 1)][i] += n;
            cell.set(all);
        });
    }

    pub fn counted() -> [[u64; ALL]; 2] {
        COUNTS.with(|cell| cell.get())
    }

    thread_local! {
        /// By team: each Breach bot's frags at held spots this life at the last tick.
        static FRAGS: std::cell::RefCell<[Vec<(usize, u32)>; 2]> = const { std::cell::RefCell::new([Vec::new(), Vec::new()]) };
    }

    /// The team's Breach bots and their frags at held spots this life (`GrenadeStats::thrown_at_spots`) at a tick: the rise since
    /// the last tick of each bot that was in a job then too is counted; a new life (fewer than before) counts from 0.
    pub fn frags(team: u8, now: &[(usize, u32)]) {
        let t = usize::from(team & 1);
        let rise: u64 = FRAGS.with(|f| {
            let before = &f.borrow()[t];
            now.iter()
                .filter_map(|&(place, n)| before.iter().find(|(p, _)| *p == place).map(|&(_, b)| if n >= b { n - b } else { n }))
                .map(u64::from)
                .sum()
        });
        FRAGS.with(|f| f.borrow_mut()[t] = now.to_vec());
        if rise > 0 {
            add(team, FRAGS_AT_SPOT, rise);
        }
    }

    fn idx(name: &str) -> usize {
        NAMES.iter().position(|n| *n == name).expect("a breach tally")
    }

    /// One spot made due.
    pub fn due(team: u8) {
        add(team, idx("due"), 1);
    }

    /// An event of [`super::tick`].
    pub fn event(team: u8, event: &Event) {
        let one = |name: &str| add(team, idx(name), 1);
        match *event {
            Event::Opened { pinner, objective, .. } => {
                one("opened");
                if pinner {
                    one("opened_with_pinner");
                }
                if objective {
                    one("opened_objective");
                }
            }
            Event::NotOpened { why, .. } => one(match why {
                NotOpened::Plan(NoPlan::NoCover) => "no_cover",
                NotOpened::Plan(NoPlan::NoPair) => "no_pair",
                NotOpened::Plan(NoPlan::NoGather) => "no_gather",
                NotOpened::NoCrossers => "no_crossers",
                NotOpened::Cap => "cap",
                NotOpened::Conceded => "conceded",
                NotOpened::LikeTheLast => "like_the_last",
            }),
            Event::Stepped { step, first_death, .. } => match step {
                Step::Went { waited_ms } => {
                    one("went");
                    add(team, idx("gather_wait_ms"), u64::from(waited_ms));
                    add(team, idx("crossers_at_go"), super::CROSSERS as u64);
                }
                Step::Holding { .. } => one("holding"),
                Step::FirstDeath { .. } => {
                    one("first_death");
                    add(team, CROSSER_DOWNS, 1);
                }
                Step::PinnerLost { .. } => add(team, PINNER_LOST, 1),
                Step::Ended(phase) => match phase {
                    super::Phase::Done { at, done: super::Done::ShooterDied { by_job } } => {
                        one(if by_job { "shooter_killed_by_job" } else { "shooter_died_other" });
                        if let Some(d) = first_death {
                            one("traded");
                            add(team, idx("trade_ms"), u64::from(at.saturating_sub(d)));
                        }
                    }
                    super::Phase::Done { .. } => one("unheld"),
                    super::Phase::Failed { why, .. } => one(match why {
                        Failed::HolderDown { .. } => "failed_holder_down",
                        Failed::Untraded => "failed_untraded",
                        Failed::GatherTimedOut => "failed_gather_timeout",
                        Failed::Aborted(Abort::ObjectiveWorse | Abort::NoCrossers) => "failed_aborted",
                        Failed::Time => "failed_time",
                    }),
                    _ => {}
                },
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RULES: BreachRules = BreachRules {
        gather_ms: 8_000,
        go_ms: 6_000,
        hold_ms: 10_000,
        trade_ms: 5_000,
        apart_m: (6.0, 12.0),
        bearing_min_deg: 30.0,
        cover_m: (8.0, 22.0),
        gather_m: (5.0, 15.0),
    };

    fn line() -> HeldLine {
        HeldLine::for_test(4, 9, Vec3::new(0.0, 0.0, 30.0), &[Vec3::new(0.0, 0.0, 60.0)], Some(0))
    }

    fn job(now: u32) -> Breach {
        let h = |place| Holder { place, since: now };
        let c = |place, x| Crosser { holder: h(place), gather: Vec3::new(x, 0.0, 0.0), cover: Vec3::new(x, 0.0, 10.0) };
        Breach::open(&line(), false, Some(h(1)), [c(2, 0.0), c(3, 8.0)], None, now, &RULES)
    }

    fn facts(a: CrosserNow, b: CrosserNow) -> Facts {
        Facts { crossers: [a, b], pinner: Some(PinnerNow::Up), shooter: Shooter::Holds, objective_worse: false }
    }

    fn failed(b: &Breach) -> Option<Failed> {
        match b.phase {
            Phase::Failed { why, .. } => Some(why),
            _ => None,
        }
    }

    use CrosserNow::*;

    /// [Breach] Gather -> Go when both are at their gather points; Go -> Hold at the cover points; the camper's death by the job
    /// ends it Done; the counts' steps on the way.
    #[test]
    fn a_breach_gathers_goes_on_one_tick_holds_and_ends_with_the_camper() {
        let mut b = job(1_000);
        assert!(b.step(&facts(AtGather, Moving), 2_000, &RULES).is_empty());
        assert_eq!(b.phase, Phase::Gather { until: 9_000 });
        assert_eq!(b.step(&facts(AtGather, AtGather), 4_000, &RULES), vec![Step::Went { waited_ms: 3_000 }]);
        assert_eq!(b.phase, Phase::Go { at: 4_000 });
        assert_eq!(b.step(&facts(AtCover, AtCover), 7_000, &RULES), vec![Step::Holding { took_ms: 3_000 }]);
        assert_eq!(b.phase, Phase::Hold { since: 7_000, first_death: None });
        let died = Facts { shooter: Shooter::Died { by_job: true }, ..facts(AtCover, AtCover) };
        let end = Phase::Done { at: 9_000, done: Done::ShooterDied { by_job: true } };
        assert_eq!(b.step(&died, 9_000, &RULES), vec![Step::Ended(end)]);
        // Ended stays ended.
        assert!(b.step(&facts(Down, Down), 10_000, &RULES).is_empty());
        assert_eq!(b.phase, end);
    }

    /// [Breach] The fails, each with its reason: no gather in time, a crosser down or gone before the go, a crosser untraded,
    /// both down, the hold's time, the objective worse; an unheld spot is Done, not a failure; a lost pinner is no failure.
    #[test]
    fn a_breach_fails_with_its_reason() {
        let mut b = job(0);
        b.step(&facts(AtGather, Moving), 8_000, &RULES);
        assert_eq!(failed(&b), Some(Failed::GatherTimedOut));
        let mut b = job(0);
        b.step(&facts(AtGather, Down), 1_000, &RULES);
        assert_eq!(failed(&b), Some(Failed::HolderDown { role: Role::Crosser }));
        let mut b = job(0);
        b.step(&facts(Gone, Moving), 1_000, &RULES);
        assert_eq!(failed(&b), Some(Failed::Aborted(Abort::NoCrossers)));
        // The pinner lost: noted, the job goes on.
        let mut b = job(0);
        let steps = b.step(&Facts { pinner: Some(PinnerNow::Down), ..facts(Moving, Moving) }, 1_000, &RULES);
        assert_eq!(steps, vec![Step::PinnerLost { down: true }]);
        assert_eq!((b.pinner, b.phase), (None, Phase::Gather { until: 8_000 }));
        // Go: the first crosser down -> Hold with its death; untraded after trade_ms.
        let mut b = job(0);
        b.step(&facts(AtGather, AtGather), 1_000, &RULES);
        let steps = b.step(&facts(Down, Moving), 2_000, &RULES);
        assert_eq!(steps, vec![Step::FirstDeath { at: 2_000 }, Step::Holding { took_ms: 1_000 }]);
        assert_eq!(b.phase, Phase::Hold { since: 2_000, first_death: Some(2_000) });
        b.step(&facts(Down, AtCover), 6_999, &RULES);
        assert!(!b.phase.ended());
        b.step(&facts(Down, AtCover), 7_000, &RULES);
        assert_eq!(failed(&b), Some(Failed::Untraded));
        // Traded: the camper dies within the window.
        let mut b = job(0);
        b.step(&facts(AtGather, AtGather), 1_000, &RULES);
        b.step(&facts(Down, Moving), 2_000, &RULES);
        b.step(&Facts { shooter: Shooter::Died { by_job: true }, ..facts(Down, AtCover) }, 5_000, &RULES);
        assert_eq!(b.phase, Phase::Done { at: 5_000, done: Done::ShooterDied { by_job: true } });
        // Both down at once in Go; both gone after the go.
        let mut b = job(0);
        b.step(&facts(AtGather, AtGather), 1_000, &RULES);
        b.step(&facts(Down, Down), 2_000, &RULES);
        assert_eq!(failed(&b), Some(Failed::HolderDown { role: Role::Crosser }));
        let mut b = job(0);
        b.step(&facts(AtGather, AtGather), 1_000, &RULES);
        b.step(&facts(Gone, Gone), 2_000, &RULES);
        assert_eq!(failed(&b), Some(Failed::Aborted(Abort::NoCrossers)));
        // The go's time runs: Hold, then its time.
        let mut b = job(0);
        b.step(&facts(AtGather, AtGather), 1_000, &RULES);
        b.step(&facts(Moving, AtCover), 7_000, &RULES);
        assert_eq!(b.phase, Phase::Hold { since: 7_000, first_death: None });
        b.step(&facts(AtCover, AtCover), 17_000, &RULES);
        assert_eq!(failed(&b), Some(Failed::Time));
        // The objective worse, in any phase before the end.
        let mut b = job(0);
        b.step(&Facts { objective_worse: true, ..facts(Moving, Moving) }, 500, &RULES);
        assert_eq!(failed(&b), Some(Failed::Aborted(Abort::ObjectiveWorse)));
        // Unheld: Done.
        let mut b = job(0);
        b.step(&Facts { shooter: Shooter::Unheld, ..facts(Moving, Moving) }, 500, &RULES);
        assert_eq!(b.phase, Phase::Done { at: 500, done: Done::Unheld });
    }

    #[test]
    fn a_role_is_found_by_its_bot() {
        let b = job(0);
        assert_eq!(b.role_of(1), Some(BreachRole::Pinner));
        assert_eq!(b.role_of(3), Some(BreachRole::Crosser(1)));
        assert_eq!(b.role_of(7), None);
        assert_eq!(b.places().collect::<Vec<_>>(), vec![1, 2, 3]);
        assert_eq!(b.from(), Vec3::new(4.0, 0.0, 10.0));
    }

    fn team<'a>(mates: &'a [Mate], kills: &'a [Kill]) -> Team<'a> {
        Team { mates, size: 12, objective_areas: &[], flank_jobs: &[], kills, points: None }
    }

    fn mate(place: Place, x: f32, z: f32, playing: Option<(Role, u32)>) -> Mate {
        Mate { place, feet: Vec3::new(x, 0.0, z), free: true, playing }
    }

    /// [Breach] A due spot opens with the two nearest free crossers out of his lanes and the next as the pinner; a crosser that
    /// does not take the play loses its role; his death by a crosser ends it Done.
    #[test]
    fn a_due_spot_opens_staffs_and_ends_with_his_death() {
        let held = [line()];
        let mut breaches = Vec::new();
        let mut due = vec![Due { spot: 4, until: 20_000 }];
        // Bot 5 stands in his lane (z 30 -> 60, x 0): not taken; 1, 2, 3 out of it.
        let mates = [mate(1, 20.0, 20.0, None), mate(2, -20.0, 20.0, None), mate(3, 25.0, 0.0, None), mate(5, 0.0, 50.0, None)];
        let mut plan = |_: &HeldLine, feet: [Vec3; CROSSERS]| Ok(feet.map(|f| Plan { gather: f, cover: f + Vec3::new(0.0, 0.0, 5.0) }));
        let ok = |_: &HeldLine, _: Vec3| Ok(());
        let events = tick(&mut breaches, &mut due, &held, &held, &team(&mates, &[]), &ok, &mut plan, 1_000, &RULES);
        assert_eq!(events, vec![Event::Opened { spot: 4, pinner: true, objective: false }]);
        assert!(due.is_empty());
        let b = &breaches[0];
        assert_eq!((b.crossers[0].holder.place, b.crossers[1].holder.place, b.pinner.map(|p| p.place)), (1, 2, Some(3)));
        // All took their plays; at the gather points: the go.
        let playing = [
            mate(1, 20.0, 20.0, Some((Role::Crosser, 4))),
            mate(2, -20.0, 20.0, Some((Role::Crosser, 4))),
            mate(3, 25.0, 0.0, Some((Role::Pinner, 4))),
        ];
        let events = tick(&mut breaches, &mut due, &held, &held, &team(&playing, &[]), &ok, &mut plan, 2_000, &RULES);
        assert!(matches!(events[..], [Event::Stepped { step: Step::Went { .. }, .. }]), "{events:?}");
        // His death by crosser 2.
        let kills = [Kill { victim: 9, killer: Some(2), time: 3_000 }];
        let events = tick(&mut breaches, &mut due, &held, &held, &team(&playing, &kills), &ok, &mut plan, 3_000, &RULES);
        assert!(
            matches!(events[..], [Event::Stepped { step: Step::Ended(Phase::Done { done: Done::ShooterDied { by_job: true }, .. }), .. }]),
            "{events:?}"
        );
        assert!(breaches.is_empty());
        // A crosser that never takes the play: gone after START_MS, the job aborted before the go.
        let mut due = vec![Due { spot: 4, until: 20_000 }];
        tick(&mut breaches, &mut due, &held, &held, &team(&mates, &[]), &ok, &mut plan, 10_000, &RULES);
        let events = tick(&mut breaches, &mut due, &held, &held, &team(&mates, &[]), &ok, &mut plan, 10_000 + START_MS, &RULES);
        assert!(
            matches!(events.last(), Some(Event::Stepped { step: Step::Ended(Phase::Failed { why: Failed::Aborted(Abort::NoCrossers), .. }), .. })),
            "{events:?}"
        );
    }

    /// [Breach] No Breach without two free crossers, over the cap, or where the memory refuses it; the refusals are events.
    #[test]
    fn a_due_spot_is_refused_with_its_reason() {
        let held = [line()];
        let mut plan = |_: &HeldLine, feet: [Vec3; CROSSERS]| Ok(feet.map(|f| Plan { gather: f, cover: f }));
        let ok = |_: &HeldLine, _: Vec3| Ok(());
        let one = [mate(1, 20.0, 20.0, None)];
        let two = [mate(1, 20.0, 20.0, None), mate(2, -20.0, 20.0, None)];
        let run = |mates: &[Mate], size, may: &MayTry, plan: &mut Planner| {
            let mut breaches = Vec::new();
            let mut due = vec![Due { spot: 4, until: 20_000 }];
            let team = Team { mates, size, objective_areas: &[], flank_jobs: &[], kills: &[], points: None };
            tick(&mut breaches, &mut due, &held, &held, &team, may, plan, 1_000, &RULES)
        };
        assert_eq!(run(&one, 12, &ok, &mut plan), vec![Event::NotOpened { spot: 4, why: NotOpened::NoCrossers }]);
        // A team of 3 has a cap of 1 bot in jobs off the objective.
        assert_eq!(run(&two, 3, &ok, &mut plan), vec![Event::NotOpened { spot: 4, why: NotOpened::Cap }]);
        let conceded = |_: &HeldLine, _: Vec3| Err(NotOpened::Conceded);
        assert_eq!(run(&two, 12, &conceded, &mut plan), vec![Event::NotOpened { spot: 4, why: NotOpened::Conceded }]);
        let mut no_cover = |_: &HeldLine, _: [Vec3; CROSSERS]| Err(NoPlan::NoCover);
        assert_eq!(run(&two, 12, &ok, &mut no_cover), vec![Event::NotOpened { spot: 4, why: NotOpened::Plan(NoPlan::NoCover) }]);
        // With two only: no pinner.
        assert_eq!(run(&two, 12, &ok, &mut plan), vec![Event::Opened { spot: 4, pinner: false, objective: false }]);
    }

    /// [Breach] On the soak field the plan finds two cover points the rules' distance and bearings apart that see his spot,
    /// out of his lanes, and a gather point for each out of his sight.
    #[test]
    fn the_plan_finds_two_cover_points_out_of_his_lanes_and_hidden_gather_points() {
        let tactics = Tactics::new(super::super::tactics::tests::field(), 1.0);
        let mut rng = Rng::new(7);
        let mut found = None;
        // Some spot of the field with a plan: the field's open middle and its walls.
        'search: for x in (-30..=30).step_by(10) {
            for z in (-30..=30).step_by(10) {
                let at = Vec3::new(x as f32, 0.0, z as f32);
                let Some(node) = tactics.world.grid.locate(Vec3::new(at.x, 1.0, at.z), 2.0) else {
                    continue;
                };
                let at = tactics.world.grid.position(node);
                let held = HeldLine::for_test(1, 9, at, &[at + Vec3::new(0.0, 0.0, 30.0)], Some(0));
                let feet = [at + Vec3::new(30.0, 0.0, 0.0), at + Vec3::new(-30.0, 0.0, 0.0)];
                if let Ok(plans) = plan(&tactics, &held, std::slice::from_ref(&held), feet, None, 0, &RULES, &mut rng) {
                    found = Some((held, plans));
                    break 'search;
                }
            }
        }
        let (held, plans) = found.expect("a spot of the field with a plan");
        let [a, b] = [plans[0].cover, plans[1].cover];
        assert!((RULES.apart_m.0..=RULES.apart_m.1).contains(&a.distance_xz(b)), "{plans:?}");
        for p in plans {
            assert!(!held.covers(p.cover) && !held.covers(p.gather), "{p:?}");
            assert!((RULES.cover_m.0..=RULES.cover_m.1).contains(&p.cover.distance_xz(held.at)), "{p:?}");
            assert!(p.gather.distance_xz(held.at) > p.cover.distance_xz(held.at), "{p:?}");
        }
    }
}
