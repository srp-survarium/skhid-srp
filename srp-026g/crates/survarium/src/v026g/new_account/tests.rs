use super::*;

/// A preset's kits as `(slot, item)`, kit by kit, by slot.
fn kits(preset: &Preset) -> Vec<Vec<(u8, &str)>> {
    (0..KITS)
        .map(|kit| sorted(preset.kit(kit).collect()))
        .collect()
}

fn sorted(mut worn: Vec<(u8, &str)>) -> Vec<(u8, &str)> {
    worn.sort_unstable();
    worn
}

/// The table as the game data has it (`new-account.tsv`): the owner's
/// numbers; preset 1 the kits of before, three pockets of three; preset 2
/// the Keeper set, the AK-12 in its first kit and the Saiga-12 in its
/// second (the owner's of 2026-11-08).
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
    assert_eq!(grant.silver, 30_000);
    assert_eq!(
        (grant.rounds, grant.faction.id(), grant.reputation_level),
        (1000, 1, 5)
    );
    let gear = [
        (0, "novice_026_helmet"),
        (1, "scav_resp_1"),
        (2, "scav_torso_4"),
        (3, "stk2_novice_026_back"),
        (4, "scav_legs_2"),
        (5, "scav_gloves_2"),
        (6, "scav_boots_2"),
        (13, "medkit"),
        (14, "bandages"),
        (15, "antitoxin"),
    ];
    let with = |more: [(u8, &'static str); 2]| sorted([&gear[..], &more[..]].concat());
    assert_eq!(
        kits(&grant.active),
        [
            with([(7, "ak_74n"), (8, "ammo_5.45x39_7n6m")]),
            with([(7, "rem_700"), (8, "ammo_7.62x51")]),
            with([(7, "mp_153"), (8, "ammo_12mm_buck")]),
        ]
    );
    assert_eq!(grant.active.pocket_units, 3);
    assert_eq!(grant.saved.len(), 1);
    let gear = [
        (0, "edge_helmet_5"),
        (1, "edge_mask_5"),
        (3, "edge_back_5"),
        (5, "edge_gloves_5"),
        (6, "edge_boots_5"),
        (13, "medkit"),
        (14, "speed_booster"),
        (15, "antitoxin"),
    ];
    // Kit 3: nothing (the owner's: "no need for e2k3").
    let with = |more: [(u8, &'static str); 4]| sorted([&gear[..], &more[..]].concat());
    assert_eq!(
        kits(&grant.saved[0]),
        [
            with([
                (2, "edge_torso_5_special"),
                (4, "edge_legs_5_special"),
                (7, "ak_12"),
                (8, "ammo_6.5x39")
            ]),
            with([
                (2, "edge_torso_5_special"),
                (4, "edge_legs_5_special"),
                (7, "saiga_12"),
                (8, "ammo_12mm_buck2")
            ]),
            Vec::new(),
        ]
    );
    assert_eq!(grant.saved[0].pocket_units, 2);
    // The owner's of 2026-11-08: each Keeper piece with armour +8 (1004)
    // and weight -15 % (1064), the strongest of their kinds; but the mask,
    // which takes no armour (weight only), the container, which takes none
    // either (weight and carried weight +5 kg, 1124), the weapons, weight -15 %
    // of their own kind (2064), and the Keeper of the Sun, which
    // comes with two (experience +20 % and reputation +50 %, its `start`) and so
    // takes one more (armour).
    let modifications: Vec<(&str, u32)> = grant
        .modifications
        .iter()
        .map(|(name, id)| (name.as_str(), *id))
        .collect();
    assert_eq!(
        modifications,
        [
            ("scav_resp_1", 1183),
            ("scav_gloves_2", 1133),
            ("edge_helmet_5", 1004),
            ("edge_helmet_5", 1064),
            ("edge_mask_5", 1064),
            ("edge_gloves_5", 1004),
            ("edge_gloves_5", 1064),
            ("edge_gloves_5", 1084),
            ("edge_boots_5", 1004),
            ("edge_boots_5", 1064),
            ("edge_back_5", 1064),
            ("edge_back_5", 1124),
            ("edge_torso_5_special", 1004),
            ("edge_legs_5_special", 1004),
            ("ak_12", 2064),
            ("ak_12", 2084),
            ("ak_12", 2074),
            ("saiga_12", 2064),
            ("saiga_12", 2084),
            ("saiga_12", 2074)
        ]
    );
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
            ("glock_17", 1),
            ("drop_card", 50),
            ("medkit", 50),
            ("antitoxin", 50),
            ("bandages", 20),
            ("speed_booster", 10)
        ]
    );
}

/// The grant's table with `edit` made to its keys.
fn edited(edit: impl FnOnce(&mut Vec<(String, ConfigValue)>)) -> Result<NewAccount, String> {
    let mut entries = match ConfigValue::parse(game_data::file(PATH)).unwrap() {
        ConfigValue::Named(entries) => entries,
        other => panic!("{other:?}"),
    };
    edit(&mut entries);
    NewAccount::parse(&ConfigValue::Named(entries).to_bytes().unwrap())
}

/// A key missing, one it has not, a slot no kit has, a preset after one
/// missing: refused, named.
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
        Err(format!("{PATH}: no `preset_1`"))
    );
    assert_eq!(
        table(vec![("kit_1", ConfigValue::Named(Vec::new()))]),
        Err(format!("{PATH}: `kit_1` is no key of it"))
    );
    assert_eq!(
        table(vec![("preset_6", ConfigValue::Named(Vec::new()))]),
        Err(format!("{PATH}: `preset_6` is no key of it"))
    );
    let worn = ConfigValue::Named(vec![("23".into(), ConfigValue::String("ppsh".into()))]);
    assert_eq!(
        table(vec![(
            "preset_1",
            ConfigValue::Named(vec![("kit_1".into(), worn)])
        )]),
        Err(format!("{PATH}: `preset_1/kit_1/23`: no slot of a kit"))
    );
    assert_eq!(
        table(vec![(
            "preset_1",
            ConfigValue::Named(vec![("kit_4".into(), ConfigValue::Named(Vec::new()))])
        )]),
        Err(format!("{PATH}: `preset_1/kit_4` is no key of it"))
    );
    // Preset 2 named preset 3: preset 2 is missing.
    let renamed = edited(|entries| {
        entries
            .iter_mut()
            .find(|(key, _)| key == "preset_2")
            .unwrap()
            .0 = "preset_3".into();
    });
    assert_eq!(renamed, Err(format!("{PATH}: no `preset_2`")));
    let without = edited(|entries| {
        let ConfigValue::Named(preset) = &mut entries
            .iter_mut()
            .find(|(key, _)| key == "preset_2")
            .unwrap()
            .1
        else {
            panic!()
        };
        preset.retain(|(key, _)| key != "pocket_units");
    });
    assert_eq!(without, Err(format!("{PATH}: no `preset_2/pocket_units`")));
}

