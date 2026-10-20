//! One 0.26g match, the server's side of the clients' lockstep
//! (`docs/notes/match.md`). The 0.21d `crate::game` carried over
//! to the 0.26g messages; the reasons behind most rules and numbers are in
//! its comments and in `MATCH-SYNC-021D.md`.
//!
//! The clients simulate; the server does not. It:
//! - takes each client through the handshake: 0xD8 → 0xE3, 0xD9 → 0xE4,
//!   0xDA → 0xE5 and 0xE8, then the inputs and roster changes since the
//!   snapshot, then 0xE9 to put the player in;
//! - keeps the match clock, relays each input (0xDC/0xDD → 0xEB/0xEC, 0xDE →
//!   0xF3) while its tick is not final (250 ms), and tells a client whose
//!   input came too late to drop it (0xF1/0xF2, 0xF5) or, by setting, to
//!   move it (0xEF/0xF0, 0xF4);
//! - gets world snapshots from patched clients (0xE0 chunks). The first one
//!   starts the match and must be a world where nothing has happened yet;
//!   later ones are taken only when asked for with 0xF6, for a tick near
//!   the match clock, and used when their CRC is what the clients reported
//!   for that tick (0xDF);
//! - measures each client's round trip and clock with 0xE6 (0.26g has no
//!   clock report of its own) and corrects the clock with 0xE7;
//! - resyncs a client whose reports disagree with the others', each time
//!   with the next sync generation, and serves late joiners and players
//!   coming back from a fresh snapshot;
//! - ends the match: by the mode's rule or the time limit, read from
//!   snapshots the clients vouch for; or because its world is lost (no
//!   client simulates any more and a player has waited
//!   [`WORLD_LOST_AFTER`] to get in: `world_lost`). Each client gets 0xEA
//!   with its own index; the lobby gets a [`MatchResult`].
//!
//! An input that comes after its tick is final is dropped, or by setting
//! moved to an open tick (`place`), which 0.21d could not do.
//!
//! New against 0.21d, because of the 0.26g transport: nothing is ordered,
//! and most messages live 250 ms (relays, roster changes, discards) or are
//! sent once (0xE5, 0xE6, 0xE7). So a sync is 0xE5 first, again until the
//! client answers it (0xDB), and only then 0xE5 with 0xE8: a world never
//! arrives without its time. A snapshot that was not acked in its 1.75 s
//! goes again as the next generation, and a relay or roster change that
//! was never acked marks its client as out of step.
//!
//! A client resynced while it plays (match.md section 30) is sent the
//! others' inputs and the roster changes since the snapshot once it has
//! answered its 0xE5, and its world once all of that is acked; a world not
//! loaded in time for the client's input rings is sent again from a fresh
//! snapshot. Its own inputs, which it goes on making on its new clock, are
//! relayed all the while, as any client's; the answers to them (dropped,
//! moved) wait for the load, as they may be refused before it.
//!
//! A game that has no world yet (it has just loaded its level) is given
//! longer for its first one (`Timing::entry_state`), and the inputs of
//! the others reach it a few times a second, many in a datagram
//! ([`WAITING_RELAY_FLUSH`]). Its world stands still until it has one, so
//! nothing is urgent for it, and a new world every 1.75 s threw away the
//! parts of the last that a thin line had let through (live, 2026-10-13:
//! `docs/notes/match.md` section 27).
//!
//! # Bots (testing only)
//!
//! A place can be a test bot's (`MatchPlayer::bot`, from the operator's
//! `SRP_BOT_ACCOUNTS`; no place is one unless that is set): a scripted
//! client (`srp-bot`) that cannot simulate the game, so that one real
//! client gets a match. It is a player for the others and no witness of
//! the world:
//! - it is taken through the handshake and the sync as any stock client
//!   (0xE3, 0xE4, 0xE5, 0xE8), put in with 0xE9, gets the relays and the
//!   roster changes, is pinged, and is told the end;
//! - its hash reports (0xDF) and uploads (0xE0 chunks) are dropped: it is
//!   never a voucher, never counted among the clients that must agree
//!   (`majority`, `check_candidates`), never asked for a world
//!   (`sources`), and, having no reports, never resynced for its hashes;
//! - the "others simulate" tests (`others_live`: how fresh a start
//!   snapshot must be, where a clock starts) count the real clients only:
//!   a bot holds no world that a newcomer could be out of step with;
//! - it is started only once a real client simulates (`bot_waits`), so
//!   its 0xE9 reaches that client ahead of its tick, as a second real
//!   player's does, and is not one of the roster changes a starting
//!   client is handed for ticks it has already played.
//!
//! So the rules for the real clients are the ones of a match without the
//! bots' places. With two or more real clients nothing changes: a world
//! needs two of them. With **one** real client and bots, the rule is the
//! existing one for a client alone in a match: its upload is trusted on
//! its own word (one voucher, `MatchResult::trusted_by` 1), within what
//! `plausible` allows. That is what a test alone wants and nothing a
//! public server should allow: the setting is off by default.
//!
//! A match with bots in which no real client is connected for
//! [`BOTS_ALONE`] is over (`MatchEnd::Deserted`): bots never leave by
//! themselves, and would hold the match, and a real player's seat in it,
//! until the time limit. (The server's own bots leave only to make room
//! for a late joiner, `game/driven.rs`; that never ends a match.)
//!
//! Not carried over from 0.21d: the diagnostic uploads and the per-input
//! trail (log aids), the estimate of when a relay reached a client (a relay
//! now either is acked while the game can still use it or is lost:
//! [`RELAY_USEFUL_MS`]).

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use survarium::v026g::dictionary::Dictionary;
use survarium::v026g::maps::mode;
use survarium::v026g::match_msgs::client_model::EVENT_HORIZON_MS;
use survarium::v026g::match_msgs::players::{self, Supplies};
use survarium::v026g::match_msgs::rules::{self, TimelimitParams, TimelimitRule, Transition};
use survarium::v026g::match_msgs::snapshot::{self, Rules, Sections, Standing, Statistics, event};
use survarium::v026g::match_msgs::upload::{Pushed, Reassembler, Upload};
use survarium::v026g::match_msgs::{
    ClientMessage, FORCED_MISMATCH, Generation, PlayerInput, ServerMessage, TICK_MS, TimeReply,
    WireTime, clock as game_clock, parse_client, tick_byte, tick_time,
};
use udp_transport::{Event, SendError, Transport};

use super::record::{Recorder, Recording};
use super::registry::{Formed, MatchEnd, MatchResult, PlayerResult};
use super::{LateInputs, Settings};

mod driven;
mod late_join;
mod live;
use late_join::{Admission, LoadingProfile};

/// A tick is final this long after it: the server's event horizon.
const HORIZON_MS: u32 = 250;
/// Inputs further ahead than this are dropped (the client moves them back).
const TOO_NEW_MS: u32 = 2_500;
/// With others simulating, a newcomer starts only from a snapshot this
/// fresh once it has waited `RESYNC_FRESH_WAIT`.
const FRESH_SNAPSHOT_MS: u32 = 1_500;
/// A client resynced, or a newcomer while others play, starts only from a
/// snapshot this fresh: every ms of its age is a ms the client runs behind.
const RESYNC_FRESH_MS: u32 = 700;
const RESYNC_FRESH_WAIT: Duration = Duration::from_secs(3);
/// How soon a client that waits for an upload asks again. The patched
/// client must not be asked twice within about a second
/// (client_patch.md §3.4).
const RESYNC_REQUEST_GAP: Duration = Duration::from_secs(1);
/// A player back in a match enters for the others first; an upload is asked
/// for once the match clock is this far past its entry.
const REJOIN_UPLOAD_AFTER_MS: u32 = 600;
/// A resynced client's world goes only while its snapshot will be at most
/// this far behind the match clock when it reaches the game: its age now
/// and the way there (`resync_world_max_age`). The client keeps the last
/// 27 inputs, times and events of each player (rings, the oldest dropped),
/// its own and the others', and its catch-up plays from the snapshot: a
/// player who made more than about 26 inputs between the snapshot and the
/// load is played wrong (PROVEN, match.md section 30). The 200 ms to
/// [`RESYNC_LOAD_BY_MS`] are for the world's parts.
const RESYNC_WORLD_MAX_AGE_MS: u32 = 1_000;
/// ... but a snapshot this old is always taken: an asked-for one is about
/// as old when it is here. A line whose way leaves less than this gets the
/// freshest there is rather than none (the load is then late by the rest).
const RESYNC_WORLD_MIN_AGE_MS: u32 = 500;
/// A resync whose only snapshot is too old keeps its generation and its
/// acked replay and asks for a newer one; after this long without one
/// it asks again ...
const RESYNC_WORLD_WAIT: Duration = Duration::from_secs(1);
/// ... this many times at most: then its world goes from the newest
/// snapshot there is, however old (within [`TICK_REACH_MS`]).
const RESYNC_WORLD_WAITS: u32 = 3;
/// A resynced client's world, with a snapshot for it there, waits for what
/// it needs (`Peer::sync_pending`) this long in all at most: then it goes
/// without the rest, said in the log (a busy match on a lossy line acks
/// some relay late all the time) ...
const RESYNC_GATE_MAX: Duration = Duration::from_millis(1_000);
/// ... and that gate is looked at this often at most, not on every poll
/// (some 500 a second at 20 players).
const RESYNC_GATE_EVERY: Duration = Duration::from_millis(15);
/// Relays sent to a resynced client while its world waits, tracked for its
/// gate at most (the replay and the roster changes always are): the acked
/// ones go at every look, so this holds a round trip of relays and their
/// lost copies. Past it the newest relays are not waited for: they are
/// the least likely to be older than the world.
const SYNC_PENDING_RELAYS: usize = 256;
/// Generations of one resync at most: past them nothing more is done again
/// for a replay lost or not acked, and the world goes by its gate's limit.
const RESYNC_GENERATIONS_MAX: u32 = 8;
/// The longest round trip that counts for a resynced world's age limit
/// (`trip_one_way`): a line longer than this is no use anyway.
const RESYNC_TRIP_MAX_MS: u32 = 1_500;
/// A resync's replay lost or not acked has the sync done again this many
/// times at most; past that its world goes all the same.
const REPLAY_REDONE: u32 = 3;
/// For the same reason a resynced client's world that was not loaded (acked,
/// less its way back) this long after its snapshot's time is sent again from
/// a fresh snapshot, as the next generation ...
const RESYNC_LOAD_BY_MS: u32 = 1_200;
/// ... this many times a resync at most: then a slower load is taken.
const SLOW_LOADS_REDONE: u32 = 3;
/// A game that enters and would get a roster change past its horizon
/// (`finish_sync`) is started again from a fresh world this many times at
/// most: then the change goes all the same.
const LATE_ROSTER_REDOS: u32 = 2;
/// The entries a game keeps of each player's inputs (its rings, match.md
/// section 30): a world loaded when some player has more inputs than this
/// after the snapshot is played wrong from its first tick, whatever the
/// time. Live, 2026-10-15: one player's precise inputs put 32 in 1.1 s,
/// under [`RESYNC_LOAD_BY_MS`]; and loads acked 1.5 to 2.4 s after their
/// snapshot were all wrong, after the time rule had given up on them.
const INPUT_RING: usize = 27;
/// A resynced game whose reports are wrong from this soon after its world
/// went (`judge_from`) is resynced again without the wait between resyncs
/// ([`QUICK_REDOS`] times in a row at most): the world did not take (it
/// was loaded past a ring, or relays came while the load held the game and
/// were moved).
const LOAD_WRONG_MS: u32 = 1_500;
/// How many resyncs in a row, none followed by a right report, may be
/// done again at once.
const QUICK_REDOS: u32 = 2;
/// A client resynced this many times within a minute waits the whole
/// interval again, even after a resync that was agreed with
/// (`Timing::resync_after_agreed`): a game that goes wrong every few
/// seconds is not resynced every few seconds.
const RESYNCS_PER_MINUTE: usize = 4;
const RESYNC_WINDOW: Duration = Duration::from_secs(60);
/// A world still on its way is done again once some player has this many
/// inputs after its snapshot (counted to the match clock, what the game
/// will have when the world gets there). With one input a tick that is
/// 1.3 s, after [`RESYNC_LOAD_BY_MS`]: this rule is for the players with
/// precise inputs, more than one a tick.
const INPUT_RING_SAFE: usize = 26;
/// The parts of a resynced client's world go again this long after their
/// last way out, at least (0xE8's own pace is 500 ms; a burst of lost parts
/// made loads of 1.4 to 1.6 s live, 2026-10-14) ...
const RESYNC_WORLD_RESEND_MIN: Duration = Duration::from_millis(120);
/// ... and its round trip and this, so that a part is not sent again before
/// its ack can be back.
const RESYNC_WORLD_RESEND_MARGIN_MS: u32 = 50;
/// A resynced client's clock is put this far past the match clock at most
/// for the way of its 0xE5 (half its round trip).
const RESYNC_ONE_WAY_MAX_MS: u32 = 250;
/// A resynced client's clock is put this far past its own clock and past
/// its newest input seen (`Game::sync_start`).
const RESYNC_CLOCK_MARGIN_MS: u32 = 100;
/// The replay of a resync (the others' inputs and the roster changes since
/// its snapshot) is acked within this, or the sync is done again.
const REPLAY_ACK_WAIT: Duration = Duration::from_secs(3);
/// An answer to an own input held while the client loaded its world goes
/// after the load while the input is this far behind the client's clock at
/// most: after the load the client refuses 0xF1/0xEF for a tick it has no
/// state before (its history is 500 ms, match.md section 30).
const ANSWER_REACH_MS: u32 = 450;
/// Answers to a client's own inputs remembered until acked.
const ANSWERS_KEPT: usize = 64;
/// Hash reports kept per client while its world is on the way.
const LOAD_REPORTS_KEPT: usize = 64;
/// A client whose clock is still being corrected is not resynced for at
/// most this long.
const RESYNC_DEFER_MAX: Duration = Duration::from_secs(20);
/// Late inputs are logged once a client has had this long of them.
const DISCARD_LOG: Duration = Duration::from_secs(1);
/// Relays, discards and roster changes awaiting their ack, per client.
const IN_FLIGHT_KEPT: usize = 4096;
/// A client that enters while others play starts its clock this far past
/// the snapshot it starts from.
const SYNC_LEAD_MS: u32 = 350;
/// With no fresh snapshot to be had for this long, a waiting client starts
/// from the newest one there is, if the wire's ticks still reach it.
const SNAPSHOT_GIVE_UP: Duration = Duration::from_secs(20);
/// A client that has waited this long for a world, in a match where no
/// client has shown a running simulation for as long, waits for nothing:
/// the match's world is lost and the match is over (`world_lost`).
const WORLD_LOST_AFTER: Duration = SNAPSHOT_GIVE_UP;
/// A hash report that goes on from the client's last one by this much at
/// most shows a game that simulates: it reports every tick or every
/// second one (50 or 100 ms). A game that stands still names one tick
/// again and again, and that byte reads as a tick 12.8 s later each time
/// the match clock has gone on by as much.
const SIMULATION_STEP_MS: u32 = 1_000;
/// A time on the wire is a tick within ±127 of the receiver's clock
/// (6.35 s). A client started from a snapshot older than this would read
/// the others' inputs at wrong ticks, so none older is ever started from
/// while others play. (0.21d, with whole times, took any.)
const TICK_REACH_MS: u32 = 5_000;
/// 0xE7 carries an i16.
const MAX_CLOCK_CORRECTION_MS: u32 = i16::MAX as u32;
/// How many clients are asked for a snapshot at a time.
const LARGE_MATCH_PLAYERS: usize = 4;
const SNAPSHOT_SOURCES: usize = 2;
/// The least time between two periodic requests for an upload.
const SNAPSHOT_WAIT: Duration = Duration::from_secs(3);
/// How long an upload waits for the hash reports that check it.
const CANDIDATE_WAIT: Duration = Duration::from_secs(3);
/// An upload is taken for this long after it was asked for.
const ASKED_FOR: Duration = Duration::from_secs(6);
/// Hash reports are kept this long (match ms).
const HASH_KEEP_MS: u32 = 30_000;
/// The match ends this long after the clients' time limit.
const END_GRACE_MS: u32 = 2_000;
/// A match without a client for this long is over.
const ABANDONED: Duration = Duration::from_secs(60);
/// A match everyone left is over at once, once every seat came in or the
/// match ran this long.
const SEATS_COME_WITHIN_MS: u32 = 60_000;
/// An upload is for a tick at most this far behind the match clock ...
const UPLOAD_MAX_AGE_MS: u32 = 5_000;
/// ... and at most this far ahead of it.
const UPLOAD_MAX_AHEAD_MS: u32 = HORIZON_MS;
/// Hash reports are for the event-horizon item: never this far ahead.
const HASH_MAX_AHEAD_MS: u32 = 1_000;
/// Inputs older than this are dropped without an answer.
const INPUT_MAX_AGE_MS: u32 = 5_000;
/// Relayed inputs are kept this long for clients that start from a
/// snapshot (none older than `TICK_REACH_MS` is started from).
const INPUTS_KEPT_MS: u32 = 60_000;
/// Rejected or unchecked snapshots saved at most this many times a match.
const MAX_DOUBTFUL_DUMPS: u32 = 40;
/// The worlds of players found out of step asked for (`ask_wrong_world`):
/// at most this many a player in a match, and as many a match.
const WRONG_WORLDS_PER_PLAYER: u32 = 2;
const WRONG_WORLDS_PER_MATCH: u32 = 8;
/// Ticks a client is known to have missed, the latest ones.
const MISSED_KEPT: usize = 64;
/// A tie names a client out of step only for a tick it missed, or an own
/// input it was told to drop, this close before the disputed tick or
/// before the start of the disagreement it is in: what it lacks shows at
/// once (a lost relay is wrong from its own tick). An older one changed
/// nothing, or it would have been resynced since; on a long
/// line some relay is always unacked, and the old rule named such a client
/// in every tie (live, 2026-10-15: 22 of 38 ties, the wrong one in them).
const MISSED_RECENT_MS: u32 = 1_000;
/// Plausibility: a player's kills at most one per this long, plus one.
const MIN_MS_PER_KILL: u32 = 2_000;
/// Plausibility: batteries home at most one per this long, plus one.
const MIN_MS_PER_BATTERY: u32 = 10_000;
/// Clock samples kept per client; the one taken over the shortest round
/// trip counts (`best_sample`).
const CLOCK_SAMPLES: usize = 4;
/// A client's latest round trips kept: the shortest of them is what its
/// link can do, and a sample is off by half of what its own was longer, at
/// most.
const ROUND_TRIPS_KEPT: usize = 32;
/// A client clock this far off is corrected (0xE7).
const CLOCK_TOLERANCE_MS: i64 = 10;
/// A game makes a correction up at the share of real time that its build
/// has for it (`game_clock::factor`: the original's 5 %, the client patch's
/// 30 % from revision 8), and a sample taken meanwhile says where its
/// clock was. This long after it is done the next samples are waited for.
const CORRECTION_SETTLE: Duration = Duration::from_secs(1);
/// A lag sample further off than this is not used: the 0xDB's clock field
/// is the client's game clock (PROVEN: the timer 0xE5 sets and 0xE7
/// shifts, game+0x52D8), but only its low 16 bits, unwrapped here against
/// the match clock, which goes wrong past half a minute.
const MAX_SANE_LAG_MS: i64 = 5_000;
/// Inputs first seen this late probably lost their first copy.
const LATE_INPUT_MS: u32 = 200;
/// A player's refused inputs are logged one by one up to this many.
const REFUSED_LOGGED: u32 = 8;
const LATE_SHARE_WEIGHT: f32 = 0.01;
/// With no majority, the side with the better links must outweigh the next
/// by this much to be taken as right.
const TRUST_MARGIN: f32 = 0.15;
/// A client counts as reporting hashes if one came within this.
const HASH_REPORTING: Duration = Duration::from_secs(1);
/// Two or more clients report and no tick got two reports for this long:
/// they are on opposite hash phases (match.md section 14).
const HASH_STARVED_AFTER: Duration = Duration::from_secs(5);
/// The ticks of each client's latest hash reports kept to see that.
const HASH_TICKS_KEPT: usize = 64;
/// How often that is logged while it lasts.
const HASH_STARVED_LOG: Duration = Duration::from_secs(30);
/// While it lasts two clients are asked for the same tick's world, this
/// often at most (a patched client is not asked twice within a second).
const STARVED_REFRESH: Duration = Duration::from_secs(3);
/// A request is aimed at the middle of a tick of the client's clock, less
/// half of how late the loop may send it (it polls every 5 ms).
const AIM_MS: i64 = (TICK_MS / 2) as i64 - 3;
/// An aimed request waits at most this long for its moment.
const AIM_WAIT_MS: i64 = 1_000;
/// A late input is moved at most this many ticks past the first open one.
const MOVE_STEPS: u32 = 4;
/// A roster change (0xE9, 0xEA) is for a tick this far ahead: it is with
/// every client before its tick, so none stamps it onto a later one of
/// its own ("CORRECTED", match.md section 5.1). Another 0.26g server ran
/// with 300 ms against real clients (upstream_compare.md topic 4).
const ROSTER_LEAD_MS: u32 = 300;
/// How often clients' disagreeing hashes are logged at most.
const DISAGREEMENT_LOG: Duration = Duration::from_secs(10);
/// How often each live client is pinged (0xE6): its round trip and clock.
const PING_EVERY: Duration = Duration::from_secs(1);
/// A ping's answer comes within this, or it is not that ping's.
const PING_ANSWER_MS: u16 = 5_000;
/// A sync's 0xE5 goes again this often until the client answers it (0xDB):
/// both are sent once and may be lost.
const SET_TIME_EVERY: Duration = Duration::from_millis(150);
/// After this many unanswered 0xE5 the snapshot goes anyway: a client that
/// takes its time but whose answers do not come (or are not read right
/// here) must still get its world.
const MAX_SET_TIMES: u32 = 5;
/// How often (match ms) each player's ping is logged.
const PING_LOG_MS: u32 = 30_000;
/// A match with test bots and no real client connected for this long is
/// over. A real client that loads its level is connected all the while.
const BOTS_ALONE: Duration = Duration::from_secs(45);

/// The next tick at or after `time`.
fn grid_up(time: u32) -> u32 {
    time.div_ceil(TICK_MS) * TICK_MS
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    /// Sent 0xE3; waits for the port probe (0xD9).
    Connecting,
    /// Sent 0xE4; the client loads the level until it is ready (0xDA).
    Loading,
    /// Ready, waiting for a snapshot to start from.
    WaitingSnapshot { since: Instant, entering: bool },
    /// A snapshot (the one at `time`) is there to start from: sent 0xE5,
    /// again every `SET_TIME_EVERY`, until the client answers. A client
    /// resynced while it plays is then sent the replay (`send_replay`), and
    /// its world goes once all of that is acked.
    Timing {
        time: u32,
        entering: bool,
        last: Instant,
        sent: u32,
    },
    /// Sent 0xE5 and 0xE8; the rest follows once 0xE8 is acked.
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

/// What a tracked message was.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Flight {
    /// Another player's input (0xEB/0xEC, 0xF3).
    Relay,
    /// Its own input dropped or moved (0xF1/0xF2, 0xEF/0xF0).
    Discard,
    /// 0xE9 or 0xEA: sent once more if it was never acked.
    Roster(ServerMessage),
}

/// What kind of message a resynced client's world waits for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GateKind {
    /// An input of the others in its replay (`send_replay`).
    Replay,
    /// A roster change, in its replay or since: kept until acked.
    Roster,
    /// An input of the others relayed since its replay: waited for only
    /// if its tick is not past the world's.
    Relay,
}

/// A message a resynced client's world waits for, until acked.
#[derive(Clone, Copy, Debug)]
struct Gated {
    serial: u64,
    sent: Instant,
    tick: u32,
    kind: GateKind,
}

impl Gated {
    /// Whether a world of `time` waits for it: the replay and the roster
    /// changes always, a relay only up to the world's time (a later one
    /// that came after the world is no older than it, and never moved).
    fn holds(&self, time: u32) -> bool {
        self.kind != GateKind::Relay || self.tick <= time
    }
}

/// A message sent to a client, waiting for its ack.
struct InFlight {
    serial: u64,
    flight: Flight,
    /// The match time it is for.
    time: u32,
    /// A relay's first send (queued then; the transport sends it within
    /// `Config::gather`).
    sent: Option<Instant>,
}

/// A 0xE5 of the sync under way.
#[derive(Clone, Copy)]
struct SetTimeSent {
    echo: u16,
    at: Instant,
    /// The clock it gave.
    start: u32,
    /// A 0xE7 of offset 0 went ahead of it in its datagram: a game that
    /// took it has no correction running (`Game::send_set_time`).
    ends_correction: bool,
}

/// The 0xE5s of one sync remembered, to know their answers.
const SET_TIMES_KEPT: usize = 8;

/// The others' inputs for a game that waits for its first world go out
/// this often, as many as came in one datagram: before its world is
/// loaded an input is only put on a list (0x1FAB50 with an empty history
/// refuses and moves nothing, match.md section 18), and one datagram for
/// each input was 265 datagrams a second to a game that acks 10 times a
/// second (live, 2026-10-13).
const WAITING_RELAY_FLUSH: Duration = Duration::from_millis(50);
/// Such an input is sent again this often until it is acked, not every
/// 50 ms: the waiting game's acks come every 100 ms.
const WAITING_RELAY_RESEND: Duration = Duration::from_millis(150);
/// How far past its own clock the match may run for a game that waits for
/// its first world. That game's clock stands where 0xE5 put it: its `move`
/// returns at once (0x2D2E4A) and nothing else moves the time the handlers
/// read a tick against (core+0x191B8, 0x27C2D5). A tick on the wire is
/// right within 127 ticks of that (6.35 s), and the others' inputs are of
/// the match clock or a little ahead of it: 850 ms are left for that.
const WAITING_REACH_MS: u32 = 5_500;
/// 0xE8's own lifetime in the client's table (`udp_transport::Table`).
const WORLD_LIFETIME: Duration = Duration::from_millis(1750);

/// A relay is of use to a game until its world time is this far past the
/// relay's tick: its history is 500 ms, and one that comes later is moved
/// to a tick of the game's own choosing (match.md section 5.1). It is sent
/// again until its last copy can still arrive by then (`relay_deadline`),
/// and at least for the table's 250 ms from its first send: a relay that
/// waited in a stall is lost there, not sent late, and one whose copies
/// were all lost in 250 ms still had a few hundred ms of use (live,
/// 2026-10-14).
const RELAY_USEFUL_MS: u32 = 500;
/// A late input is relayed at its tick (`LateInputs::Relay`) only when it
/// reaches every other game this much before that game's history has
/// passed its tick: room for a copy lost on the way.
const LATE_RELAY_MARGIN_MS: u32 = 75;
/// A drop order (0xF1/0xF2, 0xEF/0xF0) is sent again for this long after
/// it is queued. Its owner's world time is not known here (its inputs are
/// late because its clock or its line is), and one that comes after the
/// owner's history passed the tick is refused quietly (match.md section
/// 5.2).
const DROP_ORDER_LIFE: Duration = Duration::from_millis(450);

/// The last 0xE6 sent.
#[derive(Clone, Copy)]
struct PingSent {
    a: u16,
    b: u16,
}

/// One measure of how far a client's clock is behind the match clock (ms;
/// negative: ahead), the round trip it was taken over and when.
#[derive(Clone, Copy, Debug)]
struct LagSample {
    lag: i64,
    round_trip: u32,
    at: Instant,
}

/// A client whose hashes disagree with the right ones, in a row.
struct Divergence {
    first: u32,
    last: u32,
    since: Instant,
}

/// Inputs of one client that came too late in a short run.
struct DiscardRun {
    since: Instant,
    count: u32,
    worst_ms: u32,
}

/// An answer to one of a client's own inputs: dropped (0xF1/0xF2, 0xF5) or
/// moved (0xEF/0xF0, 0xF4). From a resync's 0xE5 until its world is loaded
/// the client's history is cleared, and whether it takes either then is
/// not settled (match.md section 30): an answer for then is held and goes
/// after the load (`Game::send_held_answers`).
#[derive(Clone, Debug)]
struct Answer {
    message: ServerMessage,
    /// The time of the input, as its owner has it.
    time: u32,
    /// Where the others have it, if it was moved.
    moved_to: Option<u32>,
    /// A player input's answer is kept in `in_flight`; a profile input's
    /// is not.
    tracked: bool,
    /// The copy sent, until it is acked.
    serial: Option<u64>,
}

