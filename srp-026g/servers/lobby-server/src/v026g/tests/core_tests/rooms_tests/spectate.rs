//! Queue and spectate (`docs/notes/queue-spectate.md`, the owner's of
//! 2026-11-05): only that button of a full room watches its running match
//! while its player waits; a plain Queue stays in the lobby. The first place
//! that frees takes the spectator out of the match and into it as a player;
//! leaving the match leaves the queue; the room's end of match keeps it
//! queued, to watch the next.

use super::*;
use session::matchmaking_policy::Closed;
use survarium::v026g::lobby::rooms::ROOM_WATCH_BASE;

/// A full room (6 v 6, twelve members) playing its match: its key, the
/// match server's ticket and its members.
fn full_room_playing(world: &mut World) -> (u32, u32, Vec<usize>) {
    let owner = world.connect("Owner");
    let key = create(world, owner, spec(6), None);
    let mut members = vec![owner];
    for n in 1..12 {
        let member = world.connect(&format!("Member{n}"));
        rows(world, member, ROOM_JOIN_BASE + key, 1);
        members.push(member);
    }
    world.advance(VOTE_AND_WAIT);
    let (ticket, _) = sent_to_match(&world.take(owner)).expect("a match");
    for &member in &members {
        world.take(member);
    }
    running(world, ticket);
    (key, ticket, members)
}

/// The match server runs the match: it says why it takes nobody now.
fn running(world: &mut World, ticket: u32) {
    world.host.closed.locked().insert(ticket, Closed::Joining);
}

fn session_of(world: &World, index: usize) -> u32 {
    world.ids[index].1
}

fn watched(world: &World, index: usize) -> Option<u32> {
    world
        .core
        .store
        .matchmaking()
        .watched(session_of(world, index))
        .map(|watcher| watcher.ticket)
}

fn spectators(state: &[String]) -> Vec<&str> {
    state
        .iter()
        .filter_map(|row| row.strip_prefix("V;"))
        .collect()
}

/// More spectators than the rows name: the first of the queue are named,
/// and the room's state still goes.
#[test]
fn a_room_names_its_first_twenty_spectators() {
    let mut settings = two_team_settings();
    settings.live.per_match = 21;
    settings.live.most = 21;
    let mut world = World::with_settings(settings);
    let (key, _, members) = full_room_playing(&mut world);
    for n in 0..21 {
        let watcher = world.connect(&format!("Watcher{n}"));
        rows(&mut world, watcher, ROOM_WATCH_BASE + key, 1);
    }
    world.advance(Duration::from_secs(1));
    let seen = state(&mut world, members[1]);
    assert_eq!(spectators(&seen).len(), 20, "{seen:?}");
    assert_eq!(spectators(&seen)[0], "Watcher0");
}

#[test]
fn queue_and_spectate_watches_the_match_and_plays_when_a_place_frees() {
    let mut world = World::new();
    let (key, ticket, members) = full_room_playing(&mut world);
    let carol = world.connect("Carol");
    assert_eq!(
        rows(&mut world, carol, ROOM_WATCH_BASE + key, 1),
        [format!("S1;N;0;{key};1")]
    );
    // Sent to the match at once, with no place in it.
    world.advance(Duration::from_secs(1));
    assert_eq!(sent_to_match(&world.take(carol)), Some((ticket, 0)));
    assert_eq!(watched(&world, carol), Some(ticket));
    let made = world.host.made.locked().last().unwrap().1.players.len();
    assert_eq!(made, 12, "a spectator is no player of the match");
    // The members see who watches; nobody else is told anything.
    let seen = state(&mut world, members[1]);
    assert_eq!(spectators(&seen), ["Carol"], "{seen:?}");
    let listed = rows(&mut world, carol, ROOM_MEMBERS_BASE + key, 1);
    assert_eq!(spectators(&listed), ["Carol"]);
    // A member leaves the match by its menu (out of the room, its place in
    // the match given up): Carol is the room's, out of the match she
    // watched...
    let id = world
        .core
        .matches
        .iter()
        .find(|(_, running)| running.ticket.match_id == ticket)
        .map(|(id, _)| *id)
        .unwrap();
    let (place, _) = seat_of(&world, id, members[1]);
    let leaver = world.ids[members[1]].0;
    assert_eq!(world.store.matchmaking().leave_match(leaver), Some(id));
    world.host.live.locked().insert(
        ticket,
        LiveView {
            present: ((1 << 12) - 1) & !(1 << place),
            ..LiveView::default()
        },
    );
    world.advance(Duration::from_secs(2));
    assert_eq!(world.core.rooms.key_of(world.ids[carol].0), Some(key));
    assert_eq!(watched(&world, carol), None);
    assert!(spectators(&state(&mut world, members[2])).is_empty());
    // ... and into it as a player as soon as her game, back in the lobby
    // (its word), asks for its room: no 15 s. Asked before her game is
    // back, it waits for it.
    world.host.admitting.locked().insert(ticket);
    world.take(carol);
    state(&mut world, carol);
    world.advance(Duration::from_secs(1));
    assert!(sent_to_match(&world.take(carol)).is_none(), "her game is away");
    world.ask(carol, request::leave(1, true));
    state(&mut world, carol);
    world.advance(Duration::from_secs(1));
    let (sent, _) = sent_to_match(&world.take(carol)).expect("Carol plays");
    assert_eq!(sent, ticket);
    let made = world.host.made.locked().last().unwrap().1.players.len();
    assert_eq!(made, 13);
}

