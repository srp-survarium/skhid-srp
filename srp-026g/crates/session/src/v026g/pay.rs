//! What a 0.26g match pays, in numbers: the silver of a minute played,
//! the three rows of the results screen, the rates, the level rewards, the
//! rating a new account starts with and the drop chance's bonuses
//! (`docs/notes/progression_sources.md` §2, §7 and `results.md` §10,
//! §14). [`super::rewards`] applies them.
//!
//! The client holds none of these numbers. The rows' experience and
//! reputation are fitted to two results screens of the time:
//!
//! - **SHOT1** (0.26, match level 1: `money_k` 1.47, the others 1; a win
//!   5:4, score 223, 17 kills, 4 helps): kills 551 / 41 / 17, help
//!   58 / 4 / 4, activity 463 / 143 / 5 (money / experience / reputation).
//! - **SHOT2** (LIKELY 0.27, match level 15: `money_k` 10.17, the others
//!   1.7; a win 8:2, score 289, 19 kills, 8 helps): kills 4830 / 96 / 32,
//!   help 813 / 13 / 13, activity 5064 / 270 / 11; premium 5353 / 189 / 28.
//!
//! Each cell of a row is its plain amount times the match level's
//! multiplier (`match_levels_dict`), rounded down after multiplying
//! (PROVEN by SHOT2: every cell of its kills and help rows comes out).
//! Their silver was by the rows too (kills 25 and helps 10 a piece, the
//! activity row a fixed part and about 7 a point of score: `results.md`
//! §10); srp's is by the minute played ([`SILVER_PER_MINUTE`], the owner's
//! of 2026-11-05).

/// What one event, or one point of score, pays before the match level's
/// multipliers: experience, reputation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pay {
    pub experience: f64,
    pub reputation: f64,
}

impl Pay {
    pub const NONE: Self = Self::new(0.0, 0.0);

    pub const fn new(experience: f64, reputation: f64) -> Self {
        Self {
            experience,
            reputation,
        }
    }

    /// `self × factor`.
    pub fn times(self, factor: f64) -> Self {
        Self::new(self.experience * factor, self.reputation * factor)
    }

    pub fn plus(self, other: Self) -> Self {
        Self::new(
            self.experience + other.experience,
            self.reputation + other.reputation,
        )
    }
}

/// "Enemies killed": a kill (PROVEN by SHOT2: 19 kills are 57 / 19 before
/// the multipliers).
pub const KILL: Pay = Pay::new(3.0, 1.0);
/// A lesser kind of kill, cause unknown: SHOT1's 17 kills (41 / 17 before
/// the multipliers) are exactly 7 kills of [`KILL`] and 10 of this. Not
/// used: every kill pays [`KILL`] until the cause is known.
#[allow(dead_code)]
pub const LESSER_KILL: Pay = Pay::new(2.0, 1.0);
/// "Help ally": an assist (PROVEN by both screens). A cure and a rescue of
/// a teammate are in the same row of the game's table and are paid the
/// same (LIKELY).
pub const HELP: Pay = Pay::new(1.0, 1.0);

// ---------------------------------------------------------------------
// GUESS: "Match Activity Rewards". Both screens are wins, so all that is
// known is the row's sum for a won match played to the end, at two "other
// scores" (the score less what kills and helps scored):
//
//   SHOT1, other score 33: 143 / 5 before the multipliers
//   SHOT2, other score 59: 159 / 6.5 to 7
//
// The row is taken as the line through both: a fixed part and a part for
// each point of other score. How the fixed part splits between finishing
// and winning is not known. The owner's choice (2026-10-12): a match
// played to the end and not won pays half of the fixed part
// ([`ACTIVITY_NOT_WON`]) and the same for each point; a player who left
// before the end gets none of the fixed part. Change these constants when
// a lost match's screen is seen.
// ---------------------------------------------------------------------
/// The other score of the two screens.
const SHOT1_SCORE: f64 = 33.0;
const SHOT2_SCORE: f64 = 59.0;
/// For each point of other score: (159 − 143) / 26 experience, 0.077
/// reputation.
pub const ACTIVITY_PER_SCORE: Pay = Pay::new((159.0 - 143.0) / (SHOT2_SCORE - SHOT1_SCORE), 0.077);
/// For a won match played to the end: about 122.69 experience, 2.46
/// reputation.
pub const ACTIVITY_WON: Pay = Pay::new(143.0 - SHOT1_SCORE * ACTIVITY_PER_SCORE.experience, 2.46);
/// The share of [`ACTIVITY_WON`] a match played to the end and not won
/// pays (the owner's choice).
pub const ACTIVITY_NOT_WON: f64 = 0.5;
// ------------------------------------------------------------- GUESS ends

