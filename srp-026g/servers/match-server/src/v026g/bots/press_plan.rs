//! [press forecast] When a route jump is pressed, by the body model's own forecast ([`super::body::press_forecast`],
//! `Feel::press_forecast`): a plan made on one tick from the body as reckoned, its candidate ticks, the order its flights are
//! flown in, and the press rule. Pure: the motor gives it the ground chain ([`super::body::press_chain`]) and each flight's
//! outcome, and asks it on each tick whether this tick is the press.
//!
//! A plan's tick `j` is the motor's tick `lock + j`; `chain[j]` is the body as reckoned before that tick's decision, walking
//! with the plan's keys and no turn from the lock on. A press on tick `j` is the forecast from `chain[j]` with no ground tick
//! before it.

use mapdata::Vec3;

use super::body::{Estimate, PressOutcome};
use super::kin::wrap;

/// [press forecast] What a candidate's flight says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Verdict {
    /// Up on the landing and staying ([`PressOutcome::Lands`]).
    Lands,
    /// Short, over, still in the air, or no flight at all.
    Misses,
}

impl Verdict {
    pub(super) fn of(outcome: PressOutcome) -> Self {
        match outcome {
            PressOutcome::Lands { .. } => Self::Lands,
            PressOutcome::Short { .. } | PressOutcome::Over { .. } | PressOutcome::NotDown { .. } | PressOutcome::NoFlight => Self::Misses,
        }
    }
}

/// [press forecast] What a plan is for: which ticks are candidates and how one is chosen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Ask {
    /// A body walking in: the ticks at which its feet are in the press zone (`zone`, m before the take-off along the line, the
    /// near end first), every second one flown first, then the ticks beside each landing one; the middle of the longest run of
    /// landing ticks is pressed (design §2.4-§2.5).
    Moving { zone: (f32, f32) },
    /// A body at rest: forward held for 0 to [`REST_STEPS_MAX`] ticks and pressed on the next, each flown; the first that lands
    /// with a landing neighbour is pressed (the builder's slow press, `jump::Kind::Slow`, from the body's real rest; §3.4). Its
    /// ticks count from the walk-up's first tick ([`Plan::start`]), not from the ask: the body stands while it is flown.
    Rest,
}

/// [press forecast] The most ground ticks a press from rest is walked up over before it: the builder's slow presses
/// (`jump::Kind::Slow(k)`, k up to 5, `mapdata/controller/jump.rs` `run_up_slow`).
pub(super) const REST_STEPS_MAX: u32 = 5;

/// [press forecast] How a chosen tick's neighbours stand ([`Choice::Press`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Margin {
    /// In a run of `run` landing ticks, three or more: the ticks before and after it land too (a press one input early or late).
    Run { run: u32 },
    /// In a run of one or two landing ticks, pressed only because no longer run is there (counted, logged).
    Narrow { run: u32 },
}

impl Margin {
    /// The run's length.
    pub(super) fn run(self) -> u32 {
        match self {
            Margin::Run { run } | Margin::Narrow { run } => run,
        }
    }
}

/// [press forecast] What a plan says on a tick ([`Plan::decide`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Choice {
    /// Flights to fly yet before the press tick is known.
    Pending,
    /// Press on the plan's tick `j`.
    Press { j: u32, margin: Margin },
    /// No candidate tick from now on lands: `flown` flights flew, `unflown` candidates passed before they were flown.
    NoLanding { flown: u32, unflown: u32 },
}

/// [press forecast] A plan: the chain from the lock, its candidates and their flights.
#[derive(Clone, Debug)]
pub(super) struct Plan {
    /// The motor's tick of `chain[0]`.
    lock: u32,
    ask: Ask,
    chain: Vec<Estimate>,
    /// The first candidate tick; `outcomes[i]` is tick `first + i`'s flight (none: not flown).
    first: u32,
    outcomes: Vec<Option<PressOutcome>>,
}

