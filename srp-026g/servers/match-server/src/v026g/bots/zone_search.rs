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
/// another side's spawn area, in `spawn`'s piece of the grid), over the floors [`Cheapest`] walks, each floor stepped onto
/// priced at `damage_per_s` (the caller's price of the walk, so the way found is the one its price is least on; any
/// damage outweighs any detour, as [`Cheapest`]). The floors of the way, from that dry floor (the entry) to `goal`; none
/// when no such floor leads there.
///
/// One search, bounded by the box: the sources are all those dry floors next to a marked one at price 0, so the first way
/// that reaches `goal` is the cheapest of every entry, whatever side of the zone it is on. Before, a battery's crossing was the route from
/// the team's spawn, entered where that route first met the zone (Chemical Plant's battery 0: 12.1 m inside for one
/// team against a 4.7 m entry both teams reach; Vector Laboratory's battery 2: 21.6 m against 4.8 m).
///
/// [CPU] A goal floor the search never walks onto is answered at once: the search reaches `goal` only as a source (a dry
/// floor not of another side's spawn area) or by a step onto it, and a step goes only onto a floor [`entry_walkable`] lets
/// through, which every source is. So a goal it does not let through (a zone that switches, another side's spawn area) has
/// no way, and the search over the whole box only proved it: on Chemical Plant's batteries 44 of a match's 52 searches,
/// 110-160 ms each (the gate bench of 2026-11-23, batt m2 s2), in one commander pass.
pub fn cheapest_entry(
    tactics: &Tactics,
    spawn: NodeId,
    goal: NodeId,
    side: Option<i32>,
    damage_per_s: impl Fn(NodeId) -> f32,
    within_m: f32,
) -> Option<Vec<NodeId>> {
    cheapest_entry_on(tactics, &mut EntrySteps::new(spawn, goal, side, within_m), damage_per_s)
}

/// [entry steps] [`cheapest_entry`] over `steps`' kept sources and steps (made by the searches before it, else now): the
/// same answer as a fresh search's under the same zones ([`EntrySteps`]).
pub fn cheapest_entry_on(tactics: &Tactics, steps: &mut EntrySteps, damage_per_s: impl Fn(NodeId) -> f32) -> Option<Vec<NodeId>> {
    if !entry_walkable(tactics, steps.goal, steps.side) {
        return None;
    }
    searched_entry_on(tactics, steps, damage_per_s)
}

/// [entry steps] Everything [`cheapest_entry`]'s search reads but the price, for one spawn floor, goal, side and box: the
/// sources (the box's dry floors by a marked one that the side reaches, in the box's order) and each floor's steps (its links
/// onto a floor inside the box that [`entry_walkable`] lets through, with a finite walk: the floor, the length term
/// `walk.max(metres)` and the metres), a floor's made the first time a search expands it. Most of a search's time went to these
/// tests, the same for every price (a battery off its spawner on batt m1 s7: 4 armours, 4 searches of 42-65 ms in one commander
/// pass, each 111 thousand floors expanded over 597 thousand links, alike to the floor). A search for another price over the
/// kept steps adds the same terms in the same order onto the same floors, so it finds the way a fresh search finds, bit for
/// bit, while the zones, the live fields and the spawn areas are what they were when the steps were made: their keeper keeps
/// them for one [`super::hazards::Hazards::version`] and zones' stamp (`plays::memo::Memo::entry`).
pub struct EntrySteps {
    spawn: NodeId,
    goal: NodeId,
    side: Option<i32>,
    within_m: f32,
    sources: Option<Vec<NodeId>>,
    /// Each expanded floor's steps: their start in `steps` and how many.
    of: Option<ByFloor<(u32, u32)>>,
    steps: Vec<Step>,
}

/// [entry steps] A step of [`cheapest_entry`]'s search: onto `to`, the walk's length term `walk.max(metres)`, the metres.
#[derive(Clone, Copy)]
struct Step {
    to: NodeId,
    walk: f32,
    metres: f32,
}