// ---------------------------------------------------------------------
// The match's silver (the owner's of 2026-11-05, `results.md` §14):
// "silver shouldn't even be based on player score". A player is paid for
// its time in the match, the same in every mode, by its team's result and
// by its rank among those who stayed to the end ("it should be
// progressive"); the score only ranks. These are before the match level's
// multiplier and the rate, as a row's cell is.
//
// The bounds ("just with proper bounds"): a minute pays at least
// SILVER_PER_MINUTE × NOT_WON_PERMILLE (59.5: not won, no rank bonus) and at
// most SILVER_PER_MINUTE × (1000 + BEST_RANK_BONUS_PERMILLE) (91: won, the
// best), per mille.
// ---------------------------------------------------------------------
/// The silver of a minute in the match: the owner's "high players
/// shouldn't lose, weak players should gain" set it so the best of a won
/// match (91 a minute) is paid what the winning top quarter's median was
/// paid by the score (91 on live: 285 matches of 2026-10-29 to 11-05;
/// batteries 91, Research 133, deathmatch 59).
pub const SILVER_PER_MINUTE: u32 = 70;
/// The per mille of it a player is paid that did not stay to the end of a
/// match its team won: a loss, a draw, a match with no known result, or one
/// it left (the owner's "losing team 10-20% less"). The least a minute pays.
pub const NOT_WON_PERMILLE: u32 = 850;
/// The rank bonus of the match's best, per mille on top of its team's (the
/// owner's "top players get +30%"): the most a [`RankBonus`] is.
pub const BEST_RANK_BONUS_PERMILLE: u32 = 300;

/// What a player's rank among those who stayed to the end of a match adds
/// to its silver: 0 to [`BEST_RANK_BONUS_PERMILLE`] per mille, by
/// construction.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RankBonus(u32);

impl RankBonus {
    pub const NONE: Self = Self(0);

    /// The bonus of one who stayed to the end that scored more than
    /// `below` of the match's `finishers` and less than `above` of them:
    /// [`BEST_RANK_BONUS_PERMILLE`] × (`below` − `above`) / (`finishers` −
    /// 1), rounded down, and none at or under the middle. With no ties
    /// that falls in a straight line from the best, the whole bonus, to the
    /// median, none; equal scores share a bonus, and a match whose finishers
    /// all scored alike, or that has one finisher, gives none.
    pub fn of(below: usize, above: usize, finishers: usize) -> Self {
        let Some(others) = finishers.checked_sub(1).filter(|&others| others > 0) else {
            return Self::NONE;
        };
        // `below` is at most `others`: so is the lead.
        let lead = below.saturating_sub(above).min(others);
        let permille = u64::from(BEST_RANK_BONUS_PERMILLE) * lead as u64 / others as u64;
        Self(permille as u32)
    }

    pub fn permille(self) -> u32 {
        self.0
    }
}

/// The silver of `played_ms` in a match, before the match level's
/// multiplier and the rate: [`SILVER_PER_MINUTE`] a minute, times the
/// team's per mille (1000 if `won`, else [`NOT_WON_PERMILLE`]) and 1000 +
/// the rank bonus per mille.
pub fn silver_of(played_ms: u32, won: bool, bonus: RankBonus) -> f64 {
    let team = if won { 1000 } else { NOT_WON_PERMILLE };
    let rank = 1000 + bonus.permille();
    // Under 2^64 for any u32; whole in an f64 under 2^53, a match of less
    // than 27 hours.
    let scaled =
        u64::from(played_ms) * u64::from(SILVER_PER_MINUTE) * u64::from(team) * u64::from(rank);
    scaled as f64 / (60_000.0 * 1000.0 * 1000.0)
}

/// A multiplier of `match_levels_dict` as the decimal the data means: the
/// file's f32 for 1.05 is a hair under it, and 20 × 1.05 must be 21.
pub fn multiplier(k: f32) -> f64 {
    (f64::from(k) * 10_000.0).round() / 10_000.0
}

/// A cell of the screen: `plain × k`, rounded down. The hair added keeps a
/// product that is a whole number in the game's arithmetic from falling
/// under it in ours.
pub fn cell(plain: f64, k: f64) -> f64 {
    (plain * k + 1e-6).floor().max(0.0)
}

