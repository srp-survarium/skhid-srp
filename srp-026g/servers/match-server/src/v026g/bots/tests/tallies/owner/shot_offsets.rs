//! The owner counts of where the stand-in's shots pass their target (`fact-standin-calibration.md` C0): every
//! shot at the shooter's target, by the target's distance band and the weapon's class, with how far the shot
//! line passed from his chest point ([`super::super::shot_offset`]). The stand-in's hit volume is a disc of
//! 0.45 m about the chest; the game's is the body's boxes. With the offsets in fixed bins, any other volume is
//! a sum over the same shots offline. Counts only: the stand-in resolves the shot as it did, and this reads the
//! shot's facts after.
//!
//! Keys of the `owner` object:
//! - `shot_off_cells`: `"<band>:<class>"` (bands `0-10 10-20 20-35 35-50 50-80 80+` m by the horizontal
//!   distance, classes of `Gun::class`; only cells with a shot) -> `shots` (all shots at a target), `no_line`
//!   (the chest not ahead of the eye or past the 100 m reach: no offset), `hits` (the shot hit the target),
//!   `disc45` (offset under 0.45 m: the stand-in's test, geometry alone), `on_body` (offset within 0.25 m) and
//!   `radial`, 21 counts of the offset: 20 bins of 5 cm up to 1 m, then everything farther;
//! - `shot_off_grid`: `"<band>:<stand|crouch>"` -> `beyond` (offset outside the grid) and `cells`: `"i,j"` ->
//!   shots, the cell `i` being the lateral offset from `-1.5 + 0.1 i` m (positive to the shooter's right) and
//!   `j` the vertical one from `-1.5 + 0.1 j` m (positive up).

use super::*;
use crate::v026g::bots::tests::tallies::shot_offset::{
    BODY_M, CELL_M, DISC_M, DistBand, GRID, RADIAL_BINS, ShotClass, ShotOffset, Stance,
};

/// One cell of the counts: a distance band, a weapon class.
#[derive(Clone, Copy, Default)]
struct Cell {
    shots: u32,
    no_line: u32,
    hits: u32,
    disc: u32,
    on_body: u32,
    radial: [u32; RADIAL_BINS + 1],
}

/// A distance band, a stance of the target: the grid of offsets.
struct Grid {
    beyond: u32,
    cells: [[u32; GRID]; GRID],
}

impl Default for Grid {
    fn default() -> Self {
        Self { beyond: 0, cells: [[0; GRID]; GRID] }
    }
}

#[derive(Default)]
pub(super) struct ShotOffsets {
    cells: BTreeMap<(DistBand, ShotClass), Cell>,
    grids: BTreeMap<(DistBand, Stance), Grid>,
}

impl Topic for ShotOffsets {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    /// The shots the stand-in fired this tick (the bench takes them for the fight tally after the owner counts).
    fn begin(&mut self, _run: &Run, game: &mut Game) {
        for fact in &game.shot_facts {
            if let (Some(offset), Some((target, ..))) = (fact.offset, fact.target) {
                self.shot(offset, fact.hit == Some(target));
            }
        }
    }

    fn parts(&mut self, _run: &Run, _game: &Game, _scale: &Scale, _shared: &Shared) -> Vec<(&'static str, Part)> {
        vec![("shot_offsets", self.part())]
    }
}

impl ShotOffsets {
    /// One shot at the shooter's target; `hit`: the shot hit that target.
    fn shot(&mut self, offset: ShotOffset, hit: bool) {
        let cell = self.cells.entry((offset.band, offset.class)).or_default();
        cell.shots += 1;
        cell.hits += u32::from(hit);
        let Some(line) = offset.line else {
            cell.no_line += 1;
            return;
        };
        cell.disc += u32::from(line.miss < DISC_M);
        cell.on_body += u32::from(line.miss <= BODY_M);
        cell.radial[line.radial_bin()] += 1;
        let grid = self.grids.entry((offset.band, offset.stance)).or_default();
        match line.cell() {
            Some((i, j)) => grid.cells[i][j] += 1,
            None => grid.beyond += 1,
        }
    }

