//! The replays' desk of the srp process ([`replays_from_env`]) and what the
//! replay sessions (`game::playback`, `docs/notes/replay-stream.md`) read
//! of the recordings ([`super::record`]): a match's kept recording, and its
//! places' accounts and game revisions.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use session::v026g::replay_watch::{Access, WatchSettings};
use session::v026g::replays::{self, Replays};

use super::record::{self, NOTE};

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
    Replays::new(Some(record.dir))
        .with_oldest_revision(oldest)
        .with_watch(watch)
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
                io::Error::last_os_error()
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
    (accounts, revisions)
}

#[cfg(test)]
mod tests;
