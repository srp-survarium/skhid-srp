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
