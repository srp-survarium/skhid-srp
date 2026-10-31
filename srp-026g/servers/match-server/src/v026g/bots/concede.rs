//! design-p1c §3 "no feeding" and item 11: a team's memory of the tries on each shooter's area. A try that fails (a role
//! holder down, a death at the spot nobody traded, a job that timed out or was aborted) counts for the area, not for the
//! bot. After [`ConcedeRules::fails`] the area is conceded for [`ConcedeRules::concede_ms`]: no job and no Breach is
//! offered on it, the plays aimed into it score down, and the routes keep their K1 cost. A retry must differ from the last
//! try: another kind, an approach [`ConcedeRules::retry_deg`] or more off, or past [`ConcedeRules::retry_ms`] since it.
//!
//! The memory is kept per shooter area ([`AreaId`]), not per spot id: it survives the spot's clear (a teammate's look
//! deletes the spot) and its recentre. An area is found from a spot within [`AREA_M`] of its latest centre.
//!
//! One state per area ([`SpotState`]), one transition ([`step`]) over the events ([`SpotEvent`]), total. With the switch
//! off (`ThreatTuning::concede` is `None`) nothing is recorded and every try is offered.

use super::threat::{AREA_M, Role, TeamThreat};
use super::view::Place;
use mapdata::Vec3;

/// A team job's kind (K2): today's Flank (a flanker and a pinner), and the Breach of design-p1c §1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JobKind {
    Flank,
    Breach,
}

/// Why a try on an area failed: what the memory counts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failed {
    /// A role holder went down.
    HolderDown { role: Role },
    /// A death at the spot that nobody traded.
    Untraded,
    /// A Breach's group did not gather in time.
    GatherTimedOut,
    /// The job was given up.
    Aborted(Abort),
    /// It ran its time.
    Time,
}

impl Failed {
    /// Whether it counts towards a concede (design-p1c §3: "2 failed jobs or 2 untraded deaths"): a death only. A gather
    /// timeout, an abort or a job's time cost no life and say nothing of the spot; they still set the last try, so a
    /// retry must differ.
    pub fn counts(self) -> bool {
        matches!(self, Failed::HolderDown { .. } | Failed::Untraded)
    }
}

/// Why a job was given up ([`Failed::Aborted`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Abort {
    /// The team's objective state worsened while it ran (design-p1c item 16).
    ObjectiveWorse,
    /// Too few crossers were left to go.
    NoCrossers,
}

/// What was tried: a team job, the sniper answer (play 28, one bot), or a lone bot's own peek (item 15).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TryKind {
    Job(JobKind),
    Answer,
    Peek,
}

/// A try on an area: what, from which bearing (degrees from the spot towards the bot, 0 = +x, counter-clockwise), when.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Try {
    pub kind: TryKind,
    pub bearing: f32,
    pub at: u32,
}

/// An area's state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SpotState {
    /// No try failed on it since the last success.
    Open,
    /// `fails` counted failures ([`Failed::counts`]), fewer than the rules concede after; the last try that failed.
    Tried { fails: u8, last: Try },
    /// Conceded until `until`; the try that conceded it.
    Conceded { until: u32, last: Try },
}

/// What happened on an area.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SpotEvent {
    /// A try failed.
    Failed { kind: TryKind, from: Vec3, why: Failed },
    /// Its shooter was killed: the area is open again.
    Succeeded,
}

/// The rules, parsed once from `ThreatTuning::concede`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConcedeRules {
    /// Failed tries that concede the area.
    pub fails: u8,
    /// How long it stays conceded, ms.
    pub concede_ms: u32,
    /// A retry this many degrees or more off the last try's approach differs.
    pub retry_deg: f32,
    /// A retry this long after the last try differs, ms.
    pub retry_ms: u32,
}

/// What the plays read of the team's memory of tries (`threat::Picture::concede`, `camper-decision-map.md`): the held
/// lines whose shooter's area is conceded, and those whose area was tried with its last try, by line id, sorted.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Offers {
    pub conceded: Vec<u32>,
    pub tried: Vec<(u32, Try)>,
}

