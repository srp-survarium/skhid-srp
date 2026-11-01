//! Accounts in one SQLite file, in WAL mode. The schema is versioned by
//! `PRAGMA user_version`: version N means the first N of [`MIGRATIONS`] were
//! applied. Lists keep their order in a `seq` column.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OptionalExtension, Transaction, params};
use survarium::dictionary::PROFILE_SLOTS;
use survarium::v021d::account::{Account as GameAccount, Item, Profile};
use survarium::v021d::quests::QuestInstance;

use crate::{Ban, IpBan, Net};

use super::{
    AccountRecord, AccountRepository, BACKUPS_KEPT, Loaded, PersistError, StoredSession,
    prune_backups,
};

/// The schema, step by step. Never change a step that shipped: add one.
pub(super) const MIGRATIONS: &[&str] = &[
    // 1: accounts and everything they own.
    "
    CREATE TABLE meta (
        key   TEXT PRIMARY KEY,
        value INTEGER NOT NULL
    );
    CREATE TABLE accounts (
        id                  INTEGER PRIMARY KEY,
        -- The user name in lower case (see persist::user_name_key).
        login               TEXT NOT NULL UNIQUE COLLATE NOCASE,
        -- argon2id PHC string; NULL: the next sign-in sets it.
        password_hash       TEXT,
        name                TEXT NOT NULL,
        game_name           TEXT NOT NULL,
        generic_money       INTEGER NOT NULL,
        premium_money       INTEGER NOT NULL,
        premium_until       INTEGER NOT NULL,
        rating              INTEGER NOT NULL,
        rating_before       INTEGER NOT NULL,
        next_item_id        INTEGER NOT NULL,
        next_quest_instance INTEGER NOT NULL,
        active_profile      INTEGER
    );
    CREATE TABLE loyalty (
        account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
        faction    INTEGER NOT NULL,
        percent    INTEGER NOT NULL,
        PRIMARY KEY (account_id, faction)
    );
    CREATE TABLE reputations (
        account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
        seq        INTEGER NOT NULL,
        faction    INTEGER NOT NULL,
        points     INTEGER NOT NULL,
        PRIMARY KEY (account_id, seq)
    );
    CREATE TABLE inventory (
        account_id         INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
        item_id            INTEGER NOT NULL,
        seq                INTEGER NOT NULL,
        dict_id            INTEGER NOT NULL,
        condition_or_stack INTEGER NOT NULL,
        amount             INTEGER NOT NULL,
        -- Modification ids, comma-separated; empty for none.
        modifications      TEXT NOT NULL,
        PRIMARY KEY (account_id, item_id)
    );
    CREATE TABLE profiles (
        account_id      INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
        id              INTEGER NOT NULL,
        seq             INTEGER NOT NULL,
        name            TEXT NOT NULL,
        autobuy         INTEGER NOT NULL,
        level           INTEGER NOT NULL,
        exp_total       INTEGER NOT NULL,
        exp_level_start INTEGER NOT NULL,
        exp_next_level  INTEGER NOT NULL,
        revision        INTEGER NOT NULL,
        PRIMARY KEY (account_id, id)
    );
    -- A slot holds an inventory item; its own count and amount may differ
    -- from the inventory's (see Account::sync_stacks).
    CREATE TABLE profile_slots (
        account_id         INTEGER NOT NULL,
        profile_id         INTEGER NOT NULL,
        slot               INTEGER NOT NULL,
        item_id            INTEGER NOT NULL,
        condition_or_stack INTEGER NOT NULL,
        amount             INTEGER NOT NULL,
        PRIMARY KEY (account_id, profile_id, slot),
        FOREIGN KEY (account_id, profile_id)
            REFERENCES profiles(account_id, id) ON DELETE CASCADE,
        FOREIGN KEY (account_id, item_id)
            REFERENCES inventory(account_id, item_id) ON DELETE CASCADE
    );
    CREATE TABLE profile_skills (
        account_id INTEGER NOT NULL,
        profile_id INTEGER NOT NULL,
        seq        INTEGER NOT NULL,
        skill_id   INTEGER NOT NULL,
        points     INTEGER NOT NULL,
        PRIMARY KEY (account_id, profile_id, seq),
        FOREIGN KEY (account_id, profile_id)
            REFERENCES profiles(account_id, id) ON DELETE CASCADE
    );
    CREATE TABLE profile_perks (
        account_id INTEGER NOT NULL,
        profile_id INTEGER NOT NULL,
        seq        INTEGER NOT NULL,
        perk       INTEGER NOT NULL,
        PRIMARY KEY (account_id, profile_id, seq),
        FOREIGN KEY (account_id, profile_id)
            REFERENCES profiles(account_id, id) ON DELETE CASCADE
    );
    CREATE TABLE quests (
        account_id  INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
        instance_id INTEGER NOT NULL,
        seq         INTEGER NOT NULL,
        quest_id    INTEGER NOT NULL,
        games       INTEGER NOT NULL,
        expires_at  INTEGER NOT NULL,
        active      INTEGER NOT NULL,
        PRIMARY KEY (account_id, instance_id)
    );
    CREATE TABLE quest_cooldowns (
        account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
        seq        INTEGER NOT NULL,
        quest_id   INTEGER NOT NULL,
        until      INTEGER NOT NULL,
        PRIMARY KEY (account_id, seq)
    );
    -- Friends and ignored accounts are not foreign keys: the other account
    -- may be saved later, in its own transaction.
    CREATE TABLE friends (
        account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
        seq        INTEGER NOT NULL,
        friend_id  INTEGER NOT NULL,
        PRIMARY KEY (account_id, seq)
    );
    CREATE TABLE ignored (
        account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
        seq        INTEGER NOT NULL,
        ignored_id INTEGER NOT NULL,
        PRIMARY KEY (account_id, seq)
    );
    CREATE TABLE notices (
        account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
        seq        INTEGER NOT NULL,
        kind       INTEGER NOT NULL,
        text       TEXT NOT NULL,
        PRIMARY KEY (account_id, seq)
    );
    ",
    // 2: sessions that have not ended, so a client in the lobby carries on
    // after a restart. Only the account an id maps to: no secret.
    "
    CREATE TABLE sessions (
        session_id INTEGER PRIMARY KEY,
        account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
        -- The last sighting, Unix seconds.
        last_seen  INTEGER NOT NULL
    );
    ",
    // 3: bans, and where each account last signed in from.
    "
    -- banned = 1: it may not sign in; until NULL means for good.
    ALTER TABLE accounts ADD COLUMN banned       INTEGER NOT NULL DEFAULT 0;
    ALTER TABLE accounts ADD COLUMN banned_until INTEGER;
    ALTER TABLE accounts ADD COLUMN ban_reason   TEXT NOT NULL DEFAULT '';
    ALTER TABLE accounts ADD COLUMN last_ip      TEXT;
    CREATE TABLE ip_bans (
        -- An address, or a range: 10.0.0.0/8.
        net    TEXT PRIMARY KEY,
        until  INTEGER,
        reason TEXT NOT NULL
    );
    ",
    // 4: preserve wire compatibility when a lobby session survives a restart.
    "ALTER TABLE sessions ADD COLUMN client_revision INTEGER NOT NULL DEFAULT 0;",
    // 5: existing ignores remain hidden; new mutes can show a placeholder.
    "ALTER TABLE ignored ADD COLUMN mode INTEGER NOT NULL DEFAULT 1 CHECK (mode IN (1, 2));",
    // 6: the admin's chat mute (admin-chat-mute.md). chat_ban_reason NULL:
    // none; else chat_ban_until NULL means for good.
    "
    ALTER TABLE accounts ADD COLUMN chat_ban_reason TEXT;
    ALTER TABLE accounts ADD COLUMN chat_ban_until  INTEGER;
    ",
];

