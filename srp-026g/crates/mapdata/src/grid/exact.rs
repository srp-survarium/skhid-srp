//! Exact-distance erosion ([`Erosion::Exact`]).
//!
//! The erosion rounds of [`WalkGrid::erode`] work on cells: a floor goes when a wall is in a cell beside it
//! along a side, so a wall standing diagonally to a floor's cell (a post's corner) or a face at the far edge
//! of the next cell leaves floors nearer to it than the body's radius (A84 item 13: a floor 0.25 m from a post
//! in a 0.65 m opening; the body is 0.76 m wide). This pass runs after the rounds, over the floors they kept:
//! a floor goes when a wall triangle, clipped to the band the body's capsule meets walls in (from a step over
//! the feet to the standing height), comes nearer to the floor's centre than the radius less a quarter of a cell
//! ([`WalkGrid::exact_clearance`]), measured on the ground.
//!
//! Walls are the faces no body walks on: steeper than [`STEEP_NORMAL_Y`], or `dont_walk`, and not `walk` faces
//! (stairs). Floors, ramps and ceilings are not walls here; the rounds keep treating them as before.

use super::*;

/// The side of the bins the wall triangles are kept in, metres.
const BIN_M: f32 = 2.0;

/// The wall triangles of a map, binned on the ground for the distance to a point.
pub(super) struct Walls {
    x0: f32,
    z0: f32,
    nx: usize,
    nz: usize,
    /// Per bin, its first entry in `entries`; `nx * nz + 1` of them.
    start: Vec<u32>,
    /// The triangles of each bin, as indices into `triangles`.
    entries: Vec<u32>,
    triangles: Vec<[Vec3; 3]>,
}

impl Walls {
    /// The walls among `triangles` over the area starting at (`x0`, `z0`) `nx * nz` cells of `cell` metres.
    pub(super) fn new(triangles: &[Triangle], x0: f32, z0: f32, cell: f32, nx: usize, nz: usize) -> Self {
        let (width, depth) = (nx as f32 * cell, nz as f32 * cell);
        let (bx, bz) = (((width / BIN_M).ceil() as usize).max(1), ((depth / BIN_M).ceil() as usize).max(1));
        let walls: Vec<[Vec3; 3]> = triangles
            .iter()
            .filter(|t| is_wall(t))
            .map(|t| t.v)
            .filter(|v| v.iter().all(|p| p.is_finite()))
            .collect();
        let bins_of = |v: &[Vec3; 3]| {
            let min = v[0].min(v[1]).min(v[2]);
            let max = v[0].max(v[1]).max(v[2]);
            let range = |low: f32, high: f32, origin: f32, n: usize| {
                let first = (((low - origin) / BIN_M).floor().max(0.0) as usize).min(n - 1);
                let last = (((high - origin) / BIN_M).floor().max(0.0) as usize).min(n - 1);
                first..=last
            };
            (range(min.x, max.x, x0, bx), range(min.z, max.z, z0, bz))
        };
        let mut counts = vec![0u32; bx * bz + 1];
        for v in &walls {
            let (xs, zs) = bins_of(v);
            for k in zs {
                for i in xs.clone() {
                    counts[k * bx + i + 1] += 1;
                }
            }
        }
        for b in 0..bx * bz {
            counts[b + 1] += counts[b];
        }
        let start = counts.clone();
        let mut entries = vec![0u32; *start.last().unwrap() as usize];
        let mut fill = counts;
        for (t, v) in walls.iter().enumerate() {
            let (xs, zs) = bins_of(v);
            for k in zs {
                for i in xs.clone() {
                    let slot = &mut fill[k * bx + i];
                    entries[*slot as usize] = t as u32;
                    *slot += 1;
                }
            }
        }
        Self { x0, z0, nx: bx, nz: bz, start, entries, triangles: walls }
    }

    /// Whether a wall comes within `limit` metres on the ground of (`x`, `z`) between the heights `low` and
    /// `high`.
    pub(super) fn within(&self, x: f32, z: f32, low: f32, high: f32, limit: f32) -> bool {
        self.nearest(x, z, low, high, limit).is_some_and(|d| d < limit)
    }

