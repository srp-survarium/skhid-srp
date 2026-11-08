//! Watching a replay in the running game: the ask (`3D` with a replay
//! session's player) and the polls (`3B` with `REPLAY_START`), answered
//! with the asker's own `R1` row.

use session::v026g::replays::{Replays, Status, oldest_replayable_here};
use survarium::v026g::lobby::REPLAY_START;

use super::*;

const MATCH: u32 = 0x1F07_C8FB;

/// A game of today's revision: the oldest whose recordings are given, so
/// a recording of its games plays whatever the last simulation change was.
fn current() -> u8 {
    oldest_replayable_here(0)
}

/// A player whose game signed in as `revision`.
fn player(world: &mut World, name: &str, revision: u8) -> usize {
    let (account, _) = signed_in(world.store, name);
    let session = world.store.create_session_with_revision(account, revision);
    world.connect_session((account, session))
}

/// A poll without a session gets `none` (no match); with replays off,
/// `off`. The download's ask of older movies (`3D 00 FFFFFF01 <match>`)
/// is no replay ask: nothing answers it.
#[test]
fn a_poll_without_a_session_answers_none_and_off_when_off() {
    let mut world = World::new();
    let alice = player(&mut world, "Alice", current());
    world.take(alice);
    world.ask(alice, request::player_details(0xFFFF_FF01, MATCH));
    assert!(world.take(alice).is_empty());
    assert_eq!(poll_in_room(&mut world, alice), "R1;off;00000000;0");
    world.core.replays = Some(sessions());
    assert_eq!(poll_in_room(&mut world, alice), "R1;none;00000000;0");
    world.assert_clean();
}

/// `4F` of a game in `language` (the general room in slot 1, the
/// language byte last, as 0x26C320 sends it).
fn subscribe(language: u8) -> (u8, Vec<u8>) {
    let ids: [u32; 10] = [0, 7, u32::MAX, 0, 1, 0, 0, 0, 0, 0];
    let mut body: Vec<u8> = ids.iter().flat_map(|id| id.to_le_bytes()).collect();
    body.push(language);
    (0x4F, body)
}

/// A poll: the `R1` row, and the message box's text if one came after it.
fn poll_refused(world: &mut World, index: usize) -> (String, Option<String>) {
    world.take(index);
    world.ask(index, request::ranking(REPLAY_START, 1));
    match &world.take(index)[..] {
        [Decoded::EloList(rows)] => (rows[0].0.clone(), None),
        [Decoded::EloList(rows), Decoded::Denied { op, text }] => {
            assert_eq!(*op, 0x49, "the message box and nothing else");
            (rows[0].0.clone(), Some(text.clone()))
        }
        other => panic!("not a refused answer: {other:?}"),
    }
}

#[test]
fn every_refusal_fits_the_message_box_in_every_language() {
    use crate::v026g::replay::refusal_text;
    use session::v026g::replays::Refusal;
    let refusals = [
        Refusal::Unknown,
        // The longest: two three-digit revisions and a tag.
        Refusal::Older {
            from: 199,
            to: 254,
            oldest: 255,
        },
        Refusal::Older {
            from: 13,
            to: 14,
            oldest: 17,
        },
        Refusal::Newer { from: 199, to: 255 },
    ];
    let english: Vec<String> = refusals
        .iter()
        .map(|&refusal| refusal_text(refusal, Some(0)))
        .collect();
    for language in [None, Some(0), Some(1), Some(2), Some(3), Some(4), Some(9)] {
        for (n, &refusal) in refusals.iter().enumerate() {
            let text = refusal_text(refusal, language);
            assert!(
                text.len() <= 255,
                "{language:?} {refusal:?}: {} bytes",
                text.len()
            );
            if matches!(language, Some(1..=4)) {
                assert_ne!(text, english[n], "{language:?} {refusal:?}");
            }
            for part in ["199-254", "026g-rev254", "13-14", "199-255"] {
                assert_eq!(
                    text.contains(part),
                    english[n].contains(part),
                    "{language:?} {part}"
                );
            }
        }
    }
    // 14 has no tag: the release of that revision, unnamed.
    assert!(!refusal_text(refusals[2], None).contains("026g-rev"));
}

