//! `/replays/<token>` against the hostile requests of `CONTRACT-R1`
//! ("anyone sending crafted HTTP requests by hand"), over a real socket and
//! the page's own request reader, with private files planted beside the
//! replay: a recording, its sidecar, a database, an `.env`. Only the replay
//! ever comes back, and only for a live token; every other answer is the
//! same "not found". Then every other route with the same list.

use std::net::{TcpListener, TcpStream};

use session::v026g::GameAccounts;
use session::v026g::replays::{ReplaySettings, Sidecar, TOKEN_DOWNLOADS};

use super::*;

const MATCH: u32 = 0x1F07_C8FB;
const REPLAY: &[u8] = b"REPLAY BYTES";
/// In every private file planted; never in an answer.
const SECRET: &str = "SECRET";

struct Quiet;

impl LobbyLink for Quiet {
    fn account_changed(&self, _: u32) {}
    fn set_admin_text(&self, _: &str, _: &str) {}
    fn clear_admin_text(&self) {}
    fn admin_text(&self) -> Option<(String, String)> {
        None
    }
}

impl MatchCounter for Quiet {
    fn active_matches(&self) -> usize {
        0
    }
    fn live_matches(&self) -> Vec<LiveMatch> {
        Vec::new()
    }
}

/// A replay folder with the replay of `MATCH` and private files around it.
struct Planted {
    root: PathBuf,
    replays: Arc<Replays>,
}

impl Drop for Planted {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn planted(name: &str) -> Planted {
    let root = std::env::temp_dir().join(format!(
        "srp-web-replays-{name}-{}-{}",
        std::process::id(),
        Replays::now_unix()
    ));
    let _ = std::fs::remove_dir_all(&root);
    let settings = ReplaySettings {
        replay_dir: root.join("replays"),
        record_dir: root.join("recordings"),
        modes_dir: root.join("modes"),
        max_recording_bytes: 1 << 20,
        max_replay_bytes: 1 << 20,
    }
    .checked()
    .unwrap();
    std::fs::create_dir_all(&settings.modes_dir).unwrap();
    // The replay's game was of 12: given whatever the last row of
    // simulation_changes.tsv is (the refusals have their own tests).
    let replays = Arc::new(Replays::new(Some(settings.clone()), 6).with_oldest_revision(11));
    std::fs::write(replays.sur_path(MATCH).unwrap(), REPLAY).unwrap();
    let sidecar = Sidecar {
        match_id: MATCH,
        size: REPLAY.len() as u64,
        sha256: "ab".repeat(32),
        revisions: vec![Some(12)],
        account_ids: Vec::new(),
        made_at: 0,
    };
    std::fs::write(replays.json_path(MATCH).unwrap(), sidecar.to_json()).unwrap();
    let secret = |path: PathBuf| std::fs::write(path, format!("{SECRET} file")).unwrap();
    secret(
        settings
            .record_dir
            .join(format!("rec026g_20261015-120000_{MATCH:08x}.srprec")),
    );
    secret(settings.replay_dir.join("srp.db"));
    secret(settings.replay_dir.join("00000002.json"));
    secret(settings.modes_dir.join("level_01_day_2.options"));
    secret(root.join(".env"));
    secret(root.join("srp.db"));
    Planted { root, replays }
}

fn web(replays: Option<Arc<Replays>>) -> Web {
    Web {
        store: Arc::new(SessionStore::new()),
        token: Some("tok".into()),
        admin_path: Some("/hidden".into()),
        admin_ips: vec![Net::parse("127.0.0.1").unwrap()],
        trust_proxy: false,
        token_failures: Mutex::new((0, None)),
        connections: AtomicU32::new(0),
        downloads: AtomicU32::new(0),
        resource_downloads: AtomicU32::new(0),
        replay_downloads: AtomicU32::new(0),
        replay_failures: Mutex::new(HashMap::new()),
        game: Some(Game {
            accounts: Arc::new(GameAccounts::new()),
            lobby: Arc::new(Quiet),
            matches: Arc::new(Quiet),
            patches: Vec::new(),
            replays,
        }),
    }
}

/// A live token of `account` for `MATCH`, as the lobby gives it.
fn token(replays: &Replays, account: u32) -> String {
    let now = Instant::now();
    replays.ask(account, MATCH, Some(14), None, now);
    replays.answer(account, now).ready.expect("ready").token
}

/// `raw` sent to the page as it is, over a real connection: the answer.
fn exchange(web: &Web, raw: &[u8]) -> Vec<u8> {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let raw = raw.to_vec();
    let client = thread::spawn(move || {
        let mut stream = TcpStream::connect(address).unwrap();
        let _ = stream.write_all(&raw);
        let mut got = Vec::new();
        let _ = stream.read_to_end(&mut got);
        got
    });
    let (stream, _) = listener.accept().unwrap();
    web.serve(stream);
    client.join().unwrap()
}

fn get(web: &Web, method: &str, target: &str) -> Vec<u8> {
    exchange(
        web,
        format!("{method} {target} HTTP/1.1\r\nHost: skhid.test\r\n\r\n").as_bytes(),
    )
}

fn holds(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|part| part == needle)
}

