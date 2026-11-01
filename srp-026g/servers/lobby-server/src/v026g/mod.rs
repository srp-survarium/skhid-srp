//! The 0.26g lobby (`resources/notes/v026g/lobby.md`, `lobby_requests.md`,
//! `social.md`), beside the 0.21d one.
//!
//! What 0.26g changed: the lobby is one UDP link on the match transport
//! (`udp-transport`, `Table::Lobby`), whose connect payload is the sign-in
//! `2E`, and the chat, friends and squads travel on it too. So this is one
//! socket, one thread and one [`Transport`](udp_transport::Transport) per
//! client address:
//!
//! - `core`: the links: sign-in, the session's hold, timeouts, sign-out;
//! - `dispatch`: every client request and its answer;
//! - `play`: Play, making a match and coming back from it;
//! - `rooms`: rooms, the players' "servers": every PvP match is a room's
//!   (the owner's of 2026-10-17), and Play goes into one;
//! - `pvb`: "Player vs Bots": one player against the match server's own
//!   bots, chosen in srp's changed lobby movie;
//! - `replay`: the "Watch replay" of the movie's History: the ask, the
//!   polls, the download link (`SRP_REPLAY_DIR`, set where srp starts the
//!   lobby: [`Lobby::with_replays`]);
//! - `social`: chat, friends, important messages, squads;
//! - `clans`: clans, clan chat and clan-team Play.
//!
//! The account's rules are `session::v026g::rules`; the messages are
//! `survarium::v026g::lobby`. The matches' seats are the store's
//! ([`session::Matchmaking`]), the squads are the store's too.
//!
//! ```no_run
//! use std::sync::Arc;
//! use lobby_server::v026g::{self, Inbox, MatchHost, MatchSetup, MatchTicket};
//!
//! struct Host;
//! impl MatchHost for Host {
//!     fn create(&self, _setup: MatchSetup) -> Result<MatchTicket, String> {
//!         Err("no match server".into())
//!     }
//!     fn is_open(&self, _match_id: u32) -> bool {
//!         false
//!     }
//!     fn reseat(&self, _match_id: u32, _account_id: u32, _session_id: u32) -> bool {
//!         false
//!     }
//! }
//!
//! let store = Arc::new(session::SessionStore::new());
//! let accounts = Arc::new(session::v026g::GameAccounts::new());
//! let results = Arc::new(session::v026g::rewards::Results::new());
//! let inbox = Inbox::new();
//! // The match server's results: `results.settle(..)`, then
//! // `inbox.match_finished(..)`.
//! v026g::run(store, accounts, results, Arc::new(Host), inbox).unwrap();
//! ```
//!
//! Settings (environment, read once at start):
//! - `SRP_BIND_HOST` / `SRP_LOBBY_BIND`, `SRP_LOBBY_PORT` (default 1235),
//!   `SRP_PUBLIC_HOST` / `SRP_LOBBY_PUBLIC_HOST`, `SRP_LOCAL_PEERS`: the
//!   address (`vostok::config`), the one the 0.26g login server announces;
//! - `SRP_BALANCE_PLAYER_KD` (default 1): what a player counts for by
//!   being there when the sides are made, in K/Ds, beside its K/D
//!   (`session::v026g::balance`, `docs/notes/kd-balance.md`);
//! - `SRP_BALANCE_RATED_PLAYERS` (default 4): the fewest players a match
//!   must have had in it to move the balance rating (read where a match
//!   is settled: `session::v026g::balance::rated_players`);
//! - `SRP_BALANCE_BATTERY_LEAD` (default 0.55), `SRP_BALANCE_TICKET_LEAD`
//!   (0.2): in a match being played, what a battery and a ticket of lead
//!   count for, in players (a late joiner goes to the side that counts for
//!   less: `rooms`);
//! - `SRP_MIN_PATCH_REVISION` (default none; not a number from 0 to 255,
//!   or one under the newest patch of the game data, stops srp
//!   (`check_min_patch_revision`)): a game older than that patch revision
//!   is refused at its sign-in, and its sessions from before in the lobby
//!   (`patch_check`);
//! - `SRP_MATCH_MODES`: the game modes to play, comma-separated (default
//!   `2,3,4`), as for 0.21d with 0.26g's modes;
//! - `SRP_MATCH_MAPS`: the `maps_dict` ids to play, comma-separated
//!   (default: every playable one of the modes): a room's pool by default,
//!   and what a clan match is drawn from;
//! - `SRP_CLAN_COST` (silver, default 50000), `SRP_CLAN_PLAYERS_REQUIRED`
//!   (default 1), `SRP_CLAN_TEAM_MIN` / `SRP_CLAN_TEAM_MAX` (default 1 and
//!   10), `SRP_CLAN_MAX_MEMBERS` (default 30): the clan rules' numbers
//!   (`session::v026g::clans::Config`);
//! - `SRP_LOBBY_LINK_TIMEOUT_SECONDS` (default 30): how long a silent
//!   client keeps its link;
//! - `SRP_LOBBY_MATCH_LINK_TIMEOUT_SECONDS` (default 120): the same for a
//!   client that is in a match (it may be loading its level);
//! - `SRP_LOBBY_SESSION_GRACE_SECONDS` (default 60): how long the session
//!   of a link that went silent is kept for its client to connect again
//!   (0: it ends with the link);
//! - `SRP_PACKET_LOG=<dir>`: the transport's raw datagram log (the sign-in's
//!   session id is left out of it);
//! - `SRP_PVB` (default off), `SRP_PVB_MAX_GEAR` (15), `SRP_PVB_MAX_MATCHES`
//!   (5), `SRP_PVB_PAY` (0.5), `SRP_PVB_STALL_S` (60): "Player vs Bots"
//!   (`pvb`, whose module says what each does and which server reads it).
//! - `SRP_ROOMS_MAX` (50), `SRP_ROOMS_EXTRA_MAPS` (none): rooms, the
//!   players' "servers" (`rooms`, whose module says what each does).

