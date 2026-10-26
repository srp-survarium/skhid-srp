use survarium::v026g::item_params::{covered_slots, item_params};
use survarium::v026g::lobby::FULL_CONDITION;
use survarium::v026g::lobby::requests::Relocation;
use survarium::v026g::prices;

use super::*;
use crate::v026g::account::{Item, KitSlot};

const NOW: u64 = 1_000;

fn dict(name: &str) -> u16 {
    Dictionary::shared().item_named(name).unwrap().dict_id
}

/// Alice: preset 1 active, the starter kit in kit 1 (the TT-33 in slot 7,
/// 50 rounds in slot 8), the Mosin carbine, 10 of its rounds and the
/// PPSh-41 in the storage.
fn alice() -> Account {
    Account::new("Alice")
}

fn worn(account: &Account, kit: usize) -> Vec<(usize, u32, u32)> {
    account.kits[kit]
        .slots
        .iter()
        .enumerate()
        .filter_map(|(slot, held)| held.map(|held| (slot, held.item_id, held.count)))
        .collect()
}

fn all_worn(account: &Account) -> Vec<Vec<(usize, u32, u32)>> {
    (0..KITS).map(|kit| worn(account, kit)).collect()
}

fn put(account: &mut Account, kit: usize, slot: usize, item_id: u32, count: u32) {
    account.kits[kit].slots[slot] = Some(KitSlot { item_id, count });
}

fn owned_id(account: &Account, name: &str) -> u32 {
    account
        .inventory
        .iter()
        .find(|item| item.dict_id == dict(name))
        .unwrap()
        .item_id
}

/// What choosing may never change: the items owned (ids, types, units,
/// modifications), the next item id and the money.
fn owned(account: &Account) -> (Vec<Item>, u32, u32, u32) {
    (
        account.inventory.clone(),
        account.next_item_id,
        account.generic_money,
        account.premium_money,
    )
}

/// The Mosin carbine and its rounds into kit 2.
fn rifle_kit(account: &mut Account) {
    let rifle = owned_id(account, "mosin_nagant");
    let rounds = owned_id(account, "ammo_7.62x54r_old");
    put(account, 1, 10, rifle, 1);
    put(account, 1, 9, rounds, 10);
}

#[test]
fn a_new_account_wears_preset_1_and_the_others_are_empty() {
    let account = alice();
    assert_eq!(account.active_preset, 1);
    assert_eq!(preset(&account, 1).unwrap(), snapshot(&account));
    assert!(preset(&account, 1).unwrap().is_some());
    for number in 2..=5 {
        assert_eq!(preset(&account, number).unwrap(), None);
    }
    assert_eq!(preset(&account, 6), Err(PresetError::NoSuchPreset));
}

#[test]
fn choosing_a_preset_keeps_the_kits_with_the_one_left() {
    let mut account = alice();
    let starter = all_worn(&account);

    // An empty preset starts empty (the owner's of 2026-10-23): the kits
    // emptied, preset 1 keeps them, nothing owned changes.
    let before = account.clone();
    assert_eq!(choose(&mut account, 2, NOW).unwrap(), Applied { empty: 0 });
    assert_eq!(account.active_preset, 2);
    assert!(all_worn(&account).iter().all(Vec::is_empty));
    assert_eq!(snapshot(&account), None);
    assert_eq!(account.presets[0], snapshot(&before));
    assert_eq!(account.presets[1], None);
    assert_eq!(owned(&account), owned(&before));

    // Preset 2 changed by hand: the rifle in kit 2.
    rifle_kit(&mut account);
    let rifle = all_worn(&account);
    let before = owned(&account);
    let revisions: Vec<u32> = account.kits.iter().map(|kit| kit.revision).collect();

    // Back to 1: its kits on, 2 kept as it was left.
    assert_eq!(choose(&mut account, 1, NOW).unwrap(), Applied { empty: 0 });
    assert_eq!(account.active_preset, 1);
    assert_eq!(all_worn(&account), starter);
    assert_eq!(account.presets[0], None);
    assert_eq!(
        account.presets[1].unwrap().kits[1][10],
        Some(PresetSlot {
            item_id: owned_id(&account, "mosin_nagant"),
            dict_id: dict("mosin_nagant"),
            count: 1
        })
    );
    // Kits 1 and 2 changed, kit 3 (empty in both) did not.
    let now: Vec<u32> = account.kits.iter().map(|kit| kit.revision).collect();
    assert_eq!(now, [revisions[0] + 1, revisions[1] + 1, revisions[2]]);
    assert_eq!(owned(&account), before);
    // The kit played stays.
    assert_eq!(account.active_kit, 1);

    // And to 2 again.
    choose(&mut account, 2, NOW).unwrap();
    assert_eq!(all_worn(&account), rifle);
    assert_eq!(owned(&account), before);

    // The active one chosen again: nothing changes.
    let same = account.clone();
    assert_eq!(choose(&mut account, 2, NOW).unwrap(), Applied { empty: 0 });
    assert_eq!(account, same);
    assert_eq!(choose(&mut account, 0, NOW), Err(PresetError::NoSuchPreset));
    assert_eq!(choose(&mut account, 6, NOW), Err(PresetError::NoSuchPreset));
    assert_eq!(account, same);
}

