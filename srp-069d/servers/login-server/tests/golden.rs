//! Login exchange tests against the golden logs (tests/golden/captured_session_N_login.jsonl: sign-ins of the real
//! 0.69d client with the account name, keys, stored digest and random prefix replaced by test values; see
//! tests/golden/README.md). The server key here is generated for the test: the golden ciphertext (made for the key in
//! the golden 0x0D) cannot be used, so the golden PLAINTEXT is encrypted again, as the client does, with the key of
//! 0x0D.

use login_server::frame::{frame, FrameReader};
use login_server::handshake::{parse_plaintext, parse_sign_in, Authenticator};
use login_server::keys::encrypt_with_public_der;
use login_server::*;
use serde_json::Value;
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream};
use std::sync::OnceLock;
use std::io::{Read, Write};

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap()).collect()
}

struct Golden {
    sign_in_frame: Vec<u8>,
    request_secure_data: Vec<u8>,
    secure_frame: Vec<u8>,
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
    Golden {
        sign_in_frame: get("c2s", "raw", 0),
        request_secure_data: get("s2c", "0x0D", 0),
        secure_frame: get("c2s", "raw", 1),
        plaintext: get("note", "rsa_plain", 0),
        success: get("s2c", "0x0B", 0),
    }
}

fn key() -> &'static LoginKey {
    static K: OnceLock<LoginKey> = OnceLock::new();
    K.get_or_init(|| LoginKey::generate_for_tests(1024))
}

fn other_key() -> &'static LoginKey {
    static K: OnceLock<LoginKey> = OnceLock::new();
    K.get_or_init(|| LoginKey::generate_for_tests(1024))
}

const LOBBY: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 1235);
const CHALLENGE: u32 = 0xC0FFEE01; // the challenge of the captured 0x0D, echoed in the golden plaintext
/// The session most tests use.
const LONG: &str = "captured_session_2";

/// Accepts one account with one digest (what an account database does).
struct OneAccount {
    account: &'static str,
    digest: String,
}

impl Authenticator for OneAccount {
    fn authenticate(&self, a: &Attempt<'_>) -> Result<Granted, Refusal> {
        if a.account == self.account && a.digest == self.digest {
            Ok(Granted { session: 0x12345678, lobby: LOBBY })
        } else {
            Err(Refusal::BAD_CREDENTIALS)
        }
    }
}

fn payload_of(frame_bytes: &[u8]) -> Vec<u8> {
    let mut r = FrameReader::default();
    r.push(frame_bytes);
    let p = r.next_frame().unwrap().expect("one whole frame");
    assert_eq!(r.next_frame().unwrap(), None, "exactly one frame");
    p
}

fn secure_frame(public: &rsa::RsaPublicKey, plaintext: &[u8]) -> Vec<u8> {
    let block = encrypt_with_public_der(public, plaintext);
    let mut p = vec![0x02];
    p.extend((block.len() as u32).to_le_bytes());
    p.extend(block);
    p
}

fn auth_for(g: &Golden) -> OneAccount {
    OneAccount { account: "test@example.com", digest: parse_plaintext(&g.plaintext).unwrap().digest }
}

#[test]
fn golden_sign_in_request_is_parsed() {
    for k in 1..=7 {
        let session = format!("captured_session_{k}");
        let g = golden(&session);
        let si = parse_sign_in(&payload_of(&g.sign_in_frame)).unwrap();
        assert_eq!(si.account, "test@example.com", "{session}");
        assert_eq!(si.client_version, "0.69d0");
        assert_eq!(si.build, "r1");
        assert_eq!((si.signin_type, si.tail_flag, si.tail_value), (1, 0xFF, 0x0B));
        // the DER in the golden 0x0D: the client's key CRC field is its CRC-32
        let p = payload_of(&g.request_secure_data);
        let der = &p[7..7 + 140];
        if session == LONG {
            assert_eq!(crc32fast::hash(der), si.key_crc, "key CRC field = CRC-32 of the DER the client holds");
        }
        // plaintext layout
        let pt = parse_plaintext(&g.plaintext).unwrap();
        assert_eq!((pt.signin_type, pt.password_type, pt.digest.len(), pt.challenge), (1, 2, 40, CHALLENGE));
    }
}

#[test]
fn golden_exchange_gives_byte_identical_frames() {
    let g = golden(LONG);
    let auth = auth_for(&g);
    let mut hs = Handshake::new(key(), 2, CHALLENGE);

    // sign-in -> 0D: identical except for the key bytes
    let r = hs.on_frame(&payload_of(&g.sign_in_frame), &auth).unwrap();
    assert_eq!(r.outcome, Outcome::Continue);
    let ours = frame(&r.payloads[0]);
    let want = &g.request_secure_data;
    assert_eq!(ours.len(), want.len());
    assert_eq!(&ours[..12], &want[..12], "frame head, 0D, password type, key flag, DER length");
    assert_eq!(&ours[ours.len() - 4..], &want[want.len() - 4..], "challenge");
    assert_eq!(&ours[12..152], key().public_der());

    // secure data -> 0B: byte for byte
    let public = key().public_key();
    let r = hs.on_frame(&secure_frame(&public, &g.plaintext), &auth).unwrap();
    assert_eq!(frame(&r.payloads[0]), g.success);
    match r.outcome {
        Outcome::Granted { account, granted } => {
            assert_eq!(account, "test@example.com");
            assert_eq!(granted, Granted { session: 0x12345678, lobby: LOBBY });
        }
        o => panic!("{o:?}"),
    }
    // the golden secure frame has the same shape as ours: 02, u32 128, 128 bytes
    assert_eq!(payload_of(&g.secure_frame)[..5], [0x02, 0x80, 0, 0, 0]);
}

