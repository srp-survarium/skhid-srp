//! Golden weapons: the gold-bought copies of weapons (`*_premium`,
//! `*_premium_2`, the Neostead, the MP7 special),
//! `docs/notes/golden_weapons.md`.
//!
//! The owner's rule (2026-10-21): a golden weapon keeps only its first
//! bonus, +10 % silver ([`SILVER_BONUS`]), and is upgraded like any
//! weapon. In the game's data each had its bonuses as
//! `default_modifications` (the silver one first, beside two weapon
//! bonuses; the MP7 special one weapon bonus and no silver) and
//! `base_rarity_level` 3: the lobby showed it at rank 3, so it could not be
//! crafted, and a default is never offered for a reroll. The resources.db
//! patch of `scripts/content-edit/golden-weapons-1.json` takes every
//! default off them and sets the base rarity to 0, as the plain weapons
//! have; srp gives each golden weapon the silver bonus as a modification of
//! its own, the first, when it is made ([`starting_modifications`]) and,
//! once, to those owned before (`session::v026g::items_rules::
//! give_golden_silver`). From then on it is an ordinary modification: it
//! counts for the rank, a reroll may replace it, a failed craft may take
//! it.
//!
//! In a craft any copy of the same plain weapon is a donor for any other:
//! its plain weapon, a golden one of it, both ways ([`may_donate`]).
//!
//! The premium armour (the Bandit and Survivor jackets and trousers,
//! [`PREMIUM_ARMOUR`]; the owner, 2026-10-22: the golden weapons'
//! treatment, `docs/notes/premium_armour.md`) had its plain piece's one
//! default and two premium bonuses (+5 % experience, +5 % reputation with
//! its faction) as defaults, and base rarity 3. The patch of
//! `scripts/content-edit/premium-armour-1.json` takes the two premium
//! bonuses off and sets the base rarity to 0: the plain piece's default
//! stays a default, as the plain piece has it; the two bonuses are the
//! piece's own first modifications, given when it is made and, once, to
//! those owned before (`session::v026g::items_rules::
//! give_premium_armour_bonuses`). Both groups are "golden" items here: the
//! gold-bought copy of a plain item ([`is_golden`]).
//!
//! srp applies the rule to whatever data it reads ([`super::item_params`]
//! gives a golden weapon no defaults and base rarity 0), and refuses to
//! start on data whose golden items still have them ([`check`]): the
//! players' lobby would show the bonuses twice and rank 3.

use binary_config_parser::tree::ConfigValue;

use super::dictionary::{self, Dictionary};
use super::game_data::{GameData, GameDataError};

mod content;

/// Modification 1230: `parameters/money_bonus` +0.1, the first default of
/// the golden weapons in the game's data.
pub const SILVER_BONUS: u32 = 1230;

/// The golden weapons: `(name, its plain weapon's name, dict id, the plain
/// weapon's dict id)`, by the dictionary of revision 26. Every weapon whose
/// config had [`SILVER_BONUS`] as its first default modification, and the
/// MP7 special (gold-bought, one default: 2004; the owner, 2026-10-21: it
/// joins them). Not the VSSK camo (2054, 2074, 2094; not premium). The
/// plain weapon is the same weapon bought for silver: the Neostead's is
/// `neostead_2` (the golden one has the camo model), the MP7 special's the
/// silenced MP7. srp goes by the names; the dict ids are what the lobby
/// keeps (`ItemDictId.GOLDEN_WEAPONS`, `GOLDEN_PLAIN`), and a test holds
/// both to the dictionary.
pub const GOLDEN_WEAPONS: [(&str, &str, u16, u16); 22] = [
    ("kiparis_premium", "kiparis", 332, 88),
    ("uzi_premium", "uzi", 333, 55),
    ("vityaz_premium", "vityaz", 334, 19),
    ("vss_premium", "vss", 335, 308),
    ("sr3m_premium", "sr3m", 336, 309),
    ("izh_27_lancaster_premium", "izh_27_lancaster", 337, 108),
    ("ak_74n_premium", "ak_74n", 338, 306),
    ("wpn_mp7_silencer_special", "mp7_silencer", 2222, 2221),
    ("vepr_12_premium", "vepr_12", 2226, 2225),
    ("saiga_12_premium", "saiga_12", 2228, 2227),
    ("neostead", "neostead_2", 2231, 2230),
    ("akmn_premium", "akmn", 2241, 80),
    ("akmn_premium_2", "akmn", 2242, 80),
    ("ak_74u_premium", "ak_74u", 2243, 13),
    ("vepr_premium", "vepr", 2244, 216),
    ("ak_12_premium", "ak_12", 2245, 363),
    ("sks_premium", "sks", 2246, 109),
    ("sks_premium_2", "sks", 2247, 109),
    ("vsk_94_premium", "vsk_94", 2248, 305),
    ("rem_700_premium", "rem_700", 2249, 14),
    ("izh_27_lancaster_premium_2", "izh_27_lancaster", 2251, 108),
    ("uzi_premium_2", "uzi", 2252, 55),
];