    fn part(&self) -> Part {
        let mut cells = serde_json::Map::new();
        for (&(band, class), cell) in &self.cells {
            cells.insert(
                format!("{}:{}", band.label(), class.label()),
                serde_json::json!({
                    "shots": cell.shots,
                    "no_line": cell.no_line,
                    "hits": cell.hits,
                    "disc45": cell.disc,
                    "on_body": cell.on_body,
                    "radial": cell.radial,
                }),
            );
        }
        let mut grids = serde_json::Map::new();
        for (&(band, stance), grid) in &self.grids {
            let mut counts = serde_json::Map::new();
            for (i, column) in grid.cells.iter().enumerate() {
                for (j, &n) in column.iter().enumerate().filter(|(_, n)| **n > 0) {
                    counts.insert(format!("{i},{j}"), serde_json::json!(n));
                }
            }
            grids.insert(
                format!("{}:{}", band.label(), stance.label()),
                serde_json::json!({ "beyond": grid.beyond, "cells": counts }),
            );
        }
        let text = self.text();
        Part::new(
            serde_json::json!({ "shot_off_cells": cells, "shot_off_grid": grids }),
            text,
        )
    }

    /// One line: the shots at a target, and by distance band the shots, the hits, the offsets under the disc
    /// (0.45 m) and within the body (0.25 m), in percent of the shots with a line.
    fn text(&self) -> String {
        let mut all = Cell::default();
        let mut by_band = String::new();
        for band in DistBand::ALL {
            let mut b = Cell::default();
            for (_, cell) in self.cells.iter().filter(|((x, _), _)| *x == band) {
                b.shots += cell.shots;
                b.no_line += cell.no_line;
                b.hits += cell.hits;
                b.disc += cell.disc;
                b.on_body += cell.on_body;
            }
            all.shots += b.shots;
            all.no_line += b.no_line;
            all.hits += b.hits;
            all.disc += b.disc;
            all.on_body += b.on_body;
            let lined = b.shots - b.no_line;
            by_band += &format!(
                "; {} m: {} shots, hit {:.1}%, {:.2} m disc {:.1}%, within {:.2} m {:.1}%",
                band.label(),
                b.shots,
                Scale::pct(f64::from(b.hits), f64::from(b.shots)),
                DISC_M,
                Scale::pct(f64::from(b.disc), f64::from(lined)),
                BODY_M,
                Scale::pct(f64::from(b.on_body), f64::from(lined)),
            );
        }
        format!(
            "owner-test shot offsets: {} shots at a target ({} with no line), grid cells {:.1} m{}",
            all.shots, all.no_line, CELL_M, by_band
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shot_lands_in_its_band_class_bin_and_cell() {
        let mut topic = ShotOffsets::default();
        let eye = Vec3::new(0.0, 1.6, 0.0);
        let chest = Vec3::new(0.25, 1.45, 12.0);
        let shot = ShotOffset::of(eye, Vec3::new(0.0, 0.0, 1.0), chest, 12.0, ShotClass::Smg, true);
        topic.shot(shot, true);
        topic.shot(shot, false);
        // One shot with no line (the chest behind the eye).
        let behind = ShotOffset::of(eye, Vec3::new(0.0, 0.0, 1.0), Vec3::new(0.0, 1.5, -3.0), 3.0, ShotClass::Rifle, false);
        topic.shot(behind, false);

        let cell = topic.cells[&(DistBand::To20, ShotClass::Smg)];
        // The line is 0.25 m left of the chest and 0.15 m above it: miss 0.29 m, bin 5 of 0.05 m.
        assert_eq!((cell.shots, cell.hits, cell.no_line, cell.disc, cell.on_body), (2, 1, 0, 2, 0));
        assert_eq!(cell.radial[5], 2);
        assert_eq!(cell.radial.iter().sum::<u32>(), 2);
        // Lateral -0.25 m is cell 12 (-0.3 to -0.2 m), vertical +0.15 m is cell 16 (0.1 to 0.2 m).
        let grid = &topic.grids[&(DistBand::To20, Stance::Crouched)];
        let at: Vec<_> = (0..GRID)
            .flat_map(|i| (0..GRID).map(move |j| (i, j)))
            .filter(|&(i, j)| grid.cells[i][j] > 0)
            .collect();
        assert_eq!(at, [(12, 16)]);
        assert_eq!(grid.cells[12][16], 2);
        let behind = topic.cells[&(DistBand::To10, ShotClass::Rifle)];
        assert_eq!((behind.shots, behind.no_line, behind.radial.iter().sum::<u32>()), (1, 1, 0));
        assert!(!topic.grids.contains_key(&(DistBand::To10, Stance::Standing)));
    }

    #[test]
    fn a_standin_shot_at_the_target_is_counted_in_the_cell_of_its_line() {
        // Shooter 0 faces +Z level, its target 20 m ahead and 0.25 m to the right of the line: the line passes
        // 0.25 m left of his chest, and (the chest 1.30 m up, the eye higher) above it.
        let mut game = Game::new(crate::v026g::bots::tests::soak_world(), &[0, 1], 1, None, None);
        put(&mut game, 0, 10.0, 10.0, 0);
        put(&mut game, 1, 10.25, 30.0, 0);
        game.bodies[0].yaw = 0.0;
        game.bodies[0].pitch = 0.0;
        game.bodies[0].target = Some(1);
        game.shot(0, 0.0);
        let eye = Game::eye(&game.bodies[0]);
        let above = eye.y - 1.3;
        assert_eq!(game.shot_facts.len(), 1);
        let fact = game.shot_facts[0];
        let offset = fact.offset.expect("a shot at a target has its offset");
        let line = offset.line.expect("a line");
        assert!((line.lateral + 0.25).abs() < 1e-3, "{line:?}");
        assert!((line.vertical - above).abs() < 1e-3, "{line:?} {above}");
        assert_eq!((offset.band, offset.stance), (DistBand::To20, Stance::Standing));
        assert_eq!(offset.class, ShotClass::of(game.bodies[0].gun.class()));
        // The stand-in's own decision and the count of it agree: a hit is the disc, here also in the open.
        assert_eq!(fact.hit.is_some(), line.miss < DISC_M, "{line:?}");

        let mut topic = ShotOffsets::default();
        topic.begin(&Run::new(2), &mut game);
        let cell = topic.cells[&(DistBand::To20, offset.class)];
        assert_eq!((cell.shots, cell.hits, cell.no_line), (1, u32::from(fact.hit.is_some()), 0));
        assert_eq!(cell.radial[line.radial_bin()], 1);
        let grid = &topic.grids[&(DistBand::To20, Stance::Standing)];
        let (i, j) = line.cell().expect("in the grid");
        assert_eq!((i, grid.cells[i][j]), (12, 1));
        // The shot is still in the game's list for the fight tally that takes it after.
        assert_eq!(game.shot_facts.len(), 1);
    }

    #[test]
    fn the_parts_give_the_cells_and_the_sparse_grid() {
        let mut topic = ShotOffsets::default();
        let eye = Vec3::new(0.0, 1.6, 0.0);
        let shot = ShotOffset::of(eye, Vec3::new(0.0, 0.0, 1.0), Vec3::new(2.0, 1.3, 25.0), 25.0, ShotClass::Rifle, false);
        topic.shot(shot, false);
        let part = topic.part();
        let cell = &part.json["shot_off_cells"]["20-35:rifle"];
        assert_eq!(cell["shots"], 1);
        assert_eq!(cell["radial"][RADIAL_BINS], 1);
        assert_eq!(part.json["shot_off_grid"]["20-35:stand"]["beyond"], 1);
        assert!(part.text.starts_with("owner-test shot offsets: 1 shots at a target (0 with no line)"), "{}", part.text);
    }
}
