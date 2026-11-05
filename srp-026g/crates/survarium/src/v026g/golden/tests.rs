use binary_config_parser::tree::ConfigValue;

use super::*;
use crate::v026g::game_data;
use crate::v026g::item_params::item_params;

fn dict(name: &str) -> u16 {
    Dictionary::shared().item_named(name).unwrap().dict_id
}

/// The test data is 0.26g's: seven of the 22 golden weapons (the rest came
/// with later patches) and the four premium armour pieces.
const IN_THE_TEST_DATA: [&str; 7] = [
    "kiparis_premium",
    "uzi_premium",
    "vityaz_premium",
    "vss_premium",
    "sr3m_premium",
    "izh_27_lancaster_premium",
    "ak_74n_premium",
];

/// The test data's premium armour: `(name, its plain piece's name, its
/// bonuses)`, by dict id.
fn premium_armour() -> Vec<(&'static str, &'static str, &'static [u32])> {
    Dictionary::shared()
        .items
        .iter()
        .filter(|item| is_premium_armour(item.dict_id))
        .map(|item| {
            let name = item.name.as_str();
            (
                name,
                plain_name(name).unwrap(),
                armour_bonuses(name).unwrap(),
            )
        })
        .collect()
}

/// items.tsv's copies: `(dict id, its plain item's dict id, start)`, by
/// dict id.
fn copies_of_the_table() -> Vec<(u16, u16, Vec<u32>)> {
    let table = include_str!("../../../../../scripts/content-edit/items.tsv");
    let rows: Vec<Vec<&str>> = table
        .lines()
        .filter(|line| !line.starts_with('#'))
        .skip(1)
        .map(|line| line.split('\t').collect())
        .collect();
    let id_of = |name: &str| -> u16 {
        let row = rows.iter().find(|row| row[1] == name).unwrap();
        row[0].parse().unwrap()
    };
    rows.iter()
        .filter(|row| !row[4].is_empty())
        .map(|row| {
            let start = row[5].split(' ').map(|id| id.parse().unwrap());
            (row[0].parse().unwrap(), id_of(row[4]), start.collect())
        })
        .collect()
}

#[test]
fn the_golden_items_are_premium_copies_of_plain_items() {
    let dictionary = Dictionary::shared();
    let golden: Vec<&str> = dictionary
        .items
        .iter()
        .filter(|item| is_golden(item.dict_id))
        .map(|item| item.name.as_str())
        .collect();
    let weapons: Vec<&str> = golden
        .iter()
        .copied()
        .filter(|name| is_golden_weapon_name(name))
        .collect();
    assert_eq!(weapons, IN_THE_TEST_DATA);
    let armour: Vec<&str> = premium_armour().iter().map(|row| row.0).collect();
    assert_eq!(
        armour,
        [
            "scav_torso_4_premium",
            "scav_legs_4_premium",
            "bm_torso_4_premium",
            "bm_legs_4_premium"
        ]
    );
    assert_eq!(golden.len(), weapons.len() + armour.len());
    for name in golden {
        let item = dictionary.item_named(name).unwrap();
        assert!(item.is_premium && is_golden_name(name), "{name}");
        let plain = dictionary.item_named(plain_name(name).unwrap()).unwrap();
        assert!(!is_golden(plain.dict_id), "{name}");
        assert_eq!(plain.category, item.category, "{name}");
        assert_eq!(
            is_golden_weapon(item.dict_id),
            !is_premium_armour(item.dict_id),
            "{name}"
        );
    }
    // Golden, but armour; premium, but no copy.
    assert!(!is_golden_weapon(dict("bm_legs_4_premium")));
    assert!(is_golden(dict("bm_legs_4_premium")));
    assert!(!is_golden(dict("ak_74n")));
    assert!(!is_golden(9999));
    assert!(!is_golden_weapon(9999));
}

