//! A room's bots (its "Bots per team", 0 to 3; the owner's of 2026-10-26:
//! "it should be possible in lobby to add up to 3 vs 3 bots, which will be
//! replaced with real players once those join"): the setting made, kept and
//! told; the match starting with the bots in the places its players leave,
//! a place kept for each bot within the 20; a private room's arranged teams
//! with bots beside the arrangement; and a member who comes taking a bot's
//! place, a side full of players and bots too.

use survarium::v026g::lobby::rooms::{
    BotsPerTeam, ROOM_FORM, ROOM_JOIN_BASE, ROOM_PIN_BASE, RoomSpec, create_start,
};

use super::rooms_tests::{
    arrangement, create, header, join_row, move_member, press_start, rows, spec, state,
};
use super::*;
use crate::v026g::play::Occupant;
use crate::v026g::{LiveView, MatchSetup};

/// From two players to the match: the vote's 15 s, the wait (15 s), and a
/// little more.
const VOTE_AND_WAIT: Duration = Duration::from_secs(32);
/// The state header's field of the bots a team.
const BOTS: usize = 17;

fn bots(count: u8) -> BotsPerTeam {
    BotsPerTeam::new(count).unwrap()
}

/// A room of `size` a side and `count` bots a team.
fn with_bots(size: u8, count: u8) -> RoomSpec {
    RoomSpec {
        bots: bots(count),
        ..spec(size)
    }
}

/// `names` connected, the first making the room, the others joining (with
/// the PIN, if it has one): their indices.
fn room_of(world: &mut World, room: RoomSpec, pin: Option<u16>, names: &[&str]) -> (u32, Vec<usize>) {
    let members: Vec<usize> = names.iter().map(|name| world.connect(name)).collect();
    let key = create(world, members[0], room, pin);
    for &member in &members[1..] {
        if let Some(pin) = pin {
            rows(world, member, ROOM_PIN_BASE + u32::from(pin), 1);
        }
        rows(world, member, ROOM_JOIN_BASE + key, 1);
    }
    for &member in &members {
        world.take(member);
    }
    (key, members)
}

/// The match the lobby made last: its setup, and its id in the lobby.
fn last_match(world: &World) -> (u32, MatchSetup, u32) {
    let (ticket, setup) = world.host.made.locked().last().cloned().expect("a match");
    let id = *world
        .core
        .matches
        .iter()
        .find(|(_, running)| running.ticket.match_id == ticket)
        .expect("running")
        .0;
    (ticket, setup, id)
}

/// Of a match's roster: each team's players and server's bots.
fn teams(setup: &MatchSetup) -> [(usize, usize); 2] {
    let on = |team: u8, driven: bool| {
        setup
            .players
            .iter()
            .filter(|player| player.profile.team == team && player.driven == driven)
            .count()
    };
    [(on(0, false), on(0, true)), (on(1, false), on(1, true))]
}

/// The setting is made with the room (the create code's top), told in its
/// state, and kept for its maker's next create window. Its match starts
/// with three bots a team after the two players: the server's own, no
/// account and no session, named as the bots of a match against bots.
#[test]
fn an_open_rooms_match_starts_with_its_bots_in_the_places_its_players_leave() {
    let mut world = World::new();
    let (_, members) = room_of(&mut world, with_bots(6, 3), None, &["Alice", "Bob"]);
    let alice = members[0];
    assert_eq!(header(&state(&mut world, alice))[BOTS], "3");
    let kept = world.account(alice).room_settings.clone().expect("kept");
    assert_eq!(kept.bots, bots(3));
    let form = rows(&mut world, alice, ROOM_FORM, 1);
    assert!(form[0].ends_with(";3"), "{form:?}");

    world.advance(VOTE_AND_WAIT);
    let (_, setup, id) = last_match(&world);
    assert_eq!(teams(&setup), [(1, 3), (1, 3)]);
    assert_eq!(setup.players.len(), 8);
    // After the players, team 0's first.
    let bot_rows: Vec<(u8, &str)> = setup.players[2..]
        .iter()
        .map(|player| (player.profile.team, player.profile.name.as_str()))
        .collect();
    assert_eq!(
        bot_rows,
        [
            (0, "Bot-1"),
            (0, "Bot-2"),
            (0, "Bot-3"),
            (1, "Bot-4"),
            (1, "Bot-5"),
            (1, "Bot-6")
        ]
    );
    for bot in &setup.players[2..] {
        assert!(bot.driven && !bot.bot);
        assert_eq!((bot.account_id, bot.session_id), (0, 0));
    }
    assert!(!setup.vs_bots && !setup.private);
    assert_eq!(setup.room_minimum, Some(2));
    let running = &world.core.matches[&id];
    assert_eq!(
        running.seats.iter().filter(|(occupant, _)| *occupant == Occupant::Bot).count(),
        6
    );

    // A room of none plays none, as before.
    let mut world = World::new();
    room_of(&mut world, spec(6), None, &["Carol", "Dave"]);
    world.advance(VOTE_AND_WAIT);
    assert_eq!(teams(&last_match(&world).1), [(1, 0), (1, 0)]);
}

