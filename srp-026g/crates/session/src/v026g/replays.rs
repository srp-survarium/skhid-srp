//! "Watch replay" from the History of srp's changed lobby movie
//! (`docs/notes/replay_watch.md`): what the lobby, the match server's
//! conversion thread and the web page share of the replays, in memory.
//!
//! - The match server converts a kept match recording (`SRP_RECORD_DIR`)
//!   into the game's own replay file (`match_server::v026g::replay`), one at
//!   a time: `SRP_REPLAY_DIR/<id:08x>.sur` and its sidecar `<id:08x>.json`
//!   (size, SHA-256, each place's patch revision, the accounts), at the
//!   lowest CPU priority. A match is queued here when a player first asks
//!   for its replay ([`Replays::ask`]); every asker of it waits for that one
//!   job, and the replay made is kept for later asks.
//! - The lobby asks for a match's replay for a player ([`Replays::ask`]) and
//!   answers the movie's polls with the player's own latest status
//!   ([`Replays::answer`]). A replay that is there is given a download
//!   token ([`Token`]).
//! - The web page hands out `/replays/<token>` ([`Replays::take_download`]):
//!   the file of the token's match, by a path built here from that match
//!   id alone ([`Replays::sur_path`]).
//!
//! What is checked of the asker (the account played the match) is a
//! courtesy (owner, 2026-10-15: a replay of a match one did not play is
//! fine); the line that must hold is that nothing but a converter-made
//! `.sur` is ever served. Tokens are kept in memory only: a restart ends
//! them all.
//!
//! Settings (environment, read once at start, [`ReplaySettings::from_env`]):
//! `SRP_REPLAY_DIR` and `SRP_GAME_MODES_DIR` (both needed; replays are off,
//! status `off`, without them or without `SRP_RECORD_DIR`),
//! `SRP_REPLAY_MAX_RECORDING_MB` (default [`DEFAULT_MAX_RECORDING_MB`]) and
//! `SRP_REPLAY_MAX_MB` (default [`DEFAULT_MAX_REPLAY_MB`]).

use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use password_hash::rand_core::{OsRng, RngCore};

use crate::LockExt;

/// The `player` of a `3D` that is a replay ask, not a row of a results
/// screen, and the `start` of the `3B` that polls for the answer.
pub use survarium::v026g::lobby::{REPLAY_ASK, REPLAY_START};
/// A game plays replays from this patch revision on (revision 14 starts
/// the updater from the replay link: `client_patch.md` section 3.27;
/// revisions 12 and 13 have no replay link).
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

/// The release tag a revision went out with (`release/release.sh` tags
/// every release from 15 on; 12 was tagged by hand; 13, 14 and the ones
/// before 12 have none).
pub fn release_tag(revision: u8) -> Option<String> {
    (revision == 12 || revision >= 15).then(|| format!("026g-rev{revision}"))
}
/// How long a token lives.
pub const TOKEN_LIFETIME: Duration = Duration::from_secs(10 * 60);
/// The downloads one token makes (a broken download resumes with the next).
pub const TOKEN_DOWNLOADS: u8 = 5;
/// Live tokens of one account at most.
pub const TOKENS_PER_ACCOUNT: usize = 3;
/// One ask of an account in this long; another is answered `busy`.
pub const ASK_EVERY: Duration = Duration::from_secs(2);
/// Conversions waiting at most, one for each player of a full match (a job
/// is a few seconds); one asked for past them is `busy`.
pub const MAX_JOBS: usize = 20;
/// `SRP_REPLAY_MAX_RECORDING_MB` when unset. The conversion reads a
/// recording as it goes, and keeps a copy of each relay, the uploaded
/// worlds and the options: 123 MiB at most for the largest recording seen
/// (13.5 MB, a match of 20 places and 15 minutes; `replay_watch.md`), and
/// about as much more for each 13.5 MB more. The container has 512 MB for
/// everything: a larger file is not converted (status `failed`).
pub const DEFAULT_MAX_RECORDING_MB: u64 = 24;
/// `SRP_REPLAY_MAX_MB` when unset: the replays (`.sur` files and their
/// sidecars) over this together go, the oldest first. A replay that went
/// is made again from its recording when someone asks for it.
pub const DEFAULT_MAX_REPLAY_MB: u64 = 1_000;
/// A conversion that failed for its mode's file is not tried again for
/// this long ([`Failure`]); one that failed writing, for this long at first.
const FAILED_FOR: Duration = Duration::from_secs(10 * 60);
/// One that failed writing again and again waits at most this long.
const FAILED_FOR_AT_MOST: Duration = Duration::from_secs(4 * 60 * 60);
/// A `preparing` answer is looked at again at most this often for one
/// account: the movie polls once a second, but a crafted game may send
/// dozens of polls a datagram, and each look reads the sidecar.
pub const RECHECK_EVERY: Duration = Duration::from_secs(1);
/// The recordings' folder is listed at most this often for the asks; the
/// writer's news of a file (kept, dropped, deleted) has it listed anew.
const LISTING_KEPT: Duration = Duration::from_secs(5);
/// Match ids remembered as not kept or deleted, at most.
const REMEMBERED: usize = 4096;
/// An answer this old is forgotten.
const ASK_KEPT: Duration = Duration::from_secs(15 * 60);

/// The status word of an `R1` answer (`CONTRACT-R1`): what came of the
/// player's latest ask.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// Asked, being converted or looked up: the movie polls on.
    Preparing,
    /// The link is set (`9E` goes just before the answer).
    Ready,
    /// No ask from this player yet.
    None,
    /// The match had too few players, or recordings were off.
    NotRecorded,
    /// The recording was deleted (the storage cap) or is not found.
    Gone,
    /// The asker did not play that match.
    NotYours,
    /// The asker's game is older than [`MIN_REVISION`].
    OldGame,
    /// The recording was made by games this one cannot play.
    Incompatible,
    /// Too many conversions or tokens now, or asked too fast.
    Busy,
    /// The conversion failed.
    Failed,
    /// Replays are switched off on this server.
    Off,
}

impl Status {
    /// The word the `R1` row carries.
    pub fn word(self) -> &'static str {
        match self {
            Self::Preparing => "preparing",
            Self::Ready => "ready",
            Self::None => "none",
            Self::NotRecorded => "not_recorded",
            Self::Gone => "gone",
            Self::NotYours => "not_yours",
            Self::OldGame => "old_game",
            Self::Incompatible => "incompatible",
            Self::Busy => "busy",
            Self::Failed => "failed",
            Self::Off => "off",
        }
    }
}

/// `SRP_REPLAY_DIR`, `SRP_GAME_MODES_DIR` and the recordings' folder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReplaySettings {
    /// Where the `.sur` files and their sidecars are made. Only files named
    /// `<8 hex>.sur`, `<8 hex>.json` and their `.part` are ever written or
    /// deleted there, and only `<8 hex>.sur` is ever served.
    pub replay_dir: PathBuf,
    /// `SRP_RECORD_DIR`: the match recordings.
    pub record_dir: PathBuf,
    /// The game's `gameplay/game_modes/*.options` files (game data, kept
    /// out of the repository).
    pub modes_dir: PathBuf,
    /// A recording larger than this is not converted.
    pub max_recording_bytes: u64,
    /// The replays (`.sur` and sidecars) over this many bytes together:
    /// the oldest go (`SRP_REPLAY_MAX_MB`).
    pub max_replay_bytes: u64,
}

