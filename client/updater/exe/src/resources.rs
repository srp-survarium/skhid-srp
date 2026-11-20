//! The one-time restore of the game's `resources.db`
//! (`client/updater/README.md`, "Restoring resources.db";
//! `updater_logic::resources`): the regions an earlier updater rewrote in
//! place, as its record beside the updater names them, get their stock
//! bytes back, after the overlay and before the patch step. Once the record
//! is empty this step does nothing but look for its file.
//!
//! For one region:
//!
//! 1. Its bytes are hashed (the file may be open in the game meanwhile).
//!    The stock bytes, a db of another size (another build) or none: the
//!    record of it goes. Else:
//! 2. The db is opened by this program alone (share mode 0) and closed
//!    again: a game that runs holds it, and nothing is changed this run.
//! 3. The stock bytes come from the copy the earlier updater kept
//!    (`updater_skhid.backup\<original sha256>`) when it checks, else from
//!    `<site>/resources/<original sha256>` into `updater_skhid.cache\`
//!    (streamed to the disk, its size and SHA-256 checked).
//! 4. The region is journalled (`updater_skhid.resources.journal`, flushed),
//!    the db opened by this program alone, the bytes written at the
//!    region's offset, flushed, read back and checked; then the journal goes
//!    and the region leaves the record.
//!
//! Best effort: whatever fails is logged and tried again at the next start
//! (the record stays); it never fails the update. The patches mount over
//! the db, so a region not restored yet is hidden wherever a patch has
//! that file. Once the record is empty, the record, the copies kept and the
//! cache are deleted.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use updater_logic::pipe::{ToLauncher, state, step};
use updater_logic::resources::{Look, Region, Undo, parse_record, undo, write_record};
use updater_logic::sha256::{Sha256, to_hex};
use updater_logic::url::Url;
use windows_sys::Win32::Foundation::{ERROR_HANDLE_EOF, ERROR_SHARING_VIOLATION};

use crate::Session;
use crate::http::{self, HttpError};
use crate::install::Layout;
use crate::log::log;
use crate::win::{self, Code, File};

/// Beside the updater: the record and the journal of the earlier updaters,
/// the copies of the stock bytes they kept, and blobs fetched.
const RECORD_FILE: &str = "updater_skhid.resources.state";
const JOURNAL_FILE: &str = "updater_skhid.resources.journal";
const BACKUP_DIR: &str = "updater_skhid.backup";
const CACHE_DIR: &str = "updater_skhid.cache";
/// The earlier updaters' plan (from their ZIP) and the status of a plan
/// that grew the db: no longer read, deleted when found.
const OLD_FILES: [&str; 2] = ["updater_skhid.resources", "updater_skhid.resources.status"];
/// A blob while it is written, before it is checked.
const PART: &str = ".part";
/// How often a blob is asked for in one run.
const FETCH_TRIES: u32 = 2;
const RETRY_PAUSE_MS: u32 = 3000;
/// Bytes read or written at a time.
const PIECE: usize = 64 * 1024;

fn hex(sha256: &[u8; 32]) -> String {
    String::from_utf8_lossy(&to_hex(sha256)).into()
}

/// The SHA-256 of `size` bytes of `file` from `offset`.
fn hash_range(file: &File, offset: u64, size: u64) -> Result<[u8; 32], Code> {
    file.seek(offset)?;
    let mut hash = Sha256::new();
    let mut buffer = alloc::vec![0_u8; PIECE];
    let mut left = size;
    while left > 0 {
        let want = (left.min(PIECE as u64)) as usize;
        let read = file.read(&mut buffer[..want])?;
        if read == 0 {
            return Err(ERROR_HANDLE_EOF);
        }
        hash.update(&buffer[..read]);
        left -= read as u64;
    }
    Ok(hash.finish())
}

/// Copies `size` bytes of `from` (from 0) to `to` (at `to_at`), and returns
/// the SHA-256 of what was read. `progress(done)` after each piece.
fn copy(
    from: &File,
    to: &File,
    to_at: u64,
    size: u64,
    progress: &mut dyn FnMut(u64),
) -> Result<[u8; 32], Code> {
    from.seek(0)?;
    to.seek(to_at)?;
    let mut hash = Sha256::new();
    let mut buffer = alloc::vec![0_u8; PIECE];
    let mut done = 0_u64;
    while done < size {
        let want = ((size - done).min(PIECE as u64)) as usize;
        let read = from.read(&mut buffer[..want])?;
        if read == 0 {
            return Err(ERROR_HANDLE_EOF);
        }
        hash.update(&buffer[..read]);
        to.write(&buffer[..read])?;
        done += read as u64;
        progress(done);
    }
    Ok(hash.finish())
}

