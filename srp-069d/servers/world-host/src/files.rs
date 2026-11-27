//! File system bridge: Windows paths -> Linux paths.
//!
//! * The game folder: the exe's Windows path (`--exe-win`) and its Linux path (`--exe`) name the same file, so the part
//!   of the Windows path in front of their common tail is the Linux folder in front of it ([`game_mapping`]). A native
//!   Linux install `/opt/survarium/binaries/x64/survarium.exe` seen as `G:\binaries\x64\survarium.exe` maps `G:\...` to
//!   `/opt/survarium/...`.
//! * Any other `X:\...` -> `/mnt/x/...` (WSL drive mounts). The game tree is read-only for us.
//! * Writes (and directory creation) under `/mnt/...` go to an overlay `<out>/overlay/mnt/...`;
//!   reads look at the overlay first. Nothing on the game tree is ever modified.
//! * `Z:\...` is the host's private drive (`<out>/zdrive/...`): SHGetKnownFolderPath answers there.
//! * The virtual current directory is the exe's directory (`...\game\binaries\x64`), so the client's
//!   relative paths (`../../resources`, `../../mounts`, `../../resources.NN.db`) resolve as on Windows.

use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

pub struct FsConfig {
    /// Windows form of the current directory (the exe's folder), e.g. `C:\Games\Survarium\game\binaries\x64`
    pub cwd_win: String,
    pub overlay: PathBuf,
    pub zdrive: PathBuf,
    /// (Windows folder in `win_abs` form, upper case, e.g. `G:`; its Linux folder): from [`game_mapping`]. None = the
    /// exe lies under the `/mnt/<drive>` its Windows path names.
    pub game: Option<(String, PathBuf)>,
}

/// The folder mapping of the exe's Windows and Linux paths: strip their common tail (path components compared without
/// case); what is left is (Windows folder, Linux folder). None when the plain drive rule (`X:` -> `/mnt/x`) already gives
/// the Linux path, or when the paths are not absolute.
pub fn game_mapping(exe: &Path, exe_win: &str) -> Option<(String, PathBuf)> {
    let w = exe_win.replace('/', "\\");
    if w.len() < 2 || w.as_bytes()[1] != b':' || !exe.is_absolute() {
        return None;
    }
    let drive = w[..2].to_ascii_uppercase();
    let mut wparts: Vec<&str> = w[2..].split('\\').filter(|c| !c.is_empty()).collect();
    let mut uparts: Vec<String> = exe.components().skip(1).map(|c| c.as_os_str().to_string_lossy().to_string()).collect();
    while let (Some(a), Some(b)) = (wparts.last(), uparts.last()) {
        if !a.eq_ignore_ascii_case(b) {
            break;
        }
        wparts.pop();
        uparts.pop();
    }
    let win = std::iter::once(drive.clone()).chain(wparts.iter().map(|p| p.to_ascii_uppercase())).collect::<Vec<_>>().join("\\");
    let unix = PathBuf::from("/").join(uparts.iter().collect::<PathBuf>());
    let plain = PathBuf::from(format!("/mnt/{}", drive[..1].to_ascii_lowercase())).join(wparts.iter().collect::<PathBuf>());
    if unix == plain {
        return None;
    }
    Some((win, unix))
}

pub static CFG: OnceLock<FsConfig> = OnceLock::new();

pub enum FileKind {
    File(File),
    Dir,
}

pub struct FileObj {
    pub kind: FileKind,
    pub pos: u64,
    pub path: PathBuf,
    pub win_path: String,
    pub overlapped: bool,
}

pub struct FindObj {
    pub entries: Vec<(String, std::fs::Metadata)>,
    pub next: usize,
}

static OPEN_LOG: Mutex<Vec<String>> = Mutex::new(Vec::new());

pub fn note_open(s: String) {
    OPEN_LOG.lock().unwrap().push(s);
}

/// Normalise a Windows path to an absolute `X:\a\b` form (no `.`/`..`).
pub fn win_abs(p: &str) -> String {
    let cfg = CFG.get().unwrap();
    let mut p = p.replace('/', "\\");
    if let Some(r) = p.strip_prefix("\\\\?\\") {
        p = r.to_string();
    }
    let abs = if p.len() >= 2 && p.as_bytes()[1] == b':' {
        p
    } else if p.starts_with('\\') {
        format!("{}{}", &cfg.cwd_win[..2], p)
    } else {
        format!("{}\\{}", cfg.cwd_win, p)
    };
    let drive = abs[..2].to_ascii_uppercase();
    let mut parts: Vec<&str> = Vec::new();
    for c in abs[2..].split('\\') {
        match c {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            x => parts.push(x),
        }
    }
    format!("{}\\{}", drive, parts.join("\\"))
}

