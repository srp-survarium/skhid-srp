//! The config file (`srp.toml`, TOML): every real setting of the server in one documented file. The commented example
//! with every key and its default is `config/srp.example.toml`; [`Settings`] mirrors it. A missing file or key = the
//! default. Relative paths are relative to the config file's folder.
//!
//! There are no feature switches: every feature is always on. What stays is what an operator really chooses (addresses,
//! files, the world host's limits, the economy numbers, the schedules no game file has: events, ranked seasons, faction
//! battles, and the server's own lists: game servers, login calendar, store offers).
//!
//! Test and debug knobs are not here: tests build [`crate::Config`] / `LobbyConfig` directly, and the one debug
//! environment variable left (`SRP_RECORD_DIR`, record every online match's input stream) is read by `main` only.

use std::net::{SocketAddr, SocketAddrV4};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;

use lobby_server::actions::{OfferGrant, StoreOffer, StoreTab};
use lobby_server::premium::PremiumOffer;
use lobby_server::servers::GameServer;
use protocol::s2c::quests::BonusDay;

use crate::{Config, DaemonSettings, OnlineSettings, StartMoney, WorldHostSettings};

/// The default config file name (`srp` looks for it in the current folder when no `--config` / `SRP_CONFIG` is given).
pub const DEFAULT_FILE: &str = "srp.toml";