/// Modification 1240: `parameters/experience_bonus` +0.05, on each piece
/// of [`PREMIUM_ARMOUR`].
pub const EXPERIENCE_BONUS: u32 = 1240;

/// Modification 1250: `parameters/reputation_bonus` +0.05 with the
/// Scavengers (`scav_rep_booster`), on the Survivor pieces.
pub const SCAVENGER_REPUTATION_BONUS: u32 = 1250;

/// Modification 1260: `parameters/reputation_bonus` +0.05 with the Black
/// Market (`bm_rep_booster`), on the Bandit pieces.
pub const BLACK_MARKET_REPUTATION_BONUS: u32 = 1260;

/// The premium armour: `(name, its plain piece's name, dict id, the plain
/// piece's dict id, its premium bonuses)`, by the dictionary of revision
/// 26. Every armour piece with a base rarity (3) in the game's data: the
/// Bandit bundle's (8) jacket and trousers and the Survivor bundle's (14).
/// Their configs had the plain piece's one default (a pocket, an ammo bag,
/// 2 % less stamina spent) and then these two; the plain piece is the same
/// piece without them. The gold-sold `scav_torso_4` and `scav_legs_4` are
/// those plain pieces: base rarity 0, no premium bonus, not here. srp goes
/// by the names; the dict ids and bonuses are what the lobby keeps
/// (`ItemDictId.PREMIUM_ARMOUR`, `PREMIUM_ARMOUR_PLAIN`,
/// `PREMIUM_ARMOUR_BONUSES`), and a test holds them to the dictionary and
/// the data.
pub const PREMIUM_ARMOUR: [(&str, &str, u16, u16, [u32; 2]); 4] = [
    (
        "scav_torso_4_premium",
        "scav_torso_4",
        345,
        325,
        [EXPERIENCE_BONUS, SCAVENGER_REPUTATION_BONUS],
    ),
    (
        "scav_legs_4_premium",
        "scav_legs_4",
        346,
        326,
        [EXPERIENCE_BONUS, SCAVENGER_REPUTATION_BONUS],
    ),
    (
        "bm_torso_4_premium",
        "bm_torso_4",
        352,
        351,
        [EXPERIENCE_BONUS, BLACK_MARKET_REPUTATION_BONUS],
    ),
    (
        "bm_legs_4_premium",
        "bm_legs_4",
        354,
        353,
        [EXPERIENCE_BONUS, BLACK_MARKET_REPUTATION_BONUS],
    ),
];

/// Whether `name` (a dictionary name) is a golden weapon.
pub fn is_golden_weapon_name(name: &str) -> bool {
    GOLDEN_WEAPONS.iter().any(|&(golden, ..)| golden == name)
}

