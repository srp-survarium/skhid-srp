//! The owner counts of the Range leg's runs (register C5, flip-flop (a); `fact-goal-flipflop.md`): a bot's route is thrown away and
//! searched again each time the reflex's Range step takes the legs and gives them back, so a Range leg that lasts a tick or two, or
//! that begins again within a second of ending, is the flip-flop that cost 4.4 % of bot time before the sight fix.
//!
//! A *run* is a stretch of consecutive ticks in which the bot's legs are the reflex's, for the reason [`LegsWhy::Range`]
//! (`Bot::bench_legs_why`); it is whole when it ends with the leg given back (the bot dies, or the match ends, in the middle of one
//! that is not counted whole). A run is *short* if it lasts [`SHORT_TICKS`] ticks (1 s) or fewer; a *restart* is a run that begins
//! within [`RESTART_TICKS`] ticks (1 s) of the end of the one before it, in the same life.
//!
//! Counts only: nothing decides by them.
//!
//! Keys of the `owner` object `range_leg`: `runs` (whole runs), `ticks` (the ticks they lasted), `by_length` (runs of `1`, `2-4`, `5-9`,
//! `10-20`, `21+` ticks), `one_tick`, `short`, `restarts` (whole runs that began within a second of the end of the last one), and
//! the rates `short_share_pct` and `restart_share_pct` of the whole runs.

use super::*;
use crate::v026g::bots::reflex::LegsWhy;

/// A short run lasts this many ticks or fewer (1 s); a restart begins within this many ticks of the end of the last run.
const SHORT_TICKS: u32 = 20;
const RESTART_TICKS: u32 = 20;
/// The run lengths the counts are made by, in ticks: the upper edge of each but the last.
const EDGES: [u32; 4] = [1, 4, 9, 20];
const LABELS: [&str; 5] = ["1", "2-4", "5-9", "10-20", "21+"];

/// A bot's Range leg.
#[derive(Clone, Copy)]
enum Leg {
    /// Not on the Range leg, and the ticks since the last run ended (none: no run this life).
    Off { since_end: Option<u32> },
    /// On it, for this many ticks, begun as a restart or not.
    On { ticks: u32, restart: bool },
}

#[derive(Default)]
struct Totals {
    runs: u32,
    ticks: u64,
    by_length: [u32; 5],
    short: u32,
    restarts: u32,
}

pub(super) struct RangeLegCounts {
    /// Each bot's legs this tick, read before the bodies.
    now: Vec<Option<LegsWhy>>,
    leg: Vec<Leg>,
    totals: Totals,
}

impl RangeLegCounts {
    pub(super) fn new(n: usize) -> Self {
        Self { now: vec![None; n], leg: vec![Leg::Off { since_end: None }; n], totals: Totals::default() }
    }

    /// A bot's tick: `range` if its legs are the Range leg now, `alive` if it lives.
    fn tick(&mut self, place: usize, alive: bool, range: bool) {
        let t = &mut self.totals;
        self.leg[place] = match (self.leg[place], alive && range) {
            (Leg::Off { since_end }, false) => Leg::Off { since_end: since_end.map(|n| n + 1).filter(|&n| alive && n <= RESTART_TICKS) },
            (Leg::Off { since_end }, true) => Leg::On { ticks: 1, restart: since_end.is_some_and(|n| n <= RESTART_TICKS) },
            (Leg::On { ticks, restart }, true) => Leg::On { ticks: ticks + 1, restart },
            (Leg::On { ticks, restart }, false) if alive => {
                t.runs += 1;
                t.ticks += u64::from(ticks);
                t.by_length[EDGES.iter().position(|&edge| ticks <= edge).unwrap_or(EDGES.len())] += 1;
                t.short += u32::from(ticks <= SHORT_TICKS);
                t.restarts += u32::from(restart);
                Leg::Off { since_end: Some(0) }
            }
            (Leg::On { .. }, false) => Leg::Off { since_end: None },
        };
    }

