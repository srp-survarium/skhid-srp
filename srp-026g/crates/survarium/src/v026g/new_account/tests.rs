use super::*;

/// The table as the game data has it (`new-account.tsv`): the owner's
/// numbers; the Keeper set worn, the Novice's backpack on the back, three
/// pockets of two; the AK-12 in the second kit.
#[test]
fn the_grant_is_the_tables() {
    let grant = shared();
    // Level 15, its 15 points spent (the owner's of 2026-11-06): Quick
    // Response 5, Battle Stance 2, Potent Painkiller 1, Fast Bandaging 2,
    // Metabolism 5.
    assert_eq!(grant.level, 15);
    assert_eq!(
        grant.skills,
        [(101, 5), (102, 2), (301, 1), (302, 2), (311, 5)]
    );
    // The test data lacks the medical tree (`medical-skills-1.json`); the
    // released data takes them (`session`'s `tests/starter_kit.rs`).
    assert_eq!(
        SkillsTree::shared().validate(grant.level, &grant.skills[..2]),
        Ok(())
    );
    assert_eq!(
        (grant.silver, grant.gold, grant.premium_days),
        (15_000, 150, 30)
    );
    assert_eq!(
        (grant.rounds, grant.faction.id(), grant.reputation_level),
        (1000, 1, 5)
    );
    // A month of premium, the shop's (`PREMIUM_OFFERS` id 3).
    let month = (3, grant.premium_days);
    assert!(super::super::shop_offers::PREMIUM_OFFERS.contains(&month));
    let at = |slot: u8| {
        grant
            .all_kits
            .iter()
            .find(|(at, _)| *at == slot)
            .map(|(_, name)| name.as_str())
    };
    let armour: Vec<Option<&str>> = (0..7).map(at).collect();
    assert_eq!(
        armour,
        [
            Some("edge_helmet_5"),
            Some("edge_mask_5"),
            Some("edge_torso_5"),
            Some("stk2_novice_026_back"),
            Some("edge_legs_5"),
            Some("edge_gloves_5"),
            Some("edge_boots_5")
        ]
    );
    let weapons: Vec<Vec<(u8, &str)>> = (0..KITS)
        .map(|kit| {
            grant.kits[kit]
                .iter()
                .map(|(slot, name)| (*slot, name.as_str()))
                .collect()
        })
        .collect();
    assert_eq!(
        weapons,
        [
            [(7, "ak_74n"), (8, "ammo_5.45x39_7n6m")],
            [(7, "ak_12"), (8, "ammo_6.5x39")],
            [(7, "mp_153"), (8, "ammo_12mm_buck")]
        ]
    );
    // The owner's of 2026-11-08: each Keeper piece, worn or owned, with
    // armour +8 (1004) and weight -15 % (1064), the strongest of their
    // kinds; but the mask, which takes no armour (weight only), and the
    // Keeper of the Sun, which comes with two (experience and reputation
    // +5 %, its `start`) and so takes one more (armour).
    let modifications: Vec<(&str, u32)> = grant
        .modifications
        .iter()
        .map(|(name, id)| (name.as_str(), *id))
        .collect();
    assert_eq!(
        modifications,
        [
            ("edge_helmet_5", 1004),
            ("edge_helmet_5", 1064),
            ("edge_mask_5", 1064),
            ("edge_torso_5", 1004),
            ("edge_torso_5", 1064),
            ("edge_legs_5", 1004),
            ("edge_legs_5", 1064),
            ("edge_gloves_5", 1004),
            ("edge_gloves_5", 1064),
            ("edge_boots_5", 1004),
            ("edge_boots_5", 1064),
            ("edge_torso_5_special", 1004),
            ("edge_legs_5_special", 1004)
        ]
    );
    let pockets: Vec<(u8, &str)> = POCKET_SLOTS
        .iter()
        .filter_map(|&slot| at(slot).map(|name| (slot, name)))
        .collect();
    assert_eq!(
        pockets,
        [(13, "medkit"), (14, "bandages"), (15, "antitoxin")]
    );
    assert_eq!(grant.pocket_units, 2);
    let own: Vec<(&str, u32)> = grant
        .own
        .iter()
        .map(|(name, units)| (name.as_str(), *units))
        .collect();
    assert_eq!(
        own,
        [
            ("tt_33", 1),
            ("mosin_nagant", 1),
            ("ppsh", 1),
            ("akmn", 1),
            ("akmsu", 1),
            ("rem_700", 1),
            ("edge_torso_5_special", 1),
            ("edge_legs_5_special", 1),
            ("drop_card", 10),
            ("medkit", 50),
            ("antitoxin", 50),
            ("bandages", 20)
        ]
    );
}

