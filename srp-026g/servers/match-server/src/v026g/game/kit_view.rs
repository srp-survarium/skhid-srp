//! Which kits a spectator's game may show (`replay_control::KitsNote`): the game has every
//! kit's contents in its own copy of the match's options, and shows only those a note allows.
//!
//! - A replay's viewer: every kit a place can play, from the start ([`all_kits`]).
//! - A live match's spectator: only the kits a place has actually played by the spectator's
//!   clock: a kit counts once a trusted world has the player in it (`current_index`), not when it
//!   is picked (a kit picked and never spawned with stays unknown), and never one ahead of a
//!   delayed spectator's clock. [`KitsSeen`] keeps this for the whole match, so a spectator who
//!   comes late gets what was used before it came (the kept worlds last about a minute).

use super::Snapshot;
use replay_control::{KIT_NUMBERS, KitsNote};
use survarium::v026g::match_msgs::players::{self, PlayerState};
use survarium::v026g::match_msgs::static_info::{KIT_USABLE, MatchProfile};

/// `numbers`' playable kits of `profile` as a mask; `None` when there are none.
fn mask_of(profile: &MatchProfile, numbers: impl IntoIterator<Item = u8>) -> Option<u8> {
    let mask = numbers
        .into_iter()
        .filter(|&number| number < KIT_NUMBERS && profile.kits.get(usize::from(number)).is_some_and(|kit| kit.state == KIT_USABLE))
        .fold(0u8, |mask, number| mask | 1 << number);
    (mask != 0).then_some(mask)
}

/// A replay's: every place's mask of its playable kits. A place with none has no note.
pub(super) fn all_kits(profiles: &[MatchProfile]) -> Vec<KitsNote> {
    profiles
        .iter()
        .enumerate()
        .filter_map(|(place, profile)| {
            Some(KitsNote { place: place as u8, mask: mask_of(profile, 0..profile.kits.len().min(usize::from(KIT_NUMBERS)) as u8)? })
        })
        .collect()
}

/// The kits each place of a live match has played: for the whole match, its first trusted world
/// with each.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct KitsSeen {
    /// By place: each kit once, with the time of the first trusted world it was seen in.
    places: Vec<Vec<(u32, u8)>>,
}

impl KitsSeen {
    /// A trusted world: each player's current kit, where it is new for that place.
    pub(super) fn record(&mut self, world: &Snapshot, profiles: &[MatchProfile]) {
        let Some(blocks) = players::blocks(&world.body) else {
            return;
        };
        for block in blocks {
            let Some(profile) = profiles.get(usize::from(block.place)) else {
                continue;
            };
            if let Some(head) = PlayerState::read(block.state, profile.items.len()) {
                self.see(usize::from(block.place), world.time, head.current_index);
            }
        }
    }

    fn see(&mut self, place: usize, time: u32, kit: u8) {
        if place >= self.places.len() {
            self.places.resize_with(place + 1, Vec::new);
        }
        if !self.places[place].iter().any(|&(_, seen)| seen == kit) {
            self.places[place].push((time, kit));
        }
    }

    /// `place`'s note at `clock`: its playable kits played by then; `None` before there is one.
    pub(super) fn note_by(&self, place: usize, clock: u32, profiles: &[MatchProfile]) -> Option<KitsNote> {
        let played = self.places.get(place)?.iter().filter(|&&(time, _)| time <= clock).map(|&(_, kit)| kit);
        Some(KitsNote { place: place as u8, mask: mask_of(profiles.get(place)?, played)? })
    }
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
    fn replays_list_every_playable_kit_and_never_a_locked_one() {
        assert_eq!(all_kits(&[profile()]), vec![KitsNote { place: 0, mask: 0b011 }]);
    }

    #[test]
    fn a_kit_not_yet_reached_by_the_delayed_clock_is_not_told() {
        let profiles = [profile()];
        let mut seen = KitsSeen::default();
        seen.see(0, 1_000, 0);
        seen.see(0, 5_000, 1);
        assert_eq!(seen.note_by(0, 999, &profiles), None);
        assert_eq!(seen.note_by(0, 4_999, &profiles), Some(KitsNote { place: 0, mask: 0b01 }));
        assert_eq!(seen.note_by(0, 5_000, &profiles), Some(KitsNote { place: 0, mask: 0b11 }));
    }

    #[test]
    fn a_kit_seen_again_keeps_its_first_time() {
        let mut seen = KitsSeen::default();
        seen.see(1, 1_000, 0);
        seen.see(1, 9_000, 0);
        assert_eq!(seen.places[1], vec![(1_000, 0)]);
    }

    #[test]
    fn a_locked_kit_is_not_told_even_if_a_world_has_it() {
        let profiles = [profile()];
        let mut seen = KitsSeen::default();
        seen.see(0, 1_000, 2);
        assert_eq!(seen.note_by(0, 2_000, &profiles), None);
    }
}
