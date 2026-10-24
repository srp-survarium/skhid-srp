//! A new account's starter kit on the released game data, by its dict ids
//! (the test data predates the SOK-94 and the knit cap; the unit test
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
    tutorial::signup_grant(&mut account);
    assert_eq!(
        (account.generic_money, account.premium_money),
        (60_000, 500)
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

    // Every kit, `(slot, dict id, count, modifications)`: the same gear,
    // the SOK-94, and its own second weapon.
    type Worn = (usize, u16, u32, Vec<(u32, bool)>);
    let second = [(88, 70, 100), (12, 51, 20), (110, 22, 30)];
    let mut item_ids: Vec<Vec<u32>> = Vec::new();
    for (kit, (weapon, ammo, rounds)) in account.kits.iter().zip(second) {
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
                (7, 2215, 1, vec![]),
                (8, 81, 60, vec![]),
                (10, weapon, 1, vec![]),
                (11, ammo, rounds, vec![]),
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
        assert!(dictionary.compatible(2215, 81) && dictionary.compatible(weapon, ammo));
        assert_eq!(rules::check_play(&account, kit.id, 0), Ok(()));
        let weight = account.kit_weight(kit);
        assert!(weight <= 30.0, "kit {}: {weight}", kit.id);
        // The kit's level for the rooms, the bots and the matchmaking: 6,
        // the 7.62x39's `match_making_level` (the lowest of the SOK-94's
        // ammo; the rifle is 4, the armour 2 and 3, the second weapons and
        // their ammo 1 to 4).
        assert_eq!(session::v026g::matchmaking::kit_level(&account, kit), 6);
        item_ids.push(kit.slots[..8].iter().flatten().map(|s| s.item_id).collect());
    }
    // The gear and the SOK-94 are one item each, worn by all three kits.
    assert!(item_ids.iter().all(|ids| *ids == item_ids[0]));
}
