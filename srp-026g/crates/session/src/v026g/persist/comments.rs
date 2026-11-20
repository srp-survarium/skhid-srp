//! Durable feedback, independent of account saves and deletions.

use rusqlite::{Connection, params};
use survarium::v026g::lobby::requests::PlayerComment;

use crate::persist::PersistError;
use crate::v026g::comments::StoredComment;

pub(super) const MIGRATION: &str = "
    CREATE TABLE g26_player_comments (
        seq         INTEGER PRIMARY KEY AUTOINCREMENT,
        at          INTEGER NOT NULL CHECK (at >= 0),
        sender_id   INTEGER NOT NULL,
        sender_name TEXT NOT NULL,
        target_id   INTEGER NOT NULL,
        target_name TEXT NOT NULL,
        kind        INTEGER NOT NULL,
        reason      INTEGER NOT NULL,
        CHECK (sender_id != target_id),
        CHECK ((kind = 1 AND reason BETWEEN 1 AND 4)
            OR (kind = 2 AND reason BETWEEN 1 AND 3))
    );
    CREATE UNIQUE INDEX g26_player_comments_daily
        ON g26_player_comments (sender_id, target_id, kind, reason, (at / 86400));
    CREATE INDEX g26_player_comments_target ON g26_player_comments (target_id, seq);
";

pub(super) fn save(connection: &Connection, comment: &StoredComment) -> Result<(), PersistError> {
    let (kind, reason) = comment.comment.wire();
    // This write runs on the lobby thread. The account writer's mutex is
    // already held; a different SQLite connection must not make all players
    // wait through the repository's normal five-second busy timeout.
    let timeout: u64 = connection.pragma_query_value(None, "busy_timeout", |row| row.get(0))?;
    connection.busy_timeout(std::time::Duration::ZERO)?;
    let written = connection.execute(
        "INSERT INTO g26_player_comments
            (at, sender_id, sender_name, target_id, target_name, kind, reason)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT (sender_id, target_id, kind, reason, (at / 86400)) DO NOTHING",
        params![
            comment.at,
            comment.sender_id,
            comment.sender_name,
            comment.target_id,
            comment.target_name,
            kind,
            reason,
        ],
    );
    // Restore the repository policy even when the INSERT fails.
    let restored = connection.busy_timeout(std::time::Duration::from_millis(timeout));
    written?;
    restored?;
    Ok(())
}

pub(super) fn load(
    connection: &Connection,
    target: u32,
    limit: usize,
) -> Result<Vec<StoredComment>, PersistError> {
    let mut statement = connection.prepare_cached(
        "SELECT at, sender_id, sender_name, target_id, target_name, kind, reason
         FROM g26_player_comments WHERE target_id = ?1 ORDER BY seq DESC LIMIT ?2",
    )?;
    let rows = statement.query_map(
        params![target, i64::try_from(limit).unwrap_or(i64::MAX)],
        |row| {
            let kind = row.get(5)?;
            let reason = row.get(6)?;
            let comment =
                PlayerComment::from_wire(kind, reason).ok_or(rusqlite::Error::InvalidQuery)?;
            Ok(StoredComment {
                at: row.get(0)?,
                sender_id: row.get(1)?,
                sender_name: row.get(2)?,
                target_id: row.get(3)?,
                target_name: row.get(4)?,
                comment,
            })
        },
    )?;
    Ok(rows.collect::<Result<_, _>>()?)
}

#[cfg(test)]
mod tests {
    use super::super::{GameRepository, MIGRATIONS, MemoryGameRepository, SqliteGameRepository};
    use super::*;

    fn sample() -> StoredComment {
        StoredComment {
            at: 86_400,
            sender_id: 7,
            sender_name: "Alice".into(),
            target_id: 8,
            target_name: "Боб".into(),
            comment: PlayerComment::Friendly,
        }
    }

