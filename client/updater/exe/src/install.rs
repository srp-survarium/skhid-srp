//! Puts the archive's files in their places: all of them or none.
//!
//! 1. A file that is already there with the archive's bytes is left alone.
//! 2. Every other file is first written whole beside its place, as
//!    `<name>.skhid-new`, and read back.
//! 3. Only then the places are changed, one rename each: the file that is
//!    there becomes `<name>.skhid-old`, the new one takes its name. Windows
//!    lets a running program's file be renamed, so the launcher and this
//!    updater are replaced the same way while they run.
//! 4. If a rename fails, the ones done are undone, and the game folder is
//!    as it was.
//!
//! What is left over (`.skhid-old` files of programs that were running,
//! `.skhid-new` files of a run that was cut) is deleted at the next start.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use updater_logic::file_error::Stop;
use updater_logic::manifest::player_may_edit;

use crate::win::{self, Code};

/// The name of a new file until it takes its place, and of the file it
/// replaced.
const NEW: &str = ".skhid-new";
const OLD: &str = ".skhid-old";

/// Where the archive's paths are on this computer.
pub struct Layout {
    /// `...\game\binaries\x86`: where this program is.
    pub exe_dir: String,
    /// The game folder, two above: the archive's `game/`.
    pub game_dir: String,
    /// The archive's top folder: the one that holds the game folder when
    /// that is named `game` (as it is unpacked), else the game folder itself.
    pub top_dir: String,
    /// This program's file name.
    pub exe_name: String,
}

impl Layout {
    /// From this program's own path, which must be `<game>\binaries\x86\<exe>`.
    pub fn find() -> Result<Self, String> {
        let path = win::module_path().map_err(|code| {
            format!(
                "Windows did not tell where the updater is ({})",
                win::error_text(code)
            )
        })?;
        Self::of(&path).ok_or(path)
    }

    fn of(path: &str) -> Option<Self> {
        let (exe_dir, exe_name) = win::split_path(path);
        let (binaries, x86) = win::split_path(exe_dir);
        let (game_dir, binaries_name) = win::split_path(binaries);
        if !x86.eq_ignore_ascii_case("x86")
            || !binaries_name.eq_ignore_ascii_case("binaries")
            || game_dir.is_empty()
        {
            return None;
        }
        let (parent, game_name) = win::split_path(game_dir);
        let top_dir = if game_name.eq_ignore_ascii_case("game") && !parent.is_empty() {
            parent
        } else {
            game_dir
        };
        Some(Self {
            exe_dir: exe_dir.into(),
            game_dir: game_dir.into(),
            top_dir: top_dir.into(),
            exe_name: exe_name.into(),
        })
    }

    /// The place of the archive's file `name` (a checked name:
    /// `updater_logic::zip::safe_name`).
    pub fn place(&self, name: &str) -> String {
        match name.strip_prefix("game/") {
            Some(rest) => format!("{}\\{}", self.game_dir, rest.replace('/', "\\")),
            None => format!("{}\\{}", self.top_dir, name.replace('/', "\\")),
        }
    }
}

/// One file of the archive, unpacked.
pub struct NewFile {
    pub name: String,
    pub data: Vec<u8>,
    pub sha256: [u8; 32],
}

#[derive(Debug)]
pub enum InstallError {
    /// A folder could not be made.
    Folder { path: String, code: Code },
    /// Less room on the disk than the new files need.
    DiskFull {
        path: String,
        needed: u64,
        free: u64,
    },
    /// A new file could not be written beside its place.
    Write { path: String, code: Code },
    /// A new file read back with other bytes than were written.
    ReadBack { path: String },
    /// A new file was gone, or denied to this program, when read back just
    /// after it was written: an antivirus took it
    /// (`updater_logic::file_error::Stop::after_write`).
    Removed { path: String, code: Code },
    /// A file could not be moved out of its place, or the new one into it.
    Replace { path: String, code: Code },
    /// The player closed the launcher.
    Stopped,
}

