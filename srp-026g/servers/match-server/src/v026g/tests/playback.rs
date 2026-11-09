//! Replay sessions (`game/playback.rs`): a player's game shown a match's
//! recording as a spectator, started by the lobby's desk
//! (`session::v026g::replay_watch`).
//!
//! Most tests play a real recording: a two-place Team Deathmatch on level_01
//! of 2026-11-07, 114.6 s, made into a `.sur` by `srp-recording-to-sur`
//! (with an empty mode config), put in a session as if read for it. Its key
//! frames: 50, 14 500, 24 500, 39 550, 49 550, 59 550, 69 550, 81 500,
//! 94 550, 104 600 and 114 600 ms. Its worlds, inputs and roster changes are
//! played as they are; its 0xE4 is the tests' (its profiles hold items of
//! the live game data, which the tests' dictionary does not have). The
//! others read a recording this server makes of a live match, from the
//! recordings' folder, as the real flow does.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicU32;

use session::v026g::persist::match_events::{self, MatchEvent, Player, Timing};
use session::v026g::replay_watch::{Access, Stage, WatchFrom, WatchRequest, WatchSettings};
use session::v026g::replays::{Refusal, ReplayId, Replays, Status};
use survarium::v026g::maps::MatchMap;
use survarium::v026g::match_msgs::static_info::NO_LOCAL_PLACE;
use survarium::v026g::match_msgs::static_info::{MatchOptions, SCORE_WEIGHTS, StaticInfo};
use survarium::v026g::match_msgs::{PlayerInput, ServerMessage, TICK_MS, WireTime, id};

use super::*;
use crate::v026g::game::{ReplayRecording, dev_control};
use crate::v026g::record::{self, Desk, RecordSettings};
use crate::v026g::sur;
use replay_control::{MatchMs, Op, Request, ServerNote, Speed};

const RECORDING: &str = "tests/data/replay_level_01_tdm_2p.sur";
/// Its places enter at 400 and 1 300 ms: the first key frame after.
const FIRST_VIEW: u32 = 14_500;
/// Its countdown ends at 20.4 s (its rules "in process" from then): where
/// the replay opens, its bar's start.
const OPENING: u32 = 20_400;
/// Where a game opening it is put: from the key frame at 14.5 s, as far on as
/// a player's inputs since let it.
const OPENED_AT: u32 = 19_500;
const WATCHER_ACCOUNT: u32 = 900;
const WATCHER: u32 = session_for(WATCHER_ACCOUNT);
const MODERATOR: u32 = 901;
/// Files of the tests: each its own name.
static FILES: AtomicU32 = AtomicU32::new(0);

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "srp-playback-{name}-{}-{}",
        std::process::id(),
        FILES.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The recording with the tests' 0xE4 (two places of level_01's Team
/// Deathmatch).
fn playable() -> ReplayRecording {
    let options = MatchOptions {
        players: vec![profile("a", 0), profile("b", 1)],
        map: MatchMap::of(MAP_TDM).unwrap(),
        match_id: 0x1234,
        match_level: 3,
        score_weights: SCORE_WEIGHTS,
    };
    let info = StaticInfo::new(&options, Dictionary::shared()).unwrap();
    let options = info.payload(0).unwrap()[1..].to_vec();
    let file = std::fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(RECORDING)).unwrap();
    let mut out = Vec::new();
    for (id, bytes) in sur::chunks(&file).unwrap() {
        let bytes = if id == sur::chunk::OPTIONS {
            &options
        } else {
            bytes
        };
        out.extend(id.to_le_bytes());
        out.extend((bytes.len() as u32).to_le_bytes());
        out.extend(bytes);
    }
    ReplayRecording::from_sur(&out, 0x1234).unwrap()
}

/// The recording's inputs of `player`, in order: time and input.
fn recorded(player: u8) -> Vec<(u32, PlayerInput)> {
    let file = std::fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(RECORDING)).unwrap();
    let contents = sur::read(&file).unwrap();
    contents
        .records
        .iter()
        .flat_map(|record| {
            let time = record.event.time;
            record
                .event
                .inputs
                .iter()
                .filter(move |(p, _)| *p == player)
                .map(move |(_, value)| {
                    let input = PlayerInput {
                        time: if time % TICK_MS == 0 {
                            WireTime::tick(time)
                        } else {
                            WireTime::precise(time)
                        },
                        rotation: value.rotation,
                        actions: value.actions,
                    };
                    (time, input)
                })
        })
        .collect()
}

/// As `Client::relays_of` reads them: each distinct input once, the first.
fn distinct(inputs: impl IntoIterator<Item = PlayerInput>) -> Vec<PlayerInput> {
    let mut out = Vec::new();
    for input in inputs {
        if !out.contains(&input) {
            out.push(input);
        }
    }
    out
}

