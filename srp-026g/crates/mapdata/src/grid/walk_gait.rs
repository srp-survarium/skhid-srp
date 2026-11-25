//! [bot traps, ledge line-up] The low risers a walking body does not get up ([`super::Body::walk_gait`]).
//!
//! A Walk link rising less than [`super::Body::walked_step`] is walked at any gait ([`super::WalkGait::Any`]), and the routes, the
//! motor's look-ahead and the pull walk it. Some do not walk. On level_06 day 4 the bots held forward for 4 s or more at
//! (62.03, 33.59, 120.24) under the links 465689 -> 466553 (+0.22 m) and 466553 -> 467433 (+0.35 m): the exe's own tick
//! (rig_real_v3, start level checked) gets up neither at the walk nor at the sprint, from the floor's centre or the bots' feet.
//!
//! This pass takes each such link rising more than [`RISE_M`] and judges it in three steps, every step on the game's shapes:
//!
//! 1. the walk check's test ([`super::controlled::walk_share`]): a link it walks stays as it is;
//! 2. the exe's walk ([`exe_walk`]): the body put at the take-off's centre, settled, then walked toward the top as the client's
//!    walk starts (the speed blended up over six ticks): a link it gets onto the top of stays as it is. Against the exe's own tick
//!    on 57 links the first test fails (26 at random, 31 humans walked up): the same verdict for 55, the feet within 2 cm at every
//!    tick for 51; the two left part on the first walk tick from rest (a move the port refuses, a step down on a face of normal
//!    y 0.75), each a link the exe walks;
//! 3. the risers' flight ([`WalkGrid::walk_gait_press`], as [`WalkGrid::fly_risers`] flies a riser): a press that lands on the top
//!    makes the link a riser (its press in the risers' table, its gait [`super::WalkGait::SprintOrJump`]: the motor's riser gait
//!    takes it); with none it is marked ([`WalkGrid::walk_marked`]), as the walk check marks a riser it keeps.
//!
//! The census on the d2993ba6d grids (16 maps): 78,832 of 1,033,170 such risers fail step 1; of them 31,021 walk by step 2, 17,716
//! get a press, 30,095 none.

use super::controlled::walk_share;
use super::rim::Riser;
use super::{LinkKind, WalkGrid, Wide, DIRS, NONE};
use crate::controller::tick::{Controller, CrouchInput, Settings, TickInput};
use crate::controller::Collision;
use crate::math::Vec3;

/// A link rising more than this is judged, m: the walk check's riser ([`super::controlled::CUT_RISE_M`]).
pub const RISE_M: f32 = super::controlled::CUT_RISE_M;
/// The exe's walk: the ticks the body stands to settle first …
pub const SETTLE_TICKS: usize = 20;
/// … the ticks it walks …
pub const WALK_TICKS: usize = 40;
/// … at this speed, m/s (`default.player`'s `movement_speed` standing forward) …
pub const WALK_MPS: f32 = super::controlled::WALK_MPS;
/// … blended up from rest over this many ticks (the client's walk start: n/6 of the speed on its n-th tick).
pub const BLEND_TICKS: usize = 6;
/// It gets onto the top when its feet come within this of the top's height, m …
pub const ONTO_DY_M: f32 = 0.15;
/// … and this of its centre across, m.
pub const ONTO_FLAT_M: f32 = 0.5;

/// What the pass did ([`super::BuildStats`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WalkGaitStats {
    /// Low risers judged …
    pub judged: usize,
    /// … of them failing the walk check's test …
    pub unwalked: usize,
    /// … of those walked by the exe's walk …
    pub exe_walked: usize,
    /// … made risers (a press lands on the top) …
    pub pressed: usize,
    /// … and marked (no press).
    pub marked: usize,
}

