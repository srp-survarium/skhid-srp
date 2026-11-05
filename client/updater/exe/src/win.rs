//! The few things the updater asks of Windows, each behind a small function
//! that returns the Windows error code when it fails.

use alloc::string::String;
use alloc::vec::Vec;
use core::ptr::{null, null_mut};

use updater_logic::checked::Identity;
use updater_logic::sha256::Sha256;
use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_ALREADY_EXISTS, ERROR_FILE_NOT_FOUND, ERROR_PATH_NOT_FOUND, FILETIME,
    GENERIC_READ, GENERIC_WRITE, GetLastError, HANDLE, INVALID_HANDLE_VALUE, SYSTEMTIME,
};
use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;
use windows_sys::Win32::Storage::FileSystem::{
    BY_HANDLE_FILE_INFORMATION, CREATE_ALWAYS, CreateDirectoryW, CreateFileW, DeleteFileW,
    FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_DELETE, FILE_SHARE_READ,
    FILE_SHARE_WRITE, FindClose, FindFirstFileW, FindNextFileW, FlushFileBuffers,
    GetDiskFreeSpaceExW, GetFileAttributesW, GetFileInformationByHandle, GetFileSizeEx,
    INVALID_FILE_ATTRIBUTES, MOVEFILE_REPLACE_EXISTING, MoveFileExW, OPEN_ALWAYS, OPEN_EXISTING,
    ReadFile, SetFilePointerEx, WIN32_FIND_DATAW, WriteFile,
};
use windows_sys::Win32::System::Diagnostics::Debug::{
    FORMAT_MESSAGE_FROM_SYSTEM, FORMAT_MESSAGE_IGNORE_INSERTS, FormatMessageW,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleFileNameW;
use windows_sys::Win32::System::SystemInformation::{
    GetLocalTime, GetSystemTimeAsFileTime, GetTickCount64,
};
use windows_sys::Win32::System::Threading::{
    CreateProcessW, OpenMutexW, PROCESS_INFORMATION, STARTUPINFOW, Sleep,
};

/// A Windows error code (`GetLastError`).
pub type Code = u32;

/// `text` as Windows wants it: UTF-16 with a zero at its end.
pub fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(core::iter::once(0)).collect()
}

/// UTF-16 up to its first zero, as text.
pub fn from_wide(text: &[u16]) -> String {
    let end = text
        .iter()
        .position(|&unit| unit == 0)
        .unwrap_or(text.len());
    char::decode_utf16(text[..end].iter().copied())
        .map(|unit| unit.unwrap_or(char::REPLACEMENT_CHARACTER))
        .collect()
}

pub fn last_error() -> Code {
    unsafe { GetLastError() }
}

/// What Windows says of an error code, in the system's language, without
/// the line end: "Access is denied".
pub fn error_text(code: Code) -> String {
    let mut buffer = [0_u16; 512];
    let length = unsafe {
        FormatMessageW(
            FORMAT_MESSAGE_FROM_SYSTEM | FORMAT_MESSAGE_IGNORE_INSERTS,
            null(),
            code,
            0,
            buffer.as_mut_ptr(),
            buffer.len() as u32,
            null(),
        )
    } as usize;
    let text = from_wide(&buffer[..length.min(buffer.len())]);
    text.trim_end_matches(['\r', '\n', ' ', '.']).into()
}

/// The full path of this program.
pub fn module_path() -> Result<String, Code> {
    let mut buffer = alloc::vec![0_u16; 1024];
    loop {
        let length =
            unsafe { GetModuleFileNameW(null_mut(), buffer.as_mut_ptr(), buffer.len() as u32) }
                as usize;
        if length == 0 {
            return Err(last_error());
        }
        if length < buffer.len() {
            return Ok(from_wide(&buffer[..length]));
        }
        if buffer.len() >= 32768 {
            return Err(last_error());
        }
        buffer.resize(buffer.len() * 2, 0);
    }
}

/// The folder part of `path` (no `\` at its end), and what follows it.
pub fn split_path(path: &str) -> (&str, &str) {
    match path.rfind(['\\', '/']) {
        Some(at) => (&path[..at], &path[at + 1..]),
        None => ("", path),
    }
}

struct Handle(HANDLE);

impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}

