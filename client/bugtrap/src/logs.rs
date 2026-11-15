// Optional, bounded attachments. No game logging API or game heap is touched:
// the faulting thread may hold their locks.
use crate::{
    run_name::{Kind, RunName, parse},
    text::Wide,
    zip,
};
use core::{
    fmt::{self, Write},
    ptr::null_mut,
};
use windows_sys::Win32::{
    Foundation::*,
    Globalization::{CP_UTF8, MB_ERR_INVALID_CHARS, MultiByteToWideChar, WideCharToMultiByte},
    Storage::FileSystem::*,
    System::{
        Environment::GetCommandLineW, LibraryLoader::GetModuleFileNameW, SystemInformation::*,
    },
    UI::Shell::*,
};

pub const COUNT: usize = 32;
pub const MAX_MEMBERS: usize = COUNT + 5; // whole sources, patch log, dump, crash.txt, breadcrumbs, summary
/// The client patches' log of before the runs were named (vostok/log.rs):
/// the crashed run's when no `srp_patches.<stamp>.log` is beside the exe.
pub const PATCH_LOG: &str = "srp_patches.log";
/// The crashed run's patch log can be long (8 MiB a part), so the ZIP takes
/// its last [`PATCH_LOG_BYTES`].
pub const PATCH_LOG_BYTES: u64 = 2 * 1024 * 1024;
pub const PACKED_BYTES: u64 = 16 * 1024 * 1024; // log local headers + data + central records
pub const INPUT_BYTES: u64 = 256 * 1024 * 1024; // source bytes read, including failed attempts
const AGE: u64 = 48 * 60 * 60 * 10_000_000;
pub const SCOPES: [&str; 5] = [
    "documents",
    "game-exe",
    "game-root",
    "install-root",
    "install-temp",
];

