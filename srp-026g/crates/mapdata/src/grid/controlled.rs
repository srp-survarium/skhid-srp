//! [A313] Links decided by the client's own controller ([`Body::client_controller`]): the walk tick of the 0.26g exe
//! (`crate::controller::tick`, proven against the exe in Unicorn) walks a standing body from floor to floor over the map's
//! faces, and the grid keeps what the body does instead of what the fixed rules (step, slope, erosion) guess.
//!
//! Two passes, after the gap links are made:
//! - **Bridges:** a gap link between floors within a step of each other with one cell between (a crack in the map data no face
//!   covers, as Vostok's 0.25 m slab joint, or a cell holding only a riser, as its gully) is walked when the controller walks
//!   the body across: its take-off becomes a walk-off (a plain leg, no key), where the bots pressed a jump the body does not
//!   need and failed it.
//! - **Checks:** a walk link between floors that have a face no body walks on near them (a wall, a rim, a steep pitch within
//!   the reach of the capsule) stays only when the controller gets the body at least [`CHECK_SHARE`] of the way there from
//!   one of [`STARTS`]; where the client's body stalls from all of them (a 45 deg by-slope rim, a wall corner the grid passes),
//!   the bots' map stops offering the way. The links that fail are handed back; the build takes them away except one that is
//!   a floor's only way in from the seeds and spawns or its only way back (`WalkGrid::cut_keeping_reach`).
//!
//! [A313, fx-zones] Why the starts and the guard: fx-bench's M4 score (5a18068a, ON vs OFF) lost human coverage on Cologne:
//! one camper's 205 samples on a 39 deg mossy log (`mossmoss_wood_02`, x 15-18.8, z 40-46): the check dropped the walk links at
//! both of its ends (the body walked from a cell's centre straight to the next slides off the round log), and the ~70 floors
//! went when the floors no seed gets to were dropped. A single straight walk from the cell's centre is one line of many a body
//! takes; a bridge is added, and a walk link taken away, only on what all three lines agree on.
//!
//! The controller runs whole ticks at [`WALK_MPS`] toward the target floor for the ticks the leg takes and two more.
use super::gap::{OffLink, Takeoff};
use super::*;
use crate::controller::tick::{Controller, Settings};
use crate::controller::{Capsule, Collision, Shape};

/// The walk speed the controller is driven at, m/s (a run's root motion; over the client's 3 m/s floor).
pub const WALK_MPS: f32 = 3.5;
/// A checked walk link stays when the body gets this share of the way.
pub const CHECK_SHARE: f32 = 0.5;
/// A floor with less room over it than a standing body is not checked (crouching is not ported), m.
pub const STANDING_ROOM_M: f32 = 1.8;
/// A face no body walks on within this of a floor's capsule makes its links checked, m.
pub const CHECK_REACH_M: f32 = 0.15;
/// The lines the controller walks a leg on: from the floor's centre, and this far either side of it across the leg, m. A
/// check fails, and a bridge is walked, only when every line does.
pub const STARTS: [f32; 3] = [0.0, -0.1, 0.1];
/// The body is at the target when its feet are this near it flat and this near it in height, m.
const AT_FLAT_M: f32 = 0.15;
const AT_HEIGHT_M: f32 = 0.2;
/// A walked bridge is kept only in a run of at least this many walked bridges side by side, each one cell over across the
/// bridge's way (0.75 m at the 0.25 m cell: the body's 0.76 m width). [A313, fx-zones] Cologne day 2 (x 9.7-10, z 32.1-32.6):
/// two bridges up a 0.6 m riser beside a leaning board, where the walks were dropped; bots cut the corner to them one column
/// off and stalled at the board (bench3: 14 presses that freed nothing there, 1 without the pass).
pub const BRIDGE_MIN_RUN: usize = 3;

