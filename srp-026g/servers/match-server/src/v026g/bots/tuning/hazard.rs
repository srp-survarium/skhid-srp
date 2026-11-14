//! The damage zones (`hazards`): how far a bot keeps from one, and the way out of one it starts in.

use super::*;
use super::limits::Metres;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct HazardTuning {
    /// Metres kept from a damage zone: refused by routes. Without [`Self::capsule_marks`] refused to stand on too; with
    /// it the band round the floors the client hurts on, a stand priced at the band's share ([`Self::hurt_by_shape`])
    /// that routes enter only for their last leg.
    pub margin_m: Metres,
    /// [A395a] A body may stand anywhere but where the client's damage zone lists a standing body: its controller's
    /// capsule (radius 0.38, height 1.8: `stand_capsule_width` 0.76 / 2, `stand_capsule_height`,
    /// `gameplay/players/default.player`, the grid's body) overlaps the zone's shape by Bullet's exact pair test, the
    /// contact counted only below zero distance (`damage_zone_core::tick` 0x319A70 -> `collision_sensor::contact_test`
    /// 0x307FA0 -> `bt_ghost_object::contact_test` 0x357C20 -> `btCollisionWorld::contactPairTest` 0xA3E50; the
    /// callback 0x77890, `comiss` 0x77899; the capsule's bottom at the feet, `setup_crouch_state` 0x7F28B; PROVEN,
    /// `owner-5/zone-contact-findings.md`). So no margin at or over a zone's flat top, 0.38 m beside its side
    /// (`hazards::capsule_touches`). The floors of the `margin_m` band the capsule does not touch are stands and goals
    /// (`Hazards::stand_allowed`): Vector Laboratory's battery floor, 0.42 m over the pit zone's top, which the
    /// recordings show no body losing on (49 body worlds), was refused, and a body there walked out over its "zone
    /// floors" into the zone (owner-5: 84 walk-outs, 74 from such dry floors, 10 into the zone, 7 deaths). The way out
    /// of a zone (`Tactics::zone_exit`, the drive's walk-out) starts only on the capsule's floors; from the band a body
    /// leaves over band floors, never a hurt one. Routes still keep off the band but for the last leg to a goal on it
    /// (or the first from a start on it), by the shortest walk (`Tactics::band_way`): a safety margin for our motor,
    /// not a client rule (`hazards::Marking::Capsule`: routes over the band carried bodies onto hurt floors 196 times
    /// against 11 on the A395a bench, 2026-11-14), to lift after A388. Off: the whole margin is refused, as it was.
    /// Off by default: on the A395a option (a) bench (2026-11-14, 37 of 64 matches) bodies spent 289 seconds on hurt
    /// floors against 101 off, 9 of the 15 spells traced on Vector Laboratory starting within 3 s of leaving the band:
    /// at the pit rim's gap jumps the grid held the body in the jump, and at the drop onto the battery floor it landed
    /// 2.7-3.0 m off that floor, both into the pit. Those are faults of our motor and grid builder (A388 the model and
    /// the edge, A389 drops from where the body leaves), not of the zone rule; to bench again after them.
    pub capsule_marks: bool,
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
    /// body's shape for a player is the character controller's capsule (PROVEN: [`Self::capsule_marks`]). Where in the shape
    /// only scales the damage. A battery on a floor 0.27 m over a cylinder's top overlaps nothing, but was priced as a floor in the
    /// zone (A129). Only the floor the take is made on is priced so (`Crossing::at_per_s`): the margin's band is
    /// priced there at the share of the zone's hit the motor's drift into it makes ([`Self::band_drift_m`]). The legs
    /// of a crossing stay at the margin's price, which is the motor's own (`crossing::hurt`, `crossable`): a battery
    /// offered at a lower price than the motor accepts is walked to and refused (`no way` x3-4 on the bench). Routes keep
    /// their margin. Off: the margin's marks price the stand too, as they did.
    pub hurt_by_shape: bool,
    /// The motor's lateral error from its route, metres (the scale of a half-normal drift): a body walking on a floor
    /// of the margin's band is inside the zone for the share `E[drift] / band width` of the time, `E = drift *
    /// sqrt(2/pi)`; the band floors cost that share of the zone's hit (`HurtTest::of`). A measured value of our motor,
    /// not a client value: the bodies walking their routes on band floors were a mean 0.2258 m off the route (1170
    /// samples, the A395a bench at 210f3c718 of 2026-11-14: 8 maps, Team Deathmatch and Batteries, seeds 1-2, the
    /// bench's `band_off` key, `tests/tallies/route/zones.rs`), so the scale is 0.2258 * sqrt(pi/2) = 0.283. A388
    /// changes the motor's drift: measure it again after A388. Used with `hurt_by_shape`.
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
    /// An escort's post is a floor the escort's walk reaches (`Tactics::past_fixed_zones`), not only a floor that is no
    /// zone's: a battery's stand or a dry island inside zones on all match is a goal no route leads to (the bench's
    /// Vostok Radar Station: a carrier of a zone battery gave 32 escort "no way to its goal" ends against 3, six bots in turn
    /// in 4 s on one battery, each play living a median 875 ms). A post that is none is pulled to the dry floor on the zone's
    /// edge that sees the carrier (`ring_post`), and no escort is offered a carrier no such floor sees, nor to a bot that
    /// stands inside such a pocket itself. Off: the post on his line is any goal, as it was.
    pub escort_reach: bool,
    /// A zone's hit falls off towards its rim: the client scales it by the zone's hit curve at the body's normalised
    /// distance from the zone's centre (`damage_zone_core::damage_of_body` 0x31A410, PROVEN; the shipped curves, Vector
    /// Laboratory's radiation zones `(0, 1)` to `(1, 0)`, levels 3 to 6 at the rim), and live bots lose exactly that (the
    /// clients simulate them). A floor is priced at the hit its place in the zone makes (`Hazard::intensity_at`, the
    /// curve `mapdata::Zone::intensity_at` has): the legs of a crossing, its stand and its way out. What may be crossed at
    /// all (`crossing::crossable`, the margin's marks) stays at the zone's full hit. Off: the full hit on every floor, as
    /// it was. (The zones' rate itself is the client's, the interval included, whatever this says: `Hazard::hit_per_s`.)
    pub zone_intensity: bool,
    /// A body on a floor a damage zone hurts on now escapes (`Pace::Escape(EscapeWhy::Zone)`: the legs' top speed, the sprint on any route, the sights, crouch and side
    /// steps of the reflex let go, the trigger kept) for as long as it is on it: the way out of a zone is walked at the escape pace, the play's pace aside. The zones kill by
    /// a pool that runs out in 4-5 s (the bench's Vector Laboratory: 21 of 21 zone deaths were the infection pool in a zone on all match, 3.2-4.7 s in it, the bots walking
    /// out at 2.3-3.5 m/s over 8-10 m of its floors; a plain `Pace::Run` did not help, 19 deaths against 20: it does not sprint on a short route), and a player runs out of a
    /// toxic cloud. Not in the reflex's fight legs, not crouched. Off: the play's pace.
    pub run_out: bool,
    /// A dry island, a floor of the walked map (no grid pocket) no zone on all match marks but that no walk from a spawn
    /// reaches without crossing one (`Tactics::dry_island`, `past_fixed_zones`), is never where a way out of zones ends, and a
    /// body on one walks out over the ring to the floors the spawns reach (`Tactics::on_island`). School's fruit cloud rings
    /// 48-52 floors (2 m) at y 13.6 at
    /// four corners: its level 2 zone counts as on all match, its levels 3-5 switch. A bot walking out of the cloud took
    /// the nearest dry floor, one of these, and no route left it (the ring is no crossing: its floors are the switching
    /// levels' too). 42 of 51 such stands of 60 s or more began with that walk-out (A257, the WAVE 13 camper cells, 61 % of
    /// all the stuck seconds). Off: a way out ends on the nearest dry floor, as it was: a body left standing there to the
    /// match's end, PROVEN by the probe of the server's grid and hazards (no path off without the ring's floors, no crossing,
    /// the escape ending on the island), so the all-off arm keeps it on (`KEPT_RIGHT`). Whole matches (A257, 2 x 256 pairs, 10
    /// a side, the 16 offered map-days): bot-ticks on dry islands -85 % / -95 %, standing still there -94 % / -99 %, "no way"
    /// -6.5 % / -11.9 %, kills, deliveries, zone deaths and stuck cases level.
    pub leave_islands: bool,
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
            capsule_marks: false,
            escape: EscapeRule::UnderStartZone,
            artefact_know: true,
            live_zones: true,
            zone_pools: ZonePools::Routed,
            route_crossing: RouteCrossing::Total,
            hurt_by_shape: true,
            band_drift_m: Metres::of(0.283),
            price_way_out: true,
            escort_dry_post: true,
            escort_reach: true,
            zone_intensity: true,
            run_out: true,
            leave_islands: true,
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
