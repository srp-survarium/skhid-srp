//! Lobby rules that learned skills change (the skill tree's "perks"; the full audit is docs/notes/perks.md).
//!
//! The lobby window reads three skill modifiers itself (exe `fill_skills` 0x2001C0D90, per learned skill with points:
//! `modifiers[0]` has the key -> flag): `use_two_double_handed_weapons_modifier` ("Terminator") ->
//! `root.allow_use_two_double_handed_weapons(1)`, `disable_second_weapon_modifier` -> `root.enable_second_weapon(0)`,
//! `survival_additional_medkit_modifier` -> `root.enable_additional_medkits(1)` (display only). The 0.69d tree
//! (`skills_tree.options`) has no skill with either of the first two, so in 0.69d both weapon slots are open and hold at
//! most one two-handed weapon; the rule stays data-driven so a tree that carries them changes it.
//!
//! Weapon slots of the lobby movie (`LocalPaperDoll`, drop rule at AS line 501..520): when `_doubleTwoHanded` is false,
//! a weapon cannot go into slot 7 or 10 while the OTHER slot holds a two-handed weapon (`oneHanded == false`), unless
//! the dragged weapon is one-handed or it replaces the two-handed one; a weapon cannot go into one slot while the other
//! holds the same dict (`isSameWeapon`). `oneHanded = !double_handed` (ItemFactory), `double_handed` = the weapon's
//! `parameters.double_handed` in game.options (`data/weapon_hands_069d.json` lists the one-handed ones).

use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;

use crate::GameData;

/// Table (`data/weapon_hands_069d.json`, made by `crates/data-extract`).
pub const WEAPON_HANDS_FILE: &str = "weapon_hands_069d.json";

/// Skill modifier that lets the secondary weapon be two-handed as well ("Terminator", `st_use_two_double_handed_weapons_name`).
pub const MOD_TWO_TWO_HANDED: &str = "use_two_double_handed_weapons_modifier";
/// Skill modifier that closes the secondary weapon slot.
pub const MOD_NO_SECOND_WEAPON: &str = "disable_second_weapon_modifier";
/// Skill modifier the lobby shows as "+1" on medkits in pockets (the match applies it, exe 0x2002D8433).
pub const MOD_ADDITIONAL_MEDKIT: &str = "survival_additional_medkit_modifier";

/// Primary and secondary weapon slots of the profile table.
pub const PRIMARY_SLOT: u8 = 7;
pub const SECONDARY_SLOT: u8 = 10;

#[derive(Debug, Deserialize)]
pub(crate) struct RawWeaponHands {
    pub one_handed: Vec<u16>,
}

pub(crate) fn one_handed_set(raw: RawWeaponHands) -> BTreeSet<u16> {
    raw.one_handed.into_iter().collect()
}

/// What the learned skills open in the weapon slots (the two `fill_skills` flags).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WeaponSlotRules {
    /// `allow_use_two_double_handed_weapons`: both weapon slots may hold two-handed weapons
    pub two_two_handed: bool,
    /// `enable_second_weapon`: the secondary slot takes a weapon
    pub second_weapon: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WeaponSlotError {
    #[error("two two-handed weapons ({0} and {1}) without a skill that allows it")]
    TwoTwoHanded(u16, u16),
    #[error("the same weapon {0} in both weapon slots")]
    SameWeapon(u16),
    #[error("a learned skill closes the secondary weapon slot (weapon {0} there)")]
    NoSecondWeapon(u16),
}

impl GameData {
    /// Is this weapon two-handed (`double_handed`; a dict that is not a weapon is not)?
    pub fn is_two_handed(&self, did: u16) -> bool {
        self.items.is_weapon(did) && !self.one_handed.contains(&did)
    }

    /// Does a learned skill (points > 0) carry `modifier` in its first level, the test of `fill_skills`?
    pub fn skills_carry(&self, skills: &BTreeMap<u16, u8>, modifier: &str) -> bool {
        skills.iter().any(|(&id, &pts)| pts > 0 && self.matches.skill_level_has(id, 0, modifier))
    }

    /// The weapon slot flags of a learned skill set.
    pub fn weapon_slot_rules(&self, skills: &BTreeMap<u16, u8>) -> WeaponSlotRules {
        WeaponSlotRules {
            two_two_handed: self.skills_carry(skills, MOD_TWO_TWO_HANDED),
            second_weapon: !self.skills_carry(skills, MOD_NO_SECOND_WEAPON),
        }
    }

    /// Check the weapons of slots 7 and 10 (dict ids) against the lobby movie's rules for this skill set.
    pub fn check_weapon_slots(&self, skills: &BTreeMap<u16, u8>, primary: Option<u16>, secondary: Option<u16>) -> Result<(), WeaponSlotError> {
        self.check_weapon_slots_with(self.weapon_slot_rules(skills), primary, secondary)
    }