/// What the controller pass did ([`BuildStats`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ControlledStats {
    pub bridges_tried: usize,
    pub bridges_walked: usize,
    pub bridges_added: usize,
    /// Bridges the controller walks in a run narrower than [`BRIDGE_MIN_RUN`]: not walked (a gap link stays pressed).
    pub bridges_narrow: usize,
    pub checks: usize,
    pub checks_dropped: usize,
    /// Failed checks kept as a floor's only way in or back (`WalkGrid::cut_keeping_reach`).
    pub checks_kept: usize,
}

/// [`walk_share`] on each of [`STARTS`]: the best share, and whether every line ended at `to`.
fn walk_lines(world: &Collision, from: Vec3, to: Vec3) -> (f32, bool) {
    let flat = Vec3::new(to.x - from.x, 0.0, to.z - from.z);
    let l = flat.length();
    let side = if l > 1e-4 { Vec3::new(-flat.z / l, 0.0, flat.x / l) } else { Vec3::ZERO };
    let (mut best, mut all) = (0.0f32, true);
    for off in STARTS {
        let (share, at) = walk_share(world, from + side * off, to + side * off);
        best = best.max(share);
        all &= at;
    }
    (best, all)
}

/// A floor's feet moved flat out of the faces the client's body would overlap there: the grid keeps floors 0.3175 m from a wall,
/// the body's move test holds 0.42 m (the cylinder and its margin, over the step), and the client never stands inside a wall (its
/// world pushes a body out: `bullet_physics_world::recover_from_penetrations`). At most [`OUT_OF_WALLS_M`] in all.
pub(super) fn out_of_walls(world: &Collision, feet: Vec3) -> Vec3 {
    let mut at = feet;
    let mut moved = 0.0f32;
    for _ in 0..4 {
        let cyl = Shape::Cylinder { centre: at + Vec3::new(0.0, 1.3, 0.0), half: 0.5, radius: 0.38, margin: 0.04 };
        let Some((gap, _, dir)) = world.nearest_to(cyl, 0.0, |f| f.normal.y.abs() < 0.5) else { break };
        let flat = Vec3::new(dir.x, 0.0, dir.z);
        let l = flat.length();
        if gap >= 0.0 || l < 1e-4 {
            break;
        }
        let push = (-gap + 0.002).min(OUT_OF_WALLS_M - moved);
        if push <= 0.0 {
            break;
        }
        at = at + flat * (push / l);
        moved += push;
    }
    at
}

/// How far [`out_of_walls`] moves a floor's feet at most, m (the 0.42 m hold less the grid's 0.3175 m, and a little).
pub const OUT_OF_WALLS_M: f32 = 0.15;

/// How far the controller takes a standing body from `from` toward `to` (feet), as a share of the flat way, and whether it
/// ended at `to`.
pub fn walk_share(world: &Collision, from: Vec3, to: Vec3) -> (f32, bool) {
    let flat = Vec3::new(to.x - from.x, 0.0, to.z - from.z);
    let length = flat.length();
    if length < 1e-4 {
        return (1.0, true);
    }
    let dir = flat * (1.0 / length);
    let step = WALK_MPS * 0.05;
    let ticks = (length / step).ceil() as usize + 2;
    let mut c = Controller::standing_at(out_of_walls(world, from));
    let s = Settings::LIVE;
    let mut best = 0.0f32;
    for _ in 0..ticks {
        let feet = c.feet();
        let left = Vec3::new(to.x - feet.x, 0.0, to.z - feet.z);
        if left.length() <= AT_FLAT_M && (feet.y - to.y).abs() <= AT_HEIGHT_M {
            return (1.0, true);
        }
        let along = Vec3::new(feet.x - from.x, 0.0, feet.z - from.z).dot(dir) / length;
        best = best.max(along);
        // Toward the target, at most what is left.
        let go = left.length().min(step);
        c.walk = if left.length() > 1e-6 { left * (go / left.length()) } else { Vec3::ZERO };
        if c.step(world, 0.05, &s).ceiling {
            // A low ceiling: the client slides under it or crouches (neither ported): not a stall the pass knows.
            return (1.0, false);
        }
    }
    let feet = c.feet();
    let left = Vec3::new(to.x - feet.x, 0.0, to.z - feet.z);
    let at = left.length() <= AT_FLAT_M && (feet.y - to.y).abs() <= AT_HEIGHT_M;
    (best.max(Vec3::new(feet.x - from.x, 0.0, feet.z - from.z).dot(dir) / length), at)
}

