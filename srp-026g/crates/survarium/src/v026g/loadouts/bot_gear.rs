//! The server's bots' gear at every equipment level (0 to 15): the parts a
//! bot takes of a player's kit ([`KitParts`]), the kit it makes of them
//! ([`bot_kit_of`]) and the rules every bot kit is held to
//! ([`kit_problems`]). Which players' kits a level's bots take is
//! `session::v026g::bot_kits`; what a bot wears when no player has a kit
//! of its role is its role's fixed kit (`super::bot_kits`).
//!
//! A bot takes of a player's kit a weapon of its role's category
//! (`BotRole::categories`: weapon 1, and weapon 2 for another role) with
//! the ammo the kit carries most of for it, and the armour (slots 0 to 6),
//! premium pieces too (no rule of the game keeps them off a kit). It
//! carries no
//! weapon 2 (the owner's word, 2026-10-16: only primary weapons, and the
//! most ammo), the role's rounds in each of [`BOT_AMMO_SLOTS`] (fewer where
//! the weight wants), and its own pockets ([`bot_kit_of`]): what the
//! match server's plays use, not the player's. Items only: none of the
//! player's modifications or upgrades, and nothing of the player.

use binary_config_parser::tree::ConfigValue;

use super::super::dictionary::Dictionary;
use super::super::item_params;
use super::{
    BOT_AMMO_SLOTS, BOT_BANDAGES, BOT_GRENADE, BOT_MEDKIT, BOT_RADIO, BotKitItem, BotRole, Loadout,
    POCKET_SLOTS, SlotItem, gear_level, pockets,
};

/// The weapon categories of the dictionary (`categories_dict`, the shop's
/// weapon tabs): a bot's role is by its weapon's ([`BotRole::categories`]).
/// Read in the data: the AK-74U and the AS Val are of the assault rifles,
/// the PPSh-41 of the SMGs, the SVD and the Mosin of the sniper rifles,
/// the RPD and the RPK of the machine guns, the TT-33 of the pistols
/// (`bot_gear` tests).
pub mod category {
    pub const ASSAULT_RIFLES: u8 = 8;
    pub const SNIPER_RIFLES: u8 = 13;
    pub const SHOTGUNS: u8 = 14;
    pub const SMGS: u8 = 15;
    pub const MACHINE_GUNS: u8 = 16;
    pub const PISTOLS: u8 = 17;
}

/// What the bots' kits carry over the 30 kg of a player: the
/// `max_carried_weight` modifier of skill 209 at its five ranks, +2 kg
/// each (`skills` tests), worn in every bot kit's modifier block
/// ([`bot_modifiers`]). A top player's kit of the highest levels weighs
/// more than 30 kg (Freedom's exoskeleton suit alone is 33 kg).
pub const BOT_CARRY_BONUS: f32 = 10.0;

/// The modifier index of `max_carried_weight` (`skills::MODIFIER_NAMES`).
pub const MAX_CARRIED_WEIGHT_MODIFIER: usize = 5;

/// What a bot carries: a player's limit and [`BOT_CARRY_BONUS`].
pub fn bot_carry_limit() -> f32 {
    item_params::player().max_carried_weight + BOT_CARRY_BONUS
}

/// The modifier block of a bot's kits: [`BOT_CARRY_BONUS`].
pub fn bot_modifiers() -> [f32; super::super::skills::MODIFIERS] {
    let mut modifiers = [0.0; super::super::skills::MODIFIERS];
    modifiers[MAX_CARRIED_WEIGHT_MODIFIER] = BOT_CARRY_BONUS;
    modifiers
}

/// The armour slots: helmet 0, mask 1, torso 2, back 3, legs 4, gloves 5,
/// boots 6.
const ARMOUR_SLOTS: std::ops::RangeInclusive<u8> = 0..=6;

/// The ammo slots of a kit (both weapons'; one pool, the client's table at
/// 0x9266A8).
const AMMO_SLOTS: [u8; 4] = [8, 9, 11, 12];

/// What a bot of a role takes of a player's kit: a weapon of the role's
/// class, the ammo for it and the armour. Made only by [`Self::of_kit`].
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct KitParts {
    pub role: BotRole,
    pub weapon: u16,
    pub ammo: u16,
    /// The pieces of slots 0 to 6, by slot (each one unit).
    pub armour: Vec<(u8, u16)>,
}