/// Every target of the contract's list, made from a live `token`.
fn hostile_targets(token: &str) -> Vec<String> {
    let upper = token.to_ascii_uppercase();
    let id = format!("{MATCH:08x}");
    vec![
        "/replays".into(),
        "/replays/".into(),
        "/replays//".into(),
        format!("/replays/{}", &token[1..]),
        format!("/replays/{token}0"),
        format!("/replays/{upper}"),
        format!("/replays/{token}/"),
        format!("/replays/{token}.sur"),
        format!("/replays/{token}.json"),
        format!("/replays/{token}?"),
        format!("/replays/{token}?lang=en"),
        format!("/replays/{token}?file=../{id}.json"),
        format!("/replays/{token}#x"),
        format!("/replays//{token}"),
        format!("/replays/./{token}"),
        format!("/replays/../replays/{token}"),
        format!("/replays/%2e%2e/replays/{token}"),
        format!("/replays/{}%2f", &token[..29]),
        format!("/replays/{}%5c", &token[..29]),
        format!("/replays/{}\\", &token[..31]),
        format!("/replays/{}\\..", &token[..29]),
        format!("/replays/{}%00", &token[..29]),
        format!("/replays/{}\0", &token[..31]),
        format!("/replays/{id}"),
        format!("/replays/{id}.sur"),
        format!("/replays/{id}.json"),
        format!("/replays/{MATCH}"),
        "/replays/../recordings".into(),
        "/replays/..%2f..%2f.env".into(),
        "/replays/%2e%2e%2fsrp.db".into(),
        format!("/replays/rec026g_20261015-120000_{id}.srprec"),
        "/replays/srp.db".into(),
        "/replays/00000002.json".into(),
        "/replays/level_01_day_2.options".into(),
        "/replays/0123456789abcdef0123456789abcdef".into(),
        format!("/replays/{}", "a".repeat(2000)),
        format!("/replays/{token}/{token}"),
        format!("/replays/ {token}"),
    ]
}

#[test]
fn a_token_is_the_whole_target_and_nothing_else_names_a_file() {
    let token = "fedcba9876543210fedcba9876543210";
    assert_eq!(
        replay_token("GET", &format!("/replays/{token}")),
        Some(token)
    );
    assert_eq!(
        replay_token("HEAD", &format!("/replays/{token}")),
        Some(token)
    );
    for method in ["POST", "PUT", "DELETE", "OPTIONS", "get", "TRACE", "PATCH"] {
        assert_eq!(
            replay_token(method, &format!("/replays/{token}")),
            None,
            "{method}"
        );
    }
    // Every hostile target but a well-formed token no lookup knows.
    let unknown = "/replays/0123456789abcdef0123456789abcdef";
    for target in hostile_targets(token) {
        let expected = (target == unknown).then(|| &target[REPLAYS.len()..]);
        assert_eq!(replay_token("GET", &target), expected, "{target:?}");
    }
}

