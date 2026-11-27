//! Steam-mode sign-in (`-steam` client, method 2) through the Import panel, and the proof that the normal sign-in is
//! unchanged. The Steam frames are the golden sign-in with the account string replaced by a SteamID64, as the
//! client's builder writes them (0x20011F5DE: 8 raw bytes, then the same tail); the `04` frames follow
//! `import_account` (0x200126C7A..0x200126E17) and "Create new" (0x20011ED10).

use login_server::digest::{client_password_digest, sha1_hex};
use login_server::frame::{frame, FrameReader};
use login_server::handshake::{parse_import, parse_plaintext, parse_sign_in, IMPORT_PASSWORD_TYPE, SIGNIN_STEAM};
use login_server::keys::encrypt_with_public_der;
use login_server::*;
use serde_json::Value;
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream};
use std::sync::{Mutex, OnceLock};

const LOBBY: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 1235);
const CHALLENGE: u32 = 0xC0FFEE01;
const STEAM_ID: u64 = 76561197960265728; // the SteamID64 base: account number 0, no real account
const SESSION: &str = "captured_session_2";

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap()).collect()
}

struct Golden {
    sign_in_frame: Vec<u8>,
    plaintext: Vec<u8>,
    success: Vec<u8>,
}

fn golden(session: &str) -> Golden {
    let path = format!("{}/../../tests/golden/{session}_login.jsonl", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let recs: Vec<Value> = text.lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    let get = |dir: &str, kind: &str, nth: usize| -> Vec<u8> {
        let v = recs.iter().filter(|r| r["dir"] == dir && r["kind"] == kind).nth(nth).unwrap();
        unhex(v["hex"].as_str().unwrap())
    };
    Golden { sign_in_frame: get("c2s", "raw", 0), plaintext: get("note", "rsa_plain", 0), success: get("s2c", "0x0B", 0) }
}

fn key() -> &'static LoginKey {
    static K: OnceLock<LoginKey> = OnceLock::new();
    K.get_or_init(|| LoginKey::generate_for_tests(1024))
}

fn payload_of(frame_bytes: &[u8]) -> Vec<u8> {
    let mut r = FrameReader::default();
    r.push(frame_bytes);
    let p = r.next_frame().unwrap().expect("one whole frame");
    assert_eq!(r.next_frame().unwrap(), None, "exactly one frame");
    p
}

/// The golden method-1 sign-in payload turned into the method-2 one: `01 02 <SteamID64 LE>` + the same tail.
fn steam_sign_in(g: &Golden, steam_id: u64) -> Vec<u8> {
    let p = payload_of(&g.sign_in_frame);
    assert_eq!(&p[..2], &[0x01, 0x01]);
    let account_len = usize::from(p[2]);
    let mut out = vec![0x01, SIGNIN_STEAM];
    out.extend(steam_id.to_le_bytes());
    out.extend(&p[3 + account_len..]);
    out
}

fn str_field(out: &mut Vec<u8>, s: &[u8]) {
    if s.len() < 0xFF {
        out.push(s.len() as u8);
    } else {
        out.push(0xFF);
        out.extend((s.len() as u16).to_le_bytes());
    }
    out.extend(s);
}

/// `04, u32 len, ticket, u8 2, str e-mail, str password` (the Import panel).
fn import_frame(ticket: &[u8], email: &str, password: &str) -> Vec<u8> {
    let mut p = vec![0x04];
    p.extend((ticket.len() as u32).to_le_bytes());
    p.extend(ticket);
    p.push(2);
    str_field(&mut p, email.as_bytes());
    str_field(&mut p, password.as_bytes());
    p
}

/// `04, u32 len, ticket, u8 1, u8 0x0A, str persona` ("Create new").
fn create_frame(persona: &str) -> Vec<u8> {
    let mut p = vec![0x04, 0, 0, 0, 0, 1, 0x0A];
    str_field(&mut p, persona.as_bytes());
    p
}

/// One account with one stored digest (an account database's check), recording every attempt it saw.
struct Accounts {
    rows: Mutex<Vec<(String, String)>>,
    auto_register: bool,
    banned: Option<&'static str>,
    seen: Mutex<Vec<(String, u8, u8, String, Option<u64>)>>,
}

impl Accounts {
    fn one(name: &str, password: &str) -> Accounts {
        Accounts {
            rows: Mutex::new(vec![(name.into(), sha1_hex(password.as_bytes()))]),
            auto_register: false,
            banned: None,
            seen: Mutex::new(vec![]),
        }
    }
}

