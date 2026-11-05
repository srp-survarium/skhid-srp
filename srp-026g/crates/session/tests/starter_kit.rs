//! A new account's starter kit on the released game data, by its dict ids
//! (the test data predates the knit cap and the Novice's backpack; the unit
//! test `tutorial::tests::a_new_account_starts_with_the_starter_kit` covers
//! the rest on it). Its own binary: the data must be this process's before
//! any table is used.
//!
//! `SRP_TEST_GAME_DATA=<server-game-data.db> cargo test -p session --test
//! starter_kit -- --ignored`

use session::v026g::account::Account;
use session::v026g::{known, rules, tutorial};
use survarium::v026g::dictionary::Dictionary;
use survarium::v026g::game_data;
use survarium::v026g::loadouts::{self, Loadout};
use survarium::v026g::lobby::FULL_CONDITION;

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
        survarium::v026g::item_params::item_params(item.dict_id)
            .is_some_and(|params| params.kind == "ammo")
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

    // Owned: the TT-33 (18), the PPSh-41 (106), the Mosin (107), the AKMN (80) and
    // the AKMSU (2290), 10 stash maps (153), 50
    // medkits (67), 50 antitoxins (82) and 20 bandages (66), the kits'
    // pockets taking theirs from these stacks.
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

    // No SOK-94 (the owner's of 2026-10-24), no trap (68, the owner's of
    // 2026-11-02).
    let none = [2215, 68];
    assert!(
        account
            .inventory
            .iter()
            .all(|item| !none.contains(&item.dict_id))
    );

    // Every kit, `(slot, dict id, count, modifications)`: the same gear,
    // the Novice's backpack (1869) on the back, its own one weapon in
    // weapon slot 1, weapon slot 2 empty, and three pockets of three.
    type Worn = (usize, u16, u32, Vec<(u32, bool)>);
    // `(weapon, ammo, rounds, the kit's level)`.
    // The ammo slot full: three bags of one clip each (no ammo bag
    // modification on the gear).
    for kit in 0..3 {
        assert_eq!(tutorial::starter_ammo_bags(dictionary, kit), 3);
    }
    let weapons = [(306, 7, 90, 7), (14, 51, 30, 5), (110, 22, 45, 4)];
    let mut item_ids: Vec<Vec<u32>> = Vec::new();
    for (kit, (weapon, ammo, rounds, level)) in account.kits.iter().zip(weapons) {
        let worn: Vec<Worn> = kit
            .slots
            .iter()
            .enumerate()
            .filter_map(|(slot, held)| {
                let held = (*held)?;
                let item = account.item(held.item_id).unwrap();
                Some((slot, item.dict_id, held.count, item.modifications.clone()))
            })
            .collect();
        assert_eq!(
            worn,
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
        for slot in kit.slots.iter().flatten() {
            let item = account.item(slot.item_id).unwrap();
            if !dictionary.is_stack(item.dict_id) {
                assert_eq!(item.condition_or_stack, FULL_CONDITION);
            }
        }
        // The gear opens three pockets of three units.
        let gear = Loadout {
            items: worn
                .iter()
                .map(|&(slot, dict_id, count, _)| loadouts::SlotItem {
                    slot: slot as u8,
                    dict_id,
                    count,
                    stored: count,
                    mods: &[],
                })
                .collect(),
        };
        assert_eq!(loadouts::pockets(&gear), (3, 3), "kit {}", kit.id);
        assert!(dictionary.compatible(weapon, ammo));
        assert_eq!(rules::check_play(&account, kit.id, 0), Ok(()));
        let weight = account.kit_weight(kit);
        assert!(weight <= 30.0, "kit {}: {weight}", kit.id);
        // The kit's level for the rooms, the bots (at most 5) and the
        // matchmaking: its highest item's, the AK-74N's 7, the Remington 700's 5, the
        // Novice's backpack's 4 (the MP-153 4 too).
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
}