#[test]
fn a_live_token_gets_the_replay_whole_by_head_or_by_range() {
    let planted = planted("live");
    let web = web(Some(planted.replays.clone()));
    let token = token(&planted.replays, 1);
    let whole = String::from_utf8(get(&web, "GET", &format!("/replays/{token}"))).unwrap();
    assert_eq!(
        whole,
        format!(
            "HTTP/1.0 200 OK\r\nContent-Type: application/octet-stream\r\n\
             Content-Disposition: attachment; filename=\"{MATCH:08x}.sur\"\r\n\
             Accept-Ranges: bytes\r\nContent-Length: 12\r\n\
             X-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\nREPLAY BYTES"
        )
    );
    let head = get(&web, "HEAD", &format!("/replays/{token}"));
    assert!(head.starts_with(b"HTTP/1.0 200 OK\r\n"));
    assert!(head.ends_with(b"Connection: close\r\n\r\n"));
    let part = exchange(
        &web,
        format!("GET /replays/{token} HTTP/1.1\r\nRange: bytes=7-\r\n\r\n").as_bytes(),
    );
    let part = String::from_utf8(part).unwrap();
    assert!(
        part.starts_with("HTTP/1.0 206 Partial Content\r\n"),
        "{part}"
    );
    assert!(part.contains("Content-Range: bytes 7-11/12\r\n") && part.ends_with("BYTES"));
    // Its slots are its own, and they come back.
    assert_eq!(web.replay_downloads.load(Ordering::Acquire), 0);
    assert_eq!(web.downloads.load(Ordering::Acquire), 0);
}

#[test]
fn every_hostile_request_gets_the_same_not_found_and_nothing_private() {
    let planted = planted("hostile");
    let web = web(Some(planted.replays.clone()));
    let token = token(&planted.replays, 1);
    let mut asked = 0;
    for target in hostile_targets(&token) {
        for method in ["GET", "HEAD"] {
            // Counted failures are not what this test is about.
            locked(&web.replay_failures).clear();
            let answer = get(&web, method, &target);
            assert_eq!(answer, REPLAY_NOT_FOUND, "{method} {target:?}");
            asked += 1;
        }
    }
    // Other methods, with the live token itself.
    for method in ["POST", "PUT", "DELETE", "OPTIONS", "get", "PATCH"] {
        locked(&web.replay_failures).clear();
        let answer = get(&web, method, &format!("/replays/{token}"));
        assert_eq!(answer, REPLAY_NOT_FOUND, "{method}");
    }
    // An overlong request is not read at all: nothing comes back.
    let long = get(&web, "GET", &format!("/replays/{}", "a".repeat(5000)));
    assert!(long.is_empty() || long == REPLAY_NOT_FOUND);
    // Not one of them used up a download of the token.
    for _ in 0..TOKEN_DOWNLOADS {
        let answer = get(&web, "GET", &format!("/replays/{token}"));
        assert!(answer.ends_with(REPLAY), "{asked}");
    }
    assert_eq!(
        get(&web, "GET", &format!("/replays/{token}")),
        REPLAY_NOT_FOUND
    );
}

#[test]
fn a_token_ends_with_its_downloads_and_is_its_own_matchs_only() {
    let here = planted("used");
    let other = planted("other");
    let web = web(Some(here.replays.clone()));
    let used = token(&here.replays, 1);
    let elsewhere = token(&other.replays, 1);
    for _ in 0..TOKEN_DOWNLOADS {
        assert!(get(&web, "GET", &format!("/replays/{used}")).ends_with(REPLAY));
    }
    // HEAD takes none, but a used-up token gets nothing either way.
    assert_eq!(
        get(&web, "GET", &format!("/replays/{used}")),
        REPLAY_NOT_FOUND
    );
    assert_eq!(
        get(&web, "HEAD", &format!("/replays/{used}")),
        REPLAY_NOT_FOUND
    );
    // A token of another server (one this one never gave): not found.
    assert_eq!(
        get(&web, "GET", &format!("/replays/{elsewhere}")),
        REPLAY_NOT_FOUND
    );
}

#[test]
fn only_a_regular_file_of_the_tokens_match_in_the_replay_folder_is_served() {
    let planted = planted("files");
    let web = web(Some(planted.replays.clone()));
    let sur = planted.replays.sur_path(MATCH).unwrap();
    // The file is gone: the token is good, nothing is served.
    let token_a = token(&planted.replays, 1);
    std::fs::remove_file(&sur).unwrap();
    assert_eq!(
        get(&web, "GET", &format!("/replays/{token_a}")),
        REPLAY_NOT_FOUND
    );
    // A folder in its place.
    std::fs::create_dir(&sur).unwrap();
    assert_eq!(
        get(&web, "GET", &format!("/replays/{token_a}")),
        REPLAY_NOT_FOUND
    );
    std::fs::remove_dir(&sur).unwrap();
    // A link in its place, to a private file.
    #[cfg(unix)]
    {
        let recording = std::fs::read_dir(&planted.replays.settings().unwrap().record_dir)
            .unwrap()
            .flatten()
            .next()
            .unwrap()
            .path();
        std::os::unix::fs::symlink(&recording, &sur).unwrap();
        let answer = get(&web, "GET", &format!("/replays/{token_a}"));
        assert_eq!(answer, REPLAY_NOT_FOUND);
        std::fs::remove_file(&sur).unwrap();
    }
    std::fs::write(&sur, REPLAY).unwrap();
    assert!(get(&web, "GET", &format!("/replays/{token_a}")).ends_with(REPLAY));
}

