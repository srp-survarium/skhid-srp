//! The live zones (`live_zones.rs`, `Tuning::live_zones`): the anomaly fields of a world as a second source of zones
//! next to the baked ones.

use super::*;

use mapdata::NodeId;
use crate::v026g::bots::field_source;
use crate::v026g::bots::live_zones::{FieldId, FieldKind, FieldPhase, WorldField};
use crate::v026g::bots::anomalies::tests::{core_world, cylinder, electric_map, timed, zone};
use crate::v026g::bots::anomalies::Anomalies;
use crate::v026g::bots::hazards::{Activity, Hazards, ZoneSet, PASS_MS, SWITCH_MARGIN_MS};
use crate::v026g::bots::stuck::Stuck;
use crate::v026g::bots::tactics::tests::field_with;

/// A round field on the floor at (x, z).
fn field(id: u32, x: f32, z: f32, radius: f32, phase: FieldPhase) -> WorldField {
    WorldField {
        id: FieldId(id),
        kind: FieldKind::Lifebone,
        shape: mapdata::Shape {
            kind: mapdata::ShapeKind::Cylinder,
            center: Vec3::new(x, 1.0, z),
            rotation: mapdata::Rotation::IDENTITY,
            half: Vec3::new(radius, 2.0, radius),
        },
        phase,
    }
}

const DAMAGING: FieldPhase = FieldPhase::Damaging { until: None };

fn node_at(world: &World, x: f32, z: f32) -> NodeId {
    world.grid.locate(Vec3::new(x, 0.0, z), 0.3).unwrap()
}

/// With no field the board answers what the baked zones alone answer, floor by floor: no live set, the same
/// activity, the same hit, the same baked set.
#[test]
fn an_empty_board_answers_exactly_the_baked_zones() {
    let world = field_with(|map| {
        let baked = electric_map();
        map.zones = baked.zones;
        map.anomalies = baked.anomalies;
    });
    let mut hazards = Hazards::build(&world, 0.5);
    hazards.attach(Anomalies::new(&world.map, 0x8000_0042));
    hazards.advance(2_000);
    let before = hazards.version();
    assert!(!hazards.set_fields(&[], &world.grid, 2_000), "no field: nothing changed");
    assert_eq!(hazards.version(), before);
    assert!(hazards.live.is_empty());
    let mut marked = 0;
    for node in 0..world.grid.len() as NodeId {
        let set = hazards.set_of(node);
        assert_eq!(set.live(), 0);
        assert_eq!(set.baked() == 0, set.is_none());
        let baked_hit: f32 = hazards.zones_at(node).map(|z| z.hit_per_s()).sum();
        assert_eq!(hazards.hit_per_s(node), baked_hit);
        for time in [0, 20_000, 45_000, 70_000] {
            let baked_off = hazards.zones_at(node).all(|z| z.activity.off_for(time, PASS_MS));
            assert_eq!(hazards.allowed(node, time), baked_off);
            assert_eq!(
                hazards.on_at(node, time),
                hazards.zones_at(node).any(|z| z.activity.active(time))
            );
        }
        assert_eq!(hazards.marked_at(node), hazards.zone_of(node).is_some());
        assert_eq!(hazards.hazard_free(node), hazards.zone_of(node).is_none());
        marked += usize::from(!set.is_none());
    }
    assert!(marked > 0);
}

/// A charging field is harmless until its damage starts: a floor reached after it is shut, one a body clears before
/// it stays open (the arrival rule of the timer zones).
#[test]
fn a_charging_field_is_open_to_a_body_that_clears_it_before_the_damage() {
    let world = field_with(|_| {});
    let mut hazards = Hazards::build(&world, 0.5);
    let charging = FieldPhase::Charging { damage_from: 20_000 };
    assert!(hazards.set_fields(&[field(7, 20.0, 20.0, 2.0, charging)], &world.grid, 1_000));
    let inside = node_at(&world, 20.0, 20.0);
    assert!(!hazards.set_of(inside).is_none());
    assert_eq!(hazards.set_of(inside).baked(), 0, "a live floor is no baked one");
    assert!(hazards.zone_of(inside).is_none(), "the baked zone alone");
    assert!(hazards.marked_at(inside) && !hazards.hazard_free(inside));
    // Now, and for the pass time to come: open.
    assert!(hazards.allowed(inside, 1_000));
    // At 16 s the pass time (4 s) reaches the margin before the damage: shut, but not before.
    assert!(hazards.allowed(inside, 15_000 - 1));
    assert!(!hazards.allowed(inside, 16_000));
    // A body 10 m off gets there in 2 s: planned at 14 s, it arrives at 16 s: shut.
    assert!(!hazards.allowed_arriving(inside, 14_000, 10.0));
    assert!(hazards.allowed_arriving(inside, 10_000, 10.0));
    // After the damage it is on.
    assert!(hazards.on_at(inside, 21_000));
    assert!(!hazards.on_at(inside, 19_000 - 1), "on from the margin before the damage");
    assert!(hazards.on_at(inside, 19_000));
    assert!(!hazards.allowed(inside, 21_000));
    assert_eq!(hazards.hit_per_s(inside), 0.5);
    // Charging, it switches on at the margin before the damage.
    assert_eq!(hazards.next_switch(inside), Some(20_000 - SWITCH_MARGIN_MS));
}

