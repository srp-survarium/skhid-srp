//! Files, through kernel32 as the exe imports it (its IAT slots, `raw`):
//! open, read, write and close a file, delete one, make a folder, and walk
//! a folder's names (FindFirstFileA / FindNextFileA). A path is a C string
//! the patch built ([`Path`]: its bytes up to a NUL); what Windows returns
//! is typed here. The tick count (GetTickCount) is here too: kernel32's.
//!
//! The calls are made in the order a patch makes them: a [`File`] or a
//! [`Find`] is closed when it is dropped, so a patch drops it where the
//! close belongs.

use core::ffi::c_void;

use crate::raw;

/// A C string a patch built: bytes up to (and with) a NUL.
#[derive(Clone, Copy, Debug)]
pub struct Path<'a>(&'a [u8]);

impl<'a> Path<'a> {
    /// The empty path.
    pub const EMPTY: Path<'static> = Path(b"\0");

    /// The path of `bytes` up to its first NUL; none without one.
    pub fn new(bytes: &'a [u8]) -> Option<Path<'a>> {
        bytes.contains(&0).then_some(Path(bytes))
    }

    fn ptr(self) -> *const u8 {
        self.0.as_ptr()
    }
}

/// CreateFileA's INVALID_HANDLE_VALUE, FindFirstFileA's none.
const INVALID: usize = usize::MAX;
const GENERIC_READ: u32 = 1 << 31;
const GENERIC_WRITE: u32 = 1 << 30;
const FILE_SHARE_READ: u32 = 1;
const CREATE_ALWAYS: u32 = 2;
const OPEN_EXISTING: u32 = 3;
const FILE_ATTRIBUTE_NORMAL: u32 = 0x80;
const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x10;

/// An open file (CreateFileA's handle), closed (CloseHandle) when dropped.
pub struct File(*mut c_void);

impl File {
    /// An existing file, for reading (others may read it too).
    pub fn open(path: Path) -> Option<File> {
        File::create(path, GENERIC_READ, OPEN_EXISTING)
    }

    /// The file made anew (emptied when it was there), for writing.
    pub fn create_always(path: Path) -> Option<File> {
        File::create(path, GENERIC_WRITE, CREATE_ALWAYS)
    }

    fn create(path: Path, access: u32, disposition: u32) -> Option<File> {
        // SAFETY: the exe's import (its IAT slot), a NUL-ended path (Path),
        // no security attributes and no template.
        let handle = unsafe {
            raw::engine_create_file_a(path.ptr(), access, FILE_SHARE_READ, core::ptr::null_mut(), disposition,
                                      FILE_ATTRIBUTE_NORMAL, core::ptr::null_mut())
        };
        (handle as usize != INVALID).then(|| File(handle))
    }

    /// Reads up to `into.len()` bytes: how many it read, none when ReadFile
    /// failed.
    pub fn read(&mut self, into: &mut [u8]) -> Option<u32> {
        let mut got = 0;
        // SAFETY: an open handle, `into` writable for its length, `got` ours.
        let done = unsafe {
            raw::engine_read_file(self.0, into.as_mut_ptr().cast(), into.len() as u32, &mut got, core::ptr::null_mut())
        };
        (done != 0).then_some(got)
    }

    /// Writes `bytes` (what WriteFile says is not asked).
    pub fn write(&mut self, bytes: &[u8]) {
        let mut wrote = 0;
        // SAFETY: an open handle, `bytes` readable for its length.
        unsafe {
            raw::engine_write_file(self.0, bytes.as_ptr().cast(), bytes.len() as u32, &mut wrote, core::ptr::null_mut());
        }
    }
}

impl Drop for File {
    fn drop(&mut self) {
        // SAFETY: the handle CreateFileA gave, closed once.
        unsafe {
            raw::engine_close_handle(self.0);
        }
    }
}

/// Deletes a file: whether it went.
pub fn delete(path: Path) -> bool {
    // SAFETY: the exe's import, a NUL-ended path.
    unsafe { raw::engine_delete_file_a(path.ptr()) != 0 }
}

/// Makes a folder (none of its parents; what CreateDirectoryA says is not
/// asked).
pub fn create_folder(path: Path) {
    // SAFETY: the exe's import, a NUL-ended path, no security attributes.
    unsafe {
        raw::engine_create_directory_a(path.ptr(), core::ptr::null_mut());
    }
}

/// The ms since Windows started (GetTickCount; wraps after 49 days).
pub fn tick_count() -> u32 {
    // SAFETY: the exe's import, no arguments.
    unsafe { raw::engine_get_tick_count() }
}

/// A WIN32_FIND_DATAA: what FindFirstFileA and FindNextFileA write of a
/// name a folder holds (320 bytes).
pub struct FindData([u8; 320]);

impl FindData {
    pub const fn new() -> FindData {
        FindData([0; 320])
    }

    fn word(&self, at: usize) -> u32 {
        u32::from_le_bytes([self.0[at], self.0[at + 1], self.0[at + 2], self.0[at + 3]])
    }

    /// A folder, not a file (dwFileAttributes's FILE_ATTRIBUTE_DIRECTORY).
    pub fn is_folder(&self) -> bool {
        self.word(0) & FILE_ATTRIBUTE_DIRECTORY != 0
    }

    /// nFileSizeHigh: not 0 for a file of 4 GiB or more.
    pub fn size_high(&self) -> u32 {
        self.word(28)
    }

    /// nFileSizeLow.
    pub fn size(&self) -> u32 {
        self.word(32)
    }

    /// cFileName: the name, up to its NUL.
    pub fn name(&self) -> &[u8] {
        let name = &self.0[44..];
        &name[..name.iter().position(|&b| b == 0).unwrap_or(name.len())]
    }
}

impl Default for FindData {
    fn default() -> FindData {
        FindData::new()
    }
}

/// A walk of the names a pattern matches (FindFirstFileA's handle), closed
/// (FindClose) when dropped.
pub struct Find(*mut c_void);

impl Find {
    /// The first name `pattern` matches, in `data`; none when none does.
    pub fn first(pattern: Path, data: &mut FindData) -> Option<Find> {
        // SAFETY: the exe's import, a NUL-ended pattern, `data` the 320
        // bytes of a WIN32_FIND_DATAA.
        let handle = unsafe { raw::engine_find_first_file_a(pattern.ptr(), data.0.as_mut_ptr().cast()) };
        (handle as usize != INVALID).then(|| Find(handle))
    }

    /// The next name, in `data`: whether there was one.
    pub fn next(&mut self, data: &mut FindData) -> bool {
        // SAFETY: the handle FindFirstFileA gave, `data` as first's.
        unsafe { raw::engine_find_next_file_a(self.0, data.0.as_mut_ptr().cast()) != 0 }
    }
}

impl Drop for Find {
    fn drop(&mut self) {
        // SAFETY: the handle FindFirstFileA gave, closed once.
        unsafe {
            raw::engine_find_close(self.0);
        }
    }
}
