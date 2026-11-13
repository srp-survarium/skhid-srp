//! Watching a replay in the running game: the ask (`3D` with a replay
//! session's player) and the polls (`3B` with `REPLAY_START`), answered
//! with the asker's own `R1` row.

use session::v026g::replays::{Replays, SaveSettings, Sidecar, oldest_replayable_here};
use survarium::v026g::lobby::{REPLAY_START, SAVE_ASK, SAVE_START};

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
/// `off`. A save's ask (`3D 00 FFFFFF01 <match>`) is no replay session's.
#[test]
fn a_poll_without_a_session_answers_none_and_off_when_off() {
    let mut world = World::new();
    let alice = player(&mut world, "Alice", current());
    world.take(alice);
    world.ask(alice, request::player_details(0xFFFF_FF01, MATCH));
    assert_eq!(saved(&mut world, alice), (None, "R2;off;00000000;0".into()));
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

/// A poll: the one `R1` row, and nothing else (no message box: the movie
/// says why, from the localization tables).
fn poll_refused(world: &mut World, index: usize) -> String {
    world.take(index);
    world.ask(index, request::ranking(REPLAY_START, 1));
    match &world.take(index)[..] {
        [Decoded::EloList(rows)] => rows[0].0.clone(),
        other => panic!("not a refused answer: {other:?}"),
    }
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
    let ask = ReplayWatchAsk { seconds, place };
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

/// A replay session asked by a member whose room can play with it: the room's vote begins and
/// calls it back (`room_recalls`); its game was not sent yet, so its session is over at once, and
/// it plays its room's match. A watch asked during the vote is not begun (`busy`).
#[test]
fn a_replay_session_of_a_member_ends_when_its_room_can_play() {
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
    let (account, _) = world.ids[carol];
    let watches = desk.watches();
    assert!(watches.holds(account));
    world.advance(Duration::from_secs(1));
    assert!(!watches.holds(account), "its room's vote began: called back");
    assert_eq!(ask_to_watch(&mut world, carol, 75, Some(3)), "R1;busy;1f07c8fb;0", "not during the vote");
    world.advance(ROOM_TO_MATCH);
    for (index, name) in [(alice, "Alice"), (bob, "Bob"), (carol, "Carol")] {
        assert!(sent_to_match(&world.take(index)).is_some(), "{name} plays");
    }
}

/// A member alone in its room (below the room's minimum) watches a replay to its end: `preparing`
/// while the match server reads the recording, then the game is sent to it as a spectator is (to
/// the match server, under the recorded match, no team) and the polls say `watching` and where it
/// starts; nothing of the queue or the matches is touched.
#[test]
fn a_replay_session_sends_the_game_to_watch_and_frees_its_player_at_its_end() {
    use session::v026g::replay_watch::WatchFrom;
    use super::rooms_tests::{create, spec};
    let desk = sessions();
    let mut world = World::new();
    world.core.replays = Some(desk);
    let carol = player(&mut world, "Carol", current());
    let key = create(&mut world, carol, spec(6), None);
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
        WatchFrom {
            ms: 75_000,
            place: Some(3)
        }
    );
    assert_eq!(poll_in_room(&mut world, carol), "R1;preparing;1f07c8fb;1");
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
    // Alone in her room: nothing calls her back.
    world.advance(Duration::from_secs(20));
    assert!(!watches.is_recalled(&request));
    // A second member: the room could play with her, and calls her back; the match server lets
    // her game go, which ends the session.
    let dave = player(&mut world, "Dave", current());
    super::rooms_tests::rows(&mut world, dave, survarium::v026g::lobby::rooms::ROOM_JOIN_BASE + key, 1);
    world.advance(Duration::from_secs(1));
    assert!(watches.is_recalled(&request), "called back from the replay");
    watches.ended(&request);
    assert!(!watches.holds(account));
    assert!(world.store.matchmaking().watched(session).is_none());
}

/// A game can quit as it connects to its replay, and says so: its session is over at once, so
/// the next ask does not wait for the match server to give up on it.
#[test]
fn a_game_back_by_its_own_word_is_free_to_watch_again_at_once() {
    let desk = sessions();
    let mut world = World::new();
    world.core.replays = Some(desk);
    let carol = player(&mut world, "Carol", current());
    assert_eq!(ask_to_watch(&mut world, carol, 75, Some(3)), "R1;preparing;1f07c8fb;2");
    let (account, session) = world.ids[carol];
    let watches = desk.watches();
    let request = watches.next_job(world.now).expect("a session to read");
    assert!(watches.loaded(&request, 74_500, world.now));
    world.advance(Duration::from_secs(1));
    let sent = world.take(carol);
    assert_eq!(sent_to_match(&sent), Some((MATCH, 0)));
    assert_eq!(watches.connected(session, world.now), Some((request, 74_500)));
    world.ask(carol, request::leave(order_sent(&sent).unwrap(), true));
    assert!(!watches.holds(account));
    assert_eq!(ask_to_watch(&mut world, carol, 75, Some(3)), "R1;preparing;1f07c8fb;2");
}

/// A session over by the match server's word (its game left by its menu), then the next one's game
/// sent: the game's own word for the first (`31 <its order> FF`, or the "not queued" of the
/// lobby's `85 00`) comes late and ends nothing of the next. Its word under the next's order does.
#[test]
fn a_late_word_for_the_session_before_keeps_the_next() {
    let desk = sessions();
    let mut world = World::new();
    world.core.replays = Some(desk);
    let carol = player(&mut world, "Carol", current());
    let (account, session) = world.ids[carol];
    let watches = desk.watches();
    let sent_session = |world: &mut World| {
        assert_eq!(
            ask_to_watch(world, carol, 75, Some(3)),
            "R1;preparing;1f07c8fb;2"
        );
        let request = watches.next_job(world.now).expect("a session to read");
        assert!(watches.loaded(&request, 74_500, world.now));
        world.advance(Duration::from_secs(1));
        let sent = world.take(carol);
        assert_eq!(sent_to_match(&sent), Some((MATCH, 0)));
        (request, order_sent(&sent).expect("sent to watch"))
    };
    let (first, first_order) = sent_session(&mut world);
    assert!(watches.connected(session, world.now).is_some());
    watches.ended(&first);
    world.advance(Duration::from_secs(1));
    let (second, second_order) = sent_session(&mut world);
    assert_ne!(second_order, first_order);
    for late in [first_order, u32::MAX] {
        world.ask(carol, request::leave(late, true));
        assert!(watches.in_replay(session), "still on its way to the next");
        assert!(watches.is_open(&second));
        assert_eq!(queue_state(&world.take(carol)), None, "nothing told");
    }
    world.ask(carol, request::leave(second_order, true));
    assert!(!watches.holds(account));
}

/// Refused by the match server: the poll says why; a player not free in
/// the lobby gets `busy`,
/// and a server without sessions `off`.
#[test]
fn a_refused_replay_session_says_why_and_a_busy_player_gets_none() {
    use session::v026g::replay_watch::WatchRefusal;
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
        .refuse(&request, WatchRefusal::Incompatible(why), world.now);
    assert_eq!(poll_refused(&mut world, taras), "R1;incompatible;1f07c8fb;0");
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

/// Guessing match ids runs out: after [`MISSES_ALLOWED`] unknown ones, every
/// ask is `busy` and reads nothing, until the window ends.
#[test]
fn guessing_match_ids_is_cut_off_after_a_few_misses() {
    use session::v026g::replay_watch::{MISSES_ALLOWED, MISSES_WINDOW, WatchRefusal};
    let desk = sessions();
    let mut world = World::new();
    world.core.replays = Some(desk);
    let taras = player(&mut world, "Taras", current());
    for _ in 0..MISSES_ALLOWED {
        assert_eq!(
            ask_to_watch(&mut world, taras, 0, None),
            "R1;preparing;1f07c8fb;2"
        );
        let request = desk.watches().next_job(world.now).unwrap();
        desk.watches()
            .refuse(&request, WatchRefusal::Gone, world.now);
        assert_eq!(poll_refused(&mut world, taras), "R1;gone;1f07c8fb;0");
    }
    assert_eq!(
        ask_to_watch(&mut world, taras, 0, None),
        "R1;busy;1f07c8fb;0"
    );
    assert_eq!(desk.watches().next_job(world.now), None, "nothing is read");
    world.advance(MISSES_WINDOW);
    assert_eq!(
        ask_to_watch(&mut world, taras, 0, None),
        "R1;preparing;1f07c8fb;2"
    );
}

// ------------------------------------------------------- Save

/// What came back to the asker of a save (its ask, or `3B 00 <SAVE_START>`
/// when `poll`): the `9E` link and the `R2` row.
fn save_answer(world: &mut World, index: usize) -> (Option<String>, String) {
    let decoded = world.take(index);
    let mut link = None;
    let mut row = None;
    for message in decoded {
        match message {
            Decoded::VideoLink { show, url } => {
                assert!(!show, "the link is never shown in the lobby");
                assert!(row.is_none(), "the link comes before the row");
                link = Some(url);
            }
            Decoded::EloList(rows) => {
                let [(name, place, 0, 0)] = &rows[..] else {
                    panic!("not one row: {rows:?}");
                };
                assert_eq!(*place, SAVE_START);
                row = Some(name.clone());
            }
            other => panic!("not a save's answer: {other:?}"),
        }
    }
    (link, row.expect("an R2 row"))
}

fn saved(world: &mut World, index: usize) -> (Option<String>, String) {
    save_answer(world, index)
}

fn save(world: &mut World, index: usize, match_id: u32) -> (Option<String>, String) {
    world.take(index);
    world.ask(index, request::player_details(SAVE_ASK, match_id));
    save_answer(world, index)
}

fn save_poll(world: &mut World, index: usize) -> (Option<String>, String) {
    world.take(index);
    world.ask(index, request::ranking(SAVE_START, 1));
    save_answer(world, index)
}

/// Save on in `root`, oldest given `oldest`, with a made file of `MATCH`
/// that `players` played, recorded by games of `revisions`.
fn save_desk(
    root: &std::path::Path,
    oldest: u8,
    players: &[u32],
    revisions: Vec<Option<u8>>,
) -> &'static Replays {
    let record_dir = root.join("recordings");
    std::fs::create_dir_all(&record_dir).unwrap();
    let save = SaveSettings {
        replay_dir: root.join("replays"),
        max_replay_bytes: 1 << 20,
    }
    .checked(&record_dir)
    .unwrap();
    let replays: &'static Replays = Box::leak(Box::new(
        Replays::new(Some(record_dir))
            .with_save(save)
            .with_oldest_revision(oldest),
    ));
    std::fs::write(replays.file_path(MATCH).unwrap(), b"0123456789ab").unwrap();
    let sidecar = Sidecar {
        match_id: MATCH,
        size: 12,
        sha256: "cd".repeat(32),
        revisions,
        account_ids: players.to_vec(),
        made_at: 0,
    };
    std::fs::write(replays.json_path(MATCH).unwrap(), sidecar.to_json()).unwrap();
    replays
}

