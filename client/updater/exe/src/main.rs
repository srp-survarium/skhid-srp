//! The Skhid updater: the program the patched 0.26g launcher starts in the
//! place of the game's own `survarium_updater.exe`
//! (`srp-026g/client-patches/launcher/README.md`).
//!
//! It asks the server which archive of client files it offers now
//! (`GET /api/client-files`: a SHA-256, a size, the least game revision it
//! lets in), compares that with what
//! it installed last (`updater_skhid.state` beside it) and with the files
//! themselves, and when they differ downloads the archive (`GET /download`),
//! checks it and puts its files in their places, itself and the launcher
//! included; then it brings the game's patches to the server's list
//! (`patches.rs`), says PLAY only for a game exe the server lets in, and
//! watches the server while the launcher stays open (`gate.rs`). It tells
//! the launcher what it is doing over the launcher's own pipe, in the
//! messages of the original updater, so the launcher's window shows it as
//! it always did.
//!
//! It asks its configured server for Skhid updates. Missing original game
//! files come from a separate HTTPS mirror with compiled-in SHA-256 pins. When
//! the launcher allows torrent, the release's signed newest revision is asked
//! of the DHT beside the site ([`check`]); a run the site does not answer
//! updates from the release's torrent, which is shared while the launcher is
//! open (`torrent.rs`). No crash upload, no administrator
//! rights. Successful launcher updates
//! register Skhid Replay for .sur files in this user's Open With choices.
#![windows_subsystem = "windows"]

extern crate alloc;

mod game;
mod play;
mod gate;
mod http;
mod install;
mod libtorrent;
mod log;
mod patches;
mod pipe;
mod replay;
mod replay_association;
mod resources;
mod route;
mod seed;
mod signed;
mod text;
mod torrent;
mod win;

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use std::sync::mpsc;

use updater_logic::disk;
use updater_logic::file_error::{self, Exclusion, Kind, Stop};
use updater_logic::manifest::{
    Found, Installed, InstalledFile, MAX_ARCHIVE_SIZE, Manifest, ManifestError, Reason, Verdict,
    damage, parse_installed, parse_manifest, verdict, write_installed,
};
use updater_logic::pipe::{FromLauncher, ToLauncher, Torrenting, state, step};
use updater_logic::diagnosis::{Diagnosis, next_try};
use updater_logic::problem::{Failed, Fault, Miss, Problem, Removal, Where};
use updater_logic::route::{self as routing, Event, Probed, Shown};
use updater_logic::sha256::{digest, to_hex};
use updater_logic::url::{self, Url};
use updater_logic::zip;
use windows_sys::Win32::System::Environment::GetCommandLineW;
use windows_sys::Win32::System::Threading::{ExitProcess, GetCurrentThreadId};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    FindWindowW, GetWindowThreadProcessId, IDRETRY, MB_ICONERROR, MB_ICONWARNING, MB_OK, MB_RETRYCANCEL,
    MB_SETFOREGROUND, MB_TOPMOST, MESSAGEBOX_RESULT, MESSAGEBOX_STYLE, MessageBoxW, PostMessageW,
    SetDlgItemTextW, WM_COMMAND,
};

use crate::http::HttpError;
use crate::install::{InstallError, Layout, NewFile, Step};
use crate::log::log;
use crate::pipe::Pipe;

/// The server asked when no `updater_skhid.url` is beside the program:
/// `SKHID_UPDATE_URL` of the build, else the Skhid server.
const DEFAULT_URL: &str = match option_env!("SKHID_UPDATE_URL") {
    Some(url) => url,
    None => "https://skhid.link",
};
const URL_FILE: &str = "updater_skhid.url";
const STATE_FILE: &str = "updater_skhid.state";
/// What the launcher's pipe is named before the product's name
/// (`\\.\pipe\vostok_updater_pipe-Survarium`).
const PIPE_PREFIX: &str = "\\\\.\\pipe\\vostok_updater_pipe-";

/// How often the server is asked before the updater gives up, and the pause
/// between two tries.
const CHECK_TRIES: u32 = 3;
const DOWNLOAD_TRIES: u32 = 4;
const RETRY_PAUSE_MS: u32 = 3000;
/// The pause when the server says it is busy (its download slots are taken).
const BUSY_PAUSE_MS: u32 = 15_000;
/// How often a file another program holds is read before the update stops
/// on it, and the pause between: about a second in all (an antivirus that
/// scans it lets go by then).
const IN_USE_TRIES: u32 = 6;
const IN_USE_PAUSE_MS: u32 = 200;

/// The process exit codes.
mod exit {
    pub const OK: u32 = 0;
    /// The update could not be done; the reason was shown and logged.
    pub const FAILED: u32 = 1;
    /// The launcher's pipe could not be opened.
    pub const NO_PIPE: u32 = 2;
    /// The program stopped itself on an internal error.
    pub const PANIC: u32 = 3;
}

/// What a panic does (`main` makes it the panic hook): the log says it, and
/// the program ends with its own code, not the abort's.
fn fatal_panic(info: &std::panic::PanicHookInfo) {
    log!("internal error: {}", info.payload_as_str().unwrap_or(""));
    unsafe { ExitProcess(exit::PANIC) }
}

/// Who is told what happens: the launcher over its pipe, or (started by
/// hand with `check`) a message box for an error.
struct Session {
    pipe: Option<Pipe>,
    /// `check quiet`: no message box either; the log and the exit code say
    /// it all.
    quiet: bool,
    run: Run,
    last_progress: u64,
    started: u64,
    /// The launcher's line names a file ("Extracting file <name>") that the
    /// next state message must clear.
    named_file: bool,
    /// What the server said that this run acted on, for [`gate::watch`].
    seen: Option<gate::Seen>,
    /// The last thing of the run whose every address failed: the update
    /// server not reached at its last request, or a file none of whose
    /// addresses gave it. The run's failure, if it ends in one, is said in
    /// its box (`updater_logic::problem`), in the launcher's language.
    trouble: Option<Failed>,
    /// The launcher's torrent settings, and the torrent engine (`torrent.rs`).
    torrents: torrent::Torrents,
    /// The launcher's "Relay" row, in a download with a relay.
    route: Option<RouteBox>,
}

/// What the "Relay" row shows (launcher patch 16), and the probe that
/// runs for it in Automatic.
struct RouteBox {
    config: &'static relay_client::Config,
    shown: Shown,
    probing: Option<mpsc::Receiver<Probed>>,
}

/// Whether the update pass goes on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Run {
    Going,
    /// "Allow torrent" changed: the pass stops and runs again from the
    /// start, as the original updater's `restart_downloading` did.
    Again,
    /// The launcher asked to stop, or is gone, or this updater left it.
    Stopped,
}

impl Session {
    /// The pass is to stop: told to, or to run again.
    fn stopped(&self) -> bool {
        self.run != Run::Going
    }

    /// Whether the pass is to run again; it goes on from here.
    fn again(&mut self) -> bool {
        let again = self.run == Run::Again;
        if again {
            self.run = Run::Going;
        }
        again
    }

    /// Notes how the update server answered a request: not reached
    /// (`updater_logic::network`), no secure connection (a wrong clock), or
    /// otherwise.
    fn reached(&mut self, server: &Url, result: Result<(), &HttpError>) {
        self.trouble = match result.map_err(HttpError::miss) {
            Err(Miss::Unreached(failure)) => Some(Failed::Unreached {
                what: Where::Server(server.host.clone()),
                failure,
            }),
            Err(miss @ Miss::Answered(Fault::Insecure)) => Failed::of_file(&server.host, &[miss]),
            // The site's proxy: the game server behind it is down.
            Err(Miss::Answered(Fault::Status(502..=504))) => Some(Failed::Behind {
                what: Where::Server(server.host.clone()),
            }),
            _ => None,
        };
    }

    /// Every address of `file` failed, each as `misses` says.
    fn failed(&mut self, file: &str, misses: &[Miss]) {
        self.trouble = Failed::of_file(file, misses);
    }

    fn send(&mut self, message: ToLauncher) {
        if let Some(pipe) = &mut self.pipe {
            pipe.send(message);
        }
    }

