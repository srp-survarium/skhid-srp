//! Skills: the tree (`gameplay/skills/skills_tree.options`, 69 skills in 5 branches), the point rules and the reset price.
//!
//! PROVEN: skill ids are 3 digits `B R C` (branch 1..5, tier row 1..5, column); each skill has
//! `levels_amount` levels (1..5) and `preconditions.branch_points` in {0, 5, 10, 15, 20} = tier. The client sends
//! `47 00 u8 n, n x {u16 id, u8 points}` (the whole learned set) and gets its skill list back in 8C.
//! Points available = level - 1 (AS3 level-up message "+N ability" per level gained; LIKELY no start point). LIKELY server rules: a skill with `branch_points` N needs
//! N points in other skills of the same branch; learned points can only be removed by a reset; a reset costs
//! `price * points` of the reset branch, the price per point in 8D (+0 silver, +4 gold).
//! PROVEN: the 47 01 branch byte is 0..4 = `id / 100 - 1` (exe 0x2001C0700, UI
//! `TAB_IDS`), one branch per reset, never "all"; the shown cost is price x points of that branch.

use std::collections::BTreeMap;

use serde::Deserialize;

/// Table (`data/skills_069d.json`, made by `crates/data-extract`).
pub const SKILLS_FILE: &str = "skills_069d.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct SkillRow {
    pub id: u16,
    /// `levels_amount`: most points a skill takes
    pub levels: u8,
    /// points needed in the branch before the skill can be learned
    pub branch_points: u32,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawSkills {
    pub skills: Vec<SkillRow>,
}

/// Price of a reset per learned point (8D costs 0 and 1: the client multiplies them by the points itself).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkillPrices {
    pub reset_silver_per_point: u32,
    pub reset_gold_per_point: u32,
}

impl Default for SkillPrices {
    /// The server's own choice (the client has no price table: 8D carries it).
    fn default() -> Self {
        SkillPrices { reset_silver_per_point: 1000, reset_gold_per_point: 5 }
    }
}

/// How a reset is paid (the second byte after `47 01 <branch>`): `set_reroll_cost(regular, premium)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResetPayment {
    Silver,
    Gold,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SkillError {
    #[error("unknown skill {0}")]
    Unknown(u16),
    #[error("skill {0} listed twice")]
    Duplicate(u16),
    #[error("skill {id}: {points} points, at most {max}")]
    TooManyPoints { id: u16, points: u8, max: u8 },
    #[error("skill {0}: points cannot be taken back without a reset")]
    Decrease(u16),
    #[error("{spent} points spent, {available} available")]
    OverBudget { spent: u32, available: u32 },
    #[error("skill {id} needs {need} points in its branch, has {have}")]
    Locked { id: u16, need: u32, have: u32 },
    #[error("branch {0} does not exist")]
    NoBranch(u8),
    #[error("payment type {0} unknown")]
    NoPayment(u8),
    #[error("not enough money for the reset")]
    NoMoney,
}

#[derive(Debug, Default)]
pub struct Skills {
    rows: BTreeMap<u16, SkillRow>,
    pub prices: SkillPrices,
}

/// Branch of a skill id (hundreds digit, 1..=5).
pub fn branch_of(id: u16) -> u8 {
    (id / 100) as u8
}

/// Branch byte of 47 01 for a skill id (`id / 100 - 1`, 0..=4; exe 0x2001C0700).
pub fn reset_branch_of(id: u16) -> u8 {
    branch_of(id).wrapping_sub(1)
}

/// Total points of a learned set.
pub fn spent(skills: &BTreeMap<u16, u8>) -> u32 {
    skills.values().map(|&p| p as u32).sum()
}

impl Skills {
    pub(crate) fn new(raw: RawSkills) -> Skills {
        Skills { rows: raw.skills.into_iter().map(|r| (r.id, r)).collect(), prices: SkillPrices::default() }
    }

