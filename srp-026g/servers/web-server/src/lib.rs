//! A small website for the people running srp.
//!
//! - `/` is a home page: what the project is and where it came from.
//! - `/exchange` (the 0.26g page only, `GET`) says that silver and gold are
//!   exchanged in the game now: the SHOP button of the launchers already in
//!   players' hands opens it ([`v026g::in_the_game_now`]). Nothing is
//!   exchanged on the page; on the 0.21d page it is "not found".
//! - `/launcher/…` and `/api/client-files` (the 0.26g page only) are for the
//!   patched game launcher and its updater ([`launcher`]): the page it shows
//!   in its window, and which archive `/download` hands out now.
//! - `/patches` (the 0.26g page only) is the list of `resources.db`'s patches
//!   the updater keeps in the game folder: those the server's game data was
//!   made of ([`patches`]).
//! - `/resources/<sha256>` (the 0.26g page only) is a blob from the folder
//!   `resources` beside the download file ([`resource_file`]): the originals
//!   of the `resources.db` regions earlier updaters wrote, which the
//!   updater fetches to put the stock db back once when it kept no copy.
//! - `/replays/<token>` (the 0.26g page only) is the replay file of the
//!   match a token of the lobby's "Watch replay" is for
//!   (`session::v026g::replays`): `SRP_REPLAY_DIR/<id:08x>.sur`, named from
//!   the token's match id alone ([`replay_file`]); anything else under
//!   `/replays` is one and the same "not found", and an address that fails
//!   too often is answered `429` for a while ([`Web::replay`]).
//! - The admin page changes the silver and gold of any account, for testing.
//!   It exists only when `SRP_ADMIN_TOKEN` and `SRP_ADMIN_PATH` (a hard to
//!   guess name, served at `/<name>`) are set, and only for the addresses in
//!   `SRP_ADMIN_IPS` (default: this machine). Anyone else gets the same
//!   "not found" as for a page that does not exist. It also resets a
//!   forgotten password, on its Password tab or by a script's one POST
//!   ([`password_reset`]).
//!
//! It listens on `SRP_WEB_BIND:SRP_WEB_PORT`, by default 127.0.0.1:8088: the
//! page is plain HTTP, so a public host puts HTTPS in front of it (the
//! compose file's Caddy) instead of opening the port. Every request carries
//! its own credentials, so there are no cookies and nothing to forge across
//! sites.
//!
//! The page is of one game version, by how it is started: [`run`] is the
//! 0.21d page (balances of `session::Account::game`, `-client=`), and
//! [`run_026g`] the 0.26g one ([`v026g`]: the 0.26g accounts, the lobby told
//! of every change, `-server=`). `srp` starts the 0.26g one.

mod damage;
mod launcher;
mod maps;
mod password_reset;
mod patches;
mod stats;
mod v026g;

pub use patches::OfferedPatch;
pub use v026g::{Game, LiveMatch, LivePhase, LiveStanding, LobbyLink, MatchCounter};

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::net::{IpAddr, Shutdown, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use session::v026g::replays::{Refused, Replays};
use session::{Ban, LockExt, Net, SessionStore};

/// What the home page tells a new player, fixed at start-up.
struct Connect {
    /// The login server as the client is told to reach it (`host:port`).
    address: String,
    /// `SRP_DOWNLOAD_URL`: where the patched game can be had.
    download_url: Option<String>,
    /// `SRP_DOWNLOAD_SHA256`: its checksum.
    sha256: Option<String>,
    /// `SRP_DOWNLOAD_FILE`: a file this server hands out itself at
    /// `/download` (the only file it ever serves; no path comes from a request).
    download_file: Option<PathBuf>,
    /// `SRP_OTHER_SERVER_URL`: the page of the server for the other game
    /// version (0.21d and 0.26g link to each other); `https://` only.
    other_url: Option<String>,
    /// `SRP_GAME_URL` (the 0.26g page): where the original, unchanged game
    /// can be had; `https://` only. The page's own download is then only
    /// the few files that go over it.
    game_url: Option<String>,
    /// `SRP_TEST_SERVER=on`: every page says this is the test server. The
    /// link of `SRP_OTHER_SERVER_URL` is then the players' server.
    test_server: bool,
}

/// Downloads of that file at one time; more get "busy, try again". The
/// blobs of `/resources/` have as many slots again, of their own: they never
/// take the archive's.
const MAX_DOWNLOADS: u32 = 5;
/// A download slower than this for a minute is cut, so slow readers cannot
/// hold the slots.
const MIN_DOWNLOAD_RATE: u64 = 32 * 1024;
const DOWNLOAD_GRACE: Duration = Duration::from_secs(60);

static CONNECT: OnceLock<Connect> = OnceLock::new();

fn env_text(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

const MAX_REQUEST: usize = 4096;
const MAX_CONNECTIONS: u32 = 64;
const IO_TIMEOUT: Duration = Duration::from_secs(5);
/// The longest timed ban, in hours (ten years); longer is for good.
const MAX_BAN_HOURS: u64 = 24 * 365 * 10;
/// The furthest a maintenance can be planned: a week.
const MAX_MAINTENANCE_MINUTES: u64 = 7 * 24 * 60;
/// Wrong admin tokens in a row before the admin page waits.
const MAX_TOKEN_FAILURES: u32 = 5;
const TOKEN_LOCKOUT: Duration = Duration::from_secs(60);
/// The `Content-Security-Policy` of every page but the launcher's
/// ([`launcher::policy`]).
const POLICY: &str =
    "default-src 'none'; style-src 'unsafe-inline'; font-src 'self'; form-action 'self'";

/// The page's languages: Ukrainian and English only.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Lang {
    En,
    Uk,
}

impl Lang {
    /// `?lang=` or the form's `lang`, else the browser's `Accept-Language`
    /// (Ukrainian when it comes first), else English. (`ua` is what the
    /// game's launcher calls Ukrainian in the addresses it opens.)
    fn pick(chosen: &str, accept: &str) -> Self {
        match chosen {
            "uk" | "ua" => return Self::Uk,
            "en" => return Self::En,
            _ => {}
        }
        let first = accept.split(',').next().unwrap_or("").trim().to_lowercase();
        if first.starts_with("uk") {
            Self::Uk
        } else {
            Self::En
        }
    }

    fn code(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::Uk => "uk",
        }
    }

    fn tr(self, en: &str, uk: &str) -> String {
        match self {
            Self::En => en.to_owned(),
            Self::Uk => uk.to_owned(),
        }
    }
}

/// What the home page counts.
#[derive(Clone, Copy)]
struct Stats {
    registered: usize,
    online: usize,
    /// The minutes to a planned maintenance.
    maintenance: Option<u32>,
    registrations_open: bool,
    /// The 0.26g page.
    v026g: bool,
    /// The download holds the launcher (0.26g): the page tells players to
    /// start that, and keeps the start script and the command line for
    /// those who want them.
    launcher: bool,
    /// The download's launcher brings the original game itself (0.26g):
    /// the page tells players to unpack into an empty folder, and names
    /// the game's own archive only as the way out when that fails.
    installs_game: bool,
}

enum Form {
    None,
    Home(Stats),
    Admin(Box<AdminView>),
    /// The 0.26g page's "not found".
    None26,
    /// The 0.26g page's `/exchange`: in the game now, in the launcher's
    /// language ([`v026g::launcher_code`]).
    InTheGame26(&'static str),
}

impl Form {
    fn is_026g(&self) -> bool {
        match self {
            Self::None => false,
            Self::None26 | Self::InTheGame26(_) => true,
            Self::Home(stats) => stats.v026g,
            Self::Admin(view) => view.g26,
        }
    }
}

/// The admin page as shown: before the token (all empty), or after it.
#[derive(Default)]
struct AdminView {
    /// The token, once it was right: the form keeps it, so each action
    /// needs it only once.
    token: String,
    /// The tab to show.
    tab: String,
    /// The bans, as tables (after the token only).
    bans: String,
    /// What is planned, as a sentence (after the token only).
    maintenance: String,
    /// Whether new accounts are taken, for the form's buttons.
    registrations_open: bool,
    /// The 0.26g page: the account, item and text tabs too.
    g26: bool,
    /// The operator's text that stands (0.26g, after the token only).
    admin_text: Option<(String, String)>,
    /// The password tab: the account to confirm, or the user name typed.
    reset: password_reset::ResetView,
}

/// Why the admin token was not taken.
enum TokenRefused {
    /// Too many wrong ones lately ([`MAX_TOKEN_FAILURES`]).
    Locked,
    Wrong,
}

/// Frees a download slot when the transfer ends, however it ends.
struct Slot<'a>(&'a AtomicU32);

impl Drop for Slot<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

/// The file of `/resources/<name>` beside the download file `download`:
/// `<its folder>/resources/<name>`, for a name of 64 lower-case hex digits
/// (the SHA-256 of the blob; `client/updater`, "Resources") and no other.
/// No other part of a request's path ever names a file.
fn resource_file(path: &str, download: &Path) -> Option<PathBuf> {
    let name = path.strip_prefix("/resources/")?;
    if name.len() != 64 || !name.bytes().all(|c| matches!(c, b'0'..=b'9' | b'a'..=b'f')) {
        return None;
    }
    Some(download.parent()?.join("resources").join(name))
}

/// The blob a request asks for: a `GET` or `HEAD` of `/resources/<name>`
/// on the 0.26g page (`v026g`), when there is a download file. Anything
/// else is the page's to answer, as before there were blobs.
fn resource_request(v026g: bool, request: &Request, download: Option<&Path>) -> Option<PathBuf> {
    if !v026g || !matches!(request.method.as_str(), "GET" | "HEAD") {
        return None;
    }
    resource_file(&request.path, download?)
}

/// Where the replays are handed out: `/replays/<token>` (the 0.26g page;
/// `docs/notes/replay_watch.md`).
const REPLAYS: &str = "/replays/";
/// Failed `/replays/` lookups an address may make in [`REPLAY_FAILURE_WINDOW`]
/// before it is answered `429` for [`REPLAY_BLOCK`]: a token cannot be
/// guessed (128 random bits), and cannot be scanned for either.
const REPLAY_FAILURES: u32 = 30;
const REPLAY_FAILURE_WINDOW: Duration = Duration::from_secs(60);
const REPLAY_BLOCK: Duration = Duration::from_secs(60);
/// Addresses whose failed lookups are counted, at most.
const REPLAY_FAILURE_ADDRESSES: usize = 4096;
/// Every `/replays/` request that does not get a file gets exactly this:
/// one answer, the same bytes, whatever was wrong with it.
const REPLAY_NOT_FOUND: &[u8] =
    b"HTTP/1.0 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
/// A live token whose one download at a time is under way: a token holds
/// one of the page's download slots at most, so an account
/// `TOKENS_PER_ACCOUNT` (3) of [`MAX_DOWNLOADS`] (5).
const REPLAY_BUSY: &[u8] = b"HTTP/1.0 503 Service Unavailable\r\nRetry-After: 10\r\n\
    Content-Length: 0\r\nConnection: close\r\n\r\n";

/// Whether a request is the replays' to answer: its path is `/replays` or
/// under `/replays/`. Every such request is answered by [`Web::replay`]
/// (a file or [`REPLAY_NOT_FOUND`]), never by a page.
fn replay_route(request: &Request) -> bool {
    request.path == "/replays" || request.path.starts_with(REPLAYS)
}

/// The token of a `GET` or `HEAD` whose target, exactly as sent (no query,
/// no fragment, no escape, nothing after it), is `/replays/<32 lower-case
/// hex digits>`. Nothing of it is ever a path: the token is looked up, and
/// the file named from its match id ([`replay_file`]).
fn replay_token<'a>(method: &str, target: &'a str) -> Option<&'a str> {
    if !matches!(method, "GET" | "HEAD") {
        return None;
    }
    let token = target.strip_prefix(REPLAYS)?;
    (token.len() == 32
        && token
            .bytes()
            .all(|c| matches!(c, b'0'..=b'9' | b'a'..=b'f')))
    .then_some(token)
}

/// The replay file of a match: `SRP_REPLAY_DIR/<id:08x>.sur`, its name made
/// from the match id alone (`Replays::sur_path`), and only when it is a
/// regular file (not a link, not a folder) whose real path is in the
/// replay folder.
fn replay_file(replays: &Replays, match_id: u32) -> Option<PathBuf> {
    let dir = std::fs::canonicalize(&replays.settings()?.replay_dir).ok()?;
    let path = replays.sur_path(match_id)?;
    if !std::fs::symlink_metadata(&path).ok()?.file_type().is_file() {
        return None;
    }
    let real = std::fs::canonicalize(&path).ok()?;
    (real.parent() == Some(dir.as_path())).then_some(real)
}

/// The log line of a replay handed out. The `Range` is the one value of
/// the request in it that is not checked first, and is shown escaped
/// (`{:?}`): a header line is split at `\r\n` alone, so a bare `\n` or any
/// other control character it carries cannot start a forged line. (The
/// method is `GET` or `HEAD`, the token 32 hex digits, the address parsed.)
fn replay_log_line(request: &Request, match_id: u32, token: &str) -> String {
    format!(
        "Replays: {} of match {match_id:#x} for {} (token {}…, range {:?})",
        request.method,
        request
            .client
            .map_or_else(|| "?".to_owned(), |ip| ip.to_string()),
        session::v026g::replays::token_tag(token),
        request.range
    )
}

/// The failed `/replays/` lookups of one address.
struct ReplayFailures {
    count: u32,
    since: Instant,
    blocked_until: Option<Instant>,
}

/// The file to hand out at `/download`, if one is set and there.
fn available_download() -> Option<&'static PathBuf> {
    CONNECT
        .get()?
        .download_file
        .as_ref()
        .filter(|file| file.is_file())
}

/// A `Range` header against a file of `size` bytes: `Some(None)` when there is
/// none (the whole file), `Some(Some((first, last)))` for one range (inclusive),
/// `None` when it cannot be met. Only a single `bytes=` range is understood;
/// anything else gets the whole file.
fn parse_range(header: &str, size: u64) -> Option<Option<(u64, u64)>> {
    let Some(spec) = header.trim().strip_prefix("bytes=") else {
        return Some(None);
    };
    if spec.contains(',') {
        return Some(None);
    }
    let (first, last) = spec.split_once('-')?;
    if size == 0 {
        return None;
    }
    let (first, last) = if first.is_empty() {
        // The last N bytes.
        let count: u64 = last.trim().parse().ok().filter(|count| *count > 0)?;
        (size.saturating_sub(count), size - 1)
    } else {
        let first: u64 = first.trim().parse().ok()?;
        let last = if last.trim().is_empty() {
            size - 1
        } else {
            last.trim().parse::<u64>().ok()?.min(size - 1)
        };
        (first, last)
    };
    (first <= last && first < size).then_some(Some((first, last)))
}

struct Web {
    store: Arc<SessionStore>,
    token: Option<String>,
    /// `/<SRP_ADMIN_PATH>`; the admin page is off without it.
    admin_path: Option<String>,
    /// Who may see the admin page.
    admin_ips: Vec<Net>,
    /// Take the client's address from `X-Forwarded-For` (the last entry:
    /// the one a reverse proxy added), for a proxy in front.
    trust_proxy: bool,
    token_failures: Mutex<(u32, Option<Instant>)>,
    connections: AtomicU32,
    downloads: AtomicU32,
    /// Downloads of `/resources/` blobs now.
    resource_downloads: AtomicU32,
    /// Downloads of `/replays/` files now: slots of their own.
    replay_downloads: AtomicU32,
    /// Failed `/replays/` lookups by address ([`REPLAY_FAILURES`]).
    replay_failures: Mutex<HashMap<Option<IpAddr>, ReplayFailures>>,
    /// `Some`: the page of the 0.26g stack.
    game: Option<Game>,
}

/// Who may use the admin page, and how.
#[derive(Clone, Debug, Default)]
pub struct Admin {
    /// `SRP_ADMIN_TOKEN`; the admin page is off without it.
    pub token: Option<String>,
    /// `/<SRP_ADMIN_PATH>`; the admin page is off without it.
    pub path: Option<String>,
    /// `SRP_ADMIN_IPS`.
    pub ips: Vec<Net>,
    /// `SRP_WEB_TRUST_PROXY=on`.
    pub trust_proxy: bool,
}

