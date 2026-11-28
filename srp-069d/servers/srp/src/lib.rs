//! srp (0.69d): login (TCP), lobby (UDP) and the match server (UDP) in one process, with the world host daemon.
//!
//! * [`Config`]: addresses, files, accounts, world host, lobby settings; [`Config::load`] reads the config file
//!   ([`settings`], documented example `config/srp.example.toml`). Every feature is always on: no switches.
//! * [`AccountAuth`]: the login server's `Authenticator` over the `accounts` store. The 40-hex digest the client sends
//!   is the account's password (the client never sends the typed text). An unknown name is registered on its first
//!   sign-in when `auto_register` is on (the digest becomes the password); a known name needs the same digest.
//!   A granted sign-in gets a session id from the shared [`SessionTable`]; the lobby redeems it on `2E`. A Steam-mode
//!   client signs in through its Import panel with the same name + password check (the login server computes the
//!   digest from the typed password); the SteamID64 it claims is stored per account as information only.
//! * [`world_host`]: the world host daemon srp starts itself (config generated from the data and the settings).
//! * [`update`]: the Steam launcher's update check ("latest version"), so the Steam Play button reaches the game.

pub mod host_room;
pub mod matches;
pub mod online;
pub mod settings;
pub mod setup;
pub mod update;
pub mod world_host;

use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener, UdpSocket};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use accounts::{BanKind, Error as AccountError, Store};
use game_data::GameData;
use lobby_server::{Lobby, LobbyConfig, LocalMatches, Server, SessionTable};
use login_server::{Attempt, Authenticator, Granted, LoginKey, Refusal};

#[derive(Debug, Clone)]
pub struct Config {
    /// TCP address of the login server (`-server=` of the client).
    pub login_addr: SocketAddr,
    /// UDP address the lobby binds.
    pub lobby_bind: SocketAddr,
    /// IPv4 address and port the login server tells the client to open the lobby connection to.
    pub lobby_public: SocketAddrV4,
    pub db_path: PathBuf,
    /// PEM private key of the login server (RSA-1024).
    pub key_path: PathBuf,
    /// Directory with replacement game tables; None = the tables compiled in.
    pub data_dir: Option<PathBuf>,
    /// Generated files (the world host's daemon.cfg, its default socket).
    pub run_dir: PathBuf,
    /// Logs (world-host.log, the daemon's output folder; srp.log when started by scripts/srp-host.sh).
    pub log_dir: PathBuf,
    pub auto_register: bool,
    /// Money of NEW accounts (silver, gold, spare parts) in place of the kit's zero money; None (default) = the real
    /// start with no money (progression audit).
    pub start_money: Option<StartMoney>,
    /// See [`AlreadyOnline`] (not in the config file: the server refuses; tests check both).
    pub already_online: AlreadyOnline,
    /// Accepted client version string; None = any.
    pub required_version: Option<String>,
    pub lobby: LobbyConfig,
    /// The match server's socket and what the clients are told.
    pub online: OnlineSettings,
    /// The world host daemon (required for online matches).
    pub world_host: WorldHostSettings,
    /// TCP address of the Steam launcher's update check ([`update`]: "latest version", so its Play button turns on).
    pub update_addr: SocketAddr,
    /// Debug only (environment `SRP_RECORD_DIR`, never the config file): record every online match's full input stream
    /// there (relay.rs `Recorder`).
    pub record_dir: Option<PathBuf>,
}

/// Where the match server listens and what the clients are told (the start worlds come from the world host).
#[derive(Debug, Clone)]
pub struct OnlineSettings {
    /// UDP address the match server binds.
    pub bind: SocketAddr,
    /// Host text the lobby gives the clients in `82` (at most 63 bytes); default the bind address.
    pub public_host: String,
}

/// The world host daemon: its socket, and how srp starts one when none answers there.
#[derive(Debug, Clone)]
pub struct WorldHostSettings {
    pub socket: PathBuf,
    /// None = srp starts no daemon (one must already run on `socket`).
    pub daemon: Option<DaemonSettings>,
}

