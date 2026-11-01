//! What changes a 0.26g item: its modifications and defects, the upgrade
//! ("craft") and modification change ("reroll") prices and chances
//! (`docs/notes/data.md` "Durability, repair and modding").
//!
//! The game's own files, embedded as they are in 0.26g's `resources.db`:
//!
//! - `gameplay/items/modifications/modifications.options`: 158
//!   `modifications`, 20 `defects` and 3 `defect_slots`;
//! - `gameplay/items/craft/craft.options`: prices by item level and the
//!   chances by the difference in modifications;
//! - `gameplay/items/reroll/reroll.options`: prices by item level (its
//!   multiplier is not used: every change costs the same);
//!
//! and, from every item's config, what each item may carry ([`ItemMods`]:
//! `available_modifications_mask`, `available_defects_mask`,
//! `camo_available`, `decal_available`). All of them are the players' own
//! files ([`super::game_data`]).
//!
//! **What the client knows of this** (0.26g exe, PROVEN by its strings and
//! readers): each modification's and defect's `id`, `modifiers`, `ui_desc`,
//! `lobby_info`, and a defect's three `fix_*` values (loader 0x2C9E31..;
//! tables at dictionary +0x140 modifications, +0x148 defects, 0x2C a row),
//! and the items' `default_modifications`. An item's modification on the
//! wire is `{id, state}`: **state 0 is a modification, anything else a
//! defect**, and the id is looked up in that table with no check for a
//! miss (0x229A98..0x229F3B: a NULL row is read at 0x229AC4). The lobby
//! shows state 1 as damage ("critical damages", `MessageRepair`), counts
//! the state 0 entries as the item's rank (`InventoryList.getItemRank`) and
//! lets only those be changed. The item's `default_modifications` are not
//! on the wire: the client adds them from the item's config.
//!
//! **What only the server knew**: `drop_weight`, `type_mask`,
//! `type_requirements_mask`, `required_modifications`, `defect_slots`, the
//! items' two masks, and all of the craft and reroll files (none of these
//! strings is in the exe; the craft tables reach the lobby in 0x9B and the
//! reroll prices in `83 46 06`). So every roll here is srp's reading of
//! that data:
//!
//! - A modification's `type_mask` has one character set: its **type**
//!   (1..=40), five strengths of a type sharing it with `drop_weight`
//!   10, 8, 6.4, 5.12, 4.096. An item may get the types its
//!   `available_modifications_mask` names; the item's mask is one character
//!   shorter, its character `j` standing for type `j + 1` [PROVEN by the
//!   data: only so do bolt-action rifles get "chamber a round" and automatic
//!   ones the rate of fire, and the same shift makes the defects' masks
//!   agree]. Types with a `drop_weight` of 0 (pockets, artefact slots, ammo
//!   bags, the boosters' and premium items' bonuses) are only ever
//!   `default_modifications`.
//! - An item holds one modification of a type at most [LIKELY: the types
//!   are the lobby's rows], and three in all (the lobby's rank 3,
//!   `st_craft_message_modifers_count`).
//! - A roll picks among the modifications of the allowed types the item
//!   does not hold, by `drop_weight` [LIKELY: the name, and 0.21d's drops
//!   used the same key so], and [srp, the owner's rule of 2026-10-23]
//!   never one that only changes wear (reliability: items wear nothing).
//! - `type_requirements_mask` equals `type_mask` in every row; it is not
//!   used.
//! - A defect needs its type in the item's `available_defects_mask` and,
//!   when it has `required_modifications: [low, high]`, a modification of
//!   the item (a default one counts) with an id in that range [LIKELY: the
//!   three such defects take a pocket, an artefact slot or an ammo bag away
//!   and name the ids of the modifications that give one].
//! - `defect_slots`: three slots, `condition: [0.9, 0]`, `[0.5, 0]`,
//!   `[0.1, 0]` and `probability: [0.1, 0.5]`, `[0.3, 0.7]`, `[0.5, 1]`.
//!   Read as: slot `n` opens when the condition is under its first number,
//!   and the chance of a defect in it goes from the first probability there
//!   to the second at condition 0 [LIKELY]; when the roll is made is not in
//!   the data [srp: once for each match that wore the item,
//!   [`Modifications::roll_defect`]].
//! - A modifier's `type` 1 adds its value and 2 multiplies by `1 + value`
//!   [LIKELY: armour and slot counts are type 1 with absolute steps, damage,
//!   weight and wear type 2 with small fractions].

use binary_config_parser::tree::{ConfigError, ConfigValue};

use super::dictionary::Dictionary;
use super::drops::Rng;
use super::game_data::{GameData, GameDataError};
use super::item_params::item_params;
use super::scopes::{SCOPE_SLOT, ScopeFits};

/// The paths of the three files in the archive.
pub const MODIFICATIONS_PATH: &str = "gameplay/items/modifications/modifications.options";
pub const CRAFT_PATH: &str = "gameplay/items/craft/craft.options";
pub const REROLL_PATH: &str = "gameplay/items/reroll/reroll.options";

