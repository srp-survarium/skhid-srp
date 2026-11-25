//! [A389] The floors the erosion took where the client's body comes to rest and no other floor names the place
//! ([`super::Body::rest_floors`]).
//!
//! A floor is the grid's name for a place a body can be: A385's landings map the feet to the floor within the capsule's radius
//! across and [`NAME_HEIGHT_M`] in height (`WalkGrid::floor_under`). The erosion takes every floor whose centre is nearer a
//! wall than the capsule clears; along an ordinary wall the body set there is pushed onto the floors the erosion kept, so they
//! name it. But where the walkable surface is a sliver between faces too steep to stand on, the erosion takes every floor of it
//! and the place the body rests on has no name: the grid has no way onto it, though the client stands there.
//!
//! Mamayev Kurgan day 2, R2: a walking press at lead 1.0
//! rests at (-22.79, 31.65, 121.73) on a 0.3 m sliver of 43-44 deg faces between 60-80 deg ones; the grid without the erosion has
//! floors there (-22.601, 31.847, 121.625) and (-22.351, 31.970, 121.625), the rounds take both (the steep faces are within the
//! radius), and the give-back's stand test ([`super::wall_give_back`]) refuses them (a body set on the first rests at
//! (-22.71, 31.81, 121.74), 0.16 m off its centre, on the ground: not still). So the press lands where the grid has nothing.
//!
//! A taken floor comes back when all hold:
//! - no floor the erosion kept is within the capsule's radius across and [`NAME_HEIGHT_M`] of its centre;
//! - the client's walk tick rests a standing body there: set on its centre's own point (no push out of the walls, [`settle`]),
//!   with no keys, it comes down and stands still on the ground (`on_ground` 0x7EF90; a body jammed in faces, which never
//!   moves, is no rest), its feet within the radius across and [`NAME_HEIGHT_M`] of the centre;
//! - no floor the erosion kept is within the radius across and [`NAME_HEIGHT_M`] of where it rests either.
//!
//! It comes back as a floor like any other (its links as the cells made them), before the passes that read the floors (the gap
//! links, the rim jumps): a landing there is named.

use super::controlled::parallel;
use super::*;
use crate::controller::tick::Settings;
use crate::controller::Collision;
use crate::motion::rest::{settle_near, Settle};

/// How far in height a floor names a place, m: A385's landing's (`rim::LAND_HEIGHT_M`, the walk check's `AT_HEIGHT_M`).
pub const NAME_HEIGHT_M: f32 = 0.2;
impl WalkGrid {
    /// Whether a floor `alive` keeps names `at` (within the capsule's radius across, [`NAME_HEIGHT_M`] in height).
    fn named(&self, alive: &[bool], at: Vec3) -> bool {
        let r = self.body.radius;
        self.floors_near(at, r).any(|f| alive[f as usize] && self.stand_gap(f, at.y) <= NAME_HEIGHT_M)
    }

    /// The floors within `r` across of `at` (by their centres), any height.
    pub(super) fn floors_near(&self, at: Vec3, r: f32) -> impl Iterator<Item = NodeId> + '_ {
        let i = ((at.x - self.x0) / self.cell).floor() as i32;
        let k = ((at.z - self.z0) / self.cell).floor() as i32;
        let reach = (r / self.cell).ceil() as i32;
        (-reach..=reach)
            .flat_map(move |dk| (-reach..=reach).map(move |di| (i + di, k + dk)))
            .filter_map(|(i, k)| self.cell_index(i, k))
            .flat_map(|c| self.start[c]..self.start[c + 1])
            .filter(move |&f| self.position(f).distance_xz(at) <= r)
    }

    /// [`super::Body::rest_floors`]: the floors `alive` (the erosion's) leaves out that come back (module documentation).
    pub(super) fn rest_floors(&self, alive: &[bool], world: &Collision, s: &Settings) -> Vec<usize> {
        let r = self.body.radius;
        let candidates: Vec<usize> = (0..alive.len())
            .filter(|&a| !alive[a] && self.ceiling[a] - self.height[a] >= self.body.stand_height && !self.named(alive, self.position(a as NodeId)))
            .collect();
        // The own-point test with its ring ([`settle_near`]): every candidate has a standing body's room.
        let back = parallel(&candidates, |&a| {
            let centre = self.position(a as NodeId);
            let takes = |_put: Vec3, rest: Vec3| {
                rest.distance_xz(centre) <= r && (rest.y - centre.y).abs() <= NAME_HEIGHT_M && !self.named(alive, rest)
            };
            match settle_near(world, s, centre, takes) {
                Settle::Rests { put, at } => takes(put, at),
                Settle::Jammed | Settle::Moving => false,
            }
        });
        candidates.into_iter().zip(back).filter(|&(_, b)| b).map(|(a, _)| a).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::WalkerBehaviour;

    const AREA: [f32; 4] = [0.0, 0.0, 10.0, 10.0];

    fn tri(a: Vec3, b: Vec3, c: Vec3) -> Triangle {
        Triangle { v: [a, b, c], walker: WalkerBehaviour::BySlope }
    }

    /// Ground at 0, two 3 m walls along x at z 4.55 and z 5.45: a corridor 0.9 m wide, wider than the body (0.76 m and the collision
    /// margin 0.04 a side: 0.84 m; in 0.8 m a body come down from above is wedged between the walls).
    fn corridor() -> Vec<Triangle> {
        let g = |x: f32, z: f32| Vec3::new(x, 0.0, z);
        let mut t = vec![tri(g(0.0, 0.0), g(10.0, 0.0), g(10.0, 10.0)), tri(g(0.0, 0.0), g(10.0, 10.0), g(0.0, 10.0))];
        for z in [4.55, 5.45] {
            let (a, b, c, d) = (Vec3::new(0.0, 0.0, z), Vec3::new(10.0, 0.0, z), Vec3::new(10.0, 3.0, z), Vec3::new(0.0, 3.0, z));
            t.extend([tri(a, b, c), tri(a, c, d)]);
        }
        t
    }

    fn grid(on: bool) -> WalkGrid {
        WalkGrid::build(&corridor(), AREA, 0.25, Body { rest_floors: on, exe_collision: true, start_witness: true, ..Body::client() })
    }

    /// The rounds take every floor of the corridor (each is nearer a wall than the radius), and no floor outside the walls is
    /// within the radius of it; the client's body set there rests in the corridor: with the switch its floors come back, and
    /// away from the corridor the grid is the same.
    #[test]
    fn a_corridor_the_erosion_empties_gets_its_floors_back_where_the_body_rests() {
        let inside = |g: &WalkGrid| g.floors_in([2.0, 4.55, 8.0, 5.45]).filter(|&f| g.position(f).y.abs() < 0.1).count();
        let (off, on) = (grid(false), grid(true));
        assert_eq!(inside(&off), 0, "the rounds take the corridor");
        assert!(inside(&on) > 0, "the body rests there: its floors come back");
        let far = |g: &WalkGrid| g.floors_in(AREA).filter(|&f| (g.position(f).z - 5.0).abs() > 1.5).count();
        assert_eq!(far(&on), far(&off));
    }
}