/// Whether a body put at `from` (feet) and settled ([`SETTLE_TICKS`]: the velocity a hair down first, as the exe rig's settle
/// writes it), then walked toward `to` for [`WALK_TICKS`] ticks at [`WALK_MPS`] blended up over [`BLEND_TICKS`], gets onto `to`'s
/// floor at any walk tick: its feet within [`ONTO_DY_M`] of `to`'s height and [`ONTO_FLAT_M`] across.
pub fn exe_walk(world: &Collision, s: &Settings, from: Vec3, to: Vec3) -> bool {
    let flat = Vec3::new(to.x - from.x, 0.0, to.z - from.z);
    let length = flat.length();
    if length < 1e-4 {
        return true;
    }
    let dir = flat * (1.0 / length);
    let mut c = Controller::standing_at(from);
    c.velocity = Vec3::new(0.0, -0.01, 0.0);
    let still = TickInput { walk: Vec3::ZERO, air_control: Vec3::ZERO, crouch: CrouchInput::Tested { key: false, legs_broken: false }, sprinting: false };
    for _ in 0..SETTLE_TICKS {
        c.tick(world, 0.05, s, still);
    }
    for n in 1..=WALK_TICKS {
        let speed = WALK_MPS * n.min(BLEND_TICKS) as f32 / BLEND_TICKS as f32;
        c.tick(world, 0.05, s, TickInput { walk: dir * (speed * 0.05), ..still });
        let feet = c.feet();
        if (feet.y - to.y).abs() <= ONTO_DY_M && feet.distance_xz(to) <= ONTO_FLAT_M {
            return true;
        }
    }
    false
}

/// A link's verdict ([`WalkGrid::walk_gait_links`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Verdict {
    /// The walk check's test walks it.
    #[default]
    Walks,
    /// The exe's walk gets onto the top.
    ExeWalks,
    /// A press lands on the top.
    Pressed(Riser),
    /// Neither: marked.
    Unwalked,
}

