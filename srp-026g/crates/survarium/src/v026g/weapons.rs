//! What each weapon of the game data does when it fires and reloads: its
//! rate of fire, fire modes, magazine, the states it has, its reload and
//! bolt times, its kick (`recoil`) and its damage distances, read from its
//! own config (`gameplay/items/weapons/wpn_<name>.options` of the players'
//! `resources.db` and patches, [`super::game_data`]). The match server's
//! bots handle each weapon by these (`match-server` `bots::guns`,
//! `bots::recoil`; `docs/notes/player_bots_weapons.md`), so a weapon a
//! patch adds or rebalances is fired by its own numbers, never by a table
//! kept in code (`AGENTS.md`).
//!
//! The state numbers are the `state_type` of each block of `states`
//! (0.26g's data, every weapon of it read): 300 show, 301 hide, 302 idle,
//! 303 a magazine reload, 304 working the bolt (`chamber_a_round`), 305
//! fire, 306 a reload round by round, 311 two barrels' reload, 314 a
//! reload with another clip from empty, 310 / 312 / 313 / 315 the
//! sawn-off's and the empty-weapon idle and fire variants, 316 a grenade,
//! 321 melee.

use binary_config_parser::tree::{ConfigError, ConfigValue};

use super::dictionary::Dictionary;
use super::game_data::{GameData, GameDataError};

/// `state_type` of a magazine reload.
pub const STATE_RELOAD_MAGAZINE: u16 = 303;
/// `state_type` of working the bolt between shots.
pub const STATE_CHAMBER: u16 = 304;
/// `state_type` of a reload round by round.
pub const STATE_RELOAD_ROUNDS: u16 = 306;
/// `state_type` of two barrels' reload.
pub const STATE_RELOAD_BARRELS: u16 = 311;
/// `state_type` of a reload with its own clip from empty.
pub const STATE_RELOAD_EMPTY: u16 = 314;

/// A range of the config, `(min, max)`.
pub type Range = (f32, f32);

/// One weapon's firing and handling numbers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeaponParams {
    pub dict_id: u16,
    /// The dictionary name (`items_dict`).
    pub name: &'static str,
    /// `parameters.type`: `wpn_aslt`, `wpn_smg`, `wpn_bolt`, `wpn_pstl`, ...
    pub kind: &'static str,
    /// The dictionary category: 8 rifles, 13 sniper rifles, 14 shotguns,
    /// 15 submachine guns, 16 machine guns, 17 pistols.
    pub category: u8,
    pub rounds_per_minute: u32,
    pub magazine_capacity: u32,
    /// `fire_queue_types[0]`, the fire mode a weapon has in hand: -1 the
    /// whole magazine while the trigger is held, else rounds a press.
    pub first_fire_mode: i32,
    /// `reload_time`, `reload_start_time`, `reload_end_time`, seconds.
    pub reload_time: f32,
    pub reload_start_time: f32,
    pub reload_end_time: f32,
    /// `chamber_a_round_time`, seconds: the bolt worked after a shot (the
    /// card's rate of fire is `60 / (60 / rpm + this)`, floored, for every
    /// weapon that has one).
    pub chamber_a_round_time: f32,
    /// The reload's `state_type` ([`STATE_RELOAD_MAGAZINE`], ...).
    pub reload_state: u16,
    /// It has a [`STATE_CHAMBER`] state.
    pub has_chamber: bool,
    pub show_time: f32,
    pub hide_time: f32,
    /// `aim_time`, seconds: to bring the sights up.
    pub aim_time: f32,
    /// `bullet_damage`, and full damage up to `max_damage_distance`, down
    /// to `min_damage_amount` of it at `min_damage_distance`, metres.
    pub bullet_damage: f32,
    pub max_damage_distance: f32,
    pub min_damage_distance: f32,
    /// `recoil`: `min/max_top_recoil`, `min/max_left_recoil`,
    /// `min/max_right_recoil` (the config's degrees), `recoil_power`, and
    /// the controller's gains (proportional, integral, derivational).
    pub top_recoil: Range,
    pub left_recoil: Range,
    pub right_recoil: Range,
    pub recoil_power: f32,
    pub recoil_gains: (f32, f32, f32),
}