/// The 0.21d page, as the environment asks for it.
pub fn run(store: Arc<SessionStore>) -> io::Result<()> {
    run_from_env(store, None)
}

/// The 0.26g page, as the environment asks for it (the same settings).
pub fn run_026g(store: Arc<SessionStore>, game: Game) -> io::Result<()> {
    run_from_env(store, Some(game))
}

fn run_from_env(store: Arc<SessionStore>, game: Option<Game>) -> io::Result<()> {
    let bind = std::env::var("SRP_WEB_BIND").unwrap_or_else(|_| "127.0.0.1".into());
    let port = std::env::var("SRP_WEB_PORT").unwrap_or_else(|_| "8088".into());
    let address = vostok::config::load().map_or_else(
        |_| "127.0.0.1:1234".into(),
        |config| config.login_server.public_addr(),
    );
    log::info!("Home page tells players to connect to {address}");
    let _ = CONNECT.set(Connect {
        address,
        download_url: env_text("SRP_DOWNLOAD_URL").filter(|url| url.starts_with("https://")),
        download_file: env_text("SRP_DOWNLOAD_FILE").map(PathBuf::from),
        sha256: env_text("SRP_DOWNLOAD_SHA256")
            .filter(|hash| hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit())),
        other_url: env_text("SRP_OTHER_SERVER_URL").and_then(|url| other_url(&url)),
        game_url: env_text("SRP_GAME_URL").filter(|url| url.starts_with("https://")),
        test_server: env_text("SRP_TEST_SERVER")
            .is_some_and(|value| value.eq_ignore_ascii_case("on")),
    });
    let token = std::env::var("SRP_ADMIN_TOKEN")
        .ok()
        .map(|token| token.trim().to_owned())
        .filter(|token| !token.is_empty());
    let admin_path = std::env::var("SRP_ADMIN_PATH")
        .ok()
        .map(|path| path.trim().trim_matches('/').to_owned())
        .filter(|path| !path.is_empty())
        .map(|path| format!("/{path}"));
    let admin_ips: Vec<Net> = match std::env::var("SRP_ADMIN_IPS") {
        Ok(list) => list.split(',').filter_map(Net::parse).collect(),
        Err(_) => ["127.0.0.1", "::1"]
            .into_iter()
            .filter_map(Net::parse)
            .collect(),
    };
    if let Some(file) = CONNECT
        .get()
        .and_then(|connect| connect.download_file.as_ref())
    {
        match std::fs::metadata(file) {
            Ok(meta) if meta.is_file() => {
                log::info!(
                    "Downloads: {} ({} bytes) at /download",
                    file.display(),
                    meta.len()
                );
            }
            _ => log::warn!(
                "SRP_DOWNLOAD_FILE {} is not a file; /download is off",
                file.display()
            ),
        }
    }
    let trust_proxy = std::env::var("SRP_WEB_TRUST_PROXY")
        .is_ok_and(|value| value.trim().eq_ignore_ascii_case("on"));
    let listener = TcpListener::bind(format!("{bind}:{port}"))?;
    serve_on(
        &listener,
        store,
        Admin {
            token,
            path: admin_path,
            ips: admin_ips,
            trust_proxy,
        },
        game,
    )
}

/// Serve the page on `listener` until it fails: the 0.26g page with a
/// `game`, else the 0.21d one. (What the home page tells of the address and
/// the download comes from the environment, through [`run`] / [`run_026g`]
/// only.)
pub fn serve_on(
    listener: &TcpListener,
    store: Arc<SessionStore>,
    admin: Admin,
    game: Option<Game>,
) -> io::Result<()> {
    let admin_state = match (&admin.token, &admin.path) {
        (Some(_), Some(_)) => format!("admin on for {} address(es)", admin.ips.len()),
        _ => "admin off: SRP_ADMIN_TOKEN and SRP_ADMIN_PATH are needed".to_owned(),
    };
    log::info!(
        "Web server on {} ({} page, {admin_state})",
        listener.local_addr()?,
        if game.is_some() { "0.26g" } else { "0.21d" }
    );
    let web = Arc::new(Web {
        store,
        token: admin.token,
        admin_path: admin.path,
        admin_ips: admin.ips,
        trust_proxy: admin.trust_proxy,
        token_failures: Mutex::new((0, None)),
        connections: AtomicU32::new(0),
        downloads: AtomicU32::new(0),
        resource_downloads: AtomicU32::new(0),
        replay_downloads: AtomicU32::new(0),
        replay_failures: Mutex::new(HashMap::new()),
        game,
    });
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        if web.connections.fetch_add(1, Ordering::AcqRel) >= MAX_CONNECTIONS {
            web.connections.fetch_sub(1, Ordering::AcqRel);
            continue;
        }
        let spawned = thread::Builder::new().name("web-connection".into()).spawn({
            let web = web.clone();
            move || {
                web.serve(stream);
                web.connections.fetch_sub(1, Ordering::AcqRel);
            }
        });
        if spawned.is_err() {
            web.connections.fetch_sub(1, Ordering::AcqRel);
        }
    }
    Ok(())
}

struct Request {
    method: String,
    /// The target before any `?`.
    path: String,
    /// The target exactly as sent (`/replays/` takes nothing else).
    target: String,
    lang: Lang,
    /// `X-Forwarded-For`, as sent.
    forwarded: String,
    /// `Range`, as sent.
    range: String,
    /// Who is asking: the connection's address (see `Web::trust_proxy`).
    client: Option<IpAddr>,
    form: Vec<(String, String)>,
}

impl Request {
    fn field(&self, name: &str) -> &str {
        self.form
            .iter()
            .find(|(key, _)| key == name)
            .map_or("", |(_, value)| value.trim())
    }
}

#[derive(Clone, Copy)]
enum ContentType {
    Html,
    Json,
    Text,
}

impl ContentType {
    fn as_str(self) -> &'static str {
        match self {
            Self::Html => "text/html; charset=utf-8",
            Self::Json => "application/json",
            Self::Text => "text/plain; charset=utf-8",
        }
    }
}

struct Response {
    status: &'static str,
    content_type: ContentType,
    body: String,
}

