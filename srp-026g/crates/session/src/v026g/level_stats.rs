//! What a player counts for when the sides of a match are made: its K/D
//! at the match's equipment level times its kills a minute there against
//! the server's average (the owner's of 2026-11-01: "k/d equalizer + squads
//! have their kd multiplied by 1.3", "kd should take into account playing
//! on the current match level"; of 2026-11-05: the kills a minute count
//! too, so a player who goes 6/1 sitting on a base counts for less than
//! one who goes 60/15; `docs/notes/kd-balance.md`).
//!
//! - **Kept per account and equipment level** ([`LevelStats`]): the kills
//!   and deaths of its rated matches (`rewards::settle`), each at that
//!   match's level, the highest gear level of its players
//!   (`rewards::Results::raise_match_gear`); and since schema step 14 the
//!   kills again with the seconds they took ([`TimedKills`]: kills of
//!   before have no time, so the rate counts only these).
//! - **Its K/D at a level** ([`LevelStats::kd_at`]): the level's damped
//!   toward the overall one, `(kills + PRIOR_DEATHS * overall) / (deaths +
//!   PRIOR_DEATHS)`, the overall being every level's kills and deaths
//!   damped so toward [`DEFAULT_KD`] (no deaths is never infinite, a new
//!   account is 1).
//! - **Its kills a minute at a level** ([`LevelStats::kill_rate_at`]): damped
//!   the same way by [`PRIOR_SECONDS`], toward the overall rate, itself
//!   toward [`AVERAGE_KILL_RATE`].
//! - **Its strength** ([`LevelStats::strength_at`]): the K/D times the rate
//!   over [`AVERAGE_KILL_RATE`], the rate counted at most
//!   [`KILL_RATE_MOST`].
//! - **A squad's member** counts [`SQUAD_FACTOR`] times its strength
//!   ([`in_squad`]).
//! - **The server's bots** count [`BOT_STRENGTH`].
//! - **Strong** ([`STRONG`]): the strongest two players are kept apart, and
//!   so is a squad with a strong player from every other strong one
//!   ([`rivals`]); a squad of two strong players plays no match
//!   ([`too_strong`]).

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

/// The server's kills a minute in matches against players: 77,190 kills in
/// 93,319 minutes of the 2,080 match reports kept from 2026-10-16 to
/// 2026-11-05 (`scripts/balance-refit --db`), 0.827.
pub const AVERAGE_KILL_RATE: KillRate = KillRate::from_milli(830);

/// The seconds at the next rate up (the overall, then
/// [`AVERAGE_KILL_RATE`]) that a rate is counted with: one match of 15
/// minutes. 6 kills in a match of 15 minutes are (6 + 15 * 0.83) / 30 =
/// 0.62 a minute, 60 kills 2.4. The back-test of 2026-11-05 is flat from 5
/// to 60 minutes (log loss 0.6762 to 0.6768).
pub const PRIOR_SECONDS: u32 = 15 * 60;

/// The most a rate counts for: 1.5 times the average. A player who kills
/// fast counts up to half as much more; one who kills slowly, down toward
/// none. Unbounded, a few players counted for ten and more and the sides
/// came out worse than by the K/D alone (back-test of 2026-11-05).
pub const KILL_RATE_MOST: KillRate = KillRate::from_milli(1245);

/// What a squad's member counts for, times its strength: 13 / 10 (the
/// owner's 1.3).
pub const SQUAD_FACTOR: (i64, i64) = (13, 10);

/// A strong player: 2.5 times an average one. Of the players of the
/// matches of 2026-11-01 to 05, 8 % were strong in a match or more; every
/// player the owner named as strong is (the ARTS clan's, Dr1t, Alex Stan,
/// Топотунчик at their levels: 2.6 to 7.4).
pub const STRONG: Strength = Strength::from_milli(2500);

/// A member of a squad that plays together: its strength times
/// [`SQUAD_FACTOR`].
pub fn in_squad(strength: Strength) -> Strength {
    strength.ratio(SQUAD_FACTOR.0, SQUAD_FACTOR.1)
}

/// Kills a minute, in thousandths.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct KillRate(i64);