/// The premium bonuses of the premium armour piece `name` (a dictionary
/// name); `None` for anything else.
pub fn armour_bonuses(name: &str) -> Option<&'static [u32]> {
    legacy_armour_bonuses(name).or_else(|| {
        content::shared()
            .get(name)
            .map(|item| item.bonuses.as_slice())
    })
}

/// Stock premiums whose unpatched retail defaults are normalized by item_params.
/// Pack premiums already have clean defaults, checked by content::load.
pub(super) fn legacy_armour_bonuses(name: &str) -> Option<&'static [u32]> {
    PREMIUM_ARMOUR
        .iter()
        .find(|&&(premium, ..)| premium == name)
        .map(|(.., bonuses)| &bonuses[..])
}

/// Whether `name` (a dictionary name) is a golden item: a golden weapon or
/// a premium armour piece.
pub fn is_golden_name(name: &str) -> bool {
    is_golden_weapon_name(name) || armour_bonuses(name).is_some()
}

/// Whether `dict_id` is a golden item: a golden weapon or a premium armour
/// piece.
pub fn is_golden(dict_id: u16) -> bool {
    Dictionary::shared()
        .item(dict_id)
        .is_some_and(|item| is_golden_name(&item.name))
}

/// Whether `dict_id` is a golden weapon.
pub fn is_golden_weapon(dict_id: u16) -> bool {
    Dictionary::shared()
        .item(dict_id)
        .is_some_and(|item| is_golden_weapon_name(&item.name))
}

/// Whether `dict_id` is a premium armour piece.
pub fn is_premium_armour(dict_id: u16) -> bool {
    Dictionary::shared()
        .item(dict_id)
        .is_some_and(|item| armour_bonuses(&item.name).is_some())
}

/// The plain item's name a golden item's `name` is a copy of:
/// `akmn_premium_2` -> `akmn`, `neostead` -> `neostead_2`,
/// `bm_torso_4_premium` -> `bm_torso_4`. `None` for any other name.
pub fn plain_name(name: &str) -> Option<&'static str> {
    GOLDEN_WEAPONS
        .iter()
        .find(|&&(golden, ..)| golden == name)
        .map(|&(_, plain, ..)| plain)
        .or_else(|| {
            PREMIUM_ARMOUR
                .iter()
                .find(|&&(premium, ..)| premium == name)
                .map(|&(_, plain, ..)| plain)
        })
        .or_else(|| {
            content::shared()
                .get(name)
                .map(|item| item.plain_name.as_str())
        })
}

/// The item `dict_id` counts as in a craft: a golden item's plain item,
/// when the dictionary has it; anything else itself.
pub fn kin(dict_id: u16) -> u16 {
    kin_in(Dictionary::shared(), dict_id)
}

fn kin_in(dictionary: &Dictionary, dict_id: u16) -> u16 {
    dictionary
        .item(dict_id)
        .and_then(|item| plain_name(&item.name))
        .and_then(|plain| dictionary.item_named(plain))
        .map_or(dict_id, |plain| plain.dict_id)
}

/// Whether an item of `donor` may be used up to upgrade one of `item`:
/// another of the same item, or (the owner's rules, 2026-10-21 and
/// 2026-10-22; the premium armour too) any item of the same plain item:
/// a golden item's plain item, a golden item of the same plain item
/// (`akmn_premium` and `akmn_premium_2`), and a golden item for a plain one
/// (the owner: "if I press upgrade on lancaster, I should get inside
/// premium lancasters too"; the golden one is used up). The lobby's craft
/// window lists the same (`ItemFactory.isCraftDonor`).
pub fn may_donate(donor: u16, item: u16) -> bool {
    donor == item || kin(donor) == kin(item)
}

/// The modifications of its own a new item of `dict_id` has: the silver
/// bonus for a golden weapon, its two premium bonuses for a premium armour
/// piece, none for anything else.
pub fn starting_modifications(dict_id: u16) -> Vec<(u32, bool)> {
    let Some(item) = Dictionary::shared().item(dict_id) else {
        return Vec::new();
    };
    if is_golden_weapon_name(&item.name) {
        vec![(SILVER_BONUS, false)]
    } else if let Some(bonuses) = armour_bonuses(&item.name) {
        bonuses.iter().map(|&id| (id, false)).collect()
    } else {
        Vec::new()
    }
}