fn run_secure(plain: &[u8], encrypt_with: &LoginKey, auth: &OneAccount) -> Reply {
    let g = golden(LONG);
    let mut hs = Handshake::new(key(), 2, CHALLENGE);
    hs.on_frame(&payload_of(&g.sign_in_frame), auth).unwrap();
    hs.on_frame(&secure_frame(&encrypt_with.public_key(), plain), auth).unwrap()
}

#[test]
fn wrong_key_is_refused() {
    let g = golden(LONG);
    let r = run_secure(&g.plaintext, other_key(), &auth_for(&g));
    assert_eq!(r.payloads, vec![vec![0x0C, 4]]);
    assert!(matches!(r.outcome, Outcome::Refused { .. }));
}

#[test]
fn wrong_password_is_refused_and_the_account_is_reported() {
    let g = golden(LONG);
    let auth = OneAccount { account: "test@example.com", digest: "0".repeat(40) };
    let r = run_secure(&g.plaintext, key(), &auth);
    assert_eq!(r.payloads, vec![vec![0x0C, 4]]);
    assert_eq!(r.outcome, Outcome::Refused { account: "test@example.com".into(), reason: Refusal::BAD_CREDENTIALS });
    // wrong account name, right digest
    let auth = OneAccount { account: "someone@else", digest: auth_for(&g).digest };
    assert!(matches!(run_secure(&g.plaintext, key(), &auth).outcome, Outcome::Refused { .. }));
}

#[test]
fn wrong_challenge_and_garbage_are_refused() {
    let g = golden(LONG);
    let auth = auth_for(&g);
    let mut p = g.plaintext.clone();
    let n = p.len();
    p[n - 1] ^= 0x55;
    assert!(matches!(run_secure(&p, key(), &auth).outcome, Outcome::Refused { .. }));
    // plaintext of the wrong shape
    assert!(matches!(run_secure(&p[..30], key(), &auth).outcome, Outcome::Refused { .. }));
    // a block of random bytes
    let mut hs = Handshake::new(key(), 2, CHALLENGE);
    hs.on_frame(&payload_of(&g.sign_in_frame), &auth).unwrap();
    let mut junk = vec![0x02, 0x80, 0, 0, 0];
    junk.extend((0..128u8).map(|i| i.wrapping_mul(37)));
    assert!(matches!(hs.on_frame(&junk, &auth).unwrap().outcome, Outcome::Refused { .. }));
}

#[test]
fn protocol_violations_are_errors() {
    let g = golden(LONG);
    let auth = auth_for(&g);
    let mut hs = Handshake::new(key(), 2, CHALLENGE);
    assert!(matches!(hs.on_frame(&[0x02, 0, 0, 0, 0], &auth), Err(LoginError::Unexpected { .. })));
    assert_eq!(hs.on_frame(&[], &auth), Err(LoginError::Empty));
    assert!(matches!(hs.on_frame(&[0x01, 1, 200], &auth), Err(LoginError::Malformed(_))));
}

#[test]
fn tcp_end_to_end() {
    let g = golden(LONG);
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let auth = auth_for(&g);
    let server = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        handle_connection(stream, key(), 2, &auth).unwrap()
    });
    let mut c = TcpStream::connect(addr).unwrap();
    c.write_all(&g.sign_in_frame).unwrap();
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
    let request = next(&mut c);
    assert_eq!(request[0], 0x0D);
    let der_len = u32::from_le_bytes(request[3..7].try_into().unwrap()) as usize;
    assert_eq!(der_len, 140);
    let challenge = u32::from_le_bytes(request[7 + der_len..].try_into().unwrap());
    // the client answers with the challenge it was given
    let mut plain = g.plaintext.clone();
    let n = plain.len();
    plain[n - 4..].copy_from_slice(&challenge.to_le_bytes());
    c.write_all(&frame(&secure_frame(&key().public_key(), &plain))).unwrap();
    let ok = next(&mut c);
    assert_eq!(frame(&ok), g.success);
    assert!(matches!(server.join().unwrap(), Some(Outcome::Granted { .. })));
}

#[test]
fn refusal_payloads_follow_the_client_reader() {
    // 0x20011F140: code, and u8 reason + u32 value only for codes 2 and 3
    assert_eq!(Refusal::BAD_CREDENTIALS.payload(), vec![0x0C, 4]);
    assert_eq!(Refusal::ATTEMPT_INTERVAL.payload(), vec![0x0C, 5]);
    assert_eq!(Refusal::ALREADY_ONLINE.payload(), vec![0x0C, 7]);
    assert_eq!(Refusal::INVALID_VERSION.payload(), vec![0x0C, 8]);
    assert_eq!(Refusal::banned(13, 90).payload(), vec![0x0C, 2, 13, 90, 0, 0, 0]);
    assert_eq!(Refusal::suspended(5, 0x0102).payload(), vec![0x0C, 3, 5, 2, 1, 0, 0]);
}
