//! Accounts kept across restarts.
//!
//! The store hands every durable field of an account to an
//! [`AccountRepository`] as a plain [`AccountRecord`]; the repository decides
//! where it goes. [`SqliteRepository`] keeps them in one SQLite file (srp's
//! default, see [`open_from_env`]); [`MemoryRepository`] keeps them in memory,
//! for tests. A Postgres repository would be a third implementation.
//!
//! Sessions that have not ended are kept too, as a [`StoredSession`] (the
//! account a session id maps to, and when it was last seen), so a client
//! still in the lobby carries on after srp restarts.
//!
//! Not kept: connections, the queue and formed matches, match reports and
//! results, squads and a Play waiting for its squad. They end with the
//! process, as they do when a client restarts.

use std::collections::HashMap;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use survarium::v021d;

use crate::{Account, Ban, IpBan, LockExt};

mod sqlite;
pub use sqlite::SqliteRepository;

/// Where srp keeps its database: `SRP_DATA_DIR`, by default `./data`.
pub const DATA_DIR_VAR: &str = "SRP_DATA_DIR";
/// `SRP_DB=off` (or empty) keeps every account in memory only, as before.
pub const DB_VAR: &str = "SRP_DB";
pub const DEFAULT_DATA_DIR: &str = "data";
pub const DB_FILE: &str = "srp.db";
/// Startup backups kept in `<data dir>/backups`, newest first.
pub const BACKUPS_KEPT: usize = 10;

#[derive(Debug, thiserror::Error)]
pub enum PersistError {
    #[error("database: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("{0}: {1}")]
    Io(PathBuf, std::io::Error),
    #[error(
        "the database is version {found}, newer than this srp knows ({known}); \
         run a newer srp or restore a backup"
    )]
    NewerSchema { found: i64, known: i64 },
    #[error("account {0}: {1}")]
    BadRow(u32, String),
    /// The kept accounts name a definition the game's data lacks
    /// (`v026g::known`): srp does not start.
    #[error("{0}")]
    GameData(String),
}

/// Every durable field of one account, copied out of the store so that it
/// can be written without the account locked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountRecord {
    pub id: u32,
    /// What the account is found by at sign-in (see [`login_key`]).
    pub login: String,
    /// The argon2id PHC string of its password; `None` until its next
    /// sign-in sets one (see `SessionStore::sign_in`).
    pub password_hash: Option<String>,
    pub name: String,
    pub game: v021d::account::Account,
    pub friends: Vec<u32>,
    pub ignored: Vec<u32>,
    pub active_profile: Option<u32>,
    pub notices: Vec<(u8, String)>,
    pub ban: Option<Ban>,
    pub last_ip: Option<IpAddr>,
}

impl AccountRecord {
    /// The durable fields of `account`. It is taken apart field by field,
    /// with no `..`: a new field doesn't compile here until it is kept, or
    /// named as not kept.
    pub fn of(account: &Account) -> Self {
        let Account {
            id,
            login,
            password_hash,
            name,
            game,
            friends,
            ignored,
            active_profile,
            // Not kept: a Play waiting for its squad ends with the process.
            play: _,
            notices,
            ban,
            last_ip,
            // Not kept: only the admin's password reset shows the first,
            // and the second orders sign-ins against resets while srp runs.
            last_sign_in: _,
            password_epoch: _,
        } = account;
        Self {
            id: *id,
            login: login.clone(),
            password_hash: password_hash.clone(),
            name: name.clone(),
            game: game.clone(),
            friends: friends.clone(),
            ignored: ignored.clone(),
            active_profile: *active_profile,
            notices: notices.clone(),
            ban: ban.clone(),
            last_ip: *last_ip,
        }
    }

    pub fn into_account(self) -> Account {
        Account {
            id: self.id,
            login: self.login,
            password_hash: self.password_hash,
            name: self.name,
            game: self.game,
            friends: self.friends,
            ignored: self.ignored,
            active_profile: self.active_profile,
            play: None,
            notices: self.notices,
            ban: self.ban,
            last_ip: self.last_ip,
            last_sign_in: None,
            password_epoch: 0,
        }
    }

    /// Make a loaded account consistent: the next ids past every id in use,
    /// and no profile slot holding an item the inventory lost. `true` if
    /// anything changed.
    pub fn repair(&mut self) -> bool {
        let game = &mut self.game;
        let mut changed = false;
        let next_item = game.inventory.iter().map(|item| item.item_id + 1).max();
        if let Some(next) = next_item.filter(|&next| next > game.next_item_id) {
            game.next_item_id = next;
            changed = true;
        }
        let next_quest = game.quests.iter().map(|quest| quest.instance_id + 1).max();
        if let Some(next) = next_quest.filter(|&next| next > game.next_quest_instance) {
            game.next_quest_instance = next;
            changed = true;
        }
        let inventory = &game.inventory;
        for profile in &mut game.profiles {
            for slot in &mut profile.slots {
                let lost = slot.as_ref().is_some_and(|item| {
                    !inventory.iter().any(|owned| owned.item_id == item.item_id)
                });
                if lost {
                    *slot = None;
                    changed = true;
                }
            }
        }
        changed
    }
}

