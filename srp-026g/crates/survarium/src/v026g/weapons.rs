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
use resources_db::server::ClipLengths;

use super::dictionary::Dictionary;
use super::game_data::{GameData, GameDataError};

#[cfg(any(test, feature = "fixtures"))]
pub mod fixtures;

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
    /// `parameters.movement_speed_modifier`: added to the character's
    /// movement speed while the weapon is held (+0.05 on 20 of the live
    /// weapons, -0.05 on 10; LIKELY the client's reading, added to the
    /// player modifier 4).
    pub movement_speed_modifier: f32,
    /// The reload's `state_type` ([`STATE_RELOAD_MAGAZINE`], ...).
    pub reload_state: u16,
    /// For a reload with two clips ([`STATE_RELOAD_BARRELS`]: one barrel, both; [`STATE_RELOAD_EMPTY`]: with a
    /// round left, from empty): the second clip's length over the first's, which is how much longer the second
    /// reload lasts than `reload_time`. The game scales the state's clips by `clips[0]` over `reload_time` and
    /// plays the other at that scale (0.100b `computed_reload_animation_time_scale`; PROVEN there, LIKELY the
    /// same in 0.26g, whose constants came out equal for the TT-33 and the TOZ-34). `None` when the game data has
    /// no clip lengths or the state names fewer than two clips: the bots then go by their constants.
    pub reload_share: Option<f32>,
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
    /// `bullet_pierce` (the armour piercing, before the ammo's `k_arp`) and
    /// `min_damage_amount` (the share of the damage and the piercing left
    /// from `min_damage_distance` on): [`super::damage`].
    pub bullet_pierce: f32,
    pub min_damage_amount: f32,
    /// `stamina_damage`: what a bullet (a pellet) that hits takes off the
    /// victim's stamina bar, before its skill 211 (the bots have none).
    pub stamina_damage: f32,
    /// `stopping_power`: what each bullet (pellet) that hits adds, times the ammunition's `k_power`, to the victim's
    /// stagger for `stagger_duration` (0.5 s), the sum capped at `max_stagger` (0.5): the victim's movement speed is
    /// cut by that share and he cannot sprint (PROVEN, `base_player::hit_impl` 0x2D9B90 and the stagger update
    /// 0x2DBCD0, sprint-jump RE `FINDINGS.md` §1.1).
    pub stopping_power: f32,
    /// `recoil`: `min/max_top_recoil`, `min/max_left_recoil`,
    /// `min/max_right_recoil` (the config's degrees), `recoil_power`, and
    /// the controller's gains (proportional, integral, derivational).
    pub top_recoil: Range,
    pub left_recoil: Range,
    pub right_recoil: Range,
    pub recoil_power: f32,
    pub recoil_gains: (f32, f32, f32),
    /// The `dispersion` block: how wide its shots spread.
    pub dispersion: Dispersion,
}

/// A weapon's `dispersion` block (`weapon_dispersion_params` 0x330EE0;
/// `docs/notes/player_bots_weapons.md` §9 has the whole rule). Degrees,
/// the cone's full angle; rates in degrees a second.
///
/// A shot's cone is `base` (× the ammo's `dispersion`, 1 for every ammo
/// of 0.26g) plus, until the aim-in is complete (`aim_time` after the aim
/// is pressed), the character part (the player's stance and movement
/// value × `from_the_hip` or `aim`) and the bloom (`one_shot` a shot, up
/// to `max`). Both fall at `speed_of_aiming`; the character part rises at
/// `growth_speed`. Fully aimed, only `base` is left, moving or not.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Dispersion {
    /// `base_dispersion`.
    pub base: f32,
    /// `one_shoot_dispersion_amount`.
    pub one_shot: f32,
    /// `speed_of_aiming`.
    pub speed_of_aiming: f32,
    /// `growth_speed`.
    pub growth_speed: f32,
    /// `max_dispersion`: the bloom's cap.
    pub max: f32,
    /// `from_the_hip_multiplier`, `aim_multiplier` (1 when absent).
    pub from_the_hip: f32,
    pub aim: f32,
}

