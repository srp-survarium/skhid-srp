//! The skills of the server's own bots ("Player vs Bots"), by tier: one
//! table, [`BOT_SKILL_TIERS`], keyed by the gear level of a bot's kits
//! (the matchmaking's measure). `docs/notes/player_bots_skills.md` has the
//! reasoning and the numbers.
//!
//! A bot's skills reach the match the way a player's do: summed into the
//! 68 player modifiers of each of its kits ([`SkillsTree::modifiers`],
//! `static_info.md` §3.1), which every client simulates from.
//!
//! **What carrying does** (the client, PROVEN in the exe unless marked;
//! `default.player`'s `stamina_params`):
//! - The capacity is `max_carried_weight` (30 kg) plus modifier 5
//!   `max_carried_weight`, which adds (table 0x927E08): skill 209 gives 2 kg
//!   a level, 10 kg at its five; some modifications add or take kilograms
//!   (1120.. +1..+4, the defect 4080 -3), counted through the item's nodes.
//! - From `min_carried_weight` (20 kg) to the capacity the weight costs
//!   `t = clamp((w - 20) / (capacity - 20), 0, 1)` (0x329B20): movement
//!   speed `-0.2 × t` (`max_carried_weight_movement_speed_modifier`, scaled
//!   by `1 + overweight_penalty`: skill 202 takes 5 % of it a level, a
//!   quarter at five; skill 214 removes it) and stamina spent `× (1 + 1.5 ×
//!   t)` (`max_carried_weight_spending_speed_modifier`).
//! - At or over the capacity the player is overburdened: no sprint
//!   (`player_stamina::is_overburdened` 0x329C60, read where a sprint is
//!   decided, 0x2DBF89), unless modifier 31 `can_sprint` (skill 213) is set.
//!
//! [`movement_level`] is the speed factor that follows (with skill 208's
//! movement speed), which the match server's bots take as a body's speed
//! level before a world has shown it.

use super::dictionary::Dictionary;
use super::item_params;
use super::loadouts::{Loadout, default_modifier};
use super::match_msgs::static_info::MatchProfile;
use super::modifications::{Modifications, Modifier};
use super::skills::{MODIFIERS, SkillsTree, modifier_index};

/// The player modifier `max_carried_weight` (index 5 of the 68).
pub const MAX_CARRIED_WEIGHT: usize = 5;

/// The skills of the bots whose kits' gear level is at most
/// [`BotSkillTier::up_to_gear_level`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BotSkillTier {
    /// The highest gear level ([`gear_level`]) of the kits this tier is for.
    pub up_to_gear_level: u8,
    /// The account level the points are spent at: a level gives a point,
    /// so this is the sum of `skills`' points.
    pub level: u8,
    /// `(skill id, points)`, a set the skills window would let a player of
    /// `level` have ([`SkillsTree::validate`]).
    pub skills: &'static [(u16, u8)],
}

/// The bots' skills by tier, lowest first; kits above every tier's
/// [`BotSkillTier::up_to_gear_level`] get the last.
///
/// Each tier is the build of the best players of the 0.26g server at that
/// gear level (the ten best by the hidden rating with 20 matches or more,
/// 2026-10-23; `player_bots_skills.md` §3): their skills by the points
/// they put in, each taken to five, at about their level. The carry path
/// (a row-1 skill at five, then 209 at five: 40 kg) is in every tier: the
/// owner's ask, and what all ten of the 11-15 players have. Left out, with
/// the reason in the note: 103 (the bots time a bolt's chambering by the
/// clip), 213 (two of ten). 102 (no speed in the client's aiming speed,
/// LIKELY) and 108 (the bots time a reload by the clip) are left out of
/// these rows and given only by the owner's kit ([`AMMO_POUCH`]), where
/// they are the way to skill 109.
#[rustfmt::skip]
pub const BOT_SKILL_TIERS: [BotSkillTier; 3] = [
    // Gear 1 to 5 (the starting roles): aim time, stamina regeneration,
    // the carry.
    BotSkillTier { up_to_gear_level: 5, level: 15, skills: &[
        (101, 5), (204, 5), (209, 5),
    ] },
    // Gear 6 to 10: and stamina spent.
    BotSkillTier { up_to_gear_level: 10, level: 20, skills: &[
        (101, 5), (201, 5), (204, 5), (209, 5),
    ] },
    // Gear 11 to 15: and movement speed and the overweight penalty.
    BotSkillTier { up_to_gear_level: 15, level: 30, skills: &[
        (101, 5), (201, 5), (204, 5), (209, 5), (202, 5), (208, 5),
    ] },
];

