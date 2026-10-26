//! What a player counts for when the sides of a match are made
//! (`session::v026g::balance`), and squads switched off (`SRP_SQUADS`).

use super::*;
use crate::v026g::DEFAULT_SQUADS_OFF_TEXT;
use session::STRENGTH_SCALE;
use session::v026g::balance::{Balance, UNIT, Weights};
use survarium::v026g::lobby::requests::id;

/// Five players press Play, which puts them into one room, and get its
/// match; the side of each, by client.
fn sides_of_five(world: &mut World, players: &[usize]) -> Vec<u8> {
    for &player in players {
        world.ask(player, request::play(1));
    }
    world.advance(PLAY_TO_MATCH);
    players
        .iter()
        .map(|&player| sent_to_match(&world.take(player)).expect("a match").1)
        .collect()
}

fn five(world: &mut World) -> Vec<usize> {
    ["Alice", "Bob", "Carol", "Dave", "Erin"]
        .into_iter()
        .map(|name| world.connect(name))
        .collect()
}

#[test]
fn with_an_odd_count_the_stronger_players_are_the_side_of_fewer() {
    let settings = Settings::local("127.0.0.1:0");
    assert_eq!(settings.balance, Weights::default());

    let mut world = World::new();
    let players = five(&mut world);
    // Carol and Erin have won much, over many matches: each counts for
    // six gear levels more than the others, who are new.
    for strong in [players[2], players[4]] {
        world.change(strong, |account| {
            account.balance = Balance {
                rating: 3 * UNIT / 2,
                matches: 1000,
            };
        });
    }
    let sides = sides_of_five(&mut world, &players);
    assert_eq!(sides[2], sides[4], "{sides:?}");
    let with_them = sides.iter().filter(|&&side| side == sides[2]).count();
    assert_eq!(with_them, 2, "{sides:?}");
    world.assert_clean();
}

#[test]
fn a_level_of_the_account_counts_too() {
    let mut world = World::new();
    let players = five(&mut world);
    // Bob and Dave are of level 4 (3,500 experience), the others of none.
    for veteran in [players[1], players[3]] {
        world.change(veteran, |account| account.exp_total = 3_500);
    }
    assert_eq!(world.account(players[1]).level(), 4);
    let sides = sides_of_five(&mut world, &players);
    assert_eq!(sides[1], sides[3], "{sides:?}");
    assert_eq!(
        sides.iter().filter(|&&side| side == sides[1]).count(),
        2,
        "{sides:?}"
    );
}

/// Alice (level 2, a point of balance rating over five matches) and Bob
/// (five points no match stands behind) in a match: what each counts for
/// in it, beside being there.
fn strengths_in_a_match(world: &mut World) -> [i32; 2] {
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    world.change(alice, |account| {
        account.exp_total = 1_250;
        account.balance = Balance {
            rating: UNIT,
            matches: 5,
        };
    });
    world.change(bob, |account| {
        account.balance = Balance {
            rating: 5 * UNIT,
            matches: 0,
        };
    });
    let (ticket, _) = world.match_of(alice, bob);
    let running = world
        .core
        .matches
        .values()
        .find(|running| running.ticket.match_id == ticket)
        .unwrap();
    [alice, bob].map(|player| {
        let place = running
            .seats
            .iter()
            .position(|(occupant, _)| occupant.session() == Some(world.ids[player].1))
            .unwrap();
        running.strengths[place]
    })
}

#[test]
fn a_player_counts_for_its_gear_its_level_and_its_trusted_rating() {
    // Gear level 0; 2 levels at 1.5; half of 4 for the point. A rating no
    // match stands behind counts for nothing.
    let mut world = World::new();
    assert_eq!(
        strengths_in_a_match(&mut world),
        [3 * STRENGTH_SCALE + 2 * STRENGTH_SCALE, 0]
    );

    // The weights are settings: with none, only the gear level is left.
    let mut world = World::new();
    world.core.settings.balance = Weights {
        player: 0.0,
        player_level: 0.0,
        rating: 0.0,
        ..Weights::default()
    };
    assert_eq!(strengths_in_a_match(&mut world), [0, 0]);
}

#[test]
fn squads_are_on_unless_the_server_is_told_otherwise() {
    let settings = Settings::local("127.0.0.1:0");
    assert!(settings.squads);
    assert_eq!(settings.squads_off_text, DEFAULT_SQUADS_OFF_TEXT);
}

#[test]
fn with_squads_off_an_invite_is_refused_and_its_sender_told() {
    let mut world = World::new();
    world.core.settings.squads = false;
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    world.take(alice);
    world.take(bob);

    world.ask(alice, request::important("Bob", 2, "invite"));
    // Alice is told, in the message box of `84 49`; Bob gets nothing.
    assert_eq!(
        denial(&world.take(alice), id::PROFILE).as_deref(),
        Some(DEFAULT_SQUADS_OFF_TEXT)
    );
    assert!(importants(&world.take(bob), 2, "Alice").is_empty());

    // A client that says it was invited and adds the player all the same.
    world.ask(bob, request::important("Alice", 3, "membership accepted"));
    assert!(importants(&world.take(alice), 3, "Bob").is_empty());
    world.ask(alice, request::add_squad_member("Bob"));
    assert!(world.store.squads().of(world.ids[alice].0).is_none());
    assert!(world.store.squads().of(world.ids[bob].0).is_none());
    // Nothing was said for the client's own messages.
    assert_eq!(denial(&world.take(alice), id::PROFILE), None);
    assert_eq!(denial(&world.take(bob), id::PROFILE), None);

    // The text is a setting: a key of the game, or a sentence.
    world.core.settings.squads_off_text = "No squads here.".to_owned();
    world.ask(bob, request::important("Alice", 2, "invite"));
    assert_eq!(
        denial(&world.take(bob), id::PROFILE).as_deref(),
        Some("No squads here.")
    );
    world.assert_clean();
}

#[test]
fn with_squads_off_two_friends_play_as_players_alone() {
    let mut world = World::new();
    world.core.settings.squads = false;
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    world.ask(alice, request::important("Bob", 2, "invite"));
    world.ask(alice, request::add_squad_member("Bob"));
    world.ask(alice, request::play(1));
    world.ask(bob, request::play(1));
    world.advance(PLAY_TO_MATCH);
    // Two players, two sides: a squad of two is one side.
    let a = sent_to_match(&world.take(alice)).expect("a match").1;
    let b = sent_to_match(&world.take(bob)).expect("a match").1;
    assert_ne!(a, b);
}