/// From two who can play to their room's match: the vote's 15 s, the wait
/// of `rooms_tests::spec` (15 s), and a little more.
const ROOM_TO_MATCH: Duration = Duration::from_secs(32);

/// Replays with sessions on (anyone may watch).
fn sessions() -> &'static Replays {
    use session::v026g::replay_watch::WatchSettings;
    Box::leak(Box::new(
        Replays::new(None).with_watch(WatchSettings::default()),
    ))
}

/// The movie's request to watch `MATCH`'s replay from `seconds`, `place`
/// first: its `R1` row at once.
fn ask_to_watch(world: &mut World, index: usize, seconds: u16, place: Option<u8>) -> String {
    use survarium::v026g::lobby::ReplayWatchAsk;
    world.take(index);
    let ask = ReplayWatchAsk::At { seconds, place };
    world.ask(index, request::player_details(ask.player_id(), MATCH));
    r1(&world.take(index))
}

/// The one `R1` row of what came (a room's pushes may come with it).
fn r1(decoded: &[Decoded]) -> String {
    let rows: Vec<&String> = decoded
        .iter()
        .filter_map(|message| match message {
            Decoded::EloList(rows) => Some(rows),
            _ => None,
        })
        .flatten()
        .filter(|(name, place, _, _)| *place == REPLAY_START && name.starts_with("R1;"))
        .map(|(name, ..)| name)
        .collect();
    let [row] = rows[..] else {
        panic!("not one R1 row: {decoded:?}");
    };
    row.clone()
}

/// The movie's poll in a room: its `R1` row.
fn poll_in_room(world: &mut World, index: usize) -> String {
    world.take(index);
    world.ask(index, request::ranking(REPLAY_START, 1));
    r1(&world.take(index))
}

/// A replay session: `preparing` while the match server reads the
/// recording, then the game is sent to it as a spectator is (to the match
/// server, under the recorded match, no team) and the polls say `watching`
/// and where it starts. Its player is in none of its room's matches until
/// the session is over; nothing of the queue or the matches is touched.
#[test]
fn a_replay_session_sends_the_game_to_watch_and_holds_its_room_until_it_ends() {
    use session::v026g::replay_watch::WatchFrom;
    use super::rooms_tests::{create, rows, spec};
    use survarium::v026g::lobby::rooms::ROOM_JOIN_BASE;
    let desk = sessions();
    let mut world = World::new();
    world.core.replays = Some(desk);
    let alice = player(&mut world, "Alice", current());
    let bob = player(&mut world, "Bob", current());
    let carol = player(&mut world, "Carol", current());
    let key = create(&mut world, alice, spec(6), None);
    for index in [bob, carol] {
        rows(&mut world, index, ROOM_JOIN_BASE + key, 1);
    }
    assert_eq!(
        ask_to_watch(&mut world, carol, 75, Some(3)),
        "R1;preparing;1f07c8fb;2"
    );
    let (account, session) = world.ids[carol];
    let watches = desk.watches();
    assert!(watches.holds(account));
    // The match server reads it (`match-server`'s `game/playback.rs`).
    let request = watches.next_job(world.now).expect("a session to read");
    assert_eq!(
        (request.account_id, request.session_id, request.match_id),
        (account, session, MATCH)
    );
    assert_eq!(
        request.from,
        WatchFrom::At {
            ms: 75_000,
            place: Some(3)
        }
    );
    assert_eq!(poll_in_room(&mut world, carol), "R1;preparing;1f07c8fb;1");
    // The room's match starts without her.
    world.advance(ROOM_TO_MATCH);
    assert!(sent_to_match(&world.take(alice)).is_some(), "Alice plays");
    assert!(sent_to_match(&world.take(bob)).is_some(), "Bob plays");
    assert_eq!(sent_to_match(&world.take(carol)), None);
    // Read: her game is sent.
    assert!(watches.loaded(&request, 74_500, world.now));
    world.advance(Duration::from_secs(1));
    assert_eq!(sent_to_match(&world.take(carol)), Some((MATCH, 0)));
    assert!(watches.in_replay(session));
    assert_eq!(
        watches.connected(session, world.now),
        Some((request, 74_500))
    );
    assert_eq!(
        poll_in_room(&mut world, carol),
        "R1;watching;1f07c8fb;74500"
    );
    // Not put into her room's match late meanwhile, nor told it is over.
    world.advance(Duration::from_secs(20));
    let decoded = world.take(carol);
    assert_eq!(sent_to_match(&decoded), None);
    assert!(
        !decoded
            .iter()
            .any(|message| matches!(message, Decoded::ClientState { .. })),
        "{decoded:?}"
    );
    // Over: free again; the queue never had her.
    watches.ended(&request);
    assert!(!watches.holds(account));
    assert!(world.store.matchmaking().watched(session).is_none());
    assert!(world.host.made.locked().iter().all(|(_, setup)| {
        setup
            .players
            .iter()
            .all(|player| player.account_id != account)
    }));
}

