//! An account's rows in `g26_scope_reticles`: the reticle it chose for each of its scope items
//! (`reticle-choice.md`, [`ScopeReticles`]). No row: the default. A save writes the account's
//! choices whole, so a scope item that went (sold) loses its row with the save that follows.

use rusqlite::{Connection, params};
use survarium::v026g::reticles::{Numbers, Reticles};

use super::super::account::Item;
use super::super::reticles::ScopeReticles;
use crate::persist::PersistError;

/// The schema step that adds the table.
pub(super) const MIGRATION: &str = "
    CREATE TABLE g26_scope_reticles (
        account_id INTEGER NOT NULL REFERENCES g26_accounts(account_id) ON DELETE CASCADE,
        item_id    INTEGER NOT NULL,
        pattern    INTEGER NOT NULL CHECK(pattern BETWEEN 0 AND 255),
        size       INTEGER NOT NULL CHECK(size BETWEEN 0 AND 255),
        colour     INTEGER NOT NULL CHECK(colour BETWEEN 0 AND 255),
        brightness INTEGER NOT NULL CHECK(brightness BETWEEN 0 AND 255),
        PRIMARY KEY (account_id, item_id)
    );
    ";

/// The account's choices, held against `inventory`'s scope items' lists in `reticles`
/// ([`ScopeReticles::loaded`]: one they no longer hold reads as the default).
pub(super) fn load(
    connection: &Connection,
    account_id: u32,
    inventory: &[Item],
    reticles: &Reticles,
) -> Result<ScopeReticles, PersistError> {
    let mut statement = connection.prepare_cached(
        "SELECT item_id, pattern, size, colour, brightness FROM g26_scope_reticles
         WHERE account_id = ?1 ORDER BY item_id",
    )?;
    let rows = statement.query_map([account_id], |row| {
        Ok((
            row.get::<_, u32>(0)?,
            Numbers {
                pattern: row.get(1)?,
                size: row.get(2)?,
                colour: row.get(3)?,
                brightness: row.get(4)?,
            },
        ))
    })?;
    let kept = rows.collect::<Result<Vec<_>, _>>()?;
    Ok(ScopeReticles::loaded(account_id, kept, inventory, reticles))
}

/// Replaces the account's rows with its choices.
pub(super) fn save(
    connection: &Connection,
    account_id: u32,
    reticles: &ScopeReticles,
) -> Result<(), PersistError> {
    connection.execute(
        "DELETE FROM g26_scope_reticles WHERE account_id = ?1",
        [account_id],
    )?;
    for (item_id, reticle) in reticles.chosen() {
        let numbers = reticle.numbers();
        connection.execute(
            "INSERT INTO g26_scope_reticles (account_id, item_id, pattern, size, colour, brightness)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                account_id,
                item_id,
                numbers.pattern,
                numbers.size,
                numbers.colour,
                numbers.brightness
            ],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::{GameRepository, MIGRATIONS, SqliteGameRepository};
    use super::*;
    use crate::v026g::account::Account;
    use survarium::v026g::lobby::FULL_CONDITION;
    use survarium::v026g::reticles::fixtures;

    /// 0.26g's Leupold, a scope item of the test data.
    const LEUPOLD: u16 = 69;

    fn temp(name: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "reticles-{name}-{}-{:?}.db",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_file(&path);
        path
    }

    #[test]
    fn an_account_of_before_the_step_has_none() {
        let path = temp("upgrade");
        {
            let connection = Connection::open(&path).unwrap();
            connection
                .execute_batch("CREATE TABLE g26_meta (key TEXT PRIMARY KEY, value INTEGER NOT NULL);")
                .unwrap();
            let before = MIGRATIONS.iter().position(|step| *step == MIGRATION).unwrap();
            for step in &MIGRATIONS[..before] {
                connection.execute_batch(step).unwrap();
            }
            connection
                .execute(
                    "INSERT INTO g26_meta (key, value) VALUES ('schema', ?1)",
                    [before as i64],
                )
                .unwrap();
        }
        let mut repository = SqliteGameRepository::open(&path).unwrap();
        assert_eq!(repository.version().unwrap(), MIGRATIONS.len() as i64);
        let mut account = Account::new("Old");
        account.add_item(LEUPOLD, FULL_CONDITION);
        repository.save(3, &account).unwrap();
        let loaded = repository.load_all().unwrap();
        assert_eq!(loaded[0].1.reticles.chosen().count(), 0);
        drop(repository);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_choice_survives_a_save_and_a_sold_scope_loses_its_row() {
        let path = temp("rows");
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE g26_accounts (account_id INTEGER PRIMARY KEY);
                 INSERT INTO g26_accounts VALUES (7);",
            )
            .unwrap();
        connection.execute_batch(MIGRATION).unwrap();
        let reticles = fixtures::reticles(&[LEUPOLD]);
        let mut account = Account::new("Rows");
        let scope = account.add_item(LEUPOLD, FULL_CONDITION);
        let other = account.add_item(LEUPOLD, FULL_CONDITION);
        let lists = reticles.of(LEUPOLD).unwrap();
        let cross = lists
            .reticle(Numbers { pattern: 1, size: 3, colour: 1, brightness: 0 })
            .unwrap();
        account.reticles.choose(scope, cross);
        save(&connection, 7, &account.reticles).unwrap();
        let loaded = load(&connection, 7, &account.inventory, &reticles).unwrap();
        assert_eq!(loaded, account.reticles);
        assert_eq!(loaded.of(other), survarium::v026g::reticles::Reticle::DEFAULT);
        // Sold: the next save writes no row for it.
        account.inventory.retain(|item| item.item_id != scope);
        account.sync_kits();
        save(&connection, 7, &account.reticles).unwrap();
        let rows: i64 = connection
            .query_row("SELECT COUNT(*) FROM g26_scope_reticles", [], |row| row.get(0))
            .unwrap();
        assert_eq!(rows, 0);
        // A row the lists no longer hold (a release took the cross away): the default.
        connection
            .execute(
                "INSERT INTO g26_scope_reticles VALUES (7, ?1, 5, 0, 0, 6)",
                [other],
            )
            .unwrap();
        let loaded = load(&connection, 7, &account.inventory, &reticles).unwrap();
        assert_eq!(loaded.chosen().count(), 0);
        drop(connection);
        let _ = std::fs::remove_file(&path);
    }
}
