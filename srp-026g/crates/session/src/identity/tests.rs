use std::sync::Barrier;

use super::*;
use crate::persist::MemoryRepository;
use crate::{DEFAULT_GATHER_WINDOW, SignInError};

fn register(store: &SessionStore, login: &str) -> u32 {
    store.find_or_create(&login.to_lowercase(), login, None).0
}

fn name(store: &SessionStore, id: u32) -> String {
    store.account(id).unwrap().locked().name.clone()
}

fn player(store: &SessionStore, games: &GameAccounts, login: &str) -> u32 {
    let id = register(store, login);
    games.get_or_create(id, &name(store, id));
    id
}

fn free_renames() -> premium::Settings {
    premium::Settings {
        rename_cost: 0,
        ..premium::Settings::default()
    }
}

#[test]
fn a_sign_up_whose_name_is_taken_gets_its_id_after_it_and_signs_in() {
    let store = SessionStore::new();
    let adam = store.sign_in("Adam@first.example", b"one").unwrap();
    let other = store.sign_in("adam@other.example", b"two").unwrap();
    assert_ne!(adam, other);
    assert_eq!(name(&store, adam), "Adam");
    assert_eq!(name(&store, other), format!("adam_{other}"));
    assert_eq!(store.sign_in("adam@other.example", b"two"), Ok(other));
    assert_eq!(
        store.sign_in("adam@other.example", b"one"),
        Err(SignInError::WrongPassword)
    );
}

#[test]
fn registration_checks_display_names_after_email_split_and_wire_truncation() {
    let store = SessionStore::new();
    let id = register(&store, "Mamoru@first.example");
    for login in ["Mamoru", "mAMORU@other.example"] {
        let other = register(&store, login);
        let shown = login.split('@').next().unwrap();
        assert_eq!(name(&store, other), format!("{shown}_{other}"));
    }
    assert_eq!(register(&store, "MAMORU@FIRST.EXAMPLE"), id);
    let long = "a".repeat(63);
    register(&store, &format!("{long}one"));
    let two = register(&store, &format!("{long}two"));
    assert_eq!(name(&store, two), format!("{}_{two}", &long[..61]));
}

#[test]
fn registration_cannot_take_a_renamed_players_name() {
    let store = SessionStore::new();
    let games = GameAccounts::new();
    let id = player(&store, &games, "Original");
    store
        .rename_026g(
            &games,
            &free_renames(),
            id,
            &Nickname::parse("Mamoru").unwrap(),
        )
        .unwrap();
    let other = register(&store, "Mamoru");
    assert_eq!(name(&store, other), format!("Mamoru_{other}"));
    assert_eq!(store.account_id_by_name("Original"), Some(id));
    assert_eq!(store.account(id).unwrap().locked().name(), "Mamoru");
    assert_eq!(games.get(id).unwrap().locked().name, "Mamoru");
}

#[test]
fn a_failed_rename_neither_charges_nor_reserves_the_name() {
    let store = SessionStore::new();
    let games = GameAccounts::new();
    let alice = player(&store, &games, "Alice");
    let bob = player(&store, &games, "Bob");
    let settings = premium::Settings::default();
    games.get(alice).unwrap().locked().generic_money = 100_000;
    assert_eq!(
        store.rename_026g(&games, &settings, alice, &Nickname::parse("bOB").unwrap(),),
        Err(RenameError::Taken)
    );
    assert_eq!(games.get(alice).unwrap().locked().generic_money, 100_000);
    games.get(alice).unwrap().locked().generic_money = 0;
    let free = Nickname::parse("Unclaimed").unwrap();
    assert_eq!(
        store.rename_026g(&games, &settings, alice, &free),
        Err(RenameError::NotEnoughMoney)
    );
    store
        .rename_026g(&games, &free_renames(), bob, &free)
        .unwrap();
    assert_eq!(store.account(alice).unwrap().locked().name(), "Alice");
}

