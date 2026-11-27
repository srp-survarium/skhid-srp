//! Patch 217: an additional model whose entry has `camo_available: true`
//! takes its item's camouflage (docs/notes/exo-suits.md, Camouflage).
//!
//! The character's cook (profile_skin_visual_cook::on_configs_loaded,
//! 0x204560) gives an item's own record its camouflage and its bake flag,
//! and its additional models' records neither (0x205520..0x205607):
//! bake_decals (0x129830) bakes only flagged records, so a camouflage
//! paints the item's first surface (`part_name`) alone. The exoskeleton
//! bodies (1805, 1807) draw their legs as an additional model, which stayed
//! bare under every camouflage (the owner, 2026-11-27: "applying camo to
//! exoskeleton doesn't paint legs").
//!
//! At `skin_cook.item_done` (0x20563D, a kit slot done) each of the item's
//! additional models whose entry says `camo_available: true` gets the
//! item's camouflage layer, the bake flag and a bake name of its own (the
//! camouflage's file name and the model's: `vostok::skin_cook::ItemDone`);
//! never the item's decals, which are placed on the item's own surface. The
//! bake is then the native's, as the item's: the surface's `_aoac` mask
//! beside its diffuse decides what is painted (none: the surface stays as
//! it is). An entry without the key, and every stock one, is as before.
//!
//! Nothing a match computes: the look only.

#![forbid(unsafe_code)]

use core::sync::atomic::{AtomicU32, Ordering};

use crate::vostok::config::{ConfigError, ConfigValue};
use crate::vostok::log::{self, Piece};
use crate::vostok::skin_cook::{ItemDone, Unpainted};
use crate::vostok::{Context, Flow, NoExit};

/// The cook's records (0x127C30 makes 20).
const PARTS: usize = 20;

/// An entry of an item's `additional_models`, as the native loop reads it
/// (0x205520..0x205611).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    /// A model: `base_model_hud` and `part_name_hud`, or `base_model`. The
    /// native makes it a record.
    Model { camouflage: bool },
    /// Neither: the native skips it (0x205596), no record.
    Skipped,
    /// No table: what the native makes of it is not known here.
    Unknown,
}

impl Shape {
    pub fn of(entry: Result<ConfigValue<'_>, ConfigError>) -> Shape {
        let Ok(ConfigValue::Table(entry)) = entry else { return Shape::Unknown };
        // the native's test is cfg's has (0xF6C00) alone
        let has = |name| !matches!(entry.get(name), Ok(None));
        if (has(c"base_model_hud") && has(c"part_name_hud")) || has(c"base_model") {
            Shape::Model { camouflage: matches!(entry.get(c"camo_available"), Ok(Some(ConfigValue::Boolean(true)))) }
        } else {
            Shape::Skipped
        }
    }
}

/// The additional models' records that take the camouflage, by their
/// index after the item's record (bit n: the nth).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Painted(u32);

impl Painted {
    pub fn indices(self) -> impl Iterator<Item = usize> {
        (0..PARTS).filter(move |&i| self.0 & (1 << i) != 0)
    }
}

/// What to do with an item's additional models.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Plan {
    /// Those records (none: nothing to do).
    Paint(Painted),
    /// An entry asks for the camouflage, but the entries do not tell which
    /// record is whose (one is no table, or they make another count of
    /// records than the cook has): nothing is painted.
    Unsure,
}

/// The plan for entries `shapes` whose models made `made` records.
pub fn plan(shapes: impl IntoIterator<Item = Shape>, made: usize) -> Plan {
    let (mut records, mut painted, mut asked, mut known) = (0usize, 0u32, false, true);
    for shape in shapes {
        match shape {
            Shape::Model { camouflage } => {
                if camouflage {
                    asked = true;
                    if records < PARTS {
                        painted |= 1 << records;
                    }
                }
                records += 1;
            }
            Shape::Skipped => {}
            Shape::Unknown => known = false,
        }
    }
    match (asked, known && records == made) {
        (false, _) => Plan::Paint(Painted(0)),
        (true, true) => Plan::Paint(Painted(painted)),
        (true, false) => Plan::Unsure,
    }
}

/// The run's refusals (a line at the 1st, 10th, 100th, ...).
static REFUSED: AtomicU32 = AtomicU32::new(0);

/// A kit slot done (0x20563D): its additional models that ask for it take
/// the item's camouflage.
pub fn on_item_done(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(mut cook) = ItemDone::at(ctx) else { return Flow::Continue };
    if cook.additional() == 0 || !cook.has_camouflage() {
        return Flow::Continue;
    }
    let Some(entry) = cook.entry() else { return Flow::Continue };
    let Ok(Some(models)) = entry.get(c"additional_models") else { return Flow::Continue };
    let Ok(models) = models.entries() else { return Flow::Continue };
    match plan(models.iter().map(Shape::of), cook.additional()) {
        Plan::Paint(painted) => {
            for index in painted.indices() {
                if let Err(why) = cook.paint(index) {
                    refused(match why {
                        Unpainted::NoRecord => "an additional model has no record",
                        Unpainted::NameFull => "the bake's name has no room",
                    });
                }
            }
        }
        Plan::Unsure => refused("its additional models do not tell which record is whose"),
    }
    Flow::Continue
}

fn refused(why: &str) {
    let n = REFUSED.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
    if log::is_power_of_ten(n) {
        log::line(&[Piece::Text("additional_camo: an item's camouflage not given to its additional models: "),
                    Piece::Text(why), Piece::Text("; refusal "), Piece::Dec(n)]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAINT: Shape = Shape::Model { camouflage: true };
    const BARE: Shape = Shape::Model { camouflage: false };

    fn records(plan: Plan) -> Option<[usize; 4]> {
        let Plan::Paint(painted) = plan else { return None };
        let mut out = [usize::MAX; 4];
        for (slot, i) in out.iter_mut().zip(painted.indices()) {
            *slot = i;
        }
        Some(out)
    }

    #[test]
    fn the_exoskeletons_legs() {
        // Duty: exoskeleton_s, back_s, sign_s, legs_s; Freedom: exoskeleton_s, back_s, legs_s
        assert_eq!(records(plan([BARE, BARE, BARE, PAINT], 4)), Some([3, usize::MAX, usize::MAX, usize::MAX]));
        assert_eq!(records(plan([BARE, BARE, PAINT], 3)), Some([2, usize::MAX, usize::MAX, usize::MAX]));
    }

    #[test]
    fn a_skipped_entry_makes_no_record() {
        assert_eq!(records(plan([BARE, Shape::Skipped, PAINT, PAINT], 3)), Some([1, 2, usize::MAX, usize::MAX]));
    }

    #[test]
    fn no_entry_asks() {
        assert_eq!(plan([BARE, BARE], 2), Plan::Paint(Painted(0)));
        // a count the entries do not explain is no matter when none asks
        assert_eq!(plan([BARE, Shape::Unknown], 5), Plan::Paint(Painted(0)));
        assert_eq!(plan([], 0), Plan::Paint(Painted(0)));
    }

    #[test]
    fn unsure_when_the_records_are_not_the_entries() {
        assert_eq!(plan([BARE, PAINT], 3), Plan::Unsure);
        assert_eq!(plan([PAINT, Shape::Unknown, BARE], 2), Plan::Unsure);
    }
}
