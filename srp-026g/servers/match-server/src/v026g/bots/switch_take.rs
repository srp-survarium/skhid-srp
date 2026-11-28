//! [anomaly batteries] A battery inside the zones of an energy core (level_02's fruit cloud, a zone that switches with the
//! core's energy): the forecast of a take's walk in, its stand and its walk out, on the client's own rules.
//!
//! The core's energy ([`EnergyCore`]) falls `energy_decrease_speed` a second (each 50 ms tick its share,
//! `generic_anomaly_core::tick` 0x31B5B0) and rises by the body's move while it moves in the core's zones ([`Rise`]:
//! `base_player::notify_actions_subscribers` 0x2D95B0, `generic_anomaly_core::on_player_action` 0x33BB50); a body that
//! stands adds nothing. The energy picks the state (`select_state` 0x31BA90), the state switches its zones on. The forecast
//! steps the take's plan on the core's 50 ms grid from the energy now: each step the body's floor on the plan, the energy
//! after its move, the zones of that floor the energy has on (and the other zones there by their own activity), priced
//! as the crossings price a walk ([`WalkPrice`]: the zone's hit at the floor's intensity where the rule asks, through the
//! bot's armour into the pools). Only this body's move is counted: another body in the cloud raises the energy too, which
//! the take watches as it goes (the event line names what the forecast counted).
//!
//! The other bodies known to move in the core's zones (inside, or bound for its batteries) add their rise every step, each at
//! the core's sprint rise (the most a body adds; their gait is not known), and a take may add the core's
//! `energy_af_container_use` where it is made ([`Part::Bump`]: `generic_anomaly_core::on_artefact_container_use` 0x33BCD0 adds
//! it on an artefact container's use; whether a battery's take is one is not proven, so the forecast counts it).

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use mapdata::{LinkKind, NodeId, Vec3, WalkGrid};

use super::anomalies::{EnergyCore, Rise};
use super::hazards::{Hazard, Hazards, HurtTest};
use super::tactics::Tactics;
use super::zone_cost::{Cost, WalkPrice, ZoneArmour};
use super::zone_search::{ByFloor, PopBudget};

/// The body's forward speed on a walk of the plan, m/s: the run's 3.5 forward (`kin::Kinematics::walk`, PROVEN F5).
pub const RUN_MPS: f32 = 3.5;
/// A sprint's, m/s (`kin::Kinematics::sprint`, PROVEN F5 on 10,909 of 11,214 traced steps).
pub const SPRINT_MPS: f32 = 4.9587;
/// A carrier's share of the run (`kin::CarrierSprint`: it walks at the run's 3.5 x F x 0.95, the battery's own
/// `can_sprint` refusing the sprint, PROVEN).
pub const CARRIER_SHARE: f32 = 0.95;

/// How far across from a battery's stand its way in may run, m: a box this far each way (the Chemical Plant's fruit
/// stands lie 10-30 m inside the cloud's edge).
pub const WAY_WITHIN_M: f32 = 45.0;

/// The forecast's step, s: the anomaly tick (`anomalies::TICK_MS`).
pub const STEP_S: f32 = super::anomalies::TICK_MS as f32 / 1000.0;

/// How the body moves on a walk of the plan: which of the core's rises it adds ([`Rise`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pace {
    Walk,
    Run,
    Sprint,
}

impl Pace {
    fn rise(self, rise: Rise) -> f32 {
        match self {
            Self::Walk => rise.walk,
            Self::Run => rise.run,
            Self::Sprint => rise.sprint,
        }
    }
}

/// One part of a take's plan.
#[derive(Clone, Copy, Debug)]
pub enum Part<'a> {
    /// Walking the floors `nodes` in order at `mps`, moving at `pace`.
    Walk { nodes: &'a [NodeId], mps: f32, pace: Pace },
    /// Standing on `node` for `seconds` (the take).
    Stand { node: NodeId, seconds: f32 },
    /// The core's energy raised by `energy` at once (a take that may be an artefact container's use).
    Bump { energy: f32 },
}

