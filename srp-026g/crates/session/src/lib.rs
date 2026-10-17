//! The session/account store shared between the mock servers.
//!
//! It is a **mock** (see the repo `CLAUDE.md`) of the game's servers, but
//! accounts are real: one per user name, with a password kept as a salted
//! hash (see [`SessionStore::sign_in`]), and kept across restarts when the
//! store has a repository ([`persist`]). Each sign-in gets a fresh
//! `session_id`; the lobby (and the match server) map that id back to the
//! shared [`Account`].
//!
//! The store lives in one process and is shared as an `Arc<SessionStore>` across
//! the server threads (see PLAN Phase 2). Accounts are handed out as
//! `Arc<Mutex<Account>>` so a mutation in the lobby is visible everywhere that
//! holds the same account. Whoever changes an account marks it with
//! [`SessionStore::mark_dirty`]; the saver writes it soon after.
//!
//! Sessions that have not ended are kept too: a restart of srp doesn't
//! send a client in the lobby back to the login screen. Only the account a
//! session id maps to and its last sighting are kept; its connections, the
//! queue, its match and its squad are not.

use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, Shutdown, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use survarium::dictionary::Dictionary;
use survarium::v021d;

pub mod bans;
pub mod limits;
mod matchmaking;
pub mod matchmaking_policy;
pub mod password;
pub mod persist;
mod reports;
mod squads;
// The 0.26g account model, beside the 0.21d one (`resources/notes/v026g/`).
pub mod v026g;
pub use bans::{Ban, IpBan, Net};
pub use matchmaking::{
    DEFAULT_GATHER_WINDOW, FormedMatch, Level, LevelRules, MAX_PLAYERS_PER_MATCH, Matchmaking,
    Player, QueuePool, QueueState, QueuedGroup, STRENGTH_SCALE, Seat, fill_from_env,
    gather_window_from_env, growing_start_rules_from_env, leveled_start_rules_from_env,
    quiet_from_env,
};
use persist::{AccountRecord, AccountRepository, PersistError, StoredSession};
pub use reports::{MatchReport, Reports};
pub use squads::{MAX_SQUAD_MEMBERS, MIN_SQUAD_SIZE, Squad, SquadError, Squads};

/// Locking that survives a panic. The servers share this state across
/// threads, and a thread that panics while holding a lock poisons it; with
/// `lock().unwrap()` every server would then fail on it until srp restarts.
/// What the panicking thread left is used as it is.
pub trait LockExt<T> {
    fn locked(&self) -> MutexGuard<'_, T>;
}

impl<T> LockExt<T> for Mutex<T> {
    fn locked(&self) -> MutexGuard<'_, T> {
        self.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// A new `session_id`: random, from the operating system's secure
/// generator, between 0x10000 and 0x7FFF_FFFF. The id is all a lobby, chat
/// or match connection shows to prove whose it is, so it must not be
/// guessable from one's own (ids used to be handed out in order).
fn random_session_id() -> u32 {
    use password_hash::rand_core::{OsRng, RngCore};
    OsRng.next_u32() % 0x7FFE_0000 + 0x1_0000
}

/// The wall clock, Unix seconds.
fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// How often the saver writes the accounts changed since its last pass.
pub const SAVE_INTERVAL: Duration = Duration::from_secs(2);
/// Wrong passwords in a row a user name may give before it must wait.
pub const MAX_FAILED_SIGN_INS: u32 = 5;
/// How long it waits, from its last wrong password.
pub const SIGN_IN_LOCKOUT: Duration = Duration::from_secs(60);

/// Why a sign-in was refused. The client is told the same for all three
/// (`invalid_user_name_or_password`), so nothing tells a guesser which.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignInError {
    /// Empty, or longer than the client sends.
    InvalidUserName,
    WrongPassword,
    /// [`MAX_FAILED_SIGN_INS`] wrong passwords in a row, less than
    /// [`SIGN_IN_LOCKOUT`] ago.
    TooManyAttempts,
    /// The password is right, but the account is banned.
    Banned,
    /// Its address asked for too many sign-ins lately.
    TooManyFromAddress,
    /// Its address registered as many accounts as it may for now.
    RegistrationLimit,
    /// New accounts are not taken now ([`SessionStore::set_registrations_open`]).
    RegistrationsClosed,
}

/// What a player did in one match, for the lobby to reward and charge when
/// the player is back.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MatchResult {
    pub kills: u32,
    /// Batteries taken from the enemy base.
    pub steals: u32,
    /// Whether the player was still in the match when it finished.
    pub finished: bool,
    /// Rounds, medkits, bandages, anesthetics and traps used, by dict id.
    pub used: Vec<(u16, u32)>,
}

/// Full per-account state: what the lobby serves the client, and the chat's
/// friend and ignore lists.
pub struct Account {
    pub id: u32,
    /// The user name it signs in with, in lower case
    /// ([`persist::user_name_key`]).
    pub login: String,
    /// The argon2id hash of its password ([`password`]); `None` until a
    /// sign-in sets one.
    pub password_hash: Option<String>,
    pub name: String,
    /// Profiles, inventory, money and reputations, as 0.21d has them.
    pub game: v021d::account::Account,

    /// The accounts on its friend list, in the order they were added.
    pub friends: Vec<u32>,
    /// The accounts whose chat lines its client hides.
    pub ignored: Vec<u32>,
    /// The profile the client last chose (`29 04`); squad mates see it.
    pub active_profile: Option<u32>,
    /// Its Play, while it waits: a squad queues once every member has one.
    pub play: Option<Player>,
    /// Important messages (type, text) for its chat client: quest results
    /// and reputation levels.
    pub notices: Vec<(u8, String)>,
    /// Whether, and how long, it may not sign in.
    pub ban: Option<Ban>,
    /// Where it last signed in from: what banning its address bans.
    pub last_ip: Option<IpAddr>,
}

impl Account {
    /// A new account: one profile in the game's test loadout.
    pub fn new(id: u32, name: String) -> Self {
        Self {
            id,
            login: String::new(),
            password_hash: None,
            game: v021d::account::Account::new(&name, Dictionary::shared()),
            name,
            friends: Vec::new(),
            ignored: Vec::new(),
            active_profile: None,
            play: None,
            notices: Vec::new(),
            ban: None,
            last_ip: None,
        }
    }

    /// Its ban, if it holds at `now` (Unix seconds).
    pub fn ban_at(&self, now: u64) -> Option<&Ban> {
        self.ban.as_ref().filter(|ban| ban.holds(now))
    }
}

/// How long a session may go unseen before it ends: the original login
/// server's `collect_no_ping_period` (`login_server/sources/entry_point.cpp`).
/// A signed-in client pings it every second (see the login server).
pub const SESSION_IDLE_LIMIT: Duration = Duration::from_secs(600);

/// A game unheard of this long is gone: no lobby or chat connection on its
/// session, and nothing else from it (a presence ping every 10 s, a match).
/// Its chat connection stays open through a match, and one dropped comes back
/// within seconds (the client retries every 3 s).
pub const GONE_AFTER: Duration = Duration::from_secs(30);
/// A kept session's last sighting is written again at most this often: a
/// client pings every second, and that must not write every second.
pub const SESSION_SAVE_INTERVAL: Duration = Duration::from_secs(60);
/// A kept session last seen longer ago than this (wall clock) is not
/// restored at startup, and its row is deleted.
pub const SESSION_RESTORE_LIMIT: Duration = Duration::from_secs(3600);

/// One sign-in.
struct Session {
    account_id: u32,
    client_revision: u8,
    /// When the client last showed it was there: a presence ping, or a lobby,
    /// chat or match connection.
    last_seen: Instant,
    /// Lobby and chat connections open on it (see [`SessionHold`]).
    connections: u32,
    /// Its game was found gone ([`SessionStore::drop_gone_players`]); a new
    /// connection on it clears this.
    gone: bool,
    /// Signed out or idle too long, but still in a match or held by a
    /// connection: kept until neither, so a match result still reaches its
    /// account. No new connection can use it.
    ended: bool,
    /// The sockets of its connections, by hold: a kick shuts them.
    sockets: Vec<(u64, TcpStream)>,
}

/// A lobby or chat connection on a session: the session counts as seen while
/// it is open, and doesn't end for being idle. Dropping it counts as a last
/// sighting.
pub struct SessionHold<'a> {
    store: &'a SessionStore,
    session_id: u32,
    id: u64,
}

impl SessionHold<'_> {
    /// The connection's socket: kicking the session (a ban) shuts it, so
    /// its reads end at once.
    pub fn attach(&self, stream: &TcpStream) {
        let Ok(stream) = stream.try_clone() else {
            return;
        };
        if let Some(session) = self.store.sessions.locked().get_mut(&self.session_id) {
            if session.ended {
                let _ = stream.shutdown(Shutdown::Both);
            }
            session.sockets.push((self.id, stream));
        }
    }
}

impl Drop for SessionHold<'_> {
    fn drop(&mut self) {
        if let Some(session) = self.store.sessions.locked().get_mut(&self.session_id) {
            session.connections = session.connections.saturating_sub(1);
            session.last_seen = Instant::now();
            session.sockets.retain(|(id, _)| *id != self.id);
        }
        self.store.forget_if_done(self.session_id);
    }
}

/// Wrong passwords given in a row for one user name.
struct Failures {
    count: u32,
    /// When the last one came.
    last: Instant,
}

/// Where the accounts are kept, and which changed since.
struct Persistence {
    /// Held for a whole pass, so passes never overlap.
    writer: Mutex<Writer>,
    /// Accounts changed since the last pass.
    dirty: Mutex<HashSet<u32>>,
    /// Sessions made since the last pass: written on it.
    new_sessions: Mutex<HashSet<u32>>,
    /// Sessions ended since the last pass: their rows go on it.
    ended_sessions: Mutex<HashSet<u32>>,
    /// Wakes the saver before its interval is up.
    wake: Condvar,
}

