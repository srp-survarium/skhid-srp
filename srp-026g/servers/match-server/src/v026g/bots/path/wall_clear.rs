//! A route's corners kept where the body's capsule fits (A191b, `think.wall_clear_corners`).
//!
//! The grid keeps its floors 0.3175 m from a wall, the capsule is 0.38 m in radius and the controller holds it 0.41 m off
//! (`WalkGrid::wall_clearance`), so a waypoint on a floor in between is a place the body cannot stand, and a leg pulled past a
//! wall's corner passes it at the grid's distance. After the pull (the wall analogue of the lip rule of `pull_straight`):
//!
//! 1. a corner whose floor is under [`WALL_HOLD_M`] from a wall moves to the floor within [`NUDGE_M`] that is furthest from
//!    one (up to the hold), where both its legs stay straight walks;
//! 2. a leg whose floors dip under [`CAPSULE_FIT_M`] gets a corner at the floor with room (the hold or more) nearest to the
//!    dip, within [`ALTERNATIVE_M`], when both new legs stay straight and are clearer than the leg was (the funnel's portals
//!    inset by the radius, done after the pull).
//!
//! Corners of a jump, a drop, a low floor, an edge floor (a lip, near a fall), a door gate or any point the route's records are
//! keyed by stay where they are; a leg of those is left as it is. A grid without the clearance table changes nothing.

use super::*;

/// The distance to a wall the controller holds a body at, and the radius of the capsule: the clearances of [`WalkGrid::wall_clearance`].
pub const WALL_HOLD_M: f32 = 0.41;
pub const CAPSULE_FIT_M: f32 = 0.38;
/// How far a corner moves at most, m.
const NUDGE_M: f32 = 0.5;
/// How far from the dip of a leg its new corner may be, m.
const ALTERNATIVE_M: f32 = 0.6;
/// The step a leg is looked at in, m, and the shortest leg looked at.
const LEG_STEP_M: f32 = 0.125;
const LEG_MIN_M: f32 = 1.0;
/// Floors tried for a straight pair of legs, nearest the best first.
const TRIES: usize = 6;
/// A move or a corner is made for this much more clearance or more, m.
const GAIN_M: f32 = 0.02;
/// A floor this far in height from a point is not the point's floor, m.
const SAME_FLOOR_M: f32 = 0.3;

/// What the pass does, in counts (a process-wide tally, as `below_walk`).
pub mod counts {
    use std::sync::atomic::{AtomicU64, Ordering};

    #[derive(Clone, Copy)]
    pub enum Count {
        /// Routes the pass looked at.
        Routes,
        /// Corners under the hold; of them moved, and kept (no floor with more room and straight legs).
        CornersLow,
        CornersMoved,
        CornersKept,
        /// Legs dipping under the radius; of them given a corner, and kept (no floor with room, or no clearer legs).
        LegsDipping,
        LegsCorner,
        LegsKept,
        /// Metres the routes got longer by, in millimetres.
        AddedMm,
    }
    pub const NAMES: [&str; 8] = ["routes", "corners_low", "corners_moved", "corners_kept", "legs_dipping", "legs_corner", "legs_kept", "added_mm"];
    static COUNTS: [AtomicU64; 8] = [const { AtomicU64::new(0) }; 8];

    pub fn add(count: Count, n: u64) {
        COUNTS[count as usize].fetch_add(n, Ordering::Relaxed);
    }

    pub fn counts() -> Vec<u64> {
        COUNTS.iter().map(|c| c.load(Ordering::Relaxed)).collect()
    }
}
use counts::{Count, add};

fn edge(grid: &WalkGrid, node: NodeId) -> bool {
    grid.is_lip(node) || grid.near_fall(node) || grid.near_lip(node)
}

