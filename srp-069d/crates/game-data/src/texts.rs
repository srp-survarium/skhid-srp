//! The client's own texts for the few keys the server shows in fields the client does NOT localize: the store's `BC`
//! card `label` / `description` and the `95` tab names (PROVEN: no localize call). The
//! original server sent those per language (the `5C 00` request carries the client's language index), so the server
//! keeps the values of the 11 shipped language files for these keys (`store_texts_069d.json`, made by `crates/data-extract`
//! from the client's volume 70) and sends the one of the player's language.

use std::collections::BTreeMap;

pub const FILE: &str = "store_texts_069d.json";

/// The client's language index (0x2000915E0, PROVEN; `5C 00` byte 6): 1 russian, 2 english, 3 german, 4 polish,
/// 5 ukrainian, 6 french, 7 spanish, 8 chinese, 9 portuguese, 10 turkish, 0 = other.
pub const LANGUAGES: [&str; 11] = ["", "russian", "english", "german", "polish", "ukrainian", "french", "spanish", "chinese", "portuguese", "turkish"];

/// The language used when the index is 0, unknown, or the key has no text in that language.
pub const ENGLISH: u8 = 2;

#[derive(Debug, Clone, Default)]
pub struct ClientTexts {
    by_key: BTreeMap<String, BTreeMap<String, String>>,
}

impl ClientTexts {
    pub fn parse(json: &str) -> Result<Self, serde_json::Error> {
        Ok(ClientTexts { by_key: serde_json::from_str(json)? })
    }

    /// Texts already read: `{key: {language name: text}}`.
    pub fn from_map(by_key: BTreeMap<String, BTreeMap<String, String>>) -> Self {
        ClientTexts { by_key }
    }

    /// The text of `key` in the client language `language` (English for 0 / an unknown index / a missing language). A
    /// string that is not one of the keys (an operator's own offer text) is returned unchanged.
    pub fn text(&self, key: &str, language: u8) -> String {
        let Some(by_lang) = self.by_key.get(key) else { return key.to_string() };
        let name = LANGUAGES.get(usize::from(language)).copied().filter(|n| !n.is_empty()).unwrap_or(LANGUAGES[usize::from(ENGLISH)]);
        by_lang.get(name).or_else(|| by_lang.get(LANGUAGES[usize::from(ENGLISH)])).cloned().unwrap_or_else(|| key.to_string())
    }

    /// Whether `key` has its texts here.
    pub fn has(&self, key: &str) -> bool {
        self.by_key.contains_key(key)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_key_has_all_ten_languages_and_falls_back_to_english() {
        crate::require_tables!();
        let gd = crate::testing::tables();
        let t = &gd.texts;
        for key in ["st_special_offer", "st_premium_access_7d_voucher_league_desc", "st_gold_advert_1_hint", "st_contract_with_profiteer", "st_contract_complete_after_win_descr"] {
            assert!(t.has(key), "{key}");
            for i in 1..=10u8 {
                assert_ne!(t.text(key, i), key, "{key} in language {i}");
            }
        }
        assert_eq!(t.text("st_special_offer", 2), "Special offer");
        assert_eq!(t.text("st_contract_complete_after_win_descr", 1), "Выиграть 1 бой");
        assert_eq!(t.text("st_special_offer", 0), "Special offer", "other -> English");
        assert_eq!(t.text("st_special_offer", 99), "Special offer", "unknown index -> English");
        assert_eq!(t.text("Operator's own text", 1), "Operator's own text", "not a key: unchanged");
    }
}
