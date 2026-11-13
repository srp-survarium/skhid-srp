//! The replays' desk of the srp process ([`replays_from_env`]) and what the
//! replay sessions (`game::playback`, `docs/notes/replay-stream.md`) read
//! of the recordings ([`super::record`]): a match's kept recording, and its
//! places' accounts and game revisions.
//!
//! Save (`research/local-replays-design.md` section 1): one thread
//! ([`start`]) makes the files the desk queues (`Replays::save_ask`), one
//! at a time, at the lowest priority ([`lowest_priority`]): a kept
//! recording a player asked to save, the first time anyone does. For each
//! it writes, in `SRP_REPLAY_DIR`:
//!
//! - `<id:08x>.srpreplay` ([`make`]): the container of
//!   [`super::replay_file`], the `.sur` a replay session plays of the
//!   recording (`game::playback::sur_of`, checked to load as a session's),
//!   the history's events of its timeline, and a header made of the 0xE4
//!   options and those events;
//! - `<id:08x>.json`, the sidecar (`replays::Sidecar`): the file's size and
//!   SHA-256, each place's revision and account. Never served.
//!
//! Each goes under `.part` first and gets its name when whole, the sidecar
//! last: a sidecar means its file is complete. A file lives as long as its
//! recording (the writer's cap deletes both, `Replays::recording_deleted`;
//! at start every file whose recording is gone is deleted, [`prune`]), and
//! the files over `SRP_REPLAY_MAX_MB` together go, the oldest first
//! ([`cap`]); one that went is made again when asked for.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use session::v026g::GameAccounts;
use session::v026g::persist::match_events::{EventKind, MatchEvent};
use session::v026g::replay_watch::{Access, DEFAULT_MAX_RECORDING_MB, WatchSettings};
use session::v026g::replays::{
    self, FILE_EXTENSION, Failure, ReplayId, Replays, SaveSettings, Sidecar, file_stem,
};
use survarium::v026g::dictionary::Dictionary;
use survarium::v026g::localization::{self, Language};
use survarium::v026g::match_msgs::static_info::{self, NO_LOCAL_PLACE};

use super::game::playback::{Recording, sur_of, timeline_of};
use super::record::{self, NOTE};
use super::replay_file::{Header, Place, ReplayFile};
use super::sur;

const PART: &str = ".part";

/// The replays of the srp process: the recordings of `SRP_RECORD_DIR`
/// (`SRP_RECORD_MIN_PLAYERS` and the rest, [`record::RecordSettings`]) and
/// the replay sessions' settings ([`WatchSettings::from_env`]); off, and
/// said in the log, without recordings.
pub fn replays_from_env() -> Replays {
    let Some(record) = record::RecordSettings::from_env() else {
        log::info!("Replay sessions are off (SRP_RECORD_DIR)");
        return Replays::new(None);
    };
    // A simulation change not recorded yet (`next`) is this release's own
    // revision: the one the release set as the server's minimum.
    let min_patch_revision = std::env::var("SRP_MIN_PATCH_REVISION")
        .ok()
        .and_then(|value| value.trim().parse::<u8>().ok())
        .unwrap_or(0);
    let oldest = replays::oldest_replayable_here(min_patch_revision);
    let pending = replays::simulation_changes(replays::SIMULATION_CHANGES)
        .ok()
        .and_then(|changes| changes.last().copied())
        .filter(|row| row.revision.is_none());
    if let Some(pending) = pending
        && min_patch_revision == 0
    {
        log::warn!(
            "Replays: a change of what a match computes is not released yet (`next`: {}), \
             and SRP_MIN_PATCH_REVISION is not set: it is not counted (a server of no \
             release); a release sets it",
            pending.what
        );
    }
    log::info!(
        "Replays: recordings of games of patch revision {oldest} and later are given (the \
         last change of what a match computes); older ones are refused, said to the player"
    );
    let watch = WatchSettings::from_env();
    log::info!(
        "Replay sessions: {} at once, recordings up to {} MB; watched by {}",
        watch.max_sessions,
        watch.max_recording_bytes >> 20,
        match watch.access {
            Access::Match => format!(
                "a match's players and {} moderators",
                watch.moderators.len()
            ),
            Access::All => "anyone".to_owned(),
        }
    );
    let save = SaveSettings::from_env(&record.dir);
    match &save {
        Some(save) => log::info!(
            "Save: files made in {}; over {} MB together the oldest go",
            save.replay_dir.display(),
            save.max_replay_bytes >> 20
        ),
        None => log::info!("Save is off (SRP_REPLAY_DIR)"),
    }
    let replays = Replays::new(Some(record.dir))
        .with_oldest_revision(oldest)
        .with_watch(watch);
    match save {
        Some(save) => replays.with_save(save),
        None => replays,
    }
}

