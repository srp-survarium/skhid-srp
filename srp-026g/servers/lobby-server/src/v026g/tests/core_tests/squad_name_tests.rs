//! Registration cannot hide a squad friend behind a duplicate display name.

use super::*;

#[test]
fn registration_cannot_shadow_a_squad_friend() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    for nickname in ["Alice@other.example", "bOB@other.example"] {
        assert_eq!(
            world.store.sign_in(nickname, b"password"),
            Err(session::SignInError::InvalidUserName)
        );
        assert!(world.store.account_id_by_name(nickname).is_none());
    }
    form_squad(&mut world, alice, bob);
    assert_eq!(
        world.store.squads().of(world.ids[alice].0).unwrap().members,
        [world.ids[alice].0, world.ids[bob].0]
    );
    world.assert_clean();
}

#[test]
fn admin_rename_refreshes_the_connected_player_and_squad_recipient_lookup() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    world.take(alice);
    world.take(bob);
    let id = world.ids[alice].0;
    let balance = world.account(alice).generic_money;
    world
        .store
        .admin_rename_026g(
            world.accounts,
            id,
            &session::v026g::names::Nickname::parse("Alisa").unwrap(),
        )
        .unwrap();
    world.inbox.account_changed(id);
    world.settle();
    let decoded = world.take(alice);
    assert!(decoded.iter().any(|message| matches!(message,
        Decoded::BaseInfo { nickname, .. } if nickname == "Alisa")));
    assert!(decoded.iter().any(|message| matches!(message,
        Decoded::Account(account) if account.body.name == "Alisa")));
    assert_eq!(world.core.who_is(id).unwrap().name, "Alisa");
    assert_eq!(world.account(alice).generic_money, balance);
    form_squad(&mut world, bob, alice);
    assert_eq!(world.store.squads().of(id).unwrap().members.len(), 2);
    world.assert_clean();
}

#[test]
fn an_offline_admin_rename_refreshes_the_names_in_online_friends_lists() {
    let mut world = World::new();
    let bob = world.connect("Bob");
    let alice = world
        .store
        .get_or_create_account("127.0.0.1".parse().unwrap(), "Alice");
    world.accounts.get_or_create(alice, "Alice");
    world
        .store
        .account(world.ids[bob].0)
        .unwrap()
        .locked()
        .friends
        .push(alice);
    world.take(bob);
    world
        .store
        .admin_rename_026g(
            world.accounts,
            alice,
            &session::v026g::names::Nickname::parse("Alisa").unwrap(),
        )
        .unwrap();
    world.inbox.account_changed(alice);
    world.settle();
    assert!(world.take(bob).iter().any(|message| matches!(message,
        Decoded::Friendship(FriendshipPush::Friends(friends))
        if friends.iter().any(|friend| friend.account_id == alice && friend.name == "Alisa"))));
    world.assert_clean();
}
