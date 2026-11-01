//! `updater_skhid.exe replay <id> <token> <sha256> <size>`: "Watch replay".
//!
//! The patched game (patch revision 14) starts the updater with the link
//! the server put into the lobby. The updater checks every argument
//! (`updater_logic::replay::parse`), takes its own lock (an update and a
//! replay download never wait for each other), fetches `<site>/replays/<token>`
//! from its configured site with the update's rules (HTTPS, plain HTTP for
//! this computer alone, no redirect, no cookie) into
//! `<game>\replays\srp_<id>.sur.part`, checks the size and the SHA-256 of
//! what is on the disk, renames it to `srp_<id>.sur` (a file of that name
//! with the right bytes is used again without a download), keeps the ten
//! newest replays, checks that the game's files are in place as `ready` does
//! (`patches::ready`, the check of play_skhid.bat), and starts the game as
//! the launcher's PLAY does, with `-match_replay=<the file>`. A problem is a
//! message box in the launcher's
//! language and a line in `updater_skhid.log`; the log names the token by
//! its first 6 hex digits only.

use alloc::format;
use alloc::string::String;

use updater_logic::replay::{self, Problem, Request};
use updater_logic::url::Url;

use crate::http::{self, HttpError};
use crate::install::Layout;
use crate::log::log;
use crate::win;

/// How often a download that broke off is tried, and the pause between.
const TRIES: u32 = 3;
const PAUSE_MS: u32 = 3000;

/// The process exit codes (as the update's).
const OK: u32 = 0;
const FAILED: u32 = 1;

/// The one place the replay shows a window: a message box, unless the run
/// is `quiet`. Every other path of this file reports through here or the
/// log only.
fn notify(quiet: bool, text: &str) {
    if !quiet {
        crate::message_box(text);
    }
}

fn tell(quiet: bool, language: Option<u32>, problem: &Problem) -> u32 {
    let text = replay::message(language, problem);
    log!("replay: FAILED ({problem:?}): {text}");
    notify(quiet, &text);
    FAILED
}

/// The replay: `arguments` are those after `replay`; `quiet`: no window
/// at all (the problem goes to the log and the exit code).
pub fn run(arguments: &[String], quiet: bool) -> u32 {
    let layout = match Layout::find() {
        Ok(layout) => layout,
        Err(path) => {
            notify(quiet, &crate::text::wrong_folder(&path));
            return FAILED;
        }
    };
    crate::log::start(format!("{}\\{}", layout.exe_dir, crate::LOG_FILE));
    log!(
        "---- updater {} replay started in {}{}",
        env!("CARGO_PKG_VERSION"),
        layout.exe_dir,
        if quiet { " (quiet)" } else { "" }
    );
    let language = win::game_language();
    let request = match replay::parse(arguments) {
        Ok(request) => request,
        Err(error) => {
            log!("replay: arguments refused ({error:?})");
            return tell(quiet, language, &Problem::BadLink);
        }
    };
    log!(
        "replay: match {} token {}... {} bytes",
        request.id,
        request.token_hint(),
        request.size
    );
    let _lock = match win::lock(&layout.exe_dir, replay::LOCK_FILE) {
        Ok(lock) => lock,
        Err(code) => {
            log!("replay: {} is held ({})", replay::LOCK_FILE, win::error_text(code));
            return tell(quiet, language, &Problem::AlreadyRunning);
        }
    };
    let game = format!("{}\\{}", layout.exe_dir, replay::GAME_EXE);
    if !win::exists(&game) {
        return tell(quiet, language, &Problem::NoGame);
    }
    let server = match crate::server(&layout) {
        Ok(server) => server,
        Err(message) => {
            log!("replay: FAILED (server address): {message}");
            notify(quiet, &message);
            return FAILED;
        }
    };
    let folder = format!("{}\\{}", layout.game_dir, replay::FOLDER);
    if let Err(code) = win::create_folders(&folder) {
        return tell(quiet, language, &Problem::Disk(win::error_text(code)));
    }
    let target = format!("{folder}\\{}", request.file_name());
    if let Err(problem) = fetch(&server, &request, &folder, &target) {
        return tell(quiet, language, &problem);
    }
    prune(&folder, &request.file_name());

    // The check play_skhid.bat and play_skhid.sh make (`updater_skhid.exe
    // ready`, read only): while the patches of resources.db are not checked
    // in place (the last patch step did not end `ready`, or a patch file is
    // missing, damaged or extra), the game is not started; it would stop at
    // start or play the replay with other files. Made last, so it says how things are
    // when the game would start; the replay stays, checked, for the next ask.
    if crate::patches::ready(&layout).is_err() {
        log!("replay: the game's files are not in place (as `ready` finds): the game is not started");
        return tell(quiet, language, &Problem::NotReady);
    }

    let Some(path) = win::game_path(&target, replay::MAX_GAME_PATH) else {
        log!("replay: {target} cannot be given to the game (ANSI command line)");
        return tell(quiet, language, &Problem::Path);
    };
    let Some(command) = replay::game_command(&game, &path) else {
        log!("replay: no command line for {path}");
        return tell(quiet, language, &Problem::Path);
    };
    log!("replay: starting {command}");
    match win::start(&game, &command, &layout.exe_dir) {
        Ok(()) => {
            log!("replay: started; exit 0");
            OK
        }
        Err(code) => tell(quiet, language, &Problem::Start(win::error_text(code))),
    }
}

