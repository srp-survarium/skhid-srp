//! The 0.26g static dictionary, read from the game's own file
//! (`gameplay/db_static_dictionaries` of the players' `resources.db`,
//! [`super::game_data`]; `docs/notes/data.md` §2).
//!
//! Against 0.21d: an item has `base_cost` where it had `item_level` (the
//! level moved to the item's own config, [`super::item_params`]); the
//! boosters, perks and skills lists are gone; bundles, maps, the matchmaker's
//! settings and the reward multipliers by match level are new.
//!
//! `is_stack` decides how an item is written on the wire (`lobby.md` §2.6),
//! and the client finds it in this same file, so the server must take it
//! from here and nowhere else.

use binary_config_parser::tree::{ConfigError, ConfigValue};

pub use crate::dictionary::{Category, Faction, FactionLevel, PROFILE_SLOTS};

/// Its path in the archive.
pub const PATH: &str = "gameplay/db_static_dictionaries";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub dict_id: u16,
    pub name: String,
    pub category: u8,
    pub is_stack: bool,
    pub is_premium: bool,
    /// The item's price ([`super::prices`]).
    pub base_cost: u32,
    /// The item's own config, e.g. `gameplay/items/weapons/wpn_spas_12.options`.
    pub cfg_name: String,
    /// The plain item a golden or premium copy is (`srp_like`, by its name):
    /// another item of its category that is no copy itself.
    pub like: Option<u16>,
    /// The modifications of its own a new item comes with (`srp_start`).
    pub start: Vec<u32>,
}

/// A row of `shop_bundles_dict`. The bundle's config has only its name,
/// description and icon; what it holds and costs is the server's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bundle {
    pub id: u8,
    pub cfg: String,
}

/// A row of `maps_dict`: the id a match names its map and mode by, and the
/// file under `gameplay/game_modes/` it stands for ([`super::maps`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Map {
    pub id: u8,
    pub cfg_mode_name: String,
}

/// A row of `match_maker_defines.items_levels_modifiers`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ItemLevelModifier {
    pub modifier_value: u32,
    pub item_level_min: u32,
    pub item_level_max: u32,
}

/// A row of `match_maker_defines.elo_modifiers`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EloModifier {
    pub modifier_value: u32,
    pub elo_min: u32,
    pub elo_max: u32,
}

/// `match_maker_defines`: what the original matchmaker weighed a player by.
#[derive(Clone, Debug, PartialEq)]
pub struct MatchMakerDefines {
    pub items_levels_modifiers: Vec<ItemLevelModifier>,
    pub elo_modifiers: Vec<EloModifier>,
    /// By profile slot.
    pub slots_weight: Vec<f32>,
    pub wait_modifier: f32,
}

/// A row of `match_levels_dict`: the reward multipliers of a match level
/// (the row's index).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MatchLevel {
    pub money_k: f32,
    pub experience_k: f32,
    pub reputation_k: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Dictionary {
    /// By `dict_id`.
    pub items: Vec<Item>,
    /// `(first_item, second_item)`: e.g. an ammo and a weapon that fires it.
    /// Only pairs of items that exist: the file also names ids 79, 101, 735
    /// and 738..741, which no item has.
    pub compatibility: Vec<(u16, u16)>,
    /// The pairs left out of `compatibility`.
    pub dangling_compatibility: Vec<(u16, u16)>,
    /// By `id`.
    pub categories: Vec<Category>,
    /// `(category_id, profile_slot_id)`: the categories each slot accepts.
    pub slot_restrictions: Vec<(u8, u8)>,
    /// By `id`.
    pub factions: Vec<Faction>,
    /// By `id`.
    pub bundles: Vec<Bundle>,
    /// By `id`.
    pub maps: Vec<Map>,
    pub match_maker: MatchMakerDefines,
    /// By match level.
    pub match_levels: Vec<MatchLevel>,
}

