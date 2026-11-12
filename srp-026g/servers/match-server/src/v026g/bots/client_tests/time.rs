//! Match time in the runner: a moment on the match clock and a length of time, apart, so a step cannot add two moments or
//! take a budget for a time of day. The rest of the bots keep their milliseconds as bare numbers (`u32` clocks); the runner
//! converts at the frame, once.

use std::ops::{Add, Sub};

/// A moment on the match clock, ms from its start.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct At(pub u32);

/// A length of time, ms.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Span(pub u32);

impl Span {
    pub const fn ms(ms: u32) -> Self {
        Self(ms)
    }

    pub const fn s(seconds: u32) -> Self {
        Self(seconds * 1_000)
    }

    pub fn as_ms(self) -> u32 {
        self.0
    }

    pub fn as_s(self) -> f64 {
        f64::from(self.0) / 1_000.0
    }
}

/// The time from `earlier` to this moment; none (zero) if it is not later.
impl Sub<At> for At {
    type Output = Span;

    fn sub(self, earlier: At) -> Span {
        Span(self.0.saturating_sub(earlier.0))
    }
}

impl Add<Span> for At {
    type Output = At;

    fn add(self, span: Span) -> At {
        At(self.0.saturating_add(span.0))
    }
}

impl Add<Span> for Span {
    type Output = Span;

    fn add(self, other: Span) -> Span {
        Span(self.0.saturating_add(other.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_moment_less_a_moment_is_a_span_and_never_negative() {
        assert_eq!(At(5_000) - At(2_000), Span::s(3));
        assert_eq!(At(1_000) - At(2_000), Span(0));
        assert_eq!(At(1_000) + Span::ms(500), At(1_500));
        assert!(At(10) < At(11) && Span::s(1) > Span::ms(999));
    }
}
