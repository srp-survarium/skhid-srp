//! What a bullet does to a body, as the 0.26g client computes it, from the
//! game data ([`super::game_data`]): the ammunition's multipliers, the
//! armour items' protection per hit box and the human body's pools, all read
//! from their own configs (`gameplay/items/weapons/ammo/*.options`,
//! `gameplay/items/armour/*.options`, `gameplay/hit_params/
//! human_hit_params.options`), never from a table kept in code
//! (`AGENTS.md`). The match server's bench stand-in uses it to hurt its
//! bodies as the game does (`match-server` `bots::tests::standin_damage`);
//! the rule itself is the client's, which every client of a match runs.
//!
//! Marks: **PROVEN** = read in the 0.26g exe (image base 0x10000, the
//! addresses are its virtual addresses), **LIKELY** = 0.21d's names and
//! disassembly or the data, not fully read in 0.26g, **GUESS** = an
//! assumption. The whole chain, with its other steps, is
//! `weapon-balance/DAMAGE-FORMULA.md` §1 of the owner's notes.
//!
//! 1. A bullet starts with `bullet_damage × k_damage` and
//!    `bullet_pierce × k_arp` (bullet ctor 0x32BB38..0x32BB7B; weapon
//!    fields `weapon_damage_params` ctor 0x332330, ammo fields
//!    `weapon_ammunition` ctor 0x332440). PROVEN.
//! 2. Both fall by one factor with the bullet's speed: full up to
//!    `max_damage_distance × k_max_damage_distance`, down to
//!    `min_damage_amount × k_min_damage_amount` of it at `min_damage_distance
//!    × k_min_damage_distance`, flat after (`compute_damage_speed` 0x32C520,
//!    `update_bullet_position` 0x32F044..0x32F174); the speed is
//!    `v0 (1 − k t)` (`compute_parabolic_velocity` 0x32C950; gravity left
//!    out: LIKELY), so after `d` metres it is `v0 √(1 − 2 k d / v0)`, and
//!    the flight ends at `t = 1 / (1.1 k)` (0x32C523..0x32C55B). PROVEN.
//! 3. A hit box's damage is `amount × (1 − clamp(armor − arp, 0, 1))`,
//!    `armor` the box's `ranged` armour (`body_part_parameters::
//!    hit_by_type` 0x32A058..0x32A0AC). PROVEN. The armour of a box is the
//!    worn items' `hit_params/<box>/hit_types/ranged/armor`, the largest of
//!    those covering it (`player_parameters_modifyer`: LIKELY).
//! 4. The box feeds its pools: each `bdb_coeff` (pool, c) takes `c ×
//!    amount` from the pool with no armour of its own (arp 0, the pool's
//!    armour is 0: `hit_type_parameters::apply_damage` 0x33FA80). A pool
//!    that lists a death threshold (`thresholds[].value 0` affecting 0:
//!    body, head, pain, infection) kills at 0. PROVEN (code), the death
//!    threshold LIKELY.
//! 5. A grenade's blast ([`SplashParams`], [`BodyParams::hits_taken`]): in
//!    the world's blast (0x339630) every player within `splash_radius` of the
//!    blast takes, linearly in its distance `d` from the blast (the origin of
//!    its transform), `min + (max − min) (R − d) / R`; for each body part the
//!    grenade's `splash_affected_bodies` lists (indices into the human body's
//!    part list) that amount, times what a ray from the part to the blast
//!    keeps through walls ([`wall_share`], 0x3393f0, the power
//!    `splash_pierce`), is **one hit of the grenade's `splash_damage_type`
//!    with armour piercing 1.0** (the very argument layout of the bullet's
//!    hit call, 0x32DA0D..0x32DA3A: PROVEN), so no armour holds against it
//!    (every armour is below 1). A pool is hit directly or through the box's
//!    `bdb_coeff`s, and a pool takes from one initiator only the largest of
//!    its hits (`hit_by_type`'s list, DAMAGE-FORMULA step 15f: PROVEN code,
//!    that the grenade is one initiator LIKELY). That the part indices are
//!    the human body's own: LIKELY.

use std::sync::OnceLock;

use binary_config_parser::tree::{ConfigError, ConfigValue};

use super::dictionary::Dictionary;
use super::game_data::{GameData, GameDataError};
use super::weapons::WeaponParams;

/// The human body's hit parameters, `ranged` hits (`gameplay/hit_params`).
pub const HUMAN_HIT_PARAMS: &str = "gameplay/hit_params/human_hit_params.options";