/// What a repository holds at startup.
#[derive(Debug, Default)]
pub struct Loaded {
    pub accounts: Vec<AccountRecord>,
    /// At least past every account id the repository has seen.
    pub next_account_id: u32,
}

/// A session that has not ended, as it is kept: only which account its id
/// maps to, and when it was last seen. No secret: the id is what the
/// client shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StoredSession {
    pub client_revision: u8,
    pub session_id: u32,
    pub account_id: u32,
    /// Its last sighting, Unix seconds (wall clock: it has to mean the same
    /// after a restart).
    pub last_seen_unix: u64,
}

/// Where accounts are kept. Each call stands alone: a save is one
/// transaction, so a crash leaves every account either as it was or as it
/// is.
pub trait AccountRepository: Send {
    /// Every account, by id.
    fn load_all(&mut self) -> Result<Loaded, PersistError>;
    /// Write one account whole, replacing what was kept of it, and keep the
    /// next account id past it.
    fn save_account(&mut self, record: &AccountRecord) -> Result<(), PersistError>;
    /// The id the next new account gets.
    fn next_account_id(&mut self) -> Result<u32, PersistError>;
    /// Every kept session, by id.
    fn load_sessions(&mut self) -> Result<Vec<StoredSession>, PersistError>;
    /// Write one session, replacing what was kept of it. Its account must
    /// have been saved.
    fn save_session(&mut self, session: &StoredSession) -> Result<(), PersistError>;
    /// Forget a session; one not kept is no error.
    fn delete_session(&mut self, session_id: u32) -> Result<(), PersistError>;
    /// Every kept address ban.
    fn load_ip_bans(&mut self) -> Result<Vec<IpBan>, PersistError>;
    /// Keep an address ban, replacing one of the same range.
    fn save_ip_ban(&mut self, ban: &IpBan) -> Result<(), PersistError>;
    /// Forget the ban of a range; none kept is no error.
    fn delete_ip_ban(&mut self, net: &crate::Net) -> Result<(), PersistError>;
    /// Where it keeps them, for the log.
    fn describe(&self) -> String;
}

/// Accounts kept in memory: for tests, and to see what a store saved. Clones
/// share one set of accounts, so a test can keep one and give the store
/// another.
#[derive(Clone, Default)]
pub struct MemoryRepository {
    inner: Arc<Mutex<MemoryInner>>,
}

#[derive(Default)]
struct MemoryInner {
    accounts: HashMap<u32, AccountRecord>,
    next_account_id: u32,
    saves: usize,
    sessions: HashMap<u32, StoredSession>,
    session_saves: usize,
    ip_bans: Vec<IpBan>,
    /// Account writes fail while it is on: a database refusing them.
    failing_account_saves: bool,
}

impl MemoryRepository {
    /// Make every account write fail (with `BadRow`), or work again: a
    /// database that refuses them, for tests.
    pub fn fail_account_saves(&self, failing: bool) {
        self.inner.locked().failing_account_saves = failing;
    }

    /// How many accounts were written so far.
    pub fn saves(&self) -> usize {
        self.inner.locked().saves
    }

    pub fn get(&self, account_id: u32) -> Option<AccountRecord> {
        self.inner.locked().accounts.get(&account_id).cloned()
    }

    /// How many sessions were written so far.
    pub fn session_saves(&self) -> usize {
        self.inner.locked().session_saves
    }

    pub fn session(&self, session_id: u32) -> Option<StoredSession> {
        self.inner.locked().sessions.get(&session_id).copied()
    }

    /// Keep a session as an earlier run would have.
    pub fn put_session(&self, session: StoredSession) {
        self.inner
            .locked()
            .sessions
            .insert(session.session_id, session);
    }
}

impl AccountRepository for MemoryRepository {
    fn load_all(&mut self) -> Result<Loaded, PersistError> {
        let inner = self.inner.locked();
        let mut accounts: Vec<_> = inner.accounts.values().cloned().collect();
        accounts.sort_by_key(|record| record.id);
        Ok(Loaded {
            accounts,
            next_account_id: inner.next_account_id,
        })
    }

