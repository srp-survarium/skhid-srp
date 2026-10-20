//! A `log` backend that fans records out to **one file per server** (plus
//! stdout), so the unified process stays as inspectable as the old one-tab-per-
//! server setup.
//!
//! Each record is routed by its `target()` — the `log!` macros default the target
//! to the module path, which starts with the originating crate name
//! (`login_server`, `lobby_server`, `match_server`, `web_server`). Anything
//! else (including this `srp` orchestrator) goes to `srp.log`.
//!
//! The bootstrap scripts open a tab per `logs/<server>.log` and tail it, so logs
//! remain separable even though everything runs in one process.
//!
//! `SRP_LOG_DIR=<dir>` also keeps everything stdout gets in one file per UTC
//! day, `<dir>/srp-YYYY-MM-DD.log`, appended to across restarts: a container's
//! own output is gone when the container is made anew, which every update
//! does. Files older than `SRP_LOG_KEEP_DAYS` (default 14) are deleted.

use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use log::{LevelFilter, Log, Metadata, Record};

const SERVERS: [&str; 5] = ["login", "lobby", "match", "web", "srp"];

/// Where the log of each day is kept: unset, empty or `off` for nowhere.
const LOG_DIR_VAR: &str = "SRP_LOG_DIR";
/// How many days of those files are kept.
const LOG_KEEP_DAYS_VAR: &str = "SRP_LOG_KEEP_DAYS";
const DEFAULT_LOG_KEEP_DAYS: u64 = 14;
const SECONDS_PER_DAY: u64 = 86_400;

struct FileLogger {
    level: LevelFilter,
    files: HashMap<&'static str, Mutex<File>>,
    days: Option<Mutex<DayLog>>,
}

/// The log kept across restarts: one file per UTC day in `dir`, each line
/// as stdout has it.
struct DayLog {
    dir: PathBuf,
    keep_days: u64,
    /// The day (since 1970-01-01) `file` is of.
    day: u64,
    file: Option<File>,
}

impl DayLog {
    fn new(dir: PathBuf, keep_days: u64) -> Self {
        Self {
            dir,
            keep_days,
            day: 0,
            file: None,
        }
    }

    fn file_name(day: u64) -> String {
        let (year, month, date) = civil_from_days(day);
        format!("srp-{year:04}-{month:02}-{date:02}.log")
    }

    /// The day a file of ours is of; `None` for any other name.
    fn day_of(name: &str) -> Option<u64> {
        let date = name.strip_prefix("srp-")?.strip_suffix(".log")?;
        let mut parts = date.split('-');
        let year = parts.next().filter(|part| part.len() == 4)?.parse().ok()?;
        let month = parts.next().filter(|part| part.len() == 2)?.parse().ok()?;
        let day = parts.next().filter(|part| part.len() == 2)?.parse().ok()?;
        if parts.next().is_some() || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
            return None;
        }
        days_from_civil(year, month, day)
    }

    /// `line` into the file of the day `since_epoch` is in. A file that
    /// cannot be opened or written is tried again at the next line; the
    /// logger never fails for it (stdout has the line all the same).
    fn write(&mut self, since_epoch: Duration, line: &str) {
        let day = since_epoch.as_secs() / SECONDS_PER_DAY;
        if self.file.is_none() || day != self.day {
            self.file = self.open(day);
            self.day = day;
        }
        if let Some(file) = self.file.as_mut()
            && writeln!(file, "{line}").is_err()
        {
            self.file = None;
        }
    }

    fn open(&self, day: u64) -> Option<File> {
        std::fs::create_dir_all(&self.dir).ok()?;
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.dir.join(Self::file_name(day)))
            .ok()?;
        self.prune(day);
        Some(file)
    }

    /// Delete our files of days more than `keep_days` before `today`.
    fn prune(&self, today: u64) {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return;
        };
        for entry in entries.flatten() {
            let old = entry
                .file_name()
                .to_str()
                .and_then(Self::day_of)
                .is_some_and(|day| day + self.keep_days < today);
            if old {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
}

/// The year, month and day of the month of a day counted from 1970-01-01
/// (the proleptic Gregorian calendar; Howard Hinnant's `civil_from_days`).
fn civil_from_days(days: u64) -> (u64, u64, u64) {
    let shifted = days + 719_468;
    let era = shifted / 146_097;
    let day_of_era = shifted % 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_shifted = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_shifted + 2) / 5 + 1;
    let month = if month_shifted < 10 {
        month_shifted + 3
    } else {
        month_shifted - 9
    };
    let year = year_of_era + era * 400 + u64::from(month <= 2);
    (year, month, day)
}

