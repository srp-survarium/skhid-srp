//! Scarcity's ammo (`docs/notes/scarcity.md`): a life of a Scarcity match
//! starts with each gun's magazine and one magazine more, a machine gun's
//! magazine alone.
//!
//! The game gives a life its kit's slot counts (`inventory::setup_from_profile`
//! 0x2FCB70: each ammo slot `min(the kit's count, what is in store)`, the
//! store losing it; a gun loads its magazine and chamber from its stack when
//! it is first drawn, 0x2E27C0), and the server gives the kits in the static
//! info. So the match server cuts the kits it sends ([`ration`]) and the
//! clients, a late joiner, a replay and the server's own read of the
//! snapshots (`Supplies`) all see the same counts; no client code changes.
//! The store keeps the account's whole stack, so what a match used (what was
//! in store at the start less what is left) is what the cut kits fired, and
//! the pay and the autobuy charge that.

use super::dictionary::Dictionary;
use super::item_params::item_params;
use super::loadouts::bot_gear::category::MACHINE_GUNS;
use super::match_msgs::static_info::{MatchItem, MatchKit, MatchProfile};

/// The kit's weapon slots, slot 7's gun first.
pub const WEAPON_SLOTS: [usize; 2] = [7, 10];
/// The kit's ammo slots (the client's table at 0x9C2578), in order: each
/// gun's ammo is one pool over them (`session::v026g::rules`).
pub const AMMO_SLOTS: [usize; 8] = [8, 9, 11, 12, 19, 20, 21, 22];

/// The rounds a life gives a gun: its magazine (`parameters/
/// magazine_capacity`) and one magazine more; a machine gun (`items_dict`
/// category 16, the shop's machine guns) its magazine alone. 0 for an item
/// that is no gun of the data.
pub fn rounds_of(weapon: u16, dictionary: &Dictionary) -> u32 {
    let (Some(item), Some(params)) = (dictionary.item(weapon), item_params(weapon)) else {
        return 0;
    };
    let magazines = if item.category == MACHINE_GUNS { 1 } else { 2 };
    params.magazine_capacity * magazines
}

/// Every kit of `profile` cut to Scarcity's ammo ([`ration_kit`]).
pub fn ration(profile: &mut MatchProfile, dictionary: &Dictionary) {
    for kit in &mut profile.kits {
        ration_kit(kit, &profile.items, dictionary);
    }
}

/// `kit`'s ammo slots cut to its guns' rounds ([`rounds_of`]): the slots
/// in order, each ammo slot giving what is left of the rounds of the guns
/// it fits (both guns' when it fits both), at most its own count; a slot
/// left with none, or with ammo that fits neither gun, is emptied (a gun
/// picked up could have fired it, patch 85). Anything that is not ammo, and
/// every other slot, stays as it is.
pub fn ration_kit(kit: &mut MatchKit, items: &[MatchItem], dictionary: &Dictionary) {
    let dict_of = |slot: Option<(u16, u16)>| {
        slot.and_then(|(index, _)| items.get(usize::from(index))).map(|item| item.dict_id)
    };
    let mut guns: Vec<(u16, u32)> = WEAPON_SLOTS
        .iter()
        .filter_map(|&slot| dict_of(kit.slots[slot]))
        .map(|weapon| (weapon, rounds_of(weapon, dictionary)))
        .collect();
    for slot in AMMO_SLOTS {
        let Some((index, count)) = kit.slots[slot] else { continue };
        let Some(ammo) = dict_of(kit.slots[slot]) else { continue };
        if item_params(ammo).is_none_or(|params| params.kind != "ammo") {
            continue;
        }
        let mut given = 0u16;
        for (weapon, left) in &mut guns {
            if !dictionary.compatible(*weapon, ammo) {
                continue;
            }
            let take = (*left).min(u32::from(count - given)) as u16;
            given += take;
            *left -= u32::from(take);
        }
        kit.slots[slot] = (given > 0).then_some((index, given));
    }
}

#[cfg(test)]
mod tests {
    use super::super::match_msgs::static_info::{FULL_CONDITION, NO_PROTECTION};
    use super::*;

    fn id(name: &str) -> u16 {
        Dictionary::shared().item_named(name).unwrap().dict_id
    }

