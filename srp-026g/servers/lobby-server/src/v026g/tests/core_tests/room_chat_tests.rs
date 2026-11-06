//! A room's chat (`room_chat`): its members hear it once their tab is up,
//! nobody else does; a newcomer gets its last lines; who comes and goes is
//! not said in it; a match's all-chat is kept in it, its hud given its last
//! lines once; another match's is not; leaving, a kick or a
//! switch ends it; a queue hears nothing.

use survarium::v026g::lobby::rooms::{
    ROOM_CHAT, ROOM_JOIN_BASE, ROOM_LEAVE, ROOM_PIN_BASE, ROOM_QUEUE_BASE, ROOM_STATE, kick_start,
};

use super::pvb_tests::{play_vs_bots, with_gear, world_with};
use super::rooms_tests::{create, rows, spec};
use super::*;

/// The room chat lines among `decoded`: (sender, body), the room's own
/// lines with no sender.
fn room_lines(decoded: &[Decoded]) -> Vec<(String, String)> {
    decoded
        .iter()
        .filter_map(|message| match message {
            Decoded::ChatText {
                sender_type: 5,
                sender_name,
                sender_clan_tag: Some(_),
                channel: channel::ROOM,
                body,
                ..
            } => Some((sender_name.clone(), body.clone())),
            _ => None,
        })
        .collect()
}

fn heard(world: &mut World, index: usize) -> Vec<(String, String)> {
    room_lines(&world.take(index))
}

/// The movie puts up its "Room Chat" tab: the lines it is given.
fn listen(world: &mut World, index: usize) -> Vec<(String, String)> {
    world.take(index);
    world.ask(index, request::ranking(ROOM_CHAT, 1));
    heard(world, index)
}

fn say(world: &mut World, index: usize, body: &str) {
    world.ask(index, request::chat(0, "", channel::ROOM, body));
}

fn line(sender: &str, body: &str) -> Vec<(String, String)> {
    vec![(sender.to_owned(), body.to_owned())]
}

#[test]
fn room_chat_and_history_use_each_recipients_mute_mode() {
    use survarium::v026g::lobby::ChatMuteMode::{Hidden, Placeholder};
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let carol = world.connect("Carol");
    let key = create(&mut world, alice, spec(6), None);
    for client in [bob, carol] {
        rows(&mut world, client, ROOM_JOIN_BASE + key, 1);
    }
    world.advance(Duration::from_secs(1));
    for client in [alice, bob, carol] {
        listen(&mut world, client);
    }
    world.ask(bob, request::chat_mute("Alice", Some(Placeholder)));
    world.ask(carol, request::chat_mute("Alice", Some(Hidden)));
    say(&mut world, alice, "original");
    assert_eq!(heard(&mut world, bob), line("Alice", "<muted>"));
    assert!(heard(&mut world, carol).is_empty());
    let from_alice = |lines: Vec<(String, String)>| {
        lines
            .into_iter()
            .filter(|(name, _)| name == "Alice")
            .collect::<Vec<_>>()
    };
    assert_eq!(
        from_alice(listen(&mut world, bob)),
        line("Alice", "<muted>")
    );
    assert!(from_alice(listen(&mut world, carol)).is_empty());
    world.ask(bob, request::chat_mute("Alice", None));
    assert_eq!(
        from_alice(listen(&mut world, bob)),
        line("Alice", "original")
    );
    world.assert_clean();
}

