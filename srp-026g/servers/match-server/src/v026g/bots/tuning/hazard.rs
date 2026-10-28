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
    /// The bots know the artefact places of the map and what the cores say of them
    /// (`artefact_know.rs`): where each container is and how long a take holds, and, from every
    /// checked world, whether a core has an artefact due or lying. Knowledge only: nothing
    /// decides by it (picking and using artefacts are later work). Off: none of it is built.
    pub artefact_know: bool,
    /// Whether the bots read the anomaly fields a world holds (`Bots::fields_from_world`, `live_zones.rs`): the
    /// floors they cover are kept off while a field hurts, and a bot caught in one is got out. Off: the worlds' fields are not read, today's decisions (the bench's switch,
    /// `SRP_BENCH_LIVE_ZONES=1` on).
    pub live_zones: bool,
    /// What a battery's crossing of a damage zone is weighed against (F2, [`ZonePools`]): the body
    /// pool and the zone's raw hit (as it was), or the pools (the default) the zone's hits really
    /// take from, through the bot's armour on the parts it hits (`zone_cost.rs`). The bench's
    /// switch, `SRP_BENCH_ZONE_POOLS=routed`.
    pub zone_pools: ZonePools,
    /// A battery in zones on all match that no straight walk crosses to (a wall or a drop between it and
    /// the dry floor) is offered when the motor's own crossing route over the zones' floors is borne
    /// (`collect::routed_crossing_to`), and the motor walks it; off: only the straight
    /// crossings, as it was. The bench's switch, `SRP_BENCH_ROUTE_CROSSING=1`.
    pub route_crossing: bool,
}

/// What a crossing of a damage zone is weighed against (`HazardTuning::zone_pools`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ZonePools {
    /// The body pool and the zone's raw hit: toxic and radiation zones are measured against a pool they
    /// never touch, and no armour counts.
    Raw,
    /// The pools the zone's type routes to (infection, pain, body, head), by the armour the bot wears on the
    /// parts the zone hits.
    #[default]
    Routed,
}

impl Default for HazardTuning {
    fn default() -> Self {
        Self {
            margin_m: Metres::of(1.0),
            escape: EscapeRule::UnderStartZone,
            artefact_know: true,
            live_zones: true,
            zone_pools: ZonePools::Routed,
            route_crossing: true,
        }
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