/// The settings of a daemon srp starts itself ([`world_host`]); the project list comes from the game data.
#[derive(Debug, Clone)]
pub struct DaemonSettings {
    /// the world host binary; None = `world-host` next to the srp binary
    pub bin: Option<PathBuf>,
    pub exe: PathBuf,
    pub exe_win: String,
    /// pre-warm lists; None = `prewarm` next to the world host binary if it exists
    pub prewarm_dir: Option<PathBuf>,
    pub max_matches: u32,
    pub max_zygotes: u32,
    pub idle_secs: u64,
    pub prestart: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        let lobby = SocketAddrV4::new(Ipv4Addr::LOCALHOST, 1235);
        let mut lc = LobbyConfig::default();
        lc.game_servers = lobby_server::servers::GameServer::default_list("127.0.0.1");
        Config {
            login_addr: SocketAddr::from(([127, 0, 0, 1], 65000)),
            lobby_bind: SocketAddr::V4(lobby),
            lobby_public: lobby,
            db_path: PathBuf::from("srp-069d.sqlite"),
            key_path: PathBuf::from("login_key.pem"),
            data_dir: Some(PathBuf::from("data")),
            run_dir: PathBuf::from("run"),
            log_dir: PathBuf::from("logs"),
            auto_register: true,
            start_money: None,
            already_online: AlreadyOnline::default(),
            required_version: None,
            lobby: lc,
            online: OnlineSettings { bind: SocketAddr::from(([127, 0, 0, 1], 1236)), public_host: "127.0.0.1".into() },
            world_host: WorldHostSettings { socket: PathBuf::from("run/world-host.sock"), daemon: None },
            update_addr: SocketAddr::from(([127, 0, 0, 1], update::DEFAULT_PORT)),
            record_dir: None,
        }
    }
}

impl Config {
    /// The address players use for this server (`-server=`, the Steam setup script): the first of the login IP, the
    /// lobby's public IP and the match host that is not 0.0.0.0, preferring one that is not a loopback address. None =
    /// every one is 0.0.0.0 (the operator must tell players the address).
    pub fn player_host(&self) -> Option<String> {
        let candidates: Vec<IpAddr> = [Some(self.login_addr.ip()), Some(IpAddr::V4(*self.lobby_public.ip())), self.online.public_host.parse::<IpAddr>().ok()]
            .into_iter()
            .flatten()
            .filter(|ip| !ip.is_unspecified())
            .collect();
        if let Some(ip) = candidates.iter().find(|ip| !ip.is_loopback()) {
            return Some(ip.to_string());
        }
        // a host name in match_public (not an IP) is what players reach
        let h = &self.online.public_host;
        if !h.is_empty() && h.parse::<IpAddr>().is_err() {
            return Some(h.clone());
        }
        candidates.first().map(|ip| ip.to_string())
    }
}

/// Money a new account starts with (`accounts.start_money`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StartMoney {
    pub silver: u32,
    pub gold: u32,
    pub parts: u32,
}

impl StartMoney {
    /// `silver,gold,parts`, e.g. `1000000,10000,1000`.
    pub fn parse(v: &str) -> Result<StartMoney, String> {
        let n: Vec<u32> = v
            .split(',')
            .map(|x| x.trim().parse::<u32>().map_err(|e| format!("{x:?}: {e}")))
            .collect::<Result<_, _>>()?;
        match n[..] {
            [silver, gold, parts] => Ok(StartMoney { silver, gold, parts }),
            _ => Err("expected silver,gold,parts".into()),
        }
    }
}

/// What a second sign-in of an account that is already online does (the server refuses; `Kick` is kept for tests).
///
/// * `Refuse` (default): the login server answers `0C 07` ("already online", a text the client has,
///   0x20012A3C1). A crashed client stays "online" until the lobby times its connection out (10 s without a
///   datagram), so the player waits that long.
/// * `Kick`: the sign-in is granted and the lobby drops the old connection when the new 2E arrives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlreadyOnline {
    #[default]
    Refuse,
    Kick,
}

/// Failed sign-ins allowed per account name AND client address inside `window` before `0C 05` (sign-in attempt
/// interval violated). Choice: 5 failures in 60 seconds, then that name is locked out FOR THAT ADDRESS until the window
/// ends. Per name + address (CHOICE): failures from a stranger's address cannot lock the real player out,
/// and every address is still limited in guessing a password.
#[derive(Debug, Clone, Copy)]
pub struct AttemptLimit {
    pub failures: u32,
    pub window: Duration,
}

impl Default for AttemptLimit {
    fn default() -> Self {
        AttemptLimit { failures: 5, window: Duration::from_secs(60) }
    }
}

