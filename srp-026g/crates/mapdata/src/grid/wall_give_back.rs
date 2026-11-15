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
//! - the client's own walk tick stands a body there ([`stands`]): set standing on the floor's own point (no push out of the
//!   walls; where it jams there, 0.05-0.10 m around it), with no keys, it comes down and rests within [`STAND_MOVED_M`] of where
//!   it was put across and of the floor in height (no slide, no fall, not jammed).
//!
//! It runs after the rounds (and the narrow doors), before their relinking and the exact pass, as the doors' pass does.

use super::controlled::{out_of_walls, parallel};
use super::rest_floors::Settle;
use super::exact::Walls;
use super::*;
use crate::controller::{Collision, Shape};
use crate::controller::tick::Settings;

/// How near a wall face must be for the round to be its doing, m.
pub const WALL_NEAR_M: f32 = 0.6;
/// How far the feet may move (or drop) in the stand test, m.
pub const STAND_MOVED_M: f32 = 0.1;
/// Whether the client's walk tick stands a body on `feet`: the own-point test ([`super::rest_floors::settle_near`]: a standing
/// body put 5 mm over the floor's own point, no push out of the walls, no keys; where it jams there, put again 0.05 then 0.10 m
/// around it), and it comes down and rests within [`STAND_MOVED_M`] of where it was put across and of the floor in height (no
/// slide, no fall, not jammed). The body is stepped by the grid's collision and settings (`world`, `s`: [`Body::controller`]).
///
/// [A364 C2 row 45] The body arrives with a speed, so its steps run: a body set standing with no speed and the contact normal up
/// is exactly the state `can_skip_player_step` 0x7C7E0 skips (the exactness rig: 2,160 of 2,160 steps skipped).
///
/// No push out of the walls: the exe sets a body where it is put. Pushed out of them, at the floor's height and again over it
/// (before, #1893), the body rested 0.25-0.40 m off the floor's point where the exe's body never goes, and a body jammed in faces,
/// which never moves, read as a rest: on 53 floors this pass brought back, the exactness rig (the exe's own tick on captures of
/// the running game) holds 35, jams 14 and drops 4; this test agrees on 52, the pushed one on 18 to 41
/// (rig/s7-20261114/own-point-all-20261116.tsv; nav-builder phase4/settle-20261116.txt). Of the floors the exe's body jams on at
/// their point, 9 of 10 rest from a put 0.05-0.10 m out (the exactness rig's ring, rig/s7-20261114/ring-give-back-20261116.txt):
/// the ring's floors stand.
///
/// Known gap: a floor where only a crouched body fits (a standing body jams there, a ledge at head height) is not given back: the
/// port's crouched body never stands still in the rest test's steps.
pub(super) fn stands(world: &Collision, s: &Settings, feet: Vec3) -> bool {
    let settled = super::rest_floors::settle_near(world, s, feet);
    let ok = match settled {
        // Across: no more than STAND_MOVED_M from the put (no slide). Down: no more than STAND_MOVED_M under the floor (no fall).
        // Up: a floor names a place within NAME_HEIGHT_M of its height (`rest_floors`).
        Settle::Rests { put, at } => {
            (at.x - put.x).hypot(at.z - put.z) < STAND_MOVED_M && feet.y - at.y < STAND_MOVED_M && at.y - feet.y <= super::rest_floors::NAME_HEIGHT_M
        }
        Settle::Jammed | Settle::Moving => false,
    };
    count_stand(feet, settled, ok);
    ok
}

/// The wall give-back's stand tests so far ([`stands`]), for the logs: no decision reads them. Totals of this process.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GiveBackStands {
    /// Stand tests run …
    pub tests: u64,
    /// … the body stood …
    pub stood: u64,
    /// … of them, from a ring put (jammed at the floor's point) …
    pub stood_on_ring: u64,
    /// … and of those that did not, jammed at the floor's point and at every ring put.
    pub jammed: u64,
}

static STAND_COUNTS: [std::sync::atomic::AtomicU64; 4] = [const { std::sync::atomic::AtomicU64::new(0) }; 4];

fn count_stand(feet: Vec3, settled: Settle, stood: bool) {
    use std::sync::atomic::Ordering::Relaxed;
    STAND_COUNTS[0].fetch_add(1, Relaxed);
    match settled {
        Settle::Rests { put, .. } if stood => {
            STAND_COUNTS[1].fetch_add(1, Relaxed);
            // The own point's put is straight over the floor's point; a ring put is aside.
            if put.x != feet.x || put.z != feet.z {
                STAND_COUNTS[2].fetch_add(1, Relaxed);
            }
        }
        Settle::Jammed => {
            STAND_COUNTS[3].fetch_add(1, Relaxed);
        }
        Settle::Rests { .. } | Settle::Moving => {}
    }
}