/// The modifications of its own an item holds at most: the lobby's highest
/// rank (`MessageCraftItem`: "rank >= 3" is refused).
pub const MAX_RANK: usize = 3;
/// The item levels the craft and reroll price tables have.
pub const PRICE_LEVELS: usize = 16;
/// The modifications a reroll offers (`83 46 07`).
pub const REROLL_CHOICES: usize = 3;
/// The upgrade slots of an item: decals in 0 and 1 (the lobby shows two
/// for `decal_available`, 0x28779F; a weapon's, [`WEAPON_DECAL_SLOTS`]), the camouflage in 3
/// (`DragRegion.as`: `relocate_upgrade_item(item, camo, 3)`).
pub const DECAL_SLOTS: [u8; 2] = [0, 1];
/// [srp] A weapon's two decal slots, as armour's: a weapon holds up to six
/// upgrades in logical slots (decals 0 and 1, camouflage 2, scope 3, muzzle
/// 4, grip 5, [`super::weapon_kit`]), which go to the client packed into at
/// most three records (the client keeps three an item, `limits::UPGRADES`;
/// client patch 68 reads the kit). Patch 47 bakes both decals.
pub const WEAPON_DECAL_SLOTS: [u8; 2] = [0, 1];
pub const CAMO_SLOT: u8 = 3;
/// [srp] The camouflage's slot on a weapon (one whose config has
/// `camo_available`, `srp-026g/scripts/weapon-camo`): 2, since a weapon's 3
/// holds its scope ([`SCOPE_SLOT`]). The client keeps a record whatever its
/// slot; a weapon wears every upgrade's modification (0x2F14C0), and patch
/// 44 bakes the camouflage it names.
pub const WEAPON_CAMO_SLOT: u8 = 2;

/// The slot a camouflage goes into on an item of type `kind`
/// ([`ItemParams::kind`](super::item_params::ItemParams::kind)): a weapon's
/// (`wpn_*`) [`WEAPON_CAMO_SLOT`], any other item's [`CAMO_SLOT`].
pub fn camo_slot(kind: &str) -> u8 {
    if is_weapon_kind(kind) {
        WEAPON_CAMO_SLOT
    } else {
        CAMO_SLOT
    }
}

/// An item of type `kind` is a weapon (`wpn_*`): its camouflage is in
/// [`WEAPON_CAMO_SLOT`], and the lobby builds it again with a decal's new
/// place only from the account the server sends after `46 05` (patch 47).
pub fn is_weapon_kind(kind: &str) -> bool {
    kind.starts_with("wpn_")
}

/// What an item may be modified with, from its config ([`all_item_mods`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ItemMods {
    pub dict_id: u16,
    /// Bit `t`: modifications of type `t` may be on the item.
    pub modifications: u64,
    /// Bit `t`: defects of type `t` may be on the item.
    pub defects: u32,
    /// The item takes a camouflage / decals.
    pub camo: bool,
    pub decal: bool,
}

/// One change a modification makes to the item's config.
#[derive(Clone, Debug, PartialEq)]
pub struct Modifier {
    /// Where in the item's config, e.g. `parameters/bullet_damage`.
    pub path: String,
    pub value: f32,
    /// 1 adds, 2 multiplies by `1 + value` (LIKELY).
    pub kind: u8,
}

/// A row of `modifications`.
#[derive(Clone, Debug, PartialEq)]
pub struct Modification {
    pub id: u32,
    /// The character of `type_mask` that is set, 1..=40.
    pub kind: u8,
    pub drop_weight: f32,
    /// The localization key of its name (`ui_desc.name`).
    pub name: String,
    pub modifiers: Vec<Modifier>,
    /// `lobby_info`: what the lobby shows it changes, by its own names.
    pub lobby_info: Vec<(String, f32)>,
    /// What the lobby prints for it.
    pub shown: Shown,
}

/// A modification's line in the lobby: `ui_desc.props_list`'s one row, a
/// property of [`super::ui_properties`] and the value printed beside it,
/// and its icon (`ui_desc.icon`, a cell of the lobby's `modifiers` atlas).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shown {
    pub property: u32,
    pub value: f32,
    pub icon: u32,
}

/// The item parameters of its wear: what a "reliability" modification
/// changes, and nothing else does.
pub const WEAR_PARAMETERS: [&str; 3] = [
    "parameters/degrade_per_match",
    "parameters/degrade_per_use",
    "parameters/degrade_per_death",
];

impl Modification {
    /// It changes nothing but the item's wear ([`WEAR_PARAMETERS`]): the
    /// "reliability" ones ("Надежность", the lobby's property 123,
    /// `st_multiplicative_reliability_modifier`), 1200..=1204 on gear and
    /// 2090..=2094 on weapons in the game's data.
    pub fn only_changes_wear(&self) -> bool {
        !self.modifiers.is_empty()
            && self
                .modifiers
                .iter()
                .all(|modifier| WEAR_PARAMETERS.contains(&modifier.path.as_str()))
    }

    /// A roll may give it: it has a drop weight, and it is not one that
    /// only changes wear ([srp, the owner's rule of 2026-10-23]: items wear
    /// nothing since #35, so a reliability modification would be a roll
    /// wasted; `docs/notes/no-reliability-roll.md`). Every roll (a drop, an
    /// upgrade, a reroll's offer) picks from [`Modifications::candidates`],
    /// which asks this.
    pub fn rolls(&self) -> bool {
        self.drop_weight > 0.0 && !self.only_changes_wear()
    }
}

