use binary_config_parser::tree::ConfigValue;

use super::*;
use crate::v026g::game_data;
use crate::v026g::item_params::item_params;

fn dict(name: &str) -> u16 {
    Dictionary::shared().item_named(name).unwrap().dict_id
}

/// The test data is 0.26g's: seven of the 22 (the rest came with later
/// patches), each a premium item of the dictionary.
const IN_THE_TEST_DATA: [&str; 7] = [
    "kiparis_premium",
    "uzi_premium",
    "vityaz_premium",
    "vss_premium",
    "sr3m_premium",
    "izh_27_lancaster_premium",
    "ak_74n_premium",
];

#[test]
fn the_golden_weapons_are_the_premium_copies_with_the_silver_bonus() {
    let dictionary = Dictionary::shared();
    let present: Vec<&str> = GOLDEN_WEAPONS
        .iter()
        .map(|&(name, ..)| name)
        .filter(|name| dictionary.item_named(name).is_some())
        .collect();
    assert_eq!(present, IN_THE_TEST_DATA);
    for (name, plain, dict_id, plain_id) in GOLDEN_WEAPONS {
        let Some(item) = dictionary.item_named(name) else {
            continue;
        };
        // The dict id the table and the lobby give it.
        assert_eq!(item.dict_id, dict_id, "{name}");
        assert!(item.is_premium, "{name}");
        assert!(is_golden(item.dict_id), "{name}");
        assert!(
            item.cfg_name.starts_with("gameplay/items/weapons/wpn_"),
            "{name}"
        );
        // Its plain weapon is plain, of the dict id the table gives.
        let plain = dictionary.item_named(plain).unwrap();
        assert_eq!(plain.dict_id, plain_id, "{name}");
        assert!(!is_golden(plain.dict_id), "{name}");
        assert!(!plain.is_premium || plain.name == "ak_74n", "{name}");
    }
    // Each once, by name and by dict id.
    for (i, a) in GOLDEN_WEAPONS.iter().enumerate() {
        for b in &GOLDEN_WEAPONS[i + 1..] {
            assert!(a.0 != b.0 && a.2 != b.2, "{a:?} {b:?}");
        }
    }
    // Golden, but armour; premium, but no copy.
    assert!(!is_golden_weapon(dict("bm_legs_4_premium")));
    assert!(is_golden(dict("bm_legs_4_premium")));
    assert!(!is_golden(dict("ak_74n")));
    assert!(!is_golden(9999));
    assert!(!is_golden_weapon(9999));
}

/// The premium armour is every armour piece with a base rarity in the
/// game's data: each a premium item whose config has its plain piece's
/// defaults, then its two premium bonuses; the plain piece has no base
/// rarity.
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
    for (name, plain, dict_id, plain_id, bonuses) in PREMIUM_ARMOUR {
        let item = dictionary.item_named(name).unwrap();
        assert_eq!(item.dict_id, dict_id, "{name}");
        assert!(item.is_premium, "{name}");
        assert!(
            item.cfg_name.starts_with("gameplay/items/armour/"),
            "{name}"
        );
        assert!(is_golden(dict_id) && is_premium_armour(dict_id), "{name}");
        assert!(!is_golden_weapon(dict_id), "{name}");
        assert_eq!(armour_bonuses(name), Some(&bonuses[..]), "{name}");
        let plain_item = dictionary.item_named(plain).unwrap();
        assert_eq!(plain_item.dict_id, plain_id, "{name}");
        assert!(!is_golden(plain_id), "{name}");
        // The stock config: the plain piece's defaults, then the bonuses;
        // base rarity 3 (the plain piece 0).
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

/// The lobby's lists (`ItemDictId.GOLDEN_WEAPONS` and `GOLDEN_PLAIN`,
/// `PREMIUM_ARMOUR`, `PREMIUM_ARMOUR_PLAIN` and `PREMIUM_ARMOUR_BONUSES`,
/// for the gold background and name, the craft window's donors and the
/// bonuses of a shop entry) are these tables', each row named in its
/// comment.
#[test]
fn the_lobby_knows_the_same_golden_items() {
    let source =
        include_str!("../../../../../flash/lobby_menu/src/survarium/constants/ItemDictId.as");
    // `<row>, // <name>` a line, up to the list's `];`.
    let rows = |list: &str| -> Vec<(String, String)> {
        let start = source
            .find(&format!("{list}:Array = ["))
            .unwrap_or_else(|| panic!("the lobby's {list}"));
        let rows = &source[start..];
        let rows = &rows[rows.find('\n').unwrap() + 1..rows.find("\n      ];").unwrap()];
        rows.lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| {
                let (row, name) = line.split_once("//").expect("a name for each row");
                (
                    row.trim().trim_end_matches(',').trim().to_owned(),
                    name.trim().to_owned(),
                )
            })
            .collect()
    };
    let ids = |list: &str| -> Vec<(u16, String)> {
        rows(list)
            .into_iter()
            .map(|(id, name)| (id.parse().unwrap(), name))
            .collect()
    };
    let golden: Vec<(u16, String)> = GOLDEN_WEAPONS
        .iter()
        .map(|&(name, _, id, _)| (id, name.to_owned()))
        .collect();
    let plain: Vec<(u16, String)> = GOLDEN_WEAPONS
        .iter()
        .map(|&(_, name, _, id)| (id, name.to_owned()))
        .collect();
    assert_eq!(ids("GOLDEN_WEAPONS"), golden);
    assert_eq!(ids("GOLDEN_PLAIN"), plain);
    let armour: Vec<(u16, String)> = PREMIUM_ARMOUR
        .iter()
        .map(|&(name, _, id, ..)| (id, name.to_owned()))
        .collect();
    let plain: Vec<(u16, String)> = PREMIUM_ARMOUR
        .iter()
        .map(|&(_, name, _, id, _)| (id, name.to_owned()))
        .collect();
    let bonuses: Vec<(String, String)> = PREMIUM_ARMOUR
        .iter()
        .map(|&(name, .., [a, b])| (format!("[{a},{b}]"), name.to_owned()))
        .collect();
    assert_eq!(ids("PREMIUM_ARMOUR"), armour);
    assert_eq!(ids("PREMIUM_ARMOUR_PLAIN"), plain);
    assert_eq!(rows("PREMIUM_ARMOUR_BONUSES"), bonuses);
}