impl WalkGrid {
    /// The low risers of `wide` judged (module documentation): the pressed ones join the risers' table, the others are marked.
    pub(super) fn walk_gait_links(&self, wide: &mut Wide, world: &Collision, s: &Settings) -> WalkGaitStats {
        let walked = self.body.walked_step();
        let start = self.body.walk_start();
        let crouch = &wide.crouch;
        let risers = &wide.riser;
        let mut todo: Vec<(u32, u32, usize)> = Vec::new();
        for a in 0..self.height.len() {
            for d in 0..DIRS.len() {
                let b = wide.links[a][d];
                if b == NONE || wide.kinds[a][d] != LinkKind::Walk {
                    continue;
                }
                let rise = self.height[b as usize] - self.height[a];
                let known = crouch.binary_search(&(a as u32, b)).is_ok() || risers.binary_search_by_key(&(a as u32, b), |&(f, t, _)| (f, t)).is_ok();
                if rise > RISE_M && rise < walked && !known {
                    todo.push((a as u32, b, d));
                }
            }
        }
        let verdicts = super::controlled::parallel(&todo, |&(a, b, d)| {
            let (pa, pb) = (self.position(a), self.position(b));
            if walk_share(world, s, start, pa, pb).1 {
                return Verdict::Walks;
            }
            // The exe's walk from the take-off's centre as the rig puts it (not moved out of the walls as the walk check's start).
            if exe_walk(world, s, pa, pb) {
                return Verdict::ExeWalks;
            }
            match self.walk_gait_press(world, s, a, b, d) {
                Some(riser) => Verdict::Pressed(riser),
                None => Verdict::Unwalked,
            }
        });
        let mut stats = WalkGaitStats { judged: todo.len(), ..WalkGaitStats::default() };
        for (&(a, b, _), v) in todo.iter().zip(&verdicts) {
            match *v {
                Verdict::Walks => {}
                Verdict::ExeWalks => {
                    stats.unwalked += 1;
                    stats.exe_walked += 1;
                }
                Verdict::Pressed(riser) => {
                    stats.unwalked += 1;
                    stats.pressed += 1;
                    wide.riser.push((a, b, riser));
                }
                Verdict::Unwalked => {
                    stats.unwalked += 1;
                    stats.marked += 1;
                    wide.marked.push((a, b));
                }
            }
        }
        wide.riser.sort_by_key(|&(a, b, _)| (a, b));
        wide.riser.dedup_by_key(|&mut (a, b, _)| (a, b));
        wide.marked.sort_unstable();
        wide.marked.dedup();
        stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::{Body, OffMeshLinks, WalkGait, STANDING_JUMP_M};
    use crate::{Triangle, WalkerBehaviour};

    fn quad(tris: &mut Vec<Triangle>, a: Vec3, b: Vec3, c: Vec3, d: Vec3) {
        tris.push(Triangle { v: [a, b, c], walker: WalkerBehaviour::BySlope });
        tris.push(Triangle { v: [a, c, d], walker: WalkerBehaviour::BySlope });
    }

    /// Ground at 0 for x < 5 and a step from x 5 to 12 whose top is `top` m high: the triangles.
    fn step(top: f32) -> Vec<Triangle> {
        let mut tris = Vec::new();
        quad(&mut tris, Vec3::new(-2.0, 0.0, 0.0), Vec3::new(-2.0, 0.0, 6.0), Vec3::new(5.0, 0.0, 6.0), Vec3::new(5.0, 0.0, 0.0));
        quad(&mut tris, Vec3::new(5.0, top, 0.0), Vec3::new(5.0, top, 6.0), Vec3::new(12.0, top, 6.0), Vec3::new(12.0, top, 0.0));
        quad(&mut tris, Vec3::new(5.0, 0.0, 0.0), Vec3::new(5.0, top, 0.0), Vec3::new(5.0, top, 6.0), Vec3::new(5.0, 0.0, 6.0));
        tris
    }

    fn body() -> Body {
        Body { rim_jumps: true, exe_collision: true, start_witness: true, max_jump: STANDING_JUMP_M, walk_gait: true, ..Body::client() }
    }

    /// The exe's walk gets a standing body up a 0.3 m step from 0.6 m before it, and not up a 2 m wall.
    #[test]
    fn the_exe_walk_gets_up_a_low_step_and_not_a_wall() {
        let (caster, s) = body().controller();
        let low = Collision::build(&step(0.3)).with_caster(caster);
        assert!(exe_walk(&low, &s, Vec3::new(4.4, 0.0, 3.0), Vec3::new(5.4, 0.3, 3.0)));
        let wall = Collision::build(&step(2.0)).with_caster(caster);
        assert!(!exe_walk(&wall, &s, Vec3::new(4.4, 0.0, 3.0), Vec3::new(5.4, 2.0, 3.0)));
    }

    /// With the switch, the risers a walk gets up stay Walk links of any gait: the 0.3 m step's are judged, none is made a riser or
    /// marked.
    #[test]
    fn a_step_a_walk_gets_up_is_left_as_it_is() {
        let tris = step(0.3);
        let start = [Vec3::new(1.0, 0.0, 3.0)];
        let (grid, stats) = WalkGrid::build_kept_with_links(&tris, [-2.0, 0.0, 12.0, 6.0], 0.25, body(), &start, &start, OffMeshLinks::Gaps);
        assert!(stats.walk_gait.judged > 0, "{:?}", stats.walk_gait);
        assert_eq!(stats.walk_gait.unwalked, 0, "{:?}", stats.walk_gait);
        let (a, b) = (grid.locate(Vec3::new(4.875, 0.0, 3.125), 0.05).unwrap(), grid.locate(Vec3::new(5.125, 0.3, 3.125), 0.05).unwrap());
        assert_eq!(grid.link_gait(a, b), Some(WalkGait::Any));
    }

    /// A low riser in the risers' table (what the pass stores where a press lands) is climbed by a sprint or a jump.
    #[test]
    fn a_low_riser_in_the_risers_table_is_sprint_or_jump() {
        let tris = step(0.3);
        let start = [Vec3::new(1.0, 0.0, 3.0)];
        let (mut grid, _) = WalkGrid::build_kept_with_links(&tris, [-2.0, 0.0, 12.0, 6.0], 0.25, body(), &start, &start, OffMeshLinks::Gaps);
        let (a, b) = (grid.locate(Vec3::new(4.875, 0.0, 3.125), 0.05).unwrap(), grid.locate(Vec3::new(5.125, 0.3, 3.125), 0.05).unwrap());
        assert_eq!(grid.link_gait(a, b), Some(WalkGait::Any));
        grid.riser.push((a, b, Riser::NoPress));
        grid.riser.sort_by_key(|&(f, t, _)| (f, t));
        assert_eq!(grid.link_gait(a, b), Some(WalkGait::SprintOrJump));
    }
}
