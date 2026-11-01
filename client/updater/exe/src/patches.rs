//! The patch step: after the overlay and the restore of resources.db, at
//! every run, the game folder's `resources.db.patch_<n>` files are made the
//! server's list (`GET <site>/patches`, `updater_logic::patches`): every
//! listed patch's file is checked (the launcher shows "checking" with its
//! bar), a patch file the list has not is deleted, and a listed patch that
//! is missing or damaged is downloaded from the first of its addresses that
//! gives it (plain HTTP or HTTPS, any host, redirects followed: the list's
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
//! tries again. A download goes to `<file>.skhid-part`, is flushed, read
//! back and checked, and only then renamed to its name: a patch file is
//! never half written in place. Before a patch file is deleted or renamed,
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
use updater_logic::file_error::Stop;
use updater_logic::patches::{
    Address, FILE_PREFIX, List, ListError, Local, MAX_LIST, Patch, Problem, check, overlay_patch,
    parse_list, todo, write_list,
};
use updater_logic::pipe::{ToLauncher, state, step};
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
use crate::win::{self, Code, File};

/// Beside the updater: the server's last list, as it was read.
const LIST_FILE: &str = "updater_skhid.patches";
/// `ready` once the folder was checked against that list, else `blocked <why>`.
const STATUS_FILE: &str = "updater_skhid.patches.status";
/// What each patch file was when it was last hashed (`updater_logic::checked`).
const CHECKED_FILE: &str = "updater_skhid.patches.checked";
/// The game's own db, in the game folder: the game holds it while it runs.
const DB_NAME: &str = "resources.db";
/// A patch while it is downloaded, beside its place. It is a
/// `resources.db.patch_*` the game does not take, so one left over is
/// deleted as any other.
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
            Err(_) if session.as_deref().is_some_and(|session| session.stopped) => return None,
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
        /// It sent a file with another SHA-256.
        wrong: bool,
        /// A second round may do better (the network, a cut, a 5xx).
        again: bool,
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
    let name = path.rsplit('\\').next().unwrap_or(path);
    if Stop::of(code) == Some(Stop::Virus) {
        // The patch's name, not its download's.
        let name = name.strip_suffix(PART).unwrap_or(name);
        Fail::Stop(Stop::Virus, name.into(), why)
    } else if code == ERROR_SHARING_VIOLATION || code == ERROR_LOCK_VIOLATION {
        Fail::Blocked(text::patches_held(name), why)
    } else {
        Fail::Blocked(text::patches_failed(), why)
    }
}

/// What the sink calls a write of the download that failed: a failure
/// here, not of the address.
const WRITE_STEP: &str = "write the patch";

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
}