    /// The row's transition: shown, saved, probed.
    fn route(&mut self, event: Option<Event>) {
        let Some(route) = &mut self.route else { return };
        let step = match event {
            None => Shown::start(route::choice()),
            Some(event) => route.shown.on(event),
        };
        let routing::Step { shown, save, probe } = step;
        if shown != route.shown || event.is_none() {
            log!("route: the row shows {shown:?}");
        }
        route.shown = shown;
        if let Some(choice) = save {
            route::set(choice);
        }
        if probe {
            route.probing = Some(route::probe(route.config));
        }
        self.send(ToLauncher::Route(shown));
    }

    /// The direct route is blocked while the relay answers: the row turns
    /// to auto by itself (saved also with no launcher to show it).
    fn blocked(&mut self) {
        if self.route.is_some() {
            self.route(Some(Event::Blocked));
        } else {
            route::blocked();
        }
    }

    fn state(&mut self, state: u8) {
        log!("state {state:#04x}");
        self.send(ToLauncher::State(state));
    }

    /// Reads what the launcher wrote. `false` once it asked to stop or
    /// closed the pipe.
    fn pump(&mut self) -> bool {
        let mut chosen = None;
        if let Some(pipe) = &mut self.pipe {
            for message in pipe.receive() {
                match message {
                    FromLauncher::Abort => {
                        log!("the launcher asks to stop");
                        self.run = Run::Stopped;
                    }
                    message @ (FromLauncher::AllowTorrent(_)
                    | FromLauncher::Port(_)
                    | FromLauncher::UploadLimit(_)) => {
                        let allowed = self.torrents.allowed();
                        if let Some(answer) = self.torrents.on_message(message) {
                            pipe.send(answer);
                        }
                        if self.torrents.allowed() != allowed && self.run == Run::Going {
                            log!("\"Allow torrent\" changed: the update runs again");
                            self.run = Run::Again;
                        }
                    }
                    FromLauncher::RouteChosen(choice) => chosen = Some(choice),
                    other => log!("launcher message ignored: {other:?}"),
                }
            }
            if !pipe.is_open() && self.run != Run::Stopped {
                log!("the launcher closed the pipe");
                self.run = Run::Stopped;
            }
        }
        // The menu's pick (the last, of several): saved; the update runs
        // again by it (its HTTPS goes the new way).
        if let Some(choice) = chosen
            && self.route.as_ref().is_some_and(|route| route.shown.choice() != choice)
        {
            self.route(Some(Event::Chosen(choice)));
            if self.run == Run::Going {
                log!("route changed: the update runs again");
                self.run = Run::Again;
            }
        }
        let probed = self.route.as_mut().and_then(|route| {
            let answer = route.probing.as_ref()?.try_recv();
            match answer {
                Ok(probed) => {
                    route.probing = None;
                    Some(probed)
                }
                Err(mpsc::TryRecvError::Empty) => None,
                Err(mpsc::TryRecvError::Disconnected) => {
                    route.probing = None;
                    Some(Probed::Neither)
                }
            }
        });
        if let Some(probed) = probed {
            self.route(Some(Event::Probed(probed)));
        }
        // A port asked for is answered once the session listens on it.
        if let Some(answer) = self.torrents.service()
            && let Some(pipe) = &mut self.pipe
        {
            pipe.send(answer);
        }
        !self.stopped()
    }

    /// The server restarts: after "Server maintenance..." the launcher's
    /// line counts the seconds to the next try (launcher patch 16), until
    /// the site answers by the player's route. `false` when told to stop.
    fn await_server(&mut self, server: Option<&Url>) -> bool {
        let answered = 'asked: loop {
            for left in (1..=RESTART_WAIT_S).rev() {
                self.send(ToLauncher::NextTry(left));
                if !self.pause(1000) {
                    break 'asked false;
                }
            }
            if server.is_none_or(route::site_answers) {
                break true;
            }
        };
        self.send(ToLauncher::NextTry(0));
        answered
    }

    /// Sleeps, looking at the pipe meanwhile. `false` when told to stop.
    fn pause(&mut self, milliseconds: u32) -> bool {
        let until = win::ticks() + u64::from(milliseconds);
        while win::ticks() < until {
            if !self.pump() {
                return false;
            }
            win::sleep(100);
        }
        self.pump()
    }

    fn begin_progress(&mut self) {
        self.started = win::ticks();
        self.last_progress = 0;
        self.send(ToLauncher::Progress {
            done: 0,
            total: 0,
            speed: 0,
            eta: 0,
        });
    }

    /// Reports bytes done of a total, four times a second at most (always
    /// at the end). `false` when told to stop.
    fn progress(&mut self, done: u64, total: u64) -> bool {
        let now = win::ticks();
        if done < total && now < self.last_progress + 250 {
            return !self.stopped();
        }
        self.last_progress = now;
        let elapsed = now.saturating_sub(self.started).max(1);
        let speed = done.saturating_mul(1000) / elapsed;
        let eta = if speed == 0 || total < done {
            u64::MAX
        } else {
            (total - done) / speed
        };
        self.send(ToLauncher::Progress {
            done,
            total,
            speed,
            eta,
        });
        self.pump()
    }

    /// Says something the player must read: the launcher shows it in a box
    /// of its own; started by hand, the updater shows the box.
    fn tell(&mut self, message: &str) {
        if self.pipe.is_some() {
            self.send(ToLauncher::ErrorText(message));
        } else if !self.quiet {
            message_box(message);
        }
    }

    /// The update cannot go on: says why, to the player and to the log.
    /// (The state first: the launcher's line says "Error" while its box
    /// with the reason is open.)
    fn fail(&mut self, message: &str) {
        log!("FAILED: {message}");
        self.state(state::ERROR);
        self.tell(message);
    }

    /// After the last message: stays until the launcher says stop or goes
    /// (as the original updater does; the launcher takes a closed pipe for
    /// a failure), or the pass is to run again.
    fn linger(&mut self) {
        while self.pipe.is_some() && self.pump() {
            win::sleep(100);
        }
    }

    /// Tells the launcher this updater is done for good, and closes the
    /// pipe. A launcher that did not ask for it closes itself on that.
    fn leave(&mut self) {
        self.run = Run::Stopped;
        // Sharing ends with the launcher, before the next updater takes the port.
        self.torrents.close();
        if let Some(mut pipe) = self.pipe.take() {
            pipe.send(ToLauncher::State(state::ABORTED));
            pipe.close();
        }
    }
}

/// One of the updater's own boxes, on top: Windows' answer (the button).
fn show_box(caption: &str, message: &str, style: MESSAGEBOX_STYLE) -> MESSAGEBOX_RESULT {
    let title = win::wide(caption);
    let body = win::wide(message);
    unsafe {
        MessageBoxW(
            core::ptr::null_mut(),
            body.as_ptr(),
            title.as_ptr(),
            style | MB_SETFOREGROUND | MB_TOPMOST,
        )
    }
}

fn message_box(message: &str) {
    show_box(text::TITLE, message, MB_OK | MB_ICONERROR);
}

/// A box of `updater_logic::problem` that needs no answer: OK.
fn warning_box(caption: &str, message: &str) {
    show_box(caption, message, MB_OK | MB_ICONWARNING);
}

/// A stop's box (`updater_logic::problem`, `updater_logic::file_error`):
/// Windows' Retry and Cancel. `true`: Retry.
fn retry_box(caption: &str, message: &str) -> bool {
    show_box(caption, message, MB_RETRYCANCEL | MB_ICONWARNING) == IDRETRY
}

/// Between tries while the server restarts.
const RESTART_WAIT_S: u8 = 5;