pub struct Manifest {
    pub buf: [u8; 32768],
    pub len: usize,
}
impl Manifest {
    pub fn new() -> Self {
        Self {
            buf: [0; 32768],
            len: 0,
        }
    }
}
impl Write for Manifest {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        if value.len() > self.buf.len() - self.len {
            return Err(fmt::Error);
        }
        self.buf[self.len..self.len + value.len()].copy_from_slice(value.as_bytes());
        self.len += value.len();
        Ok(())
    }
}
/// How much of a source the ZIP takes.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Take {
    Whole,
    /// Its last bytes, from the first whole line in them.
    Last(u64),
}
#[derive(Clone, Copy)]
pub struct Candidate {
    pub name: [u16; 260],
    pub len: usize,
    pub scope: usize,
    pub take: Take,
    /// A run's log (its kind and stamp, parsed from the name).
    run: Option<RunName>,
    modified: u64,
}
impl Candidate {
    const EMPTY: Self = Self {
        name: [0; 260],
        len: 0,
        scope: 0,
        take: Take::Whole,
        run: None,
        modified: 0,
    };
    /// The crashed run's patch log in the exe's folder `exe` (`folders()[1]`,
    /// or dump::folder's), its tail: the newest (last written)
    /// `srp_patches.<stamp>[_<n>].log`, else the old `srp_patches.log`.
    pub unsafe fn patch_log(exe: &[u16]) -> Self {
        let mut log = Self::EMPTY;
        log.scope = 1;
        log.take = Take::Last(PATCH_LOG_BYTES);
        let mut pattern = Wide::<1100>::new();
        // Never a scan of the working folder: no exe folder, no scan.
        if !exe.is_empty()
            && pattern.wide(exe).is_ok()
            && write!(pattern, "srp_patches.*.log").is_ok()
        {
            let mut data: WIN32_FIND_DATAW = core::mem::zeroed();
            let find = FindFirstFileW(pattern.ptr(), &mut data);
            if find != INVALID_HANDLE_VALUE {
                loop {
                    let n = data.cFileName.iter().position(|&c| c == 0).unwrap_or(0);
                    let modified = ticks(data.ftLastWriteTime);
                    let run = parse(&data.cFileName[..n]).filter(|run| run.kind == Kind::Patches);
                    if run.is_some()
                        && data.dwFileAttributes
                            & (FILE_ATTRIBUTE_DIRECTORY | FILE_ATTRIBUTE_REPARSE_POINT)
                            == 0
                        && (log.run.is_none() || modified >= log.modified)
                    {
                        log.name = data.cFileName;
                        log.len = n;
                        log.run = run;
                        log.modified = modified;
                    }
                    if FindNextFileW(find, &mut data) == 0 {
                        break;
                    }
                }
                FindClose(find);
            }
        }
        if log.run.is_none() {
            log.name = [0; 260];
            for (slot, b) in log.name.iter_mut().zip(PATCH_LOG.bytes()) {
                *slot = b as u16;
            }
            log.len = PATCH_LOG.len();
        }
        log
    }
    /// The file name (no folder) as text, for a name of ours alone: the
    /// patch log's (parsed, or the fixed old one).
    pub fn patch_log_name(&self, out: &mut dyn Write) -> fmt::Result {
        match self.run {
            Some(run) => write!(out, "srp_patches.{}.log", run.stamp()),
            None => out.write_str(PATCH_LOG),
        }
    }
    fn same_file(&self, scope: usize, name: &[u16]) -> bool {
        self.len != 0
            && self.scope == scope
            && self.len == name.len()
            && same_wide(&self.name[..self.len], name)
    }
    // Never echo user-controlled filenames into attachments or the summary:
    // a run's log adds only its kind's fixed word and the parsed stamp.
    pub fn zip_name(&self, index: usize, buf: &mut [u8; zip::MAX_NAME]) -> usize {
        let mut name = Ascii { buf, len: 0 };
        let _ = if let Take::Last(_) = self.take {
            // Our own file's name, not one of the player's.
            name.write_str("logs/game-exe/")
                .and_then(|()| self.patch_log_name(&mut name))
        } else {
            write!(name, "logs/{}/source-{:02}", SCOPES[self.scope], index + 1)
                .and_then(|()| match self.run {
                    Some(run) => write!(name, ".{}.{}", run.kind.word(), run.stamp()),
                    None => Ok(()),
                })
                .and_then(|()| name.write_str(".txt"))
        };
        name.len
    }
}
/// A ZIP member name being written: ASCII into the record's buffer.
struct Ascii<'a> {
    buf: &'a mut [u8; zip::MAX_NAME],
    len: usize,
}
impl Write for Ascii<'_> {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let end = self.len + value.len();
        if end > self.buf.len() {
            return Err(fmt::Error);
        }
        self.buf[self.len..end].copy_from_slice(value.as_bytes());
        self.len = end;
        Ok(())
    }
}
/// Two file names, as Windows compares them (ASCII case only).
fn same_wide(a: &[u16], b: &[u16]) -> bool {
    let lower = |c: u16| {
        if (b'A' as u16..=b'Z' as u16).contains(&c) {
            c + 32
        } else {
            c
        }
    };
    a.len() == b.len() && a.iter().zip(b).all(|(&x, &y)| lower(x) == lower(y))
}
fn ticks(t: FILETIME) -> u64 {
    (t.dwHighDateTime as u64) << 32 | t.dwLowDateTime as u64
}
fn same(wide: &[u16], ascii: &[u8]) -> bool {
    wide.len() == ascii.len()
        && wide
            .iter()
            .zip(ascii)
            .all(|(&w, &a)| w <= 127 && (w as u8).eq_ignore_ascii_case(&a))
}
// Game/launcher logs, game journals, DXVK's executable-prefixed render logs,
// the updater's explicit diagnostic log, and every run's log (the game's,
// the client patches' and the updater's `<kind>.<stamp>.log`, run_name.rs).
// No settings, captures or patch journals.
fn game_log(name: &[u16]) -> bool {
    parse(name).is_some() || unnamed_log(name)
}
// The logs whose names hold no run (the game's before the runs were named).
fn unnamed_log(name: &[u16]) -> bool {
    let extension = if name.len() >= 4 && same(&name[name.len() - 4..], b".log") {
        4
    } else if name.len() >= 8 && same(&name[name.len() - 8..], b".journal") {
        8
    } else {
        return false;
    };
    let stem = &name[..name.len() - extension];
    same(stem, b"Survarium")
        || (stem.len() > 10 && same(&stem[..10], b"Survarium_"))
        || (extension == 4
            && [b"updater_skhid".as_slice(), b"launcher_skhid", b"launcher"]
                .iter()
                .any(|n| same(stem, n)))
}