impl Dispersion {
    fn from_config(block: Option<&ConfigValue>) -> Result<Self, ConfigError> {
        let number = |key: &str, absent: f32| -> Result<f32, ConfigError> {
            match block.and_then(|t| t.get(key)) {
                None => Ok(absent),
                Some(value) => value
                    .as_f32()
                    .ok_or_else(|| ConfigError(format!("`dispersion.{key}` is not a number"))),
            }
        };
        Ok(Self {
            base: number("base_dispersion", 0.0)?,
            one_shot: number("one_shoot_dispersion_amount", 0.0)?,
            speed_of_aiming: number("speed_of_aiming", 0.0)?,
            growth_speed: number("growth_speed", 0.0)?,
            max: number("max_dispersion", 0.0)?,
            from_the_hip: number("from_the_hip_multiplier", 1.0)?,
            aim: number("aim_multiplier", 1.0)?,
        })
    }

    /// The cone, degrees: `character` the character part reached and
    /// `bloom` the bloom; none of either once `aimed_in`.
    pub fn cone_deg(&self, character: f32, bloom: f32, aimed_in: bool) -> f32 {
        if aimed_in {
            self.base
        } else {
            self.base + character + bloom
        }
    }
}

/// `default.player`'s `character_dispersion_params` (0x2F0D30): the
/// character part of a shot's cone by stance and movement, degrees,
/// before the weapon's multiplier. 0.26g's player: idle 5, walk 10, run
/// and jump 20, crouch 4, crouch walk 8, the aimed values the same.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CharacterDispersion {
    pub idle: f32,
    pub idle_aim: f32,
    pub walk: f32,
    pub walk_aim: f32,
    pub run: f32,
    pub jump: f32,
    pub crouch: f32,
    pub crouch_aim: f32,
    pub crouch_walk: f32,
    pub crouch_walk_aim: f32,
}

/// What a body does, for [`CharacterDispersion::of`]: "moving" is a move
/// key held (forward xor back, or left xor right; the game reads the keys,
/// never the speed).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stance {
    pub crouched: bool,
    pub moving: bool,
    pub sprinting: bool,
    pub jumping: bool,
    /// The aim is pressed (from its start, not its end).
    pub aimed: bool,
}

impl CharacterDispersion {
    pub fn from_config(config: &ConfigValue) -> Result<Self, ConfigError> {
        let params = config.req("player")?.get("character_dispersion_params");
        let number = |key: &str| -> Result<f32, ConfigError> {
            match params.and_then(|t| t.get(&format!("{key}_dispersion"))) {
                None => Ok(0.0),
                Some(value) => value.as_f32().ok_or_else(|| {
                    ConfigError(format!("`character_dispersion_params.{key}_dispersion` is not a number"))
                }),
            }
        };
        Ok(Self {
            idle: number("idle")?,
            idle_aim: number("idle_aim")?,
            walk: number("walk")?,
            walk_aim: number("walk_aim")?,
            run: number("run")?,
            jump: number("jump")?,
            crouch: number("crouch")?,
            crouch_aim: number("crouch_aim")?,
            crouch_walk: number("crouch_walk")?,
            crouch_walk_aim: number("crouch_walk_aim")?,
        })
    }

    /// The character part's target for `stance` with `weapon`'s
    /// multiplier (0x331560 and 0x331330, no skills or injuries).
    pub fn of(&self, stance: Stance, weapon: &Dispersion) -> f32 {
        let a = stance.aimed;
        let value = if stance.jumping {
            self.jump
        } else if stance.crouched {
            match (stance.moving, a) {
                (false, false) => self.crouch,
                (false, true) => self.crouch_aim,
                (true, false) => self.crouch_walk,
                (true, true) => self.crouch_walk_aim,
            }
        } else if stance.sprinting && stance.moving {
            self.run
        } else {
            match (stance.moving, a) {
                (false, false) => self.idle,
                (false, true) => self.idle_aim,
                (true, false) => self.walk,
                (true, true) => self.walk_aim,
            }
        };
        value * if a { weapon.aim } else { weapon.from_the_hip }
    }
}

