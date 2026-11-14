//! [A389] The floors the erosion took where the client's body comes to rest and no other floor names the place
//! ([`super::Body::rest_floors`]).
//!
//! A floor is the grid's name for a place a body can be: A385's landings map the feet to the floor within the capsule's radius
//! across and [`NAME_HEIGHT_M`] in height (`WalkGrid::floor_under`). The erosion takes every floor whose centre is nearer a
//! wall than the capsule clears; along an ordinary wall the body set there is pushed onto the floors the erosion kept, so they
//! name it. But where the walkable surface is a sliver between faces too steep to stand on, the erosion takes every floor of it
//! and the place the body rests on has no name: the grid has no way onto it, though the client stands there.
//!
//! Mamayev Kurgan day 2, owner-5's R2 (`traces-20261028/owner-5/a385-lost-links-client-truth.md`): a walking press at lead 1.0
//! rests at (-22.79, 31.65, 121.73) on a 0.3 m sliver of 43-44 deg faces between 60-80 deg ones; the grid without the erosion has
//! floors there (-22.601, 31.847, 121.625) and (-22.351, 31.970, 121.625), the rounds take both (the steep faces are within the
//! radius), and the give-back's stand test ([`super::wall_give_back`]) refuses them (a body set on the first rests at
//! (-22.71, 31.81, 121.74), 0.16 m off its centre, on the ground: not still). So the press lands where the grid has nothing.
//!
//! A taken floor comes back when all hold:
//! - no floor the erosion kept is within the capsule's radius across and [`NAME_HEIGHT_M`] of its centre;
//! - the client's walk tick rests a standing body there: set on its centre out of the walls, falling at 1 cm/s so its first
//!   step runs (as [`super::wall_give_back::stands`] sets it, A364 C2 row 45), 1 s with no keys, the body is on the ground
//!   (`on_ground` 0x7EF90) with its feet within the radius across and [`NAME_HEIGHT_M`] of the centre;
//! - no floor the erosion kept is within the radius across and [`NAME_HEIGHT_M`] of where it rests either.
//!
//! It comes back as a floor like any other (its links as the cells made them), before the passes that read the floors (the gap
//! links, the rim jumps): a landing there is named.

use super::controlled::{out_of_walls, parallel};
use super::*;
use crate::controller::tick::{Controller, CrouchInput, Settings, TickInput};
use crate::controller::Collision;

/// How far in height a floor names a place, m: A385's landing's (`rim::LAND_HEIGHT_M`, the walk check's `AT_HEIGHT_M`).
pub const NAME_HEIGHT_M: f32 = 0.2;
/// The client's step, s (0x9CC878; every step of the traces is 50 ms).
const DT: f32 = 0.05;
/// The rest test's steps (1 s of the client's, as the give-back's stand test).
const REST_STEPS: usize = 20;
/// The test body is set this far over the floor, m, falling at [`FALL_MPS`], so its first step runs (`can_skip_player_step`
/// 0x7C7E0 skips a body set with no speed: [`super::wall_give_back`]).
const LIFT_M: f32 = 0.005;
/// See [`LIFT_M`], m/s.
const FALL_MPS: f32 = 0.01;

/// Where the client's walk tick rests a standing body set at `feet` (module documentation), when it is on the ground then.
pub(super) fn rests_at(world: &Collision, s: &Settings, feet: Vec3) -> Option<Vec3> {
    let start = out_of_walls(world, feet);
    let mut body = Controller::standing_at(start + Vec3::new(0.0, LIFT_M, 0.0));
    body.velocity = Vec3::new(0.0, -FALL_MPS, 0.0);
    let still = TickInput { walk: Vec3::ZERO, air_control: Vec3::ZERO, crouch: CrouchInput::Tested { key: false, legs_broken: false }, sprinting: false };
    for _ in 0..REST_STEPS {
        body.tick(world, DT, s, still);
    }
    (!body.jumping && body.on_ground(s)).then(|| body.feet())
}

impl WalkGrid {
    /// Whether a floor `alive` keeps names `at` (within the capsule's radius across, [`NAME_HEIGHT_M`] in height).
    fn named(&self, alive: &[bool], at: Vec3) -> bool {
        let r = self.body.radius;
        self.floors_near(at, r).any(|f| alive[f as usize] && (self.height[f as usize] - at.y).abs() <= NAME_HEIGHT_M)
    }

    /// The floors within `r` across of `at` (by their centres), any height.
    fn floors_near(&self, at: Vec3, r: f32) -> impl Iterator<Item = NodeId> + '_ {
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
        let back = parallel(&candidates, |&a| {
            let centre = self.position(a as NodeId);
            rests_at(world, s, centre).is_some_and(|rest| {
                rest.distance_xz(centre) <= r && (rest.y - centre.y).abs() <= NAME_HEIGHT_M && !self.named(alive, rest)
            })
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

    /// Ground at 0, two 3 m walls along x at z 4.6 and z 5.4: a corridor 0.8 m wide, wider than the body (0.76 m).
    fn corridor() -> Vec<Triangle> {
        let g = |x: f32, z: f32| Vec3::new(x, 0.0, z);
        let mut t = vec![tri(g(0.0, 0.0), g(10.0, 0.0), g(10.0, 10.0)), tri(g(0.0, 0.0), g(10.0, 10.0), g(0.0, 10.0))];
        for z in [4.6, 5.4] {
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
        let inside = |g: &WalkGrid| g.floors_in([2.0, 4.6, 8.0, 5.4]).filter(|&f| g.position(f).y.abs() < 0.1).count();
        let (off, on) = (grid(false), grid(true));
        assert_eq!(inside(&off), 0, "the rounds take the corridor");
        assert!(inside(&on) > 0, "the body rests there: its floors come back");
        let far = |g: &WalkGrid| g.floors_in(AREA).filter(|&f| (g.position(f).z - 5.0).abs() > 1.5).count();
        assert_eq!(far(&on), far(&off));
    }
}
