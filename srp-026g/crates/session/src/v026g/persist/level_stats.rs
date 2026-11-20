//! The rows an account has in `g26_level_stats`: its kills and deaths by
//! the equipment level of the matches ([`crate::v026g::level_stats`]).

use rusqlite::{Connection, params};

use crate::persist::PersistError;
use crate::v026g::level_stats::{KillsDeaths, LevelStats};

/// The schema step that adds the table. An account of before has no rows:
/// it counts as a new player at every level until its first rated match
/// (nothing of before is counted: `docs/notes/kd-balance.md`).
pub(super) const MIGRATION: &str = "
    CREATE TABLE g26_level_stats (
        account_id INTEGER NOT NULL REFERENCES g26_accounts(account_id) ON DELETE CASCADE,
        -- The match's equipment level: the highest gear level of its players.
        level      INTEGER NOT NULL,
        kills      INTEGER NOT NULL,
        deaths     INTEGER NOT NULL,
        PRIMARY KEY (account_id, level)
    );
    ";

/// The account's rows.
pub(super) fn load(connection: &Connection, account_id: u32) -> Result<LevelStats, PersistError> {
    let mut statement = connection.prepare_cached(
        "SELECT level, kills, deaths FROM g26_level_stats WHERE account_id = ?1 ORDER BY level",
    )?;
    let rows = statement.query_map([account_id], |row| {
        Ok((
            row.get::<_, u8>(0)?,
            KillsDeaths {
                kills: row.get(1)?,
                deaths: row.get(2)?,
            },
        ))
    })?;
    let rows: Vec<(u8, KillsDeaths)> = rows.collect::<Result<_, _>>()?;
    Ok(LevelStats::from_rows(rows))
}

/// Replaces the account's rows.
pub(super) fn save(
    connection: &Connection,
    account_id: u32,
    stats: &LevelStats,
) -> Result<(), PersistError> {
    connection.execute(
        "DELETE FROM g26_level_stats WHERE account_id = ?1",
        [account_id],
    )?;
    for &(level, row) in stats.rows() {
        connection.execute(
            "INSERT INTO g26_level_stats (account_id, level, kills, deaths)
             VALUES (?1, ?2, ?3, ?4)",
            params![account_id, level, row.kills, row.deaths],
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

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "srp-g26-level-stats-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn kd(kills: u32, deaths: u32) -> KillsDeaths {
        KillsDeaths { kills, deaths }
    }

    #[test]
    fn the_rows_come_back_as_they_were_saved() {
        let dir = temp_dir("round-trip");
        let mut alice = Account::new("Alice");
        alice.level_stats.played(7, kd(12, 5));
        alice.level_stats.played(3, kd(1, 4));
        {
            let mut repository = SqliteGameRepository::open(&dir.join(DB_FILE)).unwrap();
            repository.save(1, &alice).unwrap();
            repository.save(2, &Account::new("Bob")).unwrap();
            // A second save replaces the rows.
            alice.level_stats.played(7, kd(2, 1));
            repository.save(1, &alice).unwrap();
        }
        let mut repository = SqliteGameRepository::open(&dir.join(DB_FILE)).unwrap();
        let loaded = repository.load_all().unwrap();
        assert_eq!(loaded[0].1.level_stats.at(7), kd(14, 6));
        assert_eq!(loaded[0].1.level_stats.at(3), kd(1, 4));
        assert_eq!(loaded[0].1, alice);
        assert!(loaded[1].1.level_stats.rows().is_empty());
        drop(repository);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_accounts_of_before_the_step_have_none() {
        let dir = temp_dir("upgrade");
        let path = dir.join(DB_FILE);
        {
            // The tables as the step before left them, with an account.
            let connection = Connection::open(&path).unwrap();
            connection
                .execute_batch(
                    "CREATE TABLE g26_meta (key TEXT PRIMARY KEY, value INTEGER NOT NULL);",
                )
                .unwrap();
            let before = MIGRATIONS
                .iter()
                .position(|step| *step == MIGRATION)
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
                     ) VALUES (5, 'Carol', '', 4321, 5, 0, 900, 2, 118, 100, 1, 0, 3, 1, 0, 1);",
                )
                .unwrap();
        }
        let mut repository = SqliteGameRepository::open(&path).unwrap();
        assert_eq!(repository.version().unwrap(), MIGRATIONS.len() as i64);
        let loaded = repository.load_all().unwrap();
        let (id, mut account) = loaded.into_iter().next().unwrap();
        assert_eq!((id, account.rating), (5, 118));
        assert_eq!(account.level_stats, LevelStats::default());
        // And it is kept from then on.
        account.level_stats.played(4, kd(3, 3));
        repository.save(id, &account).unwrap();
        let loaded = repository.load_all().unwrap();
        assert_eq!(loaded[0].1.level_stats.at(4), kd(3, 3));
        drop(repository);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
