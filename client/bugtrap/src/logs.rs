// Optional, bounded attachments. No game logging API or game heap is touched:
// the faulting thread may hold their locks.
use crate::{text::Wide, zip};
use core::{
    fmt::{self, Write},
    ptr::null_mut,
};
use windows_sys::Win32::{
    Foundation::*,
    Storage::FileSystem::*,
    System::{LibraryLoader::GetModuleFileNameW, SystemInformation::*},
    UI::Shell::*,
};

pub const COUNT: usize = 32;
pub const MAX_MEMBERS: usize = COUNT + 2; // whole sources, dump, summary
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
#[derive(Clone, Copy)]
pub struct Candidate {
    pub name: [u16; 260],
    pub len: usize,
    pub scope: usize,
    modified: u64,
}
impl Candidate {
    const EMPTY: Self = Self {
        name: [0; 260],
        len: 0,
        scope: 0,
        modified: 0,
    };
    // Never echo user-controlled filenames into attachments or the summary.
    pub fn zip_name(&self, index: usize, buf: &mut [u8; zip::MAX_NAME]) -> usize {
        let mut prefix = 0;
        for part in [
            b"logs/".as_slice(),
            SCOPES[self.scope].as_bytes(),
            b"/source-",
        ] {
            buf[prefix..prefix + part.len()].copy_from_slice(part);
            prefix += part.len();
        }
        buf[prefix] = b'0' + ((index + 1) / 10) as u8;
        buf[prefix + 1] = b'0' + ((index + 1) % 10) as u8;
        buf[prefix + 2..prefix + 6].copy_from_slice(b".txt");
        prefix + 6
    }
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
// and the updater's explicit diagnostic log. No settings, captures or patch journals.
fn game_log(name: &[u16]) -> bool {
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
            if game_log(&data.cFileName[..n]) {
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
