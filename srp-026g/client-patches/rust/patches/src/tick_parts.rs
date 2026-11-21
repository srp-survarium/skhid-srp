//! Patch 168: a tick the world steps in parts moves a body, and says its
//! speed, as a whole tick does (docs/notes/fall_damage.md, "A part of a
//! tick: its least walk and the speed read from it").
//!
//! - The least walk: a step with nothing to walk is skipped
//!   (can_skeep_player_step 0x7C7E0: the walk asked for the step under
//!   3.16 mm). A part of a tick is asked a part of the tick's walk, so a
//!   short part of a walking body was skipped, its ground lost and its speed
//!   read as 0. A part is now asked what its tick would be: the walk at the
//!   part's pace over a whole tick.
//! - The speed: m_linear_horizontal_speed is the last step's move over its
//!   time, read by the jump as it begins (its kind and its forward speed)
//!   and by the "is moving" notice. Only the step that ends a tick writes
//!   it, so a jump begun in a cut tick reads what an uncut one reads.
//!
//! A whole tick is the game's, bit for bit, in both.

#![forbid(unsafe_code)]

use crate::vostok::controller::{IdleTest, SpeedMeasure, Step, StepEnd};
use crate::vostok::{Context, Flow, NoExit};

/// The squared walk a whole tick would be asked at this step's pace; None
/// for a whole tick, whose own it is.
pub fn tick_worth(walk_squared: f32, step: Step) -> Option<f32> {
    match step {
        Step::Whole => None,
        Step::Part { of_tick } => Some(walk_squared / (of_tick * of_tick)),
    }
}

/// can_skeep_player_step before its test of the walk (`controller.idle_walk`).
pub fn on_idle_walk(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(mut test) = IdleTest::at(ctx)
        && let Some(walk) = tick_worth(test.walk_squared(), test.step())
    {
        test.set_walk_squared(walk);
    }
    Flow::Continue
}

/// assign_physics_transform's store of the step's speed (`player.speed_measured`).
pub fn on_speed_measured(ctx: &mut Context<'_>) -> Flow<NoExit> {
    match SpeedMeasure::at(ctx) {
        StepEnd::Tick => Flow::Continue,
        StepEnd::Part => Flow::Skip,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_whole_tick_is_asked_its_own_walk() {
        assert_eq!(tick_worth(4.0e-6, Step::Whole), None);
    }

    #[test]
    fn a_part_is_asked_its_ticks_walk() {
        // 1 ms of a 3.106 m/s walk: 3.106 mm, under the 3.16 mm a step needs;
        // its tick's walk is 155.3 mm.
        let walk = 0.003_106_f32;
        let asked = tick_worth(walk * walk, Step::parse(0.001, 0.05)).unwrap();
        assert!((asked.sqrt() - 0.1553).abs() < 1.0e-4, "{asked}");
        // a body creeping under 63 mm/s is at rest in a part as in a tick
        let creep = 0.05 * 0.001;
        assert!(tick_worth(creep * creep, Step::parse(0.001, 0.05)).unwrap() < 1.0e-5);
    }

    #[test]
    fn only_a_part_of_a_tick_is_a_part() {
        assert_eq!(Step::parse(0.05, 0.05), Step::Whole);
        assert_eq!(Step::parse(0.049, 0.05), Step::Part { of_tick: 0.049 / 0.05 });
        for odd in [0.0, -0.01, 0.06, f32::NAN, f32::INFINITY] {
            assert_eq!(Step::parse(odd, 0.05), Step::Whole, "{odd}");
        }
        assert_eq!(Step::parse(0.01, 0.0), Step::Whole);
    }
}