/// Why the folders cannot be used for replays.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FolderError {
    /// The replay folder is the recordings' folder, inside it or holds it:
    /// a recording must never sit where replays are served from.
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

impl ReplaySettings {
    /// What the environment asks for; `None` (replays off, said in the log)
    /// when a folder is not set or the two folders overlap.
    pub fn from_env() -> Option<Self> {
        let var = |name: &str| {
            std::env::var(name)
                .ok()
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty() && !value.eq_ignore_ascii_case("off"))
        };
        let replay_dir = var("SRP_REPLAY_DIR")?;
        let Some(record_dir) = var("SRP_RECORD_DIR") else {
            log::warn!("Replays: SRP_REPLAY_DIR is set but recordings are off; replays are off");
            return None;
        };
        let Some(modes_dir) = var("SRP_GAME_MODES_DIR") else {
            log::warn!("Replays: SRP_GAME_MODES_DIR is not set; replays are off");
            return None;
        };
        let megabytes = |name: &str, default: u64| match var(name) {
            None => default,
            Some(value) => value
                .parse::<u64>()
                .ok()
                .filter(|&mb| mb <= u64::MAX >> 20)
                .unwrap_or_else(|| {
                    log::error!("{name} is `{value}`; it must be a number. Using {default}");
                    default
                }),
        };
        let settings = Self {
            replay_dir: replay_dir.into(),
            record_dir: record_dir.into(),
            modes_dir: modes_dir.into(),
            max_recording_bytes: megabytes("SRP_REPLAY_MAX_RECORDING_MB", DEFAULT_MAX_RECORDING_MB)
                << 20,
            max_replay_bytes: megabytes("SRP_REPLAY_MAX_MB", DEFAULT_MAX_REPLAY_MB) << 20,
        };
        match settings.checked() {
            Ok(settings) => Some(settings),
            Err(error) => {
                log::error!("Replays are off: {error}");
                None
            }
        }
    }

    /// The settings with both folders made and their real paths, unless
    /// the replay folder and the recordings' overlap.
    pub fn checked(self) -> Result<Self, FolderError> {
        let real = |dir: &Path| {
            std::fs::create_dir_all(dir)
                .and_then(|()| std::fs::canonicalize(dir))
                .map_err(|error| FolderError::Io(format!("{}: {error}", dir.display())))
        };
        let replay_dir = real(&self.replay_dir)?;
        let record_dir = real(&self.record_dir)?;
        if replay_dir.starts_with(&record_dir) || record_dir.starts_with(&replay_dir) {
            return Err(FolderError::Overlap);
        }
        Ok(Self {
            replay_dir,
            record_dir,
            ..self
        })
    }
}

/// `<id:08x>`: the one form a match id takes in a replay file's name.
pub fn file_stem(match_id: u32) -> String {
    format!("{match_id:08x}")
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

/// What the sidecar `<id>.json` says of a replay ([`Sidecar::to_json`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sidecar {
    pub match_id: u32,
    pub size: u64,
    /// 64 lower-case hex digits.
    pub sha256: String,
    /// By place: the patch revision each place's game connected with (the
    /// highest, if it connected again), `None` for one that never did or
    /// a recording older than 2026-10-15.
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

    /// Why a game of `revision` may not play this replay, if it may not:
    /// every revision the recording's games had must be `oldest` (the
    /// oldest replayable, [`oldest_replayable`]) or later and no later than
    /// the asker's (a newer game's match may play out otherwise in an
    /// older one). 0 (a session the server did not know) and places that
    /// never connected are not counted; a recording with no revision at
    /// all (made before 2026-10-15, when the revisions began to be kept)
    /// is not played (the owner, 2026-10-17: its games' revision is not
    /// guessed).
    pub fn refusal(&self, revision: u8, oldest: u8) -> Option<Refusal> {
        let known = || {
            self.revisions
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
}

/// Why a made replay is not given to a game ([`Sidecar::refusal`]): its
/// status is [`Status::Incompatible`], and the lobby tells the player why
/// ([`Answer::refusal`]).
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

/// What a `ready` answer carries: the link's parts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ready {
    /// 32 lower-case hex digits.
    pub token: String,
    pub sha256: String,
    pub size: u64,
}

/// The player's latest answer ([`Replays::answer`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Answer {
    pub status: Status,
    /// The match asked for; 0 with [`Status::None`].
    pub match_id: u32,
    /// The file's size with [`Status::Ready`], else 0.
    pub size: u64,
    /// With [`Status::Preparing`], the replay's place in the conversion
    /// line: 1 being made now, 2 with one before it, ...; 0 when not known
    /// (its recording is still being written). Else 0.
    pub place: u32,
    pub ready: Option<Ready>,
    /// With [`Status::Incompatible`], why: in the first answer of the ask
    /// only (the lobby tells the player once each time it asks).
    pub refusal: Option<Refusal>,
}

impl Answer {
    /// The last field of the `R1` row (`CONTRACT-R1`): the place in line
    /// while `preparing`, else the size.
    pub fn figure(&self) -> u64 {
        if self.status == Status::Preparing {
            u64::from(self.place)
        } else {
            self.size
        }
    }

    /// The text of the `9E` that goes before a `ready` answer:
    /// `replay <id 8 hex> <token 32 hex> <sha256 64 hex> <size>`, the one
    /// form patch revision 14 passes to the updater (`CONTRACT-R1`).
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

/// What the lobby knows of the match itself (its results report, while it
/// keeps it): whether the asker played it, and how many places it had.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Known {
    pub played: bool,
    pub places: usize,
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

struct Ask {
    match_id: u32,
    at: Instant,
    status: Status,
    ready: Option<Ready>,
    /// Kept for asking again while it is `preparing`.
    revision: u8,
    known: Option<Known>,
    /// Why it is [`Status::Incompatible`], until an answer carries it.
    refusal: Option<Refusal>,
    /// When it was last looked at; `None`: at the next poll (the writer or
    /// the conversion has news of its match).
    checked: Option<Instant>,
}

/// Why a conversion failed ([`Replays::job_done`]): when it is tried again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    /// The mode's `.options` file is missing or unreadable: the operator
    /// may add it, so it is tried again after [`FAILED_FOR`].
    ModeFile,
    /// Reading or writing failed (a full disk): tried again after
    /// [`FAILED_FOR`], twice as long each time it fails again, up to
    /// [`FAILED_FOR_AT_MOST`].
    Disk,
    /// The recording itself (too large, damaged, of no known map, or the
    /// conversion's own error): it does not change, so it is not tried
    /// again while the server runs.
    Lasting,
}