/// The box of a server that restarts (`Diagnosis::Restarting`): Retry and
/// Cancel as [`retry_box`]'s, and under `message` a line that counts the
/// seconds to the next try. While it is open `back` is asked every
/// [`RESTART_WAIT_S`] seconds, and once it says yes the box presses its own
/// Retry. `true`: Retry, the player's or its own.
fn waiting_box(caption: &str, message: &str, language: Option<u32>, back: impl Fn() -> bool + Sync) -> bool {
    use core::sync::atomic::{AtomicBool, Ordering};
    /// A message box's window class, and its text's control.
    const DIALOG: &str = "#32770";
    const TEXT: i32 = 0xFFFF;
    let language = language.unwrap_or(0);
    let text = |seconds: u8| format!("{message}\n{}", next_try(language, seconds.into()));
    let title = win::wide(caption);
    let owner = unsafe { GetCurrentThreadId() };
    // This thread's box with that title, once it is shown.
    let shown = || {
        let window = unsafe { FindWindowW(win::wide(DIALOG).as_ptr(), title.as_ptr()) };
        let ours = !window.is_null() && unsafe { GetWindowThreadProcessId(window, core::ptr::null_mut()) } == owner;
        ours.then_some(window)
    };
    let closed = AtomicBool::new(false);
    std::thread::scope(|scope| {
        scope.spawn(|| {
            let mut left = RESTART_WAIT_S;
            loop {
                for _ in 0..10 {
                    if closed.load(Ordering::Relaxed) {
                        return;
                    }
                    win::sleep(100);
                }
                left -= 1;
                if left == 0 {
                    if back() {
                        log!("the server answers: the box's Retry");
                        if let Some(window) = shown() {
                            unsafe { PostMessageW(window, WM_COMMAND, IDRETRY as usize, 0) };
                        }
                        return;
                    }
                    left = RESTART_WAIT_S;
                }
                if let Some(window) = shown() {
                    unsafe { SetDlgItemTextW(window, TEXT, win::wide(&text(left)).as_ptr()) };
                }
            }
        });
        let answer = show_box(caption, &text(RESTART_WAIT_S), MB_RETRYCANCEL | MB_ICONWARNING);
        closed.store(true, Ordering::Relaxed);
        answer == IDRETRY
    })
}

/// The server to ask: the one named in `updater_skhid.url`, else the one of
/// the build.
fn server(layout: &Layout) -> Result<Url, String> {
    let path = format!("{}\\{URL_FILE}", layout.exe_dir);
    let (address, source) = match win::read_file(&path, 4096) {
        Ok(bytes) => (String::from_utf8_lossy(&bytes).trim().into(), URL_FILE),
        Err(_) => (String::from(DEFAULT_URL), "built in"),
    };
    log!("server: {address} ({source})");
    url::parse(&address).ok_or_else(|| text::bad_address(&address, source))
}

/// What a look at an installed file finds. A file that cannot be read is
/// never the same (`updater_logic::file_error`): blocked by Windows Security
/// or held by another program (read a few times first), the update stops on
/// it; else it is repaired.
fn look(layout: &Layout, file: &InstalledFile) -> Found {
    let place = layout.place(&file.name);
    let mut attempt = 1;
    loop {
        match win::hash_file(&place) {
            Ok((size, hash)) if size == file.size && hash == file.sha256 => return Found::Same,
            Ok(_) => return Found::Changed,
            Err(code) if win::is_missing(code) => return Found::Missing,
            Err(code) if file_error::kind(code) == Kind::InUse && attempt < IN_USE_TRIES => {
                log!(
                    "{}: cannot be read now: {} (error {code}), try {attempt} of {IN_USE_TRIES}",
                    file.name,
                    win::error_text(code)
                );
                win::sleep(IN_USE_PAUSE_MS);
                attempt += 1;
            }
            Err(code) => {
                log!(
                    "{}: cannot be read: {} (error {code})",
                    file.name,
                    win::error_text(code)
                );
                return Found::after_error(code);
            }
        }
    }
}

fn read_installed(layout: &Layout) -> Option<Installed> {
    let bytes = win::read_file(&format!("{}\\{STATE_FILE}", layout.exe_dir), 1 << 20).ok()?;
    parse_installed(&bytes)
}

/// The site's half of the update check: `GET /api/client-files`, tried
/// [`CHECK_TRIES`] times, each try on a thread of its own, so the pipe and
/// the DHT ([`check`]) go on while it waits.
struct Asking {
    server: Url,
    tries: u32,
    now: Ask,
}

enum Ask {
    Waiting(mpsc::Receiver<Result<Vec<u8>, HttpError>>),
    /// After a failed try, until this tick.
    Pausing(u64),
    Done(Result<Manifest, String>),
}

impl Asking {
    fn start(session: &mut Session, server: &Url) -> Self {
        let mut asking = Self {
            server: server.clone(),
            tries: 0,
            now: Ask::Pausing(0),
        };
        asking.poll(session);
        asking
    }

    fn poll(&mut self, session: &mut Session) {
        let result = match &self.now {
            Ask::Done(_) => return,
            Ask::Pausing(until) => {
                if win::ticks() >= *until {
                    self.send(session);
                }
                return;
            }
            Ask::Waiting(answer) => match answer.try_recv() {
                Ok(result) => result,
                Err(mpsc::TryRecvError::Empty) => return,
                Err(mpsc::TryRecvError::Disconnected) => Err(HttpError::Stopped),
            },
        };
        self.tried(session, result);
    }

    fn send(&mut self, session: &mut Session) {
        self.tries += 1;
        session.state(state::CHECKING);
        let (sender, answer) = mpsc::channel();
        let server = self.server.clone();
        std::thread::spawn(move || {
            let _ = sender.send(http::get(&server, "/api/client-files", 4096, &mut |_, _| true));
        });
        self.now = Ask::Waiting(answer);
    }

    fn tried(&mut self, session: &mut Session, result: Result<Vec<u8>, HttpError>) {
        let server = &self.server;
        session.reached(server, result.as_ref().map(|_| ()));
        let failed = match result {
            Ok(body) => match parse_manifest(&body) {
                Ok(manifest) => {
                    log!(
                        "server offers archive {} ({} bytes)",
                        core::str::from_utf8(&to_hex(&manifest.sha256)).unwrap_or(""),
                        manifest.size
                    );
                    self.now = Ask::Done(Ok(manifest));
                    return;
                }
                Err(ManifestError::NewerFormat) => Err(text::newer_format(server)),
                Err(ManifestError::TooLarge) => Err(text::archive_too_large(server)),
                Err(error) => Ok(text::not_the_server(server, &format!("{error:?}"))),
            },
            // The server runs but has no client files to offer: nothing to
            // wait for.
            Err(HttpError::Status(404)) => Err(text::nothing_offered(server)),
            Err(error) => Ok(text::http(server, "/api/client-files", &error)),
        };
        // `Ok`: worth another try.
        self.now = match failed {
            Ok(last) => {
                log!("check {} of {CHECK_TRIES} failed: {last}", self.tries);
                if self.tries < CHECK_TRIES {
                    session.state(state::CHECK_FAILED);
                    Ask::Pausing(win::ticks() + u64::from(RETRY_PAUSE_MS))
                } else {
                    Ask::Done(Err(last))
                }
            }
            Err(last) => Ask::Done(Err(last)),
        };
    }

    /// Whether the site has given its answer (not a failure).
    fn answered(&mut self, session: &mut Session) -> bool {
        self.poll(session);
        matches!(self.now, Ask::Done(Ok(_)))
    }

    /// The site's answer, waited for. `Err(None)`: told to stop.
    fn wait(mut self, session: &mut Session) -> Result<Manifest, Option<String>> {
        loop {
            self.poll(session);
            if let Ask::Done(answer) = self.now {
                return answer.map_err(Some);
            }
            if !session.pump() {
                return Err(None);
            }
            win::sleep(50);
        }
    }
}

/// Who answered the update check.
enum Answer {
    /// The site, whenever it answered before the DHT did: the update is
    /// the site's (HTTPS), and the folder is shared as ever.
    Site(Manifest),
    /// The release's signed DHT item named this revision before the site
    /// answered (torrent on). The site, still asked, is the fallback.
    Signed(u64, Asking),
    /// Neither: why the site could not be asked.
    Neither(String),
}

/// The update check: the site and, with torrent on, the DHT at once
/// (`torrent::discover`), so a site that does not answer never holds up
/// the DHT's answer. `None`: told to stop.
fn check(session: &mut Session, layout: &Layout, server: &Url) -> Option<Answer> {
    let mut asking = Asking::start(session, server);
    let signed = torrent::discover(session, layout, &mut |session| !asking.answered(session));
    if session.stopped() {
        return None;
    }
    match signed {
        Some(revision) if !asking.answered(session) => Some(Answer::Signed(revision, asking)),
        _ => match asking.wait(session) {
            Ok(manifest) => Some(Answer::Site(manifest)),
            Err(None) => None,
            Err(Some(why)) => Some(Answer::Neither(why)),
        },
    }
}