/// The bots fill only places players do not have, and the match keeps a
/// place for each: 7 v 7 in a room of 10 a side and 3 bots a team takes 3
/// bots in all (14 + 2 × 3 = 20), the team with more giving one up first.
#[test]
fn a_rooms_bots_never_take_the_places_its_joiners_need() {
    let mut world = World::new();
    let names: Vec<String> = (0..14).map(|n| format!("Member{n}")).collect();
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    room_of(&mut world, with_bots(10, 3), None, &names);
    world.advance(VOTE_AND_WAIT);
    let (_, setup, _) = last_match(&world);
    let [(a, a_bots), (b, b_bots)] = teams(&setup);
    assert_eq!((a, b), (7, 7));
    assert_eq!(a_bots + b_bots, 3);
    assert!(a_bots.abs_diff(b_bots) == 1);
    assert_eq!(setup.players.len() + a_bots + b_bots, 20);
    // A room of three a side, 3 bots a team: its sides full of players and
    // bots, 6 + 12 = 18.
    let mut world = World::new();
    let names = ["A", "B", "C", "D", "E", "F"];
    room_of(&mut world, with_bots(6, 3), None, &names);
    world.advance(VOTE_AND_WAIT);
    assert_eq!(teams(&last_match(&world).1), [(3, 3), (3, 3)]);
}

/// A private room's owner arranges 1 v 3: its match plays that, with two
/// bots a team beside it (the arrangement unchanged: the bots are in no
/// room); it pays nothing, as any private room's.
#[test]
fn a_private_rooms_arranged_teams_get_their_bots_beside_the_arrangement() {
    let mut world = World::new();
    let (_, members) = room_of(
        &mut world,
        with_bots(6, 2),
        Some(1234),
        &["Alice", "Bob", "Carol", "Dave"],
    );
    let owner = members[0];
    assert_eq!(move_member(&mut world, owner, 2), "0111");
    assert_eq!(header(&state(&mut world, owner))[BOTS], "2");
    assert!(sent_to_match(&press_start(&mut world, owner)).is_some());
    let (_, setup, _) = last_match(&world);
    assert_eq!(teams(&setup), [(1, 2), (3, 2)]);
    assert!(setup.private);
    assert_eq!(setup.room_minimum, Some(1));
    assert_eq!(arrangement(&mut world, owner), "0111");
}

/// The owner's of 2026-10-26: the bots "will be replaced with real players
/// once those join". A room of 6 a side with 3 v 3 players and 3 bots a
/// team plays 6 v 6; a member who comes while it is played is not told the
/// match is full: it goes in, in a bot's place on its side (the match
/// server takes the bot out, `game/driven.rs`), the sides still 6 v 6.
#[test]
fn a_member_who_comes_takes_a_bots_place_on_a_full_side() {
    let mut world = World::new();
    let names = ["A", "B", "C", "D", "E", "F"];
    let (key, _) = room_of(&mut world, with_bots(6, 3), None, &names);
    world.advance(VOTE_AND_WAIT);
    let (ticket, setup, id) = last_match(&world);
    assert_eq!(teams(&setup), [(3, 3), (3, 3)]);
    world.host.live.locked().insert(
        ticket,
        LiveView {
            present: (1 << 12) - 1,
            points: None,
            kills: [0, 0],
            played: Some(Duration::from_secs(60)),
            time_left: Some(Duration::from_secs(600)),
        },
    );
    world.host.admitting.locked().insert(ticket);
    let carol = world.connect("Newcomer");
    rows(&mut world, carol, ROOM_JOIN_BASE + key, 1);
    let waiting = state(&mut world, carol);
    assert_eq!(join_row(&waiting), Some("J;15"), "{waiting:?}");
    world.take(carol);
    world.advance(Duration::from_secs(16));
    let (sent, team) = sent_to_match(&world.take(carol)).expect("into the match");
    assert_eq!(sent, ticket);
    let running = &world.core.matches[&id];
    assert_eq!(running.seats.len(), 13);
    assert_eq!(running.seats[12].1, team);
    assert_eq!(
        running.seats[12].0.session(),
        Some(world.ids[carol].1)
    );

    // Without the bots the same sides are full: the newcomer is told so.
    let mut world = World::new();
    let names = ["A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L"];
    let (key, _) = room_of(&mut world, spec(6), None, &names);
    world.advance(VOTE_AND_WAIT);
    let (ticket, setup, _) = last_match(&world);
    assert_eq!(teams(&setup), [(6, 0), (6, 0)]);
    world.host.admitting.locked().insert(ticket);
    let late = world.connect("Late");
    let full = rows(&mut world, late, ROOM_JOIN_BASE + key, 1);
    assert_eq!(header(&full)[1], "N", "a full room takes nobody: {full:?}");
}

/// A movie from before the setting makes a room of no bots: its create
/// codes are the first of the codes.
#[test]
fn a_create_from_an_older_movie_makes_a_room_of_no_bots() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let room = rows(&mut world, alice, create_start(spec(8)), 1);
    assert_eq!(header(&room)[BOTS], "0");
    assert_eq!(
        create_start(with_bots(8, 1)),
        create_start(spec(8)) + survarium::v026g::lobby::rooms::CREATE_BOTS_STEP
    );
}
