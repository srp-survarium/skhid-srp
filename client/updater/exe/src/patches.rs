//! The patch step: after the overlay, at every run, the game folder's
//! `resources.db.patch_<n>` files are made the server's list (`GET
//! <site>/patches`, `updater_logic::patches`): every listed patch's file is
//! checked (the launcher shows "checking" with its bar), a patch file the
//! list has not is deleted, and a listed patch that is missing or damaged
//! is downloaded from the first of its addresses that gives it (plain HTTP or HTTPS, any host, redirects followed: the list's
//! SHA-256 decides). `client/updater/README.md`, "Patches".
//!
//! A file is checked by its SHA-256, or, when it is still the file hashed
//! last time (its size, last-write time and file ID) and that was less than
//! a week ago, by the record of that hash (`updater_logic::checked`,
//! `updater_skhid.patches.checked`): a start then reads no patch file.
//! `updater_skhid.exe check` hashes them all ([`Look::Full`]). Each file is
//! hashed once a run at most: a download is hashed once, read back, and is
//! known in its place by its file ID.
//!
//! Required: PLAY stays off ([`Outcome::Blocked`]) until every listed patch
//! is there with its checksum and no other patch file is; the next start
//! tries again. A download goes to its part, `.skhid-patch_<n>.<sha256>.part`
//! (`Patch::part_name`), is flushed, read back and checked, and only then
//! renamed to its name: a patch file is never half written in place. A part
//! that broke off stays: the next try, at the next address, in the next
//! round or the next run, goes on from its bytes (`Range`, as the game's
//! archive). Before a patch file is deleted or renamed,
//! `resources.db` is opened by this program alone: the game keeps it open
//! while it runs (and not its patches), so that is how a running game is
//! told. A file another program holds a moment (an antivirus that scans a
//! new file) is tried again a few times first.
//!
//! Beside the updater: `updater_skhid.patches`, the server's last list, and
//! `updater_skhid.patches.status`, `ready` or `blocked <why>`. `ready`
//! (play_skhid.bat) and a start that cannot reach the server check the
//! folder against that list. The status is written only by a run that got
//! a list it can use: `blocked` once the folder is about to change, `ready`
//! once it is checked; a run that cannot fetch the list or stops before it
//! changes anything leaves it as it was. A new overlay makes it `blocked`
//! (main.rs, [`overlay_changed`]): the new files may need new patches, so
//! only a run with the server's list makes it `ready` again.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use updater_logic::checked::{Identity, Record, parse_record, write_record};
use updater_logic::download::{Action, Decision, Event, Failure, Mirrors};
use updater_logic::file_error::Stop;
use updater_logic::patches::{
    Address, FILE_PREFIX, Fetch, List, ListError, Local, MAX_LIST, Next, PART_PATTERN, Part, Patch,
    Problem, Tried, after, check, overlay_patch, parse_list, todo, write_list,
};
use updater_logic::pipe::{FileOf, ToLauncher, state, step};
use updater_logic::problem::{self, Fault};
use updater_logic::sha256::to_hex;
use updater_logic::url::Url;
use windows_sys::Win32::Foundation::{
    ERROR_ACCESS_DENIED, ERROR_DISK_FULL, ERROR_HANDLE_DISK_FULL, ERROR_LOCK_VIOLATION,
    ERROR_SHARING_VIOLATION,
};

use crate::Session;
use crate::http::{self, HttpError};
use crate::install::Layout;
use crate::log::log;
use crate::text;
use crate::torrent;
use crate::win::{self, Code, File};

/// Beside the updater: the server's last list, as it was read.
const LIST_FILE: &str = "updater_skhid.patches";
/// `ready` once the folder was checked against that list, else `blocked <why>`.
const STATUS_FILE: &str = "updater_skhid.patches.status";
/// What each patch file was when it was last hashed (`updater_logic::checked`).
const CHECKED_FILE: &str = "updater_skhid.patches.checked";
/// The game's own db, in the game folder: the game holds it while it runs.
const DB_NAME: &str = "resources.db";
/// A patch while the torrent fetches it, beside its place. It is a
/// `resources.db.patch_*` the game does not take, so one left over is
/// deleted as any other (as are those of updaters before patch downloads
/// went on from their bytes).
const PART: &str = ".skhid-part";
/// Room left on the disk after a patch is downloaded.
const DISK_MARGIN: u64 = 64 << 20;
/// The largest patch taken: a bound only, the disk decides first.
const MAX_PATCH: u64 = 1 << 40;
/// How often the list is asked for in one run, and how many rounds over a
/// patch's addresses are made (only an address that failed on the network
/// is tried in the second).
const LIST_TRIES: u32 = 3;
const FETCH_TRIES: u32 = 2;
const RETRY_PAUSE_MS: u32 = 3000;
/// How often a file another program holds is tried (to open, delete,
/// rename), and the pause between: about a second in all.
const BUSY_TRIES: u32 = 6;
const BUSY_PAUSE_MS: u32 = 200;

/// How the step ended.
pub enum Outcome {
    /// Every listed patch is in place: the game may be played.
    Ready,
    /// The launcher asked to stop.
    Stopped,
    /// PLAY stays off, with these words.
    Blocked(String),
    /// Windows Security does not let this patch file be written, renamed
    /// or deleted (`updater_logic::file_error`): PLAY stays off, and the
    /// player is told how to let it through.
    Stop(Stop, String),
}

/// How the patch files are looked at.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Look {
    /// A file still as it was hashed in the last week is not read again.
    Quick,
    /// Every file is hashed (`updater_skhid.exe check`: the repair).
    Full,
}

fn hex(sha256: &[u8; 32]) -> String {
    String::from_utf8_lossy(&to_hex(sha256)).into()
}

fn place(layout: &Layout, file_name: &str) -> String {
    format!("{}\\{file_name}", layout.game_dir)
}

fn beside(layout: &Layout, name: &str) -> String {
    format!("{}\\{name}", layout.exe_dir)
}

/// The patch files in the game folder, by their names.
fn files(layout: &Layout) -> Vec<String> {
    win::list(&layout.game_dir, &format!("{FILE_PREFIX}*"))
}

/// The record of the last hashes, or none (every file is hashed).
fn read_record(layout: &Layout, look: Look) -> Record {
    match win::read_file(&beside(layout, CHECKED_FILE), 1 << 20) {
        Ok(bytes) => {
            let record = parse_record(&bytes);
            if record.files.is_empty() && look == Look::Quick {
                log!("patches: {CHECKED_FILE} has no file in it: every patch file is hashed");
            }
            record
        }
        Err(code) => {
            if look == Look::Quick {
                log!(
                    "patches: {CHECKED_FILE}: {}: every patch file is hashed",
                    win::error_text(code)
                );
            }
            Record::default()
        }
    }
}

