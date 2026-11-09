//! The row an account has in `g26_extras`: the daily bonus
//! ([`crate::v026g::extras`]). Its `auto_repair` column (the stock
//! auto-repair box) and `tutorial_flags` stay, unused: srp has no repair
//! (the owner, 2026-11-08) and no tutorial (2026-10-19); a new row takes
//! their default, 0.

use rusqlite::{Connection, OptionalExtension, params};
use survarium::v026g::lobby::LiveWatch;

use super::bad_row;
use crate::persist::PersistError;
use crate::v026g::extras::{DailyBonus, Extras};

/// The schema step that adds the table. An account of before keeps its
/// place in the tutorial: a nickname, the step it reported, and whether it
/// skipped (`tutorial::flag`: bits 0, 1..=4 and 5).
pub(super) const MIGRATION: &str = "
    CREATE TABLE g26_extras (
        account_id         INTEGER PRIMARY KEY
                           REFERENCES g26_accounts(account_id) ON DELETE CASCADE,
        auto_repair        INTEGER NOT NULL DEFAULT 0,
        tutorial_flags     INTEGER NOT NULL DEFAULT 0,
        bonus_day          INTEGER NOT NULL DEFAULT 0,
        bonus_last_day     INTEGER NOT NULL DEFAULT 0,
        -- Two bits a day of the run.
        bonus_wins         INTEGER NOT NULL DEFAULT 0,
        bonus_reward_dict  INTEGER NOT NULL DEFAULT 0,
        bonus_reward_count INTEGER NOT NULL DEFAULT 0,
        bonus_completed    INTEGER NOT NULL DEFAULT 0,
        bonus_shown_day    INTEGER NOT NULL DEFAULT 0
    );
    INSERT INTO g26_extras (account_id, tutorial_flags)
        SELECT account_id,
               1
               | CASE WHEN tutorial_step BETWEEN 1 AND 4 THEN 1 << tutorial_step ELSE 0 END
               | CASE WHEN tutorial_done <> 0 THEN 32 ELSE 0 END
        FROM g26_accounts;
    ";

/// The step that adds the live watch setting: every account of before may be
/// watched, as one saved later without a row is
/// ([`survarium::v026g::lobby::LiveWatch::id`]: 1 allowed, 2 hidden).
pub(super) const LIVE_WATCH_MIGRATION: &str = "
    ALTER TABLE g26_extras ADD COLUMN
        live_watch INTEGER NOT NULL DEFAULT 1 CHECK(live_watch IN (1, 2));
    ";

/// The account's row; the defaults for an account with none.
pub(super) fn load(connection: &Connection, account_id: u32) -> Result<Extras, PersistError> {
    let row = connection
        .query_row(
            "SELECT bonus_day, bonus_last_day, bonus_wins,
                    bonus_completed, bonus_shown_day, live_watch
             FROM g26_extras WHERE account_id = ?1",
            [account_id],
            |row| {
                let extras = Extras {
                    live_watch: LiveWatch::default(),
                    bonus: DailyBonus {
                        day: row.get(0)?,
                        last_day: row.get::<_, i64>(1)?.max(0) as u64,
                        wins: DailyBonus::unpack_wins(row.get(2)?),
                        completed: row.get(3)?,
                        shown_day: row.get::<_, i64>(4)?.max(0) as u64,
                    },
                };
                Ok((extras, row.get::<_, u8>(5)?))
            },
        )
        .optional()?;
    let Some((extras, live_watch)) = row else {
        return Ok(Extras::default());
    };
    let live_watch = LiveWatch::of_id(live_watch)
        .ok_or_else(|| bad_row(account_id, "live watch setting", &live_watch.to_string()))?;
    Ok(Extras { live_watch, ..extras })
}

