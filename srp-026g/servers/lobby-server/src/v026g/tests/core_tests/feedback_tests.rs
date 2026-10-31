//! What a player is told when the server can not do what it asked
//! (`social.md` §9): the game's own "Player not found." where the client
//! can show it, nothing where it can not, one general chat room for every
//! language, and a results screen whose rows are always answered.
//!
//! The expected bytes are written out from the client's readers
//! (`84`: 0x274F14, `A2`: 0x26B6F0), never taken from the writers.

use survarium::v026g::lobby::client_rules::{ChatSpamFilter, denial_is_shown};

use super::*;

/// `84 49 00 <string>`: the message box with `st_player_not_found`.
fn not_found_bytes() -> (u8, Vec<u8>) {
    let mut body = vec![0x49, 0x00, 19];
    body.extend_from_slice(b"st_player_not_found");
    (0x84, body)
}

impl World {
    /// How many messages the lobby has pushed to the client so far.
    fn mark(&self, index: usize) -> usize {
        self.core.links[&self.clients[index].0].pushed.len()
    }

    /// The messages pushed to the client since `mark`: `(id, body)`. The
    /// online counts (0x9F), repeated every few seconds, are left out.
    fn pushed_since(&self, index: usize, mark: usize) -> Vec<(u8, Vec<u8>)> {
        self.core.links[&self.clients[index].0].pushed[mark..]
            .iter()
            .filter(|message| message.id_byte() != 0x9F)
            .map(|message| (message.id_byte(), message.body().to_vec()))
            .collect()
    }

    /// The refusals pushed to the client since `mark`.
    fn denials_since(&self, index: usize, mark: usize) -> Vec<(u8, Vec<u8>)> {
        self.pushed_since(index, mark)
            .into_iter()
            .filter(|(id, _)| *id == 0x84)
            .collect()
    }
}

/// `4F` as a client of a language sends it (0x26C320): the general room of
/// its language in slot 1, the language byte last.
fn subscribe_as(general_room: u32, language: u8) -> (u8, Vec<u8>) {
    let ids: [u32; 10] = [0, general_room, u32::MAX, 0, 1, 0, 0, 0, 0, 0];
    let mut body: Vec<u8> = ids.iter().flat_map(|id| id.to_le_bytes()).collect();
    body.push(language);
    (0x4F, body)
}

/// `A2` for a line of a player with no clan (0x26B6F0).
fn chat_bytes(sender_id: u32, sender_name: &str, channel: u8, text: &str) -> (u8, Vec<u8>) {
    let mut body = vec![5];
    body.extend(sender_id.to_le_bytes());
    body.push(sender_name.len() as u8);
    body.extend_from_slice(sender_name.as_bytes());
    body.push(0);
    body.push(channel);
    body.push(text.len() as u8);
    body.extend_from_slice(text.as_bytes());
    (0xA2, body)
}

fn chat_lines(pushed: &[(u8, Vec<u8>)]) -> Vec<(u8, Vec<u8>)> {
    pushed
        .iter()
        .filter(|(id, _)| *id == 0xA2)
        .cloned()
        .collect()
}

