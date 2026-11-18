//! A `log` backend that writes every record to stdout and to **one file per
//! server**, so the unified process stays as inspectable as the old one-tab-
//! per-server setup.
//!
//! Each record is routed by its `target()` — the `log!` macros default the target
//! to the module path, which starts with the originating crate name
//! (`login_server`, `lobby_server`, `match_server`, `web_server`). Anything
//! else (including this `srp` orchestrator) goes to `srp.log`.
//!
//! The files are in `SRP_LOG_DIR` (default `logs`; `off`: stdout only), as
//! `<server>.log`, appended to across restarts: a container's own output is
//! gone when the container is made anew, which every update does, so the
//! compose file puts them in the data volume. A file that reaches
//! [`MAX_FILE_BYTES`] becomes `<server>.log.1` (the `.1` before it `.2`, and
//! so on); [`KEPT_FILES`] files per server are kept, the oldest deleted.
//!
//! What a line costs: stdout as before (one write of the line), and for the
//! file a copy into an 8 KiB buffer, an add and a compare for the size. The
//! buffer goes to the file when full, at a warning or an error, at the first
//! line a second after the last write, and at exit (Ctrl+C, SIGTERM): no
//! thread, no fsync. A process killed outright loses at most the last
//! second's info lines of the file (stdout has them).

use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use log::{Level, LevelFilter, Log, Metadata, Record};

/// Each server's file: its name, the size it is rotated at, and how many
/// files of it are kept (`<server>.log`, `.1` ...). About two days of each
/// at the heaviest seen (2026-11-01 to 11-02, 116-127 MB a day in all, of
/// it about 95 % match, 4 % lobby, 1.5 % srp, under 0.2 % login and web);
/// the lobby's about 12 days, the balance refit's input
/// (scripts/balance-refit). `memory` and `modules`: the match server's
/// lines of those targets, client patch 126's memory reports (about 2.5 KB
/// a player a match) and patch 125's module reports (about 2 KB), weeks of
/// them for `release/moderate memory` and `release/moderate modules`.
const SERVERS: [(&str, u64, u32); 7] = [
    ("login", 2 << 20, 4),
    ("lobby", 4 << 20, 16),
    ("match", 8 << 20, 32),
    ("web", 2 << 20, 2),
    ("srp", 4 << 20, 4),
    ("modules", 4 << 20, 16),
    ("memory", 4 << 20, 16),
];

/// Where the files are: unset for `logs`, empty or `off` for nowhere.
const LOG_DIR_VAR: &str = "SRP_LOG_DIR";
const DEFAULT_LOG_DIR: &str = "logs";
/// The buffer of a file is written out at a line this long after the last.
const FLUSH_AFTER: Duration = Duration::from_secs(1);
const SECONDS_PER_DAY: u64 = 86_400;

struct FileLogger {
    level: LevelFilter,
    files: HashMap<&'static str, Mutex<ServerLog>>,
}

/// One server's file and the ones rotated out of it.
struct ServerLog {
    /// `<dir>/<server>.log`; the older ones are this with `.1`, `.2`…
    path: PathBuf,
    max_bytes: u64,
    kept: u32,
    /// `None` until the file is opened, and after it could not be written
    /// (it is opened again at the next line).
    open: Option<OpenLog>,
}

struct OpenLog {
    writer: BufWriter<File>,
    /// The file's length, with what the buffer holds.
    bytes: u64,
    /// When the buffer was last written out (since the epoch).
    flushed: Duration,
}

impl ServerLog {
    fn new(dir: &Path, server: &str, max_bytes: u64, kept: u32) -> Self {
        Self {
            path: dir.join(format!("{server}.log")),
            max_bytes,
            kept,
            open: None,
        }
    }

    fn rotated(&self, n: u32) -> PathBuf {
        let mut path = self.path.clone().into_os_string();
        path.push(format!(".{n}"));
        path.into()
    }

