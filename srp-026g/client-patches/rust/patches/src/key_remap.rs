//! Patch 110: the keys a desktop moves are moved in the game
//! (docs/notes/keyboard_layout.md).
//!
//! The game reads the keyboard through DirectInput's buffered events
//! (keyboard::execute 0x3B8D20). Wine's X11 driver made each key's scan code
//! from its keysym, so a desktop's moved keys (caps:swapescape) reached the
//! game moved; Wine's Wayland driver (the launcher's choice under niri) gives
//! the physical key's code, and its virtual keys are the physical ones too.
//! What the desktop moved is left only in the layout's key names: under
//! caps:swapescape the Caps Lock key is named Escape.
//!
//! Each poll's events (`keyboard.events`) become the keys their names say
//! (`KeyMoves`, the function keys only: letters and digits stay the physical
//! keys DirectInput gives on Windows too). The names are the layout's the
//! window was last told of (patch 107's), read on the polling thread with
//! that layout lent to it, once a layout. Nothing moves on Windows (the
//! system moves keys below DirectInput there) nor before a layout is told.
//! Each client's own keys only: no simulation change.

#![forbid(unsafe_code)]

use crate::keyboard_layout;
use crate::vostok::keyboard_layout::{with_thread_layout, Hkl};
use crate::vostok::system::System;
use crate::vostok::{Context, Flow, GameCell, KeyEvents, KeyMoves, NoExit};

/// The moves of the layout they were read in.
#[derive(Clone, Copy)]
struct LayoutMoves {
    layout: Hkl,
    moves: KeyMoves,
}

/// Read once a layout, on the polling thread.
static MOVES: GameCell<Option<LayoutMoves>> = GameCell::new(None);

pub fn on_events(ctx: &mut Context) -> Flow<NoExit> {
    let ctx = &*ctx;
    let Some(layout) = keyboard_layout::told() else { return Flow::Continue };
    let Some(mut events) = KeyEvents::at(ctx) else { return Flow::Continue };
    let thread = ctx.thread();
    let moves = match MOVES.get(thread) {
        Some(known) if known.layout == layout => known.moves,
        _ => {
            let moves = match System::running() {
                System::Windows => KeyMoves::NONE,
                System::Wine => with_thread_layout(layout, || KeyMoves::named(|key| events.name(key)))
                    .unwrap_or(KeyMoves::NONE),
            };
            MOVES.set(thread, Some(LayoutMoves { layout, moves }));
            moves
        }
    };
    if moves != KeyMoves::NONE {
        events.apply(&moves);
    }
    Flow::Continue
}
