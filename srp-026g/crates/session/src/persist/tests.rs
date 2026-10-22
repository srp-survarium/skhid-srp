use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use rusqlite::Connection;
use survarium::v021d::account::{Item, Profile};
use survarium::v021d::quests::QuestInstance;

use super::sqlite::{MIGRATIONS, migrate};
use super::*;
use crate::{Account, SessionStore, SignInError};

const GATHER: Duration = Duration::from_secs(30);

/// A directory of its own under the system's temporary one, removed when
/// dropped.
struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("srp-persist-{name}-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    fn db(&self) -> PathBuf {
        self.0.join(DB_FILE)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn item(dict_id: u16, item_id: u32, condition_or_stack: u16, amount: u32, mods: &[u32]) -> Item {
    Item {
        dict_id,
        item_id,
        condition_or_stack,
        amount,
        modifications: mods.to_vec(),
    }
}

/// An account with every durable field set to something not default.
fn full_account(id: u32) -> Account {
    let mut account = Account::new(id, "Full Name".into());
    account.login = format!("full{id}@test.invalid");
    account.password_hash = crate::password::hash(b"test-password-1");
    account.friends = vec![9, 3];
    account.ignored = vec![12];
    account.active_profile = Some(2);
    account.notices = vec![(3, "#ru[1:2]".into()), (4, "quest done".into())];
    let game = &mut account.game;
    game.generic_money = 12_345;
    game.premium_money = 77;
    game.premium_until = 4_000_000_000;
    game.reputations = vec![(2, 150), (1, 40), (4, 0), (3, 9)];
    game.loyalty = [5, 0, 30, 100];
    game.rating = 1016;
    game.rating_before = 990;
    game.inventory.push(item(31, 40, 77, 1, &[3, 8, 1000]));
    game.inventory.push(item(50, 41, 300, 300, &[]));
    game.next_item_id = 42;
    let mut slots: [Option<Item>; survarium::dictionary::PROFILE_SLOTS] = Default::default();
    slots[2] = Some(item(31, 40, 77, 1, &[3, 8, 1000]));
    // A stack slot: its own per-life count and the owned amount.
    slots[11] = Some(item(50, 41, 30, 300, &[]));
    game.profiles.push(Profile {
        id: 2,
        name: "Second".into(),
        autobuy: true,
        level: 7,
        exp_total: 9_000,
        exp_level_start: 8_000,
        exp_next_level: 10_000,
        revision: 14,
        slots,
        skills: vec![(4, 2), (1, 1)],
        perks: vec![9, 2],
    });
    game.quests = vec![
        QuestInstance {
            instance_id: 5,
            quest_id: 1201,
            games: 2,
            expires_at: 1_800_000_000,
            active: true,
        },
        QuestInstance {
            instance_id: 6,
            quest_id: 1305,
            games: 0,
            expires_at: 1_800_000_100,
            active: false,
        },
    ];
    game.quest_cooldowns = vec![(1101, 1_800_000_500), (1102, 1_800_000_400)];
    game.next_quest_instance = 7;
    account
}

#[test]
fn a_full_account_comes_back_as_it_was_saved() {
    let dir = TempDir::new("round-trip");
    let record = AccountRecord::of(&full_account(3));
    let mut repository = SqliteRepository::open(&dir.db()).unwrap();
    repository.save_account(&record).unwrap();
    // Saving again replaces, it doesn't add.
    repository.save_account(&record).unwrap();
    drop(repository);

    let mut repository = SqliteRepository::open(&dir.db()).unwrap();
    let loaded = repository.load_all().unwrap();
    assert_eq!(loaded.accounts, std::slice::from_ref(&record));
    assert_eq!(loaded.next_account_id, 4);
    let mut again = loaded.accounts[0].clone();
    assert!(!again.repair());
    assert_eq!(again, record);
}

#[test]
fn accounts_and_passwords_survive_a_restart() {
    let dir = TempDir::new("restart");
    let open = || {
        let repository = SqliteRepository::open(&dir.db()).unwrap();
        SessionStore::with_repository(GATHER, Box::new(repository)).unwrap()
    };
    let store = open();
    let stepan = store.sign_in("Stepan", b"test-password-1").unwrap();
    let mail = store.sign_in("a@b.example", b"test-password-2").unwrap();
    store.account(stepan).unwrap().locked().game.generic_money = 4242;
    store.mark_dirty(stepan);
    assert_eq!(store.flush(), 2);
    drop(store);

    let store = open();
    assert_eq!(store.sign_in("stepan", b"test-password-1"), Ok(stepan));
    assert_eq!(store.sign_in("A@B.example", b"test-password-2"), Ok(mail));
    assert_eq!(
        store.sign_in("STEPAN", b"test-password-2"),
        Err(SignInError::WrongPassword)
    );
    let account = store.account(stepan).unwrap();
    assert_eq!(account.locked().game.generic_money, 4242);
    assert_eq!(account.locked().name, "Stepan");
    // The same user name from another address is the same account.
    let elsewhere = store.get_or_create_account([10, 1, 2, 3].into(), " stepan");
    assert_eq!(elsewhere, stepan);
    // New accounts carry on after the kept ones.
    assert_eq!(store.sign_in("third", b"test-password-3"), Ok(3));
}

/// The admin's password reset is in the database at once (srp may stop
/// before the saver's next pass), and the password the next sign-in sets
/// is kept too.
#[test]
fn a_reset_password_stays_reset_across_a_restart() {
    let dir = TempDir::new("reset-password");
    let open = || {
        let repository = SqliteRepository::open(&dir.db()).unwrap();
        SessionStore::with_repository(GATHER, Box::new(repository)).unwrap()
    };
    let store = open();
    let id = store.sign_in("Forgetful", b"test-password-1").unwrap();
    let kept = store.sign_in("kept", b"test-password-2").unwrap();
    let session = store.create_session(id);
    let kept_session = store.create_session(kept);
    // Signed out before the reset; its row is still to go.
    let earlier = store.create_session(id);
    store.flush();
    assert!(store.end_session(earlier));
    let reset = store.reset_password(id).unwrap();
    assert_eq!(reset.sessions_ended, 1);
    assert!(
        matches!(reset.written, crate::ResetWrite::Written),
        "{:?}",
        reset.written
    );
    // No flush: the reset wrote it, and deleted its session's row.
    let sessions = SqliteRepository::open(&dir.db())
        .unwrap()
        .load_sessions()
        .unwrap();
    let sessions: Vec<u32> = sessions.iter().map(|stored| stored.session_id).collect();
    assert_eq!(sessions, [kept_session]);
    assert!(!sessions.contains(&session) && !sessions.contains(&earlier));
    drop(store);

    let connection = Connection::open(dir.db()).unwrap();
    let rows: Vec<(String, Option<String>)> = connection
        .prepare("SELECT login, password_hash FROM accounts ORDER BY id")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    drop(connection);
    assert_eq!(rows[0], ("forgetful".to_owned(), None));
    assert!(rows[1].1.is_some());

    let store = open();
    assert_eq!(store.account(id).unwrap().locked().password_hash, None);
    assert_eq!(store.sign_in("forgetful", b"test-password-3"), Ok(id));
    assert_eq!(store.sign_in("kept", b"test-password-2"), Ok(kept));
    store.flush();
    drop(store);

    let store = open();
    assert_eq!(store.sign_in("Forgetful", b"test-password-3"), Ok(id));
    assert_eq!(
        store.sign_in("forgetful", b"test-password-1"),
        Err(SignInError::WrongPassword)
    );
}

/// The reset writes only its own account, and says so when the write
/// fails; the saver writes it later.
#[test]
fn a_reset_that_is_not_written_says_so_and_is_written_later() {
    let repository = MemoryRepository::default();
    let store = SessionStore::with_repository(GATHER, Box::new(repository.clone())).unwrap();
    let id = store.sign_in("forgetful", b"test-password-1").unwrap();
    let other = store.sign_in("other", b"test-password-2").unwrap();
    store.flush();
    let saves = repository.saves();
    // Another account changed meanwhile: not the reset's to write.
    store.account(other).unwrap().locked().game.rating = 7;
    store.mark_dirty(other);

    repository.fail_account_saves(true);
    let reset = store.reset_password(id).unwrap();
    assert!(
        matches!(&reset.written, crate::ResetWrite::Failed(PersistError::BadRow(account, _)) if *account == id),
        "{:?}",
        reset.written
    );
    assert!(repository.get(id).unwrap().password_hash.is_some());
    assert!(store.is_dirty(id));
    // Cleared in memory all the same: the next sign-in sets a password.
    assert_eq!(store.account(id).unwrap().locked().password_hash, None);
    assert_eq!(store.sign_in("forgetful", b"test-password-3"), Ok(id));

    repository.fail_account_saves(false);
    let reset = store.reset_password(id).unwrap();
    assert!(
        matches!(reset.written, crate::ResetWrite::Written),
        "{:?}",
        reset.written
    );
    assert_eq!(repository.get(id).unwrap().password_hash, None);
    assert_eq!(repository.saves(), saves + 1, "only its own account");
    assert!(store.is_dirty(other));
}

/// While the saver holds the database longer than the reset waits, the
/// reset does not hang: it says the write is pending, and the saver's next
/// pass writes the account and deletes the ended session's row.
#[test]
fn a_reset_while_the_saver_writes_is_left_to_the_saver() {
    let repository = MemoryRepository::default();
    let store = SessionStore::with_repository(GATHER, Box::new(repository.clone())).unwrap();
    let id = store.sign_in("forgetful", b"test-password-1").unwrap();
    let session = store.create_session(id);
    store.flush();
    assert!(repository.session(session).is_some());

    let reset = {
        // The saver's pass, as long as it takes.
        let _pass = store.persistence.as_ref().unwrap().writer.locked();
        let started = Instant::now();
        let reset = store.reset_password(id).unwrap();
        assert!(started.elapsed() >= crate::RESET_WRITE_WAIT);
        reset
    };
    assert!(
        matches!(reset.written, crate::ResetWrite::Pending),
        "{:?}",
        reset.written
    );
    assert_eq!(reset.sessions_ended, 1);
    assert!(repository.get(id).unwrap().password_hash.is_some());
    store.flush();
    assert_eq!(repository.get(id).unwrap().password_hash, None);
    assert!(repository.session(session).is_none());
}

#[test]
fn only_changed_accounts_are_written_again() {
    let repository = MemoryRepository::default();
    let store = SessionStore::with_repository(GATHER, Box::new(repository.clone())).unwrap();
    let first = store.get_or_create_account([127, 0, 0, 1].into(), "first");
    let second = store.get_or_create_account([127, 0, 0, 1].into(), "second");
    assert_eq!(store.flush(), 2);
    assert_eq!(repository.saves(), 2);

    // Marked but unchanged: nothing to write.
    store.mark_dirty(first);
    store.mark_dirty(second);
    assert_eq!(store.flush(), 0);
    assert_eq!(repository.saves(), 2);

    store.account(second).unwrap().locked().friends.push(first);
    store.mark_dirty(first);
    store.mark_dirty(second);
    assert_eq!(store.flush(), 1);
    assert_eq!(repository.get(second).unwrap().friends, [first]);
    // Unmarked changes wait for a mark.
    store.account(first).unwrap().locked().game.rating = 1;
    assert_eq!(store.flush(), 0);

    // A store over the same repository sees what was written.
    let reloaded = SessionStore::with_repository(GATHER, Box::new(repository.clone())).unwrap();
    assert_eq!(reloaded.account(second).unwrap().locked().friends, [first]);
    assert_eq!(
        reloaded.get_or_create_account([127, 0, 0, 1].into(), "third"),
        3
    );
}

/// A database as schema version 1 wrote it, with rows written by hand.
fn version_1_fixture(path: &std::path::Path) -> Connection {
    let connection = Connection::open(path).unwrap();
    migrate(&connection, 1).unwrap();
    connection
        .execute_batch(
            "INSERT INTO accounts (id, login, password_hash, name, game_name, generic_money,
                                   premium_money, premium_until, rating, rating_before,
                                   next_item_id, next_quest_instance, active_profile)
             VALUES (4, 'old@test.invalid', NULL, 'old', 'old', 10, 2, 0, 1000, 1000,
                     1, 1, NULL);
             INSERT INTO inventory VALUES (4, 7, 0, 18, 100, 1, '');
             INSERT INTO inventory VALUES (4, 9, 1, 50, 120, 120, '5,6');
             INSERT INTO profiles VALUES (4, 1, 0, 'old', 0, 3, 100, 50, 200, 2);
             INSERT INTO profile_slots VALUES (4, 1, 7, 7, 100, 1);
             INSERT INTO profile_slots VALUES (4, 1, 8, 9, 30, 120);
             INSERT INTO quests VALUES (4, 12, 0, 1201, 1, 1800000000, 1);
             INSERT INTO loyalty VALUES (4, 2, 15);
             INSERT INTO reputations VALUES (4, 0, 1, 25);",
        )
        .unwrap();
    connection
}

