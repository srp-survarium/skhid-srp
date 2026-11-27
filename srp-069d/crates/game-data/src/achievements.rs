//! Achievements: the 79 definitions of `gameplay/quests/achievements.options` (extracted by
//! `crates/data-extract` into `data/achievements_069d.json`).
//!
//! Each achievement has `states` (levels): a state is reached when the progress value is at least its `condition`;
//! reaching it gives `score` points to the achievement's category and a reward (`reward_type` 0 = item with dict id
//! `reward_id`, `reward_num` pieces; every row of the file has type 0). `kind` is the file's `type` (it selects the
//! counting rule; it is not sent: the byte of a B9 / `83 55 02` entry is [`Achievement::reached`], the client's
//! `current_state`, see `protocol::s2c::achievements`). `apply` 0 = the value is a current figure (level, rank, ...),
//! 1 = a counter that grows with each qualifying event, 2 = a streak.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::Value;

pub const ACHIEVEMENTS_FILE: &str = "achievements_069d.json";

#[derive(Debug, Clone, Deserialize)]
pub struct State {
    pub condition: u32,
    pub score: u32,
    pub reward_type: u32,
    pub reward_id: u32,
    pub reward_num: u32,
}

#[derive(Debug, Clone, Deserialize)]
struct RawState(u32, u32, u32, u32, u32);

#[derive(Debug, Deserialize)]
struct RawAchievement {
    id: u16,
    category: u8,
    #[serde(rename = "type")]
    kind: u8,
    apply: u8,
    pre: BTreeMap<String, Value>,
    states: Vec<RawState>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawAchievements {
    achievements: Vec<RawAchievement>,
    categories: Vec<(u8, u32)>,
}

#[derive(Debug, Clone)]
pub struct Achievement {
    pub id: u16,
    pub category: u8,
    pub kind: u8,
    pub apply: u8,
    /// `preconditions` of the file (names and values as in the file)
    pub pre: BTreeMap<String, Value>,
    pub states: Vec<State>,
}

impl Achievement {
    pub fn pre_u64(&self, name: &str) -> Option<u64> {
        self.pre.get(name).and_then(Value::as_u64)
    }

    pub fn pre_bool(&self, name: &str) -> Option<bool> {
        self.pre.get(name).and_then(Value::as_bool)
    }

    /// Number of states reached with progress `value`.
    pub fn reached(&self, value: u32) -> u8 {
        self.states.iter().filter(|s| value >= s.condition).count() as u8
    }
}

#[derive(Debug, Default)]
pub struct Achievements {
    list: Vec<Achievement>,
    /// (category id, total score of the category = sum of the scores of all its states)
    categories: Vec<(u8, u32)>,
}

impl Achievements {
    pub(crate) fn new(raw: RawAchievements) -> Achievements {
        let list = raw
            .achievements
            .into_iter()
            .map(|a| Achievement {
                id: a.id,
                category: a.category,
                kind: a.kind,
                apply: a.apply,
                pre: a.pre,
                states: a
                    .states
                    .into_iter()
                    .map(|s| State { condition: s.0, score: s.1, reward_type: s.2, reward_id: s.3, reward_num: s.4 })
                    .collect(),
            })
            .collect();
        Achievements { list, categories: raw.categories }
    }

    pub fn get(&self, id: u16) -> Option<&Achievement> {
        self.list.iter().find(|a| a.id == id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Achievement> {
        self.list.iter()
    }

    pub fn len(&self) -> usize {
        self.list.len()
    }

    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    pub fn categories(&self) -> &[(u8, u32)] {
        &self.categories
    }

    /// Earned score per category for the given progress values (`83 55 03`: `u8 category, u16 score`).
    pub fn category_scores(&self, value_of: impl Fn(u16) -> u32) -> Vec<(u8, u16)> {
        self.categories
            .iter()
            .map(|&(cat, _)| {
                let s: u32 = self
                    .list
                    .iter()
                    .filter(|a| a.category == cat)
                    .map(|a| a.states.iter().take(a.reached(value_of(a.id)) as usize).map(|s| s.score).sum::<u32>())
                    .sum();
                (cat, s.min(0xFFFF) as u16)
            })
            .collect()
    }
}

/// `mode_mask` bit of a match mode id: bit n = game mode n (`game_modes/*.options` `mode`; the exe's mode table
/// 0x200987060: 0 gvi / battery, 1 tdm, 2 capt / research, 3 gunrace, 4 pb, 5 sad, 6 slaughter, 7 pve, 8 dom, 9 ah).
/// Modes past 31 have no bit.
///
/// The client only loads the mask (descriptor +0x1F4, loader 0x20034CF58) and copies it; nothing in the exe compares it,
/// so the numbering is read from the data and the texts: 610 / 611 / 612 (masks 1 / 2 / 4) are the gvi / tdm / ctp
/// achievements by name, every other mode mask of the exe uses the same numbering (the `AC` play gate reader
/// 0x20018AEC0, the `2F` queue mask), and 627 / 628 (mask 32 = bit 5 = sad) say "in the Artifact Hunt or Search and
/// Destroy modes", 627 "with 5 - 0 score": Search and Destroy is won at 5 rounds (`rounds_to_win` 5) and its team
/// values are rounds (0x2003AE229). PROVEN numbering (data + texts + the exe's mode table). Artifact Hunt (bit 9) is not
/// in the data's mask, so an Artifact Hunt win does not count (it is custom-only in 0.69d anyway).
pub fn mode_mask(mode: u32) -> u32 {
    1u32.checked_shl(mode).unwrap_or(0)
}

#[cfg(test)]
mod mode_mask_tests {
    use super::mode_mask;

    #[test]
    fn bit_n_is_game_mode_n() {
        assert_eq!((mode_mask(0), mode_mask(1), mode_mask(2)), (1, 2, 4), "gvi, tdm, ctp: 610 / 611 / 612");
        assert_eq!(mode_mask(5), 32, "sad: 627 / 628");
        assert_eq!(mode_mask(9), 512, "ah has its own bit, not 32");
        assert_eq!((mode_mask(31), mode_mask(32)), (1 << 31, 0));
    }
}