/// Runs `job` over `items` on the machine's threads, in order.
pub(super) fn parallel<T: Sync, R: Send + Default + Clone>(items: &[T], job: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).min(16);
    let chunk = items.len().div_ceil(threads).max(1);
    let mut out = vec![R::default(); items.len()];
    std::thread::scope(|scope| {
        for (part, slot) in items.chunks(chunk).zip(out.chunks_mut(chunk)) {
            let job = &job;
            scope.spawn(move || {
                for (item, r) in part.iter().zip(slot.iter_mut()) {
                    *r = job(item);
                }
            });
        }
    });
    out
}

impl WalkGrid {
    /// The controller pass ([`Body::client_controller`], module documentation).
    /// The bridges are made here; the walk links that fail their check are returned (a floor, a direction and the share the body
    /// got), for the build to take away with `cut_keeping_reach`.
    pub(super) fn controlled_links(&self, wide: &mut Wide, triangles: &[Triangle]) -> (ControlledStats, Vec<(u32, usize, f32)>) {
        let world = Collision::build(triangles);
        let mut stats = ControlledStats::default();
        let body = self.body;
        let cells_apart = |a: usize, b: usize| {
            let (ia, ka) = self.cell_xz(self.cell_of[a] as usize);
            let (ib, kb) = self.cell_xz(self.cell_of[b] as usize);
            (ia - ib).abs().max((ka - kb).abs())
        };
        // Bridges.
        let bridges: Vec<usize> = wide
            .off
            .iter()
            .enumerate()
            .filter(|(_, l)| {
                l.kind == LinkKind::GapJump
                    && !matches!(l.press, Takeoff::WalkOff)
                    && (self.height[l.from as usize] - self.height[l.to as usize]).abs() <= body.step
                    && cells_apart(l.from as usize, l.to as usize) == 2
            })
            .map(|(i, _)| i)
            .collect();
        stats.bridges_tried = bridges.len();
        let walked = parallel(&bridges, |&i| {
            let l: &OffLink = &wide.off[i];
            walk_lines(&world, self.position(l.from), self.position(l.to)).1
        });
        let old: Vec<usize> = bridges.iter().zip(walked).filter(|&(_, ok)| ok).map(|(&i, _)| i).collect();
        // Bridges with no link yet: two cells on, a floor within a step, the cell between with no floor near that height (a
        // crack no face covers, a cell holding only a riser), walked by the controller: a walked off-mesh link.
        wide.off.sort_by_key(|l| (l.from, l.to));
        let mut new: Vec<(u32, u32)> = Vec::new();
        for a in 0..self.height.len() {
            let (i, k) = self.cell_xz(self.cell_of[a] as usize);
            let ha = self.height[a];
            for (d, &(di, dk)) in DIRS.iter().enumerate() {
                if wide.links[a][d] != NONE {
                    continue;
                }
                let (Some(c2), Some(c3)) = (self.cell_index(i + di, k + dk), self.cell_index(i + 2 * di, k + 2 * dk)) else { continue };
                if (self.start[c2]..self.start[c2 + 1]).any(|m| (self.height[m as usize] - ha).abs() <= body.step) {
                    continue;
                }
                for b in self.start[c3]..self.start[c3 + 1] {
                    if (self.height[b as usize] - ha).abs() <= body.step && wide.off.binary_search_by_key(&(a as u32, b), |l| (l.from, l.to)).is_err() {
                        new.push((a as u32, b));
                    }
                }
            }
        }
        stats.bridges_tried += new.len();
        let walked = parallel(&new, |&(a, b)| walk_lines(&world, self.position(a), self.position(b)).1);
        let new: Vec<(u32, u32)> = new.iter().zip(walked).filter(|&(_, ok)| ok).map(|(&p, _)| p).collect();
        // Only the bridges in a run at least [`BRIDGE_MIN_RUN`] wide are walked: a narrower one is crossed only by a body
        // inside its columns, which a bot steering at a point ahead of it does not keep to.
        let all: std::collections::HashSet<(u32, u32)> =
            old.iter().map(|&i| (wide.off[i].from, wide.off[i].to)).chain(new.iter().copied()).collect();
        for &i in &old {
            if self.bridge_run(&all, wide.off[i].from, wide.off[i].to) >= BRIDGE_MIN_RUN {
                wide.off[i].press = Takeoff::WalkOff;
                stats.bridges_walked += 1;
            } else {
                stats.bridges_narrow += 1;
            }
        }
        for &(a, b) in &new {
            if self.bridge_run(&all, a, b) >= BRIDGE_MIN_RUN {
                wide.off.push(OffLink { from: a, to: b, kind: LinkKind::GapJump, press: Takeoff::WalkOff });
                stats.bridges_walked += 1;
                stats.bridges_added += 1;
            } else {
                stats.bridges_narrow += 1;
            }
        }
        wide.off.sort_by_key(|l| (l.from, l.to));
        // Checks: the floors with a face no body walks on near their capsule.
        let n = self.height.len();
        let floors: Vec<usize> = (0..n).collect();
        let near = parallel(&floors, |&a| {
            let feet = self.position(a as NodeId);
            let cap = Capsule::standing(feet, 1.8, 0.38);
            world
                .nearest(cap, CHECK_REACH_M, |f| f.walker == WalkerBehaviour::DontWalk || f.normal.y.abs() < body.min_ground_normal_y)
                .is_some()
        });
        let mut checks: Vec<(usize, usize)> = Vec::new();
        for a in 0..n {
            for d in 0..8 {
                let b = wide.links[a][d];
                let room = |n: usize| self.ceiling[n] - self.height[n] >= STANDING_ROOM_M;
                if b != NONE && wide.kinds[a][d] == LinkKind::Walk && (near[a] || near[b as usize]) && room(a) && room(b as usize) {
                    checks.push((a, d));
                }
            }
        }
        stats.checks = checks.len();
        let shares = parallel(&checks, |&(a, d)| {
            let b = wide.links[a][d];
            walk_lines(&world, self.position(a as NodeId), self.position(b)).0
        });
        let failing = checks
            .iter()
            .zip(shares)
            .filter(|&(_, share)| share < CHECK_SHARE)
            .map(|(&(a, d), share)| (a as u32, d, share))
            .collect();
        (stats, failing)
    }