/// Preset 5: the PPSh-41 alone in kit 3; preset 1 active again.
fn with_preset_5(account: &mut Account) {
    choose(account, 5, NOW).unwrap();
    for kit in &mut account.kits {
        kit.slots = [None; KIT_SLOTS];
    }
    let ppsh = owned_id(account, "ppsh");
    put(account, 2, 7, ppsh, 1);
    choose(account, 1, NOW).unwrap();
}

/// The kits after a change come back as they are after it: away to preset
/// 5 and back to 1.
fn kept(account: &mut Account, what: &str) {
    let after = all_worn(account);
    let kits = snapshot(account);
    choose(account, 5, NOW).unwrap();
    assert_ne!(all_worn(account), after, "{what}: preset 5 is other kits");
    choose(account, 1, NOW).unwrap();
    assert_eq!(all_worn(account), after, "{what}");
    assert_eq!(snapshot(account), kits, "{what}");
}

/// The active preset is the kits: whatever changes them changes it, with
/// nothing to save. Each way a kit changes, then away and back.
#[test]
fn every_change_of_the_kits_is_the_active_presets() {
    let mut account = alice();
    account.generic_money = 1_000_000;
    with_preset_5(&mut account);
    kept(&mut account, "as it starts");

    // A move (`46 00`): the rifle into kit 1's second weapon slot.
    let rifle = owned_id(&account, "mosin_nagant");
    let rounds = owned_id(&account, "ammo_7.62x54r_old");
    rules::move_items(
        &mut account,
        1,
        &[
            Relocation {
                item_id: rifle,
                dict_id: dict("mosin_nagant"),
                source_slot: INVENTORY_SLOT,
                target_slot: 10,
                count: 1,
                count_in_inventory: 0,
            },
            Relocation {
                item_id: rounds,
                dict_id: dict("ammo_7.62x54r_old"),
                source_slot: INVENTORY_SLOT,
                target_slot: 9,
                count: 10,
                count_in_inventory: 10,
            },
        ],
        NOW,
    )
    .unwrap();
    assert_eq!(
        account.kits[0].slots[10].map(|held| held.item_id),
        Some(rifle)
    );
    kept(&mut account, "a move");

    // The ammo window asking for more rounds than owned: bought, in the kit.
    let ammo = dict("ammo_7.62x25");
    let pistol_rounds = account.kits[0].slots[8].unwrap().item_id;
    rules::move_items(
        &mut account,
        1,
        &[
            Relocation {
                item_id: pistol_rounds,
                dict_id: ammo,
                source_slot: 8,
                target_slot: INVENTORY_SLOT,
                count: 50,
                count_in_inventory: 50,
            },
            Relocation {
                item_id: pistol_rounds,
                dict_id: ammo,
                source_slot: INVENTORY_SLOT,
                target_slot: 8,
                count: 80,
                count_in_inventory: 80,
            },
        ],
        NOW,
    )
    .unwrap();
    assert_eq!(account.kits[0].slots[8].unwrap().count, 80);
    kept(&mut account, "the ammo window");

    // A purchase dropped onto a slot, if one is sold at level 0.
    let dictionary = Dictionary::shared();
    let record = prices::price_list()
        .iter()
        .filter(|record| {
            record.faction_id != 0 && record.level == 0 && dictionary.fits(record.dict_id, 10)
        })
        .min_by_key(|record| record.cost);
    if let Some(record) = record {
        let bought = rules::buy(
            &mut account,
            record.dict_id,
            1,
            record.faction_id,
            10,
            1,
            NOW,
        )
        .unwrap();
        assert_eq!(
            account.kits[0].slots[10].map(|held| held.item_id),
            Some(bought[0].item_id)
        );
        kept(&mut account, "a purchase onto a slot");
    }

    // A match's rounds: the stack down under what the slot took.
    account.item_mut(pistol_rounds).unwrap().condition_or_stack = 30;
    account.sync_kits();
    assert_eq!(account.kits[0].slots[8].unwrap().count, 30);
    kept(&mut account, "rounds fired");

    // A sale of what a kit wears.
    let pistol = account.kits[0].slots[7].unwrap().item_id;
    rules::sell(&mut account, pistol, 1).unwrap();
    assert_eq!(account.kits[0].slots[7], None);
    kept(&mut account, "a sale");

    // A repair at load: grenades in two pockets merged.
    let grenades = account.add_item(dict("rgd-5"), 3);
    put(&mut account, 0, 13, grenades, 1);
    put(&mut account, 0, 15, grenades, 2);
    assert!(account.repair());
    assert_eq!(account.kits[0].slots[15], None);
    kept(&mut account, "a repair");
}