/// The process's replays, with sessions on: the match's players and
/// [`MODERATOR`], or anyone.
fn replays(access: Access) -> Arc<Replays> {
    Arc::new(Replays::new(None).with_watch(WatchSettings {
        access,
        moderators: HashSet::from([MODERATOR]),
        max_sessions: 4,
        ..WatchSettings::default()
    }))
}

/// Recording into `dir`, with `replays`' sessions.
fn settings(dir: &Path, replays: &Arc<Replays>) -> Settings {
    Settings {
        record: Some(RecordSettings {
            dir: dir.to_path_buf(),
            min_players: 2,
            max_bytes: 1 << 24,
            replays: Some(Desk(Arc::clone(replays))),
        }),
        ..Settings::default()
    }
}

/// The game revision of the sessions, and of the recordings' games: today's.
fn revision() -> u8 {
    session::v026g::replays::oldest_replayable_here(0)
}

/// The session of an account's game.
const fn session_for(account_id: u32) -> u32 {
    0x5_0000 + account_id
}

fn request(account_id: u32, match_id: u32, at_ms: u32, place: Option<u8>) -> WatchRequest {
    WatchRequest {
        account_id,
        session_id: session_for(account_id),
        match_id,
        from: WatchFrom::At { ms: at_ms, place },
        revision: revision(),
    }
}

/// A bench with sessions on and nobody in its own match, and its replays.
fn bench() -> (Bench, Arc<Replays>) {
    let replays = replays(Access::Match);
    let bench = Bench::growing(2, MAP_BATTERIES, settings(&temp("bench"), &replays));
    (bench, replays)
}

/// A session of the tests' recording from `at_ms`, read and sent, as the
/// lobby leaves it.
fn session_of_the_recording(bench: &mut Bench, replays: &Replays, at_ms: u32) -> WatchRequest {
    session_with_events(bench, replays, at_ms, Vec::new())
}

/// The same, with the history's `events` of its match.
fn session_with_events(
    bench: &mut Bench,
    replays: &Replays,
    at_ms: u32,
    events: Vec<MatchEvent>,
) -> WatchRequest {
    let request = request(WATCHER_ACCOUNT, 0x1234, at_ms, None);
    prepared(bench, replays, request, events)
}

/// `request`, read and sent.
fn prepared(
    bench: &mut Bench,
    replays: &Replays,
    request: WatchRequest,
    events: Vec<MatchEvent>,
) -> WatchRequest {
    let now = bench.now();
    assert_eq!(replays.watches().start(request, now), Status::Preparing);
    assert_eq!(replays.watches().next_job(now), Some(request));
    let watch = bench.server.replay_watch.as_mut().unwrap();
    watch.prepare_for_test(request, playable(), &events, now);
    replays.watches().sent(WATCHER_ACCOUNT, now);
    request
}

/// The watcher's game connects and loads, with its first world: its
/// client's index.
fn watching(bench: &mut Bench, session: u32) -> usize {
    let at = bench.clients.len();
    bench.clients.push(Client::idle_growing(session));
    bench
        .addrs
        .push(SocketAddr::from(([127, 0, 0, 1], 45_000 + at as u16)));
    bench.clients[at].connect();
    assert!(bench.until(1_000, |b| b.clients[at].model.options.is_some()));
    assert_eq!(bench.clients[at].model.local_index, None);
    bench.clients[at].ready();
    assert!(
        bench.until(5_000, |b| b.clients[at].model.is_synced()),
        "{:?}",
        bench.clients[at].faults
    );
    at
}

fn position(bench: &Bench) -> u32 {
    bench
        .server
        .replay_watch
        .as_ref()
        .and_then(|watch| watch.position(WATCHER, bench.now()))
        .expect("a playback")
}

/// The game's control, numbered `seq`, as it sends it (0xE0).
fn control(bench: &mut Bench, at: usize, seq: u8, op: Op) {
    bench.clients[at].send(id::LEAVE, &Request { seq, op }.encode(), true);
    bench.run(100);
}

/// The playback notes its game took: (revision, paused, speed).
fn notes(client: &Client) -> Vec<(u8, bool, Speed)> {
    client
        .replay_notes
        .iter()
        .filter_map(|payload| match ServerNote::parse(payload)? {
            ServerNote::Playback(note) => Some((note.revision, note.paused, note.speed)),
            ServerNote::Events(_) | ServerNote::View(_) | ServerNote::Kits(_) => None,
        })
        .collect()
}

fn relays_taken(client: &Client) -> usize {
    client
        .got(|message| matches!(message, ServerMessage::InputRelay { .. }).then_some(()))
        .len()
}

