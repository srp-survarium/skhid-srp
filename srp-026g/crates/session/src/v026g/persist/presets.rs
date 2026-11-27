//! The rows an account has in `g26_preset_slots`, the slots of its kit
//! presets other than the active one ([`crate::v026g::presets`]; the active
//! one is its kits), one a filled slot, a preset with no rows empty; and its
//! row in `g26_preset_active`, the active preset; a row in `g26_preset_declined` a preset kit's
//! declined perk; and a row in `g26_preset_names` a preset kit's name.

use rusqlite::{Connection, OptionalExtension, params};

use super::bad_row;
use crate::persist::PersistError;
use crate::v026g::account::{KIT_SLOTS, KITS, PerkSlot, SlotPerk, cap_name};
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

/// The schema step that kept what each kit made of Artifact Container's slot: a column of the
/// kits (1 the container, 2 the pocket) and a row a preset kit whose slot was the pocket. The
/// next step ([`PERK_MIGRATION`]) turns both into the choices of every perk.
pub(super) const CONTAINER_MIGRATION: &str = "
    ALTER TABLE g26_kits ADD COLUMN container INTEGER NOT NULL DEFAULT 1;
    CREATE TABLE g26_preset_pockets (
        account_id INTEGER NOT NULL REFERENCES g26_accounts(account_id) ON DELETE CASCADE,
        preset     INTEGER NOT NULL,
        kit        INTEGER NOT NULL,
        PRIMARY KEY (account_id, preset, kit)
    );
    ";

/// The schema step that keeps what each kit makes of every perk's slot (`SlotPerk`): a column of
/// the kits, the perks it declines as bits (bit `SlotPerk::index`; 0 every perk taken, every kit
/// of before but one whose container was the pocket), and a row a preset kit's declined perk.
pub(super) const PERK_MIGRATION: &str = "
    ALTER TABLE g26_kits ADD COLUMN declined INTEGER NOT NULL DEFAULT 0;
    UPDATE g26_kits SET declined = 1 WHERE container = 2;
    ALTER TABLE g26_kits DROP COLUMN container;
    CREATE TABLE g26_preset_declined (
        account_id INTEGER NOT NULL REFERENCES g26_accounts(account_id) ON DELETE CASCADE,
        preset     INTEGER NOT NULL,
        kit        INTEGER NOT NULL,
        perk       INTEGER NOT NULL,
        PRIMARY KEY (account_id, preset, kit, perk)
    );
    INSERT INTO g26_preset_declined (account_id, preset, kit, perk)
        SELECT account_id, preset, kit, 0 FROM g26_preset_pockets;
    DROP TABLE g26_preset_pockets;
    ";

/// The schema step that keeps each preset kit's name (the owner's of 2026-11-27: a kit renamed in
/// one loadout keeps its old name in the others): a row a named kit of a preset other than the
/// active one (`preset` its index, as the other tables; `kit` the kit's place, its `seq`); an
/// unnamed kit has none. Until then a kit had one name in every preset, so each saved preset (one
/// with a row in the slots or the perks) takes the kits' names as they are (the owner's choice
/// of 2026-11-27): nothing players see changes at the release. An empty preset stays unnamed.
pub(super) const NAMES_MIGRATION: &str = "
    CREATE TABLE g26_preset_names (
        account_id INTEGER NOT NULL REFERENCES g26_accounts(account_id) ON DELETE CASCADE,
        preset     INTEGER NOT NULL,
        kit        INTEGER NOT NULL,
        name       TEXT NOT NULL,
        PRIMARY KEY (account_id, preset, kit)
    );
    INSERT INTO g26_preset_names (account_id, preset, kit, name)
        SELECT kits.account_id, saved.preset, kits.seq, kits.name
        FROM g26_kits AS kits
        JOIN (
            SELECT account_id, preset FROM g26_preset_slots
            UNION
            SELECT account_id, preset FROM g26_preset_declined
        ) AS saved ON saved.account_id = kits.account_id
        LEFT JOIN g26_preset_active AS active ON active.account_id = kits.account_id
        WHERE kits.name <> ''
            AND kits.seq BETWEEN 0 AND 2
            AND saved.preset BETWEEN 0 AND 4
            AND saved.preset + 1 <> COALESCE(active.preset, 1);
    ";