/// A key missing, one it has not, a slot no kit has: refused, named.
#[test]
fn a_table_of_other_keys_is_refused_naming_them() {
    let table = |entries: Vec<(&str, ConfigValue)>| {
        let entries = entries
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect();
        NewAccount::parse(&ConfigValue::Named(entries).to_bytes().unwrap())
    };
    assert_eq!(
        table(vec![("silver", ConfigValue::Integer(1))]),
        Err(format!("{PATH}: no `skills`"))
    );
    assert_eq!(
        table(vec![("kit_4", ConfigValue::Named(Vec::new()))]),
        Err(format!("{PATH}: `kit_4` is no key of it"))
    );
    let worn = ConfigValue::Named(vec![("23".into(), ConfigValue::String("ppsh".into()))]);
    assert_eq!(
        table(vec![("kit_1", worn)]),
        Err(format!("{PATH}: `kit_1/23`: no slot of a kit"))
    );
}

/// Pockets the gear does not open, or that take fewer units than the
/// table gives, are named: without the Novice's backpack (the test
/// data's), the gear opens two pockets of one.
#[test]
fn pockets_the_gear_does_not_give_are_named() {
    let mut grant = of_the_test_data();
    grant
        .all_kits
        .iter_mut()
        .find(|(slot, _)| *slot == 3)
        .unwrap()
        .1 = "scav_oxygen_1".into();
    let table = Modifications::shared();
    let wrong = grant.check(Dictionary::shared(), table, SkillsTree::shared());
    assert_eq!(wrong.len(), 3 * 3, "{wrong:?}");
    assert!(wrong.contains(
        &"kit 1: slot 15: the gear opens 2 pockets of 1 units, 2 of `antitoxin` asked".into()
    ));
    grant.pocket_units = 1;
    grant.all_kits.retain(|(slot, _)| *slot != 15);
    assert_eq!(
        grant.check(Dictionary::shared(), table, SkillsTree::shared()),
        Vec::<String>::new()
    );
}

/// The grant on the test data: refused, naming the AKMSU, the Keeper of
/// the Sun and the Novice's backpack of patches it predates; the released
/// data has them (`session`'s `tests/starter_kit.rs`).
#[test]
fn the_test_data_lacks_four_items_of_the_grant() {
    let error = check_data(game_data::shared()).unwrap_err().to_string();
    assert!(
        error.contains(
            "gameplay/srp/new_account.options: no item `akmsu`, no item \
             `edge_torso_5_special`, no item `edge_legs_5_special`, no item \
             `stk2_novice_026_back`. "
        ),
        "{error}"
    );
}

/// A faction whose quests the lobby does not show (5, 6 or none) is
/// refused at load: a sign-up could not choose it.
#[test]
fn a_faction_without_quests_is_refused() {
    assert_eq!(QuestFaction::try_from(1).map(QuestFaction::id), Ok(1));
    for id in [0, 5, 6] {
        assert_eq!(QuestFaction::try_from(id), Err(id));
    }
    let mut entries = match ConfigValue::parse(game_data::file(PATH)).unwrap() {
        ConfigValue::Named(entries) => entries,
        other => panic!("{other:?}"),
    };
    let faction = entries
        .iter_mut()
        .find(|(key, _)| key == "faction")
        .unwrap();
    faction.1 = ConfigValue::Integer(5);
    let config = ConfigValue::Named(entries).to_bytes().unwrap();
    assert_eq!(
        NewAccount::parse(&config),
        Err(format!(
            "{PATH}: `faction` is 5, not one whose quests the lobby shows ([1, 2, 3, 4])"
        ))
    );
}

/// An item's modifications are a list of ids, one or more; a single id is
/// refused, naming the key.
#[test]
fn an_items_modifications_are_a_list() {
    let mut entries = match ConfigValue::parse(game_data::file(PATH)).unwrap() {
        ConfigValue::Named(entries) => entries,
        other => panic!("{other:?}"),
    };
    let modifications = &mut entries
        .iter_mut()
        .find(|(key, _)| key == "own_modifications")
        .unwrap()
        .1;
    *modifications = ConfigValue::Named(vec![
        (
            "edge_mask_5".into(),
            ConfigValue::Indexed(vec![ConfigValue::Integer(1004)]),
        ),
        ("edge_gloves_5".into(), ConfigValue::Integer(1004)),
    ]);
    let config = ConfigValue::Named(entries).to_bytes().unwrap();
    assert_eq!(
        NewAccount::parse(&config),
        Err(format!(
            "{PATH}: `own_modifications/edge_gloves_5` is Integer(1004), not a list of ids"
        ))
    );
}