#[test]
fn a_sold_item_leaves_its_slot_empty() {
    let mut account = alice();
    let pistol = account.kits[0].slots[7].unwrap().item_id;
    let rounds = account.kits[0].slots[8].unwrap();
    starter_in_2(&mut account);
    rules::sell(&mut account, pistol, 1).unwrap();
    let before = owned(&account);

    assert_eq!(choose(&mut account, 2, NOW).unwrap(), Applied { empty: 1 });
    assert_eq!(account.kits[0].slots[7], None);
    assert_eq!(account.kits[0].slots[8], Some(rounds));
    // Nothing bought in its place.
    assert_eq!(owned(&account), before);
}

/// Preset 2 holds kit 1 as Alice starts; she plays preset 1 with nothing
/// worn.
fn starter_in_2(account: &mut Account) {
    account.presets[1] = snapshot(account);
    account.kits[0].slots = [None; KIT_SLOTS];
}

#[test]
fn a_stack_gets_what_is_owned_and_never_more() {
    let mut account = alice();
    let rounds = account.kits[0].slots[8].unwrap().item_id;
    let ammo = dict("ammo_7.62x25");
    starter_in_2(&mut account);

    // Fired since: 20 left of the 50 saved.
    account.item_mut(rounds).unwrap().condition_or_stack = 20;
    let before = owned(&account);
    assert_eq!(choose(&mut account, 2, NOW).unwrap(), Applied { empty: 0 });
    assert_eq!(account.kits[0].slots[8].unwrap().count, 20);
    assert_eq!(owned(&account), before);

    // Used up, then bought again (a new item): the stack owned now, at most
    // the 50 kept.
    choose(&mut account, 1, NOW).unwrap();
    account.presets[1].as_mut().unwrap().kits[0][8]
        .as_mut()
        .unwrap()
        .count = 50;
    account.inventory.retain(|item| item.item_id != rounds);
    let again = account.add_item(ammo, 30);
    let before = owned(&account);
    choose(&mut account, 2, NOW).unwrap();
    assert_eq!(
        account.kits[0].slots[8],
        Some(KitSlot {
            item_id: again,
            count: 30
        })
    );
    assert_eq!(owned(&account), before);

    // None owned: the slot stays empty, nothing is bought.
    choose(&mut account, 1, NOW).unwrap();
    account.inventory.retain(|item| item.item_id != again);
    let before = owned(&account);
    assert_eq!(choose(&mut account, 2, NOW).unwrap(), Applied { empty: 1 });
    assert_eq!(account.kits[0].slots[8], None);
    assert_eq!(owned(&account), before);
}

fn empty_preset() -> Preset {
    Preset {
        kits: [[None; KIT_SLOTS]; KITS],
    }
}

