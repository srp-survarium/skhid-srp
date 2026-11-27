//! The sign-in `94` read the way the 0.69d client reads it (the client crashes at sign-in on blobs it cannot decode).
//! A model of the client's reader, not of our protocol crate:
//! * reader 0x200189F90: `u32 N`, N records `{u32 f30, f38, f3c, f40, f44, u32 L1, u32 L2, u8 type, u8 x, L1 bytes, L2
//!   bytes}` (L1 <= 0x800, L2 <= 0x400), `u16`, `u32 M` x 12 bytes, `u32 K` x 4 bytes, and when a record has type 0x16:
//!   `u8 present`, `u16 C` (<= 60), C bytes, `u32 A`, `u32 B`, `u32 len`, len bytes;
//! * status loop 0x2001C5820 / 0x2001C5A49 / 0x2001C6350: a record with both buffers empty is skipped; data1 must be a
//!   raw LZMA2 stream of at most 0x1000 bytes (a config); data2 of the types in the mask 0x03C2A00008800100 a raw LZMA2
//!   stream of at most 0x800 bytes, read by the type's reader within the decoded bytes:
//!   0x1B (0x2001B9040): `u16 n` <= 64 daily_reward `{u32, u16, u8}`, `u8 flag`, flag: `f64, u8, u8, u8`;
//!   0x17 (0x2001B8EF0): `u8 n` <= 6 x `{u16, u32, u16, u32}`, `u8 k` <= 32 x `{u16, u8 m <= 4, m x {u16, u32}}`, `u8`;
//!   type 0x16 with a tail blob (`set_contract_data` 0x2001C5F00): LZMA2 (at most 0x800), config keys `amount`,
//!   `actionType`, `dictId`.
//!
//! The real account: `SRP_CLIENT94_DB=<dir with srp-069d.sqlite (+ -wal, -shm)> SRP_CLIENT94_ACCOUNT=<account name>` runs the
//! same check on a COPY of that database (the files are copied to a temp dir first).

mod support;

use std::sync::Arc;

use accounts::ContractState;
use lobby_server::{LobbyConfig, MinimalMatches};
use protocol::binconfig::{self, Value};
use support::*;

struct Rd<'a> {
    b: &'a [u8],
    at: usize,
}

impl<'a> Rd<'a> {
    fn take(&mut self, n: usize) -> &'a [u8] {
        assert!(self.at + n <= self.b.len(), "the client would read past the end ({} + {n} > {})", self.at, self.b.len());
        let s = &self.b[self.at..self.at + n];
        self.at += n;
        s
    }
    fn u8(&mut self) -> u8 {
        self.take(1)[0]
    }
    fn u16(&mut self) -> u16 {
        u16::from_le_bytes(self.take(2).try_into().unwrap())
    }
    fn u32(&mut self) -> u32 {
        u32::from_le_bytes(self.take(4).try_into().unwrap())
    }
}

/// The client's LZMA2 decode of a record buffer: must be a valid raw stream whose output fits `cap`.
fn lzma2(what: &str, b: &[u8], cap: usize) -> Vec<u8> {
    assert!(!b.is_empty(), "{what}: an empty stream fails in the client");
    assert!(matches!(b[0], 0x00 | 0x01 | 0x80..=0xFF), "{what}: first byte {:#04x} is not a valid LZMA2 control byte", b[0]);
    let out = protocol::lzma2::decompress(b).unwrap_or_else(|e| panic!("{what}: not a raw LZMA2 stream: {e}"));
    assert!(out.len() <= cap, "{what}: {} decoded bytes > the client's {cap:#x}", out.len());
    out
}