/// Starts Save's thread, after deleting what is left of a stopped server,
/// the files of recordings that are gone and the oldest over the cap; its
/// events come from the history of `accounts`. `None` when Save is off.
pub fn start(
    replays: Arc<Replays>,
    accounts: Option<Arc<GameAccounts>>,
) -> Option<io::Result<JoinHandle<()>>> {
    let save = replays.save_settings()?.clone();
    let record_dir = replays.record_dir()?.to_owned();
    prune(&save, &record_dir);
    cap(&save, None);
    Some(
        std::thread::Builder::new()
            .name("srp-save".into())
            .spawn(move || {
                lowest_priority();
                while let Some(match_id) = replays.next_job() {
                    let started = Instant::now();
                    let made = make(&replays, accounts.as_deref(), match_id);
                    match &made {
                        Ok(sidecar) => {
                            log::info!(
                                "Save: match {match_id:#x} made in {} ms: {} bytes, SHA-256 \
                                 {}...",
                                started.elapsed().as_millis(),
                                sidecar.size,
                                &sidecar.sha256[..12]
                            );
                            cap(&save, Some(match_id));
                        }
                        Err(error) => log::warn!(
                            "Save: match {match_id:#x} not made: {} ({})",
                            error.why,
                            match error.failure {
                                Failure::Disk => "tried again later",
                                Failure::Lasting => "not tried again until a restart",
                            }
                        ),
                    }
                    let made = made.map(|_| ()).map_err(|error| error.failure);
                    replays.job_done(match_id, made, Instant::now());
                }
            }),
    )
}

/// Why a file was not made: the log's words, and when it is tried again.
#[derive(Debug, PartialEq, Eq)]
pub struct MakeError {
    pub why: String,
    pub failure: Failure,
}

impl MakeError {
    fn lasting(why: impl Into<String>) -> Self {
        Self {
            why: why.into(),
            failure: Failure::Lasting,
        }
    }

    fn disk(why: impl Into<String>) -> Self {
        Self {
            why: why.into(),
            failure: Failure::Disk,
        }
    }

    /// A reading error: of the file itself (damaged) or of the disk.
    fn read(error: io::Error) -> Self {
        if error.kind() == io::ErrorKind::InvalidData {
            Self::lasting(error.to_string())
        } else {
            Self::disk(error.to_string())
        }
    }
}