/// The owner's rule of 2026-11-09: Queue and spectate is live watching like
/// the Live window's: its delay behind.
#[test]
fn queue_and_spectate_follows_the_rule_of_live_watching() {
    let mut world = World::new();
    let (key, ticket, _) = full_room_playing(&mut world);
    let carol = world.connect("Carol");
    rows(&mut world, carol, ROOM_WATCH_BASE + key, 1);
    world.advance(Duration::from_secs(1));
    assert_eq!(sent_to_match(&world.take(carol)), Some((ticket, 0)));
    let watcher = world
        .core
        .store
        .matchmaking()
        .watched(session_of(&world, carol))
        .expect("watches");
    assert_eq!(
        watcher.kind,
        session::WatchKind::Queued {
            delay: session::LiveDelay::DEFAULT
        }
    );
    assert_eq!(watched(&world, carol), Some(ticket));
}

#[test]
fn a_plain_queue_stays_in_the_lobby() {
    let mut world = World::new();
    let (key, _, _) = full_room_playing(&mut world);
    let dave = world.connect("Dave");
    assert_eq!(
        rows(&mut world, dave, ROOM_QUEUE_BASE + key, 1),
        [format!("S1;N;0;{key};1")]
    );
    world.advance(Duration::from_secs(2));
    assert!(sent_to_match(&world.take(dave)).is_none());
    assert_eq!(watched(&world, dave), None);
    // Queue and spectate pressed after it: the same place, watching now.
    assert_eq!(
        rows(&mut world, dave, ROOM_WATCH_BASE + key, 1),
        [format!("S1;N;0;{key};1")]
    );
    world.advance(Duration::from_secs(1));
    assert!(sent_to_match(&world.take(dave)).is_some());
    // And Queue again: back to the lobby, still first.
    rows(&mut world, dave, ROOM_QUEUE_BASE + key, 1);
    world.advance(Duration::from_secs(1));
    assert_eq!(watched(&world, dave), None);
    assert_eq!(
        world.core.rooms.queue_place(world.ids[dave].0),
        Some((key, 1))
    );
}

#[test]
fn a_spectator_that_leaves_the_match_leaves_the_queue() {
    let mut world = World::new();
    let (key, ticket, _) = full_room_playing(&mut world);
    let carol = world.connect("Carol");
    rows(&mut world, carol, ROOM_WATCH_BASE + key, 1);
    world.advance(Duration::from_secs(1));
    assert!(sent_to_match(&world.take(carol)).is_some());
    assert_eq!(watched(&world, carol), Some(ticket));
    // Its game's menu (0xE0), as the match server passes it on.
    let session = session_of(&world, carol);
    world.core.store.matchmaking().watcher_left(session, ticket);
    world.advance(Duration::from_secs(1));
    assert_eq!(world.core.rooms.queue_place(world.ids[carol].0), None);
    assert_eq!(state(&mut world, carol), ["S1;N;0;0;0"]);
    // In the lobby again: free to join or queue.
    world.advance(Duration::from_secs(1));
    assert!(sent_to_match(&world.take(carol)).is_none());
}

