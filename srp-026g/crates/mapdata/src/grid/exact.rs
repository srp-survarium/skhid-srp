//! Exact-distance erosion ([`Erosion::Exact`]).
//!
//! The erosion rounds of [`WalkGrid::erode`] work on cells: a floor goes when a wall is in a cell beside it
//! along a side, so a wall standing diagonally to a floor's cell (a post's corner) or a face at the far edge
//! of the next cell leaves floors nearer to it than the body's radius (A84 item 13: a floor 0.25 m from a post
//! in a 0.65 m opening; the body is 0.76 m wide). This pass runs after the rounds, over the floors they kept:
//! a floor goes when a wall triangle, clipped to the band the body's capsule meets walls in (from a step over
//! the feet to the standing height), comes nearer to the floor's centre than the radius less a quarter of a cell
//! ([`WalkGrid::exact_clearance`]), measured on the ground. The capsule is round at the top: above the cylinder
//! (the standing height less the radius) a wall is met at a smaller reach, [`WalkGrid::capsule_reach`].
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
        self.nearest_wall(x, z, low, high, limit).map(|(d, _)| d)
    }

    /// [`Self::nearest`] with the wall triangle it is.
    pub(super) fn nearest_wall(&self, x: f32, z: f32, low: f32, high: f32, limit: f32) -> Option<(f32, [Vec3; 3])> {
        let range = |at: f32, origin: f32, n: usize| {
            let first = (((at - limit - origin) / BIN_M).floor().max(0.0) as usize).min(n - 1);
            let last = (((at + limit - origin) / BIN_M).floor().max(0.0) as usize).min(n - 1);
            first..=last
        };
        let mut best: Option<(f32, [Vec3; 3])> = None;
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
                    if d <= limit && best.is_none_or(|b| d < b.0) {
                        best = Some((d, v));
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

/// The slabs the capsule's round top is cut into for the walls above its cylinder (each met at the reach of its lowest height).
const CAP_SLABS: usize = 32;

impl WalkGrid {
    /// How far a floor is kept from a wall by [`Erosion::Exact`], metres: the body's radius less
    /// [`EXACT_SLACK_CELLS`] of a cell.
    pub fn exact_clearance(&self) -> f32 {
        (self.body.radius - EXACT_SLACK_CELLS * self.cell).max(0.0)
    }

    /// How far from its axis the standing capsule is met by a wall at `above` metres over the cylinder's top (the standing height
    /// less the radius): the radius up to there, the round top's reach above (a circle: nothing at the standing height), less the
    /// grid's slack ([`EXACT_SLACK_CELLS`]); 0 or less where the wall is out of reach. The capsule is 0.76 m wide and 1.8 m tall
    /// with its centre 0.9 m over the feet (PROVEN, `default.player`), a capsule of that total height (LIKELY: the feet are the
    /// centre less half the height, `client_patch.md`, "A player's transform is his feet").
    pub fn capsule_reach(&self, above: f32) -> f32 {
        let r = self.body.radius;
        (r * r - above.max(0.0).powi(2)).max(0.0).sqrt() - EXACT_SLACK_CELLS * self.cell
    }

    /// Whether a wall of `walls` stops a capsule of `height` metres standing on the floor centre (`x`, `z`) with its feet at `feet`:
    /// one in the cylinder's band within [`Self::capsule_reach`] of the axis, or one in a slab of the round top within the slab's
    /// reach. Walls under a step over the feet are stepped over.
    fn capsule_meets(&self, walls: &Walls, x: f32, z: f32, feet: f32, height: f32) -> bool {
        let slab = self.body.radius / CAP_SLABS as f32;
        let cap_from = feet + height - self.body.radius;
        let step = feet + self.body.step;
        if cap_from > step && walls.within(x, z, step, cap_from, self.capsule_reach(0.0)) {
            return true;
        }
        (0..CAP_SLABS).any(|k| {
            let low = cap_from + k as f32 * slab;
            let reach = self.capsule_reach(k as f32 * slab);
            reach > 0.0 && low + slab > step && walls.within(x, z, low.max(step), low + slab, reach)
        })
    }

    /// Clears the `alive` floors that have a wall nearer than [`Self::exact_clearance`] ([`Walls::within`]) where the standing capsule
    /// meets it ([`Self::capsule_meets`]) and the crouched one does too; how many. A floor only the standing capsule meets a wall at
    /// (a pipe, an overhang between the crouched height and the standing one, which a player crouches under) stays, and is returned:
    /// its ceiling is lowered to the wall's reach so that only a crouched body walks it ([`WalkGrid::can_stand`]).
    pub(super) fn erode_exact(&self, alive: &mut [bool], walls: &Walls) -> (usize, Vec<usize>) {
        let radius = self.exact_clearance();
        let (mut removed, mut crouch_only) = (0, Vec::new());
        for a in 0..alive.len() {
            if !alive[a] {
                continue;
            }
            let (i, k) = self.cell_xz(self.cell_of[a] as usize);
            let (x, z) = (self.x0 + (i as f32 + 0.5) * self.cell, self.z0 + (k as f32 + 0.5) * self.cell);
            let feet = self.height[a];
            // None near in the whole band: the common case, one look.
            if !walls.within(x, z, feet + self.body.step, feet + self.body.stand_height, radius) {
                continue;
            }
            if !self.capsule_meets(walls, x, z, feet, self.body.stand_height) {
                continue;
            }
            if !self.body.crouch_only_floors || self.capsule_meets(walls, x, z, feet, self.body.crouch_height) {
                alive[a] = false;
                removed += 1;
            } else {
                crouch_only.push(a);
            }
        }
        (removed, crouch_only)
    }

    /// The wall nearest to the floor centre (`x`, `z`) at feet height `y`, among those within `limit` metres on the ground: its distance and
    /// triangle, for the audit of a neck ([`Erosion::Exact`] takes the floors nearer than [`Self::exact_clearance`]). Builds the walls of
    /// `triangles` for every call: for a few points, not a sweep.
    pub fn nearest_wall(&self, triangles: &[Triangle], x: f32, z: f32, y: f32, limit: f32) -> Option<(f32, [Vec3; 3])> {
        let walls = Walls::new(triangles, self.x0, self.z0, self.cell, self.nx, self.nz);
        walls.nearest_wall(x, z, y + self.body.step, y + self.body.stand_height, limit)
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

    /// A beam hanging at head height (its underside 1.7 m over the floor, a hand's breadth under the standing height) is met by the
    /// round top of the capsule at a smaller reach than a wall to the floor: the floor 0.225 m from its side stays (a body walks
    /// there, its cap clears the beam by the curve), where the same side as a wall from the floor takes it.
    #[test]
    fn a_beam_at_head_height_is_met_by_the_capsules_round_top() {
        let box_from = |y0: f32, y1: f32| {
            let mut tris = floor(0.0, 0.0, 10.0, 10.0, 0.0).to_vec();
            let (x0, x1) = (5.1f32, 6.0f32);
            for (a, b) in [((x0, 0.0), (x0, 10.0)), ((x1, 0.0), (x1, 10.0))] {
                let (p, q) = (Vec3::new(a.0, y0, a.1), Vec3::new(b.0, y0, b.1));
                let (r, t) = (Vec3::new(b.0, y1, b.1), Vec3::new(a.0, y1, a.1));
                tris.push(Triangle { v: [p, q, r], walker: WalkerBehaviour::BySlope });
                tris.push(Triangle { v: [p, r, t], walker: WalkerBehaviour::BySlope });
            }
            tris
        };
        let build = |tris: &[Triangle]| WalkGrid::build(tris, AREA, 0.25, Body { erosion: Erosion::Exact, ..Body::default() });
        let near = Vec3::new(4.875, 0.0, 5.125);
        let beam = build(&box_from(1.7, 2.4));
        assert!(beam.locate(near, 0.05).is_some(), "the floor 0.225 m from a beam at 1.7 m stays");
        let wall = build(&box_from(0.0, 2.4));
        assert!(wall.locate(near, 0.05).is_none(), "the same side from the floor takes it");
        // A beam from 1.3 m is in the standing capsule's cylinder but over the crouched one: the floor stays, to be walked crouched.
        let overhang = build(&box_from(1.3, 2.0));
        let node = overhang.locate(near, 0.05).expect("the floor under an overhang stays");
        assert!(!overhang.can_stand(node), "but only a crouched body fits");
        let plain = build(&box_from(0.0, 2.4));
        assert!(plain.locate(Vec3::new(2.125, 0.0, 5.125), 0.05).is_some_and(|n| plain.can_stand(n)), "a floor away from it stands");
        // A beam from 0.9 m is met by the crouched capsule too.
        let low_beam = build(&box_from(0.9, 1.6));
        assert!(low_beam.locate(near, 0.05).is_none(), "a beam in the crouched capsule's band takes it");
        // The reach: the radius (less the slack) up to the cylinder's top, none at the standing height.
        let grid = build(&box_from(1.7, 2.4));
        assert!((grid.capsule_reach(0.0) - grid.exact_clearance()).abs() < 1e-6);
        assert!(grid.capsule_reach(0.28) < grid.capsule_reach(0.0));
        assert!(grid.capsule_reach(0.38) <= 0.0);
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
