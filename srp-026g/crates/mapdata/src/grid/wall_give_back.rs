//! [A318] The floors the erosion rounds took beside a wall the body clears ([`Body::wall_give_back`]), given back.
//!
//! The rounds ([`WalkGrid::erode`]) take a floor when a wall is anywhere in the cell beside it, up to a cell nearer than it is.
//! Along a wall that stands 0.32-0.5 m off a floor's centre the floor goes, though the exact pass's clearance
//! ([`WalkGrid::exact_clearance`]) keeps it and the client's body stands there. In the drift lab's grid holds (A318, the 15
//! client-test recordings) the rounds took the floor under the client in 376 held worlds; with the rounds skipped the exact pass
//! keeps 346 of them, their nearest wall face 0.3-0.5 m away.
//!
//! A taken floor comes back when all three hold:
//! - the exact pass would keep it (as [`super::doors`]: no wall within the clearance where the standing capsule meets walls);
//! - a wall face ([`Walls`]: steeper than the walk limit, not a rise of the ground) stands within [`WALL_NEAR_M`] at the body's
//!   height, so a real wall made the round take it (rising ground, which the old `round_keeps_exact` also gave back, 277k
//!   floors mostly of Rudnya's hillside, has none);
//! - the client's own walk tick stands a body there ([`stands`]): come down onto the floor from up to 0.6 m over it, out of the
//!   walls, with no keys, it rests within [`STAND_MOVED_M`] of it across and in height (no slide, no fall).
//!
//! It runs after the rounds (and the narrow doors), before their relinking and the exact pass, as the doors' pass does.

use super::controlled::{out_of_walls, parallel};
use super::exact::Walls;
use super::*;
use crate::controller::{Collision, Shape};
use crate::controller::tick::Settings;

/// How near a wall face must be for the round to be its doing, m.
pub const WALL_NEAR_M: f32 = 0.6;
/// How far the feet may move (or drop) in the stand test, m.
pub const STAND_MOVED_M: f32 = 0.1;
/// Whether the client's walk tick stands a body on `feet`: set there out of the walls, the body comes down onto it from up to
/// 0.6 m over it (`room`: the floor's room over a standing body) with no keys and rests ([`super::rest_floors::rests_at`]: still on
/// the ground 4 steps in a row within 2 s) within [`STAND_MOVED_M`] of it across and in height. The body is stepped by the grid's
/// collision and settings (`world`, `s`: [`Body::controller`]).
///
/// [A364 C2 row 45] The body arrives with a speed, so its steps run: a body set standing with no speed and the contact normal up
/// is exactly the state `can_skip_player_step` 0x7C7E0 skips (the exactness rig: 2,160 of 2,160 steps skipped). [A389 follow-up]
/// Set 5 mm over the floor's own point with one nudge (before), a capsule beside steep faces overlaps them and both ticks keep
/// that pose, its steps skipped from the second (exactness 2026-11-14, Chemical Plant's broken slab: settled from 0.55 m above,
/// the exe and the port drop the body off all 7 slab floors; put on them, both hold 4). Come down from over the floor as
/// `held_floors` and `rest_floors` bring it, the body lands with the contact it lands on, and on a rim `need_slide` 0x7BC20 slides
/// it off as the client's.
///
/// Across, the rest is measured from where the body was put ([`super::rest_floors::Rest::put`]: over the floor, pushed out of the
/// walls once). Measured from the feet pushed out of the walls first, a body under a roof lower than a standing body (still in
/// the walls after one push, so pushed 0.15 m again when put) read its own put as a slide and was refused where it never moved
/// (Chemical Plant day_2 (32.125, 5.657, 12.414) and (28.625, 5.756, 12.164), 1.24 and 1.28 m under the roof: 0.150 m "across"
/// with 0.000 of moving; the exe rests a body put at the players' feet there, the exactness rig 2026-11-15).
pub(super) fn stands(world: &Collision, s: &Settings, feet: Vec3, room: f32) -> bool {
    let rest = super::rest_floors::rest(world, s, feet, room);
    let ok = rest.is_some_and(|super::rest_floors::Rest { put, at }| {
        // Down: no more than STAND_MOVED_M (no slide, no fall). Up: a floor names a place within NAME_HEIGHT_M of its height
        // (`rest_floors`), and `out_of_walls` moves the start across a slope at the floor's height, so an honest rest may sit
        // a little higher than the floor's sampled height.
        (at.x - put.x).hypot(at.z - put.z) < STAND_MOVED_M && feet.y - at.y < STAND_MOVED_M && at.y - feet.y <= super::rest_floors::NAME_HEIGHT_M
    });
    count_stand(ok, rest.is_some_and(|r| (r.put.x - feet.x).hypot(r.put.z - feet.z) >= STAND_MOVED_M));
    ok
}