mod away;
mod clans;
mod core;
mod dispatch;
mod gear;
mod items;
mod patch_check;
mod play;
mod presets;
pub mod pvb;
mod replay;
mod room_chat;
pub mod rooms;
mod shop;
mod social;

#[cfg(test)]
mod tests;

/// The scripted 0.26g lobby client: what this crate's tests, `servers/srp`'s
/// end-to-end test and the test bot (`srp-bot`, `servers/srp/src/bot.rs`)
/// talk to the lobby with.
pub mod scripted;
/// The name the tests know it by.
pub use scripted as test_client;

use std::io;
use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use session::v026g::GameAccounts;
use session::v026g::balance::Weights;
use session::v026g::replays::Replays;
use session::v026g::rewards::Results;
use session::{LockExt, SessionStore};
use survarium::v026g::lobby::ServerMessage;
use survarium::v026g::maps;
use udp_transport::PacketLog;

pub use patch_check::{check_min_patch_revision, min_patch_revision_from_env};
pub use play::{LiveView, MatchHost, MatchPlayer, MatchSetup, MatchTicket, match_profile};

/// The modes played when `SRP_MATCH_MODES` is unset: all the client has.
const DEFAULT_MODES: [u8; maps::mode::ALL.len()] = maps::mode::ALL;
/// A silent client keeps its link this long: what the store calls a game
/// that is gone (`session::GONE_AFTER`). The client itself gives up after
/// 5 s without a datagram (0x26F0ED).
const DEFAULT_LINK_TIMEOUT: Duration = Duration::from_secs(30);
/// The same for a client in a match. The client ticks its lobby link from
/// its logic thread in every state (transport.md §6.1), so it is silent
/// only while that thread is held up, which loading a level can do for
/// long [LIKELY]; and a link dropped in a match would have to sign in
/// again in the middle of it. [choice] Two minutes.
const DEFAULT_MATCH_LINK_TIMEOUT: Duration = Duration::from_secs(120);
/// How long the session of a link that went silent is kept. The client
/// connects again by itself with the session it has: every 5 s, and gives
/// up (to its login screen) after the fifth failure (0x27B243..0x27B2C7).
/// [choice] A minute covers those tries.
const DEFAULT_SESSION_GRACE: Duration = Duration::from_secs(60);