/// The kept recording of `match_id` made into its file and sidecar, with
/// the history's events of `accounts` (none without one). Its size against
/// `SRP_REPLAY_MAX_RECORDING_MB` first, as a replay session reads it.
pub fn make(
    replays: &Replays,
    accounts: Option<&GameAccounts>,
    match_id: u32,
) -> Result<Sidecar, MakeError> {
    let (Some(save), Some(record_dir)) = (replays.save_settings(), replays.record_dir()) else {
        return Err(MakeError::lasting("Save is off"));
    };
    let recording =
        recording_in(record_dir, match_id).ok_or_else(|| MakeError::lasting("no kept recording"))?;
    let max_bytes = replays
        .watches()
        .settings()
        .map_or(DEFAULT_MAX_RECORDING_MB << 20, |settings| {
            settings.max_recording_bytes
        });
    let size = fs::metadata(&recording).map_err(MakeError::read)?.len();
    if size > max_bytes {
        return Err(MakeError::lasting(format!(
            "the recording has {size} bytes, more than SRP_REPLAY_MAX_RECORDING_MB allows"
        )));
    }
    let notes = record::read_notes(&recording).map_err(MakeError::read)?;
    let sur = sur_of(&recording, &notes).map_err(MakeError::lasting)?;
    // What a replay session would refuse is not saved either.
    Recording::from_sur(&sur, match_id).map_err(MakeError::lasting)?;
    let replay_id = recording
        .file_stem()
        .and_then(|stem| stem.to_str())
        .and_then(ReplayId::parse)
        .ok_or_else(|| MakeError::lasting("the recording's name is no replay id"))?;
    let events = match accounts.and_then(GameAccounts::history) {
        Some(history) => match history.events_of_replay(&replay_id) {
            Ok(kept) => kept.map(|kept| kept.events).unwrap_or_default(),
            Err(error) => {
                return Err(MakeError::disk(format!("the history's events: {error}")));
            }
        },
        None => Vec::new(),
    };
    let (accounts_by_place, revisions) = by_place(&notes);
    let header = header_of(&sur, &replay_id, revisions.clone(), &events)?;
    let file = ReplayFile {
        header,
        events: timeline_of(&events),
        sur,
    }
    .write()
    .map_err(MakeError::lasting)?;
    let sidecar = Sidecar {
        match_id,
        size: file.len() as u64,
        sha256: openssl::sha::sha256(&file)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
        revisions,
        account_ids: accounts_by_place.into_iter().flatten().collect(),
        made_at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since| since.as_secs()),
    };
    let stem = file_stem(match_id);
    write_whole(&save.replay_dir, &format!("{stem}.{FILE_EXTENSION}"), &file)?;
    write_whole(
        &save.replay_dir,
        &format!("{stem}.json"),
        sidecar.to_json().as_bytes(),
    )?;
    Ok(sidecar)
}

/// The file's header: the map, the mode and each place's name and team of
/// the `.sur`'s 0xE4 options, its length, when its recording was made,
/// and the score, kills and deaths of the history's events.
fn header_of(
    file: &[u8],
    replay_id: &ReplayId,
    revisions: Vec<Option<u8>>,
    events: &[MatchEvent],
) -> Result<Header, MakeError> {
    let contents = sur::read(file).map_err(MakeError::lasting)?;
    let mut payload = vec![NO_LOCAL_PLACE];
    payload.extend(&contents.options_lzma2);
    let (_, options) = static_info::parse_payload(&payload, Dictionary::shared())
        .map_err(|error| MakeError::lasting(format!("the options: {error}")))?;
    let mut places: Vec<Place> = options
        .players
        .iter()
        .map(|player| Place {
            team: player.team,
            kills: 0,
            deaths: 0,
            name: if player.clan_tag.is_empty() {
                player.name.clone()
            } else {
                format!("[{}] {}", player.clan_tag, player.name)
            },
        })
        .collect();
    let mut score = None;
    let mut count = |place: u8, kill: bool| {
        if let Some(place) = places.get_mut(usize::from(place)) {
            let field = if kill { &mut place.kills } else { &mut place.deaths };
            *field = field.saturating_add(1);
        }
    };
    for event in events {
        match event.kind {
            EventKind::Kill { killer, victim } => {
                count(killer.place, true);
                if let Some(victim) = victim {
                    count(victim.place, false);
                }
            }
            EventKind::Death { victim } => count(victim.place, false),
            EventKind::End { points } => score = Some(points),
            _ => {}
        }
    }
    let map = options.map.map();
    let end = contents.records.last().map_or(0, |record| record.event.time);
    let made = replay_id
        .made()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    Ok(Header {
        epoch: session::v026g::replays::simulation_epoch(&revisions),
        match_id: replay_id.match_id(),
        made,
        length: end / 1000,
        map: map.id,
        map_slug: localization::text(Language::English, map.map_name_key)
            .unwrap_or("")
            .to_owned(),
        mode: options.map.mode().lobby_number(),
        score,
        revisions,
        places,
    })
}

