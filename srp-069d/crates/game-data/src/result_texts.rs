//! The data the `97` result window needs besides the rewards:
//!
//! * `event_categories_069d.json`: `events.options` `ui_category` of every event (0 kills, 1 help to the team, 2 the
//!   rest). The window's first three reward rows (R `+0x38`, 3 x `{str64 name, u8 show_amount, 9 x per profile}`,
//!   `score_components` of fill_match_statistic 0x20013F632..0x20013F9A8) are one row per category: the real server's
//!   sample in the lobby movie (`DebugData`) names them "Уничтожено противников" / "Помощь союзнику" / "За действия в бою"
//!   = the client keys [`ROW_LABELS`] (the exe localizes the name, 0x20013F66E).
//! * `bot_names_069d.json`: the two name lists of `ai_players_names.options` (keys) with their texts in the ten
//!   languages. The window shows a row's name raw (0x2001412BB..0x2001412DB, no localize call), so the server sends a
//!   bot's name in the receiver's language.

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::texts::ClientTexts;

pub const CATEGORIES_FILE: &str = "event_categories_069d.json";
pub const BOT_NAMES_FILE: &str = "bot_names_069d.json";

/// The client keys of the three reward rows, by `ui_category` (PROVEN by the texts: russian values equal the sample's
/// row names; english "Enemies killed", "Assists", "Activity Rewards").
pub const ROW_LABELS: [&str; 3] = ["st_statistics_score_kills", "st_statistics_score_help", "st_statistics_score_game_events"];

/// The category of an event the data does not list (none today: every `events.options` row has one).
pub const DEFAULT_CATEGORY: u8 = 2;

#[derive(Debug, Clone, Default)]
pub struct EventCategories(BTreeMap<String, u8>);

impl EventCategories {
    pub fn parse(json: &str) -> Result<Self, serde_json::Error> {
        Ok(EventCategories(serde_json::from_str(json)?))
    }

    /// `ui_category` of the event row `name` (0..=2).
    pub fn of(&self, name: &str) -> u8 {
        self.0.get(name).copied().filter(|c| *c <= 2).unwrap_or(DEFAULT_CATEGORY)
    }
}

#[derive(Debug, Deserialize)]
struct RawBotNames {
    first: Vec<String>,
    second: Vec<String>,
    texts: BTreeMap<String, BTreeMap<String, String>>,
}

/// `players_1st_names` / `players_2nd_names` and their texts.
#[derive(Debug, Clone, Default)]
pub struct BotNames {
    first: Vec<String>,
    second: Vec<String>,
    texts: ClientTexts,
}

impl BotNames {
    pub fn parse(json: &str) -> Result<Self, serde_json::Error> {
        let r: RawBotNames = serde_json::from_str(json)?;
        Ok(BotNames { first: r.first, second: r.second, texts: ClientTexts::from_map(r.texts) })
    }

    /// The name the client shows for name indexes `(first, second)` (0x20011B2D0: `players_1st_names[first]` + " " +
    /// `players_2nd_names[second]`), in client language `language` (English for 0 / unknown). None when an index is
    /// outside its list (the client then shows the record name).
    pub fn name(&self, first: usize, second: usize, language: u8) -> Option<String> {
        let f = self.first.get(first)?;
        let s = self.second.get(second)?;
        Some(format!("{} {}", self.texts.text(f, language), self.texts.text(s, language)))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn categories_and_names_are_the_client_data() {
        crate::require_tables!();
        let g = crate::testing::tables();
        assert_eq!((g.event_categories.of("kill"), g.event_categories.of("assist"), g.event_categories.of("capture_the_point")), (0, 1, 2));
        assert_eq!(g.event_categories.of("death"), 0);
        // the real server's sample: bot "Мигель Пистон" = st_bot_1st_name_24 (index 23) + st_bot_2nd_name_19 (index 17: the
        // list has no st_bot_2nd_name_6)
        assert_eq!(g.bot_names.name(23, 17, 1).as_deref(), Some("Мигель Пистон"));
        assert_eq!(g.bot_names.name(23, 17, 2).as_deref(), Some("Dean Breekie"));
        assert_eq!(g.bot_names.name(23, 17, 0), g.bot_names.name(23, 17, 2), "other -> English");
        assert_eq!(g.bot_names.name(31, 0, 1), None, "31 first names");
        assert_eq!(g.bot_names.name(0, 30, 1), None, "30 second names");
    }
}