fn locked<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl Web {
    fn serve(&self, mut stream: TcpStream) {
        let _ = stream.set_read_timeout(Some(IO_TIMEOUT));
        let _ = stream.set_write_timeout(Some(IO_TIMEOUT));
        let peer = stream
            .peer_addr()
            .map(|address| address.to_string())
            .unwrap_or_default();
        let Some(mut request) = read_request(&mut stream) else {
            return;
        };
        let connection = stream.peer_addr().ok().map(|address| address.ip());
        request.client = if self.trust_proxy {
            request
                .forwarded
                .rsplit(',')
                .next()
                .and_then(|last| last.trim().parse().ok())
        } else {
            connection
        };
        if self.game.is_some() && replay_route(&request) {
            self.replay(&mut stream, &request, Instant::now());
            return;
        }
        if request.path == "/download"
            && matches!(request.method.as_str(), "GET" | "HEAD")
            && let Some(file) = available_download()
        {
            self.download(&mut stream, &request, file);
            return;
        }
        if let Some(file) = resource_request(
            self.game.is_some(),
            &request,
            available_download().map(PathBuf::as_path),
        ) {
            self.resource(&mut stream, &request, &file);
            return;
        }
        // The calculator's and the Maps tab's files on the 0.26g site; the
        // calculator's fonts, which every page uses, on both.
        let site_file = self.game.is_some() || request.path.starts_with(damage::FONTS);
        if site_file && matches!(request.method.as_str(), "GET" | "HEAD") {
            let file = damage::file(&request.path)
                .or_else(|| self.game.as_ref().and_then(|_| maps::file(&request.path)));
            if let Some((content_type, bytes)) = file {
                damage::send(&mut stream, request.method == "HEAD", content_type, bytes);
                return;
            }
        }
        let response = self.route(&request, &peer);
        let policy = if self.game.is_some() && launcher::serves(&request.path) {
            launcher::policy()
        } else if self.game.is_some() && request.path == damage::PATH {
            damage::POLICY
        } else if self.game.is_some() && request.path == maps::PATH {
            maps::POLICY
        } else {
            POLICY
        };
        let head = format!(
            "HTTP/1.0 {}\r\nContent-Type: {}\r\n\
             Content-Length: {}\r\nCache-Control: no-store\r\n\
             X-Frame-Options: DENY\r\nX-Content-Type-Options: nosniff\r\n\
             Referrer-Policy: no-referrer\r\n\
             Content-Security-Policy: {policy}\r\nConnection: close\r\n\r\n",
            response.status,
            response.content_type.as_str(),
            response.body.len()
        );
        let _ = stream.write_all(head.as_bytes());
        let _ = stream.write_all(response.body.as_bytes());
        let _ = stream.flush();
        let _ = stream.shutdown(Shutdown::Write);
    }

    /// Hands out the download file: whole, or from a `Range` (so a broken
    /// download resumes). At most [`MAX_DOWNLOADS`] at once.
    fn download(&self, stream: &mut TcpStream, request: &Request, file: &Path) {
        self.send_file(stream, request, file, &self.downloads, "application/zip");
    }

    /// Hands out a blob of `/resources/` as the download file is handed
    /// out, in slots of its own; "not found" when there is no such file.
    fn resource(&self, stream: &mut TcpStream, request: &Request, file: &Path) {
        self.send_file(
            stream,
            request,
            file,
            &self.resource_downloads,
            "application/octet-stream",
        );
    }

    /// `/replays/<token>`: the replay file of the token's match, as the
    /// download file is handed out (a `Range` resumes), in slots of its own;
    /// each `GET` takes one of the token's downloads, and a token has one
    /// download under way at a time ([`REPLAY_BUSY`] for another). Anything
    /// else, an unknown, expired or used-up token among it, is
    /// [`REPLAY_NOT_FOUND`] and counts against the address
    /// ([`REPLAY_FAILURES`]). A log line shows six hex digits of a token at
    /// the most ([`replay_log_line`]).
    fn replay(&self, stream: &mut TcpStream, request: &Request, now: Instant) {
        if self.replay_blocked(request.client, now) {
            let _ = stream.write_all(
                b"HTTP/1.0 429 Too Many Requests\r\nRetry-After: 60\r\nContent-Length: 0\r\n\
                  Connection: close\r\n\r\n",
            );
            return;
        }
        let replays = self.game.as_ref().and_then(|game| game.replays.as_deref());
        let token = replay_token(&request.method, &request.target);
        let taken = match replays.zip(token) {
            Some((replays, token)) => replays
                .take_download(token, request.method == "GET", now)
                .map(|download| (replays, token, download)),
            None => Err(Refused::Unknown),
        };
        let (replays, token, download) = match taken {
            Ok(taken) => taken,
            // A live token whose download is under way: one at a time, so
            // that no account fills the slots. Not a failed lookup.
            Err(Refused::Busy) => {
                let _ = stream.write_all(REPLAY_BUSY);
                return;
            }
            Err(Refused::Unknown) => {
                self.replay_failed(request.client, now);
                let _ = stream.write_all(REPLAY_NOT_FOUND);
                return;
            }
        };
        let match_id = download.match_id;
        let Some(file) = replay_file(replays, match_id) else {
            log::warn!(
                "Replays: token {}… names match {match_id:#x}, whose replay is not there",
                session::v026g::replays::token_tag(token)
            );
            let _ = stream.write_all(REPLAY_NOT_FOUND);
            return;
        };
        log::info!("{}", replay_log_line(request, match_id, token));
        self.send_file(
            stream,
            request,
            &file,
            &self.replay_downloads,
            "application/octet-stream",
        );
        // The token's one download is over.
        drop(download);
    }

    /// Whether an address is waiting out its failed `/replays/` lookups.
    fn replay_blocked(&self, client: Option<IpAddr>, now: Instant) -> bool {
        locked(&self.replay_failures)
            .get(&client)
            .and_then(|failures| failures.blocked_until)
            .is_some_and(|until| now < until)
    }

    /// One failed `/replays/` lookup of an address: past [`REPLAY_FAILURES`]
    /// in a minute it is blocked for a minute.
    fn replay_failed(&self, client: Option<IpAddr>, now: Instant) {
        let mut all = locked(&self.replay_failures);
        if all.len() >= REPLAY_FAILURE_ADDRESSES && !all.contains_key(&client) {
            all.retain(|_, failures| {
                failures.blocked_until.is_some_and(|until| now < until)
                    || now.saturating_duration_since(failures.since) < REPLAY_FAILURE_WINDOW
            });
            if all.len() >= REPLAY_FAILURE_ADDRESSES {
                all.retain(|_, failures| failures.blocked_until.is_some_and(|until| now < until));
            }
        }
        let failures = all.entry(client).or_insert(ReplayFailures {
            count: 0,
            since: now,
            blocked_until: None,
        });
        if failures.blocked_until.is_some_and(|until| now >= until)
            || now.saturating_duration_since(failures.since) >= REPLAY_FAILURE_WINDOW
        {
            *failures = ReplayFailures {
                count: 0,
                since: now,
                blocked_until: None,
            };
        }
        failures.count += 1;
        if failures.count >= REPLAY_FAILURES && failures.blocked_until.is_none() {
            failures.blocked_until = Some(now + REPLAY_BLOCK);
            log::warn!(
                "Replays: {} failed {REPLAY_FAILURES} lookups in a minute; answered 429 for a \
                 minute",
                client.map_or_else(|| "?".to_owned(), |ip| ip.to_string())
            );
        }
    }

    /// A file, whole or from a `Range`, while fewer than [`MAX_DOWNLOADS`]
    /// of `slots` are taken.
    fn send_file(
        &self,
        stream: &mut TcpStream,
        request: &Request,
        file: &Path,
        slots: &AtomicU32,
        content_type: &str,
    ) {
        let reply = |stream: &mut TcpStream, status: &str, extra: &str| {
            let _ = stream.write_all(
                format!(
                    "HTTP/1.0 {status}\r\n{extra}Content-Length: 0\r\nConnection: close\r\n\r\n"
                )
                .as_bytes(),
            );
        };
        if slots.fetch_add(1, Ordering::AcqRel) >= MAX_DOWNLOADS {
            slots.fetch_sub(1, Ordering::AcqRel);
            reply(stream, "503 Service Unavailable", "Retry-After: 60\r\n");
            return;
        }
        let _slot = Slot(slots);
        let Ok(mut open) = File::open(file) else {
            reply(stream, "404 Not Found", "");
            return;
        };
        // (A folder opens too, on some systems.)
        let Some(size) = open
            .metadata()
            .ok()
            .filter(|meta| meta.is_file())
            .map(|meta| meta.len())
        else {
            reply(stream, "404 Not Found", "");
            return;
        };
        let (first, last, partial) = match parse_range(&request.range, size) {
            Some(Some((first, last))) => (first, last, true),
            Some(None) => (0, size.saturating_sub(1), false),
            None => {
                reply(
                    stream,
                    "416 Range Not Satisfiable",
                    &format!("Content-Range: bytes */{size}\r\n"),
                );
                return;
            }
        };
        let length = if size == 0 { 0 } else { last - first + 1 };
        let name: String = file
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default()
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || "._-".contains(c) {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let range = if partial {
            format!("Content-Range: bytes {first}-{last}/{size}\r\n")
        } else {
            String::new()
        };
        let head = format!(
            "HTTP/1.0 {}\r\nContent-Type: {content_type}\r\n\
             Content-Disposition: attachment; filename=\"{name}\"\r\n\
             Accept-Ranges: bytes\r\n{range}Content-Length: {length}\r\n\
             X-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n",
            if partial {
                "206 Partial Content"
            } else {
                "200 OK"
            },
        );
        if stream.write_all(head.as_bytes()).is_err() || request.method == "HEAD" || length == 0 {
            return;
        }
        if open.seek(SeekFrom::Start(first)).is_err() {
            return;
        }
        let started = Instant::now();
        let mut sent = 0_u64;
        let mut buffer = vec![0_u8; 64 * 1024];
        while sent < length {
            let want = buffer.len().min((length - sent) as usize);
            let Ok(read) = open.read(&mut buffer[..want]) else {
                return;
            };
            if read == 0 || stream.write_all(&buffer[..read]).is_err() {
                return;
            }
            sent += read as u64;
            let elapsed = started.elapsed();
            if elapsed > DOWNLOAD_GRACE && sent < MIN_DOWNLOAD_RATE * elapsed.as_secs() {
                return;
            }
        }
        let _ = stream.flush();
    }

    fn route(&self, request: &Request, peer: &str) -> Response {
        if let Some(game) = &self.game {
            if request.method == "GET" {
                match request.path.as_str() {
                    "/api/stats" => return stats::response(&self.store, game),
                    "/patches" => return patches::response(&game.patches),
                    "/api/client-files" => return launcher::client_files(),
                    damage::PATH => return damage_page(request.lang),
                    maps::PATH => return maps_page(request.lang),
                    path if path.starts_with(launcher::FRESH) => {
                        return launcher::fresh(
                            launcher::language(path, request.lang),
                            &self.store,
                            game,
                        );
                    }
                    path if path.starts_with(launcher::FRESH_BEFORE) => {
                        return launcher::fresh_before(
                            launcher::language(path, request.lang),
                            &self.store,
                            game,
                        );
                    }
                    path if launcher::serves(path) => {
                        return launcher::page(
                            launcher::language(path, request.lang),
                            &self.store,
                            game,
                        );
                    }
                    _ => {}
                }
            }
            return self.route_026g(request, peer);
        }
        match (request.method.as_str(), request.path.as_str()) {
            ("GET", "/") => page(
                request.lang,
                Form::Home(Stats {
                    registered: self.store.registered_accounts(),
                    online: self.store.players_present(),
                    maintenance: self.store.minutes_to_maintenance(),
                    registrations_open: self.store.registrations_open(),
                    v026g: false,
                    launcher: false,
                    installs_game: false,
                }),
                "",
            ),
            ("GET", path) if self.is_admin_page(path, request) => {
                page(request.lang, Form::Admin(Box::default()), "")
            }
            ("POST", path) if self.is_admin_page(path, request) => self.admin(request, peer),
            _ => Response {
                status: "404 Not Found",
                content_type: ContentType::Html,
                body: page(
                    request.lang,
                    Form::None,
                    &request.lang.tr("Not found.", "Не знайдено."),
                )
                .body,
            },
        }
    }

    /// Whether `path` is the admin page and `request` may see it.
    fn is_admin_page(&self, path: &str, request: &Request) -> bool {
        self.token.is_some()
            && self.admin_path.as_deref() == Some(path)
            && request
                .client
                .is_some_and(|ip| self.admin_ips.iter().any(|net| net.contains(ip)))
    }

    fn admin(&self, request: &Request, peer: &str) -> Response {
        // A script's reset (curl from the owner's server): a line of text
        // back, not the page.
        if request.field("tab") == password_reset::DIRECT_TAB {
            return self.admin_reset_direct(request, peer);
        }
        let lang = request.lang;
        // The tab the form was on says what to do: Enter in any field
        // submits the form, whichever button that presses.
        let g26 = self.game.is_some();
        let tab = match request.field("tab") {
            tab @ ("money" | "ban" | "unban" | "bans" | "maint" | "signups" | "password") => tab,
            tab @ ("account" | "item" | "text") if g26 => tab,
            _ => "money",
        };
        let refused = |message: String| {
            let view = AdminView {
                tab: tab.to_owned(),
                g26,
                ..AdminView::default()
            };
            page(lang, Form::Admin(Box::new(view)), &message)
        };
        match self.check_token(request, peer) {
            Ok(()) => {}
            Err(TokenRefused::Locked) => {
                return refused(lang.tr(
                    "Too many wrong tokens; wait a minute.",
                    "Забагато хибних токенів; зачекайте хвилину.",
                ));
            }
            Err(TokenRefused::Wrong) => {
                return refused(lang.tr("Wrong token.", "Неправильний токен."));
            }
        }
        let given = request.field("token");
        let mut reset = password_reset::ResetView::default();
        let message = match tab {
            "password" => {
                let (message, view) = self.admin_reset_password(request, peer);
                reset = view;
                message
            }
            "ban" => self.admin_ban(request, peer),
            "unban" => self.admin_unban(request, peer),
            "bans" => String::new(),
            "maint" => self.admin_maintenance(request, peer),
            "signups" => self.admin_signups(request, peer),
            "account" => self.admin_account_026g(request, peer),
            "item" => self.admin_item_026g(request, peer),
            "text" => self.admin_text_026g(request, peer),
            _ if g26 => self.admin_money_026g(request, peer),
            _ => self.admin_money(request, peer),
        };
        let view = AdminView {
            token: given.to_owned(),
            tab: tab.to_owned(),
            bans: bans_table(lang, &self.store),
            maintenance: maintenance_line(lang, &self.store),
            registrations_open: self.store.registrations_open(),
            g26,
            admin_text: self.game.as_ref().and_then(|game| game.lobby.admin_text()),
            reset,
        };
        page(lang, Form::Admin(Box::new(view)), &message)
    }

    /// The admin token of `request`, checked: a wrong one counts towards
    /// the lockout, a right one clears it.
    fn check_token(&self, request: &Request, peer: &str) -> Result<(), TokenRefused> {
        if self.token_locked() {
            return Err(TokenRefused::Locked);
        }
        let given = request.field("token");
        let right = self.token.as_deref().unwrap_or_default();
        if right.is_empty() || !same(given.as_bytes(), right.as_bytes()) {
            self.token_failed();
            log::warn!("Admin with a wrong token from {peer}");
            return Err(TokenRefused::Wrong);
        }
        *locked(&self.token_failures) = (0, None);
        Ok(())
    }

    fn admin_money(&self, request: &Request, peer: &str) -> String {
        let lang = request.lang;
        let user = request.field("user").trim();
        let no_such = lang.tr("No such user.", "Такого користувача немає.");
        let Some(account_id) = self.store.find_account(user) else {
            return no_such;
        };
        let (Some(silver), Some(gold)) =
            (delta(request.field("silver")), delta(request.field("gold")))
        else {
            return lang.tr(
                "Silver and gold are whole numbers, + or -, or empty.",
                "Срібло і золото — цілі числа, + або -, або порожньо.",
            );
        };
        let Some(account) = self.store.account(account_id) else {
            return no_such;
        };
        let (silver_now, gold_now) = {
            let mut account = locked(&account);
            let game = &mut account.game;
            game.generic_money = apply(game.generic_money, silver);
            game.premium_money = apply(game.premium_money, gold);
            (game.generic_money, game.premium_money)
        };
        self.store.mark_dirty(account_id);
        self.store.save_soon();
        log::info!(
            "Admin from {peer}: {user:?} silver {silver:+}, gold {gold:+} \
             (now {silver_now} silver, {gold_now} gold)"
        );
        lang.tr(
            &format!("{user}: now {silver_now} silver and {gold_now} gold."),
            &format!("{user}: тепер {silver_now} срібла і {gold_now} золота."),
        )
    }

    /// Open or close registration of new accounts.
    fn admin_signups(&self, request: &Request, peer: &str) -> String {
        let lang = request.lang;
        let open = match request.field("signups") {
            "open" => true,
            "closed" => false,
            _ => return String::new(),
        };
        self.store.set_registrations_open(open);
        log::warn!(
            "Admin from {peer}: registration of new accounts {}",
            if open { "opened" } else { "closed" }
        );
        if open {
            lang.tr(
                "New accounts are taken again.",
                "Нові акаунти знову приймаються.",
            )
        } else {
            lang.tr(
                "New accounts are refused; existing players sign in as before.",
                "Нові акаунти не приймаються; гравці з акаунтом входять як раніше.",
            )
        }
    }

    /// Plan the maintenance `minutes` from now, or cancel it. The clients
    /// are told within a few seconds: from an hour before they show the
    /// minutes left, 30 minutes before they lock Play, and at the time they
    /// quit.
    fn admin_maintenance(&self, request: &Request, peer: &str) -> String {
        let lang = request.lang;
        if !request.field("cancel").is_empty() {
            let was = self.store.maintenance_at().is_some();
            self.store.set_maintenance(None);
            if was {
                log::warn!("Admin from {peer}: cancelled the maintenance");
            }
            return if was {
                lang.tr("Maintenance cancelled.", "Технічні роботи скасовано.")
            } else {
                lang.tr(
                    "No maintenance was planned.",
                    "Технічних робіт не було заплановано.",
                )
            };
        }
        let minutes = request.field("minutes").trim();
        let Some(minutes) = minutes
            .parse::<u64>()
            .ok()
            .filter(|minutes| (1..=MAX_MAINTENANCE_MINUTES).contains(minutes))
        else {
            return lang.tr(
                &format!("Minutes: a whole number from 1 to {MAX_MAINTENANCE_MINUTES}."),
                &format!("Хвилини: ціле число від 1 до {MAX_MAINTENANCE_MINUTES}."),
            );
        };
        let at = unix_now() + minutes * 60;
        self.store.set_maintenance(Some(at));
        log::warn!("Admin from {peer}: maintenance planned in {minutes} min");
        lang.tr(
            &format!(
                "Maintenance starts in {minutes} min, at {} UTC.",
                clock_utc(at)
            ),
            &format!(
                "Технічні роботи почнуться через {minutes} хв, о {} UTC.",
                clock_utc(at)
            ),
        )
    }

    /// Ban an account (and, if asked, the address it last signed in from),
    /// or an address or range.
    fn admin_ban(&self, request: &Request, peer: &str) -> String {
        let lang = request.lang;
        let target = request.field("ban_user").trim();
        let hours = request.field("hours").trim();
        let until = match hours.parse::<u64>() {
            _ if hours.is_empty() => None,
            Ok(0) => None,
            Ok(hours) if hours <= MAX_BAN_HOURS => Some(unix_now() + hours * 3600),
            _ => {
                return lang.tr(
                    &format!("Hours: a whole number up to {MAX_BAN_HOURS}, or empty for good."),
                    &format!("Години: ціле число до {MAX_BAN_HOURS}, або порожньо — назавжди."),
                );
            }
        };
        let reason: String = request.field("reason").trim().chars().take(200).collect();
        let ban = Ban { until, reason };
        let how_long = match until {
            Some(_) => lang.tr(&format!("for {hours} h"), &format!("на {hours} год")),
            None => lang.tr("for good", "назавжди"),
        };
        if let Some(net) = Net::parse(target) {
            let kicked = self.store.ban_net(net, ban.clone());
            log::warn!(
                "Admin from {peer}: banned address {net} {how_long} ({:?}); \
                 {kicked} session(s) ended",
                ban.reason
            );
            return lang.tr(
                &format!("Address {net} banned {how_long}; {kicked} session(s) ended."),
                &format!("Адресу {net} заблоковано {how_long}; завершено сесій: {kicked}."),
            );
        }
        let Some(account_id) = self.store.find_account(target) else {
            return lang.tr(
                "No such user or address. A name several accounts share needs the user name they sign in with.",
                "Немає такого користувача чи адреси. Якщо ім'я мають кілька акаунтів, вкажіть ім'я для входу.",
            );
        };
        let kicked = self.store.ban_account(account_id, ban.clone());
        log::warn!(
            "Admin from {peer}: banned account {target:?} ({account_id}) {how_long} ({:?}); \
             {kicked} session(s) ended",
            ban.reason
        );
        let mut message = lang.tr(
            &format!("{target} banned {how_long}; {kicked} session(s) ended."),
            &format!("{target} заблоковано {how_long}; завершено сесій: {kicked}."),
        );
        if request.field("with_ip") == "on" {
            let last_ip = self
                .store
                .account(account_id)
                .and_then(|account| account.locked().last_ip);
            match last_ip {
                Some(ip) => {
                    let net = Net::single(ip);
                    let more = self.store.ban_net(net, ban);
                    log::warn!(
                        "Admin from {peer}: banned address {net} too; {more} more session(s) ended"
                    );
                    message += &lang.tr(
                        &format!(" Its address {net} too ({more} more session(s) ended)."),
                        &format!(" І його адресу {net} (ще завершено сесій: {more})."),
                    );
                }
                None => {
                    message += &lang.tr(
                        " Its address is not known, so no address was banned.",
                        " Його адреса невідома, тож адресу не заблоковано.",
                    );
                }
            }
        }
        message
    }

    fn admin_unban(&self, request: &Request, peer: &str) -> String {
        let lang = request.lang;
        let target = request.field("unban_user").trim();
        if let Some(net) = Net::parse(target) {
            return if self.store.unban_net(net) {
                log::warn!("Admin from {peer}: lifted the ban of address {net}");
                lang.tr(
                    &format!("Address {net} unbanned."),
                    &format!("Адресу {net} розблоковано."),
                )
            } else {
                lang.tr(
                    &format!("{net} was not banned (a range is lifted as it was banned)."),
                    &format!(
                        "{net} не була заблокована (діапазон знімається таким, яким його заблокували)."
                    ),
                )
            };
        }
        let Some(account_id) = self.store.find_account(target) else {
            return lang.tr(
                "No such user or address.",
                "Немає такого користувача чи адреси.",
            );
        };
        let mut message = if self.store.unban_account(account_id) {
            log::warn!("Admin from {peer}: lifted the ban of account {target:?} ({account_id})");
            lang.tr(
                &format!("{target} unbanned."),
                &format!("{target} розблоковано."),
            )
        } else {
            lang.tr(
                &format!("{target} was not banned."),
                &format!("{target} не був заблокований."),
            )
        };
        if request.field("unban_ip") == "on"
            && let Some(ip) = self
                .store
                .account(account_id)
                .and_then(|account| account.locked().last_ip)
            && self.store.unban_net(Net::single(ip))
        {
            log::warn!("Admin from {peer}: lifted the ban of address {ip}");
            message += &lang.tr(
                &format!(" Its address {ip} too."),
                &format!(" І його адресу {ip}."),
            );
        }
        message
    }

    fn token_locked(&self) -> bool {
        let mut failures = locked(&self.token_failures);
        match failures.1 {
            Some(last) if last.elapsed() >= TOKEN_LOCKOUT => {
                *failures = (0, None);
                false
            }
            _ => failures.0 >= MAX_TOKEN_FAILURES,
        }
    }

    fn token_failed(&self) {
        let mut failures = locked(&self.token_failures);
        failures.0 += 1;
        failures.1 = Some(Instant::now());
    }
}

/// A signed whole number, or nothing for 0.
fn delta(text: &str) -> Option<i64> {
    if text.is_empty() {
        return Some(0);
    }
    text.parse::<i64>()
        .ok()
        .filter(|number| number.abs() <= 1_000_000_000)
}

/// `balance` changed by `change`, not below 0.
fn apply(balance: u32, change: i64) -> u32 {
    u32::try_from((i64::from(balance) + change).max(0)).unwrap_or(u32::MAX)
}

/// Equal, in time that does not tell where they differ.
fn same(a: &[u8], b: &[u8]) -> bool {
    let mut difference = a.len() ^ b.len();
    for (x, y) in a.iter().zip(b) {
        difference |= usize::from(x ^ y);
    }
    difference == 0
}

fn read_request(stream: &mut TcpStream) -> Option<Request> {
    let mut data = Vec::new();
    let mut chunk = [0_u8; 1024];
    let head_end = loop {
        if let Some(at) = data.windows(4).position(|window| window == b"\r\n\r\n") {
            break at + 4;
        }
        if data.len() > MAX_REQUEST {
            return None;
        }
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => return None,
            Ok(n) => data.extend_from_slice(&chunk[..n]),
        }
    };
    let head = String::from_utf8_lossy(&data[..head_end]).into_owned();
    let mut lines = head.split("\r\n");
    let mut first = lines.next()?.split_whitespace();
    let method = first.next()?.to_owned();
    let target = first.next()?;
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let path = path.to_owned();
    let accept = head
        .split("\r\n")
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _)| name.eq_ignore_ascii_case("accept-language"))
        .map_or("", |(_, value)| value.trim())
        .to_owned();
    let forwarded = head
        .split("\r\n")
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _)| name.eq_ignore_ascii_case("x-forwarded-for"))
        .map_or("", |(_, value)| value.trim())
        .to_owned();
    let range = head
        .split("\r\n")
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _)| name.eq_ignore_ascii_case("range"))
        .map_or("", |(_, value)| value.trim())
        .to_owned();
    let length: usize = lines
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, value)| value.trim().parse().ok())
        .unwrap_or(0);
    if length > MAX_REQUEST {
        return None;
    }
    while data.len() < head_end + length {
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => return None,
            Ok(n) => data.extend_from_slice(&chunk[..n]),
        }
    }
    let body = String::from_utf8_lossy(&data[head_end..head_end + length]).into_owned();
    let pairs = |text: &str| -> Vec<(String, String)> {
        text.split('&')
            .filter_map(|pair| pair.split_once('='))
            .map(|(key, value)| (decode(key), decode(value)))
            .collect()
    };
    let form = pairs(&body);
    let chosen = form
        .iter()
        .chain(pairs(query).iter())
        .find(|(key, _)| key == "lang")
        .map_or("", |(_, value)| value.as_str())
        .to_owned();
    Some(Request {
        method,
        path,
        target: target.to_owned(),
        lang: Lang::pick(&chosen, &accept),
        forwarded,
        range,
        client: None,
        form,
    })
}