/// `dir/name` written under `.part` first.
fn write_whole(dir: &Path, name: &str, bytes: &[u8]) -> Result<(), MakeError> {
    let part = dir.join(format!("{name}{PART}"));
    let path = dir.join(name);
    fs::write(&part, bytes)
        .and_then(|()| fs::rename(&part, &path))
        .map_err(|error| {
            let _ = fs::remove_file(&part);
            MakeError::disk(format!("{name}: {error}"))
        })
}

/// Deletes the oldest files (by their time; the sidecar first, so a
/// half-deleted one is not "made") while the files, `<8 hex>.srpreplay`
/// and `.json`, are over `SRP_REPLAY_MAX_MB` together. `keep`'s, just made,
/// is never deleted, even alone over the cap. Nothing of another name is
/// touched.
pub fn cap(save: &SaveSettings, keep: Option<u32>) {
    let Ok(entries) = fs::read_dir(&save.replay_dir) else {
        return;
    };
    let extension = format!(".{FILE_EXTENSION}");
    // By match: the bytes of its two files, and its file's time.
    let mut found: std::collections::HashMap<u32, (u64, Option<SystemTime>)> =
        std::collections::HashMap::new();
    for entry in entries.flatten() {
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        if !meta.is_file() {
            continue;
        }
        if let Some(match_id) = replays::match_of_name(&name, &extension) {
            let file = found.entry(match_id).or_default();
            file.0 += meta.len();
            file.1 = meta.modified().ok();
        } else if let Some(match_id) = replays::match_of_name(&name, ".json") {
            found.entry(match_id).or_default().0 += meta.len();
        }
    }
    let mut total: u64 = found.values().map(|(bytes, _)| bytes).sum();
    if total <= save.max_replay_bytes {
        return;
    }
    let mut oldest: Vec<(Option<SystemTime>, u32, u64)> = found
        .into_iter()
        .filter(|&(match_id, _)| Some(match_id) != keep)
        .map(|(match_id, (bytes, time))| (time, match_id, bytes))
        .collect();
    // A sidecar with no file (no time) first, then the oldest.
    oldest.sort_unstable();
    for (_, match_id, bytes) in oldest {
        if total <= save.max_replay_bytes {
            break;
        }
        let stem = file_stem(match_id);
        let mut gone = true;
        for name in [format!("{stem}.json"), format!("{stem}{extension}")] {
            let path = save.replay_dir.join(&name);
            match fs::remove_file(&path) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => {
                    gone = false;
                    log::warn!("Save: can't delete {}: {error}", path.display());
                }
            }
        }
        if gone {
            total -= bytes;
            log::info!(
                "Save: the file of match {match_id:#x} deleted ({bytes} bytes; the files are \
                 over SRP_REPLAY_MAX_MB, {} MB)",
                save.max_replay_bytes >> 20
            );
        }
    }
}

/// Deletes what a stopped server left under `.part`, and every file
/// (`<8 hex>.srpreplay` / `.json`) whose recording in `record_dir` is gone.
/// Nothing of another name is touched.
pub fn prune(save: &SaveSettings, record_dir: &Path) {
    let Ok(entries) = fs::read_dir(&save.replay_dir) else {
        return;
    };
    let recorded: std::collections::HashSet<u32> = fs::read_dir(record_dir)
        .map(|entries| {
            entries
                .flatten()
                .filter_map(|entry| record::match_of_file(&entry.path()))
                .collect()
        })
        .unwrap_or_default();
    let extension = format!(".{FILE_EXTENSION}");
    for entry in entries.flatten() {
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let left = [format!("{extension}{PART}"), format!(".json{PART}")]
            .iter()
            .any(|suffix| replays::match_of_name(&name, suffix).is_some());
        let orphan = [extension.as_str(), ".json"].iter().any(|suffix| {
            replays::match_of_name(&name, suffix).is_some_and(|id| !recorded.contains(&id))
        });
        if left || orphan {
            let path = entry.path();
            match fs::remove_file(&path) {
                Ok(()) => log::info!(
                    "Save: {} deleted ({})",
                    path.display(),
                    if left {
                        "left by a stopped server"
                    } else {
                        "its recording is gone"
                    }
                ),
                Err(error) => log::warn!("Save: can't delete {}: {error}", path.display()),
            }
        }
    }
}