impl Offers {
    /// Whether held line `line`'s area is conceded.
    pub fn is_conceded(&self, line: u32) -> bool {
        self.conceded.binary_search(&line).is_ok()
    }

    /// The last failed try on held line `line`'s area, while it is tried (not conceded, not open).
    pub fn last_try(&self, line: u32) -> Option<Try> {
        self.tried.binary_search_by_key(&line, |&(id, _)| id).ok().map(|i| self.tried[i].1)
    }
}

/// What may be tried on an area now.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Offer {
    /// Anything.
    Any,
    /// Anything that differs from this try ([`differs`]).
    NotLike(Try),
    /// Nothing: conceded.
    None,
}

/// What an event did to an area's state ([`SpotMemory::apply`]), for the bench's counts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    /// Nothing that changes what is offered (an event on an open area that leaves it open, a try while conceded that cost no
    /// life, an unknown area).
    None,
    /// An open area's first failure.
    Tried,
    /// A failure on a tried area that did not concede it: a retry that failed.
    Retried,
    /// Conceded from open or tried.
    Conceded,
    /// Conceded again by a death while conceded.
    ConcededAgain,
    /// Opened by a success.
    Opened,
}

/// What the no-feeding rules did, counted by team for the bench's tally (`tests/tallies/owner/threat_answer.rs`, its
/// `concede` object).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tally {
    /// A death to a shooter before his spot was a hold (2 kills): how the team found him, not a try (not recorded).
    DeathBeforeHold,
    /// A death recorded on an area: a lone try's (a bot that held no role on its shooter's spot, the answer play's
    /// included) or a job role's.
    DeathPeek,
    DeathJob,
    /// A retry that failed (a failure on an area already tried), by the kind of the failed try.
    RetryPeek,
    RetryJob,
    /// An area conceded; conceded again while conceded; opened by its shooter's death.
    Conceded,
    ConcededAgain,
    Opened,
    /// A held spot given no job (spot-ticks): its area conceded; tried, and a new job would be like the last try.
    JobRefusedConceded,
    JobRefusedLike,
    /// A play's candidate scored for a target in a conceded area: refused (a fight or roam play), weighed (an objective
    /// play: never refused, item 13).
    PlayRefused,
    ObjectiveWeighed,
    /// A play taken whose target lies in a conceded area: an objective play (the objective's cost, item 13), another.
    TakenObjective,
    TakenOther,
    /// [item 15] A bot hit from a lane: a wait begun; a play refused for aiming into that lane while it waits; the wait
    /// ended by a mate near, by its time.
    PeekWaitBegun,
    PeekRefused,
    PeekWaitMate,
    PeekWaitTime,
}

/// The [`Tally`]s, in order, and their names in the tally.
pub const TALLIES: [(Tally, &str); 18] = [
    (Tally::DeathBeforeHold, "deaths_before_hold"),
    (Tally::DeathPeek, "deaths_peek"),
    (Tally::DeathJob, "deaths_job"),
    (Tally::RetryPeek, "retries_failed_peek"),
    (Tally::RetryJob, "retries_failed_job"),
    (Tally::Conceded, "conceded"),
    (Tally::ConcededAgain, "conceded_again"),
    (Tally::Opened, "opened_by_his_death"),
    (Tally::JobRefusedConceded, "job_refused_conceded"),
    (Tally::JobRefusedLike, "job_refused_like_last"),
    (Tally::PlayRefused, "play_refused"),
    (Tally::ObjectiveWeighed, "objective_weighed"),
    (Tally::TakenObjective, "taken_objective_into_conceded"),
    (Tally::TakenOther, "taken_other_into_conceded"),
    (Tally::PeekWaitBegun, "peek_wait_begun"),
    (Tally::PeekRefused, "peek_refused"),
    (Tally::PeekWaitMate, "peek_wait_ended_mate"),
    (Tally::PeekWaitTime, "peek_wait_ended_time"),
];

