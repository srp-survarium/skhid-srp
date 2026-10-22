use survarium::v026g::item_params::{covered_slots, item_params};
use survarium::v026g::lobby::FULL_CONDITION;

use super::*;
use crate::v026g::account::{Item, KitSlot};

const NOW: u64 = 1_000;

fn dict(name: &str) -> u16 {
    Dictionary::shared().item_named(name).unwrap().dict_id
}

/// Alice: the starter kit in kit 1 (the TT-33 in slot 7, 50 rounds in
/// slot 8), the Mosin carbine, 10 of its rounds and the PPSh-41 in the
/// storage.
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

fn put(account: &mut Account, kit: usize, slot: usize, item_id: u32, count: u32) {
    account.kits[kit].slots[slot] = Some(KitSlot { item_id, count });
}

/// What applying may never change: the items owned (ids, types, units,
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
fn rifle_kit(account: &mut Account) -> (u32, u32) {
    let rifle = account
        .inventory
        .iter()
        .find(|item| item.dict_id == dict("mosin_nagant"))
        .unwrap()
        .item_id;
    let rounds = account
        .inventory
        .iter()
        .find(|item| item.dict_id == dict("ammo_7.62x54r_old"))
        .unwrap()
        .item_id;
    put(account, 1, 10, rifle, 1);
    put(account, 1, 9, rounds, 10);
    (rifle, rounds)
}

#[test]
fn a_saved_preset_puts_the_three_kits_back() {
    let mut account = alice();
    let (rifle, rounds) = rifle_kit(&mut account);
    let kit1 = worn(&account, 0);
    let kit2 = worn(&account, 1);
    assert!(save(&mut account, 1).unwrap());
    let saved = account.presets[0].unwrap();
    assert_eq!(saved.slots(), 4);
    assert_eq!(
        saved.kits[1][10],
        Some(PresetSlot {
            item_id: rifle,
            dict_id: dict("mosin_nagant"),
            count: 1
        })
    );

    // The kits changed since: kit 1 empty, the rifle in kit 3 too (an item
    // may be in several kits), the PPSh in kit 2's other weapon slot.
    let ppsh = account
        .inventory
        .iter()
        .find(|item| item.dict_id == dict("ppsh"))
        .unwrap()
        .item_id;
    account.kits[0].slots = [None; KIT_SLOTS];
    put(&mut account, 1, 7, ppsh, 1);
    put(&mut account, 2, 10, rifle, 1);
    put(&mut account, 2, 9, rounds, 5);
    let before = owned(&account);
    let revisions: Vec<u32> = account.kits.iter().map(|kit| kit.revision).collect();

    assert_eq!(apply(&mut account, 1).unwrap(), Applied { empty: 0 });
    assert_eq!(worn(&account, 0), kit1);
    assert_eq!(worn(&account, 1), kit2);
    // Kit 3 was empty when saved: it is emptied.
    assert!(worn(&account, 2).is_empty());
    for (kit, revision) in account.kits.iter().zip(revisions) {
        assert_eq!(kit.revision, revision + 1);
    }
    assert_eq!(owned(&account), before);
    // The kit ids, names and the kit played stay.
    assert_eq!(account.active_kit, 1);
    assert_eq!(
        account.kits.iter().map(|kit| kit.id).collect::<Vec<_>>(),
        [1, 2, 3]
    );

    // Applied again: nothing changes, no new revision.
    let same = account.clone();
    apply(&mut account, 1).unwrap();
    assert_eq!(account, same);

    // The same items may be in several presets.
    assert!(save(&mut account, 3).unwrap());
    assert_eq!(account.presets[2], account.presets[0]);
}

#[test]
fn an_empty_or_unknown_preset_changes_nothing() {
    let mut account = alice();
    let before = account.clone();
    assert_eq!(apply(&mut account, 1), Err(PresetError::Empty));
    assert_eq!(apply(&mut account, 0), Err(PresetError::NoSuchPreset));
    assert_eq!(apply(&mut account, 6), Err(PresetError::NoSuchPreset));
    assert_eq!(save(&mut account, 6), Err(PresetError::NoSuchPreset));
    assert_eq!(account, before);
    // Kits with nothing in them save an empty preset, which clears it.
    assert!(save(&mut account, 2).unwrap());
    for kit in &mut account.kits {
        kit.slots = [None; KIT_SLOTS];
    }
    assert!(!save(&mut account, 2).unwrap());
    assert_eq!(account.presets[1], None);
}

#[test]
fn a_sold_item_leaves_its_slot_empty() {
    let mut account = alice();
    let pistol = account.kits[0].slots[7].unwrap().item_id;
    let rounds = account.kits[0].slots[8].unwrap();
    save(&mut account, 1).unwrap();
    rules::sell(&mut account, pistol, 1).unwrap();
    assert!(account.item(pistol).is_none());
    let before = owned(&account);

    assert_eq!(apply(&mut account, 1).unwrap(), Applied { empty: 1 });
    assert_eq!(account.kits[0].slots[7], None);
    assert_eq!(account.kits[0].slots[8], Some(rounds));
    // Nothing bought in its place.
    assert_eq!(owned(&account), before);
}

