//! The owner counts of the gang-up cap (PR C item 20, `team::gang_factor`): by play aimed at an enemy, the assignments by the
//! teammates (the trade partner not counted) already aimed at the chosen enemy (none, one, two, three or more), the new plays
//! whose best candidate was scored down, and the ones that lost the pick to another (the bots redirected); and for the
//! commander's passes, the enemies the team's bots were aimed at and those with three or more bots aimed at them.
//!
//! Only a team whose tuning has the switch on is counted (the tally is the tactics' own; the topic copies it). The bench
//! runs a head-to-head with one team on, so its counts are that team's.

use super::*;
use crate::v026g::bots::plays::PlayKind;

#[derive(Default)]
pub(super) struct GangUpCounts {
    passes: [u64; 3],
    plays: Vec<(PlayKind, [u64; 4], u64, u64)>,
}

impl Topic for GangUpCounts {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn bots(&mut self, bots: &Bots, _game: &Game) {
        use std::sync::atomic::Ordering::Relaxed;
        if let Some(tactics) = bots.tactics.as_ref() {
            let t = &tactics.gang_tally;
            self.passes = [t.passes.load(Relaxed), t.enemies_aimed.load(Relaxed), t.enemies_3plus.load(Relaxed)];
            self.plays = t.read();
        }
    }

    fn parts(&mut self, _run: &Run, _game: &Game, _scale: &Scale, _shared: &Shared) -> Vec<(&'static str, Part)> {
        vec![("gang_up", self.part())]
    }
}

impl GangUpCounts {
    fn part(&self) -> Part {
        let [passes, aimed, three] = self.passes;
        let by_play: serde_json::Map<String, serde_json::Value> = self
            .plays
            .iter()
            .map(|(kind, on, penalised, redirected)| {
                (
                    format!("{kind:?}"),
                    serde_json::json!({ "chosen_on_0_1_2_3plus": on, "penalised": penalised, "redirected": redirected }),
                )
            })
            .collect();
        let json = serde_json::json!({
            "gang_up": { "passes": passes, "enemies_aimed": aimed, "enemies_with_3plus": three, "by_play": by_play },
        });
        let redirected: u64 = self.plays.iter().map(|p| p.3).sum();
        let penalised: u64 = self.plays.iter().map(|p| p.2).sum();
        Part::new(
            json,
            format!(
                "owner-test gang up: {passes} passes, {aimed} enemies aimed at, {three} with 3+ bots; {penalised} new plays scored down, {redirected} bots redirected"
            ),
        )
    }
}
