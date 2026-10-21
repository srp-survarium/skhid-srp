//! Golden weapons: the gold-bought copies of weapons (`*_premium`,
//! `*_premium_2`, the Neostead), `docs/notes/golden_weapons.md`.
//!
//! The owner's rule (2026-10-21): a golden weapon keeps only its first
//! bonus, +10 % silver ([`SILVER_BONUS`]), and is upgraded like any
//! weapon. In the game's data each had it as the first of its
//! `default_modifications` (beside two weapon bonuses) and
//! `base_rarity_level` 3: the lobby showed it at rank 3, so it could not be
//! crafted, and a default is never offered for a reroll. The resources.db
//! patch of `scripts/content-edit/golden-weapons-1.json` takes every
//! default off them and sets the base rarity to 0, as the plain weapons
//! have; srp gives each golden weapon the silver bonus as a modification of
//! its own, the first, when it is made ([`starting_modifications`]) and,
//! once, to those owned before (`session::v026g::items_rules::
//! give_golden_silver`). From then on it is an ordinary modification: it
//! counts for the rank, a reroll may replace it, a failed craft may take
//! it.
//!
//! In a craft a golden weapon and its plain weapon count as one weapon
//! ([`kin`]): either may be the donor of the other.
//!
//! srp applies the rule to whatever data it reads ([`super::item_params`]
//! gives a golden weapon no defaults and base rarity 0), and refuses to
//! start on data whose golden weapons still have them ([`check`]): the
//! players' lobby would show the silver twice and rank 3.

use binary_config_parser::tree::ConfigValue;

use super::dictionary::{self, Dictionary};
use super::game_data::{GameData, GameDataError};

/// Modification 1230: `parameters/money_bonus` +0.1, the first default of
/// every golden weapon in the game's data.
pub const SILVER_BONUS: u32 = 1230;

/// The golden weapons, by their name in the dictionary: every weapon whose
/// config had [`SILVER_BONUS`] as its first default modification (revision
/// 26's data). Not the MP7 "special" (gold-bought, one default: 2004) nor
/// the VSSK camo (2054, 2074, 2094, not premium): they had no silver bonus.
pub const GOLDEN_WEAPONS: [&str; 21] = [
    "kiparis_premium",
    "uzi_premium",
    "uzi_premium_2",
    "vityaz_premium",
    "vss_premium",
    "sr3m_premium",
    "izh_27_lancaster_premium",
    "izh_27_lancaster_premium_2",
    "ak_74n_premium",
    "ak_74u_premium",
    "ak_12_premium",
    "akmn_premium",
    "akmn_premium_2",
    "sks_premium",
    "sks_premium_2",
    "vepr_premium",
    "vepr_12_premium",
    "saiga_12_premium",
    "vsk_94_premium",
    "rem_700_premium",
    "neostead",
];

/// Whether `name` (a dictionary name) is a golden weapon.
pub fn is_golden_name(name: &str) -> bool {
    GOLDEN_WEAPONS.contains(&name)
}

/// Whether `dict_id` is a golden weapon.
pub fn is_golden(dict_id: u16) -> bool {
    Dictionary::shared()
        .item(dict_id)
        .is_some_and(|item| is_golden_name(&item.name))
}

/// The plain weapon's name a golden weapon's `name` is a copy of:
/// `akmn_premium_2` -> `akmn`. `None` for any other name (the Neostead has
/// no plain weapon).
pub fn plain_name(name: &str) -> Option<&str> {
    if !is_golden_name(name) {
        return None;
    }
    let stem = name.strip_suffix("_premium").or_else(|| {
        let (stem, number) = name.rsplit_once("_premium_")?;
        number.bytes().all(|b| b.is_ascii_digit()).then_some(stem)
    })?;
    Some(stem)
}

/// The weapon `dict_id` counts as in a craft: a golden weapon's plain
/// weapon, when the dictionary has it; anything else itself.
pub fn kin(dict_id: u16) -> u16 {
    kin_in(Dictionary::shared(), dict_id)
}

fn kin_in(dictionary: &Dictionary, dict_id: u16) -> u16 {
    dictionary
        .item(dict_id)
        .and_then(|item| plain_name(&item.name))
        .and_then(|plain| dictionary.item_named(plain))
        .map_or(dict_id, |plain| plain.dict_id)
}

/// The modifications of its own a new item of `dict_id` has: the silver
/// bonus for a golden weapon, none for anything else.
pub fn starting_modifications(dict_id: u16) -> Vec<(u32, bool)> {
    if is_golden(dict_id) {
        vec![(SILVER_BONUS, false)]
    } else {
        Vec::new()
    }
}

/// The golden weapons of `data` whose config still has default
/// modifications or a base rarity, each as `<name> (<config>)`: data
/// made without the golden-weapons patch.
pub fn unpatched(data: &GameData, dictionary: &Dictionary) -> Result<Vec<String>, GameDataError> {
    let mut found = Vec::new();
    for item in dictionary
        .items
        .iter()
        .filter(|item| is_golden_name(&item.name))
    {
        let config = super::game_data::config(data, &item.cfg_name)?
            .ok_or_else(|| GameDataError(format!("{}: no {}", item.name, item.cfg_name)))?;
        let defaults = config
            .get("default_modifications")
            .is_some_and(|list| !list.children().is_empty());
        let rarity = config
            .get("parameters")
            .and_then(|p| p.get("base_rarity_level"))
            .and_then(ConfigValue::as_i32)
            .unwrap_or(0);
        if defaults || rarity != 0 {
            found.push(format!("{} ({})", item.name, item.cfg_name));
        }
    }
    Ok(found)
}

/// Refuses `data` whose golden weapons still have their defaults
/// ([`unpatched`]).
pub fn check(data: &GameData) -> Result<(), GameDataError> {
    let dictionary = Dictionary::parse(
        data.file(dictionary::PATH)
            .ok_or_else(|| GameDataError(format!("{}: no {}", data.origin, dictionary::PATH)))?,
    )
    .map_err(|cause| GameDataError(format!("{}: {}", dictionary::PATH, cause.0)))?;
    let found = unpatched(data, &dictionary)?;
    if found.is_empty() {
        return Ok(());
    }
    Err(GameDataError(format!(
        "{}: golden weapons with default modifications or a base rarity: {}. The data was made \
         without the golden-weapons patch (scripts/content-edit/golden-weapons-1.json): srp gives \
         them the silver bonus as their own modification, and the players' lobby would show it \
         twice and rank 3; build the data with every patch (srp-game-data build)",
        data.origin,
        found.join(", ")
    )))
}

#[cfg(test)]
mod tests;
