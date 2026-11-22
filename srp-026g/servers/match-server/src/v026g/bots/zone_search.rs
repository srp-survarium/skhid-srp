//! The cheapest way over the floors of zones on all match (batteries, the dry-way spawners).
//!
//! A battery whose stand lies in or beyond zones that never switch off is taken by a crossing
//! (`plays::collect`): a walk in a line from a dry floor. Where no line is clear (a wall or a
//! drop between the dry floor and the stand) the motor's crossing search (`crossing.rs`) finds
//! a way over the zones' floors, but by length: it takes the shortest, and the shortest is not
//! the one that hurts least (on the bench's maps the length-first ways cost 0.1-5 times a
//! body's health, most over its budget). This is the same search with the damage as its price:
//! a Dijkstra over the walk grid from one floor, each step costing the pool loss of the floor
//! stepped onto (`Hazards::cost_per_s`, at [`CROSS_MPS`]) first and the metres second, so the
//! way found is the cheapest in damage and the shortest of those.
//!
//! One search from a dry floor answers every stand of the map at once ([`Cheapest::path_to`]);
//! the caller keeps it for the match (`plays::memo`).
//!
//! Best practice: hazard-weighted shortest path with a hard budget checked on the result
//! (Detour's per-area query-filter costs; a constrained shortest path by relaxing the
//! constraint, then checking it). The budget is the caller's (`Crossing::fits`).

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use mapdata::NodeId;

use super::plays::collect::CROSS_MPS;
use super::tactics::Tactics;
use super::zone_cost::{Cost, ZoneArmour};

/// A pool's loss counts this many metres in the search's price: any damage outweighs any detour
/// the grid has (a map is a few hundred metres across).
const DAMAGE_M: f32 = 1_000.0;

/// The cheapest ways to every floor of the grid from one floor.
pub struct Cheapest {
    /// The price of the way to each floor ([`f32::INFINITY`]: none found).
    price: Vec<f32>,
    /// The floor before it on the way.
    prev: Vec<Option<NodeId>>,
}

impl Cheapest {
    /// The ways from floor `from` for a body of map side `side` over the floors no zone marks and
    /// the floors only zones on all match mark (not another side's spawn area), priced under
    /// `armour`. A zone that switches or a live field shuts its floors.
    pub fn from(
        tactics: &Tactics,
        from: NodeId,
        side: Option<i32>,
        armour: Option<&ZoneArmour>,
    ) -> Self {
        let grid = &tactics.world.grid;
        let hazards = &tactics.hazards;
        let n = grid.len();
        let mut price = vec![f32::INFINITY; n];
        let mut prev = vec![None; n];
        let mut heap = BinaryHeap::new();
        price[from as usize] = 0.0;
        heap.push((Reverse(0.0f32.to_bits()), from));
        while let Some((Reverse(bits), a)) = heap.pop() {
            if f32::from_bits(bits) > price[a as usize] {
                continue;
            }
            let at = grid.position(a);
            for (b, kind) in grid.links(a) {
                if !(hazards.hazard_free(b) || hazards.only_fixed_at(b))
                    || !tactics.spawn_ok(b, side, false)
                {
                    continue;
                }
                let walk = grid.link_cost(a, b, kind);
                if !walk.is_finite() {
                    continue;
                }
                let metres = (grid.position(b) - at).length();
                let damage = hazards.cost_per_s(b, armour).most() * metres / CROSS_MPS;
                let through = f32::from_bits(bits) + walk.max(metres) + damage * DAMAGE_M;
                if through < price[b as usize] {
                    price[b as usize] = through;
                    prev[b as usize] = Some(a);
                    heap.push((Reverse(through.to_bits()), b));
                }
            }
        }
        Self { price, prev }
    }

    /// The floors of the cheapest way to `to`, from the start, or none if it was not reached.
    pub fn path_to(&self, to: NodeId) -> Option<Vec<NodeId>> {
        if !self.price.get(to as usize)?.is_finite() {
            return None;
        }
        let mut path = vec![to];
        let mut at = to;
        while let Some(before) = self.prev[at as usize] {
            path.push(before);
            at = before;
        }
        path.reverse();
        Some(path)
    }
}

