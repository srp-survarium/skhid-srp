//! How far each floor is from the nearest wall the body's capsule meets (A191b): the planner's waypoints and the corners of a
//! pulled route are floors, and the grid keeps its floors only [`WalkGrid::exact_clearance`] (0.3175 m) from a wall, less than the
//! capsule's radius 0.38 m (on purpose: the full radius cut 60k floors off Tarakanovsky Fort, `grid/exact.rs`). The table lets a
//! route keep its corners where the capsule fits.
//!
//! Derived data, made once after the grid is built or read ([`WalkGrid::with_wall_clearance`], from the map's triangles): in neither
//! the file nor the digest, so no floor, link or key changes. A grid without it answers `None`.
//!
//! The clearance of a floor is the radius plus how far the nearest wall is from touching the standing capsule there: a wall in the
//! cylinder's band at ground distance `d` gives `d`; a wall in the round top, at `h` metres over the cylinder's top and ground
//! distance `d`, touches at the reach `sqrt(r^2 - h^2)`, so it gives `d + r - reach`. So **a clearance of at least the radius means
//! the capsule fits** (no wall touches it), and 0.41 m is the controller's hold. One byte a floor, [`CLEARANCE_STEP_M`] a step,
//! up to [`CLEARANCE_MAX_M`].

use super::exact::Walls;
use super::*;

/// The table's resolution, metres, and what it reads at most (255 steps).
pub const CLEARANCE_STEP_M: f32 = 0.0025;
pub const CLEARANCE_MAX_M: f32 = 255.0 * CLEARANCE_STEP_M;

/// The slabs the round top is cut into (each met at the reach of its lowest height: a wall is taken a little nearer than it is, never
/// farther).
const TOP_SLABS: usize = 8;

impl WalkGrid {
    /// This grid with its table of wall clearances made from `triangles` (the triangles it was built from).
    pub fn with_wall_clearance(mut self, triangles: &[Triangle]) -> Self {
        let walls = Walls::new(triangles, self.x0, self.z0, self.cell, self.nx, self.nz);
        let table = (0..self.len()).map(|a| self.clearance_of(&walls, a)).collect();
        self.clearance = table;
        self
    }

    /// Whether the table is made.
    pub fn has_wall_clearance(&self) -> bool {
        self.clearance.len() == self.len() && !self.clearance.is_empty()
    }

    /// How far `node`'s floor is from the nearest wall the standing capsule meets, in the sense of the module: at least the body's
    /// radius where the capsule fits, [`CLEARANCE_MAX_M`] at most. `None` without the table.
    pub fn wall_clearance(&self, node: NodeId) -> Option<f32> {
        self.clearance.get(node as usize).map(|&c| f32::from(c) * CLEARANCE_STEP_M)
    }

    fn clearance_of(&self, walls: &Walls, a: usize) -> u8 {
        let (i, k) = self.cell_xz(self.cell_of[a] as usize);
        let (x, z) = (self.x0 + (i as f32 + 0.5) * self.cell, self.z0 + (k as f32 + 0.5) * self.cell);
        let feet = self.height[a];
        let (step, top) = (feet + self.body.step, feet + self.body.stand_height);
        let radius = self.body.radius;
        // No wall within the table's reach anywhere in the band: the common case, one look.
        if walls.nearest(x, z, step, top, CLEARANCE_MAX_M).is_none() {
            return 255;
        }
        let cap_from = top - radius;
        let mut best = CLEARANCE_MAX_M;
        if cap_from > step
            && let Some(d) = walls.nearest(x, z, step, cap_from, CLEARANCE_MAX_M)
        {
            best = best.min(d);
        }
        let slab = radius / TOP_SLABS as f32;
        for s in 0..TOP_SLABS {
            let above = s as f32 * slab;
            let low = (cap_from + above).max(step);
            let high = cap_from + above + slab;
            if high <= step {
                continue;
            }
            let reach = (radius * radius - above * above).max(0.0).sqrt();
            if let Some(d) = walls.nearest(x, z, low, high, CLEARANCE_MAX_M) {
                best = best.min(d + radius - reach);
            }
        }
        (best / CLEARANCE_STEP_M).round().clamp(0.0, 255.0) as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AREA: [f32; 4] = [0.0, 0.0, 10.0, 10.0];

    fn floor() -> Vec<Triangle> {
        let p = |x, z| Vec3::new(x, 0.0, z);
        let (a, b, c, d) = (p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0), p(0.0, 10.0));
        vec![
            Triangle { v: [a, b, c], walker: WalkerBehaviour::BySlope },
            Triangle { v: [a, c, d], walker: WalkerBehaviour::BySlope },
        ]
    }

