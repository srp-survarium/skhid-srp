//! A new account's starter kit on the released game data, by its dict ids
//! (the test data predates the Keeper of the Sun and the Novice's backpack; the unit
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
use survarium::v026g::item_params;
use survarium::v026g::loadouts::{self, Loadout};
use survarium::v026g::lobby::FULL_CONDITION;
use survarium::v026g::modifications::Modifications;

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

    // Owned: the TT-33 (18), the PPSh-41 (106), the Mosin (107), the AKMN
    // (80), the AKMSU (2290) and the Remington 700 (14), 10 stash maps
    // (153), 50 medkits (67), 50 antitoxins (82) and 20 bandages (66), the
    // kits' pockets taking theirs from these stacks.
    for (dict_id, owned) in [
        (18, FULL_CONDITION),
        (106, FULL_CONDITION),
        (107, FULL_CONDITION),
        (80, FULL_CONDITION),
        (2290, FULL_CONDITION),
        (14, FULL_CONDITION),
        (153, 10),
        (67, 50),
        (82, 50),
        (66, 20),
    ] {
        assert_eq!(stacks(dict_id), [owned], "{dict_id}");
    }
    // The Keeper Of The Sun Vest (977) and Pants (978), unworn: their own
    // experience and reputation +5 % (1240, 1280) and armour +8 (1004).
    for dict_id in [977, 978] {
        let owned: Vec<_> = account
            .inventory
            .iter()
            .filter(|item| item.dict_id == dict_id)
            .collect();
        assert_eq!(owned.len(), 1, "{dict_id}");
        assert_eq!(owned[0].condition_or_stack, FULL_CONDITION);
        assert_eq!(
            owned[0].modifications,
            [(1240, false), (1280, false), (1004, false)]
        );
        assert!(
            account
                .kits
                .iter()
                .all(|kit| kit.slot_of(owned[0].item_id).is_none())
        );
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
    // the Keeper set, each piece with armour +8 (1004) and weight -15 %
    // (1064; the mask, which takes no armour, the weight only), the
    // Novice's backpack (1869) on the back, its own one weapon in weapon
    // slot 1, weapon slot 2 empty, and three pockets of two.
    type Worn = (usize, u16, u32, Vec<(u32, bool)>);
    // `(weapon, ammo, rounds, the kit's level)`.
    // The ammo slot full: five bags (the Keeper vest's and pants' 1050)
    // of one clip each.
    for kit in 0..3 {
        assert_eq!(tutorial::starter_ammo_bags(dictionary, kit), 5);
    }
    let weapons = [(306, 7, 150, 14), (363, 369, 150, 15), (110, 22, 75, 14)];
    let both = || vec![(1004, false), (1064, false)];
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
                (0, 202, 1, both()),
                (1, 200, 1, vec![(1064, false)]),
                (2, 196, 1, both()),
                (3, 1869, 1, vec![]),
                (4, 197, 1, both()),
                (5, 201, 1, both()),
                (6, 198, 1, both()),
                (7, weapon, 1, vec![]),
                (8, ammo, rounds, vec![]),
                (13, 67, 2, vec![]),
                (14, 66, 2, vec![]),
                (15, 82, 2, vec![]),
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
        // The gear opens three pockets of two units: the vest's +1, the
        // backpack's +1 and +1 unit.
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
        assert_eq!(loadouts::pockets(&gear), (3, 2), "kit {}", kit.id);
        assert!(dictionary.compatible(weapon, ammo));
        assert_eq!(rules::check_play(&account, kit.id, 0), Ok(()));
        // Within the player's 30 kg (`max_carried_weight`) with the Keeper
        // pieces' weight -15 % (1064 multiplies their weight by 0.85): the
        // bare items (`kit_weight`) weigh 29.7, 30.2 and 31.3, with it 26.7,
        // 27.3 and 28.4.
        let table = Modifications::shared();
        let carried: f32 = kit
            .slots
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
            .sum();
        assert!(carried <= 30.0, "kit {}: {carried}", kit.id);
        // The kit's level for the rooms, the bots and the matchmaking: its
        // highest item's, the Keeper set's 14, the AK-12's 15.
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
