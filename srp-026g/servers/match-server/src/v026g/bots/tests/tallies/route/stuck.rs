//! The bots' "is stuck" notes, and those of them by the walk keys alone.

use super::*;

#[derive(Default)]
pub(super) struct Stuck {
    stuck: (u32, u32),
}

impl RouteTopic for Stuck {
    fn note(&mut self, _run: &RouteRun, note: &Note) {
        if note.text.starts_with("is stuck at") {
            self.stuck.0 += 1;
            self.stuck.1 += u32::from(note.text.contains("(walk keys held"));
        }
    }

    fn line(&mut self, _run: &RouteRun, line: &mut Line) {
        line.main.push(format!("stuck {} stuck_keys {}", self.stuck.0, self.stuck.1));
    }
}
