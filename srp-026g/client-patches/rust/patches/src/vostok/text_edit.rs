//! The engine's own text edit (`vostok::ui::ui_text_edit`): the console's
//! line. The line is a buffer_string inside it (+48h, 200h bytes), cut at its
//! max length (+2A4h, 0FFh unless set); the caret and the selection are byte
//! indices into it. A typed character's binding (0x34B4D0) deletes the
//! selection (0x3486A0), then inserts at the caret (0x348000) through
//! set_text (0x3479D0), which fires the text-changed event (the console's
//! tips) and keeps an undo step; [`TextEdit::insert`] does the same with a
//! run of characters.

use core::ffi::CStr;
use core::ops::Range;

use super::{Context, Thread};
use crate::raw;

/// The line's buffer: 200h bytes, its zero among them (the base ctor,
/// 0x348CF8).
const BUFFER: usize = 0x200;

/// A character as the line holds a typed one: printable ASCII. The
/// character bindings type these (each key's own, 0x34A4C0's arguments; a
/// key's name only when it is a letter, 0x34B579).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EditChar(u8);

impl EditChar {
    pub fn of(c: char) -> Option<EditChar> {
        matches!(c, ' '..='~').then_some(EditChar(c as u8))
    }
}

impl From<EditChar> for u8 {
    fn from(c: EditChar) -> u8 {
        c.0
    }
}

/// Characters to insert, at most a full line's.
pub struct Typed {
    chars: [EditChar; BUFFER - 1],
    len: usize,
}

impl Typed {
    pub const fn new() -> Typed {
        Typed { chars: [EditChar(b' '); BUFFER - 1], len: 0 }
    }

    /// Adds one; false when full (no line would take it).
    pub fn push(&mut self, c: EditChar) -> bool {
        let Some(slot) = self.chars.get_mut(self.len) else { return false };
        *slot = c;
        self.len += 1;
        true
    }

    pub fn as_slice(&self) -> &[EditChar] {
        &self.chars[..self.len]
    }
}

impl Default for Typed {
    fn default() -> Self {
        Self::new()
    }
}

/// The text edit whose paste_from_clipboard runs (`console_paste.paste`).
pub struct TextEdit<'c, 'm> {
    ctx: &'c mut Context<'m>,
    edit: raw::UiTextEdit,
}

impl<'c, 'm> TextEdit<'c, 'm> {
    pub fn at_paste(ctx: &'c mut Context<'m>) -> Option<TextEdit<'c, 'm>> {
        // SAFETY: the context is this site's: ECX the text edit whose
        // binding calls paste_from_clipboard, alive while it runs.
        let edit = unsafe { raw::site::console_paste_paste::edit(&*ctx)? };
        Some(TextEdit { ctx, edit })
    }

    pub fn thread(&self) -> &Thread {
        self.ctx.thread()
    }

    /// Inserts `typed` at the caret as typing does: the selection deleted
    /// first (the game's own delete), then as many as the line has room for
    /// (the rest cut), the caret after them. With none, or no room, nothing
    /// changes. Returns how many went in.
    pub fn insert(&mut self, typed: &Typed) -> usize {
        let line = self.line();
        let count = line.room().min(typed.len);
        if count == 0 {
            return 0;
        }
        // SAFETY: the live text edit, on the game's thread (this hook's);
        // each function its own convention's (the re-kit rows): the delete
        // takes ESI, set_text the edit's ui_text part and a C string it
        // copies before returning, set_caret_position the edit, a position
        // and 0 (the selection collapsed to the caret).
        unsafe {
            if !line.selected.is_empty() {
                raw::engine_text_edit_delete_selection(Some(self.edit));
            }
            let line = self.line();
            let mut text = [0; BUFFER];
            let joined = joined(&mut text, &line, &typed.as_slice()[..count]);
            raw::engine_text_edit_set_text(Some(self.edit.text()), joined.as_ptr().cast());
            raw::engine_text_edit_set_caret(Some(self.edit), (line.caret + count) as u32, 0);
        }
        count
    }