    /// `line` and a newline into the file. A file that cannot be opened or
    /// written is tried again at the next line; the logger never fails for
    /// it (stdout has the line all the same).
    fn write(&mut self, now: Duration, line: &str, urgent: bool) {
        let len = line.len() as u64 + 1;
        if self.open.is_none() {
            self.open = self.open_file(now);
        }
        if self
            .open
            .as_ref()
            .is_some_and(|open| open.bytes > 0 && open.bytes + len > self.max_bytes)
        {
            self.rotate();
            self.open = self.open_file(now);
        }
        let Some(open) = self.open.as_mut() else {
            return;
        };
        let written = open
            .writer
            .write_all(line.as_bytes())
            .and_then(|()| open.writer.write_all(b"\n"));
        open.bytes += len;
        let due = urgent || now.saturating_sub(open.flushed) >= FLUSH_AFTER;
        if written.is_err() || (due && open.writer.flush().is_err()) {
            self.open = None;
        } else if due {
            open.flushed = now;
        }
    }

    fn open_file(&self, now: Duration) -> Option<OpenLog> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir).ok()?;
        }
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .ok()?;
        let bytes = file.metadata().map_or(0, |metadata| metadata.len());
        Some(OpenLog {
            writer: BufWriter::new(file),
            bytes,
            flushed: now,
        })
    }

    /// The file becomes `.1`, each `.n` `.n+1`; the last kept one goes.
    fn rotate(&mut self) {
        if let Some(mut open) = self.open.take() {
            let _ = open.writer.flush();
        }
        for n in (1..self.kept).rev() {
            let from = if n == 1 {
                self.path.clone()
            } else {
                self.rotated(n - 1)
            };
            let _ = std::fs::rename(from, self.rotated(n));
        }
        if self.kept <= 1 {
            let _ = std::fs::remove_file(&self.path);
        }
    }

    fn flush(&mut self) {
        if let Some(open) = self.open.as_mut() {
            let _ = open.writer.flush();
        }
    }
}

/// `SRP_LOG_DIR`: a directory, or `None` for stdout only.
fn log_dir(value: Option<&str>) -> Option<PathBuf> {
    let value = value.map_or(DEFAULT_LOG_DIR, str::trim);
    (!value.is_empty() && !value.eq_ignore_ascii_case("off")).then(|| Path::new(value).to_owned())
}

/// Install the file-routing logger. `level` is the maximum level emitted.
pub fn init(level: LevelFilter) -> io::Result<()> {
    let files = log_dir(std::env::var(LOG_DIR_VAR).ok().as_deref())
        .map(|dir| {
            SERVERS
                .into_iter()
                .map(|(server, max_bytes, kept)| {
                    let log = ServerLog::new(&dir, server, max_bytes, kept);
                    (server, Mutex::new(log))
                })
                .collect()
        })
        .unwrap_or_default();

    log::set_max_level(level);
    log::set_boxed_logger(Box::new(FileLogger { level, files })).map_err(io::Error::other)
}

/// `HH:MM:SS.mmmZ` (UTC) for stdout's lines. Time of day is enough to
/// correlate them with each other and with the client's own log, and
/// `docker logs -t` adds the date.
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

