//! Every table srp derives from the game's files, built once from given
//! files and checked ([`check`]): what [`super::game_data::init`] runs
//! before the files become the process's, so that whatever is wrong with
//! them stops `srp` at start, not at a player's first use.

use binary_config_parser::tree::ConfigValue;

use super::armour;
use super::awards::Awards;
use super::dictionary::{self, Dictionary};
use super::drops::{self, Drops};
use super::game_data::{GameData, GameDataError};
use super::item_params::{self, ItemParams, PlayerLimits};
use super::loadouts::{self, Loadouts};
use super::localization;
use super::maps;
use super::modifications::{self, Modifications};
use super::quests::{self, Quests};
use super::scopes::{SCOPE_CATEGORY, ScopeFits};
use super::skills::{self, SkillsTree};
use super::ui_properties::{self, UiProperties};

/// The file at `path` of `data`, which a table needs.
fn needed<'a>(data: &'a GameData, path: &str) -> Result<&'a [u8], GameDataError> {
    data.file(path)
        .ok_or_else(|| GameDataError(format!("{}: no {path}", data.origin)))
}

/// `what` of `path`, or why not.
fn parsed<T>(
    path: &str,
    what: Result<T, binary_config_parser::tree::ConfigError>,
) -> Result<T, GameDataError> {
    what.map_err(|cause| GameDataError(format!("{path}: {}", cause.0)))
}

/// The most entries (named values and list elements, at any depth) an item
/// config may have. The client rebuilds an item's config with its upgrades
/// (camouflage, decals, scope) in a 16 KB stack arena of 24-byte items that
/// it never bounds (`create_binary_config_buffer`, exe 0x358AA0); past it the
/// items overwrite the arena's allocator and `vostok::configs::free_memory`
/// (0x358A10) calls through it. The KS-23M of weapons-069@9 had 587 and,
/// camouflaged and with decals, crashed every match at load (2026-11-01).
/// 565 is the largest stock weapon config's (the premium Remington 700's),
/// which players run with camouflage and decals.
pub const MAX_CONFIG_ENTRIES: usize = 565;

/// The entries of a config: each named value and each list element.
pub fn config_entries(value: &ConfigValue) -> usize {
    match value {
        ConfigValue::Named(entries) => entries.iter().map(|(_, v)| 1 + config_entries(v)).sum(),
        ConfigValue::Indexed(items) => items.iter().map(|v| 1 + config_entries(v)).sum(),
        _ => 0,
    }
}

/// Every item config fits the client's arena ([`MAX_CONFIG_ENTRIES`]).
fn check_config_entries(data: &GameData, dictionary: &Dictionary) -> Result<(), GameDataError> {
    for item in &dictionary.items {
        let Some(bytes) = data.file(&item.cfg_name) else {
            continue;
        };
        let entries = config_entries(&parsed(&item.cfg_name, ConfigValue::parse(bytes))?);
        if entries > MAX_CONFIG_ENTRIES {
            return Err(GameDataError(format!(
                "{}: {entries} entries, over the {MAX_CONFIG_ENTRIES} the client rebuilds an \
                 upgraded item in (exe 0x358AA0); the client would crash at match load",
                item.cfg_name
            )));
        }
    }
    Ok(())
}