/// The players' character dispersion ([`super::game_data`]), read once.
pub fn character_dispersion() -> &'static CharacterDispersion {
    static SHARED: std::sync::OnceLock<CharacterDispersion> = std::sync::OnceLock::new();
    SHARED.get_or_init(|| {
        let path = super::item_params::PLAYER_PATH;
        ConfigValue::parse(super::game_data::file(path))
            .and_then(|config| CharacterDispersion::from_config(&config))
            .expect("the player's config reads")
    })
}

impl WeaponParams {
    /// The weapon with the changes its `modifications` make to the keys the
    /// bots read from it: `parameters/reload_time`, `parameters/rounds_per_minute`,
    /// `parameters/chamber_a_round_time` and `dispersion/base_dispersion`, by
    /// the client's rule ([`super::modifications::apply`]). The damage keys are
    /// the clients' own to compute and are left; `reload_start_time` and
    /// `reload_end_time` are other keys, no modification reaches them. No
    /// modification gives the weapon as it is.
    pub fn with_mods(&self, modifications: &[(u32, bool)]) -> Self {
        if modifications.is_empty() {
            return *self;
        }
        let table = super::modifications::Modifications::shared();
        let changed = |path: &str, base: f32| table.modified(modifications, path, base);
        Self {
            reload_time: changed("parameters/reload_time", self.reload_time),
            rounds_per_minute: changed("parameters/rounds_per_minute", self.rounds_per_minute as f32)
                .round()
                .max(1.0) as u32,
            chamber_a_round_time: changed("parameters/chamber_a_round_time", self.chamber_a_round_time),
            dispersion: Dispersion {
                base: changed("dispersion/base_dispersion", self.dispersion.base),
                ..self.dispersion
            },
            ..*self
        }
    }

    /// Its first fire mode empties the magazine while the trigger is held.
    pub fn automatic(&self) -> bool {
        self.first_fire_mode < 0
    }

    /// From an item's config. A number the config does not have reads as 0,
    /// never as an error (`bullet_pierce` and `min_damage_amount` included):
    /// the client's reader (`weapon_damage_params` 0x332330, through
    /// `binary_config_value::operator[]` 0xF6B10, which has no end check)
    /// would walk off the table for a missing key, so every weapon that
    /// ships has them (PROVEN as code), and the server must not stop at
    /// start for a config that does not.
    pub fn from_config(
        dict_id: u16,
        name: &'static str,
        category: u8,
        config: &ConfigValue,
        clips: Option<&ClipLengths>,
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
        let mut reload_share = None;
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
                    | STATE_RELOAD_EMPTY) => {
                        reload_state = k;
                        reload_share = clips.and_then(|clips| two_clip_share(k, state, clips));
                    }
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
            movement_speed_modifier: number(Some(p), "movement_speed_modifier")?,
            reload_state,
            reload_share,
            has_chamber,
            show_time: number(Some(p), "show_time")?,
            hide_time: number(Some(p), "hide_time")?,
            aim_time: number(Some(p), "aim_time")?,
            bullet_damage: number(Some(p), "bullet_damage")?,
            max_damage_distance: number(Some(p), "max_damage_distance")?,
            min_damage_distance: number(Some(p), "min_damage_distance")?,
            bullet_pierce: number(Some(p), "bullet_pierce")?,
            min_damage_amount: number(Some(p), "min_damage_amount")?,
            stamina_damage: number(Some(p), "stamina_damage")?,
            stopping_power: number(Some(p), "stopping_power")?,
            top_recoil: range("top")?,
            left_recoil: range("left")?,
            right_recoil: range("right")?,
            recoil_power: number(recoil, "recoil_power")?,
            recoil_gains: (
                number(recoil, "compensation_proportional_gain")?,
                number(recoil, "compensation_integral_gain")?,
                number(recoil, "compensation_derivational_gain")?,
            ),
            dispersion: Dispersion::from_config(config.get("dispersion"))?,
        })
    }
}