/// The premium armour is every armour piece with a base rarity in the
/// game's data: each a premium item whose stock config has its plain
/// piece's defaults, then its two premium bonuses; the plain piece has no
/// base rarity.
#[test]
fn the_premium_armour_is_the_plain_piece_with_two_premium_bonuses() {
    let dictionary = Dictionary::shared();
    let data = game_data::shared();
    let config = |name: &str| {
        let item = dictionary.item_named(name).unwrap();
        game_data::config(data, &item.cfg_name).unwrap().unwrap()
    };
    let defaults = |name: &str| -> Vec<u32> {
        config(name)
            .get("default_modifications")
            .map(|list| {
                list.children()
                    .into_iter()
                    .map(|id| id.as_i32().unwrap() as u32)
                    .collect()
            })
            .unwrap_or_default()
    };
    let rarity = |name: &str| {
        config(name)
            .get("parameters")
            .and_then(|p| p.get("base_rarity_level"))
            .and_then(ConfigValue::as_i32)
            .unwrap_or(0)
    };
    for (name, plain, bonuses) in premium_armour() {
        let mut expected = defaults(plain);
        assert_eq!(expected.len(), 1, "{plain}");
        expected.extend(bonuses);
        assert_eq!(defaults(name), expected, "{name}");
        assert_eq!(rarity(name), 3, "{name}");
        assert_eq!(rarity(plain), 0, "{plain}");
        assert_eq!(bonuses[0], EXPERIENCE_BONUS, "{name}");
    }
    // No other armour (nor any item but the golden weapons) has a base
    // rarity in the stock data.
    for item in &dictionary.items {
        let Ok(Some(config)) = game_data::config(data, &item.cfg_name) else {
            continue;
        };
        let rarity = config
            .get("parameters")
            .and_then(|p| p.get("base_rarity_level"))
            .and_then(ConfigValue::as_i32)
            .unwrap_or(0);
        assert!(
            rarity == 0 || is_golden_name(&item.name),
            "{} has base rarity {rarity}",
            item.name
        );
    }
}

/// The lobby's lists (`ItemDictId`'s golden weapons and premium armour, the
/// packs' `Equipment029` and `Equipment046`), for the gold background and
/// name, the craft window's donors and the bonuses of a shop entry, are
/// items.tsv's copies.
#[test]
fn the_lobby_knows_the_golden_items_of_the_table() {
    let source =
        include_str!("../../../../../flash/lobby_menu/src/survarium/constants/ItemDictId.as");
    // `<row>, // <name>` a line, up to the list's `];`.
    let rows = |list: &str| -> Vec<String> {
        let start = source
            .find(&format!("{list}:Array = ["))
            .unwrap_or_else(|| panic!("the lobby's {list}"));
        let rows = &source[start..];
        let rows = &rows[rows.find('\n').unwrap() + 1..rows.find("\n      ];").unwrap()];
        rows.lines()
            .map(|line| line.split_once("//").expect("a name for each row").0)
            .map(|row| row.trim().trim_end_matches(',').trim().to_owned())
            .collect()
    };
    let numbers = |row: &str| -> Vec<u32> {
        row.split(|c: char| !c.is_ascii_digit())
            .filter(|n| !n.is_empty())
            .map(|n| n.parse().unwrap())
            .collect()
    };
    let id = |row: &String| row.parse::<u16>().unwrap();
    let mut lobby: Vec<(u16, u16, Vec<u32>)> = Vec::new();
    for (golden, plain) in rows("GOLDEN_WEAPONS").iter().zip(rows("GOLDEN_PLAIN")) {
        lobby.push((id(golden), id(&plain), vec![SILVER_BONUS]));
    }
    let plain = rows("PREMIUM_ARMOUR_PLAIN");
    for ((armour, plain), bonuses) in rows("PREMIUM_ARMOUR")
        .iter()
        .zip(&plain)
        .zip(rows("PREMIUM_ARMOUR_BONUSES"))
    {
        lobby.push((id(armour), id(plain), numbers(&bonuses)));
    }
    for pack in [
        include_str!("../../../../../flash/lobby_menu/src/survarium/constants/Equipment029.as"),
        include_str!("../../../../../flash/lobby_menu/src/survarium/constants/Equipment046.as"),
    ] {
        for row in pack.lines().filter(|line| line.trim().starts_with("[")) {
            let [golden, plain, bonuses @ ..] = &numbers(row)[..] else {
                panic!("{row}")
            };
            lobby.push((*golden as u16, *plain as u16, bonuses.to_vec()));
        }
    }
    lobby.sort();
    assert_eq!(lobby, copies_of_the_table());
}