/// A row of `defects`.
#[derive(Clone, Debug, PartialEq)]
pub struct Defect {
    pub id: u32,
    /// The character of `type_mask` that is set, 1..=20.
    pub kind: u8,
    pub drop_weight: f32,
    pub name: String,
    /// Silver, gold and seconds to fix it.
    pub fix_cost_general: u32,
    pub fix_cost_gold: u32,
    pub fix_time_general: u32,
    /// `required_modifications`: the item must hold a modification with an
    /// id in `low..=high`.
    pub required_modifications: Option<(u32, u32)>,
    pub modifiers: Vec<Modifier>,
}

/// A row of `defect_slots`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DefectSlot {
    /// `condition`: the slot is open under the first; the second is where
    /// the chance is highest.
    pub condition: (f32, f32),
    /// `probability` at those two conditions.
    pub probability: (f32, f32),
}

impl DefectSlot {
    /// The chance of a defect in this slot at `condition` (0..=1): none
    /// until the slot is open.
    pub fn chance(&self, condition: f32) -> f32 {
        let (open, worst) = self.condition;
        if condition >= open || open <= worst {
            return 0.0;
        }
        let along = ((open - condition) / (open - worst)).clamp(0.0, 1.0);
        self.probability.0 + (self.probability.1 - self.probability.0) * along
    }
}

/// `craft.options`.
#[derive(Clone, Debug, PartialEq)]
pub struct Craft {
    /// Silver and gold by the item's level.
    pub price_general: [u16; PRICE_LEVELS],
    pub price_premium: [u16; PRICE_LEVELS],
    /// By how many more modifications the item has than its donor (0, 1,
    /// 2): `(success %, failure %)`; the rest is "nothing happened".
    pub chances_general: [(f32, f32); 3],
    pub chances_premium: [(f32, f32); 3],
}

/// What an upgrade ("craft") does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CraftOutcome {
    /// One more modification.
    Success,
    /// Nothing but the donor and the price are lost.
    Neutral,
    /// One modification less, in the game's data; srp takes none away
    /// (the owner's rule of 2026-10-23, `docs/notes/craft-menu.md`): the
    /// session does what [`CraftOutcome::Neutral`] does.
    Failure,
}

impl Craft {
    /// The price for an item of `item_level`, in silver or in gold. A level
    /// past the table costs the table's last.
    pub fn price(&self, item_level: u8, gold: bool) -> u32 {
        let table = if gold {
            &self.price_premium
        } else {
            &self.price_general
        };
        u32::from(table[usize::from(item_level).min(PRICE_LEVELS - 1)])
    }

    /// `(success %, failure %)` for an item with `difference` more
    /// modifications than its donor; `None` past the table (the lobby
    /// refuses a donor with more than the item, and 3 is the most).
    pub fn chances(&self, difference: usize, gold: bool) -> Option<(f32, f32)> {
        let table = if gold {
            &self.chances_premium
        } else {
            &self.chances_general
        };
        table.get(difference).copied()
    }

    /// The outcome for a point `at` of 0..100 (success first, then
    /// failure, then nothing).
    pub fn outcome_at(&self, difference: usize, gold: bool, at: f32) -> Option<CraftOutcome> {
        let (success, failure) = self.chances(difference, gold)?;
        Some(if at < success {
            CraftOutcome::Success
        } else if at < success + failure {
            CraftOutcome::Failure
        } else {
            CraftOutcome::Neutral
        })
    }

    /// What 0x9B carries: the twelve chances as the lobby indexes them
    /// (`MessageCraftItem.updateCraftChance`: row = the difference in
    /// rank, then success and failure in silver, success and failure in
    /// gold), and the two price tables.
    pub fn tables(&self) -> ([f32; 12], [u16; PRICE_LEVELS], [u16; PRICE_LEVELS]) {
        let mut chances = [0.0; 12];
        for row in 0..3 {
            chances[row * 4] = self.chances_general[row].0;
            chances[row * 4 + 1] = self.chances_general[row].1;
            chances[row * 4 + 2] = self.chances_premium[row].0;
            chances[row * 4 + 3] = self.chances_premium[row].1;
        }
        (chances, self.price_general, self.price_premium)
    }
}

/// `reroll.options`. Its `reroll_modificator_multiplicator` (1.1) is not
/// read: [srp, the owner's rule of 2026-10-25] every change costs the same
/// (`docs/notes/flat-reroll.md`).
#[derive(Clone, Debug, PartialEq)]
pub struct Reroll {
    pub price_general: [u16; PRICE_LEVELS],
    pub price_premium: [u16; PRICE_LEVELS],
}

impl Reroll {
    /// The price of a change of a modification on an item of `item_level`:
    /// the table's, however many changes the item had (the owner's rule of
    /// 2026-10-25).
    pub fn price(&self, item_level: u8, gold: bool) -> u32 {
        let table = if gold {
            &self.price_premium
        } else {
            &self.price_general
        };
        u32::from(table[usize::from(item_level).min(PRICE_LEVELS - 1)])
    }
}

/// What the modifications of the items a kit wears add to a match's pay
/// (`money_bonus`, `experience_bonus` and `reputation_bonus` of the item
/// configs, which only modifications 1230..=1280 change): fractions, 0.1 is
/// 10 % more.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Bonuses {
    pub money: f32,
    pub experience: f32,
    /// By lobby faction, 1..=4 at index 0..=3.
    pub reputation: [f32; 4],
}