impl EntrySteps {
    /// None made yet, for the search from spawn floor `spawn` to `goal` for side `side` within `within_m` across of it.
    pub fn new(spawn: NodeId, goal: NodeId, side: Option<i32>, within_m: f32) -> Self {
        Self { spawn, goal, side, within_m, sources: None, of: None, steps: Vec::new() }
    }

    /// Whether these are the steps of that search.
    pub fn of_search(&self, spawn: NodeId, goal: NodeId, side: Option<i32>, within_m: f32) -> bool {
        (self.spawn, self.goal, self.side, self.within_m.to_bits()) == (spawn, goal, side, within_m.to_bits())
    }

    /// The floors kept and the steps, for the keeper's bound on what it holds.
    pub fn kept(&self) -> usize {
        self.steps.len()
    }

    /// The sources, made at the first ask.
    fn sources(&mut self, tactics: &Tactics) -> &[NodeId] {
        let (grid, hazards) = (&tactics.world.grid, &tactics.hazards);
        let (spawn, side, within_m, at) = (self.spawn, self.side, self.within_m, grid.position(self.goal));
        self.sources.get_or_insert_with(|| {
            let area = [at.x - within_m, at.z - within_m, at.x + within_m, at.z + within_m];
            grid.floors_in(area)
                .filter(|&node| {
                    // Only a dry floor by a marked one can be an entry: the others' dry neighbours are sources at the same price.
                    hazards.hazard_free(node)
                        && grid.links(node).any(|(b, _)| !hazards.hazard_free(b))
                        && tactics.past_fixed_zones(node)
                        && tactics.spawn_ok(node, side, false)
                        && grid.may_reach(spawn, node)
                })
                .collect()
        })
    }

    /// Floor `a`'s steps (their range in `steps`), made at the first ask.
    fn steps_from(&mut self, tactics: &Tactics, a: NodeId) -> std::ops::Range<usize> {
        let grid = &tactics.world.grid;
        let of = self.of.get_or_insert_with(|| ByFloor::new(grid.len()));
        if let Some((start, n)) = of.get(a) {
            return start as usize..(start + n) as usize;
        }
        let start = self.steps.len();
        let from = grid.position(a);
        for (b, kind) in grid.links(a) {
            let to = grid.position(b);
            let at = grid.position(self.goal);
            if !((to.x - at.x).abs() <= self.within_m && (to.z - at.z).abs() <= self.within_m) || !entry_walkable(tactics, b, self.side) {
                continue;
            }
            let walk = grid.link_cost(a, b, kind);
            if !walk.is_finite() {
                continue;
            }
            let metres = (to - from).length();
            self.steps.push(Step { to: b, walk: walk.max(metres), metres });
        }
        let end = self.steps.len();
        self.of.as_mut().expect("made above").set(a, (start as u32, (end - start) as u32));
        start..end
    }
}

/// The floors [`cheapest_entry`]'s search steps onto: a dry one or one only zones on all match mark, not of another side's
/// spawn area.
fn entry_walkable(tactics: &Tactics, b: NodeId, side: Option<i32>) -> bool {
    let hazards = &tactics.hazards;
    (hazards.hazard_free(b) || hazards.only_fixed_at(b)) && tactics.spawn_ok(b, side, false)
}

/// [`cheapest_entry`]'s search itself, whatever the goal.
#[cfg(test)]
fn searched_entry(
    tactics: &Tactics,
    spawn: NodeId,
    goal: NodeId,
    side: Option<i32>,
    damage_per_s: impl Fn(NodeId) -> f32,
    within_m: f32,
) -> Option<Vec<NodeId>> {
    searched_entry_on(tactics, &mut EntrySteps::new(spawn, goal, side, within_m), damage_per_s)
}

