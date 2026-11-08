//! A new account's starter kit on the released game data, by its dict ids
//! (the test data predates the Novice's backpack, the Keeper of the Sun,
//! the Saiga-12 and more; the unit test
//! `tutorial::tests::a_new_account_starts_with_the_starter_kit` covers the
//! rest on it). Its own binary: the data must be this process's before any
//! table is used.
//!
//! `SRP_TEST_GAME_DATA=<server-game-data.db> cargo test -p session --test
//! starter_kit -- --ignored`

use session::v026g::account::{Account, Kit};
use session::v026g::{known, presets, rules, tutorial};
use survarium::v026g::dictionary::Dictionary;
use survarium::v026g::game_data;
use survarium::v026g::item_params;
use survarium::v026g::loadouts::{self, Loadout};
use survarium::v026g::lobby::FULL_CONDITION;
use survarium::v026g::modifications::Modifications;

/// A kit's slots, `(slot, dict id, count, modifications)`.
type Worn = (usize, u16, u32, Vec<(u32, bool)>);

fn worn(account: &Account, kit: &Kit) -> Vec<Worn> {
    kit.slots
        .iter()
        .enumerate()
        .filter_map(|(slot, held)| {
            let held = (*held)?;
            let item = account.item(held.item_id).unwrap();
            Some((slot, item.dict_id, held.count, item.modifications.clone()))
        })
        .collect()
}

/// The pockets the kit's gear opens, `(pockets, units)`, with its items'
/// own modifications.
fn pockets(account: &Account, kit: &Kit) -> (u32, u32) {
    let worn = worn(account, kit);
    let gear = Loadout {
        items: worn
            .iter()
            .map(|&(slot, dict_id, count, _)| loadouts::SlotItem {
                slot: slot as u8,
                dict_id,
                count,
            })
            .collect(),
    };
    let own: Vec<(u32, bool)> = worn
        .iter()
        .flat_map(|(_, _, _, own)| own.iter().copied())
        .collect();
    loadouts::pockets_with(&gear, &own)
}

/// What the kit weighs carried: its items' weight with their own weight
/// modifications (1064 multiplies an item's by 0.85).
fn carried(account: &Account, kit: &Kit) -> f32 {
    let table = Modifications::shared();
    kit.slots
        .iter()
        .flatten()
        .map(|slot| {
            let item = account.item(slot.item_id).unwrap();
            let factor: f32 = item
                .modifications
                .iter()
                .filter_map(|&(id, _)| table.modification(id))
                .flat_map(|row| &row.modifiers)
                .filter(|m| m.path == "parameters/weight" && m.kind == 2)
                .map(|m| 1.0 + m.value)
                .product();
            item_params::carried_weight(item.dict_id, slot.count) * factor
        })
        .sum()
}

