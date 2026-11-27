//! Golden items: the gold-bought copies of plain items, the golden weapons
//! (`docs/notes/golden_weapons.md`) and the premium armour
//! (`docs/notes/premium_armour.md`). The dictionary says which they are:
//! a copy names its plain item (`like`) and the modifications of its own a
//! new one comes with (`start`), `scripts/content-edit/items.tsv`.
//!
//! The owner's rules (2026-10-21, 2026-10-22): a copy is upgraded as its
//! plain item. Its bonuses are its own first modifications, never defaults
//! of its config: a default is never offered for a reroll, and the lobby
//! would show it twice. A copy's config is its plain item's (its level and
//! defaults) with base rarity 0, since the lobby shows rarity 3 as rank 3,
//! which no craft takes. srp refuses data whose copies are not so
//! ([`check`]). In a craft any copy of a plain item is a donor for any
//! other ([`may_donate`]).

use binary_config_parser::tree::ConfigValue;

use super::dictionary::{self, Dictionary, Item};
use super::game_data::{GameData, GameDataError};
use super::modifications::{self, MAX_RANK};
use super::scopes::WEAPON_CATEGORIES;

/// Modification 1230: `parameters/money_bonus` +0.1, the golden weapons'
/// `start`.
pub const SILVER_BONUS: u32 = 1230;

/// Modification 1240: `parameters/experience_bonus` +0.2 (0.05 in 0.26g;
/// the owner, 2026-11-08), first of the premium armour's `start`.
pub const EXPERIENCE_BONUS: u32 = 1240;

/// Modification 1250: `parameters/reputation_bonus` +0.5 (0.05 in 0.26g;
/// the owner, 2026-11-08) with the Scavengers (`scav_rep_booster`), on
/// the Survivor pieces.
pub const SCAVENGER_REPUTATION_BONUS: u32 = 1250;

/// Modification 1260: `parameters/reputation_bonus` +0.5 with the Black
/// Market (`bm_rep_booster`), on the Bandit pieces.
pub const BLACK_MARKET_REPUTATION_BONUS: u32 = 1260;

fn is_weapon(item: &Item) -> bool {
    WEAPON_CATEGORIES.contains(&item.category)
}

fn golden_named(name: &str) -> Option<&'static Item> {
    Dictionary::shared()
        .item_named(name)
        .filter(|item| item.like.is_some())
}

fn golden(dict_id: u16) -> Option<&'static Item> {
    Dictionary::shared()
        .item(dict_id)
        .filter(|item| item.like.is_some())
}

/// Whether `name` (a dictionary name) is a golden weapon.
pub fn is_golden_weapon_name(name: &str) -> bool {
    golden_named(name).is_some_and(is_weapon)
}

/// What [`super::tables::check`] asks before any table reads them: the
/// dictionary has its copies (data made without `items.tsv` has none, and
/// would sell its golden items as plain ones), and each item's `start` is
/// modifications that exist, each once, that fit its places.
pub fn check_content(data: &GameData, dictionary: &Dictionary) -> Result<(), GameDataError> {
    if !dictionary.items.iter().any(|item| item.like.is_some()) {
        return Err(GameDataError(format!(
            "{}: no item is a copy of another (srp_like): the data was made without \
             scripts/content-edit/items.tsv; build it with every patch (srp-game-data build)",
            dictionary::PATH
        )));
    }
    let mods = super::game_data::config(data, modifications::MODIFICATIONS_PATH)?
        .ok_or_else(|| GameDataError(format!("no {}", modifications::MODIFICATIONS_PATH)))?;
    let known: Vec<i32> = mods
        .get("modifications")
        .into_iter()
        .flat_map(ConfigValue::children)
        .filter_map(|row| row.get("id").and_then(ConfigValue::as_i32))
        .collect();
    for item in &dictionary.items {
        let start = &item.start;
        let once = start
            .iter()
            .enumerate()
            .all(|(i, id)| !start[..i].contains(id));
        let exist = start
            .iter()
            .all(|&id| i32::try_from(id).is_ok_and(|id| known.contains(&id)));
        if !once || !exist || start.len() > MAX_RANK {
            return Err(GameDataError(format!(
                "{}: item {} `{}`: srp_start {start:?} is not up to {MAX_RANK} modifications of \
                 {}, each once",
                dictionary::PATH,
                item.dict_id,
                item.name,
                modifications::MODIFICATIONS_PATH
            )));
        }
    }
    Ok(())
}

/// The premium bonuses of the premium armour piece `name` (a dictionary
/// name): its `start`; `None` for anything else.
pub fn armour_bonuses(name: &str) -> Option<&'static [u32]> {
    golden_named(name)
        .filter(|item| !is_weapon(item))
        .map(|item| item.start.as_slice())
}

/// Whether `name` (a dictionary name) is a golden item.
pub fn is_golden_name(name: &str) -> bool {
    golden_named(name).is_some()
}

/// Whether `dict_id` is a golden item.
pub fn is_golden(dict_id: u16) -> bool {
    golden(dict_id).is_some()
}

/// Whether `dict_id` is a golden weapon.
pub fn is_golden_weapon(dict_id: u16) -> bool {
    golden(dict_id).is_some_and(is_weapon)
}