/// The login server's authenticator over the account store.
pub struct AccountAuth {
    pub store: Arc<Store>,
    pub sessions: Arc<SessionTable>,
    pub lobby: SocketAddrV4,
    pub auto_register: bool,
    pub already_online: AlreadyOnline,
    /// The one client version string accepted (the sign-in request carries it, e.g. `0.69d0`); None = any.
    pub required_version: Option<String>,
    pub limit: AttemptLimit,
    /// Game data for the profile of a new account (the data's new-player kit). None = the test starter profile.
    pub data: Option<Arc<GameData>>,
    /// Test money for new accounts (`Config::start_money`).
    pub start_money: Option<StartMoney>,
    /// (lower-case login name, client address) -> (failures, last failure)
    failures: Mutex<HashMap<(String, Option<IpAddr>), (u32, Instant)>>,
}

impl AccountAuth {
    pub fn new(store: Arc<Store>, sessions: Arc<SessionTable>, lobby: SocketAddrV4, auto_register: bool) -> AccountAuth {
        AccountAuth {
            store,
            sessions,
            lobby,
            auto_register,
            already_online: AlreadyOnline::default(),
            required_version: None,
            limit: AttemptLimit::default(),
            data: None,
            start_money: None,
            failures: Mutex::new(HashMap::new()),
        }
    }

    /// Create the account of a first sign-in: the data's new-player kit (`lobby_server::progress::new_player`); without
    /// game data (tests) the test starter profile. The setting `start_money` replaces the kit's money.
    pub fn register(&self, name: &str, password: &str) -> Result<accounts::AccountId, AccountError> {
        match &self.data {
            Some(d) => {
                // the kit's modules get their green roll here (lobby_server::progress::new_player)
                let mut p = lobby_server::progress::new_player(d, 1, &mut lobby_server::fresh_roll());
                if let Some(m) = self.start_money {
                    p.silver = m.silver;
                    p.gold = m.gold;
                    p.parts = m.parts;
                }
                self.store.create_account_with_profile(name, password, &p)
            }
            None => self.store.create_account(name, password),
        }
    }

    /// Is the account name locked out for the client address `peer` at `now`?
    pub fn locked_out(&self, name: &str, peer: Option<IpAddr>, now: Instant) -> bool {
        let mut m = self.failures.lock().unwrap();
        m.retain(|_, (_, t)| now.saturating_duration_since(*t) < self.limit.window);
        m.get(&(name.to_lowercase(), peer)).is_some_and(|(n, _)| *n >= self.limit.failures)
    }

    fn record_failure(&self, name: &str, peer: Option<IpAddr>, now: Instant) {
        let mut m = self.failures.lock().unwrap();
        let e = m.entry((name.to_lowercase(), peer)).or_insert((0, now));
        if now.saturating_duration_since(e.1) >= self.limit.window {
            *e = (0, now);
        }
        e.0 += 1;
        // the lockout runs from the last failure
        e.1 = now;
    }

    fn fail(&self, name: &str, peer: Option<IpAddr>, now: Instant) -> Refusal {
        self.record_failure(name, peer, now);
        Refusal::BAD_CREDENTIALS
    }