/// A preset whose data names what the account does not own (another
/// account's item id, a stack of a type it has none of): nothing comes of
/// it. Choosing makes no item, takes none and moves no money.
#[test]
fn choosing_makes_no_item_and_no_money() {
    let mut account = alice();
    let mut preset = empty_preset();
    // An item id no item of the account has, of a weapon's type.
    preset.kits[0][7] = Some(PresetSlot {
        item_id: 999,
        dict_id: dict("mosin_nagant"),
        count: 1,
    });
    // A stack of a type the account owns none of, more than it could own.
    preset.kits[0][13] = Some(PresetSlot {
        item_id: 998,
        dict_id: dict("medkit"),
        count: 1000,
    });
    // An owned item id named with another type: not that item.
    preset.kits[1][7] = Some(PresetSlot {
        item_id: 1,
        dict_id: dict("ppsh"),
        count: 1,
    });
    // An owned stack asked for more units than owned.
    preset.kits[2][8] = Some(PresetSlot {
        item_id: 2,
        dict_id: dict("ammo_7.62x25"),
        count: 5000,
    });
    account.presets[4] = Some(preset);
    let before = owned(&account);
    assert_eq!(choose(&mut account, 5, NOW).unwrap(), Applied { empty: 3 });
    assert!(worn(&account, 0).is_empty());
    assert!(worn(&account, 1).is_empty());
    assert_eq!(worn(&account, 2), [(8, 2, 50)]);
    assert_eq!(owned(&account), before);
    // Every slot of every kit names an owned item, at most its units.
    for kit in &account.kits {
        for held in kit.slots.iter().flatten() {
            let item = account.item(held.item_id).unwrap();
            assert!(held.count <= item.condition_or_stack.max(1));
        }
    }
}

#[test]
fn an_item_type_goes_into_one_pocket() {
    let mut account = alice();
    let grenades = account.add_item(dict("rgd-5"), 3);
    let booster = account.add_item(dict("speed_booster"), 2);
    let mut preset = empty_preset();
    // Kept from before the rule: grenades in two pockets, a booster in two.
    for (pocket, item_id, dict_id, count) in [
        (13, grenades, dict("rgd-5"), 1),
        (14, booster, dict("speed_booster"), 1),
        (15, grenades, dict("rgd-5"), 2),
        (16, booster, dict("speed_booster"), 1),
    ] {
        preset.kits[0][pocket] = Some(PresetSlot {
            item_id,
            dict_id,
            count,
        });
    }
    account.presets[1] = Some(preset);
    let before = owned(&account);
    assert_eq!(choose(&mut account, 2, NOW).unwrap(), Applied { empty: 2 });
    let pockets: Vec<Option<KitSlot>> = account.kits[0].slots[13..=16].to_vec();
    assert_eq!(
        pockets,
        [
            Some(KitSlot {
                item_id: grenades,
                count: 1
            }),
            Some(KitSlot {
                item_id: booster,
                count: 1
            }),
            None,
            None,
        ]
    );
    assert!(account.doubled_pockets(&account.kits[0]).is_empty());
    assert_eq!(owned(&account), before);
}

/// The Zubr UM-4 helmet has its mask built in: it covers the mask's slot.
#[test]
fn a_covering_item_takes_off_what_it_covers() {
    let zubr = Dictionary::shared()
        .items
        .iter()
        .find(|item| item.cfg_name.ends_with("/renesanse_helmet_6.options"))
        .unwrap()
        .dict_id;
    assert_eq!(covered_slots(zubr), [1]);
    let mut account = alice();
    let helmet = account.add_item(zubr, FULL_CONDITION);
    let balaclava = account.add_item(1810, FULL_CONDITION);

    // Preset 2 with the helmet; preset 1 wears the balaclava.
    choose(&mut account, 2, NOW).unwrap();
    put(&mut account, 0, 0, helmet, 1);
    choose(&mut account, 1, NOW).unwrap();
    put(&mut account, 0, 1, balaclava, 1);
    let before = owned(&account);
    assert_eq!(choose(&mut account, 2, NOW).unwrap(), Applied { empty: 0 });
    assert_eq!(
        account.kits[0].slots[0].map(|held| held.item_id),
        Some(helmet)
    );
    assert_eq!(account.kits[0].slots[1], None);
    assert!(account.covered_items(&account.kits[0]).is_empty());
    assert_eq!(owned(&account), before);

    // A preset holding both (data from before the rule): the kit rule keeps
    // one, the balaclava put on last takes the helmet off.
    let mut both = snapshot(&account).unwrap();
    both.kits[0][1] = Some(PresetSlot {
        item_id: balaclava,
        dict_id: 1810,
        count: 1,
    });
    account.presets[2] = Some(both);
    assert_eq!(choose(&mut account, 3, NOW).unwrap(), Applied { empty: 1 });
    assert_eq!(account.kits[0].slots[0], None);
    assert_eq!(
        account.kits[0].slots[1].map(|held| held.item_id),
        Some(balaclava)
    );
    assert!(account.covered_items(&account.kits[0]).is_empty());
    assert_eq!(owned(&account), before);
}

