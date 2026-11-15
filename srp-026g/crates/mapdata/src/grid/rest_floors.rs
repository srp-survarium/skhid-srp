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

/// The own-point test's put over the floor, m: the exactness rig's (`held_spot_v2.py`, put 5 mm up).
const PUT_OWN_M: f32 = 0.005;
/// How far a body that rests must have come down from its put, m: a supported body settles the put's 5 mm (0 to 42 mm on the
/// exactness rig's 36 holding floors); a body jammed in faces never moves (the rig's 13 jammed floors stay at +0.005).
const CAME_DOWN_M: f32 = 0.001;

/// How the own-point test ended ([`settle`], [`settle_near`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Settle {
    /// Came down onto the ground and stood still there: where the body was put (5 mm over the floor's point, or a ring put
    /// around it) and its feet.
    Rests { put: Vec3, at: Vec3 },
    /// Stood still where it was put, never coming down: held by the faces it overlaps, not by a floor.
    Jammed,
    /// Never stood still (it falls, it slides on).
    Moving,
}

/// [the own-point stand test] Where the client's walk tick rests a standing body set on the floor's own point `feet`: put
/// [`PUT_OWN_M`] over it with no push out of the walls (the client's world sets a body where it is put: the exe never moves it
/// sideways), no keys, the rest test's steps. The exactness rig ran the exe's own tick so on 53 floors the wall give-back brought
/// back (rig/s7-20261114/own-point-all-20261116.tsv): it holds 36, jams 13 and drops 4 (on a slab's underside, or down a slope);
/// this test, standing, gives the same class on 52 (nav-builder phase4/settle-20261116.txt; the one other is a box's underside,
/// which the ground filter takes out). Put over the floor and pushed out of the walls (before), it agreed on 18 to 41 of them: a
/// pushed body rests 0.25-0.40 m from the floor's point, where the exe's body never goes.
///
/// Standing only: the port's crouched body never stands still in the rest test's steps (`Moving` on all 53), where the exe's
/// holds on most of them, so a crouched test would refuse every floor it ran on.
pub(super) fn settle(world: &Collision, s: &Settings, feet: Vec3) -> Settle {
    let put = feet + Vec3::new(0.0, PUT_OWN_M, 0.0);
    let mut body = Controller::standing_at(put);
    let input = TickInput { walk: Vec3::ZERO, air_control: Vec3::ZERO, crouch: CrouchInput::Tested { key: false, legs_broken: false }, sprinting: false };
    let mut run = 0;
    for _ in 0..REST_STEPS_MAX {
        let before = body.feet();
        body.velocity.y = body.velocity.y.min(-FALL_MPS);
        body.tick(world, DT, s, input);
        run = if !body.jumping && body.on_ground(s) && (body.feet() - before).length() < 1e-3 { run + 1 } else { 0 };
        if run >= STILL_STEPS {
            let at = body.feet();
            return if at.y < put.y - CAME_DOWN_M { Settle::Rests { put, at } } else { Settle::Jammed };
        }
    }
    Settle::Moving
}

/// The ring of [`settle_near`]: how far around a jammed point the body is put again, m, nearest first, and in how many ways:
/// the exactness rig's push-out ring (rig/s7-20261114/ring-give-back-20261116.txt: 16 ways at 0.05 m, then at 0.10 m), on
/// which 9 of 10 floors the exe's body jams on at their point rest from a put 0.05-0.10 m out. No further: the exe sets a body
/// where it is put, never 0.3 m out of the walls.
const RING_M: [f32; 2] = [0.05, 0.10];
const RING_WAYS: usize = 16;

/// [`settle`] at the floor's own point `feet`, and where the body jams there, at the ring's puts around it ([`RING_M`],
/// [`RING_WAYS`], nearest first) until it rests where the caller takes a rest (`takes(put, at)`): a floor whose own point is
/// closer to a face than a standing body fits stands where the body fits a few cm out, as the exe's ring finds. A ring put whose
/// body slides or drops past the caller's bounds is not the floor's place; the next is tried. A body that falls or slides on
/// from the floor's own point is not put again.
pub(super) fn settle_near(world: &Collision, s: &Settings, feet: Vec3, takes: impl Fn(Vec3, Vec3) -> bool) -> Settle {
    match settle(world, s, feet) {
        Settle::Jammed => RING_M
            .iter()
            .flat_map(|&r| {
                (0..RING_WAYS).map(move |k| {
                    let t = k as f32 * std::f32::consts::TAU / RING_WAYS as f32;
                    feet + Vec3::new(r * t.cos(), 0.0, r * t.sin())
                })
            })
            .map(|put| settle(world, s, put))
            .find(|rest| matches!(*rest, Settle::Rests { put, at } if takes(put, at)))
            .unwrap_or(Settle::Jammed),
        other => other,
    }
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
    // The fall is measured from the floor's own height until a step finds ground: a body that slides off on a steep way down with
    // no step finding ground on it (`step_down` 0x7E1B0 finds none there) lands 1.7-2.0 m lower and would set the ground there,
    // held (the exe review of #1914, Mamayev Kurgan (-22.851, 121.875): 1.73 m under the floor, counted held).
    let mut ground = feet.y;
    let mut landed = false;
    for _ in 0..REST_STEPS_MAX {
        body.velocity.y = body.velocity.y.min(-FALL_MPS);
        let notes = body.tick(world, DT, s, still);
        let y = body.feet().y;
        // On the ground only where the step's own down sweep found it (`step_down` 0x7E1B0): `on_ground` 0x7EF90 is the jump's
        // gate and stays true while a body walked off an edge falls its first 0.69 m (the exe's tick, a 2 m ledge).
        if !notes.airborne && !body.jumping && body.on_ground(s) {
            if ground - y > step {
                return false;
            }
            ground = y;
            landed = true;
        } else if ground - y > step {
            return false;
        }
    }
    // Never landed: it fell the whole time.
    landed
}

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
