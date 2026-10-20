//! 0.26g accounts kept across restarts, the way 0.21d's are
//! (`crate::persist`): a [`GameRepository`] takes a whole account and
//! decides where it goes. [`SqliteGameRepository`] keeps them in srp's
//! SQLite file; [`MemoryGameRepository`] in memory, for tests.
//!
//! The 0.26g state lives beside the 0.21d tables in the same `srp.db`, in
//! tables of its own (`g26_*`), keyed by the same account id. Their schema
//! is versioned on its own (`g26_meta`), not by the file's `user_version`,
//! which counts 0.21d's migrations: neither side has to know the other's
//! steps. Lists keep their order in a `seq` column, as 0.21d's do.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use rusqlite::{Connection, OptionalExtension, params};

use super::account::{Account, Item, KIT_SLOTS, Kit, KitSlot, QuestInstance, Upgrade};
use super::clans::Clan;
use crate::LockExt;
use crate::persist::{DATA_DIR_VAR, DB_FILE, DB_VAR, DEFAULT_DATA_DIR, PersistError};

mod balance;
mod clans;
mod extras;
mod items;
mod map_plays;
mod results;
mod room_settings;

/// The schema, step by step. Never change a step that shipped: add one.
const MIGRATIONS: &[&str] = &[
    // 1: the 0.26g account and everything it owns.
    "
    CREATE TABLE g26_accounts (
        account_id          INTEGER PRIMARY KEY,
        name                TEXT NOT NULL,
        clan_tag            TEXT NOT NULL,
        generic_money       INTEGER NOT NULL,
        premium_money       INTEGER NOT NULL,
        premium_until       INTEGER NOT NULL,
        exp_total           INTEGER NOT NULL,
        quest_faction       INTEGER NOT NULL,
        rating              INTEGER NOT NULL,
        rating_before       INTEGER NOT NULL,
        active_kit          INTEGER NOT NULL,
        autobuy             INTEGER NOT NULL,
        next_item_id        INTEGER NOT NULL,
        next_quest_instance INTEGER NOT NULL,
        tutorial_step       INTEGER NOT NULL,
        tutorial_done       INTEGER NOT NULL
    );
    CREATE TABLE g26_skills (
        account_id INTEGER NOT NULL REFERENCES g26_accounts(account_id) ON DELETE CASCADE,
        seq        INTEGER NOT NULL,
        skill_id   INTEGER NOT NULL,
        points     INTEGER NOT NULL,
        PRIMARY KEY (account_id, seq)
    );
    CREATE TABLE g26_reputations (
        account_id INTEGER NOT NULL REFERENCES g26_accounts(account_id) ON DELETE CASCADE,
        seq        INTEGER NOT NULL,
        faction    INTEGER NOT NULL,
        points     INTEGER NOT NULL,
        PRIMARY KEY (account_id, seq)
    );
    CREATE TABLE g26_items (
        account_id         INTEGER NOT NULL REFERENCES g26_accounts(account_id) ON DELETE CASCADE,
        item_id            INTEGER NOT NULL,
        seq                INTEGER NOT NULL,
        dict_id            INTEGER NOT NULL,
        condition_or_stack INTEGER NOT NULL,
        repair_time        INTEGER NOT NULL,
        reroll_time        INTEGER NOT NULL,
        -- `id:state`, comma-separated; empty for none.
        modifications      TEXT NOT NULL,
        -- `id:a:value:param`, comma-separated; empty for none.
        upgrades           TEXT NOT NULL,
        PRIMARY KEY (account_id, item_id)
    );
    CREATE TABLE g26_kits (
        account_id INTEGER NOT NULL REFERENCES g26_accounts(account_id) ON DELETE CASCADE,
        id         INTEGER NOT NULL,
        seq        INTEGER NOT NULL,
        name       TEXT NOT NULL,
        revision   INTEGER NOT NULL,
        PRIMARY KEY (account_id, id)
    );
    -- A slot names an item of the account and how many of it the kit takes.
    CREATE TABLE g26_kit_slots (
        account_id INTEGER NOT NULL,
        kit_id     INTEGER NOT NULL,
        slot       INTEGER NOT NULL,
        item_id    INTEGER NOT NULL,
        count      INTEGER NOT NULL,
        PRIMARY KEY (account_id, kit_id, slot),
        FOREIGN KEY (account_id, kit_id)
            REFERENCES g26_kits(account_id, id) ON DELETE CASCADE
    );
    CREATE TABLE g26_quests (
        account_id  INTEGER NOT NULL REFERENCES g26_accounts(account_id) ON DELETE CASCADE,
        instance_id INTEGER NOT NULL,
        seq         INTEGER NOT NULL,
        quest_id    INTEGER NOT NULL,
        games       INTEGER NOT NULL,
        expires_at  INTEGER NOT NULL,
        PRIMARY KEY (account_id, instance_id)
    );
    CREATE TABLE g26_quest_cooldowns (
        account_id INTEGER NOT NULL REFERENCES g26_accounts(account_id) ON DELETE CASCADE,
        seq        INTEGER NOT NULL,
        quest_id   INTEGER NOT NULL,
        until      INTEGER NOT NULL,
        PRIMARY KEY (account_id, seq)
    );
    ",
    // 2: the auto-repair box, the tutorial's steps and the daily bonus
    // (`extras`).
    extras::MIGRATION,
    // 3: clans, their ranks and members (`clans`).
    clans::MIGRATION,
    // 4: what an item keeps of its modification changes (`items`).
    items::MIGRATION,
    // 5: the matches an account was put in, by map (`map_plays`).
    map_plays::MIGRATION,
    // 6: the finished matches' reports and play periods (`results`).
    results::MIGRATION,
    // 7: the balance rating the sides of a match are made with (`balance`).
    balance::MIGRATION,
    // 8: the settings of an account's last room made by hand (`room_settings`).
    room_settings::MIGRATION,
];

