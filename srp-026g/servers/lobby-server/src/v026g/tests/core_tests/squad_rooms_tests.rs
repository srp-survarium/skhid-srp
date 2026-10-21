//! Squads in rooms (the owner's of 2026-10-20: squads of two again, with
//! rooms): a commander's Play, Join (the switch is one), create and queue
//! take its squad along, all or none; the draw keeps a squad on one side,
//! a side two players more if it must; a squad goes into its room's match
//! late together, on the side with fewer players; a member who leaves the
//! room or the squad leaves the other where it is.

use session::v026g::rooms::Stage;
use survarium::v026g::lobby::rooms::{ROOM_JOIN_BASE, ROOM_LEAVE, ROOM_PLAY, ROOM_QUEUE_BASE};

use super::rooms_tests::{KEY, MEMBERS, VOTE_AND_WAIT, create, header, rows, spec, state};
use super::*;
use crate::v026g::LiveView;

/// The room the client's account is in.
fn room_of(world: &World, index: usize) -> Option<u32> {
    world.core.rooms.key_of(world.ids[index].0)
}

/// A room of 6 v 6 made by `owner`, with `others` more players in it: 12
/// in all at most.
fn room_with(world: &mut World, owner: usize, others: usize) -> u32 {
    let key = create(world, owner, spec(6), None);
    for n in 0..others {
        let member = world.connect(&format!("Member{n}"));
        rows(world, member, ROOM_JOIN_BASE + key, 1);
    }
    key
}

/// Alice commands a squad of two with Bob. Her Play takes him into the
/// room it finds or makes; his own Play, Join or Leave is his alone.
#[test]
fn a_commanders_play_and_join_take_its_squad_along() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let carol = world.connect("Carol");
    form_squad(&mut world, alice, bob);
    // Play: a new room for both.
    rows(&mut world, alice, ROOM_PLAY, 1);
    let key = room_of(&world, alice).expect("Alice's room");
    assert_eq!(room_of(&world, bob), Some(key));
    assert_eq!(header(&state(&mut world, bob))[MEMBERS], "2");
    // Bob leaves: Alice stays, and he may go elsewhere alone.
    rows(&mut world, bob, ROOM_LEAVE, 1);
    assert_eq!(room_of(&world, alice), Some(key));
    assert_eq!(room_of(&world, bob), None);
    let other = create(&mut world, carol, spec(6), None);
    rows(&mut world, bob, ROOM_JOIN_BASE + other, 1);
    assert_eq!(room_of(&world, bob), Some(other));
    assert_eq!(room_of(&world, alice), Some(key));
    // Alice's Join (as the switch to a lower max level is) takes him
    // back with her, out of Carol's room.
    rows(&mut world, alice, ROOM_LEAVE, 1);
    let third = create(&mut world, carol, spec(6), None);
    let joined = rows(&mut world, alice, ROOM_JOIN_BASE + third, 1);
    assert_eq!(header(&joined)[KEY], third.to_string(), "{joined:?}");
    assert_eq!(room_of(&world, bob), Some(third));
    assert_eq!(header(&joined)[MEMBERS], "3");
    // Bob leaves the squad: both stay in the room.
    world.ask(bob, request::leave_squad());
    assert!(world.store.squads().of(world.ids[alice].0).is_none());
    assert_eq!(room_of(&world, alice), Some(third));
    assert_eq!(room_of(&world, bob), Some(third));
    world.assert_clean();
}

/// A room Alice makes has Bob in it too.
#[test]
fn a_commanders_room_has_its_squad_in_it() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    form_squad(&mut world, alice, bob);
    let key = create(&mut world, alice, spec(6), None);
    assert_eq!(room_of(&world, bob), Some(key));
    let made = state(&mut world, alice);
    assert_eq!(header(&made)[MEMBERS], "2", "{made:?}");
    world.assert_clean();
}