fn open(path: &str, access: u32, share: u32, how: u32) -> Result<Handle, Code> {
    let handle = unsafe {
        CreateFileW(
            wide(path).as_ptr(),
            access,
            share,
            null(),
            how,
            FILE_ATTRIBUTE_NORMAL,
            null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        Err(last_error())
    } else {
        Ok(Handle(handle))
    }
}

/// Whether the error says that the file or its folder is not there.
pub fn is_missing(code: Code) -> bool {
    code == ERROR_FILE_NOT_FOUND || code == ERROR_PATH_NOT_FOUND
}

/// Opens a file for reading, whoever else has it open (a running program's
/// own file too).
fn open_read(path: &str) -> Result<Handle, Code> {
    open(
        path,
        GENERIC_READ,
        FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
        OPEN_EXISTING,
    )
}

fn read_some(file: &Handle, buffer: &mut [u8]) -> Result<usize, Code> {
    let mut read = 0_u32;
    let ok = unsafe {
        ReadFile(
            file.0,
            buffer.as_mut_ptr(),
            buffer.len() as u32,
            &mut read,
            null_mut(),
        )
    };
    if ok == 0 {
        Err(last_error())
    } else {
        Ok(read as usize)
    }
}

/// A whole file, if it is at most `limit` bytes.
pub fn read_file(path: &str, limit: usize) -> Result<Vec<u8>, Code> {
    let file = open_read(path)?;
    let mut size = 0_i64;
    if unsafe { GetFileSizeEx(file.0, &mut size) } == 0 {
        return Err(last_error());
    }
    if size < 0 || size as u64 > limit as u64 {
        return Err(windows_sys::Win32::Foundation::ERROR_FILE_TOO_LARGE);
    }
    let mut data = alloc::vec![0_u8; size as usize];
    let mut filled = 0;
    while filled < data.len() {
        match read_some(&file, &mut data[filled..])? {
            0 => break,
            read => filled += read,
        }
    }
    data.truncate(filled);
    Ok(data)
}

/// A file's size and SHA-256, read in pieces.
pub fn hash_file(path: &str) -> Result<(u64, [u8; 32]), Code> {
    hash_file_with(path, &mut |_| true)
}

/// The same, `progress(bytes read)` after each piece: `false` from it stops
/// the hash with `ERROR_CANCELLED`.
pub fn hash_file_with(
    path: &str,
    progress: &mut dyn FnMut(u64) -> bool,
) -> Result<(u64, [u8; 32]), Code> {
    hash_file_identity(path, progress).map(|(size, sha256, _)| (size, sha256))
}

fn filetime(time: &FILETIME) -> u64 {
    (u64::from(time.dwHighDateTime) << 32) | u64::from(time.dwLowDateTime)
}

/// What Windows says of an open file: its size, last-write time and file
/// ID (`updater_logic::checked`).
fn identity_of(file: &Handle) -> Result<Identity, Code> {
    let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { core::mem::zeroed() };
    if unsafe { GetFileInformationByHandle(file.0, &mut info) } == 0 {
        return Err(last_error());
    }
    Ok(Identity {
        size: (u64::from(info.nFileSizeHigh) << 32) | u64::from(info.nFileSizeLow),
        written: filetime(&info.ftLastWriteTime),
        volume: info.dwVolumeSerialNumber,
        index: (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow),
    })
}

/// What Windows says of a file, without reading it.
pub fn identity(path: &str) -> Result<Identity, Code> {
    identity_of(&open_read(path)?)
}

/// [`hash_file_with`], and what Windows says of the file before and after
/// the read: `None` when that changed meanwhile (someone wrote to it) or
/// cannot be told.
pub fn hash_file_identity(
    path: &str,
    progress: &mut dyn FnMut(u64) -> bool,
) -> Result<(u64, [u8; 32], Option<Identity>), Code> {
    let file = open_read(path)?;
    let before = identity_of(&file).ok();
    let mut hash = Sha256::new();
    let mut buffer = alloc::vec![0_u8; 64 * 1024];
    let mut size = 0_u64;
    loop {
        match read_some(&file, &mut buffer)? {
            0 => break,
            read => {
                hash.update(&buffer[..read]);
                size += read as u64;
                if !progress(size) {
                    return Err(windows_sys::Win32::Foundation::ERROR_CANCELLED);
                }
            }
        }
    }
    let after = identity_of(&file).ok();
    let identity = before.filter(|before| Some(*before) == after && before.size == size);
    Ok((size, hash.finish(), identity))
}

/// The time now, in Windows time (100 ns since 1601, UTC).
pub fn now() -> u64 {
    let mut time = FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };
    unsafe { GetSystemTimeAsFileTime(&mut time) };
    filetime(&time)
}