/// The child tables of an account, children of children first.
const CHILD_TABLES: &[&str] = &[
    "g26_kit_slots",
    "g26_kits",
    "g26_items",
    "g26_skills",
    "g26_reputations",
    "g26_quests",
    "g26_quest_cooldowns",
];

/// Where 0.26g accounts are kept. Each call stands alone: a save is one
/// transaction, so a crash leaves an account either as it was or as it is.
pub trait GameRepository: Send {
    /// Every account, by id.
    fn load_all(&mut self) -> Result<Vec<(u32, Account)>, PersistError>;
    /// Write one account whole, replacing what was kept of it.
    fn save(&mut self, account_id: u32, account: &Account) -> Result<(), PersistError>;
    /// Where it keeps them, for the log.
    fn describe(&self) -> String;
    /// Every clan, by id. (A repository that keeps none has none.)
    fn load_clans(&mut self) -> Result<Vec<Clan>, PersistError> {
        Ok(Vec::new())
    }
    /// Write one clan whole, replacing what was kept of it.
    fn save_clan(&mut self, _clan: &Clan) -> Result<(), PersistError> {
        Ok(())
    }
    /// Forget a clan that was disbanded.
    fn delete_clan(&mut self, _clan_id: u32) -> Result<(), PersistError> {
        Ok(())
    }
    /// What `rewards::Results` keeps across a restart: `(key, body)` rows,
    /// by key. (A repository that keeps none has none.)
    fn load_results(&mut self) -> Result<Vec<(String, String)>, PersistError> {
        Ok(Vec::new())
    }
    /// Write one of those rows, replacing what was kept under its key.
    fn save_result(&mut self, _key: &str, _body: &str) -> Result<(), PersistError> {
        Ok(())
    }
    /// Forget one of those rows.
    fn delete_result(&mut self, _key: &str) -> Result<(), PersistError> {
        Ok(())
    }
}

/// Accounts kept in memory. Clones share one set, so a test can keep one
/// and give a store another.
#[derive(Clone, Default)]
pub struct MemoryGameRepository {
    inner: Arc<Mutex<(HashMap<u32, Account>, usize)>>,
    clans: Arc<Mutex<HashMap<u32, Clan>>>,
    results: Arc<Mutex<std::collections::BTreeMap<String, String>>>,
}

impl MemoryGameRepository {
    /// How many accounts were written so far.
    pub fn saves(&self) -> usize {
        self.inner.locked().1
    }

    pub fn get(&self, account_id: u32) -> Option<Account> {
        self.inner.locked().0.get(&account_id).cloned()
    }
}

impl GameRepository for MemoryGameRepository {
    fn load_all(&mut self) -> Result<Vec<(u32, Account)>, PersistError> {
        let inner = self.inner.locked();
        let mut accounts: Vec<(u32, Account)> = inner
            .0
            .iter()
            .map(|(&id, account)| (id, account.clone()))
            .collect();
        accounts.sort_by_key(|&(id, _)| id);
        Ok(accounts)
    }