/// The prefix of the armour items' configs.
const ARMOUR_PATH: &str = "gameplay/items/armour/";

/// An ammunition's multipliers and flight (`weapon_ammunition`, ctor
/// 0x332440: `distance` +0, `k_damage` +8, `k_max_damage_distance` +0x18,
/// `k_min_damage_distance` +0x1C, `k_min_damage_amount` +0x20, `k_arp`
/// +0x24, `air_resistance` +0x28, `muzzle_speed` +0x3C, `buck_shot` +0x44:
/// PROVEN). The lobby's `parameters/damage` and `parameters/arp` are not
/// read by the damage code (PROVEN).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AmmoParams {
    pub dict_id: u16,
    pub name: &'static str,
    pub k_damage: f32,
    pub k_arp: f32,
    pub k_min_damage_amount: f32,
    pub k_max_damage_distance: f32,
    pub k_min_damage_distance: f32,
    pub muzzle_speed: f32,
    pub air_resistance: f32,
    /// The flight's length at most, metres.
    pub distance: f32,
    /// Bullets a shot makes (a shotgun's pellets); 1 otherwise.
    pub buck_shot: u32,
    /// The pellets' cone, degrees (full angle; `buck_dispersion`, ammo +0x2A8 of `weapon_core::instant_fire`
    /// 0x2E23A0, PROVEN): each pellet turns the shot's direction again by `frac(N(0, 0.5)) x cone / 2`
    /// (`get_dispersed_buckshot_direction` 0x2E2120). 0 for a plain bullet.
    pub buck_dispersion: f32,
}

impl AmmoParams {
    /// From an item's config; `None` for a config that is not an ammunition
    /// (it has no `data/k_damage`).
    pub fn from_config(
        dict_id: u16,
        name: &'static str,
        config: &ConfigValue,
    ) -> Result<Option<Self>, ConfigError> {
        let Some(data) = config.get("data") else {
            return Ok(None);
        };
        if data.get("k_damage").is_none() {
            return Ok(None);
        }
        let number = |key: &str, absent: f32| -> Result<f32, ConfigError> {
            match data.get(key) {
                None => Ok(absent),
                Some(value) => value
                    .as_f32()
                    .ok_or_else(|| ConfigError(format!("`data.{key}` is not a number"))),
            }
        };
        Ok(Some(Self {
            dict_id,
            name,
            k_damage: number("k_damage", 1.0)?,
            k_arp: number("k_arp", 1.0)?,
            k_min_damage_amount: number("k_min_damage_amount", 1.0)?,
            k_max_damage_distance: number("k_max_damage_distance", 1.0)?,
            k_min_damage_distance: number("k_min_damage_distance", 1.0)?,
            muzzle_speed: number("muzzle_speed", 0.0)?,
            air_resistance: number("air_resistance", 0.0)?,
            distance: number("distance", 0.0)?,
            buck_shot: number("buck_shot", 1.0)?.round().max(1.0) as u32,
            buck_dispersion: number("buck_dispersion", 0.0)?.max(0.0),
        }))
    }

    /// The bullet's speed after `d` metres (`v0 √(1 − 2 k d / v0)`, the
    /// flight capped at its end).
    pub fn speed_at(&self, d: f32) -> f32 {
        let (v0, k) = (self.muzzle_speed, self.air_resistance);
        if k <= 0.0 {
            return v0;
        }
        let d = d.min(self.reach());
        v0 * (1.0 - 2.0 * k * d / v0).max(0.0).sqrt()
    }

    /// Where the flight ends, metres: `t = 1 / (1.1 k)`.
    pub fn reach(&self) -> f32 {
        let (v0, k) = (self.muzzle_speed, self.air_resistance);
        if k <= 0.0 {
            return f32::MAX;
        }
        let t = 1.0 / (1.1 * k);
        v0 * (t - k * t * t / 2.0)
    }
}

/// One bullet's damage and armour piercing at some distance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bullet {
    pub damage: f32,
    pub arp: f32,
}

