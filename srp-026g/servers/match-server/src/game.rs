//! One 0.21d match, the server's side of the clients' lockstep
//! (`../srp-021d/docs/notes/match.md`, `match_start_spec.md`, `option_b.md`).
//!
//! The clients simulate; the server does not. It:
//! - takes each client through the handshake: 0x40 → 0x50, 0x41 → 0x51,
//!   0x42 → 0x52 and 0x53, then the inputs and roster changes since the
//!   snapshot, then 0x56 to put the player in;
//! - keeps the match clock, relays each input (0x45 → 0x54) while its tick is
//!   not final (250 ms), and tells a client whose input came too late to drop
//!   it (0x58);
//! - gets world snapshots from patched clients (0x47 chunks). The first one
//!   starts the match, and must be a world where nothing has happened yet.
//!   Later ones are taken only when asked for with 0x59, for a tick near the
//!   match clock, and used when their CRC matches what most clients
//!   reported for that tick (0x46);
//! - reads the results only from snapshots at least two clients vouch for
//!   (their 0x46 CRC for that tick), and only when they are plausible: one
//!   modified client can't decide the winner, the kills or the rewards;
//! - resyncs a client whose reports disagree with the majority, and serves
//!   late joiners from a fresh snapshot;
//! - ends the match when its time is up: each client gets 0x57 with its own
//!   index and shows the result screen. No client decides the end.

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::snapshot::{self, Decoded, PlayerStatistics, event};
use crate::transport::{Event, Transport};
use survarium::v021d::results::{NO_WINNER, PlayerStats};
use survarium::v021d::supplies::Loadout;

/// The logic tick.
pub const TICK_MS: u32 = 50;
/// The clients' initial world time, and the first snapshot's: the tick grid
/// starts there (`game_world_core::clear`).
pub const T0: u32 = 50;
/// A tick is final this long after it: the server's event horizon.
const HORIZON_MS: u32 = 250;
/// Clients drop inputs further ahead than this.
const TOO_NEW_MS: u32 = 2500;
/// With others simulating, a newcomer starts only from a snapshot this
/// fresh: an upload is the uploader's event-horizon item, about 500 ms old
/// (1000 from a client in the background).
const FRESH_SNAPSHOT_MS: u32 = 1_500;
/// A client resynced, or a newcomer while others play, starts only from a
/// snapshot this fresh. Its clock starts `SYNC_LEAD_MS` past the snapshot, so
/// every ms of the snapshot's age is a ms it runs behind the match clock, and
/// while it is more than `HORIZON_MS` behind, its inputs are discarded. A
/// fresh upload is 560-620 ms old; a stored one was 1150-1450 and left the
/// client 800-1100 ms behind for up to 20 s (match 0x123: resynced every 15 s,
/// each time out of step again). Newcomers took any up to `FRESH_SNAPSHOT_MS`:
/// at the start of a match the initial world, which left the clients that
/// loaded last up to 1043 ms behind, their inputs discarded for 10-20 s
/// (matches 0x134 and 0x123, 2026-10-11).
const RESYNC_FRESH_MS: u32 = 700;
/// How long a client waits for such an upload; then any snapshot
/// `FRESH_SNAPSHOT_MS` fresh will do.
const RESYNC_FRESH_WAIT: Duration = Duration::from_secs(3);
/// How soon a client that waits for an upload asks again (the periodic
/// requests wait `SNAPSHOT_WAIT`).
const RESYNC_REQUEST_GAP: Duration = Duration::from_secs(1);
/// A player back in a match enters for the others first, and starts from a
/// snapshot after that: an upload is the uploader's world about 500 ms back,
/// so one is asked for once the match clock is this far past its entry.
const REJOIN_UPLOAD_AFTER_MS: u32 = 600;
/// A resynced client keeps its own inputs (`clear()` leaves the input lists),
/// and a new local input merges into its first own input later than the
/// tick it plays, moving it to the new input's tick (`change_input`
/// 0x1CB494). Started behind inputs of its own the others have, the first
/// move of the mouse changed one of those in its world alone: out of step
/// again 600-700 ms after the snapshot (match 0x123, 16:06, 3 resyncs of 4).
/// So its clock must start past the last of them (`SYNC_LEAD_MS` past the
/// snapshot), and an upload, 560-650 ms behind the match clock, is asked
/// for this long after that input.
const RESYNC_UPLOAD_AFTER_MS: u32 = 350;
/// Just before a resync's snapshot, the client is told to drop (0x58) every
/// own input it may hold after the snapshot but those the others have: the
/// ones the match dropped while it waited, and one made but not yet sent.
/// Its clock may be up to this far ahead of the match clock.
const CLEAR_AHEAD_MS: u32 = 250;
/// A client whose clock is still being corrected is not resynced (it would
/// start behind again, with its inputs discarded) for at most this long.
const RESYNC_DEFER_MAX: Duration = Duration::from_secs(20);
/// Inputs discarded as too late are logged once a client has had this long of
/// them, as a count.
const DISCARD_LOG: Duration = Duration::from_secs(1);
/// A client takes a relayed input (0x54) or drops its own (0x58) only while
/// its tick is less than this far behind the client's clock
/// (`max_ping_time_in_ms` + 250); a later one is refused as too old, and that
/// client is out of step for good: with a 0x58 its own player keeps the input
/// the others never got, with a 0x54 it lacks another player's.
const CLIENT_HORIZON_MS: i64 = 500;
/// 0x54 and 0x58 awaiting their ack, per client, at most (the oldest go).
const IN_FLIGHT_KEPT: usize = 4096;
/// A client starts its clock this far past the snapshot it starts from, so
/// that its event horizon (`max_ping_time_in_ms` + 250 = 500 ms behind its
/// clock) stays before every input since the snapshot until they reach it:
/// it rejects older ones as "too old". It is then this much behind the match
/// clock, at most, and 0x55 brings it forward.
const SYNC_LEAD_MS: u32 = 350;
/// With no fresh snapshot to be had for this long, a waiting client starts
/// from the newest one there is.
const SNAPSHOT_GIVE_UP: Duration = Duration::from_secs(20);
/// 0x55 carries an i16.
const MAX_CLOCK_CORRECTION_MS: u32 = i16::MAX as u32;
/// How many clients are asked for a snapshot at a time: the one with the
/// best link in a small match, the two best from this many players on.
/// Results need the others' hashes to agree, not more uploads.
const LARGE_MATCH_PLAYERS: usize = 4;
const SNAPSHOT_SOURCES: usize = 2;
/// The least time between two requests for an upload.
const SNAPSHOT_WAIT: Duration = Duration::from_secs(3);
/// How long an upload waits for the hash reports that check it.
const CANDIDATE_WAIT: Duration = Duration::from_secs(3);
/// The least time between two resyncs of one client.
const RESYNC_INTERVAL: Duration = Duration::from_secs(15);
/// Hash reports are kept this long, and checked once this old (a client in
/// the background reports about 1000 ms behind the match clock).
const HASH_KEEP_MS: u32 = 30_000;
const HASH_SETTLE_MS: u32 = 2_500;
/// The match ends this long after the clients' time limit.
const END_GRACE_MS: u32 = 2_000;
/// A match without a client for this long is over.
const ABANDONED: Duration = Duration::from_secs(60);
/// A match everyone in it left is over at once (a player pressing Play went
/// back into the empty match for the next minute), once every seat came in
/// or the match ran this long (a seat still loading may yet come).
const SEATS_COME_WITHIN_MS: u32 = 60_000;
/// How often a running match asks for a fresh snapshot: late joiners then
/// start close to now.
const REFRESH: Duration = Duration::from_secs(5);
/// At the time limit, how long the match waits for a last snapshot that
/// the others confirm (it asks again every `SNAPSHOT_WAIT`).
const FINAL_WAIT: Duration = Duration::from_secs(7);
/// An upload is for a tick at most this far behind the match clock (one in
/// the background is about 1000 ms behind) ...
const UPLOAD_MAX_AGE_MS: u32 = 5_000;
/// ... and at most this far ahead of it (an upload is the event-horizon
/// item, always behind).
const UPLOAD_MAX_AHEAD_MS: u32 = HORIZON_MS;
/// Hash reports are for the event-horizon item: never this far ahead.
const HASH_MAX_AHEAD_MS: u32 = 1_000;
/// Inputs older than this are dropped without an answer: a client resends
/// for a few seconds at most.
const INPUT_MAX_AGE_MS: u32 = 5_000;
/// Snapshots that were rejected or never checked are saved at most this
/// many times a match (they are for finding desyncs, and must not fill the
/// disk).
const MAX_DOUBTFUL_DUMPS: u32 = 40;
/// A client out of step uploads its world once, before it is resynced, for
/// this many resyncs of a match: to compare it with the others' offline.
const MAX_DIAGNOSTICS: u32 = 8;
/// A client just resynced uploads its world once more, this long after it is
/// in step again and its clock is corrected (and a healthy client is asked at
/// the same moment), for this many resyncs of a match: to see whether the
/// world it starts from is the others', which the world it had before the
/// resync (`MAX_DIAGNOSTICS`) cannot show. Only for the log and the dumps.
const AFTER_RESYNC_WAIT: Duration = Duration::from_millis(1_500);
/// The check waits out a clock correction this long at most.
const AFTER_RESYNC_MAX_WAIT: Duration = Duration::from_secs(10);
const MAX_AFTER_RESYNC_CHECKS: u32 = 8;
/// Inputs dropped while a client is not live are remembered up to this many.
const UNSYNCED_KEPT: usize = 256;
/// The inputs of the match are kept this long (match clock, ms) and at most
/// this many, to be logged when a client is found out of step. A resync that
/// failed is found out 15 s (`RESYNC_INTERVAL`) after it went wrong, so they
/// go back 20 s: 8 players at 20 inputs a second is 3200.
const TRAIL_KEPT_MS: u32 = 20_000;
const TRAIL_KEPT: usize = 8_192;
/// The inputs around a divergence are logged for this many of a match.
const MAX_TRAILS: u32 = 8;
/// A resynced client's clock reports are logged for this long after it is
/// live again, for this many lines a match.
const CLOCK_LOG_FOR: Duration = Duration::from_secs(12);
const MAX_CLOCK_LOGS: u32 = 40;
/// A resynced client plays again the ticks between its new clock and its old
/// one, and its input lists outlive the resync (`game_world_core::clear`
/// leaves them): a new input there is merged into the one it sent for that
/// tick before. Such a changed "copy" is logged for this long after a resync,
/// for this many lines a match.
const CHANGED_LOG_FOR: Duration = Duration::from_secs(5);
const MAX_CHANGED_LOGS: u32 = 40;
/// A resynced client is started at least this far behind the match clock
/// for the server to tell the inputs it made on its old clock (which come
/// soon after their tick) from those it makes on the new one (which come
/// that far after theirs): about 210-310 ms live (`Resynced::made_before`).
const RESYNC_LAG_TO_TELL_MS: i64 = 100;
/// Inputs made before a resync, dropped, logged this many times a match.
const MAX_BEFORE_RESYNC_LOGS: u32 = 40;
/// A 0x54 or 0x58 that reached a client this far past its horizon (the ack
/// may wait for the client's next datagram, ~33 ms) shows it lacks an input
/// the others have, or has one they lack: out of step from that tick
/// (`Game::missed_verdict`). The latest ones are kept, this many.
const MISSED_PAST_MS: i64 = 50;
const MISSED_KEPT: usize = 64;
/// Plausibility: a player's kills at most one per this long, plus one.
const MIN_MS_PER_KILL: u32 = 2_000;
/// Plausibility: batteries home at most one per this long, plus one.
const MIN_MS_PER_BATTERY: u32 = 10_000;
/// Clock samples kept per client; the smallest lag of them counts, as the
/// one least delayed on the way.
const CLOCK_SAMPLES: usize = 4;
/// A client clock this far off is corrected (0x55).
const CLOCK_TOLERANCE_MS: i64 = 10;
/// The client corrects its clock at 5% speed: 20 ms per ms of offset.
const CORRECTION_MS_PER_MS: u64 = 20;
/// After a correction ends, fresh samples are awaited this long.
const CORRECTION_SETTLE: Duration = Duration::from_secs(1);
/// Inputs first seen this late probably lost their first copy.
const LATE_INPUT_MS: u32 = 200;
/// How fast `Peer::late_share` follows: each input moves it this far.
const LATE_SHARE_WEIGHT: f32 = 0.01;
/// When clients disagree with no majority, the side whose clients' inputs
/// came on time wins if it outweighs the next side by this much (each
/// client counts 1 minus its late share). Small enough that two clients, one
/// with a clearly better link, are told apart.
const TRUST_MARGIN: f32 = 0.15;
/// A client is resynced only once its hashes have disagreed with the right
/// ones for this long (match time, over its own reports) ...
const DIVERGENCE_CONFIRM_MS: u32 = 1_000;
/// ... or for this long on the wall clock, if its reports stop coming or can
/// no longer be judged. A disagreement that goes away by itself (a world
/// that was out of step for a moment: a late relay or a rollback) costs the
/// client its clock and its inputs for seconds when it is resynced, and a
/// world found identical to the others' was resynced in matches 0x123 and
/// 0x125.
const DIVERGENCE_CONFIRM_MAX: Duration = Duration::from_secs(3);
/// How often clients' disagreeing hashes are logged at most.
const DISAGREEMENT_LOG: Duration = Duration::from_secs(10);
/// After the end, the clients get this long to confirm before the match goes.
const LINGER: Duration = Duration::from_secs(5);

/// Message ids (match.md §2).
mod id {
    pub const CONNECT: u8 = 0x40;
    pub const PORT_PROBE: u8 = 0x41;
    pub const READY: u8 = 0x42;
    pub const TIME_REPLY: u8 = 0x43;
    pub const CLOCK: u8 = 0x44;
    pub const INPUT: u8 = 0x45;
    pub const HASH: u8 = 0x46;
    pub const LEAVE: u8 = 0x47;

    pub const PORTS: u8 = 0x50;
    pub const STATIC_INFO: u8 = 0x51;
    pub const SET_TIME: u8 = 0x52;
    pub const CLOCK_OFFSET: u8 = 0x55;
    pub const SNAPSHOT: u8 = 0x53;
    pub const INPUT_RELAY: u8 = 0x54;
    pub const PLAYER_ENTERED: u8 = 0x56;
    pub const PLAYER_LEFT: u8 = 0x57;
    pub const INPUT_DISCARD: u8 = 0x58;
    /// Patched clients answer it with an upload of their state.
    pub const UPLOAD_REQUEST: u8 = 0x59;
}

/// An input as 0x45 carries it: `f32 dx, f32 dy, u32 actions, u32 time`.
const INPUT_LEN: usize = 16;
/// A leave (0x47) carries only the client's clock; longer ones are chunks.
const LEAVE_LEN: usize = 4;
const CHUNK_HEADER: usize = 8;

/// The next tick at or after `time`.
fn grid_up(time: u32) -> u32 {
    time.div_ceil(TICK_MS) * TICK_MS
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    /// Sent 0x50; waits for the port probe (0x41).
    Connecting,
    /// Sent 0x51; the client loads the level until it is ready (0x42).
    Loading,
    /// Ready, waiting for a snapshot to start from.
    WaitingSnapshot { since: Instant, entering: bool },
    /// Sent 0x52 and 0x53; the rest follows once 0x53 is acked.
    Syncing {
        serial: u64,
        time: u32,
        entering: bool,
    },
    /// Simulating: gets every relayed input and roster change.
    Live,
    /// Left, or was told the match ended; its transport winds down.
    Gone,
}

struct Peer {
    addr: SocketAddr,
    session_id: u32,
    transport: Transport,
    stage: Stage,
    /// It uploaded a snapshot, so it answers 0x59.
    patched: bool,
    /// It was asked for an upload (0x59) and hasn't given one since: the
    /// only time its upload is taken, but for the initial world.
    asked: bool,
    /// Its upload is only to be saved (`diagnostic`), never used.
    diagnostic: bool,
    /// Likewise, but its world after a resync (`AFTER_RESYNC_WAIT`).
    after_resync: bool,
    /// The ticks of its own inputs that came while it was loading a snapshot
    /// (it is not live then) and were dropped, until it is live again
    /// (`UNSYNCED_KEPT` at most). The client has applied them to its own
    /// world and nobody else gets them.
    unsynced_inputs: Vec<u32>,
    /// Its clock reports are logged until then (it was resynced), and how
    /// far behind the match clock the server started it, and when.
    clock_log_until: Option<Instant>,
    sync_lag: Option<i64>,
    sync_sent_at: Option<Instant>,
    /// Inputs it sent since its last resync for a tick it had sent an input
    /// for before, with other contents (`CHANGED_LOG_FOR`).
    inputs_changed: u32,
    /// Back in a match it left: when it entered for the others, before it
    /// was started (`enter_early`), until it is live.
    entered_early: Option<u32>,
    /// When to ask it for that, and since when it waits.
    check_after: Option<(Instant, Instant)>,
    upload: Option<Upload>,
    /// The ticks of its inputs already handled.
    inputs_seen: HashSet<u32>,
    last_resync: Option<Instant>,
    /// When it last started from a snapshot.
    synced_at: Option<Instant>,
    /// The latest estimates of how far its clock is behind the match clock.
    lags: VecDeque<i64>,
    /// Its last round trip (0x52 → 0x43), in ms.
    round_trip: u32,
    /// A clock correction runs until then.
    correcting_until: Option<Instant>,
    /// Inputs received, those first seen over `LATE_INPUT_MS` late, and the
    /// latest.
    inputs_received: u32,
    inputs_late: u32,
    /// Inputs too late to relay (0x58 sent back), and times it was resynced.
    inputs_discarded: u32,
    resyncs: u32,
    latest_input_ms: u32,
    /// The recent share of its inputs that came late (0 to 1, a moving
    /// average over its last ~100 inputs): a client whose packets are late
    /// usually also gets the others' late, so it is the likelier odd one out
    /// when clients disagree.
    late_share: f32,
    /// Since when a resync of it has been put off because its clock is still
    /// being corrected.
    resync_deferred_since: Option<Instant>,
    /// The ticks it has been disagreeing on, if it is: not yet a reason to
    /// resync it (`DIVERGENCE_CONFIRM_MS`).
    diverged: Option<Divergence>,
    /// Its reports for ticks up to this are from before it started from its
    /// last snapshot (they reach the check 2.5 s late) and say nothing.
    judge_from: u32,
    /// Inputs discarded as too late since `DiscardRun::since`, not logged yet.
    discard_run: Option<DiscardRun>,
    /// Its clock's lag behind the match clock as last measured or set (ms),
    /// and the correction (0x55) running since: when, by how much, how long.
    lag_ms: Option<i64>,
    correction: Option<(Instant, i64, Duration)>,
    /// Its last resync while it played, to tell the inputs it made before it
    /// that come after it.
    resynced: Option<Resynced>,
    /// Its own ticks between the snapshot and its new clock it is told to
    /// drop once it has the snapshot (`send_sync`, `finish_sync`).
    discard_after_load: Vec<u32>,
    /// Inputs it made before a resync that came after it, dropped.
    inputs_before_resync: u32,
    /// Ticks whose 0x54 or 0x58 reached it past its horizon since it last
    /// started from a snapshot (`MISSED_PAST_MS`), the latest `MISSED_KEPT`.
    missed: VecDeque<u32>,
    /// The 0x54 relays and 0x58 discards sent to it and not acked yet.
    in_flight: VecDeque<InFlight>,
    /// Those that reached it after its horizon, not logged yet.
    late_run: Option<LateRun>,
    /// 0x58 and 0x54 acked by it, and of those, how many reached it late.
    discards_acked: u32,
    discards_late: u32,
    relays_acked: u32,
    relays_late: u32,
}

/// An input that came: when (match clock), its tick, from whom, and what
/// became of it: `R` relayed, `D` dropped as too late, `U` sent while its
/// sender loaded a snapshot, dropped, `B` made before its sender's resync and
/// come after it, dropped, `C` a copy with other contents, dropped.
struct TrailEntry {
    at: u32,
    tick: u32,
    player: u8,
    kind: u8,
}

/// Remember an input that came, forgetting the old ones.
fn note_input(trail: &mut VecDeque<TrailEntry>, at: u32, tick: u32, player: usize, kind: u8) {
    trail.push_back(TrailEntry {
        at,
        tick,
        player: player as u8,
        kind,
    });
    while trail.len() > TRAIL_KEPT || trail.front().is_some_and(|e| e.at + TRAIL_KEPT_MS < at) {
        trail.pop_front();
    }
}

/// A resync of a client while it played: the snapshot it started from, and
/// its new clock, and the match clock then.
#[derive(Clone, Copy)]
struct Resynced {
    snapshot: u32,
    start: u32,
    clock: u32,
}

impl Resynced {
    /// Whether an own input for `time`, come at `clock`, was made on the
    /// client's clock from before the resync. The others must never get it:
    /// its world, which kept it (the input lists outlive a resync), plays it
    /// elsewhere or not at all.
    /// - A tick between the snapshot and the new clock: the new clock never
    ///   plays one, and the client was told to drop them all.
    /// - A tick from the new clock on, up to how far ahead the old clock may
    ///   have been: one made on the new clock comes at least about as far
    ///   after its tick as the clock was started behind; one made on the old
    ///   clock comes right after it. Relayed, the client's next local input
    ///   pulls that node back to its own tick (0x1CB3AD): out of step again
    ///   (matches 0x12c and 0x12e, 2026-10-11).
    fn made_before(&self, time: u32, clock: u32) -> bool {
        if time <= self.snapshot || time > self.clock + CLEAR_AHEAD_MS {
            return false;
        }
        if time < self.start {
            return true;
        }
        let lag = i64::from(self.clock) - i64::from(self.start);
        lag >= RESYNC_LAG_TO_TELL_MS && i64::from(clock) - i64::from(time) < lag / 2
    }
}

impl Peer {
    /// How long since the server sent it the snapshot it started from.
    fn synced_since(&self, now: Instant) -> Duration {
        self.sync_sent_at
            .map_or(Duration::ZERO, |at| now.saturating_duration_since(at))
    }
}

/// A client that is loading its level (0x50 sent, 0x42 not yet) sends and
/// acks nothing until it is done, which took a few clients over 10 s: the
/// time they get before they count as gone.
const LOADING_TIMEOUT: Duration = Duration::from_secs(60);

/// A 0x54 or 0x58 sent to a client, waiting for its ack.
struct InFlight {
    serial: u64,
    /// A 0x58 (its own input dropped), else a 0x54 (another's relayed).
    discard: bool,
    /// The input's tick.
    tick: u32,
}

/// 0x54 and 0x58 that reached a client after its horizon, in a short run.
struct LateRun {
    since: Instant,
    discards: u32,
    relays: u32,
    /// How far past its horizon the latest of them came, in ms.
    worst_ms: i64,
    /// The first and last tick concerned.
    first_tick: u32,
    last_tick: u32,
}

/// A client whose hashes disagree with the right ones, in a row.
struct Divergence {
    /// The first and the latest tick it was wrong on.
    first: u32,
    last: u32,
    /// When it was first found wrong.
    since: Instant,
}

/// Inputs of one client discarded as too late (0x58) in a short run.
struct DiscardRun {
    since: Instant,
    count: u32,
    /// The latest of them, in ms past the match clock.
    worst_ms: u32,
}

/// A snapshot coming in 0x47 chunks.
struct Upload {
    time: u32,
    data: Vec<u8>,
    /// Offset and length of each chunk received.
    chunks: BTreeMap<u16, usize>,
}