#[test]
fn a_private_line_for_nobody_in_the_lobby_is_answered_with_player_not_found() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    // An account that is not in the lobby.
    signed_in(world.store, "Dave");
    world.take(alice);

    // A name that is no account.
    let mark = world.mark(alice);
    world.ask(alice, request::chat(0, "Nobody", channel::PRIVATE, "hi"));
    assert_eq!(world.denials_since(alice, mark), [not_found_bytes()]);
    // The client shows it: a refusal of 0x49 opens the message box, and
    // the model read it as the client does.
    assert!(denial_is_shown(0x49));
    assert_eq!(
        denial(&world.take(alice), 0x49).as_deref(),
        Some("st_player_not_found")
    );

    // A player who is offline: the same text (the game has no other).
    let mark = world.mark(alice);
    world.ask(alice, request::chat(0, "dave", channel::PRIVATE, "hi"));
    assert_eq!(world.denials_since(alice, mark), [not_found_bytes()]);
    // More lines to the same name do not open more windows at once...
    let mark = world.mark(alice);
    world.ask(alice, request::chat(0, "Dave", channel::PRIVATE, "there?"));
    world.ask(alice, request::chat(0, "DAVE", channel::PRIVATE, "hello?"));
    assert!(world.pushed_since(alice, mark).is_empty());
    // ...another name does, and so does the same name later.
    world.ask(alice, request::chat(0, "Nobody", channel::PRIVATE, "hi"));
    assert_eq!(world.denials_since(alice, mark), [not_found_bytes()]);
    world.advance(Duration::from_secs(11));
    let mark = world.mark(alice);
    world.ask(alice, request::chat(0, "Nobody", channel::PRIVATE, "hi"));
    assert_eq!(world.denials_since(alice, mark), [not_found_bytes()]);

    // A player in the lobby gets the line and nothing is refused.
    let (mark, bob_mark) = (world.mark(alice), world.mark(bob));
    world.ask(alice, request::chat(0, "bob", channel::PRIVATE, "psst"));
    assert!(world.denials_since(alice, mark).is_empty());
    assert_eq!(
        chat_lines(&world.pushed_since(bob, bob_mark)),
        [chat_bytes(
            world.ids[alice].0,
            "Alice",
            channel::PRIVATE,
            "psst"
        )]
    );

    // One who ignores the sender is not given away: no line, no text.
    world.ask(bob, request::ignore(world.ids[alice].0));
    let (mark, bob_mark) = (world.mark(alice), world.mark(bob));
    world.ask(alice, request::chat(0, "Bob", channel::PRIVATE, "psst"));
    assert!(world.pushed_since(alice, mark).is_empty());
    assert!(chat_lines(&world.pushed_since(bob, bob_mark)).is_empty());

    // The player's own name: the game has no text for it.
    world.ask(alice, request::chat(0, "alice", channel::PRIVATE, "me"));
    assert!(world.pushed_since(alice, mark).is_empty());

    // A line on another channel that reaches nobody is no error.
    world.ask(alice, request::chat(7, "", channel::SQUAD, "anyone?"));
    assert!(world.pushed_since(alice, mark).is_empty());
    world.assert_clean();
}