struct Peer {
    addr: SocketAddr,
    session_id: u32,
    transport: Transport,
    stage: Stage,
    /// The sync generation of the last 0xE5/0xE8 sent to it: 0xFF once it
    /// is ready, as the client's own.
    generation: Generation,
    /// The 0xE5s of that generation sent, and whether the client answered
    /// one: its clock is then set, and the snapshot may follow.
    set_times: VecDeque<SetTimeSent>,
    time_confirmed: bool,
    /// It answered a 0xE5 of this generation (`time_confirmed` is also set
    /// when one goes unanswered): a 0xE9 it gets now is read on that clock.
    time_answered: bool,
    /// The entries of the others sent to a game that enters before its
    /// world is loaded: their time, the player, the serial, and whether it
    /// went after the world (an entry ahead of it in the same burst is
    /// there before it, and its ack may never be named: a game names only
    /// its newest few seqs).
    early_entries: Vec<(u32, usize, u64, bool)>,
    /// Its starts done again for roster changes that would have come past
    /// its horizon ([`LATE_ROSTER_REDOS`] at most).
    late_roster_redos: u32,
    /// It has a game clock: it answered a 0xE5, or took a whole sync. A
    /// message at a tick (0xE9, 0xEA) means nothing to a client without.
    clock_set: bool,
    ping: Option<PingSent>,
    next_ping: Instant,
    /// It uploaded a snapshot, so it answers 0xF6.
    patched: bool,
    resources_loaded: bool,
    installed_roster: usize,
    preparing: Option<LoadingProfile>,
    /// Since when it loads: from its connection (a game that never gets as
    /// far as its port probe loads too), until it has loaded.
    loading_since: Option<Instant>,
    /// It loaded past `late_join::LOADING_GRACE`: newcomers no longer wait
    /// for it, and if it loads without a place taken meanwhile it is let
    /// go, to come again with the whole roster (`late_join`).
    left_behind: bool,
    /// When it was last asked for an upload and has not given one since:
    /// the only time its upload is taken, but for the initial world.
    asked: Option<Instant>,
    /// The match clock when that request went.
    asked_clock: Option<u32>,
    /// When it was first asked for an upload after the last one it gave
    /// (or ever): how long a match against bots has waited for its world
    /// (`check_pvb_stall`).
    unanswered_since: Option<Instant>,
    /// A request waiting for its moment (match clock): aimed at a tick.
    ask_at: Option<u32>,
    /// Bounds `[low, high)` on its upload offset: asked at match clock
    /// `c`, it uploads the tick `floor((c + offset) / 50)`. Each upload
    /// gives 50 ms of bounds; they hold while its clock and link do.
    upload_offset: Option<(i64, i64)>,
    reassembler: Reassembler,
    /// Being resynced while it played: since the first 0xE5 of the resync,
    /// until its world is acked. Its game goes on with a cleared history
    /// on the clock the 0xE5 gave it (match.md section 30).
    resync_from: Option<Instant>,
    /// The snapshot time the replay of its resync was sent for, and when;
    /// what was sent of it and since, until acked.
    replayed: Option<(u32, Instant)>,
    /// What its world waits for, not acked yet: that replay and the roster
    /// changes (`sync_pending`), and the relays sent since (`sync_relays`,
    /// [`SYNC_PENDING_RELAYS`] at most). Acked entries go at every look at
    /// the gate. Emptied at every new generation, when the world goes, and
    /// at the end of the resync.
    sync_pending: Vec<Gated>,
    sync_relays: Vec<Gated>,
    /// The most entries `sync_pending` had in this match (for the tests).
    sync_pending_most: usize,
    /// When its world first had a snapshot and waited for its gate, in
    /// this generation ([`RESYNC_GATE_MAX`]); when the gate was last looked
    /// at ([`RESYNC_GATE_EVERY`]) and how often in this match.
    gate_since: Option<Instant>,
    gate_checked: Option<Instant>,
    gate_checks: u64,
    /// Its resync's replay done again for being lost or not acked
    /// ([`REPLAY_REDONE`] at most).
    replay_redos: u32,
    /// The generations of this resync ([`RESYNC_GENERATIONS_MAX`]).
    resync_generations: u32,
    /// Waits for a newer snapshot with the replay acked, in this
    /// generation ([`RESYNC_WORLD_WAITS`] at most), and when the last
    /// began.
    world_waits: u32,
    world_wait_since: Option<Instant>,
    /// Answers to its own inputs held while it is resynced.
    held_answers: Vec<Answer>,
    /// Answers sent to it, until acked: one that had not arrived when its
    /// resync began goes again after the load.
    sent_answers: VecDeque<Answer>,
    /// A resync of it starts from no snapshot older than this: one of its
    /// held answers is for a tick its game could no longer take.
    resync_floor: u32,
    /// Worlds of this resync done again for not being loaded in time
    /// ([`RESYNC_LOAD_BY_MS`]); past [`SLOW_LOADS_REDONE`]: a slow load
    /// taken.
    slow_loads: u32,
    /// Syncs done again in a row for held answers its game could no longer
    /// take (`send_held_answers`), [`SLOW_LOADS_REDONE`] at most.
    answer_redos: u32,
    sync_sent_at: Option<Instant>,
    /// Back in a match it left: when it entered for the others, before it
    /// was started, until it is live.
    entered_early: Option<u32>,
    /// Asked for its world as it was found out of step, to look at
    /// (`ask_wrong_world`): the tick of the majority's verdict and how
    /// many it gave so far.
    wrong_world_asked: Option<u32>,
    wrong_worlds: u32,
    /// The times of its inputs already handled.
    inputs_seen: HashSet<u32>,
    profiles_seen: HashSet<(u32, u8)>,
    /// Its inputs the others have: the time it gave, the time they are at.
    placed: VecDeque<(u32, u32)>,
    placed_profiles: VecDeque<(u32, u32)>,
    last_resync: Option<Instant>,
    /// Its latest resyncs, [`RESYNCS_PER_MINUTE`] at most.
    resynced_at: VecDeque<Instant>,
    /// When its last hash report for a tick still open came.
    last_hash_at: Option<Instant>,
    /// The ticks of its latest hash reports (0xDF, not uploads).
    hash_ticks: VecDeque<u32>,
    /// When its game last showed that it simulates: it loaded a world, a
    /// hash report went on from its last one, it gave a world of the
    /// present, or an input of its player was relayed. A link that is up
    /// does not show it: a game whose window is not active, or that has
    /// died and is not closed yet, acks and answers pings (live,
    /// 2026-10-12: 78 s of them from a game whose world stood still).
    simulating_at: Option<Instant>,
    /// The tick of its last hash report, whatever became of the report.
    reported_tick: Option<u32>,
    /// When it last started from a snapshot as a newcomer.
    synced_at: Option<Instant>,
    /// The latest estimates of how far its clock is behind the match clock.
    lags: VecDeque<LagSample>,
    /// Its last round trip, in ms.
    round_trip: u32,
    /// Its latest round trips ([`ROUND_TRIPS_KEPT`]).
    round_trips: VecDeque<u32>,
    /// The share of real time by which its game brings its clock up or
    /// down (`game_clock::factor` of the patch revision it signed in with).
    clock_factor: f32,
    correcting_until: Option<Instant>,
    inputs_received: u32,
    inputs_late: u32,
    inputs_discarded: u32,
    /// Its inputs that came after the server's horizon and were relayed at
    /// their tick all the same (`LateInputs::Relay`).
    inputs_relayed_late: u32,
    inputs_moved: u32,
    /// Inputs with a rotation no client makes: not relayed.
    inputs_refused: u32,
    resyncs: u32,
    /// Resyncs since its reports last agreed with the right ones.
    restores_unagreed: u32,
    latest_input_ms: u32,
    /// The recent share of its inputs that came late (0 to 1).
    late_share: f32,
    resync_deferred_since: Option<Instant>,
    diverged: Option<Divergence>,
    /// Its reports for ticks up to this are of the world it left.
    judge_from: u32,
    discard_run: Option<DiscardRun>,
    lag_ms: Option<i64>,
    correction: Option<(Instant, i64, Duration)>,
    /// The clock the last 0xE5 it answered gave it, and when that went;
    /// none once a 0xE7 moved its clock.
    clock_given: Option<(u32, Instant)>,
    /// Its hash reports for ticks past the snapshot of the world on its
    /// way (`Stage::Syncing`), with when each came: one the majority
    /// agrees with, made after the world could be there, shows that the
    /// world is loaded (`load_proven`).
    load_reports: Vec<(u32, u32, Instant)>,
    /// Until when a 0xE7 it was sent may still be running in its game
    /// (the time it takes, from its last possible start, and a round
    /// trip): every 0xE5 until then goes behind a 0xE7 of offset 0, which
    /// ends it (a 0xE5 does not, match.md section 29).
    correction_ends: Option<Instant>,
    /// The last 0xE7, until acked: its serial, when it went, the offset and
    /// how long the game takes for it.
    clock_offset_pending: Option<(u64, Instant, i16, Duration)>,
    /// Ticks whose relay, discard or roster change it never acked since it
    /// last started from a snapshot: it lacks what the others have.
    missed: VecDeque<u32>,
    /// Ticks of its own inputs it was told to drop (0xF1) since it last
    /// started from a snapshot, the latest [`MISSED_KEPT`]: a game whose
    /// drop order came past its history keeps an input nobody else has.
    discarded: VecDeque<u32>,
    in_flight: VecDeque<InFlight>,
    relays_acked: u32,
    relays_lost: u32,
    /// The 0xEA with its own index that ends the match for it.
    farewell: Option<(u64, ServerMessage)>,
    /// The world its game is starting from will not be the others' once
    /// loaded (`Game::first_load_loses_the_standing`): it is started once
    /// more as soon as it has it.
    reload_due: bool,
    /// How often that was done.
    reloads: u32,
    /// The others' inputs that came while it waits for its first world,
    /// not yet sent on ([`WAITING_RELAY_FLUSH`]), and when they may go.
    held: Vec<ServerMessage>,
    held_until: Instant,
}

