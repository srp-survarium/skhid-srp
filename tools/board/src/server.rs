//! Routing, response headers and the data file store. Nothing here touches the network, so all
//! of it is tested directly; `main.rs` only moves requests and responses through tiny_http.

use crate::model::{self, Board, Date};
use crate::render;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Longer request targets are refused outright.
pub const MAX_URL: usize = 1024;

#[derive(Debug, PartialEq, Eq)]
pub enum Route {
    Board,
    /// `/<secret>` without the trailing slash.
    AddSlash,
    NotFound,
    MethodNotAllowed,
    TooLarge,
}

/// Route one request. Only `/<secret>/` (any query string) is the board; every other path is a
/// plain 404 whatever the method, so nothing tells a wrong guess from a right one.
pub fn route(method: &str, url: &str, has_body: bool, secret: &str) -> Route {
    if url.len() > MAX_URL {
        return Route::TooLarge;
    }
    let path = url.split(['?', '#']).next().unwrap_or("");
    let rest = match path.strip_prefix('/').and_then(|p| p.strip_prefix(secret)) {
        Some(rest) if !secret.is_empty() => rest,
        _ => return Route::NotFound,
    };
    let route = match rest {
        "/" => Route::Board,
        "" => Route::AddSlash,
        _ => return Route::NotFound,
    };
    if method != "GET" && method != "HEAD" {
        return Route::MethodNotAllowed;
    }
    if has_body {
        return Route::TooLarge;
    }
    route
}

/// Headers sent with every response.
pub const HEADERS: [(&str, &str); 7] = [
    ("Cache-Control", "no-store"),
    ("X-Robots-Tag", "noindex, nofollow"),
    ("Referrer-Policy", "no-referrer"),
    ("X-Content-Type-Options", "nosniff"),
    ("X-Frame-Options", "DENY"),
    (
        "Content-Security-Policy",
        "default-src 'none'; style-src 'unsafe-inline'; img-src data:; base-uri 'none'; form-action 'none'; frame-ancestors 'none'",
    ),
    ("Content-Type", "text/html; charset=utf-8"),
];

pub struct Response {
    pub status: u16,
    pub body: String,
    /// Extra headers on top of `HEADERS`.
    pub extra: Vec<(&'static str, String)>,
}

/// The response for a routed request. The secret appears only in the redirect of `/<secret>`,
/// which is sent to a client that already knows it.
pub fn respond(route: Route, store: &mut Store, secret: &str, now: SystemTime) -> Response {
    let plain = |status, body: &str| Response { status, body: body.to_string(), extra: Vec::new() };
    match route {
        Route::Board => {
            store.refresh(now);
            Response { status: 200, body: store.page(today(now)), extra: Vec::new() }
        }
        Route::AddSlash => Response {
            status: 301,
            body: String::new(),
            extra: vec![("Location", format!("/{secret}/"))],
        },
        Route::NotFound => plain(404, render::NOT_FOUND),
        Route::MethodNotAllowed => Response {
            status: 405,
            body: String::new(),
            extra: vec![("Allow", "GET, HEAD".to_string())],
        },
        Route::TooLarge => plain(413, ""),
    }
}

pub fn today(now: SystemTime) -> Date {
    Date::from_days(unix_secs(now).div_euclid(86400))
}

fn unix_secs(t: SystemTime) -> i64 {
    match t.duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_secs() as i64,
        Err(e) => -(e.duration().as_secs() as i64),
    }
}

/// "2026-10-23 14:05 UTC".
pub fn utc(t: SystemTime) -> String {
    let s = unix_secs(t);
    let tod = s.rem_euclid(86400);
    format!("{} {:02}:{:02} UTC", Date::from_days(s.div_euclid(86400)), tod / 3600, tod % 3600 / 60)
}

/// The data file, re-read whenever it changes on disk. A file that fails to read or validate
/// never replaces the last good board; the page shows why it was rejected instead.
pub struct Store {
    path: PathBuf,
    /// What the file looked like at the last read: (mtime, size, inode).
    seen: Option<(SystemTime, u64, u64)>,
    good: Option<(Board, SystemTime)>,
    rejected: Option<(String, SystemTime)>,
}

impl Store {
    pub fn new(path: PathBuf) -> Store {
        Store { path, seen: None, good: None, rejected: None }
    }

    pub fn board(&self) -> Option<&Board> {
        self.good.as_ref().map(|(b, _)| b)
    }

    pub fn rejected(&self) -> Option<&str> {
        self.rejected.as_ref().map(|(m, _)| m.as_str())
    }

    /// Re-read the file if it changed since the last look.
    pub fn refresh(&mut self, now: SystemTime) {
        use std::os::unix::fs::MetadataExt;
        let meta = match std::fs::metadata(&self.path) {
            Ok(m) => m,
            Err(e) => {
                if self.seen.is_some() || self.rejected.is_none() {
                    self.reject(format!("cannot read the data file: {e}"), now);
                }
                self.seen = None;
                return;
            }
        };
        let stamp = (meta.modified().unwrap_or(UNIX_EPOCH), meta.len(), meta.ino());
        if self.seen == Some(stamp) {
            return;
        }
        self.seen = Some(stamp);
        let parsed = std::fs::read_to_string(&self.path)
            .map_err(|e| format!("cannot read the data file: {e}"))
            .and_then(|text| model::parse(&text));
        match parsed {
            Ok(board) => {
                eprintln!("board: loaded {} item(s), {} notice(s)", board.items.len(), board.notices.len());
                self.good = Some((board, now));
                self.rejected = None;
            }
            Err(e) => self.reject(e, now),
        }
    }