#[test]
fn a_friend_request_or_an_invite_for_nobody_is_answered_with_player_not_found() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let carol = world.connect("Carol");
    signed_in(world.store, "Dave");
    for client in [alice, bob, carol] {
        world.take(client);
    }

    // A friend request for a name that is no account.
    let mark = world.mark(alice);
    world.ask(alice, request::important("Nobody", 0, ""));
    assert_eq!(world.denials_since(alice, mark), [not_found_bytes()]);
    assert_eq!(
        denial(&world.take(alice), 0x49).as_deref(),
        Some("st_player_not_found")
    );
    // One for a player who is offline waits for it: nothing is refused.
    let mark = world.mark(alice);
    world.ask(alice, request::important("Dave", 0, ""));
    assert!(world.pushed_since(alice, mark).is_empty());

    // A squad invite: the player must be in the lobby.
    world.ask(alice, request::important("Dave", 2, "invite"));
    assert_eq!(world.denials_since(alice, mark), [not_found_bytes()]);
    world.advance(Duration::from_secs(11));
    let mark = world.mark(alice);
    world.ask(alice, request::important("Ghost", 2, "invite"));
    assert_eq!(world.denials_since(alice, mark), [not_found_bytes()]);
    // "Membership accepted" is the client's own message, not the player's
    // act: one that can not be passed on is not answered.
    let mark = world.mark(alice);
    world.ask(
        alice,
        request::important("Phantom", 3, "membership accepted"),
    );
    assert!(world.pushed_since(alice, mark).is_empty());

    // Bob ignores Alice: her request and her invite are dropped with no
    // word to her, and nothing reaches him.
    world.ask(bob, request::ignore(world.ids[alice].0));
    let (mark, bob_mark) = (world.mark(alice), world.mark(bob));
    world.ask(alice, request::important("Bob", 0, ""));
    world.ask(alice, request::important("Bob", 2, "invite"));
    assert!(world.pushed_since(alice, mark).is_empty());
    assert!(world.pushed_since(bob, bob_mark).is_empty());

    // Carol is asked, and a second request to her (now waiting, later a
    // friend) is dropped in silence: the game has no text for it.
    let carol_mark = world.mark(carol);
    world.ask(alice, request::important("Carol", 0, ""));
    world.ask(alice, request::important("Carol", 0, ""));
    assert!(world.pushed_since(alice, mark).is_empty());
    let asked: Vec<u8> = world
        .pushed_since(carol, carol_mark)
        .iter()
        .map(|(id, _)| *id)
        .collect();
    assert_eq!(asked, [0xA4]);

    // Ignoring by a typed name that is no account: the list as it is, and
    // the text.
    world.advance(Duration::from_secs(11));
    let mark = world.mark(alice);
    let mut body = vec![4, 6];
    body.extend_from_slice(b"Nobody");
    world.ask(alice, (0x4E, body));
    let pushed = world.pushed_since(alice, mark);
    // `A3 08 <u16 0>`: the ignore list, empty.
    assert_eq!(pushed, [(0xA3, vec![8, 0, 0]), not_found_bytes()]);
    world.assert_clean();
}

/// From a match (the Tab list's menu, exe patch 86) a friend request for a
/// name that is no account (a bot's place) is dropped: the lobby's message
/// box would only come up back in the lobby.
#[test]
fn a_friend_request_for_nobody_from_a_match_is_dropped_in_silence() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let carol = world.connect("Carol");
    world.ask(alice, request::play(1));
    world.ask(carol, request::play(1));
    world.advance(PLAY_TO_MATCH);
    for client in [alice, carol] {
        world.take(client);
    }
    let mark = world.mark(alice);
    world.ask(alice, request::important("Nobody", 0, ""));
    assert!(world.denials_since(alice, mark).is_empty());
    // One to a player is sent from the match as from the lobby.
    let carol_mark = world.mark(carol);
    world.ask(alice, request::important("Carol", 0, ""));
    assert!(world.pushed_since(carol, carol_mark).iter().any(|(id, _)| *id == 0xA4));
    world.assert_clean();
}

#[test]
fn a_refused_squad_action_is_answered_with_the_squad_and_no_text() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let carol = world.connect("Carol");
    form_squad(&mut world, alice, bob);
    world.settle();
    for client in [alice, bob, carol] {
        world.take(client);
    }

    // The client drops `84 4A` (0x21DEA8) and the game has no text for a
    // squad refusal: none is sent, on any op.
    assert!(!denial_is_shown(0x4A));
    let marks = [world.mark(alice), world.mark(bob), world.mark(carol)];
    // Bob is no commander: he can not disband, drop Alice or add Carol.
    world.ask(bob, (0x4A, vec![1]));
    let mut drop_alice = vec![3];
    drop_alice.extend(world.ids[alice].0.to_le_bytes());
    world.ask(bob, (0x4A, drop_alice));
    world.ask(bob, request::add_squad_member("Carol"));
    // Alice adds a player who accepted no invite, and one who is nobody.
    world.ask(alice, request::add_squad_member("Carol"));
    world.ask(alice, request::add_squad_member("Nobody"));
    // Carol is in no squad.
    world.ask(carol, request::leave_squad());
    for (client, mark) in [alice, bob, carol].into_iter().zip(marks) {
        let pushed = world.pushed_since(client, mark);
        assert!(
            pushed.iter().all(|(id, _)| *id == 0xA1),
            "client {client}: {pushed:02X?}"
        );
    }
    // The squad stands as it was.
    let squad = world.store.squads().of(world.ids[alice].0).unwrap();
    assert_eq!(squad.members, [world.ids[alice].0, world.ids[bob].0]);
    world.assert_clean();
}