#[test]
fn a_golden_item_takes_its_plain_item_as_its_donor() {
    assert_eq!(
        plain_name("izh_27_lancaster_premium"),
        Some("izh_27_lancaster")
    );
    assert_eq!(plain_name("bm_legs_4_premium"), Some("bm_legs_4"));
    assert_eq!(plain_name("scav_torso_4_premium"), Some("scav_torso_4"));
    assert_eq!(plain_name("ak_74n"), None);
    assert_eq!(plain_name("bm_legs_4"), None);
    let (golden, plain) = (dict("ak_74n_premium"), dict("ak_74n"));
    assert_eq!(kin(golden), plain);
    assert_eq!(kin(plain), plain);
    assert_eq!(kin(dict("vss_premium")), dict("vss"));
    let (legs, plain_legs) = (dict("bm_legs_4_premium"), dict("bm_legs_4"));
    assert_eq!(kin(legs), plain_legs);
    // Anything else is itself.
    let other = dict("edge_legs_2");
    assert_eq!(kin(other), other);
    assert_eq!(kin(9999), 9999);

    // The donors: the same item; for a golden item its plain item too;
    // never a golden item for a plain one, nor another item.
    assert!(may_donate(plain, golden));
    assert!(may_donate(golden, golden));
    assert!(may_donate(plain, plain));
    assert!(may_donate(golden, plain));
    assert!(!may_donate(dict("vss"), golden));
    assert!(!may_donate(dict("vss_premium"), golden));
    assert!(!may_donate(dict("ak_74m"), plain));
    assert!(may_donate(plain_legs, legs));
    assert!(may_donate(legs, legs));
    assert!(may_donate(legs, plain_legs));
    assert!(!may_donate(dict("scav_legs_4"), legs));
    assert!(!may_donate(dict("bm_torso_4"), legs));
    assert!(!may_donate(dict("scav_legs_4_premium"), legs));
}

#[test]
fn a_new_golden_item_starts_with_its_bonuses_of_its_own() {
    assert_eq!(
        starting_modifications(dict("ak_74n_premium")),
        [(SILVER_BONUS, false)]
    );
    assert!(starting_modifications(dict("ak_74n")).is_empty());
    assert_eq!(
        starting_modifications(dict("bm_legs_4_premium")),
        [
            (EXPERIENCE_BONUS, false),
            (BLACK_MARKET_REPUTATION_BONUS, false)
        ]
    );
    assert_eq!(
        starting_modifications(dict("scav_torso_4_premium")),
        [
            (EXPERIENCE_BONUS, false),
            (SCAVENGER_REPUTATION_BONUS, false)
        ]
    );
    assert!(starting_modifications(dict("bm_legs_4")).is_empty());
    assert!(starting_modifications(9999).is_empty());
}

#[test]
fn the_dictionary_has_its_copies_and_each_start_is_known_modifications() {
    let data = game_data::shared();
    assert_eq!(check_content(data, Dictionary::shared()), Ok(()));
    // Data made without items.tsv: no copy, refused.
    let mut plain = Dictionary::shared().clone();
    plain.items.iter_mut().for_each(|item| item.like = None);
    let error = check_content(data, &plain).unwrap_err().0;
    assert!(error.contains("items.tsv"), "{error}");
    for start in [vec![1240, 1240], vec![99_999], vec![1240, 1250, 1260, 1230]] {
        let mut dictionary = Dictionary::shared().clone();
        dictionary.items[0].start = start.clone();
        assert!(check_content(data, &dictionary).is_err(), "{start:?}");
    }
}

#[test]
fn srp_gives_a_golden_item_only_its_plain_defaults_and_no_base_rarity() {
    for name in IN_THE_TEST_DATA {
        let params = item_params(dict(name)).unwrap();
        assert!(params.default_modifications.is_empty(), "{name}");
        assert_eq!(params.base_rarity_level, 0, "{name}");
    }
    // The premium armour keeps its plain piece's default.
    for (name, plain, _) in premium_armour() {
        let params = item_params(dict(name)).unwrap();
        let plain = item_params(dict(plain)).unwrap();
        assert_eq!(params.default_modifications.len(), 1, "{name}");
        assert_eq!(
            params.default_modifications, plain.default_modifications,
            "{name}"
        );
        assert_eq!(params.base_rarity_level, 0, "{name}");
    }
}

/// A golden item's config as its patch makes it: no default modification
/// but `keep`, base rarity 0.
fn patched(config: &[u8], keep: &[i32]) -> Vec<u8> {
    let mut config = ConfigValue::parse(config).unwrap();
    let ConfigValue::Named(root) = &mut config else {
        panic!("a config is a named table");
    };
    for (key, value) in root.iter_mut() {
        match (key.as_str(), value) {
            ("default_modifications", ConfigValue::Indexed(list)) => {
                list.retain(|id| id.as_i32().is_some_and(|id| keep.contains(&id)))
            }
            ("parameters", ConfigValue::Named(parameters)) => {
                for (name, value) in parameters.iter_mut() {
                    if name == "base_rarity_level" {
                        *value = ConfigValue::Integer(0);
                    }
                }
            }
            _ => {}
        }
    }
    config.to_bytes().unwrap()
}

