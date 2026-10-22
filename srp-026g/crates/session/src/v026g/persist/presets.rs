//! The rows an account has in `g26_preset_slots`, the slots of its kit
//! presets other than the active one ([`crate::v026g::presets`]; the active
//! one is its kits), one a filled slot, a preset with no rows empty; and its
//! row in `g26_preset_active`, the active preset.

use rusqlite::{Connection, OptionalExtension, params};

use super::bad_row;
use crate::persist::PersistError;
use crate::v026g::account::{KIT_SLOTS, KITS};
use crate::v026g::presets::{FIRST_PRESET, PRESETS, Preset, PresetSlot};

/// The schema step that adds the tables. An account of before has no rows:
/// preset 1 active (its kits as they are), the others empty.
pub(super) const MIGRATION: &str = "
    CREATE TABLE g26_preset_active (
        account_id INTEGER PRIMARY KEY REFERENCES g26_accounts(account_id) ON DELETE CASCADE,
        preset     INTEGER NOT NULL
    );
    CREATE TABLE g26_preset_slots (
        account_id INTEGER NOT NULL REFERENCES g26_accounts(account_id) ON DELETE CASCADE,
        preset     INTEGER NOT NULL,
        kit        INTEGER NOT NULL,
        slot       INTEGER NOT NULL,
        item_id    INTEGER NOT NULL,
        dict_id    INTEGER NOT NULL,
        count      INTEGER NOT NULL,
        PRIMARY KEY (account_id, preset, kit, slot)
    );
    ";

/// The account's active preset; none kept: the first.
pub(super) fn load_active(connection: &Connection, account_id: u32) -> Result<u8, PersistError> {
    let active: Option<i64> = connection
        .query_row(
            "SELECT preset FROM g26_preset_active WHERE account_id = ?1",
            [account_id],
            |row| row.get(0),
        )
        .optional()?;
    Ok(active
        .and_then(|preset| u8::try_from(preset).ok())
        .unwrap_or(FIRST_PRESET))
}

/// Replaces the account's active preset.
pub(super) fn save_active(
    connection: &Connection,
    account_id: u32,
    active: u8,
) -> Result<(), PersistError> {
    connection.execute(
        "INSERT INTO g26_preset_active (account_id, preset) VALUES (?1, ?2)
         ON CONFLICT(account_id) DO UPDATE SET preset = excluded.preset",
        params![account_id, active],
    )?;
    Ok(())
}

/// The account's presets, preset 1 first.
pub(super) fn load(
    connection: &Connection,
    account_id: u32,
) -> Result<[Option<Preset>; PRESETS], PersistError> {
    let mut statement = connection.prepare_cached(
        "SELECT preset, kit, slot, item_id, dict_id, count FROM g26_preset_slots
         WHERE account_id = ?1",
    )?;
    let rows = statement.query_map([account_id], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, i64>(2)?,
            PresetSlot {
                item_id: row.get(3)?,
                dict_id: row.get(4)?,
                count: row.get(5)?,
            },
        ))
    })?;
    let mut presets: [Option<Preset>; PRESETS] = [None; PRESETS];
    for row in rows {
        let (preset, kit, slot, saved) = row?;
        let place =
            |value: i64, below: usize| usize::try_from(value).ok().filter(|&value| value < below);
        let (Some(preset), Some(kit), Some(slot)) = (
            place(preset, PRESETS),
            place(kit, KITS),
            place(slot, KIT_SLOTS),
        ) else {
            return Err(bad_row(
                account_id,
                "preset slot",
                &format!("{preset}/{kit}/{slot}"),
            ));
        };
        presets[preset]
            .get_or_insert(Preset {
                kits: [[None; KIT_SLOTS]; KITS],
            })
            .kits[kit][slot] = Some(saved);
    }
    Ok(presets)
}

/// Replaces the account's rows.
pub(super) fn save(
    connection: &Connection,
    account_id: u32,
    presets: &[Option<Preset>; PRESETS],
) -> Result<(), PersistError> {
    connection.execute(
        "DELETE FROM g26_preset_slots WHERE account_id = ?1",
        [account_id],
    )?;
    for (preset_index, preset) in presets.iter().enumerate() {
        let Some(preset) = preset else {
            continue;
        };
        for (kit_index, kit) in preset.kits.iter().enumerate() {
            for (slot, saved) in kit.iter().enumerate() {
                let Some(saved) = saved else {
                    continue;
                };
                connection.execute(
                    "INSERT INTO g26_preset_slots
                        (account_id, preset, kit, slot, item_id, dict_id, count)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![
                        account_id,
                        preset_index as i64,
                        kit_index as i64,
                        slot as i64,
                        saved.item_id,
                        saved.dict_id,
                        saved.count,
                    ],
                )?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::{GameRepository, SqliteGameRepository};
    use crate::persist::DB_FILE;
    use crate::v026g::account::Account;
    use crate::v026g::presets;

    #[test]
    fn the_presets_come_back_with_the_account() {
        let dir = std::env::temp_dir().join(format!(
            "srp-g26-presets-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut repository = SqliteGameRepository::open(&dir.join(DB_FILE)).unwrap();
        // Bob, as an account of before presets: no rows, preset 1 active.
        repository.save(8, &Account::new("Bob")).unwrap();
        repository
            .connection
            .execute("DELETE FROM g26_preset_active WHERE account_id = 8", [])
            .unwrap();
        // Alice keeps her starter in preset 1 and plays preset 3, emptied.
        let mut account = Account::new("Alice");
        presets::choose(&mut account, 3).unwrap();
        account.kits[0].slots = [None; crate::v026g::account::KIT_SLOTS];
        repository.save(7, &account).unwrap();
        let loaded = repository.load_all().unwrap();
        let alice = &loaded.iter().find(|(id, _)| *id == 7).unwrap().1;
        assert_eq!(alice.active_preset, 3);
        assert_eq!(alice.presets, account.presets);
        assert!(alice.presets[0].is_some());
        assert_eq!(alice.kits, account.kits);
        let bob = &loaded.iter().find(|(id, _)| *id == 8).unwrap().1;
        assert_eq!(bob.active_preset, 1);
        assert!(bob.presets.iter().all(Option::is_none));
        assert!(presets::preset(bob, 1).unwrap().is_some());

        // A later save replaces them.
        presets::choose(&mut account, 1).unwrap();
        repository.save(7, &account).unwrap();
        let loaded = repository.load_all().unwrap();
        let alice = &loaded.iter().find(|(id, _)| *id == 7).unwrap().1;
        assert_eq!(alice.active_preset, 1);
        assert_eq!(alice.presets, account.presets);
        // Preset 3 was left with nothing worn: empty.
        assert!(alice.presets.iter().all(Option::is_none));
        drop(repository);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