fn write_all(file: &Handle, mut data: &[u8]) -> Result<(), Code> {
    while !data.is_empty() {
        let mut written = 0_u32;
        let piece = data.len().min(1 << 20) as u32;
        if unsafe { WriteFile(file.0, data.as_ptr(), piece, &mut written, null_mut()) } == 0 {
            return Err(last_error());
        }
        if written == 0 {
            return Err(windows_sys::Win32::Foundation::ERROR_WRITE_FAULT);
        }
        data = &data[written as usize..];
    }
    Ok(())
}

/// Writes a new file (over one of the same name) and has Windows put it on
/// the disk before returning.
pub fn write_file(path: &str, data: &[u8]) -> Result<(), Code> {
    let file = open(path, GENERIC_WRITE, 0, CREATE_ALWAYS)?;
    write_all(&file, data)?;
    if unsafe { FlushFileBuffers(file.0) } == 0 {
        return Err(last_error());
    }
    Ok(())
}

/// Adds to the end of a file, making it if it is not there. A file longer
/// than `limit` is started anew first.
pub fn append_file(path: &str, data: &[u8], limit: u64) -> Result<(), Code> {
    let file = open(path, GENERIC_WRITE, FILE_SHARE_READ, OPEN_ALWAYS)?;
    let mut size = 0_i64;
    if unsafe { GetFileSizeEx(file.0, &mut size) } != 0 && size as u64 > limit {
        drop(file);
        return write_file(path, data);
    }
    // 2 = FILE_END.
    if unsafe { SetFilePointerEx(file.0, 0, null_mut(), 2) } == 0 {
        return Err(last_error());
    }
    write_all(&file, data)
}

pub fn exists(path: &str) -> bool {
    unsafe { GetFileAttributesW(wide(path).as_ptr()) != INVALID_FILE_ATTRIBUTES }
}

pub fn is_folder(path: &str) -> bool {
    let attributes = unsafe { GetFileAttributesW(wide(path).as_ptr()) };
    attributes != INVALID_FILE_ATTRIBUTES && attributes & FILE_ATTRIBUTE_DIRECTORY != 0
}

/// Deletes a file; one that is not there is no error.
pub fn delete(path: &str) -> Result<(), Code> {
    if unsafe { DeleteFileW(wide(path).as_ptr()) } != 0 {
        return Ok(());
    }
    match last_error() {
        code if is_missing(code) => Ok(()),
        code => Err(code),
    }
}

/// Renames a file within its disk, over `to` when `replace`.
pub fn rename(from: &str, to: &str, replace: bool) -> Result<(), Code> {
    let flags = if replace {
        MOVEFILE_REPLACE_EXISTING
    } else {
        0
    };
    if unsafe { MoveFileExW(wide(from).as_ptr(), wide(to).as_ptr(), flags) } == 0 {
        Err(last_error())
    } else {
        Ok(())
    }
}

/// Makes the folder and those above it that are missing.
pub fn create_folders(path: &str) -> Result<(), Code> {
    if is_folder(path) {
        return Ok(());
    }
    let (parent, _) = split_path(path);
    if !parent.is_empty() && !parent.ends_with(':') && parent != path {
        create_folders(parent)?;
    }
    if unsafe { CreateDirectoryW(wide(path).as_ptr(), null()) } == 0 {
        let code = last_error();
        if code != ERROR_ALREADY_EXISTS {
            return Err(code);
        }
    }
    Ok(())
}

/// The bytes this user may still write in `folder`'s disk.
pub fn free_space(folder: &str) -> Option<u64> {
    let mut free = 0_u64;
    let ok =
        unsafe { GetDiskFreeSpaceExW(wide(folder).as_ptr(), &mut free, null_mut(), null_mut()) };
    (ok != 0).then_some(free)
}

/// The names in `folder` that match `pattern` (`*.skhid-old*`), files only.
pub fn list(folder: &str, pattern: &str) -> Vec<String> {
    find(folder, pattern, false)
}

