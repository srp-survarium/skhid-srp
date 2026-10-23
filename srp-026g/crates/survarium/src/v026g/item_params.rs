//! What srp needs from each 0.26g item's own config
//! (`gameplay/items/**.options` of the players' `resources.db`,
//! [`super::game_data`]; `docs/notes/data.md` §4): its level and
//! matchmaking level, faction, weight, rarity, wear and repair values, for
//! ammo its clip, and the profile slots it covers when worn. Derived from
//! the configs when first used ([`all`]); an item without a field has 0.
//!
//! Against 0.21d: `drop_value` and `clips_in_bag` are gone (ammo has
//! `required_bags`), and `item_level` came here from the dictionary.

use binary_config_parser::tree::{ConfigError, ConfigValue};

use super::dictionary::Dictionary;
use super::game_data::{GameData, GameDataError};

/// One item's parameters.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ItemParams {
    pub dict_id: u16,
    /// `type`: `wpn_aslt`, `arm_tors`, `ammo`, `drugs`, `trap`, `grenade`,
    /// `booster`, `drp_camo`, `vch_fixr`, ...; empty for artefacts, scopes
    /// and money.
    pub kind: &'static str,
    /// 0 none, 1..=4 the lobby factions.
    pub faction: u8,
    pub faction_affinity: u32,
    /// 0..=16.
    pub item_level: u8,
    pub match_making_level: u8,
    /// Cosmetics only.
    pub base_rarity_level: u8,
    /// `rarity` of an item that is no stack: its rarity levels (weapons and
    /// armour `[0, 1, 2, 3]`, cosmetics `[0]`).
    pub gear_rarity: &'static [u8],
    /// `rarity` of a stack: `(least, most)` units, one pair but for money
    /// (five, [`super::drops`]).
    pub stack_rarity: &'static [(u32, u32)],
    /// Kilograms; ammo has `clip_weight` instead.
    pub weight: f32,
    pub drop_weight: u32,
    /// Condition lost a use, a match and a death, as a fraction of 1.
    pub degrade_per_use: f32,
    pub degrade_per_match: f32,
    pub degrade_per_death: f32,
    pub reliability: f32,
    /// The share of the item's cost a full repair costs.
    pub repair_cost_ratio: f32,
    /// Ammo: the rounds of a clip and its weight, and the ammo bags it takes.
    pub clip_size: u32,
    pub clip_weight: f32,
    pub required_bags: u32,
    /// Weapons: the rounds a magazine holds.
    pub magazine_capacity: u32,
    pub double_handed: bool,
    pub fills_whole_pocket: bool,
    /// Cosmetics: the modification that puts it on an item.
    pub modification: u32,
    /// The modifications the item always has.
    pub default_modifications: &'static [u32],
    /// The profile slots it covers when worn (`data.additional_model_slots`,
    /// [`model_slot`]): a kit that wears it has nothing in them. A helmet
    /// with a built-in mask covers the mask's; the Duty exoskeleton, worn in
    /// the back slot, all six others.
    pub covered_slots: &'static [u8],
}

/// What `gameplay/players/default.player` says of what a player carries.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerLimits {
    /// `player.stamina_params.max_carried_weight`.
    pub max_carried_weight: f32,
    /// `player.stamina_params.min_carried_weight`.
    pub min_carried_weight: f32,
    /// `player.stamina_params.max_carried_weight_movement_speed_modifier`:
    /// the movement speed lost at the capacity (`bot_skills`).
    pub max_carried_weight_movement_speed_modifier: f32,
    /// `player.stamina_params.max_carried_weight_spending_speed_modifier`:
    /// the stamina spending added at the capacity.
    pub max_carried_weight_spending_speed_modifier: f32,
    /// `player.ammo_bags_count`.
    pub ammo_bags_count: u32,
    /// `player.pocket_slots_count`.
    pub pocket_slots_count: u32,
    /// `player.pocket_slot_size`.
    pub pocket_slot_size: u32,
}

/// Its path in the archive.
pub const PLAYER_PATH: &str = "gameplay/players/default.player";