/// A file of `size` bytes with this SHA-256, held so that no one changes it
/// until it is dropped.
fn checked(path: &str, size: u64, sha256: &[u8; 32]) -> Option<File> {
    let file = File::locked(path).ok()?;
    (file.size().ok()? == size && hash_range(&file, 0, size).ok()? == *sha256).then_some(file)
}

/// What the bytes of `region` are now. The file may be open in the game.
fn look(place: &str, region: &Region) -> Look {
    let file = match File::shared(place) {
        Ok(file) => file,
        Err(code) if win::is_missing(code) => return Look::Missing,
        Err(code) => {
            log!(
                "resources: {place} cannot be read ({})",
                win::error_text(code)
            );
            return Look::Unreadable;
        }
    };
    match file.size() {
        Ok(size) if size != region.file_size => return Look::OtherSize(size),
        Ok(_) => {}
        Err(_) => return Look::Unreadable,
    }
    match hash_range(&file, region.offset, region.size) {
        Ok(hash) => Look::Bytes(hash),
        Err(code) => {
            log!(
                "resources: {place} cannot be read ({})",
                win::error_text(code)
            );
            Look::Unreadable
        }
    }
}

/// How the step ended.
pub enum Outcome {
    /// Done, or left for the next start: the game may be played.
    Done,
    /// The launcher asked to stop.
    Stopped,
}

struct Restore<'a> {
    session: &'a mut Session,
    layout: &'a Layout,
    /// Where the stock bytes are fetched when no copy is kept; `None` when
    /// the server could not be asked this run.
    server: Option<&'a Url>,
    record: Vec<Region>,
    /// The record as it is on the disk.
    saved: Vec<Region>,
    /// The launcher's line was set to "installing" by this step.
    installing: bool,
    /// The game holds the db: nothing more is tried this run.
    game_running: bool,
    stopped: bool,
}

