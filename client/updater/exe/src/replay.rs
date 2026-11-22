//! `updater_skhid.exe replay <id> <token> <sha256> <size>`: "Save replay".
//!
//! The patched game (patch revision 14) starts the updater with the link
//! the server put into the lobby. The updater checks every argument
//! (`updater_logic::replay::parse`), fetches `<site>/replays/<token>`
//! from its configured site with the update's rules (HTTPS, plain HTTP for
//! this computer alone, no redirect, no cookie) into
//! `<game>\replays\srp_<id>.srpreplay.part` (opened by this updater alone: a
//! second save of the same match meanwhile is refused), checks the size and
//! the SHA-256 of what is on the disk, reads the file's header and renames it
//! to `<date>_<time>_<map>_<id>.srpreplay` (a file of this match with the
//! right bytes is kept as it is, without a download). Saved replays are never
//! deleted. A save starts no game and shows no box when it succeeds; a
//! problem is one message box in the game's language. Either way a line goes
//! to the run's log, which names the token by its first 6 hex digits only.
//!
//! `updater_skhid.exe open-replay <path>` ([`run_local`]): a `.sur` the player
//! opened in Explorer, started in the game as it is (`launch`): nothing
//! downloaded, copied or deleted.

use alloc::format;
use alloc::string::String;

use updater_logic::replay::{self, Header, Problem, Request, Stamp};
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

/// The one place the replay shows a window: a problem's message box, unless
/// the run is `quiet`. A saved replay shows none: the game is up, a box would
/// take its focus, and its card already says it is saved. Every other path of
/// this file reports through here or the log only.
fn notify(quiet: bool, text: &str) {
    if !quiet {
        crate::message_box(text);
    }
}

/// Why a save stopped: a problem of the replay's, or the site's address
/// (`crate::server`'s own words).
enum Failure {
    Problem(Problem),
    Server(String),
}

impl From<Problem> for Failure {
    fn from(problem: Problem) -> Self {
        Self::Problem(problem)
    }
}

/// The replay: `arguments` are those after `replay`; `quiet`: no window
/// at all (the outcome goes to the log and the exit code).
pub fn run(arguments: &[String], quiet: bool) -> u32 {
    let layout = match Layout::find() {
        Ok(layout) => layout,
        Err(path) => {
            notify(quiet, &crate::text::wrong_folder(&path));
            return FAILED;
        }
    };
    crate::log::start(&layout);
    log!(
        "---- updater {} replay started in {}{}",
        env!("CARGO_PKG_VERSION"),
        layout.exe_dir,
        if quiet { " (quiet)" } else { "" }
    );
    let language = win::documents().as_deref().and_then(win::game_language);
    // The lock is let go when `save` returns: a box left open does not hold
    // the next save.
    match save(&layout, arguments) {
        Ok(path) => {
            log!("replay: saved {path}; exit 0");
            OK
        }
        Err(Failure::Problem(problem)) => tell(quiet, language, &problem),
        Err(Failure::Server(message)) => {
            log!("replay: FAILED (server address): {message}");
            notify(quiet, &message);
            FAILED
        }
    }
}

/// The replay of the link saved in the replays folder: its path.
fn save(layout: &Layout, arguments: &[String]) -> Result<String, Failure> {
    let request = replay::parse(arguments).map_err(|error| {
        log!("replay: arguments refused ({error:?})");
        Problem::BadLink
    })?;
    log!(
        "replay: match {} token {}... {} bytes",
        request.id,
        request.token_hint(),
        request.size
    );
    let server = crate::server(layout).map_err(Failure::Server)?;
    let folder = format!("{}\\{}", layout.game_dir, replay::FOLDER);
    win::create_folders(&folder).map_err(disk)?;
    if let Some(path) = saved_before(&folder, &request) {
        log!("replay: {path} is saved already, checked");
        return Ok(path);
    }
    let part = format!("{folder}\\{}", request.part_name());
    fetch(&server, &request, &part)?;
    name(&folder, &request, &part)
        .inspect_err(|_| {
            let _ = win::delete(&part);
        })
        .map_err(Failure::from)
}

fn tell(quiet: bool, language: Option<u32>, problem: &Problem) -> u32 {
    let text = replay::message(language, problem);
    log!("replay: FAILED ({problem:?}): {text}");
    notify(quiet, &text);
    FAILED
}

/// Explorer supplies an absolute local file. No download, pruning or network
/// request: this file belongs to the user, not the managed replay cache.
pub fn run_local(arguments: &[String], quiet: bool) -> u32 {
    let layout = match Layout::find() {
        Ok(layout) => layout,
        Err(path) => {
            notify(quiet, &crate::text::wrong_folder(&path));
            return FAILED;
        }
    };
    crate::log::start(&layout);
    log!("---- updater local replay started{}", if quiet { " (quiet)" } else { "" });
    let language = win::documents().as_deref().and_then(win::game_language);
    let Some(file) = updater_logic::local_replay::LocalReplay::parse(arguments) else {
        return tell(quiet, language, &Problem::BadFile);
    };
    if let Err(code) = win::file_size(file.path()) {
        return tell(quiet, language, &Problem::Open {
            path: file.path().into(), detail: win::error_text(code),
        });
    }
    launch(&layout, file.path(), quiet, language)
}

