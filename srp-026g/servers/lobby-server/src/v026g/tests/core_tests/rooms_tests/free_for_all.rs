//! A room's free-for-all (`docs/notes/ffa.md`): the card voted for plays
//! with each ready member, each for himself (a squad is no squad in it);
//! as many players as a match of two teams of the room (no most of its own:
//! the room's size, 20 at the most); a private room's owner starts one alone.

use std::collections::HashSet;

use super::*;
use crate::v026g::MatchSetup;

/// The free-for-alls of the test data.
fn free_for_alls() -> Vec<u8> {
    maps::playable_in(maps::GameMode::FreeForAll)
        .map(|map| map.id)
        .collect()
}

/// A lobby whose match pool is `pool`.
fn world_of(pool: Vec<u8>) -> World {
    let mut settings = Settings::local("127.0.0.1:0");
    settings.maps = pool;
    World::with_settings(settings)
}

/// A lobby whose rooms play free-for-alls alone: every card is one.
fn free_for_all_world() -> World {
    world_of(free_for_alls())
}

/// What the match server was asked for last, as the lobby asked it.
fn last_setup(world: &World) -> MatchSetup {
    let setup = world.host.made.locked().last().expect("a match").1.clone();
    assert_eq!(setup.played.mode(), maps::GameMode::FreeForAll);
    setup
}

/// The accounts of the setup's players, the server's bots left out.
fn seated(setup: &MatchSetup) -> HashSet<u32> {
    setup
        .players
        .iter()
        .filter(|player| !player.driven)
        .map(|player| player.account_id)
        .collect()
}

fn accounts(world: &World, members: &[usize]) -> HashSet<u32> {
    members.iter().map(|&member| world.ids[member].0).collect()
}

/// The room's pool has two maps of two teams and a free-for-all: its vote's
/// third card is the free-for-all (the owner's of 2026-11-12), the first two
/// the others. The members vote for it, and the match is that map's with
/// each of them in it, in no squad. One who comes while it is played, its
/// places not all given, goes into it.
#[test]
fn a_room_whose_vote_wins_a_free_for_all_card_plays_it_with_each_ready_member() {
    let free_for_all = free_for_alls()[0];
    let mut world = world_of(vec![61, 117, free_for_all]);
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let carol = world.connect("Carol");
    let dave = world.connect("Dave");
    let key = create(&mut world, alice, spec(6), None);
    for index in [bob, carol] {
        rows(&mut world, index, ROOM_JOIN_BASE + key, 1);
    }
    world.advance(Duration::from_secs(1));
    let voting = state(&mut world, alice);
    assert_eq!(header(&voting)[PHASE], "3", "the vote");
    let offered = cards(&voting);
    assert!(offered[..2].contains(&61) && offered[..2].contains(&117), "{offered:?}");
    assert_eq!(offered[2..], [free_for_all], "{offered:?}");
    for index in [alice, bob, carol] {
        rows(&mut world, index, ROOM_VOTE_BASE + 2, 1);
    }
    let ticket = room_match_starts(&mut world, &[alice, bob, carol], VOTE_AND_WAIT);
    assert_eq!(world.host.made.locked().len(), 1);
    let setup = last_setup(&world);
    assert_eq!(setup.played.map().id, free_for_all, "the vote's winner");
    assert_eq!(setup.players.len(), 3);
    assert_eq!(seated(&setup), accounts(&world, &[alice, bob, carol]));
    assert!(setup.players.iter().all(|player| player.profile.squad == 0));
    assert_eq!(setup.room_minimum, Some(2));
    assert!(!setup.private && !setup.vs_bots);
    let room = world.core.rooms.get(key).unwrap();
    assert!(matches!(room.stage, Stage::InMatch { .. }));
    assert_eq!(room.in_match.len(), 3);

    // Dave comes while it is played: 3 of its places are given.
    world.host.admitting.locked().insert(ticket);
    rows(&mut world, dave, ROOM_JOIN_BASE + key, 1);
    world.take(dave);
    assert_eq!(join_row(&state(&mut world, dave)), Some("J;15"));
    world.advance(Duration::from_secs(17));
    let (sent, _) = sent_to_match(&world.take(dave)).expect("Dave goes in late");
    assert_eq!(sent, ticket);
    let setup = last_setup(&world);
    assert_eq!(setup.players.len(), 4);
    assert_eq!(setup.players[3].account_id, world.ids[dave].0);
    assert_eq!(setup.players[3].profile.squad, 0);
    world.assert_clean();
}

