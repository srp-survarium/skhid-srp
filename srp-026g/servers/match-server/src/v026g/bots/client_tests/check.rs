//! The plan's first step: does the owner see the lines the runner says? The server can say a line only in the game's chat, a path
//! read in the client's code but never seen on the screen (`docs/notes/client_tests.md`), so every plan begins by asking the owner
//! to answer one with the keys the server does see: crouch twice. A try that fails says the lines do not show, and the owner
//! reads the printed card instead.

use super::frame::Frame;
use super::step::{Cond, CustomMeter, Group, Measure, Size, Span, Step, Test};

/// The step, in the group of the plan's first match (a step of any size). It starts as soon as it is armed, so a quick answer is
/// counted, and counts the owner's crouch going down and coming up: `C` held is two changes, a toggled `C` pressed twice is two.
pub fn message_check(group: Group) -> Step {
    let mut step = Step::new(
        "check-message",
        Group { size: Size::Any, ..group },
        "CHECK: if you can read this line in the game chat, hold C for a second, twice (within 20 seconds).",
    )
    .start(&[Cond::After(Span(0))])
    .settle(Span(0))
    .measure(Measure::Custom {
        names: &["crouch_changes"],
        make: |_| Box::new(Crouches::default()),
    })
    .bar("crouch_changes", Test::AtLeast(2.0))
    .budget_s(20);
    step.package = "check";
    step
}

/// The owner's crouch changes: down and up each count.
#[derive(Debug, Default)]
struct Crouches {
    /// His stance at the last frame, once seen.
    last: Option<bool>,
    changes: u32,
}

impl CustomMeter for Crouches {
    fn sample(&mut self, frame: &Frame) {
        let Some(owner) = frame.owner.as_ref() else {
            return;
        };
        let down = owner.body.crouched;
        if self.last.is_some_and(|before| before != down) {
            self.changes += 1;
        }
        self.last = Some(down);
    }

    fn value(&self, n: usize) -> Option<f64> {
        (n == 0).then(|| f64::from(self.changes))
    }
}