impl PlayerLimits {
    pub fn from_config(config: &ConfigValue) -> Result<Self, ConfigError> {
        let player = config.req("player")?;
        let stamina = player.req("stamina_params")?;
        Ok(Self {
            max_carried_weight: float(Some(stamina), "max_carried_weight")?,
            min_carried_weight: float(Some(stamina), "min_carried_weight")?,
            max_carried_weight_movement_speed_modifier: float(
                Some(stamina),
                "max_carried_weight_movement_speed_modifier",
            )?,
            max_carried_weight_spending_speed_modifier: float(
                Some(stamina),
                "max_carried_weight_spending_speed_modifier",
            )?,
            ammo_bags_count: int(Some(player), "ammo_bags_count")?,
            pocket_slots_count: int(Some(player), "pocket_slots_count")?,
            pocket_slot_size: int(Some(player), "pocket_slot_size")?,
        })
    }
}

/// The players' limits ([`super::game_data`]), read once.
pub fn player() -> &'static PlayerLimits {
    static SHARED: std::sync::OnceLock<PlayerLimits> = std::sync::OnceLock::new();
    SHARED.get_or_init(|| {
        let config = ConfigValue::parse(super::game_data::file(PLAYER_PATH));
        config
            .and_then(|config| PlayerLimits::from_config(&config))
            .expect("the player's config reads")
    })
}

/// Every item's parameters, by `dict_id`: one row for each item of the
/// dictionary, from its config ([`derive`]).
pub fn all() -> &'static [ItemParams] {
    static SHARED: std::sync::OnceLock<Vec<ItemParams>> = std::sync::OnceLock::new();
    SHARED.get_or_init(|| {
        derive(super::game_data::shared(), Dictionary::shared()).expect("the item configs read")
    })
}

/// One row for each item of `dictionary`, by `dict_id`, from its config in
/// `data` (`cfg_name`), which each must have.
pub fn derive(data: &GameData, dictionary: &Dictionary) -> Result<Vec<ItemParams>, GameDataError> {
    let mut rows = Vec::with_capacity(dictionary.items.len());
    for item in &dictionary.items {
        let path = item.cfg_name.as_str();
        let config = super::game_data::config(data, path)?.ok_or_else(|| {
            GameDataError(format!(
                "item {} `{}` has no {path}",
                item.dict_id, item.name
            ))
        })?;
        let mut row = ItemParams::from_config(item.dict_id, &config)
            .map_err(|cause| GameDataError(format!("{path}: {}", cause.0)))?;
        // The owner's rule (`super::golden`): a golden weapon has no
        // default bonus and no base rarity, a premium armour piece only its
        // plain piece's default and no base rarity, as their patched configs
        // say (srp refuses other data at start, `golden::check`).
        if super::golden::is_golden_weapon_name(&item.name) {
            row.default_modifications = &[];
            row.base_rarity_level = 0;
        } else if let Some(bonuses) = super::golden::armour_bonuses(&item.name) {
            let plain: Vec<u32> = row
                .default_modifications
                .iter()
                .copied()
                .filter(|id| !bonuses.contains(id))
                .collect();
            row.default_modifications = leak(plain);
            row.base_rarity_level = 0;
        }
        rows.push(row);
    }
    rows.sort_by_key(|row| row.dict_id);
    if let Some(pair) = rows.windows(2).find(|w| w[0].dict_id == w[1].dict_id) {
        return Err(GameDataError(format!(
            "dict_id {} is in items_dict twice",
            pair[0].dict_id
        )));
    }
    Ok(rows)
}

/// A number of a config table, as the game reads one: an integer, a float
/// cut to one, or a boolean; a missing one 0.
fn int<T: TryFrom<i32>>(table: Option<&ConfigValue>, key: &str) -> Result<T, ConfigError> {
    let number = match table.and_then(|table| table.get(key)) {
        None => 0,
        Some(value) => value
            .as_i32()
            .ok_or_else(|| ConfigError(format!("`{key}` is not a number")))?,
    };
    T::try_from(number).map_err(|_| ConfigError(format!("`{key}` = {number} is out of range")))
}

fn float(table: Option<&ConfigValue>, key: &str) -> Result<f32, ConfigError> {
    match table.and_then(|table| table.get(key)) {
        None => Ok(0.0),
        Some(ConfigValue::Boolean(value)) => Ok(f32::from(u8::from(*value))),
        Some(value) => value
            .as_f32()
            .ok_or_else(|| ConfigError(format!("`{key}` is not a number"))),
    }
}

fn flag(table: Option<&ConfigValue>, key: &str) -> Result<bool, ConfigError> {
    match table.and_then(|table| table.get(key)) {
        None => Ok(false),
        Some(value) => value
            .as_bool()
            .ok_or_else(|| ConfigError(format!("`{key}` is not a boolean"))),
    }
}

