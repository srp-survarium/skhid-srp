//! The 0.26g match server (`docs/notes/match.md`):
//! the sockets and the loop around
//! `survarium::v026g::match_msgs`, over the `udp-transport` crate.
//!
//! As for 0.21d, the clients simulate and the server does not: it takes
//! each client through the handshake, keeps the match clock, relays the
//! inputs, compares the clients' world hashes, gets world snapshots from
//! patched clients and starts or resyncs clients from them, and ends the
//! match. The 0.21d server (`crate::game`, `crate::server`) is the model;
//! `game.rs` here says what was carried over and what was not.
//!
//! - [`MatchRegistry`]: the lobby makes a match ([`MatchSetup`]) and gets
//!   the address to send its players to ([`MatchTicket`]);
//! - [`run`]: the match server, on the configured UDP port;
//! - [`ResultsSink`]: where each finished match's [`MatchResult`] goes.
//!
//! ```no_run
//! use std::sync::Arc;
//! use match_server::v026g::{self, MatchRegistry, MatchResult};
//!
//! let store = Arc::new(session::SessionStore::new());
//! let registry = Arc::new(MatchRegistry::from_config());
//! let (results, finished) = std::sync::mpsc::channel::<MatchResult>();
//! let server = {
//!     let (registry, store) = (registry.clone(), store.clone());
//!     std::thread::spawn(move || v026g::run(registry, store, results))
//! };
//! // The lobby: `registry.create(setup)` and the ticket to each player;
//! // `finished.recv()` for the rewards.
//! # let _ = (server, finished);
//! ```
//!
//! Settings (environment, read once at start):
//! - `SRP_BIND_HOST`, `SRP_PUBLIC_HOST`, `SRP_MATCH_PORT` (default 1236):
//!   the address, as for 0.21d (`vostok::config`);
//! - `SRP_SNAPSHOT_DIR`: where uploaded worlds are saved (default
//!   `logs/snapshots`; `off` or empty for none), as for 0.21d;
//! - `SRP_PACKET_LOG=<dir>`: the transport's raw datagram log;
//! - `SRP_LATE_INPUTS=discard|move` (default `discard`, what 0.21d does):
//!   what becomes of an input that comes after its tick is final: dropped
//!   (0xF1 / 0xF2), or moved to an open tick (0xEF / 0xF0, new in 0.26g);
//! - `SRP_RESYNC_WORLD_ONLY=on|off` (default `off`): resync a playing
//!   client with the world alone (0xE8 of the next generation, its clock
//!   left as it is) instead of the time and the world (0xE5, 0xE8);
//! - `SRP_RESTORE_LIMIT=<n>` (default 0, no limit): disconnect a client
//!   that is out of sync again after `n` resyncs in a row with no report
//!   of its that agreed in between;
//! - `SRP_RESYNC_AFTER_AGREED_SECONDS` (default 5, 3 to 15; 15 = the old
//!   rule): the least time between two resyncs of a client when its last
//!   one was agreed with; after one that was not, it stays 15 s, and a
//!   client resynced 4 times within a minute waits 15 s too;
//! - `SRP_MATCH_CLIENT_TIMEOUT_SECONDS` (default 10): how long a playing
//!   client may send nothing before it is taken as gone. A game window
//!   out of focus can stall for long (another server saw 58 s under
//!   Wine): raise it for tests with two clients on one PC. A client that
//!   loads its level has 60 s, or this if it is longer;
//! - test bots: no setting here. The lobby marks a match's places
//!   (`MatchPlayer::bot`) from `SRP_BOT_ACCOUNTS` (testing only;
//!   `session::v026g::bots`), and `game.rs` ("Bots") says what a bot's
//!   place is and is not;
//! - `SRP_CLOCK_CORRECTION=on|off` (default `on`): whether client clocks
//!   are brought to the match clock with 0xE7 (off: a client keeps the
//!   lag it was started with, and its inputs come that much later);
//! - `SRP_CHECK_INITIAL_WORLD=on|off` (default `on`): whether the world
//!   that starts a match is refused when it already has kills or other
//!   statistics in it;
//! - `SRP_RECORD_DIR=<dir>` (default off): every datagram of a match in
//!   and out, one file per match, kept when `SRP_RECORD_MIN_PLAYERS`
//!   (default 6 for PvP; one human for PvE, late joiners counted) have entered it; kept files over
//!   `SRP_RECORD_MAX_MB` (default 5000) together go oldest first
//!   ([`record`]);
//! - `SRP_REPLAY_DIR=<dir>`, `SRP_GAME_MODES_DIR=<dir>` (default off; both
//!   needed, with `SRP_RECORD_DIR`): a kept recording is made into the
//!   game's replay file there when a player first asks for it in the
//!   lobby's "Watch replay", and a replay goes with its recording
//!   ([`replay`]). The replay folder must be a folder of its own (not
//!   `SRP_RECORD_DIR`, inside it or around it: then replays are off).
//!   `SRP_REPLAY_MAX_RECORDING_MB` (default 24): a larger recording is not
//!   converted (memory: `docs/notes/replay_watch.md`); `SRP_REPLAY_MAX_MB`
//!   (default 1000): replays over it together go, the oldest first.
//!   Read by srp, which hands the replays to [`run_on_with_replays`], the
//!   lobby and the web page;
//! - `SRP_RELAY_PACK_MS` (default 15, at most 50; 0 = off): how long a
//!   relay or a drop order may wait for others to share its datagram
//!   (`udp_transport::Config::gather`, transport.md section 11.1);
//! - `SRP_MAPDATA_DIR=<dir>` (default off): the `<project>.mapdata` files
//!   (`mapdata-dump`) the server's own bots path on ([`bots::maps`]); a
//!   map is built on a thread of its own the first time a match asks for
//!   it, and `SRP_MAPDATA_KEEP` (default 1) built maps are kept besides
//!   those running matches hold; `SRP_MAPDATA_AT_ONCE` (default 1) maps
//!   at most are in memory, and a match on another map waits for room
//!   with its bots on their script. Without it, or without the map's
//!   file, bots keep their script. `SRP_MAPDATA_BUILT=<dir>` (default
//!   `<SRP_MAPDATA_DIR>/built`; `off`: build at each load): where the
//!   built grids, sight trees and tactics are kept and read back
//!   (`mapdata::built`). With `SRP_PVB=on` ("Player vs Bots", the lobby's
//!   `pvb`) at least `SRP_PVB_MAX_MATCHES` (default 5) maps may be in
//!   memory at once, one for each match against bots that may run;
//! - `SRP_PVB_STALL_S` (default 60, 10 to 3600): a match against the
//!   server's bots whose only player has been asked for its world this
//!   long and uploaded none is over (as deserted): its bots see the world
//!   only through that player's uploads;
//! - `SRP_ANTI_AFK` (default 180, at least 60; `off`): a player who gives
//!   no input for that many seconds of play is removed from the match, as
//!   one who leaves; never in a private room's match nor one against
//!   bots ([`afk`], `docs/notes/anti-afk.md`).

