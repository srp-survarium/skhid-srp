//! What an artefact's config says it does, for srp's site (the wiki's artefacts;
//! docs/notes/artefacts.md): each item of `gameplay/items/artefacts/` read as the game's loader
//! of its kind (`data.type`, [`ItemClass::Artefact`]) and exe patch 54 read it. A kind's
//! bonuses are its own type; the keys no code reads (Spring's `max_carried_weight_mod`, patch
//! 54's aim-time node since) are left out. [`super::tables::check`] reads them all at start.

use std::sync::OnceLock;

use binary_config_parser::tree::{ConfigError, ConfigValue};
use srp_validate::world::Artefact as Kind;

use super::armour::{BodyPart, HitType};
use super::dictionary::Dictionary;
use super::game_data::{GameData, GameDataError};
use super::item_params::ItemClass;

/// The folder of the artefacts' configs.
const FOLDER: &str = "gameplay/items/artefacts/";

/// A change of a player modifier: a node of the kit's list, summed with the others of the list
/// (every carried copy's, the skills', the gear's), the value `base × max(0, 1 + sum)`
/// (`apply_modifier` 0x2D5AE0; none of Spring's lists is additive).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Modifier(pub f32);

/// `(x + add) × mul`: a Lifebone's regeneration term, an Onyx's damage and piercing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Affine {
    pub add: f32,
    pub mul: f32,
}

/// A damage-model pool a Lifebone heals (`body_part` of its lists).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Pool {
    Body,
    Head,
    Pain,
    LeftHand,
    RightHand,
    LeftLeg,
    RightLeg,
}

impl Pool {
    fn parse(key: &str) -> Option<Self> {
        Some(match key {
            "body" => Self::Body,
            "head" => Self::Head,
            "pain" => Self::Pain,
            "left_hand" => Self::LeftHand,
            "right_hand" => Self::RightHand,
            "left_leg" => Self::LeftLeg,
            "right_leg" => Self::RightLeg,
            _ => return None,
        })
    }
}

/// A Lifebone's regeneration of one pool (`passive/regeneration_modifiers`, its callback
/// 0x312590 on the pool's tick): speed, delay and limit each `(x + add) × mul`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Regeneration {
    pub pool: Pool,
    /// Health a second (a pool's health is 0..1 of the body's).
    pub speed: Affine,
    /// Seconds without a hit before it starts.
    pub delay: Affine,
    /// The share of the pool's maximum it regenerates to (at most 1).
    pub limit: Affine,
}

/// An Onyx's change of a hit (its hit callback 0x312BF0).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reduction {
    pub damage: Affine,
    pub piercing: Affine,
}

/// What a kind does, carried (its passive) and fired (its active).
#[derive(Clone, Debug, PartialEq)]
pub enum Effects {
    Lifebone {
        regeneration: Vec<Regeneration>,
        /// The pools whose injury (`affect`) its active removes; exe patch 54 also fills every
        /// pool to its maximum.
        cures: Vec<Pool>,
    },
    Spring {
        speed: Modifier,
        /// Patch 54's `passive/aim_time_mod` (no key in the retail config).
        aim_time: Option<Modifier>,
        active: SpringActive,
    },
    Onyx {
        hit: HitType,
        parts: Vec<BodyPart>,
        passive: Reduction,
        active: Reduction,
    },
}

/// A Spring's active, on top of its passive.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpringActive {
    pub speed: Modifier,
    pub stamina_regeneration: Modifier,
    pub stamina_use: Modifier,
    /// Patch 54's `active/aim_time_mod` and `active/stopping_power_mod`.
    pub aim_time: Option<Modifier>,
    pub stopping_power: Option<Modifier>,
}

/// One artefact item.
#[derive(Clone, Debug, PartialEq)]
pub struct ArtefactConfig {
    pub dict_id: u16,
    pub kind: Kind,
    /// Kilograms (`parameters.weight`).
    pub weight: f32,
    /// `data.active.duration`: seconds, times the holder's modifier 39 (artefact duration).
    pub active_s: f32,
    /// `data.cooldown_sec`: seconds, times the holder's modifier 38 (artefact cooldown).
    pub cooldown_s: f32,
    /// `data.pickup_hint`: the localization key of the text shown at its pickup, which names it.
    pub pickup_hint: String,
    pub effects: Effects,
}

fn modifier(table: &ConfigValue, key: &str) -> Result<Modifier, ConfigError> {
    table.f32_at(key).map(Modifier)
}

fn optional(table: &ConfigValue, key: &str) -> Result<Option<Modifier>, ConfigError> {
    table.get(key).map(|_| modifier(table, key)).transpose()
}

fn affine(table: &ConfigValue, add: &str, mul: &str) -> Result<Affine, ConfigError> {
    Ok(Affine {
        add: table.f32_at(add)?,
        mul: table.f32_at(mul)?,
    })
}

fn reduction(table: &ConfigValue) -> Result<Reduction, ConfigError> {
    Ok(Reduction {
        damage: affine(table, "damage_add", "damage_mul")?,
        piercing: affine(table, "armor_piercing_add", "armor_piercing_mul")?,
    })
}