impl Upload {
    fn received(&self) -> usize {
        self.chunks.values().sum()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Snapshot {
    time: u32,
    /// `game_state_history_item::serialize(mask)`: 0x53 without its time.
    body: Vec<u8>,
}

/// A roster place as the results show it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RosterEntry {
    pub name: String,
    pub team: u8,
    pub level: u8,
    /// The dict ids of its first two weapons.
    pub weapons: [u16; 2],
}

/// How a match is played, from its options.
#[derive(Clone, Debug, PartialEq)]
pub struct Setup {
    pub roster: Vec<RosterEntry>,
    pub countdown_seconds: u8,
    pub duration_seconds: u16,
    pub batteries: u8,
    pub score_weights: [f32; snapshot::EVENTS],
    /// Each player's supplies as the match started, by roster index.
    pub loadouts: Vec<Loadout>,
}

/// Stack items' counts, by dict id.
pub type Stacks = BTreeMap<u16, u32>;

/// How a match ended, for the results screen and the rewards.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report {
    pub match_id: u32,
    /// `game_team_id`: 0, 1, 2 (a draw) or 3 (unknown).
    pub winner: u8,
    /// Batteries in each team's container.
    pub team_points: [u8; 2],
    /// By roster index.
    pub players: Vec<PlayerStats>,
    /// By roster index: the stacks owned as the match started
    /// ([`Loadout::owned`]) and what the last readable snapshot had left of
    /// them (`None`: never read, so nothing is taken).
    pub supplies: Vec<(Stacks, Option<Stacks>)>,
    /// By roster index: whether the player was ever in the match (entered).
    /// One whose game never got that far neither won nor lost.
    pub entered: Vec<bool>,
}

/// An upload that waits to be checked against the hash reports.
struct Candidate {
    from: usize,
    snapshot: Snapshot,
    since: Instant,
}

pub struct Game {
    pub id: u32,
    port: u16,
    /// 0x51 without the local index.
    static_info: Vec<u8>,
    peers: Vec<Option<Peer>>,
    /// Set when the first snapshot comes: the match clock runs from there.
    clock_start: Option<Instant>,
    snapshot: Option<Snapshot>,
    /// The newest upload no majority contradicts, checked or not: a client
    /// starting now needs one this fresh.
    recent: Option<Snapshot>,
    requested_at: Option<Instant>,
    candidates: Vec<Candidate>,
    /// Every relayed input, for clients that start from a snapshot.
    inputs: Vec<(u32, usize, [u8; INPUT_LEN])>,
    /// Roster changes: time, index, entered (else left).
    events: Vec<(u32, usize, bool)>,
    /// CRC reports by tick and client.
    hashes: BTreeMap<u32, HashMap<usize, u32>>,
    hashes_checked_until: u32,
    first_entered: Option<u32>,
    countdown_ms: u32,
    duration_ms: u32,
    finished_at: Option<Instant>,
    empty_since: Option<Instant>,
    next_refresh: Option<Instant>,
    disagreement_logged: Option<Instant>,
    /// Where accepted snapshots are saved, if anywhere.
    dump_dir: Option<PathBuf>,
    /// Rejected or unchecked snapshots saved so far.
    doubtful_dumps: u32,
    /// Out-of-step clients asked for their world so far.
    diagnostics: u32,
    /// Resynced clients asked for their world afterwards so far.
    after_resync_checks: u32,
    /// The inputs of the last seconds (`TRAIL_KEPT_MS`).
    trail: VecDeque<TrailEntry>,
    /// Divergences whose inputs were logged, and clock reports logged.
    trails_logged: u32,
    clock_logs: u32,
    changed_logs: u32,
    before_resync_logs: u32,
    /// The match clock when the pings were last logged.
    pings_logged_at: u32,
    setup: Setup,
    /// The newest state read from a snapshot, and its time.
    state: Option<(u32, Decoded)>,
    /// The last statistics read (a finished client detaches them).
    statistics: Option<Vec<PlayerStatistics>>,
    /// Each player's longest killstreak seen.
    best_streaks: Vec<u16>,
    /// What each player had left of the supplies in the newest snapshot
    /// they could be read from.
    left: Vec<Option<BTreeMap<u16, u32>>>,
    /// The end has come: waiting for a last snapshot since then.
    ending: Option<Instant>,
    report: Option<Report>,
    /// The roster places whose players left by their own word (0x47) and
    /// were not asked about yet ([`Game::take_quits`]).
    quits: Vec<usize>,
}

impl Game {
    /// A match whose clients connect to `port`.
    pub fn new(id: u32, port: u16, static_info: Vec<u8>, setup: Setup, now: Instant) -> Self {
        let players = setup.roster.len();
        Self {
            id,
            port,
            static_info,
            peers: (0..players).map(|_| None).collect(),
            clock_start: None,
            snapshot: None,
            recent: None,
            requested_at: None,
            candidates: Vec::new(),
            inputs: Vec::new(),
            events: Vec::new(),
            hashes: BTreeMap::new(),
            hashes_checked_until: 0,
            first_entered: None,
            countdown_ms: u32::from(setup.countdown_seconds) * 1000,
            duration_ms: u32::from(setup.duration_seconds) * 1000,
            finished_at: None,
            empty_since: Some(now),
            next_refresh: None,
            disagreement_logged: None,
            dump_dir: None,
            doubtful_dumps: 0,
            diagnostics: 0,
            after_resync_checks: 0,
            trail: VecDeque::new(),
            trails_logged: 0,
            clock_logs: 0,
            changed_logs: 0,
            before_resync_logs: 0,
            pings_logged_at: 0,
            best_streaks: vec![0; players],
            left: vec![None; players],
            setup,
            state: None,
            statistics: None,
            ending: None,
            report: None,
            quits: Vec::new(),
        }
    }

    /// How the match ended, once it has: taken once.
    pub fn take_report(&mut self) -> Option<Report> {
        self.report.take()
    }

    /// The roster places whose players left the match by their own word
    /// since this was last asked. Such a player has given its seat up
    /// (`Server::receive`).
    ///
    /// The 4-byte 0x47 is that word. The client sends it at one place
    /// only, in `network_client::close_current_match` for the reason 2,
    /// "disconnected_by_initiator" (0x1A2417 -> 0x1A2440, the send at
    /// 0x1A2448). The reason 1, "disconnected_by_connection_lost", goes
    /// straight to 0x1A249A and sends nothing on the match link, and a
    /// game that died sends nothing at all. PROVEN.
    pub fn take_quits(&mut self) -> Vec<usize> {
        std::mem::take(&mut self.quits)
    }

    /// Read a snapshot's statistics and rules. A team with every battery
    /// home ends the match.
    fn observe(&mut self, snapshot: &Snapshot, now: Instant) {
        let players = self.setup.roster.len();
        let batteries = usize::from(self.setup.batteries);
        let Some(decoded) = snapshot::decode(&snapshot.body, players, batteries) else {
            log::warn!(
                "Match {:#x}: the snapshot at {} ms could not be read",
                self.id,
                snapshot.time
            );
            return;
        };
        if self
            .state
            .as_ref()
            .is_some_and(|(time, _)| *time > snapshot.time)
        {
            return;
        }
        if let Err(why) = self.plausible(&decoded, snapshot.time) {
            log::warn!(
                "Match {:#x}: the snapshot at {} ms is not plausible ({why}); its results \
                 are not used",
                self.id,
                snapshot.time
            );
            return;
        }
        if let Some(statistics) = &decoded.statistics {
            for (best, player) in self.best_streaks.iter_mut().zip(statistics) {
                *best = (*best).max(player.killstreak);
            }
            self.statistics = Some(statistics.clone());
        }
        let all_home = batteries > 0
            && decoded
                .team_points
                .iter()
                .any(|points| usize::from(*points) >= batteries);
        log::debug!(
            "Match {:#x}: at {} ms batteries {:?}, kills and deaths {:?}",
            self.id,
            snapshot.time,
            decoded.team_points,
            decoded.kills_deaths
        );
        let dictionary = survarium::dictionary::Dictionary::shared();
        for (index, loadout) in self.setup.loadouts.iter().enumerate() {
            if let Some(left) =
                crate::supplies::left(&snapshot.body, index as u8, loadout, dictionary)
            {
                self.left[index] = Some(left);
            }
        }
        self.state = Some((snapshot.time, decoded));
        if all_home && self.finished_at.is_none() {
            log::info!(
                "Match {:#x}: a team has all {batteries} batteries home at {} ms",
                self.id,
                snapshot.time
            );
            self.finish(now);
        }
    }

    /// Whether a snapshot's results could have come from real play: no more
    /// batteries home than there are, or than could have been carried since
    /// the match began, no more kills than deaths, and no player killing
    /// faster than one every [`MIN_MS_PER_KILL`]. Two colluding clients can
    /// agree on any CRC; this keeps what they can claim within play.
    fn plausible(&self, decoded: &Decoded, time: u32) -> Result<(), String> {
        let batteries = self.setup.batteries;
        let played = time.saturating_sub(self.first_entered.unwrap_or(T0));
        let carried = 1 + played / MIN_MS_PER_BATTERY;
        let home = u32::from(decoded.team_points[0]) + u32::from(decoded.team_points[1]);
        if decoded.team_points.iter().any(|points| *points > batteries) {
            return Err(format!(
                "batteries home {:?} of {batteries}",
                decoded.team_points
            ));
        }
        if home > carried {
            return Err(format!("{home} batteries home after {played} ms"));
        }
        if let Some(statistics) = &decoded.statistics {
            let kills: u32 = statistics
                .iter()
                .map(|player| u32::from(player.events[event::KILL]))
                .sum();
            let deaths: u32 = statistics
                .iter()
                .map(|player| u32::from(player.events[event::DEATH]))
                .sum();
            // A little slack: a place a player comes back to may start its
            // count again.
            if kills > deaths + statistics.len() as u32 {
                return Err(format!("{kills} kills but {deaths} deaths"));
            }
            let most = 1 + played / MIN_MS_PER_KILL;
            if let Some((index, player)) = statistics
                .iter()
                .enumerate()
                .find(|(_, player)| u32::from(player.events[event::KILL]) > most)
            {
                return Err(format!(
                    "player {index} has {} kills after {played} ms",
                    player.events[event::KILL]
                ));
            }
        }
        Ok(())
    }

