//! Patch 113: the hud's kits and cursor (README "The hud's kits and cursor").
//!
//! - **A new match.** The hud is the game's for its whole life, and patch 31
//!   keeps the kits (and the cursor with them) shown until play, so a match
//!   that ends with them shown (left before the start, or dead at its end)
//!   leaves them shown for the next: there the kits' first showing returned
//!   at once, the movie was never told the new match's kit and kept the last
//!   match's pick on its tab. At each match's start the kits and the cursor
//!   are hidden, as the hud movie's `reset` hides its kits.
//! - **The main menu.** Esc over the kits (before the start, on the death
//!   screen) brings the main menu with its own cursor; the hud's stayed drawn
//!   under it where it was. While the menu is shown the hud's cursor movie is
//!   off the screen: taken off when the menu comes, neither put up nor taken
//!   off again when the hud shows or hides its cursor under it (a death, a
//!   spawn, a new match), put back when the menu closes with the cursor
//!   shown, whether the match still runs or has just ended. The hud's cursor
//!   flag is the game's throughout, so the mouse goes where it went.
//! - **The menu's own cursor.** While the menu's key-binding settings wait
//!   for a key the menu's cursor movie is off the scene, and the menu's close
//!   by anything but a key (a match's end, a kick, a match that starts over
//!   the lobby's menu) took it off again (exe+0x14EE8D). The close ends the
//!   wait first, as Esc does.

#![forbid(unsafe_code)]

use crate::vostok::{
    Context, Flow, Hud, HudCursorOnCursorHiddenExit, HudCursorOnCursorShownExit, MenuChange, NoExit,
};

pub fn on_match_start(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(hud) = Hud::at_match_start(ctx) {
        hud.start_over();
    }
    Flow::Continue
}

pub fn on_cursor_shown(ctx: &mut Context<'_>) -> Flow<HudCursorOnCursorShownExit> {
    match Hud::at_cursor_shown(ctx) {
        Some(hud) if hud.under_menu() => Flow::Exit(HudCursorOnCursorShownExit::Kept),
        _ => Flow::Continue,
    }
}

pub fn on_cursor_hidden(ctx: &mut Context<'_>) -> Flow<HudCursorOnCursorHiddenExit> {
    match Hud::at_cursor_hidden(ctx) {
        Some(hud) if hud.under_menu() => Flow::Exit(HudCursorOnCursorHiddenExit::Kept),
        _ => Flow::Continue,
    }
}

pub fn on_menu_show(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(cursor) = MenuChange::at_show(ctx).and_then(|menu| menu.shown_cursor()) {
        cursor.withdraw();
    }
    Flow::Continue
}

pub fn on_menu_close(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(cursor) = MenuChange::at_close(ctx).and_then(|menu| menu.close()) {
        cursor.restore();
    }
    Flow::Continue
}
