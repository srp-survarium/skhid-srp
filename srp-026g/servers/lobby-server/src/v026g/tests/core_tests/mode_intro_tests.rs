//! Real lobby requests: only the requesting account, three distinct matches, retries and reconnects.
use super::*;
use session::v026g::mode_intro::Mode;

/// Rudnya's Blackout map (`blackout-mode-1.json`).
fn blackout_map() -> u8 {
    maps::find(1, maps::weather::DAY, maps::GameMode::Blackout).unwrap().id
}

fn ask_intro(world: &mut World, player: usize, body: &str) -> Vec<String> {
    world.take(player);
    world.ask(player, request::chat(0, "", channel::ROOM, body));
    world
        .take(player)
        .into_iter()
        .filter_map(|message| match message {
            Decoded::ChatText {
                sender_id: 0,
                sender_name,
                channel: channel::ROOM,
                body,
                ..
            } if sender_name == "System" => Some(body),
            _ => None,
        })
        .collect()
}

#[test]
fn mode_introduction_is_private_and_counts_distinct_matches() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let carol = world.connect("Carol");
    assert!(ask_intro(&mut world, alice, "#mode-intro:blackout:abc").is_empty());
    // Onboarding is a private server notice even for a player muted by the admin.
    assert!(world.store.mute_chat(
        world.ids[alice].0,
        session::Ban {
            until: None,
            reason: "spam".to_owned(),
        }
    ));
    for match_number in 1..=4 {
        let (id, team) = world.match_of(alice, bob);
        let running = world
            .core
            .matches
            .values_mut()
            .find(|running| running.ticket.match_id == id)
            .unwrap();
        running.map = survarium::v026g::twist::Played::plain(maps::MatchMap::of(61).unwrap());
        assert!(ask_intro(&mut world, alice, "#mode-intro:blackout:abc").is_empty());
        world
            .core
            .matches
            .values_mut()
            .find(|running| running.ticket.match_id == id)
            .unwrap()
            .map = survarium::v026g::twist::Played::plain(maps::MatchMap::of(blackout_map()).unwrap());
        assert!(ask_intro(&mut world, alice, "#mode-intro:scarcity:abc").is_empty());
        assert!(ask_intro(&mut world, alice, "#mode-intro:blackout:bad-token").is_empty());
        assert!(ask_intro(&mut world, carol, "#mode-intro:blackout:abc").is_empty());
        let answer = format!(
            "#mode-intro:blackout:abc:{}",
            if match_number <= 3 { "show" } else { "done" }
        );
        for _ in 0..2 {
            assert_eq!(
                ask_intro(&mut world, alice, "#mode-intro:blackout:abc"),
                std::slice::from_ref(&answer)
            );
        }
        // A reconstructed HUD/reconnect does not spend another allowance or show again.
        assert_eq!(
            ask_intro(&mut world, alice, "#mode-intro:blackout:def"),
            ["#mode-intro:blackout:def:done"]
        );
        assert!(
            !world
                .take(bob)
                .iter()
                .any(|message| matches!(message, Decoded::ChatText { .. }))
        );
        assert!(
            !world
                .take(carol)
                .iter()
                .any(|message| matches!(message, Decoded::ChatText { .. }))
        );
        assert_eq!(
            world
                .account(alice)
                .mode_introductions
                .rows()
                .collect::<Vec<_>>(),
            [(Mode::Blackout, match_number.min(3))]
        );
        assert!(
            world
                .account(bob)
                .mode_introductions
                .rows()
                .next()
                .is_none()
        );
        world.end_match(id, team);
        world.advance(Duration::from_secs(1));
        world.take(alice);
        world.take(bob);
    }
    world.assert_clean();
}