/// A replay opens where the action is, at the end of the countdown (as a
/// seek puts a game there), not at its first world with a player in it; a
/// place asked for that entered later, after its entry; a later time asked
/// for, from the key frame at or before it. With no countdown's end, at the
/// first view.
#[test]
fn a_session_starts_at_the_countdowns_end_or_later_when_asked() {
    let recording = playable();
    assert_eq!(recording.opening(), OPENING);
    assert_eq!(recording.start_at(0, None), OPENED_AT);
    // A place asked for that entered before: the same; one that never
    // entered: anyone's.
    assert_eq!(recording.start_at(0, Some(0)), OPENED_AT);
    assert_eq!(recording.start_at(0, Some(7)), OPENED_AT);
    // Before the opening: the opening. Later: as a seek puts it (to 65 s, a
    // player's inputs since the key frame stop it short).
    assert_eq!(recording.start_at(5_000, None), OPENED_AT);
    assert_eq!(recording.start_at(65_000, None), 61_699);
    // A place that enters after the countdown: after its entry, asked for.
    let mut later = playable();
    later.move_entry(0, 30_000);
    assert_eq!(later.start_at(0, None), OPENED_AT);
    assert_eq!(later.start_at(0, Some(1)), OPENED_AT);
    assert_eq!(later.start_at(0, Some(0)), 39_550);
    assert_eq!(later.start_at(40_000, Some(0)), 40_000);
    // A recording with no countdown's end (no rules "in process" in it):
    // its first view.
    let mut no_countdown = playable();
    no_countdown.forget_countdown();
    assert_eq!(no_countdown.opening(), FIRST_VIEW);
    assert_eq!(no_countdown.start_at(0, None), FIRST_VIEW);

    let (mut bench, replays) = bench();
    session_of_the_recording(&mut bench, &replays, 0);
    let at = watching(&mut bench, WATCHER);
    assert_eq!(bench.clients[at].snapshots(), [(0, FIRST_VIEW)]);
    // Both places are in its first world.
    assert!(bench.clients[at].entered().is_empty());
    assert_eq!(
        replays.watches().stage(WATCHER_ACCOUNT),
        Some(Stage::Playing {
            start_ms: OPENED_AT
        })
    );
    bench.assert_no_faults();
}

#[test]
fn a_watcher_gets_the_world_then_the_relays_in_recorded_order() {
    let (mut bench, replays) = bench();
    session_of_the_recording(&mut bench, &replays, 0);
    let at = watching(&mut bench, WATCHER);
    // Player 0's first input is at 51.85 s.
    bench.run_long(45_000);
    let now = position(&bench);
    assert!((63_500..=65_500).contains(&now), "{now}");
    let spectator = &bench.clients[at];
    let first = |pick: fn(&ServerMessage) -> bool| spectator.taken.iter().position(pick);
    assert!(
        first(|m| matches!(m, ServerMessage::Snapshot { .. }))
            < first(|m| matches!(m, ServerMessage::InputRelay { .. }))
    );
    for player in 0..2 {
        let all = recorded(player);
        let due = distinct(
            all.iter()
                .filter(|(t, _)| (FIRST_VIEW + 1..=now).contains(t))
                .map(|(_, i)| *i),
        );
        let got = spectator.relays_of(player);
        assert!(!due.is_empty());
        assert!(
            due.iter().all(|input| got.contains(input)),
            "player {player}"
        );
    }
    assert!(spectator.entered().is_empty());
    // Its world was never started again.
    assert_eq!(spectator.snapshots().len(), 1);
    bench.assert_no_faults();
}

/// The game's controls: pause holds the flow (the viewer's game stops its
/// clock too), play goes on from where it stopped with no new world; a
/// request sent again or late (not newer) is not taken.
#[test]
fn the_games_pause_and_play_and_a_resent_request_is_taken_once() {
    let (mut bench, replays) = bench();
    session_of_the_recording(&mut bench, &replays, 0);
    let at = watching(&mut bench, WATCHER);
    bench.run_long(10_000);
    control(&mut bench, at, 1, Op::Pause(true));
    let paused = position(&bench);
    bench.run(500);
    let taken = relays_taken(&bench.clients[at]);
    bench.run_long(5_000);
    assert_eq!(position(&bench), paused);
    assert_eq!(relays_taken(&bench.clients[at]), taken);
    // Pinged each second meanwhile: never 5 s of silence.
    let pings = bench.clients[at].got(|m| matches!(m, ServerMessage::Ping { .. }).then_some(()));
    assert!((4..=6).contains(&pings.len()), "{}", pings.len());
    let worlds = bench.clients[at].snapshots().len();
    control(&mut bench, at, 2, Op::Pause(false));
    bench.run_long(3_000);
    assert!(relays_taken(&bench.clients[at]) > taken);
    assert_eq!(bench.clients[at].snapshots().len(), worlds);
    let played = position(&bench) - paused;
    assert!((2_900..=3_200).contains(&played), "{played}");
    // Request 2 again (a resend) and 1 (late): not taken.
    for seq in [2, 1] {
        control(&mut bench, at, seq, Op::Seek(MatchMs(94_550)));
    }
    bench.run_long(2_000);
    assert_eq!(bench.clients[at].snapshots().len(), worlds);
    bench.assert_no_faults();
}