    pub fn row(&self, id: u16) -> Option<&SkillRow> {
        self.rows.get(&id)
    }

    pub fn rows(&self) -> impl Iterator<Item = &SkillRow> {
        self.rows.values()
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Points available to learn given the "unlocked" byte of 8C/87 (`LevelFields::skill_points`, level - 1: see
    /// `Levels::skill_points_for_level`). The client takes available = unlocked - learned (8C), so the server
    /// budget is the same number.
    pub fn points_for_level(&self, unlocked: u32) -> u32 {
        unlocked
    }

    /// Check a requested learned set (`47 00`: the whole set after the player's Apply) against the stored one.
    /// Returns the new set (zero entries dropped). Points are only added: taking points back is the reset.
    pub fn apply(&self, current: &BTreeMap<u16, u8>, requested: &[(u16, u8)], available: u32) -> Result<BTreeMap<u16, u8>, SkillError> {
        let mut new = BTreeMap::new();
        for &(id, pts) in requested {
            let row = self.rows.get(&id).ok_or(SkillError::Unknown(id))?;
            if new.contains_key(&id) {
                return Err(SkillError::Duplicate(id));
            }
            if pts > row.levels {
                return Err(SkillError::TooManyPoints { id, points: pts, max: row.levels });
            }
            if pts > 0 {
                new.insert(id, pts);
            }
        }
        for (&id, &p) in current {
            if new.get(&id).copied().unwrap_or(0) < p {
                return Err(SkillError::Decrease(id));
            }
        }
        let total = spent(&new);
        if total > available {
            return Err(SkillError::OverBudget { spent: total, available });
        }
        for &id in new.keys() {
            let row = &self.rows[&id];
            let have: u32 = new.iter().filter(|&(&o, _)| o != id && branch_of(o) == branch_of(id)).map(|(_, &p)| p as u32).sum();
            if have < row.branch_points {
                return Err(SkillError::Locked { id, need: row.branch_points, have });
            }
        }
        Ok(new)
    }

    /// Points a reset of wire `branch` (0..=4) takes back.
    pub fn reset_points(&self, skills: &BTreeMap<u16, u8>, branch: u8) -> Result<u32, SkillError> {
        if branch > 4 {
            return Err(SkillError::NoBranch(branch));
        }
        Ok(skills.iter().filter(|&(&id, _)| reset_branch_of(id) == branch).map(|(_, &p)| p as u32).sum())
    }

    /// Price of a reset: (silver, gold) the payment type costs for `points` points.
    pub fn reset_cost(&self, points: u32, pay: ResetPayment) -> u64 {
        match pay {
            ResetPayment::Silver => points as u64 * self.prices.reset_silver_per_point as u64,
            ResetPayment::Gold => points as u64 * self.prices.reset_gold_per_point as u64,
        }
    }

    /// Payment type of the request's last byte (0 regular = silver, 1 premium = gold; `checkBoxRegularCost.money_type` 0,
    /// `checkBoxPremiumCost.money_type` 1 in MessageAskReroll.as).
    pub fn payment(byte: u8) -> Result<ResetPayment, SkillError> {
        match byte {
            0 => Ok(ResetPayment::Silver),
            1 => Ok(ResetPayment::Gold),
            b => Err(SkillError::NoPayment(b)),
        }
    }

    /// The learned set without the skills of wire `branch` (0..=4).
    pub fn without_branch(&self, skills: &BTreeMap<u16, u8>, branch: u8) -> BTreeMap<u16, u8> {
        skills.iter().filter(|&(&id, _)| reset_branch_of(id) != branch).map(|(&i, &p)| (i, p)).collect()
    }

    /// The match options' name keys (`"skill_543": 3`) of a learned set.
    pub fn match_keys(skills: &BTreeMap<u16, u8>) -> BTreeMap<String, u32> {
        skills.iter().map(|(id, &p)| (format!("skill_{id}"), p as u32)).collect()
    }
}
