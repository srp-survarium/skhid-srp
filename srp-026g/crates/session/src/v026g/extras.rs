//! What the lobby's later features keep of an account, beside the
//! [`Account`](super::account::Account)'s own fields: the live watch
//! setting and the daily bonus. Kept in its own table
//! (`g26_extras`, `persist`), one row an account.
//!
//! The rules are in [`super::tutorial`], [`super::bonus`] and
//! [`super::autobuy`]; this is only the state.

/// The daily bonus of an account (`super::bonus`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DailyBonus {
    /// The day of the run of days it is in, 1..=7; 0 before its first.
    pub day: u8,
    /// The day (Unix days, UTC) `day` was reached on.
    pub last_day: u64,
    /// The victories that got the day's bonus, for each day of the run so
    /// far (0..=3 each).
    pub wins: [u8; 7],
    /// The seventh day was made: the run is complete. (It gives nothing:
    /// its gold reward is gone, the owner's of 2026-11-08.)
    pub completed: bool,
    /// The day (Unix days) the welcome window was last opened by the
    /// server.
    pub shown_day: u64,
}

/// See the module.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Extras {
    /// Whether others may watch its matches live (the Settings window's box).
    pub live_watch: survarium::v026g::lobby::LiveWatch,
    pub bonus: DailyBonus,
}

impl DailyBonus {
    /// `wins` as one number for the table: two bits a day.
    pub fn packed_wins(&self) -> u32 {
        self.wins
            .iter()
            .enumerate()
            .fold(0, |packed, (day, &wins)| {
                packed | u32::from(wins.min(3)) << (2 * day)
            })
    }

    pub fn unpack_wins(packed: u32) -> [u8; 7] {
        std::array::from_fn(|day| (packed >> (2 * day) & 3) as u8)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_wins_of_a_run_pack_into_a_number() {
        let bonus = DailyBonus {
            wins: [3, 0, 1, 2, 3, 0, 3],
            ..DailyBonus::default()
        };
        assert_eq!(DailyBonus::unpack_wins(bonus.packed_wins()), bonus.wins);
        assert_eq!(DailyBonus::default().packed_wins(), 0);
        // More than a day counts is kept as the most.
        let over = DailyBonus {
            wins: [9; 7],
            ..DailyBonus::default()
        };
        assert_eq!(DailyBonus::unpack_wins(over.packed_wins()), [3; 7]);
    }
}
