//! Hidden Stash (524, docs/notes/artefact-skills.md "Hidden Stash"): one artefact past the kit's
//! containers, which exe patch 109 opens at a take and empties at death. The skill's edit-list
//! entry sets modifier 40 bit 14 ([`BIT`]); the hidden slot is one of the kit's six quick
//! positions (the owner, 2026-11-07), so srp keeps the bit only on a kit whose pockets and
//! containers are fewer than six ([`loadouts::quick_positions`]), decided per kit.

use super::loadouts;
use super::skills::{MODIFIERS, modifier_index};

/// Modifier 40's (`artefact_power`) bit of the skill.
pub const BIT: u32 = 1 << 14;

/// The quick positions pockets and artefact containers share (the lobby's `QUICK_POSITIONS`).
pub const QUICK_POSITIONS: u32 = 6;

/// A kit's modifiers from the skills' `modifiers`, given the modifications its worn items have
/// (their defaults and their own, `(id, defect)`): the skill's bit only where the kit's pockets
/// and containers leave a quick position free.
pub fn kit_modifiers(modifiers: &[f32; MODIFIERS], worn: &[(u32, bool)]) -> [f32; MODIFIERS] {
    let mut kit = *modifiers;
    let at = modifier_index("artefact_power").expect("artefact_power is a modifier");
    // The skills tree sums whole bits (an exact integer below 2^24).
    let bits = kit[at] as u32;
    if bits & BIT != 0 && loadouts::quick_positions(worn, modifiers) >= QUICK_POSITIONS {
        kit[at] = (bits & !BIT) as f32;
    }
    kit
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with(power: f32, pockets: f32, containers: f32) -> [f32; MODIFIERS] {
        let mut modifiers = [0.0; MODIFIERS];
        modifiers[modifier_index("artefact_power").unwrap()] = power;
        modifiers[modifier_index("pocket_slots").unwrap()] = pockets;
        modifiers[modifier_index("artefact_slots").unwrap()] = containers;
        modifiers
    }

    fn power(modifiers: &[f32; MODIFIERS]) -> u32 {
        modifiers[modifier_index("artefact_power").unwrap()] as u32
    }

    #[test]
    fn the_bit_stays_only_below_six_quick_positions() {
        // The player's own pocket (default.player: 1) and nothing worn: 1 + 0, the bit kept with
        // the other skills (Keeper, Resistance 2).
        let alone = with((BIT + 8 + 2) as f32, 0.0, 0.0);
        assert_eq!(power(&kit_modifiers(&alone, &[])), BIT + 8 + 2);
        // 1 + 3 pockets and 1 container: 5, kept; 1 + 3 and 2: 6, taken off, the rest kept.
        assert_eq!(power(&kit_modifiers(&with((BIT + 8) as f32, 3.0, 1.0), &[])), BIT + 8);
        assert_eq!(power(&kit_modifiers(&with((BIT + 8) as f32, 3.0, 2.0), &[])), 8);
        // Artifact Container (one pocket fewer, one container more) keeps the sum.
        assert_eq!(power(&kit_modifiers(&with(BIT as f32, 2.0, 2.0), &[])), BIT);
        assert_eq!(power(&kit_modifiers(&with(BIT as f32, 2.0, 3.0), &[])), 0);
        // Worn items' modifications (modifications.tsv: 1031 pockets +2, 1040 containers +1):
        // 1 + 2 + 1 = 4 kept; with 1030 (+1) and 1040 again 6, taken off.
        let skill = with(BIT as f32, 0.0, 0.0);
        assert_eq!(power(&kit_modifiers(&skill, &[(1031, false), (1040, false)])), BIT);
        let worn = [(1031, false), (1030, false), (1040, false), (1040, false)];
        assert_eq!(power(&kit_modifiers(&skill, &worn)), 0);
        // Without the skill nothing changes; the other modifiers never do.
        let none = with(8.0, 3.0, 2.0);
        assert_eq!(kit_modifiers(&none, &[]), none);
        let full = with(BIT as f32, 3.0, 2.0);
        let kit = kit_modifiers(&full, &[]);
        let at = modifier_index("artefact_power").unwrap();
        assert!((0..MODIFIERS).filter(|&n| n != at).all(|n| kit[n] == full[n]));
    }
}
