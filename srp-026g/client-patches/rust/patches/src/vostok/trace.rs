//! A patch's opt-in diagnostics: lines to a file of its own in the game's
//! user data folder (Documents\survarium), only in a build made for it.
//!
//! [`DIAGNOSTICS`] is off in every build; patch_026g.py's `--ragdoll-trace`
//! makes it 1 in a local build for us (rust_patches.md; never a release's:
//! release/build-zip does not pass it, and the self-test checks the default
//! build has it off).

use super::hook::Thread;
use crate::raw;
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

/// A file the diagnostics write to, opened for appending.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TraceFile(u32);

/// The longest path a trace file has.
const PATH: usize = 300;

impl TraceFile {
    /// `name` in the user data folder, opened (made if missing) to append;
    /// None when it cannot be.
    pub fn open(_: &Thread, name: &str) -> Option<TraceFile> {
        let mut path = [0u8; PATH];
        let mut n = 0;
        // SAFETY: the game's own folder, a C string it made once and keeps.
        let folder = unsafe { raw::engine_user_data_directory() };
        if folder.is_null() {
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
        // SAFETY: the exe's import; a 0-terminated path (`path` is zeros past
        // the name): FILE_APPEND_DATA, shared for reading and writing,
        // OPEN_ALWAYS, normal.
        let handle =
            unsafe { (raw::engine_create_file_a)(path.as_ptr(), 4, 3, null_mut(), 4, 0x80, null_mut()) } as u32;
        (handle != 0 && handle != u32::MAX).then_some(TraceFile(handle))
    }

    /// Writes `bytes` at the file's end.
    pub fn write(&self, _: &Thread, bytes: &[u8]) {
        let mut written = 0u32;
        // SAFETY: the exe's import on a handle `open` made.
        unsafe {
            (raw::engine_write_file)(self.0 as usize as *mut _, bytes.as_ptr().cast(), bytes.len() as u32,
                                     &mut written, null_mut())
        };
    }
}