/// Keeps the record, if it changed. Best effort: without it the next start
/// hashes every file.
fn save_record(layout: &Layout, record: &Record) {
    let path = beside(layout, CHECKED_FILE);
    if let Err(code) = save(&path, &write_record(record)) {
        log!(
            "patches: {CHECKED_FILE} cannot be written ({}): the next start hashes every file",
            win::error_text(code)
        );
    }
}

/// A file just downloaded, checked again in its place: by its file ID when
/// the rename kept it (the same file that was hashed), else hashed now.
fn look_in_place(layout: &Layout, patch: &Patch, record: &mut Record) -> Local {
    let path = place(layout, &patch.file_name());
    match win::hash_file_identity(&path, &mut |_| true) {
        Ok((_, sha256, identity)) => {
            match identity {
                Some(identity) => record.hashed(patch.number, sha256, identity, win::now()),
                None => record.forget(patch.number),
            }
            Local::Found(sha256)
        }
        Err(code) if win::is_missing(code) => Local::Missing,
        Err(code) => {
            log!("patches: {path} cannot be read ({})", win::error_text(code));
            Local::Unreadable
        }
    }
}

/// Every listed patch's file looked at once, by number: with [`Look::Quick`]
/// a file the record vouches for is not read; every other one is hashed,
/// and the record notes what it found. With a launcher it is told:
/// "checking", and the bar over all the bytes hashed. `None`: told to stop.
fn survey(
    layout: &Layout,
    list: &List,
    record: &mut Record,
    look: Look,
    mut session: Option<&mut Session>,
) -> Option<Vec<(u8, Local)>> {
    let now = win::now();
    let mut found = Vec::new();
    let mut to_hash: Vec<(&Patch, String)> = Vec::new();
    for patch in &list.patches {
        let path = place(layout, &patch.file_name());
        if look == Look::Quick {
            match win::identity(&path) {
                Ok(identity) if record.still(patch.number, &patch.sha256, &identity, now) => {
                    found.push((patch.number, Local::Found(patch.sha256)));
                    continue;
                }
                Err(code) if win::is_missing(code) => {
                    record.forget(patch.number);
                    found.push((patch.number, Local::Missing));
                    continue;
                }
                _ => {}
            }
        }
        to_hash.push((patch, path));
    }
    let quick = found.len();
    let total: u64 = to_hash
        .iter()
        .filter_map(|(_, path)| win::file_size(path).ok())
        .sum();
    if total > 0
        && let Some(session) = session.as_deref_mut()
    {
        session.state(state::CHECKING);
        session.begin_progress();
    }
    let started = win::ticks();
    let mut done = 0_u64;
    for (patch, path) in &to_hash {
        if let Some(session) = session.as_deref_mut()
            && !session.pump()
        {
            return None;
        }
        let result = win::hash_file_identity(path, &mut |read| match session.as_deref_mut() {
            Some(session) => session.progress(done + read, total),
            None => true,
        });
        let local = match result {
            Ok((size, sha256, identity)) => {
                done += size;
                match identity {
                    Some(identity) => record.hashed(patch.number, sha256, identity, now),
                    None => record.forget(patch.number),
                }
                Local::Found(sha256)
            }
            Err(code) if win::is_missing(code) => Local::Missing,
            Err(_) if session.as_deref().is_some_and(|session| session.stopped()) => return None,
            Err(code) => {
                log!("patches: {path} cannot be read ({})", win::error_text(code));
                Local::Unreadable
            }
        };
        if local != Local::Found(patch.sha256) {
            record.forget(patch.number);
        }
        found.push((patch.number, local));
    }
    if quick > 0 {
        log!("patches: {quick} file(s) as they were when hashed in the last week: not read again");
    }
    if total > 0 {
        log!(
            "patches: {} file(s), {done} bytes, hashed in {} ms",
            to_hash.len(),
            win::ticks().saturating_sub(started)
        );
    }
    Some(found)
}

/// What [`survey`] found for `patch`.
fn lookup(found: &[(u8, Local)], patch: &Patch) -> Local {
    found
        .iter()
        .find(|(number, _)| *number == patch.number)
        .map_or(Local::Missing, |(_, local)| *local)
}

fn problem(problem: &Problem) -> String {
    match problem {
        Problem::Extra(name) => format!("{name} is not in the list"),
        Problem::Missing(number) => format!("{FILE_PREFIX}{number} is missing"),
        Problem::Damaged(number) => {
            format!("{FILE_PREFIX}{number} does not have the list's checksum")
        }
    }
}

/// The folder checked against the list kept beside the updater, after a
/// run that ended `ready`: `Err(Some(why))` says what is not in place, for
/// the log; `Err(None)`: told to stop. `record` is used and brought up to
/// date (the caller keeps it, or not); `look`: [`Look::Full`] hashes every
/// file.
fn kept_in_place(
    layout: &Layout,
    session: Option<&mut Session>,
    record: &mut Record,
    look: Look,
) -> Result<(), Option<String>> {
    let status = win::read_file(&beside(layout, STATUS_FILE), 4096)
        .map_err(|code| Some(format!("{STATUS_FILE}: {}", win::error_text(code))))?;
    if status.trim_ascii() != b"ready" {
        return Err(Some(format!(
            "the last run did not finish: {}",
            String::from_utf8_lossy(status.trim_ascii())
        )));
    }
    let bytes = win::read_file(&beside(layout, LIST_FILE), MAX_LIST)
        .map_err(|code| Some(format!("{LIST_FILE}: {}", win::error_text(code))))?;
    let list = parse_list(&bytes).map_err(|error| Some(format!("{LIST_FILE}: {error:?}")))?;
    record.keep_only(&numbers(&list));
    let found = survey(layout, &list, record, look, session).ok_or(None)?;
    check(&list, &files(layout), &mut |patch| lookup(&found, patch)).map_err(|p| Some(problem(&p)))
}

fn numbers(list: &List) -> Vec<u8> {
    list.patches.iter().map(|patch| patch.number).collect()
}

