//! The damage zones (`hazards`): how far a bot keeps from one, and the way out of one it starts in.

use super::*;
use super::limits::Metres;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct HazardTuning {
    /// Metres kept from a damage zone.
    pub margin_m: Metres,
    /// The way out of a zone the bot starts in ([`EscapeRule`]).
    pub escape: EscapeRule,
}

impl Default for HazardTuning {
    fn default() -> Self {
        Self { margin_m: Metres::of(1.0), escape: EscapeRule::UnderStartZone }
    }
}

/// How a bot finds its way out of a damage zone (`Tactics::escape`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EscapeRule {
    /// Stays under the strength of the zone it starts in.
    UnderStartZone,
    /// The nearest way over any zone, as it was.
    Nearest,
}

impl EscapeRule {
    const UNDER_START_ZONE: &'static str = "under_start_zone";
    const NEAREST: &'static str = "nearest";
}

/// In a file, a name; or a flag, as the key `escape_ceiling` had it (`true`: under the start zone).
impl<'de> Deserialize<'de> for EscapeRule {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Wire {
            Flag(bool),
            Name(String),
        }
        match Wire::deserialize(deserializer)? {
            Wire::Flag(true) => Ok(Self::UnderStartZone),
            Wire::Flag(false) => Ok(Self::Nearest),
            Wire::Name(name) => match name.as_str() {
                Self::UNDER_START_ZONE => Ok(Self::UnderStartZone),
                Self::NEAREST => Ok(Self::Nearest),
                _ => Err(serde::de::Error::custom(format!(
                    "escape: `{}` or `{}`, not `{name}`",
                    Self::UNDER_START_ZONE,
                    Self::NEAREST
                ))),
            },
        }
    }
}

impl Serialize for EscapeRule {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(match self {
            Self::UnderStartZone => Self::UNDER_START_ZONE,
            Self::Nearest => Self::NEAREST,
        })
    }
}
