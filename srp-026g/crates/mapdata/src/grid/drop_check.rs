//! [A313] The Drop links the client's own walk tick does not take a body down ([`super::Body::drop_check`]).
//!
//! The cells see a Drop where a floor ends and a lower one lies a cell on. The client's capsule does not always go there:
//! - it stands on past the take-off cell on the rim the erosion took from the grid, or on a ledge between the floors that the
//!   grid does not have (the body stays above the landing);
//! - or it slides past the landing and comes down on lower ground (the body ends under it).
//!
//! The drops census (A313, every Drop link of the 16 map/day projects of rev-48 run through `controller::tick`): 82 % come down
//! on the landing; of the rest, 31,196 stay above it, 36,829 on the take-off's level and 20,939 end under it. Live bots stalled
//! at 2,588 of the 5,527 edge stall samples of A311 within 1 m of a failing link's end (level_06 the most).
//!
//! Each Drop link is walked as a bot walks it: the body set on the take-off, steered at the landing at [`WALK_MPS`], then on
//! along the link up to [`WALK_ON_M`] past it (a bot walks on to its next node), then left to settle. The link passes when the
//! feet come within [`REACH_M`] across and [`LAND_DY_M`] in height of the landing on the way, or end within [`LAND_DY_M`] in
//! height of it.

use super::{DIRS, LinkKind, NONE, SEED_M, SPAWN_M, Triangle, Vec3, WalkGrid, Wide};
use super::controlled::{STARTS, out_of_walls, parallel};
use crate::controller::Collision;
use crate::controller::tick::{Controller, Settings};

/// The tick, s (the census's: 30 a second).
const DT: f32 = 1.0 / 30.0;
/// A walking body's speed, m/s.
const WALK_MPS: f32 = 3.5;
/// How near the landing across the feet pass on the way, m.
const REACH_M: f32 = 0.35;
/// How near the landing's height the feet must be, m.
const LAND_DY_M: f32 = 0.4;
/// How far past the landing across the body walks on along the link, m.
const WALK_ON_M: f32 = 1.5;
/// The ticks of each phase: steering at the landing (3 s), walking on (2 s), settling (1 s).
const STEER_TICKS: usize = 90;
const ON_TICKS: usize = 60;
const SETTLE_TICKS: usize = 30;

impl WalkGrid {
    /// The Drop links of `wide` the client's walk tick does not take a body down, as (take-off, direction, rank 0). Only those
    /// out of a floor a body gets to from the seeds or a spawn are walked: a link out of any other floor is never on a route.
    /// The links are walked on the machine's threads; the result is in link order whatever the threads.
    pub(super) fn unfollowed_drops(&self, wide: &Wide, triangles: &[Triangle], seeds: &[Vec3], spawns: &[Vec3]) -> Vec<(u32, usize, f32)> {
        let n = self.height.len();
        let mut reached = vec![false; n];
        let mut queue: Vec<u32> = seeds
            .iter()
            .filter_map(|&p| self.locate(p, SEED_M))
            .chain(spawns.iter().filter_map(|&p| self.locate(p, SPAWN_M)))
            .collect();
        for &a in &queue {
            reached[a as usize] = true;
        }
        while let Some(a) = queue.pop() {
            for b in wide.out(a as usize).chain(wide.slips_from(a)) {
                if !reached[b as usize] {
                    reached[b as usize] = true;
                    queue.push(b);
                }
            }
        }
        let links: Vec<(u32, usize, Vec3, Vec3)> = (0..n)
            .filter(|&a| reached[a])
            .flat_map(|a| {
                (0..DIRS.len()).filter_map(move |d| {
                    let b = wide.links[a][d];
                    (b != NONE && wide.kinds[a][d] == LinkKind::Drop).then(|| (a as u32, d, self.position(a as u32), self.position(b)))
                })
            })
            .collect();
        if links.is_empty() {
            return Vec::new();
        }
        let world = Collision::build(triangles);
        let s = Settings::LIVE;
        let follow = parallel(&links, |l| follows(&world, &s, l.2, l.3));
        links.iter().zip(follow).filter(|(_, ok)| !ok).map(|(l, _)| (l.0, l.1, 0.0)).collect()
    }
}