impl KitParts {
    /// The parts of a player's kit, one for each role a weapon of it is of
    /// the class of: weapon 1 first, then weapon 2 where it is of another
    /// role and fits weapon 1's slot; each with the ammo of the kit that
    /// fits it and that the kit holds most units of (the lower dict id of
    /// two alike). A weapon with no ammo in the kit gives none.
    pub fn of_kit(kit: &Loadout) -> Vec<KitParts> {
        let dictionary = Dictionary::shared();
        let armour: Vec<(u8, u16)> = kit
            .items
            .iter()
            .filter(|item| ARMOUR_SLOTS.contains(&item.slot))
            .map(|item| (item.slot, item.dict_id))
            .collect();
        let mut parts: Vec<KitParts> = Vec::new();
        for slot in [7_u8, 10] {
            let Some(weapon) = kit.slot(slot) else {
                continue;
            };
            let Some(role) = BotRole::of_weapon(weapon.dict_id) else {
                continue;
            };
            if parts.iter().any(|each| each.role == role) || !dictionary.fits(weapon.dict_id, 7) {
                continue;
            }
            let ammo = AMMO_SLOTS
                .iter()
                .filter_map(|&slot| kit.slot(slot))
                .filter(|ammo| {
                    ammo.count > 0 && dictionary.compatible(weapon.dict_id, ammo.dict_id)
                })
                .fold(None, |best: Option<&SlotItem>, ammo| match best {
                    Some(kept)
                        if (kept.count, std::cmp::Reverse(kept.dict_id))
                            >= (ammo.count, std::cmp::Reverse(ammo.dict_id)) =>
                    {
                        Some(kept)
                    }
                    _ => Some(ammo),
                });
            let Some(ammo) = ammo else {
                continue;
            };
            let mut armour = armour.clone();
            armour.sort_unstable();
            parts.push(KitParts {
                role,
                weapon: weapon.dict_id,
                ammo: ammo.dict_id,
                armour,
            });
        }
        parts
    }
}

/// What a bot fills its pockets with, in this order, as far as its armour
/// opens pockets ([`pockets`]): a medkit (quick use 1, the heal play's;
/// bandages in a kit of a level under the medkit's), frag grenades (the
/// grenade play's), the radio (it works carried: no play needs it). An
/// item goes in only when the kit is of its level or higher, so the
/// pockets never raise a kit's level. The match server's plays use what
/// the pockets hold (`match-server` `bots::guns::Arms::of_kit`).
const POCKET_CHOICES: [&[BotKitItem]; 3] =
    [&[BOT_MEDKIT, BOT_BANDAGES], &[BOT_GRENADE], &[BOT_RADIO]];