/// Downloads the archive the server offers and checks it against
/// `manifest`. `Err(None)`: told to stop.
fn download(
    session: &mut Session,
    server: &Url,
    manifest: &mut Manifest,
) -> Result<Vec<u8>, Option<String>> {
    let mut last = String::new();
    // How the last try failed, for the player's box.
    let mut miss = Miss::Answered(Fault::Other);
    for attempt in 1..=DOWNLOAD_TRIES {
        session.begin_progress();
        session.state(state::DOWNLOADING);
        let size = manifest.size;
        let result = http::get(
            server,
            "/download",
            MAX_ARCHIVE_SIZE as usize,
            &mut |done, total| session.progress(done, total.unwrap_or(size)),
        );
        session.reached(server, result.as_ref().map(|_| ()));
        let mut pause = RETRY_PAUSE_MS;
        match result {
            Ok(body) => {
                if body.len() as u64 == manifest.size && digest(&body) == manifest.sha256 {
                    log!("downloaded {} bytes, checksum right", body.len());
                    return Ok(body);
                }
                // Not the archive that was announced: the server may have
                // been given another one between the two requests.
                last = text::checksum_mismatch(server);
                miss = Miss::Answered(Fault::Damaged);
                if let Ok(body) = http::get(server, "/api/client-files", 4096, &mut |_, _| true)
                    && let Ok(now) = parse_manifest(&body)
                {
                    *manifest = now;
                }
            }
            Err(HttpError::Stopped) => return Err(None),
            Err(HttpError::Status(503)) => {
                last = text::busy(server);
                miss = Miss::Answered(Fault::Status(503));
                pause = BUSY_PAUSE_MS;
            }
            Err(error) => {
                last = text::http(server, "/download", &error);
                miss = error.miss();
            }
        }
        log!("download {attempt} of {DOWNLOAD_TRIES} failed: {last}");
        if !session.pump() {
            return Err(None);
        }
        if attempt < DOWNLOAD_TRIES {
            session.state(state::DOWNLOAD_FAILED);
            if !session.pause(pause) {
                return Err(None);
            }
        }
    }
    // Not reached: the server's box ([`Session::reached`]); else the
    // download's.
    if let Miss::Answered(_) = miss {
        session.failed(&format!("{}/download", server.host), &[miss]);
    }
    Err(Some(last))
}

/// Unpacks every file of the archive and checks it, before anything is
/// written.
fn unpack(archive: &[u8]) -> Result<Vec<NewFile>, String> {
    let entries =
        zip::entries(archive).map_err(|error| text::bad_archive(&format!("{error:?}")))?;
    if entries.is_empty() {
        return Err(text::bad_archive("it holds no file"));
    }
    let mut files = Vec::new();
    for entry in entries {
        let data = entry
            .read()
            .map_err(|error| text::bad_archive(&format!("{}: {error:?}", entry.name)))?;
        files.push(NewFile {
            name: entry.name.into(),
            sha256: digest(&data),
            data,
        });
    }
    Ok(files)
}

enum Done {
    /// The files are the server's; nothing was changed.
    UpToDate,
    /// The files were changed; `restart`: the launcher too. `put_back`: a
    /// program file this updater had installed was gone at the start (its
    /// name): an antivirus removes them, and the player is told so.
    Installed {
        restart: bool,
        put_back: Option<String>,
    },
    /// The server could not be asked (why: [`gate::Seen::Unasked`]), and
    /// what is installed is whole: the game can be started with it (the
    /// words for the player).
    Offline(String),
    Stopped,
    Failed(String),
    /// The game exe says an older revision than the server lets in (the
    /// words for the player): PLAY stays off, and the server is watched.
    Behind(String),
    /// A file (its name in the archive, or its path) is blocked by Windows
    /// Security or held by another program: PLAY stays off, the player is
    /// told what to do, and Retry runs the update again.
    Stop(Stop, String),
}

/// The update: the overlay, then the revision gate (`gate::behind`), the
/// restore of resources.db and the patch step, all under the installation
/// lock.
fn update(session: &mut Session, layout: &Layout, patch_look: patches::Look) -> Done {
    session.trouble = None;
    let _lock = match win::install_lock(&layout.exe_dir) {
        Ok(lock) => lock,
        Err(code) => {
            return Done::Failed(format!(
                "Cannot lock the installation folder (another updater may be running): {}.",
                win::error_text(code)
            ));
        }
    };
    let server = match server(layout) {
        Ok(server) => server,
        Err(message) => return Done::Failed(message),
    };
    let done = overlay(session, layout, &server);
    // The overlay may have brought another torrent.
    session.torrents.forget();
    // New Skhid files may need other patches than those checked so far: until
    // a patch step with the server's list has checked the folder again,
    // `ready` and a start without the server refuse.
    if let Done::Installed { .. } = done {
        patches::overlay_changed(layout);
    }
    // The restore and the patch step run at every start the game may be
    // played after, with the server only when the server answered. When this
    // updater or the launcher was replaced, the new ones start at once and
    // do them.
    match &done {
        Done::UpToDate | Done::Installed { restart: false, .. } | Done::Offline(_) => {}
        Done::Installed { restart: true, .. } => {
            log!(
                "the updater or the launcher was replaced: the rest of the update is done by the new one"
            );
            return done;
        }
        Done::Stopped | Done::Failed(_) | Done::Behind(_) | Done::Stop(..) => return done,
    }
    // A game older than the server lets in (or, the site not asked, than
    // the newest release signed) is not played, whatever its patches.
    if let Some(seen) = &session.seen
        && let Some(message) = gate::behind(layout, seen)
    {
        return Done::Behind(message);
    }
    let unasked = match &session.seen {
        Some(gate::Seen::Unasked { why, .. }) => Some(why.clone()),
        _ => None,
    };
    let server = match &unasked {
        None => Ok(&server),
        Some(why) => Err(why.as_str()),
    };
    // Best effort: what an earlier updater wrote into resources.db in place
    // gets its stock bytes back, once.
    match resources::run(session, layout, server.ok()) {
        resources::Outcome::Done => {}
        resources::Outcome::Stopped => return Done::Stopped,
    }
    // The patches of resources.db the server lists: required, every one of
    // them checked before the game is played.
    let installed = read_installed(layout);
    let overlay: Vec<&str> = installed
        .iter()
        .flat_map(|installed| installed.files.iter().map(|file| file.name.as_str()))
        .collect();
    match patches::run(session, layout, server, &overlay, patch_look) {
        patches::Outcome::Ready => done,
        patches::Outcome::Stopped => Done::Stopped,
        patches::Outcome::Blocked(message) => Done::Failed(message),
        patches::Outcome::Stop(stop, name) => Done::Stop(stop, name),
    }
}

