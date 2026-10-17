//! The 0.26g lobby (`resources/notes/v026g/lobby.md`, `lobby_requests.md`,
//! `social.md`; `PLAN-026G.md` steps 3 and 8), beside the 0.21d one.
//!
//! What 0.26g changed: the lobby is one UDP link on the match transport
//! (`udp-transport`, `Table::Lobby`), whose connect payload is the sign-in
//! `2E`, and the chat, friends and squads travel on it too. So this is one
//! socket, one thread and one [`Transport`](udp_transport::Transport) per
//! client address:
//!
//! - `core`: the links: sign-in, the session's hold, timeouts, sign-out;
//! - `dispatch`: every client request and its answer;
//! - `play`: Play, the queue, making a match and coming back from it;
//! - `join`: the JOIN button of the "matches now" window of srp's changed
//!   lobby movie: a held place in a match being played, and the Play that
//!   takes it;
//! - `pvb`: "Player vs Bots": one player against the match server's own
//!   bots, chosen in srp's changed lobby movie;
//! - `replay`: the "Watch replay" of the movie's History: the ask, the
//!   polls, the download link (`SRP_REPLAY_DIR`, set where srp starts the
//!   lobby: [`Lobby::with_replays`]);
//! - `social`: chat, friends, important messages, squads;
//! - `clans`: clans, clan chat and clan-team Play.
//!
//! The account's rules are `session::v026g::rules`; the messages are
//! `survarium::v026g::lobby`. The queue is the store's
//! ([`session::Matchmaking`]: a match needs two players, the gather window
//! starts with the second), the squads are the store's too.
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
//! - `SRP_MATCH_GATHER_SECONDS`, `SRP_MATCH_FILL_PLAYERS`,
//!   `SRP_MATCH_QUIET_SECONDS`: the queue's, as for 0.21d (read where the
//!   store is made); and the 0.26g queue's own: `SRP_MATCH_MIN_PLAYERS`
//!   (default 4), `SRP_MATCH_JOIN_EXTENSION_SECONDS` (30),
//!   `SRP_MATCH_MAX_COUNTDOWN_SECONDS` (120);
//! - `SRP_MATCH_TEAM_SIZE` (default 10, 1 to 10): the most players of a
//!   side; a match has two sides;
//! - `SRP_BALANCE_PLAYER` (default 12), `SRP_BALANCE_PLAYER_LEVEL` (1.5),
//!   `SRP_BALANCE_RATING` (4), `SRP_BALANCE_TRUST_MATCHES` (5): what a
//!   player, a level of its account and a point of its balance rating
//!   count for when the sides are made, in gear levels, and the matches
//!   after which the rating counts for half (`session::v026g::balance`);
//! - `SRP_BALANCE_RATED_PLAYERS` (default 4): the fewest players a match
//!   must have had in it to move the balance rating (read where a match
//!   is settled: `session::v026g::balance::rated_players`);
//! - `SRP_BALANCE_BATTERY_LEAD` (default 0.55), `SRP_BALANCE_TICKET_LEAD`
//!   (0.2), `SRP_BALANCE_DRAMATIC_LEAD` (2.5): in a match being played,
//!   what a battery and a ticket of lead count for, in players, and the
//!   lead from which the side that trails may get two players more than
//!   the other when players join late; `SRP_LATE_JOIN_PATIENCE_SECONDS`
//!   (60): how long a player who would make an even match uneven waits
//!   for another before it goes in (`play.rs`);
//! - `SRP_SQUADS` (default on), `SRP_SQUADS_OFF_TEXT`: with `off` nobody
//!   is invited to a squad, and the one who tries is told so in a message
//!   box (`social.rs`); `SRP_SQUAD_SIZE` (default 3, 2 or 3),
//!   `SRP_SQUAD_FULL_TEXT`: the most players of a squad, and what the one
//!   who invites somebody more is told;
//! - `SRP_MIN_PATCH_REVISION` (default none; not a number from 0 to 255,
//!   or one under the newest patch of the game data, stops srp
//!   (`check_min_patch_revision`)): a game older than that patch revision
//!   is refused at its sign-in, and its sessions from before in the lobby
//!   (`patch_check`);
//! - `SRP_QUEUE_CHAT` (default on), `SRP_QUEUE_CHAT_NAME` (default none):
//!   lines in the general chat when somebody joins or leaves the search and
//!   when a match starts, with whom (`queue_chat.rs`);
//! - `SRP_MATCH_SEARCH_STEP_SECONDS (default 4, 1 to 3600): a waiting
//!   player's range of levels grows by one at each end every so many
//!   seconds (the client shows the range from the same number);
//! - `SRP_MATCH_MODES`: the game modes to play, comma-separated (default
//!   `2,3,4`), as for 0.21d with 0.26g's modes;
//! - `SRP_MATCH_MAPS`: the `maps_dict` ids to play, comma-separated
//!   (default: every playable one of the modes);
//! - `SRP_MATCH_MAP_SKEW` (default 1, a decimal from 0 to 4): how much more
//!   likely a match gets a map its players played less: a map is drawn
//!   with weight `1 + skew * min(behind, 5) / 5`, `behind` being how many
//!   matches per player fewer they played it than the map of the pool they
//!   played most (0: every map alike; `session::v026g::matchmaking`);
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
//! - `SRP_SERVER_BOTS` (default 0, up to 19; **testing only**): this many of
//!   the match server's own bots are put into each match made, after its
//!   players, on the side with fewer places (`play::server_bot`). They have
//!   no account and no client; the match server plays them (stage 1 of the
//!   player bots: a fixed script, `docs/notes/player_bots.md` §6). A late
//!   joiner whose side needs the room takes the place of one of them,
//!   which leaves the match (the match server's `SRP_BOTS_GIVE_WAY`;
//!   `policy::bot_gives_way`, `play::late_rule`);
//! - `SRP_PVB` (default off), `SRP_PVB_MAX_GEAR` (5), `SRP_PVB_MAX_MATCHES`
//!   (5), `SRP_PVB_PAY` (0.5), `SRP_PVB_STALL_S` (60): "Player vs Bots"
//!   (`pvb`, whose module says what each does and which server reads it).