/// `application/x-www-form-urlencoded` text.
fn decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        match bytes[at] {
            b'+' => out.push(b' '),
            b'%' => {
                let byte = text
                    .get(at + 1..at + 3)
                    .and_then(|hex| u8::from_str_radix(hex, 16).ok());
                match byte {
                    Some(byte) => {
                        out.push(byte);
                        at += 2;
                    }
                    None => out.push(b'%'),
                }
            }
            byte => out.push(byte),
        }
        at += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// `SRP_OTHER_SERVER_URL` as the pages use it: a plain `https://` address
/// without a query, or nothing.
fn other_url(url: &str) -> Option<String> {
    let url = url.trim().trim_end_matches('/');
    let host = url.strip_prefix("https://")?;
    let plain = !host.is_empty()
        && host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-._~/:".contains(&b));
    plain.then(|| url.to_owned())
}

/// The header's link to the server for the other game version (`version`),
/// in the page's language; empty without `SRP_OTHER_SERVER_URL`.
fn other_server_link(lang: Lang, version: &str, url: Option<&str>) -> String {
    url.map_or_else(String::new, |url| {
        format!(
            " <a href=\"{}/?lang={}\">Survarium {version}</a>",
            escape(url),
            lang.code()
        )
    })
}

/// The page's title: a test server says so (`SRP_TEST_SERVER`).
fn page_title(lang: Lang, version: &str, test: bool) -> String {
    match (test, lang) {
        (false, Lang::En) => format!("Skhid · Survarium {version} community server"),
        (false, Lang::Uk) => format!("Схід · ком'юніті сервер Survarium {version}"),
        (true, Lang::En) => format!("Skhid · Survarium {version} TEST server"),
        (true, Lang::Uk) => format!("Схід · ТЕСТОВИЙ сервер Survarium {version}"),
    }
}

/// The line beside the name in the header.
fn page_tag(lang: Lang, version: &str, test: bool) -> String {
    let version = version.to_uppercase();
    match (test, lang) {
        (false, Lang::En) => {
            format!("SKHID · COMMUNITY SERVER FOR SURVARIUM {version} · UNOFFICIAL")
        }
        (false, Lang::Uk) => format!("КОМ'ЮНІТІ СЕРВЕР SURVARIUM {version} · НЕОФІЦІЙНИЙ"),
        (true, Lang::En) => format!("SKHID · TEST SERVER FOR SURVARIUM {version} · UNOFFICIAL"),
        (true, Lang::Uk) => format!("ТЕСТОВИЙ СЕРВЕР SURVARIUM {version} · НЕОФІЦІЙНИЙ"),
    }
}

/// What a test server puts at the top of every page: what it is, and where
/// the players' server is (`SRP_OTHER_SERVER_URL`, when set).
fn test_server_notice(lang: Lang, players_url: Option<&str>) -> String {
    let players = players_url.map_or_else(String::new, |url| {
        format!(
            " {} <a href=\"{}/?lang={}\">{}</a>.",
            lang.tr("The players' server:", "Сервер для гравців:"),
            escape(url),
            lang.code(),
            escape(url.trim_start_matches("https://")),
        )
    });
    format!(
        "<p class=\"note test\"><b>{}</b> {}{players}</p>",
        lang.tr("Test server.", "Тестовий сервер."),
        lang.tr(
            "Changes are tried here before they reach the players: it can restart at any \
             moment, and accounts and progress here can be reset.",
            "Тут перевіряються зміни, перш ніж вони потраплять до гравців: сервер може \
             перезапуститися будь-якої миті, а облікові записи й прогрес можуть бути скинуті.",
        ),
    )
}

fn page(lang: Lang, form: Form, message: &str) -> Response {
    let connect = CONNECT.get();
    let test = connect.is_some_and(|connect| connect.test_server);
    let other_url = connect.and_then(|connect| connect.other_url.as_deref());
    page_as(lang, form, message, test, other_url)
}

/// The calculator: its page under the site's header.
fn damage_page(lang: Lang) -> Response {
    damage::page(lang, &tool_header(lang, damage::PATH))
}

/// The Maps tab: its page under the site's header.
fn maps_page(lang: Lang) -> Response {
    maps::page(lang, &tool_header(lang, maps::PATH))
}

/// The site's header over a page of its own look (the calculator's, the
/// Maps tab's), `path` the page shown.
fn tool_header(lang: Lang, path: &str) -> String {
    let connect = CONNECT.get();
    let test = connect.is_some_and(|connect| connect.test_server);
    let other_url = connect.and_then(|connect| connect.other_url.as_deref());
    let (header, test_notice) = site_header(lang, true, test, other_url, path);
    format!(
        "<div class=site><style>{}</style>{header}{test_notice}</div>",
        damage::HEADER_STYLE
    )
}

/// Every page's header (the 0.26g site's or the 0.21d's), and what a test
/// server says under it. `current`: the path of the tab shown, if one is.
fn site_header(
    lang: Lang,
    v026g: bool,
    test: bool,
    other_url: Option<&str>,
    current: &str,
) -> (String, String) {
    let code = lang.code();
    let (other, other_name) = match lang {
        Lang::En => ("uk", "Українська"),
        Lang::Uk => ("en", "English"),
    };
    let version = if v026g { "0.26g" } else { "0.21d" };
    let other_version = if v026g { "0.21d" } else { "0.26g" };
    // A test server names the other server in its notice, as the players'.
    let (other_link, badge, test_notice) = if test {
        (
            String::new(),
            format!(" <span class=badge>{}</span>", lang.tr("TEST", "ТЕСТ")),
            test_server_notice(lang, other_url),
        )
    } else {
        (
            other_server_link(lang, other_version, other_url),
            String::new(),
            String::new(),
        )
    };
    // The calculator and the maps are the 0.26g game's.
    let tab = |path: &str, name: &str| {
        format!(
            "<a href=\"{path}?lang={code}\"{}>{name}</a>",
            if current == path {
                " aria-current=page"
            } else {
                ""
            },
        )
    };
    let home_link = tab("/", &lang.tr("Home", "Головна"));
    let damage_link = if v026g {
        tab(damage::PATH, &lang.tr("Calculator", "Калькулятор"))
            + &tab(maps::PATH, &lang.tr("Maps", "Мапи"))
    } else {
        String::new()
    };
    let header = format!(
        "<header><a class=brand href=\"/?lang={code}\">СХІД</a>{badge}\
         <span class=tag>{}</span>\
         <nav>{home_link}{damage_link}{other_link}</nav> <a class=lang href=\"?lang={other}\">{other_name}</a></header>",
        page_tag(lang, version, test),
    );
    (header, test_notice)
}

/// `page` with what the settings say given to it.
fn page_as(lang: Lang, form: Form, message: &str, test: bool, other_url: Option<&str>) -> Response {
    let notice = if message.is_empty() {
        String::new()
    } else {
        format!("<p class=note>{}</p>", escape(message))
    };
    let code = lang.code();
    let version = if form.is_026g() { "0.26g" } else { "0.21d" };
    let current = if matches!(form, Form::Home(_)) {
        "/"
    } else {
        ""
    };
    let (header, test_notice) = site_header(lang, form.is_026g(), test, other_url, current);
    let body = match form {
        Form::InTheGame26(code) => v026g::in_the_game_now(code),
        Form::Admin(view) => admin_form(lang, &view),
        Form::Home(stats) => home(lang, stats),
        Form::None | Form::None26 => String::new(),
    };
    let disclaimer = lang.tr(
        "Community server, unofficial. Not affiliated with or endorsed by Vostok Games. \
         Survarium is a trademark of its owners.",
        "Неофіційний ком'юніті сервер. Не пов'язаний із Vostok Games і не схвалений ними. \
         Survarium є торговельною маркою її власників.",
    );
    Response {
        status: "200 OK",
        content_type: ContentType::Html,
        body: format!(
            "<!doctype html><html lang={code}><meta charset=utf-8>\
             <meta name=viewport content=\"width=device-width,initial-scale=1\">\
             <title>{}</title><style>{}{STYLE}</style>\
             {header}\
             <main>{test_notice}{notice}{body}</main>\
             <footer>{}</footer></html>",
            escape(&page_title(lang, version, test)),
            damage::font_faces(),
            escape(&disclaimer),
        ),
    }
}

/// The site's look: the damage calculator's (paper and ink, PT Sans, PT Sans
/// Narrow headings, PT Mono numbers), light or dark as the system is, the
/// header too: on every page, the calculator's and the maps' included
/// ([`damage::HEADER_STYLE`]).
const STYLE: &str = ":root{--bg:#e9e6dc;--panel:#f5f3ec;--ink:#1f2420;--ink-2:#4d544c;--ink-3:#7b8277;--rule:#c9c5b6;--accent:#b4501f;--accent-ink:#fff8f0;--bad:#a3322a;--code:#ece8dc;\
--display:'PT Sans Narrow','Arial Narrow',sans-serif;--body:'PT Sans','Segoe UI',Roboto,sans-serif;--data:'PT Mono',ui-monospace,Consolas,monospace}\
@media (prefers-color-scheme:dark){:root{--bg:#171a17;--panel:#1f231f;--ink:#e6e4da;--ink-2:#b4b6a8;--ink-3:#868a7e;--rule:#353a33;--accent:#e07a43;--accent-ink:#1a0f08;--bad:#e0675c;--code:#151815;color-scheme:dark}}\
*{box-sizing:border-box}body{font:15px/1.55 var(--body);background:var(--bg);color:var(--ink);margin:0;min-height:100vh}\
header{display:flex;flex-wrap:wrap;align-items:center;gap:.5rem 1.4rem;padding:.75rem 16px;background:var(--bg);border-bottom:2px solid var(--ink)}\
.brand{font:700 1.5rem var(--display);letter-spacing:.24em;color:var(--accent);text-decoration:none}.tag{font:.72rem var(--data);color:var(--ink-3);flex:1;min-width:12rem}\
nav{display:flex;flex-wrap:wrap;align-items:center;row-gap:.3rem}nav a{font:700 .85rem var(--display);text-transform:uppercase;letter-spacing:.1em;color:var(--ink-2);text-decoration:none;padding:0 .8rem;line-height:1.1}\
nav a+a{border-left:2px solid var(--ink-3)}nav a:first-child{padding-left:0}nav a:hover{color:var(--ink)}nav a[aria-current]{color:var(--accent)}\
.lang{font:.8rem var(--body);color:var(--ink-2);text-decoration:none;border:1px solid var(--rule);background:var(--bg);padding:.15rem .7rem;border-radius:999px}.lang:hover{border-color:var(--ink-3);color:var(--ink)}\
main{max-width:46rem;margin:22px auto;padding:0 16px;display:grid;gap:22px}a{color:var(--accent)}\
h1,h2,h3{font-family:var(--display);font-weight:700;margin:0;text-transform:uppercase;letter-spacing:.06em;color:var(--ink)}h1{font-size:1.35rem}h2{font-size:1.15rem}\
.card{background:var(--panel);border:1px solid var(--rule);padding:16px;display:grid;gap:12px;align-content:start;min-width:0}\
.card>h1:first-child,.card>h2:first-child{border-bottom:1px solid var(--rule);padding-bottom:8px}.card p{margin:0}\
ol{padding-left:1.3rem;margin:0}.rules{padding-left:1.1rem;margin:0}.opts{font-size:.85rem;color:var(--ink-2);padding-left:1.1rem}li{margin:.6rem 0}li>p{margin:.4rem 0}\
pre,code{background:var(--code);border:1px solid var(--rule);border-radius:2px;color:var(--ink);font:.85rem var(--data)}pre{padding:.6rem .8rem;margin:.4rem 0;white-space:pre-wrap;word-break:break-all}code{padding:.05rem .3rem;word-break:break-all}\
.dl{display:flex;flex-direction:column;align-items:flex-start;gap:.5rem;margin:.6rem 0}.dl .button{margin:0}.hash{display:block;max-width:100%;font-size:.78rem;padding:.35rem .5rem}\
.card .note{margin:.2rem 0 0}.stats{display:flex;gap:2rem;padding-top:10px;border-top:1px solid var(--rule)}.stats div{display:flex;flex-direction:column}\
.stats b{font:400 1.9rem/1.1 var(--data);color:var(--accent)}.stats span{font:700 .72rem var(--display);color:var(--ink-3);text-transform:uppercase;letter-spacing:.1em}\
label{display:block;margin:.9rem 0 .25rem;font:700 .74rem var(--display);letter-spacing:.1em;text-transform:uppercase;color:var(--ink-3)}\
input{width:100%;padding:.5rem .6rem;background:var(--bg);color:inherit;border:1px solid var(--rule);border-radius:2px;font:inherit}\
input:focus{outline:0;border-color:var(--accent);box-shadow:0 0 0 2px color-mix(in srgb,var(--accent) 30%,transparent)}\
button,.button{display:inline-block;margin-top:1rem;padding:.6rem 1.4rem;background:var(--accent);color:var(--accent-ink);border:0;border-radius:2px;font:700 .9rem var(--display);text-transform:uppercase;letter-spacing:.1em;text-decoration:none;cursor:pointer}\
button:hover,.button:hover{filter:brightness(1.08)}.note{background:var(--bg);border-left:3px solid var(--accent);padding:.7rem .9rem;border-radius:2px;color:var(--ink-2)}\
.note.test{border-left-color:var(--bad)}.badge{background:#d64531;color:#fff;font:700 .75rem system-ui,sans-serif;letter-spacing:.14em;padding:.2rem .55rem;border-radius:2px}small{color:var(--ink-3)}\
label.check{display:flex;gap:.5rem;align-items:center;font:inherit;font-size:.85rem;letter-spacing:0;text-transform:none;color:var(--ink-2)}label.check input{width:auto}\
.tabs{margin-top:1.2rem}.tabs>input{position:absolute;opacity:0;pointer-events:none}\
.tabs>label{display:inline-block;margin:0 .25rem 0 0;padding:.45rem .9rem;border:1px solid var(--rule);border-bottom:0;border-radius:2px 2px 0 0;cursor:pointer;color:var(--ink-2);background:var(--bg)}\
.tabs>input:checked+label{color:var(--accent-ink);background:var(--accent);border-color:var(--accent)}.tabs>input:focus-visible+label{outline:2px solid var(--accent)}\
.panel{display:none;border:1px solid var(--rule);border-top:2px solid var(--accent);padding:.2rem 1rem 1rem;background:var(--panel)}\
#tab-money:checked~.p-money,#tab-ban:checked~.p-ban,#tab-unban:checked~.p-unban,#tab-bans:checked~.p-bans,#tab-maint:checked~.p-maint,#tab-signups:checked~.p-signups,#tab-account:checked~.p-account,#tab-item:checked~.p-item,#tab-text:checked~.p-text,#tab-password:checked~.p-password{display:block}\
button.quiet{background:0;color:var(--accent);border:1px solid var(--accent)}button.quiet:hover{background:color-mix(in srgb,var(--accent) 12%,transparent)}\
table{width:100%;border-collapse:collapse;font-size:.85rem}th,td{text-align:left;padding:.35rem .4rem;border-bottom:1px solid var(--rule);vertical-align:top;word-break:break-word}\
th{font:700 .72rem var(--display);letter-spacing:.1em;text-transform:uppercase;color:var(--ink-3)}\
details{margin-top:.6rem}summary{cursor:pointer;font:700 .78rem var(--display);letter-spacing:.1em;text-transform:uppercase;color:var(--ink-2)}\
footer{max-width:46rem;margin:2rem auto;padding:1rem 16px;border-top:1px solid var(--rule);font-size:.75rem;color:var(--ink-3)}";