pub(super) fn save(
    connection: &Connection,
    account_id: u32,
    extras: &Extras,
) -> Result<(), PersistError> {
    let day = |day: u64| day.min(i64::MAX as u64) as i64;
    // `bonus_reward_dict` and `bonus_reward_count` (the seventh day's reward,
    // gone with gold, the owner's of 2026-11-08) stay in the table, unused:
    // a new row takes their default, 0.
    connection.execute(
        "INSERT INTO g26_extras (
            account_id, bonus_day, bonus_last_day, bonus_wins,
            bonus_completed, bonus_shown_day, live_watch
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(account_id) DO UPDATE SET
            bonus_day = excluded.bonus_day, bonus_last_day = excluded.bonus_last_day,
            bonus_wins = excluded.bonus_wins,
            bonus_completed = excluded.bonus_completed,
            bonus_shown_day = excluded.bonus_shown_day,
            live_watch = excluded.live_watch",
        params![
            account_id,
            extras.bonus.day,
            day(extras.bonus.last_day),
            extras.bonus.packed_wins(),
            extras.bonus.completed,
            day(extras.bonus.shown_day),
            extras.live_watch.id(),
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
            "srp-g26-extras-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn the_row_comes_back_as_it_was_saved() {
        let dir = temp_dir("round-trip");
        let mut repository = SqliteGameRepository::open(&dir.join(DB_FILE)).unwrap();
        let mut account = Account::new("Alice");
        account.extras = Extras {
            live_watch: LiveWatch::Hidden,
            bonus: DailyBonus {
                day: 4,
                last_day: 20_371,
                wins: [3, 0, 2, 1, 0, 0, 0],
                completed: false,
                shown_day: 20_371,
            },
        };
        repository.save(7, &account).unwrap();
        // A plain account beside it: a new account's row.
        repository.save(8, &Account::new("Bob")).unwrap();
        let loaded = repository.load_all().unwrap();
        assert_eq!(loaded[0], (7, account.clone()));
        // A change is written over the row.
        account.extras.bonus.completed = true;
        account.extras.bonus.day = 7;
        repository.save(7, &account).unwrap();
        assert_eq!(repository.load_all().unwrap()[0].1, account);
        drop(repository);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// A file of the version before the live watch step keeps its accounts,
    /// each of them watchable; a hidden one comes back hidden after a restart
    /// (a reopened file), and an account without a row is allowed.
    #[test]
    fn the_live_watch_setting_defaults_to_allowed_and_survives_a_restart() {
        let dir = temp_dir("live-watch");
        let path = dir.join(DB_FILE);
        let step = MIGRATIONS
            .iter()
            .position(|step| *step == LIVE_WATCH_MIGRATION)
            .expect("the live watch step is one of the migrations");
        {
            let connection = Connection::open(&path).unwrap();
            connection
                .execute_batch(
                    "CREATE TABLE g26_meta (key TEXT PRIMARY KEY, value INTEGER NOT NULL);",
                )
                .unwrap();
            for earlier in &MIGRATIONS[..step] {
                connection.execute_batch(earlier).unwrap();
            }
            connection
                .execute(
                    "INSERT INTO g26_meta (key, value) VALUES ('schema', ?1)",
                    [step as i64],
                )
                .unwrap();
            connection
                .execute_batch(
                    "INSERT INTO g26_accounts (
                        account_id, name, clan_tag, generic_money, premium_money, premium_until,
                        exp_total, quest_faction, rating, rating_before, active_kit, autobuy,
                        next_item_id, next_quest_instance, tutorial_step, tutorial_done
                     ) VALUES
                        (5, 'Alice', '', 777, 12, 0, 0, 0, 1000, 1000, 1, 1, 1, 1, 0, 0),
                        (6, 'Bob', '', 1, 2, 0, 0, 0, 1000, 1000, 1, 0, 1, 1, 3, 0);
                     INSERT INTO g26_extras (account_id, bonus_day) VALUES (5, 4);",
                )
                .unwrap();
        }
        let mut alice = {
            let mut repository = SqliteGameRepository::open(&path).unwrap();
            let loaded = repository.load_all().unwrap();
            assert_eq!(loaded.len(), 2);
            assert_eq!(loaded[0].1.extras.live_watch, LiveWatch::Allowed);
            assert_eq!(loaded[0].1.extras.bonus.day, 4, "the row of before is kept");
            assert_eq!(loaded[1].1.extras.live_watch, LiveWatch::Allowed);
            let mut alice = loaded[0].1.clone();
            alice.extras.live_watch = LiveWatch::Hidden;
            repository.save(5, &alice).unwrap();
            alice
        };
        let mut repository = SqliteGameRepository::open(&path).unwrap();
        assert_eq!(repository.load_all().unwrap()[0].1, alice);
        // A new account has the default, and on again is kept too.
        repository.save(9, &Account::new("Carol")).unwrap();
        assert_eq!(
            repository.load_all().unwrap()[2].1.extras.live_watch,
            LiveWatch::Allowed
        );
        alice.extras.live_watch = LiveWatch::Allowed;
        repository.save(5, &alice).unwrap();
        drop(repository);
        let mut repository = SqliteGameRepository::open(&path).unwrap();
        assert_eq!(repository.load_all().unwrap()[0].1, alice);
        drop(repository);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// A value the table cannot hold by its CHECK, written past it, is refused
    /// rather than read as either state.
    #[test]
    fn a_bad_live_watch_value_is_refused() {
        let dir = temp_dir("live-watch-bad");
        let path = dir.join(DB_FILE);
        let mut repository = SqliteGameRepository::open(&path).unwrap();
        repository.save(5, &Account::new("Alice")).unwrap();
        drop(repository);
        let connection = Connection::open(&path).unwrap();
        connection.execute_batch("PRAGMA ignore_check_constraints = ON;").unwrap();
        connection.execute("UPDATE g26_extras SET live_watch = 7", []).unwrap();
        drop(connection);
        let mut repository = SqliteGameRepository::open(&path).unwrap();
        assert!(repository.load_all().is_err());
        drop(repository);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// A real file at schema 16 (the step before the setting, whose number
    /// has shipped) comes up at 17 with every account on: the column's own
    /// default for a row of before, and the default for an account with none.
    #[test]
    fn a_file_at_schema_16_comes_up_at_17_with_live_watch_on() {
        let dir = temp_dir("schema-16-to-17");
        let path = dir.join(DB_FILE);
        assert_eq!(
            MIGRATIONS.iter().position(|step| *step == LIVE_WATCH_MIGRATION),
            Some(16),
            "the setting's step is the 17th"
        );
        {
            let connection = Connection::open(&path).unwrap();
            connection
                .execute_batch(
                    "CREATE TABLE g26_meta (key TEXT PRIMARY KEY, value INTEGER NOT NULL);",
                )
                .unwrap();
            for earlier in &MIGRATIONS[..16] {
                connection.execute_batch(earlier).unwrap();
            }
            connection
                .execute("INSERT INTO g26_meta (key, value) VALUES ('schema', 16)", [])
                .unwrap();
            connection
                .execute_batch(
                    "INSERT INTO g26_accounts (
                        account_id, name, clan_tag, generic_money, premium_money, premium_until,
                        exp_total, quest_faction, rating, rating_before, active_kit, autobuy,
                        next_item_id, next_quest_instance, tutorial_step, tutorial_done
                     ) VALUES
                        (5, 'Alice', '', 777, 12, 0, 0, 0, 1000, 1000, 1, 1, 1, 1, 0, 0),
                        (6, 'Bob', '', 1, 2, 0, 0, 0, 1000, 1000, 1, 0, 1, 1, 3, 0);
                     INSERT INTO g26_extras (account_id, bonus_day) VALUES (5, 4);",
                )
                .unwrap();
        }
        {
            let mut repository = SqliteGameRepository::open(&path).unwrap();
            assert!(repository.version().unwrap() >= 17);
            let loaded = repository.load_all().unwrap();
            assert_eq!(loaded[0].1.extras.live_watch, LiveWatch::Allowed);
            assert_eq!(loaded[0].1.extras.bonus.day, 4);
            assert_eq!(loaded[1].1.extras.live_watch, LiveWatch::Allowed);
        }
        let connection = Connection::open(&path).unwrap();
        let stored: u8 = connection
            .query_row("SELECT live_watch FROM g26_extras WHERE account_id = 5", [], |row| row.get(0))
            .unwrap();
        assert_eq!(stored, LiveWatch::Allowed.id(), "the row of before holds the default");
        drop(connection);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// A file of the version before this step keeps its accounts.
    #[test]
    fn the_file_before_is_brought_up_and_keeps_its_accounts() {
        let dir = temp_dir("migration");
        let path = dir.join(DB_FILE);
        let step = MIGRATIONS
            .iter()
            .position(|step| *step == MIGRATION)
            .expect("the extras' step is one of the migrations");
        {
            let connection = Connection::open(&path).unwrap();
            connection
                .execute_batch(
                    "CREATE TABLE g26_meta (key TEXT PRIMARY KEY, value INTEGER NOT NULL);",
                )
                .unwrap();
            for earlier in &MIGRATIONS[..step] {
                connection.execute_batch(earlier).unwrap();
            }
            connection
                .execute(
                    "INSERT INTO g26_meta (key, value) VALUES ('schema', ?1)",
                    [step as i64],
                )
                .unwrap();
            // The columns of the first step: a new account, one that
            // reported step 3, one that skipped, one that did both.
            connection
                .execute_batch(
                    "INSERT INTO g26_accounts (
                        account_id, name, clan_tag, generic_money, premium_money, premium_until,
                        exp_total, quest_faction, rating, rating_before, active_kit, autobuy,
                        next_item_id, next_quest_instance, tutorial_step, tutorial_done
                     ) VALUES
                        (5, 'Alice', '', 777, 12, 0, 0, 0, 1000, 1000, 1, 1, 1, 1, 0, 0),
                        (6, 'Bob', '', 1, 2, 0, 0, 0, 1000, 1000, 1, 0, 1, 1, 3, 0),
                        (7, 'Carol', '', 1, 2, 0, 0, 0, 1000, 1000, 1, 0, 1, 1, 0, 1),
                        (8, 'Dave', '', 1, 2, 0, 0, 0, 1000, 1000, 1, 0, 1, 1, 1, 1);",
                )
                .unwrap();
        }
        let mut repository = SqliteGameRepository::open(&path).unwrap();
        assert_eq!(repository.version().unwrap(), MIGRATIONS.len() as i64);
        let loaded = repository.load_all().unwrap();
        assert_eq!(loaded.len(), 4);
        let alice = &loaded[0].1;
        assert_eq!(
            (alice.name.as_str(), alice.generic_money),
            ("Alice", 777)
        );
        assert!(alice.autobuy);
        // The tutorial's flags the step wrote are not read any more (no tutorial).
        // Nothing else of the new state: no bonus yet.
        for (_, account) in &loaded {
            assert_eq!(account.extras.bonus, DailyBonus::default());
        }
        // An account saved after the step has its row written.
        let mut alice = alice.clone();
        alice.extras.bonus.day = 2;
        repository.save(5, &alice).unwrap();
        assert_eq!(repository.load_all().unwrap()[0].1.extras.bonus.day, 2);
        drop(repository);
        let _ = std::fs::remove_dir_all(dir);
    }
}