impl Restore<'_> {
    fn beside(&self, name: &str) -> String {
        format!("{}\\{name}", self.layout.exe_dir)
    }

    fn backup(&self, region: &Region) -> String {
        format!(
            "{}\\{BACKUP_DIR}\\{}",
            self.layout.exe_dir,
            hex(&region.original)
        )
    }

    /// Writes the record beside, then renames it over; deletes it when it
    /// is empty.
    fn save(&mut self) {
        if self.record == self.saved {
            return;
        }
        let path = self.beside(RECORD_FILE);
        let result = if self.record.is_empty() {
            win::delete(&path)
        } else {
            let staged = format!("{path}.skhid-new");
            let result = win::write_file(&staged, &write_record(&self.record))
                .and_then(|()| win::rename(&staged, &path, true));
            if result.is_err() {
                let _ = win::delete(&staged);
            }
            result
        };
        match result {
            Ok(()) => self.saved = self.record.clone(),
            Err(code) => log!(
                "resources: {RECORD_FILE} cannot be written ({})",
                win::error_text(code)
            ),
        }
    }

    fn forget(&mut self, region: &Region) {
        self.record.retain(|other| !other.same_place(region));
        self.save();
    }

    /// The db opened by this program alone. A game that runs holds it.
    fn open(&mut self, place: &str) -> Option<File> {
        match File::exclusive(place) {
            Ok(file) => Some(file),
            Err(ERROR_SHARING_VIOLATION) => {
                log!(
                    "resources: the game is running ({place} is open in another program): \
                     its stock bytes are put back next time"
                );
                self.game_running = true;
                None
            }
            Err(code) => {
                log!(
                    "resources: {place} cannot be opened to write ({}): next time",
                    win::error_text(code)
                );
                None
            }
        }
    }

    /// The region's stock bytes from the server, into the cache: the path.
    fn fetch(&mut self, region: &Region) -> Option<String> {
        let Some(server) = self.server else {
            log!(
                "resources: {}: no copy of its stock bytes is kept here and the server could not be asked: next time",
                region.name
            );
            return None;
        };
        let folder = self.beside(CACHE_DIR);
        if let Err(code) = win::create_folders(&folder) {
            log!(
                "resources: {folder} cannot be made ({})",
                win::error_text(code)
            );
            return None;
        }
        let name = hex(&region.original);
        let path = format!("{folder}\\{name}");
        if checked(&path, region.size, &region.original).is_some() {
            return Some(path);
        }
        let address = format!("/resources/{name}");
        let part = format!("{path}{PART}");
        let size = region.size;
        for attempt in 1..=FETCH_TRIES {
            self.session.state(state::DOWNLOADING);
            self.installing = false;
            self.session.begin_progress();
            let output = match File::create(&part) {
                Ok(output) => output,
                Err(code) => {
                    log!(
                        "resources: {part} cannot be written ({})",
                        win::error_text(code)
                    );
                    return None;
                }
            };
            let session = &mut *self.session;
            let result = http::get_stream(
                server,
                &address,
                size,
                &mut |chunk| {
                    output
                        .write(chunk)
                        .map_err(|code| HttpError::Network("write the blob", code))
                },
                &mut |done, _| session.progress(done, size),
            )
            .map_err(|error| crate::text::http(server, &address, &error))
            .and_then(|()| {
                output.flush().map_err(|code| {
                    format!("the blob cannot be written ({})", win::error_text(code))
                })
            });
            drop(output);
            let result = result.and_then(|()| {
                if checked(&part, size, &region.original).is_some() {
                    win::rename(&part, &path, true).map_err(|code| {
                        format!("the blob cannot be renamed ({})", win::error_text(code))
                    })
                } else {
                    Err(format!(
                        "the server's blob is not {size} bytes with that SHA-256"
                    ))
                }
            });
            match result {
                Ok(()) => {
                    log!(
                        "resources: {}: fetched {address} ({size} bytes, checksum right)",
                        region.name
                    );
                    return Some(path);
                }
                Err(message) => {
                    let _ = win::delete(&part);
                    if self.session.stopped() {
                        self.stopped = true;
                        return None;
                    }
                    log!(
                        "resources: {}: fetch {attempt} of {FETCH_TRIES} failed: {message}",
                        region.name
                    );
                    if attempt < FETCH_TRIES && !self.session.pause(RETRY_PAUSE_MS) {
                        self.stopped = true;
                        return None;
                    }
                }
            }
        }
        None
    }

    /// The journal: the region being written, on the disk before its first
    /// byte is.
    fn journal(&self, region: &Region) -> bool {
        match win::write_file(
            &self.beside(JOURNAL_FILE),
            &write_record(core::slice::from_ref(region)),
        ) {
            Ok(()) => true,
            Err(code) => {
                log!(
                    "resources: {JOURNAL_FILE} cannot be written ({}): {} is not written",
                    win::error_text(code),
                    region.name
                );
                false
            }
        }
    }

    /// Writes the stock bytes at `source` into the region, flushes, and
    /// reads it back.
    fn write(&mut self, file: &File, region: &Region, source: &str) -> Result<(), String> {
        let Some(blob) = checked(source, region.size, &region.original) else {
            return Err(format!("{source} is not the copy it was"));
        };
        if !self.installing {
            self.installing = true;
            self.session.state(state::INSTALLING);
        }
        let short = region.name.as_str();
        self.session.send(ToLauncher::InstallInfo {
            step: step::EXTRACT,
            text: short,
        });
        self.session.named_file = true;
        self.session.begin_progress();
        let size = region.size;
        let session = &mut *self.session;
        let copied = copy(&blob, file, region.offset, size, &mut |done| {
            // The launcher may say stop: a region is finished all the same.
            let _ = session.progress(done, size);
        })
        .map_err(|code| format!("write: {}", win::error_text(code)))?;
        if copied != region.original {
            return Err("the copy changed while it was written".into());
        }
        file.flush()
            .map_err(|code| format!("flush: {}", win::error_text(code)))?;
        match hash_range(file, region.offset, size) {
            Ok(hash) if hash == region.original => Ok(()),
            Ok(_) => Err("the region read back with other bytes".into()),
            Err(code) => Err(format!("read back: {}", win::error_text(code))),
        }
    }

    /// One recorded region: its stock bytes back.
    fn restore(&mut self, region: &Region) {
        let place = self.layout.place(&region.path);
        match undo(region, look(&place, region)) {
            Undo::Forget => {
                log!(
                    "resources: {} holds its stock bytes (or the db is gone or of another build): forgotten",
                    region.name
                );
                self.forget(region);
            }
            Undo::Wait => {}
            Undo::Restore => {
                // Nothing is fetched for a db the game holds, and the db is
                // not held through a download.
                match self.open(&place) {
                    Some(probe) => drop(probe),
                    None => return,
                }
                let kept = self.backup(region);
                let (source, fetched) = if checked(&kept, region.size, &region.original).is_some() {
                    (kept, false)
                } else {
                    match self.fetch(region) {
                        Some(path) => (path, true),
                        None => return,
                    }
                };
                let Some(file) = self.open(&place) else {
                    return;
                };
                if !self.journal(region) {
                    return;
                }
                match self.write(&file, region, &source) {
                    Ok(()) => {
                        log!(
                            "resources: {}: its original is back ({})",
                            region.name,
                            if fetched {
                                "from the server"
                            } else {
                                "from the copy kept"
                            }
                        );
                        let _ = win::delete(&self.beside(JOURNAL_FILE));
                        self.forget(region);
                    }
                    Err(message) => log!(
                        "resources: {}: its original could not be put back ({message}): next time",
                        region.name
                    ),
                }
                if fetched {
                    let _ = win::delete(&source);
                }
            }
        }
    }

    /// Once nothing is recorded: the copies kept and the cache go, and
    /// whatever a run cut short left in them.
    fn sweep(&self) {
        for folder in [CACHE_DIR, BACKUP_DIR] {
            let folder = self.beside(folder);
            if !win::is_folder(&folder) {
                continue;
            }
            let keep: Vec<String> = if folder.ends_with(BACKUP_DIR) {
                self.record
                    .iter()
                    .map(|region| hex(&region.original))
                    .collect()
            } else {
                Vec::new()
            };
            for name in win::list(&folder, "*") {
                if keep.iter().any(|kept| kept.eq_ignore_ascii_case(&name)) {
                    continue;
                }
                match win::delete(&format!("{folder}\\{name}")) {
                    Ok(()) => log!("resources: removed {name} from {folder}"),
                    Err(code) => log!(
                        "resources: {folder}\\{name} cannot be removed ({})",
                        win::error_text(code)
                    ),
                }
            }
            // Only when it is empty.
            win::remove_dir(&folder);
        }
    }
}