/// The least clearance of the floors under the straight line from `a` to `b`, sampled every [`LEG_STEP_M`], and where.
fn least_clearance(grid: &WalkGrid, a: Vec3, b: Vec3) -> Option<(f32, Vec3)> {
    let length = a.distance_xz(b);
    let steps = ((length / LEG_STEP_M).ceil() as usize).max(1);
    let mut least: Option<(f32, Vec3)> = None;
    for k in 0..=steps {
        let t = k as f32 / steps as f32;
        let p = Vec3::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t, a.z + (b.z - a.z) * t);
        let Some(node) = grid.locate(p, SAME_FLOOR_M) else { continue };
        let Some(c) = grid.wall_clearance(node) else { continue };
        if least.is_none_or(|(l, _)| c < l) {
            least = Some((c, p));
        }
    }
    least
}

impl Route {
    /// Whether `at` is a point something of the route is keyed by (a gate, a jump's landing, a drop's).
    fn keyed(&self, at: Vec3) -> bool {
        self.gates.contains(&at)
            || self.ledge_tops.iter().any(|(p, _)| *p == at)
            || self.ledge_leads.iter().any(|(p, _)| *p == at)
            || self.sprint_drops.iter().any(|(p, _)| *p == at)
    }

    /// Whether the leg into point `k` is a plain walk (no jump, no low floor, no drop).
    fn walked_into(&self, k: usize) -> bool {
        let (a, b) = (self.points[k - 1], self.points[k]);
        b.jump.is_none() && !b.low && b.at.y - a.at.y > -DROP_LINK_MIN_M
    }

