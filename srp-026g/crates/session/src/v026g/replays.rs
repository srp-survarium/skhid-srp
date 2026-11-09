//! What the lobby and the match server share of the replays, in memory:
//! the replay sessions ([`super::replay_watch`], `docs/notes/replay-stream.md`)
//! and what a list row says of a match's recording ([`Replays::state`]).
//!
//! - The match server's recordings' writer (`SRP_RECORD_DIR`) tells of each
//!   file it keeps, drops under the minimum of players or deletes for the
//!   storage cap ([`Replays::recording_kept`], ...).
//! - Which games may play a recording: the simulation changes
//!   ([`SIMULATION_CHANGES`], [`oldest_replayable`]) and [`refusal_of`].
//! - Save (`research/local-replays-design.md` section 1): a player asks to
//!   save a match's replay to a file ([`Replays::save_ask`], the lobby's
//!   `R2` answers, [`Replays::save_answer`]). The match server makes the
//!   file from the kept recording on a thread of its own, one at a time
//!   (`match_server::v026g::replay`): `SRP_REPLAY_DIR/<id:08x>.srpreplay`
//!   (`replay_file`'s container: a small header and the `.sur` the replay
//!   sessions play, nothing private) and its sidecar `<id:08x>.json`
//!   ([`Sidecar`]: size, SHA-256, each place's revision and account). A
//!   file made is kept for later askers ([`SaveSettings::max_replay_bytes`]
//!   together, and as long as its recording). A file there is given a
//!   download token ([`parse_token`]), which the lobby sends the game in a
//!   hidden `9E` link and the updater fetches from the web page's
//!   `/replays/<token>` ([`Replays::take_download`]) into the player's
//!   `<game>\replays`. Who may follows `SRP_REPLAY_WATCH`, as for a replay
//!   session. Tokens live in memory only: a restart ends them all.

use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use password_hash::rand_core::{OsRng, RngCore};

use super::replay_watch::{WatchSettings, Watches};
use crate::LockExt;

/// The `start` of the `3B` that polls for the answer to a replay
/// session's ask; the `player` of a `3D` that asks to save a match's
/// replay, and the `start` of the `3B` that polls for its answer.
pub use survarium::v026g::lobby::{REPLAY_START, SAVE_ASK, SAVE_START};
/// A game is given replays from this patch revision on (`old_game`
/// below it).
pub const MIN_REVISION: u8 = 14;
/// The patch revisions that changed what a match computes, oldest first
/// (`simulation_changes.tsv` beside this file, which says how a PR adds
/// one): a match of games older than the last one plays out otherwise in
/// a newer game [LIKELY: not tried], so its recording is not given to one
/// ([`oldest_replayable`]).
pub const SIMULATION_CHANGES: &str = include_str!("simulation_changes.tsv");

/// A row of [`SIMULATION_CHANGES`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SimulationChange<'a> {
    /// `None`: `next`, the revision of the release that ships it.
    pub revision: Option<u8>,
    pub what: &'a str,
}

/// The rows of a simulation changes list, or what is wrong with it: a
/// revision is 1 to 255, above the row before it, and `next` is the last
/// row and the only one.
pub fn simulation_changes(text: &str) -> Result<Vec<SimulationChange<'_>>, String> {
    let mut rows: Vec<SimulationChange<'_>> = Vec::new();
    for (n, line) in text.lines().enumerate() {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let (revision, what) = line
            .split_once('\t')
            .ok_or_else(|| format!("line {}: no tab after its revision", n + 1))?;
        if what.trim().is_empty() {
            return Err(format!("line {}: it does not say what changed", n + 1));
        }
        if rows.last().is_some_and(|row| row.revision.is_none()) {
            return Err(format!("line {}: a row after `next`", n + 1));
        }
        let revision = match revision {
            "next" => None,
            number => Some(
                number
                    .parse::<u8>()
                    .ok()
                    .filter(|&r| r > 0)
                    .ok_or_else(|| format!("line {}: `{number}` is no revision", n + 1))?,
            ),
        };
        if let (Some(revision), Some(Some(before))) =
            (revision, rows.last().map(|row| row.revision))
            && revision <= before
        {
            return Err(format!("line {}: {revision} is not above {before}", n + 1));
        }
        rows.push(SimulationChange { revision, what });
    }
    if rows.is_empty() {
        return Err("no row".to_owned());
    }
    Ok(rows)
}

/// The oldest patch revision whose recordings are given to a game: the
/// last row of [`SIMULATION_CHANGES`]. `next` is the revision of the
/// release that ships it, which is the server's `SRP_MIN_PATCH_REVISION`
/// (`min_patch_revision`; the release sets it at its swap); with none (0:
/// a server of no release), the row before it. Never below that row.
pub fn oldest_replayable(changes: &[SimulationChange<'_>], min_patch_revision: u8) -> u8 {
    let last_number = changes
        .iter()
        .rev()
        .find_map(|row| row.revision)
        .unwrap_or(0);
    match changes.last() {
        Some(SimulationChange { revision: None, .. }) if min_patch_revision > 0 => {
            min_patch_revision.max(last_number)
        }
        _ => last_number,
    }
}

/// [`oldest_replayable`] of [`SIMULATION_CHANGES`] (a list a test reads:
/// `the_simulation_changes_list_reads`).
pub fn oldest_replayable_here(min_patch_revision: u8) -> u8 {
    let changes = simulation_changes(SIMULATION_CHANGES).expect("simulation_changes.tsv reads");
    oldest_replayable(&changes, min_patch_revision)
}

/// How long a save's download token lives.
pub const TOKEN_LIFETIME: Duration = Duration::from_secs(10 * 60);
/// The downloads one token makes (a broken download resumes with the next).
pub const TOKEN_DOWNLOADS: u8 = 5;
/// Live tokens of one account at most.
pub const TOKENS_PER_ACCOUNT: usize = 3;
/// One save ask of an account in this long; another is answered `busy`.
pub const ASK_EVERY: Duration = Duration::from_secs(2);
/// Files waiting to be made at most, one for each player of a full match
/// (a job is a few seconds); one asked for past them is `busy`.
pub const MAX_JOBS: usize = 20;
/// `SRP_REPLAY_MAX_MB` when unset: the saved files (and their sidecars)
/// over this together go, the oldest first. One that went is made again
/// from its recording when someone asks for it.
pub const DEFAULT_MAX_REPLAY_MB: u64 = 1_000;
/// A file that failed for a reason that can change (a full disk) is not
/// tried again for this long at first, twice as long each time it fails
/// again, up to [`FAILED_FOR_AT_MOST`].
const FAILED_FOR: Duration = Duration::from_secs(10 * 60);
const FAILED_FOR_AT_MOST: Duration = Duration::from_secs(4 * 60 * 60);
/// A `preparing` save is looked at again at most this often for one
/// account: the movie polls once a second, but a crafted game may send
/// dozens of polls a datagram, and each look reads the sidecar.
pub const RECHECK_EVERY: Duration = Duration::from_secs(1);
/// A save's answer this old is forgotten.
const ASK_KEPT: Duration = Duration::from_secs(15 * 60);
/// The recordings' folder is listed at most this often for the list rows;
/// the writer's news of a file (kept, dropped, deleted) has it listed anew.
const LISTING_KEPT: Duration = Duration::from_secs(5);
/// Match ids remembered as not kept or deleted, at most.
const REMEMBERED: usize = 4096;

/// The status word of an `R1` answer (`CONTRACT-R1`): what came of the
/// player's latest ask.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// Asked, being read (a save: being made): the movie polls on.
    Preparing,
    /// A save's file is there: its link is set (`9E` goes just before the
    /// answer).
    Ready,
    /// No ask from this player.
    None,
    /// The recording was deleted (the storage cap) or is not found.
    Gone,
    /// The asker may not watch that match (`SRP_REPLAY_WATCH=match`).
    NotYours,
    /// The asker's game is older than [`MIN_REVISION`].
    OldGame,
    /// The recording was made by games this one cannot play.
    Incompatible,
    /// The player is in a match or queued, or too many sessions run; a
    /// save: too many files to make or tokens now, or asked too fast.
    Busy,
    /// The recording could not be read.
    Failed,
    /// Replays are switched off on this server.
    Off,
    /// A replay session ([`super::replay_watch`]): its game was sent to
    /// watch the match in the running client.
    Watching,
}

impl Status {
    /// The word the `R1` row carries.
    pub fn word(self) -> &'static str {
        match self {
            Self::Preparing => "preparing",
            Self::Ready => "ready",
            Self::None => "none",
            Self::Gone => "gone",
            Self::NotYours => "not_yours",
            Self::OldGame => "old_game",
            Self::Incompatible => "incompatible",
            Self::Busy => "busy",
            Self::Failed => "failed",
            Self::Off => "off",
            Self::Watching => "watching",
        }
    }
}