/// Two rooms, each with its own chat; a player in no room hears neither
/// and is heard by none.
#[test]
fn each_room_hears_its_own_chat_only() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let carol = world.connect("Carol");
    let dave = world.connect("Dave");
    let erin = world.connect("Erin");
    let first = create(&mut world, alice, spec(6), None);
    let second = create(&mut world, carol, spec(6), None);
    // Alone in its room: nothing said.
    world.advance(Duration::from_secs(1));
    assert!(listen(&mut world, alice).is_empty());
    assert!(listen(&mut world, carol).is_empty());
    rows(&mut world, bob, ROOM_JOIN_BASE + first, 1);
    rows(&mut world, dave, ROOM_JOIN_BASE + second, 1);

    // Who came is not said.
    world.advance(Duration::from_secs(1));
    assert!(heard(&mut world, alice).is_empty());
    assert!(heard(&mut world, carol).is_empty());
    assert!(listen(&mut world, bob).is_empty());
    assert!(listen(&mut world, dave).is_empty());
    assert!(listen(&mut world, erin).is_empty());

    // A member's line reaches the other members only (its own game shows
    // it already).
    say(&mut world, alice, "hi");
    assert_eq!(heard(&mut world, bob), line("Alice", "hi"));
    for client in [alice, carol, dave, erin] {
        assert!(heard(&mut world, client).is_empty());
    }
    say(&mut world, dave, "yo");
    assert_eq!(heard(&mut world, carol), line("Dave", "yo"));
    for client in [alice, bob, dave, erin] {
        assert!(heard(&mut world, client).is_empty());
    }
    // Out of a room: nobody hears it.
    say(&mut world, erin, "anyone?");
    for client in [alice, bob, carol, dave] {
        assert!(heard(&mut world, client).is_empty());
    }
    world.assert_clean();
}

/// A newcomer's tab gets the room's last lines (but those of a player it
/// ignores); a game that never puts up the tab is sent no line.
#[test]
fn a_newcomer_gets_the_last_lines_and_a_game_without_the_tab_none() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let carol = world.connect("Carol");
    let key = create(&mut world, alice, spec(6), None);
    world.advance(Duration::from_secs(1));
    assert!(listen(&mut world, alice).is_empty());
    // Bob's game has no "Room Chat" tab: it never asks.
    rows(&mut world, bob, ROOM_JOIN_BASE + key, 1);
    world.advance(Duration::from_secs(1));
    say(&mut world, alice, "one");
    say(&mut world, bob, "two");
    assert_eq!(heard(&mut world, alice), line("Bob", "two"));
    assert!(heard(&mut world, bob).is_empty());

    let bob_id = world.ids[bob].0;
    world.ask(carol, request::ignore(bob_id));
    rows(&mut world, carol, ROOM_JOIN_BASE + key, 1);
    assert_eq!(listen(&mut world, carol), line("Alice", "one"));
    world.advance(Duration::from_secs(1));
    assert!(heard(&mut world, carol).is_empty());
    assert!(heard(&mut world, alice).is_empty());
    // The room's chat is kept with the room.
    assert_eq!(world.core.rooms.get(key).unwrap().chat.len(), 2);
    world.assert_clean();
}

/// Leaving, a kick and a switch to another room each end the member's
/// chat, and are not said in the room it left; in the new room it hears
/// once its tab asks again. A room gone takes its chat.
#[test]
fn leaving_a_kick_or_a_switch_ends_a_members_chat() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let carol = world.connect("Carol");
    let dave = world.connect("Dave");
    let first = create(&mut world, alice, spec(8), Some(1234));
    for client in [bob, carol] {
        rows(&mut world, client, ROOM_PIN_BASE + 1234, 1);
        rows(&mut world, client, ROOM_JOIN_BASE + first, 1);
    }
    let second = create(&mut world, dave, spec(8), None);
    world.advance(Duration::from_secs(1));
    for client in [alice, bob, carol, dave] {
        listen(&mut world, client);
    }

    // Bob leaves: not said, and he hears the room no more.
    rows(&mut world, bob, ROOM_LEAVE, 1);
    world.advance(Duration::from_secs(1));
    assert!(heard(&mut world, alice).is_empty());
    assert!(heard(&mut world, carol).is_empty());
    say(&mut world, alice, "bye bob");
    assert_eq!(heard(&mut world, carol), line("Alice", "bye bob"));
    assert!(heard(&mut world, bob).is_empty());

    // Alice takes Carol out (place 1 of the list).
    let state = rows(&mut world, alice, ROOM_STATE, 1);
    let serial: u32 = state[0].split(';').nth(12).unwrap().parse().unwrap();
    rows(&mut world, alice, kick_start(1, serial), 1);
    world.advance(Duration::from_secs(1));
    assert!(heard(&mut world, alice).is_empty());
    say(&mut world, alice, "out");
    assert!(heard(&mut world, carol).is_empty());

    // Alice switches to Dave's room: hers is gone (nobody left in it), and
    // she hears Dave's once her tab asks for it.
    rows(&mut world, alice, ROOM_JOIN_BASE + second, 1);
    assert!(world.core.rooms.get(first).is_none());
    world.advance(Duration::from_secs(1));
    assert!(heard(&mut world, dave).is_empty());
    say(&mut world, dave, "welcome");
    assert!(heard(&mut world, alice).is_empty());
    assert_eq!(listen(&mut world, alice), line("Dave", "welcome"));
    say(&mut world, dave, "again");
    assert_eq!(heard(&mut world, alice), line("Dave", "again"));
    world.assert_clean();
}

