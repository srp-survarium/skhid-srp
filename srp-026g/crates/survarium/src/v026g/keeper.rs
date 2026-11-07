//! Keeper of the Artifacts' match draw (docs/notes/artefact-skills.md "Keeper of the
//! Artifacts"): srp draws one kind a match, each kind Keeper gives for its share of the matches
//! ([`artefact_carry::keeper::SHARES`] beside [`artefact_carry::keeper::KINDS`], the one table
//! the game takes them by too: Larkspur 45 %, Onyx 10 %, Spring 45 %), and writes it
//! into modifier 40 bits 9..11 of every kit of every player of the match, bots too, with the
//! skill or without: every game knows the match's kind from the static info, and exe patch 109
//! gives a Keeper a dormant artefact of it from the map's hidden Keeper pool. The kits of a joiner
//! taken once the match is in process (late: it could not choose its kit) also carry bit 13
//! ([`LATE`], [`give_late`]): its Keeper gets the gift after its first death, the others' (a
//! joiner taken in the countdown too) at the match's start.

pub use artefact_carry::keeper::{KINDS, Kind, SHARES};

use super::dictionary::Dictionary;
use super::game_data::{GameData, GameDataError};
use super::match_msgs::static_info::MatchProfile;
use super::skills::modifier_index;

/// Modifier 40's bits that hold the draw.
const SHIFT: u32 = 9;
const MASK: u32 = 7 << SHIFT;

/// Modifier 40's bit of a late joiner's kits (a profile taken into the match once in process).
pub const LATE: u32 = 1 << 13;

/// The match's kind of a random number: each kind for its [`SHARES`] of the u32s, by the roll
/// `random % 100` (the last `u32::MAX % 100 + 1` u32s favour the first kinds, by under one draw
/// in 40 million).
pub fn draw(random: u32) -> Kind {
    let total: u32 = SHARES.iter().map(|&(_, share)| share).sum();
    let mut roll = random % total;
    for (kind, share) in SHARES {
        if roll < share {
            return kind;
        }
        roll -= share;
    }
    unreachable!("a roll is below the shares' sum")
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

/// Every kit of a late joiner's `profile` given the match's `kind` and marked late ([`LATE`]).
pub fn give_late(profile: &mut MatchProfile, kind: Kind) {
    give(profile, kind);
    let at = modifier_index("artefact_power").expect("artefact_power is a modifier");
    for kit in &mut profile.kits {
        kit.modifiers[at] = (kit.modifiers[at] as u32 | LATE) as f32;
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

    /// How many draws of `randoms` give each kind, in [`KINDS`] order.
    fn tally(randoms: impl Iterator<Item = u32>) -> [u32; 3] {
        let mut seen = [0u32; 3];
        for random in randoms {
            seen[usize::from(draw(random).draw() - 1)] += 1;
        }
        seen
    }

    #[test]
    fn the_draw_gives_each_kind_its_share_and_keeps_the_skills() {
        // Every roll 0..100 once: exactly the shares (Larkspur, Onyx, Spring).
        assert_eq!(tally(0..100), [45, 10, 45]);
        // 100 000 matches of a seeded generator (splitmix64): each kind within half a point of
        // its share (Onyx's standard deviation is 0.09 points).
        let mut state = 0x5EED_u64;
        let randoms = std::iter::repeat_with(move || {
            state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = state;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            ((z ^ (z >> 31)) >> 32) as u32
        });
        let seen = tally(randoms.take(100_000));
        for (n, &(kind, share)) in SHARES.iter().enumerate() {
            let (name, percent) = (kind.name(), f64::from(seen[n]) / 1000.0);
            let near = (percent - f64::from(share)).abs() < 0.5;
            assert!(near, "{name}: {percent} %");
        }
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