/// What the status loop did with the records: (type, what happened).
fn client_reads_94(payload: &[u8]) -> Vec<(u8, &'static str)> {
    let mut r = Rd { b: payload, at: 0 };
    let n = r.u32() as usize;
    let mut recs = vec![];
    for _ in 0..n {
        for _ in 0..5 {
            r.u32();
        }
        let (l1, l2) = (r.u32() as usize, r.u32() as usize);
        assert!(l1 <= 0x800 && l2 <= 0x400, "record buffers {l1} / {l2} overflow the client's stack record");
        let ty = r.u8();
        r.u8();
        recs.push((ty, r.take(l1).to_vec(), r.take(l2).to_vec()));
    }
    r.u16();
    let m = r.u32() as usize;
    r.take(m * 12);
    let k = r.u32() as usize;
    r.take(k * 4);
    let mut tail_blob = None;
    if recs.iter().any(|x| x.0 == 0x16) && r.u8() != 0 {
        let c = r.u16() as usize;
        assert!(c <= 60, "tail C {c} > 60");
        r.take(c);
        r.u32();
        r.u32();
        let len = r.u32() as usize;
        tail_blob = Some(r.take(len).to_vec());
    }
    assert_eq!(r.at, payload.len(), "bytes left after the 94");
    const MASK: u64 = 0x03C2_A000_0880_0100;
    let mut seen = vec![];
    for (ty, d1, d2) in recs {
        if d1.is_empty() && d2.is_empty() {
            seen.push((ty, "skipped (both buffers empty)"));
            continue;
        }
        if !d1.is_empty() {
            let body = lzma2(&format!("type {ty:#x} data1"), &d1, 0x1000);
            assert!(!body.is_empty(), "type {ty:#x}: empty config");
        }
        if ty <= 0x39 && MASK & (1 << ty) != 0 {
            if d2.is_empty() {
                seen.push((ty, "action without data2"));
                continue;
            }
            let body = lzma2(&format!("type {ty:#x} data2"), &d2, 0x800);
            if ty == 0x1B {
                // 0x2001B9040 reads into a fixed buffer of 64 daily rewards (0x2001C6B62, 12 bytes each)
                let mut b = Rd { b: &body, at: 0 };
                let days = b.u16();
                assert!(days <= 64, "{days} daily rewards > the client's 64 (buffer overflow assert)");
                b.take(usize::from(days) * 7);
                if b.u8() != 0 {
                    b.take(8 + 3);
                }
                seen.push((ty, "calendar read"));
            } else if ty == 0x17 {
                // 0x2001B8EF0 into the lobby object +0x15F334: 6 rows of 16 bytes from +4, 32 entries of 40 bytes from +0x68
                let mut b = Rd { b: &body, at: 0 };
                let n = b.u8();
                assert!(n <= 6, "{n} exchange rows overflow the client's 6");
                b.take(usize::from(n) * 12);
                let k = b.u8();
                assert!(k <= 32, "{k} entries overflow the client's 32");
                for _ in 0..k {
                    b.u16();
                    let m = b.u8();
                    assert!(m <= 4, "{m} sub-entries overflow the client's 4");
                    b.take(usize::from(m) * 6);
                }
                b.u8();
                seen.push((ty, if n > 0 { "exchange rows read" } else { "event read, no rows" }));
            } else {
                seen.push((ty, "action read"));
            }
        } else if ty == 0x16 {
            let blob = tail_blob.clone().expect("type 0x16 needs the tail");
            assert!(!blob.is_empty(), "set_contract_data needs a tail blob");
            let cfg = lzma2("contract tail blob", &blob, 0x800);
            let keys = binconfig::decode(&cfg).expect("the decoded contract blob is a config");
            for key in ["amount", "actionType", "dictId"] {
                assert!(keys.iter().any(|(k, v)| k == key && matches!(v, Value::Int(_))), "contract key {key}");
            }
            seen.push((ty, "contract read"));
        } else {
            seen.push((ty, "other"));
        }
    }
    seen
}

fn sign_in_94(e: &mut Env, c: &mut Client) -> Vec<u8> {
    c.send(protocol::c2s::Request::SignIn { session: c.session, a: 0xFF, b: 0xFF });
    for _ in 0..300 {
        e.step(&mut [&mut *c], std::time::Duration::from_millis(30));
        if c.inbox.iter().any(|(id, _)| *id == 0xB8) {
            break;
        }
    }
    e.pump(&mut [&mut *c]);
    let raw: Vec<Vec<u8>> = c.inbox.iter().filter(|(id, _)| *id == 0x94).map(|(_, p)| p.clone()).collect();
    assert!(!raw.is_empty(), "a 94 at sign-in");
    raw[0].clone()
}

