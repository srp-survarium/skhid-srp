//! One online match as a relay (sans IO): the join handshake of every player, the cached first world, and the
//! per-tick DF stream (docs/notes/netcode.md, docs/notes/online.md).
//!
//! Per player: `C9 -> DC (ports) -> [DC acked or CA seen] -> DD (options + level checksum) -> CB (ready, after the level
//! load) -> DE (cached initial world, once the match starts) -> DF every tick`. The match starts when every roster
//! player is ready, or `ready_timeout` after creation with the players that are ready (others are refused: no late
//! join without a world source, see README). Match clock `M = world.time + elapsed`; tick `T` (a multiple of
//! `tick_ms` after `world.time`) is sent as soon as `M >= T + relay_delay`, so inputs stamped `T` that arrive within
//! `relay_delay` still go out as normal inputs; later ones go out as `n_late` entries of a later DF.
//!
//! Every field marked UNVERIFIED in [`crate::match_msgs`] is used here as documented there.
//!
//! Server-owned slots ([`PlayerSpec::bot`], the 17 script bots of a PvE mission): no ticket, no transport, ready from
//! the start, never synced, never silent or left. They enter in the same DF as the humans (the script spawns only
//! entered players, 0x2003CD87D) with no start profile (the script spawns them with its own profile byte; whether a bot
//! `profile_input` triggers a rule respawn is OPEN: none is sent), and are left out of the enter gate (every entered
//! human answered a DF), the silence rule, the "everybody left" end and the match result. No client runs the AI online
//! (`single_player` gate 0x20023A19B), so without host bots a bot gets no input and stands where the script spawns it.

use crate::match_msgs::{
    ClientUpdate, Connect, GameUpdate, Packing, PlayerInput, ProfileInput, Ports, QueryWorld, Ready, StaticInfo, WorldMsg, MAX_PLAYERS,
};
use crate::mtransport::{Delivery, Event, MatchTransport, MsgKey};
use crate::table::id;
use crate::world::World;
use protocol::{lzma2, Decode, Encode};
use std::collections::{BTreeMap, VecDeque};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Largest DF payload: the client copies only 0x1000 bytes of a DF into its job but parses the full length
/// (0x2005BB7B0); one byte spare in case the length counts the id.
const DF_MAX_PAYLOAD: usize = 0x1000 - 1;
/// DF list capacities of the client (fixed buffers of the DF handler 0x2001AFE7D; more asserts "buffer overflow" in the
/// reader).
const DF_MAX_LATE: usize = 60;
const DF_MAX_INPUTS: usize = 20;
const DF_MAX_PROFILES: usize = 20;
const DF_MAX_PINGS: usize = 20;
/// ai entries per DF: the client's `buffer_vector<client_update<ai_player_input>>` holds 20 (0x20032E9EE)
const DF_MAX_AI: usize = 20;
/// How far back the rejoin history of DFs reaches: the host's DE lies `final_lag_ms` (300) behind the newest DF and
/// comes within `de_wait` (2 s); its delivery to the client (up to 255 parts, a resend after 500 ms) follows.
pub const REJOIN_HISTORY_MS: u32 = 20_000;
/// The connections of a code-2 end stay at least this long after the closing DFs (see `Match::is_done`).
pub const FIN_KEEPS_WORLD_DRAIN: Duration = Duration::from_secs(6);
/// The PvP winner the lobby gets from the host's world byte `S+0xEBC0` (0 / 1 a team, 2 draw, 3 none: the statistics
/// core's initial value). A world that ended normally (state 5) with 3 had no team win: artefact hunt after its draw
/// time on a tied score, battery retrieval and slaughter on a tied time end leave the byte at 3, where tdm writes 2
/// (0x200366B4B). Both are a draw to the players: no team won. The lobby gets 2 (it pays a draw as a played, not won
/// match, and its `97` shows 2).
/// State 4 (the wait timeout: the match never started) keeps 3, "no result".
pub fn pvp_winner(state: u8, winner: u8) -> u8 {
    if state == 5 && winner == 3 {
        2
    } else {
        winner
    }
}

/// Game mode id of the gun race (Arms Race; the exe's mode table entry 3 `gunrace`).
pub const GUNRACE_MODE: u32 = 3;
/// The winner the lobby gets for a gun race world that did not end with a race result (state 4: arena.lua's pre-game
/// cancel after 7 minutes, `success false, winner_team 1`; a world that never ended): no player won and nothing is paid.
/// Not a record index (0..7) and not arena.lua's 9 (the 10-minute end, a played match nobody won).
pub const GUNRACE_NO_RESULT: u8 = 0xFE;

/// The winner byte of the host's world as the lobby takes it, per mode. The gun race (docs/notes/gun-race.md) is all
/// versus all: arena.lua's action 26 writes `player_id.player_N` (the winner's record index, his team) or 9 (the time
/// end, nobody) with `success true` (state 5); every other state is [`GUNRACE_NO_RESULT`]. Other modes: [`pvp_winner`].
pub fn mode_winner(mode: u32, state: u8, winner: u8) -> u8 {
    if mode == GUNRACE_MODE {
        if state == 5 {
            winner
        } else {
            GUNRACE_NO_RESULT
        }
    } else {
        pvp_winner(state, winner)
    }
}

/// The head of an AI player's spawn profile: profile 0, respawn request 1, autorespawn 1 (the bytes of the local start,
/// 0x2001F4BEA; the reader copies them 1:1), no entries.
pub const BOT_SPAWN_PROFILE: [u8; 4] = [0, 1, 1, 0];

/// Host bots (protocol 5) counters of one match.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct BotsStats {
    /// `Bots` frames applied (duplicates of a restarted host not counted)
    pub frames: u32,
    /// bot inputs put into their own DF / sent late (their `Bots` came after the DF)
    pub inputs: u32,
    pub late_inputs: u32,
    /// ai entries put into their DF / lost (their `Bots` came after the DF, or over the 20 of a DF)
    pub ai: u32,
    pub ai_lost: u32,
    /// DFs held for the `Bots` of the previous step / held to the deadline without it
    pub holds: u32,
    pub hold_timeouts: u32,
    /// `Bots` frames of a restarted host for times already filed: ignored
    pub duplicates: u32,
}

/// What the server puts in `server_input_confirmation {a, b}`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfMode {
    /// `b` = the match clock when the CD arrived (`world.time + elapsed`), the default. PROVEN need
    /// (0x2001AF50D): the client's servo sample is `b - a + 2` (the cvar at 0x200AA2218 defaults to 2) and only
    /// a sample of 0 stops its correction, so `b` must carry the server's real receive time: then the sample settles at
    /// 0 when the client stamp leads the server clock by 2 ms.
    ReceiveClock,
    /// `b = a`: WRONG for real clients (every sample is +2, no feedback: the client clock runs ahead without limit and
    /// after ~400 ms every DF is dropped); kept only for tests.
    EchoStamp,
    /// `b` = the server time (tick `T` of the DF that carries it); a constant positive offset makes the client clock
    /// run fast (LIKELY), kept for experiments.
    ServerTime,
}

#[derive(Clone, Debug)]
pub struct MatchConfig {
    /// DF for tick T goes out when the match clock reaches `T + relay_delay` (jitter buffer for slow CDs).
    pub relay_delay: Duration,
    /// Wait this long after creation for every player to be ready, then start with those that are.
    pub ready_timeout: Duration,
    /// Hard limit of a running match (the end by rules is seen by the clients; this is the server's backstop).
    pub max_duration: Duration,
    /// A player silent this long is treated as left.
    pub peer_timeout: Duration,
    /// After the end, keep the connections this long so the last DF and acks go out.
    pub drain: Duration,
    /// A late input older than this (ms) is not relayed: the client would "move it locally" and desync (event horizon
    /// 500 ms).
    pub late_max_ms: u32,
    pub conf: ConfMode,
    pub packing: Packing,
    /// Send the DE again when no CD arrived this long after it (the DE lifetime is only 1900 ms), up to `de_retries`.
    pub de_retry: Duration,
    pub de_retries: u8,
    /// Minimum gap between answers to CC from one player.
    pub cc_gap: Duration,
    /// The enter goes out once every entered player has sent a CD (proof its client processed the DE and takes DFs);
    /// after this long it goes out anyway.
    pub enter_wait: Duration,
    /// DFs that repeat the enter bits (a repeated enter is a no-op, 0x2002F2C55), against a lost DF.
    pub enter_repeats: u32,
    pub transport: crate::mtransport::Config,
    /// Write every match's full input stream to `<dir>/match_<id>_<unix>.srprec` ([`Recorder`]); None = off.
    pub record_dir: Option<std::path::PathBuf>,
    /// The world host ([`crate::host`]): attached to every online match (PvE: phase 1, PvP: phase 3).
    /// None = off (the default): every match runs as without it.
    pub world_host: Option<crate::host::HostConfig>,
    /// A mirrored PvE failure end (the script's cancel or 45-minute end) is handed to the lobby as the result
    /// (`Hub::take_outcomes`) instead of closing at once; the lobby shows `99` / `83 45 0B` and ends the match. This long
    /// after the hand-over the relay closes anyway (backstop). None = close at the mirrored end.
    pub pve_handover: Option<Duration>,
    /// The PvP end screen: a PvP world that ended in a live world host first gets its result
    /// (the host's `Close`, its `Final`, then the `MatchEnd` while the clients still run), so the lobby's `83 45 0A` +
    /// `97` reach the clients while their match is loaded (the in-match victory screen); the lobby then ends the match
    /// and the closing DFs carry code 2 (the client keeps its world: splash, statistics, its own way back). The relay's
    /// time-limit / empty-team close waits `HostConfig::mirror_wait` for the host's Outcome. This long after the early
    /// result the relay closes by itself (a lobby that does not answer). Always on (no switch).
    pub pvp_end_screen: Duration,
    /// A match whose start world the world host builds (protocol 4) ends with [`EndReason::NoWorld`] when the host's
    /// `Ready` has not brought it this long after the creation (a cold zygote loads the level first: about 10 s for PvE).
    pub host_start_timeout: Duration,
    /// Rejoin: with a world host attached, an entered human who drops out of a running match without the menu "leave
    /// match" (no CD for `peer_timeout`, transport silence or disconnect) keeps his slot: no leave bit, one neutral input
    /// (the last action mask persists on every client), the match goes on. His client, sent back by the lobby's `82`,
    /// connects again, loads the level and gets the host's DE of the running world plus the DFs after it; then it plays
    /// on. false = a drop is a leave.
    pub rejoin: bool,
    /// Every entered human of the match has been dropped this long (nobody to play for): they leave for good and the
    /// match ends (AllLeft). CHOICE (no client value): 180 s, room for a game restart, a sign-in and a level load.
    pub rejoin_grace: Duration,
}

impl Default for MatchConfig {
    fn default() -> Self {
        let mut transport = crate::mtransport::Config::default();
        transport.timeout = Duration::from_secs(5);
        Self {
            // below the 100 ms enter tolerance a client has before its first ping (0x2002F1400)
            relay_delay: Duration::from_millis(50),
            ready_timeout: Duration::from_secs(60),
            max_duration: Duration::from_secs(40 * 60),
            peer_timeout: Duration::from_secs(5),
            drain: Duration::from_secs(2),
            late_max_ms: 400,
            conf: ConfMode::ReceiveClock,
            packing: Packing::StoredLzma2,
            de_retry: Duration::from_secs(3),
            de_retries: 3,
            cc_gap: Duration::from_secs(2),
            enter_wait: Duration::from_secs(20),
            // one DF: a repeat would be accepted later by a client that missed the first (it would enter later than the rest)
            enter_repeats: 1,
            transport,
            record_dir: None,
            world_host: None,
            pve_handover: Some(Duration::from_secs(10)),
            pvp_end_screen: Duration::from_secs(10),
            host_start_timeout: Duration::from_secs(45),
            rejoin: true,
            rejoin_grace: Duration::from_secs(180),
        }
    }
}

/// A player of the match, in roster order (index = player index in the match options).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayerSpec {
    pub account_id: u64,
    pub name: String,
    pub team: u8,
    /// When set, C9's session must equal it (login session of the account); `None` = not checked.
    pub session: Option<u32>,
    /// A server-owned slot (a script bot of a PvE mission): no ticket, no connection, enters with the humans, sends
    /// nothing (see the module docs). Bot slots follow the humans in the options' record order.
    pub bot: bool,
}

impl PlayerSpec {
    /// A server-owned script bot slot (record name, team).
    pub fn bot(name: &str, team: u8) -> Self {
        PlayerSpec { account_id: 0, name: name.to_string(), team, session: None, bot: true }
    }
}

/// What the lobby gives to create a match.
#[derive(Clone, Debug)]
pub struct MatchSpec {
    /// The match id: the world's seed, also the options' `match_id` (header bytes 6..10, opts+0x1074; every match gets
    /// its own, [`crate::Hub::new_match_id`]). `None` = the options' value; Some(x) must equal it.
    pub match_id: Option<u32>,
    pub map: u32,
    pub mode: u32,
    /// The raw (uncompressed) `match_options` blob built by the lobby (layout: local-match `options.rs`); players in
    /// the same order as `players`. Its header gives the match id, `tick_ms` and `send_period`.
    pub options: Vec<u8>,
    pub players: Vec<PlayerSpec>,
    /// DD checksum override; default: the start world's (the host's `Ready.collision_checksum`).
    pub level_checksum: Option<u32>,
    /// Only for a given world (no world host, [`crate::Hub::new`]'s store: tests and a host-less setup): the objects
    /// this roster's kits put in the starting world, swapped for the stored world's ([`crate::World::with_roster`]).
    /// The world host builds them itself from the options.
    pub roster: Option<crate::world::RosterObjects>,
    /// The level project (`level_01_day_tdm`, `pve_01_v1_maze2`, ...): the world host builds the start world of this
    /// project (required with a host); without a host it picks the stored world (`None` = any world of (map, mode)).
    pub project: Option<String>,
    /// The clients' time-limit rule of this mode, mirrored so the relay closes the match at the tick their worlds end
    /// (a client never closes an online match by itself). None = no timed end (the lobby or the players end it).
    pub end_rule: Option<EndRule>,
    /// A PvE mission: the world-free failure ends of its script, mirrored the same way ([`PveEndRule`]). None = not PvE.
    pub pve_end: Option<PveEndRule>,
    /// The AI players ignore the humans (a custom room's "bots ignore players"): the world host's `Create` carries it
    /// (protocol 7) and the host sets the exe's harmless-bots byte. Only a host that drives the bots acts on it.
    pub ai_ignore_players: bool,
}

/// The ends of a PvE mission's script that a server with no world can compute exactly (docs/notes/pve.md "Mission
/// failure ends"). In a PvE world only the scripting rule can end the match, with action 26. Every end mirrored here
/// is a failure with `winner_team` 1 (the bots' team; the humans are team 0):
///
/// * the pre-start cancel (world state 4): 7 min after the first scripting step unless the mission started; 5 min for
///   a 3-human mission with 2 entered and 1 of them gone; at once when a leave before the start leaves at most one human;
/// * the mission timeout (world state 5): 45 min after the start (all humans entered + 10 s, or 2 of 3 entered + 60 s);
/// * every human left: the same failure.
///
/// Not computable without the world and never inferred here: the extraction (the only success) and every human dead
/// past the 15 s resurrection window.
///
/// Timing: the script steps only on tick-grid world steps (0x2003626E0); a wait completes at the first scripting step
/// with `t >= step time + ms`; processors run in script index order, so one enabled by a lower index runs in the same
/// step; enter / leave events are handled in the world step of their DF. In the start world `match` (index 1) runs in
/// the first step after the DE, `world.time + tick`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PveEndRule {
    /// `wait_before_cancel`: action 0 `time_in_ms = minutes_to_wait * 60000`, `minutes_to_wait` 7.
    pub cancel_ms: u32,
    /// `wait_before_cancel2`: action 0 `minutes_to_wait * 60000`, `minutes_to_wait` 5.
    pub cancel2_ms: u32,
    /// `all_players_ready`: action 0 `seconds_to_wait * 1000`, `seconds_to_wait` 10.
    pub start_all_ms: u32,
    /// `2_from_3_players_ready`: action 0 `seconds_to_wait * 1000`, `seconds_to_wait` 60.
    pub start_two_of_three_ms: u32,
    /// `start_match_instantly`: action 0 `minutes_per_match * 60000`, `minutes_per_match` 45.
    pub match_ms: u32,
}

impl PveEndRule {
    /// The numbers of pve.lua (gameplay/scripting/pve.lua; one script for all six `pve_01_*` projects).
    pub const PVE_LUA: PveEndRule =
        PveEndRule { cancel_ms: 7 * 60_000, cancel2_ms: 5 * 60_000, start_all_ms: 10 * 1000, start_two_of_three_ms: 60 * 1000, match_ms: 45 * 60_000 };
}

/// The relay's copy of the script state the PvE ends depend on (all world times, the DF clock).
#[derive(Clone, Copy, Debug, Default)]
struct PveMirror {
    /// World step of a pending cancel's action 26 (state 4).
    cancel_at: Option<u32>,
    /// World step where `start_match_instantly` runs (`match.started` becomes true).
    start_at: Option<u32>,
    /// World step of the 45-minute action 26 (state 5).
    timeout_at: Option<u32>,
    /// Human slots in the enter DF: only they make events 18 and 19.
    entered: u32,
    /// `has_entered_match.players_count` / `has_left_match.players_count`
    entered_n: u32,
    left_n: u32,
}

/// The timelimit rule's numbers (`gameplay/game_modes/<project>.options` `mode.parameters`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EndRule {
    pub countdown_ms: u32,
    /// 0 = no time limit
    pub match_ms: u32,
    pub advantage_ms: u32,
    /// `wait_player_percent` (f32 as the client reads it)
    pub wait_ratio: f32,
    /// `leave_player_percent`
    pub leave_ratio: f32,
    /// The mode's tdm rule ends the match 3 s after a team has no player left, and the other team wins (0x200366DB0).
    pub team_empty_end: bool,
    /// The world may play on this long after the time end: artefact hunt's draw time (`draw_additional_time`,
    /// `artefact_hunt_rule_core` +0x2F0, read 0x20035D0AD). On a tied score the world ends at start + match_time + this
    /// (e.g. level_01_day_ah: state 5 at 21100 + 900000 + 180000), so the clients' worlds are still running at the
    /// mirror's close. With a live world host the close waits for the host's Outcome, at most this long (plus 5 s);
    /// without one the mirror closes at the time end (it cannot see the score). 0 = no overtime.
    pub draw_overtime_ms: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndReason {
    /// Every player that entered has left (the clients' own match end: the rules reach state 4/5 and each client closes
    /// the match; only the menu "leave match" sends CE, the normal end `close_current_match(3)` sends nothing, so those
    /// players leave by the transport's disconnect or silence: PROVEN, 0x2001F5299 / 0x2001F53E1).
    AllLeft,
    /// `max_duration` reached.
    Timeout,
    /// Nobody was ready within `ready_timeout`.
    ReadyTimeout,
    /// The lobby asked for the end.
    Requested,
    /// The clients' time-limit or leave-advantage end, mirrored from their timelimit rule (`EndRule`).
    TimeLimit,
    /// PvE: the mission script's pre-start cancel (`wait_before_cancel`, `wait_before_cancel2`, `cancel_match_instantly`;
    /// world state 4), mirrored from [`PveEndRule`]. A failure: winner 1.
    PveCancel,
    /// PvE: the mission script's 45-minute end (`start_match_instantly`; world state 5), mirrored from [`PveEndRule`]. A
    /// failure: winner 1.
    PveTimeout,
    /// PvP with a world host: the host's world reached state 4 / 5 (any rule: tickets, deliveries, points, rounds, time)
    /// before the relay's own mirror; every client is closed at once (an online client never closes by itself) and the
    /// winner is the world's.
    HostEnd,
    /// The world host did not deliver the start world (no connection, it refused the `Create`, it failed before
    /// `Ready`, or `host_start_timeout`): nobody got a DD, nobody entered; the clients' connection times out and they
    /// show their own text, the lobby gives back what the start took.
    NoWorld,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeaveReason {
    /// Never connected / never ready / refused as a late joiner.
    NeverEntered,
    /// CE: the menu "leave match" (`close_current_match(2)`, 0x2001F53E1; the only sender of CE, PROVEN): a voluntary
    /// leave.
    Menu,
    /// The transport's disconnect (the normal end of a client, a crash or a closed game).
    Disconnected,
    /// Silent for `peer_timeout`.
    TimedOut,
    /// Still in the match when it ended.
    MatchEnded,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayerResult {
    pub slot: u8,
    pub account_id: u64,
    pub team: u8,
    pub entered: bool,
    pub leave: LeaveReason,
}

/// Callback payload to the lobby when a match ends. The server has NO world, so it cannot know kills, deaths or score:
/// `winner` is what the relay computed without a world (an empty TDM team; a PvE mission's script failure, winner_team 1:
/// [`PveEndRule`]) or what the lobby set with `set_winner`, and `None` otherwise. Per-player statistics are the lobby's
/// to synthesize.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchEnd {
    pub match_id: u32,
    pub map: u32,
    pub mode: u32,
    pub reason: EndReason,
    pub winner: Option<u8>,
    pub duration: Duration,
    pub players: Vec<PlayerResult>,
    /// The AI players of an online PvP match (account 0) with their teams: the lobby needs them to tell a kill of an
    /// enemy bot from a team kill. Empty for a PvE mission: its script bots stay out of the lobby's roster (no evidence
    /// that the original counted them, e.g. for "killed every enemy").
    pub bots: Vec<PlayerResult>,
    /// Per-player statistics when a source gave them ([`Match::set_stats`]: a referee client reading the client's
    /// statistics object, docs/notes/online.md section 3); `None` = unknown, never invented.
    pub stats: Option<FinalStats>,
}

/// Real counters of one player of a finished match (the statistics object of the world).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerStat {
    pub slot: u8,
    pub kills: u16,
    pub deaths: u16,
    pub score: u16,
    /// event row 42 `pve_open_chest` of the player's statistics block; 0 outside PvE
    pub chests: u16,
}