impl KillRate {
    pub const fn from_milli(milli: i64) -> Self {
        Self(milli)
    }

    pub const fn milli(self) -> i64 {
        self.0
    }

    /// In kills a minute, for the log.
    pub fn per_minute(self) -> f64 {
        self.0 as f64 / 1000.0
    }
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

/// Kills counted with the time they took: a player's seconds in its rated
/// matches and its kills in them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TimedKills {
    pub kills: u32,
    pub seconds: u32,
}

impl TimedKills {
    fn plus(self, other: Self) -> Self {
        Self {
            kills: self.kills.saturating_add(other.kills),
            seconds: self.seconds.saturating_add(other.seconds),
        }
    }

    /// The kills a minute of these with [`PRIOR_SECONDS`] more at `prior`.
    fn damped(self, prior: KillRate) -> KillRate {
        let weighed = i64::from(self.kills) * 60_000 + i64::from(PRIOR_SECONDS) * prior.0;
        KillRate(weighed / (i64::from(self.seconds) + i64::from(PRIOR_SECONDS)))
    }
}

/// What an account played at one equipment level.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LevelRow {
    pub kd: KillsDeaths,
    pub timed: TimedKills,
}

impl LevelRow {
    fn plus(self, other: Self) -> Self {
        Self {
            kd: self.kd.plus(other.kd),
            timed: self.timed.plus(other.timed),
        }
    }
}

/// A row of before the time was kept: its kills have no time.
impl From<KillsDeaths> for LevelRow {
    fn from(kd: KillsDeaths) -> Self {
        Self {
            kd,
            timed: TimedKills::default(),
        }
    }
}

/// An account's play by the equipment level of the matches it was in:
/// one row a level, by level, no empty row.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LevelStats {
    levels: Vec<(u8, LevelRow)>,
}

impl LevelStats {
    /// The stats of these rows: a level named twice keeps the last, an
    /// empty row is none.
    pub fn from_rows<R: Into<LevelRow>>(rows: impl IntoIterator<Item = (u8, R)>) -> Self {
        let mut stats = Self::default();
        for (level, row) in rows {
            let row = row.into();
            stats.levels.retain(|(each, _)| *each != level);
            if row != LevelRow::default() {
                stats.levels.push((level, row));
            }
        }
        stats.levels.sort_unstable_by_key(|(each, _)| *each);
        stats
    }

    /// Its rows, by level.
    pub fn rows(&self) -> &[(u8, LevelRow)] {
        &self.levels
    }

    fn row(&self, level: u8) -> LevelRow {
        self.levels
            .iter()
            .find(|(each, _)| *each == level)
            .map_or_else(LevelRow::default, |(_, row)| *row)
    }

    fn overall(&self) -> LevelRow {
        self.levels
            .iter()
            .fold(LevelRow::default(), |sum, (_, row)| sum.plus(*row))
    }

    /// The kills and deaths at equipment level `level`.
    pub fn at(&self, level: u8) -> KillsDeaths {
        self.row(level).kd
    }

    /// The kills with their time at equipment level `level`.
    pub fn timed_at(&self, level: u8) -> TimedKills {
        self.row(level).timed
    }

    /// A rated match of equipment level `level` was played with these
    /// kills and deaths, for `seconds` of it.
    pub fn played(&mut self, level: u8, kd: KillsDeaths, seconds: u32) {
        let row = LevelRow {
            kd,
            timed: TimedKills {
                kills: kd.kills,
                seconds,
            },
        };
        if row == LevelRow::default() {
            return;
        }
        match self.levels.iter_mut().find(|(each, _)| *each == level) {
            Some((_, kept)) => *kept = kept.plus(row),
            None => {
                self.levels.push((level, row));
                self.levels.sort_unstable_by_key(|(each, _)| *each);
            }
        }
    }

    /// Its K/D at equipment level `level`: the level's, damped toward the
    /// overall one, itself damped toward [`DEFAULT_KD`]. Never below 0.
    pub fn kd_at(&self, level: u8) -> Strength {
        let overall = self.overall().kd.damped(DEFAULT_KD);
        self.at(level).damped(overall)
    }

