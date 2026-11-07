//! Patch 117: a character controller's step never leaves its position
//! non-finite (board ticket T117; docs/notes/client_patch.md section 3.71).
//!
//! - A step (bullet_character_controller::player_step 0x7BDA0) that ends with
//!   an infinity or a NaN in the position puts the controller back where the
//!   step began (m_last_pos): that step did not move it.
//! - Every other step is the game's, bit for bit.
//!
//! Live match 03439962 (revision 50): a server bot's body went NaN in one
//! step and stayed NaN for the match, falling for ever in the one game that
//! simulated it; its speed, m_last_pos, bullets and animation stream followed.

#![forbid(unsafe_code)]

use crate::vostok::controller::{Position, SteppedController};
use crate::vostok::{Context, Flow, NoExit};

/// The position the step ends at: its own when finite, else its start.
/// None keeps what the step wrote (a finite end, or no finite start to go
/// back to).
pub fn settled(end: Option<Position>, start: Option<Position>) -> Option<Position> {
    match end {
        Some(_) => None,
        None => start,
    }
}

/// player_step's epilogue (`controller.step_end`).
pub fn on_step_end(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(mut controller) = SteppedController::at(ctx)
        && let Some(position) = settled(controller.end(), controller.start())
    {
        controller.set_end(position);
    }
    Flow::Continue
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(x: f32, y: f32, z: f32) -> Option<Position> {
        Position::parse([x.to_bits(), y.to_bits(), z.to_bits()])
    }

    #[test]
    fn a_finite_end_is_kept() {
        assert_eq!(settled(at(1.0, 2.0, 3.0), at(0.0, 2.0, 3.0)), None);
        assert_eq!(settled(at(1.0, 2.0, 3.0), None), None);
    }

    #[test]
    fn a_non_finite_end_goes_back_to_the_start() {
        let start = at(-12.705_914, 21.689_978, 31.455_23);
        assert_eq!(settled(None, start), start);
    }

    #[test]
    fn with_no_finite_start_nothing_is_written() {
        assert_eq!(settled(None, None), None);
    }
}