/// The folders in `folder` whose names match `pattern`.
pub fn list_folders(folder: &str, pattern: &str) -> Vec<String> {
    find(folder, pattern, true)
}

fn find(folder: &str, pattern: &str, folders: bool) -> Vec<String> {
    let mut names = Vec::new();
    let mut found: WIN32_FIND_DATAW = unsafe { core::mem::zeroed() };
    let query = alloc::format!("{folder}\\{pattern}");
    let handle = unsafe { FindFirstFileW(wide(&query).as_ptr(), &mut found) };
    if handle == INVALID_HANDLE_VALUE {
        return names;
    }
    loop {
        if (found.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY != 0) == folders {
            names.push(from_wide(&found.cFileName));
        }
        if unsafe { FindNextFileW(handle, &mut found) } == 0 {
            break;
        }
    }
    unsafe {
        FindClose(handle);
    }
    names
}

pub fn sleep(milliseconds: u32) {
    unsafe { Sleep(milliseconds) }
}

/// Milliseconds since the system started.
pub fn ticks() -> u64 {
    unsafe { GetTickCount64() }
}

/// The local date and time, `2026-10-13 02:19:27`.
pub fn timestamp() -> String {
    let mut time: SYSTEMTIME = unsafe { core::mem::zeroed() };
    unsafe { GetLocalTime(&mut time) };
    alloc::format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        time.wYear,
        time.wMonth,
        time.wDay,
        time.wHour,
        time.wMinute,
        time.wSecond
    )
}

/// Whether a mutex of this name exists now (some process holds it open).
pub fn mutex_exists(name: &str) -> bool {
    // 0x0010_0000 = SYNCHRONIZE, the least right that opens it.
    let handle = unsafe { OpenMutexW(0x0010_0000, 0, wide(name).as_ptr()) };
    if handle.is_null() {
        return false;
    }
    unsafe {
        CloseHandle(handle);
    }
    true
}

/// Starts a program with `folder` as its working folder; does not wait.
pub fn start(application: &str, command: &str, folder: &str) -> Result<(), Code> {
    let mut startup: STARTUPINFOW = unsafe { core::mem::zeroed() };
    startup.cb = core::mem::size_of::<STARTUPINFOW>() as u32;
    let mut process: PROCESS_INFORMATION = unsafe { core::mem::zeroed() };
    let mut command = wide(command);
    let ok = unsafe {
        CreateProcessW(
            wide(application).as_ptr(),
            command.as_mut_ptr(),
            null(),
            null(),
            0,
            0,
            null(),
            wide(folder).as_ptr(),
            &startup,
            &mut process,
        )
    };
    if ok == 0 {
        return Err(last_error());
    }
    unsafe {
        CloseHandle(process.hThread);
        CloseHandle(process.hProcess);
    }
    Ok(())
}

/// Keeps a download closed to writers/deletion from verification through extraction.
pub struct LockedFile(Handle);

impl LockedFile {
    pub fn open(path: &str) -> Result<Self, Code> {
        open(path, GENERIC_READ, FILE_SHARE_READ, OPEN_EXISTING).map(Self)
    }

    pub fn verify(&self, progress: &mut dyn FnMut(u64) -> bool) -> Result<(u64, [u8; 32]), Code> {
        let mut hash = Sha256::new();
        let mut buffer = alloc::vec![0; 64 * 1024];
        let mut size = 0;
        loop {
            if !progress(size) {
                return Err(windows_sys::Win32::Foundation::ERROR_CANCELLED);
            }
            let read = read_some(&self.0, &mut buffer)?;
            if read == 0 {
                return Ok((size, hash.finish()));
            }
            hash.update(&buffer[..read]);
            size += read as u64;
        }
    }
}

pub struct DownloadFile(Handle);
impl DownloadFile {
    pub fn create(path: &str) -> Result<Self, Code> {
        open(path, GENERIC_WRITE, 0, CREATE_ALWAYS).map(Self)
    }
    pub fn write(&self, bytes: &[u8]) -> Result<(), Code> {
        write_all(&self.0, bytes)
    }
    pub fn flush(&self) -> Result<(), Code> {
        if unsafe { FlushFileBuffers(self.0.0) } == 0 {
            Err(last_error())
        } else {
            Ok(())
        }
    }
}

/// An open file of the restore of resources.db and of the patch step, read
/// and written at 64-bit offsets.
pub struct File(Handle);