/// The end of the world as the world host saw it ([`crate::host`]: its latched `Outcome`), for the lobby
/// (`Hub::take_outcomes`): delivered once, as soon as it arrives, while the match still runs (the lobby owns the end
/// sequence).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostOutcome {
    pub match_id: u32,
    /// the DF time where the world's end state was set
    pub t_end: u32,
    /// world end state `W+0x180AE0`: 4 or 5
    pub state: u8,
    /// the statistics object's winner byte `S+0xEBC0` at the world's end tick: 0 / 1 = that team, 2 = draw, 3 = none (the
    /// core's initial value). tdm rewrites it at every death and leave (0x200366A60): no present player on either team
    /// -> 2; one team empty -> the other; else more tickets; equal tickets > 0 -> 2; equal at 0 -> more present players,
    /// else 2
    pub winner_team: u8,
    /// PvE: winner team 0, the extraction (OPEN until a finished mission is recorded)
    pub pve_success: bool,
    /// the host's world time from the enter DF to `t_end` (OPEN: whether the result screen shows this value)
    pub mission_ms: u32,
    pub players: Vec<OutcomePlayer>,
    pub team_scores: [u32; 2],
    /// version 3: the detail rows of the roster's slots at the end (no badges: those come with `Final`)
    pub details: Vec<world_host_proto::PlayerDetail>,
}

/// One player row of a [`HostOutcome`]; rows whose slot is not in the match are dropped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutcomePlayer {
    pub slot: u8,
    /// 0 for a server-owned bot slot
    pub account_id: u64,
    pub bot: bool,
    pub kills: u16,
    pub deaths: u16,
    pub score: u16,
    pub chests: u16,
    /// PvE paid respawns the world charged: access keys (entry `+0x54`) and gold (entry `+0x58`), protocol version 2
    pub keys_spent: u16,
    pub gold_spent: u32,
}

/// What a source that observed the world (a referee client) reports at the end: every player's counters and the two
/// team scores (tickets or points of the rule core).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FinalStats {
    pub players: Vec<PlayerStat>,
    pub team_scores: [u32; 2],
    /// world host version 3: the detail rows (event matrix, streaks, inactive flag, badge counts) of the roster's slots;
    /// from `Final` (badges after the match end), else from `Outcome` (no badges); empty = none
    pub details: Vec<world_host_proto::PlayerDetail>,
    /// world host version 3: every death of the match (`Kills` frames before `Final`); None = not complete or not
    /// consistent with the event rows (never invented)
    pub kills: Option<Vec<world_host_proto::KillRecord>>,
    /// world host version 6: per human slot what he used (profiles spawned with, their weapons, his ammo / pocket
    /// stacks' use; the `Used` frame before `Final`); None = not reported (an older host, no Final, or diverged)
    pub used: Option<Vec<world_host_proto::PlayerUse>>,
}

/// Counters for logs and tests.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MatchStats {
    pub phase: &'static str,
    pub ticks_sent: u32,
    pub inputs_relayed: u32,
    pub late_relayed: u32,
    pub late_dropped: u32,
    /// inputs moved to the next DF because a DF would pass DF_MAX_PAYLOAD
    pub df_deferred: u32,
    pub cd_duplicates: u32,
    pub cc_received: u32,
    pub de_sent: u32,
    pub ready: usize,
    /// humans that entered (server-owned bots are counted in `bots`)
    pub entered: usize,
    pub left: usize,
    pub late_refused: usize,
    /// server-owned bot slots that entered
    pub bots: usize,
    /// the world host link (`""` = none attached; else [`crate::host::LinkState::name`]) and its restarts
    pub host: &'static str,
    pub host_restarts: u32,
    /// phase 2: E0 messages sent (per recipient), host ticks too old for the clients' ring, CCs logged and not answered
    pub e0_sent: u32,
    pub e0_late: u32,
    pub cc_logged: u32,
    /// phase 2 resync (`HostConfig::resync`): WantDe frames sent; host DEs sent to clients (per recipient); CCs ignored
    /// (a tick at or below the client's last resync, or the client's breaker is off); WantDe answers refused, lost or
    /// failing the relay's checks; clients whose breaker switched resync off; E0 held back after a client's resync
    pub resync_asked: u32,
    pub resync_sent: u32,
    pub resync_ignored: u32,
    pub resync_refused: u32,
    pub resync_tripped: u32,
    pub e0_held: u32,
    /// resync v2 ([`ResyncSlot`]): first CCs that waited for a confirming second one (rule 2);
    /// WantDe / host DEs held back by the scripted-step guard (rule 3); resync DEs every part of which the client acked,
    /// or whose lifetime ended first (rule 1); CCs above the floor within 2 s of an ack (a re-trigger, should stay 0)
    pub resync_unconfirmed: u32,
    pub resync_deferred: u32,
    pub resync_acked: u32,
    pub resync_expired: u32,
    pub resync_retriggers: u32,
    /// rejoin: humans dropped with their slot kept; dropped humans whose client came back and plays again; rejoin DEs
    /// asked again (the history did not reach back to the DE, or the DE expired)
    pub dropped: u32,
    pub rejoined: u32,
    pub rejoin_retries: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Waiting,
    Running,
    Ended,
}

struct Conn {
    addr: SocketAddr,
    t: MatchTransport,
}

/// Per-slot traffic counters for the periodic diagnostics line.
#[derive(Clone, Copy, Debug, Default)]
struct SlotDiag {
    cds: u32,
    now: u32,
    late: u32,
    profile: u32,
    ping: u32,
    /// input entries of this player put into DFs (to the others)
    relayed: u32,
    /// confirmations sent to this player
    confs: u32,
    /// since the last diagnostics line: the CDs' lead (client stamp minus the match clock at arrival, ms) after the enter:
    /// sum, count, min, max. The clients' servo keeps it near -2 ms when both clocks run at one rate;
    /// a steady lead far from it means their clock runs at another rate
    lead_sum: i64,
    lead_n: u32,
    lead_min: i32,
    lead_max: i32,
}

struct Slot {
    diag: SlotDiag,
    /// when the last CD arrived (an entered client sends one every tick)
    last_cd: Option<Instant>,
    /// the DF the previous CD landed in, and that CD's time (a player's CDs land in non-decreasing DFs)
    last_d: u32,
    last_cd_time: u32,
    spec: PlayerSpec,
    order_id: u32,
    conn: Option<Conn>,
    dc_sent: Option<MsgKey>,
    probe_seen: bool,
    dd_sent: bool,
    ready: bool,
    profile: ProfileInput,
    entered: bool,
    left: Option<LeaveReason>,
    leave_pending: bool,
    late_refused: bool,
    de_sent: Option<Instant>,
    de_tries: u8,
    got_cd: bool,
    cd_seen: VecDeque<u32>,
    /// `(a = the CD's client stamp, match clock when it arrived)`
    /// Confirmations `(a = the CD's client stamp, b)` by the DF time they go out in (the DF its inputs got).
    conf_at: BTreeMap<u32, Vec<(u32, u32)>>,
    last_cc: Option<Instant>,
    /// phase 2 resync ([`Match::on_query`]): the newest CC tick waiting for a host DE
    resync_want: Option<u32>,
    /// no E0 at or below this tick and CCs at or below it are ignored: the DE time, or the newest DF when the client
    /// acked the whole DE if later (the client's own hashes of ticks up to there predate the load: DFs that overtake a
    /// resent DE part are hashed in the old world)
    resync_floor: u32,
    /// when host DEs went to this client (the breaker's window)
    resync_times: VecDeque<Instant>,
    /// resync is off for this client for the rest of the match (its `resync_match_max` DEs are used up)
    resync_off: bool,
    /// resync v2 state ([`ResyncSlot`])
    rs: ResyncSlot,
    /// rejoin ([`MatchConfig::rejoin`]): the slot dropped out at this time and waits for its client (no leave bit), and
    /// how (TimedOut / Disconnected: its leave reason if it never comes back); None = connected or never dropped
    dropped: Option<(Instant, LeaveReason)>,
    /// rejoin: a new connection came for the dropped slot (C9 with its order id): DD, CB, then the host's DE and the DFs
    /// after it; until every DE part is acked it gets nothing else
    rejoining: bool,
}

/// Per-client resync state (resync v2, rules 1-5 below).
#[derive(Debug, Default)]
struct ResyncSlot {
    /// rule 2: the first CC of a mismatch (its tick, when it came): a DE is asked only for a second CC above it within
    /// `HostConfig::resync_confirm`
    first_cc: Option<(u32, Instant)>,
    /// rule 1: the resync DE in flight (its transport key, its time, when it went out): until every part is acked the
    /// client gets no E0 and its CCs are ignored; at the ack the floor becomes the newest DF
    pending: Option<(MsgKey, u32, Instant)>,
    /// rule 4: end of the breaker's cooldown (None = not tripped)
    off_until: Option<Instant>,
    /// rule 4: host DEs sent to this client in the match
    total: u32,
    /// logging (rule 5): when the last resync DE was acked, to count CCs above the floor soon after
    acked_at: Option<Instant>,
    /// rule 3: the scripted-step guard holds this client's DE back until the host's final tick reaches this (logged once)
    deferred: Option<u32>,
}

/// The full input stream of one match, for replaying it into a world simulation (the world host's replay compares its
/// per-tick hashes with a real client's). Little-endian file:
/// `b"SRPREC01"`, u32 match id, u32 world time (the DE's), u8 tick ms, u8 records, per record {u8 bot, u8 team},
/// u32 len + the options blob (DD), u32 len + the decoded world item (DE), then per DF: u8 kind 1, u32 tick, u32 len,
/// the DF bytes BEFORE the per-recipient filtering (every player's inputs, late entries, profiles, pings, enter / leave
/// masks; no confirmations), and at the end u8 kind 0xFF, u32 tick of the close. A write error turns recording off.
pub struct Recorder {
    w: std::io::BufWriter<std::fs::File>,
    path: std::path::PathBuf,
}

/// The recorder of a match once its start world is there (logged either way).
fn open_recorder(dir: &std::path::Path, match_id: u32, players: &[(bool, u8)], options: &[u8], world: &World) -> Option<Recorder> {
    match Recorder::open(dir, match_id, players, options, world) {
        Ok(r) => {
            log::info!("match {match_id:#x}: recording the input stream to {}", r.path.display());
            Some(r)
        }
        Err(e) => {
            log::warn!("match {match_id:#x}: no input recording ({}): {e}", dir.display());
            None
        }
    }
}

impl Recorder {
    fn open(dir: &std::path::Path, match_id: u32, players: &[(bool, u8)], options: &[u8], world: &World) -> std::io::Result<Self> {
        use std::io::Write;
        std::fs::create_dir_all(dir)?;
        let unix = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let path = dir.join(format!("match_{match_id:08x}_{unix}.srprec"));
        let mut w = std::io::BufWriter::new(std::fs::File::create(&path)?);
        let item = world.item().map_err(|e| std::io::Error::other(e.to_string()))?;
        w.write_all(b"SRPREC01")?;
        w.write_all(&match_id.to_le_bytes())?;
        w.write_all(&world.time.to_le_bytes())?;
        w.write_all(&[world.tick_ms, players.len() as u8])?;
        for (bot, team) in players {
            w.write_all(&[u8::from(*bot), *team])?;
        }
        for blob in [options, &item[..]] {
            w.write_all(&(blob.len() as u32).to_le_bytes())?;
            w.write_all(blob)?;
        }
        Ok(Self { w, path })
    }

    fn df(&mut self, t: u32, bytes: &[u8]) -> std::io::Result<()> {
        use std::io::Write;
        self.w.write_all(&[1])?;
        self.w.write_all(&t.to_le_bytes())?;
        self.w.write_all(&(bytes.len() as u32).to_le_bytes())?;
        self.w.write_all(bytes)
    }

    fn close(&mut self, t: u32) -> std::io::Result<()> {
        use std::io::Write;
        self.w.write_all(&[0xFF])?;
        self.w.write_all(&t.to_le_bytes())?;
        self.w.flush()
    }
}