    /// Its kills a minute at equipment level `level`: the level's, damped
    /// toward the overall rate, itself damped toward
    /// [`AVERAGE_KILL_RATE`]. A new account's is the average.
    pub fn kill_rate_at(&self, level: u8) -> KillRate {
        let overall = self.overall().timed.damped(AVERAGE_KILL_RATE);
        self.timed_at(level).damped(overall)
    }

    /// What the account counts for in a match of equipment level `level`:
    /// its K/D there times its kills a minute there over
    /// [`AVERAGE_KILL_RATE`], the rate at most [`KILL_RATE_MOST`]. A new
    /// account is 1.
    pub fn strength_at(&self, level: u8) -> Strength {
        let rate = self.kill_rate_at(level).min(KILL_RATE_MOST);
        self.kd_at(level).ratio(rate.0, AVERAGE_KILL_RATE.0)
    }
}

/// The parties (`P`, one for each player of a squad, one of its own for
/// each player alone) that are never put on one side, of these players
/// with their strengths (beside any squad's factor):
///
/// - **the two strongest**, unless they are of one party (a squad is the
///   players' own choice) or the second is tied with a third (no two are
///   the strongest then);
/// - **a squad with a strong player** ([`STRONG`]) and every other party
///   with one.
///
/// Each pair once: the stronger of the two strongest first, a squad before
/// the party it is kept from.
pub fn rivals<P: Copy + Eq>(players: &[(P, Strength)]) -> Vec<(P, P)> {
    let mut order: Vec<(P, Strength)> = players.to_vec();
    order.sort_by_key(|&(_, strength)| std::cmp::Reverse(strength));
    let mut pairs: Vec<(P, P)> = Vec::new();
    let mut add = |a: P, b: P| {
        if a != b
            && !pairs
                .iter()
                .any(|&(x, y)| (x, y) == (a, b) || (x, y) == (b, a))
        {
            pairs.push((a, b));
        }
    };
    if let [first, second, rest @ ..] = order.as_slice()
        && rest.first().is_none_or(|third| second.1 > third.1)
    {
        add(first.0, second.0);
    }
    let in_squad = |party: P| players.iter().filter(|(each, _)| *each == party).count() > 1;
    let strong: Vec<P> = order
        .iter()
        .filter(|(_, strength)| *strength >= STRONG)
        .map(|(party, _)| *party)
        .collect();
    for &squad in strong.iter().filter(|&&party| in_squad(party)) {
        for &other in &strong {
            add(squad, other);
        }
    }
    pairs
}