/// The owner's choice (2026-10-14): the percent of the match's silver
/// ([`SILVER_PER_MINUTE`]) paid, by the match's level: 150 to level 5, ten more each
/// level after, 220 from level 12. The kits' prices grow faster than
/// `money_k` does (`progression_pace.md`: a tier's kit was 20 to 26
/// matches of silver up to tier 6 and 53 to 72 from tier 12), and past 220
/// the faction's rank is what a player waits for. It was 120 at every level
/// from 2026-10-13 and 80 from the start.
pub const MONEY_RATES: [u32; 13] = [
    150, 150, 150, 150, 150, 150, 160, 170, 180, 190, 200, 210, 220,
];
/// The owner's choice (2026-10-14): a match's experience is paid twice
/// over. The account level was the longest road by far
/// (`progression_pace.md`: level 20 after 67 hours of play, the top kit
/// after 72). It was the original's pace, 1, from 2026-10-12.
pub const DEFAULT_EXPERIENCE_RATE: u32 = 2;
/// The owner's choice (2026-10-14, "increase reputation a bit for all"; the
/// 120 is srp's reading of "a bit"): the percent of the original's
/// reputation a match's rows and its quests pay.
pub const DEFAULT_REPUTATION_RATE: u32 = 120;

/// The percent of the match's silver ([`SILVER_PER_MINUTE`]) a match pays.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoneyRate {
    /// [`MONEY_RATES`], by the match's level.
    ByLevel,
    /// One percent at every level (`SRP_MONEY_RATE`).
    Flat(u32),
}

impl MoneyRate {
    /// The percent a match of `match_level` pays.
    pub fn at(self, match_level: u16) -> u32 {
        match self {
            Self::ByLevel => MONEY_RATES[usize::from(match_level).min(MONEY_RATES.len() - 1)],
            Self::Flat(percent) => percent,
        }
    }
}

fn setting(name: &str, default: u32, most: u32) -> u32 {
    match std::env::var(name) {
        Ok(value) => match value.trim().parse::<u32>() {
            Ok(rate) if (1..=most).contains(&rate) => rate,
            _ => {
                log::error!("{name} is `{value}`; it must be 1 to {most}. Using {default}");
                default
            }
        },
        Err(_) => default,
    }
}

/// `SRP_MONEY_RATE`: one percent of the match's silver ([`SILVER_PER_MINUTE`])
/// at every level, 1 to 1000; unset, [`MONEY_RATES`] by
/// the match's level. Premium's half is counted from it, so it
/// follows. Read once.
pub fn money_rate() -> MoneyRate {
    static RATE: std::sync::OnceLock<MoneyRate> = std::sync::OnceLock::new();
    *RATE.get_or_init(|| match std::env::var("SRP_MONEY_RATE") {
        Ok(value) => match value.trim().parse::<u32>() {
            Ok(rate) if (1..=1000).contains(&rate) => MoneyRate::Flat(rate),
            _ => {
                log::error!(
                    "SRP_MONEY_RATE is `{value}`; it must be 1 to 1000. Using the rates by match level"
                );
                MoneyRate::ByLevel
            }
        },
        Err(_) => MoneyRate::ByLevel,
    })
}

/// `SRP_EXPERIENCE_RATE`: what a match's experience is multiplied by, 1 to
/// 100 (default 2; 1 is the original's pace). The reputation has its own
/// rate ([`reputation_rate`]); until 2026-10-14 this one multiplied it too.
/// Read once.
pub fn experience_rate() -> u32 {
    static RATE: std::sync::OnceLock<u32> = std::sync::OnceLock::new();
    *RATE.get_or_init(|| setting("SRP_EXPERIENCE_RATE", DEFAULT_EXPERIENCE_RATE, 100))
}

/// `SRP_REPUTATION_RATE`: the percent of the original's reputation the
/// rows of a match and the quests completed in it pay, 1 to 1000 (default
/// 120). Read once.
pub fn reputation_rate() -> u32 {
    static RATE: std::sync::OnceLock<u32> = std::sync::OnceLock::new();
    *RATE.get_or_init(|| setting("SRP_REPUTATION_RATE", DEFAULT_REPUTATION_RATE, 1000))
}

/// The rating a new 0.26g account has (LIKELY: SHOT1 shows 118 on an
/// account of level 3, and the data's `elo_modifiers` begin at `elo_min`
/// 100). 0.21d's accounts start at 1000.
pub const START_RATING: u16 = 100;