/// Members playing the room's match are sent no line as it is said (their
/// game's match chat is its own); one who comes during the match talks with
/// whoever is in the lobby, and the players of the match get what was said,
/// and the match's all-chat (not its team lines), when their tab asks again
/// after it.
#[test]
fn a_late_joiner_talks_and_the_match_gets_it_after() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let carol = world.connect("Carol");
    let key = create(&mut world, alice, spec(6), None);
    rows(&mut world, bob, ROOM_JOIN_BASE + key, 1);
    world.advance(Duration::from_secs(1));
    assert!(listen(&mut world, alice).is_empty());
    assert!(listen(&mut world, bob).is_empty());
    world.advance(Duration::from_secs(31));
    let (match_id, alice_team) = sent_to_match(&world.take(alice)).expect("the match");
    world.take(bob);

    // The match's all-chat is kept in the room's chat; its team lines not.
    world.ask(alice, request::chat(match_id, "", channel::MATCH, "gl"));
    world.ask(
        alice,
        request::chat(match_id, "", channel::TEAM_1 + alice_team, "go left"),
    );
    world.take(alice);
    world.take(bob);

    // Carol comes while the match is played: not said, nor to the match.
    rows(&mut world, carol, ROOM_JOIN_BASE + key, 1);
    world.advance(Duration::from_secs(1));
    assert!(heard(&mut world, alice).is_empty());
    assert!(heard(&mut world, bob).is_empty());
    assert_eq!(listen(&mut world, carol), line("Alice", "gl"));
    // In the lobby, she hears the match's all-chat as it is said.
    world.ask(bob, request::chat(match_id, "", channel::MATCH, "ty"));
    world.take(alice);
    assert_eq!(heard(&mut world, carol), line("Bob", "ty"));
    say(&mut world, carol, "late, sorry");
    assert!(heard(&mut world, alice).is_empty());
    assert!(heard(&mut world, bob).is_empty());
    // A tab asking during the match is given nothing yet.
    assert!(listen(&mut world, alice).is_empty());

    // After it, the tab asks again (the results screen) and gets the lines.
    world.end_match(match_id, 0);
    world.advance(Duration::from_secs(1));
    let after = [
        ("Alice".to_owned(), "gl".to_owned()),
        ("Bob".to_owned(), "ty".to_owned()),
        ("Carol".to_owned(), "late, sorry".to_owned()),
    ];
    assert_eq!(listen(&mut world, alice), after);
    assert_eq!(listen(&mut world, bob), after);
    say(&mut world, alice, "gg");
    assert_eq!(heard(&mut world, bob), line("Alice", "gg"));
}

