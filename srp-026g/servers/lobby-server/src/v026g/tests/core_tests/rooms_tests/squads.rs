//! Squads of two in rooms (`docs/notes/squads.md`, the owner's of
//! 2026-10-31): Play and Join take the mate along, a room with one place
//! queues the pair, a queue lets it in at two places, the late join keeps it
//! on one side, leaving moves only the one who leaves, the lists mark it,
//! and a full squad invites nobody.

use super::*;

fn play(world: &mut World, index: usize) {
    rows(world, index, ROOM_PLAY, 1);
}

fn room_of(world: &World, index: usize) -> Option<u32> {
    world.core.rooms.key_of(world.ids[index].0)
}

fn queued(world: &World, index: usize) -> Option<(u32, usize)> {
    world.core.rooms.queue_place(world.ids[index].0)
}

/// The state's squad marks (`S;<marks>`), if any.
fn marks(state: &[String]) -> Option<String> {
    state
        .iter()
        .find_map(|row| row.strip_prefix("S;").map(str::to_owned))
}

/// A squad of Alice (its commander) and Bob.
fn pair(world: &mut World) -> (usize, usize) {
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    form_squad(world, alice, bob);
    (alice, bob)
}

/// Members joined into the room with `key` until it has `free` places.
fn fill(world: &mut World, key: u32, free: usize) -> Vec<usize> {
    let mut members = Vec::new();
    while world.core.rooms.get(key).unwrap().free_places() > free {
        let member = world.connect(&format!("Member{}", members.len()));
        rows(world, member, ROOM_JOIN_BASE + key, 1);
        members.push(member);
    }
    members
}

#[test]
fn the_commanders_play_takes_its_mate_into_one_room_and_the_lists_mark_the_pair() {
    let mut world = World::new();
    let carol = world.connect("Carol");
    play(&mut world, carol);
    let room = room_of(&world, carol).expect("Carol's room");
    let (alice, bob) = pair(&mut world);
    play(&mut world, alice);
    assert_eq!(room_of(&world, alice), Some(room));
    assert_eq!(room_of(&world, bob), Some(room));
    // Carol, then the pair together, marked 1; Carol has no mark.
    let seen = state(&mut world, bob);
    assert_eq!(marks(&seen).as_deref(), Some("011"), "{seen:?}");
    let listed = members(&mut world, carol, room);
    let names: Vec<(&str, &str)> = listed[1..]
        .iter()
        .map(|row| (row[11].as_str(), row[10].as_str()))
        .collect();
    assert_eq!(names, [("Carol", "0"), ("Alice", "1"), ("Bob", "1")]);
    // In a room neither is ready: Play has nothing to take along.
    world.advance(Duration::from_secs(1));
    let decoded = world.take(alice);
    let ready = decoded.iter().rev().find_map(|message| match message {
        Decoded::SquadInfo { members, .. } => Some(
            members
                .iter()
                .map(|member| member.ready)
                .collect::<Vec<_>>(),
        ),
        _ => None,
    });
    assert_eq!(ready, Some(vec![false, false]));
    // A room without a pair has no marks row.
    assert_eq!(marks(&state(&mut world, carol)).as_deref(), Some("011"));
    rows(&mut world, bob, ROOM_LEAVE, 1);
    assert_eq!(marks(&state(&mut world, carol)), None);
}

#[test]
fn a_pairs_join_takes_both_and_one_place_queues_both_until_two_are_free() {
    let mut world = World::new();
    let owner = world.connect("Owner");
    let key = create(&mut world, owner, spec(6), None);
    let members = fill(&mut world, key, 1);
    let (alice, bob) = pair(&mut world);
    // One place: both wait in its queue, nobody comes in.
    assert_eq!(
        rows(&mut world, alice, ROOM_JOIN_BASE + key, 1),
        [format!("S1;N;0;{key};1")]
    );
    assert_eq!(queued(&world, bob), Some((key, 2)));
    world.advance(Duration::from_secs(1));
    assert_eq!(room_of(&world, alice), None);
    assert_eq!(room_of(&world, bob), None);
    // Two places: both in, side by side.
    rows(&mut world, members[0], ROOM_LEAVE, 1);
    world.advance(Duration::from_secs(1));
    assert_eq!(room_of(&world, alice), Some(key));
    assert_eq!(room_of(&world, bob), Some(key));
    let room = world.core.rooms.get(key).unwrap();
    let at = room
        .members
        .iter()
        .position(|member| *member == world.ids[alice].0)
        .unwrap();
    assert_eq!(room.members[at + 1], world.ids[bob].0);
    // Bob's Join of a room with places takes Alice out of hers too.
    let other_owner = world.connect("Other");
    let other = create(&mut world, other_owner, spec(6), None);
    rows(&mut world, bob, ROOM_JOIN_BASE + other, 1);
    assert_eq!(room_of(&world, alice), Some(other));
    assert_eq!(room_of(&world, bob), Some(other));
    // Leaving the queue: the pair came together and goes together.
    rows(&mut world, alice, ROOM_LEAVE, 1);
    rows(&mut world, bob, ROOM_LEAVE, 1);
    fill(&mut world, key, 1);
    rows(&mut world, alice, ROOM_QUEUE_BASE + key, 1);
    assert_eq!(queued(&world, bob), Some((key, 2)));
    rows(&mut world, alice, ROOM_UNQUEUE, 1);
    assert_eq!(queued(&world, bob), None);
}

