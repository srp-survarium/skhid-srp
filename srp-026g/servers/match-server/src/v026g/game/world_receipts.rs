//! Load receipts affect only their receiving connection, never a donor.
use std::time::{Duration, Instant};
use srp_validate::recovery::Outcome;
const RETRY_GAP: Duration = Duration::from_secs(1);

#[derive(Clone, Copy, Debug)]
pub(super) enum Pending {
    Nothing,
    OptionsRetry { due: Instant },
    /// A world is on its way: the transport gives it up if it is not acked.
    Offered { generation: u8 },
    /// Every part of it was acked: without a receipt by `due` it goes again.
    Delivered { generation: u8, due: Instant },
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
    /// The transport saw every part of the world of `generation` acked: its
    /// receipt is taken as lost at `due` (`overdue`). Only the first call
    /// for an offer counts, and none once its receipt came.
    pub(super) fn delivered(&mut self, generation: u8, due: Instant) {
        if let Self::Enabled { pending, .. } = self
            && matches!(*pending, Pending::Offered { generation: offered } if offered == generation)
        {
            *pending = Pending::Delivered { generation, due };
        }
    }
    pub(super) fn receive(&mut self, generation: u8, outcome: Outcome, now: Instant) -> Received {
        let Self::Enabled { next_retry, pending } = self else { return Received::Ignored; };
        let offered = match *pending {
            Pending::Offered { generation } | Pending::Delivered { generation, .. } => Some(generation),
            _ => None,
        };
        match outcome {
            Outcome::Loaded if offered == Some(generation) => {
                *pending = Pending::Loaded { generation };
                Received::Loaded
            }
            Outcome::Rejected if offered == Some(generation) => {
                let due = now.max(*next_retry);
                *next_retry = due + RETRY_GAP;
                *pending = Pending::Retry { generation, due };
                Received::Rejected
            }
            _ => Received::Ignored, // stale, duplicate, unsolicited or already settled
        }
    }
    /// The world of `generation` was acked and no receipt for it came by its due time.
    pub(super) fn overdue(&self, generation: u8, now: Instant) -> bool {
        matches!(self, Self::Enabled { pending: Pending::Delivered { generation: offered, due }, .. }
            if *offered == generation && now >= *due)
    }
    /// Whether the world of `generation` is there: without receipts, when
    /// its parts were all `acked`; with them, when its Loaded receipt came,
    /// acked or not.
    pub(super) fn delivery(&self, generation: u8, acked: bool) -> bool {
        match self {
            Self::Legacy => acked,
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
        assert!(state.delivery(0, true) && !state.delivery(0, false)); state.enable(now); state.offer(255);
        assert_eq!(state.receive(0, Outcome::Loaded, now), Received::Ignored); assert!(!state.delivery(255, false));
        assert_eq!(state.receive(255, Outcome::Rejected, now), Received::Rejected);
        for _ in 0..10_000 { assert_eq!(state.receive(255, Outcome::Rejected, now), Received::Ignored); }
        assert!(state.take_retry(255, now)); assert!(!state.take_retry(255, now));
        state.offer(0); state.receive(255, Outcome::Rejected, now);
        assert!(!state.take_retry(0, now)); state.receive(0, Outcome::Rejected, now);
        assert!(!state.take_retry(0, now)); assert!(state.take_retry(0, now + RETRY_GAP));
        state.offer(1); assert_eq!(state.receive(1, Outcome::Loaded, now), Received::Loaded);
        assert_eq!(state.receive(1, Outcome::Rejected, now), Received::Ignored); assert!(state.delivery(1, false));
    }

    #[test]
    fn a_receipt_is_overdue_only_past_the_wait_after_its_world_was_acked() {
        let now = Instant::now(); let due = now + Duration::from_secs(1); let mut state = WorldReceipts::default();
        state.offer(3); state.delivered(3, due); assert!(!state.overdue(3, due), "no receipts negotiated");
        state.enable(now); state.offer(3);
        assert!(!state.overdue(3, due + Duration::from_secs(60)), "a world not acked is the transport's");
        state.delivered(2, due); assert!(!state.overdue(3, due + Duration::from_secs(60)));
        state.delivered(3, due); state.delivered(3, due + Duration::from_secs(9));
        assert!(!state.overdue(3, due - Duration::from_millis(1))); assert!(state.overdue(3, due)); assert!(!state.overdue(2, due));
        assert_eq!(state.receive(3, Outcome::Loaded, now), Received::Loaded); assert!(!state.overdue(3, due));
        state.delivered(3, due); assert!(state.delivery(3, false), "a receipt before the acks settles it");
        state.offer(4); state.delivered(4, due);
        assert_eq!(state.receive(4, Outcome::Rejected, now), Received::Rejected);
        assert!(!state.overdue(4, due), "a refusal has its own retry");
    }
}