/// Its game is told the playback once live, after each change and after
/// each start again, each note newer than the last; the events (none kept
/// yet) once live.
#[test]
fn the_game_is_told_the_playback_after_its_world_each_change_and_each_seek() {
    let (mut bench, replays) = bench();
    session_of_the_recording(&mut bench, &replays, 0);
    let at = watching(&mut bench, WATCHER);
    bench.run(200);
    assert_eq!(notes(&bench.clients[at]), [(1, false, Speed::Normal)]);
    let first = ServerNote::parse(&bench.clients[at].replay_notes[0]);
    let Some(ServerNote::Playback(note)) = first else {
        panic!("{first:?}");
    };
    // The recording's range on the match clock, from the end of its
    // countdown.
    assert_eq!((note.start, note.end), (MatchMs(OPENING), MatchMs(114_600)));
    let events = ServerNote::parse(&bench.clients[at].replay_notes[1]);
    assert!(
        matches!(&events, Some(ServerNote::Events(note)) if note.events().count() == 0),
        "{events:?}"
    );
    control(&mut bench, at, 1, Op::Pause(true));
    control(&mut bench, at, 2, Op::Speed(Speed::Octuple));
    let worlds = bench.clients[at].snapshots().len();
    control(&mut bench, at, 3, Op::Seek(MatchMs(60_000)));
    assert!(bench.until(2_000, |b| b.clients[at].snapshots().len() > worlds));
    bench.run(200);
    assert_eq!(
        notes(&bench.clients[at]),
        [
            (1, false, Speed::Normal),
            (2, true, Speed::Normal),
            (3, true, Speed::Octuple),
            (4, true, Speed::Octuple),
        ]
    );
    bench.assert_no_faults();
}

/// A game whose clock did not stop or scale as told reports worlds far from
/// the playback: started again from where the playback is.
#[test]
fn a_game_off_the_playback_is_started_again() {
    let (mut bench, replays) = bench();
    session_of_the_recording(&mut bench, &replays, 0);
    let at = watching(&mut bench, WATCHER);
    bench.run_long(5_000);
    let worlds = bench.clients[at].snapshots().len();
    let off = position(&bench) + 4_000;
    bench.clients[at].report(off, 1);
    bench.clients[at].report(off + 50, 1);
    bench.run(100);
    assert_eq!(
        bench.clients[at].snapshots().len(),
        worlds,
        "two are not enough"
    );
    bench.clients[at].report(off + 100, 1);
    assert!(bench.until(2_000, |b| b.clients[at].snapshots().len() > worlds));
    // Its own reports, near the playback, are fine.
    let worlds = bench.clients[at].snapshots().len();
    bench.run_long(2_000);
    for step in 0..5 {
        let near = position(&bench) - 500 + step * 50;
        bench.clients[at].report(near, 1);
    }
    bench.run_long(1_000);
    assert_eq!(bench.clients[at].snapshots().len(), worlds);
    bench.assert_no_faults();
}

#[test]
fn the_games_speed_scales_the_playback_clock_and_the_relays_keep_up() {
    let (mut bench, replays) = bench();
    session_of_the_recording(&mut bench, &replays, 0);
    let at = watching(&mut bench, WATCHER);
    bench.run_long(3_000);
    for (seq, speed, factor) in [
        (1, Speed::Double, 2.0),
        (2, Speed::Octuple, 8.0),
        (3, Speed::Quarter, 0.25),
    ] {
        control(&mut bench, at, seq, Op::Speed(speed));
        let from = position(&bench);
        bench.run_long(4_000);
        let went = f64::from(position(&bench) - from);
        assert!(
            (went / 4_000.0 - factor).abs() < 0.05,
            "{speed:?}: {went} ms"
        );
    }
    let now = position(&bench);
    for player in 0..2 {
        let due = distinct(
            recorded(player)
                .into_iter()
                .filter(|(t, _)| (FIRST_VIEW + 1..=now).contains(t))
                .map(|(_, input)| input),
        );
        let got = bench.clients[at].relays_of(player);
        assert!(
            due.iter().all(|input| got.contains(input)),
            "player {player}"
        );
    }
    bench.assert_no_faults();
}

