//! The rows an account has in `g26_map_plays`: how many matches it was
//! put in on each map ([`crate::v026g::matchmaking`]).

use rusqlite::{Connection, params};

use crate::persist::PersistError;

/// The schema step that adds the table. An account of before has no rows:
/// it has played no map yet, as far as the matchmaker knows.
pub(super) const MIGRATION: &str = "
    CREATE TABLE g26_map_plays (
        account_id INTEGER NOT NULL REFERENCES g26_accounts(account_id) ON DELETE CASCADE,
        -- maps_dict's match_making_params.map_id: 1..8 are level_01..level_08.
        map        INTEGER NOT NULL,
        plays      INTEGER NOT NULL,
        PRIMARY KEY (account_id, map)
    );
    ";

/// The account's rows, by map.
pub(super) fn load(
    connection: &Connection,
    account_id: u32,
) -> Result<Vec<(u8, u32)>, PersistError> {
    let mut statement = connection.prepare_cached(
        "SELECT map, plays FROM g26_map_plays WHERE account_id = ?1 ORDER BY map",
    )?;
    let rows = statement.query_map([account_id], |row| Ok((row.get(0)?, row.get(1)?)))?;
    let mut plays = Vec::new();
    for row in rows {
        plays.push(row?);
    }
    Ok(plays)
}

/// Replaces the account's rows.
pub(super) fn save(
    connection: &Connection,
    account_id: u32,
    plays: &[(u8, u32)],
) -> Result<(), PersistError> {
    connection.execute(
        "DELETE FROM g26_map_plays WHERE account_id = ?1",
        [account_id],
    )?;
    for &(map, count) in plays {
        // One row a map: a list that names a map twice keeps the sum.
        connection.execute(
            "INSERT INTO g26_map_plays (account_id, map, plays) VALUES (?1, ?2, ?3)
             ON CONFLICT(account_id, map) DO UPDATE SET plays = plays + excluded.plays",
            params![account_id, map, count],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::super::{GameRepository, MIGRATIONS, SqliteGameRepository};
    use super::*;
    use crate::persist::DB_FILE;
    use crate::v026g::account::Account;
    use crate::v026g::matchmaking::{count_map_play, map_plays};

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "srp-g26-map-plays-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// The step of `MIGRATIONS` that is this file's, counted from 1.
    fn own_version() -> usize {
        MIGRATIONS
            .iter()
            .position(|step| *step == MIGRATION)
            .expect("the step is in the list")
            + 1
    }

    #[test]
    fn the_rows_come_back_as_they_were_saved() {
        let dir = temp_dir("round-trip");
        let mut repository = SqliteGameRepository::open(&dir.join(DB_FILE)).unwrap();
        let mut account = Account::new("Alice");
        for map in [3, 1, 3, 8, 3] {
            count_map_play(&mut account, map);
        }
        repository.save(7, &account).unwrap();
        repository.save(8, &Account::new("Bob")).unwrap();
        let loaded = repository.load_all().unwrap();
        let alice = &loaded.iter().find(|(id, _)| *id == 7).unwrap().1;
        assert_eq!(alice.map_plays, [(1, 1), (3, 3), (8, 1)]);
        assert_eq!(alice, &account);
        assert!(
            loaded
                .iter()
                .find(|(id, _)| *id == 8)
                .unwrap()
                .1
                .map_plays
                .is_empty()
        );

        // A later save replaces them.
        count_map_play(&mut account, 1);
        repository.save(7, &account).unwrap();
        let loaded = repository.load_all().unwrap();
        let alice = &loaded.iter().find(|(id, _)| *id == 7).unwrap().1;
        assert_eq!((map_plays(alice, 1), map_plays(alice, 3)), (2, 3));
        drop(repository);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_file_of_the_version_before_keeps_its_accounts() {
        let dir = temp_dir("migration");
        let path = dir.join(DB_FILE);
        let before = own_version() - 1;
        {
            // The file as the version before left it, with an account.
            let connection = Connection::open(&path).unwrap();
            connection
                .execute_batch(
                    "CREATE TABLE g26_meta (key TEXT PRIMARY KEY, value INTEGER NOT NULL);",
                )
                .unwrap();
            for step in &MIGRATIONS[..before] {
                connection.execute_batch(step).unwrap();
            }
            connection
                .execute(
                    "INSERT INTO g26_meta (key, value) VALUES ('schema', ?1)",
                    [before as i64],
                )
                .unwrap();
            connection
                .execute_batch(
                    "INSERT INTO g26_accounts (
                        account_id, name, clan_tag, generic_money, premium_money,
                        premium_until, exp_total, quest_faction, rating, rating_before,
                        active_kit, autobuy, next_item_id, next_quest_instance,
                        tutorial_step, tutorial_done
                     ) VALUES (7, 'Alice', 'TAG', 4321, 5, 0, 900, 2, 118, 100, 1, 0, 3, 1, 0, 1);
                     INSERT INTO g26_items (
                        account_id, item_id, seq, dict_id, condition_or_stack, repair_time,
                        reroll_time, modifications, upgrades
                     ) VALUES (7, 1, 0, 18, 10000, 0, 0, '', '');",
                )
                .unwrap();
            let tables: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE name = 'g26_map_plays'",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(tables, 0);
        }
        let mut repository = SqliteGameRepository::open(&path).unwrap();
        assert_eq!(repository.version().unwrap(), MIGRATIONS.len() as i64);
        let loaded = repository.load_all().unwrap();
        assert_eq!(loaded.len(), 1);
        let (id, mut account) = loaded.into_iter().next().unwrap();
        assert_eq!(
            (id, account.name.as_str(), account.clan_tag.as_str()),
            (7, "Alice", "TAG")
        );
        assert_eq!(
            (account.generic_money, account.exp_total, account.rating),
            (4321, 900, 118)
        );
        assert_eq!(account.inventory.len(), 1);
        assert_eq!(account.inventory[0].dict_id, 18);
        // It has played no map, as far as the matchmaker knows.
        assert!(account.map_plays.is_empty());
        // And the table takes its rows now.
        count_map_play(&mut account, 5);
        repository.save(7, &account).unwrap();
        let again = repository.load_all().unwrap();
        assert_eq!(again[0].1.map_plays, [(5, 1)]);
        assert_eq!(again[0].1, account);
        drop(repository);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