/// The grant with what the test data has: its skills
/// ([`without_medical_skills`]), no AKMSU and no Keeper of the Sun, the
/// test data's predating them.
fn of_the_test_data() -> NewAccount {
    let lacks = ["akmsu", "edge_torso_5_special", "edge_legs_5_special"];
    let mut grant = without_medical_skills();
    grant
        .own
        .retain(|(name, _)| !lacks.contains(&name.as_str()));
    grant
        .modifications
        .retain(|(name, _)| !lacks.contains(&name.as_str()));
    grant
}

/// The grant with the gear of the test data ([`of_the_test_data`], the
/// oxygen tank on the back, two pockets of one): what it checks of what it
/// owns and of its modifications.
fn on_the_test_data() -> NewAccount {
    let mut grant = of_the_test_data();
    grant.all_kits.retain(|(slot, _)| *slot != 15);
    grant
        .all_kits
        .iter_mut()
        .find(|(slot, _)| *slot == 3)
        .unwrap()
        .1 = "scav_oxygen_1".into();
    grant.pocket_units = 1;
    grant
}

/// A pocket's item must be owned with at least a pocket's units: the
/// pockets take theirs from what it owns.
#[test]
fn a_pocket_of_an_item_not_owned_is_named() {
    let table = Modifications::shared();
    let mut grant = on_the_test_data();
    assert_eq!(
        grant.check(Dictionary::shared(), table, SkillsTree::shared()),
        Vec::<String>::new()
    );
    grant.own.retain(|(name, _)| name != "bandages");
    let wrong = grant.check(Dictionary::shared(), table, SkillsTree::shared());
    assert_eq!(wrong.len(), 3, "{wrong:?}");
    assert_eq!(
        wrong[0],
        "kit 1: slot 14: 1 of `bandages` asked, 0 owned (`own/bandages`)"
    );
}

/// A modification of a type its item does not take, on a stack, or on an
/// item it is not given, is named; one on an item it owns but no kit wears
/// passes.
#[test]
fn a_modification_its_item_does_not_take_is_named() {
    let table = Modifications::shared();
    let grant = on_the_test_data();
    let with = |item: &str, id: u32| {
        let mut grant = grant.clone();
        grant.modifications = vec![(item.to_owned(), id)];
        grant.check(Dictionary::shared(), table, SkillsTree::shared())
    };
    assert_eq!(with("edge_gloves_5", 1004), Vec::<String>::new());
    assert_eq!(with("edge_gloves_5", 1064), Vec::<String>::new());
    // The respirator's isolation on the gloves: of a type they do not take.
    assert_eq!(
        with("edge_gloves_5", 1183),
        ["modification 1183 is none `edge_gloves_5` takes"]
    );
    assert_eq!(
        with("medkit", 1004),
        ["modification 1004 is none `medkit` takes"]
    );
    // The Remington 700 is owned, unworn.
    assert_eq!(
        with("rem_700", 1004),
        ["modification 1004 is none `rem_700` takes"]
    );
    assert_eq!(
        with("scav_gloves_2", 1004),
        ["modification 1004 of `scav_gloves_2`, which it is not given"]
    );
    // The Keeper mask takes no armour.
    assert_eq!(
        with("edge_mask_5", 1004),
        ["modification 1004 is none `edge_mask_5` takes"]
    );
}

/// An item holds at most [`MAX_RANK`] modifications of its own, with those
/// it comes with (items.tsv `start`).
#[test]
fn more_modifications_than_an_item_holds_are_named() {
    let mut grant = on_the_test_data();
    grant.modifications = [1004, 1024, 1064, 1204]
        .map(|id| ("edge_helmet_5".to_owned(), id))
        .to_vec();
    assert_eq!(
        grant.check(
            Dictionary::shared(),
            Modifications::shared(),
            SkillsTree::shared()
        ),
        ["`edge_helmet_5`: 4 modifications of its own and the 0 it comes with, more than 3"]
    );
    grant.modifications.pop();
    assert_eq!(
        grant.check(
            Dictionary::shared(),
            Modifications::shared(),
            SkillsTree::shared()
        ),
        Vec::<String>::new()
    );
}

/// The grant with only the skills of the test data's tree, which predates
/// the medical tree (`medical-skills-1.json`).
fn without_medical_skills() -> NewAccount {
    let mut grant = shared().clone();
    grant
        .skills
        .retain(|&(id, _)| SkillsTree::shared().skill(id).is_some());
    grant
}

/// Skills the tree does not allow at the level are named.
#[test]
fn skills_past_the_level_are_named() {
    let mut grant = on_the_test_data();
    grant.level = 6;
    assert_eq!(
        grant.check(
            Dictionary::shared(),
            Modifications::shared(),
            SkillsTree::shared()
        ),
        ["level 6: 7 points spent at level 6"]
    );
}