/// The overlay: the Skhid files of the server's archive (and the original
/// game first, when it is missing).
fn overlay(session: &mut Session, layout: &Layout, server: &Url) -> Done {
    game::sweep(layout);
    let installed = read_installed(layout);
    install::clean_up(
        layout,
        installed
            .iter()
            .flat_map(|installed| installed.files.iter().map(|file| file.name.as_str())),
    );
    let site = match check(session, layout, server) {
        None => return Done::Stopped,
        Some(Answer::Site(manifest)) => Ok(manifest),
        Some(Answer::Neither(why)) => Err((why, None)),
        Some(Answer::Signed(revision, asking)) => {
            session.seen = Some(gate::Seen::Unasked {
                server: server.clone(),
                archive: installed.as_ref().map(|installed| installed.archive),
                signed: Some(revision),
                why: text::signed_first(server, revision),
            });
            if let Some(done) = signed_overlay(session, layout, installed.as_ref(), revision) {
                return done;
            }
            log!("revision {revision} cannot be had without the site: its answer is waited for");
            match asking.wait(session) {
                Ok(manifest) => Ok(manifest),
                Err(None) => return Done::Stopped,
                Err(Some(why)) => Err((why, Some(revision))),
            }
        }
    };
    let mut manifest = match site {
        Ok(manifest) => {
            torrent::remove_skhid_parts(layout);
            session.seen = Some(gate::Seen::Site {
                server: server.clone(),
                archive: manifest.sha256,
                min_revision: manifest.min_revision,
            });
            manifest
        }
        Err((message, signed)) => {
            return unasked(session, layout, server, installed.as_ref(), message, signed);
        }
    };
    // The original game is unpacked with 7zr.exe, a file of the Skhid
    // archive. With that file in place the game comes first, as a player's
    // own copy of it would be there first. Without it (deleted, damaged, the
    // archive only half unpacked) the Skhid files come first, which brings
    // it, and the game after them.
    let game_after = !game::present(layout) && !game::extractor_ready(layout);
    let game_added = if game_after {
        log!("the original game is missing: the Skhid files first (its extractor, its torrent)");
        false
    } else {
        match game::ensure(session, layout, Some(server)) {
            Ok(added) => added,
            Err(None) => return Done::Stopped,
            Err(Some(message)) => return Done::Failed(message),
        }
    };
    let verdict = if game_added {
        Verdict::Install(Reason::NothingInstalled)
    } else {
        verdict(installed.as_ref(), &manifest, |file| look(layout, file))
    };
    let reason = match verdict {
        Verdict::Install(reason) => Some(reason),
        Verdict::UpToDate => None,
        Verdict::Stop(stop, name) => return Done::Stop(stop, name),
    };
    let Some(reason) = reason else {
        if game_after {
            // The Skhid files are the server's and the extractor is not
            // among them: this says so.
            return match game::ensure(session, layout, Some(server)) {
                Ok(_) => Done::Installed {
                    restart: false,
                    put_back: None,
                },
                Err(None) => Done::Stopped,
                Err(Some(message)) => Done::Failed(message),
            };
        }
        log!("up to date");
        return Done::UpToDate;
    };
    log!("install needed: {reason:?}");
    if let Reason::Changed(name) | Reason::Missing(name) | Reason::Unreadable(name, _) = &reason {
        log!("{}", text::repairing(name));
    }
    // A program this updater installed and that is gone: what antivirus
    // programs do to the patched files. The player is told once it is back.
    let put_back = match &reason {
        Reason::Missing(name) if is_program(name) => Some(name.clone()),
        _ => None,
    };

    let archive = match download(session, server, &mut manifest) {
        Ok(archive) => archive,
        Err(None) => return Done::Stopped,
        Err(Some(message)) => return Done::Failed(message),
    };
    // The server may have been given another archive meanwhile: this one is
    // what the run acts on.
    session.seen = Some(gate::Seen::Site {
        server: server.clone(),
        archive: manifest.sha256,
        min_revision: manifest.min_revision,
    });
    session.state(state::DOWNLOADED);
    let files = match unpack(&archive) {
        Ok(files) => files,
        Err(message) => return Done::Failed(message),
    };
    drop(archive);

    // The archive that is installed already: a repair.
    let repair = installed
        .as_ref()
        .is_some_and(|installed| installed.archive == manifest.sha256);
    let outcome = match put_in_place(session, layout, installed.as_ref(), &files, manifest.sha256, repair) {
        Ok(outcome) => outcome,
        Err(done) => return done,
    };
    // The game that waited for its extractor. When the launcher or this
    // program was replaced, the start that follows goes on with it.
    if game_after && !outcome.restart {
        match game::ensure(session, layout, Some(server)) {
            Ok(_) => {}
            Err(None) => return Done::Stopped,
            Err(Some(message)) => return Done::Failed(message),
        }
    }
    Done::Installed {
        restart: outcome.restart,
        put_back,
    }
}

/// Installs an archive's files, whatever brought them. `archive`: its
/// SHA-256 or a torrent's v2 info hash; `repair`: it is installed already.
fn put_in_place(
    session: &mut Session,
    layout: &Layout,
    installed: Option<&Installed>,
    files: &[NewFile],
    archive: [u8; 32],
    repair: bool,
) -> Result<install::Outcome, Done> {
    // The launcher's "installing" line: "Updating", then "Extracting file
    // <name>" for each file. It is entered with no file name set (this
    // updater sent none so far) and left by the next state message.
    session.begin_progress();
    session.state(state::INSTALLING);
    install::clean_up(layout, files.iter().map(|file| file.name.as_str()));
    let outcome = match install::install(layout, files, repair, &mut |step| match step {
        Step::File(name) => {
            let file_name = name.rsplit('/').next().unwrap_or(name);
            session.send(ToLauncher::InstallInfo {
                step: step::EXTRACT,
                text: file_name,
            });
            true
        }
        Step::Bytes(done, total) => session.progress(done, total),
    }) {
        Ok(outcome) => outcome,
        Err(InstallError::Stopped) => return Err(Done::Stopped),
        // Windows Security does not let the file be written: the same stop
        // as when it cannot be read.
        Err(error) => {
            return Err(match error.virus() {
                Some((path, code)) => {
                    log!(
                        "{path} cannot be written: {} (error {code})",
                        win::error_text(code)
                    );
                    Done::Stop(Stop::Virus, path.into())
                }
                None => Done::Failed(text::install(&error)),
            });
        }
    };
    for name in &outcome.written {
        log!("installed {name}");
    }
    // A file gone the moment it was put in place: an antivirus took it.
    if let Some(name) = outcome
        .written
        .iter()
        .find(|name| !win::exists(&layout.place(name)))
    {
        log!("{name} is gone right after it was installed");
        return Err(Done::Stop(Stop::Virus, name.clone()));
    }

    // What the archive before had and this one has not goes, if it is still
    // the file that was installed (a file the player changed is the
    // player's).
    for old in installed.iter().flat_map(|installed| installed.files.iter()) {
        if !files
            .iter()
            .any(|file| file.name.eq_ignore_ascii_case(&old.name))
            && look(layout, old) == Found::Same
        {
            match win::delete(&layout.place(&old.name)) {
                Ok(()) => log!("removed {} (no longer in the archive)", old.name),
                Err(code) => log!(
                    "{} could not be removed ({})",
                    old.name,
                    win::error_text(code)
                ),
            }
        }
    }

    // What is installed now, for the next start.
    save_installed(
        layout,
        &Installed {
            archive,
            files: files
                .iter()
                .map(|file| InstalledFile {
                    name: file.name.clone(),
                    sha256: file.sha256,
                    size: file.data.len() as u64,
                })
                .collect(),
        },
    );
    log!(
        "installed {} file(s) of {}",
        outcome.written.len(),
        files.len()
    );
    Ok(outcome)
}

/// The DHT named the newest release, `revision`, before the site answered:
/// the folder found whole at it, or brought to it from its signed torrent,
/// without the site. `None`: not without the site.
fn signed_overlay(session: &mut Session, layout: &Layout, installed: Option<&Installed>, revision: u64) -> Option<Done> {
    if crate::signed::game_number(layout) >= revision {
        if !game::present(layout) {
            return None;
        }
        return match damage(installed?, |file| look(layout, file)) {
            None => {
                log!("revision {revision}, the newest the release signed, is installed and whole: up to date");
                Some(Done::UpToDate)
            }
            Some(Reason::Stop(stop, name)) => Some(Done::Stop(stop, name)),
            Some(_) => None,
        };
    }
    if torrent::offered(session, layout).is_none_or(|signed| signed.revision < revision) {
        return None;
    }
    by_torrent(session, layout, installed)
}

