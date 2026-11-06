//! Keeper of the Artifacts' match draw (docs/notes/artefact-skills.md "Keeper of the
//! Artifacts"): srp draws one kind a match, uniformly over the kinds Keeper gives
//! ([`artefact_carry::keeper::KINDS`], the one table the game takes them by too), and writes it
//! into modifier 40 bits 9..11 of every kit of every player of the match, bots too, with the
//! skill or without: every game knows the match's kind from the static info, and exe patch 109
//! gives a Keeper's first spawn a dormant artefact of it from the map's hidden Keeper pool.

pub use artefact_carry::keeper::{KINDS, Kind};

use super::dictionary::Dictionary;
use super::game_data::{GameData, GameDataError};
use super::match_msgs::static_info::MatchProfile;
use super::skills::modifier_index;

/// Modifier 40's bits that hold the draw.
const SHIFT: u32 = 9;
const MASK: u32 = 7 << SHIFT;

/// The match's kind of a random number: each kind for an equal share of the u32s but the
/// last `u32::MAX % 3 + 1`, which a draw never sees in practice.
pub fn draw(random: u32) -> Kind {
    KINDS[(random % KINDS.len() as u32) as usize]
}

/// Modifier 40 (`artefact_power`, the artefact tree's packed skills) of `modifiers` with
/// `kind`'s draw in bits 9..11, its other bits as the skills gave them.
pub fn with_draw(artefact_power: f32, kind: Kind) -> f32 {
    // The skills tree sums whole bits (an exact integer below 2^24); bits 9..11 are the draw's.
    let skills = artefact_power as u32 & !MASK;
    (skills | u32::from(kind.draw()) << SHIFT) as f32
}

/// Every kit of `profile` given the match's `kind`.
pub fn give(profile: &mut MatchProfile, kind: Kind) {
    let at = modifier_index("artefact_power").expect("artefact_power is a modifier");
    for kit in &mut profile.kits {
        kit.modifiers[at] = with_draw(kit.modifiers[at], kind);
    }
}

/// The game data's dictionary names each kind's item as the table does; srp refuses to start
/// otherwise (the game would cook another item for the pool).
pub fn check(data: &GameData) -> Result<(), GameDataError> {
    let wrong: Vec<String> = KINDS
        .iter()
        .filter(|kind| {
            Dictionary::shared().item(kind.dict_id()).is_none_or(|item| item.name != kind.item_name())
        })
        .map(|kind| format!("{} is not dictionary item {} ({})", kind.name(), kind.dict_id(), kind.item_name()))
        .collect();
    if wrong.is_empty() {
        return Ok(());
    }
    Err(GameDataError(format!("{}: Keeper of the Artifacts: {}", data.origin, wrong.join(", "))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_draw_is_uniform_and_keeps_the_skills() {
        let mut seen = [0u32; 3];
        for random in 0..300 {
            seen[usize::from(draw(random).draw() - 1)] += 1;
        }
        assert_eq!(seen, [100, 100, 100]);
        // Resistance 5, Keeper, Cascade (509) with each draw, an earlier draw replaced.
        for kind in KINDS {
            let value = with_draw(509.0, kind);
            assert_eq!(value as u32, 509 | u32::from(kind.draw()) << 9);
            assert_eq!(with_draw(with_draw(509.0, Kind::Spring), kind), value);
        }
        assert_eq!(with_draw(0.0, Kind::Larkspur), 512.0);
    }

    #[test]
    fn the_test_data_names_the_kinds() {
        let data = super::super::game_data::shared();
        check(data).unwrap();
    }
}
