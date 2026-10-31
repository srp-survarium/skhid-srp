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
