//! Patch 31, its keys: a mouse button held from before the start is not
//! the player's until it is let go (client_patch.md section 3.29; the
//! rules: docs/notes/kit-change-before-start.md).
//!
//! The game world's handler takes the mouse's keys while the cursor is
//! shown (the kits before the start, the death screen); the player's
//! handler after it fired at a held state and a release alike. So a button
//! pressed while the cursor was shown (a drag of the warmup camera, a pick
//! on the death screen) and still held when the cursor went (the start,
//! the respawn) fired without a press of its own. Here a button that goes
//! down while the cursor is shown is marked; while the cursor is not shown
//! a marked button's held state is taken here (no handler after this one),
//! and so is its release, which unmarks it. A press while the cursor is not
//! shown (a fresh one, should a release have been missed) unmarks it and
//! is the original's. Anything else is the original's. Nothing a match
//! computes: the player's input only misses the held states and the
//! release of a press it never had.

#![forbid(unsafe_code)]

use crate::vostok::world_input::{MouseKey, MouseKind};
use crate::vostok::{Context, Flow, GameCell, Reg, WarmupKeysOnMouseKeyExit};

/// The buttons marked, a bit each.
static MARKED: GameCell<u32> = GameCell::new(0);

/// `warmup_keys.mouse_key`: a key the game world's handler is told.
pub fn on_mouse_key(ctx: &mut Context<'_>) -> Flow<WarmupKeysOnMouseKeyExit> {
    let key = MouseKey::at(ctx);
    let Some(button) = key.button() else { return Flow::Continue };
    let (kind, shown) = (key.kind(), key.cursor_shown());
    let bit = 1u32 << button;
    let thread = ctx.thread();
    let marked = MARKED.get(thread);
    match kind {
        MouseKind::Down => {
            MARKED.set(thread, if shown { marked | bit } else { marked & !bit });
            return Flow::Continue;
        }
        MouseKind::Up => MARKED.set(thread, marked & !bit),
        MouseKind::Held => {}
    }
    if marked & bit == 0 || shown {
        return Flow::Continue;
    }
    ctx.set_reg(Reg::Eax, button + 1); // taken: not 0
    Flow::Exit(WarmupKeysOnMouseKeyExit::Taken)
}