/// The match id of a file name of `dir` that is `<8 lower hex><suffix>`,
/// and nothing else.
pub fn match_of_name(name: &str, suffix: &str) -> Option<u32> {
    let stem = name.strip_suffix(suffix)?;
    if stem.len() != 8 || !stem.bytes().all(|c| matches!(c, b'0'..=b'9' | b'a'..=b'f')) {
        return None;
    }
    u32::from_str_radix(stem, 16).ok()
}

/// `SRP_REPLAY_DIR`, `SRP_REPLAY_MAX_MB`: where Save's files are made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SaveSettings {
    /// Only files named `<8 hex>.srpreplay`, `<8 hex>.json` and their
    /// `.part` are ever written or deleted there, and only
    /// `<8 hex>.srpreplay` is ever served. Never the recordings' folder,
    /// inside it or around it ([`Self::checked`]).
    pub replay_dir: PathBuf,
    /// The files and sidecars over this many bytes together: the oldest go.
    pub max_replay_bytes: u64,
}

/// Why the folders cannot be used for Save.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FolderError {
    /// The replay folder is the recordings' folder, inside it or holds it:
    /// a recording must never sit where files are served from.
    Overlap,
    /// A folder cannot be made or read.
    Io(String),
}

impl std::fmt::Display for FolderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Overlap => write!(
                f,
                "SRP_REPLAY_DIR must be a folder of its own, not SRP_RECORD_DIR, inside it or \
                 around it"
            ),
            Self::Io(error) => write!(f, "{error}"),
        }
    }
}

impl SaveSettings {
    /// What the environment asks for, beside the recordings of
    /// `record_dir`; `None` (Save off, said in the log) when
    /// `SRP_REPLAY_DIR` is not set (or `off`) or overlaps `record_dir`.
    pub fn from_env(record_dir: &Path) -> Option<Self> {
        let var = |name: &str| {
            std::env::var(name)
                .ok()
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty() && !value.eq_ignore_ascii_case("off"))
        };
        let replay_dir = var("SRP_REPLAY_DIR")?;
        let max_mb = match var("SRP_REPLAY_MAX_MB") {
            None => DEFAULT_MAX_REPLAY_MB,
            Some(value) => value
                .parse::<u64>()
                .ok()
                .filter(|&mb| mb <= u64::MAX >> 20)
                .unwrap_or_else(|| {
                    log::error!(
                        "SRP_REPLAY_MAX_MB is `{value}`; it must be a number. Using \
                         {DEFAULT_MAX_REPLAY_MB}"
                    );
                    DEFAULT_MAX_REPLAY_MB
                }),
        };
        let settings = Self {
            replay_dir: replay_dir.into(),
            max_replay_bytes: max_mb << 20,
        };
        match settings.checked(record_dir) {
            Ok(settings) => Some(settings),
            Err(error) => {
                log::error!("Save is off: {error}");
                None
            }
        }
    }

    /// The settings with the replay folder made and its real path, unless
    /// it and the recordings' folder overlap.
    pub fn checked(self, record_dir: &Path) -> Result<Self, FolderError> {
        let real = |dir: &Path| {
            std::fs::create_dir_all(dir)
                .and_then(|()| std::fs::canonicalize(dir))
                .map_err(|error| FolderError::Io(format!("{}: {error}", dir.display())))
        };
        let replay_dir = real(&self.replay_dir)?;
        let record_dir = real(record_dir)?;
        if replay_dir.starts_with(&record_dir) || record_dir.starts_with(&replay_dir) {
            return Err(FolderError::Overlap);
        }
        Ok(Self { replay_dir, ..self })
    }
}

/// `<id:08x>`: the one form a match id takes in a saved file's name.
pub fn file_stem(match_id: u32) -> String {
    format!("{match_id:08x}")
}

/// The extension of Save's files, here and in the player's folder.
pub const FILE_EXTENSION: &str = "srpreplay";

/// What the sidecar `<id>.json` says of a saved file ([`Sidecar::to_json`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sidecar {
    pub match_id: u32,
    pub size: u64,
    /// 64 lower-case hex digits.
    pub sha256: String,
    /// By place: the patch revision each place's game connected with (the
    /// highest, if it connected again), `None` for one that never did.
    pub revisions: Vec<Option<u8>>,
    /// Every place's account, late joiners too.
    pub account_ids: Vec<u32>,
    /// Unix seconds.
    pub made_at: u64,
}

impl Sidecar {
    pub fn to_json(&self) -> String {
        serde_json::json!({
            "match_id": file_stem(self.match_id),
            "size": self.size,
            "sha256": self.sha256,
            "revisions": self.revisions,
            "account_ids": self.account_ids,
            "made_at": self.made_at,
        })
        .to_string()
    }

    pub fn parse(text: &str) -> Option<Self> {
        let value: serde_json::Value = serde_json::from_str(text).ok()?;
        let match_id = u32::from_str_radix(value["match_id"].as_str()?, 16).ok()?;
        let sha256 = value["sha256"].as_str()?.to_owned();
        if sha256.len() != 64
            || !sha256
                .bytes()
                .all(|c| matches!(c, b'0'..=b'9' | b'a'..=b'f'))
        {
            return None;
        }
        Some(Self {
            match_id,
            size: value["size"].as_u64()?,
            sha256,
            revisions: value["revisions"]
                .as_array()?
                .iter()
                .map(|revision| revision.as_u64().and_then(|r| u8::try_from(r).ok()))
                .collect(),
            account_ids: value["account_ids"]
                .as_array()?
                .iter()
                .filter_map(|id| u32::try_from(id.as_u64()?).ok())
                .collect(),
            made_at: value["made_at"].as_u64().unwrap_or(0),
        })
    }
}

/// What a `ready` save answer carries: the link's parts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ready {
    /// 32 lower-case hex digits.
    pub token: String,
    pub sha256: String,
    pub size: u64,
}

/// The player's latest save answer ([`Replays::save_answer`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Answer {
    pub status: Status,
    /// The match asked for; 0 with [`Status::None`].
    pub match_id: u32,
    /// With [`Status::Preparing`], the file's place in the line: 1 being
    /// made now, 2 with one before it, ...; 0 when not known (its recording
    /// is still being written). Else 0.
    pub place: u32,
    /// With [`Status::Ready`].
    pub ready: Option<Ready>,
}

impl Answer {
    /// The last field of the `R2` row: the place in line while
    /// `preparing`, the file's size when `ready`, else 0.
    pub fn figure(&self) -> u64 {
        match &self.ready {
            Some(ready) => ready.size,
            None if self.status == Status::Preparing => u64::from(self.place),
            None => 0,
        }
    }

    /// The text of the `9E` that goes before a `ready` answer:
    /// `replay <id 8 hex> <token 32 hex> <sha256 64 hex> <size>`, the one
    /// form patch revision 14 passes to the updater (`client_patch.md`
    /// section 3.27).
    pub fn link(&self) -> Option<String> {
        let ready = self.ready.as_ref()?;
        Some(format!(
            "replay {} {} {} {}",
            file_stem(self.match_id),
            ready.token,
            ready.sha256,
            ready.size
        ))
    }
}

struct Token {
    account_id: u32,
    match_id: u32,
    until: Instant,
    downloads_left: u8,
    /// A download of it is under way ([`Download`]): one at a time. The
    /// token is kept while it is, even past its time, so that it still
    /// counts among its account's [`TOKENS_PER_ACCOUNT`].
    downloading: bool,
}

impl Token {
    /// Within its time, with downloads left.
    fn live(&self, now: Instant) -> bool {
        now < self.until && self.downloads_left > 0
    }
}

/// An account's latest save ask.
struct Ask {
    match_id: u32,
    at: Instant,
    status: Status,
    ready: Option<Ready>,
    /// Kept for looking again while it is `preparing`.
    revision: u8,
    /// Whether the asker played it, while the lobby keeps its report.
    played: Option<bool>,
    /// When it was last looked at; `None`: at the next poll (the writer or
    /// the job has news of its match).
    checked: Option<Instant>,
}

/// Why making a file failed ([`Replays::job_done`]): when it is tried again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    /// Reading or writing failed (a full disk): tried again after
    /// [`FAILED_FOR`], twice as long each time it fails again, up to
    /// [`FAILED_FOR_AT_MOST`].
    Disk,
    /// The recording itself (too large, damaged, of no known map, or the
    /// conversion's own error): it does not change, so it is not tried
    /// again while the server runs.
    Lasting,
}

/// A failed file: answered `failed` until `until` (`None`: for good).
struct Failed {
    until: Option<Instant>,
    /// Its [`Failure::Disk`] failures in a row.
    times: u32,
}

