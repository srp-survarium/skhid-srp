//! The owner counts of the Breach (`threat.breach`, `bots/breach.rs`, design-p1c §1), by team: spots made due by a failed
//! Flank, Breaches opened (with a pinner, on an objective spot) and the refusals by reason (no cover point, no pair, no gather
//! point, too few free crossers, the cap, the spot memory: conceded or like the last try), the go (and the gather's wait), the
//! hold, the first crosser death and the trade of it (and its time), how the job ended (his death by the job or by another,
//! the spot unheld, the failures by reason), the pinners lost and the crossers down. A 0 where the switch is on and a Flank
//! failed shows the job never ran.
//!
//! The counts are the thread's (`breach::tally::counted`), read as the difference from the topic's start: a match runs on one
//! thread.

use super::*;
use crate::v026g::bots::breach::tally;

/// The Breach's counts at the start of the match, for the difference at its end.
pub(super) struct BreachCounts {
    start: [[u64; tally::ALL]; 2],
}

impl Default for BreachCounts {
    fn default() -> Self {
        Self { start: tally::counted() }
    }
}

impl Topic for BreachCounts {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn parts(&mut self, _run: &Run, _game: &Game, _scale: &Scale, _shared: &Shared) -> Vec<(&'static str, Part)> {
        vec![("breach", self.part())]
    }
}

impl BreachCounts {
    fn part(&self) -> Part {
        let now = tally::counted();
        let mut json = serde_json::Map::new();
        let mut text = Vec::new();
        for team in 0..2usize {
            let n = |i: usize| now[team][i] - self.start[team][i];
            let mut t = serde_json::Map::new();
            for (i, name) in tally::NAMES.iter().enumerate() {
                t.insert((*name).into(), n(i).into());
            }
            t.insert("pinner_lost".into(), n(tally::PINNER_LOST).into());
            t.insert("crosser_downs".into(), n(tally::CROSSER_DOWNS).into());
            let get = |name: &str| t.get(name).and_then(serde_json::Value::as_u64).unwrap_or(0);
            let went = get("went");
            let traded = get("traded");
            text.push(format!(
                "team {team}: due {}, opened {} (pinner {}, objective {}), refused no cover {} / no pair {} / no gather {} / no crossers {} / cap {} / conceded {} / like the last {}; went {} (gather wait mean {:.1} s), held {}, first deaths {} traded {} (mean {:.1} s); ended his death by the job {} / by another {} / unheld {}; failed holder down {} / untraded {} / gather timeout {} / aborted {} / time {}; pinners lost {}, crossers down {}",
                get("due"),
                get("opened"),
                get("opened_with_pinner"),
                get("opened_objective"),
                get("no_cover"),
                get("no_pair"),
                get("no_gather"),
                get("no_crossers"),
                get("cap"),
                get("conceded"),
                get("like_the_last"),
                went,
                get("gather_wait_ms") as f64 / 1000.0 / went.max(1) as f64,
                get("holding"),
                get("first_death"),
                traded,
                get("trade_ms") as f64 / 1000.0 / traded.max(1) as f64,
                get("shooter_killed_by_job"),
                get("shooter_died_other"),
                get("unheld"),
                get("failed_holder_down"),
                get("failed_untraded"),
                get("failed_gather_timeout"),
                get("failed_aborted"),
                get("failed_time"),
                n(tally::PINNER_LOST),
                n(tally::CROSSER_DOWNS),
            ));
            json.insert(format!("team{team}"), serde_json::Value::Object(t));
        }
        Part::new(serde_json::Value::Object(json), format!("owner-test breach: {}", text.join("; ")))
    }
}