/// Builds every table of `data` and checks what one file says of another:
/// each item's config is there, each default modification is one of
/// `modifications.options`, ids are once in the dictionary, every item
/// the code names is in it ([`named_items`]), and every shop camouflage has
/// a seller ([`check_shop_camos`]).
pub fn check(data: &GameData) -> Result<(), GameDataError> {
    let dictionary = parsed(
        dictionary::PATH,
        Dictionary::parse(needed(data, dictionary::PATH)?),
    )?;
    check_named(data, &dictionary)?;
    // A scope is only ever an upgrade of a weapon (`super::scopes`): in a kit
    // slot it would go to a match as an item worn, and its `items_compatibility`
    // rows with weapons would make it count as ammo (`check_play`).
    if let Some(&(_, slot)) = dictionary
        .slot_restrictions
        .iter()
        .find(|&&(category, _)| category == SCOPE_CATEGORY)
    {
        return Err(GameDataError(format!(
            "{}: slots_restrictions puts category {SCOPE_CATEGORY} (scopes) into profile slot \
             {slot}; a scope is only an upgrade of a weapon",
            dictionary::PATH
        )));
    }
    let modifications = parsed(
        modifications::MODIFICATIONS_PATH,
        Modifications::parse(
            needed(data, modifications::MODIFICATIONS_PATH)?,
            needed(data, modifications::CRAFT_PATH)?,
            needed(data, modifications::REROLL_PATH)?,
        ),
    )?;
    // Before item_params: it reads the premium pieces' bonuses.
    super::golden::check_content(data, &dictionary)?;
    let params = item_params::derive(data, &dictionary)?;
    for row in &params {
        for &id in row.default_modifications {
            if modifications.modification(id).is_none() {
                let item = dictionary
                    .item(row.dict_id)
                    .expect("a row of the dictionary");
                return Err(GameDataError(format!(
                    "{}: default modification {id} is not in {}; the client would crash on it",
                    item.cfg_name,
                    modifications::MODIFICATIONS_PATH
                )));
            }
        }
    }
    check_shop_camos(&dictionary, &params)?;
    check_config_entries(data, &dictionary)?;
    let properties = parsed(
        ui_properties::PATH,
        UiProperties::parse(needed(data, ui_properties::PATH)?),
    )?;
    if let Some(modification) = modifications.modifications.iter().find(|modification| {
        properties
            .get(modification.shown.property)
            .and_then(|property| property.name.as_ref())
            .is_none()
    }) {
        return Err(GameDataError(format!(
            "{}: modification {} shows property {}, which {} has not with a name",
            modifications::MODIFICATIONS_PATH,
            modification.id,
            modification.shown.property,
            ui_properties::PATH
        )));
    }
    modifications::derive_item_mods(data, &dictionary)?;
    armour::derive(data, &dictionary)?;
    // A variant left out is logged when the table is first used
    // ([`ScopeFits::shared`]); only a scope config that does not read stops
    // srp.
    ScopeFits::derive(data, &dictionary)?;
    maps::derive(data, &dictionary)?;
    for language in localization::Language::ALL {
        localization::of(data, language)?;
    }
    let player = needed(data, item_params::PLAYER_PATH)?;
    parsed(
        item_params::PLAYER_PATH,
        ConfigValue::parse(player).and_then(|config| PlayerLimits::from_config(&config)),
    )?;
    parsed(drops::PATH, Drops::parse(needed(data, drops::PATH)?))?;
    parsed(quests::PATH, Quests::parse(needed(data, quests::PATH)?))?;
    parsed(skills::PATH, SkillsTree::parse(needed(data, skills::PATH)?))?;
    parsed(
        super::awards::PATH,
        Awards::parse(needed(data, super::awards::PATH)?),
    )?;
    parsed(
        loadouts::PATH,
        Loadouts::parse(needed(data, loadouts::PATH)?, &dictionary),
    )?;
    if let Some(pair) = dictionary
        .items
        .windows(2)
        .find(|w| w[0].dict_id == w[1].dict_id)
    {
        return Err(GameDataError(format!(
            "{}: dict_id {} is in items_dict twice",
            dictionary::PATH,
            pair[0].dict_id
        )));
    }
    Ok(())
}

/// An item the code names rather than the game's files: where (the
/// constant), and the item's name in the dictionary (`items_dict`'s
/// `name`, as [`Dictionary::item_named`] finds it).
pub type Named = (&'static str, &'static str);