/// The maintenance rule is in force from this many minutes before a
/// planned maintenance on: the minutes the clients are told (`9F`), at
/// which they turn Play off (`update_play_button_state`, `lobby.md`).
/// From then on no match is made (`rooms`, `pvb`).
pub(super) const MAINTENANCE_WINDOW: u32 = 30;

/// What the lobby is run with.
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    /// `host:port` of the UDP socket.
    pub bind: String,
    /// The lobby's public host, and whether a client on this machine's
    /// network is told the address it reached srp at for the match server
    /// too (`vostok::config::ServerConfig::host_for`).
    pub public_host: String,
    pub local_peers: bool,
    /// A client that sends nothing for this long is gone.
    pub link_timeout: Duration,
    /// The same for a client in a match (never less than `link_timeout`).
    pub match_link_timeout: Duration,
    /// How long the session of a link that went silent is kept for its
    /// client to connect again; for a player in a match, counted from the
    /// match's end.
    pub session_grace: Duration,
    /// How often the rooms, the online counts, the friends' statuses and
    /// the squads are brought up to date.
    pub tick: Duration,
    /// The `maps_dict` ids played: playable ones only (a room's default
    /// pool, what a clan match is drawn from).
    pub maps: Vec<u8>,
    /// The least patch revision of a game let in, 0 for any: an older one
    /// is refused at its sign-in, and its sessions from before in the lobby
    /// (`SRP_MIN_PATCH_REVISION`; `patch_check`).
    pub min_patch_revision: u8,
    /// What a player counts for when the sides of a match are made
    /// (`SRP_BALANCE_*`; `session::v026g::balance`).
    pub balance: Weights,
    /// "Player vs Bots" (`SRP_PVB`, `SRP_PVB_MAX_GEAR`,
    /// `SRP_PVB_MAX_MATCHES`; `pvb`).
    pub pvb: pvb::Config,
    /// Rooms (`SRP_ROOMS_MAX`, `SRP_ROOMS_EXTRA_MAPS`; `rooms`).
    pub rooms: rooms::Config,
}