#[test]
fn the_general_channel_is_one_room_for_every_language() {
    // THE OWNER'S RULE (2026-10-12), as on 0.21d: English, Ukrainian and
    // Russian players share the general chat. A client names a room of its
    // language in `4F`; the server does not split by it.
    let mut world = World::new();
    let english = world.connect("Emma");
    let ukrainian = world.connect("Taras");
    let russian = world.connect("Ivan");
    let german = world.connect("Hans");
    // What each client sends at sign-in (0x26C170, 0x26C36E): slot 1 is 1
    // for `russian` and `ukrainian`, else 2; then the language byte.
    let rooms = [
        (english, 2, 0),
        (ukrainian, 1, 2),
        (russian, 1, 1),
        (german, 2, 3),
    ];
    for (client, room, language) in rooms {
        world.ask(client, subscribe_as(room, language));
        world.take(client);
    }
    let names = ["Emma", "Taras", "Ivan", "Hans"];
    for (speaker, room, _) in rooms {
        let marks: Vec<usize> = rooms
            .iter()
            .map(|(client, ..)| world.mark(*client))
            .collect();
        // The client sends the line with the id of its own room.
        let text = format!("hello from {}", names[speaker]);
        world.ask(speaker, request::chat(room, "", channel::GENERAL, &text));
        let line = chat_bytes(
            world.ids[speaker].0,
            names[speaker],
            channel::GENERAL,
            &text,
        );
        for ((listener, ..), mark) in rooms.iter().zip(marks) {
            let heard = chat_lines(&world.pushed_since(*listener, mark));
            if *listener == speaker {
                // Its own client has shown it already.
                assert!(heard.is_empty());
            } else {
                assert_eq!(
                    heard,
                    std::slice::from_ref(&line),
                    "{} to {}",
                    names[speaker],
                    names[*listener]
                );
            }
        }
    }
    // And each client took the lines as chat lines of the general channel.
    for (client, ..) in rooms {
        let heard = world
            .take(client)
            .iter()
            .filter(|message| {
                matches!(
                    message,
                    Decoded::ChatText {
                        sender_type: 5,
                        channel: channel::GENERAL,
                        ..
                    }
                )
            })
            .count();
        assert_eq!(heard, 3);
    }
    world.assert_clean();
}

#[test]
fn a_stock_clients_own_spam_filter_is_stricter_than_the_servers_limit() {
    // The game's spam notice is the client's own (0x2083F0): it blocks the
    // line and prints the text itself. So the server's limit must never
    // drop a line a stock client's filter let through, or the player would
    // lose a line with no notice.
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    for client in [alice, bob] {
        world.ask(client, request::subscribe());
        world.take(client);
    }
    let heard = |world: &mut World| {
        world
            .take(bob)
            .iter()
            .filter(|message| matches!(message, Decoded::ChatText { .. }))
            .count()
    };

    // The fastest a player can talk without the filter stepping in: two
    // lines, then the next two no sooner than two seconds after the first.
    let mut filter = ChatSpamFilter::new();
    let mut sent = 0;
    for tick in 0..1200_u32 {
        let now = tick * 100;
        if tick % 20 < 2 {
            assert_eq!(filter.send(now, "gg", channel::GENERAL), Ok(()));
            let (id, body) = request::chat(2, "", channel::GENERAL, "gg");
            world.client(alice).send(id, &body);
            sent += 1;
        }
        world.advance(Duration::from_millis(100));
    }
    world.settle();
    assert_eq!(sent, 120);
    assert_eq!(heard(&mut world), sent);

    // A player who hammers the key: the filter lets a few through and
    // prints its notice for the rest; every line it sends arrives.
    let mut filter = ChatSpamFilter::new();
    let (mut sent, mut blocked) = (0, 0);
    for tick in 0..600_u32 {
        match filter.send(tick * 100, "gg", channel::GENERAL) {
            Ok(()) => {
                let (id, body) = request::chat(2, "", channel::GENERAL, "gg");
                world.client(alice).send(id, &body);
                sent += 1;
            }
            Err(_) => blocked += 1,
        }
        world.advance(Duration::from_millis(100));
    }
    world.settle();
    assert!(
        sent >= 10 && blocked > sent,
        "{sent} sent, {blocked} blocked"
    );
    assert_eq!(heard(&mut world), sent);
    world.assert_clean();
}