/// Its game's word to the lobby (`31 .. 1`, back from its match) may come
/// before the match server's: it leaves the queue all the same, and is
/// never sent back to the match.
#[test]
fn a_spectator_back_in_the_lobby_by_its_own_word_leaves_the_queue() {
    let mut world = World::new();
    let (key, ticket, _) = full_room_playing(&mut world);
    let carol = world.connect("Carol");
    rows(&mut world, carol, ROOM_WATCH_BASE + key, 1);
    world.advance(Duration::from_secs(1));
    let sent = world.take(carol);
    assert!(sent_to_match(&sent).is_some());
    world.ask(carol, request::leave(order_sent(&sent).unwrap(), true));
    world.advance(Duration::from_secs(2));
    assert!(sent_to_match(&world.take(carol)).is_none());
    assert_eq!(watched(&world, carol), None);
    assert_eq!(world.core.rooms.queue_place(world.ids[carol].0), None);
    // The match server's word, later: nothing more.
    let session = session_of(&world, carol);
    world.core.store.matchmaking().watcher_left(session, ticket);
    world.advance(Duration::from_secs(1));
    assert!(sent_to_match(&world.take(carol)).is_none());
}

#[test]
fn a_spectator_stays_queued_past_the_match_and_watches_the_next() {
    let mut world = World::new();
    let (key, ticket, members) = full_room_playing(&mut world);
    let carol = world.connect("Carol");
    rows(&mut world, carol, ROOM_WATCH_BASE + key, 1);
    world.advance(Duration::from_secs(1));
    assert_eq!(watched(&world, carol), Some(ticket));
    world.end_match(ticket, 0);
    world.advance(Duration::from_secs(2));
    assert_eq!(watched(&world, carol), None);
    assert_eq!(
        world.core.rooms.queue_place(world.ids[carol].0),
        Some((key, 1))
    );
    // The room's next match, still full: Carol watches it.
    for &member in &members {
        world.take(member);
    }
    world.take(carol);
    world.advance(NEXT_MATCH);
    let (next, _) = sent_to_match(&world.take(members[0])).expect("the next match");
    assert_ne!(next, ticket);
    // Not before the match server runs it: its game would be turned away.
    world.advance(Duration::from_secs(1));
    assert_eq!(watched(&world, carol), None);
    assert!(sent_to_match(&world.take(carol)).is_none());
    running(&mut world, next);
    world.advance(Duration::from_secs(1));
    assert_eq!(watched(&world, carol), Some(next));
    assert_eq!(sent_to_match(&world.take(carol)), Some((next, 0)));
}

/// The game's word that it left the match before (`31 <order> FF`) can come
/// after its room's next match was given to it to watch: it names that
/// match's order, or the "not queued" its end's `85 00` left, and ends
/// nothing of the next. Its word under the next's order does.
#[test]
fn a_late_word_for_the_match_before_keeps_the_next_watch() {
    let mut world = World::new();
    let (key, ticket, members) = full_room_playing(&mut world);
    let carol = world.connect("Carol");
    rows(&mut world, carol, ROOM_WATCH_BASE + key, 1);
    world.advance(Duration::from_secs(1));
    let first = order_sent(&world.take(carol)).expect("sent to watch");
    world.end_match(ticket, 0);
    world.advance(Duration::from_secs(2));
    world.advance(NEXT_MATCH);
    let (next, _) = sent_to_match(&world.take(members[0])).expect("the next match");
    running(&mut world, next);
    world.advance(Duration::from_secs(1));
    assert_eq!(watched(&world, carol), Some(next));
    let sent = world.take(carol);
    assert_eq!(sent_to_match(&sent), Some((next, 0)));
    let second = order_sent(&sent).expect("sent to watch the next");
    assert_ne!(second, first);
    for late in [first, u32::MAX] {
        world.ask(carol, request::leave(late, true));
        world.advance(Duration::from_secs(1));
        assert_eq!(watched(&world, carol), Some(next));
        assert_eq!(
            world.core.rooms.queue_place(world.ids[carol].0),
            Some((key, 1))
        );
        assert_eq!(queue_state(&world.take(carol)), None, "nothing told");
    }
    world.ask(carol, request::leave(second, true));
    world.advance(Duration::from_secs(1));
    assert_eq!(watched(&world, carol), None);
    assert_eq!(world.core.rooms.queue_place(world.ids[carol].0), None);
}

