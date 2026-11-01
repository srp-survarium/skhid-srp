//! What a 0.26g match pays, in numbers: the three rows of the results
//! screen, the two rates, the level rewards, the rating a new account
//! starts with and the drop chance's bonuses
//! (`docs/notes/progression_sources.md` §2, §7 and
//! `results.md` §10). [`super::rewards`] applies them.
//!
//! The client holds none of these numbers. They are fitted to two results
//! screens of the time:
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

/// What one event, or one point of score, pays before the match level's
/// multipliers: silver, experience, reputation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pay {
    pub money: f64,
    pub experience: f64,
    pub reputation: f64,
}

impl Pay {
    pub const NONE: Self = Self::new(0.0, 0.0, 0.0);

    pub const fn new(money: f64, experience: f64, reputation: f64) -> Self {
        Self {
            money,
            experience,
            reputation,
        }
    }

    /// `self × factor`.
    pub fn times(self, factor: f64) -> Self {
        Self::new(
            self.money * factor,
            self.experience * factor,
            self.reputation * factor,
        )
    }

    pub fn plus(self, other: Self) -> Self {
        Self::new(
            self.money + other.money,
            self.experience + other.experience,
            self.reputation + other.reputation,
        )
    }
}

/// "Enemies killed": a kill (PROVEN by SHOT2: 19 kills are 475 / 57 / 19
/// before the multipliers).
pub const KILL: Pay = Pay::new(25.0, 3.0, 1.0);
/// A lesser kind of kill, cause unknown: SHOT1's 17 kills (375 / 41 / 17
/// before the multipliers) are exactly 7 kills of [`KILL`] and 10 of
/// this. Not used: every kill pays [`KILL`] until the cause is known.
#[allow(dead_code)]
pub const LESSER_KILL: Pay = Pay::new(20.0, 2.0, 1.0);
/// "Help ally": an assist (PROVEN by both screens). A cure and a rescue of
/// a teammate are in the same row of the game's table and are paid the
/// same (LIKELY).
pub const HELP: Pay = Pay::new(10.0, 1.0, 1.0);

// ---------------------------------------------------------------------
// GUESS: "Match Activity Rewards". Both screens are wins, so all that is
// known is the row's sum for a won match played to the end, at two "other
// scores" (the score less what kills and helps scored):
//
//   SHOT1, other score 33: 315 / 143 / 5 before the multipliers
//   SHOT2, other score 59: 498 / 159 / 6.5 to 7
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
/// For each point of other score: (498 − 315) / 26 silver, (159 − 143) /
/// 26 experience, 0.077 reputation.
pub const ACTIVITY_PER_SCORE: Pay = Pay::new(
    (498.0 - 315.0) / (SHOT2_SCORE - SHOT1_SCORE),
    (159.0 - 143.0) / (SHOT2_SCORE - SHOT1_SCORE),
    0.077,
);
/// For a won match played to the end: about 82.73 silver, 122.69
/// experience, 2.46 reputation.
pub const ACTIVITY_WON: Pay = Pay::new(
    315.0 - SHOT1_SCORE * ACTIVITY_PER_SCORE.money,
    143.0 - SHOT1_SCORE * ACTIVITY_PER_SCORE.experience,
    2.46,
);
/// The share of [`ACTIVITY_WON`] a match played to the end and not won
/// pays (the owner's choice).
pub const ACTIVITY_NOT_WON: f64 = 0.5;
// ------------------------------------------------------------- GUESS ends

/// The floor of a match's lowest (the owner's, 2026-10-27: the players
/// with the fewest points need the silver most): a player who stayed to
/// the end and scored at least [`FLOOR_MIN_SCORE`] gets at least this share
/// of the silver of the match's median player's rows, the rest added to
/// its activity line (`docs/notes/results.md`, "The floor").
pub const FLOOR_OF_MEDIAN: f64 = 0.8;
/// The least score that takes the floor: an idle player gets none.
pub const FLOOR_MIN_SCORE: u32 = 30;

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

/// The owner's choice (2026-10-14): the percent of the original's silver a
/// match's rows pay, by the match's level: 150 to level 5, ten more each
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

/// The percent of the original's silver the rows of a match pay.
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

/// `SRP_MONEY_RATE`: one percent of the original's silver for the three
/// rows of a match at every level, 1 to 1000; unset, [`MONEY_RATES`] by
/// the match's level. Premium's half is counted from the rows, so it
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

