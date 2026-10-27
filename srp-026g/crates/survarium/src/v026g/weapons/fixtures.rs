//! Item data made up for tests: the game data with one item's config
//! changed as a balance patch changes it. The test of the bots' weapon
//! numbers lays a patched weapon over the test data ([`patched_item`]),
//! derives the weapon table from the copy ([`super::derive`]) and checks that
//! each number a patch can change reaches the number the bots use; the
//! process's own data ([`super::all`]) is never touched.

// The values a test sets (the crates that use this have no config parser of their own).
pub use binary_config_parser::tree::ConfigValue;

use super::super::dictionary::Dictionary;
use super::super::game_data::GameData;

/// Sets the entry at `path` (`["parameters", "reload_time"]`) of `config` to
/// `value`. The entry must be there: a test that names a key the config does
/// not have is wrong, not patching.
pub fn set(config: &mut ConfigValue, path: &[&str], value: ConfigValue) {
    let (last, parents) = path.split_last().expect("a path names a key");
    let mut table = config;
    for key in parents {
        table = entry(table, key);
    }
    *entry(table, last) = value;
}

fn entry<'a>(table: &'a mut ConfigValue, key: &str) -> &'a mut ConfigValue {
    let ConfigValue::Named(entries) = table else {
        panic!("`{key}`: its parent is not a named table");
    };
    entries
        .iter_mut()
        .find(|(name, _)| name == key)
        .map(|(_, value)| value)
        .unwrap_or_else(|| panic!("the config has no `{key}`"))
}

/// A copy of `data` in which the config of the item `item` has each of
/// `edits` applied (a path and the value to put there).
pub fn patched_item(
    data: &GameData,
    dictionary: &Dictionary,
    item: &str,
    edits: &[(&[&str], ConfigValue)],
) -> GameData {
    let row = dictionary
        .item_named(item)
        .unwrap_or_else(|| panic!("{item} is in the dictionary"));
    let bytes = data
        .file(&row.cfg_name)
        .unwrap_or_else(|| panic!("{item}: the data has {}", row.cfg_name));
    let mut config = ConfigValue::parse(bytes).expect("the item's config reads");
    for (path, value) in edits {
        set(&mut config, path, value.clone());
    }
    data.laid_over(
        &format!("{item} patched"),
        vec![(
            row.cfg_name.clone(),
            config.to_bytes().expect("the patched config writes"),
        )],
    )
}
