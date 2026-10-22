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
    // Premium, but armour; premium, but no copy.
    assert!(!is_golden(dict("bm_legs_4_premium")));
    assert!(!is_golden(dict("ak_74n")));
    assert!(!is_golden(9999));
}

/// The lobby's lists (`ItemDictId.GOLDEN_WEAPONS` and `GOLDEN_PLAIN`, for
/// the gold background and the craft window's donors) are this table's
/// dict ids, each named in its comment.
#[test]
fn the_lobby_knows_the_same_golden_weapons() {
    let source =
        include_str!("../../../../../flash/lobby_menu/src/survarium/constants/ItemDictId.as");
    // `<dict id>, // <name>` a line.
    let lobby = |list: &str| -> Vec<(u16, String)> {
        let start = source
            .find(&format!("{list}:Array = ["))
            .unwrap_or_else(|| panic!("the lobby's {list}"));
        let rows = &source[start..];
        let rows = &rows[rows.find('[').unwrap() + 1..rows.find(']').unwrap()];
        rows.lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| {
                let (id, name) = line.split_once("//").expect("a name for each id");
                let id = id.trim().trim_end_matches(',').trim().parse().unwrap();
                (id, name.trim().to_owned())
            })
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
    assert_eq!(lobby("GOLDEN_WEAPONS"), golden);
    assert_eq!(lobby("GOLDEN_PLAIN"), plain);
}

#[test]
fn a_golden_weapon_takes_its_plain_weapon_as_its_donor() {
    assert_eq!(plain_name("ak_12_premium"), Some("ak_12"));
    assert_eq!(plain_name("akmn_premium_2"), Some("akmn"));
    assert_eq!(
        plain_name("izh_27_lancaster_premium_2"),
        Some("izh_27_lancaster")
    );
    assert_eq!(plain_name("wpn_mp7_silencer_special"), Some("mp7_silencer"));
    assert_eq!(plain_name("neostead"), Some("neostead_2"));
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

    // The donors: the same item; for a golden weapon its plain weapon too;
    // never a golden weapon for a plain one, nor another weapon.
    assert!(may_donate(plain, golden));
    assert!(may_donate(golden, golden));
    assert!(may_donate(plain, plain));
    assert!(!may_donate(golden, plain));
    assert!(!may_donate(dict("vss"), golden));
    assert!(!may_donate(dict("vss_premium"), golden));
    assert!(!may_donate(dict("ak_74m"), plain));
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

/// The lobby shows a golden weapon not owned (the shop's, a bundle's) with
/// the silver bonus srp gives it (`ItemFactory.startingModifiers`): its
/// `GOLDEN_SILVER_*` constants are [`SILVER_BONUS`]'s `ui_desc`, as the exe
/// gives the movie a modification (icon, one `{prop_id, prop_value}`).
#[test]
fn the_lobby_shows_the_silver_bonus_of_the_data() {
    let source = include_str!("../../../../../flash/lobby_menu/src/survarium/item/ItemFactory.as");
    let lobby = |name: &str| -> String {
        let start = source
            .find(&format!("const {name}:"))
            .unwrap_or_else(|| panic!("the lobby's {name}"));
        let line = &source[start..];
        let line = &line[line.find('=').unwrap() + 1..line.find(';').unwrap()];
        line.trim().trim_matches('"').to_owned()
    };
    let modifications = game_data::config(
        game_data::shared(),
        "gameplay/items/modifications/modifications.options",
    )
    .unwrap()
    .unwrap();
    let rows = modifications.req("modifications").unwrap().children();
    let silver = rows
        .into_iter()
        .find(|row| row.i32_at("id").unwrap() == SILVER_BONUS as i32)
        .expect("the silver bonus");
    let ui = silver.req("ui_desc").unwrap();
    let props = ui.req("props_list").unwrap().children();
    let [prop] = props[..] else {
        panic!("one prop: {props:?}")
    };
    assert_eq!(
        lobby("GOLDEN_SILVER_ICON"),
        ui.i32_at("icon").unwrap().to_string()
    );
    assert_eq!(
        lobby("GOLDEN_SILVER_PROP_ID"),
        prop.i32_at("prop_id").unwrap().to_string()
    );
    assert_eq!(
        lobby("GOLDEN_SILVER_PROP_VALUE"),
        prop.str_at("prop_value").unwrap()
    );
}
