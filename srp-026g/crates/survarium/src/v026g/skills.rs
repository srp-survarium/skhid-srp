//! The 0.26g skills tree: the game's `gameplay/skills/skills_tree.options`
//! (0.26g's `resources.db`; `docs/notes/data.md` §3), the rules
//! for spending points, and what a set of skills adds to a kit's 68 player
//! modifiers (`static_info.md` §3.1).
//!
//! Against 0.21d (`v021d::skills`) everything changed: 26 skills in two
//! branches (101..=113 and 201..=214) of 0 to 5 levels, each level naming
//! modifiers (not booster ids) with values as fractions (not percents), and
//! `preconditions`: `branch_points` and `antiskills`.
//!
//! The rules, from the client's own skills window (`lobby_menu.swf`
//! `survarium.char.CharWindow`, decompiled with FFDec) and the exe:
//! - Points to spend are the account's level: `points_available = level -
//!   Σ points` (`fill_char_info` 0x224420). PROVEN.
//! - A skill takes at most `levels_amount` points (`max_value`). PROVEN (UI).
//! - `branch_points` (`getBranchUnlockedLevel`): the skills of a branch are
//!   walked by their row in the window (`ui.position[1]`), then id, counting
//!   the points in the skills before. The walk stops at the first skill
//!   whose `branch_points` is more than that count. The rows reached unlock
//!   the last reached skill's `branch_points` (at most 25), and a skill can
//!   take points when its `branch_points` is no more than that. The stock
//!   window rounded that down to fives (`floor(branch_points / 5) * 5`), the
//!   same for the stock tree's rows; our lobby source and srp take it whole,
//!   so the medical tree's second row opens at 3 (medical-skills.md). So a row needs
//!   its `branch_points` spent **in the rows above it** of the same branch,
//!   not in the whole branch. PROVEN (UI); that the original server checked
//!   the same is LIKELY.
//! - Skills 206 and 210 have `branch_points` 50, past the 25 the walk
//!   allows: they can never take a point. PROVEN (UI). The players' tree
//!   no longer has them (content edit list `physical-skills-1.json`, as
//!   0.46c2's tree); the tests' game data is the stock tree, which does.
//! - `antiskills`: a skill with points locks the skills it names
//!   (`handleAntiskills`): 111 against 113, 213 against 214. PROVEN (UI).
//!
//! The modifiers of a skill at `n` points are the sum of its first `n`
//! levels (the window's texts go "+2, +4, ... +10" for five levels of 0.02).
//! Two names the tree uses are not among the exe's 68 (table 0x928448):
//! `meele_hit_ultimate_modifier` (206) and `meele_hit_range_modifier` (210).
//! They are left out, and only those two unobtainable skills have them.
//!
//! In a match the kit's modifier block is the only source of skills, as in
//! 0.21d: the match profile has no skills list, gameplay reads the kit's
//! floats directly (getter 0x2D5AE0), and what the client adds on top
//! itself are item effects only (weapon, battery, medkit). So srp sums the
//! skills into the block and nothing of the items: an item's modifications
//! the client applies from its own tables (`static_info.md` section 10,
//! PROVEN in the exe; not tried live).

use binary_config_parser::tree::{ConfigError, ConfigValue};

/// Its path in the archive.
pub const PATH: &str = "gameplay/skills/skills_tree.options";

/// The player modifiers a kit carries.
pub const MODIFIERS: usize = 68;

/// The modifiers by index, without their `_modifier` suffix: the exe's name
/// table at 0x928448 (PROVEN; 0..=19 are 0.21d's `player_params_modifiers_enum`
/// with 15 and 17 renamed).
#[rustfmt::skip]
pub const MODIFIER_NAMES: [&str; MODIFIERS] = [
    "dispersion", "aiming_speed", "health_regeneration_speed", "stamina_regeneration_speed",
    "movement_speed", "max_carried_weight", "pain_health", "artefact_search_time",
    "anomaly_damage", "engineer_use_time", "engineer_success_chance", "device_damage",
    "splash_radius", "reload_speed", "artefact_slots", "pocket_slots", "ammo_bags",
    "pocket_slot_size", "recoil", "stamina_spending_speed", "movement_speed_when_aiming",
    "chamber_a_round", "change_weapon_speed", "breath_holding_time",
    "dispersion_movement_influence", "use_two_double_handed_weapons", "overweight_penalty",
    "grenade_throw_power", "stamina_boost_time", "stamina_boost_interval", "fall_damage",
    "can_sprint", "max_carried_weight_ultimate", "drug_using_time", "drug_activity_time",
    "critical_regen_duration", "pain_regeneration_speed", "use_artefact", "artefact_cooldown",
    "artefact_duration", "artefact_power", "incoming_weapon_damage_in_anomaly",
    "artefact_synergy", "armored_trap", "friendly_trap", "can_defuse_traps",
    "traps_explode_after_defuse", "spot_enemy_traps", "spot_artefacts", "aim_time",
    "friendly_anomaly", "toxic_damage", "pain_damage_after_hit_value",
    "pain_damage_after_hit_time", "second_chance_damage", "second_chance_ramaining_health",
    "dispersion_from_hip", "drug_amount", "drug_hit_coeff", "melee_damage", "respawn_time",
    "breath_vibration", "move_speed_weapon_penalty", "stamina_boost_amount",
    "disable_second_weapon", "injury_penalty", "stamina_loss_on_hit",
    "stamina_spending_speed_skill",
];

