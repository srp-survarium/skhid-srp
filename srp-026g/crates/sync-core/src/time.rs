//! Game time and the simulation's grid: the match clock in milliseconds, as
//! the game keeps it (a `u32`, wrapping), and the ticks every world in the
//! history is on, with the few operations the machines need, defined once.

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

    /// The stock grid point at or before this time ([`Grid::STOCK`]).
    pub const fn grid_floor(self) -> GameTime {
        Grid::STOCK.floor(self)
    }

    /// The earlier of the two.
    pub fn earliest(self, other: GameTime) -> GameTime {
        self.min(other)
    }
}

/// The length of a tick: the step of the grid the simulation's history is
/// on. 50 ms is the game's own (`game_world_core` +18FBCh := 50); 25 and 20
/// keep every division by the tick in the exe a byte swap
/// (`netcode_port_069.md` 6.1: `/50` and `/25` are `51EB851Fh` with `shr 4`
/// and `shr 3`, `/20` is `CCCCCCCDh` with `shr 4`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TickLen {
    Ms20,
    Ms25,
    Ms50,
}

impl TickLen {
    /// The game's own tick.
    pub const STOCK: TickLen = TickLen::Ms50;

    /// The tick in ms.
    pub const fn ms(self) -> u32 {
        match self {
            TickLen::Ms20 => 20,
            TickLen::Ms25 => 25,
            TickLen::Ms50 => 50,
        }
    }

    /// The tick of so many ms; `None` for a length the game cannot run.
    pub const fn from_ms(ms: u32) -> Option<TickLen> {
        match ms {
            20 => Some(TickLen::Ms20),
            25 => Some(TickLen::Ms25),
            50 => Some(TickLen::Ms50),
            _ => None,
        }
    }

    /// The tick in seconds (the bots' mirror of a step).
    pub const fn seconds(self) -> f32 {
        self.ms() as f32 / 1000.0
    }
}

/// The simulation's grid: the times `origin + n * tick` the history's worlds
/// are at. The origin is the tick itself (the game's constructor and
/// `clear` set it to the tick: 50 stock), so the absolute grid (`t / tick`,
/// which atomic_tick, the wire and the replay read) and the origin-relative
/// one (tick_events, `fixed_time_before`) are the same points.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Grid {
    tick: TickLen,
}

impl Grid {
    /// The game's own grid: 50 ms from 50.
    pub const STOCK: Grid = Grid::of(TickLen::STOCK);

    /// The grid of a tick, its origin the tick.
    pub const fn of(tick: TickLen) -> Grid {
        Grid { tick }
    }

    pub const fn tick(self) -> TickLen {
        self.tick
    }

    /// The tick in ms.
    pub const fn tick_ms(self) -> u32 {
        self.tick.ms()
    }

    /// The first grid point (`game_world_core` +18FBCh).
    pub const fn origin(self) -> GameTime {
        GameTime(self.tick.ms())
    }

    /// The grid point at or before `time`.
    pub const fn floor(self, time: GameTime) -> GameTime {
        GameTime(time.0 - time.0 % self.tick.ms())
    }

    /// The grid point at or after `time` (where the game snaps an input
    /// that is not precise, snap_input 0x2D4E40).
    pub const fn ceil(self, time: GameTime) -> GameTime {
        GameTime(time.0.div_ceil(self.tick.ms()) * self.tick.ms())
    }

    /// The grid point after `time`'s own (floor) one.
    pub const fn next(self, time: GameTime) -> GameTime {
        GameTime(self.floor(time).0 + self.tick.ms())
    }

    /// Whether `time` is a grid point.
    pub const fn on_grid(self, time: GameTime) -> bool {
        time.0.is_multiple_of(self.tick.ms())
    }

    /// The ms of `time` into its tick (the "precise" part of a time).
    pub const fn into_tick(self, time: GameTime) -> u32 {
        time.0 % self.tick.ms()
    }

    /// The tick number of `time`: `time / tick`.
    pub const fn ticks(self, time: GameTime) -> u32 {
        time.0 / self.tick.ms()
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

    #[test]
    fn the_stock_grid_is_the_games_and_others_keep_its_shape() {
        assert_eq!(Grid::STOCK.tick_ms(), 50);
        assert_eq!(Grid::STOCK.origin(), GameTime(50));
        for tick in [TickLen::Ms20, TickLen::Ms25, TickLen::Ms50] {
            let grid = Grid::of(tick);
            let ms = tick.ms();
            assert_eq!(TickLen::from_ms(ms), Some(tick));
            assert_eq!(grid.origin(), GameTime(ms));
            assert_eq!(grid.floor(GameTime(10 * ms + 1)), GameTime(10 * ms));
            assert_eq!(grid.ceil(GameTime(10 * ms + 1)), GameTime(11 * ms));
            assert_eq!(grid.ceil(GameTime(10 * ms)), GameTime(10 * ms));
            assert_eq!(grid.next(GameTime(10 * ms)), GameTime(11 * ms));
            assert_eq!(grid.next(GameTime(10 * ms + 3)), GameTime(11 * ms));
            assert!(grid.on_grid(GameTime(7 * ms)) && !grid.on_grid(GameTime(7 * ms + 1)));
            assert_eq!(grid.into_tick(GameTime(7 * ms + 3)), 3);
            assert_eq!(grid.ticks(GameTime(7 * ms + 3)), 7);
        }
        assert_eq!(TickLen::from_ms(40), None);
        assert_eq!(TickLen::Ms20.seconds(), 0.02);
    }
}
