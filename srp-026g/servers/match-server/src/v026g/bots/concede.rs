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

use super::threat::{AREA_M, Role};
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

/// What was tried: a team job, or a lone bot's own peek (item 15).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TryKind {
    Job(JobKind),
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

    /// `event` on `area` at `now`.
    pub fn apply(&mut self, area: AreaId, event: SpotEvent, now: u32, rules: &ConcedeRules) {
        if let Some(a) = self.areas.iter_mut().find(|a| a.id == area) {
            a.state = step(a.state, a.at, event, now, rules);
            a.touched = now;
        }
    }

    /// A job's failure on `area`, its bot (or the try's) approaching from `from`.
    pub fn record(&mut self, area: AreaId, kind: JobKind, why: Failed, from: Vec3, now: u32, rules: &ConcedeRules) {
        self.apply(area, SpotEvent::Failed { kind: TryKind::Job(kind), from, why }, now, rules);
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