pub unsafe fn folders() -> [Wide<1024>; 5] {
    let mut folders = [const { Wide::new() }; 5];
    let docs = &mut folders[0];
    if SHGetFolderPathW(
        null_mut(),
        CSIDL_PERSONAL as i32,
        null_mut(),
        0,
        docs.buf.as_mut_ptr(),
    ) == 0
    {
        docs.len = docs.buf.iter().position(|&c| c == 0).unwrap_or(0);
        if docs.len == 0 || write!(docs, "\\Survarium\\").is_err() {
            docs.truncate(0);
        }
    }
    let exe = &mut folders[1];
    let n = GetModuleFileNameW(null_mut(), exe.buf.as_mut_ptr(), exe.buf.len() as u32) as usize;
    if n == 0 || n >= exe.buf.len() {
        exe.truncate(0);
        return folders;
    }
    if let Some(at) = exe.buf[..n]
        .iter()
        .rposition(|&c| c == b'\\' as u16 || c == b'/' as u16)
    {
        exe.truncate(at + 1);
    } else {
        exe.truncate(0);
        return folders;
    }
    let n = exe.len;
    // Only infer parent folders for the known installation layout, never from CWD.
    if n >= 13 && same(&exe.buf[n - 13..n], b"binaries\\x86\\") {
        let (exe, rest) = folders.split_at_mut(2);
        let _ = rest[0].wide(&exe[1].buf[..n - 13]);
        let game_len = rest[0].len;
        if game_len >= 5 && same(&rest[0].buf[game_len - 5..game_len], b"game\\") {
            let (game, install) = rest.split_at_mut(1);
            let _ = install[0].wide(&game[0].buf[..game_len - 5]);
            let _ = install[1].wide(&game[0].buf[..game_len - 5]);
            let _ = write!(install[1], "temp\\");
        }
    }
    folders
}

pub unsafe fn recent(
    folders: &mut [Wide<1024>; 5],
    deadline: u64,
    manifest: &mut Manifest,
    patch_log: &Candidate,
) -> [Candidate; COUNT] {
    let mut selected = [Candidate::EMPTY; COUNT];
    let mut now = core::mem::zeroed();
    GetSystemTimeAsFileTime(&mut now);
    let oldest = ticks(now).saturating_sub(AGE);
    let mut recent: usize = 0;
    for (scope, folder) in folders.iter_mut().enumerate() {
        if folder.len == 0 {
            let _ = writeln!(
                manifest,
                "{}: folder unavailable or layout not applicable",
                SCOPES[scope]
            );
            continue;
        }
        if GetTickCount64() > deadline {
            let _ = writeln!(manifest, "{}: scan skipped (time limit)", SCOPES[scope]);
            continue;
        }
        let base = folder.len;
        if write!(folder, "*").is_err() {
            continue;
        }
        let mut data: WIN32_FIND_DATAW = core::mem::zeroed();
        let find = FindFirstFileW(folder.ptr(), &mut data);
        folder.truncate(base);
        if find == INVALID_HANDLE_VALUE {
            let _ = writeln!(
                manifest,
                "{}: scan unavailable, Windows error {}",
                SCOPES[scope],
                GetLastError()
            );
            continue;
        }
        let (mut matched, mut stale, mut special, mut error) = (0, 0, 0, 0);
        loop {
            let n = data.cFileName.iter().position(|&c| c == 0).unwrap_or(0);
            let modified = ticks(data.ftLastWriteTime);
            let name = &data.cFileName[..n];
            // The crashed run's patch log is taken first, by itself.
            if game_log(name) && !patch_log.same_file(scope, name) {
                matched += 1;
                if data.dwFileAttributes & (FILE_ATTRIBUTE_DIRECTORY | FILE_ATTRIBUTE_REPARSE_POINT)
                    != 0
                {
                    special += 1;
                } else if modified < oldest {
                    stale += 1;
                } else {
                    recent += 1;
                    if let Some(at) = selected
                        .iter()
                        .position(|c| c.len == 0 || modified > c.modified)
                    {
                        for i in (at + 1..COUNT).rev() {
                            selected[i] = selected[i - 1];
                        }
                        selected[at] = Candidate {
                            name: data.cFileName,
                            len: n,
                            scope,
                            take: Take::Whole,
                            run: parse(name),
                            modified,
                        };
                    }
                }
            }
            if GetTickCount64() > deadline {
                error = ERROR_TIMEOUT;
                break;
            }
            if FindNextFileW(find, &mut data) == 0 {
                let e = GetLastError();
                if e != ERROR_NO_MORE_FILES {
                    error = e;
                }
                break;
            }
        }
        FindClose(find);
        let _ = writeln!(
            manifest,
            "{}: matched={}, stale={}, directories/reparse={}, scan_error={}",
            SCOPES[scope], matched, stale, special, error
        );
    }
    let _ = writeln!(
        manifest,
        "Recent candidates: {}; omitted by 32-file limit: {}",
        recent,
        recent.saturating_sub(COUNT)
    );
    selected
}