/// A room with one place left takes no squad of two: Join and Queue are
/// refused as by a full room (code 4), nobody moves; Play finds another
/// room for the two. A member alone still joins.
#[test]
fn a_room_without_places_for_the_squad_refuses_it() {
    let mut world = World::new();
    let owner = world.connect("Owner");
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    form_squad(&mut world, alice, bob);
    let key = room_with(&mut world, owner, 10);
    assert_eq!(world.core.rooms.get(key).unwrap().members.len(), 11);
    assert_eq!(
        rows(&mut world, alice, ROOM_JOIN_BASE + key, 1),
        ["S1;N;4;0;0"]
    );
    assert_eq!(
        rows(&mut world, alice, ROOM_QUEUE_BASE + key, 1),
        ["S1;N;4;0;0"]
    );
    assert_eq!(room_of(&world, alice), None);
    assert_eq!(room_of(&world, bob), None);
    assert_eq!(world.core.rooms.queue_place(world.ids[alice].0), None);
    // Play: not that room; a new one for both.
    rows(&mut world, alice, ROOM_PLAY, 1);
    let played = room_of(&world, alice).expect("a room");
    assert_ne!(played, key);
    assert_eq!(room_of(&world, bob), Some(played));
    // Bob alone takes the last place.
    rows(&mut world, bob, ROOM_JOIN_BASE + key, 1);
    assert_eq!(room_of(&world, bob), Some(key));
    world.assert_clean();
}

/// Three squads of two: the draw keeps each on one side, so a side has
/// two players more (the owner's of 2026-10-20: "it is fine").
#[test]
fn the_draw_keeps_each_squad_on_one_side_two_more_if_it_must() {
    let mut world = World::new();
    let players: Vec<usize> = ["A1", "A2", "B1", "B2", "C1", "C2"]
        .into_iter()
        .map(|name| world.connect(name))
        .collect();
    for pair in players.chunks(2) {
        world.take(pair[0]);
        world.take(pair[1]);
        form_squad_named(&mut world, pair[0], pair[1]);
    }
    let key = create(&mut world, players[0], spec(6), None);
    for pair in players[2..].chunks(2) {
        rows(&mut world, pair[0], ROOM_JOIN_BASE + key, 1);
    }
    assert_eq!(world.core.rooms.get(key).unwrap().members.len(), 6);
    world.advance(VOTE_AND_WAIT);
    let setup = world.host.made.locked().last().unwrap().1.clone();
    assert_eq!(setup.players.len(), 6);
    let of = |name: &str| {
        let player = setup
            .players
            .iter()
            .find(|p| p.profile.name == name)
            .unwrap();
        (player.profile.team, player.profile.squad)
    };
    for pair in ["A", "B", "C"] {
        let (team, squad) = of(&format!("{pair}1"));
        assert_eq!(of(&format!("{pair}2")), (team, squad), "{pair}");
        assert_ne!(squad, 0);
    }
    let on_0 = setup.players.iter().filter(|p| p.profile.team == 0).count();
    assert_eq!(on_0.abs_diff(6 - on_0), 2);
}

/// Erin and Fay come as a squad while their room's match of 3 v 2 is
/// played: both go in together, on the side of two, as one squad.
#[test]
fn a_squad_goes_into_its_rooms_match_late_together_on_the_side_of_fewer() {
    let mut world = World::new();
    let players: Vec<usize> = ["Alice", "Bob", "Carol", "Dave", "Gil"]
        .into_iter()
        .map(|name| world.connect(name))
        .collect();
    let erin = world.connect("Erin");
    let fay = world.connect("Fay");
    form_squad_of(&mut world, (erin, "Erin"), (fay, "Fay"));
    let key = create(&mut world, players[0], spec(6), None);
    for &player in &players[1..] {
        rows(&mut world, player, ROOM_JOIN_BASE + key, 1);
    }
    world.advance(VOTE_AND_WAIT);
    let (ticket, _) = sent_to_match(&world.take(players[0])).expect("a match");
    let (&id, running) = world
        .core
        .matches
        .iter()
        .find(|(_, running)| running.ticket.match_id == ticket)
        .unwrap();
    assert_eq!(running.seats.len(), 5);
    let fewer = u8::from(running.seats.iter().filter(|seat| seat.1 == 1).count() < 3);
    assert!(matches!(
        world.core.rooms.get(key).unwrap().stage,
        Stage::InMatch { match_id, .. } if match_id == id
    ));
    // Erin's Join takes Fay; both wait the 15 s, then go in together on
    // one reservation.
    rows(&mut world, erin, ROOM_JOIN_BASE + key, 1);
    assert_eq!(room_of(&world, fay), Some(key));
    world.host.admitting.locked().insert(ticket);
    world.take(erin);
    world.take(fay);
    world.advance(Duration::from_secs(18));
    let (sent_erin, team_erin) = sent_to_match(&world.take(erin)).expect("Erin goes in");
    let (sent_fay, team_fay) = sent_to_match(&world.take(fay)).expect("Fay goes in");
    assert_eq!((sent_erin, sent_fay), (ticket, ticket));
    assert_eq!((team_erin, team_fay), (fewer, fewer));
    let setup = world.host.made.locked().last().unwrap().1.clone();
    assert_eq!(setup.players.len(), 7);
    let squads: Vec<u8> = setup.players[5..].iter().map(|p| p.profile.squad).collect();
    assert_eq!(squads[0], squads[1]);
    assert_ne!(squads[0], 0);
    assert_eq!(world.core.matches[&id].seats.len(), 7);
}

