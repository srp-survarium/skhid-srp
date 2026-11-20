//! The updater's log: a file of its own each run in the game's `logs`
//! folder, `updater_skhid.<start>.log` (`updater_logic::run_log`), one line
//! per step, so a player can send it when something went wrong. It holds no
//! account name and no password; it does hold the folder the game is in.

use alloc::format;
use alloc::string::String;
use core::cell::UnsafeCell;

use updater_logic::run_log::{self, RunName};

use crate::install::Layout;
use crate::win;

/// This run's log.
enum State {
    /// Before [`start`]: a line is dropped.
    Off,
    /// The run's file, and the bytes written to it.
    Writing { path: String, written: u64 },
    /// The run's file reached [`run_log::RUN_LIMIT`]: its last line says so.
    Full { path: String },
}

/// Written once at start, before anything else runs; the program has one
/// thread.
struct Log(UnsafeCell<State>);
unsafe impl Sync for Log {}
static LOG: Log = Log(UnsafeCell::new(State::Off));

/// Starts this run's file in the game's `logs` folder.
pub fn start(layout: &Layout) {
    start_in(layout, &layout.logs_dir);
}

/// Starts a seeding run's file in the `logs` folder's own folder of them
/// (`run_log::SEED_FOLDER`).
pub fn start_seeding(layout: &Layout) {
    start_in(layout, &format!("{}\\{}", layout.logs_dir, run_log::SEED_FOLDER));
}

/// Makes `folder`, deletes the logs earlier updaters and games left beside
/// the updater ([`clear_old`]), then starts this run's file in `folder`.
/// Once a run.
fn start_in(layout: &Layout, folder: &str) {
    if !matches!(unsafe { &*LOG.0.get() }, State::Off) {
        return;
    }
    let _ = win::create_folders(folder);
    let deleted = clear_old(layout);
    start_file(folder);
    if deleted > 0 {
        line(&format!("deleted {deleted} old logs in {}", layout.exe_dir));
    }
}

/// The logs beside the updater from before they went to the `logs` folder
/// (`run_log::old_beside_updater`) and its old seeding folder's runs,
/// deleted, the folder removed once empty: how many went. One Windows does
/// not delete (open) stays.
fn clear_old(layout: &Layout) -> usize {
    let mut deleted = 0;
    let mut delete = |folder: &str, name: &str| {
        let path = format!("{folder}\\{name}");
        if win::delete(&path).is_ok() {
            deleted += 1;
        }
    };
    let x86 = &layout.exe_dir;
    for name in win::list(x86, "*.log").iter().filter(|name| run_log::old_beside_updater(name)) {
        delete(x86, name);
    }
    let old_seed = format!("{x86}\\{}", run_log::OLD_SEED_FOLDER);
    if win::is_folder(&old_seed) {
        for name in win::list(&old_seed, run_log::PATTERN).iter().filter(|name| RunName::parse(name).is_some()) {
            delete(&old_seed, name);
        }
        win::remove_dir(&old_seed);
    }
    deleted
}

/// Starts this run's file in `folder` (the log [`State::Off`]): the least
/// recently written runs' files deleted first (`run_log::to_delete`), then
/// the next name of this second made.
fn start_file(folder: &str) {
    let state = unsafe { &mut *LOG.0.get() };
    let stamp = win::local_stamp();
    let files = win::list_run_files(folder, run_log::PATTERN);
    // one Windows refuses is passed over: the next is tried
    for name in run_log::to_delete(&files, stamp) {
        let _ = win::delete(&alloc::format!("{folder}\\{name}"));
    }
    let mut chosen = None;
    for run in RunName::candidates(stamp, &files) {
        let path = alloc::format!("{folder}\\{}", run.file_name());
        match win::create_new(&path) {
            Ok(false) => continue,
            // made; or the folder refuses files: the lines are dropped
            // as they were (no other name would do better)
            Ok(true) | Err(_) => {
                chosen = Some(path);
                break;
            }
        }
    }
    // all 99 names of this second taken: the last one is appended to
    let path = chosen.unwrap_or_else(|| {
        alloc::format!("{folder}\\{}", RunName { stamp, n: 99 }.file_name())
    });
    *state = State::Writing { path, written: 0 };
}

/// This run's file (its full path), once started.
pub fn path() -> Option<String> {
    match unsafe { &*LOG.0.get() } {
        State::Off => None,
        State::Writing { path, .. } | State::Full { path } => Some(path.clone()),
    }
}

/// Adds a line with the time before it. A log that cannot be written is no
/// reason to stop: the line is dropped.
pub fn line(text: &str) {
    let state = unsafe { &mut *LOG.0.get() };
    let State::Writing { path, written } = state else {
        return;
    };
    let line = alloc::format!("{} {}\r\n", win::timestamp(), text);
    if *written + line.len() as u64 > run_log::RUN_LIMIT {
        let last = alloc::format!(
            "{} the log is full ({} MiB): the rest of this run is not written\r\n",
            win::timestamp(),
            run_log::RUN_LIMIT >> 20
        );
        let _ = win::append_file(path, last.as_bytes());
        *state = State::Full { path: core::mem::take(path) };
        return;
    }
    if win::append_file(path, line.as_bytes()).is_ok() {
        *written += line.len() as u64;
    }
}

macro_rules! log {
    ($($argument:tt)*) => {
        $crate::log::line(&alloc::format!($($argument)*))
    };
}
pub(crate) use log;