#[test]
fn simultaneous_registrations_claim_one_display_name() {
    let store = SessionStore::new();
    let barrier = Barrier::new(8);
    let names: Vec<(u32, String)> = std::thread::scope(|scope| {
        let jobs: Vec<_> = (0..8)
            .map(|i| {
                let (store, barrier) = (&store, &barrier);
                scope.spawn(move || {
                    barrier.wait();
                    register(store, &format!("Same@{i}.example"))
                })
            })
            .collect();
        jobs.into_iter()
            .map(|job| job.join().unwrap())
            .map(|id| (id, name(&store, id)))
            .collect()
    });
    assert_eq!(names.iter().filter(|(_, name)| name == "Same").count(), 1);
    unique_names(names.iter().map(|(id, name)| (*id, name.as_str()))).unwrap();
    assert_eq!(store.registered_accounts(), 8);
}

#[test]
fn registration_and_rename_cannot_claim_the_same_name() {
    for _ in 0..12 {
        let store = SessionStore::new();
        let games = GameAccounts::new();
        let alice = player(&store, &games, "Alice");
        let barrier = Barrier::new(2);
        let (registered, renamed) = std::thread::scope(|scope| {
            let registration = scope.spawn(|| {
                barrier.wait();
                register(&store, "Target")
            });
            let rename = scope.spawn(|| {
                barrier.wait();
                store.rename_026g(
                    &games,
                    &free_renames(),
                    alice,
                    &Nickname::parse("target").unwrap(),
                )
            });
            (registration.join().unwrap(), rename.join().unwrap())
        });
        // Whoever is first has the name: a registration after the rename
        // gets its id after it, a rename after the registration is refused.
        let registered = name(&store, registered);
        match renamed {
            Ok(_) => assert_ne!(registered, "Target"),
            Err(error) => {
                assert_eq!(error, RenameError::Taken);
                assert_eq!(registered, "Target");
            }
        }
        assert_ne!(key(&registered), key(&name(&store, alice)));
    }
}

#[test]
fn simultaneous_renames_charge_only_the_winner() {
    let store = SessionStore::new();
    let games = GameAccounts::new();
    let players = [
        player(&store, &games, "Alice"),
        player(&store, &games, "Bob"),
    ];
    for id in players {
        games.get(id).unwrap().locked().generic_money = 100_000;
    }
    let barrier = Barrier::new(2);
    let successes = std::thread::scope(|scope| {
        let jobs: Vec<_> = players
            .into_iter()
            .map(|id| {
                let (store, games, barrier) = (&store, &games, &barrier);
                scope.spawn(move || {
                    barrier.wait();
                    store.rename_026g(
                        games,
                        &premium::Settings::default(),
                        id,
                        &Nickname::parse("Target").unwrap(),
                    )
                })
            })
            .collect();
        jobs.into_iter()
            .map(|job| job.join().unwrap())
            .filter(Result::is_ok)
            .count()
    });
    assert_eq!(successes, 1);
    let total: u32 = players
        .into_iter()
        .map(|id| games.get(id).unwrap().locked().generic_money)
        .sum();
    assert_eq!(total, 200_000 - premium::Settings::default().rename_cost);
}

#[test]
fn nickname_ownership_survives_reload_and_repairs_the_game_copy() {
    let repository = MemoryRepository::default();
    let store =
        SessionStore::with_repository(DEFAULT_GATHER_WINDOW, Box::new(repository.clone())).unwrap();
    let games = GameAccounts::new();
    let id = player(&store, &games, "Original");
    store
        .rename_026g(
            &games,
            &free_renames(),
            id,
            &Nickname::parse("Renamed").unwrap(),
        )
        .unwrap();
    store.flush();
    let restored =
        SessionStore::with_repository(DEFAULT_GATHER_WINDOW, Box::new(repository)).unwrap();
    let stale_games = GameAccounts::new();
    stale_games.get_or_create(id, "Original");
    restored.restore_026g_names(&stale_games);
    assert_eq!(stale_games.get(id).unwrap().locked().name, "Renamed");
    let other = register(&restored, "RENAMED");
    assert_eq!(name(&restored, other), format!("RENAMED_{other}"));
    assert_eq!(restored.account_id_by_name("Original"), Some(id));
}