/// `open-replay`'s: the installed-file gate, path conversion, localization,
/// console arguments and working directory.
fn launch(layout: &Layout, target: &str, quiet: bool, language: Option<u32>) -> u32 {
    let game = format!("{}\\{}", layout.exe_dir, replay::GAME_EXE);
    if !win::exists(&game) {
        return tell(quiet, language, &Problem::NoGame);
    }
    // The check play_skhid.bat and play_skhid.sh make (`updater_skhid.exe
    // ready`, read only): while the patches of resources.db are not checked
    // in place (the last patch step did not end `ready`, or a patch file is
    // missing, damaged or extra), the game is not started; it would stop at
    // start or play the replay with other files. Made last, so it says how things are
    // when the game would start; the replay stays, checked, for the next ask.
    if crate::patches::ready(layout).is_err() {
        log!("replay: the game's files are not in place (as `ready` finds): the game is not started");
        return tell(quiet, language, &Problem::NotReady);
    }

    let Some(path) = win::game_path(target, replay::MAX_GAME_PATH) else {
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

fn disk(code: win::Code) -> Problem {
    Problem::Disk(win::error_text(code))
}

/// A file of this match in the folder with the link's size and checksum.
fn saved_before(folder: &str, request: &Request) -> Option<String> {
    win::list(folder, "*.srpreplay")
        .into_iter()
        .filter(|name| request.is_saved_name(name))
        .map(|name| format!("{folder}\\{name}"))
        .find(|path| is_this_replay(path, request))
}

fn is_this_replay(path: &str, request: &Request) -> bool {
    win::file_size(path).is_ok_and(|size| size == request.size)
        && win::hash_file(path).is_ok_and(|(size, hash)| size == request.size && hash == request.sha256)
}

/// The replay downloaded into `part`, its size and checksum read back.
fn fetch(server: &Url, request: &Request, part: &str) -> Result<(), Problem> {
    let mut last = Problem::Network(server.host.clone());
    for attempt in 1..=TRIES {
        if let Err(problem) = download(server, request, part) {
            let _ = win::delete(part);
            let again = matches!(problem, Problem::Network(_));
            last = problem;
            if again && attempt < TRIES {
                log!("replay: download {attempt} of {TRIES} failed: {last:?}");
                win::sleep(PAUSE_MS);
                continue;
            }
            return Err(last);
        }
        // What is on the disk, read back.
        return match win::hash_file(part) {
            Ok((size, hash)) if size == request.size && hash == request.sha256 => {
                log!("replay: downloaded {} bytes, checksum right", request.size);
                Ok(())
            }
            Ok((size, _)) => {
                log!("replay: downloaded {size} bytes, the size or the checksum is wrong");
                let _ = win::delete(part);
                Err(Problem::Damaged)
            }
            Err(code) => {
                let _ = win::delete(part);
                Err(disk(code))
            }
        };
    }
    Err(last)
}

fn download(server: &Url, request: &Request, part: &str) -> Result<(), Problem> {
    // The .part is opened by one updater alone: another save of this match holds it.
    let file = win::DownloadFile::create(part).map_err(|code| match code {
        windows_sys::Win32::Foundation::ERROR_SHARING_VIOLATION => Problem::AlreadyRunning,
        code => disk(code),
    })?;
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
        return Err(disk(code));
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
    file.flush().map_err(disk)
}

/// The checked download named by its header: the path it has now.
fn name(folder: &str, request: &Request, part: &str) -> Result<String, Problem> {
    let header = read_header(part)?;
    if header.match_id != request.id {
        log!("replay: the file is of match {}, not {}", header.match_id, request.id);
        return Err(Problem::Damaged);
    }
    let start = win::local_stamp_of(header.made).unwrap_or(Stamp { year: 1970, month: 1, day: 1, hour: 0, minute: 0 });
    let path = format!("{folder}\\{}", replay::saved_name(&header, start));
    win::rename(part, &path, false).map_err(disk)?;
    Ok(path)
}

/// The header of the file at `path`.
fn read_header(path: &str) -> Result<Header, Problem> {
    let file = win::File::shared(path).map_err(disk)?;
    let mut prefix = [0_u8; replay::PREFIX];
    read_exact(&file, &mut prefix)?;
    let length = replay::header_length(&prefix).map_err(damaged)?;
    let mut text = alloc::vec![0_u8; length];
    read_exact(&file, &mut text)?;
    replay::parse_header(&text).map_err(damaged)
}

fn damaged(error: replay::HeaderError) -> Problem {
    log!("replay: the file's header is refused ({error:?})");
    Problem::Damaged
}

fn read_exact(file: &win::File, buffer: &mut [u8]) -> Result<(), Problem> {
    let mut filled = 0;
    while filled < buffer.len() {
        match file.read(&mut buffer[filled..]).map_err(disk)? {
            0 => {
                log!("replay: the file ends inside its header");
                return Err(Problem::Damaged);
            }
            read => filled += read,
        }
    }
    Ok(())
}