/// The second clip of a two-clip reload `state` over its first, by the lengths `clips` has
/// ([`WeaponParams::reload_share`]); the clips are the state's `animations` (the item's own: the
/// first is the reload with a round left or one barrel, the second from empty or both barrels).
fn two_clip_share(kind: u16, state: &ConfigValue, clips: &ClipLengths) -> Option<f32> {
    if kind != STATE_RELOAD_BARRELS && kind != STATE_RELOAD_EMPTY {
        return None;
    }
    let names = state.get("animations")?.children();
    let length = |i: usize| clips.get(names.get(i)?.as_str()?);
    let (first, second) = (length(0)?, length(1)?);
    (first > 0.0 && second > 0.0).then(|| second / first)
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
        let row = WeaponParams::from_config(
            item.dict_id,
            &item.name,
            item.category,
            &config,
            data.clip_lengths(),
        )
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

/// A grenade's numbers, from the `data` block of its config
/// (`gameplay/items/weapons/rgd-5.options`): how long it lives once thrown
/// and how far its blast reaches. The match server's bots time their throws
/// and keep teammates out of the blast by these (`bots::reflex`, play 20).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GrenadeParams {
    /// `ttl_ms`: from the throw to the blast, ms.
    pub ttl_ms: u32,
    /// `splash_radius`: the blast's reach, metres.
    pub splash_radius: f32,
    /// `splash_max_damage` / `splash_min_damage`: at its middle and at its
    /// edge (shares of a body's health, before armour).
    pub splash_max_damage: f32,
    pub splash_min_damage: f32,
    /// `throw_power`.
    pub throw_power: f32,
    /// `explode_on_contact`.
    pub explode_on_contact: bool,
}

impl GrenadeParams {
    /// From an item's config (`data`).
    pub fn from_config(config: &ConfigValue) -> Result<Self, ConfigError> {
        let data = config.req("data")?;
        let number = |key: &str| -> Result<f32, ConfigError> {
            data.req(key)?
                .as_f32()
                .ok_or_else(|| ConfigError(format!("`data.{key}` is not a number")))
        };
        let ttl = number("ttl_ms")?;
        if ttl <= 0.0 {
            return Err(ConfigError("`data.ttl_ms` is not above 0".into()));
        }
        Ok(Self {
            ttl_ms: ttl.round() as u32,
            splash_radius: number("splash_radius")?,
            splash_max_damage: number("splash_max_damage")?,
            splash_min_damage: number("splash_min_damage")?,
            throw_power: number("throw_power")?,
            explode_on_contact: data
                .get("explode_on_contact")
                .and_then(ConfigValue::as_bool)
                .unwrap_or(false),
        })
    }
}

/// Every grenade of `data` (an item of `dictionary` whose config's
/// `parameters.type` is `grenade`), by name. A test reads a patched copy of
/// the data with this ([`grenades`] is the process's own).
pub fn derive_grenades(
    data: &super::game_data::GameData,
    dictionary: &'static Dictionary,
) -> Vec<(&'static str, GrenadeParams)> {
    dictionary
        .items
        .iter()
        .filter_map(|item| {
            let config = super::game_data::config(data, &item.cfg_name).ok()??;
            let kind = config.get("parameters")?.get("type")?.as_str()?;
            (kind == "grenade")
                .then(|| GrenadeParams::from_config(&config).ok())
                .flatten()
                .map(|params| (item.name.as_str(), params))
        })
        .collect()
}