/// The process's [`GiveBackStands`] so far.
pub fn give_back_stands() -> GiveBackStands {
    let n = |i: usize| STAND_COUNTS[i].load(std::sync::atomic::Ordering::Relaxed);
    GiveBackStands { tests: n(0), stood: n(1), stood_on_ring: n(2), jammed: n(3) }
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
pub(super) fn stands_in_cell(world: &Collision, s: &Settings, centre: Vec3, half: f32) -> bool {
    let at = out_of_walls(world, centre);
    if (at.x - centre.x).abs() > half || (at.z - centre.z).abs() > half {
        return false;
    }
    let cyl = Shape::Cylinder { centre: at + Vec3::new(0.0, 1.3, 0.0), half: 0.5, radius: 0.38, margin: 0.04 };
    if world.nearest_to(cyl, 0.0, |f| f.normal.y.abs() < 0.5).is_some_and(|(gap, _, _)| gap < 0.0) {
        return false;
    }
    stands(world, s, at)
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
        let ok = parallel(&candidates, |&(a, judge)| match judge {
            Judge::Centre => stands(world, s, centre(a)),
            Judge::InCell => stands_in_cell(world, s, centre(a), half),
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
    /// and the exact pass (0.3175 m) keeps it: a candidate. A standing body (radius 0.38) set on its point overlaps the wall and
    /// never comes down (jammed); put 0.10 m away from the wall (the ring) it rests: it comes back. The one at 5.125 (0.095 m
    /// off) jams on the whole ring: it stays taken.
    #[test]
    fn a_floor_the_rounds_took_beside_a_wall_the_body_clears_comes_back() {
        use super::super::rest_floors::{settle, settle_near, Settle};
        let mut tris = ground();
        tris.extend(wall(5.22));
        let (off, on) = (floors(&tris, false), floors(&tris, true));
        let at = |g: &WalkGrid, x: f32| g.locate(Vec3::new(x, 0.0, 5.125), 0.05).is_some();
        assert!(!at(&off, 4.875), "the rounds take it");
        let (world, s) = (Collision::build(&tris), &Settings::LIVE);
        let feet = Vec3::new(4.875, 0.0, 5.125);
        assert_eq!(settle(&world, s, feet), Settle::Jammed, "0.345 m off the wall the standing body overlaps it");
        assert!(matches!(settle_near(&world, s, feet), Settle::Rests { put, .. } if put.x < feet.x - 0.09), "rests from the ring");
        assert!(at(&on, 4.875), "comes back");
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
        assert!(stands_in_cell(&Collision::build(&tris), &Settings::LIVE, Vec3::new(4.875, 0.0, 5.125), 0.125));
    }

    /// The own-point test, under a slab 1.24 m over the ground (a roof lower than a standing body, as Chemical Plant day_2's floors
    /// at (32.125, 12.414) and (28.625, 12.164), where the exe's body rests): the standing body set on the floor's point comes down
    /// and stands, never pushed out from under the slab; on open ground it stands; put on the slab's underside (a face pointing
    /// down, inside the solid) it does not.
    #[test]
    fn the_stand_test_sets_a_standing_body_on_the_floor_s_own_point() {
        use super::super::rest_floors::{settle, Settle};
        let mut tris = ground();
        // A slab over x 4.95..7 from 1.24 to 1.6 m.
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
        let s = &Settings::LIVE;
        let under = Vec3::new(5.6, 0.0, 5.0);
        assert_eq!(settle(&world, s, under), Settle::Rests { put: under + Vec3::new(0.0, 0.005, 0.0), at: under }, "the body comes down onto the floor's own point");
        assert!(stands(&world, s, under), "under the slab");
        let open = Vec3::new(2.0, 0.0, 5.0);
        assert!(stands(&world, s, open), "open ground: {:?}", settle(&world, s, open));
        let underside = Vec3::new(5.6, lo, 5.0);
        assert!(!stands(&world, s, underside), "on the slab's underside: {:?}", settle(&world, s, underside));
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
            assert!(stands(&world, &s, Vec3::new(4.0, 1.0, 5.0)), "on the top");
            assert!(stands(&world, &s, Vec3::new(7.0, 0.0, 5.0)), "on the ground");
            assert!(!stands(&world, &s, Vec3::new(7.0, 0.3, 5.0)), "0.3 m in the air");
            assert!(!stands(&world, &s, Vec3::new(5.35, 1.0, 5.0)), "0.35 m past the edge");
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