/// The phases: damaging with an end is on until then (and the margin), damaging with none and holding stay on;
/// a field the world no longer lists is gone at once, one it lists unchanged changes nothing.
#[test]
fn the_phases_and_a_field_that_goes() {
    let world = field_with(|_| {});
    let mut hazards = Hazards::build(&world, 0.5);
    let inside = node_at(&world, 20.0, 20.0);
    let ending = FieldPhase::Damaging { until: Some(30_000) };
    hazards.set_fields(&[field(1, 20.0, 20.0, 2.0, ending)], &world.grid, 25_000);
    let v = hazards.version();
    assert!(!hazards.allowed(inside, 25_000));
    assert!(!hazards.allowed_for(inside, 30_000 + SWITCH_MARGIN_MS - 1, 0));
    assert!(hazards.allowed_for(inside, 30_000 + SWITCH_MARGIN_MS, 0));
    // The same world again: no change.
    assert!(!hazards.set_fields(&[field(1, 20.0, 20.0, 2.0, ending)], &world.grid, 26_000));
    assert_eq!(hazards.version(), v);
    // Holding: on, no end.
    let holding = FieldPhase::Holding;
    assert!(hazards.set_fields(&[field(1, 20.0, 20.0, 2.0, holding)], &world.grid, 40_000));
    assert!(hazards.version() > v);
    assert!(!hazards.allowed_for(inside, 4_000_000, 0));
    // Gone from the world: gone at once, the floors open.
    let v = hazards.version();
    assert!(hazards.set_fields(&[], &world.grid, 41_000));
    assert!(hazards.version() > v);
    assert!(hazards.allowed(inside, 41_000) && hazards.hazard_free(inside));
    assert!(hazards.set_of(inside).is_none());
}

/// A floor a live field marks is never crossable and never "fixed": a baked zone on all match under a live field is
/// no longer "only fixed", and the baked-only queries stay as they were.
#[test]
fn a_live_field_is_never_fixed_nor_crossable() {
    let world = field_with(|map| {
        map.zones.push(crate::v026g::bots::anomalies::tests::zone(
            "fire",
            crate::v026g::bots::anomalies::tests::cylinder(20.0, 20.0, 3.0),
            0.5,
        ));
    });
    let mut hazards = Hazards::build(&world, 0.5);
    let inside = node_at(&world, 20.0, 20.0);
    let beside = node_at(&world, 22.0, 20.0);
    assert!(hazards.only_fixed_at(inside) && hazards.fixed_at(inside));
    hazards.set_fields(&[field(3, 20.0, 20.0, 1.0, DAMAGING)], &world.grid, 0);
    assert!(!hazards.only_fixed_at(inside), "a live field on it: not crossable");
    assert!(hazards.fixed_at(inside), "fixed_at is the baked zones'");
    assert!(hazards.only_fixed_at(beside), "baked alone, away from the field");
    assert_eq!(hazards.fixed_zones(), vec![true]);
    // A live-only floor is no free floor.
    let mut empty = Hazards::build(&field_with(|_| {}), 0.5);
    empty.set_fields(&[field(3, 20.0, 20.0, 1.0, DAMAGING)], &world.grid, 0);
    assert!(!empty.only_fixed_at(inside) && !empty.fixed_at(inside));
    assert!(!empty.set_of(inside).is_none());
}