/// A file's size as the download button shows it.
fn file_size(bytes: u64) -> String {
    if bytes >= 1_000_000_000 {
        format!("{:.2} GB", bytes as f64 / 1e9)
    } else {
        format!("{:.1} MB", bytes as f64 / 1e6)
    }
}

/// The list under "How matches start" for 0.26g: every match is a room's,
/// the players' "servers" (`lobby_server::v026g::rooms`; the owner's of
/// 2026-10-17).
fn room_rules(lang: Lang, squads: bool) -> String {
    let vote = session::v026g::rooms::VOTE.as_secs();
    let cards = session::v026g::rooms::CARDS;
    let mut items = vec![
        lang.tr(
            "Matches are played in rooms in the lobby: join one from the list, make your own (its size, max level, maps, wait and an optional PIN), or press Play.",
            "Матчі грають у кімнатах у лобі: приєднайтеся до однієї зі списку, створіть свою (розмір, макс. рівень, мапи, очікування й PIN за бажанням) або натисніть «Грати».",
        ),
        lang.tr(
            "Play puts you in the open room nearest your level, or makes a new one.",
            "«Грати» ставить вас у відкриту кімнату, найближчу до вашого рівня, або створює нову.",
        ),
        lang.tr(
            &format!(
                "Once a room has its players, they vote for {vote} s between {cards} maps, and the match starts after the room's wait."
            ),
            &format!(
                "Коли в кімнаті досить гравців, вони {vote} с голосують за одну з {cards} мап, і матч починається після очікування кімнати."
            ),
        ),
        lang.tr(
            "Join a room while its match is played: you see the room, then go into the match (on the side with fewer players) until its last 30 s. After a match everyone is back in the room.",
            "Хто приєднується до кімнати під час матчу, бачить кімнату, а потім потрапляє в матч (на сторону, де менше гравців) — до його останніх 30 с. Після матчу всі повертаються в кімнату.",
        ),
        lang.tr(
            "Teams are balanced by gear, level and past results.",
            "Команди вирівнюються за спорядженням, рівнем і результатами минулих матчів.",
        ),
    ];
    if !squads {
        items.push(lang.tr(
            "Squads are off: everyone plays alone.",
            "Загони вимкнено: кожен грає сам.",
        ));
    }
    items
        .iter()
        .map(|item| format!("<li>{item}</li>"))
        .collect()
}

fn home(lang: Lang, stats: Stats) -> String {
    let v026g = stats.v026g;
    let with_launcher = stats.launcher;
    let installs_game = with_launcher && stats.installs_game;
    let connect = CONNECT.get();
    let address = connect.map_or("127.0.0.1:1234", |connect| connect.address.as_str());
    let local = available_download().and_then(|file| std::fs::metadata(file).ok());
    let download_label = if v026g {
        lang.tr("Download the files", "Завантажити файли")
    } else {
        lang.tr("Download the game", "Завантажити гру")
    };
    let download = match connect.and_then(|connect| connect.download_url.as_deref()) {
        _ if local.is_some() => format!(
            "<a class=button href=\"/download\">{download_label} ({})</a>",
            file_size(local.as_ref().map_or(0, |meta| meta.len()))
        ),
        Some(url) => format!(
            "<a class=button href=\"{}\" rel=noopener>{download_label}</a>",
            escape(url),
        ),
        None => format!(
            "<em>{}</em>",
            lang.tr(
                "The download link is coming soon.",
                "Посилання на завантаження скоро з'явиться."
            )
        ),
    };
    let hash = match connect.and_then(|connect| connect.sha256.as_deref()) {
        Some(hash) => format!(
            "<small>SHA-256</small><code class=hash>{}<br>{}</code>",
            escape(&hash.to_ascii_lowercase()[..32]),
            escape(&hash.to_ascii_lowercase()[32..])
        ),
        None => String::new(),
    };
    // 0.26g: the game itself comes from elsewhere (step one); this server
    // hands out only the files that go over it.
    // With a download whose launcher brings the game there is no such
    // step: the link is the way out, in the small print of the download.
    let game_link = connect
        .and_then(|connect| connect.game_url.as_deref())
        .map(|url| {
            format!(
                "<a href=\"{}\" rel=noopener>{}</a>",
                escape(url),
                lang.tr("the original game", "оригінальну гру")
            )
        })
        .unwrap_or_else(|| lang.tr("the original game", "оригінальну гру").to_string());
    let original = if v026g && !installs_game {
        let link = match connect.and_then(|connect| connect.game_url.as_deref()) {
            Some(url) => format!(
                "<div class=dl><a class=button href=\"{}\" rel=noopener>{}</a></div>",
                escape(url),
                lang.tr("Get the game", "Отримати гру")
            ),
            None => format!(
                "<p><em>{}</em></p>",
                lang.tr("The link is coming soon.", "Посилання скоро з'явиться.")
            ),
        };
        format!(
            "<li>{}{link}</li>",
            lang.tr(
                "Get the original Survarium 0.26g (build 2777) and unpack it (a .7z archive: 7-Zip opens it).",
                "Отримайте оригінальну Survarium 0.26g (збірка 2777) і розпакуйте її (архів .7z: відкривається 7-Zip)."
            )
        )
    } else {
        String::new()
    };
    let mut notices = String::new();
    if let Some(minutes) = stats.maintenance {
        notices += &format!(
            "<p class=note><b>{}</b> {}</p>",
            lang.tr("Maintenance.", "Технічні роботи."),
            if minutes == 0 {
                lang.tr(
                    "The server is down for maintenance.",
                    "Сервер на технічних роботах.",
                )
            } else {
                lang.tr(
                    &format!("The game closes in {minutes} min; Play is locked in the last 30."),
                    &format!("Гра закриється через {minutes} хв; за 30 хв «Грати» вимикається."),
                )
            }
        );
    }
    if !stats.registrations_open {
        notices += &format!(
            "<p class=note>{}</p>",
            lang.tr(
                "New registrations are closed for now. Existing players can sign in and play.",
                "Нові реєстрації тимчасово закриті. Гравці з акаунтом можуть входити й грати.",
            )
        );
    }
    let stats = format!(
        "{notices}<div class=stats><div><b>{}</b><span>{}</span></div><div><b>{}</b><span>{}</span></div></div>",
        stats.online,
        lang.tr("players online", "гравців онлайн"),
        stats.registered,
        lang.tr("registered", "зареєстровано"),
    );
    let version = if v026g { "0.26g" } else { "0.21d" };
    // 0.26g: `-server=` (it has no `-client=`), and the exe that takes srp's
    // certificate (`scripts/client-patch-026g`).
    let server_key = if v026g { "-server" } else { "-client" };
    let mut options = format!(
        "<ul class=opts><li><code>{server_key}</code> {}</li><li><code>-localization</code> {}</li>",
        lang.tr("server address", "адреса сервера"),
        lang.tr(
            "language: english, russian, ukrainian",
            "мова: english, russian, ukrainian"
        ),
    );
    if v026g {
        options += &format!(
            "<li><code>-mt_allocator=768</code> {}</li><li><code>-minimum_resources_memory_size=768</code> {}</li>",
            lang.tr(
                "memory for the game (maps need more than the default), MB",
                "пам'ять для гри (мапам замало типової), МБ"
            ),
            lang.tr("memory for resources, MB", "пам'ять для ресурсів, МБ"),
        );
    }
    options += "</ul>";
    let launch = if v026g {
        format!(
            "<pre>survarium_skhid.exe -localization={} -server={} -mt_allocator=768 -minimum_resources_memory_size=768</pre><p><small>{}</small></p>",
            lang.tr("english", "ukrainian"),
            escape(address),
            lang.tr(
                "Run it from the game's <code>game\\binaries\\x86</code> folder (the download's <code>play_skhid.bat</code> does).",
                "Запускайте з теки гри <code>game\\binaries\\x86</code> (це робить <code>play_skhid.bat</code> із завантаження).",
            ),
        )
    } else {
        format!(
            "<pre>survarium.exe -localization={} -client={}</pre>",
            lang.tr("english", "ukrainian"),
            escape(address),
        )
    };
    let command_line = format!(
        "{}{launch}{options}",
        lang.tr(
            "Start it with this command line:",
            "Запустіть гру з такою командою:"
        ),
    );
    // With the launcher in the download a player starts that and types
    // nothing; the start script and the command line stay, folded away.
    let start = if with_launcher {
        format!(
            "{}<p><small>{}</small></p><details><summary>{}</summary><p>{}</p>{command_line}</details>",
            if installs_game {
                lang.tr(
                    "Run <code>Skhid.bat</code> and press <b>PLAY</b>. The first time, the launcher downloads the game (2.9 GB, from Google Drive), which takes a while. It keeps itself up to date.",
                    "Запустіть <code>Skhid.bat</code> і натисніть <b>PLAY</b>. Першого разу лаунчер завантажує гру (2,9 ГБ, з Google Drive), на що потрібен час. Оновлюється він сам.",
                )
            } else {
                lang.tr(
                    "Run <code>Skhid.bat</code> and press <b>PLAY</b>. The launcher keeps itself up to date.",
                    "Запустіть <code>Skhid.bat</code> і натисніть <b>PLAY</b>. Лаунчер оновлюється сам.",
                )
            },
            if installs_game {
                lang.tr(
                    "Windows warns about an unknown publisher? Let it run.<br>Linux: <code>bash Skhid.sh launcher</code>, see <code>LINUX.txt</code>.",
                    "Windows попереджає про невідомого видавця? Дозвольте запуск.<br>Linux: <code>bash Skhid.sh launcher</code>, див. <code>LINUX.txt</code>.",
                )
            } else {
                lang.tr(
                    "Windows warns about an unknown publisher? Let it run.<br>Linux: <code>bash Skhid.sh</code>, see <code>LINUX.txt</code>.",
                    "Windows попереджає про невідомого видавця? Дозвольте запуск.<br>Linux: <code>bash Skhid.sh</code>, див. <code>LINUX.txt</code>.",
                )
            },
            lang.tr("Without the launcher", "Без лаунчера"),
            lang.tr(
                "<code>play_skhid.bat</code> in <code>game\\binaries\\x86</code> starts the game without updating it.",
                "<code>play_skhid.bat</code> у теці <code>game\\binaries\\x86</code> запускає гру без оновлення.",
            ),
        )
    } else {
        command_line
    };
    let window = session::gather_window_from_env()
        .unwrap_or(session::DEFAULT_GATHER_WINDOW)
        .as_secs();
    let fill = session::fill_from_env();
    let quiet = session::quiet_from_env().as_secs();
    let max = session::MAX_PLAYERS_PER_MATCH;
    // 0.26g's lobby can be told to refuse squads (`SRP_SQUADS`).
    let squads = !v026g
        || !matches!(
            std::env::var("SRP_SQUADS")
                .ok()
                .map(|value| value.trim().to_ascii_lowercase())
                .as_deref(),
            Some("off" | "0" | "false" | "no")
        );
    let rules = if v026g {
        room_rules(lang, squads)
    } else {
        format!(
            "<li>{}</li><li>{}</li><li>{}</li><li>{}</li>",
            lang.tr(
                &format!("{max} players: the match starts at once."),
                &format!("{max} гравців: матч починається одразу.")
            ),
            lang.tr(
                &format!(
                    "{fill} to {} players: it starts when nobody has pressed Play for {quiet} s, or when {window} s are up, whichever is first.",
                    max - 1
                ),
                &format!(
                    "від {fill} до {} гравців: матч починається, коли {quiet} с ніхто не натискав «Грати», або через {window} с, що настане раніше.",
                    max - 1
                )
            ),
            lang.tr(
                &format!("2 to {} players: they wait the full {window} s.", fill - 1),
                &format!("від 2 до {} гравців: чекають повні {window} с.", fill - 1)
            ),
            lang.tr(
                "One player alone: no match starts until another joins.",
                "Один гравець: матч не починається, доки не приєднається ще хтось."
            ),
        )
    };
    format!(
        "<section class=card><h1>{}</h1><p>{}</p><p>{}</p><p class=note><b>{}</b> {}</p>{stats}</section> <section class=card><h2>{}</h2><ol> {original}<li>{}<div class=dl>{download}{hash}</div></li> <li>{start}</li> <li>{}</li></ol></section> <section class=card><h2>{}</h2><ul class=rules>{rules}</ul></section>",
        lang.tr("Skhid", "Схід"),
        if squads {
            lang.tr(
                &format!("Skhid (Ukrainian for \u{201c}east\u{201d}) is a small community server that lets the Survarium {version} client sign in, trade, form squads and play matches again."),
                &format!("Схід \u{2014} невеликий ком'юніті сервер, на якому клієнт Survarium {version} знову може входити, торгувати, збирати загони й грати матчі.")
            )
        } else {
            lang.tr(
                &format!("Skhid (Ukrainian for \u{201c}east\u{201d}) is a small community server that lets the Survarium {version} client sign in, trade and play matches again."),
                &format!("Схід \u{2014} невеликий ком'юніті сервер, на якому клієнт Survarium {version} знову може входити, торгувати й грати матчі.")
            )
        },
        lang.tr(
            "Project Discord: <a href=\"https://discord.gg/pxywZfaxRQ\">discord.gg/pxywZfaxRQ</a>",
            "Discord проєкту: <a href=\"https://discord.gg/pxywZfaxRQ\">discord.gg/pxywZfaxRQ</a>"
        ),
        lang.tr("Early test server.", "Тестовий сервер."),
        lang.tr(
            "It has bugs and may not work fully. Your progress, items and accounts can be lost or reset at any time, and the server may be stopped at any time. By playing you accept this.",
            "Можливі помилки та збої. Ваш прогрес, предмети й акаунти можуть зникнути або бути скинуті будь-коли, а сервер можуть зупинити будь-коли. Граючи, ви з цим погоджуєтесь."
        ),
        lang.tr("How to play", "Як зіграти"),
        if installs_game {
            format!(
                "{}<p><small>{}</small></p>",
                lang.tr(
                    "Download our files and unpack them into an empty folder with about 10 GB free. <b>You do this only once.</b>",
                    "Завантажте наші файли й розпакуйте їх у порожню теку, де вільно близько 10 ГБ. <b>Це робиться лише один раз.</b>"
                ),
                lang.tr(
                    &format!("Already have Survarium 0.26g (build 2777)? Unpack into its folder, the one with the <code>game</code> folder. Launcher can't get the game? Get {game_link} and do the same."),
                    &format!("Уже маєте Survarium 0.26g (збірка 2777)? Розпакуйте в її теку, ту, де лежить тека <code>game</code>. Лаунчер не може отримати гру? Завантажте {game_link} і зробіть так само.")
                ),
            )
        } else if with_launcher {
            lang.tr(
                "Download our files and unpack them into the game's folder, the one with the <code>game</code> folder, replacing files. <b>You do this only once.</b>",
                "Завантажте наші файли й розпакуйте їх у теку гри, ту, де лежить тека <code>game</code>, із заміною файлів. <b>Це робиться лише один раз.</b>"
            )
        } else if v026g {
            lang.tr(
                "Download our files and unpack them over the game's <code>game</code> folder. They add <code>survarium_skhid.exe</code>, <code>play_skhid.bat</code> and, for Linux, <code>play_skhid.sh</code> beside the game's own exe, and replace two of the game's files: <code>bugtrap.dll</code> (the crash reporter) and one certificate. Keep copies of those two if you want the originals back; nothing else changes.",
                "Завантажте наші файли й розпакуйте їх поверх теки <code>game</code> гри. Вони додають <code>survarium_skhid.exe</code>, <code>play_skhid.bat</code> і, для Linux, <code>play_skhid.sh</code> поруч із exe гри та замінюють два файли гри: <code>bugtrap.dll</code> (звіти про збої) і один сертифікат. Збережіть копії цих двох файлів, якщо хочете повернути оригінали; більше нічого не змінюється."
            )
        } else {
            lang.tr("Download the patched game and unpack it.", "Завантажте пропатчену гру та розпакуйте її.")
        },
        lang.tr(
            "Sign in with any name and password: a new name registers itself. Remember the password.",
            "Увійдіть з будь-яким іменем і паролем: нове ім'я реєструється саме. Запам'ятайте пароль."
        ),
        lang.tr("How matches start", "Як починаються матчі"),
    )
}

