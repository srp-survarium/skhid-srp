//! What an armour piece's config says that changes play, for srp's site (the
//! wiki's equipment; docs/notes/armour-params.md): each body part's `armor`
//! per hit type (`hit_params`), its model (`data.base_model`), and an oxygen
//! tank's seconds and gas filters (`data.amount_time_sec`, `influences`).
//! Its `parameters.*_modifier` are 0 in every config (its bonuses are its
//! modifications) and `parameters.armor` & co. are never read. Read from each item
//! of the dictionary's armour categories; [`super::tables::check`] reads them
//! all at start, so a config srp cannot read stops it there.

use std::sync::OnceLock;

use binary_config_parser::tree::{ConfigError, ConfigValue};

use super::dictionary::Dictionary;
use super::game_data::{GameData, GameDataError};

/// `items_dict` categories of armour: helmet, mask, torso, back, legs,
/// gloves, boots.
pub const CATEGORIES: std::ops::RangeInclusive<u8> = 1..=7;

/// A body part of `hit_params` (the player's hit zones).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BodyPart {
    Brain,
    Face,
    Front,
    Back,
    LeftForearm,
    RightForearm,
    LeftArm,
    RightArm,
    LeftHip,
    RightHip,
    LeftFoot,
    RightFoot,
}

impl BodyPart {
    fn parse(key: &str) -> Option<Self> {
        Some(match key {
            "brain" => Self::Brain,
            "face" => Self::Face,
            "front" => Self::Front,
            "back" => Self::Back,
            "left_forearm" => Self::LeftForearm,
            "right_forearm" => Self::RightForearm,
            "left_arm" => Self::LeftArm,
            "right_arm" => Self::RightArm,
            "left_hip" => Self::LeftHip,
            "right_hip" => Self::RightHip,
            "left_foot" => Self::LeftFoot,
            "right_foot" => Self::RightFoot,
            _ => return None,
        })
    }
}

/// A hit type of `hit_params`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HitType {
    Ranged,
    Slash,
    Explosive,
    Fire,
    Radiation,
    Shock,
    Toxic,
}

impl HitType {
    pub const ALL: [HitType; 7] = [
        Self::Ranged,
        Self::Slash,
        Self::Explosive,
        Self::Fire,
        Self::Radiation,
        Self::Shock,
        Self::Toxic,
    ];

    fn parse(key: &str) -> Option<Self> {
        Some(match key {
            "ranged" => Self::Ranged,
            "slash" => Self::Slash,
            "explosive" => Self::Explosive,
            "fire" => Self::Fire,
            "radiation" => Self::Radiation,
            "shock" => Self::Shock,
            "toxic" => Self::Toxic,
            _ => return None,
        })
    }
}

/// A gas filter of an oxygen tank (`data.influences`): while the tank is on,
/// a hit on `part` of `hit` type up to `threshold` does nothing, and above
/// it `(damage - threshold) * hit_coeff` gets through
/// (docs/notes/armour-params.md).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GasFilter {
    pub part: BodyPart,
    pub hit: HitType,
    pub threshold: f32,
    pub hit_coeff: f32,
}

/// One armour piece's config: what changes play (docs/notes/armour-params.md).
#[derive(Clone, Debug, PartialEq)]
pub struct Armour {
    pub dict_id: u16,
    /// Each part's protection per hit type, a fraction of the damage taken
    /// away (`hit_params.<part>.hit_types.<type>.armor`); worn pieces add up.
    pub protection: Vec<(BodyPart, HitType, f32)>,
    /// Its model (`data.base_model`), as the content's paths name it.
    pub model: Option<String>,
    /// An oxygen tank's seconds (`data.amount_time_sec`) and gas filters.
    pub oxygen: Option<f32>,
    pub gas: Vec<GasFilter>,
}

fn number(value: &ConfigValue, what: &str) -> Result<f32, ConfigError> {
    value
        .as_f32()
        .ok_or_else(|| ConfigError(format!("`{what}` is not a number")))
}

fn named<'a, T>(
    value: &'a ConfigValue,
    key: &str,
    parse: fn(&str) -> Option<T>,
) -> Result<T, ConfigError> {
    let text = value.str_at(key)?;
    parse(text).ok_or_else(|| ConfigError(format!("`{key}` `{text}` is not known")))
}

