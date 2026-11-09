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
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use rusqlite::{Connection, OptionalExtension, params};

use super::account::{Account, Item, KIT_SLOTS, Kit, KitSlot, QuestInstance, Upgrade};
use super::clans::Clan;
use super::moderation::AdminAction;
use crate::LockExt;
use crate::persist::{DB_FILE, DB_VAR, PersistError};

mod admin_actions;
mod balance;
mod clans;
mod drop_cards;
mod extras;
pub mod history;
mod items;
mod level_stats;
mod map_plays;
pub mod match_events;
mod mode_intro;
mod presets;
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
    // 9: the audit trail of the admin's account moderation (`admin_actions`).
    admin_actions::MIGRATION,
    // 10: the matches an account used a drop card on (`drop_cards`).
    drop_cards::MIGRATION,
    // 11: an account's kit presets and the active one (`presets`).
    presets::MIGRATION,
    // 12: an account's kills and deaths by the matches' equipment level
    // (`level_stats`).
    level_stats::MIGRATION,
    // 13: per-mode onboarding allowances.
    mode_intro::MIGRATION,
    // 14: none. It was the gold-to-silver conversion of #1523, which never
    // shipped; that is the one-off pay-back at the release's first start now
    // ([`SqliteGameRepository::pay_back_once`]). The number stays.
    "",
    // 15: the match history and its totals (`history`).
    history::MIGRATION,
    // 16: each match's events (`match_events`).
    match_events::MIGRATION,
    // 17: whether an account's matches may be watched live (`extras`).
    extras::LIVE_WATCH_MIGRATION,
    // 18: each history line's roster place (`history`).
    history::SEAT_MIGRATION,
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
    /// The admin's change of an account and its rows of the audit trail
    /// ([`AdminAction`]), together: both are kept or neither. `account`:
    /// the account to write whole too, `None` when what is kept of it has
    /// the change already. (A repository that keeps no audit trail writes
    /// the account only.)
    fn save_admin(
        &mut self,
        account_id: u32,
        account: Option<&Account>,
        _actions: &[AdminAction],
    ) -> Result<(), PersistError> {
        match account {
            Some(account) => self.save(account_id, account),
            None => Ok(()),
        }
    }
    /// The account's latest `limit` rows of the audit trail, the newest
    /// first.
    fn admin_actions(
        &mut self,
        _account_id: u32,
        _limit: usize,
    ) -> Result<Vec<AdminAction>, PersistError> {
        Ok(Vec::new())
    }
    /// Keeps a finished match in the history ([`history`]). (A repository
    /// that keeps no history: nothing.)
    fn record_match(&mut self, _record: &history::MatchRecord) -> Result<(), PersistError> {
        Ok(())
    }
    /// A reader of the history for the site, on a connection of its own.
    /// (A repository that keeps no history: `None`.)
    fn history(&self) -> Result<Option<history::MatchHistory>, PersistError> {
        Ok(None)
    }
}

/// Accounts kept in memory. Clones share one set, so a test can keep one
/// and give a store another.
#[derive(Clone, Default)]
pub struct MemoryGameRepository {
    inner: Arc<Mutex<(HashMap<u32, Account>, usize)>>,
    clans: Arc<Mutex<HashMap<u32, Clan>>>,
    results: Arc<Mutex<std::collections::BTreeMap<String, String>>>,
    admin_actions: Arc<Mutex<Vec<AdminAction>>>,
    /// Account writes and audit rows fail while it is on: a database
    /// refusing them.
    failing: Arc<Mutex<bool>>,
    /// Account writes fail while it is on, audit rows alone do not.
    failing_accounts: Arc<Mutex<bool>>,
    /// Account writes wait while it is held ([`Self::hold_saves`]).
    gate: Arc<Mutex<()>>,
    /// Account writes waiting at the gate now.
    held: Arc<std::sync::atomic::AtomicUsize>,
}

impl MemoryGameRepository {
    /// How many accounts were written so far.
    pub fn saves(&self) -> usize {
        self.inner.locked().1
    }

