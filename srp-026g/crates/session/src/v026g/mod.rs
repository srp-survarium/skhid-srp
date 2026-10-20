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
use std::sync::{Arc, Condvar, Mutex};

pub mod account;
pub mod autobuy;
pub mod balance;
pub mod bonus;
pub mod bots;
pub mod clans;
pub mod extras;
pub mod items_rules;
pub mod known;
pub mod matches_now;
pub mod matchmaking;
pub mod names;
pub mod notices;
pub mod pay;
pub mod persist;
pub mod premium;
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

use crate::LockExt;
use crate::persist::PersistError;

struct Writer {
    repository: Box<dyn GameRepository>,
    /// What was last written of each account: one equal to it is skipped.
    saved: HashMap<u32, Account>,
}

/// Something else kept in the accounts' repository, written at every
/// [`GameAccounts::flush`] after the accounts
/// ([`GameAccounts::on_flush`]).
pub type FlushHook = Box<dyn Fn(&mut dyn GameRepository) + Send + Sync>;

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
            if account.repair() || uncovered > 0 {
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
    pub fn from_env() -> Result<Self, PersistError> {
        match SqliteGameRepository::from_env()? {
            Some(mut repository) => {
                let mend = !pay::item_wear() && repository.first_time(persist::GEAR_MENDED)?;
                let store = Self::with_repository(Box::new(repository))?;
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
            tutorial::signup_grant(&mut account.locked());
        }
        (account, made)
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
        let mut written = 0;
        for account_id in dirty {
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
}