/// A squad of two in a room is one side's squad in a match of two teams
/// (`a_squad_in_a_room_plays_as_a_squad_on_one_side`); in a free-for-all its
/// members play as anyone, each for himself: no squad in the match.
#[test]
fn a_squad_in_a_free_for_all_room_is_no_squad_in_its_match() {
    let mut world = free_for_all_world();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let carol = world.connect("Carol");
    form_squad(&mut world, alice, bob);
    let key = create(&mut world, carol, spec(6), None);
    // Alice's Join takes Bob, her squad's soldier, along.
    rows(&mut world, alice, ROOM_JOIN_BASE + key, 1);
    assert_eq!(world.core.rooms.get(key).unwrap().members.len(), 3);
    room_match_starts(&mut world, &[alice, bob, carol], VOTE_AND_WAIT);
    let setup = last_setup(&world);
    assert_eq!(seated(&setup), accounts(&world, &[alice, bob, carol]));
    for player in &setup.players {
        assert_eq!(player.profile.squad, 0, "{}", player.profile.name);
    }
    // The pair is a squad in the lobby still.
    assert_eq!(
        world.store.squads().mate_of(world.ids[alice].0),
        Some(world.ids[bob].0)
    );
}

/// A full open room of 8 a side, 16 members, and a free-for-all: every
/// member plays it, as a match of two teams of the room (the owner's of
/// 2026-11-13: "it should be room limit").
#[test]
fn a_full_room_plays_a_free_for_all_with_every_member() {
    let mut world = free_for_all_world();
    let members: Vec<usize> = (0..16)
        .map(|n| world.connect(&format!("Member{n}")))
        .collect();
    let key = create(&mut world, members[0], spec(8), None);
    for &member in &members[1..] {
        rows(&mut world, member, ROOM_JOIN_BASE + key, 1);
    }
    assert_eq!(world.core.rooms.get(key).unwrap().members.len(), 16);
    room_match_starts(&mut world, &members, VOTE_AND_WAIT);
    let setup = last_setup(&world);
    assert_eq!(setup.players.len(), 16);
    assert_eq!(seated(&setup), accounts(&world, &members));
    assert_eq!(world.core.rooms.get(key).unwrap().in_match, accounts(&world, &members));
}

/// A room with bots (8 a team) fills a free-for-all with the server's bots
/// as a match of two teams: its two members and fourteen bots. Here its maps
/// have a bots' map file; the release's have none, so no room with bots
/// holds one (`ffa.md`).
#[test]
fn a_room_with_bots_fills_a_free_for_all_as_a_match_of_two_teams() {
    use survarium::v026g::lobby::rooms::ROOM_CREATE_BOTS;

    let mut world = free_for_all_world();
    world.core.settings.pvb.enabled = true;
    world.core.settings.pvb.bot_maps = free_for_alls();
    let alice = world.connect("Alice");
    rows(&mut world, alice, ROOM_PIN_BASE + 1234, 1);
    let made = rows(&mut world, alice, ROOM_CREATE_BOTS, 1);
    assert_eq!(header(&made)[1], "R", "{made:?}");
    let key: u32 = header(&made)[KEY].parse().unwrap();
    assert!(world.core.rooms.get(key).unwrap().with_bots());
    let bob = world.connect("Bob");
    rows(&mut world, bob, ROOM_PIN_BASE + 1234, 1);
    rows(&mut world, bob, ROOM_JOIN_BASE + key, 1);
    world.take(alice);
    world.take(bob);
    let (ticket, _) = sent_to_match(&press_start(&mut world, alice)).expect("a match");
    assert_eq!(sent_to_match(&world.take(bob)).map(|(id, _)| id), Some(ticket));
    let setup = last_setup(&world);
    assert_eq!(setup.players.len(), 16);
    assert_eq!(seated(&setup), accounts(&world, &[alice, bob]));
    let bots = setup.players.iter().filter(|player| player.driven).count();
    assert_eq!(bots, 14);
    // The members first, the bots after them.
    assert!(setup.players[..2].iter().all(|player| !player.driven));
    assert!(setup.vs_bots && !setup.private);
}