    /// [`GameData::check_weapon_slots`] for given flags.
    pub fn check_weapon_slots_with(&self, rules: WeaponSlotRules, primary: Option<u16>, secondary: Option<u16>) -> Result<(), WeaponSlotError> {
        if let Some(s) = secondary {
            if !rules.second_weapon && self.items.is_weapon(s) {
                return Err(WeaponSlotError::NoSecondWeapon(s));
            }
        }
        if let (Some(p), Some(s)) = (primary, secondary) {
            if p == s && self.items.is_weapon(p) {
                return Err(WeaponSlotError::SameWeapon(p));
            }
            if !rules.two_two_handed && self.is_two_handed(p) && self.is_two_handed(s) {
                return Err(WeaponSlotError::TwoTwoHanded(p, s));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gd() -> GameData {
        crate::testing::tables()
    }

    fn dict(g: &GameData, name: &str) -> u16 {
        g.items.by_cfg_name(name).unwrap()
    }

    /// The table: the 25 one-handed weapons are exactly the pistol category 17 (game.options `double_handed` false).
    #[test]
    fn one_handed_weapons_are_the_pistols() {
        crate::require_tables!();
        let g = gd();
        assert_eq!(g.one_handed.len(), 25);
        assert!(g.one_handed.iter().all(|&d| g.items.category(d) == 17));
        assert!(!g.is_two_handed(dict(&g, "glock_17")));
        assert!(g.is_two_handed(dict(&g, "ak_74m")));
        assert!(g.is_two_handed(dict(&g, "kedr")));
        assert!(!g.is_two_handed(dict(&g, "rgd-5")), "not a weapon");
    }

    /// The 0.69d tree has no skill with either weapon-slot modifier: every skill learned to its top level opens nothing.
    #[test]
    fn no_069d_skill_changes_the_weapon_slots() {
        crate::require_tables!();
        let g = gd();
        let all: BTreeMap<u16, u8> = g.skills.rows().map(|r| (r.id, r.levels)).collect();
        assert_eq!(all.len(), 69);
        assert_eq!(g.weapon_slot_rules(&all), WeaponSlotRules { two_two_handed: false, second_weapon: true });
        // the medkit flag of the same function is carried by "Additional Medicine" (351)
        assert!(g.skills_carry(&BTreeMap::from([(351, 1)]), MOD_ADDITIONAL_MEDKIT));
        assert!(!g.skills_carry(&BTreeMap::from([(351, 0)]), MOD_ADDITIONAL_MEDKIT));
    }

    /// One two-handed weapon in either slot, a pistol in the other, never the same dict twice.
    #[test]
    fn weapon_slot_rules_follow_the_lobby_movie() {
        crate::require_tables!();
        let g = gd();
        let none = BTreeMap::new();
        let (ak, svd, glock, pm) = (dict(&g, "ak_74m"), dict(&g, "svd"), dict(&g, "glock_17"), dict(&g, "pm"));
        assert_eq!(g.check_weapon_slots(&none, Some(ak), Some(glock)), Ok(()));
        assert_eq!(g.check_weapon_slots(&none, Some(glock), Some(ak)), Ok(()), "a rifle in the secondary slot next to a pistol");
        assert_eq!(g.check_weapon_slots(&none, None, Some(svd)), Ok(()));
        assert_eq!(g.check_weapon_slots(&none, Some(glock), Some(pm)), Ok(()));
        assert_eq!(g.check_weapon_slots(&none, Some(ak), Some(svd)), Err(WeaponSlotError::TwoTwoHanded(ak, svd)));
        assert_eq!(g.check_weapon_slots(&none, Some(glock), Some(glock)), Err(WeaponSlotError::SameWeapon(glock)));
        // a tree whose skill carries the "Terminator" modifier opens two two-handed weapons; the other modifier closes slot 10
        let terminator = WeaponSlotRules { two_two_handed: true, second_weapon: true };
        assert_eq!(g.check_weapon_slots_with(terminator, Some(ak), Some(svd)), Ok(()));
        assert_eq!(g.check_weapon_slots_with(terminator, Some(ak), Some(ak)), Err(WeaponSlotError::SameWeapon(ak)));
        let closed = WeaponSlotRules { two_two_handed: false, second_weapon: false };
        assert_eq!(g.check_weapon_slots_with(closed, Some(ak), Some(glock)), Err(WeaponSlotError::NoSecondWeapon(glock)));
        assert_eq!(g.check_weapon_slots_with(closed, Some(ak), None), Ok(()));
    }
}