    pub fn get(&self, account_id: u32) -> Option<Account> {
        self.inner.locked().0.get(&account_id).cloned()
    }

    /// Make every account write and audit row fail (with `BadRow`), or
    /// work again: a database that refuses them, for tests.
    pub fn fail_saves(&self, failing: bool) {
        *self.failing.locked() = failing;
    }

    /// Make every account write fail, while audit rows written alone
    /// still work: a database that refuses the account only, for tests.
    pub fn fail_account_saves(&self, failing: bool) {
        *self.failing_accounts.locked() = failing;
    }

    /// Account writes wait until the guard is dropped: a saver busy with a
    /// long pass, for tests.
    pub fn hold_saves(&self) -> std::sync::MutexGuard<'_, ()> {
        self.gate.locked()
    }

    /// How many account writes wait at [`Self::hold_saves`] now.
    pub fn saves_held(&self) -> usize {
        self.held.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Every row of the audit trail written so far, the oldest first.
    pub fn all_admin_actions(&self) -> Vec<AdminAction> {
        self.admin_actions.locked().clone()
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
        {
            use std::sync::atomic::Ordering::SeqCst;
            self.held.fetch_add(1, SeqCst);
            let _gate = self.gate.locked();
            self.held.fetch_sub(1, SeqCst);
        }
        if *self.failing.locked() || *self.failing_accounts.locked() {
            return Err(PersistError::BadRow(account_id, "disk full".into()));
        }
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

    fn save_admin(
        &mut self,
        account_id: u32,
        account: Option<&Account>,
        actions: &[AdminAction],
    ) -> Result<(), PersistError> {
        // Both or neither, as one transaction.
        if *self.failing.locked() || (account.is_some() && *self.failing_accounts.locked()) {
            return Err(PersistError::BadRow(account_id, "disk full".into()));
        }
        if let Some(account) = account {
            self.save(account_id, account)?;
        }
        self.admin_actions.locked().extend_from_slice(actions);
        Ok(())
    }

    fn admin_actions(
        &mut self,
        account_id: u32,
        limit: usize,
    ) -> Result<Vec<AdminAction>, PersistError> {
        Ok(self
            .admin_actions
            .locked()
            .iter()
            .rev()
            .filter(|action| action.account_id == account_id)
            .take(limit)
            .cloned()
            .collect())
    }
}

/// 0.26g accounts in srp's SQLite file.
pub struct SqliteGameRepository {
    connection: Connection,
    path: PathBuf,
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
            path: path.to_owned(),
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
        Self::open_in(&crate::persist::data_dir()).map(Some)
    }

    /// The schema version of the 0.26g tables.
    pub fn version(&self) -> Result<i64, PersistError> {
        schema_version(&self.connection)
    }

    /// The one-off pay-back of the release that removed gold and the
    /// vouchers but the drop card (the owner, 2026-11-08: "we should have
    /// release script which will give players silver back for gold they
    /// had", "remove them completely and for players just give back them in
    /// silver"), once in the database's life ([`PAID_BACK`]), at the first
    /// start of that release: after srp's own copy of srp.db, which a
    /// rollback puts back, and before the accounts are loaded. In one
    /// transaction with its mark:
    ///
    /// - a gold balance is silver at 100 a gold
    ///   (`survarium::v026g::prices::GOLD_IN_SILVER`) and the gold column 0,
    ///   which nothing reads or writes after it (kept in the schema, unused:
    ///   the owner's of 2026-11-08);
    /// - each repair kit, rename voucher and one-day premium voucher held
    ///   ([`PAID_BACK_VOUCHERS`]) is its shop price in silver, and is taken
    ///   out of the inventory, the kits and the presets.
    ///
    /// A balance stops at the most silver holds. Returns what each account
    /// was given; `None` when it was done before.
    /// `scripts/silver-only/preview.py` shows it for a copy of the file.
    ///
    /// Before it changes anything it copies the file to
    /// `<backups>/srp-<unix>-before-pay-back.db`; without that copy it does
    /// nothing and fails, so srp does not start and a rollback finds the
    /// file as it was (a failed backup at the start only warns).
    pub fn pay_back_once(&mut self, backups: &Path) -> Result<Option<Vec<PaidBack>>, PersistError> {
        if self.paid_back()? {
            return Ok(None);
        }
        std::fs::create_dir_all(backups).map_err(|error| PersistError::Io(backups.to_path_buf(), error))?;
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let copy = backups.join(format!("srp-{stamp}-before-pay-back.db"));
        self.connection.execute("VACUUM INTO ?1", [copy.to_string_lossy()])?;
        log::info!("Copied the database to {} before the pay-back", copy.display());
        let transaction = self.connection.transaction()?;
        let held = |dict_id: u16| {
            format!(
                "COALESCE((SELECT SUM(condition_or_stack) FROM g26_items i \
                 WHERE i.account_id = a.account_id AND i.dict_id = {dict_id}), 0)"
            )
        };
        let [kit, rename, premium] = PAID_BACK_VOUCHERS.map(|(dict_id, _)| held(dict_id));
        let mut paid = Vec::new();
        {
            let mut rows = transaction.prepare(&format!(
                "SELECT a.account_id, a.name, a.premium_money, {kit}, {rename}, {premium}
                 FROM g26_accounts a ORDER BY a.account_id"
            ))?;
            let found = rows.query_map([], |row| {
                Ok(PaidBack {
                    account_id: row.get(0)?,
                    name: row.get(1)?,
                    gold: row.get(2)?,
                    vouchers: [row.get(3)?, row.get(4)?, row.get(5)?],
                })
            })?;
            for account in found {
                let account = account?;
                if account.gold != 0 || account.vouchers != [0; 3] {
                    paid.push(account);
                }
            }
        }
        for account in &paid {
            transaction.execute(
                "UPDATE g26_accounts SET generic_money = MIN(generic_money + ?2, 4294967295),
                    premium_money = 0
                 WHERE account_id = ?1",
                params![account.account_id, account.silver().min(i64::MAX as u64) as i64],
            )?;
        }
        let ids = PAID_BACK_VOUCHERS.map(|(dict_id, _)| dict_id.to_string()).join(", ");
        transaction.execute_batch(&format!(
            "DELETE FROM g26_kit_slots WHERE EXISTS (
                SELECT 1 FROM g26_items i WHERE i.account_id = g26_kit_slots.account_id
                    AND i.item_id = g26_kit_slots.item_id AND i.dict_id IN ({ids}));
             DELETE FROM g26_preset_slots WHERE EXISTS (
                SELECT 1 FROM g26_items i WHERE i.account_id = g26_preset_slots.account_id
                    AND i.item_id = g26_preset_slots.item_id AND i.dict_id IN ({ids}));
             DELETE FROM g26_items WHERE dict_id IN ({ids});"
        ))?;
        transaction.execute(
            "INSERT INTO g26_meta (key, value) VALUES (?1, 1)",
            [PAID_BACK],
        )?;
        transaction.commit()?;
        Ok(Some(paid))
    }

    /// Whether [`Self::pay_back_once`] was done ([`PAID_BACK`]).
    fn paid_back(&self) -> Result<bool, PersistError> {
        Ok(self.connection.query_row(
            "SELECT EXISTS (SELECT 1 FROM g26_meta WHERE key = ?1)",
            [PAID_BACK],
            |row| row.get(0),
        )?)
    }
}

