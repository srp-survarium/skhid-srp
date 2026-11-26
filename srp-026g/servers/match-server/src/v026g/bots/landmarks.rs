//! [R3] Landmark lower bounds for the route searches (ALT: A*, landmarks and the triangle inequality; Goldberg and Harrelson,
//! "Computing the shortest path: A* search meets graph theory", 2005).
//!
//! For each of [`LANDMARKS`] floors `L`, the cost from `L` to every floor and from every floor to `L`, over the grid's links,
//! each link priced by its straight length (the floors' distance in 3D). Every price a route search puts on a link
//! (`WalkGrid::link_cost_reaching`: the length, plus penalties that are never negative, or infinite; times a filter's
//! weight, at least 1) is at least that length, so these costs are lower bounds of the searches' own, for every body reach
//! and every filter (a shut floor only makes a way dearer). By the triangle inequality, for a floor `v` and the goal `g`:
//! `d(v, g) >= d(v, L) - d(g, L)` and `d(v, g) >= d(L, g) - d(L, v)`. The largest of these over the landmarks is
//! [`Landmarks::lower`]: admissible, and consistent (each term is), so the search's A* keeps finding the least cost way
//! (`mapdata::SlicedSearch::step_bounded`, which takes the larger of it and the ground distance).
//!
//! Built once a map with the tactics' base, off the match loop. Costs are kept in decimetres (u16: 2 bytes a floor and
//! landmark each way), rounded down; a term reads its first cost rounded down and its second rounded up, so the bound stays
//! under the true one. A cost past `u16::MAX - 1` decimetres, or no way at all, is [`FAR`]: its terms are left out.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use mapdata::{NodeId, WalkGrid};

/// The landmarks a map keeps.
pub const LANDMARKS: usize = 8;
/// A cost not kept: no way, or past what a u16 holds in decimetres.
const FAR: u16 = u16::MAX;
/// Decimetres a unit.
const UNIT_M: f32 = 0.1;

/// The landmarks' costs of a map's grid ([`Self::build`]).
pub struct Landmarks {
    /// The landmark floors, in the order picked.
    floors: Vec<NodeId>,
    /// `from[v * k + l]`: from landmark `l` to floor `v`, decimetres rounded down ([`FAR`]: none kept).
    from: Vec<u16>,
    /// `to[v * k + l]`: from floor `v` to landmark `l`.
    to: Vec<u16>,
}

/// A float cost in decimetres, rounded down ([`FAR`] when infinite or too far).
fn units(cost: f32) -> u16 {
    if !cost.is_finite() || cost < 0.0 {
        return FAR;
    }
    let u = (cost / UNIT_M).floor();
    if u >= f32::from(FAR) { FAR } else { u as u16 }
}

/// The floors' costs from `seed` over `grid`'s links (or into it, `backwards`), each link its straight 3D length.
fn flood(grid: &WalkGrid, seed: NodeId, backwards: bool) -> Vec<f32> {
    let mut cost = vec![f32::INFINITY; grid.len()];
    let mut open = BinaryHeap::new();
    cost[seed as usize] = 0.0;
    open.push(Reverse((0u32, seed)));
    let mut next = Vec::with_capacity(16);
    while let Some(Reverse((bits, u))) = open.pop() {
        let c = f32::from_bits(bits);
        if c > cost[u as usize] {
            continue;
        }
        next.clear();
        if backwards {
            next.extend(grid.incoming(u).map(|(v, _)| v));
        } else {
            next.extend(grid.links(u).map(|(v, _)| v));
        }
        let pu = grid.position(u);
        for &v in &next {
            let cv = c + (grid.position(v) - pu).length();
            if cv < cost[v as usize] {
                cost[v as usize] = cv;
                // Non-negative floats order as their bits.
                open.push(Reverse((cv.to_bits(), v)));
            }
        }
    }
    cost
}

impl Landmarks {
    /// None kept: every bound 0 (the search's own ground distance alone).
    pub const fn none() -> Self {
        Self { floors: Vec::new(), from: Vec::new(), to: Vec::new() }
    }

