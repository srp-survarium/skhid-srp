//! The server's conversion against a recording file made here: what the
//! files are, what the sidecar says, and that nothing private of the
//! recording reaches the `.sur`.

use std::io::Write;

use survarium::v026g::dictionary::Dictionary;
use survarium::v026g::lzma2;
use survarium::v026g::match_msgs::static_info::{
    MatchItem, MatchKit, MatchOptions, MatchProfile, NO_PROTECTION, SCORE_EVENTS, StaticInfo,
};
use survarium::v026g::match_msgs::upload;

use super::*;
use crate::v026g::record::{IN, MAGIC, OUT};
use crate::v026g::sur;

const MATCH: u32 = 0x1F07_C8FB;
/// A session id, a token and an address the recording holds (a 0xD8, a
/// note) and the replay must not.
const SESSION: u32 = 0x5EC2_E7A9;
const TOKEN: &str = "0123456789abcdef0123456789abcdef";
const ADDRESS: &str = "203.0.113.77:51234";

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "srp-replay-{name}-{}-{}",
        std::process::id(),
        Replays::now_unix()
    ));
    let _ = fs::remove_dir_all(&dir);
    dir
}

fn settings(root: &Path) -> ReplaySettings {
    let settings = ReplaySettings {
        replay_dir: root.join("replays"),
        record_dir: root.join("recordings"),
        modes_dir: root.join("modes"),
        max_recording_bytes: 1 << 20,
        max_replay_bytes: 1 << 30,
    }
    .checked()
    .unwrap();
    fs::create_dir_all(&settings.modes_dir).unwrap();
    settings
}

fn profile(name: &str, team: u8) -> MatchProfile {
    MatchProfile {
        team,
        name: name.into(),
        clan_tag: String::new(),
        protection: NO_PROTECTION,
        kits: vec![MatchKit::empty(1, "kit")],
        items: vec![MatchItem::default()],
        squad: 0,
    }
}

/// `kind place at_ms len bytes`.
fn record(kind: u8, place: u8, at_ms: u32, bytes: &[u8]) -> Vec<u8> {
    let mut out = vec![kind, place];
    out.extend(at_ms.to_le_bytes());
    out.extend((bytes.len() as u32).to_le_bytes());
    out.extend(bytes);
    out
}

/// `seq rseq ack0 id [x] payload`: a datagram of one message.
fn datagram(id: u8, x: Option<u8>, payload: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0, 0, 0, id];
    bytes.extend(x);
    bytes.extend_from_slice(payload);
    bytes
}

