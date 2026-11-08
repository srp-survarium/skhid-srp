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
//! always 0, silver (gold is gone, the owner's of 2026-11-08: a level that
//! paid gold pays its silver, `pay::level_reward`).

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
        // Level 1 pays 100 gold in silver (the wiki's level 1), level 2 500
        // silver.
        assert_eq!(level_reached(1), (14, "[1][0][0][10000]".to_owned()));
        assert_eq!(level_reached(2), (14, "[2][0][0][500]".to_owned()));
        // Every fifth level of the wiki's count paid gold.
        assert_eq!(level_reached(5), (14, "[5][0][0][10000]".to_owned()));
        assert_eq!(
            levels_reached(&[2, 3]),
            [
                (14, "[2][0][0][500]".to_owned()),
                (14, "[3][0][0][500]".to_owned())
            ]
        );
        // The last level pays 250 gold in silver.
        assert_eq!(level_reached(100), (14, "[100][0][0][25000]".to_owned()));
        // What the client's parser takes: four bracketed numbers.
        let text = level_reached(50).1;
        assert_eq!(text.matches('[').count(), 4);
        assert!(text.len() < 32 * 4);
    }
}
