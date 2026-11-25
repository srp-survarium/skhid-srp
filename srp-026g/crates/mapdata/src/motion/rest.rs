//! [body physics S6b] A body come to rest where a layer puts it, as the client's walk tick brings it down (the builder's rest
//! tests: `grid::rest_floors`, `grid::held_floors`, `grid::line_tops`, `grid::wall_give_back`).

use crate::controller::tick::{Controller, CrouchInput, Settings, TickInput};
use crate::controller::Collision;
use crate::grid::controlled::out_of_walls;
use crate::Vec3;

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
/// body with no speed: `grid::wall_give_back`; the flyer's settle sets the same).
const FALL_MPS: f32 = 0.01;

/// Where the client's walk tick rests a standing body come down onto `feet` (module documentation): put [`PUT_OVER_M`] over it
/// (at most `room` m: the floor's room over a standing body) out of the walls, it falls with no keys; its feet once it has stood
/// still on the ground [`STILL_STEPS`] steps in a row; None when it has not within [`REST_STEPS_MAX`] (it falls on, slides on).
fn rests_at(world: &Collision, s: &Settings, feet: Vec3, room: f32) -> Option<Vec3> {
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
pub(crate) enum Settle {
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
///
/// The put's downward speed ([`FALL_MPS`]) is given once, before the first tick, and the controller's own speed is left alone
/// after: a body at rest in the game has speed 0 and skips its steps (`can_skip_player_step` 0x7C7E0), so a speed forced before
/// every tick runs steps the game never runs. Forced so, the exactness rig's ring11 (own point) is lifted 0.8 m onto a ledge
/// under a ceiling on tick 2, in the exe as in the port, where the game's body lands on tick 1 on a rim 6.7 cm up and stays
/// (exactness, wsl-out/bench/exactness-20261115/put-trace/ROOT-CAUSE.md).
fn settle(world: &Collision, s: &Settings, feet: Vec3) -> Settle {
    let put = feet + Vec3::new(0.0, PUT_OWN_M, 0.0);
    let mut body = Controller::standing_at(put);
    let input = TickInput { walk: Vec3::ZERO, air_control: Vec3::ZERO, crouch: CrouchInput::Tested { key: false, legs_broken: false }, sprinting: false };
    let mut run = 0;
    body.velocity.y = -FALL_MPS;
    for _ in 0..REST_STEPS_MAX {
        let before = body.feet();
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
pub(crate) fn settle_near(world: &Collision, s: &Settings, feet: Vec3, takes: impl Fn(Vec3, Vec3) -> bool) -> Settle {
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
pub(crate) fn holds(world: &Collision, s: &Settings, feet: Vec3, room: f32, step: f32) -> bool {
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

/// [body physics S6b] Where the client's walk tick rests a standing body come down onto `feet` ([`rests_at`]).
pub(crate) fn rest_spot(world: &Collision, s: &Settings, feet: Vec3, room: f32) -> Option<Vec3> {
    rests_at(world, s, feet, room)
}

/// [body physics S6b] The own-point stand test at `feet` ([`settle`]).
pub(crate) fn settled_at(world: &Collision, s: &Settings, feet: Vec3) -> Settle {
    settle(world, s, feet)
}
