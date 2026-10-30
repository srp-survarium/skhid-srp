//! The owner counts of each team's shots at its target by the target's distance (the bench's reading of a switch of the
//! bots' aim by side, `lead_flight`): shots and the hits among them, by the shooter's team and the horizontal distance band.
//! Counts only: nothing decides by them.
//!
//! Key of the `owner` object `shot_bands`: `"team<t>"` -> `"<band>"` (`0-10`, `10-20`, `20-35`, `35-50`, `50-80`, `80+` m) ->
//! `shots` (at the shooter's target) and `hits` (the shot hit that target).

use super::*;

/// The bands' upper bounds, metres.
const EDGES: [f32; 5] = [10.0, 20.0, 35.0, 50.0, 80.0];
const LABELS: [&str; 6] = ["0-10", "10-20", "20-35", "35-50", "50-80", "80+"];

#[derive(Default)]
pub(super) struct ShotBands {
    /// By team, by band: shots, hits.
    bands: [[[u32; 2]; 6]; 2],
}

impl Topic for ShotBands {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    /// The shots the stand-in fired this tick.
    fn begin(&mut self, _run: &Run, game: &mut Game) {
        for fact in &game.shot_facts {
            if let Some((target, distance, _)) = fact.target {
                let team = usize::from(game.bodies[fact.shooter].team).min(1);
                self.shot(team, distance, fact.hit == Some(target));
            }
        }
    }

    fn parts(&mut self, _run: &Run, _game: &Game, _scale: &Scale, _shared: &Shared) -> Vec<(&'static str, Part)> {
        if self.bands.iter().flatten().all(|cell| cell[0] == 0) {
            return Vec::new();
        }
        let mut json = serde_json::Map::new();
        let mut text = Vec::new();
        for (team, bands) in self.bands.iter().enumerate() {
            let mut out = serde_json::Map::new();
            let mut line = format!("team {team}:");
            for (label, cell) in LABELS.iter().zip(bands) {
                out.insert((*label).to_string(), serde_json::json!({ "shots": cell[0], "hits": cell[1] }));
                if cell[0] > 0 {
                    line += &format!(" {label} {}/{} {:.1} %", cell[1], cell[0], 100.0 * f64::from(cell[1]) / f64::from(cell[0]));
                }
            }
            json.insert(format!("team{team}"), serde_json::Value::Object(out));
            text.push(line);
        }
        vec![(
            "shot_bands",
            Part::new(serde_json::json!({ "shot_bands": json }), format!("shot bands (hits/shots by the shooter's team): {}", text.join(" | "))),
        )]
    }
}

impl ShotBands {
    fn shot(&mut self, team: usize, distance: f32, hit: bool) {
        let at = EDGES.iter().position(|&edge| distance < edge).unwrap_or(EDGES.len());
        let cell = &mut self.bands[team][at];
        cell[0] += 1;
        cell[1] += u32::from(hit);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shot_goes_to_its_teams_band_of_distance() {
        let mut bands = ShotBands::default();
        bands.shot(0, 5.0, true);
        bands.shot(0, 40.0, false);
        bands.shot(1, 40.0, true);
        bands.shot(1, 120.0, false);
        assert_eq!(bands.bands[0][0], [1, 1]);
        assert_eq!(bands.bands[0][3], [1, 0]);
        assert_eq!(bands.bands[1][3], [1, 1]);
        assert_eq!(bands.bands[1][5], [1, 0]);
    }
}