/// Sign-in of an account with a running contract, the default 7-day calendar, a premium account and two selected
/// boosters: every record the client reaches is decodable, the calendar fits, the contract keys are there.
#[test]
fn the_sign_in_94_with_a_running_contract_reads_cleanly_in_the_client_model() {
    game_data::require_tables!();
    let dir = tempfile::tempdir().unwrap();
    let cfg = LobbyConfig { achievement_rewards: false, ..LobbyConfig::default() };
    assert!(cfg.login_calendar.is_some(), "the default calendar is on");
    let mut e = Env::with(&dir.path().join("c.sqlite"), cfg, Arc::new(MinimalMatches));
    let mut c = e.client("test");
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
    e.store
        .update_profile(c.account, |p| {
            p.contract = ContractState { started: now - 3600, won_mask: 0 };
            p.premium_until = now + 86_400;
            p.boosters.entry(853).or_default().enabled = true;
            p.boosters.entry(339).or_default().enabled = true;
            Ok::<(), ()>(())
        })
        .unwrap();
    let seen = client_reads_94(&sign_in_94(&mut e, &mut c));
    assert!(seen.contains(&(0x16, "contract read")), "{seen:?}");
    assert!(seen.contains(&(0x1B, "calendar read")), "{seen:?}");
}

/// A running event (default off): the type 0x17 record's second body decodes to the exchange rows and the reader stays
/// inside the decoded bytes (a one-byte `00` stream decodes to nothing and the reader would run past it).
#[test]
fn the_sign_in_94_with_an_event_reads_the_exchange_rows_in_the_client_model() {
    game_data::require_tables!();
    let dir = tempfile::tempdir().unwrap();
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
    let event = game_data::events::Schedule::parse(&format!("{}:{}:{}", now - 60, now + 3600, now + 7200)).unwrap();
    let cfg = LobbyConfig { achievement_rewards: false, events: vec![event], ..LobbyConfig::default() };
    let mut e = Env::with(&dir.path().join("ev.sqlite"), cfg, Arc::new(MinimalMatches));
    let mut c = e.client("test");
    let seen = client_reads_94(&sign_in_94(&mut e, &mut c));
    assert!(seen.contains(&(0x17, "exchange rows read")), "{seen:?}");
    assert!(seen.contains(&(0x1B, "calendar read")), "{seen:?}");
}

/// The same check on a copy of a real database (skipped unless `SRP_CLIENT94_DB` is set).
#[test]
fn the_sign_in_94_of_a_real_account_reads_cleanly() {
    game_data::require_tables!();
    let Ok(src) = std::env::var("SRP_CLIENT94_DB") else { return };
    let name = std::env::var("SRP_CLIENT94_ACCOUNT").unwrap_or_else(|_| "069-test".into());
    let dir = tempfile::tempdir().unwrap();
    for f in ["srp-069d.sqlite", "srp-069d.sqlite-wal", "srp-069d.sqlite-shm"] {
        let from = std::path::Path::new(&src).join(f);
        if from.exists() {
            std::fs::copy(&from, dir.path().join(f)).unwrap();
        }
    }
    let cfg = LobbyConfig { achievement_rewards: false, ..LobbyConfig::default() };
    let mut e = Env::with(&dir.path().join("srp-069d.sqlite"), cfg, Arc::new(MinimalMatches));
    let mut c = e.client_of(&name);
    let p = e.store.load_profile(c.account).unwrap();
    let payload = sign_in_94(&mut e, &mut c);
    let seen = client_reads_94(&payload);
    println!("account {name}: contract {:?}, 94 = {} bytes, records {seen:?}", p.contract, payload.len());
    println!("94 hex: {}", payload.iter().map(|b| format!("{b:02x}")).collect::<String>());
}
