//! Patch 107: chat text follows the keyboard layout
//! (docs/notes/keyboard_layout.md).
//!
//! The engine types a key's text on its logic thread with that thread's
//! layout (`GetKeyboardLayout(0)` in `keyboard::translate_text`), and gives
//! the logic thread a new layout with `ActivateKeyboardLayout(hkl,
//! KLF_SETFORPROCESS)` from its window procedure. Wine does not support the
//! flag, so under Wine the text keeps the logic thread's first layout; with
//! Wine's Wayland driver, whose text depends on that layout, Alt+Shift never
//! reaches the chat.
//!
//! The window procedure records every layout it is told of, a request or a
//! change, before the game's check that a text input is focused (a request
//! outside one is dropped: Wine's driver does not send it again), and the
//! text uses the recorded layout; until one is recorded, the thread's own.

#![forbid(unsafe_code)]

use crate::vostok::keyboard_layout::{activated_layout, requested_layout, ActiveLayout, Hkl, TextLayout};
use crate::vostok::{Context, Flow, NoExit};

/// The layout the window procedure was last told of: written on the
/// window's thread, read on the logic thread.
static LAYOUT: ActiveLayout = ActiveLayout::none();

/// The layout the window procedure was last told of, if any (patch 110's
/// keys follow it too).
pub fn told() -> Option<Hkl> {
    LAYOUT.get()
}

pub fn on_request(ctx: &mut Context) -> Flow<NoExit> {
    if let Some(hkl) = requested_layout(ctx) {
        LAYOUT.set(hkl);
    }
    Flow::Continue
}

pub fn on_change(ctx: &mut Context) -> Flow<NoExit> {
    if let Some(hkl) = activated_layout(ctx) {
        LAYOUT.set(hkl);
    }
    Flow::Continue
}

pub fn on_text(ctx: &mut Context) -> Flow<NoExit> {
    if let Some(hkl) = LAYOUT.get() {
        TextLayout::at(ctx).type_with(hkl);
    }
    Flow::Continue
}