/// The vouchers [`SqliteGameRepository::pay_back_once`] takes away, by dict
/// id, with the silver each is worth: its shop price, its gold price times
/// 100 (`scripts/content-edit/items.tsv`). The repair kit, the rename
/// voucher, the one-day premium voucher.
pub const PAID_BACK_VOUCHERS: [(u16, u64); 3] = [(151, 5_000), (152, 25_000), (213, 5_000)];

/// The `g26_meta` key that says the gold and the removed vouchers were paid
/// back in silver ([`SqliteGameRepository::pay_back_once`]).
pub const PAID_BACK: &str = "silver_only_paid_back";

/// What [`SqliteGameRepository::pay_back_once`] found of an account.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaidBack {
    pub account_id: u32,
    pub name: String,
    /// Its gold balance.
    pub gold: u64,
    /// How many of each of [`PAID_BACK_VOUCHERS`] it held.
    pub vouchers: [u64; 3],
}

impl PaidBack {
    /// The silver its balance gains (before the balance's cap).
    pub fn silver(&self) -> u64 {
        let vouchers: u64 = self
            .vouchers
            .iter()
            .zip(PAID_BACK_VOUCHERS)
            .map(|(&count, (_, each))| count.saturating_mul(each))
            .sum();
        self.gold.saturating_mul(100).saturating_add(vouchers)
    }
}

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
                "SELECT account_id, name, clan_tag, generic_money,
                        exp_total, quest_faction, rating, rating_before,
                        active_kit, autobuy, next_item_id, next_quest_instance
                 FROM g26_accounts ORDER BY account_id",
            )?;
            let rows = statement.query_map([], |row| {
                Ok((
                    row.get::<_, u32>(0)?,
                    Account {
                        name: row.get(1)?,
                        clan_tag: row.get(2)?,
                        generic_money: row.get(3)?,
                        exp_total: row.get(4)?,
                        skills: Vec::new(),
                        reputations: Vec::new(),
                        quest_faction: row.get(5)?,
                        rating: row.get(6)?,
                        rating_before: row.get(7)?,
                        balance: Default::default(),
                        level_stats: Default::default(),
                        kits: Vec::new(),
                        active_kit: row.get(8)?,
                        autobuy: row.get(9)?,
                        inventory: Vec::new(),
                        next_item_id: row.get(10)?,
                        quests: Vec::new(),
                        quest_cooldowns: Vec::new(),
                        next_quest_instance: row.get(11)?,
                        extras: Default::default(),
                        map_plays: Vec::new(),
                        room_settings: None,
                        mode_introductions: Default::default(),
                        drop_cards: Vec::new(),
                        presets: [None; super::presets::PRESETS],
                        active_preset: super::presets::FIRST_PRESET,
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
            account.level_stats = level_stats::load(&transaction, account_id)?;
            account.room_settings = room_settings::load(&transaction, account_id)?;
            account.mode_introductions = mode_intro::load(&transaction, account_id)?;
            account.drop_cards = drop_cards::load(&transaction, account_id)?;
            account.presets = presets::load(&transaction, account_id)?;
            account.active_preset = presets::load_active(&transaction, account_id)?;
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
                    // repair_time and reroll_time are not read: srp has no
                    // repair, and a modification change no wait.
                    "SELECT item_id, dict_id, condition_or_stack,
                            modifications, upgrades, reroll
                     FROM g26_items WHERE account_id = ?1 ORDER BY seq",
                )?;
                let rows = statement.query_map([account_id], |row| {
                    Ok((
                        Item {
                            item_id: row.get(0)?,
                            dict_id: row.get(1)?,
                            condition_or_stack: row.get(2)?,
                            modifications: Vec::new(),
                            upgrades: Vec::new(),
                            reroll: Default::default(),
                        },
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
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
        write_account(&transaction, account_id, account)?;
        transaction.commit()?;
        Ok(())
    }

    fn describe(&self) -> String {
        self.path.display().to_string()
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

    fn save_admin(
        &mut self,
        account_id: u32,
        account: Option<&Account>,
        actions: &[AdminAction],
    ) -> Result<(), PersistError> {
        let transaction = self.connection.transaction()?;
        if let Some(account) = account {
            write_account(&transaction, account_id, account)?;
        }
        for action in actions {
            admin_actions::save(&transaction, action)?;
        }
        transaction.commit()?;
        Ok(())
    }

    fn admin_actions(
        &mut self,
        account_id: u32,
        limit: usize,
    ) -> Result<Vec<AdminAction>, PersistError> {
        admin_actions::load(&self.connection, account_id, limit)
    }

    fn record_match(&mut self, record: &history::MatchRecord) -> Result<(), PersistError> {
        history::record(&mut self.connection, record)
    }

    fn history(&self) -> Result<Option<history::MatchHistory>, PersistError> {
        history::MatchHistory::open(&self.path).map(Some)
    }
}

/// Write one account whole, replacing what was kept of it, in the
/// caller's transaction.
fn write_account(
    transaction: &Connection,
    account_id: u32,
    account: &Account,
) -> Result<(), PersistError> {
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
            generic_money = excluded.generic_money, premium_until = excluded.premium_until,
            exp_total = excluded.exp_total,
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
            // The gold column and the premium account's end: kept in the
            // schema and unused (gold removed 2026-11-08, premium
            // 2026-11-09); a new row needs a value, and a saved account's
            // old premium end is cleared.
            0,
            0,
            account.exp_total,
            account.quest_faction,
            account.rating,
            account.rating_before,
            account.active_kit,
            account.autobuy,
            account.next_item_id,
            account.next_quest_instance,
            // The tutorial's columns: kept in the schema, unused; no
            // tutorial (the owner, 2026-10-19), so every account is done.
            0,
            true,
        ],
    )?;
    extras::save(transaction, account_id, &account.extras)?;
    map_plays::save(transaction, account_id, &account.map_plays)?;
    balance::save(transaction, account_id, account.balance)?;
    level_stats::save(transaction, account_id, &account.level_stats)?;
    room_settings::save(transaction, account_id, account.room_settings.as_ref())?;
    mode_intro::save(transaction, account_id, &account.mode_introductions)?;
    drop_cards::save(transaction, account_id, &account.drop_cards)?;
    presets::save(transaction, account_id, &account.presets)?;
    presets::save_active(transaction, account_id, account.active_preset)?;
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
                // repair_time and reroll_time: no repair, no wait.
                0,
                0,
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
    Ok(())
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
        account.add_experience(40_000);
        account.add_reputation(2, 500);
        account.autobuy = true;
        rules::set_skills(&mut account, &[(101, 2), (201, 1)]).unwrap();
        rules::rename_kit(&mut account, 2, "Sniper").unwrap();
        rules::set_active_kit(&mut account, 2, 1_000).unwrap();
        quests::choose_faction(&mut account, 2).unwrap();
        quests::refresh(&mut account, 1_000, &mut Rng::new(4));
        account.quest_cooldowns.push((10101, 5_000));
        account.inventory[0].condition_or_stack = 654_321;
        account.inventory[0].modifications = vec![(1050, true), (7, false)];
        account.inventory[0].upgrades = vec![Upgrade {
            id: 5,
            a: 6,
            value: u64::MAX,
            param: 9,
        }];
        // What the later steps added: all of it must come back too.
        account.rating = 137;
        account.rating_before = 125;
        account.map_plays = vec![(1, 4), (6, 2)];
        account.balance = crate::v026g::balance::Balance {
            rating: -1250,
            matches: 9,
        };
        account.extras.bonus.day = 3;
        account.extras.bonus.last_day = 20_373;
        account.extras.bonus.wins = [3, 3, 1, 0, 0, 0, 0];
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

    /// Reliability no longer rolls (the owner's rule of 2026-10-23,
    /// docs/notes/no-reliability-roll.md), but items that have it keep it:
    /// saved, loaded and checked as the server's start does, nothing of
    /// them changes.
    #[test]
    fn items_with_reliability_come_back_with_it() {
        use survarium::v026g::dictionary::Dictionary;
        use survarium::v026g::lobby::FULL_CONDITION;

        let temp = TempDb::new("reliability");
        let dict = |name| Dictionary::shared().item_named(name).unwrap().dict_id;
        let mut account = Account::new("Alice");
        let mut give = |name, modifications: &[u32]| {
            let item_id = account.add_item(dict(name), FULL_CONDITION);
            account.item_mut(item_id).unwrap().modifications =
                modifications.iter().map(|&id| (id, false)).collect();
            item_id
        };
        let rifle = give("ak_74m", &[2093, 2000]);
        let backpack = give("scav_back_1", &[1062, 1120, 1204]);
        let mut repository = SqliteGameRepository::open(&temp.path()).unwrap();
        repository.save(7, &account).unwrap();
        let mut loaded = repository.load_all().unwrap();
        assert_eq!(loaded, [(7, account.clone())]);
        let loaded = &mut loaded[0].1;
        assert!(!loaded.repair(), "nothing to mend");
        assert_eq!(
            loaded.item(rifle).unwrap().modifications,
            [(2093, false), (2000, false)]
        );
        assert_eq!(
            loaded.item(backpack).unwrap().modifications,
            [(1062, false), (1120, false), (1204, false)]
        );
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

    /// The pay-back: gold is silver at 100 a gold, each removed voucher is
    /// its price in silver and is gone; the drop card and the rest stay; a
    /// balance stops at its most; a second start pays nothing.
    #[test]
    fn gold_and_the_removed_vouchers_are_paid_back_once() {
        let temp = TempDb::new("pay-back");
        let mut repository = SqliteGameRepository::open(&temp.path()).unwrap();
        repository
            .connection
            .execute_batch(
                "INSERT INTO g26_accounts (
                    account_id, name, clan_tag, generic_money, premium_money,
                    premium_until, exp_total, quest_faction, rating, rating_before,
                    active_kit, autobuy, next_item_id, next_quest_instance,
                    tutorial_step, tutorial_done
                 ) VALUES
                 (5, 'Carol', '', 1000, 150, 0, 900, 2, 118, 100, 1, 0, 9, 1, 0, 1),
                 (6, 'Dave', '', 4294967000, 10, 0, 900, 2, 118, 100, 1, 0, 9, 1, 0, 1),
                 (7, 'Erin', '', 70, 0, 0, 900, 2, 118, 100, 1, 0, 9, 1, 0, 1);
                 INSERT INTO g26_items (
                    account_id, item_id, seq, dict_id, condition_or_stack,
                    repair_time, reroll_time, modifications, upgrades
                 ) VALUES
                 (5, 1, 0, 151, 3, 0, 0, '', ''),
                 (5, 2, 1, 152, 2, 0, 0, '', ''),
                 (5, 3, 2, 213, 1, 0, 0, '', ''),
                 (5, 4, 3, 153, 4, 0, 0, '', ''),
                 (6, 1, 0, 152, 1, 0, 0, '', ''),
                 (7, 1, 0, 153, 1, 0, 0, '', '');",
            )
            .unwrap();
        let backups = temp.path().with_extension("backups");
        let paid = repository.pay_back_once(&backups).unwrap().unwrap();
        // The copy made before it, with the gold still there.
        let copies: Vec<_> = std::fs::read_dir(&backups).unwrap().collect();
        assert_eq!(copies.len(), 1);
        let silver: Vec<_> = paid.iter().map(|account| (account.account_id, account.silver())).collect();
        // Carol: 150 gold, 3 kits and a premium day at 5,000, 2 rename
        // vouchers at 25,000. Dave: 10 gold and a rename voucher. Erin: none.
        assert_eq!(silver, [(5, 15_000 + 4 * 5_000 + 2 * 25_000), (6, 1_000 + 25_000)]);
        assert_eq!((paid[0].gold, paid[0].vouchers), (150, [3, 2, 1]));
        let state = |repository: &mut SqliteGameRepository| {
            repository
                .load_all()
                .unwrap()
                .into_iter()
                .map(|(id, account)| {
                    let owned: Vec<_> = account
                        .inventory
                        .iter()
                        .map(|item| (item.dict_id, item.condition_or_stack))
                        .collect();
                    (id, account.generic_money, owned)
                })
                .collect::<Vec<_>>()
        };
        let expected = vec![
            (5, 1000 + 85_000, vec![(153, 4)]),
            (6, u32::MAX, vec![]),
            (7, 70, vec![(153, 1)]),
        ];
        assert_eq!(state(&mut repository), expected);
        let gold: i64 = repository
            .connection
            .query_row("SELECT SUM(premium_money) FROM g26_accounts", [], |row| row.get(0))
            .unwrap();
        assert_eq!(gold, 0);
        // Saving an account leaves the gold column alone.
        let (_, carol) = repository.load_all().unwrap().remove(0);
        repository.save(5, &carol).unwrap();
        drop(repository);
        // A second start: nothing is paid twice.
        let mut repository = SqliteGameRepository::open(&temp.path()).unwrap();
        assert_eq!(repository.pay_back_once(&backups).unwrap(), None);
        assert_eq!(state(&mut repository), expected);
        std::fs::remove_dir_all(&backups).unwrap();
    }

    /// Without its copy the pay-back changes nothing and fails: srp does not
    /// start, and the file is as it was for a rollback.
    #[test]
    fn no_pay_back_without_its_copy() {
        let temp = TempDb::new("pay-back-no-copy");
        let mut repository = SqliteGameRepository::open(&temp.path()).unwrap();
        repository
            .connection
            .execute_batch(
                "INSERT INTO g26_accounts (
                    account_id, name, clan_tag, generic_money, premium_money,
                    premium_until, exp_total, quest_faction, rating, rating_before,
                    active_kit, autobuy, next_item_id, next_quest_instance,
                    tutorial_step, tutorial_done
                 ) VALUES (5, 'Carol', '', 1000, 150, 0, 900, 2, 118, 100, 1, 0, 9, 1, 0, 1);",
            )
            .unwrap();
        // A file where the backups folder should be: no copy can be made.
        let blocked = temp.path().with_extension("blocked");
        std::fs::write(&blocked, b"").unwrap();
        assert!(repository.pay_back_once(&blocked).is_err());
        let (gold, silver): (i64, i64) = repository
            .connection
            .query_row("SELECT premium_money, generic_money FROM g26_accounts", [], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .unwrap();
        assert_eq!((gold, silver), (150, 1000));
        assert!(!repository.paid_back().unwrap());
        std::fs::remove_file(&blocked).unwrap();
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

    /// The starter grant is a new account's only: one signed up, played
    /// and written comes back from the file as it was, and its next sign-in
    /// gives it nothing again.
    #[test]
    fn an_account_loaded_from_the_file_is_not_given_the_starter_kit_again() {
        use crate::v026g::GameAccounts;
        let grant = crate::v026g::tutorial::new_account();

        let temp = TempDb::new("starter-once");
        let open = || {
            let repository = SqliteGameRepository::open(&temp.path()).unwrap();
            GameAccounts::with_repository(Box::new(repository)).unwrap()
        };
        let store = open();
        let (account, made) = store.sign_up(5, "Alice");
        assert!(made);
        // It spends silver and rounds, and its respirator is gone.
        let played = {
            let mut account = account.locked();
            assert_eq!(account.generic_money, grant.silver);
            account.generic_money = 1_234;
            let stack = account
                .inventory
                .iter_mut()
                .find(|item| item.condition_or_stack == grant.rounds)
                .unwrap();
            stack.condition_or_stack = 7;
            let respirator = account.kits[0].slots[1].unwrap().item_id;
            account.inventory.retain(|item| item.item_id != respirator);
            account.sync_kits();
            account.clone()
        };
        store.mark_dirty(5);
        assert_eq!(store.flush(), 1);
        drop(store);

        let store = open();
        let loaded = store.get(5).unwrap().locked().clone();
        assert_eq!(loaded, played);
        let (again, made) = store.sign_up(5, "Alice");
        assert!(!made);
        assert_eq!(*again.locked(), played);
        assert!(!store.is_dirty(5));
    }
}
