//! The updater's log, a file of its own each run: `updater_skhid.<stamp>.log`
//! beside the updater, `<stamp>` the run's start in local time
//! (`2026-11-15_14-03-07`), `_2` .. `_99` after it when a run of the same
//! second made that name first. A run's file is made new and only appended
//! to; an old run's file is only ever deleted, never written over: when a
//! run starts, the oldest go while 100 or more, or 500 MiB or more, are
//! there. The legacy `updater_skhid.log` is not a run's: never read,
//! written or deleted here.

use alloc::{format, string::String, vec::Vec};

/// Every run file's name starts with this.
pub const PREFIX: &str = "updater_skhid.";
/// FindFirstFileW's pattern for the candidates (each parsed before use).
pub const PATTERN: &str = "updater_skhid.*.log";
/// The runs kept with the new one, at most.
pub const KEEP_RUNS: usize = 100;
/// The bytes the old runs may hold before the new one starts.
pub const KEEP_BYTES: u64 = 500 << 20;
/// A run's own file stops growing here (one last line says so).
pub const RUN_LIMIT: u64 = 16 << 20;
/// The last suffix tried for a run of one second.
const LAST: u8 = 99;

/// A run's start, local time, to the second.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Stamp {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

impl Stamp {
    /// `2026-11-15_14-03-07`.
    pub fn text(&self) -> String {
        format!(
            "{:04}-{:02}-{:02}_{:02}-{:02}-{:02}",
            self.year, self.month, self.day, self.hour, self.minute, self.second
        )
    }

    /// The 19 bytes of [`Stamp::text`]; none for anything else.
    fn parse(text: &[u8]) -> Option<Stamp> {
        let [y0, y1, y2, y3, b'-', m0, m1, b'-', d0, d1, b'_', h0, h1, b'-', n0, n1, b'-', s0, s1] =
            *text
        else {
            return None;
        };
        let digit = |b: u8| b.is_ascii_digit().then(|| u16::from(b - b'0'));
        let two = |a, b| Some((digit(a)? * 10 + digit(b)?) as u8);
        Some(Stamp {
            year: digit(y0)? * 1000 + digit(y1)? * 100 + digit(y2)? * 10 + digit(y3)?,
            month: two(m0, m1)?,
            day: two(d0, d1)?,
            hour: two(h0, h1)?,
            minute: two(n0, n1)?,
            second: two(s0, s1)?,
        })
    }
}

/// A run's file: its start and its number among the runs of that second
/// (1: none written, the first; 2 to 99: `_<n>`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct RunName {
    pub stamp: Stamp,
    pub n: u8,
}

impl RunName {
    pub fn file_name(&self) -> String {
        match self.n {
            1 => format!("{PREFIX}{}.log", self.stamp.text()),
            n => format!("{PREFIX}{}_{n}.log", self.stamp.text()),
        }
    }

    /// A name as [`RunName::file_name`] writes it (the prefix and `.log` in
    /// any case, as Windows matches them); none for any other name.
    pub fn parse(name: &str) -> Option<RunName> {
        let name = name.as_bytes();
        let (prefix, rest) = name.split_at_checked(PREFIX.len())?;
        let (rest, extension) = rest.split_at_checked(rest.len().checked_sub(4)?)?;
        if !prefix.eq_ignore_ascii_case(PREFIX.as_bytes()) || !extension.eq_ignore_ascii_case(b".log") {
            return None;
        }
        let (stamp, suffix) = rest.split_at_checked(19)?;
        let stamp = Stamp::parse(stamp)?;
        let n = match suffix {
            [] => 1,
            [b'_', d @ b'2'..=b'9'] => d - b'0',
            [b'_', a @ b'1'..=b'9', b @ b'0'..=b'9'] => (a - b'0') * 10 + (b - b'0'),
            _ => return None,
        };
        Some(RunName { stamp, n })
    }

