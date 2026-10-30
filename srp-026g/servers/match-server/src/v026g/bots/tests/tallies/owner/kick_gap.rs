//! The owner counts of the kick the stand-in gives a shot against the bot's own mirror of it (register #28: the burst effect the first
//! shot does not have): the stand-in's kick at the shooter's first shot of a tick minus the mirror's (`Reflex::seen`: after the tick's
//! turn, before its shots), by the shot's place in its run and the distance to the target, and the ticks where the stand-in fired a
//! different number of shots than the mirror counted. Counts only: nothing decides by them.
//!
//! Key of the `owner` object `kick_gap`: `"first"` / `"later"` (as `shot_heights`) -> `"<band>"` (`0-20`, `20-40`, `40+` m, `none`: no
//! target) -> `pairs`, `gap_sum` (the sum of stand-in minus mirror, degrees of vertical kick: positive, the view is higher than the bot
//! thinks) and `gap_abs_sum`; and `ticks` -> `same` (as many shots), `fired_more`, `counted_more`, `fired_only` (the stand-in fired and
//! the mirror counted none), `counted_only`.

use super::shot_heights::first_of_run;
use super::*;
use crate::v026g::bots::recoil::TOP_DEG_PER_UNIT;

const EDGES: [f32; 2] = [20.0, 40.0];
const BANDS: [&str; 4] = ["0-20", "20-40", "40+", "none"];
const POSITIONS: [&str; 2] = ["first", "later"];

#[derive(Clone, Copy, Default)]
struct Cell {
    pairs: u32,
    gap_sum: f64,
    gap_abs_sum: f64,
}

#[derive(Default)]
pub(super) struct KickGap {
    cells: [[Cell; 4]; 2],
    /// Ticks by how the shot counts compare: same, fired more, counted more, fired only, counted only.
    ticks: [u32; 5],
    last: Vec<Option<u32>>,
}

/// How the shots the stand-in fired compare with the ones the mirror counted: an index of [`KickGap::ticks`].
fn compare(fired: u8, counted: u8) -> usize {
    match (fired, counted) {
        (f, c) if f == c => 0,
        (0, _) => 4,
        (_, 0) => 3,
        (f, c) if f > c => 1,
        _ => 2,
    }
}

impl Topic for KickGap {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn begin(&mut self, _run: &Run, game: &mut Game) {
        for pair in std::mem::take(&mut game.kick_pairs) {
            if self.last.len() <= pair.shooter {
                self.last.resize(pair.shooter + 1, None);
            }
            let first = first_of_run(self.last[pair.shooter], pair.tick);
            self.last[pair.shooter] = Some(pair.tick);
            let band = pair.distance.map_or(3, |d| EDGES.iter().position(|&edge| d < edge).unwrap_or(EDGES.len()));
            let gap = f64::from(pair.truth - pair.mirror) * f64::from(TOP_DEG_PER_UNIT);
            let cell = &mut self.cells[usize::from(!first)][band];
            cell.pairs += 1;
            cell.gap_sum += gap;
            cell.gap_abs_sum += gap.abs();
        }
        for tick in std::mem::take(&mut game.kick_ticks) {
            self.ticks[compare(tick.fired, tick.counted)] += 1;
        }
    }

    fn parts(&mut self, _run: &Run, _game: &Game, _scale: &Scale, _shared: &Shared) -> Vec<(&'static str, Part)> {
        if self.cells.iter().flatten().all(|cell| cell.pairs == 0) && self.ticks.iter().all(|&n| n == 0) {
            return Vec::new();
        }
        let mut json = serde_json::Map::new();
        let mut text = Vec::new();
        for (position, label) in POSITIONS.iter().enumerate() {
            let mut by_band = serde_json::Map::new();
            let mut line = format!("{label}:");
            for (band, band_label) in BANDS.iter().enumerate() {
                let cell = self.cells[position][band];
                by_band.insert(
                    (*band_label).to_string(),
                    serde_json::json!({ "pairs": cell.pairs, "gap_sum": cell.gap_sum, "gap_abs_sum": cell.gap_abs_sum }),
                );
                if cell.pairs > 0 {
                    line += &format!(" {band_label} {} shots {:+.3} deg;", cell.pairs, cell.gap_sum / f64::from(cell.pairs));
                }
            }
            json.insert((*label).to_string(), serde_json::Value::Object(by_band));
            text.push(line);
        }
        let [same, fired_more, counted_more, fired_only, counted_only] = self.ticks;
        json.insert(
            "ticks".to_string(),
            serde_json::json!({
                "same": same, "fired_more": fired_more, "counted_more": counted_more,
                "fired_only": fired_only, "counted_only": counted_only,
            }),
        );
        text.push(format!(
            "ticks with shots: same {same}, stand-in more {fired_more}, mirror more {counted_more}, stand-in only {fired_only}, mirror only {counted_only}"
        ));
        vec![(
            "kick_gap",
            Part::new(
                serde_json::json!({ "kick_gap": json }),
                format!("kick gap (stand-in's kick minus the bot's mirror at the first shot of a tick, vertical degrees): {}", text.join(" ")),
            ),
        )]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shot_counts_of_a_tick_are_compared() {
        assert_eq!(compare(1, 1), 0);
        assert_eq!(compare(0, 0), 0);
        assert_eq!(compare(2, 1), 1);
        assert_eq!(compare(1, 2), 2);
        assert_eq!(compare(1, 0), 3);
        assert_eq!(compare(0, 1), 4);
    }

    #[test]
    fn a_pair_goes_to_its_run_position_and_band_in_degrees() {
        let mut topic = KickGap::default();
        let mut game = Game::new(crate::v026g::bots::tests::soak_world(), &[0, 1], 1, None, None);
        game.kick_pairs.push(crate::v026g::bots::tests::tallies::fight_tally::KickPair {
            shooter: 0,
            tick: 1_000,
            truth: 0.02,
            mirror: 0.01,
            distance: Some(30.0),
        });
        topic.begin(&Run::new(2), &mut game);
        let cell = topic.cells[0][1];
        assert_eq!(cell.pairs, 1);
        assert!((cell.gap_sum - f64::from(0.01 * TOP_DEG_PER_UNIT)).abs() < 1e-5);
        assert!(game.kick_pairs.is_empty());
    }
}
