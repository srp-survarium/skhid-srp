//! Load receipts affect only their receiving connection, never a donor.
//!
//! Each refusal is answered once, at once, and told from a copy by what it
//! names: 0xE0 is plain and resent until acked, so copies of one receipt
//! come (transport.md section 3.3). A world's receipt names its generation;
//! a refusal that cannot name what it refuses (a handshake, or a world
//! whose generation could not be read) its number on the connection (1,
//! 2, ...: the client's count of such refusals). Nothing here waits on a clock: a
//! receipt the game sends always comes while its connection lives.
use srp_validate::recovery::Outcome;

#[derive(Clone, Copy, Debug)]
pub(super) enum Pending {
    Nothing,
    /// The game refused the handshake: it goes again.
    OptionsRetry,
    /// A world is on its way, until its receipt.
    Offered { generation: u8 },
    Loaded { generation: u8 },
    /// The game refused the world: a newer one goes.
    Retry { generation: u8 },
}

/// What a receipt did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Received {
    Loaded,
    /// The recipient refused what it was offered: it gets it again, or a
    /// newer world.
    Rejected,
    /// Stale, duplicate, unsolicited, or for a connection without receipts.
    Ignored,
}

#[derive(Default)]
pub(super) enum WorldReceipts {
    #[default]
    Legacy,
    Enabled {
        pending: Pending,
        /// The number of the newest refusal by number seen (a handshake's,
        /// or an unreadable world's).
        numbered_seen: Option<u8>,
    },
}
impl WorldReceipts {
    pub(super) fn enable(&mut self) {
        *self = Self::Enabled { pending: Pending::Nothing, numbered_seen: None };
    }
    /// The game's refusal number `number` of the handshake. Only a number
    /// newer than the last seen is answered: an older or the same one is a
    /// copy.
    pub(super) fn reject_options(&mut self, number: u8) -> Received {
        let Self::Enabled { pending, numbered_seen } = self else { return Received::Ignored; };
        let fresh = seen_first(numbered_seen, number);
        if fresh && matches!(pending, Pending::Nothing | Pending::OptionsRetry) {
            *pending = Pending::OptionsRetry;
            Received::Rejected
        } else {
            Received::Ignored
        }
    }
    /// The game's refusal number `number` of a world it could not read the
    /// generation of: taken as a refusal of the world on its way, and a
    /// newer one goes, as for a refusal that names it. Had a newer world
    /// gone meanwhile (the transport gave the refused one up), one world
    /// more goes than needed; none is waited for in vain.
    pub(super) fn reject_unreadable(&mut self, number: u8) -> Received {
        let Self::Enabled { pending, numbered_seen } = self else { return Received::Ignored; };
        let fresh = seen_first(numbered_seen, number);
        match *pending {
            Pending::Offered { generation } if fresh => {
                *pending = Pending::Retry { generation };
                Received::Rejected
            }
            _ => Received::Ignored,
        }
    }
    pub(super) fn take_options_retry(&mut self) -> bool {
        if let Self::Enabled { pending, .. } = self
            && matches!(*pending, Pending::OptionsRetry)
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
    pub(super) fn receive(&mut self, generation: u8, outcome: Outcome) -> Received {
        let Self::Enabled { pending, .. } = self else { return Received::Ignored; };
        match (*pending, outcome) {
            (Pending::Offered { generation: offered }, Outcome::Loaded) if offered == generation => {
                *pending = Pending::Loaded { generation };
                Received::Loaded
            }
            (Pending::Offered { generation: offered }, Outcome::Rejected) if offered == generation => {
                *pending = Pending::Retry { generation };
                Received::Rejected
            }
            _ => Received::Ignored, // stale, duplicate, unsolicited or already settled
        }
    }
    /// Whether the world of `generation` is there: without receipts, when
    /// its parts were all `acked`; with them, when its Loaded receipt came,
    /// acked or not. A world acked with no receipt yet is waited for.
    pub(super) fn delivery(&self, generation: u8, acked: bool) -> bool {
        match self {
            Self::Legacy => acked,
            Self::Enabled { pending: Pending::Loaded { generation: accepted }, .. } => *accepted == generation,
            Self::Enabled { .. } => false,
        }
    }
    pub(super) fn take_retry(&mut self, generation: u8) -> bool {
        let Self::Enabled { pending, .. } = self else { return false; };
        if matches!(*pending, Pending::Retry { generation: refused } if refused == generation) {
            *pending = Pending::Nothing;
            true
        } else { false }
    }
}

/// Whether `number` is newer than the newest numbered refusal seen (by
/// wrap-around order), now seen: an older or the same one is a copy.
fn seen_first(seen: &mut Option<u8>, number: u8) -> bool {
    let first = seen.is_none_or(|newest| (number.wrapping_sub(newest) as i8) > 0);
    if first { *seen = Some(number); }
    first
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn receipts_are_bound_to_an_offer_and_each_refusal_is_answered_once() {
        let mut state = WorldReceipts::default();
        assert!(state.delivery(0, true) && !state.delivery(0, false)); state.enable(); state.offer(255);
        assert_eq!(state.receive(0, Outcome::Loaded), Received::Ignored); assert!(!state.delivery(255, true));
        assert_eq!(state.receive(255, Outcome::Rejected), Received::Rejected);
        // Its copies, however many: one retry.
        for _ in 0..10_000 { assert_eq!(state.receive(255, Outcome::Rejected), Received::Ignored); }
        assert!(state.take_retry(255)); assert!(!state.take_retry(255));
        // The next world's refusal is answered at once too; the old one's
        // copies still nothing.
        state.offer(0); assert_eq!(state.receive(255, Outcome::Rejected), Received::Ignored);
        assert!(!state.take_retry(0)); assert_eq!(state.receive(0, Outcome::Rejected), Received::Rejected);
        assert!(state.take_retry(0));
        state.offer(1); assert_eq!(state.receive(1, Outcome::Loaded), Received::Loaded);
        assert_eq!(state.receive(1, Outcome::Rejected), Received::Ignored);
        assert!(state.delivery(1, false), "a receipt before the acks settles it");
    }

    #[test]
    fn a_handshake_refusal_is_answered_once_by_its_number() {
        let mut state = WorldReceipts::default();
        assert_eq!(state.reject_options(1), Received::Ignored, "no receipts negotiated");
        state.enable();
        assert_eq!(state.reject_options(1), Received::Rejected);
        assert_eq!(state.reject_options(1), Received::Ignored, "a copy");
        assert!(state.take_options_retry()); assert!(!state.take_options_retry());
        assert_eq!(state.reject_options(1), Received::Ignored, "a late copy");
        assert_eq!(state.reject_options(2), Received::Rejected);
        assert_eq!(state.reject_options(1), Received::Ignored, "a copy of the first, after the second");
        assert!(state.take_options_retry());
        for number in 3..=255u8 { assert_eq!(state.reject_options(number), Received::Rejected); }
        assert_eq!(state.reject_options(0), Received::Rejected, "the number wraps");
        state.offer(0);
        assert_eq!(state.reject_options(1), Received::Ignored, "no handshake once a world is offered");
    }

    #[test]
    fn an_unreadable_world_is_a_refusal_of_the_world_on_its_way_once_by_its_number() {
        let mut state = WorldReceipts::default();
        state.enable();
        assert_eq!(state.reject_options(1), Received::Rejected); assert!(state.take_options_retry());
        assert_eq!(state.reject_unreadable(2), Received::Ignored, "no world offered");
        state.offer(7);
        assert_eq!(state.reject_unreadable(2), Received::Ignored, "seen already: a copy");
        assert_eq!(state.reject_unreadable(3), Received::Rejected);
        assert_eq!(state.reject_unreadable(3), Received::Ignored, "a copy");
        assert!(state.take_retry(7));
        state.offer(8);
        assert_eq!(state.reject_unreadable(3), Received::Ignored, "a late copy, for the next world");
        assert_eq!(state.reject_unreadable(4), Received::Rejected);
        assert!(state.take_retry(8));
        state.offer(9); assert_eq!(state.receive(9, Outcome::Loaded), Received::Loaded);
        assert_eq!(state.reject_unreadable(5), Received::Ignored, "loaded: nothing on its way");
        assert!(state.delivery(9, false));
    }
}