/// What a bullet of `ammo` fired from `weapon` carries after `distance`
/// metres (steps 1 and 2 of the module's rule).
pub fn bullet_at(weapon: &WeaponParams, ammo: &AmmoParams, distance: f32) -> Bullet {
    let hi = weapon.bullet_damage * ammo.k_damage;
    let ahi = weapon.bullet_pierce * ammo.k_arp;
    let low = weapon.min_damage_amount * ammo.k_min_damage_amount;
    let reach = ammo.reach();
    let d_full = (weapon.max_damage_distance * ammo.k_max_damage_distance).min(reach);
    let d_min = (weapon.min_damage_distance * ammo.k_min_damage_distance).min(reach);
    let (v_full, v_min, v) = (ammo.speed_at(d_full), ammo.speed_at(d_min), ammo.speed_at(distance));
    let s = if v_full == v_min {
        1.0
    } else {
        ((v - v_min) / (v_full - v_min)).clamp(0.0, 1.0)
    };
    let along = low + (1.0 - low) * s;
    Bullet {
        damage: hi * along,
        arp: ahi * along,
    }
}

/// A hit box's damage after its armour (step 3 of the module's rule).
pub fn after_armour(amount: f32, arp: f32, armor: f32) -> f32 {
    amount * (1.0 - (armor - arp).clamp(0.0, 1.0))
}

/// An armour item's `ranged` armour per hit box.
#[derive(Clone, Debug, PartialEq)]
pub struct ArmourParams {
    pub dict_id: u16,
    pub name: &'static str,
    pub boxes: Vec<(String, f32)>,
}

impl ArmourParams {
    /// From an item's config; `None` for one with no `hit_params`.
    pub fn from_config(
        dict_id: u16,
        name: &'static str,
        config: &ConfigValue,
    ) -> Result<Option<Self>, ConfigError> {
        let Some(hit_params) = config.get("hit_params") else {
            return Ok(None);
        };
        let mut boxes = Vec::new();
        for (hit_box, value) in hit_params.entries() {
            let Some(armor) = value
                .get("hit_types")
                .and_then(|types| types.get("ranged"))
                .and_then(|ranged| ranged.get("armor"))
            else {
                continue;
            };
            let armor = armor
                .as_f32()
                .ok_or_else(|| ConfigError(format!("`hit_params.{hit_box}` armor is not a number")))?;
            boxes.push((hit_box.clone(), armor));
        }
        Ok(Some(Self {
            dict_id,
            name,
            boxes,
        }))
    }

    /// The armour of `hit_box` (0: the item does not cover it).
    pub fn of(&self, hit_box: &str) -> f32 {
        self.boxes
            .iter()
            .find(|(name, _)| name == hit_box)
            .map_or(0.0, |&(_, armor)| armor)
    }
}

/// One part of the human body (`human_hit_params`): a pool with a
/// `health`, or a hit box whose `ranged` hits feed pools.
#[derive(Clone, Debug, PartialEq)]
pub struct BodyPart {
    pub name: String,
    pub health: f32,
    /// A pool at 0 kills (`thresholds` with value 0 affecting 0).
    pub lethal: bool,
    /// A hit box a bullet can meet (`can_be_assigned`).
    pub hit_box: bool,
    /// The pools its `ranged` hits take `coefficient × amount` from.
    pub feeds: Vec<(String, f32)>,
    /// Its own `ranged` armour (0 on every part of 0.26g's body).
    pub armor: f32,
    /// Its regeneration (`body_part::regenerate` 0x32A4B0): `timeout_s`
    /// after its last hit it gains `speed` of its health a second, up to
    /// `threshold` of it. PROVEN as code; the data's reading LIKELY.
    pub regeneration_timeout: f32,
    pub regeneration_speed: f32,
    pub regeneration_threshold: f32,
}

/// The human body.
#[derive(Clone, Debug, PartialEq)]
pub struct BodyParams {
    pub parts: Vec<BodyPart>,
}

impl BodyParams {
    pub fn from_config(config: &ConfigValue) -> Result<Self, ConfigError> {
        let list = config.req("hit_params")?;
        let mut parts = Vec::new();
        for part in list.children() {
            let number = |key: &str| -> Result<f32, ConfigError> {
                part.req(key)?
                    .as_f32()
                    .ok_or_else(|| ConfigError(format!("`{key}` is not a number")))
            };
            let name = part.str_at("name")?.to_owned();
            let ranged = part.get("hit_types").and_then(|types| types.get("ranged"));
            let feeds = ranged
                .and_then(|ranged| ranged.get("bdb_coeff"))
                .map(|coefficients| {
                    coefficients
                        .entries()
                        .iter()
                        .filter_map(|(pool, c)| Some((pool.clone(), c.as_f32()?)))
                        .collect()
                })
                .unwrap_or_default();
            let lethal = part
                .get("thresholds")
                .map(|thresholds| {
                    thresholds.children().iter().any(|threshold| {
                        threshold.get("value").and_then(ConfigValue::as_f32) == Some(0.0)
                            && threshold
                                .get("affects")
                                .is_some_and(|a| a.children().iter().any(|v| v.as_i32() == Some(0)))
                    })
                })
                .unwrap_or(false);
            parts.push(BodyPart {
                name,
                health: number("health")?,
                lethal,
                hit_box: part
                    .get("can_be_assigned")
                    .and_then(ConfigValue::as_bool)
                    .unwrap_or(false),
                feeds,
                armor: ranged
                    .and_then(|ranged| ranged.get("armor"))
                    .and_then(ConfigValue::as_f32)
                    .unwrap_or(0.0),
                regeneration_timeout: number("regeneration_timeout").unwrap_or(0.0),
                regeneration_speed: number("regeneration_speed").unwrap_or(0.0),
                regeneration_threshold: number("regeneration_threshold").unwrap_or(0.0),
            });
        }
        Ok(Self { parts })
    }

