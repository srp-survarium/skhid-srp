//! Distinct invalid inputs are evidence about only their identified sender.
//!
//! No timer decides: strikes are counted over a stretch in step, and the
//! stretch ends at an event, the server finding the sender's world wrong.
//! No client in step makes such an input (none in the 1.6M+ inputs of the
//! 480 rev50 recordings), so a count that never expires removes no honest
//! player.
//!
//! The limit is more inputs than a world gone wrong sends before the server
//! can know it: inputs come every 50 ms, and a wrong world is judged
//! `hash_settle_ms` (2.5 s) late and confirmed `divergence_confirm_ms` (1 s)
//! after, so up to 70 of its inputs come in step. A refused input is never
//! relayed, so the limit costs nobody anything (the owner, 2026-11-07: "we
//! can increase that limit a bit").
pub(in crate::v026g) const STRIKE_LIMIT: u8 = 100;

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
    /// No invalid input since it was last known out of step (or ever).
    #[default]
    None,
    /// Invalid inputs in step since then, fewer than [`STRIKE_LIMIT`].
    Counting(u8),
    Remove,
}
impl InputViolations {
    /// Called only for distinct input times after deduplication, from the
    /// identified sender's link: retransmits cannot accumulate strikes.
    /// World compatibility guesses are not evidence, and neither are the
    /// inputs of a world known to be wrong: those clear the strikes before
    /// them, which that world most likely made too.
    pub(super) fn record(&mut self, world: SenderWorld) -> bool {
        *self = match (*self, world) {
            (Self::Remove, _) => Self::Remove,
            (_, SenderWorld::OutOfStep) => Self::None,
            (Self::Counting(count), SenderWorld::InStep) if count + 1 >= STRIKE_LIMIT => Self::Remove,
            (Self::Counting(count), SenderWorld::InStep) => Self::Counting(count + 1),
            (Self::None, SenderWorld::InStep) => Self::Counting(1),
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
    fn the_last_invalid_input_in_step_removes_its_owner() {
        let mut state = InputViolations::default();
        for _ in 1..STRIKE_LIMIT { assert!(!state.record(SenderWorld::InStep)); }
        assert!(state.record(SenderWorld::InStep));
    }

    #[test]
    fn a_world_out_of_step_strikes_nothing_and_clears_the_strikes_before() {
        let mut state = InputViolations::default();
        for _ in 1..STRIKE_LIMIT { assert!(!state.record(SenderWorld::InStep)); }
        for _ in 0..100 { assert!(!state.record(SenderWorld::OutOfStep)); }
        for _ in 1..STRIKE_LIMIT { assert!(!state.record(SenderWorld::InStep)); }
        assert!(state.record(SenderWorld::InStep));
        // Removed stays removed.
        assert!(state.record(SenderWorld::OutOfStep));
    }
}