/// Everything of the three files.
#[derive(Clone, Debug, PartialEq)]
pub struct Modifications {
    /// By `id`.
    pub modifications: Vec<Modification>,
    /// By `id`.
    pub defects: Vec<Defect>,
    pub defect_slots: Vec<DefectSlot>,
    pub craft: Craft,
    pub reroll: Reroll,
}

fn number(value: &ConfigValue) -> Result<u32, ConfigError> {
    value
        .as_i32()
        .and_then(|number| u32::try_from(number).ok())
        .ok_or_else(|| ConfigError("a modifications number is not one".into()))
}

fn float(value: &ConfigValue) -> Result<f32, ConfigError> {
    value
        .as_f32()
        .ok_or_else(|| ConfigError("a modifications number is not one".into()))
}

/// The one character of a mask that is set.
fn kind_of(row: &ConfigValue) -> Result<u8, ConfigError> {
    let mask = row.str_at("type_mask")?;
    let mut set = mask.bytes().enumerate().filter(|&(_, byte)| byte == b'1');
    match (set.next(), set.next()) {
        (Some((index, _)), None) if (1..64).contains(&index) => Ok(index as u8),
        _ => Err(ConfigError(format!("type_mask `{mask}` is not one type"))),
    }
}

fn modifiers(row: &ConfigValue) -> Result<Vec<Modifier>, ConfigError> {
    row.req("modifiers")?
        .children()
        .into_iter()
        .map(|modifier| {
            let path: Vec<&str> = modifier
                .req("path")?
                .children()
                .into_iter()
                .filter_map(ConfigValue::as_str)
                .collect();
            Ok(Modifier {
                path: path.join("/"),
                value: modifier.f32_at("value")?,
                kind: number(modifier.req("type")?)? as u8,
            })
        })
        .collect()
}

fn name_of(row: &ConfigValue) -> Result<String, ConfigError> {
    Ok(row.req("ui_desc")?.str_at("name")?.to_owned())
}

/// `ui_desc.props_list`: one row, its `prop_value` a number as text.
fn shown_of(row: &ConfigValue) -> Result<Shown, ConfigError> {
    let props = row.req("ui_desc")?.req("props_list")?.children();
    let [prop] = props[..] else {
        return Err(ConfigError(format!(
            "`ui_desc.props_list` has {} rows, not one",
            props.len()
        )));
    };
    let value = prop.req("prop_value")?;
    let value = value
        .as_str()
        .and_then(|text| text.trim().parse::<f32>().ok())
        .or_else(|| value.as_f32())
        .ok_or_else(|| ConfigError("`prop_value` is not a number".into()))?;
    Ok(Shown {
        property: number(prop.req("prop_id")?)?,
        value,
        icon: number(row.req("ui_desc")?.req("icon")?)?,
    })
}

fn prices(root: &ConfigValue, key: &str) -> Result<[u16; PRICE_LEVELS], ConfigError> {
    let list: Vec<u16> = root
        .req(key)?
        .children()
        .into_iter()
        .map(|price| {
            number(price).and_then(|price| {
                u16::try_from(price).map_err(|_| ConfigError(format!("`{key}` is over a u16")))
            })
        })
        .collect::<Result<_, _>>()?;
    list.try_into()
        .map_err(|_| ConfigError(format!("`{key}` has not {PRICE_LEVELS} prices")))
}

fn chances(root: &ConfigValue, key: &str) -> Result<[(f32, f32); 3], ConfigError> {
    let list: Vec<(f32, f32)> = root
        .req(key)?
        .children()
        .into_iter()
        .map(|row| match row.children()[..] {
            [success, failure] => Ok((float(success)?, float(failure)?)),
            _ => Err(ConfigError(format!("a row of `{key}` is not a pair"))),
        })
        .collect::<Result<_, _>>()?;
    list.try_into()
        .map_err(|_| ConfigError(format!("`{key}` has not 3 rows")))
}

fn pair(row: &ConfigValue, key: &str) -> Result<(f32, f32), ConfigError> {
    match row.req(key)?.children()[..] {
        [first, second] => Ok((float(first)?, float(second)?)),
        _ => Err(ConfigError(format!("`{key}` is not a pair"))),
    }
}

/// A fraction in `[0, 1)`.
fn unit(rng: &mut Rng) -> f32 {
    rng.below(1 << 24) as f32 / (1 << 24) as f32
}

/// A `drop_weight` as a whole weight for [`Rng::pick`] (4.096 is the
/// finest).
fn weight(drop_weight: f32) -> u32 {
    (drop_weight * 1000.0).round().max(0.0) as u32
}

/// What `dict_id` may be modified with; `None` for an item that takes
/// nothing (stacks, cosmetics, boosters).
pub fn item_mods(dict_id: u16) -> Option<&'static ItemMods> {
    let all = all_item_mods();
    all.binary_search_by_key(&dict_id, |item| item.dict_id)
        .ok()
        .map(|index| &all[index])
}