    fn save(&mut self, account_id: u32, account: &Account) -> Result<(), PersistError> {
        let mut inner = self.inner.locked();
        inner.0.insert(account_id, account.clone());
        inner.1 += 1;
        Ok(())
    }

    fn describe(&self) -> String {
        "memory".to_owned()
    }

    fn load_clans(&mut self) -> Result<Vec<Clan>, PersistError> {
        let mut clans: Vec<Clan> = self.clans.locked().values().cloned().collect();
        clans.sort_by_key(|clan| clan.id);
        Ok(clans)
    }

    fn save_clan(&mut self, clan: &Clan) -> Result<(), PersistError> {
        self.clans.locked().insert(clan.id, clan.clone());
        Ok(())
    }

    fn delete_clan(&mut self, clan_id: u32) -> Result<(), PersistError> {
        self.clans.locked().remove(&clan_id);
        Ok(())
    }

    fn load_results(&mut self) -> Result<Vec<(String, String)>, PersistError> {
        Ok(self
            .results
            .locked()
            .iter()
            .map(|(key, body)| (key.clone(), body.clone()))
            .collect())
    }

    fn save_result(&mut self, key: &str, body: &str) -> Result<(), PersistError> {
        self.results
            .locked()
            .insert(key.to_owned(), body.to_owned());
        Ok(())
    }

    fn delete_result(&mut self, key: &str) -> Result<(), PersistError> {
        self.results.locked().remove(key);
        Ok(())
    }
}

/// 0.26g accounts in srp's SQLite file.
pub struct SqliteGameRepository {
    connection: Connection,
    path: String,
}

impl SqliteGameRepository {
    /// Open (or create) the database at `path` and bring the 0.26g tables
    /// to the newest schema. Refuses tables newer than this code.
    pub fn open(path: &Path) -> Result<Self, PersistError> {
        let connection = Connection::open(path)?;
        let _mode: String =
            connection.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))?;
        connection.execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA synchronous = NORMAL;
             PRAGMA busy_timeout = 5000;",
        )?;
        migrate(&connection)?;
        Ok(Self {
            connection,
            path: path.display().to_string(),
        })
    }

    /// `<dir>/srp.db`, the directory made if missing.
    pub fn open_in(dir: &Path) -> Result<Self, PersistError> {
        std::fs::create_dir_all(dir).map_err(|error| PersistError::Io(dir.to_owned(), error))?;
        Self::open(&dir.join(DB_FILE))
    }

    /// The repository srp's environment asks for: `None` with `SRP_DB=off`
    /// (or empty), else `<SRP_DATA_DIR>/srp.db`. Open the 0.21d repository
    /// first (`crate::persist::open_from_env`): it backs the file up.
    pub fn from_env() -> Result<Option<Self>, PersistError> {
        if let Ok(value) = std::env::var(DB_VAR) {
            let value = value.trim();
            if value.is_empty() || value.eq_ignore_ascii_case("off") {
                return Ok(None);
            }
        }
        let dir = std::env::var_os(DATA_DIR_VAR)
            .filter(|dir| !dir.is_empty())
            .map_or_else(
                || std::path::PathBuf::from(DEFAULT_DATA_DIR),
                std::path::PathBuf::from,
            );
        Self::open_in(&dir).map(Some)
    }

    /// The schema version of the 0.26g tables.
    pub fn version(&self) -> Result<i64, PersistError> {
        schema_version(&self.connection)
    }

    /// Whether this is the first call with `key` in the database's life:
    /// the key is written to `g26_meta`, so every later call says no. For
    /// what is done to the accounts once ([`GEAR_MENDED`]).
    pub fn first_time(&mut self, key: &str) -> Result<bool, PersistError> {
        let written = self.connection.execute(
            "INSERT OR IGNORE INTO g26_meta (key, value) VALUES (?1, 1)",
            [key],
        )?;
        Ok(written == 1)
    }
}

/// The `g26_meta` key that says the accounts' weapons and armour were made
/// whole when wear was switched off (`GameAccounts::from_env`).
pub const GEAR_MENDED: &str = "gear_mended";

fn schema_version(connection: &Connection) -> Result<i64, PersistError> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS g26_meta (
            key   TEXT PRIMARY KEY,
            value INTEGER NOT NULL
        );",
    )?;
    let version: Option<i64> = connection
        .query_row(
            "SELECT value FROM g26_meta WHERE key = 'schema'",
            [],
            |row| row.get(0),
        )
        .optional()?;
    Ok(version.unwrap_or(0))
}