impl Authenticator for Accounts {
    fn authenticate(&self, a: &Attempt<'_>) -> Result<Granted, Refusal> {
        self.seen.lock().unwrap().push((a.account.into(), a.signin_type, a.password_type, a.digest.into(), a.steam_id));
        let mut rows = self.rows.lock().unwrap();
        let ok = match rows.iter().find(|(n, _)| n == a.account) {
            Some((_, d)) => d == a.digest,
            None if self.auto_register => {
                rows.push((a.account.into(), a.digest.into()));
                true
            }
            None => false,
        };
        if !ok {
            return Err(Refusal::BAD_CREDENTIALS);
        }
        if self.banned == Some(a.account) {
            return Err(Refusal::banned(13, 90));
        }
        Ok(Granted { session: 0x12345678, lobby: LOBBY })
    }
}

fn steam_handshake<'a>(g: &Golden, auth: &Accounts) -> Handshake<'a> {
    let mut hs = Handshake::new(key(), 1, CHALLENGE);
    let r = hs.on_frame(&steam_sign_in(g, STEAM_ID), auth).unwrap();
    assert_eq!(r.payloads, vec![vec![0x10, 0x04]]);
    assert_eq!(r.outcome, Outcome::Continue);
    hs
}

#[test]
fn steam_sign_in_is_parsed_with_the_steam_id_and_the_same_tail() {
    let g = golden(SESSION);
    let normal = parse_sign_in(&payload_of(&g.sign_in_frame)).unwrap();
    let steam = parse_sign_in(&steam_sign_in(&g, STEAM_ID)).unwrap();
    assert_eq!(steam.signin_type, 2);
    assert_eq!(steam.steam_id, Some(STEAM_ID));
    assert_eq!(steam.account, "");
    assert_eq!(
        (&steam.client_version, &steam.build, steam.start_token, steam.key_crc, steam.tail_flag, steam.tail_value, &steam.rest),
        (&normal.client_version, &normal.build, normal.start_token, normal.key_crc, normal.tail_flag, normal.tail_value, &normal.rest)
    );
    // method 1 still reads the account string and has no SteamID
    assert_eq!((normal.signin_type, normal.steam_id, normal.account.as_str()), (1, None, "test@example.com"));
    // a truncated SteamID is malformed, not misread
    assert!(parse_sign_in(&[0x01, 0x02, 1, 2, 3]).is_err());
}

#[test]
fn steam_sign_in_answers_10_04_and_nothing_reaches_the_authenticator() {
    let g = golden(SESSION);
    let auth = Accounts::one("test@example.com", "secret");
    let hs = steam_handshake(&g, &auth);
    assert!(auth.seen.lock().unwrap().is_empty(), "the SteamID alone never reaches the account check");
    assert_eq!(hs.read_timeout(), IMPORT_READ_TIMEOUT);
}

#[test]
fn import_with_the_right_password_gives_the_normal_0b() {
    let g = golden(SESSION);
    let auth = Accounts::one("test@example.com", "secret");
    let mut hs = steam_handshake(&g, &auth);
    let r = hs.on_frame(&import_frame(&[], "test@example.com", "secret"), &auth).unwrap();
    // byte for byte the 0B of the golden normal sign-in (same session and lobby)
    assert_eq!(r.payloads.len(), 1);
    assert_eq!(frame(&r.payloads[0]), g.success);
    assert_eq!(r.outcome, Outcome::Granted { account: "test@example.com".into(), granted: Granted { session: 0x12345678, lobby: LOBBY } });
    // what the authenticator saw: the name, method 2, the type-1 digest, the version from 01, the claimed SteamID
    let seen = auth.seen.lock().unwrap();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0], ("test@example.com".into(), 2, IMPORT_PASSWORD_TYPE, sha1_hex(b"secret"), Some(STEAM_ID)));
    // finished: nothing more is accepted
    drop(seen);
    assert!(matches!(hs.on_frame(&import_frame(&[], "test@example.com", "secret"), &auth), Err(LoginError::Unexpected { .. })));
    assert_eq!(hs.read_timeout(), READ_TIMEOUT);
}

#[test]
fn a_wrong_password_shows_the_panel_again_and_the_connection_goes_on() {
    let g = golden(SESSION);
    let auth = Accounts::one("test@example.com", "secret");
    let mut hs = steam_handshake(&g, &auth);
    for (email, pw) in [("test@example.com", "wrong"), ("nobody@example.com", "secret"), ("test@example.com", "SECRET")] {
        let r = hs.on_frame(&import_frame(&[], email, pw), &auth).unwrap();
        assert_eq!(r.payloads, vec![vec![0x10, 0x04]], "{email} {pw}");
        assert_eq!(r.outcome, Outcome::Continue);
        assert_eq!(hs.read_timeout(), IMPORT_READ_TIMEOUT);
    }
    // the ticket in front is skipped whatever it holds
    let r = hs.on_frame(&import_frame(&[0xAB; 37], "test@example.com", "secret"), &auth).unwrap();
    assert!(matches!(r.outcome, Outcome::Granted { .. }));
}