#[test]
fn a_golden_item_takes_its_plain_item_as_its_donor() {
    assert_eq!(plain_name("ak_12_premium"), Some("ak_12"));
    assert_eq!(plain_name("akmn_premium_2"), Some("akmn"));
    assert_eq!(
        plain_name("izh_27_lancaster_premium_2"),
        Some("izh_27_lancaster")
    );
    assert_eq!(plain_name("wpn_mp7_silencer_special"), Some("mp7_silencer"));
    assert_eq!(plain_name("neostead"), Some("neostead_2"));
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
    // A golden weapon whose plain weapon the dictionary lacks: itself.
    let mut dictionary = Dictionary::shared().clone();
    dictionary.items.retain(|item| item.name != "ak_74n");
    assert_eq!(kin_in(&dictionary, golden), golden);

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
fn srp_gives_a_golden_item_only_its_plain_defaults_and_no_base_rarity() {
    for name in IN_THE_TEST_DATA {
        let params = item_params(dict(name)).unwrap();
        assert!(params.default_modifications.is_empty(), "{name}");
        assert_eq!(params.base_rarity_level, 0, "{name}");
    }
    // The premium armour keeps its plain piece's default.
    for (name, plain, ..) in PREMIUM_ARMOUR {
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
    for (name, ..) in PREMIUM_ARMOUR {
        assert!(error.contains(&format!("{name} (")), "{error}");
    }
    assert!(error.contains("golden-weapons-1.json"), "{error}");
    assert!(error.contains("premium-armour-1.json"), "{error}");

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
    assert!(!error.contains("golden-weapons-1.json"), "{error}");
    assert!(error.contains("premium-armour-1.json"), "{error}");
    let found: Vec<&'static str> = unpatched(&data, dictionary)
        .unwrap()
        .into_iter()
        .map(|(_, patch)| patch)
        .collect();
    assert_eq!(found, [ARMOUR_PATCH; 4]);

    // A premium piece that keeps one of its bonuses: refused.
    let (name, plain, .., [_, reputation]) = PREMIUM_ARMOUR[2];
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
    for (name, plain, ..) in PREMIUM_ARMOUR {
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
    let mut given: Vec<u32> = vec![SILVER_BONUS];
    for (.., bonuses) in PREMIUM_ARMOUR {
        for bonus in bonuses {
            if !given.contains(&bonus) {
                given.push(bonus);
            }
        }
    }
    // The new equipment pack also grants the other two factions' native bonuses.
    given.extend([1270, 1280]);
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
