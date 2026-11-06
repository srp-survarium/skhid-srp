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
//! - the client's own walk tick stands a body there ([`stands`]): set on the floor out of the walls, 1 s with no keys, the feet
//!   move under [`STAND_MOVED_M`] and drop under it (no slide, no fall).
//!
//! It runs after the rounds (and the narrow doors), before their relinking and the exact pass, as the doors' pass does.

use super::controlled::{out_of_walls, parallel};
use super::exact::Walls;
use super::*;
use crate::controller::Collision;
use crate::controller::tick::{Controller, Settings};

/// How near a wall face must be for the round to be its doing, m.
pub const WALL_NEAR_M: f32 = 0.6;
/// How far the feet may move (or drop) in the stand test, m.
const STAND_MOVED_M: f32 = 0.1;
/// The stand test's ticks (1 s at 30 a second).
const STAND_TICKS: usize = 30;

/// Whether the client's walk tick stands a body on `feet`: set there out of the walls, 1 s with no keys, the feet move and drop
/// under [`STAND_MOVED_M`].
pub(super) fn stands(world: &Collision, feet: Vec3) -> bool {
    let s = Settings::LIVE;
    let start = out_of_walls(world, feet);
    let mut body = Controller::standing_at(start);
    for _ in 0..STAND_TICKS {
        body.walk = Vec3::ZERO;
        body.step(world, 1.0 / 30.0, &s);
    }
    let end = body.feet();
    (end.x - start.x).hypot(end.z - start.z) < STAND_MOVED_M && start.y - end.y < STAND_MOVED_M
}

impl WalkGrid {
    /// Gives back the floors the rounds took beside a wall the body clears (see the module); `alive` is the rounds' result.
    /// Returns how many.
    pub(super) fn give_back_by_walls(&self, alive: &mut [bool], walls: &Walls, triangles: &[Triangle]) -> usize {
        let n = alive.len();
        let stand = self.body.stand_height;
        let centre = |a: usize| {
            let (i, k) = self.cell_xz(self.cell_of[a] as usize);
            Vec3::new(self.x0 + (i as f32 + 0.5) * self.cell, self.height[a], self.z0 + (k as f32 + 0.5) * self.cell)
        };
        let candidates: Vec<usize> = (0..n)
            .filter(|&a| {
                if alive[a] {
                    return false;
                }
                let p = centre(a);
                let low = p.y + self.body.step;
                let exact_keeps =
                    !walls.within(p.x, p.z, low, p.y + stand, self.exact_clearance()) || !self.capsule_meets(walls, p.x, p.z, p.y, stand);
                exact_keeps && walls.within(p.x, p.z, low, p.y + stand, WALL_NEAR_M)
            })
            .collect();
        if candidates.is_empty() {
            return 0;
        }
        let world = Collision::build(triangles);
        let ok = parallel(&candidates, |&a| stands(&world, centre(a)));
        let mut back = 0;
        for (&a, keep) in candidates.iter().zip(ok) {
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