#[cfg(test)]
thread_local! {
    static TALLIED: std::cell::Cell<[[u64; TALLIES.len()]; 2]> = const { std::cell::Cell::new([[0; TALLIES.len()]; 2]) };
}

/// Team `team`'s `t` happened once (the bench's count; nothing live).
#[cfg(test)]
pub fn tally(team: u8, t: Tally) {
    TALLIED.with(|cell| {
        let mut all = cell.get();
        all[usize::from(team & 1)][t as usize] += 1;
        cell.set(all);
    });
}

#[cfg(not(test))]
pub fn tally(_: u8, _: Tally) {}

/// The [`Tally`]s so far, by team and [`TALLIES`] order.
#[cfg(test)]
pub fn tallied() -> [[u64; TALLIES.len()]; 2] {
    TALLIED.with(|cell| cell.get())
}

/// The bearing of `from` seen from the spot `at`, degrees in [0, 360).
pub fn bearing(at: Vec3, from: Vec3) -> f32 {
    let deg = (from.z - at.z).atan2(from.x - at.x).to_degrees();
    if deg < 0.0 { deg + 360.0 } else { deg }
}

/// The smaller angle between two bearings, degrees in [0, 180].
fn apart(a: f32, b: f32) -> f32 {
    let d = (a - b).abs() % 360.0;
    if d > 180.0 { 360.0 - d } else { d }
}

/// Whether a try `next` differs from `last` by the rules: another kind, an approach `retry_deg` or more off, or
/// `retry_ms` or more after it.
pub fn differs(last: &Try, next: &Try, rules: &ConcedeRules) -> bool {
    last.kind != next.kind
        || apart(last.bearing, next.bearing) >= rules.retry_deg
        || next.at.saturating_sub(last.at) >= rules.retry_ms
}

/// The one transition: an area's state after `event` at `now` (its centre `at`, for a failure's bearing). A conceded
/// area whose time is over is open before the event is applied.
pub fn step(state: SpotState, at: Vec3, event: SpotEvent, now: u32, rules: &ConcedeRules) -> SpotState {
    let state = match state {
        SpotState::Conceded { until, .. } if now >= until => SpotState::Open,
        s => s,
    };
    match event {
        SpotEvent::Succeeded => SpotState::Open,
        SpotEvent::Failed { kind, from, why } => {
            let last = Try { kind, bearing: bearing(at, from), at: now };
            let add = u8::from(why.counts());
            let fails = match state {
                SpotState::Open => add,
                SpotState::Tried { fails, .. } => fails.saturating_add(add),
                // A failure while conceded (a lone bot's own try): a death concedes it again from now; else it stays as it
                // was, with this try the last.
                SpotState::Conceded { until, .. } if add == 0 => return SpotState::Conceded { until, last },
                SpotState::Conceded { .. } => rules.fails,
            };
            if fails >= rules.fails {
                SpotState::Conceded { until: now.saturating_add(rules.concede_ms), last }
            } else {
                SpotState::Tried { fails, last }
            }
        }
    }
}