fn flat(a: Vec3, b: Vec3) -> f32 {
    (a.x - b.x).hypot(a.z - b.z)
}

/// Whether the client's walk tick takes a body walking off `from` down to `to` (module documentation): on any of the
/// controller pass's [`STARTS`] lines (across the link), each start moved out of the walls the body would overlap there
/// ([`out_of_walls`]: the grid keeps floors 0.3175 m from a wall, the body's move test 0.42 m). A link is unfollowed only when
/// every line fails, so one line's corner does not decide it (the controller pass's rule, learnt on Cologne's mossy log).
pub(super) fn follows(world: &Collision, s: &Settings, from: Vec3, to: Vec3) -> bool {
    let len = flat(from, to).max(1e-3);
    let side = Vec3::new(-(to.z - from.z) / len, 0.0, (to.x - from.x) / len);
    STARTS.iter().any(|&off| follows_line(world, s, out_of_walls(world, from + side * off), to + side * off))
}

/// One line of [`follows`]: the body set standing on `from`, steered at `to`, then on along the line.
fn follows_line(world: &Collision, s: &Settings, from: Vec3, to: Vec3) -> bool {
    let mut body = Controller::standing_at(from);
    let landed = |feet: Vec3| (feet.y - to.y).abs() <= LAND_DY_M;
    // Steered at the landing; over it, on to the walk.
    for _ in 0..STEER_TICKS {
        let feet = body.feet();
        let d = flat(feet, to);
        if d <= REACH_M && landed(feet) {
            return true;
        }
        if d <= 0.05 {
            break;
        }
        let step = (WALK_MPS * DT).min(d);
        body.walk = Vec3::new((to.x - feet.x) / d * step, 0.0, (to.z - feet.z) / d * step);
        body.step(world, DT, s);
    }
    // On along the link while the body is above the landing.
    let len = flat(from, to).max(1e-3);
    let (ux, uz) = ((to.x - from.x) / len, (to.z - from.z) / len);
    for _ in 0..ON_TICKS {
        let feet = body.feet();
        if feet.y <= to.y + LAND_DY_M || flat(feet, to) >= WALK_ON_M {
            break;
        }
        body.walk = Vec3::new(ux * WALK_MPS * DT, 0.0, uz * WALK_MPS * DT);
        body.step(world, DT, s);
    }
    body.walk = Vec3::ZERO;
    for _ in 0..SETTLE_TICKS {
        if !body.step(world, DT, s).airborne {
            break;
        }
    }
    landed(body.feet())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::WalkerBehaviour;

    fn quad(x0: f32, x1: f32, z0: f32, z1: f32, y: f32) -> [Triangle; 2] {
        let t = |v: [Vec3; 3]| Triangle { v, walker: WalkerBehaviour::BySlope };
        [
            t([Vec3::new(x0, y, z0), Vec3::new(x0, y, z1), Vec3::new(x1, y, z1)]),
            t([Vec3::new(x0, y, z0), Vec3::new(x1, y, z1), Vec3::new(x1, y, z0)]),
        ]
    }

    /// A step off a slab onto the ground 2 m below is followed; one whose landing lies under a deck that goes on past it is
    /// not (the body stays on the deck).
    #[test]
    fn a_walk_off_reaches_its_landing_and_a_landing_under_a_deck_is_not_reached() {
        let mut tris = Vec::new();
        tris.extend(quad(-10.0, 10.0, -10.0, 10.0, 0.0));
        tris.extend(quad(-5.0, 0.0, -2.0, 2.0, 2.0));
        let world = Collision::build(&tris);
        let s = Settings::LIVE;
        assert!(follows(&world, &s, Vec3::new(-0.25, 2.0, 0.0), Vec3::new(0.25, 0.0, 0.0)));
        let mut deck = tris.clone();
        deck.extend(quad(0.0, 5.0, -2.0, 2.0, 2.0));
        let world = Collision::build(&deck);
        assert!(!follows(&world, &s, Vec3::new(-0.25, 2.0, 0.0), Vec3::new(0.25, 0.0, 0.0)));
    }
}
