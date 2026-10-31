//! The daily bonus (`docs/notes/results.md` §4.1; lobby `99`).
//!
//! What the game says of it (its own texts, PROVEN): "Play everyday and
//! get x2 experience and reputation for the first three victories"
//! (`st_daily_bonus_description`) and "On 7th day you will get" the reward
//! shown (`st_daily_bonus_reward_description`; "Complete" once given). The
//! window (`MessageDailyReward`) shows seven days, each with a progress of
//! three, the current day, and the reward item; a day before the current
//! one is shown done whatever its progress
//! (`LobbyMenu.show_daily_reward_window`). The status bar's indicator is
//! the current day's progress, 0..=3.
//!
//! What gives the seventh day's reward is the game's `daily_bonus_drop`
//! table (`survarium::v026g::drops`: it always gives 50 gold).
//!
//! How the seven days run (PROVEN, the patch notes of 0.24a and 0.24c:
//! "the reward for three wins in a series of seven days is not reset if
//! you missed a day or were not able to win three times";
//! `progression_sources.md` §2):
//! - A day of the run **counts when its three victories are made**. The
//!   run's `day` is the place being played for, 1..=7; the places before
//!   it are made.
//! - A day on which the three are not made, or on which the account does
//!   not play, **does not reset the run**: the place is played for again
//!   on the next day, from no victories.
//! - The reward **comes with the third victory of the seventh place**.
//!   The day after, a new run starts.
//!
//! The rest is srp's own [choice], kept simple:
//! - A **day** is a day of UTC that starts at the hour
//!   `SRP_DAILY_BONUS_RESET_HOUR` names (0..=23, default 0).
//! - `SRP_DAILY_BONUS` (on unless `off`, `0`, `no` or `false`) turns all
//!   of it off: no match is paid double, no run of days is kept, and the
//!   client is sent nothing of it (with no `99` it shows no bonus, as for
//!   an account before its first day).
//! - The reward is rolled when a run starts (the window shows it from day
//!   1).
//! - A **victory** counts when the player's team won and the player stayed
//!   to the end; the first three of a day pay the match's experience and
//!   reputation once more ([`super::rewards`], the results screen's
//!   "daily" block).
//! - A run of before this rule (a day then counted when the account was
//!   seen) goes on from the place it had reached.
//! - The welcome window is opened by the server once a day, at the first
//!   sign-in of the day.

use survarium::v026g::dictionary::Dictionary;
use survarium::v026g::drops::{self, Drops, Rng};
use survarium::v026g::lobby::{self, ItemDescr, ServerMessage, WireError};

use super::account::Account;
use super::premium;

/// The days of a run.
pub const DAYS: u8 = 7;
/// The victories of a day that are paid double.
pub const WINS_A_DAY: u8 = 3;
/// Silver and gold as dropped items.
pub(crate) const GENERIC_MONEY: u16 = 214;
pub(crate) const PREMIUM_MONEY: u16 = 215;
/// What the table gives, should a roll ever give nothing: its 50 gold.
pub(crate) const FALLBACK: (u16, u32) = (PREMIUM_MONEY, 50);

/// The operator's settings of the daily bonus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    /// `SRP_DAILY_BONUS`.
    pub enabled: bool,
    /// `SRP_DAILY_BONUS_RESET_HOUR`: the hour of UTC a bonus day starts
    /// at.
    pub reset_hour: u8,
}

impl Settings {
    pub const DEFAULT: Self = Self {
        enabled: true,
        reset_hour: 0,
    };

    /// The settings from the environment `get` reads.
    pub fn read(get: impl Fn(&str) -> Option<String>) -> Self {
        let enabled =
            crate::switch_value("SRP_DAILY_BONUS", get("SRP_DAILY_BONUS").as_deref(), true);
        let reset_hour = match get("SRP_DAILY_BONUS_RESET_HOUR") {
            Some(value) => match value.trim().parse::<u8>() {
                Ok(hour) if hour < 24 => hour,
                _ => {
                    log::error!(
                        "SRP_DAILY_BONUS_RESET_HOUR is `{value}`; it must be 0 to 23. Using 0"
                    );
                    0
                }
            },
            None => 0,
        };
        Self {
            enabled,
            reset_hour,
        }
    }
}