    fn choices(repository: &mut dyn GameRepository) {
        let first = sample();
        repository.save_player_comment(&first).unwrap();
        let mut renamed = first.clone();
        renamed.at += 20;
        renamed.target_name = "New name".into();
        repository.save_player_comment(&renamed).unwrap();
        assert_eq!(
            repository.player_comments(8, 10).unwrap(),
            std::slice::from_ref(&first)
        );
        let mut tomorrow = renamed.clone();
        tomorrow.at += 86_400;
        repository.save_player_comment(&tomorrow).unwrap();
        let mut report = tomorrow.clone();
        report.comment = PlayerComment::Swearing;
        repository.save_player_comment(&report).unwrap();
        let mut another_sender = report.clone();
        another_sender.sender_id = 9;
        repository.save_player_comment(&another_sender).unwrap();
        assert_eq!(
            repository.player_comments(8, 10).unwrap(),
            [another_sender.clone(), report, tomorrow, first]
        );
        assert_eq!(repository.player_comments(8, 1).unwrap(), [another_sender]);
        assert!(repository.player_comments(7, 10).unwrap().is_empty());
        assert!(repository.player_comments(8, 0).unwrap().is_empty());
    }

    #[test]
    fn player_comments_memory_has_the_same_daily_deduplication() {
        choices(&mut MemoryGameRepository::default());
    }

    #[test]
    fn player_comments_migrate_survive_reopen_and_keep_names() {
        let path = std::env::temp_dir().join(format!(
            "srp-player-comments-{}-{:?}.db",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_file(&path);
        // Build the schema immediately before feedback, with a real account.
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch("CREATE TABLE g26_meta (key TEXT PRIMARY KEY, value INTEGER NOT NULL);")
            .unwrap();
        let own = MIGRATIONS
            .iter()
            .position(|step| *step == MIGRATION)
            .unwrap();
        for step in &MIGRATIONS[..own] {
            connection.execute_batch(step).unwrap();
        }
        connection
            .execute("INSERT INTO g26_meta VALUES ('schema', ?1)", [own as i64])
            .unwrap();
        super::super::write_account(&connection, 8, &crate::v026g::Account::new("Боб")).unwrap();
        drop(connection);

        let mut repository = SqliteGameRepository::open(&path).unwrap();
        assert_eq!(repository.load_all().unwrap()[0].1.name, "Боб");
        let blocker = Connection::open(&path).unwrap();
        blocker.execute_batch("BEGIN IMMEDIATE;").unwrap();
        let started = std::time::Instant::now();
        assert!(matches!(
            repository.save_player_comment(&sample()),
            Err(PersistError::Sqlite(rusqlite::Error::SqliteFailure(error, _)))
                if error.code == rusqlite::ErrorCode::DatabaseBusy
        ));
        assert!(started.elapsed() < std::time::Duration::from_secs(1));
        let timeout: u64 = repository
            .connection
            .pragma_query_value(None, "busy_timeout", |row| row.get(0))
            .unwrap();
        assert_eq!(timeout, 5000, "other repository writes keep their policy");
        assert!(repository.player_comments(8, 10).unwrap().is_empty());
        blocker.execute_batch("ROLLBACK;").unwrap();
        drop(blocker);
        choices(&mut repository);
        let expected = repository.player_comments(8, 10).unwrap();
        // Saving/renaming the account does not replace its feedback.
        repository
            .save(8, &crate::v026g::Account::new("Renamed"))
            .unwrap();
        drop(repository);
        let mut repository = SqliteGameRepository::open(&path).unwrap();
        assert_eq!(repository.player_comments(8, 10).unwrap(), expected);
        repository.save_player_comment(&sample()).unwrap();
        assert_eq!(repository.player_comments(8, 10).unwrap(), expected);
        drop(repository);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn player_comments_sql_errors_are_not_success_and_can_be_retried() {
        let connection = Connection::open_in_memory().unwrap();
        connection.execute_batch(MIGRATION).unwrap();
        connection.execute_batch("CREATE TRIGGER refuse BEFORE INSERT ON g26_player_comments BEGIN SELECT RAISE(ABORT, 'disk full'); END;").unwrap();
        assert!(save(&connection, &sample()).is_err());
        assert!(load(&connection, 8, 10).unwrap().is_empty());
        connection.execute_batch("DROP TRIGGER refuse;").unwrap();
        save(&connection, &sample()).unwrap();
        assert_eq!(load(&connection, 8, 10).unwrap(), [sample()]);
    }
}
