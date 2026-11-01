//! Rooms (`rooms`): a room made, listed and joined; its countdown from two
//! players, the vote and the match it starts on the card chosen; the room
//! again after the match; Play into a room; a PIN, a max level and a kick;
//! a late joiner; and no room for an older game.

use session::v026g::rooms::Stage;
use session::v026g::{matchmaking, quests};
use survarium::v026g::lobby::rooms::{
    ROOM_FORM, ROOM_FRIENDS, ROOM_JOIN_BASE, ROOM_JOIN_MATCH, ROOM_LEAVE, ROOM_MEMBERS_BASE,
    ROOM_PIN_BASE, ROOM_PLAY, ROOM_QUEUE_BASE, ROOM_START, ROOM_STATE, ROOM_UNQUEUE,
    ROOM_VOTE_BASE, ROOMS_LIST, RoomSpec, bits_start, create_start, kick_start, move_start,
};

use super::pvb_tests::with_gear;
use super::*;
use crate::v026g::LiveView;

mod scaling;
mod squads;

/// Fields of the state's header (`S1;R;<key>;<level>;<max level>;<size>;
/// <wait>;<phase>;<secs>;<owner>;<pool>;<members>;<serial>;<pin>;<next map>;
/// <min players>;<code>`).
const KEY: usize = 2;
const LEVEL: usize = 3;
const PHASE: usize = 7;
const OWNER: usize = 9;
const MEMBERS: usize = 11;
const SERIAL: usize = 12;
const NEXT: usize = 14;
const CODE: usize = 16;
/// From two players to the match: the vote's 15 s, the wait of [`spec`]
/// (15 s), and a little more.
const VOTE_AND_WAIT: Duration = Duration::from_secs(32);

/// The names of the rows of the one answer to a request at `start`.
pub(super) fn rows(world: &mut World, index: usize, start: u32, count: u8) -> Vec<String> {
    let answers = answered(world, index, start, count);
    let [rows] = &answers[..] else {
        panic!("not one answer: {answers:?}");
    };
    let first = rows[0].1;
    for (i, (_, place)) in rows.iter().enumerate() {
        assert_eq!(*place, first + i as u32);
    }
    rows.iter().map(|(name, _)| name.clone()).collect()
}

fn state(world: &mut World, index: usize) -> Vec<String> {
    rows(world, index, ROOM_STATE, 1)
}

/// The fields of the state's header.
fn header(state: &[String]) -> Vec<String> {
    state[0].split(';').map(str::to_owned).collect()
}

/// A room of `size` a side, any equipment, a wait of 15 s.
pub(super) fn spec(size: u8) -> RoomSpec {
    RoomSpec {
        size,
        min_side: 1,
        max_level: 0,
        wait: 15,
        pin: false,
    }
}

/// The asker makes a room (with `pin` asked for, given first): its key.
pub(super) fn create(world: &mut World, index: usize, spec: RoomSpec, pin: Option<u16>) -> u32 {
    // Public rooms are server-created now. Lifecycle tests can still set
    // up a particular size/pool directly; the wire refusal is tested below.
    if pin.is_none() {
        let entries: Vec<_> = world
            .core
            .room_entries()
            .iter()
            .map(|entry| entry.map_id)
            .collect();
        let default: Vec<_> = world
            .core
            .room_entries()
            .iter()
            .filter(|entry| !entry.extra)
            .map(|entry| entry.map_id)
            .collect();
        let account_id = world.ids[index].0;
        let gear = matchmaking::gear_level(
            &world.core.accounts.get(account_id).unwrap().locked(),
            quests::now(),
        );
        let rng = &mut world.core.rng;
        return world
            .core
            .rooms
            .create(
                account_id,
                gear,
                session::v026g::rooms::Spec {
                    size: spec.size,
                    wait: spec.wait,
                    door: session::v026g::rooms::Door::Open {
                        max_level: spec.max_level,
                    },
                    min_players: spec.min_side * 2,
                },
                &entries,
                &default,
                world.core.settings.rooms.max_rooms,
                world.now,
                &mut |bound| rng.below(bound as u32) as usize,
            )
            .unwrap();
    }
    if let Some(pin) = pin {
        assert_eq!(
            rows(world, index, ROOM_PIN_BASE + u32::from(pin), 1),
            ["S1;K;1"]
        );
    }
    let spec = RoomSpec {
        pin: pin.is_some(),
        ..spec
    };
    let state = rows(world, index, create_start(spec), 1);
    let header = header(&state);
    assert_eq!(header[1], "R", "{state:?}");
    header[KEY].parse().unwrap()
}

/// The cards of a state: their maps.
fn cards(state: &[String]) -> Vec<u8> {
    state
        .iter()
        .filter(|row| row.starts_with("C;"))
        .map(|row| row.split(';').nth(2).unwrap().parse().unwrap())
        .collect()
}

/// The owner's flow: Alice makes a room, Bob finds it in the list and
/// joins; the vote's 15 s start with two, both vote, its winner is the next
/// map and the room's wait runs; when it is over both go to one match on
/// that map, one a side; after it they are in the room again and the next
/// vote runs.
#[test]
fn a_room_counts_down_from_two_and_plays_the_card_voted_for() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let key = create(&mut world, alice, spec(6), None);
    let alone = state(&mut world, alice);
    let serial = header(&alone)[SERIAL].clone();
    assert_eq!(
        alone[0],
        format!(
            "S1;R;{key};0;0;6;15;0;0;1;{};1;{serial};0;0;2;0",
            world.core.room_entries().len()
        )
    );
    // The vote is there from the start, alone too.
    assert_eq!(cards(&alone).len(), 3);
    // Equipment level 0, the owner, the asker, not in a match.
    assert_eq!(alone[4], "P;0;0;1;1;0;0;0;Alice");
    assert_eq!(alone.last().unwrap(), "L;0");
    // Nothing runs alone.
    world.advance(Duration::from_secs(5));
    assert_eq!(header(&state(&mut world, alice))[PHASE], "0");

    let list = rows(&mut world, bob, ROOMS_LIST, 32);
    assert_eq!(list[0], "S1;L;1;0");
    // No vote yet: no map (the start would draw one).
    assert_eq!(list[1], format!("{key};0;0;6;1;0;0;0;0;Alice"));
    let joined = rows(&mut world, bob, ROOM_JOIN_BASE + key, 1);
    assert_eq!(header(&joined)[MEMBERS], "2");
    assert_eq!(header(&joined)[OWNER], "0", "Bob does not own it");
    assert_ne!(header(&joined)[SERIAL], serial);

    world.advance(Duration::from_secs(1));
    let counting = state(&mut world, alice);
    assert_eq!(header(&counting)[PHASE], "3", "the vote");
    assert_eq!(header(&counting)[NEXT], "0");
    let voted_cards = cards(&counting);
    assert_eq!(voted_cards.len(), 3);
    // Alice changes her mind: one vote, the last.
    rows(&mut world, alice, ROOM_VOTE_BASE + 2, 1);
    rows(&mut world, alice, ROOM_VOTE_BASE + 1, 1);
    let voted = rows(&mut world, bob, ROOM_VOTE_BASE + 1, 1);
    assert!(
        voted.contains(&format!("C;2;{};0;0", voted_cards[2])),
        "{voted:?}"
    );
    assert!(
        voted.contains(&format!("C;1;{};2;1", voted_cards[1])),
        "{voted:?}"
    );
    // The list shows the card that leads alone.
    let map_of = |list: &[String]| list[1].split(';').nth(7).unwrap().to_owned();
    let list = rows(&mut world, bob, ROOMS_LIST, 32);
    assert_eq!(map_of(&list), voted_cards[1].to_string());

    // Past the vote's 15 s it is still open, through the wait, until the
    // match starts (the owner's of 2026-10-19); no map is fixed before.
    world.advance(Duration::from_secs(15));
    let waiting = state(&mut world, alice);
    assert_eq!(header(&waiting)[PHASE], "3", "still the vote: {waiting:?}");
    assert_eq!(header(&waiting)[NEXT], "0");
    rows(&mut world, alice, ROOM_VOTE_BASE + 2, 1);
    // One vote each: a tie, no map in the list (the start draws one).
    let list = rows(&mut world, bob, ROOMS_LIST, 32);
    assert_eq!(map_of(&list), "0");
    let late = rows(&mut world, bob, ROOM_VOTE_BASE + 2, 1);
    assert!(
        late.contains(&format!("C;2;{};2;1", voted_cards[2])),
        "{late:?}"
    );
    world.take(alice);
    world.take(bob);
    world.advance(Duration::from_secs(16));
    let to_alice = sent_to_match(&world.take(alice)).expect("Alice is sent to the match");
    let to_bob = sent_to_match(&world.take(bob)).expect("Bob is sent to the match");
    assert_eq!(to_alice.0, to_bob.0);
    assert_ne!(to_alice.1, to_bob.1, "one a side");
    let setup = world.host.made.locked().last().unwrap().1.clone();
    let blackout_map = maps::blackout_played(voted_cards[2]);
    assert_eq!(
        setup.map_id,
        blackout_map.unwrap_or(voted_cards[2]),
        "the vote's winner when it started"
    );
    assert_eq!(setup.blackout, blackout_map.is_some());
    assert_eq!(setup.players.len(), 2, "no bots");
    assert!(!setup.vs_bots);
    assert!(!setup.private, "no PIN: it pays");
    world.advance(Duration::from_secs(1));
    let playing = state(&mut world, alice);
    assert_eq!(header(&playing)[PHASE], "2");
    // The vote for the next match is there while this one is played.
    assert_eq!(cards(&playing).len(), 3);
    assert!(
        playing
            .iter()
            .filter(|row| row.starts_with("P;"))
            .all(|row| row.contains(";1;0;0;Alice") || row.contains(";1;0;0;Bob"))
    );

    world.end_match(to_alice.0, 0);
    world.advance(Duration::from_secs(2));
    let after = state(&mut world, alice);
    assert_eq!(header(&after)[PHASE], "3", "the next vote: {after:?}");
    assert_eq!(after.last().unwrap(), &format!("L;{}", voted_cards[2]));
    assert_eq!(cards(&after).len(), 3);
}

/// Play makes a room for the first, and the second goes into it; both are
/// told they are in the lobby.
#[test]
fn play_puts_a_game_with_rooms_into_an_open_room() {
    let mut world = World::new();
    let carol = world.connect("Carol");
    let dave = world.connect("Dave");
    for index in [carol, dave] {
        assert_eq!(state(&mut world, index), ["S1;N;0;0;0"]);
    }
    world.take(carol);
    world.ask(carol, request::play(1));
    world.advance(Duration::from_millis(100));
    assert!(matches!(
        queue_state(&world.take(carol)),
        Some(ClientState::Lobby)
    ));
    let made = state(&mut world, carol);
    // Its own equipment level (0), any max level, 8 v 8, 30 s.
    assert!(made[0].starts_with("S1;R;"), "{made:?}");
    let key = header(&made)[KEY].clone();
    assert_eq!(&header(&made)[LEVEL..PHASE], ["0", "10", "8", "30"]);
    world.ask(dave, request::play(1));
    world.advance(Duration::from_millis(100));
    let joined = state(&mut world, dave);
    assert_eq!(header(&joined)[KEY], key);
    assert_eq!(header(&joined)[MEMBERS], "2");
}

/// Play leaves nobody out for its level: a player far above the only open
/// room goes into it; with two, each goes into the one nearest its level
/// (the owner's of 2026-10-18).
#[test]
fn play_takes_the_nearest_room_within_its_bracket() {
    let mut world = World::new();
    let carol = world.connect("Carol");
    let dave = world.connect("Dave");
    let erin = world.connect("Erin");
    let fay = world.connect("Fay");
    let play = |world: &mut World, index| {
        world.ask(index, request::play(1));
        world.advance(Duration::from_millis(100));
        header(&state(world, index))[KEY].clone()
    };
    let low = play(&mut world, carol);
    with_gear(&mut world, dave, 6);
    assert_eq!(play(&mut world, dave), low, "the only room, however far");
    // Dave leaves and makes a room of his own: one low, one high.
    rows(&mut world, dave, ROOM_LEAVE, 1);
    let high = create(
        &mut world,
        dave,
        RoomSpec {
            max_level: 10,
            ..spec(8)
        },
        None,
    )
    .to_string();
    assert_ne!(high, low);
    with_gear(&mut world, erin, 6);
    assert_eq!(play(&mut world, erin), high);
    assert_eq!(play(&mut world, fay), low);
}

/// A member whose game is started anew (closed or crashed: a new sign-in,
/// a new session) is still in its room.
#[test]
fn a_game_started_anew_keeps_its_room() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let key = create(&mut world, alice, spec(8), None);
    rows(&mut world, bob, ROOM_JOIN_BASE + key, 1);
    let (account_id, _) = world.ids[bob];
    let session = world.new_session(account_id);
    let again = world.connect_session((account_id, session));
    world.take(again);
    assert_eq!(
        world.core.rooms.of(account_id).map(|room| room.key),
        Some(key)
    );
    assert_eq!(header(&state(&mut world, alice))[MEMBERS], "2");
}

