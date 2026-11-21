//! The world's time and durations, in milliseconds.

/// A moment of the world's clock, ms. The clock is a u32 that wraps: two
/// moments are compared by their distance, never by their numbers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Time(u32);

/// A duration, ms: at most [`Ms::MAX`], so that sums of the rule's durations
/// stay far inside half the clock's range.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Ms(u32);

/// Half the clock's range: a moment less than this after another is "after".
const HALF: u32 = 1 << 31;

impl Ms {
    /// A day.
    pub const MAX: Ms = Ms(86_400_000);
    pub const ZERO: Ms = Ms(0);

    /// `None` over [`Ms::MAX`].
    pub const fn new(ms: u32) -> Option<Ms> {
        if ms <= Ms::MAX.0 { Some(Ms(ms)) } else { None }
    }

    /// A config's seconds (0.46c2 keeps them x 1000); `None` over a day.
    pub const fn from_seconds(seconds: u32) -> Option<Ms> {
        if seconds <= Ms::MAX.0 / 1000 {
            Some(Ms(seconds * 1000))
        } else {
            None
        }
    }

    pub const fn get(self) -> u32 {
        self.0
    }

    /// The sum, at most [`Ms::MAX`].
    pub const fn plus(self, other: Ms) -> Ms {
        let sum = self.0 + other.0;
        if sum > Ms::MAX.0 { Ms::MAX } else { Ms(sum) }
    }

    /// The difference, 0 when `other` is longer.
    pub const fn minus(self, other: Ms) -> Ms {
        Ms(self.0.saturating_sub(other.0))
    }
}

impl Time {
    pub const fn from_ms(ms: u32) -> Time {
        Time(ms)
    }

    pub const fn ms(self) -> u32 {
        self.0
    }

    pub const fn after(self, ms: Ms) -> Time {
        Time(self.0.wrapping_add(ms.0))
    }

    /// The time from `earlier` to this moment; `None` when this moment is
    /// before it.
    pub const fn since(self, earlier: Time) -> Option<u32> {
        let passed = self.0.wrapping_sub(earlier.0);
        if passed < HALF { Some(passed) } else { None }
    }

    /// The time from `earlier` to this moment, 0 when this moment is before it.
    pub const fn elapsed(self, earlier: Time) -> u32 {
        match self.since(earlier) {
            Some(passed) => passed,
            None => 0,
        }
    }

    /// Whether this moment (now) is at or after `deadline` (0.46c2's
    /// `now >= start + timeout`, 0x785DF0).
    pub const fn reached(self, deadline: Time) -> bool {
        self.since(deadline).is_some()
    }

    /// The time left from this moment (now) to `deadline`, 0 once reached.
    pub const fn left(self, deadline: Time) -> u32 {
        deadline.elapsed(self)
    }
}