    /// The part named `name`.
    pub fn part(&self, name: &str) -> Option<&BodyPart> {
        self.parts.iter().find(|part| part.name == name)
    }
}

/// Every ammunition of `dictionary`, by `dict_id`.
pub fn derive_ammo(
    data: &GameData,
    dictionary: &'static Dictionary,
) -> Result<Vec<AmmoParams>, GameDataError> {
    let mut rows = Vec::new();
    for item in &dictionary.items {
        let path = item.cfg_name.as_str();
        let Some(config) = super::game_data::config(data, path)? else {
            continue;
        };
        let row = AmmoParams::from_config(item.dict_id, &item.name, &config)
            .map_err(|cause| GameDataError(format!("{path}: {}", cause.0)))?;
        rows.extend(row);
    }
    rows.sort_by_key(|row| row.dict_id);
    Ok(rows)
}

/// Every armour item of `dictionary` (a config under `gameplay/items/
/// armour/` with `hit_params`), by `dict_id`.
pub fn derive_armour(
    data: &GameData,
    dictionary: &'static Dictionary,
) -> Result<Vec<ArmourParams>, GameDataError> {
    let mut rows = Vec::new();
    for item in &dictionary.items {
        let path = item.cfg_name.as_str();
        if !path.starts_with(ARMOUR_PATH) {
            continue;
        }
        let Some(config) = super::game_data::config(data, path)? else {
            continue;
        };
        let row = ArmourParams::from_config(item.dict_id, &item.name, &config)
            .map_err(|cause| GameDataError(format!("{path}: {}", cause.0)))?;
        rows.extend(row);
    }
    rows.sort_by_key(|row| row.dict_id);
    Ok(rows)
}

/// What a wall keeps of a blast or a bullet of power `power` (a bullet's
/// `arp`, a blast's `splash_pierce`): `1 − (1 − clamp(power / resistance − 1,
/// 0, 1))²`; a wall of resistance 0 keeps all (`try_pierce` 0x32D960 for
/// bullets, 0x3393f0 for blasts: PROVEN).
pub fn wall_share(power: f32, resistance: f32) -> f32 {
    if resistance == 0.0 {
        return 1.0;
    }
    let c = (power / resistance - 1.0).clamp(0.0, 1.0);
    1.0 - (1.0 - c) * (1.0 - c)
}

/// A grenade's blast: `data/splash_*` of its config (the reader of the blast,
/// `splash_parameters` 0x30A350).
#[derive(Clone, Debug, PartialEq)]
pub struct SplashParams {
    pub name: &'static str,
    /// `splash_radius`, metres.
    pub radius: f32,
    /// `splash_max_damage` at the blast, `splash_min_damage` at the radius:
    /// shares of a pool's health, before the pools' own coefficients.
    pub max_damage: f32,
    pub min_damage: f32,
    /// `splash_pierce`: the power the blast has to get through walls.
    pub pierce: f32,
    /// `splash_damage_type` (`explosive`).
    pub damage_type: String,
    /// `splash_affected_bodies`: indices into [`BodyParams::parts`].
    pub affected: Vec<usize>,
}