    /// The distance on the ground from (`x`, `z`) to the nearest wall between the heights `low` and `high`,
    /// among those within `limit` of it; `None` when there is none.
    pub(super) fn nearest(&self, x: f32, z: f32, low: f32, high: f32, limit: f32) -> Option<f32> {
        let range = |at: f32, origin: f32, n: usize| {
            let first = (((at - limit - origin) / BIN_M).floor().max(0.0) as usize).min(n - 1);
            let last = (((at + limit - origin) / BIN_M).floor().max(0.0) as usize).min(n - 1);
            first..=last
        };
        let mut best: Option<f32> = None;
        let (mut one, mut two) = (Vec::with_capacity(8), Vec::with_capacity(8));
        for k in range(z, self.z0, self.nz) {
            for i in range(x, self.x0, self.nx) {
                let bin = k * self.nx + i;
                for &t in &self.entries[self.start[bin] as usize..self.start[bin + 1] as usize] {
                    let v = self.triangles[t as usize];
                    if v.iter().all(|p| p.y < low) || v.iter().all(|p| p.y > high) {
                        continue;
                    }
                    clip(&v, 1, low, true, &mut one);
                    clip(&one, 1, high, false, &mut two);
                    if two.is_empty() {
                        continue;
                    }
                    let d = ground_distance(&two, x, z);
                    if d <= limit && best.is_none_or(|b| d < b) {
                        best = Some(d);
                    }
                }
            }
        }
        best
    }
}

/// Whether a triangle is a wall: no `walk` face, and steeper than [`STEEP_NORMAL_Y`] (which no body walks on,
/// whatever its material says).
fn is_wall(t: &Triangle) -> bool {
    let [a, b, c] = t.v;
    let normal = (b - a).cross(c - a);
    let length = normal.length();
    t.walker != WalkerBehaviour::Walk && length >= 1e-9 && normal.y.abs() / length < STEEP_NORMAL_Y
}

/// The distance on the ground from (`x`, `z`) to the convex polygon `polygon` (0 inside it).
fn ground_distance(polygon: &[Vec3], x: f32, z: f32) -> f32 {
    let n = polygon.len();
    let mut best = f32::MAX;
    let mut inside = n >= 3;
    let mut side = 0.0f32;
    for i in 0..n {
        let (a, b) = (polygon[i], polygon[(i + 1) % n]);
        let (ex, ez) = (b.x - a.x, b.z - a.z);
        let (px, pz) = (x - a.x, z - a.z);
        let length2 = ex * ex + ez * ez;
        let t = if length2 > 1e-12 { ((px * ex + pz * ez) / length2).clamp(0.0, 1.0) } else { 0.0 };
        best = best.min((px - t * ex).hypot(pz - t * ez));
        let cross = ex * pz - ez * px;
        if cross.abs() > 1e-9 {
            if side != 0.0 && cross.signum() != side.signum() {
                inside = false;
            }
            side = cross;
        }
    }
    if inside && side != 0.0 { 0.0 } else { best }
}

/// What the pass leaves of the radius, in cells: floors sit on a grid of cell centres, so a gap the body fits (a
/// door a few centimetres wider than its 0.76 m) can have no centre the radius clear of both sides, and taking
/// every floor nearer than the radius shuts it (Tarakanovsky Fort with the clients' numbers: 60k floors cut off
/// by one such passage, `a96_radii.txt`). A quarter of a cell is the most the grid's own quantisation can ask
/// for: nothing nearer than that is a floor the body stands on.
const EXACT_SLACK_CELLS: f32 = 0.25;

impl WalkGrid {
    /// How far a floor is kept from a wall by [`Erosion::Exact`], metres: the body's radius less
    /// [`EXACT_SLACK_CELLS`] of a cell.
    pub fn exact_clearance(&self) -> f32 {
        (self.body.radius - EXACT_SLACK_CELLS * self.cell).max(0.0)
    }

    /// Clears the `alive` floors that have a wall nearer than [`Self::exact_clearance`] ([`Walls::within`]); how many.
    pub(super) fn erode_exact(&self, alive: &mut [bool], walls: &Walls) -> usize {
        let radius = self.exact_clearance();
        let mut removed = 0;
        for a in 0..alive.len() {
            if !alive[a] {
                continue;
            }
            let (i, k) = self.cell_xz(self.cell_of[a] as usize);
            let (x, z) = (self.x0 + (i as f32 + 0.5) * self.cell, self.z0 + (k as f32 + 0.5) * self.cell);
            let feet = self.height[a];
            if walls.within(x, z, feet + self.body.step, feet + self.body.stand_height, radius) {
                alive[a] = false;
                removed += 1;
            }
        }
        removed
    }

