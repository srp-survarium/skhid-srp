//! The admin's chat mute (`admin-chat-mute.md`): a muted player's lines
//! reach nobody on any channel, and it is sent the mute's notice instead,
//! on the same channel; lifted or run out, its lines go as before.

use std::time::{SystemTime, UNIX_EPOCH};

use session::{ChatBan, ChatBanEnd};
use survarium::v026g::lobby::rooms::{ROOM_CHAT, ROOM_JOIN_BASE};

use super::rooms_tests::{create, rows, spec};
use super::*;

/// The account chat lines among what the client took: (sender, channel,
/// body).
fn lines(decoded: Vec<Decoded>) -> Vec<(String, u8, String)> {
    decoded
        .into_iter()
        .filter_map(|message| match message {
            Decoded::ChatText {
                sender_type: 5,
                sender_name,
                channel,
                body,
                ..
            } => Some((sender_name, channel, body)),
            _ => None,
        })
        .collect()
}

fn line(sender: &str, channel: u8, body: &str) -> Vec<(String, u8, String)> {
    vec![(sender.to_owned(), channel, body.to_owned())]
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn mute(world: &World, index: usize, ends: ChatBanEnd) {
    let account_id = world.ids[index].0;
    let ban = ChatBan {
        ends,
        reason: "spam".to_owned(),
    };
    assert!(world.store.mute_chat(account_id, ban));
}

#[test]
fn a_muted_player_reaches_nobody_in_the_lobby_and_is_told_on_each_channel() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let carol = world.connect("Carol");
    for client in [alice, bob, carol] {
        world.ask(client, request::subscribe());
    }
    // Alice and Bob are in one room, its chat tab up, and a squad.
    let key = create(&mut world, alice, spec(6), None);
    rows(&mut world, bob, ROOM_JOIN_BASE + key, 1);
    world.advance(Duration::from_secs(1));
    for client in [alice, bob] {
        world.ask(client, request::ranking(ROOM_CHAT, 1));
    }
    let (alice_id, bob_id) = (world.ids[alice].0, world.ids[bob].0);
    world.store.squads().invite_accepted(alice_id, bob_id);
    world.store.squads().add(alice_id, bob_id).unwrap();
    for client in [alice, bob, carol] {
        world.take(client);
    }
    let channels = [
        (channel::GENERAL, ""),
        (channel::PRIVATE, "Bob"),
        (channel::SQUAD, ""),
        (channel::ROOM, ""),
    ];
    // Not muted: Bob hears each line.
    for (channel, receiver) in channels {
        world.ask(alice, request::chat(0, receiver, channel, "hello"));
        assert_eq!(lines(world.take(bob)), line("Alice", channel, "hello"));
        assert!(lines(world.take(alice)).is_empty());
    }
    world.take(carol);

    mute(&world, alice, ChatBanEnd::Never);
    for (channel, receiver) in channels {
        world.ask(alice, request::chat(0, receiver, channel, "hello"));
        assert!(lines(world.take(bob)).is_empty(), "channel {channel}");
        assert!(lines(world.take(carol)).is_empty(), "channel {channel}");
        assert_eq!(
            lines(world.take(alice)),
            line("Alice", channel, "muted for spam")
        );
    }
    // The room keeps none of them: Bob's tab, asked again, has no "hello"
    // past the ones before the mute.
    world.ask(bob, request::ranking(ROOM_CHAT, 1));
    let kept = lines(world.take(bob));
    assert_eq!(
        kept.iter()
            .filter(|(_, _, body)| body == "muted for spam")
            .count(),
        0
    );
    assert_eq!(
        kept.iter().filter(|(_, _, body)| body == "hello").count(),
        1
    );

    // Timed: the notice says until when.
    let until = unix_now() + 3 * 3600;
    mute(&world, alice, ChatBanEnd::At(until));
    world.ask(alice, request::chat(0, "", channel::GENERAL, "hello"));
    let notice = format!(
        "muted until {} {} UTC: spam",
        session::utc::date(until),
        session::utc::clock(until)
    );
    assert_eq!(
        lines(world.take(alice)),
        line("Alice", channel::GENERAL, &notice)
    );
    assert!(lines(world.take(bob)).is_empty());

    // Run out: the lines go again, with no notice.
    mute(&world, alice, ChatBanEnd::At(unix_now() - 1));
    world.ask(alice, request::chat(0, "", channel::GENERAL, "back"));
    assert_eq!(
        lines(world.take(bob)),
        line("Alice", channel::GENERAL, "back")
    );
    assert!(lines(world.take(alice)).is_empty());
    world.take(carol);

    // Lifted: the same.
    mute(&world, alice, ChatBanEnd::Never);
    assert!(world.store.unmute_chat(alice_id));
    assert!(!world.store.unmute_chat(alice_id));
    world.ask(alice, request::chat(0, "", channel::GENERAL, "again"));
    assert_eq!(
        lines(world.take(carol)),
        line("Alice", channel::GENERAL, "again")
    );
    assert!(lines(world.take(alice)).is_empty());
    world.assert_clean();
}

#[test]
fn a_muted_player_reaches_nobody_in_its_match_and_team_chat() {
    let mut world = World::new();
    let players: Vec<_> = ["Alice", "Bob", "Carol", "Dave"]
        .into_iter()
        .map(|name| world.connect(name))
        .collect();
    for &player in &players {
        world.ask(player, request::play(1));
    }
    world.advance(PLAY_TO_MATCH);
    let states: Vec<_> = players
        .iter()
        .map(|&player| sent_to_match(&world.take(player)).unwrap())
        .collect();
    let (match_id, team) = states[0];
    assert!(states[1..].iter().any(|state| state.1 == team));
    mute(&world, players[0], ChatBanEnd::Never);
    for channel in [channel::MATCH, channel::TEAM_1 + team] {
        world.ask(players[0], request::chat(match_id, "", channel, "original"));
        for &other in &players[1..] {
            assert!(lines(world.take(other)).is_empty(), "channel {channel}");
        }
        assert_eq!(
            lines(world.take(players[0])),
            line("Alice", channel, "muted for spam")
        );
    }
    world.assert_clean();
}
