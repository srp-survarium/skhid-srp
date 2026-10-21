//! Weapon scopes a player buys once, owns and puts on any number of
//! weapons (`docs/notes/weapon_scopes.md`).
//!
//! **How the client shows a scope** (0.26g exe, PROVEN, the note's §2-3):
//! a weapon takes its scope from its own config, `addons.rifle_scope_dict_id`
//! (the weapon cook, 0x24A920). An item's upgrade record
//! (`{u32 id, u32 dict_id, u64, u8 slot}`) makes the client build the item
//! from a modified config: for each record it applies the
//! `parameters.modification` of the item whose dict id is **in the record**
//! (read at 0x2F15A2), not of the inventory item the record names. So one
//! owned scope item can show a different scope on each weapon: the record
//! holds a *variant*, a category-22 entry made for one weapon (its own
//! models placed for that weapon's mount and eye line), whose modification
//! sets (`type` 0, a weapon with a stock scope) or adds (`type` 3, one
//! without) `addons.rifle_scope_dict_id` = the variant itself. Without the
//! record the weapon is built from its plain config: a stock scope comes
//! back. A scope is view-only (no shooting, dispersion or hit code reads
//! it), so nothing here changes what a match computes.
//!
//! **Where the pairs come from**: the game data, as everything about items
//! (`AGENTS.md`). A variant's config names its shop scope and the weapons
//! it is made for, in a block of its own that the client does not read:
//!
//! ```text
//! srp_scope_fit {
//!     scope: <dict id or name of the scope item sold in the shop>
//!     weapons: [<dict id or name of a weapon>, ...]
//! }
//! ```
//!
//! [`ScopeFits::derive`] reads every category-22 item's config, keeps each
//! variant that the client can use and that names a scope item and weapons
//! of the dictionary, and says why it left any other out (the log at
//! start). Game data with no variant has no pair: no scope goes on any
//! weapon and none is sold, whatever the server's code.

use binary_config_parser::tree::ConfigValue;

use super::dictionary::Dictionary;
use super::game_data::{self, GameData, GameDataError};

/// The category of scopes (`categories_dict`): the three of 0.26g, the
/// shop's scope items and the variants. It has no profile slot.
pub const SCOPE_CATEGORY: u8 = 22;
/// The weapons' categories: rifles (8), sniper rifles (13), shotguns (14),
/// submachine guns (15), machine guns (16) and pistols (17).
pub const WEAPON_CATEGORIES: [u8; 6] = [8, 13, 14, 15, 16, 17];
/// The upgrade slot of a scope on a weapon: the armour camouflage's slot 3
/// ([`super::modifications::CAMO_SLOT`]), free on a weapon, whose camouflage
/// goes into 2 ([`super::modifications::WEAPON_CAMO_SLOT`]). The client sends
/// the slot of its window's drop (`relocate_upgrade_item`, 0x29A8B0) and
/// checks none.
pub const SCOPE_SLOT: u8 = 3;
/// The block of a variant's config that names its scope item and weapons.
pub const FIT_KEY: &str = "srp_scope_fit";

/// One pair: the variant a weapon shows a scope item by.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Fit {
    /// The weapon's dict id.
    pub weapon: u16,
    /// The scope item's dict id: what the player buys and owns.
    pub scope: u16,
    /// The variant's dict id: what an upgrade record of the scope on the
    /// weapon holds.
    pub variant: u16,
}

/// Every pair of the game data, by weapon and scope.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ScopeFits {
    fits: Vec<Fit>,
}

impl ScopeFits {
    /// The pairs given, each `(weapon, scope)` once: of two, the one with
    /// the lower variant id is kept, and the other is returned as left out.
    pub fn new(mut fits: Vec<Fit>) -> (Self, Vec<Fit>) {
        fits.sort_unstable_by_key(|fit| (fit.weapon, fit.scope, fit.variant));
        let mut kept: Vec<Fit> = Vec::with_capacity(fits.len());
        let mut twice = Vec::new();
        for fit in fits {
            match kept.last() {
                Some(last) if (last.weapon, last.scope) == (fit.weapon, fit.scope) => {
                    twice.push(fit);
                }
                _ => kept.push(fit),
            }
        }
        (Self { fits: kept }, twice)
    }