/// Every item this crate's code names: the starter kit and stash, the
/// shop's exceptions and ammo prices (but for [`super::prices::UNRELEASED`]),
/// the bundles' contents. Each is
/// looked up by name when used, and one the dictionary lacks would be
/// skipped there, or panic: [`check`] refuses data that lacks any.
pub fn named_items() -> Vec<Named> {
    let mut named: Vec<Named> = Vec::new();
    named.extend(
        loadouts::STARTER
            .iter()
            .map(|&(_, name)| ("loadouts::STARTER", name)),
    );
    named.extend(
        loadouts::STARTER_STASH
            .iter()
            .map(|&name| ("loadouts::STARTER_STASH", name)),
    );
    // Not the items of a patch not released yet: data may lack them.
    let released = |name: &&str| !super::prices::UNRELEASED.contains(name);
    named.extend(
        super::prices::EXCEPTIONS
            .iter()
            .map(|&(name, _)| name)
            .filter(released)
            .map(|name| ("prices::EXCEPTIONS", name)),
    );
    named.extend(
        super::prices::AMMO_PRICES
            .iter()
            .map(|&(name, _, _)| name)
            .filter(released)
            .map(|name| ("prices::AMMO_PRICES", name)),
    );
    named.extend(
        super::prices::OWNER_PRICES
            .iter()
            .map(|&(name, _, _)| ("prices::OWNER_PRICES", name)),
    );
    named.extend(
        super::prices::NOT_SOLD
            .iter()
            .map(|&(name, _)| ("prices::NOT_SOLD", name)),
    );
    for bundle in &super::shop_offers::BUNDLES {
        named.extend(
            bundle
                .items
                .iter()
                .map(|&(name, _)| ("shop_offers::BUNDLES", name)),
        );
    }
    named
}

/// Of `named`, those `dictionary` lacks, each as `<where> <name>`, once.
pub fn lacking(dictionary: &Dictionary, named: &[Named]) -> Vec<String> {
    let mut lacking: Vec<String> = named
        .iter()
        .filter(|(_, name)| dictionary.item_named(name).is_none())
        .map(|(place, name)| format!("{place} {name}"))
        .collect();
    lacking.dedup();
    lacking
}

/// The error for code that names what the game's data of `origin` lacks.
pub fn lacking_error(origin: &str, lacking: &[String]) -> GameDataError {
    GameDataError(format!(
        "{origin} lacks what srp's code names: {}. The data was made without a patch these \
         come from, or this srp names items of a patch not released yet: build the data with \
         every patch (srp-game-data build), or run the srp of the release",
        lacking.join(", ")
    ))
}

/// What [`check`] says of the code's items: [`named_items`] all in
/// `dictionary`, and the starter kit's items fit their slots.
fn check_named(data: &GameData, dictionary: &Dictionary) -> Result<(), GameDataError> {
    let mut missing = lacking(dictionary, &named_items());
    for (slot, name) in loadouts::STARTER {
        let item = dictionary.item_named(name);
        if item.is_some_and(|item| !dictionary.fits(item.dict_id, slot)) {
            missing.push(format!("loadouts::STARTER {name} (not of slot {slot})"));
        }
    }
    if missing.is_empty() {
        Ok(())
    } else {
        Err(lacking_error(&data.origin, &missing))
    }
}