#[test]
fn the_games_seek_starts_it_again_from_the_key_frame_at_or_before_the_time() {
    let (mut bench, replays) = bench();
    session_of_the_recording(&mut bench, &replays, 0);
    let at = watching(&mut bench, WATCHER);
    bench.run_long(3_000);
    // To 65 s a player's inputs since the key frame stop it short; to 20 s
    // the 5 s a tick byte reaches back.
    for (seq, (target, key_frame, resumed_at)) in [
        (65_000, 59_550, 61_699),
        (20_000, 14_500, 19_500),
        (94_550, 94_550, 94_550),
    ]
    .into_iter()
    .enumerate()
    {
        let worlds = bench.clients[at].snapshots().len();
        let request = Request {
            seq: seq as u8 + 1,
            op: Op::Seek(MatchMs(target)),
        };
        bench.clients[at].send(id::LEAVE, &request.encode(), true);
        assert!(bench.until(2_000, |b| b.clients[at].snapshots().len() > worlds));
        let resumed = position(&bench);
        assert!(
            (resumed_at..resumed_at + 200).contains(&resumed),
            "{resumed}"
        );
        let (_, time) = *bench.clients[at].snapshots().last().unwrap();
        assert_eq!(time, key_frame);
        bench.run_long(3_000);
        let now = position(&bench);
        for player in 0..2 {
            let got = bench.clients[at].relays_of(player);
            let missing: Vec<_> = recorded(player)
                .into_iter()
                .filter(|(t, input)| (key_frame + 1..=now).contains(t) && !got.contains(input))
                .collect();
            assert!(missing.is_empty(), "player {player}: {missing:?}");
        }
    }
    bench.assert_no_faults();
}

/// The end lets the game go (0xEA naming no place), then silence; the
/// session is over once the game is forgotten: its player is free again.
#[test]
fn the_end_of_the_recording_ends_the_session() {
    let (mut bench, replays) = bench();
    session_of_the_recording(&mut bench, &replays, 112_000);
    let at = watching(&mut bench, WATCHER);
    assert!(
        bench.until(5_000, |b| b.clients[at].told_to_leave),
        "{}",
        position(&bench)
    );
    assert!(replays.watches().holds(WATCHER_ACCOUNT));
    bench.run(300);
    let heard = bench.clients[at].datagrams_received;
    bench.run_long(6_000);
    assert_eq!(bench.clients[at].datagrams_received, heard);
    bench.run_long(1_000);
    assert!(!replays.watches().holds(WATCHER_ACCOUNT));
    assert!(
        bench
            .server
            .replay_watch
            .as_ref()
            .unwrap()
            .position(WATCHER, bench.now())
            .is_none()
    );
    // Nothing of the matches: no match, seat or result.
    assert!(bench.server.games.is_empty());
    assert!(bench.registry.seat(WATCHER).is_none());
    assert!(bench.results.try_recv().is_err());
    bench.assert_no_faults();
}

/// The viewer's leave (its Back) lets its game go as the recording's end
/// does: the farewell, silence, then the session is over.
#[test]
fn a_leave_request_lets_the_game_go() {
    let (mut bench, replays) = bench();
    session_of_the_recording(&mut bench, &replays, 0);
    let at = watching(&mut bench, WATCHER);
    bench.run_long(2_000);
    assert!(!bench.clients[at].told_to_leave);
    control(&mut bench, at, 1, Op::Leave);
    assert!(bench.until(1_000, |b| b.clients[at].told_to_leave));
    bench.run_long(8_000);
    assert!(!replays.watches().holds(WATCHER_ACCOUNT));
    bench.assert_no_faults();
}

#[test]
fn dev_controls_read_as_written() {
    assert_eq!(dev_control("pause"), Some(Op::Pause(true)));
    assert_eq!(dev_control(" play "), Some(Op::Pause(false)));
    assert_eq!(dev_control("speed 0.25"), Some(Op::Speed(Speed::Quarter)));
    assert_eq!(dev_control("speed 8"), Some(Op::Speed(Speed::Octuple)));
    assert_eq!(dev_control("seek 65.5"), Some(Op::Seek(MatchMs(65_500))));
    for wrong in ["speed 3", "seek", "seek -1", "pause now", "stop"] {
        assert_eq!(dev_control(wrong), None, "{wrong}");
    }
}

