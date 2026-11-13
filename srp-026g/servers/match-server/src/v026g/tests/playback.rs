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
use std::sync::Mutex;
use std::sync::atomic::AtomicU32;

use session::v026g::persist::match_events::{self, MatchEvent, Player, Timing};
use session::v026g::replay_watch::{
    Access, Stage, WatchFrom, WatchRefusal, WatchRequest, WatchSettings,
};
use session::v026g::replays::{Refusal, Replays, Status};
use survarium::v026g::maps::MatchMap;
use survarium::v026g::match_msgs::client_model::decode_server;
use survarium::v026g::match_msgs::static_info::NO_LOCAL_PLACE;
use survarium::v026g::match_msgs::static_info::{MatchOptions, SCORE_WEIGHTS, StaticInfo};
use survarium::v026g::match_msgs::{PlayerInput, ServerMessage, TICK_MS, WireTime, id, mask_bytes};

use super::*;
use crate::v026g::game::ReplayRecording;
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
/// Where a seek to 65 s puts a game: from the key frame at 59.55 s, short of
/// the time, as far as a player's inputs since let it with room for those
/// that come due while its world loads.
const PUT_FOR_65_S: u32 = 61_199;
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
    playable_of(2)
}

/// As [`playable`], with an 0xE4 of `places` places (its worlds are still
/// the recording's).
fn playable_of(places: u8) -> ReplayRecording {
    let options = MatchOptions {
        players: (0..places)
            .map(|place| profile(&format!("p{place}"), place % 2))
            .collect(),
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
        from: WatchFrom { ms: at_ms, place },
        revision: Some(revision()),
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
    prepared_of(bench, replays, request, events, playable())
}

/// `request`, read as `recording` and sent.
fn prepared_of(
    bench: &mut Bench,
    replays: &Replays,
    request: WatchRequest,
    events: Vec<MatchEvent>,
    recording: ReplayRecording,
) -> WatchRequest {
    let now = bench.now();
    assert_eq!(replays.watches().start(request, now), Status::Preparing);
    assert_eq!(replays.watches().next_job(now), Some(request));
    let watch = bench.server.replay_watch.as_mut().unwrap();
    watch.prepare_for_test(request, recording, &events, now);
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
            ServerNote::Events(_) | ServerNote::View(_) | ServerNote::Kits(_) | ServerNote::Recall(_) => None,
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
    assert_eq!(recording.start_at(65_000, None), PUT_FOR_65_S);
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
/// yet) once live. One told another speed than 1x is told 1x before it starts
/// again (a world's load is at 1x), and the speed asked once its world is in.
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
            (4, true, Speed::Normal),
            (5, true, Speed::Octuple),
        ]
    );
    bench.assert_no_faults();
}

