use binary_config_parser::tree::ConfigValue;

use super::*;
use crate::v026g::game_data;
use crate::v026g::item_params::item_params;
use crate::v026g::scopes::WEAPON_CATEGORIES;

fn dict(name: &str) -> u16 {
    Dictionary::shared().item_named(name).unwrap().dict_id
}

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

/// The test data is 0.26g's: its copies are the four premium armour pieces
/// (the rest came with later patches). The golden weapons are no copies
/// (the owner, 2026-11-14): premium items of their own that nothing gives.
#[test]
fn the_golden_items_are_the_premium_armour() {
    let dictionary = Dictionary::shared();
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
    for name in armour {
        let item = dictionary.item_named(name).unwrap();
        assert!(item.is_premium, "{name}");
        assert!(!WEAPON_CATEGORIES.contains(&item.category), "{name}");
        let plain = dictionary.item_named(plain_name(name).unwrap()).unwrap();
        assert!(!is_premium_armour(plain.dict_id), "{name}");
        assert_eq!(plain.category, item.category, "{name}");
    }
    // No weapon is a copy, the old golden ones neither; premium, but no copy.
    assert!(
        dictionary
            .items
            .iter()
            .filter(|item| WEAPON_CATEGORIES.contains(&item.category))
            .all(|item| item.like.is_none() && item.start.is_empty())
    );
    assert!(!is_premium_armour(dict("sr3m_premium")));
    assert!(!is_premium_armour(dict("ak_74n")));
    assert!(!is_premium_armour(9999));
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
    // No other armour has a base rarity in the stock data.
    for item in &dictionary.items {
        if WEAPON_CATEGORIES.contains(&item.category) {
            continue;
        }
        let Ok(Some(config)) = game_data::config(data, &item.cfg_name) else {
            continue;
        };
        let rarity = config
            .get("parameters")
            .and_then(|p| p.get("base_rarity_level"))
            .and_then(ConfigValue::as_i32)
            .unwrap_or(0);
        assert!(
            rarity == 0 || is_premium_armour(item.dict_id),
            "{} has base rarity {rarity}",
            item.name
        );
    }
}

/// The lobby's lists (`ItemDictId`'s premium armour, the packs'
/// `Equipment029` and `Equipment046`), for the gold background and name,
/// the craft window's donors and the bonuses of a shop entry, are
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
fn a_premium_piece_takes_its_plain_piece_as_its_donor() {
    assert_eq!(plain_name("bm_legs_4_premium"), Some("bm_legs_4"));
    assert_eq!(plain_name("scav_torso_4_premium"), Some("scav_torso_4"));
    assert_eq!(plain_name("ak_74n_premium"), None);
    assert_eq!(plain_name("bm_legs_4"), None);
    let (legs, plain_legs) = (dict("bm_legs_4_premium"), dict("bm_legs_4"));
    assert_eq!(kin(legs), plain_legs);
    assert_eq!(kin(plain_legs), plain_legs);
    // Anything else is itself, a weapon too.
    let other = dict("edge_legs_2");
    assert_eq!(kin(other), other);
    assert_eq!(kin(dict("sr3m")), dict("sr3m"));
    assert_eq!(kin(9999), 9999);

    // The donors: the same item; a premium piece and its plain piece, both
    // ways; nothing else.
    assert!(may_donate(dict("sr3m"), dict("sr3m")));
    assert!(!may_donate(dict("sr3m_premium"), dict("sr3m")));
    assert!(!may_donate(dict("ak_74m"), dict("ak_74n")));
    assert!(may_donate(plain_legs, legs));
    assert!(may_donate(legs, legs));
    assert!(may_donate(legs, plain_legs));
    assert!(!may_donate(dict("scav_legs_4"), legs));
    assert!(!may_donate(dict("bm_torso_4"), legs));
    assert!(!may_donate(dict("scav_legs_4_premium"), legs));
}

#[test]
fn a_new_premium_piece_starts_with_its_bonuses_of_its_own() {
    // A weapon comes with none, an old golden one neither.
    assert!(starting_modifications(dict("ak_74n_premium")).is_empty());
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
fn srp_gives_a_premium_piece_only_its_plain_default_and_no_base_rarity() {
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

/// A premium piece's config as its patch makes it: no default modification
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
fn data_without_the_premium_armour_patch_is_refused() {
    // The stock data: the four pieces refused, and nothing else (its
    // golden weapons, stock, are ordinary items).
    let stock = game_data::shared();
    let error = check(stock).unwrap_err().to_string();
    for (name, ..) in premium_armour() {
        assert!(error.contains(&format!("{name} (")), "{error}");
    }
    assert!(error.contains(ARMOUR_PATCH), "{error}");
    let dictionary = Dictionary::shared();
    assert_eq!(unpatched(stock, dictionary).unwrap().len(), 4);
    let config_of = |name: &str| dictionary.item_named(name).unwrap().cfg_name.clone();

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
    let data = stock.laid_over("one bonus left", vec![(path, half)]);
    assert!(
        unpatched(&data, dictionary)
            .unwrap()
            .iter()
            .any(|item| item.starts_with(&format!("{name} ("))),
    );

    // Every one patched (the armour keeps its plain piece's default):
    // accepted.
    let mut files = Vec::new();
    for (name, plain, _) in premium_armour() {
        let keep = item_params(dict(plain)).unwrap().default_modifications[0] as i32;
        let path = config_of(name);
        let bytes = patched(stock.file(&path).unwrap(), &[keep]);
        files.push((path, bytes));
    }
    let data = stock.laid_over("the test data with the premium-armour patch", files);
    assert_eq!(unpatched(&data, dictionary).unwrap(), Vec::<String>::new());
    assert_eq!(check(&data), Ok(()));
}

/// The lobby shows a premium piece not owned (the shop's, a bundle's) with
/// the bonuses srp gives it (`ItemFactory.startingModifiers`): its
/// `BONUS_UI` rows are those modifications' `ui_desc`, as the exe gives the
/// movie a modification (icon, one `{prop_id, prop_value}`), and hold every
/// bonus srp gives, and the silver one (1230) a weapon that was golden may
/// hold (`ModificationRolls.isPremium` shows it gold, the most there is).
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
        .chain([1230])
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
            // Its value the movie reads from modifications.tsv (`bonusValue`),
            // as the content build writes the card line from it.
            assert!(prop.str_at("prop_value").is_ok(), "{id}");
            format!(
                "[{id},{},{}]",
                ui.i32_at("icon").unwrap(),
                prop.i32_at("prop_id").unwrap()
            )
        })
        .collect();
    assert_eq!(lobby, data);
}