/// The settings of this srp, read once.
pub fn settings() -> &'static Settings {
    static SHARED: std::sync::OnceLock<Settings> = std::sync::OnceLock::new();
    SHARED.get_or_init(|| Settings::read(|name| std::env::var(name).ok()))
}

/// The bonus day `now` (Unix seconds) is in when a day starts at
/// `reset_hour` of UTC.
pub fn day_at(now: u64, reset_hour: u8) -> u64 {
    now.saturating_sub(u64::from(reset_hour) * 3600) / 86_400
}

/// The bonus day `now` (Unix seconds) is in.
pub fn day_of(now: u64) -> u64 {
    day_at(now, settings().reset_hour)
}

/// What the seventh day gave.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Given {
    pub dict_id: u16,
    pub count: u32,
    /// The instance in the inventory; 0 for silver and gold, which go to
    /// the balances.
    pub item_id: u32,
}

/// What a visit did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Visit {
    /// The account's first visit of the day.
    pub new_day: bool,
}

/// What a victory did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Win {
    /// It is one of the day's first three: the match pays its experience
    /// and reputation once more.
    pub doubled: bool,
    /// The run's reward, given by this victory: the third of the seventh
    /// day.
    pub given: Option<Given>,
}

fn start_run(account: &mut Account, today: u64, rng: &mut Rng) {
    let rolled = drops::roll(
        &Drops::shared().daily_bonus,
        Dictionary::shared(),
        0,
        0,
        rng,
    );
    let (dict_id, count) = rolled.map_or(FALLBACK, |drop| (drop.dict_id, drop.count));
    let bonus = &mut account.extras.bonus;
    bonus.day = 1;
    bonus.last_day = today;
    bonus.wins = [0; 7];
    bonus.reward_dict = dict_id;
    bonus.reward_count = count;
    bonus.completed = false;
}

/// The account is seen at `now`: its run is brought up to the day. On a
/// new day the place whose three victories were made gives way to the
/// next (after the seventh, to a new run); a place not made is played for
/// again. Nothing with the bonus off.
pub fn visit(account: &mut Account, now: u64, rng: &mut Rng) -> Visit {
    visit_with(account, now, rng, settings())
}

/// [`visit`] under `settings`.
pub fn visit_with(account: &mut Account, now: u64, rng: &mut Rng, settings: &Settings) -> Visit {
    if !settings.enabled {
        return Visit::default();
    }
    let today = day_at(now, settings.reset_hour);
    let bonus = account.extras.bonus;
    if bonus.day == 0 {
        start_run(account, today, rng);
        return Visit { new_day: true };
    }
    if today <= bonus.last_day {
        return Visit::default();
    }
    let place = usize::from(bonus.day.min(DAYS)) - 1;
    let made = bonus.wins[place] >= WINS_A_DAY;
    if bonus.day >= DAYS && (made || bonus.completed) {
        start_run(account, today, rng);
    } else {
        let bonus = &mut account.extras.bonus;
        if made {
            bonus.day += 1;
        } else {
            bonus.wins[place] = 0;
        }
        bonus.last_day = today;
    }
    Visit { new_day: true }
}

/// A victory: counts it for the day, and gives the run's reward when it
/// is the third of the seventh day. Call [`visit`] first. Nothing with
/// the bonus off.
pub fn count_win(account: &mut Account) -> Win {
    count_win_with(account, settings())
}

