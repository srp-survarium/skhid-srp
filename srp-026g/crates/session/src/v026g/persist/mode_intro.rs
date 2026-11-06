//! Persistent per-mode introduction allowances; old accounts start with all three.
use crate::persist::PersistError;
use crate::v026g::mode_intro::{Allowances, Mode};
use rusqlite::{Connection, params};

pub(super) const MIGRATION: &str = "
    CREATE TABLE g26_mode_introductions (
        account_id INTEGER NOT NULL REFERENCES g26_accounts(account_id) ON DELETE CASCADE,
        mode TEXT NOT NULL,
        shown INTEGER NOT NULL CHECK(shown BETWEEN 0 AND 3),
        PRIMARY KEY (account_id, mode)
    );";

pub(super) fn load(connection: &Connection, account_id: u32) -> Result<Allowances, PersistError> {
    let mut statement = connection.prepare_cached(
        "SELECT mode, shown FROM g26_mode_introductions WHERE account_id = ?1 ORDER BY mode",
    )?;
    let rows = statement.query_map([account_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, u8>(1)?))
    })?;
    let mut allowances = Allowances::default();
    for row in rows {
        let (key, shown) = row?;
        let mode = Mode::parse(&key)
            .ok_or_else(|| super::bad_row(account_id, "introduction mode", &key))?;
        allowances
            .insert(mode, shown)
            .ok_or_else(|| super::bad_row(account_id, "introductions shown", &shown.to_string()))?;
    }
    Ok(allowances)
}

pub(super) fn save(
    connection: &Connection,
    account_id: u32,
    state: &Allowances,
) -> Result<(), PersistError> {
    connection.execute(
        "DELETE FROM g26_mode_introductions WHERE account_id = ?1",
        [account_id],
    )?;
    for (mode, shown) in state.rows() {
        connection.execute(
            "INSERT INTO g26_mode_introductions (account_id, mode, shown) VALUES (?1, ?2, ?3)",
            params![account_id, mode.key(), shown],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::{GameRepository, SqliteGameRepository};
    use super::*;
    use crate::v026g::account::Account;
    #[test]
    fn blackout_allowance_survives_database_reopen() {
        let path = std::env::temp_dir().join(format!(
            "blackout-{}-{:?}.db",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_file(&path);
        let mut account = Account::new("Alice");
        account.mode_introductions.take(Mode::Blackout);
        {
            let mut db = SqliteGameRepository::open(&path).unwrap();
            db.save(7, &account).unwrap();
            db.save(8, &Account::new("Bob")).unwrap();
        }
        let mut db = SqliteGameRepository::open(&path).unwrap();
        let loaded = db.load_all().unwrap();
        assert_eq!(loaded.iter().find(|(id, _)| *id == 7).unwrap().1, account);
        assert_eq!(
            loaded
                .iter()
                .find(|(id, _)| *id == 8)
                .unwrap()
                .1
                .mode_introductions,
            Allowances::default()
        );
        drop(db);
        std::fs::remove_file(path).unwrap();
    }
}
