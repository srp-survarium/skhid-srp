//! Game time: the match clock in milliseconds, as the game keeps it (a
//! `u32`, wrapping), with the few operations the machines need, defined once.

use crate::TICK_MS;

/// A time on the match clock, in ms (the world's +18FC0h, the rollback
/// point, a move's target). Ordered as the game compares them (plainly);
/// a difference of two is [`GameTime::since`], wrap-safe.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GameTime(pub u32);

impl GameTime {
    /// The ms from `earlier` to `self`, modulo 2^32 (a time put back gives a
    /// large value, never a negative one).
    pub const fn since(self, earlier: GameTime) -> u32 {
        self.0.wrapping_sub(earlier.0)
    }

    /// The grid point at or before this time (the history's items are on
    /// the 50 ms grid, its origin a multiple of 50).
    pub const fn grid_floor(self) -> GameTime {
        GameTime(self.0 - self.0 % TICK_MS)
    }

    /// The earlier of the two.
    pub fn earliest(self, other: GameTime) -> GameTime {
        self.min(other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn since_wraps_and_grid_floor_is_the_grid_point_at_or_before() {
        assert_eq!(GameTime(1060).since(GameTime(1010)), 50);
        assert_eq!(GameTime(10).since(GameTime(20)), u32::MAX - 9);
        assert_eq!(GameTime(1049).grid_floor(), GameTime(1000));
        assert_eq!(GameTime(1050).grid_floor(), GameTime(1050));
        assert_eq!(GameTime(900).earliest(GameTime(850)), GameTime(850));
    }
}