#[test]
fn a_new_name_is_registered_by_the_authenticator() {
    let g = golden(SESSION);
    let mut auth = Accounts::one("test@example.com", "secret");
    auth.auto_register = true;
    let mut hs = steam_handshake(&g, &auth);
    let r = hs.on_frame(&import_frame(&[], "newcomer", "hunter2"), &auth).unwrap();
    assert_eq!(r.payloads[0][0], 0x0B);
    assert!(auth.rows.lock().unwrap().contains(&("newcomer".into(), sha1_hex(b"hunter2"))));
}

#[test]
fn a_banned_account_gets_its_0c_and_the_panel_stays_open() {
    let g = golden(SESSION);
    let mut auth = Accounts::one("test@example.com", "secret");
    auth.banned = Some("test@example.com");
    let mut hs = steam_handshake(&g, &auth);
    let r = hs.on_frame(&import_frame(&[], "test@example.com", "secret"), &auth).unwrap();
    assert_eq!(r.payloads, vec![vec![0x0C, 2, 13, 90, 0, 0, 0]]);
    assert_eq!(r.outcome, Outcome::Continue);
    assert_eq!(hs.read_timeout(), IMPORT_READ_TIMEOUT);
}

#[test]
fn create_new_is_refused_with_10_04() {
    let g = golden(SESSION);
    let auth = Accounts::one("test@example.com", "secret");
    let mut hs = steam_handshake(&g, &auth);
    let r = hs.on_frame(&create_frame("Steam Persona"), &auth).unwrap();
    assert_eq!(r.payloads, vec![vec![0x10, 0x04]]);
    assert_eq!(r.outcome, Outcome::Continue);
    assert!(auth.seen.lock().unwrap().is_empty());
    // an Import after it still works
    assert!(matches!(hs.on_frame(&import_frame(&[], "test@example.com", "secret"), &auth).unwrap().outcome, Outcome::Granted { .. }));
}

#[test]
fn import_frames_parse_and_bad_ones_are_errors() {
    let r = parse_import(&import_frame(&[1, 2, 3], "me", "pw")).unwrap();
    assert_eq!(r.ticket_len, 3);
    assert_eq!(r.kind, ImportKind::Import { email: b"me".to_vec(), password: b"pw".to_vec() });
    let r = parse_import(&create_frame("P")).unwrap();
    assert_eq!(r.kind, ImportKind::Create { fixed: 0x0A, name: b"P".to_vec() });
    // a long password uses the FF u16 string form
    let long = "p".repeat(300);
    assert!(matches!(parse_import(&import_frame(&[], "me", &long)).unwrap().kind, ImportKind::Import { password, .. } if password.len() == 300));
    assert!(parse_import(&[0x04, 9, 0, 0, 0, 1]).is_err(), "ticket longer than the frame");
    assert!(parse_import(&[0x04, 0, 0, 0, 0, 3]).is_err(), "unknown sub-request");
    assert!(parse_import(&[0x04, 0, 0, 0, 0, 2, 5, b'a']).is_err(), "cut string");
}

#[test]
fn import_outside_steam_mode_is_a_protocol_error() {
    let g = golden(SESSION);
    let auth = Accounts::one("test@example.com", "secret");
    // before any sign-in
    let mut hs = Handshake::new(key(), 1, CHALLENGE);
    assert!(matches!(hs.on_frame(&import_frame(&[], "test@example.com", "secret"), &auth), Err(LoginError::Unexpected { .. })));
    // after a normal method-1 sign-in
    let mut hs = Handshake::new(key(), 1, CHALLENGE);
    hs.on_frame(&payload_of(&g.sign_in_frame), &auth).unwrap();
    assert!(matches!(hs.on_frame(&import_frame(&[], "test@example.com", "secret"), &auth), Err(LoginError::Unexpected { .. })));
    // secure data in Steam mode
    let mut hs = steam_handshake(&g, &auth);
    assert!(matches!(hs.on_frame(&[0x02, 0, 0, 0, 0], &auth), Err(LoginError::Unexpected { .. })));
}

fn secure_frame(plaintext: &[u8]) -> Vec<u8> {
    let block = encrypt_with_public_der(&key().public_key(), plaintext);
    let mut p = vec![0x02];
    p.extend((block.len() as u32).to_le_bytes());
    p.extend(block);
    p
}