/// A member whose game is closed (not from its menu) and started again
/// within the 90 s keeps its place, but its new game, which does not know
/// of the room yet, is in no match of it (shown grey) until it asks where
/// it is; then it plays as before. Live, 2026-10-20: a game that showed no
/// room was pulled into the room's matches twice.
#[test]
fn a_game_started_anew_plays_no_match_of_its_room_until_it_is_told_the_room() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let key = create(&mut world, alice, spec(6), None);
    rows(&mut world, bob, ROOM_JOIN_BASE + key, 1);
    // Bob's game is killed; a new one signs in 10 s later.
    world.silent_for(bob, Duration::from_secs(10));
    let (account_id, _) = world.ids[bob];
    let session = world.new_session(account_id);
    let again = world.connect_session((account_id, session));
    let room = state(&mut world, alice);
    assert_eq!(header(&room)[MEMBERS], "2", "the place is kept: {room:?}");
    assert!(
        room.iter().any(|row| row.ends_with(";0;1;Bob")),
        "grey: {room:?}"
    );
    world.take(alice);
    world.take(again);
    world.advance(VOTE_AND_WAIT);
    assert!(
        sent_to_match(&world.take(again)).is_none(),
        "not told the room"
    );
    assert!(
        sent_to_match(&world.take(alice)).is_none(),
        "no match of one"
    );
    assert!(world.host.made.locked().is_empty());
    // Its movie asks where it is: the room, and from now on it plays.
    let told = state(&mut world, again);
    assert_eq!(header(&told)[KEY], key.to_string(), "{told:?}");
    assert!(told.iter().any(|row| row.ends_with(";0;0;Bob")), "{told:?}");
    world.advance(VOTE_AND_WAIT);
    let (to_alice, _) = sent_to_match(&world.take(alice)).expect("Alice plays");
    let (to_bob, _) = sent_to_match(&world.take(again)).expect("Bob plays");
    assert_eq!(to_alice, to_bob);
}

/// A game started anew that is never told its room loses the place 90 s
/// after it signed in, as a lost one does, and is never sent to a match.
#[test]
fn a_game_started_anew_and_never_told_its_room_loses_it_in_90_s() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let key = create(&mut world, alice, spec(6), None);
    rows(&mut world, bob, ROOM_JOIN_BASE + key, 1);
    let (account_id, _) = world.ids[bob];
    let session = world.new_session(account_id);
    let again = world.connect_session((account_id, session));
    world.take(again);
    world.advance(Duration::from_secs(80));
    assert_eq!(header(&state(&mut world, alice))[MEMBERS], "2");
    world.advance(Duration::from_secs(10));
    assert_eq!(header(&state(&mut world, alice))[MEMBERS], "1");
    assert!(sent_to_match(&world.take(again)).is_none());
    assert!(world.core.rooms.of(account_id).is_none());
    assert!(state(&mut world, again)[0].starts_with("S1;N;"));
}

/// A member seated in its room's match whose game is started anew goes
/// back to its seat by Play, as before; its new game is not sent to the
/// match on its own.
#[test]
fn a_game_started_anew_goes_back_to_its_seat_in_the_rooms_match_by_play() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let key = create(&mut world, alice, spec(6), None);
    rows(&mut world, bob, ROOM_JOIN_BASE + key, 1);
    world.take(alice);
    world.take(bob);
    world.advance(VOTE_AND_WAIT);
    let (match_id, _) = sent_to_match(&world.take(bob)).expect("a match");
    let (account_id, _) = world.ids[bob];
    let session = world.new_session(account_id);
    let again = world.connect_session((account_id, session));
    world.take(again);
    world.advance(Duration::from_secs(20));
    assert!(sent_to_match(&world.take(again)).is_none());
    world.ask(again, request::ranking(ROOM_PLAY, 1));
    let (back, _) = sent_to_match(&world.take(again)).expect("back to its seat");
    assert_eq!(back, match_id);
}

/// The owner's of 2026-11-01 ("you can leave the game, then enter, and you
/// will be in room, but you wouldn't be able to enter (however, you can
/// through pressing PLAY)"; live, rev40: one relaunched in its room's match
/// tried other rooms for a minute, then pressed Play): a member seated in
/// its room's match whose game is started anew, once told its room, is
/// offered Join match with the seconds, as any member who comes during the
/// match, and Join match takes it back to its seat, whatever the match
/// takes now.
#[test]
fn a_game_started_anew_goes_back_to_its_seat_by_the_rooms_join_match() {
    let mut world = World::new();
    let (_, ticket, _, players) = room_match_of_four(&mut world);
    let again = restart_game(&mut world, players[1]);
    // The match takes no newcomer now: its own seat is not a new place.
    world
        .host
        .closed
        .locked()
        .insert(ticket, session::matchmaking_policy::Closed::Joining);
    let told = state(&mut world, again);
    assert_eq!(join_row(&told), Some("J;15"), "{told:?}");
    world.ask(again, request::ranking(ROOM_JOIN_MATCH, 1));
    let (back, _) = sent_to_match(&world.take(again)).expect("back to its seat");
    assert_eq!(back, ticket);
}

/// The same member that waits goes back to its seat when its 15 s are over,
/// as any member who comes during the match goes in.
#[test]
fn a_game_started_anew_goes_back_to_its_seat_after_the_rooms_wait() {
    let mut world = World::new();
    let (_, ticket, _, players) = room_match_of_four(&mut world);
    let again = restart_game(&mut world, players[1]);
    state(&mut world, again);
    world.advance(Duration::from_secs(10));
    assert!(
        sent_to_match(&world.take(again)).is_none(),
        "the room first"
    );
    world.advance(Duration::from_secs(6));
    let (back, _) = sent_to_match(&world.take(again)).expect("back to its seat");
    assert_eq!(back, ticket);
}

/// The owner's of 2026-11-01 ("someone can leave on purpose only through
/// leave match button in menu", "and that should just kick the player out of
/// the room"): the in-game menu's Leave match (0xE0) gives the seat up and
/// takes the member out of its room, so the room's 15 s never take it back,
/// even when its client's word to the lobby comes before the match server's.
#[test]
fn one_who_leaves_by_the_menu_is_out_of_the_room_and_never_taken_back() {
    let mut world = World::new();
    let (key, ticket, id, players) = room_match_of_four(&mut world);
    let bob = players[1];
    let account = world.ids[bob].0;
    world.host.admitting.locked().insert(ticket);
    // Back in the lobby first, a tick, then the match server's 0xE0.
    world.ask(bob, request::leave(1, true));
    world.advance(Duration::from_secs(1));
    assert_eq!(world.store.matchmaking().leave_match(account), Some(id));
    world.advance(Duration::from_secs(20));
    assert!(sent_to_match(&world.take(bob)).is_none(), "not taken back");
    assert_eq!(world.core.rooms.key_of(account), None);
    let room = world.core.rooms.get(key).unwrap();
    assert!(!room.members.contains(&account));
    assert_eq!(state(&mut world, bob), ["S1;N;0;0;0"]);
}

/// A member is offered the open room of the lowest max level that takes it,
/// lower than its own room's, and one click (a Join) moves it there; none
/// over its equipment, none with a PIN, none in its own room, none out of a
/// private room (the owner's of 2026-10-19).
#[test]
fn a_member_is_offered_a_room_of_a_lower_max_level() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let carol = world.connect("Carol");
    let dave = world.connect("Dave");
    let erin = world.connect("Erin");
    let switch_of = |rows: &[String]| {
        rows.iter()
            .find(|row| row.starts_with("W;"))
            .map(|row| row.split(';').map(str::to_owned).collect::<Vec<_>>())
    };
    // Alice's room has no max level; Bob is in it.
    let any = create(
        &mut world,
        alice,
        RoomSpec {
            max_level: 15,
            ..spec(8)
        },
        None,
    );
    rows(&mut world, bob, ROOM_JOIN_BASE + any, 1);
    assert_eq!(switch_of(&state(&mut world, bob)), None);
    // Carol's takes up to level 5, Dave's up to 3 with a PIN, Erin's up to 4.
    create(
        &mut world,
        carol,
        RoomSpec {
            max_level: 5,
            ..spec(8)
        },
        None,
    );
    create(
        &mut world,
        dave,
        RoomSpec {
            max_level: 10,
            ..spec(8)
        },
        Some(1234),
    );
    let low = create(
        &mut world,
        erin,
        RoomSpec {
            max_level: 10,
            ..spec(6)
        },
        None,
    );
    // The lowest open one: Erin's (Dave's has a PIN).
    let offered = switch_of(&state(&mut world, bob)).expect("a room offered");
    assert_eq!(offered, ["W", &low.to_string(), "10", "1", "6", "Erin"]);
    // One equipped over 4 is offered Carol's.
    with_gear(&mut world, alice, 15);
    assert_eq!(switch_of(&state(&mut world, alice)), None);
    // None out of a private room: it has no max level to better.
    let frank = world.connect("Frank");
    create(&mut world, frank, spec(8), Some(4321));
    assert_eq!(switch_of(&state(&mut world, frank)), None);
    // Bob's click: into Erin's room, out of Alice's.
    let moved = rows(&mut world, bob, ROOM_JOIN_BASE + low, 1);
    assert_eq!(header(&moved)[KEY], low.to_string());
    assert_eq!(header(&state(&mut world, alice))[MEMBERS], "1");
    // In Erin's, nothing lower is open: no offer.
    assert_eq!(switch_of(&state(&mut world, bob)), None);
}

/// A member browses the list from its room (the All rooms tab, the owner's
/// of 2026-10-22): its own room is in it; a Join of another room takes it
/// out of its own, as Leave does; a refused one leaves it where it was and
/// says why, once.
#[test]
fn a_member_joins_another_room_from_the_list() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let carol = world.connect("Carol");
    let dave = world.connect("Dave");
    let first = create(&mut world, alice, spec(8), None);
    rows(&mut world, bob, ROOM_JOIN_BASE + first, 1);
    let open = create(&mut world, carol, spec(8), None);
    let private = create(&mut world, dave, spec(8), Some(1234));
    // The list names Bob's room.
    let list = rows(&mut world, bob, ROOMS_LIST, 32);
    assert_eq!(list[0], format!("S1;L;3;{first}"));
    // A wrong PIN: still in his room, told why once.
    rows(&mut world, bob, ROOM_PIN_BASE + 4321, 1);
    let refused = rows(&mut world, bob, ROOM_JOIN_BASE + private, 1);
    assert_eq!(header(&refused)[KEY], first.to_string());
    assert_eq!(header(&refused)[CODE], "3");
    assert_eq!(header(&state(&mut world, bob))[CODE], "0", "said once");
    // Into Carol's, out of Alice's.
    let moved = rows(&mut world, bob, ROOM_JOIN_BASE + open, 1);
    assert_eq!(header(&moved)[KEY], open.to_string());
    assert_eq!(header(&moved)[MEMBERS], "2");
    assert_eq!(header(&moved)[CODE], "0");
    assert_eq!(header(&state(&mut world, alice))[MEMBERS], "1");
}

/// A PIN keeps a room closed until it is given; the owner takes a member
/// out, who may not come back and is told why once.
#[test]
fn a_pin_closes_a_room_and_a_kick_keeps_its_member_out() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    // A room asked for with a PIN that was not given is none.
    assert_eq!(
        rows(
            &mut world,
            alice,
            create_start(RoomSpec {
                pin: true,
                ..spec(8)
            }),
            1
        ),
        ["S1;N;8;0;0"]
    );
    let key = create(&mut world, alice, spec(8), Some(1234));
    let list = rows(&mut world, bob, ROOMS_LIST, 32);
    assert!(
        list[1].ends_with(";1;Alice"),
        "listed with its PIN: {list:?}"
    );
    assert_eq!(
        rows(&mut world, bob, ROOM_JOIN_BASE + key, 1),
        ["S1;N;3;0;0"]
    );
    assert_eq!(rows(&mut world, bob, ROOM_PIN_BASE + 1234, 1), ["S1;K;1"]);
    assert_eq!(
        header(&rows(&mut world, bob, ROOM_JOIN_BASE + key, 1))[MEMBERS],
        "2"
    );
    // Only the owner kicks.
    let seen = state(&mut world, alice);
    let serial: u32 = header(&seen)[SERIAL].parse().unwrap();
    rows(&mut world, bob, kick_start(0, serial), 1);
    assert_eq!(header(&state(&mut world, alice))[MEMBERS], "2");
    // A kick from an older list (another serial) takes nobody out.
    rows(&mut world, alice, kick_start(1, serial + 1), 1);
    assert_eq!(header(&state(&mut world, alice))[MEMBERS], "2");
    let kicked = rows(&mut world, alice, kick_start(1, serial), 1);
    assert_eq!(header(&kicked)[MEMBERS], "1");
    assert_eq!(state(&mut world, bob), ["S1;N;10;0;0"]);
    assert_eq!(state(&mut world, bob), ["S1;N;0;0;0"], "said once");
    rows(&mut world, bob, ROOM_PIN_BASE + 1234, 1);
    assert_eq!(
        rows(&mut world, bob, ROOM_JOIN_BASE + key, 1),
        ["S1;N;10;0;0"]
    );
    // The owner leaves: the room is gone with its last member.
    assert_eq!(rows(&mut world, alice, ROOM_LEAVE, 1), ["S1;N;0;0;0"]);
    assert_eq!(rows(&mut world, bob, ROOMS_LIST, 32), ["S1;L;0;0"]);
}

/// A max level keeps higher equipment out (0 lets anyone in), and nobody
/// makes a room under its own equipment level.
#[test]
fn a_max_level_keeps_higher_equipment_out() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let carol = world.connect("Carol");
    // Alice (a fresh account: equipment level 0) caps it at 3.
    let key = create(
        &mut world,
        alice,
        RoomSpec {
            max_level: 3,
            ..spec(8)
        },
        None,
    );
    assert_eq!(header(&state(&mut world, alice))[LEVEL + 1], "3");
    with_gear(&mut world, bob, 5);
    assert_eq!(
        rows(&mut world, bob, ROOM_JOIN_BASE + key, 1),
        ["S1;N;12;0;0"]
    );
    // Nor does Play put Bob there: it makes him a room of any level.
    world.ask(bob, request::play(1));
    world.advance(Duration::from_millis(100));
    let made = state(&mut world, bob);
    assert_ne!(header(&made)[KEY], key.to_string());
    assert_eq!(&header(&made)[LEVEL..LEVEL + 2], ["5", "10"]);
    // Nobody makes a room under its own equipment level.
    with_gear(&mut world, carol, 5);
    assert_eq!(
        rows(
            &mut world,
            carol,
            create_start(RoomSpec {
                max_level: 4,
                ..spec(8)
            }),
            1
        ),
        ["S1;N;16;0;0"]
    );
    create(
        &mut world,
        carol,
        RoomSpec {
            max_level: 5,
            ..spec(8)
        },
        None,
    );
    // Max level 0: anyone.
    let dave = world.connect("Dave");
    with_gear(&mut world, dave, 5);
    let open = create(&mut world, alice, spec(8), None);
    assert_eq!(
        header(&rows(&mut world, dave, ROOM_JOIN_BASE + open, 1))[MEMBERS],
        "2"
    );
}

