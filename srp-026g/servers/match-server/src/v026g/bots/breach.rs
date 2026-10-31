//! The Breach job on a held spot (K2, `design-p1c.md` §1): when a held spot has no flank spot, or a Flank on it failed, a pinner
//! keeps its shooter busy from cover out of his lanes while two crossers gather out of his lanes, wait for each other, go on one
//! tick to cover points picked before the go, from two bearings, and the second keeps the spot in view to trade the first.
//!
//! The live humans' numbers it takes (`fx-gear-breach-human-note.md`, 478 ended campers in 402 recordings of 12+ players): two
//! comers is the common group push; half go within 5 s of each other; 84 % from two or more 45-degree sectors, the nearest pair a
//! median 12 m apart (P25 7 m); the camper dies a median 8 s after his last kill, and a pusher he kills is traded a median 4 s later.
//!
//! One state a phase ([`Phase`]) and one transition function ([`Breach::step`]) from what the team sees this tick ([`Facts`]).

use mapdata::Vec3;

use super::threat::Holder;
use super::view::Place;

/// The crossers of a Breach: two (humans' group push; with the pinner, the three bots a spot at most, §4).
pub const CROSSERS: usize = 2;

/// A Breach's timing and spacing (`ThreatTuning::breach`): the humans' numbers by default.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BreachTuning {
    /// The crossers wait for each other at their gather points this long at most, ms: the camper's median life after his
    /// last kill (8 s); a group not there by then fails ([`Failed::GatherTimedOut`]), it does not trickle.
    pub gather_ms: u32,
    /// A crosser walks from its gather point to its cover point in this at most, ms (a 3-5 s rush, FM 3-21.8, and the walk to it).
    pub go_ms: u32,
    /// At the cover points the job holds this long for the camper, ms, then ends ([`Failed::Time`]).
    pub hold_ms: u32,
    /// A crosser's death is traded when the camper dies within this, ms (the stat tools' window; humans' median 4 s).
    pub trade_ms: u32,
    /// The two cover points are at least this far apart, m (humans' nearest pair P25 7 m) ...
    pub apart_min_m: f32,
    /// ... and at most this far (their median 12 m).
    pub apart_max_m: f32,
    /// Seen from the spot, the two cover points' bearings differ by at least this, degrees (84 % of humans' pushes came from two
    /// 45-degree sectors or more).
    pub bearing_min_deg: f32,
}

impl Default for BreachTuning {
    fn default() -> Self {
        Self { gather_ms: 8_000, go_ms: 6_000, hold_ms: 10_000, trade_ms: 5_000, apart_min_m: 6.0, apart_max_m: 12.0, bearing_min_deg: 30.0 }
    }
}

/// A crosser: its bot, where it gathers (out of his lanes and his sight) and the cover point it goes to (out of his lanes, his
/// spot in sight), both picked when the job opened.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Crosser {
    pub holder: Holder,
    pub gather: Vec3,
    pub cover: Vec3,
}

/// Why a Breach failed: what the team's failure memory for the spot counts (§3, §11).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failed {
    /// A crosser or the pinner went down before the go, or both crossers went down after it.
    HolderDown,
    /// A crosser went down and the camper did not die within the trade window.
    Untraded,
    /// The crossers were not both at their gather points within `gather_ms`.
    GatherTimedOut,
    /// The team's objective state got worse while it ran (§16).
    ObjectiveWorse,
    /// It held `hold_ms` at the cover points and the camper lived.
    Time,
}

impl std::fmt::Display for Failed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Failed::HolderDown => "a role's bot went down",
            Failed::Untraded => "a crosser went down untraded",
            Failed::GatherTimedOut => "the crossers did not gather in time",
            Failed::ObjectiveWorse => "the objective got worse",
            Failed::Time => "held its time, he lived",
        })
    }
}

/// How a Breach ended well, or at least not as a failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Done {
    /// Its shooter died: by one of the job's bots (`Some`), or by someone else.
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

/// What a crosser's bot is doing this tick, as the team sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CrosserNow {
    /// Down, or no longer a free bot of the team (a carrier, dry): its role is lost.
    Lost,
    /// On its way, or at neither point.
    Moving,
    AtGather,
    AtCover,
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
    pub pinner_lost: bool,
    pub shooter: Shooter,
    /// The team's objective state is worse than when the job opened (§16).
    pub objective_worse: bool,
}

/// One phase change, for the notes and the counts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Step {
    /// Both crossers gathered after `waited_ms`: the go.
    Went { waited_ms: u32 },
    /// The crossers came to their cover points (or the go's time ran) after `took_ms`.
    Holding { took_ms: u32 },
    /// A crosser went down at `at` (the first).
    FirstDeath { at: u32 },
    Ended(Phase),
}

