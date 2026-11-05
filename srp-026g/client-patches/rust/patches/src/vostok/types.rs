//! The units the game counts in, each a type of its own (AGENTS.md "Types
//! and states"): what a `u32` means is said once, here, with its arithmetic.
//!
//! A state is one enum whose variants carry the data valid in it, never a
//! set of flags or a sentinel: `Option<GameTime>` for "no time" (the game's
//! -1 is parsed at the boundary), `#[repr(u8)]` / `#[repr(u32)]` enums where
//! the game's memory holds the state (each value the game writes a variant;
//! a byte read from the game is parsed with a `match`, never transmuted).

/// A time of the game's clock, in ms (the world's time, an input's time).
/// It wraps at 2^32 ms (49 days), as the game's does.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct GameTime(pub u32);

impl GameTime {
    /// The game's encoding of "none" in a time field (-1).
    pub const NONE_RAW: u32 = u32::MAX;

    /// A time field as the game holds it: -1 is none.
    pub fn from_raw(raw: u32) -> Option<GameTime> {
        (raw != Self::NONE_RAW).then_some(GameTime(raw))
    }

    /// A time or none, as the game writes it.
    pub fn to_raw(time: Option<GameTime>) -> u32 {
        time.map_or(Self::NONE_RAW, |t| t.0)
    }

    pub fn after(self, ms: u32) -> GameTime {
        GameTime(self.0.wrapping_add(ms))
    }

    /// The ms from `earlier` to this time (wrapping: a time just past the
    /// wrap is after one just before it).
    pub fn since(self, earlier: GameTime) -> u32 {
        self.0.wrapping_sub(earlier.0)
    }

    /// The tick this time is in.
    pub fn tick(self) -> Tick {
        Tick(self.0 / Tick::MS)
    }
}

/// A step of the simulation: the grid of 50 ms every world in the history
/// is on.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Tick(pub u32);

impl Tick {
    /// The simulation's step.
    pub const MS: u32 = 50;

    /// The tick's first ms.
    pub fn start(self) -> GameTime {
        GameTime(self.0.wrapping_mul(Self::MS))
    }
}

/// A place of a match's roster, 0..20 (the late join keeps twenty).
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Place(u8);

impl Place {
    /// Places a world keeps.
    pub const COUNT: usize = 20;

    pub fn new(raw: u32) -> Option<Place> {
        (raw < Self::COUNT as u32).then_some(Place(raw as u8))
    }

    /// The place a movie names in decimal (the Tab list row's `id`); None for
    /// anything else.
    pub fn parse(text: &[u8]) -> Option<Place> {
        if text.is_empty() || text.len() > 2 || !text.iter().all(u8::is_ascii_digit) {
            return None;
        }
        Place::new(text.iter().fold(0, |n, digit| n * 10 + u32::from(digit - b'0')))
    }

    pub fn get(self) -> u8 {
        self.0
    }

    pub fn index(self) -> usize {
        usize::from(self.0)
    }

    /// Every place, in order.
    pub fn all() -> impl Iterator<Item = Place> {
        (0..Self::COUNT as u8).map(Place)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times_wrap_and_parse() {
        assert_eq!(GameTime::from_raw(u32::MAX), None);
        assert_eq!(GameTime::from_raw(5), Some(GameTime(5)));
        assert_eq!(GameTime::to_raw(None), u32::MAX);
        assert_eq!(GameTime(u32::MAX - 9).after(20), GameTime(10));
        assert_eq!(GameTime(10).since(GameTime(u32::MAX - 9)), 20);
        assert_eq!(GameTime(149).tick(), Tick(2));
        assert_eq!(Tick(3).start(), GameTime(150));
    }

    #[test]
    fn places_are_under_twenty() {
        assert_eq!(Place::new(19).map(Place::get), Some(19));
        assert_eq!(Place::new(20), None);
        assert_eq!(Place::all().count(), 20);
        assert_eq!(Place::parse(b"0").map(Place::get), Some(0));
        assert_eq!(Place::parse(b"19").map(Place::get), Some(19));
        for bad in [&b""[..], b"20", b"-1", b"1a", b" 1", b"100"] {
            assert_eq!(Place::parse(bad), None, "{bad:?}");
        }
    }
}
