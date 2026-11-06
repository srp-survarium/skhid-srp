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
//!   off the screen: taken off when the menu comes, not put up when the hud
//!   shows its cursor (a death under the menu), put back when the menu
//!   closes over the match that still runs. The hud's cursor flag is the
//!   game's throughout, so the mouse goes where it went.

#![forbid(unsafe_code)]

use crate::vostok::{Context, Flow, HudCursorOnCursorShownExit, Hud, MainMenu, NoExit};

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

pub fn on_menu_show(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(cursor) = MainMenu::at_show(ctx).and_then(|menu| menu.match_cursor()) {
        cursor.withdraw();
    }
    Flow::Continue
}

pub fn on_menu_close(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(menu) = MainMenu::at_close(ctx)
        && menu.shown()
        && let Some(cursor) = menu.match_cursor()
    {
        cursor.restore();
    }
    Flow::Continue
}
