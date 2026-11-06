//! [A316 raised floors / A318 slopes] Slide floors ([`Body::slide_floors`]): floors the client's body cannot hold, taken out.
//!
//! The grid keeps floors on faces up to 60 deg ([`STEEP_NORMAL_Y`]) and on cells where a walkable face lies under a `dont_walk`
//! one, while the client walks ground only to 55 deg (`cc_max_slope_angle_cos` 0.57358 in the live `startup.cfg`) and holds a
//! still body only to 45 deg (`idle_max_slope_cos` 0.70711), and treats a `dont_walk` face as just past the walk limit (the walk
//! tick tilts its normal there, `controller::tick`). The bots stood on such floors and the client slid them off: the A313 census
//! counted 224 raised stalls on faces past 55 deg, and fx-zones' A318 samples (`slope-samples-*.txt`) show the client sliding
//! where the grid has a floor (a `Slope` past 55 deg or `DontWalk` under the feet).
//!
//! The test is the client's own walk tick ([`Controller::step`], [`Settings::LIVE`], proven against the exe in Unicorn), run on
//! the floors where a face no body walks on lies within [`NEAR_M`] of the standing capsule (the controller pass's test):
//! - the body is stood on the floor (out of walls, as the controller pass does), at its centre and 0.1 m either side, and run
//!   [`HOLD_TICKS`] with no keys; the floor slides only when all three stands slide (a boxed-in body that cannot move holds);
//! - it **slides** when its feet move [`SLIDE_M`] or more across or down;
//! - a slide on ground past the walk limit (its last contact normal under `max_slope_cos`) is a floor a **moving** body does not
//!   hold either: every link into it is handed to [`WalkGrid::cut_keeping_reach`], so it goes unless it is some floor's only way
//!   in or back;
//! - a slide on ground between the idle and the walk limits (45-55 deg) is a floor a body holds while it walks on: counted only
//!   ([`SlideStats::standing_only`]), a mark for where bots stop is a later step.

use super::controlled::{out_of_walls, parallel};
use super::*;
use crate::controller::tick::{Controller, Settings};
use crate::controller::{Capsule, Collision};

/// A face no body walks on within this of a floor's standing capsule makes the floor tested, m (the controller pass's reach).
pub const NEAR_M: f32 = 0.15;
/// The body is held still this many ticks of 50 ms (1 s).
pub const HOLD_TICKS: usize = 20;
/// Feet that moved this far across or down in that time slid, m.
pub const SLIDE_M: f32 = 0.3;

/// What the pass found ([`BuildStats`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SlideStats {
    /// Floors with a face no body walks on near their capsule, run in the walk tick.
    pub tested: usize,
    /// Floors a still body slides off on ground past the walk limit: their links in were handed to the cut.
    pub moving: usize,
    /// Floors a still body slides off on ground under the walk limit (45-55 deg): kept.
    pub standing_only: usize,
    /// Links into the moving-slide floors taken away, and those kept as a floor's only way in or back.
    pub cut: usize,
    pub kept: usize,
}

/// How a still body fares on a floor.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Hold {
    /// Not tested, or held.
    #[default]
    Holds,
    /// Slid, on ground under the walk limit: a walking body holds it.
    SlidesStanding,
    /// Slid, on ground past the walk limit (or a `dont_walk` face).
    SlidesMoving,
}

/// How a still body stood at `feet` fares in [`HOLD_TICKS`] of the walk tick.
pub fn hold_at(world: &Collision, feet: Vec3, s: &Settings) -> Hold {
    let start = out_of_walls(world, feet);
    let mut c = Controller::standing_at(start);
    let mut steep = false;
    for _ in 0..HOLD_TICKS {
        c.walk = Vec3::ZERO;
        if c.step(world, 0.05, s).ceiling {
            // A low ceiling: the client crouches or slides under it (neither ported): not a slide the pass knows.
            return Hold::Holds;
        }
        steep |= c.contact_normal.y < s.max_slope_cos - 1e-3;
    }
    let end = c.feet();
    let across = Vec3::new(end.x - start.x, 0.0, end.z - start.z).length();
    if across < SLIDE_M && start.y - end.y < SLIDE_M {
        Hold::Holds
    } else if steep {
        Hold::SlidesMoving
    } else {
        Hold::SlidesStanding
    }
}