impl SplashParams {
    /// From an item's config; `None` for one with no `data/splash_radius`.
    pub fn from_config(
        name: &'static str,
        config: &ConfigValue,
    ) -> Result<Option<Self>, ConfigError> {
        let Some(data) = config.get("data") else {
            return Ok(None);
        };
        if data.get("splash_radius").is_none() {
            return Ok(None);
        }
        let number = |key: &str| -> Result<f32, ConfigError> {
            data.req(key)?
                .as_f32()
                .ok_or_else(|| ConfigError(format!("`data.{key}` is not a number")))
        };
        let affected = data
            .get("splash_affected_bodies")
            .map(|list| {
                list.children()
                    .iter()
                    .map(|v| {
                        v.as_i32()
                            .and_then(|i| usize::try_from(i).ok())
                            .ok_or_else(|| {
                                ConfigError("`data.splash_affected_bodies` holds a non-index".into())
                            })
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?
            .unwrap_or_default();
        Ok(Some(Self {
            name,
            radius: number("splash_radius")?,
            max_damage: number("splash_max_damage")?,
            min_damage: number("splash_min_damage")?,
            pierce: data
                .get("splash_pierce")
                .and_then(ConfigValue::as_f32)
                .unwrap_or(0.0),
            damage_type: data
                .get("splash_damage_type")
                .and_then(ConfigValue::as_str)
                .unwrap_or("explosive")
                .to_owned(),
            affected,
        }))
    }

    /// The amount a player `distance` metres from the blast takes before
    /// walls; `None` beyond the radius.
    pub fn amount_at(&self, distance: f32) -> Option<f32> {
        let f = (self.radius - distance) / self.radius;
        (f >= 0.0).then_some(self.min_damage + (self.max_damage - self.min_damage) * f)
    }
}

impl BodyParams {
    /// The pools that hits on body parts take, as `hit_by_type` and
    /// `apply_damage` work them for one initiator: `hits` are `(part index,
    /// amount)`; a hit on a pool takes the amount from it, a hit on a box
    /// takes `coefficient × amount` after the box's armour from each pool it
    /// feeds (`armor(box)` against `arp`), and a pool takes only the largest
    /// of the hits it gets from the one initiator. Returns `(pool, damage)`.
    pub fn hits_taken(
        &self,
        hits: &[(usize, f32)],
        arp: f32,
        armor: &dyn Fn(&str) -> f32,
    ) -> Vec<(String, f32)> {
        let mut taken: Vec<(String, f32)> = Vec::new();
        let mut take = |pool: &str, amount: f32| match taken.iter_mut().find(|(name, _)| name == pool) {
            Some((_, most)) => *most = most.max(amount),
            None => taken.push((pool.to_owned(), amount)),
        };
        for &(index, amount) in hits {
            let Some(part) = self.parts.get(index) else {
                continue;
            };
            let amount = after_armour(amount, arp, armor(&part.name).max(part.armor));
            if part.feeds.is_empty() && part.health > 0.0 {
                take(&part.name, amount);
            }
            for (pool, c) in &part.feeds {
                take(pool, c * amount);
            }
        }
        taken
    }
}

/// Every ammunition of the game data, read once.
pub fn ammo() -> &'static [AmmoParams] {
    static SHARED: OnceLock<Vec<AmmoParams>> = OnceLock::new();
    SHARED.get_or_init(|| {
        derive_ammo(super::game_data::shared(), Dictionary::shared())
            .expect("the ammunition configs read")
    })
}

/// The ammunition named `name` (`items_dict`).
pub fn ammo_named(name: &str) -> Option<&'static AmmoParams> {
    ammo().iter().find(|ammo| ammo.name == name)
}

/// Every armour item of the game data, read once.
pub fn armour() -> &'static [ArmourParams] {
    static SHARED: OnceLock<Vec<ArmourParams>> = OnceLock::new();
    SHARED.get_or_init(|| {
        derive_armour(super::game_data::shared(), Dictionary::shared())
            .expect("the armour configs read")
    })
}

/// The armour item named `name` (`items_dict`).
pub fn armour_named(name: &str) -> Option<&'static ArmourParams> {
    armour().iter().find(|item| item.name == name)
}

/// The blast of the grenade named `name` (`items_dict`), read once for all.
pub fn splash_named(name: &str) -> Option<&'static SplashParams> {
    static SHARED: OnceLock<Vec<SplashParams>> = OnceLock::new();
    SHARED
        .get_or_init(|| {
            let data = super::game_data::shared();
            Dictionary::shared()
                .items
                .iter()
                .filter_map(|item| {
                    let config = super::game_data::config(data, &item.cfg_name).ok()??;
                    SplashParams::from_config(&item.name, &config).ok()?
                })
                .collect()
        })
        .iter()
        .find(|splash| splash.name == name)
}