/// Host path for a Windows path: (real path, overlay path if the target is on a read-only mount).
pub fn host_paths(win: &str) -> (PathBuf, Option<PathBuf>) {
    let cfg = CFG.get().unwrap();
    let a = win_abs(win);
    let drive = a.as_bytes()[0].to_ascii_lowercase() as char;
    let rest = a[3..].replace('\\', "/");
    if drive == 'z' {
        return (cfg.zdrive.join(&rest), None);
    }
    let real = match &cfg.game {
        Some((win, unix)) => match mapped(&a, win, unix) {
            Some(p) => p,
            None => PathBuf::from(format!("/mnt/{drive}/{rest}")),
        },
        None => PathBuf::from(format!("/mnt/{drive}/{rest}")),
    };
    let ov = cfg.overlay.join(format!("mnt/{drive}/{rest}"));
    (real, Some(ov))
}

/// The Linux path of `abs` (`win_abs` form) when it lies in the mapped Windows folder `win` (upper case).
fn mapped(abs: &str, win: &str, unix: &Path) -> Option<PathBuf> {
    let upper = abs.to_ascii_uppercase();
    let rest = upper.strip_prefix(win)?;
    if !(rest.is_empty() || rest.starts_with('\\')) {
        return None;
    }
    let tail = &abs[win.len()..];
    Some(tail.split('\\').filter(|c| !c.is_empty()).fold(unix.to_path_buf(), |p, c| p.join(c)))
}

/// Path to read: the overlay copy if it exists, else the real path. Case-insensitive fallback
/// (drvfs is already case-insensitive; the overlay is not).
pub fn read_path(win: &str) -> PathBuf {
    let (real, ov) = host_paths(win);
    if let Some(o) = ov {
        if o.exists() {
            return o;
        }
    }
    if real.exists() {
        return real;
    }
    ci_resolve(&real).unwrap_or(real)
}

/// Path to write (created parents as needed).
pub fn write_path(win: &str) -> PathBuf {
    let (real, ov) = host_paths(win);
    let p = ov.unwrap_or(real);
    if let Some(parent) = p.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    p
}

fn ci_resolve(p: &Path) -> Option<PathBuf> {
    let mut cur = PathBuf::from("/");
    for comp in p.components().skip(1) {
        let name = comp.as_os_str().to_string_lossy().to_string();
        let direct = cur.join(&name);
        if direct.exists() {
            cur = direct;
            continue;
        }
        let lower = name.to_lowercase();
        let found = std::fs::read_dir(&cur).ok()?.flatten().find(|e| e.file_name().to_string_lossy().to_lowercase() == lower)?;
        cur = found.path();
    }
    Some(cur)
}

pub const GENERIC_READ: u32 = 0x8000_0000;
pub const GENERIC_WRITE: u32 = 0x4000_0000;
pub const FILE_FLAG_OVERLAPPED: u32 = 0x4000_0000;

/// CreateFile. Returns Ok(FileObj) or Err(win32 error).
pub fn open(win: &str, access: u32, disposition: u32, flags: u32) -> Result<FileObj, u32> {
    // GENERIC_WRITE, FILE_WRITE_DATA, FILE_APPEND_DATA, GENERIC_ALL
    let write = access & (GENERIC_WRITE | 0x2 | 0x4 | 0x1000_0000) != 0;
    let rpath = read_path(win);
    if rpath.is_dir() {
        return Ok(FileObj { kind: FileKind::Dir, pos: 0, path: rpath, win_path: win.to_string(), overlapped: false });
    }
    let path = if write { write_path(win) } else { rpath };
    if write && !path.exists() {
        // copy-on-write: bring the real file into the overlay first for OPEN_ALWAYS / OPEN_EXISTING writes
        let real = read_path(win);
        if real != path && real.exists() && disposition != 2 && disposition != 5 {
            let _ = std::fs::copy(&real, &path);
        }
    }
    let exists = path.exists();
    match disposition {
        1 if exists => return Err(80),     // CREATE_NEW: ERROR_FILE_EXISTS
        3 | 5 if !exists => return Err(2), // OPEN_EXISTING / TRUNCATE_EXISTING: ERROR_FILE_NOT_FOUND
        _ => {}
    }
    if !exists && path.parent().map(|p| !p.exists()).unwrap_or(false) {
        return Err(3);
    }
    let mut oo = OpenOptions::new();
    oo.read(true);
    if write {
        oo.write(true);
        match disposition {
            1 => {
                oo.create_new(true);
            }
            2 => {
                oo.create(true).truncate(true);
            }
            4 => {
                oo.create(true);
            }
            5 => {
                oo.truncate(true);
            }
            _ => {}
        }
    }
    match oo.open(&path) {
        Ok(f) => Ok(FileObj {
            kind: FileKind::File(f),
            pos: 0,
            path,
            win_path: win.to_string(),
            overlapped: flags & FILE_FLAG_OVERLAPPED != 0,
        }),
        Err(e) => Err(match e.kind() {
            std::io::ErrorKind::NotFound => 2,
            std::io::ErrorKind::PermissionDenied => 5,
            std::io::ErrorKind::AlreadyExists => 80,
            _ => 31,
        }),
    }
}

