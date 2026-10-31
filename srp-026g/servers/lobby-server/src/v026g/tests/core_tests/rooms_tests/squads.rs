//! Squads of two in rooms (`docs/notes/squads.md`, the owner's of
//! 2026-10-31): only the commander acts for the squad (its Play, Join and
//! Queue take the mate along; the soldier's are refused); a room with one
//! place queues the pair as one entry, let in at two places; the late join
//! keeps it on one side; leaving moves only the one who leaves; the lists
//! mark it; a full squad invites nobody.

use super::*;
use session::v026g::rooms::QueueChoice;
use survarium::v026g::lobby::rooms::ROOM_PRESET_BASE;

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
    // The commander is ready (its Play is the squad's: the game greys the
    // soldier's), the soldier while the commander's actions take it.
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
    assert_eq!(ready, Some(vec![true, true]));
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
    // One place: both wait in its queue as one entry, nobody comes in.
    assert_eq!(
        rows(&mut world, alice, ROOM_JOIN_BASE + key, 1),
        [format!("S1;N;0;{key};1")]
    );
    assert_eq!(queued(&world, bob), Some((key, 1)));
    assert_eq!(world.core.rooms.queue_len(key), 1);
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
    // Alice's Join of a room with places takes Bob out of theirs too.
    let other_owner = world.connect("Other");
    let other = create(&mut world, other_owner, spec(6), None);
    rows(&mut world, alice, ROOM_JOIN_BASE + other, 1);
    assert_eq!(room_of(&world, alice), Some(other));
    assert_eq!(room_of(&world, bob), Some(other));
    // The commander leaving the queue takes the pair out; the soldier
    // leaving it leaves its commander's place to it, alone.
    rows(&mut world, alice, ROOM_LEAVE, 1);
    rows(&mut world, bob, ROOM_LEAVE, 1);
    fill(&mut world, key, 1);
    rows(&mut world, alice, ROOM_QUEUE_BASE + key, 1);
    assert_eq!(queued(&world, bob), Some((key, 1)));
    rows(&mut world, alice, ROOM_UNQUEUE, 1);
    assert_eq!((queued(&world, alice), queued(&world, bob)), (None, None));
    rows(&mut world, alice, ROOM_QUEUE_BASE + key, 1);
    rows(&mut world, bob, ROOM_UNQUEUE, 1);
    assert_eq!(
        (queued(&world, alice), queued(&world, bob)),
        (Some((key, 1)), None)
    );
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
    assert_eq!(queued(&world, bob), Some((room, 1)));
    rows(&mut world, members[0], ROOM_LEAVE, 1);
    world.advance(Duration::from_secs(1));
    assert_eq!(room_of(&world, alice), None, "one place is not enough");
    rows(&mut world, members[1], ROOM_LEAVE, 1);
    world.advance(Duration::from_secs(1));
    assert_eq!(room_of(&world, alice), Some(room));
    assert_eq!(room_of(&world, bob), Some(room));
}

/// The state's code (`S1;N;<code>;...`), for one in no room.
fn code(state: &[String]) -> &str {
    state[0].split(';').nth(2).unwrap_or("")
}

#[test]
fn a_soldier_who_leaves_goes_alone_and_comes_back_only_with_its_commander() {
    let mut world = World::new();
    let (alice, bob) = pair(&mut world);
    play(&mut world, alice);
    let room = room_of(&world, alice).expect("a room");
    rows(&mut world, bob, ROOM_LEAVE, 1);
    assert_eq!(room_of(&world, alice), Some(room));
    assert_eq!(room_of(&world, bob), None);
    // His Play is not his own: refused (17), he stays out.
    assert_eq!(code(&rows(&mut world, bob, ROOM_PLAY, 1)), "17");
    assert_eq!(room_of(&world, bob), None);
    // Alice's Join of another room takes him along, out of hers.
    let other_owner = world.connect("Other");
    let other = create(&mut world, other_owner, spec(6), None);
    rows(&mut world, alice, ROOM_JOIN_BASE + other, 1);
    let members = &world.core.rooms.get(other).unwrap().members;
    assert_eq!(
        members,
        &[
            world.ids[other_owner].0,
            world.ids[alice].0,
            world.ids[bob].0
        ]
    );
    // Leaving the squad moves nobody; then Bob is his own again.
    world.ask(bob, request::leave_squad());
    assert_eq!(room_of(&world, alice), Some(other));
    assert_eq!(room_of(&world, bob), Some(other));
    rows(&mut world, bob, ROOM_LEAVE, 1);
    play(&mut world, bob);
    assert!(room_of(&world, bob).is_some());
}

