//! The row an account has in `g26_room_settings`: the settings of the last
//! room it made by hand ([`crate::v026g::room_settings`]).

use rusqlite::{Connection, OptionalExtension, params};

use super::bad_row;
use crate::persist::PersistError;
use crate::v026g::room_settings::{RoomSettings, Tile};

/// The schema step that adds the table. An account of before has no row:
/// its create window starts with the defaults.
pub(super) const MIGRATION: &str = "
    CREATE TABLE g26_room_settings (
        account_id INTEGER PRIMARY KEY REFERENCES g26_accounts(account_id) ON DELETE CASCADE,
        size       INTEGER NOT NULL,
        min_side   INTEGER NOT NULL,
        max_level  INTEGER NOT NULL,
        wait       INTEGER NOT NULL,
        -- `level:weather` tiles, `,` between them.
        tiles_off  TEXT NOT NULL,
        tiles_on   TEXT NOT NULL,
        -- Modes, `,` between them.
        modes_off  TEXT NOT NULL
    );
    ";

fn tiles_text(tiles: &[Tile]) -> String {
    tiles
        .iter()
        .map(|(level, weather)| format!("{level}:{weather}"))
        .collect::<Vec<_>>()
        .join(",")
}

fn modes_text(modes: &[u8]) -> String {
    modes
        .iter()
        .map(u8::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

fn parse_tiles(account_id: u32, text: &str) -> Result<Vec<Tile>, PersistError> {
    text.split(',')
        .filter(|tile| !tile.is_empty())
        .map(|tile| {
            let bad = || bad_row(account_id, "room tiles", text);
            let (level, weather) = tile.split_once(':').ok_or_else(bad)?;
            Ok((
                level.parse().map_err(|_| bad())?,
                weather.parse().map_err(|_| bad())?,
            ))
        })
        .collect()
}

fn parse_modes(account_id: u32, text: &str) -> Result<Vec<u8>, PersistError> {
    text.split(',')
        .filter(|mode| !mode.is_empty())
        .map(|mode| {
            mode.parse()
                .map_err(|_| bad_row(account_id, "room modes", text))
        })
        .collect()
}

/// The account's settings; none without a row.
pub(super) fn load(
    connection: &Connection,
    account_id: u32,
) -> Result<Option<RoomSettings>, PersistError> {
    let mut statement = connection.prepare_cached(
        "SELECT size, min_side, max_level, wait, tiles_off, tiles_on, modes_off
         FROM g26_room_settings WHERE account_id = ?1",
    )?;
    let row = statement
        .query_row([account_id], |row| {
            Ok((
                row.get::<_, u8>(0)?,
                row.get::<_, u8>(1)?,
                row.get::<_, u8>(2)?,
                row.get::<_, u8>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
            ))
        })
        .optional()?;
    let Some((size, min_side, max_level, wait, tiles_off, tiles_on, modes_off)) = row else {
        return Ok(None);
    };
    Ok(Some(RoomSettings {
        size,
        min_side,
        max_level,
        wait,
        tiles_off: parse_tiles(account_id, &tiles_off)?,
        tiles_on: parse_tiles(account_id, &tiles_on)?,
        modes_off: parse_modes(account_id, &modes_off)?,
    }))
}

/// Replaces the account's row (none: no row).
pub(super) fn save(
    connection: &Connection,
    account_id: u32,
    settings: Option<&RoomSettings>,
) -> Result<(), PersistError> {
    let Some(settings) = settings else {
        connection.execute(
            "DELETE FROM g26_room_settings WHERE account_id = ?1",
            [account_id],
        )?;
        return Ok(());
    };
    connection.execute(
        "INSERT INTO g26_room_settings
            (account_id, size, min_side, max_level, wait, tiles_off, tiles_on, modes_off)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(account_id) DO UPDATE SET
            size = excluded.size, min_side = excluded.min_side,
            max_level = excluded.max_level, wait = excluded.wait,
            tiles_off = excluded.tiles_off, tiles_on = excluded.tiles_on,
            modes_off = excluded.modes_off",
        params![
            account_id,
            settings.size,
            settings.min_side,
            settings.max_level,
            settings.wait,
            tiles_text(&settings.tiles_off),
            tiles_text(&settings.tiles_on),
            modes_text(&settings.modes_off),
        ],
    )?;
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
            "srp-g26-room-settings-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn settings() -> RoomSettings {
        RoomSettings {
            size: 10,
            min_side: 3,
            max_level: 12,
            wait: 60,
            tiles_off: vec![(1, 1), (6, 5)],
            tiles_on: Vec::new(),
            modes_off: vec![2, 3],
        }
    }

    #[test]
    fn settings_come_back_are_replaced_and_go() {
        let dir = temp_dir("round-trip");
        let mut kept = Account::new("Alice");
        kept.room_settings = Some(settings());
        {
            let mut repository = SqliteGameRepository::open(&dir.join(DB_FILE)).unwrap();
            repository.save(1, &kept).unwrap();
            repository.save(2, &Account::new("Bob")).unwrap();
        }
        // Across a restart.
        let mut repository = SqliteGameRepository::open(&dir.join(DB_FILE)).unwrap();
        let loaded = repository.load_all().unwrap();
        assert_eq!(loaded[0].1.room_settings, Some(settings()));
        assert_eq!(loaded[1].1.room_settings, None);
        // A second room's settings replace the first's.
        let next = RoomSettings {
            size: 6,
            tiles_off: Vec::new(),
            tiles_on: vec![(13, 1)],
            modes_off: Vec::new(),
            ..settings()
        };
        kept.room_settings = Some(next.clone());
        repository.save(1, &kept).unwrap();
        assert_eq!(
            repository.load_all().unwrap()[0].1.room_settings,
            Some(next)
        );
        kept.room_settings = None;
        repository.save(1, &kept).unwrap();
        assert_eq!(repository.load_all().unwrap()[0].1.room_settings, None);
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
        let (id, mut account) = repository.load_all().unwrap().into_iter().next().unwrap();
        assert_eq!(
            (id, account.generic_money, &account.room_settings),
            (5, 4321, &None)
        );
        // And it keeps them from then on.
        account.room_settings = Some(settings());
        repository.save(id, &account).unwrap();
        assert_eq!(
            repository.load_all().unwrap()[0].1.room_settings,
            Some(settings())
        );
        drop(repository);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_broken_row_is_said_not_taken() {
        assert!(parse_tiles(1, "1:2,x").is_err());
        assert!(parse_tiles(1, "12").is_err());
        assert!(parse_modes(1, "2,300").is_err());
        assert_eq!(parse_tiles(1, "").unwrap(), []);
        assert_eq!(parse_tiles(1, "3:5,10:1").unwrap(), [(3, 5), (10, 1)]);
    }
}
