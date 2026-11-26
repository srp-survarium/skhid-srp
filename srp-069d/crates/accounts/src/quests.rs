//! Daily quest state of one account (stored with the profile; rules live in `lobby-server/src/quests.rs`).

/// One of the (at most 3) assigned daily quests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestSlot {
    /// instance id the client names in a reroll (`4A 03`) and in the quest messages
    pub uid: u32,
    /// index in `daily_quests.options` `quests`
    pub dict: u16,
    pub progress: u32,
    /// the goal was reached and its reward was paid
    pub done: bool,
}

/// The current set of daily quests.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DailyState {
    /// unix seconds the set was assigned at (0 = never)
    pub issued: i64,
    /// unix seconds the set ends at (`lifetime_in_minutes` after `issued`)
    pub expires: i64,
    /// free rerolls used in this set
    pub free_used: u8,
    /// reward of the daily chest once opened: item dict id (0 = not opened) and amount
    pub chest_dict: u16,
    pub chest_amount: u32,
    /// next quest instance id and next message id (never reused)
    pub next_uid: u32,
    pub next_msg: u32,
    pub slots: Vec<QuestSlot>,
}