#[test]
fn a_stack_gets_what_is_owned_and_never_more() {
    let mut account = alice();
    let rounds = account.kits[0].slots[8].unwrap().item_id;
    let ammo = dict("ammo_7.62x25");
    save(&mut account, 1).unwrap();

    // Fired since: 20 left of the 50 saved.
    account.item_mut(rounds).unwrap().condition_or_stack = 20;
    account.sync_kits();
    let before = owned(&account);
    assert_eq!(apply(&mut account, 1).unwrap(), Applied { empty: 0 });
    assert_eq!(account.kits[0].slots[8].unwrap().count, 20);
    assert_eq!(owned(&account), before);

    // Used up, then bought again (a new item): the stack owned now.
    account.inventory.retain(|item| item.item_id != rounds);
    account.sync_kits();
    let again = account.add_item(ammo, 30);
    let before = owned(&account);
    apply(&mut account, 1).unwrap();
    assert_eq!(
        account.kits[0].slots[8],
        Some(KitSlot {
            item_id: again,
            count: 30
        })
    );
    assert_eq!(owned(&account), before);

    // None owned: the slot stays empty, nothing is bought.
    account.inventory.retain(|item| item.item_id != again);
    account.sync_kits();
    let before = owned(&account);
    assert_eq!(apply(&mut account, 1).unwrap(), Applied { empty: 1 });
    assert_eq!(account.kits[0].slots[8], None);
    assert_eq!(owned(&account), before);
}

/// A preset whose data names what the account does not own (another
/// account's item id, a stack of a type it has none of): nothing comes of
/// it. Applying makes no item, takes none and moves no money.
#[test]
fn applying_makes_no_item_and_no_money() {
    let mut account = alice();
    let mut preset = Preset {
        kits: [[None; KIT_SLOTS]; KITS],
    };
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
    let applied = apply(&mut account, 5).unwrap();
    assert_eq!(applied, Applied { empty: 3 });
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
    let mut preset = Preset {
        kits: [[None; KIT_SLOTS]; KITS],
    };
    // Saved before the rule: grenades in two pockets, a booster in two.
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
    account.presets[0] = Some(preset);
    let before = owned(&account);
    assert_eq!(apply(&mut account, 1).unwrap(), Applied { empty: 2 });
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

    // Saved with the helmet; the kit wears the balaclava now.
    put(&mut account, 0, 0, helmet, 1);
    save(&mut account, 1).unwrap();
    account.kits[0].slots[0] = None;
    put(&mut account, 0, 1, balaclava, 1);
    let before = owned(&account);
    assert_eq!(apply(&mut account, 1).unwrap(), Applied { empty: 0 });
    assert_eq!(
        account.kits[0].slots[0].map(|held| held.item_id),
        Some(helmet)
    );
    assert_eq!(account.kits[0].slots[1], None);
    assert!(account.covered_items(&account.kits[0]).is_empty());
    assert_eq!(owned(&account), before);

    // A preset holding both (data from before the rule): the kit rule keeps
    // one, the balaclava put on last takes the helmet off.
    let mut both = account.presets[0].unwrap();
    both.kits[0][1] = Some(PresetSlot {
        item_id: balaclava,
        dict_id: 1810,
        count: 1,
    });
    account.presets[1] = Some(both);
    assert_eq!(apply(&mut account, 2).unwrap(), Applied { empty: 1 });
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
    save(&mut account, 1).unwrap();
    assert_eq!(level(&account, 1, NOW), Some(starter));

    // A heavy torso in kit 2 with the rifle: the highest kit that plays.
    let (torso, torso_level) = best_torso();
    assert!(torso_level > starter);
    let piece = account.add_item(torso, FULL_CONDITION);
    rifle_kit(&mut account);
    put(&mut account, 1, 2, piece, 1);
    save(&mut account, 2).unwrap();
    let mut applied = account.clone();
    apply(&mut applied, 2).unwrap();
    assert_eq!(level(&account, 2, NOW), Some(torso_level));
    assert_eq!(
        level(&account, 2, NOW),
        Some(matchmaking::gear_level(&applied, NOW))
    );

    // The torso in a kit that cannot play (no weapon) does not count.
    let mut account3 = account.clone();
    for kit in &mut account3.kits {
        kit.slots = [None; KIT_SLOTS];
    }
    put(&mut account3, 2, 2, piece, 1);
    save(&mut account3, 3).unwrap();
    assert_eq!(level(&account3, 3, NOW), None);
    // An empty preset has none either.
    assert_eq!(level(&account3, 4, NOW), None);

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
fn a_join_offers_the_highest_preset_the_room_allows() {
    let levels = [Some(3), Some(9), Some(6), None, Some(9)];
    // A room of any max level: nothing.
    assert_eq!(offer(2, 0, &levels), Offer::None);
    // Gear over the max: the highest preset at most the max.
    assert_eq!(
        offer(11, 7, &levels),
        Offer::Fits {
            preset: 3,
            level: 6
        }
    );
    // Two alike: the lower number.
    assert_eq!(
        offer(12, 10, &levels),
        Offer::Fits {
            preset: 2,
            level: 9
        }
    );
    // None fits: the refusal as before.
    assert_eq!(offer(11, 2, &levels), Offer::None);
    // The room allows more than the kits: a higher preset that fits.
    assert_eq!(
        offer(4, 7, &levels),
        Offer::Higher {
            preset: 3,
            level: 6
        }
    );
    assert_eq!(
        offer(4, 15, &levels),
        Offer::Higher {
            preset: 2,
            level: 9
        }
    );
    // None higher than the gear: no popup.
    assert_eq!(offer(6, 7, &levels), Offer::None);
    assert_eq!(offer(9, 15, &levels), Offer::None);
    // Gear at the max is no refusal: only a higher preset is offered.
    assert_eq!(offer(7, 7, &levels), Offer::None);
    // No preset saved: nothing.
    assert_eq!(offer(11, 7, &[None; PRESETS]), Offer::None);
}