pub struct Match {
    id: u32,
    map: u32,
    mode: u32,
    cfg: MatchConfig,
    rec: Option<Recorder>,
    /// the start world (the first DE); a placeholder (time 0) until the world host's `Ready` brings it
    world: Arc<World>,
    latest: Arc<World>,
    /// the start world is there (a given world, or the host's `Ready`): DD, the start and the recorder wait for it
    world_ready: bool,
    /// the start world is a host-built PvE world past its first step: pve.lua `match` ran at its own time
    start_stepped: bool,
    /// the recorder's directory, kept until the start world opens the recording
    rec_dir: Option<std::path::PathBuf>,
    /// the players' (bot, team) and the options, for the recording's head
    spec_players: Vec<(bool, u8)>,
    spec_options: Vec<u8>,
    options_lzma2: Vec<u8>,
    checksum: u32,
    tick_ms: u32,
    created: Instant,
    phase: Phase,
    slots: Vec<Slot>,
    winner: Option<u8>,
    player_stats: Option<FinalStats>,
    start: Option<Instant>,
    next_tick: u32,
    /// Match clock at the last poll (world time + elapsed).
    clock: u32,
    future: BTreeMap<u32, Vec<(u8, PlayerInput)>>,
    late_q: Vec<(u32, u8, PlayerInput)>,
    /// precise-time inputs (CD late entries) by DF: (dt ms before the DF time, player, input)
    late_at: BTreeMap<u32, Vec<(u8, u8, PlayerInput)>>,
    /// The server's own profile entries (the start profiles at the enter): to everybody, in the next DF.
    profile_q: Vec<(u8, ProfileInput)>,
    /// The CDs' profile and ping entries by the DF time their CD landed in.
    cd_profile_at: BTreeMap<u32, Vec<(u8, ProfileInput)>>,
    ping_at: BTreeMap<u32, Vec<(u8, u16)>>,
    /// DF time of the enter (every player's enter bit and start profile), None until every entered player synced.
    entered_at: Option<u32>,
    enter_repeats_left: u32,
    /// last periodic diagnostics line
    diag_at: Option<Instant>,
    /// players who left since the last `take_left`: (slot, why)
    left_events: Vec<(u8, LeaveReason)>,
    /// `enter_wait` passed without every CD: the enter goes out anyway.
    enter_forced: bool,
    end_rule: Option<EndRule>,
    /// "Match started" time of the clients' worlds and their current end (T_end; the close follows 3000 ms later)
    t_start: Option<u32>,
    t_end: Option<u32>,
    /// the first DF tick after this closes every client (the clients' worlds are in state 5 then)
    t_close: Option<u32>,
    /// the draw-overtime hold was logged ([`Match::overtime_hold`])
    overtime_logged: bool,
    /// the team counts the clients' timelimit rule holds (loaded from G3: records >> 1 each; u8 like the client)
    team_counts: [u8; 2],
    pve_end: Option<PveEndRule>,
    pve: PveMirror,
    end: Option<(EndReason, Instant)>,
    result: Option<MatchEnd>,
    stats: MatchStats,
    /// The world host link, [`crate::host`].
    host: Option<crate::host::HostLink>,
    /// the host's outcome for the lobby, not taken yet
    outcomes: Vec<HostOutcome>,
    /// the host's Outcome came while the match still ran: its values are the match's
    outcome_before_end: bool,
    /// the `MatchEnd` kept back for the host's `Final`, since when
    held: Option<(MatchEnd, Instant)>,
    /// the last normal DF (not a fin DF): `Close.t_last`
    last_df: Option<u32>,
    /// a mirrored PvE failure end waits for the host's Outcome since then ([`Match::mirror_hold`])
    mirror_wait: Option<Instant>,
    /// the closing DFs carry code 2 (the client keeps its world and HUD: the PvE result screen), see [`Match::end`]
    fin_keeps_world: bool,
    /// the relay's own mirrored failure end went to the lobby as the result ([`Match::hand_over`])
    handed_over: bool,
    /// PvE: the lobby ended the match after its result went out ([`Match::end_after_result`])
    result_sent: bool,
    /// PvP end screen: the host got its `Close` while the clients run on (since then); see [`MatchConfig::pvp_end_screen`]
    early_close: Option<Instant>,
    /// PvP end screen: the `MatchEnd` went out while the match runs (since then); the relay's end makes no second one
    early_result: Option<Instant>,
    /// PvP end screen: the time-limit / empty-team close waits for the host's Outcome since then
    pvp_hold_since: Option<Instant>,
    /// phase 2 resync: the open WantDe (request id, when asked, the CC tick its DE must lie above)
    de_req: Option<(u32, Instant, u32)>,
    de_seq: u32,
    /// host DEs that failed (refused, lost, the relay's checks): at 3 resync is off for the match
    resync_failures: u32,
    /// protocol 5: the world host drives the bot slots (`Create.drive_bots`)
    bots_driven: bool,
    /// the host's bot ai entries by DF time (`Bots.t_for`): each goes into that DF once; ai entries never go late
    bots_ai_at: BTreeMap<u32, Vec<(u8, u16, u16)>>,
    /// the newest `Bots.t_for` received: DF t is complete for the bots once `bots_upto >= t`
    bots_upto: u32,
    /// the DF due now waits for its `Bots` since then
    bots_hold_since: Option<Instant>,
    /// the DF time the last hold was counted for
    bots_hold_counted: u32,
    /// the last DF that went to the host
    host_df_last: Option<u32>,
    /// the host stopped while driving the bots: one neutral input per bot went out
    bots_neutral_sent: bool,
    /// the DF that carries the PvP bots' spawn profile (enter + tick)
    bots_spawn_at: Option<u32>,
    bots_stats: BotsStats,
    /// rejoin: the whole DFs of the last [`REJOIN_HISTORY_MS`] (before the per-recipient filtering, no confirmations) with
    /// their count of server profiles: a rejoining client gets the ones after its DE
    history: VecDeque<(GameUpdate, usize)>,
    /// slots dropped / back in the match since the last `take_dropped` / `take_rejoined`
    dropped_events: Vec<(u8, LeaveReason)>,
    rejoined_events: Vec<u8>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CreateError {
    #[error("no world host and no stored world for map {0} mode {1}")]
    NoWorld(u32, u32),
    #[error("no world host and no stored world for project {0}")]
    NoProjectWorld(String),
    #[error("a world host builds the start world of a named project: the match has none")]
    NoProject,
    #[error("match id {0:#x} differs from the options' match id {1:#x}")]
    OptionsMatchId(u32, u32),
    #[error("match id {0:#x} is in use by a running match")]
    IdInUse(u32),
    #[error("a match needs 1..=20 players, at least one of them human, the humans first")]
    PlayerCount,
    #[error("options blob too large, or its header unreadable / tick 0")]
    Options,
    #[error("the world for this roster: {0}")]
    World(String),
}

/// The options blob's header (local_match options.rs: `u16 send_period, u16 max_delay, u8 tick_ms, u8 n_players,
/// u32 match_id`): (match id, tick ms, send period ms); None when it is shorter or the tick is 0.
pub fn options_header(options: &[u8]) -> Option<(u32, u8, u16)> {
    let h = options.get(..10)?;
    let tick = h[4];
    (tick != 0).then(|| (u32::from_le_bytes([h[6], h[7], h[8], h[9]]), tick, u16::from_le_bytes([h[0], h[1]])))
}

/// Two inputs of one player at one time, merged as the client merges them (0x2001B400F): the rotation
/// words added (wrapping), the action bits OR'ed.
pub fn merge_input(a: PlayerInput, b: PlayerInput) -> PlayerInput {
    PlayerInput { rot: [a.rot[0].wrapping_add(b.rot[0]), a.rot[1].wrapping_add(b.rot[1])], actions: a.actions | b.actions }
}

/// DF `df` as player `r` gets it, without confirmations: never a player's own inputs, late entries, profile or ping
/// entries back to him (the assert in change_input, 0x2001B4038); the first `n_server` profiles (the
/// server's start profiles) go to everybody.
fn df_for(df: &GameUpdate, r: usize, n_server: usize) -> GameUpdate {
    let mut mine = df.clone();
    let nl = mine.late.len();
    let mut inputs = Vec::with_capacity(mine.inputs.len());
    let mut late = Vec::with_capacity(nl);
    for (k, (dt, p)) in mine.late.iter().enumerate() {
        if usize::from(*p) != r {
            late.push((*dt, *p));
            inputs.push(mine.inputs[k]);
        }
    }
    let mut rank = 0;
    for p in 0..32usize {
        if mine.input_mask & (1 << p) != 0 {
            if p != r {
                inputs.push(mine.inputs[nl + rank]);
            }
            rank += 1;
        }
    }
    mine.input_mask &= !(1u32 << r);
    mine.late = late;
    mine.inputs = inputs;
    mine.profile = df.profile.iter().enumerate().filter(|(k, (p, _))| *k < n_server || usize::from(*p) != r).map(|(_, x)| x.clone()).collect();
    mine.ping.retain(|(p, _)| usize::from(*p) != r);
    mine.conf.clear();
    mine
}

fn send<T: Encode>(conn: &mut Conn, mid: u8, m: &T) {
    match m.to_bytes() {
        Ok(b) => {
            if let Err(e) = conn.t.send(mid, &b) {
                log::error!("send {mid:#x}: {e:?}");
            }
        }
        Err(e) => log::error!("encode {mid:#x}: {e}"),
    }
}

impl Match {
    pub(crate) fn new(
        spec: &MatchSpec,
        given: Option<Arc<World>>,
        match_id: u32,
        order_ids: &[u32],
        cfg: MatchConfig,
        now: Instant,
    ) -> Result<Self, CreateError> {
        // at least one human, and every bot after the humans (the options' record order: humans, then the script bots)
        let humans = spec.players.iter().take_while(|p| !p.bot).count();
        if spec.players.len() > MAX_PLAYERS || humans == 0 || spec.players[humans..].iter().any(|p| !p.bot) {
            return Err(CreateError::PlayerCount);
        }
        if spec.options.len() > 0x39C0C8 {
            return Err(CreateError::Options);
        }
        let (_, opt_tick, opt_send) = options_header(&spec.options).ok_or(CreateError::Options)?;
        // the start world: a given one (no host), or a placeholder until the host's Ready (protocol 4)
        let world_ready = given.is_some();
        let world = match given {
            Some(w) => w,
            None => {
                if cfg.world_host.is_none() {
                    return Err(CreateError::NoWorld(spec.map, spec.mode));
                }
                if spec.project.as_deref().unwrap_or("").is_empty() {
                    return Err(CreateError::NoProject);
                }
                let w = World::from_item(spec.map, spec.mode, match_id, 0, opt_tick, opt_send, vec![0; 22]).map_err(|e| CreateError::World(e.to_string()))?;
                Arc::new(w)
            }
        };
        let slots = spec
            .players
            .iter()
            .zip(order_ids)
            .map(|(p, o)| Slot {
                spec: p.clone(),
                order_id: *o,
                conn: None,
                dc_sent: None,
                probe_seen: false,
                dd_sent: false,
                // a server-owned slot is ready from the start (it has no client to load)
                ready: p.bot,
                profile: ProfileInput::default(),
                entered: false,
                left: None,
                leave_pending: false,
                late_refused: false,
                de_sent: None,
                de_tries: 0,
                got_cd: false,
                cd_seen: VecDeque::new(),
                conf_at: BTreeMap::new(),
                diag: SlotDiag::default(),
                last_cd: None,
                last_d: 0,
                last_cd_time: 0,
                last_cc: None,
                resync_want: None,
                resync_floor: 0,
                resync_times: VecDeque::new(),
                resync_off: false,
                rs: ResyncSlot::default(),
                dropped: None,
                rejoining: false,
            })
            .collect();
        let world_time0 = world.time;
        let tick = u32::from(world.tick_ms);
        // `match` runs in the first step after the DE and starts `wait_before_cancel` in the same step (index 2 > 1); a
        // host-built start world is set when its Ready comes ([`Match::on_start_world`])
        let pve = PveMirror { cancel_at: spec.pve_end.filter(|_| world_ready).map(|r| world_time0 + tick + r.cancel_ms), ..Default::default() };
        // every online match gets the world host when the switch is set: PvE (phase 1) and PvP (phase 3); without a given
        // world the host builds it (protocol 4: an empty Create.world)
        let host = match &cfg.world_host {
            Some(h) => match if world_ready { world.item() } else { Ok(Vec::new()) } {
                Ok(item) => {
                    let create = world_host_proto::Create {
                        version: world_host_proto::PROTO_VERSION,
                        match_id,
                        kind: if spec.pve_end.is_some() { world_host_proto::KIND_PVE } else { world_host_proto::KIND_PVP },
                        project: spec.project.clone().unwrap_or_default(),
                        map: spec.map,
                        mode: spec.mode,
                        tick_ms: world.tick_ms,
                        records: spec.players.iter().map(|p| world_host_proto::Record { bot: p.bot, team: p.team }).collect(),
                        options: spec.options.clone(),
                        world: item,
                        // protocol 5: the host drives the bot slots: a PvE mission's script bots,
                        // an online PvP match's AI players
                        drive_bots: spec.players.iter().any(|p| p.bot),
                        ai_ignore_players: spec.ai_ignore_players,
                    };
                    match crate::host::HostLink::new(h.clone(), &create, world.time) {
                        Ok(l) => Some(l),
                        Err(e) if world_ready => {
                            log::warn!("match {match_id:#x}: no world host: Create not encodable: {e}");
                            None
                        }
                        Err(e) => return Err(CreateError::World(format!("Create not encodable: {e}"))),
                    }
                }
                Err(e) => {
                    log::warn!("match {match_id:#x}: no world host: the world item: {e}");
                    None
                }
            },
            None => None,
        };
        let bots_driven = host.as_ref().is_some_and(|h| h.drives_bots());
        if bots_driven {
            log::info!("match {match_id:#x}: the world host drives the bot slots (protocol 5)");
        }
        let spec_players: Vec<(bool, u8)> = spec.players.iter().map(|p| (p.bot, p.team)).collect();
        let rec = if world_ready { cfg.record_dir.as_deref().and_then(|dir| open_recorder(dir, match_id, &spec_players, &spec.options, &world)) } else { None };
        if !world_ready {
            log::info!("match {match_id:#x}: the world host builds the start world of '{}'", spec.project.as_deref().unwrap_or(""));
        }
        Ok(Self {
            id: match_id,
            map: spec.map,
            mode: spec.mode,
            rec_dir: if world_ready { None } else { cfg.record_dir.clone() },
            cfg,
            rec,
            world_ready,
            start_stepped: false,
            spec_players,
            spec_options: spec.options.clone(),
            latest: world.clone(),
            options_lzma2: lzma2::compress(&spec.options),
            checksum: spec.level_checksum.unwrap_or(world.level_checksum),
            tick_ms: u32::from(world.tick_ms),
            next_tick: world.time + u32::from(world.tick_ms),
            world,
            created: now,
            phase: Phase::Waiting,
            slots,
            winner: None,
            player_stats: None,
            start: None,
            clock: world_time0,
            future: BTreeMap::new(),
            late_q: Vec::new(),
            late_at: BTreeMap::new(),
            profile_q: Vec::new(),
            cd_profile_at: BTreeMap::new(),
            ping_at: BTreeMap::new(),
            entered_at: None,
            enter_repeats_left: 0,
            diag_at: None,
            left_events: Vec::new(),
            enter_forced: false,
            end_rule: spec.end_rule,
            t_start: None,
            t_end: None,
            t_close: None,
            overtime_logged: false,
            team_counts: [(spec.players.len() >> 1) as u8; 2],
            pve_end: spec.pve_end,
            pve,
            end: None,
            result: None,
            stats: MatchStats { phase: "waiting", ..Default::default() },
            host,
            outcomes: Vec::new(),
            outcome_before_end: false,
            held: None,
            last_df: None,
            mirror_wait: None,
            fin_keeps_world: false,
            handed_over: false,
            result_sent: false,
            early_close: None,
            early_result: None,
            pvp_hold_since: None,
            de_req: None,
            de_seq: 0,
            resync_failures: 0,
            bots_driven,
            bots_ai_at: BTreeMap::new(),
            bots_upto: 0,
            bots_hold_since: None,
            bots_hold_counted: 0,
            host_df_last: None,
            bots_neutral_sent: false,
            bots_spawn_at: None,
            bots_stats: BotsStats::default(),
            history: VecDeque::new(),
            dropped_events: Vec::new(),
            rejoined_events: Vec::new(),
        })
    }

    pub fn match_id(&self) -> u32 {
        self.id
    }

    pub(crate) fn order_ids(&self) -> Vec<u32> {
        self.slots.iter().filter(|s| !s.spec.bot).map(|s| s.order_id).collect()
    }

    pub(crate) fn account_ids(&self) -> Vec<u64> {
        self.slots.iter().filter(|s| !s.spec.bot).map(|s| s.spec.account_id).collect()
    }

    /// Match time since the enter DF (the mission clock every client runs from the enter), None before the enter.
    pub fn played(&self) -> Option<Duration> {
        self.entered_at.map(|e| Duration::from_millis(u64::from(self.clock.saturating_sub(e))))
    }

    pub fn stats(&self) -> MatchStats {
        let mut s = self.stats.clone();
        s.phase = match self.phase {
            Phase::Waiting => "waiting",
            Phase::Running => "running",
            Phase::Ended => "ended",
        };
        s.ready = self.slots.iter().filter(|p| p.ready && !p.spec.bot).count();
        s.entered = self.slots.iter().filter(|p| p.entered && !p.spec.bot).count();
        s.left = self.slots.iter().filter(|p| p.left.is_some()).count();
        s.late_refused = self.slots.iter().filter(|p| p.late_refused).count();
        s.bots = self.slots.iter().filter(|p| p.entered && p.spec.bot).count();
        if let Some(h) = &self.host {
            s.host = h.state().name();
            s.host_restarts = h.restarts();
        }
        s
    }

    /// The world host's outcome of this match, once (see [`HostOutcome`]).
    pub(crate) fn take_outcomes(&mut self) -> Vec<HostOutcome> {
        std::mem::take(&mut self.outcomes)
    }

    /// A world host is attached and still used: its outcome can still come.
    pub fn host_active(&self) -> bool {
        self.host.as_ref().is_some_and(|h| h.active())
    }

    pub fn set_winner(&mut self, team: u8) {
        self.winner = Some(team);
    }

    /// Statistics of the finished (or running) match from a source that really knows them; reported in [`MatchEnd`].
    pub fn set_stats(&mut self, stats: FinalStats) {
        self.player_stats = Some(stats);
    }

    /// Replace the world used for CC answers (a fresher DE from a future referee client).
    pub fn set_latest_world(&mut self, w: Arc<World>) {
        self.latest = w;
    }

    pub(crate) fn take_result(&mut self) -> Option<MatchEnd> {
        self.result.take()
    }

    /// True when the match ended and the drain time is over.
    pub(crate) fn is_done(&self, now: Instant) -> bool {
        // a code-2 end (PvE result screen, PvP end screen): the client keeps its world and plays the closing DFs at its own
        // pace; its match connection times out 2 s after the last datagram it got (`client [match] disconnected by timeout`
        // in its log), and that timeout tears the world down (close_current_match 0). So the connections stay longer
        // (CHOICE: at least FIN_KEEPS_WORLD_DRAIN)
        let drain = if self.fin_keeps_world { self.cfg.drain.max(FIN_KEEPS_WORLD_DRAIN) } else { self.cfg.drain };
        self.held.is_none() && matches!(self.end, Some((_, at)) if now.saturating_duration_since(at) >= drain)
    }

    /// The clients are still in the match while the lobby already has its result (the PvP end screen's result before the
    /// close): the lobby keeps their `85 00` until the match is gone ([`crate::Hub::has_match`]): an `85 00` that reaches a
    /// client with its world still loaded unloads it (the 85 handler 0x200175590 in client state 0: world unload and
    /// lobby return; no victory splash, then a crash at the match connection's timeout).
    pub(crate) fn holds_clients(&self) -> bool {
        self.phase == Phase::Running && self.early_result.is_some()
    }

    /// Attach a player's connection after its C9 was seen (the hub parsed it). Returns false when the connection is
    /// refused (session mismatch, slot already taken by a live peer, match over).
    pub(crate) fn attach(&mut self, slot: u8, addr: SocketAddr, mut t: MatchTransport, c9: &Connect, now: Instant) -> bool {
        if self.phase == Phase::Ended {
            return false;
        }
        let s = &mut self.slots[usize::from(slot)];
        if s.spec.bot {
            return false;
        }
        if s.spec.session.is_some_and(|x| x != c9.session) {
            log::warn!("match {:#x} slot {slot}: C9 session mismatch", self.id);
            return false;
        }
        if s.left.is_some() {
            return false;
        }
        if s.conn.as_ref().is_some_and(|c| !c.t.is_gone(now)) {
            return false;
        }
        // rejoin: a new connection of an entered slot of a running match (the lobby's `82` sent the client back). A slot
        // whose old connection only went quiet is dropped first (no leave bit); then DD, CB, the host's DE, the DFs after it
        let i = usize::from(slot);
        // (not after the world's end: the PvP end screen closed the host, no DE of the running world can come)
        if self.phase == Phase::Running && self.slots[i].entered && self.early_close.is_none() {
            if self.slots[i].dropped.is_none() && self.can_drop(i) {
                self.drop_slot(slot, LeaveReason::TimedOut, now);
            }
            if self.slots[i].dropped.is_some() {
                let s = &mut self.slots[i];
                s.rejoining = true;
                s.ready = false;
                s.de_sent = None;
                s.resync_want = None;
                s.rs = ResyncSlot::default();
                log::warn!("match {:#x} slot {slot}: rejoin: its client connects again from {addr} (C9)", self.id);
            }
        }
        let s = &mut self.slots[i];
        t.set_send_period(Duration::from_millis(u64::from(self.world.send_period_ms)));
        let mut conn = Conn { addr, t };
        // the first S->C message must be DC (PROVEN), min = max = our port: the hub fills in the port via `ports`
        s.dc_sent = None;
        s.probe_seen = false;
        s.dd_sent = false;
        conn.t.set_send_period(Duration::from_millis(u64::from(self.world.send_period_ms)));
        s.conn = Some(conn);
        true
    }

    /// DC needs the server port: the hub calls this right after `attach`.
    pub(crate) fn send_ports(&mut self, slot: u8, port: u16) {
        let s = &mut self.slots[usize::from(slot)];
        if let Some(c) = s.conn.as_mut() {
            if let Ok(b) = (Ports { min: port, max: port }).to_bytes() {
                s.dc_sent = c.t.send_tracked(id::DC_PORTS, &b).ok();
            }
        }
    }

    pub(crate) fn on_datagram(&mut self, slot: u8, d: &[u8], now: Instant) {
        let Some(conn) = self.slots[usize::from(slot)].conn.as_mut() else { return };
        let events = conn.t.receive(d, now);
        for ev in events {
            self.on_event(slot, ev, now);
        }
    }

    /// Events of a datagram the hub already fed to the transport (the one that carried C9).
    pub(crate) fn on_events(&mut self, slot: u8, events: Vec<Event>, now: Instant) {
        for ev in events {
            self.on_event(slot, ev, now);
        }
    }

    fn on_event(&mut self, slot: u8, ev: Event, now: Instant) {
        match ev {
            Event::Disconnected => self.lose(slot, LeaveReason::Disconnected, now),
            Event::Message { id: mid, payload, .. } => match mid {
                id::C9_CONNECT => {} // duplicate (plain, reliable, resent every 500 ms until acked)
                id::CA_PORT_PROBE => self.slots[usize::from(slot)].probe_seen = true,
                id::CB_READY => match Ready::from_bytes(&payload) {
                    Ok(r) => self.on_ready(slot, r),
                    Err(e) => log::warn!("match {:#x} slot {slot}: bad CB: {e}", self.id),
                },
                id::CC_QUERY_DE => match QueryWorld::from_bytes(&payload) {
                    Ok(q) => self.on_query(slot, q, now),
                    Err(e) => log::warn!("match {:#x} slot {slot}: bad CC: {e}", self.id),
                },
                id::CD_CLIENT_UPDATE => match ClientUpdate::from_bytes(&payload) {
                    Ok(cd) => self.on_client_update(slot, cd, now),
                    Err(e) => log::warn!("match {:#x} slot {slot}: bad CD: {e}", self.id),
                },
                id::CE_LEAVE => self.player_left(slot, LeaveReason::Menu),
                // CF net stats, D0 settings, D1, D2/D3 BattlEye / voice, D4 integrity: not used
                _ => {}
            },
        }
    }

    fn on_ready(&mut self, slot: u8, r: Ready) {
        let phase = self.phase;
        let last = self.last_df.unwrap_or(self.next_tick);
        let s = &mut self.slots[usize::from(slot)];
        if s.ready || s.left.is_some() {
            return; // CB is resent until acked
        }
        s.ready = true;
        if s.rejoining && phase == Phase::Running {
            // rejoin: the level is loaded; the running world comes as the host's DE of a time above every DF that may still
            // hold an input of the old connection (the drop's neutral input included), then the DFs after it. The CB's
            // profile is not used: the world holds the player's kit.
            s.resync_want = Some(last);
            log::warn!("match {:#x} slot {slot}: rejoin: level loaded (CB); waiting for the world host's DE above {last}", self.id);
            return;
        }
        s.profile = r.profile;
        if phase != Phase::Waiting {
            // no world source for a late joiner (needs a fresh DE of the running world): refused
            s.late_refused = true;
            log::warn!("match {:#x} slot {slot}: CB after the start, late join refused", self.id);
        }
    }

    fn on_query(&mut self, slot: u8, q: QueryWorld, now: Instant) {
        self.stats.cc_received += 1;
        if self.e0_rules() {
            // phase 2, resync off: a CC is the client's report of an E0 mismatch, logged, never answered (a cached first
            // world is ignored by a synced client; a live-world DE needs the animation tail). The tick from tick_lo and
            // the relay clock as the client rebuilds E0 times (0x2001EE2B4):
            // present_tick + sdiff8(tick_lo, present_tick & 0xFF)
            let pt = self.clock / self.tick_ms;
            let d = q.tick.wrapping_sub((pt & 0xFF) as u8) as i8;
            let t = (pt as i64 + i64::from(d)).max(0) as u32 * self.tick_ms;
            // one value, not bit flags (CC sender 0x2001AE98F): 4 / 2 / 8 are mismatches (8 always with a
            // mask-0 E0; with the host's mask a g0 / g1 mismatch is 4), 1 = forced after a DE that did not land
            let why = QueryWorld::reason_name(q.reason);
            if self.resync_rules() {
                self.on_resync_cc(slot, t, &q, why, now);
                return;
            }
            self.stats.cc_logged += 1;
            log::warn!(
                "match {:#x} slot {slot}: CC for tick {t} (lo {}): players differing {:#08x}, global {:#04x} (0xff = g0 or g1 differ), reason {} ({why}); resync is off: not answered",
                self.id,
                q.tick,
                q.players,
                q.global,
                q.reason
            );
            return;
        }
        log::warn!(
            "match {:#x} slot {slot}: CC (tick {}, players {:#x}, global {}, reason {}): resync requested",
            self.id,
            q.tick,
            q.players,
            q.global,
            q.reason
        );
        let gap = self.cfg.cc_gap;
        let s = &mut self.slots[usize::from(slot)];
        if !s.entered || s.left.is_some() || s.dropped.is_some() || s.last_cc.is_some_and(|t| now.saturating_duration_since(t) < gap) {
            return;
        }
        s.last_cc = Some(now);
        // LIKELY answer: a DE. The cached world is older than the client's agreed time unless `set_latest_world` gave a
        // fresher one, in which case the client ignores it ("ignoring resync request"); harmless either way.
        self.send_de(slot, true, now);
    }