#[test]
fn an_older_database_loads_and_its_counters_are_repaired() {
    let dir = TempDir::new("old");
    let connection = version_1_fixture(&dir.db());
    // The ammo stack was sold while its slot still held it.
    connection
        .execute_batch(
            "PRAGMA foreign_keys = OFF;
             DELETE FROM inventory WHERE item_id = 9;",
        )
        .unwrap();
    drop(connection);

    let repository = SqliteRepository::open_with_backup(&dir.db(), &dir.0.join("backups")).unwrap();
    assert_eq!(repository.version().unwrap(), MIGRATIONS.len() as i64);
    let store = SessionStore::with_repository(GATHER, Box::new(repository)).unwrap();
    let account = store.account(4).unwrap();
    {
        let account = account.locked();
        let game = &account.game;
        assert_eq!(game.next_item_id, 8);
        assert_eq!(game.next_quest_instance, 13);
        assert_eq!(game.loyalty, [0, 15, 0, 0]);
        assert_eq!(game.reputations, [(1, 25)]);
        let slots = &game.profiles[0].slots;
        assert_eq!(slots[7].as_ref().map(|item| item.item_id), Some(7));
        assert_eq!(slots[8], None);
        assert_eq!(account.password_hash, None);
    }
    // The repaired account is written back, and new accounts come after it.
    assert_eq!(store.flush(), 1);
    assert_eq!(store.get_or_create_account([127, 0, 0, 1].into(), "new"), 5);
    // The old account takes the password of its next sign-in.
    assert_eq!(store.sign_in("OLD@test.invalid", b"test-password-1"), Ok(4));
    assert_eq!(
        store.sign_in("old@test.invalid", b"test-password-2"),
        Err(SignInError::WrongPassword)
    );
    // The backup holds the database as it was found.
    let backups: Vec<_> = std::fs::read_dir(dir.0.join("backups"))
        .unwrap()
        .flatten()
        .collect();
    assert_eq!(backups.len(), 1);
}