/// What the forecast found.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Forecast {
    /// The plan's time moving and standing, s.
    pub moving_s: f32,
    pub standing_s: f32,
    /// The core's energy at the start, its highest on the plan and at the end.
    pub energy_start: f32,
    pub energy_peak: f32,
    pub energy_end: f32,
    /// The highest state the energy picks on the plan (the worlds' numbering: highest threshold first, so the least
    /// number), and its threshold.
    pub peak_state: Option<usize>,
    pub peak_threshold: Option<f32>,
    /// Whether a zone of the core that switches ([`Hazard::switches`]: its threshold out of reach of a body's own rise in
    /// [`super::hazards::ENERGY_REACH_S`]) is on on a floor of the plan while the body is on that floor, and for how long, s:
    /// rule (a) asks for none. The core's low states' zones, on whenever a body moves there, are priced all the same.
    pub switch_on: bool,
    pub on_s: f32,
    /// The plan's loss in the pools.
    pub cost: Cost,
}

/// The forecast of `parts` from the core's energy now (`core`), at `time` (match ms; the other zones' activity), under
/// `armour`, each zone at the intensity of the floor's place in it where `rule` asks.
/// `others_per_s`: what the other bodies known to move in the core's zones add a second, all the plan long.
#[allow(clippy::too_many_arguments)]
pub fn forecast(
    hazards: &Hazards,
    grid: &WalkGrid,
    core: EnergyCore,
    parts: &[Part],
    time: u32,
    others_per_s: f32,
    armour: Option<&ZoneArmour>,
    rule: HurtTest,
) -> Forecast {
    let mut energy = core.energy();
    let (rise, fall, maximum) = (core.rise(), core.decrease_per_s(), core.maximum());
    let clamp = |e: f32| if maximum > 0.0 { e.min(maximum) } else { e };
    let mut out = Forecast {
        moving_s: 0.0,
        standing_s: 0.0,
        energy_start: energy,
        energy_peak: energy,
        energy_end: energy,
        peak_state: core.state_at(energy),
        peak_threshold: core.state_at(energy).and_then(|s| core.threshold(s)),
        switch_on: false,
        on_s: 0.0,
        cost: Cost::ZERO,
    };
    let mut walk = WalkPrice::default();
    let mut clock_s = 0.0f32;
    let mut step = |node: NodeId, moving: Option<Pace>, out: &mut Forecast, energy: &mut f32, clock_s: &mut f32| {
        if let Some(pace) = moving {
            *energy += pace.rise(rise) * STEP_S;
        }
        *energy = clamp(*energy + others_per_s * STEP_S);
        let at_ms = time.saturating_add((*clock_s * 1000.0) as u32);
        let mut core_on = false;
        let on: Vec<(&Hazard, f32)> = hazards
            .zones_at(node)
            .filter(|zone| {
                if core.has_zone(zone.zone) {
                    let on = core.zone_on_at(zone.zone, *energy);
                    core_on |= on && zone.switches();
                    on
                } else {
                    zone.activity.active(at_ms)
                }
            })
            .map(|zone| (zone, if rule.by_intensity { zone.intensity_at(grid.position(node)) } else { 1.0 }))
            .collect();
        walk.step(on.into_iter(), armour, STEP_S, hazards.live_cost_per_s(node, armour));
        if core_on {
            out.switch_on = true;
            out.on_s += STEP_S;
        }
        if *energy > out.energy_peak {
            out.energy_peak = *energy;
            out.peak_state = core.state_at(*energy);
            out.peak_threshold = out.peak_state.and_then(|s| core.threshold(s));
        }
        *energy = (*energy - fall * STEP_S).max(0.0);
        *clock_s += STEP_S;
    };
    for part in parts {
        match *part {
            Part::Walk { nodes, mps, pace } => {
                if nodes.is_empty() || mps <= 0.0 {
                    continue;
                }
                // Metres along the walk to each floor of it.
                let mut along = Vec::with_capacity(nodes.len());
                let mut d = 0.0f32;
                for (i, &node) in nodes.iter().enumerate() {
                    if i > 0 {
                        d += (grid.position(node) - grid.position(nodes[i - 1])).length();
                    }
                    along.push(d);
                }
                let steps = (d / mps / STEP_S).ceil().max(1.0) as usize;
                for k in 0..steps {
                    let at_m = (k as f32 + 1.0) * STEP_S * mps;
                    let i = along.partition_point(|&a| a < at_m).min(nodes.len() - 1);
                    step(nodes[i], Some(pace), &mut out, &mut energy, &mut clock_s);
                }
                out.moving_s += steps as f32 * STEP_S;
            }
            Part::Bump { energy: bump } => energy = clamp(energy + bump),
            Part::Stand { node, seconds } => {
                let steps = (seconds / STEP_S).ceil().max(0.0) as usize;
                for _ in 0..steps {
                    step(node, None, &mut out, &mut energy, &mut clock_s);
                }
                out.standing_s += steps as f32 * STEP_S;
            }
        }
    }
    out.energy_end = energy;
    out.cost = walk.total();
    out
}