    fn on_client_update(&mut self, slot: u8, cd: ClientUpdate, now: Instant) {
        if self.phase != Phase::Running {
            return;
        }
        let tick = self.tick_ms;
        let t0 = self.world.time;
        // the match clock at arrival, in the world's time base (the `b` of the confirmation)
        let arrived = self.start.map_or(t0, |s| t0.saturating_add(now.saturating_duration_since(s).as_millis() as u32));
        let next = self.next_tick;
        // a CD stamped at or before the enter DF is a pre-enter CD (stamped at a DF time): confirmed as on time (b = a - 2,
        // servo sample 0), so its client clock is not pushed ahead before the enter (0x2001AF6D0)
        let pre = self.entered_at.is_none_or(|e| cd.time <= e);
        let s = &mut self.slots[usize::from(slot)];
        if !s.entered || s.left.is_some() || s.dropped.is_some() {
            return;
        }
        s.got_cd = true;
        s.last_cd = Some(now);
        // CD is plain with a lifetime: the client resends it until acked, so the same time can arrive twice
        if s.cd_seen.contains(&cd.time) {
            self.stats.cd_duplicates += 1;
            return;
        }
        if s.cd_seen.len() >= 64 {
            s.cd_seen.pop_front();
        }
        s.cd_seen.push_back(cd.time);
        let grid = |t: u32| (t >= t0).then(|| t - (t - t0) % tick);
        // the DF this CD lands in: its own time when that DF has not gone out yet, else the next DF. Its inputs, profile
        // and ping entries and its confirmation all go there: the client matches its pending tick only against the
        // confirmations of the DF it handles and moves its own input to that DF's time (0x2001B02F4, 0x2001B2650)
        let d = match grid(cd.time) {
            Some(a) if a >= next => a,
            _ => next,
        }
        .max(s.last_d);
        if cd.time < s.last_cd_time && s.diag.cds < 200 {
            log::info!("match {:#x} slot {slot}: CD {} after CD {} (out of order), filed in DF {d}", self.id, cd.time, s.last_cd_time);
        }
        s.last_d = d;
        s.last_cd_time = s.last_cd_time.max(cd.time);
        let b = if pre { cd.time.saturating_sub(2) } else { arrived };
        s.diag.cds += 1;
        s.diag.now += cd.now.len() as u32;
        s.diag.late += cd.late.len() as u32;
        s.diag.profile += cd.profile.len() as u32;
        s.diag.ping += cd.ping.len() as u32;
        if !pre {
            let lead = (i64::from(cd.time) - i64::from(arrived)).clamp(-100_000, 100_000) as i32;
            if s.diag.lead_n == 0 {
                (s.diag.lead_min, s.diag.lead_max) = (lead, lead);
            }
            s.diag.lead_sum += i64::from(lead);
            s.diag.lead_n += 1;
            s.diag.lead_min = s.diag.lead_min.min(lead);
            s.diag.lead_max = s.diag.lead_max.max(lead);
        }
        if s.diag.cds <= 6 || (!cd.now.is_empty() && s.diag.now <= 3) {
            log::info!(
                "match {:#x} slot {slot}: CD #{} time {} now {} late {:?} profile {} ping {:?} -> DF {d} (clock {}, next tick {next}, pre-enter {pre})",
                self.id,
                s.diag.cds,
                cd.time,
                cd.now.len(),
                cd.late.iter().map(|(dt, _)| *dt).collect::<Vec<_>>(),
                cd.profile.len(),
                cd.ping,
                arrived
            );
        }
        let q = s.conf_at.entry(d).or_default();
        if q.len() < 64 {
            q.push((cd.time, b));
        }
        if cd.time > t0 {
            if d > self.clock.saturating_add(2000) {
                log::warn!("match {:#x} slot {slot}: CD stamped {} is far ahead of the match clock {}", self.id, cd.time, self.clock);
            } else {
                // two inputs of one player at one time are merged as the client merges them (rotation words added,
                // action bits OR'ed: 0x2001B400F), never replaced
                for i in &cd.now {
                    let v = self.future.entry(d).or_default();
                    match v.iter_mut().find(|(p, _)| *p == slot) {
                        Some((_, old)) => *old = merge_input(*old, *i),
                        None => v.push((slot, *i)),
                    }
                }
                // precise-time inputs (fire changes off the tick grid, dt 0..49 ms before the CD time): DF d's late
                // list with the CD's own dt; the sender's confirmation moves them to d - dt (0x2001B2650)
                for (dt, i) in &cd.late {
                    let v = self.late_at.entry(d).or_default();
                    match v.iter_mut().find(|(x, p, _)| *x == *dt && *p == slot) {
                        Some((_, _, old)) => *old = merge_input(*old, *i),
                        None => v.push((*dt, slot, *i)),
                    }
                }
            }
        }
        // ping and profile entries of a player at one time: the newer wins
        for p in cd.profile {
            let v = self.cd_profile_at.entry(d).or_default();
            v.retain(|(x, _)| *x != slot);
            v.push((slot, p));
        }
        if let Some(p) = cd.ping.last() {
            let v = self.ping_at.entry(d).or_default();
            v.retain(|(x, _)| *x != slot);
            v.push((slot, *p));
        }
    }

    /// The enter at DF `t` of the slots `live`: when it brings the entered count to the timelimit rule's wait threshold,
    /// the clients' countdown starts there (0x2003663FB) and the match ends at T_start + match_time.
    fn arm_time_limit(&mut self, t: u32, live: &[usize]) {
        let Some(r) = self.end_rule else { return };
        let humans = self.slots.iter().filter(|s| !s.spec.bot).count() as f32;
        let bots = self.slots.iter().filter(|s| s.spec.bot).count() as u32;
        let k = (humans * r.wait_ratio + 0.5).floor() as u32 + bots;
        if (live.len() as u32) < k || r.match_ms == 0 {
            log::info!("match {:#x}: no timed end (entered {} < wait threshold {k}, or no time limit)", self.id, live.len());
            return;
        }
        let start = t.saturating_add(r.countdown_ms);
        self.t_start = Some(start);
        self.t_end = Some(start.saturating_add(r.match_ms));
        self.t_close = Some(start.saturating_add(r.match_ms).saturating_add(3000));
        log::info!("match {:#x}: countdown from DF {t}, match started at {start}, time-limit end {} (+3 s close)", self.id, start + r.match_ms);
    }

    /// A leave applied by the clients at DF `t`: their timelimit rule lowers the leaver's team count (u8) and cuts the
    /// remaining time to advantage_time when a team is at the leave threshold or below (on_leave 0x200365D00).
    fn mirror_leave(&mut self, t: u32, slot: usize) {
        let (Some(r), Some(start), Some(end)) = (self.end_rule, self.t_start, self.t_end) else { return };
        let team = usize::from(self.slots[slot].spec.team.min(1));
        if r.team_empty_end {
            // tdm on_leave (0x200366FA0) recounts the entered players per team; a team with none ends the match at
            // t + 3000 (0x200366DF3) and the other team wins
            let alive = |k: u8| self.slots.iter().filter(|s| s.spec.team.min(1) == k && s.entered && s.left.is_none()).count();
            let (a, b) = (alive(0), alive(1));
            if (a == 0) != (b == 0) {
                let winner = if a == 0 { 1 } else { 0 };
                let close = t.saturating_add(3000);
                if self.t_close.is_none_or(|c| close < c) {
                    self.t_close = Some(close);
                    self.winner = Some(winner);
                    log::info!("match {:#x}: slot {slot} left at {t}, team {} has nobody left: end {close}, team {winner} wins", self.id, 1 - winner);
                }
            }
        }
        self.team_counts[team] = self.team_counts[team].wrapping_sub(1);
        let humans = self.slots.iter().filter(|s| !s.spec.bot).count() as f32;
        let bots = self.slots.iter().filter(|s| s.spec.bot).count() as f32;
        let l = (humans * 0.5 * (1.0 - r.leave_ratio) + bots / 2.0 + 0.5).floor() as u8;
        if self.team_counts[team] <= l {
            let cut = t.max(start + self.tick_ms).saturating_add(r.advantage_ms);
            if cut < end {
                self.t_end = Some(cut);
                if self.t_close.is_none_or(|c| cut.saturating_add(3000) < c) {
                    self.t_close = Some(cut.saturating_add(3000));
                }
                log::info!("match {:#x}: slot {slot} left at {t}: team {team} at {} <= {l}, advantage end {cut}", self.id, self.team_counts[team]);
            }
        }
    }

    /// The enter DF `t` of a PvE mission ([`PveEndRule`]): in the step of `t` the script picks its start wait (all humans
    /// entered, or 2 of 3), which cancels the pre-start cancel; after the wait the 45-minute mission clock starts.
    fn arm_pve(&mut self, t: u32, live: &[usize]) {
        let Some(r) = self.pve_end else { return };
        let humans = self.slots.iter().filter(|s| !s.spec.bot).count() as u32;
        let entered: Vec<usize> = live.iter().copied().filter(|&i| !self.slots[i].spec.bot).collect();
        self.pve.entered = entered.iter().fold(0u32, |m, &i| m | 1 << i);
        self.pve.entered_n = entered.len() as u32;
        // on_player_entered_before_match_started: all_players_ready when every human entered; 2_from_3_players_ready when
        // a 3-human mission has 2 entered and none left (no leave can come before the enter DF)
        let wait = if self.pve.entered_n == humans {
            Some(r.start_all_ms)
        } else if humans == 3 && self.pve.entered_n == 2 {
            Some(r.start_two_of_three_ms)
        } else {
            None
        };
        match wait {
            Some(w) => {
                let start = t.saturating_add(w);
                self.pve.cancel_at = None;
                self.pve.start_at = Some(start);
                self.pve.timeout_at = Some(start.saturating_add(r.match_ms));
                log::info!(
                    "match {:#x}: PvE enter of {}/{humans} human(s) in DF {t}: mission start at {start}, the script's 45-minute end (failure) at {}",
                    self.id,
                    self.pve.entered_n,
                    start + r.match_ms
                );
            }
            None => log::info!(
                "match {:#x}: PvE enter of {}/{humans} human(s) in DF {t}: no start, the script's pre-start cancel stays at {:?}",
                self.id,
                self.pve.entered_n,
                self.pve.cancel_at
            ),
        }
    }

    /// A leave applied by the clients at DF `t` in a PvE mission (only a human of the enter DF counts). Before the start
    /// (a leave in the start's own step is still "before"): the cancel at `t` when at most one human is left, or the
    /// 5-minute cancel from the next step for a 3-human mission with 2 entered and 1 left. After the start only "every
    /// human left" is computable (the relay's AllLeft).
    fn pve_leave(&mut self, t: u32, slot: usize) {
        let Some(r) = self.pve_end else { return };
        if self.pve.entered & (1 << slot) == 0 {
            return;
        }
        self.pve.left_n += 1;
        let humans = self.slots.iter().filter(|s| !s.spec.bot).count() as u32;
        if self.pve.start_at.is_some_and(|s| t > s) {
            log::info!("match {:#x}: PvE slot {slot} left at {t}, after the start: no script end without the world", self.id);
            return;
        }
        if self.pve.left_n + 1 >= humans {
            self.pve.cancel_at = Some(self.pve.cancel_at.map_or(t, |c| c.min(t)));
            log::info!("match {:#x}: PvE slot {slot} left at {t} before the start, {} of {humans} left: the script cancels at {t}", self.id, self.pve.left_n);
        } else if humans == 3 && self.pve.entered_n == 2 && self.pve.left_n == 1 {
            // wait_before_cancel2's first action disables 2_from_3_players_ready: no start, no 45-minute end
            let at = t.saturating_add(self.tick_ms).saturating_add(r.cancel2_ms);
            self.pve.cancel_at = Some(at);
            self.pve.start_at = None;
            self.pve.timeout_at = None;
            log::info!("match {:#x}: PvE slot {slot} left at {t} before the start (2 of 3 entered): the script cancels at {at}", self.id);
        } else {
            log::info!("match {:#x}: PvE slot {slot} left at {t} before the start, {} of {humans} left: no script end", self.id, self.pve.left_n);
        }
    }

    /// A mirrored PvE failure end (pre-start cancel, 45-minute end) with a live world host: the closing DF waits for the
    /// host's Outcome, at most `HostConfig::mirror_wait`, so the lobby's `99` / `83 45 0B` reach the clients while their
    /// match is still loaded. The DFs go on meanwhile (the host needs DF `c + 300` to read tick `c` as final). Once the
    /// Outcome came, the lobby owns the end. True = do not close now. "Every human left" is not held (nobody sees a
    /// screen; `humans_in`).
    ///
    /// Hand-over: without a host Outcome the relay does not close at its mirrored end either. It hands the end itself to the lobby (`MatchConfig::pve_handover`): a [`HostOutcome`]
    /// with the script's failure (state 4 / 5, winner 1, no counters), so the lobby's `99` (success 0) and `83 45 0B`
    /// reach the client while its match is loaded; the lobby then ends the match and the closing DF carries code 2
    /// (`fin_keeps_world`). A backstop closes it when no lobby end comes.
    /// The time-limit close at `c` of a mode with a draw overtime ([`EndRule::draw_overtime_ms`]): while a live world host
    /// has sent no Outcome, the clients' worlds may still be in their draw time, so the relay keeps the match running
    /// until the host's Outcome (the PvP end in `host_poll`) or `c + draw_overtime_ms + 5 s`. True = do not close yet.
    fn overtime_hold(&mut self, c: u32) -> bool {
        let Some(r) = self.end_rule else { return false };
        if r.draw_overtime_ms == 0 || self.outcome_before_end || self.pve_end.is_some() {
            return false;
        }
        let live = self.host.as_ref().is_some_and(|h| h.state() == crate::host::LinkState::Live && !h.diverged());
        if !live {
            return false;
        }
        let cap = c.saturating_add(r.draw_overtime_ms).saturating_add(5000);
        if self.next_tick > cap {
            log::warn!("match {:#x}: no world host Outcome by {cap} (time end {c} + draw time {} ms): the relay closes", self.id, r.draw_overtime_ms);
            return false;
        }
        if !self.overtime_logged {
            self.overtime_logged = true;
            log::info!("match {:#x}: time end at {c}: the world may be in its draw time (up to {} ms); waiting for the host's Outcome", self.id, r.draw_overtime_ms);
        }
        true
    }

    fn mirror_hold(&mut self, c: u32, reason: EndReason, now: Instant) -> bool {
        let live = self.host.as_ref().filter(|h| h.state() == crate::host::LinkState::Live).map(|h| h.mirror_wait());
        let id = self.id;
        let since = *self.mirror_wait.get_or_insert_with(|| {
            log::info!("match {id:#x}: the mission script's failure end ({reason:?}) at {c}: the clients' worlds ended (host live: {})", live.is_some());
            now
        });
        let waited = now.saturating_duration_since(since);
        let host_wait = live.unwrap_or(Duration::ZERO);
        if !self.outcome_before_end {
            if waited < host_wait {
                return true; // the host's Outcome for tick c needs DF c + 300
            }
            let Some(_) = self.cfg.pve_handover else {
                if live.is_some() {
                    log::warn!("match {:#x}: no world host Outcome within {host_wait:?} of the mirrored end ({reason:?} at {c}): the relay closes", self.id);
                }
                return false;
            };
            self.hand_over(c, reason);
        }
        // handed over (the host's Outcome or the relay's own): the lobby ends the match; the backstop when it does not
        let cap = host_wait + self.cfg.pve_handover.unwrap_or(Duration::from_secs(10));
        if waited < cap {
            return true;
        }
        log::warn!("match {:#x}: the mission ended at {c} ({reason:?}) and no lobby end came within {cap:?}: the relay closes", self.id);
        false
    }

    /// The relay's own PvE failure end as the lobby's result source (see [`Match::mirror_hold`]): what the mirror knows,
    /// the end step and the script's winner_team 1; no counters (world state no client reports, never invented).
    fn hand_over(&mut self, c: u32, reason: EndReason) {
        self.outcome_before_end = true;
        self.handed_over = true;
        self.winner = Some(1);
        let state = if reason == EndReason::PveCancel { 4 } else { 5 };
        let mission_ms = self.entered_at.map_or(0, |e| c.saturating_sub(e));
        log::info!("match {:#x}: the mirrored failure end ({reason:?} at {c}, state {state}) goes to the lobby as the result; the clients stay in their ended world", self.id);
        self.outcomes.push(HostOutcome {
            match_id: self.id,
            t_end: c,
            state,
            winner_team: 1,
            pve_success: false,
            mission_ms,
            players: Vec::new(),
            team_scores: [0, 0],
            details: Vec::new(),
        });
    }

    /// The next PvE script end the relay mirrors: (world step of its action 26, reason).
    fn pve_close(&self) -> Option<(u32, EndReason)> {
        let cancel = self.pve.cancel_at.map(|c| (c, EndReason::PveCancel));
        let timeout = self.pve.timeout_at.map(|c| (c, EndReason::PveTimeout));
        match (cancel, timeout) {
            (Some(a), Some(b)) => Some(if b.0 < a.0 { b } else { a }),
            (a, b) => a.or(b),
        }
    }

    /// The server's own hard limit after the start: for a PvE mission beyond the script's latest end (a start at most
    /// `enter_wait` + 60 s after the match start, then 45 min), so the backstop never cuts a mission short.
    fn backstop(&self) -> Duration {
        match self.pve_end {
            Some(r) => {
                let script = u64::from((r.start_all_ms.max(r.start_two_of_three_ms) + r.match_ms).max(r.cancel_ms));
                self.cfg.max_duration.max(Duration::from_millis(script) + self.cfg.enter_wait + Duration::from_secs(60))
            }
            None => self.cfg.max_duration,
        }
    }

    /// Has every entered, present human's client sent a CD (it processed the DE and answers DFs)? Bots have no client.
    fn all_synced(&self) -> bool {
        let mut live = self.slots.iter().filter(|s| s.entered && s.left.is_none() && !s.spec.bot).peekable();
        live.peek().is_some() && live.all(|s| s.got_cd)
    }

    /// An entered human is still in the match (bots alone do not keep a match running).
    fn humans_in(&self) -> bool {
        self.slots.iter().any(|s| s.entered && s.left.is_none() && !s.spec.bot)
    }

    fn player_left(&mut self, slot: u8, why: LeaveReason) {
        let s = &mut self.slots[usize::from(slot)];
        if s.left.is_some() {
            return;
        }
        s.left = Some(why);
        if s.entered {
            s.leave_pending = true;
            self.left_events.push((slot, why));
        }
        log::info!("match {:#x} slot {slot}: left ({why:?})", self.id);
    }

    /// Players who left since the last call: (account, why). The lobby releases them at once.
    pub(crate) fn take_left(&mut self) -> Vec<(u64, LeaveReason)> {
        std::mem::take(&mut self.left_events).into_iter().map(|(slot, why)| (self.slots[usize::from(slot)].spec.account_id, why)).collect()
    }

    /// Rejoin: humans dropped with their slot kept since the last call (account, how). The lobby keeps them in the match
    /// and sends them back with `82`.
    pub(crate) fn take_dropped(&mut self) -> Vec<(u64, LeaveReason)> {
        std::mem::take(&mut self.dropped_events).into_iter().map(|(slot, why)| (self.slots[usize::from(slot)].spec.account_id, why)).collect()
    }

    /// Rejoin: dropped humans whose client is back in the match since the last call (accounts).
    pub(crate) fn take_rejoined(&mut self) -> Vec<u64> {
        std::mem::take(&mut self.rejoined_events).into_iter().map(|slot| self.slots[usize::from(slot)].spec.account_id).collect()
    }

    /// Rejoin: the account's slot is dropped and may still come back (its `82` may go out); `session` replaces the
    /// slot's login session when the player signed in again (C9 carries the new one). false = no rejoin.
    pub(crate) fn allow_rejoin(&mut self, account: u64, session: Option<u32>) -> bool {
        let id = self.id;
        let ok = self.phase == Phase::Running && self.host_active() && self.early_close.is_none();
        let Some((i, s)) = self.slots.iter_mut().enumerate().find(|(_, s)| !s.spec.bot && s.spec.account_id == account) else { return false };
        if !ok || s.left.is_some() || s.dropped.is_none() {
            return false;
        }
        if let (Some(old), Some(new)) = (s.spec.session, session) {
            if old != new {
                log::info!("match {id:#x} slot {i}: rejoin: login session {old:#x} -> {new:#x} (signed in again)");
                s.spec.session = Some(new);
            }
        }
        true
    }

    /// The lobby takes a dropped player out for good (he declined the rejoin): a leave bit, his drop as the reason.
    pub(crate) fn leave_dropped(&mut self, account: u64) -> bool {
        let Some(i) = self.slots.iter().position(|s| !s.spec.bot && s.spec.account_id == account && s.left.is_none()) else { return false };
        let Some((_, why)) = self.slots[i].dropped else { return false };
        log::info!("match {:#x} slot {i}: rejoin declined: the slot leaves", self.id);
        self.slots[i].conn = None;
        self.player_left(i as u8, why);
        true
    }

    /// The slot has a connection (the hub routes a datagram of a dropped slot's old address as a new peer: a C9).
    pub(crate) fn has_conn(&self, slot: u8) -> bool {
        self.slots.get(usize::from(slot)).is_some_and(|s| s.conn.is_some())
    }

    /// Rejoin applies to this slot now: switched on, a running match with a live world host (the only source of the
    /// running world's DE), an entered human still in the match.
    fn can_drop(&self, i: usize) -> bool {
        let s = &self.slots[i];
        self.cfg.rejoin && self.phase == Phase::Running && self.host_active() && self.early_close.is_none() && s.entered && !s.spec.bot && s.left.is_none()
    }