/// The replay in `target`, checked: the one there, or a download.
fn fetch(server: &Url, request: &Request, folder: &str, target: &str) -> Result<(), Problem> {
    match win::hash_file(target) {
        Ok((size, hash)) if size == request.size && hash == request.sha256 => {
            log!("replay: {} is here already, checked", request.file_name());
            return Ok(());
        }
        Ok(_) => log!("replay: {} is here but not this replay: downloaded again", request.file_name()),
        Err(_) => {}
    }
    let part = format!("{folder}\\{}", request.part_name());
    let mut last = Problem::Network(server.host.clone());
    for attempt in 1..=TRIES {
        let result = download(server, request, &part);
        match result {
            Ok(()) => {}
            Err(problem) => {
                let _ = win::delete(&part);
                let again = matches!(problem, Problem::Network(_));
                last = problem;
                if again && attempt < TRIES {
                    log!("replay: download {attempt} of {TRIES} failed: {last:?}");
                    win::sleep(PAUSE_MS);
                    continue;
                }
                return Err(last);
            }
        }
        // What is on the disk, read back.
        match win::hash_file(&part) {
            Ok((size, hash)) if size == request.size && hash == request.sha256 => {}
            Ok((size, _)) => {
                log!("replay: downloaded {size} bytes, the size or the checksum is wrong");
                let _ = win::delete(&part);
                return Err(Problem::Damaged);
            }
            Err(code) => {
                let _ = win::delete(&part);
                return Err(Problem::Disk(win::error_text(code)));
            }
        }
        if let Err(code) = win::rename(&part, target, true) {
            let _ = win::delete(&part);
            return Err(Problem::Disk(win::error_text(code)));
        }
        log!("replay: downloaded {} bytes, checksum right: {}", request.size, request.file_name());
        return Ok(());
    }
    Err(last)
}

fn download(server: &Url, request: &Request, part: &str) -> Result<(), Problem> {
    let file = win::DownloadFile::create(part).map_err(|code| Problem::Disk(win::error_text(code)))?;
    let mut written_error = None;
    let result = http::get_stream(
        server,
        &request.route(),
        request.size,
        &mut |chunk| {
            file.write(chunk).map_err(|code| {
                written_error = Some(code);
                HttpError::Network("write", code)
            })
        },
        &mut |_, _| true,
    );
    if let Some(code) = written_error {
        return Err(Problem::Disk(win::error_text(code)));
    }
    let host = server.host.clone();
    match result {
        Ok(()) => {}
        Err(HttpError::Status(404 | 410)) => return Err(Problem::Expired),
        Err(HttpError::Status(429 | 503)) => return Err(Problem::Busy(host)),
        Err(HttpError::Status(status)) => return Err(Problem::Status(host, status)),
        // More than the link says: not this replay.
        Err(HttpError::TooLarge) => return Err(Problem::Damaged),
        // get_stream follows no redirect, reads no type and asks for no
        // part: Html, Redirects, BadRedirect and BadRange are the files of
        // a list's (get_file) and never come here.
        Err(
            error @ (HttpError::Network(..)
            | HttpError::Cut { .. }
            | HttpError::Stopped
            | HttpError::Slow(_)
            | HttpError::Html(..)
            | HttpError::Redirects
            | HttpError::BadRedirect(_)
            | HttpError::BadRange),
        ) => {
            log!("replay: {error:?}");
            return Err(Problem::Network(host));
        }
    }
    file.flush().map_err(|code| Problem::Disk(win::error_text(code)))
}

/// Keeps the ten newest replays (the one just fetched among them) and
/// deletes the rest, and downloads left over; names that are not the
/// updater's own (`srp_<8 hex>.sur`) are never touched.
fn prune(folder: &str, current: &str) {
    let files = win::list_with_times(folder, "srp_*");
    for name in replay::to_delete(&files, current, replay::KEEP) {
        match win::delete(&format!("{folder}\\{name}")) {
            Ok(()) => log!("replay: removed {name} (more than {} replays)", replay::KEEP),
            Err(code) => log!("replay: {name} could not be removed ({})", win::error_text(code)),
        }
    }
}
