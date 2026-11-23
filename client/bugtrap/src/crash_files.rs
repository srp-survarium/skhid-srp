// The reporter's own files in a folder, and which of them go: a folder keeps
// the newest KEEP crashes. Pure: no OS calls and no allocation, so the host
// tests include it (tests/crash_files_tests.rs) and a crashed game's heap is
// never touched.

/// How many crashes a folder keeps, not counting this process's own.
pub const KEEP: usize = 10;
/// The most files of ours looked at in one folder at one crash: past it the
/// rest wait for a later crash (fewer seen never makes a kept crash go).
pub const MOST: usize = 512;

/// One crash's name: `SRP-crash-YYYYMMDD-HHMMSS-mmm-p<pid>-t<tid>-<n>`, its
/// files the name and an extension (`dump::name`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct CrashId {
    /// `YYYYMMDDHHMMSSmmm`, ASCII digits.
    stamp: [u8; 17],
    pub process: u32,
    thread: u32,
    sequence: u32,
}

/// The files the reporter writes under a crash's name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Part {
    Dump,
    Report,
    Zip,
    /// A ZIP being written; left only when the game died while zipping.
    PartialZip,
}

impl Part {
    const ALL: [Part; 4] = [Part::Dump, Part::Report, Part::Zip, Part::PartialZip];
    pub fn extension(self) -> &'static str {
        match self {
            Part::Dump => ".dmp",
            Part::Report => ".txt",
            Part::Zip => ".zip",
            Part::PartialZip => ".zip.part",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CrashFile {
    pub id: CrashId,
    pub part: Part,
}

const PREFIX: &[u8] = b"SRP-crash-";

/// A file name the reporter wrote, exactly: anything else is not ours.
pub fn parse(name: &[u16]) -> Option<CrashFile> {
    let mut rest = name;
    skip(&mut rest, PREFIX)?;
    let mut stamp = [0u8; 17];
    let mut at = 0;
    for (i, width) in [8, 6, 3].into_iter().enumerate() {
        if i > 0 {
            skip(&mut rest, b"-")?;
        }
        for _ in 0..width {
            let c = *rest.first()?;
            if !is_digit(c) {
                return None;
            }
            stamp[at] = c as u8;
            at += 1;
            rest = &rest[1..];
        }
    }
    let process = number(&mut rest, b"-p")?;
    let thread = number(&mut rest, b"-t")?;
    let sequence = number(&mut rest, b"-")?;
    let part = Part::ALL.into_iter().find(|part| {
        let mut end = rest;
        skip(&mut end, part.extension().as_bytes()).is_some() && end.is_empty()
    })?;
    Some(CrashFile {
        id: CrashId {
            stamp,
            process,
            thread,
            sequence,
        },
        part,
    })
}

fn is_digit(c: u16) -> bool {
    (u16::from(b'0')..=u16::from(b'9')).contains(&c)
}

/// `ascii` at the start of `rest`, taken off it.
fn skip(rest: &mut &[u16], ascii: &[u8]) -> Option<()> {
    let matches = rest.len() >= ascii.len() && rest.iter().zip(ascii).all(|(&w, &a)| w == u16::from(a));
    matches.then(|| *rest = &rest[ascii.len()..])
}

/// `lead` then a decimal u32 as `{}` writes it: no sign, no leading zero.
fn number(rest: &mut &[u16], lead: &[u8]) -> Option<u32> {
    skip(rest, lead)?;
    let digits = rest.iter().take_while(|&&c| is_digit(c)).count();
    if digits == 0 || digits > 10 || (digits > 1 && rest[0] == u16::from(b'0')) {
        return None;
    }
    let mut value = 0u64;
    for &c in &rest[..digits] {
        value = value * 10 + u64::from(c - u16::from(b'0'));
    }
    *rest = &rest[digits..];
    u32::try_from(value).ok()
}

impl core::fmt::Display for CrashFile {
    /// The name back, as the reporter writes it.
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        let s = &self.id.stamp;
        let digits = |range: core::ops::Range<usize>| core::str::from_utf8(&s[range]).unwrap_or("");
        write!(
            f,
            "SRP-crash-{}-{}-{}-p{}-t{}-{}{}",
            digits(0..8),
            digits(8..14),
            digits(14..17),
            self.id.process,
            self.id.thread,
            self.id.sequence,
            self.part.extension()
        )
    }
}

#[derive(Clone, Copy)]
pub struct Found {
    pub file: CrashFile,
    /// Its last write (FILETIME ticks).
    pub written: u64,
}

#[derive(Clone, Copy)]
struct Crash {
    newest: u64,
    id: CrashId,
}

/// A folder's files of ours, as listed once.
pub struct Folder {
    files: [Found; MOST],
    len: usize,
    crashes: [Crash; MOST],
}

impl Folder {
    pub const fn new() -> Self {
        const NONE: Found = Found {
            file: CrashFile {
                id: CrashId {
                    stamp: [0; 17],
                    process: 0,
                    thread: 0,
                    sequence: 0,
                },
                part: Part::Dump,
            },
            written: 0,
        };
        Self {
            files: [NONE; MOST],
            len: 0,
            crashes: [Crash {
                newest: 0,
                id: NONE.file.id,
            }; MOST],
        }
    }

    /// A listed file: kept when its name is ours and there is room.
    pub fn add(&mut self, name: &[u16], written: u64) {
        if let (Some(file), true) = (parse(name), self.len < MOST) {
            self.files[self.len] = Found { file, written };
            self.len += 1;
        }
    }

    /// The files that go, each once: every file of a crash past the newest
    /// [`KEEP`] (a crash dated by its newest file, a `.zip.part` aside), and
    /// every `.zip.part`; never one of `this_process`.
    pub fn going(&mut self, this_process: u32) -> impl Iterator<Item = &Found> {
        let files = &mut self.files[..self.len];
        files.sort_unstable_by_key(|found| found.file.id);
        let mut crashes = 0;
        for found in files.iter() {
            if found.file.id.process == this_process || found.file.part == Part::PartialZip {
                continue;
            }
            if crashes > 0 && self.crashes[crashes - 1].id == found.file.id {
                let crash = &mut self.crashes[crashes - 1];
                crash.newest = crash.newest.max(found.written);
            } else {
                self.crashes[crashes] = Crash {
                    newest: found.written,
                    id: found.file.id,
                };
                crashes += 1;
            }
        }
        let crashes = &mut self.crashes[..crashes];
        crashes.sort_unstable_by_key(|crash| core::cmp::Reverse((crash.newest, crash.id)));
        let old = &crashes[crashes.len().min(KEEP)..];
        self.files[..self.len].iter().filter(move |found| {
            found.file.id.process != this_process
                && (found.file.part == Part::PartialZip
                    || old.iter().any(|crash| crash.id == found.file.id))
        })
    }
}
