use super::*;
use session::v026g::persist::{GameRepository, MemoryGameRepository};
use survarium::v026g::lobby::requests::PlayerComment;

fn comment(name: &str, kind: u8, reason: u8) -> (u8, Vec<u8>) {
    let (id, mut body) = request::profile_text(5, name);
    body.extend([kind, reason]);
    (id, body)
}

fn stored(messages: &[Decoded]) -> bool {
    messages.iter().any(|message| {
        matches!(
            message,
            Decoded::Permitted(Permitted::NoEffect {
                op: 0x49,
                event: Some(5)
            })
        )
    })
}

fn world(repository: &MemoryGameRepository) -> World {
    World::with_accounts(
        Settings::local("127.0.0.1:0"),
        GameAccounts::with_repository(Box::new(repository.clone())).unwrap(),
    )
}

#[test]
fn reports_and_praise_are_stored_before_success_including_offline_targets() {
    let mut repository = MemoryGameRepository::default();
    let mut world = world(&repository);
    let alice = world.connect("Alice");
    let (bob, _) = signed_in(world.store, "Bob");
    world.take(alice);
    for (kind, reason, expected) in [
        (1, 1, PlayerComment::BotOrCheater),
        (1, 2, PlayerComment::Swearing),
        (1, 3, PlayerComment::UnsportingBehavior),
        (1, 4, PlayerComment::AccountTrading),
        (2, 1, PlayerComment::Friendly),
        (2, 2, PlayerComment::GoodPlayer),
        (2, 3, PlayerComment::BornLeader),
    ] {
        world.ask(alice, comment("bOB", kind, reason));
        let messages = world.take(alice);
        assert!(stored(&messages), "{messages:?}");
        assert!(denial(&messages, 0x49).is_none());
        let rows = repository.player_comments(bob, 10).unwrap();
        let row = &rows[0];
        assert_eq!(row.sender_id, world.ids[alice].0);
        assert_eq!(row.sender_name, "Alice");
        assert_eq!(row.target_id, bob);
        assert_eq!(row.target_name, "Bob");
        assert_eq!(row.comment, expected);
        assert!(row.at > 0);
    }
    assert_eq!(repository.player_comments(bob, 10).unwrap().len(), 7);
    // A repeated click is successful but does not inflate the counts.
    world.ask(alice, comment("Bob", 2, 3));
    assert!(stored(&world.take(alice)));
    assert_eq!(repository.player_comments(bob, 10).unwrap().len(), 7);
    world.assert_clean();
}

#[test]
fn bad_comments_and_failed_writes_never_receive_success() {
    let mut repository = MemoryGameRepository::default();
    let mut world = world(&repository);
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    world.take(alice);
    world.take(bob);
    for (name, kind, reason) in [
        ("Alice", 1, 1),
        ("Nobody", 1, 1),
        ("Bob", 0, 1),
        ("Bob", 1, 0),
        ("Bob", 1, 5),
        ("Bob", 2, 4),
        ("Bob", 255, 255),
    ] {
        world.ask(alice, comment(name, kind, reason));
        let messages = world.take(alice);
        assert!(!stored(&messages));
        assert!(denial(&messages, 0x49).is_some(), "{messages:?}");
    }
    repository.fail_saves(true);
    world.ask(alice, comment("Bob", 1, 1));
    let messages = world.take(alice);
    assert!(!stored(&messages));
    assert_eq!(
        denial(&messages, 0x49).as_deref(),
        Some("st_error_during_storing_user_comment")
    );
    assert!(
        repository
            .player_comments(world.ids[bob].0, 10)
            .unwrap()
            .is_empty()
    );
    repository.fail_saves(false);
    world.ask(alice, comment("Bob", 1, 1));
    assert!(stored(&world.take(alice)));
    assert_eq!(
        repository
            .player_comments(world.ids[bob].0, 10)
            .unwrap()
            .len(),
        1
    );
    // The target gets neither the report nor the sender's identity.
    assert!(world.take(bob).is_empty());
    world.assert_clean();
}

#[test]
fn live_renames_keep_feedback_and_daily_duplicates_attached_to_account_ids() {
    let mut repository = MemoryGameRepository::default();
    let mut world = world(&repository);
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    world.take(alice);
    world.ask(alice, comment("Bob", 1, 1));
    assert!(stored(&world.take(alice)));
    let original = repository.player_comments(world.ids[bob].0, 10).unwrap();

    for (player, name) in [(alice, "Alisa"), (bob, "Boris")] {
        let id = world.ids[player].0;
        world
            .store
            .admin_rename_026g(
                world.accounts,
                id,
                &session::v026g::names::Nickname::parse(name).unwrap(),
            )
            .unwrap();
        world.inbox.account_changed(id);
    }
    world.settle();
    world.take(alice);
    world.ask(alice, comment("Bob", 1, 2));
    assert_eq!(
        denial(&world.take(alice), 0x49).as_deref(),
        Some("st_player_not_found")
    );
    world.ask(alice, comment("Boris", 1, 1));
    assert!(stored(&world.take(alice)));
    assert_eq!(
        repository.player_comments(world.ids[bob].0, 10).unwrap(),
        original
    );

    world.ask(alice, comment("Boris", 2, 2));
    assert!(stored(&world.take(alice)));
    let rows = repository.player_comments(world.ids[bob].0, 10).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].sender_id, world.ids[alice].0);
    assert_eq!(rows[0].sender_name, "Alisa");
    assert_eq!(rows[0].target_name, "Boris");
    assert_eq!(rows[1].sender_name, "Alice");
    assert_eq!(rows[1].target_name, "Bob");
    world.assert_clean();
}