/// Who waits in a full room's queue is no member: it hears nothing and
/// gets no lines, until it is put in.
#[test]
fn a_rooms_queue_hears_nothing_until_it_is_put_in() {
    let mut world = World::new();
    let owner = world.connect("Owner");
    let key = create(&mut world, owner, spec(6), None);
    let first = world.connect("Member1");
    rows(&mut world, first, ROOM_JOIN_BASE + key, 1);
    for n in 2..12 {
        let member = world.connect(&format!("Member{n}"));
        rows(&mut world, member, ROOM_JOIN_BASE + key, 1);
    }
    world.advance(Duration::from_secs(1));
    listen(&mut world, owner);
    say(&mut world, owner, "full");
    let waiting = world.connect("Waiting");
    rows(&mut world, waiting, ROOM_QUEUE_BASE + key, 1);
    assert!(listen(&mut world, waiting).is_empty());
    say(&mut world, owner, "still full");
    world.advance(Duration::from_secs(1));
    assert!(heard(&mut world, waiting).is_empty());

    // A place frees: the first of the queue is put in, and is a member.
    rows(&mut world, first, ROOM_LEAVE, 1);
    world.advance(Duration::from_secs(1));
    assert!(heard(&mut world, owner).is_empty());
    assert_eq!(world.core.rooms.key_of(world.ids[waiting].0), Some(key));
    assert_eq!(
        listen(&mut world, waiting),
        [
            ("Owner".to_owned(), "full".to_owned()),
            ("Owner".to_owned(), "still full".to_owned())
        ]
    );
    say(&mut world, owner, "welcome");
    assert_eq!(heard(&mut world, waiting), line("Owner", "welcome"));
}

/// Play puts a player into an open room: it is not said there, and the
/// player is in its chat.
#[test]
fn play_into_a_room_is_a_member_of_its_chat() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let key = create(
        &mut world,
        alice,
        survarium::v026g::lobby::rooms::RoomSpec {
            max_level: 10,
            ..spec(8)
        },
        None,
    );
    world.advance(Duration::from_secs(1));
    assert!(listen(&mut world, alice).is_empty());
    world.ask(bob, request::play(1));
    world.advance(Duration::from_secs(1));
    assert_eq!(world.core.rooms.key_of(world.ids[bob].0), Some(key));
    assert!(heard(&mut world, alice).is_empty());
    assert!(listen(&mut world, bob).is_empty());
    say(&mut world, bob, "hi");
    assert_eq!(heard(&mut world, alice), line("Bob", "hi"));
}

/// A room disbanded (a maintenance near) takes its chat: nothing is said
/// in it, nobody hears it, and a tab asking again gets no lines.
#[test]
fn a_disbanded_room_takes_its_chat() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let key = create(&mut world, alice, spec(6), None);
    rows(&mut world, bob, ROOM_JOIN_BASE + key, 1);
    world.advance(Duration::from_secs(1));
    listen(&mut world, alice);
    listen(&mut world, bob);
    say(&mut world, alice, "soon");
    heard(&mut world, bob);
    world
        .store
        .set_maintenance(Some(session::v026g::quests::now() + 20 * 60));
    world.advance(Duration::from_secs(1));
    assert!(world.core.rooms.get(key).is_none());
    assert!(heard(&mut world, alice).is_empty());
    assert!(heard(&mut world, bob).is_empty());
    say(&mut world, alice, "gone?");
    assert!(heard(&mut world, bob).is_empty());
    assert!(listen(&mut world, bob).is_empty());
}

/// The hud's ask for its room's last lines (`RoomHistory.as`): the room's
/// lines it is given, and the server's "System" answers.
fn ask_history(
    world: &mut World,
    index: usize,
    nonce: &str,
) -> (Vec<(String, String)>, Vec<String>) {
    world.take(index);
    world.ask(
        index,
        request::chat(0, "", channel::ROOM, &format!("#room-chat:{nonce}")),
    );
    let (system, lines): (Vec<_>, Vec<_>) = room_lines(&world.take(index))
        .into_iter()
        .partition(|(sender, _)| sender == "System");
    (lines, system.into_iter().map(|(_, body)| body).collect())
}

