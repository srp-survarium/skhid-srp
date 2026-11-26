//! Positions as integers: every game gets the same distances.

const fn within(value: i32) -> bool {
    -Position::LIMIT <= value && value <= Position::LIMIT
}

/// A point of the map in whole millimetres (x, y up, z), each within
/// [`Position::LIMIT`] of the origin.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Position([i32; 3]);

impl Position {
    /// 100 km, in mm: no map comes near, and distances fit 32 bits.
    pub const LIMIT: i32 = 100_000_000;

    /// A position of the game's, metres (a config's vector, a player's
    /// transform); `None` when a coordinate is not a number or is past the
    /// limit. Each coordinate is one single-precision multiply, cut to a
    /// whole millimetre towards zero.
    pub fn from_metres(metres: [f32; 3]) -> Option<Position> {
        let mut mm = [0i32; 3];
        for (out, value) in mm.iter_mut().zip(metres) {
            // A not-a-number fails the comparison too.
            if !(-100_000.0..=100_000.0).contains(&value) {
                return None;
            }
            *out = (value * 1000.0) as i32;
        }
        Position::from_mm(mm)
    }

    /// The wire's form; `None` past the limit.
    pub const fn from_mm(mm: [i32; 3]) -> Option<Position> {
        let [x, y, z] = mm;
        if within(x) && within(y) && within(z) {
            Some(Position(mm))
        } else {
            None
        }
    }

    pub const fn mm(self) -> [i32; 3] {
        self.0
    }

    /// For the game's view (the model, the minimap).
    pub fn metres(self) -> [f32; 3] {
        self.0.map(|mm| mm as f32 / 1000.0)
    }

    /// The point `mm` higher, kept within the limit.
    pub const fn raised(self, mm: i32) -> Position {
        let y = self.0[1].saturating_add(mm);
        let y = if y > Position::LIMIT {
            Position::LIMIT
        } else {
            y
        };
        Position([self.0[0], y, self.0[2]])
    }

    /// The distance to `other`, whole mm (rounded down).
    pub fn distance(self, other: Position) -> u32 {
        let mut sum = 0u64;
        for axis in 0..3 {
            // Each difference is within 2 x LIMIT: its square fits 64 bits,
            // and so do three of them.
            let d = u64::from(self.0[axis].abs_diff(other.0[axis]));
            sum += d * d;
        }
        root(sum)
    }
}

/// The whole square root (rounded down), bit by bit: no division, no float.
fn root(value: u64) -> u32 {
    let mut left = value;
    let mut result = 0u64;
    let mut bit = 1u64 << 62;
    while bit > left {
        bit >>= 2;
    }
    while bit != 0 {
        if left >= result + bit {
            left -= result + bit;
            result = (result >> 1) + bit;
        } else {
            result >>= 1;
        }
        bit >>= 2;
    }
    result as u32
}

#[cfg(test)]
pub(crate) fn root_for_tests(value: u64) -> u32 {
    root(value)
}
