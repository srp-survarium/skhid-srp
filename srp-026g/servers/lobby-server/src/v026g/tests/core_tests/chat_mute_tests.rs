use super::*;
use survarium::v026g::lobby::ChatMuteMode::{Hidden, Placeholder};

fn lines(decoded: Vec<Decoded>) -> Vec<(String, u8, String)> {
    decoded
        .into_iter()
        .filter_map(|message| match message {
            Decoded::ChatText {
                sender_name,
                channel,
                body,
                ..
            } => Some((sender_name, channel, body)),
            Decoded::ChatTextIgnored { .. } => {
                panic!("placeholder was dropped by the native ignore filter")
            }
            _ => None,
        })
        .collect()
}

#[test]
fn both_mute_modes_work_in_match_and_team_chat() {
    let mut world = World::new();
    let players: Vec<_> = ["Alice", "Bob", "Carol", "Dave"]
        .into_iter()
        .map(|name| world.connect(name))
        .collect();
    let outsider = world.connect("Erin");
    for &player in &players {
        world.ask(player, request::play(1));
    }
    world.advance(PLAY_TO_MATCH);
    let states: Vec<_> = players
        .iter()
        .map(|&player| sent_to_match(&world.take(player)).unwrap())
        .collect();
    let (match_id, team) = states[0];
    assert!(states.iter().all(|state| state.0 == match_id));
    assert!(states[1..].iter().any(|state| state.1 == team));
    world.take(outsider);
    // Settings are applied during the match; no rejoin or restart is needed.
    for (mode, expected) in [
        (Some(Hidden), None),
        (Some(Placeholder), Some("<muted>")),
        (None, Some("original")),
    ] {
        for &recipient in &players[1..] {
            world.ask(recipient, request::chat_mute("Alice", mode));
            world.take(recipient);
        }
        for channel in [channel::MATCH, channel::TEAM_1 + team] {
            world.ask(players[0], request::chat(match_id, "", channel, "original"));
            for (index, &recipient) in players.iter().enumerate().skip(1) {
                let receives = channel == channel::MATCH || states[index].1 == team;
                let expected: Vec<_> = expected
                    .filter(|_| receives)
                    .map(|body| ("Alice".to_owned(), channel, body.to_owned()))
                    .into_iter()
                    .collect();
                assert_eq!(lines(world.take(recipient)), expected);
            }
            assert!(lines(world.take(outsider)).is_empty());
        }
    }
    world.assert_clean();
}

#[test]
fn mute_mode_is_per_recipient_and_switching_never_leaks_original_text() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let carol = world.connect("Carol");
    for client in [alice, bob, carol] {
        world.ask(client, request::subscribe());
        world.take(client);
    }
    for (mode, expected) in [
        (Some(Placeholder), Some("<muted>")),
        (Some(Hidden), None),
        (Some(Placeholder), Some("<muted>")),
        (None, Some("secret text")),
    ] {
        world.ask(bob, request::chat_mute("Alice", mode));
        world.take(bob);
        for channel in [channel::GENERAL, channel::PRIVATE] {
            let mark = world.mark(bob);
            world.ask(alice, request::chat(2, "Bob", channel, "secret text"));
            let received = lines(world.take(bob));
            assert_eq!(
                received,
                expected
                    .map(|body| ("Alice".into(), channel, body.into()))
                    .into_iter()
                    .collect::<Vec<_>>()
            );
            if mode.is_some() {
                assert!(
                    world
                        .pushed_since(bob, mark)
                        .iter()
                        .all(|(_, body)| !body.windows(11).any(|part| part == b"secret text"))
                );
            }
            let other = lines(world.take(carol));
            assert_eq!(other.len(), usize::from(channel == channel::GENERAL));
            if let Some((_, _, body)) = other.first() {
                assert_eq!(body, "secret text");
            }
        }
    }
    world.ask(bob, request::chat_mute("Bob", Some(Hidden)));
    assert!(
        !world
            .store
            .account(world.ids[bob].0)
            .unwrap()
            .locked()
            .ignored
            .contains(&world.ids[bob].0)
    );
    world.ask(bob, request::chat_mute("Nobody", Some(Placeholder)));
    assert_eq!(
        denial(&world.take(bob), 0x49).as_deref(),
        Some("st_player_not_found")
    );
    world.assert_clean();
}
