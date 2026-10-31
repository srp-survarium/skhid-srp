//! The 0.26g account model (`docs/notes/lobby.md`,
//! `lobby_requests.md`): what a 0.26g lobby keeps of an account, the rules
//! of the client's actions, the pushes it is sent, and where it is kept.
//!
//! It stands beside the 0.21d model ([`crate::Account::game`]) and changes
//! nothing of it: a 0.26g account is found by the same account id the
//! [`crate::SessionStore`] hands out at sign-in, in a store of its own,
//! [`GameAccounts`].
//!
//! - [`account`]: the state ([`Account`]): money, experience and level,
//!   skills, reputations, three kits, the inventory, quests.
//! - [`rules`]: the lobby's actions (move, buy, sell, repair, exchange,
//!   skills, kits), each refusing with an [`ActionError`] that names the
//!   game's own text for it.
//! - [`quests`]: choosing a quest faction, being given and paid quests.
//! - [`wire`]: the account as lobby messages, and the sign-in's pushes in
//!   the order the client needs.
//! - [`persist`]: the repositories (SQLite tables `g26_*` in `srp.db`,
//!   memory).
//! - [`matches_now`]: which of the matches being played the launcher's page
//!   lists, and which score it tells.
//! - [`replays`]: "Watch replay": the conversions waiting, each player's
//!   latest ask, and the download tokens the lobby gives and the web page
//!   takes.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, TryLockError};
use std::time::{Duration, Instant};

pub mod account;
pub mod autobuy;
pub mod balance;
pub mod bonus;
pub mod bot_kits;
pub mod bots;
pub mod clans;
pub mod extras;
pub mod items_rules;
pub mod known;
pub mod level_stats;
pub mod matches_now;
pub mod matchmaking;
pub mod moderation;
pub mod names;
pub mod notices;
pub mod pay;
pub mod persist;
pub mod premium;
pub mod presets;
pub mod pvb;
pub mod quests;
pub mod replays;
pub mod rewards;
pub mod room_settings;
pub mod rooms;
pub mod rules;
pub mod tutorial;
pub mod wire;

pub use account::{Account, Item, Kit, KitSlot, QuestInstance};
pub use persist::{GameRepository, MemoryGameRepository, SqliteGameRepository};
pub use rules::ActionError;

use crate::persist::PersistError;
use crate::{LockExt, RESET_WRITE_WAIT, ResetWrite};
use moderation::AdminAction;

struct Writer {
    repository: Box<dyn GameRepository>,
    /// What was last written of each account: one equal to it is skipped.
    saved: HashMap<u32, Account>,
}

/// Something else kept in the accounts' repository, written at every
/// [`GameAccounts::flush`] after the accounts
/// ([`GameAccounts::on_flush`]).
pub type FlushHook = Box<dyn Fn(&mut dyn GameRepository) + Send + Sync>;

/// Bonuses srp gives the items owned once in a database's life, when a
/// rule makes a config default a modification of the item's own
/// ([`GameAccounts::grant_once`]).
struct OnceGrant {
    /// The `g26_meta` key written once it is done.
    mark: &'static str,
    /// For the log: the rule, the items, what they get.
    title: &'static str,
    items: &'static str,
    bonus: &'static str,
    give: fn(&mut Account) -> items_rules::GoldenGrant,
}

const GOLDEN_SILVER: OnceGrant = OnceGrant {
    mark: persist::GOLDEN_SILVER_GIVEN,
    title: "Golden weapons",
    items: "golden weapons",
    bonus: "the silver bonus",
    give: items_rules::give_golden_silver,
};

const PREMIUM_ARMOUR_BONUSES: OnceGrant = OnceGrant {
    mark: persist::PREMIUM_ARMOUR_GIVEN,
    title: "Premium armour",
    items: "premium armour pieces",
    bonus: "the premium bonuses",
    give: items_rules::give_premium_armour_bonuses,
};

/// One line of [`GameAccounts::board`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoardRow {
    pub name: String,
    /// Empty for a player of no clan.
    pub clan_tag: String,
    pub rating: u16,
    pub level: u8,
}

/// [`GameAccounts::board`]: the best players and the battles fought.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Board {
    pub top: Vec<BoardRow>,
    /// How many times an account was put in a match, all accounts together
    /// (a match of eight players counts eight).
    pub battles: u64,
}

/// The 0.26g accounts of a running srp, by account id. Shared as an
/// `Arc<GameAccounts>`; each account is handed out as `Arc<Mutex<Account>>`
/// so a change in the lobby is seen wherever the account is held. Whoever
/// changes one marks it with [`GameAccounts::mark_dirty`]; the saver
/// ([`GameAccounts::spawn_saver`]) or [`GameAccounts::flush`] writes it.
pub struct GameAccounts {
    accounts: Mutex<HashMap<u32, Arc<Mutex<Account>>>>,
    dirty: Mutex<HashSet<u32>>,
    wake: Condvar,
    /// `None`: kept in memory only.
    writer: Option<Mutex<Writer>>,
    /// The clans, kept by the same repository.
    clans: clans::Clans,
    /// The accounts that are test bots, by name in lower case
    /// ([`bots`], `SRP_BOT_ACCOUNTS`): none unless the operator set it.
    bots: Mutex<HashSet<String>>,
    /// What the repository had of the finished matches when it was opened
    /// ([`GameRepository::load_results`]), until
    /// [`rewards::Results::attached`] takes it.
    restored_results: Mutex<Vec<(String, String)>>,
    flush_hooks: Mutex<Vec<FlushHook>>,
    /// The admin's audit rows not written yet ([`Self::write_admin`]).
    pending_actions: Mutex<Vec<AdminAction>>,
    /// A new account wears the starter kits and has the starter premium
    /// ([`tutorial::signup_grant_with`]); off only in tests ([`Self::set_starter_kit`]).
    starter_kit: AtomicBool,
}

impl Default for GameAccounts {
    fn default() -> Self {
        Self::new()
    }
}

impl GameAccounts {
    /// A store kept in memory only.
    pub fn new() -> Self {
        Self {
            accounts: Mutex::new(HashMap::new()),
            dirty: Mutex::new(HashSet::new()),
            wake: Condvar::new(),
            writer: None,
            clans: clans::Clans::new(),
            bots: Mutex::new(HashSet::new()),
            restored_results: Mutex::new(Vec::new()),
            flush_hooks: Mutex::new(Vec::new()),
            pending_actions: Mutex::new(Vec::new()),
            starter_kit: AtomicBool::new(true),
        }
    }