impl Plan {
    /// The plan of `ask` from the chain `chain` made on the motor's tick `lock`; `before(p)`: how far `p` is before the take-off
    /// along the line, m. None when no tick of the chain is a candidate (a moving body whose chain never enters the zone).
    pub(super) fn new(lock: u32, ask: Ask, chain: Vec<Estimate>, before: impl Fn(Vec3) -> f32) -> Option<Self> {
        let ticks = chain.len() as u32;
        let (first, last) = match ask {
            Ask::Moving { zone: (near, far) } => {
                let inside = |j: u32| {
                    let b = before(chain[j as usize].feet);
                    (near..=far).contains(&b)
                };
                let first = (0..ticks).find(|&j| inside(j))?;
                let last = (first..ticks).take_while(|&j| inside(j)).last()?;
                (first, last)
            }
            Ask::Rest => (0, REST_STEPS_MAX.min(ticks.checked_sub(1)?)),
        };
        Some(Self { lock, ask, chain, first, outcomes: vec![None; (last - first + 1) as usize] })
    }

    pub(super) fn lock(&self) -> u32 {
        self.lock
    }

    /// [`Ask::Rest`]: the walk-up starts on the motor's tick `now`, so its tick 0 is `now` (the body stood still since the ask:
    /// the chain from it still holds, [`same_body`] checks it).
    pub(super) fn start(&mut self, now: u32) {
        self.lock = now;
    }

    pub(super) fn ask(&self) -> Ask {
        self.ask
    }

    /// The plan's tick of the motor's tick `now` (none before the lock).
    fn tick(&self, now: u32) -> Option<u32> {
        now.checked_sub(self.lock)
    }

    /// The body the chain has before the decision of the motor's tick `now`; none past the chain.
    pub(super) fn expected(&self, now: u32) -> Option<&Estimate> {
        self.chain.get(self.tick(now)? as usize)
    }

    /// The body the chain has before plan tick `j`: where a candidate's flight starts.
    pub(super) fn at(&self, j: u32) -> &Estimate {
        &self.chain[j as usize]
    }

    /// Tick `j`'s flight, if flown.
    pub(super) fn outcome(&self, j: u32) -> Option<PressOutcome> {
        j.checked_sub(self.first).and_then(|i| self.outcomes.get(i as usize).copied().flatten())
    }

    fn verdict(&self, j: u32) -> Option<Verdict> {
        self.outcome(j).map(Verdict::of)
    }

    fn candidate(&self, j: u32) -> bool {
        j >= self.first && ((j - self.first) as usize) < self.outcomes.len()
    }

    /// The candidates: the first tick and how many.
    #[cfg(test)]
    pub(super) fn candidates(&self) -> (u32, u32) {
        (self.first, self.outcomes.len() as u32)
    }

    /// The next candidate to fly on the motor's tick `now`: one not flown and not past. A moving plan flies every second
    /// candidate first, then the candidates beside each landing one; a plan from rest flies them in order.
    pub(super) fn next_flight(&self, now: u32) -> Option<u32> {
        // A plan from rest is flown before its walk-up starts: none of its ticks is past.
        let from = match self.ask {
            Ask::Rest => 0,
            Ask::Moving { .. } => self.tick(now)?,
        };
        let open = |j: &u32| *j >= from && self.candidate(*j) && self.verdict(*j).is_none();
        let last = self.first + self.outcomes.len() as u32 - 1;
        match self.ask {
            Ask::Rest => (self.first..=last).find(open),
            Ask::Moving { .. } => (self.first..=last).step_by(2).find(open).or_else(|| {
                (self.first..=last)
                    .filter(|&j| self.verdict(j) == Some(Verdict::Lands))
                    .flat_map(|j| [j.checked_sub(1), Some(j + 1)])
                    .flatten()
                    .find(open)
            }),
        }
    }

    /// Tick `j`'s flight came out as `outcome`.
    pub(super) fn record(&mut self, j: u32, outcome: PressOutcome) {
        if let Some(slot) = j.checked_sub(self.first).and_then(|i| self.outcomes.get_mut(i as usize)) {
            *slot = Some(outcome);
        }
    }

    /// For the log: the candidate ticks and each flight's verdict (L lands, S short, O over, A still in the air, N no flight, .
    /// not flown), e.g. `t8-16 S.L.LLL.S`.
    pub(super) fn text(&self) -> String {
        let marks: String = self
            .outcomes
            .iter()
            .map(|outcome| match outcome {
                None => '.',
                Some(PressOutcome::Lands { .. }) => 'L',
                Some(PressOutcome::Short { .. }) => 'S',
                Some(PressOutcome::Over { .. }) => 'O',
                Some(PressOutcome::NotDown { .. }) => 'A',
                Some(PressOutcome::NoFlight) => 'N',
            })
            .collect();
        format!("t{}-{} {marks}", self.first, self.first + self.outcomes.len() as u32 - 1)
    }