/// A list of numbers, each in range of `T`.
fn numbers<T: TryFrom<i32>>(list: &ConfigValue, key: &str) -> Result<Vec<T>, ConfigError> {
    list.children()
        .into_iter()
        .map(|value| {
            let number = value
                .as_i32()
                .ok_or_else(|| ConfigError(format!("`{key}` holds a non-number")))?;
            T::try_from(number)
                .map_err(|_| ConfigError(format!("`{key}` holds {number}, out of range")))
        })
        .collect()
}

/// A slice that lives as long as the process: the configs are read once.
fn leak<T>(list: Vec<T>) -> &'static [T] {
    if list.is_empty() {
        &[]
    } else {
        Box::leak(list.into_boxed_slice())
    }
}

impl ItemParams {
    /// The row of an item's config (`gameplay/items/**.options`):
    /// `parameters.*` by the same names (`type` is `kind`), a missing one 0,
    /// the config's own `default_modifications`, and the slots of
    /// `data.additional_model_slots`. The strings and lists are leaked: the
    /// rows live as long as the process.
    pub fn from_config(dict_id: u16, config: &ConfigValue) -> Result<Self, ConfigError> {
        let p = config.get("parameters");
        let kind = match p.and_then(|p| p.get("type")) {
            None => "",
            Some(value) => {
                let text = value
                    .as_str()
                    .ok_or_else(|| ConfigError("`parameters.type` is not a string".into()))?;
                Box::leak(text.to_owned().into_boxed_str())
            }
        };
        // A stack's `rarity` is pairs of counts, anything else's levels.
        let rarity = p.and_then(|p| p.get("rarity"));
        let rows = rarity.map(ConfigValue::children).unwrap_or_default();
        let (gear_rarity, stack_rarity) = match rows.first() {
            Some(ConfigValue::Indexed(_) | ConfigValue::Named(_)) => {
                let pairs = rows
                    .iter()
                    .map(|row| match numbers::<u32>(row, "parameters.rarity")?[..] {
                        [least, most] => Ok((least, most)),
                        _ => Err(ConfigError(
                            "a `parameters.rarity` row is not a pair".into(),
                        )),
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                (Vec::new(), pairs)
            }
            Some(_) => (
                numbers(rarity.expect("rows"), "parameters.rarity")?,
                Vec::new(),
            ),
            None => (Vec::new(), Vec::new()),
        };
        let default_modifications = match config.get("default_modifications") {
            Some(list) => numbers(list, "default_modifications")?,
            None => Vec::new(),
        };
        Ok(Self {
            dict_id,
            kind,
            faction: int(p, "faction")?,
            faction_affinity: int(p, "faction_affinity")?,
            item_level: int(p, "item_level")?,
            match_making_level: int(p, "match_making_level")?,
            base_rarity_level: int(p, "base_rarity_level")?,
            gear_rarity: leak(gear_rarity),
            stack_rarity: leak(stack_rarity),
            weight: float(p, "weight")?,
            drop_weight: int(p, "drop_weight")?,
            degrade_per_use: float(p, "degrade_per_use")?,
            degrade_per_match: float(p, "degrade_per_match")?,
            degrade_per_death: float(p, "degrade_per_death")?,
            reliability: float(p, "reliability")?,
            repair_cost_ratio: float(p, "repair_cost_ratio")?,
            clip_size: int(p, "clip_size")?,
            clip_weight: float(p, "clip_weight")?,
            required_bags: int(p, "required_bags")?,
            magazine_capacity: int(p, "magazine_capacity")?,
            double_handed: flag(p, "double_handed")?,
            fills_whole_pocket: flag(p, "fills_whole_pocket")?,
            modification: int(p, "modification")?,
            default_modifications: leak(default_modifications),
            covered_slots: leak(covered_slots_of(config)?),
        })
    }
}

/// The profile slot `data.additional_model_slots` names: the slots whose
/// models an item's own models replace. Profile slots 0..=6 are helmet,
/// mask, torso, back, legs, gloves and boots ([`super::loadouts`]).
pub fn model_slot(name: &str) -> Option<u8> {
    Some(match name {
        "helmet" => 0,
        "mask" => 1,
        "torso" => 2,
        "back" => 3,
        "pants" | "legs" => 4,
        "gloves" => 5,
        "boots" => 6,
        _ => return None,
    })
}

/// The profile slots a config covers (`data.additional_model_slots`,
/// [`model_slot`]), in its order, each once.
pub fn covered_slots_of(config: &ConfigValue) -> Result<Vec<u8>, ConfigError> {
    let Some(list) = config
        .get("data")
        .and_then(|data| data.get("additional_model_slots"))
    else {
        return Ok(Vec::new());
    };
    let mut slots = Vec::new();
    for value in list.children() {
        let name = value.as_str().ok_or_else(|| {
            ConfigError("`data.additional_model_slots` holds a non-string".into())
        })?;
        let slot = model_slot(name).ok_or_else(|| {
            ConfigError(format!(
                "`data.additional_model_slots` names `{name}`, no profile slot"
            ))
        })?;
        if !slots.contains(&slot) {
            slots.push(slot);
        }
    }
    Ok(slots)
}

/// The profile slots `dict_id` covers when worn.
pub fn covered_slots(dict_id: u16) -> &'static [u8] {
    item_params(dict_id).map_or(&[], |params| params.covered_slots)
}

/// The parameters of `dict_id`, if it is an item.
pub fn item_params(dict_id: u16) -> Option<&'static ItemParams> {
    let all = all();
    all.binary_search_by_key(&dict_id, |item| item.dict_id)
        .ok()
        .map(|index| &all[index])
}