/// `updater_skhid.exe ready`, from play_skhid.bat and play_skhid.sh: reads
/// only (the quick check too: the record is not written). `Err` (the words
/// for the player) unless the last run ended with every patch in place and
/// the folder still is so.
pub fn ready(layout: &Layout) -> Result<(), String> {
    let mut record = read_record(layout, Look::Quick);
    match kept_in_place(layout, None, &mut record, Look::Quick) {
        Ok(()) => {
            log!("ready: the patches are in place");
            Ok(())
        }
        Err(why) => {
            log!(
                "ready: NOT READY: patches: {}: the game is not started",
                why.unwrap_or_default()
            );
            Err(text::patches_not_ready())
        }
    }
}

/// The overlay installed new files: the patches kept are not known to be
/// theirs, so `ready` and a start without the server refuse until a run
/// with the server's list checks the folder again.
pub fn overlay_changed(layout: &Layout) {
    let path = beside(layout, STATUS_FILE);
    if let Err(code) = save(&path, b"blocked overlay changed\r\n") {
        log!(
            "patches: {STATUS_FILE} cannot be written ({}): it is deleted",
            win::error_text(code)
        );
        if let Err(code) = win::delete(&path) {
            log!(
                "patches: {STATUS_FILE} cannot be deleted either ({})",
                win::error_text(code)
            );
        }
    }
}

/// Why the step stopped short.
enum Fail {
    Stopped,
    /// PLAY stays off: what the player is told, what the log and the status
    /// file say.
    Blocked(String, String),
    /// A file Windows Security blocks (its name), what the log and the
    /// status file say.
    Stop(Stop, String, String),
    /// The list could not be fetched (the network, a status but 404): as
    /// when the server could not be asked at all. The player's words, the
    /// log's.
    Offline(String, String),
}

/// Why one address did not give a patch's file.
enum Miss {
    Stopped,
    /// Left for a faster address (`Mirrors::at`), its bytes kept.
    Moved,
    /// Here: the disk, a file that cannot be written. No other address
    /// would do better: the fetch ends.
    Here(Fail),
    /// The address: the network, its answer, the file it sent. The next
    /// address is tried.
    There {
        /// A sentence for the player.
        reason: String,
        /// For the log.
        why: String,
        /// How: not reached, or what it sent (another SHA-256: damaged).
        miss: problem::Miss,
        /// The network (a second pass may do better), or an answer that is
        /// not the file.
        failure: Failure,
    },
}

/// A code of a file that another program has open, or is deleting.
fn held(code: Code) -> bool {
    code == ERROR_SHARING_VIOLATION || code == ERROR_LOCK_VIOLATION || code == ERROR_ACCESS_DENIED
}

/// `operation` on `path`, tried again a few times while another program
/// holds the file (an antivirus scanning it).
fn retried<T>(
    what: &str,
    path: &str,
    operation: &mut dyn FnMut() -> Result<T, Code>,
) -> Result<T, Code> {
    let mut attempt = 1;
    loop {
        match operation() {
            Err(code) if held(code) && attempt < BUSY_TRIES => {
                log!(
                    "patches: {path} cannot be {what} now ({}), try {attempt} of {BUSY_TRIES}: \
                     another program holds it",
                    win::error_text(code)
                );
                win::sleep(BUSY_PAUSE_MS);
                attempt += 1;
            }
            other => return other,
        }
    }
}

/// The words for a patch file that cannot be deleted, written or replaced,
/// tried a few times. The game does not hold these (that is told by
/// resources.db, [`Run::guard`]): another program does.
fn file_fail(path: &str, code: Code, what: &str) -> Fail {
    let why = format!(
        "{path} cannot be {what} ({}, error {code})",
        win::error_text(code)
    );
    let file = path.rsplit('\\').next().unwrap_or(path);
    // The patch's name, not its download's.
    let name = match updater_logic::patches::part_of(file) {
        Some((number, _)) => format!("{FILE_PREFIX}{number}"),
        None => file.strip_suffix(PART).unwrap_or(file).into(),
    };
    if Stop::of(code) == Some(Stop::Virus) {
        Fail::Stop(Stop::Virus, name, why)
    } else if code == ERROR_SHARING_VIOLATION || code == ERROR_LOCK_VIOLATION {
        Fail::Blocked(text::patches_held(&name), why)
    } else {
        Fail::Blocked(text::patches_failed(), why)
    }
}

struct Run<'a> {
    session: &'a mut Session,
    layout: &'a Layout,
    /// Where the list comes from: the server, or the list built into this
    /// updater ([`built_in`]) when the server could not be reached.
    source: Source<'a>,
    look: Look,
    record: &'a mut Record,
    /// The folder is about to change, or has: the status says so.
    changing: bool,
    /// Patches the torrent gave, checked in their parts, with their identity
    /// when hashed.
    torrented: Vec<(u8, Option<Identity>)>,
}