impl WeaponParams {
    /// Its first fire mode empties the magazine while the trigger is held.
    pub fn automatic(&self) -> bool {
        self.first_fire_mode < 0
    }

    /// From an item's config.
    pub fn from_config(
        dict_id: u16,
        name: &'static str,
        category: u8,
        config: &ConfigValue,
    ) -> Result<Self, ConfigError> {
        let p = config.req("parameters")?;
        let recoil = config.get("recoil");
        let number = |table: Option<&ConfigValue>, key: &str| -> Result<f32, ConfigError> {
            match table.and_then(|t| t.get(key)) {
                None => Ok(0.0),
                Some(value) => value
                    .as_f32()
                    .ok_or_else(|| ConfigError(format!("`{key}` is not a number"))),
            }
        };
        let range = |key: &str| -> Result<Range, ConfigError> {
            Ok((
                number(recoil, &format!("min_{key}_recoil"))?,
                number(recoil, &format!("max_{key}_recoil"))?,
            ))
        };
        let kind = p
            .req("type")?
            .as_str()
            .ok_or_else(|| ConfigError("`parameters.type` is not a string".into()))?;
        let first_fire_mode = p
            .get("fire_queue_types")
            .and_then(|list| list.children().first().and_then(|v| v.as_i32()))
            .ok_or_else(|| ConfigError("no `fire_queue_types`".into()))?;
        let mut reload_state = 0;
        let mut has_chamber = false;
        if let Some(states) = config.get("states") {
            for (_, state) in states.entries() {
                let Some(kind) = state.get("state_type").and_then(ConfigValue::as_i32) else {
                    continue;
                };
                match kind as u16 {
                    STATE_CHAMBER => has_chamber = true,
                    k @ (STATE_RELOAD_MAGAZINE
                    | STATE_RELOAD_ROUNDS
                    | STATE_RELOAD_BARRELS
                    | STATE_RELOAD_EMPTY) => reload_state = k,
                    _ => {}
                }
            }
        }
        let rpm = number(Some(p), "rounds_per_minute")?;
        if rpm <= 0.0 {
            return Err(ConfigError("`rounds_per_minute` is not above 0".into()));
        }
        Ok(Self {
            dict_id,
            name,
            kind: Box::leak(kind.to_owned().into_boxed_str()),
            category,
            rounds_per_minute: rpm.round() as u32,
            magazine_capacity: number(Some(p), "magazine_capacity")?.max(0.0) as u32,
            first_fire_mode,
            reload_time: number(Some(p), "reload_time")?,
            reload_start_time: number(Some(p), "reload_start_time")?,
            reload_end_time: number(Some(p), "reload_end_time")?,
            chamber_a_round_time: number(Some(p), "chamber_a_round_time")?,
            reload_state,
            has_chamber,
            show_time: number(Some(p), "show_time")?,
            hide_time: number(Some(p), "hide_time")?,
            aim_time: number(Some(p), "aim_time")?,
            bullet_damage: number(Some(p), "bullet_damage")?,
            max_damage_distance: number(Some(p), "max_damage_distance")?,
            min_damage_distance: number(Some(p), "min_damage_distance")?,
            top_recoil: range("top")?,
            left_recoil: range("left")?,
            right_recoil: range("right")?,
            recoil_power: number(recoil, "recoil_power")?,
            recoil_gains: (
                number(recoil, "compensation_proportional_gain")?,
                number(recoil, "compensation_integral_gain")?,
                number(recoil, "compensation_derivational_gain")?,
            ),
        })
    }
}

/// Every weapon of `dictionary` (an item whose config's `parameters.type`
/// starts with `wpn_`), by `dict_id`.
pub fn derive(
    data: &GameData,
    dictionary: &'static Dictionary,
) -> Result<Vec<WeaponParams>, GameDataError> {
    let mut rows = Vec::new();
    for item in &dictionary.items {
        let path = item.cfg_name.as_str();
        let Some(config) = super::game_data::config(data, path)? else {
            continue;
        };
        let is_weapon = config
            .get("parameters")
            .and_then(|p| p.get("type"))
            .and_then(ConfigValue::as_str)
            .is_some_and(|kind| kind.starts_with("wpn_"));
        if !is_weapon {
            continue;
        }
        let row = WeaponParams::from_config(item.dict_id, &item.name, item.category, &config)
            .map_err(|cause| GameDataError(format!("{path}: {}", cause.0)))?;
        rows.push(row);
    }
    rows.sort_by_key(|row| row.dict_id);
    Ok(rows)
}