/// A bot's kit of `armour` and `weapon` with its `ammo`, the rounds of a
/// bot of `role` ([`BotRole::rounds_a_slot`], fewer to the ten where the
/// carry limit wants: at least a magazine and the category's minimum), and
/// the pockets the armour opens ([`POCKET_CHOICES`]). The error: what
/// [`kit_problems`] finds wrong with the kit.
pub fn bot_kit_of(
    armour: &[(u8, u16)],
    weapon: u16,
    ammo: u16,
    role: BotRole,
) -> Result<Loadout, Vec<String>> {
    let dictionary = Dictionary::shared();
    let mut items = vec![SlotItem {
        slot: 7,
        dict_id: weapon,
        count: 1,
    }];
    items.extend(armour.iter().map(|&(slot, dict_id)| SlotItem {
        slot,
        dict_id,
        count: 1,
    }));
    let level = gear_level(
        items
            .iter()
            .map(|item| (item.slot, item.dict_id, 1))
            .chain([(BOT_AMMO_SLOTS[0], ammo, 1)]),
    );
    let (count, size) = pockets(&Loadout {
        items: items.clone(),
    });
    let mut open = POCKET_SLOTS.iter().take(count as usize);
    for choices in POCKET_CHOICES {
        let Some(item) = choices.iter().find_map(|choice| {
            let item = dictionary.item_named(choice.name)?;
            let params = item_params::item_params(item.dict_id)?;
            (params.match_making_level <= level).then_some((item.dict_id, choice.count, params))
        }) else {
            continue;
        };
        let Some(&slot) = open.next() else {
            break;
        };
        let (dict_id, units, params) = item;
        items.push(SlotItem {
            slot,
            dict_id,
            count: if params.fills_whole_pocket {
                1
            } else {
                units.min(size)
            },
        });
    }
    let rest: f32 = items
        .iter()
        .map(|item| item_params::carried_weight(item.dict_id, item.count))
        .sum();
    let per_round = item_params::carried_weight(ammo, 1);
    let slots = BOT_AMMO_SLOTS.len() as u32;
    let fit = if per_round > 0.0 {
        ((bot_carry_limit() - rest) / (per_round * slots as f32)).max(0.0) as u32
    } else {
        u32::MAX
    };
    let rounds = role.rounds_a_slot().min(fit / 10 * 10);
    items.extend(BOT_AMMO_SLOTS.iter().map(|&slot| SlotItem {
        slot,
        dict_id: ammo,
        count: rounds,
    }));
    let kit = Loadout { items };
    let problems = kit_problems(&kit, bot_carry_limit());
    if problems.is_empty() {
        Ok(kit)
    } else {
        Err(problems)
    }
}

/// What a weapon's config says of how it fires and reloads
/// (`gameplay/items/weapons/wpn_<name>.options`): for the match server's
/// bots, which count the rounds they fire (`match-server` `bots::guns`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeaponFacts {
    pub magazine: u32,
    pub rounds_per_minute: f32,
    pub reload_s: f32,
    /// The first fire mode's queue: -1 automatic, else rounds a press.
    pub first_queue: i32,
    pub show_s: f32,
    pub hide_s: f32,
    /// The reload state's `state_type`: 303 a magazine, 306 round by
    /// round, 311 two barrels, 314 a pistol's (with a reload from empty).
    pub reload_state: i32,
    /// It has a `chamber_a_round` state (a bolt or a pump).
    pub chamber: bool,
}

/// [`WeaponFacts`] of the weapon `dict_id`; none for an item that is no
/// weapon or whose config the game data lacks or says none of them.
pub fn weapon_facts(dict_id: u16) -> Option<WeaponFacts> {
    let item = Dictionary::shared().item(dict_id)?;
    let config =
        ConfigValue::parse(super::super::game_data::shared().file(&item.cfg_name)?).ok()?;
    let parameters = config.get("parameters")?;
    let number = |key: &str| parameters.get(key).and_then(ConfigValue::as_f32);
    let states = config.get("states")?;
    Some(WeaponFacts {
        magazine: number("magazine_capacity")? as u32,
        rounds_per_minute: number("rounds_per_minute")?,
        reload_s: number("reload_time")?,
        first_queue: parameters
            .get("fire_queue_types")
            .and_then(|queues| queues.children().first().and_then(|queue| queue.as_i32()))
            .unwrap_or(1),
        show_s: number("show_time").unwrap_or(0.8),
        hide_s: number("hide_time").unwrap_or(0.8),
        reload_state: states
            .get("reload")
            .and_then(|state| state.get("state_type"))
            .and_then(ConfigValue::as_i32)
            .unwrap_or(303),
        chamber: states.get("chamber_a_round").is_some(),
    })
}