#[test]
fn a_database_newer_than_the_code_is_refused() {
    let dir = TempDir::new("newer");
    let connection = Connection::open(dir.db()).unwrap();
    migrate(&connection, MIGRATIONS.len()).unwrap();
    connection
        .pragma_update(None, "user_version", MIGRATIONS.len() as i64 + 1)
        .unwrap();
    drop(connection);
    for result in [
        SqliteRepository::open(&dir.db()),
        SqliteRepository::open_with_backup(&dir.db(), &dir.0.join("backups")),
    ] {
        assert!(matches!(
            result,
            Err(PersistError::NewerSchema { found, known })
                if found == known + 1 && known == MIGRATIONS.len() as i64
        ));
    }
}

#[test]
fn only_the_newest_backups_are_kept() {
    let dir = TempDir::new("backups");
    for stamp in 1..=13 {
        std::fs::write(dir.0.join(format!("srp-{stamp}.db")), b"").unwrap();
    }
    std::fs::write(dir.0.join("notes.txt"), b"").unwrap();
    prune_backups(&dir.0, BACKUPS_KEPT);
    let mut left: Vec<String> = std::fs::read_dir(&dir.0)
        .unwrap()
        .flatten()
        .map(|entry| entry.file_name().into_string().unwrap())
        .collect();
    left.sort();
    assert_eq!(left.len(), BACKUPS_KEPT + 1);
    assert!(left.contains(&"notes.txt".to_string()));
    assert!(left.contains(&"srp-13.db".to_string()));
    assert!(!left.contains(&"srp-3.db".to_string()));
}