    /// This route with its corners kept where the capsule fits (module documentation). A grid with no clearance table, or a
    /// route of fewer than two points, comes back as it is.
    pub fn wall_cleared(self, grid: &WalkGrid) -> Self {
        let n = self.points.len();
        if n < 2 || !grid.has_wall_clearance() {
            return self;
        }
        add(Count::Routes, 1);
        let mut points = self.points.clone();
        // 1. The corners.
        for k in 1..n - 1 {
            let p = points[k];
            if !self.walked_into(k) || !self.walked_into(k + 1) || self.keyed(p.at) || edge(grid, p.node) {
                continue;
            }
            let Some(here) = grid.wall_clearance(p.node) else { continue };
            if here >= WALL_HOLD_M {
                continue;
            }
            add(Count::CornersLow, 1);
            let mut candidates: Vec<(f32, f32, NodeId)> = grid
                .floors_in([p.at.x - NUDGE_M, p.at.z - NUDGE_M, p.at.x + NUDGE_M, p.at.z + NUDGE_M])
                .filter_map(|c| {
                    let at = grid.position(c);
                    let room = grid.wall_clearance(c)?.min(WALL_HOLD_M);
                    (at.distance_xz(p.at) <= NUDGE_M
                        && (at.y - p.at.y).abs() <= SAME_FLOOR_M
                        && room >= here.min(WALL_HOLD_M) + GAIN_M
                        && grid.can_stand(c)
                        && !edge(grid, c))
                    .then_some((room, at.distance_xz(p.at), c))
                })
                .collect();
            // Most room first, then the nearest.
            candidates.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.total_cmp(&b.1)).then(a.2.cmp(&b.2)));
            let (before, after) = (points[k - 1].node, points[k + 1].node);
            let chosen = candidates.iter().take(TRIES).find(|&&(_, _, c)| straight(grid, before, c) && straight(grid, c, after));
            match chosen {
                Some(&(_, _, c)) => {
                    points[k] = Waypoint { at: grid.position(c), node: c, ..p };
                    add(Count::CornersMoved, 1);
                }
                None => add(Count::CornersKept, 1),
            }
        }
        // 2. The legs: a corner put in the one that dips.
        let mut corner: Vec<Option<Waypoint>> = vec![None; n];
        for k in 1..n {
            let (a, b) = (points[k - 1], points[k]);
            if !self.walked_into(k) || a.at.distance_xz(b.at) < LEG_MIN_M || self.gates.contains(&a.at) && self.gates.contains(&b.at) {
                continue;
            }
            let Some((dip, at)) = least_clearance(grid, a.at, b.at) else { continue };
            if dip >= CAPSULE_FIT_M {
                continue;
            }
            add(Count::LegsDipping, 1);
            let mut candidates: Vec<(f32, NodeId)> = grid
                .floors_in([at.x - ALTERNATIVE_M, at.z - ALTERNATIVE_M, at.x + ALTERNATIVE_M, at.z + ALTERNATIVE_M])
                .filter_map(|c| {
                    let p = grid.position(c);
                    (p.distance_xz(at) <= ALTERNATIVE_M
                        && (p.y - at.y).abs() <= SAME_FLOOR_M
                        && grid.wall_clearance(c)? >= WALL_HOLD_M
                        && grid.can_stand(c)
                        && !edge(grid, c))
                    .then_some((p.distance_xz(at), c))
                })
                .collect();
            candidates.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
            let clearer = |c: NodeId| {
                let p = grid.position(c);
                let (one, two) = (least_clearance(grid, a.at, p), least_clearance(grid, p, b.at));
                matches!((one, two), (Some((x, _)), Some((y, _))) if x.min(y) >= dip + GAIN_M)
            };
            let chosen = candidates
                .iter()
                .take(TRIES)
                .find(|&&(_, c)| straight(grid, a.node, c) && straight(grid, c, b.node) && clearer(c));
            match chosen {
                Some(&(_, c)) => {
                    corner[k] = Some(Waypoint { at: grid.position(c), node: c, jump: None, low: false });
                    add(Count::LegsCorner, 1);
                }
                None => add(Count::LegsKept, 1),
            }
        }
        if points.iter().zip(&self.points).all(|(a, b)| a.at == b.at) && corner.iter().all(Option::is_none) {
            return self;
        }
        let mut with_corners: Vec<Waypoint> = Vec::with_capacity(n + 2);
        let mut new_at = Vec::with_capacity(n);
        let mut total = 0.0f32;
        for k in 0..n {
            if k > 0 {
                let (a, b) = (points[k - 1].at, points[k].at);
                total += corner[k].map_or(a.distance_xz(b), |c| a.distance_xz(c.at) + c.at.distance_xz(b));
            }
            new_at.push(total);
            if let Some(c) = corner[k] {
                with_corners.push(c);
            }
            with_corners.push(points[k]);
        }
        add(Count::AddedMm, ((total - self.along[n - 1]).max(0.0) * 1000.0) as u64);
        let floors = self.rescaled_floors(&new_at);
        let Self { sprint_drops, ledge_leads, ledge_tops, gates, planned, below_walk_m, .. } = self;
        let mut route = Self::with_floors(with_corners, floors);
        route.sprint_drops = sprint_drops;
        route.ledge_leads = ledge_leads;
        route.ledge_tops = ledge_tops;
        route.gates = gates;
        route.planned = planned;
        route.below_walk_m = below_walk_m;
        route
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mapdata::{Body, Erosion, Triangle, WalkerBehaviour};

    const AREA: [f32; 4] = [0.0, 0.0, 10.0, 10.0];

    fn tri(a: Vec3, b: Vec3, c: Vec3) -> Triangle {
        Triangle { v: [a, b, c], walker: WalkerBehaviour::BySlope }
    }

    fn floor() -> Vec<Triangle> {
        let p = |x, z| Vec3::new(x, 0.0, z);
        vec![tri(p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0)), tri(p(0.0, 0.0), p(10.0, 10.0), p(0.0, 10.0))]
    }

    /// A box of walls, 2 m high, from (`x0`, `z0`) to (`x1`, `z1`).
    fn block(x0: f32, z0: f32, x1: f32, z1: f32) -> Vec<Triangle> {
        let p = |x, y, z| Vec3::new(x, y, z);
        let mut out = Vec::new();
        for ((ax, az), (bx, bz)) in [((x0, z0), (x1, z0)), ((x1, z0), (x1, z1)), ((x1, z1), (x0, z1)), ((x0, z1), (x0, z0))] {
            out.push(tri(p(ax, 0.0, az), p(bx, 0.0, bz), p(bx, 2.0, bz)));
            out.push(tri(p(ax, 0.0, az), p(bx, 2.0, bz), p(ax, 2.0, az)));
        }
        out
    }

    fn grid_of(tris: &[Triangle]) -> WalkGrid {
        WalkGrid::build(tris, AREA, 0.25, Body { erosion: Erosion::Exact, ..Body::default() }).with_wall_clearance(tris)
    }

    fn waypoint(grid: &WalkGrid, x: f32, z: f32) -> Waypoint {
        let node = grid.locate(Vec3::new(x, 0.0, z), 0.05).expect("a floor");
        Waypoint { at: grid.position(node), node, jump: None, low: false }
    }

    fn least(route: &Route, grid: &WalkGrid) -> f32 {
        route.points().windows(2).filter_map(|w| least_clearance(grid, w[0].at, w[1].at)).map(|(c, _)| c).fold(f32::MAX, f32::min)
    }

    /// A corner on a floor 0.375 m from a wall (the grid keeps it; the capsule does not fit) moves to the floor 0.625 m from
    /// it, the nearest with the hold's room, and the route's least clearance rises.
    #[test]
    fn a_corner_beside_a_wall_moves_to_where_the_capsule_fits() {
        let mut tris = floor();
        tris.extend(block(5.0, 0.0, 5.5, 10.0));
        let grid = grid_of(&tris);
        let route = Route::new(vec![waypoint(&grid, 1.125, 2.125), waypoint(&grid, 4.625, 5.125), waypoint(&grid, 1.125, 8.125)]);
        assert!(grid.wall_clearance(route.points()[1].node).unwrap() < CAPSULE_FIT_M);
        let cleared = route.clone().wall_cleared(&grid);
        let moved = cleared.points()[1];
        assert!(grid.wall_clearance(moved.node).unwrap() >= WALL_HOLD_M, "{:?}", grid.wall_clearance(moved.node));
        assert!(moved.at.distance_xz(route.points()[1].at) <= NUDGE_M + 1e-4);
        assert!(least(&cleared, &grid) > least(&route, &grid));
        // The first and last points are not moved.
        assert_eq!(cleared.points()[0].at, route.points()[0].at);
        assert_eq!(cleared.points()[2].at, route.points()[2].at);
    }

    /// A leg that passes a post's face at 0.375 m gets a corner on the floor with room beside the dip, and the legs
    /// beside it are clearer; a route with no clearance table, and one clear of walls, come back as they were.
    #[test]
    fn a_leg_past_a_post_gets_a_corner_with_room() {
        let mut tris = floor();
        tris.extend(block(5.0, 5.0, 5.5, 5.5));
        let grid = grid_of(&tris);
        let route = Route::new(vec![waypoint(&grid, 2.125, 4.625), waypoint(&grid, 8.125, 4.625)]);
        assert!(least(&route, &grid) < CAPSULE_FIT_M);
        let cleared = route.clone().wall_cleared(&grid);
        assert_eq!(cleared.points().len(), 3, "a corner put in the leg");
        assert!(least(&cleared, &grid) >= CAPSULE_FIT_M, "{}", least(&cleared, &grid));
        let plain = WalkGrid::build(&tris, AREA, 0.25, Body { erosion: Erosion::Exact, ..Body::default() });
        let same = Route::new(route.points().to_vec()).wall_cleared(&plain);
        assert_eq!(same.points().len(), 2, "no table, no change");
        let open = Route::new(vec![waypoint(&grid, 1.125, 1.125), waypoint(&grid, 8.125, 1.125)]);
        assert_eq!(open.clone().wall_cleared(&grid).points().len(), 2, "a leg clear of walls is as it was");
    }

    /// A door gate is a corner at the doorway's middle (A66): it stays where it is, where the same corner not a gate moves.
    #[test]
    fn a_gate_stays() {
        let mut tris = floor();
        tris.extend(block(5.0, 0.0, 5.5, 10.0));
        let grid = grid_of(&tris);
        let low = waypoint(&grid, 4.625, 5.125);
        let route = Route::new(vec![waypoint(&grid, 1.125, 2.125), low, waypoint(&grid, 1.125, 8.125)]);
        assert_ne!(route.clone().wall_cleared(&grid).points()[1].at, low.at, "moves as a plain corner");
        let gated = route.with_gate(low.at);
        assert_eq!(gated.wall_cleared(&grid).points()[1].at, low.at);
    }
}