// The step's bounds are the kits' and the presets' counts as they were then.
const _: () = assert!(KITS == 3 && PRESETS == 5);

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
    let place =
        |value: i64, below: usize| usize::try_from(value).ok().filter(|&value| value < below);
    let mut presets: [Option<Preset>; PRESETS] = [const { None }; PRESETS];
    for row in rows {
        let (preset, kit, slot, saved) = row?;
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
        presets[preset].get_or_insert(Preset::EMPTY).kits[kit][slot] = Some(saved);
    }
    let mut statement = connection
        .prepare_cached("SELECT preset, kit, perk FROM g26_preset_declined WHERE account_id = ?1")?;
    let rows = statement.query_map([account_id], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?, row.get::<_, i64>(2)?))
    })?;
    for row in rows {
        let (preset, kit, perk) = row?;
        let (Some(preset), Some(kit), Some(perk)) = (
            place(preset, PRESETS),
            place(kit, KITS),
            place(perk, SlotPerk::ALL.len()).map(|index| SlotPerk::ALL[index]),
        ) else {
            return Err(bad_row(account_id, "preset perk", &format!("{preset}/{kit}/{perk}")));
        };
        let saved = presets[preset].get_or_insert(Preset::EMPTY);
        saved.perks[kit] = saved.perks[kit].with(perk, PerkSlot::Declined);
    }
    let mut statement = connection
        .prepare_cached("SELECT preset, kit, name FROM g26_preset_names WHERE account_id = ?1")?;
    let rows = statement.query_map([account_id], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?, row.get::<_, String>(2)?))
    })?;
    for row in rows {
        let (preset, kit, name) = row?;
        let (Some(preset), Some(kit)) = (place(preset, PRESETS), place(kit, KITS)) else {
            return Err(bad_row(account_id, "preset name", &format!("{preset}/{kit}")));
        };
        // Capped as the kits' names are at load (`Account::repair`).
        presets[preset].get_or_insert(Preset::EMPTY).names[kit] = cap_name(&name);
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
    connection.execute(
        "DELETE FROM g26_preset_declined WHERE account_id = ?1",
        [account_id],
    )?;
    connection.execute(
        "DELETE FROM g26_preset_names WHERE account_id = ?1",
        [account_id],
    )?;
    for (preset_index, preset) in presets.iter().enumerate() {
        let Some(preset) = preset else {
            continue;
        };
        for (kit_index, name) in preset.names.iter().enumerate().filter(|(_, name)| !name.is_empty()) {
            connection.execute(
                "INSERT INTO g26_preset_names (account_id, preset, kit, name)
                 VALUES (?1, ?2, ?3, ?4)",
                params![account_id, preset_index as i64, kit_index as i64, name],
            )?;
        }
        for (kit_index, perks) in preset.perks.iter().enumerate() {
            for perk in SlotPerk::ALL.into_iter().filter(|&perk| perks.get(perk) == PerkSlot::Declined) {
                connection.execute(
                    "INSERT INTO g26_preset_declined (account_id, preset, kit, perk)
                     VALUES (?1, ?2, ?3, ?4)",
                    params![account_id, preset_index as i64, kit_index as i64, perk.index() as i64],
                )?;
            }
        }
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
    use crate::v026g::{presets, rules};

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
        // Alice keeps her starter in preset 1 and plays preset 3, emptied; her second kit
        // declines both perks, in preset 1 and in the kits.
        use crate::v026g::account::{ARTIFACT_CONTAINER, DEEP_POCKETS, KitPerks, PerkSlot, SlotPerk};
        let both = SlotPerk::ALL
            .into_iter()
            .fold(KitPerks::ALL_TAKEN, |perks, perk| perks.with(perk, PerkSlot::Declined));
        let mut account = Account::new("Alice");
        account.skills = vec![(ARTIFACT_CONTAINER, 1), (DEEP_POCKETS, 1)];
        account.kits[1].perks = both;
        presets::choose(&mut account, 3, 0).unwrap();
        account.kits[1].perks = both;
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
        presets::choose(&mut account, 1, 0).unwrap();
        repository.save(7, &account).unwrap();
        let loaded = repository.load_all().unwrap();
        let alice = &loaded.iter().find(|(id, _)| *id == 7).unwrap().1;
        assert_eq!(alice.active_preset, 1);
        assert_eq!(alice.presets, account.presets);
        // Preset 3 was left with nothing worn, its second kit's perks declined: that alone.
        let left = alice.presets[2].as_ref().unwrap();
        assert_eq!(left.slots(), 0);
        assert_eq!(left.perks[1], both);
        assert!(alice.presets[1].is_none() && alice.presets[3].is_none());
        drop(repository);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A temporary folder of its own, removed when dropped.
    struct TempDir(std::path::PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "srp-g26-{name}-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ).replace(['(', ')'], ""));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn loaded(repository: &mut SqliteGameRepository, account_id: u32) -> Account {
        repository
            .load_all()
            .unwrap()
            .into_iter()
            .find(|(id, _)| *id == account_id)
            .unwrap()
            .1
    }

    /// Alice: kit 1 named "Starter" in preset 1, kit 2 "Sniper" in preset 3 (played), and
    /// preset 2 holding only kit 3's name.
    fn named_presets() -> Account {
        let mut account = Account::new("Alice");
        let id = account.kits[0].id;
        rules::rename_kit(&mut account, id, "Starter").unwrap();
        presets::choose(&mut account, 2, 0).unwrap();
        let id = account.kits[2].id;
        rules::rename_kit(&mut account, id, "Spare").unwrap();
        presets::choose(&mut account, 3, 0).unwrap();
        let id = account.kits[1].id;
        rules::rename_kit(&mut account, id, "Sniper").unwrap();
        account
    }

    #[test]
    fn the_preset_kits_names_come_back_with_the_account() {
        let dir = TempDir::new("preset-names");
        let mut repository = SqliteGameRepository::open(&dir.0.join(DB_FILE)).unwrap();
        let mut account = named_presets();
        repository.save(7, &account).unwrap();
        let alice = loaded(&mut repository, 7);
        assert_eq!(alice.presets, account.presets);
        assert_eq!(alice.presets[0].as_ref().unwrap().names[0], "Starter");
        assert_eq!(alice.presets[1].as_ref().unwrap().names[2], "Spare");
        assert_eq!(alice.kits[1].name, "Sniper");

        // A later save replaces them: preset 1 put on again, its name with it.
        presets::choose(&mut account, 1, 0).unwrap();
        repository.save(7, &account).unwrap();
        let alice = loaded(&mut repository, 7);
        assert_eq!(alice.presets, account.presets);
        assert_eq!(alice.kits[0].name, "Starter");
        assert!(alice.presets[0].is_none());
        assert_eq!(alice.presets[2].as_ref().unwrap().names[1], "Sniper");
    }

    /// Step 26 on a file of step 25, as live has it: each kit had one name in every preset, so
    /// every saved preset other than the active one takes the kits' names; an empty preset, the
    /// active one's place and an unnamed kit take none.
    #[test]
    fn the_saved_presets_take_the_kits_names_at_the_upgrade() {
        let dir = TempDir::new("preset-names-upgrade");
        let path = dir.0.join(DB_FILE);
        let mut account = Account::new("Alice");
        let id = account.kits[0].id;
        rules::rename_kit(&mut account, id, "Starter").unwrap();
        let id = account.kits[2].id;
        rules::rename_kit(&mut account, id, "Rifle").unwrap();
        // Preset 1 saved with the starter; preset 4 with nothing but a declined perk; preset 2
        // played; 3 and 5 empty.
        use crate::v026g::account::{ARTIFACT_CONTAINER, PerkSlot, SlotPerk};
        account.skills = vec![(ARTIFACT_CONTAINER, 1)];
        presets::choose(&mut account, 4, 0).unwrap();
        account.kits[0].perks = account.kits[0].perks.with(SlotPerk::ALL[0], PerkSlot::Declined);
        presets::choose(&mut account, 2, 0).unwrap();
        // Bob: no presets saved, a named kit.
        let mut bob = Account::new("Bob");
        let id = bob.kits[1].id;
        rules::rename_kit(&mut bob, id, "Mine").unwrap();
        {
            let mut repository = SqliteGameRepository::open(&path).unwrap();
            repository.save(7, &account).unwrap();
            repository.save(8, &bob).unwrap();
            // The file as step 25 left it: one name a kit, kept with the kits. Then step 26
            // itself, run on it as the upgrade runs it (the steps after it are already there,
            // so the file's version is not wound back: they would run again).
            repository
                .connection
                .execute_batch(
                    "DROP TABLE g26_preset_names;
                     UPDATE g26_kits SET name = 'Starter' WHERE account_id = 7 AND seq = 0;
                     UPDATE g26_kits SET name = 'Rifle' WHERE account_id = 7 AND seq = 2;",
                )
                .unwrap();
            repository.connection.execute_batch(super::NAMES_MIGRATION).unwrap();
        }
        let mut repository = SqliteGameRepository::open(&path).unwrap();
        let alice = loaded(&mut repository, 7);
        assert_eq!(alice.active_preset, 2);
        let names = |preset: usize| alice.presets[preset].as_ref().map(|preset| preset.names.clone());
        let kits_names = Some(["Starter".to_string(), String::new(), "Rifle".to_string()]);
        assert_eq!(names(0), kits_names);
        assert_eq!(names(3), kits_names);
        assert_eq!(names(1), None);
        assert_eq!(names(2), None);
        assert_eq!(names(4), None);
        // The kits keep theirs: nothing players see changes.
        assert_eq!(alice.kits[0].name, "Starter");
        assert_eq!(alice.kits[2].name, "Rifle");
        let bob = loaded(&mut repository, 8);
        assert!(bob.presets.iter().all(Option::is_none));
        assert_eq!(bob.kits[1].name, "Mine");
        let rows: i64 = repository
            .connection
            .query_row("SELECT count(*) FROM g26_preset_names", [], |row| row.get(0))
            .unwrap();
        assert_eq!(rows, 4);
    }
}
