//! The desync capture (`game/desync_capture.rs`): at the first tick the
//! games disagree on, one game of each side is asked for its world, timed
//! for one tick, and both are saved with what differs.

use std::path::{Path, PathBuf};

use super::{Bench, MAP_BATTERIES};
use crate::v026g::Settings;

/// A bench whose worlds are saved in a folder of its own.
fn bench(players: usize, name: &str) -> (Bench, PathBuf) {
    made(Bench::new, players, name, &[])
}

/// The same with a bench made by `make`, the games at `exact` answering a
/// request for a tick (patch 211; their 0xD8 says so).
fn made(
    make: fn(usize, survarium::v026g::dictionary::MapId, Settings) -> Bench,
    players: usize,
    name: &str,
    exact: &[usize],
) -> (Bench, PathBuf) {
    let dir = std::env::temp_dir().join(format!("srp-capture-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let settings = Settings {
        snapshot_dir: Some(dir.clone()),
        ..Settings::default()
    };
    let mut bench = make(players, MAP_BATTERIES, settings);
    for place in exact {
        bench.clients[*place].exact_worlds = true;
    }
    bench.join_all();
    bench.run(4_000);
    bench.run(3_000);
    (bench, dir)
}

/// Each capture folder's files, by name.
fn captures(dir: &Path) -> Vec<(String, Vec<String>)> {
    let mut found: Vec<(String, Vec<String>)> = std::fs::read_dir(dir.join("captures"))
        .map(|entries| {
            entries
                .flatten()
                .map(|entry| {
                    let mut files: Vec<String> = std::fs::read_dir(entry.path())
                        .map(|files| {
                            files
                                .flatten()
                                .map(|file| file.file_name().to_string_lossy().into_owned())
                                .collect()
                        })
                        .unwrap_or_default();
                    files.sort();
                    (entry.file_name().to_string_lossy().into_owned(), files)
                })
                .collect()
        })
        .unwrap_or_default();
    found.sort();
    found
}

fn summary(dir: &Path, folder: &str) -> serde_json::Value {
    let text = std::fs::read(dir.join("captures").join(folder).join("capture.json")).unwrap();
    serde_json::from_slice(&text).unwrap()
}

/// Live, 2026-11-25, match 0x421b579c: the games split 7 against 7, no
/// side a majority, and no world was asked for until 9 s later. Here two
/// games against two: one of each side is asked for its world at once,
/// both give the same tick, and the summary names the first field.
#[test]
fn a_split_with_no_majority_has_a_world_of_each_side_saved_at_one_tick() {
    let (mut bench, dir) = bench(4, "no-majority");
    // Places 1 and 3 have the better links on their sides.
    bench.one_way = vec![40, 10, 40, 10];
    bench.run(5_000);
    let from = bench.clients[2].clock();
    for place in [2, 3] {
        bench.clients[place].sim.salt_from = Some((from, 7));
    }
    // The first wrong report comes a horizon (500 ms) and a way later.
    assert!(bench.until(1_500, |bench| bench.game().captures_taken() == 1));
    assert!(bench.until(2_000, |bench| { !bench.game().capture_pending() }));
    let found = captures(&dir);
    let [(folder, files)] = &found[..] else {
        panic!("{found:?}");
    };
    let summary = summary(&dir, folder);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(files.len(), 3, "{files:?}");
    assert!(
        files[1].contains("_p1_capture_") && files[2].contains("_p3_capture_"),
        "{files:?}"
    );
    assert_eq!(files[0], "capture.json");
    let ticks: Vec<u64> = summary["worlds"]
        .as_array()
        .unwrap()
        .iter()
        .map(|world| world["tick"].as_u64().unwrap())
        .collect();
    assert_eq!(ticks[0], ticks[1], "{summary:#}");
    assert_eq!(summary["same_tick"], true);
    assert_eq!(summary["asked"], "aimed", "{summary:#}");
    assert_eq!(
        summary["asked_tick"].as_u64(),
        Some(ticks[0]),
        "{summary:#}"
    );
    let split = summary["split_tick"].as_u64().unwrap();
    assert!(
        split >= u64::from(from) && split < u64::from(from) + 100,
        "{summary:#}"
    );
    let after = summary["after_split_ms"].as_u64().unwrap();
    assert!(after <= 300, "{summary:#}");
    assert_eq!(summary["groups"][0]["places"].as_array().unwrap().len(), 2);
    assert_eq!(summary["groups"][1]["places"].as_array().unwrap().len(), 2);
    // The salt is the bullets section's first byte: its header's time.
    let first = &summary["diff"]["differences"][0];
    assert_eq!(first["part"], "bullets", "{summary:#}");
    assert_eq!(first["in_part"], 0);
    assert_eq!(first["field"]["object"], "header", "{summary:#}");
    assert_eq!(first["field"]["path"], "current_time_ms", "{summary:#}");
    assert_eq!(summary["diff"]["differences"].as_array().unwrap().len(), 1);
}

/// A game against the majority: its world and a majority game's, at its
/// first wrong report; once while the split lasts; and its resync goes as
/// it would without the capture.
#[test]
fn a_game_against_the_majority_is_captured_once_and_resynced_as_before() {
    let (mut bench, dir) = bench(3, "majority");
    let from = bench.clients[2].clock();
    bench.clients[2].sim.salt_from = Some((from, 7));
    assert!(bench.until(1_500, |bench| bench.game().captures_taken() == 1));
    assert_eq!(bench.game().resyncs(2), 0);
    assert!(bench.until(2_000, |bench| !bench.game().capture_pending()));
    assert!(bench.until(10_000, |bench| bench.game().resyncs(2) == 1));
    bench.clients[2].sim.salt_from = None;
    bench.run(5_000);
    assert_eq!(bench.game().captures_taken(), 1);
    let found = captures(&dir);
    let _ = std::fs::remove_dir_all(&dir);
    let [(_, files)] = &found[..] else {
        panic!("{found:?}");
    };
    assert!(
        files.iter().any(|file| file.contains("_p2_capture_")),
        "{files:?}"
    );
    assert!(
        files.iter().any(|file| file.contains("_p0_capture_")),
        "{files:?}"
    );
}

/// A capture's request its game never answered (a full packet pool skips
/// it) takes none of its later uploads: asked later as a source for
/// another game's resync, its world is a vote. The capture is written with
/// the world that came.
#[test]
fn an_unanswered_capture_request_does_not_take_the_next_upload() {
    let (mut bench, dir) = bench(3, "unanswered");
    bench.clients[1].no_uploads = true;
    let from = bench.clients[1].clock();
    bench.clients[1].sim.salt_from = Some((from, 7));
    assert!(bench.until(1_500, |bench| bench.game().capture_awaits(1)));
    // Wrong for a moment only, and its request skipped.
    bench.clients[1].sim.salt_from = None;
    bench.run(5_000);
    assert!(!bench.game().capture_awaits(1));
    bench.clients[1].no_uploads = false;
    let asked = bench.clients[1].upload_requests;
    // Player 0 goes wrong: its resync's source is player 1 (the best link;
    // the lowest place of equal ones). No capture: within 20 s of the last.
    let from = bench.clients[0].clock();
    bench.clients[0].sim.salt_from = Some((from, 9));
    assert!(bench.until(15_000, |bench| bench.game().resyncs(0) == 1));
    assert!(bench.until(5_000, |bench| bench.clients[1].upload_requests > asked));
    bench.run(2_000);
    assert_eq!(bench.game().captures_taken(), 1);
    let found = captures(&dir);
    let _ = std::fs::remove_dir_all(&dir);
    let [(_, files)] = &found[..] else {
        panic!("{found:?}");
    };
    assert!(
        !files.iter().any(|file| file.contains("_p1_capture_")),
        "{files:?}"
    );
    assert_eq!(
        files.iter().filter(|file| file.ends_with(".bin")).count(),
        1,
        "{files:?}"
    );
}

/// Patch 211: games that answer a request for a tick are asked for the
/// split's first tick itself, at once, and both give it.
#[test]
fn games_that_answer_for_a_tick_give_the_splits_own_tick() {
    let (mut bench, dir) = made(Bench::growing, 4, "exact", &[0, 1, 2, 3]);
    bench.one_way = vec![40, 10, 40, 10];
    bench.run(5_000);
    let from = bench.clients[2].clock();
    for place in [2, 3] {
        bench.clients[place].sim.salt_from = Some((from, 7));
    }
    assert!(bench.until(1_500, |bench| bench.game().captures_taken() == 1));
    assert!(bench.until(2_000, |bench| !bench.game().capture_pending()));
    let found = captures(&dir);
    let [(folder, files)] = &found[..] else {
        panic!("{found:?}");
    };
    let summary = summary(&dir, folder);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(files.len(), 3, "{files:?}");
    assert_eq!(summary["asked"], "tick", "{summary:#}");
    assert_eq!(summary["exact"], true, "{summary:#}");
    // Written once the longer links' reports of the tick came: all four.
    assert_eq!(
        summary["groups"][0]["places"].as_array().unwrap().len(),
        2,
        "{summary:#}"
    );
    assert_eq!(
        summary["groups"][1]["places"].as_array().unwrap().len(),
        2,
        "{summary:#}"
    );
    let split = summary["split_tick"].as_u64().unwrap();
    assert_eq!(summary["asked_tick"].as_u64(), Some(split), "{summary:#}");
    assert_eq!(summary["after_split_ms"].as_u64(), Some(0), "{summary:#}");
    for world in summary["worlds"].as_array().unwrap() {
        assert_eq!(world["tick"].as_u64(), Some(split), "{summary:#}");
    }
    let first = &summary["diff"]["differences"][0];
    assert_eq!(first["part"], "bullets", "{summary:#}");
    assert_eq!(first["field"]["path"], "current_time_ms", "{summary:#}");
    bench.assert_no_faults();
}

/// A game of each side that does not answer for a tick (an earlier exe):
/// both are asked for their horizons' worlds, timed, as before.
#[test]
fn a_game_that_does_not_answer_for_a_tick_keeps_the_timed_capture() {
    let (mut bench, dir) = made(Bench::growing, 4, "mixed", &[0, 1, 2]);
    bench.one_way = vec![40, 10, 40, 10];
    bench.run(5_000);
    let from = bench.clients[2].clock();
    for place in [2, 3] {
        bench.clients[place].sim.salt_from = Some((from, 7));
    }
    assert!(bench.until(1_500, |bench| bench.game().captures_taken() == 1));
    assert!(bench.until(2_000, |bench| !bench.game().capture_pending()));
    let found = captures(&dir);
    let [(folder, _)] = &found[..] else {
        panic!("{found:?}");
    };
    let summary = summary(&dir, folder);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(summary["asked"], "aimed", "{summary:#}");
    assert_eq!(summary["same_tick"], true, "{summary:#}");
    // Place 3 (the better link of its side) has no patch 211: it was sent
    // the one byte an earlier game takes.
    assert!(
        bench.clients[3].faults.is_empty(),
        "{:?}",
        bench.clients[3].faults
    );
}
