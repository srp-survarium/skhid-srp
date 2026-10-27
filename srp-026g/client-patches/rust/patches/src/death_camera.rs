//! Patch 46: on the death screen a drag turns the camera; the wheel does
//! nothing (client_patch.md section 3.43; the rules:
//! docs/notes/death-camera.md).
//!
//! A death leaves the player's input handler in the first person mode, and
//! the view's update gives the corpse's eyes, still; the hud shows the kits
//! and the cursor, so the game world's handler takes every move. Here, while
//! the kits are shown, the player's handler is in the first person mode and
//! the local player is dead:
//! - a button held: a drag as the warmup camera's (warmup_camera::press).
//!   Its moves turn a yaw and a pitch relative to the head, by the player's
//!   look's step (the mouse's sensitivity; the pitch also by the window's
//!   height over its width, inverted with the option), within 70 degrees
//!   to either side, 60 up and 40 down, and are taken (1): neither the
//!   cursor's nor the player's. The wheel in a drag does nothing.
//! - no button held: a drag ends (warmup_camera::drag_end); the move goes on
//!   to the original: the cursor's.
//!
//! Any other move is the warmup camera's (patch 31), then the original's.
//!
//! The view: while that player is dead, in the first person mode, the kits
//! shown and the turn not 0, the eyes' transform turned as a head turns:
//! the game's own update in its third-person step on the identity, with the
//! turn's yaw and pitch and the distance 0, gives the turn L; the view is L
//! times the eyes' transform (the head bone's, every frame), at the eyes'
//! place. The handler's warmup fields it borrows, and its mode (the update
//! reads it once, at 0x2422C6: any but 0 and 4 is the third-person step),
//! are put back. A live player clears the turn. Nothing a match computes:
//! the camera alone.

#![forbid(unsafe_code)]

use crate::vostok::world_mouse::{Mouse, ViewUpdate, Warmup, WorldMouse, look_options};
use crate::vostok::{Context, DeathCameraOnMouseMoveExit, DeathCameraOnViewUpdateExit, Flow, GameCell, Reg, Thread};
use crate::warmup_camera::{self, LEFT, LOBBY, Move, Press};

/// The player's input handler's first person mode (a death leaves it so).
const FIRST_PERSON: u32 = 0;
/// A mode whose view update is the third-person step (the update's own:
/// every mode but 0 and 4).
const THIRD_PERSON: u32 = 2;
/// A unit's turn, pi / 180 * 0.1 = 0.0017453292, as the player's look
/// (0x2402F1: 1/180, pi, 0.1), and its aspect factor 0.95492965 (0xA2DAC4).
const STEP: f32 = 0.001_745_329_2;
const ASPECT: f32 = 0.954_929_65;
/// The limits, relative to the head: 70 degrees (1.2217305) to either side
/// of the face, 60 (1.0471976) up (above the brow: the head), 40
/// (0.6981317) down (short of the chest).
const YAW_MAX: f32 = 1.221_730_5;
const YAW_MIN: f32 = -1.221_730_5;
const PITCH_UP: f32 = core::f32::consts::FRAC_PI_3;
const PITCH_DOWN: f32 = -0.698_131_7;
/// The transform the turn alone is made on.
const IDENTITY: [[f32; 4]; 4] = [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]];

/// The turn: a yaw and a pitch relative to the head.
#[repr(C)]
#[derive(Clone, Copy)]
struct Turn {
    yaw: f32,
    pitch: f32,
}

static TURN: GameCell<Turn> = GameCell::new(Turn { yaw: 0.0, pitch: 0.0 });

/// SSE's minss: `a` when it is below `b`, else `b` (a not-a-number: `b`).
fn minss(a: f32, b: f32) -> f32 {
    if a < b { a } else { b }
}

/// SSE's maxss.
fn maxss(a: f32, b: f32) -> f32 {
    if a > b { a } else { b }
}

/// `world_mouse.move`: a mouse move of the game world's handler.
pub fn on_mouse_move(ctx: &mut Context<'_>) -> Flow<DeathCameraOnMouseMoveExit> {
    let answer = {
        let Some(mouse) = WorldMouse::at(ctx) else { return Flow::Continue };
        let thread = ctx.thread();
        if death_screen(&mouse) {
            death_move(thread, &mouse)
        } else {
            match warmup_camera::mouse_move(thread, &mouse) {
                Move::Original => None,
                Move::OnToPlayer => Some(0),
            }
        }
    };
    match answer {
        None => Flow::Continue,
        Some(al) => {
            ctx.set_reg8(Reg::Eax, al);
            Flow::Exit(DeathCameraOnMouseMoveExit::Answered)
        }
    }
}