/// The calling thread at the lowest priority, nice 19 (Linux; nothing
/// elsewhere): reading a recording takes only the CPU the matches leave. Said in
/// the log if it cannot be set.
pub(crate) fn lowest_priority() {
    #[cfg(target_os = "linux")]
    {
        // SAFETY: the `gettid` system call and `setpriority` take and return
        // plain integers; on Linux `PRIO_PROCESS` with a thread id sets that
        // thread's nice alone (glibc and musl alike).
        let set = unsafe {
            let thread = libc::syscall(libc::SYS_gettid);
            libc::setpriority(libc::PRIO_PROCESS, thread as libc::id_t, 19)
        };
        if set != 0 {
            log::warn!(
                "Replays: a reader's priority is not lowered: {}",
                std::io::Error::last_os_error()
            );
        }
    }
}

/// The newest kept recording of a match in `record_dir`: a regular file
/// named `rec026g_<UTC start>_<id:08x>.srprec`.
pub(crate) fn recording_in(record_dir: &Path, match_id: u32) -> Option<PathBuf> {
    fs::read_dir(record_dir)
        .ok()?
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .map(|entry| entry.path())
        .filter(|path| record::match_of_file(path) == Some(match_id))
        // The newest: an id comes again after a restart, and the name
        // starts with when it was made.
        .max()
}

/// Every place's account, and the patch revision each place's game
/// connected with (the highest), from a recording's notes.
pub fn places(notes: &[record::Record]) -> (Vec<u32>, Vec<Option<u8>>) {
    let (accounts, revisions) = by_place(notes);
    (accounts.into_iter().flatten().collect(), revisions)
}

/// [`places`] by place: `None` for a place no note names.
pub(crate) fn by_place(notes: &[record::Record]) -> (Vec<Option<u32>>, Vec<Option<u8>>) {
    let mut accounts: Vec<Option<u32>> = Vec::new();
    let mut revisions: Vec<Option<u8>> = Vec::new();
    for note in notes.iter().filter(|record| record.kind == NOTE) {
        let Ok(note) = serde_json::from_slice::<serde_json::Value>(&note.bytes) else {
            continue;
        };
        let listed = match note["note"].as_str() {
            Some("header" | "places") => note["places"].as_array(),
            Some("revision") => {
                let place = note["place"].as_u64().map(|place| place as usize);
                let revision = note["revision"]
                    .as_u64()
                    .and_then(|revision| u8::try_from(revision).ok())
                    // 0: a session the server did not know.
                    .filter(|&revision| revision != 0);
                if let (Some(place), Some(revision)) = (place, revision)
                    && place < 32
                {
                    if revisions.len() <= place {
                        revisions.resize(place + 1, None);
                    }
                    let known = &mut revisions[place];
                    *known = Some(known.map_or(revision, |known| known.max(revision)));
                }
                None
            }
            _ => None,
        };
        for place in listed.into_iter().flatten() {
            let (Some(index), Some(account)) =
                (place["place"].as_u64(), place["account_id"].as_u64())
            else {
                continue;
            };
            let index = index as usize;
            if index >= 32 {
                continue;
            }
            if accounts.len() <= index {
                accounts.resize(index + 1, None);
            }
            accounts[index] = u32::try_from(account).ok();
        }
    }
    let places = accounts.len().max(revisions.len());
    revisions.resize(places, None);
    (accounts, revisions)
}

#[cfg(test)]
mod tests;