mod away;
mod clans;
mod core;
mod dispatch;
mod items;
mod join;
mod patch_check;
mod play;
pub mod pvb;
mod queue_chat;
mod replay;
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
use std::num::NonZeroU16;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use session::matchmaking_policy::DEFAULT_LATE_JOIN_PATIENCE;
use session::v026g::GameAccounts;
use session::v026g::balance::Weights;
use session::v026g::matchmaking::{DEFAULT_MAP_SKEW, MAX_MAP_SKEW};
use session::v026g::replays::Replays;
use session::v026g::rewards::Results;
use session::{LockExt, MAX_SQUAD_MEMBERS, MIN_SQUAD_SIZE, SessionStore};
use survarium::v026g::lobby::ServerMessage;
use survarium::v026g::maps;
use udp_transport::PacketLog;

pub use patch_check::{check_min_patch_revision, min_patch_revision_from_env};
pub use play::{
    LiveView, MatchHost, MatchPlayer, MatchSetup, MatchTicket, OpenMatch, match_profile,
};
/// `SRP_MATCH_TEAM_SIZE` when unset: "two teams of up to 8 players" (the
/// wiki of February 2015; `progression_sources.md` §9).
const DEFAULT_TEAM_SIZE: usize = 10;
/// The most a side can be: the client's roster has 20 places.
const MAX_TEAM_SIZE: usize = session::MAX_PLAYERS_PER_MATCH / 2;
/// `SRP_MATCH_SEARCH_STEP_SECONDS` when unset. [choice] The step is not in
/// the client's data. The levels are 0..=15 and a player starts with its
/// own and the next, so at 4 s every level is in range after 56 s: within
/// the default gather window of a minute, and a server of few players
/// starts its matches when it did before levels counted.
const DEFAULT_SEARCH_STEP: u16 = 4;
/// The modes played when `SRP_MATCH_MODES` is unset: all the client has.
const DEFAULT_MODES: [u8; 3] = maps::mode::ALL;
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
    /// How often a waiting player's queue screen, the online counts, the
    /// friends' statuses and the squads are brought up to date.
    pub tick: Duration,
    /// The `maps_dict` ids a match is drawn from: playable ones only.
    pub maps: Vec<u8>,
    /// How much more likely a map its players played less is
    /// (`SRP_MATCH_MAP_SKEW`, 0 to 4; `session::v026g::matchmaking::map_weight`).
    pub map_skew: f64,
    /// 0x85's search step, in seconds: the client shows its level widened
    /// by `wait / step` at each end as its search range, and divides by it
    /// (never 0). The queue widens a waiting player's range the same way.
    pub search_step: NonZeroU16,
    /// The most players of a side of a match.
    pub team_size: usize,
    /// Lines in the general chat about the search: who joined it, who left
    /// it, and with whom a match starts (`SRP_QUEUE_CHAT`, default on).
    pub queue_chat: bool,
    /// The sender name of those lines (`SRP_QUEUE_CHAT_NAME`, default
    /// none: the client then shows the line with no name).
    pub queue_chat_name: String,
    /// The least patch revision of a game let in, 0 for any: an older one
    /// is refused at its sign-in, and its sessions from before in the lobby
    /// (`SRP_MIN_PATCH_REVISION`; `patch_check`).
    pub min_patch_revision: u8,
    /// What a player counts for when the sides of a match are made
    /// (`SRP_BALANCE_*`; `session::v026g::balance`).
    pub balance: Weights,
    /// How long a player waits for a match being played that it would
    /// make less even, before it goes in all the same
    /// (`SRP_LATE_JOIN_PATIENCE_SECONDS`).
    pub late_join_patience: Duration,
    /// Whether players may form squads (`SRP_SQUADS`, default on).
    pub squads: bool,
    /// What the message box of a refused squad invite says: a text key of
    /// the game, or a sentence (`SRP_SQUADS_OFF_TEXT`).
    pub squads_off_text: String,
    /// The most players of a squad, 2 or 3 (`SRP_SQUAD_SIZE`, default 3:
    /// the client's own limit). Given to the store's squads when the lobby
    /// starts (`Core::new`).
    pub squad_size: usize,
    /// What the message box of an invite into a full squad says: a text
    /// key of the game, or a sentence (`SRP_SQUAD_FULL_TEXT`). None: the
    /// sentence of [`squad_full_text_for`], from the size.
    pub squad_full_text: Option<String>,
    /// The match server's own bots put into each match made
    /// (`SRP_SERVER_BOTS`, default 0; testing only).
    pub server_bots: u8,
    /// "Player vs Bots" (`SRP_PVB`, `SRP_PVB_MAX_GEAR`,
    /// `SRP_PVB_MAX_MATCHES`; `pvb`).
    pub pvb: pvb::Config,
}