/// A private room has no max level (the owner's of 2026-10-26): its maker's
/// window's (3 here) is kept for the next open room only. A maker over it
/// makes the room; with the PIN a high and a low equipment join, both fit
/// to play; the room and its row in the list tell no max level; its match
/// takes everyone. An open room's max level still keeps higher equipment
/// out.
#[test]
fn a_private_room_lets_in_any_equipment() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let carol = world.connect("Carol");
    with_gear(&mut world, alice, 5);
    with_gear(&mut world, bob, 5);
    let capped = RoomSpec {
        max_level: 3,
        ..spec(8)
    };
    let key = create(&mut world, alice, capped, Some(1234));
    rows(&mut world, bob, ROOM_PIN_BASE + 1234, 1);
    assert_eq!(
        header(&rows(&mut world, bob, ROOM_JOIN_BASE + key, 1))[MEMBERS],
        "2"
    );
    rows(&mut world, carol, ROOM_PIN_BASE + 1234, 1);
    let joined = rows(&mut world, carol, ROOM_JOIN_BASE + key, 1);
    assert_eq!(header(&joined)[MEMBERS], "3");
    assert_eq!(header(&joined)[LEVEL + 1], "0", "no max level");
    // P;<place>;<gear>;<owner>;<me>;<in match>;<fit>;<away>;<name>
    let members: Vec<(String, String)> = joined
        .iter()
        .filter(|row| row.starts_with("P;"))
        .map(|row| {
            let fields: Vec<&str> = row.split(';').collect();
            (fields[2].to_owned(), fields[6].to_owned())
        })
        .collect();
    let fit = |gear: &str| (gear.to_owned(), "0".to_owned());
    assert_eq!(members, [fit("5"), fit("5"), fit("0")]);
    // Its row: <key>;<level>;<max level>;<size>;<players>;<phase>;<secs>;
    // <map>;<pin>;<name>.
    let list = rows(&mut world, carol, ROOMS_LIST, 32);
    let row: Vec<&str> = list[1].split(';').collect();
    assert_eq!((row[0], row[2], row[8]), (&*key.to_string(), "0", "1"));
    // Its match takes all three: nobody is out for its equipment.
    for member in [alice, bob, carol] {
        world.take(member);
    }
    let (ticket, _) = sent_to_match(&press_start(&mut world, alice)).expect("Alice plays");
    for member in [bob, carol] {
        let (sent, _) = sent_to_match(&world.take(member)).expect("each member plays");
        assert_eq!(sent, ticket);
    }
    assert_eq!(world.core.rooms.get(key).unwrap().members.len(), 3);
    // An open room of the same window keeps Bob's equipment out.
    let dave = world.connect("Dave");
    let open = create(&mut world, dave, capped, None);
    let erin = world.connect("Erin");
    with_gear(&mut world, erin, 5);
    assert_eq!(
        rows(&mut world, erin, ROOM_JOIN_BASE + open, 1),
        ["S1;N;12;0;0"]
    );
}

/// Carol joins a room whose match is being played and admits players: she
/// sees the room first (who is in it, a switch, her own room) and goes into
/// the match 15 s later; Dave asks to go in at once (the owner's of
/// 2026-10-19).
#[test]
fn a_member_who_comes_while_the_match_is_played_goes_into_it() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let carol = world.connect("Carol");
    let dave = world.connect("Dave");
    let key = create(&mut world, alice, spec(6), None);
    rows(&mut world, bob, ROOM_JOIN_BASE + key, 1);
    world.advance(VOTE_AND_WAIT);
    let (ticket, _) = sent_to_match(&world.take(alice)).expect("a match");
    let (&id, _) = world
        .core
        .matches
        .iter()
        .find(|(_, running)| running.ticket.match_id == ticket)
        .unwrap();
    assert!(matches!(
        world.core.rooms.get(key).unwrap().stage,
        Stage::InMatch { match_id, .. } if match_id == id
    ));
    world.host.admitting.locked().insert(ticket);
    world.take(carol);
    // The answer, then "match found" and where it is.
    let addr = world.clients[carol].0;
    let now = world.now;
    world.core.handle(
        addr,
        ClientMessage::QueryRanking {
            start: ROOM_JOIN_BASE + key,
            count: 1,
        },
        now,
    );
    world.advance(Duration::from_millis(100));
    let decoded = world.take(carol);
    let joined = find(&decoded, |message| match message {
        Decoded::EloList(rows) => Some(rows[0].0.clone()),
        _ => None,
    })
    .expect("the room's state");
    assert_eq!(joined.split(';').nth(MEMBERS), Some("3"), "{joined}");
    // The room first: not in the match yet, 15 s to go.
    assert!(sent_to_match(&decoded).is_none());
    let waiting = state(&mut world, carol);
    assert!(waiting.iter().any(|row| row == "J;15"), "{waiting:?}");
    world.advance(Duration::from_secs(16));
    let (sent, _) = sent_to_match(&world.take(carol)).expect("Carol goes in late");
    assert_eq!(sent, ticket);
    // Dave does not wait: Join match puts him in at once (the match takes
    // the next one once Carol is in).
    world.host.admitting.locked().insert(ticket);
    rows(&mut world, dave, ROOM_JOIN_BASE + key, 1);
    world.take(dave);
    let addr = world.clients[dave].0;
    let now = world.now;
    world.core.handle(
        addr,
        ClientMessage::QueryRanking {
            start: ROOM_JOIN_MATCH,
            count: 1,
        },
        now,
    );
    // Sent on the lobby's next tick, not after 15 s.
    world.advance(Duration::from_secs(2));
    let (sent, _) = sent_to_match(&world.take(dave)).expect("Dave goes in at once");
    assert_eq!(sent, ticket);
    let setup = world.host.made.locked().last().unwrap().1.clone();
    assert_eq!(setup.players.len(), 4);
}

/// The match server cannot make the room's match: nobody is left marked as
/// searching, the countdown runs again, and the next try plays.
#[test]
fn a_room_whose_match_cannot_be_made_tries_again() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let key = create(&mut world, alice, spec(6), None);
    rows(&mut world, bob, ROOM_JOIN_BASE + key, 1);
    world.host.fail.store(true, Ordering::Relaxed);
    world.take(alice);
    world.take(bob);
    world.advance(VOTE_AND_WAIT);
    for index in [alice, bob] {
        let decoded = world.take(index);
        assert!(sent_to_match(&decoded).is_none());
        assert!(
            !matches!(queue_state(&decoded), Some(ClientState::InQueue { .. })),
            "left searching: {decoded:?}"
        );
    }
    // The vote and the wait again, whole.
    assert_eq!(header(&state(&mut world, alice))[PHASE], "3");
    world.host.fail.store(false, Ordering::Relaxed);
    world.advance(Duration::from_secs(16));
    assert!(
        sent_to_match(&world.take(alice)).is_none(),
        "the 30 s at the least"
    );
    world.advance(Duration::from_secs(16));
    assert!(sent_to_match(&world.take(alice)).is_some());
    assert!(sent_to_match(&world.take(bob)).is_some());
}

/// A member whose equipment rose over the room's max level after it came
/// in stays out of the room's match; the others play.
#[test]
fn equipment_over_the_max_level_keeps_a_member_out_of_the_match() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let carol = world.connect("Carol");
    let key = create(
        &mut world,
        alice,
        RoomSpec {
            max_level: 3,
            ..spec(6)
        },
        None,
    );
    rows(&mut world, bob, ROOM_JOIN_BASE + key, 1);
    rows(&mut world, carol, ROOM_JOIN_BASE + key, 1);
    with_gear(&mut world, bob, 5);
    world.take(bob);
    // Marked in the room: equipment over the max level (fit 2).
    let marked = state(&mut world, alice);
    assert!(
        marked.iter().any(|row| row.ends_with(";2;0;Bob")),
        "{marked:?}"
    );
    world.advance(VOTE_AND_WAIT);
    assert!(sent_to_match(&world.take(alice)).is_some());
    assert!(sent_to_match(&world.take(carol)).is_some());
    assert!(sent_to_match(&world.take(bob)).is_none());
    let setup = world.host.made.locked().last().unwrap().1.clone();
    assert_eq!(setup.players.len(), 2);
    // The match started without him: he is out of the room, told why.
    assert_eq!(state(&mut world, bob), ["S1;N;12;0;0"]);
}

/// A member whose kit cannot play (no ammo) is marked in the room, counts
/// for no start, and is taken out of the room when the match starts
/// without it; it may come back once its kit can play (the door refuses a
/// kit that can not, code 15).
#[test]
fn a_kit_without_ammo_counts_for_nothing_and_is_taken_out_at_the_start() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let dave = world.connect("Dave");
    let carol = world.connect("Carol");
    let key = create(&mut world, alice, spec(6), None);
    let ammo = world.account(dave).kits[0].slots;
    // In the room, then the ammo taken out of the kit.
    rows(&mut world, dave, ROOM_JOIN_BASE + key, 1);
    world.change(dave, |account| {
        for slot in [8, 9, 11, 12, 19, 20, 21, 22] {
            account.kits[0].slots[slot] = None;
        }
    });
    let marked = state(&mut world, alice);
    assert!(
        marked.iter().any(|row| row.ends_with(";1;0;Dave")),
        "{marked:?}"
    );
    // Alice and Dave: one who can play, so no countdown.
    world.advance(VOTE_AND_WAIT);
    assert_eq!(header(&state(&mut world, alice))[PHASE], "0");
    rows(&mut world, carol, ROOM_JOIN_BASE + key, 1);
    world.advance(VOTE_AND_WAIT);
    assert!(sent_to_match(&world.take(alice)).is_some());
    assert!(sent_to_match(&world.take(carol)).is_some());
    assert!(sent_to_match(&world.take(dave)).is_none());
    assert_eq!(state(&mut world, dave), ["S1;N;13;0;0"]);
    // Taken out, not turned out: Dave may come back, once his kit plays.
    assert_eq!(
        rows(&mut world, dave, ROOM_JOIN_BASE + key, 1),
        ["S1;N;15;0;0"]
    );
    world.change(dave, |account| account.kits[0].slots = ammo);
    let back = rows(&mut world, dave, ROOM_JOIN_BASE + key, 1);
    assert_eq!(header(&back)[1], "R", "{back:?}");
}

/// A squad in a room is one side's and a squad in the match: its members
/// share a non-zero squad id; a player alone has none.
#[test]
fn a_squad_in_a_room_plays_as_a_squad_on_one_side() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let carol = world.connect("Carol");
    let dave = world.connect("Dave");
    super::form_squad(&mut world, alice, bob);
    // Alice's Join takes Bob, her squad's soldier, along.
    let key = create(&mut world, carol, spec(6), None);
    for index in [alice, dave] {
        rows(&mut world, index, ROOM_JOIN_BASE + key, 1);
    }
    world.advance(VOTE_AND_WAIT);
    let setup = world.host.made.locked().last().unwrap().1.clone();
    assert_eq!(setup.players.len(), 4);
    let of = |name: &str| {
        let player = setup
            .players
            .iter()
            .find(|p| p.profile.name == name)
            .unwrap();
        (player.profile.team, player.profile.squad)
    };
    let (alice_side, alice_squad) = of("Alice");
    assert_eq!(of("Bob"), (alice_side, alice_squad));
    assert_ne!(alice_squad, 0);
    assert_eq!(of("Carol").1, 0);
    assert_ne!(
        of("Carol").0,
        alice_side,
        "2 v 2: the two alone on the other side"
    );
}

/// A maintenance near: a room not playing ends at once; one playing plays
/// its match out and ends with it. Its members are told why.
#[test]
fn a_maintenance_lets_the_match_finish_then_ends_the_room() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let carol = world.connect("Carol");
    let key = create(&mut world, alice, spec(6), None);
    rows(&mut world, bob, ROOM_JOIN_BASE + key, 1);
    world.take(alice);
    world.advance(VOTE_AND_WAIT);
    let (match_id, _) = sent_to_match(&world.take(alice)).expect("a match");
    // Another room, idle.
    let idle = create(&mut world, carol, spec(6), None);
    world
        .store
        .set_maintenance(Some(session::v026g::quests::now() + 20 * 60));
    world.advance(Duration::from_secs(1));
    assert_eq!(
        state(&mut world, carol),
        ["S1;N;9;0;0"],
        "room {idle} ends at once"
    );
    // The match goes on, and the room with it.
    assert_eq!(header(&state(&mut world, alice))[PHASE], "2");
    world.end_match(match_id, 0);
    world.advance(Duration::from_secs(1));
    assert_eq!(state(&mut world, alice), ["S1;N;9;0;0"]);
    assert_eq!(state(&mut world, bob), ["S1;N;9;0;0"]);
    // Said once.
    assert_eq!(state(&mut world, bob), ["S1;N;0;0;0"]);
}

/// From the end of a match to the start of the next of a room of [`spec`]:
/// the vote's 15 s, 10 s more after a match, the wait's 15 s, and a little.
const NEXT_MATCH: Duration = Duration::from_secs(42);
/// From two players to the match in a room Play made: the vote's 15 s, its
/// wait of 30 s, and a little.
const QUICK_START: Duration = Duration::from_secs(47);