    fn save_account(&mut self, record: &AccountRecord) -> Result<(), PersistError> {
        let mut inner = self.inner.locked();
        if inner.failing_account_saves {
            return Err(PersistError::BadRow(record.id, "disk full".into()));
        }
        inner.next_account_id = inner.next_account_id.max(record.id + 1);
        inner.accounts.insert(record.id, record.clone());
        inner.saves += 1;
        Ok(())
    }

    fn next_account_id(&mut self) -> Result<u32, PersistError> {
        Ok(self.inner.locked().next_account_id)
    }

    fn load_sessions(&mut self) -> Result<Vec<StoredSession>, PersistError> {
        let mut sessions: Vec<_> = self.inner.locked().sessions.values().copied().collect();
        sessions.sort_by_key(|session| session.session_id);
        Ok(sessions)
    }

    fn save_session(&mut self, session: &StoredSession) -> Result<(), PersistError> {
        let mut inner = self.inner.locked();
        inner.sessions.insert(session.session_id, *session);
        inner.session_saves += 1;
        Ok(())
    }

    fn delete_session(&mut self, session_id: u32) -> Result<(), PersistError> {
        self.inner.locked().sessions.remove(&session_id);
        Ok(())
    }

    fn load_ip_bans(&mut self) -> Result<Vec<IpBan>, PersistError> {
        Ok(self.inner.locked().ip_bans.clone())
    }

    fn save_ip_ban(&mut self, ban: &IpBan) -> Result<(), PersistError> {
        let mut inner = self.inner.locked();
        inner.ip_bans.retain(|kept| kept.net != ban.net);
        inner.ip_bans.push(ban.clone());
        Ok(())
    }

    fn delete_ip_ban(&mut self, net: &crate::Net) -> Result<(), PersistError> {
        self.inner.locked().ip_bans.retain(|kept| kept.net != *net);
        Ok(())
    }

    fn describe(&self) -> String {
        "memory".to_owned()
    }
}

/// The longest user name the client sends: its `m_account_email` buffer is
/// 100 bytes, zero-terminated (`../srp-021d/docs/notes/login.md`).
pub const MAX_USER_NAME_LEN: usize = 99;

/// What an account is found by at sign-in: the user name the client sent
/// (an email or not; srp doesn't care), trimmed and in lower case, so
/// `Stepan`, `stepan ` and `STEPAN` are one account wherever they sign in
/// from. `None` for an empty or over-long name.
pub fn user_name_key(user_name: &str) -> Option<String> {
    let user_name = user_name.trim();
    if user_name.is_empty() || user_name.len() > MAX_USER_NAME_LEN {
        return None;
    }
    Some(user_name.to_lowercase())
}

/// [`user_name_key`], with an empty name falling back to the address it
/// came from (an account made without signing in, as tests do).
pub fn login_key(ip: std::net::IpAddr, user_name: &str) -> String {
    user_name_key(user_name).unwrap_or_else(|| format!("ip:{ip}"))
}

/// The repository srp's environment asks for: `None` with `SRP_DB=off` (or
/// empty), else `<SRP_DATA_DIR>/srp.db`, backed up first.
pub fn open_from_env() -> Result<Option<SqliteRepository>, PersistError> {
    if let Ok(value) = std::env::var(DB_VAR) {
        let value = value.trim();
        if value.is_empty() || value.eq_ignore_ascii_case("off") {
            return Ok(None);
        }
    }
    let dir = std::env::var_os(DATA_DIR_VAR)
        .filter(|dir| !dir.is_empty())
        .map_or_else(|| PathBuf::from(DEFAULT_DATA_DIR), PathBuf::from);
    open_in(&dir).map(Some)
}

/// `<dir>/srp.db`, the directory made if missing, after a backup of what
/// was there.
pub fn open_in(dir: &Path) -> Result<SqliteRepository, PersistError> {
    std::fs::create_dir_all(dir).map_err(|error| PersistError::Io(dir.to_owned(), error))?;
    let path = dir.join(DB_FILE);
    SqliteRepository::open_with_backup(&path, &dir.join("backups"))
}

/// The newest of `srp-<unix>.db` in `dir` kept, `keep` of them.
fn prune_backups(dir: &Path, keep: usize) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut backups: Vec<(u64, PathBuf)> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            let stamp = name
                .strip_prefix("srp-")?
                .strip_suffix(".db")?
                .parse()
                .ok()?;
            Some((stamp, entry.path()))
        })
        .collect();
    backups.sort_by_key(|backup| std::cmp::Reverse(backup.0));
    for (_, path) in backups.into_iter().skip(keep) {
        if let Err(error) = std::fs::remove_file(&path) {
            log::warn!(
                "Could not remove the old backup {}: {error}",
                path.display()
            );
        }
    }
}

#[cfg(test)]
mod tests;
