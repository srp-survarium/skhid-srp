//! Rooms (`rooms`): a room made, listed and joined; its countdown from two
//! players, the vote and the match it starts on the card chosen; the room
//! again after the match; Play into a room; a PIN, a max level and a kick;
//! a late joiner; and no room for an older game.

use session::v026g::matchmaking;
use session::v026g::rooms::Stage;
use survarium::v026g::lobby::rooms::{
    ROOM_FORM, ROOM_JOIN_BASE, ROOM_JOIN_MATCH, ROOM_LEAVE, ROOM_PIN_BASE, ROOM_PLAY,
    ROOM_QUEUE_BASE, ROOM_STATE, ROOM_UNQUEUE, ROOM_VOTE_BASE, ROOMS_LIST, RoomSpec, bits_start,
    create_start, kick_start,
};

use super::pvb_tests::with_gear;
use super::*;
use crate::v026g::LiveView;

/// Fields of the state's header (`S1;R;<key>;<level>;<max level>;<size>;
/// <wait>;<phase>;<secs>;<owner>;<pool>;<members>;<serial>;<pin>`).
const KEY: usize = 2;
const LEVEL: usize = 3;
const PHASE: usize = 7;
const OWNER: usize = 9;
const MEMBERS: usize = 11;
const SERIAL: usize = 12;
const NEXT: usize = 14;
/// From two players to the match: the vote's 15 s, the wait of [`spec`]
/// (15 s), and a little more.
const VOTE_AND_WAIT: Duration = Duration::from_secs(32);

/// The names of the rows of the one answer to a request at `start`.
fn rows(world: &mut World, index: usize, start: u32, count: u8) -> Vec<String> {
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
            "S1;R;{key};0;0;6;15;0;0;1;{};1;{serial};0;0;2",
            world.core.settings.maps.len()
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
    assert_eq!(
        setup.map_id, voted_cards[2],
        "the vote's winner when it started"
    );
    assert_eq!(setup.players.len(), 2, "no bots");
    assert!(!setup.vs_bots);
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
    assert_eq!(&header(&made)[LEVEL..PHASE], ["0", "0", "8", "30"]);
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
fn play_takes_the_room_nearest_the_players_level_however_far() {
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
    let high = create(&mut world, dave, spec(8), None).to_string();
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

/// A member is offered the open room of the lowest max level that takes it,
/// lower than its own room's, and one click (a Join) moves it there; none
/// over its equipment, none with a PIN, none in its own room (the owner's
/// of 2026-10-19).
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
    let any = create(&mut world, alice, spec(8), None);
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
            max_level: 3,
            ..spec(8)
        },
        Some(1234),
    );
    let low = create(
        &mut world,
        erin,
        RoomSpec {
            max_level: 4,
            ..spec(6)
        },
        None,
    );
    // The lowest open one: Erin's (Dave's has a PIN).
    let offered = switch_of(&state(&mut world, bob)).expect("a room offered");
    assert_eq!(offered, ["W", &low.to_string(), "4", "1", "6", "Erin"]);
    // One equipped over 4 is offered Carol's.
    with_gear(&mut world, alice, 5);
    let offered = switch_of(&state(&mut world, alice)).expect("Carol's");
    assert_eq!(offered[2], "5");
    // Bob's click: into Erin's room, out of Alice's.
    let moved = rows(&mut world, bob, ROOM_JOIN_BASE + low, 1);
    assert_eq!(header(&moved)[KEY], low.to_string());
    assert_eq!(header(&state(&mut world, alice))[MEMBERS], "1");
    // In Erin's, nothing lower is open: no offer.
    assert_eq!(switch_of(&state(&mut world, bob)), None);
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
    assert_eq!(&header(&made)[LEVEL..LEVEL + 2], ["5", "0"]);
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
        ["S1;N;12;0;0"]
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
/// without it; it may come back.
#[test]
fn a_kit_without_ammo_counts_for_nothing_and_is_taken_out_at_the_start() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let dave = world.connect("Dave");
    let carol = world.connect("Carol");
    let key = create(&mut world, alice, spec(6), None);
    world.change(dave, |account| {
        for slot in [8, 9, 11, 12, 19, 20, 21, 22] {
            account.kits[0].slots[slot] = None;
        }
    });
    rows(&mut world, dave, ROOM_JOIN_BASE + key, 1);
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
    // Taken out, not turned out: Dave may come back.
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
    let key = create(&mut world, alice, spec(6), None);
    for index in [bob, carol, dave] {
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

/// Who comes by Play is a member as any other: after the room's match it
/// is in the room still, until it leaves (the owner's of 2026-10-17). The
/// movie's own Play (no search) puts it there.
#[test]
fn a_player_who_came_by_play_stays_in_the_room_after_the_match() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let key = create(&mut world, alice, spec(6), None);
    let joined = rows(&mut world, bob, ROOM_PLAY, 1);
    assert_eq!(header(&joined)[KEY], key.to_string(), "{joined:?}");
    world.take(alice);
    world.take(bob);
    world.advance(VOTE_AND_WAIT);
    let (match_id, _) = sent_to_match(&world.take(alice)).expect("a match");
    assert!(sent_to_match(&world.take(bob)).is_some());
    world.end_match(match_id, 0);
    world.advance(Duration::from_secs(1));
    let after = state(&mut world, bob);
    assert_eq!(header(&after)[KEY], key.to_string(), "{after:?}");
    assert_eq!(header(&after)[MEMBERS], "2");
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
    // Play's room after it keeps what the hand made.
    rows(&mut world, alice, ROOM_LEAVE, 1);
    world.ask(alice, request::play(1));
    world.advance(Duration::from_millis(100));
    assert_eq!(form(&mut world, alice).1, made);
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
                running.strengths.push(0);
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
