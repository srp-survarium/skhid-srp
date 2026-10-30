//! The stand-in camper's trigger, from the live human snipers (`SRP_BENCH_CAMPER_SNIPER=human|top`, A182,
//! `HUMAN-SNIPERS.md`): the reflex fires on sight, which no live sniper does. 190 live recordings (2026-10-16..22) give, for
//! the enemies that come into a holding sniper's sight line (an entry: first seen after 3 s unseen), a first aimed shot that
//! is slow and often never comes: median 2.75 s after the entry, a shot at 23 % of the entries (36 % when the sniper is
//! already looking that way, 11 % when he looks 15-60 degrees off). Fitted to those counts with the time the enemy stayed in
//! the line (`fit_trigger.txt`), the slowness is a long delay that most draws win, not a choice to leave most enemies be: a
//! delay drawn from a log-normal (the enemy goes before it is over: no shot), by where the sniper looks:
//!
//! | | share that engage | median delay | spread (log) |
//! |---|---|---|---|
//! | `human`, looking within 15 deg | 100 % | 6.0 s | 1.40 |
//! | `human`, looking 15 deg or more away | 77 % | 14.8 s | 1.00 |
//! | `top` (the accounts in the top quarter by kill share), looking | 98 % | 4.0 s | 1.50 |
//! | `top`, away | 88 % | 9.0 s | 0.90 |
//!
//! The trigger holds the shot back ([`Trigger::lets_fire`]) until the delay of the crossing it drew is over; the aim, the
//! hit and the rest are the reflex's own. A target gone for 3 s ends a crossing; back inside that time it is the same one.

use super::*;

/// A target unseen this long, ms, makes the next one a new crossing (the human rule's 3 s unexposed).
const GAP_MS: u32 = 3_000;
/// The sniper looks at the enemy when his view is within this, degrees (the human rule's 15).
const LOOKING_DEG: f32 = 15.0;
/// The longest delay drawn, ms (a crossing does not last longer).
const MOST_MS: f32 = 60_000.0;

/// Whose numbers the trigger has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SniperPreset {
    /// All the live accounts that held a long gun.
    Human,
    /// The top quarter of those accounts by kill share per crossing.
    Top,
}

/// The delay of a draw: the share that engage and the log-normal of the delay, ms (median, log spread).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Draw {
    engage: f32,
    median_ms: f32,
    spread: f32,
}

impl SniperPreset {
    pub(crate) fn parse(value: Option<&str>) -> Result<Option<Self>, String> {
        match value {
            None => Ok(None),
            Some("human") => Ok(Some(Self::Human)),
            Some("top") => Ok(Some(Self::Top)),
            Some(other) => Err(format!("SRP_BENCH_CAMPER_SNIPER={other:?}: human, top or unset")),
        }
    }

    fn draw(self, looking: bool) -> Draw {
        let (engage, median_ms, spread) = match (self, looking) {
            (Self::Human, true) => (1.0, 6_003.0, 1.40),
            (Self::Human, false) => (0.775, 14_765.0, 1.00),
            (Self::Top, true) => (0.975, 4_024.0, 1.50),
            (Self::Top, false) => (0.875, 8_955.0, 0.90),
        };
        Draw { engage, median_ms, spread }
    }
}

/// What the trigger does with the crossing in front of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Plan {
    /// Shoots from this clock time on.
    Shoot { at: u32 },
    /// Does not shoot at this crossing.
    Hold,
}

/// Where the trigger is in the target's coming and going.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    /// No target for 3 s or more (or none yet): the next is a new crossing.
    Idle,
    /// A crossing: the plan it drew and, if the target is out of sight now, since when.
    Crossing { plan: Plan, lost: Option<u32> },
}

/// What the trigger sees of a tick: the clock and, if the reflex has a target, how far off the view is from it, degrees.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Sight {
    pub(crate) now: u32,
    pub(crate) off_deg: Option<f32>,
}

/// The camper's trigger.
#[derive(Clone, Debug)]
pub(crate) struct Trigger {
    preset: SniperPreset,
    state: State,
    rng: Rng,
    /// The crossings drawn, those with a plan to shoot (the log line).
    crossings: (u32, u32),
}

impl Trigger {
    pub(crate) fn new(preset: SniperPreset, seed: u64) -> Self {
        Self { preset, state: State::Idle, rng: Rng::new(seed), crossings: (0, 0) }
    }

    /// One tick: the state moves on `sight`; whether the shot may go out.
    pub(crate) fn lets_fire(&mut self, sight: Sight) -> bool {
        let Sight { now, off_deg } = sight;
        let (state, fire) = match (self.state, off_deg) {
            (State::Idle, None) => (State::Idle, true),
            (State::Idle, Some(off)) => {
                let plan = self.draw(now, off <= LOOKING_DEG);
                (State::Crossing { plan, lost: None }, Self::open(plan, now))
            }
            (State::Crossing { plan, lost: None }, None) => (State::Crossing { plan, lost: Some(now) }, Self::open(plan, now)),
            (State::Crossing { lost: Some(since), .. }, None) if now.saturating_sub(since) >= GAP_MS => (State::Idle, true),
            (State::Crossing { plan, lost: Some(since) }, None) => (State::Crossing { plan, lost: Some(since) }, Self::open(plan, now)),
            (State::Crossing { plan, lost }, Some(off)) => match lost {
                Some(since) if now.saturating_sub(since) >= GAP_MS => {
                    let plan = self.draw(now, off <= LOOKING_DEG);
                    (State::Crossing { plan, lost: None }, Self::open(plan, now))
                }
                _ => (State::Crossing { plan, lost: None }, Self::open(plan, now)),
            },
        };
        self.state = state;
        fire
    }