impl Run<'_> {
    /// The server's list, asked for a few times; or the one built in.
    fn list(&mut self) -> Result<List, Fail> {
        let server = match &self.source {
            Source::Server(server) => *server,
            Source::BuiltIn(list) | Source::Torrent(list) => return Ok(list.clone()),
        };
        let mut last = String::new();
        for attempt in 1..=LIST_TRIES {
            let session = &mut *self.session;
            let result = http::get(server, "/patches", MAX_LIST, &mut |_, _| session.pump());
            self.session.reached(server, result.as_ref().map(|_| ()));
            let error = match result {
                Ok(body) => {
                    return parse_list(&body).map_err(|error| {
                        Fail::Blocked(
                            text::patches_bad_list(server, &list_error(&error)),
                            format!("the list is not one ({error:?})"),
                        )
                    });
                }
                Err(HttpError::Stopped) => return Err(Fail::Stopped),
                Err(HttpError::Status(404)) => {
                    return Err(Fail::Blocked(
                        text::patches_not_offered(server),
                        "the server has no /patches".into(),
                    ));
                }
                Err(HttpError::TooLarge) => {
                    return Err(Fail::Blocked(
                        text::patches_bad_list(server, "too large"),
                        "the list is too large".into(),
                    ));
                }
                Err(error) => error,
            };
            last = text::http(server, "/patches", &error);
            log!("patches: the list, try {attempt} of {LIST_TRIES}: {last}");
            if attempt < LIST_TRIES && !self.session.pause(RETRY_PAUSE_MS) {
                return Err(Fail::Stopped);
            }
        }
        Err(Fail::Offline(
            text::patches_unreachable(&last),
            format!("the list could not be fetched: {last}"),
        ))
    }

    /// Before a patch file is deleted or renamed: `resources.db` opened by
    /// this program alone, and closed again. The game keeps it open while it
    /// runs; a moment's hold (an antivirus) is waited for.
    fn guard(&self) -> Result<(), Fail> {
        let db = place(self.layout, DB_NAME);
        let mut last = 0;
        for attempt in 1..=BUSY_TRIES {
            match File::alone(&db) {
                Ok(_) => return Ok(()),
                // No db, no game that runs: the patch files are this step's.
                Err(code) if win::is_missing(code) => return Ok(()),
                Err(code) if code == ERROR_SHARING_VIOLATION || code == ERROR_LOCK_VIOLATION => {
                    last = code;
                    if attempt < BUSY_TRIES {
                        win::sleep(BUSY_PAUSE_MS);
                    }
                }
                Err(code) => {
                    return Err(Fail::Blocked(
                        text::patches_failed(),
                        format!("{db} cannot be opened ({})", win::error_text(code)),
                    ));
                }
            }
        }
        Err(Fail::Blocked(
            text::patches_game_running(),
            format!(
                "{db} is open in another program ({}): the game is running",
                win::error_text(last)
            ),
        ))
    }

    fn delete(&mut self, name: &str) -> Result<(), Fail> {
        self.guard()?;
        let path = place(self.layout, name);
        match retried("deleted", &path, &mut || win::delete(&path)) {
            Ok(()) => {
                log!("patches: deleted {name}");
                if let Some(number) = updater_logic::patches::file_number(name) {
                    self.record.forget(number);
                }
                Ok(())
            }
            Err(code) => Err(file_fail(&path, code, "deleted")),
        }
    }

    /// Downloads one patch into its place, from the first of its addresses
    /// that gives it, unless the launcher turns torrent on meanwhile. Each
    /// try goes on from what its part holds.
    fn fetch(&mut self, fetch: &Fetch) -> Result<bool, Fail> {
        let patch = &fetch.patch;
        let name = patch.file_name();
        let target = place(self.layout, &name);
        let part = place(self.layout, &patch.part_name());
        if let Some(at) = self.torrented.iter().position(|(number, _)| *number == patch.number) {
            let (_, identity) = self.torrented.remove(at);
            let torrented = format!("{target}{PART}");
            let placed = self.put(patch, &torrented, &target, identity, "by torrent", 0)?;
            if fetch.part != Part::Nothing {
                self.drop_part(&part, "the torrent gave the patch");
            }
            return Ok(placed);
        }
        if patch.addresses.is_empty() {
            return Err(Fail::Blocked(
                text::patches_download(&name, "it is the revision's torrent's alone, and no peer gave it."),
                format!("{name}: no address, and the torrent did not give it"),
            ));
        }
        let count = patch.addresses.len();
        // What the part holds, which address is in use and why (failures in
        // order, two passes; a slow one leaves for the fastest measured),
        // the first failure's words, and how each address failed last.
        let mut held = fetch.part;
        let mut mirrors = Mirrors::new(count, FETCH_TRIES);
        let mut first: Option<(String, problem::Miss, &Address)> = None;
        let mut misses: Vec<Option<problem::Miss>> = alloc::vec![None; count];
        self.session.begin_progress();
        loop {
            if !self.session.pump() {
                return Err(Fail::Stopped);
            }
            let at = mirrors.at();
            let address = &patch.addresses[at];
            let pass = mirrors.pass();
            self.session.state(state::DOWNLOADING);
            self.session.restart_speed();
            log!(
                "patches: {name}: address {} of {count}, {} (try {pass} of {FETCH_TRIES}): \
                 asking, from byte {}",
                at + 1,
                address.text,
                held.from()
            );
            mirrors.on(Event::Begin {
                ticks: win::ticks(),
                done: held.from(),
            });
            let (result, now) = self.download(&part, patch, &mut mirrors, held);
            held = now;
            let (reason, why, miss, failure) = match result {
                Ok(identity) => {
                    return self.put(patch, &part, &target, identity, &address.text, at + 1);
                }
                Err(Miss::Moved) => continue,
                Err(Miss::Stopped) => return Err(Fail::Stopped),
                Err(Miss::Here(fail)) => return Err(fail),
                Err(Miss::There {
                    reason,
                    why,
                    miss,
                    failure,
                }) => (reason, why, miss, failure),
            };
            log!(
                "patches: {name}: address {} of {count}, {} (try {pass} of {FETCH_TRIES}): {why}",
                at + 1,
                address.text
            );
            misses[at] = Some(miss);
            if first.is_none() {
                first = Some((reason, miss, address));
            }
            match mirrors.on(Event::Failed {
                ticks: win::ticks(),
                failure,
            }) {
                Action::Go { to, pause, decision } => {
                    if let Some(decision) = decision {
                        log!("patches: {name}: {}", decision_line(patch, to, held.from(), &decision));
                    }
                    if pause && !self.session.pause(RETRY_PAUSE_MS) {
                        return Err(Fail::Stopped);
                    }
                }
                Action::Stay | Action::GiveUp => break,
            }
        }
        // Bytes got stay for the next start; no byte, no file.
        if held == Part::Nothing {
            let _ = win::delete(&part);
        } else {
            log!(
                "patches: {name}: {} bytes kept in {}: the next start goes on from them",
                held.from(),
                patch.part_name()
            );
        }
        let (reason, miss, address) = first.expect("an address failed");
        let misses: Vec<problem::Miss> = misses.into_iter().flatten().collect();
        self.session.failed(&name, &misses);
        let message = if count > 1 {
            text::patches_no_address(&name, count, &reason)
        } else if miss == problem::Miss::Answered(Fault::Damaged) {
            text::patches_wrong(&address.url, &name)
        } else {
            text::patches_download(&name, &reason)
        };
        Err(Fail::Blocked(
            message,
            format!("{name}: none of its {count} address(es) gave it"),
        ))
    }

    /// A part deleted, best effort: no list goes on from it.
    fn drop_part(&self, part: &str, why: &str) {
        match win::delete(part) {
            Ok(()) => log!("patches: deleted {part}: {why}"),
            Err(code) if win::is_missing(code) => {}
            Err(code) => log!("patches: {part} cannot be deleted ({})", win::error_text(code)),
        }
    }

    /// One try of one address, from what `part` holds (`held`): its bytes
    /// on into the part, flushed, read back and checked once whole. What
    /// Windows says of the part then (to know it in its place); and what the
    /// part holds after the try.
    fn download(
        &mut self,
        part: &str,
        patch: &Patch,
        mirrors: &mut Mirrors,
        held: Part,
    ) -> (Result<Option<Identity>, Miss>, Part) {
        let from = held.from();
        let at = mirrors.at();
        let address = &patch.addresses[at];
        let mut moved = false;
        let output = match retried("written", part, &mut || File::download(part, from)) {
            Ok(output) => output,
            Err(code) => return (Err(Miss::Here(file_fail(part, code, "written"))), held),
        };
        let folder = self.layout.game_dir.clone();
        let name = patch.file_name();
        let mut no_room: Option<(u64, u64)> = None;
        let mut announced: Option<u64> = None;
        let mut asked = false;
        let session = &mut *self.session;
        let saved = http::get_file_into(
            &output,
            &address.url,
            from,
            MAX_PATCH,
            &mut |to| log!("patches: {name}: {} redirects to {to}", address.text),
            &mut |done, total| {
                // Before the first byte: is there room for what is still to
                // come? (A whole file again from byte 0 takes the part's
                // place.)
                if !asked {
                    asked = true;
                    announced = total;
                    if done == 0 && from > 0 {
                        log!("patches: {name}: {} answers the whole file: from byte 0", address.text);
                    }
                    let needed = total.unwrap_or(0).saturating_sub(done).saturating_add(DISK_MARGIN);
                    let freed = if done == 0 { from } else { 0 };
                    if let Some(free) = win::free_space(&folder)
                        && free.saturating_add(freed) < needed
                    {
                        no_room = Some((needed, free));
                        return false;
                    }
                }
                if !session.progress(done, total.unwrap_or(0)) {
                    return false;
                }
                let event = Event::Progress {
                    ticks: win::ticks(),
                    done,
                    size: total,
                };
                match mirrors.on(event) {
                    Action::Go { to, decision, .. } => {
                        if let Some(decision) = decision {
                            log!("patches: {name}: {}", decision_line(patch, to, done, &decision));
                        }
                        // The whole file here already: nothing to move for.
                        moved = to != at && total != Some(done);
                        !moved
                    }
                    Action::Stay | Action::GiveUp => true,
                }
            },
        );
        drop(output);
        let saved = match saved {
            Ok(saved) => saved,
            Err(http::DiskError::Write(code) | http::DiskError::Flush(code)) => {
                let now = Part::of_size(win::file_size(part).unwrap_or(0));
                if code == ERROR_DISK_FULL || code == ERROR_HANDLE_DISK_FULL {
                    // What the patch still needs, past what is written of it.
                    let free = win::free_space(&folder).unwrap_or(0);
                    let needed = announced
                        .map_or(0, |total| total.saturating_sub(now.from()))
                        .saturating_add(DISK_MARGIN);
                    return (
                        Err(Miss::Here(Fail::Blocked(
                            text::disk_full(&folder, needed, free),
                            format!(
                                "disk: the disk is full ({free} bytes free in {folder}, {needed} \
                                 needed)"
                            ),
                        ))),
                        now,
                    );
                }
                return (Err(Miss::Here(file_fail(part, code, "written"))), now);
            }
        };
        if let Some((needed, free)) = no_room {
            return (
                Err(Miss::Here(Fail::Blocked(
                    text::disk_full(&folder, needed, free),
                    format!("disk: {free} bytes free in {folder}, {needed} needed"),
                ))),
                Part::of_size(saved.end),
            );
        }
        if moved {
            return (Err(Miss::Moved), Part::of_size(saved.end));
        }
        let tried = match &saved.result {
            Ok(()) => Tried::Complete { end: saved.end },
            Err(HttpError::PastEnd(size)) => Tried::PastEnd(*size),
            Err(_) => Tried::Broke { end: saved.end },
        };
        match after(held, tried) {
            Next::Keep(now) => {
                let error = saved.result.expect_err("a try that broke off");
                if error == HttpError::Stopped {
                    return (Err(Miss::Stopped), now);
                }
                let message = text::http(&address.url, &address.url.prefix, &error);
                (
                    Err(Miss::There {
                        why: message.clone(),
                        reason: message,
                        miss: error.miss(),
                        failure: error.failure(),
                    }),
                    now,
                )
            }
            Next::Delete => {
                self.drop_part(part, "longer than the address's file");
                let error = saved.result.expect_err("a 416");
                let message = text::http(&address.url, &address.url.prefix, &error);
                // The part was at fault, not the address: the next one, and
                // the second pass, ask from byte 0.
                (
                    Err(Miss::There {
                        why: message.clone(),
                        reason: message,
                        miss: error.miss(),
                        failure: Failure::Network,
                    }),
                    Part::Nothing,
                )
            }
            Next::Hash => self.verify(part, patch, address, from, saved.end),
        }
    }

    /// A part that may be whole, hashed: the list's SHA-256 decides. Another
    /// one deletes it; when the try went on from bytes kept (`from` > 0),
    /// those may be what was wrong, so a second round asks the address
    /// again from byte 0.
    fn verify(
        &mut self,
        part: &str,
        patch: &Patch,
        address: &Address,
        from: u64,
        end: u64,
    ) -> (Result<Option<Identity>, Miss>, Part) {
        match win::hash_file_identity(part, &mut |_| true) {
            Ok((_, sha256, identity)) if sha256 == patch.sha256 => (Ok(identity), Part::of_size(end)),
            Ok((size, sha256, _)) => {
                self.drop_part(part, "another SHA-256 than the list's");
                (
                    Err(Miss::There {
                        reason: format!(
                            "{} sent a file with another checksum than the list says.",
                            address.url.host
                        ),
                        why: format!(
                            "{} ({size} bytes{}) has SHA-256 {}, the list says {}",
                            address.text,
                            if from > 0 {
                                format!(", gone on from byte {from}")
                            } else {
                                String::new()
                            },
                            hex(&sha256),
                            hex(&patch.sha256)
                        ),
                        miss: problem::Miss::Answered(Fault::Damaged),
                        failure: if from > 0 { Failure::Network } else { Failure::Refused },
                    }),
                    Part::Nothing,
                )
            }
            // Gone or denied the moment it was written: an antivirus took it.
            Err(code) if Stop::after_write(code) == Some(Stop::Virus) => (
                Err(Miss::Here(Fail::Stop(
                    Stop::Virus,
                    patch.file_name(),
                    format!(
                        "{part} cannot be read back ({}, error {code}): an antivirus took it",
                        win::error_text(code)
                    ),
                ))),
                Part::of_size(win::file_size(part).unwrap_or(0)),
            ),
            Err(code) => (
                Err(Miss::Here(file_fail(part, code, "read back"))),
                Part::of_size(end),
            ),
        }
    }

    /// A checked part into its place, unless the game runs now; `true` as in
    /// [`Got::Placed`]. `address`: its number in the list, 0 for the torrent.
    /// A part that cannot be put in place stays: the next start finds it
    /// whole.
    fn put(
        &mut self,
        patch: &Patch,
        part: &str,
        target: &str,
        identity: Option<Identity>,
        from: &str,
        address: usize,
    ) -> Result<bool, Fail> {
        let name = patch.file_name();
        self.guard()?;
        if let Err(code) = retried("renamed", part, &mut || win::rename(part, target, true)) {
            return Err(file_fail(target, code, "replaced"));
        }
        self.session.state(state::INSTALLING);
        self.session.send(ToLauncher::InstallInfo {
            step: step::EXTRACT,
            text: &name,
        });
        self.session.named_file = true;
        let count = patch.addresses.len();
        log!(
            "patches: {name}: downloaded {from}{}, checksum right ({})",
            if address > 0 {
                format!(" (address {address} of {count})")
            } else {
                String::new()
            },
            hex(&patch.sha256)
        );
        // The file hashed is the one in place when the rename kept its ID.
        let kept = identity
            .filter(|identity| win::identity(target).ok() == Some(*identity))
            .inspect(|identity| {
                self.record
                    .hashed(patch.number, patch.sha256, *identity, win::now());
            });
        Ok(kept.is_some())
    }

    /// Downloads every patch of `files`, the launcher's line saying which
    /// of how many: each one's number, and whether it is known in place by
    /// the file ID.
    fn fetch_all(&mut self, files: &[Fetch]) -> Result<Vec<(u8, bool)>, Fail> {
        let mut downloaded = Vec::new();
        for (index, fetch) in files.iter().enumerate() {
            if !self.session.pump() {
                return Err(Fail::Stopped);
            }
            if fetch.damaged {
                log!(
                    "patches: {} does not have the list's checksum: downloaded again",
                    fetch.patch.file_name()
                );
                self.delete(&fetch.patch.file_name())?;
            }
            self.session.send(ToLauncher::FileOf(FileOf::at(index, files.len())));
            let known = self.fetch(fetch)?;
            downloaded.push((fetch.patch.number, known));
        }
        Ok(downloaded)
    }

    /// What the torrent has of `fetch`, into parts checked against the
    /// list's SHA-256 for [`Run::fetch`] to put in place; the rest comes
    /// from their addresses.
    fn by_torrent(&mut self, fetch: &[&Patch]) -> Result<(), Fail> {
        let Some(offered) = torrent::offered(self.session, self.layout) else {
            return Ok(());
        };
        let wanted: Vec<(&Patch, usize, String)> = fetch
            .iter()
            .filter_map(|patch| {
                let index = torrent::file_index(&offered.torrent, &format!("game/{}", patch.file_name()))?;
                let part = format!("{}{PART}", place(self.layout, &patch.file_name()));
                Some((*patch, index, part))
            })
            .collect();
        if wanted.is_empty() {
            return Ok(());
        }
        let needed = wanted
            .iter()
            .map(|(_, index, _)| offered.torrent.files[*index].size)
            .sum::<u64>()
            .saturating_add(DISK_MARGIN);
        if win::free_space(&self.layout.game_dir).is_none_or(|free| free < needed) {
            return Ok(());
        }
        log!("patches: {} by torrent", wanted.len());
        let files: Vec<(usize, String)> = wanted
            .iter()
            .map(|(_, index, part)| (*index, part.clone()))
            .collect();
        let outcome = torrent::fetch(self.session, &offered, &files);
        for (patch, _, part) in &wanted {
            if let torrent::Fetched::Done = outcome {
                match win::hash_file_identity(part, &mut |_| true) {
                    Ok((_, sha256, identity)) if sha256 == patch.sha256 => {
                        self.torrented.push((patch.number, identity));
                        continue;
                    }
                    Ok((_, sha256, _)) => log!(
                        "patches: {}: the torrent's has SHA-256 {}, the list says {}",
                        patch.file_name(),
                        hex(&sha256),
                        hex(&patch.sha256)
                    ),
                    Err(code) => log!(
                        "patches: {part} cannot be read back ({})",
                        win::error_text(code)
                    ),
                }
            }
            let _ = win::delete(part);
        }
        match outcome {
            torrent::Fetched::Stopped => Err(Fail::Stopped),
            torrent::Fetched::Not(why) => {
                log!("patches: by torrent: {why}: from their addresses");
                Ok(())
            }
            torrent::Fetched::Done => Ok(()),
        }
    }

    fn step(&mut self, overlay: &[&str]) -> Result<(), Fail> {
        if let Some(name) = overlay_patch(overlay) {
            // The overlay put it in the folder already.
            self.changing = true;
            return Err(Fail::Blocked(
                text::patches_failed(),
                format!("the overlay has {name}: patches come from the list alone"),
            ));
        }
        let list = self.list()?;
        self.record.keep_only(&numbers(&list));
        let found = files(self.layout);
        let hashed = survey(
            self.layout,
            &list,
            self.record,
            self.look,
            Some(&mut *self.session),
        )
        .ok_or(Fail::Stopped)?;
        let parts: Vec<(String, u64)> = win::list(&self.layout.game_dir, PART_PATTERN)
            .into_iter()
            .map(|name| {
                let size = win::file_size(&place(self.layout, &name)).unwrap_or(0);
                (name, size)
            })
            .collect();
        let mut todo = todo(&list, &found, &parts, &mut |patch| lookup(&hashed, patch));
        // The list built in (the server not reached) only adds: the game exe
        // may be newer than this updater (one restored from an old archive,
        // or the .bat's install, which keeps the updater it has) and need
        // patches this list does not name. Nor is it kept as the server's.
        let built_in = matches!(self.source, Source::BuiltIn(_));
        if built_in && !todo.delete.is_empty() {
            log!(
                "patches: the list built in: {} patch file(s) it has not are kept ({})",
                todo.delete.len(),
                todo.delete.join(", ")
            );
            todo.delete.clear();
        }
        if built_in {
            todo.stale.clear();
        }
        for name in &todo.stale {
            self.drop_part(&place(self.layout, name), "no patch to download has it");
        }
        for fetch in &todo.fetch {
            if let Part::Partial(have) = fetch.part {
                log!(
                    "patches: {}: {have} bytes of it kept from before: gone on from",
                    fetch.patch.file_name()
                );
            }
        }
        log!(
            "patches: the list has {} patch(es){}; {} to download, {} to delete",
            list.patches.len(),
            if list.patches.is_empty() {
                String::new()
            } else {
                format!(
                    " ({})",
                    list.patches
                        .iter()
                        .map(Patch::name)
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            },
            todo.fetch.len(),
            todo.delete.len()
        );
        // The list is kept before the folder is changed, and the status says
        // `blocked` until the run is through: `ready` checks against the
        // newest.
        let list_path = beside(self.layout, LIST_FILE);
        let list_text = write_list(&list);
        let kept = win::read_file(&list_path, MAX_LIST).ok();
        if !built_in && (!todo.is_empty() || kept.as_deref() != Some(&list_text[..])) {
            self.changing = true;
            if let Err(code) = save(&beside(self.layout, STATUS_FILE), b"blocked checking\r\n") {
                log!(
                    "patches: {STATUS_FILE} cannot be written ({})",
                    win::error_text(code)
                );
            }
            if let Err(code) = save(&list_path, &list_text) {
                return Err(Fail::Blocked(
                    text::patches_failed(),
                    format!("{LIST_FILE} cannot be written ({})", win::error_text(code)),
                ));
            }
        }
        for name in &todo.delete {
            self.delete(name)?;
        }
        // The site answered: its addresses, as with torrent off.
        if !matches!(self.source, Source::Server(_)) {
            let wanted: Vec<&Patch> = todo.fetch.iter().map(|fetch| &fetch.patch).collect();
            self.by_torrent(&wanted)?;
        }
        let fetched = self.fetch_all(&todo.fetch);
        self.session.send(ToLauncher::FileOf(None));
        let downloaded = fetched?;
        // The folder against the list, as `ready` checks it: no other patch
        // file, the files downloaded as they were checked (by their file ID,
        // else hashed in their place), the others as they were looked at
        // above (nothing has changed them since).
        let layout = self.layout;
        let record = &mut *self.record;
        // Built in: the patches it lists in place, any other let be.
        let others = if built_in { Vec::new() } else { files(layout) };
        check(&list, &others, &mut |patch| match downloaded
            .iter()
            .find(|(number, _)| *number == patch.number)
        {
            Some((_, true)) => Local::Found(patch.sha256),
            Some((_, false)) => look_in_place(layout, patch, record),
            None => lookup(&hashed, patch),
        })
        .map_err(|problem| {
            Fail::Blocked(
                text::patches_failed(),
                format!("after the step: {}", self::problem(&problem)),
            )
        })?;
        log!(
            "patches: {} patch(es) in place, checked",
            list.patches.len()
        );
        Ok(())
    }
}