    /// The results from the last statistics, with what the client adds at
    /// the end (`shared_statistics::finish_match`): participation for those
    /// still in, the win, and the winners' battery bonus.
    fn build_report(&self, present: &[bool]) -> Report {
        let (winner, team_points) = match &self.state {
            Some((_, decoded)) => (snapshot::winner(decoded.team_points), decoded.team_points),
            None => (NO_WINNER, [0, 0]),
        };
        // Which teams stole a battery: the other team's base was robbed.
        let mut stole = [false; 2];
        for (entry, player) in self
            .setup
            .roster
            .iter()
            .zip(self.statistics.iter().flatten())
        {
            if player.events[event::STEAL_VICTORY_ITEM] > 0 && entry.team < 2 {
                stole[usize::from(entry.team)] = true;
            }
        }
        let players = self
            .setup
            .roster
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                let statistics = self
                    .statistics
                    .as_ref()
                    .and_then(|statistics| statistics.get(index));
                let mut events = statistics.map(|player| player.events).unwrap_or_default();
                if present[index] {
                    events[event::PARTICIPATING] += 1;
                }
                if entry.team == winner {
                    events[event::WON] += 1;
                    events[event::WINNING_TEAM_VI_BONUS] +=
                        u16::from(team_points[usize::from(winner)]);
                }
                PlayerStats {
                    id: index as u32 + 1,
                    name: entry.name.clone(),
                    team: entry.team,
                    level: entry.level,
                    score: snapshot::score(&events, &self.setup.score_weights),
                    kills: events[event::KILL],
                    deaths: events[event::DEATH],
                    assists: events[event::ASSIST],
                    healed: events[event::TEAMMATE_CURE],
                    saved: events[event::TEAMMATE_RESCUE],
                    headshots: events[event::HEADSHOT_KILL],
                    silent_kills: events[event::SILENT_KILL],
                    melee_kills: events[event::MELEE_KILL],
                    stolen_x100: events[event::STEAL_VICTORY_ITEM],
                    found: events[event::VICTORY_ITEM_FOUND],
                    defended: events[event::RECOVER_VICTORY_ITEM] / 100,
                    artefacts: events[event::ARTEFACT],
                    max_killstreak: self.best_streaks[index],
                    shots: events[event::BULLET_FIRE],
                    hits: events[event::BULLET_HIT],
                    weapons: entry.weapons,
                    finished: present[index],
                    won: entry.team == winner,
                    delivered: events[event::PUT_VICTORY_ITEM_IN_CONTAINER],
                    faction_kills: statistics
                        .map(|player| player.faction_kills)
                        .unwrap_or_default(),
                    team_batteries: team_points
                        .get(usize::from(entry.team))
                        .copied()
                        .unwrap_or(0),
                    base_robbed: entry.team < 2 && stole[usize::from(1 - entry.team)],
                }
            })
            .collect();
        let supplies = (0..self.setup.roster.len())
            .map(|index| {
                let owned = self
                    .setup
                    .loadouts
                    .get(index)
                    .map(|loadout| loadout.owned.clone())
                    .unwrap_or_default();
                (owned, self.left[index].clone())
            })
            .collect();
        let entered = (0..self.setup.roster.len())
            .map(|index| {
                self.events
                    .iter()
                    .any(|(_, who, entered)| *who == index && *entered)
            })
            .collect();
        Report {
            match_id: self.id,
            winner,
            team_points,
            players,
            supplies,
            entered,
        }
    }

    /// End the match: the results are kept, and every client gets 0x57 with
    /// its own index, shows the result screen and goes back to the lobby.
    fn finish(&mut self, now: Instant) {
        if self.finished_at.is_some() {
            return;
        }
        let clock = self.clock(now);
        let present: Vec<bool> = self
            .peers
            .iter()
            .map(|peer| peer.as_ref().is_some_and(|peer| peer.stage == Stage::Live))
            .collect();
        let report = self.build_report(&present);
        log::info!(
            "Match {:#x}: over at {clock} ms; winner {}, batteries {:?}, scores {:?}",
            self.id,
            report.winner,
            report.team_points,
            report
                .players
                .iter()
                .map(|player| player.score)
                .collect::<Vec<_>>()
        );
        self.report = Some(report);
        for (index, peer) in self.peers.iter_mut().enumerate() {
            if let Some(peer) = peer
                && peer.stage != Stage::Gone
            {
                send(peer, id::PLAYER_LEFT, &roster_change(index, clock));
                log::info!(
                    "Match {:#x}: player {index}: {}",
                    self.id,
                    input_summary(peer)
                );
                peer.stage = Stage::Gone;
            }
        }
        self.finished_at = Some(now);
    }

    /// The match clock in ms: `T0` until the first snapshot, then running.
    fn clock(&self, now: Instant) -> u32 {
        match self.clock_start {
            Some(start) => T0 + now.saturating_duration_since(start).as_millis() as u32,
            None => T0,
        }
    }

    /// The roster index of the client at `addr`.
    pub fn peer_index(&self, addr: SocketAddr) -> Option<usize> {
        self.peers
            .iter()
            .position(|peer| peer.as_ref().is_some_and(|peer| peer.addr == addr))
    }

    /// The sessions of the clients connected.
    pub fn sessions(&self) -> Vec<u32> {
        self.peers
            .iter()
            .flatten()
            .filter(|peer| peer.stage != Stage::Gone)
            .map(|peer| peer.session_id)
            .collect()
    }

    /// A client connected (0x40) for roster place `index`. A client already
    /// there is replaced: it left.
    pub fn attach(
        &mut self,
        index: usize,
        addr: SocketAddr,
        session_id: u32,
        transport: Transport,
        now: Instant,
    ) {
        if index >= self.peers.len() || self.finished_at.is_some() {
            return;
        }
        if self.peers[index].is_some() {
            self.leave(index, now);
        }
        log::info!(
            "Match {:#x}: player {index} (session {session_id:#x}) connected from {addr}",
            self.id
        );
        let mut peer = Peer {
            addr,
            session_id,
            transport,
            stage: Stage::Connecting,
            patched: false,
            asked: false,
            diagnostic: false,
            after_resync: false,
            unsynced_inputs: Vec::new(),
            clock_log_until: None,
            sync_lag: None,
            sync_sent_at: None,
            inputs_changed: 0,
            entered_early: None,
            check_after: None,
            upload: None,
            inputs_seen: HashSet::new(),
            last_resync: None,
            synced_at: None,
            lags: VecDeque::new(),
            round_trip: 0,
            late_share: 0.0,
            resync_deferred_since: None,
            diverged: None,
            judge_from: 0,
            discard_run: None,
            lag_ms: None,
            correction: None,
            resynced: None,
            discard_after_load: Vec::new(),
            inputs_before_resync: 0,
            missed: VecDeque::new(),
            in_flight: VecDeque::new(),
            late_run: None,
            discards_acked: 0,
            discards_late: 0,
            relays_acked: 0,
            relays_late: 0,
            correcting_until: None,
            inputs_received: 0,
            inputs_late: 0,
            inputs_discarded: 0,
            resyncs: 0,
            latest_input_ms: 0,
        };
        // One port: the client probes it once and sends everything there.
        let mut ports = self.port.to_le_bytes().to_vec();
        ports.extend(self.port.to_le_bytes());
        send(&mut peer, id::PORTS, &ports);
        self.peers[index] = Some(peer);
        self.empty_since = None;
    }

    /// A datagram from the client at roster place `index`.
    pub fn receive(&mut self, index: usize, datagram: &[u8], now: Instant) {
        let Some(peer) = self.peers.get_mut(index).and_then(Option::as_mut) else {
            return;
        };
        let events = peer.transport.receive(datagram, now);
        self.check_in_flight(index, now);
        for event in events {
            match event {
                Event::Message { id, payload } => self.message(index, id, &payload, now),
                Event::Disconnected => self.leave(index, now),
            }
        }
    }

    fn message(&mut self, index: usize, message: u8, payload: &[u8], now: Instant) {
        let clock = self.clock(now);
        let Some(peer) = self.peers[index].as_mut() else {
            return;
        };
        match message {
            id::CONNECT => {}
            id::PORT_PROBE => {
                if peer.stage == Stage::Connecting {
                    let mut info = self.static_info.clone();
                    info.push(index as u8);
                    send(peer, id::STATIC_INFO, &info);
                    peer.stage = Stage::Loading;
                }
            }
            id::READY => {
                if peer.stage == Stage::Loading {
                    log::info!("Match {:#x}: player {index} has loaded", self.id);
                    if self.returns(index) {
                        self.enter_early(index, now);
                    }
                    self.start_sync(index, true, now);
                }
            }
            // `u32 echo, u32 client clock`, sent as soon as 0x52 set it.
            id::TIME_REPLY => {
                if let Ok(reply) = <[u8; 8]>::try_from(payload) {
                    let echo = u32::from_le_bytes(reply[..4].try_into().unwrap());
                    let client = u32::from_le_bytes(reply[4..].try_into().unwrap());
                    peer.round_trip = clock.saturating_sub(echo);
                    log::debug!(
                        "Match {:#x}: player {index}'s round trip is {} ms",
                        self.id,
                        peer.round_trip
                    );
                    sample_lag(peer, client, clock);
                    self.log_clock_report(index, now);
                }
            }
            // `u32 client clock`, every second.
            id::CLOCK => {
                if let Ok(client) = <[u8; 4]>::try_from(payload) {
                    sample_lag(peer, u32::from_le_bytes(client), clock);
                    self.log_clock_report(index, now);
                    self.correct_clock(index, now);
                }
            }
            id::INPUT => self.input(index, payload, now),
            id::HASH => {
                if let Ok(report) = <[u8; 8]>::try_from(payload) {
                    let time = u32::from_le_bytes(report[..4].try_into().unwrap());
                    let crc = u32::from_le_bytes(report[4..].try_into().unwrap());
                    // The client's debug hotkey sends this instead of a hash.
                    // Only ticks still to be checked, and none from the
                    // future: they would pile up, or vouch for a forgery
                    // ahead of time.
                    if crc != u32::MAX
                        && time >= self.hashes_checked_until
                        && time <= clock + HASH_MAX_AHEAD_MS
                        && time.is_multiple_of(TICK_MS)
                    {
                        self.hashes.entry(time).or_default().insert(index, crc);
                    }
                }
            }
            id::LEAVE if payload.len() == LEAVE_LEN => {
                // By its own will, while the match runs: its seat is given
                // up (`take_quits`).
                if self.finished_at.is_none() && peer.stage != Stage::Gone {
                    self.quits.push(index);
                }
                self.leave(index, now);
            }
            id::LEAVE => self.chunk(index, payload, now),
            _ => log::debug!(
                "Match {:#x}: message {message:#04x} from player {index}; ignored",
                self.id
            ),
        }
    }

    /// Whether a client other than `index` is simulating.
    fn others_live(&self, index: usize) -> bool {
        self.peers.iter().enumerate().any(|(other, peer)| {
            other != index && peer.as_ref().is_some_and(|peer| peer.stage == Stage::Live)
        })
    }

    /// The snapshot the client at `index` can start from now: the newest
    /// after its own last roster change (an older one has the player as it
    /// was, which the others no longer have), and with others simulating,
    /// fresh enough (`max_age` ms behind the match clock) that every input
    /// since it is still open on the client.
    fn usable_snapshot(&self, index: usize, clock: u32, max_age: u32) -> Option<Snapshot> {
        let own_last = self
            .events
            .iter()
            .filter(|(_, who, _)| *who == index)
            .map(|(at, _, _)| *at)
            .max();
        let alone = !self.others_live(index);
        let relayed_last = self.own_relayed_last(index, clock);
        [self.snapshot.as_ref(), self.recent.as_ref()]
            .into_iter()
            .flatten()
            .filter(|snapshot| own_last.is_none_or(|at| snapshot.time > at))
            .filter(|snapshot| alone || clock.saturating_sub(snapshot.time) <= max_age)
            // Its tick is final: no input for it or before it can be relayed
            // any more, and the client gets those after it. (The initial
            // world has no input before it.) An upload is ~500 ms old, but a
            // client whose clock is under ~500 ms uploads its newest item.
            .filter(|snapshot| {
                alone || snapshot.time == T0 || snapshot.time + HORIZON_MS <= clock
            })
            // Its clock starts past every input of its own the others have
            // (`RESYNC_UPLOAD_AFTER_MS`).
            .filter(|snapshot| {
                alone || relayed_last.is_none_or(|last| snapshot.time + SYNC_LEAD_MS > last)
            })
            .max_by_key(|snapshot| snapshot.time)
            .cloned()
    }

    /// The ticks of the inputs of `index` relayed to the others, for ticks
    /// after `after`. They are kept in the order they came, a tick at most
    /// `HORIZON_MS` behind the clock it came at.
    fn own_relayed(&self, index: usize, after: u32) -> impl Iterator<Item = u32> + '_ {
        self.inputs
            .iter()
            .rev()
            .take_while(move |(at, _, _)| *at + 4 * HORIZON_MS > after)
            .filter(move |(at, from, _)| *from == index && *at > after)
            .map(|(at, _, _)| *at)
    }

    /// The last tick of an input of `index` the others have, of the last
    /// seconds (an older one is before any snapshot it could start from).
    fn own_relayed_last(&self, index: usize, clock: u32) -> Option<u32> {
        self.own_relayed(index, clock.saturating_sub(FRESH_SNAPSHOT_MS))
            .max()
    }

    /// Whether a client waiting to be resynced may have an upload asked for:
    /// once the uploads are late enough that its clock starts past its last
    /// input the others have (`RESYNC_UPLOAD_AFTER_MS`).
    fn resync_upload_due(&self, index: usize, clock: u32) -> bool {
        self.own_relayed_last(index, clock)
            .is_none_or(|last| clock >= last + RESYNC_UPLOAD_AFTER_MS)
    }

    /// How old a snapshot a client may start from, after waiting `waited` for
    /// one: one fresh, so that it starts close to the match clock, else, once
    /// none came for a while, any fresh enough to have every input since it
    /// open. A newcomer as a client resynced: it runs as far behind.
    fn snapshot_max_age(waited: Duration) -> u32 {
        if waited >= RESYNC_FRESH_WAIT {
            FRESH_SNAPSHOT_MS
        } else {
            RESYNC_FRESH_MS
        }
    }

    /// The newest snapshot there is, however old.
    fn newest_snapshot(&self) -> Option<Snapshot> {
        [self.snapshot.as_ref(), self.recent.as_ref()]
            .into_iter()
            .flatten()
            .max_by_key(|snapshot| snapshot.time)
            .cloned()
    }

    /// Whether the player at `index` is back in a match it left, with
    /// others playing and a patched client to give a snapshot.
    fn returns(&self, index: usize) -> bool {
        self.events
            .iter()
            .any(|(_, who, entered)| *who == index && !*entered)
            && self.others_live(index)
            && !self.sources(index).is_empty()
    }

    /// A player back in a match it left enters for the others now, before it
    /// is started. The others still have its player as it was when it left,
    /// but a snapshot holds only the players in the match (its mask): one
    /// taken while it was away has none, the client makes its own, new one,
    /// and its world differs from the others' from the tick it enters (every
    /// rejoin, matches 0x123-0x126). A snapshot after this entry has its
    /// player as the others have it, and the client takes it in as any
    /// player of the mask (0x1CC6DA → `make_client_active` 0x1CC0E7, as a
    /// 0x56 does); `usable_snapshot` waits for one.
    fn enter_early(&mut self, index: usize, now: Instant) {
        let left = self
            .events
            .iter()
            .filter(|(_, who, _)| *who == index)
            .map(|(at, _, _)| *at)
            .max()
            .unwrap_or(0);
        let at = grid_up(self.clock(now).max(left + 1));
        self.broadcast(Some(index), id::PLAYER_ENTERED, &roster_change(index, at));
        self.events.push((at, index, true));
        self.first_entered.get_or_insert(at);
        if let Some(peer) = self.peers[index].as_mut() {
            peer.entered_early = Some(at);
        }
        log::info!(
            "Match {:#x}: player {index} entered at {at} ms, back in the match: for the others \
             first; it starts from a snapshot after that, which has its player as they do",
            self.id
        );
    }

    /// Start the client at `index` from a snapshot: at once if there is a
    /// fresh one, else once a patched client has uploaded one.
    fn start_sync(&mut self, index: usize, entering: bool, now: Instant) {
        let clock = self.clock(now);
        let max_age = Self::snapshot_max_age(Duration::ZERO);
        if let Some(snapshot) = self.usable_snapshot(index, clock, max_age) {
            self.send_sync(index, snapshot, entering, now);
            return;
        }
        let can_ask = !self.sources(index).is_empty();
        let early = self.peers[index]
            .as_ref()
            .is_some_and(|peer| peer.entered_early.is_some());
        if let Some(peer) = self.peers[index].as_mut() {
            peer.stage = Stage::WaitingSnapshot {
                since: now,
                entering,
            };
        }
        if early || (!entering && !self.resync_upload_due(index, clock)) {
            // Asked for once the uploads are past its entry, or past its
            // last input the others have (`poll`).
        } else if can_ask {
            self.request_upload(index, now, RESYNC_REQUEST_GAP);
        } else {
            log::info!(
                "Match {:#x}: player {index} waits for a snapshot; only a patched client \
                 (survarium_srp.exe) can give one",
                self.id
            );
        }
    }

    /// The patched clients in the match that can upload for `except` and
    /// are asked: the ones with the best links (inputs on time, then the
    /// shortest round trip), [`SNAPSHOT_SOURCES`] of them. A client with a
    /// bad link is the likelier to be out of step, and its uploads are the
    /// ones the others' hashes reject.
    fn sources(&self, except: usize) -> Vec<usize> {
        let mut sources: Vec<(f32, u32, usize)> = self
            .peers
            .iter()
            .enumerate()
            .filter_map(|(index, peer)| {
                let peer = peer.as_ref()?;
                (index != except && peer.patched && peer.stage == Stage::Live).then_some((
                    peer.late_share,
                    peer.round_trip,
                    index,
                ))
            })
            .collect();
        sources.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
        // At the end every one is asked: the results need a snapshot the
        // others' hashes agree with, and with only the best link asked
        // there would be none if it happened to be the odd one out.
        let live = self
            .peers
            .iter()
            .flatten()
            .filter(|peer| peer.stage == Stage::Live)
            .count();
        let asked = if self.ending.is_some() {
            usize::MAX
        } else if live < LARGE_MATCH_PLAYERS {
            1
        } else {
            SNAPSHOT_SOURCES
        };
        sources.truncate(asked);
        sources.into_iter().map(|(_, _, index)| index).collect()
    }

    /// Ask the best-linked clients for an upload, unless one was asked for
    /// less than `gap` ago.
    fn request_upload(&mut self, except: usize, now: Instant, gap: Duration) {
        if self
            .requested_at
            .is_some_and(|at| now.duration_since(at) < gap)
        {
            return;
        }
        let sources = self.sources(except);
        if sources.is_empty() {
            return;
        }
        for index in &sources {
            if let Some(peer) = self.peers[*index].as_mut() {
                send(peer, id::UPLOAD_REQUEST, &[]);
                peer.asked = true;
                peer.diagnostic = false;
            }
        }
        log::info!(
            "Match {:#x}: asked players {sources:?} for a snapshot",
            self.id
        );
        self.requested_at = Some(now);
    }

    /// Clients resynced a moment ago are asked for their world, and a healthy
    /// client at the same moment, so that both uploads are for the same tick
    /// and can be compared offline. It waits until the resynced client's clock
    /// is corrected (its uploads are for its clock) and nothing is asked of it.
    fn check_after_resyncs(&mut self, now: Instant) {
        if self.after_resync_checks >= MAX_AFTER_RESYNC_CHECKS {
            return;
        }
        for index in 0..self.peers.len() {
            let Some(peer) = self.peers[index].as_ref() else {
                continue;
            };
            let Some((due, since)) = peer.check_after else {
                continue;
            };
            if now < due {
                continue;
            }
            let waiting = now.duration_since(since);
            let gone = peer.stage != Stage::Live;
            // Its uploads are for its own clock: wait until it is the match's.
            let behind =
                estimated_lag(peer, now).is_none_or(|lag| lag.abs() > 2 * CLOCK_TOLERANCE_MS);
            let busy = peer.asked
                || behind
                || peer.correcting_until.is_some_and(|until| now < until)
                || self
                    .requested_at
                    .is_some_and(|at| now.duration_since(at) < RESYNC_REQUEST_GAP);
            if gone || waiting >= AFTER_RESYNC_MAX_WAIT {
                self.peers[index].as_mut().unwrap().check_after = None;
                continue;
            }
            if busy {
                continue;
            }
            let sources = self.sources(index);
            let Some(&source) = sources.first() else {
                continue;
            };
            if self.peers[source].as_ref().is_none_or(|peer| peer.asked) {
                continue;
            }
            for (who, mine) in [(index, true), (source, false)] {
                if let Some(peer) = self.peers[who].as_mut() {
                    send(peer, id::UPLOAD_REQUEST, &[]);
                    peer.asked = true;
                    peer.diagnostic = false;
                    peer.after_resync = mine;
                }
            }
            self.peers[index].as_mut().unwrap().check_after = None;
            self.after_resync_checks += 1;
            self.requested_at = Some(now);
            log::info!(
                "Match {:#x}: asked player {index} for its world after its resync, and player \
                 {source} to compare with",
                self.id
            );
        }
    }

    /// 0x52 then 0x53: the client wipes its world and starts from the
    /// snapshot. With others simulating, its clock starts `SYNC_LEAD_MS`
    /// past the snapshot so that the inputs since it are still open when
    /// they arrive (`finish_sync`); 0x55 then brings it to the match clock.
    fn send_sync(&mut self, index: usize, snapshot: Snapshot, entering: bool, now: Instant) {
        let clock = self.clock(now);
        let start = if self.others_live(index) {
            (snapshot.time + SYNC_LEAD_MS)
                .min(clock)
                .max(clock.saturating_sub(MAX_CLOCK_CORRECTION_MS))
        } else {
            clock
        };
        // A client resynced while playing drops its own inputs after the
        // snapshot that nobody else has (`CLEAR_AHEAD_MS`); the input lists
        // outlive the load, and nothing else removes them (notes §6). A
        // discard needs the input at exactly that tick and a world-history
        // item before it (`discard_input` 0x1CB54F, 0x1CEE73):
        // - from its new clock on, before it loads, in the old world: after
        //   the load its next local input would pull such an input back to
        //   its own tick (0x1CB3AD);
        // - before its new clock, after the load, when the snapshot is the
        //   item before them: the old world's history, ~500 ms, no longer
        //   reaches back to the first ticks after a snapshot 560-660 ms old,
        //   and a 0x58 there was refused, the input played again after the
        //   load (0x128 and 0x12a, 2026-10-11). The new clock never plays
        //   such a tick, nor pulls an input back to one.
        let resync = !entering && self.others_live(index);
        let kept: HashSet<u32> = if resync {
            self.own_relayed(index, snapshot.time).collect()
        } else {
            HashSet::new()
        };
        let (before, after): (Vec<u32>, Vec<u32>) = if resync {
            (snapshot.time / TICK_MS + 1..=(clock + CLEAR_AHEAD_MS) / TICK_MS)
                .map(|tick| tick * TICK_MS)
                .filter(|tick| !kept.contains(tick))
                .partition(|tick| *tick >= start)
        } else {
            (Vec::new(), Vec::new())
        };
        let Some(peer) = self.peers[index].as_mut() else {
            return;
        };
        for tick in &before {
            send(peer, id::INPUT_DISCARD, &tick.to_le_bytes());
        }
        if let (Some(first), Some(last)) = (
            after.first().or(before.first()),
            before.last().or(after.last()),
        ) {
            log::info!(
                "Match {:#x}: player {index} told to drop its own inputs for ticks {first} to {last}, \
                 {} before the snapshot and {} after it, but the {} the others have",
                self.id,
                before.len(),
                after.len(),
                kept.len()
            );
        }
        if resync {
            // What it sends from here for those ticks is new, not a copy.
            peer.inputs_seen
                .retain(|tick| *tick <= snapshot.time || kept.contains(tick));
            peer.resynced = Some(Resynced {
                snapshot: snapshot.time,
                start,
                clock,
            });
        } else {
            peer.resynced = None;
        }
        peer.discard_after_load = after.clone();
        peer.missed.clear();
        // Its reports up to now are of the world it leaves, and reach the
        // check up to 2.5 s from now.
        peer.diverged = None;
        peer.judge_from = clock;
        // Counted from here: what it sent before is of the world it leaves.
        peer.unsynced_inputs.clear();
        // `u32 echo` (the round trip), `u32 the client's new clock`.
        let mut set_time = clock.to_le_bytes().to_vec();
        set_time.extend(start.to_le_bytes());
        peer.lag_ms = Some(i64::from(clock) - i64::from(start));
        peer.correction = None;
        peer.sync_lag = peer.lag_ms;
        // What it reported before is of the clock it leaves: left in, the
        // smallest of them hid the lag of its new one (a few hundred ms) for
        // the next 3 or 4 reports, and the correction waited for them.
        peer.lags.clear();
        peer.sync_sent_at = Some(now);
        peer.inputs_changed = 0;
        peer.clock_log_until = (!entering).then_some(now + CLOCK_LOG_FOR);
        send(peer, id::SET_TIME, &set_time);
        let mut body = snapshot.time.to_le_bytes().to_vec();
        body.extend(&snapshot.body);
        let Some(serial) = send(peer, id::SNAPSHOT, &body) else {
            return;
        };
        // Right behind it, so as to reach it just after the load; again once
        // it has the snapshot (`finish_sync`), in case they came first.
        for tick in &after {
            send(peer, id::INPUT_DISCARD, &tick.to_le_bytes());
        }
        log::info!(
            "Match {:#x}: player {index} starts from the snapshot at {} ms ({} bytes) at {clock} ms, \
             its clock at {start} ms",
            self.id,
            snapshot.time,
            body.len()
        );
        // Only a client joining counts as the latest to start: a resync of
        // one already playing must not make it the next one blamed (it
        // would be resynced again and again, whoever is at fault).
        if entering {
            peer.synced_at = Some(now);
        }
        peer.stage = Stage::Syncing {
            serial,
            time: snapshot.time,
            entering,
        };
    }

    /// The snapshot reached the client: what happened since, and the player
    /// itself if it enters now.
    fn finish_sync(&mut self, index: usize, time: u32, entering: bool, now: Instant) {
        let inputs: Vec<_> = self
            .inputs
            .iter()
            .filter(|(at, _, _)| *at > time)
            .map(|(_, from, input)| relay(*from, input))
            .collect();
        // Never its own: a client told it left (0x57 with its own index)
        // closes the match and waits for the results.
        let early = self.peers[index]
            .as_ref()
            .and_then(|peer| peer.entered_early);
        // A tick of its own the others have is never discarded: a copy it
        // sent again (a new input merged into it) would take the one they
        // play out of its world too.
        let relayed: HashSet<u32> = self.own_relayed(index, time).collect();
        // Its own entry only if it entered early and the snapshot (given up
        // waiting for) is from before it.
        let events: Vec<_> = self
            .events
            .iter()
            .filter(|(at, who, entered)| {
                *at > time && (*who != index || (*entered && early.is_some()))
            })
            .copied()
            .collect();
        let Some(peer) = self.peers[index].as_mut() else {
            return;
        };
        for input in &inputs {
            send(peer, id::INPUT_RELAY, input);
        }
        for (at, who, entered) in events {
            let kind = if entered {
                id::PLAYER_ENTERED
            } else {
                id::PLAYER_LEFT
            };
            send(peer, kind, &roster_change(who, at));
        }
        peer.stage = Stage::Live;
        // Its own ticks between the snapshot and its new clock, again
        // (`send_sync`): the snapshot is loaded now.
        let after_load = std::mem::take(&mut peer.discard_after_load);
        for tick in &after_load {
            if let Some(serial) = send(peer, id::INPUT_DISCARD, &tick.to_le_bytes()) {
                track(peer, serial, true, *tick);
            }
        }
        let unsynced = std::mem::take(&mut peer.unsynced_inputs);
        if !unsynced.is_empty() {
            // Its own inputs the match never got. Those after the snapshot are
            // in its world and nobody else's: its predicted input stays until
            // a 0x58 removes it, which left its aim one input off everyone
            // else's and had it resynced again 15 s later.
            let after: Vec<u32> = unsynced
                .iter()
                .copied()
                .filter(|tick| *tick > time && !relayed.contains(tick) && !after_load.contains(tick))
                .collect();
            for tick in &after {
                peer.inputs_discarded += 1;
                if let Some(serial) = send(peer, id::INPUT_DISCARD, &tick.to_le_bytes()) {
                    track(peer, serial, true, *tick);
                }
            }
            log::info!(
                "Match {:#x}: player {index} sent {} inputs while it loaded the snapshot (ticks {} to {}), \
                 {} of them after the snapshot at {time} ms; dropped, and those discarded (0x58)",
                self.id,
                unsynced.len(),
                unsynced.iter().min().unwrap(),
                unsynced.iter().max().unwrap(),
                after.len()
            );
        }
        if !entering && peer.patched && self.after_resync_checks < MAX_AFTER_RESYNC_CHECKS {
            peer.check_after = Some((now + AFTER_RESYNC_WAIT, now));
        }
        log::info!(
            "Match {:#x}: player {index} is in sync ({} inputs since the snapshot)",
            self.id,
            inputs.len()
        );
        if let Some(peer) = self.peers[index].as_mut() {
            peer.entered_early = None;
        }
        if let Some(at) = early {
            log::info!(
                "Match {:#x}: player {index} is back in, from the snapshot at {time} ms \
                 (it entered at {at} ms)",
                self.id
            );
        } else if entering {
            let at = grid_up(self.clock(now).max(time + 1));
            self.broadcast(None, id::PLAYER_ENTERED, &roster_change(index, at));
            self.events.push((at, index, true));
            self.first_entered.get_or_insert(at);
            log::info!("Match {:#x}: player {index} entered at {at} ms", self.id);
        } else {
            // It started behind the match clock (`SYNC_LEAD_MS`): until its
            // clock is brought up its inputs come too late to relay. Its
            // first reports are in already; no reason to wait for more.
            self.correct_clock(index, now);
        }
    }

    fn input(&mut self, index: usize, payload: &[u8], now: Instant) {
        let Ok(input) = <[u8; INPUT_LEN]>::try_from(payload) else {
            return;
        };
        let clock = self.clock(now);
        let Some(peer) = self.peers[index].as_mut() else {
            return;
        };
        let time = u32::from_le_bytes(input[12..].try_into().unwrap());
        if peer.stage != Stage::Live {
            if matches!(
                peer.stage,
                Stage::WaitingSnapshot { .. } | Stage::Syncing { .. }
            ) && peer.unsynced_inputs.len() < UNSYNCED_KEPT
                && !peer.unsynced_inputs.contains(&time)
            {
                peer.unsynced_inputs.push(time);
                note_input(&mut self.trail, clock, time, index, b'U');
            }
            return;
        }
        if time > clock + TOO_NEW_MS {
            log::warn!(
                "Match {:#x}: player {index}'s input at {time} ms is ahead of {clock} ms; dropped",
                self.id
            );
            return;
        }
        if time + INPUT_MAX_AGE_MS < clock {
            return;
        }
        // The client resends until acked; each copy comes again.
        if !peer.inputs_seen.insert(time) {
            if peer
                .sync_sent_at
                .is_some_and(|at| now.duration_since(at) < CHANGED_LOG_FOR)
                && let Some((_, _, before)) = self
                    .inputs
                    .iter()
                    .rev()
                    .take_while(|(at, _, _)| *at + INPUT_MAX_AGE_MS >= time)
                    .find(|(at, from, _)| *at == time && *from == index)
                && *before != input
            {
                peer.inputs_changed += 1;
                note_input(&mut self.trail, clock, time, index, b'C');
                if self.changed_logs < MAX_CHANGED_LOGS {
                    self.changed_logs += 1;
                    log::info!(
                        "Match {:#x}: player {index} sent another input for {time} ms ({} since its resync, \
                         {} ms after it), which it had sent before; dropped as a copy, and it has both",
                        self.id,
                        peer.inputs_changed,
                        peer.synced_since(now).as_millis()
                    );
                }
            }
            return;
        }
        if peer
            .resynced
            .is_some_and(|resynced| resynced.made_before(time, clock))
        {
            // Not handled: one it makes for that tick on its new clock is new.
            peer.inputs_seen.remove(&time);
            peer.inputs_before_resync += 1;
            if let Some(serial) = send(peer, id::INPUT_DISCARD, &time.to_le_bytes()) {
                track(peer, serial, true, time);
            }
            note_input(&mut self.trail, clock, time, index, b'B');
            if self.before_resync_logs < MAX_BEFORE_RESYNC_LOGS {
                self.before_resync_logs += 1;
                log::info!(
                    "Match {:#x}: player {index}'s input for {time} ms, {} ms past its tick, was made \
                     before its resync ({} ms after it); dropped and discarded (0x58), not relayed",
                    self.id,
                    i64::from(clock) - i64::from(time),
                    peer.synced_since(now).as_millis()
                );
            }
            return;
        }
        if peer.inputs_seen.len() > 1024 {
            let oldest = clock.saturating_sub(INPUT_MAX_AGE_MS);
            peer.inputs_seen.retain(|seen| *seen >= oldest);
        }
        peer.inputs_received += 1;
        let late = clock.saturating_sub(time);
        peer.latest_input_ms = peer.latest_input_ms.max(late);
        // Its link, not its clock: a client just resynced runs a few hundred
        // ms behind the match clock by design, and every input of it is
        // that late for a few seconds.
        let behind = estimated_lag(peer, now).map_or(0, |lag| lag.max(0) as u32);
        let in_transit = late.saturating_sub(behind);
        peer.late_share +=
            (f32::from(u8::from(in_transit > LATE_INPUT_MS)) - peer.late_share) * LATE_SHARE_WEIGHT;
        if in_transit > LATE_INPUT_MS {
            peer.inputs_late += 1;
            log::debug!(
                "Match {:#x}: player {index}'s input at {time} ms first came {late} ms late; \
                 its first copy was probably lost",
                self.id
            );
        }
        if time + HORIZON_MS <= clock {
            // Final already: everyone else has played that tick without it.
            peer.inputs_discarded += 1;
            let run = peer.discard_run.get_or_insert(DiscardRun {
                since: now,
                count: 0,
                worst_ms: 0,
            });
            run.count += 1;
            run.worst_ms = run.worst_ms.max(late);
            if let Some(serial) = send(peer, id::INPUT_DISCARD, &time.to_le_bytes()) {
                track(peer, serial, true, time);
            }
            note_input(&mut self.trail, clock, time, index, b'D');
            log::debug!(
                "Match {:#x}: player {index}'s input at {time} ms came at {clock} ms; discarded",
                self.id
            );
            return;
        }
        let payload = relay(index, &input);
        for (other, peer) in self.peers.iter_mut().enumerate() {
            if other == index {
                continue;
            }
            if let Some(peer) = peer
                && peer.stage == Stage::Live
                && let Some(serial) = send(peer, id::INPUT_RELAY, &payload)
            {
                track(peer, serial, false, time);
            }
        }
        self.inputs.push((time, index, input));
        note_input(&mut self.trail, clock, time, index, b'R');
    }

    /// A snapshot chunk: `u32 item_time, u16 offset, u16 total, bytes`.
    fn chunk(&mut self, index: usize, payload: &[u8], now: Instant) {
        if payload.len() < CHUNK_HEADER {
            return;
        }
        let time = u32::from_le_bytes(payload[..4].try_into().unwrap());
        let offset = u16::from_le_bytes([payload[4], payload[5]]);
        let total = usize::from(u16::from_le_bytes([payload[6], payload[7]]));
        let data = &payload[CHUNK_HEADER..];
        let clock = self.clock(now);
        let initial = self.snapshot.is_none() && self.requested_at.is_none();
        let id = self.id;
        let Some(peer) = self.peers[index].as_mut() else {
            return;
        };
        // After the initial world, only an upload that was asked for, of a
        // tick near the match clock: one sent unasked, or for a time no
        // client is at, could put a forged world or results in.
        if !initial {
            let in_window = time.is_multiple_of(TICK_MS)
                && time <= clock + UPLOAD_MAX_AHEAD_MS
                && time + UPLOAD_MAX_AGE_MS >= clock;
            if !peer.asked || !in_window {
                // It can upload, though not now: ask it later. (A client
                // sends its initial world on every join, also one that is
                // not the first.)
                peer.patched = true;
                if peer.upload.is_none() {
                    log::warn!(
                        "Match {id:#x}: player {index} uploaded a snapshot at {time} ms \
                         unasked or out of time (clock {clock} ms); ignored"
                    );
                }
                peer.upload = Some(Upload {
                    time,
                    data: Vec::new(),
                    chunks: BTreeMap::new(),
                });
                return;
            }
        }
        peer.patched = true;
        let upload = peer.upload.get_or_insert_with(|| Upload {
            time,
            data: vec![0; total],
            chunks: BTreeMap::new(),
        });
        if upload.time != time || upload.data.len() != total {
            *upload = Upload {
                time,
                data: vec![0; total],
                chunks: BTreeMap::new(),
            };
        }
        let start = usize::from(offset);
        let Some(target) = upload.data.get_mut(start..start + data.len()) else {
            return;
        };
        if upload.chunks.contains_key(&offset) {
            return;
        }
        target.copy_from_slice(data);
        upload.chunks.insert(offset, data.len());
        if upload.received() < total {
            return;
        }
        let upload = peer.upload.take().unwrap();
        peer.asked = false;
        let diagnostic = std::mem::take(&mut peer.diagnostic);
        let after_resync = std::mem::take(&mut peer.after_resync);
        let snapshot = Snapshot {
            time: upload.time,
            body: upload.data,
        };
        if after_resync && !initial {
            // The world a client has after a resync: saved to compare with
            // what a healthy client uploaded for the same tick, nothing else.
            log::info!(
                "Match {id:#x}: player {index}'s world at {} ms, after its resync, saved to \
                 compare (CRC {:08x})",
                snapshot.time,
                crc32fast::hash(&snapshot.body)
            );
            self.dump(&snapshot, index, "after_resync");
            return;
        }
        if diagnostic && !initial {
            // The world of a client that is out of step: saved to compare,
            // and nothing else (it is no report and no candidate).
            log::info!(
                "Match {id:#x}: player {index}'s world at {} ms saved to compare (CRC {:08x})",
                snapshot.time,
                crc32fast::hash(&snapshot.body)
            );
            self.dump(&snapshot, index, "diagnostic");
            return;
        }
        if initial {
            self.first_snapshot(index, snapshot, now);
        } else {
            // What the client uploads is what its hash covers: a report.
            let crc = crc32fast::hash(&snapshot.body);
            if snapshot.time >= self.hashes_checked_until {
                self.hashes
                    .entry(snapshot.time)
                    .or_default()
                    .insert(index, crc);
            }
            let contradicted = self
                .majority(snapshot.time)
                .is_some_and(|majority| majority != crc);
            let newer = self
                .recent
                .as_ref()
                .is_none_or(|recent| recent.time < snapshot.time);
            if !contradicted && newer {
                self.recent = Some(snapshot.clone());
            }
            self.candidates.push(Candidate {
                from: index,
                snapshot,
                since: now,
            });
        }
    }

    /// The first upload starts the match. Every client built the same
    /// initial world, so it needs no check (there is nothing to check it
    /// against yet).
    fn first_snapshot(&mut self, index: usize, mut snapshot: Snapshot, now: Instant) {
        if let Some(why) = self.not_initial(&snapshot.body) {
            log::warn!(
                "Match {:#x}: player {index}'s initial world has {why}; not used",
                self.id
            );
            self.dump(&snapshot, index, "forged_initial");
            if let Some(peer) = self.peers[index].as_mut() {
                peer.patched = false;
            }
            return;
        }
        if snapshot.time < T0 || !snapshot.time.is_multiple_of(TICK_MS) {
            log::info!(
                "Match {:#x}: the initial world is at {} ms; it starts at {T0} ms",
                self.id,
                snapshot.time
            );
            snapshot.time = T0;
        }
        log::info!(
            "Match {:#x}: player {index} gave the initial world ({} bytes, CRC {:08x})",
            self.id,
            snapshot.body.len(),
            crc32fast::hash(&snapshot.body)
        );
        // The clock starts on the snapshot's time.
        self.clock_start = Some(now - Duration::from_millis(u64::from(snapshot.time - T0)));
        self.dump(&snapshot, index, "initial");
        self.observe(&snapshot, now);
        self.snapshot = Some(snapshot);
    }

    /// What a world that claims to be the initial one has that it can't:
    /// batteries home, kills, deaths or any statistic. `None` for a clean
    /// one, or one that can't be read (nothing in it can count either).
    fn not_initial(&self, body: &[u8]) -> Option<String> {
        let players = self.setup.roster.len();
        let decoded = snapshot::decode(body, players, usize::from(self.setup.batteries))?;
        if decoded.team_points != [0, 0] {
            return Some(format!("batteries home {:?}", decoded.team_points));
        }
        if decoded
            .kills_deaths
            .iter()
            .any(|kills_deaths| *kills_deaths != (0, 0))
        {
            return Some(format!("kills and deaths {:?}", decoded.kills_deaths));
        }
        let played =
            decoded.statistics.iter().flatten().any(|player| {
                player.events.iter().any(|count| *count != 0) || player.killstreak != 0
            });
        played.then(|| "statistics".to_owned())
    }

    /// How many clients reported `crc` for the tick at `time`.
    fn vouchers(&self, time: u32, crc: u32) -> usize {
        self.hashes.get(&time).map_or(0, |reports| {
            reports.values().filter(|&&each| each == crc).count()
        })
    }

    /// The CRC most clients reported for the tick at `time`, if more than
    /// half of those reporting agree. One report is enough only when one
    /// client is in the simulation: with two, both must agree.
    fn majority(&self, time: u32) -> Option<u32> {
        let reports = self.hashes.get(&time)?;
        let live = self
            .peers
            .iter()
            .flatten()
            .filter(|peer| peer.stage == Stage::Live)
            .count();
        if reports.len() < live.min(2) {
            return None;
        }
        let mut counts: HashMap<u32, usize> = HashMap::new();
        for crc in reports.values() {
            *counts.entry(*crc).or_default() += 1;
        }
        counts
            .into_iter()
            .find(|(_, count)| count * 2 > reports.len())
            .map(|(crc, _)| crc)
    }

    /// Check waiting uploads; a matching one becomes the snapshot.
    fn check_candidates(&mut self, now: Instant) {
        let candidates = std::mem::take(&mut self.candidates);
        for candidate in candidates {
            let crc = crc32fast::hash(&candidate.snapshot.body);
            match self.majority(candidate.snapshot.time) {
                Some(majority) if majority == crc => {
                    let newer = self
                        .snapshot
                        .as_ref()
                        .is_none_or(|snapshot| snapshot.time < candidate.snapshot.time);
                    if newer {
                        log::info!(
                            "Match {:#x}: player {}'s snapshot at {} ms matches the majority",
                            self.id,
                            candidate.from,
                            candidate.snapshot.time
                        );
                        self.dump(&candidate.snapshot, candidate.from, "checked");
                        // The results only when another client vouches for
                        // it too: alone, the uploader vouches for itself.
                        if self.vouchers(candidate.snapshot.time, crc) >= 2 {
                            self.observe(&candidate.snapshot, now);
                        } else {
                            log::info!(
                                "Match {:#x}: only player {} vouches for the snapshot at {} ms; \
                                 its results are not used",
                                self.id,
                                candidate.from,
                                candidate.snapshot.time
                            );
                        }
                        self.snapshot = Some(candidate.snapshot);
                        self.requested_at = None;
                    }
                }
                Some(majority) => {
                    log::warn!(
                        "Match {:#x}: player {}'s snapshot at {} ms has CRC {crc:08x}, the \
                         majority {majority:08x}; not used",
                        self.id,
                        candidate.from,
                        candidate.snapshot.time
                    );
                    self.dump(&candidate.snapshot, candidate.from, "rejected");
                    // Nobody may start from it.
                    if self.recent.as_ref() == Some(&candidate.snapshot) {
                        self.recent = None;
                    }
                }
                None if now.duration_since(candidate.since) < CANDIDATE_WAIT => {
                    self.candidates.push(candidate)
                }
                None => {
                    log::info!(
                        "Match {:#x}: no agreed hash for player {}'s snapshot at {} ms (CRC \
                         {crc:08x}, reports {:x?}); not used",
                        self.id,
                        candidate.from,
                        candidate.snapshot.time,
                        self.hashes.get(&candidate.snapshot.time)
                    );
                    // Its world may still serve a late joiner (a wrong one
                    // only desyncs that client, and it is resynced), but its
                    // results never count: nobody vouched for them.
                    self.dump(&candidate.snapshot, candidate.from, "unchecked");
                }
            }
        }
    }

    /// With no majority: the clients known to be out of step at `time` (a
    /// 0x54 or 0x58 for a tick up to it reached them past their horizon, so
    /// they lack an input the others play or keep one they don't), as wrong,
    /// and the others as right, if all the others agree with each other and
    /// none of the known ones does. In match 0x128 (2 players) a relay for
    /// tick 451900 reached one client 75 ms past its horizon, the clients
    /// disagreed from 451950, and the other client was resynced from that
    /// one's world, 5 times over.
    fn missed_verdict(
        &self,
        time: u32,
        reports: &HashMap<usize, u32>,
    ) -> Option<(Vec<usize>, Vec<usize>)> {
        let (mut wrong, mut right): (Vec<usize>, Vec<usize>) =
            reports.keys().copied().partition(|index| {
                self.peers[*index]
                    .as_ref()
                    .is_some_and(|peer| peer.missed.iter().any(|tick| *tick <= time))
            });
        let right_crcs: HashSet<u32> = right.iter().map(|index| reports[index]).collect();
        if wrong.is_empty()
            || right_crcs.len() != 1
            || wrong.iter().any(|index| right_crcs.contains(&reports[index]))
        {
            return None;
        }
        wrong.sort_unstable();
        right.sort_unstable();
        Some((wrong, right))
    }

    /// With no majority: the CRC of the side whose clients are the most
    /// reliable (inputs on time), if it clearly outweighs every other side.
    /// A client with a bad link is the one that falls out of step, so it is
    /// resynced rather than the clients whose play is fine.
    fn trusted_crc(&self, reports: &HashMap<usize, u32>) -> Option<u32> {
        let mut weights: HashMap<u32, f32> = HashMap::new();
        for (index, crc) in reports {
            let late = self.peers[*index]
                .as_ref()
                .map_or(0.0, |peer| peer.late_share.clamp(0.0, 1.0));
            *weights.entry(*crc).or_default() += 1.0 - late;
        }
        let mut sides: Vec<(f32, u32)> = weights.into_iter().map(|(crc, w)| (w, crc)).collect();
        sides.sort_by(|a, b| b.0.total_cmp(&a.0));
        match sides.as_slice() {
            [(best, crc), (next, _), ..] if best - next >= TRUST_MARGIN => Some(*crc),
            _ => None,
        }
    }

    /// Of `indices`, the client that started from a snapshot strictly
    /// after all the others.
    fn latest_synced(&self, indices: impl Iterator<Item = usize>) -> Option<usize> {
        let mut synced: Vec<(Option<Instant>, usize)> = indices
            .map(|index| {
                let at = self.peers[index].as_ref().and_then(|peer| peer.synced_at);
                (at, index)
            })
            .collect();
        synced.sort();
        match synced.as_slice() {
            [.., (before, _), (Some(last), index)] if before.is_none_or(|at| at < *last) => {
                Some(*index)
            }
            _ => None,
        }
    }

    /// Who was right and who wrong at the tick `time`, by its reports: the
    /// majority's CRC is right; with none, the clients known to be out of
    /// step are wrong (`missed_verdict`), else the side with the better links
    /// is right, failing which the client that started from a snapshot last is
    /// the likely odd one out (the others agreed before it came); with no
    /// such one, nobody can tell (nobody is named).
    fn judge(&mut self, time: u32, now: Instant) -> (Vec<usize>, Vec<usize>) {
        let reports = self.hashes[&time].clone();
        let split = |crc: u32| {
            let named = |wanted: bool| {
                reports
                    .iter()
                    .filter(|(_, each)| (**each == crc) == wanted)
                    .map(|(index, _)| *index)
                    .collect::<Vec<_>>()
            };
            (named(false), named(true))
        };
        if let Some(majority) = self.majority(time) {
            return split(majority);
        }
        let differ = reports
            .values()
            .any(|crc| Some(crc) != reports.values().next());
        if !differ {
            return (Vec::new(), Vec::new());
        }
        let missed = self.missed_verdict(time, &reports);
        if self
            .disagreement_logged
            .is_none_or(|at| now.duration_since(at) >= DISAGREEMENT_LOG)
        {
            self.disagreement_logged = Some(now);
            log::warn!(
                "Match {:#x}: the clients disagree at {time} ms, no majority: {reports:x?}",
                self.id
            );
            if let Some((wrong, _)) = &missed {
                log::info!(
                    "Match {:#x}: players {wrong:?} got an input (0x54) or discard (0x58) for a tick \
                     up to {time} ms past their horizon: they are out of step",
                    self.id
                );
            }
        }
        if let Some(verdict) = missed {
            return verdict;
        }
        if let Some(trusted) = self.trusted_crc(&reports) {
            return split(trusted);
        }
        match self.latest_synced(reports.keys().copied()) {
            Some(suspect) => (vec![suspect], Vec::new()),
            None => (Vec::new(), Vec::new()),
        }
    }

    /// A client's report at `time` was wrong (`right` false) or right.
    fn note_report(&mut self, index: usize, time: u32, right: bool, now: Instant) {
        let id = self.id;
        let Some(peer) = self.peers[index].as_mut() else {
            return;
        };
        if peer.stage != Stage::Live || time <= peer.judge_from {
            return;
        }
        if !right {
            let divergence = peer.diverged.get_or_insert(Divergence {
                first: time,
                last: time,
                since: now,
            });
            divergence.last = divergence.last.max(time);
        } else if let Some(divergence) = peer.diverged.take() {
            log::info!(
                "Match {id:#x}: player {index} disagreed from {} to {} ms ({} ms) and agrees \
                 again at {time} ms; not resynced",
                divergence.first,
                divergence.last,
                divergence.last - divergence.first
            );
        }
    }

    /// Find clients whose reports have disagreed with the right ones for a
    /// while, and resync them. A single wrong report is not enough: the
    /// clients report every 100 ms, a world that was out of step for a moment
    /// reports right again, and a client that is resynced starts behind the
    /// match clock with its inputs discarded for seconds. One that is really
    /// out of step is wrong on every tick after it.
    fn check_hashes(&mut self, now: Instant) {
        let clock = self.clock(now);
        let settled = clock.saturating_sub(HASH_SETTLE_MS);
        let times: Vec<u32> = self
            .hashes
            .range(self.hashes_checked_until..settled)
            .map(|(time, _)| *time)
            .collect();
        for time in times {
            let (wrong, right) = self.judge(time, now);
            for index in wrong {
                self.note_report(index, time, false, now);
            }
            for index in right {
                self.note_report(index, time, true, now);
            }
        }
        self.hashes_checked_until = self.hashes_checked_until.max(settled);
        let keep_from = clock.saturating_sub(HASH_KEEP_MS);
        self.hashes = self.hashes.split_off(&keep_from);
        let desynced: Vec<usize> = self
            .peers
            .iter()
            .enumerate()
            .filter(|(_, peer)| {
                peer.as_ref().is_some_and(|peer| {
                    peer.stage == Stage::Live
                        && peer.diverged.as_ref().is_some_and(|divergence| {
                            divergence.last - divergence.first >= DIVERGENCE_CONFIRM_MS
                                || now.duration_since(divergence.since) >= DIVERGENCE_CONFIRM_MAX
                        })
                })
            })
            .map(|(index, _)| index)
            .collect();
        for index in desynced {
            let Some(peer) = self.peers[index].as_mut() else {
                continue;
            };
            if peer.stage != Stage::Live
                || peer
                    .last_resync
                    .is_some_and(|at| now.duration_since(at) < RESYNC_INTERVAL)
            {
                continue;
            }
            // Its clock is still being brought up to the match clock (after
            // an earlier resync, mostly): until then its inputs come too late
            // to relay, so the others play without them and it would be out
            // of step again, and a resync now starts it behind once more.
            // The disagreement stands, and is found again once it caught up.
            if peer.correcting_until.is_some_and(|until| now < until) {
                let since = *peer.resync_deferred_since.get_or_insert(now);
                if now.duration_since(since) < RESYNC_DEFER_MAX {
                    if since == now {
                        log::info!(
                            "Match {:#x}: player {index} is out of sync, but its clock is still \
                             being corrected; resync put off",
                            self.id
                        );
                    }
                    continue;
                }
            }
            peer.resync_deferred_since = None;
            let first_wrong = peer.diverged.as_ref().map(|divergence| divergence.first);
            let wrong = peer.diverged.take().map_or(String::new(), |divergence| {
                format!(
                    ", wrong from {} to {} ms",
                    divergence.first, divergence.last
                )
            });
            peer.last_resync = Some(now);
            peer.resyncs += 1;
            let diagnose = peer.patched && !peer.asked && self.diagnostics < MAX_DIAGNOSTICS;
            if diagnose {
                peer.asked = true;
                peer.diagnostic = true;
                send(peer, id::UPLOAD_REQUEST, &[]);
                self.diagnostics += 1;
            }
            log::warn!(
                "Match {:#x}: player {index} is out of sync{wrong}; resyncing it",
                self.id
            );
            if let Some(first) = first_wrong {
                self.log_trail(index, first);
            }
            // A fresh snapshot from the others: the inputs since it bring
            // the client to now.
            self.start_sync(index, false, now);
        }
    }

    /// The inputs that came around the tick a client first went wrong: its
    /// own one by one (tick:age on arrival and what became of it), the
    /// others' counted. For finding what was different at that moment.
    fn log_trail(&mut self, index: usize, around: u32) {
        if self.trails_logged >= MAX_TRAILS {
            return;
        }
        self.trails_logged += 1;
        let (from, to) = (around.saturating_sub(1_500), around + 500);
        let mut own = Vec::new();
        // Per other player: relayed, dropped, the oldest on arrival (ms).
        let mut others: BTreeMap<u8, (u32, u32, i64)> = BTreeMap::new();
        for entry in &self.trail {
            if entry.tick < from || entry.tick > to {
                continue;
            }
            let age = i64::from(entry.at) - i64::from(entry.tick);
            if usize::from(entry.player) == index {
                own.push(format!("{}:{age:+}{}", entry.tick, entry.kind as char));
            } else {
                let each = others.entry(entry.player).or_default();
                if entry.kind == b'R' {
                    each.0 += 1;
                } else {
                    each.1 += 1;
                }
                each.2 = each.2.max(age);
            }
        }
        let others: Vec<String> = others
            .iter()
            .map(|(player, (relayed, dropped, oldest))| {
                format!(
                    "player {player}: {relayed} relayed, {dropped} dropped, oldest {oldest:+} ms"
                )
            })
            .collect();
        log::info!(
            "Match {:#x}: inputs for ticks {from} to {to}, around the divergence of player {index} (tick:age on arrival, R relayed, D dropped as too late, U sent while it loaded, B made before its resync, C a changed copy): its own [{}]; others: {}",
            self.id,
            own.join(" "),
            others.join("; ")
        );
    }

    /// A line for a clock report of a client just resynced: what it says
    /// against what the server set when it started it.
    fn log_clock_report(&mut self, index: usize, now: Instant) {
        if self.clock_logs >= MAX_CLOCK_LOGS {
            return;
        }
        let id = self.id;
        let Some(peer) = self.peers[index].as_ref() else {
            return;
        };
        if !peer.clock_log_until.is_some_and(|until| now < until) {
            return;
        }
        let Some(lag) = peer.lags.back() else {
            return;
        };
        let known = peer.sync_lag.map_or("?".to_owned(), |lag| lag.to_string());
        log::info!(
            "Match {id:#x}: player {index}'s clock report after its resync says it is {lag} ms behind (it was started {known} ms behind; round trip {} ms; {} ms since)",
            peer.round_trip,
            peer.synced_since(now).as_millis()
        );
        self.clock_logs += 1;
    }

    /// The player left: 0x47, a disconnect, or a new connection for its
    /// place. The others see it go.
    fn leave(&mut self, index: usize, now: Instant) {
        let Some(peer) = self.peers[index].as_mut() else {
            return;
        };
        // Entered early, it is in the others' world already.
        let was_live = matches!(peer.stage, Stage::Live)
            || peer.entered_early.is_some()
            || matches!(
                peer.stage,
                Stage::Syncing {
                    entering: false,
                    ..
                }
            )
            || matches!(
                peer.stage,
                Stage::WaitingSnapshot {
                    entering: false,
                    ..
                }
            );
        if peer.stage == Stage::Gone {
            return;
        }
        peer.stage = Stage::Gone;
        log::info!(
            "Match {:#x}: player {index} left; {}",
            self.id,
            input_summary(peer)
        );
        if !was_live {
            return;
        }
        let entered_at = self
            .events
            .iter()
            .rev()
            .find(|(_, who, _)| *who == index)
            .map_or(0, |(at, _, _)| *at);
        let at = grid_up(self.clock(now).max(entered_at + 1));
        self.broadcast(Some(index), id::PLAYER_LEFT, &roster_change(index, at));
        self.events.push((at, index, false));
    }

    /// Send to every client in the simulation but `except`.
    fn broadcast(&mut self, except: Option<usize>, message: u8, payload: &[u8]) {
        for (index, peer) in self.peers.iter_mut().enumerate() {
            if Some(index) == except {
                continue;
            }
            if let Some(peer) = peer
                && peer.stage == Stage::Live
            {
                send(peer, message, payload);
            }
        }
    }

    /// When the clients' time limit is up: the countdown starts with the
    /// first player in, then the match plays its duration.
    fn end_time(&self) -> Option<u32> {
        let first = self.first_entered?;
        Some(first + self.countdown_ms + self.duration_ms + END_GRACE_MS)
    }

    /// Bring a client's clock to the match clock: 0x55 with the whole
    /// remaining lag, once the last correction is done and fresh samples
    /// came in. The client corrects at 5% speed, so a correction of d ms
    /// takes 20·d ms.
    fn correct_clock(&mut self, index: usize, now: Instant) {
        let id = self.id;
        let Some(peer) = self.peers[index].as_mut() else {
            return;
        };
        if peer.stage != Stage::Live
            || peer.lags.len() < 2
            || peer.correcting_until.is_some_and(|until| now < until)
        {
            return;
        }
        let lag = *peer.lags.iter().min().unwrap();
        if lag.abs() < CLOCK_TOLERANCE_MS {
            return;
        }
        let offset = lag.clamp(i64::from(i16::MIN), i64::from(i16::MAX)) as i16;
        send(peer, id::CLOCK_OFFSET, &offset.to_le_bytes());
        let takes = Duration::from_millis(offset.unsigned_abs() as u64 * CORRECTION_MS_PER_MS);
        peer.correcting_until = Some(now + takes + CORRECTION_SETTLE);
        peer.lags.clear();
        peer.lag_ms = Some(lag);
        peer.correction = Some((now, i64::from(offset), takes));
        log::info!(
            "Match {id:#x}: player {index}'s clock is {lag} ms behind; correcting it over {} ms (round trip {} ms)",
            takes.as_millis(),
            peer.round_trip
        );
    }

    /// Save accepted snapshots to `dir`, one file per snapshot.
    pub fn dump_snapshots_to(&mut self, dir: PathBuf) {
        self.dump_dir = Some(dir);
    }

    fn dump(&mut self, snapshot: &Snapshot, from: usize, kind: &str) {
        let Some(dir) = &self.dump_dir else {
            return;
        };
        if !matches!(kind, "initial" | "checked") {
            if self.doubtful_dumps >= MAX_DOUBTFUL_DUMPS {
                return;
            }
            self.doubtful_dumps += 1;
        }
        // Match ids start over with every start of the server: the start
        // time tells apart matches that had the same id.
        let path = dir.join(format!(
            "match_{:x}_{}_p{from}_{kind}_r{}.bin",
            self.id,
            snapshot.time,
            run_stamp()
        ));
        if let Err(error) =
            std::fs::create_dir_all(dir).and_then(|()| std::fs::write(&path, &snapshot.body))
        {
            log::warn!("Can't save {}: {error}", path.display());
        }
    }

    /// Each player's round trip over the last [`PING_LOG_MS`], from the acks
    /// of the datagrams sent to it: the mean and the longest.
    fn log_pings(&mut self, clock: u32) {
        if clock.saturating_sub(self.pings_logged_at) < PING_LOG_MS {
            return;
        }
        self.pings_logged_at = clock;
        for (index, peer) in self.peers.iter_mut().enumerate() {
            let Some(peer) = peer.as_mut() else {
                continue;
            };
            if let Some((mean, max)) = peer.transport.take_round_trips() {
                log::info!(
                    "Match {:#x}: player {index}'s ping over the last {} s: {mean} ms on average, \
                     {max} ms at most",
                    self.id,
                    PING_LOG_MS / 1000
                );
            }
        }
    }

    /// The 0x54 and 0x58 the client at `index` has just acked: whether each
    /// reached it in time to be taken. It got one about half a round trip
    /// before its ack came, when its clock read the match clock less its lag;
    /// the input's tick had to be within `CLIENT_HORIZON_MS` of that. (The ack
    /// may wait for the client's next datagram, so a few may look late by up
    /// to its send interval that were not.)
    fn check_in_flight(&mut self, index: usize, now: Instant) {
        let clock = i64::from(self.clock(now));
        let Some(peer) = self.peers[index].as_mut() else {
            return;
        };
        if peer.in_flight.is_empty() {
            return;
        }
        let half_trip = i64::from(peer.transport.last_round_trip().unwrap_or(peer.round_trip) / 2);
        let lag = estimated_lag(peer, now).unwrap_or(0);
        let client_clock = clock - half_trip - lag;
        let in_flight = std::mem::take(&mut peer.in_flight);
        for each in in_flight {
            if !peer.transport.is_delivered(each.serial) {
                peer.in_flight.push_back(each);
                continue;
            }
            let past = client_clock - (i64::from(each.tick) + CLIENT_HORIZON_MS);
            if each.discard {
                peer.discards_acked += 1;
            } else {
                peer.relays_acked += 1;
            }
            if past < 0 {
                continue;
            }
            if each.discard {
                peer.discards_late += 1;
            } else {
                peer.relays_late += 1;
            }
            if past >= MISSED_PAST_MS {
                if peer.missed.len() == MISSED_KEPT {
                    peer.missed.pop_front();
                }
                peer.missed.push_back(each.tick);
            }
            let run = peer.late_run.get_or_insert(LateRun {
                since: now,
                discards: 0,
                relays: 0,
                worst_ms: 0,
                first_tick: each.tick,
                last_tick: each.tick,
            });
            if each.discard {
                run.discards += 1;
            } else {
                run.relays += 1;
            }
            run.worst_ms = run.worst_ms.max(past);
            run.first_tick = run.first_tick.min(each.tick);
            run.last_tick = run.last_tick.max(each.tick);
        }
    }

    /// The inputs of each client discarded as too late during the last
    /// [`DISCARD_LOG`], with how far its clock is behind and when it was last
    /// resynced: a client just resynced runs behind the match clock, and all
    /// its inputs are late until it has caught up.
    fn log_discards(&mut self, now: Instant) {
        for (index, peer) in self.peers.iter_mut().enumerate() {
            let Some(peer) = peer.as_mut() else {
                continue;
            };
            if peer
                .late_run
                .as_ref()
                .is_some_and(|run| now.duration_since(run.since) >= DISCARD_LOG)
            {
                let run = peer.late_run.take().unwrap();
                log::info!(
                    "Match {:#x}: player {index}: {} discards (0x58) and {} relays (0x54) reached it \
                     after its {CLIENT_HORIZON_MS} ms horizon in {} ms, for ticks {} to {}, the latest \
                     {} ms past it (its clock {} behind)",
                    self.id,
                    run.discards,
                    run.relays,
                    now.duration_since(run.since).as_millis(),
                    run.first_tick,
                    run.last_tick,
                    run.worst_ms,
                    estimated_lag(peer, now)
                        .map_or("unknown".to_owned(), |lag| format!("{lag} ms"))
                );
            }
            if peer
                .discard_run
                .as_ref()
                .is_none_or(|run| now.duration_since(run.since) < DISCARD_LOG)
            {
                continue;
            }
            let run = peer.discard_run.take().unwrap();
            let lag =
                estimated_lag(peer, now).map_or("unknown".to_owned(), |lag| format!("{lag} ms"));
            let resynced = peer.last_resync.map_or("never resynced".to_owned(), |at| {
                format!("resynced {} ms ago", now.duration_since(at).as_millis())
            });
            log::info!(
                "Match {:#x}: player {index}: {} inputs too late to relay in {} ms, the latest {} ms \
                 past the match clock (its clock {lag} behind, {resynced})",
                self.id,
                run.count,
                now.duration_since(run.since).as_millis(),
                run.worst_ms
            );
        }
    }

    /// Advance the match, and return the datagrams to send.
    pub fn poll(&mut self, now: Instant) -> Vec<(SocketAddr, Vec<u8>)> {
        let clock = self.clock(now);
        self.check_candidates(now);
        self.check_hashes(now);
        self.log_pings(clock);
        self.log_discards(now);

        for index in 0..self.peers.len() {
            let Some(peer) = self.peers[index].as_ref() else {
                continue;
            };
            let timeout = if matches!(peer.stage, Stage::Connecting | Stage::Loading) {
                LOADING_TIMEOUT
            } else {
                crate::transport::TIMEOUT
            };
            if peer.stage != Stage::Gone && peer.transport.is_gone_after(now, timeout) {
                log::info!("Match {:#x}: player {index} timed out", self.id);
                self.leave(index, now);
                continue;
            }
            let early = peer.entered_early;
            match peer.stage {
                Stage::WaitingSnapshot { since, entering } => {
                    let max_age = Self::snapshot_max_age(now.duration_since(since));
                    if let Some(snapshot) = self.usable_snapshot(index, clock, max_age) {
                        self.send_sync(index, snapshot, entering, now);
                    } else if now.duration_since(since) >= SNAPSHOT_GIVE_UP
                        && let Some(snapshot) = self.newest_snapshot()
                    {
                        log::warn!(
                            "Match {:#x}: no fresh snapshot for player {index} in {} s; it \
                             starts from the one at {} ms",
                            self.id,
                            SNAPSHOT_GIVE_UP.as_secs(),
                            snapshot.time
                        );
                        self.send_sync(index, snapshot, entering, now);
                    } else if let Some(at) = early {
                        if clock >= at + REJOIN_UPLOAD_AFTER_MS {
                            self.request_upload(index, now, RESYNC_REQUEST_GAP);
                        }
                    } else if !entering {
                        if self.resync_upload_due(index, clock) {
                            self.request_upload(index, now, RESYNC_REQUEST_GAP);
                        }
                    } else {
                        self.request_upload(index, now, RESYNC_REQUEST_GAP);
                    }
                }
                Stage::Syncing {
                    serial,
                    time,
                    entering,
                } if peer.transport.is_delivered(serial) => {
                    self.finish_sync(index, time, entering, now);
                }
                _ => {}
            }
        }

        self.check_after_resyncs(now);

        if self.finished_at.is_none()
            && self.ending.is_none()
            && let Some(end) = self.end_time()
            && clock >= end
        {
            log::info!("Match {:#x}: time is up at {clock} ms", self.id);
            self.ending = Some(now);
            // A last snapshot for the results, if a patched client can.
            self.requested_at = None;
            self.request_upload(usize::MAX, now, SNAPSHOT_WAIT);
        }
        if let Some(since) = self.ending {
            self.request_upload(usize::MAX, now, SNAPSHOT_WAIT);
            let fresh = self
                .state
                .as_ref()
                .is_some_and(|(time, _)| *time + HORIZON_MS * 4 >= self.clock(since));
            if fresh || self.sources(usize::MAX).is_empty() || now >= since + FINAL_WAIT {
                self.finish(now);
            }
        }

        if self.clock_start.is_some()
            && self.finished_at.is_none()
            && self.next_refresh.is_none_or(|at| now >= at)
        {
            self.next_refresh = Some(now + REFRESH);
            self.request_upload(usize::MAX, now, SNAPSHOT_WAIT);
        }
        let mut datagrams = Vec::new();
        for slot in &mut self.peers {
            let Some(peer) = slot else {
                continue;
            };
            let addr = peer.addr;
            datagrams.extend(
                peer.transport
                    .poll(now)
                    .into_iter()
                    .map(|datagram| (addr, datagram)),
            );
            if peer.stage == Stage::Gone && peer.transport.is_gone(now) {
                *slot = None;
            }
        }
        let empty = self
            .peers
            .iter()
            .flatten()
            .all(|peer| peer.stage == Stage::Gone);
        if !empty {
            self.empty_since = None;
        } else if self.empty_since.is_none() {
            self.empty_since = Some(now);
            if self.finished_at.is_none() && self.everyone_left(now) {
                log::info!(
                    "Match {:#x}: everyone left at {} ms; it is over",
                    self.id,
                    self.clock(now)
                );
            }
        }
        datagrams
    }

    /// Whether the players were in the match and all left it: with nobody
    /// in it, a seat that never came is not waited for once the match ran
    /// `SEATS_COME_WITHIN_MS`.
    fn everyone_left(&self, now: Instant) -> bool {
        self.empty_since.is_some()
            && self.first_entered.is_some_and(|first| {
                let all_came = (0..self.setup.roster.len()).all(|index| {
                    self.events
                        .iter()
                        .any(|(_, who, entered)| *who == index && *entered)
                });
                all_came || self.clock(now) >= first + SEATS_COME_WITHIN_MS
            })
    }

    /// The match is done: it ended and the clients had time to hear it,
    /// everyone in it left, or nobody has been in it for a while. Over, its
    /// seats are free and Play queues anew.
    pub fn is_over(&self, now: Instant) -> bool {
        self.finished_at
            .is_some_and(|at| now.duration_since(at) >= LINGER)
            || (self.finished_at.is_none() && self.everyone_left(now))
            || self
                .empty_since
                .is_some_and(|since| now.duration_since(since) >= ABANDONED)
    }
}