/// The modifiers that open or close a kit's slots, by index
/// ([`MODIFIER_NAMES`]; `loadouts::KitLayout`).
pub mod slot_modifier {
    pub const POCKET_SLOTS: usize = 15;
    pub const AMMO_BAGS: usize = 16;
    pub const POCKET_SLOT_SIZE: usize = 17;
    pub const USE_TWO_DOUBLE_HANDED_WEAPONS: usize = 25;
    /// `armored_trap`: the medical skills' ranks, packed as an exact
    /// integer below 2^24 (`docs/notes/medical-skills.md`).
    pub const MEDICAL_RANKS: usize = 43;
    /// The bit of [`MEDICAL_RANKS`] that is Additional Medicine (322).
    pub const ADDITIONAL_MEDICINE_BIT: u32 = 15;
    pub const DISABLE_SECOND_WEAPON: usize = 64;
}

/// The suffix every modifier's key has in the configs.
const SUFFIX: &str = "_modifier";

/// The most `branch_points` the skills window's walk takes (`SKILL_LEVELS`).
const MAX_BRANCH_POINTS: u32 = 25;

/// The index of a modifier by its config key (`aim_time_modifier`) or its
/// bare name.
pub fn modifier_index(name: &str) -> Option<usize> {
    let name = name.strip_suffix(SUFFIX).unwrap_or(name);
    MODIFIER_NAMES.iter().position(|&known| known == name)
}

#[derive(Clone, Debug, PartialEq)]
pub struct Skill {
    pub id: u16,
    /// The most points the skill takes (`levels_amount`).
    pub levels_amount: u8,
    /// Points needed in the rows above, in the same branch.
    pub branch_points: u32,
    /// Skills that can not have points while this one has.
    pub antiskills: Vec<u16>,
    /// `ui.position`: the column and row in the skills window, from 1.
    pub column: u8,
    pub row: u8,
    /// By level - 1: what the level adds, as `(modifier index, value)`.
    pub levels: Vec<Vec<(usize, f32)>>,
    /// The modifier keys of the skill that are not among the 68.
    pub unknown_modifiers: Vec<String>,
}

impl Skill {
    /// The branch (the window's tab): 1 for 101..=113, 2 for 201..=214
    /// (`CharWindow.fillSkills`: `id / 100 - 1` is the tab index).
    pub fn branch(&self) -> u16 {
        self.id / 100
    }
}

/// Why a set of skills is refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkillError {
    /// No such skill in the tree.
    Unknown(u16),
    /// The skill is named twice.
    Repeated(u16),
    /// More points than the skill has levels.
    TooManyPoints { skill: u16, points: u8, most: u8 },
    /// More points spent than the level gives.
    OverLevel { spent: u32, level: u8 },
    /// Both skills of an exclusive pair have points.
    Exclusive(u16, u16),
    /// The skill's row is not unlocked: it needs `needs` points in the rows
    /// above it, and those rows unlock only `unlocked`.
    Locked {
        skill: u16,
        needs: u32,
        unlocked: u32,
    },
}

impl std::fmt::Display for SkillError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Self::Unknown(skill) => write!(f, "no skill {skill}"),
            Self::Repeated(skill) => write!(f, "skill {skill} is named twice"),
            Self::TooManyPoints {
                skill,
                points,
                most,
            } => write!(f, "skill {skill} takes {most} points, not {points}"),
            Self::OverLevel { spent, level } => {
                write!(f, "{spent} points spent at level {level}")
            }
            Self::Exclusive(a, b) => write!(f, "skills {a} and {b} exclude each other"),
            Self::Locked {
                skill,
                needs,
                unlocked,
            } => write!(
                f,
                "skill {skill} needs {needs} branch points, {unlocked} are unlocked"
            ),
        }
    }
}

impl std::error::Error for SkillError {}

#[derive(Clone, Debug, PartialEq)]
pub struct SkillsTree {
    /// By id.
    pub skills: Vec<Skill>,
}

fn small<T: TryFrom<i32>>(value: &ConfigValue, key: &str) -> Result<T, ConfigError> {
    let number = value.i32_at(key)?;
    T::try_from(number).map_err(|_| ConfigError(format!("`{key}` = {number} is out of range")))
}

fn parse_skill(skill: &ConfigValue) -> Result<Skill, ConfigError> {
    let preconditions = skill.req("preconditions")?;
    let antiskills = match preconditions.get("antiskills") {
        Some(list) => list
            .children()
            .into_iter()
            .map(|id| {
                id.as_i32()
                    .and_then(|id| u16::try_from(id).ok())
                    .ok_or_else(|| ConfigError("an antiskill is not a skill id".into()))
            })
            .collect::<Result<_, _>>()?,
        None => Vec::new(),
    };
    let position = skill.req("ui")?.req("position")?.children();
    let cell = |index: usize| {
        position
            .get(index)
            .and_then(|value| value.as_i32())
            .and_then(|value| u8::try_from(value).ok())
            .ok_or_else(|| ConfigError("a skill has no position".into()))
    };
    let mut unknown_modifiers = Vec::new();
    let levels = skill
        .req("modifiers")?
        .children()
        .into_iter()
        .map(|level| {
            let mut modifiers = Vec::new();
            for (name, value) in level.entries() {
                let value = value
                    .as_f32()
                    .ok_or_else(|| ConfigError(format!("`{name}` is not a number")))?;
                match modifier_index(name) {
                    Some(index) => modifiers.push((index, value)),
                    None if !unknown_modifiers.contains(name) => {
                        unknown_modifiers.push(name.clone());
                    }
                    None => {}
                }
            }
            Ok(modifiers)
        })
        .collect::<Result<_, ConfigError>>()?;
    Ok(Skill {
        id: small(skill, "id")?,
        levels_amount: small(skill, "levels_amount")?,
        branch_points: small(preconditions, "branch_points")?,
        antiskills,
        column: cell(0)?,
        row: cell(1)?,
        levels,
        unknown_modifiers,
    })
}