    fn reject(&mut self, msg: String, now: SystemTime) {
        eprintln!("board: data file rejected: {}", msg.replace('\n', "; "));
        self.rejected = Some((msg, now));
    }

    pub fn page(&self, today: Date) -> String {
        let banner = self.rejected.as_ref().map(|(msg, at)| {
            let kept = match &self.good {
                Some((_, loaded)) => format!("Showing the last good version, loaded {}.", utc(*loaded)),
                None => "No good version has been loaded yet.".to_string(),
            };
            format!("The data file was rejected at {}. {kept}\n{msg}", utc(*at))
        });
        render::page(self.board(), banner.as_deref(), today)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    const S: &str = "0123456789abcdef0123456789abcdef";

    #[test]
    fn only_the_secret_path_is_the_board() {
        assert_eq!(route("GET", &format!("/{S}/"), false, S), Route::Board);
        assert_eq!(route("HEAD", &format!("/{S}/?x=1"), false, S), Route::Board);
        assert_eq!(route("GET", &format!("/{S}"), false, S), Route::AddSlash);
        for url in ["/", "/other", "/favicon.ico", "", "*"] {
            assert_eq!(route("GET", url, false, S), Route::NotFound, "{url}");
        }
        let wrong = format!("/{}/", &S[..S.len() - 1]);
        assert_eq!(route("GET", &wrong, false, S), Route::NotFound);
        assert_eq!(route("GET", &format!("/{S}x/"), false, S), Route::NotFound);
        assert_eq!(route("GET", &format!("/{S}/card/x"), false, S), Route::NotFound);
        assert_eq!(route("GET", &format!("//{S}/"), false, S), Route::NotFound);
        assert_eq!(route("GET", "//", false, ""), Route::NotFound, "an empty secret matches nothing");
    }

    #[test]
    fn methods_bodies_and_size() {
        assert_eq!(route("POST", &format!("/{S}/"), false, S), Route::MethodNotAllowed);
        assert_eq!(route("POST", "/other", false, S), Route::NotFound);
        assert_eq!(route("GET", &format!("/{S}/"), true, S), Route::TooLarge);
        let long = format!("/{S}/?{}", "a".repeat(MAX_URL));
        assert_eq!(route("GET", &long, false, S), Route::TooLarge);
    }

    #[test]
    fn not_found_never_shows_the_secret() {
        let dir = tempdir("nf");
        let mut store = Store::new(dir.join("board.json"));
        for r in [Route::NotFound, Route::MethodNotAllowed, Route::TooLarge] {
            let resp = respond(r, &mut store, S, SystemTime::now());
            assert!(!resp.body.contains(S));
            assert!(resp.extra.iter().all(|(_, v)| !v.contains(S)));
        }
        assert!(HEADERS.iter().any(|(k, v)| *k == "Referrer-Policy" && *v == "no-referrer"));
        assert!(HEADERS.iter().any(|(k, v)| *k == "Cache-Control" && *v == "no-store"));
    }

    fn tempdir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("skhid-board-test-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// Write and push the mtime forward, so a rewrite in the same instant still counts as new.
    fn write(path: &std::path::Path, text: &str, n: u64) {
        std::fs::write(path, text).unwrap();
        let f = std::fs::File::options().write(true).open(path).unwrap();
        f.set_modified(UNIX_EPOCH + Duration::from_secs(1_800_000_000 + n)).unwrap();
    }

    #[test]
    fn reload_keeps_the_last_good_board() {
        let dir = tempdir("reload");
        let path = dir.join("board.json");
        let now = SystemTime::now();
        let mut store = Store::new(path.clone());

        store.refresh(now);
        assert!(store.board().is_none());
        assert!(store.rejected().unwrap().contains("cannot read"));

        let good = include_str!("../example.json");
        write(&path, good, 1);
        store.refresh(now);
        let n = store.board().unwrap().items.len();
        assert!(store.rejected().is_none());

        // A broken file: the old board stays, with a banner naming the problem.
        write(&path, &good.replacen("\"state\": \"next\"", "\"state\": \"someday\"", 1), 2);
        store.refresh(now);
        assert_eq!(store.board().unwrap().items.len(), n);
        let why = store.rejected().unwrap();
        assert!(why.contains(".state: unknown state \"someday\""), "{why}");
        let page = store.page(today(now));
        assert!(page.contains("class=\"banner\""));
        assert!(page.contains("Showing the last good version"));

        // Unparseable JSON: same.
        write(&path, "{ not json", 3);
        store.refresh(now);
        assert_eq!(store.board().unwrap().items.len(), n);
        assert!(store.rejected().unwrap().starts_with("invalid JSON"));

        // A good file again: new content, banner gone.
        let fewer = r#"{"updated":"later","items":[{"n":1,"id":"a","title":"A","state":"next","next":"x","updated":"2026-10-23"}]}"#;
        write(&path, fewer, 4);
        store.refresh(now);
        assert_eq!(store.board().unwrap().items.len(), 1);
        assert_eq!(store.board().unwrap().updated, "later");
        assert!(store.rejected().is_none());
        assert!(!store.page(today(now)).contains("class=\"banner\""));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn utc_formatting() {
        let t = UNIX_EPOCH + Duration::from_secs(1_792_681_020); // 2026-10-22 14:57 UTC
        assert_eq!(utc(t), "2026-10-22 14:57 UTC");
        assert_eq!(today(t), Date { y: 2026, m: 10, d: 22 });
    }
}