#[test]
fn a_pairs_play_into_full_rooms_waits_for_two_places() {
    let mut world = World::new();
    let first = world.connect("First");
    play(&mut world, first);
    let room = room_of(&world, first).expect("a room");
    let members = fill(&mut world, room, 0);
    let (alice, bob) = pair(&mut world);
    play(&mut world, alice);
    assert_eq!(queued(&world, alice), Some((room, 1)));
    assert_eq!(queued(&world, bob), Some((room, 2)));
    rows(&mut world, members[0], ROOM_LEAVE, 1);
    world.advance(Duration::from_secs(1));
    assert_eq!(room_of(&world, alice), None, "one place is not enough");
    rows(&mut world, members[1], ROOM_LEAVE, 1);
    world.advance(Duration::from_secs(1));
    assert_eq!(room_of(&world, alice), Some(room));
    assert_eq!(room_of(&world, bob), Some(room));
}

#[test]
fn a_member_who_leaves_goes_alone_and_its_play_brings_it_back_beside_its_mate() {
    let mut world = World::new();
    let (alice, bob) = pair(&mut world);
    play(&mut world, alice);
    let room = room_of(&world, alice).expect("a room");
    rows(&mut world, bob, ROOM_LEAVE, 1);
    assert_eq!(room_of(&world, alice), Some(room));
    assert_eq!(room_of(&world, bob), None);
    // Out of the room Bob is free, Alice is not: his Play is his own, into
    // her room.
    let carol = world.connect("Carol");
    rows(&mut world, carol, ROOM_JOIN_BASE + room, 1);
    play(&mut world, bob);
    assert_eq!(room_of(&world, bob), Some(room));
    let members = &world.core.rooms.get(room).unwrap().members;
    assert_eq!(
        members,
        &[world.ids[alice].0, world.ids[bob].0, world.ids[carol].0]
    );
    // Her room full, his Play waits in its queue and comes in beside her.
    rows(&mut world, bob, ROOM_LEAVE, 1);
    let others = fill(&mut world, room, 0);
    play(&mut world, bob);
    assert_eq!(queued(&world, bob), Some((room, 1)));
    rows(&mut world, others[0], ROOM_LEAVE, 1);
    world.advance(Duration::from_secs(1));
    let members = &world.core.rooms.get(room).unwrap().members;
    assert_eq!(&members[..2], &[world.ids[alice].0, world.ids[bob].0]);
    // Leaving the squad moves nobody.
    world.ask(bob, request::leave_squad());
    assert_eq!(room_of(&world, alice), Some(room));
    assert_eq!(room_of(&world, bob), Some(room));
}

#[test]
fn a_pair_that_comes_while_the_match_is_played_goes_in_on_one_side() {
    let mut world = World::new();
    let owner = world.connect("Owner");
    let other = world.connect("Other");
    let key = create(&mut world, owner, spec(6), None);
    rows(&mut world, other, ROOM_JOIN_BASE + key, 1);
    world.advance(VOTE_AND_WAIT);
    let (ticket, _) = sent_to_match(&world.take(owner)).expect("a match");
    let (&id, _) = world
        .core
        .matches
        .iter()
        .find(|(_, running)| running.ticket.match_id == ticket)
        .unwrap();
    let (alice, bob) = pair(&mut world);
    rows(&mut world, alice, ROOM_JOIN_BASE + key, 1);
    assert_eq!(room_of(&world, bob), Some(key));
    let teams = late_teams(&mut world, ticket, id, &[alice, bob]);
    assert_eq!(teams.len(), 2);
    assert_eq!(teams[0], teams[1], "{teams:?}");
}

#[test]
fn a_squad_of_two_invites_nobody_more() {
    let mut world = World::new();
    let (alice, bob) = pair(&mut world);
    let carol = world.connect("Carol");
    world.take(carol);
    for (inviter, name) in [(alice, "Alice"), (bob, "Bob")] {
        world.ask(inviter, request::important("Carol", 2, "invite"));
        assert!(importants(&world.take(carol), 2, name).is_empty());
    }
    world.ask(alice, request::add_squad_member("Carol"));
    assert_eq!(
        world.store.squads().of(world.ids[alice].0).unwrap().members,
        [world.ids[alice].0, world.ids[bob].0]
    );
}

