//! A small website for the people running srp.
//!
//! - `/` is a home page: what the project is and where it came from.
//! - `/exchange` lets a player exchange silver for gold: user name and password,
//!   then how much gold ([`SILVER_PER_GOLD`] silver each).
//! - The admin page changes the silver and gold of any account, for testing.
//!   It exists only when `SRP_ADMIN_TOKEN` and `SRP_ADMIN_PATH` (a hard to
//!   guess name, served at `/<name>`) are set, and only for the addresses in
//!   `SRP_ADMIN_IPS` (default: this machine). Anyone else gets the same
//!   "not found" as for a page that does not exist.
//!
//! It listens on `SRP_WEB_BIND:SRP_WEB_PORT`, by default 127.0.0.1:8088: the
//! page is plain HTTP, so a public host puts HTTPS in front of it (the
//! compose file's Caddy) instead of opening the port. Every request carries
//! its own credentials, so there are no cookies and nothing to forge across
//! sites.

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::net::{IpAddr, Shutdown, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use session::{Ban, LockExt, Net, SessionStore, SignInError};

/// 10 000 silver for 100 gold.
pub const SILVER_PER_GOLD: u32 = 100;
/// The most gold one exchange buys.
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
    /// `SRP_GAME_URL`: where the original, unchanged game can be had;
    /// `https://` only. The page's own download is only the few files that
    /// go over it.
    game_url: Option<String>,
}

/// Downloads of that file at one time; more get "busy, try again".
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

const MAX_EXCHANGE_GOLD: u32 = 10_000;
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

/// The page's languages: Ukrainian and English only.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Lang {
    En,
    Uk,
}

impl Lang {
    /// `?lang=` or the form's `lang`, else the browser's `Accept-Language`
    /// (Ukrainian when it comes first), else English.
    fn pick(chosen: &str, accept: &str) -> Self {
        match chosen {
            "uk" => return Self::Uk,
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
}

enum Form {
    None,
    Home(Stats),
    Exchange,
    Admin(AdminView),
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
}

/// Frees a download slot when the transfer ends, however it ends.
struct Slot<'a>(&'a AtomicU32);

impl Drop for Slot<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
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
}