/// [anomaly batteries] The way into a battery's stand floor `stand` inside `core`'s zones for a body of map side `side`, the
/// floors in order from its entry: over walk links only (stairs and ramps; no jump or drop, which a take's straight walk
/// does not make), on floors where only zones on all match or `core`'s zones lie (no other switching zone, no live field,
/// not another side's spawn area), from a dry floor next to a marked one within `within_m` across that the side reaches (no
/// zone marks it, a way leads to it past the zones on all match, in the piece of the grid of the side's spawn). The
/// shortest such way: the least time inside.
pub fn way_in(tactics: &Tactics, core: EnergyCore, side: i32, stand: NodeId, within_m: f32) -> Option<Vec<NodeId>> {
    match WayInSearch::begin(tactics, core, side, stand, within_m).run(tactics, core, &mut PopBudget::Unbounded) {
        WayStep::Done(way) => way,
        WayStep::Pending => unreachable!("an unbounded search ends"),
    }
}

/// [core ways] The box [`way_in`]'s search keeps to round `stand` (min x, min z, max x, max z): every floor it reads is in it.
pub fn way_box(grid: &WalkGrid, stand: NodeId, within_m: f32) -> [f32; 4] {
    let at = grid.position(stand);
    [at.x - within_m, at.z - within_m, at.x + within_m, at.z + within_m]
}

/// [core ways] The energy core whose zone that switches marks the floor `stand`, if one does (`zone_take`'s test, on the tactics).
pub fn core_of(tactics: &Tactics, stand: NodeId) -> Option<EnergyCore<'_>> {
    let hazards = &tactics.hazards;
    let anomalies = hazards.anomalies()?;
    hazards.zones_at(stand).filter(|zone| zone.switches()).find_map(|zone| anomalies.energy_core(zone.zone))
}

/// [core ways] Where a [`WayInSearch`] stands after a run: [`way_in`]'s answer, or not yet (its pops ran out first).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WayStep {
    Done(Option<Vec<NodeId>>),
    Pending,
}

/// [core ways] [`way_in`]'s search kept between runs: [`Self::begin`] lays its sources, each [`Self::run`] pops floors until the
/// stand, the heap's end or the pops' end. The pops are the whole search's in its order, whatever the slices, so a search run in
/// slices finds the way one run at once finds, floor for floor, while the board stays what it was in its box.
pub struct WayInSearch {
    state: WayState,
}

enum WayState {
    /// No way: the side has no spawn floor, or the stand is not a floor the way walks onto.
    NoWay,
    Running(Box<WayRunning>),
}

struct WayRunning {
    side: i32,
    stand: NodeId,
    at: Vec3,
    within_m: f32,
    length: ByFloor<f32>,
    prev: ByFloor<NodeId>,
    heap: BinaryHeap<(Reverse<u32>, NodeId)>,
}

/// The floors [`way_in`]'s way steps onto for side `side` into `core`'s zones: not another side's spawn area, no live field's
/// hurt, and no zone that switches but `core`'s.
fn way_walkable(tactics: &Tactics, core: EnergyCore, side: i32, b: NodeId) -> bool {
    let hazards = &tactics.hazards;
    tactics.spawn_ok(b, Some(side), false)
        && hazards.live_cost_per_s(b, None) == Cost::ZERO
        && hazards.zones_at(b).all(|zone| !zone.switches() || core.has_zone(zone.zone))
}

