//! 9-bit sequence numbers (PROVEN: masked to 9 bits at 0x2001F7490, cyclic comparator 0x2001652F0).

use std::collections::BTreeSet;

/// Sequence numbers count modulo 512.
pub const MASK: u16 = 0x1FF;

/// "Nothing received yet" as an `R` value: the client's first datagram carries R = 0x1FF.
pub const NONE: u16 = MASK;

/// Half window of the cyclic comparison.
pub const HALF: u16 = 256;

/// `a` is newer than `b` (cyclic, half window 256).
pub fn is_newer(a: u16, b: u16) -> bool {
    a != b && (a.wrapping_sub(b) & MASK) < HALF
}

/// The seq after `s`.
pub fn next(s: u16) -> u16 {
    s.wrapping_add(1) & MASK
}

/// What [`Received::record`] found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Recorded {
    New,
    /// Seen before: its messages are not delivered again.
    Duplicate,
    /// 256 or more behind the newest: too old to judge, dropped.
    TooOld,
}

/// The peer seqs received: the newest one and the set inside the window behind it.
#[derive(Clone, Debug, Default)]
pub struct Received {
    newest: Option<u16>,
    got: BTreeSet<u16>,
}

impl Received {
    pub fn newest(&self) -> Option<u16> {
        self.newest
    }

    pub fn contains(&self, seq: u16) -> bool {
        self.got.contains(&seq)
    }

    /// The seqs kept (all within 255 behind the newest).
    pub fn set(&self) -> &BTreeSet<u16> {
        &self.got
    }

    pub fn record(&mut self, seq: u16) -> Recorded {
        let seq = seq & MASK;
        match self.newest {
            None => {
                self.newest = Some(seq);
                self.got.insert(seq);
                Recorded::New
            }
            Some(newest) => {
                if seq == newest || self.got.contains(&seq) {
                    return Recorded::Duplicate;
                }
                if is_newer(seq, newest) {
                    self.newest = Some(seq);
                    self.got.insert(seq);
                    // forget seqs 256+ behind
                    self.got.retain(|g| (seq.wrapping_sub(*g) & MASK) < HALF);
                    Recorded::New
                } else if (newest.wrapping_sub(seq) & MASK) < HALF {
                    self.got.insert(seq);
                    Recorded::New
                } else {
                    Recorded::TooOld
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comparator_wraps() {
        assert!(is_newer(1, 0));
        assert!(is_newer(0, 0x1FF));
        assert!(is_newer(0x100, 0x0FF));
        assert!(!is_newer(0x0FF, 0x100));
        assert!(!is_newer(5, 5));
        // exactly 256 apart: not newer either way... (a - b) & M == 256 is not < 256
        assert!(!is_newer(0x100, 0));
        assert!(!is_newer(0, 0x100));
        assert_eq!(next(0x1FF), 0);
    }

    #[test]
    fn received_window() {
        let mut r = Received::default();
        assert_eq!(r.record(0), Recorded::New);
        assert_eq!(r.record(0), Recorded::Duplicate);
        assert_eq!(r.record(3), Recorded::New);
        assert_eq!(r.record(2), Recorded::New); // late but inside the window
        assert_eq!(r.record(2), Recorded::Duplicate);
        assert_eq!(r.newest(), Some(3));
        // wrap past 255 keeps counting
        for s in 4..=0x1FF {
            r.record(s);
        }
        // seq 0 was forgotten (256+ behind) and is the next seq after the wrap
        assert_eq!(r.record(0), Recorded::New);
    }
}
