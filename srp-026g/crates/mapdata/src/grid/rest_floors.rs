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
//! - the client's walk tick rests a standing body there: come down onto its centre from [`PUT_OVER_M`] over it, out of the
//!   walls (a body arrives by a move), with no keys until it stands still on the ground (`on_ground` 0x7EF90; [`rests_at`]),
//!   its feet within the radius across and [`NAME_HEIGHT_M`] of the centre;
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
/// The most steps the rest test runs (2 s: A385's settle, `controller::jump`'s `SETTLE_STEPS_MAX`) …
const REST_STEPS_MAX: usize = 40;
/// … until the body has been on the ground with its feet moving under a millimetre a step for this many steps in a row (a body
/// sliding off a broken edge is still moving after 1 s: Chemical Plant's slab). Our
/// choice: the bound of the test, not a client value.
const STILL_STEPS: usize = 4;
/// The test body is put this far over the floor and comes down onto it, m (the stand feeds and the flyer's run-up,
/// `controller::jump`'s `RUN_UP_DROP_M`): a body arrives at a place by a move. Put at the floor's own point it may overlap faces
/// over it (a capsule at the height of Chemical Plant's broken slab is inside its steep faces) and both ticks keep that pose,
/// its steps skipped (exactness 2026-11-14, the Chemical Plant slab recheck: settled from
/// 0.55 m above, the exe and the port drop the body off all 7 slab floors; put on them, both hold 4). Less where the room over
/// the floor is shorter than a standing body and this.
const PUT_OVER_M: f32 = 0.6;
/// The body comes down at least this fast on every step, m/s, so no step is skipped (`can_skip_player_step` 0x7C7E0 skips a
/// body with no speed: [`super::wall_give_back`]; the flyer's settle sets the same).
const FALL_MPS: f32 = 0.01;

/// Where the client's walk tick rests a standing body come down onto `feet` (module documentation): put [`PUT_OVER_M`] over it
/// (at most `room` m: the floor's room over a standing body) out of the walls, it falls with no keys; its feet once it has stood
/// still on the ground [`STILL_STEPS`] steps in a row; None when it has not within [`REST_STEPS_MAX`] (it falls on, slides on).
pub(super) fn rests_at(world: &Collision, s: &Settings, feet: Vec3, room: f32) -> Option<Vec3> {
    let over = PUT_OVER_M.min(room).max(0.005);
    let start = out_of_walls(world, feet + Vec3::new(0.0, over, 0.0));
    let mut body = Controller::standing_at(start);
    let still = TickInput { walk: Vec3::ZERO, air_control: Vec3::ZERO, crouch: CrouchInput::Tested { key: false, legs_broken: false }, sprinting: false };
    let mut run = 0;
    for _ in 0..REST_STEPS_MAX {
        let before = body.feet();
        body.velocity.y = body.velocity.y.min(-FALL_MPS);
        body.tick(world, DT, s, still);
        run = if !body.jumping && body.on_ground(s) && (body.feet() - before).length() < 1e-3 { run + 1 } else { 0 };
        if run >= STILL_STEPS {
            return Some(body.feet());
        }
    }
    None
}

/// [A389] Whether the client's walk tick holds a standing body come down onto `feet` (as [`rests_at`] brings it, `room` the
/// floor's room over a standing body) on the ground there: in [`REST_STEPS_MAX`] steps with no keys, once it has landed it never
/// drops more than `step` under the ground it last stood on while off the ground (it falls off). A body sliding down a slope
/// steeper than standing still holds (`need_slide` 0x7BC20, from 40 deg) stays on the ground: held, the slope is walked
/// (Chemical Plant (33.18, 12.28, -14.32), a ramp 3 players stood on: it slides at 1 m/s, on the ground every step). Chemical
/// Plant's broken slab drops it 4-5 m to the ground: not held.
pub(super) fn holds(world: &Collision, s: &Settings, feet: Vec3, room: f32, step: f32) -> bool {
    let over = PUT_OVER_M.min(room).max(0.005);
    let start = out_of_walls(world, feet + Vec3::new(0.0, over, 0.0));
    let mut body = Controller::standing_at(start);
    let still = TickInput { walk: Vec3::ZERO, air_control: Vec3::ZERO, crouch: CrouchInput::Tested { key: false, legs_broken: false }, sprinting: false };
    let mut ground: Option<f32> = None;
    for _ in 0..REST_STEPS_MAX {
        body.velocity.y = body.velocity.y.min(-FALL_MPS);
        body.tick(world, DT, s, still);
        let y = body.feet().y;
        if !body.jumping && body.on_ground(s) {
            ground = Some(y);
        } else if let Some(g) = ground
            && g - y > step
        {
            return false;
        }
    }
    // Never landed: it fell the whole time.
    ground.is_some()
}

impl WalkGrid {
    /// Whether a floor `alive` keeps names `at` (within the capsule's radius across, [`NAME_HEIGHT_M`] in height).
    fn named(&self, alive: &[bool], at: Vec3) -> bool {
        let r = self.body.radius;
        self.floors_near(at, r).any(|f| alive[f as usize] && self.stand_gap(f, at.y) <= NAME_HEIGHT_M)
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
            rests_at(world, s, centre, self.ceiling[a] - self.height[a] - self.body.stand_height).is_some_and(|rest| {
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
