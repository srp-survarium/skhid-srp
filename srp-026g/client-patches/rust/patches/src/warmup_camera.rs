//! Patch 31, its camera: before the start, while the kits are shown, the
//! warmup camera is the lobby's character view (client_patch.md section
//! 3.29; the rules: docs/notes/kit-change-before-start.md).
//!
//! The input world gives a mouse move to its handlers by priority until one
//! answers 1: the game world's (7) before the player's input handler (10),
//! whose warmup mode turns the warmup camera by the moves and moves it by
//! the wheel. The game world's own answers whether the cursor is shown, and
//! the kits show it: before the start every move was the cursor's. Here,
//! while the kits are shown and the player's handler is in the warmup mode:
//! - a button of the lobby camera's held (right, 5th, 7th): a drag at once;
//!   the left one (the kits' click): a drag once its moves since it went
//!   down add up to more than CLICK, the cursor's before, so a click stays a
//!   click. A drag begins by keeping the hud's pointer's place and taking
//!   the pointer off the screen (a press on a kit is let go outside it). In
//!   a drag the move goes on (0) to the player's handler: the camera turns.
//! - no button held: a drag ends with the pointer back where it began (as
//!   the lobby camera puts its cursor back). A move of the wheel goes on to
//!   the player's handler with no turn (x and y 0): the camera nearer or
//!   further. Any other move is the original's: the cursor's.
//!
//! And the camera keeps the lobby camera's limits: process_warmup_mode's
//! store of the pitch keeps it within +-60 degrees and the distance within
//! 1.5 and 4.6, both compared as integers (a float's bits order as signed
//! integers for a positive one, any negative one below; a not-a-number is
//! past the limit). Nothing a match computes: the warmup mode gives the
//! player's input no turn.
//!
//! The drag and its end are patch 46's too (death_camera.rs: the death
//! screen's look).

#![forbid(unsafe_code)]

use crate::vostok::world_mouse::{Mouse, PlayerInput, WorldMouse, stored_pitch};
use crate::vostok::{Context, Flow, GameCell, NoExit, Thread};

/// The player's input handler's warmup mode (switch_to_warmup_camera
/// 0x1F8190 alone sets it: before play).
const WARMUP: u32 = 3;
/// The mouse's buttons: the left one, and the lobby camera's (0x283D81).
pub const LEFT: u8 = 0x01;
pub const LOBBY: u8 = 0x52;
/// A left press moved no more is a click.
const CLICK: u32 = 8;
/// The hud's pointer in a drag.
const AWAY: i32 = -128;
/// 1.0471976f, +-60 degrees: the lobby camera's pitch (0x9C56C0, 0xA2DB08),
/// as its bits.
const PITCH_MAX: u32 = core::f32::consts::FRAC_PI_3.to_bits();
/// 1.5f and 4.6f, the lobby camera's nearest and farthest (0xA7AFA4,
/// 0xA2DB0C), as integers.
const NEAREST: i32 = 1.5_f32.to_bits() as i32;
const FARTHEST: i32 = 4.6_f32.to_bits() as i32;
/// A float's sign bit.
const SIGN: u32 = 1 << 31;

/// The drag: the moves of the buttons held (above CLICK: a drag), the hud's
/// pointer when it began.
#[repr(C)]
#[derive(Clone, Copy)]
struct Drag {
    travel: u32,
    x: u32,
    y: u32,
}

static DRAG: GameCell<Drag> = GameCell::new(Drag { travel: 0, x: 0, y: 0 });

/// A press of a button that turns the camera.
pub enum Press {
    /// In a drag (begun now, or before).
    Drag,
    /// Still a click: the cursor's.
    Click,
}

/// A button held: a drag, or still a click.
pub fn press(thread: &Thread, mouse: &WorldMouse<'_>, moves: Mouse) -> Press {
    let mut drag = DRAG.get(thread);
    if drag.travel > CLICK {
        return Press::Drag;
    }
    if moves.buttons & LOBBY == 0 {
        // the left button: |x| + |y| since it went down
        drag.travel = drag.travel.wrapping_add(moves.x.wrapping_abs() as u32).wrapping_add(moves.y.wrapping_abs() as u32);
        DRAG.set(thread, drag);
        if drag.travel <= CLICK {
            return Press::Click;
        }
    }
    let (x, y) = mouse.pointer();
    DRAG.set(thread, Drag { travel: u32::MAX, x, y });
    mouse.set_pointer(AWAY, AWAY, 0);
    Press::Drag
}

/// No button held: a drag ends with the hud's pointer, and the system's
/// cursor, where it began; the moves are 0 again.
pub fn drag_end(thread: &Thread, mouse: &WorldMouse<'_>) {
    let drag = DRAG.get(thread);
    DRAG.set(thread, Drag { travel: 0, ..drag });
    if drag.travel > CLICK {
        mouse.set_cursor(drag.x, drag.y);
    }
}

/// What the warmup camera makes of a move.
pub enum Move {
    /// The original's.
    Original,
    /// On to the player's handler (the game world's answer 0).
    OnToPlayer,
}

/// A move of the game world's on_mouse_move, before the start.
pub fn mouse_move(thread: &Thread, mouse: &WorldMouse<'_>) -> Move {
    if !mouse.kits_shown() || mouse.player_input().is_none_or(|input| input.mode() != WARMUP) {
        return Move::Original;
    }
    let moves = mouse.mouse();
    if moves.buttons & (LOBBY | LEFT) == 0 {
        drag_end(thread, mouse);
        if moves.wheel == 0 {
            return Move::Original;
        }
        mouse.clear_moves(); // the wheel alone on to the player's handler
        return Move::OnToPlayer;
    }
    match press(thread, mouse, moves) {
        Press::Click => Move::Original,
        Press::Drag => Move::OnToPlayer,
    }
}

/// `warmup_camera.pitch_store`: the new pitch and the distance, within the
/// lobby camera's limits.
pub fn on_pitch_store(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(input) = PlayerInput::at_pitch_store(ctx) else { return Flow::Continue };
    let bits = stored_pitch(ctx);
    let pitch = if bits & !SIGN > PITCH_MAX { bits & SIGN | PITCH_MAX } else { bits };
    input.set_warmup_pitch(pitch);
    let distance = (input.warmup().distance as i32).clamp(NEAREST, FARTHEST);
    input.set_warmup_distance(distance as u32);
    Flow::Skip
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_limits_are_the_asm_bits() {
        assert_eq!((PITCH_MAX, NEAREST, FARTHEST), (0x3F86_0A92, 0x3FC0_0000, 0x4093_3333));
    }
}