pub fn run(store: Arc<SessionStore>) -> io::Result<()> {
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
    let admin_state = match (&token, &admin_path) {
        (Some(_), Some(_)) => format!("admin on for {} address(es)", admin_ips.len()),
        _ => "admin off: SRP_ADMIN_TOKEN and SRP_ADMIN_PATH are needed".to_owned(),
    };
    log::info!(
        "Web server on {} (exchange, {admin_state})",
        listener.local_addr()?
    );
    let web = Arc::new(Web {
        store,
        token,
        admin_path,
        admin_ips,
        trust_proxy,
        token_failures: Mutex::new((0, None)),
        connections: AtomicU32::new(0),
        downloads: AtomicU32::new(0),
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
    path: String,
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

struct Response {
    status: &'static str,
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
        if request.path == "/download" && matches!(request.method.as_str(), "GET" | "HEAD") {
            if let Some(file) = available_download() {
                self.download(&mut stream, &request, file);
                return;
            }
        }
        let response = self.route(&request, &peer);
        let head = format!(
            "HTTP/1.0 {}\r\nContent-Type: text/html; charset=utf-8\r\n\
             Content-Length: {}\r\nCache-Control: no-store\r\n\
             X-Frame-Options: DENY\r\nX-Content-Type-Options: nosniff\r\n\
             Referrer-Policy: no-referrer\r\n\
             Content-Security-Policy: default-src 'none'; style-src 'unsafe-inline'; \
             form-action 'self'\r\nConnection: close\r\n\r\n",
            response.status,
            response.body.len()
        );
        let _ = stream.write_all(head.as_bytes());
        let _ = stream.write_all(response.body.as_bytes());
        let _ = stream.flush();
        let _ = stream.shutdown(Shutdown::Write);
    }

    /// Hands out the download file: whole, or from a `Range` (so a broken
    /// download resumes). At most [`MAX_DOWNLOADS`] at once.
    fn download(&self, stream: &mut TcpStream, request: &Request, file: &PathBuf) {
        let reply = |stream: &mut TcpStream, status: &str, extra: &str| {
            let _ = stream.write_all(
                format!(
                    "HTTP/1.0 {status}\r\n{extra}Content-Length: 0\r\nConnection: close\r\n\r\n"
                )
                .as_bytes(),
            );
        };
        if self.downloads.fetch_add(1, Ordering::AcqRel) >= MAX_DOWNLOADS {
            self.downloads.fetch_sub(1, Ordering::AcqRel);
            reply(stream, "503 Service Unavailable", "Retry-After: 60\r\n");
            return;
        }
        let _slot = Slot(&self.downloads);
        let Ok(mut open) = File::open(file) else {
            reply(stream, "404 Not Found", "");
            return;
        };
        let Ok(size) = open.metadata().map(|meta| meta.len()) else {
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
            "HTTP/1.0 {}\r\nContent-Type: application/zip\r\n\
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
        match (request.method.as_str(), request.path.as_str()) {
            ("GET", "/") => page(
                request.lang,
                Form::Home(Stats {
                    registered: self.store.registered_accounts(),
                    online: self.store.players_present(),
                    maintenance: self.store.minutes_to_maintenance(),
                    registrations_open: self.store.registrations_open(),
                }),
                "",
            ),
            ("GET", "/exchange") => page(request.lang, Form::Exchange, ""),
            ("POST", "/exchange") => self.exchange(request, peer),
            ("GET", path) if self.is_admin_page(path, request) => {
                page(request.lang, Form::Admin(AdminView::default()), "")
            }
            ("POST", path) if self.is_admin_page(path, request) => self.admin(request, peer),
            _ => Response {
                status: "404 Not Found",
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

    fn exchange(&self, request: &Request, peer: &str) -> Response {
        let lang = request.lang;
        let user = request.field("user");
        let wrong_login = lang.tr(
            "Wrong user name or password.",
            "Неправильне ім'я користувача або пароль.",
        );
        let gold: u32 = match request.field("gold").parse() {
            Ok(gold) if (1..=MAX_EXCHANGE_GOLD).contains(&gold) => gold,
            _ => {
                return page(
                    lang,
                    Form::Exchange,
                    &lang.tr(
                        &format!("Gold must be 1 to {MAX_EXCHANGE_GOLD}."),
                        &format!("Золота має бути від 1 до {MAX_EXCHANGE_GOLD}."),
                    ),
                );
            }
        };
        let password = request.field("password").as_bytes();
        let checked = match request.client {
            Some(ip) => self.store.check_password_from(user, password, ip),
            None => self.store.check_password(user, password),
        };
        let account_id = match checked {
            Ok(id) => id,
            Err(SignInError::Banned) => {
                log::warn!("Exchange for banned account {user:?} from {peer} refused");
                return page(
                    lang,
                    Form::Exchange,
                    &lang.tr("This account is banned.", "Цей акаунт заблоковано."),
                );
            }
            Err(SignInError::TooManyAttempts | SignInError::TooManyFromAddress) => {
                return page(
                    lang,
                    Form::Exchange,
                    &lang.tr(
                        "Too many wrong passwords; wait a minute.",
                        "Забагато хибних паролів; зачекайте хвилину.",
                    ),
                );
            }
            Err(_) => {
                log::warn!("Exchange for {user:?} from {peer}: wrong user name or password");
                return page(lang, Form::Exchange, &wrong_login);
            }
        };
        let Some(account) = self.store.account(account_id) else {
            return page(lang, Form::Exchange, &wrong_login);
        };
        let cost = gold * SILVER_PER_GOLD;
        let result = {
            let mut account = locked(&account);
            let game = &mut account.game;
            match (
                game.generic_money.checked_sub(cost),
                game.premium_money.checked_add(gold),
            ) {
                (Some(silver), Some(total)) => {
                    game.generic_money = silver;
                    game.premium_money = total;
                    Ok((silver, total))
                }
                (None, _) => Err(lang.tr(
                    &format!(
                        "Not enough silver: {gold} gold costs {cost}, you have {}.",
                        game.generic_money
                    ),
                    &format!(
                        "Недостатньо срібла: {gold} золота коштує {cost}, у вас {}.",
                        game.generic_money
                    ),
                )),
                _ => Err(lang.tr(
                    "That would overflow the gold balance.",
                    "Баланс золота переповниться.",
                )),
            }
        };
        match result {
            Ok((silver, total)) => {
                self.store.mark_dirty(account_id);
                self.store.save_soon();
                log::info!(
                    "Exchange for {user:?} from {peer}: {cost} silver for {gold} gold \
                     (now {silver} silver, {total} gold)"
                );
                page(
                    lang,
                    Form::Exchange,
                    &lang.tr(
                        &format!(
                            "Done: {cost} silver for {gold} gold. Now {silver} silver and \
                             {total} gold. An open game shows it within a second."
                        ),
                        &format!(
                            "Готово: {cost} срібла за {gold} золота. Тепер {silver} срібла і \
                             {total} золота. Відкрита гра покаже це за секунду."
                        ),
                    ),
                )
            }
            Err(error) => page(lang, Form::Exchange, &error),
        }
    }

    fn admin(&self, request: &Request, peer: &str) -> Response {
        let lang = request.lang;
        // The tab the form was on says what to do: Enter in any field
        // submits the form, whichever button that presses.
        let tab = match request.field("tab") {
            tab @ ("money" | "ban" | "unban" | "bans" | "maint" | "signups") => tab,
            _ => "money",
        };
        let refused = |message: String| {
            let view = AdminView {
                tab: tab.to_owned(),
                ..AdminView::default()
            };
            page(lang, Form::Admin(view), &message)
        };
        if self.token_locked() {
            return refused(lang.tr(
                "Too many wrong tokens; wait a minute.",
                "Забагато хибних токенів; зачекайте хвилину.",
            ));
        }
        let given = request.field("token");
        let right = self.token.as_deref().unwrap_or_default();
        if !same(given.as_bytes(), right.as_bytes()) {
            self.token_failed();
            log::warn!("Admin with a wrong token from {peer}");
            return refused(lang.tr("Wrong token.", "Неправильний токен."));
        }
        *locked(&self.token_failures) = (0, None);
        let message = match tab {
            "ban" => self.admin_ban(request, peer),
            "unban" => self.admin_unban(request, peer),
            "bans" => String::new(),
            "maint" => self.admin_maintenance(request, peer),
            "signups" => self.admin_signups(request, peer),
            _ => self.admin_money(request, peer),
        };
        let view = AdminView {
            token: given.to_owned(),
            tab: tab.to_owned(),
            bans: bans_table(lang, &self.store),
            maintenance: maintenance_line(lang, &self.store),
            registrations_open: self.store.registrations_open(),
        };
        page(lang, Form::Admin(view), &message)
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

fn page(lang: Lang, form: Form, message: &str) -> Response {
    let notice = if message.is_empty() {
        String::new()
    } else {
        format!("<p class=note>{}</p>", escape(message))
    };
    let code = lang.code();
    let (other, other_name) = match lang {
        Lang::En => ("uk", "Українська"),
        Lang::Uk => ("en", "English"),
    };
    let other_link = other_server_link(
        lang,
        "0.26g",
        CONNECT.get().and_then(|connect| connect.other_url.as_deref()),
    );
    let body = match form {
        Form::Exchange => exchange_form(lang),
        Form::Admin(view) => admin_form(lang, &view),
        Form::Home(stats) => home(lang, stats),
        Form::None => String::new(),
    };
    let disclaimer = lang.tr(
        "Community server, unofficial. Not affiliated with or endorsed by Vostok Games. \
         Survarium is a trademark of its owners.",
        "Неофіційний ком'юніті сервер. Не пов'язаний із Vostok Games і не схвалений ними. \
         Survarium є торговельною маркою її власників.",
    );
    Response {
        status: "200 OK",
        body: format!(
            "<!doctype html><html lang={code}><meta charset=utf-8>\
             <meta name=viewport content=\"width=device-width,initial-scale=1\">\
             <title>{}</title><style>{STYLE}</style>\
             <header><span class=brand>СХІД</span>\
             <span class=tag>{}</span>\
             <nav><a href=\"/?lang={code}\">{}</a> <a href=\"/exchange?lang={code}\">{}</a>{other_link}</nav> <a class=lang href=\"?lang={other}\">{other_name}</a></header>\
             <main>{notice}{body}</main>\
             <footer>{}</footer></html>",
            lang.tr(
                "Skhid · Survarium 0.21d community server",
                "Схід · ком'юніті сервер Survarium 0.21d"
            ),
            lang.tr(
                "SKHID · COMMUNITY SERVER FOR SURVARIUM 0.21D · UNOFFICIAL",
                "КОМ'ЮНІТІ СЕРВЕР SURVARIUM 0.21D · НЕОФІЦІЙНИЙ"
            ),
            lang.tr("Home", "Головна"),
            lang.tr("Exchange", "Обмін"),
            escape(&disclaimer),
        ),
    }
}

const STYLE: &str = ":root{color-scheme:dark}*{box-sizing:border-box}body{font:16px/1.55 system-ui,'Segoe UI',sans-serif;background:radial-gradient(circle at 50% -10%,#232a2e,#101214 60%) fixed;color:#e3e3e0;margin:0;min-height:100vh}header{display:flex;flex-wrap:wrap;align-items:center;gap:.4rem 1.2rem;padding:.9rem 1.4rem;background:#0b0c0d;border-bottom:2px solid #d4a017;box-shadow:0 2px 18px #0008}.brand{font:700 1.6rem Impact,'Arial Narrow',sans-serif;letter-spacing:.28em;color:#d4a017}.tag{font-size:.7rem;letter-spacing:.12em;color:#8a8f94;flex:1;min-width:12rem}nav a{margin-right:.9rem;color:#e3e3e0;font-size:.85rem;text-decoration:none;text-transform:uppercase;letter-spacing:.08em}nav a:hover{color:#d4a017}.lang{color:#d4a017;font-size:.85rem;text-decoration:none;border:1px solid #d4a01766;padding:.15rem .6rem;border-radius:2px}main{max-width:34rem;margin:2rem auto;padding:0 1rem}a{color:#d4a017}h1{font-size:1.3rem;text-transform:uppercase;letter-spacing:.08em;margin:.2rem 0 .8rem}h2{font-size:1rem;text-transform:uppercase;letter-spacing:.08em;color:#d4a017}.card{background:#171a1dcc;border:1px solid #2a2f34;border-top:2px solid #d4a017;border-radius:3px;padding:1rem 1.3rem;margin-bottom:1.2rem}ol{padding-left:1.3rem}.rules{padding-left:1.1rem}.opts{font-size:.85rem;color:#b4b8bb;padding-left:1.1rem}li{margin:.7rem 0}pre,code{background:#0b0c0d;border:1px solid #2a2f34;border-radius:2px;color:#e8c766;font:.85rem Consolas,monospace}pre{padding:.6rem .8rem;margin:.4rem 0;white-space:pre-wrap;word-break:break-all}.dl{display:flex;flex-direction:column;align-items:flex-start;gap:.5rem;margin:.6rem 0}.dl .button{margin:0}.hash{display:block;max-width:100%;font-size:.78rem;padding:.35rem .5rem}li>p{margin:.4rem 0}.card .note{margin:1rem 0 0}.stats{display:flex;gap:2rem;margin-top:1rem;padding-top:.8rem;border-top:1px solid #2a2f34}.stats div{display:flex;flex-direction:column}.stats b{font-size:1.6rem;color:#d4a017;line-height:1.1}.stats span{font-size:.75rem;color:#8a8f94;text-transform:uppercase;letter-spacing:.08em}code{padding:.1rem .3rem;word-break:break-all}label{display:block;margin:.9rem 0 .25rem;font-size:.85rem;color:#b4b8bb}input{width:100%;padding:.55rem .6rem;background:#0f1113;color:inherit;border:1px solid #3a3f44;border-radius:2px;font:inherit}input:focus{outline:0;border-color:#d4a017;box-shadow:0 0 0 2px #d4a01740}button,.button{display:inline-block;margin-top:1.2rem;padding:.65rem 1.5rem;background:#d4a017;color:#101214;border:0;border-radius:2px;font:700 .9rem system-ui,sans-serif;text-transform:uppercase;letter-spacing:.08em;text-decoration:none;cursor:pointer}button:hover,.button:hover{background:#e8b52a}.note{background:#1d2125;border-left:3px solid #d4a017;padding:.7rem .9rem;border-radius:2px}small{color:#8a8f94}label.check{display:flex;gap:.5rem;align-items:center;font-size:.85rem}label.check input{width:auto}.tabs{margin-top:1.2rem}.tabs>input{position:absolute;opacity:0;pointer-events:none}.tabs>label{display:inline-block;margin:0 .25rem 0 0;padding:.45rem .9rem;border:1px solid #2a2f34;border-bottom:0;border-radius:3px 3px 0 0;cursor:pointer;font-size:.78rem;text-transform:uppercase;letter-spacing:.08em;color:#b4b8bb;background:#0f1113}.tabs>input:checked+label{color:#101214;background:#d4a017;border-color:#d4a017}.tabs>input:focus-visible+label{outline:2px solid #e8c766}.panel{display:none;border:1px solid #2a2f34;border-top:2px solid #d4a017;padding:.2rem 1rem 1rem;background:#171a1dcc}#tab-money:checked~.p-money,#tab-ban:checked~.p-ban,#tab-unban:checked~.p-unban,#tab-bans:checked~.p-bans,#tab-maint:checked~.p-maint,#tab-signups:checked~.p-signups{display:block}button.quiet{background:0;color:#d4a017;border:1px solid #d4a01766}button.quiet:hover{background:#d4a01722}table{width:100%;border-collapse:collapse;font-size:.85rem}th,td{text-align:left;padding:.35rem .4rem;border-bottom:1px solid #2a2f34;vertical-align:top;word-break:break-word}th{color:#8a8f94;font-weight:600}footer{max-width:34rem;margin:2rem auto;padding:1rem;border-top:1px solid #2a2f34;font-size:.75rem;color:#6d7378}";

/// A file's size as the download button shows it.
fn file_size(bytes: u64) -> String {
    if bytes >= 1_000_000_000 {
        format!("{:.2} GB", bytes as f64 / 1e9)
    } else {
        format!("{:.1} MB", bytes as f64 / 1e6)
    }
}

fn home(lang: Lang, stats: Stats) -> String {
    let connect = CONNECT.get();
    let address = connect.map_or("127.0.0.1:1234", |connect| connect.address.as_str());
    let local = available_download().and_then(|file| std::fs::metadata(file).ok());
    let download_label = lang.tr("Download the files", "Завантажити файли");
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
    // The game itself comes from elsewhere (step one); this server hands
    // out only the files that go over it.
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
    let original = format!(
        "<li>{}{link}</li>",
        lang.tr(
            "Get the original Survarium 0.21d (build 2010) and unpack it (a .7z archive: 7-Zip opens it).",
            "Отримайте оригінальну Survarium 0.21d (збірка 2010) і розпакуйте її (архів .7z: відкривається 7-Zip)."
        )
    );
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
    let options = format!(
        "<ul class=opts><li><code>-client</code> {}</li><li><code>-localization</code> {}</li></ul>",
        lang.tr("server address", "адреса сервера"),
        lang.tr(
            "language: english, russian, ukrainian",
            "мова: english, russian, ukrainian"
        ),
    );
    let window = session::gather_window_from_env()
        .unwrap_or(session::DEFAULT_GATHER_WINDOW)
        .as_secs();
    let fill = session::fill_from_env();
    let quiet = session::quiet_from_env().as_secs();
    let max = session::MAX_PLAYERS_PER_MATCH;
    let rules = format!(
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
    );
    format!(
        "<section class=card><h1>{}</h1><p>{}</p><p>{}</p><p class=note><b>{}</b> {}</p>{stats}</section> <section class=card><h2>{}</h2><ol> {original}<li>{}<div class=dl>{download}{hash}</div></li> <li>{}<pre>survarium.exe -localization={} -client={}</pre>{}</li> <li>{}</li></ol></section> <section class=card><h2>{}</h2><ul class=rules>{rules}</ul></section> <p><a class=button href=\"/exchange?lang={}\">{}</a></p>",
        lang.tr("Skhid", "Схід"),
        lang.tr(
            "Skhid (Ukrainian for \u{201c}east\u{201d}) is a community server that lets the Survarium 0.21d client sign in, trade, form squads and play matches again.",
            "Схід \u{2014} ком'юніті сервер, на якому клієнт Survarium 0.21d знову може входити, торгувати, збирати загони й грати матчі."
        ),
        lang.tr(
            "Built on the open source project SRP (Survarium Restoration Project), which gave this server its start: <a href=\"https://github.com/srp-survarium/srp\">github.com/srp-survarium/srp</a>.",
            "Створено на основі open source проєкту SRP (Survarium Restoration Project), який дав початок цьому серверу: <a href=\"https://github.com/srp-survarium/srp\">github.com/srp-survarium/srp</a>."
        ),
        lang.tr("Early test server.", "Тестовий сервер."),
        lang.tr(
            "It has bugs and may not work fully. Your progress, items and accounts can be lost or reset at any time, and the server may be stopped at any time. By playing you accept this.",
            "Можливі помилки та збої. Ваш прогрес, предмети й акаунти можуть зникнути або бути скинуті будь-коли, а сервер можуть зупинити будь-коли. Граючи, ви з цим погоджуєтесь."
        ),
        lang.tr("How to play", "Як зіграти"),
        lang.tr(
            "Download our files and unpack them over the game's <code>game</code> folder. They replace <code>survarium.exe</code> and one certificate and add <code>Play.bat</code>; nothing else changes.",
            "Завантажте наші файли й розпакуйте їх поверх теки <code>game</code> гри. Вони замінюють <code>survarium.exe</code> і один сертифікат та додають <code>Play.bat</code>; більше нічого не змінюється."
        ),
        lang.tr("Start it with this command line:", "Запустіть гру з такою командою:"),
        lang.tr("english", "ukrainian"),
        escape(address),
        options,
        lang.tr(
            "Sign in with any user name and password. A new name is registered on its first sign-in; remember the password.",
            "Увійдіть з будь-яким іменем і паролем. Нове ім'я реєструється під час першого входу; запам'ятайте пароль."
        ),
        lang.tr("How matches start", "Як починаються матчі"),
        lang.code(),
        lang.tr("Exchange silver for gold", "Обміняти срібло на золото"),
    )
}

fn exchange_form(lang: Lang) -> String {
    let code = lang.code();
    format!(
        "<h1>{}</h1><form method=post action=/exchange autocomplete=off>\
         <input type=hidden name=lang value={code}>\
         <label for=u>{}</label><input id=u name=user required maxlength=99>\
         <label for=p>{}</label><input id=p name=password type=password required maxlength=47>\
         <label for=g>{}</label><input id=g name=gold type=number min=1 max=10000 required>\
         <small>{}</small><br><button>{}</button></form>",
        lang.tr("Silver for gold", "Срібло на золото"),
        lang.tr("User name", "Ім'я користувача"),
        lang.tr("Password", "Пароль"),
        lang.tr("Gold to buy", "Скільки золота купити"),
        lang.tr(
            "100 silver for each gold (10 000 silver = 100 gold).",
            "100 срібла за 1 золота (10 000 срібла = 100 золота)."
        ),
        lang.tr("Exchange", "Обміняти"),
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
    format!(
        "<h1>{}</h1><form method=post action=\"\" autocomplete=off>\
         <input type=hidden name=lang value={code}>\
         <label for=t>{}</label><input id=t name=token type=password required value=\"{}\">\
         <div class=tabs>{}{}{}{}{}{}\
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
         </div></form>",
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
        }
    }

    fn post(path: &str, form: &[(&str, &str)]) -> Request {
        Request {
            method: "POST".into(),
            path: path.into(),
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
    fn silver_becomes_gold_for_the_right_password_only() {
        let web = web(None);
        web.store.sign_in("fighter", b"secret").unwrap();
        let (silver, gold) = balances(&web, "fighter");

        let wrong = web.exchange(
            &post(
                "/exchange",
                &[("user", "fighter"), ("password", "nope"), ("gold", "1")],
            ),
            "t",
        );
        assert!(wrong.body.contains("Wrong user name or password"));
        assert_eq!(balances(&web, "fighter"), (silver, gold));

        let unknown = web.exchange(
            &post(
                "/exchange",
                &[("user", "ghost"), ("password", "x"), ("gold", "1")],
            ),
            "t",
        );
        assert!(unknown.body.contains("Wrong user name or password"));
        assert!(web.store.account_id_by_name("ghost").is_none());

        let done = web.exchange(
            &post(
                "/exchange",
                &[("user", "Fighter"), ("password", "secret"), ("gold", "2")],
            ),
            "t",
        );
        assert!(done.body.contains("Done"), "{}", done.body);
        assert_eq!(balances(&web, "fighter"), (silver - 200, gold + 2));
    }

    #[test]
    fn an_exchange_cannot_spend_silver_that_is_not_there() {
        let web = web(None);
        web.store.sign_in("poor", b"secret").unwrap();
        let before = balances(&web, "poor");
        let answer = web.exchange(
            &post(
                "/exchange",
                &[("user", "poor"), ("password", "secret"), ("gold", "10000")],
            ),
            "t",
        );
        assert!(answer.body.contains("Not enough silver"), "{}", answer.body);
        assert_eq!(balances(&web, "poor"), before);
        let zero = web.exchange(
            &post(
                "/exchange",
                &[("user", "poor"), ("password", "secret"), ("gold", "0")],
            ),
            "t",
        );
        assert!(zero.body.contains("Gold must be"));
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
            Err(SignInError::Banned)
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

    #[test]
    fn the_admin_page_is_for_its_addresses_and_its_path_only() {
        let web = web(Some("tok"));
        let get = |path: &str, client: &str| {
            web.route(
                &Request {
                    method: "GET".into(),
                    path: path.into(),
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
        assert!(!page(Lang::En, Form::Exchange, "").body.contains("Survarium 0.26g</a>"));
    }

    #[test]
    fn pages_are_branded_unofficial_and_localized() {
        let en = page(Lang::En, Form::Exchange, "").body;
        let uk = page(Lang::Uk, Form::Exchange, "").body;
        assert!(en.contains("СХІД") && en.contains("SKHID") && en.contains("unofficial"));
        assert!(uk.contains("СХІД") && uk.contains("Неофіційний ком'юніті"));
        assert!(uk.contains("Срібло на золото") && en.contains("Silver for gold"));
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

    #[test]
    fn the_home_page_counts_registered_and_online_players() {
        let web = web(None);
        let count = |web: &Web| {
            let body = web
                .route(
                    &Request {
                        method: "GET".into(),
                        path: "/".into(),
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
    fn forms_decode_and_text_is_escaped() {
        assert_eq!(decode("a+b%20c%3D%26%zz%4"), "a b c=&%zz%4");
        assert_eq!(escape("<b>\"&"), "&lt;b&gt;&quot;&amp;");
        assert_eq!(apply(5, -10), 0);
        assert_eq!(delta("+12"), Some(12));
        assert_eq!(delta("x"), None);
    }
}