    /// A profile of one kit: `slots` as `(slot, item name, count)`, each
    /// name an item of its own.
    fn profile(slots: &[(usize, &str, u16)]) -> MatchProfile {
        let mut kit = MatchKit::empty(1, "kit");
        let mut items = Vec::new();
        for (index, &(slot, name, count)) in slots.iter().enumerate() {
            kit.slots[slot] = Some((index as u16, count));
            let dict_id = id(name);
            let stack = Dictionary::shared().is_stack(dict_id);
            items.push(MatchItem {
                instance_id: 100 + index as u32,
                dict_id,
                condition_or_stack: if stack { 1000 } else { FULL_CONDITION },
                repair_time: 0,
                reroll_time: 0,
                modifications: Vec::new(),
                upgrades: Vec::new(),
            });
        }
        MatchProfile {
            team: 0,
            name: "p".into(),
            clan_tag: String::new(),
            protection: NO_PROTECTION,
            kits: vec![kit],
            items,
            squad: 0,
        }
    }

    fn counts(profile: &MatchProfile) -> Vec<(usize, Option<u16>)> {
        AMMO_SLOTS.iter().map(|&slot| (slot, profile.kits[0].slots[slot].map(|(_, n)| n))).collect()
    }

    #[test]
    fn a_gun_gets_two_magazines_and_a_machine_gun_one() {
        let dictionary = Dictionary::shared();
        // The data's: the AK-74M's magazine 30, the TT-33's 8, the RPD
        // (a machine gun, category 16) 100, the RPK 45.
        assert_eq!(rounds_of(id("ak_74m"), dictionary), 60);
        assert_eq!(rounds_of(id("tt_33"), dictionary), 16);
        assert_eq!(rounds_of(id("rpd"), dictionary), 100);
        assert_eq!(rounds_of(id("rpk"), dictionary), 45);
        assert_eq!(rounds_of(id("ammo_5.45x39_7n10"), dictionary), 0);
    }

    #[test]
    fn a_rifle_and_a_pistol_each_get_their_own() {
        // The lobby's fill of 3 bags: 60 of 7N10 in slot 8, 50 of 7.62x25 in slot 9.
        let mut p = profile(&[
            (7, "ak_74m", 1),
            (10, "tt_33", 1),
            (8, "ammo_5.45x39_7n10", 60),
            (9, "ammo_7.62x25", 50),
        ]);
        ration(&mut p, Dictionary::shared());
        assert_eq!(&counts(&p)[..2], &[(8, Some(60)), (9, Some(16))]);
        assert_eq!(p.kits[0].slots[7], Some((0, 1)));
        assert_eq!(p.kits[0].slots[10], Some((1, 1)));
    }

    #[test]
    fn a_machine_gun_gets_its_belt_alone() {
        let mut p = profile(&[(7, "rpd", 1), (8, "ammo_7.62x39_m", 300)]);
        ration(&mut p, Dictionary::shared());
        assert_eq!(counts(&p)[0], (8, Some(100)));
    }

    #[test]
    fn a_guns_stacks_share_its_rounds_in_slot_order_and_the_rest_are_emptied() {
        // Two kinds of the AK's ammo, a stack no gun takes, and a pocket.
        let mut p = profile(&[
            (7, "ak_74m", 1),
            (8, "ammo_5.45x39_7n10", 45),
            (9, "ammo_5.45x39_7n22", 90),
            (11, "ammo_9x18_makarov", 50),
            (12, "ammo_5.45x39_7n10", 30),
        ]);
        ration(&mut p, Dictionary::shared());
        assert_eq!(&counts(&p)[..4], &[(8, Some(45)), (9, Some(15)), (11, None), (12, None)]);
    }

    #[test]
    fn two_guns_of_one_calibre_share_one_pool() {
        // The PPSh-41 (35) and the TT-33 (8) both fire 7.62x25: 70 + 16.
        let mut p = profile(&[(7, "ppsh", 1), (10, "tt_33", 1), (8, "ammo_7.62x25", 200)]);
        ration(&mut p, Dictionary::shared());
        assert_eq!(counts(&p)[0], (8, Some(86)));
        // Less than both are given: all of it.
        let mut p = profile(&[(7, "ppsh", 1), (10, "tt_33", 1), (8, "ammo_7.62x25", 50)]);
        ration(&mut p, Dictionary::shared());
        assert_eq!(counts(&p)[0], (8, Some(50)));
    }

    #[test]
    fn every_gun_of_the_data_has_a_magazine() {
        let dictionary = Dictionary::shared();
        for item in dictionary.items.iter().filter(|item| super::super::weapon::CATEGORIES.contains(&item.category)) {
            assert!(rounds_of(item.dict_id, dictionary) > 0, "{}", item.name);
        }
    }
}