impl File {
    /// For reading and writing by this program alone. While another program
    /// has the file open (the game its `resources.db`), this fails with
    /// `ERROR_SHARING_VIOLATION`; while this is open, no other program can
    /// open it.
    pub fn exclusive(path: &str) -> Result<Self, Code> {
        open(path, GENERIC_READ | GENERIC_WRITE, 0, OPEN_EXISTING).map(Self)
    }

    /// For reading by this program alone: as [`File::exclusive`], without
    /// asking to write (a read-only file opens too).
    pub fn alone(path: &str) -> Result<Self, Code> {
        open(path, GENERIC_READ, 0, OPEN_EXISTING).map(Self)
    }

    /// For reading, whoever else has it open (the game too).
    pub fn shared(path: &str) -> Result<Self, Code> {
        open_read(path).map(Self)
    }

    /// For reading; others may read it too, but no one may change it while
    /// it is open.
    pub fn locked(path: &str) -> Result<Self, Code> {
        open(path, GENERIC_READ, FILE_SHARE_READ, OPEN_EXISTING).map(Self)
    }

    /// A new file (over one of the same name), to write and read back, by
    /// this program alone.
    pub fn create(path: &str) -> Result<Self, Code> {
        open(path, GENERIC_READ | GENERIC_WRITE, 0, CREATE_ALWAYS).map(Self)
    }

    pub fn size(&self) -> Result<u64, Code> {
        let mut size = 0_i64;
        if unsafe { GetFileSizeEx(self.0.0, &mut size) } == 0 {
            return Err(last_error());
        }
        Ok(size as u64)
    }

    /// Moves to byte `at` from the start: a 64-bit offset, past 4 GB too.
    pub fn seek(&self, at: u64) -> Result<(), Code> {
        // 0 = FILE_BEGIN.
        if unsafe { SetFilePointerEx(self.0.0, at as i64, null_mut(), 0) } == 0 {
            return Err(last_error());
        }
        Ok(())
    }

    pub fn read(&self, buffer: &mut [u8]) -> Result<usize, Code> {
        read_some(&self.0, buffer)
    }

    pub fn write(&self, data: &[u8]) -> Result<(), Code> {
        write_all(&self.0, data)
    }

    /// Has Windows put what was written on the disk.
    pub fn flush(&self) -> Result<(), Code> {
        if unsafe { FlushFileBuffers(self.0.0) } == 0 {
            return Err(last_error());
        }
        Ok(())
    }
}

