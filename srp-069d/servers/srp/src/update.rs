//! The Steam launcher's update check (docs/notes/steam.md): a tiny HTTP/1.0 responder, always on.
//!
//! Steam's Play button starts `survarium_launcher.exe`. Its Play button turns on only after the updater
//! (`survarium_updater.exe`) reports "latest version" (launcher jump table 0x20000C118: updater state 18), and the updater
//! asks the `update_server` of launcher.cfg for that. The official host is gone, so a player's per-user launcher.cfg
//! (`%LOCALAPPDATA%\Survarium-Steam\launcher.cfg`, written by `scripts/steam-play/steam-play-setup.cmd`) points
//! `update_server` here. What the updater sends and accepts (0.69d `survarium_updater.exe`):
//!
//! * `update_server` = `host[:port][/path]`, split at the last `:` (0x20002B520..0x20002B57F): the text after it, at most
//!   15 bytes, is the port; no `:` = port 80. So `host:port` works, but then without a path (the path would end up in the
//!   port). The path is the text from the first `/` (0x20002C879), "" without one.
//! * The request (`http_client::get` 0x200007890): `GET <path><target> HTTP/1.0`, `Host: <host>`, `Accept: */*`,
//!   `Connection: close`. Targets: `/download_type.php` (0x20002C8E1, sent first when the launcher gave no download type)
//!   and `/?update_type=<update|full_update>&version=<v>&download_type=<http|torrent>&public_key` (0x20002DB05).
//! * The reply: status line `HTTP/...` with status 200 (206 and 416 also pass, 0x2000095EB..0x20000966F); headers read
//!   line by line up to the empty line, `Content-Length:` case-sensitive (0x200005BF7); the body is everything after.
//! * `download_type.php`: the body compared as a whole with `http` (0x20002D6CB) -> HTTP download type; anything else ->
//!   torrent. We answer `http`: the "latest version" path then starts no torrent seeding (0x20002EBC5..0x20002EBE2 seeds
//!   only for the torrent type).
//! * The version request: the body (before its last `;`, if any) compared as a whole with `<you use the latest version!>` (0x20002EAD1)
//!   -> state 18 -> Play on. The updater asks again every 30 s while the launcher is open (timer 0x20002EB48).
//!   An HTTP error retries every 3 s for ever (0x20002DC50).
//!
//! The same port serves one more read-only file for players without Steam: `GET /skhid/client.txt` ([`CLIENT_TXT_PATH`],
//! [`client_txt`]), the login address and port that `scripts/player/skhid-setup.ps1` writes into the player's start file
//! `Skhid.cmd`. Nothing secret. No login key: the game takes it from the login server's `0D` at every sign-in.
//!
//! Anything else gets 404 / 405 / 400. Every reply is `HTTP/1.0` with `Content-Length` and the connection closed.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The responder's port when `network.update` is "" (the address is the login address's).
pub const DEFAULT_PORT: u16 = 65001;
/// The version reply the updater turns into "latest version" (0x200221B60).
pub const LATEST: &str = "<you use the latest version!>";
/// The download type reply for the HTTP type (0x20002D6D5 compares with "http").
pub const DOWNLOAD_TYPE_HTTP: &str = "http";
/// The player start data's path (the player's setup script asks `http://<server>:<update port>/skhid/client.txt`).
pub const CLIENT_TXT_PATH: &str = "/skhid/client.txt";
/// The start data's format number; the player's script refuses another one (a newer server needs a newer script).
pub const CLIENT_TXT_FORMAT: u32 = 1;

const MAX_HEAD: usize = 8192;
const IO_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_CONNECTIONS: usize = 64;
/// One info line per client address per this long; repeats (the 30-s re-check) go to debug.
const LOG_EVERY: Duration = Duration::from_secs(600);

/// What a request asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    /// `GET <path>/download_type.php`
    DownloadType,
    /// `GET <path>/?update_type=..&version=..&download_type=..`
    Version { update_type: String, version: String, download_type: String },
    /// `GET /skhid/client.txt`: the player start data.
    ClientTxt,
    NotFound,
    BadMethod,
    BadRequest,
}