/// The child tables of an account, children of children first.
const CHILD_TABLES: &[&str] = &[
    "profile_slots",
    "profile_skills",
    "profile_perks",
    "profiles",
    "inventory",
    "loyalty",
    "reputations",
    "quests",
    "quest_cooldowns",
    "friends",
    "ignored",
    "notices",
];

pub struct SqliteRepository {
    connection: Connection,
    path: String,
}

impl SqliteRepository {
    /// Open (or create) the database at `path` and bring it to the newest
    /// schema. Refuses a database newer than this code.
    pub fn open(path: &Path) -> Result<Self, PersistError> {
        let connection = connect(path)?;
        migrate(&connection, MIGRATIONS.len())?;
        Ok(Self {
            connection,
            path: path.display().to_string(),
        })
    }

    /// [`Self::open`], after a copy of an existing database went to
    /// `backups/srp-<unix>.db` (the newest [`BACKUPS_KEPT`] are kept). The
    /// copy is made with `VACUUM INTO`, so it holds what the write-ahead log
    /// had too.
    pub fn open_with_backup(path: &Path, backups: &Path) -> Result<Self, PersistError> {
        if path.exists() {
            let connection = connect(path)?;
            check_version(&connection)?;
            backup(&connection, backups);
        }
        Self::open(path)
    }

