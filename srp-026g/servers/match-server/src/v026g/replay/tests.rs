//! What the recordings give the replays: each place's account and game
//! revision ([`places`]), and that nothing private of a recording reaches
//! a `.sur` made of it (`sur_convert`, which `srp-recording-to-sur` runs).

use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

use survarium::v026g::dictionary::Dictionary;
use survarium::v026g::lzma2;
use survarium::v026g::match_msgs::static_info::{
    MatchItem, MatchKit, MatchOptions, MatchProfile, NO_PROTECTION, SCORE_EVENTS, StaticInfo,
};
use survarium::v026g::match_msgs::upload;

use super::*;
use crate::v026g::record::{ACCEPTED_WORLD, AcceptedWorld, IN, MAGIC, NO_PLACE, OUT};
use crate::v026g::sur;
use crate::v026g::sur_convert::{self, Settings};

const MATCH: u32 = 0x1F07_C8FB;
/// A session id, a token and an address the recording holds (a 0xD8, a
/// note) and the replay must not.
const SESSION: u32 = 0x5EC2_E7A9;
const TOKEN: &str = "0123456789abcdef0123456789abcdef";
const ADDRESS: &str = "203.0.113.77:51234";

/// The game revisions of the recording's places 0 and 1: one after today's,
/// and today's (the oldest whose recordings are given), so a game of the
/// newer plays it whatever the last simulation change was.
fn revisions() -> (u8, u8) {
    let today = replays::oldest_replayable_here(0);
    (today + 1, today)
}

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "srp-replay-{name}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
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
    // A match of two teams (its places are on teams 0 and 1): the first
    // battery map.
    let map = survarium::v026g::maps::playable_in(survarium::v026g::maps::GameMode::Batteries)
        .next()
        .expect("the game has battery maps");
    let options = MatchOptions {
        players: vec![profile("a", 0), profile("b", 1)],
        map: survarium::v026g::maps::MatchMap::of(map.id).unwrap(),
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
            "note": "header", "map": map.name, "map_id": map.id, "mode": map.mode.rules().number(),
            "match_id": format!("{MATCH:#x}"), "accepted_worlds": true,
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
    let (newer, today) = revisions();
    for (place, revision) in [(0, newer), (1, today), (0, newer)] {
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
    // Worlds the server took.
    for (time, fill) in [(50u32, 1u8), (1050, 2)] {
        let body = vec![fill; 700];
        for chunk in upload::split(time, &body).unwrap() {
            raw.extend(record(IN, 0, 3000 + time, &datagram(0xE0, None, &chunk)));
        }
        let taken = AcceptedWorld::of(time, &body).to_bytes();
        raw.extend(record(ACCEPTED_WORLD, NO_PLACE, 3000 + time, &taken));
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
fn a_recordings_places_read_and_its_sur_holds_nothing_private() {
    let root = temp("convert");
    let recording = root.join(format!("rec026g_20261015-120000_{MATCH:08x}.srprec"));
    let dir = root.to_string_lossy().into_owned();
    write_recording(&recording, &[TOKEN, ADDRESS, &dir]);
    assert_eq!(recording_in(&root, MATCH), Some(recording.clone()));
    assert_eq!(recording_in(&root, 0x1234), None);

    let notes = record::read_notes(&recording).unwrap();
    let (account_ids, revisions) = places(&notes);
    assert_eq!(account_ids, [1001, 1002, 1003]);
    let (newer, today) = self::revisions();
    assert_eq!(revisions, [Some(newer), Some(today), None]);

    let (file, _) =
        sur_convert::convert_file(&recording, &notes, b"the mode's config", Settings::default())
            .unwrap();
    sur::verify(&file).unwrap();
    // What the players were sent, and the mode's config; nothing of the
    // session, the token, the address or the server's folders, in the file
    // or in what the game unpacks of it.
    let chunks = decoded(&file);
    assert!(holds(&chunks[4], b"the mode's config"));
    for place in [&file[..], &chunks.concat()] {
        assert!(!holds(place, &SESSION.to_le_bytes()));
        for secret in [TOKEN, ADDRESS, &dir, "planted", "account_id"] {
            assert!(!holds(place, secret.as_bytes()), "{secret}");
        }
    }
    let _ = fs::remove_dir_all(&root);
}

/// Recordings in `root/recordings`, Save's files in `root/replays`.
fn saving(root: &Path) -> Replays {
    let record_dir = root.join("recordings");
    fs::create_dir_all(&record_dir).unwrap();
    let save = SaveSettings {
        replay_dir: root.join("replays"),
        max_replay_bytes: 1 << 30,
    }
    .checked(&record_dir)
    .unwrap();
    Replays::new(Some(record_dir)).with_save(save)
}

fn names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn a_kept_recording_becomes_a_save_file_that_holds_nothing_private() {
    let root = temp("save");
    let replays = saving(&root);
    let record_dir = replays.record_dir().unwrap().to_owned();
    let replay_dir = replays.save_settings().unwrap().replay_dir.clone();
    let recording = record_dir.join(format!("rec026g_20261015-120000_{MATCH:08x}.srprec"));
    let secrets = [
        TOKEN,
        ADDRESS,
        &record_dir.to_string_lossy(),
        &replay_dir.to_string_lossy(),
    ]
    .map(str::to_owned);
    write_recording(&recording, &secrets.each_ref().map(String::as_str));

    let sidecar = make(&replays, None, MATCH).unwrap();
    assert_eq!(names(&replay_dir), ["1f07c8fb.json", "1f07c8fb.srpreplay"]);
    let bytes = fs::read(replay_dir.join("1f07c8fb.srpreplay")).unwrap();
    assert_eq!(sidecar.size, bytes.len() as u64);
    let sha: String = openssl::sha::sha256(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    assert_eq!(sidecar.sha256, sha);
    assert_eq!(sidecar.account_ids, [1001, 1002, 1003]);
    let (newer, today) = revisions();
    assert_eq!(sidecar.revisions, [Some(newer), Some(today), None]);
    let json = fs::read_to_string(replay_dir.join("1f07c8fb.json")).unwrap();
    assert_eq!(Sidecar::parse(&json), Some(sidecar));

    // The container: the header of the options and the recording's name,
    // and a `.sur` a replay session plays.
    let file = ReplayFile::read(&bytes).unwrap();
    let map = survarium::v026g::maps::playable_in(survarium::v026g::maps::GameMode::Batteries)
        .next()
        .unwrap();
    assert_eq!(file.header.match_id, MATCH);
    // 2026-10-15 12:00:00 UTC.
    assert_eq!(file.header.made, 1_792_065_600);
    assert_eq!(file.header.map, map.id);
    assert_eq!(file.header.mode, map.mode.lobby_number());
    assert_eq!(file.header.score, None);
    let names: Vec<&str> = file.header.places.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["a", "b"]);
    assert!(file.events.is_empty());
    crate::v026g::game::playback::Recording::from_sur(&file.sur, MATCH).unwrap();
    // Nothing of the session, the token, the address, the accounts or the
    // server's folders, in the file or in what the game unpacks of it.
    let chunks = decoded(&file.sur);
    for place in [&bytes[..], &chunks.concat()] {
        assert!(!holds(place, &SESSION.to_le_bytes()));
        assert!(!holds(place, &1001u32.to_le_bytes()));
        for secret in secrets.iter().map(String::as_str).chain(["planted", "account_id", "1001"]) {
            assert!(!holds(place, secret.as_bytes()), "{secret}");
        }
    }

    // A recording still being written, or of another match: none made.
    assert!(make(&replays, None, 0x1234).is_err());
    fs::rename(&recording, recording.with_extension("srprec.part")).unwrap();
    assert_eq!(
        make(&replays, None, MATCH).unwrap_err(),
        MakeError {
            why: "no kept recording".into(),
            failure: Failure::Lasting
        }
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn the_oldest_files_over_the_cap_go_and_the_new_one_stays() {
    let root = temp("cap");
    let replays = saving(&root);
    let mut save = replays.save_settings().unwrap().clone();
    let dir = save.replay_dir.clone();
    let at = |seconds: u64| UNIX_EPOCH + std::time::Duration::from_secs(seconds);
    // Four files of 1,000 bytes and a sidecar of 10 each, the newest last.
    for (n, id) in [3u32, 1, 4, 2].into_iter().enumerate() {
        let path = dir.join(format!("{id:08x}.srpreplay"));
        fs::write(&path, vec![0u8; 1000]).unwrap();
        fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(at(1_000_000 + n as u64))
            .unwrap();
        fs::write(dir.join(format!("{id:08x}.json")), [b'x'; 10]).unwrap();
    }
    // Other names are neither counted nor touched.
    for name in ["notes.txt", "00000009.srpreplay.part", "ABCDEF01.srpreplay"] {
        fs::write(dir.join(name), vec![0u8; 5000]).unwrap();
    }
    save.max_replay_bytes = 4_040;
    cap(&save, None);
    assert_eq!(names(&dir).len(), 11);
    // 4,040 bytes over 2,500: the two oldest (3, then 1) go.
    save.max_replay_bytes = 2_500;
    cap(&save, None);
    let left = [
        "00000002.json",
        "00000002.srpreplay",
        "00000004.json",
        "00000004.srpreplay",
        "00000009.srpreplay.part",
        "ABCDEF01.srpreplay",
        "notes.txt",
    ];
    assert_eq!(names(&dir), left);
    // The one just made stays, even when it alone is over the cap.
    save.max_replay_bytes = 100;
    cap(&save, Some(4));
    assert_eq!(names(&dir), left[2..]);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn files_of_gone_recordings_and_parts_are_deleted_and_nothing_else() {
    let root = temp("prune");
    let replays = saving(&root);
    let save = replays.save_settings().unwrap();
    let record_dir = replays.record_dir().unwrap();
    fs::write(
        record_dir.join("rec026g_20261015-120000_00000001.srprec"),
        b"x",
    )
    .unwrap();
    for name in [
        "00000001.srpreplay",
        "00000001.json",
        "00000002.srpreplay",
        "00000002.json",
        "00000003.srpreplay.part",
        "00000004.json.part",
        "ABCDEF01.srpreplay",
        "notes.txt",
    ] {
        fs::write(save.replay_dir.join(name), b"x").unwrap();
    }
    prune(save, record_dir);
    assert_eq!(
        names(&save.replay_dir),
        [
            "00000001.json",
            "00000001.srpreplay",
            "ABCDEF01.srpreplay",
            "notes.txt"
        ]
    );
    let _ = fs::remove_dir_all(&root);
}

/// A note's revision 0 is a session the server did not know: `None`, which
/// a replay's check does not count, not a game of revision 0.
#[test]
fn a_note_of_revision_zero_is_not_known() {
    let note = |place: u8, revision: u8| record::Record {
        kind: NOTE,
        place: 0,
        at_ms: 0,
        bytes: format!(r#"{{"note":"revision","place":{place},"revision":{revision}}}"#)
            .into_bytes(),
    };
    let (_, revisions) = by_place(&[note(0, 0), note(1, 40)]);
    assert_eq!(revisions, [None, Some(40)]);
}
