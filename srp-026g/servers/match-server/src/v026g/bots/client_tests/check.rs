//! The plan's first step: does the owner see the lines the runner says? The server can say a line only in the game's chat, a path
//! read in the client's code but never seen on the screen (`docs/notes/client_tests.md`), so every plan begins by asking the owner
//! to answer one with the keys the server does see: crouch twice. A try that fails says the lines do not show, and the owner
//! reads the printed card instead.

use super::frame::Frame;
use super::step::{Cond, Group, Measure, Scratch, Size, Span, Step, Test};

/// The step, in the group of the plan's first match (a step of any size).
pub fn message_check(group: Group) -> Step {
    let mut step = Step::new(
        "check-message",
        Group { size: Size::Any, ..group },
        "CHECK: if you can read this line in the game chat, crouch twice (C, C) within 20 seconds.",
    )
    .start(&[Cond::After(Span::s(2))])
    .measure(Measure::Custom {
        names: &["crouch_presses"],
        sample: count_crouches,
        values: crouch_values,
    })
    .bar("crouch_presses", Test::AtLeast(2.0))
    .budget_s(20);
    step.package = "check";
    step
}

/// The owner's crouch presses: `numbers[0]` counts them, `numbers[1]` is 1 while he is down.
fn count_crouches(frame: &Frame, scratch: &mut Scratch) {
    if scratch.numbers.is_empty() {
        scratch.numbers = vec![0.0, 0.0];
    }
    let Some(owner) = frame.owner.as_ref() else {
        return;
    };
    let down = owner.body.crouched;
    if down && scratch.numbers[1] == 0.0 {
        scratch.numbers[0] += 1.0;
    }
    scratch.numbers[1] = f64::from(u8::from(down));
}

fn crouch_values(scratch: &Scratch) -> Vec<Option<f64>> {
    vec![Some(scratch.numbers.first().copied().unwrap_or(0.0))]
}