    /// The whole decision at `now` (unix seconds `unix` for ban expiry).
    pub fn decide(&self, a: &Attempt<'_>, now: Instant, unix: i64) -> Result<Granted, Refusal> {
        if let Some(v) = &self.required_version {
            if a.client_version != v {
                log::info!("login of {:?}: client version {:?} is not {v:?}", a.account, a.client_version);
                return Err(Refusal::INVALID_VERSION);
            }
        }
        if self.locked_out(a.account, a.peer, now) {
            log::info!("login of {:?}: too many failed attempts from this address", a.account);
            return Err(Refusal::ATTEMPT_INTERVAL);
        }
        let id = match self.store.find_account(a.account) {
            Ok(Some(_)) => match self.store.verify_login(a.account, a.digest) {
                Ok(id) => id,
                Err(AccountError::BadCredentials) => {
                    log::info!("login of {:?}: wrong password digest (signin type {}, password type {}, {} digest bytes)", a.account, a.signin_type, a.password_type, a.digest.len());
                    return Err(self.fail(a.account, a.peer, now));
                }
                Err(e) => {
                    log::error!("login of {:?}: {e}", a.account);
                    return Err(Refusal::BAD_CREDENTIALS);
                }
            },
            Ok(None) if self.auto_register => match self.register(a.account, a.digest) {
                Ok(id) => {
                    log::info!("account {:?} registered by its first sign-in (id {id})", a.account);
                    id
                }
                Err(e) => {
                    log::info!("account {:?} not registered: {e}", a.account);
                    return Err(self.fail(a.account, a.peer, now));
                }
            },
            Ok(None) => {
                log::info!("login of unknown account {:?} refused", a.account);
                return Err(self.fail(a.account, a.peer, now));
            }
            Err(e) => {
                log::error!("account lookup of {:?}: {e}", a.account);
                return Err(Refusal::BAD_CREDENTIALS);
            }
        };
        // the password was right: the failure count starts again
        self.failures.lock().unwrap().remove(&(a.account.to_lowercase(), a.peer));
        match self.store.ban_at(id, unix) {
            Ok(Some(ban)) => {
                let minutes = ban.minutes_left(unix);
                log::info!("login of {:?}: {:?} (reason {})", a.account, ban.kind, ban.reason);
                return Err(match ban.kind {
                    BanKind::Banned => Refusal::banned(ban.reason, minutes),
                    BanKind::Suspended => Refusal::suspended(ban.reason, minutes),
                });
            }
            Ok(None) => {}
            Err(e) => {
                log::error!("ban lookup of {:?}: {e}", a.account);
                return Err(Refusal::BAD_CREDENTIALS);
            }
        }
        if self.already_online == AlreadyOnline::Refuse && self.sessions.is_online(id) {
            log::info!("login of {:?}: already online", a.account);
            return Err(Refusal::ALREADY_ONLINE);
        }
        // Steam mode (the Import panel): keep the SteamID64 the client claimed, as information only (it is unverified
        // and never part of the decision above)
        if let Some(steam_id) = a.steam_id {
            match self.store.set_steam_id(id, steam_id, unix) {
                Ok(()) => log::info!("login of {:?} in steam mode: claimed SteamID64 {steam_id} stored (information only)", a.account),
                Err(e) => log::error!("login of {:?}: storing the claimed SteamID64 {steam_id}: {e}", a.account),
            }
        }
        Ok(Granted { session: self.sessions.grant(id), lobby: self.lobby })
    }
}

impl Authenticator for AccountAuth {
    fn authenticate(&self, a: &Attempt<'_>) -> Result<Granted, Refusal> {
        let unix = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
        self.decide(a, Instant::now(), unix)
    }
}

/// Admin commands on the account database (run as `srp <command> ...`, the server need not be stopped):
///
/// * `ban <account> <ban|suspend> <reason 0..15> <minutes, 0 = permanent>`: the next sign-in is refused with `0C 02`
///   (banned; reason 15 shows the "soft ban" text) or `0C 03` (suspended) and the reason text and end time.
/// * `unban <account>`: lifts it.
/// * `bans`: not supported (look into the database).
/// * `comments [<account> [<limit>]]`: the complaints and compliments players filed (`48 06`), newest first; with an
///   account only the ones about it.
///
/// Returns the line to print.
pub fn admin(args: &[String], store: &Store) -> Result<String, String> {
    let find = |name: &str| -> Result<accounts::Account, String> {
        store.find_account(name).map_err(|e| e.to_string())?.ok_or_else(|| format!("no account {name:?}"))
    };
    match args.first().map(String::as_str) {
        Some("ban") if args.len() == 5 => {
            let acc = find(&args[1])?;
            let kind = match args[2].as_str() {
                "ban" => BanKind::Banned,
                "suspend" => BanKind::Suspended,
                other => return Err(format!("kind {other:?}: use ban or suspend")),
            };
            let reason: u8 = args[3].parse().map_err(|_| "reason must be 0..15".to_string())?;
            let minutes: u32 = args[4].parse().map_err(|_| "minutes must be a number (0 = permanent)".to_string())?;
            store.set_ban(acc.id, kind, reason, minutes).map_err(|e| e.to_string())?;
            Ok(format!("{} is now {:?} (reason {reason}, {})", acc.name, kind, if minutes == 0 { "permanent".to_string() } else { format!("{minutes} minutes") }))
        }
        Some("unban") if args.len() == 2 => {
            let acc = find(&args[1])?;
            store.clear_ban(acc.id).map_err(|e| e.to_string())?;
            Ok(format!("{} is free to sign in", acc.name))
        }
        Some("comments") if args.len() <= 3 => {
            let target = match args.get(1) {
                Some(n) => Some(find(n)?.id),
                None => None,
            };
            let limit: u32 = match args.get(2) {
                Some(n) => n.parse().map_err(|_| "limit must be a number".to_string())?,
                None => 50,
            };
            let rows = store.list_comments(target, limit).map_err(|e| e.to_string())?;
            let mut out = format!("{} comment(s), newest first\n", rows.len());
            for r in rows {
                let kind = if r.kind == accounts::comments::KIND_COMPLAINT { "complaint" } else { "compliment" };
                out.push_str(&format!("#{} unix {} {} from {} about {} (a={}, b={})\n", r.id, r.at, kind, r.from_name, r.target_name, r.a, r.b));
            }
            Ok(out.trim_end().to_string())
        }
        _ => Err("usage: srp ban <account> <ban|suspend> <reason 0..15> <minutes|0>  |  srp unban <account>  |  srp comments [account [limit]]".into()),
    }
}