struct Writer {
    repository: Box<dyn AccountRepository>,
    /// Each account as it was last written: one marked but unchanged is
    /// not written again.
    saved: HashMap<u32, AccountRecord>,
    /// When each kept session was last written (or restored): its last
    /// sighting is written again [`SESSION_SAVE_INTERVAL`] later at the
    /// soonest.
    sessions: HashMap<u32, Instant>,
}

/// Maps sign-ins to shared accounts. Cheap to clone behind an `Arc`.
///
/// Lock order: `sessions` is never held while the matchmaking is asked;
/// `logins` before `accounts`. No account lock is held while another is
/// taken, and none while a password is hashed or an account written. The
/// saver's writer comes before `sessions`, which is never held while a
/// session is written; the saver's sets of sessions are taken alone.
pub struct SessionStore {
    accounts: Mutex<HashMap<u32, Arc<Mutex<Account>>>>,
    /// Account ids by [`persist::login_key`].
    logins: Mutex<HashMap<String, u32>>,
    next_account_id: AtomicU32,
    failures: Mutex<HashMap<String, Failures>>,
    persistence: Option<Persistence>,
    sessions: Mutex<HashMap<u32, Session>>,
    matchmaking: Matchmaking,
    reports: Reports,
    /// Match results not yet given to the lobby, by account.
    match_results: Mutex<HashMap<u32, Vec<MatchResult>>>,
    squads: Squads,
    /// Banned addresses and ranges.
    ip_bans: Mutex<Vec<IpBan>>,
    next_hold_id: AtomicU64,
    /// Sign-ins and registrations per address.
    limits: limits::Limits,
    /// When the maintenance starts (Unix seconds), if one is planned. Not
    /// kept: a restart is the maintenance.
    maintenance_at: Mutex<Option<u64>>,
    /// Whether a new user name may register. Existing accounts sign in
    /// either way.
    registrations_open: AtomicBool,
}

impl SessionStore {
    /// A store whose queue gathers players for the default window.
    pub fn new() -> Self {
        Self::with_gather_window(DEFAULT_GATHER_WINDOW)
    }

    /// [`Self::with_gather_window`] with the start rules of the leveled
    /// queue set (`Matchmaking::with_start_rules`), for tests.
    pub fn with_start_rules(
        gather: Duration,
        min_players: usize,
        extension: Duration,
        longest: Duration,
    ) -> Self {
        let mut store = Self::with_gather_window(gather);
        store.matchmaking = store
            .matchmaking
            .with_start_rules(min_players, extension, longest);
        store
    }

    /// The start rules of the leveled queue (`Matchmaking::set_start_rules`).
    pub fn set_start_rules(&self, min_players: usize, extension: Duration, longest: Duration) {
        self.matchmaking
            .set_start_rules(min_players, extension, longest);
    }

    /// A store whose queue gathers players for `gather` before a match
    /// starts with all of them. Its accounts are kept in memory only.
    pub fn with_gather_window(gather: Duration) -> Self {
        Self {
            accounts: Mutex::new(HashMap::new()),
            logins: Mutex::new(HashMap::new()),
            next_account_id: AtomicU32::new(1),
            failures: Mutex::new(HashMap::new()),
            persistence: None,
            sessions: Mutex::new(HashMap::new()),
            matchmaking: Matchmaking::new(gather),
            reports: Reports::default(),
            match_results: Mutex::new(HashMap::new()),
            squads: Squads::default(),
            ip_bans: Mutex::new(Vec::new()),
            next_hold_id: AtomicU64::new(1),
            limits: limits::Limits::default(),
            maintenance_at: Mutex::new(None),
            registrations_open: AtomicBool::new(true),
        }
    }

    /// A store with every account `repository` keeps, which writes changed
    /// accounts back to it. Counters behind the ids in use are moved past
    /// them, and slots holding an item the inventory lacks are emptied.
    ///
    /// Then the sessions it keeps come back, but for those of an account it
    /// lacks or last seen more than [`SESSION_RESTORE_LIMIT`] ago, which are
    /// deleted. A restored session counts as seen now: the time srp was down
    /// doesn't count against it. New session ids follow the restored ones.
    pub fn with_repository(
        gather: Duration,
        mut repository: Box<dyn AccountRepository>,
    ) -> Result<Self, PersistError> {
        let loaded = repository.load_all()?;
        let stored_sessions = repository.load_sessions()?;
        log::info!(
            "Loaded {} accounts from {}",
            loaded.accounts.len(),
            repository.describe()
        );
        let mut store = Self::with_gather_window(gather);
        let mut saved = HashMap::new();
        let mut dirty = HashSet::new();
        let mut next_account_id = loaded.next_account_id.max(1);
        {
            let mut accounts = store.accounts.locked();
            let mut logins = store.logins.locked();
            for mut record in loaded.accounts {
                if record.repair() {
                    log::warn!("Account {} was repaired on loading", record.id);
                    dirty.insert(record.id);
                } else {
                    saved.insert(record.id, record.clone());
                }
                next_account_id = next_account_id.max(record.id + 1);
                logins.insert(record.login.clone(), record.id);
                accounts.insert(record.id, Arc::new(Mutex::new(record.into_account())));
            }
        }
        store.next_account_id = AtomicU32::new(next_account_id);

        let now = Instant::now();
        let now_unix = unix_now();
        let mut written = HashMap::new();
        let mut dropped = Vec::new();
        {
            let accounts = store.accounts.locked();
            let mut sessions = store.sessions.locked();
            for stored in stored_sessions {
                let age = now_unix.saturating_sub(stored.last_seen_unix);
                if !accounts.contains_key(&stored.account_id)
                    || age > SESSION_RESTORE_LIMIT.as_secs()
                {
                    dropped.push(stored.session_id);
                    continue;
                }
                sessions.insert(
                    stored.session_id,
                    Session {
                        account_id: stored.account_id,
                        client_revision: stored.client_revision,
                        last_seen: now,
                        connections: 0,
                        gone: false,
                        ended: false,
                        sockets: Vec::new(),
                    },
                );
                written.insert(stored.session_id, now);
            }
        }
        log::info!(
            "Restored {} sessions ({} too old or without an account dropped)",
            written.len(),
            dropped.len()
        );
        // A row that can't go now goes on the saver's first pass.
        let mut ended_sessions = HashSet::new();
        for session_id in dropped {
            if let Err(error) = repository.delete_session(session_id) {
                log::warn!("Session {session_id:#x} not deleted: {error}; retrying");
                ended_sessions.insert(session_id);
            }
        }

        let ip_bans = repository.load_ip_bans()?;
        if !ip_bans.is_empty() {
            log::info!("Loaded {} address bans", ip_bans.len());
        }
        *store.ip_bans.locked() = ip_bans;
        store.persistence = Some(Persistence {
            writer: Mutex::new(Writer {
                repository,
                saved,
                sessions: written,
            }),
            dirty: Mutex::new(dirty),
            new_sessions: Mutex::new(HashSet::new()),
            ended_sessions: Mutex::new(ended_sessions),
            wake: Condvar::new(),
        });
        Ok(store)
    }

    /// The store srp's environment asks for (see [`persist::open_from_env`]):
    /// accounts in `<SRP_DATA_DIR>/srp.db`, or in memory with `SRP_DB=off`.
    pub fn from_env(gather: Duration) -> Result<Self, PersistError> {
        let mut store = Self::open_from_env(gather)?;
        store.limits = limits::Limits::from_env();
        // `SRP_REGISTRATION=closed` starts with new accounts refused.
        if std::env::var("SRP_REGISTRATION")
            .is_ok_and(|value| value.trim().eq_ignore_ascii_case("closed"))
        {
            store.set_registrations_open(false);
        }
        Ok(store)
    }

    fn open_from_env(gather: Duration) -> Result<Self, PersistError> {
        match persist::open_from_env()? {
            Some(repository) => Self::with_repository(gather, Box::new(repository)),
            None => {
                log::warn!("SRP_DB is off: accounts are kept in memory only");
                Ok(Self::with_gather_window(gather))
            }
        }
    }

    /// Sign in as `user_name` with `password`, as the login server hears
    /// them. A user name not seen before is registered with this password:
    /// 0.21d has no separate sign-up, so the first sign-in is it. An account
    /// without a password (one whose password was cleared) takes this one.
    /// Returns the account id. Hashing takes a while: no lock is held then.
    pub fn sign_in(&self, user_name: &str, password: &[u8]) -> Result<u32, SignInError> {
        self.sign_in_at(user_name, password, Instant::now())
    }

    /// [`Self::sign_in`] at `now`, for the wrong-password lockout.
    pub fn sign_in_at(
        &self,
        user_name: &str,
        password: &[u8],
        now: Instant,
    ) -> Result<u32, SignInError> {
        self.authenticate(user_name, password, None, now)
    }

    /// [`Self::sign_in`] from `ip`, as the login server hears it: the
    /// address may ask so often, and register so many accounts
    /// ([`limits`]), and wrong passwords lock the name out for that address
    /// only, so nobody can lock another player out.
    pub fn sign_in_from(
        &self,
        user_name: &str,
        password: &[u8],
        ip: IpAddr,
        now: Instant,
    ) -> Result<u32, SignInError> {
        self.authenticate(user_name, password, Some(ip), now)
    }

    /// [`Self::sign_in_from`] for the 0.26g client, which gives the SHA-1 of
    /// its password as 40 hex characters and, when the account is
    /// remembered, nothing else ([`password::is_sha1_digest`]). The digest
    /// is the account's secret: its argon2id hash is what is kept. Anything
    /// that is not such a digest is a wrong password, and counts as none.
    pub fn sign_in_with_digest_from(
        &self,
        user_name: &str,
        digest: &[u8],
        ip: IpAddr,
        now: Instant,
    ) -> Result<u32, SignInError> {
        if !password::is_sha1_digest(digest) {
            return Err(SignInError::WrongPassword);
        }
        self.authenticate(user_name, digest, Some(ip), now)
    }

    /// The id of the account signing in as `user_name`, if there is one.
    pub fn account_id_by_name(&self, user_name: &str) -> Option<u32> {
        let key = persist::user_name_key(user_name)?;
        self.logins.locked().get(&key).copied()
    }

