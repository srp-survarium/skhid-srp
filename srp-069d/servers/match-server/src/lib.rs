//! match-server (0.69d): online match relay. See README.md of this crate (open risks) and docs/notes/online.md.
//!
//! The server has no game simulation of its own. It relays inputs: per player the match handshake (C9 / DC / CA / DD /
//! CB), the start world as the first DE, then one DF per tick with the inputs of all players. With a world host
//! attached ([`host`]) the start world, E0 hashes, resync DEs, results and bots come from the host.
//!
//! * [`table`]: the match connection's message table (S->C DC..E5, C->S C9..D4).
//! * [`mtransport`]: the reliable-UDP transport with that table (udp-transport's `Transport` only knows the lobby table).
//! * [`match_msgs`]: payload codecs of the match messages (`crates/protocol` has the lobby ones only).
//! * [`world`]: cached worlds (file format `.srpde`) and the collision checksum.
//! * [`relay`]: one match (join, DE, DF loop, leave, end), sans IO.
//! * [`host`]: the link of a match to the world host daemon (start world, PvE end, winner, counters, E0, resync and
//!   bots from the exe's own world code), socket IO on its own threads; [`host::query_projects`] asks the daemon which
//!   level projects it serves, [`host::query_status`] for its room (loaded maps, matches, whether a new map loads).
//! * [`hub`]: the lobby-facing API (create match -> tickets, end callback) and the datagram router, sans IO.
//! * [`server`]: UDP socket driver thread around the hub.

pub mod clock;
pub mod host;
pub mod hub;
pub mod match_msgs;
pub mod mtransport;
pub mod relay;
pub mod server;
pub mod table;
pub mod world;

pub use hub::{Hub, HubConfig, MatchTicket, PlayerTicket};
pub use relay::{
    options_header, ConfMode, CreateError, EndReason, LeaveReason, MatchConfig, MatchEnd, MatchSpec, MatchStats, PlayerResult, PlayerSpec,
    FinalStats, PlayerStat, HostOutcome, OutcomePlayer, BotsStats, BOT_SPAWN_PROFILE, pvp_winner, mode_winner, GUNRACE_MODE, GUNRACE_NO_RESULT,
};
pub use host::{query_projects, query_status, Connector, HostConfig, HostStream, LinkState};
pub use server::MatchServer;
pub use relay::{EndRule, PveEndRule};
pub use world::{RosterObjects, StartObject, World, WorldStore};