/// A bot of team `team` died at `from` to `shooter`, whom the team sensed at `at` lately (the K1 spot's rule,
/// `ThreatSpots::died`), his spot a hold already (`camper-decision-map.md` §2: the deaths that make the hold are how the
/// team finds him, `facts.rs` counts them apart): a failed try on his area, the one record of a death. A bot that held a
/// role of a job on his spot failed that job ([`Failed::HolderDown`]); any other failed what it was doing, `play` (the
/// answer play on him, else a lone peek: [`Failed::Untraded`], his death opens the area again, so a death traded before
/// he kills again is undone).
#[allow(clippy::too_many_arguments)]
pub fn died(threat: &mut TeamThreat, team: u8, victim: Place, from: Vec3, shooter: Place, at: Vec3, play: TryKind, now: u32, rules: &ConcedeRules) {
    let role = threat.jobs.iter().filter(|j| j.shooter == shooter).find_map(|j| j.role_of(victim));
    let (kind, why) = match role {
        Some(role) => (TryKind::Job(JobKind::Flank), Failed::HolderDown { role }),
        None => (play, Failed::Untraded),
    };
    let memory = &mut threat.spot_memory;
    let area = memory.area_of(at, Some(shooter), now);
    let retry = !matches!(memory.state(area, now), SpotState::Open);
    let change = memory.apply(area, SpotEvent::Failed { kind, from, why }, now, rules);
    let job = role.is_some();
    tally(team, if job { Tally::DeathJob } else { Tally::DeathPeek });
    if retry {
        tally(team, if job { Tally::RetryJob } else { Tally::RetryPeek });
    }
    match change {
        Change::Conceded => tally(team, Tally::Conceded),
        Change::ConcededAgain => tally(team, Tally::ConcededAgain),
        _ => {}
    }
}

/// Enemy `shooter` died at `now`: team `team`'s areas of his are open again.
pub fn shooter_died(threat: &mut TeamThreat, team: u8, shooter: Place, now: u32, rules: &ConcedeRules) {
    for _ in 0..threat.spot_memory.shooter_died(shooter, now, rules) {
        tally(team, Tally::Opened);
    }
}

/// A shooter area in a team's [`SpotMemory`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AreaId(u32);

#[derive(Clone, Debug)]
struct Area {
    id: AreaId,
    /// The latest centre of its spots.
    at: Vec3,
    shooter: Option<Place>,
    state: SpotState,
    /// The last event, for forgetting an area that has been open this long.
    touched: u32,
}

/// An open area untouched this long is forgotten, ms.
const FORGET_MS: u32 = 300_000;

/// A team's memory of the tries on each shooter area.
#[derive(Clone, Debug, Default)]
pub struct SpotMemory {
    areas: Vec<Area>,
    next: u32,
}

impl SpotMemory {
    /// The area of a spot at `at` (its shooter, if known): the nearest within [`AREA_M`] of its latest centre, which
    /// moves to `at`; else a new one.
    pub fn area_of(&mut self, at: Vec3, shooter: Option<Place>, now: u32) -> AreaId {
        let near = self
            .areas
            .iter_mut()
            .filter(|a| a.at.distance_xz(at) <= AREA_M)
            .min_by(|a, b| a.at.distance_xz(at).total_cmp(&b.at.distance_xz(at)));
        if let Some(area) = near {
            area.at = at;
            area.shooter = shooter.or(area.shooter);
            return area.id;
        }
        let id = AreaId(self.next);
        self.next = self.next.wrapping_add(1);
        self.areas.push(Area { id, at, shooter, state: SpotState::Open, touched: now });
        id
    }

    fn get(&self, area: AreaId) -> Option<&Area> {
        self.areas.iter().find(|a| a.id == area)
    }

    /// The area of a spot at `at`, if one is kept: the nearest within [`AREA_M`] of its latest centre. Reads only (a
    /// spot's lookup by the jobs and the plays moves no centre).
    pub fn area_at(&self, at: Vec3) -> Option<AreaId> {
        self.areas
            .iter()
            .filter(|a| a.at.distance_xz(at) <= AREA_M)
            .min_by(|a, b| a.at.distance_xz(at).total_cmp(&b.at.distance_xz(at)))
            .map(|a| a.id)
    }

    /// Whether the area of a spot at `at` is conceded at `now`.
    pub fn conceded_at(&self, at: Vec3, now: u32) -> bool {
        self.area_at(at).is_some_and(|a| self.conceded(a, now))
    }