/// What each item may be modified with, by `dict_id`; an item that takes
/// nothing has no row ([`derive_item_mods`]).
pub fn all_item_mods() -> &'static [ItemMods] {
    static SHARED: std::sync::OnceLock<Vec<ItemMods>> = std::sync::OnceLock::new();
    SHARED.get_or_init(|| {
        derive_item_mods(super::game_data::shared(), Dictionary::shared())
            .expect("the item configs read")
    })
}

/// A row for each item of `dictionary` whose config (in `data`) lets it
/// take anything, by `dict_id`.
pub fn derive_item_mods(
    data: &GameData,
    dictionary: &Dictionary,
) -> Result<Vec<ItemMods>, GameDataError> {
    let mut rows = Vec::new();
    for item in &dictionary.items {
        let path = item.cfg_name.as_str();
        let config = super::game_data::config(data, path)?.ok_or_else(|| {
            GameDataError(format!(
                "item {} `{}` has no {path}",
                item.dict_id, item.name
            ))
        })?;
        if let Some(row) = ItemMods::from_config(item.dict_id, &config)
            .map_err(|cause| GameDataError(format!("{path}: {}", cause.0)))?
        {
            rows.push(row);
        }
    }
    rows.sort_by_key(|row| row.dict_id);
    Ok(rows)
}

/// A mask of an item's config as a number: character `j` is bit `j + 1`
/// (`modifications.options`' `type_mask`s have one character more in
/// front).
fn mask_bits(mask: &str, key: &str) -> Result<u64, ConfigError> {
    let mut bits = 0_u64;
    for (j, byte) in mask.bytes().enumerate() {
        match byte {
            b'0' => {}
            b'1' if j + 1 < 64 => bits |= 1 << (j + 1),
            _ => {
                return Err(ConfigError(format!(
                    "`parameters.{key}` `{mask}` is not a mask"
                )));
            }
        }
    }
    Ok(bits)
}

impl ItemMods {
    /// The row of an item's config: `parameters.available_modifications_mask`
    /// and `available_defects_mask`, `camo_available` and
    /// `decal_available`; `None` for an item with nothing set.
    pub fn from_config(dict_id: u16, config: &ConfigValue) -> Result<Option<Self>, ConfigError> {
        let p = config.get("parameters");
        let mask = |key: &str| -> Result<u64, ConfigError> {
            match p.and_then(|p| p.get(key)) {
                None => Ok(0),
                Some(value) => {
                    let text = value.as_str().ok_or_else(|| {
                        ConfigError(format!("`parameters.{key}` is not a string"))
                    })?;
                    mask_bits(text, key)
                }
            }
        };
        let flag =
            |key: &str| p.and_then(|p| p.get(key)).and_then(ConfigValue::as_bool) == Some(true);
        let modifications = mask("available_modifications_mask")?;
        let defects = mask("available_defects_mask")?;
        let defects = u32::try_from(defects).map_err(|_| {
            ConfigError("`parameters.available_defects_mask` is over 31 types".into())
        })?;
        let (camo, decal) = (flag("camo_available"), flag("decal_available"));
        if modifications == 0 && defects == 0 && !camo && !decal {
            return Ok(None);
        }
        Ok(Some(Self {
            dict_id,
            modifications,
            defects,
            camo,
            decal,
        }))
    }
}

/// The upgrade slots `upgrade_dict_id` may go into on `host_dict_id`, by
/// the game data's scopes ([`upgrade_slots_in`]).
pub fn upgrade_slots(host_dict_id: u16, upgrade_dict_id: u16) -> &'static [u8] {
    upgrade_slots_in(ScopeFits::shared(), host_dict_id, upgrade_dict_id)
}

/// The upgrade slots `upgrade_dict_id` may go into on `host_dict_id`: a
/// scope item that has a variant for the weapon (`scopes`) into
/// [`SCOPE_SLOT`]; a camouflage (category 25, the kinds `*_camo`) into
/// [`CAMO_SLOT`] of an item with `camo_available` ([`WEAPON_CAMO_SLOT`] of a
/// weapon, [`camo_slot`]), a decal (category 26,
/// `*_decl`) into the [`DECAL_SLOTS`] of one with `decal_available`. Empty:
/// it does not go on. `items_compatibility` pairs no cosmetic with
/// anything, so any camouflage goes on any such item [as far as the data
/// says].
pub fn upgrade_slots_in(
    scopes: &ScopeFits,
    host_dict_id: u16,
    upgrade_dict_id: u16,
) -> &'static [u8] {
    if scopes.variant(host_dict_id, upgrade_dict_id).is_some() {
        return &[SCOPE_SLOT];
    }
    let (Some(host), Some(upgrade)) = (item_mods(host_dict_id), item_params(upgrade_dict_id))
    else {
        return &[];
    };
    if upgrade.modification == 0 {
        return &[];
    }
    let host_kind = item_params(host_dict_id).map_or("", |params| params.kind);
    cosmetic_slots(host, host_kind, upgrade.kind)
}