/// What the owner's kit ([`new_kit`](super::loadouts::new_kit)) adds to
/// every tier's skills, 11 points, for **skill 109**, the extra ammo bag
/// (`ammo_bags` +1): a pouch of ammo more, one more magazine of rounds.
/// 109 is in row 4 of the aim branch and needs 15 points in its rows 1 to
/// 3, which the tiers already have 5 of (101); the other ten are the least
/// that carry no aim, damage or protection skill (the rest of the rows
/// are 103 bolt cocking speed, 105 breath hold, 106 and 107 dispersion):
/// - 102 moving while aiming, 5: no speed (LIKELY, §5 of
///   `player_bots_skills.md`);
/// - 104 weapon swap speed, 2: the bots carry one weapon;
/// - 108 reload speed, 3: the client reloads 12 % faster; the bots time a
///   reload by the clip (`Gun::reload_ms`), so they wait a little longer
///   than needed unless the bots' upgrades are on (`SRP_BOT_UPGRADES`), where
///   the gun learns the factor (`guns::Factors`).
pub const AMMO_POUCH: &[(u16, u8)] = &[(102, 5), (104, 2), (108, 3), (109, 1)];

/// The points of [`AMMO_POUCH`].
pub const AMMO_POUCH_POINTS: u8 = 11;

impl BotSkillTier {
    /// Its skills with the owner's kit `pouch` ([`AMMO_POUCH`]) or without.
    pub fn skills_with(&self, pouch: bool) -> Vec<(u16, u8)> {
        let mut skills = self.skills.to_vec();
        if pouch {
            skills.extend_from_slice(AMMO_POUCH);
        }
        skills
    }

    /// Its account level with the owner's kit `pouch` or without.
    pub fn level_with(&self, pouch: bool) -> u8 {
        self.level + if pouch { AMMO_POUCH_POINTS } else { 0 }
    }

    /// The player modifiers its skills give with the owner's kit `pouch`
    /// or without.
    pub fn modifiers_with(&self, pouch: bool) -> [f32; MODIFIERS] {
        SkillsTree::shared().modifiers(&self.skills_with(pouch))
    }

    /// The tier of kits whose gear level is `gear_level`.
    pub fn for_gear_level(gear_level: u8) -> &'static BotSkillTier {
        BOT_SKILL_TIERS
            .iter()
            .find(|tier| gear_level <= tier.up_to_gear_level)
            .unwrap_or(&BOT_SKILL_TIERS[BOT_SKILL_TIERS.len() - 1])
    }

    /// The tier of a bot wearing `kits` ([`gear_level`]).
    pub fn for_kits(kits: &[&Loadout]) -> &'static BotSkillTier {
        Self::for_gear_level(gear_level(kits))
    }

    /// The player modifiers its skills give ([`Self::modifiers_with`],
    /// the owner's kit unless it is off).
    pub fn modifiers(&self) -> [f32; MODIFIERS] {
        self.modifiers_with(super::loadouts::new_kit())
    }
}

/// A kit's gear level as the matchmaking counts a player's
/// (`session::v026g::matchmaking::kit_level`): the highest
/// `match_making_level` of what it wears times its slot's weight, rounded.
pub fn kit_gear_level(kit: &Loadout) -> u8 {
    let weights = &Dictionary::shared().match_maker.slots_weight;
    let highest = kit
        .items
        .iter()
        .filter(|item| item.count > 0)
        .filter_map(|item| {
            let level = item_params::item_params(item.dict_id)?.match_making_level;
            let weight = weights.get(usize::from(item.slot)).copied().unwrap_or(1.0);
            Some(f32::from(level) * weight)
        })
        .fold(0.0_f32, f32::max);
    (highest + 0.5).floor().clamp(0.0, 255.0) as u8
}

/// The highest gear level of `kits` ([`kit_gear_level`]).
pub fn gear_level(kits: &[&Loadout]) -> u8 {
    kits.iter()
        .map(|kit| kit_gear_level(kit))
        .max()
        .unwrap_or(0)
}

