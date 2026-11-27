//! Migration 13's table, `g26_mode_introductions`: an account's matches of
//! each mode the hud introduces with a notice, up to three
//! ([`crate::v026g::mode_intro`]). A row's `shown` is that count: the
//! notice is shown in the matches it counts. The rows of revision 45 (the
//! notices its HUD asked for, `blackout`) read the same way. An account
//! with no row of a mode has had none of it.

use rusqlite::{Connection, params};

use crate::persist::PersistError;
use crate::v026g::mode_intro::Introductions;
use survarium::v026g::maps::ModeNotice;

pub(super) const MIGRATION: &str = "
    CREATE TABLE g26_mode_introductions (
        account_id INTEGER NOT NULL REFERENCES g26_accounts(account_id) ON DELETE CASCADE,
        mode TEXT NOT NULL,
        shown INTEGER NOT NULL CHECK(shown BETWEEN 0 AND 3),
        PRIMARY KEY (account_id, mode)
    );";

/// The account's rows.
pub(super) fn load(connection: &Connection, account_id: u32) -> Result<Introductions, PersistError> {
    let mut statement = connection.prepare_cached(
        "SELECT mode, shown FROM g26_mode_introductions WHERE account_id = ?1 ORDER BY mode",
    )?;
    let rows = statement.query_map([account_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, u8>(1)?))
    })?;
    let mut introductions = Introductions::default();
    for row in rows {
        let (key, shown) = row?;
        let mode = ModeNotice::of_key(&key)
            .ok_or_else(|| super::bad_row(account_id, "introduction mode", &key))?;
        introductions
            .insert(mode, shown)
            .ok_or_else(|| super::bad_row(account_id, "introductions shown", &shown.to_string()))?;
    }
    Ok(introductions)
}

/// Replaces the account's rows.
pub(super) fn save(
    connection: &Connection,
    account_id: u32,
    introductions: &Introductions,
) -> Result<(), PersistError> {
    connection.execute(
        "DELETE FROM g26_mode_introductions WHERE account_id = ?1",
        [account_id],
    )?;
    for (mode, shown) in introductions.rows() {
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
    use crate::v026g::account::Account;
    use survarium::v026g::maps::GameMode;
    use survarium::v026g::match_msgs::static_info::Notice;

    /// The counts survive a reopen, each mode its own; an account of none
    /// reads none.
    #[test]
    fn mode_counts_survive_a_reopen() {
        let path = std::env::temp_dir().join(format!(
            "mode-intro-{}-{:?}.db",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_file(&path);
        let mut account = Account::new("Alice");
        for _ in 0..3 {
            account.mode_introductions.count(GameMode::Blackout);
        }
        account.mode_introductions.count(GameMode::FreezeTag);
        {
            let mut db = SqliteGameRepository::open(&path).unwrap();
            db.save(7, &account).unwrap();
            db.save(8, &Account::new("Bob")).unwrap();
        }
        let mut db = SqliteGameRepository::open(&path).unwrap();
        let loaded = db.load_all().unwrap();
        let alice = &loaded.iter().find(|(id, _)| *id == 7).unwrap().1;
        assert_eq!(alice, &account);
        assert_eq!(alice.mode_introductions.notice(GameMode::Blackout), Notice::Skipped);
        assert_eq!(alice.mode_introductions.notice(GameMode::FreezeTag), Notice::Shown);
        let bob = &loaded.iter().find(|(id, _)| *id == 8).unwrap().1;
        assert_eq!(bob.mode_introductions, Default::default());
        drop(db);
        std::fs::remove_file(path).unwrap();
    }
}