/// The most a match moves a 0.26g rating, and the rating difference that
/// makes the odds ten to one: 0.21d's 32 and 400 for ratings that start at
/// a tenth of its 1000 (the owner's choice, 2026-10-12; the original rule
/// is not known). Equal teams move by 2. With 0.21d's own 32 a new account
/// was at 0 after seven lost matches.
pub const RATING_STEP: f64 = 4.0;
pub const RATING_SPREAD: f64 = 40.0;

/// What reaching a level pays.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LevelReward {
    Silver(u32),
    Gold(u32),
}

/// The gold of the last level (the wiki's table of February 2015).
pub const LAST_LEVEL_GOLD: u32 = 250;

/// The reward for reaching `level` (counted from 0, as a 0.26g account's
/// is): 0.21d's table, which is the wiki's (100 gold at level 1 and every
/// fifth, silver between: 500 rising by 250 each five levels to 5,000),
/// and 250 gold at the last level, 100.
pub fn level_reward(level: u8) -> LevelReward {
    use survarium::v021d::results::{LevelReward as Old, level_reward};
    if level >= super::account::MAX_LEVEL {
        return LevelReward::Gold(LAST_LEVEL_GOLD);
    }
    match level_reward(level.saturating_add(1)) {
        Old::Money(amount) => LevelReward::Silver(amount),
        Old::Premium(amount) => LevelReward::Gold(amount),
    }
}