/// What the type 1 (added) modifiers of `modifiers` add to `parameter`.
fn added(modifiers: &[Modifier], parameter: &str) -> f32 {
    modifiers
        .iter()
        .filter(|modifier| {
            modifier.kind == 1
                && modifier
                    .path
                    .strip_prefix("parameters/")
                    .is_some_and(|path| path == parameter)
        })
        .map(|modifier| modifier.value)
        .sum()
}

/// What an item adds to `parameter` (`max_carried_weight_modifier`,
/// `movement_speed_modifier`): its default modifications, which the client
/// adds from the item's config, and `modifications`, the `(id, defect)` it
/// carries on the wire.
pub fn item_added(dict_id: u16, modifications: &[(u32, bool)], parameter: &str) -> f32 {
    let table = Modifications::shared();
    let carried: f32 = modifications
        .iter()
        .map(|&(id, defect)| {
            if defect {
                table
                    .defect(id)
                    .map_or(0.0, |row| added(&row.modifiers, parameter))
            } else {
                table
                    .modification(id)
                    .map_or(0.0, |row| added(&row.modifiers, parameter))
            }
        })
        .sum();
    default_modifier(dict_id, parameter) + carried
}

/// What `item_weight` multiplies an item's weight by: `1 +` the sum of the
/// type 2 (multiplying) `parameters/weight` changes of its `modifications`
/// (the weight upgrades, -5 % to -15 %; the client's rule, as
/// [`Modifier::kind`]). The path was `weight` once, which no modification
/// has, so no weight upgrade ever lightened a bot.
pub fn item_weight(dict_id: u16, count: u32, modifications: &[(u32, bool)]) -> f32 {
    let table = Modifications::shared();
    let change: f32 = modifications
        .iter()
        .filter(|&&(_, defect)| !defect)
        .filter_map(|&(id, _)| table.modification(id))
        .flat_map(|row| &row.modifiers)
        .filter(|modifier| modifier.kind == 2 && modifier.path == "parameters/weight")
        .map(|modifier| modifier.value)
        .sum();
    item_params::carried_weight(dict_id, count) * (1.0 + change).max(0.0)
}

/// A slot item's modifications as the wire gives them, `(id, defect)`.
pub fn wire_mods(mods: &[u32]) -> Vec<(u32, bool)> {
    mods.iter().map(|&id| (id, false)).collect()
}

/// What a player with `modifiers` (a kit's skills) carries in `kit` before
/// being overburdened: `max_carried_weight` plus the skills' and the worn
/// items' default modifications' `max_carried_weight` and the carry
/// upgrades they wear (see the module).
pub fn carry_capacity(kit: &Loadout, modifiers: &[f32; MODIFIERS]) -> f32 {
    let items: f32 = kit
        .items
        .iter()
        .map(|item| {
            item_added(
                item.dict_id,
                &wire_mods(item.mods),
                "max_carried_weight_modifier",
            )
        })
        .sum();
    item_params::player().max_carried_weight + modifiers[MAX_CARRIED_WEIGHT] + items
}

/// The weight's share of its cost, 0 to 1: `t` of the module.
pub fn weight_cost(weight: f32, capacity: f32) -> f32 {
    let min = item_params::player().min_carried_weight;
    if capacity <= min {
        return if weight > min { 1.0 } else { 0.0 };
    }
    ((weight - min) / (capacity - min)).clamp(0.0, 1.0)
}

/// The body's speed factor (`kin::Kinematics::level_of` of the match
/// server's bots): `1 + movement_speed` of the skills and the items'
/// modifications (`items_movement`), plus the weight's node (see the
/// module). A weapon's own `movement_speed_modifier` (±0.05) is the
/// caller's to add to `items_movement`
/// ([`profile_movement_level_in`] does, with `weapon` set).
pub fn movement_level(
    weight: f32,
    capacity: f32,
    modifiers: &[f32; MODIFIERS],
    items_movement: f32,
) -> f32 {
    let index = |name| modifier_index(name).expect("a modifier of the 68");
    let carried = if modifiers[index("max_carried_weight_ultimate")] > 0.0 {
        0.0
    } else {
        item_params::player().max_carried_weight_movement_speed_modifier
            * (1.0 + modifiers[index("overweight_penalty")]).max(0.0)
            * weight_cost(weight, capacity)
    };
    (1.0 + modifiers[index("movement_speed")] + items_movement + carried).max(0.0)
}

