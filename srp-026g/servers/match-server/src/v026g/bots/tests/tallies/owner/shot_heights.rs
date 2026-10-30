//! The owner counts of how high the shooters' view pointed over the line to the target's chest at the shot (register #28: the
//! +0.12 degrees at every skill that the recoil does not explain), split by where the shot stands in its burst and by the target's
//! stance: the first shot of a run has no kick on the view yet, a later one has what the pull did not take back. Counts only:
//! nothing decides by them.
//!
//! Key of the `owner` object `shot_heights`: `"first"` (no shot of the shooter in the 500 ms before) and `"later"` ->
//! `"stand"`, `"crouch"` (the target's, `Stance`) and `"air"` (a target in a jump, whatever the stance) -> `"<band>"` (the
//! horizontal distance: `0-20`, `20-40`, `40+` m) -> `shots`, `hits`, `high_sum` (the sum of how high the view pointed over the chest
//! line before the cone, degrees, positive up: `Rise::high_deg`, the shots the target is known for) and `high_shots` (their count),
//! `line_vertical_sum` and `line_shots` (the sum of the shot line's own height over the chest at the target, metres, with the cone).

use super::*;
use crate::v026g::bots::tests::tallies::shot_offset::Stance;

/// A shot this many ms (the stand-in's ticks count ms) after the shooter's last is the first of a run: the kick has returned.
const RUN_GAP_MS: u32 = 500;

/// The bands' upper bounds, metres.
const EDGES: [f32; 2] = [20.0, 40.0];
const BANDS: [&str; 3] = ["0-20", "20-40", "40+"];
const POSITIONS: [&str; 2] = ["first", "later"];
const STANCES: [&str; 3] = ["stand", "crouch", "air"];

/// Whether a shot at `tick` is the first of a run, the shooter's last shot having been at `last`.
fn first_of_run(last: Option<u32>, tick: u32) -> bool {
    last.is_none_or(|last| tick >= last + RUN_GAP_MS)
}

#[derive(Clone, Copy, Default)]
struct Cell {
    shots: u32,
    hits: u32,
    high_sum: f64,
    high_shots: u32,
    line_vertical_sum: f64,
    line_shots: u32,
}

#[derive(Default)]
pub(super) struct ShotHeights {
    /// By burst position, target stance, band.
    cells: [[[Cell; 3]; 3]; 2],
    /// The tick of each shooter's last shot.
    last: Vec<Option<u32>>,
}

impl Topic for ShotHeights {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    /// The shots the stand-in fired this tick.
    fn begin(&mut self, _run: &Run, game: &mut Game) {
        for fact in &game.shot_facts {
            if self.last.len() <= fact.shooter {
                self.last.resize(fact.shooter + 1, None);
            }
            let first = first_of_run(self.last[fact.shooter], fact.tick);
            self.last[fact.shooter] = Some(fact.tick);
            let Some((target, distance, _)) = fact.target else { continue };
            let in_air = game.bodies.get(target).is_some_and(|t| t.air_until > fact.tick || t.flight.is_some());
            let stance = match fact.offset.map(|o| o.stance) {
                _ if in_air => 2,
                Some(Stance::Crouched) => 1,
                _ => 0,
            };
            let band = EDGES.iter().position(|&edge| distance < edge).unwrap_or(EDGES.len());
            let cell = &mut self.cells[usize::from(!first)][stance][band];
            cell.shots += 1;
            cell.hits += u32::from(fact.hit == Some(target));
            if let Some(rise) = fact.rise {
                cell.high_sum += f64::from(rise.high_deg);
                cell.high_shots += 1;
            }
            if let Some(line) = fact.offset.and_then(|o| o.line) {
                cell.line_vertical_sum += f64::from(line.vertical);
                cell.line_shots += 1;
            }
        }
    }

    fn parts(&mut self, _run: &Run, _game: &Game, _scale: &Scale, _shared: &Shared) -> Vec<(&'static str, Part)> {
        if self.cells.iter().flatten().flatten().all(|cell| cell.shots == 0) {
            return Vec::new();
        }
        let mut json = serde_json::Map::new();
        let mut text = Vec::new();
        for (position, label) in POSITIONS.iter().enumerate() {
            let mut by_stance = serde_json::Map::new();
            let mut line = format!("{label}:");
            for (stance, stance_label) in STANCES.iter().enumerate() {
                let mut by_band = serde_json::Map::new();
                let mut pooled = Cell::default();
                for (band, band_label) in BANDS.iter().enumerate() {
                    let cell = self.cells[position][stance][band];
                    by_band.insert(
                        (*band_label).to_string(),
                        serde_json::json!({
                            "shots": cell.shots, "hits": cell.hits,
                            "high_sum": cell.high_sum, "high_shots": cell.high_shots,
                            "line_vertical_sum": cell.line_vertical_sum, "line_shots": cell.line_shots,
                        }),
                    );
                    pooled.shots += cell.shots;
                    pooled.hits += cell.hits;
                    pooled.high_sum += cell.high_sum;
                    pooled.high_shots += cell.high_shots;
                }
                by_stance.insert((*stance_label).to_string(), serde_json::Value::Object(by_band));
                if pooled.high_shots > 0 {
                    line += &format!(
                        " {stance_label} {} shots {:.1} % hit {:+.2} deg;",
                        pooled.shots,
                        100.0 * f64::from(pooled.hits) / f64::from(pooled.shots),
                        pooled.high_sum / f64::from(pooled.high_shots)
                    );
                }
            }
            json.insert((*label).to_string(), serde_json::Value::Object(by_stance));
            text.push(line);
        }
        vec![(
            "shot_heights",
            Part::new(
                serde_json::json!({ "shot_heights": json }),
                format!("shot heights (view over the chest line by burst position and target stance): {}", text.join(" ")),
            ),
        )]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shot_after_a_pause_is_the_first_of_its_run() {
        assert!(first_of_run(None, 5));
        assert!(!first_of_run(Some(5), 5 + RUN_GAP_MS - 1));
        assert!(first_of_run(Some(5), 5 + RUN_GAP_MS));
        // The stand-in's ticks are ms: two shots of a rifle (about 100 ms apart) are one run.
        assert!(!first_of_run(Some(1_000), 1_100));
    }
}