    /// A human's client is gone without the menu leave (silence, transport timeout, disconnect): dropped with his slot
    /// kept when rejoin applies, else he left.
    fn lose(&mut self, slot: u8, why: LeaveReason, now: Instant) {
        if self.early_close.is_some() {
            // the world ended (PvP end screen, the host is closed): this client played the match to its end (its own
            // `close_victory_screen` returns it to the lobby, or it crashed on the end screen). Out of the DFs, but no leave
            // event: the lobby pays it as a finisher. A dropped slot stays dropped (its drop is its leave at the end).
            let id = self.id;
            let s = &mut self.slots[usize::from(slot)];
            s.conn = None;
            s.rejoining = false;
            if s.left.is_none() && s.dropped.is_none() {
                s.left = Some(why);
                s.leave_pending = s.entered;
                log::info!("match {id:#x} slot {slot}: gone after the world's end ({why:?}): out of the DFs, no leave event (it finished the match)");
            }
            return;
        }
        if self.can_drop(usize::from(slot)) {
            self.drop_slot(slot, why, now);
        } else {
            self.player_left(slot, why);
        }
    }

    /// Rejoin: the slot stays in the world (no leave bit) and gets nothing until its client is back. Its pending inputs
    /// are dropped and one neutral input goes into the next DF: every client keeps a player's last action mask (as for
    /// the host's bots when the host stops), so a running or firing player would keep doing it. A rejoining slot whose new connection is lost too goes back to waiting.
    fn drop_slot(&mut self, slot: u8, why: LeaveReason, now: Instant) {
        let i = usize::from(slot);
        let s = &mut self.slots[i];
        if s.left.is_some() {
            return;
        }
        s.conn = None;
        s.resync_want = None;
        s.rs = ResyncSlot::default();
        s.resync_off = false;
        s.resync_times.clear();
        s.conf_at.clear();
        if s.dropped.is_some() {
            if std::mem::replace(&mut s.rejoining, false) {
                log::warn!("match {:#x} slot {slot}: rejoin: the new connection is lost too ({why:?}); the slot waits again", self.id);
            }
            return;
        }
        s.dropped = Some((now, why));
        s.rejoining = false;
        s.last_cd = None;
        s.cd_seen.clear();
        for v in self.future.values_mut() {
            v.retain(|(p, _)| *p != slot);
        }
        for v in self.late_at.values_mut() {
            v.retain(|(_, p, _)| *p != slot);
        }
        for v in self.cd_profile_at.values_mut() {
            v.retain(|(p, _)| *p != slot);
        }
        for v in self.ping_at.values_mut() {
            v.retain(|(p, _)| *p != slot);
        }
        self.late_q.retain(|(_, p, _)| *p != slot);
        let t = self.next_tick;
        self.future.entry(t).or_default().push((slot, PlayerInput::default()));
        self.stats.dropped += 1;
        self.dropped_events.push((slot, why));
        log::warn!(
            "match {:#x} slot {slot}: dropped ({why:?}): the slot is kept for a rejoin (no leave bit, a neutral input in DF {t}); every human gone for {:?} ends the match",
            self.id,
            self.cfg.rejoin_grace
        );
    }

    /// Rejoin bookkeeping of a running match: without a live world host no rejoin can complete (dropped slots leave);
    /// when every entered human has been dropped for `rejoin_grace` (and none is reconnecting), they leave for good.
    fn rejoin_watch(&mut self, now: Instant) {
        let dropped: Vec<usize> = (0..self.slots.len()).filter(|&i| self.slots[i].left.is_none() && self.slots[i].dropped.is_some()).collect();
        // after the world's end (PvP end screen) a dropped slot stays dropped until the match closes: its drop is its leave
        // in the result (the lobby's zero 97, then 85 00), never a leave event racing the result
        if dropped.is_empty() || self.early_close.is_some() {
            return;
        }
        let why = if !self.host_active() {
            "the world host is gone: no rejoin can complete"
        } else {
            let humans = self.slots.iter().filter(|s| s.entered && !s.spec.bot && s.left.is_none());
            let mut all_away = true;
            let mut newest: Option<Instant> = None;
            for s in humans {
                match s.dropped {
                    Some((at, _)) if !s.rejoining => newest = newest.max(Some(at)),
                    _ => all_away = false,
                }
            }
            if !(all_away && newest.is_some_and(|at| now.saturating_duration_since(at) >= self.cfg.rejoin_grace)) {
                return;
            }
            "every human has been dropped for the rejoin grace"
        };
        for i in dropped {
            let (_, how) = self.slots[i].dropped.expect("dropped");
            log::warn!("match {:#x} slot {i}: rejoin over ({why}): the slot leaves ({how:?})", self.id);
            self.slots[i].conn = None;
            self.player_left(i as u8, how);
        }
    }

    /// A rejoining client waits for the host's DE.
    fn rejoin_waiting(&self) -> bool {
        self.host_active() && self.early_close.is_none() && self.slots.iter().any(|s| s.rejoining && s.resync_want.is_some() && s.left.is_none())
    }

    /// Rejoin: every part of the DE at `t` is acked: the DFs after it from the history, then the slot plays again (its
    /// CDs count, it gets every DF, E0 above the newest DF). A history that does not reach back to `t` asks again.
    fn resume(&mut self, i: usize, t: u32, now: Instant) {
        let tick = self.tick_ms;
        let last = self.last_df.unwrap_or(t);
        let hist: Vec<(GameUpdate, usize)> = self.history.iter().filter(|(d, _)| d.time > t).cloned().collect();
        let want = last.saturating_sub(t) / tick;
        let complete = hist.len() as u32 == want && hist.first().is_none_or(|(d, _)| d.time == t + tick);
        if !complete {
            self.stats.rejoin_retries += 1;
            self.slots[i].resync_want = Some(last);
            log::error!("match {:#x} slot {i}: rejoin: the DF history does not cover {t}..{last} ({} of {want} DFs): asking for a newer DE", self.id, hist.len());
            return;
        }
        let id = self.id;
        let s = &mut self.slots[i];
        if let Some(c) = s.conn.as_mut() {
            for (d, n) in &hist {
                send(c, id::DF_GAME_UPDATE, &df_for(d, i, *n));
            }
        }
        s.rejoining = false;
        s.dropped = None;
        s.last_cd = Some(now);
        self.stats.rejoined += 1;
        self.rejoined_events.push(i as u8);
        log::warn!("match {id:#x} slot {i}: rejoin: the DE at {t} is acked; {} DF(s) after it sent ({}..={last}); the slot plays again", hist.len(), t + tick);
    }

    /// Run the timers and state machine, then collect the datagrams to send.
    pub fn poll(&mut self, now: Instant) -> Vec<(SocketAddr, Vec<u8>)> {
        self.advance(now);
        // the host's frames are applied after this poll's DFs were built: no client traffic waits for the host
        self.host_poll(now);
        self.resync_step(now);
        let mut out = Vec::new();
        for s in &mut self.slots {
            if let Some(c) = s.conn.as_mut() {
                for d in c.t.poll(now) {
                    out.push((c.addr, d));
                }
            }
        }
        out
    }

    fn advance(&mut self, now: Instant) {
        if self.phase == Phase::Ended {
            return;
        }
        for i in 0..self.slots.len() {
            let gone = self.slots[i].left.is_none() && self.slots[i].conn.as_ref().is_some_and(|c| c.t.is_gone(now));
            if gone {
                self.lose(i as u8, LeaveReason::TimedOut, now);
            }
        }
        // DD once the client took DC (state 1): DC acked, or its first CA arrived
        for i in 0..self.slots.len() {
            let s = &mut self.slots[i];
            if s.dd_sent || s.left.is_some() || !self.world_ready {
                continue;
            }
            let Some(c) = s.conn.as_mut() else { continue };
            let acked = s.dc_sent.is_some_and(|k| c.t.delivery(k) == Delivery::Acked);
            if s.probe_seen || acked {
                let info = StaticInfo {
                    local_player_index: i as u8,
                    level_checksum: self.checksum,
                    options_lzma2: self.options_lzma2.clone(),
                };
                send(c, id::DD_STATIC_INFO, &info);
                s.dd_sent = true;
            }
        }
        match self.phase {
            Phase::Waiting if !self.world_ready => {
                // protocol 4: the host builds the start world; no DD and no start before its Ready
                let gone = self.host.as_ref().is_none_or(|h| !h.active());
                let late = now.saturating_duration_since(self.created) >= self.cfg.host_start_timeout;
                if gone || late {
                    log::warn!(
                        "match {:#x}: no start world from the world host ({}): the match ends before anybody entered",
                        self.id,
                        if gone { "the host link is down" } else { "host_start_timeout" }
                    );
                    self.end(EndReason::NoWorld, now);
                }
            }
            Phase::Waiting => {
                let timeout = now.saturating_duration_since(self.created) >= self.cfg.ready_timeout;
                // the humans decide the start (bot slots are ready from the start)
                let live: Vec<&Slot> = self.slots.iter().filter(|s| s.left.is_none() && !s.spec.bot).collect();
                if live.is_empty() {
                    self.end(EndReason::AllLeft, now);
                } else if live.iter().all(|s| s.ready) || (timeout && live.iter().any(|s| s.ready)) {
                    self.start(now);
                } else if timeout {
                    self.end(EndReason::ReadyTimeout, now);
                }
            }
            Phase::Running => {
                let start = self.start.expect("running");
                if now.saturating_duration_since(start) >= self.backstop() {
                    self.end(EndReason::Timeout, now);
                    return;
                }
                // PvP end screen: the result went out and no lobby end came (the lobby is slow or gone): close with code 2
                if let (Some(at), cap) = (self.early_result, self.cfg.pvp_end_screen) {
                    if now.saturating_duration_since(at) >= cap {
                        log::warn!("match {:#x}: the result went out {cap:?} ago and no lobby end came: the relay closes (code 2)", self.id);
                        self.end(EndReason::HostEnd, now);
                        return;
                    }
                }
                // the clients' worlds reach state 5 three seconds after T_end; their match closes only on the DF leave bit
                if let Some(c) = self.t_close {
                    if self.next_tick > c && !self.overtime_hold(c) && !self.pvp_screen_hold(c, now) {
                        log::info!("match {:#x}: the clients' end at {c} (winner {:?}), closing every client at {}", self.id, self.winner, self.next_tick);
                        self.end(EndReason::TimeLimit, now);
                        return;
                    }
                }
                // PvE: the script's action 26 runs in the world step `c` (state 4 / 5); every client is closed from the
                // first DF after it, as for the time limit
                if let Some((c, reason)) = self.pve_close() {
                    if self.next_tick > c && !self.mirror_hold(c, reason, now) {
                        log::info!("match {:#x}: the mission script's failure end ({reason:?}) at {c}, closing every client at {}", self.id, self.next_tick);
                        self.end(reason, now);
                        return;
                    }
                }
                self.rejoin_watch(now);
                if !self.humans_in() {
                    self.end(EndReason::AllLeft, now);
                    return;
                }
                self.clock = self.world.time.saturating_add(now.saturating_duration_since(start).as_millis() as u32);
                // a client leaving with the menu or closing the match keeps its transport alive but stops its CDs
                // (one per tick while in the match): no CD for peer_timeout = left
                let silent: Vec<u8> = (0..self.slots.len())
                    .filter(|&i| {
                        let s = &self.slots[i];
                        s.entered && !s.spec.bot && s.left.is_none() && s.dropped.is_none() && s.last_cd.is_some_and(|t| now.saturating_duration_since(t) >= self.cfg.peer_timeout)
                    })
                    .map(|i| i as u8)
                    .collect();
                for i in silent {
                    log::warn!("match {:#x} slot {i}: no CD for {:?}", self.id, self.cfg.peer_timeout);
                    self.lose(i, LeaveReason::TimedOut, now);
                }
                if self.diag_at.is_none_or(|t| now.saturating_duration_since(t) >= Duration::from_secs(5)) {
                    self.diag_at = Some(now);
                    // cd / now / late / prof / ping / relayed / confs are counts since the start (CDs, and the entries the
                    // CDs carried); lead is the CDs' stamp minus the match clock at arrival over the last line's window
                    let per: Vec<String> = self
                        .slots
                        .iter_mut()
                        .enumerate()
                        .filter(|(_, s)| !s.spec.bot)
                        .map(|(i, s)| {
                            let d = s.diag;
                            let lead = if d.lead_n > 0 { format!("{}/{}/{}", d.lead_sum / i64::from(d.lead_n), d.lead_min, d.lead_max) } else { "-".into() };
                            (s.diag.lead_sum, s.diag.lead_n) = (0, 0);
                            format!("slot {i}: cd {} now {} late {} prof {} ping {} relayed {} confs {} lead avg/min/max {lead} ms", d.cds, d.now, d.late, d.profile, d.ping, d.relayed, d.confs)
                        })
                        .collect();
                    log::info!("match {:#x} clock {} next DF {}: {}", self.id, self.clock, self.next_tick, per.join("; "));
                }
                if self.entered_at.is_none() && now.saturating_duration_since(start) >= self.cfg.enter_wait && !self.all_synced() {
                    let missing: Vec<usize> = (0..self.slots.len()).filter(|&i| self.slots[i].entered && self.slots[i].left.is_none() && !self.slots[i].got_cd && !self.slots[i].spec.bot).collect();
                    log::warn!("match {:#x}: no CD from slot(s) {missing:?} within {:?}: entering everybody anyway", self.id, self.cfg.enter_wait);
                    self.enter_forced = true;
                }
                let delay = self.cfg.relay_delay.as_millis() as u32;
                let mut n = 0;
                while self.next_tick.saturating_add(delay) <= self.clock && n < 400 {
                    if self.bots_hold(now) {
                        break;
                    }
                    self.emit_df(self.next_tick, false);
                    self.next_tick += self.tick_ms;
                    n += 1;
                    // a leave in this DF can make the script cancel in its step: the closing DF is the very next one
                    if let Some((c, reason)) = self.pve_close().filter(|(c, _)| self.next_tick > *c) {
                        if self.mirror_hold(c, reason, now) {
                            continue;
                        }
                        log::info!("match {:#x}: the mission script's failure end ({reason:?}) at {c}, closing every client at {}", self.id, self.next_tick);
                        self.end(reason, now);
                        return;
                    }
                }
                // DE lifetime is 1900 ms: when no CD ever came, the DE may have been lost for good
                for i in 0..self.slots.len() {
                    let s = &self.slots[i];
                    let due = s.entered
                        && s.left.is_none()
                        && s.dropped.is_none()
                        && !s.got_cd
                        && s.de_tries < self.cfg.de_retries
                        && s.de_sent.is_some_and(|t| now.saturating_duration_since(t) >= self.cfg.de_retry);
                    if due {
                        self.slots[i].de_tries += 1;
                        log::warn!("match {:#x} slot {i}: no CD after the DE, sending it again", self.id);
                        self.send_de(i as u8, false, now);
                    }
                }
            }
            Phase::Ended => {}
        }
    }

    fn start(&mut self, now: Instant) {
        self.phase = Phase::Running;
        self.start = Some(now);
        self.clock = self.world.time;
        self.enter_repeats_left = self.cfg.enter_repeats.max(1);
        for i in 0..self.slots.len() {
            if self.slots[i].ready && self.slots[i].left.is_none() {
                // the start profile goes out with the enter bits (emit_df), once every client is synced
                self.slots[i].entered = true;
            }
        }
        for i in 0..self.slots.len() {
            if self.slots[i].entered && !self.slots[i].spec.bot {
                self.send_de(i as u8, false, now);
            }
        }
        log::info!(
            "match {:#x} started at world time {} with {} player(s) and {} server-owned bot slot(s)",
            self.id,
            self.world.time,
            self.slots.iter().filter(|s| s.entered && !s.spec.bot).count(),
            self.slots.iter().filter(|s| s.entered && s.spec.bot).count()
        );
    }

    fn send_de(&mut self, slot: u8, resync: bool, now: Instant) {
        let w = if resync { self.latest.clone() } else { self.world.clone() };
        let payload = match w.de_payload(self.cfg.packing) {
            Ok(p) => p,
            Err(e) => {
                log::error!("match {:#x}: DE: {e}", self.id);
                return;
            }
        };
        let s = &mut self.slots[usize::from(slot)];
        if let Some(c) = s.conn.as_mut() {
            if let Err(e) = c.t.send(id::DE_WORLD, &payload) {
                log::error!("match {:#x}: DE send: {e:?}", self.id);
                return;
            }
            s.de_sent = Some(now);
            self.stats.de_sent += 1;
            // the scoreboard's per-player activity mask (nc+0x18, 2 bits per player, 0x2001A3684) is written only by
            // E2 and never initialised by the client: all zero = every player active
            send(c, id::E2_TIME, &crate::match_msgs::ServerTime { value: 0.0 });
        }
    }

    /// Phase 2 resync is on for this match: E0 rules, `HostConfig::resync`, and not switched off by its failures (three in
    /// a row: a resync DE every part of which a client acked resets the count, rule 4).
    fn resync_rules(&self) -> bool {
        self.e0_rules() && self.resync_failures < 3 && self.host.as_ref().is_some_and(|h| h.resync_enabled())
    }

    /// Item times S at which a script step adds to an empty per-record modifier list. The clients' scene loader skips an
    /// empty record (0x2003FE7EE), so a DE at T < S that a client loads after it simulated S keeps the value and the
    /// re-simulation adds it once more. PvE only (no PvP mode the lobby offers has the scripting rule): the script's
    /// `match` in the first step after the start DE (the bots' resists of a Normal / Hard mission) and the mission start
    /// ([`PveMirror::start_at`]: a modifier on every entered human). The step's own item S already holds the value.
    fn resync_steps(&self) -> Vec<u32> {
        if self.pve_end.is_none() {
            return vec![];
        }
        // a host-built PvE start world is past its first step (world-host create_own): `match` ran in the step to the
        // DE's own time, which every DE at or above it holds, so only `start_at` is a step to guard
        let mut v = if self.start_stepped { vec![] } else { vec![self.world.time + self.tick_ms] };
        v.extend(self.pve.start_at);
        v
    }

    /// Rule 3: when `t` lies in [S - `guard_ms`, S + tick) of a scripted step S ([`Match::resync_steps`]), the first time
    /// a DE may be taken at (S + tick); None = `t` is clear of every step.
    fn resync_guard(&self, t: u32, guard_ms: u32) -> Option<u32> {
        let tick = self.tick_ms;
        self.resync_steps().into_iter().filter(|&s| t.saturating_add(guard_ms) >= s && t < s + tick).map(|s| s + tick).max()
    }

    /// A CC under the resync rules: `t` = the CC's tick rebuilt from
    /// `tick_lo`. In this order:
    /// * a client whose breaker cools down (rule 4) or whose `resync_match_max` DEs are used up: ignored;
    /// * a resync DE to it still in flight (not every part acked, rule 1): ignored (its hashes may predate the load);
    /// * at or below its floor (its pre-DE hashes): ignored;
    /// * reason 1 after a host DE (the DE did not land), or `resync_max` DEs within `resync_window`: the breaker trips for
    ///   `resync_cooldown`;
    /// * the first CC of a mismatch waits for a second one above it within `resync_confirm` (rule 2);
    /// * then the tick waits for a host DE above it ([`Match::resync_step`]).
    fn on_resync_cc(&mut self, slot: u8, t: u32, q: &QueryWorld, why: &str, now: Instant) {
        let id = self.id;
        let Some(cfg) = self.host.as_ref().map(|h| h.cfg().clone()) else { return };
        let s = &mut self.slots[usize::from(slot)];
        if !s.entered || s.left.is_some() || s.spec.bot || !s.got_cd || s.conn.is_none() || s.dropped.is_some() {
            return;
        }
        if let Some(u) = s.rs.off_until {
            if now < u {
                self.stats.resync_ignored += 1;
                self.stats.cc_logged += 1;
                if self.stats.resync_ignored % 50 == 1 {
                    log::warn!("match {id:#x} slot {slot}: CC for tick {t} (reason {:#x}): the resync breaker cools down for {:?} more ({} ignored in the match)", q.reason, u.saturating_duration_since(now), self.stats.resync_ignored);
                }
                return;
            }
            s.rs.off_until = None;
            s.resync_times.clear();
            log::warn!("match {id:#x} slot {slot}: the resync breaker's cooldown is over: resync on again for this client");
        }
        if !s.resync_off && s.rs.total >= cfg.resync_match_max {
            s.resync_off = true;
            s.resync_want = None;
            s.rs.first_cc = None;
            self.stats.resync_tripped += 1;
            log::error!("match {id:#x} slot {slot}: {} host DEs in this match: RESYNC OFF for this client for the rest of the match", s.rs.total);
        }
        if s.resync_off {
            self.stats.resync_ignored += 1;
            self.stats.cc_logged += 1;
            if self.stats.resync_ignored % 50 == 1 {
                log::warn!("match {id:#x} slot {slot}: CC for tick {t} (reason {:#x}): resync is off for this client ({} ignored in the match)", q.reason, self.stats.resync_ignored);
            }
            return;
        }
        if let Some((_, de_t, _)) = s.rs.pending {
            self.stats.resync_ignored += 1;
            log::info!("match {id:#x} slot {slot}: CC for tick {t} while its resync DE at {de_t} is in flight: ignored");
            return;
        }
        if t <= s.resync_floor {
            self.stats.resync_ignored += 1;
            log::info!("match {id:#x} slot {slot}: CC for tick {t} at or below its resync floor {}: ignored (pre-DE hashes)", s.resync_floor);
            return;
        }
        if s.rs.acked_at.is_some_and(|a| now.saturating_duration_since(a) < Duration::from_secs(2)) {
            self.stats.resync_retriggers += 1;
            log::warn!("match {id:#x} slot {slot}: CC for tick {t} above its floor {} within 2 s of its DE's ack (a re-trigger)", s.resync_floor);
        }
        while s.resync_times.front().is_some_and(|x| now.saturating_duration_since(*x) >= cfg.resync_window) {
            s.resync_times.pop_front();
        }
        // reason 1 (one value) = forced; 2, 4 and 8 are real mismatches and wait for a DE below
        let failed_load = q.reason == 1 && !s.resync_times.is_empty();
        if failed_load || s.resync_times.len() >= cfg.resync_max as usize {
            s.rs.off_until = Some(now + cfg.resync_cooldown);
            s.resync_want = None;
            s.rs.first_cc = None;
            self.stats.resync_tripped += 1;
            log::error!(
                "match {id:#x} slot {slot}: RESYNC OFF for this client for {:?}: {} (CC for tick {t}, reason {} ({why})); its CCs are ignored until then",
                cfg.resync_cooldown,
                if failed_load { "a host DE did not land (CC reason 1)".to_string() } else { format!("{} host DEs within {:?}", s.resync_times.len(), cfg.resync_window) },
                q.reason
            );
            return;
        }
        // rule 2: stale world state repeats a CC every 500 ms of world time; a hash taken by the 500 ms timeout (a late
        // DF) sends one, and the next equal compare clears its mark (0x2001AE449). A tick already waiting needs no second.
        let confirmed = s.rs.first_cc.is_some_and(|(t1, at)| t > t1 && now.saturating_duration_since(at) <= cfg.resync_confirm);
        if s.resync_want.is_none() && !confirmed {
            s.rs.first_cc = Some((t, now));
            self.stats.resync_unconfirmed += 1;
            log::info!(
                "match {id:#x} slot {slot}: CC for tick {t} (lo {}): players differing {:#08x}, global {:#04x}, reason {} ({why}): first of a mismatch, a DE needs a second CC above it within {:?}",
                q.tick,
                q.players,
                q.global,
                q.reason,
                cfg.resync_confirm
            );
            return;
        }
        s.rs.first_cc = None;
        let want = s.resync_want.map_or(t, |w| w.max(t));
        s.resync_want = Some(want);
        log::warn!(
            "match {id:#x} slot {slot}: CC for tick {t} (lo {}): players differing {:#08x}, global {:#04x}, reason {} ({why}): resync, waiting for a host DE above {want}",
            q.tick,
            q.players,
            q.global,
            q.reason
        );
    }

