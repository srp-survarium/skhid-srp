//! The owner counts of the view's pitch (`pitch.rs`): every shot at the shooter's target by the height of his chest over
//! the shooter's eye, with how high above the line to his chest the view pointed (before the cone). A view with the pitch
//! unit taken for a radian points low at a target far below the eye and high at one far above it; the client's map
//! (`kin.pitch_scale`) takes that out. Counts only: nothing decides by them.
//!
//! Key of the `owner` object `pitch_bands`: `"<band>"` (the target's elevation over the eye, degrees: `-90..-30`,
//! `-30..-15`, `-15..-5`, `-5..5`, `5..15`, `15..30`, `30..90`) -> `shots`, `hits` (the shot hit the target),
//! `high_sum` (the sum of how high the view pointed, degrees, positive up) and `high_abs_sum`.

use super::*;

/// The bands' upper bounds, degrees.
const EDGES: [f32; 6] = [-30.0, -15.0, -5.0, 5.0, 15.0, 30.0];
const LABELS: [&str; 7] = ["-90..-30", "-30..-15", "-15..-5", "-5..5", "5..15", "15..30", "30..90"];

#[derive(Clone, Copy, Default)]
struct Band {
    shots: u32,
    hits: u32,
    high_sum: f64,
    high_abs_sum: f64,
}

#[derive(Default)]
pub(super) struct PitchBands {
    bands: [Band; 7],
}

impl Topic for PitchBands {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    /// The shots the stand-in fired this tick.
    fn begin(&mut self, _run: &Run, game: &mut Game) {
        for fact in &game.shot_facts {
            if let (Some(rise), Some((target, ..))) = (fact.rise, fact.target) {
                self.shot(rise.target_deg, rise.high_deg, fact.hit == Some(target));
            }
        }
    }

    fn parts(&mut self, _run: &Run, _game: &Game, _scale: &Scale, _shared: &Shared) -> Vec<(&'static str, Part)> {
        if self.bands.iter().all(|band| band.shots == 0) {
            return Vec::new();
        }
        let mut out = serde_json::Map::new();
        for (label, band) in LABELS.iter().zip(&self.bands) {
            out.insert(
                (*label).to_string(),
                serde_json::json!({
                    "shots": band.shots,
                    "hits": band.hits,
                    "high_sum": band.high_sum,
                    "high_abs_sum": band.high_abs_sum,
                }),
            );
        }
        let text = LABELS
            .iter()
            .zip(&self.bands)
            .filter(|(_, band)| band.shots > 0)
            .map(|(label, band)| {
                let n = f64::from(band.shots);
                format!(
                    "{label}: {} shots {:.1} % hit, view {:+.2} deg off the chest line",
                    band.shots,
                    100.0 * f64::from(band.hits) / n,
                    band.high_sum / n
                )
            })
            .collect::<Vec<_>>()
            .join("; ");
        vec![("pitch_bands", Part::new(serde_json::json!({ "pitch_bands": out }), format!("pitch bands: {text}")))]
    }
}

impl PitchBands {
    fn shot(&mut self, target_deg: f32, high_deg: f32, hit: bool) {
        let at = EDGES.iter().position(|&edge| target_deg < edge).unwrap_or(EDGES.len());
        let band = &mut self.bands[at];
        band.shots += 1;
        band.hits += u32::from(hit);
        band.high_sum += f64::from(high_deg);
        band.high_abs_sum += f64::from(high_deg.abs());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shot_goes_to_the_band_of_its_targets_height() {
        let mut bands = PitchBands::default();
        bands.shot(-20.0, -2.0, true);
        bands.shot(-20.0, -4.0, false);
        bands.shot(0.0, 0.5, true);
        bands.shot(45.0, 1.0, false);
        assert_eq!((bands.bands[1].shots, bands.bands[1].hits), (2, 1));
        assert_eq!(bands.bands[1].high_sum, -6.0);
        assert_eq!(bands.bands[3].shots, 1);
        assert_eq!(bands.bands[6].shots, 1);
    }
}
