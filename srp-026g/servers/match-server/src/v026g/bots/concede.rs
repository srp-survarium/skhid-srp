//! design-p1c §3 "no feeding" and item 11: a team's memory of the tries on each shooter's area. A try that fails (a role
//! holder down, a death at the spot nobody traded, a job that timed out or was aborted) counts for the area, not for the
//! bot. After [`ConcedeRules::fails`] the area is conceded for [`ConcedeRules::concede_ms`]: no job and no Breach is
//! offered on it, the plays aimed into it score down, and the routes keep their K1 cost. A retry must differ from the last
//! try: another kind, an approach [`ConcedeRules::retry_deg`] or more off, or past [`ConcedeRules::retry_ms`] since it.
//!
//! The memory is kept per shooter area ([`AreaId`]), not per spot id: it survives the spot's clear (a teammate's look
//! deletes the spot) and its recentre. An area is found from a spot within [`AREA_M`] of its latest centre.
//!
//! His death ([`SpotEvent::Succeeded`]) opens his areas, or, with [`ConcedeRules::keep_ms`], keeps their tries for that
//! long ([`SpotState::Down`]): a camper killed at his spot is back there within 180 s in 69 % of the live PvB runs (median
//! 41 s after his death) and kills from it again in 53 % (`traces-20261028/fx-threat-obj-same-spot-note.md`). While he is
//! down anything is offered; the team sensing him back in it, or a failure on it, takes up the tries where they were.
//!
//! One state per area ([`SpotState`]), one transition ([`step`]) over the events ([`SpotEvent`]), total. With the switch
//! off (`ThreatTuning::concede` is `None`) nothing is recorded and every try is offered.

use super::threat::{AREA_M, Picture, Role, TeamThreat};
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
    /// Its shooter died ([`ConcedeRules::keep_ms`]): anything is offered while he is down, and until `until` the tries are
    /// kept, `fails` as counted (a conceded area's: the rules' `fails`), to take up when he is back ([`SpotEvent::Returned`])
    /// or a try fails on it again. Open after `until`.
    Down { fails: u8, last: Try, until: u32 },
}

/// `state` at `now`: a concede that is over, or tries kept past their time, read open.
fn settled(state: SpotState, now: u32) -> SpotState {
    match state {
        SpotState::Conceded { until, .. } | SpotState::Down { until, .. } if now >= until => SpotState::Open,
        s => s,
    }
}

/// What happened on an area.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SpotEvent {
    /// A try failed.
    Failed { kind: TryKind, from: Vec3, why: Failed },
    /// A death to him before his spot was a hold (how the team found him, not a try on him): it counts toward no concede,
    /// but it is the last try, so the next must differ from it (`camper-decision-map.md` §2).
    Found { from: Vec3 },
    /// Its shooter was killed: the area is open again, or [`SpotState::Down`] with [`ConcedeRules::keep_ms`].
    Succeeded,
    /// Its shooter, down, was sensed back in it: the tries kept are taken up (conceded again from now if they had conceded it).
    Returned,
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
    /// His death keeps his areas' tries this long, ms ([`SpotState::Down`]); none: it opens them.
    pub keep_ms: Option<u32>,
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
    /// Its tries kept by its shooter's death ([`SpotState::Down`]).
    Downed,
    /// Its shooter sensed back in it: conceded again, or tried again.
    BackConceded,
    BackTried,
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
    /// An area conceded; conceded again while conceded; opened by its shooter's death; its tries kept by his death; he was
    /// sensed back in it, and it was conceded again or tried again.
    Conceded,
    ConcededAgain,
    Opened,
    Downed,
    BackConceded,
    BackTried,
    /// A held spot given no job (spot-ticks): its area conceded; tried, and a new job would be like the last try.
    JobRefusedConceded,
    JobRefusedLike,
    /// A play's candidate scored for a target in a conceded area: refused (a fight or roam play), weighed (an objective
    /// play: never refused, item 13).
    PlayRefused,
    ObjectiveWeighed,
    /// The same for a target on a tried area whose try would be like the last ([`like_last`]).
    PlayRefusedLike,
    ObjectiveWeighedLike,
    /// A play taken whose target lies in a conceded area: an objective play (the objective's cost, item 13), another.
    TakenObjective,
    TakenOther,
    /// [item 15] A hit a bot took from a shooter it sensed (its reflex's, any shooter); a play of a lone bot refused (or,
    /// objective, weighed) for aiming back into a held spot's lane it was hit from; one let go because a mate stood near.
    LoneHit,
    LonePeekRefused,
    LonePeekWithMate,
}

