//! The scope pairs of game data released up to revision 68, for the
//! preservation check alone (`srp-check-scope-fits`): no server reads them.
//!
//! Until the scopes moved onto the modules table, a pair was a *variant*: a
//! category-22 entry made for one weapon (the optic's models placed for that
//! weapon's mount and eye line), whose modification set (`type` 0, a weapon
//! with a stock scope) or added (`type` 3, one without)
//! `addons.rifle_scope_dict_id` = the variant itself, and whose config named
//! its shop scope and weapons in a block the client did not read:
//!
//! ```text
//! srp_scope_fit {
//!     scope: <dict id or name of the scope item sold in the shop>
//!     weapons: [<dict id or name of a weapon>, ...]
//! }
//! ```
//!
//! An upgrade record of a scope held the variant's dict id. [`pairs`] reads
//! them as the servers of those releases did, so the check compares the new
//! data with exactly what players could put on.

use binary_config_parser::tree::ConfigValue;

use super::super::dictionary::Dictionary;
use super::super::game_data::{self, GameData, GameDataError};
use super::super::weapon_modules::{Fitted, Modules, Unfit};
use super::{SCOPE_CATEGORY, WEAPON_CATEGORIES};

/// The block of a variant's config that names its scope item and weapons.
pub const FIT_KEY: &str = "srp_scope_fit";

/// One released pair: the variant a weapon showed a scope item by.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Released {
    /// The weapon's dict id.
    pub weapon: u16,
    /// The scope item's dict id: what the player bought and owns.
    pub scope: u16,
    /// The variant's dict id: what a record of the scope on the weapon held.
    pub variant: u16,
}

/// The pairs of `data`, whose dictionary is `dictionary`, by weapon and
/// scope, and why each variant (or pair) left out was. `Err` only for a
/// category-22 item whose config is missing or does not parse.
pub fn pairs(
    data: &GameData,
    dictionary: &Dictionary,
) -> Result<(Vec<Released>, Vec<String>), GameDataError> {
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
    Ok(from_configs(dictionary, &configs, &weapon_config))
}

/// What a candidate game data's servers answer about scopes: its
/// dictionary and its modules table.
pub struct Candidate<'a> {
    pub dictionary: &'a Dictionary,
    pub modules: &'a Modules,
}

impl Candidate<'_> {
    /// Why the released pair `weapon` and `scope` (each its dict id and its
    /// name in the released data) can no longer be put on, if it can not:
    /// an item gone or under another name, a scope that is no scope item
    /// the shop sells any more, or the refusal of the one fit check
    /// ([`Modules::check`], what an attach and an account's load run).
    pub fn lost(
        &self,
        (weapon, weapon_name): (u16, &str),
        (scope, scope_name): (u16, &str),
    ) -> Option<String> {
        for (id, name, what) in [(weapon, weapon_name, "weapon"), (scope, scope_name, "scope")] {
            match self.dictionary.item(id) {
                None => return Some(format!("{what} {id} `{name}` is gone")),
                Some(item) if item.name != name => {
                    return Some(format!("{what} {id} `{name}` is now `{}`", item.name));
                }
                Some(_) => {}
            }
        }
        // (`super::super::prices::sellers_of_with`'s rule for a module.)
        let sold = self.dictionary.item(scope).is_some_and(|item| {
            item.category == SCOPE_CATEGORY && !item.is_premium && item.base_cost != 0
        });
        if !sold || !self.modules.is_module(scope) {
            return Some(format!("scope {scope} `{scope_name}` is not sold"));
        }
        let fitted = Fitted {
            scope: Some(scope),
            ..Fitted::default()
        };
        match self.modules.check(weapon, fitted) {
            Ok(()) => None,
            Err(Unfit::NoGun) => Some(format!(
                "weapon {weapon} `{weapon_name}` is no gun of the modules table"
            )),
            Err(Unfit::Refused(refusal)) => Some(format!(
                "the modules table refuses scope {scope} `{scope_name}` on weapon {weapon} \
                 `{weapon_name}`: {refusal:?}"
            )),
        }
    }
}

/// The pairs of the category-22 configs given as `(dict id, config)`, with
/// each named weapon's config from `weapon_config`, and why anything was
/// left out.
fn from_configs(
    dictionary: &Dictionary,
    configs: &[(u16, ConfigValue)],
    weapon_config: &dyn Fn(u16) -> Option<ConfigValue>,
) -> (Vec<Released>, Vec<String>) {
    let variants: Vec<u16> = configs
        .iter()
        .filter(|(_, config)| config.get(FIT_KEY).is_some())
        .map(|&(dict_id, _)| dict_id)
        .collect();
    // What no shop scope was: a variant, or a scope a weapon's own config
    // names (0.26g's Leupold 69, PSO-1 304, 1P69 362).
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
    let mut found = Vec::new();
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
            Ok(fits) => found.extend(fits),
            Err(why) => left_out.push(format!("variant {dict_id} `{name}` left out: {why}")),
        }
    }
    // Each `(weapon, scope)` once: of two, the lower variant id was kept.
    found.sort_unstable();
    let mut kept: Vec<Released> = Vec::with_capacity(found.len());
    for fit in found {
        match kept.last() {
            Some(last) if (last.weapon, last.scope) == (fit.weapon, fit.scope) => {
                left_out.push(format!(
                    "variant {} left out for weapon {}: scope {} has variant {} on it already",
                    fit.variant, fit.weapon, fit.scope, last.variant
                ));
            }
            _ => kept.push(fit),
        }
    }
    (kept, left_out)
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