/// Two fields on a floor are a set of their own, stable while the same two lie there; the escapes of the stuck
/// counter are by set, so a second way out of the same field is counted and one out of another is not.
#[test]
fn a_second_escape_from_the_same_field_is_counted_and_from_another_is_not() {
    let world = field_with(|_| {});
    let mut hazards = Hazards::build(&world, 0.5);
    hazards.set_fields(
        &[field(1, 20.0, 20.0, 2.0, DAMAGING), field(2, 30.0, 20.0, 2.0, DAMAGING)],
        &world.grid,
        0,
    );
    let (one, two) = (node_at(&world, 20.0, 20.0), node_at(&world, 30.0, 20.0));
    let (a, b) = (hazards.set_of(one), hazards.set_of(two));
    assert_ne!(a, b);
    // Another phase of the same fields: the same sets.
    hazards.set_fields(
        &[field(1, 20.0, 20.0, 2.0, FieldPhase::Holding), field(2, 30.0, 20.0, 2.0, DAMAGING)],
        &world.grid,
        1_000,
    );
    assert_eq!((hazards.set_of(one), hazards.set_of(two)), (a, b));
    let goal = Vec3::new(5.0, 0.0, 5.0);
    let mut stuck = Stuck::default();
    assert!(!stuck.zone_escape(1_000, a, goal));
    stuck.zone_on(ZoneSet::default());
    assert!(stuck.zone_escape(2_000, a, goal), "out of the same field toward the same goal again");
    let mut stuck = Stuck::default();
    assert!(!stuck.zone_escape(1_000, a, goal));
    stuck.zone_on(ZoneSet::default());
    assert!(!stuck.zone_escape(2_000, b, goal), "another field is another escape");
}

/// The live path and the baked path agree on a map where both exist: the electric pair re-expressed as fields from
/// the same simulation's prediction (a field per on span: charging for the ten seconds before it, damaging till its
/// end and the margin) gives the same `allowed` as the baked activities, for the floors of both zones, every half
/// second over the first 200 s.
#[test]
fn the_live_path_agrees_with_the_baked_one_on_an_electric_pair() {
    let baked_world = field_with(|map| {
        let baked = electric_map();
        map.zones = baked.zones;
        map.anomalies = baked.anomalies;
    });
    let mut baked = Hazards::build(&baked_world, 0.5);
    baked.attach(Anomalies::new(&baked_world.map, 0x8000_0042));
    baked.advance(2_000);
    let schedules: Vec<_> = (0..2)
        .map(|z| baked.anomalies().unwrap().schedule(z).cloned().unwrap())
        .collect();
    let activities: Vec<_> = schedules
        .iter()
        .map(|s| Activity::from_schedule(s, SWITCH_MARGIN_MS))
        .collect();
    let horizon = schedules[0].until.min(200_000).saturating_sub(20_000);
    assert!(horizon > 60_000, "{horizon}");
    // The live side: the same world without the baked zones.
    let world = field_with(|_| {});
    let mut live = Hazards::build(&world, 0.5);
    let centres = [10.0, 30.0];
    let nodes = [node_at(&world, 10.0, 20.0), node_at(&world, 30.0, 20.0)];
    let mut checked = 0;
    let mut t = 2_000;
    while t < horizon {
        let mut fields = Vec::new();
        for (z, schedule) in schedules.iter().enumerate() {
            for (k, &(s, e)) in schedule.spans.iter().enumerate() {
                let id = (z * 1_000 + k) as u32;
                let phase = if (s..e.saturating_add(SWITCH_MARGIN_MS)).contains(&t) {
                    FieldPhase::Damaging { until: Some(e) }
                } else if s > t && s - t <= 10_000 {
                    FieldPhase::Charging { damage_from: s }
                } else {
                    continue;
                };
                fields.push(field(id, centres[z], 20.0, 2.0, phase));
            }
        }
        live.set_fields(&fields, &world.grid, t);
        for z in 0..2 {
            assert_eq!(
                live.allowed(nodes[z], t),
                activities[z].off_for(t, PASS_MS),
                "zone {z} at {t} ms: {:?}",
                schedules[z].spans
            );
            assert_eq!(live.on_at(nodes[z], t), activities[z].active(t), "zone {z} on at {t} ms");
            checked += 1;
        }
        t += 500;
    }
    assert!(checked > 300);
}

/// A map with one timed core (charge and damage zones at (20, 20)) and the stock electric pair of level_03.
fn timed_map() -> mapdata::MapData {
    let mut map = electric_map();
    map.zones.push(zone("charge", cylinder(20.0, 20.0, 3.0), 0.0));
    map.zones.push(zone("damage", cylinder(20.0, 20.0, 3.0), 0.5));
    map.anomalies.push(timed("timed", "charge", "damage"));
    map
}

