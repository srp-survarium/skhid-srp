//! What a weapon's config says of its damage, handling, recoil and spread,
//! for srp's per-release item tables (`srp-item-tables`): the values the
//! wiki's weapon cards and the calculator read from the same configs
//! (`parameters`, `recoil`, `dispersion`). Read from each item of the
//! dictionary's weapon categories; a config without one of them is refused.

use binary_config_parser::tree::{ConfigError, ConfigValue};

use super::dictionary::Dictionary;
use super::game_data::{GameData, GameDataError};

/// `items_dict` categories of weapons: assault rifle, sniper rifle,
/// shotgun, submachine gun, machine gun, pistol.
pub const CATEGORIES: [u8; 6] = [8, 13, 14, 15, 16, 17];

/// The kick a shot gives (`recoil`): each a range a shot draws from, in the
/// configs' units (a degree of view upward: docs/notes and bots/recoil.rs).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Recoil {
    pub top: (f32, f32),
    pub left: (f32, f32),
    pub right: (f32, f32),
    /// `recoil_power`: how the kick shrinks as it accumulates.
    pub power: f32,
}

/// The spread (`dispersion`, get_dispersed_bullet_direction 0x2E1D50): the
/// base aimed, what a shot adds from the hip, its limit and its fall a
/// second.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spread {
    pub base: f32,
    pub per_shot: f32,
    pub max: f32,
    pub recovery: f32,
}

/// One weapon's config: what the tables show.
#[derive(Clone, Debug, PartialEq)]
pub struct Weapon {
    pub dict_id: u16,
    /// `bullet_damage`, `bullet_pierce`, `min_damage_amount`: fractions.
    pub damage: f32,
    pub pierce: f32,
    pub min_damage_amount: f32,
    /// Metres: full damage up to, the minimum from.
    pub max_damage_distance: f32,
    pub min_damage_distance: f32,
    pub rounds_per_minute: f32,
    pub magazine: u32,
    /// Seconds.
    pub reload_time: f32,
    pub aim_time: Option<f32>,
    /// Kilograms.
    pub weight: f32,
    pub recoil: Recoil,
    pub spread: Spread,
}

fn number(table: &ConfigValue, key: &str) -> Result<f32, ConfigError> {
    table
        .req(key)?
        .as_f32()
        .ok_or_else(|| ConfigError(format!("`{key}` is not a number")))
}

fn range(table: &ConfigValue, axis: &str) -> Result<(f32, f32), ConfigError> {
    Ok((
        number(table, &format!("min_{axis}_recoil"))?,
        number(table, &format!("max_{axis}_recoil"))?,
    ))
}

impl Weapon {
    /// The row of the weapon config of `dict_id`.
    pub fn from_config(dict_id: u16, config: &ConfigValue) -> Result<Self, ConfigError> {
        let p = config.req("parameters")?;
        let recoil = config.req("recoil")?;
        let dispersion = config.req("dispersion")?;
        let magazine = number(p, "magazine_capacity")?;
        Ok(Self {
            dict_id,
            damage: number(p, "bullet_damage")?,
            pierce: number(p, "bullet_pierce")?,
            min_damage_amount: number(p, "min_damage_amount")?,
            max_damage_distance: number(p, "max_damage_distance")?,
            min_damage_distance: number(p, "min_damage_distance")?,
            rounds_per_minute: number(p, "rounds_per_minute")?,
            magazine: if magazine >= 0.0 && magazine.fract() == 0.0 {
                magazine as u32
            } else {
                return Err(ConfigError(format!(
                    "`magazine_capacity` {magazine} is no count"
                )));
            },
            reload_time: number(p, "reload_time")?,
            aim_time: p
                .get("aim_time")
                .map(|_| number(p, "aim_time"))
                .transpose()?,
            weight: number(p, "weight")?,
            recoil: Recoil {
                top: range(recoil, "top")?,
                left: range(recoil, "left")?,
                right: range(recoil, "right")?,
                power: number(recoil, "recoil_power")?,
            },
            spread: Spread {
                // The dispersion loader's (0x330EE0); older configs carry it
                // in `parameters` too.
                base: match dispersion.get("base_dispersion") {
                    Some(_) => number(dispersion, "base_dispersion")?,
                    None => number(p, "base_dispersion")?,
                },
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
        assert_eq!(ak.damage, 0.5);
        assert_eq!(ak.rounds_per_minute, 650.0);
        assert_eq!(ak.magazine, 30);
        assert_eq!(ak.recoil.top, (1.8, 2.0));
        assert_eq!(ak.spread.base, 0.2);
    }
}
