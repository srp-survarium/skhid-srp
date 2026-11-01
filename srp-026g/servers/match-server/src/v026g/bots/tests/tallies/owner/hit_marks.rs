//! The owner counts of the hit marker (A227, `aim.combat.hit_bearing`, `reflex::hit_marks`), by team: hits with no
//! shooter sensed (with the switch on or off, and those taken while carrying), the marker's direction turned to (and
//! while carrying), not turned to (a target kept, no shooter found), the look's ticks with the legs going, standing or
//! given way to a carry or a use, the cover sought from it (declined, taken, no spot), and the teammates' glances along
//! its callout. A switch that is on where its side shows 0 marks did not fire.
//!
//! The counts are the thread's (`hit_marks::counted`), read as the difference from the topic's start: a match runs on
//! one thread.

use super::*;
use crate::v026g::bots::reflex::hit_marks;

/// The hit marker's counts at the start of the match, for the difference at its end.
pub(super) struct HitMarkCounts {
    from: [[u64; hit_marks::MARKS.len()]; 2],
}

impl Default for HitMarkCounts {
    fn default() -> Self {
        Self { from: hit_marks::counted() }
    }
}

impl Topic for HitMarkCounts {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn parts(&mut self, _run: &Run, _game: &Game, _scale: &Scale, _shared: &Shared) -> Vec<(&'static str, Part)> {
        vec![("hit_marks", self.part())]
    }
}

impl HitMarkCounts {
    fn part(&self) -> Part {
        let now = hit_marks::counted();
        let team = |t: usize| {
            let mut json = serde_json::Map::new();
            for (k, (_, name)) in hit_marks::MARKS.iter().enumerate() {
                json.insert((*name).into(), (now[t][k] - self.from[t][k]).into());
            }
            serde_json::Value::Object(json)
        };
        let json = serde_json::json!({ "hit_marks": { "by_team": { "0": team(0), "1": team(1) } } });
        let line = |t: usize| {
            hit_marks::MARKS
                .iter()
                .enumerate()
                .map(|(k, (_, name))| format!("{name} {}", now[t][k] - self.from[t][k]))
                .collect::<Vec<_>>()
                .join(" ")
        };
        let text = format!("owner-test hit marks: team 0: {}; team 1: {}", line(0), line(1));
        Part::new(json, text)
    }
}