    /// The schema version of the file.
    pub fn version(&self) -> Result<i64, PersistError> {
        user_version(&self.connection)
    }
}

fn connect(path: &Path) -> Result<Connection, PersistError> {
    let connection = Connection::open(path)?;
    let _mode: String = connection.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))?;
    connection.execute_batch(
        "PRAGMA foreign_keys = ON;
         PRAGMA synchronous = NORMAL;
         PRAGMA busy_timeout = 5000;",
    )?;
    Ok(connection)
}

fn user_version(connection: &Connection) -> Result<i64, PersistError> {
    Ok(connection.query_row("PRAGMA user_version", [], |row| row.get(0))?)
}

fn check_version(connection: &Connection) -> Result<i64, PersistError> {
    let found = user_version(connection)?;
    let known = MIGRATIONS.len() as i64;
    if found > known {
        return Err(PersistError::NewerSchema { found, known });
    }
    Ok(found)
}

/// Apply the steps the file lacks, up to `target`, each in its own
/// transaction with its version.
pub(super) fn migrate(connection: &Connection, target: usize) -> Result<(), PersistError> {
    let found = check_version(connection)? as usize;
    for (index, step) in MIGRATIONS.iter().enumerate().take(target).skip(found) {
        let transaction = connection.unchecked_transaction()?;
        transaction.execute_batch(step)?;
        transaction.pragma_update(None, "user_version", index as i64 + 1)?;
        transaction.commit()?;
        log::info!("Database schema upgraded to version {}", index + 1);
    }
    Ok(())
}

fn backup(connection: &Connection, dir: &Path) {
    if let Err(error) = std::fs::create_dir_all(dir) {
        log::warn!("No backup: {}: {error}", dir.display());
        return;
    }
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let target = dir.join(format!("srp-{stamp}.db"));
    if target.exists() {
        // Restarted within the second: that backup is as good.
        return;
    }
    match connection.execute("VACUUM INTO ?1", [target.to_string_lossy()]) {
        Ok(_) => log::info!("Backed up the database to {}", target.display()),
        Err(error) => log::warn!("No backup to {}: {error}", target.display()),
    }
    prune_backups(dir, BACKUPS_KEPT);
}

/// The chat mute of the columns `chat_ban_reason`, `chat_ban_until`
/// (migration 6): none without a reason.
fn chat_ban(reason: Option<String>, until: Option<i64>) -> Option<Ban> {
    Some(Ban {
        until: until.map(|until| until.max(0) as u64),
        reason: reason?,
    })
}

fn meta_next_account_id(connection: &Connection) -> Result<u32, PersistError> {
    let next: Option<i64> = connection
        .query_row(
            "SELECT value FROM meta WHERE key = 'next_account_id'",
            [],
            |row| row.get(0),
        )
        .optional()?;
    Ok(next.unwrap_or(1).clamp(1, i64::from(u32::MAX)) as u32)
}