/// The site could not be asked (`message`): the folder from the run's
/// signed torrent when that is newer, else as it is when whole. `signed`:
/// the newest revision the DHT named.
fn unasked(
    session: &mut Session,
    layout: &Layout,
    server: &Url,
    installed: Option<&Installed>,
    message: String,
    signed: Option<u64>,
) -> Done {
    session.seen = Some(gate::Seen::Unasked {
        server: server.clone(),
        archive: installed.map(|installed| installed.archive),
        signed,
        why: message.clone(),
    });
    if let Some(done) = by_torrent(session, layout, installed) {
        return match done {
            // Still without the server: the rest of the run is offline.
            Done::Installed { restart: false, .. } => {
                patches::overlay_changed(layout);
                Done::Offline(text::offline(&message))
            }
            done => done,
        };
    }
    // The original game from the addresses built into this updater
    // (a provider that blocks the server need not block them).
    if !game::present(layout) {
        log!(
            "the original game is missing and the update server cannot be reached: the \
             addresses built in"
        );
        match game::ensure(session, layout, None) {
            Ok(_) => {}
            Err(None) => return Done::Stopped,
            Err(Some(why)) => {
                return Done::Failed(format!(
                    "The original game is not installed and the update server cannot be reached. {message} {why}"
                ));
            }
        }
    }
    // No answer. What is installed and whole may still be played, and so
    // may files this updater never installed (copied by hand, the first
    // start): it knows nothing against them. Files it installed and now
    // finds damaged may not.
    match installed {
        None => Done::Offline(text::offline_unchecked(&message)),
        Some(installed) => match damage(installed, |file| look(layout, file)) {
            None => Done::Offline(text::offline(&message)),
            Some(Reason::Missing(name) | Reason::Changed(name)) => {
                Done::Failed(text::offline_damaged(&message, &name))
            }
            Some(Reason::Unreadable(name, code)) => {
                Done::Failed(text::offline_unreadable(&message, &name, code))
            }
            Some(Reason::Stop(stop, name)) => Done::Stop(stop, name),
            Some(_) => Done::Failed(message),
        },
    }
}

/// The Skhid files from the run's signed torrent ([`torrent_overlay`]),
/// then the original game when it is missing. `None`: not by torrent.
fn by_torrent(session: &mut Session, layout: &Layout, installed: Option<&Installed>) -> Option<Done> {
    let done = torrent_overlay(session, layout, installed)?;
    // Nothing installed (stopped, failed): no game download either.
    if let Done::Installed { .. } = done
        && !game::present(layout)
    {
        match game::ensure(session, layout, None) {
            Ok(_) => {}
            Err(None) => return Some(Done::Stopped),
            Err(Some(why)) => return Some(Done::Failed(why)),
        }
    }
    Some(done)
}

/// When the server cannot be asked: the Skhid files from the signed
/// torrent, if this folder has none or an older revision's. Installed as an
/// archive whose id is the torrent's info hash, so the server's archive goes
/// over them once it answers. `None`: not by torrent.
fn torrent_overlay(session: &mut Session, layout: &Layout, installed: Option<&Installed>) -> Option<Done> {
    let signed = torrent::offered(session, layout)?;
    let torrent = &signed.torrent;
    let game = crate::signed::game_number(layout);
    let newer = match installed {
        None => true,
        Some(installed) => installed.archive != torrent.v2 && signed.revision > game,
    };
    if !newer {
        return None;
    }
    let skhid = torrent::skhid_files(torrent);
    let part = |index: usize| torrent::skhid_part(layout, &torrent.files[index].path);
    let places: Vec<(usize, String)> = skhid.iter().map(|&index| (index, layout.place(&torrent.files[index].path))).collect();
    let Some(whole) = torrent::whole(session, &signed, &places) else {
        return if session.stopped() { Some(Done::Stopped) } else { None };
    };
    let missing: Vec<(usize, String)> = skhid
        .iter()
        .zip(&whole)
        .filter(|(_, whole)| !**whole)
        .map(|(&index, _)| (index, part(index)))
        .collect();
    log!(
        "the Skhid files of revision {} by torrent: {} of {} to fetch",
        signed.revision,
        missing.len(),
        skhid.len()
    );
    for (_, path) in &missing {
        let _ = win::create_folders(win::split_path(path).0);
    }
    if !missing.is_empty() {
        match torrent::fetch(session, &signed, &missing) {
            torrent::Fetched::Done => {}
            torrent::Fetched::Stopped => return Some(Done::Stopped),
            torrent::Fetched::Not(why) => {
                log!("the Skhid files by torrent: {why}");
                return None;
            }
        }
    }
    let mut files = Vec::new();
    for &index in &skhid {
        let file = &torrent.files[index];
        let from = missing
            .iter()
            .find(|(fetched, _)| *fetched == index)
            .map_or_else(|| layout.place(&file.path), |(_, part)| part.clone());
        let data = match win::read_file(&from, MAX_ARCHIVE_SIZE as usize) {
            Ok(data) => data,
            Err(code) => return Some(Done::Failed(format!("Cannot read {from}: {}.", win::error_text(code)))),
        };
        // Every piece was checked; only extra bytes could be wrong.
        if data.len() as u64 != file.size {
            return Some(Done::Failed(format!(
                "{from} has {} bytes, not the {} of the torrent's {}.",
                data.len(),
                file.size,
                file.path
            )));
        }
        files.push(NewFile {
            sha256: digest(&data),
            name: file.path.clone(),
            data,
        });
    }
    log!("the Skhid files of revision {} from the torrent: installing", signed.revision);
    let outcome = put_in_place(session, layout, installed, &files, torrent.v2, false);
    for (_, part) in &missing {
        let _ = win::delete(part);
    }
    Some(match outcome {
        Ok(outcome) => Done::Installed {
            restart: outcome.restart,
            put_back: None,
        },
        Err(done) => done,
    })
}

/// Writes `updater_skhid.state`. A failure is logged: the next start
/// compares the files and writes it again.
fn save_installed(layout: &Layout, installed: &Installed) {
    let state_path = format!("{}\\{STATE_FILE}", layout.exe_dir);
    let staged = format!("{state_path}.skhid-new");
    let saved = win::write_file(&staged, &write_installed(installed))
        .and_then(|()| win::rename(&staged, &state_path, true));
    if let Err(code) = saved {
        log!("{STATE_FILE} could not be written ({})", win::error_text(code));
        let _ = win::delete(&staged);
    }
}

/// A file of the archive that is run: the files antivirus programs remove.
fn is_program(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    name.ends_with(".exe") || name.ends_with(".dll")
}

struct Options {
    /// The launcher's pipe; `None` when started by hand.
    pipe_name: Option<String>,
    /// The original's `copy` action: nothing to do here.
    copy: bool,
    /// `ready`: only whether the game's files are updated (play_skhid.bat).
    ready: bool,
    quiet: bool,
    /// `check`: every patch file is hashed, whatever was hashed before (the
    /// repair); the launcher's update takes the quick check.
    look: patches::Look,
    /// The launcher's "Allow torrent" and port (its 4th and 13th arguments).
    torrenting: Torrenting,
}

/// The command line: the launcher's (`"<exe>" "update" "<pipe>" ...`, the
/// original updater's arguments; all but the first two are of no use here),
/// or `check [quiet]` by hand.
fn options() -> Result<Options, String> {
    options_of(&arguments())
}

/// The program's command line, cut into its arguments.
fn arguments() -> Vec<String> {
    let line = win::from_wide(unsafe {
        let pointer = GetCommandLineW();
        let mut length = 0;
        while *pointer.add(length) != 0 {
            length += 1;
        }
        core::slice::from_raw_parts(pointer, length)
    });
    updater_logic::args::split(&line)
}

fn options_of(arguments: &[String]) -> Result<Options, String> {
    let action = arguments.get(1).map_or("", String::as_str);
    let rest = arguments.get(2..).unwrap_or(&[]);
    let no_pipe = rest
        .iter()
        .any(|argument| argument.eq_ignore_ascii_case("disable_pipe"));
    let pipe_name = || {
        rest.first()
            .filter(|name| name.starts_with("\\\\.\\pipe\\") && !no_pipe)
            .cloned()
    };
    match action.to_ascii_lowercase().as_str() {
        action @ ("check" | "ready") => Ok(Options {
            pipe_name: None,
            copy: false,
            ready: action == "ready",
            quiet: rest
                .iter()
                .any(|argument| argument.eq_ignore_ascii_case("quiet")),
            look: if action == "check" {
                patches::Look::Full
            } else {
                patches::Look::Quick
            },
            // `torrent`/`direct` as `finish_by_new` passes it, else the
            // launcher's setting; the kept port. `ready` only reads.
            torrenting: if action == "check" {
                let said = rest.iter().find_map(|argument| {
                    ["torrent", "direct"]
                        .into_iter()
                        .find(|word| argument.eq_ignore_ascii_case(word))
                });
                let saved = if win::launcher_torrent_allowed() { "torrent" } else { "direct" };
                Torrenting::of_arguments(said.unwrap_or(saved), "")
            } else {
                Torrenting::Off { port: None }
            },
        }),
        "update" | "full_update" => Ok(Options {
            pipe_name: pipe_name(),
            copy: false,
            ready: false,
            quiet: false,
            look: patches::Look::Quick,
            torrenting: Torrenting::of_arguments(
                rest.get(2).map_or("", String::as_str),
                rest.get(11).map_or("", String::as_str),
            ),
        }),
        "copy" => Ok(Options {
            pipe_name: pipe_name(),
            copy: true,
            ready: false,
            quiet: false,
            look: patches::Look::Quick,
            torrenting: Torrenting::Off { port: None },
        }),
        _ => Err(text::usage()),
    }
}

