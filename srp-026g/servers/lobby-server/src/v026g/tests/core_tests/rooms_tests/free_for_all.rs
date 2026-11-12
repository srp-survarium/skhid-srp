//! A room's free-for-all (`docs/notes/ffa.md`): the card voted for plays
//! with each ready member, each for himself (a squad is no squad in it);
//! no more players than the map's most (10), the server's bots among them,
//! the members past that waiting in the room; a private room's owner starts
//! one alone.

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

/// The most players of a free-for-all: 10.
fn cap() -> usize {
    let cap = maps::player_cap(maps::map_mode(free_for_alls()[0]).unwrap()).expect("its most");
    assert_eq!(cap, 10);
    cap
}

/// What the match server was asked for last, as the lobby asked it.
fn last_setup(world: &World) -> MatchSetup {
    let setup = world.host.made.locked().last().expect("a match").1.clone();
    assert_eq!(setup.map.mode(), maps::GameMode::FreeForAll);
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
    assert_eq!(setup.map.map().id, free_for_all, "the vote's winner");
    assert_eq!(setup.players.len(), 3);
    assert_eq!(seated(&setup), accounts(&world, &[alice, bob, carol]));
    assert!(setup.players.iter().all(|player| player.profile.squad == 0));
    assert_eq!(setup.room_minimum, Some(2));
    assert!(!setup.private && !setup.vs_bots);
    let room = world.core.rooms.get(key).unwrap();
    assert!(matches!(room.stage, Stage::InMatch { .. }));
    assert_eq!(room.in_match.len(), 3);

    // Dave comes while it is played: 3 of its 10 places are given.
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

/// Twelve members and a free-for-all: its first ten (as they came into the
/// room) play, the other two stay in the room and are told the match has no
/// place (`JoinHold::PlayerLimit`), as is one who comes while it is played;
/// none of them is sent to it, however long they wait.
#[test]
fn a_free_for_all_seats_its_first_ten_members_and_the_others_wait_in_the_room() {
    let mut world = free_for_all_world();
    let members: Vec<usize> = (0..12)
        .map(|n| world.connect(&format!("Member{n}")))
        .collect();
    let key = create(&mut world, members[0], spec(8), None);
    for &member in &members[1..] {
        rows(&mut world, member, ROOM_JOIN_BASE + key, 1);
    }
    assert_eq!(world.core.rooms.get(key).unwrap().members.len(), 12);
    let (first, rest) = members.split_at(cap());
    for &member in rest {
        world.take(member);
    }
    let ticket = room_match_starts(&mut world, first, VOTE_AND_WAIT);
    let setup = last_setup(&world);
    assert_eq!(setup.players.len(), cap());
    assert_eq!(seated(&setup), accounts(&world, first));
    for &member in rest {
        assert!(sent_to_match(&world.take(member)).is_none());
    }
    let room = world.core.rooms.get(key).unwrap();
    assert_eq!(room.members.len(), 12, "the others are its members still");
    assert_eq!(room.in_match, accounts(&world, first));

    // The match server takes players; this match has no place to give.
    world.host.admitting.locked().insert(ticket);
    let late = world.connect("Late");
    rows(&mut world, late, ROOM_JOIN_BASE + key, 1);
    world.take(late);
    let waiting: Vec<usize> = rest.iter().copied().chain([late]).collect();
    for &member in &waiting {
        assert_eq!(
            join_row(&state(&mut world, member)),
            Some("J;-1;7"),
            "every place is given"
        );
    }
    world.advance(Duration::from_secs(40));
    for &member in &waiting {
        assert!(sent_to_match(&world.take(member)).is_none());
        assert_eq!(world.core.rooms.key_of(world.ids[member].0), Some(key));
    }
    assert_eq!(last_setup(&world).players.len(), cap());
    assert_eq!(world.host.made.locked().len(), 1);
    assert_eq!(
        *world.host.join_asks.locked(),
        Vec::<u32>::new(),
        "the match server was not asked for a place it cannot give"
    );
}

/// A room with bots (8 a team in a match of two teams) fills a free-for-all
/// with the server's bots up to its most players and no further: its two
/// members and eight bots.
#[test]
fn a_room_with_bots_fills_a_free_for_all_with_bots_up_to_its_most_players() {
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
    assert_eq!(setup.players.len(), cap());
    assert_eq!(seated(&setup), accounts(&world, &[alice, bob]));
    let bots = setup.players.iter().filter(|player| player.driven).count();
    assert_eq!(bots, cap() - 2);
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

/// The members a free-for-all had no place for are the first seated in the
/// room's next match (`Rooms::left_out`), so nobody waits twice while ten
/// others play twice.
#[test]
fn the_members_a_free_for_all_had_no_place_for_play_its_rooms_next_match() {
    let mut world = free_for_all_world();
    let members: Vec<usize> = (0..12)
        .map(|n| world.connect(&format!("Member{n}")))
        .collect();
    let key = create(&mut world, members[0], spec(8), None);
    for &member in &members[1..] {
        rows(&mut world, member, ROOM_JOIN_BASE + key, 1);
    }
    let (first, rest) = members.split_at(cap());
    let ticket = room_match_starts(&mut world, first, VOTE_AND_WAIT);
    world.end_match(ticket, 0);
    for &member in &members {
        world.take(member);
    }
    world.advance(Duration::from_secs(120));
    assert_eq!(world.host.made.locked().len(), 2, "the next match");
    let waited = accounts(&world, rest);
    let again = seated(&last_setup(&world));
    assert!(
        waited.is_subset(&again),
        "the two who waited were left out again: {waited:?} of {again:?}"
    );
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

/// The Rooms list shows a team modes' players by their sides
/// (`a_member_list_says_the_side_of_each_player_in_the_match`); a
/// free-for-all's are in the match and on no side, though the lobby seated
/// them two a side.
#[test]
fn a_member_list_says_no_side_for_the_players_of_a_free_for_all() {
    let mut world = free_for_all_world();
    let players: Vec<usize> = ["Alice", "Bob", "Carol", "Dave"]
        .iter()
        .map(|name| world.connect(name))
        .collect();
    let key = create(&mut world, players[0], spec(6), None);
    for &player in &players[1..] {
        rows(&mut world, player, ROOM_JOIN_BASE + key, 1);
    }
    room_match_starts(&mut world, &players, VOTE_AND_WAIT);
    let eve = world.connect("Eve");
    let seen = members(&mut world, eve, key);
    assert_eq!(seen[0][3], "4", "{seen:?}");
    for row in &seen[1..] {
        // In the match; its side.
        assert_eq!((row[5].as_str(), row[8].as_str()), ("1", "0"), "{row:?}");
    }
}
