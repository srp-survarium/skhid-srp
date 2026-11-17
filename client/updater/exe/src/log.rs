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

/// Makes `folder`, moves into the `logs` folder the run files earlier
/// updaters and games left beside the updater ([`gather`]), then starts
/// this run's file in `folder`. Once a run.
fn start_in(layout: &Layout, folder: &str) {
    if !matches!(unsafe { &*LOG.0.get() }, State::Off) {
        return;
    }
    let _ = win::create_folders(folder);
    let moved = gather(layout);
    start_file(folder);
    if moved > 0 {
        line(&format!("moved {moved} earlier runs' logs from {} into {}", layout.exe_dir, layout.logs_dir));
    }
}

/// The run files beside the updater (`run_log::moves_to_folder`) and in
/// its old seeding folder, moved into the `logs` folder and its seeding
/// folder, the old seeding folder removed once empty: how many moved. A
/// file that does not move (its name taken there, held without sharing)
/// stays where it is.
fn gather(layout: &Layout) -> usize {
    let mut moved = 0;
    let mut bring = |from: &str, to: &str, name: &str| {
        if win::rename(&format!("{from}\\{name}"), &format!("{to}\\{name}"), false).is_ok() {
            moved += 1;
        }
    };
    let x86 = &layout.exe_dir;
    for name in win::list(x86, "*.log").iter().filter(|name| run_log::moves_to_folder(name)) {
        bring(x86, &layout.logs_dir, name);
    }
    let old_seed = format!("{x86}\\{}", run_log::OLD_SEED_FOLDER);
    if win::is_folder(&old_seed) {
        let seed = format!("{}\\{}", layout.logs_dir, run_log::SEED_FOLDER);
        let _ = win::create_folders(&seed);
        for name in win::list(&old_seed, run_log::PATTERN).iter().filter(|name| RunName::parse(name).is_some()) {
            bring(&old_seed, &seed, name);
        }
        win::remove_dir(&old_seed);
    }
    moved
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