    /// Rule 1: the resync DEs in flight. Every part acked: the floor becomes the newest DF now (the client may have
    /// handled DFs that overtook a resent part and hashed them in the old world; it never re-hashes them, 0x2001AFD05),
    /// and the failures in a row start again at 0. The lifetime ended first: a failure, the client CCs again.
    fn resync_deliveries(&mut self, now: Instant) {
        let last = self.last_df.unwrap_or(0);
        let mut expired = Vec::new();
        let mut resumed = Vec::new();
        for (i, s) in self.slots.iter_mut().enumerate() {
            let Some((key, t, sent)) = s.rs.pending else { continue };
            let Some(c) = s.conn.as_ref() else {
                s.rs.pending = None;
                continue;
            };
            match c.t.delivery(key) {
                Delivery::InFlight => {}
                Delivery::Acked if s.rejoining => {
                    // rejoin: E0 above the newest DF (the history DFs go out now; the client hashes them after the load)
                    s.rs.pending = None;
                    s.resync_floor = s.resync_floor.max(t).max(last);
                    resumed.push((i, t));
                }
                Delivery::Expired if s.rejoining => {
                    s.rs.pending = None;
                    s.resync_want = Some(last);
                    self.stats.rejoin_retries += 1;
                    log::warn!("match {:#x} slot {i}: rejoin: the DE at {t} expired before every part was acked: asking again", self.id);
                }
                Delivery::Acked => {
                    s.rs.pending = None;
                    s.rs.acked_at = Some(now);
                    s.resync_floor = s.resync_floor.max(t).max(last);
                    self.stats.resync_acked += 1;
                    self.resync_failures = 0;
                    log::warn!(
                        "match {:#x} slot {i}: RESYNC DE at {t} acked after {} ms: floor {} (the newest DF); E0 again above it",
                        self.id,
                        now.saturating_duration_since(sent).as_millis(),
                        s.resync_floor
                    );
                }
                Delivery::Expired => {
                    s.rs.pending = None;
                    self.stats.resync_expired += 1;
                    expired.push((i, t));
                }
            }
        }
        for (i, t) in expired {
            self.resync_failed(&format!("the resync DE at {t} to slot {i} expired before every part was acked"));
        }
        for (i, t) in resumed {
            self.resume(i, t, now);
        }
    }

    /// Phase 2 resync: one WantDe at a time, only while the link is live. It goes out once the host's newest final tick
    /// (the last DF minus `final_lag_ms`, the time its DE will have) lies above the lowest waiting CC tick (a DE at or
    /// below the client's agreed time is ignored) and outside every scripted step's guard (rule 3).
    /// An answer that does not come within `de_wait` is dropped.
    fn resync_step(&mut self, now: Instant) {
        self.resync_deliveries(now);
        if self.phase != Phase::Running || !(self.resync_rules() || self.rejoin_waiting()) {
            if let Some((rid, _, _)) = self.de_req.take() {
                log::warn!("match {:#x}: WantDe {rid} dropped: resync is not running any more", self.id);
            }
            return;
        }
        let Some(h) = self.host.as_ref() else { return };
        let (live, cfg) = (h.state() == crate::host::LinkState::Live, h.cfg().clone());
        if !live {
            // never while the link is not live: what waited is dropped (a client CCs again while it differs)
            if let Some((rid, _, _)) = self.de_req.take() {
                log::warn!("match {:#x}: WantDe {rid} dropped: the world host link is {}", self.id, h.state().name());
            }
            for (i, s) in self.slots.iter_mut().enumerate() {
                s.rs.first_cc = None;
                if let Some(w) = s.resync_want.take() {
                    log::warn!("match {:#x} slot {i}: resync for tick {w} dropped: the world host link is not live", self.id);
                }
            }
            return;
        }
        if let Some((rid, at, _)) = self.de_req {
            if now.saturating_duration_since(at) < cfg.de_wait {
                return;
            }
            self.de_req = None;
            self.resync_failed(&format!("WantDe {rid}: no De within {:?}", cfg.de_wait));
            return;
        }
        let Some(need) = self.slots.iter().filter(|s| !s.resync_off && s.left.is_none()).filter_map(|s| s.resync_want).min() else { return };
        let Some(last) = self.last_df else { return };
        let t = last.saturating_sub(cfg.final_lag_ms);
        if t <= need {
            return; // the host's newest final tick is not above the CC's tick yet
        }
        if let Some(clear) = self.resync_guard(t, cfg.resync_step_guard_ms) {
            let mut first = false;
            for s in self.slots.iter_mut().filter(|s| s.resync_want.is_some()) {
                if s.rs.deferred != Some(clear) {
                    s.rs.deferred = Some(clear);
                    first = true;
                }
            }
            if first {
                self.stats.resync_deferred += 1;
                log::info!("match {:#x}: a DE now would be of time {t}, inside a scripted step's guard: WantDe waits until the host's final tick reaches {clear}", self.id);
            }
            return;
        }
        self.de_seq += 1;
        let rid = self.de_seq;
        let asked = self.host.as_mut().is_some_and(|h| h.want_de(rid));
        if asked {
            self.de_req = Some((rid, now, need));
            self.stats.resync_asked += 1;
            log::info!("match {:#x}: WantDe {rid} to the world host (last DF {last}, for CC ticks above {need})", self.id);
        }
    }

    /// A host DE failed (refused, lost, the relay's checks, a resync DE that expired). Three in a row switch resync off
    /// for the match; a delivered resync DE starts the count again ([`Match::resync_deliveries`]).
    fn resync_failed(&mut self, why: &str) {
        self.resync_failures += 1;
        self.stats.resync_refused += 1;
        if self.resync_failures >= 3 {
            log::error!("match {:#x}: {why}; {} failed host DEs in a row: RESYNC OFF for the match (CCs are logged only)", self.id, self.resync_failures);
            for s in &mut self.slots {
                s.resync_want = None;
                s.rs.first_cc = None;
            }
        } else {
            log::warn!("match {:#x}: {why} (failed host DE {} in a row of 3)", self.id, self.resync_failures);
        }
    }

    /// The host's DE for the open WantDe. Checked here: the request id,
    /// time on the tick grid, after the first DE and not after the relay's last DF, the item's own time (+0) equal to it,
    /// the decoded size (0x44800) and the wire size (255 parts), and the presence mask at +0x11 (the u32 the loader's tail
    /// loop reads) against the relay's roster: every set bit an entered slot, every entered slot still in the match
    /// set. The tail's block count and each block's parse are the host's own checks before it sends (`de_with_tail`);
    /// the relay cannot locate the item's end without the item length. A time inside a
    /// scripted step's guard (rule 3) is held back, not a failure: the next WantDe goes out after the step. Sent as the DE
    /// to every client that waits for a tick below its time, to nobody else, tracked: until the client acked every part
    /// it gets no E0 and its CCs are ignored (rule 1, [`Match::resync_deliveries`]).
    fn on_host_de(&mut self, d: world_host_proto::De, now: Instant) {
        let Some((rid, _, need)) = self.de_req else {
            log::info!("match {:#x}: host De {} (time {}) for no open WantDe: ignored", self.id, d.request_id, d.time);
            return;
        };
        if d.request_id != rid {
            log::info!("match {:#x}: host De {} for an old WantDe (open {rid}): ignored", self.id, d.request_id);
            return;
        }
        self.de_req = None;
        let t = d.time;
        let tick = self.tick_ms;
        let last = self.last_df.unwrap_or(0);
        let mask = d.item.get(0x11..0x15).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
        let entered: u32 = self.slots.iter().enumerate().filter(|(_, s)| s.entered).map(|(i, _)| 1u32 << i).sum();
        let present: u32 = self.slots.iter().enumerate().filter(|(_, s)| s.entered && s.left.is_none()).map(|(i, _)| 1u32 << i).sum();
        let check = if t % tick != 0 {
            Err(format!("time {t} off the {tick} ms grid"))
        } else if t <= self.world.time {
            Err(format!("time {t} not after the first DE ({})", self.world.time))
        } else if t > last {
            Err(format!("time {t} after the relay's last DF {last}"))
        } else if t <= need {
            Err(format!("time {t} not above the CC tick {need} (the client would ignore it)"))
        } else if d.item.len() < 22 || u32::from_le_bytes([d.item[0], d.item[1], d.item[2], d.item[3]]) != t {
            Err("the item's own time differs (or the item is shorter than its header)".to_string())
        } else if d.item.len() > crate::match_msgs::DE_MAX_BODY {
            Err(format!("{} bytes decoded, over 0x44800", d.item.len()))
        } else if !self.entered_at.is_some_and(|e| t >= e) {
            Err(format!("time {t} before the enter DF"))
        } else if let Some(m) = mask.filter(|m| m & !entered != 0 || present & !m != 0) {
            Err(format!("presence mask {m:#x}: the relay's entered {entered:#x}, still in the match {present:#x}"))
        } else {
            WorldMsg::pack(t, &d.item, self.cfg.packing)
                .map_err(|_| "the DE does not pack".to_string())
                .and_then(|m| m.to_bytes().map_err(|_| "the DE does not encode".to_string()))
                .and_then(|p| if p.len() > crate::world::DE_MAX_PAYLOAD { Err(format!("{} bytes on the wire, over 255 parts", p.len())) } else { Ok(p) })
        };
        let payload = match check {
            Ok(p) => p,
            Err(e) => {
                self.resync_failed(&format!("host De {rid} at {t} refused: {e}"));
                return;
            }
        };
        let guard = self.host.as_ref().map_or(0, |h| h.cfg().resync_step_guard_ms);
        if let Some(clear) = self.resync_guard(t, guard) {
            self.stats.resync_deferred += 1;
            log::info!("match {:#x}: host De {rid} at {t} lies inside a scripted step's guard: held back, the next WantDe waits until {clear}", self.id);
            return;
        }
        let floor = t.max(last);
        let id = self.id;
        for (i, s) in self.slots.iter_mut().enumerate() {
            let Some(w) = s.resync_want.filter(|w| *w < t) else { continue };
            if s.resync_off || s.left.is_some() || s.spec.bot {
                s.resync_want = None;
                continue;
            }
            let Some(c) = s.conn.as_mut() else { continue };
            let key = match c.t.send_tracked(id::DE_WORLD, &payload) {
                Ok(k) => k,
                Err(e) => {
                    log::error!("match {id:#x} slot {i}: resync DE send: {e:?}");
                    continue;
                }
            };
            if s.rejoining {
                // as at the start (send_de): the scoreboard's activity mask is written only by E2
                send(c, id::E2_TIME, &crate::match_msgs::ServerTime { value: 0.0 });
            }
            s.resync_want = None;
            s.rs.deferred = None;
            s.rs.pending = Some((key, t, now));
            s.resync_floor = floor;
            self.stats.resync_sent += 1;
            if s.rejoining {
                // a rejoin DE is the client's first world, not a resync: no breaker count
                log::warn!("match {id:#x} slot {i}: rejoin: the host's DE at {t} ({} bytes item+tail, {} on the wire); the DFs after it follow its ack", d.item.len(), payload.len());
                continue;
            }
            s.resync_times.push_back(now);
            s.rs.total += 1;
            log::warn!(
                "match {id:#x} slot {i}: RESYNC: the host's DE at {t} ({} bytes item+tail, {} on the wire) for its CC tick {w} (DE {} of the match); no E0 and no CC until every part is acked",
                d.item.len(),
                payload.len(),
                s.rs.total
            );
        }
    }

    /// The phase-2 relay rules apply (E0 switched on for a match with a world host, `HostConfig::e0`): the CD-together
    /// rule and no CC answer by DE. E0 itself goes out only while the link is live ([`Match::on_host_tick`]).
    fn e0_rules(&self) -> bool {
        self.host.as_ref().is_some_and(|h| h.e0_enabled() && h.active())
    }

    /// Room for the largest confirmation list of a recipient in DF `t` (8 bytes per entry, at most 64 per DF).
    fn conf_room(&self, recipients: &[usize], t: u32) -> usize {
        recipients.iter().map(|r| self.slots[*r].conf_at.range(..=t).map(|(_, v)| v.len()).sum::<usize>().min(64)).max().unwrap_or(0) * 8
    }

    /// Move every CD-bound entry of slot `p` out of DF `t` into DF `t + tick`: its precise-time entries (same dt), its
    /// input of this tick (merged into its input of the next tick, as the sender's own mover merges it), its CD profile
    /// and ping (a newer one of the next DF wins) and its confirmations.
    #[allow(clippy::too_many_arguments)]
    fn move_bundle(&mut self, t: u32, p: u8, df: &mut GameUpdate, precise: &mut Vec<bool>, late_src: &mut Vec<u32>, now_players: &mut Vec<u8>, n_server: usize) {
        let next = t + self.tick_ms;
        let mut k = 0;
        while k < df.late.len() {
            if precise[k] && df.late[k].1 == p {
                let (dt, _) = df.late.remove(k);
                let i = df.inputs.remove(k);
                late_src.remove(k);
                precise.remove(k);
                let v = self.late_at.entry(next).or_default();
                match v.iter_mut().find(|(x, q, _)| *x == dt && *q == p) {
                    Some((_, _, old)) => *old = merge_input(i, *old),
                    None => v.push((dt, p, i)),
                }
            } else {
                k += 1;
            }
        }
        if let Some(pos) = now_players.iter().position(|q| *q == p) {
            let i = df.inputs.remove(df.late.len() + pos);
            now_players.remove(pos);
            df.input_mask &= !(1 << p);
            let v = self.future.entry(next).or_default();
            match v.iter_mut().find(|(q, _)| *q == p) {
                Some((_, old)) => *old = merge_input(i, *old),
                None => v.push((p, i)),
            }
        }
        if let Some(pos) = df.profile[n_server..].iter().position(|(q, _)| *q == p) {
            let x = df.profile.remove(n_server + pos);
            let v = self.cd_profile_at.entry(next).or_default();
            if !v.iter().any(|(q, _)| *q == p) {
                v.insert(0, x);
            }
        }
        if let Some(pos) = df.ping.iter().position(|(q, _)| *q == p) {
            let x = df.ping.remove(pos);
            let v = self.ping_at.entry(next).or_default();
            if !v.iter().any(|(q, _)| *q == p) {
                v.insert(0, x);
            }
        }
        let s = &mut self.slots[usize::from(p)];
        let keys: Vec<u32> = s.conf_at.range(..=t).map(|(k, _)| *k).collect();
        let mut moved = Vec::new();
        for k in keys {
            moved.extend(s.conf_at.remove(&k).unwrap_or_default());
        }
        s.conf_at.entry(next).or_default().splice(0..0, moved);
        self.stats.df_deferred += 1;
        log::debug!("match {:#x}: DF {t} over a limit: slot {p}'s whole CD bundle moves to DF {next}", self.id);
    }

    /// The world host's E0 bytes of item `tk.t`: with E0 on and the link live, one E0 `{tick_lo = (t / tick) & 0xFF, the
    /// host's mask, one per-player byte per set bit, g0, g1, g2}` (layout PROVEN, reader 0x2001AE040: player i = mask byte
    /// `i >> 3`, bit `i & 7`; no assert, no length check) to every entered, synced human, at once. The per-player bytes
    /// are what the respawn screen needs: a client moves player i's agreed time `nc+0x7A693C[i]` on every tick where its
    /// own byte i equals the E0 byte (0x2001AE330), and the screen waits until that time passes death + 1000, so a
    /// mask-0 E0 would leave it waiting. Not before the enter DF + 500 ms, not for a tick the clients' 64-slot ring no
    /// longer holds (3.2 s behind the newest DF).
    fn on_host_tick(&mut self, tk: &world_host_proto::Tick) {
        let Some(h) = self.host.as_ref() else { return };
        if !h.e0_enabled() || h.state() != crate::host::LinkState::Live || self.phase != Phase::Running {
            return;
        }
        let Some(enter) = self.entered_at else { return };
        if tk.t < enter.saturating_add(500) || tk.t % self.tick_ms != self.world.time % self.tick_ms {
            return;
        }
        if self.last_df.is_some_and(|d| tk.t.saturating_add(3000) <= d) {
            self.stats.e0_late += 1;
            return;
        }
        // the host's mask in wire order (h[20..22], little endian) and its bytes in bit order (the frame checked the count)
        let players = u32::from_le_bytes([tk.mask[0], tk.mask[1], tk.mask[2], 0]);
        let msg = crate::match_msgs::Hash { tick: ((tk.t / self.tick_ms) & 0xFF) as u8, players, hashes: tk.hashes.clone(), globals: tk.g };
        // a dropped slot gets no E0 (a rejoining client has no world before its DE; the DE's ack sets its floor)
        for s in self.slots.iter_mut().filter(|s| s.entered && s.left.is_none() && !s.spec.bot && s.got_cd && s.dropped.is_none()) {
            if tk.t <= s.resync_floor || s.rs.pending.is_some() {
                // after a resync DE: the client's hashes up to the floor predate the load; while the DE is in flight the
                // floor is not known yet (DFs that overtake a resent part are hashed in the old world): nothing until
                // every part is acked
                self.stats.e0_held += 1;
                continue;
            }
            if let Some(c) = s.conn.as_mut() {
                send(c, id::E0_HASH, &msg);
                self.stats.e0_sent += 1;
            }
        }
    }

    /// Build and send the DF of tick `t` to every player in the match. `fin` = the match ends here: every recipient's
    /// own bit goes into `leave_mask`, which makes its client call `close_current_match(3)`.
    /// The host bots' counters (protocol 5).
    pub fn bots_stats(&self) -> BotsStats {
        self.bots_stats
    }

    /// Bit i = slot i is a bot slot the host drives.
    fn bot_mask(&self) -> u32 {
        if !self.bots_driven {
            return 0;
        }
        self.slots.iter().enumerate().filter(|(_, s)| s.spec.bot).fold(0u32, |m, (i, _)| m | 1 << i)
    }