/// How often (match clock, ms) a player's round trip is logged.
const PING_LOG_MS: u32 = 30_000;

/// How far the client's clock is behind the match clock, from a clock it
/// sent: the match clock at arrival less the half round trip it travelled.
fn sample_lag(peer: &mut Peer, client: u32, clock: u32) {
    let lag = i64::from(clock) - i64::from(peer.round_trip / 2) - i64::from(client);
    if peer.lags.len() == CLOCK_SAMPLES {
        peer.lags.pop_front();
    }
    peer.lags.push_back(lag);
    peer.lag_ms = peer.lags.iter().min().copied();
    peer.correction = None;
}

/// How far the client's clock is behind the match clock now: as last
/// measured, less what the correction running since has made up (the client
/// moves at 5 % speed, evenly over its course).
fn estimated_lag(peer: &Peer, now: Instant) -> Option<i64> {
    let lag = peer.lag_ms?;
    let Some((since, offset, takes)) = peer.correction else {
        return Some(lag);
    };
    let done = if takes.is_zero() {
        1.0
    } else {
        (now.duration_since(since).as_secs_f64() / takes.as_secs_f64()).min(1.0)
    };
    Some(lag - (offset as f64 * done).round() as i64)
}

/// When this server process started, in unix seconds.
fn run_stamp() -> u64 {
    static STARTED: OnceLock<u64> = OnceLock::new();
    *STARTED.get_or_init(|| {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since| since.as_secs())
    })
}