    /// [`Self::none`], for a search with no map's base (`path::plan_in`, tests).
    pub fn none_ref() -> &'static Self {
        static NONE: Landmarks = Landmarks::none();
        &NONE
    }

    /// [`Self::build`], and its log line: the landmarks, their bytes and the build's time (the log's only: no decision
    /// reads the clock).
    pub fn build_logged(grid: &WalkGrid) -> Self {
        let started = std::time::Instant::now();
        let marks = Self::build(grid);
        log::info!(
            "route landmarks: {} for {} floors, {:.1} MB, built in {} ms",
            marks.count(),
            grid.len(),
            marks.bytes() as f64 / 1e6,
            started.elapsed().as_millis()
        );
        marks
    }

    /// Picks [`LANDMARKS`] floors far apart (the farthest-point rule: the first the farthest from floor 0, each next the
    /// floor farthest from those picked, by the costs from them) and floods from and to each.
    pub fn build(grid: &WalkGrid) -> Self {
        let n = grid.len();
        if n == 0 {
            return Self::none();
        }
        let farthest = |near: &[f32]| {
            (0..n).filter(|&v| near[v].is_finite()).max_by(|&a, &b| near[a].total_cmp(&near[b]).then(b.cmp(&a))).map(|v| v as NodeId)
        };
        let first = flood(grid, 0, false);
        let Some(mut pick) = farthest(&first) else {
            return Self::none();
        };
        let mut nearest = vec![f32::INFINITY; n];
        let mut floors = Vec::new();
        let mut from_l = Vec::new();
        let mut to_l = Vec::new();
        while floors.len() < LANDMARKS {
            let from = flood(grid, pick, false);
            let to = flood(grid, pick, true);
            for v in 0..n {
                nearest[v] = nearest[v].min(from[v]);
            }
            floors.push(pick);
            from_l.push(from);
            to_l.push(to);
            match farthest(&nearest) {
                Some(next) if nearest[next as usize] > 0.0 && !floors.contains(&next) => pick = next,
                _ => break,
            }
        }
        let k = floors.len();
        let mut from = vec![FAR; n * k];
        let mut to = vec![FAR; n * k];
        for l in 0..k {
            for v in 0..n {
                from[v * k + l] = units(from_l[l][v]);
                to[v * k + l] = units(to_l[l][v]);
            }
        }
        Self { floors, from, to }
    }

    /// The landmarks kept.
    pub fn count(&self) -> usize {
        self.floors.len()
    }

    /// Bytes held.
    pub fn bytes(&self) -> usize {
        (self.from.len() + self.to.len()) * 2 + self.floors.len() * 4
    }

    /// A lower bound of the cost from floor `v` to floor `goal` (m; 0 with no landmark, or for a floor past the grid).
    #[inline]
    pub fn lower(&self, v: NodeId, goal: NodeId) -> f32 {
        let k = self.floors.len();
        let (v, goal) = (v as usize, goal as usize);
        if k == 0 || (v + 1) * k > self.from.len() || (goal + 1) * k > self.from.len() {
            return 0.0;
        }
        let mut best = 0i32;
        for l in 0..k {
            // d(v, g) >= d(v, L) - d(g, L): d(v, L) down (as kept), d(g, L) up (one unit more).
            let (vl, gl) = (self.to[v * k + l], self.to[goal * k + l]);
            if vl != FAR && gl != FAR {
                best = best.max(i32::from(vl) - i32::from(gl) - 1);
            }
            // d(v, g) >= d(L, g) - d(L, v).
            let (lg, lv) = (self.from[goal * k + l], self.from[v * k + l]);
            if lg != FAR && lv != FAR {
                best = best.max(i32::from(lg) - i32::from(lv) - 1);
            }
        }
        best as f32 * UNIT_M
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every floor's bound to a goal is at most the search's own least cost there (its floods, by the links' own prices), on
    /// the test field: admissible.
    #[test]
    fn the_landmarks_bound_is_under_the_least_cost() {
        let world = crate::v026g::bots::tactics::tests::field();
        let grid = &world.grid;
        let marks = Landmarks::build(grid);
        assert!(marks.count() >= 2, "landmarks picked: {}", marks.count());
        let goal = grid.locate(mapdata::Vec3::new(75.0, 0.0, 12.0), 1.0).unwrap();
        // The grid's own flood into the goal, by the search's link prices.
        let true_cost = grid.distances_to(&[goal]);
        let mut bounded = 0;
        for v in 0..grid.len() as NodeId {
            let lower = marks.lower(v, goal);
            let cost = true_cost.get(v);
            assert!(lower <= cost + 1e-3, "floor {v}: bound {lower} over its cost {cost}");
            bounded += usize::from(lower > 0.0);
        }
        assert!(bounded > grid.len() / 2, "{bounded} of {} floors bounded", grid.len());
    }

    /// [R3] On weighed searches (weights 1 to 4) and plain ones, round a shut strip, the search bounded by the landmarks finds
    /// a way of the same cost as the ground distance's search, and expands no more floors.
    #[test]
    fn a_landmark_bounded_search_costs_what_the_plain_one_finds() {
        let world = crate::v026g::bots::tactics::tests::field();
        let grid = &world.grid;
        let regions = mapdata::regions::Regions::build(grid, None);
        let hot = mapdata::grid::HotIndex::for_grid(grid);
        let marks = Landmarks::build(grid);
        let shut = |n: NodeId| {
            let at = grid.position(n);
            (55.0..57.0).contains(&at.x) && at.z > 4.0
        };
        let allowed = |n: NodeId| !shut(n);
        let weighed = |n: NodeId| 1.0 + (n % 4) as f32;
        let plain = |_: NodeId| 1.0f32;
        let ends = [((5.0, 10.0), (75.0, 12.0)), ((75.0, 2.0), (5.0, 18.0)), ((30.0, 15.0), (70.0, 15.0))];
        let mut fewer = 0;
        for ((ax, az), (bx, bz)) in ends {
            let from = grid.locate(mapdata::Vec3::new(ax, 0.0, az), 1.0).unwrap();
            let to = grid.locate(mapdata::Vec3::new(bx, 0.0, bz), 1.0).unwrap();
            for weight in [&plain as &dyn Fn(NodeId) -> f32, &weighed] {
                let run = |bounded: bool| {
                    let mut search =
                        regions.start_route_search_reaching(grid, from, to, allowed, |_| true, usize::MAX, mapdata::Reach::FULL).ok().unwrap();
                    let mut queue = mapdata::grid::HotOpen::hot_open_empty();
                    let mut iters = usize::MAX;
                    let lower = |v: NodeId| if bounded { marks.lower(v, to) } else { 0.0 };
                    let (found, _) = search
                        .step_bounded(&regions, grid, &hot, &mut queue, allowed, |_| true, weight, lower, &mut iters)
                        .unwrap();
                    let mapdata::Search::Found(path) = found else { panic!("a way from {from} to {to}") };
                    (path.cost, search.expanded())
                };
                let ((cost, expanded), (bounded_cost, bounded_expanded)) = (run(false), run(true));
                assert!((cost - bounded_cost).abs() <= 1e-3 * cost.max(1.0), "{from} -> {to}: cost {cost}, bounded {bounded_cost}");
                assert!(bounded_expanded <= expanded, "{from} -> {to}: expanded {expanded}, bounded {bounded_expanded}");
                fewer += usize::from(bounded_expanded < expanded);
            }
        }
        assert!(fewer > 0, "the bounds cut no search");
    }

    /// Every link's search price is at least its straight length, the landmarks' price, for the full body and one that
    /// cannot sprint.
    #[test]
    fn every_link_is_priced_at_least_its_length() {
        let world = crate::v026g::bots::tactics::tests::field();
        let grid = &world.grid;
        let walker = mapdata::Reach { sprint: mapdata::grid::SprintReach::Cannot { max_rise: Some(mapdata::grid::STANDING_JUMP_M) }, ..mapdata::Reach::FULL };
        for u in 0..grid.len() as NodeId {
            for (v, kind) in grid.links(u) {
                let length = (grid.position(v) - grid.position(u)).length();
                for reach in [mapdata::Reach::FULL, walker] {
                    assert!(grid.link_cost_reaching(u, v, kind, reach) >= length, "{u} -> {v} {kind:?}");
                }
            }
        }
    }
}