impl Peer {
    fn is_asked(&self, now: Instant) -> bool {
        self.asked
            .is_some_and(|at| now.saturating_duration_since(at) < ASKED_FOR)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Snapshot {
    time: u32,
    /// `game_state_history_item::serialize(mask)`: 0xE8's state without
    /// its time.
    body: Vec<u8>,
}

/// An upload that waits to be checked against the hash reports.
struct Candidate {
    from: usize,
    snapshot: Snapshot,
    since: Instant,
}

/// What the newest trusted snapshot said.
struct Observed {
    time: u32,
    /// How many clients reported its CRC for its tick.
    vouchers: u8,
    statistics: Option<Statistics>,
    rules: Option<Rules>,
    standing: Option<Standing>,
    /// By place: what its player has left.
    supplies: Vec<Option<Supplies>>,
}

/// Something relayed, kept for clients that start from a snapshot.
#[derive(Clone, Copy)]
enum Relayed {
    Input(PlayerInput),
    Profile { tick: u8, value: u8 },
}

pub(crate) struct Game {
    pub id: u32,
    port: u16,
    formed: Arc<Formed>,
    settings: Settings,
    created: Instant,
    peers: Vec<Option<Peer>>,
    admissions: Vec<Option<Admission>>,
    next_proposal: u32,
    /// Set when the first snapshot comes: the match clock runs from there.
    clock_start: Option<(Instant, u32)>,
    snapshot: Option<Snapshot>,
    /// The newest upload no majority contradicts, checked or not.
    recent: Option<Snapshot>,
    requested_at: Option<Instant>,
    candidates: Vec<Candidate>,
    /// Everything relayed: its time, whose it is, what.
    inputs: Vec<(u32, usize, Relayed)>,
    /// Roster changes: time, place, entered (else left).
    events: Vec<(u32, usize, bool)>,
    /// CRC reports by tick and client.
    hashes: BTreeMap<u32, HashMap<usize, u32>>,
    hashes_checked_until: u32,
    /// Reports for ticks up to this are not judged: the games were
    /// installing a new place then ([`Game::installing`]).
    hashes_unjudged_until: u32,
    /// Since when two or more clients have been reporting with no tick
    /// that two of them named.
    hash_unshared_since: Option<Instant>,
    /// That has lasted [`HASH_STARVED_AFTER`]: no world can be trusted.
    hash_starved: bool,
    hash_starved_logged: Option<Instant>,
    hash_starved_warnings: u32,
    first_entered: Option<u32>,
    /// The clients' timelimit rule, stepped alongside them.
    rule: TimelimitRule,
    /// When the mode's own rule reports the match over (match ms).
    mode_end_at: Option<u32>,
    state: Option<Observed>,
    best_streaks: Vec<u16>,
    /// By place: the kit choices relayed for a tick before play began
    /// (`TimelimitRule::before_play`), when a patched game (patch 31)
    /// respawns a live player in the chosen kit where it stands. At most as
    /// many lives more than its deaths: [`PlayerResult::kit_changes`].
    kit_changes: Vec<u16>,
    /// By place: whether a game of it loading long is left behind
    /// (`late_join::LOADING_GRACE`).
    load_waits: Vec<late_join::LoadWait>,
    /// The end has come: waiting for a last snapshot since then.
    ending: Option<(Instant, MatchEnd)>,
    finished_at: Option<Instant>,
    empty_since: Option<Instant>,
    next_refresh: Option<Instant>,
    disagreement_logged: Option<Instant>,
    respawn_points_logged: bool,
    doubtful_dumps: u32,
    /// Worlds of players out of step asked for (`ask_wrong_world`).
    wrong_worlds_asked: u32,
    pings_logged_at: u32,
    /// Since when a match with test bots has had no real client connected.
    bots_alone_since: Option<Instant>,
    /// By place: the script of the server's own bot ([`driven`]).
    drives: Vec<Option<super::drive::Drive>>,
    /// The server's bots that walk the map, with `SRP_MAPDATA_DIR`
    /// ([`driven`], stage 2).
    walkers: Option<driven::Walkers>,
    result: Option<MatchResult>,
    result_made: bool,
    /// The players who left by their own word (0xE0) and were not asked
    /// about yet ([`Game::take_quits`]): session and account.
    quits: Vec<(u32, u32)>,
    /// `SRP_RECORD_DIR`: every datagram of the match ([`super::record`]).
    recording: Option<Recording>,
}

impl Game {
    /// A match whose clients connect to `port`.
    pub fn new(formed: Arc<Formed>, port: u16, settings: Settings, now: Instant) -> Self {
        let players = formed.places.len();
        let rule = TimelimitRule::new(TimelimitParams::of(formed.map, players as u8));
        let walkers = driven::Walkers::new(&formed, &settings);
        Self {
            id: formed.match_id,
            port,
            settings,
            created: now,
            peers: (0..players).map(|_| None).collect(),
            admissions: (0..players).map(|_| None).collect(),
            next_proposal: 0,
            clock_start: None,
            snapshot: None,
            recent: None,
            requested_at: None,
            candidates: Vec::new(),
            inputs: Vec::new(),
            events: Vec::new(),
            hashes: BTreeMap::new(),
            hashes_checked_until: 0,
            hashes_unjudged_until: 0,
            hash_unshared_since: None,
            hash_starved: false,
            hash_starved_logged: None,
            hash_starved_warnings: 0,
            first_entered: None,
            rule,
            mode_end_at: None,
            state: None,
            best_streaks: vec![0; players],
            kit_changes: vec![0; players],
            load_waits: vec![late_join::LoadWait::First; players],
            ending: None,
            finished_at: None,
            empty_since: Some(now),
            next_refresh: None,
            disagreement_logged: None,
            respawn_points_logged: false,
            doubtful_dumps: 0,
            wrong_worlds_asked: 0,
            pings_logged_at: 0,
            bots_alone_since: None,
            result: None,
            result_made: false,
            quits: Vec::new(),
            recording: None,
            drives: Self::new_drives(&formed),
            walkers,
            formed,
        }
    }

    /// Records this match from now on (`SRP_RECORD_DIR`): its header first.
    pub(crate) fn start_recording(&mut self, recorder: &Recorder, now: Instant) {
        let map = self.formed.map;
        let header = serde_json::json!({
            "match_id": format!("{:#x}", self.id),
            "map_id": map.id,
            "map": map.name,
            "mode": map.mode,
            "match_level": self.formed.match_level,
            "growing_roster": self.formed.growing_roster,
            "initial_players": self.formed.initial_players,
            "late_inputs": format!("{:?}", self.settings.late_inputs),
            "places": self.places_note(0),
        });
        self.recording = Some(recorder.open(self.id, header, now));
    }

    /// The places from `from` on, for a recording.
    fn places_note(&self, from: usize) -> serde_json::Value {
        let places: Vec<serde_json::Value> = self
            .formed
            .places
            .iter()
            .enumerate()
            .skip(from)
            .map(|(index, place)| {
                serde_json::json!({
                    "place": index,
                    "account_id": place.account_id,
                    "name": place.name,
                    "team": place.team,
                    "squad": self.formed.profiles.get(index).map(|profile| profile.squad),
                    "bot": place.bot,
                })
            })
            .collect();
        places.into()
    }

    /// A datagram of the client at `index` the server took (its 0xD8 comes
    /// through here: `Server::connect`).
    pub(crate) fn record_in(&mut self, index: usize, datagram: &[u8], now: Instant) {
        if let Some(recording) = self.recording.as_mut() {
            recording.datagram_in(index, datagram, now);
        }
    }

    fn record_note(&mut self, note: serde_json::Value, now: Instant) {
        if let Some(recording) = self.recording.as_mut() {
            recording.note(&note, now);
        }
    }

    fn record_result(&mut self, result: &MatchResult, now: Instant) {
        if self.recording.is_none() {
            return;
        }
        let players: Vec<serde_json::Value> = result
            .players
            .iter()
            .map(|player| {
                serde_json::json!({
                    "place": player.place,
                    "account_id": player.account_id,
                    "team": player.team,
                    "entered": player.entered,
                    "present_at_end": player.present_at_end,
                    "left": player.left,
                    "time_in_match_ms": player.time_in_match_ms,
                    "score": player.score,
                    "kills": player.kills,
                    "deaths": player.deaths,
                    "longest_killstreak": player.longest_killstreak,
                })
            })
            .collect();
        let note = serde_json::json!({
            "note": "result",
            "end": format!("{:?}", result.end),
            "winner": result.winner,
            "team_points": result.team_points,
            "duration_ms": result.duration_ms,
            "played_ms": result.played_ms,
            "trusted_snapshot_ms": result.trusted_snapshot_ms,
            "trusted_by": result.trusted_by,
            "players": players,
        });
        self.record_note(note, now);
    }

    fn players(&self) -> usize {
        self.formed.places.len()
    }

    fn game_mode(&self) -> u8 {
        self.formed.map.mode
    }

    /// Whether the place is a test bot's (`SRP_BOT_ACCOUNTS`) or the
    /// server's own ([`driven`]): a player for the others, no witness of
    /// the world (the top of this file).
    fn is_bot(&self, index: usize) -> bool {
        self.formed
            .places
            .get(index)
            .is_some_and(|place| place.bot || place.driven)
    }

    /// The batteries of modes 2 and 3; none in Team Deathmatch.
    fn batteries(&self) -> usize {
        if self.game_mode() == mode::TEAM_DEATHMATCH {
            0
        } else {
            self.formed.map.victory_items_count as usize
        }
    }

    /// How the match ended, once it has: taken once.
    pub fn take_result(&mut self) -> Option<MatchResult> {
        self.result.take()
    }

    /// The players who left the match by their own word since this was
    /// last asked: each one's session and account. Such a player has given
    /// its place up (`Server::receive`).
    ///
    /// 0xE0 is that word. The client sends it at one place only, in
    /// `close_current_match` for the reason 2, "disconnected_by_initiator"
    /// (0x27A828..0x27A838; the jump table at 0x27A958). The reasons of a
    /// link that broke (1 "disconnected_by_connection_lost", 5 "expired
    /// ... message") go to 0x27A867 and send nothing on the match link,
    /// and a game that died sends nothing at all. PROVEN.
    pub fn take_quits(&mut self) -> Vec<(u32, u32)> {
        std::mem::take(&mut self.quits)
    }

    /// The match is dropped without having ended by itself (everyone left,
    /// or nobody came): its result, unless it gave one already.
    pub fn close(&mut self, now: Instant) -> Option<MatchResult> {
        if self.result_made {
            return self.result.take();
        }
        let end = if self.events.is_empty() && self.clock_start.is_none() {
            MatchEnd::NoShow
        } else {
            MatchEnd::Deserted
        };
        let present = vec![false; self.players()];
        self.result_made = true;
        let result = self.build_result(end, self.clock(now), &present);
        self.record_result(&result, now);
        Some(result)
    }

    /// The match clock in ms: 0 until the first snapshot, then running from
    /// that snapshot's time.
    fn clock(&self, now: Instant) -> u32 {
        match self.clock_start {
            Some((start, at)) => at + now.saturating_duration_since(start).as_millis() as u32,
            None => 0,
        }
    }

    /// The server's own ms, for round trips: its low 16 bits go out in
    /// 0xE5 and 0xE6.
    fn mono(&self, now: Instant) -> u32 {
        now.saturating_duration_since(self.created).as_millis() as u32
    }

    /// The roster place of the client at `addr`.
    pub fn peer_index(&self, addr: SocketAddr) -> Option<usize> {
        self.peers
            .iter()
            .position(|peer| peer.as_ref().is_some_and(|peer| peer.addr == addr))
    }

    /// The session of the client that was at place `index` and is gone
    /// (it left, or timed out) while its link is still kept.
    pub fn gone_session(&self, index: usize) -> Option<u32> {
        self.peers
            .get(index)?
            .as_ref()
            .filter(|peer| peer.stage == Stage::Gone)
            .map(|peer| peer.session_id)
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

    /// A client connected (0xD8) for roster place `index`. A client
    /// already there is replaced: it left. `revision` is the patch
    /// revision its game signed in with (0 when that is not known): how
    /// fast its clock takes a correction goes by it.
    pub fn attach(
        &mut self,
        index: usize,
        addr: SocketAddr,
        session_id: u32,
        revision: u8,
        mut transport: Transport,
        now: Instant,
    ) {
        if index >= self.peers.len() || self.finished_at.is_some() {
            transport.disconnect(now);
            return;
        }
        if self.peers[index].is_some() {
            self.leave(index, now);
        }
        // A place it left before, from this address (it left by its own
        // word and came again as a new place): that link is done with, and
        // the address is this place's.
        for (other, slot) in self.peers.iter_mut().enumerate() {
            if other != index
                && slot
                    .as_ref()
                    .is_some_and(|peer| peer.addr == addr && peer.stage == Stage::Gone)
            {
                *slot = None;
            }
        }
        self.reset_catchup(index, true);
        log::info!(
            "Match {:#x}: player {index} (session {session_id:#x}) connected from {addr}",
            self.id
        );
        // For the replay's sidecar: which games played the match
        // (`replay.rs`; the session and the address stay out of it).
        self.record_note(
            serde_json::json!({ "note": "revision", "place": index, "revision": revision }),
            now,
        );
        // Known from here on: the client learns it from the static info.
        transport.set_roster_size(self.snapshot_players() as u8);
        let mut peer = Peer {
            addr,
            session_id,
            transport,
            stage: Stage::Connecting,
            generation: Generation::AT_READY,
            set_times: VecDeque::new(),
            time_confirmed: false,
            time_answered: false,
            early_entries: Vec::new(),
            late_roster_redos: 0,
            clock_set: false,
            ping: None,
            next_ping: now,
            patched: false,
            resources_loaded: false,
            installed_roster: 0,
            preparing: None,
            loading_since: Some(now),
            left_behind: false,
            asked: None,
            asked_clock: None,
            unanswered_since: None,
            ask_at: None,
            upload_offset: None,
            reassembler: Reassembler::new(),
            resync_from: None,
            replayed: None,
            sync_pending: Vec::new(),
            sync_relays: Vec::new(),
            sync_pending_most: 0,
            gate_since: None,
            gate_checked: None,
            gate_checks: 0,
            replay_redos: 0,
            resync_generations: 0,
            world_waits: 0,
            world_wait_since: None,
            held_answers: Vec::new(),
            sent_answers: VecDeque::new(),
            resync_floor: 0,
            slow_loads: 0,
            answer_redos: 0,
            sync_sent_at: None,
            entered_early: None,
            wrong_world_asked: None,
            wrong_worlds: 0,
            inputs_seen: HashSet::new(),
            profiles_seen: HashSet::new(),
            placed: VecDeque::new(),
            placed_profiles: VecDeque::new(),
            last_resync: None,
            resynced_at: VecDeque::new(),
            last_hash_at: None,
            hash_ticks: VecDeque::new(),
            simulating_at: None,
            reported_tick: None,
            synced_at: None,
            lags: VecDeque::new(),
            round_trip: 0,
            round_trips: VecDeque::new(),
            clock_factor: game_clock::factor(revision),
            correcting_until: None,
            inputs_received: 0,
            inputs_late: 0,
            reload_due: false,
            reloads: 0,
            held: Vec::new(),
            held_until: now,
            inputs_discarded: 0,
            inputs_relayed_late: 0,
            inputs_moved: 0,
            inputs_refused: 0,
            resyncs: 0,
            restores_unagreed: 0,
            latest_input_ms: 0,
            late_share: 0.0,
            resync_deferred_since: None,
            diverged: None,
            judge_from: 0,
            discard_run: None,
            lag_ms: None,
            correction: None,
            clock_offset_pending: None,
            clock_given: None,
            load_reports: Vec::new(),
            correction_ends: None,
            missed: VecDeque::new(),
            discarded: VecDeque::new(),
            in_flight: VecDeque::new(),
            relays_acked: 0,
            relays_lost: 0,
            farewell: None,
        };
        // One port: the client probes it once and sends everything there.
        send(
            &mut peer,
            &ServerMessage::Ports {
                min: self.port,
                max: self.port,
            },
        );
        self.peers[index] = Some(peer);
        self.empty_since = None;
    }

    /// A datagram from the client at roster place `index`.
    pub fn receive(&mut self, index: usize, datagram: &[u8], now: Instant) {
        if self.peers.get(index).is_some_and(Option::is_some) {
            self.record_in(index, datagram, now);
        }
        let Some(peer) = self.peers.get_mut(index).and_then(Option::as_mut) else {
            return;
        };
        let skips = peer.transport.skips();
        let events = peer.transport.receive(datagram, now);
        if peer.transport.skips() != skips {
            log::info!(
                "Match {:#x}: player {index}'s game took a datagram of ours from a lap before as \
                 its newest; our seq skipped ahead to it ({} times so far)",
                self.id,
                peer.transport.skips()
            );
        }
        self.check_in_flight(index, now);
        for event in events {
            match event {
                Event::Message { id, payload } => self.message(index, id, &payload, now),
                Event::Disconnected => self.leave(index, now),
            }
        }
    }

    fn message(&mut self, index: usize, id: u8, payload: &[u8], now: Instant) {
        if self.accept_prepared(index, id, payload) {
            return;
        }
        let message = match parse_client(id, payload) {
            Ok(message) => message,
            Err(error) => {
                log::debug!(
                    "Match {:#x}: player {index} sent what is no message ({error}); ignored",
                    self.id
                );
                return;
            }
        };
        let bot = self.is_bot(index);
        let Some(peer) = self.peers[index].as_mut() else {
            return;
        };
        // It left or timed out: whatever it still sends is of no match.
        if peer.stage == Stage::Gone {
            return;
        }
        // A test bot is no witness: what it says of the world is not heard.
        if bot
            && matches!(
                message,
                ClientMessage::Hash { .. } | ClientMessage::Chunk(_)
            )
        {
            return;
        }
        match message {
            // A resent copy: the client is placed already.
            ClientMessage::Connect { .. } => {}
            ClientMessage::PortProbe { .. } => {
                if peer.stage == Stage::Connecting
                    && let Some(info) = self.formed.info.message(index as u8)
                {
                    send(peer, &info);
                    peer.installed_roster = self.formed.places.len();
                    peer.stage = Stage::Loading;
                }
            }
            ClientMessage::Ready => {
                if peer.stage == Stage::Loading {
                    peer.resources_loaded = true;
                    peer.loading_since = None;
                    // The client set its own generation to 0xFF with it.
                    peer.generation = Generation::AT_READY;
                    log::info!("Match {:#x}: player {index} has loaded", self.id);
                    if self.loaded_behind(index, now) {
                        return;
                    }
                    if self.returns(index) {
                        self.enter_early(index, now);
                    }
                    self.start_sync(index, true, now);
                }
            }
            ClientMessage::TimeReply(reply) => self.time_reply(index, reply, now),
            ClientMessage::Input(input) => self.input(index, input, now),
            ClientMessage::ProfileInput { tick, value } => {
                self.profile_input(index, tick, value, now);
            }
            ClientMessage::Hash { tick, crc } => self.hash(index, tick, crc, now),
            ClientMessage::Leave => {
                // By its own will, and the match is not over: its place is
                // given up (`take_quits`).
                if self.finished_at.is_none()
                    && let Some(place) = self.formed.places.get(index)
                {
                    self.quits.push((peer.session_id, place.account_id));
                }
                self.leave(index, now);
            }
            ClientMessage::Chunk(chunk) => {
                peer.patched = true;
                if let Pushed::Complete(upload) = peer.reassembler.push(&chunk) {
                    self.uploaded(index, upload, now);
                }
            }
            ClientMessage::KeepAlive => {}
        }
    }

    /// The client's 0xDB: the answer to the 0xE5 of its sync, or to a ping.
    fn time_reply(&mut self, index: usize, reply: TimeReply, now: Instant) {
        let clock = self.clock(now);
        let mono = self.mono(now);
        let id = self.id;
        let Some(peer) = self.peers[index].as_mut() else {
            return;
        };
        // To 0xE5 the client answers the echo as it was.
        if !matches!(peer.stage, Stage::Live)
            && let Some(sent) = peer
                .set_times
                .iter()
                .rev()
                .find(|sent| sent.echo == reply.stamp)
                .copied()
        {
            peer.time_confirmed = true;
            peer.time_answered = true;
            peer.clock_set = true;
            peer.clock_given = Some((sent.start, sent.at));
            if sent.ends_correction {
                // Its game took the 0xE7 of offset 0 ahead of that 0xE5.
                peer.correction_ends = None;
            }
            peer.round_trip = now.saturating_duration_since(sent.at).as_millis() as u32;
            log::debug!(
                "Match {id:#x}: player {index} took its time (generation {}), round trip {} ms",
                peer.generation.0,
                peer.round_trip
            );
            sample_lag(peer, reply.client_clock(clock), clock, now);
            return;
        }
        let Some(ping) = peer.ping else {
            return;
        };
        // `b` as it went, and `a` plus how long the client held it.
        if reply.first != ping.b || reply.stamp.wrapping_sub(ping.a) > PING_ANSWER_MS {
            return;
        }
        let round_trip = reply.round_trip(mono);
        if round_trip > PING_ANSWER_MS {
            return;
        }
        peer.ping = None;
        peer.round_trip = u32::from(round_trip);
        if peer.stage == Stage::Live {
            sample_lag(peer, reply.client_clock(clock), clock, now);
            self.correct_clock(index, now);
        }
    }

    fn hash(&mut self, index: usize, tick: u8, crc: u32, now: Instant) {
        // The client's debug hotkey sends this instead of a hash. Only the
        // clients in the simulation are heard: one that left or is being
        // started anew reports a world the others do not have.
        let live = self.peers[index]
            .as_ref()
            .is_some_and(|peer| peer.stage == Stage::Live);
        let clock = self.clock(now);
        if crc != FORCED_MISMATCH
            && let Some(peer) = self.peers[index].as_mut()
            && let Stage::Syncing { time: from, .. } = peer.stage
        {
            // Its world on the way: a report of a tick past its snapshot
            // may show that it has it (`load_proven`).
            if let Some(time) = tick_time(tick, clock)
                && time > from
                && peer.load_reports.len() < LOAD_REPORTS_KEPT
            {
                peer.load_reports.push((time, crc, now));
            }
            return;
        }
        if crc == FORCED_MISMATCH || !live {
            return;
        }
        let Some(time) = tick_time(tick, clock) else {
            return;
        };
        if let Some(peer) = self.peers[index].as_mut() {
            // Its game is a tick or two further than at its last report:
            // it simulates.
            if peer
                .reported_tick
                .is_some_and(|last| time > last && time - last <= SIMULATION_STEP_MS)
            {
                peer.simulating_at = Some(now);
            }
            peer.reported_tick = Some(time);
        }
        // Only ticks still to be checked, and none from the future: they
        // would pile up, or vouch for a forgery ahead of time.
        if time >= self.hashes_checked_until && time <= clock + HASH_MAX_AHEAD_MS {
            if self.waiting_entry(index) {
                self.catchup_hash(index, time, crc);
                return;
            }
            self.hashes.entry(time).or_default().insert(index, crc);
            // A tick two clients sent a hash for ends a run of ticks nobody
            // shared (`check_hash_phases`). Uploads do not count there: one
            // lands on the other client's ticks only by chance.
            let shared = self.peers.iter().enumerate().any(|(other, peer)| {
                other != index
                    && peer.as_ref().is_some_and(|peer| {
                        peer.stage == Stage::Live && peer.hash_ticks.contains(&time)
                    })
            });
            if shared {
                self.hash_unshared_since = None;
            }
            if let Some(peer) = self.peers[index].as_mut() {
                peer.last_hash_at = Some(now);
                peer.hash_ticks.push_back(time);
                if peer.hash_ticks.len() > HASH_TICKS_KEPT {
                    peer.hash_ticks.pop_front();
                }
            }
        }
    }

    /// Notice clients on opposite hash phases. A stock client reports every
    /// second tick, on a phase of its own (match.md section 14): two of
    /// them may never name the same tick. Then worlds are compared only
    /// at the tick of an upload that happens to be one the other client
    /// reports (about one upload in two): a client out of step is found
    /// late, and the other uploads have one voucher. Nothing is trusted
    /// on one report because of this: it is only said.
    fn check_hash_phases(&mut self, now: Instant) {
        let reporting = self
            .peers
            .iter()
            .flatten()
            .filter(|peer| {
                peer.stage == Stage::Live
                    && peer
                        .last_hash_at
                        .is_some_and(|at| now.saturating_duration_since(at) < HASH_REPORTING)
            })
            .count();
        let since = if reporting >= 2 && self.finished_at.is_none() {
            *self.hash_unshared_since.get_or_insert(now)
        } else {
            self.hash_unshared_since = None;
            now
        };
        let lasted = now.saturating_duration_since(since);
        if lasted < HASH_STARVED_AFTER {
            if self.hash_starved {
                log::info!(
                    "Match {:#x}: the clients' hash reports name the same ticks again",
                    self.id
                );
            }
            self.hash_starved = false;
            self.hash_starved_logged = None;
            return;
        }
        self.hash_starved = true;
        if self
            .hash_starved_logged
            .is_some_and(|at| now.saturating_duration_since(at) < HASH_STARVED_LOG)
        {
            return;
        }
        self.hash_starved_logged = Some(now);
        self.hash_starved_warnings += 1;
        log::warn!(
            "Match {:#x}: {reporting} clients report hashes, but no tick got two reports for {} s: \
             they are on opposite hash phases (each reports every second tick). Until that \
             ends, two clients are asked for the world of one tick every {} s and those are \
             compared: a client out of step is found later than by hashes. The clients need \
             the 50 ms hash interval: the fully patched exe, or -hash_check_interval_in_ms=50 \
             on the command line",
            self.id,
            lasted.as_secs(),
            self.settings.timing.refresh.min(STARVED_REFRESH).as_secs()
        );
    }

    /// Whether a client other than `index` is simulating. (A test bot
    /// simulates nothing.)
    fn others_live(&self, index: usize) -> bool {
        self.peers.iter().enumerate().any(|(other, peer)| {
            other != index
                && !self.is_bot(other)
                && !self.waiting_entry(other)
                && peer.as_ref().is_some_and(|peer| peer.stage == Stage::Live)
        })
    }

    /// The clients in the others' worlds: simulating, or being started
    /// anew from a snapshot (a resync). Each is a witness of the world
    /// (test bots are none, and are not counted).
    fn in_match(&self) -> usize {
        self.peers
            .iter()
            .enumerate()
            .filter(|(index, _)| !self.is_bot(*index) && !self.waiting_entry(*index))
            .filter_map(|(_, peer)| peer.as_ref())
            .filter(|peer| {
                matches!(
                    peer.stage,
                    Stage::Live
                        | Stage::WaitingSnapshot {
                            entering: false,
                            ..
                        }
                        | Stage::Timing {
                            entering: false,
                            ..
                        }
                        | Stage::Syncing {
                            entering: false,
                            ..
                        }
                )
            })
            .count()
    }

    /// The clients simulating now: the ones whose hash reports count.
    /// (Test bots simulate nothing, and are not counted.)
    fn live(&self) -> usize {
        self.peers
            .iter()
            .enumerate()
            .filter(|(index, peer)| {
                !self.is_bot(*index)
                    && !self.waiting_entry(*index)
                    && peer.as_ref().is_some_and(|peer| peer.stage == Stage::Live)
            })
            .count()
    }

    /// The snapshot the client at `index` can start from now: the newest
    /// after its own last roster change, and with others simulating, fresh
    /// enough (`max_age` ms behind the match clock) and of a final tick.
    /// One entering starts its clock past every input of its own the
    /// others have; one resynced while it plays keeps its clock and its
    /// inputs (its own inputs after the snapshot are in its lists and in
    /// the others' worlds alike), but starts from no snapshot before its
    /// `resync_floor`.
    fn usable_snapshot(
        &self,
        index: usize,
        clock: u32,
        max_age: u32,
        entering: bool,
    ) -> Option<Snapshot> {
        if !self.join_ready(index) {
            return None;
        }
        let floor = if entering {
            0
        } else {
            self.peers[index]
                .as_ref()
                .map_or(0, |peer| peer.resync_floor)
        };
        let own_last = self
            .events
            .iter()
            .filter(|(_, who, _)| *who == index)
            .map(|(at, _, _)| *at)
            .max();
        let alone = !self.others_live(index);
        let relayed_last = self.own_relayed_last(index, clock);
        // The initial world's time: the clock started on it.
        let first_time = self.clock_start.map(|(_, at)| at);
        // An upload not yet checked, only when another game reported its
        // hash too: no majority contradicting it is not enough (three games
        // at three CRCs contradict nobody; a back-in started from the wrong
        // one of them, 2026-10-14). With one game simulating its own word
        // is all there is.
        let recent = self.recent.as_ref().filter(|recent| {
            self.admissions[index]
                .as_ref()
                .is_none_or(Admission::entered)
                && (self.live() <= 1
                    || self.vouchers(recent.time, snapshot::crc(&recent.body)) >= 2)
        });
        [self.snapshot.as_ref(), recent]
            .into_iter()
            .flatten()
            .filter(|snapshot| self.after_preparation(index, snapshot.time))
            .filter(|snapshot| own_last.is_none_or(|at| snapshot.time > at))
            .filter(|snapshot| snapshot.time >= floor)
            .filter(|snapshot| alone || clock.saturating_sub(snapshot.time) <= max_age)
            // Its tick is final: no input for it or before it can be
            // relayed any more. (The initial world has none before it.)
            .filter(|snapshot| {
                alone || Some(snapshot.time) == first_time || snapshot.time + HORIZON_MS <= clock
            })
            .filter(|snapshot| {
                alone
                    || !entering
                    || relayed_last.is_none_or(|last| snapshot.time + SYNC_LEAD_MS > last)
            })
            .max_by_key(|snapshot| snapshot.time)
            .cloned()
    }

    /// The times of the inputs of `index` relayed to the others, after
    /// `after`.
    fn own_relayed(&self, index: usize, after: u32) -> impl Iterator<Item = u32> + '_ {
        self.inputs
            .iter()
            .rev()
            .take_while(move |(at, _, _)| *at + 4 * HORIZON_MS > after)
            .filter(move |(at, from, what)| {
                *from == index && *at > after && matches!(what, Relayed::Input(_))
            })
            .map(|(at, _, _)| *at)
    }

    /// The most inputs any one player has for ticks after `after`, up to
    /// `upto`: what a game loading a world of `after` must still hold in
    /// that player's ring ([`INPUT_RING`]).
    fn ring_fill(&self, after: u32, upto: u32) -> usize {
        let mut counts: HashMap<usize, usize> = HashMap::new();
        for (at, from, what) in self
            .inputs
            .iter()
            .rev()
            .take_while(|(at, _, _)| *at + 4 * HORIZON_MS > after)
        {
            if *at > after && *at <= upto && matches!(what, Relayed::Input(_)) {
                *counts.entry(*from).or_default() += 1;
            }
        }
        counts.into_values().max().unwrap_or(0)
    }

    fn own_relayed_last(&self, index: usize, clock: u32) -> Option<u32> {
        self.own_relayed(index, clock.saturating_sub(FRESH_SNAPSHOT_MS))
            .max()
    }

    /// How old a snapshot a client may start from, having waited `waited`.
    /// A client resynced while it plays waits for a fresh one however long
    /// (its world goes only while the snapshot is
    /// [`RESYNC_WORLD_MAX_AGE_MS`] old at most), or gives up as anyone
    /// after [`SNAPSHOT_GIVE_UP`].
    fn snapshot_max_age(waited: Duration, entering: bool) -> u32 {
        if entering && waited >= RESYNC_FRESH_WAIT {
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

    /// Whether a game whose first world is `snapshot` has other statistics
    /// than the snapshot's once it has loaded it: a Team Deathmatch in
    /// play whose tickets are no longer the ones of the start
    /// ([`snapshot::Decoded::first_load_loses_the_standing`], a defect of
    /// the 0.26g client; match.md section 21).
    fn first_load_loses_the_standing(&self, snapshot: &Snapshot) -> bool {
        if self.game_mode() != mode::TEAM_DEATHMATCH {
            return false;
        }
        let tickets = u16::try_from(self.formed.map.tickets_count).unwrap_or(u16::MAX);
        snapshot::decode(
            snapshot.time,
            &snapshot.body,
            self.snapshot_players(),
            self.game_mode(),
            self.batteries(),
        )
        .is_some_and(|decoded| decoded.first_load_loses_the_standing(self.game_mode(), tickets))
    }

    /// A player back in a Team Deathmatch has loaded its first world, and
    /// its game has the tickets of the start in its statistics for it
    /// (`first_load_loses_the_standing`): out of step from its first
    /// tick. A world loaded by a game that is in play already is taken as
    /// it is, so the client is started once more, at once, as a resync is.
    /// Left alone it was resynced all the same, but only once its reports
    /// had been wrong for a while and its clock, 300 ms behind after the
    /// first start, had been brought up over 6 s: 7 s of a wrong world,
    /// then a second 6 s of inputs too late to relay (live, 2026-10-12).
    ///
    /// Only with somebody to be in step with and to give a world: a
    /// client alone has the only world there is, and its statistics are
    /// put right by the rule's next event (0x3161CF).
    fn reload(&mut self, index: usize, now: Instant) {
        let id = self.id;
        if !self.others_live(index) || self.sources(index).is_empty() {
            log::info!(
                "Match {id:#x}: player {index}'s game loaded a Team Deathmatch in play and has \
                 the tickets of the start in its statistics; nobody else is there to be in step \
                 with or to give a world, so it is left as it is"
            );
            return;
        }
        if let Some(peer) = self.peers[index].as_mut() {
            peer.reloads += 1;
        }
        log::info!(
            "Match {id:#x}: player {index}'s game loaded a Team Deathmatch in play and has the \
             tickets of the start in its statistics; started once more, from a world it takes as \
             it is"
        );
        self.start_sync(index, false, now);
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

    /// A roster change happens: everyone simulating but `except` hears it,
    /// and the server's copy of the timelimit rule follows.
    fn roster_change(
        &mut self,
        except: Option<usize>,
        index: usize,
        entered: bool,
        at: u32,
        now: Instant,
    ) {
        if entered && let Some(admission) = &mut self.admissions[index] {
            admission.enter();
        }
        let tick = tick_byte(at);
        let player = index as u8;
        let message = if entered {
            ServerMessage::PlayerEntered { tick, player }
        } else {
            ServerMessage::PlayerLeft { tick, player }
        };
        for (other, peer) in self.peers.iter_mut().enumerate() {
            if Some(other) == except {
                continue;
            }
            let Some(peer) = peer else {
                continue;
            };
            // A client in a resync had the roster changes up to its replay
            // with it (`send_replay`); before that, this one will be in it.
            // Its world waits for those sent while it is timed. To it a
            // roster change is kept until acked, whatever becomes of its
            // `in_flight` (`send_sync` empties that), and forgotten only
            // when a next generation's replay or world holds it
            // (`clear_gate`).
            let to_resyncing = match peer.stage {
                Stage::Timing {
                    entering: false, ..
                } => peer.replayed.is_some(),
                Stage::Syncing {
                    entering: false, ..
                } => peer.resync_from.is_some(),
                _ => false,
            };
            // A game that enters gets it after its load (`finish_sync`):
            // the client's 0xEA handler reads the match's view, which a
            // game has only once its world is loaded (0x27C776, [esi+4]),
            // and crashed on a leave that came before (live, 2026-10-14).
            // An entry to a game that enters, its world on the way: now,
            // not after its load (`entries_since`).
            let to_starting = entered
                && other != index
                && peer.time_answered
                && matches!(peer.stage, Stage::Syncing { entering: true, time, .. } if at > time);
            if to_starting {
                if let Some(serial) = send_waiting(peer, &message) {
                    peer.early_entries.push((at, index, serial, true));
                }
            } else if to_resyncing {
                if let Some(serial) = send_kept(peer, &message) {
                    track(peer, serial, Flight::Roster(message.clone()), at);
                    gate(peer, serial, now, at, GateKind::Roster);
                }
            } else if peer.stage == Stage::Live
                && let Some(serial) = send(peer, &message)
            {
                track(peer, serial, Flight::Roster(message.clone()), at);
            }
        }
        self.events.push((at, index, entered));
        let bot = self.is_bot(index);
        if let Some(recording) = self.recording.as_mut() {
            if entered && !bot {
                recording.entered(index);
            }
            let note = serde_json::json!({
                "note": if entered { "entered" } else { "left" },
                "place": index,
                "match_ms": at,
            });
            recording.note(&note, Instant::now());
        }
        let step = if entered {
            self.first_entered.get_or_insert(at);
            self.rule.on_player_entered(player, at)
        } else {
            self.rule.on_player_left(player, at)
        };
        self.log_transition(step, at);
    }

    fn log_transition(&self, step: Option<Transition>, at: u32) {
        match step {
            Some(Transition::CountdownStarted) => log::info!(
                "Match {:#x}: the countdown starts at {at} ms ({} of {} players in)",
                self.id,
                self.rule.mask.count_ones(),
                self.players()
            ),
            Some(Transition::MatchStarted) => log::info!(
                "Match {:#x}: play begins at {} ms",
                self.id,
                self.rule.start.unwrap_or(at)
            ),
            None => {}
        }
    }

    /// A player back in a match it left enters for the others now, before
    /// it is started: a snapshot after this entry has its player as the
    /// others have it (0.21d `enter_early`).
    fn enter_early(&mut self, index: usize, now: Instant) {
        let left = self
            .events
            .iter()
            .filter(|(_, who, _)| *who == index)
            .map(|(at, _, _)| *at)
            .max()
            .unwrap_or(0);
        let at = grid_up(self.clock(now).max(left + 1) + ROSTER_LEAD_MS);
        self.roster_change(Some(index), index, true, at, now);
        if let Some(peer) = self.peers[index].as_mut() {
            peer.entered_early = Some(at);
        }
        log::info!(
            "Match {:#x}: player {index} entered at {at} ms, back in the match: for the others \
             first; it starts from a snapshot after that, which has its player as they do",
            self.id
        );
    }

    /// Whether the client at `index` is a test bot with no real client
    /// simulating yet: it enters a world somebody holds, not before.
    fn bot_waits(&self, index: usize) -> bool {
        self.is_bot(index) && self.live() == 0
    }

    /// Start the client at `index` from a snapshot: at once if there is a
    /// fresh one, else once a patched client has uploaded one.
    fn start_sync(&mut self, index: usize, entering: bool, now: Instant) {
        self.reset_catchup(index, false);
        let clock = self.clock(now);
        let max_age = Self::snapshot_max_age(Duration::ZERO, entering);
        if let Some(peer) = self.peers[index].as_mut() {
            peer.time_confirmed = false;
            peer.set_times.clear();
        }
        if self.bot_waits(index) || !self.join_ready(index) {
            if let Some(peer) = self.peers[index].as_mut() {
                peer.stage = Stage::WaitingSnapshot {
                    since: now,
                    entering,
                };
            }
            log::info!(
                "Match {:#x}: player {index} waits for admission and a live snapshot source",
                self.id
            );
            return;
        }
        // What it reported for ticks not judged yet is of the world it
        // leaves: left in, those ticks would be judged again once it is no
        // longer known to be the one out of step, and with no majority
        // somebody else blamed for them.
        for reports in self
            .hashes
            .range_mut(self.hashes_checked_until..)
            .map(|(_, reports)| reports)
        {
            reports.remove(&index);
        }
        if let Some(snapshot) = self.usable_snapshot(index, clock, max_age, entering) {
            self.begin_timing(index, &snapshot, entering, now);
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
        if early {
            // Asked for once the uploads are past its entry (`poll`).
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
    /// are asked: the ones with the best links, [`SNAPSHOT_SOURCES`] of
    /// them (every one at the end of the match).
    fn sources(&self, except: usize) -> Vec<usize> {
        let mut sources: Vec<(f32, u32, usize)> = self
            .peers
            .iter()
            .enumerate()
            .filter_map(|(index, peer)| {
                let peer = peer.as_ref()?;
                (index != except
                    && !self.waiting_entry(index)
                    && peer.patched
                    && peer.stage == Stage::Live)
                    .then_some((peer.late_share, peer.round_trip, index))
            })
            .collect();
        sources.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
        let asked = if self.ending.is_some() {
            usize::MAX
        } else if self.hash_starved {
            // No tick has two hash reports: two worlds of one tick are
            // compared instead (`aim`).
            SNAPSHOT_SOURCES
        } else if self.live() < LARGE_MATCH_PLAYERS {
            1
        } else {
            SNAPSHOT_SOURCES
        };
        sources.truncate(asked);
        sources.into_iter().map(|(_, _, index)| index).collect()
    }

    /// Ask the best-linked clients for an upload (0xF6), unless one was
    /// asked for less than `gap` ago.
    fn request_upload(&mut self, except: usize, now: Instant, gap: Duration) {
        if self
            .requested_at
            .is_some_and(|at| now.saturating_duration_since(at) < gap)
        {
            return;
        }
        let sources = self.sources(except);
        if sources.is_empty() {
            return;
        }
        let clock = self.clock(now);
        let plan = self.aim(&sources, clock);
        log::debug!(
            "Match {:#x}: players {sources:?} are asked for a snapshot (at {plan:?}, clock {clock})",
            self.id
        );
        for (index, at) in plan {
            if let Some(peer) = self.peers[index].as_mut() {
                peer.ask_at = Some(at);
            }
        }
        self.requested_at = Some(now);
        self.send_requests(now);
    }

    /// When each of `sources` is asked for its world (match clock): now,
    /// unless the clients are on opposite hash phases (`hash_starved`).
    ///
    /// A client uploads its event-horizon world of the moment it handles
    /// the request (client_patch.md section 3.4), a tick of its own clock.
    /// Each upload tells where that tick lies against the match clock at
    /// the request, to 50 ms (`upload_offset`). While starved, a world is
    /// trusted only if another client names its tick too, so the requests
    /// are timed for one tick `T`:
    /// - two sources: both are asked for `T`, each when the middle of its
    ///   known bounds is in the middle of `T`. Their two uploads are two reports
    ///   for `T`: equal CRCs are a trusted world, unequal ones a
    ///   disagreement that is judged as any other.
    /// - one source (the other client is not patched): `T` is a tick the
    ///   other client sends its hash for.
    ///
    /// With no bounds yet everyone is asked at once: that round gives
    /// them. Bounds are 50 ms wide at most, so a request aimed by their
    /// middle misses its tick by one at most, and one that does narrows
    /// them to a few ms (`observe_upload`): while a client's clock and
    /// link hold, it misses once at most. A client whose link jitters by
    /// more than about 20 ms gives bounds that do not agree; then the
    /// newest count, and it is hit or miss as it was without aiming.
    fn aim(&self, sources: &[usize], clock: u32) -> Vec<(usize, u32)> {
        let now: Vec<(usize, u32)> = sources.iter().map(|index| (*index, clock)).collect();
        if !self.hash_starved || self.ending.is_some() {
            return now;
        }
        let middles: Option<Vec<i64>> = sources
            .iter()
            .map(|index| {
                let (low, high) = self.peers[*index].as_ref()?.upload_offset?;
                Some((low + high) / 2)
            })
            .collect();
        let Some(middles) = middles else {
            return now;
        };
        // The ticks another client in the simulation sends hashes for,
        // when there is one source only.
        let phase = match sources {
            [source] => {
                let other = self.peers.iter().enumerate().find_map(|(other, peer)| {
                    let peer = peer.as_ref()?;
                    if other == *source || peer.stage != Stage::Live {
                        return None;
                    }
                    let mut ticks = peer.hash_ticks.iter().rev();
                    let (last, before) = (*ticks.next()?, *ticks.next()?);
                    Some((last.checked_sub(before)?, last))
                });
                match other {
                    Some(phase) => Some(phase),
                    None => return now,
                }
            }
            _ => None,
        };
        match aim_at(clock, &middles, phase) {
            Some(times) => sources.iter().copied().zip(times).collect(),
            None => now,
        }
    }

    /// 0xF6 to each client whose request's moment has come.
    fn send_requests(&mut self, now: Instant) {
        let clock = self.clock(now);
        for peer in self.peers.iter_mut().flatten() {
            if peer.ask_at.is_none_or(|at| at > clock) {
                continue;
            }
            peer.ask_at = None;
            if peer.stage != Stage::Live {
                continue;
            }
            send(peer, &ServerMessage::UploadRequest);
            peer.asked = Some(now);
            peer.asked_clock = Some(clock);
            peer.unanswered_since.get_or_insert(now);
        }
    }

    /// The world of the player at `index`, found out of step: asked for to
    /// look at (`wrong_world_uploaded`), no vote and never a world anyone
    /// starts from. A patched client uploads its event-horizon world of the
    /// moment it handles the request (client_patch.md section 3.4); asked
    /// for ahead of the 0xE5 of its resync, that is a world of the game it
    /// played. Live, 2026-10-15: only agreed worlds were kept, none of an
    /// out-of-step game, and no wrong world could be compared byte for byte.
    /// [`WRONG_WORLDS_PER_PLAYER`] a player and [`WRONG_WORLDS_PER_MATCH`]
    /// a match at most.
    fn ask_wrong_world(&mut self, index: usize, now: Instant) {
        if self.wrong_worlds_asked >= WRONG_WORLDS_PER_MATCH {
            return;
        }
        let clock = self.clock(now);
        let Some(peer) = self.peers[index].as_mut() else {
            return;
        };
        if !peer.patched
            || peer.stage != Stage::Live
            || peer.wrong_worlds >= WRONG_WORLDS_PER_PLAYER
            || peer.is_asked(now)
        {
            return;
        }
        peer.wrong_worlds += 1;
        peer.wrong_world_asked = Some(clock);
        send(peer, &ServerMessage::UploadRequest);
        peer.asked = Some(now);
        peer.asked_clock = None;
        self.wrong_worlds_asked += 1;
    }

    /// The world a player out of step was asked for came: saved as
    /// "wrong" with what it and the majority say of its tick.
    fn wrong_world_uploaded(&mut self, index: usize, asked_at: u32, snapshot: &Snapshot) {
        let crc = snapshot::crc(&snapshot.body);
        let majority = self.majority(snapshot.time);
        log::warn!(
            "Match {:#x}: player {index}, out of step, gave its world at {} ms ({} bytes, CRC \
             {crc:08x}; the majority's for that tick {}; asked at {asked_at} ms); saved as \
             \"wrong\"",
            self.id,
            snapshot.time,
            snapshot.body.len(),
            majority.map_or("unknown".to_owned(), |majority| format!("{majority:08x}"))
        );
        self.dump(snapshot, index, "wrong");
    }

    /// Where a client that starts from the snapshot at `time` has its clock
    /// put (0xE5).
    ///
    /// One that enters while others simulate: `SYNC_LEAD_MS` past the
    /// snapshot, so that the inputs since it are still open when they
    /// arrive; 0xE7 then brings it to the match clock.
    ///
    /// One resynced while it plays (match.md section 30): never behind its
    /// own clock as the 0xE5 arrives, nor behind its newest own input seen,
    /// by [`RESYNC_CLOCK_MARGIN_MS`] each. A game whose clock was put back
    /// made its inputs for ticks it had sent once more, with other contents
    /// (live, 2026-10-14: one player 22 such ticks in a match), which the
    /// others never get (a time seen is a copy here): the guard 0xE5
    /// stores (no input at or before its newest own one) does not cover
    /// that. Its own clock is the match clock and the way of the 0xE5
    /// (half its round trip, at most [`RESYNC_ONE_WAY_MAX_MS`]), less how
    /// far it is behind by the pings (`estimated_lag`; games run about
    /// their way ahead); and once it has answered a 0xE5 of this sync, the
    /// clock that 0xE5 gave it and the time since, which is exact. The
    /// margin is for inputs on their way, newer than the newest seen. By
    /// the world alone (`SRP_RESYNC_WORLD_ONLY`) the clock stays where it
    /// is.
    ///
    /// Its clock once it took a 0xE5 of this sync: the latest clock any
    /// copy of the generation gives (`given_clock`), not the last copy
    /// sent: copies whose clocks disagree may arrive in any order.
    fn sync_start(&self, index: usize, time: u32, entering: bool, clock: u32, now: Instant) -> u32 {
        let others = self.others_live(index);
        if !entering {
            if self.settings.resync_world_only && others {
                return clock;
            }
            let Some(peer) = self.peers[index].as_ref() else {
                return clock;
            };
            let newest = peer
                .inputs_seen
                .iter()
                .copied()
                .chain(peer.profiles_seen.iter().map(|(at, _)| *at))
                .max()
                .map_or(0, |at| at + RESYNC_CLOCK_MARGIN_MS);
            // The game follows the last copy of the generation it took:
            // the latest any copy gives (`given_clock`).
            let given = peer
                .time_confirmed
                .then(|| given_clock(&peer.set_times, now))
                .flatten()
                .or(peer.clock_given.map(|(start, at)| {
                    start + now.saturating_duration_since(at).as_millis() as u32
                }));
            let lag = estimated_lag(peer, now)
                .unwrap_or(0)
                .clamp(-MAX_SANE_LAG_MS, MAX_SANE_LAG_MS);
            let measured = (i64::from(clock + one_way(peer)) - lag).max(0) as u32;
            let own = match given {
                // Known: no margin but the pings' own error, ahead only.
                Some(given) => given.max(measured),
                None => measured + RESYNC_CLOCK_MARGIN_MS,
            };
            return own.max(newest);
        }
        let back_in = self.peers[index]
            .as_ref()
            .filter(|peer| peer.entered_early.is_some());
        if let Some(peer) = back_in
            && others
        {
            // Back in the match: the others have its player since its
            // entry, and take its inputs as a live one's. Its game goes at
            // the match clock as the 0xE5 arrives, as a resynced one's
            // does: put behind it by `SYNC_LEAD_MS` its inputs of the first
            // second came 250-400 ms past their ticks, too late for the
            // others, and their drops reached its game past its history
            // (live, 2026-10-15, 9 drops and out of step 2 s after its
            // load). Never behind the lead.
            (clock + one_way(peer)).max(time + SYNC_LEAD_MS)
        } else if others {
            (time + SYNC_LEAD_MS)
                .min(clock)
                .max(clock.saturating_sub(MAX_CLOCK_CORRECTION_MS))
        } else {
            clock
        }
    }

    /// 0xE5 of the client's sync generation, with its clock for the
    /// snapshot at `time`: that clock.
    fn send_set_time(&mut self, index: usize, time: u32, entering: bool, now: Instant) -> u32 {
        let clock = self.clock(now);
        let echo = self.mono(now) as u16;
        let start = self.sync_start(index, time, entering, clock, now);
        let id = self.id;
        let Some(peer) = self.peers[index].as_mut() else {
            return start;
        };
        if peer.set_times.len() == SET_TIMES_KEPT {
            peer.set_times.pop_front();
        }
        let set_time = ServerMessage::SetTime {
            generation: peer.generation.0,
            echo,
            match_time: start,
        };
        // A 0xE5 sets the base of the game's clock and leaves a running
        // 0xE7 correction in (match.md section 29): one that may still run
        // is ended by a 0xE7 of offset 0 ahead of the 0xE5, in its
        // datagram. Never alone after it: the drift until then would go
        // into the base.
        let ends_correction = peer.clock_offset_pending.is_some()
            || peer.correction_ends.is_some_and(|end| now < end);
        if ends_correction {
            let stop = ServerMessage::ClockOffset(0);
            match peer.transport.send_joined(
                (stop.id(), &stop.payload()),
                (set_time.id(), &set_time.payload()),
            ) {
                Ok(_) | Err(SendError::NotConnected) => {}
                Err(error) => {
                    log::warn!("Match {id:#x}: 0xE7 and 0xE5 to player {index} not sent: {error:?}")
                }
            }
        } else {
            send(peer, &set_time);
        }
        peer.set_times.push_back(SetTimeSent {
            echo,
            at: now,
            start,
            ends_correction,
        });
        start
    }

    /// A snapshot is there for the client at `index`: its next sync
    /// generation begins with 0xE5 alone, until the client answers it.
    /// 0xE5 is sent once and may be lost, and a world loaded without it
    /// would run on the clock the client had.
    ///
    /// A client resynced while it plays goes on from the 0xE5 with a
    /// cleared history on its new clock, until its world is loaded
    /// (`Peer::resync_from`, match.md section 30). Its inputs are relayed
    /// all the while as ever; the answers to them wait for the load.
    fn begin_timing(&mut self, index: usize, snapshot: &Snapshot, entering: bool, now: Instant) {
        // A clock correction not taken yet would move the clock this sync
        // gives: it goes no more.
        if let Some(peer) = self.peers[index].as_mut()
            && let Some((serial, _, _, takes)) = peer.clock_offset_pending.take()
        {
            peer.transport.forget(serial, now);
            // A copy on its way may still start it: the 0xE5s of this
            // sync end it (`send_set_time`).
            let runs = now + takes + Duration::from_millis(u64::from(peer.round_trip));
            peer.correction_ends = Some(peer.correction_ends.map_or(runs, |end| end.max(runs)));
        }
        if self.settings.resync_world_only && !entering && self.others_live(index) {
            // `SRP_RESYNC_WORLD_ONLY`: no 0xE5, the world of the next
            // generation at once (the client takes a 0xE8 that is newer
            // than its own generation).
            if let Some(peer) = self.peers[index].as_mut() {
                peer.generation = peer.generation.next();
                peer.set_times.clear();
                peer.time_confirmed = true;
            }
            self.send_sync(index, snapshot.clone(), entering, now);
            return;
        }
        let Some(peer) = self.peers[index].as_mut() else {
            return;
        };
        // One past the last: the client takes a 0xE5 only when it is newer
        // than its own, and a late 0xE8 of an older sync is skipped. Until
        // it has taken the 0xE8 it takes every 0xE5 of this generation.
        peer.generation = peer.generation.next();
        peer.set_times.clear();
        peer.time_confirmed = false;
        peer.time_answered = false;
        peer.early_entries.clear();
        peer.replayed = None;
        clear_gate(peer, None, now);
        peer.world_waits = 0;
        peer.world_wait_since = None;
        if !entering && peer.resync_from.is_none() {
            peer.resync_from = Some(now);
            // An answer sent to an own input that may not have arrived
            // yet would be refused from the 0xE5 on: it goes again after
            // the load. Taken twice it does nothing the second time (the
            // node is gone, match.md section 16).
            let transport = &peer.transport;
            let unsure: Vec<Answer> = peer
                .sent_answers
                .drain(..)
                .filter(|sent| {
                    sent.serial
                        .is_some_and(|serial| !transport.is_delivered(serial))
                })
                .map(|sent| Answer {
                    serial: None,
                    ..sent
                })
                .collect();
            peer.held_answers.extend(unsure);
        }
        if !entering && peer.resync_from.is_some() {
            peer.resync_generations += 1;
        }
        peer.stage = Stage::Timing {
            time: snapshot.time,
            entering,
            last: now,
            sent: 1,
        };
        self.send_set_time(index, snapshot.time, entering, now);
        if !entering {
            // A fresher world may be there by the time its replay is acked
            // (`resync_timed`).
            self.request_upload(index, now, RESYNC_REQUEST_GAP);
        }
    }

    /// The roster changes after `time` for the client at `index`, which
    /// starts from a snapshot of then. Never its own leaving: a client told
    /// it left closes the match. Its own entry only if it entered early and
    /// the snapshot (given up waiting for) is from before it.
    fn roster_since(&self, index: usize, time: u32) -> Vec<(u32, ServerMessage)> {
        let early = self.peers[index]
            .as_ref()
            .and_then(|peer| peer.entered_early);
        self.events
            .iter()
            .filter(|(at, who, entered)| {
                *at > time && (*who != index || (*entered && early.is_some()))
            })
            .map(|&(at, who, entered)| {
                let (tick, player) = (tick_byte(at), who as u8);
                let message = if entered {
                    ServerMessage::PlayerEntered { tick, player }
                } else {
                    ServerMessage::PlayerLeft { tick, player }
                };
                (at, message)
            })
            .collect()
    }

    /// The other players' entries after `time`, for a game that enters
    /// from a world of then: with its world, not after its load. The
    /// client takes a 0xE9 that comes after its 0xE5 and before its world
    /// is loaded, whatever its tick, and keeps it through the load (0xE8
    /// loads in mode 1, 0x27C107); once the world is loaded it refuses one
    /// whose tick is the event horizon behind its present ("too old",
    /// `enter_match` 0x2D15C2), and every copy after. Sent after the load
    /// (until 2026-10-15), a slow line had them past it: live, the oldest
    /// were refused and the game was out of step 3 s into the match. 0xE9
    /// never reads the match view, so it is safe before the load; 0xEA
    /// does, and stays after it (`roster_change`). Never its own entry.
    fn entries_since(&self, index: usize, time: u32) -> Vec<(u32, usize, ServerMessage)> {
        self.events
            .iter()
            .filter(|(at, who, entered)| *entered && *at > time && *who != index)
            .map(|&(at, who, _)| {
                let message = ServerMessage::PlayerEntered {
                    tick: tick_byte(at),
                    player: who as u8,
                };
                (at, who, message)
            })
            .collect()
    }

    /// A client resynced while it plays has answered its 0xE5: its history
    /// is cleared, and a relay or roster change is taken into its lists
    /// (match.md section 30). It is sent the other
    /// players' inputs and the roster changes since the snapshot at
    /// `time`, kept until acked, and its world goes once all of them are
    /// (`resync_timed`). Not with the first 0xE5: a relay that came before
    /// it, to a game that still plays, would be moved by that game to a
    /// tick of its own if older than its horizon.
    fn send_replay(&mut self, index: usize, time: u32, now: Instant) {
        let replay: Vec<(u32, ServerMessage)> = self
            .inputs
            .iter()
            .filter(|(at, from, _)| *at > time && *from != index)
            .map(|(at, from, what)| (*at, relay(*from, what)))
            .collect();
        let events = self.roster_since(index, time);
        let id = self.id;
        let Some(peer) = self.peers[index].as_mut() else {
            return;
        };
        peer.replayed = Some((time, now));
        clear_gate(peer, None, now);
        // Kept until acked, and packed: the transport puts as many in a
        // datagram as fit (transport.md section 11.1).
        for (at, message) in &replay {
            if let Some(serial) = send_kept(peer, message) {
                gate(peer, serial, now, *at, GateKind::Replay);
            }
        }
        for (at, message) in &events {
            if let Some(serial) = send_kept(peer, message) {
                track(peer, serial, Flight::Roster(message.clone()), *at);
                gate(peer, serial, now, *at, GateKind::Roster);
            }
        }
        log::debug!(
            "Match {id:#x}: player {index} took its time; sent {} inputs of the others and {} \
             roster changes since the snapshot at {time} ms",
            replay.len(),
            events.len()
        );
    }

    /// The resync of the client at `index`, which took its time, begun at
    /// `since`: its replay (`send_replay`), then, once every part of it and
    /// every relay and roster change sent since is acked, its world. The
    /// world is the newest snapshot from `time` on that is young enough to
    /// be loaded in time (`resync_world_max_age`): the replay sent for
    /// `time` holds whatever a newer one needs, and the inputs before a
    /// newer one's time are the oldest in the game's rings, the first to
    /// go. With none such, the generation and its acked replay are kept,
    /// a newer snapshot is asked for at once, and the world goes when one
    /// comes: a new 0xE5 and replay would take two more round trips, and on
    /// a line of about 250 ms and more a snapshot is never young enough
    /// after them (live, 2026-10-14: 20 s of new generations). After
    /// [`RESYNC_WORLD_WAITS`] such waits, or [`SNAPSHOT_GIVE_UP`], the
    /// newest snapshot there is goes.
    ///
    /// A replay lost, or not acked in [`REPLAY_ACK_WAIT`], has the sync
    /// done again, [`REPLAY_REDONE`] times at most; then the world goes
    /// all the same.
    fn resync_timed(&mut self, index: usize, time: u32, since: Instant, now: Instant) {
        let clock = self.clock(now);
        let id = self.id;
        let Some(peer) = self.peers[index].as_mut() else {
            return;
        };
        let Some((replayed_for, _)) = peer.replayed else {
            self.send_replay(index, time, now);
            return;
        };
        // Looked at every RESYNC_GATE_EVERY at most; each look is one O(1)
        // test per entry not acked yet.
        if peer
            .gate_checked
            .is_some_and(|at| now.saturating_duration_since(at) < RESYNC_GATE_EVERY)
        {
            return;
        }
        peer.gate_checked = Some(now);
        peer.gate_checks += 1;
        prune_gate(peer);
        // Roster changes are kept until acked now, never lost; this is
        // for a transport that drops one all the same.
        let lost = peer
            .sync_pending
            .iter()
            .any(|entry| peer.transport.is_lost(entry.serial));
        let overdue = peer
            .sync_pending
            .iter()
            .map(|entry| entry.sent)
            .min()
            .is_some_and(|at| now.saturating_duration_since(at) >= REPLAY_ACK_WAIT);
        // A bad line is not taken through new generations for ever: the
        // redos of this resync are capped, and so are its generations
        // whatever made them.
        let exhausted = peer.replay_redos >= REPLAY_REDONE
            || peer.resync_generations >= RESYNC_GENERATIONS_MAX;
        if (lost || overdue) && !exhausted {
            let why = if lost {
                "a roster change of its replay was lost".to_owned()
            } else {
                format!(
                    "its replay was not acked in {} s",
                    REPLAY_ACK_WAIT.as_secs()
                )
            };
            peer.replay_redos += 1;
            let why = format!("{why} ({} of {REPLAY_REDONE} times)", peer.replay_redos);
            self.redo_sync(index, false, &why, now);
            return;
        }
        // Past the caps the replay stays kept and the world goes at once,
        // without what is missing up to its time.
        let stuck = lost || overdue;
        let timed_out = now.saturating_duration_since(since) >= SNAPSHOT_GIVE_UP;
        // Three waits of RESYNC_WORLD_WAIT each, the third one too.
        let given_up = timed_out
            || (peer.world_waits >= RESYNC_WORLD_WAITS
                && peer
                    .world_wait_since
                    .is_some_and(|at| now.saturating_duration_since(at) >= RESYNC_WORLD_WAIT));
        let max_age = if given_up {
            TICK_REACH_MS
        } else {
            resync_world_max_age(trip_one_way(peer))
        };
        let mut snapshot = self
            .usable_snapshot(index, clock, max_age, false)
            .filter(|snapshot| snapshot.time >= replayed_for);
        if snapshot.is_none() && timed_out {
            // No snapshot past its floor came in SNAPSHOT_GIVE_UP: the
            // newest there is goes, floor or not, rather than none ever.
            // Its replay is for `replayed_for`, so none older; and the
            // wire's ticks must reach it. A held answer its game can no
            // longer take is then done with as ever (`send_held_answers`).
            snapshot = self.newest_snapshot().filter(|snapshot| {
                snapshot.time >= replayed_for
                    && snapshot.time + TICK_REACH_MS >= clock
                    && self.after_preparation(index, snapshot.time)
            });
            if let Some(snapshot) = &snapshot {
                log::warn!(
                    "Match {id:#x}: player {index} had no snapshot past its floor in {} s; its \
                     world goes from the newest, at {} ms",
                    SNAPSHOT_GIVE_UP.as_secs(),
                    snapshot.time
                );
            }
        }
        let Some(peer) = self.peers[index].as_mut() else {
            return;
        };
        if let Some(snapshot) = snapshot {
            // The replay, every roster change and every relay up to the
            // world's time are in the game's lists first: one older than
            // the world that came after it would be moved by the game
            // (match.md section 5.1). A relay past the world's time is
            // never waited for: on a busy match some always are on their
            // way, and the world would never go. And the wait is capped:
            // RESYNC_GATE_MAX in all, or as long as the snapshot stays
            // young enough to be loaded in time.
            let missing = peer
                .sync_pending
                .iter()
                .chain(peer.sync_relays.iter())
                .filter(|entry| entry.holds(snapshot.time))
                .count();
            if missing == 0 {
                self.send_sync(index, snapshot, false, now);
                return;
            }
            let gate_since = *peer.gate_since.get_or_insert(now);
            let waited = now.saturating_duration_since(gate_since);
            let edge = clock + 2 * RESYNC_GATE_EVERY.as_millis() as u32
                >= snapshot.time + max_age;
            if given_up || stuck || waited >= RESYNC_GATE_MAX || edge {
                log::warn!(
                    "Match {id:#x}: player {index}'s world of {} ms goes without {missing} \
                     messages up to its time not acked yet, after {} ms of waiting for them \
                     ({}; generation {})",
                    snapshot.time,
                    waited.as_millis(),
                    if given_up {
                        "given up"
                    } else if stuck {
                        "redos used up"
                    } else if edge {
                        "the snapshot would be too old"
                    } else {
                        "the gate's limit"
                    },
                    peer.generation.0
                );
                self.send_sync(index, snapshot, false, now);
            }
            return;
        }
        let newest = self.newest_snapshot().map(|snapshot| snapshot.time);
        let Some(peer) = self.peers[index].as_mut() else {
            return;
        };
        let due = peer
            .world_wait_since
            .is_none_or(|at| now.saturating_duration_since(at) >= RESYNC_WORLD_WAIT);
        if due {
            peer.world_waits += 1;
            peer.world_wait_since = Some(now);
            log::info!(
                "Match {id:#x}: player {index} has its replay since {replayed_for} ms; no snapshot \
                 from then on is {max_age} ms fresh, a newer one is asked for (generation {} \
                 kept; {} of {RESYNC_WORLD_WAITS} waits; the newest at {:?} ms, its floor {} ms, \
                 clock {clock} ms)",
                peer.generation.0,
                peer.world_waits,
                newest,
                peer.resync_floor
            );
        }
        self.request_upload(index, now, RESYNC_REQUEST_GAP);
    }

    /// 0xE5 again, the other players' inputs since the snapshot, and 0xE8,
    /// all of the client's sync generation: the client sets its clock,
    /// takes the inputs and loads the world.
    ///
    /// The inputs go between the two (0.21d's order since 2026-10-12, see
    /// `srp-021d` `upstream_recovery.md`; the 0.26g client code is the
    /// same in what matters, match.md section 18): 0xE5 empties the
    /// client's history and puts its world time back to 50 ms, so an
    /// input relayed then is neither too old (nothing is moved) nor too
    /// new (that test is skipped with no history), and the input lists
    /// outlive the load. So the inputs are in whatever the round trip is,
    /// and the first move after the load plays them all. They are kept
    /// until acked (a relay's own 250 ms would lose them on a slow link).
    /// The client's own inputs are never sent back to it: it has them,
    /// and one that came back late would be moved to another tick by the
    /// client (seen live by another server, upstream_compare.md topic 7).
    ///
    /// A client resynced while it plays had the inputs and the roster
    /// changes since the snapshot before (`send_replay`), all acked: it
    /// gets the 0xE5 once more and the world.
    fn send_sync(&mut self, index: usize, snapshot: Snapshot, entering: bool, now: Instant) {
        let clock = self.clock(now);
        let others = self.others_live(index);
        let resync = !entering && others;
        let world_only = self.settings.resync_world_only && resync;
        let resync_from = self.peers[index].as_ref().and_then(|peer| peer.resync_from);
        // A client in a resync has had its replay (`send_replay`), and the
        // relays since as they came.
        let replay: Vec<ServerMessage> = if resync_from.is_some() {
            Vec::new()
        } else {
            self.inputs
                .iter()
                .filter(|(at, from, _)| *at > snapshot.time && *from != index)
                .map(|(_, from, what)| relay(*from, what))
                .collect()
        };
        // A game that enters: the others' entries since its world, ahead
        // of the world (`entries_since`).
        let entries = if entering && resync_from.is_none() {
            self.entries_since(index, snapshot.time)
        } else {
            Vec::new()
        };
        let start = self.sync_start(index, snapshot.time, entering, clock, now);
        let id = self.id;
        // Only a game's first world: one that is in play takes a world as
        // it is. And only a player back in the match, whose entry the
        // others have already: its second start needs no world newer than
        // the first.
        let reload_due = entering
            && (self.waiting_entry(index)
                || self.peers[index]
                    .as_ref()
                    .is_some_and(|peer| peer.entered_early.is_some()))
            && self.first_load_loses_the_standing(&snapshot);
        let Some(peer) = self.peers[index].as_mut() else {
            return;
        };
        peer.reload_due = reload_due;
        // What its world waited for and did not get acked: up to the
        // world's time it is in the world, and a copy that came after it
        // would be moved by the game; past it, it stays kept until acked.
        let forgotten = clear_gate(peer, Some(snapshot.time), now);
        if forgotten > 0 {
            log::info!(
                "Match {id:#x}: player {index}'s world of {} ms holds {forgotten} messages not \
                 acked yet: no more copies of them go",
                snapshot.time
            );
        }
        peer.missed.clear();
        peer.discarded.clear();
        peer.in_flight.clear();
        // Its reports up to now are of the world it leaves.
        peer.diverged = None;
        peer.judge_from = clock;
        // Its 0xE5 puts it at the match clock as it arrives: no lag but
        // what the newest of its own inputs put it ahead by.
        let arrives = if resync_from.is_some() {
            one_way(peer)
        } else {
            0
        };
        peer.lag_ms = Some(i64::from(clock + arrives) - i64::from(start));
        peer.correction = None;
        peer.correcting_until = None;
        // What it reported before is of the clock it leaves.
        peer.lags.clear();
        peer.upload_offset = None;
        peer.ask_at = None;
        peer.ping = None;
        peer.sync_sent_at = Some(now);
        peer.load_reports.clear();
        // What was held for a sync before this one is in the replay.
        peer.held.clear();
        let generation = peer.generation.0;
        let world = if self.settings.compress_state {
            ServerMessage::packed_snapshot(generation, snapshot.time, &snapshot.body)
        } else {
            ServerMessage::snapshot(generation, snapshot.time, &snapshot.body)
        };
        // A game with no world yet stands still until it has one: its
        // first is kept on its way for longer than the client table's
        // 1.75 s, so that what a thin line lets through of it adds up.
        let lifetime = entering
            .then(|| entry_lifetime(self.settings.timing.entry_state, clock, start))
            .flatten();
        // The time once more, for this very moment: it travels ahead of
        // the inputs and the snapshot's first part.
        if !world_only {
            self.send_set_time(index, snapshot.time, entering, now);
        }
        let Some(peer) = self.peers[index].as_mut() else {
            return;
        };
        for message in &replay {
            if entering {
                send_waiting(peer, message);
            } else {
                send_kept(peer, message);
            }
        }
        peer.early_entries.clear();
        if peer.time_answered {
            for (at, who, message) in &entries {
                if let Some(serial) = send_waiting(peer, message) {
                    peer.early_entries.push((*at, *who, serial, false));
                }
            }
        }
        let payload = world.payload();
        // A world a game that plays waits for: its parts lost go again
        // sooner than 0xE8's 500 ms (`RESYNC_WORLD_RESEND_MIN`).
        let resend = resync_from.is_some().then(|| resend_pace(peer));
        let Some(serial) = send_world(peer, &payload, lifetime, resend) else {
            return;
        };
        let replayed = match (resync_from, peer.replayed) {
            (Some(since), Some((_, at))) => format!(
                "its replay went {} ms after its first 0xE5, the world {} ms after it",
                at.saturating_duration_since(since).as_millis(),
                now.saturating_duration_since(since).as_millis()
            ),
            _ => format!(
                "{} inputs of the others since the snapshot ahead of it",
                replay.len()
            ),
        };
        log::info!(
            "Match {id:#x}: player {index} starts from the snapshot at {} ms ({} bytes, generation \
             {generation}) at {clock} ms, its clock at {start} ms, {replayed}, {} bytes on the \
             wire",
            snapshot.time,
            snapshot.body.len(),
            payload.len()
        );
        // Only a client joining counts as the latest to start.
        if entering {
            peer.synced_at = Some(now);
        }
        peer.stage = Stage::Syncing {
            serial,
            time: snapshot.time,
            entering,
        };
    }

    /// Whether the client at `index`, whose world is on its way, shows that
    /// it has loaded it: a hash report it sent since, of a tick past the
    /// snapshot, with the CRC the majority has for that tick. A game's acks
    /// name only the newest few of its seqs (transport.md section 2.2): on
    /// a short round trip the parts of a world sent in one burst were most
    /// of them never acked, though loaded (live, 2026-10-14: the right hash
    /// 230 to 290 ms after the burst, in 45 of 47 syncs, and the sync done
    /// again all the same). A report that does not agree proves nothing: a
    /// game goes on reporting while its world is on the way, from the old
    /// world. So only a report that came at least the line's shortest
    /// round trip after the world first went counts (sooner, it was made
    /// before the world could be there), of a tick past the world's time,
    /// with the CRC the majority has for that tick.
    fn load_proven(&self, index: usize) -> bool {
        self.peers[index].as_ref().is_some_and(|peer| {
            let Stage::Syncing { time: from, .. } = peer.stage else {
                return false;
            };
            let Some(sent) = peer.sync_sent_at else {
                return false;
            };
            let earliest = sent + Duration::from_millis(u64::from(shortest_round_trip(peer)));
            peer.load_reports.iter().any(|&(time, crc, at)| {
                time > from && at >= earliest && self.majority(time) == Some(crc)
            })
        })
    }

    /// A sync that did not arrive whole goes again from the start, as the
    /// next generation.
    fn redo_sync(&mut self, index: usize, entering: bool, why: &str, now: Instant) {
        self.reset_catchup(index, false);
        log::warn!(
            "Match {:#x}: player {index}'s sync is done again: {why}",
            self.id
        );
        if let Some(peer) = self.peers[index].as_mut() {
            peer.time_confirmed = false;
            peer.time_answered = false;
            peer.early_entries.clear();
            peer.set_times.clear();
            // The next sync replays every input since its own snapshot.
            peer.held.clear();
            peer.replayed = None;
            // The next generation's replay or world holds all of it.
            clear_gate(peer, None, now);
            peer.world_waits = 0;
            peer.world_wait_since = None;
            peer.stage = Stage::WaitingSnapshot {
                since: now,
                entering,
            };
        }
    }

    /// Whether the game at `index`, whose world is loaded, would get one of
    /// the others' roster changes past its horizon: one of `events`, or an
    /// entry sent ahead of its load and not acked yet. Its present as a
    /// message sent now reaches it is the clock its last 0xE5 gave plus
    /// the time since that 0xE5 went (both take a way to it), and one way
    /// more for a line that holds packets. Then its start is done again
    /// from a fresh world, which holds them, [`LATE_ROSTER_REDOS`] times at
    /// most: a game without the change is out of step from its tick on. A
    /// 0xE9 past the horizon is refused (`enter_match` 0x2D15C2, PROVEN);
    /// a 0xEA LIKELY the same way (`leave_match` 0x2D18B9 not read for
    /// it), and a start done again for one costs no more than a load.
    fn roster_too_late(
        &mut self,
        index: usize,
        entering: bool,
        events: &[(u32, ServerMessage)],
        now: Instant,
    ) -> bool {
        let Some(peer) = self.peers[index].as_ref() else {
            return false;
        };
        if peer.late_roster_redos >= LATE_ROSTER_REDOS {
            return false;
        }
        let Some(sent) = peer.set_times.back() else {
            return false;
        };
        let present =
            sent.start + now.saturating_duration_since(sent.at).as_millis() as u32 + one_way(peer);
        let late = |at: u32| at + EVENT_HORIZON_MS <= present;
        // Its own entry (a player back in) is as ever.
        let mut too_late: Vec<u32> = events
            .iter()
            .filter(|(at, message)| {
                let who = match message {
                    ServerMessage::PlayerEntered { player, .. }
                    | ServerMessage::PlayerLeft { player, .. } => usize::from(*player),
                    _ => index,
                };
                who != index && late(*at)
            })
            .map(|(at, _)| *at)
            .collect();
        too_late.extend(
            peer.early_entries
                .iter()
                .filter(|(at, _, serial, after)| {
                    *after && !peer.transport.is_delivered(*serial) && late(*at)
                })
                .map(|(at, _, _, _)| *at),
        );
        if too_late.is_empty() {
            return false;
        }
        let redo = peer.late_roster_redos + 1;
        if let Some(peer) = self.peers[index].as_mut() {
            peer.late_roster_redos = redo;
        }
        let why = format!(
            "{} roster changes since its world (the oldest at {} ms) would reach it past its \
             horizon (its game at about {present} ms; {redo} of {LATE_ROSTER_REDOS} times)",
            too_late.len(),
            too_late.iter().min().copied().unwrap_or(0)
        );
        self.redo_sync(index, entering, &why, now);
        true
    }

    /// The snapshot reached the client: the roster changes since, and the
    /// player itself if it enters now. The inputs since went ahead of the
    /// snapshot (`send_sync`) and to the loading client as they came
    /// (`relay_to_others`): none are replayed here. A client resynced
    /// while it plays had the roster changes with its replay and as they
    /// came, and now gets the answers to its own inputs held while it
    /// stood still (`send_held_answers`).
    fn finish_sync(&mut self, index: usize, time: u32, entering: bool, now: Instant) {
        // A connection can change while E8 is in flight. Do not activate a
        // new slot against an existing client that is still loading it.
        if entering && !self.join_ready(index) {
            self.redo_sync(index, entering, "roster preparation changed", now);
            return;
        }
        let early = self.peers[index]
            .as_ref()
            .and_then(|peer| peer.entered_early);
        let resync_from = self.peers[index].as_ref().and_then(|peer| peer.resync_from);
        // A game that enters gets the roster changes since its world now,
        // after its load: before it, the client's 0xEA handler crashes
        // (`roster_change`). The entries sent ahead of the load
        // (`entries_since`) are not sent again.
        let events: Vec<(u32, ServerMessage)> = if resync_from.is_some() {
            Vec::new()
        } else {
            let sent = self.peers[index]
                .as_ref()
                .map_or(&[][..], |peer| &peer.early_entries[..]);
            self.roster_since(index, time)
                .into_iter()
                .filter(|(at, message)| {
                    !matches!(message, ServerMessage::PlayerEntered { player, .. }
                        if sent.iter().any(|(t, who, _, _)| t == at && *who == usize::from(*player)))
                })
                .collect()
        };
        if resync_from.is_none() && self.roster_too_late(index, entering, &events, now) {
            return;
        }
        let clock = self.clock(now);
        let id = self.id;
        let Some(peer) = self.peers[index].as_mut() else {
            return;
        };
        peer.early_entries.clear();
        peer.late_roster_redos = 0;
        // The inputs still held for it: from here on they come one by one.
        flush_held(peer);
        for (at, message) in events {
            if let Some(serial) = send(peer, &message) {
                track(peer, serial, Flight::Roster(message), at);
            }
        }
        peer.stage = Stage::Live;
        peer.clock_set = true;
        peer.next_ping = now;
        // It has just loaded a world; its first hash report is a horizon
        // away.
        peer.simulating_at = Some(now);
        peer.reported_tick = None;
        if let Some(since) = peer.resync_from.take() {
            log::info!(
                "Match {id:#x}: player {index} is in sync, {} ms after its first 0xE5 (its world \
                 went {} ms after it; acked {} ms after the snapshot's time, worlds done again \
                 for a slow load {})",
                now.saturating_duration_since(since).as_millis(),
                peer.sync_sent_at
                    .map_or(0, |at| at.saturating_duration_since(since).as_millis()),
                clock.saturating_sub(time),
                peer.slow_loads.min(SLOW_LOADS_REDONE)
            );
            peer.slow_loads = 0;
            peer.replay_redos = 0;
            peer.resync_generations = 0;
            peer.world_waits = 0;
            peer.world_wait_since = None;
            // What was kept of it since its world went stays kept until
            // acked; only the tracking ends.
            peer.sync_pending.clear();
            peer.sync_relays.clear();
            peer.gate_since = None;
            peer.gate_checked = None;
        } else {
            log::info!("Match {id:#x}: player {index} is in sync");
        }
        peer.entered_early = None;
        let reload = std::mem::take(&mut peer.reload_due);
        if let Some(at) = early {
            log::info!(
                "Match {id:#x}: player {index} is back in, from the snapshot at {time} ms (it \
                 entered at {at} ms)"
            );
            if reload {
                self.reload(index, now);
            }
        } else if entering && self.waiting_entry(index) {
            if reload {
                self.reload(index, now);
            }
            // poll_roster schedules entry only after a matching catch-up
            // hash, independently of this snapshot's transport ACK.
        } else if entering {
            let at = grid_up(clock.max(time + 1) + ROSTER_LEAD_MS);
            self.roster_change(None, index, true, at, now);
            log::info!("Match {id:#x}: player {index} entered at {at} ms");
        } else if !self.send_held_answers(index, time, now) {
            // Its clock was put at the match clock, or ahead of it by its
            // newest input: brought there by the pings that follow.
            self.correct_clock(index, now);
        }
    }

    /// The answers to its own inputs held while the client at `index` stood
    /// still, now that it has loaded the snapshot at `time`. Whether the
    /// sync is done again instead.
    ///
    /// The client never plays its own nodes up to the snapshot, and the
    /// snapshot has what the others did (PROVEN, match.md section 30): an
    /// answer for an input up to it is not needed. But an input moved past
    /// the snapshot is in the others' worlds at its new tick and not in the
    /// client's. One for a later input goes now while the client still has
    /// the state before it (its clock less [`ANSWER_REACH_MS`]: the last
    /// 0xE5's clock and what passed since, a little ahead of it). Any other
    /// takes a snapshot past it: the sync is done again from one.
    fn send_held_answers(&mut self, index: usize, time: u32, now: Instant) -> bool {
        let clock = self.clock(now);
        let id = self.id;
        let Some(peer) = self.peers[index].as_mut() else {
            return false;
        };
        let held = std::mem::take(&mut peer.held_answers);
        if held.is_empty() {
            return false;
        }
        let client_clock = given_clock(&peer.set_times, now).unwrap_or(clock);
        let past =
            |answer: &Answer| answer.time > time || answer.moved_to.is_some_and(|to| to > time);
        let lost = held
            .iter()
            .filter(|answer| {
                past(answer)
                    && (answer.time <= time || answer.time + ANSWER_REACH_MS <= client_clock)
            })
            .count();
        if lost > 0 && peer.answer_redos >= SLOW_LOADS_REDONE {
            // A link that stalls would have the sync done again for ever:
            // the answers go as they are, and the game may be out of step.
            log::warn!(
                "Match {id:#x}: player {index}: {lost} answers to its own inputs held while it \
                 loaded the snapshot at {time} ms could no longer be taken (its clock about \
                 {client_clock} ms), after {} syncs done again for them; sent all the same",
                peer.answer_redos
            );
        } else if lost > 0 {
            peer.answer_redos += 1;
            let floor = held
                .iter()
                .map(|answer| answer.moved_to.unwrap_or(0).max(answer.time))
                .max()
                .unwrap_or(0);
            peer.resync_floor = peer.resync_floor.max(floor);
            peer.held_answers = held.into_iter().filter(past).collect();
            let why = format!(
                "{lost} answers to its own inputs held while it loaded the snapshot at {time} ms \
                 are past the snapshot and could no longer be taken (its clock about \
                 {client_clock} ms); again from a snapshot from {floor} ms on"
            );
            self.redo_sync(index, false, &why, now);
            return true;
        }
        peer.answer_redos = 0;
        let mut sent = 0;
        for answer in held.into_iter().filter(past) {
            sent += 1;
            answer_owner(peer, answer, now);
        }
        log::info!(
            "Match {id:#x}: player {index} is sent {sent} answers to its own inputs held while it \
             loaded the snapshot at {time} ms"
        );
        false
    }

    fn input(&mut self, index: usize, input: PlayerInput, now: Instant) {
        if self.waiting_entry(index) {
            return;
        }
        let clock = self.clock(now);
        let id = self.id;
        let late_inputs = self.settings.late_inputs;
        let reach = self.late_relay_reach(index);
        let Some(peer) = self.peers[index].as_mut() else {
            return;
        };
        // A client resynced while it plays is in the others' worlds all
        // the while: its inputs are handled as a live one's (match.md
        // section 30). A client that enters has no world, and makes none;
        // one back in the match is in the others' worlds since its entry,
        // and makes them once its world is loaded, before the server has
        // that word (`takes_inputs`).
        if !takes_inputs(peer) {
            return;
        }
        let Some(time) = input.time.resolve(clock) else {
            return;
        };
        if time > clock + TOO_NEW_MS {
            log::warn!(
                "Match {id:#x}: player {index}'s input at {time} ms is ahead of {clock} ms; dropped"
            );
            return;
        }
        if time + INPUT_MAX_AGE_MS < clock {
            return;
        }
        // The client resends until acked, and the transport does not dedup
        // plain messages: each copy comes again.
        // A resynced game never makes a second input for a tick it has one
        // for (`sync_start`): a time seen is always a copy.
        if !peer.inputs_seen.insert(time) {
            return;
        }
        if peer.inputs_seen.len() > 1024 {
            let oldest = clock.saturating_sub(INPUT_MAX_AGE_MS);
            peer.inputs_seen.retain(|seen| *seen >= oldest);
        }
        if !input.rotation_is_sane() {
            // Relayed, it would be in every client's world for good
            // (`PlayerInput::rotation_is_sane`). Its owner drops it as it
            // drops a late one; a copy of it is known by its time and goes
            // nowhere.
            peer.inputs_refused += 1;
            answer_owner(
                peer,
                Answer::input(ServerMessage::DiscardInput { time: input.time }, time, None),
                now,
            );
            if peer.inputs_refused <= REFUSED_LOGGED {
                log::warn!(
                    "Match {id:#x}: player {index}'s input at {time} ms has a rotation no client \
                     makes (bytes {:02x?}, actions {:#x}); not relayed, its owner told to drop it",
                    input.rotation,
                    input.actions
                );
            }
            return;
        }
        peer.inputs_received += 1;
        // Made by its game for a tick of the present: it simulates.
        peer.simulating_at = Some(now);
        let late = clock.saturating_sub(time);
        peer.latest_input_ms = peer.latest_input_ms.max(late);
        // Its link, not its clock: a client that just entered runs behind
        // the match clock by design.
        let behind = estimated_lag(peer, now).map_or(0, |lag| lag.max(0) as u32);
        let in_transit = late.saturating_sub(behind);
        peer.late_share +=
            (f32::from(u8::from(in_transit > LATE_INPUT_MS)) - peer.late_share) * LATE_SHARE_WEIGHT;
        if in_transit > LATE_INPUT_MS {
            peer.inputs_late += 1;
        }
        let mut relayed = input;
        let mut relayed_time = time;
        forget_placed(&mut peer.placed, clock);
        let placing = place(
            late_inputs,
            clock,
            time,
            &peer.placed,
            clock.saturating_sub(behind),
            reach,
        );
        if time + HORIZON_MS <= clock {
            if placing == Placing::Keep {
                // Past the horizon, but every other game can still take it
                // at its tick (`late_relay_reach`).
                peer.inputs_relayed_late += 1;
            } else {
                // Final already: everyone else has played that tick
                // without it.
                let run = peer.discard_run.get_or_insert(DiscardRun {
                    since: now,
                    count: 0,
                    worst_ms: 0,
                });
                run.count += 1;
                run.worst_ms = run.worst_ms.max(late);
            }
        }
        match placing {
            Placing::Keep => {}
            Placing::Discard => {
                peer.inputs_discarded += 1;
                if peer.discarded.len() == MISSED_KEPT {
                    peer.discarded.pop_front();
                }
                peer.discarded.push_back(time);
                answer_owner(
                    peer,
                    Answer::input(ServerMessage::DiscardInput { time: input.time }, time, None),
                    now,
                );
                log::debug!(
                    "Match {id:#x}: player {index}'s input at {time} ms came at {clock} ms; \
                     discarded"
                );
                return;
            }
            Placing::Move(to) => {
                // Its owner hears where it went, the others get it there.
                peer.inputs_moved += 1;
                relayed.time = WireTime {
                    tick: tick_byte(to),
                    ms: input.time.ms,
                };
                relayed_time = to;
                let message = ServerMessage::MoveInput {
                    from: input.time,
                    to: relayed.time,
                };
                answer_owner(peer, Answer::input(message, time, Some(to)), now);
                log::debug!(
                    "Match {id:#x}: player {index}'s input at {time} ms came at {clock} ms; moved \
                     to {to} ms"
                );
            }
        }
        peer.placed.push_back((time, relayed_time));
        self.observe_input(index, relayed_time, &relayed);
        let what = Relayed::Input(relayed);
        self.relay_to_others(index, &what, relayed_time, now);
        self.inputs.push((relayed_time, index, what));
    }

    /// 0xDE: LIKELY the kit the player picks for its next respawn. Relayed
    /// as an input is.
    fn profile_input(&mut self, index: usize, tick: u8, value: u8, now: Instant) {
        if self.waiting_entry(index) {
            return;
        }
        let clock = self.clock(now);
        let late_inputs = self.settings.late_inputs;
        let reach = self.late_relay_reach(index);
        let Some(peer) = self.peers[index].as_mut() else {
            return;
        };
        // As an input (`input`).
        if !takes_inputs(peer) {
            return;
        }
        let Some(time) = tick_time(tick, clock) else {
            return;
        };
        if time > clock + TOO_NEW_MS || time + INPUT_MAX_AGE_MS < clock {
            return;
        }
        // Resent every 50 ms until acked, with no transport dedup.
        if !peer.profiles_seen.insert((time, value)) {
            return;
        }
        if peer.profiles_seen.len() > 256 {
            let oldest = clock.saturating_sub(INPUT_MAX_AGE_MS);
            peer.profiles_seen.retain(|(seen, _)| *seen >= oldest);
        }
        let mut relayed_tick = tick;
        let mut relayed_time = time;
        let behind = estimated_lag(peer, now).map_or(0, |lag| lag.max(0) as u32);
        forget_placed(&mut peer.placed_profiles, clock);
        match place(
            late_inputs,
            clock,
            time,
            &peer.placed_profiles,
            clock.saturating_sub(behind),
            reach,
        ) {
            Placing::Keep => {}
            Placing::Discard => {
                if peer.discarded.len() == MISSED_KEPT {
                    peer.discarded.pop_front();
                }
                peer.discarded.push_back(time);
                answer_owner(
                    peer,
                    Answer::profile(ServerMessage::DiscardProfileInput { tick }, time, None),
                    now,
                );
                return;
            }
            Placing::Move(to) => {
                relayed_tick = tick_byte(to);
                relayed_time = to;
                let message = ServerMessage::MoveProfileInput {
                    from_tick: tick,
                    to_tick: relayed_tick,
                };
                answer_owner(peer, Answer::profile(message, time, Some(to)), now);
            }
        }
        peer.placed_profiles.push_back((time, relayed_time));
        self.note_kit_choice(index, relayed_time);
        let what = Relayed::Profile {
            tick: relayed_tick,
            value,
        };
        self.relay_to_others(index, &what, relayed_time, now);
        self.inputs.push((relayed_time, index, what));
    }

    /// A kit choice of the player at `index`, relayed for `time`: counted
    /// when it comes before play begins, when the games may respawn the
    /// player in that kit where it stands (patch 31) without a death.
    pub(crate) fn note_kit_choice(&mut self, index: usize, time: u32) {
        if self.rule.before_play(time)
            && let Some(count) = self.kit_changes.get_mut(index)
        {
            *count = count.saturating_add(1);
        }
    }

    /// Send something of the player at `index` to every other client in
    /// the simulation, and to those loading a snapshot (their sync's
    /// inputs went out with it, `send_sync`, or with their replay,
    /// `send_replay`; what comes since goes as it comes, kept until acked).
    /// How far behind the match clock an input of `index` may be and still
    /// be relayed at its tick (`LateInputs::Relay`): every other game in
    /// the simulation must have it before its history is
    /// [`RELAY_USEFUL_MS`] past the tick. A relay takes a way to a game,
    /// and a game's clock runs about a way ahead of the match clock (its
    /// inputs reach the server about at their tick), so it is two ways of
    /// its usual round trip, and [`LATE_RELAY_MARGIN_MS`] more. A game
    /// that enters gets relays only after its load, from the others' worlds
    /// and its replay, which hold such an input once it is relayed (an
    /// upload is of its game's tick 500 ms back, as its hash: a relay that
    /// reached that game in its history is in it).
    fn late_relay_reach(&self, index: usize) -> u32 {
        let mut reach = RELAY_USEFUL_MS - LATE_RELAY_MARGIN_MS;
        for (other, peer) in self.peers.iter().enumerate() {
            if other == index || self.is_bot(other) || self.waiting_entry(other) {
                continue;
            }
            let Some(peer) = peer else {
                continue;
            };
            let simulating = match peer.stage {
                Stage::Live => true,
                Stage::Syncing { entering, .. }
                | Stage::Timing { entering, .. }
                | Stage::WaitingSnapshot { entering, .. } => !entering,
                _ => false,
            };
            if simulating {
                let round_trip = usual_round_trip(&peer.round_trips, peer.round_trip);
                reach =
                    reach.min((RELAY_USEFUL_MS - LATE_RELAY_MARGIN_MS).saturating_sub(round_trip));
            }
        }
        reach
    }

    fn relay_to_others(&mut self, index: usize, what: &Relayed, time: u32, now: Instant) {
        let message = relay(index, what);
        let clock = self.clock(now);
        for (other, peer) in self.peers.iter_mut().enumerate() {
            if other == index {
                continue;
            }
            let Some(peer) = peer else {
                continue;
            };
            match peer.stage {
                Stage::Live => {
                    let round_trip = usual_round_trip(&peer.round_trips, peer.round_trip);
                    let until = relay_deadline(round_trip, clock, time, now);
                    if let Some(serial) = send_until(peer, &message, until) {
                        track_sent(peer, serial, Flight::Relay, time, Some(now));
                    }
                }
                // A game with no world yet: in no hurry, and it acks ten
                // times a second (`WAITING_RELAY_FLUSH`).
                Stage::Syncing {
                    time: from,
                    entering: true,
                    ..
                } if time > from => peer.held.push(message.clone()),
                Stage::Syncing { time: from, .. } if time > from => {
                    send_kept(peer, &message);
                }
                // Resynced, its replay sent: its world waits for this too
                // (`resync_timed`). Before the replay it is in the replay.
                Stage::Timing {
                    time: from,
                    entering: false,
                    ..
                } if time > from && peer.replayed.is_some() => {
                    if let Some(serial) = send_kept(peer, &message) {
                        gate(peer, serial, now, time, GateKind::Relay);
                    }
                }
                _ => {}
            }
        }
    }

    /// A client's whole world came (its 0xE0 chunks).
    fn uploaded(&mut self, index: usize, upload: Upload, now: Instant) {
        let clock = self.clock(now);
        let initial = self.snapshot.is_none() && self.requested_at.is_none();
        let joiner = self.waiting_entry(index);
        let id = self.id;
        let snapshot = Snapshot {
            time: upload.item_time,
            body: upload.body,
        };
        let Some(peer) = self.peers[index].as_mut() else {
            return;
        };
        // After the initial world, only an upload that was asked for, of a
        // tick near the match clock: one sent unasked, or for a time no
        // client is at, could put a forged world or results in. (A client
        // sends its initial world on every join, also when not the first.)
        if !initial {
            let in_window = snapshot.time.is_multiple_of(TICK_MS)
                && snapshot.time <= clock + UPLOAD_MAX_AHEAD_MS
                && snapshot.time + UPLOAD_MAX_AGE_MS >= clock;
            if !peer.is_asked(now) || !in_window {
                log::debug!(
                    "Match {id:#x}: player {index} uploaded a snapshot at {} ms unasked or out of \
                     time (clock {clock} ms); ignored",
                    snapshot.time
                );
                return;
            }
        }
        peer.asked = None;
        peer.unanswered_since = None;
        if !initial && let Some(asked_at) = peer.wrong_world_asked.take() {
            // Asked for as it was found out of step: a world to look at,
            // no vote (`ask_wrong_world`).
            self.wrong_world_uploaded(index, asked_at, &snapshot);
            return;
        }
        if !initial && joiner {
            // Asked for over its wrong reports: a world to look at, no vote
            // (`check_joiner_worlds`).
            peer.asked_clock = None;
            self.joiner_uploaded(index, &snapshot);
            return;
        }
        // A world of the present: its game simulates.
        peer.simulating_at = Some(now);
        if !initial && let Some(asked) = peer.asked_clock.take() {
            observe_upload(&mut peer.upload_offset, asked, snapshot.time);
        }
        // The server needs no more of a world than its bytes; that it does
        // not split into its sections here says the reader is wrong (no
        // 0.26g world was seen when it was written) as likely as the world.
        // It is used, saved to look at, and gives no results.
        if Sections::candidates(&snapshot.body).is_empty() {
            log::warn!(
                "Match {id:#x}: player {index}'s upload at {} ms ({} bytes) does not read as a \
                 world (a mask, players and six sections); used as it is, without results",
                snapshot.time,
                snapshot.body.len()
            );
            self.dump(&snapshot, index, "unreadable");
        }
        if initial {
            self.first_snapshot(index, snapshot, now);
            return;
        }
        // What the client uploads is what its hash covers: a report.
        let crc = snapshot::crc(&snapshot.body);
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

    /// The first upload starts the match. Every client built the same
    /// initial world, so it needs no check (there is nothing to check it
    /// against yet).
    fn first_snapshot(&mut self, index: usize, mut snapshot: Snapshot, now: Instant) {
        if self.settings.check_initial_world
            && let Some(why) = self.not_initial(&snapshot)
        {
            log::warn!(
                "Match {:#x}: player {index}'s initial world has {why}; not used (if every \
                 client's is refused so, the reader is wrong: SRP_CHECK_INITIAL_WORLD=off)",
                self.id
            );
            self.dump(&snapshot, index, "forged_initial");
            if let Some(peer) = self.peers[index].as_mut() {
                peer.patched = false;
            }
            return;
        }
        // The clock runs on the tick grid [LIKELY the initial world is on
        // it: 0.21d's was at 50 ms].
        if !snapshot.time.is_multiple_of(TICK_MS) {
            log::warn!(
                "Match {:#x}: the initial world is at {} ms, off the tick grid",
                self.id,
                snapshot.time
            );
            snapshot.time = grid_up(snapshot.time);
        }
        log::info!(
            "Match {:#x}: player {index} gave the initial world ({} bytes at {} ms, CRC {:08x})",
            self.id,
            snapshot.body.len(),
            snapshot.time,
            snapshot::crc(&snapshot.body)
        );
        self.clock_start = Some((now, snapshot.time));
        self.record_note(
            serde_json::json!({ "note": "clock", "match_ms": snapshot.time, "place": index }),
            now,
        );
        self.rule.on_match_ready(snapshot.time);
        self.dump(&snapshot, index, "initial");
        self.observe(&snapshot, 1);
        self.snapshot = Some(snapshot);
    }

    /// What a world that claims to be the initial one has that it can't:
    /// kills, deaths or any counted event (everyone starts from this world,
    /// so what it holds is in every later one). `None` for a clean one, or
    /// one that can't be read (nothing in it can count either). The points
    /// are not looked at: what they hold before the rules' first event was
    /// not traced.
    fn not_initial(&self, snapshot: &Snapshot) -> Option<String> {
        let decoded = snapshot::decode(
            snapshot.time,
            &snapshot.body,
            self.snapshot_players(),
            self.game_mode(),
            self.batteries(),
        )?;
        if let Some(read) = &decoded.rules
            && read
                .kills_deaths
                .iter()
                .any(|record| record.kills != 0 || record.deaths != 0)
        {
            return Some("kills or deaths".to_owned());
        }
        let statistics = decoded.statistics.as_ref()?;
        let played = statistics.players.iter().any(|player| {
            player.events.iter().flatten().any(|count| *count != 0) || player.killstreak != 0
        });
        played.then(|| "statistics".to_owned())
    }

    /// Read a trusted snapshot's statistics and rules: the standing, the
    /// phase, and whether the mode's rule has its end.
    fn observe(&mut self, snapshot: &Snapshot, vouchers: usize) {
        let Some(decoded) = snapshot::decode(
            snapshot.time,
            &snapshot.body,
            self.snapshot_players(),
            self.game_mode(),
            self.batteries(),
        ) else {
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
            .is_some_and(|state| state.time > snapshot.time)
        {
            return;
        }
        self.observe_driven(snapshot, &decoded);
        let teams: Vec<u8> = self.formed.places.iter().map(|place| place.team).collect();
        let standing = decoded.standing(self.game_mode(), &teams);
        if let Err(why) = self.plausible(&decoded, standing, snapshot.time) {
            log::warn!(
                "Match {:#x}: the snapshot at {} ms is not plausible ({why}); its results are \
                 not used",
                self.id,
                snapshot.time
            );
            return;
        }
        if let Some(statistics) = &decoded.statistics {
            for (best, player) in self.best_streaks.iter_mut().zip(&statistics.players) {
                *best = (*best)
                    .max(player.longest_killstreak)
                    .max(u16::from(player.killstreak));
            }
        }
        if let Some(read) = &decoded.rules {
            // The map's respawn points are known from its project: a rule
            // read with another number was read wrong, or the data is.
            let expected = Some(self.formed.map.respawn_points);
            let points = read.respawn.as_ref().map(|respawn| respawn.points.len());
            if points != expected && !self.respawn_points_logged {
                self.respawn_points_logged = true;
                log::warn!(
                    "Match {:#x}: the snapshot at {} ms has {points:?} respawn points in its \
                     rules, the map {expected:?}",
                    self.id,
                    snapshot.time
                );
            }
            // The clients' own rule says when the countdown began (a 0xE9
            // they stamped onto a later tick moves it).
            let theirs = &read.timelimit;
            if theirs.state >= rules::state::COUNTDOWN && theirs.start.is_some() {
                if self.rule.state != theirs.state || self.rule.start != theirs.start {
                    log::debug!(
                        "Match {:#x}: the clients' time rule is in state {} since {:?} ms (here \
                         {} since {:?})",
                        self.id,
                        theirs.state,
                        theirs.start,
                        self.rule.state,
                        self.rule.start
                    );
                }
                self.rule.state = theirs.state;
                self.rule.start = theirs.start;
            }
        }
        // The mode rule's end needs play to have begun: a wrong read must
        // not end a match. Where the rules did not read, the server's own
        // copy of the time rule says whether play began.
        let playing = match &decoded.rules {
            Some(read) => read.timelimit.state == rules::state::IN_PROCESS,
            None => self.rule.state == rules::state::IN_PROCESS,
        };
        let over = playing
            && if self.game_mode() == mode::TEAM_DEATHMATCH {
                decoded
                    .rules
                    .as_ref()
                    .and_then(|read| read.tdm)
                    .is_some_and(|tdm| {
                        rules::tdm_finished(
                            tdm.tickets,
                            rules::alive_by_team(tdm.alive_players_mask, &teams),
                        )
                    })
            } else {
                standing.is_some_and(|standing| {
                    rules::gather_finished(standing.points, self.batteries() as u32)
                })
            };
        log::debug!(
            "Match {:#x}: at {} ms the standing is {standing:?}",
            self.id,
            snapshot.time
        );
        if over && self.mode_end_at.is_none() {
            let at = snapshot.time + self.settings.timing.end_delay_ms;
            log::info!(
                "Match {:#x}: the mode's rule has its end at {} ms (standing {standing:?}); the \
                 match is over at {at} ms",
                self.id,
                snapshot.time
            );
            self.mode_end_at = Some(at);
        }
        // What each player has left, from its block.
        let mut supplies = vec![None; self.players()];
        for block in players::blocks(&snapshot.body).unwrap_or_default() {
            let place = usize::from(block.place);
            if let (Some(slot), Some(profile)) =
                (supplies.get_mut(place), self.formed.profiles.get(place))
            {
                *slot = Supplies::read(&block, profile, Dictionary::shared());
            }
        }
        self.state = Some(Observed {
            time: snapshot.time,
            vouchers: vouchers.min(usize::from(u8::MAX)) as u8,
            statistics: decoded.statistics,
            rules: decoded.rules,
            standing,
            supplies,
        });
    }

    /// Whether a snapshot's results could have come from real play: no
    /// more points than there are, or than could have been made since the
    /// match began, no more kills than deaths, and no player killing faster
    /// than one every [`MIN_MS_PER_KILL`]. Two colluding clients can agree
    /// on any CRC; this keeps what they can claim within play.
    fn plausible(
        &self,
        decoded: &snapshot::Decoded,
        standing: Option<Standing>,
        time: u32,
    ) -> Result<(), String> {
        let played = time.saturating_sub(self.first_entered.unwrap_or(0));
        if let Some(standing) = standing {
            if self.game_mode() == mode::TEAM_DEATHMATCH {
                let tickets = self.formed.map.tickets_count;
                if standing
                    .points
                    .iter()
                    .any(|team| u32::from(*team) > tickets)
                {
                    return Err(format!("tickets {:?} of {tickets}", standing.points));
                }
            } else {
                let batteries = self.batteries() as u32;
                let home = u32::from(standing.points[0]) + u32::from(standing.points[1]);
                if standing
                    .points
                    .iter()
                    .any(|team| u32::from(*team) > batteries)
                {
                    return Err(format!(
                        "batteries home {:?} of {batteries}",
                        standing.points
                    ));
                }
                if home > 1 + played / MIN_MS_PER_BATTERY {
                    return Err(format!("{home} batteries home after {played} ms"));
                }
            }
        }
        if let Some(statistics) = &decoded.statistics {
            let total = |event: usize| -> u32 {
                statistics
                    .players
                    .iter()
                    .map(|player| player.total(event))
                    .sum()
            };
            let (kills, deaths) = (total(event::KILL), total(event::DEATH));
            // A little slack: a place a player comes back to may start its
            // count again.
            if kills > deaths + statistics.players.len() as u32 {
                return Err(format!("{kills} kills but {deaths} deaths"));
            }
            let most = 1 + played / MIN_MS_PER_KILL;
            if let Some((index, player)) = statistics
                .players
                .iter()
                .enumerate()
                .find(|(_, player)| player.total(event::KILL) > most)
            {
                return Err(format!(
                    "player {index} has {} kills after {played} ms",
                    player.total(event::KILL)
                ));
            }
        }
        Ok(())
    }

    /// How many clients reported `crc` for the tick at `time`.
    fn vouchers(&self, time: u32, crc: u32) -> usize {
        self.hashes.get(&time).map_or(0, |reports| {
            reports.values().filter(|&&each| each == crc).count()
        })
    }

    /// The CRC most clients reported for the tick at `time`, if more than
    /// half of those reporting agree. One report is enough only when one
    /// client is in the simulation: with two, both must agree. Two stock
    /// clients on opposite hash phases never send a hash for the same tick
    /// (match.md section 14): a tick then has two reports only when one is
    /// an upload, which `check_hash_phases` says in the log.
    fn majority(&self, time: u32) -> Option<u32> {
        let reports = self.hashes.get(&time)?;
        if reports.len() < self.live().min(2) {
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
            let crc = snapshot::crc(&candidate.snapshot.body);
            match self.majority(candidate.snapshot.time) {
                Some(majority) if majority == crc => {
                    let newer = self
                        .snapshot
                        .as_ref()
                        .is_none_or(|snapshot| snapshot.time < candidate.snapshot.time);
                    if !newer {
                        continue;
                    }
                    log::debug!(
                        "Match {:#x}: player {}'s snapshot at {} ms matches the majority",
                        self.id,
                        candidate.from,
                        candidate.snapshot.time
                    );
                    self.dump(&candidate.snapshot, candidate.from, "checked");
                    // The results only when another client vouches for it
                    // too: alone, the uploader vouches for itself. With one
                    // client in the match there is nobody else to ask (one
                    // being resynced still counts: its word is missing).
                    let needed = self.in_match().clamp(1, 2);
                    let vouchers = self.vouchers(candidate.snapshot.time, crc);
                    if vouchers >= needed {
                        self.observe(&candidate.snapshot, vouchers);
                    } else {
                        log::info!(
                            "Match {:#x}: only player {} vouches for the snapshot at {} ms; its \
                             results are not used",
                            self.id,
                            candidate.from,
                            candidate.snapshot.time
                        );
                    }
                    self.snapshot = Some(candidate.snapshot);
                    self.requested_at = None;
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
                None if now.saturating_duration_since(candidate.since) < CANDIDATE_WAIT => {
                    self.candidates.push(candidate);
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
                    // Its world may still serve a late joiner, but its
                    // results never count: nobody vouched for them.
                    self.dump(&candidate.snapshot, candidate.from, "unchecked");
                }
            }
        }
    }

    /// With no majority: the clients known to be out of step at `time` (a
    /// relay, discard or roster change for a tick shortly before it never
    /// reached them, or an own input of theirs was dropped then:
    /// [`MISSED_RECENT_MS`]) as wrong and the others as right, if all the
    /// others agree with each other and none of the known ones does.
    fn missed_verdict(
        &self,
        time: u32,
        reports: &HashMap<usize, u32>,
    ) -> Option<(Vec<usize>, Vec<usize>)> {
        let (mut wrong, mut right): (Vec<usize>, Vec<usize>) =
            reports.keys().copied().partition(|index| {
                self.peers[*index].as_ref().is_some_and(|peer| {
                    // Near the disputed tick, or near the start of the
                    // disagreement it is in since (it may wait for its
                    // resync for seconds, and stays the one named).
                    let since = peer.diverged.as_ref().map(|divergence| divergence.first);
                    let recent = |tick: &u32| {
                        *tick <= time
                            && (*tick + MISSED_RECENT_MS > time
                                || since.is_some_and(|first| *tick + MISSED_RECENT_MS > first))
                    };
                    peer.missed.iter().any(recent) || peer.discarded.iter().any(recent)
                })
            });
        let right_crcs: HashSet<u32> = right.iter().map(|index| reports[index]).collect();
        if wrong.is_empty()
            || right_crcs.len() != 1
            || wrong
                .iter()
                .any(|index| right_crcs.contains(&reports[index]))
        {
            return None;
        }
        wrong.sort_unstable();
        right.sort_unstable();
        Some((wrong, right))
    }

    /// With no majority: the CRC of the side whose clients are the most
    /// reliable (inputs on time), if it clearly outweighs every other side.
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
    /// step are wrong, else the side with the better links is right,
    /// failing which the client that started from a snapshot last is the
    /// likely odd one out; with no such one, nobody is named.
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
            .is_none_or(|at| now.saturating_duration_since(at) >= DISAGREEMENT_LOG)
        {
            self.disagreement_logged = Some(now);
            log::warn!(
                "Match {:#x}: the clients disagree at {time} ms, no majority: {reports:x?}",
                self.id
            );
            if let Some((wrong, _)) = &missed {
                log::info!(
                    "Match {:#x}: players {wrong:?} never acked a relay, discard or roster change \
                     for a tick up to {time} ms: they are out of step",
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
    ///
    /// A report taken for right by no strict majority (a tie judged by the
    /// links or by who started last) does not end a disagreement that
    /// stands: live, 2026-10-14, two reports at a tick named a player that
    /// had been wrong for 10.5 s right, and wiped that.
    fn note_report(&mut self, index: usize, time: u32, right: bool, strict: bool, now: Instant) {
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
        } else if peer.diverged.is_none() {
            // A report that agrees, with none against it standing: its
            // last restore worked, once it is past the time its world may
            // still turn out not to have taken (`LOAD_WRONG_MS`). Live,
            // 2026-10-15: right for one or two reports, wrong 105 and 261
            // ms after the load, and the quick redo was lost to them.
            if time > peer.judge_from + LOAD_WRONG_MS {
                peer.restores_unagreed = 0;
            }
        } else if !strict {
            // Not enough to say it agrees again.
        } else if let Some(divergence) = peer.diverged.take() {
            peer.restores_unagreed = 0;
            log::info!(
                "Match {id:#x}: player {index} disagreed from {} to {} ms and agrees again at \
                 {time} ms; not resynced",
                divergence.first,
                divergence.last
            );
        }
    }

    /// Find clients whose reports have disagreed with the right ones for a
    /// while, and resync them. A single wrong report is not enough: a world
    /// that was out of step for a moment reports right again, and a resync
    /// costs the client its clock and its inputs for seconds.
    fn check_hashes(&mut self, now: Instant) {
        let clock = self.clock(now);
        let timing = self.settings.timing.clone();
        let settled = clock.saturating_sub(timing.hash_settle_ms);
        let installing = self.installing();
        if installing {
            // The games' worlds differ by the new place until each has
            // it: no report up to now says that a game is out of step,
            // and a resync would give it a world of the other kind. What
            // stood against a game before stands (until 2026-10-14 it was
            // forgotten, and a real desync seen 3 s late), and is acted
            // on once every game has the place.
            self.hashes_unjudged_until = clock;
        }
        let unjudged_until = self.hashes_unjudged_until;
        let times: Vec<u32> = self
            .hashes
            .range(self.hashes_checked_until..settled)
            .map(|(time, _)| *time)
            .filter(|time| *time > unjudged_until)
            .collect();
        for time in times {
            let (wrong, right) = self.judge(time, now);
            let strict = self.majority(time).is_some();
            for index in wrong {
                self.note_report(index, time, false, strict, now);
            }
            for index in right {
                self.note_report(index, time, true, strict, now);
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
                    !installing
                        && peer.stage == Stage::Live
                        && peer.diverged.as_ref().is_some_and(|divergence| {
                            divergence.last - divergence.first >= timing.divergence_confirm_ms
                                || now.saturating_duration_since(divergence.since)
                                    >= timing.divergence_confirm_max
                        })
                })
            })
            .map(|(index, _)| index)
            .collect();
        for index in desynced {
            let Some(peer) = self.peers[index].as_mut() else {
                continue;
            };
            // Wrong from its first reports after a world it was resynced
            // with: that world did not take, and waiting the interval out
            // leaves it out of step for nothing (live, 2026-10-15: 9 to 13
            // s more each time). Twice in a row at most: a game that no
            // world helps keeps the interval.
            let wrong_at_load = peer
                .diverged
                .as_ref()
                .is_some_and(|divergence| divergence.first <= peer.judge_from + LOAD_WRONG_MS)
                && peer.restores_unagreed > 0
                && peer.restores_unagreed <= QUICK_REDOS;
            // Its last resync was agreed with past the load: that world
            // took, and this is a new desync, not the same one again (live,
            // 2026-10-15: 21 such waited the interval out, 4 to 12 s out of
            // step each). A game resynced every few seconds all the same
            // waits the whole interval.
            let busy = peer.resynced_at.len() >= RESYNCS_PER_MINUTE
                && peer
                    .resynced_at
                    .front()
                    .is_some_and(|at| now.saturating_duration_since(*at) < RESYNC_WINDOW);
            let interval = if peer.restores_unagreed == 0 && !busy {
                timing.resync_after_agreed.min(timing.resync_interval)
            } else {
                timing.resync_interval
            };
            if !wrong_at_load
                && peer
                    .last_resync
                    .is_some_and(|at| now.saturating_duration_since(at) < interval)
            {
                continue;
            }
            // Its clock is still being brought up to the match clock: until
            // then its inputs come too late to relay, and a resync now
            // starts it behind once more. The disagreement stands.
            if peer.correcting_until.is_some_and(|until| now < until) {
                let since = *peer.resync_deferred_since.get_or_insert(now);
                if now.saturating_duration_since(since) < RESYNC_DEFER_MAX {
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
            let wrong = peer.diverged.take().map_or(String::new(), |divergence| {
                format!(
                    ", wrong from {} to {} ms",
                    divergence.first, divergence.last
                )
            });
            let limit = self.settings.restore_limit;
            if limit != 0 && peer.restores_unagreed >= limit {
                // `SRP_RESTORE_LIMIT`: so many restores in a row never
                // brought it to agree; it is put out instead of restored
                // for ever.
                log::warn!(
                    "Match {:#x}: player {index} is out of sync{wrong} after {} restores that \
                     did not help; disconnected",
                    self.id,
                    peer.restores_unagreed
                );
                peer.transport.kick(now);
                self.leave(index, now);
                continue;
            }
            peer.last_resync = Some(now);
            if peer.resynced_at.len() >= RESYNCS_PER_MINUTE {
                peer.resynced_at.pop_front();
            }
            peer.resynced_at.push_back(now);
            peer.resyncs += 1;
            peer.restores_unagreed += 1;
            log::warn!(
                "Match {:#x}: player {index} is out of sync{wrong}; resyncing it",
                self.id
            );
            // Its wrong world first, to look at: asked for ahead of its
            // 0xE5, it is the world it played in.
            self.ask_wrong_world(index, now);
            // A fresh snapshot from the others: the inputs since it bring
            // the client to now.
            self.start_sync(index, false, now);
        }
    }

    /// The player left: 0xE0, a disconnect, a timeout, or a new connection
    /// for its place. The others see it go.
    fn leave(&mut self, index: usize, now: Instant) {
        let waiting_entry = self.waiting_entry(index);
        let Some(peer) = self.peers[index].as_mut() else {
            return;
        };
        if peer.stage == Stage::Gone {
            return;
        }
        // Entered early, or being resynced, it is in the others' world.
        let was_in = !waiting_entry
            && (matches!(peer.stage, Stage::Live)
                || peer.entered_early.is_some()
                || matches!(
                    peer.stage,
                    Stage::Syncing {
                        entering: false,
                        ..
                    } | Stage::Timing {
                        entering: false,
                        ..
                    } | Stage::WaitingSnapshot {
                        entering: false,
                        ..
                    }
                ));
        peer.stage = Stage::Gone;
        peer.held.clear();
        peer.reassembler.clear();
        log::info!(
            "Match {:#x}: player {index} left; {}",
            self.id,
            input_summary(peer)
        );
        if !was_in {
            return;
        }
        let entered_at = self
            .events
            .iter()
            .rev()
            .find(|(_, who, _)| *who == index)
            .map_or(0, |(at, _, _)| *at);
        let at = grid_up(self.clock(now).max(entered_at + 1) + ROSTER_LEAD_MS);
        self.roster_change(Some(index), index, false, at, now);
    }

    /// Bring a client's clock to the match clock: 0xE7 with the whole
    /// remaining lag, once the last correction is done and fresh samples
    /// came in.
    ///
    /// Of the samples, the one taken over the shortest round trip counts
    /// (`best_sample`), and only when the lag it shows is more than its
    /// round trip leaves open. A sample halves its round trip, so a ping
    /// held up on its way to the client reads as a clock that is ahead by
    /// half the delay, and one held up on its way back as a clock that
    /// much behind. The smallest lag of four, which counted before, was
    /// the first kind whenever there was one: the game was told to slow
    /// its clock by 183 to 365 ms after single round trips of 665 to
    /// 924 ms on a link of 196 (live, 2026-10-14), and its inputs came
    /// too late for the seconds that followed.
    fn correct_clock(&mut self, index: usize, now: Instant) {
        if !self.settings.clock_correction {
            return;
        }
        let id = self.id;
        let Some(peer) = self.peers[index].as_mut() else {
            return;
        };
        if peer.stage != Stage::Live
            || peer.correcting_until.is_some_and(|until| now < until)
            || peer.clock_offset_pending.is_some()
        {
            return;
        }
        let usual = peer.round_trips.iter().min().copied();
        let Some(offset) = correction_for(peer.lags.iter(), corrected_at(peer), usual) else {
            return;
        };
        let lag = i64::from(offset);
        let takes = Duration::from_millis(game_clock::correction_ms(
            u32::from(offset.unsigned_abs()),
            peer.clock_factor,
        ));
        // Kept until acked (sent once by the table, it was lost again and
        // again on a lossy line, live, 2026-10-14); no newer one before.
        let message = ServerMessage::ClockOffset(offset);
        let resend = resend_pace(peer);
        if let Ok(serial) =
            peer.transport
                .send_unique_kept(message.id(), &message.payload(), resend)
        {
            peer.clock_offset_pending = Some((serial, now, offset, takes));
            peer.clock_given = None;
            peer.correction_ends =
                Some(now + takes + Duration::from_millis(u64::from(peer.round_trip)));
        }
        peer.correcting_until = Some(now + takes + CORRECTION_SETTLE);
        // Its clock moves against the match clock: so do its ticks.
        peer.upload_offset = None;
        peer.lags.clear();
        peer.lag_ms = Some(lag);
        peer.correction = Some((now, i64::from(offset), takes));
        log::info!(
            "Match {id:#x}: player {index}'s clock is {lag} ms behind; correcting it over {} ms \
             (round trip {} ms)",
            takes.as_millis(),
            peer.round_trip
        );
    }

    fn dump(&mut self, snapshot: &Snapshot, from: usize, kind: &str) {
        let Some(dir) = &self.settings.snapshot_dir else {
            return;
        };
        // A newcomer's world is asked for once at most
        // (`check_joiner_worlds`): not counted.
        if !matches!(kind, "initial" | "checked" | "joiner") {
            if self.doubtful_dumps >= MAX_DOUBTFUL_DUMPS {
                return;
            }
            self.doubtful_dumps += 1;
        }
        // Match ids are random; the start time tells runs apart all the
        // same.
        let path: PathBuf = dir.join(format!(
            "match026g_{:x}_{}_p{from}_{kind}_r{}.bin",
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

    /// Each player's round trip over the last [`PING_LOG_MS`], from the
    /// acks of the datagrams sent to it: the mean and the longest.
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

    /// What became of the relays, discards and roster changes sent to the
    /// client at `index`: acked, or dropped unacked at the end of their
    /// life (250 ms, or a relay's or drop order's deadline). One that was lost leaves the client without something the
    /// others have: out of step from that tick. A lost roster change goes
    /// once more, so that the client at least has the player.
    ///
    /// A relay that reached the game past its history (its tick 500 ms or
    /// more behind the game's clock then) was moved by the game to about
    /// its clock less 50 ms (match.md section 5.1). It is a tick the client
    /// is out of step from, and the node stays in its lists through a
    /// resync, where the replay's copy at the right tick is then ignored
    /// as a duplicate (by time, recorded before the move): a resync of it
    /// starts from a snapshot past the moved tick (`resync_floor`; LIKELY,
    /// live 2026-10-14). The ack may name a copy sent again while an
    /// earlier one had arrived in time, so a relay counts as moved only
    /// when even its first copy, on the line's shortest way, came past the
    /// game's history; the floor is put past the latest it can have come
    /// (the ack's time less the way back, on the game's clock).
    fn check_in_flight(&mut self, index: usize, now: Instant) {
        let clock = self.clock(now);
        let Some(peer) = self.peers[index].as_mut() else {
            return;
        };
        if peer.in_flight.is_empty() {
            return;
        }
        let in_flight = std::mem::take(&mut peer.in_flight);
        for each in in_flight {
            if peer.transport.is_lost(each.serial) {
                peer.relays_lost += 1;
                if peer.missed.len() == MISSED_KEPT {
                    peer.missed.pop_front();
                }
                peer.missed.push_back(each.time);
                if let Flight::Roster(message) = &each.flight {
                    send(peer, message);
                }
            } else if peer.transport.is_delivered(each.serial) {
                peer.relays_acked += 1;
                let lag = estimated_lag(peer, now)
                    .unwrap_or(0)
                    .clamp(-MAX_SANE_LAG_MS, MAX_SANE_LAG_MS);
                // The game's clock when it acked: the latest it can have
                // had the relay.
                let game = i64::from(clock) - i64::from(one_way(peer)) - lag;
                // The earliest: its first copy, on the shortest way the
                // line has shown. The ack may name a copy sent again, and
                // an earlier copy may have arrived all the same (a game
                // acks only its newest few seqs).
                let earliest = each.sent.map(|sent| {
                    let ago = now.saturating_duration_since(sent).as_millis() as i64;
                    let way = i64::from(shortest_round_trip(peer) / 2);
                    (i64::from(clock) - ago + way - lag).min(game)
                });
                let moved_for_sure = each.flight == Flight::Relay
                    && earliest.is_some_and(|earliest| i64::from(each.time) + 500 <= earliest);
                if moved_for_sure {
                    let moved = (game - 50).max(0) as u32;
                    peer.resync_floor = peer.resync_floor.max(moved + TICK_MS);
                    if peer.missed.len() == MISSED_KEPT {
                        peer.missed.pop_front();
                    }
                    peer.missed.push_back(each.time);
                    log::debug!(
                        "Match {:#x}: player {index}'s relay for {} ms reached it about {game} ms \
                         on its clock: moved by its game to about {moved} ms",
                        self.id,
                        each.time
                    );
                }
            } else {
                peer.in_flight.push_back(each);
            }
        }
    }

    /// The inputs of each client that came too late during the last
    /// [`DISCARD_LOG`], with how far its clock is behind.
    fn log_discards(&mut self, now: Instant) {
        for (index, peer) in self.peers.iter_mut().enumerate() {
            let Some(peer) = peer.as_mut() else {
                continue;
            };
            if peer
                .discard_run
                .as_ref()
                .is_none_or(|run| now.saturating_duration_since(run.since) < DISCARD_LOG)
            {
                continue;
            }
            let Some(run) = peer.discard_run.take() else {
                continue;
            };
            let lag =
                estimated_lag(peer, now).map_or("unknown".to_owned(), |lag| format!("{lag} ms"));
            let resynced = peer.last_resync.map_or("never resynced".to_owned(), |at| {
                format!(
                    "resynced {} ms ago",
                    now.saturating_duration_since(at).as_millis()
                )
            });
            log::info!(
                "Match {:#x}: player {index}: {} inputs too late to relay in {} ms, the latest {} \
                 ms past the match clock (its clock {lag} behind, {resynced})",
                self.id,
                run.count,
                now.saturating_duration_since(run.since).as_millis(),
                run.worst_ms
            );
        }
    }

    /// By place: whether its client is in the simulation now.
    fn present(&self) -> Vec<bool> {
        self.peers
            .iter()
            .enumerate()
            .map(|(index, peer)| {
                !self.waiting_entry(index)
                    && peer.as_ref().is_some_and(|peer| peer.stage == Stage::Live)
            })
            .collect()
    }

    /// The results from the last trusted snapshot.
    fn build_result(&self, end: MatchEnd, clock: u32, present: &[bool]) -> MatchResult {
        let state = self.state.as_ref();
        let standing = state.and_then(|state| state.standing);
        let statistics = state.and_then(|state| state.statistics.as_ref());
        let kills_deaths = state
            .and_then(|state| state.rules.as_ref())
            .map(|read| &read.kills_deaths);
        let players = self
            .formed
            .places
            .iter()
            .enumerate()
            .map(|(index, place)| {
                let own = statistics.and_then(|statistics| statistics.players.get(index));
                let counted = |event: usize| {
                    own.map_or(0, |player| player.total(event))
                        .min(u32::from(u16::MAX)) as u16
                };
                let record = kills_deaths.and_then(|records| records.get(index));
                let mut in_match = 0u32;
                let mut entered_at = None;
                let mut left = false;
                for (at, who, entered) in &self.events {
                    if *who != index {
                        continue;
                    }
                    if *entered {
                        entered_at.get_or_insert(*at);
                    } else {
                        left = true;
                        if let Some(since) = entered_at.take() {
                            in_match += at.saturating_sub(since);
                        }
                    }
                }
                if let Some(since) = entered_at {
                    in_match += clock.saturating_sub(since);
                }
                PlayerResult {
                    place: index as u8,
                    // The session its client last came with: a player who
                    // signed in again has another than it was seated with.
                    session_id: self
                        .peers
                        .get(index)
                        .and_then(Option::as_ref)
                        .map_or(place.session_id, |peer| peer.session_id),
                    account_id: place.account_id,
                    name: place.name.clone(),
                    team: place.team,
                    driven: place.driven,
                    entered: self
                        .events
                        .iter()
                        .any(|(_, who, entered)| *who == index && *entered),
                    present_at_end: present.get(index).copied().unwrap_or(false),
                    left,
                    time_in_match_ms: in_match,
                    statistics: own.cloned(),
                    score: own.map_or(0, |player| {
                        snapshot::score(player, &self.formed.score_weights)
                    }),
                    kills: record.map_or(counted(event::KILL), |record| record.kills),
                    deaths: record.map_or(counted(event::DEATH), |record| record.deaths),
                    longest_killstreak: self.best_streaks[index],
                    kit_changes: self.kit_changes[index],
                    supplies: state.and_then(|state| state.supplies.get(index).cloned().flatten()),
                }
            })
            .collect();
        MatchResult {
            match_id: self.id,
            map_id: self.formed.map.id,
            game_mode: self.game_mode(),
            match_level: self.formed.match_level,
            end,
            winner: standing.map(|standing| standing.winner),
            team_points: standing.map_or([0, 0], |standing| standing.points),
            duration_ms: clock,
            played_ms: self
                .rule
                .match_started_at()
                .map(|started| clock.saturating_sub(started)),
            trusted_snapshot_ms: state.map(|state| state.time),
            trusted_by: state.map_or(0, |state| state.vouchers),
            players,
            vs_bots: self.formed.vs_bots,
        }
    }

    /// End the match: the result is kept for the lobby, and every client
    /// gets 0xEA with its own index, which closes the match on it.
    fn finish(&mut self, end: MatchEnd, now: Instant) {
        if self.finished_at.is_some() {
            return;
        }
        let clock = self.clock(now);
        let present = self.present();
        let result = self.build_result(end, clock, &present);
        self.record_result(&result, now);
        log::info!(
            "Match {:#x}: over at {clock} ms ({end:?}); winner {:?}, points {:?}, scores {:?}",
            self.id,
            result.winner,
            result.team_points,
            result
                .players
                .iter()
                .map(|player| player.score)
                .collect::<Vec<_>>()
        );
        self.result = Some(result);
        self.result_made = true;
        for (index, peer) in self.peers.iter_mut().enumerate() {
            if let Some(peer) = peer
                && peer.stage != Stage::Gone
            {
                // A game that was never given a time (it waited for a
                // world that did not come) gets the tick 0: its core's
                // time is the 50 ms of its start (0x2D57F0), so the tick
                // is 0 ms, which `leave_match` refuses as "older than
                // last resync" before anything else (0x2D18B9), and the
                // 0xEA handler closes the match whatever that returned
                // (0x27C720..0x27C76F): the world is ended and the
                // results shown, as at any end. PROVEN. Until 2026-10-12
                // its link was closed instead (0xFD): that reaches
                // `close_current_match` as reason 4, which ends nothing
                // (0x27A958: entry 4 is the function's return), and the
                // game stays on the screen it waited on.
                let message = ServerMessage::PlayerLeft {
                    tick: farewell_tick(peer, clock),
                    player: index as u8,
                };
                peer.farewell = send(peer, &message).map(|serial| (serial, message));
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

    /// Advance the match, and return the datagrams to send.
    pub fn poll(&mut self, now: Instant) -> Vec<(SocketAddr, Vec<u8>)> {
        self.poll_roster(now);
        let clock = self.clock(now);
        let mono16 = self.mono(now) as u16;
        let timing = self.settings.timing.clone();
        self.check_candidates(now);
        self.check_hashes(now);
        self.check_hash_phases(now);
        self.log_pings(clock);
        self.log_discards(now);
        if self.clock_start.is_some() && self.finished_at.is_none() {
            let step = self.rule.tick(clock);
            self.log_transition(step, clock);
        }
        self.poll_driven(now);

        for index in 0..self.peers.len() {
            self.check_in_flight(index, now);
            let join_ready = self.join_ready(index);
            let Some(peer) = self.peers[index].as_mut() else {
                continue;
            };
            let timeout = if matches!(peer.stage, Stage::Connecting | Stage::Loading) {
                timing.loading_timeout
            } else {
                timing.client_timeout
            };
            if peer.stage != Stage::Gone && peer.transport.is_gone_after(now, timeout) {
                log::info!("Match {:#x}: player {index} timed out", self.id);
                self.leave(index, now);
                continue;
            }
            let early = peer.entered_early;
            match peer.stage {
                Stage::WaitingSnapshot { since, entering } => {
                    if !join_ready {
                        continue;
                    }
                    // A client in a resync has waited since its first
                    // 0xE5, and is given a new one and a replay with its
                    // next snapshot.
                    let waited = now.saturating_duration_since(peer.resync_from.unwrap_or(since));
                    let max_age = Self::snapshot_max_age(waited, entering);
                    let confirmed = peer.time_confirmed && peer.resync_from.is_none();
                    if self.bot_waits(index) {
                        // Its wait for a snapshot begins when a real
                        // client is in.
                        if let Some(peer) = self.peers[index].as_mut() {
                            peer.stage = Stage::WaitingSnapshot {
                                since: now,
                                entering,
                            };
                        }
                    } else if let Some(snapshot) =
                        self.usable_snapshot(index, clock, max_age, entering)
                    {
                        if confirmed {
                            self.send_sync(index, snapshot, entering, now);
                        } else {
                            self.begin_timing(index, &snapshot, entering, now);
                        }
                    } else if waited >= SNAPSHOT_GIVE_UP
                        && self.admissions[index]
                            .as_ref()
                            .is_none_or(Admission::entered)
                        && let Some(snapshot) = self
                            .newest_snapshot()
                            .filter(|snapshot| self.after_preparation(index, snapshot.time))
                            .filter(|snapshot| snapshot.time + TICK_REACH_MS >= clock)
                    {
                        log::warn!(
                            "Match {:#x}: no fresh snapshot for player {index} in {} s; it \
                             starts from the one at {} ms",
                            self.id,
                            SNAPSHOT_GIVE_UP.as_secs(),
                            snapshot.time
                        );
                        if confirmed {
                            self.send_sync(index, snapshot, entering, now);
                        } else {
                            self.begin_timing(index, &snapshot, entering, now);
                        }
                    } else if let Some(at) = early {
                        if clock >= at + REJOIN_UPLOAD_AFTER_MS {
                            self.request_upload(index, now, RESYNC_REQUEST_GAP);
                        }
                    } else {
                        self.request_upload(index, now, RESYNC_REQUEST_GAP);
                    }
                }
                Stage::Timing {
                    time,
                    entering,
                    last,
                    sent,
                } => {
                    let unanswered = !peer.time_confirmed && sent >= MAX_SET_TIMES;
                    let due = now.saturating_duration_since(last) >= SET_TIME_EVERY;
                    if peer.time_confirmed || (unanswered && due) {
                        if unanswered {
                            log::warn!(
                                "Match {:#x}: player {index} does not answer its time (0xE5); its \
                                 snapshot goes all the same",
                                self.id
                            );
                            peer.time_confirmed = true;
                        }
                        if let Some(since) = peer.resync_from {
                            self.resync_timed(index, time, since, now);
                            continue;
                        }
                        // The snapshot it was timed for, if it still serves:
                        // else the next one that does.
                        let snapshot = self
                            .usable_snapshot(index, clock, FRESH_SNAPSHOT_MS, entering)
                            .filter(|snapshot| snapshot.time >= time);
                        match snapshot {
                            Some(snapshot) => self.send_sync(index, snapshot, entering, now),
                            None => {
                                if let Some(peer) = self.peers[index].as_mut() {
                                    peer.stage = Stage::WaitingSnapshot {
                                        since: now,
                                        entering,
                                    };
                                }
                            }
                        }
                    } else if due {
                        peer.stage = Stage::Timing {
                            time,
                            entering,
                            last: now,
                            sent: sent + 1,
                        };
                        self.send_set_time(index, time, entering, now);
                    }
                }
                Stage::Syncing {
                    serial,
                    time,
                    entering,
                } => {
                    let way = one_way(peer);
                    let load_by = time + RESYNC_LOAD_BY_MS + way;
                    // The inputs after its snapshot the game will have when
                    // the world reaches it (a relay, as the world, takes a
                    // way to it), and had when it loaded it, seen acked
                    // now: a way back before, the relays then a way older.
                    let fill_on_arrival = self.ring_fill(time, clock);
                    let fill_now = self.ring_fill(time, clock.saturating_sub(2 * way));
                    if self.load_proven(index) {
                        // Its game reports the world's own hashes: loaded,
                        // whatever came of the parts' acks.
                        if let Some(peer) = self.peers[index].as_mut() {
                            peer.transport.forget(serial, now);
                        }
                        log::info!(
                            "Match {:#x}: player {index} reports the right hash past its \
                             snapshot at {time} ms: its world is loaded, acked or not",
                            self.id
                        );
                        self.finish_sync(index, time, entering, now);
                        continue;
                    }
                    let Some(peer) = self.peers[index].as_mut() else {
                        continue;
                    };
                    let delivered = peer.transport.is_delivered(serial);
                    if peer.transport.is_lost(serial) {
                        // Not acked in its 1.75 s: dropped by the transport.
                        self.redo_sync(index, entering, "the snapshot did not arrive", now);
                    } else if delivered
                        && peer.resync_from.is_some()
                        && peer.slow_loads < SLOW_LOADS_REDONE
                        && fill_now > INPUT_RING
                    {
                        // Loaded, but past a ring: its catch-up lacks the
                        // first inputs after the snapshot, and its game is
                        // wrong from its first tick. Done again at once,
                        // not after its wrong reports and the wait between
                        // resyncs.
                        peer.slow_loads += 1;
                        let why = format!(
                            "its world of {time} ms was loaded with {fill_now} inputs of a player \
                             after it, more than the game keeps ({INPUT_RING}; {} of \
                             {SLOW_LOADS_REDONE} times), so again from a fresh one",
                            peer.slow_loads
                        );
                        self.redo_sync(index, entering, &why, now);
                    } else if delivered {
                        self.finish_sync(index, time, entering, now);
                    } else if peer.resync_from.is_some()
                        && peer.slow_loads < SLOW_LOADS_REDONE
                        && (clock > load_by || fill_on_arrival >= INPUT_RING_SAFE)
                    {
                        // Loaded now, the game would have lost the first
                        // inputs after the snapshot from its rings. The
                        // world on its way is of an older generation than
                        // the next 0xE5, and skipped once that is taken.
                        peer.slow_loads += 1;
                        let why = if clock > load_by {
                            format!(
                                "its world of {time} ms was not loaded {RESYNC_LOAD_BY_MS} ms \
                                 after it (at {clock} ms; {} of {SLOW_LOADS_REDONE} times), so \
                                 again from a fresh one",
                                peer.slow_loads
                            )
                        } else {
                            format!(
                                "its world of {time} ms is not loaded and a player has \
                                 {fill_on_arrival} inputs after it (the game keeps {INPUT_RING}; \
                                 {} of {SLOW_LOADS_REDONE} times), so again from a fresh one",
                                peer.slow_loads
                            )
                        };
                        self.redo_sync(index, entering, &why, now);
                    } else if peer.resync_from.is_some()
                        && peer.slow_loads == SLOW_LOADS_REDONE
                        && (clock > load_by || fill_on_arrival >= INPUT_RING_SAFE)
                    {
                        // A line that loads no world in time: the slow load
                        // is taken, once said.
                        peer.slow_loads += 1;
                        log::warn!(
                            "Match {:#x}: player {index}'s world of {time} ms was not loaded \
                             {RESYNC_LOAD_BY_MS} ms after it, after {SLOW_LOADS_REDONE} worlds \
                             done again for that; this one is waited for",
                            self.id
                        );
                    }
                }
                Stage::Live => {
                    if let Some((serial, sent, offset, takes)) = peer.clock_offset_pending
                        && peer.transport.is_delivered(serial)
                    {
                        peer.clock_offset_pending = None;
                        // Taken a way before this ack at the latest.
                        let runs = now + takes;
                        peer.correction_ends =
                            Some(peer.correction_ends.map_or(runs, |end| end.max(runs)));
                        // It went again: the game took it about a way
                        // before this ack, not when it first went.
                        let took = now
                            .checked_sub(Duration::from_millis(u64::from(one_way(peer))))
                            .unwrap_or(now)
                            .max(sent);
                        if took > sent + Duration::from_millis(u64::from(peer.round_trip)) {
                            peer.correcting_until = Some(took + takes + CORRECTION_SETTLE);
                            peer.correction = Some((took, i64::from(offset), takes));
                            peer.lags.clear();
                        }
                    }
                    if now >= peer.next_ping {
                        // Its round trip and its clock: 0.26g clients send
                        // no clock of their own.
                        peer.next_ping = now + PING_EVERY;
                        let ping = PingSent {
                            a: mono16,
                            b: peer.round_trip.min(u32::from(u16::MAX)) as u16,
                        };
                        peer.ping = Some(ping);
                        send(
                            peer,
                            &ServerMessage::Ping {
                                a: ping.a,
                                b: ping.b,
                            },
                        );
                    }
                }
                Stage::Gone => {
                    // The end, again, if the first was never acked.
                    if self.finished_at.is_some()
                        && let Some((serial, message)) = peer.farewell.take()
                    {
                        if peer.transport.is_lost(serial) {
                            // With the tick of now: the client's
                            // `leave_match` refuses a tick too far back
                            // (it closes the match all the same, 0x27C720
                            // .. 0x27C76F, but the world would keep the
                            // player).
                            let message = ServerMessage::PlayerLeft {
                                tick: farewell_tick(peer, clock),
                                player: index as u8,
                            };
                            peer.farewell = send(peer, &message).map(|serial| (serial, message));
                        } else if !peer.transport.is_delivered(serial) {
                            peer.farewell = Some((serial, message));
                        }
                    }
                }
                Stage::Connecting | Stage::Loading => {}
            }
        }

        self.check_end(clock, now);
        self.check_world_lost(now);
        self.check_bots_alone(now);
        self.check_pvb_stall(now);

        if self.clock_start.is_some()
            && self.finished_at.is_none()
            && self.next_refresh.is_none_or(|at| now >= at)
        {
            // On opposite hash phases the paired uploads are the only
            // check there is: asked for more often.
            let every = if self.hash_starved {
                timing.refresh.min(STARVED_REFRESH)
            } else {
                timing.refresh
            };
            let every = self.driven_refresh(every);
            self.next_refresh = Some(now + every);
            self.request_upload(usize::MAX, now, SNAPSHOT_WAIT.min(every));
        }
        self.send_requests(now);
        if self.inputs.len() > 4096
            && self
                .inputs
                .first()
                .is_some_and(|(at, _, _)| *at + 2 * INPUTS_KEPT_MS < clock)
        {
            let keep_from = clock.saturating_sub(INPUTS_KEPT_MS);
            self.inputs.retain(|(at, _, _)| *at >= keep_from);
        }

        let mut datagrams = Vec::new();
        for (index, slot) in self.peers.iter_mut().enumerate() {
            let Some(peer) = slot else {
                continue;
            };
            let addr = peer.addr;
            if !peer.held.is_empty() && now >= peer.held_until {
                flush_held(peer);
                peer.held_until = now + WAITING_RELAY_FLUSH;
            }
            let sent = peer.transport.poll(now);
            if let Some(recording) = self.recording.as_mut() {
                for datagram in &sent {
                    recording.datagram_out(index, datagram, now);
                }
            }
            datagrams.extend(sent.into_iter().map(|datagram| (addr, datagram)));
            if peer.stage == Stage::Gone && peer.transport.is_gone_after(now, timing.client_timeout)
            {
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

    /// The end of the match: the mode's rule, as a trusted snapshot showed
    /// it, or the time limit, as the server's copy of the clients' rule
    /// has it.
    fn check_end(&mut self, clock: u32, now: Instant) {
        let timing = self.settings.timing.clone();
        if self.finished_at.is_some() || self.clock_start.is_none() {
            return;
        }
        if self
            .mode_end_at
            .is_some_and(|at| rules::mode_rule_completed(self.game_mode(), at, clock))
        {
            // The snapshot that showed the end is the standing.
            self.finish(MatchEnd::ModeRule, now);
            return;
        }
        // The rule's own arithmetic wraps before its state began.
        let limit_reached = clock
            .checked_sub(END_GRACE_MS)
            .filter(|time| self.rule.start.is_some_and(|start| *time >= start))
            .is_some_and(|time| self.rule.completed(time));
        if self.ending.is_none() && limit_reached {
            log::info!("Match {:#x}: time is up at {clock} ms", self.id);
            self.ending = Some((now, MatchEnd::TimeLimit));
            // A last snapshot for the results, if a patched client can.
            self.requested_at = None;
        }
        if let Some((since, end)) = self.ending {
            self.request_upload(usize::MAX, now, SNAPSHOT_WAIT.min(timing.refresh));
            let fresh = self
                .state
                .as_ref()
                .is_some_and(|state| state.time + HORIZON_MS * 4 >= self.clock(since));
            if fresh
                || self.sources(usize::MAX).is_empty()
                || now.saturating_duration_since(since) >= timing.final_wait
            {
                self.finish(end, now);
            }
        }
    }

    /// The real clients that have waited [`WORLD_LOST_AFTER`] for a world
    /// to start from, if in that time no client of the match has shown a
    /// running simulation: the match's world is lost.
    ///
    /// The world of a match is in its clients' games and nowhere else. A
    /// client is started from the world of one that simulates (a fresh
    /// upload), or, with nobody simulating, from the newest world there is
    /// after its own last coming or going (`usable_snapshot`). When every
    /// game of a match has stopped, neither exists for a player who comes
    /// back: the worlds kept are from before it left, and a game that
    /// stands still gives none of the present. It would wait for as long
    /// as it stays connected, and its link keeps the match from being
    /// "left by everyone", so the lobby sends every player who presses
    /// Play back to its place in it (live, 2026-10-12, match 0x1f793b8:
    /// six players, nine and a half minutes, up to three restarts each).
    ///
    /// Not lost, and so never ended here: a match somebody plays, whatever
    /// the one who waits cannot get (hash reports, uploads and inputs show
    /// a game that simulates, a stock client's too); and a match nobody
    /// waits to get into (games that stand still for a while, a window
    /// that is not active, go on when they come back).
    fn world_lost(&self, now: Instant) -> Option<Vec<usize>> {
        if self.clock_start.is_none() || self.finished_at.is_some() || self.ending.is_some() {
            return None;
        }
        let long = |since: Instant| now.saturating_duration_since(since) >= WORLD_LOST_AFTER;
        let real = |index: &usize| !self.is_bot(*index);
        let waiting: Vec<usize> = (0..self.peers.len())
            .filter(real)
            .filter(|index| {
                self.peers[*index].as_ref().is_some_and(|peer| {
                    matches!(peer.stage, Stage::WaitingSnapshot { since, .. } if long(since))
                })
            })
            .collect();
        if waiting.is_empty() {
            return None;
        }
        let played = (0..self.peers.len()).filter(real).any(|index| {
            self.peers[index].as_ref().is_some_and(|peer| {
                peer.stage == Stage::Live && peer.simulating_at.is_some_and(|at| !long(at))
            })
        });
        (!played).then_some(waiting)
    }

    /// A match whose world is lost (`world_lost`) is over, as one that
    /// everyone left: the players who wait are told and can play another.
    fn check_world_lost(&mut self, now: Instant) {
        let Some(waiting) = self.world_lost(now) else {
            return;
        };
        log::warn!(
            "Match {:#x}: no client has simulated for {} s, and players {waiting:?} have waited \
             that long for a world to start from: the match's world is lost; it is over",
            self.id,
            WORLD_LOST_AFTER.as_secs()
        );
        self.finish(MatchEnd::Deserted, now);
    }

    /// A match with bots (test bots, or the server's own) and no real
    /// client connected for [`BOTS_ALONE`] is over: the bots never leave by
    /// themselves. A server's bot that gave its place up to a late joiner
    /// is no reason to end it: only the real clients are counted here.
    fn check_bots_alone(&mut self, now: Instant) {
        if self.finished_at.is_some() || !(0..self.players()).any(|index| self.is_bot(index)) {
            return;
        }
        let real = self.peers.iter().enumerate().any(|(index, peer)| {
            !self.is_bot(index) && peer.as_ref().is_some_and(|peer| peer.stage != Stage::Gone)
        });
        if real {
            self.bots_alone_since = None;
            return;
        }
        let since = *self.bots_alone_since.get_or_insert(now);
        if now.saturating_duration_since(since) >= BOTS_ALONE {
            log::info!(
                "Match {:#x}: only bots for {} s; it is over",
                self.id,
                BOTS_ALONE.as_secs()
            );
            self.finish(MatchEnd::Deserted, now);
        }
    }

    /// A match of one player against the server's bots
    /// ([`Formed::vs_bots`]) whose player plays (in the match, its world
    /// given) and has been asked for its world for `Timing::pvb_stall`
    /// with none uploaded is over, as one everyone left
    /// (`MatchEnd::Deserted`): the bots see the world only through that
    /// player's uploads, and a game that stands still (a window out of
    /// focus, a stalled link) would hold the match, and one of the few
    /// bots' places, to its time limit. A player in no stage to upload
    /// (loading, resynced, waiting for a world) has the other rules:
    /// `check_world_lost`, the client timeout, `check_bots_alone`.
    fn check_pvb_stall(&mut self, now: Instant) {
        if !self.formed.vs_bots || self.finished_at.is_some() || self.ending.is_some() {
            return;
        }
        let stall = self.settings.timing.pvb_stall;
        // The wait is counted in play only: a request a game did not
        // answer before it was resynced or left is no stall of the game it
        // is now.
        for peer in self.peers.iter_mut().flatten() {
            if peer.stage != Stage::Live {
                peer.unanswered_since = None;
            }
        }
        let stalled = (0..self.peers.len()).find(|&index| {
            !self.is_bot(index)
                && self.peers[index].as_ref().is_some_and(|peer| {
                    peer.stage == Stage::Live
                        && peer
                            .unanswered_since
                            .is_some_and(|since| now.saturating_duration_since(since) >= stall)
                })
        });
        if let Some(index) = stalled {
            log::warn!(
                "Match {:#x} (vs bots): player {index} was asked for its world for {} s and \
                 uploaded none; the match is over",
                self.id,
                stall.as_secs()
            );
            self.finish(MatchEnd::Deserted, now);
        }
    }

    /// Whether the players were in the match and all left it.
    fn everyone_left(&self, now: Instant) -> bool {
        self.empty_since.is_some()
            && self.first_entered.is_some_and(|first| {
                let all_came = (0..self.players()).all(|index| {
                    self.events
                        .iter()
                        .any(|(_, who, entered)| *who == index && *entered)
                });
                all_came || self.clock(now) >= first + SEATS_COME_WITHIN_MS
            })
    }

    /// The match is done: it ended and the clients had time to hear it,
    /// everyone in it left, or nobody has been in it for a while.
    pub fn is_over(&self, now: Instant) -> bool {
        self.finished_at
            .is_some_and(|at| now.saturating_duration_since(at) >= self.settings.timing.linger)
            || (self.finished_at.is_none() && self.everyone_left(now))
            || self
                .empty_since
                .is_some_and(|since| now.saturating_duration_since(since) >= ABANDONED)
    }
}

/// What becomes of a client's input for `time` that came at `clock`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Placing {
    /// Relayed at its own time.
    Keep,
    /// Relayed at another time; its owner is told (0xEF / 0xF0, 0xF4).
    Move(u32),
    /// Not relayed; its owner is told to drop it (0xF1 / 0xF2, 0xF5).
    Discard,
}

/// Where the input a client made for `time` goes. `placed` are its inputs
/// the others have (the time it gave, the time they are at); `owner_clock`
/// is where the client's own clock is at least.
///
/// What the client does with the two answers (match.md section 5.2,
/// PROVEN): 0xF1 removes its input node at that time, whole; 0xEF removes
/// it and puts the input back at the new time by the same code that takes
/// a relay (0x27D2D0 -> 0x1FAB50), so its list becomes what the others
/// get. An input is the state of the keys, so one that is dropped leaves
/// the player with the keys of its input before until its next one; one
/// that is moved is only late. So with `LateInputs::Move`:
/// - an input in time, with nothing in its way, stays;
/// - a late one goes to the first open tick (keeping its ms into the
///   tick) that is free and after every earlier input of the player. It is
///   dropped instead when a later input of the player is already at or
///   before that tick (it is stale: moved past it, it would bring old keys
///   back), when the tick is not yet in the owner's past (the client would
///   make a new input on top of the moved one, and its node would then
///   not hold the two apart), or when no such tick is within
///   [`MOVE_STEPS`];
/// - an input for the very time a moved one sits at must be moved on, to
///   the next free tick, whatever else: the owner's node there holds both
///   (its own part and the moved one), 0xEF takes its own part out and
///   leaves the other, while 0xF1 would remove both and leave the others
///   with an input the owner no longer has.
///
/// With `LateInputs::Discard` a late input is dropped, as by 0.21d. With
/// `LateInputs::Relay` it stays at its tick while it is less than `reach`
/// ms behind the match clock (every other game can still take it:
/// `Game::late_relay_reach`), and is dropped past that.
fn place(
    late_inputs: LateInputs,
    clock: u32,
    time: u32,
    placed: &VecDeque<(u32, u32)>,
    owner_clock: u32,
    reach: u32,
) -> Placing {
    let late = time + HORIZON_MS <= clock;
    if late_inputs != LateInputs::Move {
        let relayed = late_inputs == LateInputs::Relay && clock < time + reach;
        return if late && !relayed {
            Placing::Discard
        } else {
            Placing::Keep
        };
    }
    let taken = |at: u32| placed.iter().any(|(_, each)| *each == at);
    // The last of its earlier inputs, and the first of its later ones.
    let before = placed
        .iter()
        .filter(|(given, _)| *given < time)
        .map(|(_, at)| *at)
        .max();
    let after = placed
        .iter()
        .filter(|(given, _)| *given > time)
        .map(|(_, at)| *at)
        .min();
    let collides = taken(time);
    if !late && !collides && before.is_none_or(|at| at < time) {
        return Placing::Keep;
    }
    let first = (clock.saturating_sub(HORIZON_MS) / TICK_MS + 1) * TICK_MS + time % TICK_MS;
    let mut to = first;
    while to <= time || taken(to) || before.is_some_and(|at| to <= at) {
        to += TICK_MS;
    }
    if !collides
        && (after.is_some_and(|at| to >= at)
            || to + TICK_MS > owner_clock
            || to > first + MOVE_STEPS * TICK_MS)
    {
        return Placing::Discard;
    }
    Placing::Move(to)
}

/// Drop the placements too old to matter.
fn forget_placed(placed: &mut VecDeque<(u32, u32)>, clock: u32) {
    let oldest = clock.saturating_sub(INPUT_MAX_AGE_MS);
    while placed.front().is_some_and(|(_, at)| *at < oldest) {
        placed.pop_front();
    }
}

/// An upload of the tick `time` answered the request sent at match clock
/// `asked`: the client's upload offset is within `[time - asked, + 50)`.
/// Bounds that agree with the ones known narrow them; bounds that do not
/// (its clock or its link changed) replace them.
fn observe_upload(bounds: &mut Option<(i64, i64)>, asked: u32, time: u32) {
    let low = i64::from(time) - i64::from(asked);
    let high = low + i64::from(TICK_MS);
    *bounds = match *bounds {
        Some((known_low, known_high)) if low.max(known_low) < high.min(known_high) => {
            Some((low.max(known_low), high.min(known_high)))
        }
        _ => Some((low, high)),
    };
}

/// When to ask clients for their worlds so that all upload one tick: the
/// match clock for each, given the middle of each one's upload offset
/// bounds. The tick is the first all can still be asked for, and with
/// `phase` (`(period, a tick)`) one of the ticks `tick + n * period`.
/// A client asked at `tick + AIM_MS - middle` handles the request
/// [`AIM_MS`] into `tick` on its own clock if its offset is the middle.
/// `None`: no such tick within [`AIM_WAIT_MS`].
fn aim_at(clock: u32, middles: &[i64], phase: Option<(u32, u32)>) -> Option<Vec<u32>> {
    let tick = i64::from(TICK_MS);
    let clock = i64::from(clock);
    let earliest = middles.iter().map(|middle| clock + middle - AIM_MS).max()?;
    let mut target = earliest.max(0).div_euclid(tick) * tick;
    if target < earliest {
        target += tick;
    }
    if let Some((period, at)) = phase {
        let period = i64::from(period);
        if period <= tick || period % tick != 0 {
            return None;
        }
        while (target - i64::from(at)).rem_euclid(period) != 0 {
            target += tick;
        }
    }
    let times: Vec<i64> = middles
        .iter()
        .map(|middle| (target + AIM_MS - middle).max(clock))
        .collect();
    times
        .iter()
        .all(|at| *at <= clock + AIM_WAIT_MS)
        .then(|| times.into_iter().map(|at| at as u32).collect())
}

/// The tick of the 0xEA that ends the match for `peer`: the match
/// clock's, or 0 for a game that was never given a time (`Game::finish`).
fn farewell_tick(peer: &Peer, clock: u32) -> u8 {
    if peer.clock_set { tick_byte(clock) } else { 0 }
}

/// The relay of something of the player at `index`.
fn relay(index: usize, what: &Relayed) -> ServerMessage {
    match *what {
        Relayed::Input(input) => ServerMessage::InputRelay {
            player: index as u8,
            input,
        },
        Relayed::Profile { tick, value } => ServerMessage::ProfileInputRelay {
            tick,
            player: index as u8,
            value,
        },
    }
}

/// How far the client's clock is behind the match clock, from a clock it
/// sent over the round trip just measured (`peer.round_trip`): the match
/// clock at arrival less the half round trip it travelled.
fn sample_lag(peer: &mut Peer, client: u32, clock: u32, now: Instant) {
    let round_trip = peer.round_trip;
    if peer.round_trips.len() == ROUND_TRIPS_KEPT {
        peer.round_trips.pop_front();
    }
    peer.round_trips.push_back(round_trip);
    let lag = i64::from(clock) - i64::from(round_trip / 2) - i64::from(client);
    if lag.abs() > MAX_SANE_LAG_MS {
        return;
    }
    if peer.lags.len() == CLOCK_SAMPLES {
        peer.lags.pop_front();
    }
    let sample = LagSample {
        lag,
        round_trip,
        at: now,
    };
    peer.lags.push_back(sample);
    // While its game is still making a correction up, the latest sample
    // says best where its clock is; after, the best of the ones since.
    let best = best_sample(fresh(peer.lags.iter(), corrected_at(peer)));
    peer.lag_ms = Some(best.map_or(lag, |best| best.lag));
    peer.correction = None;
}

/// The samples taken since `since`: the ones before are of a clock that
/// was still being brought up or down.
fn fresh<'a>(
    samples: impl Iterator<Item = &'a LagSample> + Clone,
    since: Option<Instant>,
) -> impl Iterator<Item = &'a LagSample> + Clone {
    samples.filter(move |sample| since.is_none_or(|at| sample.at >= at))
}

/// When the client's game has made up the last correction it was sent:
/// samples from before are of a clock that was still on its way.
fn corrected_at(peer: &Peer) -> Option<Instant> {
    peer.correcting_until
        .map(|until| until.checked_sub(CORRECTION_SETTLE).unwrap_or(until))
}

/// The correction (0xE7) that the samples of a client's clock call for,
/// if any: those taken since `since`, when its game had made up the
/// correction before. `usual` is the shortest round trip its link has
/// shown of late. Two samples at least; the best of them (`best_sample`)
/// counts, and only for a lag beyond what it leaves open: its ping may
/// have been held up on one way by all that its round trip was longer than
/// the usual one, which puts the lag it shows off by half of that.
fn correction_for<'a>(
    samples: impl Iterator<Item = &'a LagSample> + Clone,
    since: Option<Instant>,
    usual: Option<u32>,
) -> Option<i16> {
    let samples = fresh(samples, since);
    if samples.clone().count() < 2 {
        return None;
    }
    let best = best_sample(samples)?;
    let usual = usual.unwrap_or(best.round_trip).min(best.round_trip);
    let unsure = i64::from((best.round_trip - usual) / 2);
    if best.lag.abs() < CLOCK_TOLERANCE_MS + unsure {
        return None;
    }
    Some(best.lag.clamp(i64::from(i16::MIN), i64::from(i16::MAX)) as i16)
}

/// Of some clock samples, the one taken over the shortest round trip, the
/// latest of them where several were: a sample is wrong by half of what
/// its ping was held up on one way more than on the other, and the
/// shortest round trip was held up least.
fn best_sample<'a>(samples: impl Iterator<Item = &'a LagSample>) -> Option<LagSample> {
    samples
        .min_by(|a, b| a.round_trip.cmp(&b.round_trip).then(b.at.cmp(&a.at)))
        .copied()
}

/// How far the client's clock is behind the match clock now: as last
/// measured, less what the correction running since has made up.
fn estimated_lag(peer: &Peer, now: Instant) -> Option<i64> {
    let lag = peer.lag_ms?;
    let Some((since, offset, takes)) = peer.correction else {
        return Some(lag);
    };
    let done = if takes.is_zero() {
        1.0
    } else {
        (now.saturating_duration_since(since).as_secs_f64() / takes.as_secs_f64()).min(1.0)
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

/// Whether a client in `stage` is being resynced while it plays: it is in
/// the others' worlds all the while.
fn resyncing(stage: Stage) -> bool {
    matches!(
        stage,
        Stage::WaitingSnapshot {
            entering: false,
            ..
        } | Stage::Timing {
            entering: false,
            ..
        } | Stage::Syncing {
            entering: false,
            ..
        }
    )
}

/// How soon a message kept for the client goes again when not acked: its
/// round trip and a little, [`RESYNC_WORLD_RESEND_MIN`] at least, the
/// table's 500 ms at most.
fn resend_pace(peer: &Peer) -> Duration {
    Duration::from_millis(u64::from(
        (peer.round_trip + RESYNC_WORLD_RESEND_MARGIN_MS).min(500),
    ))
    .max(RESYNC_WORLD_RESEND_MIN)
}

/// Whether an input of the client is taken: it simulates in the others'
/// worlds. Live; resynced while it plays (`resyncing`); or back in the
/// match (`Peer::entered_early`: the others have its player since that
/// entry) with its world on its way: the client makes inputs from its
/// load, and the ack of the world comes a way later. Live, 2026-10-15: a
/// player back in made an input 395 ms past its tick in that while, which
/// was neither relayed nor dropped.
fn takes_inputs(peer: &Peer) -> bool {
    peer.stage == Stage::Live
        || resyncing(peer.stage)
        || (peer.entered_early.is_some() && matches!(peer.stage, Stage::Syncing { .. }))
}

/// The way of a message to the client, as far as a resync's clock goes:
/// half its round trip, at most [`RESYNC_ONE_WAY_MAX_MS`].
fn one_way(peer: &Peer) -> u32 {
    (peer.round_trip / 2).min(RESYNC_ONE_WAY_MAX_MS)
}

/// The way to the client for a resynced world's age limit: half its usual
/// (median) round trip, not capped at [`RESYNC_ONE_WAY_MAX_MS`] as the
/// clock's [`one_way`] is (with that cap the limit's floor never applied),
/// with round trips over [`RESYNC_TRIP_MAX_MS`] taken as that.
fn trip_one_way(peer: &Peer) -> u32 {
    age_limit_way(&peer.round_trips, peer.round_trip)
}

fn age_limit_way(round_trips: &VecDeque<u32>, last: u32) -> u32 {
    usual_round_trip(round_trips, last).min(RESYNC_TRIP_MAX_MS) / 2
}

/// How old a resynced client's snapshot may be when its world goes, the
/// way to it being `one_way`: [`RESYNC_WORLD_MAX_AGE_MS`] at the load,
/// and never less than [`RESYNC_WORLD_MIN_AGE_MS`] now.
fn resync_world_max_age(one_way: u32) -> u32 {
    RESYNC_WORLD_MAX_AGE_MS
        .saturating_sub(one_way)
        .max(RESYNC_WORLD_MIN_AGE_MS)
}

impl Answer {
    /// To a player input at `time` (0xF1/0xF2, or 0xEF/0xF0 to `moved_to`).
    fn input(message: ServerMessage, time: u32, moved_to: Option<u32>) -> Self {
        Self {
            message,
            time,
            moved_to,
            tracked: true,
            serial: None,
        }
    }

    /// To a profile input at `time` (0xF5, or 0xF4 to `moved_to`).
    fn profile(message: ServerMessage, time: u32, moved_to: Option<u32>) -> Self {
        Self {
            tracked: false,
            ..Self::input(message, time, moved_to)
        }
    }
}

/// Tell a client what became of an input of its own, or hold that from the
/// first 0xE5 of its resync until its world is loaded (`Peer::resync_from`,
/// match.md section 30): its history is cleared, and whether it takes a
/// drop or a move then is not settled; after the load it does. A sent
/// answer is remembered until acked: if the client's resync begins before
/// it arrives, it goes again after the load.
fn answer_owner(peer: &mut Peer, mut answer: Answer, now: Instant) {
    if peer.resync_from.is_some() {
        // Its inputs on its new clock are in time: these are mostly the
        // few it made before, come late.
        if peer.held_answers.len() < IN_FLIGHT_KEPT {
            peer.held_answers.push(answer);
        }
        return;
    }
    answer.serial = send_drop_order(peer, &answer.message, now);
    let Some(serial) = answer.serial else {
        return;
    };
    if answer.tracked {
        track(
            peer,
            serial,
            Flight::Discard,
            answer.moved_to.unwrap_or(answer.time),
        );
    }
    let transport = &peer.transport;
    peer.sent_answers.retain(|sent| {
        sent.serial
            .is_some_and(|serial| !transport.is_delivered(serial))
    });
    if peer.sent_answers.len() == ANSWERS_KEPT {
        peer.sent_answers.pop_front();
    }
    peer.sent_answers.push_back(answer);
}

/// A message the world of the client's resync waits for (`Peer::sync_pending`).
/// A relay past [`SYNC_PENDING_RELAYS`] is not tracked.
fn gate(peer: &mut Peer, serial: u64, now: Instant, tick: u32, kind: GateKind) {
    let entry = Gated {
        serial,
        sent: now,
        tick,
        kind,
    };
    if kind == GateKind::Relay {
        if peer.sync_relays.len() >= SYNC_PENDING_RELAYS {
            return;
        }
        peer.sync_relays.push(entry);
    } else {
        peer.sync_pending.push(entry);
    }
    let size = peer.sync_pending.len() + peer.sync_relays.len();
    peer.sync_pending_most = peer.sync_pending_most.max(size);
}

/// The acked entries of the client's gate go: each is one O(1) look.
fn prune_gate(peer: &mut Peer) {
    let transport = &peer.transport;
    peer.sync_pending
        .retain(|entry| !transport.is_delivered(entry.serial));
    peer.sync_relays
        .retain(|entry| !transport.is_delivered(entry.serial));
}

/// The gate of the client's resync is emptied, for a new generation or
/// with its world gone: what was not acked of it and is for a tick up to
/// `upto` (all, if none) is forgotten by the transport (in the world, or
/// in the next generation's replay: a copy that came after would be moved
/// by the game). What is past `upto` stays kept until acked. Returns how
/// many were forgotten.
fn clear_gate(peer: &mut Peer, upto: Option<u32>, now: Instant) -> usize {
    let mut forgotten = 0;
    for entry in peer.sync_pending.drain(..).chain(peer.sync_relays.drain(..)) {
        if upto.is_none_or(|time| entry.tick <= time)
            && !peer.transport.is_delivered(entry.serial)
        {
            peer.transport.forget(entry.serial, now);
            forgotten += 1;
        }
    }
    peer.gate_since = None;
    peer.gate_checked = None;
    forgotten
}

/// Remember a message sent to the client, to see what becomes of it.
fn track(peer: &mut Peer, serial: u64, flight: Flight, time: u32) {
    track_sent(peer, serial, flight, time, None);
}

/// As [`track`], with when it was first sent (queued).
fn track_sent(peer: &mut Peer, serial: u64, flight: Flight, time: u32, sent: Option<Instant>) {
    if peer.in_flight.len() == IN_FLIGHT_KEPT {
        peer.in_flight.pop_front();
    }
    peer.in_flight.push_back(InFlight {
        serial,
        flight,
        time,
        sent,
    });
}

/// The shortest round trip the client's line has shown lately: no copy of
/// a message reaches it sooner than half of it.
fn shortest_round_trip(peer: &Peer) -> u32 {
    peer.round_trips
        .iter()
        .copied()
        .min()
        .map_or(peer.round_trip, |least| least.min(peer.round_trip))
}

/// The clock of a game that took a 0xE5 of its sync, now. It follows the
/// last copy it received, which is not known here: the latest clock any
/// copy of the generation gives, so that what is put past it is never
/// behind it.
fn given_clock(set_times: &VecDeque<SetTimeSent>, now: Instant) -> Option<u32> {
    set_times
        .iter()
        .map(|sent| sent.start + now.saturating_duration_since(sent.at).as_millis() as u32)
        .max()
}

fn input_summary(peer: &Peer) -> String {
    format!(
        "{} inputs, {} first seen over {LATE_INPUT_MS} ms late on the way, {} too late to relay \
         and discarded, {} moved, {} refused for their rotation, the latest {} ms, resynced {} \
         times, started once more after its first world {} times, round trip {} ms, {} messages \
         to it sent again, our seq skipped ahead {} times, {} of {} relays and roster changes \
         never acked, {} relayed late at their tick",
        peer.inputs_received,
        peer.inputs_late,
        peer.inputs_discarded,
        peer.inputs_moved,
        peer.inputs_refused,
        peer.latest_input_ms,
        peer.resyncs,
        peer.reloads,
        peer.round_trip,
        peer.transport.resent(),
        peer.transport.skips(),
        peer.relays_lost,
        peer.relays_lost + peer.relays_acked,
        peer.inputs_relayed_late
    )
}

/// When a relay for the match time `time`, queued at match clock `clock`,
/// stops going to a game whose usual round trip is `round_trip` ms
/// ([`usual_round_trip`]): its last copy must reach the game before the
/// game's world is [`RELAY_USEFUL_MS`] past the tick, so half a round trip
/// (50 ms at the least) before that on the match clock, which a live
/// game's clock keeps to. One copy goes in any case, and the transport
/// keeps a relay that went for the table's 250 ms from its first send at
/// least.
fn relay_deadline(round_trip: u32, clock: u32, time: u32, now: Instant) -> Instant {
    let way = (round_trip / 2).max(50);
    let left = (time + RELAY_USEFUL_MS).saturating_sub(clock + way).max(50);
    now + Duration::from_millis(u64::from(left))
}

/// The median of a client's latest round trips ([`ROUND_TRIPS_KEPT`]), or
/// its `last` before there are any: on a spiky line one long sample would
/// otherwise cut every relay of the next second short.
fn usual_round_trip(latest: &VecDeque<u32>, last: u32) -> u32 {
    let mut round_trips: Vec<u32> = latest.iter().copied().collect();
    if round_trips.is_empty() {
        return last;
    }
    round_trips.sort_unstable();
    round_trips[round_trips.len() / 2]
}

/// As [`send`], given up at `until` (`Transport::send_until`).
fn send_until(peer: &mut Peer, message: &ServerMessage, until: Instant) -> Option<u64> {
    match peer
        .transport
        .send_until(message.id(), &message.payload(), until)
    {
        Ok(serial) => Some(serial),
        Err(SendError::NotConnected) => None,
        Err(error) => {
            log::warn!(
                "Can't send {:#04x} to {}: {error:?}",
                message.id(),
                peer.addr
            );
            None
        }
    }
}

/// A drop order for the player's own input, for [`DROP_ORDER_LIFE`].
fn send_drop_order(peer: &mut Peer, message: &ServerMessage, now: Instant) -> Option<u64> {
    send_until(peer, message, now + DROP_ORDER_LIFE)
}

/// As [`send`], resent until acked whatever the message's own lifetime.
fn send_kept(peer: &mut Peer, message: &ServerMessage) -> Option<u64> {
    peer.transport
        .send_kept(message.id(), &message.payload())
        .ok()
}

/// As [`send_kept`], for a game that waits for its first world: sent again
/// at the pace of its acks ([`WAITING_RELAY_RESEND`]).
fn send_waiting(peer: &mut Peer, message: &ServerMessage) -> Option<u64> {
    peer.transport
        .send_kept_every(message.id(), &message.payload(), WAITING_RELAY_RESEND)
        .ok()
}

/// The inputs held for a waiting game go out: queued together, the
/// transport puts as many in a datagram as fit.
fn flush_held(peer: &mut Peer) {
    for message in std::mem::take(&mut peer.held) {
        send_waiting(peer, &message);
    }
}

/// How long the first world of a game is kept on its way, when that is
/// longer than 0xE8's own 1.75 s: `wanted` (`Timing::entry_state`), but
/// not past the moment the match clock is [`WAITING_REACH_MS`] ahead of
/// the clock the game was given (`start`, with the match at `clock`). An
/// input relayed after that would be read at a tick 12.8 s back, before
/// the world, and never played.
fn entry_lifetime(wanted: Duration, clock: u32, start: u32) -> Option<Duration> {
    let reach = WAITING_REACH_MS.saturating_sub(clock.saturating_sub(start));
    let kept = wanted.min(Duration::from_millis(u64::from(reach)));
    (kept > WORLD_LIFETIME).then_some(kept)
}

/// 0xE8 with its payload as it goes on the wire, for the table's 1.75 s or
/// for `lifetime`; its parts again every `resend` rather than the table's
/// 500 ms.
fn send_world(
    peer: &mut Peer,
    payload: &[u8],
    lifetime: Option<Duration>,
    resend: Option<Duration>,
) -> Option<u64> {
    let id = survarium::v026g::match_msgs::id::SNAPSHOT;
    let sent = match (lifetime, resend) {
        (Some(lifetime), _) => peer.transport.send_lasting(id, payload, lifetime),
        (None, Some(resend)) => peer.transport.send_every(id, payload, resend),
        (None, None) => peer.transport.send(id, payload),
    };
    match sent {
        Ok(serial) => Some(serial),
        // It is leaving: nothing more reaches it.
        Err(SendError::NotConnected) => None,
        Err(error) => {
            log::warn!("Can't send {id:#04x} to {}: {error:?}", peer.addr);
            None
        }
    }
}

fn send(peer: &mut Peer, message: &ServerMessage) -> Option<u64> {
    match peer.transport.send(message.id(), &message.payload()) {
        Ok(serial) => Some(serial),
        // It is leaving: nothing more reaches it.
        Err(SendError::NotConnected) => None,
        Err(error) => {
            log::warn!(
                "Can't send {:#04x} to {}: {error:?}",
                message.id(),
                peer.addr
            );
            None
        }
    }
}

#[cfg(test)]
impl Game {
    /// For tests: the stage of each place, as a letter.
    pub fn stages(&self) -> String {
        self.peers
            .iter()
            .map(|peer| match peer.as_ref().map(|peer| peer.stage) {
                None => '-',
                Some(Stage::Connecting) => 'c',
                Some(Stage::Loading) => 'l',
                Some(Stage::WaitingSnapshot { .. }) => 'w',
                Some(Stage::Timing { .. }) => 't',
                Some(Stage::Syncing { .. }) => 's',
                Some(Stage::Live) => 'L',
                Some(Stage::Gone) => 'g',
            })
            .collect()
    }

    pub fn match_clock(&self, now: Instant) -> u32 {
        self.clock(now)
    }

    pub fn generation(&self, index: usize) -> Option<u8> {
        self.peers[index].as_ref().map(|peer| peer.generation.0)
    }

    pub fn resyncs(&self, index: usize) -> u32 {
        self.peers[index].as_ref().map_or(0, |peer| peer.resyncs)
    }

    /// The roster changes so far: when, who, entered or left.
    pub fn events(&self) -> &[(u32, usize, bool)] {
        &self.events
    }

    /// How often it was started once more right after its first world
    /// (`reload`).
    pub fn reloads(&self, index: usize) -> u32 {
        self.peers[index].as_ref().map_or(0, |peer| peer.reloads)
    }

    /// How many of its inputs were not relayed for their rotation.
    pub fn inputs_refused(&self, index: usize) -> u32 {
        self.peers[index]
            .as_ref()
            .map_or(0, |peer| peer.inputs_refused)
    }

    pub fn rule_state(&self) -> u8 {
        self.rule.state
    }

    /// The time and the standing of the newest trusted snapshot.
    pub fn observed(&self) -> Option<(u32, Option<Standing>)> {
        self.state
            .as_ref()
            .map(|state| (state.time, state.standing))
    }

    pub fn has_clock(&self) -> bool {
        self.clock_start.is_some()
    }

    /// Clients report hashes and no tick has two of them (opposite hash
    /// phases), for [`HASH_STARVED_AFTER`] now.
    pub fn hash_starved(&self) -> bool {
        self.hash_starved
    }

    /// How often that was logged.
    pub fn hash_starved_warnings(&self) -> u32 {
        self.hash_starved_warnings
    }

    /// The snapshot time a resync of the client starts from at the least.
    pub fn resync_floor(&self, index: usize) -> u32 {
        self.peers[index]
            .as_ref()
            .map_or(0, |peer| peer.resync_floor)
    }

    /// The bounds known on a client's upload offset.
    pub fn upload_offset(&self, index: usize) -> Option<(i64, i64)> {
        self.peers[index]
            .as_ref()
            .and_then(|peer| peer.upload_offset)
    }

    /// How many clients vouched for the newest trusted snapshot.
    pub fn trusted_by(&self) -> u8 {
        self.state.as_ref().map_or(0, |state| state.vouchers)
    }

    pub fn set_clock_correction(&mut self, on: bool) {
        self.settings.clock_correction = on;
    }

    /// Whether the client is in a resync (`Peer::resync_from`), and
    /// whether its replay went.
    pub fn resync_state(&self, index: usize) -> (bool, bool) {
        self.peers[index].as_ref().map_or((false, false), |peer| {
            (peer.resync_from.is_some(), peer.replayed.is_some())
        })
    }

    /// The time of the snapshot the client is being started from, while it
    /// is timed or loading.
    pub fn sync_time(&self, index: usize) -> Option<u32> {
        match self.peers[index].as_ref()?.stage {
            Stage::Timing { time, .. } | Stage::Syncing { time, .. } => Some(time),
            _ => None,
        }
    }

    /// The gate of the client's resync: the entries it holds now, the most
    /// it held in this match, and how often it was looked at.
    pub fn gate_stats(&self, index: usize) -> (usize, usize, u64) {
        self.peers[index].as_ref().map_or((0, 0, 0), |peer| {
            (
                peer.sync_pending.len() + peer.sync_relays.len(),
                peer.sync_pending_most,
                peer.gate_checks,
            )
        })
    }

    /// The worlds the client was asked for as it was found out of step,
    /// and whether one of them is still awaited (`ask_wrong_world`).
    pub fn wrong_worlds(&self, index: usize) -> (u32, bool) {
        self.peers[index].as_ref().map_or((0, false), |peer| {
            (peer.wrong_worlds, peer.wrong_world_asked.is_some())
        })
    }

    /// As if the client's resync had been done again for its replay
    /// `redos` times.
    #[cfg(test)]
    pub fn set_replay_redos(&mut self, index: usize, redos: u32) {
        if let Some(peer) = self.peers[index].as_mut() {
            peer.replay_redos = redos;
        }
    }

    /// Worlds of the client's resync done again for a slow load.
    pub fn slow_loads(&self, index: usize) -> u32 {
        self.peers[index].as_ref().map_or(0, |peer| peer.slow_loads)
    }

    /// The time of the newest snapshot there is.
    pub fn newest_snapshot_time(&self) -> Option<u32> {
        self.newest_snapshot().map(|snapshot| snapshot.time)
    }

    /// How many hash reports of the client are kept for judging.
    pub fn hash_reports_of(&self, index: usize) -> usize {
        self.hashes
            .values()
            .filter(|reports| reports.contains_key(&index))
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Clock samples a second apart: (the lag each shows, its round trip).
    fn samples(taken: &[(i64, u32)]) -> Vec<LagSample> {
        let start = Instant::now();
        taken
            .iter()
            .enumerate()
            .map(|(second, &(lag, round_trip))| LagSample {
                lag,
                round_trip,
                at: start + Duration::from_secs(second as u64),
            })
            .collect()
    }

    #[test]
    fn a_clock_is_corrected_by_the_sample_of_the_shortest_round_trip() {
        // A clock that is behind, on a steady link: the latest of the
        // samples that tie counts.
        let behind = samples(&[(465, 196), (470, 200), (460, 196)]);
        assert_eq!(correction_for(behind.iter(), None, Some(195)), Some(460));
        // Ahead as well.
        let ahead = samples(&[(-98, 197), (-100, 196)]);
        assert_eq!(correction_for(ahead.iter(), None, Some(196)), Some(-100));
        // Within the tolerance, or one sample alone: nothing.
        let right = samples(&[(4, 196), (-9, 197)]);
        assert_eq!(correction_for(right.iter(), None, Some(196)), None);
        assert_eq!(correction_for(behind[..1].iter(), None, Some(195)), None);
        assert_eq!(correction_for([].iter(), None, None), None);
        // More than a 0xE7 holds.
        let far = samples(&[(40_000, 20), (40_000, 20)]);
        assert_eq!(correction_for(far.iter(), None, Some(20)), Some(i16::MAX));
        // A sample from before the game had made the last correction up is
        // of a clock still on its way: it does not count, whatever its
        // round trip, and neither does it make two of one.
        let moving = samples(&[(240, 190), (130, 191), (20, 196), (22, 197)]);
        assert_eq!(
            correction_for(moving.iter(), Some(moving[2].at), Some(190)),
            Some(20)
        );
        assert_eq!(
            correction_for(moving.iter(), Some(moving[3].at), Some(190)),
            None
        );
        assert_eq!(correction_for(moving.iter(), None, Some(190)), Some(240));
    }

    #[test]
    fn a_ping_held_up_on_one_way_does_not_turn_a_clock_back() {
        // Live, 2026-10-14: a link of 196 ms and one round trip of 665.
        // The ping was 469 ms longer on its way there, the sample reads
        // 234 ms ahead, and as the smallest lag of the four it had the
        // game told to slow its clock by 238 ms.
        let spike = samples(&[(12, 196), (8, 197), (-238, 665), (15, 198)]);
        assert_eq!(correction_for(spike.iter(), None, Some(195)), Some(12));
        let quiet = samples(&[(2, 196), (8, 197), (-238, 665), (5, 198)]);
        assert_eq!(correction_for(quiet.iter(), None, Some(195)), None);
        // Held up on its way back it reads as far behind: not that either.
        let late = samples(&[(3, 196), (236, 665), (-4, 197)]);
        assert_eq!(correction_for(late.iter(), None, Some(195)), None);
        // Nothing but samples over long round trips (the link stalls):
        // what they show is within what they leave open, and waits.
        let stall = samples(&[(-238, 665), (-183, 560)]);
        assert_eq!(correction_for(stall.iter(), None, Some(195)), None);
        // A lag beyond that is real however long the round trip was.
        let real = samples(&[(-600, 665), (-590, 560)]);
        assert_eq!(correction_for(real.iter(), None, Some(195)), Some(-590));
        // With no round trip known from before, the samples' shortest is
        // the usual one.
        assert_eq!(correction_for(stall.iter(), None, None), Some(-183));
    }

    #[test]
    fn one_long_round_trip_does_not_cut_the_relays_short() {
        let now = Instant::now();
        let left = |until: Instant| until.duration_since(now).as_millis();
        // A line of 80 ms with one sample of 900 ms, the latest.
        let mut latest: VecDeque<u32> = [80, 85, 78, 82, 90, 81, 79].into();
        latest.push_back(900);
        let usual = usual_round_trip(&latest, 900);
        assert_eq!(usual, 82);
        // An input on time (its tick 100 ms behind the match clock): 400
        // ms less 50, not cut to the one copy of 50 ms.
        assert_eq!(left(relay_deadline(usual, 10_100, 10_000, now)), 350);
        assert_eq!(left(relay_deadline(900, 10_100, 10_000, now)), 50);
        // Before any sample, the last round trip; a slow line all along
        // does shorten it.
        assert_eq!(usual_round_trip(&VecDeque::new(), 120), 120);
        let slow: VecDeque<u32> = [400, 420, 380].into();
        assert_eq!(
            left(relay_deadline(
                usual_round_trip(&slow, 400),
                10_100,
                10_000,
                now
            )),
            200
        );
    }

    #[test]
    fn ticks_round_up_to_the_grid() {
        assert_eq!(grid_up(0), 0);
        assert_eq!(grid_up(1), 50);
        assert_eq!(grid_up(50), 50);
        assert_eq!(grid_up(51), 100);
    }

    #[test]
    fn a_first_world_is_not_kept_past_the_reach_of_the_ticks() {
        let secs = Duration::from_secs;
        let ms = Duration::from_millis;
        // A fresh snapshot: the game's clock is given the match's.
        assert_eq!(entry_lifetime(secs(4), 60_000, 60_000), Some(secs(4)));
        // The oldest that is fresh (1.5 s, the clock 350 ms past it).
        assert_eq!(entry_lifetime(secs(4), 60_000, 58_850), Some(secs(4)));
        assert_eq!(entry_lifetime(secs(5), 60_000, 58_850), Some(ms(4_350)));
        // An old one, taken when no fresh one came: the game's clock
        // stands 4.65 s behind, and 127 ticks from it are 6.35 s. Then
        // 0xE8's own 1.75 s, as before.
        assert_eq!(entry_lifetime(secs(4), 60_000, 55_350), None);
        assert_eq!(entry_lifetime(secs(4), 60_000, 57_000), Some(ms(2_500)));
        // Every relay sent while the world is on its way is within 127
        // ticks of the game's clock, with 850 ms for an input ahead of
        // the match clock.
        for behind in (0..5_000).step_by(50) {
            let kept = entry_lifetime(secs(5), 60_000, 60_000 - behind).unwrap_or(WORLD_LIFETIME);
            let last = behind + kept.as_millis() as u32 + 850;
            assert!(last <= 127 * TICK_MS || kept == WORLD_LIFETIME, "{behind}");
        }
    }

    #[test]
    fn a_late_input_is_dropped_or_moved_to_the_first_open_tick() {
        let none = VecDeque::new();
        let at = |clock: u32, time: u32, placed: &VecDeque<(u32, u32)>| {
            place(LateInputs::Move, clock, time, placed, clock, 0)
        };
        // In time: stays, with either setting.
        assert_eq!(at(1_000, 800, &none), Placing::Keep);
        assert_eq!(
            place(LateInputs::Discard, 1_000, 800, &none, 1_000, 0),
            Placing::Keep
        );
        // At 1000 ms the ticks up to 750 are final: 800 is the first open.
        assert_eq!(
            place(LateInputs::Discard, 1_000, 750, &none, 1_000, 0),
            Placing::Discard
        );
        assert_eq!(at(1_000, 600, &none), Placing::Move(800));
        assert_eq!(at(1_049, 600, &none), Placing::Move(800));
        assert_eq!(at(1_050, 600, &none), Placing::Move(850));
        // A precise input keeps its ms into the tick.
        assert_eq!(at(1_000, 623, &none), Placing::Move(823));
        // Two late ones in a row keep their order.
        let one = VecDeque::from([(600, 800)]);
        assert_eq!(at(1_000, 650, &one), Placing::Move(850));
        // A later input of the player is in already: the late one is stale.
        let later = VecDeque::from([(800, 800)]);
        assert_eq!(at(1_000, 600, &later), Placing::Discard);
        let later = VecDeque::from([(900, 900)]);
        assert_eq!(at(1_000, 600, &later), Placing::Move(800));
        // The owner's clock runs behind (just resynced): the open tick is
        // not in its past, where it could still make an input.
        assert_eq!(
            place(LateInputs::Move, 1_000, 600, &none, 820, 0),
            Placing::Discard
        );
        assert_eq!(
            place(LateInputs::Move, 1_000, 600, &none, 850, 0),
            Placing::Move(800)
        );
        // Too many of its ticks taken: dropped.
        let many: VecDeque<(u32, u32)> = (0..6).map(|step| (400 + step, 800 + step * 50)).collect();
        assert_eq!(at(1_000, 600, &many), Placing::Discard);
    }

    #[test]
    fn an_input_for_the_time_a_moved_one_sits_at_is_moved_on() {
        // 600 was moved to 800; the player's own input for 800 comes in
        // time, and must not stay there, nor be dropped.
        let moved = VecDeque::from([(600, 800)]);
        assert_eq!(
            place(LateInputs::Move, 1_000, 800, &moved, 1_000, 0),
            Placing::Move(850)
        );
        // Also when the owner's clock is behind, or a later one is in.
        assert_eq!(
            place(LateInputs::Move, 1_000, 800, &moved, 700, 0),
            Placing::Move(850)
        );
        let chain = VecDeque::from([(600, 800), (850, 850)]);
        assert_eq!(
            place(LateInputs::Move, 1_000, 800, &chain, 1_000, 0),
            Placing::Move(900)
        );
        // An input in time that an earlier, moved one went past follows it.
        let past = VecDeque::from([(600, 850)]);
        assert_eq!(
            place(LateInputs::Move, 1_050, 830, &past, 1_050, 0),
            Placing::Move(880)
        );
    }

    #[test]
    fn uploads_bound_a_clients_tick_and_requests_are_aimed_into_it() {
        // Asked at 10 000 it uploaded 9 450: its offset is in [-550, -500).
        let mut bounds = None;
        observe_upload(&mut bounds, 10_000, 9_450);
        assert_eq!(bounds, Some((-550, -500)));
        // Asked at 10 020 it uploaded 9 500: [-520, -470) narrows it.
        observe_upload(&mut bounds, 10_020, 9_500);
        assert_eq!(bounds, Some((-520, -500)));
        // Bounds that cannot both hold: its clock moved; the new ones count.
        observe_upload(&mut bounds, 20_000, 19_600);
        assert_eq!(bounds, Some((-400, -350)));

        // Two clients, offsets about -510 and -470: asked for one tick, the
        // first both can still be asked for, each by its own middle.
        let times = aim_at(10_000, &[-510, -470], None).unwrap();
        assert_eq!(times, [10_082, 10_042]);
        for (at, middle) in times.iter().zip([-510i64, -470]) {
            assert_eq!((i64::from(*at) + middle).div_euclid(50) * 50, 9_550);
            assert_eq!((i64::from(*at) + middle).rem_euclid(50), AIM_MS);
        }
        // One source, the other client hashing every 100 ms on the ticks
        // ending in 50: the tick asked for is one of those.
        let times = aim_at(10_000, &[-510], Some((100, 8_950))).unwrap();
        assert_eq!(times, [10_082]);
        assert_eq!(
            aim_at(10_000, &[-510], Some((100, 8_900))).unwrap(),
            [10_032]
        );
        // A client that reports every tick has no phase to aim at.
        assert_eq!(aim_at(10_000, &[-510], Some((50, 8_900))), None);
        // Bounds that would mean a wait of seconds are not used.
        assert_eq!(aim_at(10_000, &[-510, -3_000], None), None);
    }

    #[test]
    fn an_aimed_request_misses_its_tick_once_at_most() {
        // Whatever the true offset within the first 50 ms of bounds, and
        // the server late by up to 5 ms with each request: one request
        // misses at most, and the bounds always hold the truth.
        for truth in -550i64..-500 {
            let mut bounds = Some((-550i64, -500i64));
            let mut misses = 0;
            for (round, late) in [4i64, 0, 3, 5, 1, 4, 2, 0, 5].into_iter().enumerate() {
                let clock = 10_000 + round as u32 * 3_000;
                let (low, high) = bounds.unwrap();
                let middle = (low + high) / 2;
                let planned = i64::from(aim_at(clock, &[middle], None).unwrap()[0]);
                let wanted = (planned + middle).div_euclid(50) * 50;
                let at = planned + late;
                let uploaded = (at + truth).div_euclid(50) * 50;
                misses += u32::from(uploaded != wanted);
                observe_upload(&mut bounds, at as u32, uploaded as u32);
                let (low, high) = bounds.unwrap();
                assert!(low <= truth && truth < high, "{truth}: {bounds:?}");
            }
            assert!(misses <= 1, "{truth}: {misses} misses");
        }
    }

    #[test]
    fn a_resynced_clock_is_the_latest_any_copy_of_the_0xe5_gives() {
        // Two copies of one generation whose clocks disagree: the first
        // went at 10 000 ms, the second 150 ms on with a clock 50 ms
        // behind the first's. The game follows whichever it took last,
        // and the server cannot tell which: 150 ms after the second, the
        // first gives 10 300 ms and the second 10 250 ms.
        let at = Instant::now();
        let copies: VecDeque<SetTimeSent> = [
            SetTimeSent {
                echo: 1,
                at,
                start: 10_000,
                ends_correction: false,
            },
            SetTimeSent {
                echo: 2,
                at: at + Duration::from_millis(150),
                start: 10_100,
                ends_correction: false,
            },
        ]
        .into();
        let now = at + Duration::from_millis(300);
        // Not the last copy sent (it would put the clock behind the game's
        // if the game took the first one last) ...
        assert_eq!(given_clock(&copies, now), Some(10_300));
        // ... and either way round.
        let reversed: VecDeque<SetTimeSent> = copies.iter().rev().copied().collect();
        assert_eq!(given_clock(&reversed, now), Some(10_300));
        assert_eq!(given_clock(&VecDeque::new(), now), None);
    }

    #[test]
    fn a_resync_worlds_age_limit_leaves_room_for_the_way_to_the_game() {
        assert_eq!(resync_world_max_age(0), 1_000);
        assert_eq!(resync_world_max_age(150), 850);
        assert_eq!(resync_world_max_age(250), 750);
        // A line too long for any snapshot to be loaded in time: the
        // freshest one there is still goes.
        assert_eq!(resync_world_max_age(700), RESYNC_WORLD_MIN_AGE_MS);
    }

    #[test]
    fn the_age_limit_counts_the_whole_way_of_a_long_line() {
        let trips = |ms: &[u32]| ms.iter().copied().collect::<VecDeque<u32>>();
        // 600 ms of round trip: 300 ms of way, not the clock's 250.
        let way = age_limit_way(&trips(&[600; 8]), 600);
        assert_eq!(way, 300);
        assert_eq!(resync_world_max_age(way), 700);
        // The median: one spike does not count.
        assert_eq!(age_limit_way(&trips(&[600, 600, 3_000]), 3_000), 300);
        // Over 1000 ms of round trip the 500 ms floor applies, which it
        // never did with the way capped at 250 ms.
        let way = age_limit_way(&trips(&[1_200; 4]), 1_200);
        assert_eq!(way, 600);
        assert_eq!(resync_world_max_age(way), RESYNC_WORLD_MIN_AGE_MS);
        // A line beyond reason counts as RESYNC_TRIP_MAX_MS.
        assert_eq!(age_limit_way(&trips(&[9_000; 4]), 9_000), RESYNC_TRIP_MAX_MS / 2);
        // No samples yet: the last round trip.
        assert_eq!(age_limit_way(&VecDeque::new(), 640), 320);
    }
}