/// Parse the request head (the bytes up to and including the empty line).
pub fn parse(head: &[u8]) -> Request {
    let line_end = head.iter().position(|&b| b == b'\n').unwrap_or(head.len());
    let Ok(line) = std::str::from_utf8(&head[..line_end]) else { return Request::BadRequest };
    let mut parts = line.trim_end_matches('\r').split(' ').filter(|p| !p.is_empty());
    let (Some(method), Some(target), Some(version)) = (parts.next(), parts.next(), parts.next()) else {
        return Request::BadRequest;
    };
    if !version.starts_with("HTTP/") || parts.next().is_some() {
        return Request::BadRequest;
    }
    if method != "GET" {
        return Request::BadMethod;
    }
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    if path == CLIENT_TXT_PATH {
        return Request::ClientTxt;
    }
    if path.ends_with("/download_type.php") {
        return Request::DownloadType;
    }
    if path.ends_with('/') {
        let mut fields: HashMap<&str, &str> = HashMap::new();
        for pair in query.split('&') {
            let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
            fields.entry(k).or_insert(v);
        }
        if let Some(update_type) = fields.get("update_type") {
            let field = |k: &str| fields.get(k).map(|v| printable(v)).unwrap_or_default();
            return Request::Version { update_type: printable(update_type), version: field("version"), download_type: field("download_type") };
        }
    }
    Request::NotFound
}

/// A query value for the log: printable ASCII only, at most 40 characters.
fn printable(v: &str) -> String {
    v.chars().filter(|c| c.is_ascii_graphic()).take(40).collect()
}

/// The player start data (`GET /skhid/client.txt`): `key=value` lines, LF, ASCII. `login_host` = the address players use
/// ([`crate::Config::player_host`]; empty when every address is 0.0.0.0: the script then keeps the address the player
/// typed), `login_port` = the login port. `scripts/player/skhid-setup.ps1` reads exactly these keys and checks each value.
/// No login key: the game has no argument for one and takes the server's key from `0D` at every sign-in.
pub fn client_txt(login_host: Option<&str>, login_port: u16) -> String {
    format!("format={CLIENT_TXT_FORMAT}\nlogin_host={}\nlogin_port={login_port}\n", login_host.unwrap_or(""))
}