impl AccountRepository for SqliteRepository {
    fn load_all(&mut self) -> Result<Loaded, PersistError> {
        let transaction = self.connection.transaction()?;
        let mut accounts = Vec::new();
        {
            let mut statement = transaction.prepare(
                "SELECT id, login, password_hash, name, game_name, generic_money,
                        premium_money, premium_until, rating, rating_before,
                        next_item_id, next_quest_instance, active_profile,
                        banned, banned_until, ban_reason, last_ip,
                        chat_ban_reason, chat_ban_until
                 FROM accounts ORDER BY id",
            )?;
            let rows = statement.query_map([], |row| {
                Ok(AccountRecord {
                    id: row.get(0)?,
                    login: row.get(1)?,
                    password_hash: row.get(2)?,
                    name: row.get(3)?,
                    game: GameAccount {
                        name: row.get(4)?,
                        generic_money: row.get(5)?,
                        premium_money: row.get(6)?,
                        premium_until: row.get::<_, i64>(7)?.max(0) as u64,
                        reputations: Vec::new(),
                        loyalty: [0; 4],
                        rating: row.get(8)?,
                        rating_before: row.get(9)?,
                        profiles: Vec::new(),
                        inventory: Vec::new(),
                        next_item_id: row.get(10)?,
                        quests: Vec::new(),
                        quest_cooldowns: Vec::new(),
                        next_quest_instance: row.get(11)?,
                    },
                    friends: Vec::new(),
                    ignored: crate::chat_mutes::ChatMutes::default(),
                    active_profile: row.get(12)?,
                    notices: Vec::new(),
                    ban: row
                        .get::<_, bool>(13)?
                        .then(|| -> rusqlite::Result<Ban> {
                            Ok(Ban {
                                until: row
                                    .get::<_, Option<i64>>(14)?
                                    .map(|until| until.max(0) as u64),
                                reason: row.get(15)?,
                            })
                        })
                        .transpose()?,
                    last_ip: row
                        .get::<_, Option<String>>(16)?
                        .and_then(|ip| ip.parse().ok()),
                    chat_ban: chat_ban(row.get(17)?, row.get(18)?),
                })
            })?;
            for row in rows {
                accounts.push(row?);
            }
        }
        for record in &mut accounts {
            load_children(&transaction, record)?;
        }
        let stored = meta_next_account_id(&transaction)?;
        let past_ids = accounts
            .iter()
            .map(|record| record.id + 1)
            .max()
            .unwrap_or(1);
        transaction.commit()?;
        Ok(Loaded {
            accounts,
            next_account_id: stored.max(past_ids),
        })
    }

    fn save_account(&mut self, record: &AccountRecord) -> Result<(), PersistError> {
        let transaction = self.connection.transaction()?;
        write_account(&transaction, record)?;
        transaction.commit()?;
        Ok(())
    }

    fn next_account_id(&mut self) -> Result<u32, PersistError> {
        meta_next_account_id(&self.connection)
    }

    fn load_sessions(&mut self) -> Result<Vec<StoredSession>, PersistError> {
        let mut statement = self.connection.prepare(
            "SELECT session_id, account_id, last_seen, client_revision FROM sessions ORDER BY session_id",
        )?;
        let sessions = statement
            .query_map([], |row| {
                Ok(StoredSession {
                    session_id: row.get(0)?,
                    account_id: row.get(1)?,
                    last_seen_unix: row.get::<_, i64>(2)?.max(0) as u64,
                    client_revision: row.get(3)?,
                })
            })?
            .collect::<Result<_, _>>()?;
        Ok(sessions)
    }

    fn save_session(&mut self, session: &StoredSession) -> Result<(), PersistError> {
        self.connection.execute(
            "INSERT INTO sessions (session_id, account_id, last_seen, client_revision) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT (session_id) DO UPDATE SET
                 account_id = excluded.account_id, last_seen = excluded.last_seen,
                 client_revision = excluded.client_revision",
            params![
                session.session_id,
                session.account_id,
                session.last_seen_unix as i64,
                session.client_revision
            ],
        )?;
        Ok(())
    }

    fn delete_session(&mut self, session_id: u32) -> Result<(), PersistError> {
        self.connection
            .execute("DELETE FROM sessions WHERE session_id = ?1", [session_id])?;
        Ok(())
    }

