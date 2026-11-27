//! Golden tests: messages that worked with the real client (`tests/vectors/*.json`) and real client requests from
//! lobby logs. Every vector must decode into a typed message; re-encoding must give the same bytes (packed LZMA2
//! messages: the same message after a second decode, the vectors are compressed for real and this crate writes stored
//! chunks).

use protocol::c2s::Request;
use protocol::s2c::{Reply, S2c};
use std::path::PathBuf;

struct Vector {
    name: String,
    id: u8,
    payload: Vec<u8>,
}

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap()).collect()
}

fn load(file: &str) -> Vec<Vector> {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests/vectors");
    p.push(file);
    let text = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    v.as_array()
        .unwrap()
        .iter()
        .map(|e| Vector {
            name: e["name"].as_str().unwrap().to_string(),
            id: e["id"].as_u64().unwrap() as u8,
            payload: ac_count_fix(e["id"].as_u64().unwrap() as u8, unhex(e["payload"].as_str().unwrap())),
        })
        .collect()
}

/// Some vectors hold `AC` as 8 bytes; the client's reader (0x20018AEC0) always reads a `u8` server-row count after
/// them, so those are short by that byte. Compared with the count 0 added.
fn ac_count_fix(id: u8, mut payload: Vec<u8>) -> Vec<u8> {
    if id == 0xAC && payload.len() == 8 {
        payload.push(0);
    }
    payload
}

/// Messages whose wire form is LZMA2: compared after a second decode.
fn packed(m: &S2c) -> bool {
    matches!(m, S2c::Inventory(_) | S2c::Prices(_) | S2c::Reply(Reply::Play(_)))
}

fn check_s2c(file: &str) -> usize {
    let vs = load(file);
    for v in &vs {
        let m = S2c::decode(v.id, &v.payload).unwrap_or_else(|e| panic!("{file} {}: decode {:#04x}: {e}", v.name, v.id));
        match &m {
            S2c::Raw { .. } => panic!("{file} {}: id {:#04x} is not modelled", v.name, v.id),
            S2c::Reply(Reply::Raw { request, .. }) => panic!("{file} {}: reply to {request:#04x} is Raw", v.name),
            _ => {}
        }
        assert_eq!(m.id(), v.id, "{file} {}", v.name);
        let out = m.encode_payload().unwrap_or_else(|e| panic!("{file} {}: encode: {e}", v.name));
        if packed(&m) {
            let again = S2c::decode(v.id, &out).unwrap();
            assert_eq!(m, again, "{file} {}: packed round trip", v.name);
        } else {
            assert_eq!(out, v.payload, "{file} {}: re-encoded bytes differ ({m:?})", v.name);
        }
    }
    vs.len()
}

#[test]
fn s2c_pushes() {
    assert!(check_s2c("s2c_pushes.json") >= 30);
}

#[test]
fn s2c_replies() {
    assert!(check_s2c("s2c_replies.json") >= 25);
}

#[test]
fn s2c_social() {
    assert!(check_s2c("s2c_social.json") >= 14);
}

#[test]
fn s2c_results() {
    assert!(check_s2c("s2c_results.json") >= 4);
}

#[test]
fn s2c_misc_layouts_from_notes() {
    assert!(check_s2c("s2c_misc.json") >= 20);
}

#[test]
fn c2s_handler_requests() {
    let vs = load("c2s_handlers.json");
    for v in &vs {
        let r = Request::decode(v.id, &v.payload).unwrap_or_else(|e| panic!("{}: {e}", v.name));
        assert!(!matches!(r, Request::Other { .. }), "{}: not modelled", v.name);
        let (id, out) = r.encode().unwrap();
        assert_eq!((id, out), (v.id, v.payload.clone()), "{}", v.name);
    }
    assert!(vs.len() >= 14);
}

#[test]
fn c2s_from_real_logs() {
    let vs = load("c2s_logs.json");
    let mut other = std::collections::BTreeMap::<u8, usize>::new();
    for v in &vs {
        if v.id == 0x50 {
            continue; // hardware report, payload not stored
        }
        let r = Request::decode(v.id, &v.payload).unwrap_or_else(|e| panic!("{} id {:#04x} {}: {e}", v.name, v.id, hex(&v.payload)));
        if let Request::Other { id, .. } = &r {
            *other.entry(*id).or_default() += 1;
        }
        let (id, out) = r.encode().unwrap();
        assert_eq!((id, &out), (v.id, &v.payload), "{} {}", v.name, hex(&v.payload));
    }
    assert!(other.is_empty(), "requests of the logs that are not modelled: {other:?}");
    assert!(vs.len() >= 100);
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}
