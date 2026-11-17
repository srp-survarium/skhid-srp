//! The Windows clipboard's text, read through the game's own imports
//! (user32's clipboard functions are delay-loaded imports of its, GlobalLock
//! and GlobalUnlock kernel32's) and kernel32's GlobalSize (not imported),
//! which bounds the read.

use core::char::DecodeUtf16;
use core::iter::Copied;
use core::ptr::{NonNull, null_mut};
use core::slice::Iter;

use super::Thread;
use crate::raw;

/// The clipboard's UTF-16 text (CF_UNICODETEXT).
const CF_UNICODETEXT: u32 = 13;

/// The clipboard's text as characters, up to its zero or its memory's end;
/// an unpaired surrogate is U+FFFD.
pub struct Text<'a>(DecodeUtf16<Copied<Iter<'a, u16>>>);

impl Iterator for Text<'_> {
    type Item = char;

    fn next(&mut self) -> Option<char> {
        self.0.next().map(|c| c.unwrap_or(char::REPLACEMENT_CHARACTER))
    }
}

/// Calls `read` with the clipboard's text while the clipboard is open, and
/// gives its answer; None when the clipboard holds no text or another
/// program has it open.
pub fn text<R>(_: &Thread, read: impl FnOnce(Text<'_>) -> R) -> Option<R> {
    // SAFETY: the exe's import with a NUL-ended name: kernel32 is loaded for
    // the process's life.
    let size = unsafe {
        let kernel32 = (raw::engine_get_module_handle_a)(c"kernel32.dll".as_ptr().cast());
        if kernel32.is_null() {
            return None;
        }
        NonNull::new((raw::engine_get_proc_address)(kernel32, c"GlobalSize".as_ptr().cast()))?
    };
    // SAFETY: the clipboard opened for this thread (no window: the task's)
    // and closed again on every path; its data handle is the clipboard's,
    // valid while it is open; the memory locked is read only up to the size
    // GlobalSize gives, as u16 (GlobalLock's memory is 8-aligned), and only
    // while locked: `read` gets it for its call alone.
    unsafe {
        if (raw::engine_open_clipboard)(null_mut()) == 0 {
            return None;
        }
        let answer = NonNull::new((raw::engine_get_clipboard_data)(CF_UNICODETEXT)).and_then(|data| {
            let units = raw::engine_global_size(size, data.as_ptr()) as usize / 2;
            let at = NonNull::new((raw::engine_global_lock)(data.as_ptr()))?;
            let all = core::slice::from_raw_parts(at.as_ptr().cast::<u16>(), units);
            let text = all.split(|&unit| unit == 0).next().unwrap_or(&[]);
            let answer = read(Text(char::decode_utf16(text.iter().copied())));
            (raw::engine_global_unlock)(data.as_ptr());
            Some(answer)
        });
        (raw::engine_close_clipboard)();
        answer
    }
}