fn int<T: TryFrom<i32>>(value: &ConfigValue, key: &str) -> Result<T, ConfigError> {
    let number = value.i32_at(key)?;
    T::try_from(number).map_err(|_| ConfigError(format!("`{key}` = {number} is out of range")))
}

fn flag(value: &ConfigValue, key: &str) -> Result<bool, ConfigError> {
    value
        .req(key)?
        .as_bool()
        .ok_or_else(|| ConfigError(format!("`{key}` is not a boolean")))
}

/// The rows of a section, each read by `row`.
fn rows<T>(
    root: &ConfigValue,
    key: &str,
    row: impl Fn(&ConfigValue) -> Result<T, ConfigError>,
) -> Result<Vec<T>, ConfigError> {
    root.req(key)?.children().into_iter().map(row).collect()
}

impl Dictionary {
    /// The players' dictionary ([`super::game_data`]), parsed once.
    pub fn shared() -> &'static Self {
        static SHARED: std::sync::OnceLock<Dictionary> = std::sync::OnceLock::new();
        SHARED.get_or_init(|| {
            Self::parse(super::game_data::file(PATH)).expect("the dictionary parses")
        })
    }

    pub fn parse(dictionaries: &[u8]) -> Result<Self, ConfigError> {
        let root = ConfigValue::parse(dictionaries)?;

        let mut entries = rows(&root, "items_dict", |item| {
            let like = item.get("srp_like").map(|name| {
                name.as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| ConfigError("`srp_like` is not a name".into()))
            });
            let start = item.get("srp_start").map_or(Ok(Vec::new()), |ids| {
                let ConfigValue::Indexed(ids) = ids else {
                    return Err(ConfigError("`srp_start` is not a list of ids".into()));
                };
                ids.iter()
                    .map(|id| {
                        id.as_i32()
                            .and_then(|id| u32::try_from(id).ok())
                            .ok_or_else(|| ConfigError("`srp_start` holds no id".into()))
                    })
                    .collect()
            })?;
            let entry = Item {
                dict_id: int(item, "dict_id")?,
                name: item.str_at("name")?.to_owned(),
                category: int(item, "item_category")?,
                is_stack: flag(item, "is_stack")?,
                is_premium: flag(item, "is_premium")?,
                base_cost: int(item, "base_cost")?,
                cfg_name: item.str_at("cfg_name")?.to_owned(),
                like: None,
                start,
            };
            Ok((entry, like.transpose()?))
        })?;
        entries.sort_by_key(|(item, _)| item.dict_id);
        let plain = |item: &Item, name: &str| {
            let refuse = |why: &str| {
                ConfigError(format!(
                    "item {} `{}`: srp_like `{name}` {why}",
                    item.dict_id, item.name
                ))
            };
            let mut named = entries.iter().filter(|(other, _)| other.name == name);
            match (named.next(), named.next()) {
                (Some((plain, None)), None)
                    if plain.dict_id != item.dict_id && plain.category == item.category =>
                {
                    Ok(plain.dict_id)
                }
                (Some(_), None) => Err(refuse("is not another plain item of its category")),
                (None, _) => Err(refuse("is no item")),
                _ => Err(refuse("names two items")),
            }
        };
        let likes = entries
            .iter()
            .map(|(item, like)| like.as_deref().map(|name| plain(item, name)).transpose())
            .collect::<Result<Vec<_>, _>>()?;
        let items: Vec<Item> = entries
            .into_iter()
            .zip(likes)
            .map(|((item, _), like)| Item { like, ..item })
            .collect();

        let pairs = rows(&root, "items_compatibility", |pair| {
            Ok((
                int::<u16>(pair, "first_item")?,
                int::<u16>(pair, "second_item")?,
            ))
        })?;
        let known = |id: u16| items.binary_search_by_key(&id, |item| item.dict_id).is_ok();
        let (compatibility, dangling_compatibility) = pairs
            .into_iter()
            .partition(|&(first, second)| known(first) && known(second));

        let mut categories = rows(&root, "categories_dict", |category| {
            Ok(Category {
                id: int(category, "id")?,
                minimum_count: int(category, "minimum_count")?,
                optimal_count: int(category, "optimal_count")?,
            })
        })?;
        categories.sort_by_key(|category| category.id);

        let slot_restrictions = rows(&root, "slots_restrictions", |rule| {
            Ok((int(rule, "category_id")?, int(rule, "profile_slot_id")?))
        })?;

        let mut factions = rows(&root, "factions_dict", |faction| {
            Ok(Faction {
                id: int(faction, "id")?,
                name: faction.str_at("name")?.to_owned(),
                levels: rows(faction, "levels", |level| {
                    Ok(FactionLevel {
                        value: int(level, "value")?,
                        name: level.str_at("name")?.to_owned(),
                        unlock_message: level.str_at("unlock_message")?.to_owned(),
                    })
                })?,
            })
        })?;
        factions.sort_by_key(|faction| faction.id);

        let mut bundles = rows(&root, "shop_bundles_dict", |bundle| {
            Ok(Bundle {
                id: int(bundle, "id")?,
                cfg: bundle.str_at("cfg")?.to_owned(),
            })
        })?;
        bundles.sort_by_key(|bundle| bundle.id);

        let mut maps = rows(&root, "maps_dict", |map| {
            Ok(Map {
                id: int(map, "id")?,
                cfg_mode_name: map.str_at("cfg_mode_name")?.to_owned(),
            })
        })?;
        maps.sort_by_key(|map| map.id);

        let defines = root.req("match_maker_defines")?;
        let match_maker = MatchMakerDefines {
            items_levels_modifiers: rows(defines, "items_levels_modifiers", |row| {
                Ok(ItemLevelModifier {
                    modifier_value: int(row, "modifier_value")?,
                    item_level_min: int(row, "item_level_min")?,
                    item_level_max: int(row, "item_level_max")?,
                })
            })?,
            elo_modifiers: rows(defines, "elo_modifiers", |row| {
                Ok(EloModifier {
                    modifier_value: int(row, "modifier_value")?,
                    elo_min: int(row, "elo_min")?,
                    elo_max: int(row, "elo_max")?,
                })
            })?,
            slots_weight: rows(defines, "slots_weight", |weight| {
                weight
                    .as_f32()
                    .ok_or_else(|| ConfigError("a slot weight is not a number".into()))
            })?,
            wait_modifier: defines.f32_at("wait_modifier")?,
        };

        let match_levels = rows(&root, "match_levels_dict", |level| {
            Ok(MatchLevel {
                money_k: level.f32_at("money_k")?,
                experience_k: level.f32_at("experience_k")?,
                reputation_k: level.f32_at("reputation_k")?,
            })
        })?;

        Ok(Self {
            items,
            compatibility,
            dangling_compatibility,
            categories,
            slot_restrictions,
            factions,
            bundles,
            maps,
            match_maker,
            match_levels,
        })
    }

    pub fn item(&self, dict_id: u16) -> Option<&Item> {
        self.items
            .binary_search_by_key(&dict_id, |item| item.dict_id)
            .ok()
            .map(|index| &self.items[index])
    }

    pub fn item_named(&self, name: &str) -> Option<&Item> {
        self.items.iter().find(|item| item.name == name)
    }

    /// How the wire writes an item of `dict_id`: as a stack (a count) or not
    /// (a condition and the repair and reroll times). An unknown id reads as
    /// a stack, as the client's reader takes it (`lobby.md` §2.6); the
    /// client crashes on such an item later, so never send one.
    pub fn is_stack(&self, dict_id: u16) -> bool {
        self.item(dict_id).is_none_or(|item| item.is_stack)
    }

    /// Whether an item may go into a profile slot.
    pub fn fits(&self, dict_id: u16, profile_slot_id: u8) -> bool {
        self.item(dict_id).is_some_and(|item| {
            self.slot_restrictions
                .contains(&(item.category, profile_slot_id))
        })
    }

    /// Whether the file pairs the two items, in either order.
    pub fn compatible(&self, a: u16, b: u16) -> bool {
        self.compatibility.contains(&(a, b)) || self.compatibility.contains(&(b, a))
    }

    /// The reward multipliers of a match level; past the table, its last row.
    pub fn match_level(&self, level: usize) -> Option<&MatchLevel> {
        self.match_levels
            .get(level)
            .or_else(|| self.match_levels.last())
    }

    /// A faction's reputation level for `points`: the highest level whose
    /// threshold the points reach.
    pub fn reputation_level(&self, faction: u8, points: u32) -> u8 {
        self.factions
            .iter()
            .find(|entry| entry.id == faction)
            .and_then(|entry| entry.levels.iter().rposition(|level| points >= level.value))
            .unwrap_or(0) as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_026g_dictionary() {
        let dictionary = Dictionary::shared();
        // 0.26g's 384, and the Bandit balaclava (1810) of the test patch 15
        // (`resources/game-026g/`).
        assert_eq!(dictionary.items.len(), 385);
        assert_eq!(dictionary.categories.len(), 29);
        assert_eq!(dictionary.slot_restrictions.len(), 77);
        assert_eq!(dictionary.factions.len(), 6);
        assert_eq!(dictionary.bundles.len(), 14);
        // 0.26g's 89 and Blackout's 24 (`game_data::test_archive`).
        assert_eq!(dictionary.maps.len(), 89 + 24);
        assert_eq!(dictionary.match_levels.len(), 21);
        assert_eq!(dictionary.items.last().unwrap().dict_id, 1810);
        assert_eq!(dictionary.items[383].dict_id, 737);

        let ak = dictionary.item(3).unwrap();
        assert_eq!(ak.name, "ak_74m");
        assert_eq!((ak.category, ak.base_cost), (8, 47500));
        assert!(!ak.is_stack && !ak.is_premium);
        assert_eq!(ak.cfg_name, "gameplay/items/weapons/wpn_ak_74m.options");
        let aks = dictionary.item_named("ak_74u").unwrap();
        assert_eq!((aks.dict_id, aks.category, aks.base_cost), (13, 8, 12920));
        assert_eq!(
            dictionary.item_named("scav_oxygen_1").unwrap().base_cost,
            2380
        );
        // 0.21d's id 73 is gone.
        assert!(dictionary.item(73).is_none());
    }

    #[test]
    fn items_per_category_are_those_of_the_dump() {
        let dictionary = Dictionary::shared();
        // 0.26g's, and the test patch's balaclava (a mask, 2).
        #[rustfmt::skip]
        let expected = [
            (1, 14), (2, 9), (3, 24), (4, 10), (5, 23), (6, 14), (7, 17), (8, 15), (9, 9),
            (10, 10), (11, 1), (12, 3), (13, 10), (14, 8), (15, 10), (16, 2), (17, 5), (18, 8),
            (19, 2), (20, 9), (21, 1), (22, 3), (23, 0), (24, 2), (25, 50), (26, 120), (27, 4),
            (28, 1), (29, 1),
        ];
        for (category, count) in expected {
            let found = dictionary
                .items
                .iter()
                .filter(|item| item.category == category)
                .count();
            assert_eq!(found, count, "category {category}");
            assert!(
                dictionary
                    .categories
                    .iter()
                    .any(|entry| entry.id == category)
            );
        }
        let premium = dictionary.items.iter().filter(|item| item.is_premium);
        assert_eq!(premium.count(), 123);
        // The weapon categories' counts changed against 0.21d.
        let category = |id| dictionary.categories.iter().find(|c| c.id == id).unwrap();
        assert_eq!(
            (category(8).minimum_count, category(8).optimal_count),
            (10, 60)
        );
        assert_eq!(
            (category(16).minimum_count, category(16).optimal_count),
            (30, 100)
        );
    }

    #[test]
    fn is_stack_is_the_wire_layout_switch() {
        let dictionary = Dictionary::shared();
        assert!(dictionary.is_stack(7)); // 5.45x39
        assert!(dictionary.is_stack(5)); // RGD-5
        assert!(!dictionary.is_stack(3)); // AK-74M
        // The artefacts became stacks in 0.26g.
        for id in [54, 74, 76] {
            assert!(dictionary.is_stack(id), "{id}");
        }
        // Money and vouchers are stacks; cosmetics are not.
        assert!(dictionary.is_stack(214) && dictionary.is_stack(215));
        let of = |category: u8| {
            dictionary
                .items
                .iter()
                .filter(move |item| item.category == category)
        };
        assert!(of(27).all(|item| item.is_stack));
        assert!(of(25).chain(of(26)).all(|item| !item.is_stack));
        // An unknown id reads as a stack, as the client's reader does.
        assert!(dictionary.is_stack(9999));
    }

    #[test]
    fn slots_and_compatibility() {
        let dictionary = Dictionary::shared();
        // 23 profile slots, each taking something.
        let slots = &dictionary.slot_restrictions;
        assert!((0..PROFILE_SLOTS as u8).all(|slot| slots.iter().any(|&(_, s)| s == slot)));
        assert!(slots.iter().all(|&(_, s)| usize::from(s) < PROFILE_SLOTS));
        assert!(dictionary.fits(3, 7) && dictionary.fits(3, 10));
        assert!(!dictionary.fits(3, 0));
        assert!(dictionary.fits(7, 8)); // ammo in an ammo slot
        // Money goes in no slot.
        assert!((0..PROFILE_SLOTS as u8).all(|slot| !dictionary.fits(214, slot)));

        // 94 pairs in the file; those naming an id no item has are set aside.
        assert_eq!(
            dictionary.compatibility.len() + dictionary.dangling_compatibility.len(),
            94
        );
        let mut dangling: Vec<u16> = dictionary
            .dangling_compatibility
            .iter()
            .flat_map(|&(a, b)| [a, b])
            .filter(|&id| dictionary.item(id).is_none())
            .collect();
        dangling.sort();
        dangling.dedup();
        assert_eq!(dangling, [79, 101, 735, 738, 739, 740, 741]);
        // 5.45x39 fits the AKS-74U; the 7N10 fits the AK-74M.
        assert!(dictionary.compatible(7, 13));
        assert!(dictionary.compatible(3, 217));
        assert!(!dictionary.compatible(3, 22));
    }

    #[test]
    fn factions_bundles_maps_and_matchmaking() {
        let dictionary = Dictionary::shared();
        let scavengers = &dictionary.factions[0];
        assert_eq!(
            (scavengers.id, scavengers.name.as_str()),
            (1, "st_scavengers_faction")
        );
        let thresholds: Vec<u32> = scavengers.levels.iter().map(|l| l.value).collect();
        assert_eq!(thresholds, [0, 100, 420, 990, 1840, 3000]);
        assert_eq!(dictionary.factions[2].levels.len(), 11);
        assert_eq!(dictionary.factions[2].levels[10].value, 35250);
        assert_eq!(dictionary.reputation_level(1, 0), 0);
        assert_eq!(dictionary.reputation_level(1, 100), 1);
        assert_eq!(dictionary.reputation_level(1, 2999), 4);
        assert_eq!(dictionary.reputation_level(1, 1_000_000), 5);

        assert_eq!(
            dictionary.bundles[0].cfg,
            "gameplay/bundles/premium_3_day_bundle.options"
        );
        assert_eq!(dictionary.bundles[13].id, 14);

        let map = |id| dictionary.maps.iter().find(|map| map.id == id).unwrap();
        assert_eq!(map(61).cfg_mode_name, "level_01_day_2.options");
        assert_eq!(map(109).cfg_mode_name, "level_08_prerain_2.options");
        assert_eq!(map(140).cfg_mode_name, "level_08_prerain_4.options");

        let defines = &dictionary.match_maker;
        assert_eq!(defines.items_levels_modifiers.len(), 21);
        assert_eq!(defines.items_levels_modifiers[20].modifier_value, 20);
        assert_eq!(defines.elo_modifiers.len(), 6);
        let first = EloModifier {
            modifier_value: 0,
            elo_min: 100,
            elo_max: 10000,
        };
        assert_eq!(defines.elo_modifiers[0], first);
        assert_eq!(defines.slots_weight, [1.0; PROFILE_SLOTS]);
        assert_eq!(defines.wait_modifier, 1.0);

        let first = MatchLevel {
            money_k: 1.0,
            experience_k: 1.0,
            reputation_k: 1.0,
        };
        assert_eq!(dictionary.match_levels[0], first);
        assert_eq!(dictionary.match_levels[1].money_k, 1.47);
        assert_eq!(dictionary.match_level(20).unwrap().money_k, 13.59);
        assert_eq!(dictionary.match_level(99).unwrap().reputation_k, 1.95);
    }

    #[test]
    fn starting_modifications_require_an_indexed_list_of_ids() {
        let parse = |start| {
            let mut root = ConfigValue::parse(super::super::game_data::file(PATH)).unwrap();
            let ConfigValue::Named(sections) = &mut root else {
                panic!("a table")
            };
            let (_, ConfigValue::Indexed(items)) = sections
                .iter_mut()
                .find(|(name, _)| name == "items_dict")
                .unwrap()
            else {
                panic!("items")
            };
            let Some(ConfigValue::Named(keys)) = items
                .iter_mut()
                .find(|item| item.i32_at("dict_id") == Ok(3))
            else {
                panic!("AK-74M")
            };
            keys.push(("srp_start".into(), start));
            Dictionary::parse(&root.to_bytes().unwrap()).map(|d| d.item(3).unwrap().start.clone())
        };
        assert_eq!(
            parse(ConfigValue::Indexed(vec![ConfigValue::Integer(1230)])),
            Ok(vec![1230])
        );
        assert_eq!(parse(ConfigValue::Indexed(vec![])), Ok(vec![]));
        for value in [
            ConfigValue::Integer(1230),
            ConfigValue::String("1230".into()),
            ConfigValue::Named(vec![]),
            ConfigValue::Indexed(vec![ConfigValue::Integer(-1)]),
        ] {
            assert!(parse(value).is_err());
        }
    }

    #[test]
    fn a_copy_names_another_plain_item_of_its_category() {
        // The AK-74M (3, a rifle) said to be a copy of `like`.
        let copy_of = |like: &str| {
            let mut root = ConfigValue::parse(super::super::game_data::file(PATH)).unwrap();
            let ConfigValue::Named(sections) = &mut root else {
                panic!("a named table")
            };
            let (_, items) = sections
                .iter_mut()
                .find(|(n, _)| n == "items_dict")
                .unwrap();
            let ConfigValue::Indexed(items) = items else {
                panic!("a list")
            };
            let entry = items.iter_mut().find(|e| e.i32_at("dict_id") == Ok(3));
            let Some(ConfigValue::Named(keys)) = entry else {
                panic!("the AK-74M")
            };
            keys.push(("srp_like".into(), ConfigValue::String(like.into())));
            Dictionary::parse(&root.to_bytes().unwrap()).map(|d| d.item(3).unwrap().like)
        };
        assert_eq!(copy_of("ak_74n"), Ok(Some(306)));
        for (like, why) in [
            ("ak_74m", "is not another plain item of its category"),
            ("scav_torso_4", "is not another plain item of its category"),
            (
                "ak_74n_premium",
                "is not another plain item of its category",
            ),
            ("no_such_item", "is no item"),
        ] {
            let error = copy_of(like).unwrap_err().0;
            assert!(error.contains(why), "{like}: {error}");
        }
        assert_eq!(Dictionary::shared().item(338).unwrap().like, Some(306));
        assert_eq!(Dictionary::shared().item(3).unwrap().like, None);
    }
}