/// Serializes installers in the same game folder, including while downloading.
pub struct InstallLock(#[allow(dead_code)] Handle);
pub fn install_lock(folder: &str) -> Result<InstallLock, Code> {
    lock(folder, "updater_skhid.lock")
}

/// A file held open by this program alone while it lives: a second program
/// that asks for the same one is refused (`ERROR_SHARING_VIOLATION`).
pub fn lock(folder: &str, name: &str) -> Result<InstallLock, Code> {
    open(
        &alloc::format!("{folder}\\{name}"),
        GENERIC_WRITE,
        0,
        OPEN_ALWAYS,
    )
    .map(InstallLock)
}

/// The files in `folder` that match `pattern`, each with the time it was
/// last written (100 ns since 1601).
pub fn list_with_times(folder: &str, pattern: &str) -> Vec<(String, u64)> {
    let mut out = Vec::new();
    let mut found: WIN32_FIND_DATAW = unsafe { core::mem::zeroed() };
    let query = alloc::format!("{folder}\\{pattern}");
    let handle = unsafe { FindFirstFileW(wide(&query).as_ptr(), &mut found) };
    if handle == INVALID_HANDLE_VALUE {
        return out;
    }
    loop {
        if found.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY == 0 {
            let time = u64::from(found.ftLastWriteTime.dwHighDateTime) << 32
                | u64::from(found.ftLastWriteTime.dwLowDateTime);
            out.push((from_wide(&found.cFileName), time));
        }
        if unsafe { FindNextFileW(handle, &mut found) } == 0 {
            break;
        }
    }
    unsafe {
        FindClose(handle);
    }
    out
}

/// The game's language, which its Settings and the launcher's menu keep
/// (`localization` of `<Documents>\survarium\user.cfg`, the folder the game
/// takes: CSIDL_PERSONAL), as a number of `replay::LANGUAGES`; read only.
pub fn game_language() -> Option<u32> {
    use windows_sys::Win32::UI::Shell::SHGetFolderPathW;
    const CSIDL_PERSONAL: i32 = 5;
    let mut folder = [0_u16; 260];
    let status = unsafe { SHGetFolderPathW(null_mut(), CSIDL_PERSONAL, null_mut(), 0, folder.as_mut_ptr()) };
    if status != 0 {
        return None;
    }
    let end = folder.iter().position(|&unit| unit == 0)?;
    let path = alloc::format!("{}\\survarium\\user.cfg", from_wide(&folder[..end]));
    updater_logic::replay::language_of_user_cfg(&read_file(&path, 1 << 16).ok()?)
}

/// Whether the game, which reads its command line as ANSI text
/// (`GetCommandLineA`), gets `text` back as it is, and in how many bytes.
fn ansi_length(text: &str) -> Option<usize> {
    use windows_sys::Win32::Globalization::{CP_ACP, CP_UTF8, GetACP, WC_NO_BEST_FIT_CHARS, WideCharToMultiByte};
    let units: Vec<u16> = text.encode_utf16().collect();
    if units.is_empty() {
        return Some(0);
    }
    if unsafe { GetACP() } == CP_UTF8 {
        // The system's ANSI code page is UTF-8: every character is kept.
        return Some(text.len());
    }
    let mut lost = 0;
    let length = unsafe {
        WideCharToMultiByte(
            CP_ACP,
            WC_NO_BEST_FIT_CHARS,
            units.as_ptr(),
            units.len() as i32,
            null_mut(),
            0,
            null(),
            &mut lost,
        )
    };
    (length > 0 && lost == 0).then_some(length as usize)
}

/// The path of a file as the game can be given it on its command line: the
/// path itself when the ANSI code page holds every character of it and it
/// is at most `limit` bytes so, else its short (8.3) form when that is.
pub fn game_path(path: &str, limit: usize) -> Option<String> {
    use windows_sys::Win32::Storage::FileSystem::GetShortPathNameW;
    if ansi_length(path).is_some_and(|length| length <= limit) {
        return Some(path.into());
    }
    let mut buffer = alloc::vec![0_u16; 1024];
    let length = unsafe { GetShortPathNameW(wide(path).as_ptr(), buffer.as_mut_ptr(), buffer.len() as u32) }
        as usize;
    if length == 0 || length >= buffer.len() {
        return None;
    }
    let short = from_wide(&buffer[..length]);
    ansi_length(&short)
        .is_some_and(|length| length <= limit)
        .then_some(short)
}

pub fn file_size(path: &str) -> Result<u64, Code> {
    let file = open_read(path)?;
    let mut size = 0_i64;
    if unsafe { GetFileSizeEx(file.0, &mut size) } == 0 {
        return Err(last_error());
    }
    Ok(size as u64)
}

/// Makes a fresh extraction directory. It never reuses a directory from a previous run.
pub fn staging_dir(parent: &str) -> Result<String, Code> {
    for n in 0..100 {
        let path = alloc::format!("{parent}\\.skhid-extract-{}-{n}", ticks());
        if unsafe { CreateDirectoryW(wide(&path).as_ptr(), null()) } != 0 {
            return Ok(path);
        }
        let code = last_error();
        if code != ERROR_ALREADY_EXISTS {
            return Err(code);
        }
    }
    Err(ERROR_ALREADY_EXISTS)
}

pub fn remove_dir(path: &str) {
    unsafe {
        windows_sys::Win32::Storage::FileSystem::RemoveDirectoryW(wide(path).as_ptr());
    }
}

/// Puts `process` into a job that Windows ends when its last handle closes,
/// and returns that handle. This program holds the only one: if it is killed
/// while the extractor runs, the extractor goes with it, and does not write
/// gigabytes on into a folder nobody waits for (the next start removes what
/// it left). `None` when Windows refuses (a process already in a job that
/// allows no other, before Windows 8): the extractor then runs as it did.
fn ends_with_us(process: &Handle) -> Option<Handle> {
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        SetInformationJobObject,
    };
    let job = unsafe { CreateJobObjectW(null(), null()) };
    if job.is_null() {
        return None;
    }
    let job = Handle(job);
    let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { core::mem::zeroed() };
    limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    let set = unsafe {
        SetInformationJobObject(
            job.0,
            JobObjectExtendedLimitInformation,
            (&raw const limits).cast(),
            core::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        )
    };
    (set != 0 && unsafe { AssignProcessToJobObject(job.0, process.0) } != 0).then_some(job)
}