impl InstallError {
    /// The file Windows Security, or another antivirus, did not let be
    /// written or replaced, and the code (`updater_logic::file_error`,
    /// `Stop::Virus`).
    pub fn virus(&self) -> Option<(&str, Code)> {
        match self {
            Self::Removed { path, code } => Some((path, *code)),
            Self::Folder { path, code }
            | Self::Write { path, code }
            | Self::Replace { path, code }
                if Stop::of(*code) == Some(Stop::Virus) =>
            {
                Some((path, *code))
            }
            _ => None,
        }
    }
}

/// What an install changed.
#[derive(Debug, Default)]
pub struct Outcome {
    /// The files that were written (not those that were already right).
    pub written: Vec<String>,
    /// This program or the launcher was among them: the launcher must be
    /// started again to run the new ones.
    pub restart: bool,
}

/// The file names (in the archive) of the launcher this updater belongs to.
pub const LAUNCHER_NAME: &str = "launcher_skhid.exe";

struct Change {
    name: String,
    place: String,
    staged: String,
    /// Where the file that was there went; `None` when there was none.
    backup: Option<String>,
    done: bool,
}

fn same_as(place: &str, file: &NewFile) -> bool {
    matches!(win::hash_file(place), Ok((size, hash)) if size == file.data.len() as u64 && hash == file.sha256)
}

/// A name for the file that leaves its place: `.skhid-old`, or with a digit
/// when an older one is still held by a running program.
fn backup_name(place: &str) -> Option<String> {
    (0..10)
        .map(|index| {
            if index == 0 {
                format!("{place}{OLD}")
            } else {
                format!("{place}{OLD}{index}")
            }
        })
        .find(|name| win::delete(name).is_ok())
}

fn undo(changes: &mut [Change]) {
    for change in changes.iter_mut().rev() {
        if change.done {
            // The new file leaves; the one that was there comes back.
            let _ = win::delete(&change.place);
            if let Some(backup) = &change.backup {
                let _ = win::rename(backup, &change.place, true);
            }
            change.done = false;
        }
        let _ = win::delete(&change.staged);
    }
}

/// What an install tells while it runs.
pub enum Step<'a> {
    /// The file that is written next.
    File(&'a str),
    /// The bytes written so far, of all.
    Bytes(u64, u64),
}