    /// How many floors of the grid have a wall of `triangles` (the walls [`Erosion::Exact`] erodes by) nearer than
    /// `radius` metres: the audit of a grid, built either way. Also the nearest one, metres (`f32::MAX` when none
    /// is within `radius`).
    pub fn floors_near_walls(&self, triangles: &[Triangle], radius: f32) -> (usize, f32) {
        let walls = Walls::new(triangles, self.x0, self.z0, self.cell, self.nx, self.nz);
        let (mut near, mut nearest) = (0, f32::MAX);
        for node in 0..self.len() as NodeId {
            let p = self.position(node);
            if let Some(d) = walls.nearest(p.x, p.z, p.y + self.body.step, p.y + self.body.stand_height, radius) {
                nearest = nearest.min(d);
                near += usize::from(d < radius);
            }
        }
        (near, nearest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AREA: [f32; 4] = [0.0, 0.0, 10.0, 10.0];

    /// A horizontal quad of `walker` ground at height `y`.
    fn quad(x0: f32, z0: f32, x1: f32, z1: f32, y: f32, walker: WalkerBehaviour) -> [Triangle; 2] {
        let (a, b, c, d) = (Vec3::new(x0, y, z0), Vec3::new(x1, y, z0), Vec3::new(x1, y, z1), Vec3::new(x0, y, z1));
        [Triangle { v: [a, b, c], walker }, Triangle { v: [a, c, d], walker }]
    }

    fn floor(x0: f32, z0: f32, x1: f32, z1: f32, y: f32) -> [Triangle; 2] {
        quad(x0, z0, x1, z1, y, WalkerBehaviour::BySlope)
    }

    /// A post (a box `size` across, 2 m high) with its low corner at (`x`, `z`).
    fn post(x: f32, z: f32, size: f32) -> Vec<Triangle> {
        let (x1, z1) = (x + size, z + size);
        let face = |a: Vec3, b: Vec3| {
            let (c, d) = (Vec3::new(b.x, 2.0, b.z), Vec3::new(a.x, 2.0, a.z));
            [
                Triangle { v: [a, b, c], walker: WalkerBehaviour::BySlope },
                Triangle { v: [a, c, d], walker: WalkerBehaviour::BySlope },
            ]
        };
        let p = |x, z| Vec3::new(x, 0.0, z);
        let mut tris = Vec::new();
        for (a, b) in [(p(x, z), p(x1, z)), (p(x1, z), p(x1, z1)), (p(x1, z1), p(x, z1)), (p(x, z1), p(x, z))] {
            tris.extend(face(a, b));
        }
        tris
    }

    /// A post's corner leaves floors nearer than the radius to the cell rounds (the one diagonal to it 0.18 m
    /// from the corner); the exact pass takes every floor nearer than its clearance, and no other: the floor
    /// 0.375 m from a face (the radius less a hair) stays.
    #[test]
    fn the_exact_pass_takes_the_floors_nearer_a_post_than_its_clearance() {
        let mut tris = floor(0.0, 0.0, 10.0, 10.0, 0.0).to_vec();
        tris.extend(post(5.0, 5.0, 0.5));
        let build = |erosion| {
            let body = Body { erosion, ..Body::default() };
            WalkGrid::build(&tris, AREA, 0.25, body)
        };
        let (cells, exact) = (build(Erosion::Cells), build(Erosion::Exact));
        let clearance = exact.exact_clearance();
        assert!((clearance - (0.38 - 0.0625)).abs() < 1e-4, "{clearance}");
        let (under_cells, _) = cells.floors_near_walls(&tris, clearance);
        let (under_exact, _) = exact.floors_near_walls(&tris, clearance);
        assert!(under_cells > 0, "the rounds leave floors under the clearance from the post");
        assert_eq!(under_exact, 0);
        // The diagonal floor of the corner is gone, a floor 0.6 m from the post is not.
        let corner = Vec3::new(4.875, 0.0, 4.875);
        assert!(cells.locate(corner, 0.05).is_some());
        assert!(exact.locate(corner, 0.05).is_none());
        let far = Vec3::new(4.375, 0.0, 5.125);
        assert!(exact.locate(far, 0.05).is_some(), "floors outside the radius stay");
        let at_radius = Vec3::new(4.625, 0.0, 5.125);
        assert!(exact.locate(at_radius, 0.05).is_some(), "a floor 0.375 m from the face stays");
        assert!(exact.len() < cells.len());
    }

    /// Stairs (`walk` faces) and floors are no walls: a staircase beside a floor takes no floors.
    #[test]
    fn stairs_and_floors_are_no_walls() {
        let mut tris = floor(0.0, 0.0, 10.0, 10.0, 0.0).to_vec();
        for s in 0..5 {
            let x0 = 5.0 + 0.4 * s as f32;
            tris.extend(quad(x0, 0.0, x0 + 0.4, 10.0, 0.2 * (s + 1) as f32, WalkerBehaviour::Walk));
        }
        let walls = Walls::new(&tris, 0.0, 0.0, 0.25, 41, 41);
        assert!(walls.triangles.is_empty());
    }
}