/// The drop chance each battery the player's own team has at the end adds
/// (LIKELY: the wiki of February 2015; SHOT1's 90 % does not come out of
/// it).
pub const DROP_CHANCE_PER_BATTERY: f32 = 0.025;
/// The drop chance a premium account adds (LIKELY, the same source).
pub const DROP_CHANCE_PREMIUM: f32 = 0.25;

/// The condition a weapon or an armour piece a match dropped arrives in,
/// in percent, when `SRP_DROP_CONDITION` is unset. That it arrives worn is
/// PROVEN for 0.24a and never taken back (patch note 0.24a: "all the
/// items found after a match will be in poor condition with critical
/// malfunctions"); the 40 is the owner's choice (2026-10-12).
pub const DEFAULT_DROP_CONDITION: u32 = 40;

/// `SRP_DROP_CONDITION`: that condition, 1 to 100 percent. Read once.
pub fn drop_condition() -> u32 {
    static PERCENT: std::sync::OnceLock<u32> = std::sync::OnceLock::new();
    *PERCENT.get_or_init(|| setting("SRP_DROP_CONDITION", DEFAULT_DROP_CONDITION, 100))
}

/// `SRP_DROP_DEFECT`: whether such an item arrives with one defect (the
/// same patch note); on unless `off`, `0`, `no` or `false`. Read once.
pub fn drop_defect() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| crate::switch("SRP_DROP_DEFECT", true))
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

/// `SRP_DROP_PLAYTIME_BONUS`: whether the first games of a play period
/// raise the chance of a drop (`drops.options`, `playtime_dependence`: 100,
/// 75, 50 and 25 for games 1 to 4). Off unless `on`: the owner's choice
/// (2026-10-13). How the original applied those numbers is not known
/// (GUESS: srp multiplied the table's chance by `1 + bonus / 100`); with
/// it, 73 % of the players who finished a match on the live server found
/// something, against the table's 40 %. Read once.
pub fn drop_playtime_bonus() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| crate::switch("SRP_DROP_PLAYTIME_BONUS", false))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_activity_line_goes_through_both_screens() {
        let at = |score: f64| ACTIVITY_WON.plus(ACTIVITY_PER_SCORE.times(score));
        let first = at(33.0);
        // SHOT1, level 1: 463 / 143 / 5.
        assert_eq!(cell(first.money, multiplier(1.47)), 463.0);
        assert_eq!(cell(first.experience, 1.0), 143.0);
        assert_eq!(cell(first.reputation, 1.0), 5.0);
        let second = at(59.0);
        // SHOT2, level 15: 5064 / 270 / 11.
        assert_eq!(cell(second.money, multiplier(10.17)), 5064.0);
        assert_eq!(cell(second.experience, multiplier(1.7)), 270.0);
        assert_eq!(cell(second.reputation, multiplier(1.7)), 11.0);
        // The fixed part is what the coordinator's fit names.
        assert!((ACTIVITY_WON.money - 82.73).abs() < 0.01);
        assert!((ACTIVITY_WON.experience - 122.69).abs() < 0.01);
        assert!((ACTIVITY_PER_SCORE.money - 7.04).abs() < 0.01);
        assert!((ACTIVITY_PER_SCORE.experience - 0.615).abs() < 0.001);
    }

    #[test]
    fn kills_and_help_are_the_screens() {
        let (money_k, k) = (multiplier(10.17), multiplier(1.7));
        // SHOT2: 19 kills, 8 helps.
        let kills = KILL.times(19.0);
        assert_eq!(
            (
                cell(kills.money, money_k),
                cell(kills.experience, k),
                cell(kills.reputation, k)
            ),
            (4830.0, 96.0, 32.0)
        );
        let help = HELP.times(8.0);
        assert_eq!(
            (
                cell(help.money, money_k),
                cell(help.experience, k),
                cell(help.reputation, k)
            ),
            (813.0, 13.0, 13.0)
        );
        // SHOT1: 4 helps at level 1.
        assert_eq!(cell(HELP.times(4.0).money, multiplier(1.47)), 58.0);
        // Its kills are 7 full and 10 lesser ones.
        let mixed = KILL.times(7.0).plus(LESSER_KILL.times(10.0));
        assert_eq!(
            (
                cell(mixed.money, multiplier(1.47)),
                mixed.experience,
                mixed.reputation
            ),
            (551.0, 41.0, 17.0)
        );
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