/// Spectators are capped (the Live window's caps count them too): one
/// queued past them waits in the queue without watching, and watches when
/// a spectator's slot frees.
#[test]
fn queue_and_spectate_past_the_cap_waits_without_watching() {
    let mut world = World::new();
    let (key, ticket, _) = full_room_playing(&mut world);
    let cap = world.core.settings.live.per_match;
    let waiters: Vec<usize> = (0..=cap)
        .map(|n| {
            let waiter = world.connect(&format!("Watcher{n}"));
            rows(&mut world, waiter, ROOM_WATCH_BASE + key, 1);
            waiter
        })
        .collect();
    world.advance(Duration::from_secs(1));
    let watching = |world: &World| {
        waiters
            .iter()
            .filter(|&&waiter| watched(world, waiter) == Some(ticket))
            .count()
    };
    assert_eq!(watching(&world), cap);
    // The newest account waits, still queued.
    let last = waiters[cap];
    assert_eq!(watched(&world, last), None);
    assert!(sent_to_match(&world.take(last)).is_none());
    assert_eq!(
        world.core.rooms.queue_place(world.ids[last].0),
        Some((key, cap + 1))
    );
    // One watching leaves the match, and with it the queue: its slot frees.
    let session = session_of(&world, waiters[0]);
    world.core.store.matchmaking().watcher_left(session, ticket);
    world.advance(Duration::from_secs(1));
    assert_eq!(watching(&world), cap);
    assert_eq!(watched(&world, last), Some(ticket));
}

/// The review of #1537: Queue and spectate's watch is its session's, as a
/// Live watch is. The same game connecting again (its lobby link dropped,
/// or went silent, while it watches) watches on and is not sent to the
/// match again; a game gone for good gives its watch, its place in the
/// queue and its room up with its session, its wait counted from its link
/// going.
#[test]
fn a_queue_spectator_whose_game_connects_again_watches_on() {
    let mut world = World::new();
    let (key, ticket, _) = full_room_playing(&mut world);
    let carol = world.connect("Carol");
    rows(&mut world, carol, ROOM_WATCH_BASE + key, 1);
    world.advance(Duration::from_secs(1));
    assert_eq!(sent_to_match(&world.take(carol)), Some((ticket, 0)));
    let session = session_of(&world, carol);
    // Its game is there: the match server took it as a spectator.
    world
        .core
        .store
        .matchmaking()
        .watcher_connected(session, ticket);
    let account = world.ids[carol].0;
    // Its link dropped on the game's side: it connects again at once.
    world.game_connects_again(carol, 41_000);
    world.advance(Duration::from_secs(2));
    assert_eq!(watched(&world, carol), Some(ticket));
    assert!(sent_to_match(&world.take(carol)).is_none());
    // Its link went silent on the lobby's side first (a long level load).
    let silent = world.core.settings.match_link_timeout + Duration::from_secs(1);
    world.silent_for(carol, silent);
    assert!(
        !world.core.links.contains_key(&world.clients[carol].0),
        "its link went"
    );
    assert_eq!(watched(&world, carol), Some(ticket));
    world.game_connects_again(carol, 41_001);
    world.advance(Duration::from_secs(2));
    assert_eq!(watched(&world, carol), Some(ticket));
    assert!(!world.store.has_ended(session));
    assert!(sent_to_match(&world.take(carol)).is_none());
    assert_eq!(world.core.rooms.queue_place(account), Some((key, 1)));
    // Its game crashed: nothing more comes from it. The match runs on.
    drop(std::mem::replace(
        &mut world.clients[carol].1,
        TestClient::new(),
    ));
    world.advance(silent);
    assert_eq!(watched(&world, carol), Some(ticket), "its session waits");
    world.advance(world.core.settings.session_grace + Duration::from_secs(2));
    assert!(world.store.has_ended(session));
    assert_eq!(watched(&world, carol), None);
    assert_eq!(world.core.rooms.queue_place(account), None);
}

/// A Queue and spectate watcher whose game never reaches the match server:
/// its watch ends after [`session::WATCH_CONNECT_WITHIN`], it stays in its
/// queue, and it watches again only after [`session::WATCH_COOLDOWN`].
#[test]
fn a_queue_spectator_whose_game_never_comes_stays_queued_and_cools_down() {
    let mut world = World::new();
    let (key, ticket, _) = full_room_playing(&mut world);
    let carol = world.connect("Carol");
    rows(&mut world, carol, ROOM_WATCH_BASE + key, 1);
    world.advance(Duration::from_secs(1));
    assert_eq!(sent_to_match(&world.take(carol)), Some((ticket, 0)));
    let account = world.ids[carol].0;
    world.advance(session::WATCH_CONNECT_WITHIN);
    assert_eq!(watched(&world, carol), None);
    assert_eq!(world.core.rooms.queue_place(account), Some((key, 1)));
    world.advance(session::WATCH_COOLDOWN - Duration::from_secs(5));
    assert_eq!(watched(&world, carol), None, "queued, not watching");
    assert_eq!(world.core.rooms.queue_place(account), Some((key, 1)));
    world.advance(Duration::from_secs(10));
    assert_eq!(watched(&world, carol), Some(ticket));
}