fn run() -> u32 {
    // "Watch replay": started by the patched game with a replay link
    // (`replay.rs`). Exactly this word, as the game's patch sends it.
    let arguments = arguments();
    // Started by the patched game on its login screen and in its lobby.
    if let Some(pipe) = updater_logic::seed::command(&arguments) {
        return seed::run(pipe);
    }
    if let Some((file_arguments, quiet)) = updater_logic::local_replay::command(&arguments) {
        return replay::run_local(file_arguments, quiet);
    }
    if let Some((replay_arguments, quiet)) = updater_logic::replay::command(&arguments) {
        return replay::run(replay_arguments, quiet);
    }
    if arguments.get(1).is_some_and(|a| a == "register-replays") {
        return register_replays(&arguments[2..]);
    }
    if arguments.get(1).is_some_and(|a| a == "play") {
        return play::run(&arguments[2..]);
    }
    let options = options();
    let layout = match Layout::find() {
        Ok(layout) => layout,
        Err(path) => {
            if !options.as_ref().is_ok_and(|options| options.quiet) {
                message_box(&text::wrong_folder(&path));
            }
            return exit::FAILED;
        }
    };
    log::start(&layout);
    log!(
        "---- updater {} started in {}",
        env!("CARGO_PKG_VERSION"),
        layout.exe_dir
    );
    let options = match options {
        Ok(options) => options,
        Err(message) => {
            log!("{message}");
            message_box(&message);
            return exit::FAILED;
        }
    };
    if options.ready {
        // Read only: no lock, no server, no pipe.
        return match patches::ready(&layout) {
            Ok(()) => {
                log!("exit 0");
                exit::OK
            }
            Err(message) => {
                log!("FAILED: {message}");
                if !options.quiet {
                    message_box(&message);
                }
                exit::FAILED
            }
        };
    }
    let pipe = match &options.pipe_name {
        Some(name) => match Pipe::connect(name) {
            Ok(pipe) => Some(pipe),
            Err(code) => {
                log!("pipe {name}: {}", win::error_text(code));
                return exit::NO_PIPE;
            }
        },
        None => None,
    };
    let mut session = Session {
        pipe,
        quiet: options.quiet,
        run: Run::Going,
        last_progress: 0,
        started: 0,
        named_file: false,
        seen: None,
        trouble: None,
        torrents: torrent::Torrents::new(options.torrenting, &layout),
        route: None,
    };
    // The "Relay" row, shown only to a launcher (one that knows it, patch
    // 16) in a download with a relay.
    if session.pipe.is_some() && !options.copy {
        match play::config() {
            Ok(Some(config)) => {
                session.route = Some(RouteBox { config, shown: Shown::Direct, probing: None });
                session.route(None);
            }
            Ok(None) => {}
            Err(message) => log!("route: {message}"),
        }
    }

    if options.copy {
        // The original copied the launcher and itself to a temp folder to
        // run them from there. This updater replaces running files where
        // they are, so the copy is "done" at once.
        session.state(state::COPIED);
        session.linger();
        session.leave();
        return exit::OK;
    }

    let product = options
        .pipe_name
        .as_deref()
        .and_then(|name| name.strip_prefix(PIPE_PREFIX));
    let code = loop {
        let code = pass(&mut session, &layout, options.look, product);
        if !session.again() {
            break code;
        }
    };
    session.leave();
    log!("exit {code}");
    code
}