/// The admin page: the token once, then a tab for each action. The tabs are
/// radio buttons in the form (no script), so the token goes with whichever
/// tab is open, and the page comes back on that tab.
fn admin_form(lang: Lang, view: &AdminView) -> String {
    let code = lang.code();
    let tab = |name: &str, title: String| {
        let open = view.tab == name || (view.tab.is_empty() && name == "money");
        format!(
            "<input type=radio name=tab value={name} id=tab-{name}{}><label for=tab-{name}>{title}</label>",
            if open { " checked" } else { "" }
        )
    };
    let (signups_state, signups_to, signups_button) = if view.registrations_open {
        (
            lang.tr("New accounts are taken.", "Нові акаунти приймаються."),
            "closed",
            lang.tr("Stop new registrations", "Зупинити нові реєстрації"),
        )
    } else {
        (
            lang.tr(
                "New accounts are refused for now.",
                "Нові акаунти зараз не приймаються.",
            ),
            "open",
            lang.tr("Take new registrations", "Приймати нові реєстрації"),
        )
    };
    let maintenance = if view.token.is_empty() {
        String::new()
    } else {
        view.maintenance.clone()
    };
    let bans = if view.token.is_empty() {
        format!(
            "<p><small>{}</small></p>",
            lang.tr(
                "Shown once the token is right.",
                "Показуються після правильного токена."
            )
        )
    } else {
        view.bans.clone()
    };
    let (extra_tabs, extra_panels) = if view.g26 {
        admin_extras_026g(lang, view, &tab)
    } else {
        (String::new(), String::new())
    };
    let password_tab = tab("password", lang.tr("Password", "Пароль"));
    let password_panel = password_reset::panel(lang, view);
    format!(
        "<h1>{}</h1><form method=post action=\"\" autocomplete=off>\
         <input type=hidden name=lang value={code}>\
         <label for=t>{}</label><input id=t name=token type=password required value=\"{}\">\
         <div class=tabs>{}{}{}{}{}{}{extra_tabs}{password_tab}\
         <div class=\"panel p-money\">\
         <label for=u>{}</label><input id=u name=user maxlength=99>\
         <label for=s>{}</label><input id=s name=silver placeholder=+10000>\
         <label for=g>{}</label><input id=g name=gold placeholder=+500>\
         <button>{}</button></div>\
         <div class=\"panel p-ban\">\
         <label for=b>{}</label><input id=b name=ban_user maxlength=99>\
         <label for=h>{}</label><input id=h name=hours inputmode=numeric placeholder=24>\
         <label for=r>{}</label><input id=r name=reason maxlength=200>\
         <label class=check><input type=checkbox name=with_ip checked> {}</label>\
         <button>{}</button></div>\
         <div class=\"panel p-unban\">\
         <label for=n>{}</label><input id=n name=unban_user maxlength=99>\
         <label class=check><input type=checkbox name=unban_ip> {}</label>\
         <button>{}</button></div>\
         <div class=\"panel p-bans\">{bans}<button>{}</button></div>\
         <div class=\"panel p-signups\"><p class=note>{signups_state}</p>\
         <small>{}</small><br>\
         <button name=signups value={signups_to}>{signups_button}</button></div>\
         <div class=\"panel p-maint\">{maintenance}\
         <label for=m>{}</label><input id=m name=minutes inputmode=numeric placeholder=30>\
         <small>{}</small><br>\
         <button>{}</button> <button name=cancel value=1 class=quiet>{}</button></div>\
         {extra_panels}{password_panel}</div></form>",
        lang.tr("Admin", "Адміністратор"),
        lang.tr("Token", "Токен"),
        escape(&view.token),
        tab("money", lang.tr("Money", "Гроші")),
        tab("ban", lang.tr("Ban", "Бан")),
        tab("unban", lang.tr("Unban", "Розбан")),
        tab("bans", lang.tr("Bans", "Бани")),
        tab("maint", lang.tr("Maintenance", "Роботи")),
        tab("signups", lang.tr("Signups", "Реєстрація")),
        lang.tr("User name", "Ім'я користувача"),
        lang.tr("Silver, + or -", "Срібло, + або -"),
        lang.tr("Gold, + or -", "Золото, + або -"),
        lang.tr("Change", "Змінити"),
        lang.tr(
            "User name, or an address or range (1.2.3.4, 1.2.3.0/24)",
            "Ім'я, або адреса чи діапазон (1.2.3.4, 1.2.3.0/24)"
        ),
        lang.tr("Hours (empty: for good)", "Години (порожньо — назавжди)"),
        lang.tr(
            "Reason (only admins see it)",
            "Причина (бачать лише адміни)"
        ),
        lang.tr(
            "Also ban the address it last signed in from",
            "Також заблокувати адресу, з якої він входив востаннє"
        ),
        lang.tr("Ban", "Заблокувати"),
        lang.tr(
            "User name, or an address or range",
            "Ім'я, або адреса чи діапазон"
        ),
        lang.tr(
            "Also lift the ban of its last address",
            "Також розблокувати його останню адресу"
        ),
        lang.tr("Unban", "Розблокувати"),
        lang.tr("Refresh", "Оновити"),
        lang.tr(
            "Existing players keep signing in and playing either way. Registration is open by default; a restart goes back to that (or to SRP_REGISTRATION=closed).",
            "Гравці з акаунтом входять і грають у будь-якому разі. За замовчуванням реєстрація відкрита; перезапуск повертає це (або SRP_REGISTRATION=closed)."
        ),
        lang.tr("Starts in (minutes)", "Почнуться через (хвилин)"),
        lang.tr(
            "Players see the minutes left from 60 on, cannot press Play from 30, and the game closes at 0. Restarting srp cancels it.",
            "Гравці бачать залишок хвилин за годину, не можуть натиснути «Грати» за 30, а гра закривається на 0. Перезапуск srp скасовує."
        ),
        lang.tr("Plan", "Запланувати"),
        lang.tr("Cancel", "Скасувати"),
    )
}

/// The 0.26g admin's own tabs and panels: the account, an item, the text.
fn admin_extras_026g(
    lang: Lang,
    view: &AdminView,
    tab: &dyn Fn(&str, String) -> String,
) -> (String, String) {
    let tabs = format!(
        "{}{}{}",
        tab("account", lang.tr("Account", "Акаунт")),
        tab("item", lang.tr("Item", "Предмет")),
        tab("text", lang.tr("Text", "Текст")),
    );
    let standing = match &view.admin_text {
        Some((first, second)) => format!(
            "<p class=note>{}<br>{}<br>{}</p>",
            lang.tr("Shown now:", "Зараз показано:"),
            escape(first),
            escape(second)
        ),
        None if view.token.is_empty() => String::new(),
        None => format!(
            "<p><small>{}</small></p>",
            lang.tr("No text is shown.", "Текст не показано.")
        ),
    };
    let max = v026g::MAX_TEXT_LEN;
    let panels = format!(
        "<div class=\"panel p-account\">\
         <label for=au>{}</label><input id=au name=acc_user maxlength=99>\
         <label for=ph>{}</label><input id=ph name=premium_hours placeholder=+24>\
         <label for=lv>{}</label><input id=lv name=level inputmode=numeric>\
         <label for=xp>{}</label><input id=xp name=experience placeholder=+500>\
         <label for=fa>{}</label><input id=fa name=faction inputmode=numeric>\
         <label for=rp>{}</label><input id=rp name=reputation placeholder=+100>\
         <small>{}</small><br><button>{}</button></div>\
         <div class=\"panel p-item\">\
         <label for=iu>{}</label><input id=iu name=item_user maxlength=99>\
         <label for=it>{}</label><input id=it name=item maxlength=99>\
         <label for=ic>{}</label><input id=ic name=count inputmode=numeric placeholder=1>\
         <button>{}</button></div>\
         <div class=\"panel p-text\">{standing}\
         <label for=l1>{}</label><input id=l1 name=line1 maxlength={max}>\
         <label for=l2>{}</label><input id=l2 name=line2 maxlength={max}>\
         <small>{}</small><br>\
         <button>{}</button> <button name=clear_text value=1 class=quiet>{}</button></div>",
        lang.tr("User name", "Ім'я користувача"),
        lang.tr("Premium hours, + or -", "Години преміуму, + або -"),
        lang.tr("Set the level (0 to 50)", "Встановити рівень (0–50)"),
        lang.tr("Experience, + or -", "Досвід, + або -"),
        lang.tr("Faction (1 to 4)", "Фракція (1–4)"),
        lang.tr("Reputation with it, + or -", "Репутація в ній, + або -"),
        lang.tr(
            "Empty fields change nothing. An open game is sent the account again.",
            "Порожні поля нічого не змінюють. Відкрита гра отримає акаунт знову."
        ),
        lang.tr("Change", "Змінити"),
        lang.tr("User name", "Ім'я користувача"),
        lang.tr(
            "Item: its id in the 0.26g dictionary, or its exact name",
            "Предмет: id у словнику 0.26g або точна назва"
        ),
        lang.tr(
            "Count (rounds for ammo; up to 8 of anything else)",
            "Кількість (набої для боєприпасів; до 8 для решти)"
        ),
        lang.tr("Give", "Видати"),
        lang.tr("First line", "Перший рядок"),
        lang.tr("Second line", "Другий рядок"),
        lang.tr(
            "Everyone in the lobby sees it, and whoever signs in, until it is cleared or srp restarts.",
            "Його бачать усі в лобі й ті, хто входить, доки його не приберуть або srp не перезапустять."
        ),
        lang.tr("Show", "Показати"),
        lang.tr("Clear", "Прибрати"),
    );
    (tabs, panels)
}

/// `HH:MM` (UTC) of a Unix time.
fn clock_utc(at: u64) -> String {
    let seconds = at % 86_400;
    format!("{:02}:{:02}", seconds / 3600, seconds / 60 % 60)
}

/// What is planned, as a line for the admin and the home page.
fn maintenance_line(lang: Lang, store: &SessionStore) -> String {
    let Some(minutes) = store.minutes_to_maintenance() else {
        return format!(
            "<p><small>{}</small></p>",
            lang.tr(
                "No maintenance is planned.",
                "Технічних робіт не заплановано."
            )
        );
    };
    let at = store.maintenance_at().unwrap_or_default();
    let text = if minutes == 0 {
        lang.tr(
            "Maintenance has started: the game closes for everyone who signs in.",
            "Технічні роботи почалися: гра закривається в усіх, хто входить.",
        )
    } else {
        lang.tr(
            &format!(
                "Maintenance starts in {minutes} min, at {} UTC.",
                clock_utc(at)
            ),
            &format!(
                "Технічні роботи почнуться через {minutes} хв, о {} UTC.",
                clock_utc(at)
            ),
        )
    };
    format!("<p class=note>{}</p>", escape(&text))
}

/// The bans in force, as two tables.
fn bans_table(lang: Lang, store: &SessionStore) -> String {
    let (accounts, nets) = store.bans();
    let until = |ban: &Ban| match ban.until {
        Some(until) => {
            let hours = until.saturating_sub(unix_now()).div_ceil(3600);
            lang.tr(&format!("{hours} h left"), &format!("ще {hours} год"))
        }
        None => lang.tr("for good", "назавжди"),
    };
    let none = format!("<p><small>{}</small></p>", lang.tr("None.", "Немає."));
    let mut html = format!("<h2>{}</h2>", lang.tr("Accounts", "Акаунти"));
    if accounts.is_empty() {
        html += &none;
    } else {
        html += &format!(
            "<table><tr><th>{}</th><th>{}</th><th>{}</th><th>{}</th></tr>",
            lang.tr("Account", "Акаунт"),
            lang.tr("Until", "Доки"),
            lang.tr("Reason", "Причина"),
            lang.tr("Last address", "Остання адреса"),
        );
        for (_, login, name, ban, ip) in &accounts {
            html += &format!(
                "<tr><td>{}<br><small>{}</small></td><td>{}</td><td>{}</td><td>{}</td></tr>",
                escape(name),
                escape(login),
                until(ban),
                escape(&ban.reason),
                ip.map(|ip| ip.to_string()).unwrap_or_default(),
            );
        }
        html += "</table>";
    }
    html += &format!("<h2>{}</h2>", lang.tr("Addresses", "Адреси"));
    if nets.is_empty() {
        html += &none;
    } else {
        html += &format!(
            "<table><tr><th>{}</th><th>{}</th><th>{}</th></tr>",
            lang.tr("Address", "Адреса"),
            lang.tr("Until", "Доки"),
            lang.tr("Reason", "Причина"),
        );
        for ban in &nets {
            html += &format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td></tr>",
                ban.net,
                until(&ban.ban),
                escape(&ban.ban.reason),
            );
        }
        html += "</table>";
    }
    html
}

/// The wall clock, Unix seconds.
fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// `/replays/` against every hostile request of `CONTRACT-R1`, and the
/// other routes against the same list.
#[cfg(test)]
mod replay_tests;

#[cfg(test)]
mod tests {
    use super::*;

    fn web(token: Option<&str>) -> Web {
        Web {
            store: Arc::new(SessionStore::new()),
            token: token.map(str::to_owned),
            admin_path: Some("/hidden".into()),
            admin_ips: vec![
                Net::parse("127.0.0.1").unwrap(),
                Net::parse("10.1.0.0/16").unwrap(),
            ],
            trust_proxy: false,
            token_failures: Mutex::new((0, None)),
            connections: AtomicU32::new(0),
            downloads: AtomicU32::new(0),
            resource_downloads: AtomicU32::new(0),
            replay_downloads: AtomicU32::new(0),
            replay_failures: Mutex::new(HashMap::new()),
            game: None,
        }
    }

    fn post(path: &str, form: &[(&str, &str)]) -> Request {
        Request {
            method: "POST".into(),
            path: path.into(),
            target: path.into(),
            lang: Lang::En,
            forwarded: String::new(),
            range: String::new(),
            client: Some("127.0.0.1".parse().unwrap()),
            form: form
                .iter()
                .map(|(key, value)| (key.to_string(), value.to_string()))
                .collect(),
        }
    }

    fn get(path: &str) -> Request {
        Request {
            method: "GET".into(),
            form: Vec::new(),
            ..post(path, &[])
        }
    }

    fn balances(web: &Web, user: &str) -> (u32, u32) {
        let id = web.store.account_id_by_name(user).unwrap();
        let account = web.store.account(id).unwrap();
        let account = locked(&account);
        (account.game.generic_money, account.game.premium_money)
    }

    #[test]
    fn the_0_21d_page_has_no_exchange() {
        let web = web(None);
        web.store.sign_in("fighter", b"secret").unwrap();
        let before = balances(&web, "fighter");
        assert_eq!(web.route(&get("/exchange"), "t").status, "404 Not Found");
        let sent = web.route(
            &post(
                "/exchange",
                &[("user", "fighter"), ("password", "secret"), ("gold", "1")],
            ),
            "t",
        );
        assert_eq!(sent.status, "404 Not Found");
        assert_eq!(balances(&web, "fighter"), before);
        let home = web.route(&get("/"), "t").body;
        assert!(!home.contains("/exchange"), "{home}");
    }

    #[test]
    fn the_admin_needs_its_token_and_changes_balances() {
        let web = web(Some("tok"));
        web.store.sign_in("tester", b"secret").unwrap();
        let (silver, gold) = balances(&web, "tester");
        let refused = web.admin(
            &post(
                "/hidden",
                &[("token", "bad"), ("user", "tester"), ("gold", "+9")],
            ),
            "t",
        );
        assert!(refused.body.contains("Wrong token"));
        assert_eq!(balances(&web, "tester"), (silver, gold));

        let done = web.admin(
            &post(
                "/hidden",
                &[
                    ("token", "tok"),
                    ("user", "tester"),
                    ("gold", "+500"),
                    ("silver", "-999999999"),
                ],
            ),
            "t",
        );
        assert!(done.body.contains("now 0 silver"), "{}", done.body);
        assert_eq!(balances(&web, "tester"), (0, gold + 500));
    }