/// How a variant's modification put its scope on a weapon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Puts {
    /// `type` 0 on `[addons, rifle_scope_dict_id]`: for a weapon with a
    /// stock scope.
    Set,
    /// `type` 3 on `[addons]` with `[rifle_scope_dict_id, <variant>]`: for a
    /// weapon with no scope.
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

/// Why `weapon` (its config) could not take a variant that `puts` so, if it
/// could not: a set needs the weapon's own `rifle_scope_dict_id`; an add
/// needs an `addons` table with a child and no `rifle_scope_dict_id` in it
/// (the client asserts on a duplicate name, 0x3581C0).
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

/// The pairs one variant's `block` gave, or why it gave none: its
/// `parameters.modification` had to be a modification of its own config
/// that put the variant itself into each weapon's scope as that weapon
/// could take it ([`refuses`]).
fn variant_fits(
    dictionary: &Dictionary,
    not_scopes: &[u16],
    weapon_config: &dyn Fn(u16) -> Option<ConfigValue>,
    variant: u16,
    config: &ConfigValue,
    block: &ConfigValue,
) -> Result<Vec<Released>, String> {
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
            Ok(Released {
                weapon,
                scope,
                variant,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::super::weapon_modules::Kind;
    use super::super::fixtures::{self, COBRA, LONE_SCOPE, PSO_1};
    use super::*;

    fn named(entries: Vec<(&str, ConfigValue)>) -> ConfigValue {
        ConfigValue::Named(
            entries
                .into_iter()
                .map(|(name, value)| (name.to_owned(), value))
                .collect(),
        )
    }

    fn text(value: &str) -> ConfigValue {
        ConfigValue::String(value.to_owned())
    }

    /// A released variant's config: its modification adds or sets
    /// `addons.rifle_scope_dict_id` = itself, and its block names `scope`
    /// and `weapons`.
    fn variant(dict_id: u16, puts: Puts, scope: ConfigValue, weapons: Vec<ConfigValue>) -> ConfigValue {
        let id = 1_000_000 + i32::from(dict_id);
        let modifier = match puts {
            Puts::Set => named(vec![
                (
                    "path",
                    ConfigValue::Indexed(vec![text("addons"), text("rifle_scope_dict_id")]),
                ),
                ("value", ConfigValue::Integer(dict_id.into())),
                ("type", ConfigValue::Integer(0)),
            ]),
            Puts::Add => named(vec![
                ("path", ConfigValue::Indexed(vec![text("addons")])),
                (
                    "value",
                    ConfigValue::Indexed(vec![
                        text("rifle_scope_dict_id"),
                        ConfigValue::Integer(dict_id.into()),
                    ]),
                ),
                ("type", ConfigValue::Integer(3)),
            ]),
        };
        named(vec![
            (
                "parameters",
                named(vec![("modification", ConfigValue::Integer(id))]),
            ),
            (
                "modifications",
                ConfigValue::Indexed(vec![named(vec![
                    ("id", ConfigValue::Integer(id)),
                    ("modifiers", ConfigValue::Indexed(vec![modifier])),
                ])]),
            ),
            (
                FIT_KEY,
                named(vec![
                    ("scope", scope),
                    ("weapons", ConfigValue::Indexed(weapons)),
                ]),
            ),
        ])
    }

    /// The reader as the released servers ran it: a pair by number or by
    /// name, one variant for several weapons, a pair given twice keeps the
    /// lower variant, and a variant the client could not use is left out
    /// with its reason.
    #[test]
    fn the_released_pairs_are_read_from_the_variants_configs() {
        let data = fixtures::with_scopes();
        let dictionary = fixtures::dictionary_of(&data);
        let dict = |name: &str| dictionary.item_named(name).unwrap().dict_id;
        let (ak, ak_premium, svd) = (dict("ak_74n"), dict("ak_74n_premium"), dict("svd"));
        let number = |id: u16| ConfigValue::Integer(id.into());
        let configs = vec![
            // made for two weapons, named by name; its scope by number
            (2001, variant(2001, Puts::Add, number(PSO_1), vec![text("ak_74n"), text("ak_74n_premium")])),
            // the same pair again, a higher id: left out
            (2050, variant(2050, Puts::Add, number(PSO_1), vec![text("ak_74n")])),
            // on the SVD, which has a stock scope: a set
            (2029, variant(2029, Puts::Set, text("shop_scope_pso_1"), vec![number(svd)])),
            // an add on the SVD would assert in the client: left out
            (2096, variant(2096, Puts::Add, number(PSO_1), vec![text("svd")])),
            // the SVD's stock PSO-1 as its shop scope: left out
            (2098, variant(2098, Puts::Add, number(304), vec![text("ak_74n")])),
            // a shop scope item: no block, no pair
            (PSO_1, named(vec![])),
        ];
        // the weapons as the released data had them: the AK with the
        // placeholder an add needs, the SVD with its stock scope
        let weapon_config = |weapon: u16| {
            let addons = if weapon == svd {
                named(vec![("rifle_scope_dict_id", ConfigValue::Integer(304))])
            } else {
                named(vec![("mount", ConfigValue::Integer(1))])
            };
            Some(named(vec![("addons", addons)]))
        };
        let (pairs, left_out) = from_configs(&dictionary, &configs, &weapon_config);
        let pair = |weapon, variant| Released { weapon, scope: PSO_1, variant };
        let mut expected = vec![pair(ak, 2001), pair(ak_premium, 2001), pair(svd, 2029)];
        expected.sort_unstable();
        assert_eq!(pairs, expected);
        assert_eq!(left_out.len(), 3, "{left_out:#?}");
        for (variant, why) in [
            (2050, "has variant 2001 on it already"),
            (2096, "the client asserts"),
            (2098, "a weapon's stock scope"),
        ] {
            assert!(
                left_out
                    .iter()
                    .any(|line| line.contains(&variant.to_string()) && line.contains(why)),
                "{variant}: {left_out:#?}"
            );
        }
        // data without a variant has no released pair
        let (pairs, left_out) = pairs_of_the_test_data();
        assert!(pairs.is_empty() && left_out.is_empty());
    }

    fn pairs_of_the_test_data() -> (Vec<Released>, Vec<String>) {
        pairs(game_data::shared(), Dictionary::shared()).unwrap()
    }

    /// The fixture's candidate (the AK-74N and its premium copy take the
    /// PSO-1 and the Cobra, the SVD the Cobra), or one with `table`.
    fn with_candidate<T>(table: Option<&Modules>, ask: impl FnOnce(&Candidate<'_>) -> T) -> T {
        let dictionary = fixtures::dictionary_of(&fixtures::with_scopes());
        ask(&Candidate {
            dictionary: &dictionary,
            modules: table.unwrap_or(fixtures::modules()),
        })
    }

    fn dict(name: &str) -> u16 {
        Dictionary::shared().item_named(name).unwrap().dict_id
    }

    const PSO_1_NAME: &str = "shop_scope_pso_1";
    const COBRA_NAME: &str = "shop_scope_cobra_ekp_8";

    #[test]
    fn a_released_pair_the_table_fits_is_preserved() {
        let (ak, svd) = (dict("ak_74n"), dict("svd"));
        with_candidate(None, |candidate| {
            assert_eq!(candidate.lost((ak, "ak_74n"), (PSO_1, PSO_1_NAME)), None);
            assert_eq!(candidate.lost((svd, "svd"), (COBRA, COBRA_NAME)), None);
        });
    }

    #[test]
    fn a_pair_the_table_does_not_fit_is_lost_whatever_else_it_gained() {
        let (ak, svd, ak_74m) = (dict("ak_74n"), dict("svd"), dict("ak_74m"));
        with_candidate(None, |candidate| {
            // the gun takes scopes, not this one
            let why = candidate.lost((svd, "svd"), (PSO_1, PSO_1_NAME)).unwrap();
            assert!(why.contains("refuses scope 2101") && why.contains("NoFit"), "{why}");
            // no gun of the table at all
            let why = candidate.lost((ak_74m, "ak_74m"), (PSO_1, PSO_1_NAME)).unwrap();
            assert!(why.contains("is no gun of the modules table"), "{why}");
            // a scope item nothing takes is not sold
            let why = candidate.lost((ak, "ak_74n"), (LONE_SCOPE, "shop_scope_lone")).unwrap();
            assert!(why.contains("is not sold"), "{why}");
        });
        // a table with more guns and pairs that drops one released pair
        let grown = super::super::super::weapon_modules::fixtures::modules(&[
            (ak, COBRA, Kind::Scope),
            (svd, COBRA, Kind::Scope),
            (svd, PSO_1, Kind::Scope),
            (ak_74m, PSO_1, Kind::Scope),
        ]);
        with_candidate(Some(&grown), |candidate| {
            assert!(candidate.lost((ak, "ak_74n"), (PSO_1, PSO_1_NAME)).is_some());
            assert_eq!(candidate.lost((ak, "ak_74n"), (COBRA, COBRA_NAME)), None);
        });
    }

    #[test]
    fn an_item_gone_or_under_another_name_loses_its_pairs() {
        let ak = dict("ak_74n");
        with_candidate(None, |candidate| {
            let why = candidate.lost((ak, "ak_74n"), (2198, "shop_scope_gone")).unwrap();
            assert!(why.contains("scope 2198 `shop_scope_gone` is gone"), "{why}");
            let why = candidate.lost((ak, "ak_47"), (PSO_1, PSO_1_NAME)).unwrap();
            assert!(why.contains("weapon") && why.contains("is now `ak_74n`"), "{why}");
        });
    }
}
