//! The owner counts of the damage-zone fixes of the crossings and the escort posts (`HazardTuning::route_crossing`,
//! `price_way_out`, `hurt_by_shape`, `escort_dry_post`): how often each fires, by the team that asked, so that a head-to-head run
//! (one team ON, the other OFF) reads the ON team's counts as the ON team's. Keys are `dzf_<kind>_<event>` (and `dzf_by_team`) with the objective kind
//! of the plays that ask ([`damage_zone_fixes::KIND`], the batteries'); a mode with other plays over damage zones counts under
//! its own kind. Damage zones only: a research mode's capture zones are another thing and another topic.
//!
//! The decisions count in the thread of the match ([`damage_zone_fixes`]); the topic reads what they counted since it began.

use super::*;
use crate::v026g::bots::hazards::damage_zone_fixes::{self, Counts, KIND, NAMES};

pub(super) struct DamageZoneFixes {
    start: Counts,
}

impl Default for DamageZoneFixes {
    fn default() -> Self {
        Self { start: damage_zone_fixes::counts() }
    }
}

impl DamageZoneFixes {
    /// What the match counted so far, by team.
    fn since(&self) -> Counts {
        let now = damage_zone_fixes::counts();
        std::array::from_fn(|t| std::array::from_fn(|k| now[t][k] - self.start[t][k]))
    }
}

impl Topic for DamageZoneFixes {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn parts(&mut self, run: &Run, game: &Game, _scale: &Scale, _shared: &Shared) -> Vec<(&'static str, Part)> {
        let by = self.since();
        let drops = drops_by_play(run, game);
        let keyed = |counts: &[u64]| -> serde_json::Value {
            NAMES.iter().zip(counts).map(|(name, n)| (format!("dzf_{KIND}_{name}"), serde_json::json!(n))).collect()
        };
        let all: Vec<u64> = (0..NAMES.len()).map(|k| by[0][k] + by[1][k]).collect();
        let mut json = keyed(&all);
        json["dzf_by_team"] = serde_json::json!({ "0": keyed(&by[0]), "1": keyed(&by[1]) });
        // Drops of 4 m or more by the play the body was in when the second began, and by the body's team: which decision walks
        // bots down a ledge (the escort's dry post was one cause of the PR 704 drops warning).
        json["dzf_drops_4m_by_play"] = serde_json::json!({ "0": drops[0], "1": drops[1] });
        let line = |counts: &[u64]| {
            NAMES.iter().zip(counts).map(|(name, n)| format!("{name} {n}")).collect::<Vec<_>>().join(", ")
        };
        let text = format!(
            "owner-test damage-zone fixes ({KIND}): {}; team 0: [{}]; team 1: [{}]; drops of 4 m or more by play, team 0 {:?}, team 1 {:?}",
            line(&all),
            line(&by[0]),
            line(&by[1]),
            drops[0],
            drops[1],
        );
        vec![("damage_zone_fixes", Part::new(json, text))]
    }
}

/// The drops of [`DROP_M`]`[1]` or more a body made in one second (the moving topic's rule: the second whole and alive, the feet
/// down that far from the last tick of the second before), filed under the play it was in when the second began and its team.
fn drops_by_play(run: &Run, game: &Game) -> [BTreeMap<String, u32>; 2] {
    let mut out = [BTreeMap::new(), BTreeMap::new()];
    for (place, recs) in run.recs.iter().enumerate() {
        let team = usize::from(game.bodies.get(place).is_some_and(|b| b.team != 0));
        for s in 1..recs.len() / TPS {
            let sec = &recs[s * TPS..(s + 1) * TPS];
            if sec.iter().all(|r| r.alive) && recs[s * TPS - 1].alive {
                let down = recs[s * TPS - 1].feet.y - sec[TPS - 1].feet.y;
                if down >= DROP_M[1] {
                    *out[team].entry(run.plays[usize::from(sec[0].play)].clone()).or_default() += 1;
                }
            }
        }
    }
    out
}

/// The counts are the match's own and are filed under the team that asked: a crossing asked by team 1's bot is team 1's.
#[test]
pub(crate) fn the_fixes_are_counted_by_the_team_that_asked() {
    use damage_zone_fixes::{Ev, asked_by, count};
    let (game, mut tally) = made_up(&[0, 1]);
    asked_by(0, || count(Ev::CrossingAsked));
    asked_by(1, || {
        count(Ev::CrossingAsked);
        count(Ev::RoutedTotalOnly);
    });
    asked_by(1, || count(Ev::PostNone));
    let (_, json) = tally.finish(&game);
    assert_eq!(json["dzf_by_team"]["0"]["dzf_bat_cross_asked"], 1);
    assert_eq!(json["dzf_by_team"]["1"]["dzf_bat_cross_asked"], 1);
    assert_eq!(json["dzf_by_team"]["1"]["dzf_bat_routed_total_only"], 1);
    assert_eq!(json["dzf_by_team"]["0"]["dzf_bat_routed_total_only"], 0);
    assert_eq!(json["dzf_bat_post_none"], 1);
}

/// A drop of 4 m or more is filed under the play and the team of the body that made it (the plays of a made-up match are `Roam`).
#[test]
pub(crate) fn a_drop_of_four_metres_is_filed_under_its_play_and_team() {
    let (mut game, mut tally) = made_up(&[0, 1]);
    play(&mut game, &mut tally, 60, |k, game| {
        put(game, 0, 10.0, 10.0, 0);
        put(game, 1, 60.0, 20.0, 0);
        game.bodies[0].feet.y = if k < 20 { 10.0 } else { 4.5 };
        game.bodies[1].feet.y = if k < 20 { 10.0 } else { 8.0 };
        vec![bot_now(), bot_now()]
    });
    let (_, json) = tally.finish(&game);
    assert_eq!(json["dzf_drops_4m_by_play"]["0"]["Roam"], 1, "{json}");
    assert_eq!(json["dzf_drops_4m_by_play"]["1"], serde_json::json!({}), "{json}");
}