/// The movie's Play (no search): the key of the room it put the player in.
fn play_into_room(world: &mut World, index: usize) -> String {
    let state = rows(world, index, ROOM_PLAY, 1);
    assert_eq!(header(&state)[1], "R", "{state:?}");
    header(&state)[KEY].clone()
}

/// The room's match starts within `wait`: each of `players` is sent to it.
/// Its ticket.
fn room_match_starts(world: &mut World, players: &[usize], wait: Duration) -> u32 {
    for &player in players {
        world.take(player);
    }
    world.advance(wait);
    let tickets: Vec<u32> = players
        .iter()
        .map(|&player| {
            sent_to_match(&world.take(player))
                .expect("sent to the match")
                .0
        })
        .collect();
    assert!(tickets.iter().all(|ticket| *ticket == tickets[0]));
    tickets[0]
}

/// The owner's of 2026-10-31: after a match every member is in the room
/// still, however it came in. Carol (the movie's Play) and Dave (the
/// game's Play, `2F`) came by Play, Alice made the room and Bob joined it
/// from the list: all four see their results and are its members after
/// the match, and its next vote makes the next match of the four.
#[test]
fn who_came_by_play_stays_in_the_room_after_the_match_as_who_chose_it() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let carol = world.connect("Carol");
    let dave = world.connect("Dave");
    let key = create(
        &mut world,
        alice,
        RoomSpec {
            max_level: 10,
            ..spec(6)
        },
        None,
    );
    rows(&mut world, bob, ROOM_JOIN_BASE + key, 1);
    assert_eq!(play_into_room(&mut world, carol), key.to_string());
    world.ask(dave, request::play(1));
    world.advance(Duration::from_millis(100));
    assert_eq!(header(&state(&mut world, dave))[KEY], key.to_string());
    let players = [alice, bob, carol, dave];
    let ticket = room_match_starts(&mut world, &players, VOTE_AND_WAIT);

    world.end_match(ticket, 0);
    world.advance(Duration::from_secs(1));
    for index in players {
        let decoded = world.take(index);
        assert!(matches!(queue_state(&decoded), Some(ClientState::Lobby)));
        assert_eq!(screens(&decoded).len(), 1, "its results: {decoded:?}");
        let after = state(&mut world, index);
        assert_eq!(header(&after)[KEY], key.to_string(), "{after:?}");
        assert_eq!(header(&after)[MEMBERS], "4");
    }
    let next = room_match_starts(&mut world, &players, NEXT_MATCH);
    assert_ne!(next, ticket);
}

/// Carol and Dave pressed Play with no room open: Play made one for Carol
/// and put Dave in it. After its match both are in it still, the room
/// with them.
#[test]
fn a_room_play_made_stays_with_its_players_after_its_match() {
    let mut world = World::new();
    let carol = world.connect("Carol");
    let dave = world.connect("Dave");
    let key = play_into_room(&mut world, carol);
    assert_eq!(play_into_room(&mut world, dave), key);
    let ticket = room_match_starts(&mut world, &[carol, dave], QUICK_START);
    world.end_match(ticket, 0);
    world.advance(Duration::from_secs(1));
    assert_eq!(world.core.rooms.len(), 1);
    for index in [carol, dave] {
        let after = state(&mut world, index);
        assert_eq!(header(&after)[KEY], key, "{after:?}");
        assert_eq!(header(&after)[MEMBERS], "2");
    }
}

/// A member whose game is gone (closed, killed: its link goes silent)
/// keeps its place, grey (`away`), while its session is kept: the link's
/// 30 s and the session's 60 s; then it is out of the room.
#[test]
fn a_member_whose_game_is_gone_keeps_its_place_for_90_s() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let key = create(&mut world, alice, spec(6), None);
    rows(&mut world, bob, ROOM_JOIN_BASE + key, 1);
    // Its link goes after 30 s of silence: still in the room, grey.
    world.silent_for(bob, Duration::from_secs(40));
    let lost = state(&mut world, alice);
    assert_eq!(header(&lost)[MEMBERS], "2", "{lost:?}");
    assert!(lost.iter().any(|row| row.ends_with(";0;1;Bob")), "{lost:?}");
    // Its session waits a minute more; then it is out.
    world.advance(Duration::from_secs(61));
    let after = state(&mut world, alice);
    assert_eq!(header(&after)[MEMBERS], "1", "{after:?}");
}

/// A room's match is of its players' mean level (the row of
/// `match_levels_dict` its rewards take), and one more match on its map
/// for each of them.
#[test]
fn a_rooms_match_has_its_players_mean_level_and_counts_its_map() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    with_gear(&mut world, alice, 6);
    assert_eq!(matchmaking::level(&world.account(alice), 0).min, 6);
    assert_eq!(matchmaking::level(&world.account(bob), 0).min, 0);
    let key = create(&mut world, alice, spec(6), None);
    rows(&mut world, bob, ROOM_JOIN_BASE + key, 1);
    world.advance(VOTE_AND_WAIT);
    let (ticket, _) = sent_to_match(&world.take(alice)).expect("a match");
    let made = world.host.made.locked().clone();
    let [(id, setup)] = &made[..] else {
        panic!("{made:?}");
    };
    assert_eq!(*id, ticket);
    assert_eq!(setup.match_level, 3);
    let map = maps::map_mode(setup.map_id).unwrap().level;
    for player in [alice, bob] {
        let account = world.account(player);
        assert_eq!(account.map_plays, [(map, 1)]);
    }
    world.assert_clean();
}

/// Only a game that plays in matches that take players (patch revision 5
/// on) plays in rooms: an older one's Play makes no room and says why.
#[test]
fn an_older_game_plays_in_no_room() {
    use survarium::v026g::match_msgs::late_join::FIRST_REVISION;

    let mut world = World::new();
    let old = world.connect_revision("Four", FIRST_REVISION - 1);
    world.ask(old, request::play(1));
    world.advance(Duration::from_millis(100));
    assert!(world.core.rooms.of(world.ids[old].0).is_none());
    assert_eq!(state(&mut world, old), ["S1;N;7;0;0"]);
    assert!(world.host.made.locked().is_empty());
}

/// The create window's answer: its header's fields and the `maps_dict` ids
/// it puts in (tile in, mode not out).
fn form(world: &mut World, index: usize) -> (Vec<String>, Vec<u8>) {
    let rows = rows(world, index, ROOM_FORM, 1);
    let header = header(&rows);
    let modes_off: Vec<u8> = header[7]
        .split(',')
        .filter(|mode| !mode.is_empty())
        .map(|mode| mode.parse().unwrap())
        .collect();
    let mut tiles_on = Vec::new();
    for (chunk, row) in rows[1..].iter().enumerate() {
        let fields: Vec<&str> = row.split(';').collect();
        assert_eq!(fields[..2], ["E", &chunk.to_string()]);
        let bits: u32 = fields[2].parse().unwrap();
        tiles_on.extend((0..20).map(|b| bits & 1 << b != 0));
    }
    let chosen = world
        .core
        .room_entries()
        .iter()
        .zip(tiles_on)
        .filter(|(entry, on)| *on && !modes_off.contains(&entry.mode))
        .map(|(entry, _)| entry.map_id)
        .collect();
    (header[2..7].to_vec(), chosen)
}

/// A room made by hand is its maker's next create window (the owner's of
/// 2026-10-20): its team size, players to start, max level, wait and pool,
/// not its PIN; a max level under the maker's equipment now is raised to
/// it. With none made, the defaults; a room Play made keeps nothing.
#[test]
fn a_room_made_by_hand_starts_its_makers_next_create_window() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let entries = world.core.room_entries();
    let regular: Vec<u8> = entries
        .iter()
        .filter(|entry| !entry.extra)
        .map(|entry| entry.map_id)
        .collect();
    assert!(entries.len() >= 3, "{entries:?}");
    let defaults = (
        ["0", "8", "1", "0", "30"].map(str::to_owned).to_vec(),
        regular.clone(),
    );
    assert_eq!(form(&mut world, alice), defaults);
    world.ask(alice, request::play(1));
    world.advance(Duration::from_millis(100));
    assert_eq!(header(&state(&mut world, alice))[1], "R");
    assert_eq!(form(&mut world, alice), defaults);
    assert!(world.account(alice).room_settings.is_none());

    // By hand: the first and third entries, 9 v 9 from 2 v 2, max level 3,
    // a wait of 45 s, a PIN.
    rows(&mut world, alice, bits_start(0, 0b101), 1);
    create(
        &mut world,
        alice,
        RoomSpec {
            size: 9,
            min_side: 2,
            max_level: 3,
            wait: 45,
            pin: false,
        },
        Some(1234),
    );
    let (fields, chosen) = form(&mut world, alice);
    assert_eq!(fields, ["1", "9", "2", "3", "45"]);
    // The pool as made: the window's tiles and modes give it back (an entry
    // of the same map, weather and mode would come with it).
    let made: Vec<u8> = world
        .core
        .rooms
        .of(world.ids[alice].0)
        .unwrap()
        .pool
        .clone();
    assert_eq!(made, [entries[0].map_id, entries[2].map_id]);
    assert_eq!(chosen, made);
    assert!(world.account(alice).room_settings.is_some());
    // Equipment over the max level now: the window offers its level.
    with_gear(&mut world, alice, 5);
    assert_eq!(form(&mut world, alice).0, ["1", "9", "2", "5", "45"]);
    // Lowered again (the movie asks anew when the gear changes): the max
    // level made.
    with_gear(&mut world, alice, 2);
    assert_eq!(form(&mut world, alice).0, ["1", "9", "2", "3", "45"]);
    // Play's room after it keeps what the hand made.
    rows(&mut world, alice, ROOM_LEAVE, 1);
    world.ask(alice, request::play(1));
    world.advance(Duration::from_millis(100));
    assert_eq!(form(&mut world, alice).1, made);
}

/// The movie asks for its create window's start once, as its lobby comes
/// up (the owner's of 2026-10-25: the window opens filled, nothing applied
/// after it is drawn): the form is answered right after the sign-in,
/// before any rooms window, and a room made by hand is in the answer to
/// the next sign-in. Asking it tells no game its room (`room_unseen`).
#[test]
fn the_create_window_start_is_there_from_the_sign_in() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    // The lobby's first asks, before anything is opened: a new player's is
    // the defaults.
    let (fields, _) = form(&mut world, alice);
    assert_eq!(fields, ["0", "8", "1", "0", "30"]);

    // A room made by hand: what the movie keeps for its next window (the
    // same game asking again gets the same).
    rows(&mut world, alice, bits_start(0, 0b11), 1);
    let made_spec = RoomSpec {
        size: 7,
        min_side: 3,
        max_level: 0,
        wait: 60,
        pin: false,
    };
    let key = create(&mut world, alice, made_spec, Some(1234));
    let made = (
        ["1", "7", "3", "0", "60"].map(str::to_owned).to_vec(),
        world.core.rooms.get(key).unwrap().pool.clone(),
    );
    assert_eq!(form(&mut world, alice), made);

    // Her game started anew: its first ask has it, and she is still not
    // told her room by it.
    let (account_id, _) = world.ids[alice];
    let session = world.new_session(account_id);
    let again = world.connect_session((account_id, session));
    world.take(again);
    assert_eq!(form(&mut world, again), made);
    assert!(world.core.room_unseen.contains_key(&account_id));
}

/// A full room's queue (the owner's of 2026-10-17): who is in no room waits
/// for a place and is told its place; when a member leaves, the first in
/// the queue is in the room at once, and the next moves up; leaving the
/// queue takes one out of it.
#[test]
fn a_full_room_takes_the_first_of_its_queue_when_a_member_leaves() {
    let mut world = World::new();
    let owner = world.connect("Owner");
    let key = create(&mut world, owner, spec(6), None);
    let members: Vec<usize> = (1..12)
        .map(|n| world.connect(&format!("Member{n}")))
        .collect();
    for &member in &members {
        rows(&mut world, member, ROOM_JOIN_BASE + key, 1);
    }
    let first = world.connect("First");
    let second = world.connect("Second");
    assert_eq!(
        rows(&mut world, first, ROOM_QUEUE_BASE + key, 1),
        [format!("S1;N;0;{key};1")]
    );
    assert_eq!(
        rows(&mut world, second, ROOM_QUEUE_BASE + key, 1),
        [format!("S1;N;0;{key};2")]
    );
    // Full: the queue waits.
    world.advance(Duration::from_secs(1));
    assert_eq!(state(&mut world, first), [format!("S1;N;0;{key};1")]);
    rows(&mut world, members[0], ROOM_LEAVE, 1);
    world.advance(Duration::from_secs(1));
    let inside = state(&mut world, first);
    assert_eq!(header(&inside)[KEY], key.to_string(), "{inside:?}");
    assert_eq!(state(&mut world, second), [format!("S1;N;0;{key};1")]);
    // Leaving the queue.
    assert_eq!(rows(&mut world, second, ROOM_UNQUEUE, 1), ["S1;N;0;0;0"]);
    // The site's count, as of the lobby's next tick (each second).
    world.advance(Duration::from_secs(1));
    assert_eq!(world.core.store.room_counts().queued, 0);
}