#[test]
fn lookalikes_get_the_id_at_registration_and_are_refused_before_charging_for_a_rename() {
    let store = SessionStore::new();
    let games = GameAccounts::new();
    player(&store, &games, "sovspiranha");
    for login in ["sоvspiranha", "SОVSPIRANHA@other.example"] {
        let other = register(&store, login);
        let shown = login.split('@').next().unwrap();
        assert_eq!(name(&store, other), format!("{shown}_{other}"));
    }
    let other = player(&store, &games, "Other");
    games.get(other).unwrap().locked().generic_money = 100_000;
    let name = Nickname::parse("sоvspiranha").unwrap();
    assert!(store.nickname_taken(name.as_str(), other));
    assert_eq!(
        store.rename_026g(&games, &premium::Settings::default(), other, &name),
        Err(RenameError::Taken)
    );
    assert_eq!(games.get(other).unwrap().locked().generic_money, 100_000);
    assert_eq!(store.account(other).unwrap().locked().name(), "Other");
}

#[test]
fn admin_rename_is_free_and_written_without_ending_sessions_or_changing_login() {
    let repository = MemoryRepository::default();
    let store =
        SessionStore::with_repository(DEFAULT_GATHER_WINDOW, Box::new(repository.clone())).unwrap();
    let games = GameAccounts::new();
    let id = player(&store, &games, "Mamoru");
    let session_id = store.create_session(id);
    let before = games.get(id).unwrap().locked().clone();
    let result = store
        .admin_rename_026g(&games, id, &Nickname::parse("Mamoru_777").unwrap())
        .unwrap();
    assert_eq!(result.previous, "Mamoru");
    assert_eq!(store.session_account(session_id).unwrap().locked().id, id);
    assert!(matches!(result.written, ResetWrite::Written));
    let mut expected = before;
    expected.name = "Mamoru_777".into();
    assert_eq!(*games.get(id).unwrap().locked(), expected);
    let restored =
        SessionStore::with_repository(DEFAULT_GATHER_WINDOW, Box::new(repository)).unwrap();
    assert_eq!(restored.account(id).unwrap().locked().name(), "Mamoru_777");
    assert_eq!(restored.account_id_by_name("Mamoru"), Some(id));
}

#[test]
fn admin_rename_handles_accounts_without_a_game_and_retries_failed_writes() {
    let repository = MemoryRepository::default();
    let store =
        SessionStore::with_repository(DEFAULT_GATHER_WINDOW, Box::new(repository.clone())).unwrap();
    let games = GameAccounts::new();
    let id = register(&store, "Original");
    repository.fail_account_saves(true);
    let result = store
        .admin_rename_026g(&games, id, &Nickname::parse("Renamed").unwrap())
        .unwrap();
    assert!(matches!(result.written, ResetWrite::Failed(_)));
    assert_eq!(store.account(id).unwrap().locked().name(), "Renamed");
    assert!(games.get(id).is_none());
    repository.fail_account_saves(false);
    store.flush();
    let restored =
        SessionStore::with_repository(DEFAULT_GATHER_WINDOW, Box::new(repository)).unwrap();
    assert_eq!(restored.account(id).unwrap().locked().name(), "Renamed");
}

#[test]
fn first_lobby_entry_cannot_restore_a_name_changed_by_an_admin() {
    for _ in 0..16 {
        let store = SessionStore::new();
        let games = GameAccounts::new();
        let id = register(&store, "Original");
        let barrier = Barrier::new(2);
        std::thread::scope(|scope| {
            scope.spawn(|| {
                barrier.wait();
                store.sign_up_026g(&games, id).unwrap();
            });
            scope.spawn(|| {
                barrier.wait();
                store
                    .admin_rename_026g(&games, id, &Nickname::parse("Renamed").unwrap())
                    .unwrap();
            });
        });
        assert_eq!(store.account(id).unwrap().locked().name(), "Renamed");
        assert_eq!(games.get(id).unwrap().locked().name, "Renamed");
    }
}
