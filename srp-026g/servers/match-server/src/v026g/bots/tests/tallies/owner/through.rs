//! The owner counts of fire through cover (`Reflex::fire_through`, P4b): by team, the ticks the trigger was pressed at an enemy
//! the bot no longer saw, the bursts begun, and the chances refused: the damage share of the line under the bar, a mate in it,
//! the gap since the last burst. What came of the shots is the pierce topic's (hits and kills by bullets that crossed cover).

use super::*;
use crate::v026g::bots::reflex::ThroughStats;

#[derive(Default)]
pub(super) struct ThroughCounts {
    /// By team: the lives that ended.
    banked: [ThroughStats; 2],
    /// By place: the team and the counts of the life under way.
    now: Vec<Option<(usize, ThroughStats)>>,
}

impl Topic for ThroughCounts {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn read(&mut self, place: Place, bot: Option<&Bot>, _tick: u32) {
        if self.now.len() <= place {
            self.now.resize(place + 1, None);
        }
        let Some(bot) = bot else {
            return;
        };
        let (team, stats) = (usize::from(bot.team).min(1), bot.reflex.through_stats());
        // A reflex younger than the last reading is a new life: the last one is banked.
        if let Some((last_team, last)) = self.now[place]
            && stats.total() < last.total()
        {
            self.banked[last_team].add(&last);
        }
        self.now[place] = Some((team, stats));
    }

    fn parts(&mut self, _run: &Run, _game: &Game, _scale: &Scale, _shared: &Shared) -> Vec<(&'static str, Part)> {
        vec![("through", self.part())]
    }
}

impl ThroughCounts {
    fn totals(&self) -> [ThroughStats; 2] {
        let mut all = self.banked;
        for &(team, stats) in self.now.iter().flatten() {
            all[team].add(&stats);
        }
        all
    }

    fn part(&self) -> Part {
        let all = self.totals();
        let one = |s: &ThroughStats| {
            serde_json::json!({
                "ticks": s.ticks,
                "bursts": s.bursts,
                "refused_share": s.refused_share,
                "refused_friend": s.refused_friend,
                "refused_gap": s.refused_gap,
            })
        };
        let json = serde_json::json!({
            "through_ticks": all[0].ticks + all[1].ticks,
            "through_bursts": all[0].bursts + all[1].bursts,
            "through_refused_share": all[0].refused_share + all[1].refused_share,
            "through_refused_friend": all[0].refused_friend + all[1].refused_friend,
            "through_refused_gap": all[0].refused_gap + all[1].refused_gap,
            "through": { "by_team": { "0": one(&all[0]), "1": one(&all[1]) } },
        });
        let text = format!(
            "owner-test through: fire at a target not in sight: ticks {} bursts {} (team 0 {} / team 1 {}); refused: damage share under the bar {}, a mate in the line {}, the gap since the last burst {}",
            all[0].ticks + all[1].ticks,
            all[0].bursts + all[1].bursts,
            all[0].bursts,
            all[1].bursts,
            all[0].refused_share + all[1].refused_share,
            all[0].refused_friend + all[1].refused_friend,
            all[0].refused_gap + all[1].refused_gap,
        );
        Part::new(json, text)
    }
}