/// The highest `match_making_level` of an armour piece for the torso.
fn best_torso() -> (u16, u8) {
    let dictionary = Dictionary::shared();
    dictionary
        .items
        .iter()
        .filter(|item| !item.is_stack && dictionary.fits(item.dict_id, 2))
        .filter_map(|item| Some((item.dict_id, item_params(item.dict_id)?.match_making_level)))
        .max_by_key(|&(_, level)| level)
        .unwrap()
}

#[test]
fn a_presets_level_is_the_rooms_measure_of_the_kits_it_gives() {
    let mut account = alice();
    let starter = matchmaking::gear_level(&account, NOW);
    // The active one: the kits' own level.
    assert_eq!(level(&account, 1, NOW), Some(starter));

    // Preset 2: a heavy torso in kit 2 with the rifle, the highest kit that
    // plays.
    let (torso, torso_level) = best_torso();
    assert!(torso_level > starter);
    let piece = account.add_item(torso, FULL_CONDITION);
    choose(&mut account, 2, NOW).unwrap();
    rifle_kit(&mut account);
    put(&mut account, 1, 2, piece, 1);
    assert_eq!(matchmaking::gear_level(&account, NOW), torso_level);
    assert_eq!(level(&account, 2, NOW), Some(torso_level));
    choose(&mut account, 1, NOW).unwrap();
    assert_eq!(level(&account, 2, NOW), Some(torso_level));
    assert_eq!(level(&account, 1, NOW), Some(starter));

    // Preset 3: the torso in a kit that cannot play (no weapon).
    choose(&mut account, 3, NOW).unwrap();
    for kit in &mut account.kits {
        kit.slots = [None; KIT_SLOTS];
    }
    put(&mut account, 2, 2, piece, 1);
    choose(&mut account, 1, NOW).unwrap();
    assert_eq!(level(&account, 3, NOW), None);
    // An empty preset has none either.
    assert_eq!(level(&account, 4, NOW), None);

    // Sold since: the preset's level is that of what is left.
    rules::sell(&mut account, piece, 1).unwrap();
    assert_eq!(level(&account, 2, NOW), Some(starter));
    // Reading the levels changes nothing.
    let before = account.clone();
    assert_eq!(
        levels(&account, NOW),
        [Some(starter), Some(starter), None, None, None]
    );
    assert_eq!(account, before);
}

#[test]
fn a_loaded_account_has_an_active_preset_and_keeps_none_in_its_place() {
    let mut account = alice();
    account.active_preset = 9;
    account.presets[0] = Some(empty_preset());
    account.presets[3] = snapshot(&account);
    assert!(account.repair());
    assert_eq!(account.active_preset, 1);
    assert_eq!(account.presets[0], None);
    assert!(account.presets[3].is_some());
    assert!(!account.repair());
}

#[test]
fn a_join_offers_the_highest_preset_the_room_allows() {
    let levels = [Some(3), Some(9), Some(6), None, Some(9)];
    // A room of any max level: nothing.
    assert_eq!(offer(2, 0, &levels, true, 1), Offer::None);
    // Gear over the max: the highest preset at most the max.
    assert_eq!(
        offer(11, 7, &levels, true, 1),
        Offer::Fits {
            preset: 3,
            level: 6
        }
    );
    // Two alike: the lower number.
    assert_eq!(
        offer(12, 10, &levels, true, 1),
        Offer::Fits {
            preset: 2,
            level: 9
        }
    );
    // None fits: the refusal as before.
    assert_eq!(offer(11, 2, &levels, true, 1), Offer::None);
    // The room allows more than the kits: a higher preset that fits.
    assert_eq!(
        offer(4, 7, &levels, true, 1),
        Offer::Higher {
            preset: 3,
            level: 6
        }
    );
    assert_eq!(
        offer(4, 15, &levels, true, 1),
        Offer::Higher {
            preset: 2,
            level: 9
        }
    );
    // None higher than the gear: no popup.
    assert_eq!(offer(6, 7, &levels, true, 1), Offer::None);
    assert_eq!(offer(9, 15, &levels, true, 1), Offer::None);
    // Gear at the max is no refusal: only a higher preset is offered.
    assert_eq!(offer(7, 7, &levels, true, 1), Offer::None);
    // No preset saved: nothing.
    assert_eq!(offer(11, 7, &[None; PRESETS], true, 1), Offer::None);
}