/// `YYYY-MM-DD` (UTC) for the files' lines, which span days.
fn utc_date(since_epoch: Duration) -> String {
    let (year, month, day) = civil_from_days(since_epoch.as_secs() / SECONDS_PER_DAY);
    format!("{year:04}-{month:02}-{day:02}")
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

/// Which server's log a record belongs to, decided from its target prefix
/// (a client report's kind its own file: `memory`, `modules`).
fn server_for_target(target: &str) -> &'static str {
    if target == "memory" {
        "memory"
    } else if target == "modules" {
        "modules"
    } else if target.starts_with("login_server") {
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
        println!("{time} [{server}] [{level}] {args}");

        // ... while each server's file holds only its own lines, dated.
        if let Some(file) = self.files.get(server) {
            let line = format!("{}T{time} [{level}] {args}", utc_date(since_epoch));
            file.lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .write(since_epoch, &line, level <= Level::Warn);
        }
    }

    fn flush(&self) {
        for file in self.files.values() {
            file.lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .flush();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A directory of this test's own, empty.
    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("srp-logger-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn names(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn client_reports_go_to_their_own_files() {
        for kind in ["memory", "modules"] {
            assert_eq!(server_for_target(kind), kind);
            assert!(SERVERS.iter().any(|&(server, _, _)| server == kind));
        }
        assert_eq!(server_for_target("match_server::v026g::game"), "match");
    }

    #[test]
    fn a_day_number_is_a_date() {
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
        }
        assert_eq!(
            utc_date(Duration::from_secs(20_738 * SECONDS_PER_DAY + 5)),
            "2026-10-12"
        );
    }

    #[test]
    fn the_directory_is_read() {
        assert_eq!(log_dir(None), Some(PathBuf::from("logs")));
        assert_eq!(log_dir(Some("")), None);
        assert_eq!(log_dir(Some(" OFF ")), None);
        assert_eq!(
            log_dir(Some(" /data/logs ")),
            Some(PathBuf::from("/data/logs"))
        );
    }

    /// The size cap and the count: a file is rotated before a line would
    /// take it past the cap, the oldest goes past the count, a restart adds
    /// to the file it finds, and nothing that is not ours is touched.
    #[test]
    fn files_are_rotated_at_the_cap_and_only_so_many_kept() {
        let dir = temp_dir("rotate");
        let logs = dir.join("logs");
        let read = |name: &str| std::fs::read_to_string(logs.join(name)).unwrap();
        let line = |n: u32| format!("line {n:03}"); // 8 bytes and a newline
        let at = Duration::from_secs(1);

        // Two lines (18 bytes) fit under 20; a third makes a new file.
        let mut log = ServerLog::new(&logs, "lobby", 20, 3);
        for n in 0..2 {
            log.write(at, &line(n), false);
        }
        // The buffer holds them until a second has gone, a warning comes or
        // the file is rotated.
        assert_eq!(read("lobby.log"), "");
        log.write(at, &line(2), false);
        assert_eq!(read("lobby.log.1"), "line 000\nline 001\n");
        assert_eq!(read("lobby.log"), "");
        log.write(at + FLUSH_AFTER, &line(3), false);
        assert_eq!(read("lobby.log"), "line 002\nline 003\n");

        // The process ends; another adds to the file it finds and counts
        // what is in it.
        log.flush();
        drop(log);
        std::fs::write(logs.join("srp.db"), "not a log").unwrap();
        let mut log = ServerLog::new(&logs, "lobby", 20, 3);
        for n in 4..9 {
            log.write(at, &line(n), true);
        }
        assert_eq!(read("lobby.log"), "line 008\n");
        assert_eq!(read("lobby.log.1"), "line 006\nline 007\n");
        assert_eq!(read("lobby.log.2"), "line 004\nline 005\n");
        assert_eq!(
            names(&logs),
            ["lobby.log", "lobby.log.1", "lobby.log.2", "srp.db"]
        );

        // A line longer than the cap still goes in, alone in its file.
        let long = "x".repeat(30);
        log.write(at, &long, true);
        log.write(at, &line(9), true);
        assert_eq!(read("lobby.log.1"), format!("{long}\n"));
        assert_eq!(read("lobby.log"), "line 009\n");
        assert_eq!(names(&logs).len(), 4);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_directory_that_cannot_be_made_loses_the_file_not_the_logger() {
        let dir = temp_dir("blocked");
        let blocked = dir.join("file");
        std::fs::write(&blocked, "in the way").unwrap();
        let mut log = ServerLog::new(&blocked.join("logs"), "match", 20, 3);
        log.write(Duration::ZERO, "lost", true);
        log.write(Duration::ZERO, "lost too", true);
        assert!(log.open.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