/// The golden plaintext as a TYPED password sends it: password type byte 1 and the SHA-1 hex of the password.
fn typed_plaintext(g: &Golden, password: &str) -> Vec<u8> {
    let pt = parse_plaintext(&g.plaintext).unwrap();
    let mut out = pt.prefix.to_vec();
    out.extend([pt.signin_type, 1]);
    let digest = sha1_hex(password.as_bytes());
    out.push(digest.len() as u8);
    out.extend(digest.as_bytes());
    out.extend(pt.challenge.to_le_bytes());
    out
}

#[test]
fn normal_method_1_sign_in_is_unchanged_and_keeps_the_short_timeout() {
    // remember-me (password type 2 + stored digest, the golden captures) and a typed password (type 1), both 0B
    // byte for byte, with no 10 and no change of the read timeout on the way
    let g = golden(SESSION);
    let remembered = parse_plaintext(&g.plaintext).unwrap();
    assert_eq!(remembered.password_type, 2);
    let auth_remembered = Accounts {
        rows: Mutex::new(vec![("test@example.com".into(), remembered.digest.clone())]),
        auto_register: false,
        banned: None,
        seen: Mutex::new(vec![]),
    };
    let auth_typed = Accounts::one("test@example.com", "secret");
    for (auth, plain) in [(&auth_remembered, g.plaintext.clone()), (&auth_typed, typed_plaintext(&g, "secret"))] {
        let mut hs = Handshake::new(key(), 1, CHALLENGE);
        assert_eq!(hs.read_timeout(), READ_TIMEOUT);
        let r = hs.on_frame(&payload_of(&g.sign_in_frame), auth).unwrap();
        assert_eq!(r.payloads[0][0], 0x0D);
        assert_eq!(hs.read_timeout(), READ_TIMEOUT);
        let r = hs.on_frame(&secure_frame(&plain), auth).unwrap();
        assert_eq!(frame(&r.payloads[0]), g.success);
        let seen = auth.seen.lock().unwrap();
        assert_eq!(seen[0].4, None, "no SteamID on the normal path");
        assert_eq!(seen[0].1, 1);
    }
}

#[test]
fn one_account_signs_in_through_both_paths_with_the_same_password() {
    let g = golden(SESSION);
    let auth = Accounts::one("test@example.com", "correct horse");
    // normal path: the client's digest of the typed password
    let mut hs = Handshake::new(key(), 1, CHALLENGE);
    hs.on_frame(&payload_of(&g.sign_in_frame), &auth).unwrap();
    let normal = hs.on_frame(&secure_frame(&typed_plaintext(&g, "correct horse")), &auth).unwrap();
    // Steam path: the same password in clear
    let mut hs = steam_handshake(&g, &auth);
    let steam = hs.on_frame(&import_frame(&[], "test@example.com", "correct horse"), &auth).unwrap();
    assert_eq!(normal.payloads, steam.payloads);
    let seen = auth.seen.lock().unwrap();
    assert_eq!(seen[0].3, seen[1].3, "the same digest reaches the account check");
    assert_eq!(seen[1].3, client_password_digest(b"correct horse"));
}

#[test]
fn tcp_steam_sign_in_import_retry_and_lobby() {
    let g = golden(SESSION);
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let auth = Accounts::one("test@example.com", "secret");
        let (stream, _) = listener.accept().unwrap();
        handle_connection(stream, key(), 1, &auth).unwrap()
    });
    let mut c = TcpStream::connect(addr).unwrap();
    let mut reader = FrameReader::default();
    let mut buf = [0u8; 512];
    let mut next = |c: &mut TcpStream| loop {
        if let Some(p) = reader.next_frame().unwrap() {
            return p;
        }
        let n = c.read(&mut buf).unwrap();
        assert!(n > 0, "server closed early");
        reader.push(&buf[..n]);
    };
    // both frame forms: the long one for the sign-in (over 255 bytes with its key), the short one for small frames
    let short = |p: &[u8]| {
        let mut f = vec![p.len() as u8];
        f.extend(p);
        f
    };
    c.write_all(&frame(&steam_sign_in(&g, STEAM_ID))).unwrap();
    assert_eq!(next(&mut c), vec![0x10, 0x04]);
    c.write_all(&short(&import_frame(&[], "test@example.com", "nope"))).unwrap();
    assert_eq!(next(&mut c), vec![0x10, 0x04]);
    c.write_all(&frame(&import_frame(&[], "test@example.com", "secret"))).unwrap();
    assert_eq!(frame(&next(&mut c)), g.success);
    assert!(matches!(server.join().unwrap(), Some(Outcome::Granted { .. })));
}