    /// The hold: DF t waits for the `Bots` of the step before it (`t_for = t`) while the
    /// link is live and that step's DF went to the host, at most `bots_hold_ms` past its time; then it goes without
    /// (its ai entries are lost, its inputs follow late). True = do not emit DF `next_tick` yet.
    fn bots_hold(&mut self, now: Instant) -> bool {
        let t = self.next_tick;
        let Some(h) = self.host.as_ref() else { return false };
        // a closed host (PvP end screen: Close went out, the clients' DFs go on) sends no more `Bots`
        if !self.bots_driven || h.state() != crate::host::LinkState::Live || h.closed() || self.bots_upto >= t {
            self.bots_hold_since = None;
            return false;
        }
        // only a DF whose previous step the host has: the first DF, or one after a gap, has no `Bots` to wait for
        if self.host_df_last.is_none_or(|l| l.saturating_add(self.tick_ms) < t) {
            self.bots_hold_since = None;
            return false;
        }
        let hold = Duration::from_millis(u64::from(h.cfg().bots_hold_ms));
        let since = *self.bots_hold_since.get_or_insert(now);
        if self.bots_hold_counted != t {
            self.bots_hold_counted = t;
            self.bots_stats.holds += 1;
        }
        if now.saturating_duration_since(since) >= hold {
            self.bots_stats.hold_timeouts += 1;
            self.bots_hold_since = None;
            log::warn!("match {:#x}: DF {t} held {:?} for the host's bots: going out without them", self.id, hold);
            return false;
        }
        true
    }

    /// A `Bots` frame (protocol 5): each input once into DF `t_for` (late with its own time when that DF already went
    /// out), each ai entry once into DF `t_for` (lost when it already went out: ai entries carry no dt). A restarted
    /// host's frames for times already filed are dropped.
    fn on_host_bots(&mut self, b: world_host_proto::bots::Bots) {
        if b.t_for <= self.bots_upto {
            self.bots_stats.duplicates += 1;
            return;
        }
        self.bots_upto = b.t_for;
        self.bots_stats.frames += 1;
        let emitted = self.last_df.unwrap_or(0);
        let on_time = b.t_for > emitted;
        let bot_mask = self.bot_mask();
        for (slot, bytes) in b.inputs {
            if slot >= 32 || bot_mask >> slot & 1 == 0 {
                log::warn!("match {:#x}: host Bots for {}: slot {slot} is not a bot slot: dropped", self.id, b.t_for);
                continue;
            }
            let mut full = [0u8; 8];
            full[1..].copy_from_slice(&bytes[4..11]);
            let input = PlayerInput { rot: [u16::from_le_bytes([bytes[0], bytes[1]]), u16::from_le_bytes([bytes[2], bytes[3]])], actions: u64::from_be_bytes(full) };
            if on_time {
                let e = self.future.entry(b.t_for).or_default();
                e.retain(|(p, _)| *p != slot);
                e.push((slot, input));
                self.bots_stats.inputs += 1;
            } else {
                self.late_q.push((b.t_for, slot, input));
                self.bots_stats.late_inputs += 1;
            }
        }
        if !b.ai.is_empty() {
            if on_time {
                self.bots_ai_at.entry(b.t_for).or_default().extend(b.ai.iter().filter(|(s, _, _)| *s < 32 && bot_mask >> *s & 1 != 0));
            } else {
                self.bots_stats.ai_lost += b.ai.len() as u32;
                log::warn!("match {:#x}: host Bots for {} came after DF {emitted}: {} ai entr(y/ies) lost", self.id, b.t_for, b.ai.len());
            }
        }
        if b.offgrid != 0 {
            log::warn!("match {:#x}: host Bots for {}: {} entr(y/ies) off the grid (the host reports the divergence)", self.id, b.t_for, b.offgrid);
        }
    }

    /// DF t gets the host bots' ai entries filed for it; once the host is gone, one neutral input per bot slot (the
    /// last action mask persists on every client: a running or firing bot would keep doing it).
    fn bots_into_df(&mut self, t: u32, df: &mut GameUpdate) {
        if !self.bots_driven {
            return;
        }
        let old: Vec<u32> = self.bots_ai_at.range(..t).map(|(k, _)| *k).collect();
        for k in old {
            self.bots_stats.ai_lost += self.bots_ai_at.remove(&k).map_or(0, |v| v.len() as u32);
        }
        if let Some(mut a) = self.bots_ai_at.remove(&t) {
            if a.len() > DF_MAX_AI {
                self.bots_stats.ai_lost += (a.len() - DF_MAX_AI) as u32;
                a.truncate(DF_MAX_AI);
            }
            self.bots_stats.ai += a.len() as u32;
            df.ai = a;
        }
        let host_on = self.host.as_ref().is_some_and(|h| h.active());
        if !host_on && !self.bots_neutral_sent {
            self.bots_neutral_sent = true;
            let bots: Vec<u8> = self.slots.iter().enumerate().filter(|(_, s)| s.spec.bot).map(|(i, _)| i as u8).collect();
            // the neutral one replaces an input the gone host filed for this DF: nothing compares it any more, and the
            // neutral input must be the last one every client keeps
            let e = self.future.entry(t).or_default();
            e.retain(|(p, _)| !bots.contains(p));
            e.extend(bots.iter().map(|&b| (b, PlayerInput::default())));
            // inputs the gone host filed for later DFs would follow it: dropped
            for v in self.future.range_mut(t + 1..).map(|(_, v)| v) {
                v.retain(|(p, _)| !bots.contains(p));
            }
            self.late_q.retain(|(_, p, _)| !bots.contains(p));
            log::warn!("match {:#x}: the world host stopped driving the bots: a neutral input for slot(s) {bots:?} in DF {t}", self.id);
        }
    }

    fn emit_df(&mut self, t: u32, fin: bool) {
        // defensive: inputs filed for ticks that already passed
        let stale: Vec<u32> = self.future.range(..t).map(|(k, _)| *k).collect();
        for k in stale {
            for (p, i) in self.future.remove(&k).unwrap_or_default() {
                self.late_q.push((k, p, i));
            }
        }
        let mut df = GameUpdate::empty(t);
        if !fin {
            self.bots_into_df(t, &mut df);
        }
        if fin && self.fin_keeps_world {
            // code 2: the client closes the match (close_current_match(3): the match connection, on_match_finished with
            // the rules' end hooks) but keeps its world and HUD (the tear-down is skipped, 0x2001B0B7C); see `end`
            df.code = 2;
        }
        // enter: held until every entered player's client is synced (its CD arrived), so every client applies the same
        // enter at the same DF and the DFs a slow client dropped before its DE carried nothing; then all enter bits and
        // the start profiles in one DF, the enter bits repeated (a repeated enter is a no-op, 0x2002F2C55, PROVEN)
        if self.enter_repeats_left > 0 && (self.entered_at.is_some() || self.enter_forced || self.all_synced()) {
            let live: Vec<usize> = (0..self.slots.len()).filter(|&i| self.slots[i].entered && self.slots[i].left.is_none()).collect();
            if self.entered_at.is_none() {
                self.entered_at = Some(t);
                self.arm_time_limit(t, &live);
                self.arm_pve(t, &live);
                // start profiles of the humans only: a bot's spawn and profile are the mission script's
                let starts: Vec<(u8, ProfileInput)> = live.iter().filter(|&&i| !self.slots[i].spec.bot).map(|&i| (i as u8, self.slots[i].profile.clone())).collect();
                self.profile_q.splice(0..0, starts);
                let bots: Vec<usize> = live.iter().copied().filter(|&i| self.slots[i].spec.bot).collect();
                if self.pve_end.is_none() && !bots.is_empty() {
                    // PvP has no script to spawn the AI players; the local start (0x2001F4BE0) inserts each bot's profile
                    // `{0, 1 respawn request, 1 autorespawn}` at start + tick (PROVEN offline): the relay sends it as a
                    // server profile in DF t + tick
                    self.bots_spawn_at = Some(t + self.tick_ms);
                    log::info!("match {:#x}: enter of slot(s) {live:?} in DF {t} (AI players {bots:?}: spawn profile in DF {})", self.id, t + self.tick_ms);
                } else {
                    log::info!("match {:#x}: enter of slot(s) {live:?} in DF {t} (server-owned bots {bots:?}, no start profile, no input)", self.id);
                }
            }
            for &i in &live {
                df.enter_mask |= 1 << i;
            }
            self.enter_repeats_left -= 1;
        }
        // a dropped slot (rejoin) gets nothing until its client is back: the history DFs after its DE, then these
        let recipients: Vec<usize> = (0..self.slots.len())
            .filter(|i| self.slots[*i].entered && self.slots[*i].left.is_none() && !self.slots[*i].spec.bot && self.slots[*i].dropped.is_none())
            .collect();
        let mut leavers = Vec::new();
        for s in self.slots.iter_mut().enumerate() {
            if s.1.leave_pending {
                df.leave_mask |= 1 << s.0;
                s.1.leave_pending = false;
                leavers.push(s.0);
            }
        }
        if !fin {
            for i in leavers {
                self.mirror_leave(t, i);
                self.pve_leave(t, i);
            }
        }
        if fin {
            for i in &recipients {
                df.leave_mask |= 1 << i;
            }
        }
        // late inputs first, then the inputs of this tick in player order. A DF holds at most 255 entries per list and
        // DF_MAX_PAYLOAD bytes: what does not fit waits for the next DF (a deferred input of this tick goes out late)
        let mut late_inputs = Vec::new();
        let mut late_src: Vec<u32> = Vec::new();
        // per df.late entry: true = a CD's precise-time entry (bound to its CD's confirmation), false = a deferred input
        let mut precise_flag: Vec<bool> = Vec::new();
        let mut waiting = Vec::new();
        // with a world host attached a late entry keeps dt < 2 ticks (phase 2 rule: no input may reach back to a tick the
        // clients already hashed, which they do after DF t + tick): a deferred
        // input waits at most one DF, older ones are dropped like any too-old input
        let host_attached = self.host.as_ref().is_some_and(|h| h.active());
        let late_cap = if host_attached { self.cfg.late_max_ms.min(255).min(2 * self.tick_ms - 1) } else { self.cfg.late_max_ms.min(255) };
        let cd_together = self.e0_rules();
        for (stamp, p, i) in std::mem::take(&mut self.late_q) {
            let age = t.saturating_sub(stamp);
            if age > late_cap {
                self.stats.late_dropped += 1;
                if host_attached {
                    log::warn!("match {:#x}: an input of slot {p} stamped {stamp} would reach back {age} ms in DF {t}: dropped (world host attached)", self.id);
                }
                continue;
            }
            if df.late.len() >= DF_MAX_LATE {
                waiting.push((stamp, p, i));
                continue;
            }
            df.late.push((age as u8, p)); // dt in milliseconds (0x20032ED00)
            late_inputs.push(i);
            late_src.push(stamp);
            precise_flag.push(false);
        }
        // precise-time inputs filed at this DF, with their own dt (with the CD-together rule all of them first: the
        // bundle shedding below moves whole CDs when the DF is over a limit)
        let keys: Vec<u32> = self.late_at.range(..=t).map(|(k, _)| *k).collect();
        let mut precise: Vec<(u8, u8, PlayerInput)> = Vec::new();
        for k in keys {
            precise.extend(self.late_at.remove(&k).unwrap_or_default());
        }
        for (dt, p, i) in precise {
            if df.late.len() >= DF_MAX_LATE && !cd_together {
                self.late_at.entry(t + self.tick_ms).or_default().push((dt, p, i));
                continue;
            }
            df.late.push((dt, p));
            late_inputs.push(i);
            late_src.push(t.saturating_sub(u32::from(dt)));
            precise_flag.push(true);
        }
        let mut now_inputs: Vec<(u8, PlayerInput)> = self.future.remove(&t).unwrap_or_default();
        now_inputs.sort_by_key(|(p, _)| *p);
        // one input per player and tick, at most 20 players: never more than DF_MAX_INPUTS
        now_inputs.truncate(DF_MAX_INPUTS);
        let mut now_players: Vec<u8> = now_inputs.iter().map(|(p, _)| *p).collect();
        for p in &now_players {
            df.input_mask |= 1 << p;
        }
        df.inputs = late_inputs;
        df.inputs.extend(now_inputs.into_iter().map(|(_, i)| i));
        // profiles: the server's start profiles (to everybody), then the CDs' profile entries filed at this DF; pings of
        // the CDs filed at this DF; what does not fit the client's buffers (20 each) waits for the next DF
        let tick = self.tick_ms;
        let mut cd_profiles: Vec<(u8, ProfileInput)> = Vec::new();
        let mut pings: Vec<(u8, u16)> = Vec::new();
        let old_keys: Vec<u32> = self.cd_profile_at.range(..=t).map(|(k, _)| *k).collect();
        for k in old_keys {
            cd_profiles.extend(self.cd_profile_at.remove(&k).unwrap_or_default());
        }
        let old_keys: Vec<u32> = self.ping_at.range(..=t).map(|(k, _)| *k).collect();
        for k in old_keys {
            pings.extend(self.ping_at.remove(&k).unwrap_or_default());
        }
        if self.bots_spawn_at == Some(t) {
            self.bots_spawn_at = None;
            let spawn: Vec<(u8, ProfileInput)> = self
                .slots
                .iter()
                .enumerate()
                .filter(|(_, s)| s.spec.bot && s.entered)
                .map(|(i, _)| (i as u8, ProfileInput { head: BOT_SPAWN_PROFILE, entries: vec![] }))
                .collect();
            self.profile_q.splice(0..0, spawn);
        }
        let mut server_profiles = std::mem::take(&mut self.profile_q);
        if server_profiles.len() > DF_MAX_PROFILES {
            self.profile_q = server_profiles.split_off(DF_MAX_PROFILES);
        }
        let room = DF_MAX_PROFILES - server_profiles.len();
        // without the CD-together rule a CD's profile / ping entries over the client's buffers wait alone (the sender
        // keeps them at its own time: a one-tick per-player hash split)
        if cd_profiles.len() > room && !cd_together {
            let rest = cd_profiles.split_off(room);
            self.cd_profile_at.entry(t + tick).or_default().splice(0..0, rest);
        }
        if pings.len() > DF_MAX_PINGS && !cd_together {
            let rest = pings.split_off(DF_MAX_PINGS);
            self.ping_at.entry(t + tick).or_default().splice(0..0, rest);
        }
        let n_server = server_profiles.len();
        df.profile = server_profiles;
        df.profile.extend(cd_profiles);
        df.ping = pings;
        if cd_together {
            // phase 2 CD-together rule: a CD's inputs, precise entries, profile, ping
            // and its confirmation stay in one DF; over a limit the whole bundle of the highest slot moves to the next DF
            // (its confirmation too, so the sender's own mover moves its entries the same way, 0x2001B2650)
            loop {
                let conf_room = self.conf_room(&recipients, t);
                let over = df.to_bytes().map_or(usize::MAX, |b| b.len()) + conf_room > DF_MAX_PAYLOAD
                    || df.late.len() > DF_MAX_LATE
                    || df.profile.len() > DF_MAX_PROFILES
                    || df.ping.len() > DF_MAX_PINGS;
                if !over {
                    break;
                }
                let mut cand: Option<u8> = None;
                for (k, (_, p)) in df.late.iter().enumerate() {
                    if precise_flag[k] {
                        cand = cand.max(Some(*p));
                    }
                }
                // a host bot's input is never moved: it has no CD bundle, and its time is the host's
                let bot_mask = self.bot_mask();
                for p in now_players.iter().filter(|p| bot_mask >> **p & 1 == 0).chain(df.profile[n_server..].iter().map(|(p, _)| p)).chain(df.ping.iter().map(|(p, _)| p)) {
                    cand = cand.max(Some(*p));
                }
                let Some(p) = cand else { break };
                self.move_bundle(t, p, &mut df, &mut precise_flag, &mut late_src, &mut now_players, n_server);
            }
        }
        // room for the largest confirmation list of a recipient (8 bytes per entry, at most 64 per DF)
        let conf_room = self.conf_room(&recipients, t);
        let (mut back_late, mut back_now) = (Vec::new(), Vec::new());
        // host bot inputs outrank human ones: a human input can go out late, a bot's ai entries never
        let bot_mask = self.bot_mask();
        while df.to_bytes().map_or(usize::MAX, |b| b.len()) + conf_room > DF_MAX_PAYLOAD {
            if let Some(x) = df.ping.pop() {
                self.ping_at.entry(t + tick).or_default().insert(0, x);
            } else if df.profile.len() > n_server {
                let x = df.profile.pop().expect("a CD profile");
                self.cd_profile_at.entry(t + tick).or_default().insert(0, x);
            } else if let Some(x) = df.profile.pop() {
                self.profile_q.insert(0, x);
            } else if let Some(pos) = now_players.iter().rposition(|p| bot_mask >> *p & 1 == 0) {
                let p = now_players.remove(pos);
                let i = df.inputs.remove(df.late.len() + pos);
                df.input_mask &= !(1 << p);
                back_now.push((t, p, i));
            } else if let Some((_, p)) = df.late.pop() {
                let i = df.inputs.pop().expect("one input per late entry");
                back_late.push((late_src.pop().expect("one stamp per late entry"), p, i));
            } else {
                break;
            }
        }
        if !back_late.is_empty() || !back_now.is_empty() {
            self.stats.df_deferred += (back_late.len() + back_now.len()) as u32;
            log::debug!("match {:#x}: DF {t} over {DF_MAX_PAYLOAD} bytes, {} input(s) wait", self.id, back_late.len() + back_now.len());
        }
        back_late.reverse();
        back_now.reverse();
        back_late.extend(waiting);
        back_late.extend(back_now);
        self.late_q = back_late;
        self.stats.inputs_relayed += now_players.len() as u32;
        for p in &now_players {
            if let Some(s) = self.slots.get_mut(usize::from(*p)) {
                s.diag.relayed += 1;
            }
        }
        for (_, p) in &df.late {
            if let Some(s) = self.slots.get_mut(usize::from(*p)) {
                s.diag.relayed += 1;
            }
        }
        self.stats.late_relayed += df.late.len() as u32;
        // the whole tick as the world sees it (before the no-echo filtering below; confirmations are per recipient)
        let to_host = !fin && self.host.as_ref().is_some_and(|h| h.active());
        let whole = if self.rec.is_some() || to_host { df.to_bytes().ok() } else { None };
        if let Some(r) = self.rec.as_mut() {
            let res = whole.as_deref().ok_or_else(|| std::io::Error::other("DF encode")).and_then(|b| r.df(t, b));
            if let Err(e) = res {
                log::warn!("match {:#x}: input recording stopped: {e}", self.id);
                self.rec = None;
            }
        }
        if !fin {
            self.last_df = Some(t);
            // the same bytes to the world host; never a fin DF (every recipient's leave bit would end its world)
            if let (Some(h), true) = (self.host.as_mut(), to_host) {
                match whole.as_deref() {
                    Some(b) => h.df(t, b),
                    None => h.give_up(&format!("DF {t} not encodable: the host would miss it")),
                }
                self.host_df_last = Some(t);
            }
        }
        let n_server = n_server.min(df.profile.len());
        if !fin && self.cfg.rejoin && host_attached {
            self.history.push_back((df.clone(), n_server));
            while self.history.front().is_some_and(|(d, _)| d.time.saturating_add(REJOIN_HISTORY_MS) < t) {
                self.history.pop_front();
            }
        }
        for r in recipients {
            let mut mine = df_for(&df, r, n_server);
            let s = &mut self.slots[r];
            let keys: Vec<u32> = s.conf_at.range(..=t).map(|(k, _)| *k).collect();
            let mut conf: Vec<(u32, u32)> = Vec::new();
            for k in keys {
                conf.extend(s.conf_at.remove(&k).unwrap_or_default());
            }
            if conf.len() > 64 {
                let rest = conf.split_off(64);
                s.conf_at.entry(t + tick).or_default().splice(0..0, rest);
            }
            s.diag.confs += conf.len() as u32;
            mine.conf = conf
                .into_iter()
                .map(|(a, at)| {
                    let b = match self.cfg.conf {
                        ConfMode::ReceiveClock => at,
                        ConfMode::EchoStamp => a,
                        ConfMode::ServerTime => t,
                    };
                    (a, b)
                })
                .collect();
            if let Some(c) = s.conn.as_mut() {
                send(c, id::DF_GAME_UPDATE, &mine);
            }
        }
        self.stats.ticks_sent += 1;
    }

    /// The lobby ends a PvE match after its result (`99`, `83 45 0B`) went out: the result came from the mission's end
    /// (the host's Outcome, the hand-over, or the clients' end reports 53 event 5 / 6, sent when their world ended), so
    /// the closing DFs keep the clients' world (code 2, see [`Match::end`]). Not PvE: a plain [`Match::end`].
    pub fn end_after_result(&mut self, now: Instant) {
        if (self.pve_end.is_some() || self.early_result.is_some()) && self.phase != Phase::Ended {
            self.result_sent = true;
        }
        self.end(EndReason::Requested, now);
    }

