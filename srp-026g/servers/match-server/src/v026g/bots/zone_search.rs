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

/// The loss a body walking `path` over the grid takes, at [`CROSS_MPS`], in each pool, and the
/// metres it walks.
pub fn walk_cost(tactics: &Tactics, path: &[NodeId], armour: Option<&ZoneArmour>) -> (Cost, f32) {
    let grid = &tactics.world.grid;
    let mut cost = Cost::ZERO;
    let mut metres = 0.0;
    for pair in path.windows(2) {
        let d = (grid.position(pair[1]) - grid.position(pair[0])).length();
        metres += d;
        cost = cost + tactics.hazards.cost_per_s(pair[1], armour) * (d / CROSS_MPS);
    }
    (cost, metres)
}