    /// The players' game data's ([`game_data::shared`]), read once; what
    /// was left out of it goes to the log.
    pub fn shared() -> &'static Self {
        static SHARED: std::sync::OnceLock<ScopeFits> = std::sync::OnceLock::new();
        SHARED.get_or_init(|| {
            let (fits, left_out) = Self::derive(game_data::shared(), Dictionary::shared())
                .expect("the scope configs read");
            for why in &left_out {
                log::warn!("weapon scopes: {why}");
            }
            log::info!(
                "weapon scopes: {} pair(s) of {} scope item(s) on {} weapon(s){}",
                fits.fits.len(),
                fits.scopes().len(),
                fits.weapons().len(),
                if fits.is_empty() {
                    " (the game data has no variant: no scope is sold or put on)"
                } else {
                    ""
                }
            );
            fits
        })
    }

    /// The pairs of `data`, whose dictionary is `dictionary`, and why each
    /// variant (or pair) left out was. `Err` only for a category-22 item
    /// whose config is missing or does not parse.
    pub fn derive(
        data: &GameData,
        dictionary: &Dictionary,
    ) -> Result<(Self, Vec<String>), GameDataError> {
        let mut configs = Vec::new();
        for item in &dictionary.items {
            if item.category != SCOPE_CATEGORY {
                continue;
            }
            let path = item.cfg_name.as_str();
            let config = game_data::config(data, path)?.ok_or_else(|| {
                GameDataError(format!(
                    "item {} `{}` has no {path}",
                    item.dict_id, item.name
                ))
            })?;
            configs.push((item.dict_id, config));
        }
        let weapon_config = |dict_id: u16| {
            let item = dictionary.item(dict_id)?;
            game_data::config(data, &item.cfg_name).ok().flatten()
        };
        Ok(Self::from_configs(dictionary, &configs, &weapon_config))
    }

    /// The pairs of the category-22 configs given as `(dict id, config)`,
    /// with each named weapon's config from `weapon_config`, and why
    /// anything was left out.
    pub fn from_configs(
        dictionary: &Dictionary,
        configs: &[(u16, ConfigValue)],
        weapon_config: &dyn Fn(u16) -> Option<ConfigValue>,
    ) -> (Self, Vec<String>) {
        let variants: Vec<u16> = configs
            .iter()
            .filter(|(_, config)| config.get(FIT_KEY).is_some())
            .map(|&(dict_id, _)| dict_id)
            .collect();
        // What no shop scope may be: a variant, or a scope a weapon's own
        // config names (0.26g's Leupold 69, PSO-1 304, 1P69 362: the stock
        // scope comes back when a bought one is taken off, so it is never
        // sold or put on itself).
        let mut not_scopes = variants.clone();
        if !variants.is_empty() {
            for item in &dictionary.items {
                if !WEAPON_CATEGORIES.contains(&item.category) {
                    continue;
                }
                let stock = weapon_config(item.dict_id).and_then(|config| {
                    let addons = config.get("addons")?;
                    addons.get("rifle_scope_dict_id")?.as_i32()
                });
                if let Some(stock) = stock.and_then(|id| u16::try_from(id).ok()) {
                    not_scopes.push(stock);
                }
            }
        }
        let mut fits = Vec::new();
        let mut left_out = Vec::new();
        for (dict_id, config) in configs {
            let Some(block) = config.get(FIT_KEY) else {
                continue;
            };
            let name = dictionary
                .item(*dict_id)
                .map_or("?", |item| item.name.as_str());
            match variant_fits(
                dictionary,
                &not_scopes,
                weapon_config,
                *dict_id,
                config,
                block,
            ) {
                Ok(found) => fits.extend(found),
                Err(why) => left_out.push(format!("variant {dict_id} `{name}` left out: {why}")),
            }
        }
        let (fits, twice) = Self::new(fits);
        for fit in twice {
            left_out.push(format!(
                "variant {} left out for weapon {}: scope {} has variant {} on it already",
                fit.variant,
                fit.weapon,
                fit.scope,
                fits.variant(fit.weapon, fit.scope).unwrap_or(0)
            ));
        }
        (fits, left_out)
    }

    /// No pair at all.
    pub fn is_empty(&self) -> bool {
        self.fits.is_empty()
    }

    /// Every pair, by weapon and scope.
    pub fn all(&self) -> &[Fit] {
        &self.fits
    }

    /// The variant that shows `scope` on `weapon`, if it fits.
    pub fn variant(&self, weapon: u16, scope: u16) -> Option<u16> {
        self.fits
            .binary_search_by_key(&(weapon, scope), |fit| (fit.weapon, fit.scope))
            .ok()
            .map(|index| self.fits[index].variant)
    }

    /// Whether `variant` is made for `weapon`.
    pub fn fits(&self, weapon: u16, variant: u16) -> bool {
        self.fits
            .iter()
            .any(|fit| fit.weapon == weapon && fit.variant == variant)
    }

    /// Whether `dict_id` is a scope item of a pair: one the shop sells.
    pub fn is_scope(&self, dict_id: u16) -> bool {
        self.fits.iter().any(|fit| fit.scope == dict_id)
    }

    /// Whether `dict_id` is a variant of a pair: never sold, dropped or
    /// owned, only held by upgrade records.
    pub fn is_variant(&self, dict_id: u16) -> bool {
        self.fits.iter().any(|fit| fit.variant == dict_id)
    }

    /// The scope items, ascending.
    pub fn scopes(&self) -> Vec<u16> {
        let mut scopes: Vec<u16> = self.fits.iter().map(|fit| fit.scope).collect();
        scopes.sort_unstable();
        scopes.dedup();
        scopes
    }

    /// The weapons a scope goes on, ascending.
    pub fn weapons(&self) -> Vec<u16> {
        let mut weapons: Vec<u16> = self.fits.iter().map(|fit| fit.weapon).collect();
        weapons.dedup();
        weapons
    }
}