/// The hud asks once its match chat is up: a player of its room's match is
/// given the room's last lines (as many as the hud's log shows, but an
/// ignored sender's), then "done"; again (a retry, a reconnect) "done"
/// only. In the lobby the ask is a line like any other.
#[test]
fn a_match_starts_with_the_rooms_last_lines() {
    let ask = |world: &mut World, index: usize| ask_history(world, index, "ab12").0;
    let done = || vec!["#room-chat:ab12:done".to_owned()];
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let carol = world.connect("Carol");
    let erin = world.connect("Erin");
    let key = create(&mut world, alice, spec(6), None);
    for client in [bob, carol] {
        rows(&mut world, client, ROOM_JOIN_BASE + key, 1);
    }
    world.advance(Duration::from_secs(1));
    for client in [alice, bob, carol] {
        listen(&mut world, client);
    }
    // In the lobby the ask is no request: a line typed so is said.
    assert_eq!(ask_history(&mut world, alice, "ab12"), (vec![], vec![]));
    assert_eq!(heard(&mut world, bob), line("Alice", "#room-chat:ab12"));
    assert_eq!(ask_history(&mut world, erin, "ab12"), (vec![], vec![]));
    let bob_id = world.ids[bob].0;
    world.ask(carol, request::ignore(bob_id));
    for n in 0..12 {
        // One line a second: within the flood allowance.
        say(
            &mut world,
            if n == 11 { bob } else { alice },
            &format!("line {n}"),
        );
        world.advance(Duration::from_secs(1));
    }

    world.advance(Duration::from_secs(31));
    sent_to_match(&world.take(alice)).expect("the match");
    let last = |from: usize| -> Vec<(String, String)> {
        (from..11)
            .map(|n| ("Alice".to_owned(), format!("line {n}")))
            .collect()
    };
    let mut alices = last(2);
    alices.push(("Bob".to_owned(), "line 11".to_owned()));
    assert_eq!(ask_history(&mut world, alice, "ab12"), (alices, done()));
    // A retry, or a new hud's ask in the same match: "done" only.
    assert_eq!(ask_history(&mut world, alice, "ab12"), (vec![], done()));
    assert_eq!(
        ask_history(&mut world, alice, "cd34"),
        (vec![], vec!["#room-chat:cd34:done".to_owned()])
    );
    // Carol ignores Bob: her ten are Alice's.
    assert_eq!(ask(&mut world, carol), last(1));
    // Asking is never charged to the chat's allowance: a hud whose player
    // just used it up is answered.
    for n in 0..20 {
        say(&mut world, bob, &format!("flood {n}"));
    }
    assert_eq!(ask_history(&mut world, bob, "ab12").1, done());
}

/// A member in another match than its room's (one against bots, started
/// from the room's vote): its all-chat is not kept in the room, and its
/// hud is given none of the room's lines.
#[test]
fn another_match_is_not_the_rooms() {
    let mut world = world_with(5);
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    with_gear(&mut world, alice, 5);
    let key = create(&mut world, alice, spec(6), None);
    rows(&mut world, bob, ROOM_JOIN_BASE + key, 1);
    world.advance(Duration::from_secs(1));
    listen(&mut world, bob);
    say(&mut world, bob, "hi");
    let (match_id, _) = play_vs_bots(&mut world, alice, 8, 61);
    assert_eq!(world.core.rooms.key_of(world.ids[alice].0), Some(key));
    world.ask(
        alice,
        request::chat(match_id, "", channel::MATCH, "lol bots"),
    );
    assert!(heard(&mut world, bob).is_empty());
    assert_eq!(
        ask_history(&mut world, alice, "ab12"),
        (vec![], vec!["#room-chat:ab12:done".to_owned()])
    );
    world.end_match(match_id, 0);
    world.advance(Duration::from_secs(1));
    assert_eq!(listen(&mut world, alice), line("Bob", "hi"));
}