/// Remember a 0x54 or 0x58 sent to the client, to see when it is acked.
fn track(peer: &mut Peer, serial: u64, discard: bool, tick: u32) {
    if peer.in_flight.len() == IN_FLIGHT_KEPT {
        peer.in_flight.pop_front();
    }
    peer.in_flight.push_back(InFlight {
        serial,
        discard,
        tick,
    });
}

fn input_summary(peer: &Peer) -> String {
    format!(
        "{} inputs, {} first seen over {LATE_INPUT_MS} ms late on the way, {} too late to relay, \
         {} made before a resync and come after it, the latest {} ms, resynced {} times, round \
         trip {} ms, {} messages to it sent again, {} of {} discards (0x58) and {} of {} relays \
         (0x54) reached it after its horizon",
        peer.inputs_received,
        peer.inputs_late,
        peer.inputs_discarded,
        peer.inputs_before_resync,
        peer.latest_input_ms,
        peer.resyncs,
        peer.round_trip,
        peer.transport.resent(),
        peer.discards_late,
        peer.discards_acked,
        peer.relays_late,
        peer.relays_acked
    )
}

fn send(peer: &mut Peer, message: u8, payload: &[u8]) -> Option<u64> {
    match peer.transport.send(message, payload) {
        Ok(serial) => Some(serial),
        Err(error) => {
            log::warn!("Can't send {message:#04x} to {}: {error:?}", peer.addr);
            None
        }
    }
}

/// 0x54: the player's index, then its input as it sent it.
fn relay(index: usize, input: &[u8; INPUT_LEN]) -> Vec<u8> {
    let mut relay = vec![index as u8];
    relay.extend(input);
    relay
}

/// 0x56 and 0x57: `u8 index, u32 time`.
fn roster_change(index: usize, time: u32) -> Vec<u8> {
    let mut change = vec![index as u8];
    change.extend(time.to_le_bytes());
    change
}

#[cfg(test)]
mod tests {
    use super::*;

    const PORT: u16 = 1236;

    /// A client's half of the transport, enough to talk to the game.
    struct Client {
        index: usize,
        addr: SocketAddr,
        seq: u16,
        /// The newest server seq received.
        newest: Option<u16>,
        /// Multipart messages being put together, by order id.
        parts: HashMap<u16, BTreeMap<u8, Vec<u8>>>,
        part_counts: HashMap<u16, (u8, u8)>,
        received: Vec<(u8, Vec<u8>)>,
    }

    fn ordered(id: u8) -> bool {
        matches!(id, 0x51 | 0x52 | 0x53 | 0x55 | 0x59)
    }

    impl Client {
        fn new(index: usize) -> Self {
            Self {
                index,
                addr: SocketAddr::from(([127, 0, 0, 1], 50_000 + index as u16)),
                seq: 0,
                newest: None,
                parts: HashMap::new(),
                part_counts: HashMap::new(),
                received: Vec::new(),
            }
        }

        /// A datagram with one message, acking everything received.
        fn datagram(&mut self, id: u8, payload: &[u8]) -> Vec<u8> {
            let mut datagram = self.seq.to_le_bytes().to_vec();
            self.seq += 1;
            datagram.extend(self.newest.unwrap_or(0xFFFF).to_le_bytes());
            let acks: u64 = if self.newest.is_some() { !1 } else { 0 };
            datagram.extend(acks.to_le_bytes());
            datagram.push(id);
            datagram.extend(payload);
            datagram
        }

        fn connect(&mut self, game: &mut Game, now: Instant) {
            let mut transport = Transport::new(now);
            let mut payload = (0x100 + self.index as u32).to_le_bytes().to_vec();
            payload.push(0);
            let datagram = self.datagram(id::CONNECT, &payload);
            transport.receive(&datagram, now);
            game.attach(self.index, self.addr, 0x100, transport, now);
        }

        fn send(&mut self, game: &mut Game, id: u8, payload: &[u8], now: Instant) {
            let datagram = self.datagram(id, payload);
            game.receive(self.index, &datagram, now);
        }

        fn take(&mut self, datagram: &[u8]) {
            let seq = u16::from_le_bytes([datagram[0], datagram[1]]);
            if self
                .newest
                .is_none_or(|newest| seq.wrapping_sub(newest) < 0x8000)
            {
                self.newest = Some(seq);
            }
            let body = &datagram[12..];
            if datagram[4] & 1 == 0 {
                self.message(body);
                return;
            }
            let mut rest = body;
            while let Some((&len, tail)) = rest.split_first() {
                let (len, tail) = if len == 0 {
                    (
                        usize::from(u16::from_le_bytes([tail[0], tail[1]])),
                        &tail[2..],
                    )
                } else {
                    (usize::from(len), tail)
                };
                let (message, tail) = tail.split_at(len);
                if !(rest.len() == body.len() && tail.is_empty()) {
                    self.message(message);
                }
                rest = tail;
            }
        }

        fn message(&mut self, message: &[u8]) {
            let id = message[0];
            if id != 0xFF {
                let skip = if ordered(id) { 3 } else { 1 };
                self.received.push((id, message[skip..].to_vec()));
                return;
            }
            let (kind, order, index) = (
                message[1],
                u16::from_le_bytes([message[2], message[3]]),
                message[4],
            );
            let data = if index == 0 {
                self.part_counts.insert(order, (kind, message[5]));
                &message[6..]
            } else {
                &message[5..]
            };
            let parts = self.parts.entry(order).or_default();
            parts.insert(index, data.to_vec());
            if let Some(&(kind, count)) = self.part_counts.get(&order)
                && parts.len() == usize::from(count)
            {
                let whole = parts.values().flatten().copied().collect();
                self.parts.remove(&order);
                self.received.push((kind, whole));
            }
        }

        /// What it got, but the periodic snapshot requests.
        fn drain(&mut self) -> Vec<(u8, Vec<u8>)> {
            let mut received = self.drain_all();
            received.retain(|(id, _)| *id != id::UPLOAD_REQUEST);
            received
        }

        fn drain_all(&mut self) -> Vec<(u8, Vec<u8>)> {
            std::mem::take(&mut self.received)
        }
    }

    /// Deliver what the game sends.
    fn deliver(game: &mut Game, clients: &mut [Client], now: Instant) {
        for (addr, datagram) in game.poll(now) {
            if let Some(client) = clients.iter_mut().find(|client| client.addr == addr) {
                client.take(&datagram);
            }
        }
    }

    fn game(players: usize, now: Instant) -> Game {
        let setup = Setup {
            roster: (0..players)
                .map(|index| RosterEntry {
                    name: format!("p{index}"),
                    team: (index % 2) as u8,
                    level: 1,
                    weapons: [0, 0],
                })
                .collect(),
            countdown_seconds: 0,
            duration_seconds: 60,
            batteries: 5,
            score_weights: [0.0; snapshot::EVENTS],
            loadouts: Vec::new(),
        };
        Game::new(0x123, PORT, vec![0xAA, 0xBB], setup, now)
    }

    fn ids(messages: &[(u8, Vec<u8>)]) -> Vec<u8> {
        messages.iter().map(|(id, _)| *id).collect()
    }

    /// Upload `body` as a patched client does, in 480-byte chunks.
    fn upload(client: &mut Client, game: &mut Game, time: u32, body: &[u8], now: Instant) {
        for (n, chunk) in body.chunks(480).enumerate() {
            let mut payload = time.to_le_bytes().to_vec();
            payload.extend(((n * 480) as u16).to_le_bytes());
            payload.extend((body.len() as u16).to_le_bytes());
            payload.extend(chunk);
            client.send(game, id::LEAVE, &payload, now);
        }
    }

    /// Take client `who` from 0x40 to being in the match, with `world` as
    /// the initial world if it uploads one. Returns what it was sent.
    fn join(
        game: &mut Game,
        clients: &mut [Client],
        who: usize,
        world: Option<&[u8]>,
        now: Instant,
    ) -> Vec<(u8, Vec<u8>)> {
        clients[who].connect(game, now);
        deliver(game, clients, now);
        clients[who].send(game, id::PORT_PROBE, &[0; 4], now);
        deliver(game, clients, now);
        if let Some(world) = world {
            upload(&mut clients[who], game, 0, world, now);
        }
        clients[who].send(game, id::READY, &[], now);
        deliver(game, clients, now);
        // Its next datagram acks the snapshot.
        clients[who].send(game, id::CLOCK, &T0.to_le_bytes(), now);
        deliver(game, clients, now);
        clients[who].drain()
    }

    fn clients(count: usize) -> Vec<Client> {
        (0..count).map(Client::new).collect()
    }

    #[test]
    fn the_first_client_starts_the_match_from_its_own_upload() {
        let now = Instant::now();
        let mut game = game(2, now);
        let mut clients = clients(1);
        clients[0].connect(&mut game, now);
        deliver(&mut game, &mut clients, now);
        assert_eq!(clients[0].drain(), [(id::PORTS, vec![0xD4, 4, 0xD4, 4])]);

        clients[0].send(&mut game, id::PORT_PROBE, &[0; 4], now);
        deliver(&mut game, &mut clients, now);
        assert_eq!(clients[0].drain(), [(id::STATIC_INFO, vec![0xAA, 0xBB, 0])]);

        let world: Vec<u8> = (0..1000u32).map(|i| i as u8).collect();
        upload(&mut clients[0], &mut game, 0, &world, now);
        clients[0].send(&mut game, id::READY, &[], now);
        deliver(&mut game, &mut clients, now);
        let messages = clients[0].drain();
        assert_eq!(ids(&messages), [id::SET_TIME, id::SNAPSHOT]);
        assert_eq!(messages[0].1, [T0.to_le_bytes(), T0.to_le_bytes()].concat());
        assert_eq!(messages[1].1, [&T0.to_le_bytes()[..], &world].concat());

        // Once the snapshot is acked, the player enters.
        let later = now + Duration::from_millis(120);
        clients[0].send(&mut game, id::CLOCK, &T0.to_le_bytes(), later);
        deliver(&mut game, &mut clients, later);
        assert_eq!(
            clients[0].drain(),
            [(id::PLAYER_ENTERED, roster_change(0, 200))]
        );
    }

    #[test]
    fn inputs_are_relayed_while_their_tick_is_open() {
        let now = Instant::now();
        let mut game = game(2, now);
        let mut clients = clients(2);
        join(&mut game, &mut clients, 0, Some(&[1, 2, 3]), now);
        let bob_start = join(&mut game, &mut clients, 1, None, now);
        // Bob starts from alice's world and sees her in.
        assert_eq!(bob_start[3].1, [&T0.to_le_bytes()[..], &[1, 2, 3]].concat());
        assert!(bob_start.contains(&(id::PLAYER_ENTERED, roster_change(0, 100))));
        assert_eq!(
            clients[0].drain(),
            [(id::PLAYER_ENTERED, roster_change(1, 100))]
        );

        let at = now + Duration::from_millis(1000);
        let mut input = [0u8; 16];
        input[8] = 4;
        input[12..].copy_from_slice(&1000u32.to_le_bytes());
        clients[1].send(&mut game, id::CLOCK, &0u32.to_le_bytes(), at);
        clients[0].send(&mut game, id::INPUT, &input, at);
        // A resent copy is not relayed again.
        clients[0].send(&mut game, id::INPUT, &input, at);
        deliver(&mut game, &mut clients, at);
        assert_eq!(clients[1].drain(), [(id::INPUT_RELAY, relay(0, &input))]);
        assert!(clients[0].drain().is_empty());

        // An input whose tick is final goes back as a discard.
        let mut late = input;
        late[12..].copy_from_slice(&750u32.to_le_bytes());
        clients[0].send(&mut game, id::INPUT, &late, at);
        deliver(&mut game, &mut clients, at);
        assert_eq!(
            clients[0].drain(),
            [(id::INPUT_DISCARD, 750u32.to_le_bytes().to_vec())]
        );
        assert!(clients[1].drain().is_empty());
    }

