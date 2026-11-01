//! A231 (the coordinator, 2026-11-01): battery collectors dying at the battery. A220 (the live bots, read-only): 46 matches of
//! a human's kill-to-death of 10 or more, 41 of them on Tarakanovsky Fort's batteries. Does the team's threat picture keep its
//! collectors off a battery a shooter watches? On our tip a Collect's way is weighed by remembered snipers' lanes
//! (`threat.sniper_memory`), no-feeding's objective weight (`threat.concede`) is off by default, and nothing refuses a Collect
//! into a held lane (design-p1c item 13: an objective is weighed, never refused). The count, at each death of a bot whose last
//! play was a Collect of a lying battery (within [`RECENT_MS`] of its death): at the battery (within [`AT_M`]) or on its way;
//! whether the battery lay in a held lane of the dying bot's team (`HeldLine::covers`) and in its killer's own; whether the
//! killer was the stand-in human; by the battery's index in the match. Nothing decides by these counts.

use super::*;
use crate::v026g::bots::plays::{PlayKind, TargetRef};
use crate::v026g::bots::tests::Lying;
use crate::v026g::bots::threat::HeldLine;

/// A collector this near its battery died at it, m.
const AT_M: f32 = 5.0;
/// A Collect read this long before the death was the dying bot's play, ms (its assignment may be gone at the death's tick).
const RECENT_MS: u32 = 1_000;

/// The counts of one battery: deaths, at the battery, the battery in a held lane of the team, in the killer's own held lane,
/// by the stand-in human, by him with the battery in his held lane.
const FIELDS: [&str; 6] = ["deaths", "at_battery", "in_team_lane", "in_killer_lane", "by_human", "by_human_in_his_lane"];

/// The [`FIELDS`] of one collector's death at `feet`, its battery at `battery`, by `killer`, the team's held lines `held`.
fn death_fields(held: &[HeldLine], killer: Place, human: Option<Place>, feet: Vec3, battery: Vec3) -> [bool; FIELDS.len()] {
    let in_team_lane = held.iter().any(|l| l.covers(battery));
    let in_killer_lane = held.iter().any(|l| l.shooter == killer && l.covers(battery));
    let by_human = human == Some(killer);
    [true, feet.distance_xz(battery) <= AT_M, in_team_lane, in_killer_lane, by_human, by_human && in_killer_lane]
}

#[derive(Default)]
pub(super) struct CollectAtCounts {
    human: Option<Place>,
    held: [Vec<HeldLine>; 2],
    /// Each place's last Collect of a battery: the battery's index and when it was read.
    collecting: Vec<Option<(usize, u32)>>,
    kills_seen: usize,
    by_battery: BTreeMap<usize, [u64; FIELDS.len()]>,
}

impl CollectAtCounts {
    pub(super) fn new(n: usize) -> Self {
        Self { collecting: vec![None; n], ..Self::default() }
    }

    pub(super) fn set_human(&mut self, human: Option<Place>) {
        self.human = human;
    }
}

impl Topic for CollectAtCounts {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn read(&mut self, place: Place, bot: Option<&Bot>, tick: u32) {
        if place >= self.collecting.len() {
            self.collecting.resize(place + 1, None);
        }
        // Another play replaces the Collect; no play (a death, a respawn) keeps the last one for the death's tick.
        match bot.and_then(|b| b.assignment.as_ref()).map(|a| (a.kind, a.target)) {
            Some((PlayKind::Collect, TargetRef::Battery(n))) => self.collecting[place] = Some((n, tick)),
            Some(_) => self.collecting[place] = None,
            None => {}
        }
    }

    fn bots(&mut self, bots: &Bots, _game: &Game) {
        for (team, held) in self.held.iter_mut().enumerate() {
            *held = bots.teams.get(team).map(|state| state.knowledge.threat.held.clone()).unwrap_or_default();
        }
    }

    fn end(&mut self, run: &Run, game: &mut Game, _sight: &mapdata::RayTree, _rays: &mut u64) {
        for &(t, killer, victim) in &run.kills[self.kills_seen.min(run.kills.len())..] {
            let Some((battery, read_at)) = self.collecting.get_mut(victim).and_then(Option::take) else {
                continue;
            };
            // The stand-in human is a bot place: his own collects are not a bot's.
            if self.human == Some(victim) {
                continue;
            }
            if t.saturating_sub(read_at) > RECENT_MS {
                continue;
            }
            let Some(&Lying::Free(at, _)) = game.batteries.get(battery) else {
                continue;
            };
            let Some(body) = game.bodies.get(victim) else {
                continue;
            };
            let feet = run.recs[victim].last().map_or(body.feet, |r| r.feet);
            let add = death_fields(&self.held[usize::from(body.team).min(1)], killer, self.human, feet, at);
            let c = self.by_battery.entry(battery).or_default();
            for (n, yes) in c.iter_mut().zip(add) {
                *n += u64::from(yes);
            }
        }
        self.kills_seen = run.kills.len();
    }

    fn parts(&mut self, _run: &Run, _game: &Game, _scale: &Scale, _shared: &Shared) -> Vec<(&'static str, Part)> {
        let mut total = [0u64; FIELDS.len()];
        let mut by = serde_json::Map::new();
        for (battery, c) in &self.by_battery {
            for (t, n) in total.iter_mut().zip(c) {
                *t += n;
            }
            by.insert(battery.to_string(), serde_json::json!(c));
        }
        let mut json: serde_json::Map<String, serde_json::Value> =
            FIELDS.iter().zip(total).map(|(name, n)| ((*name).to_owned(), n.into())).collect();
        json.insert("by_battery".to_owned(), serde_json::Value::Object(by));
        let text = format!(
            "owner-test collect at (A231: deaths of collectors of a lying battery; per battery {FIELDS:?}): {}",
            FIELDS.iter().zip(total).map(|(name, n)| format!("{name} {n}")).collect::<Vec<_>>().join(", ")
        );
        vec![("collect_at", Part::new(serde_json::json!({ "collect_at": json }), text))]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_collectors_death_is_marked_by_the_lanes_its_battery_lay_in() {
        // Shooter 7 (the stand-in human) holds a lane down z to the battery at z 10; shooter 8 holds one along x.
        let battery = Vec3::new(0.0, 0.0, 10.0);
        let his = HeldLine::for_test(1, 7, Vec3::new(0.0, 0.0, 40.0), &[Vec3::new(0.0, 0.0, 0.0)], None);
        let other = HeldLine::for_test(2, 8, Vec3::new(40.0, 0.0, 40.0), &[Vec3::new(80.0, 0.0, 40.0)], None);
        let held = [his, other];
        let at = Vec3::new(1.0, 0.0, 12.0);
        assert_eq!(death_fields(&held, 7, Some(7), at, battery), [true, true, true, true, true, true]);
        assert_eq!(death_fields(&held, 8, Some(7), at, battery), [true, true, true, false, false, false], "a bot of his lane's side");
        let on_the_way = Vec3::new(0.0, 0.0, 30.0);
        assert_eq!(death_fields(&held[1..], 7, None, on_the_way, battery), [true, false, false, false, false, false]);
    }
}