/// Every weapon of the game data, read once.
pub fn all() -> &'static [WeaponParams] {
    static SHARED: std::sync::OnceLock<Vec<WeaponParams>> = std::sync::OnceLock::new();
    SHARED.get_or_init(|| {
        derive(super::game_data::shared(), Dictionary::shared()).expect("the weapon configs read")
    })
}

/// The weapon `dict_id`.
pub fn weapon(dict_id: u16) -> Option<&'static WeaponParams> {
    let all = all();
    all.binary_search_by_key(&dict_id, |w| w.dict_id)
        .ok()
        .map(|i| &all[i])
}

/// The weapon named `name` (`items_dict`).
pub fn weapon_named(name: &str) -> Option<&'static WeaponParams> {
    all().iter().find(|w| w.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v026g::item_params::item_params;

    #[test]
    fn the_weapons_are_read_from_their_configs() {
        let all = all();
        assert!(all.len() > 30, "{} weapons", all.len());
        for w in all {
            assert!(w.rounds_per_minute > 0, "{}", w.name);
            assert_eq!(
                w.magazine_capacity,
                item_params(w.dict_id).unwrap().magazine_capacity,
                "{}",
                w.name
            );
            assert_ne!(w.reload_state, 0, "{} has a reload state", w.name);
            // A bolt (a chamber state) never fires a burst.
            if w.has_chamber {
                assert!(!w.automatic(), "{}", w.name);
            }
        }
        let mosin = weapon_named("mosin_nagant").unwrap();
        assert_eq!((mosin.rounds_per_minute, mosin.magazine_capacity), (160, 5));
        assert!(mosin.has_chamber && !mosin.automatic());
        assert_eq!(mosin.reload_state, STATE_RELOAD_ROUNDS);
        assert_eq!(
            (
                mosin.chamber_a_round_time,
                mosin.reload_start_time,
                mosin.reload_end_time
            ),
            (1.8, 1.5, 1.5)
        );
        let uzi = weapon_named("uzi").unwrap();
        assert!(uzi.automatic() && !uzi.has_chamber);
        assert_eq!(uzi.reload_state, STATE_RELOAD_MAGAZINE);
        assert_eq!(uzi.top_recoil, (1.0, 1.2));
        assert_eq!(uzi.recoil_gains, (5.0, 15.0, 0.0));
        let toz = weapon_named("toz_34").unwrap();
        assert_eq!(
            (toz.first_fire_mode, toz.reload_state),
            (1, STATE_RELOAD_BARRELS)
        );
        let tt = weapon_named("tt_33").unwrap();
        assert_eq!(tt.reload_state, STATE_RELOAD_EMPTY);
        assert_eq!(weapon(mosin.dict_id), Some(mosin));
    }

    /// The players' game data with the 0.29a3 weapons (revision 19 on):
    /// `SRP_TEST_NEW_WEAPONS_DATA=<server-game-data.db> cargo test -p
    /// survarium --lib -- --ignored every_weapon_of_the_players_data`.
    #[test]
    #[ignore = "needs the players' server game data (SRP_TEST_NEW_WEAPONS_DATA)"]
    fn every_weapon_of_the_players_data_reads() {
        let path = std::env::var_os("SRP_TEST_NEW_WEAPONS_DATA").expect("the data's path");
        let data = GameData::open(std::path::Path::new(&path)).unwrap();
        let dictionary: &'static Dictionary = Box::leak(Box::new(
            Dictionary::parse(data.file(super::super::dictionary::PATH).unwrap()).unwrap(),
        ));
        let all = derive(&data, dictionary).unwrap();
        for name in [
            "kriss_vector",
            "hk416",
            "ak_12",
            "fn_p90",
            "vsk_94",
            "vssk",
            "sv_98",
        ] {
            let w = all.iter().find(|w| w.name == name).unwrap();
            assert!(w.rounds_per_minute > 0 && w.reload_state != 0, "{name}");
        }
        let vssk = all.iter().find(|w| w.name == "vssk").unwrap();
        assert!(vssk.has_chamber && !vssk.automatic());
    }
}
