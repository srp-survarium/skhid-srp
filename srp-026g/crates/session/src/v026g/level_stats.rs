//! What a player counts for when the sides of a match are made: its K/D
//! and its rating at the match's equipment level (the owner's of
//! 2026-11-01: "k/d equalizer + squads have their kd multiplied by 1.3",
//! "kd should take into account playing on the current match level", "we
//! need also some rating system"; `docs/notes/kd-balance.md`).
//!
//! - **Kept per account and equipment level** ([`LevelStats`]): the kills,
//!   deaths and rating of its rated matches (`rewards::settle`), each at
//!   that match's level, the highest gear level of its players
//!   (`rewards::Results::raise_match_gear`).
//! - **Its strength at a level** ([`LevelStats::strength_at`]):
//!   `kd + RATING_KD * trust * rating`, never below 0, where
//!   - `kd` is the level's K/D damped toward the overall one:
//!     `(kills + PRIOR_DEATHS * overall) / (deaths + PRIOR_DEATHS)`, the
//!     overall being every level's kills and deaths damped so toward
//!     [`DEFAULT_KD`] (no deaths is never infinite, a new account is 1);
//!   - `rating` is the level's rating in points (`super::balance::changes`:
//!     the Elo-like update of the team's result against what was
//!     expected), `trust = matches / (matches + TRUST_MATCHES)`.
//!
//!   The K/D carries the fighting, the rating the winning (batteries,
//!   points, playing for the team), which kills alone do not show.
//! - **A squad's member** counts [`SQUAD_FACTOR`] times its strength
//!   ([`in_squad`]).
//! - **The server's bots** count [`BOT_STRENGTH`].

use super::balance::{Balance, PLAYER, UNIT};
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

/// The matches at a level after which its rating counts for half: a new
/// player's rating is provisional, and counts for nothing before its first
/// rated match there.
pub const TRUST_MATCHES: u32 = 5;

/// What a point of rating counts for, in K/Ds: a player more is
/// [`PLAYER`] (2.2) points of the rating's odds, and a player is one K/D
/// (`balance::Weights::player`), so a point is 1 / 2.2 of a player.
pub const RATING_KD: f64 = 1.0 / PLAYER;

/// What a squad's member counts for, times its strength: 13 / 10 (the
/// owner's 1.3).
pub const SQUAD_FACTOR: (i64, i64) = (13, 10);

/// A member of a squad that plays together: its strength times
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
        let weighed = Strength::from_milli(Strength::UNIT).times(self.kills as usize)
            + prior.times(PRIOR_DEATHS as usize);
        weighed
            .ratio(1, i64::from(self.deaths) + i64::from(PRIOR_DEATHS))
            .max(Strength::ZERO)
    }
}

/// What an account has of one equipment level.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AtLevel {
    pub kd: KillsDeaths,
    /// The level's rating and its rated matches.
    pub rating: Balance,
}

impl AtLevel {
    fn is_empty(self) -> bool {
        self == Self::default()
    }

    /// How far its rating is trusted, 0 to 1.
    fn trust(self) -> f64 {
        let matches = f64::from(self.rating.matches);
        matches / (matches + f64::from(TRUST_MATCHES))
    }
}

/// An account's kills, deaths and ratings by the equipment level of the
/// matches they were made in: one row a level, by level, no empty row.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LevelStats {
    levels: Vec<(u8, AtLevel)>,
}

impl LevelStats {
    /// The stats of these rows: a level named twice keeps the last, an
    /// empty row is none.
    pub fn from_rows(rows: impl IntoIterator<Item = (u8, AtLevel)>) -> Self {
        let mut stats = Self::default();
        for (level, row) in rows {
            stats.levels.retain(|(each, _)| *each != level);
            if !row.is_empty() {
                stats.levels.push((level, row));
            }
        }
        stats.levels.sort_unstable_by_key(|(each, _)| *each);
        stats
    }

    /// Its rows, by level.
    pub fn rows(&self) -> &[(u8, AtLevel)] {
        &self.levels
    }

