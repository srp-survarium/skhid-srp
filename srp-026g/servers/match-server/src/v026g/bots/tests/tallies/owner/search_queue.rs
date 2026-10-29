//! The owner counts of the route-search queue (`search.rs`, `ThinkTuning::search_queue`), by team: searches
//! granted, bot ticks starved (a search wanted and none to be had this poll), floors spent, the polls a bot waited
//! before its search, and the ticks with a goal and no route by cause (`path::NoRoute`). A fair queue shows as
//! the two teams' counts alike whatever their places, and fewer starved ticks than the first-come pool's.
//!
//! The counts are the thread's (`search::queue`, `path::no_routes_by_team`), read as the difference from the
//! topic's start: a match runs on one thread.

use super::*;
use crate::v026g::bots::path;
use crate::v026g::bots::search;

/// The queue's counts at the start of the match, for the difference at its end.
pub(super) struct SearchQueueCounts {
    queue: [[u64; 13]; 2],
    no_route: [[u64; 6]; 2],
}

impl Default for SearchQueueCounts {
    fn default() -> Self {
        Self { queue: search::queue::counted(), no_route: path::no_routes_by_team() }
    }
}

/// The [`path::NoRoute`] causes, in order, as the tally names them.
const NO_ROUTE_NAMES: [&str; 6] = ["no_way", "searching", "starved", "searched", "ended", "shut_in"];

impl Topic for SearchQueueCounts {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn parts(&mut self, _run: &Run, _game: &Game, _scale: &Scale, _shared: &Shared) -> Vec<(&'static str, Part)> {
        vec![("search_queue", self.part())]
    }
}

impl SearchQueueCounts {
    fn part(&self) -> Part {
        let (queue, no_route) = (search::queue::counted(), path::no_routes_by_team());
        let mut json = serde_json::Map::new();
        let mut text = Vec::new();
        for team in 0..2 {
            let mut t = serde_json::Map::new();
            for (i, name) in search::queue::NAMES.iter().enumerate() {
                t.insert((*name).into(), (queue[team][i] - self.queue[team][i]).into());
            }
            let mut causes = serde_json::Map::new();
            for (i, name) in NO_ROUTE_NAMES.iter().enumerate() {
                causes.insert((*name).into(), (no_route[team][i] - self.no_route[team][i]).into());
            }
            let n = |key: &str| t.get(key).and_then(serde_json::Value::as_u64).unwrap_or(0);
            let c = |key: &str| causes.get(key).and_then(serde_json::Value::as_u64).unwrap_or(0);
            text.push(format!(
                "team {team}: searches granted {}, starved ticks {}, floors {} (handed over {}, idle with demand {}), waited {} times ({} polls), under way dropped (goal moved {}, hazards {}, bot gone {}), partial route ticks {}, unstick jumps {} ({} on a partial route); goal and no route ticks: no way {}, searching {}, starved {}, searched {}, ended {}, shut in {}",
                n("granted"),
                n("starved"),
                n("floors"),
                n("handed_over"),
                n("idle_with_demand"),
                n("waited"),
                n("waited_polls"),
                n("dropped_moved"),
                n("dropped_hazards"),
                n("dropped_gone"),
                n("partial_ticks"),
                n("unsticks"),
                n("unsticks_partial"),
                c("no_way"),
                c("searching"),
                c("starved"),
                c("searched"),
                c("ended"),
                c("shut_in"),
            ));
            t.insert("no_route".into(), serde_json::Value::Object(causes));
            json.insert(format!("team{team}"), serde_json::Value::Object(t));
        }
        Part::new(serde_json::Value::Object(json), format!("owner-test search queue: {}", text.join("; ")))
    }
}