/// The review's case: preset 2's kit 1 is empty and its kit 2 plays, and
/// kit 1 is the one played. Chosen ("Switch and enter"), the kit played is
/// kit 2, whose level is the preset's: the room's gear check and the kit
/// its match takes agree.
#[test]
fn a_chosen_preset_plays_a_kit_that_can_play() {
    let mut account = alice();
    choose(&mut account, 2, NOW).unwrap();
    account.kits[0].slots = [None; KIT_SLOTS];
    rifle_kit(&mut account);
    // Played by hand: kit 1, empty (the lobby lets a kit be chosen so).
    account.active_kit = 1;
    choose(&mut account, 1, NOW).unwrap();
    assert_eq!(account.active_kit, 1);
    let level_2 = level(&account, 2, NOW).unwrap();

    assert_eq!(choose(&mut account, 2, NOW).unwrap(), Applied { empty: 0 });
    assert_eq!(account.active_kit, 2);
    assert!(rules::check_play(&account, 2, NOW).is_ok());
    let kit = account.kit(2).unwrap();
    assert_eq!(matchmaking::kit_level(&account, kit), level_2);
    assert_eq!(matchmaking::gear_level(&account, NOW), level_2);

    // A kit played that can play stays played.
    account.active_kit = 3;
    choose(&mut account, 1, NOW).unwrap();
    assert_eq!(account.active_kit, 1);
    account.kits[1].slots = [None; KIT_SLOTS];
    choose(&mut account, 3, NOW).unwrap();
    rifle_kit(&mut account);
    let (pistol, rounds) = (
        owned_id(&account, "tt_33"),
        owned_id(&account, "ammo_7.62x25"),
    );
    put(&mut account, 0, 7, pistol, 1);
    put(&mut account, 0, 8, rounds, 50);
    choose(&mut account, 1, NOW).unwrap();
    assert_eq!(account.active_kit, 1);
    choose(&mut account, 3, NOW).unwrap();
    assert_eq!(account.active_kit, 1);
}

/// A slot kept with a preset whose item no longer fits it (the game's data
/// changed which slot it goes in; the repair at load takes such an item off
/// a kit, not off a snapshot): it stays empty, counted, and the preset can
/// still be chosen.
#[test]
fn an_item_that_no_longer_fits_its_slot_stays_out() {
    let mut account = alice();
    let mut preset = snapshot(&account).unwrap();
    let pistol = preset.kits[0][7].unwrap();
    preset.kits[0][7] = None;
    // The pistol kept in the helmet's slot.
    preset.kits[0][0] = Some(pistol);
    assert!(!Dictionary::shared().fits(pistol.dict_id, 0));
    account.presets[1] = Some(preset);
    let before = owned(&account);
    assert_eq!(choose(&mut account, 2, NOW).unwrap(), Applied { empty: 1 });
    assert_eq!(account.kits[0].slots[0], None);
    assert_eq!(account.kits[0].slots[7], None);
    assert!(account.kits[0].slots[8].is_some());
    assert_eq!(owned(&account), before);
}