/// [`cheapest_entry_on`]'s search itself, whatever the goal: an [`EntrySearch`] run to its end at once.
fn searched_entry_on(tactics: &Tactics, steps: &mut EntrySteps, damage_per_s: impl Fn(NodeId) -> f32) -> Option<Vec<NodeId>> {
    let mut search = EntrySearch::sources(tactics, steps);
    match search.run(tactics, steps, damage_per_s, &mut PopBudget::Unbounded) {
        EntryStep::Done(way) => way,
        EntryStep::Pending => unreachable!("an unbounded search ends"),
    }
}

/// [entry slices] How many floors an [`EntrySearch`] may pop now: all of them, or what is left of a poll's budget (counted down
/// as it pops, so the searches of one poll share it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PopBudget {
    Unbounded,
    Left(u64),
}

impl PopBudget {
    /// One pop taken, if one is left.
    pub(crate) fn take(&mut self) -> bool {
        match self {
            Self::Unbounded => true,
            Self::Left(0) => false,
            Self::Left(n) => {
                *n -= 1;
                true
            }
        }
    }
}

/// [entry slices] Where an [`EntrySearch`] stands after a run: its answer ([`cheapest_entry`]'s), or not yet (its budget ran
/// out first).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EntryStep {
    Done(Option<Vec<NodeId>>),
    Pending,
}

/// [entry slices] [`cheapest_entry_on`]'s search kept between runs: [`Self::begin`] lays its sources, and each [`Self::run`]
/// pops floors until the goal, the heap's end or the budget's end, stepping by the search's [`EntrySteps`]. The pops are the
/// whole search's, in its order, whatever the slices, so a search run in slices gives the same way as one run at once, floor
/// for floor, while the zones stay what they were (the steps' keeper's bound, as for the steps).
pub struct EntrySearch {
    state: SearchState,
}

enum SearchState {
    /// The goal is not a floor the search walks onto: no way ([`cheapest_entry_on`]'s first test).
    NoWay,
    Running(Box<Running>),
}

struct Running {
    // [CPU] By floor, not in maps (a box holds 180-240 thousand floors on Chemical Plant's batteries, and the maps'
    // hashing was a tenth of a search), and the price of a floor stepped onto worked out once, not at every link into it.
    price: ByFloor<f32>,
    prev: ByFloor<NodeId>,
    stepped_onto: ByFloor<f32>,
    heap: BinaryHeap<(Reverse<u32>, NodeId)>,
}

impl EntrySearch {
    /// [`cheapest_entry_on`]'s search over `steps`: no way at once where the goal is not a floor it walks onto, else its
    /// sources laid.
    pub fn begin(tactics: &Tactics, steps: &mut EntrySteps) -> Self {
        if !entry_walkable(tactics, steps.goal, steps.side) {
            return Self { state: SearchState::NoWay };
        }
        Self::sources(tactics, steps)
    }

    /// The search with its sources laid at price 0, whatever the goal.
    fn sources(tactics: &Tactics, steps: &mut EntrySteps) -> Self {
        let floors = tactics.world.grid.len();
        let mut running = Running {
            price: ByFloor::new(floors),
            prev: ByFloor::new(floors),
            stepped_onto: ByFloor::new(floors),
            heap: BinaryHeap::new(),
        };
        for &node in steps.sources(tactics) {
            running.price.set(node, 0.0);
            running.heap.push((Reverse(0.0f32.to_bits()), node));
        }
        Self { state: SearchState::Running(Box::new(running)) }
    }

