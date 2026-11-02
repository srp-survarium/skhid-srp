//! What a player counts for when the sides of a match are made: its K/D
//! at the match's equipment level (the owner's of 2026-11-01: "k/d
//! equalizer + squads have their kd multiplied by 1.3", "kd should take
//! into account playing on the current match level"; a rating, "we don't
//! need that", "simpler the better"; `docs/notes/kd-balance.md`).
//!
//! - **Kept per account and equipment level** ([`LevelStats`]): the kills
//!   and deaths of its rated matches (`rewards::settle`), each at that
//!   match's level, the highest gear level of its players
//!   (`rewards::Results::raise_match_gear`).
//! - **Its K/D at a level** ([`LevelStats::kd_at`]): the level's damped
//!   toward the overall one, `(kills + PRIOR_DEATHS * overall) / (deaths +
//!   PRIOR_DEATHS)`, the overall being every level's kills and deaths
//!   damped so toward [`DEFAULT_KD`] (no deaths is never infinite, a new
//!   account is 1).
//! - **A squad's member** counts [`SQUAD_FACTOR`] times its K/D
//!   ([`in_squad`]).
//! - **The server's bots** count [`BOT_STRENGTH`].

use crate::Strength;

/// What a level of no deaths, or an account of none, leans on: one kill a
/// death.
pub const DEFAULT_KD: Strength = Strength::from_milli(Strength::UNIT);

/// What one of the server's own bots counts for: an average player.
pub const BOT_STRENGTH: Strength = DEFAULT_KD;

/// The deaths at the next K/D up (the overall, then [`DEFAULT_KD`]) that a
/// K/D is counted with: 3 kills and no death are (3 + 10) / (0 + 10) = 1.3,
/// 30 kills and 10 deaths (30 + 10) / (10 + 10) = 2.
pub const PRIOR_DEATHS: u32 = 10;

/// What a squad's member counts for, times its K/D: 13 / 10 (the owner's
/// 1.3).
pub const SQUAD_FACTOR: (i64, i64) = (13, 10);

/// A member of a squad that plays together: its K/D times
/// [`SQUAD_FACTOR`].
pub fn in_squad(strength: Strength) -> Strength {
    strength.ratio(SQUAD_FACTOR.0, SQUAD_FACTOR.1)
}

/// Kills and deaths counted.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KillsDeaths {
    pub kills: u32,
    pub deaths: u32,
}

impl KillsDeaths {
    fn plus(self, other: Self) -> Self {
        Self {
            kills: self.kills.saturating_add(other.kills),
            deaths: self.deaths.saturating_add(other.deaths),
        }
    }

    /// The K/D of these with [`PRIOR_DEATHS`] deaths more at `prior`.
    fn damped(self, prior: Strength) -> Strength {
        let weighed = DEFAULT_KD.times(self.kills as usize) + prior.times(PRIOR_DEATHS as usize);
        weighed.ratio(1, i64::from(self.deaths) + i64::from(PRIOR_DEATHS))
    }
}

/// An account's kills and deaths by the equipment level of the matches
/// they were made in: one row a level, by level, no empty row.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LevelStats {
    levels: Vec<(u8, KillsDeaths)>,
}

impl LevelStats {
    /// The stats of these rows: a level named twice keeps the last, an
    /// empty row is none.
    pub fn from_rows(rows: impl IntoIterator<Item = (u8, KillsDeaths)>) -> Self {
        let mut stats = Self::default();
        for (level, row) in rows {
            stats.levels.retain(|(each, _)| *each != level);
            if row != KillsDeaths::default() {
                stats.levels.push((level, row));
            }
        }
        stats.levels.sort_unstable_by_key(|(each, _)| *each);
        stats
    }

    /// Its rows, by level.
    pub fn rows(&self) -> &[(u8, KillsDeaths)] {
        &self.levels
    }

    /// The kills and deaths at equipment level `level`.
    pub fn at(&self, level: u8) -> KillsDeaths {
        self.levels
            .iter()
            .find(|(each, _)| *each == level)
            .map_or_else(KillsDeaths::default, |(_, row)| *row)
    }

