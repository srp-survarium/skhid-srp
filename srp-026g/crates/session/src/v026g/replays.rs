//! What the lobby and the match server share of the replays, in memory:
//! the replay sessions ([`super::replay_watch`], `docs/notes/replay-stream.md`)
//! and what a list row says of a match's recording ([`Replays::state`]).
//!
//! - The match server's recordings' writer (`SRP_RECORD_DIR`) tells of each
//!   file it keeps, drops under the minimum of players or deletes for the
//!   storage cap ([`Replays::recording_kept`], ...).
//! - Which games may play a recording: the simulation changes
//!   ([`SIMULATION_CHANGES`], [`oldest_replayable`]) and [`refusal_of`].

use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use super::replay_watch::{WatchSettings, Watches};
use crate::LockExt;

/// The `start` of the `3B` that polls for the answer to a replay
/// session's ask.
pub use survarium::v026g::lobby::REPLAY_START;
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

/// The recordings' folder is listed at most this often for the list rows;
/// the writer's news of a file (kept, dropped, deleted) has it listed anew.
const LISTING_KEPT: Duration = Duration::from_secs(5);
/// Match ids remembered as not kept or deleted, at most.
const REMEMBERED: usize = 4096;

/// The status word of an `R1` answer (`CONTRACT-R1`): what came of the
/// player's latest ask.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// Asked, being read: the movie polls on.
    Preparing,
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
    /// The player is in a match or queued, or too many sessions run.
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

/// The replays: one for the srp process, shared by the lobby and the
/// match server.
pub struct Replays {
    /// `SRP_RECORD_DIR`; `None`: recordings are off.
    record_dir: Option<PathBuf>,
    /// The oldest patch revision whose recordings are given
    /// ([`oldest_replayable`]).
    oldest: u8,
    inner: Mutex<Inner>,
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
            oldest: oldest_replayable_here(0),
            inner: Mutex::new(Inner::default()),
            watches: Watches::new(None),
        }
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

    /// The match server kept a match's recording.
    pub fn recording_kept(&self, match_id: u32) {
        let mut inner = self.inner.locked();
        inner.not_kept.retain(|&id| id != match_id);
        inner.deleted.retain(|&id| id != match_id);
        inner.listing = None;
    }

    /// A match's recording was closed under the minimum of players.
    pub fn recording_not_kept(&self, match_id: u32) {
        let mut inner = self.inner.locked();
        remember(&mut inner.not_kept, match_id);
        inner.listing = None;
    }

    /// The storage cap deleted a match's recording.
    pub fn recording_deleted(&self, match_id: u32) {
        let mut inner = self.inner.locked();
        remember(&mut inner.deleted, match_id);
        inner.listing = None;
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
        assert_eq!(ReplayId::parse(id.as_str()), Some(id));
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
}