/// Whether `dict_id` is a premium armour piece.
pub fn is_premium_armour(dict_id: u16) -> bool {
    golden(dict_id).is_some_and(|item| !is_weapon(item))
}

/// The plain item's name a golden item's `name` is a copy of:
/// `akmn_premium_2` -> `akmn`, `neostead` -> `neostead_2`,
/// `bm_torso_4_premium` -> `bm_torso_4`. `None` for any other name.
pub fn plain_name(name: &str) -> Option<&'static str> {
    let plain = golden_named(name)?.like?;
    Dictionary::shared()
        .item(plain)
        .map(|plain| plain.name.as_str())
}

/// The item `dict_id` counts as in a craft: a golden item's plain item;
/// anything else itself.
pub fn kin(dict_id: u16) -> u16 {
    golden(dict_id)
        .and_then(|item| item.like)
        .unwrap_or(dict_id)
}

/// Whether an item of `donor` may be used up to upgrade one of `item`:
/// another of the same item, or (the owner's rules, 2026-10-21 and
/// 2026-10-22; the premium armour too) any item of the same plain item:
/// a golden item's plain item, a golden item of the same plain item
/// (`akmn_premium` and `akmn_premium_2`), and a golden item for a plain one
/// (the owner: "if I press upgrade on lancaster, I should get inside
/// premium lancasters too"; the golden one is used up). The lobby's craft
/// window lists the same (`ItemFactory.isCraftDonor`).
pub fn may_donate(donor: u16, item: u16) -> bool {
    donor == item || kin(donor) == kin(item)
}

/// The modifications of its own a new item of `dict_id` has: its `start`.
pub fn starting_modifications(dict_id: u16) -> Vec<(u32, bool)> {
    Dictionary::shared()
        .item(dict_id)
        .map(|item| item.start.iter().map(|&id| (id, false)).collect())
        .unwrap_or_default()
}

/// The patch that takes a golden weapon's defaults off.
pub const WEAPONS_PATCH: &str = "scripts/content-edit/weapon-stats.tsv";

/// The patch that takes a premium armour piece's premium bonuses off.
pub const ARMOUR_PATCH: &str = "scripts/content-edit/armor.tsv";

/// An item config's default modifications, item level and base rarity.
fn own(data: &GameData, item: &Item) -> Result<(Vec<u32>, ConfigValue, i32), GameDataError> {
    let config = super::game_data::config(data, &item.cfg_name)?
        .ok_or_else(|| GameDataError(format!("{}: no {}", item.name, item.cfg_name)))?;
    let defaults = config
        .get("default_modifications")
        .into_iter()
        .flat_map(ConfigValue::children)
        .filter_map(|id| id.as_i32().and_then(|id| u32::try_from(id).ok()))
        .collect();
    let parameters = config.get("parameters");
    let level = parameters
        .and_then(|p| p.get("item_level"))
        .cloned()
        .unwrap_or(ConfigValue::Integer(0));
    let rarity = parameters
        .and_then(|p| p.get("base_rarity_level"))
        .and_then(ConfigValue::as_i32)
        .unwrap_or(0);
    Ok((defaults, level, rarity))
}

/// The items of `data` made without their patch, each as
/// `(<name> (<config>), the patch)`: a copy whose config is not its plain
/// item's (its defaults and level) with base rarity 0, or an item with one
/// of its `start` as a default.
pub fn unpatched(
    data: &GameData,
    dictionary: &Dictionary,
) -> Result<Vec<(String, &'static str)>, GameDataError> {
    let mut found = Vec::new();
    for item in &dictionary.items {
        if item.like.is_none() && item.start.is_empty() {
            continue;
        }
        let (defaults, level, rarity) = own(data, item)?;
        let copy = match item.like.and_then(|plain| dictionary.item(plain)) {
            Some(plain) => {
                let (plain_defaults, plain_level, _) = own(data, plain)?;
                defaults == plain_defaults && level == plain_level && rarity == 0
            }
            None => true,
        };
        if !copy || item.start.iter().any(|id| defaults.contains(id)) {
            let patch = if is_weapon(item) {
                WEAPONS_PATCH
            } else {
                ARMOUR_PATCH
            };
            found.push((format!("{} ({})", item.name, item.cfg_name), patch));
        }
    }
    Ok(found)
}

/// Refuses `data` whose golden items are not as their patches make them
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
    let mut problems = Vec::new();
    for patch in [WEAPONS_PATCH, ARMOUR_PATCH] {
        let items: Vec<&str> = found
            .iter()
            .filter(|(_, of)| *of == patch)
            .map(|(item, _)| item.as_str())
            .collect();
        if !items.is_empty() {
            problems.push(format!(
                "golden items that are not their plain item's copy with base rarity 0, or have \
                 their bonuses as defaults: {}. The data was made without the patch {patch}: srp \
                 gives them their bonuses as their own modifications, and the players' lobby \
                 would show them twice and rank 3",
                items.join(", ")
            ));
        }
    }
    Err(GameDataError(format!(
        "{}: {}; build the data with every patch (srp-game-data build)",
        data.origin,
        problems.join("; ")
    )))
}

#[cfg(test)]
mod tests;