impl WayInSearch {
    /// [`way_in`]'s search for side `side` into `core`'s zones to `stand` within `within_m` across: no way at once where the side
    /// has no spawn floor or the stand is not walked onto, else its sources laid (the box's dry floors next to a marked one that
    /// the side reaches, at length 0).
    pub fn begin(tactics: &Tactics, core: EnergyCore, side: i32, stand: NodeId, within_m: f32) -> Self {
        let grid = &tactics.world.grid;
        let hazards = &tactics.hazards;
        let Some(from) = tactics.world.map.spawns.iter().find(|p| p.team == side).and_then(|spawn| grid.locate(spawn.position, 3.0)) else {
            return Self { state: WayState::NoWay };
        };
        if !way_walkable(tactics, core, side, stand) {
            return Self { state: WayState::NoWay };
        }
        let floors = grid.len();
        let mut running = WayRunning {
            side,
            stand,
            at: grid.position(stand),
            within_m,
            length: ByFloor::new(floors),
            prev: ByFloor::new(floors),
            heap: BinaryHeap::new(),
        };
        for node in grid.floors_in(way_box(grid, stand, within_m)) {
            if hazards.hazard_free(node)
                && grid.links(node).any(|(b, _)| !hazards.hazard_free(b))
                && tactics.past_fixed_zones(node)
                && tactics.spawn_ok(node, Some(side), false)
                && grid.may_reach(from, node)
            {
                running.length.set(node, 0.0);
                running.heap.push((Reverse(0.0f32.to_bits()), node));
            }
        }
        Self { state: WayState::Running(Box::new(running)) }
    }

    /// Pops floors while `pops` lasts (`core`: the search's own): the way, none, or not yet.
    pub fn run(&mut self, tactics: &Tactics, core: EnergyCore, pops: &mut PopBudget) -> WayStep {
        let WayState::Running(running) = &mut self.state else {
            return WayStep::Done(None);
        };
        let WayRunning { side, stand, at, within_m, length, prev, heap } = &mut **running;
        let (side, stand, at, within_m) = (*side, *stand, *at, *within_m);
        let grid = &tactics.world.grid;
        let inside = |p: Vec3| (p.x - at.x).abs() <= within_m && (p.z - at.z).abs() <= within_m;
        loop {
            if heap.is_empty() {
                return WayStep::Done(None);
            }
            if !pops.take() {
                return WayStep::Pending;
            }
            let Some((Reverse(bits), a)) = heap.pop() else {
                return WayStep::Done(None);
            };
            let here = f32::from_bits(bits);
            if length.get(a).is_some_and(|l| here > l) {
                continue;
            }
            if a == stand {
                let mut way = vec![stand];
                let mut node = stand;
                while let Some(before) = prev.get(node) {
                    way.push(before);
                    node = before;
                }
                way.reverse();
                return WayStep::Done(Some(way));
            }
            let pa = grid.position(a);
            for (b, kind) in grid.links(a) {
                let pb = grid.position(b);
                if kind != LinkKind::Walk || !inside(pb) || !way_walkable(tactics, core, side, b) {
                    continue;
                }
                let through = here + (pb - pa).length();
                if length.get(b).is_none_or(|l| through < l) {
                    length.set(b, through);
                    prev.set(b, a);
                    heap.push((Reverse(through.to_bits()), b));
                }
            }
        }
    }
}