/// The patch that takes a golden weapon's defaults off.
pub const WEAPONS_PATCH: &str = "scripts/content-edit/golden-weapons-1.json";

/// The patch that takes a premium armour piece's premium bonuses off.
pub const ARMOUR_PATCH: &str = "scripts/content-edit/premium-armour-1.json";

/// The golden items of `data` made without their patch, each as
/// `(<name> (<config>), the patch)`: a golden weapon whose config still
/// has default modifications or a base rarity, a premium armour piece
/// whose config still has one of its premium bonuses as a default or a
/// base rarity (the plain piece's default stays).
pub fn unpatched(
    data: &GameData,
    dictionary: &Dictionary,
) -> Result<Vec<(String, &'static str)>, GameDataError> {
    // Validate pack metadata against this exact data, not the process-global dictionary.
    content::load(data, dictionary)?;
    let mut found = Vec::new();
    for item in dictionary.items.iter().filter(|item| {
        is_golden_weapon_name(&item.name) || PREMIUM_ARMOUR.iter().any(|row| row.0 == item.name)
    }) {
        let config = super::game_data::config(data, &item.cfg_name)?
            .ok_or_else(|| GameDataError(format!("{}: no {}", item.name, item.cfg_name)))?;
        let defaults: Vec<i32> = config
            .get("default_modifications")
            .map(|list| {
                list.children()
                    .into_iter()
                    .filter_map(ConfigValue::as_i32)
                    .collect()
            })
            .unwrap_or_default();
        let rarity = config
            .get("parameters")
            .and_then(|p| p.get("base_rarity_level"))
            .and_then(ConfigValue::as_i32)
            .unwrap_or(0);
        let (defaults_left, patch) = match legacy_armour_bonuses(&item.name) {
            Some(bonuses) => (
                defaults
                    .iter()
                    .any(|&id| u32::try_from(id).is_ok_and(|id| bonuses.contains(&id))),
                ARMOUR_PATCH,
            ),
            None => (!defaults.is_empty(), WEAPONS_PATCH),
        };
        if defaults_left || rarity != 0 {
            found.push((format!("{} ({})", item.name, item.cfg_name), patch));
        }
    }
    Ok(found)
}

/// Refuses `data` whose golden items still have their defaults
/// ([`unpatched`]).
pub fn check(data: &GameData) -> Result<(), GameDataError> {
    let dictionary = Dictionary::parse(
        data.file(dictionary::PATH)
            .ok_or_else(|| GameDataError(format!("{}: no {}", data.origin, dictionary::PATH)))?,
    )
    .map_err(|cause| GameDataError(format!("{}: {}", dictionary::PATH, cause.0)))?;
    let found = unpatched(data, &dictionary)?;
    if found.is_empty() {
        return Ok(());
    }
    let mut problems = Vec::new();
    for (patch, what, gives) in [
        (
            WEAPONS_PATCH,
            "golden weapons with default modifications or a base rarity",
            "the silver bonus",
        ),
        (
            ARMOUR_PATCH,
            "premium armour with its premium bonuses as defaults or a base rarity",
            "the experience and reputation bonuses",
        ),
    ] {
        let items: Vec<&str> = found
            .iter()
            .filter(|(_, of)| *of == patch)
            .map(|(item, _)| item.as_str())
            .collect();
        if !items.is_empty() {
            problems.push(format!(
                "{what}: {}. The data was made without the patch {patch}: srp gives them {gives} \
                 as their own modifications, and the players' lobby would show them twice and \
                 rank 3",
                items.join(", ")
            ));
        }
    }
    Err(GameDataError(format!(
        "{}: {}; build the data with every patch (srp-game-data build)",
        data.origin,
        problems.join("; ")
    )))
}

#[cfg(test)]
mod tests;