/// Pockets the gear does not open, or that take fewer units than the
/// table gives, are named: preset 1's three pockets of three, with the
/// oxygen tank on the back, two pockets of two.
#[test]
fn pockets_the_gear_does_not_give_are_named() {
    let mut grant = on_the_test_data();
    grant.active.pocket_units = 3;
    grant.active.all_kits.push((15, "antitoxin".into()));
    let table = Modifications::shared();
    let wrong = grant.check(Dictionary::shared(), table, SkillsTree::shared());
    assert_eq!(wrong.len(), 3 * 3, "{wrong:?}");
    assert!(
        wrong.contains(
            &"preset 1 kit 1: slot 15: the gear opens 2 pockets of 2 units, 3 of `antitoxin` asked"
                .into()
        )
    );
}

/// A modification of the grant opens a pocket as the item's own do: preset
/// 2's container given a pocket of its own (1030; the released data has it
/// built in, the test data does not) opens the third. Without it the
/// antitoxins' pocket is shut.
#[test]
fn a_pocket_a_modification_of_the_grant_opens_is_open() {
    let table = Modifications::shared();
    let mut grant = on_the_test_data();
    grant.saved = shared().saved.clone();
    grant.modifications = shared().modifications.clone();
    // Kit 2: the test data has the container, not the Keeper of the Sun nor
    // the Saiga-12: the Keeper vest (its pocket, 1030) in the Sun's place.
    let lacks = |name: &str| Dictionary::shared().item_named(name).is_none();
    grant
        .modifications
        .retain(|(name, _)| !lacks(name) && name != "ak_12");
    grant.modifications.push(("edge_back_5".into(), 1030));
    let preset = &mut grant.saved[0];
    preset.kits[0].clear();
    preset.kits[1].retain(|(_, name)| !lacks(name));
    preset.kits[1].push((2, "edge_torso_5".into()));
    assert_eq!(
        grant.check(Dictionary::shared(), table, SkillsTree::shared()),
        Vec::<String>::new()
    );
    grant
        .modifications
        .retain(|(name, id)| !(name == "edge_back_5" && *id == 1030));
    let wrong = grant.check(Dictionary::shared(), table, SkillsTree::shared());
    assert_eq!(
        wrong,
        ["preset 2 kit 2: slot 15: the gear opens 2 pockets of 2 units, 2 of `antitoxin` asked"]
    );
}