    /// `event` on `area` at `now`; what it changed (the bench's counts).
    pub fn apply(&mut self, area: AreaId, event: SpotEvent, now: u32, rules: &ConcedeRules) -> Change {
        let Some(a) = self.areas.iter_mut().find(|a| a.id == area) else {
            return Change::None;
        };
        let was = match a.state {
            SpotState::Conceded { until, .. } if now >= until => SpotState::Open,
            s => s,
        };
        a.state = step(a.state, a.at, event, now, rules);
        a.touched = now;
        match (was, a.state) {
            (SpotState::Open, SpotState::Open) => Change::None,
            (_, SpotState::Open) => Change::Opened,
            (SpotState::Conceded { until: before, .. }, SpotState::Conceded { until, .. }) if until != before => Change::ConcededAgain,
            // A try while conceded that cost no life: the last try only.
            (SpotState::Conceded { .. }, _) => Change::None,
            (_, SpotState::Conceded { .. }) => Change::Conceded,
            (SpotState::Open, SpotState::Tried { .. }) => Change::Tried,
            (SpotState::Tried { .. }, SpotState::Tried { .. }) => Change::Retried,
        }
    }

    /// Shooter `shooter` died at `now`: each area of his is open again (a death of his traded or ended the try).
    pub fn shooter_died(&mut self, shooter: Place, now: u32, rules: &ConcedeRules) -> usize {
        let ids: Vec<AreaId> = self.areas.iter().filter(|a| a.shooter == Some(shooter)).map(|a| a.id).collect();
        ids.into_iter().filter(|&id| self.apply(id, SpotEvent::Succeeded, now, rules) == Change::Opened).count()
    }

    /// A job's failure on `area`, its bot (or the try's) approaching from `from`.
    pub fn record(&mut self, area: AreaId, kind: JobKind, why: Failed, from: Vec3, now: u32, rules: &ConcedeRules) -> Change {
        self.apply(area, SpotEvent::Failed { kind: TryKind::Job(kind), from, why }, now, rules)
    }

    /// The state of `area` at `now` (a concede that is over reads open).
    pub fn state(&self, area: AreaId, now: u32) -> SpotState {
        match self.get(area).map(|a| a.state) {
            Some(SpotState::Conceded { until, .. }) if now >= until => SpotState::Open,
            Some(s) => s,
            None => SpotState::Open,
        }
    }

    /// Whether `area` is conceded at `now`.
    pub fn conceded(&self, area: AreaId, now: u32) -> bool {
        matches!(self.state(area, now), SpotState::Conceded { .. })
    }

    /// The last failed try on `area`: its kind, approach bearing and time.
    pub fn last_try(&self, area: AreaId) -> Option<(TryKind, f32, u32)> {
        match self.get(area)?.state {
            SpotState::Open => None,
            SpotState::Tried { last, .. } | SpotState::Conceded { last, .. } => Some((last.kind, last.bearing, last.at)),
        }
    }

    /// What may be tried on `area` at `now`.
    pub fn offers(&self, area: AreaId, now: u32) -> Offer {
        match self.state(area, now) {
            SpotState::Open => Offer::Any,
            SpotState::Tried { last, .. } => Offer::NotLike(last),
            SpotState::Conceded { .. } => Offer::None,
        }
    }

    /// The areas open and untouched for [`FORGET_MS`] are forgotten.
    pub fn forget_old(&mut self, now: u32) {
        self.areas.retain(|a| !(matches!(a.state, SpotState::Open) && now.saturating_sub(a.touched) >= FORGET_MS));
    }