/// Why `/replays/<token>` gets no download ([`Replays::take_download`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refused {
    /// Not a live token (malformed, unknown, past its time or downloads).
    Unknown,
    /// A live token whose one download at a time is under way.
    Busy,
}

/// A download of a token under way: the token's one at a time, until this
/// is dropped.
pub struct Download<'a> {
    replays: &'a Replays,
    token: [u8; 16],
    /// The match whose file the token downloads.
    pub match_id: u32,
}

impl Drop for Download<'_> {
    fn drop(&mut self) {
        if let Some(token) = self.replays.inner.locked().tokens.get_mut(&self.token) {
            token.downloading = false;
        }
    }
}

/// The first six hex digits of a token: all a log line ever shows of one.
pub fn token_tag(token: &str) -> &str {
    token.get(..6).unwrap_or("")
}

/// A token's 16 bytes, from exactly 32 lower-case hex digits.
pub fn parse_token(text: &str) -> Option<[u8; 16]> {
    if text.len() != 32 {
        return None;
    }
    let mut bytes = [0u8; 16];
    for (byte, pair) in bytes.iter_mut().zip(text.as_bytes().chunks(2)) {
        let digit = |c: u8| match c {
            b'0'..=b'9' => Some(c - b'0'),
            b'a'..=b'f' => Some(c - b'a' + 10),
            _ => None,
        };
        *byte = digit(pair[0])? << 4 | digit(pair[1])?;
    }
    Some(bytes)
}

fn hex(bytes: &[u8]) -> String {
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(text, "{byte:02x}");
    }
    text
}

/// A match for good: the stem of its recording's file,
/// `rec026g_<UTC YYYYMMDD-HHMMSS it was made>_<match id:08x>`, whether it
/// was recorded or not. The match server's id alone comes again after a
/// restart (it is random from each start); with the second it was made it
/// does not.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ReplayId {
    match_id: u32,
    stem: String,
}

impl ReplayId {
    const PREFIX: &str = "rec026g_";

    /// The match `match_id`, made at `made`.
    pub fn new(match_id: u32, made: SystemTime) -> Self {
        Self {
            match_id,
            stem: format!("{}{}_{match_id:08x}", Self::PREFIX, utc_stamp(made)),
        }
    }

    /// `rec026g_<8 digits>-<6 digits>_<8 lower hex>`, and nothing else.
    pub fn parse(text: &str) -> Option<Self> {
        let rest = text.strip_prefix(Self::PREFIX)?;
        let (stamp, id) = rest.split_once('_')?;
        let digits =
            |part: &str, len: usize| part.len() == len && part.bytes().all(|c| c.is_ascii_digit());
        let (date, time) = stamp.split_once('-')?;
        if !digits(date, 8) || !digits(time, 6) {
            return None;
        }
        Some(Self {
            match_id: match_of_name(id, "")?,
            stem: text.to_owned(),
        })
    }

    /// The match server's id.
    pub fn match_id(&self) -> u32 {
        self.match_id
    }

    pub fn as_str(&self) -> &str {
        &self.stem
    }

    /// When its recording was made: its name's UTC second.
    pub fn made(&self) -> SystemTime {
        let at = Self::PREFIX.len();
        let digits = |from: usize, len: usize| {
            self.stem
                .get(at + from..at + from + len)
                .and_then(|text| text.parse::<i64>().ok())
                .unwrap_or(0)
        };
        let (year, month, day) = (digits(0, 4), digits(4, 2), digits(6, 2));
        // A civil date to days since 1970-01-01 (H. Hinnant's algorithm).
        let y = if month <= 2 { year - 1 } else { year };
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let doy = (153 * ((month + 9) % 12) + 2) / 5 + day - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        let days = era * 146_097 + doe - 719_468;
        let secs = days * 86_400 + digits(9, 2) * 3_600 + digits(11, 2) * 60 + digits(13, 2);
        UNIX_EPOCH + Duration::from_secs(u64::try_from(secs).unwrap_or(0))
    }
}

/// `YYYYMMDD-HHMMSS`, UTC.
fn utc_stamp(time: SystemTime) -> String {
    let secs = time
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    let (days, rest) = (secs / 86_400, secs % 86_400);
    // Days since 1970-01-01 to a civil date (H. Hinnant's algorithm).
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}{month:02}{day:02}-{:02}{:02}{:02}",
        rest / 3_600,
        rest % 3_600 / 60,
        rest % 60
    )
}

/// Why a game of `revision` may not play a recording made by games of
/// `revisions` (by place), if it may not: every revision the recording's
/// games had must be `oldest` (the oldest replayable, [`oldest_replayable`])
/// or later and no later than the asker's (a newer game's match may play
/// out otherwise in an older one). 0 (a session the server did not know)
/// and places that never connected are not counted; a recording with no
/// revision at all (made before 2026-10-15, when the revisions began to be
/// kept) is not played (the owner, 2026-10-17: its games' revision is not
/// guessed).
pub fn refusal_of(revisions: &[Option<u8>], revision: u8, oldest: u8) -> Option<Refusal> {
    let known = || {
        revisions
            .iter()
            .flatten()
            .copied()
            .filter(|&known| known != 0)
    };
    let (Some(from), Some(to)) = (known().min(), known().max()) else {
        return Some(Refusal::Unknown);
    };
    if from < oldest {
        Some(Refusal::Older { from, to, oldest })
    } else if to > revision {
        Some(Refusal::Newer { from, to })
    } else {
        None
    }
}

/// Why a recording is not played to a game ([`refusal_of`]): its status
/// is [`Status::Incompatible`], and the lobby tells the player why.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// No game of the recording has a known revision: recorded before the
    /// revisions were kept.
    Unknown,
    /// Recorded by games of revisions `from` to `to`, below `oldest`, the
    /// oldest whose recordings the server gives.
    Older { from: u8, to: u8, oldest: u8 },
    /// Recorded by games newer than the asker's.
    Newer { from: u8, to: u8 },
}

/// One listing of the recordings' folder: the kept recordings' matches,
/// and those of recordings still being written.
struct Listing {
    at: Instant,
    kept: HashSet<u32>,
    writing: HashSet<u32>,
}

#[derive(Default)]
struct Inner {
    /// Recordings closed under the minimum of players, and recordings the
    /// storage cap deleted, the newest last.
    not_kept: VecDeque<u32>,
    deleted: VecDeque<u32>,
    /// The latest listing of the recordings' folder ([`LISTING_KEPT`]).
    listing: Option<Listing>,
    /// Save: the download tokens, each account's latest ask, the files
    /// waiting to be made and the one being made, and the failed ones
    /// (bounded by the kept recordings: a deleted one's entry goes).
    tokens: HashMap<[u8; 16], Token>,
    asks: HashMap<u32, Ask>,
    jobs: VecDeque<u32>,
    making: Option<u32>,
    failed: HashMap<u32, Failed>,
    stopped: bool,
    /// Save asks looked at, and folder listings made: for the tests.
    #[cfg(test)]
    evaluations: usize,
    #[cfg(test)]
    listings: usize,
}

impl Inner {
    /// The writer or the job has news of a match: its save asks are looked
    /// at at their next poll, with the folder listed anew.
    fn news_of(&mut self, match_id: u32) {
        self.listing = None;
        for ask in self.asks.values_mut() {
            if ask.match_id == match_id {
                ask.checked = None;
            }
        }
    }

    /// Tokens past their time or downloads go, but not while downloading.
    fn prune_tokens(&mut self, now: Instant) {
        self.tokens
            .retain(|_, token| token.downloading || token.live(now));
    }
}

/// What the recordings' folder has of a match ([`Replays::recorded`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Recorded {
    Kept,
    Writing,
    No,
}

/// What a list row says of a match's replay ([`Replays::state`]); its
/// number on the wire is [`ReplayState::code`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ReplayState {
    /// No recording kept: never recorded, or closed under the minimum of
    /// players.
    NotRecorded = 0,
    /// The recording is kept (or still being written).
    Kept = 1,
    /// The storage cap deleted the recording.
    Gone = 3,
    /// The asker's game is older than [`MIN_REVISION`].
    OldVersion = 4,
}

impl ReplayState {
    pub fn code(self) -> u8 {
        self as u8
    }
}

fn remember(list: &mut VecDeque<u32>, match_id: u32) {
    list.retain(|&id| id != match_id);
    if list.len() >= REMEMBERED {
        list.pop_front();
    }
    list.push_back(match_id);
}