/// A decision of speed for the log, the patch's addresses named by their
/// hosts.
fn decision_line(patch: &Patch, to: usize, from: u64, decision: &Decision) -> String {
    decision.line(to, from, &|at| patch.addresses[at].url.host.clone())
}

fn list_error(error: &ListError) -> String {
    match error {
        ListError::TooLarge => "too large".into(),
        ListError::Syntax(at) => format!("not JSON at byte {at}"),
        ListError::BadName(name) => format!("bad name {}", name.escape_debug()),
        ListError::BadEntry(name) => format!("bad entry {}", name.escape_debug()),
        ListError::BadHash(name) => format!("bad checksum of {}", name.escape_debug()),
        ListError::BadUrl(name) => format!("bad address of {}", name.escape_debug()),
        ListError::SameUrl(name) => format!("an address of {} twice", name.escape_debug()),
        ListError::Duplicate(name) => format!("{} twice", name.escape_debug()),
    }
}

/// Where the patch step's list comes from.
enum Source<'a> {
    /// `GET <server>/patches`.
    Server(&'a Url),
    /// The server could not be asked: the signed torrent's patches
    /// ([`torrent_list`]).
    Torrent(List),
    /// The server could not be reached: the list built in ([`built_in`]).
    BuiltIn(List),
}