/// The Breach job on spot `spot` (its shooter's place `shooter`).
#[derive(Clone, Debug, PartialEq)]
pub struct Breach {
    pub spot: u32,
    pub shooter: Place,
    /// Its lanes cover an objective area: objective work, outside the team's cap (§4, §13).
    pub objective: bool,
    pub since: u32,
    pub pinner: Holder,
    pub crossers: [Crosser; CROSSERS],
    pub phase: Phase,
}

impl Breach {
    /// A new job: the crossers start for their gather points.
    pub fn open(spot: u32, shooter: Place, objective: bool, pinner: Holder, crossers: [Crosser; CROSSERS], now: u32, t: &BreachTuning) -> Self {
        Self { spot, shooter, objective, since: now, pinner, crossers, phase: Phase::Gather { until: now + t.gather_ms } }
    }

    /// The job's role of `place`, if it has one.
    pub fn role_of(&self, place: Place) -> Option<BreachRole> {
        if self.pinner.place == place {
            return Some(BreachRole::Pinner);
        }
        self.crossers.iter().position(|c| c.holder.place == place).map(BreachRole::Crosser)
    }

    /// The bots it holds.
    pub fn places(&self) -> impl Iterator<Item = Place> + '_ {
        std::iter::once(self.pinner.place).chain(self.crossers.iter().map(|c| c.holder.place))
    }

    /// The transition: the phase from what the team sees at `now`, and what changed. An ended job stays ended.
    pub fn step(&mut self, facts: &Facts, now: u32, t: &BreachTuning) -> Vec<Step> {
        let mut steps = Vec::new();
        let next = self.next(facts, now, t, &mut steps);
        if next != self.phase {
            if next.ended() {
                steps.push(Step::Ended(next));
            }
            self.phase = next;
        }
        steps
    }

    fn next(&self, facts: &Facts, now: u32, t: &BreachTuning, steps: &mut Vec<Step>) -> Phase {
        if self.phase.ended() {
            return self.phase;
        }
        match facts.shooter {
            Shooter::Died { by_job } => return Phase::Done { at: now, done: Done::ShooterDied { by_job } },
            Shooter::Unheld => return Phase::Done { at: now, done: Done::Unheld },
            Shooter::Holds => {}
        }
        if facts.objective_worse {
            return Phase::Failed { at: now, why: Failed::ObjectiveWorse };
        }
        let lost = facts.crossers.iter().filter(|c| **c == CrosserNow::Lost).count();
        match self.phase {
            Phase::Gather { until } => {
                if lost > 0 || facts.pinner_lost {
                    Phase::Failed { at: now, why: Failed::HolderDown }
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
                let first_death = (lost > 0).then_some(now);
                if let Some(at) = first_death {
                    steps.push(Step::FirstDeath { at });
                }
                if lost == CROSSERS {
                    Phase::Failed { at: now, why: Failed::HolderDown }
                } else if lost > 0 || facts.crossers.iter().all(|c| *c == CrosserNow::AtCover) || now >= at + t.go_ms {
                    steps.push(Step::Holding { took_ms: now.saturating_sub(at) });
                    Phase::Hold { since: now, first_death }
                } else {
                    self.phase
                }
            }
            Phase::Hold { since, first_death } => {
                let first_death = first_death.or_else(|| {
                    (lost > 0).then(|| {
                        steps.push(Step::FirstDeath { at: now });
                        now
                    })
                });
                if lost == CROSSERS {
                    Phase::Failed { at: now, why: Failed::HolderDown }
                } else if first_death.is_some_and(|d| now >= d + t.trade_ms) {
                    Phase::Failed { at: now, why: Failed::Untraded }
                } else if now >= since + t.hold_ms {
                    Phase::Failed { at: now, why: Failed::Time }
                } else {
                    Phase::Hold { since, first_death }
                }
            }
            ended => ended,
        }
    }
}