/// Every shop camouflage of `dictionary` ([`super::prices::is_shop_camo`])
/// is in a folder a faction sells from ([`super::prices::camo_shop`]): one
/// in another folder (a patch's new one, or the Edge's) would be left out
/// of the shop unseen, so the start is refused naming it, until
/// `prices::CAMO_SHOPS` names its folder.
fn check_shop_camos(dictionary: &Dictionary, params: &[ItemParams]) -> Result<(), GameDataError> {
    let unsold: Vec<String> = dictionary
        .items
        .iter()
        .filter(|item| {
            params
                .binary_search_by_key(&item.dict_id, |row| row.dict_id)
                .is_ok_and(|index| super::prices::is_shop_camo(item, &params[index]))
                && super::prices::camo_shop(&item.cfg_name).is_none()
        })
        .map(|item| format!("{} `{}` ({})", item.dict_id, item.name, item.cfg_name))
        .collect();
    if unsold.is_empty() {
        return Ok(());
    }
    Err(GameDataError(format!(
        "shop camouflages ({}) in a folder no faction sells from: {}; name the folder in \
         prices::CAMO_SHOPS",
        super::prices::SHOP_CAMO,
        unsold.join(", ")
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_config_counts_each_named_value_and_list_element() {
        let pair = ConfigValue::Indexed(vec![
            ConfigValue::Integer(257),
            ConfigValue::String("weapons/x".into()),
        ]);
        let config = ConfigValue::Named(vec![
            (
                "parameters".into(),
                ConfigValue::Named(vec![("weight".into(), ConfigValue::Float(3.0))]),
            ),
            (
                "sounds".into(),
                ConfigValue::Indexed(vec![pair.clone(), pair]),
            ),
        ]);
        // parameters, weight, sounds, two pairs of two.
        assert_eq!(config_entries(&config), 3 + 2 + 4);
    }

    #[test]
    fn every_stock_item_config_fits_the_clients_arena() {
        let data = super::super::game_data::shared();
        let largest = Dictionary::shared()
            .items
            .iter()
            .filter_map(|item| data.file(&item.cfg_name).map(|b| (item, b)))
            .map(|(item, bytes)| {
                (
                    config_entries(&ConfigValue::parse(bytes).unwrap()),
                    &item.name,
                )
            })
            .max()
            .unwrap();
        assert!(largest.0 <= MAX_CONFIG_ENTRIES, "{largest:?}");
    }

    #[test]
    fn the_items_the_code_names_are_in_the_data() {
        let named = named_items();
        assert!(named.contains(&("loadouts::STARTER", "tt_33")));
        assert!(named.contains(&("shop_offers::BUNDLES", "vss_premium")));
        assert!(named.contains(&("prices::EXCEPTIONS", "medkit")));
        assert!(named.contains(&("prices::EXCEPTIONS", "vityaz")));
        assert!(named.contains(&("prices::OWNER_PRICES", "vityaz")));
        // The rows of a patch not released are not asked for.
        assert!(
            named
                .iter()
                .all(|(_, name)| !super::super::prices::UNRELEASED.contains(name))
        );
        assert_eq!(lacking(Dictionary::shared(), &named), Vec::<String>::new());
        assert_eq!(check(super::super::game_data::shared()), Ok(()));
    }

    /// Data without an item the code names (a patch left out of it): the
    /// start is refused, every such item named with where.
    #[test]
    fn data_without_an_item_the_code_names_is_refused_naming_it() {
        let mut dictionary = Dictionary::shared().clone();
        dictionary
            .items
            .retain(|item| !matches!(item.name.as_str(), "tt_33" | "vss_premium"));
        let missing = lacking(&dictionary, &named_items());
        assert_eq!(
            missing,
            [
                "loadouts::STARTER tt_33",
                "prices::OWNER_PRICES tt_33",
                "shop_offers::BUNDLES vss_premium"
            ]
        );
        let error = lacking_error("server-game-data.db", &missing).to_string();
        assert!(
            error.contains(
                "server-game-data.db lacks what srp's code names: loadouts::STARTER tt_33, \
                 prices::OWNER_PRICES tt_33, shop_offers::BUNDLES vss_premium."
            ),
            "{error}"
        );
        assert!(error.contains("srp-game-data build"), "{error}");
    }

    /// A shop camouflage in a folder no faction sells from refuses the
    /// start, naming it; the test data's eight are in their factions'.
    #[test]
    fn a_shop_camouflage_of_no_factions_folder_is_refused() {
        let dictionary = Dictionary::shared();
        let params = item_params::all();
        assert_eq!(check_shop_camos(dictionary, params), Ok(()));
        let mut moved = dictionary.clone();
        let denim = moved
            .items
            .iter_mut()
            .find(|item| item.name == "camo_denim")
            .unwrap();
        denim.cfg_name = "gameplay/items/upgrades/camouflages/edge/camo_denim.options".into();
        let error = check_shop_camos(&moved, params).unwrap_err().to_string();
        assert!(
            error.contains(
                "163 `camo_denim` (gameplay/items/upgrades/camouflages/edge/camo_denim.options)"
            ),
            "{error}"
        );
        assert!(error.contains("prices::CAMO_SHOPS"), "{error}");
        // A drop camouflage of that folder is no shop camouflage.
        let mut moved = dictionary.clone();
        let raven = moved
            .items
            .iter_mut()
            .find(|item| item.name == "camo_black_leather_01")
            .unwrap();
        raven.cfg_name =
            "gameplay/items/upgrades/camouflages/edge/camo_black_leather_01.options".into();
        assert_eq!(check_shop_camos(&moved, params), Ok(()));
    }
}
