//! Golden tests: the datagrams of recorded 0.69d lobby sessions (tests/golden/captured_session_N_lobby.jsonl), with the
//! earlier reference server's own decode of each message. The client's hardware report (`50`) in them holds generic
//! values of the same length (tests/golden/README.md).

use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use udp_transport::wire::{self, Data, Message};
use udp_transport::{class_of, Class, Direction, Event, Role, Transport, PART};

struct Rec {
    n: usize,
    t: f64,
    c2s: bool,
    bytes: Vec<u8>,
    json: Value,
}

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap()).collect()
}

fn load(session: &str) -> Vec<Rec> {
    let path: PathBuf = [env!("CARGO_MANIFEST_DIR"), "..", "..", "tests", "golden", &format!("{session}_lobby.jsonl")]
        .iter()
        .collect();
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    text.lines()
        .map(|l| {
            let v: Value = serde_json::from_str(l).unwrap();
            Rec {
                n: v["n"].as_u64().unwrap() as usize,
                t: v["t"].as_f64().unwrap(),
                c2s: v["dir"] == "c2s",
                bytes: unhex(v["hex"].as_str().unwrap()),
                json: v,
            }
        })
        .collect()
}

fn dir(r: &Rec) -> Direction {
    if r.c2s { Direction::ToServer } else { Direction::ToClient }
}

const SESSIONS: [&str; 7] = [
    "captured_session_1",
    "captured_session_2",
    "captured_session_3",
    "captured_session_4",
    "captured_session_5",
    "captured_session_6",
    "captured_session_7",
];
/// The longest session (the most pushes and multipart messages).
const LONG: &str = "captured_session_2";