impl Settings {
    /// The settings the environment asks for.
    pub fn from_env() -> Self {
        let config = &vostok::config::get().lobby_server;
        let var = |name: &str| std::env::var(name).ok();
        let modes = match var("SRP_MATCH_MODES") {
            Some(value) => parse_modes(&value).unwrap_or_else(|| {
                log::error!(
                    "SRP_MATCH_MODES is `{value}`; it must list modes of 2, 3 and 4. Using 2,3,4"
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
        let map_skew = match var("SRP_MATCH_MAP_SKEW") {
            Some(value) => parse_map_skew(&value).unwrap_or_else(|| {
                log::error!(
                    "SRP_MATCH_MAP_SKEW is `{value}`; it must be a number from 0 to \
                     {MAX_MAP_SKEW}. Using {DEFAULT_MAP_SKEW}"
                );
                DEFAULT_MAP_SKEW
            }),
            None => DEFAULT_MAP_SKEW,
        };
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
        let team_size = match var("SRP_MATCH_TEAM_SIZE") {
            Some(value) => parse_team_size(&value).unwrap_or_else(|| {
                log::error!(
                    "SRP_MATCH_TEAM_SIZE is `{value}`; it must be a number from 1 to \
                     {MAX_TEAM_SIZE}. Using {DEFAULT_TEAM_SIZE}"
                );
                DEFAULT_TEAM_SIZE
            }),
            None => DEFAULT_TEAM_SIZE,
        };
        let default_step = NonZeroU16::new(DEFAULT_SEARCH_STEP).expect("the default is not 0");
        let search_step = match var("SRP_MATCH_SEARCH_STEP_SECONDS") {
            Some(value) => parse_search_step(&value).unwrap_or_else(|| {
                log::error!(
                    "SRP_MATCH_SEARCH_STEP_SECONDS is `{value}`; it must be a number of \
                     seconds from 1 to 3600. Using {DEFAULT_SEARCH_STEP}"
                );
                default_step
            }),
            None => default_step,
        };
        let queue_chat = match var("SRP_QUEUE_CHAT") {
            Some(value) => match value.trim().to_ascii_lowercase().as_str() {
                "on" | "1" | "true" | "yes" => true,
                "off" | "0" | "false" | "no" => false,
                _ => {
                    log::error!("SRP_QUEUE_CHAT is `{value}`; it must be on or off. Using on");
                    true
                }
            },
            None => true,
        };
        let queue_chat_name = var("SRP_QUEUE_CHAT_NAME")
            .map(|name| name.trim().chars().take(20).collect())
            .unwrap_or_default();
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
            player: weight("SRP_BALANCE_PLAYER", defaults.player),
            player_level: weight("SRP_BALANCE_PLAYER_LEVEL", defaults.player_level),
            rating: weight("SRP_BALANCE_RATING", defaults.rating),
            trust_matches: weight("SRP_BALANCE_TRUST_MATCHES", defaults.trust_matches),
            battery_lead: weight("SRP_BALANCE_BATTERY_LEAD", defaults.battery_lead),
            ticket_lead: weight("SRP_BALANCE_TICKET_LEAD", defaults.ticket_lead),
            dramatic_lead: weight("SRP_BALANCE_DRAMATIC_LEAD", defaults.dramatic_lead),
        };
        let late_join_patience = seconds(
            "SRP_LATE_JOIN_PATIENCE_SECONDS",
            DEFAULT_LATE_JOIN_PATIENCE,
            0,
        );
        log::info!(
            "The sides of a match: a player counts for {} gear levels, a level of its account \
             for {}, a point of its balance rating for {} (for half after {} matches)",
            balance.player,
            balance.player_level,
            balance.rating,
            balance.trust_matches
        );
        log::info!(
            "The balance rating: a match moves it when {} players or more were in it for half \
             of it",
            session::v026g::balance::rated_players()
        );
        log::info!(
            "A match being played: a battery of lead counts for {} of a player, a ticket for \
             {}; from a lead of {} players the side that trails may get two players more; a \
             player who would make it less even waits {} s",
            balance.battery_lead,
            balance.ticket_lead,
            balance.dramatic_lead,
            late_join_patience.as_secs()
        );
        let squads = match var("SRP_SQUADS") {
            Some(value) => match value.trim().to_ascii_lowercase().as_str() {
                "on" | "1" | "true" | "yes" => true,
                "off" | "0" | "false" | "no" => false,
                _ => {
                    log::error!("SRP_SQUADS is `{value}`; it must be on or off. Using on");
                    true
                }
            },
            None => true,
        };
        // A text the client reads into 256 bytes (`lobby::limits::TEXT`).
        let squads_off_text = var("SRP_SQUADS_OFF_TEXT")
            .map(|text| text.trim().to_owned())
            .filter(|text| !text.is_empty() && text.len() <= 255)
            .unwrap_or_else(|| DEFAULT_SQUADS_OFF_TEXT.to_owned());
        if !squads {
            log::info!("Squads are off: an invite is refused with `{squads_off_text}`");
        }
        let squad_size = match var("SRP_SQUAD_SIZE") {
            Some(value) => parse_squad_size(&value).unwrap_or_else(|| {
                log::error!(
                    "SRP_SQUAD_SIZE is `{value}`; it must be {MIN_SQUAD_SIZE} or \
                     {MAX_SQUAD_MEMBERS}. Using {MAX_SQUAD_MEMBERS}"
                );
                MAX_SQUAD_MEMBERS
            }),
            None => MAX_SQUAD_MEMBERS,
        };
        let squad_full_text = var("SRP_SQUAD_FULL_TEXT")
            .map(|text| text.trim().to_owned())
            .filter(|text| !text.is_empty() && text.len() <= 255);
        if squads {
            log::info!(
                "A squad has at most {squad_size} players: an invite into a full one is refused \
                 with `{}`",
                squad_full_text
                    .clone()
                    .unwrap_or_else(|| squad_full_text_for(squad_size))
            );
        }
        let most_bots = (session::MAX_PLAYERS_PER_MATCH - 1) as u8;
        let server_bots = match var("SRP_SERVER_BOTS") {
            Some(value) => match value.trim().parse::<u8>() {
                Ok(count) if count <= most_bots => count,
                _ => {
                    log::error!(
                        "SRP_SERVER_BOTS is `{value}`; it must be a number from 0 to                          {most_bots}. Using 0"
                    );
                    0
                }
            },
            None => 0,
        };
        if server_bots > 0 {
            log::warn!(
                "SRP_SERVER_BOTS: {server_bots} of the match server's own bots go into each match                  made (testing only)"
            );
        }
        Self {
            server_bots,
            pvb: pvb::Config::from_env(),
            balance,
            late_join_patience,
            squads,
            squads_off_text,
            squad_size,
            squad_full_text,
            queue_chat,
            queue_chat_name,
            min_patch_revision,
            bind: config.bind_addr(),
            public_host: config.public_host.clone(),
            local_peers: config.local_peers,
            link_timeout,
            match_link_timeout,
            session_grace,
            maps,
            map_skew,
            team_size,
            search_step,
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
            map_skew: DEFAULT_MAP_SKEW,
            search_step: NonZeroU16::new(DEFAULT_SEARCH_STEP).expect("the default is not 0"),
            team_size: DEFAULT_TEAM_SIZE,
            queue_chat: true,
            queue_chat_name: String::new(),
            min_patch_revision: 0,
            balance: Weights::default(),
            late_join_patience: DEFAULT_LATE_JOIN_PATIENCE,
            squads: true,
            squads_off_text: DEFAULT_SQUADS_OFF_TEXT.to_owned(),
            squad_size: MAX_SQUAD_MEMBERS,
            squad_full_text: None,
            server_bots: 0,
            pvb: pvb::Config::default(),
        }
    }
}

/// What a refused squad invite says (`SRP_SQUADS_OFF_TEXT`). The game has
/// no text of its own for it (`social.md` §9.2), and its client shows a
/// text that is no key of its as it is (`lobby_requests.md` §4.2).
pub const DEFAULT_SQUADS_OFF_TEXT: &str = "Squads are switched off on this server.";

/// What an invite into a full squad says when `SRP_SQUAD_FULL_TEXT` is not
/// set. The game has no text for it either (`social.md` §9.2).
pub fn squad_full_text_for(size: usize) -> String {
    format!("Squads are limited to {size} players.")
}

/// `SRP_SQUAD_SIZE`: 2 or 3 (the client keeps three members).
fn parse_squad_size(value: &str) -> Option<usize> {
    value
        .trim()
        .parse::<usize>()
        .ok()
        .filter(|size| (MIN_SQUAD_SIZE..=MAX_SQUAD_MEMBERS).contains(size))
}

/// A `SRP_BALANCE_*` number: 0 to 1000.
fn parse_weight(value: &str) -> Option<f64> {
    value
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|weight| weight.is_finite() && (0.0..=1000.0).contains(weight))
}

/// `SRP_MATCH_MODES`: modes of 2, 3 and 4, at least one.
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

/// `SRP_MATCH_TEAM_SIZE`: 1 to [`MAX_TEAM_SIZE`].
fn parse_team_size(value: &str) -> Option<usize> {
    value
        .trim()
        .parse()
        .ok()
        .filter(|size| (1..=MAX_TEAM_SIZE).contains(size))
}

/// `SRP_MATCH_SEARCH_STEP_SECONDS`: 1 to 3600.
fn parse_search_step(value: &str) -> Option<NonZeroU16> {
    value
        .trim()
        .parse()
        .ok()
        .filter(|step: &NonZeroU16| step.get() <= 3600)
}

/// `SRP_MATCH_MAP_SKEW`: 0 to [`MAX_MAP_SKEW`].
fn parse_map_skew(value: &str) -> Option<f64> {
    value
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|skew| skew.is_finite() && (0.0..=MAX_MAP_SKEW).contains(skew))
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
