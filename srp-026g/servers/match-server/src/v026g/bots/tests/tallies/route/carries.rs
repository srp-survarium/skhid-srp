//! The carries: the batteries delivered (how long each took) and lost.

use super::*;

#[derive(Default)]
pub(super) struct Carries {
    /// The carries delivered (ms each) and lost.
    carried: Vec<u32>,
    lost: u32,
}

impl RouteTopic for Carries {
    fn tick(&mut self, _run: &RouteRun, at: &TickAt<'_>) {
        if let Some(CarryEvent::Ended { ms, home }) = at.carry {
            if home {
                self.carried.push(ms);
            } else {
                self.lost += 1;
            }
        }
    }

    fn line(&mut self, _run: &RouteRun, line: &mut Line) {
        let mean = self.carried.iter().map(|&x| f64::from(x)).sum::<f64>() / self.carried.len().max(1) as f64;
        line.main.push(format!("carried {} carry_ms {:.0} lost {}", self.carried.len(), mean, self.lost));
    }
}