/// The owner's of 2026-10-31: a soldier goes into no room on its own. Its
/// Play, Join (the list, a friend's menu, a PIN), Queue, Switch and a room
/// made are refused (17), "Switch and enter" too, its loadout kept; it may
/// leave its room and its squad.
#[test]
fn a_soldier_goes_into_no_room_on_its_own() {
    let mut world = World::new();
    let owner = world.connect("Owner");
    let open = create(&mut world, owner, spec(6), None);
    let pin_owner = world.connect("PinOwner");
    let private = create(&mut world, pin_owner, spec(6), Some(1234));
    let (_alice, bob) = pair(&mut world);
    let refused = |world: &mut World, start: u32| {
        let answer = rows(world, bob, start, 1);
        assert_eq!(code(&answer), "17", "{start}: {answer:?}");
        assert_eq!(room_of(world, bob), None);
        assert_eq!(queued(world, bob), None);
    };
    refused(&mut world, ROOM_PLAY);
    refused(&mut world, ROOM_JOIN_BASE + open);
    refused(&mut world, ROOM_QUEUE_BASE + open);
    rows(&mut world, bob, ROOM_PIN_BASE + 1234, 1);
    refused(&mut world, ROOM_JOIN_BASE + private);
    let preset = world.account(bob).active_preset;
    refused(&mut world, ROOM_PRESET_BASE + 100_000 + open);
    assert_eq!(world.account(bob).active_preset, preset);
    rows(&mut world, bob, ROOM_PIN_BASE + 4321, 1);
    let made = rows(
        &mut world,
        bob,
        create_start(RoomSpec {
            pin: true,
            ..spec(6)
        }),
        1,
    );
    assert_eq!(code(&made), "17", "{made:?}");
    assert_eq!(room_of(&world, bob), None);
}