impl Settings {
    /// The settings the environment asks for.
    pub fn from_env() -> Self {
        let config = &vostok::config::get().lobby_server;
        let var = |name: &str| std::env::var(name).ok();
        let modes = match var("SRP_MATCH_MODES") {
            Some(value) => parse_modes(&value).unwrap_or_else(|| {
                log::error!(
                    "SRP_MATCH_MODES is `{value}`; it must list modes of 2 to 5. Using 2,3,4,5"
                );
                DEFAULT_MODES.to_vec()
            }),
            None => DEFAULT_MODES.to_vec(),
        };
        let mut maps = map_pool(&modes, &[]);
        if let Some(value) = var("SRP_MATCH_MAPS") {
            match parse_maps(&value) {
                Some(ids) if !map_pool(&modes, &ids).is_empty() => maps = map_pool(&modes, &ids),
                _ => log::error!(
                    "SRP_MATCH_MAPS is `{value}`; it must list playable maps_dict ids of the \
                     modes played. Using every map"
                ),
            }
        }
        let link_timeout = match var("SRP_LOBBY_LINK_TIMEOUT_SECONDS") {
            Some(value) => match value.trim().parse::<u16>() {
                Ok(seconds) if seconds > 0 => Duration::from_secs(seconds.into()),
                _ => {
                    log::error!(
                        "SRP_LOBBY_LINK_TIMEOUT_SECONDS is `{value}`; it must be a number of \
                         seconds over 0. Using {}",
                        DEFAULT_LINK_TIMEOUT.as_secs()
                    );
                    DEFAULT_LINK_TIMEOUT
                }
            },
            None => DEFAULT_LINK_TIMEOUT,
        };
        let seconds = |name: &str, default: Duration, least: u16| match var(name) {
            Some(value) => match value.trim().parse::<u16>() {
                Ok(seconds) if seconds >= least => Duration::from_secs(seconds.into()),
                _ => {
                    log::error!(
                        "{name} is `{value}`; it must be a number of seconds, {least} or more. \
                         Using {}",
                        default.as_secs()
                    );
                    default
                }
            },
            None => default,
        };
        let match_link_timeout = seconds(
            "SRP_LOBBY_MATCH_LINK_TIMEOUT_SECONDS",
            DEFAULT_MATCH_LINK_TIMEOUT,
            1,
        )
        .max(link_timeout);
        let session_grace = seconds("SRP_LOBBY_SESSION_GRACE_SECONDS", DEFAULT_SESSION_GRACE, 0);
        // srp checked it before it started anything.
        let min_patch_revision =
            patch_check::min_patch_revision_from_env().unwrap_or_else(|error| panic!("{error}"));
        if min_patch_revision > 0 {
            log::info!("Games older than patch revision {min_patch_revision} are refused");
        }
        let weight = |name: &str, default: f64| match var(name) {
            Some(value) => parse_weight(&value).unwrap_or_else(|| {
                log::error!(
                    "{name} is `{value}`; it must be a number from 0 to 1000. Using {default}"
                );
                default
            }),
            None => default,
        };
        let defaults = Weights::default();
        let balance = Weights {
            player: weight("SRP_BALANCE_PLAYER_KD", defaults.player),
            battery_lead: weight("SRP_BALANCE_BATTERY_LEAD", defaults.battery_lead),
            ticket_lead: weight("SRP_BALANCE_TICKET_LEAD", defaults.ticket_lead),
        };
        log::info!(
            "The sides of a match: each player counts for its K/D at the match's equipment \
             level, and {} K/D for being there",
            balance.player
        );
        log::info!(
            "The balance rating: a match moves it when {} players or more were in it for half \
             of it",
            session::v026g::balance::rated_players()
        );
        log::info!(
            "A match being played: a battery of lead counts for {} of a player, a ticket for {}",
            balance.battery_lead,
            balance.ticket_lead
        );
        Self {
            pvb: pvb::Config::from_env(),
            rooms: rooms::Config::from_env(),
            balance,
            min_patch_revision,
            bind: config.bind_addr(),
            public_host: config.public_host.clone(),
            local_peers: config.local_peers,
            link_timeout,
            match_link_timeout,
            session_grace,
            maps,
            ..Self::local(&config.bind_addr())
        }
    }

    /// The defaults, on `bind`: every playable map, a client on this
    /// machine told the address it reached.
    pub fn local(bind: &str) -> Self {
        Self {
            bind: bind.to_owned(),
            public_host: "127.0.0.1".to_owned(),
            local_peers: true,
            link_timeout: DEFAULT_LINK_TIMEOUT,
            match_link_timeout: DEFAULT_MATCH_LINK_TIMEOUT,
            session_grace: DEFAULT_SESSION_GRACE,
            tick: Duration::from_secs(1),
            maps: map_pool(&DEFAULT_MODES, &[]),
            min_patch_revision: 0,
            balance: Weights::default(),
            pvb: pvb::Config::default(),
            rooms: rooms::Config::default(),
        }
    }
}

/// A `SRP_BALANCE_*` number: 0 to 1000.
fn parse_weight(value: &str) -> Option<f64> {
    value
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|weight| weight.is_finite() && (0.0..=1000.0).contains(weight))
}