/// A dict id named by number or by the item's name.
fn named_item(dictionary: &Dictionary, value: &ConfigValue) -> Result<u16, String> {
    let item = match value {
        ConfigValue::String(name) => dictionary.item_named(name),
        value => value
            .as_i32()
            .and_then(|id| u16::try_from(id).ok())
            .and_then(|id| dictionary.item(id)),
    };
    item.map(|item| item.dict_id)
        .ok_or_else(|| format!("{value:?} is no item of the dictionary"))
}

/// How a variant's modification puts its scope on a weapon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Puts {
    /// `type` 0 on `[addons, rifle_scope_dict_id]`: sets it, for a weapon
    /// with a stock scope (the stock one comes back without the record).
    Set,
    /// `type` 3 on `[addons]` with `[rifle_scope_dict_id, <variant>]`: adds
    /// it, for a weapon with no scope.
    Add,
}

fn texts(value: &ConfigValue) -> Vec<&str> {
    value
        .children()
        .into_iter()
        .filter_map(ConfigValue::as_str)
        .collect()
}

/// Which of [`Puts`] the modification row `row` makes with `variant`, if
/// any of its modifiers puts the variant itself into the weapon's scope.
fn puts(row: &ConfigValue, variant: u16) -> Option<Puts> {
    let modifiers = row.get("modifiers")?.children();
    modifiers.into_iter().find_map(|modifier| {
        let path = texts(modifier.get("path")?);
        let value = modifier.get("value")?;
        match (modifier.get("type")?.as_i32()?, path.as_slice()) {
            (0, ["addons", "rifle_scope_dict_id"]) => {
                (value.as_i32() == Some(variant.into())).then_some(Puts::Set)
            }
            (3, ["addons"]) => {
                let pair = value.children();
                let named = pair.first().and_then(|name| name.as_str());
                let id = pair.get(1).and_then(|id| id.as_i32());
                (pair.len() == 2
                    && named == Some("rifle_scope_dict_id")
                    && id == Some(variant.into()))
                .then_some(Puts::Add)
            }
            _ => None,
        }
    })
}

