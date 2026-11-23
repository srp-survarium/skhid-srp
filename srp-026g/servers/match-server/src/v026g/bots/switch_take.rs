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
use super::zone_search::ByFloor;

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
    let grid = &tactics.world.grid;
    let hazards = &tactics.hazards;
    let spawn = tactics.world.map.spawns.iter().find(|p| p.team == side)?;
    let from = grid.locate(spawn.position, 3.0)?;
    let at = grid.position(stand);
    let inside = |p: Vec3| (p.x - at.x).abs() <= within_m && (p.z - at.z).abs() <= within_m;
    let walkable = |b: NodeId| {
        tactics.spawn_ok(b, Some(side), false)
            && hazards.live_cost_per_s(b, None) == Cost::ZERO
            && hazards.zones_at(b).all(|zone| !zone.switches() || core.has_zone(zone.zone))
    };
    if !walkable(stand) {
        return None;
    }
    let floors = grid.len();
    let mut length: ByFloor<f32> = ByFloor::new(floors);
    let mut prev: ByFloor<NodeId> = ByFloor::new(floors);
    let mut heap = BinaryHeap::new();
    for node in grid.floors_in([at.x - within_m, at.z - within_m, at.x + within_m, at.z + within_m]) {
        if hazards.hazard_free(node)
            && grid.links(node).any(|(b, _)| !hazards.hazard_free(b))
            && tactics.past_fixed_zones(node)
            && tactics.spawn_ok(node, Some(side), false)
            && grid.may_reach(from, node)
        {
            length.set(node, 0.0);
            heap.push((Reverse(0.0f32.to_bits()), node));
        }
    }
    while let Some((Reverse(bits), a)) = heap.pop() {
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
            return Some(way);
        }
        let pa = grid.position(a);
        for (b, kind) in grid.links(a) {
            let pb = grid.position(b);
            if kind != LinkKind::Walk || !inside(pb) || !walkable(b) {
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
    None
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
}