    /// A wall plane across x = `x` from height `y0` to `y1`, all along z.
    fn wall(x: f32, y0: f32, y1: f32) -> [Triangle; 2] {
        let (p, q) = (Vec3::new(x, y0, 0.0), Vec3::new(x, y0, 10.0));
        let (r, t) = (Vec3::new(x, y1, 10.0), Vec3::new(x, y1, 0.0));
        [
            Triangle { v: [p, q, r], walker: WalkerBehaviour::BySlope },
            Triangle { v: [p, r, t], walker: WalkerBehaviour::BySlope },
        ]
    }

    fn grid(tris: &[Triangle]) -> WalkGrid {
        WalkGrid::build(tris, AREA, 0.25, Body { erosion: Erosion::Exact, ..Body::default() }).with_wall_clearance(tris)
    }

    fn at(grid: &WalkGrid, x: f32) -> Option<f32> {
        let node = grid.locate(Vec3::new(x, 0.0, 5.125), 0.05)?;
        grid.wall_clearance(node)
    }

    /// A wall from the floor: a floor's clearance is its distance to the wall; far from it, and with no wall at all, the table's
    /// most; a grid without the table says `None`.
    #[test]
    fn a_floor_wall_gives_the_distance() {
        let mut tris = floor();
        tris.extend(wall(5.0, 0.0, 2.4));
        let g = grid(&tris);
        assert!(g.has_wall_clearance());
        let near = at(&g, 4.625).unwrap();
        assert!((near - 0.375).abs() <= CLEARANCE_STEP_M + 1e-4, "{near}");
        let far = at(&g, 2.125).unwrap();
        assert!((far - CLEARANCE_MAX_M).abs() < 1e-4, "{far}");
        let plain = WalkGrid::build(&floor(), AREA, 0.25, Body::default());
        assert!(!plain.has_wall_clearance());
        assert_eq!(plain.wall_clearance(0), None);
    }

    /// A wall under the step height is stepped over (no clearance taken), and the capsule's round top meets a beam at a smaller reach:
    /// the same distance from a beam over the head is a larger clearance than from a wall from the floor.
    #[test]
    fn a_low_step_is_stepped_over_and_a_beam_at_head_height_is_met_by_the_round_top() {
        let mut low = floor();
        low.extend(wall(5.0, 0.0, 0.4));
        assert!((at(&grid(&low), 4.875).unwrap() - CLEARANCE_MAX_M).abs() < 1e-4, "a 0.4 m step is stepped over");
        let mut beam = floor();
        beam.extend(wall(5.0, 1.7, 2.4));
        let mut full = floor();
        full.extend(wall(5.0, 0.0, 2.4));
        // 0.125 m from the plane: the floor is taken by the full wall (its clearance under the radius) and kept under the beam.
        let (b, f) = (at(&grid(&beam), 4.875), at(&grid(&full), 4.875));
        assert!(b.unwrap() > 0.38, "under the beam the capsule fits by its round top: {b:?}");
        assert!(f.is_none(), "the floor 0.125 m from a wall from the floor is not a floor");
    }

    /// The table reads the same twice and is no part of the grid: floors and links are as without it.
    #[test]
    fn the_table_changes_no_floor() {
        let mut tris = floor();
        tris.extend(wall(5.0, 0.0, 2.4));
        let with = grid(&tris);
        let without = WalkGrid::build(&tris, AREA, 0.25, Body { erosion: Erosion::Exact, ..Body::default() });
        assert_eq!(with.len(), without.len());
        assert_eq!(with.digest(), without.digest());
    }
}