/// The units of `dict_id` one step of the shop's slider buys or sells: a
/// clip of ammo (`clip_size`), else one item. The 0.26g lobby shows a price
/// for such a step: `MessageAsk.onSliderUpdate` is
/// `slider / step * price` with `step = clip_size` (PROVEN,
/// `lobby_menu.swf`).
pub fn shop_unit(dict_id: u16) -> u32 {
    item_params(dict_id)
        .map(|params| params.clip_size)
        .filter(|&size| size > 0)
        .unwrap_or(1)
}

/// What `count` of `dict_id` weighs in a kit, as the lobby counts it:
/// ammo `count / clip_size * weight`, any other stack `weight * count`
/// (PROVEN, `lobby_menu.swf` `AmmoShareControll.as` 211, 217). That ammo's
/// `weight` there is the config's `clip_weight` is LIKELY: ammo configs have
/// no `weight`, and the exe's item descriptor fill reads `clip_weight` beside
/// `clip_size` (0x286BA1). An item that is no stack weighs its `weight`
/// whatever its condition: pass 1.
pub fn carried_weight(dict_id: u16, count: u32) -> f32 {
    let Some(params) = item_params(dict_id) else {
        return 0.0;
    };
    if params.clip_size > 0 {
        count as f32 / params.clip_size as f32 * params.clip_weight
    } else {
        params.weight * count as f32
    }
}

#[cfg(test)]
mod tests {
    use super::super::dictionary::Dictionary;
    use super::*;

    #[test]
    fn every_dictionary_item_has_parameters() {
        let dictionary = Dictionary::shared();
        // 0.26g's 384, and the Bandit balaclava of the test patch 15.
        assert_eq!(all().len(), 385);
        assert!(all().windows(2).all(|w| w[0].dict_id < w[1].dict_id));
        for item in &dictionary.items {
            let params = item_params(item.dict_id).unwrap();
            // A stack has counts for its rarity, anything else levels.
            if item.is_stack {
                assert!(params.gear_rarity.is_empty(), "{}", item.name);
            } else {
                assert!(params.stack_rarity.is_empty(), "{}", item.name);
            }
            assert!(params.item_level <= 16, "{}", item.name);
            assert!(params.faction <= 4, "{}", item.name);
            assert!(params.default_modifications.len() <= 8, "{}", item.name);
        }
        assert!(item_params(73).is_none());
    }

    #[test]
    fn the_carry_limit_is_the_players_config() {
        let player = player();
        assert_eq!(player.max_carried_weight, 30.0);
        assert_eq!(player.min_carried_weight, 20.0);
        assert_eq!(player.max_carried_weight_movement_speed_modifier, -0.2);
        assert_eq!(player.max_carried_weight_spending_speed_modifier, 1.5);
        assert_eq!(player.ammo_bags_count, 3);
        assert_eq!((player.pocket_slots_count, player.pocket_slot_size), (1, 1));
    }