/// A failed conversion: answered `failed` until `until` (`None`: for good).
struct Failed {
    until: Option<Instant>,
    /// Its [`Failure::Disk`] failures in a row.
    times: u32,
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
    tokens: HashMap<[u8; 16], Token>,
    asks: HashMap<u32, Ask>,
    jobs: VecDeque<u32>,
    converting: Option<u32>,
    /// Bounded by the kept recordings: a deleted one's entry goes.
    failed: HashMap<u32, Failed>,
    /// Recordings closed under the minimum of players, and recordings the
    /// storage cap deleted, the newest last.
    not_kept: VecDeque<u32>,
    deleted: VecDeque<u32>,
    /// The latest listing of the recordings' folder ([`LISTING_KEPT`]).
    listing: Option<Listing>,
    stopped: bool,
    /// Asks looked at, and folder listings made: for the tests.
    #[cfg(test)]
    evaluations: usize,
    #[cfg(test)]
    listings: usize,
}

impl Inner {
    /// The writer or the conversion has news of a match: its asks are
    /// looked at at their next poll, with the folder listed anew.
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
    /// The match whose replay the token downloads.
    pub match_id: u32,
}

impl Drop for Download<'_> {
    fn drop(&mut self) {
        if let Some(token) = self.replays.inner.locked().tokens.get_mut(&self.token) {
            token.downloading = false;
        }
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
/// server's conversion thread and the web page.
pub struct Replays {
    settings: Option<ReplaySettings>,
    /// The minimum of players a recording is kept for (`SRP_RECORD_MIN_PLAYERS`).
    min_players: usize,
    /// The oldest patch revision whose recordings are given
    /// ([`oldest_replayable`]).
    oldest: u8,
    inner: Mutex<Inner>,
    work: Condvar,
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

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

impl Replays {
    /// `None` settings: replays are off, every ask is answered `off`.
    pub fn new(settings: Option<ReplaySettings>, min_players: usize) -> Self {
        Self {
            settings,
            min_players,
            oldest: oldest_replayable_here(0),
            inner: Mutex::new(Inner::default()),
            work: Condvar::new(),
        }
    }

    /// The oldest patch revision whose recordings are given: the server's
    /// [`oldest_replayable_here`] (else the list's last number).
    pub fn with_oldest_revision(mut self, oldest: u8) -> Self {
        self.oldest = oldest;
        self
    }

    pub fn oldest_revision(&self) -> u8 {
        self.oldest
    }

    pub fn settings(&self) -> Option<&ReplaySettings> {
        self.settings.as_ref()
    }

    /// `SRP_REPLAY_DIR/<id:08x>.sur`: the only file `/replays/` serves,
    /// its name made here from the match id alone.
    pub fn sur_path(&self, match_id: u32) -> Option<PathBuf> {
        let dir = &self.settings.as_ref()?.replay_dir;
        Some(dir.join(format!("{}.sur", file_stem(match_id))))
    }

    /// Its sidecar: never served.
    pub fn json_path(&self, match_id: u32) -> Option<PathBuf> {
        let dir = &self.settings.as_ref()?.replay_dir;
        Some(dir.join(format!("{}.json", file_stem(match_id))))
    }

    /// Whether the recordings' folder has a match's kept recording
    /// (`rec026g_<UTC start>_<id:08x>.srprec`, a regular file) or one still
    /// being written (`.srprec.part`), by a listing at most
    /// [`LISTING_KEPT`] old: the folder holds a file for every match kept,
    /// and is not read for each look at an ask.
    fn recorded(&self, inner: &mut Inner, match_id: u32, now: Instant) -> Recorded {
        let Some(settings) = &self.settings else {
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
            inner.listing = Some(list_recordings(&settings.record_dir, now));
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

    /// A match's place in the conversion line: 1 being converted, 2 with one
    /// before it, ...; 0 when it is not in the line.
    fn place_in_line(inner: &Inner, match_id: u32) -> u32 {
        inner
            .converting
            .iter()
            .chain(inner.jobs.iter())
            .position(|&id| id == match_id)
            .map_or(0, |at| at as u32 + 1)
    }

    /// The sidecar of a replay whose `.sur` is there (a regular file of the
    /// size the sidecar says).
    fn made(&self, match_id: u32) -> Option<Sidecar> {
        let sidecar = Sidecar::parse(&std::fs::read_to_string(self.json_path(match_id)?).ok()?)?;
        let meta = std::fs::symlink_metadata(self.sur_path(match_id)?).ok()?;
        (sidecar.match_id == match_id && meta.is_file() && meta.len() == sidecar.size)
            .then_some(sidecar)
    }

    // ------------------------------------------------------- the lobby

    /// A player asks for a match's replay (`3D` with [`REPLAY_ASK`]).
    /// `revision`: its game's patch revision, if known; `known`: what the
    /// lobby knows of the match. The status is kept for its polls.
    pub fn ask(
        &self,
        account_id: u32,
        match_id: u32,
        revision: Option<u8>,
        known: Option<Known>,
        now: Instant,
    ) -> Status {
        let mut inner = self.inner.locked();
        inner
            .asks
            .retain(|_, ask| now.saturating_duration_since(ask.at) < ASK_KEPT);
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
                    revision: revision.unwrap_or(0),
                    known,
                    refusal: None,
                    checked: Some(at),
                },
            );
            return Status::Busy;
        }
        let revision = revision.unwrap_or(0);
        let (status, ready, refusal) =
            self.evaluate(&mut inner, account_id, match_id, revision, known, now);
        inner.asks.insert(
            account_id,
            Ask {
                match_id,
                at: now,
                status,
                ready,
                revision,
                known,
                refusal,
                checked: Some(now),
            },
        );
        status
    }

    /// The player's latest answer (`3B` with `REPLAY_START`): its own and
    /// no other's. One that was `preparing` is looked at again, once in
    /// [`RECHECK_EVERY`] at most (or at once after news of its match);
    /// polls in between get the same answer, the place in line made anew.
    pub fn answer(&self, account_id: u32, now: Instant) -> Answer {
        let mut inner = self.inner.locked();
        let Some(ask) = inner.asks.get(&account_id) else {
            return Answer {
                status: if self.settings.is_some() {
                    Status::None
                } else {
                    Status::Off
                },
                match_id: 0,
                size: 0,
                place: 0,
                ready: None,
                refusal: None,
            };
        };
        let (match_id, status, revision, known) =
            (ask.match_id, ask.status, ask.revision, ask.known);
        let due = ask
            .checked
            .is_none_or(|at| now.saturating_duration_since(at) >= RECHECK_EVERY);
        if status == Status::Preparing && due {
            let (status, ready, refusal) =
                self.evaluate(&mut inner, account_id, match_id, revision, known, now);
            if let Some(ask) = inner.asks.get_mut(&account_id) {
                ask.status = status;
                ask.ready = ready;
                ask.refusal = refusal;
                ask.checked = Some(now);
            }
        }
        let refusal = inner
            .asks
            .get_mut(&account_id)
            .and_then(|ask| ask.refusal.take());
        let ask = &inner.asks[&account_id];
        Answer {
            status: ask.status,
            match_id: ask.match_id,
            size: ask.ready.as_ref().map_or(0, |ready| ready.size),
            place: if ask.status == Status::Preparing {
                Self::place_in_line(&inner, ask.match_id)
            } else {
                0
            },
            ready: ask.ready.clone(),
            refusal,
        }
    }

    /// [`Self::look`], with why it is [`Status::Incompatible`] when it is.
    fn evaluate(
        &self,
        inner: &mut Inner,
        account_id: u32,
        match_id: u32,
        revision: u8,
        known: Option<Known>,
        now: Instant,
    ) -> (Status, Option<Ready>, Option<Refusal>) {
        match self.look(inner, account_id, match_id, revision, known, now) {
            Ok((status, ready)) => (status, ready, None),
            Err(refusal) => {
                log::info!(
                    "Replays: account {account_id} is not given match {match_id:#x} for its game \
                     revision {revision}: {refusal:?} (the oldest given: {})",
                    self.oldest
                );
                (Status::Incompatible, None, Some(refusal))
            }
        }
    }

    /// What an ask gets: its status (and the link's parts when ready), or
    /// why the replay made is not for the asker's game.
    fn look(
        &self,
        inner: &mut Inner,
        account_id: u32,
        match_id: u32,
        revision: u8,
        known: Option<Known>,
        now: Instant,
    ) -> Result<(Status, Option<Ready>), Refusal> {
        #[cfg(test)]
        {
            inner.evaluations += 1;
        }
        if self.settings.is_none() {
            return Ok((Status::Off, None));
        }
        if revision < MIN_REVISION {
            return Ok((Status::OldGame, None));
        }
        if known.is_some_and(|known| !known.played) {
            return Ok((Status::NotYours, None));
        }
        if let Some(sidecar) = self.made(match_id) {
            if !sidecar.account_ids.is_empty() && !sidecar.account_ids.contains(&account_id) {
                return Ok((Status::NotYours, None));
            }
            if let Some(refusal) = sidecar.refusal(revision, self.oldest) {
                return Err(refusal);
            }
            return Ok(match self.issue(inner, account_id, match_id, now) {
                Some(token) => {
                    log::info!(
                        "Replays: account {account_id} is given match {match_id:#x} ({} bytes), \
                         token {}…",
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
        if inner.converting == Some(match_id) || inner.jobs.contains(&match_id) {
            return Ok((Status::Preparing, None));
        }
        if inner
            .failed
            .get(&match_id)
            .is_some_and(|failed| failed.until.is_none_or(|until| now < until))
        {
            return Ok((Status::Failed, None));
        }
        if inner.deleted.contains(&match_id) {
            return Ok((Status::Gone, None));
        }
        if inner.not_kept.contains(&match_id) {
            return Ok((Status::NotRecorded, None));
        }
        Ok(match self.recorded(inner, match_id, now) {
            Recorded::Kept => {
                if inner.jobs.len() >= MAX_JOBS {
                    return Ok((Status::Busy, None));
                }
                // Converted when first asked for, not when its match ends.
                inner.jobs.push_back(match_id);
                self.work.notify_all();
                log::info!(
                    "Replays: match {match_id:#x} is converted for account {account_id} \
                     (place {} in line)",
                    Self::place_in_line(inner, match_id)
                );
                (Status::Preparing, None)
            }
            // Being written: it is kept or dropped when its match closes.
            Recorded::Writing => (Status::Preparing, None),
            Recorded::No => match known {
                Some(known) if known.places < self.min_players => (Status::NotRecorded, None),
                _ => (Status::Gone, None),
            },
        })
    }

    /// A new token for the account and the match: the account's live one
    /// for that match if it has one, else a new one while it has fewer
    /// than [`TOKENS_PER_ACCOUNT`] (a token past its time whose download
    /// is still under way counted). `None`: no room, or no randomness.
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
            log::error!("Replays: no randomness for a token: {error}");
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

    // ------------------------------------------------------- the web page

    /// A download of a token's replay: the token's one at a time (a `GET`
    /// or a `HEAD`) until the [`Download`] is dropped, so that one account
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

    // ------------------------------------------------------- the conversion

    /// The match server kept a match's recording. Its replay is made when
    /// a player first asks for it ([`Self::ask`]), not now: most matches'
    /// replays are never watched, and a conversion takes the CPU the
    /// matches share.
    pub fn recording_kept(&self, match_id: u32) {
        if self.settings.is_none() {
            return;
        }
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

    /// The storage cap deleted a match's recording: its replay goes too
    /// (a replay lives as long as its recording).
    pub fn recording_deleted(&self, match_id: u32) {
        {
            let mut inner = self.inner.locked();
            remember(&mut inner.deleted, match_id);
            inner.failed.remove(&match_id);
            inner.news_of(match_id);
        }
        self.delete_replay(match_id);
    }

    /// Deletes `<id>.sur` and `<id>.json` (names made here).
    pub fn delete_replay(&self, match_id: u32) {
        for path in [self.sur_path(match_id), self.json_path(match_id)]
            .into_iter()
            .flatten()
        {
            match std::fs::remove_file(&path) {
                Ok(()) => log::info!("Replays: {} deleted", path.display()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => log::warn!("Replays: can't delete {}: {error}", path.display()),
            }
        }
    }

    /// The conversions waiting, the next first.
    pub fn jobs(&self) -> Vec<u32> {
        self.inner.locked().jobs.iter().copied().collect()
    }

    /// The next match to convert, waiting for one; `None` once stopped.
    pub fn next_job(&self) -> Option<u32> {
        let mut inner = self.inner.locked();
        loop {
            if inner.stopped {
                return None;
            }
            if let Some(match_id) = inner.jobs.pop_front() {
                inner.converting = Some(match_id);
                return Some(match_id);
            }
            inner = self
                .work
                .wait(inner)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
    }

    /// The conversion of [`Self::next_job`]'s match is over: made, or
    /// failed and tried again as its [`Failure`] says. Its askers' next
    /// polls look at once.
    pub fn job_done(&self, match_id: u32, made: Result<(), Failure>, now: Instant) {
        let mut inner = self.inner.locked();
        if inner.converting == Some(match_id) {
            inner.converting = None;
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
            Failure::ModeFile => (Some(now + FAILED_FOR), 0),
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

    /// Unix seconds now, for a sidecar.
    pub fn now_unix() -> u64 {
        unix_now()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn settings(root: &Path) -> ReplaySettings {
        ReplaySettings {
            replay_dir: root.join("replays"),
            record_dir: root.join("recordings"),
            modes_dir: root.join("modes"),
            max_recording_bytes: 1 << 20,
            max_replay_bytes: 1 << 20,
        }
        .checked()
        .unwrap()
    }

    /// Replays in `root`, giving recordings from revision 11 on whatever the
    /// last row of `simulation_changes.tsv` is (the tests' recordings are of
    /// 11 to 15; the refusals of older ones have their own tests).
    fn desk(root: &Path) -> Replays {
        Replays::new(Some(settings(root)), 6).with_oldest_revision(11)
    }

    /// A download of `token` taken and given back at once: its match.
    fn take(replays: &Replays, token: &str, count: bool, now: Instant) -> Result<u32, Refused> {
        replays
            .take_download(token, count, now)
            .map(|download| download.match_id)
    }

    /// A kept recording of `match_id`, as the writer leaves it.
    fn kept(replays: &Replays, match_id: u32) {
        let dir = &replays.settings().unwrap().record_dir;
        std::fs::write(
            dir.join(format!("rec026g_20261015-120000_{match_id:08x}.srprec")),
            b"x",
        )
        .unwrap();
        replays.recording_kept(match_id);
    }

    /// The sidecar and `.sur` of `match_id`, for every account.
    fn made_for_all(replays: &Replays, match_id: u32) {
        std::fs::write(replays.sur_path(match_id).unwrap(), b"x").unwrap();
        let sidecar = Sidecar {
            match_id,
            size: 1,
            sha256: "0".repeat(64),
            revisions: vec![Some(14)],
            account_ids: Vec::new(),
            made_at: 0,
        };
        std::fs::write(replays.json_path(match_id).unwrap(), sidecar.to_json()).unwrap();
    }

    const MATCH: u32 = 0x1F07_C8FB;
    const ALICE: u32 = 0x1_0001;
    const BOB: u32 = 0x1_0002;

    /// A made replay of `MATCH` for ALICE, recorded by games of `revisions`.
    fn made(replays: &Replays, revisions: Vec<Option<u8>>) {
        let sur = replays.sur_path(MATCH).unwrap();
        std::fs::write(&sur, b"replay bytes").unwrap();
        let sidecar = Sidecar {
            match_id: MATCH,
            size: 12,
            sha256: "ab".repeat(32),
            revisions,
            account_ids: vec![ALICE],
            made_at: 1,
        };
        std::fs::write(replays.json_path(MATCH).unwrap(), sidecar.to_json()).unwrap();
    }

    fn played() -> Option<Known> {
        Some(Known {
            played: true,
            places: 10,
        })
    }

    #[test]
    fn status_words_are_the_contracts() {
        let all = [
            (Status::Preparing, "preparing"),
            (Status::Ready, "ready"),
            (Status::None, "none"),
            (Status::NotRecorded, "not_recorded"),
            (Status::Gone, "gone"),
            (Status::NotYours, "not_yours"),
            (Status::OldGame, "old_game"),
            (Status::Incompatible, "incompatible"),
            (Status::Busy, "busy"),
            (Status::Failed, "failed"),
            (Status::Off, "off"),
        ];
        for (status, word) in all {
            assert_eq!(status.word(), word);
        }
    }

    #[test]
    fn the_replay_folder_is_never_the_recordings_or_inside_or_around_it() {
        let root = temp("overlap");
        let overlap = |replay: PathBuf, record: PathBuf| {
            ReplaySettings {
                replay_dir: replay,
                record_dir: record,
                modes_dir: root.join("modes"),
                max_recording_bytes: 1,
                max_replay_bytes: 1,
            }
            .checked()
        };
        assert_eq!(
            overlap(root.join("a"), root.join("a")),
            Err(FolderError::Overlap)
        );
        assert_eq!(
            overlap(root.join("a").join("b"), root.join("a")),
            Err(FolderError::Overlap)
        );
        assert_eq!(
            overlap(root.join("a"), root.join("a").join("b")),
            Err(FolderError::Overlap)
        );
        // The same folder by another spelling.
        assert_eq!(
            overlap(root.join("a").join("..").join("a"), root.join("a")),
            Err(FolderError::Overlap)
        );
        assert!(overlap(root.join("replays"), root.join("recordings")).is_ok());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn replays_off_answer_off_and_give_no_path() {
        let replays = Replays::new(None, 6);
        let now = Instant::now();
        assert_eq!(replays.answer(ALICE, now).status, Status::Off);
        assert_eq!(
            replays.ask(ALICE, MATCH, Some(14), played(), now),
            Status::Off
        );
        assert_eq!(replays.sur_path(MATCH), None);
        assert_eq!(
            take(&replays, &"0".repeat(32), true, now),
            Err(Refused::Unknown)
        );
    }

    #[test]
    fn a_made_replay_is_ready_with_a_token_of_five_downloads_for_ten_minutes() {
        let root = temp("ready");
        let replays = desk(&root);
        made(&replays, vec![Some(12), None, Some(11)]);
        let now = Instant::now();
        assert_eq!(replays.answer(ALICE, now).status, Status::None);
        assert_eq!(
            replays.ask(ALICE, MATCH, Some(14), played(), now),
            Status::Ready
        );
        let answer = replays.answer(ALICE, now);
        assert_eq!(
            (answer.status, answer.match_id, answer.size),
            (Status::Ready, MATCH, 12)
        );
        let ready = answer.ready.clone().unwrap();
        assert_eq!(ready.token.len(), 32);
        assert!(parse_token(&ready.token).is_some());
        let link = answer.link().unwrap();
        assert_eq!(
            link,
            format!("replay 1f07c8fb {} {} 12", ready.token, "ab".repeat(32))
        );
        // 114 + the size's digits ("replay " 7, 8, 32 and 64 hex digits,
        // three spaces), within the client's 245 units at any size.
        assert_eq!(link.len(), 114 + 2);
        // Another player's poll is its own.
        assert_eq!(replays.answer(BOB, now).status, Status::None);
        // Five downloads, HEADs free.
        for _ in 0..5 {
            assert_eq!(take(&replays, &ready.token, false, now), Ok(MATCH));
            assert_eq!(take(&replays, &ready.token, true, now), Ok(MATCH));
        }
        assert_eq!(
            take(&replays, &ready.token, true, now),
            Err(Refused::Unknown)
        );
        assert_eq!(
            take(&replays, &ready.token, false, now),
            Err(Refused::Unknown)
        );
        // A new ask gives a new token, which ends with its ten minutes.
        let later = now + ASK_EVERY;
        assert_eq!(
            replays.ask(ALICE, MATCH, Some(14), played(), later),
            Status::Ready
        );
        let second = replays.answer(ALICE, later).ready.unwrap().token;
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
            format!("{}.", "0".repeat(31)),
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
        let replays = desk(&root);
        let mut now = Instant::now();
        let mut matches = Vec::new();
        for n in 0..4u32 {
            let match_id = 0x100 + n;
            std::fs::write(replays.sur_path(match_id).unwrap(), b"x").unwrap();
            let sidecar = Sidecar {
                match_id,
                size: 1,
                sha256: "0".repeat(64),
                revisions: vec![Some(14)],
                account_ids: Vec::new(),
                made_at: 0,
            };
            std::fs::write(replays.json_path(match_id).unwrap(), sidecar.to_json()).unwrap();
            matches.push(match_id);
        }
        for &match_id in &matches[..3] {
            assert_eq!(
                replays.ask(ALICE, match_id, Some(14), None, now),
                Status::Ready
            );
            // Asked again at once: it stands; another match: busy.
            assert_eq!(
                replays.ask(ALICE, match_id, Some(14), None, now),
                Status::Ready
            );
            assert_eq!(replays.ask(ALICE, 0x999, Some(14), None, now), Status::Busy);
            assert_eq!(replays.answer(ALICE, now).status, Status::Busy);
            now += ASK_EVERY;
        }
        assert_eq!(
            replays.ask(ALICE, matches[3], Some(14), None, now),
            Status::Busy
        );
        // The same match again keeps its live token.
        now += ASK_EVERY;
        assert_eq!(
            replays.ask(ALICE, matches[0], Some(14), None, now),
            Status::Ready
        );
        // Another account has its own three.
        assert_eq!(
            replays.ask(BOB, matches[3], Some(14), None, now),
            Status::Ready
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn an_ask_is_refused_for_an_old_game_another_players_match_and_other_revisions() {
        let root = temp("refused");
        let replays = desk(&root);
        made(&replays, vec![Some(14), Some(15)]);
        let mut now = Instant::now();
        let mut ask = |account, revision, known| {
            now += ASK_EVERY;
            replays.ask(account, MATCH, revision, known, now)
        };
        assert_eq!(ask(ALICE, Some(11), played()), Status::OldGame);
        // Revisions 12 and 13 (the patches' mount, the effects' scrolling)
        // have no replay link.
        assert_eq!(ask(ALICE, Some(12), played()), Status::OldGame);
        assert_eq!(ask(ALICE, Some(13), played()), Status::OldGame);
        assert_eq!(ask(ALICE, None, played()), Status::OldGame);
        // The lobby's report says so, or the sidecar does.
        assert_eq!(
            ask(
                ALICE,
                Some(14),
                Some(Known {
                    played: false,
                    places: 10
                })
            ),
            Status::NotYours
        );
        assert_eq!(ask(BOB, Some(15), None), Status::NotYours);
        // Made by a revision 15 game: not for 14.
        assert_eq!(ask(ALICE, Some(14), played()), Status::Incompatible);
        assert_eq!(ask(ALICE, Some(15), played()), Status::Ready);
        // No token was given for any of the refusals.
        assert_eq!(replays.inner.locked().tokens.len(), 1);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn revisions_older_than_eleven_and_newer_than_the_askers_do_not_play() {
        let sidecar = |revisions: Vec<Option<u8>>| Sidecar {
            match_id: 1,
            size: 1,
            sha256: "0".repeat(64),
            revisions,
            account_ids: Vec::new(),
            made_at: 0,
        };
        // No revision kept (before 2026-10-15), or none known: not played.
        assert_eq!(sidecar(Vec::new()).refusal(14, 11), Some(Refusal::Unknown));
        assert_eq!(
            sidecar(vec![None, Some(0)]).refusal(14, 11),
            Some(Refusal::Unknown)
        );
        // Every recorded revision between 11 and the asker's own: 12 and
        // 13 compute a match as 11 and 14 do.
        assert_eq!(
            sidecar(vec![None, Some(0), Some(11), Some(12), Some(14)]).refusal(14, 11),
            None
        );
        assert_eq!(
            sidecar(vec![Some(10), Some(14)]).refusal(14, 11),
            Some(Refusal::Older {
                from: 10,
                to: 14,
                oldest: 11
            })
        );
        assert_eq!(
            sidecar(vec![Some(15)]).refusal(14, 11),
            Some(Refusal::Newer { from: 15, to: 15 })
        );
        assert_eq!(sidecar(vec![Some(15)]).refusal(16, 11), None);
        // A release that changed the simulation, 17: 16's recordings go.
        assert_eq!(
            sidecar(vec![Some(16), Some(16)]).refusal(17, 17),
            Some(Refusal::Older {
                from: 16,
                to: 16,
                oldest: 17
            })
        );
        assert_eq!(sidecar(vec![Some(17)]).refusal(18, 17), None);
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
    fn a_tag_is_named_for_the_releases_that_have_one() {
        assert_eq!(release_tag(16).as_deref(), Some("026g-rev16"));
        assert_eq!(release_tag(12).as_deref(), Some("026g-rev12"));
        for untagged in [11, 13, 14] {
            assert_eq!(release_tag(untagged), None);
        }
    }

    #[test]
    fn a_refused_replay_says_why_once_an_ask_and_gives_no_token() {
        let root = temp("why");
        let replays = Replays::new(Some(settings(&root)), 6).with_oldest_revision(17);
        made(&replays, vec![Some(16), None]);
        let mut now = Instant::now();
        assert_eq!(
            replays.ask(ALICE, MATCH, Some(17), played(), now),
            Status::Incompatible
        );
        let first = replays.answer(ALICE, now);
        assert_eq!(first.status, Status::Incompatible);
        assert_eq!(
            first.refusal,
            Some(Refusal::Older {
                from: 16,
                to: 16,
                oldest: 17
            })
        );
        assert!(first.ready.is_none() && first.link().is_none());
        // The movie's next polls: the status, the why said already.
        let again = replays.answer(ALICE, now);
        assert_eq!((again.status, again.refusal), (Status::Incompatible, None));
        // The same ask twice at once stands; asked again later, it is said
        // again.
        assert_eq!(
            replays.ask(ALICE, MATCH, Some(17), played(), now),
            Status::Incompatible
        );
        assert_eq!(replays.answer(ALICE, now).refusal, None);
        now += ASK_EVERY;
        replays.ask(ALICE, MATCH, Some(17), played(), now);
        assert!(replays.answer(ALICE, now).refusal.is_some());
        // A recording with no revision at all: refused too.
        made(&replays, Vec::new());
        now += ASK_EVERY;
        replays.ask(ALICE, MATCH, Some(17), played(), now);
        assert_eq!(replays.answer(ALICE, now).refusal, Some(Refusal::Unknown));
        assert!(replays.inner.locked().tokens.is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_replay_made_while_the_asker_waits_says_why_it_is_refused() {
        let root = temp("why-later");
        let replays = desk(&root);
        kept(&replays, MATCH);
        let now = Instant::now();
        assert_eq!(
            replays.ask(ALICE, MATCH, Some(16), played(), now),
            Status::Preparing
        );
        assert_eq!(replays.answer(ALICE, now).refusal, None);
        assert_eq!(replays.next_job(), Some(MATCH));
        made(&replays, Vec::new());
        replays.job_done(MATCH, Ok(()), now);
        let answer = replays.answer(ALICE, now);
        assert_eq!(
            (answer.status, answer.refusal),
            (Status::Incompatible, Some(Refusal::Unknown))
        );
        assert_eq!(replays.answer(ALICE, now).refusal, None);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_recording_without_a_replay_is_queued_once_and_answered_when_made() {
        let root = temp("queue");
        let replays = desk(&root);
        let record_dir = replays.settings().unwrap().record_dir.clone();
        let now = Instant::now();
        // Nothing at all: gone; a known match of 4 places: not recorded.
        assert_eq!(
            replays.ask(ALICE, MATCH, Some(14), played(), now),
            Status::Gone
        );
        assert_eq!(
            replays.ask(
                BOB,
                MATCH,
                Some(14),
                Some(Known {
                    played: true,
                    places: 4
                }),
                now
            ),
            Status::NotRecorded
        );
        // A file of another name pattern is no recording of the match.
        std::fs::write(record_dir.join("x_1f07c8fb.srprec"), b"x").unwrap();
        std::fs::write(record_dir.join("rec026g_1_1f07c8fb.srprec.part"), b"x").unwrap();
        // (Seen once the folder is listed anew.)
        let later = now + LISTING_KEPT;
        assert_eq!(
            replays.ask(ALICE, MATCH, Some(14), played(), later),
            Status::Preparing
        );
        assert!(replays.inner.locked().jobs.is_empty());
        // Still being written: its place in line is not known.
        assert_eq!(replays.answer(ALICE, later).figure(), 0);
        // The writer keeps it: nothing is converted until someone asks.
        std::fs::write(record_dir.join("rec026g_2_1f07c8fb.srprec"), b"x").unwrap();
        replays.recording_kept(MATCH);
        assert!(replays.jobs().is_empty());
        let later = later + ASK_EVERY;
        assert_eq!(
            replays.ask(ALICE, MATCH, Some(14), played(), later),
            Status::Preparing
        );
        assert_eq!(
            replays.ask(BOB, MATCH, Some(14), played(), later),
            Status::Preparing
        );
        assert_eq!(replays.inner.locked().jobs, [MATCH]);
        assert_eq!(replays.next_job(), Some(MATCH));
        let answer = replays.answer(ALICE, later);
        assert_eq!((answer.status, answer.place), (Status::Preparing, 1));
        made(&replays, vec![Some(14)]);
        replays.job_done(MATCH, Ok(()), later);
        // The conversion's news is looked at by the next poll at once.
        let answer = replays.answer(ALICE, later);
        assert_eq!(answer.status, Status::Ready);
        assert_eq!(answer.figure(), 12);
        assert!(answer.ready.is_some());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_failed_or_deleted_or_unkept_recording_says_so() {
        let root = temp("fates");
        let replays = desk(&root);
        kept(&replays, MATCH);
        let now = Instant::now();
        assert_eq!(
            replays.ask(ALICE, MATCH, Some(14), played(), now),
            Status::Preparing
        );
        assert_eq!(replays.next_job(), Some(MATCH));
        replays.job_done(MATCH, Err(Failure::Lasting), now);
        assert_eq!(replays.answer(ALICE, now).status, Status::Failed);
        assert_eq!(
            replays.ask(BOB, MATCH, Some(14), played(), now),
            Status::Failed
        );
        made(&replays, vec![Some(14)]);
        replays.recording_deleted(MATCH);
        assert!(!replays.sur_path(MATCH).unwrap().exists());
        assert!(!replays.json_path(MATCH).unwrap().exists());
        let later = now + FAILED_FOR;
        assert_eq!(
            replays.ask(ALICE, MATCH, Some(14), played(), later),
            Status::Gone
        );
        replays.recording_not_kept(7);
        assert_eq!(
            replays.ask(ALICE, 7, Some(14), played(), later + ASK_EVERY),
            Status::NotRecorded
        );
        replays.stop();
        assert_eq!(replays.next_job(), None);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_failure_is_tried_again_only_when_its_cause_can_change() {
        let root = temp("retry");
        let replays = desk(&root);
        let start = Instant::now();
        let mut at = start;
        // Asks of one account, each past the two seconds.
        let ask = |match_id: u32, now: Instant| {
            let status = replays.ask(ALICE, match_id, Some(14), played(), now);
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
        // The mode's file: again after ten minutes, each time.
        kept(&replays, 2);
        at += ASK_EVERY;
        assert_eq!(ask(2, at), (Status::Preparing, true));
        for _ in 0..3 {
            fail(2, Failure::ModeFile, at);
            at += FAILED_FOR - ASK_EVERY;
            assert_eq!(ask(2, at), (Status::Failed, false));
            at += ASK_EVERY;
            assert_eq!(ask(2, at), (Status::Preparing, true));
        }
        // Disk: ten minutes, then twenty, forty, ... four hours at most.
        assert_eq!(replays.next_job(), Some(2));
        replays.job_done(2, Ok(()), at);
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
        // Made at last: its failures are forgotten.
        assert_eq!(replays.next_job(), Some(3));
        replays.job_done(3, Ok(()), at);
        assert!(!replays.inner.locked().failed.contains_key(&3));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_burst_of_polls_is_looked_at_once_a_second_and_the_folder_listed_once() {
        let root = temp("burst");
        let replays = desk(&root);
        let record_dir = replays.settings().unwrap().record_dir.clone();
        // A thousand other recordings in the folder.
        for n in 0..1000u32 {
            std::fs::write(
                record_dir.join(format!("rec026g_20261015-120000_{:08x}.srprec", 0x1000 + n)),
                b"",
            )
            .unwrap();
        }
        std::fs::write(
            record_dir.join(format!("rec026g_20261015-120000_{MATCH:08x}.srprec.part")),
            b"x",
        )
        .unwrap();
        let now = Instant::now();
        assert_eq!(
            replays.ask(ALICE, MATCH, Some(14), played(), now),
            Status::Preparing
        );
        let counts = |replays: &Replays| {
            let inner = replays.inner.locked();
            (inner.evaluations, inner.listings)
        };
        assert_eq!(counts(&replays), (1, 1));
        // 80 polls in one datagram, and more within the second: the ask's
        // look stands.
        for step in 0..80u64 {
            let at = now + Duration::from_millis(step * 12);
            assert_eq!(replays.answer(ALICE, at).status, Status::Preparing);
        }
        assert_eq!(counts(&replays), (1, 1));
        // A second on: one look for the whole burst.
        let next = now + RECHECK_EVERY;
        for _ in 0..80 {
            assert_eq!(replays.answer(ALICE, next).status, Status::Preparing);
        }
        assert_eq!(counts(&replays), (2, 1));
        // Another account's burst is its own one look, on the same listing.
        assert_eq!(
            replays.ask(BOB, MATCH, Some(14), played(), next),
            Status::Preparing
        );
        for _ in 0..80 {
            replays.answer(BOB, next);
        }
        assert_eq!(counts(&replays), (3, 1));
        // The listing is made anew when it is old, or on the writer's news.
        let later = now + LISTING_KEPT;
        replays.answer(ALICE, later);
        assert_eq!(counts(&replays), (4, 2));
        std::fs::rename(
            record_dir.join(format!("rec026g_20261015-120000_{MATCH:08x}.srprec.part")),
            record_dir.join(format!("rec026g_20261015-120000_{MATCH:08x}.srprec")),
        )
        .unwrap();
        replays.recording_kept(MATCH);
        let answer = replays.answer(ALICE, later);
        assert_eq!((answer.status, answer.place), (Status::Preparing, 1));
        assert_eq!(counts(&replays), (5, 3));
        assert_eq!(replays.jobs(), [MATCH]);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn askers_of_one_match_share_its_one_job_and_each_gets_its_own_token() {
        let root = temp("shared");
        let replays = desk(&root);
        kept(&replays, MATCH);
        let now = Instant::now();
        let players: Vec<u32> = (0..20).map(|n| 0x2_0000 + n).collect();
        for &player in &players {
            assert_eq!(
                replays.ask(player, MATCH, Some(14), None, now),
                Status::Preparing
            );
        }
        // One job for all of them, all at the same place.
        assert_eq!(replays.jobs(), [MATCH]);
        for &player in &players {
            assert_eq!(replays.answer(player, now).figure(), 1);
        }
        // The conversion goes on with no one polling; asked again while it
        // runs, the match is not queued twice.
        assert_eq!(replays.next_job(), Some(MATCH));
        let later = now + ASK_EVERY;
        assert_eq!(
            replays.ask(players[0], MATCH, Some(14), None, later),
            Status::Preparing
        );
        assert!(replays.jobs().is_empty());
        made_for_all(&replays, MATCH);
        replays.job_done(MATCH, Ok(()), later);
        let mut tokens = HashSet::new();
        for &player in &players {
            let answer = replays.answer(player, later);
            assert_eq!(answer.status, Status::Ready);
            tokens.insert(answer.ready.unwrap().token);
        }
        assert_eq!(tokens.len(), players.len());
        // All of them download the one file at once, a slot each.
        let downloads: Vec<Download<'_>> = tokens
            .iter()
            .map(|token| replays.take_download(token, true, later).unwrap())
            .collect();
        assert!(downloads.iter().all(|download| download.match_id == MATCH));
        // A later ask is answered from the file, with no job.
        let late = 0x3_0000;
        assert_eq!(
            replays.ask(late, MATCH, Some(14), None, later),
            Status::Ready
        );
        assert!(replays.jobs().is_empty());
        drop(downloads);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn other_matches_wait_in_line_up_to_twenty_and_a_busy_asker_is_queued_later() {
        let root = temp("line");
        let replays = desk(&root);
        let now = Instant::now();
        let first = 0x500;
        for n in 0..=MAX_JOBS as u32 + 1 {
            kept(&replays, first + n);
        }
        // The first is being converted; the others wait behind it.
        assert_eq!(
            replays.ask(1, first, Some(14), None, now),
            Status::Preparing
        );
        assert_eq!(replays.next_job(), Some(first));
        for n in 1..=MAX_JOBS as u32 {
            assert_eq!(
                replays.ask(1 + n, first + n, Some(14), None, now),
                Status::Preparing
            );
            assert_eq!(replays.answer(1 + n, now).figure(), u64::from(n) + 1);
        }
        assert_eq!(replays.answer(1, now).figure(), 1);
        assert_eq!(replays.jobs().len(), MAX_JOBS);
        // One more is busy, and not queued.
        let past = first + MAX_JOBS as u32 + 1;
        assert_eq!(replays.ask(99, past, Some(14), None, now), Status::Busy);
        assert_eq!(replays.answer(99, now).figure(), 0);
        assert_eq!(replays.jobs().len(), MAX_JOBS);
        // The line moves: each place one less.
        replays.job_done(first, Ok(()), now);
        assert_eq!(replays.next_job(), Some(first + 1));
        assert_eq!(replays.answer(2, now).figure(), 1);
        assert_eq!(replays.answer(3, now).figure(), 2);
        // The busy asker, asking again, is queued at the end.
        let later = now + ASK_EVERY;
        assert_eq!(
            replays.ask(99, past, Some(14), None, later),
            Status::Preparing
        );
        assert_eq!(replays.answer(99, later).figure(), MAX_JOBS as u64 + 1);
        assert_eq!(replays.jobs().last(), Some(&past));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_token_downloads_one_at_a_time_and_an_account_three_at_most() {
        let root = temp("one-at-a-time");
        let replays = desk(&root);
        let mut now = Instant::now();
        let mut tokens = Vec::new();
        for match_id in 0x100..0x103 {
            made_for_all(&replays, match_id);
            assert_eq!(
                replays.ask(ALICE, match_id, Some(14), None, now),
                Status::Ready
            );
            tokens.push(replays.answer(ALICE, now).ready.unwrap().token);
            now += ASK_EVERY;
        }
        let held: Vec<Download<'_>> = tokens
            .iter()
            .map(|token| replays.take_download(token, true, now).unwrap())
            .collect();
        // A second download of a token waits for the first, a HEAD too,
        // and takes none of its downloads.
        for token in &tokens {
            for count in [true, false] {
                assert_eq!(take(&replays, token, count, now), Err(Refused::Busy));
            }
        }
        // Past their ten minutes, while downloading, they still count: no
        // fourth token for the account.
        let late = now + TOKEN_LIFETIME;
        made_for_all(&replays, 0x103);
        assert_eq!(
            replays.ask(ALICE, 0x103, Some(14), None, late),
            Status::Busy
        );
        for token in &tokens {
            assert_eq!(take(&replays, token, true, late), Err(Refused::Unknown));
        }
        drop(held);
        // Done: the tokens go, and the account gets a new one.
        assert_eq!(
            replays.ask(ALICE, 0x103, Some(14), None, late + ASK_EVERY),
            Status::Ready
        );
        // Given back, a token downloads again, its four left.
        let token = replays.answer(ALICE, late + ASK_EVERY).ready.unwrap().token;
        for n in 1..=TOKEN_DOWNLOADS {
            let download = replays.take_download(&token, true, late).unwrap();
            // The last leaves it used up: unknown from then on.
            let other = if n < TOKEN_DOWNLOADS {
                Refused::Busy
            } else {
                Refused::Unknown
            };
            assert_eq!(take(&replays, &token, true, late), Err(other));
            drop(download);
        }
        assert_eq!(take(&replays, &token, true, late), Err(Refused::Unknown));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_replay_whose_file_is_not_its_sidecars_is_not_given() {
        let root = temp("mismatch");
        let replays = desk(&root);
        made(&replays, vec![Some(14)]);
        std::fs::write(replays.sur_path(MATCH).unwrap(), b"longer than it was").unwrap();
        let now = Instant::now();
        assert_eq!(
            replays.ask(ALICE, MATCH, Some(14), played(), now),
            Status::Gone
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn only_eight_hex_digit_names_are_replay_files() {
        assert_eq!(match_of_name("1f07c8fb.sur", ".sur"), Some(MATCH));
        assert_eq!(file_stem(MATCH), "1f07c8fb");
        assert_eq!(file_stem(7), "00000007");
        for bad in [
            "1F07C8FB.sur",
            "1f07c8f.sur",
            "01f07c8fb.sur",
            "1f07c8fb.json",
            "../1f07c8f.sur",
            "1f07c8fb.sur.part",
        ] {
            assert_eq!(match_of_name(bad, ".sur"), None, "{bad}");
        }
    }
}
