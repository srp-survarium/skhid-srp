use binary_config_parser::tree::ConfigValue;

use super::*;
use crate::v026g::game_data;
use crate::v026g::item_params::item_params;

fn dict(name: &str) -> u16 {
    Dictionary::shared().item_named(name).unwrap().dict_id
}

/// The test data is 0.26g's: seven of the 21 (the rest came with later
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
        .copied()
        .filter(|name| dictionary.item_named(name).is_some())
        .collect();
    assert_eq!(present, IN_THE_TEST_DATA);
    for name in IN_THE_TEST_DATA {
        let item = dictionary.item_named(name).unwrap();
        assert!(item.is_premium, "{name}");
        assert!(is_golden(item.dict_id), "{name}");
        assert!(
            item.cfg_name.starts_with("gameplay/items/weapons/wpn_"),
            "{name}"
        );
    }
    // Premium, but armour; premium, but no copy.
    assert!(!is_golden(dict("bm_legs_4_premium")));
    assert!(!is_golden(dict("ak_74n")));
    assert!(!is_golden(9999));
}

#[test]
fn a_golden_weapon_is_its_plain_weapon_in_a_craft() {
    assert_eq!(plain_name("ak_12_premium"), Some("ak_12"));
    assert_eq!(plain_name("akmn_premium_2"), Some("akmn"));
    assert_eq!(
        plain_name("izh_27_lancaster_premium_2"),
        Some("izh_27_lancaster")
    );
    assert_eq!(plain_name("neostead"), None);
    assert_eq!(plain_name("ak_74n"), None);
    assert_eq!(plain_name("bm_legs_4_premium"), None);
    let (golden, plain) = (dict("ak_74n_premium"), dict("ak_74n"));
    assert_eq!(kin(golden), plain);
    assert_eq!(kin(plain), plain);
    assert_eq!(kin(dict("vss_premium")), dict("vss"));
    // Anything else is itself.
    let legs = dict("bm_legs_4_premium");
    assert_eq!(kin(legs), legs);
    assert_eq!(kin(9999), 9999);
    // A golden weapon whose plain weapon the dictionary lacks: itself.
    let mut dictionary = Dictionary::shared().clone();
    dictionary.items.retain(|item| item.name != "ak_74n");
    assert_eq!(kin_in(&dictionary, golden), golden);
}

#[test]
fn a_new_golden_weapon_starts_with_the_silver_bonus_of_its_own() {
    assert_eq!(
        starting_modifications(dict("ak_74n_premium")),
        [(SILVER_BONUS, false)]
    );
    assert!(starting_modifications(dict("ak_74n")).is_empty());
    assert!(starting_modifications(dict("bm_legs_4_premium")).is_empty());
}

#[test]
fn srp_gives_a_golden_weapon_no_defaults_and_no_base_rarity() {
    for name in IN_THE_TEST_DATA {
        let params = item_params(dict(name)).unwrap();
        assert!(params.default_modifications.is_empty(), "{name}");
        assert_eq!(params.base_rarity_level, 0, "{name}");
    }
    // The premium armour keeps its bonuses.
    let legs = item_params(dict("bm_legs_4_premium")).unwrap();
    assert!(!legs.default_modifications.is_empty());
}

/// A golden weapon's config as the patch makes it: no default
/// modifications, base rarity 0.
fn patched(config: &[u8]) -> Vec<u8> {
    let mut config = ConfigValue::parse(config).unwrap();
    let ConfigValue::Named(root) = &mut config else {
        panic!("a config is a named table");
    };
    for (key, value) in root.iter_mut() {
        match (key.as_str(), value) {
            ("default_modifications", ConfigValue::Indexed(list)) => list.clear(),
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
fn data_without_the_golden_weapons_patch_is_refused() {
    let stock = game_data::shared();
    let error = check(stock).unwrap_err().to_string();
    for name in IN_THE_TEST_DATA {
        assert!(error.contains(&format!("{name} (")), "{error}");
    }
    assert!(error.contains("golden-weapons-1.json"), "{error}");

    // The seven patched: accepted.
    let dictionary = Dictionary::shared();
    let files: Vec<(String, Vec<u8>)> = IN_THE_TEST_DATA
        .iter()
        .map(|name| {
            let path = dictionary.item_named(name).unwrap().cfg_name.clone();
            let bytes = patched(stock.file(&path).unwrap());
            (path, bytes)
        })
        .collect();
    let data = stock.laid_over("the test data with the golden-weapons patch", files);
    assert_eq!(unpatched(&data, dictionary).unwrap(), Vec::<String>::new());
    assert_eq!(check(&data), Ok(()));
}