impl Armour {
    /// The row of the armour config of `dict_id`.
    pub fn from_config(dict_id: u16, config: &ConfigValue) -> Result<Self, ConfigError> {
        let mut protection = Vec::new();
        if let Some(parts) = config.get("hit_params") {
            for (part_key, part) in parts.entries() {
                let body = BodyPart::parse(part_key).ok_or_else(|| {
                    ConfigError(format!("`hit_params.{part_key}` is no body part"))
                })?;
                for (type_key, hit) in part.req("hit_types")?.entries() {
                    let hit_type = HitType::parse(type_key).ok_or_else(|| {
                        ConfigError(format!("`hit_params.{part_key}.{type_key}` is no hit type"))
                    })?;
                    let armor = number(hit.req("armor")?, "armor")?;
                    protection.push((body, hit_type, armor));
                }
            }
        }
        let data = config.get("data");
        let gas = match data.and_then(|data| data.get("influences")) {
            None => Vec::new(),
            Some(rows) => rows
                .children()
                .into_iter()
                .map(|row| {
                    Ok(GasFilter {
                        part: named(row, "body_part", BodyPart::parse)?,
                        hit: named(row, "hit_type", HitType::parse)?,
                        threshold: number(row.req("threshold")?, "threshold")?,
                        hit_coeff: number(row.req("hit_coeff")?, "hit_coeff")?,
                    })
                })
                .collect::<Result<_, ConfigError>>()?,
        };
        Ok(Self {
            dict_id,
            protection,
            model: data
                .and_then(|data| data.get("base_model"))
                .and_then(ConfigValue::as_str)
                .map(str::to_owned),
            oxygen: data
                .and_then(|data| data.get("amount_time_sec"))
                .map(|value| number(value, "amount_time_sec"))
                .transpose()?,
            gas,
        })
    }
}

/// Every armour piece of `dictionary` (its armour categories), by `dict_id`.
pub fn derive(data: &GameData, dictionary: &Dictionary) -> Result<Vec<Armour>, GameDataError> {
    let mut rows = Vec::new();
    for item in dictionary
        .items
        .iter()
        .filter(|item| CATEGORIES.contains(&item.category))
    {
        let path = item.cfg_name.as_str();
        let config = super::game_data::config(data, path)?.ok_or_else(|| {
            GameDataError(format!(
                "item {} `{}` has no {path}",
                item.dict_id, item.name
            ))
        })?;
        rows.push(
            Armour::from_config(item.dict_id, &config)
                .map_err(|cause| GameDataError(format!("{path}: {}", cause.0)))?,
        );
    }
    rows.sort_by_key(|row| row.dict_id);
    Ok(rows)
}