    fn open(plan: Plan, now: u32) -> bool {
        matches!(plan, Plan::Shoot { at } if now >= at)
    }

    /// The plan for a crossing that begins at `now`.
    fn draw(&mut self, now: u32, looking: bool) -> Plan {
        self.crossings.0 += 1;
        let Draw { engage, median_ms, spread } = self.preset.draw(looking);
        if self.rng.unit() >= engage {
            return Plan::Hold;
        }
        self.crossings.1 += 1;
        let delay = (median_ms * (spread * self.normal()).exp()).min(MOST_MS);
        Plan::Shoot { at: now + delay as u32 }
    }

    /// A standard normal draw (Box-Muller).
    fn normal(&mut self) -> f32 {
        let (u, v) = (self.rng.unit().max(1e-6), self.rng.unit());
        (-2.0 * u.ln()).sqrt() * (std::f32::consts::TAU * v).cos()
    }

    /// The crossings drawn and the ones planned to be shot at.
    pub(crate) fn counts(&self) -> (u32, u32) {
        self.crossings
    }
}

/// How far the view `yaw` is from the bearing of `to` as seen from `from`, degrees.
pub(crate) fn off_deg(yaw: f32, from: Vec3, to: Vec3) -> f32 {
    let bearing = kin::yaw_of([to.x - from.x, to.z - from.z]);
    let mut d = (bearing - yaw).rem_euclid(std::f32::consts::TAU);
    if d > std::f32::consts::PI {
        d -= std::f32::consts::TAU;
    }
    d.abs().to_degrees()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sight(now: u32, off: Option<f32>) -> Sight {
        Sight { now, off_deg: off }
    }

    /// Runs one crossing of `seen_ms` at clock `from`; the clock the first shot may go at, if it does.
    fn first_shot(trigger: &mut Trigger, from: u32, seen_ms: u32, off: f32) -> Option<u32> {
        (from..from + seen_ms).step_by(50).find(|&now| trigger.lets_fire(sight(now, Some(off))))
    }

    #[test]
    fn with_nothing_in_sight_the_trigger_is_open() {
        let mut trigger = Trigger::new(SniperPreset::Human, 1);
        assert!(trigger.lets_fire(sight(0, None)));
        assert!(trigger.lets_fire(sight(5_000, None)));
    }

    #[test]
    fn a_crossing_is_held_until_its_delay_is_over_and_stays_open_after() {
        let mut trigger = Trigger::new(SniperPreset::Human, 7);
        let at = first_shot(&mut trigger, 10_000, 90_000, 5.0).expect("a looking sniper engages");
        assert!(at > 10_000, "not at once");
        assert!(trigger.lets_fire(sight(at + 50, Some(5.0))), "open after its delay");
        assert!(trigger.lets_fire(sight(at + 5_000, Some(5.0))));
    }

    #[test]
    fn the_delays_of_the_human_preset_are_the_fitted_ones() {
        let mut waits = Vec::new();
        for seed in 0..2_000_u64 {
            let mut trigger = Trigger::new(SniperPreset::Human, seed);
            if let Some(at) = first_shot(&mut trigger, 0, 120_000, 3.0) {
                waits.push(at);
            }
        }
        waits.sort_unstable();
        let median = waits[waits.len() / 2] as f32;
        assert!((median - 6_003.0).abs() < 600.0, "median {median}");
        assert!(waits.len() > 1_950, "everybody looking engages: {}", waits.len());
    }

    #[test]
    fn a_sniper_looking_away_engages_at_the_fitted_share() {
        let engaged = (0..4_000_u64)
            .filter(|&seed| first_shot(&mut Trigger::new(SniperPreset::Top, seed), 0, 120_000, 40.0).is_some())
            .count() as f32
            / 4_000.0;
        assert!((engaged - 0.875).abs() < 0.03, "{engaged}");
    }

    #[test]
    fn a_target_back_within_three_seconds_is_the_same_crossing_and_after_that_a_new_one() {
        let mut trigger = Trigger::new(SniperPreset::Human, 3);
        trigger.lets_fire(sight(1_000, Some(5.0)));
        assert_eq!(trigger.counts().0, 1);
        trigger.lets_fire(sight(1_500, None));
        trigger.lets_fire(sight(3_000, Some(5.0)));
        assert_eq!(trigger.counts().0, 1, "back after 1.5 s");
        trigger.lets_fire(sight(3_050, None));
        trigger.lets_fire(sight(6_100, None));
        trigger.lets_fire(sight(6_200, Some(5.0)));
        assert_eq!(trigger.counts().0, 2, "back after 3.15 s");
    }

    #[test]
    fn the_view_off_the_target_is_the_angle_between_them() {
        let from = Vec3::new(0.0, 0.0, 0.0);
        let to = Vec3::new(10.0, 0.0, 0.0);
        let yaw = kin::yaw_of([10.0, 0.0]);
        assert!(off_deg(yaw, from, to) < 0.01);
        assert!((off_deg(yaw + 0.5, from, to) - 28.65).abs() < 0.1);
        assert!((off_deg(yaw - 0.5, from, to) - 28.65).abs() < 0.1);
        assert!((off_deg(yaw + std::f32::consts::PI - 0.1, from, to) - 174.27).abs() < 0.1);
    }

    #[test]
    fn unknown_presets_are_refused() {
        assert_eq!(SniperPreset::parse(None), Ok(None));
        assert_eq!(SniperPreset::parse(Some("top")), Ok(Some(SniperPreset::Top)));
        assert!(SniperPreset::parse(Some("pro")).is_err());
    }
}
