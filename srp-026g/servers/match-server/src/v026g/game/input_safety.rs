//! Distinct invalid inputs are evidence about only their identified sender.
use std::time::{Duration, Instant};
const STRIKE_WINDOW: Duration = Duration::from_secs(10);
const STRIKE_LIMIT: u8 = 8;

/// What the server knows of a sender's world as its invalid input comes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SenderWorld {
    /// Nothing stands against it: an invalid input is its sender's doing.
    InStep,
    /// Its reports disagree with the majority's, or it is being resynced
    /// and has not loaded its world yet. A world gone wrong makes invalid
    /// inputs, and its resync is what puts that right (live, 2026-11-02:
    /// match 0x30e6dd21, player 10 sent 11 inputs with NaN rotations while
    /// it was found out of step and resynced, then played on).
    OutOfStep,
}

impl super::Peer {
    /// Its world, as far as its invalid inputs go.
    pub(super) fn sender_world(&self) -> SenderWorld {
        if self.diverged.is_some() || super::resyncing(self.stage) || self.resync_from.is_some() {
            SenderWorld::OutOfStep
        } else {
            SenderWorld::InStep
        }
    }
}

#[derive(Clone, Copy, Default)]
pub(super) enum InputViolations {
    #[default]
    None,
    Counting { since: Instant, count: u8 },
    Remove,
}
impl InputViolations {
    /// Called only for distinct input times after deduplication, from the
    /// identified sender's link. Isolated corruption expires; retransmits
    /// cannot accumulate strikes. World compatibility guesses are not evidence,
    /// and neither are the inputs of a world known to be wrong: those clear
    /// the strikes before them, which that world most likely made too.
    pub(super) fn record(&mut self, now: Instant, world: SenderWorld) -> bool {
        *self = match (*self, world) {
            (Self::Remove, _) => Self::Remove,
            (_, SenderWorld::OutOfStep) => Self::None,
            (Self::Counting { since, count }, SenderWorld::InStep)
                if now.saturating_duration_since(since) < STRIKE_WINDOW =>
            {
                if count + 1 >= STRIKE_LIMIT { Self::Remove }
                else { Self::Counting { since, count: count + 1 } }
            }
            (_, SenderWorld::InStep) => Self::Counting { since: now, count: 1 },
        };
        matches!(self, Self::Remove)
    }
}

#[cfg(test)]
impl super::Game {
    /// For tests: its invalid inputs now would not count against it.
    pub fn out_of_step(&self, index: usize) -> bool {
        self.peers[index]
            .as_ref()
            .is_some_and(|peer| peer.sender_world() == SenderWorld::OutOfStep)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn isolated_input_corruption_expires_and_a_burst_removes_only_its_owner() {
        let now = Instant::now(); let mut state = InputViolations::default();
        for n in 0..20 { assert!(!state.record(now + STRIKE_WINDOW * n, SenderWorld::InStep)); }
        for _ in 1..STRIKE_LIMIT-1 { assert!(!state.record(now + STRIKE_WINDOW * 20, SenderWorld::InStep)); }
        assert!(!state.record(now + STRIKE_WINDOW * 20, SenderWorld::InStep));
        assert!(state.record(now + STRIKE_WINDOW * 20, SenderWorld::InStep));
    }

    #[test]
    fn a_world_out_of_step_strikes_nothing_and_clears_the_strikes_before() {
        let now = Instant::now(); let mut state = InputViolations::default();
        for _ in 1..STRIKE_LIMIT { assert!(!state.record(now, SenderWorld::InStep)); }
        for _ in 0..100 { assert!(!state.record(now, SenderWorld::OutOfStep)); }
        for _ in 1..STRIKE_LIMIT { assert!(!state.record(now, SenderWorld::InStep)); }
        assert!(state.record(now, SenderWorld::InStep));
        // Removed stays removed.
        assert!(state.record(now, SenderWorld::OutOfStep));
    }
}