pub fn unix_to_filetime(secs: i64, nanos: i64) -> u64 {
    ((secs + 11_644_473_600) as u64) * 10_000_000 + (nanos as u64) / 100
}

pub fn attrs_of(md: &std::fs::Metadata) -> u32 {
    if md.is_dir() { 0x10 } else { 0x80 } // FILE_ATTRIBUTE_DIRECTORY / NORMAL
}

pub fn times_of(md: &std::fs::Metadata) -> (u64, u64, u64) {
    use std::os::unix::fs::MetadataExt;
    let m = unix_to_filetime(md.mtime(), md.mtime_nsec());
    let a = unix_to_filetime(md.atime(), md.atime_nsec());
    let c = unix_to_filetime(md.ctime(), md.ctime_nsec());
    (c, a, m)
}

/// Simple case-insensitive wildcard match (`*`, `?`).
pub fn wild(pat: &[char], s: &[char]) -> bool {
    match (pat.first(), s.first()) {
        (None, None) => true,
        (Some('*'), _) => wild(&pat[1..], s) || (!s.is_empty() && wild(pat, &s[1..])),
        (Some('?'), Some(_)) => wild(&pat[1..], &s[1..]),
        (Some(a), Some(b)) => a.to_lowercase().eq(b.to_lowercase()) && wild(&pat[1..], &s[1..]),
        _ => false,
    }
}

/// FindFirstFile: list matches of `dir\pattern` (union of real dir and overlay dir).
pub fn find(win_pattern: &str) -> Option<FindObj> {
    let a = win_abs(win_pattern);
    let (dir, pat) = a.rsplit_once('\\')?;
    let pat: Vec<char> = pat.chars().collect();
    let (real, ov) = host_paths(dir);
    let mut entries: Vec<(String, std::fs::Metadata)> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let real = if real.exists() { real } else { ci_resolve(&real).unwrap_or(real) };
    for d in ov.into_iter().chain(std::iter::once(real)) {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            let cs: Vec<char> = name.chars().collect();
            if wild(&pat, &cs) && seen.insert(name.to_lowercase()) {
                if let Ok(md) = e.metadata() {
                    entries.push((name, md));
                }
            }
        }
    }
    if entries.is_empty() {
        return None;
    }
    entries.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
    Some(FindObj { entries, next: 0 })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wsl_drive_path_needs_no_mapping() {
        assert_eq!(game_mapping(Path::new("/mnt/c/Games/Survarium/game/binaries/x64/survarium.exe"), "C:\\Games\\Survarium\\game\\binaries\\x64\\survarium.exe"), None);
        assert_eq!(game_mapping(Path::new("/mnt/c/Games/S/binaries/x64/survarium.exe"), "c:\\games\\s\\binaries\\x64\\survarium.exe"), None, "case does not matter");
    }

    #[test]
    fn a_native_linux_folder_maps_to_the_windows_folder() {
        let m = game_mapping(Path::new("/opt/survarium/game/binaries/x64/survarium.exe"), "G:\\binaries\\x64\\survarium.exe").unwrap();
        assert_eq!(m, ("G:".to_string(), PathBuf::from("/opt/survarium/game")));
        let (w, u) = &m;
        assert_eq!(mapped("G:\\resources.00.db", w, u), Some(PathBuf::from("/opt/survarium/game/resources.00.db")));
        assert_eq!(mapped("G:\\binaries\\x64", w, u), Some(PathBuf::from("/opt/survarium/game/binaries/x64")));
        assert_eq!(mapped("G:\\", w, u), Some(PathBuf::from("/opt/survarium/game")));
        assert_eq!(mapped("C:\\Windows\\system32", w, u), None, "other drives keep the /mnt rule");
        // a mapped folder deeper in a drive, and a WSL exe the user gave another Windows path
        let m = game_mapping(Path::new("/home/me/survarium/binaries/x64/survarium.exe"), "D:\\Games\\S\\binaries\\x64\\survarium.exe").unwrap();
        assert_eq!(m, ("D:\\GAMES\\S".to_string(), PathBuf::from("/home/me/survarium")));
        assert_eq!(mapped("D:\\Games\\S\\resources\\a.txt", &m.0, &m.1), Some(PathBuf::from("/home/me/survarium/resources/a.txt")));
        assert_eq!(mapped("D:\\Games\\Sx\\a.txt", &m.0, &m.1), None, "only whole folder names match");
    }
}