    /// A store that keeps its accounts in `repository`, with what the
    /// repository holds loaded and made consistent ([`Account::repair`]; an
    /// account that needed it is written again).
    ///
    /// First every account and kept results screen is checked against the
    /// game's data ([`known::Lacking`]): an item, a modification or a quest
    /// the data lacks is an error ([`PersistError::GameData`], naming each)
    /// and nothing is changed, so that data made without a patch never
    /// costs a player an item.
    pub fn with_repository(mut repository: Box<dyn GameRepository>) -> Result<Self, PersistError> {
        let loaded = repository.load_all()?;
        let clans = repository.load_clans()?;
        let results = repository.load_results()?;
        let mut lacking = known::Lacking::default();
        for (account_id, account) in &loaded {
            lacking.account(*account_id, account);
        }
        for report in rewards::kept_reports(&results) {
            lacking.report(&report);
        }
        if !lacking.is_empty() {
            return Err(PersistError::GameData(lacking.to_string()));
        }
        log::info!("Loaded {} clans", clans.len());
        log::info!(
            "Loaded {} 0.26g accounts from {}",
            loaded.len(),
            repository.describe()
        );
        let mut accounts = HashMap::new();
        let mut saved = HashMap::new();
        let mut dirty = HashSet::new();
        for (account_id, mut account) in loaded {
            let kept = account.clone();
            // Kits saved before the covered-slots rule: said once, apart.
            let uncovered = account.uncover_kits();
            if uncovered > 0 {
                log::warn!(
                    "0.26g account {account_id}: {uncovered} items were worn in slots another \
                     worn item covers; taken off to the inventory"
                );
            }
            // Kits saved before the rule of one pocket an item type.
            let merged = account.merge_pockets();
            if merged > 0 {
                log::warn!(
                    "0.26g account {account_id}: {merged} pockets held an item type another \
                     pocket of their kit held; merged into that pocket"
                );
            }
            if account.repair() || uncovered > 0 || merged > 0 {
                log::warn!("0.26g account {account_id} was not consistent; repaired");
                for (host, record, why) in items_rules::upgrades_taken_off(&kept, &account) {
                    log::warn!(
                        "0.26g account {account_id}: upgrade {} (dict id {}, slot {}) taken off \
                         item {host}: {why}",
                        record.id,
                        record.a,
                        record.param
                    );
                }
                dirty.insert(account_id);
            }
            saved.insert(account_id, kept);
            accounts.insert(account_id, Arc::new(Mutex::new(account)));
        }
        Ok(Self {
            accounts: Mutex::new(accounts),
            dirty: Mutex::new(dirty),
            wake: Condvar::new(),
            writer: Some(Mutex::new(Writer { repository, saved })),
            clans: clans::Clans::loaded(clans),
            bots: Mutex::new(HashSet::new()),
            restored_results: Mutex::new(results),
            flush_hooks: Mutex::new(Vec::new()),
            pending_actions: Mutex::new(Vec::new()),
            starter_kit: AtomicBool::new(true),
        })
    }

    /// The rows the repository had for [`rewards::Results`] when it was
    /// opened, once: empty for a store kept in memory, and at every call
    /// after the first.
    pub fn take_restored_results(&self) -> Vec<(String, String)> {
        std::mem::take(&mut *self.restored_results.locked())
    }

    /// `hook` is called at every [`Self::flush`] with the repository, after
    /// the accounts are written and with no account locked: for what is
    /// kept beside them. Never called for a store kept in memory.
    pub fn on_flush(&self, hook: FlushHook) {
        self.flush_hooks.locked().push(hook);
    }

    /// The store srp's environment asks for: in `srp.db` under
    /// `SRP_DATA_DIR`, or in memory with `SRP_DB=off`.
    ///
    /// The first time a database is opened with wear off
    /// ([`pay::item_wear`], the default) every weapon and armour piece in
    /// it is made whole ([`Self::mend_gear`]): what matches wore before the
    /// switch would stay worn for ever. Once in the database's life
    /// (`persist::GEAR_MENDED`), so an item found later, which arrives worn
    /// ([`pay::drop_condition`]), stays as found until its owner repairs it.
    ///
    /// The first time a database is opened by an srp with the golden-weapons
    /// rule, every golden weapon in it gets the silver bonus as its own
    /// first modification ([`Self::give_golden_silver_once`]); the first
    /// time with the premium-armour rule, every premium armour piece its
    /// premium bonuses ([`Self::give_premium_armour_bonuses_once`]); the
    /// first time with autobuy on by default, every account's autobuy is
    /// turned on ([`Self::turn_autobuy_on_once`]).
    pub fn from_env() -> Result<Self, PersistError> {
        match SqliteGameRepository::from_env()? {
            Some(mut repository) => {
                let mend = !pay::item_wear() && repository.first_time(persist::GEAR_MENDED)?;
                let store = Self::with_repository(Box::new(repository))?;
                store.give_golden_silver_once()?;
                store.give_premium_armour_bonuses_once()?;
                store.turn_autobuy_on_once()?;
                if mend {
                    let (items, accounts) = store.mend_gear();
                    store.flush();
                    log::info!(
                        "Wear is off: {items} worn weapons and armour pieces of {accounts} \
                         accounts made whole, once"
                    );
                }
                Ok(store)
            }
            None => Ok(Self::new()),
        }
    }

    /// [`Self::give_golden_silver`] once in the database's life
    /// (`persist::GOLDEN_SILVER_GIVEN`; [`Self::grant_once`]): until the
    /// rule the silver bonus was a default of the item's config, which the
    /// golden-weapons patch takes off; after it, a silver bonus rerolled
    /// away stays away. Returns `(items, accounts)` given; `None` when it
    /// was done before, or for a store kept in memory.
    pub fn give_golden_silver_once(&self) -> Result<Option<(usize, usize)>, PersistError> {
        self.grant_once(&GOLDEN_SILVER)
    }

    /// Gives every account's golden weapons the silver bonus as their own
    /// first modification ([`items_rules::give_golden_silver`]) and marks
    /// the accounts it changed. Returns the items given and the accounts
    /// changed. A golden weapon that already has three modifications of
    /// its own keeps them, and is logged.
    pub fn give_golden_silver(&self) -> (usize, Vec<u32>) {
        self.grant(&GOLDEN_SILVER)
    }

    /// [`Self::give_premium_armour_bonuses`] once in the database's life
    /// (`persist::PREMIUM_ARMOUR_GIVEN`; [`Self::grant_once`]): until the
    /// rule the premium bonuses were defaults of the item's config, which
    /// the premium-armour patch takes off; after it, a bonus rerolled away
    /// stays away. Returns `(items, accounts)` given; `None` when it was
    /// done before, or for a store kept in memory.
    pub fn give_premium_armour_bonuses_once(&self) -> Result<Option<(usize, usize)>, PersistError> {
        self.grant_once(&PREMIUM_ARMOUR_BONUSES)
    }

    /// Gives every account's premium armour the premium bonuses it lacks as
    /// its own first modifications
    /// ([`items_rules::give_premium_armour_bonuses`]) and marks the
    /// accounts it changed. Returns the items given and the accounts
    /// changed. A piece with no place left for a bonus keeps its own, and
    /// is logged.
    pub fn give_premium_armour_bonuses(&self) -> (usize, Vec<u32>) {
        self.grant(&PREMIUM_ARMOUR_BONUSES)
    }

    /// Every account's autobuy turned on, once in the database's life
    /// (`persist::AUTOBUY_TURNED_ON`; the owner's of 2026-10-23: on by
    /// default). An account keeps only the box's state, not whether its
    /// player ever touched it, so each account of before gets it on once; a
    /// box turned off later stays off. A new account starts with it on
    /// ([`Account::with_loadout`]). A mark, not a schema step: an older srp
    /// still opens the database (a rollback keeps it). The accounts changed
    /// are written first and the mark after them, as [`Self::grant_once`]
    /// does: an account not written stops the start, and the next start
    /// turns on those still off. Returns the accounts turned on; `None` when
    /// it was done before, or for a store kept in memory.
    pub fn turn_autobuy_on_once(&self) -> Result<Option<usize>, PersistError> {
        let Some(writer) = &self.writer else {
            return Ok(None);
        };
        let done = writer
            .locked()
            .repository
            .is_done(persist::AUTOBUY_TURNED_ON)?;
        if done {
            return Ok(None);
        }
        let accounts: Vec<(u32, Arc<Mutex<Account>>)> = self
            .accounts
            .locked()
            .iter()
            .map(|(&id, account)| (id, Arc::clone(account)))
            .collect();
        let mut changed = Vec::new();
        for (account_id, account) in accounts {
            let turned = {
                let mut account = account.locked();
                !std::mem::replace(&mut account.autobuy, true)
            };
            if turned {
                changed.push(account_id);
                self.mark_dirty(account_id);
            }
        }
        self.flush();
        let unsaved: Vec<u32> = changed
            .iter()
            .copied()
            .filter(|&account_id| self.is_dirty(account_id))
            .collect();
        if !unsaved.is_empty() {
            return Err(PersistError::NotWritten(format!(
                "autobuy turned on at start was not written for accounts {unsaved:?}; srp does \
                 not start, and the next start turns it on for the accounts still without it"
            )));
        }
        writer
            .locked()
            .repository
            .mark_done(persist::AUTOBUY_TURNED_ON)?;
        log::info!("Autobuy turned on for {} accounts, once", changed.len());
        Ok(Some(changed.len()))
    }

