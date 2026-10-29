//! The owner counts of the first-leg corridor cost (A151, `corridor.rs`): by team, the plans of the first way of a respawned
//! body younger than `young_s` (the cost's candidates), those whose plain way ran `shared_m` metres or more through regions
//! mates were in lately (the search made), how the searches ended (a costed way kept; too long; not through fewer such
//! regions; no way; out of floors), the floors they expanded, and for the kept ways the metres longer and the metres through
//! such regions before and after.
//!
//! Only a team whose tuning has the switch on is counted (the tally is the tactics' own, by the team of the bot). The bench's
//! head-to-head arms run one team on, so the other team's counts are zero.

use super::*;
use crate::v026g::bots::corridor::Tally;

#[derive(Default)]
pub(super) struct FirstLegCounts {
    counts: Vec<(&'static str, [u64; 2])>,
}

impl Topic for FirstLegCounts {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn bots(&mut self, bots: &Bots, _game: &Game) {
        if let Some(tactics) = bots.tactics.as_ref() {
            let t = &tactics.corridor_tally;
            self.counts = vec![
                ("young_plans", Tally::read(&t.young_plans)),
                ("searched", Tally::read(&t.searched)),
                ("kept", Tally::read(&t.kept)),
                ("too_long", Tally::read(&t.too_long)),
                ("not_better", Tally::read(&t.not_better)),
                ("no_way", Tally::read(&t.no_way)),
                ("no_splice", Tally::read(&t.no_splice)),
                ("budget", Tally::read(&t.budget)),
                ("floors", Tally::read(&t.floors)),
                ("detour_mm", Tally::read(&t.detour_mm)),
                ("shared_before_mm", Tally::read(&t.shared_before_mm)),
                ("shared_after_mm", Tally::read(&t.shared_after_mm)),
            ];
        }
    }

    fn parts(&mut self, _run: &Run, _game: &Game, _scale: &Scale, _shared: &Shared) -> Vec<(&'static str, Part)> {
        vec![("first_leg", self.part())]
    }
}

impl FirstLegCounts {
    fn part(&self) -> Part {
        let by_team: serde_json::Map<String, serde_json::Value> = self.counts.iter().map(|(name, v)| (name.to_string(), serde_json::json!(v))).collect();
        let get = |name: &str| self.counts.iter().find(|(n, _)| *n == name).map_or([0, 0], |(_, v)| *v);
        let (plans, searched, kept) = (get("young_plans"), get("searched"), get("kept"));
        let total = |v: [u64; 2]| v[0] + v[1];
        let json = serde_json::json!({ "first_leg": { "by_team_0_1": by_team } });
        Part::new(
            json,
            format!(
                "owner-test first leg: {} young plans, {} searched, {} kept (team 0 {} / team 1 {}), floors {}, detour {:.0} m, shared {:.0} -> {:.0} m",
                total(plans),
                total(searched),
                total(kept),
                kept[0],
                kept[1],
                total(get("floors")),
                total(get("detour_mm")) as f64 / 1000.0,
                total(get("shared_before_mm")) as f64 / 1000.0,
                total(get("shared_after_mm")) as f64 / 1000.0,
            ),
        )
    }
}