/// `srp account add <name> <password>` (a new account exactly as its first sign-in would make it: the data's new-player
/// kit, `start_money`) and `srp account password <name> <password>` (a new password). The stored password is the digest
/// the client sends for that password (`login_server::digest::client_password_digest`), so the player signs in with it
/// through the normal form and the Steam Import panel alike. Works whatever `auto_register` says.
pub fn account_admin(args: &[String], c: &Config, store: Arc<Store>, data: Arc<GameData>) -> Result<String, String> {
    let usage = "usage: srp account add <name> <password>  |  srp account password <name> <password>";
    let (Some(cmd), Some(name), Some(password), None) = (args.get(1), args.get(2), args.get(3), args.get(4)) else {
        return Err(usage.into());
    };
    let digest = login_server::digest::client_password_digest(password.as_bytes());
    match cmd.as_str() {
        "add" => {
            let sessions = Arc::new(SessionTable::persistent(store.clone()));
            let mut auth = AccountAuth::new(store, sessions, c.lobby_public, true);
            auth.data = Some(data);
            auth.start_money = c.start_money;
            let id = auth.register(name, &digest).map_err(|e| format!("account {name:?}: {e}"))?;
            Ok(format!("account {name:?} created (id {id}); it signs in with this name and password"))
        }
        "password" => {
            let acc = store.find_account(name).map_err(|e| e.to_string())?.ok_or_else(|| format!("no account {name:?}"))?;
            store.change_password(acc.id, &digest).map_err(|e| format!("account {name:?}: {e}"))?;
            Ok(format!("account {name:?} has a new password"))
        }
        _ => Err(usage.into()),
    }
}

/// Everything `run` needs, built from a [`Config`].
pub struct Parts {
    pub store: Arc<Store>,
    pub sessions: Arc<SessionTable>,
    pub data: Arc<GameData>,
    pub key: Arc<LoginKey>,
}

/// The game tables of `files.data_dir` (`srp setup` builds them from the game install).
pub fn load_tables(c: &Config) -> Result<GameData, String> {
    let d = c.data_dir.as_ref().ok_or("files.data_dir is not set: run `srp setup` (it builds the game tables from your game install)")?;
    GameData::from_dir(d).map_err(|e| format!("game data: {e}"))
}

pub fn open_parts(c: &Config) -> Result<Parts, String> {
    let store = Arc::new(Store::open(&c.db_path).map_err(|e| format!("database {}: {e}", c.db_path.display()))?);
    let data = load_tables(c)?;
    let key = LoginKey::from_file(&c.key_path).map_err(|e| format!("login key {}: {e}", c.key_path.display()))?;
    // sessions in the database: the client's own lobby reconnect after a server restart re-sends its session (2E)
    let sessions = Arc::new(SessionTable::persistent(store.clone()));
    Ok(Parts { store, sessions, data: Arc::new(data), key: Arc::new(key) })
}

