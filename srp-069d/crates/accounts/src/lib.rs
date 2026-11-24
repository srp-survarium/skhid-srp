//! accounts (0.69d): SQLite store of accounts (argon2 password hashes), one profile per account,
//! friends and ignore lists. See README.md.
//!
//! ```
//! use accounts::{Store, Currency};
//! let store = Store::open_in_memory().unwrap();
//! # store.set_hash_cost(accounts::HashCost::FAST_FOR_TESTS);
//! let id = store.create_account("scav", "secret").unwrap();
//! assert_eq!(store.verify_login("SCAV", "secret").unwrap(), id);
//! store.update_profile(id, |p| p.spend(Currency::Silver, 500)).unwrap();
//! assert_eq!(store.load_profile(id).unwrap().silver, 1_000_000 - 500);
//! ```

mod bans;
pub mod comments;
pub mod equip;
pub mod clans;
mod friends;
pub mod hero;
mod import;
pub mod league;
mod periodic;
mod presets;
mod profile;
pub mod pve;
pub mod quests;
mod schema;
mod sessions;
mod steam;

use std::path::Path;
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng};
use argon2::{Algorithm, Argon2, Params, Version};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

pub use bans::{Ban, BanKind, MAX_BAN_MINUTES};
pub use clans::{Clan, ClanError, ClanMember, ClanRank};
pub use friends::{FriendEntry, FriendState};
pub use hero::{HeroResult, HeroTitle, SpecificError};
pub use import::ImportReport;
pub use league::{LadderRow, LeagueState, SeasonRecord};
pub use presets::{Preset, PresetSlot, MAX_NAME as MAX_PRESET_NAME, MAX_PRESETS, MAX_PRESETS_PREMIUM};
pub use periodic::{BattleRow, CalendarState, HuntEntry, HuntState};
pub use pve::{pve_run_key, pve_wire_id, ChestRefusal, PveLadderRow, PveRewardRow, PveRun, PveRunPlayer};
pub use quests::{DailyState, QuestSlot};
pub use profile::{AchProgress, BoosterState, Consumed, ContractState, Currency, DecalRow, Insufficient, ItemRow, LeavePenalty, Profile, CLAIM_FIRST_GAME, CLAIM_GAME_WORLD_TUTORIAL, CLAIM_TUTORIAL, DEFAULT_REMOVED, UID_BASE};

/// Account id (the SQLite row id, never reused).
pub type AccountId = i64;

/// Public player id the lobby shows and the chat/friend messages carry (u64): a fixed prefix plus the account id.
pub const PUBLIC_ID_BASE: u64 = 0x5000_0000_0000_0000;

pub fn public_id(id: AccountId) -> u64 {
    PUBLIC_ID_BASE | id as u64
}

