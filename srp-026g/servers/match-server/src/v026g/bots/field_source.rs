//! Where the live zones come from today (`Tuning::live_zones`): the timed anomaly cores of the redesign's first form
//! (#225), read from the same world bytes the bots' simulation already reads (`Anomalies::sync_from_world`, the core's
//! random value found in the objects section and its state decoded). This is the **only** place that knows how a world
//! shows a field; a developer's other layout replaces [`v1`] (or adds a parser next to it) and nothing else changes:
//! [`WorldField`], [`FieldBoard`](super::live_zones::FieldBoard), `Hazards`, the switch and the counters are the
//! contract.
//!
//! A timed core is a stock `generic_anomaly_core` the pack builder makes (`scripts/dynamic-anomalies/README.md`, the
//! state table of `dynamic_anomalies.md` §3): energy on, an artefact that never respawns by itself (a respawn time of
//! 100,000 s or more), and the states
//!
//! | state | threshold | active | zones |
//! |---|---|---|---|
//! | charging | 3000 | 20 s | a charge zone (hurts nobody) |
//! | damage | 2000 | 4 s | the damage zone and the charge zone |
//! | artefact | 1000 | until taken | the damage zone |
//! | after the pick-up | 50 | 60 s | none |
//! | idle | 0 | until chosen | none |
//!
//! Its zones are not baked: a map holds up to 87 such cores (162-174 zones, idle nearly all match), and baking them
//! would cut hundreds of regions round places where nothing is. [`Hazards::zones_of`](super::hazards::Hazards::zones_of)
//! leaves them out; the board holds the few that are charging, damaging or waiting.

use mapdata::{Anomaly, MapData, Shape};

use super::anomalies::Anomalies;
use super::live_zones::{FieldId, FieldKind, FieldPhase, WorldField};

/// The energy thresholds of a timed core's states, highest first.
const CHARGING: i32 = 3000;
const DAMAGE: i32 = 2000;
const ARTEFACT: i32 = 1000;

/// An artefact that never respawns by itself: its respawn time, s, at least this (no stock core has more than 80).
const NEVER_RESPAWNS_S: f32 = 100_000.0;

/// Whether `anomaly` is a timed core as the pack builder makes them: energy on, an artefact that never respawns by
/// itself, and the charging and damage states with a time of their own, over an artefact state that lasts until the
/// artefact is taken.
pub fn timed(anomaly: &Anomaly) -> bool {
    let state = |threshold: i32| {
        anomaly
            .states
            .iter()
            .find(|s| s.enabled && s.threshold.round() as i32 == threshold)
    };
    anomaly.energy.enabled
        && anomaly.artefacts.respawn_s >= NEVER_RESPAWNS_S
        && state(CHARGING).is_some_and(|s| s.active_time_s > 0)
        && state(DAMAGE).is_some_and(|s| s.active_time_s > 0)
        && state(ARTEFACT).is_some_and(|s| s.active_time_s == 0)
}

/// The shape and kind of a timed core's damage zone: the harmful zone of its damage state (the charge zone beside it
/// hurts nobody).
fn damage_zone(anomaly: &Anomaly, map: &MapData) -> Option<(Shape, FieldKind)> {
    let damage = anomaly
        .states
        .iter()
        .find(|s| s.threshold.round() as i32 == DAMAGE)?;
    let zone = damage
        .groups
        .iter()
        .flat_map(|g| &g.zones)
        .filter_map(|name| map.zones.iter().find(|z| &z.name == name))
        .find(|z| z.damage.as_ref().is_some_and(|d| d.harmful()))?;
    let kind = match zone.damage_type.as_str() {
        "shock" => FieldKind::Spring,
        "radiation" => FieldKind::Onyx,
        // The lifebone's toxic zone, and any other taken as the strongest of the three kinds' hit.
        _ => FieldKind::Lifebone,
    };
    Some((*zone.shapes.first()?, kind))
}

/// The fields the timed cores of `map` show now: those a world gave in the charging, damage or artefact state (the
/// simulation runs a core on from its world, so a state that ends between worlds is followed). A core no world has
/// shown yet, one after its pick-up and one idle are no field. `FieldId` is the core's index in the map file.
pub fn v1(anomalies: &Anomalies, map: &MapData) -> Vec<WorldField> {
    let mut fields = Vec::new();
    for (c, core) in anomalies.cores().iter().enumerate() {
        let Some(anomaly) = map.anomalies.get(c).filter(|a| timed(a)) else {
            continue;
        };
        if core.synced_at().is_none() {
            continue;
        }
        let phase = match core.threshold().map(|t| t.round() as i32) {
            Some(CHARGING) => FieldPhase::Charging { damage_from: core.finish() },
            Some(DAMAGE) => FieldPhase::Damaging { until: Some(core.finish()) },
            Some(ARTEFACT) => FieldPhase::Holding,
            _ => continue,
        };
        let Some((shape, kind)) = damage_zone(anomaly, map) else {
            continue;
        };
        fields.push(WorldField { id: FieldId(c as u32), kind, shape, phase });
    }
    fields
}

/// Whether the zone `name` is a zone of a timed core of `map`: baked nowhere (the board holds it when it matters).
pub fn live_only(map: &MapData, zone: &str) -> bool {
    map.anomalies
        .iter()
        .any(|a| a.zones.iter().any(|z| z == zone) && timed(a))
}