    /// Pops floors while `budget` lasts, by `steps` (the search's: the same spawn, goal, side and box), each floor stepped onto
    /// priced at `damage_per_s` (the same at every run of one search): the way, none, or not yet.
    pub fn run(&mut self, tactics: &Tactics, steps: &mut EntrySteps, damage_per_s: impl Fn(NodeId) -> f32, budget: &mut PopBudget) -> EntryStep {
        let SearchState::Running(running) = &mut self.state else {
            return EntryStep::Done(None);
        };
        let Running { price, prev, stepped_onto, heap } = &mut **running;
        let goal = steps.goal;
        let mut floor_price = |b: NodeId| match stepped_onto.get(b) {
            Some(d) => d,
            None => {
                let d = damage_per_s(b);
                stepped_onto.set(b, d);
                d
            }
        };
        loop {
            if heap.is_empty() {
                return EntryStep::Done(None);
            }
            if !budget.take() {
                return EntryStep::Pending;
            }
            let Some((Reverse(bits), a)) = heap.pop() else {
                return EntryStep::Done(None);
            };
            let here = f32::from_bits(bits);
            if price.get(a).is_some_and(|p| here > p) {
                continue;
            }
            if a == goal {
                let mut path = vec![goal];
                let mut node = goal;
                while let Some(before) = prev.get(node) {
                    path.push(before);
                    node = before;
                }
                path.reverse();
                return EntryStep::Done(Some(path));
            }
            for k in steps.steps_from(tactics, a) {
                let Step { to: b, walk, metres } = steps.steps[k];
                let damage = floor_price(b) * metres / CROSS_MPS;
                let through = here + walk + damage * DAMAGE_M;
                if price.get(b).is_none_or(|p| through < p) {
                    price.set(b, through);
                    prev.set(b, a);
                    heap.push((Reverse(through.to_bits()), b));
                }
            }
        }
    }
}

/// [entry slices] The price [`cheapest_entry`]'s search gives `path` (its entry at 0, each step priced as the search prices it,
/// in the same order): the search's own way is the least of every way's price, so another way priced with the same
/// `damage_per_s` is never below it.
pub fn entry_price(tactics: &Tactics, path: &[NodeId], damage_per_s: impl Fn(NodeId) -> f32) -> f32 {
    let grid = &tactics.world.grid;
    path.windows(2).fold(0.0f32, |here, w| {
        let (a, b) = (w[0], w[1]);
        let (from, to) = (grid.position(a), grid.position(b));
        let walk = grid.links(a).find(|&(n, _)| n == b).map_or(f32::INFINITY, |(_, kind)| grid.link_cost(a, b, kind));
        let metres = (to - from).length();
        let damage = damage_per_s(b) * metres / CROSS_MPS;
        here + walk.max(metres) + damage * DAMAGE_M
    })
}