    /// The bench's: areas conceded at `now`.
    pub fn conceded_count(&self, now: u32) -> usize {
        self.areas.iter().filter(|a| self.conceded(a.id, now)).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RULES: ConcedeRules = ConcedeRules { fails: 2, concede_ms: 100_000, retry_deg: 45.0, retry_ms: 30_000 };
    const SPOT: Vec3 = Vec3::new(0.0, 0.0, 0.0);

    fn fail(from: Vec3) -> SpotEvent {
        SpotEvent::Failed { kind: TryKind::Job(JobKind::Flank), from, why: Failed::Untraded }
    }

    #[test]
    fn two_failures_concede_and_the_concede_ends() {
        let s = step(SpotState::Open, SPOT, fail(Vec3::new(10.0, 0.0, 0.0)), 1_000, &RULES);
        assert!(matches!(s, SpotState::Tried { fails: 1, .. }));
        let s = step(s, SPOT, fail(Vec3::new(0.0, 0.0, 10.0)), 2_000, &RULES);
        assert_eq!(s, SpotState::Conceded { until: 102_000, last: Try { kind: TryKind::Job(JobKind::Flank), bearing: 90.0, at: 2_000 } });
        // Over: open before the next event.
        let s = step(s, SPOT, fail(SPOT + Vec3::new(10.0, 0.0, 0.0)), 102_000, &RULES);
        assert!(matches!(s, SpotState::Tried { fails: 1, .. }));
    }

    #[test]
    fn a_success_opens_from_any_state() {
        for s in [
            SpotState::Open,
            step(SpotState::Open, SPOT, fail(Vec3::new(1.0, 0.0, 0.0)), 0, &RULES),
            SpotState::Conceded { until: 50_000, last: Try { kind: TryKind::Peek, bearing: 0.0, at: 0 } },
        ] {
            assert_eq!(step(s, SPOT, SpotEvent::Succeeded, 1_000, &RULES), SpotState::Open);
        }
    }

    #[test]
    fn a_retry_differs_by_kind_angle_or_time() {
        let last = Try { kind: TryKind::Job(JobKind::Flank), bearing: 350.0, at: 0 };
        let like = Try { kind: TryKind::Job(JobKind::Flank), bearing: 20.0, at: 10_000 };
        assert!(!differs(&last, &like, &RULES), "30 degrees apart across 0, 10 s later");
        assert!(differs(&last, &Try { kind: TryKind::Job(JobKind::Breach), ..like }, &RULES));
        assert!(differs(&last, &Try { bearing: 50.0, ..like }, &RULES));
        assert!(differs(&last, &Try { at: 30_000, ..like }, &RULES));
    }

    #[test]
    fn an_area_survives_a_recentre_and_its_spot_id() {
        let mut m = SpotMemory::default();
        let a = m.area_of(SPOT, Some(3), 0);
        m.record(a, JobKind::Flank, Failed::Untraded, Vec3::new(20.0, 0.0, 0.0), 1_000, &RULES);
        // The spot was cleared and made again 8 m off: the same area, its failure kept.
        let b = m.area_of(Vec3::new(8.0, 0.0, 0.0), None, 5_000);
        assert_eq!(a, b);
        // A gather timeout costs no life: not counted, but it is the last try.
        m.record(b, JobKind::Breach, Failed::GatherTimedOut, Vec3::new(0.0, 0.0, 20.0), 5_500, &RULES);
        assert!(!m.conceded(a, 5_500));
        assert!(matches!(m.last_try(a), Some((TryKind::Job(JobKind::Breach), _, 5_500))));
        m.record(b, JobKind::Breach, Failed::HolderDown { role: Role::Pinner }, Vec3::new(0.0, 0.0, 20.0), 6_000, &RULES);
        assert!(m.conceded(a, 6_000));
        assert_eq!(m.offers(a, 6_000), Offer::None);
        assert_eq!(m.offers(a, 106_000), Offer::Any);
        // Far from it: another area.
        assert_ne!(m.area_of(Vec3::new(40.0, 0.0, 0.0), None, 7_000), a);
    }

    #[test]
    fn a_tried_area_offers_only_a_different_try() {
        let mut m = SpotMemory::default();
        let a = m.area_of(SPOT, None, 0);
        assert_eq!(m.offers(a, 0), Offer::Any);
        m.record(a, JobKind::Flank, Failed::HolderDown { role: Role::Flanker }, Vec3::new(0.0, 0.0, -10.0), 1_000, &RULES);
        let Offer::NotLike(last) = m.offers(a, 2_000) else { panic!("tried once") };
        assert_eq!((last.kind, last.bearing), (TryKind::Job(JobKind::Flank), 270.0));
        assert_eq!(m.last_try(a), Some((TryKind::Job(JobKind::Flank), 270.0, 1_000)));
    }

    #[test]
    fn each_event_says_what_it_changed() {
        let mut m = SpotMemory::default();
        let a = m.area_of(SPOT, Some(7), 0);
        let death = |why| SpotEvent::Failed { kind: TryKind::Peek, from: Vec3::new(10.0, 0.0, 0.0), why };
        assert_eq!(m.apply(a, SpotEvent::Succeeded, 0, &RULES), Change::None, "open stays open");
        assert_eq!(m.apply(a, death(Failed::Untraded), 1_000, &RULES), Change::Tried);
        assert_eq!(m.apply(a, death(Failed::Time), 2_000, &RULES), Change::Retried, "a retry that cost no life");
        assert_eq!(m.apply(a, death(Failed::Untraded), 3_000, &RULES), Change::Conceded);
        assert_eq!(m.apply(a, death(Failed::Time), 4_000, &RULES), Change::None, "while conceded, no life lost");
        assert_eq!(m.apply(a, death(Failed::Untraded), 5_000, &RULES), Change::ConcededAgain);
        assert_eq!(m.state(a, 5_000), SpotState::Conceded { until: 105_000, last: Try { kind: TryKind::Peek, bearing: 0.0, at: 5_000 } });
        assert_eq!(m.apply(a, SpotEvent::Succeeded, 6_000, &RULES), Change::Opened);
        assert_eq!(m.apply(AreaId(99), SpotEvent::Succeeded, 6_000, &RULES), Change::None, "an unknown area");
    }

    #[test]
    fn two_lone_deaths_to_a_shooter_concede_his_area_until_he_dies() {
        let mut threat = TeamThreat::default();
        died(&mut threat, 1, 3, Vec3::new(20.0, 0.0, 0.0), 7, SPOT, TryKind::Peek, 1_000, &RULES);
        assert!(!threat.spot_memory.conceded_at(SPOT, 1_000), "one death");
        // Sensed 2 m off: his area, its centre moved; the answer play's try.
        died(&mut threat, 1, 4, Vec3::new(21.0, 0.0, 3.0), 7, Vec3::new(2.0, 0.0, 0.0), TryKind::Answer, 5_000, &RULES);
        assert!(matches!(threat.spot_memory.last_try(threat.spot_memory.area_at(SPOT).expect("kept")), Some((TryKind::Answer, _, 5_000))));
        assert!(threat.spot_memory.conceded_at(SPOT, 5_000));
        assert!(threat.spot_memory.area_at(Vec3::new(30.0, 0.0, 0.0)).is_none(), "far from it: no area");
        shooter_died(&mut threat, 1, 8, 6_000, &RULES);
        assert!(threat.spot_memory.conceded_at(SPOT, 6_000), "another enemy's death");
        shooter_died(&mut threat, 1, 7, 7_000, &RULES);
        assert!(!threat.spot_memory.conceded_at(SPOT, 7_000), "his death opens it");
        assert_eq!(threat.spot_memory.offers(threat.spot_memory.area_at(SPOT).expect("kept"), 7_000), Offer::Any);
    }

    #[test]
    fn the_tallies_are_named_in_their_order() {
        for (i, (t, _)) in TALLIES.iter().enumerate() {
            assert_eq!(*t as usize, i, "{t:?}");
        }
    }

    #[test]
    fn open_areas_are_forgotten_and_tried_ones_kept() {
        let mut m = SpotMemory::default();
        let open = m.area_of(SPOT, None, 0);
        let tried = m.area_of(Vec3::new(50.0, 0.0, 0.0), None, 0);
        m.record(tried, JobKind::Flank, Failed::Time, SPOT, 0, &RULES);
        m.forget_old(FORGET_MS);
        assert!(m.get(open).is_none());
        assert!(m.get(tried).is_some());
    }
}