    fn load_ip_bans(&mut self) -> Result<Vec<IpBan>, PersistError> {
        let mut statement = self
            .connection
            .prepare("SELECT net, until, reason FROM ip_bans ORDER BY net")?;
        let rows: Vec<(String, Option<i64>, String)> = statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
            .collect::<Result<_, _>>()?;
        Ok(rows
            .into_iter()
            .filter_map(|(net, until, reason)| {
                let Some(net) = Net::parse(&net) else {
                    log::warn!("Ignored an address ban of {net:?}: not an address");
                    return None;
                };
                Some(IpBan {
                    net,
                    ban: Ban {
                        until: until.map(|until| until.max(0) as u64),
                        reason,
                    },
                })
            })
            .collect())
    }

    fn save_ip_ban(&mut self, ban: &IpBan) -> Result<(), PersistError> {
        self.connection.execute(
            "INSERT INTO ip_bans (net, until, reason) VALUES (?1, ?2, ?3)
             ON CONFLICT (net) DO UPDATE SET until = excluded.until, reason = excluded.reason",
            params![
                ban.net.to_string(),
                ban.ban.until.map(|until| until.min(i64::MAX as u64) as i64),
                ban.ban.reason,
            ],
        )?;
        Ok(())
    }

    fn delete_ip_ban(&mut self, net: &Net) -> Result<(), PersistError> {
        self.connection
            .execute("DELETE FROM ip_bans WHERE net = ?1", [net.to_string()])?;
        Ok(())
    }

    fn describe(&self) -> String {
        self.path.clone()
    }
}