/// Reverse of `public_id`; None when the id does not have the prefix.
pub fn account_of_public_id(public: u64) -> Option<AccountId> {
    (public & !0x0FFF_FFFF_FFFF_FFFF == PUBLIC_ID_BASE).then_some((public & 0x0FFF_FFFF_FFFF_FFFF) as AccountId)
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("database: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("account name already taken")]
    NameTaken,
    #[error("invalid account name: {0}")]
    InvalidName(&'static str),
    #[error("invalid password: {0}")]
    InvalidPassword(&'static str),
    #[error("no such account")]
    NoSuchAccount,
    #[error("wrong name or password")]
    BadCredentials,
    #[error("password hashing: {0}")]
    Hash(String),
    #[error("database schema version {0} is newer than this server knows")]
    SchemaTooNew(i64),
    #[error("{0}")]
    Invalid(&'static str),
    #[error("import: {0}")]
    Import(String),
    #[error("you cannot do that to yourself")]
    SelfTarget,
    #[error("already friends")]
    AlreadyFriends,
    #[error("a request is already pending")]
    AlreadyRequested,
    #[error("no pending request")]
    NoRequest,
    #[error("value out of range in the database: {0}")]
    Range(&'static str),
}

/// Failure of `update_profile`: the store failed, or the closure refused the change (nothing is saved then).
#[derive(Debug, thiserror::Error)]
pub enum UpdateError<E> {
    #[error(transparent)]
    Store(#[from] Error),
    #[error("change rejected")]
    Rejected(E),
}

/// Argon2id cost parameters for new password hashes (existing hashes carry their own).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HashCost {
    pub m_kib: u32,
    pub t: u32,
    pub p: u32,
}

impl HashCost {
    /// argon2 crate default (19 MiB, 2 passes, 1 lane): the production value.
    pub const DEFAULT: HashCost = HashCost { m_kib: 19 * 1024, t: 2, p: 1 };
    /// Cheap settings for unit tests only.
    pub const FAST_FOR_TESTS: HashCost = HashCost { m_kib: 8, t: 1, p: 1 };
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    pub id: AccountId,
    pub name: String,
    pub created_at: i64,
}

/// Experience and reputation of an account at one moment: what a progress hook compares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProgressMark {
    pub xp: u32,
    /// Reputation points of the factions 1..=4 (index = faction - 1).
    pub points: [u32; 4],
}

impl ProgressMark {
    pub fn of(p: &Profile) -> ProgressMark {
        ProgressMark { xp: p.xp, points: [p.points(1), p.points(2), p.points(3), p.points(4)] }
    }
}

/// Called after a committed profile update that changed the experience or a faction's reputation: (account, before,
/// after). It runs outside the store lock but on the writer's thread: it must be quick and must not wait on the store.
pub type ProgressHook = Box<dyn Fn(AccountId, ProgressMark, ProgressMark) + Send + Sync>;

pub struct Store {
    conn: Mutex<Connection>,
    cost: Mutex<HashCost>,
    dummy: OnceLock<String>,
    progress_hook: Mutex<Option<ProgressHook>>,
}

pub(crate) fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

fn check_name(name: &str) -> Result<(), Error> {
    if name.is_empty() {
        return Err(Error::InvalidName("empty"));
    }
    if name.chars().count() > 32 {
        return Err(Error::InvalidName("longer than 32 characters"));
    }
    if name != name.trim() {
        return Err(Error::InvalidName("leading or trailing space"));
    }
    if name.chars().any(|c| c.is_control()) {
        return Err(Error::InvalidName("control character"));
    }
    Ok(())
}

/// Longest nickname in characters. PROVEN: the client's nickname box is `TextFields.NICKNAME_MAX_CHARS` = 30 (lobby
/// movie, `MessageEnterName.configUI`). Login names keep the 32 of `check_name`.
pub const MAX_NICKNAME_CHARS: usize = 30;

/// The nickname rules: `check_name` plus the client's 30-character box.
fn check_nick(name: &str) -> Result<(), Error> {
    check_name(name)?;
    if name.chars().count() > MAX_NICKNAME_CHARS {
        return Err(Error::InvalidName("longer than 30 characters"));
    }
    Ok(())
}

/// Another account already uses `name` as its nickname (case-insensitive).
fn nick_taken(conn: &Connection, id: AccountId, name: &str) -> Result<bool, Error> {
    Ok(conn.query_row("SELECT EXISTS(SELECT 1 FROM accounts WHERE nick = ?1 COLLATE NOCASE AND id != ?2)", params![name, id], |r| r.get(0))?)
}

fn check_password(pw: &str) -> Result<(), Error> {
    if pw.is_empty() {
        return Err(Error::InvalidPassword("empty"));
    }
    if pw.len() > 256 {
        return Err(Error::InvalidPassword("longer than 256 bytes"));
    }
    Ok(())
}

fn u32_of(v: i64, what: &'static str) -> Result<u32, Error> {
    u32::try_from(v).map_err(|_| Error::Range(what))
}

impl Store {
    /// Open (and create or migrate) the database file.
    pub fn open(path: &Path) -> Result<Store, Error> {
        let conn = Connection::open(path)?;
        let _: String = conn.query_row("PRAGMA journal_mode=WAL", [], |r| r.get(0))?;
        Store::init(conn)
    }

    pub fn open_in_memory() -> Result<Store, Error> {
        Store::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Store, Error> {
        conn.execute_batch("PRAGMA foreign_keys=ON; PRAGMA synchronous=NORMAL; PRAGMA busy_timeout=5000;")?;
        let v: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if v > schema::VERSION {
            return Err(Error::SchemaTooNew(v));
        }
        if v == 0 {
            conn.execute_batch(&format!(
                "BEGIN; {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} PRAGMA user_version = {}; COMMIT;",
                schema::SCHEMA_V1,
                schema::SCHEMA_V2_UP,
                schema::SCHEMA_V3_UP,
                schema::SCHEMA_V4_UP,
                schema::SCHEMA_V5_UP,
                schema::SCHEMA_V6_UP,
                schema::SCHEMA_V7_UP,
                schema::SCHEMA_V8_UP,
                schema::SCHEMA_V9_UP,
                schema::SCHEMA_V10_UP,
                schema::SCHEMA_V11_UP,
                schema::SCHEMA_V12_UP,
                schema::SCHEMA_V13_UP,
                schema::SCHEMA_V14_UP,
                schema::SCHEMA_V15_UP,
                schema::SCHEMA_V16_UP,
                schema::SCHEMA_V17_UP,
                schema::SCHEMA_V18_UP,
                schema::SCHEMA_V19_UP,
                schema::SCHEMA_V20_UP,
                schema::SCHEMA_V21_UP,
                schema::SCHEMA_V22_UP,
                schema::SCHEMA_V23_UP,
                schema::VERSION
            ))?;
        } else {
            if v == 1 {
                // migration: nothing of v1 changes, only the skills table is new
                conn.execute_batch(&format!("BEGIN; {} PRAGMA user_version = 2; COMMIT;", schema::SCHEMA_V2_UP))?;
            }
            if v <= 2 {
                // migration (progression): achievement progress table
                conn.execute_batch(&format!("BEGIN; {} PRAGMA user_version = 3; COMMIT;", schema::SCHEMA_V3_UP))?;
            }
            if v <= 3 {
                // migration (social): ban columns and clan tables are new
                conn.execute_batch(&format!("BEGIN; {} PRAGMA user_version = 4; COMMIT;", schema::SCHEMA_V4_UP))?;
            }
            if v <= 4 {
                // migration (items): item uids start at 100001, presets tables
                conn.execute_batch(&format!("BEGIN; {} PRAGMA user_version = 5; COMMIT;", schema::SCHEMA_V5_UP))?;
            }
            if v <= 5 {
                // migration (store): premium account expiry table
                conn.execute_batch(&format!("BEGIN; {} PRAGMA user_version = 6; COMMIT;", schema::SCHEMA_V6_UP))?;
            }
            if v <= 6 {
                // migration (quests): daily quest tables
                conn.execute_batch(&format!("BEGIN; {} PRAGMA user_version = 7; COMMIT;", schema::SCHEMA_V7_UP))?;
            }
            if v <= 7 {
                // migration (periodic features): treasure hunt, faction battle rewards, login calendar
                conn.execute_batch(&format!("BEGIN; {} PRAGMA user_version = 8; COMMIT;", schema::SCHEMA_V8_UP))?;
            }
            if v <= 8 {
                // migration (store): boosters table, nickname column
                conn.execute_batch(&format!("BEGIN; {} PRAGMA user_version = 9; COMMIT;", schema::SCHEMA_V9_UP))?;
            }
            if v <= 9 {
                // migration (ranked leagues): league state and season history tables
                conn.execute_batch(&format!("BEGIN; {} PRAGMA user_version = 10; COMMIT;", schema::SCHEMA_V10_UP))?;
            }
            if v <= 10 {
                // migration (onboarding): tutorial-gate flags and face per profile; existing accounts are past onboarding
                conn.execute_batch(&format!("BEGIN; {} PRAGMA user_version = 11; COMMIT;", schema::SCHEMA_V11_UP))?;
            }
            if v <= 11 {
                // migration (decals, one-time claims); written idempotent (IF NOT EXISTS)
                conn.execute_batch(&format!("BEGIN; {} PRAGMA user_version = 12; COMMIT;", schema::SCHEMA_V12_UP))?;
            }
            if v <= 12 {
                // migration (hero of the day, bought specifics); written idempotent (IF NOT EXISTS)
                conn.execute_batch(&format!("BEGIN; {} PRAGMA user_version = 13; COMMIT;", schema::SCHEMA_V13_UP))?;
            }
            if v <= 13 {
                // migration (PvE missions): results, rewards, daily wins; written idempotent (IF NOT EXISTS)
                conn.execute_batch(&format!("BEGIN; {} PRAGMA user_version = 14; COMMIT;", schema::SCHEMA_V14_UP))?;
            }
            if v <= 14 {
                // migration (equipment profiles, weapon skins); written idempotent (IF NOT EXISTS)
                conn.execute_batch(&format!("BEGIN; {} PRAGMA user_version = 15; COMMIT;", schema::SCHEMA_V15_UP))?;
            }
            if v <= 15 {
                // migration (complaints / compliments); written idempotent (IF NOT EXISTS)
                conn.execute_batch(&format!("BEGIN; {} PRAGMA user_version = 16; COMMIT;", schema::SCHEMA_V16_UP))?;
            }
            if v <= 16 {
                // migration (event currencies); written idempotent (IF NOT EXISTS)
                conn.execute_batch(&format!("BEGIN; {} PRAGMA user_version = 17; COMMIT;", schema::SCHEMA_V17_UP))?;
            }
            if v <= 17 {
                // migration (weapons wearing a skin keep their real item); written idempotent (IF NOT EXISTS)
                conn.execute_batch(&format!("BEGIN; {} PRAGMA user_version = 18; COMMIT;", schema::SCHEMA_V18_UP))?;
            }
            if v <= 18 {
                // migration (leaver penalty); written idempotent (IF NOT EXISTS)
                conn.execute_batch(&format!("BEGIN; {} PRAGMA user_version = 19; COMMIT;", schema::SCHEMA_V19_UP))?;
            }
            if v <= 19 {
                // migration (the Winner's Contract); written idempotent (IF NOT EXISTS)
                conn.execute_batch(&format!("BEGIN; {} PRAGMA user_version = 20; COMMIT;", schema::SCHEMA_V20_UP))?;
            }
            if v <= 20 {
                // migration (clan rank names: the client's standard ranks); matches only the old default names
                conn.execute_batch(&format!("BEGIN; {} PRAGMA user_version = 21; COMMIT;", schema::SCHEMA_V21_UP))?;
            }
            if v <= 21 {
                // migration (clan registration deposits); idempotent: the column is added only when it is missing
                let has: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM pragma_table_info('clan_reservations') WHERE name = 'collected')", [], |r| r.get(0))?;
                let add = if has { "" } else { "ALTER TABLE clan_reservations ADD COLUMN collected INTEGER NOT NULL DEFAULT 0;" };
                conn.execute_batch(&format!("BEGIN; {add} {} PRAGMA user_version = 22; COMMIT;", schema::SCHEMA_V22_UP_DATA))?;
            }
            if v <= 22 {
                // finished-match count (no data fix-ups: 0.69d databases are built fresh)
                let has: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM pragma_table_info('profiles') WHERE name = 'matches')", [], |r| r.get(0))?;
                let add = if has { "" } else { schema::SCHEMA_V23_UP };
                conn.execute_batch(&format!("BEGIN; {add} PRAGMA user_version = 23; COMMIT;"))?;
            }
        }
        // server runtime state outside the versioned schema: made on every open, any version (sessions.rs)
        conn.execute_batch(schema::LOGIN_SESSIONS)?;
        conn.execute_batch(schema::STEAM_IDS)?;
        Ok(Store { conn: Mutex::new(conn), cost: Mutex::new(HashCost::DEFAULT), dummy: OnceLock::new(), progress_hook: Mutex::new(None) })
    }

    /// Set the in-game NICKNAME of an account (the first choice of the onboarding screen, or the rename voucher); the
    /// login name stays. Same rules as account creation; the nickname must differ from every other account's
    /// NICKNAME (case-insensitive; the account's own nickname in another case is allowed). Login names are private
    /// (a separate name space) and are not compared, so a nickname never reveals that a login name exists.
    pub fn rename_account(&self, id: AccountId, new_name: &str) -> Result<(), Error> {
        check_nick(new_name)?;
        let mut conn = self.conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if nick_taken(&tx, id, new_name)? {
            return Err(Error::NameTaken);
        }
        if tx.execute("UPDATE accounts SET nick = ?1 WHERE id = ?2", params![new_name, id])? == 0 {
            return Err(Error::NoSuchAccount);
        }
        tx.commit()?;
        Ok(())
    }

    /// Would `name` be accepted as the nickname of account `id` now (48 04 validation)? `Ok` = free and well-formed;
    /// `Err(InvalidName)` / `Err(NameTaken)` otherwise.
    pub fn check_nickname(&self, id: AccountId, name: &str) -> Result<(), Error> {
        check_nick(name)?;
        let conn = self.conn();
        if nick_taken(&conn, id, name)? {
            return Err(Error::NameTaken);
        }
        Ok(())
    }

    /// Has the account chosen a nickname (onboarding screen or earlier)? NULL nick = not yet.
    pub fn has_nickname(&self, id: AccountId) -> Result<bool, Error> {
        let n: Option<Option<String>> = self.conn().query_row("SELECT nick FROM accounts WHERE id = ?1", [id], |r| r.get(0)).optional()?;
        match n {
            Some(n) => Ok(n.is_some()),
            None => Err(Error::NoSuchAccount),
        }
    }

    /// Cost of new password hashes (tests lower it; the default is the argon2 crate's).
    pub fn set_hash_cost(&self, cost: HashCost) {
        *self.cost.lock().unwrap() = cost;
    }

    pub(crate) fn conn(&self) -> MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn hasher(&self) -> Result<Argon2<'static>, Error> {
        let c = *self.cost.lock().unwrap();
        let params = Params::new(c.m_kib, c.t, c.p, None).map_err(|e| Error::Hash(e.to_string()))?;
        Ok(Argon2::new(Algorithm::Argon2id, Version::V0x13, params))
    }

    fn hash_password(&self, pw: &str) -> Result<String, Error> {
        let salt = SaltString::generate(&mut OsRng);
        self.hasher()?.hash_password(pw.as_bytes(), &salt).map(|h| h.to_string()).map_err(|e| Error::Hash(e.to_string()))
    }

    // ---- accounts

    /// Create an account with the starter profile ([`Profile::starter`]).
    pub fn create_account(&self, name: &str, password: &str) -> Result<AccountId, Error> {
        self.create_account_with_profile(name, password, &Profile::starter())
    }

    /// Create an account with the given profile. A profile still in onboarding (`profile.nickname_chosen()` false: a
    /// new player) gets no nickname yet (NULL: chosen on the client's nickname screen); a profile past it (the test
    /// starter) keeps the login name as its nickname.
    pub fn create_account_with_profile(&self, name: &str, password: &str, profile: &Profile) -> Result<AccountId, Error> {
        check_name(name)?;
        check_password(password)?;
        let hash = self.hash_password(password)?; // before the lock: argon2 is slow
        let mut conn = self.conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let taken: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM accounts WHERE name = ?1 OR nick = ?1)", [name], |r| r.get(0))?;
        if taken {
            return Err(Error::NameTaken);
        }
        let nick = if profile.nickname_chosen() {
            if nick_taken(&tx, -1, name)? {
                return Err(Error::NameTaken);
            }
            Some(name)
        } else {
            None
        };
        tx.execute("INSERT INTO accounts (name, pass_hash, created_at, nick) VALUES (?1, ?2, ?3, ?4)", params![name, hash, now(), nick])?;
        let id = tx.last_insert_rowid();
        save_profile_tx(&tx, id, profile)?;
        tx.commit()?;
        Ok(id)
    }

    /// Check a name and password; the same error for an unknown name and a wrong password.
    pub fn verify_login(&self, name: &str, password: &str) -> Result<AccountId, Error> {
        let row: Option<(AccountId, String)> = self
            .conn()
            .query_row("SELECT id, pass_hash FROM accounts WHERE name = ?1", [name], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()?;
        match row {
            Some((id, hash)) => {
                let ok = PasswordHash::new(&hash)
                    .map(|h| Argon2::default().verify_password(password.as_bytes(), &h).is_ok())
                    .unwrap_or(false);
                if ok { Ok(id) } else { Err(Error::BadCredentials) }
            }
            None => {
                // spend the same time as a real check
                let dummy = self.dummy.get_or_init(|| self.hash_password("dummy password").unwrap_or_default());
                if let Ok(h) = PasswordHash::new(dummy) {
                    let _ = Argon2::default().verify_password(password.as_bytes(), &h);
                }
                Err(Error::BadCredentials)
            }
        }
    }

    pub fn change_password(&self, id: AccountId, new_password: &str) -> Result<(), Error> {
        check_password(new_password)?;
        let hash = self.hash_password(new_password)?;
        let n = self.conn().execute("UPDATE accounts SET pass_hash = ?1 WHERE id = ?2", params![hash, id])?;
        if n == 0 { Err(Error::NoSuchAccount) } else { Ok(()) }
    }

    pub fn account(&self, id: AccountId) -> Result<Option<Account>, Error> {
        Ok(self
            .conn()
            .query_row("SELECT id, COALESCE(nick, 'Player' || id), created_at FROM accounts WHERE id = ?1", [id], |r| {
                Ok(Account { id: r.get(0)?, name: r.get(1)?, created_at: r.get(2)? })
            })
            .optional()?)
    }

    /// Case-insensitive lookup by LOGIN name (the returned `name` is the in-game nickname).
    pub fn find_account(&self, name: &str) -> Result<Option<Account>, Error> {
        Ok(self
            .conn()
            .query_row("SELECT id, COALESCE(nick, 'Player' || id), created_at FROM accounts WHERE name = ?1", [name], |r| {
                Ok(Account { id: r.get(0)?, name: r.get(1)?, created_at: r.get(2)? })
            })
            .optional()?)
    }

    pub fn account_count(&self) -> Result<u64, Error> {
        Ok(self.conn().query_row("SELECT COUNT(*) FROM accounts", [], |r| r.get::<_, i64>(0))? as u64)
    }

    /// Accounts whose name contains `query` (case-insensitive), alphabetical, at most `limit`, never `exclude`
    /// (4E 04 search). `%`, `_` and `\` in the query are literal.
    pub fn search_accounts(&self, query: &str, limit: usize, exclude: Option<AccountId>) -> Result<Vec<Account>, Error> {
        if query.is_empty() {
            return Ok(Vec::new());
        }
        let pat = format!("%{}%", query.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_"));
        let conn = self.conn();
        let mut st = conn.prepare(
            // nicknames only: an account without a chosen nickname (still on the onboarding screen) is not findable,
            // and login names are never searched (private)
            "SELECT id, nick, created_at FROM accounts WHERE nick LIKE ?1 ESCAPE '\\' AND id <> ?2 ORDER BY nick LIMIT ?3",
        )?;
        let rows = st.query_map(params![pat, exclude.unwrap_or(-1), limit as i64], |r| {
            Ok(Account { id: r.get(0)?, name: r.get(1)?, created_at: r.get(2)? })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    // ---- profiles

    pub fn load_profile(&self, id: AccountId) -> Result<Profile, Error> {
        let conn = self.conn();
        load_profile_conn(&conn, id)
    }

    /// Replace the stored profile.
    pub fn save_profile(&self, id: AccountId, profile: &Profile) -> Result<(), Error> {
        let mut conn = self.conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        save_profile_tx(&tx, id, profile)?;
        tx.commit()?;
        Ok(())
    }

    /// Atomic read-modify-write: loads the profile, runs `f`, saves when `f` returns Ok. When `f` returns
    /// Err nothing is written. Other writers wait (the transaction is IMMEDIATE and the store is locked).
    pub fn update_profile<T, E>(&self, id: AccountId, f: impl FnOnce(&mut Profile) -> Result<T, E>) -> Result<T, UpdateError<E>> {
        self.update_profile_in(id, |_, p| f(p).map_err(UpdateError::Rejected))
    }

    /// [`Store::update_profile`] for changes that also write other tables: `f` gets the open transaction, so its own
    /// statements commit together with the profile, or not at all when it returns Err.
    pub(crate) fn update_profile_in<T, E>(
        &self,
        id: AccountId,
        f: impl FnOnce(&Connection, &mut Profile) -> Result<T, UpdateError<E>>,
    ) -> Result<T, UpdateError<E>> {
        let mut conn = self.conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate).map_err(Error::from)?;
        let mut p = load_profile_conn(&tx, id)?;
        let before = ProgressMark::of(&p);
        let out = f(&tx, &mut p)?;
        save_profile_tx(&tx, id, &p)?;
        tx.commit().map_err(Error::from)?;
        let after = ProgressMark::of(&p);
        drop(conn);
        if before != after {
            if let Some(hook) = self.progress_hook.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
                hook(id, before, after);
            }
        }
        Ok(out)
    }

    /// Install the progress hook (replaces an earlier one). The lobby uses it to send the level-up and rank-up
    /// notices whichever code path gave the experience or reputation.
    pub fn set_progress_hook(&self, hook: ProgressHook) {
        *self.progress_hook.lock().unwrap_or_else(|e| e.into_inner()) = Some(hook);
    }
}

fn load_profile_conn(conn: &Connection, id: AccountId) -> Result<Profile, Error> {
    #[allow(clippy::type_complexity)]
    let head: Option<(i64, i64, i64, i64, i64, i64, i64, i64, i64, i64)> = conn
        .query_row(
            "SELECT silver, gold, parts, xp, faction, version, next_uid, onboarding, face, matches FROM profiles WHERE account_id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?, r.get(8)?, r.get(9)?)),
        )
        .optional()?;
    let Some((silver, gold, parts, xp, faction, version, next_uid, onboarding, face, matches)) = head else {
        return Err(Error::NoSuchAccount);
    };
    let mut p = Profile {
        items: Vec::new(),
        slots: Default::default(),
        slot_rounds: Default::default(),
        silver: u32_of(silver, "silver")?,
        gold: u32_of(gold, "gold")?,
        parts: u32_of(parts, "parts")?,
        xp: u32_of(xp, "xp")?,
        mods: Default::default(),
        attach: Default::default(),
        removed_defaults: Default::default(),
        bound: Default::default(),
        faction: u8::try_from(faction).map_err(|_| Error::Range("faction"))?,
        fac_points: Default::default(),
        skills: Default::default(),
        ach: Default::default(),
        daily: Default::default(),
        hunt: Default::default(),
        battle: Default::default(),
        calendar: Default::default(),
        premium_until: 0,
        boosters: Default::default(),
        league: Default::default(),
        event: [0; 3],
        hero: Vec::new(),
        equip: Default::default(),
        skins: Default::default(),
        skin_base: Default::default(),
        decals: Default::default(),
        claims: Default::default(),
        penalty: Default::default(),
        contract: Default::default(),
        version: u32_of(version, "version")?,
        next_uid: u32_of(next_uid, "next_uid")?,
        onboarding: u32_of(onboarding, "onboarding")?,
        face: u8::try_from(face).map_err(|_| Error::Range("face"))?,
        matches: u32_of(matches, "matches")?,
    };
    let mut st = conn.prepare("SELECT uid, dict_id, count FROM items WHERE account_id = ?1 ORDER BY pos")?;
    for r in st.query_map([id], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?)))? {
        let (uid, d, c) = r?;
        p.items.push(ItemRow {
            uid: u32_of(uid, "uid")?,
            dict_id: u16::try_from(d).map_err(|_| Error::Range("dict_id"))?,
            count: u32_of(c, "count")?,
        });
    }
    let mut st = conn.prepare("SELECT slot, uid, rounds FROM slots WHERE account_id = ?1")?;
    for r in st.query_map([id], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Option<i64>>(1)?, r.get::<_, Option<i64>>(2)?)))? {
        let (slot, uid, rounds) = r?;
        let slot = u8::try_from(slot).map_err(|_| Error::Range("slot"))?;
        if let Some(u) = uid {
            p.slots.insert(slot, u32_of(u, "slot uid")?);
        }
        if let Some(n) = rounds {
            p.slot_rounds.insert(slot, u32_of(n, "rounds")?);
        }
    }
    let mut st = conn.prepare("SELECT uid, grp, grade FROM item_mods WHERE account_id = ?1 ORDER BY uid, pos")?;
    for r in st.query_map([id], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?)))? {
        let (uid, g, k) = r?;
        p.mods.entry(u32_of(uid, "uid")?).or_default().push((
            u16::try_from(g).map_err(|_| Error::Range("group"))?,
            u16::try_from(k).map_err(|_| Error::Range("grade"))?,
        ));
    }
    let mut st = conn.prepare("SELECT weapon_uid, slot_id, module_uid FROM attachments WHERE account_id = ?1")?;
    for r in st.query_map([id], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?)))? {
        let (w, s, m) = r?;
        let (w, m) = (u32_of(w, "weapon uid")?, u32_of(m, "module uid")?);
        // module uid 0 = the player removed the weapon's default module from this slot (no schema change)
        if m == DEFAULT_REMOVED {
            p.removed_defaults.entry(w).or_default().insert(s);
        } else {
            p.attach.entry(w).or_default().insert(s, m);
        }
    }
    let mut st = conn.prepare("SELECT module_uid, dict_id FROM bound WHERE account_id = ?1")?;
    for r in st.query_map([id], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))? {
        let (m, d) = r?;
        p.bound.insert(u32_of(m, "module uid")?, u16::try_from(d).map_err(|_| Error::Range("dict_id"))?);
    }
    let mut st = conn.prepare("SELECT faction, points FROM faction_points WHERE account_id = ?1")?;
    for r in st.query_map([id], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))? {
        let (f, pts) = r?;
        p.fac_points.insert(u8::try_from(f).map_err(|_| Error::Range("faction"))?, u32_of(pts, "points")?);
    }
    let mut st = conn.prepare("SELECT skill_id, points FROM skills WHERE account_id = ?1")?;
    for r in st.query_map([id], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))? {
        let (s, pts) = r?;
        p.skills.insert(
            u16::try_from(s).map_err(|_| Error::Range("skill id"))?,
            u8::try_from(pts).map_err(|_| Error::Range("skill points"))?,
        );
    }
    let mut st = conn.prepare("SELECT ach_id, value, run FROM achievements WHERE account_id = ?1")?;
    for r in st.query_map([id], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?)))? {
        let (a, v, run) = r?;
        p.ach.insert(
            u16::try_from(a).map_err(|_| Error::Range("achievement id"))?,
            AchProgress { value: u32_of(v, "achievement value")?, run: u32_of(run, "achievement run")? },
        );
    }
    let mut st = conn.prepare("SELECT item_uid, slot, decal_uid, dict_id, x_bits FROM item_decals WHERE account_id = ?1 ORDER BY item_uid, slot")?;
    for r in st.query_map([id], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?, r.get::<_, i64>(3)?, r.get::<_, i64>(4)?)))? {
        let (item, slot, decal, dict, bits) = r?;
        p.decals.entry(u32_of(item, "decal owner uid")?).or_default().push(DecalRow {
            slot: u8::try_from(slot).map_err(|_| Error::Range("decal slot"))?,
            decal_uid: u32_of(decal, "decal uid")?,
            dict: u16::try_from(dict).map_err(|_| Error::Range("decal dict id"))?,
            x_bits: bits as u64,
        });
    }
    let mut st = conn.prepare("SELECT name FROM claims WHERE account_id = ?1")?;
    for r in st.query_map([id], |r| r.get::<_, String>(0))? {
        p.claims.insert(r?);
    }
    equip::load(conn, id, &mut p)?;
    p.daily = load_daily(conn, id)?;
    load_periodic(conn, id, &mut p)?;
    p.league = league::load_league(conn, id)?;
    let mut st = conn.prepare("SELECT slot, amount FROM event_wallet WHERE account_id = ?1")?;
    for r in st.query_map([id], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))? {
        let (slot, amount) = r?;
        if let Some(w) = usize::try_from(slot).ok().and_then(|s| p.event.get_mut(s)) {
            *w = u32_of(amount, "event currency")?;
        }
    }
    p.hero = hero::titles_of(conn, id, 0)?;
    if let Some((day, count, until)) = conn
        .query_row("SELECT day, count, blocked_until FROM leave_penalty WHERE account_id = ?1", [id], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?))
        })
        .optional()?
    {
        p.penalty = LeavePenalty { day, count: u32_of(count, "leave count")?, blocked_until: until };
    }
    if let Some((started, mask)) = conn
        .query_row("SELECT started, won_mask FROM contracts WHERE account_id = ?1", [id], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))
        .optional()?
    {
        p.contract = ContractState { started, won_mask: u8::try_from(mask).map_err(|_| Error::Range("contract won mask"))? };
    }
    p.premium_until = conn
        .query_row("SELECT until FROM premium WHERE account_id = ?1", [id], |r| r.get::<_, i64>(0))
        .optional()?
        .unwrap_or(0);
    let mut st = conn.prepare("SELECT dict_id, enabled, until, autobuy FROM boosters WHERE account_id = ?1")?;
    for r in st.query_map([id], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?, r.get::<_, i64>(3)?)))? {
        let (d, e, u, a) = r?;
        p.boosters.insert(u16::try_from(d).map_err(|_| Error::Range("booster dict id"))?, BoosterState { enabled: e != 0, until: u, autobuy: a != 0 });
    }
    Ok(p)
}