#[test]
#[ignore = "needs the released server game data (SRP_TEST_GAME_DATA)"]
fn a_new_account_on_the_released_data_wears_the_whole_kit() {
    let path = std::env::var_os("SRP_TEST_GAME_DATA").expect("the data's path");
    // srp starts on it.
    game_data::init_from(std::path::Path::new(&path)).unwrap();
    assert_eq!(known::check_code(), Ok(()));

    // As `GameAccounts::sign_up` makes it.
    let mut account = Account::with_loadout("Alice", &Loadout { items: Vec::new() });
    tutorial::signup_grant(&mut account, 1_800_000_000);
    assert_eq!(
        (account.generic_money, account.premium_money),
        (15_000, 150)
    );
    assert_eq!(account.reputation_level(1), 5);
    // Level 15, its points spent in the five skills of the table (the
    // released tree takes them: `game_data::init` checked it).
    assert_eq!(account.level(), 15);
    assert_eq!(account.skills.len(), 5);
    assert_eq!(account.free_skill_points(), 0);

    // Every ammo, the 0.29a3 weapons' too: 1000 rounds.
    let dictionary = Dictionary::shared();
    let ammo = tutorial::starter_ammo(dictionary);
    assert_eq!(
        ammo,
        [
            7, 20, 22, 50, 51, 52, 53, 70, 71, 72, 81, 91, 103, 104, 147, 148, 217, 218, 300, 301,
            302, 322, 323, 331, 368, 369, 370, 371, 372, 2201, 2202, 2203, 2204, 2205, 2253, 2254,
            2265, 2266, 2275
        ]
    );
    // Every item the game calls ammo.
    let kinds = dictionary.items.iter().filter(|item| {
        item_params::item_params(item.dict_id).is_some_and(|params| params.kind == "ammo")
    });
    assert_eq!(kinds.count(), ammo.len());
    let stacks = |dict_id: u16| -> Vec<u32> {
        account
            .inventory
            .iter()
            .filter(|item| item.dict_id == dict_id)
            .map(|item| item.condition_or_stack)
            .collect()
    };
    for &dict_id in &ammo {
        assert_eq!(stacks(dict_id), [1000], "{dict_id}");
    }

    // Owned: the TT-33 (18), the PPSh-41 (106), the Mosin (107), the AKMN
    // (80) and the AKMSU (2290), 10 stash maps (153), 50 medkits (67), 50
    // antitoxins (82) and 20 bandages (66), the kits' pockets taking theirs
    // from these stacks.
    for (dict_id, owned) in [
        (18, FULL_CONDITION),
        (106, FULL_CONDITION),
        (107, FULL_CONDITION),
        (80, FULL_CONDITION),
        (2290, FULL_CONDITION),
        (153, 10),
        (67, 50),
        (82, 50),
        (66, 20),
    ] {
        assert_eq!(stacks(dict_id), [owned], "{dict_id}");
    }
    // One of each item the presets wear, new.
    for dict_id in [
        1821, 43, 325, 1869, 46, 42, 38, 306, 14, 110, 202, 200, 196, 197, 977, 978, 201, 198, 199,
        363, 2227, 2283,
    ] {
        assert_eq!(stacks(dict_id), [FULL_CONDITION], "{dict_id}");
    }

    // No SOK-94 (the owner's of 2026-10-24), no trap (68, the owner's of
    // 2026-11-02).
    let none = [2215, 68];
    assert!(
        account
            .inventory
            .iter()
            .all(|item| !none.contains(&item.dict_id))
    );

    // Preset 1, the kits, active: the same gear, the Novice's backpack
    // (1869) on the back, its own one weapon in weapon slot 1, weapon slot
    // 2 empty, and three pockets of three.
    let grant = tutorial::new_account();
    assert_eq!(account.active_preset, 1);
    // The ammo slot full: three bags of one clip each (no ammo bag
    // modification on the gear).
    for kit in 0..3 {
        assert_eq!(
            tutorial::starter_ammo_bags(dictionary, &grant.active, kit),
            3
        );
    }
    // `(weapon, ammo, rounds, the kit's level)`.
    let weapons = [(306, 7, 90, 7), (14, 51, 30, 5), (110, 22, 45, 4)];
    let mut item_ids: Vec<Vec<u32>> = Vec::new();
    for (kit, (weapon, ammo, rounds, level)) in account.kits.iter().zip(weapons) {
        assert_eq!(
            worn(&account, kit),
            [
                (0, 1821, 1, vec![]),
                (1, 43, 1, vec![(1183, false)]),
                (2, 325, 1, vec![]),
                (3, 1869, 1, vec![]),
                (4, 46, 1, vec![]),
                (5, 42, 1, vec![(1133, false)]),
                (6, 38, 1, vec![]),
                (7, weapon, 1, vec![]),
                (8, ammo, rounds, vec![]),
                (13, 67, 3, vec![]),
                (14, 66, 3, vec![]),
                (15, 82, 3, vec![]),
            ],
            "kit {}",
            kit.id
        );
        assert_eq!(pockets(&account, kit), (3, 3), "kit {}", kit.id);
        assert!(dictionary.compatible(weapon, ammo));
        assert_eq!(rules::check_play(&account, kit.id, 0), Ok(()));
        let weight = account.kit_weight(kit);
        assert!(weight <= 30.0, "kit {}: {weight}", kit.id);
        // The kit's level for the rooms, the bots (at most 5) and the
        // matchmaking: its highest item's, the AK-74N's 7, the Remington
        // 700's 5, the Novice's backpack's 4 (the MP-153 4 too).
        assert_eq!(
            session::v026g::matchmaking::kit_level(&account, kit),
            level,
            "kit {}",
            kit.id
        );
        item_ids.push(kit.slots[..7].iter().flatten().map(|s| s.item_id).collect());
    }
    // The gear is one item each, worn by all three kits.
    assert!(item_ids.iter().all(|ids| *ids == item_ids[0]));
    let rifle = account.kits[1].slots[7].unwrap().item_id;

    // Preset 2, chosen as the player chooses it: the Keeper set (the
    // owner's of 2026-11-08), each piece with armour +8 (1004) and weight
    // -15 % (1064; the mask, which takes no armour, the weight only), the
    // "Wanderer" container (199: an artefact slot and a pocket's unit; its
    // own weight -15 % and a pocket, 1030), a Glock 17 (2283) with one bag
    // of 9x19 7N31 (368) in weapon slot 2, and three pockets of two. Kit 1
    // wears the Keeper Of The Sun Vest (977) and Pants (978), their own
    // experience and reputation +5 % (1240, 1280) and armour +8, with the
    // AK-12 (363); kit 2 the Keeper vest (196) and pants (197) with the
    // Saiga-12 (2227) and its buckshot (2205); kit 3 the same with the
    // Remington 700 (14), the one preset 1's kit 2 wears.
    assert_eq!(
        presets::choose(&mut account, 2, 0),
        Ok(presets::Applied { empty: 0 })
    );
    assert!(account.presets[0].is_some());
    for kit in 0..3 {
        assert_eq!(
            tutorial::starter_ammo_bags(dictionary, &grant.saved[0], kit),
            5
        );
    }
    let both = || vec![(1004, false), (1064, false)];
    let sun = || vec![(1240, false), (1280, false), (1004, false)];
    // `(vest and pants, weapon, ammo, rounds, the kit's level)`: four bags
    // of one clip of the weapon's ammo, one of the Glock's.
    let weapons = [
        ((977, 978, sun()), 363, 369, 120, 15),
        ((196, 197, both()), 2227, 2205, 80, 14),
        ((196, 197, both()), 14, 51, 40, 14),
    ];
    for (kit, ((vest, pants, own), weapon, ammo, rounds, level)) in account.kits.iter().zip(weapons)
    {
        assert_eq!(
            worn(&account, kit),
            [
                (0, 202, 1, both()),
                (1, 200, 1, vec![(1064, false)]),
                (2, vest, 1, own.clone()),
                (3, 199, 1, vec![(1064, false), (1030, false)]),
                (4, pants, 1, own),
                (5, 201, 1, both()),
                (6, 198, 1, both()),
                (7, weapon, 1, vec![]),
                (8, ammo, rounds, vec![]),
                (10, 2283, 1, vec![]),
                (11, 368, 50, vec![]),
                (13, 67, 2, vec![]),
                (14, 66, 2, vec![]),
                (15, 82, 2, vec![]),
            ],
            "kit {}",
            kit.id
        );
        // Three pockets of two: the vest's pocket and the container's, a
        // unit more of the container.
        assert_eq!(pockets(&account, kit), (3, 2), "kit {}", kit.id);
        assert!(dictionary.compatible(weapon, ammo));
        assert!(dictionary.compatible(2283, 368));
        assert_eq!(rules::check_play(&account, kit.id, 0), Ok(()));
        let weight = carried(&account, kit);
        assert!(weight < 30.0, "kit {}: {weight}", kit.id);
        assert_eq!(
            session::v026g::matchmaking::kit_level(&account, kit),
            level,
            "kit {}",
            kit.id
        );
    }
    // The same Remington 700 in both presets.
    assert_eq!(account.kits[2].slots[7].unwrap().item_id, rifle);
    // And back: preset 1 as it was.
    assert_eq!(
        presets::choose(&mut account, 1, 0),
        Ok(presets::Applied { empty: 0 })
    );
    assert_eq!(account.kits[1].slots[7].unwrap().item_id, rifle);
    let back = account.kits[0].slots[3].unwrap().item_id;
    assert_eq!(account.item(back).unwrap().dict_id, 1869);
}
