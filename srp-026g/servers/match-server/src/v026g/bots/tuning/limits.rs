//! Numbers whose limits are parsed, not validated: each type holds only values in its range, and a file or a
//! patch that gives another fails when it is read, naming the type. A section that uses them has no check to
//! keep in step with its fields.

use serde::{Deserialize, Serialize};

/// A length in metres, not below nothing.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(try_from = "f32", into = "f32")]
pub struct Metres(f32);

impl Metres {
    /// For a default: a value out of range does not compile.
    pub const fn of(metres: f32) -> Self {
        assert!(metres >= 0.0, "metres below nothing");
        Self(metres)
    }

    pub const fn get(self) -> f32 {
        self.0
    }
}

impl TryFrom<f32> for Metres {
    type Error = String;

    fn try_from(metres: f32) -> Result<Self, String> {
        if metres >= 0.0 { Ok(Self(metres)) } else { Err(format!("{metres} m is below nothing")) }
    }
}

impl From<Metres> for f32 {
    fn from(metres: Metres) -> Self {
        metres.0
    }
}

/// A number above nothing (a period, a rate).
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(try_from = "f32", into = "f32")]
pub struct Positive(f32);

impl Positive {
    pub const fn of(value: f32) -> Self {
        assert!(value > 0.0, "not above nothing");
        Self(value)
    }

    pub const fn get(self) -> f32 {
        self.0
    }
}

impl TryFrom<f32> for Positive {
    type Error = String;

    fn try_from(value: f32) -> Result<Self, String> {
        if value > 0.0 { Ok(Self(value)) } else { Err(format!("{value} is not above nothing")) }
    }
}

impl From<Positive> for f32 {
    fn from(value: Positive) -> Self {
        value.0
    }
}

/// A `(low, high)` pair in order (a range to draw from); in a file, `[low, high]`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "(f32, f32)", into = "(f32, f32)")]
pub struct Span(f32, f32);

impl Span {
    pub const fn of(low: f32, high: f32) -> Self {
        assert!(low <= high, "a span out of order");
        Self(low, high)
    }

    pub const fn get(self) -> (f32, f32) {
        (self.0, self.1)
    }
}

impl TryFrom<(f32, f32)> for Span {
    type Error = String;

    fn try_from((low, high): (f32, f32)) -> Result<Self, String> {
        if low <= high { Ok(Self(low, high)) } else { Err(format!("[{low}, {high}] is out of order")) }
    }
}

impl From<Span> for (f32, f32) {
    fn from(span: Span) -> Self {
        (span.0, span.1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_value_out_of_range_is_refused_when_it_is_read() {
        assert_eq!(serde_json::from_str::<Metres>("1.5").unwrap().get(), 1.5);
        assert!(serde_json::from_str::<Metres>("-0.1").is_err());
        assert_eq!(serde_json::from_str::<Positive>("4").unwrap().get(), 4.0);
        assert!(serde_json::from_str::<Positive>("0").is_err());
        assert_eq!(serde_json::from_str::<Span>("[0.25, 0.9]").unwrap().get(), (0.25, 0.9));
        assert!(serde_json::from_str::<Span>("[1, 0]").is_err());
        // And they are written as the plain numbers they were.
        assert_eq!(serde_json::to_string(&Span::of(0.25, 0.9)).unwrap(), "[0.25,0.9]");
        assert_eq!(serde_json::to_string(&Metres::of(1.0)).unwrap(), "1.0");
    }
}
