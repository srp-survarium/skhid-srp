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
use crate::v026g::record::{IN, MAGIC, OUT};
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
    let map = survarium::v026g::maps::playable().next().unwrap();
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