/// The death screen: the kits shown, the player's handler in the first
/// person mode, the local player dead.
fn death_screen(mouse: &WorldMouse<'_>) -> bool {
    mouse.kits_shown() && mouse.player_input().is_some_and(|input| input.mode() == FIRST_PERSON)
        && mouse.local_alive() == Some(false)
}

/// A move on the death screen: the answer, or none (the original's).
fn death_move(thread: &Thread, mouse: &WorldMouse<'_>) -> Option<u8> {
    let moves = mouse.mouse();
    if moves.buttons & (LOBBY | LEFT) == 0 {
        warmup_camera::drag_end(thread, mouse);
        return None;
    }
    match warmup_camera::press(thread, mouse, moves) {
        Press::Click => None,
        Press::Drag => {
            turn(thread, mouse, moves);
            Some(1) // neither the cursor's nor the player's
        }
    }
}

/// The drag's move: the yaw and the pitch turned, within the limits.
fn turn(thread: &Thread, mouse: &WorldMouse<'_>, moves: Mouse) {
    let (width, height) = mouse.window_size();
    let ratio = height / width;
    let (sensitivity, inverted) = look_options();
    let unit = sensitivity * STEP;
    let mut vertical = ratio * unit * ASPECT;
    if inverted {
        vertical = -vertical;
    }
    let mut turn = TURN.get(thread);
    turn.yaw = maxss(minss(turn.yaw - moves.x as f32 * unit, YAW_MAX), YAW_MIN);
    TURN.set(thread, turn);
    if moves.y != 0 {
        turn.pitch = maxss(minss(turn.pitch - moves.y as f32 * vertical, PITCH_UP), PITCH_DOWN);
        TURN.set(thread, turn);
    }
}

/// `camera.view_update`: update_camera's call of the view's update.
pub fn on_view_update(ctx: &mut Context<'_>) -> Flow<DeathCameraOnViewUpdateExit> {
    let view = ViewUpdate::at(ctx);
    let thread = ctx.thread();
    if view.player_alive() {
        TURN.set(thread, Turn { yaw: 0.0, pitch: 0.0 });
        return Flow::Continue;
    }
    let Some(input) = view.handler() else { return Flow::Continue };
    if input.mode() != FIRST_PERSON || !input.kits_shown() {
        return Flow::Continue;
    }
    let turn = TURN.get(thread);
    if (turn.yaw.to_bits() | turn.pitch.to_bits()) << 1 == 0 {
        return Flow::Continue; // no turn (-0.0 is 0)
    }
    let Some(eyes) = view.transform() else { return Flow::Continue };
    let kept = input.warmup();
    input.set_warmup(Warmup { yaw: turn.yaw.to_bits(), pitch: turn.pitch.to_bits(), distance: 0 });
    input.set_mode(THIRD_PERSON);
    input.update_view(&IDENTITY); // the turn L, at the origin
    input.set_mode(FIRST_PERSON);
    input.set_warmup(kept);
    let l = input.view();
    let mut out = [[0f32; 4]; 4];
    for (row, l) in out.iter_mut().zip(&l).take(3) {
        for (c, value) in row.iter_mut().enumerate() {
            *value = l[0] * eyes[0][c] + l[1] * eyes[1][c] + l[2] * eyes[2][c];
        }
    }
    out[3] = eyes[3]; // at the eyes' place
    input.set_view(out);
    Flow::Exit(DeathCameraOnViewUpdateExit::Turned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_constants_are_the_asm_bits() {
        let bits = [STEP, ASPECT, YAW_MAX, YAW_MIN, PITCH_UP, PITCH_DOWN].map(f32::to_bits);
        assert_eq!(bits, [0x3AE4_C388, 0x3F74_7645, 0x3F9C_61AA, 0xBF9C_61AA, 0x3F86_0A92, 0xBF32_B8C2]);
    }

    #[test]
    fn sse_min_and_max() {
        assert_eq!((minss(1.0, 2.0), minss(f32::NAN, 2.0), maxss(3.0, 2.0)), (1.0, 2.0, 3.0));
        assert!(maxss(2.0, f32::NAN).is_nan() && minss(2.0, f32::NAN).is_nan());
    }
}