    /// The kills and deaths of every level.
    pub fn overall(&self) -> KillsDeaths {
        self.levels
            .iter()
            .fold(KillsDeaths::default(), |sum, (_, row)| sum.plus(*row))
    }

    /// A rated match of equipment level `level` was played with these
    /// kills and deaths.
    pub fn played(&mut self, level: u8, kd: KillsDeaths) {
        if kd == KillsDeaths::default() {
            return;
        }
        match self.levels.iter_mut().find(|(each, _)| *each == level) {
            Some((_, kept)) => *kept = kept.plus(kd),
            None => {
                self.levels.push((level, kd));
                self.levels.sort_unstable_by_key(|(each, _)| *each);
            }
        }
    }

    /// What the account counts for in a match of equipment level `level`:
    /// the level's K/D, damped toward the overall one, itself damped
    /// toward [`DEFAULT_KD`]. Never below 0.
    pub fn kd_at(&self, level: u8) -> Strength {
        let overall = self.overall().damped(DEFAULT_KD);
        self.at(level).damped(overall)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kd(kills: u32, deaths: u32) -> KillsDeaths {
        KillsDeaths { kills, deaths }
    }

    fn played(rows: &[(u8, u32, u32)]) -> LevelStats {
        let mut stats = LevelStats::default();
        for &(level, kills, deaths) in rows {
            stats.played(level, kd(kills, deaths));
        }
        stats
    }

    #[test]
    fn a_new_account_counts_one() {
        assert_eq!(LevelStats::default().kd_at(0), DEFAULT_KD);
        assert_eq!(LevelStats::default().kd_at(15), Strength::from_milli(1000));
    }

    #[test]
    fn few_deaths_never_make_it_infinite() {
        // 3 kills, no death: 13 / 10 overall.
        let stats = played(&[(5, 3, 0)]);
        // The level's own: (3 + 10 * 1.3) / 10.
        assert_eq!(stats.kd_at(5), Strength::from_milli(1600));
        // Another level leans on the overall alone.
        assert_eq!(stats.kd_at(9), Strength::from_milli(1300));
        // A great many kills and no death: large, still a number.
        let stats = played(&[(5, u32::MAX, 0)]);
        assert!(stats.kd_at(5) > Strength::from_milli(1_000_000));
        // Deaths and no kill: down toward 0, never below.
        let stats = played(&[(5, 0, u32::MAX)]);
        assert_eq!(stats.kd_at(5), Strength::ZERO);
    }

    #[test]
    fn many_deaths_are_the_levels_own_k_d() {
        // 400 kills, 200 deaths at level 7: about 2.
        let stats = played(&[(7, 400, 200), (3, 10, 40)]);
        let at_seven = stats.kd_at(7).milli();
        assert!((1900..=2000).contains(&at_seven), "{at_seven}");
        // At level 3, 10 / 40 leans toward the overall (410 / 240).
        let at_three = stats.kd_at(3).milli();
        assert!(at_three > 250 && at_three < 600, "{at_three}");
        assert_eq!(stats.overall(), kd(410, 240));
    }

    #[test]
    fn a_squads_member_counts_one_and_three_tenths() {
        assert_eq!(in_squad(DEFAULT_KD), Strength::from_milli(1300));
        assert_eq!(
            in_squad(Strength::from_milli(2000)),
            Strength::from_milli(2600)
        );
        assert_eq!(in_squad(Strength::ZERO), Strength::ZERO);
    }

    #[test]
    fn playing_adds_by_level_and_rows_read_back() {
        let stats = played(&[(7, 3, 2), (2, 1, 0), (7, 4, 5), (9, 0, 0)]);
        assert_eq!(stats.rows(), [(2, kd(1, 0)), (7, kd(7, 7))]);
        assert_eq!(stats.at(9), KillsDeaths::default());
        assert_eq!(
            LevelStats::from_rows([(7, kd(7, 7)), (9, kd(0, 0)), (2, kd(1, 0))]),
            stats
        );
    }
}