/// A squad too strong to play (the owner's of 2026-11-05: "a squad that is
/// too strong can't start a game at all"): two or more of its members
/// strong ([`STRONG`]), each by its own strength.
pub fn too_strong(members: &[Strength]) -> bool {
    members.len() > 1 && members.iter().filter(|&&each| each >= STRONG).count() > 1
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

    fn milli(milli: i64) -> Strength {
        Strength::from_milli(milli)
    }

    #[test]
    fn a_new_account_counts_one() {
        assert_eq!(LevelStats::default().kd_at(0), DEFAULT_KD);
        assert_eq!(LevelStats::default().kill_rate_at(15), AVERAGE_KILL_RATE);
        assert_eq!(LevelStats::default().strength_at(15), milli(1000));
    }

    #[test]
    fn few_deaths_never_make_it_infinite() {
        // 3 kills, no death: 13 / 10 overall.
        let stats = played(&[(5, 3, 0)]);
        // The level's own: (3 + 10 * 1.3) / 10.
        assert_eq!(stats.kd_at(5), milli(1600));
        // Another level leans on the overall alone.
        assert_eq!(stats.kd_at(9), milli(1300));
        // A great many kills and no death: large, still a number.
        let stats = played(&[(5, u32::MAX, 0)]);
        assert!(stats.kd_at(5) > milli(1_000_000));
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
    }

    /// The owner's of 2026-11-05: a player who goes 6/1 sitting on a base
    /// counts for less than one who goes 60/15, in the same 15 minutes.
    #[test]
    fn the_kills_a_minute_count_too() {
        let mut camper = LevelStats::default();
        camper.played(10, kd(6, 1), 15 * 60);
        let mut fighter = LevelStats::default();
        fighter.played(10, kd(60, 15), 15 * 60);
        // 6 kills in 15 minutes: (6 + 15 * 0.62) / 30 a minute at the
        // level, its overall (6 + 15 * 0.83) / 30 = 0.615.
        assert_eq!(camper.kill_rate_at(10), KillRate::from_milli(507));
        assert!(camper.strength_at(10) < camper.kd_at(10));
        // 60 kills: past the most a rate counts for.
        assert!(fighter.kill_rate_at(10) > KILL_RATE_MOST);
        assert_eq!(
            fighter.strength_at(10),
            fighter
                .kd_at(10)
                .ratio(KILL_RATE_MOST.milli(), AVERAGE_KILL_RATE.milli())
        );
        assert!(fighter.strength_at(10) > camper.strength_at(10));
        // Kills of before the time was kept count in the K/D, not the rate.
        let old = LevelStats::from_rows([(10, kd(60, 15))]);
        assert_eq!(old.kill_rate_at(10), AVERAGE_KILL_RATE);
        assert_eq!(old.strength_at(10), old.kd_at(10));
    }

    #[test]
    fn a_squads_member_counts_one_and_three_tenths() {
        assert_eq!(in_squad(DEFAULT_KD), milli(1300));
        assert_eq!(in_squad(milli(2000)), milli(2600));
        assert_eq!(in_squad(Strength::ZERO), Strength::ZERO);
    }

    #[test]
    fn playing_adds_by_level_and_rows_read_back() {
        let mut stats = played(&[(7, 3, 2), (2, 1, 0), (7, 4, 5), (9, 0, 0)]);
        stats.played(7, kd(0, 0), 60);
        assert_eq!(
            stats.rows(),
            [
                (
                    2,
                    LevelRow {
                        kd: kd(1, 0),
                        timed: TimedKills {
                            kills: 1,
                            seconds: 0
                        }
                    }
                ),
                (
                    7,
                    LevelRow {
                        kd: kd(7, 7),
                        timed: TimedKills {
                            kills: 7,
                            seconds: 60
                        }
                    }
                )
            ]
        );
        assert_eq!(stats.at(9), KillsDeaths::default());
        assert_eq!(LevelStats::from_rows(stats.rows().iter().copied()), stats);
    }

    #[test]
    fn the_two_strongest_and_strong_squads_are_rivals() {
        // Alone: the two strongest, whatever else.
        let players = [
            (1, milli(900)),
            (2, milli(3000)),
            (3, milli(1200)),
            (4, milli(2000)),
        ];
        assert_eq!(rivals(&players), [(2, 4)]);
        // A tie for the second: no two are the strongest.
        let tied = [(1, milli(3000)), (2, milli(1000)), (3, milli(1000))];
        assert!(rivals(&tied).is_empty());
        assert!(rivals(&[(1, milli(1000))]).is_empty());
        assert_eq!(rivals(&[(1, milli(1000)), (2, milli(1000))]), [(1, 2)]);
        // The two strongest in one squad: they stay together, and the
        // squad, strong, has every other strong party against it.
        let squad = [
            (7, milli(4000)),
            (7, milli(3000)),
            (1, milli(2600)),
            (2, milli(2500)),
            (3, milli(900)),
        ];
        assert_eq!(rivals(&squad), [(7, 1), (7, 2)]);
        // Two strong alone are no rivals but as the two strongest.
        let alone = [
            (1, milli(4000)),
            (2, milli(3000)),
            (3, milli(2600)),
            (5, milli(500)),
        ];
        assert_eq!(rivals(&alone), [(1, 2)]);
        // A squad of no strong player has none.
        let weak = [
            (7, milli(2000)),
            (7, milli(2000)),
            (1, milli(3000)),
            (2, milli(2600)),
        ];
        assert_eq!(rivals(&weak), [(1, 2)]);
    }

    #[test]
    fn a_squad_of_two_strong_is_too_strong() {
        assert!(too_strong(&[milli(2500), milli(9000)]));
        assert!(!too_strong(&[milli(2499), milli(9000)]));
        assert!(!too_strong(&[milli(9000)]));
        assert!(too_strong(&[milli(100), milli(3000), milli(3000)]));
    }
}