/// A room of `count` players, `size` a side, playing its match: the room,
/// the match's ticket and id, and the players.
fn playing(world: &mut World, size: u8, count: usize) -> (u32, u32, u32, Vec<usize>) {
    let players: Vec<usize> = (0..count)
        .map(|n| world.connect(&format!("Player{n}")))
        .collect();
    let key = create(world, players[0], spec(size), None);
    for &player in &players[1..] {
        rows(world, player, ROOM_JOIN_BASE + key, 1);
    }
    world.advance(VOTE_AND_WAIT);
    let (ticket, _) = sent_to_match(&world.take(players[0])).expect("a match");
    for &player in &players[1..] {
        assert!(sent_to_match(&world.take(player)).is_some());
    }
    let id = *world
        .core
        .matches
        .iter()
        .find(|(_, running)| running.ticket.match_id == ticket)
        .unwrap()
        .0;
    (key, ticket, id, players)
}

/// The match shows the places of `out` (players) not in it.
fn without(world: &mut World, ticket: u32, id: u32, out: &[usize]) {
    let mut present = (1_u32 << world.core.matches[&id].seats.len()) - 1;
    for &player in out {
        present &= !(1 << seat_of(world, id, player).0);
    }
    world.host.live.locked().insert(
        ticket,
        LiveView {
            present,
            played: Some(Duration::from_secs(60)),
            time_left: Some(Duration::from_secs(600)),
            ..LiveView::default()
        },
    );
}

/// The player leaves the match by its own word: its place is given up.
fn quits(world: &mut World, id: u32, player: usize) {
    assert_eq!(
        world.store.matchmaking().leave_match(world.ids[player].0),
        Some(id)
    );
    world.ask(player, request::leave(1, true));
    world.advance(Duration::from_secs(1));
}

/// Whether `players` go into the match within 20 s, it taking joiners.
fn go_in(world: &mut World, ticket: u32, players: &[usize]) -> Vec<Option<u32>> {
    let mut sent = vec![None; players.len()];
    for _ in 0..20 {
        world.host.admitting.locked().insert(ticket);
        world.advance(Duration::from_secs(1));
        for (at, &player) in players.iter().enumerate() {
            if let Some((ticket, _)) = sent_to_match(&world.take(player)) {
                sent[at] = Some(ticket);
            }
        }
    }
    sent
}

/// An open room has no teams before its match: two places take a pair,
/// and the sides drawn at the start keep it on one.
#[test]
fn an_open_room_before_its_match_takes_a_pair_on_one_side_at_the_start() {
    let mut world = World::new();
    let owner = world.connect("Owner");
    let other = world.connect("Other");
    let key = create(&mut world, owner, spec(2), None);
    rows(&mut world, other, ROOM_JOIN_BASE + key, 1);
    let (alice, bob) = pair(&mut world);
    rows(&mut world, alice, ROOM_JOIN_BASE + key, 1);
    assert_eq!(room_of(&world, bob), Some(key));
    world.advance(VOTE_AND_WAIT);
    let (_, alice_team) = sent_to_match(&world.take(alice)).expect("Alice's match");
    let (_, bob_team) = sent_to_match(&world.take(bob)).expect("Bob's match");
    let (_, owner_team) = sent_to_match(&world.take(owner)).expect("the owner's match");
    assert_eq!(alice_team, bob_team);
    assert_ne!(alice_team, owner_team);
}

/// A private room of 9 v 9: one place on each team is no place for a pair:
/// it waits in the queue, and comes in on a team that has two.
#[test]
fn a_private_room_with_one_place_on_each_team_queues_the_pair() {
    let mut world = World::new();
    let members: Vec<usize> = (0..18)
        .map(|n| world.connect(&format!("Member{n}")))
        .collect();
    let key = create(&mut world, members[0], spec(6), Some(1234));
    for &member in &members[1..] {
        rows(&mut world, member, ROOM_PIN_BASE + 1234, 1);
        rows(&mut world, member, ROOM_JOIN_BASE + key, 1);
    }
    let owner = members[0];
    assert_eq!(arrangement(&mut world, owner), "010101010101010101");
    let (alice, bob) = pair(&mut world);
    rows(&mut world, alice, ROOM_PIN_BASE + 1234, 1);
    assert_eq!(
        rows(&mut world, alice, ROOM_JOIN_BASE + key, 1),
        [format!("S1;N;0;{key};1")]
    );
    assert_eq!(queued(&world, bob), Some((key, 2)));
    world.advance(Duration::from_secs(1));
    assert_eq!(room_of(&world, alice), None);
    // A member of team 2 leaves: both come there, together.
    rows(&mut world, members[1], ROOM_LEAVE, 1);
    world.advance(Duration::from_secs(1));
    assert_eq!(room_of(&world, alice), Some(key));
    assert_eq!(room_of(&world, bob), Some(key));
    let room = world.core.rooms.get(key).unwrap();
    let side = |player: usize| room.sides[&world.ids[player].0];
    assert_eq!((side(alice), side(bob)), (1, 1));
}