/// Anti-AFK: a player without input for three minutes of play is removed.
mod afk;
/// A player's battery events pay at most N times per kind, N the map's
/// batteries (the owner's rule against farming them).
mod battery_cap;
/// The server's own bots (`MatchPlayer::driven`): their maps.
pub mod bots;
/// The input script of a bot: the test bot's (`srp-bot --drive`) and the
/// server's own bot places' (`MatchPlayer::driven`).
pub mod drive;
mod game;
/// A player's kills of one enemy pay at most 10 times a match.
mod kill_cap;
/// Match recordings (`SRP_RECORD_DIR`) and their reader.
pub mod record;
mod registry;
/// "Watch replay": kept recordings made into replay files (`SRP_REPLAY_DIR`).
pub mod replay;
/// A player's Research units pay at most 200 score a match.
mod research_cap;
mod reward_caps;
mod server;
pub mod sur;
pub mod sur_convert;

#[cfg(test)]
mod tests;

/// The scripted 0.26g match client: what this crate's tests, `servers/srp`'s
/// end-to-end test and the test bot (`srp-bot`, `servers/srp/src/bot.rs`)
/// talk to the match server with.
pub mod scripted;
/// The name the tests know it by.
pub use scripted as test_client;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use udp_transport::{Config, Table};

pub use registry::{
    CreateError, DRAW, IdleRemoved, JoinError, LiveMatch, LivePhase, LiveStanding, MatchEnd,
    MatchPlayer, MatchRegistry, MatchResult, MatchSetup, MatchTicket, PlayerResult, ResultsSink,
    Seat, static_id,
};
pub use replay::{replays_from_env, replays_of};
pub use server::{run, run_on, run_on_with_replays};
pub use survarium::v026g::match_msgs::static_info::MatchProfile;