/// `SRP_ITEM_WEAR`: whether a match wears the weapons and armour of the
/// kits played (the items' `degrade_per_match` and `degrade_per_use`), and
/// with it whether a worn item may get a defect. Off unless `on`: the
/// owner's choice (2026-10-13), an item a player has stays as it is. The
/// game's own rule is `on`. Read once.
pub fn item_wear() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| crate::switch("SRP_ITEM_WEAR", false))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_activity_line_goes_through_both_screens() {
        let at = |score: f64| ACTIVITY_WON.plus(ACTIVITY_PER_SCORE.times(score));
        let first = at(33.0);
        // SHOT1, level 1: 143 / 5.
        assert_eq!(cell(first.experience, 1.0), 143.0);
        assert_eq!(cell(first.reputation, 1.0), 5.0);
        let second = at(59.0);
        // SHOT2, level 15: 270 / 11.
        assert_eq!(cell(second.experience, multiplier(1.7)), 270.0);
        assert_eq!(cell(second.reputation, multiplier(1.7)), 11.0);
        // The fixed part is what the coordinator's fit names.
        assert!((ACTIVITY_WON.experience - 122.69).abs() < 0.01);
        assert!((ACTIVITY_PER_SCORE.experience - 0.615).abs() < 0.001);
    }

    #[test]
    fn kills_and_help_are_the_screens() {
        let k = multiplier(1.7);
        // SHOT2: 19 kills, 8 helps.
        let kills = KILL.times(19.0);
        assert_eq!(
            (cell(kills.experience, k), cell(kills.reputation, k)),
            (96.0, 32.0)
        );
        let help = HELP.times(8.0);
        assert_eq!(
            (cell(help.experience, k), cell(help.reputation, k)),
            (13.0, 13.0)
        );
        // SHOT1: its kills are 7 full and 10 lesser ones.
        let mixed = KILL.times(7.0).plus(LESSER_KILL.times(10.0));
        assert_eq!((mixed.experience, mixed.reputation), (41.0, 17.0));
    }

    #[test]
    fn a_minute_pays_its_silver_by_the_team_and_the_rank() {
        let minute = 60_000;
        let best = RankBonus::of(15, 0, 16);
        assert_eq!(best.permille(), BEST_RANK_BONUS_PERMILLE);
        assert_eq!(silver_of(minute, true, RankBonus::NONE), 70.0);
        assert_eq!(silver_of(minute, false, RankBonus::NONE), 59.5);
        assert_eq!(silver_of(minute, true, best), 91.0);
        assert_eq!(silver_of(minute, false, best), 77.35);
        // A whole match of 15 minutes, and a second of it.
        assert_eq!(silver_of(15 * minute, true, RankBonus::NONE), 1050.0);
        assert_eq!(silver_of(1000, true, RankBonus::NONE), 70.0 / 60.0);
        assert_eq!(silver_of(0, true, best), 0.0);
        // At a match level's multiplier and rate, rounded down as a cell:
        // the best of a won match of 15 minutes at level 15 (220 %).
        let k = multiplier(10.17) * 2.2;
        assert_eq!(cell(silver_of(15 * minute, true, best), k), 30_540.0);
    }

    /// The bounds: a minute pays from 59.5 (not won, no bonus) to 91 (won,
    /// the best), whatever the ranks given.
    #[test]
    fn a_minute_pays_between_its_bounds() {
        let minute = 60_000;
        let least = f64::from(SILVER_PER_MINUTE * NOT_WON_PERMILLE) / 1000.0;
        let most = f64::from(SILVER_PER_MINUTE * (1000 + BEST_RANK_BONUS_PERMILLE)) / 1000.0;
        assert_eq!((least, most), (59.5, 91.0));
        for finishers in 0..=20 {
            for below in 0..=finishers + 2 {
                for above in 0..=finishers + 2 {
                    let bonus = RankBonus::of(below, above, finishers);
                    assert!(bonus.permille() <= BEST_RANK_BONUS_PERMILLE);
                    for won in [false, true] {
                        let paid = silver_of(minute, won, bonus);
                        assert!((least..=most).contains(&paid), "{paid}");
                    }
                }
            }
        }
    }

    #[test]
    fn the_rank_bonus_falls_from_the_best_to_the_middle() {
        // 16 finishers, no ties: rank r scored more than 15 - r and less
        // than r.
        let of_rank = |rank: usize| RankBonus::of(15 - rank, rank, 16).permille();
        assert_eq!(
            (0..16).map(of_rank).collect::<Vec<_>>(),
            [300, 260, 220, 180, 140, 100, 60, 20, 0, 0, 0, 0, 0, 0, 0, 0]
        );
        // Five: the median, rank 2, gets none.
        let of_rank = |rank: usize| RankBonus::of(4 - rank, rank, 5).permille();
        assert_eq!((0..5).map(of_rank).collect::<Vec<_>>(), [300, 150, 0, 0, 0]);
        // One finisher: no bonus; two: the better the whole, the other none.
        assert_eq!(RankBonus::of(0, 0, 1), RankBonus::NONE);
        assert_eq!(RankBonus::of(0, 0, 0), RankBonus::NONE);
        assert_eq!(RankBonus::of(1, 0, 2).permille(), 300);
        assert_eq!(RankBonus::of(0, 1, 2), RankBonus::NONE);
        // Two of four tied for the first: each beat two of three, 200.
        assert_eq!(RankBonus::of(2, 0, 4).permille(), 200);
        // Everyone alike, two or sixteen: nobody.
        assert_eq!(RankBonus::of(0, 0, 2), RankBonus::NONE);
        assert_eq!(RankBonus::of(0, 0, 16), RankBonus::NONE);
    }

    #[test]
    fn a_multiplier_is_the_datas_decimal() {
        assert_eq!(multiplier(1.05), 1.05);
        assert_eq!(multiplier(10.17), 10.17);
        assert_eq!(cell(20.0, multiplier(1.05)), 21.0);
        assert_eq!(cell(100.0, multiplier(1.05)), 105.0);
        // Every multiplier of the data has at most two decimals.
        for level in &survarium::v026g::dictionary::Dictionary::shared().match_levels {
            for k in [level.money_k, level.experience_k, level.reputation_k] {
                let hundredths = multiplier(k) * 100.0;
                assert!((hundredths - hundredths.round()).abs() < 1e-9, "{k}");
            }
        }
    }

    #[test]
    fn the_silver_rate_rises_with_the_match_level() {
        let by_level = MoneyRate::ByLevel;
        for level in 0..=5 {
            assert_eq!(by_level.at(level), 150);
        }
        assert_eq!(
            (6..=12).map(|level| by_level.at(level)).collect::<Vec<_>>(),
            [160, 170, 180, 190, 200, 210, 220]
        );
        // Every level of the data past 12 pays as 12 does.
        let levels = survarium::v026g::dictionary::Dictionary::shared()
            .match_levels
            .len() as u16;
        assert!(levels > 13);
        for level in 12..levels + 5 {
            assert_eq!(by_level.at(level), 220);
        }
        assert_eq!(MoneyRate::Flat(80).at(0), 80);
        assert_eq!(MoneyRate::Flat(80).at(20), 80);
    }

    #[test]
    fn the_last_level_pays_250_gold() {
        assert_eq!(level_reward(1), LevelReward::Gold(100));
        assert_eq!(level_reward(2), LevelReward::Silver(500));
        assert_eq!(level_reward(5), LevelReward::Gold(100));
        assert_eq!(level_reward(6), LevelReward::Silver(750));
        assert_eq!(level_reward(95), LevelReward::Gold(100));
        assert_eq!(level_reward(99), LevelReward::Silver(5_000));
        assert_eq!(level_reward(100), LevelReward::Gold(250));
    }
}