#[test]
fn failed_lookups_of_an_address_are_answered_429_for_a_minute() {
    let planted = planted("limit");
    let web = web(Some(planted.replays.clone()));
    let token = token(&planted.replays, 1);
    let request = |target: &str| Request {
        method: "GET".into(),
        path: target.into(),
        target: target.into(),
        lang: Lang::En,
        forwarded: String::new(),
        range: String::new(),
        client: Some("203.0.113.9".parse().unwrap()),
        form: Vec::new(),
    };
    let answer = |request: &Request, now: Instant| -> Vec<u8> {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let client = thread::spawn(move || {
            let mut stream = TcpStream::connect(address).unwrap();
            let mut got = Vec::new();
            let _ = stream.read_to_end(&mut got);
            got
        });
        let (mut stream, _) = listener.accept().unwrap();
        web.replay(&mut stream, request, now);
        drop(stream);
        client.join().unwrap()
    };
    let now = Instant::now();
    let wrong = request(&format!("/replays/{}", "0".repeat(32)));
    for _ in 0..REPLAY_FAILURES {
        assert_eq!(answer(&wrong, now), REPLAY_NOT_FOUND);
    }
    let blocked = answer(&request(&format!("/replays/{token}")), now);
    assert!(blocked.starts_with(b"HTTP/1.0 429 Too Many Requests\r\nRetry-After: 60\r\n"));
    // Another address is not.
    let mut other = request(&format!("/replays/{token}"));
    other.client = Some("203.0.113.10".parse().unwrap());
    assert!(answer(&other, now).ends_with(REPLAY));
    // A minute later the address is heard again.
    let later = now + REPLAY_BLOCK;
    assert!(answer(&request(&format!("/replays/{token}")), later).ends_with(REPLAY));
    // Failures spread over more than a minute do not add up.
    let mut at = later;
    for _ in 0..3 * REPLAY_FAILURES {
        at += Duration::from_secs(3);
        assert_eq!(answer(&wrong, at), REPLAY_NOT_FOUND);
    }
}

#[test]
fn a_token_downloads_one_at_a_time_and_many_tokens_share_the_file() {
    let planted = planted("one-at-a-time");
    let web = web(Some(planted.replays.clone()));
    let token = token(&planted.replays, 1);
    // Its download under way (as `Web::replay` holds it while it sends):
    // another, GET or HEAD, is told to come back, takes none of its
    // downloads and is no failed lookup.
    let held = planted
        .replays
        .take_download(&token, true, Instant::now())
        .unwrap();
    for _ in 0..2 * REPLAY_FAILURES {
        for method in ["GET", "HEAD"] {
            assert_eq!(get(&web, method, &format!("/replays/{token}")), REPLAY_BUSY);
        }
    }
    assert!(locked(&web.replay_failures).is_empty());
    drop(held);
    // Four downloads left.
    for _ in 1..TOKEN_DOWNLOADS {
        assert!(get(&web, "GET", &format!("/replays/{token}")).ends_with(REPLAY));
    }
    assert_eq!(
        get(&web, "GET", &format!("/replays/{token}")),
        REPLAY_NOT_FOUND
    );
    // Five players' tokens of one replay, downloading at once: each gets
    // the whole file, a slot each.
    let tokens: Vec<String> = (10..10 + MAX_DOWNLOADS)
        .map(|account| self::token(&planted.replays, account))
        .collect();
    let held: Vec<_> = tokens
        .iter()
        .map(|token| {
            planted
                .replays
                .take_download(token, false, Instant::now())
                .unwrap()
        })
        .collect();
    drop(held);
    let answers: Vec<Vec<u8>> = thread::scope(|scope| {
        let running: Vec<_> = tokens
            .iter()
            .map(|token| {
                let web = &web;
                scope.spawn(move || get(web, "GET", &format!("/replays/{token}")))
            })
            .collect();
        running
            .into_iter()
            .map(|thread| thread.join().unwrap())
            .collect()
    });
    for answer in answers {
        assert!(answer.starts_with(b"HTTP/1.0 200 OK\r\n"));
        assert!(answer.ends_with(REPLAY));
    }
    assert_eq!(web.replay_downloads.load(Ordering::Acquire), 0);
}

