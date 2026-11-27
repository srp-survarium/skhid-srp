use binary_config_parser::tree::ConfigValue;

use super::super::game_data;
use super::fixtures::{self, COBRA, LONE_SCOPE, PSO_1};
use super::*;

fn dict(name: &str) -> u16 {
    Dictionary::shared().item_named(name).unwrap().dict_id
}

#[test]
fn game_data_without_the_table_has_no_pair() {
    // The test data has no modules table: nothing fits anything.
    assert!(pairs(Dictionary::shared(), Modules::shared()).is_empty());
    assert_eq!(check(game_data::shared(), Dictionary::shared()), Ok(()));
    // 0.26g's own three scopes are no modules of anything.
    for stock in [69, 304, 362] {
        assert!(!Modules::shared().is_module(stock));
    }
}

#[test]
fn the_pairs_are_the_modules_tables() {
    let data = fixtures::with_scopes();
    let dictionary = fixtures::dictionary_of(&data);
    let modules = fixtures::modules();
    let (ak, ak_premium, svd) = (dict("ak_74n"), dict("ak_74n_premium"), dict("svd"));
    let mut expected = vec![
        (ak, PSO_1),
        (ak, COBRA),
        (ak_premium, PSO_1),
        (ak_premium, COBRA),
        (svd, COBRA),
    ];
    expected.sort_unstable();
    assert_eq!(pairs(&dictionary, modules), expected);
    assert!(modules.is_module(PSO_1) && modules.is_module(COBRA));
    assert!(!modules.is_module(LONE_SCOPE) && !modules.is_module(304));
    assert_eq!(check_pairs(&dictionary, modules), Ok(()));
    assert_eq!(check(&data, &dictionary), Ok(()));
}

/// What goes into a weapon's scope slot is a scope item: a table that fits
/// anything else as a scope (a weapon, an id of no item), or that does not
/// parse, stops srp at start.
#[test]
fn a_table_that_fits_no_scope_item_as_a_scope_is_refused() {
    use super::super::weapon_modules::{TABLE_PATH, fixtures::modules};
    let (ak, svd) = (dict("ak_74n"), dict("svd"));
    for (wrong, why) in [(svd, "an item of category 13"), (2999, "no item of the dictionary")] {
        let refused = check_pairs(Dictionary::shared(), &modules(&[(ak, wrong, Kind::Scope)]));
        assert!(refused.as_ref().is_err_and(|refused| refused.contains(why)), "{refused:?}");
    }
    // as another kind it is none of the scopes' business
    assert_eq!(check_pairs(Dictionary::shared(), &modules(&[(ak, 2999, Kind::Muzzle)])), Ok(()));
    let broken = game_data::shared().laid_over(
        "the test data and a broken table",
        vec![(TABLE_PATH.to_owned(), b"SRPX".to_vec())],
    );
    let refused = check(&broken, Dictionary::shared()).unwrap_err();
    assert!(refused.0.contains("does not parse"), "{refused}");
}

