//! An account's row in `g26_faces`: the face it chose
//! (`docs/notes/face-choice.md`). No row: face 0, every account's of before.

use rusqlite::{Connection, OptionalExtension, params};
use survarium::v026g::faces::{Face, Faces};

use crate::persist::PersistError;

/// The schema step that adds the table.
pub(super) const MIGRATION: &str = "
    CREATE TABLE g26_faces (
        account_id INTEGER PRIMARY KEY REFERENCES g26_accounts(account_id) ON DELETE CASCADE,
        face       INTEGER NOT NULL CHECK(face BETWEEN 0 AND 255)
    );
    ";

/// The account's face, by `faces` (the game data's): face 0 for none kept,
/// or one the data no longer lists (logged).
pub(super) fn load(
    connection: &Connection,
    account_id: u32,
    faces: Faces,
) -> Result<Face, PersistError> {
    let kept: Option<u32> = connection
        .query_row(
            "SELECT face FROM g26_faces WHERE account_id = ?1",
            [account_id],
            |row| row.get(0),
        )
        .optional()?;
    let Some(index) = kept else {
        return Ok(Face::DEFAULT);
    };
    Ok(faces.face(index).unwrap_or_else(|| {
        log::warn!(
            "account {account_id}: face {index} is not one of the game data's {}: face 0",
            faces.count()
        );
        Face::DEFAULT
    }))
}

/// Replaces the account's face; face 0 keeps no row.
pub(super) fn save(
    connection: &Connection,
    account_id: u32,
    face: Face,
) -> Result<(), PersistError> {
    if face == Face::DEFAULT {
        connection.execute("DELETE FROM g26_faces WHERE account_id = ?1", [account_id])?;
        return Ok(());
    }
    connection.execute(
        "INSERT INTO g26_faces (account_id, face) VALUES (?1, ?2)
         ON CONFLICT(account_id) DO UPDATE SET face = excluded.face",
        params![account_id, face.index()],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::{GameRepository, MIGRATIONS, SqliteGameRepository};
    use super::*;
    use crate::v026g::account::Account;

    #[test]
    fn the_accounts_of_before_the_step_have_face_0_and_keep_one_chosen() {
        let path = std::env::temp_dir().join(format!(
            "faces-upgrade-{}-{:?}.db",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_file(&path);
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
        assert_eq!((id, account.name.as_str(), account.face), (5, "Carol", Face::DEFAULT));
        // And a face chosen is kept from then on.
        account.face = Faces::shared().face(1).unwrap();
        repository.save(id, &account).unwrap();
        assert_eq!(repository.load_all().unwrap()[0].1.face, account.face);
        drop(repository);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_face_survives_a_reopen_and_an_unlisted_one_reads_as_face_0() {
        let path = std::env::temp_dir().join(format!(
            "faces-{}-{:?}.db",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_file(&path);
        let faces = Faces::shared();
        let mut account = Account::new("Face");
        assert_eq!(account.face, Face::DEFAULT);
        account.face = faces.face(2).unwrap();
        {
            let mut repository = SqliteGameRepository::open(&path).unwrap();
            repository.save(7, &account).unwrap();
        }
        let mut repository = SqliteGameRepository::open(&path).unwrap();
        let loaded = repository.load_all().unwrap();
        assert_eq!(loaded, vec![(7, account.clone())]);
        // Face 0 again: no row.
        account.face = Face::DEFAULT;
        repository.save(7, &account).unwrap();
        assert_eq!(repository.load_all().unwrap(), vec![(7, account)]);
        drop(repository);
        // A face the data does not list (the pack took faces away): face 0.
        let connection = Connection::open(&path).unwrap();
        connection
            .execute(
                "INSERT INTO g26_faces (account_id, face) VALUES (7, 200)",
                [],
            )
            .unwrap();
        assert_eq!(load(&connection, 7, faces).unwrap(), Face::DEFAULT);
        connection
            .execute("UPDATE g26_faces SET face = 1 WHERE account_id = 7", [])
            .unwrap();
        assert_eq!(load(&connection, 7, faces).unwrap().index(), 1);
        drop(connection);
        let _ = std::fs::remove_file(&path);
    }
}
