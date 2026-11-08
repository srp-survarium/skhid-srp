use super::*;

/// The table as the game data has it (`new-account.tsv`): the owner's
/// numbers; the Novice's backpack on the back, three pockets of three.
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
        (grant.silver, grant.premium_days),
        (30_000, 30)
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
    assert_eq!(at(3), Some("stk2_novice_026_back"));
    let pockets: Vec<(u8, &str)> = POCKET_SLOTS
        .iter()
        .filter_map(|&slot| at(slot).map(|name| (slot, name)))
        .collect();
    assert_eq!(
        pockets,
        [(13, "medkit"), (14, "bandages"), (15, "antitoxin")]
    );
    assert_eq!(grant.pocket_units, 3);
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
/// data's), the gear opens two pockets of two. No AKMSU (the test data's
/// predates it).
#[test]
fn pockets_the_gear_does_not_give_are_named() {
    let mut grant = without_medical_skills();
    grant.own.retain(|(name, _)| name != "akmsu");
    grant.all_kits.retain(|(slot, _)| *slot != 0);
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
        &"kit 1: slot 15: the gear opens 2 pockets of 2 units, 3 of `antitoxin` asked".into()
    ));
    grant.pocket_units = 2;
    grant.all_kits.retain(|(slot, _)| *slot != 15);
    assert_eq!(
        grant.check(Dictionary::shared(), table, SkillsTree::shared()),
        Vec::<String>::new()
    );
}

/// The grant on the test data: refused, naming the AKMSU, the knit cap
/// and the Novice's backpack of patches it predates; the released data has them
/// (`session`'s `tests/starter_kit.rs`).
#[test]
fn the_test_data_lacks_three_items_of_the_grant() {
    let error = check_data(game_data::shared()).unwrap_err().to_string();
    assert!(
        error.contains(
            "gameplay/srp/new_account.options: no item `akmsu`, no item `novice_026_helmet`, \
             no item `stk2_novice_026_back`. "
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

/// The grant with the gear of the test data (no knit cap, no AKMSU, the oxygen
/// tank on the back) and its skills ([`without_medical_skills`]): what it
/// checks of what it owns and of its modifications.
fn on_the_test_data() -> NewAccount {
    let mut grant = without_medical_skills();
    grant.own.retain(|(name, _)| name != "akmsu");
    grant.all_kits.retain(|(slot, _)| *slot != 0 && *slot != 15);
    grant
        .all_kits
        .iter_mut()
        .find(|(slot, _)| *slot == 3)
        .unwrap()
        .1 = "scav_oxygen_1".into();
    grant.pocket_units = 2;
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
        "kit 1: slot 14: 2 of `bandages` asked, 0 owned (`own/bandages`)"
    );
}

/// A modification of a type its item does not take, on a stack, or on an
/// item no kit wears, is named; one beside a default of its type (the
/// gloves' 1133 with their 1130) is the owner's choice and passes.
#[test]
fn a_modification_its_item_does_not_take_is_named() {
    let table = Modifications::shared();
    let grant = on_the_test_data();
    let with = |item: &str, id: u32| {
        let mut grant = grant.clone();
        grant.modifications = vec![(item.to_owned(), id)];
        grant.check(Dictionary::shared(), table, SkillsTree::shared())
    };
    assert_eq!(with("scav_gloves_2", 1133), Vec::<String>::new());
    // The respirator's isolation on the gloves' collection time slot, and
    // the reverse: of a type the other does not take.
    assert_eq!(
        with("scav_gloves_2", 1183),
        ["modification 1183 is none `scav_gloves_2` takes"]
    );
    assert_eq!(
        with("medkit", 1133),
        ["modification 1133 is none `medkit` takes"]
    );
    assert_eq!(
        with("tt_33", 1133),
        [
            "modification 1133 is none `tt_33` takes",
            "modification 1133 of `tt_33`, which no kit wears"
        ]
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