    /// The flights recorded.
    pub(super) fn flown(&self) -> u32 {
        self.outcomes.iter().filter(|v| v.is_some()).count() as u32
    }

    /// What the plan says on the motor's tick `now`. A candidate not flown counts as a miss once no flight is left to fly
    /// from `now` on: nothing is pressed that was not flown.
    pub(super) fn decide(&self, now: u32) -> Choice {
        let from = match self.ask {
            Ask::Rest => 0,
            Ask::Moving { .. } => match self.tick(now) {
                Some(from) => from,
                None => return Choice::Pending,
            },
        };
        let lands = |j: u32| self.verdict(j) == Some(Verdict::Lands);
        let last = self.first + self.outcomes.len() as u32 - 1;
        // Decided before every flight is flown, to fly fewer: from rest, the first step that lands with the next landing too (the
        // steps are flown in order, so no earlier step qualifies); walking in, the first run of three or more landing ticks whose
        // ends are known (a miss or the zone's end on each side), pressed at its middle.
        match self.ask {
            Ask::Rest => {
                if let Some(j) = (self.first..last).find(|&j| lands(j) && lands(j + 1)) {
                    let run = (j..=last).take_while(|&k| lands(k)).count() as u32;
                    return Choice::Press { j, margin: if run >= 3 { Margin::Run { run } } else { Margin::Narrow { run } } };
                }
            }
            Ask::Moving { .. } => {
                let known_end = |j: Option<u32>| j.is_none_or(|j| j > last || self.verdict(j) == Some(Verdict::Misses));
                let mut j = self.first;
                while j <= last {
                    if !lands(j) {
                        j += 1;
                        continue;
                    }
                    let start = j;
                    while j <= last && lands(j) {
                        j += 1;
                    }
                    let len = j - start;
                    let bounded = (start == self.first || known_end(start.checked_sub(1))) && known_end(Some(j));
                    let middle = start + (len - 1) / 2;
                    if len >= 3 && bounded && middle >= from {
                        return Choice::Press { j: middle, margin: Margin::Run { run: len } };
                    }
                }
            }
        }
        if self.next_flight(now).is_some() {
            return Choice::Pending;
        }
        let chosen = match self.ask {
            Ask::Rest => (self.first..=last).filter(|&j| j >= from).find(|&j| {
                lands(j) && (j.checked_sub(1).is_some_and(lands) || lands(j + 1))
            }).map(|j| {
                let run = (j..=last).take_while(|&k| lands(k)).count() as u32 + (self.first..j).rev().take_while(|&k| lands(k)).count() as u32;
                (j, run)
            }),
            Ask::Moving { .. } => {
                // The runs of landing ticks, (start, length); a run's press is its middle tick (the earlier of two).
                let mut runs: Vec<(u32, u32)> = Vec::new();
                for j in self.first..=last {
                    if lands(j) {
                        match runs.last_mut() {
                            Some((start, len)) if *start + *len == j => *len += 1,
                            _ => runs.push((j, 1)),
                        }
                    }
                }
                runs.iter()
                    .map(|&(start, len)| (start + (len - 1) / 2, len))
                    .filter(|&(middle, _)| middle >= from)
                    .fold(None, |best: Option<(u32, u32)>, (middle, len)| match best {
                        Some((_, best_len)) if best_len >= len => best,
                        _ => Some((middle, len)),
                    })
            }
        };
        match chosen {
            Some((j, run)) if run >= 3 => Choice::Press { j, margin: Margin::Run { run } },
            Some((j, run)) => Choice::Press { j, margin: Margin::Narrow { run } },
            None => {
                let flown = self.flown();
                Choice::NoLanding { flown, unflown: self.outcomes.len() as u32 - flown }
            }
        }
    }
}