/// The other way; `None` before 1970.
fn days_from_civil(year: u64, month: u64, day: u64) -> Option<u64> {
    let year = if month <= 2 { year.checked_sub(1)? } else { year };
    let era = year / 400;
    let year_of_era = year % 400;
    let month_shifted = if month > 2 { month - 3 } else { month + 9 };
    let day_of_year = (153 * month_shifted + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    (era * 146_097 + day_of_era).checked_sub(719_468)
}

/// `SRP_LOG_DIR`, if it names a directory.
fn log_dir(value: Option<&str>) -> Option<PathBuf> {
    let value = value?.trim();
    (!value.is_empty() && !value.eq_ignore_ascii_case("off")).then(|| Path::new(value).to_owned())
}

/// `SRP_LOG_KEEP_DAYS`: a number of days, at least 1.
fn log_keep_days(value: Option<&str>) -> u64 {
    value
        .and_then(|value| value.trim().parse().ok())
        .filter(|days| *days >= 1)
        .unwrap_or(DEFAULT_LOG_KEEP_DAYS)
}

/// Install the file-routing logger. `level` is the maximum level emitted; the
/// `logs/` directory and one `<server>.log` per server are created/appended.
pub fn init(level: LevelFilter) -> io::Result<()> {
    let mut files = HashMap::new();
    // `SRP_LOG_FILES=off`: stdout only, for a container whose runtime keeps
    // and rotates the output.
    let to_files =
        !std::env::var("SRP_LOG_FILES").is_ok_and(|value| value.trim().eq_ignore_ascii_case("off"));
    if to_files {
        std::fs::create_dir_all("logs")?;
    }
    for name in SERVERS.into_iter().filter(|_| to_files) {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(format!("logs/{name}.log"))?;
        files.insert(name, Mutex::new(file));
    }

    let days = log_dir(std::env::var(LOG_DIR_VAR).ok().as_deref()).map(|dir| {
        let keep = log_keep_days(std::env::var(LOG_KEEP_DAYS_VAR).ok().as_deref());
        Mutex::new(DayLog::new(dir, keep))
    });

    log::set_max_level(level);
    log::set_boxed_logger(Box::new(FileLogger { level, files, days }))
        .map_err(io::Error::other)
}

/// `HH:MM:SS.mmmZ` (UTC) for log lines. Time of day is enough to correlate the
/// per-server files with each other and with the client's own log, and keeps the
/// logger free of a date/time-zone dependency. (The files of `SRP_LOG_DIR`
/// have the date in their names.)
fn utc_time_of_day(since_epoch: Duration) -> String {
    let seconds = since_epoch.as_secs() % SECONDS_PER_DAY;
    format!(
        "{:02}:{:02}:{:02}.{:03}Z",
        seconds / 3600,
        seconds / 60 % 60,
        seconds % 60,
        since_epoch.subsec_millis()
    )
}

/// Which server's log a record belongs to, decided from its target prefix.
fn server_for_target(target: &str) -> &'static str {
    if target.starts_with("login_server") {
        "login"
    } else if target.starts_with("lobby_server") || target.starts_with("messaging") {
        "lobby"
    } else if target.starts_with("match_server") {
        "match"
    } else if target.starts_with("web_server") {
        "web"
    } else {
        "srp"
    }
}

