//! A new account's starter kit on the released game data, by its dict ids
//! (the test data predates the knit cap; the unit test
//! `tutorial::tests::a_new_account_starts_with_the_starter_kit` covers the
//! rest on it). Its own binary: the data must be this process's before any
//! table is used.
//!
//! `SRP_TEST_GAME_DATA=<server-game-data.db> cargo test -p session --test
//! starter_kit -- --ignored`

use session::v026g::account::Account;
use session::v026g::{known, rules, tutorial};
use survarium::v026g::dictionary::Dictionary;
use survarium::v026g::game_data;
use survarium::v026g::lobby::FULL_CONDITION;

#[test]
#[ignore = "needs the released server game data (SRP_TEST_GAME_DATA)"]
fn a_new_account_on_the_released_data_wears_the_whole_kit() {
    let path = std::env::var_os("SRP_TEST_GAME_DATA").expect("the data's path");
    // srp starts on it.
    game_data::init_from(std::path::Path::new(&path)).unwrap();
    assert_eq!(known::check_code(), Ok(()));

    let mut account = Account::new("Alice");
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
            302, 322, 323, 331, 368, 369, 370, 371, 372, 2201, 2202, 2203, 2204, 2205
        ]
    );
    // Every item the game calls ammo.
    let kinds = dictionary.items.iter().filter(|item| {
        survarium::v026g::item_params::item_params(item.dict_id)
            .is_some_and(|params| params.kind == "ammo")
    });
    assert_eq!(kinds.count(), ammo.len());
    for dict_id in ammo {
        let stacks: Vec<u32> = account
            .inventory
            .iter()
            .filter(|item| item.dict_id == dict_id)
            .map(|item| item.condition_or_stack)
            .collect();
        assert_eq!(stacks, [1000], "{dict_id}");
    }

    // 50 medkits (67) and 50 antitoxins (82) owned.
    for dict_id in [67, 82] {
        let stacks: Vec<u32> = account
            .inventory
            .iter()
            .filter(|item| item.dict_id == dict_id)
            .map(|item| item.condition_or_stack)
            .collect();
        assert_eq!(stacks, [50], "{dict_id}");
    }

    // No SOK-94 (the owner's of 2026-10-24).
    assert!(account.inventory.iter().all(|item| item.dict_id != 2215));

    // Every kit, `(slot, dict id, count, modifications)`: the same gear and
    // its own one weapon in weapon slot 1, weapon slot 2 empty.
    type Worn = (usize, u16, u32, Vec<(u32, bool)>);
    // `(weapon, ammo, rounds, the kit's level)`.
    // The ammo slot full: three bags of one clip each (no ammo bag
    // modification on the gear).
    assert_eq!(tutorial::starter_ammo_bags(dictionary), 3);
    let weapons = [(88, 70, 150, 3), (12, 51, 30, 3), (110, 22, 45, 4)];
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
                (3, 9, 1, vec![]),
                (4, 46, 1, vec![]),
                (5, 42, 1, vec![(1133, false)]),
                (6, 38, 1, vec![]),
                (7, weapon, 1, vec![]),
                (8, ammo, rounds, vec![]),
                (13, 67, 2, vec![]),
                (14, 82, 2, vec![]),
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
        assert!(dictionary.compatible(weapon, ammo));
        assert_eq!(rules::check_play(&account, kit.id, 0), Ok(()));
        let weight = account.kit_weight(kit);
        assert!(weight <= 30.0, "kit {}: {weight}", kit.id);
        // The kit's level for the rooms, the bots (at most 5) and the
        // matchmaking: the armour, the oxygen tank and the drugs are 2 and
        // 3, the Kiparis and its ammo 2, the TOZ-122 and its ammo 3, the
        // MP-153 4 and its ammo 1.
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