/// The length of `way` over the grid, m.
pub fn way_m(grid: &WalkGrid, way: &[NodeId]) -> f32 {
    way.windows(2).map(|w| (grid.position(w[1]) - grid.position(w[0])).length()).sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v026g::bots::anomalies::Anomalies;
    use crate::v026g::bots::anomalies::tests::{cloud, cylinder, zone};
    use crate::v026g::bots::tactics::tests::field_with;

    /// The field with level_02's cloud at (20, 30): "low" (energy 0 and up, 1 m), "lvl1" (20 and up, 5 m), "lvl3" (1000 and
    /// up, 8 m: the zone that switches), at `energy`.
    fn clouded(energy: f32) -> Tactics {
        let world = field_with(|map| {
            map.zones.push(zone("low", cylinder(20.0, 30.0, 1.0), 0.07));
            map.zones.push(zone("lvl1", cylinder(20.0, 30.0, 5.0), 0.07));
            map.zones.push(zone("lvl3", cylinder(20.0, 30.0, 8.0), 0.1));
            map.anomalies.push(cloud());
        });
        let mut tactics = Tactics::new(world.clone(), 1.0);
        let mut anomalies = Anomalies::new(&world.map, 0x8000_0001);
        anomalies.advance(1_000);
        anomalies.set_energy(0, energy);
        tactics.hazards.attach(anomalies);
        tactics
    }

    fn way_and_core(tactics: &Tactics) -> (Vec<NodeId>, EnergyCore<'_>) {
        let grid = &tactics.world.grid;
        let stand = grid.locate(Vec3::new(20.0, 0.0, 30.0), 1.0).unwrap();
        let switching = tactics.hazards.zones_at(stand).find(|z| z.switches()).expect("lvl3 switches");
        let core = tactics.hazards.anomalies().unwrap().energy_core(switching.zone).unwrap();
        (way_in(tactics, core, 0, stand, WAY_WITHIN_M).expect("a way in"), core)
    }

    /// The way in starts on a dry floor next to the zones, walks only walk links, and ends at the stand.
    #[test]
    fn the_way_in_walks_from_a_dry_edge_to_the_stand() {
        let tactics = clouded(0.0);
        let grid = &tactics.world.grid;
        let (way, _) = way_and_core(&tactics);
        assert!(tactics.hazards.hazard_free(way[0]), "starts dry");
        assert!(!tactics.hazards.hazard_free(way[1]), "next floor marked");
        assert!(grid.position(*way.last().unwrap()).distance_xz(Vec3::new(20.0, 0.0, 30.0)) < 0.5, "ends at the stand");
        for w in way.windows(2) {
            assert!(grid.links(w[0]).any(|(b, kind)| b == w[1] && kind == LinkKind::Walk));
        }
        let m = way_m(grid, &way);
        assert!((7.5..10.0).contains(&m), "about the cloud's 8 m radius: {m}");
    }

    /// From energy 0 the run in, the stand and the walk out never reach lvl3's 1000: rule (a)'s plan, priced by the low
    /// states' zones. Near 1000 the same plan switches lvl3 on. Standing lets the energy fall; the container's use adds its value.
    #[test]
    fn the_forecast_follows_the_energy_of_the_body_and_the_take() {
        let rule = HurtTest::default();
        for (energy, on) in [(0.0, false), (990.0, true)] {
            let tactics = clouded(energy);
            let (way, core) = way_and_core(&tactics);
            let back: Vec<NodeId> = way.iter().rev().copied().collect();
            let stand = *way.last().unwrap();
            let parts = [
                Part::Walk { nodes: &way, mps: RUN_MPS, pace: Pace::Run },
                Part::Stand { node: stand, seconds: 4.0 },
                Part::Walk { nodes: &back, mps: RUN_MPS * CARRIER_SHARE, pace: Pace::Run },
            ];
            let f = forecast(&tactics.hazards, &tactics.world.grid, core, &parts, 1_000, 0.0, None, rule);
            assert_eq!(f.switch_on, on, "energy {energy}: {f:?}");
            assert!(f.cost.most() > 0.0, "the low states' zones hurt: {f:?}");
            // In at 45 - 25 = 20 a second.
            let rise = (f.energy_peak - energy) / (way_m(&tactics.world.grid, &way) / RUN_MPS);
            assert!((rise - 20.0).abs() < 2.0, "rise {rise}");
            let stood = forecast(&tactics.hazards, &tactics.world.grid, core, &[Part::Stand { node: stand, seconds: 2.0 }], 1_000, 0.0, None, rule);
            assert!((stood.energy_end - (energy - 50.0).max(0.0)).abs() < 0.01, "{stood:?}");
            let bumped = forecast(
                &tactics.hazards,
                &tactics.world.grid,
                core,
                &[Part::Bump { energy: 100.0 }, Part::Stand { node: stand, seconds: 1.0 }],
                1_000,
                0.0,
                None,
                rule,
            );
            assert!((bumped.energy_end - (energy + 75.0)).abs() < 0.01, "{bumped:?}");
            // Others moving in raise it as much again a second.
            let crowded = forecast(&tactics.hazards, &tactics.world.grid, core, &[Part::Stand { node: stand, seconds: 1.0 }], 1_000, 70.0, None, rule);
            assert!((crowded.energy_end - (energy + 45.0)).abs() < 0.01, "{crowded:?}");
        }
    }

    /// [core ways] `clouded`'s field with a battery spawner at the cloud's centre.
    fn clouded_spawner(energy: f32) -> Tactics {
        let world = field_with(|map| {
            map.zones.push(zone("low", cylinder(20.0, 30.0, 1.0), 0.07));
            map.zones.push(zone("lvl1", cylinder(20.0, 30.0, 5.0), 0.07));
            map.zones.push(zone("lvl3", cylinder(20.0, 30.0, 8.0), 0.1));
            map.anomalies.push(cloud());
            let at = Vec3::new(20.0, 0.3, 30.0);
            map.battery_spawners = vec![mapdata::BatterySpawner { position: at, item: at, layer: String::new() }];
        });
        let mut tactics = Tactics::new(world.clone(), 1.0);
        let mut anomalies = Anomalies::new(&world.map, 0x8000_0001);
        anomalies.advance(1_000);
        anomalies.set_energy(0, energy);
        tactics.hazards.attach(anomalies);
        tactics
    }

    /// [core ways] A live field: a spring of `radius` at `at`, damaging.
    fn spring(id: u32, at: Vec3, radius: f32) -> crate::v026g::bots::live_zones::WorldField {
        use crate::v026g::bots::live_zones::{FieldId, FieldKind, FieldPhase, WorldField};
        use mapdata::{Rotation, Shape, ShapeKind};
        WorldField {
            id: FieldId(id),
            kind: FieldKind::Spring,
            shape: Shape { kind: ShapeKind::Cylinder, center: Vec3::new(at.x, 1.0, at.z), rotation: Rotation::IDENTITY, half: Vec3::new(radius, 2.0, radius) },
            phase: FieldPhase::Damaging { until: None },
            hit: crate::v026g::bots::tests::standin_fields::standin_hit(),
        }
    }

    /// [core ways] The ways made at map load (`Memo::prewarm_zone_ways`: the core spawner's stand, both sides) are the first
    /// asks' with no live field in the box; a live field out of the box keeps them; a field in the box drops the way, and the
    /// ask gets the search's way under the board now.
    #[test]
    fn a_way_made_at_load_is_the_first_asks_and_goes_with_the_fields_of_its_box() {
        use crate::v026g::bots::plays::memo::{Memo, ZoneWayAsk};
        let mut tactics = clouded_spawner(0.0);
        let stand = tactics.stand_to_take(Vec3::new(20.0, 0.3, 30.0)).and_then(|s| tactics.world.grid.locate(s, 3.0)).expect("a stand");
        let core_way = |tactics: &Tactics, side: i32| way_in(tactics, core_of(tactics, stand).expect("the core"), side, stand, WAY_WITHIN_M);
        let loaded = Memo::default();
        let (asked, _) = loaded.prewarm_zone_ways(&tactics);
        assert_eq!(asked, 2, "one stand, two sides");
        let ask = |memo: &Memo, tactics: &Tactics, side: i32| memo.zone_way_ask(tactics, core_of(tactics, stand).expect("the core"), side, stand);
        let mut kept = Vec::new();
        for side in [0, 1] {
            let first = ask(&Memo::default(), &tactics, side);
            let made = ask(&loaded, &tactics, side);
            assert_eq!(made, first, "side {side}: the load's way is the first ask's");
            assert_eq!(made, ZoneWayAsk::Way(core_way(&tactics, side).map(std::sync::Arc::new)), "side {side}");
            let ZoneWayAsk::Way(way) = made else { panic!("an answer for side {side}") };
            kept.push(way);
        }
        assert!(kept[0].is_some(), "side 0 has a way in on the field");
        assert_eq!(loaded.zone_way_counts().prewarmed, 2);
        // A field on the grid (80 x 40 m) out of the box (45 m round the stand: x to 65 m): the ways stay, the same ones.
        let world = tactics.world.clone();
        let grid = &world.grid;
        tactics.hazards.set_fields(&[spring(1, Vec3::new(75.0, 0.0, 20.0), 2.0)], grid, 2_000);
        let in_box = |tactics: &Tactics| tactics.hazards.live.fields_in(&tactics.world.grid, way_box(&tactics.world.grid, stand, WAY_WITHIN_M)).len();
        assert!(tactics.hazards.live.floors_covered() > 0, "the field lies on the grid");
        assert_eq!(in_box(&tactics), 0, "and out of the box");
        for side in [0, 1] {
            let ZoneWayAsk::Way(way) = ask(&loaded, &tactics, side) else { panic!("kept") };
            let same = match (&way, &kept[side as usize]) {
                (Some(a), Some(b)) => std::sync::Arc::ptr_eq(a, b),
                (None, None) => true,
                _ => false,
            };
            assert!(same, "side {side}: kept across a field out of its box");
        }
        assert_eq!(loaded.zone_way_counts().dropped, 0);
        // A field in the box: dropped, and worked out again under the board now.
        tactics.hazards.set_fields(&[spring(1, Vec3::new(75.0, 0.0, 20.0), 2.0), spring(2, Vec3::new(20.0, 0.0, 36.0), 1.5)], grid, 3_000);
        assert_eq!(in_box(&tactics), 1, "the second field lies in the box");
        for side in [0, 1] {
            let now = ask(&loaded, &tactics, side);
            assert_eq!(now, ZoneWayAsk::Way(core_way(&tactics, side).map(std::sync::Arc::new)), "side {side}: the search's way now");
        }
        assert_eq!(loaded.zone_way_counts().dropped, 2, "both sides' ways dropped");
    }

    /// [sliced load] The map's load in polls (`plays::memo::MapLoad`, a few floors a poll) keeps the ways in the load made at
    /// once kept, whole (each with its box's fields and the board's version), though a live field comes into the box between
    /// two of its polls: its searches read the hazards of its beginning. Through the bots, the map is theirs once it ends.
    #[test]
    fn a_load_in_polls_keeps_the_ways_in_the_load_made_at_once() {
        use crate::v026g::bots::plays::memo::{LoadBudget, Memo};
        let mut tactics = clouded_spawner(0.0);
        let budget = LoadBudget { way_pops: 7, stand_floors: 3, entry_pops: 7, straight_checks: 1 };
        let world = tactics.world.clone();
        let counts = Memo::assert_load_is_the_prewarm(&mut tactics, &[], budget, |tactics, poll| {
            if poll == 1 {
                tactics.hazards.set_fields(&[spring(2, Vec3::new(20.0, 0.0, 36.0), 1.5)], &world.grid, 2_000);
            }
        });
        assert_eq!(counts.core_ways, 2, "{counts:?}");
        assert!(counts.polls > 3, "{counts:?}");
        assert_eq!(tactics.hazards.live.fields().len(), 1, "the match's own board kept its field");
    }

    /// [core ways] A way-in search in slices (`PopBudget::Left`) finds the way the whole search finds, floor for floor, whatever the
    /// pops; through the memo with a poll's budget the ask is not yet answered, the polls run it on, and the answer is the whole
    /// search's.
    #[test]
    fn a_way_in_in_slices_is_the_whole_search() {
        use crate::v026g::bots::plays::memo::{Memo, ZoneWayAsk};
        let tactics = clouded_spawner(0.0);
        let stand = tactics.stand_to_take(Vec3::new(20.0, 0.3, 30.0)).and_then(|s| tactics.world.grid.locate(s, 3.0)).expect("a stand");
        let core = core_of(&tactics, stand).expect("the core");
        let whole = way_in(&tactics, core, 0, stand, WAY_WITHIN_M);
        assert!(whole.as_ref().is_some_and(|w| w.len() > 2), "{whole:?}");
        for pops in [1u64, 7, 100, 1_000] {
            let mut search = WayInSearch::begin(&tactics, core, 0, stand, WAY_WITHIN_M);
            let mut runs = 0;
            let sliced = loop {
                runs += 1;
                match search.run(&tactics, core, &mut PopBudget::Left(pops)) {
                    WayStep::Done(way) => break way,
                    WayStep::Pending => assert!(runs < 10_000_000, "it ends"),
                }
            };
            assert_eq!(sliced, whole, "{pops} pops a run");
            if pops < 100 {
                assert!(runs > 1, "{pops} pops a run: {runs} runs");
            }
        }
        let memo = Memo::default();
        memo.new_poll_ways(Some(&tactics), 7);
        assert_eq!(memo.zone_way_ask(&tactics, core, 0, stand), ZoneWayAsk::Pending, "not yet");
        let mut polls = 0;
        while memo.zone_way_counts().runs == 0 {
            memo.new_poll_ways(Some(&tactics), 7);
            polls += 1;
            assert!(polls < 10_000_000, "it ends");
        }
        assert_eq!(memo.zone_way_ask(&tactics, core, 0, stand), ZoneWayAsk::Way(whole.map(std::sync::Arc::new)), "the whole search's");
        let counts = memo.zone_way_counts();
        assert_eq!((counts.searches, counts.sliced, counts.waited_polls), (1, 1, polls), "{counts:?}");
        assert!(counts.runs > 1, "{counts:?}");
    }
}
