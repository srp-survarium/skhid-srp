//! "Watch replay" (`docs/notes/replay_watch.md`): the match recordings
//! ([`super::record`]) made into the game's own replay files
//! ([`super::sur`]) by the server, for the lobby's History.
//!
//! One thread ([`start`]) takes the matches the replays' desk
//! (`session::v026g::replays::Replays`) queues, one at a time: a kept
//! recording a player has asked for the replay of, the first time anyone
//! does (most are never watched; a match's end converts nothing). For each
//! it writes, in `SRP_REPLAY_DIR`:
//!
//! - `<id:08x>.sur`: [`sur_convert::convert_file`] of the recording with the
//!   mode's `.options` file of the server's game data (the file the map's
//!   `maps_dict` row names, [`sur_convert::config`]: the one the map was
//!   played with, so every map and mode srp plays has it);
//! - `<id:08x>.json`, the sidecar ([`Sidecar`]): the size and SHA-256 of
//!   the `.sur`, each place's patch revision (the recording's `revision`
//!   notes, from 2026-10-15), every place's account (the header and the
//!   `places` notes: late joiners too), and when it was made. Never served.
//!
//! Each goes under `.part` first and gets its name when whole, the sidecar
//! last: a sidecar means its `.sur` is complete. A replay lives as long as
//! its recording: the writer's storage cap deletes both
//! (`Replays::recording_deleted`), and at start every replay whose recording
//! is gone is deleted ([`prune`]). The replays over `SRP_REPLAY_MAX_MB`
//! together go too, the oldest first ([`cap`]); one that went is made again
//! when asked for.
//!
//! What can be checked before the recording is unpacked whole is checked
//! first (its size, its header's map), and a failure is tried again only
//! when its cause can change (`replays::Failure`).
//!
//! What a `.sur` holds is what every player of the match was sent in it
//! (`sur_convert`'s chunks: the 0xE4 options of the largest roster, the
//! relayed inputs, roster changes and kit choices, the worlds the clients
//! uploaded) and the mode's config, which every game has: no address, no
//! session, no token, no server path ([`tests`]).
//!
//! The thread runs at the lowest priority on Linux (nice 19,
//! [`lowest_priority`]): the container's one CPU is shared by the lobby,
//! the web page and every match's ticks, which must never wait for a
//! conversion. One at a time, about two seconds of CPU for a match of 20
//! places.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Instant, SystemTime};

use session::v026g::replays::{self, Failure, ReplaySettings, Replays, Sidecar, file_stem};

use super::record::{self, NOTE};
use super::sur_convert::{self, Settings};

const PART: &str = ".part";

/// `SRP_REPLAY_DIR`, `SRP_REPLAY_MAX_RECORDING_MB`,
/// `SRP_REPLAY_MAX_MB` with `SRP_RECORD_DIR` and `SRP_RECORD_MIN_PLAYERS`:
/// the replays of the srp process (off, and said in the log, when a setting
/// is missing or the replay folder overlaps the recordings').
pub fn replays_from_env() -> Replays {
    replays_of(ReplaySettings::from_env())
}

/// The replays of these settings (`None`: off), with the recordings'
/// minimum of players of the environment (`SRP_RECORD_MIN_PLAYERS`).
pub fn replays_of(settings: Option<ReplaySettings>) -> Replays {
    let min_players = record::RecordSettings::from_env().map_or(6, |record| record.min_players);
    if let Some(settings) = &settings {
        log::info!(
            "Replays: made in {} from the recordings of {}, with the modes of the game data; \
             recordings over {} MB are not converted; replays over {} MB together go, the \
             oldest first",
            settings.replay_dir.display(),
            settings.record_dir.display(),
            settings.max_recording_bytes >> 20,
            settings.max_replay_bytes >> 20
        );
    }
    // A simulation change not recorded yet (`next`) is this release's own
    // revision: the one the release set as the server's minimum.
    let min_patch_revision = std::env::var("SRP_MIN_PATCH_REVISION")
        .ok()
        .and_then(|value| value.trim().parse::<u8>().ok())
        .unwrap_or(0);
    let oldest = replays::oldest_replayable_here(min_patch_revision);
    if settings.is_some() {
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
    }
    Replays::new(settings, min_players).with_oldest_revision(oldest)
}