/// [`movement_level`] of a match profile's `kit` (0 on): its items' weight
/// and modifications, its modifiers; the held weapon's own speed modifier
/// when the bots' upgrades are on ([`super::loadouts::upgrades_on`]). `None`
/// without that kit.
pub fn profile_movement_level(profile: &MatchProfile, kit: usize) -> Option<f32> {
    profile_movement_level_in(profile, kit, super::loadouts::upgrades_on())
}

/// The slot of weapon 1 in a profile's kit.
const WEAPON_SLOT: usize = 7;

/// [`profile_movement_level`], with the weapon in slot 7 adding its own
/// `parameters/movement_speed_modifier` (LIKELY: the client adds it to the
/// player modifier 4 while the weapon is held) if `weapon`.
pub fn profile_movement_level_in(profile: &MatchProfile, kit: usize, weapon: bool) -> Option<f32> {
    let kit = profile.kits.get(kit)?;
    let (mut weight, mut capacity_added, mut movement) = (0.0, 0.0, 0.0);
    for (slot, &(index, count)) in kit
        .slots
        .iter()
        .enumerate()
        .filter_map(|(slot, entry)| entry.as_ref().map(|entry| (slot, entry)))
    {
        let Some(item) = profile.items.get(usize::from(index)) else {
            continue;
        };
        if weapon && slot == WEAPON_SLOT {
            movement += super::weapons::weapon(item.dict_id).map_or(0.0, |w| w.movement_speed_modifier);
        }
        weight += item_weight(item.dict_id, u32::from(count), &item.modifications);
        capacity_added += item_added(
            item.dict_id,
            &item.modifications,
            "max_carried_weight_modifier",
        );
        movement += item_added(item.dict_id, &item.modifications, "movement_speed_modifier");
    }
    let capacity = item_params::player().max_carried_weight
        + kit.modifiers[MAX_CARRIED_WEIGHT]
        + capacity_added;
    Some(movement_level(weight, capacity, &kit.modifiers, movement))
}