    #[test]
    fn the_admin_bans_from_its_tabs_with_the_token_given_once() {
        let web = web(Some("tok"));
        let id = web.store.sign_in("cheater@x.test", b"secret").unwrap();
        web.store.signed_in_from(id, "203.0.113.9".parse().unwrap());
        let banned = web.admin(
            &post(
                "/hidden",
                &[
                    ("token", "tok"),
                    ("tab", "ban"),
                    ("ban_user", "cheater"),
                    ("hours", ""),
                    ("reason", "<aimbot>"),
                    ("with_ip", "on"),
                ],
            ),
            "t",
        );
        assert!(
            banned.body.contains("cheater banned for good"),
            "{}",
            banned.body
        );
        assert!(banned.body.contains("203.0.113.9 too"));
        // The page comes back on its tab, with the token kept.
        assert!(banned.body.contains("value=ban id=tab-ban checked"));
        assert!(banned.body.contains("value=\"tok\""));
        // The list escapes what it shows.
        assert!(banned.body.contains("&lt;aimbot&gt;"));
        assert_eq!(
            web.store.sign_in("cheater@x.test", b"secret"),
            Err(session::SignInError::Banned)
        );
        assert!(web.store.ip_ban("203.0.113.9".parse().unwrap()).is_some());

        let lifted = web.admin(
            &post(
                "/hidden",
                &[
                    ("token", "tok"),
                    ("tab", "unban"),
                    ("unban_user", "cheater@x.test"),
                    ("unban_ip", "on"),
                ],
            ),
            "t",
        );
        assert!(
            lifted.body.contains("cheater@x.test unbanned"),
            "{}",
            lifted.body
        );
        assert_eq!(web.store.sign_in("cheater@x.test", b"secret"), Ok(id));
        assert!(web.store.ip_ban("203.0.113.9".parse().unwrap()).is_none());

        // A wrong token keeps nothing.
        let refused = web.admin(&post("/hidden", &[("token", "bad"), ("tab", "bans")]), "t");
        assert!(!refused.body.contains("value=\"bad\""));
    }

    #[test]
    fn the_admin_plans_maintenance_and_closes_registration() {
        let web = web(Some("tok"));
        let act = |fields: &[(&str, &str)]| {
            let mut all = vec![("token", "tok")];
            all.extend_from_slice(fields);
            web.admin(&post("/hidden", &all), "t")
        };
        let planned = act(&[("tab", "maint"), ("minutes", "45")]);
        assert!(
            planned.body.contains("starts in 45 min"),
            "{}",
            planned.body
        );
        assert_eq!(web.store.minutes_to_maintenance(), Some(45));
        let bad = act(&[("tab", "maint"), ("minutes", "0")]);
        assert!(bad.body.contains("a whole number from 1"));
        assert_eq!(web.store.minutes_to_maintenance(), Some(45));
        // The home page says so.
        let home = web.route(&get("/"), "t");
        assert!(home.body.contains("closes in 4"), "{}", home.body);
        let cancelled = act(&[("tab", "maint"), ("cancel", "1")]);
        assert!(cancelled.body.contains("Maintenance cancelled"));
        assert_eq!(web.store.maintenance_at(), None);

        assert!(web.store.registrations_open());
        let closed = act(&[("tab", "signups"), ("signups", "closed")]);
        assert!(closed.body.contains("refused"), "{}", closed.body);
        assert!(!web.store.registrations_open());
        assert!(
            web.route(&get("/"), "t")
                .body
                .contains("registrations are closed")
        );
        act(&[("tab", "signups"), ("signups", "open")]);
        assert!(web.store.registrations_open());
    }

    #[test]
    fn wrong_tokens_lock_the_admin_for_a_while() {
        let web = web(Some("tok"));
        web.store.sign_in("tester", b"secret").unwrap();
        for _ in 0..MAX_TOKEN_FAILURES {
            web.admin(&post("/hidden", &[("token", "bad")]), "t");
        }
        let locked = web.admin(
            &post(
                "/hidden",
                &[("token", "tok"), ("user", "tester"), ("gold", "1")],
            ),
            "t",
        );
        assert!(locked.body.contains("wait a minute"));
    }

    fn password_hash(web: &Web, id: u32) -> Option<String> {
        web.store
            .account(id)
            .unwrap()
            .locked()
            .password_hash
            .clone()
    }

    #[test]
    fn the_admin_resets_a_password_once_the_account_is_confirmed() {
        let web = web(Some("tok"));
        let id = web.store.sign_in("Forgetful@x.test", b"old").unwrap();
        web.store.signed_in_from(id, "203.0.113.9".parse().unwrap());
        let session = web.store.create_session(id);
        let act = |fields: &[(&str, &str)]| {
            let mut all = vec![("token", "tok"), ("tab", "password")];
            all.extend_from_slice(fields);
            web.admin(&post("/hidden", &all), "t").body
        };

        // A session with no lobby or chat connection open is not "signed in now".
        let idle = act(&[("reset_user", "forgetful@x.test")]);
        assert!(idle.contains("Signed in now</th><td>no</td>"), "{idle}");
        let held = web.store.hold(session).unwrap();

        // The user name in any case: the account to confirm, nothing changed.
        let shown = act(&[("reset_user", "FORGETFUL@x.test")]);
        assert!(shown.contains("Check the account below"), "{shown}");
        assert!(shown.contains("<td>forgetful@x.test</td>"), "{shown}");
        assert!(shown.contains("<td>Forgetful</td>"), "{shown}");
        assert!(shown.contains("203.0.113.9"), "{shown}");
        assert!(shown.contains(" UTC</td>"), "{shown}");
        assert!(shown.contains("yes: its game is signed out"), "{shown}");
        assert!(
            shown.contains(&format!(
                "name=reset_id value={id}>Reset the password of forgetful@x.test"
            )),
            "{shown}"
        );
        assert!(shown.contains("value=password id=tab-password checked"));
        assert!(password_hash(&web, id).is_some());
        assert!(!web.store.has_ended(session));

        // The character name is not a user name.
        let by_name = act(&[("reset_user", "Forgetful")]);
        assert!(by_name.contains("No account signs in with that user name"));
        assert!(!by_name.contains("name=reset_id"));
        // A confirm for another account changes nothing.
        let other = web.store.sign_in("other", b"pw").unwrap();
        let mismatch = act(&[
            ("reset_user", "forgetful@x.test"),
            ("reset_id", &other.to_string()),
        ]);
        assert!(mismatch.contains("now means another account"), "{mismatch}");
        assert!(password_hash(&web, id).is_some());
        assert!(password_hash(&web, other).is_some());

        let done = act(&[
            ("reset_user", "forgetful@x.test"),
            ("reset_id", &id.to_string()),
        ]);
        assert!(
            done.contains("The password of forgetful@x.test is cleared; 1 session(s) ended"),
            "{done}"
        );
        assert!(!done.contains("name=reset_id"));
        assert_eq!(password_hash(&web, id), None);
        assert!(web.store.has_ended(session));
        drop(held);
        // The next sign-in sets the new password; the old one is refused.
        assert_eq!(web.store.sign_in("forgetful@x.test", b"new"), Ok(id));
        assert_eq!(
            web.store.sign_in("forgetful@x.test", b"old"),
            Err(session::SignInError::WrongPassword)
        );
        assert!(password_hash(&web, other).is_some());

        // Without the right token, nothing is looked up or changed.
        let refused = web.admin(
            &post(
                "/hidden",
                &[
                    ("token", "bad"),
                    ("tab", "password"),
                    ("reset_user", "other"),
                    ("reset_id", &other.to_string()),
                ],
            ),
            "t",
        );
        assert!(refused.body.contains("Wrong token"));
        assert!(!refused.body.contains("name=reset_id"));
        assert!(password_hash(&web, other).is_some());
    }

    #[test]
    fn a_reset_the_database_refused_is_an_error_not_a_success() {
        let mut web = web(Some("tok"));
        let repository = session::persist::MemoryRepository::default();
        web.store = Arc::new(
            SessionStore::with_repository(Duration::from_secs(30), Box::new(repository.clone()))
                .unwrap(),
        );
        let id = web.store.sign_in("forgetful", b"old").unwrap();
        web.store.flush();
        // The database refuses every account write from now on.
        repository.fail_account_saves(true);
        let direct = web.admin(
            &post(
                "/hidden",
                &[
                    ("token", "tok"),
                    ("tab", "reset_password"),
                    ("login", "forgetful"),
                ],
            ),
            "t",
        );
        assert_eq!(direct.status, "500 Internal Server Error");
        assert!(
            direct.body.starts_with(&format!(
                "error: forgetful {id} cleared in memory, not written: account {id}: disk full"
            )),
            "{}",
            direct.body
        );
        let page = web.admin(
            &post(
                "/hidden",
                &[
                    ("token", "tok"),
                    ("tab", "password"),
                    ("reset_user", "forgetful"),
                    ("reset_id", &id.to_string()),
                ],
            ),
            "t",
        );
        assert!(
            page.body
                .contains("Error: the password of forgetful is cleared"),
            "{}",
            page.body
        );
        assert!(!page.body.contains("Their next sign-in"));
    }

    #[test]
    fn a_script_resets_a_password_with_one_post() {
        let web = web(Some("tok"));
        let id = web.store.sign_in("Forgetful", b"old").unwrap();
        let session = web.store.create_session(id);
        let send = |client: &str, fields: &[(&str, &str)]| {
            let mut request = post("/hidden", fields);
            request.client = client.parse().ok();
            web.route(&request, "t")
        };
        let reset = |fields: &[(&str, &str)]| send("127.0.0.1", fields);

        // An address not in SRP_ADMIN_IPS: the plain "not found".
        let elsewhere = send(
            "203.0.113.5",
            &[
                ("token", "tok"),
                ("tab", "reset_password"),
                ("login", "forgetful"),
            ],
        );
        assert_eq!(elsewhere.status, "404 Not Found");
        assert!(password_hash(&web, id).is_some());
        // No token, or a wrong one.
        let none = reset(&[("tab", "reset_password"), ("login", "forgetful")]);
        assert_eq!(
            (none.status, none.body.as_str()),
            ("403 Forbidden", "wrong token\n")
        );
        let wrong = reset(&[
            ("token", "bad"),
            ("tab", "reset_password"),
            ("login", "forgetful"),
        ]);
        assert_eq!(wrong.status, "403 Forbidden");
        assert!(password_hash(&web, id).is_some());
        assert!(!web.store.has_ended(session));
        // No login, an unknown one, a character name.
        let empty = reset(&[("token", "tok"), ("tab", "reset_password"), ("login", " ")]);
        assert_eq!(
            (empty.status, empty.body.as_str()),
            ("400 Bad Request", "no login\n")
        );
        let unknown = reset(&[
            ("token", "tok"),
            ("tab", "reset_password"),
            ("login", "nobody"),
        ]);
        assert_eq!(
            (unknown.status, unknown.body.as_str()),
            ("404 Not Found", "no such login\n")
        );
        assert!(matches!(unknown.content_type, ContentType::Text));

        // At once, no confirm step; in any case.
        let done = reset(&[
            ("token", "tok"),
            ("tab", "reset_password"),
            ("login", " FORGETFUL "),
        ]);
        assert_eq!(
            (done.status, done.body.clone()),
            ("200 OK", format!("ok forgetful {id}\n"))
        );
        assert!(matches!(done.content_type, ContentType::Text));
        assert_eq!(password_hash(&web, id), None);
        assert!(web.store.has_ended(session));
        assert_eq!(web.store.sign_in("forgetful", b"new"), Ok(id));
        assert_eq!(
            web.store.sign_in("forgetful", b"old"),
            Err(session::SignInError::WrongPassword)
        );

        // Wrong tokens lock it as they lock the page.
        for _ in 0..MAX_TOKEN_FAILURES {
            reset(&[
                ("token", "bad"),
                ("tab", "reset_password"),
                ("login", "forgetful"),
            ]);
        }
        let locked = reset(&[
            ("token", "tok"),
            ("tab", "reset_password"),
            ("login", "forgetful"),
        ]);
        assert_eq!(
            (locked.status, locked.body.as_str()),
            ("429 Too Many Requests", "locked: too many wrong tokens\n")
        );
        assert!(password_hash(&web, id).is_some());
    }

    #[test]
    fn the_admin_page_is_for_its_addresses_and_its_path_only() {
        let web = web(Some("tok"));
        let get = |path: &str, client: &str| {
            web.route(
                &Request {
                    method: "GET".into(),
                    path: path.into(),
                    target: path.into(),
                    lang: Lang::En,
                    forwarded: String::new(),
                    range: String::new(),
                    client: client.parse().ok(),
                    form: Vec::new(),
                },
                "t",
            )
        };
        assert_eq!(get("/hidden", "127.0.0.1").status, "200 OK");
        assert_eq!(get("/hidden", "10.1.200.3").status, "200 OK");
        assert_eq!(get("/hidden", "203.0.113.5").status, "404 Not Found");
        assert_eq!(get("/hidden", "10.2.0.1").status, "404 Not Found");
        assert_eq!(get("/admin", "127.0.0.1").status, "404 Not Found");
        assert_eq!(get("/hidden", "::1").status, "404 Not Found");
    }

    #[test]
    fn networks_parse_and_match() {
        let net = Net::parse("192.168.0.0/16").unwrap();
        assert!(net.contains("192.168.9.9".parse().unwrap()));
        assert!(!net.contains("192.169.0.1".parse().unwrap()));
        assert!(
            Net::parse("0.0.0.0/0")
                .unwrap()
                .contains("8.8.8.8".parse().unwrap())
        );
        assert!(Net::parse("::1").unwrap().contains("::1".parse().unwrap()));
        assert!(Net::parse("1.2.3.4/33").is_none() && Net::parse("nope").is_none());
    }

    #[test]
    fn the_admin_page_does_not_exist_without_a_token() {
        let web = web(None);
        let request = Request {
            method: "GET".into(),
            path: "/hidden".into(),
            target: "/hidden".into(),
            lang: Lang::En,
            forwarded: String::new(),
            range: String::new(),
            client: Some("127.0.0.1".parse().unwrap()),
            form: Vec::new(),
        };
        assert_eq!(web.route(&request, "t").status, "404 Not Found");
    }

    #[test]
    fn the_language_is_ukrainian_or_english_only() {
        assert_eq!(Lang::pick("uk", "en"), Lang::Uk);
        assert_eq!(Lang::pick("en", "uk"), Lang::En);
        assert_eq!(Lang::pick("de", "uk-UA,uk;q=0.9"), Lang::Uk);
        assert_eq!(Lang::pick("", "ru-RU,ru"), Lang::En);
        assert_eq!(Lang::pick("", ""), Lang::En);
    }

    #[test]
    fn the_rooms_are_told_as_they_are() {
        let en = room_rules(Lang::En, false);
        assert!(en.contains("vote for 15 s between 3 maps"));
        assert!(en.contains("until its last 30 s"));
        assert!(en.contains("Squads are off"));
        // Nothing of the queue's rules is left in it.
        assert!(!en.contains("countdown"));
        assert!(!en.contains("JOIN"));
        assert!(!en.contains("minutes are left"));

        let squads = room_rules(Lang::En, true);
        assert!(!squads.contains("Squads are off"));

        let uk = room_rules(Lang::Uk, false);
        assert!(uk.contains("15 с голосують за одну з 3 мап"));
        assert!(uk.contains("Загони вимкнено"));
        assert_eq!(en.matches("<li>").count(), uk.matches("<li>").count());
    }