/// The restore. `server`: `None` when the server could not be asked this
/// run (only the copies kept are used then). Tells the launcher something
/// only when it writes or fetches.
pub fn run(session: &mut Session, layout: &Layout, server: Option<&Url>) -> Outcome {
    let beside = |name: &str| format!("{}\\{name}", layout.exe_dir);
    for name in OLD_FILES {
        let path = beside(name);
        if win::exists(&path) {
            match win::delete(&path) {
                Ok(()) => log!("resources: removed {name} (no longer used)"),
                Err(code) => log!(
                    "resources: {name} cannot be removed ({})",
                    win::error_text(code)
                ),
            }
        }
    }
    let record = match win::read_file(&beside(RECORD_FILE), 1 << 20) {
        Ok(bytes) => parse_record(&bytes).unwrap_or_else(|| {
            log!(
                "resources: {RECORD_FILE} cannot be read as a record: nothing is restored from it"
            );
            Vec::new()
        }),
        Err(_) => Vec::new(),
    };
    let journal = win::read_file(&beside(JOURNAL_FILE), 1 << 20).ok();
    let nothing_kept = !win::is_folder(&beside(CACHE_DIR)) && !win::is_folder(&beside(BACKUP_DIR));
    if record.is_empty() && journal.is_none() && nothing_kept {
        if win::exists(&beside(RECORD_FILE)) {
            let _ = win::delete(&beside(RECORD_FILE));
        }
        return Outcome::Done;
    }
    let mut step = Restore {
        session,
        layout,
        server,
        saved: record.clone(),
        record,
        installing: false,
        game_running: false,
        stopped: false,
    };
    // A write that was cut short: its region is recorded, and what it holds
    // is told by its checksum.
    if let Some(bytes) = journal {
        for region in parse_record(&bytes).unwrap_or_default() {
            log!("resources: a write of {} was cut short", region.name);
            if !step.record.iter().any(|other| other.same_place(&region)) {
                step.record.push(region);
            }
        }
        step.save();
        if step.record == step.saved {
            let _ = win::delete(&beside(JOURNAL_FILE));
        }
    }
    if !step.record.is_empty() {
        log!(
            "resources: {} region(s) of resources.db changed by an earlier updater: their stock bytes go back",
            step.record.len()
        );
    }
    for region in step.record.clone() {
        if step.game_running || step.stopped || !step.session.pump() {
            break;
        }
        step.restore(&region);
    }
    step.save();
    if step.record.is_empty() {
        let _ = win::delete(&beside(RECORD_FILE));
    } else if !step.stopped && !step.session.stopped() {
        log!(
            "resources: {} region(s) not restored yet ({}): next time",
            step.record.len(),
            step.record
                .iter()
                .map(|region| region.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    step.sweep();
    if step.stopped || step.session.stopped() {
        log!("resources: stopped; the rest next time");
        return Outcome::Stopped;
    }
    Outcome::Done
}