/// Every grenade of the game data, by name, read once.
pub fn grenades() -> &'static [(&'static str, GrenadeParams)] {
    static SHARED: std::sync::OnceLock<Vec<(&'static str, GrenadeParams)>> =
        std::sync::OnceLock::new();
    SHARED.get_or_init(|| derive_grenades(super::game_data::shared(), Dictionary::shared()))
}

impl GrenadeParams {
    /// Whether the blast hurts: a fragmentation grenade. A grenade that does
    /// not (the smoke `rdg-m`: no splash damage) is a danger to no one.
    pub fn is_frag(&self) -> bool {
        self.splash_radius > 0.0 && self.splash_max_damage > 0.0
    }
}

/// Whether the item `name` is a grenade of any kind: an item whose config's
/// `parameters.type` is `grenade`, whether or not its `data` block reads as
/// [`GrenadeParams`] (a smoke grenade need not have a blast).
pub fn is_grenade_item(name: &str) -> bool {
    static SHARED: std::sync::OnceLock<Vec<&'static str>> = std::sync::OnceLock::new();
    SHARED
        .get_or_init(|| {
            let data = super::game_data::shared();
            Dictionary::shared()
                .items
                .iter()
                .filter(|item| {
                    super::game_data::config(data, &item.cfg_name)
                        .ok()
                        .flatten()
                        .and_then(|config| {
                            config
                                .get("parameters")?
                                .get("type")?
                                .as_str()
                                .map(|kind| kind == "grenade")
                        })
                        .unwrap_or(false)
                })
                .map(|item| item.name.as_str())
                .collect()
        })
        .contains(&name)
}

/// `stock` with the numbers of the `data` block of the grenade `name`'s config changed to `numbers`
/// (`(key, value)`, an integer key stays an integer): what a grenade patch does. For tests of what
/// reads a grenade's numbers from the game data.
#[cfg(any(test, feature = "fixtures"))]
pub fn with_grenade_patched(stock: &GameData, name: &str, numbers: &[(&str, f32)]) -> GameData {
    let item = Dictionary::shared()
        .item_named(name)
        .unwrap_or_else(|| panic!("no item {name}"));
    let mut config = super::game_data::config(stock, &item.cfg_name)
        .unwrap()
        .unwrap_or_else(|| panic!("no config {}", item.cfg_name));
    let ConfigValue::Named(root) = &mut config else {
        panic!("{} is not a named table", item.cfg_name);
    };
    let (_, data) = root
        .iter_mut()
        .find(|(key, _)| key == "data")
        .unwrap_or_else(|| panic!("{} has no data block", item.cfg_name));
    let ConfigValue::Named(entries) = data else {
        panic!("data is not a named table");
    };
    for &(key, new) in numbers {
        let (_, value) = entries
            .iter_mut()
            .find(|(k, _)| k == key)
            .unwrap_or_else(|| panic!("no data.{key}"));
        match value {
            ConfigValue::Integer(i) => *i = new as i32,
            ConfigValue::Float(f) => *f = new,
            other => panic!("data.{key} is {other:?}"),
        }
    }
    stock.laid_over(
        "the test data and a grenade patch",
        vec![(item.cfg_name.clone(), config.to_bytes().unwrap())],
    )
}

/// The grenade `name` (`items_dict`).
pub fn grenade_named(name: &str) -> Option<GrenadeParams> {
    grenades().iter().find(|(n, _)| *n == name).map(|(_, g)| *g)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v026g::item_params::item_params;

    /// The keys `with_mods` changes.
    const MODDED_KEYS: [&str; 4] = [
        "parameters/reload_time",
        "parameters/rounds_per_minute",
        "parameters/chamber_a_round_time",
        "dispersion/base_dispersion",
    ];

    #[test]
    fn a_weapons_mods_make_the_numbers_the_bots_read() {
        let ak = weapon_named("ak_74u").unwrap();
        assert_eq!(ak.with_mods(&[]), *ak);
        // 2044: the reload time -15 % (3.167 s to 2.692 s); nothing else moves.
        let fast = ak.with_mods(&[(2044, false)]);
        assert!((fast.reload_time - ak.reload_time * 0.85).abs() < 1e-5);
        assert_eq!(
            WeaponParams {
                reload_time: ak.reload_time,
                ..fast
            },
            *ak
        );
        // 2034: the rate of fire +10 %; 2024: the base dispersion -10 %.
        let rate = ak.with_mods(&[(2034, false)]);
        assert_eq!(
            rate.rounds_per_minute,
            (ak.rounds_per_minute as f32 * 1.10).round() as u32
        );
        let rifled = ak.with_mods(&[(2024, false)]);
        assert!((rifled.dispersion.base - ak.dispersion.base * 0.90).abs() < 1e-5);
        // 2054: the bolt's chamber time -15 %.
        let mosin = weapon_named("mosin_nagant").unwrap();
        assert!(mosin.chamber_a_round_time > 0.0);
        let bolt = mosin.with_mods(&[(2054, false)]);
        assert!((bolt.chamber_a_round_time - mosin.chamber_a_round_time * 0.85).abs() < 1e-5);
        // Aim time (2074) and recoil (2084) change no key of a weapon, and the
        // damage keys are the clients' own to compute.
        for id in [2074, 2084, 2000, 2010, 2110] {
            assert_eq!(ak.with_mods(&[(id, false)]), *ak, "{id}");
        }
    }

    /// `with_mods` counts only the item's own mods: no weapon's default
    /// modifications touch a key it reads, so that leaves nothing out.
    #[test]
    fn no_weapon_has_a_default_modification_on_a_key_the_bots_read() {
        let table = super::super::modifications::Modifications::shared();
        for w in all() {
            let defaults = item_params(w.dict_id).unwrap().default_modifications;
            for row in defaults.iter().filter_map(|&id| table.modification(id)) {
                for modifier in &row.modifiers {
                    assert!(
                        !MODDED_KEYS.contains(&modifier.path.as_str()),
                        "{}: {}",
                        w.name,
                        modifier.path
                    );
                }
            }
        }
    }

    #[test]
    fn a_weapons_own_speed_modifier_is_read() {
        assert_eq!(weapon_named("ak_74u").unwrap().movement_speed_modifier, 0.05);
        assert_eq!(weapon_named("rem_700").unwrap().movement_speed_modifier, -0.05);
    }

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

    #[test]
    fn the_bots_grenade_is_read_from_its_config() {
        // The RGD-5 (`loadouts::BOT_GRENADE`): 4 s from the throw, a 10 m
        // blast, not on contact.
        let rgd = grenade_named("rgd-5").unwrap();
        assert_eq!(rgd.ttl_ms, 4_000);
        assert_eq!(rgd.splash_radius, 10.0);
        assert!(rgd.splash_max_damage > rgd.splash_min_damage);
        assert!(!rgd.explode_on_contact);
        assert_eq!(grenade_named("rgd-5"), Some(rgd));
        assert_eq!(grenade_named("no such grenade"), None);
    }

    /// A frag hurts and a smoke does not; both are grenade items.
    #[test]
    fn the_grenades_are_told_apart_by_their_blast() {
        assert!(grenade_named("rgd-5").unwrap().is_frag());
        assert!(is_grenade_item("rgd-5") && is_grenade_item("rdg-m"));
        assert!(!is_grenade_item("uzi") && !is_grenade_item("no such item"));
        if let Some(smoke) = grenade_named("rdg-m") {
            assert!(!smoke.is_frag(), "{smoke:?}");
        }
    }

    /// What a grenade patch changes is what is read: the fuse, the blast and the throw speed of
    /// a patched copy of the data; the process's own data is untouched.
    #[test]
    fn a_grenade_patch_is_what_is_read() {
        let stock = GameData::test_archive();
        let patched = with_grenade_patched(
            &stock,
            "rgd-5",
            &[("ttl_ms", 3_000.0), ("splash_radius", 8.0), ("throw_power", 20.0)],
        );
        let rows = derive_grenades(&patched, Dictionary::shared());
        let rgd = rows.iter().find(|(name, _)| *name == "rgd-5").unwrap().1;
        assert_eq!(
            (rgd.ttl_ms, rgd.splash_radius, rgd.throw_power),
            (3_000, 8.0, 20.0)
        );
        let before = derive_grenades(&stock, Dictionary::shared());
        assert_eq!(before.iter().find(|(name, _)| *name == "rgd-5").unwrap().1, grenade_named("rgd-5").unwrap());
        assert_eq!(grenade_named("rgd-5").unwrap().ttl_ms, 4_000);
    }

    /// The `dispersion` blocks and the player's character dispersion, as
    /// the RE of 0.26g's shot read them (`player_bots_weapons.md` §9).
    #[test]
    fn the_dispersion_is_read_from_the_configs() {
        let uzi = weapon_named("uzi").unwrap().dispersion;
        assert_eq!(
            (uzi.base, uzi.one_shot, uzi.speed_of_aiming, uzi.growth_speed, uzi.max),
            (0.3, 1.5, 15.0, 20.0, 20.0)
        );
        assert_eq!((uzi.from_the_hip, uzi.aim), (1.0, 1.0));
        let mosin = weapon_named("mosin_nagant").unwrap().dispersion;
        assert_eq!((mosin.base, mosin.from_the_hip, mosin.aim), (0.1, 2.0, 1.0));
        let c = character_dispersion();
        assert_eq!((c.idle, c.walk, c.run, c.jump), (5.0, 10.0, 20.0, 20.0));
        assert_eq!((c.crouch, c.crouch_walk, c.walk_aim), (4.0, 8.0, 10.0));
        // Strafing from the hip: 10 of the character and the base; fully
        // aimed, the base alone.
        let strafing = Stance {
            moving: true,
            ..Stance::default()
        };
        let character = c.of(strafing, &uzi);
        assert_eq!(uzi.cone_deg(character, 0.0, false), 10.3);
        assert_eq!(uzi.cone_deg(character, 3.0, true), 0.3);
        assert_eq!(c.of(strafing, &mosin), 20.0);
    }

    /// The test of "read live" at the layer that reads: a weapon config
    /// patched in a copy of the data (the shapes of a balance patch) comes
    /// out of [`derive`] with the patched numbers, every other weapon is as it
    /// was, and the process's own table ([`all`]) is not the copy's.
    #[test]
    fn a_patched_weapon_config_reaches_the_derived_table() {
        use super::fixtures::patched_item;
        let dictionary = Dictionary::shared();
        let before = *weapon_named("ak_74u").unwrap();
        let number = ConfigValue::Float;
        let data = patched_item(
            crate::v026g::game_data::shared(),
            dictionary,
            "ak_74u",
            &[
                (&["parameters", "reload_time"], number(2.0)),
                (&["parameters", "magazine_capacity"], ConfigValue::Integer(45)),
                (&["parameters", "rounds_per_minute"], number(700.0)),
                (&["parameters", "aim_time"], number(0.4)),
                (&["parameters", "show_time"], number(0.5)),
                (&["recoil", "min_top_recoil"], number(3.0)),
                (&["parameters", "type"], ConfigValue::String("wpn_new".into())),
            ],
        );
        let table = derive(&data, dictionary).unwrap();
        let ak = table.iter().find(|w| w.name == "ak_74u").unwrap();
        assert_eq!(
            (ak.reload_time, ak.magazine_capacity, ak.rounds_per_minute),
            (2.0, 45, 700)
        );
        assert_eq!((ak.aim_time, ak.show_time, ak.top_recoil.0), (0.4, 0.5, 3.0));
        assert_eq!(ak.kind, "wpn_new");
        // What the patch left alone is as it was.
        assert_eq!(
            (ak.dict_id, ak.category, ak.hide_time, ak.reload_state),
            (before.dict_id, before.category, before.hide_time, before.reload_state)
        );
        assert_eq!(table.len(), all().len());
        for w in table.iter().filter(|w| w.name != "ak_74u") {
            assert_eq!(Some(w), weapon_named(w.name), "{}", w.name);
        }
        // The copy is not the process's data.
        assert_ne!(before.reload_time, 2.0);
        assert_ne!(before.magazine_capacity, 45);
        assert_eq!(weapon_named("ak_74u"), Some(&before));
    }

    /// A weapon config with none of the damage keys reads (as zeros) rather
    /// than failing, so `all()` cannot panic on a patch that lacks them.
    #[test]
    fn a_weapon_config_without_the_damage_keys_reads() {
        let named = |entries: Vec<(&str, ConfigValue)>| {
            ConfigValue::Named(entries.into_iter().map(|(k, v)| (k.to_owned(), v)).collect())
        };
        let config = named(vec![(
            "parameters",
            named(vec![
                ("type", ConfigValue::String("wpn_aslt".into())),
                ("fire_queue_types", ConfigValue::Indexed(vec![ConfigValue::Integer(-1)])),
                ("rounds_per_minute", ConfigValue::Float(600.0)),
            ]),
        )]);
        let w = WeaponParams::from_config(1, "bare", 8, &config, None).unwrap();
        assert_eq!(
            (w.bullet_damage, w.bullet_pierce, w.min_damage_amount, w.max_damage_distance),
            (0.0, 0.0, 0.0, 0.0)
        );
        assert_eq!(w.rounds_per_minute, 600);
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
        // The damage keys every weapon has (the client needs them).
        for w in &all {
            assert!(
                w.bullet_damage > 0.0 && w.bullet_pierce > 0.0 && w.min_damage_amount > 0.0,
                "{} {w:?}",
                w.name
            );
        }
    }

    /// The names of the clips the reload state of the weapon `name` plays (the item's own).
    fn reload_clip_names(name: &str) -> Vec<String> {
        let item = Dictionary::shared().item_named(name).unwrap();
        let config = super::super::game_data::config(
            super::super::game_data::shared(),
            item.cfg_name.as_str(),
        )
        .unwrap()
        .unwrap();
        config
            .get("states")
            .and_then(|states| states.get("reload"))
            .and_then(|state| state.get("animations"))
            .unwrap()
            .children()
            .iter()
            .map(|clip| clip.as_str().unwrap().to_owned())
            .collect()
    }

    /// W9: a two-clip reload's share is its second clip's length over its first's, by the clip lengths the game
    /// data has; without them, or for a reload of one clip, there is none.
    #[test]
    fn a_two_clip_reloads_share_is_the_second_clips_length_over_the_first() {
        let data = super::super::game_data::shared();
        let mut clips = ClipLengths::default();
        for (name, frames) in [
            ("tt_33", [65.0, 125.0]),
            ("toz_34", [80.0, 110.0]),
            ("toz_66", [97.0, 97.0]),
            ("uzi", [95.0, 95.0]),
        ] {
            let names = reload_clip_names(name);
            for (clip, frames) in names.iter().zip(frames) {
                clips.insert(clip.clone(), frames);
            }
        }
        let with = data.laid_over("clips", Vec::new()).with_clip_lengths(clips);
        let table = derive(&with, Dictionary::shared()).unwrap();
        let share = |name: &str| table.iter().find(|w| w.name == name).unwrap().reload_share;
        assert_eq!(share("tt_33"), Some(125.0 / 65.0));
        assert_eq!(share("toz_34"), Some(110.0 / 80.0));
        assert_eq!(share("toz_66"), Some(1.0));
        // One clip (a magazine reload, state 303): no share, whatever the lengths.
        assert_eq!(share("uzi"), None);
        // A weapon whose clips have no length: none.
        assert_eq!(share("izh_27_lancaster"), None);
        // Data made without the lengths: none for any weapon.
        let bare = derive(data, Dictionary::shared()).unwrap();
        assert!(bare.iter().all(|w| w.reload_share.is_none()));
        // A zero-length clip gives no share (nothing to scale by).
        let mut zero = ClipLengths::default();
        for (clip, frames) in reload_clip_names("tt_33").iter().zip([0.0, 125.0]) {
            zero.insert(clip.clone(), frames);
        }
        let with = data.laid_over("zero", Vec::new()).with_clip_lengths(zero);
        let table = derive(&with, Dictionary::shared()).unwrap();
        assert_eq!(table.iter().find(|w| w.name == "tt_33").unwrap().reload_share, None);
    }
}