    fn authenticate(
        &self,
        user_name: &str,
        password: &[u8],
        ip: Option<IpAddr>,
        now: Instant,
    ) -> Result<u32, SignInError> {
        let key = persist::user_name_key(user_name).ok_or(SignInError::InvalidUserName)?;
        // Wrong passwords count per name and address: a guesser locks out
        // only itself.
        let failure_key = match ip {
            Some(ip) => format!(
                "{key}
{}",
                ip.to_canonical()
            ),
            None => key.clone(),
        };
        if self.locked_out(&failure_key, now) {
            return Err(SignInError::TooManyAttempts);
        }
        // Every check costs a password hash: an address gets so many.
        if let Some(ip) = ip
            && !self.limits.attempt(ip, now)
        {
            return Err(SignInError::TooManyFromAddress);
        }
        let existing = self.logins.locked().get(&key).copied();
        let account_id = match existing {
            Some(account_id) => account_id,
            None => {
                if !self.registrations_open() {
                    return Err(SignInError::RegistrationsClosed);
                }
                // A name others will see: no control characters.
                if user_name.chars().any(char::is_control) {
                    return Err(SignInError::InvalidUserName);
                }
                if let Some(ip) = ip
                    && !self.limits.may_register(ip, now)
                {
                    return Err(SignInError::RegistrationLimit);
                }
                let hash = password::hash(password).ok_or(SignInError::WrongPassword)?;
                let (account_id, created) = self.find_or_create(&key, user_name.trim(), Some(hash));
                if created {
                    if let Some(ip) = ip {
                        self.limits.registered(ip, now);
                    }
                    log::info!("Registered new account {}", user_name.trim());
                    return Ok(account_id);
                }
                // Registered by another sign-in meanwhile: check against it.
                account_id
            }
        };
        let account = self.account(account_id).ok_or(SignInError::WrongPassword)?;
        let stored = account.locked().password_hash.clone();
        let accepted = match stored {
            Some(stored) => password::verify(&stored, password),
            None => self.adopt_password(&account, password),
        };
        if accepted {
            self.failures.locked().remove(&failure_key);
            if account.locked().ban_at(unix_now()).is_some() {
                return Err(SignInError::Banned);
            }
            Ok(account_id)
        } else {
            let mut failures = self.failures.locked();
            if failures.len() >= 100_000 {
                failures.retain(|_, failure| {
                    now.saturating_duration_since(failure.last) < SIGN_IN_LOCKOUT
                });
            }
            let failure = failures.entry(failure_key).or_insert(Failures {
                count: 0,
                last: now,
            });
            failure.count += 1;
            failure.last = now;
            Err(SignInError::WrongPassword)
        }
    }

    /// An account without a password takes `password`; `false` if another
    /// sign-in set a different one meanwhile.
    fn adopt_password(&self, account: &Mutex<Account>, password: &[u8]) -> bool {
        let Some(new_hash) = password::hash(password) else {
            return false;
        };
        let mut locked = account.locked();
        if let Some(stored) = locked.password_hash.clone() {
            drop(locked);
            return password::verify(&stored, password);
        }
        locked.password_hash = Some(new_hash);
        let (account_id, login) = (locked.id, locked.login.clone());
        drop(locked);
        log::info!("Account {login} takes the password it signed in with");
        self.mark_dirty(account_id);
        self.save_soon();
        true
    }

    /// Whether `key` gave too many wrong passwords too recently. Once the
    /// wait is over its count starts again.
    fn locked_out(&self, key: &str, now: Instant) -> bool {
        let mut failures = self.failures.locked();
        let Some(failure) = failures.get(key) else {
            return false;
        };
        if now.saturating_duration_since(failure.last) >= SIGN_IN_LOCKOUT {
            failures.remove(key);
            return false;
        }
        failure.count >= MAX_FAILED_SIGN_INS
    }

    /// Resolve the account for a client, creating a default one, with no
    /// password, if there is none. The account is found by its user name
    /// ([`persist::login_key`]); an empty one falls back to the address.
    pub fn get_or_create_account(&self, ip: IpAddr, user_name: &str) -> u32 {
        let key = persist::login_key(ip, user_name);
        self.find_or_create(&key, user_name.trim(), None).0
    }

    /// The account of `key`, made if there is none: `(id, made)`. Ids are
    /// handed out in order. Other players see the display name, so a user
    /// name with an `@` shows only its part before it; with nothing there,
    /// the name falls back to a unique value derived from the id.
    fn find_or_create(
        &self,
        key: &str,
        user_name: &str,
        password_hash: Option<String>,
    ) -> (u32, bool) {
        let mut logins = self.logins.locked();
        if let Some(&account_id) = logins.get(key) {
            return (account_id, false);
        }
        let account_id = self.next_account_id.fetch_add(1, Ordering::Relaxed);
        let shown = user_name.split('@').next().unwrap_or_default();
        let name = if shown.is_empty() {
            format!("player_{account_id:08x}")
        } else {
            v021d::account::cap(shown)
        };
        let mut account = Account::new(account_id, name);
        account.login = key.to_owned();
        account.password_hash = password_hash;
        self.accounts
            .locked()
            .insert(account_id, Arc::new(Mutex::new(account)));
        logins.insert(key.to_owned(), account_id);
        drop(logins);
        self.mark_dirty(account_id);
        self.save_soon();
        (account_id, true)
    }

    /// The account changed: the saver writes it on its next pass. Never
    /// waits on an account, so it may be called with one locked.
    pub fn mark_dirty(&self, account_id: u32) {
        if let Some(persistence) = &self.persistence {
            persistence.dirty.locked().insert(account_id);
        }
    }

    /// Whether the account is marked and not written yet.
    pub fn is_dirty(&self, account_id: u32) -> bool {
        self.persistence
            .as_ref()
            .is_some_and(|persistence| persistence.dirty.locked().contains(&account_id))
    }

    /// Wake the saver now rather than at its next interval.
    pub fn save_soon(&self) {
        if let Some(persistence) = &self.persistence {
            persistence.wake.notify_one();
        }
    }

    /// Write every account marked since the last pass, now: after a match's
    /// rewards, and before the process exits. Returns how many accounts were
    /// written. The sessions made, seen or ended since go with them. Call it
    /// with no account locked.
    pub fn flush(&self) -> usize {
        let saved = self.save_dirty();
        if saved > 0 {
            log::info!("Saved {saved} accounts");
        }
        saved
    }

    /// One pass at the present time ([`Self::save_dirty_at`]).
    fn save_dirty(&self) -> usize {
        self.save_dirty_at(Instant::now())
    }

    /// One pass: each marked account is copied out under its own lock, one
    /// at a time, and written with none held. An account equal to what was
    /// last written is skipped; one that fails stays marked for the next
    /// pass. Then the sessions ([`Self::save_sessions`]), as at `now`.
    /// Returns how many accounts were written.
    fn save_dirty_at(&self, now: Instant) -> usize {
        let Some(persistence) = &self.persistence else {
            return 0;
        };
        let mut writer = persistence.writer.locked();
        let mut dirty: Vec<u32> = std::mem::take(&mut *persistence.dirty.locked())
            .into_iter()
            .collect();
        dirty.sort_unstable();
        let mut written = 0;
        for account_id in dirty {
            let Some(account) = self.account(account_id) else {
                continue;
            };
            let record = AccountRecord::of(&account.locked());
            if writer.saved.get(&account_id) == Some(&record) {
                continue;
            }
            match writer.repository.save_account(&record) {
                Ok(()) => {
                    writer.saved.insert(account_id, record);
                    written += 1;
                }
                Err(error) => {
                    log::error!("Account {account_id} not saved: {error}; retrying");
                    persistence.dirty.locked().insert(account_id);
                }
            }
        }
        // After the accounts: a session's row needs its account's.
        let sessions = self.save_sessions(persistence, &mut writer, now);
        if sessions > 0 {
            log::debug!("Saved {sessions} sessions");
        }
        written
    }

    /// The sessions' part of a pass. A session made since the last pass is
    /// written; a kept one whose client was seen since it was written, at
    /// least [`SESSION_SAVE_INTERVAL`] ago, is written again with its last
    /// sighting (one held by a connection counts as seen at `now`). The
    /// sessions are copied out first, so none is written with `sessions`
    /// locked. Then the rows of the sessions ended since go. A write that
    /// fails is retried on the next pass. Returns how many were written.
    fn save_sessions(&self, persistence: &Persistence, writer: &mut Writer, now: Instant) -> usize {
        let made = std::mem::take(&mut *persistence.new_sessions.locked());
        let live: Vec<(u32, u32, Instant, u8)> = self
            .sessions
            .locked()
            .iter()
            .filter(|(_, session)| !session.ended)
            .map(|(&session_id, session)| {
                let seen = if session.connections > 0 {
                    now
                } else {
                    session.last_seen
                };
                (
                    session_id,
                    session.account_id,
                    seen,
                    session.client_revision,
                )
            })
            .collect();
        // The wall clock's now and the monotonic one, to convert sightings
        // (the pass's `now` may be a test's later time).
        let (now_unix, clock) = (unix_now(), Instant::now());
        let mut written = 0;
        for (session_id, account_id, last_seen, client_revision) in live {
            let due = made.contains(&session_id)
                || writer.sessions.get(&session_id).is_some_and(|&at| {
                    now.saturating_duration_since(at) >= SESSION_SAVE_INTERVAL && last_seen > at
                });
            // An account the store lacks has no row to point at.
            if !due || self.account(account_id).is_none() {
                continue;
            }
            let stored = StoredSession {
                session_id,
                account_id,
                client_revision,
                last_seen_unix: now_unix
                    .saturating_sub(clock.saturating_duration_since(last_seen).as_secs()),
            };
            match writer.repository.save_session(&stored) {
                Ok(()) => {
                    writer.sessions.insert(session_id, now);
                    written += 1;
                }
                Err(error) => {
                    log::error!("Session {session_id:#x} not saved: {error}; retrying");
                    persistence.new_sessions.locked().insert(session_id);
                }
            }
        }
        // Taken after the writes: a session that ended meanwhile loses the
        // row just written in this pass, not the next.
        let ended = std::mem::take(&mut *persistence.ended_sessions.locked());
        for session_id in ended {
            match writer.repository.delete_session(session_id) {
                Ok(()) => {
                    writer.sessions.remove(&session_id);
                }
                Err(error) => {
                    log::error!("Session {session_id:#x} not deleted: {error}; retrying");
                    persistence.ended_sessions.locked().insert(session_id);
                }
            }
        }
        written
    }