/// The slots a cosmetic of type `upgrade_kind` goes into on `host`, an item
/// of type `host_kind`: a camouflage (`*_camo`) into [`CAMO_SLOT`] of one
/// with `camo_available` ([`WEAPON_CAMO_SLOT`] on a weapon, [`camo_slot`]),
/// a decal (`*_decl`) into the [`DECAL_SLOTS`] of one with
/// `decal_available` ([`WEAPON_DECAL_SLOTS`] on a weapon).
pub fn cosmetic_slots(host: &ItemMods, host_kind: &str, upgrade_kind: &str) -> &'static [u8] {
    if upgrade_kind.ends_with("_camo") && host.camo {
        if camo_slot(host_kind) == WEAPON_CAMO_SLOT {
            &[WEAPON_CAMO_SLOT]
        } else {
            &[CAMO_SLOT]
        }
    } else if upgrade_kind.ends_with("_decl") && host.decal {
        if is_weapon_kind(host_kind) {
            &WEAPON_DECAL_SLOTS
        } else {
            &DECAL_SLOTS
        }
    } else {
        &[]
    }
}

/// The dict id an upgrade record of `upgrade_dict_id` on `host_dict_id`
/// holds: for a scope item, the variant made for that weapon (the client
/// builds the weapon with the modification of the record's dict id,
/// [`super::scopes`]); for any other upgrade, its own.
pub fn record_dict_in(scopes: &ScopeFits, host_dict_id: u16, upgrade_dict_id: u16) -> u16 {
    scopes
        .variant(host_dict_id, upgrade_dict_id)
        .unwrap_or(upgrade_dict_id)
}

/// The slots a record holding `record_dict_id` may be in on `host_dict_id`:
/// a variant made for that weapon in [`SCOPE_SLOT`]; none for any other
/// variant (placed for another weapon's mount and eye line) or a scope
/// item's own id (records hold variants); a cosmetic's
/// ([`upgrade_slots_in`]).
pub fn record_slots_in(
    scopes: &ScopeFits,
    host_dict_id: u16,
    record_dict_id: u16,
) -> &'static [u8] {
    if scopes.fits(host_dict_id, record_dict_id) {
        &[SCOPE_SLOT]
    } else if scopes.is_variant(record_dict_id) || scopes.is_scope(record_dict_id) {
        &[]
    } else {
        upgrade_slots_in(scopes, host_dict_id, record_dict_id)
    }
}