/// Refused by the match server: the poll says why, with the box for
/// another epoch's recording; a player not free in the lobby gets `busy`,
/// and a server without sessions `off`.
#[test]
fn a_refused_replay_session_says_why_and_a_busy_player_gets_none() {
    use session::v026g::replays::Refusal;
    let desk = sessions();
    let mut world = World::new();
    let taras = player(&mut world, "Taras", 17);
    world.ask(taras, subscribe(1));
    assert_eq!(
        ask_to_watch(&mut world, taras, 0, None),
        "R1;off;1f07c8fb;0"
    );
    world.core.replays = Some(desk);
    assert_eq!(
        ask_to_watch(&mut world, taras, 0, None),
        "R1;preparing;1f07c8fb;2"
    );
    let request = desk.watches().next_job(world.now).unwrap();
    let why = Refusal::Older {
        from: 16,
        to: 16,
        oldest: 17,
    };
    desk.watches()
        .refuse(&request, Status::Incompatible, Some(why), world.now);
    let (row, said) = poll_refused(&mut world, taras);
    assert_eq!(row, "R1;incompatible;1f07c8fb;0");
    assert!(
        said.unwrap()
            .starts_with("Этот повтор записан старой версией игры")
    );
    // A refusal holds nothing; a session not over: `busy`.
    assert_eq!(
        ask_to_watch(&mut world, taras, 0, None),
        "R1;preparing;1f07c8fb;2"
    );
    assert_eq!(
        ask_to_watch(&mut world, taras, 0, None),
        "R1;busy;1f07c8fb;0"
    );
}

/// The results screen's "Watch my deaths" (`0xC1000000`): a session from
/// the asker's deaths, which the match server finds.
#[test]
fn watch_my_deaths_asks_for_a_session_from_the_askers_deaths() {
    use session::v026g::replay_watch::WatchFrom;
    use survarium::v026g::lobby::REPLAY_WATCH_MY_DEATHS;
    let desk = sessions();
    let mut world = World::new();
    world.core.replays = Some(desk);
    let taras = player(&mut world, "Taras", current());
    world.take(taras);
    world.ask(taras, request::player_details(REPLAY_WATCH_MY_DEATHS, MATCH));
    assert_eq!(r1(&world.take(taras)), "R1;preparing;1f07c8fb;2");
    let request = desk.watches().next_job(world.now).unwrap();
    assert_eq!(request.from, WatchFrom::MyDeaths);
}