    /// The saver writes these sessions, made just now, soon.
    fn sessions_made(&self, session_ids: &[u32]) {
        if let Some(persistence) = &self.persistence {
            persistence.new_sessions.locked().extend(session_ids);
            persistence.wake.notify_one();
        }
    }

    /// The saver deletes the rows of these sessions, ended just now, soon.
    fn sessions_ended(&self, session_ids: &[u32]) {
        if let Some(persistence) = &self.persistence
            && !session_ids.is_empty()
        {
            persistence.ended_sessions.locked().extend(session_ids);
            persistence.wake.notify_one();
        }
    }

    /// Start the saver: every [`SAVE_INTERVAL`], or sooner when woken, it
    /// writes the accounts marked since, and the sessions made, seen or
    /// ended since. `None` for a store kept in memory.
    pub fn spawn_saver(self: &Arc<Self>) -> Option<std::thread::JoinHandle<()>> {
        self.persistence.as_ref()?;
        let store = self.clone();
        let saver = std::thread::Builder::new()
            .name("saver".to_owned())
            .spawn(move || {
                loop {
                    if let Some(persistence) = &store.persistence {
                        let dirty = persistence.dirty.locked();
                        let _ = persistence.wake.wait_timeout(dirty, SAVE_INTERVAL);
                    }
                    let saved = store.save_dirty();
                    if saved > 0 {
                        log::debug!("Saved {saved} accounts");
                    }
                }
            })
            .expect("failed to spawn the saver thread");
        Some(saver)
    }

    /// The account with this id, if there is one.
    pub fn account(&self, account_id: u32) -> Option<Arc<Mutex<Account>>> {
        self.accounts.locked().get(&account_id).cloned()
    }

    /// The accounts whose name contains `part`, ignoring ASCII case, by
    /// name: at most `limit`, `except` left out.
    pub fn find_accounts(&self, part: &[u8], except: u32, limit: usize) -> Vec<(u32, String)> {
        if part.is_empty() {
            return Vec::new();
        }
        let accounts: Vec<_> = self.accounts.locked().values().cloned().collect();
        let mut found: Vec<(u32, String)> = accounts
            .iter()
            .filter_map(|account| {
                let account = account.locked();
                let name = account.name.as_bytes();
                let matches = name
                    .windows(part.len())
                    .any(|window| window.eq_ignore_ascii_case(part));
                (account.id != except && matches).then(|| (account.id, account.name.clone()))
            })
            .collect();
        found.sort_by(|lhs, rhs| lhs.1.cmp(&rhs.1).then(lhs.0.cmp(&rhs.0)));
        found.truncate(limit);
        found
    }

    /// Every account by rating, best first (then by name): the ranking.
    /// Locks each account in turn, so the caller must hold none.
    pub fn ranking(&self) -> Vec<survarium::v021d::lobby::Ranked> {
        let accounts: Vec<_> = self.accounts.locked().values().cloned().collect();
        let mut rows: Vec<_> = accounts
            .iter()
            .map(|account| {
                let account = account.locked();
                survarium::v021d::lobby::Ranked {
                    account_id: account.id,
                    name: account.name.clone(),
                    rating: account.game.rating,
                    rating_before: account.game.rating_before,
                }
            })
            .collect();
        rows.sort_by(|lhs, rhs| {
            rhs.rating
                .cmp(&lhs.rating)
                .then_with(|| lhs.name.cmp(&rhs.name))
                .then(lhs.account_id.cmp(&rhs.account_id))
        });
        rows
    }

    /// Allocate a fresh `session_id` bound to `account_id`. The saver keeps
    /// it soon.
    pub fn create_session(&self, account_id: u32) -> u32 {
        self.create_session_with_revision(account_id, 0)
    }

    /// Keep the advertised patch revision with this sign-in, including restarts.
    pub fn create_session_with_revision(&self, account_id: u32, client_revision: u8) -> u32 {
        // A sign-in is a new game: a squad the account is in is one of the
        // game before, which quit or crashed without leaving it (Alt+F4 says
        // nothing), and a place in the queue is that game's too.
        self.leave_squad(account_id, "it signed in again");
        let before: Vec<u32> = self
            .sessions
            .locked()
            .iter()
            .filter(|(_, session)| session.account_id == account_id && !session.ended)
            .map(|(session_id, _)| *session_id)
            .collect();
        for session_id in before {
            self.matchmaking.leave_queue(session_id);
        }
        let mut sessions = self.sessions.locked();
        let session_id = loop {
            let session_id = random_session_id();
            if !sessions.contains_key(&session_id) {
                break session_id;
            }
        };
        sessions.insert(
            session_id,
            Session {
                account_id,
                client_revision,
                last_seen: Instant::now(),
                connections: 0,
                gone: false,
                ended: false,
                sockets: Vec::new(),
            },
        );
        self.sessions_made(&[session_id]);
        session_id
    }

    /// Revision advertised at login; absent for unknown or ended sessions.
    pub fn client_revision(&self, session_id: u32) -> Option<u8> {
        self.sessions
            .locked()
            .get(&session_id)
            .filter(|session| !session.ended)
            .map(|session| session.client_revision)
    }

    /// The shared account a `session_id` resolves to, if the session is known
    /// and hasn't ended.
    pub fn account_for_session(&self, session_id: u32) -> Option<Arc<Mutex<Account>>> {
        let account_id = self.live_account_id(session_id)?;
        self.accounts.locked().get(&account_id).cloned()
    }

    /// The account of a session that hasn't ended.
    fn live_account_id(&self, session_id: u32) -> Option<u32> {
        let sessions = self.sessions.locked();
        let session = sessions.get(&session_id).filter(|session| !session.ended)?;
        Some(session.account_id)
    }

    /// The account of a session, ended or not: a match result still goes to
    /// the account of a session that ended during its match.
    fn any_account_id(&self, session_id: u32) -> Option<u32> {
        Some(self.sessions.locked().get(&session_id)?.account_id)
    }

    /// A presence ping: the session is still there. `false` for a session
    /// that is unknown or has ended.
    pub fn touch(&self, session_id: u32) -> bool {
        match self.sessions.locked().get_mut(&session_id) {
            Some(session) if !session.ended => {
                session.last_seen = Instant::now();
                true
            }
            _ => false,
        }
    }

