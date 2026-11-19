//! Patch 157: Ctrl+V pastes the clipboard's text into the console's line
//! (docs/notes/console-paste.md). The engine's text edit binds Ctrl+V and
//! Shift+Insert to its paste_from_clipboard (init_internals 0x34AA5C), a stub
//! that only logs "not implemented"; the hook at its start
//! (`console_paste.paste`) pastes instead and returns.
//!
//! The text's first line goes in at the caret as typed characters: a
//! control character (a line's end, a tab) or a line or paragraph separator
//! ends it, a character the line cannot hold (not printable ASCII) is left
//! out, and what the line has no room for is cut. Nothing is run: Enter is
//! the console's alone. No text on the clipboard: nothing happens.

#![forbid(unsafe_code)]

use crate::vostok::clipboard;
use crate::vostok::text_edit::{EditChar, TextEdit, Typed};
use crate::vostok::{ConsolePasteOnPasteExit, Context, Flow};

pub fn on_paste(ctx: &mut Context<'_>) -> Flow<ConsolePasteOnPasteExit> {
    let Some(mut edit) = TextEdit::at_paste(ctx) else {
        return Flow::Continue;
    };
    if let Some(typed) = clipboard::text(edit.thread(), |text| first_line(text)) {
        edit.insert(&typed);
    }
    Flow::Exit(ConsolePasteOnPasteExit::Pasted)
}

/// What a pasted text types: its first line's characters the line can hold.
fn first_line(text: impl Iterator<Item = char>) -> Typed {
    let mut typed = Typed::new();
    for c in text.take_while(|&c| !ends_line(c)) {
        if let Some(c) = EditChar::of(c)
            && !typed.push(c)
        {
            break;
        }
    }
    typed
}

fn ends_line(c: char) -> bool {
    c.is_control() || matches!(c, '\u{2028}' | '\u{2029}')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pasted(text: &str) -> String {
        first_line(text.chars()).as_slice().iter().map(|&c| char::from(u8::from(c))).collect()
    }

    #[test]
    fn the_first_line_alone() {
        assert_eq!(pasted("collision walk"), "collision walk");
        assert_eq!(pasted("where\r\ncollision"), "where");
        assert_eq!(pasted("where\ncollision"), "where");
        assert_eq!(pasted("a\tb"), "a");
        assert_eq!(pasted("a\u{2028}b"), "a");
        assert_eq!(pasted("a\u{85}b"), "a");
        assert_eq!(pasted("\nwhere"), "");
        assert_eq!(pasted(""), "");
    }

    #[test]
    fn characters_the_line_cannot_hold_are_left_out() {
        assert_eq!(pasted("say пр ok"), "say  ok");
        assert_eq!(pasted("\u{feff}where\u{a0}"), "where");
        assert_eq!(pasted("\u{fffd}x"), "x");
    }

    #[test]
    fn a_long_line_stops_at_a_full_line() {
        assert_eq!(pasted(&"x".repeat(5000)).len(), 511);
    }
}