#[test]
fn the_data_directory_is_made_and_holds_the_database() {
    let dir = TempDir::new("data-dir");
    let data = dir.0.join("nested").join("data");
    let repository = open_in(&data).unwrap();
    assert_eq!(
        repository.describe(),
        data.join(DB_FILE).display().to_string()
    );
    assert!(data.join(DB_FILE).exists());
}

/// The account a played session leaves (rewards, quests, ammo used and
/// bought back, premium, loyalty, rating, friends, notices) comes back from
/// the database whole after a restart.
#[test]
fn a_played_account_comes_back_whole_after_a_restart() {
    use std::collections::BTreeMap;
    use survarium::dictionary::Dictionary;
    use survarium::v021d::account::Account as GameAccount;
    use survarium::v021d::drops::Rng;
    use survarium::v021d::results::PlayerStats;
    use survarium::v021d::{loyalty, premium, quests, rating, results, supplies};

    let dir = TempDir::new("played");
    let open = || {
        let repository = SqliteRepository::open(&dir.db()).unwrap();
        SessionStore::with_repository(GATHER, Box::new(repository)).unwrap()
    };
    let store = open();
    let id = store.sign_in("Veteran", b"test-password-1").unwrap();
    let friend = store.sign_in("friend", b"test-password-2").unwrap();
    let account = store.account(id).unwrap();
    {
        let mut account = account.locked();
        let account = &mut *account;
        let now = quests::now();
        let mut rng = Rng::new(11);
        account.game = GameAccount::with_test_loadout("Veteran", Dictionary::shared());
        account.game.profiles[0].autobuy = true;
        let notices = quests::refresh(&mut account.game, now, &mut rng);
        account.notices.extend(notices);
        let stats = PlayerStats {
            name: "Veteran".into(),
            score: 150,
            kills: 4,
            finished: true,
            won: true,
            team_batteries: 3,
            ..PlayerStats::default()
        };
        let gear = results::gear_faction(&account.game.profiles[0]).0;
        results::apply_with(&mut account.game, 1, &stats, &mut rng);
        account.game.rating_before = account.game.rating;
        account.game.rating = rating::moved(account.game.rating, 16);
        let settled = supplies::settle(
            &mut account.game,
            1,
            &BTreeMap::from([(22, 50)]),
            &BTreeMap::from([(22, 15)]),
        );
        assert!(!settled.bought.is_empty());
        let notices = quests::evaluate(&mut account.game, &stats, gear, now, &mut rng);
        account.notices.extend(notices);
        premium::buy(&mut account.game, 1, now).unwrap();
        loyalty::raise(&mut account.game, 2);
        account.friends.push(friend);
        account.ignored.push(friend + 7);
        account.active_profile = Some(1);
        account.notices.push((3, "#ru[2:1]".into()));
    }
    store.mark_dirty(id);
    store.flush();
    let played = AccountRecord::of(&account.locked());
    assert!(played.game.profiles[0].exp_total > 0);
    assert!(played.game.premium_until > 0);
    drop(account);
    drop(store);

    let store = open();
    let account = store.account(id).unwrap();
    assert_eq!(AccountRecord::of(&account.locked()), played);
    assert!(account.locked().game.profiles[0].autobuy);
    drop(account);
    assert_eq!(store.sign_in("veteran", b"test-password-1"), Ok(id));
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// A store over `repository` holding one account, written.
fn store_with_account(repository: &MemoryRepository, user_name: &str) -> (SessionStore, u32) {
    let store = SessionStore::with_repository(GATHER, Box::new(repository.clone())).unwrap();
    let account_id = store.get_or_create_account([127, 0, 0, 1].into(), user_name);
    store.flush();
    (store, account_id)
}

/// A client signed in, srp restarts: the client's session still resolves to
/// its account, and its lobby connection is let in.
#[test]
fn a_session_survives_a_restart() {
    let dir = TempDir::new("sessions");
    let open = || {
        let repository = SqliteRepository::open(&dir.db()).unwrap();
        SessionStore::with_repository(GATHER, Box::new(repository)).unwrap()
    };
    let store = open();
    let account_id = store.sign_in("Stepan", b"test-password-1").unwrap();
    let session = store.create_session(account_id);
    store.flush();
    drop(store);

    let store = open();
    let account = store.account_for_session(session).unwrap();
    assert_eq!(account.locked().id, account_id);
    let hold = store.hold(session).unwrap();
    assert!(store.touch(session));
    assert_eq!(store.players_online(), 1);
    drop(hold);
    // New ids don't collide with the restored one.
    assert_ne!(store.create_session(account_id), session);
}

#[test]
fn an_ended_session_is_not_restored_and_its_row_is_gone() {
    let dir = TempDir::new("sessions-ended");
    let open = || {
        let repository = SqliteRepository::open(&dir.db()).unwrap();
        SessionStore::with_repository(GATHER, Box::new(repository)).unwrap()
    };
    let store = open();
    let account_id = store.sign_in("leaver", b"test-password-1").unwrap();
    let left = store.create_session(account_id);
    let stays = store.create_session(account_id);
    store.flush();
    assert!(store.end_session(left));
    store.flush();
    drop(store);

    let kept = SqliteRepository::open(&dir.db())
        .unwrap()
        .load_sessions()
        .unwrap();
    let kept: Vec<u32> = kept.iter().map(|session| session.session_id).collect();
    assert_eq!(kept, [stays]);
    let store = open();
    assert!(store.account_for_session(left).is_none());
    assert!(store.hold(left).is_none());
    assert!(store.account_for_session(stays).is_some());
}

#[test]
fn a_session_made_and_ended_between_passes_is_never_kept() {
    let repository = MemoryRepository::default();
    let (store, account_id) = store_with_account(&repository, "brief");
    let session = store.create_session(account_id);
    assert!(store.end_session(session));
    store.flush();
    assert_eq!(repository.session(session), None);
    assert_eq!(repository.session_saves(), 0);
}

#[test]
fn idle_ended_sessions_are_deleted() {
    let repository = MemoryRepository::default();
    let (store, account_id) = store_with_account(&repository, "idler");
    let session = store.create_session(account_id);
    store.flush();
    assert!(repository.session(session).is_some());
    let later = Instant::now() + crate::SESSION_IDLE_LIMIT + Duration::from_secs(1);
    assert_eq!(
        store.end_idle_sessions(later, crate::SESSION_IDLE_LIMIT),
        [session]
    );
    store.flush();
    assert_eq!(repository.session(session), None);
}

#[test]
fn only_recent_sessions_of_known_accounts_are_restored() {
    let repository = MemoryRepository::default();
    let (store, account_id) = store_with_account(&repository, "returning");
    drop(store);
    let limit = crate::SESSION_RESTORE_LIMIT.as_secs();
    let stored = |session_id, account_id, age: u64| StoredSession {
        client_revision: 0,
        session_id,
        account_id,
        last_seen_unix: unix_now() - age,
    };
    repository.put_session(stored(0x2_0001, account_id, limit - 60));
    repository.put_session(stored(0x2_0002, account_id, limit + 60));
    repository.put_session(stored(0x2_0003, 0xBAD, 10));

    let store = SessionStore::with_repository(GATHER, Box::new(repository.clone())).unwrap();
    assert!(store.account_for_session(0x2_0001).is_some());
    assert!(store.account_for_session(0x2_0002).is_none());
    assert!(store.account_for_session(0x2_0003).is_none());
    assert_eq!(store.session_count(), 1);
    // Dropped ones are deleted at once.
    assert!(repository.session(0x2_0001).is_some());
    assert_eq!(repository.session(0x2_0002), None);
    assert_eq!(repository.session(0x2_0003), None);
}

/// The time srp was down doesn't count: a restored session has the whole
/// idle window to come back, and ends once it passed.
#[test]
fn a_restored_session_gets_a_whole_idle_window() {
    let repository = MemoryRepository::default();
    let (store, account_id) = store_with_account(&repository, "patient");
    drop(store);
    // Last seen 50 minutes ago: far past the idle limit, within the
    // restore limit.
    repository.put_session(StoredSession {
        client_revision: 0,
        session_id: 0x2_0000,
        account_id,
        last_seen_unix: unix_now() - 3000,
    });
    let store = SessionStore::with_repository(GATHER, Box::new(repository.clone())).unwrap();
    let start = Instant::now();
    let idle = crate::SESSION_IDLE_LIMIT;
    assert!(store.end_idle_sessions(start, idle).is_empty());
    let almost = start + idle - Duration::from_secs(1);
    assert!(store.end_idle_sessions(almost, idle).is_empty());
    let past = start + idle + Duration::from_secs(1);
    assert_eq!(store.end_idle_sessions(past, idle), [0x2_0000]);
    store.flush();
    assert_eq!(repository.session(0x2_0000), None);
}

#[test]
fn a_restored_session_keeps_its_id_and_new_ones_never_take_it() {
    let repository = MemoryRepository::default();
    let (store, account_id) = store_with_account(&repository, "counter");
    drop(store);
    repository.put_session(StoredSession {
        client_revision: 0,
        session_id: 0x7FFF_FFF0,
        account_id,
        last_seen_unix: unix_now(),
    });
    let store = SessionStore::with_repository(GATHER, Box::new(repository)).unwrap();
    assert!(store.account_for_session(0x7FFF_FFF0).is_some());
    for _ in 0..100 {
        assert_ne!(store.create_session(account_id), 0x7FFF_FFF0);
    }
}

#[test]
fn bans_and_last_addresses_come_back_after_a_restart() {
    let dir = TempDir::new("bans");
    let open = || {
        let repository = SqliteRepository::open(&dir.db()).unwrap();
        SessionStore::with_repository(GATHER, Box::new(repository)).unwrap()
    };
    let store = open();
    let account_id = store.sign_in("banned", b"pw").unwrap();
    store.signed_in_from(account_id, "192.0.2.7".parse().unwrap());
    store.ban_account(
        account_id,
        crate::Ban {
            until: Some(unix_now() + 3600),
            reason: "test".into(),
        },
    );
    store.ban_net(
        crate::Net::parse("198.51.100.0/24").unwrap(),
        crate::Ban {
            until: None,
            reason: "range".into(),
        },
    );
    store.flush();
    drop(store);

    let store = open();
    assert_eq!(
        store.sign_in("banned", b"pw"),
        Err(crate::SignInError::Banned)
    );
    let (accounts, nets) = store.bans();
    assert_eq!(accounts.len(), 1);
    assert_eq!(accounts[0].3.reason, "test");
    assert_eq!(accounts[0].4, Some("192.0.2.7".parse().unwrap()));
    assert_eq!(nets.len(), 1);
    assert_eq!(nets[0].net.to_string(), "198.51.100.0/24");
    assert!(store.ip_ban("198.51.100.9".parse().unwrap()).is_some());
    assert!(store.unban_net(nets[0].net));
    assert!(store.unban_account(account_id));
    drop(store);

    let store = open();
    assert_eq!(store.sign_in("banned", b"pw"), Ok(account_id));
    assert!(store.ip_ban("198.51.100.9".parse().unwrap()).is_none());
}

/// A session's last sighting is written again only once
/// [`crate::SESSION_SAVE_INTERVAL`] passed since it was last written, and
/// only if it was seen since.
#[test]
fn a_last_sighting_is_written_at_most_once_a_minute() {
    let repository = MemoryRepository::default();
    let (store, account_id) = store_with_account(&repository, "pinger");
    let session = store.create_session(account_id);
    store.flush();
    let written = repository.session(session).unwrap();
    assert_eq!(repository.session_saves(), 1);
    let start = Instant::now();
    let interval = crate::SESSION_SAVE_INTERVAL;

    // Pings, but the interval isn't over.
    std::thread::sleep(Duration::from_millis(5));
    assert!(store.touch(session));
    store.save_dirty_at(start + interval / 2);
    assert_eq!(repository.session_saves(), 1);
    // Now it is: written, with its last sighting.
    store.save_dirty_at(start + interval + Duration::from_secs(1));
    assert_eq!(repository.session_saves(), 2);
    let rewritten = repository.session(session).unwrap();
    assert!(rewritten.last_seen_unix >= written.last_seen_unix);
    // Not seen since: nothing to write, however long.
    store.save_dirty_at(start + interval * 5);
    assert_eq!(repository.session_saves(), 2);
    // A connection held counts as seen all along.
    let hold = store.hold(session).unwrap();
    store.save_dirty_at(start + interval * 7);
    assert_eq!(repository.session_saves(), 3);
    drop(hold);
}

#[test]
fn a_version_1_database_gains_the_sessions_and_keeps_its_accounts() {
    let dir = TempDir::new("v1-sessions");
    drop(version_1_fixture(&dir.db()));
    let mut repository = SqliteRepository::open(&dir.db()).unwrap();
    assert_eq!(repository.version().unwrap(), MIGRATIONS.len() as i64);
    assert!(repository.load_sessions().unwrap().is_empty());
    let loaded = repository.load_all().unwrap();
    assert_eq!(loaded.accounts.len(), 1);
    assert_eq!(loaded.accounts[0].login, "old@test.invalid");
    let session = StoredSession {
        client_revision: 0,
        session_id: 0x1_0000,
        account_id: 4,
        last_seen_unix: unix_now(),
    };
    repository.save_session(&session).unwrap();
    assert_eq!(repository.load_sessions().unwrap(), [session]);
    repository.delete_session(0x1_0000).unwrap();
    assert!(repository.load_sessions().unwrap().is_empty());
}

#[test]
fn sessions_are_not_kept_without_a_repository() {
    let store = SessionStore::with_gather_window(GATHER);
    let account_id = store.get_or_create_account([127, 0, 0, 1].into(), "volatile");
    let session = store.create_session(account_id);
    assert_eq!(store.flush(), 0);
    assert!(store.end_session(session));
    assert_eq!(store.flush(), 0);
}

#[test]
fn client_revision_survives_restart_and_is_bound_to_one_sign_in() {
    let dir = TempDir::new("session-revision");
    let open = || {
        SessionStore::with_repository(GATHER, Box::new(SqliteRepository::open(&dir.db()).unwrap()))
            .unwrap()
    };
    let store = open();
    let account = store.get_or_create_account([127, 0, 0, 1].into(), "revision");
    let session = store.create_session_with_revision(account, 0x80);
    store.flush();
    drop(store);
    let store = open();
    assert_eq!(store.client_revision(session), Some(0x80));
    let stock = store.create_session(account);
    assert_eq!(store.client_revision(stock), Some(0));
    assert_eq!(store.client_revision(session), Some(0x80));
    store.end_session(session);
    assert_eq!(store.client_revision(session), None);
}

#[test]
fn old_sessions_migrate_to_the_stock_client_revision() {
    let dir = TempDir::new("old-session-revision");
    let connection = version_1_fixture(&dir.db());
    migrate(&connection, 3).unwrap();
    connection
        .execute("INSERT INTO sessions VALUES (12345, 4, 1234567)", [])
        .unwrap();
    drop(connection);
    let mut repository = SqliteRepository::open(&dir.db()).unwrap();
    let sessions = repository.load_sessions().unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].session_id, 12345);
    assert_eq!(sessions[0].client_revision, 0);
}