/// A recording of a match of two places and a late joiner, in one block,
/// as the writer makes them; it holds a session id, a token, an address
/// and the server's folders, where the writer would never put them, to
/// show that none reaches the replay. Returns the mode config's name.
fn write_recording(path: &Path, secrets: &[&str]) -> String {
    let map = survarium::v026g::maps::playable().next().unwrap();
    let options = MatchOptions {
        players: vec![profile("a", 0), profile("b", 1)],
        map_id: map.id,
        match_id: MATCH | 0x8000_0000,
        match_level: 1,
        score_weights: [0.0; SCORE_EVENTS],
    };
    let info = StaticInfo::new(&options, Dictionary::shared()).unwrap();
    let note =
        |at: u32, value: serde_json::Value| record(NOTE, 0xFF, at, value.to_string().as_bytes());
    let mut raw = Vec::new();
    raw.extend(note(
        0,
        serde_json::json!({
            "note": "header", "map": map.name, "map_id": map.id, "mode": map.mode,
            "match_id": format!("{MATCH:#x}"),
            "places": [
                {"place": 0, "account_id": 1001, "name": "a"},
                {"place": 1, "account_id": 1002, "name": "b"},
            ],
            "planted": secrets,
        }),
    ));
    // The client's 0xD8 with its session, as the server records it.
    raw.extend(record(
        IN,
        0,
        10,
        &datagram(0xD8, None, &SESSION.to_le_bytes()),
    ));
    for (place, revision) in [(0, 12), (1, 11), (0, 12)] {
        raw.extend(note(
            20,
            serde_json::json!({"note": "revision", "place": place, "revision": revision}),
        ));
    }
    raw.extend(note(
        1350,
        serde_json::json!({"note": "clock", "match_ms": 50}),
    ));
    for place in 0..2u8 {
        raw.extend(record(
            OUT,
            place,
            1400,
            &datagram(0xE4, Some(0), &info.payload(place).unwrap()),
        ));
        raw.extend(record(OUT, place, 1700, &datagram(0xE9, None, &[8, 0])));
    }
    let mut relay = vec![20, 1, 1, 2, 3, 4];
    relay.extend(0x10u32.to_le_bytes());
    raw.extend(record(OUT, 0, 2400, &datagram(0xEB, None, &relay)));
    // A late joiner's place: in the accounts, never connected.
    raw.extend(note(
        2500,
        serde_json::json!({"note": "places", "places": [{"place": 2, "account_id": 1003}]}),
    ));
    for (time, fill) in [(50u32, 1u8), (1050, 2)] {
        for chunk in upload::split(time, &vec![fill; 700]).unwrap() {
            raw.extend(record(IN, 0, 3000 + time, &datagram(0xE0, None, &chunk)));
        }
    }
    let packed = lzma2::packed(&raw);
    let mut file = fs::File::create(path).unwrap();
    file.write_all(MAGIC).unwrap();
    file.write_all(&(packed.len() as u32).to_le_bytes())
        .unwrap();
    file.write_all(&(raw.len() as u32).to_le_bytes()).unwrap();
    file.write_all(&packed).unwrap();
    map.config.to_string()
}

/// Every byte the game reads of a `.sur`: chunk 0 as it is, the others
/// unpacked.
fn decoded(file: &[u8]) -> Vec<Vec<u8>> {
    sur::chunks(file)
        .unwrap()
        .into_iter()
        .map(|(id, bytes)| match id {
            sur::chunk::VERSION => bytes.to_vec(),
            sur::chunk::KEY_FRAMES => lzma2::unpack(&bytes[2..], sur::MAX_DECODED).unwrap(),
            _ => lzma2::unpack(bytes, sur::MAX_DECODED).unwrap(),
        })
        .collect()
}

fn holds(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|part| part == needle)
}