fn temp(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "srp-lobby-save-{name}-{}-{:?}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// A made file: its `R2 ready` with the hidden link just before it, at
/// the ask and at each poll; each asker its own token; the others' polls
/// show nobody's answer.
#[test]
fn a_save_gets_the_link_before_a_ready_row() {
    let root = temp("ready");
    let mut world = World::new();
    let alice = player(&mut world, "Alice", current());
    let bob = player(&mut world, "Bob", current());
    let carol = player(&mut world, "Carol", 12);
    let dave = player(&mut world, "Dave", current());
    let ids = [world.ids[alice].0, world.ids[carol].0];
    let desk = save_desk(&root, current(), &ids, vec![Some(current()), None]);
    world.core.replays = Some(desk);

    assert_eq!(save_poll(&mut world, alice), (None, "R2;none;00000000;0".into()));
    let (link, row) = save(&mut world, alice, MATCH);
    assert_eq!(row, "R2;ready;1f07c8fb;12");
    let link = link.expect("a 9E before the ready row");
    let parts: Vec<&str> = link.split(' ').collect();
    assert_eq!(
        (parts.len(), parts[0], parts[1], parts[3], parts[4]),
        (5, "replay", "1f07c8fb", "cd".repeat(32).as_str(), "12")
    );
    let token = parts[2].to_owned();
    // The token downloads that match, and only from the web page.
    assert_eq!(
        desk.take_download(&token, false, world.now)
            .map(|download| download.match_id),
        Ok(MATCH)
    );
    // A poll: the same link again.
    assert_eq!(save_poll(&mut world, alice).0, Some(link));
    // Bob did not play it: anyone may (the default), his own token.
    let (bobs, row) = save(&mut world, bob, MATCH);
    assert_eq!(row, "R2;ready;1f07c8fb;12");
    assert_ne!(bobs.unwrap().split(' ').nth(2), Some(token.as_str()));
    // Carol's game (revision 12) has no replay link.
    assert_eq!(
        save(&mut world, carol, MATCH),
        (None, "R2;old_game;1f07c8fb;0".into())
    );
    assert_eq!(save_poll(&mut world, dave), (None, "R2;none;00000000;0".into()));
    // Another match, of no recording: gone; asked again at once: busy.
    world.advance(Duration::from_secs(3));
    assert_eq!(
        save(&mut world, alice, 0x1234),
        (None, "R2;gone;00001234;0".into())
    );
    assert_eq!(
        save(&mut world, alice, MATCH),
        (None, "R2;busy;1f07c8fb;0".into())
    );
    // A replay session's polls are their own.
    assert_eq!(poll_in_room(&mut world, alice), "R1;off;00000000;0");
    world.assert_clean();
    let _ = std::fs::remove_dir_all(&root);
}

/// A file of games before the last simulation change: `incompatible`, the
/// movie says why (no message box).
#[test]
fn a_save_of_games_before_the_last_simulation_change_is_refused_and_said_why() {
    let root = temp("older");
    let mut world = World::new();
    let taras = player(&mut world, "Taras", 17);
    world.core.replays = Some(save_desk(&root, 17, &[], vec![Some(16), None]));
    world.ask(taras, subscribe(2));
    let (link, row) = save(&mut world, taras, MATCH);
    assert_eq!((link, row.as_str()), (None, "R2;incompatible;1f07c8fb;0"));
    assert_eq!(
        save_poll(&mut world, taras),
        (None, "R2;incompatible;1f07c8fb;0".into())
    );
    world.assert_clean();
    let _ = std::fs::remove_dir_all(&root);
}

/// A kept recording with no file yet: `preparing` with its place in the
/// line, from the ask on.
#[test]
fn a_preparing_save_carries_its_place_in_the_line() {
    let root = temp("line");
    let mut world = World::new();
    let alice = player(&mut world, "Alice", current());
    let bob = player(&mut world, "Bob", current());
    let desk = save_desk(&root, current(), &[], vec![Some(current())]);
    world.core.replays = Some(desk);
    for id in [0x0A, 0x0B] {
        std::fs::write(
            desk.record_dir()
                .unwrap()
                .join(format!("rec026g_20261015-120000_{id:08x}.srprec")),
            b"x",
        )
        .unwrap();
        desk.recording_kept(id);
    }
    assert_eq!(save(&mut world, alice, 0x0A).1, "R2;preparing;0000000a;1");
    assert_eq!(save(&mut world, bob, 0x0B).1, "R2;preparing;0000000b;2");
    assert_eq!(desk.next_job(), Some(0x0A));
    assert_eq!(save_poll(&mut world, bob).1, "R2;preparing;0000000b;2");
    world.assert_clean();
    let _ = std::fs::remove_dir_all(&root);
}