/// A floor's verdict from three stands, its centre and [`START_OFF_M`] either side of it along x: a slide only when every stand
/// slid (one that holds, or a boxed-in body that cannot move, says the floor holds), moving only when every one did.
pub fn hold_floor(world: &Collision, feet: Vec3, s: &Settings) -> Hold {
    let holds = [0.0, -START_OFF_M, START_OFF_M].map(|dx| hold_at(world, feet + Vec3::new(dx, 0.0, 0.0), s));
    if holds.contains(&Hold::Holds) {
        Hold::Holds
    } else if holds.iter().all(|&h| h == Hold::SlidesMoving) {
        Hold::SlidesMoving
    } else {
        Hold::SlidesStanding
    }
}

/// The side stands of [`hold_floor`], m (the controller pass's [`super::controlled::STARTS`]).
pub const START_OFF_M: f32 = 0.1;

impl WalkGrid {
    /// The slide-floors pass ([`Body::slide_floors`], module documentation): the floors a moving body cannot hold lose every way
    /// in (walk links through `cut_keeping_reach`, and the off-mesh links landing on them), so the build drops them; except one
    /// whose loss would leave a floor that is not itself a slide floor out of reach from the seeds and spawns (it stays: a way
    /// through to the rest of the map).
    pub(super) fn cut_slide_floors(&self, wide: &mut Wide, triangles: &[Triangle], seeds: &[Vec3], spawns: &[Vec3]) -> SlideStats {
        let mut stats = SlideStats::default();
        let world = Collision::build(triangles);
        let s = Settings::LIVE;
        let n = self.height.len();
        let floors: Vec<usize> = (0..n).collect();
        let near = parallel(&floors, |&a| {
            let cap = Capsule::standing(self.position(a as NodeId), 1.8, 0.38);
            world.nearest(cap, NEAR_M, |f| f.walker == WalkerBehaviour::DontWalk || f.normal.y.abs() < s.max_slope_cos).is_some()
        });
        let tested: Vec<usize> = floors.into_iter().filter(|&a| near[a]).collect();
        stats.tested = tested.len();
        let holds = parallel(&tested, |&a| hold_floor(&world, self.position(a as NodeId), &s));
        let mut sliding = vec![false; n];
        for (&a, hold) in tested.iter().zip(&holds) {
            match hold {
                Hold::SlidesMoving => sliding[a] = true,
                Hold::SlidesStanding => stats.standing_only += 1,
                Hold::Holds => {}
            }
        }
        // The floors the starts reach with and without the sliding ones; a sliding floor with a link out to a floor lost that way
        // stays, until nothing more is lost.
        let starts: Vec<u32> = spawns
            .iter()
            .filter_map(|&p| self.locate(p, SPAWN_M))
            .chain(seeds.iter().filter_map(|&p| self.locate(p, SEED_M)))
            .collect();
        let (first, to) = wide.adjacency();
        let reach = |skip: &[bool]| {
            let mut seen = vec![false; n];
            let mut queue: Vec<u32> = starts.iter().copied().filter(|&a| !skip[a as usize]).collect();
            for &a in &queue {
                seen[a as usize] = true;
            }
            while let Some(a) = queue.pop() {
                for &b in &to[first[a as usize] as usize..first[a as usize + 1] as usize] {
                    if !seen[b as usize] && !skip[b as usize] {
                        seen[b as usize] = true;
                        queue.push(b);
                    }
                }
            }
            seen
        };
        let before = reach(&vec![false; n]);
        for _ in 0..8 {
            let after = reach(&sliding);
            let lost: Vec<bool> = (0..n).map(|b| before[b] && !after[b] && !sliding[b]).collect();
            let mut freed = false;
            for a in 0..n {
                if sliding[a] && to[first[a] as usize..first[a + 1] as usize].iter().any(|&b| lost[b as usize]) {
                    sliding[a] = false;
                    freed = true;
                }
            }
            if !freed {
                break;
            }
        }
        stats.moving = sliding.iter().filter(|&&x| x).count();
        let failing: Vec<(u32, usize, f32)> = (0..n)
            .flat_map(|a| (0..8).map(move |d| (a, d)))
            .filter(|&(a, d)| {
                let b = wide.links[a][d];
                b != NONE && sliding[b as usize] && !sliding[a]
            })
            .map(|(a, d)| (a as u32, d, 0.0))
            .collect();
        (stats.cut, stats.kept) = self.cut_keeping_reach(wide, failing, seeds, spawns, false);
        let off = wide.off.len();
        wide.off.retain(|l| !(sliding[l.to as usize] && !sliding[l.from as usize]));
        stats.cut += off - wide.off.len();
        stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quad(tris: &mut Vec<Triangle>, a: Vec3, b: Vec3, c: Vec3, d: Vec3, walker: WalkerBehaviour) {
        tris.push(Triangle { v: [a, b, c], walker });
        tris.push(Triangle { v: [a, c, d], walker });
    }

    /// A plane rising along x at `deg` degrees over x 0-4, z 0-4, from y 0.
    fn slope(deg: f32, walker: WalkerBehaviour) -> Vec<Triangle> {
        let rise = 4.0 * deg.to_radians().tan();
        let mut tris = Vec::new();
        quad(&mut tris, Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 4.0), Vec3::new(4.0, rise, 4.0), Vec3::new(4.0, rise, 0.0), walker);
        tris
    }