impl SkillsTree {
    /// The players' skills tree ([`super::game_data`]), parsed once.
    pub fn shared() -> &'static Self {
        static SHARED: std::sync::OnceLock<SkillsTree> = std::sync::OnceLock::new();
        SHARED.get_or_init(|| {
            Self::parse(super::game_data::file(PATH)).expect("the skills tree parses")
        })
    }

    pub fn parse(bytes: &[u8]) -> Result<Self, ConfigError> {
        let mut skills = ConfigValue::parse(bytes)?
            .req("skills_tree")?
            .children()
            .into_iter()
            .map(parse_skill)
            .collect::<Result<Vec<_>, _>>()?;
        skills.sort_by_key(|skill| skill.id);
        Ok(Self { skills })
    }

    pub fn skill(&self, id: u16) -> Option<&Skill> {
        self.skills
            .binary_search_by_key(&id, |skill| skill.id)
            .ok()
            .map(|index| &self.skills[index])
    }

    /// The `branch_points` the points in `skills` unlock in `branch`: the
    /// skills window's walk (`CharWindow.getBranchUnlockedLevel`).
    pub fn unlocked_branch_points(&self, branch: u16, skills: &[(u16, u8)]) -> u32 {
        let mut walk: Vec<&Skill> = self
            .skills
            .iter()
            .filter(|skill| skill.branch() == branch)
            .collect();
        walk.sort_by_key(|skill| (skill.row, skill.id));
        let (mut before, mut unlocked) = (0, 0);
        for skill in walk {
            if skill.branch_points > MAX_BRANCH_POINTS {
                continue;
            }
            if skill.branch_points > before {
                break;
            }
            unlocked = skill.branch_points;
            before += skills
                .iter()
                .filter(|&&(id, _)| id == skill.id)
                .map(|&(_, points)| u32::from(points))
                .sum::<u32>();
        }
        unlocked
    }

    /// Whether an account of `level` may have `skills` (`(skill id,
    /// points)`, its whole set): the module's rules.
    pub fn validate(&self, level: u8, skills: &[(u16, u8)]) -> Result<(), SkillError> {
        let mut spent = 0;
        for (index, &(id, points)) in skills.iter().enumerate() {
            let skill = self.skill(id).ok_or(SkillError::Unknown(id))?;
            if skills[..index].iter().any(|&(other, _)| other == id) {
                return Err(SkillError::Repeated(id));
            }
            if points > skill.levels_amount {
                return Err(SkillError::TooManyPoints {
                    skill: id,
                    points,
                    most: skill.levels_amount,
                });
            }
            spent += u32::from(points);
        }
        if spent > u32::from(level) {
            return Err(SkillError::OverLevel { spent, level });
        }
        let has_points = |id: u16| {
            skills
                .iter()
                .any(|&(other, points)| other == id && points > 0)
        };
        for &(id, _) in skills.iter().filter(|&&(_, points)| points > 0) {
            let skill = self.skill(id).ok_or(SkillError::Unknown(id))?;
            if let Some(&other) = skill.antiskills.iter().find(|&&other| has_points(other)) {
                return Err(SkillError::Exclusive(id, other));
            }
            let unlocked = self.unlocked_branch_points(skill.branch(), skills);
            if skill.branch_points > unlocked {
                return Err(SkillError::Locked {
                    skill: id,
                    needs: skill.branch_points,
                    unlocked,
                });
            }
        }
        Ok(())
    }

    /// A saved set of `skills` made valid again after the tree changed,
    /// taking points back and never adding any (the points to spend are the
    /// level minus the points held, so what is taken back is refunded):
    /// - a skill keeps at most its levels;
    /// - a skill the tree no longer has, in a branch it still has, goes;
    /// - of two exclusive skills that both have points, the higher id stays
    ///   (Supply Pocket 331 over Additional Medicine 322);
    /// - then a skill whose row its branch no longer unlocks goes, until
    ///   every row is reached (taking one back can lock rows below it).
    ///
    /// Skills of a branch the tree lacks, and a set spending more than the
    /// level, are left as they are.
    pub fn settle(&self, level: u8, skills: &[(u16, u8)]) -> Vec<(u16, u8)> {
        let has_branch = |branch: u16| self.skills.iter().any(|skill| skill.branch() == branch);
        let mut kept: Vec<(u16, u8)> = skills
            .iter()
            .filter_map(|&(id, points)| match self.skill(id) {
                Some(skill) => Some((id, points.min(skill.levels_amount))),
                None if has_branch(id / 100) => None,
                None => Some((id, points)),
            })
            .collect();
        let held = |set: &[(u16, u8)], id: u16| {
            set.iter().any(|&(other, points)| other == id && points > 0)
        };
        let excluded: Vec<u16> = kept
            .iter()
            .filter(|&&(_, points)| points > 0)
            .filter_map(|&(id, _)| {
                let skill = self.skill(id)?;
                skill
                    .antiskills
                    .iter()
                    .any(|&other| other > id && held(&kept, other))
                    .then_some(id)
            })
            .collect();
        kept.retain(|(id, _)| !excluded.contains(id));
        while let Err(SkillError::Locked { skill, .. }) = self.validate(level, &kept) {
            kept.retain(|&(id, _)| id != skill);
        }
        kept
    }

    /// The modifiers `skills` (`(skill id, points)`) give: for each skill
    /// the sum of its first `points` levels. Unknown skills add nothing.
    pub fn modifiers(&self, skills: &[(u16, u8)]) -> [f32; MODIFIERS] {
        let mut modifiers = [0.0; MODIFIERS];
        for &(id, points) in skills {
            let Some(skill) = self.skill(id) else {
                continue;
            };
            let points = points.min(skill.levels_amount);
            for level in skill.levels.iter().take(points.into()) {
                for &(modifier, value) in level {
                    modifiers[modifier] += value;
                }
            }
        }
        modifiers
    }
}

