//! Account level from experience, faction level from reputation points, match rewards.

/// The five fields shared by 87 (tail) and 8C: u8 level, u8 skill points (= level), u32 experience,
/// u32 previous-level threshold, u32 next-level threshold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LevelFields {
    pub level: u8,
    pub skill_points: u8,
    pub xp: u32,
    pub prev: u32,
    pub next: u32,
}

impl LevelFields {
    /// 14 bytes, wire order.
    pub fn to_bytes(&self) -> [u8; 14] {
        let mut b = [0u8; 14];
        b[0] = self.level;
        b[1] = self.skill_points;
        b[2..6].copy_from_slice(&self.xp.to_le_bytes());
        b[6..10].copy_from_slice(&self.prev.to_le_bytes());
        b[10..14].copy_from_slice(&self.next.to_le_bytes());
        b
    }
}

#[derive(Debug, Default)]
pub struct Levels {
    /// total XP at which level i+2 starts (level 1 = 0..table[0])
    xp_table: Vec<u32>,
    /// (reward amount, currency) per level
    rewards: Vec<(u32, u32)>,
}

impl Levels {
    pub(crate) fn new(xp_table: Vec<u32>, rewards: Vec<(u32, u32)>) -> Levels {
        Levels { xp_table, rewards }
    }

    pub fn xp_table(&self) -> &[u32] {
        &self.xp_table
    }

    /// Highest character level: 100 (PROVEN by the data: `character_leveling` has 100 rows, row i = the experience that
    /// ends level i + 1, so the last row is the bar of level 100; OFFICIAL: "level 100 and Prestige 10 in all factions",
    /// Steam Q&A 2021-03 gid 4048146376729246088, decals for the levels 10, 20, ... 100 in 0.66).
    pub fn max_level(&self) -> u32 {
        self.xp_table.len() as u32
    }

    /// Level of an experience total: 1 + number of thresholds reached, capped at [`Levels::max_level`].
    pub fn level_of(&self, xp: u32) -> u32 {
        (1 + self.xp_table.iter().filter(|&&t| xp >= t).count() as u32).min(self.max_level())
    }

    /// Silver the level-ups between two experience totals pay: the sum of `reward_amount` of every level reached
    /// (`character_leveling[level - 2]`, currency 0 = silver). The client's level-up window lists the same rewards from
    /// its own copy of the table (LobbyMenu `MessagePlayerRank`); the server pays them.
    pub fn level_up_silver(&self, old_xp: u32, new_xp: u32) -> u32 {
        let (a, b) = (self.level_of(old_xp), self.level_of(new_xp));
        (a + 1..=b).filter_map(|l| self.reward_for_level(l)).filter(|&(_, cur)| cur == 0).map(|(n, _)| n).sum()
    }

    pub fn level_fields(&self, xp: u32) -> LevelFields {
        let lvl = self.level_of(xp) as usize;
        let prev = if lvl >= 2 { self.xp_table[lvl - 2] } else { 0 };
        let next = self.xp_table.get(lvl - 1).or(self.xp_table.last()).copied().unwrap_or(0);
        LevelFields { level: lvl.min(255) as u8, skill_points: Self::skill_points_for_level(lvl as u32) as u8, xp, prev, next }
    }

    /// Skill points unlocked at `level`: one per level GAINED, so level 1 has none (rule below).
    ///
    /// PROVEN (AS3, `MessagePlayerRank` 0x...: label `"+" + int(levelOffset || 1) + " " + getName("ability")`; `LobbyMenu`
    /// sets `levelOffset` = number of levels the reward list passed, default 1): a level-up message grants
    /// "+N ability points" for N levels gained. LIKELY (no start bonus found in the client or in game.options): a new
    /// account at level 1 has 0, so the total is `level - 1`.
    pub fn skill_points_for_level(level: u32) -> u32 {
        level.saturating_sub(1).min(255)
    }

    /// Reward for reaching `level` (level 2 uses entry 0): (amount, currency 0 = silver).
    pub fn reward_for_level(&self, level: u32) -> Option<(u32, u32)> {
        level.checked_sub(2).and_then(|i| self.rewards.get(i as usize)).copied()
    }
}

/// `gameplay/reputation_leveling.options`: points at which each faction level starts (same for factions 1..4).
/// Two zero entries: points 0 already is level 1.
pub const FACTION_THRESHOLDS: [u32; 16] = [
    0, 0, 5000, 20_000, 80_000, 200_000, 350_000, 500_000, 650_000, 800_000, 950_000, 1_100_000, 1_250_000,
    1_400_000, 1_550_000, 1_700_000,
];

/// Faction levels above this one are the "Prestige" ranks (6..=15, 10 per faction).
pub const PRESTIGE_FROM_LEVEL: u32 = 5;

/// Skill points of the Prestige ranks reached: one per faction level above [`PRESTIGE_FROM_LEVEL`], summed over the factions
/// (OFFICIAL 0.56, 2018-12-20: "Each Prestige rank rewards your character with a skill point"; at most 40). Added to the
/// character level's points ([`Levels::skill_points_for_level`]). Faction levels 2..5 give none (no source says they do).
pub fn prestige_skill_points(points: impl IntoIterator<Item = u32>) -> u32 {
    points.into_iter().map(|p| faction_level(p).saturating_sub(PRESTIGE_FROM_LEVEL)).sum()
}

/// Level the client shows for a faction: index of the last threshold <= points (1..=15).
pub fn faction_level(points: u32) -> u32 {
    let mut level = 0;
    for (i, t) in FACTION_THRESHOLDS.iter().enumerate() {
        if *t <= points {
            level = i as u32;
        } else {
            break;
        }
    }
    level
}

/// Highest level that unlocks items: levels above 5 are shown as prestige (level - 5).
pub fn faction_label(points: u32) -> (u32, u32) {
    let l = faction_level(points);
    (l.min(5), l.saturating_sub(5))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prestige_ranks_give_one_point_each() {
        // level 5 (200 000) gives nothing, level 6 (350 000) one, level 15 (1 700 000) ten; summed over the factions
        assert_eq!(prestige_skill_points([0, 200_000, 199_999, 5_000]), 0);
        assert_eq!(prestige_skill_points([350_000]), 1);
        assert_eq!(prestige_skill_points([1_700_000, 1_700_000, 1_700_000, 1_700_000]), 40);
        assert_eq!(prestige_skill_points([500_000, 0, 800_000, 0]), 2 + 4);
    }
}