/// The code page the game writes its log in, and where that came from. The
/// engine's `core::preinitialize` (0.26g 0xE3C10) sets the C locale from its
/// `-localization`: original and russian `.1251`, english `.1252`, polish
/// `.1250`; any other leaves the "C" locale, whose wide-to-narrow conversion
/// (the log's `%S`, e.g. the audio device names) writes Latin-1 bytes. With
/// no `-localization` the option keeps its default, "original" (its static
/// initializer 0x84E3E0 sets the string at 0x21E75E0; the parser 0xDDEB0 only
/// overwrites it when the option is given): cp1251. So the transcoder only
/// ever sees single-byte code pages, where no lead byte can swallow an ASCII
/// byte of a credential label.
pub unsafe fn code_page() -> (u32, &'static str) {
    const KEY: &[u8] = b"-localization=";
    let line = GetCommandLineW();
    let mut n = 0;
    while !line.is_null() && *line.add(n) != 0 {
        n += 1;
    }
    let line = if line.is_null() {
        &[][..]
    } else {
        core::slice::from_raw_parts(line, n)
    };
    let Some(at) = line.windows(KEY.len()).position(|w| same(w, KEY)) else {
        return (1251, "the game's default -localization (original)");
    };
    let value = &line[at + KEY.len()..];
    let end = value
        .iter()
        .position(|&c| c == b' ' as u16 || c == b'\t' as u16 || c == b'"' as u16)
        .unwrap_or(value.len());
    let value = &value[..end];
    let exact = |name: &[u8]| {
        value.len() == name.len() && value.iter().zip(name).all(|(&c, &b)| c == b as u16)
    };
    if exact(b"original") || exact(b"russian") {
        (1251, "the game's -localization")
    } else if exact(b"english") {
        (1252, "the game's -localization")
    } else if exact(b"polish") {
        (1250, "the game's -localization")
    } else {
        (28591, "the game's -localization (C locale)")
    }
}

/// The UTF-16 between the two conversions: a line has at most LINE units.
/// Captures are serialized (dump.rs), so one buffer serves every filter.
struct Units(core::cell::UnsafeCell<[u16; crate::sanitize::LINE]>);
unsafe impl Sync for Units {}
static UNITS: Units = Units(core::cell::UnsafeCell::new([0; crate::sanitize::LINE]));

/// sanitize::Transcode: `line[..len]` of `code_page` to UTF-8 in place.
pub fn transcode(code_page: u32, line: &mut [u8], len: usize) -> Result<usize, u32> {
    let len = len.min(line.len());
    unsafe {
        let units = &mut *UNITS.0.get();
        let wide = MultiByteToWideChar(
            code_page,
            MB_ERR_INVALID_CHARS,
            line.as_ptr(),
            len as i32,
            units.as_mut_ptr(),
            units.len() as i32,
        );
        if wide <= 0 {
            return Err(crate::sanitize::ERROR_UNSAFE_TEXT);
        }
        let n = WideCharToMultiByte(
            CP_UTF8,
            0,
            units.as_ptr(),
            wide,
            line.as_mut_ptr(),
            line.len() as i32,
            core::ptr::null(),
            null_mut(),
        );
        if n <= 0 {
            return Err(if GetLastError() == ERROR_INSUFFICIENT_BUFFER {
                crate::sanitize::ERROR_LONG_LINE
            } else {
                crate::sanitize::ERROR_UNSAFE_TEXT
            });
        }
        Ok(n as usize)
    }
}