/// What becomes of an input that comes after its tick is final for the
/// others (`SRP_LATE_INPUTS`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LateInputs {
    /// The owner is told to drop it (0xF1 / 0xF2), as 0.21d's 0x58. The
    /// player's keys then stay as its input before had them until its
    /// next input.
    Discard,
    /// It is moved to the first tick still open: the owner gets 0xEF /
    /// 0xF0, the others the input at the new tick, and all have the same
    /// list (PROVEN from the client's handler: `game.rs::place`). Dropped
    /// all the same where a move could not be told apart or would bring
    /// old keys back. LIKELY what the original server did (the messages
    /// exist for nothing else); not tried live, and not the default: the
    /// owner keeps 0.21d's discard, which is live-proven there.
    Move,
    /// The default (since 2026-10-15): relayed at its own tick, as one in
    /// time, while every other game can still take it into its history
    /// (its tick 500 ms back, `RELAY_USEFUL_MS`) by its way there and back
    /// (`game.rs::late_relay_reach`); else discarded as with `Discard`.
    /// The owner then needs no answer: it keeps the input, as everyone
    /// does. Discarding it sent the owner a drop order that came past its
    /// own history on the very lines that made inputs late, and the owner
    /// kept the input nobody else had (live, 2026-10-15: 58 of 215
    /// desyncs had such a drop just before).
    Relay,
}

/// The waits a test may want shorter. The defaults are the 0.21d server's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Timing {
    /// A playing client that sends nothing for this long is gone.
    pub client_timeout: Duration,
    /// A client loading its level sends and acks nothing; it gets longer.
    pub loading_timeout: Duration,
    /// A made match no client has reached in this long is given up.
    pub unstarted: Duration,
    /// How often a running match asks for a fresh snapshot.
    pub refresh: Duration,
    /// Hash reports are judged once this old (match ms).
    pub hash_settle_ms: u32,
    /// A client is resynced once its hashes were wrong for this long
    /// (match ms, over its own reports) ...
    pub divergence_confirm_ms: u32,
    /// ... or for this long on the wall clock.
    pub divergence_confirm_max: Duration,
    /// The least time between two resyncs of one client.
    pub resync_interval: Duration,
    /// ... once its last resync has been agreed with
    /// (`SRP_RESYNC_AFTER_AGREED_SECONDS`): that world took, and a new
    /// disagreement is a new desync, not a loop. Never more than
    /// `resync_interval`, and back to it for a client resynced
    /// `game.rs::RESYNCS_PER_MINUTE` times within a minute.
    pub resync_after_agreed: Duration,
    /// A mode rule that has its end reports it this long after (match ms).
    pub end_delay_ms: u32,
    /// After the end the clients get this long to hear it.
    pub linger: Duration,
    /// At the time limit, how long a last agreed snapshot is waited for.
    pub final_wait: Duration,
    /// How long the world sent to a game that has none yet is kept on its
    /// way before the sync starts over (`SRP_ENTRY_STATE_SECONDS`). A game
    /// that plays is resynced within the 1.75 s of the client's own table.
    pub entry_state: Duration,
    /// A match against the server's bots ([`MatchSetup::vs_bots`]) whose
    /// only player has been asked for its world this long and sent none
    /// is over (`SRP_PVB_STALL_S`): its bots see the world through that
    /// player's uploads alone.
    pub pvb_stall: Duration,
}

impl Default for Timing {
    fn default() -> Self {
        Self {
            client_timeout: Duration::from_secs(10),
            loading_timeout: Duration::from_secs(60),
            unstarted: Duration::from_secs(120),
            refresh: Duration::from_secs(5),
            hash_settle_ms: 2_500,
            divergence_confirm_ms: 1_000,
            divergence_confirm_max: Duration::from_secs(3),
            resync_interval: Duration::from_secs(15),
            resync_after_agreed: Duration::from_secs(5),
            end_delay_ms: survarium::v026g::match_msgs::rules::END_DELAY_MS,
            linger: Duration::from_secs(5),
            final_wait: Duration::from_secs(7),
            entry_state: Duration::from_secs(4),
            pvb_stall: Duration::from_secs(DEFAULT_PVB_STALL_S),
        }
    }
}

