//! A room's free-for-all (`docs/notes/ffa.md`): the card voted for plays
//! with each ready member, each for himself (a squad is no squad in it);
//! as many players as a match of two teams of the room (no most of its own:
//! the room's size, 20 at the most); a private room's owner starts one alone.

use std::collections::HashSet;
use survarium::v026g::game_data::TEST_TWISTS_ONLY;
use survarium::v026g::maps::MapId;

use super::*;
use crate::v026g::MatchSetup;

/// The free-for-alls of the test data.
fn free_for_alls() -> Vec<MapId> {
    maps::playable_in(maps::GameMode::FreeForAll)
        .map(|map| map.id)
        .collect()
}

/// A lobby whose match pool is `pool`.
fn world_of(pool: Vec<MapId>) -> World {
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

/// The room's pool has two maps of two teams and a free-for-all: its vote
/// shows the three, the free-for-all on any card (the owner's of
/// 2026-11-22). The members vote for it, and the match is that map's with
/// each of them in it, in no squad. One who comes while it is played, its
/// places not all given, goes into it.
#[test]
fn a_room_whose_vote_wins_a_free_for_all_card_plays_it_with_each_ready_member() {
    let free_for_all = free_for_alls()[0];
    let mut world = world_of(vec![MapId::new(61), MapId::new(117), free_for_all]);
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
    let mut sorted = offered.clone();
    sorted.sort_unstable();
    let mut pool = vec![MapId::new(61), MapId::new(117), free_for_all];
    pool.sort_unstable();
    assert_eq!(sorted, pool, "{offered:?}");
    let card = offered.iter().position(|&id| id == free_for_all).unwrap() as u32;
    for index in [alice, bob, carol] {
        rows(&mut world, index, ROOM_VOTE_BASE + card, 1);
    }
    let ticket = room_match_starts(&mut world, &[alice, bob, carol], VOTE_AND_WAIT);
    assert_eq!(world.host.made.locked().len(), 1);
    let setup = last_setup(&world);
    assert_eq!(setup.played.map().id, free_for_all, "the vote's winner");
    assert_eq!(setup.players.len(), 3);
    assert_eq!(seated(&setup), accounts(&world, &[alice, bob, carol]));
    assert!(setup.players.iter().all(|player| player.profile.squad == 0));
    assert_eq!(setup.room_minimum, Some(2));
    assert_eq!(setup.kind, session::v026g::match_kind::MatchKind::Normal);
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

/// A room with bots holds no free-for-all, even with its maps named as bots'
/// maps: the server's bots play none (the match server refuses them in one),
/// so the lobby takes no such map into a room with bots (`pvb::bots_play`).
#[test]
fn a_room_with_bots_holds_no_free_for_all() {
    let mut world = free_for_all_world();
    world.core.settings.pvb.enabled = true;
    world.core.settings.pvb.bot_maps = free_for_alls();
    assert!(!world.core.settings.pvb.bot_maps.is_empty());
    assert!(world.core.settings.pvb.bot_maps_ready().is_empty());
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
    assert_eq!(
        setup.kind,
        session::v026g::match_kind::MatchKind::PrivateRoom
    );
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

/// A map played with a twist only (`MapMode::twists_only`: Military Warehouses in the live data,
/// the test data's [`TEST_TWISTS_ONLY`]) in a private room: the create window lists it, a
/// private room made with it alone holds it, and its owner's Start plays it plain (the owner,
/// 2026-11-27: "we removed FFA from them, which is fine for pub lobbies, not for private ones").
#[test]
fn a_private_room_plays_a_map_played_with_a_twist_only_plain() {
    let twisted = TEST_TWISTS_ONLY;
    let map = maps::map_mode(twisted).expect("its config");
    assert!(map.twists_only);
    let mut world = world_of(vec![twisted]);
    let (key, members) = private_room(&mut world, 1);
    let pool = rows(&mut world, members[0], ROOM_POOL_BASE, 255);
    // Offered (the last field): the create window lists it.
    let row = format!("0;{twisted};{};{};8;0;0;1", map.level, map.weather);
    assert_eq!(pool, ["S1;M;1;0", row.as_str()]);
    assert_eq!(world.core.rooms.get(key).unwrap().pool, [twisted]);
    sent_to_match(&press_start(&mut world, members[0])).expect("a match");
    let setup = last_setup(&world);
    assert_eq!(setup.played.map().id, twisted);
    assert_eq!(setup.played.armed(), None, "plain");
    assert_eq!(setup.played.lobby_number(), maps::GameMode::FreeForAll.lobby_number());
}

/// The same map in an open room: every card on it is a twist, and the match it wins plays it.
#[test]
fn an_open_rooms_card_on_a_map_played_with_a_twist_only_is_a_twist() {
    let twisted = TEST_TWISTS_ONLY;
    let plain = maps::GameMode::FreeForAll.lobby_number();
    let mut world = world_of(vec![twisted]);
    let players: Vec<usize> = ["Alice", "Bob", "Carol"]
        .into_iter()
        .map(|name| world.connect(name))
        .collect();
    let key = create(&mut world, players[0], spec(6), None);
    for &index in &players[1..] {
        rows(&mut world, index, ROOM_JOIN_BASE + key, 1);
    }
    world.advance(Duration::from_secs(1));
    let voting = state(&mut world, players[0]);
    let modes: Vec<(MapId, u8)> = voting
        .iter()
        .filter(|row| row.starts_with("C;"))
        .map(|row| {
            let fields: Vec<&str> = row.split(';').collect();
            (fields[2].parse().unwrap(), fields[5].parse().unwrap())
        })
        .collect();
    assert!(!modes.is_empty(), "{voting:?}");
    for &(map, mode) in &modes {
        assert_eq!(map, twisted);
        assert_ne!(mode, plain, "a card on it plain: {voting:?}");
    }
    for &index in &players {
        rows(&mut world, index, ROOM_VOTE_BASE, 1);
    }
    room_match_starts(&mut world, &players, VOTE_AND_WAIT);
    let setup = last_setup(&world);
    assert_eq!(setup.played.map().id, twisted);
    assert!(setup.played.armed().is_some(), "a twist");
    assert_eq!(setup.played.lobby_number(), modes[0].1);
}
