//! kernel32, as the patches ask it: through the exe's import slots, and an
//! export the exe does not import (a `proc` row of engine_026g.tsv) through
//! GetModuleHandleA and GetProcAddress at run time, every time it is asked
//! (as the asm patches did). A test never calls these.

use core::ffi::{CStr, c_void};
use core::ptr::{NonNull, null_mut};

use crate::raw;

/// The address of an export the exe does not import: GetModuleHandleA of
/// its module, then GetProcAddress of its name; None when either says 0.
fn proc_address(proc: &raw::Proc) -> Option<NonNull<c_void>> {
    // SAFETY: kernel32's GetModuleHandleA and GetProcAddress through the
    // exe's import slots, which the loader filled; NUL-ended names.
    unsafe {
        let module = NonNull::new((raw::engine_get_module_handle_a)(proc.module.as_ptr().cast()))?;
        NonNull::new((raw::engine_get_proc_address)(module.as_ptr(), proc.name.as_ptr().cast()))
    }
}

/// GetTempPathA into `buffer` (its length the size given): the folder's
/// length without its NUL, or the size it needs, or 0 when it failed, as
/// GetTempPathA answers; None when kernel32 has no GetTempPathA.
pub fn temp_path(buffer: &mut [u8]) -> Option<u32> {
    let at = proc_address(&raw::ENGINE_GET_TEMP_PATH_A)?;
    // SAFETY: GetTempPathA's address (GetProcAddress said so); it writes
    // at most `buffer.len()` bytes into it.
    Some(unsafe { raw::engine_get_temp_path_a(at.as_ptr(), buffer.len() as u32, buffer.as_mut_ptr()) })
}

/// Whether a file of that path can be opened, for no access (only whether
/// it is there), shared for reading, writing and deleting; the handle is
/// closed again.
pub fn file_exists(path: &CStr) -> bool {
    const NO_ACCESS: u32 = 0;
    const SHARE_ALL: u32 = 7; // FILE_SHARE_READ | WRITE | DELETE
    const OPEN_EXISTING: u32 = 3;
    const NORMAL: u32 = 0x80; // FILE_ATTRIBUTE_NORMAL
    // SAFETY: CreateFileA and CloseHandle through the exe's import slots; a
    // NUL-ended name, a handle CreateFileA gave.
    unsafe {
        let file = (raw::engine_create_file_a)(path.as_ptr().cast(), NO_ACCESS, SHARE_ALL, null_mut(),
                                               OPEN_EXISTING, NORMAL, null_mut());
        if file as isize == -1 {
            return false;
        }
        (raw::engine_close_handle)(file);
    }
    true
}