    /// The names a run of `stamp` tries, in turn: past the highest number a
    /// run file of that second in `files` has, so a name a run of that
    /// second took is never taken again (even after it was deleted).
    pub fn candidates(stamp: Stamp, files: &[RunFile]) -> impl Iterator<Item = RunName> {
        let highest = files
            .iter()
            .filter_map(|file| RunName::parse(&file.name))
            .filter(|run| run.stamp == stamp)
            .map(|run| run.n)
            .max()
            .unwrap_or(0);
        (highest + 1..=LAST).map(move |n| RunName { stamp, n })
    }
}

/// A file of a folder that matches [`PATTERN`]: its name, size and when it
/// was last written (FILETIME, 100 ns since 1601).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunFile {
    pub name: String,
    pub size: u64,
    pub written: u64,
}

/// The run files of `files` (a folder's names that match [`PATTERN`]) to
/// delete before the run of `run` makes its own: the least recently written
/// first (not by the name's stamp: a clock set back would make the run
/// before it the oldest), while [`KEEP_RUNS`] or more or [`KEEP_BYTES`] or
/// more are there. Every run file counts; the files of `run`'s second (this
/// run's, or another's started with it) are never named, nor other names.
/// One Windows does not delete is passed over: the caller goes on with the
/// next.
pub fn to_delete(files: &[RunFile], run: Stamp) -> Vec<String> {
    let runs: Vec<(RunName, &RunFile)> =
        files.iter().filter_map(|file| Some((RunName::parse(&file.name)?, file))).collect();
    let mut total: u64 = runs.iter().map(|(_, file)| file.size).sum();
    let mut left = runs.len();
    let mut old: Vec<&(RunName, &RunFile)> = runs.iter().filter(|(name, _)| name.stamp != run).collect();
    old.sort_by_key(|(name, file)| (file.written, *name));
    let mut out = Vec::new();
    for (_, file) in old {
        if left < KEEP_RUNS && total < KEEP_BYTES {
            break;
        }
        out.push(file.name.clone());
        left -= 1;
        total -= file.size;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;

    const STAMP: Stamp = Stamp { year: 2026, month: 11, day: 15, hour: 14, minute: 3, second: 7 };

    #[test]
    fn names_are_the_start_and_a_number() {
        let first = RunName { stamp: STAMP, n: 1 };
        assert_eq!(first.file_name(), "updater_skhid.2026-11-15_14-03-07.log");
        assert_eq!(RunName { stamp: STAMP, n: 2 }.file_name(), "updater_skhid.2026-11-15_14-03-07_2.log");
        assert_eq!(RunName { stamp: STAMP, n: 99 }.file_name(), "updater_skhid.2026-11-15_14-03-07_99.log");
        for n in 1..=99 {
            let name = RunName { stamp: STAMP, n };
            assert_eq!(RunName::parse(&name.file_name()), Some(name));
        }
        assert_eq!(RunName::candidates(STAMP, &[]).count(), 99);
        assert_eq!(RunName::parse("UPDATER_SKHID.2026-11-15_14-03-07.LOG"), Some(first));
    }

    #[test]
    fn other_names_are_no_runs() {
        for name in [
            "updater_skhid.log",
            "updater_skhid.old.log",
            "updater_skhid.2026-11-15_14-03-07.log.part",
            "updater_skhid.2026-11-15_14-03-07_1.log",
            "updater_skhid.2026-11-15_14-03-07_02.log",
            "updater_skhid.2026-11-15_14-03-07_100.log",
            "updater_skhid.2026-11-15 14-03-07.log",
            "updater_skhid.2026-1x-15_14-03-07.log",
            "updater_skhid.2026-11-15_14-03-07.txt",
            "srp_patches.2026-11-15_14-03-07.log",
            "updater_skhid.",
            "",
        ] {
            assert_eq!(RunName::parse(name), None, "{name}");
        }
    }

    #[test]
    fn runs_sort_by_time_then_number() {
        let later = Stamp { second: 8, ..STAMP };
        let a = RunName { stamp: STAMP, n: 1 };
        let b = RunName { stamp: STAMP, n: 2 };
        let c = RunName { stamp: STAMP, n: 10 };
        let d = RunName { stamp: later, n: 1 };
        assert!(a < b && b < c && c < d);
        assert!(Stamp { year: 2027, month: 1, ..STAMP } > Stamp { month: 12, ..STAMP });
    }

    fn at(second: u32) -> Stamp {
        Stamp { minute: (second / 60) as u8, second: (second % 60) as u8, ..STAMP }
    }

    fn run(second: u32) -> String {
        RunName { stamp: at(second), n: 1 }.file_name()
    }

    /// A file written in the order of its stamp (its run's minute and second).
    fn file(name: String, size: u64) -> RunFile {
        let written = RunName::parse(&name)
            .map_or(0, |run| u64::from(run.stamp.minute) * 60 + u64::from(run.stamp.second));
        RunFile { name, size, written }
    }

    /// A run started after every file below.
    const NOW: Stamp = Stamp { hour: 15, ..STAMP };

    #[test]
    fn the_oldest_go_past_a_hundred_runs() {
        let files: Vec<RunFile> = (0..120).rev().map(|s| file(run(s), 1000)).collect();
        let gone = to_delete(&files, NOW);
        assert_eq!(gone, (0..21).map(run).collect::<Vec<_>>());
        let few: Vec<RunFile> = (0..99).map(|s| file(run(s), 1000)).collect();
        assert!(to_delete(&few, NOW).is_empty());
        let hundred: Vec<RunFile> = (0..100).map(|s| file(run(s), 1000)).collect();
        assert_eq!(to_delete(&hundred, NOW), [run(0)]);
    }

    #[test]
    fn the_oldest_go_past_500_mib() {
        let files = [file(run(3), 300 << 20), file(run(1), 100 << 20), file(run(2), 100 << 20)];
        assert_eq!(to_delete(&files, NOW), [run(1)]);
        let exact = [file(run(1), 250 << 20), file(run(2), 250 << 20)];
        assert_eq!(to_delete(&exact, NOW), [run(1)]);
        let under = [file(run(1), 250 << 20), file(run(2), (250 << 20) - 1)];
        assert!(to_delete(&under, NOW).is_empty());
        let huge = [file(run(1), 600 << 20)];
        assert_eq!(to_delete(&huge, NOW), [run(1)]);
    }

    #[test]
    fn other_files_are_never_counted_or_deleted() {
        let mut files: Vec<RunFile> = (0..99).map(|s| file(run(s), 1)).collect();
        files.push(RunFile { name: "updater_skhid.log".to_string(), size: 1 << 40, written: 0 });
        files.push(RunFile { name: "updater_skhid.notes.log".to_string(), size: 1, written: 0 });
        assert!(to_delete(&files, NOW).is_empty());
    }

    #[test]
    fn the_least_recently_written_go_first_never_this_seconds() {
        // the clock set back: the file written last has the earliest stamp
        let mut files: Vec<RunFile> = (1..100).map(|s| file(run(s), 1)).collect();
        files.push(RunFile { name: run(0), size: 1, written: 1000 });
        assert_eq!(to_delete(&files, NOW), [run(1)]);
        // this second's files are counted, never deleted
        let own = RunName { stamp: NOW, n: 1 }.file_name();
        let mut this: Vec<RunFile> = (0..99).map(|s| file(run(s), 1)).collect();
        this.push(RunFile { name: own.clone(), size: 600 << 20, written: 0 });
        let gone = to_delete(&this, NOW);
        assert_eq!(gone.len(), 99);
        assert!(!gone.contains(&own));
    }

    #[test]
    fn a_seconds_numbers_are_never_taken_again() {
        let first = |files: &[RunFile]| RunName::candidates(STAMP, files).next();
        assert_eq!(first(&[]), Some(RunName { stamp: STAMP, n: 1 }));
        // the second's first file deleted, its _2 there: the next is _3
        let two = [file(RunName { stamp: STAMP, n: 2 }.file_name(), 1), file(run(5), 1)];
        assert_eq!(first(&two), Some(RunName { stamp: STAMP, n: 3 }));
        let last = [file(RunName { stamp: STAMP, n: 99 }.file_name(), 1)];
        assert_eq!(first(&last), None);
    }
}
