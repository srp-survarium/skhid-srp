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
    /// (`collect::routed_crossing_to`, [`RouteCrossing`]), and the motor walks it. The bench's switch,
    /// `SRP_BENCH_ROUTE_CROSSING=1`.
    pub route_crossing: RouteCrossing,
    /// Where a body is hurt is where its capsule overlaps the zone's shape (the grid body's radius and height), not
    /// the floors `margin_m` keeps routes off. The client's test (read in the exe, 2026-10-29): each tick the zone's
    /// ghost object lists the bodies whose physics shape overlaps its own by an exact contact test (distance below 0,
    /// no tolerance; `damage_zone_core::tick` 0x319a70, `collision_sensor::contact_test` 0x307fa0, PROVEN); the
    /// body's shape for a player is the character controller's capsule (LIKELY, not confirmed). Where in the shape
    /// only scales the damage. A battery on a floor 0.27 m over a cylinder's top overlaps nothing, but was priced as a floor in the
    /// zone (A129). Only the floor the take is made on is priced so (`Crossing::at_per_s`): the margin's band is
    /// priced there at the share of the zone's hit the motor's drift into it makes ([`Self::band_drift_m`]). The legs
    /// of a crossing stay at the margin's price, which is the motor's own (`crossing::hurt`, `crossable`): a battery
    /// offered at a lower price than the motor accepts is walked to and refused (`no way` x3-4 on the bench). Routes keep
    /// their margin. Off: the margin's marks price the stand too, as they did.
    pub hurt_by_shape: bool,
    /// The motor's lateral error from its route, metres (the scale of a half-normal drift): a body walking on a floor
    /// of the margin's band is inside the zone for the share `E[drift] / band width` of the time, `E = drift *
    /// sqrt(2/pi)`; the band floors cost that share of the zone's hit (`HurtTest::of`). GUESS 0.2: the bench's
    /// route-off median for bodies on their route (`paths-findings.md` §1, p50 ~0.2 m); the live client's is not
    /// measured. Used with `hurt_by_shape`.
    pub band_drift_m: Metres,
    /// A routed crossing's way back out is priced: a carrier of a battery in zones walks home, and where the way in is a
    /// drop onto a floor the margin alone marks (Vector Laboratory's spawner 26) the way out crosses the zone's floors
    /// (the owner's carry of 2026-10-27: 9 s on the zone's floor). The cost is that of the same search from the stand
    /// to the side's spawn, not the way in doubled. Off: the way in doubled, as it was.
    pub price_way_out: bool,
    /// An escort's post is the nearest dry floor that sees the carrier when none lies on its line
    /// (`escort::post`), and no escort is offered to a carrier no dry floor sees, instead of a post on the
    /// carrier's own feet in a zone ("no way to its goal" 18 and 54 times in a match). Off: the line, then the
    /// carrier's feet.
    pub escort_dry_post: bool,
}

/// How a battery no straight walk crosses to is offered a crossing by the motor's own route over the zones' floors
/// (`HazardTuning::route_crossing`; in a file also a flag: `true` is `total`, `false` `off`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum RouteCrossing {
    /// Only the straight crossings.
    Off,
    /// The route is a crossing when it meets a zone floor before its stop (as it was before A129).
    Strict,
    /// Whatever the route meets before the stop, even nothing: it is a way to the battery (A129).
    #[default]
    Total,
}

impl RouteCrossing {
    const OFF: &'static str = "off";
    const STRICT: &'static str = "strict";
    const TOTAL: &'static str = "total";
}

impl<'de> Deserialize<'de> for RouteCrossing {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Wire {
            Flag(bool),
            Name(String),
        }
        match Wire::deserialize(deserializer)? {
            Wire::Flag(true) => Ok(Self::Total),
            Wire::Flag(false) => Ok(Self::Off),
            Wire::Name(name) => match name.as_str() {
                Self::OFF => Ok(Self::Off),
                Self::STRICT => Ok(Self::Strict),
                Self::TOTAL => Ok(Self::Total),
                _ => Err(serde::de::Error::custom(format!(
                    "route_crossing: `{}`, `{}` or `{}`, not `{name}`",
                    Self::OFF,
                    Self::STRICT,
                    Self::TOTAL
                ))),
            },
        }
    }
}

impl Serialize for RouteCrossing {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(match self {
            Self::Off => Self::OFF,
            Self::Strict => Self::STRICT,
            Self::Total => Self::TOTAL,
        })
    }
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
            route_crossing: RouteCrossing::Total,
            hurt_by_shape: true,
            band_drift_m: Metres::of(0.2),
            price_way_out: true,
            escort_dry_post: true,
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