/// Starts the conversion thread, after deleting what is left of a stopped
/// server, the replays of recordings that are gone and the oldest over the
/// cap. `None` when replays are off.
pub fn start(replays: Arc<Replays>) -> Option<io::Result<JoinHandle<()>>> {
    let settings = replays.settings()?.clone();
    prune(&settings);
    cap(&settings, None);
    Some(
        std::thread::Builder::new()
            .name("srp-replay".into())
            .spawn(move || {
                lowest_priority();
                while let Some(match_id) = replays.next_job() {
                    let started = Instant::now();
                    let made = make(&settings, match_id);
                    match &made {
                        Ok(sidecar) => {
                            log::info!(
                                "Replays: match {match_id:#x} converted in {} ms: {} bytes, \
                                 SHA-256 {}…",
                                started.elapsed().as_millis(),
                                sidecar.size,
                                &sidecar.sha256[..12]
                            );
                            cap(&settings, Some(match_id));
                        }
                        Err(error) => log::warn!(
                            "Replays: match {match_id:#x} not converted: {} ({})",
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

/// The calling thread at the lowest priority, nice 19 (Linux; nothing
/// elsewhere): a conversion takes only the CPU the matches leave. Said in
/// the log if it cannot be set.
fn lowest_priority() {
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
                "Replays: the conversion thread's priority is not lowered: {}",
                io::Error::last_os_error()
            );
        }
    }
}

/// Why a conversion failed: the log's words, and when it is tried again.
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

/// The recording of `match_id` made into its replay and sidecar. Before
/// the recording is unpacked whole: its size against
/// `SRP_REPLAY_MAX_RECORDING_MB`, and the map its header names (the header
/// is in the first block). The mode's config is the game data's own.
pub fn make(settings: &ReplaySettings, match_id: u32) -> Result<Sidecar, MakeError> {
    let recording =
        recording(settings, match_id).ok_or_else(|| MakeError::lasting("no kept recording"))?;
    let size = fs::metadata(&recording).map_err(MakeError::read)?.len();
    if size > settings.max_recording_bytes {
        return Err(MakeError::lasting(format!(
            "the recording has {size} bytes, more than SRP_REPLAY_MAX_RECORDING_MB allows"
        )));
    }
    let header = record::read_header(&recording)
        .map_err(MakeError::read)?
        .ok_or_else(|| MakeError::lasting("the recording has no header"))?;
    let name = sur_convert::config_name(std::slice::from_ref(&header))
        .ok_or_else(|| MakeError::lasting("the recording names no known map"))?;
    let config = sur_convert::config(&name)
        .ok_or_else(|| MakeError::lasting(format!("the game data has no mode config {name}")))?;
    let notes = record::read_notes(&recording).map_err(MakeError::read)?;
    let (file, _) = sur_convert::convert_file(&recording, &notes, config, Settings::default())
        .map_err(|error| MakeError::lasting(error.to_string()))?;
    let (account_ids, revisions) = places(&notes);
    let sidecar = Sidecar {
        match_id,
        size: file.len() as u64,
        sha256: openssl::sha::sha256(&file)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
        revisions,
        account_ids,
        made_at: Replays::now_unix(),
    };
    let stem = file_stem(match_id);
    write_whole(&settings.replay_dir, &format!("{stem}.sur"), &file)?;
    write_whole(
        &settings.replay_dir,
        &format!("{stem}.json"),
        sidecar.to_json().as_bytes(),
    )?;
    Ok(sidecar)
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

/// Deletes the oldest replays (by their `.sur`'s time; the sidecar first,
/// so a half-deleted one is not "made") while the replays, `<8 hex>.sur`
/// and `.json`, are over `SRP_REPLAY_MAX_MB` together. `keep`'s, just
/// made, is never deleted, even alone over the cap. Nothing of another
/// name is touched.
pub fn cap(settings: &ReplaySettings, keep: Option<u32>) {
    let Ok(entries) = fs::read_dir(&settings.replay_dir) else {
        return;
    };
    // By match: the bytes of its two files, and its `.sur`'s time.
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
        if let Some(match_id) = replays::match_of_name(&name, ".sur") {
            let replay = found.entry(match_id).or_default();
            replay.0 += meta.len();
            replay.1 = meta.modified().ok();
        } else if let Some(match_id) = replays::match_of_name(&name, ".json") {
            found.entry(match_id).or_default().0 += meta.len();
        }
    }
    let mut total: u64 = found.values().map(|(bytes, _)| bytes).sum();
    if total <= settings.max_replay_bytes {
        return;
    }
    let mut oldest: Vec<(Option<SystemTime>, u32, u64)> = found
        .into_iter()
        .filter(|&(match_id, _)| Some(match_id) != keep)
        .map(|(match_id, (bytes, time))| (time, match_id, bytes))
        .collect();
    // A sidecar with no `.sur` (no time) first, then the oldest.
    oldest.sort_unstable();
    for (_, match_id, bytes) in oldest {
        if total <= settings.max_replay_bytes {
            break;
        }
        let stem = file_stem(match_id);
        let mut gone = true;
        for name in [format!("{stem}.json"), format!("{stem}.sur")] {
            let path = settings.replay_dir.join(&name);
            match fs::remove_file(&path) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => {
                    gone = false;
                    log::warn!("Replays: can't delete {}: {error}", path.display());
                }
            }
        }
        if gone {
            total -= bytes;
            log::info!(
                "Replays: the replay of match {match_id:#x} deleted ({bytes} bytes; the \
                 replays are over SRP_REPLAY_MAX_MB, {} MB)",
                settings.max_replay_bytes >> 20
            );
        }
    }
}

/// The kept recording of a match in the recordings' folder.
fn recording(settings: &ReplaySettings, match_id: u32) -> Option<PathBuf> {
    fs::read_dir(&settings.record_dir)
        .ok()?
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .map(|entry| entry.path())
        .find(|path| record::match_of_file(path) == Some(match_id))
}

/// Every place's account, by place, and the patch revision each place's
/// game connected with (the highest), from a recording's notes.
pub fn places(notes: &[record::Record]) -> (Vec<u32>, Vec<Option<u8>>) {
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
                    .and_then(|revision| u8::try_from(revision).ok());
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
    (accounts.into_iter().flatten().collect(), revisions)
}

/// Deletes what a stopped server left under `.part`, and every replay
/// (`<8 hex>.sur` / `.json`) whose recording is gone. Nothing of another
/// name is touched.
pub fn prune(settings: &ReplaySettings) {
    let Ok(entries) = fs::read_dir(&settings.replay_dir) else {
        return;
    };
    let recorded: std::collections::HashSet<u32> = fs::read_dir(&settings.record_dir)
        .map(|entries| {
            entries
                .flatten()
                .filter_map(|entry| record::match_of_file(&entry.path()))
                .collect()
        })
        .unwrap_or_default();
    for entry in entries.flatten() {
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let left = [".sur.part", ".json.part"]
            .iter()
            .any(|suffix| replays::match_of_name(&name, suffix).is_some());
        let orphan = [".sur", ".json"].iter().any(|suffix| {
            replays::match_of_name(&name, suffix).is_some_and(|id| !recorded.contains(&id))
        });
        if left || orphan {
            let path = entry.path();
            match fs::remove_file(&path) {
                Ok(()) => log::info!(
                    "Replays: {} deleted ({})",
                    path.display(),
                    if left {
                        "left by a stopped server"
                    } else {
                        "its recording is gone"
                    }
                ),
                Err(error) => log::warn!("Replays: can't delete {}: {error}", path.display()),
            }
        }
    }
}

#[cfg(test)]
mod tests;