fn load_children(
    transaction: &Transaction,
    record: &mut AccountRecord,
) -> Result<(), PersistError> {
    let id = record.id;
    let game = &mut record.game;

    let mut statement =
        transaction.prepare_cached("SELECT faction, percent FROM loyalty WHERE account_id = ?1")?;
    let rows = statement.query_map([id], |row| Ok((row.get::<_, u8>(0)?, row.get::<_, u8>(1)?)))?;
    for row in rows {
        let (faction, percent) = row?;
        match game.loyalty.get_mut(usize::from(faction).wrapping_sub(1)) {
            Some(loyalty) => *loyalty = percent,
            None => {
                return Err(PersistError::BadRow(
                    id,
                    format!("loyalty faction {faction}"),
                ));
            }
        }
    }

    let mut statement = transaction.prepare_cached(
        "SELECT faction, points FROM reputations WHERE account_id = ?1 ORDER BY seq",
    )?;
    game.reputations = statement
        .query_map([id], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<_, _>>()?;

    let mut statement = transaction.prepare_cached(
        "SELECT item_id, dict_id, condition_or_stack, amount, modifications
         FROM inventory WHERE account_id = ?1 ORDER BY seq",
    )?;
    let rows = statement.query_map([id], |row| {
        Ok((
            row.get::<_, u32>(0)?,
            row.get::<_, u16>(1)?,
            row.get::<_, u16>(2)?,
            row.get::<_, u32>(3)?,
            row.get::<_, String>(4)?,
        ))
    })?;
    for row in rows {
        let (item_id, dict_id, condition_or_stack, amount, modifications) = row?;
        let modifications = parse_ids(&modifications)
            .ok_or_else(|| PersistError::BadRow(id, format!("item {item_id} modifications")))?;
        game.inventory.push(Item {
            dict_id,
            item_id,
            condition_or_stack,
            amount,
            modifications,
        });
    }

    let mut statement = transaction.prepare_cached(
        "SELECT id, name, autobuy, level, exp_total, exp_level_start, exp_next_level, revision
         FROM profiles WHERE account_id = ?1 ORDER BY seq",
    )?;
    game.profiles = statement
        .query_map([id], |row| {
            Ok(Profile {
                id: row.get(0)?,
                name: row.get(1)?,
                autobuy: row.get(2)?,
                level: row.get(3)?,
                exp_total: row.get(4)?,
                exp_level_start: row.get(5)?,
                exp_next_level: row.get(6)?,
                revision: row.get(7)?,
                slots: Default::default(),
                skills: Vec::new(),
                perks: Vec::new(),
            })
        })?
        .collect::<Result<_, _>>()?;

    for profile in &mut game.profiles {
        let mut statement = transaction.prepare_cached(
            "SELECT slot, item_id, condition_or_stack, amount
             FROM profile_slots WHERE account_id = ?1 AND profile_id = ?2",
        )?;
        let rows = statement.query_map([id, profile.id], |row| {
            Ok((
                row.get::<_, u32>(0)?,
                row.get::<_, u32>(1)?,
                row.get::<_, u16>(2)?,
                row.get::<_, u32>(3)?,
            ))
        })?;
        for row in rows {
            let (slot, item_id, condition_or_stack, amount) = row?;
            let slot = slot as usize;
            if slot >= PROFILE_SLOTS {
                log::warn!(
                    "Account {id}: profile {} has no slot {slot}; dropped",
                    profile.id
                );
                continue;
            }
            // The rest of the item is the inventory's; a slot whose item is
            // gone is dropped by `AccountRecord::repair`.
            let Some(owned) = game.inventory.iter().find(|item| item.item_id == item_id) else {
                continue;
            };
            profile.slots[slot] = Some(Item {
                condition_or_stack,
                amount,
                ..owned.clone()
            });
        }

        let mut statement = transaction.prepare_cached(
            "SELECT skill_id, points FROM profile_skills
             WHERE account_id = ?1 AND profile_id = ?2 ORDER BY seq",
        )?;
        profile.skills = statement
            .query_map([id, profile.id], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<_, _>>()?;

        let mut statement = transaction.prepare_cached(
            "SELECT perk FROM profile_perks
             WHERE account_id = ?1 AND profile_id = ?2 ORDER BY seq",
        )?;
        profile.perks = statement
            .query_map([id, profile.id], |row| row.get(0))?
            .collect::<Result<_, _>>()?;
    }

    let mut statement = transaction.prepare_cached(
        "SELECT instance_id, quest_id, games, expires_at, active
         FROM quests WHERE account_id = ?1 ORDER BY seq",
    )?;
    game.quests = statement
        .query_map([id], |row| {
            Ok(QuestInstance {
                instance_id: row.get(0)?,
                quest_id: row.get(1)?,
                games: row.get(2)?,
                expires_at: row.get::<_, i64>(3)?.max(0) as u64,
                active: row.get(4)?,
            })
        })?
        .collect::<Result<_, _>>()?;

    let mut statement = transaction.prepare_cached(
        "SELECT quest_id, until FROM quest_cooldowns WHERE account_id = ?1 ORDER BY seq",
    )?;
    game.quest_cooldowns = statement
        .query_map([id], |row| {
            Ok((row.get(0)?, row.get::<_, i64>(1)?.max(0) as u64))
        })?
        .collect::<Result<_, _>>()?;

    let mut statement = transaction
        .prepare_cached("SELECT friend_id FROM friends WHERE account_id = ?1 ORDER BY seq")?;
    record.friends = statement
        .query_map([id], |row| row.get(0))?
        .collect::<Result<_, _>>()?;

    let mut statement = transaction.prepare_cached(
        "SELECT ignored_id, mode FROM ignored WHERE account_id = ?1 ORDER BY seq",
    )?;
    for entry in statement.query_map([id], |row| {
        Ok((row.get::<_, u32>(0)?, row.get::<_, u8>(1)?))
    })? {
        let (target, mode) = entry?;
        let mode = match mode {
            1 => crate::chat_mutes::ChatMuteMode::Hidden,
            2 => crate::chat_mutes::ChatMuteMode::Placeholder,
            _ => return Err(PersistError::BadRow(id, "unknown chat mute mode".into())),
        };
        record.ignored.set(target, Some(mode));
    }

    let mut statement = transaction
        .prepare_cached("SELECT kind, text FROM notices WHERE account_id = ?1 ORDER BY seq")?;
    record.notices = statement
        .query_map([id], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<_, _>>()?;
    Ok(())
}

/// Replace everything kept of one account with `record`.
///
/// Every struct is taken apart field by field, with no `..`: a field added
/// to an account, profile, item or quest doesn't compile here until it is
/// kept (or marked as not kept, with `_`).
fn write_account(transaction: &Transaction, record: &AccountRecord) -> Result<(), PersistError> {
    let AccountRecord {
        id,
        login,
        password_hash,
        name,
        game,
        friends,
        ignored,
        active_profile,
        notices,
        ban,
        chat_ban,
        last_ip,
    } = record;
    let GameAccount {
        name: game_name,
        generic_money,
        premium_money,
        premium_until,
        reputations,
        loyalty,
        rating,
        rating_before,
        profiles,
        inventory,
        next_item_id,
        quests,
        quest_cooldowns,
        next_quest_instance,
    } = game;
    let id = *id;
    transaction.execute(
        "INSERT INTO accounts (id, login, password_hash, name, game_name, generic_money,
                               premium_money, premium_until, rating, rating_before,
                               next_item_id, next_quest_instance, active_profile,
                               banned, banned_until, ban_reason, last_ip,
                               chat_ban_reason, chat_ban_until)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
                 ?14, ?15, ?16, ?17, ?18, ?19)
         ON CONFLICT (id) DO UPDATE SET
             login = excluded.login, password_hash = excluded.password_hash,
             name = excluded.name, game_name = excluded.game_name,
             generic_money = excluded.generic_money, premium_money = excluded.premium_money,
             premium_until = excluded.premium_until, rating = excluded.rating,
             rating_before = excluded.rating_before, next_item_id = excluded.next_item_id,
             next_quest_instance = excluded.next_quest_instance,
             active_profile = excluded.active_profile,
             banned = excluded.banned, banned_until = excluded.banned_until,
             ban_reason = excluded.ban_reason, last_ip = excluded.last_ip,
             chat_ban_reason = excluded.chat_ban_reason,
             chat_ban_until = excluded.chat_ban_until",
        params![
            id,
            login,
            password_hash,
            name,
            game_name,
            generic_money,
            premium_money,
            *premium_until as i64,
            rating,
            rating_before,
            next_item_id,
            next_quest_instance,
            active_profile,
            ban.is_some(),
            ban.as_ref()
                .and_then(|ban| ban.until)
                .map(|until| until.min(i64::MAX as u64) as i64),
            ban.as_ref().map_or("", |ban| ban.reason.as_str()),
            last_ip.map(|ip| ip.to_string()),
            chat_ban.as_ref().map(|ban| ban.reason.as_str()),
            chat_ban
                .as_ref()
                .and_then(|ban| ban.until)
                .map(|until| until.min(i64::MAX as u64) as i64),
        ],
    )?;
    for table in CHILD_TABLES {
        transaction.execute(&format!("DELETE FROM {table} WHERE account_id = ?1"), [id])?;
    }

    let mut statement = transaction
        .prepare_cached("INSERT INTO loyalty (account_id, faction, percent) VALUES (?1, ?2, ?3)")?;
    for (index, percent) in loyalty.iter().enumerate() {
        statement.execute(params![id, index as u32 + 1, percent])?;
    }

    let mut statement = transaction.prepare_cached(
        "INSERT INTO reputations (account_id, seq, faction, points) VALUES (?1, ?2, ?3, ?4)",
    )?;
    for (seq, (faction, points)) in reputations.iter().enumerate() {
        statement.execute(params![id, seq as u32, faction, points])?;
    }

    let mut statement = transaction.prepare_cached(
        "INSERT INTO inventory (account_id, item_id, seq, dict_id, condition_or_stack,
                                amount, modifications)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )?;
    for (seq, item) in inventory.iter().enumerate() {
        let Item {
            dict_id,
            item_id,
            condition_or_stack,
            amount,
            modifications,
        } = item;
        statement.execute(params![
            id,
            item_id,
            seq as u32,
            dict_id,
            condition_or_stack,
            amount,
            join_ids(modifications),
        ])?;
    }

    for (seq, profile) in profiles.iter().enumerate() {
        let Profile {
            id: profile_id,
            name,
            autobuy,
            level,
            exp_total,
            exp_level_start,
            exp_next_level,
            revision,
            slots,
            skills,
            perks,
        } = profile;
        transaction
            .prepare_cached(
                "INSERT INTO profiles (account_id, id, seq, name, autobuy, level, exp_total,
                                       exp_level_start, exp_next_level, revision)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            )?
            .execute(params![
                id,
                profile_id,
                seq as u32,
                name,
                autobuy,
                level,
                exp_total,
                exp_level_start,
                exp_next_level,
                revision,
            ])?;
        for (slot, item) in slots.iter().enumerate() {
            let Some(item) = item else {
                continue;
            };
            // The item's kind and modifications are the inventory's; the
            // slot keeps its own count and amount.
            let Item {
                dict_id: _,
                item_id,
                condition_or_stack,
                amount,
                modifications: _,
            } = item;
            // A slot can only hold what the inventory has; one that doesn't
            // would not load either.
            if !inventory.iter().any(|owned| owned.item_id == *item_id) {
                continue;
            }
            transaction
                .prepare_cached(
                    "INSERT INTO profile_slots (account_id, profile_id, slot, item_id,
                                                condition_or_stack, amount)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                )?
                .execute(params![
                    id,
                    profile_id,
                    slot as u32,
                    item_id,
                    condition_or_stack,
                    amount,
                ])?;
        }
        for (seq, (skill_id, points)) in skills.iter().enumerate() {
            transaction
                .prepare_cached(
                    "INSERT INTO profile_skills (account_id, profile_id, seq, skill_id, points)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                )?
                .execute(params![id, profile_id, seq as u32, skill_id, points])?;
        }
        for (seq, perk) in perks.iter().enumerate() {
            transaction
                .prepare_cached(
                    "INSERT INTO profile_perks (account_id, profile_id, seq, perk)
                     VALUES (?1, ?2, ?3, ?4)",
                )?
                .execute(params![id, profile_id, seq as u32, perk])?;
        }
    }

    let mut statement = transaction.prepare_cached(
        "INSERT INTO quests (account_id, instance_id, seq, quest_id, games, expires_at, active)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )?;
    for (seq, quest) in quests.iter().enumerate() {
        let QuestInstance {
            instance_id,
            quest_id,
            games,
            expires_at,
            active,
        } = quest;
        statement.execute(params![
            id,
            instance_id,
            seq as u32,
            quest_id,
            games,
            *expires_at as i64,
            active,
        ])?;
    }

    let mut statement = transaction.prepare_cached(
        "INSERT INTO quest_cooldowns (account_id, seq, quest_id, until) VALUES (?1, ?2, ?3, ?4)",
    )?;
    for (seq, (quest_id, until)) in quest_cooldowns.iter().enumerate() {
        statement.execute(params![id, seq as u32, quest_id, *until as i64])?;
    }

    let mut statement = transaction
        .prepare_cached("INSERT INTO friends (account_id, seq, friend_id) VALUES (?1, ?2, ?3)")?;
    for (seq, friend) in friends.iter().enumerate() {
        statement.execute(params![id, seq as u32, friend])?;
    }

    let mut statement = transaction.prepare_cached(
        "INSERT INTO ignored (account_id, seq, ignored_id, mode) VALUES (?1, ?2, ?3, ?4)",
    )?;
    for (seq, (ignored, mode)) in ignored.entries().enumerate() {
        statement.execute(params![id, seq as u32, ignored, mode as u8])?;
    }

    let mut statement = transaction.prepare_cached(
        "INSERT INTO notices (account_id, seq, kind, text) VALUES (?1, ?2, ?3, ?4)",
    )?;
    for (seq, (kind, text)) in notices.iter().enumerate() {
        statement.execute(params![id, seq as u32, kind, text])?;
    }

    transaction.execute(
        "INSERT INTO meta (key, value) VALUES ('next_account_id', ?1)
         ON CONFLICT (key) DO UPDATE SET value = max(value, excluded.value)",
        [i64::from(id) + 1],
    )?;
    Ok(())
}

fn join_ids(ids: &[u32]) -> String {
    let ids: Vec<String> = ids.iter().map(u32::to_string).collect();
    ids.join(",")
}

fn parse_ids(text: &str) -> Option<Vec<u32>> {
    if text.is_empty() {
        return Some(Vec::new());
    }
    text.split(',').map(|id| id.trim().parse().ok()).collect()
}