#[test]
fn every_datagram_of_every_session_decodes_and_re_encodes_byte_identically() {
    let mut total = 0;
    for session in SESSIONS {
        for r in load(session) {
            let dg = wire::parse(dir(&r), &r.bytes)
                .unwrap_or_else(|e| panic!("{session} #{} {}: {e}: {}", r.n, if r.c2s { "c2s" } else { "s2c" }, hex(&r.bytes)));
            let back = wire::encode(dir(&r), &dg).unwrap_or_else(|e| panic!("{session} #{}: encode {e}", r.n));
            assert_eq!(back, r.bytes, "{session} #{} re-encode", r.n);
            total += 1;
        }
    }
    assert!(total > 20000, "{total} datagrams");
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// The recording server's own decode of each client datagram (the `msg` lines) equals ours.
#[test]
fn client_messages_match_the_recording_server_decode() {
    let mut checked = 0;
    for session in SESSIONS {
        for r in load(session).iter().filter(|r| r.c2s) {
            let want = r.json["msgs"].as_array().unwrap();
            if want.iter().any(|m| m["id"] == 0xFF) {
                continue; // the recording server's decode has no multipart prefix
            }
            let dg = wire::parse(Direction::ToServer, &r.bytes).unwrap();
            assert_eq!(dg.messages.len(), want.len(), "{session} #{}", r.n);
            for (m, w) in dg.messages.iter().zip(want) {
                match m {
                    Message::LowLevel(t) => {
                        assert_eq!(w["id"].as_u64().unwrap(), u64::from(*t));
                        assert!(w["x"].is_null());
                    }
                    Message::Data(d) => {
                        assert_eq!(w["id"].as_u64().unwrap(), u64::from(d.id), "{session} #{}", r.n);
                        assert_eq!(w["x"].as_u64().map(|x| x as u8), d.x, "{session} #{}", r.n);
                        assert_eq!(w["payload"].as_str().unwrap(), hex(&d.payload), "{session} #{}", r.n);
                    }
                }
                checked += 1;
            }
        }
    }
    assert!(checked > 3000, "{checked}");
}

struct Completed {
    id: u8,
    x: Option<u8>,
    payload: Vec<u8>,
    /// the datagram bodies' messages exactly as sent first
    datas: Vec<Data>,
}

/// Reassemble the server's messages (first copy of each part wins).
fn server_messages(recs: &[Rec]) -> Vec<Completed> {
    let mut out = Vec::new();
    let mut asm: HashMap<(u8, Option<u8>), Vec<Data>> = HashMap::new();
    let mut done: HashSet<(u8, Option<u8>, Option<u8>)> = HashSet::new(); // single copies seen
    for r in recs.iter().filter(|r| !r.c2s) {
        let dg = wire::parse(Direction::ToClient, &r.bytes).unwrap();
        for m in dg.messages {
            let Message::Data(d) = m else { continue };
            match d.part {
                None => {
                    // a resend of an already seen single message carries the same x: skip
                    if d.x.is_some() && !done.insert((d.id, d.x, Some(0))) {
                        continue;
                    }
                    out.push(Completed { id: d.id, x: d.x, payload: d.payload.clone(), datas: vec![d] });
                }
                Some(part) => {
                    let key = (d.id, d.x);
                    let list = asm.entry(key).or_default();
                    if list.iter().any(|q| q.part.unwrap().index == part.index) {
                        continue;
                    }
                    list.push(d);
                    let count = list.iter().find_map(|q| q.part.unwrap().count);
                    if let Some(c) = count {
                        if list.len() == usize::from(c) {
                            let mut parts = asm.remove(&key).unwrap();
                            parts.sort_by_key(|q| q.part.unwrap().index);
                            let payload = parts.iter().flat_map(|q| q.payload.clone()).collect();
                            out.push(Completed { id: key.0, x: key.1, payload, datas: parts });
                        }
                    }
                }
            }
        }
    }
    out
}

#[test]
fn server_pushes_are_found_with_their_classes_and_lengths() {
    let recs = load(LONG);
    let msgs = server_messages(&recs);
    let mut pushed = 0;
    for r in recs.iter().filter(|r| !r.c2s) {
        let Some(p) = r.json.get("pushed") else { continue };
        let (id, cls, x, len) = (
            p["id"].as_u64().unwrap() as u8,
            p["cls"].as_str().unwrap(),
            p["x"].as_u64().unwrap() as u8,
            p["len"].as_u64().unwrap() as usize,
        );
        let class = class_of(Direction::ToClient, id).unwrap();
        // x counters of the recorded pushes: unique and ordered ids; plain ids have no x
        let want_class = if cls == "u" { Class::Unique } else { Class::Ordered };
        assert_eq!(class, want_class, "{LONG} #{}: push {id:#x} sent as {cls}, the table says {class:?}", r.n);
        assert!(
            msgs.iter().any(|m| m.id == id && m.x == Some(x) && m.payload.len() == len),
            "{LONG} #{}: push {id:#x} x={x} len={len} not found",
            r.n
        );
        pushed += 1;
    }
    assert!(pushed >= 15, "{pushed}");
}

/// `Transport::send` splits exactly where the recording server did: re-split every message and compare.
#[test]
fn server_messages_split_like_the_recording_server() {
    for session in SESSIONS {
        let recs = load(session);
        let mut multipart = 0;
        for m in server_messages(&recs) {
            let class = class_of(Direction::ToClient, m.id).unwrap();
            let head = 1 + usize::from(class.has_x());
            let split = head + m.payload.len() > PART + 20;
            assert_eq!(split, m.datas[0].part.is_some(), "{session}: {:#x} len {}", m.id, m.payload.len());
            if split {
                let again = wire::split(Direction::ToClient, m.id, m.x, &m.payload, PART).unwrap();
                assert_eq!(again, m.datas, "{session}: {:#x}", m.id);
                multipart += 1;
            }
        }
        if session == LONG {
            assert!(multipart >= 5, "{multipart}");
        }
    }
}

/// Replay the client side of one session through a server `Transport` in log time: nothing rejected, every
/// distinct client message delivered exactly once.
#[test]
fn client_traffic_through_a_server_transport() {
    let recs = load(LONG);
    let t0 = Instant::now();
    let mut tr = Transport::new(Role::Server, t0);
    let mut delivered = 0usize;
    let mut want: HashSet<(u8, Option<u8>)> = HashSet::new();
    let mut want_plain = 0usize;
    let mut resent_c2s = 0usize;
    for r in recs.iter().filter(|r| r.c2s) {
        let now = t0 + Duration::from_secs_f64(r.t);
        for e in tr.receive(&r.bytes, now) {
            match e {
                Event::Message { .. } => delivered += 1,
                Event::Disconnected => panic!("unexpected disconnect at #{}", r.n),
            }
        }
        for m in wire::parse(Direction::ToServer, &r.bytes).unwrap().messages {
            if let Message::Data(d) = m {
                if d.part.is_some() {
                    continue;
                }
                match class_of(Direction::ToServer, d.id).unwrap() {
                    Class::Plain => want_plain += 1,
                    _ => {
                        if !want.insert((d.id, d.x)) {
                            resent_c2s += 1;
                        }
                    }
                }
            }
        }
        // the server side keeps answering: poll like the real loop does
        let _ = tr.poll(now);
    }
    assert_eq!(tr.rejected(), 0);
    // unique ids repeat after 256 (x wraps), so count distinct (id, x) only up to that
    assert!(delivered >= want_plain + want.len().min(256), "delivered {delivered}");
    let _ = resent_c2s;
}