    #[test]
    fn a_late_joiner_gets_the_inputs_since_the_snapshot() {
        let now = Instant::now();
        let mut game = game(2, now);
        let mut clients = clients(2);
        join(&mut game, &mut clients, 0, Some(&[9]), now);
        let at = now + Duration::from_millis(500);
        let mut input = [0u8; 16];
        input[12..].copy_from_slice(&500u32.to_le_bytes());
        clients[0].send(&mut game, id::INPUT, &input, at);

        let start = join(&mut game, &mut clients, 1, None, at);
        assert_eq!(
            ids(&start),
            [
                id::PORTS,
                id::STATIC_INFO,
                id::SET_TIME,
                id::SNAPSHOT,
                id::INPUT_RELAY,
                id::PLAYER_ENTERED,
                id::PLAYER_ENTERED
            ]
        );
        assert_eq!(start[1].1, [0xAA, 0xBB, 1]);
        // Its clock starts at the snapshot (50) + 350, not at the match
        // clock (550): the input at 500 must not be past its horizon.
        assert_eq!(
            start[2].1,
            [550u32.to_le_bytes(), 400u32.to_le_bytes()].concat()
        );
        assert_eq!(start[4].1, relay(0, &input));
        assert_eq!(start[6].1, roster_change(1, 550));
    }

    #[test]
    fn a_newcomer_once_the_match_has_run_a_while_waits_for_a_fresh_snapshot() {
        let now = Instant::now();
        let mut game = game(2, now);
        let mut clients = clients(2);
        join(&mut game, &mut clients, 0, Some(&[1]), now);
        clients[0].drain_all();
        // It loaded last, 1 s in: the initial world would start it ~650 ms
        // behind the match clock (`SYNC_LEAD_MS` past 50 ms).
        let at = now + Duration::from_millis(1_000);
        let clock = game.clock(at);
        clients[0].send(&mut game, id::CLOCK, &clock.to_le_bytes(), at);
        let start = join(&mut game, &mut clients, 1, None, at);
        assert!(!ids(&start).contains(&id::SET_TIME));
        assert!(was_asked(&mut clients[0]));
        // The upload, its event-horizon item: from it, ~210 ms behind.
        let time = clock - 550;
        upload(&mut clients[0], &mut game, time, &[5, 5], at);
        deliver(&mut game, &mut clients, at);
        let messages = clients[1].drain();
        assert_eq!(ids(&messages)[..2], [id::SET_TIME, id::SNAPSHOT]);
        assert_eq!(
            messages[0].1,
            [clock.to_le_bytes(), (time + SYNC_LEAD_MS).to_le_bytes()].concat()
        );
    }

    #[test]
    fn a_snapshot_whose_tick_is_not_final_is_not_started_from() {
        let now = Instant::now();
        let (mut game, _) = three_joined(now);
        let at = now + Duration::from_secs(6);
        let clock = game.clock(at);
        // Inputs for ticks up to `HORIZON_MS` back may still be relayed.
        for (age, usable) in [(HORIZON_MS - 50, false), (HORIZON_MS, true)] {
            let snapshot = Snapshot {
                time: clock - age,
                body: vec![9],
            };
            game.recent = Some(snapshot.clone());
            assert_eq!(
                game.usable_snapshot(2, clock, RESYNC_FRESH_MS),
                usable.then_some(snapshot)
            );
        }
    }

    #[test]
    fn a_fresh_snapshot_is_used_once_the_majority_agrees() {
        let now = Instant::now();
        let mut game = game(3, now);
        let mut clients = clients(3);
        join(&mut game, &mut clients, 0, Some(&[1]), now);
        join(&mut game, &mut clients, 1, Some(&[1]), now);

        // Much later a third player comes: the snapshot is stale.
        let at = now + Duration::from_secs(30);
        for client in &mut clients[..2] {
            client.send(&mut game, id::CLOCK, &0u32.to_le_bytes(), at);
        }
        join(&mut game, &mut clients, 2, None, at);
        assert!(
            clients[0]
                .drain_all()
                .iter()
                .any(|(id, _)| *id == id::UPLOAD_REQUEST)
        );

        let world = vec![7u8; 1500];
        let crc = crc32fast::hash(&world);
        let tick = 29_500u32;
        for client in &mut clients[..2] {
            let mut hash = tick.to_le_bytes().to_vec();
            hash.extend(crc.to_le_bytes());
            client.send(&mut game, id::HASH, &hash, at);
        }
        upload(&mut clients[0], &mut game, tick, &world, at);
        deliver(&mut game, &mut clients, at);
        let messages = clients[2].drain();
        assert_eq!(ids(&messages), [id::SET_TIME, id::SNAPSHOT]);
        assert_eq!(messages[1].1, [&tick.to_le_bytes()[..], &world].concat());
    }

    #[test]
    fn a_client_out_of_sync_is_resynced_from_a_fresh_upload() {
        let now = Instant::now();
        let mut game = game(3, now);
        let mut clients = clients(3);
        for who in 0..3 {
            join(&mut game, &mut clients, who, Some(&[1]), now);
        }
        for client in &mut clients {
            client.drain();
        }
        let at = now + Duration::from_secs(6);
        sustained(
            &mut game,
            &mut clients,
            3000,
            &[(0, 1u32), (1, 1), (2, 2)],
            at,
        );
        deliver(&mut game, &mut clients, at);
        // The snapshot from the start is too old: the others are asked.
        assert!(clients[2].drain().is_empty());
        assert!(
            clients[0]
                .drain_all()
                .iter()
                .any(|(id, _)| *id == id::UPLOAD_REQUEST)
        );
        upload(&mut clients[0], &mut game, 5500, &[5, 5], at);
        deliver(&mut game, &mut clients, at);
        let messages = cleared_then(clients[2].drain(), 5500);
        assert_eq!(ids(&messages), [id::SET_TIME, id::SNAPSHOT]);
        assert_eq!(
            messages[0].1,
            [6050u32.to_le_bytes(), 5850u32.to_le_bytes()].concat()
        );
        assert_eq!(
            messages[1].1,
            [&5500u32.to_le_bytes()[..], &[5, 5]].concat()
        );
    }

    #[test]
    fn a_resync_forgets_the_clock_reports_from_before_it() {
        let now = Instant::now();
        let (mut game, _) = three_joined(now);
        let at = now + Duration::from_secs(6);
        for lag in [3, 4, 2, 5] {
            game.peers[1].as_mut().unwrap().lags.push_back(lag);
        }
        let snapshot = Snapshot {
            time: game.clock(at) - 600,
            body: vec![1, 2, 3],
        };
        game.send_sync(1, snapshot, false, at);
        let peer = game.peers[1].as_ref().unwrap();
        // Started well behind the match clock, and the server knows it.
        assert!(peer.lags.is_empty());
        assert!(peer.lag_ms.is_some_and(|lag| lag > 100));
        assert_eq!(estimated_lag(peer, at), peer.lag_ms);
        // Its first reports (0x43, 0x44) say so too: corrected at once.
        let clock = game.clock(at);
        let peer = game.peers[1].as_mut().unwrap();
        sample_lag(peer, clock - 210, clock);
        sample_lag(peer, clock - 212, clock);
        game.finish_sync(1, clock - 600, false, at);
        let peer = game.peers[1].as_ref().unwrap();
        assert!(peer.correcting_until.is_some());
        assert!(peer.correction.is_some_and(|(_, offset, _)| offset == 210));
    }

    #[test]
    fn inputs_a_client_sends_while_it_loads_a_snapshot_are_counted_and_dropped() {
        let now = Instant::now();
        let (mut game, mut clients) = three_joined(now);
        let at = now + Duration::from_secs(6);
        let clock = game.clock(at);
        game.peers[1].as_mut().unwrap().stage = Stage::Syncing {
            serial: u64::MAX,
            time: clock - 600,
            entering: false,
        };
        send_input(&mut game, &mut clients, 1, clock - 100, at);
        send_input(&mut game, &mut clients, 1, clock - 100, at);
        send_input(&mut game, &mut clients, 1, clock - 50, at);
        deliver(&mut game, &mut clients, at);
        let peer = game.peers[1].as_ref().unwrap();
        // Dropped, each tick once, and not relayed.
        assert_eq!(peer.unsynced_inputs, [clock - 100, clock - 50]);
        assert!(game.inputs.is_empty());
        clients[1].drain();
        game.finish_sync(1, clock - 120, false, at);
        assert!(game.peers[1].as_ref().unwrap().unsynced_inputs.is_empty());
        // Those after the snapshot are in its world only: it is told to
        // drop them, as it would be had they come too late.
        deliver(&mut game, &mut clients, at);
        let discards: Vec<_> = clients[1]
            .drain()
            .into_iter()
            .filter(|(id, _)| *id == id::INPUT_DISCARD)
            .collect();
        assert_eq!(
            discards,
            [
                (id::INPUT_DISCARD, (clock - 100).to_le_bytes().to_vec()),
                (id::INPUT_DISCARD, (clock - 50).to_le_bytes().to_vec())
            ]
        );
    }

    #[test]
    fn a_new_input_for_a_tick_sent_before_a_resync_is_logged() {
        let now = Instant::now();
        let (mut game, mut clients) = three_joined(now);
        let at = now + Duration::from_secs(6);
        let clock = game.clock(at);
        send_input(&mut game, &mut clients, 1, clock - 100, at);
        deliver(&mut game, &mut clients, at);
        let snapshot = Snapshot {
            time: clock - 600,
            body: vec![1, 2, 3],
        };
        game.send_sync(1, snapshot, false, at);
        game.finish_sync(1, clock - 600, false, at);
        // The same input again is a resent copy.
        send_input(&mut game, &mut clients, 1, clock - 100, at);
        deliver(&mut game, &mut clients, at);
        assert_eq!(game.peers[1].as_ref().unwrap().inputs_changed, 0);
        // Another for the same tick: played again after the resync.
        let mut input = vec![4u8; 12];
        input.extend((clock - 100).to_le_bytes());
        clients[1].send(&mut game, id::INPUT, &input, at);
        deliver(&mut game, &mut clients, at);
        assert_eq!(game.peers[1].as_ref().unwrap().inputs_changed, 1);
        assert_eq!(game.changed_logs, 1);
        assert_eq!(game.trail.back().map(|entry| entry.kind), Some(b'C'));
    }

    #[test]
    fn a_resync_starts_the_client_past_its_own_inputs_the_others_have() {
        let now = Instant::now();
        let (mut game, mut clients) = three_joined(now);
        // Its hashes up to 3450 are checked.
        deliver(&mut game, &mut clients, now + Duration::from_millis(5_900));
        let at = now + Duration::from_secs(7);
        let clock = game.clock(at);
        // Player 2's input for the last tick reached the others.
        send_input(&mut game, &mut clients, 2, clock - 50, at);
        assert_eq!(game.own_relayed_last(2, clock), Some(clock - 50));
        // A snapshot from which its clock would start at that tick or before
        // will not do: the next move of its mouse would merge into it.
        game.recent = Some(Snapshot {
            time: clock - 400,
            body: vec![9],
        });
        assert_eq!(game.usable_snapshot(2, clock, RESYNC_FRESH_MS), None);
        let later = Snapshot {
            time: clock - 350,
            body: vec![9],
        };
        game.recent = Some(later.clone());
        assert_eq!(
            game.usable_snapshot(2, clock, RESYNC_FRESH_MS),
            Some(later)
        );
        game.recent = None;

        for client in &mut clients {
            client.drain_all();
        }
        game.requested_at = None;
        disagree(&mut game, &mut clients, 4500, at);
        assert!(matches!(
            game.peers[2].as_ref().unwrap().stage,
            Stage::WaitingSnapshot {
                entering: false,
                ..
            }
        ));
        // Not asked yet: an upload now is from too early.
        assert_eq!(game.requested_at, None);
        deliver(&mut game, &mut clients, at + Duration::from_millis(250));
        assert_eq!(game.requested_at, None);
        // What it plays meanwhile is dropped.
        let due = at + Duration::from_millis(300);
        let due_clock = clock + 300;
        send_input(&mut game, &mut clients, 2, clock + 250, due);
        deliver(&mut game, &mut clients, due);
        assert_eq!(game.requested_at, Some(due));
        assert!(was_asked(&mut clients[0]));
        clients[2].drain_all();

        let snapshot_time = due_clock - 600;
        upload(&mut clients[0], &mut game, snapshot_time, &[5, 5], due);
        deliver(&mut game, &mut clients, due);
        // Every own input after the snapshot but the one the others have is
        // dropped: from its new clock on before the snapshot, while its
        // world is the old one; before that after the snapshot, which the
        // old world's history no longer reaches back to.
        let cleared: Vec<u32> = (snapshot_time / TICK_MS + 1
            ..=(due_clock + CLEAR_AHEAD_MS) / TICK_MS)
            .map(|tick| tick * TICK_MS)
            .filter(|tick| *tick != clock - 50)
            .collect();
        assert!(cleared.contains(&(clock + 250)));
        let start = snapshot_time + SYNC_LEAD_MS;
        assert!(start > clock - 50);
        let discard = |tick: &u32| (id::INPUT_DISCARD, tick.to_le_bytes().to_vec());
        let expected: Vec<(u8, Vec<u8>)> = cleared
            .iter()
            .filter(|tick| **tick >= start)
            .map(discard)
            .chain([
                (
                    id::SET_TIME,
                    [due_clock.to_le_bytes(), start.to_le_bytes()].concat(),
                ),
                (
                    id::SNAPSHOT,
                    [&snapshot_time.to_le_bytes()[..], &[5, 5]].concat(),
                ),
            ])
            .chain(cleared.iter().filter(|tick| **tick < start).map(discard))
            .collect();
        assert_eq!(clients[2].drain_all(), expected);
        // Again once it has the snapshot.
        let acked = due + Duration::from_millis(50);
        clients[2].send(&mut game, id::CLOCK, &start.to_le_bytes(), acked);
        deliver(&mut game, &mut clients, acked);
        let again: Vec<_> = clients[2]
            .drain()
            .into_iter()
            .filter(|(id, _)| *id == id::INPUT_DISCARD)
            .collect();
        let early: Vec<_> = cleared.iter().filter(|tick| **tick < start).map(discard).collect();
        assert_eq!(again, early);
    }

    #[test]
    fn a_tick_of_its_own_the_others_have_is_never_discarded_after_a_resync() {
        let now = Instant::now();
        let (mut game, mut clients) = three_joined(now);
        let at = now + Duration::from_secs(6);
        let clock = game.clock(at);
        send_input(&mut game, &mut clients, 2, clock - 100, at);
        deliver(&mut game, &mut clients, at);
        game.peers[2].as_mut().unwrap().stage = Stage::Syncing {
            serial: u64::MAX,
            time: clock - 600,
            entering: false,
        };
        // While it loads: that tick again (a new input merged into it), and
        // a new one.
        let mut merged = vec![4u8; 12];
        merged.extend((clock - 100).to_le_bytes());
        clients[2].send(&mut game, id::INPUT, &merged, at);
        send_input(&mut game, &mut clients, 2, clock - 50, at);
        deliver(&mut game, &mut clients, at);
        clients[2].drain_all();
        game.finish_sync(2, clock - 600, false, at);
        deliver(&mut game, &mut clients, at);
        let discards: Vec<_> = clients[2]
            .drain()
            .into_iter()
            .filter(|(id, _)| *id == id::INPUT_DISCARD)
            .collect();
        assert_eq!(
            discards,
            [(id::INPUT_DISCARD, (clock - 50).to_le_bytes().to_vec())]
        );
    }

    /// Player 2 of `three_joined` resynced from a snapshot 600 ms old at
    /// `at`, and in step again; its clock starts 250 ms behind.
    fn resynced_at(game: &mut Game, clients: &mut [Client], at: Instant) -> u32 {
        let snapshot = game.clock(at) - 600;
        game.send_sync(
            2,
            Snapshot {
                time: snapshot,
                body: vec![1, 2, 3],
            },
            false,
            at,
        );
        game.finish_sync(2, snapshot, false, at);
        deliver(game, clients, at);
        for client in clients.iter_mut() {
            client.drain_all();
        }
        snapshot
    }

    #[test]
    fn an_input_made_before_a_resync_that_comes_after_it_is_never_relayed() {
        let now = Instant::now();
        let (mut game, mut clients) = three_joined(now);
        let at = now + Duration::from_secs(6);
        let clock = game.clock(at);
        let snapshot = resynced_at(&mut game, &mut clients, at);
        assert_eq!(snapshot + SYNC_LEAD_MS, clock - 250);
        // Made on its old clock, come right after their ticks: one from its
        // new clock on, one before it.
        send_input(&mut game, &mut clients, 2, clock, at);
        send_input(&mut game, &mut clients, 2, snapshot + 100, at);
        deliver(&mut game, &mut clients, at);
        for who in [0, 1] {
            assert!(
                clients[who]
                    .drain()
                    .iter()
                    .all(|(id, _)| *id != id::INPUT_RELAY)
            );
        }
        let discards: Vec<_> = clients[2]
            .drain()
            .into_iter()
            .filter(|(id, _)| *id == id::INPUT_DISCARD)
            .collect();
        assert_eq!(
            discards,
            [
                (id::INPUT_DISCARD, clock.to_le_bytes().to_vec()),
                (id::INPUT_DISCARD, (snapshot + 100).to_le_bytes().to_vec())
            ]
        );
        let peer = game.peers[2].as_ref().unwrap();
        assert_eq!(peer.inputs_before_resync, 2);
        assert!(!peer.inputs_seen.contains(&clock));
        assert_eq!(game.trail.back().map(|entry| entry.kind), Some(b'B'));
        // Made on its new clock, 250 ms behind, it comes that late: new, and
        // relayed while its tick is open.
        let later = at + Duration::from_millis(200);
        send_input(&mut game, &mut clients, 2, clock, later);
        deliver(&mut game, &mut clients, later);
        assert_eq!(game.trail.back().map(|entry| entry.kind), Some(b'R'));
        assert!(
            clients[0]
                .drain()
                .iter()
                .any(|(id, _)| *id == id::INPUT_RELAY)
        );
    }

    #[test]
    fn a_tick_dropped_before_a_resync_takes_a_new_input_after_it() {
        let now = Instant::now();
        let (mut game, mut clients) = three_joined(now);
        let at = now + Duration::from_secs(6);
        let clock = game.clock(at);
        // Before the resync: one the others have, and one dropped as too
        // late, which it will play again from its new clock.
        send_input(
            &mut game,
            &mut clients,
            2,
            clock - 400,
            at - Duration::from_millis(300),
        );
        send_input(&mut game, &mut clients, 2, clock - 250, at);
        deliver(&mut game, &mut clients, at);
        assert!(game.peers[2].as_ref().unwrap().inputs_seen.contains(&(clock - 250)));
        resynced_at(&mut game, &mut clients, at);
        let seen = &game.peers[2].as_ref().unwrap().inputs_seen;
        assert!(seen.contains(&(clock - 400)));
        assert!(!seen.contains(&(clock - 250)));
        // Played again, still too late: discarded, not dropped as a copy
        // without a word (it would keep it, and nobody else has it).
        let later = at + Duration::from_millis(50);
        send_input(&mut game, &mut clients, 2, clock - 250, later);
        deliver(&mut game, &mut clients, later);
        assert!(
            clients[2]
                .drain()
                .contains(&(id::INPUT_DISCARD, (clock - 250).to_le_bytes().to_vec()))
        );
    }

    #[test]
    fn with_no_majority_a_client_that_missed_an_input_is_the_one_resynced() {
        let now = Instant::now();
        let mut game = game(2, now);
        let mut clients = clients(2);
        for who in 0..2 {
            join(&mut game, &mut clients, who, Some(&[1]), now);
        }
        for client in &mut clients {
            client.drain_all();
        }
        // Player 0's inputs come late more often, but a relay for tick 1900
        // reached player 1 past its horizon: its world lacks that input.
        game.peers[0].as_mut().unwrap().late_share = 0.4;
        game.peers[1].as_mut().unwrap().missed.push_back(1900);
        let mut reports = HashMap::new();
        reports.insert(0, 7);
        reports.insert(1, 8);
        assert_eq!(game.trusted_crc(&reports), Some(8));
        assert_eq!(game.missed_verdict(2000, &reports), Some((vec![1], vec![0])));
        // Not before that tick, nor if it agrees with the others after all.
        assert_eq!(game.missed_verdict(1850, &reports), None);
        reports.insert(1, 7);
        assert_eq!(game.missed_verdict(2000, &reports), None);

        let at = now + Duration::from_secs(6);
        sustained(&mut game, &mut clients, 3000, &[(0, 7u32), (1, 8)], at);
        deliver(&mut game, &mut clients, at);
        let resynced: Vec<bool> = game
            .peers
            .iter()
            .map(|peer| peer.as_ref().unwrap().last_resync.is_some())
            .collect();
        assert_eq!(resynced, [false, true]);
    }

    #[test]
    fn the_inputs_of_the_last_seconds_are_kept_with_what_became_of_them() {
        let now = Instant::now();
        let (mut game, mut clients) = three_joined(now);
        let at = now + Duration::from_secs(6);
        let clock = game.clock(at);
        // On time, so relayed; 300 ms old, so dropped as too late.
        send_input(&mut game, &mut clients, 0, clock - 50, at);
        send_input(&mut game, &mut clients, 1, clock - 300, at);
        deliver(&mut game, &mut clients, at);
        let kept: Vec<_> = game
            .trail
            .iter()
            .map(|e| (e.player, e.tick, e.kind as char))
            .collect();
        assert_eq!(kept, [(0, clock - 50, 'R'), (1, clock - 300, 'D')]);
        // Old ones go, and the number is bounded.
        let mut trail = VecDeque::new();
        note_input(&mut trail, 1_000, 900, 0, b'R');
        note_input(&mut trail, 1_000 + TRAIL_KEPT_MS + 1, 7_000, 1, b'R');
        assert_eq!(trail.len(), 1);
        for tick in 0..(2 * TRAIL_KEPT as u32) {
            note_input(&mut trail, 20_000, tick, 0, b'R');
        }
        assert_eq!(trail.len(), TRAIL_KEPT);
    }

    #[test]
    fn the_inputs_around_a_divergence_are_logged_a_few_times_a_match_only() {
        let now = Instant::now();
        let (mut game, mut clients) = three_joined(now);
        let at = now + Duration::from_secs(6);
        let clock = game.clock(at);
        send_input(&mut game, &mut clients, 1, clock - 100, at);
        deliver(&mut game, &mut clients, at);
        for _ in 0..(MAX_TRAILS + 3) {
            game.log_trail(1, clock - 100);
        }
        assert_eq!(game.trails_logged, MAX_TRAILS);
    }

    #[test]
    fn a_resynced_clients_clock_reports_are_logged_for_a_while_only() {
        let now = Instant::now();
        let (mut game, _) = three_joined(now);
        let peer = game.peers[1].as_mut().unwrap();
        peer.lags.push_back(210);
        game.log_clock_report(1, now);
        assert_eq!(game.clock_logs, 0);
        game.peers[1].as_mut().unwrap().clock_log_until = Some(now + CLOCK_LOG_FOR);
        game.log_clock_report(1, now + Duration::from_secs(1));
        assert_eq!(game.clock_logs, 1);
        game.log_clock_report(1, now + CLOCK_LOG_FOR + Duration::from_secs(1));
        assert_eq!(game.clock_logs, 1);
        game.clock_logs = MAX_CLOCK_LOGS;
        game.log_clock_report(1, now + Duration::from_secs(2));
        assert_eq!(game.clock_logs, MAX_CLOCK_LOGS);
    }