/// The replays: one for the srp process, shared by the lobby, the match
/// server and the web page.
pub struct Replays {
    /// `SRP_RECORD_DIR`; `None`: recordings are off.
    record_dir: Option<PathBuf>,
    /// Save's folder (`SRP_REPLAY_DIR`); `None`: Save is off, every save ask
    /// is answered `off`.
    save: Option<SaveSettings>,
    /// The oldest patch revision whose recordings are given
    /// ([`oldest_replayable`]).
    oldest: u8,
    inner: Mutex<Inner>,
    /// Wakes the job thread ([`Replays::next_job`]).
    work: Condvar,
    /// The replay sessions ([`super::replay_watch`]).
    watches: Watches,
}

/// One listing of the recordings' folder: regular files named
/// `rec026g_<UTC start>_<id:08x>.srprec`, and `.srprec.part`.
fn list_recordings(dir: &Path, now: Instant) -> Listing {
    let mut listing = Listing {
        at: now,
        kept: HashSet::new(),
        writing: HashSet::new(),
    };
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let name = entry.file_name();
        let Some((_, id)) = name
            .to_str()
            .and_then(|name| name.strip_prefix("rec026g_"))
            .and_then(|stem| stem.rsplit_once('_'))
        else {
            continue;
        };
        if !entry.file_type().is_ok_and(|kind| kind.is_file()) {
            continue;
        }
        if let Some(match_id) = match_of_name(id, ".srprec") {
            listing.kept.insert(match_id);
        } else if let Some(match_id) = match_of_name(id, ".srprec.part") {
            listing.writing.insert(match_id);
        }
    }
    listing
}

impl Replays {
    /// The recordings of `record_dir` (`None`: off, every row says
    /// [`ReplayState::NotRecorded`]); replay sessions off until
    /// [`Self::with_watch`].
    pub fn new(record_dir: Option<PathBuf>) -> Self {
        Self {
            record_dir,
            save: None,
            oldest: oldest_replayable_here(0),
            inner: Mutex::new(Inner::default()),
            work: Condvar::new(),
            watches: Watches::new(None),
        }
    }

    /// Save on, with this folder (recordings on: else it stays off).
    pub fn with_save(mut self, settings: SaveSettings) -> Self {
        if self.record_dir.is_some() {
            self.save = Some(settings);
        }
        self
    }

    /// Save's folder, when Save is on.
    pub fn save_settings(&self) -> Option<&SaveSettings> {
        self.save.as_ref()
    }

    /// The recordings' folder, when recordings are on.
    pub fn record_dir(&self) -> Option<&Path> {
        self.record_dir.as_deref()
    }

    /// Replay sessions on, with these settings.
    pub fn with_watch(mut self, settings: WatchSettings) -> Self {
        self.watches = Watches::new(Some(settings));
        self
    }

    /// The replay sessions.
    pub fn watches(&self) -> &Watches {
        &self.watches
    }

    /// The oldest patch revision whose recordings are given.
    pub fn oldest(&self) -> u8 {
        self.oldest
    }

    /// The oldest patch revision whose recordings are given: the server's
    /// [`oldest_replayable_here`] (else the list's last number).
    pub fn with_oldest_revision(mut self, oldest: u8) -> Self {
        self.oldest = oldest;
        self
    }

    /// Whether the recordings' folder has a match's kept recording
    /// (`rec026g_<UTC start>_<id:08x>.srprec`, a regular file) or one still
    /// being written (`.srprec.part`), by a listing at most
    /// [`LISTING_KEPT`] old: the folder holds a file for every match kept,
    /// and is not read for each row.
    fn recorded(&self, inner: &mut Inner, match_id: u32, now: Instant) -> Recorded {
        let Some(record_dir) = &self.record_dir else {
            return Recorded::No;
        };
        let fresh = inner
            .listing
            .as_ref()
            .is_some_and(|listing| now.saturating_duration_since(listing.at) < LISTING_KEPT);
        if !fresh {
            #[cfg(test)]
            {
                inner.listings += 1;
            }
            inner.listing = Some(list_recordings(record_dir, now));
        }
        let listing = inner.listing.as_ref().expect("listed");
        if listing.kept.contains(&match_id) {
            Recorded::Kept
        } else if listing.writing.contains(&match_id) {
            Recorded::Writing
        } else {
            Recorded::No
        }
    }

    /// What a list row says of the match's replay, for a game of
    /// `revision` (`None`: not known, never [`ReplayState::OldVersion`]).
    /// A recording the cap deleted is [`ReplayState::Gone`] only while this
    /// process remembers it.
    pub fn state(&self, match_id: u32, revision: Option<u8>, now: Instant) -> ReplayState {
        let too_old = revision.is_some_and(|r| r < MIN_REVISION);
        let mut inner = self.inner.locked();
        match self.recorded(&mut inner, match_id, now) {
            Recorded::Kept | Recorded::Writing if too_old => ReplayState::OldVersion,
            Recorded::Kept | Recorded::Writing => ReplayState::Kept,
            Recorded::No if inner.deleted.contains(&match_id) => ReplayState::Gone,
            Recorded::No => ReplayState::NotRecorded,
        }
    }

    /// The match server kept a match's recording. Its file is made when a
    /// player first asks to save it ([`Self::save_ask`]), not now: most
    /// matches are never saved, and making one takes the CPU the matches
    /// share.
    pub fn recording_kept(&self, match_id: u32) {
        let mut inner = self.inner.locked();
        inner.not_kept.retain(|&id| id != match_id);
        inner.deleted.retain(|&id| id != match_id);
        inner.failed.remove(&match_id);
        inner.news_of(match_id);
    }

    /// A match's recording was closed under the minimum of players.
    pub fn recording_not_kept(&self, match_id: u32) {
        let mut inner = self.inner.locked();
        remember(&mut inner.not_kept, match_id);
        inner.news_of(match_id);
    }

    /// The storage cap deleted a match's recording: its saved file goes too
    /// (a file lives as long as its recording).
    pub fn recording_deleted(&self, match_id: u32) {
        {
            let mut inner = self.inner.locked();
            remember(&mut inner.deleted, match_id);
            inner.failed.remove(&match_id);
            inner.news_of(match_id);
        }
        self.delete_file(match_id);
    }

    // ------------------------------------------------------- Save

    /// `SRP_REPLAY_DIR/<id:08x>.srpreplay`: the only file `/replays/`
    /// serves, its name made here from the match id alone.
    pub fn file_path(&self, match_id: u32) -> Option<PathBuf> {
        let dir = &self.save.as_ref()?.replay_dir;
        Some(dir.join(format!("{}.{FILE_EXTENSION}", file_stem(match_id))))
    }

    /// Its sidecar: never served.
    pub fn json_path(&self, match_id: u32) -> Option<PathBuf> {
        let dir = &self.save.as_ref()?.replay_dir;
        Some(dir.join(format!("{}.json", file_stem(match_id))))
    }

    /// The sidecar of a file that is there (a regular file of the size the
    /// sidecar says).
    fn made(&self, match_id: u32) -> Option<Sidecar> {
        let sidecar = Sidecar::parse(&std::fs::read_to_string(self.json_path(match_id)?).ok()?)?;
        let meta = std::fs::symlink_metadata(self.file_path(match_id)?).ok()?;
        (sidecar.match_id == match_id && meta.is_file() && meta.len() == sidecar.size)
            .then_some(sidecar)
    }

    /// A match's place in the line: 1 being made, 2 with one before it,
    /// ...; 0 when it is not in the line.
    fn place_in_line(inner: &Inner, match_id: u32) -> u32 {
        inner
            .making
            .iter()
            .chain(inner.jobs.iter())
            .position(|&id| id == match_id)
            .map_or(0, |at| at as u32 + 1)
    }

    /// A player asks to save a match's replay (`3D` with [`SAVE_ASK`]).
    /// `revision`: its game's patch revision, if known; `played`: whether
    /// it played the match, while the lobby keeps its report. The status is
    /// kept for its polls.
    pub fn save_ask(
        &self,
        account_id: u32,
        match_id: u32,
        revision: Option<u8>,
        played: Option<bool>,
        now: Instant,
    ) -> Status {
        let mut inner = self.inner.locked();
        inner
            .asks
            .retain(|_, ask| now.saturating_duration_since(ask.at) < ASK_KEPT);
        let revision = revision.unwrap_or(0);
        if let Some(last) = inner.asks.get(&account_id)
            && now.saturating_duration_since(last.at) < ASK_EVERY
        {
            if last.match_id == match_id {
                // The same ask twice: it stands.
                return last.status;
            }
            let at = last.at;
            inner.asks.insert(
                account_id,
                Ask {
                    match_id,
                    at,
                    status: Status::Busy,
                    ready: None,
                    revision,
                    played,
                    checked: Some(at),
                },
            );
            return Status::Busy;
        }
        let (status, ready) =
            self.evaluate(&mut inner, account_id, match_id, revision, played, now);
        inner.asks.insert(
            account_id,
            Ask {
                match_id,
                at: now,
                status,
                ready,
                revision,
                played,
                checked: Some(now),
            },
        );
        status
    }