/// A live match of the two players of a bench recorded into `dir`, its
/// games of today's revision: its match id, once its file is closed.
fn recorded_live(dir: &Path) -> (u32, PathBuf) {
    let settings = Settings {
        record: Some(RecordSettings {
            dir: dir.to_path_buf(),
            min_players: 2,
            max_bytes: 1 << 24,
            replays: None,
        }),
        ..Settings::default()
    };
    let mut live = Bench::growing(2, MAP_TDM, settings);
    for place in 0..2 {
        live.server.revisions.insert(session_of(place), revision());
    }
    live.join_all();
    live.run_long(12_000);
    let match_id = live.match_id;
    drop(live);
    let closed = |path: &Path| {
        path.extension().is_some_and(|ext| ext == "srprec")
            && record::read(path).is_ok_and(|records| {
                records.last().is_some_and(|last| {
                    last.kind == record::NOTE && last.bytes.starts_with(b"{\"dropped\"")
                })
            })
    };
    for _ in 0..200 {
        let path = std::fs::read_dir(dir)
            .unwrap()
            .flatten()
            .map(|entry| entry.path())
            .find(|path| closed(path));
        if let Some(path) = path {
            return (match_id, path);
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    panic!("the live match's recording");
}

/// The bench runs until the desk's session of `account` is no longer
/// waiting for its recording (read on its own thread): its stage.
fn read_for(bench: &mut Bench, replays: &Replays, account: u32) -> Stage {
    for _ in 0..1_000 {
        bench.run(50);
        match replays.watches().stage(account) {
            Some(Stage::Queued | Stage::Loading) => {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            Some(stage) => return stage,
            None => panic!("no session"),
        }
    }
    panic!("never read");
}

/// The real flow: the recording read from the recordings' folder, judged
/// for each asker, shared by the sessions of its match. With `match`, a
/// player of the match and a moderator watch it and anyone else is
/// refused; a game of another epoch is refused with the lobby's words;
/// `all` (the default) lets anyone. What the
/// game gets before it loads is what the recorded game got, byte for byte,
/// but its 0xE4 makes no place its own.
#[test]
fn a_recording_is_read_and_judged_for_each_asker_and_played_as_recorded() {
    let dir = temp("live");
    let (match_id, path) = recorded_live(&dir);
    let replays = replays(Access::Match);
    let mut bench = Bench::growing(2, MAP_TDM, settings(&dir, &replays));
    let watches = replays.watches();
    let ask = |bench: &mut Bench, account: u32, revision: u8| {
        let request = WatchRequest {
            revision,
            ..request(account, match_id, 0, None)
        };
        assert_eq!(watches.start(request, bench.now()), Status::Preparing);
        read_for(bench, &replays, account)
    };
    // Not a player.
    assert_eq!(
        ask(&mut bench, 555, revision()),
        Stage::Refused {
            status: Status::NotYours,
            refusal: None
        }
    );
    // A player, with a game older than the recording's.
    assert!(matches!(
        ask(&mut bench, 100, revision() - 1),
        Stage::Refused {
            status: Status::Incompatible,
            refusal: Some(Refusal::Newer { .. })
        }
    ));
    watches.answer(100);
    // A player, then a moderator (the recording shared: read once).
    assert!(matches!(
        ask(&mut bench, 100, revision()),
        Stage::Ready { .. }
    ));
    assert!(matches!(
        ask(&mut bench, MODERATOR, revision()),
        Stage::Ready { .. }
    ));
    // Anyone, with `all`.
    let open = self::replays(Access::All);
    let mut anyone = Bench::growing(2, MAP_TDM, settings(&dir, &open));
    let request = request(555, match_id, 0, Some(1));
    open.watches().start(request, anyone.now());
    let Stage::Ready { start_ms } = read_for(&mut anyone, &open, 555) else {
        panic!("anyone may watch");
    };
    // "My deaths" of place 1's player with no deaths kept: from where its
    // place is in view.
    let deaths = WatchRequest {
        from: WatchFrom::MyDeaths,
        ..self::request(101, match_id, 0, None)
    };
    open.watches().start(deaths, anyone.now());
    assert_eq!(read_for(&mut anyone, &open, 101), Stage::Ready { start_ms });
    // From 6 s, place 1 in view: what "my deaths" starts at with place 1
    // dead between 10 and 14 s (4 s before the earliest it can have been).
    let at_six = self::request(556, match_id, 6_000, Some(1));
    open.watches().start(at_six, anyone.now());
    let Stage::Ready { start_ms } = read_for(&mut anyone, &open, 556) else {
        panic!("anyone may watch");
    };
    let replay_id = path
        .file_stem()
        .and_then(|stem| ReplayId::parse(stem.to_str()?))
        .expect("a recording named by its replay id");
    let died = MatchEvent {
        timing: Timing::Between {
            after: 10_000,
            by: 14_000,
        },
        kind: match_events::EventKind::Death {
            victim: Player {
                place: 1,
                account_id: 101,
            },
        },
    };
    let kept = self::replays(Access::All);
    let mut with_history = Bench::growing(
        2,
        MAP_TDM,
        Settings {
            accounts: Some(history(&dir.join("history"), replay_id, vec![died])),
            ..settings(&dir, &kept)
        },
    );
    kept.watches().start(deaths, with_history.now());
    assert_eq!(
        read_for(&mut with_history, &kept, 101),
        Stage::Ready { start_ms }
    );
    assert!(start_ms > 0);
    drop(with_history);
    drop(anyone);

    // Player 100's game is sent and connects.
    watches.sent(100, bench.now());
    let at = watching(&mut bench, session_for(100));
    let recorded = recorded_start(&path, 0);
    assert_eq!(
        recorded.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
        [id::PORTS, id::STATIC_INFO]
    );
    let before_world = bench.clients[at]
        .taken
        .iter()
        .take_while(|message| !matches!(message, ServerMessage::SetTime { .. }))
        .map(|message| (message.id(), message.payload()))
        .collect::<Vec<_>>();
    let mut expected = recorded;
    expected[1].1[0] = NO_LOCAL_PLACE;
    assert_eq!(before_world, expected);
    assert!(!bench.clients[at].snapshots().is_empty());
    std::fs::remove_dir_all(dir).unwrap();
}

/// A match with no recording is `gone`.
#[test]
fn a_match_without_a_recording_is_gone() {
    let (mut bench, replays) = bench();
    let request = request(WATCHER_ACCOUNT, 0xDEAD, 0, None);
    replays.watches().start(request, bench.now());
    assert_eq!(
        read_for(&mut bench, &replays, WATCHER_ACCOUNT),
        Stage::Refused {
            status: Status::Gone,
            refusal: None
        }
    );
}

/// The messages (not the transport's own) the server sent a game before its
/// 0xDA, in a recording: each id and its whole payload (a split message's
/// parts put together).
fn recorded_start(path: &Path, place: u8) -> Vec<(u8, Vec<u8>)> {
    use udp_transport::table::{Direction, Table};
    let mut sent: Vec<(u8, Vec<u8>)> = Vec::new();
    // A split message: its parts so far, and how many it has.
    let mut parts: Vec<u8> = Vec::new();
    let mut count = 0;
    for r in record::read(path).unwrap() {
        if r.place != place || !matches!(r.kind, record::IN | record::OUT) {
            continue;
        }
        let direction = if r.kind == record::IN {
            Direction::ToServer
        } else {
            Direction::ToClient
        };
        let datagram = udp_transport::wire::parse(Table::Match, direction, &r.bytes, 3).unwrap();
        for message in datagram.messages.iter().filter(|m| !m.low_level) {
            if r.kind == record::IN {
                if message.id == survarium::v026g::match_msgs::id::READY {
                    return sent;
                }
                continue;
            }
            parts.extend(message.payload);
            let whole = match message.part {
                None => true,
                Some(part) => {
                    count = part.count.unwrap_or(count);
                    part.index + 1 == count
                }
            };
            if whole {
                sent.push((message.id, std::mem::take(&mut parts)));
            }
        }
    }
    panic!("place {place} never sent 0xDA");
}

/// A kill of the history at `at`.
fn kill(at: u32, killer: u8, victim: Option<u8>) -> MatchEvent {
    let player = |place| Player {
        place,
        account_id: 100 + u32::from(place),
    };
    MatchEvent {
        timing: Timing::Exact(at),
        kind: match_events::EventKind::Kill {
            killer: player(killer),
            victim: victim.map(player),
        },
    }
}

/// The events notes its game took: (first, total, events).
fn events_noted(client: &Client) -> Vec<(u16, u16, Vec<replay_control::Event>)> {
    client
        .replay_notes
        .iter()
        .filter_map(|payload| match ServerNote::parse(payload)? {
            ServerNote::Events(note) => Some((
                note.first,
                note.total,
                note.events().filter_map(|(_, event)| event).collect(),
            )),
            ServerNote::Playback(_) | ServerNote::View(_) | ServerNote::Kits(_) => None,
        })
        .collect()
}

/// The history's events of the match go to its game once live, 64 a note:
/// kills, deaths (no actor) and batteries; scores and the like do not.
#[test]
fn the_game_is_told_the_matchs_events_in_notes_of_64() {
    let (mut bench, replays) = bench();
    let mut events: Vec<MatchEvent> = (0..69)
        .map(|n| kill(15_000 + n * 1_000, (n % 2) as u8, Some(((n + 1) % 2) as u8)))
        .collect();
    events.push(MatchEvent {
        timing: Timing::Between {
            after: 90_000,
            by: 95_000,
        },
        kind: match_events::EventKind::Death {
            victim: Player {
                place: 1,
                account_id: 101,
            },
        },
    });
    events.push(MatchEvent {
        timing: Timing::Exact(114_600),
        kind: match_events::EventKind::End { points: [3, 2] },
    });
    session_with_events(&mut bench, &replays, 0, events);
    let at = watching(&mut bench, WATCHER);
    bench.run(200);
    let noted = events_noted(&bench.clients[at]);
    assert_eq!(
        noted
            .iter()
            .map(|(first, total, events)| (*first, *total, events.len()))
            .collect::<Vec<_>>(),
        [(0, 70, 64), (64, 70, 6)]
    );
    let first = noted[0].2[0];
    assert_eq!(
        (first.time, first.kind, first.actor, first.victim),
        (
            MatchMs(15_000),
            replay_control::EventKind::Kill,
            Some(0),
            Some(1)
        )
    );
    let death = noted[1].2[5];
    assert_eq!(
        (death.time, death.actor, death.victim),
        (MatchMs(95_000), None, Some(1))
    );
    bench.assert_no_faults();
}

/// The srp process's accounts with a match history holding `events` of
/// the match `replay_id`.
fn history(dir: &Path, replay_id: ReplayId, events: Vec<MatchEvent>) -> crate::v026g::Accounts {
    use session::v026g::GameAccounts;
    use session::v026g::persist::history::{
        Eligibility, MatchInfo, MatchKind, MatchRecord, Winner,
    };
    use session::v026g::persist::{GameRepository, SqliteGameRepository};
    let _ = std::fs::remove_dir_all(dir);
    let mut repository = SqliteGameRepository::open_in(dir).unwrap();
    let record = MatchRecord {
        replay_id,
        kind: MatchKind::Public(Eligibility::Counted),
        info: MatchInfo {
            ended_at: 1,
            duration_ms: 12_000,
            map_id: MAP_TDM,
            game_mode: survarium::v026g::maps::GameMode::TeamDeathmatch,
            match_level: 3,
            winner: Winner::Unknown,
            team_points: [0, 0],
        },
        players: Vec::new(),
        events,
    };
    repository.record_match(&record).unwrap();
    let accounts = GameAccounts::with_repository(Box::new(repository)).unwrap();
    crate::v026g::Accounts(Arc::new(accounts))
}

/// A match id comes again after a restart: the session plays the newest
/// recording of it (the one the history's newest match is).
#[test]
fn the_newest_recording_of_a_match_id_is_read() {
    let dir = temp("newest");
    for stamp in ["20261107-235959", "20261108-010203", "20261101-120000"] {
        std::fs::write(dir.join(format!("rec026g_{stamp}_00001234.srprec")), b"x").unwrap();
    }
    std::fs::write(dir.join("rec026g_20261109-000000_00005678.srprec"), b"x").unwrap();
    assert_eq!(
        crate::v026g::replay::recording_in(&dir, 0x1234),
        Some(dir.join("rec026g_20261108-010203_00001234.srprec"))
    );
    std::fs::remove_dir_all(dir).unwrap();
}

/// The place asked to be in view is told the game once it watches (a view
/// note, as a live match's spectator gets it), and not again after a seek;
/// with none asked, none.
#[test]
fn the_place_asked_for_is_told_once_its_game_watches() {
    let (mut bench, replays) = bench();
    prepared(
        &mut bench,
        &replays,
        request(WATCHER_ACCOUNT, 0x1234, 0, Some(1)),
        Vec::new(),
    );
    let at = watching(&mut bench, WATCHER);
    bench.run(200);
    let views = |client: &Client| {
        client
            .replay_notes
            .iter()
            .filter(|payload| matches!(ServerNote::parse(payload), Some(ServerNote::View(_))))
            .cloned()
            .collect::<Vec<_>>()
    };
    assert_eq!(views(&bench.clients[at]), [ServerNote::view(1).to_vec()]);
    let worlds = bench.clients[at].snapshots().len();
    control(&mut bench, at, 1, Op::Seek(MatchMs(60_000)));
    assert!(bench.until(2_000, |b| b.clients[at].snapshots().len() > worlds));
    bench.run(500);
    assert_eq!(views(&bench.clients[at]).len(), 1);

    bench.assert_no_faults();

    let (mut none, replays) = self::bench();
    session_of_the_recording(&mut none, &replays, 0);
    let at = watching(&mut none, WATCHER);
    none.run(200);
    assert!(views(&none.clients[at]).is_empty());
    none.assert_no_faults();
}

/// A seek before the opening (the bar's 0) starts the game there, where its
/// playback's range starts, not at the recording's first world.
#[test]
fn a_seek_to_zero_starts_at_the_opening() {
    let (mut bench, replays) = bench();
    session_of_the_recording(&mut bench, &replays, 60_000);
    let at = watching(&mut bench, WATCHER);
    bench.run_long(2_000);
    control(&mut bench, at, 1, Op::Seek(MatchMs(0)));
    let at_seek = position(&bench);
    assert!((OPENED_AT..OPENED_AT + 300).contains(&at_seek), "{at_seek}");
    bench.assert_no_faults();
}