/// A paused replay does not hold its session for as long as the game stays
/// open: after [`PAUSED_MOST`] it is shown the end, as at the recording's,
/// and the session goes (the sessions are few).
#[test]
fn a_replay_paused_too_long_ends_and_frees_its_session() {
    use crate::v026g::game::playback::PAUSED_MOST;
    let (mut bench, replays) = bench();
    session_of_the_recording(&mut bench, &replays, 0);
    let at = watching(&mut bench, WATCHER);
    bench.run_long(2_000);
    control(&mut bench, at, 1, Op::Pause(true));
    bench.run_long(PAUSED_MOST.as_millis() as u64 - 1_000);
    assert!(!bench.clients[at].told_to_leave, "still within its pause");
    assert!(bench.until(2_000, |b| b.clients[at].told_to_leave));
    assert!(replays.watches().holds(WATCHER_ACCOUNT), "not yet gone");
    // The session is over once the farewell had its time.
    bench.run_long(10_000);
    assert!(!replays.watches().holds(WATCHER_ACCOUNT));
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
        (65_000, 59_550, PUT_FOR_65_S),
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
    // Put before 112 s (its key frame, as far as the inputs since let it
    // be): some 5 s of play from its world, the clock held until it went.
    assert!(
        bench.until(6_000, |b| b.clients[at].told_to_leave),
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
/// The lobby heard the game give up: its playback goes, and the same account can start again.
#[test]
fn a_session_ended_by_the_lobby_drops_its_playback() {
    let (mut bench, replays) = bench();
    session_of_the_recording(&mut bench, &replays, 0);
    let first = watching(&mut bench, WATCHER);
    assert!(
        bench.until(8_000, |b| relays_taken(&b.clients[first]) > 0),
        "it was being played to"
    );
    assert!(replays.watches().game_back(WATCHER));
    assert!(!replays.watches().holds(WATCHER_ACCOUNT));
    bench.run_long(200);
    let taken = relays_taken(&bench.clients[first]);
    bench.run_long(2_000);
    assert_eq!(relays_taken(&bench.clients[first]), taken, "its playback is gone");
    session_of_the_recording(&mut bench, &replays, 0);
    let again = watching(&mut bench, WATCHER);
    assert!(bench.until(5_000, |b| b.clients[again].model.is_synced()));
    assert!(replays.watches().holds(WATCHER_ACCOUNT));
}

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

/// Review of 2026-11-13: the match server ended (srp starts it again in the same process, whose
/// replays outlive it) with a session playing and another read: both are over, or the one playing
/// (no time limit at the desk) would hold its player out of its room's matches until srp
/// restarted.
#[test]
fn a_match_server_that_ends_ends_its_sessions() {
    const READ_ACCOUNT: u32 = WATCHER_ACCOUNT + 2;
    let (mut bench, replays) = bench();
    session_of_the_recording(&mut bench, &replays, 0);
    watching(&mut bench, WATCHER);
    let read = request(READ_ACCOUNT, 0x1234, 0, None);
    let now = bench.now();
    assert_eq!(replays.watches().start(read, now), Status::Preparing);
    assert_eq!(replays.watches().next_job(now), Some(read));
    let watch = bench.server.replay_watch.as_mut().unwrap();
    watch.prepare_for_test(read, playable(), &[], now);
    assert!(matches!(
        replays.watches().stage(WATCHER_ACCOUNT),
        Some(Stage::Playing { .. })
    ));
    assert!(matches!(
        replays.watches().stage(READ_ACCOUNT),
        Some(Stage::Ready { .. })
    ));
    drop(bench);
    assert_eq!(replays.watches().stage(WATCHER_ACCOUNT), None);
    assert_eq!(replays.watches().stage(READ_ACCOUNT), None);
    assert!(!replays.watches().holds(WATCHER_ACCOUNT));
    assert!(!replays.watches().holds(READ_ACCOUNT));
}

/// The recall notes a game took: the seconds each told.
fn recalls(client: &Client) -> Vec<u8> {
    client
        .replay_notes
        .iter()
        .filter_map(|payload| match ServerNote::parse(payload)? {
            ServerNote::Recall(seconds) => Some(seconds),
            _ => None,
        })
        .collect()
}

/// Review of 2026-11-13: a note goes only to a game that watches, so one called back while its
/// world was on its way was let go with no countdown. It is told once its world is in, the
/// seconds left of the [`replay_control::RECALL_SECONDS`] from the call, and let go when they are
/// over, as one told at once.
#[test]
fn a_viewer_called_back_while_its_world_loads_is_told_once_it_watches() {
    let (mut bench, replays) = bench();
    session_of_the_recording(&mut bench, &replays, 0);
    let at = bench.clients.len();
    bench.clients.push(Client::idle_growing(WATCHER));
    bench
        .addrs
        .push(SocketAddr::from(([127, 0, 0, 1], 45_000 + at as u16)));
    bench.clients[at].connect();
    assert!(bench.until(1_000, |b| b.clients[at].model.options.is_some()));
    assert!(replays.watches().recall(WATCHER_ACCOUNT));
    let called = bench.ms;
    bench.run(1_000);
    assert!(
        recalls(&bench.clients[at]).is_empty(),
        "its game does not watch yet"
    );
    bench.clients[at].ready();
    assert!(bench.until(3_000, |b| !recalls(&b.clients[at]).is_empty()));
    let told = recalls(&bench.clients[at]);
    assert_eq!(told.len(), 1);
    assert!(
        (1..replay_control::RECALL_SECONDS).contains(&told[0]),
        "the seconds left: {told:?}"
    );
    assert!(bench.until(5_000, |b| b.clients[at].told_to_leave));
    let let_go = bench.ms - called;
    let recall_ms = u64::from(replay_control::RECALL_SECONDS) * 1_000;
    assert!(
        (recall_ms..recall_ms + 200).contains(&let_go),
        "let go {let_go} ms after the call"
    );
    bench.run(500);
    assert_eq!(recalls(&bench.clients[at]).len(), 1, "told once");
}

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
    // A server gone has had its recordings written and closed
    // (`Recorder::finish`): a match server started again in the same
    // process finds no `.part` of it.
    drop(live);
    let closed = |path: &Path| {
        path.extension().is_some_and(|ext| ext == "srprec")
            && record::read(path).is_ok_and(|records| {
                records.last().is_some_and(|last| {
                    last.kind == record::NOTE && last.bytes.starts_with(b"{\"dropped\"")
                })
            })
    };
    let path = std::fs::read_dir(dir)
        .unwrap()
        .flatten()
        .map(|entry| entry.path())
        .find(|path| closed(path))
        .expect("the live match's recording, closed with its server");
    (match_id, path)
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
            revision: Some(revision),
            ..request(account, match_id, 0, None)
        };
        assert_eq!(watches.start(request, bench.now()), Status::Preparing);
        read_for(bench, &replays, account)
    };
    // Not a player.
    assert_eq!(
        ask(&mut bench, 555, revision()),
        Stage::Refused(WatchRefusal::NotYours)
    );
    // A player, with a game older than the recording's.
    assert!(matches!(
        ask(&mut bench, 100, revision() - 1),
        Stage::Refused(WatchRefusal::Incompatible(Refusal::Newer { .. }))
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
    assert!(
        matches!(read_for(&mut anyone, &open, 555), Stage::Ready { .. }),
        "anyone may watch"
    );
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
        Stage::Refused(WatchRefusal::Gone)
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
            ServerNote::Playback(_) | ServerNote::View(_) | ServerNote::Kits(_) | ServerNote::Recall(_) => None,
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

/// Until its world goes the clock waits where a seek put it (no more
/// of a player's inputs go than its rings keep); seeks asked meanwhile wait
/// for that world to be in or given up, and the latest alone starts it
/// again: one world for them all.
#[test]
fn a_game_without_its_world_holds_the_clock_and_takes_the_latest_seek_once() {
    let (mut bench, replays) = bench();
    session_of_the_recording(&mut bench, &replays, 0);
    let at = watching(&mut bench, WATCHER);
    bench.run_long(3_000);
    let worlds = bench.clients[at].snapshots().len();
    bench.clients[at].mute_time_replies = true;
    control(&mut bench, at, 1, Op::Seek(MatchMs(65_000)));
    assert_eq!(position(&bench), PUT_FOR_65_S);
    let relays = relays_taken(&bench.clients[at]);
    bench.run_long(2_000);
    assert_eq!(position(&bench), PUT_FOR_65_S);
    assert_eq!(relays_taken(&bench.clients[at]), relays);
    for (seq, target) in [(2, 20_000), (3, 65_000), (4, 94_550)] {
        bench.clients[at].send(
            id::LEAVE,
            &Request {
                seq,
                op: Op::Seek(MatchMs(target)),
            }
            .encode(),
            true,
        );
    }
    bench.run(300);
    assert_eq!(position(&bench), PUT_FOR_65_S, "its 0xE5 is still asked");
    // Its 0xE5 given up: the latest seek, the clock waiting there.
    bench.run_long(3_500);
    assert_eq!(position(&bench), 94_550);
    bench.clients[at].mute_time_replies = false;
    assert!(bench.until(3_000, |b| b.clients[at].snapshots().len() > worlds));
    bench.run_long(1_000);
    assert_eq!(bench.clients[at].snapshots().len(), worlds + 1);
    assert_eq!(bench.clients[at].snapshots().last().unwrap().1, 94_550);
    assert!(position(&bench) > 94_550);
    bench.assert_no_faults();
}

/// A seek is no keyboard: a paused replay ends [`PAUSED_MOST`] after its
/// pause, sought or not.
#[test]
fn a_seek_does_not_hold_a_paused_replay_longer() {
    use crate::v026g::game::playback::PAUSED_MOST;
    let (mut bench, replays) = bench();
    session_of_the_recording(&mut bench, &replays, 0);
    let at = watching(&mut bench, WATCHER);
    bench.run_long(2_000);
    control(&mut bench, at, 1, Op::Pause(true));
    bench.run_long(PAUSED_MOST.as_millis() as u64 - 3_000);
    let worlds = bench.clients[at].snapshots().len();
    control(&mut bench, at, 2, Op::Seek(MatchMs(60_000)));
    assert!(bench.until(2_000, |b| b.clients[at].snapshots().len() > worlds));
    assert!(!bench.clients[at].told_to_leave);
    assert!(bench.until(3_000, |b| b.clients[at].told_to_leave));
    bench.assert_no_faults();
}

fn backlog(bench: &Bench) -> Option<usize> {
    bench
        .server
        .replay_watch
        .as_ref()
        .and_then(|watch| watch.backlog(WATCHER))
}

/// Review of 2026-11-13: each pause, play and speed queued a playback note kept until acked: a
/// game that acks nothing and asks for one change a datagram grew its queue by one a datagram
/// for ever, and every poll of the match thread walked it. Only the newest note goes.
#[test]
fn a_viewer_that_acks_nothing_piles_up_no_playback_notes() {
    let (mut bench, replays) = bench();
    session_of_the_recording(&mut bench, &replays, 0);
    let at = watching(&mut bench, WATCHER);
    bench.run(500);
    bench.clients[at].mute_acks = true;
    for seq in 1..=600u32 {
        let op = Op::Pause(seq % 2 == 1);
        bench.clients[at].send(id::LEAVE, &Request { seq: seq as u8, op }.encode(), true);
        bench.run(10);
    }
    let queued = backlog(&bench).expect("its playback");
    assert!(queued < 100, "{queued} messages queued");
}

/// Review of 2026-11-13: whatever a game that acks nothing piles up, it is let go past a cap
/// (some thousands of messages), and its session ends.
#[test]
fn a_viewer_that_acks_nothing_is_let_go() {
    let (mut bench, replays) = bench();
    session_of_the_recording(&mut bench, &replays, 0);
    let at = watching(&mut bench, WATCHER);
    bench.run(500);
    bench.clients[at].mute_acks = true;
    let watch = bench.server.replay_watch.as_mut().unwrap();
    watch.note_for_test(WATCHER, 4_000);
    bench.run(500);
    assert!(replays.watches().holds(WATCHER_ACCOUNT), "within the cap");
    let watch = bench.server.replay_watch.as_mut().unwrap();
    watch.note_for_test(WATCHER, 200);
    bench.run_long(8_000);
    assert!(
        !replays.watches().holds(WATCHER_ACCOUNT),
        "let go and forgotten"
    );
    assert_eq!(backlog(&bench), None);
}

/// Review of 2026-11-13: a game let go swallowed what came from its address until it was
/// forgotten (7 s), a new connection of its game too (a late join into its room's match, another
/// watch): the game drops a link silent for 5 s.
#[test]
fn a_viewer_let_go_that_connects_again_is_heard_at_once() {
    let (mut bench, replays) = bench();
    bench.join_all();
    bench.run_long(25_000);
    session_of_the_recording(&mut bench, &replays, 0);
    let at = watching(&mut bench, WATCHER);
    assert_eq!(at, 2, "the late joiner's place");
    bench.run_long(2_000);
    control(&mut bench, at, 1, Op::Leave);
    assert!(bench.until(1_000, |b| b.clients[at].told_to_leave));
    bench.run(300);
    // Its session joins the match from the same address.
    bench
        .registry
        .join_running(
            bench.match_id,
            MatchPlayer {
                session_id: WATCHER,
                account_id: WATCHER_ACCOUNT,
                profile: profile("watcher", 0),
                bot: false,
                driven: false,
            },
        )
        .unwrap();
    bench.clients[at] = Client::growing(WATCHER, 3, MAP_BATTERIES);
    bench.join(at);
    assert!(!replays.watches().holds(WATCHER_ACCOUNT));
    bench.assert_no_faults();
}

/// Review of 2026-11-13: each start again packs a world on the match thread, and a game that
/// sought, answered its 0xE5, acked the world and sought again started anew as fast as it went
/// round. One second apart at least: a seek asked sooner waits, the latest alone taken.
#[test]
fn a_game_that_seeks_without_end_starts_again_once_a_second() {
    let (mut bench, replays) = bench();
    session_of_the_recording(&mut bench, &replays, 0);
    let at = watching(&mut bench, WATCHER);
    bench.run_long(2_000);
    let worlds = bench.clients[at].snapshots().len();
    // A seek every 50 ms for 3 s, the last to 94.55 s.
    for seq in 1..=60u8 {
        let target = if seq == 60 {
            94_550
        } else {
            20_000 + u32::from(seq % 2) * 45_000
        };
        let request = Request {
            seq,
            op: Op::Seek(MatchMs(target)),
        };
        bench.clients[at].send(id::LEAVE, &request.encode(), true);
        bench.run(50);
    }
    let started = bench.clients[at].snapshots().len() - worlds;
    assert!((2..=4).contains(&started), "{started} starts in 3 s");
    bench.run_long(2_000);
    assert_eq!(
        bench.clients[at].snapshots().last().unwrap().1,
        94_550,
        "the latest seek"
    );
    assert!(bench.clients[at].snapshots().len() - worlds <= started + 1);
    bench.assert_no_faults();
}

/// Review of 2026-11-13: a game off the playback started again after every third report, with
/// no cap: bogus reports right after its world was in started it again and again. It starts
/// again for its reports as rarely as a live match's spectator.
#[test]
fn a_game_off_the_playback_is_started_again_as_rarely_as_a_spectator() {
    let (mut bench, replays) = bench();
    session_of_the_recording(&mut bench, &replays, 0);
    let at = watching(&mut bench, WATCHER);
    bench.run_long(2_000);
    let off_reports = |bench: &mut Bench| {
        let off = position(bench) + 4_000;
        for step in 0..3 {
            bench.clients[at].report(off + step * 50, 1);
        }
    };
    let worlds = bench.clients[at].snapshots().len();
    off_reports(&mut bench);
    assert!(bench.until(2_000, |b| b.clients[at].snapshots().len() > worlds));
    assert!(bench.until(2_000, |b| b.clients[at].model.is_synced()));
    bench.run_long(1_500);
    // Within the resync interval: none more.
    let worlds = bench.clients[at].snapshots().len();
    off_reports(&mut bench);
    bench.run_long(3_000);
    assert_eq!(bench.clients[at].snapshots().len(), worlds);
    bench.assert_no_faults();
}

/// Review of 2026-11-13: a session ended only at the recording's end, so a game that sought
/// back or paused now and then kept one of the few sessions for ever. It lasts the recording's
/// length and a quarter of an hour at most (or twice its length), then is shown the end.
#[test]
fn a_session_lasts_its_longest_then_ends_as_at_the_recordings_end() {
    let most = playable().session_most();
    assert_eq!(
        most,
        Duration::from_millis(114_600) + Duration::from_secs(15 * 60)
    );
    let (mut bench, replays) = bench();
    session_of_the_recording(&mut bench, &replays, 0);
    let connected = bench.now();
    let at = watching(&mut bench, WATCHER);
    // Paused two minutes, then played a second, again and again: never at
    // the recording's end, never paused too long.
    let lasted = |bench: &Bench| bench.now().saturating_duration_since(connected);
    let mut seq = 0u8;
    'played: loop {
        for (paused, seconds) in [(true, 120), (false, 1)] {
            seq = seq.wrapping_add(1);
            control(&mut bench, at, seq, Op::Pause(paused));
            for _ in 0..seconds {
                if lasted(&bench) >= most - Duration::from_secs(2) {
                    break 'played;
                }
                bench.run_long(1_000);
                assert!(!bench.clients[at].told_to_leave, "within its longest");
            }
        }
    }
    assert!(bench.until(30_000, |b| b.clients[at].told_to_leave));
    let lasted = lasted(&bench);
    assert!(
        lasted >= most && lasted < most + Duration::from_secs(1),
        "{lasted:?}"
    );
    bench.run_long(10_000);
    assert!(!replays.watches().holds(WATCHER_ACCOUNT));
    bench.assert_no_faults();
}

/// Whether a datagram to a game carries a message of `id`.
fn carries(datagram: &[u8], id: u8) -> bool {
    udp_transport::wire::parse(
        udp_transport::Table::Match,
        udp_transport::Direction::ToClient,
        datagram,
        1,
    )
    .is_ok_and(|parsed| parsed.messages.iter().any(|message| message.id == id))
}

/// The watcher's game connects (again: from an address of its own) and
/// loads: its client's index. Its world is the server's to send.
fn connect_watcher(bench: &mut Bench) -> usize {
    let at = bench.clients.len();
    bench.clients.push(Client::idle_growing(WATCHER));
    bench
        .addrs
        .push(SocketAddr::from(([127, 0, 0, 1], 45_000 + at as u16)));
    bench.clients[at].connect();
    assert!(bench.until(1_000, |b| b.clients[at].model.options.is_some()));
    bench.clients[at].ready();
    at
}

/// Review of 2026-11-13: the playback clock waited at its start until the
/// game's world was in, while the game's own clock runs from the 0xE5 its
/// world goes with, through the load (`match.md` section 27): after a slow
/// load the game was ahead of the playback by the load for the whole
/// session (under what its reports are judged by), and every relay reached
/// it that much later than a live game gets one, past its history. The
/// clock runs from that 0xE5, as the game's does.
#[test]
fn a_slow_load_leaves_the_playback_clock_with_the_games() {
    let (mut bench, replays) = bench();
    session_of_the_recording(&mut bench, &replays, 0);
    let held = Arc::new(AtomicBool::new(true));
    let holds = Arc::clone(&held);
    // Its world's parts are lost for 1.5 s (the 0xE5 that goes with them
    // is not: the game's clock is set by it).
    let at = bench.clients.len();
    bench.clients.push(Client::idle_growing(WATCHER));
    bench.clients[at].lose = Some(Box::new(move |datagram| {
        holds.load(Ordering::Relaxed)
            && carries(datagram, id::SNAPSHOT)
            && !carries(datagram, id::SET_TIME)
    }));
    bench.addrs.push(SocketAddr::from(([127, 0, 0, 1], 45_000)));
    bench.clients[at].connect();
    assert!(bench.until(1_000, |b| b.clients[at].model.options.is_some()));
    bench.clients[at].ready();
    bench.run(1_500);
    assert!(!bench.clients[at].model.is_synced());
    held.store(false, Ordering::Relaxed);
    assert!(bench.until(3_000, |b| b.clients[at].model.is_synced()));
    assert_eq!(bench.clients[at].snapshots().len(), 1, "one start");
    bench.run(500);
    let off = i64::from(bench.clients[at].clock()) - i64::from(position(&bench));
    assert!(off.abs() <= 100, "the game is {off} ms off the playback");
    bench.assert_no_faults();
}

/// Review of 2026-11-13: what came due went `HORIZON_MS` ahead of the
/// playback at every speed: at 8x that is some 30 ms of real time, and a
/// relay whose way took longer (or a lost one's copy) reached the game past
/// its tick. The lead is as much real time at every speed (bounded by what
/// a game takes ahead of its clock). At 2x here: the tests' game does not
/// scale its clock, and at 8x it would be behind past a tick byte's reach
/// within a second.
#[test]
fn at_a_higher_speed_the_relays_go_as_far_ahead_in_real_time() {
    let (mut bench, replays) = bench();
    session_of_the_recording(&mut bench, &replays, 0);
    let at = watching(&mut bench, WATCHER);
    control(&mut bench, at, 1, Op::Speed(Speed::Double));
    let recorded = [recorded(0), recorded(1)];
    let mut seen = 0;
    for _ in 0..120 {
        bench.run(25);
        // 300 to 450 ms past the playback: 150 to 225 ms of real time at
        // 2x, gone already only with the lead scaled.
        let now = position(&bench);
        for (player, recorded) in recorded.iter().enumerate() {
            let got = bench.clients[at].relays_of(player as u8);
            for (time, input) in recorded {
                if (now + 300..=now + 450).contains(time) {
                    seen += 1;
                    assert!(
                        got.contains(input),
                        "player {player}'s at {time} ms, at {now} ms"
                    );
                }
            }
        }
    }
    assert!(seen > 0, "no input looked at");
    bench.assert_no_faults();
}

/// Review of 2026-11-13: a game that connected again from another port was
/// its session's anew, with no starts behind it: each round (0xD8, probe,
/// 0xDA, its answer to the 0xE5) packed a world on the match thread, as
/// fast as it went round. Its session's starts go on from its last: one a
/// second at most, and the world (the same key frame) is packed once.
#[test]
fn a_game_connecting_again_and_again_starts_from_a_world_once_a_second() {
    let (mut bench, replays) = bench();
    session_of_the_recording(&mut bench, &replays, 0);
    let first = watching(&mut bench, WATCHER);
    let started = bench.ms;
    let mut games = vec![first];
    for _ in 0..15 {
        bench.clients[*games.last().unwrap()].silent = true;
        let at = connect_watcher(&mut bench);
        games.push(at);
        bench.run(200);
    }
    let lasted = bench.ms - started;
    let worlds: usize = games
        .iter()
        .skip(1)
        .map(|&at| bench.clients[at].snapshots().len())
        .sum();
    assert!(
        worlds as u64 <= lasted / 1_000 + 1,
        "{worlds} worlds in {lasted} ms"
    );
    // The last gets its world, the first one's key frame packed once.
    let last = *games.last().unwrap();
    assert!(bench.until(3_000, |b| b.clients[last].model.is_synced()));
    assert_eq!(
        bench.clients[last].snapshots().last().unwrap().1,
        FIRST_VIEW
    );
    bench.assert_no_faults();
}

/// The generations of the 0xE5s a game took.
fn generations(client: &Client) -> HashSet<u8> {
    client
        .got(|message| match message {
            ServerMessage::SetTime { generation, .. } => Some(*generation),
            _ => None,
        })
        .into_iter()
        .collect()
}

/// Review of 2026-11-13: what went with a world and came due in its load
/// went at the playback's speed, and at 8x that is 1.5 s past the clock
/// and 5.5 s of load: a game's rings hold 26 of a player's inputs (some
/// 1.3 s of a player moving the mouse), so a seek at 8x was put at its key
/// frame, and a busier recording's start was done again three times, then
/// loaded wrong. A world goes at 1x, to a game told 1x, whatever the speed:
/// put where a seek at 1x puts it, started once, its clock with the game's
/// through the load; the speed asked from when its world is in.
#[test]
fn at_8x_a_world_goes_and_loads_at_1x_and_the_speed_comes_after() {
    let (mut bench, replays) = bench();
    session_of_the_recording(&mut bench, &replays, 0);
    let at = watching(&mut bench, WATCHER);
    control(&mut bench, at, 1, Op::Speed(Speed::Octuple));
    bench.run(200);
    let (told, _, speed) = *notes(&bench.clients[at]).last().unwrap();
    assert_eq!(speed, Speed::Octuple);
    let worlds = bench.clients[at].snapshots().len();
    let started = generations(&bench.clients[at]);
    control(&mut bench, at, 2, Op::Seek(MatchMs(65_000)));
    assert!(bench.until(2_000, |b| b.clients[at].snapshots().len() > worlds));
    // Where a seek at 1x puts it, from the same key frame.
    let resumed = position(&bench);
    assert!(
        (PUT_FOR_65_S..PUT_FOR_65_S + 200).contains(&resumed),
        "{resumed}"
    );
    assert_eq!(bench.clients[at].snapshots().last().unwrap().1, 59_550);
    // Told 1x before its world, and started once.
    assert_eq!(
        notes(&bench.clients[at]).last(),
        Some(&(told + 1, false, Speed::Normal))
    );
    let new: Vec<u8> = generations(&bench.clients[at])
        .difference(&started)
        .copied()
        .collect();
    assert_eq!(new.len(), 1, "{new:?}");
    // The playback at 1x through the load, as the game's clock (the tests'
    // runs at 1x).
    assert!(bench.until(3_000, |b| b.clients[at].model.is_synced()));
    let off = i64::from(bench.clients[at].clock()) - i64::from(position(&bench));
    assert!(off.abs() <= 100, "the game is {off} ms off the playback");
    // Its world in: 8x, and the game told it.
    bench.run(100);
    assert_eq!(
        notes(&bench.clients[at]).last(),
        Some(&(told + 2, false, Speed::Octuple))
    );
    let from = position(&bench);
    bench.run(500);
    let went = position(&bench) - from;
    assert!((3_900..=4_100).contains(&went), "{went} ms in 500 ms");
    bench.assert_no_faults();
}

/// A game told 8x is sent no world before the note that tells it 1x is in:
/// its clock would run through the load at 8x.
#[test]
fn a_world_waits_for_its_games_note_of_1x() {
    let (mut bench, replays) = bench();
    session_of_the_recording(&mut bench, &replays, 0);
    let at = watching(&mut bench, WATCHER);
    control(&mut bench, at, 1, Op::Speed(Speed::Octuple));
    bench.run(200);
    let (told, _, _) = *notes(&bench.clients[at]).last().unwrap();
    let held = Arc::new(AtomicBool::new(true));
    let holds = Arc::clone(&held);
    bench.clients[at].lose = Some(Box::new(move |datagram| {
        holds.load(Ordering::Relaxed) && carries(datagram, id::UPLOAD_REQUEST)
    }));
    let worlds = bench.clients[at].snapshots().len();
    let started = generations(&bench.clients[at]);
    control(&mut bench, at, 2, Op::Seek(MatchMs(65_000)));
    bench.run(1_500);
    assert_eq!(bench.clients[at].snapshots().len(), worlds);
    assert_eq!(generations(&bench.clients[at]), started, "an 0xE5 went");
    held.store(false, Ordering::Relaxed);
    assert!(bench.until(3_000, |b| b.clients[at].snapshots().len() > worlds));
    assert!(
        notes(&bench.clients[at]).contains(&(told + 1, false, Speed::Normal)),
        "{:?}",
        notes(&bench.clients[at])
    );
    bench.assert_no_faults();
}

/// Review of 2026-11-13: a report was judged against the playback as if
/// it came at once, and at 8x a 500 ms round trip is 4 s of match time: a
/// game in step was taken for one off and started again and again. Judged
/// past the round trip at the speed: an in-step game's reports pass, a game
/// off by as much as before is started again.
#[test]
fn at_8x_a_long_round_trip_is_no_drift() {
    use survarium::v026g::match_msgs::client_model::EVENT_HORIZON_MS;
    const WAY_MS: u64 = 250;
    let (mut bench, replays) = bench();
    session_of_the_recording(&mut bench, &replays, 0);
    let at = watching(&mut bench, WATCHER);
    bench.one_way = vec![0; at + 1];
    bench.one_way[at] = WAY_MS;
    let request = Request {
        seq: 1,
        op: Op::Speed(Speed::Octuple),
    };
    bench.clients[at].send(id::LEAVE, &request.encode(), true);
    bench.run_long(2_000);
    // A game in step: behind the playback by the way of the speed's note
    // (7x the way at 8x); its report comes a way later, at 8x.
    let in_step = |bench: &Bench| position(bench) - 7 * WAY_MS as u32 - EVENT_HORIZON_MS;
    let worlds = bench.clients[at].snapshots().len();
    for _ in 0..6 {
        let time = in_step(&bench);
        bench.clients[at].report(time, 1);
        bench.run(50);
    }
    bench.run_long(2_000);
    assert_eq!(bench.clients[at].snapshots().len(), worlds, "started again");
    // 4 s ahead of that: off.
    for _ in 0..3 {
        let time = in_step(&bench) + 4_000;
        bench.clients[at].report(time, 1);
        bench.run(50);
    }
    assert!(bench.until(3_000, |b| b.clients[at].snapshots().len() > worlds));
    bench.assert_no_faults();
}

/// The watcher's game sought (its control `seq`) to 40 s, where a player's
/// inputs are few (they do not overrun its rings while its world loads
/// for seconds): its world's parts are lost until the switch given back is
/// let off, its 0xE5 (that sets its clock) is not. With the number of notes
/// it had taken before.
fn seek_into_a_slow_load(bench: &mut Bench, at: usize, seq: u8) -> (Arc<AtomicBool>, usize) {
    let held = Arc::new(AtomicBool::new(true));
    let holds = Arc::clone(&held);
    bench.clients[at].lose = Some(Box::new(move |datagram| {
        holds.load(Ordering::Relaxed)
            && carries(datagram, id::SNAPSHOT)
            && !carries(datagram, id::SET_TIME)
    }));
    let told = notes(&bench.clients[at]).len();
    control(bench, at, seq, Op::Seek(MatchMs(40_000)));
    (held, told)
}

/// The paused bit of the last playback note its game took.
fn told_paused(client: &Client) -> Option<bool> {
    notes(client).last().map(|&(_, paused, _)| paused)
}

/// Review of 2026-11-13: a pause asked while a world loaded stopped the
/// playback clock at once, while its game's ran on from its 0xE5 until it was
/// live and told (a note goes only to a live game): the game was ahead of the
/// playback by the rest of the load for good, and every relay reached it that
/// much late. The pause is taken once the world is in, where its game's
/// clock then is.
#[test]
fn a_pause_asked_while_a_world_loads_stops_where_its_game_stops() {
    let (mut bench, replays) = bench();
    session_of_the_recording(&mut bench, &replays, 0);
    let at = watching(&mut bench, WATCHER);
    bench.run_long(2_000);
    let (held, told) = seek_into_a_slow_load(&mut bench, at, 1);
    bench.run(300);
    control(&mut bench, at, 2, Op::Pause(true));
    bench.run(1_000);
    assert_eq!(
        notes(&bench.clients[at]).len(),
        told,
        "told before its world"
    );
    held.store(false, Ordering::Relaxed);
    assert!(bench.until(3_000, |b| notes(&b.clients[at]).len() > told));
    assert_eq!(told_paused(&bench.clients[at]), Some(true));
    // The viewer's game stops where its clock is as it takes the note.
    let stopped = bench.clients[at].clock();
    let off = i64::from(stopped) - i64::from(position(&bench));
    assert!(off.abs() <= 100, "the game stops {off} ms off the playback");
    let paused = position(&bench);
    bench.run_long(2_000);
    assert_eq!(position(&bench), paused);
    // Played: both from there.
    control(&mut bench, at, 3, Op::Pause(false));
    bench.run(900);
    let went = i64::from(position(&bench)) - i64::from(stopped);
    assert!((900..=1_100).contains(&went), "{went} ms in 1 s");
    bench.assert_no_faults();
}

/// Review of 2026-11-13: a pause and a play asked within one load lost the
/// time between them on the playback clock, the game's running on (told
/// neither). It is as if neither was asked.
#[test]
fn a_pause_and_a_play_within_one_load_leave_the_clock_running() {
    let (mut bench, replays) = bench();
    session_of_the_recording(&mut bench, &replays, 0);
    let at = watching(&mut bench, WATCHER);
    bench.run_long(2_000);
    let (held, told) = seek_into_a_slow_load(&mut bench, at, 1);
    bench.run(300);
    control(&mut bench, at, 2, Op::Pause(true));
    bench.run(400);
    control(&mut bench, at, 3, Op::Pause(false));
    bench.run(400);
    held.store(false, Ordering::Relaxed);
    assert!(bench.until(3_000, |b| notes(&b.clients[at]).len() > told));
    assert_eq!(told_paused(&bench.clients[at]), Some(false));
    bench.run(500);
    let off = i64::from(bench.clients[at].clock()) - i64::from(position(&bench));
    assert!(off.abs() <= 100, "the game is {off} ms off the playback");
    bench.assert_no_faults();
}

/// A paused game sought stays paused through its new world's load (the
/// viewer's patch keeps its pause), its clock standing where the 0xE5 put it:
/// a play asked meanwhile is taken once the world is in (told then), from
/// there, not from when it was asked.
#[test]
fn a_play_asked_while_a_paused_games_world_loads_starts_from_its_world() {
    let (mut bench, replays) = bench();
    session_of_the_recording(&mut bench, &replays, 0);
    let at = watching(&mut bench, WATCHER);
    bench.run_long(2_000);
    control(&mut bench, at, 1, Op::Pause(true));
    let (held, told) = seek_into_a_slow_load(&mut bench, at, 2);
    let stood = position(&bench);
    bench.run(300);
    control(&mut bench, at, 3, Op::Pause(false));
    bench.run(1_000);
    assert_eq!(position(&bench), stood, "its game's clock stands");
    held.store(false, Ordering::Relaxed);
    assert!(bench.until(3_000, |b| notes(&b.clients[at]).len() > told));
    assert_eq!(told_paused(&bench.clients[at]), Some(false));
    // Its game plays from where it stood as it takes the note.
    let ahead = i64::from(position(&bench)) - i64::from(stood);
    assert!(
        (0..=100).contains(&ahead),
        "the playback is {ahead} ms ahead"
    );
    bench.assert_no_faults();
}

/// The recording made a busy match of 20 places, all moving the mouse from
/// its first key frame on: 15 make an input every tick, 5 two a tick
/// (precise inputs).
fn busy() -> ReplayRecording {
    const PLACES: u8 = 20;
    let mut recording = playable_of(PLACES);
    let mut inputs = Vec::new();
    for tick in (FIRST_VIEW / TICK_MS)..(114_000 / TICK_MS) {
        let at = tick * TICK_MS;
        for place in 0..PLACES {
            let input = |time: u32, precise: bool| PlayerInput {
                time: if precise {
                    WireTime::precise(time)
                } else {
                    WireTime::tick(time)
                },
                rotation: [place, tick as u8, (tick >> 8) as u8, (time % TICK_MS) as u8],
                actions: 0,
            };
            if place < 15 {
                inputs.push((at, usize::from(place), input(at, false)));
            } else {
                inputs.push((at + 10, usize::from(place), input(at + 10, true)));
                inputs.push((at + 35, usize::from(place), input(at + 35, true)));
            }
        }
    }
    inputs.sort_by_key(|(at, _, _)| *at);
    recording.replace_inputs(inputs);
    recording
}

/// Relays tapped off a client's datagrams ([`tap_relays`]): place and input.
type Tapped = Arc<Mutex<Vec<(u8, PlayerInput)>>>;

/// Every input relayed to a client as its datagrams come (taken by its
/// game's model or not: the bench's game does not scale its clock), place
/// and input, gathered until taken out.
fn tap_relays(client: &mut Client) -> Tapped {
    let tapped = Arc::new(Mutex::new(Vec::new()));
    let into = Arc::clone(&tapped);
    client.lose = Some(Box::new(move |datagram| {
        let parsed = udp_transport::wire::parse(
            udp_transport::Table::Match,
            udp_transport::Direction::ToClient,
            datagram,
            mask_bytes(20),
        );
        for message in parsed.map(|parsed| parsed.messages).unwrap_or_default() {
            if let Ok(ServerMessage::InputRelay { player, input }) =
                decode_server(message.id, message.payload, 20)
            {
                into.lock().unwrap().push((player, input));
            }
        }
        false
    }));
    tapped
}

/// The busy match's session watched at `speed` on a line of `way` ms each
/// way, settled: the bench, the watcher's client and its relays
/// ([`tap_relays`]).
fn busy_at(speed: Speed, way: u64) -> (Bench, Arc<Replays>, usize, Tapped) {
    let (mut bench, replays) = bench();
    let request = request(WATCHER_ACCOUNT, 0x1234, 0, None);
    prepared_of(&mut bench, &replays, request, Vec::new(), busy());
    let at = watching(&mut bench, WATCHER);
    let tapped = tap_relays(&mut bench.clients[at]);
    bench.one_way = vec![0; at + 1];
    bench.one_way[at] = way;
    bench.run(1_000);
    control(&mut bench, at, 1, Op::Speed(speed));
    bench.run(500);
    (bench, replays, at, tapped)
}

/// Review of 2026-11-13: at 8x what was due went 1.5 s of match ahead of the
/// playback (its time lead), and the game's clock is behind it by the way
/// of its speed's note: some 2 s of each player's inputs it had not played,
/// 40 of one moving the mouse, more of one with precise inputs, while its
/// rings keep 27 a player, the oldest dropped (`match.md` section 30). The
/// earliest were dropped unplayed and its world went wrong, unseen by the
/// drift (which compares times); at 4x a player with precise inputs passed
/// the rings too. While it plays, no player's inputs go further past its
/// clock than its rings hold: by the bench's model of its clock (behind the
/// playback by the way of its speed's note, at the speed), no player ever
/// has more than `INPUT_RING_SAFE` (26) inputs it has not played, and every
/// input comes, before its tick.
#[test]
fn at_a_higher_speed_no_player_has_more_inputs_ahead_of_the_game_than_its_rings_hold() {
    const WAY_MS: u32 = 25;
    const RING_SAFE: usize = 26;
    for speed in [Speed::Quadruple, Speed::Octuple] {
        let (mut bench, _replays, at, tapped) = busy_at(speed, WAY_MS.into());
        let behind = WAY_MS * u32::from(speed.percent()) / 100;
        let worlds = bench.clients[at].snapshots().len();
        let mut taken: Vec<HashSet<u32>> = vec![HashSet::new(); 20];
        let from = position(&bench) - behind;
        let mut game = from;
        // What came before.
        for (player, input) in tapped.lock().unwrap().drain(..) {
            taken[usize::from(player)].insert(input.time.resolve(from).unwrap());
        }
        for _ in 0..600 {
            let now = position(&bench);
            for (player, input) in tapped.lock().unwrap().drain(..) {
                let time = input.time.resolve(now).unwrap();
                assert!(
                    !taken[usize::from(player)].insert(time) || time > game,
                    "{speed:?}: player {player}'s input at {time} ms came with the game at {game} ms"
                );
            }
            for (player, times) in taken.iter().enumerate() {
                let ahead = times.iter().filter(|&&time| time > game).count();
                assert!(
                    ahead <= RING_SAFE,
                    "{speed:?}: player {player} has {ahead} inputs unplayed at {game} ms"
                );
            }
            bench.step(5);
            game = position(&bench) - behind;
        }
        assert!(
            game - from > 2_000 * u32::from(speed.percent()) / 100,
            "{speed:?}"
        );
        for (player, times) in taken.iter().enumerate() {
            let missing = (from / TICK_MS + 1..game / TICK_MS)
                .flat_map(|tick| match player {
                    0..15 => vec![tick * TICK_MS],
                    _ => vec![tick * TICK_MS + 10, tick * TICK_MS + 35],
                })
                .filter(|time| !times.contains(time))
                .count();
            assert_eq!(missing, 0, "{speed:?}: player {player}");
        }
        assert_eq!(
            bench.clients[at].snapshots().len(),
            worlds,
            "{speed:?}: started again"
        );
        bench.assert_no_faults();
    }
}

/// The busy match at 8x: its game acks nothing for 0.6 s (more than a live
/// spectator's cap piles up meanwhile) and is not taken for one that acks
/// too little.
#[test]
fn at_8x_a_busy_matchs_viewer_that_stalls_its_acks_is_kept() {
    let (mut bench, replays, at, _) = busy_at(Speed::Octuple, 25);
    let mut most = 0;
    bench.clients[at].mute_acks = true;
    for _ in 0..120 {
        bench.step(5);
        most = most.max(backlog(&bench).unwrap_or(0));
    }
    bench.clients[at].mute_acks = false;
    bench.run(1_000);
    // Past a live spectator's cap (`BACKLOG_MOST`, 2 048).
    assert!(most > 2_048, "{most} messages not acked at most");
    let addr = SocketAddr::from(([127, 0, 0, 1], 45_000 + at as u16));
    let watch = bench.server.replay_watch.as_ref().unwrap();
    assert!(watch.owns(addr));
    assert!(
        !watch.is_let_go(addr),
        "let go at {most} messages not acked"
    );
    assert!(replays.watches().holds(WATCHER_ACCOUNT));
    bench.assert_no_faults();
}
