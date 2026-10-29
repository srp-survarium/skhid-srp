//! What a profile's kits hold in their pockets, read once: the one reader of the pockets (slots 13..=18) for the bots'
//! drug count ([`super::stock::Pantry::of_profile`]) and for the live grenade list
//! (`live_grenades::KitPockets::of_profile`), so both go by the same store the profile was built with
//! (`loadouts::PocketStore`) and a team that plays the other store has the right prior.
//!
//! The client keeps a stack twice (`inventory::setup_from_profile` 0x2FCB70, PROVEN): the slot (`units`, what a
//! life starts with) and the item's storage (`stored`, the profile item's `condition_or_stack`), a spawn fills the
//! slot with `min(units, stored)` and the storage loses it, a death gives back what is left.

use survarium::v026g::dictionary::Dictionary;
use survarium::v026g::loadouts::POCKET_SLOTS;
use survarium::v026g::match_msgs::static_info::MatchProfile;

/// A kit's six pockets, by position in [`POCKET_SLOTS`].
pub type KitSlots = [Option<ProfilePocket>; POCKET_SLOTS.len()];

/// One filled pocket of a kit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProfilePocket {
    /// The kit slot it is in (one of [`POCKET_SLOTS`]).
    pub slot: u8,
    /// The dictionary's name of the item.
    pub name: &'static str,
    /// Units the slot starts a life with.
    pub units: u16,
    /// The item's storage behind the slot, for a stack (a unit item has none).
    pub stored: Option<u32>,
}

/// The pockets (by position in [`POCKET_SLOTS`]) the quick-use keys number, in order: those `held` says hold something.
/// The one home of the numbering: [`QUICK_USE_ORDER`] begins with the six pockets in position order (PROVEN, table
/// 0x925FD8), and the six keys reach the pockets first. **Which pockets count as held is the open question** (LIKELY the
/// kit's filled slots at the spawn, as the heal count goes; the live grenade list counts the units left now, so a spent
/// pocket closes the gap): each caller says it in `held`, and the client steps P6-1..P6-3 settle which is the client's.
pub fn quick_use_order(held: impl Fn(usize) -> bool) -> impl Iterator<Item = usize> {
    (0..POCKET_SLOTS.len()).filter(move |&i| held(i))
}

/// The quick-use key (0 on) of the pocket in `slot` of `kit`, numbering the pockets the profile fills ([`quick_use_order`]
/// with a pocket holding units at the spawn as held).
pub fn quick_use_of(kit: &KitSlots, slot: u8) -> Option<usize> {
    let at = POCKET_SLOTS.iter().position(|&s| s == slot)?;
    quick_use_order(|i| kit[i].is_some_and(|pocket| pocket.units > 0)).position(|i| i == at)
}

/// The pockets of every kit of a profile (the first kit's only, when asked).
#[derive(Clone, Debug, PartialEq)]
pub struct ProfilePockets {
    kits: Vec<KitSlots>,
}

impl ProfilePockets {
    /// The pockets of `profile`'s kits; `all_kits` off reads the first kit only. `None` for a profile with no kit.
    pub fn of_profile(profile: &MatchProfile, all_kits: bool) -> Option<Self> {
        let dictionary = Dictionary::shared();
        let kits: Vec<KitSlots> = profile
            .kits
            .iter()
            .take(if all_kits { usize::MAX } else { 1 })
            .map(|kit| {
                std::array::from_fn(|i| {
                    let slot = POCKET_SLOTS[i];
                    let (index, units) = kit.slots[usize::from(slot)]?;
                    let item = profile.items.get(usize::from(index))?;
                    let name = dictionary.item(item.dict_id)?.name.as_str();
                    Some(ProfilePocket {
                        slot,
                        name,
                        units,
                        stored: dictionary.is_stack(item.dict_id).then_some(item.condition_or_stack),
                    })
                })
            })
            .collect();
        (!kits.is_empty()).then_some(Self { kits })
    }

    /// The kits' pockets, in the profile's order.
    pub fn kits(&self) -> &[KitSlots] {
        &self.kits
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use survarium::v026g::loadouts::{
        BotEquipment, BotMask, BotRole, BotTier, PocketStore, RusherKit, bot_profile_stored,
    };

    fn support(store: PocketStore) -> MatchProfile {
        let equipment = BotEquipment { tier: BotTier::Mid, rusher: RusherKit::ALL[0], mask: BotMask::Table };
        bot_profile_stored(1, 0, BotRole::Support, equipment, store)
    }

    /// A profile's pockets carry the store it was built with: the slot's units and the storage behind it.
    #[test]
    fn the_pockets_of_a_profile_carry_its_store() {
        for (store, expect) in [(PocketStore::PerMatch, None), (PocketStore::PerLife, Some(100))] {
            let pockets = ProfilePockets::of_profile(&support(store), true).expect("a bot has kits");
            assert_eq!(pockets.kits().len(), 3);
            let held: Vec<&ProfilePocket> = pockets.kits()[0].iter().flatten().collect();
            assert!(!held.is_empty(), "the first kit has pockets");
            for pocket in held {
                let stored = pocket.stored.expect("a pocket stack has a storage");
                // The stack of two medkits: 2 in the slot, 100 behind it (or 2: the old kit).
                assert_eq!(stored, expect.unwrap_or_else(|| u32::from(pocket.units)), "{pocket:?}");
            }
        }
        let one = ProfilePockets::of_profile(&support(PocketStore::PerLife), false).unwrap();
        assert_eq!(one.kits().len(), 1);
        let mut none = support(PocketStore::PerLife);
        none.kits.clear();
        assert_eq!(ProfilePockets::of_profile(&none, true), None);
    }
}
