//! The Live window (`live`): the list of the matches being played, a
//! match's roster, and watching one as it is played, behind it.

use survarium::v026g::lobby::rooms::ROOM_JOIN_BASE;
use survarium::v026g::lobby::{
    LIVE_ROSTER_ASK, LIVE_ROSTER_START, LIVE_START, REPLAY_START, ReplayWatchAsk,
};

use super::rooms_tests::{befriend, create, rows, spec};
use super::*;
use crate::v026g::{LivePhase, LiveView};

/// From two players to their room's match: the vote, the wait, a little
/// more.
const ROOM_TO_MATCH: Duration = Duration::from_secs(32);

/// Alice and Bob in a room's match, which the match server has in play:
/// its ticket, the world's indexes `[alice, bob]`.
fn a_match_in_play(world: &mut World) -> (u32, [usize; 2]) {
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let key = create(world, alice, spec(6), None);
    rows(world, bob, ROOM_JOIN_BASE + key, 1);
    world.advance(ROOM_TO_MATCH);
    let (ticket, _) = sent_to_match(&world.take(alice)).expect("Alice plays");
    let mut kills_deaths = [(0, 0); session::MAX_PLAYERS_PER_MATCH];
    kills_deaths[0] = (3, 1);
    kills_deaths[1] = (1, 3);
    world.host.live.locked().insert(
        ticket,
        LiveView {
            present: 0b11,
            points: Some([2, 1]),
            played: Some(Duration::from_secs(75)),
            phase: LivePhase::Playing,
            kills_deaths,
            ..LiveView::default()
        },
    );
    (ticket, [alice, bob])
}

/// The movie's ask to watch `match_id` (a replay session's): its `R1` row.
fn ask_to_watch(world: &mut World, index: usize, match_id: u32) -> String {
    world.take(index);
    let ask = ReplayWatchAsk::At {
        seconds: 0,
        place: None,
    };
    world.ask(index, request::player_details(ask.player_id(), match_id));
    world
        .take(index)
        .iter()
        .filter_map(|message| match message {
            Decoded::EloList(rows) => rows
                .iter()
                .find(|(name, place, _, _)| *place == REPLAY_START && name.starts_with("R1;"))
                .map(|(name, ..)| name.clone()),
            _ => None,
        })
        .next()
        .expect("an R1 row")
}

#[test]
fn the_live_window_lists_a_match_its_roster_and_watches_it_behind() {
    let mut world = World::new();
    let (ticket, _) = a_match_in_play(&mut world);
    let carol = world.connect("Carol");
    let list = rows(&mut world, carol, LIVE_START, 20);
    assert_eq!(list.len(), 2, "{list:?}");
    assert_eq!(list[0], "L0;1;30");
    let fields: Vec<&str> = list[1].split(';').collect();
    assert_eq!(fields[0], "L1");
    assert_eq!(fields[1], format!("{ticket:08x}"));
    // In play, one a side, 2 to 1, 75 s, nobody watching, may be watched.
    assert_eq!(&fields[4..], ["2", "1", "1", "2", "1", "75", "0", "1"]);
    assert!(list.iter().all(|row| row.len() <= 63));

    world.ask(carol, request::player_details(LIVE_ROSTER_ASK, ticket));
    let roster = rows(&mut world, carol, LIVE_ROSTER_START, 24);
    assert_eq!(roster.len(), 2, "{roster:?}");
    let alice: Vec<&str> = roster[0].split(';').collect();
    assert_eq!(alice[..2], ["L2", format!("{ticket:08x}").as_str()]);
    assert_eq!(alice[2], "0");
    assert_eq!(alice[4..], ["3", "1", "Alice"]);
    assert!(roster[1].ends_with(";1;3;Bob"), "{}", roster[1]);

    // Watching: sent to the match as a spectator, 30 s behind.
    world.take(carol);
    let ask = ReplayWatchAsk::At {
        seconds: 0,
        place: None,
    };
    world.ask(carol, request::player_details(ask.player_id(), ticket));
    world.advance(Duration::from_secs(1));
    let decoded = world.take(carol);
    assert_eq!(sent_to_match(&decoded), Some((ticket, 0)));
    assert!(decoded.iter().any(|message| matches!(
        message,
        Decoded::EloList(rows) if rows[0].0 == format!("R1;watching;{ticket:08x};0")
    )));
    let session = world.ids[carol].1;
    let watcher = world.store.matchmaking().watched(session).expect("watches");
    assert_eq!(
        watcher.kind,
        session::WatchKind::Live {
            delay: Duration::from_secs(30)
        }
    );
    // No room's queue keeps it, and none ends it.
    world.advance(Duration::from_secs(5));
    assert!(world.store.matchmaking().watched(session).is_some());
    let list = rows(&mut world, carol, LIVE_START, 20);
    assert!(list[1].ends_with(";1;1"), "{}", list[1]);
}

/// The asker's friends in a listed match, after the `L1` rows: those
/// present in it, by place and team.
#[test]
fn the_live_list_names_the_askers_friends_in_each_match() {
    let mut world = World::new();
    let (ticket, [alice, bob]) = a_match_in_play(&mut world);
    let carol = world.connect("Carol");
    befriend(&mut world, carol, bob);
    let list = rows(&mut world, carol, LIVE_START, 20);
    assert_eq!(list.len(), 3, "{list:?}");
    assert!(list[1].starts_with("L1;"));
    assert_eq!(list[2], format!("F1;{ticket:08x};1;1;Bob"));
    // Alice too; Bob gone from the match: Alice alone.
    befriend(&mut world, carol, alice);
    world.host.live.locked().get_mut(&ticket).unwrap().present = 0b01;
    let list = rows(&mut world, carol, LIVE_START, 20);
    assert_eq!(list[2..], [format!("F1;{ticket:08x};0;0;Alice")]);
    // Nobody's friend: no F1 row.
    let dave = world.connect("Dave");
    assert_eq!(rows(&mut world, dave, LIVE_START, 20).len(), 2);
}

/// Caps: one Live spectator a match here; past it, `busy` and the list
/// says full. A match not begun is not watched yet.
#[test]
fn a_full_match_is_busy_and_one_not_begun_is_not_watched() {
    let mut settings = Settings::local("127.0.0.1:0");
    settings.live.per_match = 1;
    let mut world = World::with_settings(settings);
    let (ticket, _) = a_match_in_play(&mut world);
    let carol = world.connect("Carol");
    let dave = world.connect("Dave");
    assert!(ask_to_watch(&mut world, carol, ticket).starts_with("R1;watching;"));
    assert_eq!(
        ask_to_watch(&mut world, dave, ticket),
        format!("R1;busy;{ticket:08x};0")
    );
    let list = rows(&mut world, dave, LIVE_START, 20);
    assert!(list[1].ends_with(";1;0"), "{}", list[1]);
    world.host.live.locked().get_mut(&ticket).unwrap().phase = LivePhase::Gathering;
    let erin = world.connect("Erin");
    assert_eq!(
        ask_to_watch(&mut world, erin, ticket),
        format!("R1;busy;{ticket:08x};0")
    );
}
