//! [A313] The Drop links the client's own walk tick does not take a body down ([`super::Body::drop_check`]).
//!
//! The cells see a Drop where a floor ends and a lower one lies a cell on. The client's capsule does not always go down there: it
//! stands on past the take-off cell on a rim the erosion took from the grid, or on a deck that goes on over the landing.
//!
//! Each Drop link is walked as a bot walks it: the body set on the take-off (out of the walls), steered at the landing at
//! [`WALK_MPS`], then on along the link up to [`WALK_ON_M`] past it (a bot walks on to its next node), then left to settle, on
//! three lines across the link. The link is unfollowed when on every line the body walks on ([`MOVED_M`]) and the feet never
//! leave the take-off's level ([`LEAVE_M`] down); a line whose body cannot move from its start says nothing and follows. Where the body goes down but lands elsewhere (past the landing, on a ledge partway) the link stays: the
//! body did take the drop, and the reckoning needs the link to take it down with the client (a first rule that asked for the
//! landing itself made fx-bench's drift score worse: drop-only landings 0.87 -> 0.99 points).
//!
//! The drops census (A313, every Drop link of rev-48's 16 map/day projects through `controller::tick`, one line from the cell
//! centre): 36,829 of 494,533 links (7.4 %) left the body on the take-off's level.

use super::{DIRS, LinkKind, NONE, SEED_M, SPAWN_M, Triangle, Vec3, WalkGrid, Wide};
use super::controlled::{STARTS, out_of_walls, parallel};
use crate::controller::Collision;
use crate::controller::tick::{Controller, Settings};

/// The tick, s (the census's: 30 a second).
const DT: f32 = 1.0 / 30.0;
/// A walking body's speed, m/s.
const WALK_MPS: f32 = 3.5;
/// How far under the take-off the feet must go to have left it, m (or half the drop, when less).
const LEAVE_M: f32 = 0.4;
/// How far from its start across a body must get for its line to say anything, m.
const MOVED_M: f32 = 0.3;
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
        let (caster, s) = self.body.controller();
        let world = Collision::build(triangles).with_caster(caster);
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

/// One line of [`follows`]: the body set standing on `from`, steered at `to`, then on along the line. It follows when it leaves
/// the take-off's level on the way down ([`left_take_off`]), wherever it then lands: a body that slides past the landing or
/// stops on a ledge partway down still went down the drop, and the reckoning needs the link to take it down (fx-bench's drift
/// score of the first rule, which asked for the landing itself: drop-only landings 0.87 -> 0.99 points, the body held up while
/// the world fell). Unfollowed is a body that walks on ([`MOVED_M`] or more from its start) and stays on the take-off's level: a
/// landing under a deck that goes on, a rim the erosion took. A body that cannot move from its start at all (a start boxed in by
/// walls, every move refused) says nothing of the drop: that line follows (fx-bench's second score: level_07's shallow steps).
fn follows_line(world: &Collision, s: &Settings, from: Vec3, to: Vec3) -> bool {
    let mut body = Controller::standing_at(from);
    let down = left_take_off(from.y, to.y);
    let gone = |feet: Vec3| feet.y <= down;
    let mut moved = 0.0f32;
    // Steered at the landing; over it, on along the line.
    for _ in 0..STEER_TICKS {
        let feet = body.feet();
        if gone(feet) {
            return true;
        }
        moved = moved.max(flat(feet, from));
        let d = flat(feet, to);
        if d <= 0.05 {
            break;
        }
        let step = (WALK_MPS * DT).min(d);
        body.walk = Vec3::new((to.x - feet.x) / d * step, 0.0, (to.z - feet.z) / d * step);
        body.step(world, DT, s);
    }
    let len = flat(from, to).max(1e-3);
    let (ux, uz) = ((to.x - from.x) / len, (to.z - from.z) / len);
    for _ in 0..ON_TICKS {
        let feet = body.feet();
        if gone(feet) {
            return true;
        }
        moved = moved.max(flat(feet, from));
        if flat(feet, to) >= WALK_ON_M {
            break;
        }
        body.walk = Vec3::new(ux * WALK_MPS * DT, 0.0, uz * WALK_MPS * DT);
        body.step(world, DT, s);
    }
    body.walk = Vec3::ZERO;
    for _ in 0..SETTLE_TICKS {
        if gone(body.feet()) {
            return true;
        }
        if !body.step(world, DT, s).airborne {
            break;
        }
    }
    gone(body.feet()) || moved.max(flat(body.feet(), from)) < MOVED_M
}

/// The height under which the feet have left a take-off at `from_y` for a landing at `to_y`: [`LEAVE_M`] down, or half the drop
/// when that is less.
fn left_take_off(from_y: f32, to_y: f32) -> f32 {
    from_y - LEAVE_M.min(0.5 * (from_y - to_y).max(0.0))
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

    /// A body that walks off and comes down under the grid's landing (here the ground lies 1 m lower than the landing says) did
    /// take the drop: the link is followed (fx-bench's drift score of the first rule, which cut such links).
    #[test]
    fn a_walk_off_that_lands_under_its_landing_is_followed() {
        let mut tris = Vec::new();
        tris.extend(quad(-10.0, 10.0, -10.0, 10.0, 0.0));
        tris.extend(quad(-5.0, 0.0, -2.0, 2.0, 2.0));
        let world = Collision::build(&tris);
        assert!(follows(&world, &Settings::LIVE, Vec3::new(-0.25, 2.0, 0.0), Vec3::new(0.25, 1.0, 0.0)));
    }

    /// A take-off boxed in by walls (the body cannot move from it on any line) says nothing of the drop: the link is kept.
    #[test]
    fn a_boxed_in_take_off_keeps_its_drop() {
        let wall = |a: Vec3, b: Vec3| {
            let t = |v: [Vec3; 3]| Triangle { v, walker: WalkerBehaviour::BySlope };
            let (a2, b2) = (a + Vec3::new(0.0, 3.0, 0.0), b + Vec3::new(0.0, 3.0, 0.0));
            [t([a, b, b2]), t([a, b2, a2])]
        };
        let mut tris = Vec::new();
        tris.extend(quad(-10.0, 10.0, -10.0, 10.0, 0.0));
        tris.extend(quad(-5.0, 0.0, -2.0, 2.0, 2.0));
        let y = 2.0;
        for (a, b) in [((-0.6, -0.35), (0.1, -0.35)), ((0.1, -0.35), (0.1, 0.35)), ((0.1, 0.35), (-0.6, 0.35)), ((-0.6, 0.35), (-0.6, -0.35))] {
            tris.extend(wall(Vec3::new(a.0, y, a.1), Vec3::new(b.0, y, b.1)));
        }
        let world = Collision::build(&tris);
        assert!(follows(&world, &Settings::LIVE, Vec3::new(-0.25, 2.0, 0.0), Vec3::new(0.25, 0.0, 0.0)));
    }
}