/// Why `weapon` (its config) can not take a variant that `puts` so, if it
/// can not (note §2, PROVEN in local games): a set needs the weapon's own
/// `rifle_scope_dict_id` (on a missing node it is skipped: no scope); an
/// add needs an `addons` table with a child (on an empty one nothing is
/// added) and no `rifle_scope_dict_id` in it (the client asserts on a
/// duplicate name, 0x3581C0).
fn refuses(weapon: &ConfigValue, puts: Puts) -> Option<&'static str> {
    let addons = weapon.get("addons");
    let stock = addons.and_then(|addons| addons.get("rifle_scope_dict_id"));
    match puts {
        Puts::Set if stock.is_none() => Some("it sets a scope the weapon does not have"),
        Puts::Add if stock.is_some() => {
            Some("it adds a scope to a weapon that has one (the client asserts)")
        }
        Puts::Add if addons.is_none_or(|addons| addons.entries().is_empty()) => {
            Some("it adds a scope to a weapon with no `addons` child (nothing is added)")
        }
        _ => None,
    }
}

/// The pairs one variant's `block` gives, or why it gives none. The
/// variant must be one the client can use: its `parameters.modification`
/// is a modification of its own config (the client looks the record's
/// modification up in the table it collects from every item's
/// `modifications`, 0x2F1750) that puts the variant itself into each
/// weapon's scope as that weapon can take it ([`refuses`]): attaching
/// shows the variant in place of a stock scope, and taking it off gives
/// the stock scope back.
fn variant_fits(
    dictionary: &Dictionary,
    not_scopes: &[u16],
    weapon_config: &dyn Fn(u16) -> Option<ConfigValue>,
    variant: u16,
    config: &ConfigValue,
    block: &ConfigValue,
) -> Result<Vec<Fit>, String> {
    let modification = config
        .get("parameters")
        .and_then(|parameters| parameters.get("modification"))
        .and_then(ConfigValue::as_i32)
        .filter(|&id| id != 0)
        .ok_or("it has no parameters.modification")?;
    let row = config.get("modifications").and_then(|rows| {
        rows.children()
            .into_iter()
            .find(|row| row.get("id").and_then(ConfigValue::as_i32) == Some(modification))
    });
    let Some(row) = row else {
        return Err(format!(
            "its modification {modification} is not in its own `modifications`"
        ));
    };
    let puts = puts(row, variant)
        .ok_or("its modification does not set or add `addons.rifle_scope_dict_id` = the variant")?;
    let scope = named_item(dictionary, block.get("scope").ok_or("`scope` is missing")?)?;
    let scope_item = dictionary.item(scope).expect("found by named_item");
    if scope_item.category != SCOPE_CATEGORY {
        return Err(format!(
            "its scope {scope} is no scope (category {})",
            scope_item.category
        ));
    }
    if scope_item.is_premium || not_scopes.contains(&scope) {
        return Err(format!(
            "its scope {scope} is a premium item, a variant or a weapon's stock scope"
        ));
    }
    let weapons = block.get("weapons").ok_or("`weapons` is missing")?;
    let named: Vec<&ConfigValue> = match weapons {
        ConfigValue::Indexed(list) => list.iter().collect(),
        single => vec![single],
    };
    if named.is_empty() {
        return Err("`weapons` is empty".into());
    }
    named
        .into_iter()
        .map(|value| {
            let weapon = named_item(dictionary, value)?;
            let category = dictionary.item(weapon).expect("found").category;
            if !WEAPON_CATEGORIES.contains(&category) {
                return Err(format!("{weapon} is no weapon (category {category})"));
            }
            let config = weapon_config(weapon).ok_or(format!("weapon {weapon} has no config"))?;
            if let Some(why) = refuses(&config, puts) {
                return Err(format!("on weapon {weapon}: {why}"));
            }
            Ok(Fit {
                weapon,
                scope,
                variant,
            })
        })
        .collect()
}

#[cfg(any(test, feature = "fixtures"))]
pub mod fixtures;

#[cfg(test)]
mod tests;