/// An empty preset chosen starts empty (the owner's of 2026-10-23): the
/// three kits emptied, every item still owned as it was (a kit names owned
/// items; stacks whole), nothing made or sold; back to the preset left,
/// its kits exactly; the empty one chosen again, nothing changes.
#[test]
fn an_empty_preset_starts_empty_and_gives_everything_back() {
    let mut account = alice();
    rifle_kit(&mut account);
    let worn_before = all_worn(&account);
    let kits_before = account.kits.clone();
    let before = owned(&account);
    assert!(worn_before.iter().flatten().count() >= 4);

    assert_eq!(choose(&mut account, 4, NOW).unwrap(), Applied { empty: 0 });
    assert_eq!(account.active_preset, 4);
    assert!(all_worn(&account).iter().all(Vec::is_empty));
    assert_eq!(owned(&account), before);
    assert_eq!(snapshot(&account), None);
    assert!(account.presets[0].is_some());
    assert_eq!(level(&account, 4, NOW), None);
    // No kit can play: Play is refused (no weapon), the gear level is 0;
    // an empty preset is never offered. The join popup offers another
    // preset whose kits can play, if one fits the room, any max level too
    // ("Exit" / "Switch and enter"); the active one never.
    let levels = levels(&account, NOW);
    assert_eq!(levels[3], None);
    assert_eq!(levels[0], Some(0));
    assert!(!rules::kits_can_play(&account, NOW));
    assert_eq!(
        offer(0, 0, &levels, false, 4),
        Offer::Unfit {
            preset: 1,
            level: 0
        }
    );
    assert_eq!(
        offer(0, 3, &[None, Some(2), None, Some(9), Some(4)], false, 4),
        Offer::Unfit {
            preset: 2,
            level: 2
        }
    );
    assert_eq!(
        offer(0, 0, &[None, None, None, Some(9), None], false, 4),
        Offer::None
    );

    // Chosen again, or another empty one: nothing changes on the kits or
    // what is owned.
    let same = account.clone();
    assert_eq!(choose(&mut account, 4, NOW).unwrap(), Applied { empty: 0 });
    assert_eq!(account, same);
    choose(&mut account, 5, NOW).unwrap();
    assert!(all_worn(&account).iter().all(Vec::is_empty));
    assert_eq!(owned(&account), before);
    assert_eq!(account.presets[3], None);

    // Back to 1: its kits exactly, the kit played as it was.
    choose(&mut account, 1, NOW).unwrap();
    assert_eq!(all_worn(&account), worn_before);
    assert_eq!(
        account.kits.iter().map(|kit| kit.slots).collect::<Vec<_>>(),
        kits_before.iter().map(|kit| kit.slots).collect::<Vec<_>>()
    );
    assert_eq!(account.active_kit, kits_before[0].id);
    assert_eq!(owned(&account), before);
}

/// What the lobby gets to ask before a sale: each item a preset uses, with
/// its presets; an item sold is in none.
#[test]
fn the_items_the_presets_use_are_listed_for_the_sale_question() {
    let mut account = alice();
    let pistol = account.kits[0].slots[7].unwrap().item_id;
    let rounds = account.kits[0].slots[8].unwrap().item_id;
    assert_eq!(uses(&account), [(pistol, 1), (rounds, 1)]);
    // Preset 3 the starter plus the rifle kit, active; preset 1 left as
    // the starter.
    account.presets[2] = snapshot(&account);
    choose(&mut account, 3, NOW).unwrap();
    rifle_kit(&mut account);
    let rifle = owned_id(&account, "mosin_nagant");
    let rifle_rounds = owned_id(&account, "ammo_7.62x54r_old");
    let mut expected = vec![(pistol, 5), (rounds, 5), (rifle, 4), (rifle_rounds, 4)];
    expected.sort();
    assert_eq!(uses(&account), expected);
    // The pistol sold: in no preset.
    rules::sell(&mut account, pistol, 1).unwrap();
    assert!(!uses(&account).iter().any(|(item, _)| *item == pistol));
    // An empty preset uses nothing.
    choose(&mut account, 2, NOW).unwrap();
    assert!(uses(&account).iter().all(|(_, presets)| presets & 2 == 0));
}

/// Demo: the 15 gear-level kits as the five presets, three kits each;
/// every kit plays, at the level it was made for (its level as the lobby
/// counts it: the highest piece's), and a second run gives nothing more.
#[test]
fn the_demo_puts_the_fifteen_gear_levels_on_the_presets() {
    use survarium::v026g::loadouts::player_gear_kit_items;

    let mut account = alice();
    let (pieces, units) = demo_gear_presets(&mut account, NOW).unwrap();
    assert!(pieces > 20 && units > 0, "{pieces} {units}");
    assert_eq!(account.active_preset, 1);
    for number in 1..=PRESETS as u8 {
        choose(&mut account, number, NOW).unwrap();
        for (k, kit) in account.kits.iter().enumerate() {
            let level = (number - 1) * 3 + k as u8 + 1;
            rules::check_play(&account, kit.id, NOW)
                .unwrap_or_else(|why| panic!("preset {number} kit {k}: {why:?}"));
            let wanted = player_gear_kit_items(level).unwrap();
            assert_eq!(
                kit.slots.iter().flatten().count(),
                wanted.len(),
                "level {level}"
            );
            let highest = wanted
                .iter()
                .map(|item| item_params(dict(item.name)).unwrap().match_making_level)
                .max()
                .unwrap();
            assert_eq!(
                matchmaking::kit_level(&account, kit),
                highest,
                "level {level}"
            );
        }
    }
    assert_eq!(demo_gear_presets(&mut account, NOW).unwrap(), (0, 0));
    assert!(!account.repair());
}
