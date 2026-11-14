//! Which kits a spectator's game may show (`replay_control::KitsNote`): the game has every
//! kit's contents in its own copy of the match's options, and shows only those a note allows.
//! Every kit a place can play, from the start ([`all_kits`]): a replay's viewer and a live
//! match's spectator alike (the owner's of 2026-11-10: a live one was told only the kits a place
//! had played by its clock, which is gone).

use replay_control::{KIT_NUMBERS, KitsNote};
use survarium::v026g::match_msgs::static_info::{KIT_USABLE, MatchProfile};

/// `numbers`' playable kits of `profile` as a mask; `None` when there are none.
fn mask_of(profile: &MatchProfile, numbers: impl IntoIterator<Item = u8>) -> Option<u8> {
    let mask = numbers
        .into_iter()
        .filter(|&number| number < KIT_NUMBERS && profile.kits.get(usize::from(number)).is_some_and(|kit| kit.state == KIT_USABLE))
        .fold(0u8, |mask, number| mask | 1 << number);
    (mask != 0).then_some(mask)
}

/// Every place's mask of its playable kits. A place with none has no note.
pub(super) fn all_kits(profiles: &[MatchProfile]) -> Vec<KitsNote> {
    profiles
        .iter()
        .enumerate()
        .filter_map(|(place, profile)| {
            Some(KitsNote { place: place as u8, mask: mask_of(profile, 0..profile.kits.len().min(usize::from(KIT_NUMBERS)) as u8)? })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use survarium::dictionary::PROFILE_SLOTS;
    use survarium::v026g::match_msgs::static_info::MatchKit;
    use survarium::v026g::skills::MODIFIERS;

    /// A profile of three kits: kit 2 locked (the client's state 2).
    fn profile() -> MatchProfile {
        let kit = |id: u32, state: u8| MatchKit { state, id, name: String::new(), slots: [None; PROFILE_SLOTS], modifiers: [0.0; MODIFIERS] };
        MatchProfile {
            team: 0,
            name: "p".into(),
            clan_tag: String::new(),
            protection: 0,
            kits: vec![kit(1, KIT_USABLE), kit(2, KIT_USABLE), kit(3, 2)],
            items: Vec::new(),
            squad: 0,
        }
    }

    #[test]
    fn every_playable_kit_is_listed_and_never_a_locked_one() {
        assert_eq!(all_kits(&[profile()]), vec![KitsNote { place: 0, mask: 0b011 }]);
    }
}
