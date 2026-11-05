//! [CG-17] Planned drops on an exact walk (`Motor::exact_end`, this tick's or the last), and of them those whose
//! bot picked a battery up within 10 s.

use super::*;

pub(super) struct ExactFalls {
    /// When each body last landed a planned drop on an exact walk.
    exact_drop: Vec<Option<u32>>,
    falls_exact: (u32, u32),
}

impl ExactFalls {
    pub(super) fn new(bodies: usize) -> Self {
        Self { exact_drop: vec![None; bodies], falls_exact: (0, 0) }
    }
}

impl RouteTopic for ExactFalls {
    fn tick(&mut self, _run: &RouteRun, at: &TickAt<'_>) {
        if let Some(fall) = at.fall
            && fall.planned
            && fall.exact
        {
            self.falls_exact.0 += 1;
            self.exact_drop[at.place] = Some(at.time);
        }
        if let Some(CarryEvent::Began) = at.carry
            && let Some(dropped) = self.exact_drop[at.place].take()
            && at.time <= dropped + 10_000
        {
            self.falls_exact.1 += 1;
        }
    }

    fn line(&mut self, _run: &RouteRun, line: &mut Line) {
        line.main.push(format!("falls_exact {} falls_exact_taken {}", self.falls_exact.0, self.falls_exact.1));
    }
}