/// [cheapest entry] The cheapest way into the zones round floor `goal` (a battery's stand): from any dry floor within
/// `within_m` across of it that the side reaches (no zone marks it, a way leads to it past the zones on all match, not
/// another side's spawn area, in `spawn`'s piece of the grid), over the floors [`Cheapest`] walks, priced as it prices
/// them (any damage outweighs any detour). The floors of the way, from that dry floor (the entry) to `goal`; none when
/// no such floor leads there.
///
/// One search, bounded by the box: the sources are all those dry floors next to a marked one at price 0, so the first way
/// that reaches `goal` is the cheapest of every entry, whatever side of the zone it is on. Before, a battery's crossing was the route from
/// the team's spawn, entered where that route first met the zone (Chemical Plant's battery 0: 12.1 m inside for one
/// team against a 4.7 m entry both teams reach; Vector Laboratory's battery 2: 21.6 m against 4.8 m).
pub fn cheapest_entry(
    tactics: &Tactics,
    spawn: NodeId,
    goal: NodeId,
    side: Option<i32>,
    armour: Option<&ZoneArmour>,
    within_m: f32,
) -> Option<Vec<NodeId>> {
    use std::collections::HashMap;
    let grid = &tactics.world.grid;
    let hazards = &tactics.hazards;
    let at = grid.position(goal);
    let area = [at.x - within_m, at.z - within_m, at.x + within_m, at.z + within_m];
    let inside = |p: mapdata::Vec3| (p.x - at.x).abs() <= within_m && (p.z - at.z).abs() <= within_m;
    let walkable = |b: NodeId| (hazards.hazard_free(b) || hazards.only_fixed_at(b)) && tactics.spawn_ok(b, side, false);
    let mut price: HashMap<NodeId, f32> = HashMap::new();
    let mut prev: HashMap<NodeId, NodeId> = HashMap::new();
    let mut heap = BinaryHeap::new();
    for node in grid.floors_in(area) {
        // Only a dry floor by a marked one can be an entry: the others' dry neighbours are sources at the same price.
        if hazards.hazard_free(node)
            && grid.links(node).any(|(b, _)| !hazards.hazard_free(b))
            && tactics.past_fixed_zones(node)
            && tactics.spawn_ok(node, side, false)
            && grid.may_reach(spawn, node)
        {
            price.insert(node, 0.0);
            heap.push((Reverse(0.0f32.to_bits()), node));
        }
    }
    while let Some((Reverse(bits), a)) = heap.pop() {
        let here = f32::from_bits(bits);
        if price.get(&a).is_some_and(|&p| here > p) {
            continue;
        }
        if a == goal {
            let mut path = vec![goal];
            let mut node = goal;
            while let Some(&before) = prev.get(&node) {
                path.push(before);
                node = before;
            }
            path.reverse();
            return Some(path);
        }
        let from = grid.position(a);
        for (b, kind) in grid.links(a) {
            let to = grid.position(b);
            if !inside(to) || !walkable(b) {
                continue;
            }
            let walk = grid.link_cost(a, b, kind);
            if !walk.is_finite() {
                continue;
            }
            let metres = (to - from).length();
            let damage = hazards.cost_per_s(b, armour).most() * metres / CROSS_MPS;
            let through = here + walk.max(metres) + damage * DAMAGE_M;
            if price.get(&b).is_none_or(|&p| through < p) {
                price.insert(b, through);
                prev.insert(b, a);
                heap.push((Reverse(through.to_bits()), b));
            }
        }
    }
    None
}

/// The loss a body walking `path` over the grid takes, at [`CROSS_MPS`], in each pool, and the
/// metres it walks.
pub fn walk_cost(tactics: &Tactics, path: &[NodeId], armour: Option<&ZoneArmour>) -> (Cost, f32) {
    let grid = &tactics.world.grid;
    // [Gap 12] By the client's rule of when a zone hits (`zone_cost::WalkPrice`): an interval zone's whole hits.
    let mut walk = super::zone_cost::WalkPrice::default();
    let mut metres = 0.0;
    for pair in path.windows(2) {
        let d = (grid.position(pair[1]) - grid.position(pair[0])).length();
        metres += d;
        tactics.hazards.walk_step(&mut walk, pair[1], armour, d / CROSS_MPS, |_| 1.0);
    }
    (walk.total(), metres)
}