/// `SRP_PVB_STALL_S` when unset.
pub(crate) const DEFAULT_PVB_STALL_S: u64 = 60;

/// How the match server behaves.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Settings {
    pub late_inputs: LateInputs,
    /// Resync a playing client with 0xE8 alone.
    pub resync_world_only: bool,
    /// Resyncs in a row without agreement after which a client is
    /// disconnected; 0 for no limit.
    pub restore_limit: u32,
    pub clock_correction: bool,
    pub check_initial_world: bool,
    /// The world goes to the clients as LZMA2 where that is smaller
    /// (`SRP_COMPRESS_STATE`).
    pub compress_state: bool,
    pub snapshot_dir: Option<PathBuf>,
    /// `SRP_RECORD_DIR`: the match recordings, if any.
    pub record: Option<record::RecordSettings>,
    /// `SRP_RELAY_PACK_MS`: the match link's `Config::gather`.
    pub pack: Duration,
    /// `SRP_MAPDATA_DIR`: the bots' maps, shared by every match.
    pub maps: Option<Arc<bots::maps::Maps>>,
    /// `SRP_ANTI_AFK`: when an idle player is removed, if ever.
    pub anti_afk: afk::Setting,
    pub timing: Timing,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            late_inputs: LateInputs::Relay,
            resync_world_only: false,
            restore_limit: 0,
            clock_correction: true,
            check_initial_world: true,
            compress_state: true,
            snapshot_dir: None,
            record: None,
            pack: Config::for_table(Table::Match).gather,
            maps: None,
            anti_afk: afk::Setting::default(),
            timing: Timing::default(),
        }
    }
}

/// Where accepted snapshots are kept unless `SRP_SNAPSHOT_DIR` says
/// otherwise: the 0.21d server's place.
const SNAPSHOT_DIR: &str = "logs/snapshots";

impl Settings {
    pub fn from_env() -> Self {
        let var = |name: &str| std::env::var(name).ok();
        Self {
            late_inputs: parse_late_inputs(var("SRP_LATE_INPUTS").as_deref()),
            resync_world_only: session::switch("SRP_RESYNC_WORLD_ONLY", false),
            restore_limit: parse_number(
                "SRP_RESTORE_LIMIT",
                var("SRP_RESTORE_LIMIT").as_deref(),
                0,
            ),
            clock_correction: session::switch("SRP_CLOCK_CORRECTION", true),
            check_initial_world: session::switch("SRP_CHECK_INITIAL_WORLD", true),
            compress_state: session::switch("SRP_COMPRESS_STATE", true),
            snapshot_dir: parse_snapshot_dir(var("SRP_SNAPSHOT_DIR").as_deref()),
            record: record::RecordSettings::from_env(),
            pack: match var("SRP_RELAY_PACK_MS") {
                Some(value) => Duration::from_millis(u64::from(
                    parse_number("SRP_RELAY_PACK_MS", Some(&value), 15).min(50),
                )),
                None => Config::for_table(Table::Match).gather,
            },
            maps: parse_mapdata_dir(
                var("SRP_MAPDATA_DIR").as_deref(),
                parse_number(
                    "SRP_MAPDATA_KEEP",
                    var("SRP_MAPDATA_KEEP").as_deref(),
                    bots::maps::KEEP as u32,
                ),
                // With "Player vs Bots" on, one map for each match against
                // bots that may run at once (`SRP_PVB_MAX_MATCHES`).
                session::v026g::pvb::maps_at_once(
                    parse_number(
                        "SRP_MAPDATA_AT_ONCE",
                        var("SRP_MAPDATA_AT_ONCE").as_deref(),
                        bots::maps::AT_ONCE as u32,
                    ) as usize,
                    session::v026g::pvb::enabled(),
                    session::v026g::pvb::max_matches(),
                ) as u32,
                var("SRP_MAPDATA_BUILT").as_deref(),
            ),
            anti_afk: afk::Setting::parse(var("SRP_ANTI_AFK").as_deref()),
            timing: {
                let client_timeout = Duration::from_secs(u64::from(
                    parse_number(
                        "SRP_MATCH_CLIENT_TIMEOUT_SECONDS",
                        var("SRP_MATCH_CLIENT_TIMEOUT_SECONDS").as_deref(),
                        10,
                    )
                    .max(2),
                ));
                Timing {
                    client_timeout,
                    // A client loading its level is never given less than
                    // a playing one.
                    loading_timeout: Timing::default().loading_timeout.max(client_timeout),
                    // Never under the client table's own 1.75 s. The game
                    // catches up 20 ticks for each second of it, and the
                    // ticks on the wire reach 6.35 s from the clock it
                    // waits at: a sync cuts this short where its snapshot
                    // is old (`game.rs`, `entry_lifetime`).
                    entry_state: Duration::from_secs(u64::from(
                        parse_number(
                            "SRP_ENTRY_STATE_SECONDS",
                            var("SRP_ENTRY_STATE_SECONDS").as_deref(),
                            4,
                        )
                        .clamp(2, 5),
                    )),
                    // 15 for the old rule: the interval after any resync.
                    resync_after_agreed: Duration::from_secs(u64::from(
                        parse_number(
                            "SRP_RESYNC_AFTER_AGREED_SECONDS",
                            var("SRP_RESYNC_AFTER_AGREED_SECONDS").as_deref(),
                            5,
                        )
                        .clamp(3, 15),
                    )),
                    // Never under 10 s: an upload asked for is taken for
                    // 6 s (`game.rs`, `ASKED_FOR`).
                    pvb_stall: Duration::from_secs(u64::from(
                        parse_number(
                            "SRP_PVB_STALL_S",
                            var("SRP_PVB_STALL_S").as_deref(),
                            DEFAULT_PVB_STALL_S as u32,
                        )
                        .clamp(10, 3600),
                    )),
                    ..Timing::default()
                }
            },
        }
    }
}