/// [`count_win`] under `settings`.
pub fn count_win_with(account: &mut Account, settings: &Settings) -> Win {
    if !settings.enabled {
        return Win::default();
    }
    let bonus = &mut account.extras.bonus;
    let Some(wins) = usize::from(bonus.day)
        .checked_sub(1)
        .and_then(|index| bonus.wins.get_mut(index))
    else {
        return Win::default();
    };
    if *wins >= WINS_A_DAY {
        return Win::default();
    }
    *wins += 1;
    let mut given = None;
    if *wins == WINS_A_DAY && bonus.day == DAYS && !bonus.completed {
        bonus.completed = true;
        let (dict_id, count) = (bonus.reward_dict, bonus.reward_count);
        let item_id = match dict_id {
            GENERIC_MONEY => {
                account.generic_money = account.generic_money.saturating_add(count);
                0
            }
            PREMIUM_MONEY => {
                account.premium_money = account.premium_money.saturating_add(count);
                0
            }
            _ => premium::grant(account, dict_id, count)
                .first()
                .map_or(0, |bought| bought.item_id),
        };
        given = Some(Given {
            dict_id,
            count,
            item_id,
        });
    }
    Win {
        doubled: true,
        given,
    }
}

/// The current day's progress, 0..=3.
pub fn progress(account: &Account) -> u8 {
    let bonus = &account.extras.bonus;
    usize::from(bonus.day)
        .checked_sub(1)
        .and_then(|index| bonus.wins.get(index))
        .copied()
        .unwrap_or(0)
}

/// Whether the server has yet to open the welcome window today.
pub fn window_due(account: &Account, now: u64) -> bool {
    settings().enabled
        && account.extras.bonus.day != 0
        && account.extras.bonus.shown_day != day_of(now)
}

/// The welcome window was opened at `now`.
pub fn window_shown(account: &mut Account, now: u64) {
    account.extras.bonus.shown_day = day_of(now);
}

/// `99` for the account: the days of its run with their progress, the
/// reward and whether it was given; `show_window` opens the window. `None`
/// before the account's first [`visit`], and with the bonus off.
pub fn message(account: &Account, show_window: bool) -> Option<Result<ServerMessage, WireError>> {
    message_with(account, show_window, settings())
}

