//! The open sides of the floors (A269): the edges a body walks off where the grid has no link. A side of a floor is open when no
//! link of the floor goes that way, the cell beside holds no floor within a step of its height, and no wall stands on the way out a
//! step over the floor (the walker's triangles, met by the capsule's path out to where its support at the edge gives). The grid
//! models such an edge as a wall (a body moves past its floor only toward a drop link or along a lip), but the client's capsule walks
//! on while its hemisphere still touches the edge and falls once it does not (`bullet_character_controller::step_down` 0x7E1B0):
//! a drop deeper than the harmless fall has no link (Vector Laboratory's 5.3 m pit edge), nor does a drop the grid leaves unlinked
//! (Tarakanovsky Fort's walkway over its shut pocket, 1.7 m).
//!
//! Derived data, made once after the grid is built or read ([`WalkGrid::with_open_sides`], from the map's triangles): in neither the
//! file nor the digest, so no floor, link or key changes. A grid without it answers `None`.

use super::exact::{WallFaces, Walls};
use super::*;

/// The bit of a floor's entry for "within the body's radius and [`FALL_EDGE_MARGIN_M`] of a floor with an open side, within a step of
/// its height" ([`WalkGrid::near_open_side`]); bits 0-7 are the open sides, by [`DIRS`].
const NEAR_OPEN: u16 = 1 << 8;
/// How near the way out a wall must come to stop the capsule there, metres: the stand-in's ray, a wall across the way.
const ACROSS_M: f32 = 0.05;
/// A wall this far over a step above the floor stops the capsule (the stand-in's ray height); lower, the body steps over it.
const OVER_STEP_M: f32 = 0.1;

impl WalkGrid {
    /// This grid with its table of open sides made from `triangles` (the triangles it was built from): a wall within `support_m` and
    /// the body's radius of a floor's centre on the way out holds the capsule there. [A318] With [`Body::drop_room_walk_faces`] a
    /// steep `walk` face is such a wall too, as for [`Body::drop_room`]: Level 7's deck parapet (16.15-16.30, 20.20-22.03, 1.09 m over
    /// the deck) has `walk` faces, and the table called the deck's side open where the client stands held (145 + 79 drifts in the
    /// drift lab with the reckoning's walk-off).
    pub fn with_open_sides(mut self, triangles: &[Triangle], support_m: f32) -> Self {
        let faces = if self.body.drop_room_walk_faces { WallFaces::AnyWalker } else { WallFaces::NotWalk };
        let walls = Walls::new_with(triangles, self.x0, self.z0, self.cell, self.nx, self.nz, faces);
        let mut table: Vec<u16> = (0..self.len()).map(|a| self.open_sides_of(&walls, a, support_m)).collect();
        let r = self.body.radius + FALL_EDGE_MARGIN_M;
        for a in 0..self.len() {
            if table[a] & 0xFF == 0 {
                continue;
            }
            let at = self.position(a as NodeId);
            let near: Vec<NodeId> = self
                .floors_in([at.x - r, at.z - r, at.x + r, at.z + r])
                .filter(|&b| {
                    let p = self.position(b);
                    p.distance_xz(at) <= r && (p.y - at.y).abs() <= self.body.step
                })
                .collect();
            for b in near {
                table[b as usize] |= NEAR_OPEN;
            }
        }
        self.open = table;
        self
    }

    /// Whether the table is made.
    pub fn has_open_sides(&self) -> bool {
        self.open.len() == self.len() && !self.open.is_empty()
    }

    /// Whether a body on `node` going the way (`dx`, `dz`) goes out over an open side of it (the side nearest that way); `None` without
    /// the table or with no way.
    pub fn open_toward(&self, node: NodeId, dx: f32, dz: f32) -> Option<bool> {
        let bits = *self.open.get(node as usize)?;
        let length = dx.hypot(dz);
        if length < 1e-6 {
            return None;
        }
        let d = (0..DIRS.len()).max_by(|&a, &b| {
            let along = |d: usize| {
                let (i, k) = DIRS[d];
                (i as f32 * dx + k as f32 * dz) / (((i * i + k * k) as f32).sqrt() * length)
            };
            along(a).total_cmp(&along(b))
        })?;
        Some(bits & (1 << d) != 0)
    }

    /// Whether `node` is within the body's radius and [`FALL_EDGE_MARGIN_M`] of a floor with an open side: false without the table.
    pub fn near_open_side(&self, node: NodeId) -> bool {
        self.open.get(node as usize).is_some_and(|&bits| bits & NEAR_OPEN != 0)
    }