fn parse_number(name: &str, value: Option<&str>, default: u32) -> u32 {
    match value.map(str::trim) {
        None | Some("") => default,
        Some(value) => value.parse().unwrap_or_else(|_| {
            log::error!("{name} is `{value}`; it must be a number");
            default
        }),
    }
}

fn parse_late_inputs(value: Option<&str>) -> LateInputs {
    match value
        .map(|value| value.trim().to_ascii_lowercase())
        .as_deref()
    {
        None | Some("") | Some("relay") => LateInputs::Relay,
        Some("discard") => LateInputs::Discard,
        Some("move") => LateInputs::Move,
        Some(other) => {
            log::error!(
                "SRP_LATE_INPUTS is `{other}`; it must be `relay`, `discard` or `move`. Using \
                 relay"
            );
            LateInputs::Relay
        }
    }
}

fn parse_snapshot_dir(value: Option<&str>) -> Option<PathBuf> {
    match value {
        Some(value) if value.trim().is_empty() || value.trim().eq_ignore_ascii_case("off") => None,
        Some(value) => Some(value.into()),
        None => Some(SNAPSHOT_DIR.into()),
    }
}

/// `SRP_MAPDATA_DIR`: unset, empty or `off` for none; `keep` built worlds
/// kept (`SRP_MAPDATA_KEEP`), `at_once` in memory at most
/// (`SRP_MAPDATA_AT_ONCE`); the built files in `built`
/// (`SRP_MAPDATA_BUILT`: unset or empty for `<dir>/built`, `off` for none).
fn parse_mapdata_dir(
    value: Option<&str>,
    keep: u32,
    at_once: u32,
    built: Option<&str>,
) -> Option<Arc<bots::maps::Maps>> {
    let dir = value
        .map(str::trim)
        .filter(|value| !value.is_empty() && !value.eq_ignore_ascii_case("off"))?;
    let built = match built.map(str::trim).filter(|value| !value.is_empty()) {
        Some(value) if value.eq_ignore_ascii_case("off") => None,
        Some(value) => Some(std::path::PathBuf::from(value)),
        None => Some(std::path::Path::new(dir).join(bots::maps::BUILT_DIR)),
    };
    if std::path::Path::new(dir).is_dir() {
        let files = match &built {
            Some(built) => built.display().to_string(),
            None => "none (built at each load)".into(),
        };
        log::info!(
            "Bot maps from {dir}, {} kept built, {} in memory at most; built files in {files}",
            keep.max(1),
            at_once.max(1)
        );
    } else {
        log::warn!("SRP_MAPDATA_DIR is `{dir}`, which is no directory; bots will have no maps");
    }
    Some(bots::maps::Maps::configured(
        Some(dir.into()),
        keep as usize,
        at_once as usize,
        built,
    ))
}