/// The modifiers of a bot wearing `kits`: its tier's skills.
pub fn bot_modifiers(kits: &[&Loadout]) -> [f32; MODIFIERS] {
    BotSkillTier::for_kits(kits).modifiers()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v026g::loadouts::{BotEquipment, BotRole, BotTier, RusherKit, bot_kits, bot_kits_in};
    use crate::v026g::skills::SkillError;

    /// The rusher kits of `tier` the game data in use can make.
    fn made_kits(tier: BotTier) -> impl Iterator<Item = RusherKit> {
        tier.rusher_kits()
            .iter()
            .copied()
            .filter(move |&kit| kit.worn_in(tier) == kit)
    }

    fn index(name: &str) -> usize {
        modifier_index(name).unwrap()
    }

    /// The weight upgrades lighten what they are on (the path was `weight`,
    /// which no modification has): gear 1061 and a weapon's 2061.
    #[test]
    fn a_weight_upgrade_lightens_the_piece() {
        let dictionary = Dictionary::shared();
        let table = Modifications::shared();
        for (name, id) in [("edge_torso_2", 1061), ("ak_74u", 2061)] {
            let dict_id = dictionary.item_named(name).unwrap().dict_id;
            let value = table.modification(id).unwrap().modifiers[0].value;
            assert!(value < 0.0, "{id}");
            let plain = item_weight(dict_id, 1, &[]);
            let upgraded = item_weight(dict_id, 1, &[(id, false)]);
            assert!(plain > 0.0, "{name}");
            assert!((upgraded - plain * (1.0 + value)).abs() < 1e-5, "{name}: {plain} {upgraded}");
        }
    }

    /// The held weapon's own speed modifier moves a bot's speed level, only
    /// when asked for.
    #[test]
    fn the_held_weapons_own_speed_modifier_moves_the_level() {
        use crate::v026g::loadouts::RusherKits;
        use crate::v026g::match_msgs::static_info::MatchProfile;
        let equipment = BotEquipment::of(BotTier::Low, RusherKits::default());
        let kits = bot_kits(equipment, BotRole::Flanker);
        let weapon = crate::v026g::weapons::weapon(kits[0].slot(7).unwrap().dict_id).unwrap();
        assert_ne!(weapon.movement_speed_modifier, 0.0, "{}", weapon.name);
        let profile = MatchProfile::from_loadouts(
            "Bot-1",
            0,
            &[kits[0]],
            [0.0; MODIFIERS],
            Dictionary::shared(),
            1,
        );
        let off = profile_movement_level_in(&profile, 0, false).unwrap();
        let on = profile_movement_level_in(&profile, 0, true).unwrap();
        assert!((on - off - weapon.movement_speed_modifier).abs() < 1e-5, "{off} {on}");
        // Off by the switch: the plain level.
        assert_eq!(profile_movement_level(&profile, 0), Some(off));
    }

    /// Every tier and role, with each rusher kit the data can make, the
    /// owner's kit and the upgrades off and on.
    fn every_bot() -> Vec<(BotEquipment, BotRole, bool, bool)> {
        let mut all = Vec::new();
        for new in [false, true] {
            for upgrades in [false, true] {
                for tier in BotTier::ALL {
                    for rusher in made_kits(tier) {
                        let equipment = BotEquipment { tier, rusher, mask: None };
                        for role in BotRole::ALL {
                            all.push((equipment, role, new, upgrades));
                        }
                    }
                }
            }
        }
        all
    }

    #[test]
    fn every_tier_is_a_set_a_player_could_have() {
        let tree = SkillsTree::shared();
        let mut last = None;
        for tier in &BOT_SKILL_TIERS {
            // The rows, and the rows with the owner's kit's pouch skills.
            for pouch in [false, true] {
                let skills = tier.skills_with(pouch);
                let level = tier.level_with(pouch);
                let spent: u32 = skills.iter().map(|&(_, points)| u32::from(points)).sum();
                assert_eq!(spent, u32::from(level), "{tier:?} {pouch}");
                assert_eq!(tree.validate(level, &skills), Ok(()), "{tier:?} {pouch}");
                assert_eq!(
                    tree.validate(level - 1, &skills),
                    Err(SkillError::OverLevel {
                        spent,
                        level: level - 1
                    })
                );
            }
            // Higher tiers are more levelled and keep what the lower had.
            if let Some(lower) = last {
                let lower: &BotSkillTier = lower;
                assert!(tier.up_to_gear_level > lower.up_to_gear_level);
                assert!(tier.level > lower.level);
                for &(id, points) in lower.skills {
                    let has = tier.skills.iter().find(|&&(other, _)| other == id);
                    assert!(has.is_some_and(|&(_, more)| more >= points), "{id}");
                }
            }
            last = Some(tier);
            // The carry path at its most in every tier: 40 kg.
            assert!(tier.skills.contains(&(209, 5)), "{tier:?}");
            assert_eq!(tier.modifiers()[MAX_CARRIED_WEIGHT], 10.0);
            // None of the skills left out (see the table); 102 and 108 only
            // with the pouch.
            for left_out in [102, 103, 108, 213, 214] {
                assert!(
                    tier.skills.iter().all(|&(id, _)| id != left_out),
                    "{left_out}"
                );
            }
            let with = tier.skills_with(true);
            for left_out in [103, 213, 214] {
                assert!(with.iter().all(|&(id, _)| id != left_out), "{left_out}");
            }
            assert!(tier.skills_with(false) == tier.skills);
        }
        // The tiers by the kits' gear level.
        let level = |gear| BotSkillTier::for_gear_level(gear).level_with(false);
        assert_eq!([level(0), level(5), level(6), level(10)], [15, 15, 20, 20]);
        assert_eq!([level(11), level(15), level(16)], [30, 30, 30]);
        let level = |gear| BotSkillTier::for_gear_level(gear).level_with(true);
        assert_eq!([level(0), level(5), level(6), level(10)], [26, 26, 31, 31]);
        assert_eq!([level(11), level(15), level(16)], [41, 41, 41]);
        // The pouch skills give the extra bag (109), 12 % faster reloads
        // (108), 15 % faster weapon swaps (104), 5 % more speed while
        // aiming (102), and nothing else: no aim, damage or protection.
        for tier in &BOT_SKILL_TIERS {
            let (off, on) = (tier.modifiers_with(false), tier.modifiers_with(true));
            let changed: Vec<usize> = (0..MODIFIERS)
                .filter(|&i| (off[i] - on[i]).abs() > 1e-6)
                .collect();
            let wanted = [
                index("ammo_bags"),
                index("reload_speed"),
                index("change_weapon_speed"),
                index("movement_speed_when_aiming"),
            ];
            assert!(changed.iter().all(|i| wanted.contains(i)), "{changed:?}");
            assert_eq!(on[index("ammo_bags")], 1.0);
            assert!((on[index("reload_speed")] - 0.12).abs() < 1e-6);
            assert!((on[index("change_weapon_speed")] - 0.15).abs() < 1e-6);
            assert!((on[index("movement_speed_when_aiming")] - 0.05).abs() < 1e-6);
            assert_eq!(off[index("ammo_bags")], 0.0);
        }
        // What the top tier's skills give.
        let top = BOT_SKILL_TIERS[2].modifiers();
        assert!((top[index("aim_time")] + 0.1).abs() < 1e-6);
        assert!((top[index("movement_speed")] - 0.05).abs() < 1e-6);
        assert!((top[index("overweight_penalty")] + 0.25).abs() < 1e-6);
        assert!((top[index("stamina_regeneration_speed")] - 0.1).abs() < 1e-6);
        assert!((top[index("stamina_spending_speed_skill")] + 0.1).abs() < 1e-6);
        assert_eq!(top[index("can_sprint")], 0.0);
        assert_eq!(top[index("max_carried_weight_ultimate")], 0.0);
    }

    /// No bot kit is overburdened: every kit of every tier and role
    /// weighs less than the capacity its skills give (40 kg), and its
    /// skills are those of its tier's gear level.
    #[test]
    fn no_bot_kit_is_over_its_capacity() {
        assert_eq!(item_params::player().max_carried_weight, 30.0);
        for (equipment, role, new, upgrades) in every_bot() {
            let tier = equipment.tier;
            let owned = bot_kits_in(equipment, role, new, upgrades);
            let kits: Vec<&Loadout> = owned.iter().collect();
            let gear = gear_level(&kits);
            assert!(tier.levels().contains(&gear), "{tier} {role}: {gear}");
            let skills = BotSkillTier::for_kits(&kits);
            assert_eq!(
                skills.up_to_gear_level,
                *tier.levels().end(),
                "{tier} {role}"
            );
            let modifiers = skills.modifiers_with(new);
            for kit in kits {
                let capacity = carry_capacity(kit, &modifiers);
                // 40 kg, and the kilograms of the carry upgrades.
                assert!(capacity >= 40.0, "{tier} {role}");
                assert!(capacity <= 45.0, "{tier} {role}: {capacity}");
                assert!(kit.weight() < capacity, "{tier} {role}: {}", kit.weight());
            }
        }
    }

    #[test]
    fn the_speed_level_follows_the_weight_and_the_skills() {
        let none = [0.0; MODIFIERS];
        let close = |a: f32, b: f32| (a - b).abs() < 1e-5;
        // Up to 20 kg nothing; at the capacity -20 %; between, in line.
        assert!(close(movement_level(20.0, 30.0, &none, 0.0), 1.0));
        assert!(close(movement_level(25.0, 30.0, &none, 0.0), 0.9));
        assert!(close(movement_level(30.0, 30.0, &none, 0.0), 0.8));
        assert!(close(movement_level(45.0, 30.0, &none, 0.0), 0.8));
        assert!(close(weight_cost(25.0, 40.0), 0.25));
        // The top tier: 40 kg, a quarter less of the cost, +5 %.
        let top = BOT_SKILL_TIERS[2].modifiers();
        assert!(close(movement_level(30.0, 40.0, &top, 0.0), 1.05 - 0.075));
        // Heavyweight: no speed lost to the weight.
        let mut heavy = none;
        heavy[index("max_carried_weight_ultimate")] = 1.0;
        assert!(close(movement_level(30.0, 30.0, &heavy, 0.0), 1.0));
        // A modification's movement speed adds.
        assert!(close(movement_level(0.0, 30.0, &none, -0.1), 0.9));
        // The defect 4080 takes 3 kg of capacity; the modification 1120
        // adds 1 kg.
        assert_eq!(
            item_added(0, &[(4080, true)], "max_carried_weight_modifier"),
            -3.0
        );
        assert_eq!(
            item_added(0, &[(1120, false)], "max_carried_weight_modifier"),
            1.0
        );
    }

    /// The rusher's SMG kit slows a bot no more than the SPAS-12 kit it
    /// replaces did: lighter in both tiers (the SMG, its ammo and the
    /// helmet weigh less than the shotgun and its shells, or the helmet is
    /// all it adds at 13-15), so no more of the weight's cost.
    #[test]
    fn the_rushers_smg_kit_is_no_slower_than_the_old_one() {
        use crate::v026g::match_msgs::static_info::MatchProfile;
        let level = |equipment: BotEquipment| {
            let kits = bot_kits(equipment, BotRole::Rusher);
            let profile = MatchProfile::from_loadouts(
                "bot",
                0,
                &kits,
                bot_modifiers(&kits),
                Dictionary::shared(),
                1,
            );
            profile_movement_level(&profile, 0).unwrap()
        };
        for tier in [BotTier::Mid, BotTier::High] {
            let smg = RusherKit::Smg.worn_in(tier);
            if smg == RusherKit::Control {
                continue;
            }
            let (new, old) = (
                level(BotEquipment { tier, rusher: smg, mask: None }),
                level(BotEquipment {
                    tier,
                    rusher: RusherKit::Control,
                    mask: None,
                }),
            );
            assert!(new >= old - 1e-6, "{tier}: {new} against {old}");
        }
    }

    /// On the live game data (`SRP_GAME_DATA`, the server's file) the same
    /// holds for every rusher kit of both higher tiers, the SMGs of the
    /// 0.29a3 weapons among them. Run alone:
    /// `SRP_GAME_DATA=<server-game-data.db> cargo test -p survarium --lib
    /// -- --ignored on_the_live_data_no_rusher_kit`
    #[test]
    #[ignore = "needs SRP_GAME_DATA, the server's game data"]
    fn on_the_live_data_no_rusher_kit_is_over_its_capacity() {
        crate::v026g::game_data::init().expect("SRP_GAME_DATA reads and passes srp's checks");
        no_bot_kit_is_over_its_capacity();
        the_rushers_smg_kit_is_no_slower_than_the_old_one();
        a_bots_profile_gives_its_speed_level();
        for tier in [BotTier::Mid, BotTier::High] {
            assert_eq!(made_kits(tier).count(), RusherKit::ALL.len(), "{tier}");
        }
    }

    #[test]
    fn a_bots_profile_gives_its_speed_level() {
        use crate::v026g::match_msgs::static_info::MatchProfile;
        for (equipment, role, new, upgrades) in every_bot() {
            let tier = equipment.tier;
            let owned = bot_kits_in(equipment, role, new, upgrades);
            let kits: Vec<&Loadout> = owned.iter().collect();
            let modifiers = BotSkillTier::for_kits(&kits).modifiers_with(new);
            let profile =
                MatchProfile::from_loadouts("bot", 0, &kits, modifiers, Dictionary::shared(), 1);
            for (k, kit) in kits.iter().enumerate() {
                let capacity = carry_capacity(kit, &modifiers);
                // The items' own movement speed (their default
                // modifications and the upgrades they wear) and weight.
                let items: f32 = kit
                    .items
                    .iter()
                    .map(|item| {
                        item_added(
                            item.dict_id,
                            &wire_mods(item.mods),
                            "movement_speed_modifier",
                        )
                    })
                    .sum();
                let weight: f32 = kit
                    .items
                    .iter()
                    .map(|item| item_weight(item.dict_id, item.count, &wire_mods(item.mods)))
                    .sum();
                let expected = movement_level(weight, capacity, &modifiers, items);
                let level = profile_movement_level(&profile, k).unwrap();
                assert!(
                    (level - expected).abs() < 1e-5,
                    "{tier} {role}: {level} {expected}"
                );
                // The tiers' kits: under 31 kg of 40, so a few percent
                // slower at most (the owner's kit is 1.5 to 2.5 kg
                // heavier); the skills' and the armour's speed above.
                assert!((0.93..=1.15).contains(&level), "{tier} {role}: {level}");
            }
            assert_eq!(profile_movement_level(&profile, kits.len()), None);
        }
    }
}
