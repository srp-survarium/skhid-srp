//! The game's own texts: `localization/<language>/localization`, a binary
//! config whose `strings` table holds every text the client shows by its
//! key (`st_level_01_name` "Rudnya", `st_mode_04_name` "Team Deathmatch").
//! A map's and a mode's keys are in its game mode config
//! ([`super::maps::MapMode::map_name_key`]), so a map a patch adds names
//! itself, as the patch's localizations name it.
//!
//! The server data has the languages of srp's web page
//! ([`resources_db::server::LOCALIZATIONS`]). The strings are UTF-8 in the
//! file (the binary config reader takes them for Windows-1251, which gives
//! every byte back as it was: they are read back so here).

use std::collections::HashMap;
use std::sync::OnceLock;

use binary_config_parser::tree::{ConfigError, ConfigValue};
use encoding_rs::WINDOWS_1251;

use super::game_data::{GameData, GameDataError};

/// A language of the game's that the server data has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    English,
    Ukrainian,
}

impl Language {
    pub const ALL: [Language; 2] = [Language::English, Language::Ukrainian];

    /// Its file in the game's archive.
    pub fn path(self) -> &'static str {
        resources_db::server::LOCALIZATIONS[self as usize]
    }
}

/// One language's texts, by key.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Localization {
    strings: HashMap<String, String>,
}

impl Localization {
    /// The texts of a `localization` file: every string of its `strings`.
    pub fn parse(bytes: &[u8]) -> Result<Self, ConfigError> {
        let root = ConfigValue::parse(bytes)?;
        let strings = root
            .req("strings")?
            .entries()
            .iter()
            .filter_map(|(key, value)| Some((key.clone(), utf8(value.as_str()?))))
            .collect();
        Ok(Self { strings })
    }

    /// The text of `key`, if the language has it.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.strings.get(key).map(String::as_str)
    }

    pub fn len(&self) -> usize {
        self.strings.len()
    }

    pub fn is_empty(&self) -> bool {
        self.strings.is_empty()
    }
}

/// A string of the file as its bytes say: UTF-8, else as read.
fn utf8(read: &str) -> String {
    let (bytes, _, unmappable) = WINDOWS_1251.encode(read);
    match (unmappable, String::from_utf8(bytes.into_owned())) {
        (false, Ok(text)) => text,
        _ => read.to_owned(),
    }
}

/// The texts of `language` in `data`, if it has them.
pub fn of(data: &GameData, language: Language) -> Result<Option<Localization>, GameDataError> {
    let path = language.path();
    data.file(path)
        .map(|bytes| {
            Localization::parse(bytes)
                .map_err(|cause| GameDataError(format!("{path}: {}", cause.0)))
        })
        .transpose()
}

/// The texts of `language` of this process's game data
/// ([`super::game_data::shared`]); empty if it has none.
pub fn shared(language: Language) -> &'static Localization {
    static SHARED: [OnceLock<Localization>; 2] = [OnceLock::new(), OnceLock::new()];
    SHARED[language as usize].get_or_init(|| {
        of(super::game_data::shared(), language)
            .expect("the localizations read")
            .unwrap_or_default()
    })
}

/// The game's text of `key` in `language`, else in English.
pub fn text(language: Language, key: &str) -> Option<&'static str> {
    shared(language)
        .get(key)
        .or_else(|| shared(Language::English).get(key))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_maps_and_modes_are_named_as_the_game_names_them() {
        assert_eq!(text(Language::English, "st_level_01_name"), Some("Rudnya"));
        assert_eq!(text(Language::Ukrainian, "st_level_01_name"), Some("Рудня"));
        assert_eq!(
            text(Language::Ukrainian, "st_level_03_name"),
            Some("РЛС \"Восток\"")
        );
        assert_eq!(
            text(Language::Ukrainian, "st_mode_04_name"),
            Some("Командний бій")
        );
        assert_eq!(
            text(Language::English, "st_mode_02_name"),
            Some("Protective device")
        );
        assert_eq!(text(Language::English, "no such key"), None);
        // Every playable map's and mode's key is in both; Blackout's mode
        // name is srp's own (`client-patches/localization`), which the test
        // data's stock localizations lack.
        let blackout = super::super::maps::GameMode::Blackout;
        for map in super::super::maps::playable().filter(|map| map.mode != blackout) {
            for language in Language::ALL {
                assert!(
                    shared(language).get(map.map_name_key).is_some(),
                    "{}",
                    map.id
                );
                assert!(
                    shared(language).get(map.mode_name_key).is_some(),
                    "{}",
                    map.id
                );
            }
        }
    }

    #[test]
    fn a_key_the_language_lacks_is_the_english_text() {
        // A text in English only (the test data's localizations have every
        // map), as a patch that names a map in English alone gives it.
        let english = Localization::parse(
            &ConfigValue::Named(vec![(
                "strings".into(),
                ConfigValue::Named(vec![
                    (
                        "st_level_12_name".into(),
                        ConfigValue::String("Twelve".into()),
                    ),
                    ("not a text".into(), ConfigValue::Integer(1)),
                ]),
            )])
            .to_bytes()
            .unwrap(),
        )
        .unwrap();
        assert_eq!(english.get("st_level_12_name"), Some("Twelve"));
        assert_eq!(english.len(), 1);
        assert!(Localization::parse(b"not a config").is_err());
        let no_strings = ConfigValue::Named(Vec::new()).to_bytes().unwrap();
        assert!(Localization::parse(&no_strings).is_err());
    }
}
