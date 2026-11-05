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
//!   the reach of the capsule) stays only when the controller gets the body at least [`CHECK_SHARE`] of the way there; where
//!   the client's body stalls (a 45 deg by-slope rim, a wall corner the grid passes), the bots' map stops offering the way.
//!
//! The controller runs whole ticks at [`WALK_MPS`] toward the target floor for the ticks the leg takes and two more.
use super::gap::{OffLink, Takeoff};
use super::*;
use crate::controller::tick::{Controller, Settings};
use crate::controller::{Capsule, Collision};

/// The walk speed the controller is driven at, m/s (a run's root motion; over the client's 3 m/s floor).
pub const WALK_MPS: f32 = 3.5;
/// A checked walk link stays when the body gets this share of the way.
pub const CHECK_SHARE: f32 = 0.5;
/// A face no body walks on within this of a floor's capsule makes its links checked, m.
pub const CHECK_REACH_M: f32 = 0.15;
/// The body is at the target when its feet are this near it flat and this near it in height, m.
const AT_FLAT_M: f32 = 0.15;
const AT_HEIGHT_M: f32 = 0.2;

/// What the controller pass did ([`BuildStats`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ControlledStats {
    pub bridges_tried: usize,
    pub bridges_walked: usize,
    pub bridges_added: usize,
    pub checks: usize,
    pub checks_dropped: usize,
}

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
    let mut c = Controller::standing_at(from);
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
        c.step(world, 0.05, &s);
    }
    let feet = c.feet();
    let left = Vec3::new(to.x - feet.x, 0.0, to.z - feet.z);
    let at = left.length() <= AT_FLAT_M && (feet.y - to.y).abs() <= AT_HEIGHT_M;
    (best.max(Vec3::new(feet.x - from.x, 0.0, feet.z - from.z).dot(dir) / length), at)
}

/// Runs `job` over `items` on the machine's threads, in order.
fn parallel<T: Sync, R: Send + Default + Clone>(items: &[T], job: impl Fn(&T) -> R + Sync) -> Vec<R> {
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
    pub(super) fn controlled_links(&self, wide: &mut Wide, triangles: &[Triangle]) -> ControlledStats {
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
            walk_share(&world, self.position(l.from), self.position(l.to)).1
        });
        for (&i, ok) in bridges.iter().zip(walked) {
            if ok {
                wide.off[i].press = Takeoff::WalkOff;
                stats.bridges_walked += 1;
            }
        }
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
        let walked = parallel(&new, |&(a, b)| walk_share(&world, self.position(a), self.position(b)).1);
        for (&(a, b), ok) in new.iter().zip(walked) {
            if ok {
                wide.off.push(OffLink { from: a, to: b, kind: LinkKind::GapJump, press: Takeoff::WalkOff });
                stats.bridges_walked += 1;
                stats.bridges_added += 1;
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
                if b != NONE && wide.kinds[a][d] == LinkKind::Walk && (near[a] || near[b as usize]) {
                    checks.push((a, d));
                }
            }
        }
        stats.checks = checks.len();
        let shares = parallel(&checks, |&(a, d)| {
            let b = wide.links[a][d];
            walk_share(&world, self.position(a as NodeId), self.position(b)).0
        });
        for (&(a, d), share) in checks.iter().zip(shares) {
            if share < CHECK_SHARE {
                wide.links[a][d] = NONE;
                stats.checks_dropped += 1;
            }
        }
        stats
    }
}