#[test]
fn a_kept_recording_becomes_a_replay_and_its_sidecar() {
    let root = temp("make");
    let settings = settings(&root);
    let recording = settings
        .record_dir
        .join(format!("rec026g_20261015-120000_{MATCH:08x}.srprec"));
    let replay_dir = settings.replay_dir.to_string_lossy().into_owned();
    let record_dir = settings.record_dir.to_string_lossy().into_owned();
    let config = write_recording(&recording, &[TOKEN, ADDRESS, &replay_dir, &record_dir]);
    fs::write(settings.modes_dir.join(&config), b"the mode's config").unwrap();

    let sidecar = make(&settings, MATCH).unwrap();
    let sur_path = settings.replay_dir.join("1f07c8fb.sur");
    let file = fs::read(&sur_path).unwrap();
    sur::verify(&file).unwrap();
    assert_eq!(sidecar.size, file.len() as u64);
    assert_eq!(
        sidecar.sha256,
        openssl::sha::sha256(&file)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    );
    assert_eq!(sidecar.account_ids, [1001, 1002, 1003]);
    assert_eq!(sidecar.revisions, [Some(12), Some(11), None]);
    let json = fs::read_to_string(settings.replay_dir.join("1f07c8fb.json")).unwrap();
    assert_eq!(Sidecar::parse(&json), Some(sidecar));
    // Nothing is left under `.part`, and nothing else is made.
    let mut names: Vec<String> = fs::read_dir(&settings.replay_dir)
        .unwrap()
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, ["1f07c8fb.json", "1f07c8fb.sur"]);

    // What the players were sent, and the mode's config; nothing of the
    // session, the token, the address or the server's folders, in the file
    // or in what the game unpacks of it.
    let chunks = decoded(&file);
    assert!(holds(&chunks[4], b"the mode's config"));
    for place in [&file[..], &chunks.concat()] {
        assert!(!holds(place, &SESSION.to_le_bytes()));
        for secret in [
            TOKEN,
            ADDRESS,
            &replay_dir,
            &record_dir,
            "planted",
            "account_id",
        ] {
            assert!(!holds(place, secret.as_bytes()), "{secret}");
        }
    }
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_recording_over_the_cap_or_without_its_config_is_not_converted() {
    let root = temp("refused");
    let mut settings = settings(&root);
    let recording = settings
        .record_dir
        .join(format!("rec026g_20261015-120000_{MATCH:08x}.srprec"));
    let config = write_recording(&recording, &[]);
    let error = make(&settings, MATCH).unwrap_err();
    assert!(error.why.contains("config"), "{error:?}");
    // The operator may add the mode's file: tried again.
    assert_eq!(error.failure, Failure::ModeFile);
    fs::write(settings.modes_dir.join(&config), b"config").unwrap();
    settings.max_recording_bytes = 100;
    let error = make(&settings, MATCH).unwrap_err();
    assert!(error.why.contains("bytes"), "{error:?}");
    assert_eq!(error.failure, Failure::Lasting);
    // A recording still being written, or of another match: none.
    assert!(make(&settings, 0x1234).is_err());
    fs::rename(&recording, recording.with_extension("srprec.part")).unwrap();
    settings.max_recording_bytes = 1 << 20;
    assert_eq!(
        make(&settings, MATCH).unwrap_err(),
        MakeError {
            why: "no kept recording".into(),
            failure: Failure::Lasting
        }
    );
    assert_eq!(fs::read_dir(&settings.replay_dir).unwrap().count(), 0);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn the_mode_file_is_checked_before_the_recording_is_unpacked_whole() {
    let root = temp("header-first");
    let settings = settings(&root);
    let recording = settings
        .record_dir
        .join(format!("rec026g_20261015-120000_{MATCH:08x}.srprec"));
    let config = write_recording(&recording, &[]);
    // A second block that does not unpack: reading the whole file fails.
    let mut file = fs::OpenOptions::new()
        .append(true)
        .open(&recording)
        .unwrap();
    file.write_all(&4u32.to_le_bytes()).unwrap();
    file.write_all(&100u32.to_le_bytes()).unwrap();
    file.write_all(&[0xFF; 4]).unwrap();
    drop(file);
    assert!(record::read_notes(&recording).is_err());
    // The header is read from the first block alone.
    let header = record::read_header(&recording).unwrap().unwrap();
    assert_eq!(
        sur_convert::config_name(std::slice::from_ref(&header)),
        Some(config.clone())
    );
    // No mode file: said before the bad block is reached.
    assert_eq!(
        make(&settings, MATCH).unwrap_err().failure,
        Failure::ModeFile
    );
    // With it, the bad block: the recording's own failure, for good.
    fs::write(settings.modes_dir.join(&config), b"config").unwrap();
    let error = make(&settings, MATCH).unwrap_err();
    assert_eq!(error.failure, Failure::Lasting, "{error:?}");
    assert!(error.why.contains("unpack"), "{error:?}");
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn the_oldest_replays_over_the_cap_go_and_the_new_one_stays() {
    let root = temp("cap");
    let mut settings = settings(&root);
    settings.max_replay_bytes = 2_500;
    let dir = settings.replay_dir.clone();
    let at = |seconds: u64| SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(seconds);
    // Four replays of 1,000 bytes and a sidecar of 10 each, the newest last.
    for (n, id) in [3u32, 1, 4, 2].into_iter().enumerate() {
        let sur = dir.join(format!("{id:08x}.sur"));
        fs::write(&sur, vec![0u8; 1000]).unwrap();
        fs::File::options()
            .write(true)
            .open(&sur)
            .unwrap()
            .set_modified(at(1_000_000 + n as u64))
            .unwrap();
        fs::write(dir.join(format!("{id:08x}.json")), [b'x'; 10]).unwrap();
    }
    // Other names are neither counted nor touched.
    for name in ["notes.txt", "00000009.sur.part", "ABCDEF01.sur"] {
        fs::write(dir.join(name), vec![0u8; 5000]).unwrap();
    }
    let names = || {
        let mut names: Vec<String> = fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    };
    // Under the cap: nothing goes.
    settings.max_replay_bytes = 4_040;
    cap(&settings, None);
    assert_eq!(names().len(), 11);
    // 4,040 bytes over 2,500: the two oldest (3, then 1) go.
    settings.max_replay_bytes = 2_500;
    cap(&settings, None);
    assert_eq!(
        names(),
        [
            "00000002.json",
            "00000002.sur",
            "00000004.json",
            "00000004.sur",
            "00000009.sur.part",
            "ABCDEF01.sur",
            "notes.txt"
        ]
    );
    // The one just made stays, even when it alone is over the cap.
    settings.max_replay_bytes = 100;
    cap(&settings, Some(4));
    assert_eq!(
        names(),
        [
            "00000004.json",
            "00000004.sur",
            "00000009.sur.part",
            "ABCDEF01.sur",
            "notes.txt"
        ]
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn replays_of_gone_recordings_and_parts_are_deleted_and_nothing_else() {
    let root = temp("prune");
    let settings = settings(&root);
    fs::write(
        settings
            .record_dir
            .join("rec026g_20261015-120000_00000001.srprec"),
        b"x",
    )
    .unwrap();
    let names = [
        "00000001.sur",
        "00000001.json",
        "00000002.sur",
        "00000002.json",
        "00000003.sur.part",
        "00000004.json.part",
        "ABCDEF01.sur",
        "notes.txt",
        "srp.db",
    ];
    for name in names {
        fs::write(settings.replay_dir.join(name), b"x").unwrap();
    }
    prune(&settings);
    let mut left: Vec<String> = fs::read_dir(&settings.replay_dir)
        .unwrap()
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    left.sort();
    assert_eq!(
        left,
        [
            "00000001.json",
            "00000001.sur",
            "ABCDEF01.sur",
            "notes.txt",
            "srp.db"
        ]
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn the_thread_converts_what_is_queued_and_the_desk_hears_of_it() {
    let root = temp("thread");
    let settings = settings(&root);
    let recording = settings
        .record_dir
        .join(format!("rec026g_20261015-120000_{MATCH:08x}.srprec"));
    let config = write_recording(&recording, &[]);
    fs::write(settings.modes_dir.join(&config), b"config").unwrap();
    // The recording's games were of 12 and 11: given whatever the last row
    // of simulation_changes.tsv is (the refusals have their own tests).
    let replays = Arc::new(Replays::new(Some(settings.clone()), 6).with_oldest_revision(11));
    let thread = start(replays.clone()).unwrap().unwrap();
    // Kept: nothing is made until someone asks.
    replays.recording_kept(MATCH);
    std::thread::sleep(std::time::Duration::from_millis(100));
    assert!(!settings.replay_dir.join("1f07c8fb.sur").exists());
    let known = Some(session::v026g::replays::Known {
        played: true,
        places: 2,
    });
    let first = replays.ask(1001, MATCH, Some(14), known, Instant::now());
    assert_eq!(first, replays::Status::Preparing);
    // The movie's polls: each looks again while it is `preparing`.
    let mut status = None;
    for _ in 0..500 {
        let answer = replays.answer(1001, Instant::now());
        if answer.status != replays::Status::Preparing {
            status = Some(answer.status);
            assert!(answer.link().is_some());
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    // Revision 14's game: place 0 played with 12 and place 1 with 11, which
    // 14 plays.
    assert_eq!(status, Some(replays::Status::Ready));
    replays.stop();
    thread.join().unwrap();
    let _ = fs::remove_dir_all(&root);
}