#[test]
fn no_logged_value_of_a_request_can_start_a_line() {
    // A `Range` with a bare `\n` and other control characters, read by the
    // page's own reader: the header split keeps them in the value.
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let raw = format!(
        "GET /replays/{} HTTP/1.1\r\nRange: bytes=0-\nINFO forged line\r\u{1b}[2J\0\r\n\r\n",
        "0".repeat(32)
    );
    let client = thread::spawn(move || {
        let mut stream = TcpStream::connect(address).unwrap();
        stream.write_all(raw.as_bytes()).unwrap();
        stream
    });
    let (mut stream, _) = listener.accept().unwrap();
    let mut request = read_request(&mut stream).unwrap();
    drop(client.join().unwrap());
    assert!(request.range.contains('\n'));
    request.client = Some("203.0.113.9".parse().unwrap());
    let line = replay_log_line(&request, MATCH, &"ab".repeat(16));
    assert!(!line.chars().any(char::is_control), "{line:?}");
    assert!(
        line.contains(r"bytes=0-\nINFO forged line\r\u{1b}[2J\0"),
        "{line}"
    );
    assert!(line.contains("token ababab…"), "{line}");
    assert!(!line.contains(&"ab".repeat(4)), "{line}");
}

#[test]
fn replays_off_and_the_0_21d_page_serve_none() {
    let planted = planted("off");
    let token = token(&planted.replays, 1);
    let off = web(None);
    assert_eq!(
        get(&off, "GET", &format!("/replays/{token}")),
        REPLAY_NOT_FOUND
    );
    let mut old = web(Some(planted.replays.clone()));
    old.game = None;
    let answer = get(&old, "GET", &format!("/replays/{token}"));
    assert!(answer.starts_with(b"HTTP/1.0 404 Not Found\r\n"));
    assert!(!holds(&answer, REPLAY));
}

/// Every other route of the page, with the same hostile list after it:
/// none reads a file a request names, so none answers with a private file
/// (or the replay). `/download` and `/resources/` serve only the
/// configured archive and the 64-hex blobs beside it (their own tests);
/// with no archive here they are "not found" pages.
#[test]
fn no_other_route_reaches_a_private_file() {
    let planted = planted("routes");
    let web = web(Some(planted.replays.clone()));
    let token = token(&planted.replays, 1);
    let blob = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
    let routes = [
        "/".to_owned(),
        "/download".to_owned(),
        format!("/resources/{blob}"),
        "/api/stats".to_owned(),
        "/patches".to_owned(),
        "/api/client-files".to_owned(),
        "/launcher".to_owned(),
        "/launcher/places/en".to_owned(),
        "/launcher/fresh/uk".to_owned(),
        "/exchange".to_owned(),
        "/hidden".to_owned(),
    ];
    let id = format!("{MATCH:08x}");
    let tails = [
        String::new(),
        "/".into(),
        "/..".into(),
        "/../replays/".to_owned() + &id + ".sur",
        "/../../.env".into(),
        "/..%2f..%2f.env".into(),
        "/%2e%2e/%2e%2e/srp.db".into(),
        "\\..\\srp.db".into(),
        "%5c..%5csrp.db".into(),
        "//srp.db".into(),
        format!("?file=../replays/{id}.sur"),
        format!("?token={token}"),
        "%00".into(),
        "\0".into(),
        format!("/{token}"),
        format!("/{id}.json"),
        format!("/rec026g_20261015-120000_{id}.srprec"),
    ];
    for route in &routes {
        for tail in &tails {
            for method in ["GET", "HEAD", "POST"] {
                let target = format!("{route}{tail}");
                let answer = get(&web, method, &target);
                assert!(!holds(&answer, SECRET.as_bytes()), "{method} {target:?}");
                assert!(!holds(&answer, REPLAY), "{method} {target:?}");
                assert!(
                    !holds(&answer, b"account_ids") && !holds(&answer, b"sha256\":"),
                    "{method} {target:?}"
                );
            }
        }
    }
}
