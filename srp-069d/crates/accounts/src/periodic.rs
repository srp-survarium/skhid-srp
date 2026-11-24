//! Per-account state of the periodic features (schema v8): the treasure hunt (item search roulette), the faction
//! battle rewards and the daily login calendar. Stored with the profile; the rules live in `lobby-server`
//! (`hunt.rs`, `faction_battle.rs`, `login_calendar.rs`).

/// One item of the current hunt page: the concrete dictionary id and whether a search already found it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HuntEntry {
    pub dict: u16,
    pub found: bool,
}

/// The treasure hunt of an account.
///
/// `period` is the unix second the current weekly period started at (0 = the hunt was never made). `free_used` counts
/// the free drop cards spent in this period; the free cards held are the stack of dict 1132 in the inventory.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HuntState {
    pub period: i64,
    pub level: u8,
    pub page: u8,
    pub free_used: u8,
    pub entries: Vec<HuntEntry>,
}

/// Reputation of one faction in one battle and the reward tiers already paid for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BattleRow {
    /// start (unix seconds) of the battle the numbers belong to
    pub start: i64,
    pub reputation: u32,
    /// tiers received, counted from the first (`2^n - 1` is the mask the client reads)
    pub received: u8,
}

/// Daily login calendar of an account: which day of the cycle is next and which days were collected.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CalendarState {
    /// day number (days since the unix epoch, UTC) of the last collection, 0 = never
    pub last_day: i64,
    /// day of the cycle (0-based) the last collection paid
    pub day: u8,
    /// bit i = day i of the cycle collected
    pub received_mask: u64,
}
