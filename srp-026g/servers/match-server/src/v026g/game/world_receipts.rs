//! Load receipts affect only their receiving connection, never a donor.
use std::time::{Duration, Instant};
use srp_validate::recovery::Outcome;
const RETRY_GAP: Duration = Duration::from_secs(1);

#[derive(Clone, Copy, Debug)]
pub(super) enum Pending {
    Nothing,
    OptionsRetry { due: Instant },
    Offered { generation: u8 },
    Loaded { generation: u8 },
    Retry { generation: u8, due: Instant },
}

/// What a receipt did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Received {
    Loaded,
    /// The recipient refused the world it was offered: it gets a newer one.
    Rejected,
    /// Stale, duplicate, unsolicited, or for a connection without receipts.
    Ignored,
}

#[derive(Default)]
pub(super) enum WorldReceipts {
    #[default]
    Legacy,
    Enabled { next_retry: Instant, pending: Pending },
}
impl WorldReceipts {
    pub(super) fn enable(&mut self, now: Instant) {
        *self = Self::Enabled { next_retry: now, pending: Pending::Nothing };
    }
    pub(super) fn reject_options(&mut self, now: Instant) -> Received {
        if let Self::Enabled { next_retry, pending: pending @ Pending::Nothing } = self {
            let due = now.max(*next_retry);
            *next_retry = due + RETRY_GAP;
            *pending = Pending::OptionsRetry { due };
            Received::Rejected
        } else {
            Received::Ignored
        }
    }
    pub(super) fn take_options_retry(&mut self, now: Instant) -> bool {
        if let Self::Enabled { pending, .. } = self
            && matches!(*pending, Pending::OptionsRetry { due } if now >= due)
        {
            *pending = Pending::Nothing;
            true
        } else { false }
    }
    pub(super) fn offer(&mut self, generation: u8) {
        if let Self::Enabled { pending, .. } = self {
            *pending = Pending::Offered { generation };
        }
    }
    pub(super) fn receive(&mut self, generation: u8, outcome: Outcome, now: Instant) -> Received {
        let Self::Enabled { next_retry, pending } = self else { return Received::Ignored; };
        match (*pending, outcome) {
            (Pending::Offered { generation: expected }, Outcome::Loaded) if generation == expected => {
                *pending = Pending::Loaded { generation };
                Received::Loaded
            }
            (Pending::Offered { generation: expected }, Outcome::Rejected) if generation == expected => {
                let due = now.max(*next_retry);
                *next_retry = due + RETRY_GAP;
                *pending = Pending::Retry { generation, due };
                Received::Rejected
            }
            _ => Received::Ignored, // stale, duplicate, unsolicited or already settled
        }
    }
    pub(super) fn requires_receipt(&self) -> bool { !matches!(self, Self::Legacy) }
    pub(super) fn loaded(&self, generation: u8) -> bool {
        match self {
            Self::Legacy => true,
            Self::Enabled { pending: Pending::Loaded { generation: accepted }, .. } => *accepted == generation,
            Self::Enabled { .. } => false,
        }
    }
    pub(super) fn take_retry(&mut self, generation: u8, now: Instant) -> bool {
        let Self::Enabled { pending, .. } = self else { return false; };
        if matches!(*pending, Pending::Retry { generation: expected, due } if expected == generation && now >= due) {
            *pending = Pending::Nothing;
            true
        } else { false }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn receipts_are_bound_to_an_offer_and_retries_are_bounded() {
        let now = Instant::now(); let mut state = WorldReceipts::default();
        assert!(state.loaded(0)); state.enable(now); state.offer(255);
        assert_eq!(state.receive(0, Outcome::Loaded, now), Received::Ignored); assert!(!state.loaded(255));
        assert_eq!(state.receive(255, Outcome::Rejected, now), Received::Rejected);
        for _ in 0..10_000 { assert_eq!(state.receive(255, Outcome::Rejected, now), Received::Ignored); }
        assert!(state.take_retry(255, now)); assert!(!state.take_retry(255, now));
        state.offer(0); state.receive(255, Outcome::Rejected, now);
        assert!(!state.take_retry(0, now)); state.receive(0, Outcome::Rejected, now);
        assert!(!state.take_retry(0, now)); assert!(state.take_retry(0, now + RETRY_GAP));
        state.offer(1); assert_eq!(state.receive(1, Outcome::Loaded, now), Received::Loaded);
        assert_eq!(state.receive(1, Outcome::Rejected, now), Received::Ignored); assert!(state.loaded(1));
    }
}