    #[test]
    fn spot_checks_against_the_dump() {
        // item_params_026g.tsv.
        let ak = item_params(3).unwrap();
        assert_eq!(ak.kind, "wpn_aslt");
        assert_eq!((ak.faction, ak.faction_affinity), (3, 30));
        assert_eq!((ak.item_level, ak.match_making_level), (10, 10));
        assert_eq!(ak.gear_rarity, [0, 1, 2, 3]);
        assert_eq!((ak.weight, ak.drop_weight), (3.4, 60));
        assert_eq!(
            (
                ak.degrade_per_use,
                ak.degrade_per_match,
                ak.degrade_per_death
            ),
            (0.0005, 0.01, 0.0)
        );
        assert_eq!((ak.reliability, ak.repair_cost_ratio), (1.0, 0.1));
        assert_eq!(ak.magazine_capacity, 30);
        assert!(ak.double_handed);

        let ammo = item_params(7).unwrap();
        assert_eq!(ammo.kind, "ammo");
        assert_eq!(
            (ammo.clip_size, ammo.clip_weight, ammo.required_bags),
            (30, 0.25, 1)
        );
        assert_eq!(ammo.stack_rarity, [(60, 150)]);
        assert_eq!(ammo.item_level, 5);
        // The belt-fed 7.62x39 takes two bags.
        assert_eq!(item_params(147).unwrap().required_bags, 2);

        let oxygen = item_params(9).unwrap();
        assert_eq!(
            (oxygen.kind, oxygen.faction, oxygen.weight),
            ("arm_oxy", 1, 8.0)
        );
        assert_eq!(oxygen.degrade_per_use, 0.01);

        // Weights that changed against 0.21d.
        let dictionary = Dictionary::shared();
        let weight = |name: &str| {
            item_params(dictionary.item_named(name).unwrap().dict_id)
                .unwrap()
                .weight
        };
        assert_eq!(weight("ren_helmet_1"), 1.4);
        assert_eq!(weight("scav_back_1"), 3.0);
        assert_eq!(weight("ak_74u"), 2.7);

        // Money has five rarities, [`super::super::drops`].
        assert_eq!(item_params(214).unwrap().stack_rarity.len(), 5);
        assert_eq!(item_params(215).unwrap().stack_rarity[4], (50, 50));
        // Armour comes with its modifications: 83 of 0.26g's, but for the
        // seven golden weapons, whose go (`super::super::golden`).
        assert_eq!(
            all()
                .iter()
                .filter(|params| !params.default_modifications.is_empty())
                .count(),
            83 - 7
        );
    }

    #[test]
    fn a_shop_step_is_a_clip_and_weight_goes_by_clips() {
        assert_eq!(shop_unit(7), 30);
        assert_eq!(shop_unit(22), 15);
        assert_eq!(shop_unit(3), 1);
        assert_eq!(shop_unit(67), 1); // a medkit
        assert_eq!(shop_unit(9999), 1);
        // Two clips of 5.45, 0.25 kg each.
        assert_eq!(carried_weight(7, 60), 0.5);
        // Three RGD-5, 0.3 kg each.
        assert!((carried_weight(5, 3) - 0.9).abs() < 1e-6);
        assert_eq!(carried_weight(3, 1), 3.4);
        assert_eq!(carried_weight(9999, 5), 0.0);
    }

    #[test]
    fn the_patched_items_are_read_from_their_configs() {
        // The Bandit balaclava of the test patch 15, a mask of item level 8.
        let mask = item_params(1810).unwrap();
        assert_eq!(mask.kind, "arm_mask");
        assert_eq!((mask.item_level, mask.weight), (8, 1.0));
        assert!(mask.covered_slots.is_empty());
        // 0.26g's own helmets with a built-in mask cover the mask's slot,
        // and nothing else covers anything.
        let dictionary = Dictionary::shared();
        let covering: Vec<&str> = all()
            .iter()
            .filter(|params| !params.covered_slots.is_empty())
            .map(|params| dictionary.item(params.dict_id).unwrap().name.as_str())
            .collect();
        assert_eq!(covering.len(), 4, "{covering:?}");
        for params in all() {
            assert!(params.covered_slots.iter().all(|&slot| slot == 1));
        }
        let zubr = dictionary
            .items
            .iter()
            .find(|item| item.cfg_name.ends_with("/renesanse_helmet_6.options"))
            .unwrap();
        assert_eq!(covered_slots(zubr.dict_id), [1]);
    }
}