/// Every armour piece of this process's game data
/// ([`super::game_data::shared`]), which srp read at start.
pub fn all() -> &'static [Armour] {
    static SHARED: OnceLock<Vec<Armour>> = OnceLock::new();
    SHARED.get_or_init(|| {
        derive(super::game_data::shared(), Dictionary::shared()).expect("the armour configs read")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_armour_piece_reads_with_its_protection() {
        let all = all();
        assert!(all.len() > 100, "{}", all.len());
        let dictionary = Dictionary::shared();
        // A body armour protects the chest from bullets.
        assert!(all.iter().any(|row| {
            dictionary.item(row.dict_id).unwrap().category == 3
                && row.protection.iter().any(|&(part, hit, armor)| {
                    part == BodyPart::Front && hit == HitType::Ranged && armor > 0.5
                })
        }));
        // An oxygen tank has its seconds and filters gas on the face.
        let tank = all
            .iter()
            .find(|row| row.oxygen.is_some_and(|seconds| seconds > 0.0))
            .unwrap();
        assert!(
            tank.gas
                .iter()
                .any(|filter| filter.part == BodyPart::Face && filter.hit == HitType::Toxic)
        );
        // A helmet names its model.
        assert!(all.iter().any(|row| {
            row.model
                .as_deref()
                .is_some_and(|model| model.contains("helmet"))
        }));
    }

    /// Each STK2 outfit piece's numbers (the owner's, 2026-10-31;
    /// docs/notes/stk2-outfits.md) are inside the range of the other armour
    /// pieces of its slot and level in the game data: its armour (the most
    /// any part gets against bullets) and isolation (against fire), weight,
    /// price and drop weight; and its bonuses are ones other armour carries.
    /// A slot with fewer than two other pieces at the level is measured
    /// against the nearest levels below and above too. Premium copies and
    /// pieces no longer dropped are left out. It makes that data the
    /// process's: run it alone.
    #[test]
    #[ignore = "needs game data with the STK2 outfits' packs (SRP_GAME_DATA); run alone"]
    fn the_stk2_outfits_are_inside_their_slot_and_levels_stock_range() {
        use super::super::{game_data, item_params::item_params};
        let path = std::env::var_os(game_data::GAME_DATA_ENV).expect("SRP_GAME_DATA");
        game_data::init_from(std::path::Path::new(&path)).expect("its data, alone");
        let dictionary = Dictionary::shared();
        let armour: std::collections::HashMap<u16, &Armour> =
            all().iter().map(|row| (row.dict_id, row)).collect();
        let most = |id: u16, hit: HitType| {
            armour[&id]
                .protection
                .iter()
                .filter(|&&(_, h, _)| h == hit)
                .map(|&(_, _, a)| a)
                .fold(0.0_f32, f32::max)
        };
        let ours: Vec<_> = dictionary
            .items
            .iter()
            .filter(|item| item.name.starts_with("stk2_") && CATEGORIES.contains(&item.category))
            .collect();
        assert_eq!(ours.len(), 14);
        let others: Vec<_> = dictionary
            .items
            .iter()
            .filter(|item| {
                CATEGORIES.contains(&item.category)
                    && !item.is_premium
                    && !item.name.starts_with("stk2_")
                    && item_params(item.dict_id).is_some_and(|p| p.drop_weight > 1)
            })
            .collect();
        let stock_bonuses: std::collections::HashSet<u32> = others
            .iter()
            .flat_map(|item| item_params(item.dict_id).unwrap().default_modifications.iter().copied())
            .collect();
        for item in ours {
            let params = item_params(item.dict_id).unwrap();
            let level = params.item_level;
            let slot: Vec<_> = others.iter().filter(|o| o.category == item.category).collect();
            let at = |l: u8| slot.iter().filter(|o| item_params(o.dict_id).unwrap().item_level == l).count();
            let mut levels = vec![level];
            if at(level) < 2 {
                let lower = slot.iter().map(|o| item_params(o.dict_id).unwrap().item_level).filter(|&l| l < level).max();
                let higher = slot.iter().map(|o| item_params(o.dict_id).unwrap().item_level).filter(|&l| l > level).min();
                levels.extend(lower.into_iter().chain(higher));
            }
            let frame: Vec<_> = slot
                .iter()
                .filter(|o| levels.contains(&item_params(o.dict_id).unwrap().item_level))
                .collect();
            assert!(!frame.is_empty(), "{}: no other piece of its slot near level {level}", item.name);
            let check = |what: &str, value: f32, of: &dyn Fn(u16) -> f32| {
                let values: Vec<f32> = frame.iter().map(|o| of(o.dict_id)).collect();
                let (lo, hi) = values.iter().fold((f32::MAX, f32::MIN), |(lo, hi), &v| (lo.min(v), hi.max(v)));
                assert!(
                    value >= lo - 1e-4 && value <= hi + 1e-4,
                    "{} (level {level}): {what} {value} outside {lo}..{hi} of levels {levels:?}",
                    item.name
                );
            };
            check("armour", most(item.dict_id, HitType::Ranged), &|id| most(id, HitType::Ranged));
            check("isolation", most(item.dict_id, HitType::Fire), &|id| most(id, HitType::Fire));
            check("weight", params.weight, &|id| item_params(id).unwrap().weight);
            check("price", item.base_cost as f32, &|id| dictionary.item(id).unwrap().base_cost as f32);
            check("drop weight", params.drop_weight as f32, &|id| item_params(id).unwrap().drop_weight as f32);
            for bonus in params.default_modifications {
                assert!(stock_bonuses.contains(bonus), "{}: bonus {bonus} no other armour has", item.name);
            }
        }
    }
}