    /// End the match now (the lobby asked, or a timer fired). The result is queued for [`take_result`](Self::take_result).
    pub fn end(&mut self, reason: EndReason, now: Instant) {
        if self.phase == Phase::Ended {
            return;
        }
        if self.bots_driven {
            let b = self.bots_stats;
            log::info!(
                "match {:#x}: host bots: {} Bots frames ({} duplicates), inputs {} on time / {} late, ai {} / {} lost, held DFs {} ({} to the deadline){}",
                self.id,
                b.frames,
                b.duplicates,
                b.inputs,
                b.late_inputs,
                b.ai,
                b.ai_lost,
                b.holds,
                b.hold_timeouts,
                if self.host.as_ref().is_some_and(|h| h.diverged()) { "; the host DIVERGED" } else { "" }
            );
        }
        // PvE: the script's own failure ends are winner_team 1 (action 26 of every cancel, of the 45-minute end and of
        // "every human left"); with nobody ready or nobody left the script could only reach its 7-minute cancel. Not for
        // the backstop or the lobby's request: those are not script ends.
        // A world host's earlier Outcome stays the match's result (its world ended first).
        if self.pve_end.is_some()
            && !self.outcome_before_end
            && matches!(reason, EndReason::PveCancel | EndReason::PveTimeout | EndReason::AllLeft | EndReason::ReadyTimeout)
        {
            self.winner = Some(1);
        }
        // PvE after the script's end (the host's or the mirror's Outcome, or a mirrored failure end): the closing DFs carry
        // code 2 (PROVEN, DF handler 0x2001B0B76): the client closes the match connection and runs the rules' end hooks
        // (the PvE result screen with a mouse pointer) but skips the tear-down (UI clean-up, world unload, lobby return),
        // so the result screen stays usable. Its "Return to lobby" button (`close_pve_stats_clicked`) does the tear-down
        // itself and sends 5F 04. With code 0xFF the client returns to the lobby at once. Not for a PvE end without the
        // script's end (backstop, lobby request before any result, ready timeout): nothing would ever show the screen.
        self.fin_keeps_world = self.pve_end.is_some() && (self.outcome_before_end || self.result_sent || matches!(reason, EndReason::PveCancel | EndReason::PveTimeout));
        if self.fin_keeps_world && self.phase == Phase::Running {
            log::info!("match {:#x}: PvE end ({reason:?}, winner {:?}): the closing DFs carry code 2 (the clients keep their ended world: the result screen)", self.id, self.winner);
        }
        // PvP end screen: the world ended in the host and its result went out while the match ran: code 2 too. The
        // client's end hooks show the victory splash 1 s later, then the statistics; its `close_victory_screen` unloads
        // the world and returns to the lobby itself (0x20025F710). A player who left is not in the closing DF anyway.
        if self.pve_end.is_none() && self.early_result.is_some() {
            self.fin_keeps_world = true;
            if self.phase == Phase::Running {
                log::info!("match {:#x}: PvP end after its result ({reason:?}, winner {:?}): the closing DFs carry code 2 (the in-match victory screen)", self.id, self.winner);
            }
        }
        if self.phase == Phase::Running {
            // the DF is plain with a short lifetime: the closing leave bits go out on 4 consecutive ticks (a repeated
            // own leave bit is harmless, the client already closed)
            for _ in 0..4 {
                self.emit_df(self.next_tick, true);
                self.next_tick += self.tick_ms;
            }
        }
        if let Some(mut r) = self.rec.take() {
            if let Err(e) = r.close(self.next_tick) {
                log::warn!("match {:#x}: input recording not closed cleanly: {e}", self.id);
            }
        }
        // the humans only: a server-owned bot has no account
        let players = self
            .slots
            .iter()
            .enumerate()
            .filter(|(_, s)| !s.spec.bot)
            .map(|(i, s)| PlayerResult {
                slot: i as u8,
                account_id: s.spec.account_id,
                team: s.spec.team,
                entered: s.entered,
                // a slot still dropped at the end (its client never came back): its drop is its leave
                leave: if !s.entered { LeaveReason::NeverEntered } else { s.left.unwrap_or(s.dropped.map_or(LeaveReason::MatchEnded, |(_, why)| why)) },
            })
            .collect();
        let result = MatchEnd {
            match_id: self.id,
            map: self.map,
            mode: self.mode,
            reason,
            winner: self.winner,
            stats: self.player_stats.clone(),
            duration: self.start.map(|s| now.saturating_duration_since(s)).unwrap_or_default(),
            players,
            bots: self
                .slots
                .iter()
                .enumerate()
                .filter(|(_, s)| s.spec.bot && self.pve_end.is_none())
                .map(|(i, s)| PlayerResult { slot: i as u8, account_id: 0, team: s.spec.team, entered: s.entered, leave: if s.entered { LeaveReason::MatchEnded } else { LeaveReason::NeverEntered } })
                .collect(),
        };
        // a world host still in use: Close at the last normal DF (the fin DFs above never reached it), and the result is
        // kept back until its Final or `final_wait` (inside the drain, no client traffic waits for it)
        let t_last = self.last_df.unwrap_or(self.world.time);
        if self.early_result.is_some() {
            // the PvP end screen's result went out while the match ran: none again
            self.phase = Phase::Ended;
            self.end = Some((reason, now));
            log::info!("match {:#x} ended: {:?} (its result went out before)", self.id, reason);
            return;
        }
        match self.host.as_mut().filter(|h| h.active()) {
            // a diverged host that kept driving the bots (protocol 5) gets its Close, but its Final is never used
            Some(h) if h.diverged() => {
                h.close(t_last);
                self.result = Some(result);
            }
            Some(h) => {
                h.close(t_last);
                self.held = Some((result, now));
            }
            None => self.result = Some(result),
        }
        self.phase = Phase::Ended;
        self.end = Some((reason, now));
        log::info!("match {:#x} ended: {:?}", self.id, reason);
    }

    /// The PvP end screen's result: the `MatchEnd` of the world's end (reason `HostEnd`, the host's winner, the Final's
    /// counters and badges, else the Outcome's) goes to the lobby while the clients still run; the lobby's
    /// [`end_after_result`](Self::end_after_result) closes them with code 2, the backstop when it does not come.
    fn publish_early_result(&mut self, now: Instant) {
        let players = self
            .slots
            .iter()
            .enumerate()
            .filter(|(_, s)| !s.spec.bot)
            .map(|(i, s)| PlayerResult {
                slot: i as u8,
                account_id: s.spec.account_id,
                team: s.spec.team,
                entered: s.entered,
                // as in `end`: a slot still dropped (its client never came back) has its drop as its leave
                leave: if !s.entered { LeaveReason::NeverEntered } else { s.left.unwrap_or(s.dropped.map_or(LeaveReason::MatchEnded, |(_, why)| why)) },
            })
            .collect();
        let mut r = MatchEnd {
            match_id: self.id,
            map: self.map,
            mode: self.mode,
            reason: EndReason::HostEnd,
            winner: self.winner,
            stats: self.player_stats.clone(),
            duration: self.start.map(|s| now.saturating_duration_since(s)).unwrap_or_default(),
            players,
            bots: self
                .slots
                .iter()
                .enumerate()
                .filter(|(_, s)| s.spec.bot)
                .map(|(i, s)| PlayerResult { slot: i as u8, account_id: 0, team: s.spec.team, entered: s.entered, leave: if s.entered { LeaveReason::MatchEnded } else { LeaveReason::NeverEntered } })
                .collect(),
        };
        self.fill_from_host(&mut r);
        log::info!(
            "match {:#x}: the result goes to the lobby while the clients stay in the match (winner {:?}, Final {}); the lobby ends it after its 97, the relay at the latest in {:?}",
            self.id,
            r.winner,
            self.host.as_ref().is_some_and(|h| h.final_outcome().is_some()),
            self.cfg.pvp_end_screen
        );
        self.result = Some(r);
        self.early_result = Some(now);
    }

    /// The relay's time-limit / empty-team close of a PvP match with a live world host and the end screen on: it waits
    /// `HostConfig::mirror_wait` for the host's Outcome (which needs DF `c + 300` to read tick `c` as final), so the end
    /// goes the end screen's way ([`MatchConfig::pvp_end_screen`]). True = do not close now.
    fn pvp_screen_hold(&mut self, c: u32, now: Instant) -> bool {
        if self.pve_end.is_some() {
            return false;
        }
        // the Outcome came: the end goes the end screen's way (`host_poll`: the host's Close, its Final, the result first)
        if self.outcome_before_end {
            return self.early_close.is_some();
        }
        let Some(wait) = self.host.as_ref().filter(|h| h.state() == crate::host::LinkState::Live && !h.diverged() && !h.closed()).map(|h| h.mirror_wait()) else {
            return false;
        };
        let id = self.id;
        let since = *self.pvp_hold_since.get_or_insert_with(|| {
            log::info!("match {id:#x}: the clients' end at {c}: waiting up to {wait:?} for the host's Outcome (the PvP end screen)");
            now
        });
        if now.saturating_duration_since(since) < wait {
            return true;
        }
        log::warn!("match {:#x}: no world host Outcome within {wait:?} of the clients' end at {c}: the relay closes (no end screen)", self.id);
        false
    }

    /// Apply the world host's frames (after this poll's DFs), and publish a held `MatchEnd` once its `Final` came or
    /// `final_wait` passed.
    fn host_poll(&mut self, now: Instant) {
        let Some(h) = self.host.as_mut() else { return };
        for n in h.poll() {
            match n {
                crate::host::Note::Outcome(o) => self.on_host_outcome(o),
                crate::host::Note::Final(o) => self.on_host_final(&o),
                crate::host::Note::Tick(tk) => self.on_host_tick(&tk),
                crate::host::Note::De(d) => self.on_host_de(d, now),
                crate::host::Note::Start { checksum, item } => self.on_start_world(checksum, item),
                crate::host::Note::Bots(b) => self.on_host_bots(b),
                crate::host::Note::DeRefused { request_id, reason } => {
                    if self.de_req.is_some_and(|(r, _, _)| r == request_id) {
                        self.de_req = None;
                        self.resync_failed(&format!("the host refused WantDe {request_id}: {reason}"));
                    }
                }
            }
        }
        // PvP (phase 3): the world ended in the host before any mirrored end: the clients' worlds are in the same state
        // and wait for the server's close (an online client never closes by itself), so it comes now. PvE keeps the
        // lobby's end sequence (the result screen in the match).
        if self.phase == Phase::Running && self.pve_end.is_none() && self.outcome_before_end && self.early_close.is_none() {
            let screen = self.host.as_ref().is_some_and(|h| h.active() && !h.diverged() && !h.closed());
            if screen {
                // the PvP end screen: Close the host now (its match end, the badges: `Final`), the clients run on
                let t_last = self.last_df.unwrap_or(self.world.time);
                log::info!(
                    "match {:#x}: PvP end from the world host's Outcome (winner {:?}): Close {t_last} to the host for its Final; the clients stay in the match for the result screen",
                    self.id,
                    self.winner
                );
                if let Some(h) = self.host.as_mut() {
                    h.close(t_last);
                }
                self.early_close = Some(now);
            } else {
                log::info!("match {:#x}: PvP end from the world host's Outcome (winner {:?}): closing every client at {}", self.id, self.winner, self.next_tick);
                self.end(EndReason::HostEnd, now);
            }
        }
        if let (Some(at), None, Phase::Running) = (self.early_close, self.early_result, self.phase) {
            let done = self.host.as_ref().is_none_or(|h| h.settled() || now.saturating_duration_since(at) >= h.final_wait());
            if done {
                self.publish_early_result(now);
            }
        }
        let Some((_, at)) = &self.held else { return };
        let Some(h) = self.host.as_ref() else { return };
        if h.settled() || now.saturating_duration_since(*at) >= h.final_wait() {
            let (mut r, _) = self.held.take().expect("held");
            self.fill_from_host(&mut r);
            self.result = Some(r);
        }
    }

    /// Protocol 4: the start world the host built (its `Ready`): the first DE, the DD checksum, the match clock, the PvE
    /// mirror's cancel step and the recording start from it. A host-built PvE world is past its first step (world-host
    /// `create_own`): pve.lua's `match` ran in the step to its time, so `wait_before_cancel` started there.
    fn on_start_world(&mut self, checksum: u32, item: Vec<u8>) {
        if self.world_ready || self.phase != Phase::Waiting {
            return;
        }
        let w = match World::from_item(self.map, self.mode, self.id, checksum, self.world.tick_ms, self.world.send_period_ms, item) {
            Ok(w) => w,
            Err(e) => {
                log::error!("match {:#x}: the world host's start world is unusable: {e}", self.id);
                if let Some(h) = self.host.as_mut() {
                    h.give_up(&format!("unusable start world: {e}"));
                }
                return;
            }
        };
        let t0 = w.time;
        log::info!("match {:#x}: start world from the world host: time {t0}, {} bytes, level checksum {checksum:#x}", self.id, w.body.len());
        if self.pve_end.is_some() {
            self.pve.cancel_at = self.pve_end.map(|r| t0 + r.cancel_ms);
            self.start_stepped = true;
        }
        let w = Arc::new(w);
        self.world = w.clone();
        self.latest = w.clone();
        if self.checksum == 0 {
            self.checksum = checksum;
        }
        self.next_tick = t0 + self.tick_ms;
        self.clock = t0;
        self.world_ready = true;
        if let Some(dir) = self.rec_dir.take() {
            self.rec = open_recorder(&dir, self.id, &self.spec_players, &self.spec_options, &w);
        }
    }

    fn host_stats(&self, players: &[world_host_proto::PlayerStats], team: [u32; 2], details: &[world_host_proto::PlayerDetail]) -> FinalStats {
        let n = self.slots.len();
        FinalStats {
            players: players
                .iter()
                .filter(|p| usize::from(p.slot) < n)
                .map(|p| PlayerStat { slot: p.slot, kills: p.kills, deaths: p.deaths, score: p.score, chests: p.chests })
                .collect(),
            team_scores: team,
            details: details.iter().filter(|d| usize::from(d.slot) < n).copied().collect(),
            kills: None,
            used: None,
        }
    }

    /// The host's world ended (latched once by the link). While the match runs, its values are the match's: the winner
    /// (also for PvE, so the lobby needs no reports) and the counters. The relay does not close the clients: the
    /// lobby shows the result and ends the match later.
    fn on_host_outcome(&mut self, o: world_host_proto::Outcome) {
        let players: Vec<OutcomePlayer> = o
            .players
            .iter()
            .filter_map(|p| {
                let s = self.slots.get(usize::from(p.slot))?;
                Some(OutcomePlayer {
                    slot: p.slot,
                    account_id: s.spec.account_id,
                    bot: s.spec.bot,
                    kills: p.kills,
                    deaths: p.deaths,
                    score: p.score,
                    chests: p.chests,
                    keys_spent: p.keys_spent,
                    gold_spent: p.gold_spent,
                })
            })
            .collect();
        if self.handed_over {
            // the relay's mirrored end went to the lobby first: it stays the result (one Outcome per match)
            if o.winner_team != 1 {
                log::error!("match {:#x}: HOST AND MIRROR DISAGREE: the relay handed over a failure end (winner 1), the host's world ended at {} with winner {}", self.id, o.t_end, o.winner_team);
            } else {
                log::info!("match {:#x}: the host's Outcome (end {}, winner 1) after the relay's handed-over failure end", self.id, o.t_end);
            }
            return;
        }
        if self.phase == Phase::Ended {
            // the relay's own end came first (its mirror, the lobby or the backstop): both are logged, the relay's winner
            // stays; a disagreement is a host or mirror bug
            let reason = self.end.map(|(r, _)| r);
            let mirror_fail = matches!(reason, Some(EndReason::PveCancel | EndReason::PveTimeout | EndReason::AllLeft));
            if mirror_fail && o.winner_team != 1 {
                log::error!("match {:#x}: HOST AND MIRROR DISAGREE: the relay ended {reason:?} (winner 1), the host's world ended at {} with winner {}", self.id, o.t_end, o.winner_team);
            } else {
                log::info!("match {:#x}: the host's Outcome (end {}, winner {}) after the relay's own end {reason:?} (winner {:?})", self.id, o.t_end, o.winner_team, self.winner);
            }
        } else {
            self.outcome_before_end = true;
            self.winner = Some(if self.pve_end.is_none() { mode_winner(self.mode, o.state, o.winner_team) } else { o.winner_team });
            self.player_stats = Some(self.host_stats(&o.players, o.team_score, &o.details));
            log::info!(
                "match {:#x}: the world ended in the host at {} (state {}, winner {}), before the relay's mirror (next mirrored end {:?}): reported to the lobby",
                self.id,
                o.t_end,
                o.state,
                o.winner_team,
                self.pve_close()
            );
        }
        self.outcomes.push(HostOutcome {
            match_id: self.id,
            t_end: o.t_end,
            state: o.state,
            winner_team: o.winner_team,
            pve_success: o.pve_success,
            mission_ms: o.mission_ms,
            players,
            team_scores: o.team_score,
            details: o.details.iter().filter(|d| usize::from(d.slot) < self.slots.len()).copied().collect(),
        });
    }

    fn on_host_final(&mut self, f: &world_host_proto::Outcome) {
        let reason = self.end.map(|(r, _)| r);
        log::info!("match {:#x}: the host's Final at {} (state {}, winner {}); the relay's end: {reason:?}, winner {:?}", self.id, f.t_end, f.state, f.winner_team, self.winner);
        let mirror_fail = matches!(reason, Some(EndReason::PveCancel | EndReason::PveTimeout));
        if mirror_fail && (!matches!(f.state, 4 | 5) || f.winner_team != 1) {
            log::error!("match {:#x}: HOST AND MIRROR DISAGREE: the relay mirrored {reason:?} (a script end, winner 1); the host's world at {} has state {} winner {}", self.id, f.t_end, f.state, f.winner_team);
        }
    }

    /// The held `MatchEnd` gets the host's values: the winner when the host's world ended before the relay did, the
    /// counters of `Final`, else of the last `Stats`, else of the `Outcome`.
    fn fill_from_host(&self, r: &mut MatchEnd) {
        let Some(h) = self.host.as_ref() else { return };
        if self.outcome_before_end && !self.handed_over {
            if let Some(o) = h.outcome() {
                r.winner = Some(if self.pve_end.is_none() { mode_winner(self.mode, o.state, o.winner_team) } else { o.winner_team });
            }
        } else if self.pve_end.is_none() {
            // PvP (phase 3): the relay's mirror ended first (time limit, empty team, a lobby request). The winner byte is
            // a running value (the tdm rule rewrites it at every death and leave, 0x200366A60), so the one that counts
            // is the byte at the world's end tick:
            // * the host's Outcome (latched at the first state 4 / 5 tick) is that byte: it is used;
            // * a Final (read at Close.t_last, after the end tick) can carry a later value (a leave after the end; in a
            //   replay of a recording the fin DFs empty both teams -> 2): it only fills a winner the relay does not know
            //   (a time end); the relay's own winner (the tdm empty-team mirror, PROVEN to the step) is kept.
            // Every difference is logged.
            if let Some(o) = h.outcome() {
                let w = mode_winner(self.mode, o.state, o.winner_team);
                if r.winner.is_some_and(|x| x != w) {
                    log::error!("match {:#x}: HOST AND MIRROR DISAGREE: the relay's winner {:?} ({:?}), the host's world at its end {}: the host's is used", self.id, r.winner, r.reason, o.winner_team);
                }
                r.winner = Some(w);
            } else if let Some(f) = h.final_outcome().filter(|f| matches!(f.state, 4 | 5)) {
                match r.winner {
                    None => r.winner = Some(mode_winner(self.mode, f.state, f.winner_team)),
                    Some(w) if w != mode_winner(self.mode, f.state, f.winner_team) => log::error!(
                        "match {:#x}: HOST AND MIRROR DISAGREE: the relay's winner {w} ({:?}), the host's Final at {} has {}: the relay's is kept (a Final is read after the end tick)",
                        self.id,
                        r.reason,
                        f.t_end,
                        f.winner_team
                    ),
                    Some(_) => {}
                }
            }
        }
        let src = h
            .final_outcome()
            .map(|f| (&f.players, f.team_score, "Final"))
            .or_else(|| h.last_stats().map(|s| (&s.players, s.team_score, "the last Stats")))
            .or_else(|| h.outcome().map(|o| (&o.players, o.team_score, "the Outcome")));
        // the detail rows (version 3): Final's (badges after the match end), else the Outcome's (no badges)
        let details = h.final_outcome().or_else(|| h.outcome()).map(|o| o.details.as_slice()).unwrap_or_default();
        match src {
            Some((players, team, what)) => {
                let mut st = self.host_stats(players, team, details);
                st.kills = h.final_outcome().and_then(|f| crate::host::checked_kills(self.id, h.kills(), f));
                // only the roster's human slots (a bot slot has no account to charge)
                st.used = h.used().map(|u| {
                    u.players.iter().filter(|p| self.slots.get(usize::from(p.slot)).is_some_and(|s| !s.spec.bot)).cloned().collect()
                });
                log::info!(
                    "match {:#x}: result with the host's counters from {what} (host {}), {} detail row(s), kill log {}",
                    self.id,
                    h.state().name(),
                    st.details.len(),
                    st.kills.as_ref().map_or("none".to_string(), |k| format!("{} death(s)", k.len()))
                );
                r.stats = Some(st);
            }
            None => log::info!("match {:#x}: no host counters (host {}): the result is the relay's", self.id, h.state().name()),
        }
    }
}