    /// `grant` once in the database's life. The accounts it changed are
    /// written first and the mark (`grant.mark` in `g26_meta`) after them.
    /// A crash between leaves it unmarked; so does an account not written,
    /// and then srp does not start ([`PersistError::NotWritten`]): running
    /// unmarked, a bonus rerolled away could come back at a later start.
    /// Either way the next start gives it again to the items still without
    /// it (those written have it, and are left alone).
    fn grant_once(&self, grant: &OnceGrant) -> Result<Option<(usize, usize)>, PersistError> {
        let Some(writer) = &self.writer else {
            return Ok(None);
        };
        let done = writer.locked().repository.is_done(grant.mark)?;
        if done {
            return Ok(None);
        }
        let (items, accounts) = self.grant(grant);
        self.flush();
        let unsaved: Vec<u32> = accounts
            .iter()
            .copied()
            .filter(|&account_id| self.is_dirty(account_id))
            .collect();
        if !unsaved.is_empty() {
            return Err(PersistError::NotWritten(format!(
                "{}: {} given at start was not written for accounts {unsaved:?}; srp does not \
                 start, and the next start gives it again to the {} still without it",
                grant.items, grant.bonus, grant.items
            )));
        }
        writer.locked().repository.mark_done(grant.mark)?;
        log::info!(
            "{}: {items} of {} accounts got {} as their own first modifications, once",
            grant.title,
            accounts.len(),
            grant.bonus
        );
        Ok(Some((items, accounts.len())))
    }

    /// Gives every account's items `grant` and marks the accounts it
    /// changed; logs each account changed and each item with no place left.
    fn grant(&self, grant: &OnceGrant) -> (usize, Vec<u32>) {
        let accounts: Vec<(u32, Arc<Mutex<Account>>)> = self
            .accounts
            .locked()
            .iter()
            .map(|(&id, account)| (id, Arc::clone(account)))
            .collect();
        let (mut items, mut changed) = (0, Vec::new());
        for (account_id, account) in accounts {
            let done = {
                let mut account = account.locked();
                let done = (grant.give)(&mut account);
                if !done.given.is_empty() {
                    log::info!(
                        "{}: {} (account {account_id}) got {}: items {:?}",
                        grant.title,
                        account.name,
                        grant.bonus,
                        done.given
                    );
                }
                if !done.full.is_empty() {
                    log::warn!(
                        "{}: {} (account {account_id}) has {} with three modifications of their \
                         own; they keep them, without {}: items {:?}",
                        grant.title,
                        account.name,
                        grant.items,
                        grant.bonus,
                        done.full
                    );
                }
                done
            };
            if !done.given.is_empty() {
                items += done.given.len();
                changed.push(account_id);
                self.mark_dirty(account_id);
            }
        }
        (items, changed)
    }

    /// Makes every account's weapons and armour whole
    /// ([`items_rules::mend_all`]) and marks the accounts it changed.
    /// Returns `(items, accounts)` mended.
    pub fn mend_gear(&self) -> (usize, usize) {
        let accounts: Vec<(u32, Arc<Mutex<Account>>)> = self
            .accounts
            .locked()
            .iter()
            .map(|(&id, account)| (id, Arc::clone(account)))
            .collect();
        let (mut items, mut changed) = (0, 0);
        for (account_id, account) in accounts {
            let mended = items_rules::mend_all(&mut account.locked());
            if mended > 0 {
                items += mended;
                changed += 1;
                self.mark_dirty(account_id);
            }
        }
        (items, changed)
    }

    /// The clans and clan teams.
    pub fn clans(&self) -> &clans::Clans {
        &self.clans
    }

    /// The account with this id, if it has 0.26g state.
    pub fn get(&self, account_id: u32) -> Option<Arc<Mutex<Account>>> {
        self.accounts.locked().get(&account_id).cloned()
    }

    /// The account with this id; made with the game's starting state
    /// ([`Account::new`]) under `name` if it has none (a sign-in makes it
    /// with [`Self::sign_up`]).
    pub fn get_or_create(&self, account_id: u32, name: &str) -> Arc<Mutex<Account>> {
        let mut accounts = self.accounts.locked();
        if let Some(account) = accounts.get(&account_id) {
            return account.clone();
        }
        let account = Arc::new(Mutex::new(Account::new(name)));
        accounts.insert(account_id, account.clone());
        drop(accounts);
        self.mark_dirty(account_id);
        account
    }

    /// The account with this id; at its first 0.26g sign-in it is made, as
    /// [`Self::get_or_create`] makes it, and given what the tutorial it no
    /// longer has paid ([`tutorial::signup_grant`]). Whether it was made
    /// now.
    pub fn sign_up(&self, account_id: u32, name: &str) -> (Arc<Mutex<Account>>, bool) {
        let made = !self.accounts.locked().contains_key(&account_id);
        let account = self.get_or_create(account_id, name);
        if made {
            let wear_kit = self.starter_kit.load(Ordering::Relaxed);
            tutorial::signup_grant_with(&mut account.locked(), wear_kit, quests::now());
        }
        (account, made)
    }

    /// **Testing only**: whether a new account wears the starter kits and
    /// has the starter premium (on by default). Off, its first kit is the
    /// stock one (the TT-33), it has no premium, and the rest of the grant
    /// is the same: the lobby's tests, written against such an account.
    #[cfg(any(test, feature = "test-support"))]
    pub fn set_starter_kit(&self, on: bool) {
        self.starter_kit.store(on, Ordering::Relaxed);
    }

    /// How many accounts have 0.26g state.
    pub fn len(&self) -> usize {
        self.accounts.locked().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// **Testing only**: the accounts with these names are test bots
    /// ([`bots`]; `SRP_BOT_ACCOUNTS`). An empty list, the default, turns
    /// everything about bots off.
    pub fn set_bots(&self, names: &[String]) {
        *self.bots.locked() = names
            .iter()
            .map(|name| name.trim().to_ascii_lowercase())
            .filter(|name| !name.is_empty())
            .collect();
    }

    /// The names set with [`Self::set_bots`], sorted.
    pub fn bot_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.bots.locked().iter().cloned().collect();
        names.sort();
        names
    }

    /// Whether an account of this name is a test bot. Locks no account.
    pub fn is_bot_name(&self, name: &str) -> bool {
        let bots = self.bots.locked();
        !bots.is_empty() && bots.contains(&name.to_ascii_lowercase())
    }

    /// Whether the account is a test bot. It locks the account: call it
    /// with that account not locked.
    pub fn is_bot(&self, account_id: u32) -> bool {
        if self.bots.locked().is_empty() {
            return false;
        }
        let Some(account) = self.get(account_id) else {
            return false;
        };
        let name = account.locked().name.clone();
        self.is_bot_name(&name)
    }

    /// `(account id, name, rating, rating before)` of every account, best
    /// rating first (ties by id): the ranking. Test bots ([`bots`]) are
    /// not in it.
    pub fn ranking(&self) -> Vec<(u32, String, u16, u16)> {
        let accounts: Vec<(u32, Arc<Mutex<Account>>)> = self
            .accounts
            .locked()
            .iter()
            .map(|(&id, account)| (id, account.clone()))
            .collect();
        let mut rows: Vec<(u32, String, u16, u16)> = accounts
            .into_iter()
            .map(|(id, account)| {
                let account = account.locked();
                (
                    id,
                    account.name.clone(),
                    account.rating,
                    account.rating_before,
                )
            })
            .collect();
        {
            let bots = self.bots.locked();
            if !bots.is_empty() {
                rows.retain(|row| !bots.contains(&row.1.to_ascii_lowercase()));
            }
        }
        rows.sort_by(|a, b| b.2.cmp(&a.2).then(a.0.cmp(&b.0)));
        rows
    }