/// Bob leaves the room's match by his own word (his game's 0xE0, which the
/// match server passes on): he is out of the room, as by Leave, and may
/// join it again; while the match is played he goes into it again 15 s
/// later as a new place, fresh. Its results count his latest place only,
/// and his map once (the owner's of 2026-10-19).
#[test]
fn one_who_leaves_the_match_by_its_own_word_is_out_of_the_room_and_may_come_again() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let key = create(&mut world, alice, spec(6), None);
    rows(&mut world, bob, ROOM_JOIN_BASE + key, 1);
    world.advance(VOTE_AND_WAIT);
    let (ticket, _) = sent_to_match(&world.take(alice)).expect("a match");
    assert!(sent_to_match(&world.take(bob)).is_some());
    let (&id, _) = world
        .core
        .matches
        .iter()
        .find(|(_, running)| running.ticket.match_id == ticket)
        .unwrap();
    let (bob_account, bob_session) = world.ids[bob];

    // His game leaves the match: the match server gives his seat up, and
    // his client is back in the lobby.
    assert_eq!(world.store.matchmaking().leave_match(bob_account), Some(id));
    world.ask(bob, request::leave(1, true));
    world.advance(Duration::from_secs(1));
    assert_eq!(world.core.rooms.key_of(bob_account), None);
    let room = world.core.rooms.get(key).unwrap();
    assert_eq!(room.members.len(), 1);
    assert!(!room.kicked.contains(&bob_account));
    assert!(!room.in_match.contains(&bob_account));
    assert_eq!(state(&mut world, bob), ["S1;N;0;0;0"]);

    // He joins again while the match is played: into it 15 s later, as a
    // new place.
    world.host.admitting.locked().insert(ticket);
    let joined = rows(&mut world, bob, ROOM_JOIN_BASE + key, 1);
    assert_eq!(header(&joined)[MEMBERS], "2", "{joined:?}");
    world.take(bob);
    world.advance(Duration::from_secs(16));
    let (sent, _) = sent_to_match(&world.take(bob)).expect("Bob goes in again");
    assert_eq!(sent, ticket);
    let setup = world.host.made.locked().last().unwrap().1.clone();
    let places: Vec<u32> = setup
        .players
        .iter()
        .map(|player| player.account_id)
        .collect();
    assert_eq!(places, [world.ids[alice].0, bob_account, bob_account]);
    assert_eq!(world.store.matchmaking().seat(bob_session), Some((id, 2)));
    assert!(
        world
            .core
            .rooms
            .get(key)
            .unwrap()
            .in_match
            .contains(&bob_account)
    );
    let map = maps::map_mode(setup.map_id).unwrap().level;
    assert_eq!(world.account(bob).map_plays, [(map, 1)]);

    // The match ends: Bob's latest place is his result, once.
    world.end_match(ticket, 0);
    world.settle();
    let report = world.results.find(ticket).expect("its report");
    assert_eq!(report.players.len(), 2);
    let bob_report = report.player(bob_account).unwrap();
    assert_eq!(bob_report.player_id, 3);
    assert_eq!(report.stats.rows.len(), 2);
    world.assert_clean();
}

/// A room match of four that has started: the room's key, the match's
/// ticket and lobby id, and the four clients.
fn room_match_of_four(world: &mut World) -> (u32, u32, u32, [usize; 4]) {
    let players = ["Alice", "Bob", "Carol", "Dave"].map(|name| world.connect(name));
    let key = create(world, players[0], spec(6), None);
    for &player in &players[1..] {
        rows(world, player, ROOM_JOIN_BASE + key, 1);
    }
    world.advance(VOTE_AND_WAIT);
    let (ticket, _) = sent_to_match(&world.take(players[0])).expect("a match");
    for &player in &players[1..] {
        assert!(sent_to_match(&world.take(player)).is_some());
    }
    let (&id, running) = world
        .core
        .matches
        .iter()
        .find(|(_, running)| running.ticket.match_id == ticket)
        .unwrap();
    assert_eq!(running.seats.len(), 4);
    (key, ticket, id, players)
}

/// The roster place and team of the client's player in the match.
fn seat_of(world: &World, id: u32, player: usize) -> (usize, u8) {
    let session = world.ids[player].1;
    let seats = &world.core.matches[&id].seats;
    let place = seats.iter().position(|&(s, _)| s == session).unwrap();
    (place, seats[place].1)
}

/// The player leaves the match by its own word, then (the match showing
/// `present`) joins the room again and goes into the match: its new team.
fn leave_and_come_back(
    world: &mut World,
    (key, ticket, id): (u32, u32, u32),
    player: usize,
    present: u32,
) -> u8 {
    let account = world.ids[player].0;
    assert_eq!(world.store.matchmaking().leave_match(account), Some(id));
    world.ask(player, request::leave(1, true));
    world.advance(Duration::from_secs(1));
    world.host.live.locked().insert(
        ticket,
        LiveView {
            present,
            points: None,
            kills: [0, 0],
            played: Some(Duration::from_secs(60)),
            time_left: Some(Duration::from_secs(600)),
        },
    );
    world.host.admitting.locked().insert(ticket);
    rows(world, player, ROOM_JOIN_BASE + key, 1);
    world.take(player);
    world.advance(Duration::from_secs(16));
    let (sent, _) = sent_to_match(&world.take(player)).expect("back into the match");
    assert_eq!(sent, ticket);
    let seats = &world.core.matches[&id].seats;
    assert_eq!(seats.len(), 5);
    assert_eq!(seats[4].0, world.ids[player].1);
    seats[4].1
}

/// The owner's of 2026-10-19 (players, then the previous team, then the
/// balance): one who comes back into its room's match goes, of two sides
/// with as many players in it, to the side it played on.
#[test]
fn one_who_comes_back_into_the_match_goes_to_its_side_when_the_players_are_alike() {
    let mut world = World::new();
    let (key, ticket, id, players) = room_match_of_four(&mut world);
    // A player of team 1 leaves, and one of team 0 is out of the match too
    // (its game gone): 1 v 1 in it. The players are alike, so the balance
    // alone would send the one coming back to team 0.
    let leaver = players[1..]
        .iter()
        .copied()
        .find(|&player| seat_of(&world, id, player).1 == 1)
        .unwrap();
    let other = players
        .iter()
        .copied()
        .find(|&player| seat_of(&world, id, player).1 == 0)
        .unwrap();
    let present =
        0b1111 & !(1 << seat_of(&world, id, leaver).0) & !(1 << seat_of(&world, id, other).0);
    let team = leave_and_come_back(&mut world, (key, ticket, id), leaver, present);
    assert_eq!(team, 1);
}

/// Players come first: one who comes back goes to the side with fewer
/// players in the match, though it played on the other.
#[test]
fn one_who_comes_back_goes_to_the_side_with_fewer_players_whatever_its_team() {
    let mut world = World::new();
    let (key, ticket, id, players) = room_match_of_four(&mut world);
    let leaver = players[1..]
        .iter()
        .copied()
        .find(|&player| seat_of(&world, id, player).1 == 1)
        .unwrap();
    // Team 0's two are out of the match: 0 v 1 without it.
    let mut present = 0b1111 & !(1 << seat_of(&world, id, leaver).0);
    for &player in &players {
        if seat_of(&world, id, player).1 == 0 {
            present &= !(1 << seat_of(&world, id, player).0);
        }
    }
    let team = leave_and_come_back(&mut world, (key, ticket, id), leaver, present);
    assert_eq!(team, 0);
}

/// The `J` row of a state: `J;<seconds>`, or `J;-1;<why>`.
fn join_row(state: &[String]) -> Option<&str> {
    state
        .iter()
        .find(|row| row.starts_with("J;"))
        .map(String::as_str)
}

/// The owner's of 2026-10-20: a member waiting in its room while the
/// room's match is played, and the match takes nobody now, is told why
/// (`J;-1;<why>`, `JoinHold`): another joiner on its way in, a game
/// loading, the last 30 s, the end, a full side, a maintenance. Its own 15 s
/// over, it goes in once joining opens again, and is shown the seconds
/// again.
#[test]
fn a_member_who_cannot_go_into_the_match_is_told_why() {
    use session::matchmaking_policy::Closed;
    let mut world = World::new();
    let (key, ticket, id, _) = room_match_of_four(&mut world);
    let erin = world.connect("Erin");
    // The match server says why it takes nobody now.
    let held = [
        (Closed::Joining, "J;-1;1"),
        (Closed::Loading, "J;-1;2"),
        (Closed::LastSeconds, "J;-1;3"),
        (Closed::Full, "J;-1;4"),
        (Closed::Ending, "J;-1;6"),
    ];
    world.host.closed.locked().insert(ticket, Closed::Loading);
    let joined = rows(&mut world, erin, ROOM_JOIN_BASE + key, 1);
    assert_eq!(join_row(&joined), Some("J;-1;2"), "{joined:?}");
    for (closed, row) in held {
        world.host.closed.locked().insert(ticket, closed);
        let state = state(&mut world, erin);
        assert_eq!(join_row(&state), Some(row), "{closed:?}: {state:?}");
    }
    // Held past her 15 s, she stays in the room.
    world.host.closed.locked().insert(ticket, Closed::Joining);
    world.advance(Duration::from_secs(16));
    assert!(sent_to_match(&world.take(erin)).is_none());
    // The match says nothing: no reason, and no seconds.
    world.host.closed.locked().clear();
    assert_eq!(join_row(&state(&mut world, erin)), None);
    // Its sides full (6 a side, the room's size): the lobby's own reason,
    // whatever the match server says.
    world.host.admitting.locked().insert(ticket);
    let seats = world.core.matches[&id].seats.clone();
    {
        let running = world.core.matches.get_mut(&id).unwrap();
        for team in [0, 1] {
            let more = 6 - seats.iter().filter(|seat| seat.1 == team).count();
            for n in 0..more {
                running
                    .seats
                    .push((9000 + u32::from(team) * 100 + n as u32, team));
                running.strengths.push(session::Strength::ZERO);
                running.names.push(String::new());
            }
        }
    }
    let full = state(&mut world, erin);
    assert_eq!(join_row(&full), Some("J;-1;4"), "{full:?}");
    world.advance(Duration::from_secs(2));
    assert!(sent_to_match(&world.take(erin)).is_none());
    // A place frees on a side: joining opens, the seconds are back, and
    // she goes in.
    {
        let running = world.core.matches.get_mut(&id).unwrap();
        running.seats.truncate(seats.len());
        running.strengths.truncate(seats.len());
        running.names.truncate(seats.len());
    }
    let open = state(&mut world, erin);
    assert_eq!(join_row(&open), Some("J;0"), "{open:?}");
    world.advance(Duration::from_secs(1));
    let (sent, _) = sent_to_match(&world.take(erin)).expect("Erin goes in");
    assert_eq!(sent, ticket);
}

#[test]
fn a_used_up_match_roster_keeps_a_newcomer_in_the_room_for_the_next_match() {
    let mut world = World::new();
    let players = ["Alice", "Bob", "Carol", "Dave"].map(|name| world.connect(name));
    let key = create(
        &mut world,
        players[0],
        RoomSpec {
            max_level: 10,
            ..spec(8)
        },
        None,
    );
    for &player in &players[1..] {
        rows(&mut world, player, ROOM_JOIN_BASE + key, 1);
    }
    let ticket = room_match_starts(&mut world, &players, VOTE_AND_WAIT);
    let id = *world
        .core
        .matches
        .iter()
        .find(|(_, running)| running.ticket.match_id == ticket)
        .unwrap()
        .0;
    // Twenty lifetime places, but only twelve currently occupied (6 v 6).
    // The room still has space, and its match cannot reuse the departed seats.
    {
        let running = world.core.matches.get_mut(&id).unwrap();
        for n in running.seats.len()..20 {
            running.seats.push((9000 + n as u32, (n % 2) as u8));
            running.strengths.push(session::Strength::ZERO);
            running.names.push(String::new());
        }
    }
    world.host.live.locked().insert(
        ticket,
        LiveView {
            present: (1 << 12) - 1,
            ..LiveView::default()
        },
    );
    world.host.admitting.locked().insert(ticket);
    let erin = world.connect("Erin");
    rows(&mut world, erin, ROOM_PLAY, 1);
    assert_eq!(world.core.rooms.key_of(world.ids[erin].0), Some(key));
    assert_eq!(join_row(&state(&mut world, erin)), Some("J;-1;7"));
    world.advance(Duration::from_secs(20));
    assert!(sent_to_match(&world.take(erin)).is_none());
    assert_eq!(world.core.rooms.key_of(world.ids[erin].0), Some(key));
    world.end_match(ticket, 0);
    for player in players {
        world.take(player);
    }
    world.advance(Duration::from_secs(1));
    world.advance(Duration::from_secs(90));
    assert!(
        sent_to_match(&world.take(erin)).is_some(),
        "waiter plays the next match"
    );
}

/// A maintenance within 30 min: a member waiting in its room while the
/// match is played is told nobody goes in now; with the seconds it would
/// have otherwise.
#[test]
fn a_member_waiting_while_a_maintenance_is_near_is_told_so() {
    let mut world = World::new();
    let (key, ticket, _, _) = room_match_of_four(&mut world);
    let erin = world.connect("Erin");
    world.host.admitting.locked().insert(ticket);
    let joined = rows(&mut world, erin, ROOM_JOIN_BASE + key, 1);
    assert_eq!(join_row(&joined), Some("J;15"), "{joined:?}");
    world
        .store
        .set_maintenance(Some(session::v026g::quests::now() + 20 * 60));
    let state = state(&mut world, erin);
    assert_eq!(join_row(&state), Some("J;-1;5"), "{state:?}");
}

#[test]
fn a_late_joiner_of_higher_gear_raises_the_matchs_equipment_level() {
    // The owner's of 2026-11-01 (`docs/notes/kd-balance.md`): the match's
    // level is the highest gear level of its players; a late joiner keeps
    // it, or raises it with higher gear, and is counted at the level then.
    let mut world = World::new();
    let (key, ticket, _, players) = room_match_of_four(&mut world);
    let at_start = matchmaking::gear_level(&world.account(players[0]), quests::now());
    assert_eq!(world.results.match_gear(ticket), Some(at_start));
    let erin = world.connect("Erin");
    let raised = at_start + 3;
    with_gear(&mut world, erin, raised);
    world.host.admitting.locked().insert(ticket);
    rows(&mut world, erin, ROOM_JOIN_BASE + key, 1);
    world.take(erin);
    world.advance(Duration::from_secs(16));
    assert_eq!(
        sent_to_match(&world.take(erin)).map(|(id, _)| id),
        Some(ticket)
    );
    assert_eq!(world.results.match_gear(ticket), Some(raised));
    let running = world
        .core
        .matches
        .values()
        .find(|running| running.ticket.match_id == ticket)
        .unwrap();
    assert_eq!(
        running.strengths.last().copied(),
        Some(world.account(erin).level_stats.strength_at(raised))
    );
}

