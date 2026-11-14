//! The Live window (`live`): the list of the matches being played, a
//! match's roster, and watching one as it is played, behind it.

use survarium::v026g::lobby::rooms::{ROOM_JOIN_BASE, ROOM_PLAY};
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
pub(super) fn a_match_in_play(world: &mut World) -> (u32, [usize; 2]) {
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
pub(super) fn ask_to_watch(world: &mut World, index: usize, match_id: u32) -> String {
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

/// A Live spectator whose game never reaches the match server gives its
/// place back when its session goes, its link silent for the session's
/// wait: not only when the match ends. Until then the same game connecting
/// again watches on. (Timeouts short enough for the session to end before
/// [`session::WATCH_CONNECT_WITHIN`], which ends such a watch too.)
#[test]
fn a_live_watcher_gives_its_place_back_when_its_link_goes() {
    let mut settings = Settings::local("127.0.0.1:0");
    settings.match_link_timeout = settings.link_timeout;
    settings.session_grace = Duration::from_secs(20);
    assert!(
        settings.link_timeout + settings.session_grace + Duration::from_secs(3)
            < session::WATCH_CONNECT_WITHIN
    );
    let mut world = World::with_settings(settings);
    let (ticket, _) = a_match_in_play(&mut world);
    let carol = world.connect("Carol");
    assert!(ask_to_watch(&mut world, carol, ticket).starts_with("R1;watching;"));
    let session = world.ids[carol].1;
    let matchmaking = |world: &World| world.store.matchmaking().watched(session);
    assert!(matchmaking(&world).is_some());
    // Its game crashed while loading: nothing more comes from it.
    drop(std::mem::replace(&mut world.clients[carol].1, TestClient::new()));
    world.advance(world.core.settings.match_link_timeout + Duration::from_secs(1));
    assert!(matchmaking(&world).is_some(), "its session waits");
    world.advance(world.core.settings.session_grace + Duration::from_secs(2));
    assert_eq!(matchmaking(&world), None);
    assert!(world.store.has_ended(session));
}

/// The review of #1537: a spectator whose game connects again with its
/// session (its lobby link dropped, or went silent, while its game watches)
/// watches on, as a seated player keeps its place.
#[test]
fn a_live_watcher_whose_game_connects_again_watches_on() {
    let mut world = World::new();
    let (ticket, _) = a_match_in_play(&mut world);
    let carol = world.connect("Carol");
    assert!(ask_to_watch(&mut world, carol, ticket).starts_with("R1;watching;"));
    let session = world.ids[carol].1;
    // Its game is there: the match server took it as a spectator.
    world.store.matchmaking().watcher_connected(session, ticket);
    let watched = |world: &World| world.store.matchmaking().watched(session);
    let before = watched(&world);
    assert!(before.is_some());
    // Its link dropped on the game's side: it connects again at once.
    world.game_connects_again(carol, 41_000);
    assert_eq!(watched(&world), before);
    // Its link went silent on the lobby's side first.
    let silent = world.core.settings.match_link_timeout + Duration::from_secs(1);
    world.silent_for(carol, silent);
    assert!(!world.core.links.contains_key(&world.clients[carol].0), "its link went");
    world.game_connects_again(carol, 41_001);
    assert_eq!(watched(&world), before);
    assert!(!world.store.has_ended(session));
    // Its game is not sent to the match again: it is there.
    world.advance(Duration::from_secs(2));
    assert!(sent_to_match(&world.take(carol)).is_none());
}

/// The review of #1537: a player whose game started anew while its seat is
/// kept, its room keeping no place for it now (the match takes nobody), is
/// free in the lobby but does not watch its own match from the Live window:
/// it would see both teams, then go back to its seat by Play. Play still
/// takes it back.
#[test]
fn a_player_whose_seat_is_kept_does_not_watch_its_match() {
    let mut world = World::new();
    let (ticket, [_, bob]) = a_match_in_play(&mut world);
    // Bob's game is killed and started anew: a new session.
    world.silent_for(bob, Duration::from_secs(10));
    let account = world.ids[bob].0;
    let session = world.new_session(account);
    let again = world.connect_session((account, session));
    assert_eq!(
        ask_to_watch(&mut world, again, ticket),
        format!("R1;busy;{ticket:08x};0")
    );
    assert_eq!(world.store.matchmaking().watched(session), None);
    world.ask(again, request::ranking(ROOM_PLAY, 1));
    let (back, _) = sent_to_match(&world.take(again)).expect("back to its seat");
    assert_eq!(back, ticket);
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

/// A Live watch whose game never reaches the match server, its lobby link
/// alive, ends after [`session::WATCH_CONNECT_WITHIN`] and frees its place
/// for another; one whose game the match server took watches on. Its
/// account is `busy` for [`session::WATCH_COOLDOWN`], taking no place.
#[test]
fn a_live_watch_whose_game_never_comes_frees_its_place() {
    let mut settings = Settings::local("127.0.0.1:0");
    settings.live.per_match = 1;
    let mut world = World::with_settings(settings);
    let (ticket, _) = a_match_in_play(&mut world);
    let carol = world.connect("Carol");
    let dave = world.connect("Dave");
    assert!(ask_to_watch(&mut world, carol, ticket).starts_with("R1;watching;"));
    let carols = world.ids[carol].1;
    world.advance(session::WATCH_CONNECT_WITHIN - Duration::from_secs(2));
    assert!(world.store.matchmaking().watched(carols).is_some());
    assert_eq!(
        ask_to_watch(&mut world, dave, ticket),
        format!("R1;busy;{ticket:08x};0")
    );
    world.advance(Duration::from_secs(3));
    assert_eq!(world.store.matchmaking().watched(carols), None);
    // Carol asks again at once: busy, and the place stays free.
    let busy = format!("R1;busy;{ticket:08x};0");
    assert_eq!(ask_to_watch(&mut world, carol, ticket), busy);
    assert_eq!(world.store.matchmaking().watched(carols), None);
    assert!(ask_to_watch(&mut world, dave, ticket).starts_with("R1;watching;"));
    // Dave's game comes (the match server says so): its watch goes on.
    let daves = world.ids[dave].1;
    world.store.matchmaking().watcher_connected(daves, ticket);
    world.advance(session::WATCH_CONNECT_WITHIN + Duration::from_secs(1));
    assert!(world.store.matchmaking().watched(daves).is_some());
    // Its game leaves; Carol is still cooling down, then watches.
    world.store.matchmaking().watcher_left(daves, ticket);
    assert_eq!(ask_to_watch(&mut world, carol, ticket), busy);
    world.advance(session::WATCH_COOLDOWN);
    assert!(ask_to_watch(&mut world, carol, ticket).starts_with("R1;watching;"));
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

/// A squad alone in an open room is no match (`room_tick`): one of it
/// watching live is not called back for its mate alone.
#[test]
fn a_member_waiting_with_only_its_squad_mate_is_not_called_back() {
    use survarium::v026g::lobby::rooms::ROOM_LEAVE;
    let mut world = World::new();
    let (ticket, _) = a_match_in_play(&mut world);
    let dave = world.connect("Dave");
    let erin = world.connect("Erin");
    let frank = world.connect("Frank");
    form_squad(&mut world, dave, erin);
    let key = create(&mut world, frank, spec(6), None);
    rows(&mut world, dave, ROOM_JOIN_BASE + key, 1);
    rows(&mut world, frank, ROOM_LEAVE, 1);
    assert_eq!(world.core.rooms.get(key).map(|room| room.members.len()), Some(2));
    assert!(ask_to_watch(&mut world, erin, ticket).starts_with("R1;watching;"));
    let session = world.ids[erin].1;
    world.advance(Duration::from_secs(5));
    assert!(world.store.matchmaking().watched(session).is_some());
    assert!(!world.store.matchmaking().is_recalled(session));
}

/// A Live spectator that leaves its match keeps its place in a room's
/// queue: that queue is not of the match it left (Queue and spectate's is).
#[test]
fn a_live_watcher_leaving_its_match_keeps_its_room_queue() {
    use survarium::v026g::lobby::rooms::ROOM_QUEUE_BASE;
    let mut world = World::new();
    let (ticket, _) = a_match_in_play(&mut world);
    let dave = world.connect("Dave");
    let erin = world.connect("Erin");
    let carol = world.connect("Carol");
    let key = create(&mut world, dave, spec(1), None);
    rows(&mut world, erin, ROOM_JOIN_BASE + key, 1);
    rows(&mut world, carol, ROOM_QUEUE_BASE + key, 1);
    let account = world.ids[carol].0;
    assert_eq!(world.core.rooms.queue_place(account), Some((key, 1)));
    assert!(ask_to_watch(&mut world, carol, ticket).starts_with("R1;watching;"));
    // Its game's Leave match, as the match server passes it on.
    let session = world.ids[carol].1;
    world.store.matchmaking().watcher_left(session, ticket);
    world.advance(Duration::from_secs(1));
    assert_eq!(world.store.matchmaking().watched(session), None);
    assert_eq!(world.core.rooms.queue_place(account), Some((key, 1)));
}

/// Queue and spectate's spectators count toward the Live window's caps:
/// a match they fill is busy for the Live window.
#[test]
fn queue_spectators_fill_a_match_for_the_live_window() {
    use survarium::v026g::lobby::rooms::ROOM_WATCH_BASE;
    let mut settings = Settings::local("127.0.0.1:0");
    settings.live.per_match = 1;
    let mut world = World::with_settings(settings);
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let key = create(&mut world, alice, spec(1), None);
    rows(&mut world, bob, ROOM_JOIN_BASE + key, 1);
    world.advance(ROOM_TO_MATCH);
    let (ticket, _) = sent_to_match(&world.take(alice)).expect("Alice plays");
    world.host.live.locked().insert(
        ticket,
        LiveView {
            present: 0b11,
            phase: LivePhase::Playing,
            ..LiveView::default()
        },
    );
    world
        .host
        .closed
        .locked()
        .insert(ticket, session::matchmaking_policy::Closed::Joining);
    let carol = world.connect("Carol");
    rows(&mut world, carol, ROOM_WATCH_BASE + key, 1);
    world.advance(Duration::from_secs(1));
    assert!(world.store.matchmaking().watched(world.ids[carol].1).is_some());
    let dave = world.connect("Dave");
    assert_eq!(
        ask_to_watch(&mut world, dave, ticket),
        format!("R1;busy;{ticket:08x};0")
    );
}

/// A match over waits for its spectators before it ends, and its recording
/// is not readable before that: the list says it may not be watched, and an
/// ask is `busy`, never a lookup of its recording (`gone`, a miss counted
/// each time: a dozen would lock the account out for an hour).
#[test]
fn a_match_over_is_listed_not_watchable_and_answered_busy() {
    use session::v026g::replay_watch::{MISSES_ALLOWED, WatchSettings};
    use session::v026g::replays::Replays;
    let desk: &'static Replays =
        Box::leak(Box::new(Replays::new(None).with_watch(WatchSettings::default())));
    let mut world = World::new();
    world.core.replays = Some(desk);
    let (ticket, _) = a_match_in_play(&mut world);
    world.host.live.locked().get_mut(&ticket).unwrap().phase = LivePhase::Over;
    let carol = world.connect("Carol");
    let (account, session) = world.ids[carol];
    let row = rows(&mut world, carol, LIVE_START, 20)[1].clone();
    let fields: Vec<&str> = row.split(';').collect();
    assert_eq!((fields[4], fields[11]), ("3", "2"), "{row}");
    for _ in 0..=MISSES_ALLOWED {
        assert_eq!(
            ask_to_watch(&mut world, carol, ticket),
            format!("R1;busy;{ticket:08x};0")
        );
    }
    assert!(!desk.watches().holds(account), "no replay session");
    assert_eq!(desk.watches().next_job(world.now), None, "nothing to read");
    assert!(!desk.watches().limited(account, world.now), "no miss");
    assert_eq!(world.store.matchmaking().watched(session), None);
}

/// A Live spectator is no player of the match (`queue-spectate.md`: "never
/// as players"): its lines on the match's channel reach none of the
/// players, and theirs never reach it (it would read them live, ahead of
/// its delay), nor does the idle removal's line on that channel.
#[test]
fn a_live_watcher_is_out_of_the_matchs_chat_both_ways() {
    let mut world = World::new();
    let (ticket, [alice, bob]) = a_match_in_play(&mut world);
    let carol = world.connect("Carol");
    assert!(ask_to_watch(&mut world, carol, ticket).starts_with("R1;watching;"));
    // Its game was sent to the players' match: the lobby has it there.
    let in_match = |world: &World, index: usize| {
        world
            .core
            .player(world.clients[index].0)
            .and_then(|player| player.in_match)
    };
    assert!(in_match(&world, carol).is_some());
    assert_eq!(in_match(&world, carol), in_match(&world, alice));
    world.take(carol);
    world.take(alice);
    world.take(bob);
    let channels = |decoded: &[Decoded]| {
        decoded
            .iter()
            .filter_map(|message| match message {
                Decoded::ChatText { channel, .. } => Some(*channel),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    world.ask(carol, request::chat(ticket, "", channel::MATCH, "hi"));
    assert!(channels(&world.take(alice)).is_empty());
    assert!(channels(&world.take(bob)).is_empty());
    world.ask(alice, request::chat(ticket, "", channel::MATCH, "gl"));
    assert_eq!(channels(&world.take(bob)), [channel::MATCH]);
    assert!(channels(&world.take(carol)).is_empty());
    world.inbox.idle_removed(ticket, world.ids[bob].0, "Bob");
    world.settle();
    assert_eq!(channels(&world.take(alice)), [channel::MATCH]);
    assert!(channels(&world.take(carol)).is_empty());
}
