//! The owner counts of what the stand-in's bullets and pellets did (`fact-standin-calibration.md` C1 and C2):
//! per shot at the shooter's target, by the target's distance band and the weapon's class, the bullets or pellets
//! fired, those that landed on an enemy and the part of the body the hit shape gave ([`Landed`]). With the
//! boxes volume (`standin_hitbox.rs`) a shotgun's shot is its ten pellets, each a ray, so `landed` counts
//! pellets as the game's statistics do; with the old disc (`SRP_BENCH_HIT_MODEL=off`) it counts the pellets the
//! stand-in let land after a centre-ray hit, and the parts are not known (drawn by chance, not recorded).
//! Counts only: it reads the shot's facts after the stand-in resolved the shot.
//!
//! Keys of the `owner` object:
//! - `hit_cells`: `"<band>:<class>"` (the bands and classes of `shot_off_cells`; only cells with a shot) ->
//!   `shots` (at a target), `fired` (bullets or pellets), `landed` (on an enemy, whoever it was) and `parts`
//!   (of those landed: torso, head, arm, leg);
//! - `hit_all`: the same over every shot, with or without a target: `shots`, `fired`, `landed`, `parts`.

use super::*;
use crate::v026g::bots::tests::tallies::fight_tally::Landed;
use crate::v026g::bots::tests::tallies::shot_offset::{DistBand, ShotClass};

/// The counts of a set of shots.
#[derive(Clone, Copy, Default)]
struct Counts {
    shots: u32,
    fired: u32,
    landed: u32,
    parts: [u32; 4],
}

impl Counts {
    fn add(&mut self, landed: Landed) {
        self.shots += 1;
        self.fired += u32::from(landed.fired);
        self.landed += u32::from(landed.on_enemy);
        for (sum, n) in self.parts.iter_mut().zip(landed.parts) {
            *sum += u32::from(n);
        }
    }

    fn json(&self) -> serde_json::Value {
        serde_json::json!({
            "shots": self.shots,
            "fired": self.fired,
            "landed": self.landed,
            "parts": self.parts,
        })
    }
}

#[derive(Default)]
pub(super) struct HitShapes {
    cells: BTreeMap<(DistBand, ShotClass), Counts>,
    all: Counts,
}

impl Topic for HitShapes {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    /// The shots the stand-in fired this tick (the bench takes them for the fight tally after the owner counts).
    fn begin(&mut self, _run: &Run, game: &mut Game) {
        for fact in &game.shot_facts {
            self.shot(fact.offset.map(|o| (o.band, o.class)), fact.landed);
        }
    }

    fn parts(&mut self, _run: &Run, _game: &Game, _scale: &Scale, _shared: &Shared) -> Vec<(&'static str, Part)> {
        vec![("hit_shapes", self.part())]
    }
}

impl HitShapes {
    /// One shot, at a target of this band and class or at none.
    fn shot(&mut self, at: Option<(DistBand, ShotClass)>, landed: Landed) {
        self.all.add(landed);
        if let Some(cell) = at {
            self.cells.entry(cell).or_default().add(landed);
        }
    }

    fn part(&self) -> Part {
        let cells: serde_json::Map<String, serde_json::Value> = self
            .cells
            .iter()
            .map(|(&(band, class), counts)| (format!("{}:{}", band.label(), class.label()), counts.json()))
            .collect();
        Part::new(
            serde_json::json!({ "hit_cells": cells, "hit_all": self.all.json() }),
            self.text(),
        )
    }

    /// One line: every shot's bullets or pellets fired and landed on an enemy, the shares of the parts, and by
    /// distance band (shots at a target) the landed per fired.
    fn text(&self) -> String {
        let all = &self.all;
        let parts: u32 = all.parts.iter().sum();
        let share = |i: usize| Scale::pct(f64::from(all.parts[i]), f64::from(parts));
        let mut by_band = String::new();
        for band in DistBand::ALL {
            let (mut fired, mut landed) = (0u32, 0u32);
            for (_, c) in self.cells.iter().filter(|((b, _), _)| *b == band) {
                fired += c.fired;
                landed += c.landed;
            }
            by_band += &format!(
                "; {} m: {}/{} {:.1}%",
                band.label(),
                landed,
                fired,
                Scale::pct(f64::from(landed), f64::from(fired))
            );
        }
        format!(
            "owner-test hit shapes: {} shots, {} bullets or pellets fired, {} landed on an enemy; parts torso {:.1}% head {:.1}% arm {:.1}% leg {:.1}%{}",
            all.shots,
            all.fired,
            all.landed,
            share(0),
            share(1),
            share(2),
            share(3),
            by_band
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn landed(fired: u8, on_enemy: u8, parts: [u8; 4]) -> Landed {
        Landed { fired, on_enemy, parts }
    }

    #[test]
    fn a_shot_lands_in_its_cell_and_in_the_totals() {
        let mut topic = HitShapes::default();
        let rifle = Some((DistBand::To20, ShotClass::Rifle));
        topic.shot(rifle, landed(1, 1, [1, 0, 0, 0]));
        topic.shot(rifle, landed(1, 0, [0; 4]));
        topic.shot(Some((DistBand::To10, ShotClass::Shotgun)), landed(10, 4, [2, 1, 1, 0]));
        // A shot with no target counts in the totals only.
        topic.shot(None, landed(1, 1, [0, 0, 0, 1]));

        let cell = topic.cells[&(DistBand::To20, ShotClass::Rifle)];
        assert_eq!((cell.shots, cell.fired, cell.landed, cell.parts), (2, 2, 1, [1, 0, 0, 0]));
        let part = topic.part();
        assert_eq!(part.json["hit_cells"]["0-10:shotgun"]["landed"], 4);
        assert_eq!(part.json["hit_cells"]["0-10:shotgun"]["parts"], serde_json::json!([2, 1, 1, 0]));
        assert_eq!(part.json["hit_all"]["shots"], 4);
        assert_eq!(part.json["hit_all"]["fired"], 13);
        assert_eq!(part.json["hit_all"]["landed"], 6);
        assert!(
            part.text.starts_with("owner-test hit shapes: 4 shots, 13 bullets or pellets fired, 6 landed on an enemy"),
            "{}",
            part.text
        );
    }
}