/// The grant on the test data: refused, naming the items of patches it
/// predates; the released data has them (`session`'s
/// `tests/starter_kit.rs`).
#[test]
fn the_test_data_lacks_items_of_the_grant() {
    let error = check_data(game_data::shared()).unwrap_err().to_string();
    assert!(
        error.contains("gameplay/srp/new_account.options: no item `akmsu`, "),
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
    let grant = edited(|entries| {
        entries
            .iter_mut()
            .find(|(key, _)| key == "faction")
            .unwrap()
            .1 = ConfigValue::Integer(5);
    });
    assert_eq!(
        grant,
        Err(format!(
            "{PATH}: `faction` is 5, not one whose quests the lobby shows ([1, 2, 3, 4])"
        ))
    );
}

/// An item's modifications are a list of ids, one or more; a single id is
/// refused, naming the key.
#[test]
fn an_items_modifications_are_a_list() {
    let grant = edited(|entries| {
        entries
            .iter_mut()
            .find(|(key, _)| key == "own_modifications")
            .unwrap()
            .1 = ConfigValue::Named(vec![
            (
                "edge_mask_5".into(),
                ConfigValue::Indexed(vec![ConfigValue::Integer(1064)]),
            ),
            ("edge_gloves_5".into(), ConfigValue::Integer(1004)),
        ]);
    });
    assert_eq!(
        grant,
        Err(format!(
            "{PATH}: `own_modifications/edge_gloves_5` is Integer(1004), not a list of ids"
        ))
    );
}

/// The grant with what the test data has ([`without_medical_skills`]):
/// no AKMSU; preset 1 with no knit cap, the oxygen tank on the back, two
/// pockets of two; no preset 2, whose Keeper of the Sun, Saiga-12 and
/// Glock 17 it predates. What it checks of what it owns and of its
/// modifications.
fn on_the_test_data() -> NewAccount {
    let mut grant = without_medical_skills();
    grant
        .own
        .retain(|(name, _)| name != "akmsu" && name != "glock_17");
    let preset = &mut grant.active;
    preset
        .all_kits
        .retain(|(slot, _)| *slot != 0 && *slot != 15);
    preset
        .all_kits
        .iter_mut()
        .find(|(slot, _)| *slot == 3)
        .unwrap()
        .1 = "scav_oxygen_1".into();
    preset.pocket_units = 2;
    grant.saved.clear();
    let given = ["scav_resp_1", "scav_gloves_2"];
    grant
        .modifications
        .retain(|(name, _)| given.contains(&name.as_str()));
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
        "preset 1 kit 1: slot 14: 2 of `bandages` asked, 0 owned (`own/bandages`)"
    );
}

/// A modification of a type its item does not take, on a stack, or on an
/// item it is not given, is named; one on an item it owns but no kit wears
/// passes, and one beside a default of its type (the gloves' 1133 with
/// their 1130) is the owner's choice and passes.
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
    assert_eq!(
        with("scav_gloves_2", 1183),
        ["modification 1183 is none `scav_gloves_2` takes"]
    );
    assert_eq!(
        with("medkit", 1133),
        ["modification 1133 is none `medkit` takes"]
    );
    // The TT-33 is owned, unworn.
    assert_eq!(
        with("tt_33", 1133),
        ["modification 1133 is none `tt_33` takes"]
    );
    assert_eq!(
        with("edge_gloves_5", 1004),
        ["modification 1004 of `edge_gloves_5`, which it is not given"]
    );
    // The Keeper mask takes no armour.
    let mut keeper = grant.clone();
    keeper.own.push(("edge_mask_5".into(), 1));
    keeper.modifications = vec![("edge_mask_5".into(), 1004)];
    assert_eq!(
        keeper.check(Dictionary::shared(), table, SkillsTree::shared()),
        ["modification 1004 is none `edge_mask_5` takes"]
    );
}

/// An item holds at most [`MAX_RANK`] modifications of its own, with those
/// it comes with (items.tsv `start`).
#[test]
fn more_modifications_than_an_item_holds_are_named() {
    let mut grant = on_the_test_data();
    grant.own.push(("edge_helmet_5".into(), 1));
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