/// [press forecast] The most the body may be off the chain's place on a tick, m, flat and in height (design §2.5 rule 5).
pub(super) const SAME_FEET_M: f32 = 0.001;
/// … and off its velocity, m/s.
pub(super) const SAME_VELOCITY_MS: f32 = 0.001;
/// … and off its yaw, rad.
pub(super) const SAME_YAW_RAD: f32 = 1e-4;

/// [press forecast] Whether the body as reckoned now is where the chain put it (design §2.5 rule 5): the feet within
/// [`SAME_FEET_M`] flat and in height, the velocity within [`SAME_VELOCITY_MS`], the yaw within [`SAME_YAW_RAD`] and the keys
/// held the same. With the same keys the reckoning runs the chain's own steps, so only a world's fix, a hit, a push or another
/// module's keys break it; the plan is then made again from the body as it is.
pub(super) fn same_body(now: &Estimate, expected: &Estimate) -> bool {
    let flat = (now.feet.x - expected.feet.x).hypot(now.feet.z - expected.feet.z);
    let velocity = (now.velocity[0] - expected.velocity[0]).hypot(now.velocity[1] - expected.velocity[1]);
    let yaw = wrap(now.yaw - expected.yaw).abs();
    flat <= SAME_FEET_M
        && (now.feet.y - expected.feet.y).abs() <= SAME_FEET_M
        && velocity <= SAME_VELOCITY_MS
        && yaw <= SAME_YAW_RAD
        && now.keys == expected.keys
        && now.arc.is_none()
        && expected.arc.is_none()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A body on the line at `x` m (the take-off at x 10, the way +x), at rest.
    fn body(x: f32) -> Estimate {
        Estimate::at_rest(Vec3::new(x, 0.0, 0.0), 0.0)
    }

    fn before(p: Vec3) -> f32 {
        10.0 - p.x
    }

    /// A flight that lands or comes down short.
    fn flight(lands: bool) -> PressOutcome {
        if lands { PressOutcome::Lands { rest: Vec3::new(11.0, 0.0, 0.0) } } else { PressOutcome::Short { rest: Vec3::new(10.5, -1.0, 0.0) } }
    }

    /// The zone, m before the take-off: x 8.35-9.85, ticks 8-16 of [`chain`] (off its points, so no tick is on an end).
    const ZONE: (f32, f32) = (0.15, 1.65);

    /// A chain of `n` ticks, 0.175 m a tick from x 7.
    fn chain(n: usize) -> Vec<Estimate> {
        (0..n).map(|j| body(7.0 + 0.175 * j as f32)).collect()
    }

    /// Flies every flight the plan asks for on its ticks from `lock`, one a tick, with `lands(j)` as the flight's verdict;
    /// returns the plan's choice once it is not pending (or at the chain's end) and the flights flown.
    fn run(mut plan: Plan, lands: impl Fn(u32) -> bool) -> (Choice, u32) {
        let lock = plan.lock();
        for now in lock..lock + 40 {
            if let Some(j) = plan.next_flight(now) {
                plan.record(j, flight(lands(j)));
            }
            match plan.decide(now) {
                Choice::Pending => {}
                choice => return (choice, plan.flown()),
            }
        }
        (plan.decide(lock + 40), plan.flown())
    }

    /// [press forecast] The zone's ticks are the chain's ticks with the feet in it: 1.65-0.15 m before the take-off is x
    /// 8.35-9.85, ticks 8-16 of a chain from x 7 at 0.175 m a tick.
    #[test]
    fn a_moving_plans_candidates_are_the_chains_ticks_in_the_zone() {
        let plan = Plan::new(100, Ask::Moving { zone: ZONE }, chain(20), before).unwrap();
        assert_eq!(plan.candidates(), (8, 9));
        assert!(Plan::new(100, Ask::Moving { zone: ZONE }, chain(5), before).is_none(), "a chain that never gets there");
    }

    /// [press forecast] Every second candidate first, then the neighbours of the landing ones; the middle of a run of three
    /// or more is pressed (rule 2-3), and a run's ends are flown so its length is known.
    #[test]
    fn a_moving_plan_presses_the_middle_of_the_longest_run() {
        let plan = Plan::new(100, Ask::Moving { zone: ZONE }, chain(20), before).unwrap();
        // Ticks 10-14 land (a run of 5), 16 alone.
        let (choice, flown) = run(plan, |j| (10..=14).contains(&j) || j == 16);
        assert_eq!(choice, Choice::Press { j: 12, margin: Margin::Run { run: 5 } });
        assert!(flown <= 9, "{flown}");
    }

    /// [press forecast] A run of two (map 91's forecast from a walk-in: ticks 6 and 7) is pressed only with no longer run, as
    /// a narrow one, on its earlier tick; a run of three wins over it.
    #[test]
    fn a_narrow_run_is_pressed_only_with_no_longer_one() {
        let plan = Plan::new(100, Ask::Moving { zone: ZONE }, chain(20), before).unwrap();
        let (choice, _) = run(plan.clone(), |j| j == 10 || j == 11);
        assert_eq!(choice, Choice::Press { j: 10, margin: Margin::Narrow { run: 2 } });
        let (choice, _) = run(plan, |j| j == 9 || j == 10 || (13..=15).contains(&j));
        assert_eq!(choice, Choice::Press { j: 14, margin: Margin::Run { run: 3 } });
    }

    /// [press forecast] No candidate lands: no press, with the flights counted.
    #[test]
    fn a_plan_with_no_landing_tick_says_so() {
        let plan = Plan::new(100, Ask::Moving { zone: ZONE }, chain(20), before).unwrap();
        let (choice, flown) = run(plan, |_| false);
        assert_eq!(choice, Choice::NoLanding { flown: 5, unflown: 4 });
        assert_eq!(flown, 5, "every second of 9 candidates");
    }

    /// [press forecast] A candidate whose tick passed before it was flown is a miss: a plan whose flights start late flies
    /// only ticks to come, and presses nothing it did not fly (ticks 12-14 land; flights from plan tick 13 on find 14 landing
    /// and 15 not, and by then 14 is past).
    #[test]
    fn a_candidate_not_flown_in_time_is_never_pressed() {
        let mut plan = Plan::new(100, Ask::Moving { zone: ZONE }, chain(20), before).unwrap();
        for now in 113..140 {
            if let Some(j) = plan.next_flight(now) {
                assert!(j >= now - 100, "a past tick flown: {j} at {now}");
                plan.record(j, flight((12..=14).contains(&j)));
            }
            match plan.decide(now) {
                Choice::Pending => {}
                choice => {
                    assert_eq!(choice, Choice::NoLanding { flown: 3, unflown: 6 }, "at {now}");
                    return;
                }
            }
        }
        panic!("never decided");
    }

    /// [press forecast] From rest: the steps 0-5 in order; the first that lands with the next landing too is pressed (decided
    /// then), one that lands alone is not.
    #[test]
    fn a_rest_plan_presses_the_first_landing_step_with_a_landing_neighbour() {
        let plan = Plan::new(100, Ask::Rest, vec![body(9.5); 7], before).unwrap();
        assert_eq!(plan.candidates(), (0, 6));
        let (choice, flown) = run(plan.clone(), |j| j == 1 || j == 3 || j == 4);
        assert_eq!(choice, Choice::Press { j: 3, margin: Margin::Narrow { run: 2 } });
        assert_eq!(flown, 5, "decided at the second landing step in a row: step 5 is not flown");
        let (choice, _) = run(plan, |j| j == 2);
        assert_eq!(choice, Choice::NoLanding { flown: 6, unflown: 0 });
    }

    /// [press forecast] Rule 5: the body where the chain put it, to the millimetre, with the same keys; anything else is not.
    #[test]
    fn the_body_is_the_chains_only_to_the_millimetre_with_the_same_keys() {
        let a = Estimate { keys: 1, velocity: [3.5, 0.0], ..body(9.0) };
        assert!(same_body(&a, &a));
        assert!(same_body(&Estimate { feet: a.feet + Vec3::new(0.0009, 0.0, 0.0), ..a }, &a));
        assert!(!same_body(&Estimate { feet: a.feet + Vec3::new(0.0, 0.0, 0.002), ..a }, &a), "pushed sideways");
        assert!(!same_body(&Estimate { velocity: [3.49, 0.0], ..a }, &a), "slowed");
        assert!(!same_body(&Estimate { keys: 3, ..a }, &a), "other keys");
        assert!(!same_body(&Estimate { yaw: a.yaw + 0.01, ..a }, &a), "turned");
    }
}