/// Apply the steps the file lacks, each in its own transaction with its
/// version.
fn migrate(connection: &Connection) -> Result<(), PersistError> {
    let found = schema_version(connection)?;
    let known = MIGRATIONS.len() as i64;
    if found > known {
        return Err(PersistError::NewerSchema { found, known });
    }
    for (index, step) in MIGRATIONS.iter().enumerate().skip(found as usize) {
        let transaction = connection.unchecked_transaction()?;
        transaction.execute_batch(step)?;
        transaction.execute(
            "INSERT INTO g26_meta (key, value) VALUES ('schema', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [index as i64 + 1],
        )?;
        transaction.commit()?;
        log::info!("0.26g account tables upgraded to version {}", index + 1);
    }
    Ok(())
}

fn modifications_text(item: &Item) -> String {
    item.modifications
        .iter()
        .map(|&(id, state)| format!("{id}:{}", u8::from(state)))
        .collect::<Vec<_>>()
        .join(",")
}

fn upgrades_text(item: &Item) -> String {
    item.upgrades
        .iter()
        .map(|upgrade| {
            format!(
                "{}:{}:{}:{}",
                upgrade.id, upgrade.a, upgrade.value, upgrade.param
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn bad_row(account_id: u32, what: &str, text: &str) -> PersistError {
    PersistError::BadRow(account_id, format!("{what} `{text}`"))
}

fn parse_modifications(account_id: u32, text: &str) -> Result<Vec<(u32, bool)>, PersistError> {
    text.split(',')
        .filter(|entry| !entry.is_empty())
        .map(|entry| {
            let bad = || bad_row(account_id, "modifications", text);
            let (id, state) = entry.split_once(':').ok_or_else(bad)?;
            Ok((id.parse().map_err(|_| bad())?, state != "0"))
        })
        .collect()
}

fn parse_upgrades(account_id: u32, text: &str) -> Result<Vec<Upgrade>, PersistError> {
    text.split(',')
        .filter(|entry| !entry.is_empty())
        .map(|entry| {
            let bad = || bad_row(account_id, "upgrades", text);
            let mut fields = entry.split(':');
            let mut next = || fields.next().ok_or_else(bad);
            Ok(Upgrade {
                id: next()?.parse().map_err(|_| bad())?,
                a: next()?.parse().map_err(|_| bad())?,
                value: next()?.parse().map_err(|_| bad())?,
                param: next()?.parse().map_err(|_| bad())?,
            })
        })
        .collect()
}

impl GameRepository for SqliteGameRepository {
    fn load_all(&mut self) -> Result<Vec<(u32, Account)>, PersistError> {
        let transaction = self.connection.transaction()?;
        let mut accounts: Vec<(u32, Account)> = Vec::new();
        {
            let mut statement = transaction.prepare(
                "SELECT account_id, name, clan_tag, generic_money, premium_money,
                        premium_until, exp_total, quest_faction, rating, rating_before,
                        active_kit, autobuy, next_item_id, next_quest_instance,
                        tutorial_step, tutorial_done
                 FROM g26_accounts ORDER BY account_id",
            )?;
            let rows = statement.query_map([], |row| {
                Ok((
                    row.get::<_, u32>(0)?,
                    Account {
                        name: row.get(1)?,
                        clan_tag: row.get(2)?,
                        generic_money: row.get(3)?,
                        premium_money: row.get(4)?,
                        premium_until: row.get::<_, i64>(5)?.max(0) as u64,
                        exp_total: row.get(6)?,
                        skills: Vec::new(),
                        reputations: Vec::new(),
                        quest_faction: row.get(7)?,
                        rating: row.get(8)?,
                        rating_before: row.get(9)?,
                        balance: Default::default(),
                        kits: Vec::new(),
                        active_kit: row.get(10)?,
                        autobuy: row.get(11)?,
                        inventory: Vec::new(),
                        next_item_id: row.get(12)?,
                        quests: Vec::new(),
                        quest_cooldowns: Vec::new(),
                        next_quest_instance: row.get(13)?,
                        tutorial_step: row.get(14)?,
                        tutorial_done: row.get(15)?,
                        extras: Default::default(),
                        map_plays: Vec::new(),
                        room_settings: None,
                    },
                ))
            })?;
            for row in rows {
                accounts.push(row?);
            }
        }
        for (account_id, account) in &mut accounts {
            let account_id = *account_id;
            account.extras = extras::load(&transaction, account_id)?;
            account.map_plays = map_plays::load(&transaction, account_id)?;
            account.balance = balance::load(&transaction, account_id)?;
            account.room_settings = room_settings::load(&transaction, account_id)?;
            {
                let mut statement = transaction.prepare_cached(
                    "SELECT skill_id, points FROM g26_skills WHERE account_id = ?1 ORDER BY seq",
                )?;
                let rows =
                    statement.query_map([account_id], |row| Ok((row.get(0)?, row.get(1)?)))?;
                account.skills = rows.collect::<Result<_, _>>()?;
            }
            {
                let mut statement = transaction.prepare_cached(
                    "SELECT faction, points FROM g26_reputations
                     WHERE account_id = ?1 ORDER BY seq",
                )?;
                let rows =
                    statement.query_map([account_id], |row| Ok((row.get(0)?, row.get(1)?)))?;
                account.reputations = rows.collect::<Result<_, _>>()?;
            }
            {
                let mut statement = transaction.prepare_cached(
                    "SELECT item_id, dict_id, condition_or_stack, repair_time, reroll_time,
                            modifications, upgrades, reroll
                     FROM g26_items WHERE account_id = ?1 ORDER BY seq",
                )?;
                let rows = statement.query_map([account_id], |row| {
                    Ok((
                        Item {
                            item_id: row.get(0)?,
                            dict_id: row.get(1)?,
                            condition_or_stack: row.get(2)?,
                            repair_time: row.get(3)?,
                            reroll_time: row.get(4)?,
                            modifications: Vec::new(),
                            upgrades: Vec::new(),
                            reroll: Default::default(),
                        },
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, String>(7)?,
                    ))
                })?;
                for row in rows {
                    let (mut item, modifications, upgrades, reroll) = row?;
                    item.modifications = parse_modifications(account_id, &modifications)?;
                    item.upgrades = parse_upgrades(account_id, &upgrades)?;
                    item.reroll = items::parse_reroll(account_id, &reroll)?;
                    account.inventory.push(item);
                }
            }
            {
                let mut statement = transaction.prepare_cached(
                    "SELECT id, name, revision FROM g26_kits WHERE account_id = ?1 ORDER BY seq",
                )?;
                let rows = statement.query_map([account_id], |row| {
                    Ok(Kit {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        revision: row.get(2)?,
                        slots: [None; KIT_SLOTS],
                    })
                })?;
                account.kits = rows.collect::<Result<_, _>>()?;
            }
            {
                let mut statement = transaction.prepare_cached(
                    "SELECT kit_id, slot, item_id, count FROM g26_kit_slots WHERE account_id = ?1",
                )?;
                let rows = statement.query_map([account_id], |row| {
                    Ok((
                        row.get::<_, u32>(0)?,
                        row.get::<_, usize>(1)?,
                        KitSlot {
                            item_id: row.get(2)?,
                            count: row.get(3)?,
                        },
                    ))
                })?;
                for row in rows {
                    let (kit_id, slot, held) = row?;
                    if slot >= KIT_SLOTS {
                        return Err(PersistError::BadRow(account_id, format!("kit slot {slot}")));
                    }
                    if let Some(kit) = account.kits.iter_mut().find(|kit| kit.id == kit_id) {
                        kit.slots[slot] = Some(held);
                    }
                }
            }
            {
                let mut statement = transaction.prepare_cached(
                    "SELECT instance_id, quest_id, games, expires_at
                     FROM g26_quests WHERE account_id = ?1 ORDER BY seq",
                )?;
                let rows = statement.query_map([account_id], |row| {
                    Ok(QuestInstance {
                        instance_id: row.get(0)?,
                        quest_id: row.get(1)?,
                        games: row.get(2)?,
                        expires_at: row.get::<_, i64>(3)?.max(0) as u64,
                    })
                })?;
                account.quests = rows.collect::<Result<_, _>>()?;
            }
            {
                let mut statement = transaction.prepare_cached(
                    "SELECT quest_id, until FROM g26_quest_cooldowns
                     WHERE account_id = ?1 ORDER BY seq",
                )?;
                let rows = statement.query_map([account_id], |row| {
                    Ok((row.get(0)?, row.get::<_, i64>(1)?.max(0) as u64))
                })?;
                account.quest_cooldowns = rows.collect::<Result<_, _>>()?;
            }
        }
        transaction.commit()?;
        Ok(accounts)
    }

    fn save(&mut self, account_id: u32, account: &Account) -> Result<(), PersistError> {
        let transaction = self.connection.transaction()?;
        for table in CHILD_TABLES {
            transaction.execute(
                &format!("DELETE FROM {table} WHERE account_id = ?1"),
                [account_id],
            )?;
        }
        transaction.execute(
            "INSERT INTO g26_accounts (
                account_id, name, clan_tag, generic_money, premium_money, premium_until,
                exp_total, quest_faction, rating, rating_before, active_kit, autobuy,
                next_item_id, next_quest_instance, tutorial_step, tutorial_done
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
             ON CONFLICT(account_id) DO UPDATE SET
                name = excluded.name, clan_tag = excluded.clan_tag,
                generic_money = excluded.generic_money, premium_money = excluded.premium_money,
                premium_until = excluded.premium_until, exp_total = excluded.exp_total,
                quest_faction = excluded.quest_faction, rating = excluded.rating,
                rating_before = excluded.rating_before, active_kit = excluded.active_kit,
                autobuy = excluded.autobuy, next_item_id = excluded.next_item_id,
                next_quest_instance = excluded.next_quest_instance,
                tutorial_step = excluded.tutorial_step, tutorial_done = excluded.tutorial_done",
            params![
                account_id,
                account.name,
                account.clan_tag,
                account.generic_money,
                account.premium_money,
                account.premium_until.min(i64::MAX as u64) as i64,
                account.exp_total,
                account.quest_faction,
                account.rating,
                account.rating_before,
                account.active_kit,
                account.autobuy,
                account.next_item_id,
                account.next_quest_instance,
                account.tutorial_step,
                account.tutorial_done,
            ],
        )?;
        extras::save(&transaction, account_id, &account.extras)?;
        map_plays::save(&transaction, account_id, &account.map_plays)?;
        balance::save(&transaction, account_id, account.balance)?;
        room_settings::save(&transaction, account_id, account.room_settings.as_ref())?;
        for (seq, &(skill_id, points)) in account.skills.iter().enumerate() {
            transaction.execute(
                "INSERT INTO g26_skills (account_id, seq, skill_id, points)
                 VALUES (?1, ?2, ?3, ?4)",
                params![account_id, seq as i64, skill_id, points],
            )?;
        }
        for (seq, &(faction, points)) in account.reputations.iter().enumerate() {
            transaction.execute(
                "INSERT INTO g26_reputations (account_id, seq, faction, points)
                 VALUES (?1, ?2, ?3, ?4)",
                params![account_id, seq as i64, faction, points],
            )?;
        }
        for (seq, item) in account.inventory.iter().enumerate() {
            transaction.execute(
                "INSERT INTO g26_items (
                    account_id, item_id, seq, dict_id, condition_or_stack, repair_time,
                    reroll_time, modifications, upgrades, reroll
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    account_id,
                    item.item_id,
                    seq as i64,
                    item.dict_id,
                    item.condition_or_stack,
                    item.repair_time,
                    item.reroll_time,
                    modifications_text(item),
                    upgrades_text(item),
                    items::reroll_text(item),
                ],
            )?;
        }
        for (seq, kit) in account.kits.iter().enumerate() {
            transaction.execute(
                "INSERT INTO g26_kits (account_id, id, seq, name, revision)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![account_id, kit.id, seq as i64, kit.name, kit.revision],
            )?;
            for (slot, held) in kit.slots.iter().enumerate() {
                if let Some(held) = held {
                    transaction.execute(
                        "INSERT INTO g26_kit_slots (account_id, kit_id, slot, item_id, count)
                         VALUES (?1, ?2, ?3, ?4, ?5)",
                        params![account_id, kit.id, slot as i64, held.item_id, held.count],
                    )?;
                }
            }
        }
        for (seq, quest) in account.quests.iter().enumerate() {
            transaction.execute(
                "INSERT INTO g26_quests (account_id, instance_id, seq, quest_id, games, expires_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    account_id,
                    quest.instance_id,
                    seq as i64,
                    quest.quest_id,
                    quest.games,
                    quest.expires_at.min(i64::MAX as u64) as i64,
                ],
            )?;
        }
        for (seq, &(quest_id, until)) in account.quest_cooldowns.iter().enumerate() {
            transaction.execute(
                "INSERT INTO g26_quest_cooldowns (account_id, seq, quest_id, until)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    account_id,
                    seq as i64,
                    quest_id,
                    until.min(i64::MAX as u64) as i64
                ],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    fn describe(&self) -> String {
        self.path.clone()
    }

    fn load_clans(&mut self) -> Result<Vec<Clan>, PersistError> {
        clans::load(&mut self.connection)
    }

    fn save_clan(&mut self, clan: &Clan) -> Result<(), PersistError> {
        clans::save(&mut self.connection, clan)
    }

    fn delete_clan(&mut self, clan_id: u32) -> Result<(), PersistError> {
        clans::delete(&mut self.connection, clan_id)
    }

    fn load_results(&mut self) -> Result<Vec<(String, String)>, PersistError> {
        results::load(&self.connection)
    }

    fn save_result(&mut self, key: &str, body: &str) -> Result<(), PersistError> {
        results::save(&self.connection, key, body)
    }

    fn delete_result(&mut self, key: &str) -> Result<(), PersistError> {
        results::delete(&self.connection, key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persist::SqliteRepository;
    use crate::v026g::quests;
    use crate::v026g::rules;
    use survarium::v026g::drops::Rng;

    /// A file of its own in the system's temporary directory, removed when
    /// dropped.
    struct TempDb(std::path::PathBuf);

    impl TempDb {
        fn new(name: &str) -> Self {
            let unique = format!(
                "srp-g26-{name}-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            );
            let dir = std::env::temp_dir().join(unique.replace(['(', ')'], ""));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        fn path(&self) -> std::path::PathBuf {
            self.0.join(DB_FILE)
        }
    }

    impl Drop for TempDb {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// An account with something in every list.
    fn busy_account() -> Account {
        let mut account = Account::new("Alice");
        account.clan_tag = "SRP".into();
        account.generic_money = 123_456;
        account.premium_until = 9_999_999_999;
        account.add_experience(40_000);
        account.add_reputation(2, 500);
        account.autobuy = true;
        account.tutorial_step = 3;
        account.tutorial_done = true;
        rules::set_skills(&mut account, &[(101, 2), (201, 1)]).unwrap();
        rules::rename_kit(&mut account, 2, "Sniper").unwrap();
        rules::set_active_kit(&mut account, 2, 1_000).unwrap();
        quests::choose_faction(&mut account, 2).unwrap();
        quests::refresh(&mut account, 1_000, &mut Rng::new(4));
        account.quest_cooldowns.push((10101, 5_000));
        account.inventory[0].condition_or_stack = 654_321;
        account.inventory[0].repair_time = 7;
        account.inventory[0].reroll_time = 8;
        account.inventory[0].modifications = vec![(1050, true), (7, false)];
        account.inventory[0].upgrades = vec![Upgrade {
            id: 5,
            a: 6,
            value: u64::MAX,
            param: 9,
        }];
        // What the later steps added: all of it must come back too.
        account.premium_money = 321;
        account.rating = 137;
        account.rating_before = 125;
        account.map_plays = vec![(1, 4), (6, 2)];
        account.balance = crate::v026g::balance::Balance {
            rating: -1250,
            matches: 9,
        };
        account.extras.auto_repair = true;
        account.extras.tutorial_flags = 0b101;
        account.extras.bonus.day = 3;
        account.extras.bonus.last_day = 20_373;
        account.extras.bonus.wins = [3, 3, 1, 0, 0, 0, 0];
        account.extras.bonus.reward_dict = 77;
        account.extras.bonus.reward_count = 50;
        account.extras.bonus.completed = true;
        account.extras.bonus.shown_day = 20_373;
        account.room_settings = Some(crate::v026g::room_settings::RoomSettings {
            size: 9,
            min_side: 2,
            max_level: 7,
            wait: 45,
            tiles_off: vec![(2, 1), (4, 3)],
            tiles_on: vec![(10, 1)],
            modes_off: vec![4],
        });
        account
    }

    #[test]
    fn an_account_comes_back_as_it_was_saved() {
        let temp = TempDb::new("round-trip");
        let account = busy_account();
        let other = Account::new("Bob");
        {
            let mut repository = SqliteGameRepository::open(&temp.path()).unwrap();
            assert_eq!(repository.version().unwrap(), MIGRATIONS.len() as i64);
            assert!(repository.load_all().unwrap().is_empty());
            repository.save(7, &account).unwrap();
            repository.save(3, &other).unwrap();
        }
        let mut repository = SqliteGameRepository::open(&temp.path()).unwrap();
        let loaded = repository.load_all().unwrap();
        assert_eq!(loaded, [(3, other), (7, account.clone())]);

        // A second save replaces the first whole.
        let mut changed = account;
        changed.inventory.remove(1);
        changed.sync_kits();
        changed.skills.clear();
        changed.quests.clear();
        repository.save(7, &changed).unwrap();
        let loaded = repository.load_all().unwrap();
        assert_eq!(loaded[1], (7, changed));
        assert_eq!(repository.describe(), temp.path().display().to_string());
    }

    /// One cosmetic worn by two items (patch 30): each item's record of it
    /// comes back, with its own slot and value.
    #[test]
    fn a_cosmetic_worn_by_two_items_comes_back_on_both() {
        let temp = TempDb::new("shared-upgrade");
        let mut account = Account::new("Alice");
        let cosmetic = account.inventory[2].item_id;
        let record = |param, value| Upgrade {
            id: cosmetic,
            a: account.inventory[2].dict_id.into(),
            value,
            param,
        };
        let (first, second) = (record(3, 0), record(0, 7));
        account.inventory[0].upgrades = vec![first];
        account.inventory[1].upgrades = vec![second];
        let mut repository = SqliteGameRepository::open(&temp.path()).unwrap();
        repository.save(7, &account).unwrap();
        let loaded = repository.load_all().unwrap();
        assert_eq!(loaded, [(7, account.clone())]);
        assert_eq!(loaded[0].1.inventory[0].upgrades, [first]);
        assert_eq!(loaded[0].1.inventory[1].upgrades, [second]);
    }

    #[test]
    fn the_tables_live_beside_021ds_in_one_file() {
        let temp = TempDb::new("shared");
        // 0.21d's repository makes and versions the file.
        let old = SqliteRepository::open(&temp.path()).unwrap();
        let old_version = old.version().unwrap();
        let mut repository = SqliteGameRepository::open(&temp.path()).unwrap();
        repository.save(1, &Account::new("Alice")).unwrap();
        // Its version is its own.
        assert_eq!(old.version().unwrap(), old_version);
        drop(old);
        // And 0.21d's opens the file again with the 0.26g tables in it.
        let old = SqliteRepository::open(&temp.path()).unwrap();
        assert_eq!(old.version().unwrap(), old_version);
        assert_eq!(repository.load_all().unwrap().len(), 1);
    }

    #[test]
    fn newer_tables_are_refused() {
        let temp = TempDb::new("newer");
        drop(SqliteGameRepository::open(&temp.path()).unwrap());
        let connection = Connection::open(temp.path()).unwrap();
        connection
            .execute("UPDATE g26_meta SET value = 99 WHERE key = 'schema'", [])
            .unwrap();
        drop(connection);
        assert!(matches!(
            SqliteGameRepository::open(&temp.path()),
            Err(PersistError::NewerSchema { found: 99, .. })
        ));
    }

    #[test]
    fn a_first_time_is_once_in_the_databases_life() {
        let temp = TempDb::new("first-time");
        let mut repository = SqliteGameRepository::open(&temp.path()).unwrap();
        assert!(repository.first_time(GEAR_MENDED).unwrap());
        assert!(!repository.first_time(GEAR_MENDED).unwrap());
        assert!(repository.first_time("another").unwrap());
        // It outlives the repository, and the schema's own row is as it was.
        let version = repository.version().unwrap();
        drop(repository);
        let mut repository = SqliteGameRepository::open(&temp.path()).unwrap();
        assert!(!repository.first_time(GEAR_MENDED).unwrap());
        assert_eq!(repository.version().unwrap(), version);
    }

    #[test]
    fn a_damaged_item_row_is_an_error_not_a_guess() {
        let temp = TempDb::new("damaged");
        let mut repository = SqliteGameRepository::open(&temp.path()).unwrap();
        repository.save(1, &busy_account()).unwrap();
        repository
            .connection
            .execute(
                "UPDATE g26_items SET upgrades = '5:6' WHERE item_id = 1",
                [],
            )
            .unwrap();
        assert!(matches!(
            repository.load_all(),
            Err(PersistError::BadRow(1, _))
        ));
    }

    #[test]
    fn the_memory_repository_keeps_accounts() {
        let mut repository = MemoryGameRepository::default();
        let shared = repository.clone();
        repository.save(2, &Account::new("Bob")).unwrap();
        repository.save(1, &Account::new("Alice")).unwrap();
        assert_eq!(shared.saves(), 2);
        assert_eq!(shared.get(1).unwrap().name, "Alice");
        let ids: Vec<u32> = repository
            .load_all()
            .unwrap()
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        assert_eq!(ids, [1, 2]);
    }
}