/// `SRP_MATCH_MODES`: modes of 2 to 5, at least one.
fn parse_modes(value: &str) -> Option<Vec<u8>> {
    let modes: Option<Vec<u8>> = value
        .split(',')
        .map(|mode| {
            mode.trim()
                .parse::<u8>()
                .ok()
                .filter(|mode| maps::mode::ALL.contains(mode))
        })
        .collect();
    modes.filter(|modes| !modes.is_empty())
}

/// `SRP_MATCH_MAPS`: `maps_dict` ids, at least one.
fn parse_maps(value: &str) -> Option<Vec<u8>> {
    let ids: Option<Vec<u8>> = value
        .split(',')
        .map(|id| id.trim().parse::<u8>().ok())
        .collect();
    ids.filter(|ids| !ids.is_empty())
}

/// The playable ids of `modes`; with `only` not empty, those of them it
/// names.
fn map_pool(modes: &[u8], only: &[u8]) -> Vec<u8> {
    maps::playable()
        .filter(|map| modes.contains(&map.mode))
        .filter(|map| only.is_empty() || only.contains(&map.id))
        .map(|map| map.id)
        .collect()
}

/// A match is over (the match server's result, or its registry no longer
/// has it): its players are back in the lobby. Settle its rewards first
/// (`session::v026g::rewards::Results::settle`, on the `Results` the lobby
/// was given): each player still marked in the match is then sent what it
/// changed and the results screen with its `85 00`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MatchFinished {
    /// The id the players were sent to the match with
    /// ([`MatchTicket::match_id`]).
    pub match_id: u32,
    /// A room's match called off before its countdown (the match server's
    /// `MatchEnd::NotMade`): its room goes on as for a match that could not
    /// be made, and nobody is shown results.
    pub not_made: bool,
}

pub(crate) enum Notice {
    MatchFinished(MatchFinished),
    Push {
        account_id: u32,
        messages: Vec<ServerMessage>,
    },
    AccountChanged {
        account_id: u32,
    },
    /// To every client in the lobby now.
    Broadcast {
        messages: Vec<ServerMessage>,
    },
}

/// What the other servers tell the lobby. The lobby's thread takes the
/// notices on its next pass (within a few milliseconds).
#[derive(Default)]
pub struct Inbox {
    notices: Mutex<Vec<Notice>>,
    stop: AtomicBool,
    /// The operator's text every client shows (`A5`), until it is cleared.
    admin_text: Mutex<Option<(String, String)>>,
}

impl Inbox {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// The match is over: its seats are freed and its players' clients are
    /// told they are in the lobby (`85 00`).
    pub fn match_finished(&self, finished: MatchFinished) {
        self.notices.locked().push(Notice::MatchFinished(finished));
    }

    /// Send messages to the account's client, in order, if it is in the
    /// lobby now (a results screen, a notice). Dropped otherwise.
    pub fn push(&self, account_id: u32, messages: Vec<ServerMessage>) {
        self.notices.locked().push(Notice::Push {
            account_id,
            messages,
        });
    }

    /// The account was changed outside the lobby (a match's rewards, the
    /// web page): its client is sent its inventory, account, money, level
    /// and skills, reputations and quests again.
    pub fn account_changed(&self, account_id: u32) {
        self.notices
            .locked()
            .push(Notice::AccountChanged { account_id });
    }

    /// Send messages to every client in the lobby now, in order. A client
    /// that signs in later does not get them.
    pub fn broadcast(&self, messages: Vec<ServerMessage>) {
        self.notices.locked().push(Notice::Broadcast { messages });
    }

    /// The operator's text (`A5`, two lines the client shows in its lobby,
    /// each cut to the 63 bytes its buffers take): to everyone in the
    /// lobby now and to whoever signs in while it stands. Kept in memory:
    /// a restart clears it.
    pub fn set_admin_text(&self, first: &str, second: &str) {
        *self.admin_text.locked() = Some((first.to_owned(), second.to_owned()));
        self.broadcast(vec![survarium::v026g::lobby::admin_text(first, second)]);
    }