    /// The line as it stands, read once.
    fn line(&self) -> Line {
        // SAFETY: the live text edit's fields, on its thread; the line's
        // bytes are read only between its begin and its end, both checked
        // within its buffer first.
        unsafe {
            let string = self.edit.line();
            let (begin, end, room) = (string.begin(), string.end(), string.capacity_end());
            let capacity = (room as usize).saturating_sub(begin as usize).min(BUFFER);
            let len = (end as usize).saturating_sub(begin as usize).min(capacity.saturating_sub(1));
            let mut text = [0; BUFFER];
            if !begin.is_null() {
                text[..len].copy_from_slice(core::slice::from_raw_parts(begin, len));
            }
            Line::new(
                text,
                len,
                usize::from(*self.edit.max_length()).min(capacity.saturating_sub(1)),
                usize::from(*self.edit.caret()),
                (usize::from(*self.edit.selection_start()), usize::from(*self.edit.selection_end())),
            )
        }
    }
}

/// A line: its text, the most it may hold, the caret and the selection
/// within it.
struct Line {
    text: [u8; BUFFER],
    len: usize,
    limit: usize,
    caret: usize,
    selected: Range<usize>,
}

impl Line {
    fn new(text: [u8; BUFFER], len: usize, limit: usize, caret: usize, (a, b): (usize, usize)) -> Line {
        let (a, b) = (a.min(len), b.min(len));
        Line { text, len, limit, caret: caret.min(len), selected: a.min(b)..a.max(b) }
    }

    /// The characters an insert may add: the limit less what stays after the
    /// selection is deleted.
    fn room(&self) -> usize {
        self.limit.saturating_sub(self.len - self.selected.len())
    }
}

/// The line with `typed` at its caret, its zero after it, in `into`.
fn joined<'b>(into: &'b mut [u8; BUFFER], line: &Line, typed: &[EditChar]) -> &'b CStr {
    let (head, tail) = line.text[..line.len].split_at(line.caret);
    let mut at = 0;
    for byte in head.iter().copied().chain(typed.iter().map(|c| c.0)).chain(tail.iter().copied()) {
        if at == BUFFER - 1 {
            break;
        }
        into[at] = byte;
        at += 1;
    }
    into[at] = 0;
    CStr::from_bytes_until_nul(&into[..=at]).unwrap_or(c"")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(text: &str, limit: usize, caret: usize, selection: (usize, usize)) -> Line {
        let mut bytes = [0; BUFFER];
        bytes[..text.len()].copy_from_slice(text.as_bytes());
        Line::new(bytes, text.len(), limit, caret, selection)
    }

    fn typed(text: &str) -> Typed {
        let mut typed = Typed::new();
        text.chars().for_each(|c| assert!(typed.push(EditChar::of(c).unwrap())));
        typed
    }

    #[test]
    fn printable_ascii_only() {
        assert_eq!(EditChar::of(' '), Some(EditChar(b' ')));
        assert_eq!(EditChar::of('~'), Some(EditChar(b'~')));
        for c in ['\t', '\n', '\u{7f}', '\u{a0}', 'é', 'ж', '\u{fffd}'] {
            assert_eq!(EditChar::of(c), None, "{c:?}");
        }
    }

    #[test]
    fn inserted_at_the_caret() {
        let at = line("whre", 255, 2, (2, 2));
        let mut into = [0; BUFFER];
        assert_eq!(joined(&mut into, &at, typed("e").as_slice()).to_bytes(), b"where");
        let end = line("collision", 255, 9, (9, 9));
        assert_eq!(joined(&mut into, &end, typed(" walk").as_slice()).to_bytes(), b"collision walk");
    }

    #[test]
    fn room_is_the_limit_less_what_stays() {
        assert_eq!(line("", 255, 0, (0, 0)).room(), 255);
        assert_eq!(line("abc", 5, 3, (3, 3)).room(), 2);
        assert_eq!(line("abcde", 5, 5, (5, 5)).room(), 0);
        assert_eq!(line("abcde", 5, 4, (1, 4)).room(), 3, "the selection is deleted first");
        assert_eq!(line("abcde", 3, 0, (0, 0)).room(), 0, "a line over its limit takes none");
    }

    #[test]
    fn caret_and_selection_kept_within_the_line() {
        let at = line("abc", 255, 9, (7, 2));
        assert_eq!((at.caret, at.selected.clone()), (3, 2..3));
    }

    #[test]
    fn a_full_typed_run_stops() {
        let mut typed = Typed::new();
        let x = EditChar::of('x').unwrap();
        assert!((0..BUFFER - 1).all(|_| typed.push(x)));
        assert!(!typed.push(x));
        assert_eq!(typed.as_slice().len(), BUFFER - 1);
    }
}
