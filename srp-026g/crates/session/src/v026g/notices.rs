//! The important messages (`A4`) srp sends of its own that are not a
//! quest's or a reputation level's ([`super::quests`]).
//!
//! **Type 14, a level reached** (client `lobby_menu::
//! important_message_arrived` 0x226B5B, PROVEN): the client logs "skill
//! point reward: %s", then takes four numbers in square brackets off the
//! text, in this order (0x226C1E..0x226C43, each a call of the bracket
//! parser 0x227120): `skill_level`, `reward_type`, `reward_id`,
//! `reward_amount`, and gives them to the lobby as an important message of
//! its type 8. The lobby (`LobbyMenu.add_important_message`) shows "Level
//! N achieved" with two rewards, `reward_amount` of money and one skill
//! point, and opens the level window (`MessagePlayerRank`), which names
//! the money by `reward_id`: 0 silver (`st_regular_money`), anything else
//! gold. The client answers the message itself (`50 <id> 00`) and asks for
//! its skills (`37 00`).
//!
//! `reward_type` is handed to the lobby and read by nothing of the level
//! window; srp sends the same number as `reward_id` [GUESS: a money type]:
//! always 0, silver. A level that paid gold pays nothing (gold is gone and
//! its rewards with it, the owner's of 2026-11-08, `pay::level_reward`): its
//! money is 0, its skill point stays.

use super::quests::Notice;

/// The important message type of a level reached.
pub const LEVEL_MESSAGE: u8 = 14;

/// The notice of reaching `level` (the account's level, counted from 0 as
/// 0.26g does): the level and what it paid
/// (`pay::level_reward`, the rule `rewards` pays by).
pub fn level_reached(level: u8) -> Notice {
    let amount = super::pay::level_reward(level);
    (LEVEL_MESSAGE, format!("[{level}][0][0][{amount}]"))
}

/// The notices of the `levels` an account reached, in order.
pub fn levels_reached(levels: &[u8]) -> Vec<Notice> {
    levels.iter().map(|&level| level_reached(level)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_level_names_its_reward() {
        // Level 1 paid 100 gold (the wiki's level 1): nothing now; level 2
        // 500 silver.
        assert_eq!(level_reached(1), (14, "[1][0][0][0]".to_owned()));
        assert_eq!(level_reached(2), (14, "[2][0][0][500]".to_owned()));
        // Every fifth level of the wiki's count paid gold: nothing.
        assert_eq!(level_reached(5), (14, "[5][0][0][0]".to_owned()));
        assert_eq!(
            levels_reached(&[2, 3]),
            [
                (14, "[2][0][0][500]".to_owned()),
                (14, "[3][0][0][500]".to_owned())
            ]
        );
        // The last level paid 250 gold: nothing.
        assert_eq!(level_reached(100), (14, "[100][0][0][0]".to_owned()));
        // What the client's parser takes: four bracketed numbers.
        let text = level_reached(50).1;
        assert_eq!(text.matches('[').count(), 4);
        assert!(text.len() < 32 * 4);
    }
}