fn named<T>(
    value: &ConfigValue,
    key: &str,
    parse: fn(&str) -> Option<T>,
) -> Result<T, ConfigError> {
    let text = value.str_at(key)?;
    parse(text).ok_or_else(|| ConfigError(format!("`{key}` `{text}` is not known")))
}

impl ArtefactConfig {
    /// The artefact config of `dict_id`; an item of no artefact class is refused.
    pub fn from_config(dict_id: u16, config: &ConfigValue) -> Result<Self, ConfigError> {
        let ItemClass::Artefact(kind) = ItemClass::from_config(config)? else {
            return Err(ConfigError("`data.type` is no artefact's".into()));
        };
        let data = config.req("data")?;
        let passive = data.req("passive")?;
        let active = data.req("active")?;
        let effects = match kind {
            Kind::Lifebone => Effects::Lifebone {
                regeneration: passive
                    .req("regeneration_modifiers")?
                    .children()
                    .into_iter()
                    .map(|row| {
                        Ok(Regeneration {
                            pool: named(row, "body_part", Pool::parse)?,
                            speed: affine(row, "regen_add", "regen_mul")?,
                            delay: affine(row, "timeout_add", "timeout_mul")?,
                            limit: affine(row, "threshold_add", "threshold_mul")?,
                        })
                    })
                    .collect::<Result<_, ConfigError>>()?,
                cures: active
                    .get("remove_affects")
                    .map(ConfigValue::children)
                    .unwrap_or_default()
                    .into_iter()
                    .map(|row| named(row, "body_part", Pool::parse))
                    .collect::<Result<_, ConfigError>>()?,
            },
            Kind::Spring => Effects::Spring {
                speed: modifier(passive, "movement_speed_mod")?,
                aim_time: optional(passive, "aim_time_mod")?,
                active: SpringActive {
                    speed: modifier(active, "movement_speed_mod")?,
                    stamina_regeneration: modifier(active, "stamina_regen_mod")?,
                    stamina_use: modifier(active, "stamina_spend_mod")?,
                    aim_time: optional(active, "aim_time_mod")?,
                    stopping_power: optional(active, "stopping_power_mod")?,
                },
            },
            Kind::Onyx => Effects::Onyx {
                hit: named(data, "hit_type", HitType::parse)?,
                parts: data
                    .req("protected_body_parts")?
                    .children()
                    .into_iter()
                    .map(|part| {
                        let key = part.as_str().ok_or_else(|| {
                            ConfigError("`protected_body_parts` holds no name".into())
                        })?;
                        BodyPart::parse(key)
                            .ok_or_else(|| ConfigError(format!("`{key}` is no body part")))
                    })
                    .collect::<Result<_, ConfigError>>()?,
                passive: reduction(passive)?,
                active: reduction(active)?,
            },
        };
        Ok(Self {
            dict_id,
            kind,
            weight: config.req("parameters")?.f32_at("weight")?,
            active_s: active.f32_at("duration")?,
            cooldown_s: data.f32_at("cooldown_sec")?,
            pickup_hint: data.str_at("pickup_hint")?.to_owned(),
            effects,
        })
    }
}

/// Every artefact of `dictionary` (its items of [`FOLDER`]), by `dict_id`.
pub fn derive(
    data: &GameData,
    dictionary: &Dictionary,
) -> Result<Vec<ArtefactConfig>, GameDataError> {
    let mut rows = Vec::new();
    for item in dictionary
        .items
        .iter()
        .filter(|item| item.cfg_name.starts_with(FOLDER))
    {
        let path = item.cfg_name.as_str();
        let config = super::game_data::config(data, path)?.ok_or_else(|| {
            GameDataError(format!(
                "item {} `{}` has no {path}",
                item.dict_id, item.name
            ))
        })?;
        rows.push(
            ArtefactConfig::from_config(item.dict_id, &config)
                .map_err(|cause| GameDataError(format!("{path}: {}", cause.0)))?,
        );
    }
    rows.sort_by_key(|row| row.dict_id);
    Ok(rows)
}

/// Every artefact of this process's game data ([`super::game_data::shared`]).
pub fn all() -> &'static [ArtefactConfig] {
    static SHARED: OnceLock<Vec<ArtefactConfig>> = OnceLock::new();
    SHARED.get_or_init(|| {
        derive(super::game_data::shared(), Dictionary::shared()).expect("the artefact configs read")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_reads_with_its_numbers() {
        let all = all();
        let kinds: Vec<Kind> = all.iter().map(|row| row.kind).collect();
        assert_eq!(kinds, [Kind::Lifebone, Kind::Spring, Kind::Onyx]);
        for row in all {
            assert!(
                row.weight > 0.0 && row.active_s > 0.0 && row.cooldown_s > 0.0,
                "{row:?}"
            );
            match &row.effects {
                Effects::Lifebone {
                    regeneration,
                    cures,
                } => {
                    assert_eq!(regeneration.len(), 7);
                    assert_eq!(cures.len(), 4);
                }
                Effects::Spring { speed, .. } => assert!(speed.0 > 0.0),
                Effects::Onyx {
                    hit,
                    parts,
                    passive,
                    active,
                } => {
                    assert_eq!(*hit, HitType::Ranged);
                    assert_eq!(parts.len(), 12);
                    assert!(active.damage.mul < passive.damage.mul && passive.damage.mul < 1.0);
                }
            }
        }
    }
}