/// A match being played with one place on each side: the pair waits in
/// the room (told the match is full) and goes in together once a side has
/// two.
#[test]
fn a_running_match_with_one_place_on_each_side_takes_the_pair_only_on_a_side_of_two() {
    let mut world = World::new();
    let (key, ticket, id, players) = playing(&mut world, 2, 2);
    without(&mut world, ticket, id, &[]);
    let (alice, bob) = pair(&mut world);
    rows(&mut world, alice, ROOM_JOIN_BASE + key, 1);
    assert_eq!(room_of(&world, bob), Some(key));
    assert_eq!(go_in(&mut world, ticket, &[alice, bob]), [None, None]);
    assert_eq!(join_row(&state(&mut world, alice)), Some("J;-1;4"));
    // One of the two leaves the match: its side has two places.
    let team = seat_of(&world, id, players[1]).1;
    quits(&mut world, id, players[1]);
    without(&mut world, ticket, id, &[players[1]]);
    assert_eq!(
        go_in(&mut world, ticket, &[alice, bob]),
        [Some(ticket), Some(ticket)]
    );
    assert_eq!(seat_of(&world, id, alice).1, team);
    assert_eq!(seat_of(&world, id, bob).1, team);
}

/// A place is kept for a player whose game broke (not in the match, its
/// seat held: Play takes it back there): its side counts it, for one alone
/// as for a pair, so nobody fills a side it comes back to.
#[test]
fn a_place_kept_for_a_player_who_may_come_back_counts_on_its_side() {
    // 3 v 3: of team 0 two are gone (their places kept), one of team 1
    // left for good: 1 v 2 in the match. Team 0 has the fewer players
    // but no place (1 + 2 kept of 3): one alone goes to team 1.
    let mut world = World::new();
    let (key, ticket, id, players) = playing(&mut world, 3, 6);
    let team = |world: &World, side: u8| -> Vec<usize> {
        players
            .iter()
            .copied()
            .filter(|&player| seat_of(world, id, player).1 == side)
            .collect()
    };
    let (team_0, team_1) = (team(&world, 0), team(&world, 1));
    quits(&mut world, id, team_1[0]);
    without(&mut world, ticket, id, &[team_0[0], team_0[1], team_1[0]]);
    let carol = world.connect("Carol");
    rows(&mut world, carol, ROOM_JOIN_BASE + key, 1);
    assert_eq!(go_in(&mut world, ticket, &[carol]), [Some(ticket)]);
    assert_eq!(seat_of(&world, id, carol).1, 1);

    // A pair: 3 v 3, one of each team left for good and one more of team 0
    // gone (its place kept): 1 v 2 in the match. Two places on team 0
    // counting the players only; one counting the kept place, and one on
    // team 1: the pair waits.
    let mut world = World::new();
    let (key, ticket, id, players) = playing(&mut world, 3, 6);
    let team = |world: &World, side: u8| -> Vec<usize> {
        players
            .iter()
            .copied()
            .filter(|&player| seat_of(world, id, player).1 == side)
            .collect()
    };
    let (team_0, team_1) = (team(&world, 0), team(&world, 1));
    quits(&mut world, id, team_0[0]);
    quits(&mut world, id, team_1[0]);
    without(&mut world, ticket, id, &[team_0[0], team_0[1], team_1[0]]);
    let (alice, bob) = pair(&mut world);
    rows(&mut world, alice, ROOM_JOIN_BASE + key, 1);
    assert_eq!(room_of(&world, bob), Some(key));
    assert_eq!(go_in(&mut world, ticket, &[alice, bob]), [None, None]);
    assert_eq!(join_row(&state(&mut world, alice)), Some("J;-1;4"));
    // Its seat given up (it left for good): two places on team 0.
    quits(&mut world, id, team_0[1]);
    assert_eq!(
        go_in(&mut world, ticket, &[alice, bob]),
        [Some(ticket), Some(ticket)]
    );
    assert_eq!(seat_of(&world, id, alice).1, 0);
    assert_eq!(seat_of(&world, id, bob).1, 0);
}
