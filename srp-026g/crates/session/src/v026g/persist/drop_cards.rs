//! The rows an account has in `g26_drop_cards`: the matches whose results
//! screen it used a drop card on ([`crate::v026g::rewards::Results::use_drop_card`]).
//! They are written with the account, in the same transaction as the card
//! taken and the item given: a crash keeps all three or none, so no card is
//! taken twice for a match (`docs/notes/drop_cards.md`).

use rusqlite::{Connection, params};

use crate::persist::PersistError;

/// The schema step that adds the table. An account of before has no rows:
/// no card used on a match still kept (before it, nothing kept them).
pub(super) const MIGRATION: &str = "
    CREATE TABLE g26_drop_cards (
        account_id INTEGER NOT NULL REFERENCES g26_accounts(account_id) ON DELETE CASCADE,
        match_id   INTEGER NOT NULL,
        PRIMARY KEY (account_id, match_id)
    );
    ";

/// The account's matches, by id.
pub(super) fn load(connection: &Connection, account_id: u32) -> Result<Vec<u32>, PersistError> {
    let mut statement = connection.prepare_cached(
        "SELECT match_id FROM g26_drop_cards WHERE account_id = ?1 ORDER BY match_id",
    )?;
    let rows = statement.query_map([account_id], |row| row.get(0))?;
    let mut matches = Vec::new();
    for row in rows {
        matches.push(row?);
    }
    Ok(matches)
}

/// Replaces the account's rows.
pub(super) fn save(
    connection: &Connection,
    account_id: u32,
    matches: &[u32],
) -> Result<(), PersistError> {
    connection.execute(
        "DELETE FROM g26_drop_cards WHERE account_id = ?1",
        [account_id],
    )?;
    for &match_id in matches {
        connection.execute(
            "INSERT OR IGNORE INTO g26_drop_cards (account_id, match_id) VALUES (?1, ?2)",
            params![account_id, match_id],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::{GameRepository, SqliteGameRepository};
    use crate::persist::DB_FILE;
    use crate::v026g::account::Account;

    #[test]
    fn the_matches_come_back_with_the_account() {
        let dir = std::env::temp_dir().join(format!(
            "srp-g26-drop-cards-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut repository = SqliteGameRepository::open(&dir.join(DB_FILE)).unwrap();
        let mut account = Account::new("Alice");
        account.drop_cards = vec![0x62153e96, 0x48e3f076];
        repository.save(7, &account).unwrap();
        repository.save(8, &Account::new("Bob")).unwrap();
        let loaded = repository.load_all().unwrap();
        let alice = &loaded.iter().find(|(id, _)| *id == 7).unwrap().1;
        assert_eq!(alice.drop_cards, [0x48e3f076, 0x62153e96]);
        let bob = &loaded.iter().find(|(id, _)| *id == 8).unwrap().1;
        assert!(bob.drop_cards.is_empty());

        // A later save replaces them.
        account.drop_cards = vec![5];
        repository.save(7, &account).unwrap();
        let loaded = repository.load_all().unwrap();
        let alice = &loaded.iter().find(|(id, _)| *id == 7).unwrap().1;
        assert_eq!(alice.drop_cards, [5]);
        drop(repository);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