/// The search [`searched_entry`] was before its prices were kept by floor: the reference its test holds it to.
#[cfg(test)]
fn searched_entry_by_maps(
    tactics: &Tactics,
    spawn: NodeId,
    goal: NodeId,
    side: Option<i32>,
    damage_per_s: impl Fn(NodeId) -> f32,
    within_m: f32,
) -> Option<Vec<NodeId>> {
    use std::collections::HashMap;
    let grid = &tactics.world.grid;
    let hazards = &tactics.hazards;
    let at = grid.position(goal);
    let area = [at.x - within_m, at.z - within_m, at.x + within_m, at.z + within_m];
    let inside = |p: mapdata::Vec3| (p.x - at.x).abs() <= within_m && (p.z - at.z).abs() <= within_m;
    let walkable = |b: NodeId| entry_walkable(tactics, b, side);
    let mut price: HashMap<NodeId, f32> = HashMap::new();
    let mut prev: HashMap<NodeId, NodeId> = HashMap::new();
    let mut heap = BinaryHeap::new();
    for node in grid.floors_in(area) {
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
            let damage = damage_per_s(b) * metres / CROSS_MPS;
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

/// [CPU] A value by floor, none till it is set: [`searched_entry`]'s prices, ways back and floor prices. Zeroed memory
/// (the allocator's own: only the floors a search reaches are touched) and a bit a floor for "set".
pub(super) struct ByFloor<T> {
    value: Vec<T>,
    set: Vec<u64>,
}

impl<T: Copy + Default> ByFloor<T> {
    pub(super) fn new(floors: usize) -> Self {
        Self { value: vec![T::default(); floors], set: vec![0; floors.div_ceil(64)] }
    }

    pub(super) fn get(&self, node: NodeId) -> Option<T> {
        let n = node as usize;
        (self.set[n / 64] >> (n % 64) & 1 != 0).then(|| self.value[n])
    }

    pub(super) fn set(&mut self, node: NodeId, value: T) {
        let n = node as usize;
        self.set[n / 64] |= 1 << (n % 64);
        self.value[n] = value;
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use mapdata::{Rotation, Shape, ShapeKind, Vec3, Zone, ZoneKind};

    /// [CPU] [`cheapest_entry`]'s answer at once for a goal it never steps onto is the search's own: on the field with a
    /// zone on all match in its middle, for the floors round the zone, side 1's spawn area (which side 0's search never
    /// steps onto) and floors all over, the two answer alike, and both kinds of goal are met.
    #[test]
    fn a_goal_the_search_never_steps_onto_is_answered_at_once_as_the_search_answers() {
        let world = crate::v026g::bots::tactics::tests::field_with(|map| {
            map.zones.push(Zone {
                kind: ZoneKind::Damage,
                name: "slab".into(),
                team: -1,
                position: Vec3::new(25.0, -1.27, 32.0),
                protection: None,
                damage_type: "radiation".into(),
                damage: Some(mapdata::Damage { max_hit: 0.1, interval_ms: 1000, parts: 2, ..mapdata::Damage::default() }),
                shapes: vec![Shape {
                    kind: ShapeKind::Box,
                    center: Vec3::new(25.0, -1.27, 32.0),
                    rotation: Rotation::IDENTITY,
                    half: Vec3::new(3.0, 1.0, 3.0),
                }],
            });
        });
        let tactics = Tactics::new(world, 1.0);
        let grid = &tactics.world.grid;
        let side = Some(0);
        let spawn = grid.locate(Vec3::new(5.0, 0.0, 10.0), 1.0).unwrap();
        let price = |b: NodeId| tactics.hazards.hit_per_s(b);
        let round_zone = grid.floors_in([20.0, 27.0, 30.0, 37.0]).step_by(7);
        let spawn_area = grid.floors_in([72.0, 8.0, 78.0, 22.0]).step_by(11);
        let all_over = (0..grid.len() as NodeId).step_by(4001);
        let (mut never, mut found, mut asked) = (0, 0, 0);
        for goal in round_zone.chain(spawn_area).chain(all_over) {
            let at_once = cheapest_entry(&tactics, spawn, goal, side, price, 40.0);
            assert_eq!(at_once, searched_entry(&tactics, spawn, goal, side, price, 40.0), "goal {goal}");
            never += usize::from(!entry_walkable(&tactics, goal, side));
            found += usize::from(at_once.is_some());
            asked += 1;
        }
        assert!(never > 0 && found > 0, "{never} goals never stepped onto, {found} found, of {asked}");
    }

    /// [CPU] The search with its prices kept by floor finds the ways the search with maps found, floor for floor: on the
    /// field with a zone on all match (a price that varies by floor), for the floors round the zone and all over.
    #[test]
    fn the_search_by_floor_finds_the_ways_the_search_by_maps_found() {
        let world = crate::v026g::bots::tactics::tests::field_with(|map| {
            map.zones.push(Zone {
                kind: ZoneKind::Damage,
                name: "slab".into(),
                team: -1,
                position: Vec3::new(30.0, -1.27, 30.0),
                protection: None,
                damage_type: "radiation".into(),
                damage: Some(mapdata::Damage { max_hit: 0.1, interval_ms: 1000, parts: 2, ..mapdata::Damage::default() }),
                shapes: vec![Shape {
                    kind: ShapeKind::Box,
                    center: Vec3::new(30.0, -1.27, 30.0),
                    rotation: Rotation::IDENTITY,
                    half: Vec3::new(4.0, 1.0, 4.0),
                }],
            });
        });
        let tactics = Tactics::new(world, 1.0);
        let grid = &tactics.world.grid;
        let spawn = grid.locate(Vec3::new(5.0, 0.0, 10.0), 1.0).unwrap();
        // A price by floor: the zone's hit, and a little more toward the north (no two ways price alike by chance).
        let price = |b: NodeId| tactics.hazards.hit_per_s(b) + grid.position(b).z * 1e-4;
        let mut found = 0;
        for goal in grid.floors_in([24.0, 24.0, 36.0, 36.0]).step_by(5).chain((0..grid.len() as NodeId).step_by(3001)) {
            let by_floor = searched_entry(&tactics, spawn, goal, Some(0), price, 20.0);
            assert_eq!(by_floor, searched_entry_by_maps(&tactics, spawn, goal, Some(0), price, 20.0), "goal {goal}");
            found += usize::from(by_floor.is_some());
        }
        assert!(found > 10, "{found} ways found");
    }

    /// [entry steps] Searches for three prices over one kept set of steps (the first search makes them, the others walk them)
    /// find the ways fresh searches find, floor for floor, and the kept steps are the first search's: on the field with a zone on
    /// all match, for the floors round the zone and all over.
    #[test]
    fn searches_for_other_prices_on_kept_steps_find_the_fresh_searches_ways() {
        let world = crate::v026g::bots::tactics::tests::field_with(|map| {
            map.zones.push(Zone {
                kind: ZoneKind::Damage,
                name: "slab".into(),
                team: -1,
                position: Vec3::new(30.0, -1.27, 30.0),
                protection: None,
                damage_type: "radiation".into(),
                damage: Some(mapdata::Damage { max_hit: 0.1, interval_ms: 1000, parts: 2, ..mapdata::Damage::default() }),
                shapes: vec![Shape {
                    kind: ShapeKind::Box,
                    center: Vec3::new(30.0, -1.27, 30.0),
                    rotation: Rotation::IDENTITY,
                    half: Vec3::new(4.0, 1.0, 4.0),
                }],
            });
        });
        let tactics = Tactics::new(world, 1.0);
        let grid = &tactics.world.grid;
        let spawn = grid.locate(Vec3::new(5.0, 0.0, 10.0), 1.0).unwrap();
        // Three prices by floor that order the ways differently: the zone's hit, more toward the north, more toward the east.
        let hit = |b: NodeId| tactics.hazards.hit_per_s(b);
        let north = |b: NodeId| tactics.hazards.hit_per_s(b) + grid.position(b).z * 1e-4;
        let east = |b: NodeId| tactics.hazards.hit_per_s(b) * 2.0 + grid.position(b).x * 3e-4;
        let (mut found, mut kept_reused) = (0, 0);
        for goal in grid.floors_in([24.0, 24.0, 36.0, 36.0]).step_by(5).chain((0..grid.len() as NodeId).step_by(3001)) {
            let mut steps = EntrySteps::new(spawn, goal, Some(0), 20.0);
            let first = cheapest_entry_on(&tactics, &mut steps, hit);
            assert_eq!(first, cheapest_entry(&tactics, spawn, goal, Some(0), hit, 20.0), "goal {goal}, the first price");
            let made = steps.kept();
            for (name, price) in [("north", &north as &dyn Fn(NodeId) -> f32), ("east", &east)] {
                let on_kept = cheapest_entry_on(&tactics, &mut steps, price);
                assert_eq!(on_kept, searched_entry_by_maps(&tactics, spawn, goal, Some(0), price, 20.0), "goal {goal}, {name}");
            }
            kept_reused += usize::from(made > 0);
            found += usize::from(first.is_some());
            assert!(steps.of_search(spawn, goal, Some(0), 20.0) && !steps.of_search(spawn, goal, Some(1), 20.0));
        }
        assert!(found > 10 && kept_reused > 10, "{found} ways found, {kept_reused} searches walked kept steps");
    }
}