/// Live, 2026-10-19 (room 2168): a member that went into its room's match
/// late and was let go before it entered (its game never came, left, or did
/// not catch up; the match server gives its seat up) stayed marked in the
/// match, without its `J` row, and waited out the whole match. Now it waits
/// in the room again, is told when it goes in, and goes in at its next turn
/// as a new place.
#[test]
fn a_member_whose_late_join_fails_waits_in_its_room_again_and_goes_in_later() {
    let mut world = World::new();
    let (key, ticket, id, _) = room_match_of_four(&mut world);
    let erin = world.connect("Erin");
    let (erin_account, erin_session) = world.ids[erin];
    world.host.admitting.locked().insert(ticket);
    let joined = rows(&mut world, erin, ROOM_JOIN_BASE + key, 1);
    assert_eq!(join_row(&joined), Some("J;15"), "{joined:?}");
    world.take(erin);
    world.advance(Duration::from_secs(16));
    let (sent, _) = sent_to_match(&world.take(erin)).expect("Erin goes in");
    assert_eq!(sent, ticket);
    assert_eq!(world.store.matchmaking().seat(erin_session), Some((id, 4)));
    let room = world.core.rooms.get(key).unwrap();
    assert!(room.in_match.contains(&erin_account));

    // The match server lets her go before she entered (her game did not
    // come in 20 s) and takes joiners again; her game is back in the lobby.
    world.advance(Duration::from_secs(20));
    assert_eq!(
        world.store.matchmaking().join_failed(erin_account),
        Some(id)
    );
    world.host.admitting.locked().insert(ticket);
    world.ask(erin, request::leave(1, true));
    world.advance(Duration::from_secs(1));
    let room = world.core.rooms.get(key).unwrap();
    assert!(room.members.contains(&erin_account));
    assert!(!room.in_match.contains(&erin_account));
    let waiting = state(&mut world, erin);
    let row = join_row(&waiting).expect("her J row");
    assert!(!row.starts_with("J;-1"), "{waiting:?}");

    // At her next turn she goes in again, as a new place.
    world.take(erin);
    world.advance(Duration::from_secs(16));
    let (sent, _) = sent_to_match(&world.take(erin)).expect("Erin goes in again");
    assert_eq!(sent, ticket);
    assert_eq!(world.store.matchmaking().seat(erin_session), Some((id, 5)));
    assert!(
        world
            .core
            .rooms
            .get(key)
            .unwrap()
            .in_match
            .contains(&erin_account)
    );
}

/// The members of a member list (`S1;G;<key>;<members>`, then two rows a
/// member: `P;<i>;<gear>;<owner>;<me>;<in match>;<fit>;<away>;<side>;
/// <account>;<squad>` and its whole name) that the asker gets for the room
/// of `key`: the header's fields, then each member's fields with its name
/// last (index 11).
fn members(world: &mut World, index: usize, key: u32) -> Vec<Vec<String>> {
    let rows = rows(world, index, ROOM_MEMBERS_BASE + key, 1);
    let mut seen = vec![rows[0].split(';').map(str::to_owned).collect::<Vec<_>>()];
    assert_eq!(rows.len() % 2, 1, "{rows:?}");
    for pair in rows[1..].chunks(2) {
        let mut fields: Vec<String> = pair[0].split(';').map(str::to_owned).collect();
        assert_eq!(fields.len(), 11, "{pair:?}");
        fields.push(pair[1].clone());
        seen.push(fields);
    }
    seen
}

/// Who is in a room, for anyone looking at the list (the owner's of
/// 2026-10-20): a player in no room sees a room with a PIN's members too,
/// a member's game gone shows grey (`away`), and a room gone has nobody.
#[test]
fn anyone_sees_who_is_in_a_room_with_a_pin_too() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let carol = world.connect("Carol");
    let key = create(&mut world, alice, spec(8), Some(1234));
    rows(&mut world, bob, ROOM_PIN_BASE + 1234, 1);
    rows(&mut world, bob, ROOM_JOIN_BASE + key, 1);
    // Carol is in no room and gave no PIN.
    let seen = members(&mut world, carol, key);
    assert_eq!(seen[0], ["S1", "G", &key.to_string(), "2"], "{seen:?}");
    let names: Vec<&str> = seen[1..].iter().map(|row| row[11].as_str()).collect();
    assert_eq!(names, ["Alice", "Bob"]);
    for (i, row) in seen[1..].iter().enumerate() {
        assert_eq!(row[0], "P");
        assert_eq!(row[1], i.to_string());
        // Owner, me, in match, fit, away, side.
        let flags = &row[3..9];
        let owner = if i == 0 { "1" } else { "0" };
        assert_eq!(flags, [owner, "0", "0", "0", "0", "0"], "{row:?}");
        // Its account.
        let account = world.ids[[alice, bob][i]].0;
        assert_eq!(row[9], account.to_string());
    }
    // A member asking is itself in it.
    let own = members(&mut world, bob, key);
    assert_eq!(own[2][4], "1", "{own:?}");
    assert_eq!(own[1][4], "0", "{own:?}");
    // Bob's game is gone: grey, its place kept.
    world.silent_for(bob, Duration::from_secs(40));
    let lost = members(&mut world, carol, key);
    assert_eq!(lost[2][11], "Bob");
    assert_eq!(lost[2][7], "1", "away: {lost:?}");
    assert_eq!(lost[1][7], "0", "{lost:?}");
    // The room gone: nobody; a key no room ever had the same.
    world.advance(Duration::from_secs(61));
    rows(&mut world, alice, ROOM_LEAVE, 1);
    assert_eq!(
        members(&mut world, carol, key),
        [["S1", "G", &key.to_string(), "0"]]
    );
    assert_eq!(
        members(&mut world, carol, 99_999),
        [["S1", "G", "99999", "0"]]
    );
}

/// While the room plays its match, each member's side is the team of its
/// place in it, 1 or 2; a member waiting outside the match has none.
#[test]
fn a_member_list_says_the_side_of_each_player_in_the_match() {
    let mut world = World::new();
    let (key, _, id, players) = room_match_of_four(&mut world);
    let eve = world.connect("Eve");
    let seen = members(&mut world, eve, key);
    assert_eq!(seen[0][3], "4", "{seen:?}");
    for (row, &player) in seen[1..].iter().zip(&players) {
        let (_, team) = seat_of(&world, id, player);
        assert_eq!(row[5], "1", "in the match: {row:?}");
        assert_eq!(row[8], (team + 1).to_string(), "{row:?}");
    }
    let sides: Vec<&str> = seen[1..].iter().map(|row| row[8].as_str()).collect();
    assert_eq!(
        sides.iter().filter(|&&side| side == "1").count(),
        2,
        "{sides:?}"
    );
    // One who joins now waits outside the match first: no side.
    let frank = world.connect("Frank");
    rows(&mut world, frank, ROOM_JOIN_BASE + key, 1);
    let now = members(&mut world, eve, key);
    let row = &now[5];
    assert_eq!(row[11], "Frank");
    assert_eq!((row[5].as_str(), row[8].as_str()), ("0", "0"), "{now:?}");
}

/// The longest names (63 bytes: wide Latin, Cyrillic, mixed with `;`) come
/// whole in the list, for a room of 20; the state's rows cut them to 32
/// bytes and say so with "…".
#[test]
fn a_member_list_gives_long_names_whole() {
    let mut world = World::new();
    let name = |i: usize| match i % 3 {
        0 => format!("{}{i:02}", "W".repeat(61)),
        1 => format!("{}W{i:02}", "Ш".repeat(30)),
        _ => format!("Ж;{}{i:02}", "m".repeat(58)),
    };
    let players: Vec<usize> = (0..20).map(|i| world.connect(&name(i))).collect();
    let key = create(&mut world, players[0], spec(10), None);
    for &player in &players[1..] {
        rows(&mut world, player, ROOM_JOIN_BASE + key, 1);
    }
    let watcher = world.connect("Watcher");
    let seen = members(&mut world, watcher, key);
    assert_eq!(seen.len(), 21, "{seen:?}");
    for (i, row) in seen[1..].iter().enumerate() {
        let name = world.account(players[i]).name.clone();
        assert_eq!(row[11], name);
        assert_eq!(row[11].len(), 63, "{name}");
    }
    let state = state(&mut world, players[0]);
    let cut = state
        .iter()
        .find(|row| row.starts_with("P;0;"))
        .expect("a member row");
    assert!(cut.ends_with('…'), "{cut}");
    assert!(cut.rsplit(';').next().unwrap().len() <= 32, "{cut}");
}

/// A right-click on a member (the list's side panel, by its account; the
/// room's own window, by its place and the state's serial): its kits as
/// they are now; an account in no room or a place from an older list,
/// nothing.
#[test]
fn a_member_shows_its_kits_from_the_list_and_from_the_room() {
    use survarium::v026g::lobby::rooms::member_gear;
    use survarium::v026g::lobby::{GEAR_BY_ACCOUNT, GEAR_IN_ROOM};

    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let carol = world.connect("Carol");
    let key = create(&mut world, alice, spec(8), Some(1234));
    rows(&mut world, bob, ROOM_PIN_BASE + 1234, 1);
    rows(&mut world, bob, ROOM_JOIN_BASE + key, 1);
    let gear = |world: &mut World, index: usize| -> Vec<(String, usize)> {
        world
            .take(index)
            .into_iter()
            .filter_map(|message| match message {
                Decoded::PlayerDetails { player, .. } => Some((player.name, player.kits.len())),
                _ => None,
            })
            .collect()
    };
    let kits = world.account(bob).kits.len();
    assert!(kits > 0);
    // Carol, in no room, from the list: Bob's account in his row.
    let seen = members(&mut world, carol, key);
    let account: u32 = seen[2][9].parse().unwrap();
    world.take(carol);
    world.ask(carol, request::player_details(GEAR_BY_ACCOUNT, account));
    assert_eq!(gear(&mut world, carol), [("Bob".to_owned(), kits)]);
    // Alice, the PIN room's owner, the same (its members are shown to
    // anyone).
    let alice_id: u32 = seen[1][9].parse().unwrap();
    world.ask(carol, request::player_details(GEAR_BY_ACCOUNT, alice_id));
    assert_eq!(gear(&mut world, carol).len(), 1);
    // An account in no room: no answer, whether it exists or not, online
    // or not (account ids cannot be walked).
    world.accounts.sign_up(900_001, "Offline");
    let carol_id = world.ids[carol].0;
    for account in [900_001, carol_id, 12_345_678] {
        world.ask(carol, request::player_details(GEAR_BY_ACCOUNT, account));
    }
    assert!(gear(&mut world, carol).is_empty());
    // Alice, in her room's window: Bob at place 1 of the state.
    let serial: u32 = header(&state(&mut world, alice))[12].parse().unwrap();
    world.take(alice);
    world.ask(
        alice,
        request::player_details(GEAR_IN_ROOM, member_gear(1, serial)),
    );
    assert_eq!(gear(&mut world, alice), [("Bob".to_owned(), kits)]);
    // Her own row too.
    let own = world.account(alice).kits.len();
    world.ask(
        alice,
        request::player_details(GEAR_IN_ROOM, member_gear(0, serial)),
    );
    assert_eq!(gear(&mut world, alice), [("Alice".to_owned(), own)]);
    // Bob left: the place of an older list is nobody's now.
    rows(&mut world, bob, ROOM_LEAVE, 1);
    world.ask(
        alice,
        request::player_details(GEAR_IN_ROOM, member_gear(1, serial)),
    );
    world.ask(
        alice,
        request::player_details(GEAR_IN_ROOM, member_gear(0, serial)),
    );
    assert!(gear(&mut world, alice).is_empty());
    // Bob, out of the room, is in no list now.
    world.ask(carol, request::player_details(GEAR_BY_ACCOUNT, account));
    assert!(gear(&mut world, carol).is_empty());
    // Carol, in no room, has no room's places.
    world.ask(
        carol,
        request::player_details(GEAR_IN_ROOM, member_gear(0, serial)),
    );
    assert!(gear(&mut world, carol).is_empty());
    world.assert_clean();
}

/// A room's match carries the room's minimum to the match server, which
/// starts it with who is in, or calls it off when fewer than that can play
/// it (`match.md` section 34). Called off, it shows nobody results and
/// pays nothing; its room votes and waits again, whole, and plays its next
/// match with the members it has (the live room 5209 of 2026-10-20).
#[test]
fn a_room_whose_match_is_called_off_votes_again_and_plays_with_who_is_there() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let key = create(&mut world, alice, spec(8), None);
    rows(&mut world, bob, ROOM_JOIN_BASE + key, 1);
    world.take(alice);
    world.take(bob);
    world.advance(VOTE_AND_WAIT);
    let (ticket, _) = sent_to_match(&world.take(alice)).expect("a match");
    assert!(sent_to_match(&world.take(bob)).is_some());
    let setup = world.host.made.locked().last().unwrap().1.clone();
    assert_eq!(setup.room_minimum, Some(2), "1 v 1: two players");
    let paid = |world: &World, index: usize| {
        let account = world.account(index);
        (account.generic_money, account.rating, account.level())
    };
    let before = [paid(&world, alice), paid(&world, bob)];

    world.call_off(ticket);
    world.advance(Duration::from_secs(1));
    for index in [alice, bob] {
        let decoded = world.take(index);
        assert!(
            decoded.iter().any(|message| matches!(
                message,
                Decoded::ClientState {
                    state: ClientState::Lobby,
                    ..
                }
            )),
            "back in the lobby: {decoded:?}"
        );
        assert!(
            !decoded
                .iter()
                .any(|message| matches!(message, Decoded::MatchStats { .. })),
            "no results of a match that was not made: {decoded:?}"
        );
    }
    assert_eq!(
        [paid(&world, alice), paid(&world, bob)],
        before,
        "no pay, no rating"
    );
    assert!(world.results.find(ticket).is_none(), "no report");
    let room = world.core.rooms.get(key).unwrap();
    assert_eq!(room.members.len(), 2);
    assert!(room.in_match.is_empty());
    assert_eq!(
        header(&state(&mut world, alice))[PHASE],
        "3",
        "the vote again"
    );

    // The vote and the wait, whole (no results to come back from), then
    // the next match with the two.
    world.take(alice);
    world.take(bob);
    world.advance(Duration::from_secs(16));
    assert!(
        sent_to_match(&world.take(alice)).is_none(),
        "the 30 s at the least"
    );
    world.advance(Duration::from_secs(16));
    let (again, _) = sent_to_match(&world.take(alice)).expect("Alice plays the next");
    let (with_bob, _) = sent_to_match(&world.take(bob)).expect("Bob plays the next");
    assert_eq!(again, with_bob);
    assert_ne!(again, ticket);
}