impl Run<'_> {
    /// The server's list, asked for a few times; or the one built in.
    fn list(&mut self) -> Result<List, Fail> {
        let server = match &self.source {
            Source::Server(server) => *server,
            Source::BuiltIn(list) => return Ok(list.clone()),
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
    /// that gives it: `true` when the file in place is known by its file ID
    /// to be the one hashed (else it is hashed there again).
    fn fetch(&mut self, patch: &Patch) -> Result<bool, Fail> {
        let name = patch.file_name();
        let target = place(self.layout, &name);
        let part = format!("{target}{PART}");
        let count = patch.addresses.len();
        // Which addresses a second round tries, and the first failure's
        // words.
        let mut again = alloc::vec![true; count];
        let mut first: Option<(String, bool, &Address)> = None;
        for round in 1..=FETCH_TRIES {
            if round > 1 {
                if !again.contains(&true) {
                    break;
                }
                if !self.session.pause(RETRY_PAUSE_MS) {
                    return Err(Fail::Stopped);
                }
            }
            for (at, address) in patch.addresses.iter().enumerate() {
                if !again[at] {
                    continue;
                }
                if !self.session.pump() {
                    return Err(Fail::Stopped);
                }
                self.session.state(state::DOWNLOADING);
                self.session.begin_progress();
                log!(
                    "patches: {name}: address {} of {count}, {} (try {round} of {FETCH_TRIES}): \
                     asking",
                    at + 1,
                    address.text
                );
                // Too slow is a failure only while another address will be
                // tried this round (the second tries again only those that
                // failed on the network).
                let slow = if again[at + 1..].contains(&true) {
                    http::Slow::Leave
                } else {
                    http::Slow::Wait
                };
                let result = self.download(&part, patch, address, slow);
                let identity = match result {
                    Ok(identity) => identity,
                    Err(miss) => {
                        let _ = win::delete(&part);
                        match miss {
                            Miss::Stopped => return Err(Fail::Stopped),
                            Miss::Here(fail) => return Err(fail),
                            Miss::There {
                                reason,
                                why,
                                wrong,
                                again: worth,
                            } => {
                                log!(
                                    "patches: {name}: address {} of {count}, {} (try {round} of \
                                     {FETCH_TRIES}): {why}",
                                    at + 1,
                                    address.text
                                );
                                again[at] = worth && round < FETCH_TRIES;
                                if first.is_none() {
                                    first = Some((reason, wrong, address));
                                }
                                continue;
                            }
                        }
                    }
                };
                // Checked: into its place, unless the game runs now.
                if let Err(fail) = self.guard() {
                    let _ = win::delete(&part);
                    return Err(fail);
                }
                if let Err(code) =
                    retried("renamed", &part, &mut || win::rename(&part, &target, true))
                {
                    let _ = win::delete(&part);
                    return Err(file_fail(&target, code, "replaced"));
                }
                self.session.state(state::INSTALLING);
                self.session.send(ToLauncher::InstallInfo {
                    step: step::EXTRACT,
                    text: &name,
                });
                self.session.named_file = true;
                log!(
                    "patches: {name}: downloaded {} (address {} of {count}), checksum right ({})",
                    address.text,
                    at + 1,
                    hex(&patch.sha256)
                );
                // The file hashed is the one in place when the rename kept
                // its ID.
                let kept = identity
                    .filter(|identity| win::identity(&target).ok() == Some(*identity))
                    .inspect(|identity| {
                        self.record
                            .hashed(patch.number, patch.sha256, *identity, win::now());
                    });
                return Ok(kept.is_some());
            }
        }
        let (reason, wrong, address) = first.expect("an address failed");
        let message = if count > 1 {
            text::patches_no_address(&name, count, &reason)
        } else if wrong {
            text::patches_wrong(&address.url, &name)
        } else {
            text::patches_download(&name, &reason)
        };
        Err(Fail::Blocked(
            message,
            format!("{name}: none of its {count} address(es) gave it"),
        ))
    }

    /// One try of one address: its file into `part`, flushed, read back and
    /// checked. What Windows says of `part` then (to know it in its place).
    fn download(
        &mut self,
        part: &str,
        patch: &Patch,
        address: &Address,
        slow: http::Slow,
    ) -> Result<Option<Identity>, Miss> {
        let output = retried("written", part, &mut || File::create(part))
            .map_err(|code| Miss::Here(file_fail(part, code, "written")))?;
        let folder = self.layout.game_dir.clone();
        let name = patch.file_name();
        let mut no_room: Option<(u64, u64)> = None;
        let mut announced: Option<u64> = None;
        let mut asked = false;
        let mut written = 0_u64;
        let session = &mut *self.session;
        let result = http::get_file(
            &address.url,
            0,
            MAX_PATCH,
            slow,
            &mut |to| log!("patches: {name}: {} redirects to {to}", address.text),
            &mut |_, chunk| {
                written += chunk.len() as u64;
                output
                    .write(chunk)
                    .map_err(|code| HttpError::Network(WRITE_STEP, code))
            },
            &mut |done, total| {
                // Before the first byte: is there room for what the server
                // says it sends?
                if !asked {
                    asked = true;
                    announced = total;
                    let needed = total.unwrap_or(0).saturating_add(DISK_MARGIN);
                    if let Some(free) = win::free_space(&folder)
                        && free < needed
                    {
                        no_room = Some((needed, free));
                        return false;
                    }
                }
                session.progress(done, total.unwrap_or(0))
            },
        );
        if let Some((needed, free)) = no_room {
            drop(output);
            return Err(Miss::Here(Fail::Blocked(
                text::disk_full(&folder, needed, free),
                format!("disk: {free} bytes free in {folder}, {needed} needed"),
            )));
        }
        match result {
            Ok(()) => {}
            Err(HttpError::Stopped) => return Err(Miss::Stopped),
            Err(HttpError::Network(WRITE_STEP, code))
                if code == ERROR_DISK_FULL || code == ERROR_HANDLE_DISK_FULL =>
            {
                // What the patch needs, measured without what was written
                // of it.
                drop(output);
                let _ = win::delete(part);
                let free = win::free_space(&folder).unwrap_or(0);
                let needed = announced.unwrap_or(written).saturating_add(DISK_MARGIN);
                return Err(Miss::Here(Fail::Blocked(
                    text::disk_full(&folder, needed, free),
                    format!(
                        "disk: the disk is full ({free} bytes free in {folder}, {needed} needed)"
                    ),
                )));
            }
            Err(HttpError::Network(WRITE_STEP, code)) => {
                return Err(Miss::Here(file_fail(part, code, "written")));
            }
            Err(error) => {
                let message = text::http(&address.url, &address.url.prefix, &error);
                let again = matches!(
                    error,
                    HttpError::Network(..) | HttpError::Cut { .. } | HttpError::Status(500..=599)
                );
                return Err(Miss::There {
                    why: message.clone(),
                    reason: message,
                    wrong: false,
                    again,
                });
            }
        }
        output
            .flush()
            .map_err(|code| Miss::Here(file_fail(part, code, "written")))?;
        drop(output);
        match win::hash_file_identity(part, &mut |_| true) {
            Ok((_, sha256, identity)) if sha256 == patch.sha256 => Ok(identity),
            Ok((size, sha256, _)) => Err(Miss::There {
                reason: format!(
                    "{} sent a file with another checksum than the list says.",
                    address.url.host
                ),
                why: format!(
                    "{} ({size} bytes) has SHA-256 {}, the list says {}",
                    address.text,
                    hex(&sha256),
                    hex(&patch.sha256)
                ),
                wrong: true,
                again: false,
            }),
            Err(code) => Err(Miss::Here(file_fail(part, code, "read back"))),
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
        let mut todo = todo(&list, &found, &mut |patch| lookup(&hashed, patch));
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
        // Downloaded: whether known in place by the file ID.
        let mut downloaded: Vec<(u8, bool)> = Vec::new();
        for fetch in &todo.fetch {
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
            let known = self.fetch(&fetch.patch)?;
            downloaded.push((fetch.patch.number, known));
        }
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
        Err(Some(why)) => match built_in() {
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
        },
    }
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
