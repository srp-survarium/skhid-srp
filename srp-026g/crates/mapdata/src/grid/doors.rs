//! [A313] Narrow doors ([`Body::narrow_doors`]): the floors the cell erosion took in an opening the body fits, given back.
//!
//! The erosion rounds ([`WalkGrid::erode`]) take a floor when a wall is anywhere in the cell beside it, so up to a cell nearer
//! than it is. In a doorway 0.8-1.2 m wide every floor is beside a jamb's cell and goes, though the 0.76 m capsule passes: the
//! all-maps census (2026-11-05, the client's own walk tick through each opening both ways) found 438 openings of 0.6-1.1 m the
//! client walks through, 6 of them closed by the grid; this pass opens the 4 that are doors (Vector Laboratory's strip doorway
//! between two posts 0.90 m apart among them). Of the other two, one is narrower than the body by the exact pass's own rule
//! (0.75 m at the body's height) and one is a passage about 1.5 m wide the rounds took for another reason (a ledge side).
//!
//! The pass runs after the rounds, before their relinking and the exact pass: a floor the rounds took comes back when
//! - the exact pass would keep it (no wall nearer than [`WalkGrid::exact_clearance`] where the standing capsule meets walls),
//! - it lies across a way: walk links on two opposite sides to floors kept or given back,
//! - and the way is pinched: square to it, walls on both sides within [`NARROW_DOOR_M`] together (a door, not a wall's foot).
//!
//! So the floors along a single wall stay eroded, as before.

use super::exact::Walls;
use super::*;

/// The widest opening, wall to wall square to the way through it, whose floors are given back, metres.
pub const NARROW_DOOR_M: f32 = 1.2;
/// The step the sides are looked along, and how near a wall counts as met there, metres.
const SIDE_STEP_M: f32 = 0.025;
/// Each side is looked along a fan this wide either side of square to the way, in this many rays a half.
const SIDE_FAN_DEG: f32 = 60.0;
const SIDE_FAN_RAYS: i32 = 4;

impl WalkGrid {
    /// Gives back the floors of narrow doors the erosion rounds took (see the module); `alive` is the rounds' result. Returns how
    /// many.
    pub(super) fn open_narrow_doors(&self, wide: &Wide, alive: &mut [bool], walls: &Walls) -> usize {
        let n = alive.len();
        let centre = |a: usize| {
            let (i, k) = self.cell_xz(self.cell_of[a] as usize);
            Vec3::new(self.x0 + (i as f32 + 0.5) * self.cell, self.height[a], self.z0 + (k as f32 + 0.5) * self.cell)
        };
        let stand = self.body.stand_height;
        let candidate: Vec<bool> = (0..n)
            .map(|a| {
                if alive[a] {
                    return false;
                }
                let p = centre(a);
                !walls.within(p.x, p.z, p.y + self.body.step, p.y + stand, self.exact_clearance())
                    || !self.capsule_meets(walls, p.x, p.z, p.y, stand)
            })
            .collect();
        let open = |b: u32| b != NONE && (alive[b as usize] || candidate[b as usize]);
        // How far the nearest wall is from `p` on the side (`ux`, `uz`) points to, up to the door's width: the first wall along
        // each direction of a fan [`SIDE_FAN_DEG`] either side of it (a door across the grid's diagonal has its jambs off the
        // square line), the nearest of them.
        let side = |p: Vec3, ux: f32, uz: f32| {
            let steps = (NARROW_DOOR_M / SIDE_STEP_M) as usize;
            (-SIDE_FAN_RAYS..=SIDE_FAN_RAYS)
                .filter_map(|r| {
                    let (s, c) = (r as f32 * SIDE_FAN_DEG / SIDE_FAN_RAYS as f32).to_radians().sin_cos();
                    let (vx, vz) = (ux * c - uz * s, ux * s + uz * c);
                    (1..=steps).map(|s| s as f32 * SIDE_STEP_M).find(|&t| {
                        walls.within(p.x + vx * t, p.z + vz * t, p.y + self.body.step, p.y + stand, SIDE_STEP_M)
                    })
                })
                .min_by(f32::total_cmp)
        };
        let back: Vec<usize> = (0..n)
            .filter(|&a| {
                candidate[a]
                    && (0..8).step_by(2).any(|d| {
                        let o = d + 1;
                        let across = [d, o].iter().all(|&e| wide.kinds[a][e] == LinkKind::Walk && open(wide.links[a][e]));
                        if !across {
                            return false;
                        }
                        let (di, dk) = DIRS[d];
                        let length = ((di * di + dk * dk) as f32).sqrt();
                        let (px, pz) = (-dk as f32 / length, di as f32 / length);
                        let p = centre(a);
                        matches!((side(p, px, pz), side(p, -px, -pz)), (Some(l), Some(r)) if l + r <= NARROW_DOOR_M)
                    })
            })
            .collect();
        for &a in &back {
            alive[a] = true;
        }
        back.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AREA: [f32; 4] = [0.0, 0.0, 10.0, 10.0];

    /// A box `size` across and 2 m high with its low corner at (`x`, `z`), on ground at 0.
    fn post(x: f32, z: f32, size: f32) -> Vec<Triangle> {
        let (x1, z1) = (x + size, z + size);
        let p = |x, z| Vec3::new(x, 0.0, z);
        let mut tris = Vec::new();
        for (a, b) in [(p(x, z), p(x1, z)), (p(x1, z), p(x1, z1)), (p(x1, z1), p(x, z1)), (p(x, z1), p(x, z))] {
            let (c, d) = (Vec3::new(b.x, 2.0, b.z), Vec3::new(a.x, 2.0, a.z));
            tris.push(Triangle { v: [a, b, c], walker: WalkerBehaviour::BySlope });
            tris.push(Triangle { v: [a, c, d], walker: WalkerBehaviour::BySlope });
        }
        tris
    }

    /// A diagonal wall across the yard from corner to corner (0.15 m boxes corner to corner) with a doorway 0.85 m wide between
    /// its two posts at (5.15, 5.15) and (5.75, 5.75): the 0.76 m body fits; the cell rounds close it; the pass opens it and gives
    /// back no floor away from it.
    /// A wall across the yard from (1, 1) to (9, 9) (0.15 m boxes corner to corner) with no door, and a post standing alone: the
    /// floors the rounds took along them stay taken (no opening 1.2 m or less across a way), so the grid is the one without the
    /// pass.
    #[test]
    fn no_floor_comes_back_along_a_wall_or_by_a_post() {
        let (a, b) = (Vec3::new(0.0, 0.0, 0.0), Vec3::new(10.0, 0.0, 10.0));
        let (c, d) = (Vec3::new(10.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 10.0));
        let mut tris = vec![
            Triangle { v: [a, c, b], walker: WalkerBehaviour::BySlope },
            Triangle { v: [a, b, d], walker: WalkerBehaviour::BySlope },
        ];
        for i in 0..=53 {
            tris.extend(post(1.0 + 0.15 * i as f32, 1.0 + 0.15 * i as f32, 0.15));
        }
        tris.extend(post(7.0, 2.0, 0.3));
        let build = |narrow_doors| WalkGrid::build(&tris, AREA, 0.25, Body { erosion: Erosion::Exact, narrow_doors, ..Body::default() });
        let (off, on) = (build(false), build(true));
        let new: Vec<Vec3> = on.floors_in(AREA).map(|n| on.position(n)).filter(|&p| off.locate(p, 0.05).is_none()).collect();
        assert!(new.is_empty(), "floors given back: {new:?}");
        assert_eq!(on.digest(), off.digest());
    }
}