/// The wall give-back's stand tests so far ([`stands`]), for the logs: no decision reads them. Totals of this process.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GiveBackStands {
    /// Stand tests run.
    pub tests: u64,
    /// … the body stood …
    pub stood: u64,
    /// … and of those, put [`STAND_MOVED_M`] or more across from the floor's point by the push out of the walls: the stands a
    /// measure from the floor's point would have read as a slide.
    pub stood_put_aside: u64,
}

static STAND_COUNTS: [std::sync::atomic::AtomicU64; 3] = [const { std::sync::atomic::AtomicU64::new(0) }; 3];

fn count_stand(stood: bool, put_aside: bool) {
    use std::sync::atomic::Ordering::Relaxed;
    STAND_COUNTS[0].fetch_add(1, Relaxed);
    if stood {
        STAND_COUNTS[1].fetch_add(1, Relaxed);
        if put_aside {
            STAND_COUNTS[2].fetch_add(1, Relaxed);
        }
    }
}

/// The process's [`GiveBackStands`] so far.
pub fn give_back_stands() -> GiveBackStands {
    let n = |i: usize| STAND_COUNTS[i].load(std::sync::atomic::Ordering::Relaxed);
    GiveBackStands { tests: n(0), stood: n(1), stood_put_aside: n(2) }
}

/// How [`WalkGrid::give_back_by_walls`] judges a taken floor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Judge {
    /// The exact pass keeps it at its cell's centre: a body stands there ([`stands`]).
    Centre,
    /// The exact pass takes it at its centre ([`Body::give_back_in_cell`]): a body pushed out of the walls stands inside the
    /// cell ([`stands_in_cell`]).
    InCell,
}

/// [A331] Whether the client's walk tick stands a body somewhere in the cell of `centre` (half its side `half`): the body set at
/// the centre and pushed out of the walls as the client's world pushes it ([`out_of_walls`]: the standing cylinder with its
/// margin), clear of every wall there, inside the cell, and standing there ([`stands`]).
pub(super) fn stands_in_cell(world: &Collision, s: &Settings, centre: Vec3, half: f32, room: f32) -> bool {
    let at = out_of_walls(world, centre);
    if (at.x - centre.x).abs() > half || (at.z - centre.z).abs() > half {
        return false;
    }
    let cyl = Shape::Cylinder { centre: at + Vec3::new(0.0, 1.3, 0.0), half: 0.5, radius: 0.38, margin: 0.04 };
    if world.nearest_to(cyl, 0.0, |f| f.normal.y.abs() < 0.5).is_some_and(|(gap, _, _)| gap < 0.0) {
        return false;
    }
    stands(world, s, at, room)
}

