//! A patch's opt-in diagnostics: lines to a file of its own in the game's
//! user data folder (Documents\survarium), only in a build made for it.
//!
//! [`DIAGNOSTICS`] is off in every build; patch_026g.py's `--diagnostics`
//! (or `--ragdoll-trace`) makes it 1 in a local build for us (rust_patches.md; never a release's:
//! release/build-zip does not pass it, and the self-test checks the default
//! build has it off).

use super::hook::Thread;
use crate::raw;
use core::ffi::c_void;
use core::ptr::null_mut;

/// The diagnostics' switch: [`DIAGNOSTICS_OFF`] in every build, 1 in a
/// diagnostics build (its address is the arena's symbol `SRP_DIAGNOSTICS`).
#[unsafe(no_mangle)]
pub static SRP_DIAGNOSTICS: u32 = DIAGNOSTICS_OFF;
/// (not 0: a zero static would be the arena's state, not its data)
pub const DIAGNOSTICS_OFF: u32 = u32::from_be_bytes(*b"OFF\0");

/// Whether this build writes the diagnostics.
pub fn enabled() -> bool {
    // SAFETY: a static of ours; patch_026g.py writes it in the image only.
    unsafe { core::ptr::read_volatile(&raw const SRP_DIAGNOSTICS) == 1 }
}

/// A file the diagnostics write to, opened for appending: kernel32's handle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TraceFile(*mut c_void);

// SAFETY: a Windows file handle is process-wide, not thread-affine. Access
// to the ragdoll's owned trace file is serialized by its Lock<State>.
unsafe impl Send for TraceFile {}

/// The longest path a trace file has.
const PATH: usize = 300;

/// `name` in the user data folder (Documents\survarium), 0-terminated
/// (zeros past it); None without the folder or with no room for the name.
fn path_of(name: &str) -> Option<[u8; PATH]> {
    let mut path = [0u8; PATH];
    let mut n = 0;
    // SAFETY: the game's own folder, a C string it made once and keeps.
    let folder = unsafe { raw::engine_user_data_directory() };
    if folder.is_null() || name.len() + 2 >= PATH {
        return None;
    }
    // SAFETY: a C string; read up to its 0 or our room.
    unsafe {
        while n < PATH - name.len() - 2 && *folder.add(n) != 0 {
            path[n] = *folder.add(n);
            n += 1;
        }
    }
    path[n] = b'\\';
    path[n + 1..n + 1 + name.len()].copy_from_slice(name.as_bytes());
    Some(path)
}

impl TraceFile {
    /// `name` in the user data folder, opened (made if missing) to append;
    /// None when it cannot be.
    pub fn open(_: &Thread, name: &str) -> Option<TraceFile> {
        let path = path_of(name)?;
        // SAFETY: the exe's import; a 0-terminated path (`path` is zeros past
        // the name): FILE_APPEND_DATA, shared for reading and writing,
        // OPEN_ALWAYS, normal.
        let handle = unsafe { (raw::engine_create_file_a)(path.as_ptr(), 4, 3, null_mut(), 4, 0x80, null_mut()) };
        // INVALID_HANDLE_VALUE is -1
        (!handle.is_null() && handle as usize != usize::MAX).then_some(TraceFile(handle))
    }

    /// Writes `bytes` at the file's end.
    pub fn write(&self, _: &Thread, bytes: &[u8]) {
        let mut written = 0u32;
        // SAFETY: the exe's import on a handle `open` made.
        unsafe {
            (raw::engine_write_file)(self.0, bytes.as_ptr().cast(), bytes.len() as u32,
                                     &mut written, null_mut())
        };
    }
}

/// CreateFileA's GENERIC_READ (the top bit).
const GENERIC_READ: u32 = 1 << 31;

/// A diagnostics build's input file of the user data folder (Documents\survarium\`name`), read
/// whole into `into`: its length, None when it is not there (or none of it could be read).
pub fn read_input(_: &Thread, name: &str, into: &mut [u8]) -> Option<usize> {
    let path = path_of(name)?;
    // SAFETY: the exe's import; a 0-terminated path: GENERIC_READ, shared for reading and writing,
    // OPEN_EXISTING, normal.
    let handle = unsafe { (raw::engine_create_file_a)(path.as_ptr(), GENERIC_READ, 3, null_mut(), 3, 0x80, null_mut()) };
    if handle.is_null() || handle as usize == usize::MAX {
        return None;
    }
    let mut read = 0u32;
    // SAFETY: the exe's imports on the handle just made; `into` is ours, its length the most read.
    let ok = unsafe {
        let ok = (raw::engine_read_file)(handle, into.as_mut_ptr().cast(), into.len() as u32, &mut read, null_mut());
        (raw::engine_close_handle)(handle);
        ok
    };
    (ok != 0 && read > 0).then_some(read as usize)
}

/// Appends `row` to the table `name` of the user data folder, after its
/// `header` when the file is new (or empty), and closes it: whether all of
/// it was written.
pub fn append_row(_: &Thread, name: &str, header: &[u8], row: &[u8]) -> bool {
    let Some(path) = path_of(name) else { return false };
    // SAFETY: the exe's import; a 0-terminated path: FILE_APPEND_DATA, shared
    // for reading and writing, OPEN_ALWAYS, normal.
    let handle = unsafe { (raw::engine_create_file_a)(path.as_ptr(), 4, 3, null_mut(), 4, 0x80, null_mut()) };
    if handle.is_null() || handle as usize == usize::MAX {
        return false;
    }
    let write = |bytes: &[u8]| {
        let mut written = 0u32;
        // SAFETY: the exe's import on the handle just made; the bytes are ours.
        let ok = unsafe {
            (raw::engine_write_file)(handle, bytes.as_ptr().cast(), bytes.len() as u32, &mut written, null_mut())
        };
        ok != 0 && written as usize == bytes.len()
    };
    let mut high = 0u32;
    // SAFETY: the exe's imports on the handle just made, closed last.
    let empty = unsafe { (raw::engine_get_file_size)(handle, &mut high) } == 0 && high == 0;
    let ok = (!empty || write(header)) && write(row);
    // SAFETY: as above.
    unsafe { (raw::engine_close_handle)(handle) };
    ok
}