/// The [`Tally`]s, in order, and their names in the tally.
pub const TALLIES: [(Tally, &str); 22] = [
    (Tally::DeathBeforeHold, "deaths_before_hold"),
    (Tally::DeathPeek, "deaths_peek"),
    (Tally::DeathJob, "deaths_job"),
    (Tally::RetryPeek, "retries_failed_peek"),
    (Tally::RetryJob, "retries_failed_job"),
    (Tally::Conceded, "conceded"),
    (Tally::ConcededAgain, "conceded_again"),
    (Tally::Opened, "opened_by_his_death"),
    (Tally::Downed, "kept_by_his_death"),
    (Tally::BackConceded, "conceded_on_his_return"),
    (Tally::BackTried, "tried_on_his_return"),
    (Tally::JobRefusedConceded, "job_refused_conceded"),
    (Tally::JobRefusedLike, "job_refused_like_last"),
    (Tally::PlayRefused, "play_refused"),
    (Tally::ObjectiveWeighed, "objective_weighed"),
    (Tally::PlayRefusedLike, "play_refused_like_last"),
    (Tally::ObjectiveWeighedLike, "objective_weighed_like_last"),
    (Tally::TakenObjective, "taken_objective_into_conceded"),
    (Tally::TakenOther, "taken_other_into_conceded"),
    (Tally::LoneHit, "lone_hits"),
    (Tally::LonePeekRefused, "lone_peek_refused"),
    (Tally::LonePeekWithMate, "lone_peek_with_mate"),
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

/// One change of a team's spot memory, for the bench's events (`tests/bench.rs`, its `"concede"` lines; nothing live): when,
/// the team, the area's shooter, what caused it (a death to him before his hold, a counted failure, his death, his return),
/// the change, the area's centre and when its concede or kept tries end.
#[cfg(test)]
#[derive(Clone, Copy, Debug)]
pub struct Logged {
    pub at: u32,
    pub team: u8,
    pub shooter: Place,
    pub cause: &'static str,
    pub change: Change,
    pub centre: Vec3,
    pub until: Option<u32>,
}

#[cfg(test)]
thread_local! {
    static LOGGED: std::cell::RefCell<Vec<Logged>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Team `team`'s memory `memory` changed `area` by `change` (none: nothing logged).
#[cfg(test)]
fn log(memory: &SpotMemory, team: u8, shooter: Place, area: AreaId, cause: &'static str, change: Change, now: u32) {
    let Some(a) = memory.get(area).filter(|_| change != Change::None) else {
        return;
    };
    let until = match a.state {
        SpotState::Conceded { until, .. } | SpotState::Down { until, .. } => Some(until),
        SpotState::Open | SpotState::Tried { .. } => None,
    };
    LOGGED.with(|log| log.borrow_mut().push(Logged { at: now, team, shooter, cause, change, centre: a.at, until }));
}

/// The changes logged since the last call.
#[cfg(test)]
pub fn take_logged() -> Vec<Logged> {
    LOGGED.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

/// [item 15] A hit a bot took from a shooter it sensed (`Reflex::last_hit`): where it sensed him, where the bot stood, when.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    pub shooter_at: Vec3,
    pub from: Vec3,
    pub at: u32,
}

/// [item 15] Whether a try aimed at `to` by a bot at `feet` goes back into the lane it was `hit` from: the shooter's spot a
/// held line of `picture` (he was sensed within its area), `to` in that area or its lanes, and the try's bearing (from where
/// the bot stands at his area, from `to` at a point of his lanes) within `retry_deg` of the bearing the hit came from.
pub fn into_hit_lane(picture: &Picture, hit: &Hit, to: Vec3, feet: Vec3, rules: &ConcedeRules) -> bool {
    picture.held.iter().any(|line| {
        if line.at.distance_xz(hit.shooter_at) > AREA_M {
            return false;
        }
        let into_area = line.at.distance_xz(to) <= AREA_M;
        if !into_area && !line.covers(to) {
            return false;
        }
        let from = if into_area { feet } else { to };
        apart(bearing(line.at, from), bearing(line.at, hit.from)) < rules.retry_deg
    })
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
/// area whose time is over, or tries kept past theirs, are open before the event is applied ([`settled`]).
pub fn step(state: SpotState, at: Vec3, event: SpotEvent, now: u32, rules: &ConcedeRules) -> SpotState {
    let state = settled(state, now);
    let (kind, from, add) = match event {
        SpotEvent::Succeeded => {
            let Some(keep) = rules.keep_ms else {
                return SpotState::Open;
            };
            let until = now.saturating_add(keep);
            return match state {
                SpotState::Open => SpotState::Open,
                SpotState::Tried { fails, last } | SpotState::Down { fails, last, .. } => SpotState::Down { fails, last, until },
                SpotState::Conceded { last, .. } => SpotState::Down { fails: rules.fails, last, until },
            };
        }
        SpotEvent::Returned => {
            return match state {
                SpotState::Down { fails, last, .. } if fails >= rules.fails => {
                    SpotState::Conceded { until: now.saturating_add(rules.concede_ms), last }
                }
                SpotState::Down { fails, last, .. } => SpotState::Tried { fails, last },
                s => s,
            };
        }
        SpotEvent::Found { from } => (TryKind::Peek, from, 0),
        SpotEvent::Failed { kind, from, why } => (kind, from, u8::from(why.counts())),
    };
    let last = Try { kind, bearing: bearing(at, from), at: now };
    let fails = match state {
        SpotState::Open => add,
        // A failure while he was down takes his kept tries up: a death on his return counts on them.
        SpotState::Tried { fails, .. } | SpotState::Down { fails, .. } => fails.saturating_add(add),
        // A failure while conceded (a lone bot's own try): a death concedes it again from now; else it stays as it was, with
        // this try the last.
        SpotState::Conceded { until, .. } if add == 0 => return SpotState::Conceded { until, last },
        SpotState::Conceded { .. } => rules.fails,
    };
    if fails >= rules.fails {
        SpotState::Conceded { until: now.saturating_add(rules.concede_ms), last }
    } else {
        SpotState::Tried { fails, last }
    }
}

/// A bot of team `team` died at `from` to `shooter`, whom the team sensed at `at` lately (the K1 spot's rule,
/// `ThreatSpots::died`), his spot a hold already (`camper-decision-map.md` §2: the deaths that make the hold are how the
/// team finds him, `facts.rs` counts them apart): a failed try on his area, the one record of a death. A bot that held a
/// role of a job on his spot (a Flank job, or a Breach on him) failed that job ([`Failed::HolderDown`]); any other failed
/// what it was doing, `play` (the answer play on him, else a lone peek: [`Failed::Untraded`]; without
/// [`ConcedeRules::keep_ms`] his death opens the area again, so a death traded before he kills again is undone).
#[allow(clippy::too_many_arguments)]
pub fn died(threat: &mut TeamThreat, team: u8, victim: Place, from: Vec3, shooter: Place, at: Vec3, play: TryKind, now: u32, rules: &ConcedeRules) {
    let role = threat
        .jobs
        .iter()
        .filter(|j| j.shooter == shooter)
        .find_map(|j| j.role_of(victim).map(|role| (JobKind::Flank, role)))
        .or_else(|| threat.breaches.iter().filter(|b| b.shooter == shooter).find_map(|b| b.role_of(victim).map(|role| (JobKind::Breach, role.role()))));
    let (kind, why) = match role {
        Some((job, role)) => (TryKind::Job(job), Failed::HolderDown { role }),
        None => (play, Failed::Untraded),
    };
    let memory = &mut threat.spot_memory;
    let area = memory.area_of(at, Some(shooter), now);
    let retry = !matches!(memory.state(area, now), SpotState::Open);
    let change = memory.apply(area, SpotEvent::Failed { kind, from, why }, now, rules);
    #[cfg(test)]
    log(memory, team, shooter, area, "failed", change, now);
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

/// A bot of team `team` died at `from` to `shooter`, sensed at `at`, before his spot was a hold: how the team found him,
/// counted toward no concede, but the last try on his area, so the next must differ ([`SpotEvent::Found`]).
pub fn found(threat: &mut TeamThreat, team: u8, from: Vec3, shooter: Place, at: Vec3, now: u32, rules: &ConcedeRules) {
    let memory = &mut threat.spot_memory;
    let area = memory.area_of(at, Some(shooter), now);
    let _change = memory.apply(area, SpotEvent::Found { from }, now, rules);
    #[cfg(test)]
    log(memory, team, shooter, area, "found", _change, now);
    tally(team, Tally::DeathBeforeHold);
}

/// design-p1c §3 "each retry must differ from the last": whether a try of `kind` aimed at `to` by a bot at `feet` is like
/// the last failed try on a tried held line of `picture` it aims into (within the line's area, or in its lanes): the same
/// kind, within `retry_deg` of its bearing, sooner than `retry_ms` after it. A try aimed at his area comes from where the
/// bot stands; one at a point of his lanes, from that point.
pub fn like_last(picture: &Picture, to: Vec3, feet: Vec3, kind: TryKind, now: u32, rules: &ConcedeRules) -> bool {
    picture.held.iter().any(|line| {
        let Some(last) = picture.concede.last_try(line.id) else {
            return false;
        };
        let into_area = line.at.distance_xz(to) <= AREA_M;
        if !into_area && !line.covers(to) {
            return false;
        }
        let from = if into_area { feet } else { to };
        !differs(&last, &Try { kind, bearing: bearing(line.at, from), at: now }, rules)
    })
}

/// Enemy `shooter` died at `now`: team `team`'s areas of his are open again, or keep their tries ([`SpotState::Down`]).
pub fn shooter_died(threat: &mut TeamThreat, team: u8, shooter: Place, now: u32, rules: &ConcedeRules) {
    #[cfg(test)]
    let areas = threat.spot_memory.areas_of(shooter);
    let changes = threat.spot_memory.shooter_died(shooter, now, rules);
    #[cfg(test)]
    for (&area, &change) in areas.iter().zip(&changes) {
        log(&threat.spot_memory, team, shooter, area, "his_death", change, now);
    }
    for change in changes {
        match change {
            Change::Opened => tally(team, Tally::Opened),
            Change::Downed => tally(team, Tally::Downed),
            _ => {}
        }
    }
}

/// Team `team` sensed enemy `shooter` at `at` at `now` (its own senses, as K1's `ThreatSpots::shooter_sensed`): an area of
/// his that keeps its tries since his death, with `at` in it, takes them up ([`SpotEvent::Returned`]).
pub fn shooter_sensed(threat: &mut TeamThreat, team: u8, shooter: Place, at: Vec3, now: u32, rules: &ConcedeRules) {
    #[cfg(test)]
    let area = threat.spot_memory.back_area(shooter, at, now);
    let change = threat.spot_memory.shooter_sensed(shooter, at, now, rules);
    #[cfg(test)]
    if let Some(area) = area {
        log(&threat.spot_memory, team, shooter, area, "his_return", change, now);
    }
    match change {
        Change::BackConceded => tally(team, Tally::BackConceded),
        Change::BackTried => tally(team, Tally::BackTried),
        _ => {}
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
        let was = settled(a.state, now);
        a.state = step(a.state, a.at, event, now, rules);
        a.touched = now;
        let back = event == SpotEvent::Returned;
        match (was, a.state) {
            (SpotState::Open, SpotState::Open) | (SpotState::Down { .. }, SpotState::Down { .. }) => Change::None,
            (_, SpotState::Open) => Change::Opened,
            (_, SpotState::Down { .. }) => Change::Downed,
            (SpotState::Down { .. }, SpotState::Conceded { .. }) if back => Change::BackConceded,
            (SpotState::Down { .. }, SpotState::Tried { .. }) if back => Change::BackTried,
            (SpotState::Conceded { until: before, .. }, SpotState::Conceded { until, .. }) if until != before => Change::ConcededAgain,
            // A try while conceded that cost no life: the last try only.
            (SpotState::Conceded { .. }, _) => Change::None,
            (_, SpotState::Conceded { .. }) => Change::Conceded,
            (SpotState::Open, SpotState::Tried { .. }) => Change::Tried,
            (SpotState::Tried { .. } | SpotState::Down { .. }, SpotState::Tried { .. }) => Change::Retried,
        }
    }

    /// Shooter `shooter` died at `now`: each area of his is open again (a death of his traded or ended the try), or keeps
    /// its tries ([`ConcedeRules::keep_ms`]); what each changed.
    pub fn shooter_died(&mut self, shooter: Place, now: u32, rules: &ConcedeRules) -> Vec<Change> {
        self.areas_of(shooter).into_iter().map(|id| self.apply(id, SpotEvent::Succeeded, now, rules)).collect()
    }

    /// The areas of shooter `shooter`, in order.
    fn areas_of(&self, shooter: Place) -> Vec<AreaId> {
        self.areas.iter().filter(|a| a.shooter == Some(shooter)).map(|a| a.id).collect()
    }

    /// Shooter `shooter` was sensed at `at` at `now`: the area of his that keeps its tries since his death and holds `at`
    /// takes them up ([`SpotEvent::Returned`]); what it changed.
    pub fn shooter_sensed(&mut self, shooter: Place, at: Vec3, now: u32, rules: &ConcedeRules) -> Change {
        self.back_area(shooter, at, now).map_or(Change::None, |id| self.apply(id, SpotEvent::Returned, now, rules))
    }

    /// The area of shooter `shooter` that keeps its tries since his death at `now` and holds `at`, the nearest.
    fn back_area(&self, shooter: Place, at: Vec3, now: u32) -> Option<AreaId> {
        self.areas
            .iter()
            .filter(|a| a.shooter == Some(shooter) && matches!(settled(a.state, now), SpotState::Down { .. }))
            .filter(|a| a.at.distance_xz(at) <= AREA_M)
            .min_by(|a, b| a.at.distance_xz(at).total_cmp(&b.at.distance_xz(at)))
            .map(|a| a.id)
    }

    /// A job's failure on `area`, its bot (or the try's) approaching from `from`.
    pub fn record(&mut self, area: AreaId, kind: JobKind, why: Failed, from: Vec3, now: u32, rules: &ConcedeRules) -> Change {
        self.apply(area, SpotEvent::Failed { kind: TryKind::Job(kind), from, why }, now, rules)
    }

    /// The state of `area` at `now` (a concede that is over, or tries kept past their time, read open).
    pub fn state(&self, area: AreaId, now: u32) -> SpotState {
        self.get(area).map_or(SpotState::Open, |a| settled(a.state, now))
    }

    /// Whether `area` is conceded at `now`.
    pub fn conceded(&self, area: AreaId, now: u32) -> bool {
        matches!(self.state(area, now), SpotState::Conceded { .. })
    }

    /// The last failed try on `area`: its kind, approach bearing and time.
    pub fn last_try(&self, area: AreaId) -> Option<(TryKind, f32, u32)> {
        match self.get(area)?.state {
            SpotState::Open => None,
            SpotState::Tried { last, .. } | SpotState::Conceded { last, .. } | SpotState::Down { last, .. } => {
                Some((last.kind, last.bearing, last.at))
            }
        }
    }

    /// What may be tried on `area` at `now` (anything while its shooter is down).
    pub fn offers(&self, area: AreaId, now: u32) -> Offer {
        match self.state(area, now) {
            SpotState::Open | SpotState::Down { .. } => Offer::Any,
            SpotState::Tried { last, .. } => Offer::NotLike(last),
            SpotState::Conceded { .. } => Offer::None,
        }
    }

    /// The areas open (or whose kept tries are over) and untouched for [`FORGET_MS`] are forgotten.
    pub fn forget_old(&mut self, now: u32) {
        self.areas.retain(|a| !(matches!(settled(a.state, now), SpotState::Open) && now.saturating_sub(a.touched) >= FORGET_MS));
    }

    /// The bench's: areas conceded at `now`.
    pub fn conceded_count(&self, now: u32) -> usize {
        self.areas.iter().filter(|a| self.conceded(a.id, now)).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RULES: ConcedeRules = ConcedeRules { fails: 2, concede_ms: 100_000, retry_deg: 45.0, retry_ms: 30_000, keep_ms: None };
    /// His death keeps the tries for 120 s.
    const KEEP: ConcedeRules = ConcedeRules { keep_ms: Some(120_000), ..RULES };
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
            SpotState::Down { fails: 1, last: Try { kind: TryKind::Peek, bearing: 0.0, at: 0 }, until: 50_000 },
        ] {
            assert_eq!(step(s, SPOT, SpotEvent::Succeeded, 1_000, &RULES), SpotState::Open, "without keep_ms");
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
    fn deaths_that_find_him_concede_nothing_but_are_the_last_try() {
        let mut threat = TeamThreat::default();
        for (k, at) in [(1_000, 0.0), (2_000, 1.0), (3_000, 2.0)] {
            found(&mut threat, 0, Vec3::new(0.0, 0.0, 20.0), 7, Vec3::new(at, 0.0, 0.0), k, &RULES);
        }
        let area = threat.spot_memory.area_at(SPOT).expect("kept");
        assert!(matches!(threat.spot_memory.state(area, 3_000), SpotState::Tried { fails: 0, .. }));
        assert!(matches!(threat.spot_memory.last_try(area), Some((TryKind::Peek, _, 3_000))));
        died(&mut threat, 0, 3, Vec3::new(0.0, 0.0, 20.0), 7, SPOT, TryKind::Peek, 4_000, &RULES);
        assert!(!threat.spot_memory.conceded(area, 4_000), "one counted death");
        died(&mut threat, 0, 4, Vec3::new(20.0, 0.0, 0.0), 7, SPOT, TryKind::Peek, 5_000, &RULES);
        assert!(threat.spot_memory.conceded(area, 5_000));
    }

    #[test]
    fn a_try_into_a_tried_lane_is_like_the_last_until_its_angle_or_time_differs() {
        use crate::v026g::bots::threat::HeldLine;
        let victim = Vec3::new(0.0, 0.0, 20.0);
        let line = HeldLine::for_test(7, 9, SPOT, &[victim], Some(0));
        let last = Try { kind: TryKind::Peek, bearing: bearing(SPOT, victim), at: 10_000 };
        let picture = Picture { held: vec![line], concede: Offers { conceded: Vec::new(), tried: vec![(7, last)] }, ..Picture::default() };
        let lane = Vec3::new(0.0, 0.0, 25.0);
        let far = Vec3::new(60.0, 0.0, 60.0);
        assert!(like_last(&picture, lane, far, TryKind::Peek, 20_000, &RULES), "a point of his lane, 10 s after");
        assert!(!like_last(&picture, lane, far, TryKind::Peek, 40_000, &RULES), "30 s after");
        assert!(!like_last(&picture, lane, far, TryKind::Answer, 20_000, &RULES), "another kind");
        assert!(!like_last(&picture, far, far, TryKind::Peek, 20_000, &RULES), "not into his area or lanes");
        // At him: from where the bot stands, in the lane (like) or off it by 90 deg (not).
        assert!(like_last(&picture, Vec3::new(1.0, 0.0, 1.0), Vec3::new(0.0, 0.0, 30.0), TryKind::Peek, 20_000, &RULES));
        assert!(!like_last(&picture, Vec3::new(1.0, 0.0, 1.0), Vec3::new(30.0, 0.0, 0.0), TryKind::Peek, 20_000, &RULES));
    }

    #[test]
    fn his_death_keeps_the_tries_and_his_return_takes_them_up() {
        let last = Try { kind: TryKind::Peek, bearing: 0.0, at: 0 };
        let conceded = SpotState::Conceded { until: 50_000, last };
        // Down: anything offered; a concede kept at the rules' fails, a tried area at its own.
        let down = step(conceded, SPOT, SpotEvent::Succeeded, 10_000, &KEEP);
        assert_eq!(down, SpotState::Down { fails: 2, last, until: 130_000 });
        assert_eq!(step(SpotState::Tried { fails: 1, last }, SPOT, SpotEvent::Succeeded, 10_000, &KEEP), SpotState::Down { fails: 1, last, until: 130_000 });
        assert_eq!(step(SpotState::Open, SPOT, SpotEvent::Succeeded, 10_000, &KEEP), SpotState::Open);
        // Sensed back in it 40 s later: conceded again from then.
        assert_eq!(step(down, SPOT, SpotEvent::Returned, 50_000, &KEEP), SpotState::Conceded { until: 150_000, last });
        // Or a death before his spot is held again: conceded again, that death the last try.
        assert!(matches!(step(down, SPOT, SpotEvent::Found { from: Vec3::new(0.0, 0.0, 20.0) }, 50_000, &KEEP), SpotState::Conceded { until: 150_000, .. }));
        // A tried area: taken up as it was, the next counted death concedes.
        let once = SpotState::Down { fails: 1, last, until: 130_000 };
        assert_eq!(step(once, SPOT, SpotEvent::Returned, 50_000, &KEEP), SpotState::Tried { fails: 1, last });
        assert!(matches!(step(once, SPOT, fail(Vec3::new(10.0, 0.0, 0.0)), 50_000, &KEEP), SpotState::Conceded { .. }));
        // Kept no longer than 120 s; a return of nobody's area changes nothing.
        assert_eq!(step(down, SPOT, SpotEvent::Returned, 130_000, &KEEP), SpotState::Open);
        assert!(matches!(step(down, SPOT, fail(Vec3::new(10.0, 0.0, 0.0)), 130_000, &KEEP), SpotState::Tried { fails: 1, .. }));
        assert_eq!(step(SpotState::Tried { fails: 1, last }, SPOT, SpotEvent::Returned, 1_000, &KEEP), SpotState::Tried { fails: 1, last });
    }

    #[test]
    fn a_shooter_sensed_back_in_his_kept_area_concedes_it_again() {
        let mut threat = TeamThreat::default();
        died(&mut threat, 1, 3, Vec3::new(20.0, 0.0, 0.0), 7, SPOT, TryKind::Peek, 1_000, &KEEP);
        died(&mut threat, 1, 4, Vec3::new(0.0, 0.0, 20.0), 7, SPOT, TryKind::Peek, 5_000, &KEEP);
        let area = threat.spot_memory.area_at(SPOT).expect("kept");
        shooter_died(&mut threat, 1, 7, 7_000, &KEEP);
        assert_eq!(threat.spot_memory.offers(area, 7_000), Offer::Any, "he is down");
        let m = &mut threat.spot_memory;
        assert_eq!(m.shooter_sensed(8, SPOT, 40_000, &KEEP), Change::None, "another enemy there");
        assert_eq!(m.shooter_sensed(7, Vec3::new(30.0, 0.0, 0.0), 40_000, &KEEP), Change::None, "he is elsewhere");
        assert_eq!(m.shooter_sensed(7, Vec3::new(3.0, 0.0, 0.0), 48_000, &KEEP), Change::BackConceded);
        assert_eq!(m.offers(area, 48_000), Offer::None);
        assert_eq!(m.shooter_sensed(7, SPOT, 49_000, &KEEP), Change::None, "taken up once");
        // His next death keeps them again; nobody sees him for 120 s: open.
        assert_eq!(m.shooter_died(7, 60_000, &KEEP), vec![Change::Downed]);
        assert_eq!(m.state(area, 180_000), SpotState::Open);
        assert_eq!(m.shooter_sensed(7, SPOT, 180_000, &KEEP), Change::None);
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
