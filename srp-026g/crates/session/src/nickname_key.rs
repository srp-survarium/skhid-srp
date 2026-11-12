//! A deliberately small nickname equivalence rule, shared with the repair tool.
//! Fold Unicode case and the explicit Cyrillic/Latin lookalikes; preserve the
//! actual display name. This is not fuzzy matching or a login/password rewrite.

use std::collections::HashMap;
use std::sync::OnceLock;

const RULES: &str = include_str!("../../../scripts/nickname-lookalikes.tsv");

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct NicknameKey(String);

pub(crate) fn key(name: &str) -> NicknameKey {
    static MAP: OnceLock<HashMap<char, char>> = OnceLock::new();
    let map = MAP.get_or_init(|| {
        RULES
            .lines()
            .filter(|line| !line.starts_with('#') && !line.is_empty())
            .map(|line| {
                let (from, to) = line.split_once('\t').expect("nickname lookalike pair");
                let mut source = from.chars();
                let mut target = to.chars();
                let pair = (
                    source.next().expect("source character"),
                    target.next().expect("target character"),
                );
                assert!(
                    source.next().is_none() && target.next().is_none(),
                    "one character per rule"
                );
                pair
            })
            .collect()
    });
    NicknameKey(
        name.to_lowercase()
            .chars()
            .map(|c| map.get(&c).copied().unwrap_or(c))
            .collect(),
    )
}

pub(crate) fn sqlite_collation(connection: &rusqlite::Connection) -> rusqlite::Result<()> {
    connection.create_collation("skhid_nickname", |a, b| key(a).cmp(&key(b)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cyrillic_lookalikes_and_unicode_case_cannot_disguise_a_nickname() {
        for (a, b) in [
            ("sovspiranha", "sоvspiranha"),
            ("SOVSPIRANHA", "SОVSPIRANHA"),
            ("Сталкер", "сталкер"),
            ("Mamoru", "Маmоru"),
            ("Player", "Рlауеr"),
        ] {
            assert_eq!(key(a), key(b), "{a} / {b}");
        }
        for line in RULES
            .lines()
            .filter(|line| !line.starts_with('#') && !line.is_empty())
        {
            let (a, b) = line.split_once('\t').unwrap();
            assert_eq!(key(a), key(b));
            assert_eq!(key(&a.to_uppercase()), key(b));
        }
    }

    #[test]
    fn distinct_spelling_digits_and_punctuation_remain_distinct() {
        for other in [
            "sovspiranhia",
            "sovspiranha2",
            "sov_spiranha",
            "s0vspiranha",
        ] {
            assert_ne!(key("sovspiranha"), key(other));
        }
    }
}
