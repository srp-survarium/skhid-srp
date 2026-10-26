//! Patch 25: the sign-in reports a stale shell_32.dll in the temporary
//! folder (patch revision 12, client_patch.md section 3.25).
//!
//! Whether %TEMP%\shell_32.dll exists goes to the login server in byte 8
//! of the sign-in's version field: 80h the check ran (a game that says 0
//! is older than revision 12), 01h the file exists. Nothing else of the
//! file or of the machine is read or sent. A kernel32 without
//! GetTempPathA, or a folder too long for the name, is a file that does
//! not exist.

#![forbid(unsafe_code)]

use core::ffi::CStr;

use crate::vostok::kernel32::{file_exists, temp_path};
use crate::vostok::sign_in::set_version_byte_8;
use crate::vostok::{Context, Flow, NoExit};

/// The check ran.
const CHECKED: u8 = 0x80;
/// The file exists.
const EXISTS: u8 = 0x01;
/// The file's name, NUL-ended.
const NAME: &[u8] = b"shell_32.dll\0";
/// The folder and the name, on the stack.
const PATH_SIZE: usize = 272;

/// The flags of byte 8.
fn flags() -> u8 {
    const ROOM: usize = PATH_SIZE - NAME.len(); // the folder's, the name after it
    let mut path = [0u8; PATH_SIZE];
    let (folder, _) = path.split_at_mut(ROOM);
    // GetTempPathA's length without its NUL; 0, or the size it needs, if it
    // failed.
    let Some(length) = temp_path(folder) else { return CHECKED };
    let length = length as usize;
    if length == 0 || length >= ROOM {
        return CHECKED;
    }
    let Some(name) = path.get_mut(length..length + NAME.len()) else { return CHECKED };
    name.copy_from_slice(NAME);
    match CStr::from_bytes_until_nul(&path) {
        Ok(path) if file_exists(path) => CHECKED | EXISTS,
        _ => CHECKED,
    }
}

/// `sign_in.version_tail`: the version field's last two writes, which
/// Continue replays with byte 8 the flags.
pub fn on_version_tail(ctx: &mut Context<'_>) -> Flow<NoExit> {
    set_version_byte_8(ctx, flags());
    Flow::Continue
}