/// What is wrong with a kit a bot would wear, by the rules a kit is held
/// to: every item is one the game data has and fits its slot (the
/// dictionary's `slots_restrictions`), one unit of an item that is no
/// stack; no item is worn in a slot another worn item covers (the lobby's
/// Play check, `CoveredSlot`); no item type is in two pockets (its
/// `PocketTaken`); weapon 1 is in slot 7, of a role's class, with the facts
/// the match server's bots fire it by ([`weapon_facts`]), and no weapon 2;
/// every ammo slot holds ammo weapon 1 fires, together at least its
/// category's `minimum_count` and a magazine (the Play check's `NoAmmo`);
/// the pocket items are in open pockets, a stack at most a pocket's units
/// ([`pockets`]); the kit weighs at most `limit`. Empty: none.
pub fn kit_problems(loadout: &Loadout, limit: f32) -> Vec<String> {
    let dictionary = Dictionary::shared();
    let mut problems = Vec::new();
    let name = |dict_id: u16| {
        dictionary
            .item(dict_id)
            .map_or_else(|| format!("#{dict_id}"), |item| item.name.clone())
    };
    let mut slots: Vec<u8> = loadout.items.iter().map(|item| item.slot).collect();
    slots.sort_unstable();
    slots.dedup();
    if slots.len() != loadout.items.len() {
        problems.push("two items in one slot".to_owned());
    }
    for item in &loadout.items {
        let Some(entry) = dictionary.item(item.dict_id) else {
            problems.push(format!("no item #{}", item.dict_id));
            continue;
        };
        if !dictionary.fits(item.dict_id, item.slot) {
            problems.push(format!("{} does not fit slot {}", entry.name, item.slot));
        }
        if !entry.is_stack && item.count != 1 {
            problems.push(format!("{} x{}", entry.name, item.count));
        }
        for &covered in item_params::covered_slots(item.dict_id) {
            if let Some(under) = loadout.slot(covered) {
                problems.push(format!(
                    "{} covers slot {covered}, where {} is",
                    entry.name,
                    name(under.dict_id)
                ));
            }
        }
    }
    let mut pocket_types: Vec<u16> = loadout
        .items
        .iter()
        .filter(|item| POCKET_SLOTS.contains(&item.slot))
        .map(|item| item.dict_id)
        .collect();
    let in_pockets = pocket_types.len();
    pocket_types.sort_unstable();
    pocket_types.dedup();
    if pocket_types.len() != in_pockets {
        problems.push("an item type in two pockets".to_owned());
    }
    match loadout.slot(7) {
        None => problems.push("no weapon 1".to_owned()),
        Some(weapon) => {
            if BotRole::of_weapon(weapon.dict_id).is_none() {
                problems.push(format!("{} is of no role's category", name(weapon.dict_id)));
            }
            if weapon_facts(weapon.dict_id).is_none() {
                problems.push(format!("{} has no weapon facts", name(weapon.dict_id)));
            }
            let mut rounds = 0_u64;
            for slot in AMMO_SLOTS {
                if let Some(ammo) = loadout.slot(slot) {
                    if dictionary.compatible(weapon.dict_id, ammo.dict_id) {
                        rounds += u64::from(ammo.count);
                    } else {
                        problems.push(format!(
                            "{} in slot {slot} does not fit {}",
                            name(ammo.dict_id),
                            name(weapon.dict_id)
                        ));
                    }
                }
            }
            let category = dictionary.item(weapon.dict_id).map(|item| item.category);
            let minimum = dictionary
                .categories
                .iter()
                .find(|each| Some(each.id) == category)
                .map_or(0, |each| u64::from(each.minimum_count));
            let magazine = item_params::item_params(weapon.dict_id)
                .map_or(0, |params| u64::from(params.magazine_capacity));
            if rounds < minimum.max(magazine) {
                problems.push(format!(
                    "{} has {rounds} rounds (minimum {minimum}, magazine {magazine})",
                    name(weapon.dict_id)
                ));
            }
        }
    }
    if loadout.slot(10).is_some() {
        problems.push("a weapon 2".to_owned());
    }
    let (count, size) = pockets(loadout);
    for item in loadout
        .items
        .iter()
        .filter(|item| POCKET_SLOTS.contains(&item.slot))
    {
        if !POCKET_SLOTS[..count as usize].contains(&item.slot) {
            problems.push(format!(
                "{} in closed pocket {}",
                name(item.dict_id),
                item.slot
            ));
        }
        let whole = item_params::item_params(item.dict_id).is_some_and(|p| p.fills_whole_pocket);
        let most = if whole { 1 } else { size };
        if item.count == 0 || item.count > most {
            problems.push(format!(
                "{} x{} in a pocket of {most}",
                name(item.dict_id),
                item.count
            ));
        }
    }
    if loadout.weight() > limit {
        problems.push(format!("{:.2} kg over {limit} kg", loadout.weight()));
    }
    problems
}

#[cfg(test)]
mod tests;