/// A member whose game started anew is an older one (it plays in no match
/// that takes players) counts for nothing: a room of four that needs four
/// makes no match of three, which the match server would call off at once
/// (`MatchSetup::room_minimum`) and the room make again, round after round.
#[test]
fn a_room_makes_no_match_below_its_minimum() {
    use survarium::v026g::match_msgs::late_join::FIRST_REVISION;

    let mut world = World::new();
    let players = ["Alice", "Bob", "Carol", "Dave"].map(|name| world.connect(name));
    let two_a_side = RoomSpec {
        min_side: 2,
        ..spec(6)
    };
    let key = create(&mut world, players[0], two_a_side, None);
    for &player in &players[1..] {
        rows(&mut world, player, ROOM_JOIN_BASE + key, 1);
    }
    let (dave, _) = world.ids[players[3]];
    let older = world
        .store
        .create_session_with_revision(dave, FIRST_REVISION - 1);
    let again = world.connect_session((dave, older));
    world.take(again);
    assert_eq!(world.core.rooms.of(dave).map(|room| room.key), Some(key));
    world.advance(VOTE_AND_WAIT * 3);
    assert!(world.host.made.locked().is_empty(), "no match of three");
    for &player in &players[..3] {
        assert!(sent_to_match(&world.take(player)).is_none());
    }
}

/// A private room (a PIN; the owner's of 2026-10-26) members join with its
/// PIN; `count` members in all, the first its owner: the room's key and the
/// members.
fn private_room(world: &mut World, count: usize) -> (u32, Vec<usize>) {
    let names = [
        "Alice", "Bob", "Carol", "Dave", "Erin", "Frank", "Grace", "Heidi", "Ivan", "Judy", "Kim",
    ];
    let members: Vec<usize> = (0..count)
        .map(|i| match names.get(i) {
            Some(name) => world.connect(name),
            None => world.connect(&format!("Member{i}")),
        })
        .collect();
    let key = create(world, members[0], spec(6), Some(1234));
    for &member in &members[1..] {
        rows(world, member, ROOM_PIN_BASE + 1234, 1);
        rows(world, member, ROOM_JOIN_BASE + key, 1);
    }
    for &member in &members {
        world.take(member);
    }
    (key, members)
}

/// The asker presses Start in its room: what it is sent (its state; "match
/// found" too when its match is made).
fn press_start(world: &mut World, index: usize) -> Vec<Decoded> {
    let addr = world.clients[index].0;
    let now = world.now;
    world.core.handle(
        addr,
        ClientMessage::QueryRanking {
            start: ROOM_START,
            count: 1,
        },
        now,
    );
    world.advance(Duration::from_millis(100));
    world.take(index)
}

/// The owner's of 2026-10-26: a private room has no team size (10 v 10, the
/// most a match has) and no countdown; its owner's Start makes its match at
/// once with whoever is in, 1 v 0 too, the sides drawn as in any room. The
/// match server is told a minimum of one (an empty side plays) and that it
/// pays nothing; its result pays nobody anything.
#[test]
fn a_private_rooms_owner_starts_its_match_with_one_three_or_five() {
    for (count, sides) in [(1, [1, 0]), (3, [2, 1]), (5, [3, 2])] {
        let mut world = World::new();
        let (key, members) = private_room(&mut world, count);
        let owner = members[0];
        let waiting = state(&mut world, owner);
        assert_eq!(header(&waiting)[5], "10", "10 v 10: {waiting:?}");
        assert_eq!(header(&waiting)[15], "1");
        // No countdown, however long.
        world.advance(VOTE_AND_WAIT * 3);
        assert!(world.host.made.locked().is_empty(), "{count}: no match");
        assert_eq!(header(&state(&mut world, owner))[PHASE], "0");
        let held: Vec<_> = members
            .iter()
            .map(|&member| {
                let account = world.account(member);
                (account.generic_money, account.exp_total, account.rating)
            })
            .collect();
        let (ticket, _) = sent_to_match(&press_start(&mut world, owner)).expect("the owner plays");
        assert_eq!(header(&state(&mut world, owner))[PHASE], "2");
        for &member in &members[1..] {
            let (sent, _) = sent_to_match(&world.take(member)).expect("each member plays");
            assert_eq!(sent, ticket);
        }
        let setup = world.host.made.locked().last().unwrap().1.clone();
        assert_eq!(setup.players.len(), count, "no bots");
        let team = |side: u8| {
            setup
                .players
                .iter()
                .filter(|player| player.profile.team == side)
                .count()
        };
        assert_eq!([team(0), team(1)], sides, "{count}");
        assert_eq!(setup.room_minimum, Some(1));
        assert!(setup.private);

        world.end_match(ticket, 0);
        world.advance(Duration::from_secs(2));
        let report = world.results.find(ticket).expect("its report");
        assert!(report.private);
        for player in &report.players {
            let reward = &player.reward;
            assert_eq!((reward.experience, reward.money, reward.drop), (0, 0, None));
            assert_eq!(reward.rating_after, reward.rating_before);
        }
        for (&member, held) in members.iter().zip(held) {
            let account = world.account(member);
            assert_eq!(
                (account.generic_money, account.exp_total, account.rating),
                held
            );
        }
        // After it the room waits for its owner again: no vote, no match.
        world.advance(VOTE_AND_WAIT * 3);
        assert_eq!(world.host.made.locked().len(), 1, "{count}: one match");
        assert_eq!(header(&state(&mut world, owner))[PHASE], "0");
        assert_eq!(world.core.rooms.get(key).unwrap().members.len(), count);
    }
}

/// Only the owner of a private room starts it, never while its match is
/// played; an open room's owner has no Start (its countdown starts it).
#[test]
fn start_is_refused_to_a_member_an_open_room_and_a_room_playing() {
    let mut world = World::new();
    let (_, members) = private_room(&mut world, 2);
    let (owner, bob) = (members[0], members[1]);
    let carol = world.connect("Carol");
    create(&mut world, carol, spec(6), None);
    assert!(sent_to_match(&press_start(&mut world, bob)).is_none());
    assert!(sent_to_match(&press_start(&mut world, carol)).is_none());
    world.advance(Duration::from_secs(2));
    assert!(world.host.made.locked().is_empty(), "nobody started");
    assert_eq!(header(&state(&mut world, carol))[PHASE], "0");
    assert!(sent_to_match(&press_start(&mut world, owner)).is_some());
    assert_eq!(world.host.made.locked().len(), 1);
    assert!(sent_to_match(&press_start(&mut world, owner)).is_none());
    world.advance(Duration::from_secs(2));
    assert_eq!(world.host.made.locked().len(), 1, "not twice");
}

/// A private room's 1 v 0 takes a member who comes while it is played
/// (with the PIN) on its empty side.
#[test]
fn a_private_rooms_1_v_0_takes_a_late_joiner_on_its_empty_side() {
    let mut world = World::new();
    let (key, members) = private_room(&mut world, 1);
    let bob = world.connect("Bob");
    let (ticket, _) = sent_to_match(&press_start(&mut world, members[0])).expect("a match");
    let (&id, _) = world
        .core
        .matches
        .iter()
        .find(|(_, running)| running.ticket.match_id == ticket)
        .unwrap();
    world.host.admitting.locked().insert(ticket);
    rows(&mut world, bob, ROOM_PIN_BASE + 1234, 1);
    rows(&mut world, bob, ROOM_JOIN_BASE + key, 1);
    world.take(bob);
    world.advance(Duration::from_secs(17));
    let (sent, _) = sent_to_match(&world.take(bob)).expect("Bob goes in late");
    assert_eq!(sent, ticket);
    assert_eq!(seat_of(&world, id, bob).1, 1);
    assert_eq!(arrangement(&mut world, members[0]), "01");
}

/// A private room's teams as its state tells them (`A;<sides>`).
fn arrangement(world: &mut World, index: usize) -> String {
    let state = state(world, index);
    state
        .iter()
        .find_map(|row| row.strip_prefix("A;"))
        .unwrap_or_else(|| panic!("no arrangement: {state:?}"))
        .to_owned()
}

/// The owner moves the member at `place` to the other team, from the state
/// it has now.
fn move_member(world: &mut World, owner: usize, place: u8) -> String {
    let serial: u32 = header(&state(world, owner))[SERIAL].parse().unwrap();
    rows(world, owner, move_start(place, serial), 1);
    arrangement(world, owner)
}

/// The owner's of 2026-10-26 ("the owner should be able to arrange both
/// teams (even 1 vs 10)"): newcomers come on the team with fewer; the
/// owner moves anyone, itself too; Start plays exactly that, 3 v 0 or
/// 1 v 10; a team holds 10 at most.
#[test]
fn a_private_rooms_owner_arranges_the_teams_and_start_plays_them() {
    let mut world = World::new();
    let (_, members) = private_room(&mut world, 3);
    let owner = members[0];
    assert_eq!(arrangement(&mut world, owner), "010");
    assert_eq!(move_member(&mut world, owner, 1), "000");
    // Itself, and back.
    assert_eq!(move_member(&mut world, owner, 0), "100");
    assert_eq!(move_member(&mut world, owner, 0), "000");
    let (ticket, _) = sent_to_match(&press_start(&mut world, owner)).expect("a match");
    let teams = |world: &World| {
        let setup = world.host.made.locked().last().unwrap().1.clone();
        let on = |side| {
            setup
                .players
                .iter()
                .filter(|player| player.profile.team == side)
                .count()
        };
        [on(0), on(1)]
    };
    assert_eq!(teams(&world), [3, 0], "3 v 0");
    world.end_match(ticket, 0);

    let mut world = World::new();
    let (_, members) = private_room(&mut world, 11);
    let owner = members[0];
    assert_eq!(arrangement(&mut world, owner), "01010101010");
    for place in (2..11).step_by(2) {
        move_member(&mut world, owner, place);
    }
    assert_eq!(arrangement(&mut world, owner), "01111111111");
    // Ten on team 1: nobody more goes there.
    assert_eq!(move_member(&mut world, owner, 0), "01111111111");
    assert!(sent_to_match(&press_start(&mut world, owner)).is_some());
    assert_eq!(teams(&world), [1, 10], "1 v 10");
}

/// The member's game started anew (it misses the next start: `room_unseen`):
/// its new game, unseen until told the room.
fn restart_game(world: &mut World, member: usize) -> usize {
    world.silent_for(member, Duration::from_secs(10));
    let (account_id, _) = world.ids[member];
    let session = world.new_session(account_id);
    let again = world.connect_session((account_id, session));
    world.take(again);
    again
}

/// The members' games told their room go into its match late, one at a
/// time, the match taking players: each one's team in it.
fn late_teams(world: &mut World, ticket: u32, id: u32, games: &[usize]) -> Vec<u8> {
    let mut teams = Vec::new();
    for &game in games {
        state(world, game);
    }
    let mut left: Vec<usize> = games.to_vec();
    for _ in 0..60 {
        world.host.admitting.locked().insert(ticket);
        world.advance(Duration::from_secs(1));
        left.retain(|&game| {
            let Some((sent, _)) = sent_to_match(&world.take(game)) else {
                return true;
            };
            assert_eq!(sent, ticket);
            let session = world.ids[game].1;
            let seats = &world.core.matches[&id].seats;
            teams.push(seats.iter().rev().find(|(s, _)| *s == session).unwrap().1);
            false
        });
        if left.is_empty() {
            return teams;
        }
    }
    panic!("not all went in: {left:?}");
}

/// The review of #527: a member who misses its owner's Start (its game
/// started anew) goes in late on the team its owner put it on, not the side
/// with fewer, and the arrangement stays: 1 v 3 stays 1 v 3 (`A;0111`).
#[test]
fn a_member_who_missed_the_start_goes_in_on_its_arranged_team() {
    let mut world = World::new();
    let (_, members) = private_room(&mut world, 4);
    let owner = members[0];
    assert_eq!(move_member(&mut world, owner, 2), "0111");
    let dave = restart_game(&mut world, members[3]);
    let (ticket, _) = sent_to_match(&press_start(&mut world, owner)).expect("a match");
    let setup = world.host.made.locked().last().unwrap().1.clone();
    assert_eq!(setup.players.len(), 3, "Dave not told the room");
    let id = *world
        .core
        .matches
        .iter()
        .find(|(_, running)| running.ticket.match_id == ticket)
        .unwrap()
        .0;
    // 1 v 2 in the match: Dave goes back to team 2, not to the side of one.
    assert_eq!(late_teams(&mut world, ticket, id, &[dave]), [1]);
    assert_eq!(arrangement(&mut world, owner), "0111");
}