/// The list of the release this updater was built for (`SKHID_BUILTIN_PATCHES`,
/// the `/patches` of `release/patches.tsv`, which `release/build-zip` passes
/// to the build), for when the server cannot be reached: a provider that
/// blocks the server need not block the patches' addresses. A list of no
/// patch is none: it would delete every patch file.
fn built_in() -> Option<List> {
    option_env!("SKHID_BUILTIN_PATCHES")
        .and_then(|text| parse_list(text.as_bytes()).ok())
        .filter(|list| !list.patches.is_empty())
}

/// Writes `data` beside, then renames it over `path`.
fn save(path: &str, data: &[u8]) -> Result<(), Code> {
    if win::read_file(path, data.len() + 1).ok().as_deref() == Some(data) {
        return Ok(());
    }
    let staged = format!("{path}.skhid-new");
    let result = win::write_file(&staged, data).and_then(|()| win::rename(&staged, path, true));
    if result.is_err() {
        let _ = win::delete(&staged);
    }
    result
}

/// The server, or its list, could not be fetched: the game may be played
/// when the folder is as the list kept says, after a run that ended
/// `ready`. Else the step is made with the list built in, when there is
/// one; else PLAY stays off with `message`.
fn offline(
    session: &mut Session,
    layout: &Layout,
    overlay: &[&str],
    record: &mut Record,
    look: Look,
    message: &str,
) -> Outcome {
    match kept_in_place(layout, Some(&mut *session), record, look) {
        Ok(()) => {
            log!("patches: the server could not be asked; the patches kept are in place");
            Outcome::Ready
        }
        Err(None) => {
            log!("patches: stopped");
            Outcome::Stopped
        }
        Err(Some(why)) => match torrent_list(session, layout) {
            Ok(Some(list)) => {
                log!("patches: the server could not be asked and {why}: the torrent's patches");
                step(session, layout, Source::Torrent(list), overlay, look, record)
            }
            Err(None) => {
                log!("patches: stopped");
                Outcome::Stopped
            }
            Err(Some(fail)) => {
                log!("patches: BLOCKED: {fail}. The player is told: {message}");
                Outcome::Blocked(message.into())
            }
            // No torrent, or it gave nothing.
            Ok(None) => offline_built_in(session, layout, overlay, record, look, message, &why),
        },
    }
}