/// The lobby movie reads an `items_compatibility` row for each weapon and
/// the scope items it takes. srp reads those rows in one place,
/// `check_play`, and only for items in a kit's ammo slots; no scope can be
/// in a kit slot, and the start check refuses data that would let one.
#[test]
fn compatibility_rows_of_scopes_leave_srp_as_it_was() {
    use super::super::dictionary::PROFILE_SLOTS;
    use super::super::tables;
    let data = fixtures::with_scopes();
    let dictionary = fixtures::dictionary_of(&data);
    let modules = fixtures::modules();
    let rows = fixtures::compatibility_rows();
    assert_eq!(rows.len(), 5 + fixtures::STOCK_ROWS.len());
    for (weapon, scope) in rows {
        assert!(dictionary.compatible(weapon, scope) && dictionary.compatible(scope, weapon));
    }
    // No scope item goes into any kit slot.
    let items = dictionary
        .items
        .iter()
        .filter(|item| item.category == SCOPE_CATEGORY);
    for item in items {
        for slot in 0..PROFILE_SLOTS as u8 {
            assert!(
                !dictionary.fits(item.dict_id, slot),
                "{} {slot}",
                item.dict_id
            );
        }
    }
    // The seven rows weapon / default optic (69, 304, 362) make no default
    // optic a shop scope: not sold, put on no weapon (the table fits none),
    // and no slot (checked above).
    for (weapon, stock) in fixtures::STOCK_ROWS {
        let weapon = dict(weapon);
        assert!(dictionary.compatible(weapon, stock));
        assert!(!modules.is_module(stock));
        assert!(
            super::super::modifications::upgrade_slots_with(modules, weapon, stock).is_empty()
        );
        let item = dictionary.item(stock).unwrap();
        let config = game_data::config(&data, &item.cfg_name).unwrap().unwrap();
        let params = super::super::item_params::ItemParams::from_config(stock, &config).unwrap();
        assert!(
            super::super::prices::sellers_of_with(&dictionary, modules, item, &params).is_empty(),
            "{stock}"
        );
    }
    assert_eq!(tables::check(&data), Ok(()));
    // Data with a slot for scopes stops srp at start.
    let wrong = fixtures::with_scopes_and_slots(&[(SCOPE_CATEGORY, 8)]);
    let refused = tables::check(&wrong).unwrap_err();
    assert!(refused.0.contains("into profile slot 8"), "{refused}");
}

/// A weapon's own default optic is its config's, and a scope's record only
/// adds `data/srp_scope`: on the SVD (default PSO-1, 304) the bought scope
/// shows while its record is on, and without the record the client builds
/// the plain config, which has the default optic. Never a bare SVD, and no
/// default optic is a shop scope.
#[test]
fn a_bought_scope_leaves_the_weapons_default_optic_in_its_config() {
    let data = fixtures::with_scopes();
    let dictionary = fixtures::dictionary_of(&data);
    let config = |dict_id: u16| {
        let path = &dictionary.item(dict_id).unwrap().cfg_name;
        game_data::config(&data, path).unwrap().unwrap()
    };
    let default_optic = |weapon: u16| {
        let weapon = config(weapon);
        weapon.get("addons")?.get("rifle_scope_dict_id")?.as_i32()
    };
    let svd = dict("svd");
    assert_eq!(default_optic(svd), Some(304));
    // The Cobra's modification names no key of `addons`: one modifier, an
    // add of `srp_scope` = itself to `data`.
    let cobra = config(COBRA);
    let modifiers = cobra.get("modifications").unwrap().children()[0]
        .get("modifiers")
        .unwrap()
        .children();
    assert_eq!(modifiers.len(), 1);
    let path: Vec<&str> = modifiers[0]
        .get("path")
        .unwrap()
        .children()
        .into_iter()
        .filter_map(ConfigValue::as_str)
        .collect();
    assert_eq!(path, ["data"]);
    let value = modifiers[0].get("value").unwrap().children();
    assert_eq!(
        (value[0].as_str(), value[1].as_i32()),
        (Some("srp_scope"), Some(i32::from(COBRA)))
    );
    // No default optic is sold or put on anything.
    let mut found: Vec<u16> = dictionary
        .items
        .iter()
        .filter(|item| WEAPON_CATEGORIES.contains(&item.category))
        .filter_map(|item| default_optic(item.dict_id))
        .map(|id| id as u16)
        .collect();
    found.sort_unstable();
    found.dedup();
    assert_eq!(found, [69, 304, 362]);
    for stock in found {
        assert!(!fixtures::modules().is_module(stock), "{stock}");
        for (weapon, _) in pairs(&dictionary, fixtures::modules()) {
            assert!(
                super::super::modifications::upgrade_slots_with(fixtures::modules(), weapon, stock)
                    .is_empty(),
                "{stock} on {weapon}"
            );
        }
    }
}