/// The documented example (the same text as `config/srp.example.toml`): every key with its default.
pub const EXAMPLE: &str = include_str!("../../../config/srp.example.toml");

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Settings {
    pub network: Network,
    pub files: Files,
    pub accounts: Accounts,
    pub world_host: WorldHost,
    pub matches: Matches,
    pub social: Social,
    pub league: League,
    /// the server list (9E); absent = one entry "Skhid" on the match host
    pub game_servers: Option<Vec<ServerEntry>>,
    /// the daily login calendar; absent = the server's default 7-day table, empty = no calendar
    pub login_calendar: Option<Vec<CalendarDay>>,
    pub store: Store,
    /// the event schedule; empty = no event
    pub events: Vec<EventEntry>,
    /// the faction battle schedule; empty = no battle
    pub faction_battles: Vec<BattleEntry>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Network {
    pub login: String,
    pub lobby: String,
    pub lobby_public: String,
    #[serde(rename = "match")]
    pub match_bind: String,
    pub match_public: String,
    /// the Steam launcher's update check (`update` module); "" = the login IP with port 65001
    pub update: String,
}

impl Default for Network {
    fn default() -> Self {
        Network {
            login: "127.0.0.1:65000".into(),
            lobby: "127.0.0.1:1235".into(),
            lobby_public: String::new(),
            match_bind: "127.0.0.1:1236".into(),
            match_public: String::new(),
            update: String::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Files {
    pub database: PathBuf,
    pub login_key: PathBuf,
    pub data_dir: PathBuf,
    pub run_dir: PathBuf,
    pub log_dir: PathBuf,
}

impl Default for Files {
    fn default() -> Self {
        Files {
            database: "srp-069d.sqlite".into(),
            login_key: "login_key.pem".into(),
            data_dir: "data".into(),
            run_dir: "run".into(),
            log_dir: "logs".into(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Accounts {
    pub auto_register: bool,
    pub client_version: String,
    pub start_money: Option<MoneyEntry>,
}

impl Default for Accounts {
    fn default() -> Self {
        Accounts { auto_register: true, client_version: String::new(), start_money: None }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MoneyEntry {
    #[serde(default)]
    pub silver: u32,
    #[serde(default)]
    pub gold: u32,
    #[serde(default)]
    pub parts: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct WorldHost {
    pub socket: PathBuf,
    pub exe: PathBuf,
    pub exe_win: String,
    pub bin: PathBuf,
    pub prewarm_dir: PathBuf,
    pub max_matches: u32,
    pub max_zygotes: u32,
    pub idle_secs: u64,
    pub prestart: Vec<String>,
}

impl Default for WorldHost {
    fn default() -> Self {
        WorldHost {
            socket: PathBuf::new(),
            exe: PathBuf::new(),
            exe_win: String::new(),
            bin: PathBuf::new(),
            prewarm_dir: PathBuf::new(),
            max_matches: 4,
            max_zygotes: 8,
            idle_secs: 600,
            prestart: vec!["pve_01_v1_maze2".into()],
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Matches {
    pub min_players: u32,
    pub bot_wait_secs: u64,
    pub report_grace_secs: u64,
    pub equipment_use_costs: Vec<u32>,
    pub match_silver_factors: Vec<f64>,
}

impl Default for Matches {
    fn default() -> Self {
        let l = lobby_server::LobbyConfig::default();
        Matches {
            min_players: l.online_min_players,
            bot_wait_secs: l.online_bot_wait.map_or(0, |d| d.as_secs()),
            report_grace_secs: l.online_report_grace.as_secs(),
            equipment_use_costs: l.equipment_use_costs,
            match_silver_factors: l.match_silver_factors,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Social {
    pub claims_per_day: u32,
    pub compliments_per_day: u32,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct League {
    pub seasons: Vec<SeasonEntry>,
    pub tokens: Vec<u32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SeasonEntry {
    pub number: u16,
    pub start: toml::Value,
    pub end: toml::Value,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServerEntry {
    #[serde(default)]
    pub host: String,
    pub label: String,
    #[serde(default)]
    pub x: u16,
    #[serde(default)]
    pub y: u16,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CalendarDay {
    pub item: u16,
    #[serde(default = "one")]
    pub amount: u32,
    #[serde(default)]
    pub star: bool,
}

fn one() -> u32 {
    1
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Store {
    pub tabs: Option<Vec<TabEntry>>,
    pub offers: Option<Vec<OfferEntry>>,
    pub premium: Option<Vec<PremiumEntry>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TabEntry {
    pub id: u32,
    pub name: String,
    #[serde(default)]
    pub gold: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OfferEntry {
    pub id: u32,
    pub tab: u32,
    pub label: String,
    #[serde(default)]
    pub description: String,
    pub gold: u32,
    #[serde(default)]
    pub items: Vec<(u16, u32)>,
    #[serde(default)]
    pub ttl_secs: Option<u32>,
    pub images: [String; 3],
    pub grant: String,
    #[serde(default)]
    pub premium_hours: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PremiumEntry {
    pub id: u32,
    pub bundle: u32,
    pub gold: u32,
    pub hours: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventEntry {
    #[serde(default = "new_year")]
    pub event: String,
    pub start: toml::Value,
    pub end: toml::Value,
    #[serde(default)]
    pub exchange_end: Option<toml::Value>,
    #[serde(default)]
    pub header: String,
    #[serde(default)]
    pub text: String,
}

fn new_year() -> String {
    EVENT_NEW_YEAR.into()
}

/// The one event the 0.69d client data has rows for (the lobby movie's `globalEventName`; the exchange rows and drops of
/// New Year 2022). The other four event windows of the client have no tables, so they cannot be scheduled.
pub const EVENT_NEW_YEAR: &str = "new_year";

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BattleEntry {
    pub start: toml::Value,
    pub end: toml::Value,
    pub factions: [u8; 2],
}

/// Unix seconds of a TOML time: a datetime with an offset (`2026-12-24T12:00:00Z`), a local datetime or a date (both read
/// as UTC), or an integer (unix seconds).
pub fn unix_of(v: &toml::Value, what: &str) -> Result<i64, String> {
    match v {
        toml::Value::Integer(i) => Ok(*i),
        toml::Value::Datetime(d) => {
            let date = d.date.ok_or_else(|| format!("{what}: {d} has no date"))?;
            let days = game_data::league::days_from_civil(i64::from(date.year), u32::from(date.month), u32::from(date.day));
            let secs = d.time.map_or(0, |t| i64::from(t.hour) * 3600 + i64::from(t.minute) * 60 + i64::from(t.second));
            let off = match d.offset {
                Some(toml::value::Offset::Custom { minutes }) => i64::from(minutes) * 60,
                _ => 0,
            };
            Ok(days * 86400 + secs - off)
        }
        other => Err(format!("{what}: {other} is not a date (e.g. 2026-12-24T12:00:00Z) or unix seconds")),
    }
}

/// `2026-12-24 12:00 UTC` (for the log).
pub fn fmt_utc(t: i64) -> String {
    let z = t.div_euclid(86400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    let s = t.rem_euclid(86400);
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02} UTC", s / 3600, s / 60 % 60)
}

impl Settings {
    pub fn parse(text: &str) -> Result<Settings, String> {
        toml::from_str(text).map_err(|e| e.to_string())
    }
}

/// `path` relative to `base` (an absolute path stays); an empty path = None.
fn resolve(base: &Path, p: &Path) -> Option<PathBuf> {
    if p.as_os_str().is_empty() {
        None
    } else if p.is_absolute() {
        Some(p.to_path_buf())
    } else {
        Some(base.join(p))
    }
}

fn addr(what: &str, v: &str) -> Result<SocketAddr, String> {
    v.parse().map_err(|e| format!("{what} = {v:?}: {e}"))
}

/// The drive a game folder outside `/mnt/<drive>` is shown to the exe as (`G:\binaries\x64\survarium.exe`).
pub const GAME_DRIVE: &str = "G:";

/// The Windows path the exe is told it has. An exe under `/mnt/<drive>/...` (WSL drive mounts) has `<DRIVE>:\...`. Any
/// other exe in the game's own layout `<game folder>/binaries/x64/<exe>` (a native Linux install) is shown as
/// `G:\binaries\x64\<exe>`: the world host maps the Windows folder in front of the common tail of both paths to the Linux
/// game folder (world-host `files::game_mapping`). Anything else needs `world_host.exe_win`.
pub fn exe_win_of(exe: &Path) -> Option<String> {
    let s = exe.to_str()?;
    if let Some(rest) = s.strip_prefix("/mnt/") {
        let mut it = rest.splitn(2, '/');
        let drive = it.next()?;
        let tail = it.next()?;
        if drive.len() == 1 && drive.as_bytes()[0].is_ascii_alphabetic() {
            return Some(format!("{}:\\{}", drive.to_ascii_uppercase(), tail.replace('/', "\\")));
        }
    }
    let name = exe.file_name()?.to_str()?;
    let x64 = exe.parent()?;
    let bin = x64.parent()?;
    let game = bin.parent()?;
    let named = |p: &Path, n: &str| p.file_name().and_then(|f| f.to_str()).is_some_and(|f| f.eq_ignore_ascii_case(n));
    if !(named(x64, "x64") && named(bin, "binaries")) || game.parent().is_none() {
        return None;
    }
    Some(format!("{GAME_DRIVE}\\binaries\\x64\\{name}"))
}

impl Config {
    /// The runtime config of a settings file whose relative paths are relative to `base`.
    pub fn from_settings(s: Settings, base: &Path) -> Result<Config, String> {
        let mut c = Config::default();
        let n = &s.network;
        c.login_addr = addr("network.login", &n.login)?;
        c.lobby_bind = addr("network.lobby", &n.lobby)?;
        c.lobby_public = match (n.lobby_public.is_empty(), c.lobby_bind) {
            (true, SocketAddr::V4(v4)) => v4,
            (true, SocketAddr::V6(_)) => return Err("network.lobby_public: needed for an IPv6 lobby address (the client takes IPv4)".into()),
            (false, _) => n.lobby_public.parse::<SocketAddrV4>().map_err(|e| format!("network.lobby_public = {:?}: {e} (IPv4 address:port)", n.lobby_public))?,
        };
        if c.lobby_public.ip().is_unspecified() {
            log::warn!("network.lobby_public is {}: clients cannot connect to 0.0.0.0; set the address players reach", c.lobby_public);
        }
        let bind = addr("network.match", &n.match_bind)?;
        let public_host = if n.match_public.is_empty() { bind.ip().to_string() } else { n.match_public.clone() };
        if public_host.len() > protocol::s2c::online::MAX_HOST {
            return Err(format!("network.match_public = {public_host:?}: at most {} bytes", protocol::s2c::online::MAX_HOST));
        }
        c.online = OnlineSettings { bind, public_host };
        c.update_addr = if n.update.is_empty() { SocketAddr::new(c.login_addr.ip(), crate::update::DEFAULT_PORT) } else { addr("network.update", &n.update)? };
        if c.update_addr == c.login_addr {
            return Err(format!("network.update = {}: the same address as network.login", c.update_addr));
        }

        let f = &s.files;
        c.db_path = resolve(base, &f.database).ok_or("files.database is empty")?;
        c.key_path = resolve(base, &f.login_key).ok_or("files.login_key is empty")?;
        c.data_dir = resolve(base, &f.data_dir);
        c.run_dir = resolve(base, &f.run_dir).unwrap_or_else(|| base.to_path_buf());
        c.log_dir = resolve(base, &f.log_dir).unwrap_or_else(|| base.to_path_buf());

        c.auto_register = s.accounts.auto_register;
        c.required_version = Some(s.accounts.client_version.clone()).filter(|v| !v.is_empty());
        c.start_money = s.accounts.start_money.map(|m| StartMoney { silver: m.silver, gold: m.gold, parts: m.parts });

        let w = &s.world_host;
        let socket = resolve(base, &w.socket).unwrap_or_else(|| c.run_dir.join("world-host.sock"));
        let daemon = match resolve(base, &w.exe) {
            None => None,
            Some(exe) => {
                let exe_win = if w.exe_win.is_empty() {
                    exe_win_of(&exe).ok_or_else(|| {
                        format!(
                            "world_host.exe = {}: the exe must be survarium.exe in the game folder's binaries/x64 folder (else set world_host.exe_win, the exe's Windows path)",
                            exe.display()
                        )
                    })?
                } else {
                    w.exe_win.clone()
                };
                for p in &w.prestart {
                    if p.is_empty() || p.contains(char::is_whitespace) || p.contains(',') {
                        return Err(format!("world_host.prestart: bad project name {p:?}"));
                    }
                }
                Some(DaemonSettings {
                    bin: resolve(base, &w.bin),
                    exe,
                    exe_win,
                    prewarm_dir: resolve(base, &w.prewarm_dir),
                    max_matches: w.max_matches,
                    max_zygotes: w.max_zygotes,
                    idle_secs: w.idle_secs,
                    prestart: w.prestart.clone(),
                })
            }
        };
        c.world_host = WorldHostSettings { socket, daemon };

        let l = &mut c.lobby;
        let m = &s.matches;
        l.online_min_players = m.min_players;
        l.online_bot_wait = (m.bot_wait_secs > 0).then(|| Duration::from_secs(m.bot_wait_secs));
        l.online_report_grace = Duration::from_secs(m.report_grace_secs);
        l.equipment_use_costs = m.equipment_use_costs.clone();
        l.match_silver_factors = m.match_silver_factors.clone();
        if l.match_silver_factors.iter().any(|f| !f.is_finite() || *f < 0.0) {
            return Err("matches.match_silver_factors: every factor must be a number >= 0".into());
        }
        l.claims_per_day = (s.social.claims_per_day > 0).then_some(s.social.claims_per_day);
        l.compliments_per_day = (s.social.compliments_per_day > 0).then_some(s.social.compliments_per_day);

        let mut seasons = Vec::new();
        for (i, e) in s.league.seasons.iter().enumerate() {
            let what = format!("league.seasons[{i}]");
            let (start, end) = (unix_of(&e.start, &what)?, unix_of(&e.end, &what)?);
            if e.number == 0 || end <= start {
                return Err(format!("{what}: number must be above 0 and end after start"));
            }
            seasons.push(game_data::league::Season { number: e.number, start, end });
        }
        seasons.sort_by_key(|x| x.start);
        if seasons.windows(2).any(|w| w[0].end > w[1].start) {
            return Err("league.seasons overlap".into());
        }
        l.league = game_data::LeagueSchedule { seasons, tokens: s.league.tokens.clone() };

        let host = c.online.public_host.clone();
        l.game_servers = match &s.game_servers {
            None => GameServer::default_list(&host),
            Some(list) => {
                if list.len() > lobby_server::servers::MAX_SERVERS {
                    return Err(format!("game_servers: at most {}", lobby_server::servers::MAX_SERVERS));
                }
                list.iter()
                    .map(|e| GameServer { host: if e.host.is_empty() { host.clone() } else { e.host.clone() }, label: e.label.clone(), x: e.x, y: e.y })
                    .collect()
            }
        };
        if l.game_servers.iter().any(|g| g.host.len() > lobby_server::servers::MAX_TEXT || g.label.len() > lobby_server::servers::MAX_TEXT) {
            return Err(format!("game_servers: host and label at most {} bytes", lobby_server::servers::MAX_TEXT));
        }

        if let Some(days) = &s.login_calendar {
            l.login_calendar = (!days.is_empty()).then(|| days.iter().map(|d| BonusDay { amount: d.amount, reward: d.item, quality: u8::from(d.star) }).collect());
        }

        if let Some(tabs) = &s.store.tabs {
            l.store_tabs = tabs.iter().map(|t| StoreTab { id: t.id, name: t.name.clone(), default_gold: t.gold }).collect();
        }
        if let Some(offers) = &s.store.offers {
            l.store_offers = offers
                .iter()
                .map(|o| {
                    let grant = match o.grant.as_str() {
                        "items" => OfferGrant::Items,
                        "premium" if o.premium_hours > 0 => OfferGrant::Premium { hours: o.premium_hours },
                        "premium" => return Err(format!("store.offers id {}: grant \"premium\" needs premium_hours", o.id)),
                        "winners_contract" => OfferGrant::WinnersContract,
                        g => return Err(format!("store.offers id {}: grant {g:?}: use items, premium or winners_contract", o.id)),
                    };
                    Ok(StoreOffer {
                        id: o.id,
                        tab: o.tab,
                        label: o.label.clone(),
                        description: o.description.clone(),
                        gold: o.gold,
                        items: o.items.clone(),
                        ttl_secs: o.ttl_secs,
                        images: o.images.clone(),
                        grant,
                    })
                })
                .collect::<Result<_, String>>()?;
        }
        if let Some(p) = &s.store.premium {
            l.premium_offers = p.iter().map(|o| PremiumOffer { id: o.id, bundle: o.bundle, gold: o.gold, hours: o.hours }).collect();
        }

        let mut events = Vec::new();
        for (i, e) in s.events.iter().enumerate() {
            let what = format!("events[{i}]");
            if e.event != EVENT_NEW_YEAR {
                return Err(format!("{what}: event {:?}: the 0.69d data has only {EVENT_NEW_YEAR:?}", e.event));
            }
            let (start, end) = (unix_of(&e.start, &what)?, unix_of(&e.end, &what)?);
            let exchange_end = match &e.exchange_end {
                Some(v) => unix_of(v, &what)?,
                None => end,
            };
            if end <= start || exchange_end < end {
                return Err(format!("{what}: need start < end <= exchange_end"));
            }
            for (k, t) in [("header", &e.header), ("text", &e.text)] {
                if !t.is_empty() && !t.starts_with("st_") {
                    log::warn!("{what}.{k} = {t:?} is not a client text key (st_...): the window shows it as it is");
                }
            }
            events.push(game_data::events::Schedule { start, end, exchange_end, label: e.header.clone(), description: e.text.clone() });
        }
        events.sort_by_key(|e| e.start);
        if events.windows(2).any(|w| w[0].exchange_end.max(w[0].end) > w[1].start) {
            return Err("events overlap (one event's exchange window must end before the next starts)".into());
        }
        l.events = events;

        let mut intervals = Vec::new();
        for (i, b) in s.faction_battles.iter().enumerate() {
            let what = format!("faction_battles[{i}]");
            let (start, end) = (unix_of(&b.start, &what)?, unix_of(&b.end, &what)?);
            if end <= start || b.factions.iter().any(|f| !(1..=4).contains(f)) || b.factions[0] == b.factions[1] {
                return Err(format!("{what}: need start < end and two different factions 1..4"));
            }
            intervals.push(game_data::faction_battle::Interval { start, end, factions: b.factions });
        }
        intervals.sort_by_key(|x| x.start);
        if intervals.windows(2).any(|w| w[0].end > w[1].start) {
            return Err("faction_battles overlap".into());
        }
        l.faction_battles = (!intervals.is_empty()).then_some(game_data::faction_battle::Schedule { intervals });
        Ok(c)
    }

    /// Load the config: `path` (must exist), else `srp.toml` in the current folder if it exists, else every default.
    /// Returns the config and the file it came from.
    pub fn load(path: Option<&Path>) -> Result<(Config, Option<PathBuf>), String> {
        let file = match path {
            Some(p) => Some(p.to_path_buf()),
            None => Some(PathBuf::from(DEFAULT_FILE)).filter(|p| p.is_file()),
        };
        let Some(file) = file else {
            let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
            return Ok((Config::from_settings(Settings::default(), &cwd)?, None));
        };
        let text = std::fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
        let s = Settings::parse(&text).map_err(|e| format!("{}: {e}", file.display()))?;
        let abs = std::path::absolute(&file).map_err(|e| format!("{}: {e}", file.display()))?;
        let base = abs.parent().map(Path::to_path_buf).unwrap_or_default();
        let c = Config::from_settings(s, &base).map_err(|e| format!("{}: {e}", file.display()))?;
        Ok((c, Some(abs)))
    }
}

/// One log line per scheduled event (which event, from when until when, running / coming / over), or "no event
/// scheduled".
pub fn event_lines(events: &[game_data::events::Schedule], now: i64) -> Vec<String> {
    if events.is_empty() {
        return vec!["event: no event scheduled".into()];
    }
    events
        .iter()
        .map(|e| {
            let when = if e.active(now) {
                "RUNNING now".to_string()
            } else if e.window_open(now) {
                "drops over, exchange window open".to_string()
            } else if now < e.start {
                format!("starts in {} h", (e.start - now + 3599) / 3600)
            } else {
                "over".to_string()
            };
            let exch = if e.exchange_end > e.end { format!(", exchange until {}", fmt_utc(e.exchange_end)) } else { String::new() };
            format!("event {EVENT_NEW_YEAR}: {} .. {}{exch} ({when})", fmt_utc(e.start), fmt_utc(e.end))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The documented example parses and every value in it is the default: the file documents the real defaults.
    #[test]
    fn the_example_file_is_the_defaults() {
        let base = Path::new("/srv/srp");
        let ex = Config::from_settings(Settings::parse(EXAMPLE).unwrap(), base).unwrap();
        let def = Config::from_settings(Settings::default(), base).unwrap();
        assert_eq!(format!("{ex:?}"), format!("{def:?}"));
        assert_eq!(def.db_path, base.join("srp-069d.sqlite"));
        assert_eq!(def.update_addr, "127.0.0.1:65001".parse::<SocketAddr>().unwrap(), "the update check: the login IP, port 65001");
        assert_eq!(def.world_host.socket, base.join("run/world-host.sock"));
        assert!(def.world_host.daemon.is_none(), "no exe: srp starts no daemon");
        assert!(def.lobby.events.is_empty() && def.lobby.faction_battles.is_none());
        assert_eq!(def.lobby.game_servers.len(), 1);
        assert_eq!(def.lobby.login_calendar, lobby_server::LobbyConfig::default().login_calendar);
    }

    /// The commented example entries of the file, uncommented, are valid and do what their comments say.
    #[test]
    fn the_example_entries_work_when_uncommented() {
        let un: String = EXAMPLE
            .lines()
            .map(|l| l.strip_prefix("#: ").or_else(|| (l == "#:").then_some("")).unwrap_or(l))
            .collect::<Vec<_>>()
            .join("\n");
        let c = Config::from_settings(Settings::parse(&un).unwrap(), Path::new("/srv/srp")).unwrap();
        let e = &c.lobby.events;
        assert_eq!(e.len(), 1);
        assert_eq!((fmt_utc(e[0].start), fmt_utc(e[0].end), fmt_utc(e[0].exchange_end)), ("2026-12-24 12:00 UTC".into(), "2027-01-10 12:00 UTC".into(), "2027-01-17 12:00 UTC".into()));
        assert_eq!(e[0].label, "st_merry_christmas");
        assert_eq!(c.lobby.faction_battles.as_ref().unwrap().intervals.len(), 1);
        assert_eq!(c.lobby.league.seasons.len(), 2);
        assert!(c.world_host.daemon.is_some());
    }

    #[test]
    fn times_are_utc_dates_or_unix_seconds() {
        let t: toml::Table = toml::from_str("a = 2026-12-24T12:00:00Z\nb = 2026-12-24T14:00:00+02:00\nc = 2026-12-24\nd = 1798113600\ne = \"x\"").unwrap();
        assert_eq!(unix_of(&t["a"], "a").unwrap(), 1_798_113_600);
        assert_eq!(unix_of(&t["b"], "b").unwrap(), 1_798_113_600);
        assert_eq!(unix_of(&t["c"], "c").unwrap(), 1_798_070_400);
        assert_eq!(unix_of(&t["d"], "d").unwrap(), 1_798_113_600);
        assert!(unix_of(&t["e"], "e").is_err());
        assert_eq!(fmt_utc(1_798_113_600), "2026-12-24 12:00 UTC");
    }

    #[test]
    fn bad_settings_are_refused_with_the_key() {
        let base = Path::new("/srv");
        let bad = |t: &str| Config::from_settings(Settings::parse(t).map_err(|e| e.to_string()).unwrap(), base).unwrap_err();
        assert!(Settings::parse("[network]\nlogn = \"x\"").is_err(), "unknown keys are refused");
        assert!(bad("[network]\nlogin = \"nope\"").contains("network.login"));
        assert!(bad("[network]\nupdate = \"nope\"").contains("network.update"));
        assert!(bad("[network]\nupdate = \"127.0.0.1:65000\"").contains("network.update"));
        assert!(bad("[[events]]\nevent = \"halloween\"\nstart = 2026-10-25\nend = 2026-11-05").contains("only \"new_year\""));
        assert!(bad("[[events]]\nstart = 2026-12-25\nend = 2026-12-24").contains("start < end"));
        assert!(bad("[[events]]\nstart = 2026-12-01\nend = 2026-12-10\n[[events]]\nstart = 2026-12-05\nend = 2026-12-20").contains("overlap"));
        assert!(bad("[world_host]\nexe = \"/opt/game/survarium.exe\"").contains("exe_win"));
        assert!(bad("[[store.offers]]\nid = 9\ntab = 1\nlabel = \"st_x\"\ngold = 1\nimages = [\"a\", \"b\", \"c\"]\ngrant = \"gift\"").contains("grant"));
    }

    #[test]
    fn the_update_check_follows_the_login_ip_and_players_get_the_reachable_host() {
        let base = Path::new("/srv");
        let c = |t: &str| Config::from_settings(Settings::parse(t).unwrap(), base).unwrap();
        assert_eq!(c("").player_host().as_deref(), Some("127.0.0.1"));
        let open = c("[network]\nlogin = \"0.0.0.0:65000\"\nlobby = \"0.0.0.0:1235\"\nlobby_public = \"203.0.113.5:1235\"\nmatch = \"0.0.0.0:1236\"\nmatch_public = \"203.0.113.5\"");
        assert_eq!(open.update_addr, "0.0.0.0:65001".parse::<SocketAddr>().unwrap());
        assert_eq!(open.player_host().as_deref(), Some("203.0.113.5"));
        let named = c("[network]\nlogin = \"127.0.0.1:65000\"\nmatch_public = \"play.example.org\"\nupdate = \"0.0.0.0:8080\"");
        assert_eq!(named.player_host().as_deref(), Some("play.example.org"));
        assert_eq!(named.update_addr.port(), 8080);
        let none = c("[network]\nlogin = \"0.0.0.0:65000\"\nlobby = \"0.0.0.0:1235\"\nlobby_public = \"0.0.0.0:1235\"\nmatch = \"0.0.0.0:1236\"");
        assert_eq!(none.player_host(), None);
    }

    #[test]
    fn the_exe_windows_path_comes_from_the_wsl_mount() {
        assert_eq!(exe_win_of(Path::new("/mnt/d/games/survarium/binaries/x64/survarium.exe")).as_deref(), Some("D:\\games\\survarium\\binaries\\x64\\survarium.exe"));
        assert_eq!(exe_win_of(Path::new("/opt/survarium.exe")), None);
        // native Linux: the game folder becomes drive G:
        assert_eq!(exe_win_of(Path::new("/opt/survarium/game/binaries/x64/survarium.exe")).as_deref(), Some("G:\\binaries\\x64\\survarium.exe"));
        assert_eq!(exe_win_of(Path::new("/opt/game/bin/survarium.exe")), None);
    }

    #[test]
    fn the_event_log_line_says_which_event_runs_until_when() {
        assert_eq!(event_lines(&[], 0), vec!["event: no event scheduled".to_string()]);
        let s = game_data::events::Schedule { start: 1_798_113_600, end: 1_798_113_600 + 86400, exchange_end: 1_798_113_600 + 86400, label: String::new(), description: String::new() };
        assert_eq!(event_lines(std::slice::from_ref(&s), 1_798_113_600 + 60), vec!["event new_year: 2026-12-24 12:00 UTC .. 2026-12-25 12:00 UTC (RUNNING now)".to_string()]);
        assert!(event_lines(&[s], 0)[0].ends_with("h)"));
    }
}