    fn open_sides_of(&self, walls: &Walls, a: usize, support_m: f32) -> u16 {
        let (i, k) = self.cell_xz(self.cell_of[a] as usize);
        let feet = self.height[a];
        let centre = Vec3::new(self.x0 + (i as f32 + 0.5) * self.cell, feet, self.z0 + (k as f32 + 0.5) * self.cell);
        let (low, high) = (feet + self.body.step + OVER_STEP_M, feet + self.body.stand_height);
        let mut bits = 0u16;
        for (d, &(di, dk)) in DIRS.iter().enumerate() {
            if self.to[a][d] != NO_LINK {
                continue;
            }
            // The map area's edge is a wall; a floor of its level beside is no edge.
            let Some(cell) = self.cell_index(i + di, k + dk) else {
                continue;
            };
            if (self.start[cell]..self.start[cell + 1]).any(|b| (self.height[b as usize] - feet).abs() <= self.body.step) {
                continue;
            }
            let length = ((di * di + dk * dk) as f32).sqrt();
            let reach = support_m + self.body.radius;
            // A269c: only a side over a floor further down than a step is a drop the capsule is known to walk off. The cells out
            // along the side within the reach: the first that holds a floor within a step of the feet (the ground goes on past a
            // cell the grid eroded by a wall off the way out) makes it no drop, the first with one further down makes it one (a
            // cliff's foot is eroded by its face: the floor below starts a cell or two out); cells with only higher floors (a rise
            // over a step, a floor overhead) or none (ground too steep for a floor) all the way are no known drop: the body is held
            // there as at a wall, as the default model holds it. A269's table took any cell beside with no floor of the feet's level
            // for a drop: most of the open sides with no route cost on the 8 maps had no floor below beside (the bench's
            // open_sides_bare), and the lip guard braked at them a key change a tick.
            if !self.drop_along(i, k, di, dk, feet, (reach / (self.cell * length)).floor() as i32) {
                continue;
            }
            let out = Vec3::new(centre.x + di as f32 / length * reach, feet, centre.z + dk as f32 / length * reach);
            if walls.nearest_ahead(centre, out, low, high, ACROSS_M).is_some() {
                continue;
            }
            bits |= 1 << d;
        }
        bits
    }