    fn hold(deg: f32, walker: WalkerBehaviour) -> Hold {
        let tris = slope(deg, walker);
        let world = Collision::build(&tris);
        let y = 2.0 * deg.to_radians().tan();
        hold_at(&world, Vec3::new(2.0, y, 2.0), &Settings::LIVE)
    }

    /// The client's limits: a still body holds 40 deg, slides on 50 deg (past the idle limit, under the walk limit) and on 58
    /// deg (past the walk limit), and on a gentle `dont_walk` face (the tick tilts it past the walk limit).
    #[test]
    fn a_still_body_slides_by_the_clients_limits() {
        assert_eq!(hold(0.0, WalkerBehaviour::BySlope), Hold::Holds);
        assert_eq!(hold(40.0, WalkerBehaviour::BySlope), Hold::Holds);
        assert_eq!(hold(50.0, WalkerBehaviour::BySlope), Hold::SlidesStanding);
        assert_eq!(hold(58.0, WalkerBehaviour::BySlope), Hold::SlidesMoving);
        assert_eq!(hold(58.0, WalkerBehaviour::Walk), Hold::Holds, "a walk face is ground whatever its slope");
        assert_eq!(hold(20.0, WalkerBehaviour::DontWalk), Hold::SlidesMoving);
    }
}

#[cfg(test)]
mod probe {
    use super::*;

    /// How far a still body on a plane of each slope goes in 1 s and 3 s (feet across, down), and the contact normal's slope.
    #[test]
    #[ignore = "a probe: prints, asserts nothing"]
    fn print_still_body_slides() {
        for (deg, walker) in [
            (40.0f32, WalkerBehaviour::BySlope),
            (45.0, WalkerBehaviour::BySlope),
            (48.0, WalkerBehaviour::BySlope),
            (50.0, WalkerBehaviour::BySlope),
            (53.0, WalkerBehaviour::BySlope),
            (56.0, WalkerBehaviour::BySlope),
            (58.0, WalkerBehaviour::BySlope),
            (65.0, WalkerBehaviour::BySlope),
            (20.0, WalkerBehaviour::DontWalk),
        ] {
            let rise = 8.0 * deg.to_radians().tan();
            let mut tris = Vec::new();
            let (a, b, c, d) = (Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 8.0), Vec3::new(8.0, rise, 8.0), Vec3::new(8.0, rise, 0.0));
            tris.push(Triangle { v: [a, b, c], walker });
            tris.push(Triangle { v: [a, c, d], walker });
            let world = Collision::build(&tris);
            let feet = Vec3::new(5.0, 5.0 * deg.to_radians().tan(), 4.0);
            let mut c = Controller::standing_at(feet);
            let mut line = format!("{deg:>4} deg {walker:?}:");
            for t in 1..=60 {
                c.walk = Vec3::ZERO;
                c.step(&world, 0.05, &Settings::LIVE);
                if t == 20 || t == 60 {
                    let e = c.feet();
                    line += &format!(
                        " {}s across {:.2} down {:.2} n.y {:.3};",
                        t / 20,
                        Vec3::new(e.x - feet.x, 0.0, e.z - feet.z).length(),
                        feet.y - e.y,
                        c.contact_normal.y
                    );
                }
            }
            eprintln!("{line}");
        }
    }
}