fn save_profile_tx(tx: &rusqlite::Transaction<'_>, id: AccountId, p: &Profile) -> Result<(), Error> {
    let exists: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM accounts WHERE id = ?1)", [id], |r| r.get(0))?;
    if !exists {
        return Err(Error::NoSuchAccount);
    }
    tx.execute(
        "INSERT INTO profiles (account_id, silver, gold, parts, xp, faction, version, next_uid, onboarding, face, matches)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
         ON CONFLICT(account_id) DO UPDATE SET silver = ?2, gold = ?3, parts = ?4, xp = ?5, faction = ?6,
                                               version = ?7, next_uid = ?8, onboarding = ?9, face = ?10, matches = ?11",
        params![id, p.silver, p.gold, p.parts, p.xp, p.faction, p.version, p.next_uid, p.onboarding, p.face, p.matches],
    )?;
    // the tables are rewritten below: keep the time each claim was first made
    let claimed_at: std::collections::BTreeMap<String, i64> = {
        let mut st = tx.prepare("SELECT name, claimed_at FROM claims WHERE account_id = ?1")?;
        let rows = st.query_map([id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
        rows.collect::<Result<_, _>>()?
    };
    for table in ["items", "slots", "item_mods", "attachments", "bound", "faction_points", "skills", "achievements", "premium", "boosters", "daily_quests", "daily_quest_slots", "hunt", "hunt_entries", "battle_rows", "login_calendar", "league", "league_history", "item_decals", "claims", "equip_state", "equip_profiles", "equip_profile_slots", "skin_unlocks", "event_wallet", "item_skin_base", "leave_penalty", "contracts"] {
        tx.execute(&format!("DELETE FROM {table} WHERE account_id = ?1"), [id])?;
    }
    let mut st = tx.prepare("INSERT INTO items (account_id, uid, pos, dict_id, count) VALUES (?1, ?2, ?3, ?4, ?5)")?;
    for (pos, it) in p.items.iter().enumerate() {
        st.execute(params![id, it.uid, pos as i64, it.dict_id, it.count])?;
    }
    let mut st = tx.prepare("INSERT INTO slots (account_id, slot, uid, rounds) VALUES (?1, ?2, ?3, ?4)")?;
    let all: std::collections::BTreeSet<u8> = p.slots.keys().chain(p.slot_rounds.keys()).copied().collect();
    for s in all {
        st.execute(params![id, s, p.slots.get(&s), p.slot_rounds.get(&s)])?;
    }
    let mut st = tx.prepare("INSERT INTO item_mods (account_id, uid, pos, grp, grade) VALUES (?1, ?2, ?3, ?4, ?5)")?;
    for (uid, mods) in &p.mods {
        for (pos, (g, k)) in mods.iter().enumerate() {
            st.execute(params![id, uid, pos as i64, g, k])?;
        }
    }
    let mut st = tx.prepare("INSERT INTO attachments (account_id, weapon_uid, slot_id, module_uid) VALUES (?1, ?2, ?3, ?4)")?;
    for (w, slots) in &p.attach {
        for (slot, m) in slots {
            st.execute(params![id, w, slot, m])?;
        }
    }
    // "default removed" marks: module uid 0; a slot that holds a module keeps the module (primary key weapon + slot)
    for (w, slots) in &p.removed_defaults {
        for slot in slots.iter().filter(|s| !p.attach.get(w).is_some_and(|a| a.contains_key(*s))) {
            st.execute(params![id, w, slot, DEFAULT_REMOVED])?;
        }
    }
    let mut st = tx.prepare("INSERT INTO bound (account_id, module_uid, dict_id) VALUES (?1, ?2, ?3)")?;
    for (m, d) in &p.bound {
        st.execute(params![id, m, d])?;
    }
    let mut st = tx.prepare("INSERT INTO faction_points (account_id, faction, points) VALUES (?1, ?2, ?3)")?;
    for (f, pts) in &p.fac_points {
        st.execute(params![id, f, pts])?;
    }
    let mut st = tx.prepare("INSERT INTO skills (account_id, skill_id, points) VALUES (?1, ?2, ?3)")?;
    for (s, pts) in p.skills.iter().filter(|(_, p)| **p > 0) {
        st.execute(params![id, s, pts])?;
    }
    let mut st = tx.prepare("INSERT INTO achievements (account_id, ach_id, value, run) VALUES (?1, ?2, ?3, ?4)")?;
    for (a, pr) in &p.ach {
        st.execute(params![id, a, pr.value, pr.run])?;
    }
    let mut st = tx.prepare("INSERT INTO item_decals (account_id, item_uid, slot, decal_uid, dict_id, x_bits) VALUES (?1, ?2, ?3, ?4, ?5, ?6)")?;
    for (item, list) in &p.decals {
        for d in list {
            st.execute(params![id, item, d.slot, d.decal_uid, d.dict, d.x_bits as i64])?;
        }
    }
    let mut st = tx.prepare("INSERT INTO claims (account_id, name, claimed_at) VALUES (?1, ?2, ?3)")?;
    for name in &p.claims {
        st.execute(params![id, name, claimed_at.get(name).copied().unwrap_or_else(now)])?;
    }
    equip::save(tx, id, p)?;
    save_daily(tx, id, &p.daily)?;
    save_periodic(tx, id, p)?;
    league::save_league(tx, id, &p.league)?;
    let mut st = tx.prepare("INSERT INTO event_wallet (account_id, slot, amount) VALUES (?1, ?2, ?3)")?;
    for (slot, amount) in p.event.iter().enumerate().filter(|(_, a)| **a > 0) {
        st.execute(params![id, slot as i64, amount])?;
    }
    if p.premium_until > 0 {
        tx.execute("INSERT INTO premium (account_id, until) VALUES (?1, ?2)", params![id, p.premium_until])?;
    }
    if p.penalty != LeavePenalty::default() {
        tx.execute(
            "INSERT INTO leave_penalty (account_id, day, count, blocked_until) VALUES (?1, ?2, ?3, ?4)",
            params![id, p.penalty.day, p.penalty.count, p.penalty.blocked_until],
        )?;
    }
    if p.contract != ContractState::default() {
        tx.execute("INSERT INTO contracts (account_id, started, won_mask) VALUES (?1, ?2, ?3)", params![id, p.contract.started, p.contract.won_mask])?;
    }
    let mut st = tx.prepare("INSERT INTO boosters (account_id, dict_id, enabled, until, autobuy) VALUES (?1, ?2, ?3, ?4, ?5)")?;
    for (d, b) in &p.boosters {
        st.execute(params![id, d, b.enabled, b.until, b.autobuy])?;
    }
    Ok(())
}

fn load_daily(conn: &Connection, id: AccountId) -> Result<DailyState, Error> {
    let head = conn
        .query_row(
            "SELECT issued, expires, free_used, chest_dict, chest_amount, next_uid, next_msg FROM daily_quests WHERE account_id = ?1",
            [id],
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?, r.get::<_, i64>(3)?, r.get::<_, i64>(4)?, r.get::<_, i64>(5)?, r.get::<_, i64>(6)?)),
        )
        .optional()?;
    let Some((issued, expires, free_used, chest_dict, chest_amount, next_uid, next_msg)) = head else {
        return Ok(DailyState::default());
    };
    let mut d = DailyState {
        issued,
        expires,
        free_used: u8::try_from(free_used).map_err(|_| Error::Range("free rerolls"))?,
        chest_dict: u16::try_from(chest_dict).map_err(|_| Error::Range("chest dict"))?,
        chest_amount: u32_of(chest_amount, "chest amount")?,
        next_uid: u32_of(next_uid, "quest uid")?,
        next_msg: u32_of(next_msg, "message id")?,
        slots: Vec::new(),
    };
    let mut st = conn.prepare("SELECT uid, dict_id, progress, done FROM daily_quest_slots WHERE account_id = ?1 ORDER BY slot")?;
    for r in st.query_map([id], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?, r.get::<_, i64>(3)?)))? {
        let (uid, dict, progress, done) = r?;
        d.slots.push(QuestSlot {
            uid: u32_of(uid, "quest uid")?,
            dict: u16::try_from(dict).map_err(|_| Error::Range("quest dict"))?,
            progress: u32_of(progress, "quest progress")?,
            done: done != 0,
        });
    }
    Ok(d)
}