    /// The player's latest save answer (`3B` with [`SAVE_START`]): its own
    /// and no other's. One that was `preparing` is looked at again, once in
    /// [`RECHECK_EVERY`] at most (or at once after news of its match);
    /// polls in between get the same answer, the place in line made anew.
    pub fn save_answer(&self, account_id: u32, now: Instant) -> Answer {
        let mut inner = self.inner.locked();
        let Some(ask) = inner.asks.get(&account_id) else {
            return Answer {
                status: if self.save.is_some() {
                    Status::None
                } else {
                    Status::Off
                },
                match_id: 0,
                place: 0,
                ready: None,
            };
        };
        let (match_id, status, revision, played) =
            (ask.match_id, ask.status, ask.revision, ask.played);
        let due = ask
            .checked
            .is_none_or(|at| now.saturating_duration_since(at) >= RECHECK_EVERY);
        if status == Status::Preparing && due {
            let (status, ready) =
                self.evaluate(&mut inner, account_id, match_id, revision, played, now);
            if let Some(ask) = inner.asks.get_mut(&account_id) {
                ask.status = status;
                ask.ready = ready;
                ask.checked = Some(now);
            }
        }
        let ask = &inner.asks[&account_id];
        Answer {
            status: ask.status,
            match_id: ask.match_id,
            place: if ask.status == Status::Preparing {
                Self::place_in_line(&inner, ask.match_id)
            } else {
                0
            },
            ready: ask.ready.clone(),
        }
    }

    /// [`Self::look`], the why of a [`Status::Incompatible`] logged.
    fn evaluate(
        &self,
        inner: &mut Inner,
        account_id: u32,
        match_id: u32,
        revision: u8,
        played: Option<bool>,
        now: Instant,
    ) -> (Status, Option<Ready>) {
        match self.look(inner, account_id, match_id, revision, played, now) {
            Ok((status, ready)) => (status, ready),
            Err(refusal) => {
                log::info!(
                    "Save: account {account_id} is not given match {match_id:#x} for its game \
                     revision {revision}: {refusal:?} (the oldest given: {})",
                    self.oldest
                );
                (Status::Incompatible, None)
            }
        }
    }

    /// What a save ask gets: its status (and the link's parts when ready),
    /// or why the file made is not for the asker's game.
    fn look(
        &self,
        inner: &mut Inner,
        account_id: u32,
        match_id: u32,
        revision: u8,
        played: Option<bool>,
        now: Instant,
    ) -> Result<(Status, Option<Ready>), Refusal> {
        #[cfg(test)]
        {
            inner.evaluations += 1;
        }
        if self.save.is_none() {
            return Ok((Status::Off, None));
        }
        // Revision 14 is the first whose game hands the link to the updater.
        if revision < MIN_REVISION {
            return Ok((Status::OldGame, None));
        }
        // Who may (`SRP_REPLAY_WATCH`, anyone by default), as for a replay
        // session.
        let may_watch = |played: bool| {
            self.watches
                .settings()
                .is_none_or(|settings| settings.may_watch(account_id, played))
        };
        if played == Some(false) && !may_watch(false) {
            return Ok((Status::NotYours, None));
        }
        if let Some(sidecar) = self.made(match_id) {
            if !may_watch(sidecar.account_ids.contains(&account_id)) {
                return Ok((Status::NotYours, None));
            }
            if let Some(refusal) = refusal_of(&sidecar.revisions, revision, self.oldest) {
                return Err(refusal);
            }
            return Ok(match self.issue(inner, account_id, match_id, now) {
                Some(token) => {
                    log::info!(
                        "Save: account {account_id} is given match {match_id:#x} ({} bytes), \
                         token {}...",
                        sidecar.size,
                        token_tag(&token)
                    );
                    (
                        Status::Ready,
                        Some(Ready {
                            token,
                            sha256: sidecar.sha256,
                            size: sidecar.size,
                        }),
                    )
                }
                None => (Status::Busy, None),
            });
        }
        // Asked for already: every asker of the match waits for the one job.
        if inner.making == Some(match_id) || inner.jobs.contains(&match_id) {
            return Ok((Status::Preparing, None));
        }
        if inner
            .failed
            .get(&match_id)
            .is_some_and(|failed| failed.until.is_none_or(|until| now < until))
        {
            return Ok((Status::Failed, None));
        }
        if inner.deleted.contains(&match_id) || inner.not_kept.contains(&match_id) {
            return Ok((Status::Gone, None));
        }
        Ok(match self.recorded(inner, match_id, now) {
            Recorded::Kept => {
                if inner.jobs.len() >= MAX_JOBS {
                    return Ok((Status::Busy, None));
                }
                // Made when first asked for, not when its match ends.
                inner.jobs.push_back(match_id);
                self.work.notify_all();
                log::info!(
                    "Save: match {match_id:#x} is made for account {account_id} (place {} in \
                     line)",
                    Self::place_in_line(inner, match_id)
                );
                (Status::Preparing, None)
            }
            // Being written: it is kept or dropped when its match closes.
            Recorded::Writing => (Status::Preparing, None),
            Recorded::No => (Status::Gone, None),
        })
    }

    /// A token for the account and the match: the account's live one for
    /// that match if it has one, else a new one while it has fewer than
    /// [`TOKENS_PER_ACCOUNT`] (a token past its time whose download is still
    /// under way counted). `None`: no room, or no randomness.
    fn issue(
        &self,
        inner: &mut Inner,
        account_id: u32,
        match_id: u32,
        now: Instant,
    ) -> Option<String> {
        inner.prune_tokens(now);
        if let Some((bytes, _)) = inner.tokens.iter().find(|(_, token)| {
            token.account_id == account_id && token.match_id == match_id && token.live(now)
        }) {
            return Some(hex(bytes));
        }
        let live = inner
            .tokens
            .values()
            .filter(|token| token.account_id == account_id)
            .count();
        if live >= TOKENS_PER_ACCOUNT {
            return None;
        }
        let mut bytes = [0u8; 16];
        if let Err(error) = OsRng.try_fill_bytes(&mut bytes) {
            log::error!("Save: no randomness for a token: {error}");
            return None;
        }
        inner.tokens.insert(
            bytes,
            Token {
                account_id,
                match_id,
                until: now + TOKEN_LIFETIME,
                downloads_left: TOKEN_DOWNLOADS,
                downloading: false,
            },
        );
        Some(hex(&bytes))
    }