/// A role of a Breach.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BreachRole {
    /// Fires at his spot from cover out of his lanes (Crossfire's pin), and throws its frag or smoke on the go.
    Pinner,
    /// Crosser 0 or 1.
    Crosser(usize),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job(now: u32) -> Breach {
        let h = |place| Holder { place, since: now };
        let c = |place, x| Crosser { holder: h(place), gather: Vec3::new(x, 0.0, 0.0), cover: Vec3::new(x, 0.0, 10.0) };
        Breach::open(4, 9, false, h(1), [c(2, 0.0), c(3, 8.0)], now, &BreachTuning::default())
    }

    fn facts(a: CrosserNow, b: CrosserNow) -> Facts {
        Facts { crossers: [a, b], pinner_lost: false, shooter: Shooter::Holds, objective_worse: false }
    }

    use CrosserNow::*;

    /// [Breach] Gather -> Go when both are at their gather points; Go -> Hold at the cover points; the camper's death by the job
    /// ends it Done; the counts' steps on the way.
    #[test]
    fn a_breach_gathers_goes_on_one_tick_holds_and_ends_with_the_camper() {
        let t = BreachTuning::default();
        let mut b = job(1_000);
        assert!(b.step(&facts(AtGather, Moving), 2_000, &t).is_empty());
        assert_eq!(b.phase, Phase::Gather { until: 9_000 });
        assert_eq!(b.step(&facts(AtGather, AtGather), 4_000, &t), vec![Step::Went { waited_ms: 3_000 }]);
        assert_eq!(b.phase, Phase::Go { at: 4_000 });
        assert_eq!(b.step(&facts(AtCover, AtCover), 7_000, &t), vec![Step::Holding { took_ms: 3_000 }]);
        assert_eq!(b.phase, Phase::Hold { since: 7_000, first_death: None });
        let died = Facts { shooter: Shooter::Died { by_job: true }, ..facts(AtCover, AtCover) };
        let end = Phase::Done { at: 9_000, done: Done::ShooterDied { by_job: true } };
        assert_eq!(b.step(&died, 9_000, &t), vec![Step::Ended(end)]);
        // Ended stays ended.
        assert!(b.step(&facts(Lost, Lost), 10_000, &t).is_empty());
        assert_eq!(b.phase, end);
    }

    /// [Breach] The fails, each with its reason: no gather in time, a role lost before the go, a crosser untraded, both down, the
    /// hold's time, the objective worse; an unheld spot is Done, not a failure.
    #[test]
    fn a_breach_fails_with_its_reason() {
        let t = BreachTuning::default();
        let failed = |b: &Breach| match b.phase {
            Phase::Failed { why, .. } => Some(why),
            _ => None,
        };
        let mut b = job(0);
        b.step(&facts(AtGather, Moving), 8_000, &t);
        assert_eq!(failed(&b), Some(Failed::GatherTimedOut));
        let mut b = job(0);
        b.step(&facts(AtGather, Lost), 1_000, &t);
        assert_eq!(failed(&b), Some(Failed::HolderDown));
        let mut b = job(0);
        b.step(&Facts { pinner_lost: true, ..facts(Moving, Moving) }, 1_000, &t);
        assert_eq!(failed(&b), Some(Failed::HolderDown));
        // Go: the first crosser down -> Hold with its death; untraded after trade_ms.
        let mut b = job(0);
        b.step(&facts(AtGather, AtGather), 1_000, &t);
        let steps = b.step(&facts(Lost, Moving), 2_000, &t);
        assert_eq!(steps, vec![Step::FirstDeath { at: 2_000 }, Step::Holding { took_ms: 1_000 }]);
        assert_eq!(b.phase, Phase::Hold { since: 2_000, first_death: Some(2_000) });
        b.step(&facts(Lost, AtCover), 6_999, &t);
        assert!(!b.phase.ended());
        b.step(&facts(Lost, AtCover), 7_000, &t);
        assert_eq!(failed(&b), Some(Failed::Untraded));
        // Traded: the camper dies within the window.
        let mut b = job(0);
        b.step(&facts(AtGather, AtGather), 1_000, &t);
        b.step(&facts(Lost, Moving), 2_000, &t);
        b.step(&Facts { shooter: Shooter::Died { by_job: true }, ..facts(Lost, AtCover) }, 5_000, &t);
        assert_eq!(b.phase, Phase::Done { at: 5_000, done: Done::ShooterDied { by_job: true } });
        // Both down at once in Go.
        let mut b = job(0);
        b.step(&facts(AtGather, AtGather), 1_000, &t);
        b.step(&facts(Lost, Lost), 2_000, &t);
        assert_eq!(failed(&b), Some(Failed::HolderDown));
        // The go's time runs: Hold, then its time.
        let mut b = job(0);
        b.step(&facts(AtGather, AtGather), 1_000, &t);
        b.step(&facts(Moving, AtCover), 7_000, &t);
        assert_eq!(b.phase, Phase::Hold { since: 7_000, first_death: None });
        b.step(&facts(AtCover, AtCover), 17_000, &t);
        assert_eq!(failed(&b), Some(Failed::Time));
        // The objective worse, in any phase before the end.
        let mut b = job(0);
        b.step(&Facts { objective_worse: true, ..facts(Moving, Moving) }, 500, &t);
        assert_eq!(failed(&b), Some(Failed::ObjectiveWorse));
        // Unheld: Done.
        let mut b = job(0);
        b.step(&Facts { shooter: Shooter::Unheld, ..facts(Moving, Moving) }, 500, &t);
        assert_eq!(b.phase, Phase::Done { at: 500, done: Done::Unheld });
    }

    #[test]
    fn a_role_is_found_by_its_bot() {
        let b = job(0);
        assert_eq!(b.role_of(1), Some(BreachRole::Pinner));
        assert_eq!(b.role_of(3), Some(BreachRole::Crosser(1)));
        assert_eq!(b.role_of(7), None);
        assert_eq!(b.places().collect::<Vec<_>>(), vec![1, 2, 3]);
    }
}