    /// What the launcher's page shows of the accounts: the `count` best by
    /// rating among those that were put in a match at least once (ties by
    /// id, as in [`Self::ranking`]), and how many times an account was put
    /// in a match, all accounts together. Test bots ([`bots`]) are not
    /// counted.
    pub fn board(&self, count: usize) -> Board {
        let accounts: Vec<(u32, Arc<Mutex<Account>>)> = self
            .accounts
            .locked()
            .iter()
            .map(|(&id, account)| (id, account.clone()))
            .collect();
        let bots = self.bots.locked().clone();
        let mut battles = 0_u64;
        let mut rows: Vec<(u32, BoardRow)> = Vec::new();
        for (id, account) in accounts {
            let account = account.locked();
            if bots.contains(&account.name.to_ascii_lowercase()) {
                continue;
            }
            let played: u64 = account
                .map_plays
                .iter()
                .map(|&(_, plays)| u64::from(plays))
                .sum();
            if played == 0 {
                continue;
            }
            battles += played;
            rows.push((
                id,
                BoardRow {
                    name: account.name.clone(),
                    clan_tag: account.clan_tag.clone(),
                    rating: account.rating,
                    level: account.level(),
                },
            ));
        }
        rows.sort_by(|a, b| b.1.rating.cmp(&a.1.rating).then(a.0.cmp(&b.0)));
        rows.truncate(count);
        Board {
            top: rows.into_iter().map(|(_, row)| row).collect(),
            battles,
        }
    }

    /// The server's bots' kits of every level from the accounts' kits and
    /// presets ([`bot_kits`]); test bots ([`bots`]) are not counted. Locks
    /// each account in turn: call it with none locked.
    pub fn bot_kits(&self, now: u64) -> bot_kits::BotKits {
        let accounts: Vec<(u32, Arc<Mutex<Account>>)> = self
            .accounts
            .locked()
            .iter()
            .map(|(&id, account)| (id, account.clone()))
            .collect();
        let bots = self.bots.locked().clone();
        let mut gathering = bot_kits::Gathering::default();
        for (id, account) in accounts {
            let account = account.locked();
            if !bots.contains(&account.name.to_ascii_lowercase()) {
                gathering.add(id, &account, now);
            }
        }
        gathering.finish()
    }

    /// The account changed: it is written on the next pass. Never waits on
    /// an account, so it may be called with one locked.
    pub fn mark_dirty(&self, account_id: u32) {
        if self.writer.is_some() {
            self.dirty.locked().insert(account_id);
            self.wake.notify_one();
        }
    }

    /// Whether the account is marked and not written yet.
    pub fn is_dirty(&self, account_id: u32) -> bool {
        self.dirty.locked().contains(&account_id)
    }

    /// Write every account marked since the last pass, now. Each is copied
    /// out under its own lock and written with none held; one equal to what
    /// was last written is skipped, and one that fails stays marked. Call
    /// it with no account locked. Returns how many were written.
    pub fn flush(&self) -> usize {
        let Some(writer) = &self.writer else {
            return 0;
        };
        let mut writer = writer.locked();
        // The clans that changed, disbanded ones first (an id is not used
        // twice, but a write that failed is tried again in any order).
        let (changed, deleted) = self.clans.take_changes();
        for clan_id in deleted {
            if let Err(error) = writer.repository.delete_clan(clan_id) {
                log::error!("Clan {clan_id} not deleted: {error}; retrying");
                self.clans.mark_unsaved(clan_id, true);
            }
        }
        for clan in changed {
            if let Err(error) = writer.repository.save_clan(&clan) {
                log::error!("Clan {} not saved: {error}; retrying", clan.id);
                self.clans.mark_unsaved(clan.id, false);
            }
        }
        let mut dirty: Vec<u32> = std::mem::take(&mut *self.dirty.locked())
            .into_iter()
            .collect();
        dirty.sort_unstable();
        // The accounts the admin changed first, each with its audit rows in
        // one write: a row is never kept without the change it tells of.
        let mut written = 0;
        let (_, admin_failed) = self.write_actions(&mut writer, &mut written);
        for account_id in dirty {
            // Retried with its rows on the next pass, never without them.
            if admin_failed.contains(&account_id) {
                continue;
            }
            let Some(account) = self.get(account_id) else {
                continue;
            };
            let copy = account.locked().clone();
            if writer.saved.get(&account_id) == Some(&copy) {
                continue;
            }
            match writer.repository.save(account_id, &copy) {
                Ok(()) => {
                    writer.saved.insert(account_id, copy);
                    written += 1;
                }
                Err(error) => {
                    log::error!("0.26g account {account_id} not saved: {error}; retrying");
                    self.dirty.locked().insert(account_id);
                }
            }
        }
        // After the accounts: a match's report is never kept ahead of what
        // the match gave them.
        for hook in self.flush_hooks.locked().iter() {
            hook(writer.repository.as_mut());
        }
        written
    }

    /// Start the saver: every [`crate::SAVE_INTERVAL`], or sooner when an
    /// account is marked, it writes the marked accounts. `None` for a store
    /// kept in memory.
    pub fn spawn_saver(self: &Arc<Self>) -> Option<std::thread::JoinHandle<()>> {
        self.writer.as_ref()?;
        let store = self.clone();
        let saver = std::thread::Builder::new()
            .name("saver-026g".to_owned())
            .spawn(move || {
                loop {
                    {
                        let dirty = store.dirty.locked();
                        let _ = store.wake.wait_timeout(dirty, crate::SAVE_INTERVAL);
                    }
                    let saved = store.flush();
                    if saved > 0 {
                        log::debug!("Saved {saved} 0.26g accounts");
                    }
                }
            })
            .expect("failed to spawn the 0.26g saver thread");
        Some(saver)
    }