/// A new file (over one of the same name) that a program started by this
/// one may write to as its output; others may read it meanwhile.
fn open_output(path: &str) -> Result<Handle, Code> {
    let inherited = SECURITY_ATTRIBUTES {
        nLength: core::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: null_mut(),
        bInheritHandle: 1,
    };
    let handle = unsafe {
        CreateFileW(
            wide(path).as_ptr(),
            GENERIC_WRITE,
            FILE_SHARE_READ,
            &inherited,
            CREATE_ALWAYS,
            FILE_ATTRIBUTE_NORMAL,
            null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        Err(last_error())
    } else {
        Ok(Handle(handle))
    }
}

/// The end of a file: its last `limit` bytes, or all of it when it is
/// shorter.
pub fn read_tail(path: &str, limit: usize) -> Result<Vec<u8>, Code> {
    let file = open_read(path)?;
    let mut size = 0_i64;
    if unsafe { GetFileSizeEx(file.0, &mut size) } == 0 {
        return Err(last_error());
    }
    let skipped = (size.max(0) as u64).saturating_sub(limit as u64);
    // 0 = FILE_BEGIN.
    if unsafe { SetFilePointerEx(file.0, skipped as i64, null_mut(), 0) } == 0 {
        return Err(last_error());
    }
    let mut data = alloc::vec![0_u8; limit];
    let mut filled = 0;
    while filled < data.len() {
        match read_some(&file, &mut data[filled..])? {
            0 => break,
            read => filled += read,
        }
    }
    data.truncate(filled);
    Ok(data)
}

/// Runs the pinned extractor, pumping cancellation while it works. A cancelled
/// process is terminated and reaped before the caller removes staging files.
/// What the extractor prints (its messages and its errors) goes to the file
/// `output`; when that file cannot be made, the extractor runs all the same
/// and what it prints is lost.
pub fn run_wait(
    application: &str,
    command: &str,
    folder: &str,
    output: &str,
    pump: &mut dyn FnMut() -> bool,
) -> Result<u32, Code> {
    use windows_sys::Win32::System::Threading::{
        CREATE_NO_WINDOW, GetExitCodeProcess, STARTF_USESTDHANDLES, TerminateProcess,
        WaitForSingleObject,
    };
    let mut startup: STARTUPINFOW = unsafe { core::mem::zeroed() };
    startup.cb = core::mem::size_of::<STARTUPINFOW>() as u32;
    // Closed when this function returns: the caller then reads the file.
    let output = open_output(output).ok();
    if let Some(output) = &output {
        startup.dwFlags = STARTF_USESTDHANDLES;
        startup.hStdOutput = output.0;
        startup.hStdError = output.0;
    }
    let mut process: PROCESS_INFORMATION = unsafe { core::mem::zeroed() };
    let mut command = wide(command);
    if unsafe {
        CreateProcessW(
            wide(application).as_ptr(),
            command.as_mut_ptr(),
            null(),
            null(),
            // Of the handles this program opens, only the output file can
            // be inherited.
            i32::from(output.is_some()),
            CREATE_NO_WINDOW,
            null(),
            wide(folder).as_ptr(),
            &startup,
            &mut process,
        )
    } == 0
    {
        return Err(last_error());
    }
    let process_handle = Handle(process.hProcess);
    unsafe {
        CloseHandle(process.hThread);
    }
    // Held to the end of this function: see `ends_with_us`.
    let _job = ends_with_us(&process_handle);
    loop {
        let result = unsafe { WaitForSingleObject(process_handle.0, 100) };
        if result == 0 {
            break;
        }
        if result != windows_sys::Win32::Foundation::WAIT_TIMEOUT {
            return Err(last_error());
        }
        if !pump() {
            unsafe {
                TerminateProcess(process_handle.0, 1);
                WaitForSingleObject(process_handle.0, u32::MAX);
            }
            return Err(windows_sys::Win32::Foundation::ERROR_CANCELLED);
        }
    }
    let mut code = 0;
    if unsafe { GetExitCodeProcess(process_handle.0, &mut code) } == 0 {
        return Err(last_error());
    }
    Ok(code)
}