/// A soldier in a room of a higher max level is offered no Switch (its
/// commander's would be the squad's); one alone there is.
#[test]
fn a_soldier_is_offered_no_switch() {
    let mut world = World::new();
    let low = world.connect("Low");
    play(&mut world, low);
    let (alice, bob) = pair(&mut world);
    with_gear(&mut world, alice, 12);
    play(&mut world, alice);
    let high = room_of(&world, alice).expect("a room of 15");
    assert_ne!(Some(high), room_of(&world, low));
    assert_eq!(room_of(&world, bob), Some(high));
    let carol = world.connect("Carol");
    rows(&mut world, carol, ROOM_JOIN_BASE + high, 1);
    let switch = |state: &[String]| state.iter().any(|row| row.starts_with("W;"));
    assert!(switch(&state(&mut world, carol)));
    assert!(!switch(&state(&mut world, bob)));
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
    assert_eq!(queued(&world, bob), Some((key, 1)));
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

/// A kept place frees once the room lets its player go (its game gone past
/// the ~90 s, or Leave): 2 v 2, one of team 0 gone (kept, still a member),
/// one of team 1 out of the room but still playing: no side has a place
/// for Carol until the room drops the gone one.
#[test]
fn a_kept_place_frees_when_the_room_lets_its_player_go() {
    let mut world = World::new();
    let (key, ticket, id, players) = playing(&mut world, 2, 4);
    let on = |world: &World, side: u8| {
        players
            .iter()
            .copied()
            .find(|&player| seat_of(world, id, player).1 == side)
            .unwrap()
    };
    let (gone, leaver) = (on(&world, 0), on(&world, 1));
    without(&mut world, ticket, id, &[gone]);
    rows(&mut world, leaver, ROOM_LEAVE, 1);
    let carol = world.connect("Carol");
    rows(&mut world, carol, ROOM_JOIN_BASE + key, 1);
    assert_eq!(room_of(&world, carol), Some(key));
    assert_eq!(go_in(&mut world, ticket, &[carol]), [None]);
    assert_eq!(join_row(&state(&mut world, carol)), Some("J;-1;4"));
    // The room drops the gone one (its session over): its place is free.
    world.core.room_gone(world.ids[gone].0);
    assert_eq!(go_in(&mut world, ticket, &[carol]), [Some(ticket)]);
    assert_eq!(seat_of(&world, id, carol).1, 0);
}

/// A queued pair goes in as one: the first no longer fitting the room
/// (its gear now over the room's bracket) takes both out of the queue, and
/// Play takes both into one room of its bracket; neither is left behind.
#[test]
fn a_queued_pair_that_does_not_fit_leaves_the_queue_together() {
    let mut world = World::new();
    let first = world.connect("First");
    play(&mut world, first);
    let room = room_of(&world, first).expect("a room");
    let members = fill(&mut world, room, 0);
    let (alice, bob) = pair(&mut world);
    play(&mut world, alice);
    assert_eq!(queued(&world, bob), Some((room, 1)));
    with_gear(&mut world, alice, 15);
    rows(&mut world, members[0], ROOM_LEAVE, 1);
    rows(&mut world, members[1], ROOM_LEAVE, 1);
    world.advance(Duration::from_secs(1));
    let (a, b) = (room_of(&world, alice), room_of(&world, bob));
    assert!(a.is_some() && a != Some(room), "{a:?}");
    assert_eq!(a, b);
    assert_eq!((queued(&world, alice), queued(&world, bob)), (None, None));
}

/// The scale-out keeps a pair together at its cut: 15 waiting alone fill
/// the new room but one place, and the pair behind them stays queued, both.
#[test]
fn the_scale_out_keeps_a_pair_together_at_its_cut() {
    let mut world = World::new();
    let first = world.connect("First");
    play(&mut world, first);
    let room = room_of(&world, first).expect("a room");
    fill(&mut world, room, 0);
    let singles: Vec<usize> = (0..15)
        .map(|n| world.connect(&format!("Single{n}")))
        .collect();
    let (alice, bob) = pair(&mut world);
    let now = world.now;
    for &player in &singles {
        world
            .core
            .rooms
            .enqueue_play(world.ids[player].0, 0, room, now)
            .unwrap();
    }
    let mate = Some((world.ids[bob].0, 0));
    world
        .core
        .rooms
        .enqueue_party(world.ids[alice].0, 0, mate, room, now, QueueChoice::Play)
        .unwrap();
    world.advance(Duration::from_secs(1));
    let next = room_of(&world, singles[0]).expect("the new room");
    assert_ne!(next, room);
    assert!(
        singles
            .iter()
            .all(|&player| room_of(&world, player) == Some(next))
    );
    assert_eq!(world.core.rooms.get(next).unwrap().free_places(), 1);
    assert_eq!((room_of(&world, alice), room_of(&world, bob)), (None, None));
    assert!(queued(&world, alice).is_some() && queued(&world, bob).is_some());
}

/// The owner's of 2026-10-31: a pair alone in a room plays no match
/// against itself; the room waits, and a third player starts it, the pair
/// on one side.
#[test]
fn a_pair_alone_waits_and_a_third_starts_the_match_with_the_pair_on_one_side() {
    let mut world = World::new();
    let (alice, bob) = pair(&mut world);
    play(&mut world, alice);
    let room = room_of(&world, alice).expect("a room");
    assert_eq!(room_of(&world, bob), Some(room));
    world.advance(VOTE_AND_WAIT * 2);
    assert!(sent_to_match(&world.take(alice)).is_none());
    assert!(sent_to_match(&world.take(bob)).is_none());
    assert_eq!(header(&state(&mut world, alice))[PHASE], "0", "waiting");
    let carol = world.connect("Carol");
    play(&mut world, carol);
    assert_eq!(room_of(&world, carol), Some(room));
    world.advance(VOTE_AND_WAIT * 2);
    let (_, alice_team) = sent_to_match(&world.take(alice)).expect("Alice's match");
    let (_, bob_team) = sent_to_match(&world.take(bob)).expect("Bob's match");
    let (_, carol_team) = sent_to_match(&world.take(carol)).expect("Carol's match");
    assert_eq!(alice_team, bob_team);
    assert_ne!(alice_team, carol_team);
}

/// The review of #884 (D1): a mate waiting in a queue of its own is not
/// free: its commander's Play leaves it there.
#[test]
fn a_commanders_play_leaves_its_mate_in_a_queue_of_its_own() {
    let mut world = World::new();
    let owner = world.connect("Owner");
    let key = create(&mut world, owner, spec(6), Some(1234));
    let mut n = 0;
    while world.core.rooms.get(key).unwrap().free_places() > 0 {
        let member = world.connect(&format!("M{n}"));
        n += 1;
        rows(&mut world, member, ROOM_PIN_BASE + 1234, 1);
        rows(&mut world, member, ROOM_JOIN_BASE + key, 1);
    }
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    rows(&mut world, bob, ROOM_PIN_BASE + 1234, 1);
    rows(&mut world, bob, ROOM_QUEUE_BASE + key, 1);
    assert_eq!(queued(&world, bob), Some((key, 1)));
    form_squad(&mut world, alice, bob);
    assert!(
        !world.core.squad_ready(world.ids[bob].0),
        "Bob is not ready"
    );
    play(&mut world, alice);
    assert_eq!(
        (queued(&world, bob), room_of(&world, bob)),
        (Some((key, 1)), None)
    );
    assert!(room_of(&world, alice).is_some());
}

/// The review of #884 (D2): a soldier cannot queue, so it never jumps
/// those queued between it and its commander; its commander's own place
/// (queued alone, before the squad) keeps its turn.
#[test]
fn a_soldier_cannot_queue_so_nobody_is_jumped() {
    let mut world = World::new();
    let owner = world.connect("Owner");
    let key = create(&mut world, owner, spec(6), None);
    let members = fill(&mut world, key, 0);
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    rows(&mut world, alice, ROOM_QUEUE_BASE + key, 1);
    let singles: Vec<usize> = (0..3)
        .map(|n| world.connect(&format!("Single{n}")))
        .collect();
    for &single in &singles {
        rows(&mut world, single, ROOM_QUEUE_BASE + key, 1);
    }
    form_squad(&mut world, alice, bob);
    let answer = rows(&mut world, bob, ROOM_QUEUE_BASE + key, 1);
    assert_eq!(code(&answer), "17", "{answer:?}");
    assert_eq!(queued(&world, bob), None);
    rows(&mut world, members[0], ROOM_LEAVE, 1);
    rows(&mut world, members[1], ROOM_LEAVE, 1);
    world.advance(Duration::from_secs(1));
    assert_eq!(room_of(&world, alice), Some(key));
    assert_eq!(room_of(&world, singles[0]), Some(key));
    assert_eq!(room_of(&world, singles[1]), None);
}

/// The review of #884 (D3): at the pair's turn its mate may not come (its
/// gear now over the room's max level): the commander goes in alone, its
/// mate stays where it is (squads.md).
#[test]
fn a_queued_pairs_mate_that_may_not_come_leaves_its_commander_to_go_in_alone() {
    let mut world = World::new();
    let first = world.connect("First");
    play(&mut world, first);
    let room = room_of(&world, first).expect("a room");
    let members = fill(&mut world, room, 0);
    let (alice, bob) = pair(&mut world);
    rows(&mut world, alice, ROOM_QUEUE_BASE + room, 1);
    assert_eq!(queued(&world, bob), Some((room, 1)));
    with_gear(&mut world, bob, 15);
    rows(&mut world, members[0], ROOM_LEAVE, 1);
    rows(&mut world, members[1], ROOM_LEAVE, 1);
    world.advance(Duration::from_secs(1));
    assert_eq!(room_of(&world, alice), Some(room));
    assert_eq!((room_of(&world, bob), queued(&world, bob)), (None, None));
}

/// Starvation, kept aside (the owner has not decided): a pair first in a
/// queue, singles behind it, places freeing one at a time: the singles go
/// in, the pair waits on. What it shows is today's rule, left as it is.
#[test]
#[ignore = "starvation of a queued pair: the owner's decision is pending"]
fn a_queued_pair_starves_behind_singles_taking_single_places() {
    let mut world = World::new();
    let owner = world.connect("Owner");
    let key = create(&mut world, owner, spec(6), None);
    let members = fill(&mut world, key, 0);
    let (alice, bob) = pair(&mut world);
    rows(&mut world, alice, ROOM_QUEUE_BASE + key, 1);
    let singles: Vec<usize> = (0..4)
        .map(|n| world.connect(&format!("Single{n}")))
        .collect();
    for &single in &singles {
        rows(&mut world, single, ROOM_QUEUE_BASE + key, 1);
    }
    for at in 0..4 {
        rows(&mut world, members[at], ROOM_LEAVE, 1);
        world.advance(Duration::from_secs(1));
    }
    let singles_in = singles
        .iter()
        .filter(|&&single| room_of(&world, single) == Some(key))
        .count();
    assert_eq!(
        (room_of(&world, alice), room_of(&world, bob), singles_in),
        (Some(key), Some(key), 2),
        "the pair first in the queue was passed over for every single"
    );
}

/// The owner's of 2026-10-31: a commander's entry into a private room
/// carries the pair, no PIN for the mate: a room it makes, and a join with
/// the PIN; the mate on its team.
#[test]
fn a_commander_brings_its_mate_into_a_private_room_without_a_pin() {
    let mut world = World::new();
    let (alice, bob) = pair(&mut world);
    // A room made: both in, one team.
    let key = create(&mut world, alice, spec(6), Some(1234));
    assert_eq!(room_of(&world, bob), Some(key));
    assert_eq!(arrangement(&mut world, alice), "00");
    // A join with the PIN (Bob gave none): both in, on one team.
    rows(&mut world, alice, ROOM_LEAVE, 1);
    rows(&mut world, bob, ROOM_LEAVE, 1);
    let owner = world.connect("Owner");
    let other = create(&mut world, owner, spec(6), Some(4321));
    rows(&mut world, alice, ROOM_PIN_BASE + 4321, 1);
    rows(&mut world, alice, ROOM_JOIN_BASE + other, 1);
    assert_eq!(room_of(&world, alice), Some(other));
    assert_eq!(room_of(&world, bob), Some(other));
    let room = world.core.rooms.get(other).unwrap();
    let side = |player: usize| room.sides[&world.ids[player].0];
    assert_eq!(side(alice), side(bob));
}

/// A full private room: the commander's join with the PIN puts the pair in
/// its queue, one entry, the mate on its commander's PIN; both come in when
/// two places on one team free.
#[test]
fn a_full_private_room_queues_the_pair_on_its_commanders_pin() {
    let mut world = World::new();
    let owner = world.connect("Owner");
    let key = create(&mut world, owner, spec(6), Some(1234));
    let members = {
        let mut members = Vec::new();
        while world.core.rooms.get(key).unwrap().free_places() > 0 {
            let member = world.connect(&format!("M{}", members.len()));
            rows(&mut world, member, ROOM_PIN_BASE + 1234, 1);
            rows(&mut world, member, ROOM_JOIN_BASE + key, 1);
            members.push(member);
        }
        members
    };
    let (alice, bob) = pair(&mut world);
    rows(&mut world, alice, ROOM_PIN_BASE + 1234, 1);
    assert_eq!(
        rows(&mut world, alice, ROOM_JOIN_BASE + key, 1),
        [format!("S1;N;0;{key};1")]
    );
    assert_eq!(queued(&world, bob), Some((key, 1)));
    // Two of one team leave.
    let team: Vec<usize> = members
        .iter()
        .copied()
        .filter(|&member| world.core.rooms.get(key).unwrap().sides[&world.ids[member].0] == 1)
        .take(2)
        .collect();
    for &member in &team {
        rows(&mut world, member, ROOM_LEAVE, 1);
    }
    world.advance(Duration::from_secs(1));
    assert_eq!(room_of(&world, alice), Some(key));
    assert_eq!(room_of(&world, bob), Some(key));
}

/// The review of 1314956a: a commander's Join of a full room while its mate
/// idles in another room queues the pair, as a Join with places would take
/// both; at their turn both come in, the mate out of its room.
#[test]
fn a_full_rooms_queue_takes_the_mate_from_its_room_at_the_pairs_turn() {
    let mut world = World::new();
    let owner = world.connect("Owner");
    let x = create(&mut world, owner, spec(6), None);
    let members = fill(&mut world, x, 0);
    let other = world.connect("Other");
    let y = create(&mut world, other, spec(6), None);
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    rows(&mut world, bob, ROOM_JOIN_BASE + y, 1);
    form_squad(&mut world, alice, bob);
    assert_eq!(
        rows(&mut world, alice, ROOM_JOIN_BASE + x, 1),
        [format!("S1;N;0;{x};1")]
    );
    assert_eq!(queued(&world, bob), Some((x, 1)));
    assert_eq!(room_of(&world, bob), Some(y), "still in its room meanwhile");
    rows(&mut world, members[0], ROOM_LEAVE, 1);
    rows(&mut world, members[1], ROOM_LEAVE, 1);
    world.advance(Duration::from_secs(1));
    assert_eq!(room_of(&world, alice), Some(x));
    assert_eq!(room_of(&world, bob), Some(x));
    assert!(
        !world
            .core
            .rooms
            .get(y)
            .unwrap()
            .members
            .contains(&world.ids[bob].0)
    );
}

/// The review of 1314956a: a queued pair follows its squad. It breaks and
/// forms again with Bob commanding: the entry splits in two at its place,
/// Alice first, then Bob; at their turns both go in, nobody dropped; the
/// new squad's rules from then on.
#[test]
fn a_queued_pair_follows_its_squad_and_nobody_loses_a_place() {
    let mut world = World::new();
    let owner = world.connect("Owner");
    let key = create(&mut world, owner, spec(6), None);
    let members = fill(&mut world, key, 0);
    let (alice, bob) = pair(&mut world);
    rows(&mut world, alice, ROOM_QUEUE_BASE + key, 1);
    let single = world.connect("Single");
    rows(&mut world, single, ROOM_QUEUE_BASE + key, 1);
    assert_eq!(queued(&world, bob), Some((key, 1)));
    world.ask(bob, request::leave_squad());
    let (alice_id, bob_id) = (world.ids[alice].0, world.ids[bob].0);
    world.store.squads().invite_accepted(bob_id, alice_id);
    world.store.squads().add(bob_id, alice_id).unwrap();
    world.advance(Duration::from_secs(1));
    assert_eq!(queued(&world, alice), Some((key, 1)));
    assert_eq!(queued(&world, bob), Some((key, 2)));
    assert_eq!(queued(&world, single), Some((key, 3)));
    // Places free: both get their turns, in that order.
    rows(&mut world, members[0], ROOM_LEAVE, 1);
    rows(&mut world, members[1], ROOM_LEAVE, 1);
    world.advance(Duration::from_secs(1));
    assert_eq!(room_of(&world, alice), Some(key));
    assert_eq!(room_of(&world, bob), Some(key));
    // The new squad's rules: Alice is Bob's soldier, no room of her own.
    let other_owner = world.connect("Other");
    let other = create(&mut world, other_owner, spec(6), None);
    let refused = rows(&mut world, alice, ROOM_JOIN_BASE + other, 1);
    assert_eq!(header(&refused)[CODE], "17", "{refused:?}");
    rows(&mut world, bob, ROOM_JOIN_BASE + other, 1);
    assert_eq!(room_of(&world, alice), Some(other));
    assert_eq!(queued(&world, single), Some((key, 1)));
}