/// The update, what the player is told of it and its last message, then
/// the watch, until the launcher goes or the pass is to run again. The exit
/// code.
fn pass(session: &mut Session, layout: &Layout, look: patches::Look, product: Option<&str>) -> u32 {
    // `watch`: PLAY's state stands on what the server said; while the
    // launcher stays open the server is watched (`gate::watch`).
    let mut done = update(session, layout, look);
    // Said in a box of the updater's own, in the launcher's language: the
    // launcher is not told it again.
    let mut said = false;
    let documents = win::documents();
    let language = documents.as_deref().and_then(win::game_language);
    let log_path = log::path().unwrap_or_else(|| layout.logs_dir.clone());
    let antivirus = |name: &str, removal| {
        let file = name.rsplit(['/', '\\']).next().unwrap_or(name);
        let folder = file_error::exclusion_folder(name, &layout.exe_dir, &layout.torrent_dir, &layout.game_dir);
        Problem::Antivirus {
            file: file.into(),
            folder: folder.into(),
            exclusion: Exclusion::of(name),
            removal,
        }
    };
    /// Who presses Retry in the pass's box.
    enum Retry {
        Player,
        /// The player, or the box once this server answers (it restarts).
        Server(Url),
    }
    loop {
        let (caption, message, retry) = match (&done, session.trouble.clone()) {
            // A file blocked by Windows Security or held by another program:
            // PLAY stays off; the box says what to do, and Retry runs the
            // update again.
            (Done::Stop(stop, name), _) => {
                let file = name.rsplit(['/', '\\']).next().unwrap_or(name);
                log!(
                    "STOPPED: {file} is {}: PLAY stays off until the player has seen to it",
                    match stop {
                        Stop::Virus => "blocked by Windows Security",
                        Stop::InUse => "held by another program",
                    }
                );
                session.state(state::ERROR);
                match stop {
                    Stop::Virus => {
                        let problem = antivirus(name, Removal::Blocked);
                        (problem.caption(), problem.message(language, &log_path), Retry::Player)
                    }
                    Stop::InUse => (text::TITLE.into(), file_error::in_use(language, file), Retry::Player),
                }
            }
            // Every address of the server, or of a file, failed: the box
            // says what (a download that failed, no connection, a server
            // this network blocks) and what to run; Retry runs the update
            // again. Cancel: what the run came to (an error, or PLAY with
            // the files installed).
            (Done::Failed(_) | Done::Offline(_), Some(failed)) => {
                log!("every address failed: {failed:?}");
                if let Done::Failed(_) = done {
                    session.state(state::ERROR);
                }
                if session.quiet || session.stopped() {
                    break;
                }
                let diagnose = || match server(layout) {
                    Ok(server) => route::diagnose_site(&server),
                    Err(_) => Some(Diagnosis::Unclear),
                };
                // `None`: it answers now, as after a restart.
                let problem = Problem::of(failed.clone(), diagnose).unwrap_or_else(|| {
                    Problem::of(failed, || Some(Diagnosis::Restarting)).expect("unreached")
                });
                let retry = match problem {
                    Problem::Unreached { diagnosis: Diagnosis::Restarting, .. } if session.pipe.is_some() => {
                        // Back in a minute: the launcher's line says so
                        // and counts to each next try (PLAY off), and the
                        // update runs again once the server answers, or
                        // the launcher is closed.
                        log!("the server is restarting: asked again every {RESTART_WAIT_S} s");
                        session.state(state::MAINTENANCE);
                        if !session.await_server(server(layout).ok().as_ref()) {
                            // Stopped, or to run again (another route picked):
                            // nothing is told of a wait that was cut.
                            done = Done::Stopped;
                            break;
                        }
                        log!("the server answers: the update runs again");
                        done = update(session, layout, look);
                        continue;
                    }
                    // By hand: the box waits for it.
                    Problem::Unreached { diagnosis: Diagnosis::Restarting, .. } => {
                        server(layout).map_or(Retry::Player, Retry::Server)
                    }
                    Problem::Unreached { diagnosis: Diagnosis::Blocked, .. } => {
                        session.blocked();
                        Retry::Player
                    }
                    _ => Retry::Player,
                };
                log!("the player is told: {}", problem.title(None));
                (problem.caption(), problem.message(language, &log_path), retry)
            }
            _ => break,
        };
        if session.quiet || session.stopped() {
            break;
        }
        let again = match retry {
            Retry::Player => retry_box(&caption, &message),
            Retry::Server(server) => waiting_box(&caption, &message, language, || route::site_answers(&server)),
        };
        if !again || !session.pump() {
            said = true;
            break;
        }
        log!("the player asks to try again");
        done = update(session, layout, look);
    }
    // A program file was gone at the start and is back: the player is told
    // what removes it, before PLAY.
    if let Done::Installed {
        put_back: Some(name),
        ..
    } = &done
    {
        log!("{name} was gone at the start and is put back: an antivirus may remove it again");
        if !session.quiet && !session.stopped() {
            let problem = antivirus(name, Removal::PutBack);
            warning_box(&problem.caption(), &problem.message(language, &log_path));
        }
    }
    // The game keeps up to 3 GB of baked textures in Documents: with less
    // than disk::NEEDED free on its drive PLAY stays off; Retry asks again.
    if matches!(done, Done::UpToDate | Done::Installed { restart: false, .. } | Done::Offline(_)) {
        match documents.as_deref().map(|folder| (folder, win::free_space(folder))) {
            None => log!("the Documents folder is not known: its free space not checked"),
            Some((folder, None)) => log!("the free space of {folder} is not known: not checked"),
            Some((folder, Some(free))) if free < disk::NEEDED => {
                log!("{} MB free in {folder}, {} MB needed: PLAY stays off", free >> 20, disk::NEEDED >> 20);
                session.state(state::ERROR);
                let mut free = free;
                while free < disk::NEEDED && !session.quiet && !session.stopped()
                    && retry_box(text::TITLE, &disk::too_little(language, folder)) && session.pump()
                {
                    free = win::free_space(folder).unwrap_or(0);
                    log!("the player asks to try again: {} MB free", free >> 20);
                }
                if free < disk::NEEDED {
                    said = true;
                    done = Done::Failed(format!("{} MB free in {folder}, {} MB needed", free >> 20, disk::NEEDED >> 20));
                } else {
                    session.state(state::CHECKING);
                }
            }
            Some(_) => {}
        }
    }
    let (code, watch) = match done {
        Done::UpToDate => {
            // "Checking" clears a file name the restore or the patch step
            // left in the launcher's line.
            if session.named_file {
                session.state(state::CHECKING);
            }
            session.state(state::LATEST);
            (exit::OK, true)
        }
        Done::Installed { restart: true, .. } => match product {
            // The launcher or this updater was replaced: the launcher
            // closes, and its new file is started, which starts the new
            // updater: that one restores resources.db and does the patch
            // step.
            Some(product) if session.pipe.is_some() => {
                restart_launcher(session, layout, product);
                return exit::OK;
            }
            // No launcher to start again: the new updater is run here, and
            // its outcome is this one's. PLAY is never said before the
            // patch step ran.
            _ => (finish_by_new(session, layout), false),
        },
        Done::Installed { restart: false, .. } => {
            // "Checking" clears the launcher's file name line.
            session.state(state::CHECKING);
            session.state(state::LATEST);
            (exit::OK, true)
        }
        Done::Offline(message) => {
            log!("the server could not be asked: {message}");
            if session.named_file {
                session.state(state::CHECKING);
            }
            if !said {
                session.tell(&message);
            }
            session.state(state::LATEST);
            (exit::OK, true)
        }
        Done::Stopped => {
            log!("stopped");
            (exit::OK, false)
        }
        Done::Failed(message) if said => {
            log!("FAILED: {message}");
            (exit::FAILED, false)
        }
        Done::Failed(message) => {
            // The whole reason in the log; the launcher's line has the log's
            // name and where to send it after its start.
            let reported = text::reported(&message);
            log!("FAILED: {message}");
            log!("the player is told: {reported}");
            session.state(state::ERROR);
            session.tell(&reported);
            (exit::FAILED, false)
        }
        Done::Behind(message) => {
            session.fail(&message);
            (exit::FAILED, true)
        }
        Done::Stop(..) => {
            log!("FAILED: stopped on a file blocked or held (above)");
            (exit::FAILED, false)
        }
    };
    if code == exit::OK && watch && session.pipe.is_some()
        && let Err(code) = replay_association::register(layout) {
        log!("replay: file registration failed: {} (error {code})", win::error_text(code));
    }
    match (product, session.seen.take()) {
        (Some(product), Some(seen)) if watch && session.pipe.is_some() => {
            gate::watch(session, layout, product, &seen);
        }
        _ => session.linger(),
    }
    code
}

/// Optional repair for a portable installation; normal launcher updates do this.
fn register_replays(arguments: &[String]) -> u32 {
    let quiet = arguments == ["quiet"];
    let result = (|| {
        if !arguments.is_empty() && !quiet { return Err(text::usage()); }
        let layout = Layout::find().map_err(|path| text::wrong_folder(&path))?;
        log::start(&layout);
        replay_association::register(&layout).map_err(win::error_text)
    })();
    match result {
        Ok(()) => exit::OK,
        Err(message) => {
            log!("replay: registration failed: {message}");
            if !quiet { message_box(&message); }
            exit::FAILED
        }
    }
}

/// `check` by hand (no launcher) replaced this updater: the new one is run,
/// as it was started, to finish the update (the restore and the patch step,
/// which this one leaves to it); its exit code is this one's.
fn finish_by_new(session: &mut Session, layout: &Layout) -> u32 {
    let exe = format!("{}\\{}", layout.exe_dir, layout.exe_name);
    let command = format!(
        "\"{exe}\" check{}{}",
        if session.quiet { " quiet" } else { "" },
        if session.torrents.allowed() { " torrent" } else { " direct" }
    );
    log!("this updater was replaced: the new one finishes the update ({command})");
    // Its port and files are the new one's.
    session.torrents.close();
    match win::run_wait(&exe, &command, &layout.exe_dir, "NUL", &mut || true) {
        Ok(code) => {
            log!("the new updater ended with {code}");
            if code == exit::OK {
                session.state(state::CHECKING);
                session.state(state::LATEST);
            } else if session.pipe.is_some() {
                session.state(state::ERROR);
            }
            code
        }
        Err(code) => {
            session.fail(&text::replaced_not_run(&win::error_text(code)));
            exit::FAILED
        }
    }
}

/// How long [`restart_launcher`] waits for the launcher to close: it closes
/// on the closed pipe only once its message loop runs again, which a box it
/// shows (the updater's words) holds up until the player clicks OK.
const LAUNCHER_GONE_MS: u64 = 60 * 60 * 1000;

/// The launcher is to start anew: it was replaced, or the server changed
/// while it stayed open (`gate::watch`). It is told that this updater is
/// done, on which it closes; once it is gone, its file is started again.
/// The launcher allows one of itself at a time, by a named mutex: a second
/// one started while the first holds it says "already running" and ends,
/// and the first then closes too, leaving none. So the new one is never
/// started while the mutex is there: the wait has no short limit, and after
/// [`LAUNCHER_GONE_MS`] the updater ends without starting one.
fn restart_launcher(session: &mut Session, layout: &Layout, product: &str) {
    let launcher = format!("{}\\{}", layout.exe_dir, install::LAUNCHER_NAME);
    log!("the launcher starts anew: it closes and {launcher} starts");
    session.leave();
    let mutex = format!("{product}_or_its_installer_is_already_running");
    let started = win::ticks();
    let mut said = false;
    while win::mutex_exists(&mutex) {
        let waited = win::ticks().saturating_sub(started);
        if waited >= LAUNCHER_GONE_MS {
            log!("the launcher did not close in an hour: no other one is started");
            return;
        }
        if !said && waited >= 15_000 {
            log!("the launcher has not closed yet (a box of it may be open): waiting for it");
            said = true;
        }
        win::sleep(100);
    }
    if said {
        log!("the launcher closed");
    }
    let command = format!("\"{launcher}\" -product_name={product}");
    if let Err(code) = win::start(&launcher, &command, &layout.exe_dir) {
        let message = text::restart_failed(&win::error_text(code));
        log!("FAILED: {message}");
        message_box(&message);
    }
}

fn main() {
    std::panic::set_hook(Box::new(fatal_panic));
    let code = run();
    unsafe { ExitProcess(code) }
}