/// The whole reply for a request. `client` = the start data ([`client_txt`]); None = 404 for it.
pub fn reply(r: &Request, client: Option<&str>) -> Vec<u8> {
    let (status, body) = match r {
        Request::DownloadType => ("200 OK", DOWNLOAD_TYPE_HTTP),
        Request::Version { .. } => ("200 OK", LATEST),
        Request::ClientTxt => match client {
            Some(c) => ("200 OK", c),
            None => ("404 Not Found", "not found"),
        },
        Request::NotFound => ("404 Not Found", "not found"),
        Request::BadMethod => ("405 Method Not Allowed", "GET only"),
        Request::BadRequest => ("400 Bad Request", "bad request"),
    };
    format!("HTTP/1.0 {status}\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).into_bytes()
}

/// Read the request head: up to the empty line, at most [`MAX_HEAD`] bytes. None = nothing usable arrived.
fn read_head(s: &mut TcpStream) -> Option<Vec<u8>> {
    let mut head = Vec::new();
    let mut buf = [0u8; 1024];
    loop {
        if head.windows(4).any(|w| w == b"\r\n\r\n") || head.windows(2).any(|w| w == b"\n\n") {
            return Some(head);
        }
        if head.len() >= MAX_HEAD {
            return Some(head);
        }
        match s.read(&mut buf) {
            Ok(0) | Err(_) => return (!head.is_empty()).then_some(head),
            Ok(n) => head.extend_from_slice(&buf[..n]),
        }
    }
}

/// Rate limit for the info log lines.
#[derive(Default)]
struct Seen(Mutex<HashMap<IpAddr, Instant>>);

impl Seen {
    fn first_in_a_while(&self, ip: IpAddr) -> bool {
        let mut m = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if m.len() > 4096 {
            m.clear();
        }
        let now = Instant::now();
        match m.get(&ip) {
            Some(t) if now.duration_since(*t) < LOG_EVERY => false,
            _ => {
                m.insert(ip, now);
                true
            }
        }
    }
}

fn handle(mut s: TcpStream, peer: SocketAddr, seen: &Seen, client: Option<&str>) {
    let _ = s.set_read_timeout(Some(IO_TIMEOUT));
    let _ = s.set_write_timeout(Some(IO_TIMEOUT));
    let Some(head) = read_head(&mut s) else { return };
    let r = parse(&head);
    let _ = s.write_all(&reply(&r, client));
    let _ = s.flush();
    let _ = s.shutdown(std::net::Shutdown::Write);
    let level = match r {
        Request::DownloadType | Request::Version { .. } if seen.first_in_a_while(peer.ip()) => log::Level::Info,
        Request::DownloadType | Request::Version { .. } => log::Level::Debug,
        _ => log::Level::Info,
    };
    match &r {
        Request::DownloadType => log::log!(level, "update check from {peer}: download_type.php -> \"{DOWNLOAD_TYPE_HTTP}\""),
        Request::Version { update_type, version, download_type } => log::log!(
            level,
            "update check from {peer}: {update_type} version {version:?} ({download_type}) -> \"{LATEST}\" (the launcher's Play button turns on; it asks again every 30 s)"
        ),
        Request::ClientTxt if client.is_some() => {
            log::log!(level, "player start data {CLIENT_TXT_PATH} sent to {peer} (skhid-setup writes its Skhid.cmd from it)")
        }
        other => {
            let line = String::from_utf8_lossy(&head[..head.iter().position(|&b| b == b'\n').unwrap_or(head.len()).min(120)]).trim_end().to_string();
            log::log!(level, "update responder: {other:?} from {peer} ({line:?})");
        }
    }
}

/// Serve the update check on `listener` until the process ends: one thread per connection, at most 64 at once.
/// `client` = the player start data for [`CLIENT_TXT_PATH`] ([`client_txt`]); None answers it 404.
pub fn serve(listener: TcpListener, client: Option<String>) {
    let client: Option<Arc<str>> = client.map(Arc::from);
    let seen = Arc::new(Seen::default());
    let active = Arc::new(AtomicUsize::new(0));
    for conn in listener.incoming() {
        match conn {
            Ok(s) => {
                let Ok(peer) = s.peer_addr() else { continue };
                if active.load(Ordering::Relaxed) >= MAX_CONNECTIONS {
                    log::warn!("update responder: {MAX_CONNECTIONS} connections open, dropping {peer}");
                    continue;
                }
                active.fetch_add(1, Ordering::Relaxed);
                let (seen, active, client) = (seen.clone(), active.clone(), client.clone());
                std::thread::spawn(move || {
                    handle(s, peer, &seen, client.as_deref());
                    active.fetch_sub(1, Ordering::Relaxed);
                });
            }
            Err(e) => {
                log::warn!("update responder: accept: {e}");
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }
}

/// The per-user launcher.cfg the player's setup script writes (`scripts/steam-play/steam-play-setup.ps1` builds the same
/// lines). The launcher reads it after the install's launcher.cfg with the same key callback (0x20001487C), so each key
/// here replaces that key only. Reader limits (0x200014180): a line over 259 UTF-16 units is cut, a line without a space
/// ends the file, `arguments` holds at most 1023 units (0x200014732). No login key: the 0.69d game has no argument for
/// one (it stops at an unknown argument), and the login server sends its key in `0D` at every sign-in.
///
/// `language` None (the default): no `-localization` here, so the launcher's own language decides (it appends
/// ` -localization=<word>` after `arguments`, 0x20000F104). `Some(word)` comes first, and the game keeps the first value
/// of a repeated flag (0x200089A80), so the override wins.
pub fn launcher_cfg_lines(host: &str, login_port: u16, update_host: &str, update_port: u16, language: Option<&str>) -> Result<Vec<String>, String> {
    let host_ok = |h: &str| !h.is_empty() && h.len() <= 200 && h.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-');
    if !host_ok(host) || !host_ok(update_host) {
        return Err(format!("server address {host:?} / {update_host:?}: a host name or IPv4 address (letters, digits, '.', '-')"));
    }
    let localization = match language {
        None => String::new(),
        Some(l) if !l.is_empty() && l.len() <= 20 && l.bytes().all(|b| b.is_ascii_lowercase()) => format!(" -localization={l}"),
        Some(l) => return Err(format!("language {l:?}: lower-case letters only, e.g. russian")),
    };
    let lines = vec![
        format!("update_server {update_host}:{update_port}"),
        "file_to_execute game\\binaries\\x64\\survarium.exe".to_string(),
        format!("arguments -product_name=Survarium-Steam -server={host}:{login_port}{localization}"),
        "steam 1".to_string(),
        "srp_steam_play 1".to_string(),
    ];
    if let Some(l) = lines.iter().find(|l| l.len() > MAX_LINE) {
        return Err(format!("launcher.cfg line too long for the launcher ({} > {MAX_LINE} characters): {l}", l.len()));
    }
    Ok(lines)
}

/// The longest launcher.cfg line the launcher reads whole (260-unit `fgetws` buffer, minus CR LF and the end zero).
pub const MAX_LINE: usize = 257;

/// The file as the launcher reads it: UTF-16LE without a BOM (like the shipped launcher.cfg), lines joined by CR LF.
pub fn launcher_cfg_bytes(lines: &[String]) -> Vec<u8> {
    lines.join("\r\n").encode_utf16().flat_map(u16::to_le_bytes).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The exact bytes the updater writes (`http_client::get` 0x200007CC4..0x200007D75, no proxy, no Range).
    fn updater_request(target: &str, host: &str) -> Vec<u8> {
        format!("GET {target} HTTP/1.0\r\nHost: {host}\r\nAccept: */*\r\nConnection: close\r\n\r\n").into_bytes()
    }

    /// What the updater does with a reply: status line `HTTP/` + status, headers to the empty line, the body is the rest.
    fn updater_reads(reply: &[u8]) -> (u16, String) {
        let text = String::from_utf8(reply.to_vec()).unwrap();
        let (head, body) = text.split_once("\r\n\r\n").unwrap();
        let mut lines = head.split("\r\n");
        let mut status = lines.next().unwrap().split(' ');
        assert!(status.next().unwrap().starts_with("HTTP/"));
        let code: u16 = status.next().unwrap().parse().unwrap();
        let len: usize = lines.find_map(|l| l.strip_prefix("Content-Length:")).expect("Content-Length, case as the updater compares").trim().parse().unwrap();
        assert_eq!(len, body.len());
        (code, body.to_string())
    }

    #[test]
    fn download_type_is_answered_http_exactly() {
        let r = parse(&updater_request("/download_type.php", "203.0.113.5"));
        assert_eq!(r, Request::DownloadType);
        assert_eq!(updater_reads(&reply(&r, None)), (200, "http".to_string()));
        // with a path in update_server (host/path, port 80)
        assert_eq!(parse(&updater_request("/survarium/public/download_type.php", "h")), Request::DownloadType);
    }

    #[test]
    fn version_request_gets_latest_exactly() {
        for (target, ut, dt) in [
            ("/?update_type=update&version=0.69d0&download_type=http&public_key", "update", "http"),
            ("/?update_type=full_update&version=&download_type=torrent&public_key", "full_update", "torrent"),
            ("/survarium/public/?update_type=update&version=0.69d0&download_type=http&public_key", "update", "http"),
        ] {
            let r = parse(&updater_request(target, "h"));
            assert_eq!(r, Request::Version { update_type: ut.into(), version: if ut == "update" { "0.69d0".into() } else { String::new() }, download_type: dt.into() });
            let (code, body) = updater_reads(&reply(&r, None));
            assert_eq!(code, 200);
            // compared as a whole (strcmp at 0x20002EAD1): no newline, no ';'
            assert_eq!(body, "<you use the latest version!>");
            assert!(!body.contains(';'));
        }
    }

    #[test]
    fn everything_else_is_refused() {
        assert_eq!(parse(&updater_request("/", "h")), Request::NotFound);
        assert_eq!(parse(&updater_request("/index.html", "h")), Request::NotFound);
        assert_eq!(parse(&updater_request("/x?update_type=update", "h")), Request::NotFound);
        assert_eq!(parse(b"POST /download_type.php HTTP/1.0\r\n\r\n"), Request::BadMethod);
        assert_eq!(parse(b"GET /download_type.php\r\n\r\n"), Request::BadRequest);
        assert_eq!(parse(b"\xff\xfe\r\n\r\n"), Request::BadRequest);
        assert_eq!(updater_reads(&reply(&Request::NotFound, None)).0, 404);
        assert_eq!(updater_reads(&reply(&Request::BadMethod, None)).0, 405);
        assert_eq!(updater_reads(&reply(&Request::BadRequest, None)).0, 400);
    }

    /// Over TCP as the updater does it: connect, write the request, read to EOF (Connection: close).
    #[test]
    fn tcp_round_trip() {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap();
        let start_data = client_txt(Some("203.0.113.5"), 65000);
        let served = start_data.clone();
        std::thread::spawn(move || serve(l, Some(served)));
        let ask = |req: &[u8]| {
            let mut s = TcpStream::connect(addr).unwrap();
            s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            s.write_all(req).unwrap();
            let mut out = Vec::new();
            s.read_to_end(&mut out).unwrap();
            updater_reads(&out)
        };
        assert_eq!(ask(&updater_request("/download_type.php", "127.0.0.1")), (200, "http".into()));
        assert_eq!(ask(&updater_request("/?update_type=update&version=0.69d0&download_type=http&public_key", "127.0.0.1")), (200, LATEST.into()));
        // the player script's request (Windows PowerShell 5.1 Invoke-WebRequest: HTTP/1.1, keep-alive asked, we close)
        let iwr = b"GET /skhid/client.txt HTTP/1.1\r\nUser-Agent: Mozilla/5.0 (Windows NT; Windows NT 10.0; en-US) WindowsPowerShell/5.1\r\nHost: 127.0.0.1:65001\r\nConnection: Keep-Alive\r\n\r\n";
        assert_eq!(ask(iwr), (200, start_data));
        // the head split over two writes
        let mut s = TcpStream::connect(addr).unwrap();
        s.write_all(b"GET /download_type.php HTTP/1.0\r\nHost: x\r\n").unwrap();
        std::thread::sleep(Duration::from_millis(50));
        s.write_all(b"Accept: */*\r\n\r\n").unwrap();
        let mut out = Vec::new();
        s.read_to_end(&mut out).unwrap();
        assert_eq!(updater_reads(&out), (200, "http".into()));
        // a head that never ends is cut at MAX_HEAD and refused
        let mut s = TcpStream::connect(addr).unwrap();
        let _ = s.write_all(&vec![b'a'; MAX_HEAD + 10]);
        let mut out = Vec::new();
        let _ = s.read_to_end(&mut out);
        assert_eq!(updater_reads(&out).0, 400);
    }

    #[test]
    fn client_txt_is_served_at_its_path_only() {
        assert_eq!(parse(&updater_request(CLIENT_TXT_PATH, "h")), Request::ClientTxt);
        assert_eq!(parse(&updater_request("/skhid/client.txt?x=1", "h")), Request::ClientTxt);
        for other in ["/skhid/client.txt/", "/a/skhid/client.txt", "/skhid/", "/skhid/client.TXT", "/skhid/../srp.toml"] {
            assert_eq!(parse(&updater_request(other, "h")), Request::NotFound, "{other}");
        }
        assert_eq!(parse(b"POST /skhid/client.txt HTTP/1.0\r\n\r\n"), Request::BadMethod);
        let body = client_txt(Some("203.0.113.5"), 65000);
        assert_eq!(body, "format=1\nlogin_host=203.0.113.5\nlogin_port=65000\n");
        assert_eq!(updater_reads(&reply(&Request::ClientTxt, Some(&body))), (200, body.clone()));
        assert_eq!(updater_reads(&reply(&Request::ClientTxt, None)).0, 404);
        // unknown address: an empty value, the key stays (the script then keeps the typed address)
        assert!(client_txt(None, 65000).contains("\nlogin_host=\n"));
    }

    /// The checks `skhid-setup.ps1` makes on the start data, done here on what the server really serves (a real key).
    #[test]
    fn client_txt_passes_the_player_script_checks() {
        let mut c = crate::Config::default();
        c.login_addr = "0.0.0.0:65000".parse().unwrap();
        c.online.public_host = "203.0.113.7".into();
        let body = client_txt(c.player_host().as_deref(), c.login_addr.port());
        let mut data = HashMap::new();
        for line in body.split('\n').filter(|l| !l.is_empty()) {
            let (key, value) = line.split_once('=').unwrap();
            assert!(!key.is_empty() && key.bytes().all(|b| b.is_ascii_lowercase() || b == b'_'), "{line}");
            assert!(value.bytes().all(|b| (b' '..=b'~').contains(&b)), "{line}");
            data.insert(key, value);
        }
        assert_eq!(data["format"], "1");
        assert_eq!(data["login_host"], "203.0.113.7");
        assert_eq!(data["login_port"].parse::<u16>().unwrap(), 65000);
        assert_eq!(data.len(), 3, "format, login_host, login_port only");
        assert!(body.len() < 4096, "the script refuses longer answers");
    }

    /// `scripts/player/skhid-setup.ps1` reads the start data and writes the launch line the server prints (it cannot run
    /// here; its text is checked).
    #[test]
    fn the_player_script_reads_the_same_format() {
        let ps = include_str!("../../../scripts/player/skhid-setup.ps1");
        let body = client_txt(Some("h"), 1);
        for key in body.lines().map(|l| l.split_once('=').unwrap().0).filter(|k| *k != "format") {
            assert!(ps.contains(&format!("$data['{key}']")), "skhid-setup.ps1 does not read {key}");
        }
        for needle in [
            "/skhid/client.txt\"".to_string(),
            format!("$data['format'] -ne '{CLIENT_TXT_FORMAT}'"),
            format!("[int]$UpdatePort = {DEFAULT_PORT}"),
        ] {
            assert!(ps.contains(&needle), "skhid-setup.ps1 lacks {needle}");
        }
        assert!(ps.contains(CLIENT_TXT_PATH));
        assert!(!ps.contains("login_public_key"), "the 0.69d game stops at an unregistered argument");
        // the start file's game line = the server's launch line (setup::launch_line) behind `start ""`
        let template = ps.lines().map(str::trim).find(|l| l.starts_with("\"start `\"`\" survarium.exe")).expect("the start line");
        let mut c = crate::Config::default();
        c.login_addr = "0.0.0.0:65000".parse().unwrap();
        c.online.public_host = "203.0.113.7".into();
        let filled = template
            .trim_end_matches(',')
            .trim_matches('"')
            .replace("start `\"`\" ", "")
            .replace("${loginHost}", "203.0.113.7")
            .replace("$loginPort", "65000")
            .replace("$Language", "english");
        assert_eq!(filled, crate::setup::launch_line(&c));
        // the languages: the game's list (texts::LANGUAGES), no more, no fewer
        let langs_line = ps.lines().find(|l| l.starts_with("$languages = @(")).expect("the language list");
        let mut script_langs: Vec<&str> = langs_line.split('\'').skip(1).step_by(2).collect();
        let mut game_langs: Vec<&str> = game_data::texts::LANGUAGES.iter().copied().filter(|l| !l.is_empty()).collect();
        script_langs.sort_unstable();
        game_langs.sort_unstable();
        assert_eq!(script_langs, game_langs);
        // the marker: setup writes it, undo deletes only a file that starts with it
        let marker = "$marker = '@rem skhid-setup:'";
        assert!(ps.contains(marker) && ps.contains("$old.StartsWith($marker)"));
        let undo = include_str!("../../../scripts/player/skhid-undo.ps1");
        assert!(undo.contains(marker) && undo.contains("$old.StartsWith($marker)") && undo.contains("'Skhid.cmd'"));
        assert!(ps.contains("'Skhid.cmd'"));
    }

    #[test]
    fn launcher_cfg_lines_fit_the_launcher() {
        // default: no -localization, so the launcher's own (Steam's game language) decides
        let lines = launcher_cfg_lines("203.0.113.5", 65000, "203.0.113.5", 65001, None).unwrap();
        assert_eq!(
            lines,
            [
                "update_server 203.0.113.5:65001",
                "file_to_execute game\\binaries\\x64\\survarium.exe",
                "arguments -product_name=Survarium-Steam -server=203.0.113.5:65000",
                "steam 1",
                "srp_steam_play 1",
            ]
        );
        // an override goes into arguments, ahead of the launcher's -localization; the other lines stay the same
        let over = launcher_cfg_lines("203.0.113.5", 65000, "203.0.113.5", 65001, Some("russian")).unwrap();
        assert_eq!(over[2], "arguments -product_name=Survarium-Steam -server=203.0.113.5:65000 -localization=russian");
        assert_eq!([&over[..2], &over[3..]].concat(), [&lines[..2], &lines[3..]].concat());
        // every line has a key, one space, a value (a line without a space ends the launcher's reading)
        for set in [&lines, &over] {
            assert!(set.iter().all(|l| l.split_once(' ').is_some_and(|(k, v)| !k.is_empty() && !v.is_empty())));
        }
        let b = launcher_cfg_bytes(&lines);
        assert_eq!(&b[..4], b"u\0p\0", "UTF-16LE without BOM, like the shipped file");
        assert!(launcher_cfg_lines("a".repeat(150).as_str(), 65000, "h", 65001, Some("english")).is_ok());
        let long = launcher_cfg_lines("a".repeat(200).as_str(), 65000, "h", 65001, Some("english")).unwrap_err();
        assert!(long.contains("too long") && long.contains("arguments"), "{long}");
        assert!(launcher_cfg_lines("bad host", 65000, "h", 65001, None).is_err());
        assert!(launcher_cfg_lines("h", 65000, "h:1", 65001, None).is_err());
        assert!(launcher_cfg_lines("h", 65000, "h", 65001, Some("English")).is_err());
        assert!(launcher_cfg_lines("h", 65000, "h", 65001, Some("")).is_err());
    }

    /// The player's script writes the same lines (it cannot run here; its text is checked).
    #[test]
    fn the_setup_script_writes_the_same_lines() {
        let ps = include_str!("../../../scripts/steam-play/steam-play-setup.ps1");
        for needle in [
            "\"update_server ${UpdateServer}:$UpdatePort\"",
            "'file_to_execute game\\binaries\\x64\\survarium.exe'",
            "'steam 1'",
            "'srp_steam_play 1'",
            "UnicodeEncoding($false, $false)",
            &format!("-gt {MAX_LINE}"),
            "$localization = if ($Language) { \" -localization=$Language\" } else { '' }",
        ] {
            assert!(ps.contains(needle), "steam-play-setup.ps1 lacks {needle}");
        }
        // -Language has no default: left out, the file has no -localization (launcher_cfg_lines(.., None))
        assert!(ps.lines().any(|l| l.trim() == "[string]$Language,"), "-Language must have no default");
        // the script's arguments line, filled for no language and for an override, = launcher_cfg_lines
        let template = ps.lines().map(str::trim).find(|l| l.starts_with("\"arguments ")).expect("the arguments line");
        let fill = |loc: &str| {
            template.trim_end_matches(',').trim_matches('"').replace("${Server}", "203.0.113.5").replace("$Port", "65000").replace("$localization", loc)
        };
        for (lang, loc) in [(None, ""), (Some("ukrainian"), " -localization=ukrainian")] {
            let rust = launcher_cfg_lines("203.0.113.5", 65000, "203.0.113.5", 65001, lang).unwrap();
            assert_eq!(fill(loc), rust[2], "{lang:?}");
        }
        let undo = include_str!("../../../scripts/steam-play/steam-play-undo.ps1");
        assert!(undo.contains("srp_steam_play") && undo.contains("'Survarium-Steam\\launcher.cfg'"));
    }
}