    #[test]
    fn a_test_server_says_so_on_every_page() {
        let plain = page_as(Lang::En, Form::None, "", false, None).body;
        assert!(plain.contains("<title>Skhid · Survarium 0.21d community server</title>"));
        assert!(!plain.contains("class=badge>"));
        assert!(!plain.contains("Test server."));

        let url = Some("https://skhid.link");
        for form in [Form::None, Form::InTheGame26("en"), Form::None26] {
            let test = page_as(Lang::En, form, "", true, url).body;
            assert!(test.contains(" TEST server</title>"), "{test}");
            assert!(test.contains("<span class=badge>TEST</span>"));
            assert!(test.contains("TEST SERVER FOR SURVARIUM 0."));
            assert!(test.contains("<b>Test server.</b>"));
            assert!(test.contains("accounts and progress here can be reset"));
            // The other server is the players', named in the notice and
            // not as the other game version in the header.
            assert!(test.contains(
                "The players' server: <a href=\"https://skhid.link/?lang=en\">skhid.link</a>."
            ));
            assert!(!test.contains(">Survarium 0.21d</a>"));
            assert!(!test.contains(">Survarium 0.26g</a>"));
        }
        let uk = page_as(Lang::Uk, Form::None26, "", true, None).body;
        assert!(uk.contains("<span class=badge>ТЕСТ</span>"));
        assert!(uk.contains("<b>Тестовий сервер.</b>"));
        assert!(!uk.contains("Сервер для гравців:"));
        // A server that is not one links the other version as before.
        let live = page_as(Lang::En, Form::None26, "", false, url).body;
        assert!(live.contains(" <a href=\"https://skhid.link/?lang=en\">Survarium 0.21d</a>"));
    }

    #[test]
    fn the_damage_calculator_is_the_026g_sites_only() {
        let web = web(None);
        assert_eq!(web.route(&get("/damage"), "t").status, "404 Not Found");
        let home = web.route(&get("/"), "t").body;
        assert!(!home.contains("href=\"/damage"));
        // The site's fonts are the calculator's, served on both servers.
        assert!(home.contains("src: url(\"/damage/fonts/pt-sans-regular.woff2?v="));
        assert!(crate::damage::file("/damage/fonts/pt-sans-regular.woff2").is_some());
    }

    #[test]
    fn the_maps_are_the_026g_sites_only() {
        let web = web(None);
        assert_eq!(web.route(&get("/maps"), "t").status, "404 Not Found");
        assert!(!web.route(&get("/"), "t").body.contains("href=\"/maps"));
    }

    #[test]
    fn the_other_server_is_linked_only_by_a_plain_https_address() {
        assert_eq!(
            other_url(" https://026g.skhid.link/ ").as_deref(),
            Some("https://026g.skhid.link")
        );
        assert_eq!(other_url("http://skhid.link"), None);
        assert_eq!(other_url("https://"), None);
        assert_eq!(other_url("https://a.b/\"><script>"), None);
        assert_eq!(other_url("https://a.b/?x=1"), None);
        assert_eq!(other_server_link(Lang::Uk, "0.26g", None), "");
        assert_eq!(
            other_server_link(Lang::Uk, "0.26g", Some("https://026g.skhid.link")),
            " <a href=\"https://026g.skhid.link/?lang=uk\">Survarium 0.26g</a>"
        );
        // Without the setting the pages are as they were.
        assert!(
            !page(Lang::En, Form::None, "")
                .body
                .contains("Survarium 0.26g</a>")
        );
    }

    #[test]
    fn pages_are_branded_unofficial_and_localized() {
        let en = page(Lang::En, Form::None, "").body;
        let uk = page(Lang::Uk, Form::None, "").body;
        assert!(en.contains("СХІД") && en.contains("SKHID") && en.contains("unofficial"));
        assert!(uk.contains("СХІД") && uk.contains("Неофіційний ком'юніті"));
        assert!(uk.contains("Головна") && en.contains(">Home</a>"));
    }

    #[test]
    fn a_range_is_read_against_the_file_size() {
        assert_eq!(parse_range("", 100), Some(None));
        assert_eq!(parse_range("bytes=0-", 100), Some(Some((0, 99))));
        assert_eq!(parse_range("bytes=10-19", 100), Some(Some((10, 19))));
        assert_eq!(parse_range("bytes=90-500", 100), Some(Some((90, 99))));
        assert_eq!(parse_range("bytes=-10", 100), Some(Some((90, 99))));
        assert_eq!(parse_range("bytes=100-", 100), None);
        assert_eq!(parse_range("bytes=50-10", 100), None);
        assert_eq!(parse_range("bytes=0-1,5-6", 100), Some(None));
        assert_eq!(parse_range("items=1-2", 100), Some(None));
        assert_eq!(parse_range("bytes=x-y", 100), None);
    }

    #[test]
    fn at_most_five_downloads_run_at_once_and_the_slot_comes_back() {
        let counter = AtomicU32::new(0);
        let mut slots = Vec::new();
        for _ in 0..MAX_DOWNLOADS {
            assert!(counter.fetch_add(1, Ordering::AcqRel) < MAX_DOWNLOADS);
            slots.push(Slot(&counter));
        }
        assert!(counter.fetch_add(1, Ordering::AcqRel) >= MAX_DOWNLOADS);
        counter.fetch_sub(1, Ordering::AcqRel);
        slots.pop();
        assert_eq!(counter.load(Ordering::Acquire), MAX_DOWNLOADS - 1);
        drop(slots);
        assert_eq!(counter.load(Ordering::Acquire), 0);
    }

    const BLOB: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

    #[test]
    fn a_blob_is_named_by_its_checksum_and_nothing_else() {
        let download = Path::new("/srv/download/files.zip");
        let blob = format!("/resources/{BLOB}");
        assert_eq!(
            resource_file(&blob, download),
            Some(PathBuf::from("/srv/download/resources").join(BLOB))
        );
        for path in [
            "/resources/".to_owned(),
            "/resources".to_owned(),
            format!("/resources/{}", &BLOB[1..]),
            format!("/resources/{BLOB}0"),
            format!("/resources/{BLOB}/"),
            format!("/resources/{}", BLOB.to_ascii_uppercase()),
            format!("/resources/{}g", &BLOB[1..]),
            format!("/resources/../{}", &BLOB[3..]),
            format!("/resources/%2e%2e%2f{}", &BLOB[9..]),
            format!("/resources//{}", &BLOB[1..]),
            format!("/Resources/{BLOB}"),
            format!("/download/{BLOB}"),
            format!("{blob}.zip"),
            "/resources/..".to_owned(),
            // The replays' hostile list (`replay_tests`), here too.
            format!("/resources/{}\\..", &BLOB[3..]),
            format!("/resources/{}%5c", &BLOB[3..]),
            format!("/resources/{}%00", &BLOB[3..]),
            format!("/resources/{}\0", &BLOB[1..]),
            format!("/resources/{BLOB}.json"),
            format!("/resources/{BLOB}#x"),
            format!("/resources/./{BLOB}"),
            format!("/resources/{}", "a".repeat(2000)),
        ] {
            assert_eq!(resource_file(&path, download), None, "{path}");
        }
        // Only a GET or a HEAD of the 0.26g page, with a download file.
        let get_blob = get(&blob);
        assert!(resource_request(true, &get_blob, Some(download)).is_some());
        let head = Request {
            method: "HEAD".into(),
            ..get(&blob)
        };
        assert!(resource_request(true, &head, Some(download)).is_some());
        assert_eq!(resource_request(false, &get_blob, Some(download)), None);
        assert_eq!(resource_request(true, &get_blob, None), None);
        assert_eq!(
            resource_request(true, &post(&blob, &[]), Some(download)),
            None
        );
        assert_eq!(
            resource_request(true, &get("/download"), Some(download)),
            None
        );
    }

    /// What `answer` writes to a connection, as a client reads it.
    fn answered(answer: impl FnOnce(&mut TcpStream)) -> Vec<u8> {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let client = thread::spawn(move || {
            let mut stream = TcpStream::connect(address).unwrap();
            let mut got = Vec::new();
            stream.read_to_end(&mut got).unwrap();
            got
        });
        let (mut stream, _) = listener.accept().unwrap();
        answer(&mut stream);
        drop(stream);
        client.join().unwrap()
    }

    #[test]
    fn a_blob_is_handed_out_as_the_download_is_in_slots_of_its_own() {
        let dir = std::env::temp_dir().join(format!("srp-web-resources-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("resources")).unwrap();
        let download = dir.join("files.zip");
        std::fs::write(&download, b"the archive").unwrap();
        let file = resource_file(&format!("/resources/{BLOB}"), &download).unwrap();
        std::fs::write(&file, b"abc").unwrap();
        let web = web(None);
        let request = get(&format!("/resources/{BLOB}"));
        let whole = answered(|stream| web.resource(stream, &request, &file));
        assert_eq!(
            String::from_utf8(whole).unwrap(),
            format!(
                "HTTP/1.0 200 OK\r\nContent-Type: application/octet-stream\r\n\
                 Content-Disposition: attachment; filename=\"{BLOB}\"\r\n\
                 Accept-Ranges: bytes\r\nContent-Length: 3\r\n\
                 X-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\nabc"
            )
        );
        // HEAD: the head alone. A range: that part.
        let head = Request {
            method: "HEAD".into(),
            ..get(&format!("/resources/{BLOB}"))
        };
        let answer = answered(|stream| web.resource(stream, &head, &file));
        assert!(answer.ends_with(
            b"Content-Length: 3\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n"
        ));
        let part = Request {
            range: "bytes=1-".into(),
            ..get(&format!("/resources/{BLOB}"))
        };
        let answer =
            String::from_utf8(answered(|stream| web.resource(stream, &part, &file))).unwrap();
        assert!(
            answer.starts_with("HTTP/1.0 206 Partial Content\r\n"),
            "{answer}"
        );
        assert!(
            answer.contains("Content-Range: bytes 1-2/3\r\n") && answer.ends_with("\r\n\r\nbc")
        );
        // A name of the pattern with no file, or a folder: not found.
        let missing = dir.join("resources").join(BLOB.replace('a', "0"));
        for path in [&missing, &dir.join("resources")] {
            let answer = answered(|stream| web.resource(stream, &request, path));
            assert_eq!(
                answer,
                b"HTTP/1.0 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            );
        }
        // Its slots are its own: full, it is busy, and the archive is not.
        web.resource_downloads
            .store(MAX_DOWNLOADS, Ordering::Release);
        let answer = answered(|stream| web.resource(stream, &request, &file));
        assert!(answer.starts_with(b"HTTP/1.0 503 Service Unavailable\r\nRetry-After: 60\r\n"));
        let archive = answered(|stream| web.download(stream, &get("/download"), &download));
        assert_eq!(
            String::from_utf8(archive).unwrap(),
            "HTTP/1.0 200 OK\r\nContent-Type: application/zip\r\n\
             Content-Disposition: attachment; filename=\"files.zip\"\r\n\
             Accept-Ranges: bytes\r\nContent-Length: 11\r\n\
             X-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\nthe archive"
        );
        assert_eq!(web.downloads.load(Ordering::Acquire), 0);
        assert_eq!(
            web.resource_downloads.load(Ordering::Acquire),
            MAX_DOWNLOADS
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_home_page_counts_registered_and_online_players() {
        let web = web(None);
        let count = |web: &Web| {
            let body = web
                .route(
                    &Request {
                        method: "GET".into(),
                        path: "/".into(),
                        target: "/".into(),
                        lang: Lang::En,
                        forwarded: String::new(),
                        range: String::new(),
                        client: None,
                        form: Vec::new(),
                    },
                    "t",
                )
                .body;
            body.split("<div class=stats>")
                .nth(1)
                .unwrap()
                .split("</div></div>")
                .next()
                .unwrap()
                .replace("</b><span>", " ")
                .replace("</span></div><div><b>", " | ")
                .replace("<div><b>", "")
        };
        assert!(count(&web).starts_with("0 players online | 0 registered"));
        let first = web.store.sign_in("a", b"secret").unwrap();
        web.store.sign_in("b", b"secret").unwrap();
        let session = web.store.create_session(first);
        assert!(web.store.hold(session).is_some());
        let shown = count(&web);
        assert!(shown.contains("| 2 registered"), "{shown}");
    }

    #[test]
    fn the_home_page_says_how_to_start_what_the_download_holds() {
        let stats = |launcher| Stats {
            registered: 0,
            online: 0,
            maintenance: None,
            registrations_open: true,
            v026g: true,
            launcher,
            installs_game: false,
        };
        // With the launcher: one file to start, nothing to type, and the
        // download is needed once. The script and the command line are
        // folded away below.
        for (lang, press, once, folded) in [
            (
                Lang::En,
                "press <b>PLAY</b>",
                "You do this only once",
                "Without the launcher",
            ),
            (
                Lang::Uk,
                "натисніть <b>PLAY</b>",
                "лише один раз",
                "Без лаунчера",
            ),
        ] {
            let page = home(lang, stats(true));
            let steps = page.split("<ol>").nth(1).unwrap();
            let (shown, hidden) = steps.split_once("<details>").unwrap();
            assert!(shown.contains("<code>Skhid.bat</code>"), "{shown}");
            assert!(shown.contains(press), "{shown}");
            assert!(shown.contains(once), "{shown}");
            assert!(!shown.contains("binaries"), "{shown}");
            // Where to unpack is said by what the folder holds, not by a
            // name a player may read as "inside game".
            assert!(!shown.contains("over the game's"), "{shown}");
            assert!(!shown.contains("поверх теки"), "{shown}");
            assert!(!shown.contains("<pre>"), "{shown}");
            assert!(!shown.contains("play_skhid.bat"), "{shown}");
            assert!(hidden.contains(&format!("<summary>{folded}</summary>")));
            assert!(hidden.contains("play_skhid.bat"));
            assert!(hidden.contains("<pre>survarium_skhid.exe -localization="));
            assert_eq!(steps.matches("<details>").count(), 1);
            assert_eq!(steps.matches("</details>").count(), 1);
        }
        // A download whose launcher brings the game: no step that sends a
        // player for the game first; an empty folder, the room it needs, and
        // that the first start downloads. The game's own archive is named
        // only as the way out, and the folder of a game a player has already.
        for (lang, empty, first, have, way_out) in [
            (
                Lang::En,
                "into an empty folder with about 10 GB free",
                "The first time, the launcher downloads the game",
                "Already have Survarium 0.26g",
                "Launcher can't get the game?",
            ),
            (
                Lang::Uk,
                "у порожню теку, де вільно близько 10 ГБ",
                "Першого разу лаунчер завантажує гру",
                "Уже маєте Survarium 0.26g",
                "Лаунчер не може отримати гру?",
            ),
        ] {
            let page = home(
                lang,
                Stats {
                    installs_game: true,
                    ..stats(true)
                },
            );
            let steps = page.split("<ol>").nth(1).unwrap();
            let steps = steps.split("</ol>").next().unwrap();
            // The first step is the download, not the game.
            let first_step = steps.split("</li>").next().unwrap();
            assert!(first_step.contains(empty), "{first_step}");
            assert!(first_step.contains("class=dl"), "{first_step}");
            assert!(!steps.contains("Get the original Survarium"), "{steps}");
            assert!(!steps.contains("Отримайте оригінальну"), "{steps}");
            for said in [empty, first, have, way_out, "2.9", "Google Drive"] {
                let said = if lang == Lang::Uk && said == "2.9" {
                    "2,9"
                } else {
                    said
                };
                assert!(steps.contains(said), "{said}: {steps}");
            }
            assert!(steps.contains("<code>Skhid.bat</code>"), "{steps}");
            assert!(
                steps.contains("<code>bash Skhid.sh launcher</code>"),
                "{steps}"
            );
            assert_eq!(steps.matches("<details>").count(), 1);
        }
        // An archive with the extractor and no launcher is not such a one.
        let page = home(
            Lang::En,
            Stats {
                installs_game: true,
                ..stats(false)
            },
        );
        assert!(page.contains("Get the original Survarium"));
        assert!(!page.contains("empty folder"));
        // Without it: the page as it was.
        let page = home(Lang::En, stats(false));
        assert!(!page.contains("launcher_skhid.exe"));
        assert!(!page.contains("<details>"));
        assert!(page.contains("Start it with this command line:<pre>survarium_skhid.exe"));
        assert!(page.contains("<code>play_skhid.bat</code>"));
    }

    #[test]
    fn forms_decode_and_text_is_escaped() {
        assert_eq!(decode("a+b%20c%3D%26%zz%4"), "a b c=&%zz%4");
        assert_eq!(escape("<b>\"&"), "&lt;b&gt;&quot;&amp;");
        assert_eq!(apply(5, -10), 0);
        assert_eq!(delta("+12"), Some(12));
        assert_eq!(delta("x"), None);
    }
}