/// Installs `files`. `progress` is called before each file and with the
/// bytes written so far; `false` from it stops the install (nothing is
/// changed then). `keep_edited`: the archive is the one that was installed
/// before (a repair), so a text file the player may have edited
/// (`player_may_edit`) stays as it is when it is there.
pub fn install(
    layout: &Layout,
    files: &[NewFile],
    keep_edited: bool,
    progress: &mut dyn FnMut(Step) -> bool,
) -> Result<Outcome, InstallError> {
    // 1. What differs from what is there.
    let mut changes: Vec<(usize, Change)> = Vec::new();
    for (index, file) in files.iter().enumerate() {
        let place = layout.place(&file.name);
        if same_as(&place, file) {
            continue;
        }
        if keep_edited && player_may_edit(&file.name) && win::exists(&place) {
            continue;
        }
        changes.push((
            index,
            Change {
                name: file.name.clone(),
                staged: format!("{place}{NEW}"),
                place,
                backup: None,
                done: false,
            },
        ));
    }
    let total: u64 = changes
        .iter()
        .map(|(index, _)| files[*index].data.len() as u64)
        .sum();
    if let Some(free) = win::free_space(&layout.game_dir) {
        // Room for the new files beside the old ones, and a little more.
        let needed = total + (1 << 20);
        if free < needed {
            return Err(InstallError::DiskFull {
                path: layout.game_dir.clone(),
                needed,
                free,
            });
        }
    }

    // 2. Every new file beside its place, read back.
    let mut written = 0_u64;
    for at in 0..changes.len() {
        let (index, change) = &changes[at];
        let file = &files[*index];
        let staged = (|| {
            if !progress(Step::File(&change.name)) || !progress(Step::Bytes(written, total)) {
                return Err(InstallError::Stopped);
            }
            let (folder, _) = win::split_path(&change.place);
            win::create_folders(folder).map_err(|code| InstallError::Folder {
                path: folder.into(),
                code,
            })?;
            win::write_file(&change.staged, &file.data).map_err(|code| InstallError::Write {
                path: change.place.clone(),
                code,
            })?;
            match win::hash_file(&change.staged) {
                Ok((size, hash)) if size == file.data.len() as u64 && hash == file.sha256 => Ok(()),
                Ok(_) => Err(InstallError::ReadBack {
                    path: change.place.clone(),
                }),
                Err(code) if Stop::after_write(code) == Some(Stop::Virus) => {
                    Err(InstallError::Removed {
                        path: change.place.clone(),
                        code,
                    })
                }
                Err(code) => Err(InstallError::Write {
                    path: change.place.clone(),
                    code,
                }),
            }
        })();
        if let Err(error) = staged {
            for (_, change) in &changes[..=at] {
                let _ = win::delete(&change.staged);
            }
            return Err(error);
        }
        written += file.data.len() as u64;
    }
    if !progress(Step::Bytes(written, total)) {
        for (_, change) in &changes {
            let _ = win::delete(&change.staged);
        }
        return Err(InstallError::Stopped);
    }

    // 3. The places change: the running programs last, this one last of all,
    // so that a failure before them leaves them untouched.
    let rank = |name: &str| {
        let file_name = name.rsplit('/').next().unwrap_or(name);
        if file_name.eq_ignore_ascii_case(&layout.exe_name) {
            2
        } else if file_name.eq_ignore_ascii_case(LAUNCHER_NAME) {
            1
        } else {
            0
        }
    };
    let mut changes: Vec<Change> = changes.into_iter().map(|(_, change)| change).collect();
    changes.sort_by_key(|change| rank(&change.name));
    for at in 0..changes.len() {
        let result = (|| {
            let change = &mut changes[at];
            if win::exists(&change.place) {
                let backup = backup_name(&change.place).ok_or(InstallError::Replace {
                    path: change.place.clone(),
                    code: windows_sys::Win32::Foundation::ERROR_SHARING_VIOLATION,
                })?;
                win::rename(&change.place, &backup, true).map_err(|code| {
                    InstallError::Replace {
                        path: change.place.clone(),
                        code,
                    }
                })?;
                change.backup = Some(backup);
            }
            // From here on an undo puts the old file back.
            change.done = true;
            win::rename(&change.staged, &change.place, false).map_err(|code| {
                InstallError::Replace {
                    path: change.place.clone(),
                    code,
                }
            })
        })();
        if let Err(error) = result {
            undo(&mut changes);
            return Err(error);
        }
    }

    // 4. The files that were replaced go, those that can (a running
    // program's stays until the next start).
    let mut outcome = Outcome::default();
    for change in &changes {
        if let Some(backup) = &change.backup {
            let _ = win::delete(backup);
        }
        outcome.restart |= rank(&change.name) > 0;
        outcome.written.push(change.name.clone());
    }
    Ok(outcome)
}

/// Deletes what earlier runs left in the folders of `names` and in this
/// program's own: `.skhid-new` and `.skhid-old` files. What is still held by
/// a running program stays for the next time.
pub fn clean_up<'a>(layout: &Layout, names: impl Iterator<Item = &'a str>) {
    let mut folders: Vec<String> = alloc::vec![layout.exe_dir.clone()];
    for name in names {
        let place = layout.place(name);
        let (folder, _) = win::split_path(&place);
        if !folders
            .iter()
            .any(|known| known.eq_ignore_ascii_case(folder))
        {
            folders.push(folder.into());
        }
    }
    for folder in &folders {
        for pattern in ["*.skhid-new", "*.skhid-old*"] {
            for name in win::list(folder, pattern) {
                let _ = win::delete(&format!("{folder}\\{name}"));
            }
        }
    }
}
