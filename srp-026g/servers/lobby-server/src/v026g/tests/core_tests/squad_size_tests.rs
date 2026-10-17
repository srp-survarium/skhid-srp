//! The most players of a squad (`SRP_SQUAD_SIZE`, `SRP_SQUAD_FULL_TEXT`):
//! the invite that would make a squad bigger is refused, and its sender
//! told.

use super::*;
use crate::v026g::{parse_squad_size, squad_full_text_for};
use survarium::v026g::lobby::requests::id;

/// A lobby whose squads hold `size` players.
fn world_of(size: usize, gather: Duration) -> World {
    let mut settings = Settings::local("127.0.0.1:0");
    settings.squad_size = size;
    World::with_settings(gather, settings)
}

/// `inviter` invites `invited` (by name); the invite's id as `invited`
/// got it, if it got one.
fn invite(world: &mut World, inviter: (usize, &str), invited: (usize, &str)) -> Option<u32> {
    world.take(inviter.0);
    world.take(invited.0);
    world.ask(inviter.0, request::important(invited.1, 2, "invite"));
    importants(&world.take(invited.0), 2, inviter.1)
        .first()
        .copied()
}

/// `invited` accepts invite `invite_id` of `inviter`, as its client does it
/// ("membership accepted", then the answer); what `invited` was sent.
fn accept(world: &mut World, invited: usize, inviter_name: &str, invite_id: u32) -> Vec<Decoded> {
    world.ask(
        invited,
        request::important(inviter_name, 3, "membership accepted"),
    );
    world.ask(invited, request::answer(invite_id, 0));
    world.take(invited)
}

/// The inviter's client answers each acceptance it was passed by itself.
/// Returns how many it was passed from `invited_name`.
fn acceptances(world: &mut World, inviter: usize, invited_name: &str) -> usize {
    let accepted = importants(&world.take(inviter), 3, invited_name);
    for id in &accepted {
        world.ask(inviter, request::answer(*id, 0));
    }
    accepted.len()
}

/// The whole flow, `inviter` adding `invited` at the end.
fn join(world: &mut World, inviter: (usize, &str), invited: (usize, &str)) {
    let invite_id = invite(world, inviter, invited).expect("the invite");
    accept(world, invited.0, inviter.1, invite_id);
    assert_eq!(acceptances(world, inviter.0, invited.1), 1);
    world.ask(inviter.0, request::add_squad_member(invited.1));
}

fn members(world: &World, client: usize) -> Vec<u32> {
    world
        .store
        .squads()
        .of(world.ids[client].0)
        .map(|squad| squad.members)
        .unwrap_or_default()
}

#[test]
fn the_size_is_three_unless_the_server_is_told_otherwise() {
    let settings = Settings::local("127.0.0.1:0");
    assert_eq!(settings.squad_size, 3);
    assert_eq!(settings.squad_full_text, None);
    assert_eq!(parse_squad_size(" 2 "), Some(2));
    assert_eq!(parse_squad_size("3"), Some(3));
    for wrong in ["1", "4", "0", "two", ""] {
        assert_eq!(parse_squad_size(wrong), None, "{wrong}");
    }
    assert_eq!(squad_full_text_for(2), "Squads are limited to 2 players.");
    let world = World::new(Duration::ZERO);
    assert_eq!(world.store.squads().size(), 3);
}

#[test]
fn with_squads_of_two_two_players_squad_and_queue_together() {
    let mut world = world_of(2, Duration::ZERO);
    assert_eq!(world.store.squads().size(), 2);
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let carol = world.connect("Carol");
    join(&mut world, (alice, "Alice"), (bob, "Bob"));
    let (alice_id, bob_id) = (world.ids[alice].0, world.ids[bob].0);
    assert_eq!(members(&world, alice), [alice_id, bob_id]);
    assert_eq!(denial(&world.take(alice), id::PROFILE), None);

    world.ask(bob, request::play(1));
    world.ask(alice, request::play(1));
    world.ask(carol, request::play(1));
    world.advance(Duration::from_secs(2));
    let (alice_match, alice_team) = sent_to_match(&world.take(alice)).expect("Alice's match");
    let (bob_match, bob_team) = sent_to_match(&world.take(bob)).expect("Bob's match");
    let (carol_match, carol_team) = sent_to_match(&world.take(carol)).expect("Carol's match");
    assert_eq!((alice_match, alice_team), (bob_match, bob_team));
    assert_eq!(carol_match, alice_match);
    assert_ne!(carol_team, alice_team);
    world.assert_clean();
}

#[test]
fn with_squads_of_two_a_third_invite_is_refused_and_its_sender_told() {
    let mut world = world_of(2, Duration::from_secs(600));
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let carol = world.connect("Carol");
    join(&mut world, (alice, "Alice"), (bob, "Bob"));
    let squad = members(&world, alice);

    // The commander invites a third: Carol gets nothing, Alice is told.
    world.take(alice);
    world.ask(alice, request::important("Carol", 2, "invite"));
    assert_eq!(
        denial(&world.take(alice), id::PROFILE).as_deref(),
        Some("Squads are limited to 2 players.")
    );
    assert!(importants(&world.take(carol), 2, "Alice").is_empty());
    // Nor does a member who is not the commander get one through.
    world.take(bob);
    world.ask(bob, request::important("Carol", 2, "invite"));
    assert_eq!(
        denial(&world.take(bob), id::PROFILE).as_deref(),
        Some("Squads are limited to 2 players.")
    );
    assert!(importants(&world.take(carol), 2, "Bob").is_empty());

    // A client that adds Carol all the same adds nobody.
    world.ask(carol, request::important("Alice", 3, "membership accepted"));
    assert!(importants(&world.take(alice), 3, "Carol").is_empty());
    world.ask(alice, request::add_squad_member("Carol"));
    assert_eq!(members(&world, alice), squad);
    assert!(members(&world, carol).is_empty());

    // The text is a setting: a key of the game, or a sentence.
    world.core.settings.squad_full_text = Some("Two at most.".to_owned());
    world.take(alice);
    world.ask(alice, request::important("Carol", 2, "invite"));
    assert_eq!(
        denial(&world.take(alice), id::PROFILE).as_deref(),
        Some("Two at most.")
    );
    world.assert_clean();
}

