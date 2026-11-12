use std::sync::Barrier;

use super::*;
use crate::persist::MemoryRepository;
use crate::{DEFAULT_GATHER_WINDOW, SignInError};

fn register(store: &SessionStore, login: &str) -> Result<u32, SignInError> {
    store
        .find_or_create(&login.to_lowercase(), login, None)
        .map(|(id, _)| id)
}

fn player(store: &SessionStore, games: &GameAccounts, login: &str) -> u32 {
    let id = register(store, login).unwrap();
    let name = store.account(id).unwrap().locked().name.clone();
    games.get_or_create(id, &name);
    id
}

fn free_renames() -> premium::Settings {
    premium::Settings {
        rename_cost: 0,
        ..premium::Settings::default()
    }
}

#[test]
fn registration_checks_display_names_after_email_split_and_wire_truncation() {
    let store = SessionStore::new();
    let id = register(&store, "Mamoru@first.example").unwrap();
    for login in ["Mamoru", "mAMORU@other.example"] {
        assert_eq!(register(&store, login), Err(SignInError::InvalidUserName));
        assert!(store.account_id_by_name(login).is_none());
    }
    assert_eq!(register(&store, "MAMORU@FIRST.EXAMPLE"), Ok(id));
    let long = "a".repeat(63);
    register(&store, &format!("{long}one")).unwrap();
    assert_eq!(
        register(&store, &format!("{long}two")),
        Err(SignInError::InvalidUserName)
    );
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
    assert_eq!(
        register(&store, "Mamoru"),
        Err(SignInError::InvalidUserName)
    );
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
    let successes = std::thread::scope(|scope| {
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
            .filter(Result::is_ok)
            .count()
    });
    assert_eq!(successes, 1);
    assert_eq!(store.registered_accounts(), 1);
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
        assert_ne!(registered.is_ok(), renamed.is_ok());
        assert_eq!(store.find_accounts(b"target", 0, 10).len(), 1);
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
    assert_eq!(
        register(&restored, "RENAMED"),
        Err(SignInError::InvalidUserName)
    );
    assert_eq!(restored.account_id_by_name("Original"), Some(id));
}

#[test]
fn lookalikes_are_refused_at_registration_and_before_charging_for_a_rename() {
    let store = SessionStore::new();
    let games = GameAccounts::new();
    player(&store, &games, "sovspiranha");
    for login in ["sоvspiranha", "SОVSPIRANHA@other.example"] {
        assert_eq!(register(&store, login), Err(SignInError::InvalidUserName));
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