impl World {
    /// Another match of the client's account ends, settled behind the
    /// lobby's back (the lobby is not told: no screen is shown for it).
    fn settle_another(&mut self, index: usize, match_id: u32) {
        let map_id = self.host.made.lock().unwrap()[0].1.map_id;
        let (account_id, session_id) = self.ids[index];
        let result = MatchResult {
            match_id,
            map_id,
            game_mode: maps::map_mode(map_id).unwrap().mode,
            match_level: 0,
            end: MatchEnd::ModeRule,
            winner: Some(0),
            team_points: [3, 1],
            duration_ms: 700_000,
            played_ms: Some(600_000),
            trusted_snapshot_ms: Some(690_000),
            trusted_by: 2,
            vs_bots: false,
            private: false,
            blackout: false,
            players: vec![PlayerResult {
                place: 0,
                session_id,
                account_id,
                name: "Alice".to_owned(),
                team: 0,
                driven: false,
                entered: true,
                present_at_end: true,
                left: false,
                time_in_match_ms: 600_000,
                score: 0,
                kills: 0,
                deaths: 0,
                longest_killstreak: 0,
                kit_changes: 0,
                statistics: Some(PlayerStatistics::default()),
                pay_statistics: None,
                supplies: None,
            }],
        };
        let terms = Terms::at(session::v026g::quests::now());
        let report = self
            .results
            .settle(&result, self.accounts, &terms, &mut Rng::new(7));
        assert!(report.is_some());
    }
}

#[test]
fn a_row_of_an_open_results_screen_is_answered_however_many_matches_ended_since() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let (match_id, _) = world.match_of(alice, bob);
    world.end_match(match_id, 0);
    world.settle();
    assert_eq!(screens(&world.take(alice)).len(), 1);
    world.take(bob);

    let details = |world: &mut World, player_id: u32, match_id: u32| {
        world.ask(alice, request::player_details(player_id, match_id));
        world
            .take(alice)
            .iter()
            .filter(|message| matches!(message, Decoded::PlayerDetails { .. }))
            .count()
    };
    // More matches end than the lobby keeps reports of, while her screen
    // is open: a click on a row is still answered.
    for index in 0..80 {
        world.settle_another(alice, 0xB000 + index);
    }
    assert_eq!(details(&mut world, 2, match_id), 1);
    // A match whose screen nobody was sent has gone, as before.
    assert!(world.results.find(0xB000).is_none());
    assert_eq!(details(&mut world, 1, 0xB000), 0);

    // The screen asked for again (`41 00`) is kept in its turn.
    world.ask(alice, request::match_stats(0xB000 + 79));
    assert_eq!(screens(&world.take(alice)).len(), 1);
    for index in 0..80 {
        world.settle_another(alice, 0xC000 + index);
    }
    assert_eq!(details(&mut world, 1, 0xB000 + 79), 1);
    assert!(world.results.find(0xC000).is_none());
    // (The first match is kept too: Bob's screen of it may be open.)
    assert_eq!(details(&mut world, 2, match_id), 1);
    world.assert_clean();
}