    /// How many bridges of `all` are side by side with the bridge `a` -> `b`, it included, counted out to [`BRIDGE_MIN_RUN`]
    /// either side: one floor's cell over across the bridge's way each, to a floor one cell over from `b`'s.
    fn bridge_run(&self, all: &std::collections::HashSet<(u32, u32)>, a: u32, b: u32) -> usize {
        let (ia, ka) = self.cell_xz(self.cell_of[a as usize] as usize);
        let (ib, kb) = self.cell_xz(self.cell_of[b as usize] as usize);
        let (dx, dz) = ((ib - ia) as f32, (kb - ka) as f32);
        let l = dx.hypot(dz);
        let (sx, sz) = ((-dz / l).round() as i32, (dx / l).round() as i32);
        let mut run = 1;
        for side in [1, -1] {
            for s in 1..BRIDGE_MIN_RUN as i32 {
                let (ox, oz) = (side * s * sx, side * s * sz);
                let (Some(ca), Some(cb)) = (self.cell_index(ia + ox, ka + oz), self.cell_index(ib + ox, kb + oz)) else { break };
                let found = (self.start[ca]..self.start[ca + 1])
                    .any(|a2| (self.start[cb]..self.start[cb + 1]).any(|b2| all.contains(&(a2, b2))));
                if !found {
                    break;
                }
                run += 1;
            }
        }
        run
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quad(tris: &mut Vec<Triangle>, a: Vec3, b: Vec3, c: Vec3, d: Vec3) {
        tris.push(Triangle { v: [a, b, c], walker: WalkerBehaviour::BySlope });
        tris.push(Triangle { v: [a, c, d], walker: WalkerBehaviour::BySlope });
    }

    fn floor(tris: &mut Vec<Triangle>, x0: f32, z0: f32, x1: f32, z1: f32, y: f32) {
        quad(tris, Vec3::new(x0, y, z0), Vec3::new(x0, y, z1), Vec3::new(x1, y, z1), Vec3::new(x1, y, z0));
    }

    /// A floor at 0 and a top 0.6 m up past one 0.25 m cell (x 5.0-5.25), 6 m wide. With `strip`, that cell holds a 0.3 m
    /// step (walked up in two) except over `strip` metres of the width, where it holds only the riser (none: the riser all the
    /// way): the walk-off bridges the pass makes up it, and the ones it walks but leaves pressed as too narrow a run.
    fn bridges_up(strip: Option<f32>) -> (usize, usize) {
        let mut tris = Vec::new();
        floor(&mut tris, 0.0, 0.0, 5.0, 6.0, 0.0);
        floor(&mut tris, 5.25, 0.0, 10.0, 6.0, 0.6);
        quad(&mut tris, Vec3::new(5.25, 0.0, 0.0), Vec3::new(5.25, 0.6, 0.0), Vec3::new(5.25, 0.6, 6.0), Vec3::new(5.25, 0.0, 6.0));
        if let Some(w) = strip {
            let (z0, z1) = (3.0 - w / 2.0, 3.0 + w / 2.0);
            for (a, b) in [(0.0, z0), (z1, 6.0)] {
                floor(&mut tris, 5.0, a, 5.25, b, 0.3);
                quad(&mut tris, Vec3::new(5.0, 0.0, a), Vec3::new(5.0, 0.0, b), Vec3::new(5.0, 0.3, b), Vec3::new(5.0, 0.3, a));
            }
        }
        let body = Body { client_controller: true, ..Body::client() };
        let (grid, stats) = WalkGrid::build_kept_with_links(&tris, [0.0, 0.0, 10.0, 6.0], 0.25, body, &[], &[], OffMeshLinks::Gaps);
        let walked = grid
            .off_links()
            .iter()
            .filter(|l| {
                l.kind == LinkKind::GapJump
                    && l.press == Takeoff::WalkOff
                    && grid.position(l.from).y.abs() < 0.05
                    && (grid.position(l.to).y - 0.6).abs() < 0.05
            })
            .count();
        (walked, stats.controlled.bridges_narrow)
    }

    /// [A313] A riser bare its whole width is bridged; bare over a 0.5 m strip only (two columns of the grid, the rest
    /// stepped) it is walked by the controller but not bridged: a bot steering at a point ahead does not keep inside so
    /// narrow a run, and walks the step beside it.
    #[test]
    fn a_narrow_bridge_run_is_not_walked() {
        let (open, _) = bridges_up(None);
        assert!(open >= BRIDGE_MIN_RUN, "the bare riser: {open} bridges");
        let (walked, narrow) = bridges_up(Some(0.5));
        assert_eq!(walked, 0, "the 0.5 m strip");
        assert!(narrow > 0, "the 0.5 m strip had bridges the controller walks, left pressed");
    }
}