/// No team ever goes past 10 in the arrangement: 10 v 9 with five of team 1
/// away at the start plays 5 v 9; a newcomer with the PIN during the match
/// goes to the side with fewer (team 1), its team in the arrangement stays
/// team 2 (team 1 has its 10); the five come back on team 1 while it has
/// places in the match, the last on team 2, and keep team 1 in the
/// arrangement.
#[test]
fn late_joiners_never_push_a_team_past_ten() {
    let mut world = World::new();
    let (key, members) = private_room(&mut world, 19);
    let owner = members[0];
    let arranged = "0101010101010101010";
    assert_eq!(arrangement(&mut world, owner), arranged);
    let away: Vec<usize> = [2, 4, 6, 8, 10]
        .iter()
        .map(|&place| restart_game(&mut world, members[place]))
        .collect();
    let (ticket, _) = sent_to_match(&press_start(&mut world, owner)).expect("a match");
    let setup = world.host.made.locked().last().unwrap().1.clone();
    let on = |side| {
        setup
            .players
            .iter()
            .filter(|player| player.profile.team == side)
            .count()
    };
    assert_eq!([on(0), on(1)], [5, 9]);
    let id = *world
        .core
        .matches
        .iter()
        .find(|(_, running)| running.ticket.match_id == ticket)
        .unwrap()
        .0;
    let newcomer = world.connect("Newcomer");
    rows(&mut world, newcomer, ROOM_PIN_BASE + 1234, 1);
    rows(&mut world, newcomer, ROOM_JOIN_BASE + key, 1);
    world.take(newcomer);
    assert_eq!(late_teams(&mut world, ticket, id, &[newcomer]), [0]);
    assert_eq!(arrangement(&mut world, owner), format!("{arranged}1"));
    let teams = late_teams(&mut world, ticket, id, &away);
    assert_eq!(teams, [0, 0, 0, 0, 1], "team 1 has 10 in the match");
    let after = arrangement(&mut world, owner);
    assert_eq!(after, format!("{arranged}1"), "10 v 10 in the arrangement");
    for side in ['0', '1'] {
        assert!(after.chars().filter(|&c| c == side).count() <= 10);
    }
}

/// A move is the owner's alone, never while the match is played; the
/// arrangement stays across a match, and a member who leaves is out of it.
#[test]
fn the_teams_are_the_owners_to_move_and_kept_across_matches() {
    let mut world = World::new();
    let (_, members) = private_room(&mut world, 4);
    let (owner, bob) = (members[0], members[1]);
    assert_eq!(arrangement(&mut world, owner), "0101");
    // Bob's move: nothing.
    let serial: u32 = header(&state(&mut world, bob))[SERIAL].parse().unwrap();
    rows(&mut world, bob, move_start(0, serial), 1);
    assert_eq!(arrangement(&mut world, owner), "0101");
    // A move from an older list: nothing.
    rows(&mut world, owner, move_start(0, serial + 1), 1);
    assert_eq!(arrangement(&mut world, owner), "0101");
    assert_eq!(move_member(&mut world, owner, 3), "0100");
    let (ticket, _) = sent_to_match(&press_start(&mut world, owner)).expect("a match");
    // While it is played: no move.
    assert_eq!(move_member(&mut world, owner, 3), "0100");
    world.end_match(ticket, 0);
    world.advance(Duration::from_secs(2));
    assert_eq!(
        arrangement(&mut world, owner),
        "0100",
        "kept after the match"
    );
    // Carol leaves: out of it, the others as they were.
    rows(&mut world, members[2], ROOM_LEAVE, 1);
    assert_eq!(arrangement(&mut world, owner), "010");
}

/// Two accounts made friends, both ways, as an accepted request does.
fn befriend(world: &mut World, a: usize, b: usize) {
    let (a, b) = (world.ids[a].0, world.ids[b].0);
    world.store.account(a).unwrap().locked().friends.push(b);
    world.store.account(b).unwrap().locked().friends.push(a);
}

fn friend_rooms(world: &mut World, index: usize) -> Vec<String> {
    rows(world, index, ROOM_FRIENDS, 1)
}

/// The owner's of 2026-10-20: a friend's right-click menu offers "Join
/// room" while the friend is a member of a room, and joins it as the list
/// does. The answer names the friends in rooms (not one in no room, not one
/// waiting in a queue, not someone who is no friend, not the asker's own
/// room) with what the join needs: a PIN is asked first, a full room is
/// queued for.
#[test]
fn a_friends_menu_knows_which_friends_are_in_a_room_and_joins_it_as_the_list() {
    let mut world = World::new();
    let me = world.connect("Me");
    // Alice: a friend in a room with a PIN. Bob: a friend in no room.
    // Carol: in a room, no friend. Dave: a friend waiting in a queue. Erin:
    // a friend in a full room.
    let [alice, bob, carol, dave, erin] =
        ["Alice", "Bob", "Carol", "Dave", "Erin"].map(|name| world.connect(name));
    for friend in [alice, bob, dave, erin] {
        befriend(&mut world, me, friend);
    }
    assert_eq!(friend_rooms(&mut world, me), ["S1;Y;0"]);
    let pinned = create(&mut world, alice, spec(8), Some(1234));
    create(&mut world, carol, spec(8), None);
    let full = create(&mut world, erin, spec(6), None);
    for n in 1..12 {
        let member = world.connect(&format!("Member{n}"));
        rows(&mut world, member, ROOM_JOIN_BASE + full, 1);
    }
    assert_eq!(
        rows(&mut world, dave, ROOM_QUEUE_BASE + full, 1),
        [format!("S1;N;0;{full};1")]
    );
    let (alice_id, erin_id) = (world.ids[alice].0, world.ids[erin].0);
    assert_eq!(
        friend_rooms(&mut world, me),
        [
            "S1;Y;2".to_owned(),
            // A private room is 10 v 10 whatever was asked.
            format!("{alice_id};{pinned};1;10;1"),
            format!("{erin_id};{full};12;6;0"),
        ]
    );
    // Somebody else's friends are its own: Carol has none.
    assert_eq!(friend_rooms(&mut world, carol), ["S1;Y;0"]);

    // The full room: the movie queues, as the list's Queue.
    assert_eq!(
        rows(&mut world, me, ROOM_QUEUE_BASE + full, 1),
        [format!("S1;N;0;{full};2")]
    );
    // A queue is no membership: still both rooms offered.
    assert_eq!(friend_rooms(&mut world, me).len(), 3);
    // The PIN room: refused without it (the queue kept), joined with it
    // (the queue left).
    assert_eq!(
        rows(&mut world, me, ROOM_JOIN_BASE + pinned, 1),
        [format!("S1;N;3;{full};2")]
    );
    assert_eq!(rows(&mut world, me, ROOM_PIN_BASE + 1234, 1), ["S1;K;1"]);
    let joined = rows(&mut world, me, ROOM_JOIN_BASE + pinned, 1);
    assert_eq!(header(&joined)[KEY], pinned.to_string(), "{joined:?}");
    // In Alice's room now: hers is not offered, Erin's still is; Alice is
    // offered nothing (Me is in her own room).
    assert_eq!(
        friend_rooms(&mut world, me),
        ["S1;Y;1".to_owned(), format!("{erin_id};{full};12;6;0")]
    );
    assert_eq!(friend_rooms(&mut world, alice), ["S1;Y;0"]);
    // Erin leaves her room: nothing of it is offered.
    rows(&mut world, erin, ROOM_LEAVE, 1);
    assert_eq!(friend_rooms(&mut world, me), ["S1;Y;0"]);
    // Bob, in no room, is offered Me's room (Alice's), with its PIN.
    let me_id = world.ids[me].0;
    assert_eq!(
        friend_rooms(&mut world, bob),
        ["S1;Y;1".to_owned(), format!("{me_id};{pinned};2;10;1")]
    );
}

/// A friend's room whose match is being played: joined from the friend's
/// menu as from the list, the joiner sees the room first and goes into the
/// match later (the late join's path).
#[test]
fn joining_a_friends_room_during_its_match_goes_the_late_join_path() {
    let mut world = World::new();
    let (key, ticket, _, players) = room_match_of_four(&mut world);
    let me = world.connect("Me");
    befriend(&mut world, me, players[1]);
    let bob_id = world.ids[players[1]].0;
    assert_eq!(
        friend_rooms(&mut world, me),
        ["S1;Y;1".to_owned(), format!("{bob_id};{key};4;6;0")]
    );
    world.host.admitting.locked().insert(ticket);
    let joined = rows(&mut world, me, ROOM_JOIN_BASE + key, 1);
    assert_eq!(header(&joined)[KEY], key.to_string(), "{joined:?}");
    assert!(sent_to_match(&world.take(me)).is_none());
    let waiting = state(&mut world, me);
    assert!(waiting.iter().any(|row| row == "J;15"), "{waiting:?}");
}

/// A member whose kit can not play (an empty loadout) is offered no room to
/// switch to: the switch is a Join, which the door would refuse (code 15).
#[test]
fn a_member_whose_kit_can_not_play_is_offered_no_switch() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let erin = world.connect("Erin");
    let switch_of = |rows: &[String]| rows.iter().any(|row| row.starts_with("W;"));
    let any = create(
        &mut world,
        alice,
        RoomSpec {
            max_level: 15,
            ..spec(8)
        },
        None,
    );
    rows(&mut world, bob, ROOM_JOIN_BASE + any, 1);
    create(
        &mut world,
        erin,
        RoomSpec {
            max_level: 10,
            ..spec(6)
        },
        None,
    );
    assert!(switch_of(&state(&mut world, bob)));
    let starter = world.account(bob).kits[0].slots;
    world.change(bob, |account| account.kits[0].slots = [None; 23]);
    assert!(!switch_of(&state(&mut world, bob)));
    world.change(bob, |account| account.kits[0].slots = starter);
    assert!(switch_of(&state(&mut world, bob)));
}

/// A queued player whose kit can not play when a place frees (it switched
/// to an empty loadout while waiting) keeps its place, passed over: the next
/// goes in; it goes in itself once its kit plays and a place frees.
#[test]
fn a_queued_player_whose_kit_can_not_play_keeps_its_place_and_the_next_goes_in() {
    let mut world = World::new();
    let owner = world.connect("Owner");
    let key = create(&mut world, owner, spec(6), None);
    let members: Vec<usize> = (1..12)
        .map(|n| world.connect(&format!("Member{n}")))
        .collect();
    for &member in &members {
        rows(&mut world, member, ROOM_JOIN_BASE + key, 1);
    }
    let first = world.connect("First");
    let second = world.connect("Second");
    rows(&mut world, first, ROOM_QUEUE_BASE + key, 1);
    rows(&mut world, second, ROOM_QUEUE_BASE + key, 1);
    let starter = world.account(first).kits[0].slots;
    world.change(first, |account| account.kits[0].slots = [None; 23]);
    rows(&mut world, members[0], ROOM_LEAVE, 1);
    world.advance(Duration::from_secs(1));
    let inside = state(&mut world, second);
    assert_eq!(header(&inside)[KEY], key.to_string(), "{inside:?}");
    assert_eq!(state(&mut world, first), [format!("S1;N;0;{key};1")]);
    // Its kit plays again, a place frees: in.
    world.change(first, |account| account.kits[0].slots = starter);
    rows(&mut world, members[1], ROOM_LEAVE, 1);
    world.advance(Duration::from_secs(1));
    let inside = state(&mut world, first);
    assert_eq!(header(&inside)[KEY], key.to_string(), "{inside:?}");
}

/// Blackout (`docs/notes/blackout.md`): the rooms always offer
/// each plain battery map of the pool once more as mode 6, by a lobby id of
/// its own (its tile the battery map's), as part of the standard pool (the
/// owner's of 2026-10-25): in the create window's defaults and in a room
/// Play makes; Play puts players into a room that offers it; its match
/// plays the battery map with the flag.
#[test]
fn blackout_is_a_mode_of_the_standard_pool() {
    let mut settings = Settings::local("127.0.0.1:0");
    // Rudnya's day: batteries (61), chargers (62), Team Deathmatch (117).
    settings.maps = vec![61, 62, 117];
    let mut world = World::with_settings(settings);
    let entries = world.core.room_entries();
    let blackout = maps::mode::BLACKOUT;
    let offered: Vec<_> = entries
        .iter()
        .filter(|entry| entry.mode == blackout)
        .copied()
        .collect();
    assert_eq!(offered.len(), 1, "{entries:?}");
    let lobby_id = offered[0].map_id;
    assert_eq!(maps::blackout_played(lobby_id), Some(61));
    assert_eq!(
        (offered[0].level, offered[0].weather),
        (entries[0].level, entries[0].weather)
    );
    let index = entries
        .iter()
        .position(|entry| entry.mode == blackout)
        .unwrap();

    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let carol = world.connect("Carol");
    // The defaults have it.
    let (_, chosen) = form(&mut world, alice);
    assert!(chosen.contains(&lobby_id), "{chosen:?}");
    // A room Play makes has it.
    let made_by_play: u32 = play_into_room(&mut world, carol).parse().unwrap();
    assert!(
        world
            .core
            .rooms
            .get(made_by_play)
            .unwrap()
            .pool
            .contains(&lobby_id)
    );
    rows(&mut world, carol, ROOM_LEAVE, 1);
    // Alice's room: Blackout alone. Play puts Bob into it.
    rows(&mut world, alice, bits_start(0, 1 << index), 1);
    let key = create(
        &mut world,
        alice,
        RoomSpec {
            max_level: 10,
            ..spec(6)
        },
        None,
    );
    assert_eq!(play_into_room(&mut world, bob), key.to_string());
    let ticket = room_match_starts(&mut world, &[alice, bob], VOTE_AND_WAIT);
    let made = world.host.made.locked().clone();
    let (_, setup) = made
        .iter()
        .find(|(id, _)| *id == ticket)
        .expect("the room's match");
    assert_eq!(setup.map_id, 61, "the battery map");
    assert!(setup.blackout);
}
