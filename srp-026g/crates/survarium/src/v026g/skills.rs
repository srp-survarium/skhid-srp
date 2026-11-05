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
//!   `floor(branch_points / 5) * 5` (at most 25), and a skill can take
//!   points when its `branch_points` is no more than that. So a row needs
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

/// The suffix every modifier's key has in the configs.
const SUFFIX: &str = "_modifier";

/// The most `branch_points` the skills window's walk takes (`SKILL_LEVELS`).
const MAX_BRANCH_POINTS: u32 = 25;
/// `POINTS_PER_LEVEL`: the points a row of the window stands for.
const POINTS_PER_ROW: u32 = 5;

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
        let (mut before, mut rows) = (0, 0);
        for skill in walk {
            if skill.branch_points > MAX_BRANCH_POINTS {
                continue;
            }
            if skill.branch_points > before {
                break;
            }
            rows = skill.branch_points / POINTS_PER_ROW;
            before += skills
                .iter()
                .filter(|&&(id, _)| id == skill.id)
                .map(|&(_, points)| u32::from(points))
                .sum::<u32>();
        }
        rows.min(MAX_BRANCH_POINTS / POINTS_PER_ROW) * POINTS_PER_ROW
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
        // physical-skills-1.json: 206 and 210 out, 209 to column 4 of its row.
        let stock = SkillsTree::shared();
        let mut players = stock.clone();
        players
            .skills
            .retain(|skill| ![206, 210].contains(&skill.id));
        let packrat = players.skills.iter_mut().find(|skill| skill.id == 209);
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
}