    fn part(&self) -> Part {
        let t = &self.totals;
        let pct = |n: u32| if t.runs > 0 { (1000.0 * f64::from(n) / f64::from(t.runs)).round() / 10.0 } else { 0.0 };
        let by_length: serde_json::Map<String, serde_json::Value> =
            LABELS.iter().zip(t.by_length).map(|(label, n)| ((*label).to_string(), n.into())).collect();
        let json = serde_json::json!({ "range_leg": {
            "runs": t.runs,
            "ticks": t.ticks,
            "by_length": by_length,
            "one_tick": t.by_length[0],
            "short": t.short,
            "restarts": t.restarts,
            "short_share_pct": pct(t.short),
            "restart_share_pct": pct(t.restarts),
        } });
        let text = format!(
            "owner-test range leg: {} whole runs ({} ticks), by length {}; short (1 s or less) {} ({} %), one tick {}, restarted within a second of the last run {} ({} %)",
            t.runs,
            t.ticks,
            LABELS.iter().zip(t.by_length).map(|(label, n)| format!("{label}: {n}")).collect::<Vec<_>>().join(", "),
            t.short,
            pct(t.short),
            t.by_length[0],
            t.restarts,
            pct(t.restarts),
        );
        Part::new(json, text)
    }
}

impl Topic for RangeLegCounts {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn read(&mut self, place: Place, bot: Option<&Bot>, _tick: u32) {
        if let Some(slot) = self.now.get_mut(place) {
            *slot = bot.and_then(|bot| bot.bench_legs_why);
        }
    }

    #[cfg(test)]
    fn inject(&mut self, place: Place, now: Option<&BotNow>) {
        if let Some(slot) = self.now.get_mut(place) {
            *slot = now.and_then(|n| n.legs);
        }
    }

    fn body(&mut self, _run: &Run, at: &BodyAt<'_>, _rays: &mut u64) {
        if at.place >= self.leg.len() {
            return;
        }
        let range = self.now[at.place] == Some(LegsWhy::Range);
        self.tick(at.place, at.game.bodies[at.place].alive, range);
    }

    fn parts(&mut self, _run: &Run, _game: &Game, _scale: &Scale, _shared: &Shared) -> Vec<(&'static str, Part)> {
        vec![("range_leg", self.part())]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed(topic: &mut RangeLegCounts, pattern: &[(bool, bool)]) {
        for &(alive, range) in pattern {
            topic.tick(0, alive, range);
        }
    }

    /// A one-tick run, a restart of five ticks within a second, and a run begun after more than a second: whole runs counted by
    /// length, the first run is not a restart, the second is, the third is not.
    #[test]
    fn runs_are_counted_by_length_and_restarts_within_a_second() {
        let mut topic = RangeLegCounts::new(1);
        let mut pattern = vec![(true, true)]; // run of 1
        pattern.extend([(true, false); 5]);
        pattern.extend([(true, true); 5]); // run of 5, a restart (5 ticks after the end)
        pattern.extend([(true, false); 25]);
        pattern.extend([(true, true); 30]); // run of 30, not a restart
        pattern.push((true, false));
        feed(&mut topic, &pattern);
        let t = &topic.totals;
        assert_eq!((t.runs, t.ticks), (3, 36));
        assert_eq!(t.by_length, [1, 0, 1, 0, 1]);
        assert_eq!((t.short, t.restarts), (2, 1));
    }

    /// A death in the middle of a run does not count the run, and the next life's first run is not a restart.
    #[test]
    fn a_death_cuts_a_run_and_the_next_life_does_not_restart() {
        let mut topic = RangeLegCounts::new(1);
        feed(&mut topic, &[(true, true), (true, true), (false, false), (true, false), (true, true), (true, false)]);
        assert_eq!(topic.totals.runs, 1, "only the run of the new life");
        assert_eq!(topic.totals.restarts, 0);
    }
}