/// Alice invites Bob and Carol before either is in: both invites go out.
/// Both accept before her client adds either (the two `4A 02` come one
/// after the other): only the first is added, and she and Carol are told.
#[test]
fn with_squads_of_two_two_invites_accepted_at_once_add_one() {
    let mut world = world_of(2, Duration::from_secs(600));
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let carol = world.connect("Carol");
    let to_bob = invite(&mut world, (alice, "Alice"), (bob, "Bob")).expect("Bob's invite");
    let to_carol = invite(&mut world, (alice, "Alice"), (carol, "Carol")).expect("Carol's invite");
    accept(&mut world, bob, "Alice", to_bob);
    accept(&mut world, carol, "Alice", to_carol);
    let decoded = world.take(alice);
    assert_eq!(importants(&decoded, 3, "Bob").len(), 1);
    assert_eq!(importants(&decoded, 3, "Carol").len(), 1);

    world.take(bob);
    world.take(carol);
    world.ask(alice, request::add_squad_member("Bob"));
    world.ask(alice, request::add_squad_member("Carol"));
    let (alice_id, bob_id) = (world.ids[alice].0, world.ids[bob].0);
    assert_eq!(members(&world, alice), [alice_id, bob_id]);
    assert!(members(&world, carol).is_empty());
    assert_eq!(
        denial(&world.take(alice), id::PROFILE).as_deref(),
        Some("Squads are limited to 2 players.")
    );
    assert_eq!(
        denial(&world.take(carol), id::PROFILE).as_deref(),
        Some("Squads are limited to 2 players.")
    );
    assert_eq!(denial(&world.take(bob), id::PROFILE), None);
    // Bob leaves: Carol's old acceptance does not take the place.
    world.ask(bob, request::leave_squad());
    world.ask(alice, request::add_squad_member("Carol"));
    assert!(members(&world, alice).is_empty());
    assert!(members(&world, carol).is_empty());
    world.assert_clean();
}

/// The other order: Bob is in before Carol accepts. Her acceptance is not
/// passed on, and she is told why nothing happened.
#[test]
fn with_squads_of_two_an_acceptance_after_the_squad_filled_is_refused() {
    for answer_first in [false, true] {
        let mut world = world_of(2, Duration::from_secs(600));
        let alice = world.connect("Alice");
        let bob = world.connect("Bob");
        let carol = world.connect("Carol");
        let to_carol =
            invite(&mut world, (alice, "Alice"), (carol, "Carol")).expect("Carol's invite");
        join(&mut world, (alice, "Alice"), (bob, "Bob"));
        world.take(alice);

        let decoded = if answer_first {
            world.ask(carol, request::answer(to_carol, 0));
            let decoded = world.take(carol);
            world.ask(carol, request::important("Alice", 3, "membership accepted"));
            decoded
        } else {
            accept(&mut world, carol, "Alice", to_carol)
        };
        assert_eq!(
            denial(&decoded, id::PROFILE).as_deref(),
            Some("Squads are limited to 2 players."),
            "answer first: {answer_first}"
        );
        // Told once.
        assert_eq!(denial(&world.take(carol), id::PROFILE), None);
        assert!(importants(&world.take(alice), 3, "Carol").is_empty());
        world.ask(alice, request::add_squad_member("Carol"));
        assert_eq!(members(&world, alice).len(), 2);
        assert!(members(&world, carol).is_empty());
        world.assert_clean();
    }
}

#[test]
fn with_squads_of_two_a_member_leaving_frees_the_place() {
    let mut world = world_of(2, Duration::from_secs(600));
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let carol = world.connect("Carol");
    join(&mut world, (alice, "Alice"), (bob, "Bob"));
    world.ask(bob, request::leave_squad());
    assert!(members(&world, alice).is_empty());

    join(&mut world, (alice, "Alice"), (carol, "Carol"));
    let (alice_id, carol_id) = (world.ids[alice].0, world.ids[carol].0);
    assert_eq!(members(&world, alice), [alice_id, carol_id]);
    assert_eq!(denial(&world.take(alice), id::PROFILE), None);
    world.assert_clean();
}

#[test]
fn with_squads_of_three_a_third_joins_and_a_fourth_is_refused() {
    let mut world = World::new(Duration::from_secs(600));
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let carol = world.connect("Carol");
    let dave = world.connect("Dave");
    join(&mut world, (alice, "Alice"), (bob, "Bob"));
    join(&mut world, (alice, "Alice"), (carol, "Carol"));
    assert_eq!(members(&world, alice).len(), 3);
    assert_eq!(denial(&world.take(alice), id::PROFILE), None);

    assert_eq!(invite(&mut world, (alice, "Alice"), (dave, "Dave")), None);
    assert_eq!(
        denial(&world.take(alice), id::PROFILE).as_deref(),
        Some("Squads are limited to 3 players.")
    );
    world.assert_clean();
}
