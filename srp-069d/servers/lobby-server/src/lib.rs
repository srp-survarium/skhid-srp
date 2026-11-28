//! lobby-server (0.69d): the UDP lobby, sans IO core plus a thin socket loop.
//!
//! * [`Server`] owns one [`Peer`] (UDP address -> `udp_transport::Transport` + session state) per client.
//!   `receive(addr, datagram, now)` and `poll(now)` are the whole interface; [`net::serve`] puts them on a socket.
//! * Sign-in: the client signs in at the login server (TCP), which hands the account over through the shared
//!   [`SessionTable`] (`grant`); the lobby's 2E carries that session id (`redeem`; the client re-sends it on its own
//!   lobby reconnect, and `SessionTable::persistent` keeps it across a server restart). Every peer then has its own account
//!   and profile from the `accounts` store; nothing is shared between peers but the store, the game tables and the
//!   [`Registry`] (account -> address) that the social relay uses to find other players.
//! * Each module documents the client behaviour it answers. Local matches (Training, tutorial) are a hook:
//!   [`LocalMatches`].
//!
//! Modules: `login` (2E, login pushes, 5B 02 handshake), `build` (87 89 8A 8B 8C 8D 8E), `shop` (46 xx), `equip` (45 00),
//! `skills` (47 00, 47 01), `workshop` (60 xx, 45 05..12), `play` (5B, 53, results), `social` (4C..4E), `dispatch` (request table, fixed replies).

// nested `if let` instead of let chains: builds on older stable toolchains too
#![allow(clippy::collapsible_if)]

pub mod achievements;
pub mod actions;
pub mod awards;
pub mod boosters;
pub mod build;
pub mod busy;
pub mod charge;
pub mod chat;
pub mod chests;
pub mod clans;
pub mod comments;
pub mod config;
pub mod contract;
pub mod details;
pub mod decals;
pub mod dispatch;
pub mod equip;
pub mod events;
pub mod faction_battle;
pub mod hero;
pub mod host_room;
pub mod hunt;
pub mod items;
pub mod league;
pub mod local;
pub mod match_data;
pub mod match_report;
pub mod online;
pub mod login_calendar;
pub mod login;
pub mod net;
pub mod notices;
pub mod peer;
pub mod rooms;
pub mod presets;
pub mod profiles;
pub mod queue;
pub mod play;
pub mod pve;
pub mod pve_end;
pub mod premium;
pub mod progress;
pub mod quests;
pub mod rename;
pub mod server;
pub mod servers;
pub mod sessions;
pub mod shop;
pub mod skills;
pub mod skins;
pub mod stats;
pub mod social;
pub mod squads;
pub mod workshop;

use std::sync::{Arc, Mutex};

use accounts::Store;
use game_data::{GameData, Roll, SplitMix};

pub use config::LobbyConfig;
pub use local::{LocalMatches, MatchCtx, MatchOutcome, MinimalMatches};
pub use online::{BotFill, Ended, EndedPlayer, EndedStat, HostOutcome, HostPlayerStat, OnlineMatches, PveSetup, Seat, Ticket82};
pub use peer::{Cross, Peer, Phase, Registry};
pub use server::Server;
pub use sessions::SessionTable;

/// A random generator seeded from the clock and the OS (the lobby's own generator, and the rolls of code that runs
/// outside a lobby handler: account creation in the login path, a module granted inside a profile update).
pub fn fresh_roll() -> SplitMix {
    let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(1)
        ^ rand::random::<u64>();
    SplitMix(seed)
}

/// What every handler shares: settings, the account store, the game tables, the login handover, the local match hook.
pub struct Lobby {
    pub cfg: LobbyConfig,
    pub store: Arc<Store>,
    pub data: Arc<GameData>,
    pub sessions: Arc<SessionTable>,
    pub matches: Arc<dyn LocalMatches>,
    /// The match server hook of online matches (None = none attached; see online.rs).
    pub online: Option<online::Hook>,
    /// Experience / reputation changes waiting to become notices (notices.rs).
    pub notices: notices::Queue,
    rng: Mutex<SplitMix>,
}

impl Lobby {
    pub fn new(cfg: LobbyConfig, store: Arc<Store>, data: Arc<GameData>, sessions: Arc<SessionTable>, matches: Arc<dyn LocalMatches>) -> Lobby {
        let notices = notices::install(&store);
        Lobby { cfg, store, data, sessions, matches, online: None, notices, rng: Mutex::new(fresh_roll()) }
    }

    /// Attach the match server hook: online matches (queue, custom rooms, squads, PvE missions) start through it.
    pub fn with_online(mut self, hook: online::Hook) -> Lobby {
        self.online = Some(hook);
        self
    }

    /// Fix the random generator (tests).
    pub fn seed(&self, seed: u64) {
        *self.rng.lock().unwrap() = SplitMix(seed);
    }

    /// Run `f` with the server's random generator.
    pub fn roll<T>(&self, f: impl FnOnce(&mut dyn Roll) -> T) -> T {
        let mut g = self.rng.lock().unwrap();
        f(&mut *g)
    }
}