impl Log for FileLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let server = server_for_target(record.target());
        let level = record.level();
        let args = record.args();
        let since_epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let time = utc_time_of_day(since_epoch);

        // stdout keeps a combined view (prefixed with the server) ...
        let line = format!("{time} [{server}] [{level}] {args}");
        println!("{line}");

        // ... which `SRP_LOG_DIR` keeps across restarts ...
        if let Some(days) = &self.days {
            days.lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .write(since_epoch, &line);
        }

        // ... while each server's file holds only its own lines.
        if let Some(file) = self.files.get(server) {
            let mut file = file
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let _ = writeln!(file, "{time} [{level}] {args}");
        }
    }

    fn flush(&self) {
        for file in self.files.values() {
            let _ = file
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .flush();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(day: u64, second: u64) -> Duration {
        Duration::from_secs(day * SECONDS_PER_DAY + second)
    }

    /// A directory of this test's own, empty.
    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("srp-logger-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_day_number_is_a_date_and_back() {
        for (day, date) in [
            (0, (1970, 1, 1)),
            (58, (1970, 2, 28)),
            (59, (1970, 3, 1)),
            (365, (1971, 1, 1)),
            (11_016, (2000, 2, 29)),
            (11_017, (2000, 3, 1)),
            (19_723, (2024, 1, 1)),
            (19_782, (2024, 2, 29)),
            (20_088, (2024, 12, 31)),
            (20_738, (2026, 10, 12)),
            (47_540, (2100, 2, 28)),
            (47_541, (2100, 3, 1)),
        ] {
            assert_eq!(civil_from_days(day), date, "{day}");
            assert_eq!(days_from_civil(date.0, date.1, date.2), Some(day), "{date:?}");
        }
        // Every day of 60 years comes back as itself.
        for day in 0..22_000 {
            let (year, month, date) = civil_from_days(day);
            assert_eq!(days_from_civil(year, month, date), Some(day));
        }
        assert_eq!(days_from_civil(1969, 12, 31), None);
    }

    #[test]
    fn only_our_own_file_names_are_days() {
        assert_eq!(DayLog::file_name(20_738), "srp-2026-10-12.log");
        assert_eq!(DayLog::day_of("srp-2026-10-12.log"), Some(20_738));
        for other in [
            "srp.db",
            "srp-2026-10-12.log.gz",
            "srp-2026-10-12.txt",
            "srp-2026-13-01.log",
            "srp-2026-10-00.log",
            "srp-2026-1-12.log",
            "srp-2026-10-12-1.log",
            "srp-1969-12-31.log",
            "match-2026-10-12.log",
        ] {
            assert_eq!(DayLog::day_of(other), None, "{other}");
        }
    }

    #[test]
    fn the_settings_are_read() {
        assert_eq!(log_dir(None), None);
        assert_eq!(log_dir(Some("")), None);
        assert_eq!(log_dir(Some(" OFF ")), None);
        assert_eq!(log_dir(Some(" /data/logs ")), Some(PathBuf::from("/data/logs")));
        assert_eq!(log_keep_days(None), DEFAULT_LOG_KEEP_DAYS);
        assert_eq!(log_keep_days(Some("0")), DEFAULT_LOG_KEEP_DAYS);
        assert_eq!(log_keep_days(Some("many")), DEFAULT_LOG_KEEP_DAYS);
        assert_eq!(log_keep_days(Some(" 3 ")), 3);
    }

    /// What a restart must not lose: a second logger on the same directory
    /// adds to the day's file; a new day gets a new file; files older than
    /// the days kept go, and nothing that is not ours.
    #[test]
    fn a_day_has_one_file_which_restarts_add_to_and_old_days_go() {
        let dir = temp_dir("days");
        let logs = dir.join("logs");
        let read = |name: &str| std::fs::read_to_string(logs.join(name)).unwrap();
        let names = || {
            let mut names: Vec<String> = std::fs::read_dir(&logs)
                .unwrap()
                .map(|entry| entry.unwrap().file_name().into_string().unwrap())
                .collect();
            names.sort();
            names
        };

        let mut log = DayLog::new(logs.clone(), 2);
        log.write(at(20_738, 10), "one");
        log.write(at(20_738, 86_399), "two");
        // The process ends and another starts on the same day.
        drop(log);
        let mut log = DayLog::new(logs.clone(), 2);
        log.write(at(20_738, 86_399), "three");
        assert_eq!(read("srp-2026-10-12.log"), "one\ntwo\nthree\n");

        // Midnight: the next line is the next day's.
        log.write(at(20_739, 0), "four");
        assert_eq!(read("srp-2026-10-12.log"), "one\ntwo\nthree\n");
        assert_eq!(read("srp-2026-10-13.log"), "four\n");

        std::fs::write(logs.join("srp.db"), "not a log").unwrap();
        std::fs::write(logs.join("srp-2026-10-01.log.keep"), "not ours").unwrap();
        log.write(at(20_740, 5), "five");
        assert_eq!(names().len(), 5);
        // Two days kept besides today's: the 12th goes on the 15th.
        log.write(at(20_741, 5), "six");
        assert_eq!(
            names(),
            [
                "srp-2026-10-01.log.keep",
                "srp-2026-10-13.log",
                "srp-2026-10-14.log",
                "srp-2026-10-15.log",
                "srp.db"
            ]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_directory_that_cannot_be_made_loses_the_file_not_the_logger() {
        let dir = temp_dir("blocked");
        let blocked = dir.join("file");
        std::fs::write(&blocked, "in the way").unwrap();
        let mut log = DayLog::new(blocked.join("logs"), 2);
        log.write(at(20_738, 1), "lost");
        log.write(at(20_738, 2), "lost too");
        assert!(log.file.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