/// Fay goes into the match alone (Erin, her commander, is not in the room
/// yet); Erin comes later and goes to Fay's side, though that side has
/// more players in the match now and a player alone would go to the other.
#[test]
fn one_whose_mate_plays_the_match_goes_to_the_mates_side() {
    let mut world = World::new();
    let players: Vec<usize> = ["Alice", "Bob", "Carol", "Dave"]
        .into_iter()
        .map(|name| world.connect(name))
        .collect();
    let erin = world.connect("Erin");
    let fay = world.connect("Fay");
    form_squad_of(&mut world, (erin, "Erin"), (fay, "Fay"));
    let key = create(&mut world, players[0], spec(6), None);
    for &player in &players[1..] {
        rows(&mut world, player, ROOM_JOIN_BASE + key, 1);
    }
    world.advance(VOTE_AND_WAIT);
    let (ticket, _) = sent_to_match(&world.take(players[0])).expect("a match");
    let id = *world
        .core
        .matches
        .iter()
        .find(|(_, running)| running.ticket.match_id == ticket)
        .unwrap()
        .0;
    // Fay alone (not the commander: Erin stays out).
    world.host.admitting.locked().insert(ticket);
    rows(&mut world, fay, ROOM_JOIN_BASE + key, 1);
    assert_eq!(room_of(&world, erin), None);
    world.take(fay);
    world.advance(Duration::from_secs(18));
    let (_, fay_team) = sent_to_match(&world.take(fay)).expect("Fay goes in");
    // 3 v 2 with Fay; a player of the other side is out of the match now:
    // 3 v 1 as the match stands.
    let seats = world.core.matches[&id].seats.clone();
    assert_eq!(seats.len(), 5);
    let out = seats
        .iter()
        .position(|seat| seat.1 != fay_team)
        .expect("the other side");
    world.host.live.locked().insert(
        ticket,
        LiveView {
            present: 0b11111 & !(1 << out),
            ..LiveView::default()
        },
    );
    world.host.admitting.locked().insert(ticket);
    rows(&mut world, erin, ROOM_JOIN_BASE + key, 1);
    world.take(erin);
    world.advance(Duration::from_secs(18));
    let (_, erin_team) = sent_to_match(&world.take(erin)).expect("Erin goes in");
    assert_eq!(erin_team, fay_team);
}

/// `form_squad` for two clients of any names: `inviter` commands.
fn form_squad_of(world: &mut World, inviter: (usize, &str), invited: (usize, &str)) {
    world.take(inviter.0);
    world.take(invited.0);
    world.ask(inviter.0, request::important(invited.1, 2, "invite"));
    let invite = find(&world.take(invited.0), |message| match message {
        Decoded::Important {
            kind: 2,
            message_id,
            sender_name,
            ..
        } if sender_name == inviter.1 => Some(*message_id),
        _ => None,
    })
    .expect("the invite");
    world.ask(
        invited.0,
        request::important(inviter.1, 3, "membership accepted"),
    );
    world.ask(invited.0, request::answer(invite, 0));
    let accepted = find(&world.take(inviter.0), |message| match message {
        Decoded::Important {
            kind: 3,
            message_id,
            sender_name,
            ..
        } if sender_name == invited.1 => Some(*message_id),
        _ => None,
    })
    .expect("the acceptance");
    world.ask(inviter.0, request::answer(accepted, 0));
    world.ask(inviter.0, request::add_squad_member(invited.1));
    let squad = world
        .store
        .squads()
        .of(world.ids[inviter.0].0)
        .expect("a squad");
    assert_eq!(
        squad.members,
        [world.ids[inviter.0].0, world.ids[invited.0].0]
    );
}

/// [`form_squad_of`] by the names the clients signed in with.
fn form_squad_named(world: &mut World, inviter: usize, invited: usize) {
    let name = |world: &World, index: usize| world.account(index).name.clone();
    let (a, b) = (name(world, inviter), name(world, invited));
    form_squad_of(world, (inviter, &a), (invited, &b));
}