impl Modifications {
    /// The players' files ([`super::game_data`]), parsed once.
    pub fn shared() -> &'static Self {
        static SHARED: std::sync::OnceLock<Modifications> = std::sync::OnceLock::new();
        SHARED.get_or_init(|| {
            use super::game_data::file;
            Self::parse(
                file(MODIFICATIONS_PATH),
                file(CRAFT_PATH),
                file(REROLL_PATH),
            )
            .expect("the modification files parse")
        })
    }

    pub fn parse(modifications: &[u8], craft: &[u8], reroll: &[u8]) -> Result<Self, ConfigError> {
        let root = ConfigValue::parse(modifications)?;
        let mut list: Vec<Modification> = root
            .req("modifications")?
            .children()
            .into_iter()
            .map(|row| {
                Ok(Modification {
                    id: number(row.req("id")?)?,
                    kind: kind_of(row)?,
                    drop_weight: row.f32_at("drop_weight")?,
                    name: name_of(row)?,
                    modifiers: modifiers(row)?,
                    lobby_info: row
                        .req("lobby_info")?
                        .entries()
                        .iter()
                        .filter_map(|(name, value)| Some((name.clone(), value.as_f32()?)))
                        .collect(),
                    shown: shown_of(row)?,
                })
            })
            .collect::<Result<_, ConfigError>>()?;
        list.sort_by_key(|modification| modification.id);
        let mut defects: Vec<Defect> = root
            .req("defects")?
            .children()
            .into_iter()
            .map(|row| {
                let required = match row.get("required_modifications") {
                    Some(range) => match range.children()[..] {
                        [low, high] => Some((number(low)?, number(high)?)),
                        _ => return Err(ConfigError("required_modifications is no pair".into())),
                    },
                    None => None,
                };
                Ok(Defect {
                    id: number(row.req("id")?)?,
                    kind: kind_of(row)?,
                    drop_weight: row.f32_at("drop_weight")?,
                    name: name_of(row)?,
                    fix_cost_general: number(row.req("fix_cost_general")?)?,
                    fix_cost_gold: number(row.req("fix_cost_gold")?)?,
                    fix_time_general: number(row.req("fix_time_general")?)?,
                    required_modifications: required,
                    modifiers: modifiers(row)?,
                })
            })
            .collect::<Result<_, ConfigError>>()?;
        defects.sort_by_key(|defect| defect.id);
        let defect_slots = root
            .req("defect_slots")?
            .children()
            .into_iter()
            .map(|row| {
                Ok(DefectSlot {
                    condition: pair(row, "condition")?,
                    probability: pair(row, "probability")?,
                })
            })
            .collect::<Result<_, ConfigError>>()?;

        let craft = ConfigValue::parse(craft)?;
        let craft = craft.req("parameters")?;
        let reroll = ConfigValue::parse(reroll)?;
        let reroll = reroll.req("parameters")?;
        Ok(Self {
            modifications: list,
            defects,
            defect_slots,
            craft: Craft {
                price_general: prices(craft, "craft_price_general")?,
                price_premium: prices(craft, "craft_price_premium")?,
                chances_general: chances(craft, "craft_probabilities_general")?,
                chances_premium: chances(craft, "craft_probabilities_premium")?,
            },
            reroll: Reroll {
                price_general: prices(reroll, "reroll_price_general")?,
                price_premium: prices(reroll, "reroll_price_premium")?,
            },
        })
    }

    pub fn modification(&self, id: u32) -> Option<&Modification> {
        self.modifications
            .binary_search_by_key(&id, |modification| modification.id)
            .ok()
            .map(|index| &self.modifications[index])
    }

    pub fn defect(&self, id: u32) -> Option<&Defect> {
        self.defects
            .binary_search_by_key(&id, |defect| defect.id)
            .ok()
            .map(|index| &self.defects[index])
    }

    /// Whether the client has the row an item's `{id, state}` names: a
    /// modification for a state of false, a defect for true. One it lacks
    /// crashes it (0x229AC4).
    pub fn is_known(&self, id: u32, state: bool) -> bool {
        if state {
            self.defect(id).is_some()
        } else {
            self.modification(id).is_some()
        }
    }

    /// The types of the modifications in `ids` (ids of no row are
    /// skipped), as a mask.
    fn kinds_of(&self, ids: impl IntoIterator<Item = u32>) -> u64 {
        ids.into_iter()
            .filter_map(|id| self.modification(id))
            .fold(0, |mask, modification| mask | 1 << modification.kind)
    }

    /// Whether `modification` may be put on `dict_id` beside `held` (the
    /// item's own modifications; its defaults are counted too): its type
    /// is one the item takes and holds none of.
    pub fn fits(&self, dict_id: u16, held: &[u32], modification: u32) -> bool {
        let (Some(item), Some(row)) = (item_mods(dict_id), self.modification(modification)) else {
            return false;
        };
        let defaults = item_params(dict_id).map_or(&[][..], |params| params.default_modifications);
        let taken = self.kinds_of(held.iter().chain(defaults).copied());
        item.modifications & (1 << row.kind) != 0 && taken & (1 << row.kind) == 0
    }

    /// The modifications a roll for `dict_id` picks among: of the types
    /// the item takes and holds none of (`held` and its defaults), with a
    /// drop weight, and none that only changes wear ([`Modification::rolls`]).
    /// The one pool of every roll: [`Self::roll`] (drops, upgrades) and
    /// [`Self::reroll_choices`].
    pub fn candidates(&self, dict_id: u16, held: &[u32]) -> Vec<&Modification> {
        let Some(item) = item_mods(dict_id) else {
            return Vec::new();
        };
        let defaults = item_params(dict_id).map_or(&[][..], |params| params.default_modifications);
        let taken = self.kinds_of(held.iter().chain(defaults).copied());
        self.modifications
            .iter()
            .filter(|row| {
                row.rolls()
                    && item.modifications & (1 << row.kind) != 0
                    && taken & (1 << row.kind) == 0
            })
            .collect()
    }

    /// One more modification for `dict_id`, which holds `held`: by drop
    /// weight. `None` when the item takes no more.
    pub fn roll(&self, dict_id: u16, held: &[u32], rng: &mut Rng) -> Option<u32> {
        if held.len() >= MAX_RANK {
            return None;
        }
        let candidates = self.candidates(dict_id, held);
        let weights: Vec<u32> = candidates
            .iter()
            .map(|row| weight(row.drop_weight))
            .collect();
        rng.pick(&weights).map(|index| candidates[index].id)
    }

    /// The modifications of a new item of `rarity` (a drop's rarity is how
    /// many it has: the lobby's rank, `min(3, base + count)`): that many,
    /// each of another type; fewer when the item takes fewer.
    pub fn roll_for_rarity(&self, dict_id: u16, rarity: u8, rng: &mut Rng) -> Vec<u32> {
        let mut held = Vec::new();
        for _ in 0..usize::from(rarity).min(MAX_RANK) {
            match self.roll(dict_id, &held, rng) {
                Some(id) => held.push(id),
                None => break,
            }
        }
        held
    }

    /// The three modifications offered in place of `replaced` on an item
    /// that holds `held` (`replaced` among them): of the types the item's
    /// other modifications and its defaults leave free, the replaced one's
    /// own type included but not `replaced` itself; three different types
    /// while there are, then other strengths of the types offered (an item
    /// that takes few types, as a backpack or a balaclava with one
    /// modification, has only two types left). `None` when the item has not
    /// three modifications to offer [srp: the lobby's window shows three]:
    /// an item whose types besides reliability are all held has none for its
    /// reliability (reliability never rolls, [`Modification::rolls`]). On
    /// live data every item rolls three types or more (`third-mod-1.json`),
    /// so a full item always has one.
    pub fn reroll_choices(
        &self,
        dict_id: u16,
        held: &[u32],
        replaced: u32,
        rng: &mut Rng,
    ) -> Option<[u32; REROLL_CHOICES]> {
        let others: Vec<u32> = held.iter().copied().filter(|&id| id != replaced).collect();
        let mut pool: Vec<&Modification> = self
            .candidates(dict_id, &others)
            .into_iter()
            .filter(|row| row.id != replaced)
            .collect();
        let mut choices = [0; REROLL_CHOICES];
        let mut offered: u64 = 0;
        for choice in &mut choices {
            let fresh: Vec<&Modification> = pool
                .iter()
                .copied()
                .filter(|row| offered & (1 << row.kind) == 0)
                .collect();
            let from = if fresh.is_empty() { &pool } else { &fresh };
            let weights: Vec<u32> = from.iter().map(|row| weight(row.drop_weight)).collect();
            let picked = from[rng.pick(&weights)?];
            *choice = picked.id;
            offered |= 1 << picked.kind;
            pool.retain(|row| row.id != picked.id);
        }
        Some(choices)
    }

    /// The defects `dict_id` may get beside those it has: of the types its
    /// mask names, none of a type it has, and with the modification a
    /// defect requires among `modifications` (the item's own; its defaults
    /// are counted too).
    pub fn defect_candidates(
        &self,
        dict_id: u16,
        modifications: &[u32],
        defects: &[u32],
    ) -> Vec<&Defect> {
        let Some(item) = item_mods(dict_id) else {
            return Vec::new();
        };
        let defaults = item_params(dict_id).map_or(&[][..], |params| params.default_modifications);
        let has = |low: u32, high: u32| {
            modifications
                .iter()
                .chain(defaults)
                .any(|id| (low..=high).contains(id))
        };
        let taken = defects
            .iter()
            .filter_map(|&id| self.defect(id))
            .fold(0_u32, |mask, defect| mask | 1 << defect.kind);
        self.defects
            .iter()
            .filter(|row| {
                row.drop_weight > 0.0
                    && item.defects & (1 << row.kind) != 0
                    && taken & (1 << row.kind) == 0
                    && row
                        .required_modifications
                        .is_none_or(|(low, high)| has(low, high))
            })
            .collect()
    }

    /// The chance, of 1, that an item at `condition` (0..=1) with
    /// `defects` defects gets one more: that of the first slot it has no
    /// defect in; 0 with every slot taken.
    pub fn defect_chance(&self, condition: f32, defects: usize) -> f32 {
        self.defect_slots
            .get(defects)
            .map_or(0.0, |slot| slot.chance(condition))
    }

    /// The roll made for an item a match wore down to `condition` (0..=1):
    /// a defect for its first free slot, by that slot's chance, among the
    /// defects the item may get, by drop weight.
    pub fn roll_defect(
        &self,
        dict_id: u16,
        modifications: &[u32],
        defects: &[u32],
        condition: f32,
        rng: &mut Rng,
    ) -> Option<u32> {
        let chance = self.defect_chance(condition, defects.len());
        if chance <= 0.0 || unit(rng) >= chance {
            return None;
        }
        let candidates = self.defect_candidates(dict_id, modifications, defects);
        let weights: Vec<u32> = candidates
            .iter()
            .map(|row| weight(row.drop_weight))
            .collect();
        rng.pick(&weights).map(|index| candidates[index].id)
    }

    /// What the item's wear is multiplied by: the product of `1 + value`
    /// of its modifications' and defects' changes to `parameter`
    /// (`degrade_per_match`, `degrade_per_use`, `degrade_per_death`; only
    /// the "reliability" modifications change them). `modifications` are
    /// the item's `{id, state}`; its defaults are counted too.
    pub fn wear_factor(&self, dict_id: u16, modifications: &[(u32, bool)], parameter: &str) -> f32 {
        let defaults = item_params(dict_id).map_or(&[][..], |params| params.default_modifications);
        let path = format!("parameters/{parameter}");
        let own = modifications.iter().copied();
        let all = own.chain(defaults.iter().map(|&id| (id, false)));
        let mut factor = 1.0_f32;
        for (id, state) in all {
            let modifiers = if state {
                self.defect(id).map(|row| &row.modifiers)
            } else {
                self.modification(id).map(|row| &row.modifiers)
            };
            for modifier in modifiers.into_iter().flatten() {
                if modifier.path == path && modifier.kind == 2 {
                    factor *= 1.0 + modifier.value;
                }
            }
        }
        factor.max(0.0)
    }

    /// What the modifications of one item add to a match's pay: its own
    /// and its defaults (in the game's data the premium copies of weapons
    /// and armour carried 1230..=1280 as defaults; srp makes them their own
    /// modifications, `super::golden`).
    pub fn bonuses(&self, dict_id: u16, modifications: &[(u32, bool)]) -> Bonuses {
        let defaults = item_params(dict_id).map_or(&[][..], |params| params.default_modifications);
        let own = modifications
            .iter()
            .filter(|&&(_, state)| !state)
            .map(|&(id, _)| id);
        let mut bonuses = Bonuses::default();
        for row in own
            .chain(defaults.iter().copied())
            .filter_map(|id| self.modification(id))
        {
            for modifier in row.modifiers.iter().filter(|modifier| modifier.kind == 1) {
                match modifier.path.as_str() {
                    "parameters/money_bonus" => bonuses.money += modifier.value,
                    "parameters/experience_bonus" => bonuses.experience += modifier.value,
                    // One parameter for the four factions: which one is in
                    // the modification's `lobby_info` name.
                    "parameters/reputation_bonus" => {
                        let faction = row.lobby_info.iter().find_map(|(name, _)| {
                            ["scav_", "bm_", "ren_", "edge_"]
                                .iter()
                                .position(|prefix| name.starts_with(prefix))
                        });
                        if let Some(faction) = faction {
                            bonuses.reputation[faction] += modifier.value;
                        }
                    }
                    _ => {}
                }
            }
        }
        bonuses
    }
}

#[cfg(test)]
mod tests;