    /// What it has of equipment level `level`.
    pub fn at(&self, level: u8) -> AtLevel {
        self.levels
            .iter()
            .find(|(each, _)| *each == level)
            .map_or_else(AtLevel::default, |(_, row)| *row)
    }

    /// The kills and deaths of every level.
    pub fn overall(&self) -> KillsDeaths {
        self.levels
            .iter()
            .fold(KillsDeaths::default(), |sum, (_, row)| sum.plus(row.kd))
    }

    /// A rated match of equipment level `level` was played so: these kills
    /// and deaths, and its rating there moved by `change` (thousandths,
    /// `balance::changes_by_share`).
    pub fn played(&mut self, level: u8, kd: KillsDeaths, change: i32) {
        let mut row = self.at(level);
        row.kd = row.kd.plus(kd);
        row.rating = row.rating.moved(change);
        match self.levels.iter_mut().find(|(each, _)| *each == level) {
            Some((_, kept)) => *kept = row,
            None => {
                self.levels.push((level, row));
                self.levels.sort_unstable_by_key(|(each, _)| *each);
            }
        }
    }

    /// The K/D that counts at equipment level `level`: the level's, damped
    /// toward the overall one, itself damped toward [`DEFAULT_KD`].
    pub fn kd_at(&self, level: u8) -> Strength {
        let overall = self.overall().damped(DEFAULT_KD);
        self.at(level).kd.damped(overall)
    }

    /// What the account counts for in a match of equipment level `level`
    /// (the module's formula): its K/D there and its rating there as far
    /// as that is trusted, never below 0.
    pub fn strength_at(&self, level: u8) -> Strength {
        let row = self.at(level);
        let points = f64::from(row.rating.rating) / f64::from(UNIT);
        let rated = Strength::of_kd(RATING_KD * row.trust() * points);
        (self.kd_at(level) + rated).max(Strength::ZERO)
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
            stats.played(level, kd(kills, deaths), 0);
        }
        stats
    }

    #[test]
    fn a_new_account_counts_one() {
        assert_eq!(LevelStats::default().strength_at(0), DEFAULT_KD);
        assert_eq!(
            LevelStats::default().strength_at(15),
            Strength::from_milli(1000)
        );
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
        assert_eq!(stats.strength_at(5), Strength::ZERO);
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
    fn the_rating_counts_as_far_as_its_matches_trust_it() {
        // Two points over five matches at level 4: half of 2 / 2.2.
        let mut stats = LevelStats::default();
        for _ in 0..5 {
            stats.played(4, kd(0, 0), 400);
        }
        assert_eq!(
            stats.at(4).rating,
            Balance {
                rating: 2000,
                matches: 5
            }
        );
        assert_eq!(stats.kd_at(4), DEFAULT_KD);
        assert_eq!(stats.strength_at(4), Strength::from_milli(1000 + 455));
        // Not at another level.
        assert_eq!(stats.strength_at(5), DEFAULT_KD);
        // A low rating takes off, never below 0.
        let mut stats = LevelStats::default();
        for _ in 0..50 {
            stats.played(4, kd(0, 1), -1000);
        }
        assert_eq!(stats.strength_at(4), Strength::ZERO);
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
        let mut stats = LevelStats::default();
        stats.played(7, kd(3, 2), 100);
        stats.played(2, kd(1, 0), -50);
        stats.played(7, kd(4, 5), 25);
        let seven = AtLevel {
            kd: kd(7, 7),
            rating: Balance {
                rating: 125,
                matches: 2,
            },
        };
        let two = AtLevel {
            kd: kd(1, 0),
            rating: Balance {
                rating: -50,
                matches: 1,
            },
        };
        assert_eq!(stats.rows(), [(2, two), (7, seven)]);
        assert_eq!(stats.at(9), AtLevel::default());
        assert_eq!(
            LevelStats::from_rows([(7, seven), (9, AtLevel::default()), (2, two)]),
            stats
        );
    }
}