    #[test]
    fn a_client_loading_its_level_is_given_longer_than_one_playing() {
        let now = Instant::now();
        let mut game = game(2, now);
        let mut clients = clients(2);
        clients[0].connect(&mut game, now);
        deliver(&mut game, &mut clients, now);
        clients[0].send(&mut game, id::PORT_PROBE, &[0; 4], now);
        deliver(&mut game, &mut clients, now);
        assert_eq!(game.peers[0].as_ref().unwrap().stage, Stage::Loading);
        // Silent for 20 s: still loading.
        game.poll(now + Duration::from_secs(20));
        assert_eq!(game.peers[0].as_ref().unwrap().stage, Stage::Loading);
        // Silent for good: gone, after the longer time.
        game.poll(now + LOADING_TIMEOUT + Duration::from_secs(1));
        assert!(
            game.peers[0]
                .as_ref()
                .is_none_or(|peer| peer.stage == Stage::Gone)
        );
    }

    #[test]
    fn a_resynced_client_and_a_healthy_one_are_asked_for_their_worlds_to_compare() {
        let now = Instant::now();
        let mut game = game(3, now);
        let mut clients = clients(3);
        for who in 0..3 {
            join(&mut game, &mut clients, who, Some(&[1]), now);
            game.peers[who].as_mut().unwrap().patched = true;
        }
        for client in &mut clients {
            client.drain_all();
        }
        let at = now + Duration::from_secs(6);
        game.finish_sync(2, 5500, false, at);
        for client in &mut clients {
            client.drain_all();
        }
        assert!(game.peers[2].as_ref().unwrap().check_after.is_some());
        // Its clock still behind: not yet either.
        game.peers[2].as_mut().unwrap().lag_ms = Some(300);
        game.check_after_resyncs(at + Duration::from_millis(1_600));
        assert_eq!(game.after_resync_checks, 0);
        game.peers[2].as_mut().unwrap().lag_ms = Some(0);
        // Not yet.
        game.check_after_resyncs(at + Duration::from_millis(500));
        assert_eq!(game.after_resync_checks, 0);
        // Not while its clock is being corrected.
        game.peers[2].as_mut().unwrap().correcting_until = Some(at + Duration::from_millis(1_700));
        game.check_after_resyncs(at + Duration::from_millis(1_600));
        assert_eq!(game.after_resync_checks, 0);
        // Then: it and the best-linked other are asked, once.
        let later = at + Duration::from_millis(1_900);
        game.check_after_resyncs(later);
        assert_eq!(game.after_resync_checks, 1);
        deliver(&mut game, &mut clients, later);
        let asked = |client: &mut Client| {
            client
                .drain_all()
                .iter()
                .any(|(id, _)| *id == id::UPLOAD_REQUEST)
        };
        assert!(asked(&mut clients[2]));
        let source = game.sources(2)[0];
        assert!(asked(&mut clients[source]));
        assert!(game.peers[2].as_ref().unwrap().after_resync);
        assert!(!game.peers[source].as_ref().unwrap().after_resync);
        assert!(game.peers[2].as_ref().unwrap().check_after.is_none());
        // Its upload is only saved: no report, no candidate.
        upload(&mut clients[2], &mut game, 5550, &[7], later);
        deliver(&mut game, &mut clients, later);
        assert!(game.candidates.is_empty());
        assert!(!game.peers[2].as_ref().unwrap().after_resync);
        assert!(!game.hashes.get(&5550).is_some_and(|r| r.contains_key(&2)));
    }

    #[test]
    fn a_world_after_a_resync_is_asked_for_a_few_times_a_match_only() {
        let now = Instant::now();
        let mut game = game(3, now);
        let mut clients = clients(3);
        for who in 0..3 {
            join(&mut game, &mut clients, who, Some(&[1]), now);
            game.peers[who].as_mut().unwrap().patched = true;
        }
        let at = now + Duration::from_secs(6);
        game.after_resync_checks = MAX_AFTER_RESYNC_CHECKS;
        game.finish_sync(2, 5500, false, at);
        assert!(game.peers[2].as_ref().unwrap().check_after.is_none());
        // A client entering is not a resync.
        game.after_resync_checks = 0;
        game.finish_sync(1, 5500, true, at);
        assert!(game.peers[1].as_ref().unwrap().check_after.is_none());
    }

    #[test]
    fn only_the_clients_with_the_best_links_are_asked_for_a_snapshot() {
        let now = Instant::now();
        let mut game = game(4, now);
        let mut clients = clients(4);
        for who in 0..4 {
            join(&mut game, &mut clients, who, Some(&[1]), now);
        }
        for (who, late_share) in [(0, 0.5), (1, 0.0), (2, 0.3), (3, 0.0)] {
            game.peers[who].as_mut().unwrap().late_share = late_share;
        }
        game.peers[3].as_mut().unwrap().round_trip = 20;
        game.peers[1].as_mut().unwrap().round_trip = 90;
        // The same lateness, so the shorter round trip first.
        assert_eq!(game.sources(usize::MAX), [3, 1]);
        // The one that needs the snapshot is not asked.
        assert_eq!(game.sources(3), [1, 2]);
    }

    #[test]
    fn a_small_match_asks_one_client_for_a_snapshot_until_the_end() {
        let now = Instant::now();
        let mut game = game(3, now);
        let mut clients = clients(3);
        for who in 0..3 {
            join(&mut game, &mut clients, who, Some(&[1]), now);
        }
        game.peers[0].as_mut().unwrap().late_share = 0.4;
        assert_eq!(game.sources(usize::MAX), [1]);
        assert_eq!(game.sources(1), [2]);
        game.ending = Some(now);
        assert_eq!(game.sources(usize::MAX), [1, 2, 0]);
    }

    #[test]
    fn with_no_majority_the_clients_with_the_late_inputs_are_resynced() {
        let now = Instant::now();
        let mut game = game(4, now);
        let mut clients = clients(4);
        for who in 0..4 {
            join(&mut game, &mut clients, who, Some(&[1]), now);
        }
        for client in &mut clients {
            client.drain_all();
        }
        // Players 0 and 1 have a bad link; 2 and 3 agree, which is half of
        // the reports: no majority.
        for who in [0, 1] {
            game.peers[who].as_mut().unwrap().late_share = 0.4;
        }
        let at = now + Duration::from_secs(6);
        sustained(
            &mut game,
            &mut clients,
            3000,
            &[(0, 7u32), (1, 8), (2, 9), (3, 9)],
            at,
        );
        deliver(&mut game, &mut clients, at);
        let resynced: Vec<bool> = game
            .peers
            .iter()
            .map(|peer| peer.as_ref().unwrap().last_resync.is_some())
            .collect();
        assert_eq!(resynced, [true, true, false, false]);
    }

    #[test]
    fn with_no_majority_and_alike_links_nobody_is_blamed_by_them() {
        let now = Instant::now();
        let mut game = game(4, now);
        let mut clients = clients(4);
        for who in 0..4 {
            join(&mut game, &mut clients, who, Some(&[1]), now);
        }
        let mut reports = HashMap::new();
        for (who, crc) in [(0usize, 7u32), (1, 8), (2, 9), (3, 9)] {
            reports.insert(who, crc);
        }
        // All links alike: the two sides weigh 1 and 1, and 2 against 1 and 1
        // is a clear lead for the pair, but not 1 against 1.
        assert_eq!(game.trusted_crc(&reports), Some(9));
        reports.remove(&3);
        assert_eq!(game.trusted_crc(&reports), None);
    }

    /// Each of `crcs` (client, CRC) reports the same hash for every tick from
    /// a second before `last` to `last`, as the clients do every 100 ms: a
    /// disagreement that lasts (`DIVERGENCE_CONFIRM_MS`).
    fn sustained(
        game: &mut Game,
        clients: &mut [Client],
        last: u32,
        crcs: &[(usize, u32)],
        at: Instant,
    ) {
        for tick in (last - DIVERGENCE_CONFIRM_MS..=last).step_by(100) {
            for (who, crc) in crcs {
                report_hash(&mut clients[*who], game, tick, *crc, at);
            }
        }
        deliver(game, clients, at);
    }

    /// Three joined clients; player 2's hash differs from the other two for
    /// a second up to `tick`, as reported at `at`.
    fn disagree(game: &mut Game, clients: &mut [Client], tick: u32, at: Instant) {
        sustained(game, clients, tick, &[(0, 1), (1, 1), (2, 2)], at);
    }

    /// What a client resynced while playing gets but the 0x58s that clear
    /// its own inputs after the snapshot at `time` (`send_sync`): those from
    /// its new clock on before the 0x52, the others after the 0x53.
    fn cleared_then(messages: Vec<(u8, Vec<u8>)>, time: u32) -> Vec<(u8, Vec<u8>)> {
        let is_discard = |(id, _): &&(u8, Vec<u8>)| *id == id::INPUT_DISCARD;
        let before = messages.iter().take_while(is_discard).count();
        let after = messages[before..].iter().rev().take_while(is_discard).count();
        assert!(before > 0 && after > 0);
        let end = messages.len() - after;
        for (_, tick) in messages[..before].iter().chain(&messages[end..]) {
            assert!(u32::from_le_bytes(tick[..].try_into().unwrap()) > time);
        }
        messages[before..end].to_vec()
    }

    /// Whether the client was asked for an upload since it last looked.
    fn was_asked(client: &mut Client) -> bool {
        client
            .drain_all()
            .iter()
            .any(|(id, _)| *id == id::UPLOAD_REQUEST)
    }

    fn three_joined(now: Instant) -> (Game, Vec<Client>) {
        let mut game = game(3, now);
        let mut clients = clients(3);
        for who in 0..3 {
            join(&mut game, &mut clients, who, Some(&[1]), now);
        }
        for client in &mut clients {
            client.drain_all();
        }
        (game, clients)
    }

    #[test]
    fn a_resync_waits_for_a_fresh_snapshot() {
        assert_eq!(Game::snapshot_max_age(Duration::ZERO), RESYNC_FRESH_MS);
        // With none coming, an older one.
        assert_eq!(
            Game::snapshot_max_age(RESYNC_FRESH_WAIT),
            FRESH_SNAPSHOT_MS
        );

        let now = Instant::now();
        let (mut game, mut clients) = three_joined(now);
        let at = now + Duration::from_secs(6);
        let clock = game.clock(at);
        // Stored, 1050 ms behind the match clock: fine once none came for a
        // while, but a client started from it would run that far behind.
        let stale = Snapshot {
            time: clock - 1_050,
            body: vec![9],
        };
        game.recent = Some(stale.clone());
        assert_eq!(
            game.usable_snapshot(2, clock, FRESH_SNAPSHOT_MS),
            Some(stale)
        );
        assert_eq!(game.usable_snapshot(2, clock, RESYNC_FRESH_MS), None);

        disagree(&mut game, &mut clients, 3000, at);
        assert!(clients[2].drain().is_empty());
        assert!(matches!(
            game.peers[2].as_ref().unwrap().stage,
            Stage::WaitingSnapshot {
                entering: false,
                ..
            }
        ));
        assert!(
            clients[0]
                .drain_all()
                .iter()
                .any(|(id, _)| *id == id::UPLOAD_REQUEST)
        );
        // A fresh upload (550 ms behind) starts it, 200 ms behind the clock.
        upload(&mut clients[0], &mut game, clock - 550, &[5, 5], at);
        deliver(&mut game, &mut clients, at);
        let messages = cleared_then(clients[2].drain(), clock - 550);
        assert_eq!(ids(&messages), [id::SET_TIME, id::SNAPSHOT]);
        assert_eq!(
            messages[0].1,
            [
                clock.to_le_bytes(),
                (clock - 550 + SYNC_LEAD_MS).to_le_bytes()
            ]
            .concat()
        );
    }

    #[test]
    fn a_waiting_resync_asks_again_sooner_than_the_periodic_requests() {
        let now = Instant::now();
        let (mut game, mut clients) = three_joined(now);
        let at = now + Duration::from_secs(6);
        let clock = game.clock(at);
        game.recent = Some(Snapshot {
            time: clock - 1_050,
            body: vec![9],
        });
        disagree(&mut game, &mut clients, 3000, at);
        assert!(
            clients[0]
                .drain_all()
                .iter()
                .any(|(id, _)| *id == id::UPLOAD_REQUEST)
        );
        // Within the gap: not again (the transport resending the unacked
        // request is not a new one). After it: again.
        assert_eq!(game.requested_at, Some(at));
        deliver(&mut game, &mut clients, at + Duration::from_millis(500));
        assert_eq!(game.requested_at, Some(at));
        let later = at + RESYNC_REQUEST_GAP + Duration::from_millis(50);
        deliver(&mut game, &mut clients, later);
        assert_eq!(game.requested_at, Some(later));
        assert!(RESYNC_REQUEST_GAP < SNAPSHOT_WAIT);
    }

    #[test]
    fn a_client_whose_clock_is_still_corrected_is_not_resynced_yet() {
        let now = Instant::now();
        let (mut game, mut clients) = three_joined(now);
        let at = now + Duration::from_secs(6);
        game.peers[2].as_mut().unwrap().correcting_until = Some(at + Duration::from_secs(5));
        disagree(&mut game, &mut clients, 3000, at);
        let peer = game.peers[2].as_ref().unwrap();
        assert!(peer.last_resync.is_none());
        assert_eq!(peer.stage, Stage::Live);
        assert_eq!(peer.resync_deferred_since, Some(at));
        // Once its clock has caught up, the disagreement that still stands
        // is acted on.
        let later = at + Duration::from_secs(6);
        disagree(&mut game, &mut clients, 8000, later);
        let peer = game.peers[2].as_ref().unwrap();
        assert!(peer.last_resync.is_some());
        assert_eq!(peer.resync_deferred_since, None);
    }

    #[test]
    fn a_resync_is_put_off_for_a_while_only() {
        let now = Instant::now();
        let (mut game, mut clients) = three_joined(now);
        let at = now + Duration::from_secs(6);
        game.peers[2].as_mut().unwrap().correcting_until = Some(at + Duration::from_secs(120));
        disagree(&mut game, &mut clients, 3000, at);
        assert!(game.peers[2].as_ref().unwrap().last_resync.is_none());
        let later = at + RESYNC_DEFER_MAX + Duration::from_secs(1);
        disagree(&mut game, &mut clients, 20_000, later);
        assert!(game.peers[2].as_ref().unwrap().last_resync.is_some());
    }

    #[test]
    fn a_disagreement_that_goes_away_is_not_a_reason_to_resync() {
        let now = Instant::now();
        let (mut game, mut clients) = three_joined(now);
        let at = now + Duration::from_secs(6);
        // Player 2 is wrong from 2000 to 2300 ms, then right again.
        for tick in (2000..=3000).step_by(100) {
            let wrong = if tick <= 2300 { 2 } else { 1 };
            for (who, crc) in [(0, 1u32), (1, 1), (2, wrong)] {
                report_hash(&mut clients[who], &mut game, tick, crc, at);
            }
        }
        deliver(&mut game, &mut clients, at);
        let peer = game.peers[2].as_ref().unwrap();
        assert!(peer.diverged.is_none());
        assert!(peer.last_resync.is_none());
        assert_eq!(peer.resyncs, 0);
        assert_eq!(peer.stage, Stage::Live);
        // And far past the wall-clock limit nothing is left to act on.
        deliver(&mut game, &mut clients, at + DIVERGENCE_CONFIRM_MAX * 2);
        assert!(game.peers[2].as_ref().unwrap().last_resync.is_none());
    }

    #[test]
    fn a_disagreement_that_lasts_is_acted_on_once_it_has_lasted() {
        let now = Instant::now();
        let (mut game, mut clients) = three_joined(now);
        let at = now + Duration::from_secs(6);
        // Wrong for 900 ms: not yet.
        for tick in (2000..=2900).step_by(100) {
            for (who, crc) in [(0, 1u32), (1, 1), (2, 2)] {
                report_hash(&mut clients[who], &mut game, tick, crc, at);
            }
        }
        deliver(&mut game, &mut clients, at);
        let peer = game.peers[2].as_ref().unwrap();
        assert_eq!(
            peer.diverged.as_ref().map(|d| (d.first, d.last)),
            Some((2000, 2900))
        );
        assert!(peer.last_resync.is_none());
        // Still wrong at the next tick that is checked: over a second of it.
        let later = at + Duration::from_secs(1);
        for (who, crc) in [(0, 1u32), (1, 1), (2, 2)] {
            report_hash(&mut clients[who], &mut game, 3600, crc, later);
        }
        deliver(&mut game, &mut clients, later);
        let peer = game.peers[2].as_ref().unwrap();
        assert!(peer.last_resync.is_some());
        assert!(peer.diverged.is_none());
        assert_eq!(peer.resyncs, 1);
        // The others, who were right, are left alone.
        assert!(game.peers[0].as_ref().unwrap().last_resync.is_none());
        assert!(game.peers[1].as_ref().unwrap().last_resync.is_none());
    }

    #[test]
    fn a_disagreement_that_cannot_be_followed_is_acted_on_after_a_while() {
        let now = Instant::now();
        let (mut game, mut clients) = three_joined(now);
        let at = now + Duration::from_secs(6);
        // One wrong report and no more of them.
        for (who, crc) in [(0, 1u32), (1, 1), (2, 2)] {
            report_hash(&mut clients[who], &mut game, 2000, crc, at);
        }
        deliver(&mut game, &mut clients, at);
        assert!(game.peers[2].as_ref().unwrap().diverged.is_some());
        let before = at + DIVERGENCE_CONFIRM_MAX - Duration::from_millis(100);
        deliver(&mut game, &mut clients, before);
        assert!(game.peers[2].as_ref().unwrap().last_resync.is_none());
        let after = at + DIVERGENCE_CONFIRM_MAX + Duration::from_millis(100);
        deliver(&mut game, &mut clients, after);
        assert!(game.peers[2].as_ref().unwrap().last_resync.is_some());
    }

    #[test]
    fn reports_from_before_a_resync_say_nothing_about_the_world_after_it() {
        let now = Instant::now();
        let (mut game, _) = three_joined(now);
        let at = now + Duration::from_secs(6);
        game.peers[2].as_mut().unwrap().diverged = Some(Divergence {
            first: 1000,
            last: 1500,
            since: at,
        });
        let snapshot = Snapshot {
            time: 5500,
            body: vec![5],
        };
        let clock = game.clock(at);
        game.send_sync(2, snapshot, false, at);
        let peer = game.peers[2].as_mut().unwrap();
        assert!(peer.diverged.is_none());
        assert_eq!(peer.judge_from, clock);
        // Once it is simulating again, a wrong report of the old world is
        // not held against it, one of the new world is.
        peer.stage = Stage::Live;
        game.note_report(2, clock - 50, false, at);
        assert!(game.peers[2].as_ref().unwrap().diverged.is_none());
        game.note_report(2, clock, false, at);
        assert!(game.peers[2].as_ref().unwrap().diverged.is_none());
        game.note_report(2, clock + 50, false, at);
        assert!(game.peers[2].as_ref().unwrap().diverged.is_some());
    }

    /// Player `who` sends an input for `tick` at `at`.
    fn send_input(game: &mut Game, clients: &mut [Client], who: usize, tick: u32, at: Instant) {
        let mut input = vec![0u8; 12];
        input.extend(tick.to_le_bytes());
        clients[who].send(game, id::INPUT, &input, at);
    }

    #[test]
    fn a_discard_acked_soon_reached_the_client_in_time() {
        let now = Instant::now();
        let (mut game, mut clients) = three_joined(now);
        let at = now + Duration::from_secs(6);
        let clock = game.clock(at);
        // 300 ms old: final for the others, so player 1 is told to drop it.
        send_input(&mut game, &mut clients, 1, clock - 300, at);
        deliver(&mut game, &mut clients, at);
        assert_eq!(game.peers[1].as_ref().unwrap().in_flight.len(), 1);
        // Its ack 20 ms on: the 0x58 got there ~10 ms after it went, when
        // its clock was ~310 ms past the tick, inside its 500 ms.
        let soon = at + Duration::from_millis(20);
        clients[1].send(&mut game, id::CLOCK, &clock.to_le_bytes(), soon);
        let peer = game.peers[1].as_ref().unwrap();
        assert!(peer.in_flight.is_empty());
        assert_eq!((peer.discards_acked, peer.discards_late), (1, 0));
        assert!(peer.late_run.is_none());
    }

    #[test]
    fn a_discard_acked_after_the_clients_horizon_is_counted_and_logged() {
        let now = Instant::now();
        let (mut game, mut clients) = three_joined(now);
        let at = now + Duration::from_secs(6);
        let clock = game.clock(at);
        send_input(&mut game, &mut clients, 1, clock - 300, at);
        deliver(&mut game, &mut clients, at);
        // Acked 600 ms on: it got there ~300 ms after it went, when the tick
        // was ~600 ms behind the client's clock, past its 500 ms.
        let late = at + Duration::from_millis(600);
        clients[1].send(&mut game, id::CLOCK, &clock.to_le_bytes(), late);
        let peer = game.peers[1].as_ref().unwrap();
        assert_eq!((peer.discards_acked, peer.discards_late), (1, 1));
        let run = peer.late_run.as_ref().unwrap();
        assert_eq!((run.discards, run.relays), (1, 0));
        assert_eq!((run.first_tick, run.last_tick), (clock - 300, clock - 300));
        assert_eq!(run.worst_ms, 100);
        assert!(input_summary(peer).contains("1 of 1 discards (0x58)"));
        // It keeps an input the others lack: out of step from that tick.
        assert_eq!(peer.missed, [clock - 300]);
        // Logged once a second of them has gone by.
        deliver(&mut game, &mut clients, late + DISCARD_LOG);
        assert!(game.peers[1].as_ref().unwrap().late_run.is_none());
    }

    #[test]
    fn relays_are_followed_to_each_client_that_gets_them() {
        let now = Instant::now();
        let (mut game, mut clients) = three_joined(now);
        let at = now + Duration::from_secs(6);
        let clock = game.clock(at);
        send_input(&mut game, &mut clients, 0, clock - 50, at);
        deliver(&mut game, &mut clients, at);
        assert!(game.peers[0].as_ref().unwrap().in_flight.is_empty());
        let soon = at + Duration::from_millis(20);
        for who in [1, 2] {
            assert_eq!(game.peers[who].as_ref().unwrap().in_flight.len(), 1);
            clients[who].send(&mut game, id::CLOCK, &clock.to_le_bytes(), soon);
            let peer = game.peers[who].as_ref().unwrap();
            assert_eq!((peer.relays_acked, peer.relays_late), (1, 0));
            assert!(peer.in_flight.is_empty());
        }
    }

    #[test]
    fn a_clock_correction_makes_up_the_lag_evenly() {
        let now = Instant::now();
        let (mut game, _) = three_joined(now);
        let peer = game.peers[1].as_mut().unwrap();
        peer.lag_ms = Some(400);
        peer.correction = Some((now, 400, Duration::from_secs(8)));
        assert_eq!(estimated_lag(peer, now), Some(400));
        assert_eq!(estimated_lag(peer, now + Duration::from_secs(4)), Some(200));
        assert_eq!(estimated_lag(peer, now + Duration::from_secs(10)), Some(0));
        // A fresh measurement replaces the estimate.
        sample_lag(peer, 1000, 1100);
        assert!(peer.correction.is_none());
        assert_eq!(estimated_lag(peer, now), peer.lags.iter().min().copied());
    }

    #[test]
    fn dumps_are_named_by_the_run_too() {
        assert!(run_stamp() > 0);
        assert_eq!(run_stamp(), run_stamp());
    }