/// The pack builder's cores are timed; the stock ones (the electric pair, a cloud) are not.
#[test]
fn a_timed_core_is_known_by_its_state_chain() {
    let map = timed_map();
    assert!(field_source::timed(&map.anomalies[1]));
    assert!(!field_source::timed(&map.anomalies[0]), "the electric pair");
    let mut other = map.anomalies[1].clone();
    other.artefacts.respawn_s = 60.0;
    assert!(!field_source::timed(&other), "an artefact that respawns by itself");
    let mut other = map.anomalies[1].clone();
    other.states.retain(|s| s.threshold.round() as i32 != 2000);
    assert!(!field_source::timed(&other), "no damage state");
    assert!(field_source::live_only(&map, "damage") && field_source::live_only(&map, "charge"));
    assert!(!field_source::live_only(&map, "a"), "a stock zone stays baked");
}

/// The zones of a timed core are not baked: the stock electric pair is, and the timed core's floors are no baked
/// zone's (so no region is cut round the place).
#[test]
fn a_timed_cores_zones_are_not_baked() {
    let world = field_with(|map| {
        let timed = timed_map();
        map.zones = timed.zones;
        map.anomalies = timed.anomalies;
    });
    let hazards = Hazards::build(&world, 0.5);
    let names: Vec<_> = hazards.zones.iter().map(|z| z.name.as_str()).collect();
    assert_eq!(names, ["a", "b"], "{names:?}");
    assert!(hazards.hazard_free(node_at(&world, 20.0, 20.0)));
}

/// A world shows the core charging, damaging, waiting and picked: the board follows, field by field.
#[test]
fn the_timed_core_of_a_world_is_a_field_that_follows_its_states() {
    let world = field_with(|map| {
        let timed = timed_map();
        map.zones = timed.zones;
        map.anomalies = timed.anomalies;
    });
    let mut hazards = Hazards::build(&world, 0.5);
    hazards.attach(Anomalies::new(&world.map, 0x8000_0042));
    let inside = node_at(&world, 20.0, 20.0);
    let fields = |hazards: &Hazards| field_source::v1(hazards.anomalies().unwrap(), &world.map);
    // No world has shown the core: no field.
    assert!(fields(&hazards).is_empty());
    // Charging at 40 s, its damage from 60 s (core 1 of the map: the electric pair is core 0).
    let bytes = core_world(hazards.anomalies().unwrap(), 1, Some(0), 3000.0, 60_000);
    let report = hazards.sync_from_world(&bytes, 40_000);
    assert!(report.synced.iter().any(|s| s.core == 1), "{report:?}");
    let charging = fields(&hazards);
    assert_eq!(charging.len(), 1);
    assert_eq!(charging[0].id, FieldId(1));
    assert_eq!(charging[0].kind, FieldKind::Spring, "the zone's damage type");
    assert_eq!(charging[0].phase, FieldPhase::Charging { damage_from: 60_000 });
    assert!(hazards.set_fields(&charging, &world.grid, 40_000));
    assert!(hazards.allowed(inside, 41_000) && !hazards.allowed(inside, 57_000));
    // Run on to the first tick past 60 s: the simulation follows the state chain to the damage state.
    hazards.advance(61_000);
    let damaging = fields(&hazards);
    assert!(
        matches!(damaging[..], [WorldField { phase: FieldPhase::Damaging { until: Some(until) }, .. }] if until >= 63_000),
        "{damaging:?}"
    );
    assert!(hazards.set_fields(&damaging, &world.grid, 61_000));
    assert!(hazards.on_at(inside, 61_000) && !hazards.allowed(inside, 61_000));
    // The artefact waits (a world at 70 s): holding.
    let bytes = core_world(hazards.anomalies().unwrap(), 1, Some(2), 1000.0, 0);
    hazards.sync_from_world(&bytes, 70_000);
    let waiting = fields(&hazards);
    assert_eq!(waiting.iter().map(|f| f.phase).collect::<Vec<_>>(), [FieldPhase::Holding]);
    hazards.set_fields(&waiting, &world.grid, 70_000);
    assert!(!hazards.allowed_for(inside, 1_000_000, 0), "on until a world drops it");
    // Picked up (a world at 80 s, energy 50 on the way to idle): gone at once.
    let bytes = core_world(hazards.anomalies().unwrap(), 1, Some(3), 50.0, 140_000);
    hazards.sync_from_world(&bytes, 80_000);
    assert!(fields(&hazards).is_empty());
    assert!(hazards.set_fields(&fields(&hazards), &world.grid, 80_000));
    assert!(hazards.allowed(inside, 80_000));
}
