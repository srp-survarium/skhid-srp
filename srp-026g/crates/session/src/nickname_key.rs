//! A deliberately small nickname equivalence rule. Fold Unicode case and the
//! explicit Cyrillic/Latin lookalikes; preserve the actual display name. This
//! is not fuzzy matching or a login/password rewrite.

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use survarium::v026g::lobby::{cut, limits};

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

/// An account's names in a database from before nicknames were unique.
pub(crate) struct Named {
    pub id: u32,
    pub login: String,
    pub name: String,
}

/// A nickname changed so that its key belongs to one account.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Rename {
    pub id: u32,
    pub from: String,
    pub to: String,
}

/// The renames that leave every key to one account, for `accounts` in the
/// order of their ids: the lowest id keeps a shared name and each other
/// account becomes `<name>_<id>`, cut to the client's buffer, with a counter
/// after it while that is another account's name or login.
pub(crate) fn renames(accounts: &[Named]) -> Vec<Rename> {
    let mut taken: HashSet<NicknameKey> = accounts
        .iter()
        .flat_map(|account| [key(&account.login), key(&account.name)])
        .collect();
    let mut kept = HashSet::new();
    let mut renames = Vec::new();
    for account in accounts {
        if kept.insert(key(&account.name)) {
            continue;
        }
        let to = (0u32..)
            .map(|attempt| {
                let suffix = match attempt {
                    0 => format!("_{}", account.id),
                    _ => format!("_{}_{attempt}", account.id),
                };
                let name = cut(&account.name, limits::NAME - suffix.len());
                format!("{name}{suffix}")
            })
            .find(|to| !taken.contains(&key(to)))
            .expect("a counter no account's name ends with");
        taken.insert(key(&to));
        renames.push(Rename {
            id: account.id,
            from: account.name.clone(),
            to,
        });
    }
    renames
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