    /// Takes the operator's text away (`A6`).
    pub fn clear_admin_text(&self) {
        *self.admin_text.locked() = None;
        self.broadcast(vec![survarium::v026g::lobby::admin_text_clear()]);
    }

    /// The operator's text that stands, if any.
    pub fn admin_text(&self) -> Option<(String, String)> {
        self.admin_text.locked().clone()
    }

    /// Ends [`Lobby::run`] on its next pass.
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }

    pub(crate) fn stopped(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }

    pub(crate) fn take(&self) -> Vec<Notice> {
        std::mem::take(&mut *self.notices.locked())
    }
}

/// The lobby, bound to its socket.
pub struct Lobby {
    socket: UdpSocket,
    store: Arc<SessionStore>,
    accounts: Arc<GameAccounts>,
    results: Arc<Results>,
    host: Arc<dyn MatchHost>,
    inbox: Arc<Inbox>,
    settings: Settings,
    /// "Watch replay" (`replay`): srp's replays, shared with the match
    /// server and the web page; none, every ask is answered `off`.
    replays: Option<Arc<Replays>>,
}

/// How long the loop waits for a datagram before it looks at its timers.
const WAIT: Duration = Duration::from_millis(2);

impl Lobby {
    pub fn bind(
        settings: Settings,
        store: Arc<SessionStore>,
        accounts: Arc<GameAccounts>,
        results: Arc<Results>,
        host: Arc<dyn MatchHost>,
        inbox: Arc<Inbox>,
    ) -> io::Result<Self> {
        let socket = UdpSocket::bind(&settings.bind)?;
        socket.set_read_timeout(Some(WAIT))?;
        Ok(Self {
            socket,
            store,
            accounts,
            results,
            host,
            inbox,
            settings,
            replays: None,
        })
    }

    /// The replays the History's "Watch replay" asks for (`replay`).
    pub fn with_replays(mut self, replays: Arc<Replays>) -> Self {
        self.replays = Some(replays);
        self
    }

    /// Where the socket is bound (for a port of 0, the one it was given).
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.socket.local_addr()
    }

    /// Serve until [`Inbox::stop`].
    pub fn run(self) -> io::Result<()> {
        let local = self.socket.local_addr()?;
        log::info!(
            "0.26g lobby on UDP {local}; {} maps to play",
            self.settings.maps.len()
        );
        let mut core = core::Core::new(
            &self.store,
            &self.accounts,
            &self.results,
            &*self.host,
            &self.inbox,
            self.settings.clone(),
            Instant::now(),
        );
        core.packet_log = PacketLog::from_env();
        core.replays = self.replays.as_deref();
        let mut buffer = [0_u8; 2048];
        while !self.inbox.stopped() {
            match self.socket.recv_from(&mut buffer) {
                Ok((len, from)) => core.datagram(from, &buffer[..len], Instant::now()),
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock
                            | io::ErrorKind::TimedOut
                            // Windows reports a datagram sent to a port
                            // nobody listens on any more this way.
                            | io::ErrorKind::ConnectionReset
                            | io::ErrorKind::Interrupted
                    ) => {}
                Err(error) => {
                    log::error!("The lobby socket failed to receive: {error}");
                    std::thread::sleep(WAIT);
                }
            }
            for (to, datagram) in core.step(Instant::now()) {
                if let Err(error) = self.socket.send_to(&datagram, to) {
                    log::debug!("A lobby datagram to {to} was not sent: {error}");
                }
            }
        }
        core.shutdown();
        Ok(())
    }
}

/// The 0.26g lobby of srp, on the address the 0.26g login server announces.
/// It returns only when its socket cannot be bound or `inbox` is stopped.
pub fn run(
    store: Arc<SessionStore>,
    accounts: Arc<GameAccounts>,
    results: Arc<Results>,
    host: Arc<dyn MatchHost>,
    inbox: Arc<Inbox>,
) -> io::Result<()> {
    Lobby::bind(Settings::from_env(), store, accounts, results, host, inbox)?.run()
}