/// The human body of the game data, read once.
pub fn human() -> &'static BodyParams {
    static SHARED: OnceLock<BodyParams> = OnceLock::new();
    SHARED.get_or_init(|| {
        ConfigValue::parse(super::game_data::file(HUMAN_HIT_PARAMS))
            .and_then(|config| BodyParams::from_config(&config))
            .expect("the human body's hit parameters read")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v026g::weapons::weapon_named;

    fn near(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    /// The human body of 0.26g: body 1.0, head 0.4 and pain 1.0 kill; a
    /// torso box feeds body and pain, a brain box head and pain, the arm
    /// boxes the hand and pain at 0.8 / 1, the hips and feet the leg and
    /// pain at 0.9 / 0.8; no part has an armour of its own
    /// (`DAMAGE-FORMULA.md` §2.1).
    #[test]
    fn the_human_body_is_read_from_its_config() {
        let body = human();
        for (pool, health) in [("body", 1.0), ("head", 0.4), ("pain", 1.0)] {
            let part = body.part(pool).unwrap();
            assert!(near(part.health, health), "{pool}");
            assert!(part.lethal && !part.hit_box, "{pool}");
        }
        // Regeneration: body and head 0.03 a second up to half, pain 0.10 up
        // to full, all 8 s after their last hit (`DAMAGE-FORMULA.md` step 17).
        for (pool, speed, threshold) in [("body", 0.03, 0.5), ("head", 0.03, 0.5), ("pain", 0.10, 1.0)] {
            let part = body.part(pool).unwrap();
            assert!(near(part.regeneration_speed, speed), "{pool}");
            assert!(near(part.regeneration_threshold, threshold), "{pool}");
            assert!(near(part.regeneration_timeout, 8.0), "{pool}");
        }
        assert!(!body.part("right_leg").unwrap().lethal);
        assert!(!body.part("right_hand").unwrap().lethal);
        let feeds = |hit_box: &str| {
            let part = body.part(hit_box).unwrap();
            assert!(part.hit_box && part.armor == 0.0, "{hit_box}");
            let mut feeds = part.feeds.clone();
            feeds.sort_by(|a, b| a.0.cmp(&b.0));
            feeds
        };
        let pair = |a: (&str, f32), b: (&str, f32)| vec![(a.0.to_owned(), a.1), (b.0.to_owned(), b.1)];
        assert_eq!(feeds("front"), pair(("body", 1.0), ("pain", 1.0)));
        assert_eq!(feeds("back"), pair(("body", 1.0), ("pain", 1.0)));
        assert_eq!(feeds("brain"), pair(("head", 1.0), ("pain", 1.0)));
        assert_eq!(feeds("face"), pair(("head", 1.0), ("pain", 1.0)));
        // A limb's box takes the limb's own pool (its side) and pain.
        let limb = |hit_box: &str, pool: &str, pain: f32| {
            let feeds = feeds(hit_box);
            assert_eq!(feeds.len(), 2, "{hit_box}");
            assert!(feeds.iter().any(|(p, c)| p.ends_with(pool) && *c == 1.0), "{hit_box} {feeds:?}");
            assert!(feeds.iter().any(|(p, c)| p == "pain" && near(*c, pain)), "{hit_box} {feeds:?}");
        };
        limb("right_arm", "_hand", 0.8);
        limb("left_forearm", "_hand", 1.0);
        limb("right_hip", "_leg", 0.9);
        limb("left_foot", "_leg", 0.8);
    }

    /// The armour of the tiers' kits: a torso item's front, back and
    /// forearms, a helmet's brain, as `armour_items.csv` lists them for the
    /// live data (`bm_torso_1` 0.15 front, back and forearms 0.1; `bm_helmet_1` 0.2
    /// brain).
    #[test]
    fn armour_items_are_read_from_their_configs() {
        let torso = armour_named("bm_torso_1").unwrap();
        assert!(near(torso.of("front"), 0.15) && near(torso.of("back"), 0.1), "{torso:?}");
        assert!(near(torso.of("right_forearm"), 0.1));
        assert_eq!(torso.of("brain"), 0.0);
        let helmet = armour_named("bm_helmet_1").unwrap();
        assert!(near(helmet.of("brain"), 0.2));
        assert!(armour_named("ak_12").is_none());
    }

    /// The ammunition's multipliers: 5.45x39 7N6M is plain (1, 1), the
    /// buckshot's pellet carries 0.2 of the weapon's damage
    /// (`DAMAGE-FORMULA.md` §1.1 step 13).
    #[test]
    fn ammunition_is_read_from_its_config() {
        let plain = ammo_named("ammo_5.45x39_7n6m").unwrap();
        assert_eq!((plain.k_damage, plain.k_arp), (1.0, 1.0));
        assert_eq!(plain.buck_shot, 1);
        assert!(plain.muzzle_speed > 0.0 && plain.air_resistance > 0.0);
        let buck = ammo_named("ammo_12mm_buck").unwrap();
        assert!(near(buck.k_damage, 0.2));
        assert!(buck.buck_shot > 1);
    }

    /// The pellets of the 12 mm ammunition: 10 each, in a cone of 4 degrees (buck) or 2.5 (buck2), none for a
    /// plain bullet (`fact-hitboxes-026g.md` 4, read from the data and the exe).
    #[test]
    fn the_pellet_cones_are_read_from_the_data() {
        let cone = |name: &str| {
            let ammo = ammo_named(name).unwrap();
            (ammo.buck_shot, ammo.buck_dispersion)
        };
        assert_eq!(cone("ammo_12mm_buck"), (10, 4.0));
        assert_eq!(cone("ammo_5.45x39_7n6m"), (1, 0.0));
        // The test archive is older than the live data and may lack the second buck ammunition.
        if ammo_named("ammo_12mm_buck2").is_some() {
            assert_eq!(cone("ammo_12mm_buck2"), (10, 2.5));
        }
    }

    /// Damage is full up to `max_damage_distance`, `min_damage_amount` of
    /// it from `min_damage_distance` on, armour piercing falling with it
    /// (steps 1 and 2), and between the two the speed's own curve, which
    /// is within 0.3 % of a line in distance (`DAMAGE-FORMULA.md` step 8).
    #[test]
    fn a_bullet_falls_off_with_distance() {
        let weapon = weapon_named("ak_74u").unwrap();
        let ammo = ammo_named("ammo_5.45x39_7n6m").unwrap();
        let near_in = bullet_at(weapon, ammo, 1.0);
        assert!(near(near_in.damage, weapon.bullet_damage * ammo.k_damage));
        assert!(near(near_in.arp, weapon.bullet_pierce * ammo.k_arp));
        let far = bullet_at(weapon, ammo, weapon.min_damage_distance * ammo.k_min_damage_distance + 50.0);
        let low = weapon.min_damage_amount * ammo.k_min_damage_amount;
        assert!(near(far.damage, near_in.damage * low), "{far:?} {near_in:?}");
        assert!(near(far.arp, near_in.arp * low));
        let (d0, d1) = (
            weapon.max_damage_distance * ammo.k_max_damage_distance,
            weapon.min_damage_distance * ammo.k_min_damage_distance,
        );
        let mid = bullet_at(weapon, ammo, (d0 + d1) / 2.0);
        let line = (near_in.damage + far.damage) / 2.0;
        assert!((mid.damage - line).abs() < 0.003 * near_in.damage, "{mid:?} {line}");
    }

    /// The players' game data, whole: `SRP_TEST_NEW_WEAPONS_DATA=<server-game-data.db> cargo test -p
    /// survarium --lib -- --ignored of_the_players_data`. Every ammunition, every armour item and the body
    /// read, and every weapon works out a bullet with every ammunition at some distances, finite.
    #[test]
    #[ignore = "needs the players' server game data (SRP_TEST_NEW_WEAPONS_DATA)"]
    fn everything_of_the_players_data_reads() {
        let path = std::env::var_os("SRP_TEST_NEW_WEAPONS_DATA").expect("the data's path");
        let data = GameData::open(std::path::Path::new(&path)).unwrap();
        let dictionary: &'static Dictionary = Box::leak(Box::new(
            Dictionary::parse(data.file(super::super::dictionary::PATH).unwrap()).unwrap(),
        ));
        let ammo = derive_ammo(&data, dictionary).unwrap();
        let armour = derive_armour(&data, dictionary).unwrap();
        let weapons = crate::v026g::weapons::derive(&data, dictionary).unwrap();
        let body = BodyParams::from_config(
            &ConfigValue::parse(data.file(HUMAN_HIT_PARAMS).unwrap()).unwrap(),
        )
        .unwrap();
        assert!(!ammo.is_empty() && !armour.is_empty() && !weapons.is_empty());
        for pool in ["body", "head", "pain"] {
            assert!(body.part(pool).unwrap().lethal, "{pool}");
        }
        for a in &ammo {
            for w in &weapons {
                for d in [0.0, 10.0, 50.0, 500.0] {
                    let b = bullet_at(w, a, d);
                    assert!(
                        b.damage.is_finite() && b.arp.is_finite() && b.damage >= 0.0,
                        "{} {} {d} {b:?}",
                        w.name,
                        a.name
                    );
                }
            }
        }
        println!("{} ammo, {} armour, {} weapons", ammo.len(), armour.len(), weapons.len());
    }

    /// The RGD-5's blast: 10 m, 1.3 at the blast to 0.05 at the radius, linear; power
    /// 0.15; explosive; the body parts 6, 0, 1, 7, 11, 12 (right leg, body, back, left leg,
    /// left foot, right arm).
    #[test]
    fn the_grenades_blast_is_read_from_its_config() {
        let rgd = splash_named("rgd-5").unwrap();
        assert_eq!((rgd.radius, rgd.max_damage, rgd.min_damage), (10.0, 1.3, 0.05));
        assert!(near(rgd.pierce, 0.15) && rgd.damage_type == "explosive");
        let body = human();
        let names: Vec<_> = rgd.affected.iter().map(|&i| body.parts[i].name.as_str()).collect();
        assert_eq!(
            names,
            ["right_leg", "body", "back", "left_leg", "left_foot", "right_arm"]
        );
        assert!(near(rgd.amount_at(0.0).unwrap(), 1.3) && near(rgd.amount_at(10.0).unwrap(), 0.05));
        assert!(near(rgd.amount_at(5.0).unwrap(), 0.675) && rgd.amount_at(10.1).is_none());
        assert!(splash_named("ak_74u").is_none());
    }

    /// A wall keeps `1 − (1 − clamp(power / resistance − 1, 0, 1))²`: a wall of twice the
    /// power stops it, one of half the power's resistance passes it all, 0 keeps all.
    #[test]
    fn a_wall_keeps_what_the_power_beats() {
        assert_eq!(wall_share(0.15, 0.30), 0.0);
        assert_eq!(wall_share(0.15, 0.075), 1.0);
        assert!(near(wall_share(0.15, 0.1), 0.75));
        assert_eq!(wall_share(0.15, 0.0), 1.0);
    }

    /// One blast of 0.8 on the parts of the RGD-5: the body pool takes the largest of the body
    /// part's and the back's 0.8, pain the largest of the back's, the foot's and the arm's
    /// (0.8, not the sum); a leg pool its own part's; armour does not help (arp 1.0).
    #[test]
    fn a_blast_takes_each_pool_its_largest_hit() {
        let body = human();
        let rgd = splash_named("rgd-5").unwrap();
        let hits: Vec<_> = rgd.affected.iter().map(|&i| (i, 0.8)).collect();
        let taken = body.hits_taken(&hits, 1.0, &|_| 0.9);
        let of = |pool: &str| taken.iter().find(|(p, _)| p == pool).map(|&(_, d)| d);
        assert!(near(of("body").unwrap(), 0.8) && near(of("pain").unwrap(), 0.8), "{taken:?}");
        assert!(near(of("right_leg").unwrap(), 0.8) && near(of("left_leg").unwrap(), 0.8));
        assert_eq!(of("head"), None);
        // Walls cut the back's blast to 0.2: the body pool keeps the direct hit's 0.8, pain the
        // foot's and the arm's 0.8 x 0.8.
        let hits = [(6, 0.8), (0, 0.8), (1, 0.2), (7, 0.8), (11, 0.8), (12, 0.8)];
        let taken = body.hits_taken(&hits, 1.0, &|_| 0.0);
        let of = |pool: &str| taken.iter().find(|(p, _)| p == pool).map(|&(_, d)| d);
        assert!(near(of("body").unwrap(), 0.8) && near(of("pain").unwrap(), 0.64), "{taken:?}");
    }

    /// `amount × (1 − clamp(armor − arp, 0, 1))` (step 3): armour below the
    /// bullet's piercing does nothing, each 0.1 above removes a tenth, and
    /// the best armour against no piercing leaves a tenth.
    #[test]
    fn armour_takes_what_the_piercing_does_not_beat() {
        assert!(near(after_armour(0.4, 0.3, 0.2), 0.4));
        assert!(near(after_armour(0.4, 0.3, 0.6), 0.4 * 0.7));
        assert!(near(after_armour(0.4, 0.0, 0.9), 0.04));
        assert!(near(after_armour(0.4, 0.3, 0.3), 0.4));
    }
}