/// The 128-bit mask of a kit's modifier block: bit `i` for each modifier
/// that is not 0.
pub fn modifier_mask(modifiers: &[f32; MODIFIERS]) -> u128 {
    modifiers
        .iter()
        .enumerate()
        .filter(|&(_, &value)| value != 0.0)
        .fold(0, |mask, (index, _)| mask | 1 << index)
}

/// Writes a kit's modifier block (`static_info.md` §3.1, `lobby.md` §2.5):
/// the mask as two u64, **bits 64..=127 first**, then bits 0..=63, then one
/// f32 for each set bit, lowest first. The client adds each to a zeroed
/// modifier, so a modifier of 0 is left out.
pub fn write_modifier_block(modifiers: &[f32; MODIFIERS], out: &mut Vec<u8>) {
    let mask = modifier_mask(modifiers);
    out.extend_from_slice(&((mask >> 64) as u64).to_le_bytes());
    out.extend_from_slice(&(mask as u64).to_le_bytes());
    for value in modifiers.iter().filter(|&&value| value != 0.0) {
        out.extend_from_slice(&value.to_le_bytes());
    }
}

/// [`write_modifier_block`] into a new buffer.
pub fn modifier_block(modifiers: &[f32; MODIFIERS]) -> Vec<u8> {
    let mut out = Vec::new();
    write_modifier_block(modifiers, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn index(name: &str) -> usize {
        modifier_index(name).unwrap()
    }

    #[test]
    fn reads_the_026g_skills_tree() {
        let tree = SkillsTree::shared();
        assert_eq!(tree.skills.len(), 26);
        let ids: Vec<u16> = tree.skills.iter().map(|skill| skill.id).collect();
        let expected: Vec<u16> = (101..=113).chain((201..=211).chain([213, 214])).collect();
        assert_eq!(ids, expected);
        // `(levels, branch_points)` of each, data.md §3.
        #[rustfmt::skip]
        let table = [
            (101, 5, 0), (102, 5, 0), (103, 3, 5), (104, 2, 5), (105, 3, 10), (106, 5, 10),
            (107, 5, 10), (108, 3, 10), (109, 1, 15), (110, 5, 15), (111, 1, 20), (112, 1, 15),
            (113, 1, 20), (201, 5, 0), (202, 5, 10), (203, 2, 5), (204, 5, 0), (205, 2, 15),
            (206, 3, 50), (207, 2, 10), (208, 5, 10), (209, 5, 5), (210, 0, 50), (211, 5, 15),
            (213, 1, 20), (214, 1, 20),
        ];
        for (id, levels, branch_points) in table {
            let skill = tree.skill(id).unwrap();
            assert_eq!(
                (skill.levels_amount, skill.branch_points),
                (levels, branch_points),
                "{id}"
            );
        }
        assert_eq!(tree.skill(111).unwrap().antiskills, [113]);
        assert_eq!(tree.skill(113).unwrap().antiskills, [111]);
        assert_eq!(tree.skill(213).unwrap().antiskills, [214]);
        assert_eq!(tree.skill(214).unwrap().antiskills, [213]);
        let exclusive = tree.skills.iter().filter(|s| !s.antiskills.is_empty());
        assert_eq!(exclusive.count(), 4);
        // A row of the window is 5 branch points, but for 206 and 210.
        for skill in &tree.skills {
            if ![206, 210].contains(&skill.id) {
                assert_eq!(
                    skill.branch_points,
                    (u32::from(skill.row) - 1) * 5,
                    "{}",
                    skill.id
                );
                assert_eq!(skill.levels.len(), usize::from(skill.levels_amount));
            }
        }
    }

    #[test]
    fn modifier_names_map_to_the_exes_indexes() {
        assert_eq!(index("dispersion_modifier"), 0);
        assert_eq!(index("max_carried_weight_modifier"), 5);
        assert_eq!(index("pocket_slots_modifier"), 15);
        assert_eq!(index("ammo_bags"), 16);
        assert_eq!(index("stamina_spending_speed_modifier"), 19);
        assert_eq!(index("movement_speed_when_aiming_modifier"), 20);
        assert_eq!(index("aim_time_modifier"), 49);
        assert_eq!(index("stamina_spending_speed_skill_modifier"), 67);
        assert_eq!(modifier_index("meele_hit_range_modifier"), None);
        // No name twice.
        for (i, name) in MODIFIER_NAMES.iter().enumerate() {
            assert_eq!(modifier_index(name), Some(i));
        }
        // Only the two melee skills name a modifier outside the 68.
        let tree = SkillsTree::shared();
        for skill in &tree.skills {
            let expected: &[&str] = match skill.id {
                206 => &["meele_hit_ultimate_modifier"],
                210 => &["meele_hit_range_modifier"],
                _ => &[],
            };
            assert_eq!(skill.unknown_modifiers, expected, "{}", skill.id);
        }
    }

    #[test]
    fn a_skill_adds_its_levels() {
        let tree = SkillsTree::shared();
        // 209: +2 kg carried a level, five levels.
        let mut expected = [0.0; MODIFIERS];
        expected[5] = 10.0;
        assert_eq!(tree.modifiers(&[(209, 5)]), expected);
        expected[5] = 4.0;
        assert_eq!(tree.modifiers(&[(209, 2)]), expected);
        // Points past the skill's levels add nothing more.
        expected[5] = 10.0;
        assert_eq!(tree.modifiers(&[(209, 9)]), expected);

        // 101: aim time -0.02 a level.
        let aim = tree.modifiers(&[(101, 3)])[index("aim_time")];
        assert!((aim + 0.06).abs() < 1e-6);
        // 113 sets two modifiers; 205 three, one of them 0 a level.
        let two = tree.modifiers(&[(113, 1)]);
        assert_eq!(two[index("disable_second_weapon")], 1.0);
        assert_eq!(two[index("pocket_slots")], 1.0);
        assert_eq!(modifier_mask(&two).count_ones(), 2);
        for (name, at) in [
            ("pocket_slots", slot_modifier::POCKET_SLOTS),
            ("pocket_slot_size", slot_modifier::POCKET_SLOT_SIZE),
            (
                "use_two_double_handed_weapons",
                slot_modifier::USE_TWO_DOUBLE_HANDED_WEAPONS,
            ),
            ("armored_trap", slot_modifier::MEDICAL_RANKS),
            (
                "disable_second_weapon",
                slot_modifier::DISABLE_SECOND_WEAPON,
            ),
        ] {
            assert_eq!(index(name), at);
        }
        let boost = tree.modifiers(&[(205, 2)]);
        assert_eq!(boost[index("stamina_boost_time")], 200.0);
        assert_eq!(boost[index("stamina_boost_interval")], 0.0);
        assert_eq!(boost[index("stamina_boost_amount")], 0.2);
        // Several skills, and ones the tree does not have.
        let both = tree.modifiers(&[(209, 1), (208, 5), (999, 3), (210, 1)]);
        assert_eq!(both[5], 2.0);
        assert!((both[4] - 0.05).abs() < 1e-6);
        assert_eq!(modifier_mask(&both).count_ones(), 2);
        assert_eq!(tree.modifiers(&[]), [0.0; MODIFIERS]);
    }

    #[test]
    fn the_modifier_block_is_high_qword_first() {
        let mut modifiers = [0.0; MODIFIERS];
        assert_eq!(modifier_block(&modifiers), [0; 16]);

        modifiers[5] = 10.0; // max_carried_weight
        modifiers[0] = -0.1; // dispersion
        modifiers[67] = -0.04; // stamina_spending_speed_skill: bit 3 of the high qword
        modifiers[64] = 1.0; // disable_second_weapon: bit 0 of the high qword
        let mut expected = Vec::new();
        expected.extend_from_slice(&[0x09, 0, 0, 0, 0, 0, 0, 0]); // bits 64..=127
        expected.extend_from_slice(&[0x21, 0, 0, 0, 0, 0, 0, 0]); // bits 0..=63
        for value in [-0.1f32, 10.0, 1.0, -0.04] {
            expected.extend_from_slice(&value.to_le_bytes());
        }
        assert_eq!(modifier_block(&modifiers), expected);
        assert_eq!(modifier_mask(&modifiers), (0x09 << 64) | 0x21);

        // Bit 63 is the last of the low qword.
        let mut modifiers = [0.0; MODIFIERS];
        modifiers[63] = 0.5;
        let block = modifier_block(&modifiers);
        assert_eq!(block[..8], [0; 8]);
        assert_eq!(block[8..16], [0, 0, 0, 0, 0, 0, 0, 0x80]);
        assert_eq!(block[16..], 0.5f32.to_le_bytes());

        // All 68.
        let block = modifier_block(&[1.0; MODIFIERS]);
        assert_eq!(block.len(), 16 + 68 * 4);
        assert_eq!(block[..8], [0x0F, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(block[8..16], [0xFF; 8]);
    }

    #[test]
    fn points_are_the_level_and_a_skill_has_its_levels() {
        let tree = SkillsTree::shared();
        assert_eq!(tree.validate(0, &[]), Ok(()));
        assert_eq!(tree.validate(5, &[(101, 5)]), Ok(()));
        assert_eq!(tree.validate(5, &[(101, 3), (201, 2)]), Ok(()));
        assert_eq!(
            tree.validate(4, &[(101, 5)]),
            Err(SkillError::OverLevel { spent: 5, level: 4 })
        );
        assert_eq!(
            tree.validate(50, &[(101, 6)]),
            Err(SkillError::TooManyPoints {
                skill: 101,
                points: 6,
                most: 5
            })
        );
        assert_eq!(
            tree.validate(50, &[(100, 1)]),
            Err(SkillError::Unknown(100))
        );
        assert_eq!(
            tree.validate(50, &[(101, 1), (101, 1)]),
            Err(SkillError::Repeated(101))
        );
        // A skill named with no points is fine anywhere.
        assert_eq!(tree.validate(0, &[(113, 0), (209, 0)]), Ok(()));
    }

    #[test]
    fn a_row_needs_its_branch_points_in_the_rows_above() {
        let tree = SkillsTree::shared();
        // 209 (row 2, 5 branch points) needs five points in row 1 of branch 2.
        assert_eq!(
            tree.validate(50, &[(209, 1)]),
            Err(SkillError::Locked {
                skill: 209,
                needs: 5,
                unlocked: 0
            })
        );
        assert_eq!(
            tree.validate(50, &[(201, 4), (209, 1)]),
            Err(SkillError::Locked {
                skill: 209,
                needs: 5,
                unlocked: 0
            })
        );
        assert_eq!(tree.validate(50, &[(201, 5), (209, 5)]), Ok(()));
        assert_eq!(tree.validate(50, &[(201, 3), (204, 2), (209, 5)]), Ok(()));
        // Points in the other branch do not count.
        assert_eq!(
            tree.validate(50, &[(101, 5), (209, 1)]),
            Err(SkillError::Locked {
                skill: 209,
                needs: 5,
                unlocked: 0
            })
        );
        // Row 3 (10) needs ten points in rows 1 and 2; the row's own points
        // do not count for itself.
        assert_eq!(tree.unlocked_branch_points(2, &[(201, 5), (209, 5)]), 10);
        assert_eq!(tree.validate(50, &[(201, 5), (209, 5), (208, 5)]), Ok(()));
        assert_eq!(
            tree.validate(50, &[(201, 5), (209, 4), (208, 5)]),
            Err(SkillError::Locked {
                skill: 208,
                needs: 10,
                unlocked: 5
            })
        );
        // The whole of branch 1 down to the last row.
        let deep = [
            (101, 5),
            (102, 5),
            (103, 3),
            (104, 2),
            (106, 5),
            (110, 5),
            (111, 1),
        ];
        assert_eq!(tree.unlocked_branch_points(1, &deep), 20);
        assert_eq!(tree.validate(26, &deep), Ok(()));
        assert_eq!(tree.unlocked_branch_points(1, &[]), 0);
        // 206 and 210 ask for 50 branch points, which nothing unlocks.
        let full = [
            (201, 5),
            (204, 5),
            (203, 2),
            (209, 5),
            (202, 5),
            (208, 5),
            (211, 5),
        ];
        assert_eq!(tree.unlocked_branch_points(2, &full), 20);
        let mut with = full.to_vec();
        with.push((206, 1));
        assert_eq!(
            tree.validate(99, &with),
            Err(SkillError::Locked {
                skill: 206,
                needs: 50,
                unlocked: 20
            })
        );
    }

    #[test]
    fn the_players_tree_without_206_and_210_unlocks_as_the_stock_one() {
        // physical-skills-1.json: 206 and 210 out, 202 to column 4 of its row.
        let stock = SkillsTree::shared();
        let mut players = stock.clone();
        players
            .skills
            .retain(|skill| ![206, 210].contains(&skill.id));
        let packrat = players.skills.iter_mut().find(|skill| skill.id == 202);
        packrat.unwrap().column = 4;
        let sets: [&[(u16, u8)]; 5] = [
            &[],
            &[(201, 5)],
            &[(201, 3), (204, 2), (209, 5)],
            &[(201, 5), (209, 4), (208, 5)],
            &[
                (201, 5),
                (204, 5),
                (203, 2),
                (209, 5),
                (202, 5),
                (208, 5),
                (211, 5),
            ],
        ];
        for set in sets {
            assert_eq!(
                players.unlocked_branch_points(2, set),
                stock.unlocked_branch_points(2, set),
                "{set:?}"
            );
            assert_eq!(
                players.validate(50, set),
                stock.validate(50, set),
                "{set:?}"
            );
        }
        // A point in either was refused before and is now.
        for id in [206, 210] {
            assert!(stock.validate(99, &[(id, 1)]).is_err());
            assert_eq!(
                players.validate(99, &[(id, 1)]),
                Err(SkillError::Unknown(id))
            );
        }
    }

    #[test]
    fn exclusive_skills_do_not_go_together() {
        let tree = SkillsTree::shared();
        let base = [(101, 5), (102, 5), (103, 3), (104, 2), (106, 5)];
        let with = |extra: &[(u16, u8)]| [&base[..], extra].concat();
        assert_eq!(tree.validate(50, &with(&[(111, 1)])), Ok(()));
        assert_eq!(tree.validate(50, &with(&[(113, 1)])), Ok(()));
        assert_eq!(
            tree.validate(50, &with(&[(111, 1), (113, 1)])),
            Err(SkillError::Exclusive(111, 113))
        );
        // One of the pair named with no points does not lock the other.
        assert_eq!(tree.validate(50, &with(&[(111, 1), (113, 0)])), Ok(()));
        let body = [(201, 5), (204, 5), (209, 5), (202, 5), (213, 1), (214, 1)];
        assert_eq!(
            tree.validate(50, &body),
            Err(SkillError::Exclusive(213, 214))
        );
    }

    /// `(id, levels, branch_points, column, row, antiskills)`.
    type Row = (u16, u8, u32, u8, u8, &'static [u16]);

    /// The medical branch as the content builds it (medical-skills.md):
    /// Brothers in Arms (342) in row 1, one rank.
    fn medical_tree() -> SkillsTree {
        medical_tree_with((342, 1, 0, 1, 1, &[]))
    }

    /// The branch builds were saved in before 2026-11-14: Brothers in Arms
    /// in row 3, two ranks.
    fn medical_tree_before_brothers_moved() -> SkillsTree {
        medical_tree_with((342, 2, 10, 1, 3, &[]))
    }

    fn medical_tree_with(brothers: Row) -> SkillsTree {
        #[rustfmt::skip]
        let rows: [Row; 13] = [
            brothers, (301, 3, 0, 2, 1, &[]), (302, 2, 0, 3, 1, &[]),
            (311, 5, 3, 1, 2, &[]), (313, 2, 3, 3, 2, &[]), (312, 3, 3, 4, 2, &[]),
            (303, 2, 10, 2, 3, &[]), (321, 3, 10, 4, 3, &[]),
            (332, 5, 15, 2, 4, &[]), (323, 4, 15, 3, 4, &[333]), (333, 4, 15, 4, 4, &[323]),
            (331, 1, 20, 2, 5, &[322]), (322, 1, 20, 3, 5, &[331]),
        ];
        let mut skills: Vec<Skill> = rows
            .iter()
            .map(
                |&(id, levels_amount, branch_points, column, row, antiskills)| Skill {
                    id,
                    levels_amount,
                    branch_points,
                    antiskills: antiskills.to_vec(),
                    column,
                    row,
                    levels: vec![Vec::new(); levels_amount.into()],
                    unknown_modifiers: Vec::new(),
                },
            )
            .collect();
        skills.sort_by_key(|skill| skill.id);
        SkillsTree { skills }
    }

    #[test]
    fn a_changed_tree_settles_saved_skills_by_refunding() {
        let tree = medical_tree();
        let full = [
            (301, 3),
            (302, 2),
            (303, 2),
            (311, 5),
            (312, 3),
            (313, 2),
            (321, 3),
            (322, 0),
            (323, 4),
            (331, 1),
            (332, 5),
            (342, 1),
        ];
        assert_eq!(tree.validate(100, &full), Ok(()));
        assert_eq!(tree.settle(100, &full), full);
        // A full build of the tree before (Numb Legs in row 1, Unflinching
        // at its old two ranks) is still valid: nothing is taken back.
        let mut before = full;
        before[8] = (323, 2);
        assert_eq!(tree.validate(100, &before), Ok(()));
        assert_eq!(tree.settle(100, &before), before);
        // Numb Legs saved in the old row 1 stays when rows 1 and 2 hold its
        // ten points now (row 3), and goes when they do not.
        let numb = [(303, 2), (301, 3), (302, 2), (311, 5)];
        assert_eq!(tree.settle(100, &numb), numb);
        assert_eq!(
            tree.settle(100, &[(303, 2), (301, 3), (302, 2)]),
            [(301, 3), (302, 2)]
        );
        // Row 2 opens at 3 points: the old save that reached it with Numb
        // Legs' two keeps row 2 and gives Numb Legs back.
        assert_eq!(
            tree.settle(100, &[(303, 2), (301, 3), (311, 2)]),
            [(301, 3), (311, 2)]
        );
        // Ranks past the new levels are taken back; Medic (341) is gone.
        assert_eq!(
            tree.settle(100, &[(301, 5), (302, 5), (311, 5), (341, 1), (342, 5)]),
            [(301, 3), (302, 2), (311, 5), (342, 1)]
        );
        // Without row 2 the points of row 1 no longer reach row 3; Brothers
        // in Arms, a row-1 skill now, stays at its one rank.
        assert_eq!(
            tree.settle(100, &[(301, 5), (302, 5), (321, 5), (342, 5)]),
            [(301, 3), (302, 2), (342, 1)]
        );
        // Both of the new exclusive pair: Supply Pocket stays.
        let both = [
            (301, 3),
            (302, 2),
            (303, 2),
            (311, 5),
            (312, 3),
            (313, 2),
            (321, 3),
            (322, 1),
            (323, 4),
            (331, 1),
            (332, 5),
        ];
        let settled = tree.settle(100, &both);
        assert!(!settled.iter().any(|&(id, _)| id == 322));
        assert!(settled.contains(&(331, 1)));
        assert_eq!(tree.validate(100, &settled), Ok(()));
        // Concentration instead of Unflinching is a full build too; both
        // (a set the tree never made) is refused, and settling takes
        // Unflinching back, its points free again, Concentration kept.
        let mut concentration = full;
        concentration[8] = (333, 4);
        assert_eq!(tree.validate(100, &concentration), Ok(()));
        assert_eq!(tree.settle(100, &concentration), concentration);
        let mut both = full.to_vec();
        both.push((333, 4));
        assert_eq!(
            tree.validate(100, &both),
            Err(SkillError::Exclusive(323, 333))
        );
        let settled = tree.settle(100, &both);
        assert!(!settled.iter().any(|&(id, _)| id == 323));
        assert!(settled.contains(&(333, 4)));
        assert_eq!(tree.validate(100, &settled), Ok(()));
        assert_eq!(settled.iter().map(|&(_, n)| u32::from(n)).sum::<u32>(), 31);
        // A row no longer reached goes, and with it the rows it unlocked:
        // 302 at 5 is 2 now, under row 2's three points, so row 2 goes and
        // row 3 with it.
        assert_eq!(
            tree.settle(100, &[(302, 5), (311, 5), (321, 5)]),
            [(302, 2)]
        );
        // 301 at 5 is 3 now: row 2 stays, row 3 (ten) is out of reach at 8.
        assert_eq!(
            tree.settle(100, &[(301, 5), (311, 5), (321, 5)]),
            [(301, 3), (311, 5)]
        );
        // The old tier-4 Supply Pocket is out of reach at 16 of 20.
        assert_eq!(
            tree.settle(
                100,
                &[(301, 5), (302, 5), (311, 5), (312, 5), (321, 5), (331, 1)]
            ),
            [(301, 3), (302, 2), (311, 5), (312, 3), (321, 3)]
        );
        // A branch the tree does not have is not this tree's to change.
        assert_eq!(
            tree.settle(100, &[(101, 9), (301, 3)]),
            [(101, 9), (301, 3)]
        );
    }

    /// A row unlocks the branch points it asks, not rounded down to fives:
    /// the medical tree's second row opens at 3.
    #[test]
    fn a_row_opens_at_its_own_branch_points() {
        let tree = medical_tree();
        assert_eq!(tree.unlocked_branch_points(3, &[(301, 2)]), 0);
        assert_eq!(tree.unlocked_branch_points(3, &[(301, 3)]), 3);
        assert_eq!(tree.validate(100, &[(301, 3), (311, 1)]), Ok(()));
        assert_eq!(
            tree.validate(100, &[(301, 2), (311, 1)]),
            Err(SkillError::Locked {
                skill: 311,
                needs: 3,
                unlocked: 0
            })
        );
        // Row 3 (ten) counts rows 1 and 2: 5 + 5.
        assert_eq!(
            tree.unlocked_branch_points(3, &[(301, 3), (302, 2), (311, 5)]),
            10
        );
        // Brothers in Arms' point is a row-1 point: two more open row 2.
        assert_eq!(tree.unlocked_branch_points(3, &[(342, 1), (301, 2)]), 3);
    }

    /// Brothers in Arms went from row 3 (two ranks of -25%) to row 1 (one
    /// rank of -50%) on 2026-11-14 (medical-skills.md). A build saved before
    /// keeps all it holds but the second rank, whose point comes back,
    /// unless that point was the last one its row 4 or row 5 asked.
    #[test]
    fn brothers_in_arms_in_row_one_settles_the_builds_saved_before() {
        let (before, now) = (medical_tree_before_brothers_moved(), medical_tree());
        let settle = |saved: &[(u16, u8)]| {
            assert_eq!(before.validate(100, saved), Ok(()), "{saved:?}");
            let settled = now.settle(100, saved);
            assert_eq!(now.validate(100, &settled), Ok(()), "{saved:?}");
            // Settled once, it stays as it is.
            assert_eq!(now.settle(100, &settled), settled, "{saved:?}");
            settled
        };
        // One rank, in a build with no point to spare (rows 1-3 hold 15,
        // rows 1-4 hold 20): the point counts from row 1, nothing changes.
        #[rustfmt::skip]
        let one = [(301, 3), (302, 2), (311, 5), (342, 1), (303, 2), (321, 2), (332, 5), (331, 1)];
        assert_eq!(settle(&one), one);
        // Two ranks: the second comes back. The full build of before (32
        // points) is the full build now (31).
        #[rustfmt::skip]
        let full = [
            (301, 3), (302, 2), (311, 5), (312, 3), (313, 2), (342, 2), (303, 2), (321, 3),
            (332, 5), (323, 4), (331, 1),
        ];
        let mut full_now = full;
        full_now[5] = (342, 1);
        assert_eq!(settle(&full), full_now);
        // Rows 1-3 held 16 with both ranks, 15 now: row 4 and row 5 stay.
        #[rustfmt::skip]
        let spare = [(301, 3), (302, 2), (311, 5), (312, 3), (313, 1), (342, 2), (332, 5), (322, 1)];
        let mut spare_now = spare;
        spare_now[5] = (342, 1);
        assert_eq!(settle(&spare), spare_now);
        // Rows 1-3 held exactly row 4's 15 with both ranks, 14 now: row 4
        // goes, and row 5 with it (21 points held, 14 now).
        #[rustfmt::skip]
        let fifteen = [(301, 3), (302, 2), (311, 5), (312, 3), (342, 2), (332, 5), (331, 1)];
        assert_eq!(
            settle(&fifteen),
            [(301, 3), (302, 2), (311, 5), (312, 3), (342, 1)]
        );
        // Rows 1-4 held exactly row 5's 20 with both ranks, 19 now: the
        // tier-5 unlock goes, row 4 stays (21 points held, 19 now).
        #[rustfmt::skip]
        let twenty = [(301, 3), (302, 2), (311, 5), (312, 3), (313, 1), (342, 2), (323, 4), (322, 1)];
        let mut twenty_now = twenty.to_vec();
        twenty_now[5] = (342, 1);
        twenty_now.pop();
        assert_eq!(settle(&twenty), twenty_now);

        // Every build the tree before could save, by the points held in
        // each row (a row's skills filled in turn, from either end) and the
        // ranks of Brothers in Arms: those two cases are the only ones that
        // lose more than the second rank, and what they lose is a whole row.
        const ROWS: [&[(u16, u8)]; 5] = [
            &[(301, 3), (302, 2)],
            &[(311, 5), (312, 3), (313, 2)],
            &[(303, 2), (321, 3)],
            &[(332, 5), (323, 4)],
            &[(331, 1)],
        ];
        let options = ROWS.map(|row| row.iter().map(|&(_, most)| u32::from(most)).sum::<u32>() + 1);
        let (mut saved_builds, mut lose_row_four, mut lose_row_five_alone) = (0, 0, 0);
        for reverse in [false, true] {
            let build = |rows: [u8; 5], brothers: u8| {
                let mut build = Vec::new();
                for (row, mut left) in ROWS.iter().zip(rows) {
                    let mut skills = row.to_vec();
                    if reverse {
                        skills.reverse();
                    }
                    for (id, most) in skills {
                        let points = left.min(most);
                        left -= points;
                        if points > 0 {
                            build.push((id, points));
                        }
                    }
                }
                if brothers > 0 {
                    build.push((342, brothers));
                }
                build
            };
            for mut code in 0..options.iter().product::<u32>() * 3 {
                let mut digit = |options: u32| {
                    let digit = (code % options) as u8;
                    code /= options;
                    digit
                };
                let rows = options.map(&mut digit);
                let brothers = digit(3);
                let saved = build(rows, brothers);
                if before.validate(100, &saved).is_err() {
                    continue;
                }
                // Rows 1-3 beside Brothers in Arms: with both ranks they
                // held two more, now one. Row 4 asks 15, row 5 asks 20.
                let upper = rows[0] + rows[1] + rows[2];
                let four_goes = brothers == 2 && upper == 13 && rows[3] > 0;
                let five_goes =
                    brothers == 2 && rows[4] > 0 && (upper == 13 || upper + rows[3] == 18);
                let mut kept = rows;
                if four_goes {
                    kept[3] = 0;
                }
                if five_goes {
                    kept[4] = 0;
                }
                assert_eq!(settle(&saved), build(kept, brothers.min(1)), "{saved:?}");
                saved_builds += 1;
                lose_row_four += u32::from(four_goes);
                lose_row_five_alone += u32::from(five_goes && !four_goes);
            }
        }
        // Of the 3,124 distributions the tree before allowed (each from
        // both ends), 168 lose row 4 and 56 the tier-5 unlock alone.
        assert_eq!(
            (saved_builds, lose_row_four, lose_row_five_alone),
            (2 * 3_124, 2 * 168, 2 * 56)
        );
    }
}