    /// Whether the cells `1..=cells` out from (`i`, `k`) along (`di`, `dk`) come to a floor further down than a step from `feet` before
    /// one within a step of it ([`Self::open_sides_of`]); the map area's edge ends the look with none.
    fn drop_along(&self, i: i32, k: i32, di: i32, dk: i32, feet: f32, cells: i32) -> bool {
        for m in 1..=cells.max(1) {
            let Some(cell) = self.cell_index(i + m * di, k + m * dk) else {
                return false;
            };
            let heights = || (self.start[cell]..self.start[cell + 1]).map(|b| self.height[b as usize]);
            if heights().any(|h| (h - feet).abs() <= self.body.step) {
                return false;
            }
            if heights().any(|h| h < feet - self.body.step) {
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AREA: [f32; 4] = [0.0, 0.0, 12.0, 10.0];

    fn quad(x0: f32, x1: f32, y: f32, z0: f32, z1: f32) -> [Triangle; 2] {
        let p = |x, z| Vec3::new(x, y, z);
        let (a, b, c, d) = (p(x0, z0), p(x1, z0), p(x1, z1), p(x0, z1));
        [Triangle { v: [a, b, c], walker: WalkerBehaviour::BySlope }, Triangle { v: [a, c, d], walker: WalkerBehaviour::BySlope }]
    }

    /// A wall plane across x = `x` from height `y0` to `y1`, along z from `z0` to `z1`.
    fn wall(x: f32, y0: f32, y1: f32, z0: f32, z1: f32) -> [Triangle; 2] {
        let (p, q) = (Vec3::new(x, y0, z0), Vec3::new(x, y0, z1));
        let (r, t) = (Vec3::new(x, y1, z1), Vec3::new(x, y1, z0));
        [Triangle { v: [p, q, r], walker: WalkerBehaviour::BySlope }, Triangle { v: [p, r, t], walker: WalkerBehaviour::BySlope }]
    }

    /// The last floor of a ledge's level going +x, at z 5.125, and the ledge's top.
    fn edge(grid: &WalkGrid, top: f32) -> NodeId {
        let row: Vec<NodeId> = grid.floors_in([0.0, 5.0, 12.0, 5.25]).filter(|&n| (grid.position(n).y - top).abs() < 0.05).collect();
        row.into_iter().max_by(|&a, &b| grid.position(a).x.total_cmp(&grid.position(b).x)).expect("a floor on the ledge")
    }

    /// A ledge 6 m over the ground (deeper than the harmless drop: no link down) is open on its edge, the way off it, and the floors a
    /// body's width in are near it; a parapet along the edge closes it; a grid without the table answers `None` and nothing is near.
    #[test]
    fn a_deep_ledge_with_no_link_is_open_on_its_edge_and_a_parapet_closes_it() {
        let mut tris: Vec<Triangle> = quad(0.0, 12.0, 0.0, 0.0, 10.0).to_vec();
        tris.extend(quad(0.0, 6.0, 6.0, 0.0, 10.0));
        tris.extend(wall(6.0, 0.0, 6.0, 0.0, 10.0));
        let body = Body { erosion: Erosion::Exact, ..Body::default() };
        let grid = WalkGrid::build(&tris, AREA, 0.25, body).with_open_sides(&tris, 0.435);
        assert!(grid.has_open_sides());
        let top = edge(&grid, 6.0);
        assert!(grid.links(top).all(|(b, _)| grid.position(b).x <= grid.position(top).x + 1e-3), "no link off the 6 m edge");
        assert_eq!(grid.open_toward(top, 1.0, 0.0), Some(true), "off the edge");
        assert_eq!(grid.open_toward(top, -1.0, 0.0), Some(false), "back onto the ledge");
        assert!(grid.near_open_side(top));
        let inside = grid.locate(Vec3::new(grid.position(top).x - 0.5, 6.0, 5.125), 0.05).expect("a floor 0.5 m in");
        assert!(grid.near_open_side(inside), "a body's width in");
        let far = grid.locate(Vec3::new(1.125, 6.0, 5.125), 0.05).expect("a floor far in");
        assert!(!grid.near_open_side(far));

        tris.extend(wall(6.05, 6.0, 7.0, 0.0, 10.0));
        let walled = WalkGrid::build(&tris, AREA, 0.25, body).with_open_sides(&tris, 0.435);
        assert_eq!(walled.open_toward(edge(&walled, 6.0), 1.0, 0.0), Some(false), "a parapet 1 m high holds the capsule");

        let bare = WalkGrid::build(&tris, AREA, 0.25, body);
        assert_eq!(bare.open_toward(edge(&bare, 6.0), 1.0, 0.0), None);
        assert!(!bare.near_open_side(edge(&bare, 6.0)));
    }

    /// [A318] A parapet with `walk` faces (Level 7's deck) holds the capsule as any wall does with [`Body::drop_room_walk_faces`];
    /// without it the table takes it for no wall, as it did.
    #[test]
    fn a_parapet_with_walk_faces_closes_the_edge_with_drop_room_walk_faces() {
        let mut tris: Vec<Triangle> = quad(0.0, 12.0, 0.0, 0.0, 10.0).to_vec();
        // An overhanging ledge (no face to erode the ground at its foot), so the drop is seen in the cell beside, as on Level 7's deck.
        tris.extend(quad(0.0, 6.0, 6.0, 0.0, 10.0));
        // The parapet within the capsule's reach of the last floor's centre, from the ledge's height to 1 m over it.
        tris.extend(wall(6.6, 6.0, 7.0, 0.0, 10.0).map(|t| Triangle { walker: WalkerBehaviour::Walk, ..t }));
        let open_with = |walk_faces: bool| {
            let body = Body { erosion: Erosion::Exact, drop_room_walk_faces: walk_faces, ..Body::default() };
            let grid = WalkGrid::build(&tris, AREA, 0.25, body).with_open_sides(&tris, 0.435);
            grid.open_toward(edge(&grid, 6.0), 1.0, 0.0)
        };
        assert_eq!(open_with(true), Some(false), "the walk-faced parapet holds the capsule");
        assert_eq!(open_with(false), Some(true), "as it was: no wall");
    }

    /// A269c: a side is open only over a floor further down than a step. On level ground a wall along the way out, a little to its
    /// side (z 5.4 from x 6.1 on), erodes the cells out from a floor (no floor there, no link that way, and the way out misses the
    /// wall): the ground is there, the capsule walks on, and the side is not open. The 6 m ledge's edge stays open over the ground
    /// though its face erodes the cell at its foot (`a_deep_ledge_with_no_link_is_open_on_its_edge_and_a_parapet_closes_it`).
    #[test]
    fn cells_eroded_by_a_wall_beside_the_way_are_no_open_side() {
        let body = Body { erosion: Erosion::Exact, ..Body::default() };
        let mut tris: Vec<Triangle> = quad(0.0, 12.0, 0.0, 0.0, 10.0).to_vec();
        let p = |x: f32, y: f32| Vec3::new(x, y, 5.4);
        let (a, b, c, d) = (p(6.1, 0.0), p(7.5, 0.0), p(7.5, 3.0), p(6.1, 3.0));
        tris.push(Triangle { v: [a, b, c], walker: WalkerBehaviour::BySlope });
        tris.push(Triangle { v: [a, c, d], walker: WalkerBehaviour::BySlope });
        let grid = WalkGrid::build(&tris, AREA, 0.25, body).with_open_sides(&tris, 0.435);
        let floor = grid.locate(Vec3::new(5.875, 0.0, 5.125), 0.05).expect("a floor short of the wall");
        assert_eq!(grid.position(floor).x, 5.875);
        assert_eq!(grid.floors_in([6.01, 5.01, 6.24, 5.24]).count(), 0, "the cell beside is eroded");
        let east = |b: NodeId| grid.position(b).x > 6.0 && (grid.position(b).z - 5.125).abs() < 0.01;
        assert!(grid.links(floor).all(|(b, _)| !east(b)), "no link that way");
        assert_eq!(grid.open_toward(floor, 1.0, 0.0), Some(false), "ground along the way, not a drop");
    }
}