/// The list built into this updater, when the server could not be asked
/// and nothing else can be checked against.
fn offline_built_in(
    session: &mut Session,
    layout: &Layout,
    overlay: &[&str],
    record: &mut Record,
    look: Look,
    message: &str,
    why: &str,
) -> Outcome {
    match built_in() {
            Some(list) => {
                log!(
                    "patches: the server could not be asked and {why}: the list built into this \
                     updater is used"
                );
                step(
                    session,
                    layout,
                    Source::BuiltIn(list),
                    overlay,
                    look,
                    record,
                )
            }
            None => {
                log!("patches: BLOCKED: {why}. The player is told: {message}");
                Outcome::Blocked(message.into())
            }
    }
}

/// The signed torrent's patches as a list, each checked in place or fetched
/// by it, its SHA-256 from the checked file. Addresses are the kept list's
/// for the same file, else none until the server's list names it again.
/// `Ok(None)`: not by torrent; `Err(None)`: stopped.
fn torrent_list(session: &mut Session, layout: &Layout) -> Result<Option<List>, Option<String>> {
    let Some(offered) = torrent::offered(session, layout) else {
        return Ok(None);
    };
    let torrent = &offered.torrent;
    let ours: Vec<(u8, usize)> = (0..torrent.files.len())
        .filter_map(|index| {
            let name = torrent.files[index].path.strip_prefix("game/")?;
            Some((updater_logic::patches::file_number(name)?, index))
        })
        .collect();
    if ours.is_empty() {
        return Ok(None);
    }
    let places: Vec<(usize, String)> = ours
        .iter()
        .map(|(number, index)| (*index, place(layout, &format!("{FILE_PREFIX}{number}"))))
        .collect();
    let Some(whole) = torrent::whole(session, &offered, &places) else {
        return if session.stopped() { Err(None) } else { Ok(None) };
    };
    let missing: Vec<(usize, String)> = ours
        .iter()
        .zip(&whole)
        .filter(|(_, whole)| !**whole)
        .map(|(patch, _)| patch)
        .map(|(number, index)| (*index, format!("{}{PART}", place(layout, &format!("{FILE_PREFIX}{number}")))))
        .collect();
    if !missing.is_empty() {
        log!("patches: {} of the torrent's to fetch by it", missing.len());
        match torrent::fetch(session, &offered, &missing) {
            torrent::Fetched::Done => {}
            torrent::Fetched::Stopped => return Err(None),
            torrent::Fetched::Not(why) => {
                for (_, part) in &missing {
                    let _ = win::delete(part);
                }
                log!("patches: the torrent did not give its patches: {why}");
                return Ok(None);
            }
        }
        for (index, part) in &missing {
            // Every piece was checked; only extra bytes could be wrong.
            let size = torrent.files[*index].size;
            if win::file_size(part).ok() != Some(size) {
                let _ = win::delete(part);
                return Err(Some(format!("{part} is not of the torrent's {size} bytes")));
            }
            let target = part.strip_suffix(PART).expect("a part");
            if let Err(code) = File::alone(&place(layout, DB_NAME))
                .map(drop)
                .or_else(|code| if win::is_missing(code) { Ok(()) } else { Err(code) })
                .and_then(|()| win::rename(part, target, true))
            {
                let _ = win::delete(part);
                return Err(Some(format!("{target} cannot be put in place: {}", win::error_text(code))));
            }
        }
    }
    let kept = win::read_file(&beside(layout, LIST_FILE), MAX_LIST)
        .ok()
        .and_then(|bytes| parse_list(&bytes).ok())
        .unwrap_or_default();
    let mut patches = Vec::new();
    for (number, _) in &ours {
        let path = place(layout, &format!("{FILE_PREFIX}{number}"));
        let (_, sha256) = win::hash_file(&path).map_err(|code| Some(format!("{path}: {}", win::error_text(code))))?;
        let addresses = kept
            .get(*number)
            .filter(|patch| patch.sha256 == sha256)
            .map(|patch| patch.addresses.clone())
            .unwrap_or_default();
        patches.push(Patch {
            number: *number,
            sha256,
            addresses,
        });
    }
    patches.sort_by_key(|patch| patch.number);
    log!("patches: the torrent lists {} patch(es)", patches.len());
    Ok(Some(List { patches }))
}

