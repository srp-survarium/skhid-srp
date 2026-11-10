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
    let ask = ReplayWatchAsk {
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

/// A Live spectator whose game never reaches the match server, or which
/// signs in anew, gives its place back when its lobby link or session goes:
/// not only when the match ends.
#[test]
fn a_live_watcher_gives_its_place_back_when_its_link_goes() {
    let mut world = World::new();
    let (ticket, _) = a_match_in_play(&mut world);
    let carol = world.connect("Carol");
    assert!(ask_to_watch(&mut world, carol, ticket).starts_with("R1;watching;"));
    let session = world.ids[carol].1;
    let matchmaking = |world: &World| world.store.matchmaking().watched(session);
    assert!(matchmaking(&world).is_some());
    // Its game crashed while loading: nothing more comes from it.
    drop(std::mem::replace(&mut world.clients[carol].1, TestClient::new()));
    world.advance(world.core.settings.match_link_timeout + Duration::from_secs(1));
    assert_eq!(matchmaking(&world), None);
}

/// The session's end is enough on its own, whichever way it comes.
#[test]
fn a_live_watcher_watches_no_more_when_its_session_ends() {
    let mut world = World::new();
    let (ticket, _) = a_match_in_play(&mut world);
    let carol = world.connect("Carol");
    assert!(ask_to_watch(&mut world, carol, ticket).starts_with("R1;watching;"));
    let session = world.ids[carol].1;
    assert!(world.store.end_session(session));
    assert_eq!(world.store.matchmaking().watched(session), None);
}

/// A player with a replay session on (its recording still being read, so
/// its game is free in the lobby) does not also become a spectator.
#[test]
fn a_replay_session_keeps_its_player_from_watching_a_match_too() {
    use session::v026g::replay_watch::WatchSettings;
    use session::v026g::replays::Replays;
    let desk: &'static Replays =
        Box::leak(Box::new(Replays::new(None).with_watch(WatchSettings::default())));
    let mut world = World::new();
    world.core.replays = Some(desk);
    let (ticket, _) = a_match_in_play(&mut world);
    let carol = world.connect("Carol");
    assert!(ask_to_watch(&mut world, carol, 0x1F07_C8FB).starts_with("R1;preparing;"));
    assert_eq!(
        ask_to_watch(&mut world, carol, ticket),
        format!("R1;busy;{ticket:08x};0")
    );
    assert_eq!(world.store.matchmaking().watched(world.ids[carol].1), None);
}

/// Past the limit of lookups of unknown matches, an ask says nothing of a
/// match being played either: asking was how to tell a running private
/// match from no match at all.
#[test]
fn past_the_guessing_limit_a_running_match_is_not_told_apart() {
    use session::v026g::replay_watch::{MISSES_ALLOWED, WatchSettings};
    use session::v026g::replays::Replays;
    let desk: &'static Replays =
        Box::leak(Box::new(Replays::new(None).with_watch(WatchSettings::default())));
    let mut world = World::new();
    world.core.replays = Some(desk);
    let (ticket, _) = a_match_in_play(&mut world);
    let carol = world.connect("Carol");
    let account = world.ids[carol].0;
    for n in 0..MISSES_ALLOWED {
        desk.watches().missed(account, 0x2000 + n, world.now);
    }
    assert_eq!(
        ask_to_watch(&mut world, carol, ticket),
        format!("R1;busy;{ticket:08x};0")
    );
    assert_eq!(world.store.matchmaking().watched(world.ids[carol].1), None);
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

    // Watching: sent to the match as a spectator, 30 s behind, Bob (place
    // 1) in view first.
    world.take(carol);
    let ask = ReplayWatchAsk {
        seconds: 0,
        place: Some(1),
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
            delay: session::LiveDelay::DEFAULT,
            view: Some(1),
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

/// A room's member who watches a live match meanwhile is called back when its room could play
/// with it (`room_recalls`): alone in its room it watches on; a second member makes the room's
/// minimum with it, and its watch ends.
#[test]
fn a_member_watching_live_is_called_back_when_its_room_reaches_its_minimum() {
    let mut world = World::new();
    let (ticket, _) = a_match_in_play(&mut world);
    let carol = world.connect("Carol");
    let key = create(&mut world, carol, spec(6), None);
    assert!(ask_to_watch(&mut world, carol, ticket).starts_with("R1;watching;"));
    let session = world.ids[carol].1;
    world.advance(Duration::from_secs(5));
    assert!(world.store.matchmaking().watched(session).is_some(), "alone in its room: it watches on");
    let dave = world.connect("Dave");
    rows(&mut world, dave, ROOM_JOIN_BASE + key, 1);
    world.advance(Duration::from_secs(1));
    assert!(world.store.matchmaking().is_recalled(session), "its room has its minimum with it: called back");
    // The match server tells its game, and stops the watch a few seconds later.
    assert!(world.store.matchmaking().watched(session).is_some());
    world.store.matchmaking().stop_watching(session);
    assert!(!world.store.matchmaking().is_recalled(session));
}

/// One in no room is nobody's to call back, whatever the rooms do.
#[test]
fn a_watcher_in_no_room_is_not_called_back() {
    let mut world = World::new();
    let (ticket, _) = a_match_in_play(&mut world);
    let carol = world.connect("Carol");
    assert!(ask_to_watch(&mut world, carol, ticket).starts_with("R1;watching;"));
    let session = world.ids[carol].1;
    let dave = world.connect("Dave");
    let erin = world.connect("Erin");
    let key = create(&mut world, dave, spec(6), None);
    rows(&mut world, erin, ROOM_JOIN_BASE + key, 1);
    world.advance(Duration::from_secs(5));
    assert!(world.store.matchmaking().watched(session).is_some());
    assert!(!world.store.matchmaking().is_recalled(session));
}
