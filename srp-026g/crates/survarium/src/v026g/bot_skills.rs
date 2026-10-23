//! The skills of the server's own bots ("Player vs Bots"), by tier: one
//! table, [`BOT_SKILL_TIERS`], keyed by the highest item level a bot's kits
//! wear. `docs/notes/player_bots_skills.md` has the reasoning and numbers.
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

use super::item_params;
use super::loadouts::{Loadout, default_modifier};
use super::skills::{MODIFIERS, SkillsTree};

/// The player modifier `max_carried_weight` (index 5 of the 68).
pub const MAX_CARRIED_WEIGHT: usize = 5;

/// The skills of the bots whose kits wear items of item level at most
/// [`BotSkillTier::up_to_gear_level`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BotSkillTier {
    /// The highest item level of a kit this tier is for.
    pub up_to_gear_level: u8,
    /// The account level the points are spent at: a level gives a point,
    /// so this is the sum of `skills`' points.
    pub level: u8,
    /// `(skill id, points)`, a set the skills window would let a player of
    /// `level` have ([`SkillsTree::validate`]).
    pub skills: &'static [(u16, u8)],
}

/// The bots' skills by tier, lowest first; a kit above every tier's
/// [`BotSkillTier::up_to_gear_level`] gets the last. Modelled on what the
/// best players of the 0.26g server took (2026-10-23): the carry path
/// (201 to open the row, then 209 at five: 40 kg) in every tier, then
/// aim time (101), 202 and movement speed (208), aiming movement (102) and
/// stamina regeneration (204), and at the top sprinting when overloaded
/// (213). Skills 201 (stamina spent), 209 and 202 are the "Athlete" and
/// "Packrat" carry path.
#[rustfmt::skip]
pub const BOT_SKILL_TIERS: [BotSkillTier; 3] = [
    // The kits of item level 1 to 5 (the starting roles).
    BotSkillTier { up_to_gear_level: 5, level: 15, skills: &[
        (101, 5), (201, 5), (209, 5),
    ] },
    // Up to the 9 and 10 equipment.
    BotSkillTier { up_to_gear_level: 10, level: 25, skills: &[
        (101, 5), (201, 5), (209, 5), (202, 5), (208, 5),
    ] },
    // The 11 to 15 equipment.
    BotSkillTier { up_to_gear_level: 15, level: 36, skills: &[
        (101, 5), (102, 5), (201, 5), (204, 5), (209, 5), (202, 5), (208, 5), (213, 1),
    ] },
];

impl BotSkillTier {
    /// The tier of kits whose highest item level is `gear_level`.
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

/// The highest item level of anything `kits` wear.
pub fn gear_level(kits: &[&Loadout]) -> u8 {
    kits.iter()
        .flat_map(|kit| &kit.items)
        .filter_map(|item| item_params::item_params(item.dict_id))
        .map(|params| params.item_level)
        .max()
        .unwrap_or(0)
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

/// The modifiers of a bot wearing `kits`: its tier's skills.
pub fn bot_modifiers(kits: &[&Loadout]) -> [f32; MODIFIERS] {
    BotSkillTier::for_kits(kits).modifiers()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v026g::dictionary::Dictionary;
    use crate::v026g::loadouts::{BOT_KIT, BotRole, bot_kits};
    use crate::v026g::skills::{SkillError, modifier_index};

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
        }
        // The tiers by the kits' item level.
        let level = |gear| BotSkillTier::for_gear_level(gear).level;
        assert_eq!([level(0), level(5), level(6), level(10)], [15, 15, 25, 25]);
        assert_eq!([level(11), level(15), level(16)], [36, 36, 36]);
        // The top tier sprints when overloaded, and has less of the
        // overweight penalty.
        let top = BOT_SKILL_TIERS[2].modifiers();
        assert_eq!(top[modifier_index("can_sprint").unwrap()], 1.0);
        assert!((top[modifier_index("overweight_penalty").unwrap()] + 0.25).abs() < 1e-6);
        assert_eq!(
            top[modifier_index("max_carried_weight_ultimate").unwrap()],
            0.0
        );
    }

    /// No bot kit is overburdened: each weighs less than its tier's
    /// capacity.
    #[test]
    fn no_bot_kit_is_over_its_capacity() {
        assert_eq!(item_params::player().max_carried_weight, 30.0);
        for role in BotRole::ALL {
            let kits = bot_kits(role);
            let tier = BotSkillTier::for_kits(&kits);
            assert_eq!(tier.up_to_gear_level, 5, "{role}: the starting kits");
            let modifiers = bot_modifiers(&kits);
            for kit in kits {
                let capacity = carry_capacity(kit, &modifiers);
                assert_eq!(capacity, 40.0, "{role}");
                assert!(kit.weight() < capacity, "{role}: {}", kit.weight());
            }
        }
    }

    /// A kit of `armour` and a weapon with `rounds` in each of the three
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
    /// can make it fit (the most is 40 kg): the top tier sprints all the
    /// same (213).
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
        for (kit, tier_level) in [(&nine, 25), (&fourteen, 36)] {
            let kits = [kit];
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
}