impl WalkGrid {
    /// Gives back the floors the rounds took beside a wall the body clears (see the module); `alive` is the rounds' result.
    /// Returns how many.
    /// The stand test runs on `world`, the build's: the game's shapes where the build has them (`Collision::build_exe`, as the rest
    /// and held floors), else the walker triangles with the body's caster (as before).
    pub(super) fn give_back_by_walls(&self, alive: &mut [bool], walls: &Walls, world: &Collision, s: &Settings) -> usize {
        let n = alive.len();
        let stand = self.body.stand_height;
        let centre = |a: usize| {
            let (i, k) = self.cell_xz(self.cell_of[a] as usize);
            Vec3::new(self.x0 + (i as f32 + 0.5) * self.cell, self.height[a], self.z0 + (k as f32 + 0.5) * self.cell)
        };
        // Each candidate with how it is judged: at its centre (the exact pass keeps it) or anywhere in its cell
        // ([`Body::give_back_in_cell`]).
        let candidates: Vec<(usize, Judge)> = (0..n)
            .filter_map(|a| {
                if alive[a] {
                    return None;
                }
                let p = centre(a);
                let low = p.y + self.body.step;
                if !walls.within(p.x, p.z, low, p.y + stand, WALL_NEAR_M) {
                    return None;
                }
                let exact_keeps =
                    !walls.within(p.x, p.z, low, p.y + stand, self.exact_clearance()) || !self.capsule_meets(walls, p.x, p.z, p.y, stand);
                match (exact_keeps, self.body.give_back_in_cell) {
                    (true, _) => Some((a, Judge::Centre)),
                    (false, true) => Some((a, Judge::InCell)),
                    (false, false) => None,
                }
            })
            .collect();
        if candidates.is_empty() {
            return 0;
        }
        let half = self.cell / 2.0;
        // The room over a standing body: the test body comes down from at most this over the floor.
        let room = |a: usize| self.ceiling[a] - self.height[a] - stand;
        let ok = parallel(&candidates, |&(a, judge)| match judge {
            Judge::Centre => stands(world, s, centre(a), room(a)),
            Judge::InCell => stands_in_cell(world, s, centre(a), half, room(a)),
        });
        let mut back = 0;
        for (&(a, _), keep) in candidates.iter().zip(ok) {
            if keep {
                alive[a] = true;
                back += 1;
            }
        }
        back
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AREA: [f32; 4] = [0.0, 0.0, 10.0, 10.0];

    fn ground() -> Vec<Triangle> {
        let (a, b) = (Vec3::new(0.0, 0.0, 0.0), Vec3::new(10.0, 0.0, 10.0));
        let (c, d) = (Vec3::new(10.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 10.0));
        vec![Triangle { v: [a, c, b], walker: WalkerBehaviour::BySlope }, Triangle { v: [a, b, d], walker: WalkerBehaviour::BySlope }]
    }

    /// A 2 m wall face along x = `x` from z 1 to 9.
    fn wall(x: f32) -> Vec<Triangle> {
        let (a, b) = (Vec3::new(x, 0.0, 1.0), Vec3::new(x, 0.0, 9.0));
        let (c, d) = (Vec3::new(x, 2.0, 9.0), Vec3::new(x, 2.0, 1.0));
        vec![Triangle { v: [a, b, c], walker: WalkerBehaviour::BySlope }, Triangle { v: [a, c, d], walker: WalkerBehaviour::BySlope }]
    }

    fn floors(tris: &[Triangle], on: bool) -> WalkGrid {
        WalkGrid::build(tris, AREA, 0.25, Body { erosion: Erosion::Exact, wall_give_back: on, ..Body::default() })
    }

    /// A wall at x = 5.22: the floor centred at x 4.875 is 0.345 m off it, so the rounds take it (the wall is in the cell beside)
    /// and the exact pass (0.3175 m) keeps it: it comes back; the one at 5.125 (0.095 m off) stays taken; away from the wall the
    /// grid is the same.
    #[test]
    fn a_floor_the_rounds_took_beside_a_wall_the_body_clears_comes_back() {
        let mut tris = ground();
        tris.extend(wall(5.22));
        let (off, on) = (floors(&tris, false), floors(&tris, true));
        let at = |g: &WalkGrid, x: f32| g.locate(Vec3::new(x, 0.0, 5.125), 0.05).is_some();
        assert!(!at(&off, 4.875), "the rounds take it");
        assert!(at(&on, 4.875), "past the clearance comes back");
        assert!(!at(&on, 5.125), "under the clearance stays taken");
        let far = |g: &WalkGrid| g.floors_in(AREA).map(|n| g.position(n)).filter(|p| (p.x - 5.22).abs() > 1.0).count();
        assert_eq!(far(&on), far(&off), "no floor comes back away from the wall");
    }

    /// [A331] A wall at x = 5.175: the floor centred at x 4.875 is 0.30 m off it, nearer than the exact pass's 0.3175 m, so
    /// `wall_give_back` alone leaves it taken. The client's push out of walls sets the body 0.42 m off the wall, at x 4.755:
    /// inside the cell (x 4.75-5.0), so with `give_back_in_cell` it comes back. The floor at 5.375 (0.20 m off, on the far side)
    /// would be pushed to 5.595, out of its cell: it stays taken.
    #[test]
    fn a_cell_whose_far_half_the_body_stands_in_comes_back_with_give_back_in_cell() {
        let mut tris = ground();
        tris.extend(wall(5.175));
        let build = |in_cell: bool| {
            WalkGrid::build(&tris, AREA, 0.25, Body { erosion: Erosion::Exact, wall_give_back: true, give_back_in_cell: in_cell, ..Body::default() })
        };
        let (off, on) = (build(false), build(true));
        let at = |g: &WalkGrid, x: f32| g.locate(Vec3::new(x, 0.0, 5.125), 0.05).is_some();
        assert!(!at(&off, 4.875), "the centre is under the exact clearance");
        assert!(at(&on, 4.875), "the body stands in the cell's far half");
        assert!(!at(&on, 5.375), "pushed out of its cell: stays taken");
        assert!(stands_in_cell(&Collision::build(&tris), &Settings::LIVE, Vec3::new(4.875, 0.0, 5.125), 0.125, f32::INFINITY));
    }

    /// The push out of the walls moves a body at most `OUT_OF_WALLS_M` (0.15 m) at a time. Under a slab 1.24 m over the floor (a
    /// roof lower than a standing body, as Chemical Plant day_2's floors at (32.125, 12.414) and (28.625, 12.164)), set 0.07 m from
    /// its side face, one push leaves the standing cylinder (0.42 m with its margin) in the slab and the put pushes it again. The
    /// rest is measured from where the body was put: it never moves there and stands. Measured from the feet pushed first
    /// (before), the put's own second push read as a slide and the floor was refused (0.150 m "across" with 0.000 of moving there).
    #[test]
    fn the_stand_test_measures_the_rest_from_where_the_body_was_put() {
        let mut tris = ground();
        // A slab over x 4.95..7 from 1.24 to 1.6 m: its underside and its side face at x 4.95 (in the push's cylinder, 0.8-1.8 m).
        let t = |a: Vec3, b: Vec3, c: Vec3| Triangle { v: [a, b, c], walker: WalkerBehaviour::BySlope };
        let (lo, hi) = (1.24, 1.6);
        let p = |x: f32, y: f32, z: f32| Vec3::new(x, y, z);
        tris.extend([
            t(p(4.95, lo, 0.0), p(7.0, lo, 0.0), p(7.0, lo, 10.0)),
            t(p(4.95, lo, 0.0), p(7.0, lo, 10.0), p(4.95, lo, 10.0)),
            t(p(4.95, lo, 0.0), p(4.95, lo, 10.0), p(4.95, hi, 10.0)),
            t(p(4.95, lo, 0.0), p(4.95, hi, 10.0), p(4.95, hi, 0.0)),
            t(p(4.95, hi, 0.0), p(4.95, hi, 10.0), p(7.0, hi, 10.0)),
            t(p(4.95, hi, 0.0), p(7.0, hi, 10.0), p(7.0, hi, 0.0)),
        ]);
        let world = Collision::build(&tris);
        let (feet, room) = (Vec3::new(4.88, 0.0, 5.0), lo - 1.8);
        let first = out_of_walls(&world, feet);
        let again = out_of_walls(&world, first + Vec3::new(0.0, 0.005, 0.0));
        assert!((again.x - first.x).abs() >= STAND_MOVED_M, "one push leaves it in the slab: {first:?} -> {again:?}");
        let rest = super::super::rest_floors::rest(&world, &Settings::LIVE, feet, room).expect("rests");
        assert!((rest.at.x - rest.put.x).hypot(rest.at.z - rest.put.z) < 1e-3, "it never moves across: {rest:?}");
        let old = super::super::rest_floors::rests_at(&world, &Settings::LIVE, first, room).expect("rests");
        assert!((old.x - first.x).hypot(old.z - first.z) >= STAND_MOVED_M, "the old measure reads the second push as a slide: {old:?}");
        assert!(stands(&world, &Settings::LIVE, feet, room), "measured from where it was put, it stands");
    }

    /// [A364 C2 row 45] The stand test runs the client's steps: a body on flat ground stands, one in mid-air or with its centre
    /// well past a top's edge does not (before, every step was skipped and both passed).
    #[test]
    fn the_stand_test_refuses_mid_air_and_past_an_edge() {
        let mut tris = ground();
        // A 1 m top over x < 5 (its edge face at x 5).
        let top = |a: Vec3, b: Vec3, c: Vec3, d: Vec3| {
            [Triangle { v: [a, b, c], walker: WalkerBehaviour::BySlope }, Triangle { v: [a, c, d], walker: WalkerBehaviour::BySlope }]
        };
        tris.extend(top(Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 1.0, 10.0), Vec3::new(5.0, 1.0, 10.0), Vec3::new(5.0, 1.0, 0.0)));
        tris.extend(top(Vec3::new(5.0, 0.0, 0.0), Vec3::new(5.0, 1.0, 0.0), Vec3::new(5.0, 1.0, 10.0), Vec3::new(5.0, 0.0, 10.0)));
        for caster in [crate::controller::Caster::Conservative, Body::client().controller().0] {
            let world = Collision::build(&tris).with_caster(caster);
            let s = Body::client().controller().1;
            let room = f32::INFINITY;
            assert!(stands(&world, &s, Vec3::new(4.0, 1.0, 5.0), room), "on the top");
            assert!(stands(&world, &s, Vec3::new(7.0, 0.0, 5.0), room), "on the ground");
            assert!(!stands(&world, &s, Vec3::new(7.0, 0.3, 5.0), room), "0.3 m in the air");
            assert!(!stands(&world, &s, Vec3::new(5.35, 1.0, 5.0), room), "0.35 m past the edge");
        }
    }

    /// Off, the grid is the one without the pass.
    #[test]
    fn off_is_the_old_grid() {
        let mut tris = ground();
        tris.extend(wall(5.22));
        let a = floors(&tris, false);
        let b = WalkGrid::build(&tris, AREA, 0.25, Body { erosion: Erosion::Exact, ..Body::default() });
        assert_eq!(a.digest(), b.digest());
    }
}