/// Start the login server on its own thread, the match server on its own, and run the lobby loop on this one until
/// `stop` is set. The world host must answer on `c.world_host.socket` for online matches ([`world_host::start`] starts
/// one first when needed); its project list is asked for in the background.
pub fn run(c: &Config, parts: Parts, matches: Arc<dyn LocalMatches>, stop: &AtomicBool) -> std::io::Result<()> {
    let mut auth = AccountAuth::new(parts.store.clone(), parts.sessions.clone(), c.lobby_public, c.auto_register);
    auth.already_online = c.already_online;
    auth.required_version = c.required_version.clone();
    auth.data = Some(parts.data.clone());
    auth.start_money = c.start_money;
    if let Some(m) = c.start_money {
        log::warn!("accounts.start_money: new accounts start with {} silver, {} gold, {} parts", m.silver, m.gold, m.parts);
    }
    let auth = Arc::new(auth);
    let listener = TcpListener::bind(c.login_addr)?;
    log::info!("login on tcp {}, lobby on udp {} (client is told {})", c.login_addr, c.lobby_bind, c.lobby_public);
    let unix_now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    for l in settings::event_lines(&c.lobby.events, unix_now) {
        log::info!("{l}");
    }
    let key = parts.key.clone();
    // the player start data on the update port (`/skhid/client.txt`, scripts/player/skhid-setup.cmd): public values only
    let start_data = update::client_txt(c.player_host().as_deref(), c.login_addr.port());
    std::thread::spawn(move || {
        let on_outcome: Arc<dyn Fn(&login_server::Outcome) + Send + Sync> =
            Arc::new(|o| log::debug!("login outcome: {o:?}"));
        if let Err(e) = login_server::serve(listener, key, login_server::DEFAULT_PASSWORD_TYPE, auth, on_outcome) {
            log::error!("login server stopped: {e}");
        }
    });
    let update = TcpListener::bind(c.update_addr)?;
    log::info!(
        "update check on tcp {} (the Steam launcher's update_server {}:{}: answers \"latest version\", so its Play button turns on; docs/notes/steam.md; also serves {} for skhid-setup.cmd)",
        c.update_addr,
        c.player_host().unwrap_or_else(|| "<server address>".into()),
        c.update_addr.port(),
        update::CLIENT_TXT_PATH
    );
    std::thread::spawn(move || update::serve(update, Some(start_data)));
    let socket = UdpSocket::bind(c.lobby_bind)?;
    let lobby = Lobby::new(c.lobby.clone(), parts.store, parts.data, parts.sessions, matches);
    let o = &c.online;
    let mut hub_cfg = match_server::HubConfig { public_host: o.public_host.clone(), ..Default::default() };
    // the match clock runs at the wall clock's rate (match_server::clock: a VM's monotonic clock can run several percent
    // slow, and every client's world with it); off only in tests, which drive the hub with virtual instants
    hub_cfg.discipline_clock = true;
    if let Some(d) = &c.record_dir {
        log::warn!("debug SRP_RECORD_DIR: online matches are recorded to {}", d.display());
        hub_cfg.match_config.record_dir = Some(d.clone());
    }
    #[cfg(unix)]
    {
        let h = match_server::HostConfig::unix(&c.world_host.socket);
        log::info!(
            "world host {}: start worlds, ends, winners, statistics, E0, resync and AI players of every online match (a DF waits at most {} ms for the host's bots)",
            c.world_host.socket.display(),
            h.bots_hold_ms
        );
        hub_cfg.match_config.world_host = Some(h);
    }
    #[cfg(not(unix))]
    log::warn!("the world host needs a unix socket (Linux / WSL2): every online start is refused on this system");
    let online = Arc::new(online::Online::new(hub_cfg, match_server::WorldStore::new()));
    // the projects the world host serves: the PvP queue offers (AC) and plays only those rows of the data
    #[cfg(unix)]
    {
        let (online, socket) = (online.clone(), c.world_host.socket.clone());
        std::thread::spawn(move || world_host::fetch_projects(&socket, &online));
    }
    // the world host's room (loaded maps, their matches, whether a new map loads) about every 0.5 s: the queues start
    // only matches the host can take now
    #[cfg(unix)]
    {
        let (online, socket) = (online.clone(), c.world_host.socket.clone());
        std::thread::spawn(move || world_host::watch_room(&socket, &online));
    }
    let ms = online.start_server(o.bind)?;
    log::info!("match server on udp {} (clients are told {})", ms.local_addr(), o.public_host);
    // the match server lives as long as the lobby loop; dropping it stops its thread
    let _match_server = ms;
    let lobby = lobby.with_online(online);
    let mut server = Server::new(lobby);
    lobby_server::net::serve(&mut server, &socket, stop)
}

/// Default local match hook: Training and tutorial options, results and rewards of `crates/local-match`.
pub fn default_matches() -> Arc<dyn LocalMatches> {
    Arc::new(matches::Matches::default())
}