    /// The writer, waited for [`RESET_WRITE_WAIT`] at most: the saver holds
    /// it for its whole pass, and an admin's request should not hang on a
    /// long one. `None`: still busy.
    fn writer_soon(writer: &Mutex<Writer>) -> Option<MutexGuard<'_, Writer>> {
        let deadline = Instant::now() + RESET_WRITE_WAIT;
        loop {
            match writer.try_lock() {
                Ok(writer) => return Some(writer),
                Err(TryLockError::Poisoned(poisoned)) => return Some(poisoned.into_inner()),
                Err(TryLockError::WouldBlock) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(TryLockError::WouldBlock) => return None,
            }
        }
    }

    /// The admin changed the account (`moderation`, the admin page): it is
    /// written now, on the caller's thread, with `action` added to the
    /// audit trail in the same write (both are kept or neither); this
    /// account and the audit rows waiting, nothing else (not the saver's
    /// whole pass). Call it with no account locked.
    ///
    /// Past [`RESET_WRITE_WAIT`] for the writer the write is left to the
    /// saver, woken ([`ResetWrite::Pending`]); what the database refuses
    /// stays for the saver to retry, the account and its rows together, and
    /// the first error is returned. In every case the change stands in
    /// memory. A store kept in memory keeps no audit rows: the log has them.
    pub fn write_admin(&self, account_id: u32, action: AdminAction) -> ResetWrite {
        let Some(writer) = &self.writer else {
            return ResetWrite::Written;
        };
        self.pending_actions.locked().push(action);
        self.mark_dirty(account_id);
        let Some(mut writer) = Self::writer_soon(writer) else {
            self.wake.notify_one();
            return ResetWrite::Pending;
        };
        match self.write_actions(&mut writer, &mut 0).0 {
            Ok(()) => ResetWrite::Written,
            Err(error) => ResetWrite::Failed(error),
        }
    }

    /// Write the audit rows waiting, each account's with the account as it
    /// is now, in one write ([`GameRepository::save_admin`]): the rows
    /// alone when what was written of it is the account now (it has the
    /// change: a row waits only for a change made already). The rows of an
    /// account that fails stay, the account marked, for the next write.
    /// Returns the first error and the accounts that failed; `written`
    /// counts the accounts written.
    fn write_actions(
        &self,
        writer: &mut Writer,
        written: &mut usize,
    ) -> (Result<(), PersistError>, HashSet<u32>) {
        let waiting = std::mem::take(&mut *self.pending_actions.locked());
        let mut accounts: Vec<u32> = Vec::new();
        for action in &waiting {
            if !accounts.contains(&action.account_id) {
                accounts.push(action.account_id);
            }
        }
        let mut result = Ok(());
        let mut failed = HashSet::new();
        for account_id in accounts {
            let rows: Vec<AdminAction> = waiting
                .iter()
                .filter(|action| action.account_id == account_id)
                .cloned()
                .collect();
            let copy = self.get(account_id).map(|account| account.locked().clone());
            let write = copy
                .as_ref()
                .is_some_and(|copy| writer.saved.get(&account_id) != Some(copy));
            let account = if write { copy.as_ref() } else { None };
            match writer.repository.save_admin(account_id, account, &rows) {
                Ok(()) => {
                    if write && let Some(copy) = copy {
                        writer.saved.insert(account_id, copy);
                        *written += 1;
                    }
                }
                Err(error) => {
                    log::error!(
                        "Admin change of 0.26g account {account_id} not saved: {error}; retrying"
                    );
                    // Marked again (the saver may have taken the mark
                    // meanwhile): it retries, with the rows.
                    self.dirty.locked().insert(account_id);
                    failed.insert(account_id);
                    if result.is_ok() {
                        result = Err(error);
                    }
                }
            }
        }
        if !failed.is_empty() {
            let mut kept: Vec<AdminAction> = waiting
                .into_iter()
                .filter(|action| failed.contains(&action.account_id))
                .collect();
            let mut pending = self.pending_actions.locked();
            kept.append(&mut pending);
            *pending = kept;
        }
        (result, failed)
    }

    /// The account's latest `limit` rows of the admin's audit trail, the
    /// newest first, those not written yet included. `wait`: wait for a
    /// busy saver for [`RESET_WRITE_WAIT`] at most; else not at all (a
    /// request that waited for [`Self::write_admin`] already). `Err` when
    /// the database was busy or refused; empty for a store kept in memory.
    pub fn admin_actions(
        &self,
        account_id: u32,
        limit: usize,
        wait: bool,
    ) -> Result<Vec<AdminAction>, String> {
        let Some(writer) = &self.writer else {
            return Ok(Vec::new());
        };
        let writer = if wait {
            Self::writer_soon(writer)
        } else {
            match writer.try_lock() {
                Ok(writer) => Some(writer),
                Err(TryLockError::Poisoned(poisoned)) => Some(poisoned.into_inner()),
                Err(TryLockError::WouldBlock) => None,
            }
        };
        let Some(mut writer) = writer else {
            return Err("the database is busy; shown next time".to_owned());
        };
        // Under the writer: a row is either waiting or written, never both.
        let mut actions: Vec<AdminAction> = self
            .pending_actions
            .locked()
            .iter()
            .rev()
            .filter(|action| action.account_id == account_id)
            .take(limit)
            .cloned()
            .collect();
        let written = writer
            .repository
            .admin_actions(account_id, limit)
            .map_err(|error| error.to_string())?;
        actions.extend(written);
        actions.truncate(limit);
        Ok(actions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_first_sign_in_makes_the_account() {
        let store = GameAccounts::new();
        assert!(store.is_empty());
        assert!(store.get(5).is_none());
        let account = store.get_or_create(5, "Alice");
        assert_eq!(account.locked().name, "Alice");
        // The same account from then on, whatever name is given.
        let again = store.get_or_create(5, "Other");
        assert!(Arc::ptr_eq(&account, &again));
        assert_eq!(store.len(), 1);
        // In memory only: nothing is marked or written.
        store.mark_dirty(5);
        assert!(!store.is_dirty(5));
        assert_eq!(store.flush(), 0);
        assert!(Arc::new(store).spawn_saver().is_none());
    }

    #[test]
    fn autobuy_is_turned_on_once_for_every_account() {
        // A new account starts with it on.
        assert!(Account::new("Carol").autobuy);
        let repository = MemoryGameRepository::default();
        let store = GameAccounts::with_repository(Box::new(repository.clone())).unwrap();
        // Alice and Bob as the store had them: Alice off, Bob on.
        store.get_or_create(5, "Alice").locked().autobuy = false;
        store.get_or_create(6, "Bob");
        store.flush();
        let autobuy = |repository: &MemoryGameRepository, account_id| {
            repository.get(account_id).unwrap().autobuy
        };
        let mut marks = repository.clone();

        // The write fails: not marked done, and srp does not start.
        repository.fail_account_saves(true);
        let error = store.turn_autobuy_on_once().unwrap_err();
        assert!(matches!(error, PersistError::NotWritten(_)), "{error}");
        assert!(error.to_string().contains("accounts [5]"), "{error}");
        assert!(!autobuy(&repository, 5));
        assert!(!marks.is_done(persist::AUTOBUY_TURNED_ON).unwrap());

        // The next start turns it on, writes it, then marks it done.
        repository.fail_account_saves(false);
        let store = GameAccounts::with_repository(Box::new(repository.clone())).unwrap();
        assert_eq!(store.turn_autobuy_on_once().unwrap(), Some(1));
        assert!(autobuy(&repository, 5) && autobuy(&repository, 6));
        assert!(marks.is_done(persist::AUTOBUY_TURNED_ON).unwrap());

        // Once: turned off after, it stays off at later starts.
        store.get(5).unwrap().locked().autobuy = false;
        store.mark_dirty(5);
        store.flush();
        let store = GameAccounts::with_repository(Box::new(repository.clone())).unwrap();
        assert_eq!(store.turn_autobuy_on_once().unwrap(), None);
        assert!(!autobuy(&repository, 5));
        assert!(!store.get(5).unwrap().locked().autobuy);
        // A store kept in memory has no marks and nothing to do once.
        assert_eq!(GameAccounts::new().turn_autobuy_on_once().unwrap(), None);
    }

    #[test]
    fn the_golden_weapons_of_every_account_get_the_silver_bonus_once() {
        use survarium::v026g::dictionary::Dictionary;
        use survarium::v026g::golden::SILVER_BONUS;
        use survarium::v026g::lobby::FULL_CONDITION;
        let dict = |name: &str| Dictionary::shared().item_named(name).unwrap().dict_id;
        let repository = MemoryGameRepository::default();
        let store = GameAccounts::with_repository(Box::new(repository.clone())).unwrap();
        // Alice's golden rifle as the store had it before the rule: the
        // silver bonus was the config's, not the item's.
        let rifle = {
            let account = store.get_or_create(5, "Alice");
            let mut account = account.locked();
            let rifle = account.add_item(dict("ak_74n_premium"), FULL_CONDITION);
            account.item_mut(rifle).unwrap().modifications.clear();
            rifle
        };
        store.get_or_create(6, "Bob");
        store.flush();
        let kept = |repository: &MemoryGameRepository| {
            repository
                .get(5)
                .unwrap()
                .item(rifle)
                .unwrap()
                .modifications
                .clone()
        };
        let mut marks = repository.clone();

        // The write fails: not marked done, and srp does not start.
        repository.fail_account_saves(true);
        let error = store.give_golden_silver_once().unwrap_err();
        assert!(matches!(error, PersistError::NotWritten(_)), "{error}");
        assert!(error.to_string().contains("accounts [5]"), "{error}");
        assert!(store.is_dirty(5) && !store.is_dirty(6));
        assert!(kept(&repository).is_empty());
        assert!(!marks.is_done(persist::GOLDEN_SILVER_GIVEN).unwrap());

        // The next start gives it again, writes it, then marks it done.
        repository.fail_account_saves(false);
        let store = GameAccounts::with_repository(Box::new(repository.clone())).unwrap();
        assert_eq!(store.give_golden_silver_once().unwrap(), Some((1, 1)));
        assert_eq!(kept(&repository), [(SILVER_BONUS, false)]);
        assert!(marks.is_done(persist::GOLDEN_SILVER_GIVEN).unwrap());

        // Once: the silver bonus rerolled away stays away at later starts.
        store
            .get(5)
            .unwrap()
            .locked()
            .item_mut(rifle)
            .unwrap()
            .modifications = vec![(2000, false)];
        store.mark_dirty(5);
        store.flush();
        let store = GameAccounts::with_repository(Box::new(repository.clone())).unwrap();
        assert_eq!(store.give_golden_silver_once().unwrap(), None);
        assert_eq!(kept(&repository), [(2000, false)]);
        // A store kept in memory has no marks and nothing to give once.
        assert_eq!(GameAccounts::new().give_golden_silver_once().unwrap(), None);
    }

    #[test]
    fn the_premium_armour_of_every_account_gets_its_bonuses_once() {
        use survarium::v026g::dictionary::Dictionary;
        use survarium::v026g::golden::{BLACK_MARKET_REPUTATION_BONUS, EXPERIENCE_BONUS};
        use survarium::v026g::lobby::FULL_CONDITION;
        let dict = |name: &str| Dictionary::shared().item_named(name).unwrap().dict_id;
        let repository = MemoryGameRepository::default();
        // The golden weapons' grant was done before: it is not done again.
        repository
            .clone()
            .mark_done(persist::GOLDEN_SILVER_GIVEN)
            .unwrap();
        let store = GameAccounts::with_repository(Box::new(repository.clone())).unwrap();
        // Alice's Bandit jacket and golden rifle as the store had them
        // before the rule: the bonuses were the config's, the silver bonus
        // rerolled away.
        let (jacket, rifle) = {
            let account = store.get_or_create(5, "Alice");
            let mut account = account.locked();
            let jacket = account.add_item(dict("bm_torso_4_premium"), FULL_CONDITION);
            account.item_mut(jacket).unwrap().modifications.clear();
            let rifle = account.add_item(dict("ak_74n_premium"), FULL_CONDITION);
            account.item_mut(rifle).unwrap().modifications = vec![(2000, false)];
            (jacket, rifle)
        };
        store.get_or_create(6, "Bob");
        store.flush();
        let kept = |repository: &MemoryGameRepository, item_id| {
            repository
                .get(5)
                .unwrap()
                .item(item_id)
                .unwrap()
                .modifications
                .clone()
        };
        let mut marks = repository.clone();

        // The write fails: not marked done, and srp does not start.
        repository.fail_account_saves(true);
        let error = store.give_premium_armour_bonuses_once().unwrap_err();
        assert!(matches!(error, PersistError::NotWritten(_)), "{error}");
        assert!(error.to_string().contains("accounts [5]"), "{error}");
        assert!(kept(&repository, jacket).is_empty());
        assert!(!marks.is_done(persist::PREMIUM_ARMOUR_GIVEN).unwrap());

        // The next start gives them again, writes them, then marks it done;
        // the silver grant, done before, gives nothing.
        repository.fail_account_saves(false);
        let store = GameAccounts::with_repository(Box::new(repository.clone())).unwrap();
        assert_eq!(store.give_golden_silver_once().unwrap(), None);
        assert_eq!(
            store.give_premium_armour_bonuses_once().unwrap(),
            Some((1, 1))
        );
        assert_eq!(
            kept(&repository, jacket),
            [
                (EXPERIENCE_BONUS, false),
                (BLACK_MARKET_REPUTATION_BONUS, false)
            ]
        );
        assert_eq!(kept(&repository, rifle), [(2000, false)]);
        assert!(marks.is_done(persist::PREMIUM_ARMOUR_GIVEN).unwrap());

        // Once: a bonus rerolled away stays away at later starts.
        store
            .get(5)
            .unwrap()
            .locked()
            .item_mut(jacket)
            .unwrap()
            .modifications = vec![(EXPERIENCE_BONUS, false), (1090, false)];
        store.mark_dirty(5);
        store.flush();
        let store = GameAccounts::with_repository(Box::new(repository.clone())).unwrap();
        assert_eq!(store.give_premium_armour_bonuses_once().unwrap(), None);
        assert_eq!(
            kept(&repository, jacket),
            [(EXPERIENCE_BONUS, false), (1090, false)]
        );
        assert_eq!(
            GameAccounts::new()
                .give_premium_armour_bonuses_once()
                .unwrap(),
            None
        );
    }

    #[test]
    fn mending_makes_weapons_and_armour_whole_and_leaves_the_stacks() {
        use survarium::v026g::dictionary::Dictionary;
        use survarium::v026g::lobby::FULL_CONDITION;
        let dict = |name: &str| Dictionary::shared().item_named(name).unwrap().dict_id;
        let repository = MemoryGameRepository::default();
        let store = GameAccounts::with_repository(Box::new(repository.clone())).unwrap();
        let (rifle, vest, whole, rounds) = {
            let account = store.get_or_create(5, "Alice");
            let mut account = account.locked();
            // A rifle found in a match: 40 % and a defect beside its own
            // modification; a vest a match wore, in repair.
            let rifle = account.add_item(dict("ak_74m"), FULL_CONDITION * 2 / 5);
            account.item_mut(rifle).unwrap().modifications = vec![(2110, false), (3060, true)];
            let vest = account.add_item(dict("scav_torso_1"), FULL_CONDITION - 10_000);
            account.item_mut(vest).unwrap().repair_time = 2_000_000_000;
            let whole = account.add_item(dict("toz_34"), FULL_CONDITION);
            let rounds = account.add_item(dict("ammo_5.45x39_7n6m"), 90);
            (rifle, vest, whole, rounds)
        };
        store.get_or_create(6, "Bob");
        store.flush();
        // Two items of one account; Bob's starting kit needs nothing.
        assert_eq!(store.mend_gear(), (2, 1));
        assert!(store.is_dirty(5) && !store.is_dirty(6));
        {
            let account = store.get(5).unwrap();
            let account = account.locked();
            let rifle = account.item(rifle).unwrap();
            assert_eq!(rifle.condition_or_stack, FULL_CONDITION);
            assert_eq!(rifle.modifications, [(2110, false)]);
            let vest = account.item(vest).unwrap();
            assert_eq!(
                (vest.condition_or_stack, vest.repair_time),
                (FULL_CONDITION, 0)
            );
            assert_eq!(
                account.item(whole).unwrap().condition_or_stack,
                FULL_CONDITION
            );
            assert_eq!(account.item(rounds).unwrap().condition_or_stack, 90);
        }
        // Nothing left to mend.
        store.flush();
        assert_eq!(store.mend_gear(), (0, 0));
    }

    #[test]
    fn changes_are_written_and_come_back() {
        let repository = MemoryGameRepository::default();
        let store = GameAccounts::with_repository(Box::new(repository.clone())).unwrap();
        let account = store.get_or_create(5, "Alice");
        assert!(store.is_dirty(5));
        assert_eq!(store.flush(), 1);
        assert_eq!(repository.saves(), 1);
        // Nothing marked: nothing written.
        assert_eq!(store.flush(), 0);
        // Marked but equal to what was written: skipped.
        store.mark_dirty(5);
        assert_eq!(store.flush(), 0);
        assert_eq!(repository.saves(), 1);
        // A change.
        account.locked().generic_money = 77;
        store.mark_dirty(5);
        assert_eq!(store.flush(), 1);
        assert_eq!(repository.get(5).unwrap().generic_money, 77);

        // A new store over the same repository has it.
        let store = GameAccounts::with_repository(Box::new(repository.clone())).unwrap();
        assert_eq!(store.get(5).unwrap().locked().generic_money, 77);
        assert!(!store.is_dirty(5));
    }

    #[test]
    fn a_damaged_account_is_repaired_at_load_and_written_again() {
        let mut repository = MemoryGameRepository::default();
        let mut account = Account::new("Alice");
        account.active_kit = 99;
        account.kits[0].slots[0] = Some(KitSlot {
            item_id: 1234,
            count: 1,
        });
        repository.save(5, &account).unwrap();
        let store = GameAccounts::with_repository(Box::new(repository.clone())).unwrap();
        {
            let account = store.get(5).unwrap();
            let account = account.locked();
            assert_eq!(account.active_kit, 1);
            assert_eq!(account.kits[0].slots[0], None);
        }
        assert!(store.is_dirty(5));
        assert_eq!(store.flush(), 1);
        assert_eq!(repository.get(5).unwrap().active_kit, 1);
    }

    /// An item of `cfg_name`'s config, owned by `account`, worn in `slot`
    /// of its first kit as a kit saved before the covered-slots rule.
    fn worn(account: &mut Account, cfg_name: &str, slot: usize) -> u32 {
        let dictionary = survarium::v026g::dictionary::Dictionary::shared();
        let item = dictionary
            .items
            .iter()
            .find(|item| item.cfg_name == cfg_name)
            .unwrap_or_else(|| panic!("{cfg_name}"));
        assert!(
            dictionary.fits(item.dict_id, slot as u8),
            "{cfg_name} in {slot}"
        );
        let item_id = account.add_item(item.dict_id, 100);
        account.kits[0].slots[slot] = Some(KitSlot { item_id, count: 1 });
        item_id
    }

    #[test]
    fn kits_saved_against_the_covered_slots_rule_are_mended_at_load() {
        let mut repository = MemoryGameRepository::default();
        // The Pilgrim helmet, which has its mask built in, over a mask, and
        // a torso beside them.
        let mut pilgrim = Account::new("Alice");
        let helmet = worn(
            &mut pilgrim,
            "gameplay/items/armour/helmet/edge_helmet_6.options",
            0,
        );
        let mask = worn(
            &mut pilgrim,
            "gameplay/items/armour/mask/bandit_026.options",
            1,
        );
        let torso = worn(
            &mut pilgrim,
            "gameplay/items/armour/torso/scavenger_torso_1.options",
            2,
        );
        let pistol = pilgrim.kits[0].slots[7];
        repository.save(5, &pilgrim).unwrap();
        // The Zubr UM-4 helmet, which has its mask built in, over a mask.
        let mut zubr = Account::new("Bob");
        let zubr_helmet = worn(
            &mut zubr,
            "gameplay/items/armour/helmet/renesanse_helmet_6.options",
            0,
        );
        let zubr_mask = worn(
            &mut zubr,
            "gameplay/items/armour/mask/bandit_026.options",
            1,
        );
        repository.save(6, &zubr).unwrap();

        let store = GameAccounts::with_repository(Box::new(repository.clone())).unwrap();
        {
            let account = store.get(5).unwrap();
            let account = account.locked();
            let slots = account.kits[0].slots;
            assert_eq!(slots[0].map(|held| held.item_id), Some(helmet));
            assert!(slots[1].is_none());
            assert_eq!(slots[2].map(|held| held.item_id), Some(torso));
            assert_eq!(slots[7], pistol);
            assert!(account.item(mask).is_some(), "taken off, still owned");
            assert!(account.kits[0].revision > pilgrim.kits[0].revision);
        }
        {
            let account = store.get(6).unwrap();
            let account = account.locked();
            let slots = account.kits[0].slots;
            assert_eq!(slots[0].map(|held| held.item_id), Some(zubr_helmet));
            assert!(slots[1].is_none());
            assert!(account.item(zubr_mask).is_some());
        }
        assert!(store.is_dirty(5) && store.is_dirty(6));
        assert_eq!(store.flush(), 2);
        assert!(repository.get(5).unwrap().kits[0].slots[1].is_none());
        // Mended once: loaded again, there is nothing to do.
        let store = GameAccounts::with_repository(Box::new(repository.clone())).unwrap();
        assert!(!store.is_dirty(5) && !store.is_dirty(6));
    }

    /// A kit saved before the rule of one pocket an item type: the speed
    /// booster in three pockets. Merged at load into the first, written
    /// once, with the inventory as it was.
    #[test]
    fn kits_saved_with_a_type_in_two_pockets_are_merged_at_load() {
        let mut repository = MemoryGameRepository::default();
        let mut account = Account::new("Alice");
        let speed = survarium::v026g::dictionary::Dictionary::shared()
            .item_named("speed_booster")
            .unwrap()
            .dict_id;
        let booster = account.add_item(speed, 3);
        for pocket in [14, 15, 17] {
            account.kits[0].slots[pocket] = Some(KitSlot {
                item_id: booster,
                count: 1,
            });
        }
        repository.save(5, &account).unwrap();

        let store = GameAccounts::with_repository(Box::new(repository.clone())).unwrap();
        {
            let loaded = store.get(5).unwrap();
            let loaded = loaded.locked();
            let pockets: Vec<Option<u32>> = loaded.kits[0].slots[13..=18]
                .iter()
                .map(|slot| slot.map(|held| held.item_id))
                .collect();
            assert_eq!(pockets, [None, Some(booster), None, None, None, None]);
            assert_eq!(loaded.inventory, account.inventory);
        }
        assert!(store.is_dirty(5));
        assert_eq!(store.flush(), 1);
        let store = GameAccounts::with_repository(Box::new(repository.clone())).unwrap();
        assert!(!store.is_dirty(5));
    }

    /// Data made without a patch: the accounts' items, modifications and
    /// quests it lacks stop the load, each named with its accounts, and
    /// nothing of any account is changed or written (the old `repair`
    /// threw such items away and saved that).
    #[test]
    fn accounts_naming_what_the_game_data_lacks_stop_the_load_unchanged() {
        let mut repository = MemoryGameRepository::default();
        let mut alice = Account::new("Alice");
        let lost = alice.add_item(9999, 100);
        alice.inventory[0].modifications.push((987_654, false));
        alice.extras.bonus.reward_dict = 9998;
        repository.save(5, &alice).unwrap();
        let mut bob = Account::new("Bob");
        bob.add_item(9999, 100);
        bob.quest_cooldowns.push((987_655, 1_900_000_000));
        repository.save(6, &bob).unwrap();
        // One whose items are all known is no part of the message.
        repository.save(7, &Account::new("Carol")).unwrap();
        let saves = repository.saves();

        let error = GameAccounts::with_repository(Box::new(repository.clone()))
            .err()
            .expect("refused");
        assert!(matches!(error, PersistError::GameData(_)), "{error}");
        let error = error.to_string();
        for named in [
            "item dict ids 9998 (account 5), 9999 (account 5, account 6)",
            "modification ids 987654 (account 5)",
            "quest ids 987655 (account 6)",
            "srp-game-data build",
        ] {
            assert!(error.contains(named), "{named}: {error}");
        }
        assert!(!error.contains("account 7"), "{error}");
        assert_eq!(repository.saves(), saves);
        assert_eq!(repository.get(5).unwrap(), alice);
        assert!(repository.get(5).unwrap().item(lost).is_some());

        // The data has them again (here: they are taken out): it loads.
        alice.inventory.retain(|item| item.item_id != lost);
        alice.inventory[0].modifications.pop();
        alice.extras.bonus.reward_dict = 0;
        repository.save(5, &alice).unwrap();
        bob.inventory.retain(|item| item.dict_id != 9999);
        bob.quest_cooldowns.clear();
        repository.save(6, &bob).unwrap();
        assert!(GameAccounts::with_repository(Box::new(repository)).is_ok());
    }

    #[test]
    fn the_ranking_is_by_rating() {
        let store = GameAccounts::new();
        store.get_or_create(1, "Alice");
        store.get_or_create(2, "Bob").locked().rating = 1200;
        store.get_or_create(3, "Eve");
        let names: Vec<String> = store.ranking().into_iter().map(|row| row.1).collect();
        assert_eq!(names, ["Bob", "Alice", "Eve"]);
    }

    #[test]
    fn the_board_is_of_those_who_played_and_counts_their_battles() {
        let store = GameAccounts::new();
        // Never in a match: not on the board, whatever the rating.
        store.get_or_create(1, "Alice").locked().rating = 2000;
        for (id, name, rating, plays) in [
            (2, "Bob", 1200, 3),
            (3, "Eve", 1300, 1),
            (4, "Dan", 1200, 2),
        ] {
            let account = store.get_or_create(id, name);
            let mut account = account.locked();
            account.rating = rating;
            account.clan_tag = if id == 3 { "TAG".into() } else { String::new() };
            for _ in 0..plays {
                matchmaking::count_map_play(&mut account, 1 + id as u8 % 2);
            }
        }
        let board = store.board(10);
        let rows: Vec<(&str, &str, u16)> = board
            .top
            .iter()
            .map(|row| (row.name.as_str(), row.clan_tag.as_str(), row.rating))
            .collect();
        assert_eq!(
            rows,
            [("Eve", "TAG", 1300), ("Bob", "", 1200), ("Dan", "", 1200)]
        );
        assert_eq!(board.battles, 6);
        assert_eq!(store.board(2).top.len(), 2);
        assert_eq!(store.board(2).battles, 6);
        // A test bot is neither listed nor counted.
        store.set_bots(&["bob".to_owned()]);
        let board = store.board(10);
        assert_eq!(board.top.len(), 2);
        assert_eq!(board.battles, 3);
        assert_eq!(GameAccounts::new().board(10), Board::default());
    }

    fn admin_action(account_id: u32, change: &str) -> AdminAction {
        AdminAction {
            at: 1,
            admin: "t".into(),
            account_id,
            account: "Alice".into(),
            kind: "money".into(),
            change: change.into(),
            reason: "farmed".into(),
            before: "a".into(),
            after: "b".into(),
        }
    }

    #[test]
    fn an_admin_change_is_written_at_once_with_its_audit_row() {
        let repository = MemoryGameRepository::default();
        let store = GameAccounts::with_repository(Box::new(repository.clone())).unwrap();
        let account = store.get_or_create(5, "Alice");
        store.get_or_create(6, "Bob");
        store.flush();
        let saves = repository.saves();
        account.locked().generic_money = 7;
        // Bob changed too, but only the admin's account is written now.
        store.get(6).unwrap().locked().generic_money = 9;
        store.mark_dirty(6);
        assert!(matches!(
            store.write_admin(5, admin_action(5, "first")),
            ResetWrite::Written
        ));
        assert_eq!(repository.saves(), saves + 1);
        assert_eq!(repository.get(5).unwrap().generic_money, 7);
        assert_ne!(repository.get(6).unwrap().generic_money, 9);
        assert_eq!(repository.all_admin_actions(), [admin_action(5, "first")]);
        assert_eq!(
            store.admin_actions(5, 10, true).unwrap(),
            [admin_action(5, "first")]
        );
        assert!(store.admin_actions(6, 10, true).unwrap().is_empty());
        // A store kept in memory keeps the log only.
        let memory = GameAccounts::new();
        memory.get_or_create(5, "Alice");
        assert!(matches!(
            memory.write_admin(5, admin_action(5, "x")),
            ResetWrite::Written
        ));
        assert!(memory.admin_actions(5, 10, true).unwrap().is_empty());
    }

    #[test]
    fn an_admin_change_the_database_refused_is_an_error_and_retried() {
        let repository = MemoryGameRepository::default();
        let store = GameAccounts::with_repository(Box::new(repository.clone())).unwrap();
        let account = store.get_or_create(5, "Alice");
        store.flush();
        repository.fail_saves(true);
        account.locked().premium_money = 3;
        assert!(matches!(
            store.write_admin(5, admin_action(5, "refused")),
            ResetWrite::Failed(_)
        ));
        assert!(store.is_dirty(5));
        assert!(repository.all_admin_actions().is_empty());
        // Waiting rows are shown all the same, the newest first.
        store.write_admin(5, admin_action(5, "second"));
        let shown: Vec<String> = store
            .admin_actions(5, 10, true)
            .unwrap()
            .into_iter()
            .map(|action| action.change)
            .collect();
        assert_eq!(shown, ["second", "refused"]);
        // The saver's next pass writes the account and the rows, in order.
        repository.fail_saves(false);
        store.flush();
        assert_eq!(repository.get(5).unwrap().premium_money, 3);
        let written: Vec<String> = repository
            .all_admin_actions()
            .into_iter()
            .map(|action| action.change)
            .collect();
        assert_eq!(written, ["refused", "second"]);
        assert_eq!(store.admin_actions(5, 1, true).unwrap().len(), 1);
    }

    #[test]
    fn an_audit_row_is_never_kept_without_its_account_change() {
        let repository = MemoryGameRepository::default();
        let store = GameAccounts::with_repository(Box::new(repository.clone())).unwrap();
        let account = store.get_or_create(5, "Alice");
        account.locked().generic_money = 500;
        store.flush();
        // The database refuses the account, not a row alone.
        repository.fail_account_saves(true);
        account.locked().generic_money = 0;
        assert!(matches!(
            store.write_admin(5, admin_action(5, "taken")),
            ResetWrite::Failed(_)
        ));
        assert_eq!(repository.get(5).unwrap().generic_money, 500);
        assert!(
            repository.all_admin_actions().is_empty(),
            "the row tells of a change the database does not have"
        );
        // The saver's retry, refused again: still neither.
        store.flush();
        assert_eq!(repository.get(5).unwrap().generic_money, 500);
        assert!(repository.all_admin_actions().is_empty());
        assert!(store.is_dirty(5));
        // Then both, in one write.
        repository.fail_account_saves(false);
        assert_eq!(store.flush(), 1);
        assert_eq!(repository.get(5).unwrap().generic_money, 0);
        assert_eq!(repository.all_admin_actions(), [admin_action(5, "taken")]);
        // Nothing is written twice.
        store.flush();
        assert_eq!(repository.all_admin_actions().len(), 1);
    }

    #[test]
    fn an_admin_change_waits_for_a_busy_saver_only_so_long() {
        let repository = MemoryGameRepository::default();
        let store = Arc::new(GameAccounts::with_repository(Box::new(repository.clone())).unwrap());
        store.get_or_create(5, "Alice").locked().generic_money = 4;
        let held = store.writer.as_ref().unwrap().locked();
        let pending = std::thread::scope(|scope| {
            scope
                .spawn(|| store.write_admin(5, admin_action(5, "busy")))
                .join()
                .unwrap()
        });
        assert!(matches!(pending, ResetWrite::Pending));
        // The history after a write that waited already: no second wait.
        let started = Instant::now();
        let history = std::thread::scope(|scope| {
            scope
                .spawn(|| store.admin_actions(5, 10, false))
                .join()
                .unwrap()
        });
        assert!(started.elapsed() < RESET_WRITE_WAIT, "it waited again");
        assert_eq!(
            history,
            Err("the database is busy; shown next time".to_owned())
        );
        drop(held);
        assert!(store.is_dirty(5));
        store.flush();
        assert_eq!(repository.get(5).unwrap().generic_money, 4);
        assert_eq!(repository.all_admin_actions().len(), 1);
    }
}