/// A private room's owner starts a free-for-all alone: a match of one, the
/// room's minimum of one, paying nothing.
#[test]
fn a_private_rooms_owner_starts_a_free_for_all_alone() {
    let mut world = free_for_all_world();
    let (key, members) = private_room(&mut world, 1);
    let owner = members[0];
    let (ticket, _) = sent_to_match(&press_start(&mut world, owner)).expect("a match");
    let setup = last_setup(&world);
    assert_eq!(setup.players.len(), 1);
    assert_eq!(setup.players[0].account_id, world.ids[owner].0);
    assert_eq!(setup.room_minimum, Some(1));
    assert!(setup.private && !setup.vs_bots);
    assert!(matches!(
        world.core.rooms.get(key).unwrap().stage,
        Stage::InMatch { .. }
    ));
    assert!(world.host.open.locked().contains(&ticket));
}

/// A squad of two alone in an open room waits for a third, as before any
/// match (the owner's of 2026-10-31: a squad plays no match against
/// itself): the room's tick does not count a pair alone, before the map is
/// known, so its vote does not end, though every card is a free-for-all.
#[test]
fn a_squad_alone_in_an_open_room_waits_for_a_third_before_a_free_for_all_too() {
    let mut world = free_for_all_world();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    form_squad(&mut world, alice, bob);
    // The commander's Play takes its mate into one room.
    rows(&mut world, alice, ROOM_PLAY, 1);
    let key = world.core.rooms.key_of(world.ids[alice].0).expect("a room");
    assert_eq!(world.core.rooms.get(key).unwrap().members.len(), 2);
    world.advance(QUICK_START * 2);
    assert_eq!(world.host.made.locked().len(), 0, "the pair waits for a third");
}

/// A private room of 20, the most a match has (the game's 20 places): its
/// owner's Start plays a free-for-all with all twenty.
#[test]
fn a_private_room_of_twenty_plays_a_free_for_all_with_all_twenty() {
    let mut world = free_for_all_world();
    let (_, members) = private_room(&mut world, 20);
    sent_to_match(&press_start(&mut world, members[0])).expect("a match");
    let setup = last_setup(&world);
    assert_eq!(setup.players.len(), 20);
    assert_eq!(seated(&setup), accounts(&world, &members));
}

/// A free-for-all has no teams: each of its players is a side of his own
/// (`Side::Alone`), as the match server has it. A line on a team channel
/// reaches nobody but its sender, whose game has shown it, though the
/// lobby's seating has another of the four on the sender's side; the
/// match's channel reaches the three others.
#[test]
fn a_team_line_in_a_free_for_all_reaches_its_sender_only() {
    let mut world = free_for_all_world();
    let players: Vec<usize> = ["Alice", "Bob", "Carol", "Dave"]
        .iter()
        .map(|name| world.connect(name))
        .collect();
    let (alice, others) = (players[0], &players[1..]);
    let key = create(&mut world, alice, spec(6), None);
    for &other in others {
        rows(&mut world, other, ROOM_JOIN_BASE + key, 1);
    }
    let ticket = room_match_starts(&mut world, &players, VOTE_AND_WAIT);
    let seats = &world
        .core
        .matches
        .values()
        .find(|running| running.ticket.match_id == ticket)
        .expect("the match")
        .seats;
    let seated_on = |team: u8| seats.iter().filter(|seat| seat.1 == team).count();
    assert_eq!([seated_on(0), seated_on(1)], [2, 2], "{seats:?}");
    let heard = |world: &mut World, player: usize| {
        world
            .take(player)
            .iter()
            .filter(|message| matches!(message, Decoded::ChatText { .. }))
            .count()
    };
    // The game sends a team line on the channel of its lobby seat.
    for team_channel in [channel::TEAM_1, channel::TEAM_2] {
        world.ask(alice, request::chat(ticket, "", team_channel, "go left"));
        for &other in others {
            assert_eq!(heard(&mut world, other), 0, "channel {team_channel}");
        }
    }
    world.ask(alice, request::chat(ticket, "", channel::MATCH, "gl"));
    for &other in others {
        assert_eq!(heard(&mut world, other), 1);
    }
    world.assert_clean();
}