    /// A download of a token's file: the token's one at a time (a `GET` or
    /// a `HEAD`) until the [`Download`] is dropped, so that one account
    /// holds [`TOKENS_PER_ACCOUNT`] of the page's download slots at most.
    /// `count`: a `GET`, which takes one of the token's downloads (a `HEAD`
    /// takes none). [`Refused::Unknown`] for anything but exactly 32
    /// lower-case hex digits of a live token; [`Refused::Busy`] while
    /// another download of it is under way (none of its downloads taken).
    pub fn take_download(
        &self,
        token: &str,
        count: bool,
        now: Instant,
    ) -> Result<Download<'_>, Refused> {
        let bytes = parse_token(token).ok_or(Refused::Unknown)?;
        let mut inner = self.inner.locked();
        inner.prune_tokens(now);
        let token = inner
            .tokens
            .get_mut(&bytes)
            .filter(|token| token.live(now))
            .ok_or(Refused::Unknown)?;
        if token.downloading {
            return Err(Refused::Busy);
        }
        if count {
            token.downloads_left -= 1;
        }
        token.downloading = true;
        Ok(Download {
            replays: self,
            token: bytes,
            match_id: token.match_id,
        })
    }

    /// Deletes `<id>.srpreplay` and `<id>.json` (names made here).
    pub fn delete_file(&self, match_id: u32) {
        for path in [self.file_path(match_id), self.json_path(match_id)]
            .into_iter()
            .flatten()
        {
            match std::fs::remove_file(&path) {
                Ok(()) => log::info!("Save: {} deleted", path.display()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => log::warn!("Save: can't delete {}: {error}", path.display()),
            }
        }
    }

    /// The files waiting to be made, the next first.
    pub fn jobs(&self) -> Vec<u32> {
        self.inner.locked().jobs.iter().copied().collect()
    }

    /// The next match to make the file of, waiting for one; `None` once
    /// stopped.
    pub fn next_job(&self) -> Option<u32> {
        let mut inner = self.inner.locked();
        loop {
            if inner.stopped {
                return None;
            }
            if let Some(match_id) = inner.jobs.pop_front() {
                inner.making = Some(match_id);
                return Some(match_id);
            }
            inner = self
                .work
                .wait(inner)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
    }

    /// The file of [`Self::next_job`]'s match is made, or failed and is
    /// tried again as its [`Failure`] says. Its askers' next polls look at
    /// once.
    pub fn job_done(&self, match_id: u32, made: Result<(), Failure>, now: Instant) {
        let mut inner = self.inner.locked();
        if inner.making == Some(match_id) {
            inner.making = None;
        }
        inner.news_of(match_id);
        let failure = match made {
            Ok(()) => {
                inner.failed.remove(&match_id);
                return;
            }
            Err(failure) => failure,
        };
        let times = inner.failed.get(&match_id).map_or(0, |failed| failed.times);
        let (until, times) = match failure {
            Failure::Disk => {
                let wait = FAILED_FOR
                    .saturating_mul(1 << times.min(16))
                    .min(FAILED_FOR_AT_MOST);
                (Some(now + wait), times + 1)
            }
            Failure::Lasting => (None, 0),
        };
        inner.failed.insert(match_id, Failed { until, times });
    }

    /// Ends [`Self::next_job`]'s wait for good.
    pub fn stop(&self) {
        self.inner.locked().stopped = true;
        self.work.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_replay_id_is_the_recordings_stem_and_parses_back() {
        assert_eq!(utc_stamp(UNIX_EPOCH), "19700101-000000");
        let time = UNIX_EPOCH + Duration::from_secs(1_791_945_296);
        assert_eq!(utc_stamp(time), "20261014-023456");
        let leap = UNIX_EPOCH + Duration::from_secs(951_782_400 + 3_661);
        assert_eq!(utc_stamp(leap), "20000229-010101");
        let id = ReplayId::new(0xAB, time);
        assert_eq!(id.as_str(), "rec026g_20261014-023456_000000ab");
        assert_eq!(ReplayId::parse(id.as_str()), Some(id.clone()));
        assert_eq!(id.made(), time);
        assert_eq!(ReplayId::new(1, leap).made(), leap);
        assert_eq!(ReplayId::new(1, UNIX_EPOCH).made(), UNIX_EPOCH);
        for bad in [
            "rec026g_20261014-023456_000000AB",
            "rec026g_20261014-02345_000000ab",
            "rec026g_20261014023456_000000ab",
            "rec026g_20261014-023456_000000ab.srprec",
            "20261014-023456_000000ab",
        ] {
            assert_eq!(ReplayId::parse(bad), None, "{bad}");
        }
    }

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "srp-replays-{name}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    const MATCH: u32 = 0x1F07_C8FB;

    /// A game of today's revision: the oldest whose recordings are given
    /// ([`oldest_replayable_here`]).
    fn current() -> u8 {
        oldest_replayable_here(0)
    }

    /// A list row's replay state, by what the recordings' folder holds and
    /// the asker's game.
    #[test]
    fn a_rows_replay_state_follows_the_recording_and_the_askers_game() {
        let root = temp("state");
        let record_dir = root.join("recordings");
        std::fs::create_dir_all(&record_dir).unwrap();
        let replays = Replays::new(Some(record_dir.clone()));
        let now = Instant::now();
        let state = |match_id, revision| replays.state(match_id, revision, now);
        let file = record_dir.join(format!("rec026g_20261015-120000_{MATCH:08x}.srprec"));
        assert_eq!(state(MATCH, Some(current())), ReplayState::NotRecorded);
        std::fs::write(&file, b"x").unwrap();
        replays.recording_kept(MATCH);
        assert_eq!(state(MATCH, Some(current())), ReplayState::Kept);
        assert_eq!(state(MATCH, None), ReplayState::Kept);
        assert_eq!(
            state(MATCH, Some(MIN_REVISION - 1)),
            ReplayState::OldVersion
        );
        // Deleted by the cap; one closed under the minimum was never kept.
        std::fs::remove_file(&file).unwrap();
        replays.recording_deleted(MATCH);
        assert_eq!(state(MATCH, Some(current())), ReplayState::Gone);
        replays.recording_not_kept(7);
        assert_eq!(state(7, Some(current())), ReplayState::NotRecorded);
        // Still being written: kept.
        std::fs::write(
            record_dir.join("rec026g_20261015-120000_00000008.srprec.part"),
            b"x",
        )
        .unwrap();
        replays.recording_kept(8);
        assert_eq!(state(8, None), ReplayState::Kept);
        let off = Replays::new(None);
        assert_eq!(off.state(MATCH, None, now), ReplayState::NotRecorded);
        let codes = [
            ReplayState::NotRecorded,
            ReplayState::Kept,
            ReplayState::Gone,
            ReplayState::OldVersion,
        ]
        .map(ReplayState::code);
        assert_eq!(codes, [0, 1, 3, 4]);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn status_words_are_the_contracts() {
        let all = [
            (Status::Preparing, "preparing"),
            (Status::Ready, "ready"),
            (Status::None, "none"),
            (Status::Gone, "gone"),
            (Status::NotYours, "not_yours"),
            (Status::OldGame, "old_game"),
            (Status::Incompatible, "incompatible"),
            (Status::Busy, "busy"),
            (Status::Failed, "failed"),
            (Status::Off, "off"),
            (Status::Watching, "watching"),
        ];
        for (status, word) in all {
            assert_eq!(status.word(), word);
        }
    }

    #[test]
    fn revisions_older_than_eleven_and_newer_than_the_askers_do_not_play() {
        // No revision kept (before 2026-10-15), or none known: not played.
        assert_eq!(refusal_of(&[], 14, 11), Some(Refusal::Unknown));
        assert_eq!(refusal_of(&[None, Some(0)], 14, 11), Some(Refusal::Unknown));
        // Every recorded revision between 11 and the asker's own: 12 and
        // 13 compute a match as 11 and 14 do.
        assert_eq!(
            refusal_of(&[None, Some(0), Some(11), Some(12), Some(14)], 14, 11),
            None
        );
        assert_eq!(
            refusal_of(&[Some(10), Some(14)], 14, 11),
            Some(Refusal::Older {
                from: 10,
                to: 14,
                oldest: 11
            })
        );
        assert_eq!(
            refusal_of(&[Some(15)], 14, 11),
            Some(Refusal::Newer { from: 15, to: 15 })
        );
        assert_eq!(refusal_of(&[Some(15)], 16, 11), None);
        // A release that changed the simulation, 17: 16's recordings go.
        assert_eq!(
            refusal_of(&[Some(16), Some(16)], 17, 17),
            Some(Refusal::Older {
                from: 16,
                to: 16,
                oldest: 17
            })
        );
        assert_eq!(refusal_of(&[Some(17)], 18, 17), None);
    }

    #[test]
    fn the_simulation_changes_list_reads() {
        let changes = simulation_changes(SIMULATION_CHANGES).expect("it reads");
        assert!(changes.iter().all(|row| !row.what.trim().is_empty()));
        // 11 the oldest given now; `next` (a PR's change not yet released)
        // would be the server's own revision.
        let last = changes.last().unwrap();
        let before = changes.iter().rev().find_map(|row| row.revision).unwrap();
        assert!(before >= 11);
        match last.revision {
            Some(revision) => assert_eq!(oldest_replayable_here(20), revision),
            None => assert_eq!(oldest_replayable_here(200), 200),
        }
        assert_eq!(oldest_replayable_here(0), before);
    }

    #[test]
    fn next_is_the_release_s_revision_and_the_list_s_rules_hold() {
        let list =
            "# a comment\n\n3\tthe battery rule\n11\tthe TDM end\nnext\ta new kit at the start\n";
        let changes = simulation_changes(list).unwrap();
        assert_eq!(
            changes.iter().map(|row| row.revision).collect::<Vec<_>>(),
            [Some(3), Some(11), None]
        );
        assert_eq!(changes[2].what, "a new kit at the start");
        // The release of 17 runs with SRP_MIN_PATCH_REVISION=17.
        assert_eq!(oldest_replayable(&changes, 17), 17);
        // No release (a local server): the number before it.
        assert_eq!(oldest_replayable(&changes, 0), 11);
        // Never below the last number.
        assert_eq!(oldest_replayable(&changes, 5), 11);
        // Its record wrote 17 for `next`: 17, whatever the server's revision.
        let recorded = list.replace("next\t", "17\t");
        assert_eq!(
            oldest_replayable(&simulation_changes(&recorded).unwrap(), 18),
            17
        );
        assert_eq!(
            oldest_replayable(&simulation_changes("11\tx\n").unwrap(), 16),
            11
        );
        for bad in [
            "",
            "# only a comment\n",
            "11 the TDM end\n",
            "11\t\n",
            "0\tzero\n",
            "256\ttoo high\n",
            "x\tno number\n",
            "11\ta\n11\tthe same again\n",
            "12\ta\n11\tlower\n",
            "next\ta\nnext\tanother\n",
            "next\ta\n18\tafter next\n",
        ] {
            assert!(simulation_changes(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn only_eight_hex_digit_names_are_recordings_ids() {
        assert_eq!(match_of_name("1f07c8fb.srprec", ".srprec"), Some(MATCH));
        for bad in [
            "1F07C8FB.srprec",
            "1f07c8f.srprec",
            "01f07c8fb.srprec",
            "1f07c8fb.json",
            "../1f07c8f.srprec",
            "1f07c8fb.srprec.part",
        ] {
            assert_eq!(match_of_name(bad, ".srprec"), None, "{bad}");
        }
    }

    // ------------------------------------------------------- Save

    const ALICE: u32 = 0x1_0001;
    const BOB: u32 = 0x1_0002;

    /// Recordings in `root/recordings`, Save's files in `root/replays`.
    fn on(root: &Path) -> Replays {
        let record_dir = root.join("recordings");
        std::fs::create_dir_all(&record_dir).unwrap();
        let save = SaveSettings {
            replay_dir: root.join("replays"),
            max_replay_bytes: 1 << 20,
        }
        .checked(&record_dir)
        .unwrap();
        Replays::new(Some(record_dir)).with_save(save)
    }

    /// A download of `token` taken and given back at once: its match.
    fn take(replays: &Replays, token: &str, count: bool, now: Instant) -> Result<u32, Refused> {
        replays
            .take_download(token, count, now)
            .map(|download| download.match_id)
    }

    /// A kept recording of `match_id`, as the writer leaves it.
    fn kept(replays: &Replays, match_id: u32) {
        std::fs::write(
            replays
                .record_dir()
                .unwrap()
                .join(format!("rec026g_20261015-120000_{match_id:08x}.srprec")),
            b"x",
        )
        .unwrap();
        replays.recording_kept(match_id);
    }

    /// The file of `match_id` made, played by ALICE and by games of
    /// `revisions`.
    fn made_by(replays: &Replays, match_id: u32, revisions: Vec<Option<u8>>) {
        std::fs::write(replays.file_path(match_id).unwrap(), b"replay bytes").unwrap();
        let sidecar = Sidecar {
            match_id,
            size: 12,
            sha256: "ab".repeat(32),
            revisions,
            account_ids: vec![ALICE],
            made_at: 1,
        };
        std::fs::write(replays.json_path(match_id).unwrap(), sidecar.to_json()).unwrap();
    }

    fn made(replays: &Replays, match_id: u32) {
        made_by(replays, match_id, vec![Some(current())]);
    }

    #[test]
    fn the_replay_folder_is_never_the_recordings_or_inside_or_around_it() {
        let root = temp("overlap");
        let overlap = |replay: PathBuf, record: PathBuf| {
            SaveSettings {
                replay_dir: replay,
                max_replay_bytes: 1,
            }
            .checked(&record)
        };
        let a = root.join("a");
        assert_eq!(overlap(a.clone(), a.clone()), Err(FolderError::Overlap));
        assert_eq!(overlap(a.join("b"), a.clone()), Err(FolderError::Overlap));
        assert_eq!(overlap(a.clone(), a.join("b")), Err(FolderError::Overlap));
        // The same folder by another spelling.
        assert_eq!(
            overlap(a.join("..").join("a"), a.clone()),
            Err(FolderError::Overlap)
        );
        assert!(overlap(root.join("replays"), root.join("recordings")).is_ok());
        // Without recordings Save stays off.
        let save = SaveSettings {
            replay_dir: root.join("replays"),
            max_replay_bytes: 1,
        };
        assert!(Replays::new(None).with_save(save).save_settings().is_none());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn save_off_answers_off_and_gives_no_path() {
        let replays = Replays::new(Some(temp("off")));
        let now = Instant::now();
        assert_eq!(replays.save_answer(ALICE, now).status, Status::Off);
        assert_eq!(
            replays.save_ask(ALICE, MATCH, Some(current()), None, now),
            Status::Off
        );
        assert_eq!(replays.file_path(MATCH), None);
        assert_eq!(
            take(&replays, &"0".repeat(32), true, now),
            Err(Refused::Unknown)
        );
    }

    #[test]
    fn a_made_file_is_ready_with_a_token_of_five_downloads_for_ten_minutes() {
        let root = temp("ready");
        let replays = on(&root);
        made(&replays, MATCH);
        let now = Instant::now();
        assert_eq!(replays.save_answer(ALICE, now).status, Status::None);
        assert_eq!(
            replays.save_ask(ALICE, MATCH, Some(current()), Some(true), now),
            Status::Ready
        );
        let answer = replays.save_answer(ALICE, now);
        assert_eq!((answer.status, answer.match_id), (Status::Ready, MATCH));
        assert_eq!(answer.figure(), 12);
        let ready = answer.ready.clone().unwrap();
        assert!(parse_token(&ready.token).is_some());
        let link = answer.link().unwrap();
        assert_eq!(
            link,
            format!("replay 1f07c8fb {} {} 12", ready.token, "ab".repeat(32))
        );
        // 114 + the size's digits, within the client's 245 units.
        assert_eq!(link.len(), 114 + 2);
        // Another player's poll is its own.
        assert_eq!(replays.save_answer(BOB, now).status, Status::None);
        // Five downloads, HEADs free.
        for _ in 0..5 {
            assert_eq!(take(&replays, &ready.token, false, now), Ok(MATCH));
            assert_eq!(take(&replays, &ready.token, true, now), Ok(MATCH));
        }
        assert_eq!(
            take(&replays, &ready.token, true, now),
            Err(Refused::Unknown)
        );
        // A new ask gives a new token, which ends with its ten minutes.
        let later = now + ASK_EVERY;
        replays.save_ask(ALICE, MATCH, Some(current()), None, later);
        let second = replays.save_answer(ALICE, later).ready.unwrap().token;
        assert_ne!(second, ready.token);
        assert_eq!(take(&replays, &second, true, later), Ok(MATCH));
        assert_eq!(
            take(&replays, &second, true, later + TOKEN_LIFETIME),
            Err(Refused::Unknown)
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_token_is_exactly_32_lower_hex_digits() {
        assert!(parse_token(&"0123456789abcdef".repeat(2)).is_some());
        for bad in [
            "0123456789ABCDEF0123456789abcdef".to_owned(),
            "0".repeat(31),
            "0".repeat(33),
            format!("{}g", "0".repeat(31)),
            format!("{}/", "0".repeat(31)),
            format!("{}\u{e9}", "0".repeat(30)),
            String::new(),
        ] {
            assert_eq!(parse_token(&bad), None, "{bad}");
        }
        assert_eq!(token_tag("0123456789abcdef"), "012345");
    }

    #[test]
    fn three_live_tokens_an_account_and_one_ask_in_two_seconds() {
        let root = temp("tokens");
        let replays = on(&root);
        let mut now = Instant::now();
        let matches: Vec<u32> = (0x100..0x104).collect();
        for &match_id in &matches {
            made(&replays, match_id);
        }
        for &match_id in &matches[..3] {
            let ask = |match_id| replays.save_ask(ALICE, match_id, Some(current()), None, now);
            assert_eq!(ask(match_id), Status::Ready);
            // Asked again at once: it stands; another match: busy.
            assert_eq!(ask(match_id), Status::Ready);
            assert_eq!(ask(0x999), Status::Busy);
            assert_eq!(replays.save_answer(ALICE, now).status, Status::Busy);
            now += ASK_EVERY;
        }
        let ask = |account, match_id, now| replays.save_ask(account, match_id, Some(current()), None, now);
        assert_eq!(ask(ALICE, matches[3], now), Status::Busy);
        // The same match again keeps its live token; another account has
        // its own three.
        now += ASK_EVERY;
        assert_eq!(ask(ALICE, matches[0], now), Status::Ready);
        assert_eq!(ask(BOB, matches[3], now), Status::Ready);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_save_is_refused_for_an_old_game_another_players_match_and_other_revisions() {
        let root = temp("refused");
        let replays = on(&root).with_watch(WatchSettings {
            access: super::super::replay_watch::Access::Match,
            ..WatchSettings::default()
        });
        let today = current();
        let newer = today + 1;
        made_by(&replays, MATCH, vec![Some(today), Some(newer)]);
        let mut now = Instant::now();
        let mut ask = |account, revision, played| {
            now += ASK_EVERY;
            replays.save_ask(account, MATCH, revision, played, now)
        };
        // Revisions before 14 have no replay link.
        for old in [Some(11), Some(13), None] {
            assert_eq!(ask(ALICE, old, Some(true)), Status::OldGame);
        }
        // The lobby's report says so, or the sidecar does.
        assert_eq!(ask(ALICE, Some(today), Some(false)), Status::NotYours);
        assert_eq!(ask(BOB, Some(newer), None), Status::NotYours);
        // Made by a newer game too: not for the current one.
        assert_eq!(ask(ALICE, Some(today), Some(true)), Status::Incompatible);
        assert_eq!(ask(ALICE, Some(newer), Some(true)), Status::Ready);
        // Anyone may by default: Bob gets it.
        let open = on(&root);
        assert_eq!(
            open.save_ask(BOB, MATCH, Some(newer), None, Instant::now()),
            Status::Ready
        );
        // No token was given for any of the refusals.
        assert_eq!(replays.inner.locked().tokens.len(), 1);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_refused_save_gives_no_token() {
        let root = temp("why");
        let replays = on(&root).with_oldest_revision(17);
        made_by(&replays, MATCH, vec![Some(16), None]);
        let now = Instant::now();
        assert_eq!(
            replays.save_ask(ALICE, MATCH, Some(17), Some(true), now),
            Status::Incompatible
        );
        let first = replays.save_answer(ALICE, now);
        assert_eq!(first.status, Status::Incompatible);
        assert!(first.link().is_none());
        assert!(replays.inner.locked().tokens.is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_recording_without_a_file_is_queued_once_and_answered_when_made() {
        let root = temp("queue");
        let replays = on(&root);
        let record_dir = replays.record_dir().unwrap().to_owned();
        let now = Instant::now();
        assert_eq!(
            replays.save_ask(ALICE, MATCH, Some(current()), None, now),
            Status::Gone
        );
        // Still being written: preparing, its place not known.
        std::fs::write(record_dir.join("rec026g_1_1f07c8fb.srprec.part"), b"x").unwrap();
        let later = now + LISTING_KEPT;
        assert_eq!(
            replays.save_ask(ALICE, MATCH, Some(current()), None, later),
            Status::Preparing
        );
        assert!(replays.jobs().is_empty());
        assert_eq!(replays.save_answer(ALICE, later).figure(), 0);
        // Kept: nothing is made until someone asks; askers share one job.
        std::fs::write(record_dir.join("rec026g_2_1f07c8fb.srprec"), b"x").unwrap();
        replays.recording_kept(MATCH);
        assert!(replays.jobs().is_empty());
        let later = later + ASK_EVERY;
        for player in [ALICE, BOB] {
            assert_eq!(
                replays.save_ask(player, MATCH, Some(current()), None, later),
                Status::Preparing
            );
        }
        assert_eq!(replays.jobs(), [MATCH]);
        assert_eq!(replays.next_job(), Some(MATCH));
        let answer = replays.save_answer(ALICE, later);
        assert_eq!((answer.status, answer.figure()), (Status::Preparing, 1));
        made(&replays, MATCH);
        replays.job_done(MATCH, Ok(()), later);
        // The job's news is looked at by the next poll at once; each asker
        // gets a token of its own.
        let (a, b) = (replays.save_answer(ALICE, later), replays.save_answer(BOB, later));
        assert_eq!((a.status, a.figure()), (Status::Ready, 12));
        assert_ne!(a.ready.unwrap().token, b.ready.unwrap().token);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_failure_is_tried_again_only_when_its_cause_can_change() {
        let root = temp("retry");
        let replays = on(&root);
        let mut at = Instant::now();
        let ask = |match_id: u32, now: Instant| {
            let status = replays.save_ask(ALICE, match_id, Some(current()), None, now);
            (status, replays.jobs().contains(&match_id))
        };
        let fail = |match_id: u32, failure: Failure, now: Instant| {
            assert_eq!(replays.next_job(), Some(match_id));
            replays.job_done(match_id, Err(failure), now);
        };
        // The recording itself: never again while the server runs.
        kept(&replays, 1);
        assert_eq!(ask(1, at), (Status::Preparing, true));
        fail(1, Failure::Lasting, at);
        at += 100 * FAILED_FOR_AT_MOST;
        assert_eq!(ask(1, at), (Status::Failed, false));
        // Disk: ten minutes, then twenty, forty, ... four hours at most.
        kept(&replays, 3);
        at += ASK_EVERY;
        assert_eq!(ask(3, at), (Status::Preparing, true));
        for wait in [10, 20, 40, 80, 160, 240, 240] {
            let wait = Duration::from_secs(wait * 60);
            fail(3, Failure::Disk, at);
            at += wait - ASK_EVERY;
            assert_eq!(ask(3, at), (Status::Failed, false), "{wait:?}");
            at += ASK_EVERY;
            assert_eq!(ask(3, at), (Status::Preparing, true), "{wait:?}");
        }
        // Made at last: its failures are forgotten. Its recording deleted:
        // the file goes with it, and the match is gone.
        assert_eq!(replays.next_job(), Some(3));
        made(&replays, 3);
        replays.job_done(3, Ok(()), at);
        assert!(!replays.inner.locked().failed.contains_key(&3));
        replays.recording_deleted(3);
        assert!(!replays.file_path(3).unwrap().exists());
        assert!(!replays.json_path(3).unwrap().exists());
        at += ASK_EVERY;
        assert_eq!(ask(3, at), (Status::Gone, false));
        replays.stop();
        assert_eq!(replays.next_job(), None);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_burst_of_polls_is_looked_at_once_a_second_and_the_folder_listed_once() {
        let root = temp("burst");
        let replays = on(&root);
        let record_dir = replays.record_dir().unwrap().to_owned();
        std::fs::write(
            record_dir.join(format!("rec026g_20261015-120000_{MATCH:08x}.srprec.part")),
            b"x",
        )
        .unwrap();
        let now = Instant::now();
        assert_eq!(
            replays.save_ask(ALICE, MATCH, Some(current()), None, now),
            Status::Preparing
        );
        let counts = |replays: &Replays| {
            let inner = replays.inner.locked();
            (inner.evaluations, inner.listings)
        };
        assert_eq!(counts(&replays), (1, 1));
        // 80 polls in one datagram: the ask's look stands.
        for step in 0..80u64 {
            let at = now + Duration::from_millis(step * 12);
            assert_eq!(replays.save_answer(ALICE, at).status, Status::Preparing);
        }
        assert_eq!(counts(&replays), (1, 1));
        // A second on: one look for the whole burst.
        let next = now + RECHECK_EVERY;
        for _ in 0..80 {
            replays.save_answer(ALICE, next);
        }
        assert_eq!(counts(&replays), (2, 1));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn other_matches_wait_in_line_up_to_twenty() {
        let root = temp("line");
        let replays = on(&root);
        let now = Instant::now();
        let first = 0x500;
        for n in 0..=MAX_JOBS as u32 + 1 {
            kept(&replays, first + n);
        }
        let ask = |account, match_id| replays.save_ask(account, match_id, Some(current()), None, now);
        assert_eq!(ask(1, first), Status::Preparing);
        assert_eq!(replays.next_job(), Some(first));
        for n in 1..=MAX_JOBS as u32 {
            assert_eq!(ask(1 + n, first + n), Status::Preparing);
            assert_eq!(replays.save_answer(1 + n, now).figure(), u64::from(n) + 1);
        }
        // One more is busy, and not queued.
        assert_eq!(ask(99, first + MAX_JOBS as u32 + 1), Status::Busy);
        assert_eq!(replays.jobs().len(), MAX_JOBS);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_token_downloads_one_at_a_time() {
        let root = temp("one-at-a-time");
        let replays = on(&root);
        made(&replays, MATCH);
        let now = Instant::now();
        replays.save_ask(ALICE, MATCH, Some(current()), None, now);
        let token = replays.save_answer(ALICE, now).ready.unwrap().token;
        let held = replays.take_download(&token, true, now).unwrap();
        // A second download of it waits for the first, a HEAD too, and
        // takes none of its downloads.
        for count in [true, false] {
            assert_eq!(take(&replays, &token, count, now), Err(Refused::Busy));
        }
        // Past its time while downloading: kept, but no more downloads.
        let late = now + TOKEN_LIFETIME;
        assert_eq!(take(&replays, &token, true, late), Err(Refused::Unknown));
        drop(held);
        assert_eq!(take(&replays, &token, true, now), Ok(MATCH));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_file_whose_size_is_not_its_sidecars_is_not_given() {
        let root = temp("mismatch");
        let replays = on(&root);
        made(&replays, MATCH);
        std::fs::write(replays.file_path(MATCH).unwrap(), b"longer than it was").unwrap();
        assert_eq!(
            replays.save_ask(ALICE, MATCH, Some(current()), None, Instant::now()),
            Status::Gone
        );
        assert_eq!(
            replays.file_path(MATCH).unwrap().file_name().unwrap(),
            "1f07c8fb.srpreplay"
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