#[test]
fn data_without_the_golden_patches_is_refused() {
    let stock = game_data::shared();
    let error = check(stock).unwrap_err().to_string();
    for name in IN_THE_TEST_DATA {
        assert!(error.contains(&format!("{name} (")), "{error}");
    }
    for (name, ..) in premium_armour() {
        assert!(error.contains(&format!("{name} (")), "{error}");
    }
    assert!(error.contains("weapon-stats.tsv"), "{error}");
    assert!(error.contains("armor.tsv"), "{error}");

    let dictionary = Dictionary::shared();
    let config_of = |name: &str| dictionary.item_named(name).unwrap().cfg_name.clone();
    let weapons: Vec<(String, Vec<u8>)> = IN_THE_TEST_DATA
        .iter()
        .map(|name| {
            let path = config_of(name);
            let bytes = patched(stock.file(&path).unwrap(), &[]);
            (path, bytes)
        })
        .collect();
    // The weapons patched, the armour not: the armour refused.
    let data = stock.laid_over(
        "the test data with the golden-weapons patch",
        weapons.clone(),
    );
    let error = check(&data).unwrap_err().to_string();
    assert!(!error.contains("weapon-stats.tsv"), "{error}");
    assert!(error.contains("armor.tsv"), "{error}");
    let found: Vec<&'static str> = unpatched(&data, dictionary)
        .unwrap()
        .into_iter()
        .map(|(_, patch)| patch)
        .collect();
    assert_eq!(found, [ARMOUR_PATCH; 4]);

    // A premium piece that keeps one of its bonuses: refused.
    let (name, plain, &[_, reputation]) = premium_armour()[2] else {
        panic!("two bonuses")
    };
    let plain_default = item_params(dict(plain)).unwrap().default_modifications[0] as i32;
    let path = config_of(name);
    let half = patched(
        stock.file(&path).unwrap(),
        &[plain_default, reputation as i32],
    );
    let mut files = weapons.clone();
    files.push((path, half));
    let data = stock.laid_over("one bonus left", files);
    assert!(
        unpatched(&data, dictionary)
            .unwrap()
            .iter()
            .any(|(item, _)| item.starts_with(&format!("{name} ("))),
    );

    // Every one patched (the armour keeps its plain piece's default):
    // accepted.
    let mut files = weapons;
    for (name, plain, _) in premium_armour() {
        let keep = item_params(dict(plain)).unwrap().default_modifications[0] as i32;
        let path = config_of(name);
        let bytes = patched(stock.file(&path).unwrap(), &[keep]);
        files.push((path, bytes));
    }
    let data = stock.laid_over("the test data with the golden patches", files);
    assert_eq!(unpatched(&data, dictionary).unwrap(), Vec::new());
    assert_eq!(check(&data), Ok(()));
}

/// The lobby shows a golden item not owned (the shop's, a bundle's) with
/// the bonuses srp gives it (`ItemFactory.startingModifiers`): its
/// `BONUS_UI` rows are those modifications' `ui_desc`, as the exe gives the
/// movie a modification (icon, one `{prop_id, prop_value}`), and hold every
/// bonus srp gives.
#[test]
fn the_lobby_shows_the_bonuses_of_the_data() {
    let source = include_str!("../../../../../flash/lobby_menu/src/survarium/item/ItemFactory.as");
    let start = source
        .find("BONUS_UI:Array = [")
        .expect("the lobby's BONUS_UI");
    let rows = &source[start..];
    let rows = &rows[rows.find('\n').unwrap() + 1..rows.find("\n      ];").unwrap()];
    let lobby: Vec<String> = rows
        .lines()
        .map(|line| {
            let (row, _) = line.split_once("//").expect("a comment for each row");
            row.trim().trim_end_matches(',').trim().to_owned()
        })
        .collect();
    assert!(source.contains(&format!("const SILVER_BONUS:int = {SILVER_BONUS};")));
    let modifications = game_data::config(
        game_data::shared(),
        "gameplay/items/modifications/modifications.options",
    )
    .unwrap()
    .unwrap();
    let rows = modifications.req("modifications").unwrap().children();
    let mut given: Vec<u32> = copies_of_the_table()
        .into_iter()
        .flat_map(|(.., start)| start)
        .collect();
    given.sort();
    given.dedup();
    let data: Vec<String> = given
        .iter()
        .map(|&id| {
            let row = rows
                .iter()
                .find(|row| row.i32_at("id").unwrap() == id as i32)
                .unwrap_or_else(|| panic!("modification {id}"));
            let ui = row.req("ui_desc").unwrap();
            let props = ui.req("props_list").unwrap().children();
            let [prop] = props[..] else {
                panic!("{id}: one prop: {props:?}")
            };
            format!(
                "[{id},{},{},\"{}\"]",
                ui.i32_at("icon").unwrap(),
                prop.i32_at("prop_id").unwrap(),
                prop.str_at("prop_value").unwrap()
            )
        })
        .collect();
    assert_eq!(lobby, data);
}