/// The patch step. `server`: `Err(why)` when the server could not be asked
/// this run: the folder is then checked against the list kept from the last
/// run. `overlay`: the files the overlay installed (the ZIP's names).
/// `look`: [`Look::Full`] hashes every file.
pub fn run(
    session: &mut Session,
    layout: &Layout,
    server: Result<&Url, &str>,
    overlay: &[&str],
    look: Look,
) -> Outcome {
    if look == Look::Full {
        log!("patches: every patch file is hashed (check)");
    }
    let mut record = read_record(layout, look);
    let before = record.clone();
    let outcome = match server {
        Ok(server) => step(
            session,
            layout,
            Source::Server(server),
            overlay,
            look,
            &mut record,
        ),
        Err(reason) => offline(
            session,
            layout,
            overlay,
            &mut record,
            look,
            &text::patches_offline(reason),
        ),
    };
    if record != before {
        save_record(layout, &record);
    }
    outcome
}

fn step(
    session: &mut Session,
    layout: &Layout,
    source: Source,
    overlay: &[&str],
    look: Look,
    record: &mut Record,
) -> Outcome {
    let built_in = matches!(source, Source::BuiltIn(_));
    let mut run = Run {
        session,
        layout,
        source,
        look,
        record,
        changing: false,
        torrented: Vec::new(),
    };
    let outcome = run.step(overlay);
    let changing = run.changing;
    let session = run.session;
    let record = run.record;
    let status = |text: &[u8]| {
        if let Err(code) = save(&beside(layout, STATUS_FILE), text) {
            log!(
                "patches: {STATUS_FILE} cannot be written ({})",
                win::error_text(code)
            );
        }
    };
    match outcome {
        // The list built in is not the server's: `ready` and the next start
        // without the server check the folder against the one kept, as
        // before; this run's PLAY stands on the patches it lists in place.
        Ok(()) if built_in => Outcome::Ready,
        Ok(()) => {
            status(b"ready\r\n");
            Outcome::Ready
        }
        Err(Fail::Offline(message, why)) => {
            log!("patches: {why}: the folder is checked against the list kept");
            offline(session, layout, overlay, record, look, &message)
        }
        Err(Fail::Stopped) => {
            log!("patches: stopped; the rest next time");
            if changing {
                status(b"blocked stopped\r\n");
            }
            Outcome::Stopped
        }
        Err(Fail::Stop(stop, name, why)) => {
            log!("patches: BLOCKED: {why}");
            if changing {
                status(format!("blocked {why}\r\n").as_bytes());
            }
            Outcome::Stop(stop, name)
        }
        Err(Fail::Blocked(message, why)) => {
            log!("patches: BLOCKED: {why}. The player is told: {message}");
            if changing {
                status(format!("blocked {why}\r\n").as_bytes());
            } else {
                log!("patches: nothing in the folder was changed: {STATUS_FILE} stays as it was");
            }
            Outcome::Blocked(message)
        }
    }
}
