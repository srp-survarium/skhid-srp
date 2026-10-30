//! The owner counts of each team's shots at its target by the target's distance (the bench's reading of a switch of the
//! bots' aim by side, `lead_flight`): shots and the hits among them, by the shooter's team and the horizontal distance band.
//! Counts only: nothing decides by them.
//!
//! Key of the `owner` object `shot_bands`: `"team<t>"` -> `"<band>"` (`0-10`, `10-20`, `20-35`, `35-50`, `50-80`, `80+` m) ->
//! `shots` (at the shooter's target) and `hits` (the shot hit that target), and for a target crossing the line at 1.5 m/s or more
//! `strafer_shots`, `strafer_hits`, `strafer_lead_sum` (how far ahead of him, in his direction of crossing, the shot line passed his chest
//! at the shot, metres, summed) and `strafer_cross_sum` (his crossing speed, m/s, summed).

use super::*;

/// The bands' upper bounds, metres.
const EDGES: [f32; 5] = [10.0, 20.0, 35.0, 50.0, 80.0];
const LABELS: [&str; 6] = ["0-10", "10-20", "20-35", "35-50", "50-80", "80+"];

#[derive(Default)]
pub(super) struct ShotBands {
    /// By team, by band: shots, hits.
    bands: [[[u32; 2]; 6]; 2],
    /// By team, by band, for a target crossing at [`STRAFER_MPS`] or more: shots, hits, the lead's sum, the crossing speed's sum.
    strafers: [[[f64; 4]; 6]; 2],
    /// By team, by band, for every shot at its target with a line: shots, the vertical offset's sum (m, up positive), the lateral
    /// offset's absolute sum (m).
    offsets: [[[f64; 3]; 6]; 2],
}

/// A target crossing the line of fire at this speed or more is a strafer, m/s.
const STRAFER_MPS: f32 = 1.5;

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
                if let Some(line) = fact.offset.and_then(|o| o.line) {
                    let at = EDGES.iter().position(|&edge| distance < edge).unwrap_or(EDGES.len());
                    let cell = &mut self.offsets[team][at];
                    cell[0] += 1.0;
                    cell[1] += f64::from(line.vertical);
                    cell[2] += f64::from(line.lateral.abs());
                }
                // How the target crossed the line (to the shooter's right positive) and how far ahead of him the shot passed.
                if let (Some(t), Some(s), Some(line)) =
                    (game.bodies.get(target), game.bodies.get(fact.shooter), fact.offset.and_then(|o| o.line))
                {
                    let velocity = t.step_vec * (1.0 / TICK_S);
                    let to = Vec3::new(t.feet.x - s.feet.x, 0.0, t.feet.z - s.feet.z);
                    let length = to.length();
                    if length > 1e-3 {
                        let cross = (velocity.x * to.z - velocity.z * to.x) / length;
                        if cross.abs() >= STRAFER_MPS {
                            self.strafer(team, distance, fact.hit == Some(target), line.lateral * cross.signum(), cross.abs());
                        }
                    }
                }
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
                let at = LABELS.iter().position(|l| l == label).unwrap_or(0);
                let (strafer, offset) = (self.strafers[team][at], self.offsets[team][at]);
                out.insert(
                    (*label).to_string(),
                    serde_json::json!({
                        "shots": cell[0], "hits": cell[1],
                        "line_shots": offset[0], "line_vertical_sum": offset[1], "line_lateral_abs_sum": offset[2],
                        "strafer_shots": strafer[0], "strafer_hits": strafer[1],
                        "strafer_lead_sum": strafer[2], "strafer_cross_sum": strafer[3],
                    }),
                );
                if cell[0] > 0 {
                    line += &format!(" {label} {}/{} {:.1} %", cell[1], cell[0], 100.0 * f64::from(cell[1]) / f64::from(cell[0]));
                }
                if offset[0] > 0.0 {
                    line += &format!(" [line {:+.2} m up, {:.2} m across]", offset[1] / offset[0], offset[2] / offset[0]);
                }
                if strafer[0] > 0.0 {
                    line += &format!(
                        " [strafers {:.0}, {:.1} % hit, lead {:+.2} m at {:.1} m/s]",
                        strafer[0],
                        100.0 * strafer[1] / strafer[0],
                        strafer[2] / strafer[0],
                        strafer[3] / strafer[0]
                    );
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
    fn strafer(&mut self, team: usize, distance: f32, hit: bool, lead: f32, cross: f32) {
        let at = EDGES.iter().position(|&edge| distance < edge).unwrap_or(EDGES.len());
        let cell = &mut self.strafers[team][at];
        cell[0] += 1.0;
        cell[1] += f64::from(u8::from(hit));
        cell[2] += f64::from(lead);
        cell[3] += f64::from(cross);
    }

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
