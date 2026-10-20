//! The rows of `g26_admin_actions`: the audit trail of the admin's account
//! moderation ([`AdminAction`], `docs/notes/account-moderation.md`). Rows
//! are only added: none is changed or deleted by srp. They name the account
//! by id without a foreign key, so they stay whatever becomes of it.

use rusqlite::{Connection, params};

use crate::persist::PersistError;
use crate::v026g::moderation::AdminAction;

/// The schema step that adds the table.
pub(super) const MIGRATION: &str = "
    CREATE TABLE g26_admin_actions (
        seq        INTEGER PRIMARY KEY AUTOINCREMENT,
        at         INTEGER NOT NULL,
        admin      TEXT NOT NULL,
        account_id INTEGER NOT NULL,
        account    TEXT NOT NULL,
        kind       TEXT NOT NULL,
        change     TEXT NOT NULL,
        reason     TEXT NOT NULL,
        before     TEXT NOT NULL,
        after      TEXT NOT NULL
    );
    CREATE INDEX g26_admin_actions_by_account ON g26_admin_actions (account_id, seq);
    ";

/// Adds one row.
pub(super) fn save(connection: &Connection, action: &AdminAction) -> Result<(), PersistError> {
    connection.execute(
        "INSERT INTO g26_admin_actions
            (at, admin, account_id, account, kind, change, reason, before, after)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            action.at.min(i64::MAX as u64) as i64,
            action.admin,
            action.account_id,
            action.account,
            action.kind,
            action.change,
            action.reason,
            action.before,
            action.after,
        ],
    )?;
    Ok(())
}

/// The account's latest `limit` rows, the newest first.
pub(super) fn load(
    connection: &Connection,
    account_id: u32,
    limit: usize,
) -> Result<Vec<AdminAction>, PersistError> {
    let mut statement = connection.prepare_cached(
        "SELECT at, admin, account_id, account, kind, change, reason, before, after
         FROM g26_admin_actions WHERE account_id = ?1 ORDER BY seq DESC LIMIT ?2",
    )?;
    let rows = statement.query_map(
        params![account_id, i64::try_from(limit).unwrap_or(i64::MAX)],
        |row| {
            Ok(AdminAction {
                at: row.get::<_, i64>(0)?.max(0) as u64,
                admin: row.get(1)?,
                account_id: row.get(2)?,
                account: row.get(3)?,
                kind: row.get(4)?,
                change: row.get(5)?,
                reason: row.get(6)?,
                before: row.get(7)?,
                after: row.get(8)?,
            })
        },
    )?;
    let mut actions = Vec::new();
    for row in rows {
        actions.push(row?);
    }
    Ok(actions)
}

#[cfg(test)]
mod tests {
    use super::super::{GameRepository, MIGRATIONS, SqliteGameRepository};
    use super::*;
    use crate::persist::DB_FILE;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "srp-g26-admin-actions-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn action(account_id: u32, at: u64, change: &str) -> AdminAction {
        AdminAction {
            at,
            admin: "203.0.113.1".into(),
            account_id,
            account: format!("player{account_id}"),
            kind: "money".into(),
            change: change.into(),
            reason: "farmed".into(),
            before: "10 silver, 0 gold".into(),
            after: "0 silver, 0 gold".into(),
        }
    }

    #[test]
    fn the_rows_outlive_the_repository_newest_first() {
        let dir = temp_dir("round-trip");
        let path = dir.join(DB_FILE);
        let mut repository = SqliteGameRepository::open(&path).unwrap();
        assert!(repository.admin_actions(7, 10).unwrap().is_empty());
        for (at, change) in [(1, "first"), (2, "second"), (3, "third")] {
            repository
                .save_admin_action(&action(7, at, change))
                .unwrap();
        }
        repository
            .save_admin_action(&action(8, 4, "other"))
            .unwrap();
        drop(repository);

        let mut repository = SqliteGameRepository::open(&path).unwrap();
        let rows = repository.admin_actions(7, 2).unwrap();
        assert_eq!(rows, [action(7, 3, "third"), action(7, 2, "second")]);
        assert_eq!(repository.admin_actions(8, 10).unwrap().len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A database made before this step gets the table, empty, and keeps
    /// everything it had.
    #[test]
    fn a_database_of_before_gets_the_table() {
        let dir = temp_dir("upgrade");
        let path = dir.join(DB_FILE);
        let own = MIGRATIONS
            .iter()
            .position(|step| *step == MIGRATION)
            .expect("the step is in the list");
        {
            let connection = Connection::open(&path).unwrap();
            connection
                .execute_batch(
                    "CREATE TABLE g26_meta (key TEXT PRIMARY KEY, value INTEGER NOT NULL);",
                )
                .unwrap();
            for step in &MIGRATIONS[..own] {
                connection.execute_batch(step).unwrap();
            }
            connection
                .execute(
                    "INSERT INTO g26_meta (key, value) VALUES ('schema', ?1)",
                    [own as i64],
                )
                .unwrap();
        }
        let mut repository = SqliteGameRepository::open(&path).unwrap();
        repository
            .save(3, &crate::v026g::Account::new("Kept"))
            .unwrap();
        drop(repository);
        let mut repository = SqliteGameRepository::open(&path).unwrap();
        assert_eq!(repository.version().unwrap(), MIGRATIONS.len() as i64);
        assert_eq!(repository.load_all().unwrap().len(), 1);
        assert!(repository.admin_actions(3, 10).unwrap().is_empty());
        repository.save_admin_action(&action(3, 1, "x")).unwrap();
        assert_eq!(repository.admin_actions(3, 10).unwrap().len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
