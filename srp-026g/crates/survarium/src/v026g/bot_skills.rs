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
/// the reason in the note: 102 (no speed in the client's aiming speed,
/// LIKELY), 103 (the bots time a bolt's chambering by the clip), 213 (two
/// of ten).
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

impl BotSkillTier {
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

    /// The player modifiers its skills give.
    pub fn modifiers(&self) -> [f32; MODIFIERS] {
        SkillsTree::shared().modifiers(self.skills)
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

/// What a player with `modifiers` (a kit's skills) carries in `kit` before
/// being overburdened: `max_carried_weight` plus the skills' and the worn
/// items' default modifications' `max_carried_weight` (see the module).
pub fn carry_capacity(kit: &Loadout, modifiers: &[f32; MODIFIERS]) -> f32 {
    let items: f32 = kit
        .items
        .iter()
        .map(|item| default_modifier(item.dict_id, "max_carried_weight_modifier"))
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
/// module). Not counted: a weapon's own `movement_speed_modifier` (±0.05,
/// not in `item_params`).
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
/// and modifications, its modifiers. `None` without that kit.
pub fn profile_movement_level(profile: &MatchProfile, kit: usize) -> Option<f32> {
    let kit = profile.kits.get(kit)?;
    let (mut weight, mut capacity_added, mut movement) = (0.0, 0.0, 0.0);
    for &(index, count) in kit.slots.iter().flatten() {
        let Some(item) = profile.items.get(usize::from(index)) else {
            continue;
        };
        weight += item_params::carried_weight(item.dict_id, u32::from(count));
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
    use crate::v026g::loadouts::{BOT_KIT, BotRole, bot_kits};
    use crate::v026g::skills::SkillError;

    fn index(name: &str) -> usize {
        modifier_index(name).unwrap()
    }

    #[test]
    fn every_tier_is_a_set_a_player_could_have() {
        let tree = SkillsTree::shared();
        let mut last = None;
        for tier in &BOT_SKILL_TIERS {
            let spent: u32 = tier
                .skills
                .iter()
                .map(|&(_, points)| u32::from(points))
                .sum();
            assert_eq!(spent, u32::from(tier.level), "{tier:?}");
            assert_eq!(tree.validate(tier.level, tier.skills), Ok(()), "{tier:?}");
            assert_eq!(
                tree.validate(tier.level - 1, tier.skills),
                Err(SkillError::OverLevel {
                    spent,
                    level: tier.level - 1
                })
            );
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
            // None of the skills left out (see the table).
            for left_out in [102, 103, 108, 213, 214] {
                assert!(
                    tier.skills.iter().all(|&(id, _)| id != left_out),
                    "{left_out}"
                );
            }
        }
        // The tiers by the kits' gear level.
        let level = |gear| BotSkillTier::for_gear_level(gear).level;
        assert_eq!([level(0), level(5), level(6), level(10)], [15, 15, 20, 20]);
        assert_eq!([level(11), level(15), level(16)], [30, 30, 30]);
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

    /// No bot kit is overburdened: each weighs less than its tier's
    /// capacity.
    #[test]
    fn no_bot_kit_is_over_its_capacity() {
        assert_eq!(item_params::player().max_carried_weight, 30.0);
        for role in BotRole::ALL {
            let kits = bot_kits(role);
            assert!(gear_level(&kits) <= 5, "{role}: the starting kits");
            let tier = BotSkillTier::for_kits(&kits);
            assert_eq!(tier.up_to_gear_level, 5, "{role}");
            let modifiers = bot_modifiers(&kits);
            for kit in kits {
                let capacity = carry_capacity(kit, &modifiers);
                assert_eq!(capacity, 40.0, "{role}");
                assert!(kit.weight() < capacity, "{role}: {}", kit.weight());
            }
        }
    }

    /// A kit of `armour` and a weapon with 200 rounds in each of the three
    /// ammo slots, besides [`BOT_KIT`] (whose trousers `armour` may
    /// replace).
    fn tier_kit(
        armour: &[(u8, &'static str)],
        weapon: &'static str,
        ammo: &'static str,
    ) -> Loadout {
        let dictionary = Dictionary::shared();
        let mut names: Vec<(u8, &str)> = armour.to_vec();
        names.push((7, weapon));
        names.extend([8, 9, 11].map(|slot| (slot, ammo)));
        for item in BOT_KIT {
            if !names.iter().any(|&(slot, _)| slot == item.slot) {
                names.push((item.slot, item.name));
            }
        }
        let mut kit = Loadout::from_names(dictionary, names).expect("the items fit");
        for item in &mut kit.items {
            if [8, 9, 11].contains(&item.slot) {
                item.count = 200;
            } else if let Some(bot) = BOT_KIT.iter().find(|bot| bot.slot == item.slot) {
                item.count = bot.count;
            }
        }
        kit
    }

    /// The heaviest sets a tier of 9-10 or 13-15 equipment would plausibly
    /// wear fit the capacity its skills give; the whole heaviest set of
    /// item level 15 with a rifle and 600 rounds does not, and no skill
    /// can make it fit (the most is 40 kg).
    #[test]
    fn the_heavier_tiers_kits_fit_their_capacity() {
        let weight_of = |kit: &Loadout| kit.weight();
        // 9-10: Renaissance's heaviest of level 9 (the 12 kg torso) and the
        // AK-74M with 600 rounds.
        let nine = tier_kit(
            &[
                (0, "ren_helmet_2"),
                (1, "ren_mask_2"),
                (2, "ren_torso_2"),
                (4, "ren_legs_2"),
                (5, "edge_gloves_2"),
                (6, "edge_boots_2"),
            ],
            "ak_74m",
            "ammo_5.45x39_7n10",
        );
        // 13-15: Renaissance's level 14 set and the AK-12.
        let fourteen = tier_kit(
            &[
                (0, "ren_helmet_5"),
                (1, "ren_mask_5"),
                (2, "ren_torso_5"),
                (4, "ren_legs_5"),
                (5, "ren_gloves_5"),
                (6, "ren_boots_5"),
            ],
            "ak_12",
            "ammo_5.45x39_7n22",
        );
        for (kit, gear, tier_level) in [(&nine, 10, 20), (&fourteen, 15, 30)] {
            let kits = [kit];
            assert_eq!(gear_level(&kits), gear);
            let tier = BotSkillTier::for_kits(&kits);
            assert_eq!(tier.level, tier_level);
            let capacity = carry_capacity(kit, &tier.modifiers());
            assert_eq!(capacity, 40.0);
            assert!(weight_of(kit) < capacity, "{}", weight_of(kit));
            // Over what a player with no skills carries.
            assert!(weight_of(kit) > 30.0, "{}", weight_of(kit));
        }
        // The whole level 15 set with the same rifle: over any capacity.
        let fifteen = tier_kit(
            &[
                (0, "ren_helmet_6"),
                (2, "ren_torso_6"),
                (4, "ren_legs_6"),
                (5, "ren_gloves_6"),
                (6, "ren_boots_6"),
            ],
            "ak_12",
            "ammo_5.45x39_7n22",
        );
        assert!(weight_of(&fifteen) > 40.0, "{}", weight_of(&fifteen));
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

    #[test]
    fn a_bots_profile_gives_its_speed_level() {
        use crate::v026g::match_msgs::static_info::MatchProfile;
        for role in BotRole::ALL {
            let kits = bot_kits(role);
            let modifiers = bot_modifiers(&kits);
            let profile =
                MatchProfile::from_loadouts("bot", 0, &kits, modifiers, Dictionary::shared(), 1);
            for (k, kit) in kits.iter().enumerate() {
                let capacity = carry_capacity(kit, &modifiers);
                let expected = movement_level(kit.weight(), capacity, &modifiers, 0.0);
                let level = profile_movement_level(&profile, k).unwrap();
                assert!(
                    (level - expected).abs() < 1e-5,
                    "{role}: {level} {expected}"
                );
                // Today's kits: 19 to 25 kg of 40, so a few percent slower.
                assert!((0.95..=1.0).contains(&level), "{role}: {level}");
            }
            assert_eq!(profile_movement_level(&profile, 2), None);
        }
    }
}
