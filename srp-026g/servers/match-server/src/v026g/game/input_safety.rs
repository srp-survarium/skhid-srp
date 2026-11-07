//! Distinct invalid inputs are evidence about only their identified sender.
use std::time::{Duration, Instant};
const STRIKE_WINDOW: Duration = Duration::from_secs(10);
const STRIKE_LIMIT: u8 = 8;

#[derive(Default)]
pub(super) enum InputViolations {
    #[default]
    None,
    Counting { since: Instant, count: u8 },
    Remove,
}
impl InputViolations {
    /// Called only for distinct input times after deduplication, from the
    /// identified sender's link. Isolated corruption expires; retransmits
    /// cannot accumulate strikes. World compatibility guesses are not evidence.
    pub(super) fn record(&mut self, now: Instant) -> bool {
        *self = match *self {
            Self::Counting { since, count } if now.saturating_duration_since(since) < STRIKE_WINDOW => {
                if count + 1 >= STRIKE_LIMIT { Self::Remove }
                else { Self::Counting { since, count: count + 1 } }
            }
            Self::Remove => Self::Remove,
            _ => Self::Counting { since: now, count: 1 },
        };
        matches!(self, Self::Remove)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn isolated_input_corruption_expires_and_a_burst_removes_only_its_owner() {
        let now = Instant::now(); let mut state = InputViolations::default();
        for n in 0..20 { assert!(!state.record(now + STRIKE_WINDOW * n)); }
        for _ in 1..STRIKE_LIMIT-1 { assert!(!state.record(now + STRIKE_WINDOW * 20)); }
        assert!(!state.record(now + STRIKE_WINDOW * 20));
        assert!(state.record(now + STRIKE_WINDOW * 20));
    }
}
