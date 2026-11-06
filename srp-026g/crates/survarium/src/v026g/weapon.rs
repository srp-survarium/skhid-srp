//! What a weapon's config says of its damage, handling, recoil and spread,
//! for srp's per-release item tables (`srp-item-tables`): the values the
//! wiki's weapon cards and the calculator read from the same configs
//! (`parameters`, `recoil`, `dispersion`). Read from each item of the
//! dictionary's weapon categories. As the server's item params, a key or a
//! section the config does not have is not an error (here: no value); a
//! value of the wrong kind is, and a weapon without a config.

use binary_config_parser::tree::{ConfigError, ConfigValue};

use super::dictionary::Dictionary;
use super::game_data::{GameData, GameDataError};

/// `items_dict` categories of weapons: assault rifle, sniper rifle,
/// shotgun, submachine gun, machine gun, pistol.
pub const CATEGORIES: [u8; 6] = [8, 13, 14, 15, 16, 17];

/// The kick a shot gives (`recoil`): each a range a shot draws from, in the
/// configs' units (a degree of view upward: docs/notes and bots/recoil.rs).
/// A range the config does not give both ends of is `None`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Recoil {
    pub top: Option<(f32, f32)>,
    pub left: Option<(f32, f32)>,
    pub right: Option<(f32, f32)>,
    /// `recoil_power`: how the kick shrinks as it accumulates.
    pub power: Option<f32>,
}

/// The spread (`dispersion`, get_dispersed_bullet_direction 0x2E1D50): the
/// base aimed, what a shot adds from the hip, its limit and its fall a
/// second.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spread {
    pub base: Option<f32>,
    pub per_shot: Option<f32>,
    pub max: Option<f32>,
    pub recovery: Option<f32>,
}

/// One weapon's config: what the tables show, `None` where it says nothing.
#[derive(Clone, Debug, PartialEq)]
pub struct Weapon {
    pub dict_id: u16,
    /// `bullet_damage`, `bullet_pierce`, `min_damage_amount`: fractions.
    pub damage: Option<f32>,
    pub pierce: Option<f32>,
    pub min_damage_amount: Option<f32>,
    /// Metres: full damage up to, the minimum from.
    pub max_damage_distance: Option<f32>,
    pub min_damage_distance: Option<f32>,
    pub rounds_per_minute: Option<f32>,
    pub magazine: Option<u32>,
    /// Seconds.
    pub reload_time: Option<f32>,
    pub aim_time: Option<f32>,
    /// Kilograms.
    pub weight: Option<f32>,
    pub recoil: Recoil,
    pub spread: Spread,
}

/// `key` of `table`: `None` without the table or the key, an error when it
/// is not a number.
fn number(table: Option<&ConfigValue>, key: &str) -> Result<Option<f32>, ConfigError> {
    table
        .and_then(|table| table.get(key))
        .map(|value| {
            value
                .as_f32()
                .ok_or_else(|| ConfigError(format!("`{key}` is not a number")))
        })
        .transpose()
}

fn range(table: Option<&ConfigValue>, axis: &str) -> Result<Option<(f32, f32)>, ConfigError> {
    Ok(number(table, &format!("min_{axis}_recoil"))?
        .zip(number(table, &format!("max_{axis}_recoil"))?))
}

impl Weapon {
    /// The row of the weapon config of `dict_id`.
    pub fn from_config(dict_id: u16, config: &ConfigValue) -> Result<Self, ConfigError> {
        let p = config.get("parameters");
        let recoil = config.get("recoil");
        let dispersion = config.get("dispersion");
        let magazine = match number(p, "magazine_capacity")? {
            None => None,
            Some(count) if count >= 0.0 && count.fract() == 0.0 => Some(count as u32),
            Some(other) => {
                return Err(ConfigError(format!(
                    "`magazine_capacity` {other} is no count"
                )));
            }
        };
        Ok(Self {
            dict_id,
            damage: number(p, "bullet_damage")?,
            pierce: number(p, "bullet_pierce")?,
            min_damage_amount: number(p, "min_damage_amount")?,
            max_damage_distance: number(p, "max_damage_distance")?,
            min_damage_distance: number(p, "min_damage_distance")?,
            rounds_per_minute: number(p, "rounds_per_minute")?,
            magazine,
            reload_time: number(p, "reload_time")?,
            aim_time: number(p, "aim_time")?,
            weight: number(p, "weight")?,
            recoil: Recoil {
                top: range(recoil, "top")?,
                left: range(recoil, "left")?,
                right: range(recoil, "right")?,
                power: number(recoil, "recoil_power")?,
            },
            spread: Spread {
                // The dispersion loader's (0x330EE0).
                base: number(dispersion, "base_dispersion")?,
                per_shot: number(dispersion, "one_shoot_dispersion_amount")?,
                max: number(dispersion, "max_dispersion")?,
                recovery: number(dispersion, "speed_of_aiming")?,
            },
        })
    }
}

/// Every weapon of `dictionary` (its weapon categories), by `dict_id`.
pub fn derive(data: &GameData, dictionary: &Dictionary) -> Result<Vec<Weapon>, GameDataError> {
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
            Weapon::from_config(item.dict_id, &config)
                .map_err(|cause| GameDataError(format!("{path}: {}", cause.0)))?,
        );
    }
    rows.sort_by_key(|row| row.dict_id);
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_weapon_reads_and_the_ak_74m_is_as_the_lobby_says() {
        let dictionary = Dictionary::shared();
        let all = derive(super::super::game_data::shared(), dictionary).unwrap();
        assert!(all.len() >= 40, "{}", all.len());
        let ak = dictionary.item_named("ak_74m").unwrap().dict_id;
        let ak = all.iter().find(|row| row.dict_id == ak).unwrap();
        assert_eq!(ak.damage, Some(0.5));
        assert_eq!(ak.rounds_per_minute, Some(650.0));
        assert_eq!(ak.magazine, Some(30));
        assert_eq!(ak.recoil.top, Some((1.8, 2.0)));
        assert_eq!(ak.spread.base, Some(0.2));
    }

    #[test]
    fn a_key_or_section_left_out_is_no_value_and_a_wrong_one_an_error() {
        let config = |json: &str| {
            let json = serde_json::from_str(json).unwrap();
            ConfigValue::parse(binary_config_parser::BinaryConfig::parse_json(&json).as_bytes())
                .unwrap()
        };
        let bare =
            Weapon::from_config(1, &config(r#"{"parameters": {"bullet_damage": 0.4}}"#)).unwrap();
        assert_eq!(bare.damage, Some(0.4));
        assert_eq!(bare.magazine, None);
        assert_eq!(bare.recoil.top, None);
        assert_eq!(bare.spread.base, None);
        let half = config(r#"{"recoil": {"min_top_recoil": 1}}"#);
        assert_eq!(Weapon::from_config(1, &half).unwrap().recoil.top, None);
        assert!(Weapon::from_config(1, &config(r#"{"parameters": {"weight": "heavy"}}"#)).is_err());
        assert!(
            Weapon::from_config(1, &config(r#"{"parameters": {"magazine_capacity": 2.5}}"#))
                .is_err()
        );
    }
}