/// [`message`] under `settings`.
pub fn message_with(
    account: &Account,
    show_window: bool,
    settings: &Settings,
) -> Option<Result<ServerMessage, WireError>> {
    let bonus = &account.extras.bonus;
    if bonus.day == 0 || !settings.enabled {
        return None;
    }
    let days = usize::from(bonus.day.min(DAYS));
    let reward = ItemDescr::plain(0, bonus.reward_dict, bonus.reward_count);
    Some(lobby::daily_bonus(
        &bonus.wins[..days],
        &reward,
        bonus.completed,
        show_window,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use survarium::v026g::lobby::client_model::{Client, Decoded};

    const DAY: u64 = 86_400;
    /// Noon of some day.
    const NOON: u64 = 20_000 * DAY + DAY / 2;

    fn rng() -> Rng {
        Rng::new(7)
    }

    #[test]
    fn a_run_starts_with_the_first_visit() {
        let mut account = Account::new("Alice");
        assert_eq!(message(&account, true), None);
        assert!(!window_due(&account, NOON));
        let visit = super::visit(&mut account, NOON, &mut rng());
        assert_eq!(visit, Visit { new_day: true });
        let bonus = account.extras.bonus;
        assert_eq!((bonus.day, bonus.last_day), (1, 20_000));
        // The table's reward: 50 gold.
        assert_eq!((bonus.reward_dict, bonus.reward_count), (215, 50));
        assert!(!bonus.completed);
        // The same day again, later: nothing.
        assert_eq!(
            super::visit(&mut account, NOON + 3600, &mut rng()),
            Visit::default()
        );
        // The window is due once a day.
        assert!(window_due(&account, NOON));
        window_shown(&mut account, NOON);
        assert!(!window_due(&account, NOON + 3600));
        assert!(window_due(&account, NOON + DAY));
    }

    #[test]
    fn the_message_is_the_clients_layout() {
        let mut account = Account::new("Alice");
        super::visit(&mut account, NOON, &mut rng());
        account.extras.bonus.wins[0] = 2;
        let message = message(&account, true).unwrap().unwrap();
        // One day with progress 2; item id 0, a stack, dict 215, 50 of it,
        // no modifications or upgrades; not given; open the window.
        assert_eq!(
            message.body(),
            [1, 2, 1, 0, 0, 0, 0, 215, 0, 50, 0, 0, 0, 0, 0, 0, 1]
        );
        assert_eq!(message.id_byte(), 0x99);
        let mut client = Client::signed_in();
        let Decoded::DailyBonus {
            progress,
            reward,
            completed,
            show_window,
        } = client.receive(&message).unwrap()
        else {
            panic!("not the daily bonus");
        };
        assert_eq!(progress, [2]);
        assert_eq!((reward.dict_id, reward.condition_or_stack), (215, 50));
        assert!(!completed && show_window);
    }

    /// Three victories on the day of `now`; what the third gave.
    fn win_three(account: &mut Account, now: u64, rng: &mut Rng) -> Option<Given> {
        super::visit(account, now, rng);
        let plain = Win {
            doubled: true,
            given: None,
        };
        assert_eq!(count_win(account), plain);
        assert_eq!(count_win(account), plain);
        let third = count_win(account);
        assert!(third.doubled);
        third.given
    }

    #[test]
    fn the_seventh_day_made_gives_the_reward() {
        let mut account = Account::new("Alice");
        let gold = account.premium_money;
        let mut rng = rng();
        for day in 0..6 {
            assert_eq!(win_three(&mut account, NOON + day * DAY, &mut rng), None);
            assert_eq!(u64::from(account.extras.bonus.day), day + 1);
        }
        assert_eq!(account.premium_money, gold);
        // The seventh day: seen, two victories, nothing yet.
        super::visit(&mut account, NOON + 6 * DAY, &mut rng);
        assert_eq!(account.extras.bonus.day, 7);
        count_win(&mut account);
        count_win(&mut account);
        assert_eq!(account.premium_money, gold);
        assert!(!account.extras.bonus.completed);
        // The third gives it.
        assert_eq!(
            count_win(&mut account),
            Win {
                doubled: true,
                given: Some(Given {
                    dict_id: 215,
                    count: 50,
                    item_id: 0
                })
            }
        );
        assert_eq!(account.premium_money, gold + 50);
        assert!(account.extras.bonus.completed);
        // Seven days on the wire, the reward given.
        let message = message(&account, false).unwrap().unwrap();
        assert_eq!(message.body()[..8], [7, 3, 3, 3, 3, 3, 3, 3]);
        let mut client = Client::signed_in();
        assert!(matches!(
            client.receive(&message).unwrap(),
            Decoded::DailyBonus {
                completed: true,
                show_window: false,
                ..
            }
        ));
        // Once: a fourth victory of the day is a plain one.
        assert_eq!(count_win(&mut account), Win::default());
        assert_eq!(account.premium_money, gold + 50);
        // The day after starts a new run.
        super::visit(&mut account, NOON + 7 * DAY, &mut rng);
        assert_eq!(account.extras.bonus.day, 1);
        assert_eq!(account.extras.bonus.wins, [0; 7]);
        assert!(!account.extras.bonus.completed);
        assert_eq!(account.premium_money, gold + 50);
    }

    #[test]
    fn a_day_missed_or_not_made_does_not_reset_the_run() {
        let mut account = Account::new("Alice");
        let mut rng = rng();
        // Two days made.
        win_three(&mut account, NOON, &mut rng);
        win_three(&mut account, NOON + DAY, &mut rng);
        // The third: seen, one victory only. The day does not count.
        super::visit(&mut account, NOON + 2 * DAY, &mut rng);
        assert_eq!(account.extras.bonus.day, 3);
        count_win(&mut account);
        assert_eq!(account.extras.bonus.wins, [3, 3, 1, 0, 0, 0, 0]);
        // Three days away, then back: the third place again, from none.
        assert_eq!(
            super::visit(&mut account, NOON + 6 * DAY, &mut rng),
            Visit { new_day: true }
        );
        assert_eq!(account.extras.bonus.day, 3);
        assert_eq!(account.extras.bonus.wins, [3, 3, 0, 0, 0, 0, 0]);
        assert_eq!(progress(&account), 0);
        // A day only seen changes nothing either.
        super::visit(&mut account, NOON + 7 * DAY, &mut rng);
        assert_eq!(account.extras.bonus.day, 3);
        // Five more days made, whenever: the reward on the seventh.
        let mut at = NOON + 9 * DAY;
        for place in 3..=6 {
            assert_eq!(win_three(&mut account, at, &mut rng), None, "{place}");
            at += 2 * DAY;
        }
        assert_eq!(account.extras.bonus.day, 6);
        let given = win_three(&mut account, at, &mut rng);
        assert_eq!(account.extras.bonus.day, 7);
        assert_eq!(given.map(|given| given.count), Some(50));
        // A clock that went back changes nothing.
        assert_eq!(super::visit(&mut account, NOON, &mut rng), Visit::default());
    }

    #[test]
    fn the_settings_are_read_from_the_environment() {
        let read = |pairs: &[(&str, &str)]| {
            Settings::read(|name| {
                pairs
                    .iter()
                    .find(|(key, _)| *key == name)
                    .map(|(_, value)| (*value).to_owned())
            })
        };
        assert_eq!(read(&[]), Settings::DEFAULT);
        assert_eq!(
            Settings::DEFAULT,
            Settings {
                enabled: true,
                reset_hour: 0
            }
        );
        for off in ["off", "0", "no", "false", " OFF "] {
            assert!(!read(&[("SRP_DAILY_BONUS", off)]).enabled, "{off}");
        }
        for on in ["on", "1", "yes", ""] {
            assert!(read(&[("SRP_DAILY_BONUS", on)]).enabled, "{on}");
        }
        assert_eq!(read(&[("SRP_DAILY_BONUS_RESET_HOUR", "5")]).reset_hour, 5);
        assert_eq!(
            read(&[("SRP_DAILY_BONUS_RESET_HOUR", " 23 ")]).reset_hour,
            23
        );
        // Not an hour: the default.
        for bad in ["24", "-1", "noon", ""] {
            assert_eq!(
                read(&[("SRP_DAILY_BONUS_RESET_HOUR", bad)]).reset_hour,
                0,
                "{bad}"
            );
        }
    }

    #[test]
    fn with_the_bonus_off_nothing_runs_and_nothing_is_sent() {
        let off = Settings {
            enabled: false,
            reset_hour: 0,
        };
        let mut rng = rng();
        // A new account: no run starts, no victory counts, no message.
        let mut account = Account::new("Alice");
        let before = account.clone();
        assert_eq!(
            visit_with(&mut account, NOON, &mut rng, &off),
            Visit::default()
        );
        assert_eq!(count_win_with(&mut account, &off), Win::default());
        assert_eq!(account, before);
        assert_eq!(message_with(&account, true, &off), None);
        // An account in a run of before, two victories into its seventh
        // day: it stays as it is, gets nothing and is sent nothing.
        super::visit(&mut account, NOON, &mut rng);
        account.extras.bonus.day = 7;
        account.extras.bonus.wins = [3, 3, 3, 3, 3, 3, 2];
        let before = account.clone();
        assert_eq!(
            visit_with(&mut account, NOON + DAY, &mut rng, &off),
            Visit::default()
        );
        assert_eq!(count_win_with(&mut account, &off), Win::default());
        assert_eq!(account, before);
        assert_eq!(message_with(&account, false, &off), None);
        // Turned on again, the run goes on.
        assert!(message_with(&account, false, &Settings::DEFAULT).is_some());
        assert!(
            count_win_with(&mut account, &Settings::DEFAULT)
                .given
                .is_some()
        );
    }

    #[test]
    fn a_bonus_day_starts_at_the_reset_hour() {
        // Midnight of UTC by default.
        assert_eq!(day_at(20_000 * DAY - 1, 0), 19_999);
        assert_eq!(day_at(20_000 * DAY, 0), 20_000);
        // At 05:00: the hours before it belong to the day before.
        assert_eq!(day_at(20_000 * DAY + 5 * 3600 - 1, 5), 19_999);
        assert_eq!(day_at(20_000 * DAY + 5 * 3600, 5), 20_000);
        assert_eq!(day_at(100, 23), 0);

        let at_five = Settings {
            enabled: true,
            reset_hour: 5,
        };
        let mut rng = rng();
        let mut account = Account::new("Alice");
        // 23:00: three victories make the day.
        let evening = 20_000 * DAY + 23 * 3600;
        visit_with(&mut account, evening, &mut rng, &at_five);
        for _ in 0..3 {
            assert!(count_win_with(&mut account, &at_five).doubled);
        }
        // 04:00 of the next date is the same bonus day: no new one, and a
        // victory is a plain one.
        let night = evening + 5 * 3600;
        assert_eq!(
            visit_with(&mut account, night, &mut rng, &at_five),
            Visit::default()
        );
        assert!(!count_win_with(&mut account, &at_five).doubled);
        assert_eq!(account.extras.bonus.day, 1);
        // 05:00: the second day.
        assert_eq!(
            visit_with(&mut account, night + 3600, &mut rng, &at_five),
            Visit { new_day: true }
        );
        assert_eq!(account.extras.bonus.day, 2);
        assert!(count_win_with(&mut account, &at_five).doubled);
    }

    #[test]
    fn a_run_of_the_old_rule_goes_on() {
        // Day 7 reached by being seen, its reward given, no victory made:
        // the next day starts a new run.
        let mut account = Account::new("Alice");
        let mut rng = rng();
        super::visit(&mut account, NOON, &mut rng);
        account.extras.bonus.day = 7;
        account.extras.bonus.completed = true;
        account.extras.bonus.wins = [1, 0, 3, 0, 0, 0, 0];
        super::visit(&mut account, NOON + DAY, &mut rng);
        assert_eq!(account.extras.bonus.day, 1);
        assert!(!account.extras.bonus.completed);
        // Day 4 reached so: it is played for from there.
        account.extras.bonus.day = 4;
        account.extras.bonus.wins = [1, 0, 3, 2, 0, 0, 0];
        super::visit(&mut account, NOON + 2 * DAY, &mut rng);
        assert_eq!(account.extras.bonus.day, 4);
        assert_eq!(account.extras.bonus.wins, [1, 0, 3, 0, 0, 0, 0]);
    }

    #[test]
    fn the_first_three_victories_of_a_day_count() {
        let mut account = Account::new("Alice");
        // Before any visit there is no day to count in.
        assert_eq!(count_win(&mut account), Win::default());
        let mut rng = rng();
        super::visit(&mut account, NOON, &mut rng);
        assert_eq!(progress(&account), 0);
        assert!(count_win(&mut account).doubled);
        assert!(count_win(&mut account).doubled);
        assert!(count_win(&mut account).doubled);
        assert!(!count_win(&mut account).doubled);
        assert_eq!(progress(&account), 3);
        // The next day counts anew, and the day before keeps its three.
        super::visit(&mut account, NOON + DAY, &mut rng);
        assert_eq!(progress(&account), 0);
        assert!(count_win(&mut account).doubled);
        assert_eq!(account.extras.bonus.wins[..2], [3, 1]);
    }
}