fn save_daily(tx: &rusqlite::Transaction<'_>, id: AccountId, d: &DailyState) -> Result<(), Error> {
    if d.issued == 0 && d.slots.is_empty() {
        return Ok(());
    }
    tx.execute(
        "INSERT INTO daily_quests (account_id, issued, expires, free_used, chest_dict, chest_amount, next_uid, next_msg)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![id, d.issued, d.expires, d.free_used, d.chest_dict, d.chest_amount, d.next_uid, d.next_msg],
    )?;
    let mut st = tx.prepare("INSERT INTO daily_quest_slots (account_id, slot, uid, dict_id, progress, done) VALUES (?1, ?2, ?3, ?4, ?5, ?6)")?;
    for (i, s) in d.slots.iter().enumerate() {
        st.execute(params![id, i as i64, s.uid, s.dict, s.progress, s.done as i64])?;
    }
    Ok(())
}

fn load_periodic(conn: &Connection, id: AccountId, p: &mut Profile) -> Result<(), Error> {
    let head = conn
        .query_row("SELECT period, level, page, free_used FROM hunt WHERE account_id = ?1", [id], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?, r.get::<_, i64>(3)?))
        })
        .optional()?;
    if let Some((period, level, page, free_used)) = head {
        p.hunt = HuntState {
            period,
            level: u8::try_from(level).map_err(|_| Error::Range("hunt level"))?,
            page: u8::try_from(page).map_err(|_| Error::Range("hunt page"))?,
            free_used: u8::try_from(free_used).map_err(|_| Error::Range("hunt free used"))?,
            entries: Vec::new(),
        };
        let mut st = conn.prepare("SELECT dict_id, found FROM hunt_entries WHERE account_id = ?1 ORDER BY pos")?;
        for r in st.query_map([id], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))? {
            let (d, f) = r?;
            p.hunt.entries.push(HuntEntry { dict: u16::try_from(d).map_err(|_| Error::Range("hunt dict"))?, found: f != 0 });
        }
    }
    let mut st = conn.prepare("SELECT faction, start, reputation, received FROM battle_rows WHERE account_id = ?1")?;
    for r in st.query_map([id], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?, r.get::<_, i64>(3)?)))? {
        let (f, start, rep, got) = r?;
        p.battle.insert(
            u8::try_from(f).map_err(|_| Error::Range("battle faction"))?,
            BattleRow { start, reputation: u32_of(rep, "battle reputation")?, received: u8::try_from(got).map_err(|_| Error::Range("battle tiers"))? },
        );
    }
    let cal = conn
        .query_row("SELECT last_day, day, received_mask FROM login_calendar WHERE account_id = ?1", [id], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?))
        })
        .optional()?;
    if let Some((last_day, day, mask)) = cal {
        p.calendar = CalendarState {
            last_day,
            day: u8::try_from(day).map_err(|_| Error::Range("calendar day"))?,
            // stored as the two's complement i64 of the u64 mask
            received_mask: mask as u64,
        };
    }
    Ok(())
}

fn save_periodic(tx: &rusqlite::Transaction<'_>, id: AccountId, p: &Profile) -> Result<(), Error> {
    let h = &p.hunt;
    if h.period != 0 {
        tx.execute(
            "INSERT INTO hunt (account_id, period, level, page, free_used) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, h.period, h.level, h.page, h.free_used],
        )?;
        let mut st = tx.prepare("INSERT INTO hunt_entries (account_id, pos, dict_id, found) VALUES (?1, ?2, ?3, ?4)")?;
        for (i, e) in h.entries.iter().enumerate() {
            st.execute(params![id, i as i64, e.dict, e.found as i64])?;
        }
    }
    let mut st = tx.prepare("INSERT INTO battle_rows (account_id, faction, start, reputation, received) VALUES (?1, ?2, ?3, ?4, ?5)")?;
    for (f, b) in &p.battle {
        st.execute(params![id, f, b.start, b.reputation, b.received])?;
    }
    let c = &p.calendar;
    if *c != CalendarState::default() {
        tx.execute(
            "INSERT INTO login_calendar (account_id, last_day, day, received_mask) VALUES (?1, ?2, ?3, ?4)",
            params![id, c.last_day, c.day, c.received_mask as i64],
        )?;
    }
    Ok(())
}