    #[test]
    fn inputs_discarded_as_too_late_are_counted_and_logged_in_runs() {
        let now = Instant::now();
        let (mut game, mut clients) = three_joined(now);
        let at = now + Duration::from_secs(6);
        let clock = game.clock(at);
        for time in [clock - 1_050, clock - 900] {
            let mut input = vec![0u8; 12];
            input.extend(time.to_le_bytes());
            clients[1].send(&mut game, id::INPUT, &input, at);
        }
        let run = game.peers[1]
            .as_ref()
            .unwrap()
            .discard_run
            .as_ref()
            .unwrap();
        assert_eq!((run.count, run.worst_ms), (2, 1_050));
        assert_eq!(game.peers[1].as_ref().unwrap().inputs_discarded, 2);
        // Logged once a second of them has gone by, then counted afresh.
        deliver(&mut game, &mut clients, at + Duration::from_millis(500));
        assert!(game.peers[1].as_ref().unwrap().discard_run.is_some());
        deliver(&mut game, &mut clients, at + DISCARD_LOG);
        assert!(game.peers[1].as_ref().unwrap().discard_run.is_none());
    }

    #[test]
    fn a_client_out_of_step_is_asked_for_its_world_to_compare_and_it_is_not_used() {
        let now = Instant::now();
        let mut game = game(2, now);
        let mut clients = clients(2);
        join(&mut game, &mut clients, 0, Some(&[1]), now);
        let later = now + Duration::from_millis(500);
        clients[0].send(&mut game, id::CLOCK, &0u32.to_le_bytes(), later);
        join(&mut game, &mut clients, 1, Some(&[1]), later);
        for client in &mut clients {
            client.drain_all();
        }
        let at = now + Duration::from_secs(6);
        sustained(&mut game, &mut clients, 3000, &[(0, 1u32), (1, 2)], at);
        deliver(&mut game, &mut clients, at);
        let resynced = game
            .peers
            .iter()
            .position(|peer| peer.as_ref().unwrap().last_resync.is_some())
            .unwrap();
        let other = 1 - resynced;
        assert_eq!(game.diagnostics, 1);
        assert!(
            clients[resynced]
                .drain_all()
                .iter()
                .any(|(id, _)| *id == id::UPLOAD_REQUEST)
        );
        upload(&mut clients[resynced], &mut game, 5550, &[7], at);
        deliver(&mut game, &mut clients, at);
        // Saved, and neither a report nor a candidate for anyone.
        assert!(game.candidates.is_empty());
        assert!(!game.peers[resynced].as_ref().unwrap().diagnostic);
        assert!(
            !game
                .hashes
                .get(&5550)
                .is_some_and(|r| r.contains_key(&resynced))
        );
        let _ = other;
    }

    #[test]
    fn of_two_disagreeing_clients_the_one_with_the_better_link_is_trusted() {
        let now = Instant::now();
        let mut game = game(2, now);
        let mut clients = clients(2);
        for who in 0..2 {
            join(&mut game, &mut clients, who, Some(&[1]), now);
        }
        let reports: HashMap<usize, u32> = [(0, 7u32), (1, 8)].into_iter().collect();
        // Alike links: nobody can tell.
        assert_eq!(game.trusted_crc(&reports), None);
        game.peers[0].as_mut().unwrap().late_share = 0.3;
        assert_eq!(game.trusted_crc(&reports), Some(8));
    }

    #[test]
    fn a_resync_does_not_make_the_client_the_next_one_blamed() {
        let now = Instant::now();
        let mut game = game(2, now);
        let mut clients = clients(2);
        join(&mut game, &mut clients, 0, Some(&[1]), now);
        let later = now + Duration::from_millis(500);
        clients[0].send(&mut game, id::CLOCK, &0u32.to_le_bytes(), later);
        join(&mut game, &mut clients, 1, None, later);
        let joined = game.peers[1].as_ref().unwrap().synced_at;
        assert!(joined.is_some());
        let first = game.peers[0].as_ref().unwrap().synced_at;
        // Player 0 is resynced later; player 1 stays the latest to start.
        let snapshot = Snapshot {
            time: 5550,
            body: vec![6],
        };
        game.send_sync(0, snapshot, false, later + Duration::from_secs(10));
        assert_eq!(game.peers[0].as_ref().unwrap().synced_at, first);
        assert_eq!(game.peers[1].as_ref().unwrap().synced_at, joined);
        assert_eq!(game.latest_synced([0, 1].into_iter()), Some(1));
    }

    #[test]
    fn of_two_disagreeing_clients_the_last_to_start_is_resynced() {
        let now = Instant::now();
        let mut game = game(2, now);
        let mut clients = clients(2);
        join(&mut game, &mut clients, 0, Some(&[1]), now);
        let later = now + Duration::from_millis(500);
        clients[0].send(&mut game, id::CLOCK, &0u32.to_le_bytes(), later);
        join(&mut game, &mut clients, 1, None, later);
        for client in &mut clients {
            client.drain_all();
        }
        let at = now + Duration::from_secs(6);
        sustained(&mut game, &mut clients, 3000, &[(0, 1u32), (1, 2)], at);
        deliver(&mut game, &mut clients, at);
        // Player 1 joined later: it is the one asked to start over, from
        // player 0's upload.
        assert!(
            clients[0]
                .drain_all()
                .iter()
                .any(|(id, _)| *id == id::UPLOAD_REQUEST)
        );
        upload(&mut clients[0], &mut game, 5550, &[6], at);
        deliver(&mut game, &mut clients, at);
        assert_eq!(
            ids(&cleared_then(clients[1].drain(), 5550)),
            [id::SET_TIME, id::SNAPSHOT]
        );
    }

    #[test]
    fn a_returning_player_enters_for_the_others_first_and_starts_after_that() {
        let now = Instant::now();
        let mut game = game(2, now);
        let mut clients = clients(2);
        join(&mut game, &mut clients, 0, Some(&[1]), now);
        join(&mut game, &mut clients, 1, None, now);
        // Player 1 leaves at 1050 ms and comes back at 2000.
        let left = now + Duration::from_secs(1);
        clients[0].send(&mut game, id::CLOCK, &0u32.to_le_bytes(), left);
        clients[1].send(&mut game, id::LEAVE, &0u32.to_le_bytes(), left);
        deliver(&mut game, &mut clients, left);
        clients[0].drain_all();
        let back = now + Duration::from_millis(1950);
        clients[0].send(&mut game, id::CLOCK, &0u32.to_le_bytes(), back);
        clients[1] = Client::new(1);
        clients[1].connect(&mut game, back);
        deliver(&mut game, &mut clients, back);
        clients[1].send(&mut game, id::PORT_PROBE, &[0; 4], back);
        clients[1].send(&mut game, id::READY, &[], back);
        deliver(&mut game, &mut clients, back);
        // The other client has it enter at once; nobody is asked for a
        // world yet, as an upload now would be from before that.
        let seen = clients[0].drain_all();
        assert!(seen.contains(&(id::PLAYER_ENTERED, roster_change(1, 2000))));
        assert!(!ids(&seen).contains(&id::UPLOAD_REQUEST));
        assert!(!ids(&clients[1].drain()).contains(&id::SNAPSHOT));
        // Once the uploads are past its entry, the other is asked.
        let asked = back + Duration::from_millis(700);
        clients[0].send(&mut game, id::CLOCK, &0u32.to_le_bytes(), asked);
        deliver(&mut game, &mut clients, asked);
        assert!(ids(&clients[0].drain_all()).contains(&id::UPLOAD_REQUEST));
        // An upload from its entry or before has no player for it: not used
        // (though fresh: 700 ms behind the match clock).
        upload(&mut clients[0], &mut game, 2000, &[2], asked);
        deliver(&mut game, &mut clients, asked);
        assert!(!ids(&clients[1].drain()).contains(&id::SNAPSHOT));
        // The next one, after its entry, is.
        let again = asked + RESYNC_REQUEST_GAP;
        clients[0].send(&mut game, id::CLOCK, &0u32.to_le_bytes(), again);
        deliver(&mut game, &mut clients, again);
        assert!(ids(&clients[0].drain_all()).contains(&id::UPLOAD_REQUEST));
        // As uploads are: ~550 ms behind the match clock.
        upload(&mut clients[0], &mut game, 3150, &[3], again);
        deliver(&mut game, &mut clients, again);
        // (After a resend of the unacked 0x51.)
        assert!(ids(&clients[1].drain()).ends_with(&[id::SET_TIME, id::SNAPSHOT]));
        // Once synced, it is told neither of its leave nor its entry, and
        // the other client does not see it enter twice.
        let synced = again + Duration::from_millis(100);
        clients[0].send(&mut game, id::CLOCK, &0u32.to_le_bytes(), synced);
        clients[1].send(&mut game, id::CLOCK, &0u32.to_le_bytes(), synced);
        deliver(&mut game, &mut clients, synced);
        let own = |messages: &[(u8, Vec<u8>)]| {
            messages.iter().any(|(id, payload)| {
                (*id == id::PLAYER_ENTERED || *id == id::PLAYER_LEFT) && payload[..1] == [1]
            })
        };
        assert!(!own(&clients[1].drain()));
        assert!(!own(&clients[0].drain()));
        let peer = game.peers[1].as_ref().unwrap();
        assert_eq!(peer.stage, Stage::Live);
        assert_eq!(peer.entered_early, None);
    }

    #[test]
    fn a_returning_player_that_goes_before_it_is_started_leaves_the_others_too() {
        let now = Instant::now();
        let mut game = game(2, now);
        let mut clients = clients(2);
        join(&mut game, &mut clients, 0, Some(&[1]), now);
        join(&mut game, &mut clients, 1, None, now);
        let left = now + Duration::from_secs(1);
        clients[0].send(&mut game, id::CLOCK, &0u32.to_le_bytes(), left);
        clients[1].send(&mut game, id::LEAVE, &0u32.to_le_bytes(), left);
        deliver(&mut game, &mut clients, left);
        let back = now + Duration::from_millis(1950);
        clients[0].send(&mut game, id::CLOCK, &0u32.to_le_bytes(), back);
        clients[1] = Client::new(1);
        clients[1].connect(&mut game, back);
        deliver(&mut game, &mut clients, back);
        clients[1].send(&mut game, id::PORT_PROBE, &[0; 4], back);
        clients[1].send(&mut game, id::READY, &[], back);
        deliver(&mut game, &mut clients, back);
        clients[0].drain_all();
        // It goes again while it waits for its snapshot.
        let gone = back + Duration::from_millis(300);
        clients[0].send(&mut game, id::CLOCK, &0u32.to_le_bytes(), gone);
        clients[1].send(&mut game, id::LEAVE, &0u32.to_le_bytes(), gone);
        deliver(&mut game, &mut clients, gone);
        assert!(
            clients[0]
                .drain()
                .iter()
                .any(|(id, payload)| *id == id::PLAYER_LEFT && payload[..1] == [1])
        );
    }

    #[test]
    fn a_client_clock_behind_is_moved_forward_once() {
        let now = Instant::now();
        let mut game = game(1, now);
        let mut clients = clients(1);
        join(&mut game, &mut clients, 0, Some(&[1]), now);
        // A full window of reports, each 100 ms behind the match clock.
        for second in 1..=4 {
            let at = now + Duration::from_secs(second);
            let behind = T0 + second as u32 * 1000 - 100;
            clients[0].send(&mut game, id::CLOCK, &behind.to_le_bytes(), at);
        }
        let at = now + Duration::from_secs(4);
        deliver(&mut game, &mut clients, at);
        assert_eq!(
            clients[0].drain(),
            [(id::CLOCK_OFFSET, 100i16.to_le_bytes().to_vec())]
        );
        // Not again while the client corrects (20 ms per ms).
        let later = at + Duration::from_secs(1);
        clients[0].send(&mut game, id::CLOCK, &(T0 + 4900).to_le_bytes(), later);
        clients[0].send(&mut game, id::CLOCK, &(T0 + 4900).to_le_bytes(), later);
        deliver(&mut game, &mut clients, later);
        assert!(clients[0].drain().is_empty());
    }

    #[test]
    fn a_leaver_is_seen_going_and_the_end_sends_everyone_home() {
        let now = Instant::now();
        let mut game = game(2, now);
        let mut clients = clients(2);
        join(&mut game, &mut clients, 0, Some(&[1]), now);
        join(&mut game, &mut clients, 1, None, now);
        clients[0].drain();

        let at = now + Duration::from_secs(10);
        clients[0].send(&mut game, id::CLOCK, &0u32.to_le_bytes(), at);
        clients[1].send(&mut game, id::LEAVE, &0u32.to_le_bytes(), at);
        deliver(&mut game, &mut clients, at);
        assert_eq!(
            clients[0].drain(),
            [(id::PLAYER_LEFT, roster_change(1, 10_050))]
        );

        // 60 s of match after alice entered at 100 ms, and the grace: a last
        // snapshot is asked for, and the end waits for it at most 3 s.
        let end = now + Duration::from_millis(62_100);
        clients[0].send(&mut game, id::CLOCK, &0u32.to_le_bytes(), end);
        deliver(&mut game, &mut clients, end);
        assert!(clients[0].drain().is_empty());
        let last = end + FINAL_WAIT;
        clients[0].send(&mut game, id::CLOCK, &0u32.to_le_bytes(), last);
        deliver(&mut game, &mut clients, last);
        assert_eq!(
            clients[0].drain(),
            [(id::PLAYER_LEFT, roster_change(0, 69_150))]
        );
        let report = game.take_report().unwrap();
        assert_eq!(report.winner, NO_WINNER);
        assert!(report.players[0].finished);
        assert!(!report.players[1].finished);
        assert!(!game.is_over(last));
        assert!(game.is_over(last + LINGER));
    }

    #[test]
    fn a_match_everyone_left_is_over_at_once() {
        let now = Instant::now();
        let mut game = game(2, now);
        let mut clients = clients(2);
        join(&mut game, &mut clients, 0, Some(&[1]), now);
        join(&mut game, &mut clients, 1, None, now);
        let at = now + Duration::from_secs(5);
        clients[0].send(&mut game, id::CLOCK, &0u32.to_le_bytes(), at);
        clients[1].send(&mut game, id::LEAVE, &0u32.to_le_bytes(), at);
        deliver(&mut game, &mut clients, at);
        // One still plays.
        assert!(!game.is_over(at));
        clients[0].send(&mut game, id::LEAVE, &0u32.to_le_bytes(), at);
        deliver(&mut game, &mut clients, at);
        assert!(game.is_over(at));
    }

    #[test]
    fn a_seat_that_may_still_come_keeps_a_left_match_for_a_while() {
        let now = Instant::now();
        let mut game = game(2, now);
        let mut clients = clients(2);
        join(&mut game, &mut clients, 0, Some(&[1]), now);
        // Player 1 is still loading the game; player 0 leaves.
        let at = now + Duration::from_secs(5);
        clients[0].send(&mut game, id::LEAVE, &0u32.to_le_bytes(), at);
        deliver(&mut game, &mut clients, at);
        assert!(!game.is_over(at));
        // A minute into the match it is not waited for.
        let late = now + Duration::from_millis(u64::from(SEATS_COME_WITHIN_MS) + 200);
        deliver(&mut game, &mut clients, late);
        assert!(game.is_over(late));
    }

    /// level_03, 2 players, 539.65 s in: player 1 (team 1) has brought one
    /// battery home, with 3 kills and 3 headshots.
    const LATE: &[u8] = include_bytes!("../tests/data/level_03_mode2_539650.bin");
    /// The same level's initial world.
    const START: &[u8] = include_bytes!("../tests/data/level_03_mode2_50.bin");
    const LATE_TIME: u32 = 539_650;

    /// Two players in a match started from the real initial world, 539.6 s
    /// later (the match long enough), both still there, player 0 asked for
    /// an upload.
    fn late_match(now: Instant) -> (Game, Vec<Client>, Instant) {
        let mut game = game(2, now);
        game.duration_ms = 1_000_000;
        game.setup.score_weights[event::KILL] = 100.0;
        game.setup.score_weights[event::WON] = 50.0;
        let mut clients = clients(2);
        join(&mut game, &mut clients, 0, Some(START), now);
        join(&mut game, &mut clients, 1, None, now);
        assert!(game.snapshot.is_some());
        let at = now + Duration::from_millis(u64::from(LATE_TIME - T0));
        for client in &mut clients {
            client.send(&mut game, id::CLOCK, &LATE_TIME.to_le_bytes(), at);
        }
        game.next_refresh = None;
        game.requested_at = None;
        deliver(&mut game, &mut clients, at);
        assert!(game.peers[0].as_ref().unwrap().asked);
        (game, clients, at)
    }

    fn report_hash(client: &mut Client, game: &mut Game, time: u32, crc: u32, now: Instant) {
        let mut hash = time.to_le_bytes().to_vec();
        hash.extend(crc.to_le_bytes());
        client.send(game, id::HASH, &hash, now);
    }

    #[test]
    fn the_results_come_from_a_snapshot_the_others_vouch_for() {
        let now = Instant::now();
        let (mut game, mut clients, at) = late_match(now);
        let crc = crc32fast::hash(LATE);
        report_hash(&mut clients[1], &mut game, LATE_TIME, crc, at);
        upload(&mut clients[0], &mut game, LATE_TIME, LATE, at);
        deliver(&mut game, &mut clients, at);
        // One battery of five home: the match goes on.
        assert!(game.take_report().is_none());
        assert_eq!(game.state.as_ref().unwrap().1.team_points, [0, 1]);

        game.finish(at);
        let report = game.take_report().unwrap();
        // Team 1 has more batteries home.
        assert_eq!(report.winner, 1);
        let bob = &report.players[1];
        assert_eq!(
            (bob.team, bob.kills, bob.deaths, bob.headshots),
            (1, 3, 2, 3)
        );
        // 3 kills and the win; the bonus counts the winners' batteries.
        assert_eq!(bob.score, 350);
        assert_eq!(bob.max_killstreak, 1);
        assert_eq!(report.players[0].deaths, 3);
        assert_eq!(report.players[0].score, 0);
    }

    #[test]
    fn a_later_clients_own_initial_world_still_makes_it_a_source_of_snapshots() {
        let now = Instant::now();
        let mut game = game(2, now);
        let mut clients = clients(2);
        // Both are patched clients: each uploads its initial world as it joins.
        join(&mut game, &mut clients, 0, Some(START), now);
        join(&mut game, &mut clients, 1, Some(START), now);
        assert!(game.peers[1].as_ref().unwrap().patched);
        // So when the first one leaves, the other can be asked (in a small
        // match one is asked at a time, the one with the better link).
        assert_eq!(game.sources(usize::MAX), [0]);
        assert_eq!(game.sources(0), [1]);
        // At the end, both.
        game.ending = Some(now);
        assert_eq!(game.sources(usize::MAX), [0, 1]);
    }

    #[test]
    fn a_snapshot_only_its_uploader_vouches_for_decides_nothing() {
        let now = Instant::now();
        let (mut game, mut clients, at) = late_match(now);
        // The other client saw something else at that tick.
        report_hash(&mut clients[1], &mut game, LATE_TIME, 0x1234, at);
        upload(&mut clients[0], &mut game, LATE_TIME, LATE, at);
        deliver(&mut game, &mut clients, at);
        // ... or nothing at all: still not counted, however long it waits.
        let later = at + CANDIDATE_WAIT + Duration::from_millis(100);
        for client in &mut clients {
            client.send(&mut game, id::CLOCK, &0u32.to_le_bytes(), later);
        }
        deliver(&mut game, &mut clients, later);
        game.finish(later);
        let report = game.take_report().unwrap();
        assert_eq!(report.team_points, [0, 0]);
        assert_eq!(report.players[1].kills, 0);
    }

    #[test]
    fn an_upload_nobody_asked_for_or_for_a_time_nobody_is_at_is_ignored() {
        let now = Instant::now();
        let (mut game, mut clients, at) = late_match(now);
        let crc = crc32fast::hash(LATE);
        // Player 1 was not asked; both vouch, still nothing.
        for client in &mut clients {
            report_hash(client, &mut game, LATE_TIME, crc, at);
        }
        upload(&mut clients[1], &mut game, LATE_TIME, LATE, at);
        // Player 0 was, but for a time far ahead of the clock.
        let ahead = LATE_TIME + 60_000;
        upload(&mut clients[0], &mut game, ahead, LATE, at);
        deliver(&mut game, &mut clients, at);
        assert!(game.candidates.is_empty());
        assert!(game.recent.is_none());
        assert_eq!(game.state.as_ref().map(|(time, _)| *time), Some(T0));
        // A hash report from the future is not kept either.
        report_hash(&mut clients[1], &mut game, ahead, crc, at);
        assert!(!game.hashes.contains_key(&ahead));
    }

    #[test]
    fn an_initial_world_where_things_already_happened_is_refused() {
        let now = Instant::now();
        let mut game = game(2, now);
        let mut clients = clients(1);
        clients[0].connect(&mut game, now);
        deliver(&mut game, &mut clients, now);
        upload(&mut clients[0], &mut game, T0, LATE, now);
        assert!(game.snapshot.is_none() && game.state.is_none());
        // The real one starts the match.
        upload(&mut clients[0], &mut game, T0, START, now);
        assert!(game.snapshot.is_some());
        game.finish(now);
        assert_eq!(game.take_report().unwrap().team_points, [0, 0]);
    }

    /// Every snapshot saved from real matches (`logs/snapshots`) passes
    /// the plausibility check: `cargo test -p match-server -- --ignored`.
    #[test]
    #[ignore]
    fn real_snapshots_are_plausible() {
        let now = Instant::now();
        let mut checked = 0;
        for entry in std::fs::read_dir("../../logs/snapshots").unwrap().flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let Some(time) = name
                .split('_')
                .nth(2)
                .and_then(|time| time.parse::<u32>().ok())
            else {
                continue;
            };
            let body = std::fs::read(entry.path()).unwrap();
            let Some((players, batteries, decoded)) = (1..=20usize)
                .flat_map(|players| (0..=5usize).map(move |batteries| (players, batteries)))
                .find_map(|(players, batteries)| {
                    let decoded = snapshot::decode(&body, players, batteries)?;
                    decoded
                        .statistics
                        .is_some()
                        .then_some((players, batteries, decoded))
                })
            else {
                continue;
            };
            let mut game = game(players, now);
            game.setup.batteries = batteries.max(1) as u8;
            game.first_entered = Some(T0);
            if let Err(why) = game.plausible(&decoded, time) {
                panic!("{name}: {why}");
            }
            if name.ends_with("_initial.bin") {
                assert_eq!(game.not_initial(&body), None, "{name}");
            }
            checked += 1;
        }
        println!("{checked} real snapshots are plausible");
        assert!(checked > 0);
    }

    #[test]
    fn results_no_play_could_reach_are_not_used() {
        let now = Instant::now();
        let mut game = game(2, now);
        game.first_entered = Some(100);
        let decoded = snapshot::decode(LATE, 2, 5).unwrap();
        assert!(game.plausible(&decoded, LATE_TIME).is_ok());
        // One battery home 2 s in: not carried yet.
        assert!(game.plausible(&decoded, 2_100).is_err());
        let mut more_kills = decoded.clone();
        more_kills.statistics.as_mut().unwrap()[1].events[event::KILL] = 50;
        assert!(game.plausible(&more_kills, LATE_TIME).is_err());
        let mut all_home = decoded;
        all_home.team_points = [0, 6];
        assert!(game.plausible(&all_home, LATE_TIME).is_err());
    }
}