    /// Hold a session for a lobby or chat connection. `None` for a session
    /// that is unknown or has ended: the connection must be refused.
    pub fn hold(&self, session_id: u32) -> Option<SessionHold<'_>> {
        let mut sessions = self.sessions.locked();
        let session = sessions
            .get_mut(&session_id)
            .filter(|session| !session.ended)?;
        session.connections += 1;
        session.last_seen = Instant::now();
        session.gone = false;
        Some(SessionHold {
            store: self,
            session_id,
            id: self.next_hold_id.fetch_add(1, Ordering::Relaxed),
        })
    }

    /// End a session: signed out, or idle too long. It leaves the queue, and
    /// no new connection can use it. A session in a match or held by a
    /// connection is kept, ended, until neither; then it is forgotten. Its
    /// kept row goes at once, so a restart doesn't bring it back.
    /// `false` if it was unknown or had ended already.
    pub fn end_session(&self, session_id: u32) -> bool {
        let account_id = match self.sessions.locked().get_mut(&session_id) {
            Some(session) if !session.ended => {
                session.ended = true;
                session.account_id
            }
            _ => return false,
        };
        self.sessions_ended(&[session_id]);
        self.matchmaking.leave_queue(session_id);
        if !self.plays_on_elsewhere(account_id, session_id, Instant::now()) {
            self.leave_squad(account_id, "its session ended (signed out)");
        }
        self.forget_if_done(session_id);
        true
    }

    /// Players whose game is gone ([`GONE_AFTER`] without a lobby or chat
    /// connection or any other sign of it). A game closed without signing
    /// out (Alt+F4, a crash, a lost connection) says nothing: its session
    /// kept its place in the queue until it ended for being idle (10 min),
    /// and its squad for good, so the mates saw it in theirs and it found
    /// the squad again when it signed in. Each leaves the queue, and its
    /// squad unless the account plays on under another session; the mates'
    /// lobbies show the squad without it on their next update. Returns
    /// their session ids, each once.
    pub fn drop_gone_players(&self, now: Instant) -> Vec<u32> {
        let gone: Vec<(u32, u32)> = self
            .sessions
            .locked()
            .iter_mut()
            .filter(|(_, session)| {
                !session.ended
                    && !session.gone
                    && session.connections == 0
                    && now.saturating_duration_since(session.last_seen) > GONE_AFTER
            })
            .map(|(session_id, session)| {
                session.gone = true;
                (*session_id, session.account_id)
            })
            .collect();
        for &(session_id, account_id) in &gone {
            self.matchmaking.leave_queue(session_id);
            if !self.plays_on_elsewhere(account_id, session_id, now) {
                self.leave_squad(
                    account_id,
                    "its game is gone (no lobby or chat connection, nor anything else from it, for 30 s)",
                );
            }
        }
        gone.into_iter().map(|(session_id, _)| session_id).collect()
    }

    /// Whether the account has a game other than `session_id` that is there.
    fn plays_on_elsewhere(&self, account_id: u32, session_id: u32, now: Instant) -> bool {
        self.sessions.locked().iter().any(|(other, session)| {
            *other != session_id
                && session.account_id == account_id
                && !session.ended
                && (session.connections > 0
                    || now.saturating_duration_since(session.last_seen) <= GONE_AFTER)
        })
    }

    /// The account leaves its squad, if it is in one; `why` goes to the log.
    fn leave_squad(&self, account_id: u32, why: &str) {
        let Some(squad) = self.squads.of(account_id) else {
            return;
        };
        if self.squads.leave(account_id).is_ok() {
            let name = self.account(account_id).map_or_else(
                || format!("account {account_id}"),
                |account| account.locked().name.clone(),
            );
            log::info!("{name} left squad {}: {why}", squad.id);
        }
    }

    /// End the sessions unseen for longer than `idle` at `now` which no
    /// connection holds and which aren't in a match or the queue (the
    /// original login server's collector, `query_processor.cpp`). Their kept
    /// rows go too. Returns their ids.
    pub fn end_idle_sessions(&self, now: Instant, idle: Duration) -> Vec<u32> {
        let mut ended = Vec::new();
        let busy = self.matchmaking.busy_sessions();
        let mut sessions = self.sessions.locked();
        for (session_id, session) in sessions.iter_mut() {
            if !session.ended
                && session.connections == 0
                && now.saturating_duration_since(session.last_seen) > idle
                && !busy.contains(session_id)
            {
                session.ended = true;
                ended.push(*session_id);
            }
        }
        // Not in a match and not held: nothing needs them any more.
        sessions.retain(|session_id, _| !ended.contains(session_id));
        drop(sessions);
        self.sessions_ended(&ended);
        ended
    }

    /// The players online: accounts with a lobby or chat connection open on
    /// a session that hasn't ended.
    pub fn players_online(&self) -> usize {
        let accounts: std::collections::HashSet<u32> = self
            .sessions
            .locked()
            .values()
            .filter(|session| session.connections > 0 && !session.ended)
            .map(|session| session.account_id)
            .collect();
        accounts.len()
    }

    /// How many accounts are registered.
    pub fn registered_accounts(&self) -> usize {
        self.logins.locked().len()
    }

    /// The players on the server: those online (see [`Self::players_online`])
    /// and those playing a match, whose lobby connection is closed meanwhile.
    pub fn players_present(&self) -> usize {
        let busy = self.matchmaking.busy_sessions();
        let accounts: std::collections::HashSet<u32> = self
            .sessions
            .locked()
            .iter()
            .filter(|(session_id, session)| {
                !session.ended && (session.connections > 0 || busy.contains(session_id))
            })
            .map(|(_, session)| session.account_id)
            .collect();
        accounts.len()
    }

    /// Whether the account has a lobby or chat connection open.
    pub fn is_online(&self, account_id: u32) -> bool {
        self.sessions.locked().values().any(|session| {
            session.account_id == account_id && session.connections > 0 && !session.ended
        })
    }

    /// The queue and the matches being played.
    pub fn matchmaking(&self) -> &Matchmaking {
        &self.matchmaking
    }

    /// Finished matches' results.
    pub fn reports(&self) -> &Reports {
        &self.reports
    }

    /// How many matches are being played.
    pub fn match_count(&self) -> usize {
        self.matchmaking.match_count()
    }

    /// The match server saw the session's client: it counts as a sighting.
    /// A session that ended meanwhile is forgotten once it is not seated.
    pub fn seen_in_match(&self, session_id: u32) {
        if let Some(session) = self.sessions.locked().get_mut(&session_id) {
            session.last_seen = Instant::now();
        }
        self.forget_if_done(session_id);
    }

    /// The account of a session, ended or not, for the match server.
    pub fn session_account(&self, session_id: u32) -> Option<Arc<Mutex<Account>>> {
        let account_id = self.any_account_id(session_id)?;
        self.account(account_id)
    }

    /// The squads, shared by the chat (invites) and the lobby.
    pub fn squads(&self) -> &Squads {
        &self.squads
    }

    /// How many sessions are kept, ended ones still in a match included.
    pub fn session_count(&self) -> usize {
        self.sessions.locked().len()
    }

    /// Forget an ended session nothing needs any more: no connection holds
    /// it and it isn't in a match.
    fn forget_if_done(&self, session_id: u32) {
        let busy = self.matchmaking.busy_sessions().contains(&session_id);
        let mut sessions = self.sessions.locked();
        let done = !busy
            && sessions
                .get(&session_id)
                .is_some_and(|session| session.ended && session.connections == 0);
        if done {
            sessions.remove(&session_id);
        }
    }

    /// The account the admin means by `text`: a user name it signs in with,
    /// or else the name other players see, if only one account has it.
    pub fn find_account(&self, text: &str) -> Option<u32> {
        if let Some(account_id) = self.account_id_by_name(text) {
            return Some(account_id);
        }
        let text = text.trim();
        let accounts: Vec<_> = self.accounts.locked().values().cloned().collect();
        let mut found = accounts.iter().filter_map(|account| {
            let account = account.locked();
            account
                .name
                .eq_ignore_ascii_case(text)
                .then_some(account.id)
        });
        let first = found.next()?;
        found.next().is_none().then_some(first)
    }

    /// The account signed in from `ip`: banning its address bans that.
    pub fn signed_in_from(&self, account_id: u32, ip: IpAddr) {
        if let Some(account) = self.account(account_id) {
            let mut account = account.locked();
            if account.last_ip == Some(ip.to_canonical()) {
                return;
            }
            account.last_ip = Some(ip.to_canonical());
        }
        self.mark_dirty(account_id);
    }

    /// The ban of an account, if one holds now: why a sign-in was refused
    /// with [`SignInError::Banned`].
    pub fn account_ban(&self, account_id: u32) -> Option<Ban> {
        let account = self.account(account_id)?;
        let ban = account.locked().ban_at(unix_now()).cloned();
        ban
    }

    /// The ban of `ip`, if one holds now.
    pub fn ip_ban(&self, ip: IpAddr) -> Option<IpBan> {
        let now = unix_now();
        self.ip_bans
            .locked()
            .iter()
            .find(|ban| ban.ban.holds(now) && ban.net.contains(ip))
            .cloned()
    }

    /// Ban an account: it can't sign in, and its sessions end now, their
    /// lobby and chat connections closed and its place in the queue given
    /// up. Returns how many sessions ended.
    pub fn ban_account(&self, account_id: u32, ban: Ban) -> usize {
        let Some(account) = self.account(account_id) else {
            return 0;
        };
        account.locked().ban = Some(ban);
        self.mark_dirty(account_id);
        self.flush();
        // What it won in a match it was playing is not paid out.
        self.match_results.locked().remove(&account_id);
        self.kick(|session| session.account_id == account_id)
    }

    /// Lift an account's ban. `false` if it had none.
    pub fn unban_account(&self, account_id: u32) -> bool {
        let Some(account) = self.account(account_id) else {
            return false;
        };
        let had = account.locked().ban.take().is_some();
        if had {
            self.mark_dirty(account_id);
            self.flush();
        }
        had
    }

    /// Ban an address or a range: nobody signs in or registers from it, and
    /// the sessions of the accounts last signed in from it end now. Returns
    /// how many sessions ended.
    pub fn ban_net(&self, net: Net, ban: Ban) -> usize {
        let ip_ban = IpBan { net, ban };
        if let Some(persistence) = &self.persistence
            && let Err(error) = persistence.writer.locked().repository.save_ip_ban(&ip_ban)
        {
            log::error!("The ban of {net} was not saved: {error}");
        }
        {
            let mut bans = self.ip_bans.locked();
            bans.retain(|kept| kept.net != net);
            bans.push(ip_ban);
        }
        let accounts: Vec<_> = self.accounts.locked().values().cloned().collect();
        let covered: HashSet<u32> = accounts
            .iter()
            .filter_map(|account| {
                let account = account.locked();
                account
                    .last_ip
                    .is_some_and(|ip| net.contains(ip))
                    .then_some(account.id)
            })
            .collect();
        self.kick(|session| covered.contains(&session.account_id))
    }

    /// Lift the ban of exactly this address or range. `false` if none.
    pub fn unban_net(&self, net: Net) -> bool {
        let had = {
            let mut bans = self.ip_bans.locked();
            let before = bans.len();
            bans.retain(|kept| kept.net != net);
            bans.len() != before
        };
        if had
            && let Some(persistence) = &self.persistence
            && let Err(error) = persistence.writer.locked().repository.delete_ip_ban(&net)
        {
            log::error!("The lifted ban of {net} was not deleted: {error}");
        }
        had
    }

    /// The bans in force: accounts (id, user name, shown name, ban, last
    /// address), then addresses. Ones that ran out are left out.
    pub fn bans(&self) -> (Vec<(u32, String, String, Ban, Option<IpAddr>)>, Vec<IpBan>) {
        let now = unix_now();
        let accounts: Vec<_> = self.accounts.locked().values().cloned().collect();
        let mut banned: Vec<_> = accounts
            .iter()
            .filter_map(|account| {
                let account = account.locked();
                let ban = account.ban_at(now)?.clone();
                Some((
                    account.id,
                    account.login.clone(),
                    account.name.clone(),
                    ban,
                    account.last_ip,
                ))
            })
            .collect();
        banned.sort_by_key(|row| row.0);
        let mut nets: Vec<_> = self
            .ip_bans
            .locked()
            .iter()
            .filter(|ban| ban.ban.holds(now))
            .cloned()
            .collect();
        nets.sort_by_key(|ban| ban.net.to_string());
        (banned, nets)
    }

    /// End the sessions `which` picks and shut their connections. Returns
    /// how many there were.
    fn kick(&self, which: impl Fn(&Session) -> bool) -> usize {
        let picked: Vec<u32> = self
            .sessions
            .locked()
            .iter()
            .filter(|(_, session)| which(session))
            .map(|(session_id, _)| *session_id)
            .collect();
        for &session_id in &picked {
            self.end_session(session_id);
            self.matchmaking.leave_queue(session_id);
            if let Some(session) = self.sessions.locked().get(&session_id) {
                for (_, socket) in &session.sockets {
                    let _ = socket.shutdown(Shutdown::Both);
                }
            }
        }
        picked.len()
    }

    /// Take new accounts, or stop taking them: those who have one sign in
    /// and play as before. Kept in memory: a restart goes back to
    /// `SRP_REGISTRATION`.
    pub fn set_registrations_open(&self, open: bool) {
        self.registrations_open.store(open, Ordering::Relaxed);
    }

    pub fn registrations_open(&self) -> bool {
        self.registrations_open.load(Ordering::Relaxed)
    }

    /// Plan the maintenance for `at` (Unix seconds), or cancel it with
    /// `None`. The clients show the minutes left from an hour on, lock Play
    /// at 30 and quit when the time comes.
    pub fn set_maintenance(&self, at: Option<u64>) {
        *self.maintenance_at.locked() = at;
    }

    /// When the maintenance starts, if one is planned.
    pub fn maintenance_at(&self) -> Option<u64> {
        *self.maintenance_at.locked()
    }

    /// The minutes left to it, rounded up, as the clients are told: `None`
    /// when none is planned, 0 once its time has come.
    pub fn minutes_to_maintenance(&self) -> Option<u32> {
        let at = self.maintenance_at()?;
        let left = at.saturating_sub(unix_now());
        Some(left.div_ceil(60).min(u64::from(u32::MAX)) as u32)
    }

    /// Whether the session was kicked or has otherwise ended.
    pub fn has_ended(&self, session_id: u32) -> bool {
        self.live_account_id(session_id).is_none()
    }

    /// Keep what a session's player did in a match until the lobby takes it.
    pub fn record_match_result(&self, session_id: u32, result: MatchResult) {
        let Some(account_id) = self.any_account_id(session_id) else {
            return;
        };
        self.match_results
            .locked()
            .entry(account_id)
            .or_default()
            .push(result);
    }

    /// The match results of a session's account not taken yet, oldest first.
    pub fn take_match_results(&self, session_id: u32) -> Vec<MatchResult> {
        let Some(account_id) = self.live_account_id(session_id) else {
            return Vec::new();
        };
        self.match_results
            .locked()
            .remove(&account_id)
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    fn ip(a: u8, b: u8, c: u8, d: u8) -> IpAddr {
        IpAddr::V4(Ipv4Addr::new(a, b, c, d))
    }

    #[test]
    fn same_client_resolves_to_same_account() {
        let store = SessionStore::new();
        let first = store.get_or_create_account(ip(127, 0, 0, 1), "a@b.c");
        let again = store.get_or_create_account(ip(127, 0, 0, 1), "a@b.c");
        assert_eq!(first, again);
    }

    #[test]
    fn the_user_name_alone_finds_the_account_in_any_case_from_anywhere() {
        let store = SessionStore::new();
        let a = store.get_or_create_account(ip(127, 0, 0, 1), "a@b.c");
        let b = store.get_or_create_account(ip(127, 0, 0, 1), "other@b.c");
        let c = store.get_or_create_account(ip(10, 0, 0, 1), " A@B.c ");
        assert_ne!(a, b);
        assert_eq!(a, c);
        // Ids are handed out in order, from 1.
        assert_eq!((a, b), (1, 2));
        // Without a user name, the address tells clients apart.
        let first = store.get_or_create_account(ip(127, 0, 0, 1), "");
        let second = store.get_or_create_account(ip(10, 0, 0, 1), "");
        assert_ne!(first, second);
    }

    #[test]
    fn the_first_sign_in_registers_and_its_password_signs_in_again() {
        let store = SessionStore::new();
        let id = store.sign_in("stepan", b"test-password-1").unwrap();
        assert_eq!(store.sign_in("stepan", b"test-password-1"), Ok(id));
        assert_eq!(store.sign_in(" Stepan ", b"test-password-1"), Ok(id));
        assert_eq!(
            store.sign_in("STEPAN", b"test-password-2"),
            Err(SignInError::WrongPassword)
        );
        // The name keeps the casing it registered with.
        let account = store.account(id).unwrap();
        let account = account.locked();
        assert_eq!(
            (account.name.as_str(), account.login.as_str()),
            ("stepan", "stepan")
        );
        let hash = account.password_hash.clone().unwrap();
        assert!(hash.starts_with("$argon2id$"));
        assert!(!hash.contains("test-password-1"));
    }

    #[test]
    fn an_email_is_a_user_name_with_an_at() {
        let store = SessionStore::new();
        let id = store
            .sign_in("Ivan@Mail.example", b"test-password-1")
            .unwrap();
        assert_eq!(
            store.sign_in("ivan@mail.example", b"test-password-1"),
            Ok(id)
        );
        let other = store.sign_in("a@b", b"test-password-2").unwrap();
        assert_eq!(store.sign_in("A@B", b"test-password-2"), Ok(other));
        assert_ne!(id, other);
        assert_eq!(store.account(id).unwrap().locked().name, "Ivan");
    }

    #[test]
    fn an_empty_or_over_long_user_name_is_refused() {
        let store = SessionStore::new();
        for refused in ["", "   ", &"x".repeat(persist::MAX_USER_NAME_LEN + 1)] {
            assert_eq!(
                store.sign_in(refused, b"test-password-1"),
                Err(SignInError::InvalidUserName)
            );
        }
        let longest = "x".repeat(persist::MAX_USER_NAME_LEN);
        assert!(store.sign_in(&longest, b"test-password-1").is_ok());
        assert_eq!(store.account(1).unwrap().locked().name.len(), 63);
    }

    #[test]
    fn an_account_without_a_password_takes_the_next_one() {
        let store = SessionStore::new();
        let id = store.get_or_create_account(ip(127, 0, 0, 1), "legacy");
        assert_eq!(store.sign_in("legacy", b"test-password-1"), Ok(id));
        assert_eq!(
            store.sign_in("legacy", b"test-password-2"),
            Err(SignInError::WrongPassword)
        );
    }

    #[test]
    fn five_wrong_passwords_lock_the_name_out_for_a_while() {
        let store = SessionStore::new();
        let start = Instant::now();
        let id = store
            .sign_in_at("guarded", b"test-password-1", start)
            .unwrap();
        for _ in 0..MAX_FAILED_SIGN_INS {
            assert_eq!(
                store.sign_in_at("guarded", b"wrong", start),
                Err(SignInError::WrongPassword)
            );
        }
        // Even the right one waits; another name doesn't.
        let soon = start + SIGN_IN_LOCKOUT / 2;
        assert_eq!(
            store.sign_in_at("Guarded", b"test-password-1", soon),
            Err(SignInError::TooManyAttempts)
        );
        assert!(store.sign_in_at("other", b"test-password-2", soon).is_ok());
        // After the wait it is let in, and a success forgets the failures.
        let after = start + SIGN_IN_LOCKOUT;
        assert_eq!(
            store.sign_in_at("guarded", b"test-password-1", after),
            Ok(id)
        );
        for _ in 0..MAX_FAILED_SIGN_INS - 1 {
            let _ = store.sign_in_at("guarded", b"wrong", after);
        }
        assert_eq!(
            store.sign_in_at("guarded", b"test-password-1", after),
            Ok(id)
        );
    }

    #[test]
    fn empty_email_is_accepted_with_fallback_name() {
        let store = SessionStore::new();
        let id = store.get_or_create_account(ip(127, 0, 0, 1), "");
        let session = store.create_session(id);
        let account = store.account_for_session(session).unwrap();
        assert_eq!(account.locked().name, format!("player_{id:08x}"));
    }

    #[test]
    fn sessions_are_random_distinct_and_resolve() {
        let store = SessionStore::new();
        let id = store.get_or_create_account(ip(127, 0, 0, 1), "a@b.c");
        let sessions: Vec<u32> = (0..200).map(|_| store.create_session(id)).collect();
        assert!(
            sessions
                .iter()
                .all(|session| (0x1_0000..0x8000_0000).contains(session))
        );
        assert_eq!(
            sessions.iter().collect::<HashSet<_>>().len(),
            sessions.len()
        );
        // Not one after the other: one's own id tells nothing of the others.
        let next = sessions.windows(2).filter(|pair| pair[1] == pair[0] + 1);
        assert!(next.count() < 3);

        let account = store.account_for_session(sessions[0]).unwrap();
        assert_eq!(account.locked().id, id);
        assert!(store.account_for_session(0xFFFF_FFFF).is_none());
    }

    fn forever() -> Ban {
        Ban {
            until: None,
            reason: "test".into(),
        }
    }

    #[test]
    fn a_banned_account_is_refused_only_with_its_right_password_until_lifted() {
        let store = SessionStore::new();
        let id = store.sign_in("cheater", b"pw").unwrap();
        let session = store.create_session(id);
        let held = store.hold(session).unwrap();
        assert_eq!(store.ban_account(id, forever()), 1);
        assert!(store.has_ended(session));
        assert!(store.hold(session).is_none());
        drop(held);
        assert_eq!(store.sign_in("cheater", b"pw"), Err(SignInError::Banned));
        assert_eq!(
            store.sign_in("cheater", b"wrong"),
            Err(SignInError::WrongPassword)
        );
        assert_eq!(store.bans().0.len(), 1);
        assert!(store.unban_account(id));
        assert_eq!(store.sign_in("cheater", b"pw"), Ok(id));
        assert!(store.bans().0.is_empty());

        // A ban that ran out holds no more.
        store.ban_account(
            id,
            Ban {
                until: Some(1),
                reason: String::new(),
            },
        );
        assert_eq!(store.sign_in("cheater", b"pw"), Ok(id));
    }

    #[test]
    fn a_digest_registers_and_signs_in_as_a_password_does() {
        const DIGEST: &[u8] = b"da39a3ee5e6b4b0d3255bfef95601890afd80709";
        const OTHER: &[u8] = b"356a192b7913b04c54574d18c28d46e6395428ab";
        let store = SessionStore::new();
        let from = ip(127, 0, 0, 1);
        let now = Instant::now();
        let id = store
            .sign_in_with_digest_from("stepan", DIGEST, from, now)
            .unwrap();
        assert_eq!(
            store.sign_in_with_digest_from("Stepan", DIGEST, from, now),
            Ok(id)
        );
        assert_eq!(
            store.sign_in_with_digest_from("stepan", OTHER, from, now),
            Err(SignInError::WrongPassword)
        );
        // Kept as an argon2id hash of the digest, never the digest itself.
        let hash = store
            .account(id)
            .unwrap()
            .locked()
            .password_hash
            .clone()
            .unwrap();
        assert!(hash.starts_with("$argon2id$"));
        assert!(!hash.contains(std::str::from_utf8(DIGEST).unwrap()));
        assert!(password::verify(&hash, DIGEST));

        // Not a digest: refused, nothing registered, no wrong password counted.
        for _ in 0..=MAX_FAILED_SIGN_INS {
            assert_eq!(
                store.sign_in_with_digest_from("newcomer", b"test-password-1", from, now),
                Err(SignInError::WrongPassword)
            );
        }
        assert_eq!(store.account_id_by_name("newcomer"), None);
        assert!(
            store
                .sign_in_with_digest_from("newcomer", DIGEST, from, now)
                .is_ok()
        );
    }

    #[test]
    fn an_account_says_its_ban_while_it_holds() {
        let store = SessionStore::new();
        let id = store.sign_in("cheater", b"pw").unwrap();
        assert_eq!(store.account_ban(id), None);
        store.ban_account(id, forever());
        assert_eq!(store.account_ban(id), Some(forever()));
        store.ban_account(
            id,
            Ban {
                until: Some(1),
                reason: String::new(),
            },
        );
        assert_eq!(store.account_ban(id), None);
        assert_eq!(store.account_ban(0xBAD), None);
    }

    #[test]
    fn closed_registrations_refuse_new_names_only() {
        let store = SessionStore::new();
        let id = store.sign_in("old_hand", b"pw").unwrap();
        store.set_registrations_open(false);
        assert_eq!(
            store.sign_in("newcomer", b"pw"),
            Err(SignInError::RegistrationsClosed)
        );
        assert_eq!(store.account_id_by_name("newcomer"), None);
        assert_eq!(store.sign_in("old_hand", b"pw"), Ok(id));
        assert_eq!(
            store.sign_in("old_hand", b"wrong"),
            Err(SignInError::WrongPassword)
        );
        store.set_registrations_open(true);
        assert!(store.sign_in("newcomer", b"pw").is_ok());
    }

    #[test]
    fn maintenance_counts_minutes_up_and_ends_at_zero() {
        let store = SessionStore::new();
        assert_eq!(store.minutes_to_maintenance(), None);
        store.set_maintenance(Some(unix_now() + 61));
        assert_eq!(store.minutes_to_maintenance(), Some(2));
        store.set_maintenance(Some(unix_now() + 3600));
        assert_eq!(store.minutes_to_maintenance(), Some(60));
        store.set_maintenance(Some(unix_now().saturating_sub(5)));
        assert_eq!(store.minutes_to_maintenance(), Some(0));
        store.set_maintenance(None);
        assert_eq!(store.minutes_to_maintenance(), None);
    }

    #[test]
    fn an_address_registers_so_many_accounts_a_day() {
        let store = SessionStore::new();
        let now = Instant::now();
        let spammer = ip(203, 0, 113, 5);
        for n in 0..limits::DEFAULT_REGISTRATIONS {
            assert!(
                store
                    .sign_in_from(&format!("bot{n}"), b"pw", spammer, now)
                    .is_ok()
            );
        }
        assert_eq!(
            store.sign_in_from("bot_more", b"pw", spammer, now),
            Err(SignInError::RegistrationLimit)
        );
        assert_eq!(store.account_id_by_name("bot_more"), None);
        // Its accounts still sign in; others still register.
        assert!(store.sign_in_from("bot0", b"pw", spammer, now).is_ok());
        assert!(
            store
                .sign_in_from("friend", b"pw", ip(203, 0, 113, 6), now)
                .is_ok()
        );
        assert!(
            store
                .sign_in_from("local", b"pw", ip(127, 0, 0, 1), now)
                .is_ok()
        );
        // Names with control characters are not taken.
        let bad = format!("bad{}name", char::from(7));
        assert_eq!(
            store.sign_in_from(&bad, b"pw", ip(198, 51, 100, 1), now),
            Err(SignInError::InvalidUserName)
        );
    }

    #[test]
    fn a_guesser_locks_out_only_itself_and_an_address_asks_so_often() {
        let store = SessionStore::new();
        let now = Instant::now();
        let owner = ip(198, 51, 100, 7);
        let guesser = ip(203, 0, 113, 9);
        let id = store.sign_in_from("victim", b"right", owner, now).unwrap();
        for _ in 0..MAX_FAILED_SIGN_INS {
            assert_eq!(
                store.sign_in_from("victim", b"guess", guesser, now),
                Err(SignInError::WrongPassword)
            );
        }
        assert_eq!(
            store.sign_in_from("victim", b"right", guesser, now),
            Err(SignInError::TooManyAttempts)
        );
        assert_eq!(store.sign_in_from("victim", b"right", owner, now), Ok(id));
        // However many names it tries, an address gets so many checks.
        let mut refused = 0;
        for n in 0..limits::DEFAULT_ATTEMPTS + 5 {
            if store.sign_in_from(&format!("victim{n}"), b"x", ip(192, 0, 2, 1), now)
                == Err(SignInError::TooManyFromAddress)
            {
                refused += 1;
            }
        }
        assert_eq!(refused, 5);
    }

    #[test]
    fn a_ban_shuts_the_connections_of_the_account() {
        use std::io::Read;
        use std::net::TcpListener;
        let store = SessionStore::new();
        let id = store.sign_in("kicked", b"pw").unwrap();
        let session = store.create_session(id);
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let mut client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (server, _) = listener.accept().unwrap();
        let hold = store.hold(session).unwrap();
        hold.attach(&server);
        store.ban_account(id, forever());
        client
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        // The server's end was shut: the client reads the end of the stream.
        assert_eq!(client.read(&mut [0; 8]).unwrap(), 0);
        drop(hold);
    }

    #[test]
    fn an_address_ban_covers_its_range_and_kicks_who_signed_in_from_it() {
        let store = SessionStore::new();
        let id = store.sign_in("from_home", b"pw").unwrap();
        store.signed_in_from(id, ip(10, 1, 2, 3));
        let session = store.create_session(id);
        let other = store.sign_in("elsewhere", b"pw").unwrap();
        store.signed_in_from(other, ip(10, 2, 0, 1));
        let other_session = store.create_session(other);
        let net = Net::parse("10.1.0.0/16").unwrap();
        assert_eq!(store.ban_net(net, forever()), 1);
        assert!(store.has_ended(session) && !store.has_ended(other_session));
        assert!(store.ip_ban(ip(10, 1, 200, 7)).is_some());
        assert!(store.ip_ban(ip(10, 2, 0, 1)).is_none());
        // The account itself is not banned: from elsewhere it signs in.
        assert_eq!(store.sign_in("from_home", b"pw"), Ok(id));
        assert!(!store.unban_net(Net::parse("10.1.2.3").unwrap()));
        assert!(store.unban_net(net));
        assert!(store.ip_ban(ip(10, 1, 2, 3)).is_none());
    }

    #[test]
    fn the_admin_finds_an_account_by_user_name_or_by_a_shown_name_only_one_has() {
        let store = SessionStore::new();
        let a = store.sign_in("Alpha@one.test", b"pw").unwrap();
        store.sign_in("alpha@two.test", b"pw").unwrap();
        let b = store.sign_in("bravo@x.test", b"pw").unwrap();
        assert_eq!(store.find_account("ALPHA@one.test"), Some(a));
        assert_eq!(store.find_account("Bravo"), Some(b));
        // Two accounts show "alpha": the user name is needed.
        assert_eq!(store.find_account("alpha"), None);
        assert_eq!(store.find_account("nobody"), None);
    }

    #[test]
    fn a_name_shows_only_the_part_of_an_email_before_the_at() {
        let store = SessionStore::new();
        let name = |login: &str| {
            let id = store.get_or_create_account(ip(127, 0, 0, 1), login);
            let session = store.create_session(id);
            let account = store.account_for_session(session).unwrap();
            let name = account.locked().name.clone();
            (id, name)
        };
        assert_eq!(name("stepan@example.com").1, "stepan");
        assert_eq!(name("dummy0").1, "dummy0");
        let (id, fallback) = name("@example.com");
        assert_eq!(fallback, format!("player_{id:08x}"));
    }

    #[test]
    fn a_lock_poisoned_by_a_panic_still_opens() {
        let shared = Arc::new(Mutex::new(1));
        let holder = shared.clone();
        let _ = std::thread::spawn(move || {
            let _guard = holder.lock().unwrap();
            panic!("a server thread fails while holding the lock");
        })
        .join();
        assert!(shared.lock().is_err());
        assert_eq!(*shared.locked(), 1);
    }

    /// Queue a session alone and let its match form at once.
    fn seat(store: &SessionStore, session_id: u32) {
        let account_id = store.any_account_id(session_id).unwrap_or(session_id);
        let player = Player {
            session_id,
            account_id,
            profile_id: 1,
        };
        // A match needs a second player.
        let other = Player {
            session_id: session_id + 0x4000_0000,
            account_id: account_id + 0x4000_0000,
            profile_id: 1,
        };
        store
            .matchmaking()
            .queue(&[player, other], false, Instant::now());
        store.matchmaking().poll(session_id, later(3600));
    }

    #[test]
    fn match_results_wait_for_the_lobby_by_account() {
        let store = SessionStore::new();
        let id = store.get_or_create_account(ip(127, 0, 0, 1), "fighter");
        let session = store.create_session(id);
        let result = MatchResult {
            kills: 2,
            finished: true,
            ..MatchResult::default()
        };
        store.record_match_result(session, result.clone());
        store.record_match_result(0xFFFF_FFFF, result.clone());

        // Another sign-in of the same account gets them.
        let later = store.create_session(id);
        assert_eq!(store.take_match_results(later), [result]);
        assert!(store.take_match_results(session).is_empty());
    }

    fn signed_in(store: &SessionStore, email: &str) -> u32 {
        let account_id = store.get_or_create_account(ip(127, 0, 0, 1), email);
        store.create_session(account_id)
    }

    fn later(seconds: u64) -> Instant {
        Instant::now() + Duration::from_secs(seconds)
    }

    #[test]
    fn accounts_are_found_by_part_of_their_name() {
        let store = SessionStore::new();
        let ivan = store.get_or_create_account(ip(127, 0, 0, 1), "Ivanov@mail");
        let petr = store.get_or_create_account(ip(127, 0, 0, 1), "petrovich");
        let ivo = store.get_or_create_account(ip(127, 0, 0, 2), "ivo");
        assert_eq!(
            store.find_accounts(b"IV", 0, 10),
            [(ivan, "Ivanov".to_string()), (ivo, "ivo".to_string()),]
        );
        assert_eq!(
            store.find_accounts(b"iv", ivo, 1),
            [(ivan, "Ivanov".to_string())]
        );
        assert!(store.find_accounts(b"", 0, 10).is_empty());
        assert!(store.account(petr).is_some());
        assert!(store.account(0xBAD).is_none());
    }

    #[test]
    fn an_ended_session_is_refused_and_forgotten() {
        let store = SessionStore::new();
        let session = signed_in(&store, "leaver");
        assert!(store.touch(session));
        assert!(store.end_session(session));
        assert!(!store.end_session(session));
        assert!(!store.touch(session));
        assert!(store.hold(session).is_none());
        assert!(store.account_for_session(session).is_none());
        assert_eq!(store.session_count(), 0);
        assert!(!store.touch(0xFFFF_FFFF));
    }

    #[test]
    fn a_session_ends_after_the_idle_limit_and_not_before() {
        let store = SessionStore::new();
        let idle = signed_in(&store, "idle");
        let pinging = signed_in(&store, "pinging");
        assert!(
            store
                .end_idle_sessions(later(599), SESSION_IDLE_LIMIT)
                .is_empty()
        );

        // A ping counts from when it came.
        let limit = Duration::from_millis(50);
        std::thread::sleep(limit * 2);
        assert!(store.touch(pinging));
        assert_eq!(store.end_idle_sessions(Instant::now(), limit), [idle]);
        assert!(store.account_for_session(pinging).is_some());
        assert!(store.account_for_session(idle).is_none());
    }

    #[test]
    fn only_connected_players_are_online_each_once() {
        let store = SessionStore::new();
        let lobby = signed_in(&store, "lobby");
        let chat = store.create_session(store.account_for_session(lobby).unwrap().locked().id);
        let gone = signed_in(&store, "gone");
        assert_eq!(store.players_online(), 0);
        let _lobby = store.hold(lobby).unwrap();
        let _chat = store.hold(chat).unwrap();
        let gone_hold = store.hold(gone).unwrap();
        assert_eq!(store.players_online(), 2);
        // A client killed without signing out: its connections close.
        drop(gone_hold);
        assert_eq!(store.players_online(), 1);
        assert_eq!(store.session_count(), 3);
    }

    #[test]
    fn a_held_queued_or_playing_session_never_idles_out() {
        let store = SessionStore::new();
        let held = signed_in(&store, "lobby");
        let playing = signed_in(&store, "playing");
        let queued = signed_in(&store, "queued");
        let hold = store.hold(held).unwrap();
        seat(&store, playing);
        let player = Player {
            session_id: queued,
            account_id: 1,
            profile_id: 1,
        };
        store.matchmaking().queue(&[player], false, Instant::now());
        assert!(
            store
                .end_idle_sessions(later(3600), SESSION_IDLE_LIMIT)
                .is_empty()
        );

        // Its connection's end is its last sighting.
        drop(hold);
        assert!(
            store
                .end_idle_sessions(later(599), SESSION_IDLE_LIMIT)
                .is_empty()
        );
        assert_eq!(
            store.end_idle_sessions(later(601), SESSION_IDLE_LIMIT),
            [held]
        );
    }

    #[test]
    fn a_session_that_ends_in_a_match_still_gets_its_result() {
        let store = SessionStore::new();
        let session = signed_in(&store, "quitter");
        seat(&store, session);
        assert!(store.end_session(session));
        assert!(store.account_for_session(session).is_none());
        assert!(store.session_account(session).is_some());
        assert_eq!(store.session_count(), 1);

        // The match server records it; the match ends and lets it go.
        store.record_match_result(session, MatchResult::default());
        let (match_id, _) = store.matchmaking().seat(session).unwrap();
        store.matchmaking().end_match(match_id);
        store.seen_in_match(session);
        assert_eq!(store.session_count(), 0);
        let again = signed_in(&store, "quitter");
        assert_eq!(store.take_match_results(again), [MatchResult::default()]);
    }

    #[test]
    fn ending_a_session_takes_it_out_of_the_queue() {
        let store = SessionStore::new();
        let first = signed_in(&store, "first");
        signed_in(&store, "second");
        let player = Player {
            session_id: first,
            account_id: 1,
            profile_id: 1,
        };
        store.matchmaking().queue(&[player], false, Instant::now());
        let _hold = store.hold(first).unwrap();
        assert!(store.end_session(first));
        assert_eq!(store.matchmaking().poll(first, Instant::now()), None);
        // Still held by its connection, but no new one gets in.
        assert_eq!(store.session_count(), 2);
        assert!(store.hold(first).is_none());
    }

    /// Two accounts' sessions with a squad of them; the first commands.
    fn squad_of_two(store: &SessionStore) -> ((u32, u32), (u32, u32)) {
        let a = store.get_or_create_account(ip(127, 0, 0, 1), "alpha");
        let b = store.get_or_create_account(ip(127, 0, 0, 1), "bravo");
        let (sa, sb) = (store.create_session(a), store.create_session(b));
        store.squads().invite_accepted(a, b);
        store.squads().add(a, b).unwrap();
        ((a, sa), (b, sb))
    }

    #[test]
    fn a_new_sign_in_leaves_the_squad_of_the_game_before() {
        let store = SessionStore::new();
        let ((a, _), (b, _)) = squad_of_two(&store);
        // Alpha closed the game without a word and signs in again.
        store.create_session(a);
        assert_eq!(store.squads().of(a), None);
        assert_eq!(store.squads().of(b), None);
    }

    #[test]
    fn signing_out_leaves_the_squad() {
        let store = SessionStore::new();
        let ((a, sa), (b, _)) = squad_of_two(&store);
        assert!(store.end_session(sa));
        assert_eq!(store.squads().of(a), None);
        assert_eq!(store.squads().of(b), None);
    }

    #[test]
    fn a_game_gone_without_a_word_leaves_its_squad_and_the_queue() {
        let store = SessionStore::new();
        let ((a, sa), (b, sb)) = squad_of_two(&store);
        let c = store.get_or_create_account(ip(127, 0, 0, 1), "charlie");
        let sc = store.create_session(c);
        store.squads().invite_accepted(a, c);
        store.squads().add(a, c).unwrap();
        let players: Vec<Player> = [(sa, a), (sb, b), (sc, c)]
            .into_iter()
            .map(|(session_id, account_id)| Player {
                session_id,
                account_id,
                profile_id: 1,
            })
            .collect();
        store.matchmaking().queue(&players, true, Instant::now());
        // Bravo and Charlie play on; Alpha's game is closed.
        let _b = store.hold(sb).unwrap();
        let _c = store.hold(sc).unwrap();
        let soon = Instant::now() + GONE_AFTER / 2;
        assert!(store.drop_gone_players(soon).is_empty());
        let later = Instant::now() + GONE_AFTER + Duration::from_secs(1);
        assert_eq!(store.drop_gone_players(later), [sa]);
        assert_eq!(store.squads().of(b).unwrap().members, [b, c]);
        assert_eq!(store.squads().of(b).unwrap().commander(), b);
        // The squad is out of the queue with it.
        assert_eq!(store.matchmaking().poll(sb, later), None);
        // Once.
        assert!(store.drop_gone_players(later).is_empty());
    }

    #[test]
    fn a_game_that_is_there_keeps_its_squad() {
        let store = SessionStore::new();
        let ((a, sa), (_, sb)) = squad_of_two(&store);
        // A lobby or chat connection, or a presence ping, says it is there.
        let _hold = store.hold(sa).unwrap();
        std::thread::sleep(Duration::from_millis(10));
        assert!(store.touch(sb));
        let later = Instant::now() + GONE_AFTER - Duration::from_secs(1);
        assert!(store.drop_gone_players(later).is_empty());
        assert!(store.squads().of(a).is_some());
    }

    #[test]
    fn an_old_game_found_gone_leaves_the_new_ones_squad_alone() {
        let store = SessionStore::new();
        let ((a, sa), _) = squad_of_two(&store);
        // Alpha signs in again and forms a new squad with Charlie.
        let sa2 = store.create_session(a);
        let c = store.get_or_create_account(ip(127, 0, 0, 1), "charlie");
        let sc = store.create_session(c);
        store.squads().invite_accepted(a, c);
        store.squads().add(a, c).unwrap();
        let _hold = store.hold(sa2).unwrap();
        let _charlie = store.hold(sc).unwrap();
        let later = Instant::now() + GONE_AFTER + Duration::from_secs(1);
        assert!(store.drop_gone_players(later).contains(&sa));
        assert_eq!(store.squads().of(a).unwrap().members, [a, c]);
    }

    #[test]
    fn being_seen_in_a_match_counts_as_a_sighting() {
        let store = SessionStore::new();
        let session = signed_in(&store, "back");
        std::thread::sleep(Duration::from_millis(20));
        let before_leaving = Instant::now();
        store.seen_in_match(session);
        let idle = Duration::from_millis(10);
        assert!(
            store
                .end_idle_sessions(before_leaving + idle, idle)
                .is_empty()
        );
    }
}
