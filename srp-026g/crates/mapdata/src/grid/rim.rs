//! [A364 C2 rows 11/12] The rim jumps ([`Body::rim_jumps`]): Jump links up a ledge higher than the sprint jump's lowest top
//! ([`SPRINT_JUMP_LOW_M`]), each flown on the exact tick ([`crate::controller::jump`]) as the bots' motor presses it, and kept
//! only where every such press lands.
//!
//! The client's air branch never steps up (`player_step` 0x7BF26: the jump branch has no `step_up`; 0.0000 m over the clip in
//! 2,245 flights): a top over the clip's apex is got onto when the capsule's round bottom comes to rest on its edge as the clip
//! comes down and the ground steps after `end_jump` climb the rest, and only from a run-up (exactness `notes-mapflights.md` §2 C,
//! Chemical Plant's deck: walking 44 of 396, sprint 143 of 424, standing 0 of 396). Whether it does depends on the edge's
//! shape, so no rule of heights says it: each link is flown.
//!
//! As the motor presses it (`servers/match-server/src/v026g/bots/motor.rs`): a link above the grid body's standing jump is a
//! sprint jump (`Leap::SprintLedge`), run up from 2 m behind the take-off (`motor.rs` `SPRINT_RUN_UP_M`; flown with the
//! exactness rig's run-up, [`jump::run_up`]: up to speed over the 0.3 s blend, then 4 steady steps) and pressed on the first tick
//! the take-off is `JUMP_LEAD_S x speed + PRESS_M` or nearer ([`PRESS_LEAD_S`], [`PRESS_EXTRA_M`]): a tick moves the body a
//! step of its speed, so the press falls anywhere in the last step before that lead ([`PRESSES`] presses across it). The
//! sprint jump's leg and the prepare's phase are the run cycle's, not the bot's to choose: both legs and both ends of the
//! phase (6.57 and 7.44 frames, [`SPRINT_PHASES`]) are flown. A link is kept when every flight lands. A walking press (the
//! short jump at the walk, pressed the same way) is flown too and recorded ([`WalkGrid::jump_walks`]) for the motor.
use super::gap::{self, OffLink, PressLead, Takeoff};
use super::{Body, LinkKind, NodeId, WalkGrid, Wide, DIRS, MARGINAL_JUMP_M, NONE, RIM_REACH_M, SPRINT_JUMP_LOW_M};
use crate::controller::jump::{self, Gait, Leg};
use crate::controller::tick::Settings;
use crate::controller::Collision;
use crate::math::Vec3;

/// The motor's press lead, seconds of the body's speed (`motor.rs` `JUMP_LEAD_S`: the game's jump rises from about 0.2 s
/// after the press) …
pub const PRESS_LEAD_S: f32 = 0.2;
/// … and this much more, metres (`motor.rs` `PRESS_M`).
pub const PRESS_EXTRA_M: f32 = 0.15;
/// Presses flown across the last step before the lead.
pub const PRESSES: usize = 4;
/// The sprint jump's prepare phase at its first air step, clip frames: its least and its most (exactness's driver, counted
/// 6.57-7.44 over the traced sprint jumps).
pub const SPRINT_PHASES: [f32; 2] = [6.57, 7.44];
/// Ground steps flown after the landing step (the climb onto the top takes 1-4, exactness §2 C; the rest show the body stays).
pub const GROUND_STEPS: u32 = 10;
/// A flight lands when the feet end within this of the landing floor's height, metres (the walk check's [`super::controlled`]
/// arrival band, `AT_HEIGHT_M`).
const LAND_HEIGHT_M: f32 = 0.2;

/// What the rim pass did ([`super::BuildStats`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RimStats {
    /// Jump links above [`SPRINT_JUMP_LOW_M`] flown.
    pub tried: usize,
    /// Of them, kept: every sprint press landed.
    pub kept: usize,
    /// Of the kept, those every walking press lands too ([`WalkGrid::jump_walks`]).
    pub walks: usize,
    /// Of them, given back to what the old cap made of the side (a lower jump, a drop or none).
    pub cut: usize,
    /// Flights flown.
    pub flights: usize,
    /// Rim gap links tried ([`WalkGrid::fly_rim_gaps`]) …
    pub gaps_tried: usize,
    /// … made as a short jump's (a walking press lands) …
    pub gaps_walked: usize,
    /// … and as a sprint jump's.
    pub gaps_sprinted: usize,
}

/// The sprint jump's gaits a bot's press may play: both legs, both ends of the prepare's phase.
fn sprint_gaits() -> [Gait; 4] {
    [
        Gait::Sprint { leg: Leg::Left, first_air_frame: SPRINT_PHASES[0] },
        Gait::Sprint { leg: Leg::Left, first_air_frame: SPRINT_PHASES[1] },
        Gait::Sprint { leg: Leg::Right, first_air_frame: SPRINT_PHASES[0] },
        Gait::Sprint { leg: Leg::Right, first_air_frame: SPRINT_PHASES[1] },
    ]
}

/// Whether every press a bot makes for a jump from the floor at `from` to the one at `to` along `way` (flat, unit) lands: pressed
/// on the first tick the take-off is `lead` m or nearer, so anywhere in the last step of `speed` before it ([`PRESSES`] presses),
/// each with each of `gaits`; and the flights flown (it stops at the first that does not land). A flight lands when the feet end
/// within [`LAND_HEIGHT_M`] of `to`'s height and no more than half a cell short of it along the way.
#[allow(clippy::too_many_arguments)]
fn all_land(world: &Collision, s: &Settings, from: Vec3, to: Vec3, way: (f32, f32), cell: f32, speed: f32, lead: f32, gaits: &[Gait]) -> (bool, usize) {
    let step = speed * jump::DT;
    let across = (to.x - from.x) * way.0 + (to.z - from.z) * way.1;
    let mut flights = 0;
    for k in 0..PRESSES {
        let before = lead - step * k as f32 / PRESSES as f32;
        let at = Vec3::new(from.x - way.0 * before, from.y, from.z - way.1 * before);
        for &gait in gaits {
            flights += 1;
            let landed = jump::run_and_fly(world, s, at, way, gait, GROUND_STEPS).is_some_and(|steps| {
                let end = steps.last().map_or(at, |f| f.feet);
                let ahead = (end.x - from.x) * way.0 + (end.z - from.z) * way.1;
                (end.y - to.y).abs() <= LAND_HEIGHT_M && ahead >= across - 0.5 * cell
            });
            if !landed {
                return (false, flights);
            }
        }
    }
    (true, flights)
}

/// The motor's press lead for a ledge jump at `speed`, metres ([`PRESS_LEAD_S`], [`PRESS_EXTRA_M`]).
fn ledge_lead(speed: f32) -> f32 {
    PRESS_LEAD_S * speed + PRESS_EXTRA_M
}

/// One rim ledge link's flights ([`WalkGrid::fly_rim_jumps`]): whether every sprint press lands, whether every walking press
/// does, and the flights flown.
fn fly_link(world: &Collision, s: &Settings, from: Vec3, to: Vec3, way: (f32, f32), cell: f32) -> (bool, bool, usize) {
    let (sprint, f1) = all_land(world, s, from, to, way, cell, jump::SPRINT_MS, ledge_lead(jump::SPRINT_MS), &sprint_gaits());
    let (walk, f2) = all_land(world, s, from, to, way, cell, jump::WALK_CAP_MS, ledge_lead(jump::WALK_CAP_MS), &[Gait::Walk]);
    (sprint, walk, f1 + f2)
}

/// The farthest a rim gap's landing face is searched from the take-off floor's centre, metres: the sprint clip's travel to its
/// apex (frame 14: [`gap::SPRINT_JUMP_AHEAD`]'s 2.65 at frame 14, PROVEN, the clip) from the nearest press a gap link makes
/// ([`gap::GAP_PRESS_MIN_M`] 0.6), and the capsule's radius (0.38, it meets the face with its side): a face farther is passed
/// while the clip still rises or never met before it comes down. Only bounds the search; each link is flown.
pub const RIM_GAP_ACROSS_M: f32 = 2.65 - gap::GAP_PRESS_MIN_M + 0.38;

/// A rim gap link's press ([`WalkGrid::fly_rim_gaps`]): the first lead (from [`gap::GAP_PRESS_MIN_M`] by [`gap::GAP_PRESS_STEP_M`]
/// to [`gap::GAP_PRESS_MAX_M`]) at which every walking press lands, as a short jump's press; else every sprint press, as a sprint
/// jump's; and the flights flown.
fn fly_gap(world: &Collision, s: &Settings, from: Vec3, to: Vec3, way: (f32, f32), cell: f32) -> (Option<Takeoff>, usize) {
    let leads = ((gap::GAP_PRESS_MAX_M - gap::GAP_PRESS_MIN_M) / gap::GAP_PRESS_STEP_M).round() as usize;
    let mut flights = 0;
    for (speed, gaits, walk) in [(jump::WALK_CAP_MS, &[Gait::Walk][..], true), (jump::SPRINT_MS, &sprint_gaits()[..], false)] {
        for i in 0..=leads {
            let lead = gap::GAP_PRESS_MIN_M + gap::GAP_PRESS_STEP_M * i as f32;
            let (ok, n) = all_land(world, s, from, to, way, cell, speed, lead, gaits);
            flights += n;
            if ok {
                let press = PressLead::from_metres(lead);
                return (Some(if walk { Takeoff::Press(press) } else { Takeoff::Sprint(press) }), flights);
            }
        }
    }
    (None, flights)
}

impl WalkGrid {
    /// [`Body::rim_jumps`]: flies every Jump link of `wide` that rises above [`SPRINT_JUMP_LOW_M`] on `world` and gives a link
    /// whose sprint presses do not all land back to what the old cap made of its side ([`Self::ledge_capped`] at the cap of a
    /// body without rim jumps). The kept are recorded as flown (`wide.flown`: the flight is their landing's proof) and, where
    /// the walking presses land too, as walked (`wide.jump_walks`).
    pub(super) fn fly_rim_jumps(&self, wide: &mut Wide, world: &Collision, s: &Settings) -> RimStats {
        let mut todo: Vec<(usize, usize, u32)> = Vec::new();
        for a in 0..self.height.len() {
            for d in 0..DIRS.len() {
                let b = wide.links[a][d];
                if b != NONE && wide.kinds[a][d] == LinkKind::Jump && self.height[b as usize] - self.height[a] > SPRINT_JUMP_LOW_M {
                    todo.push((a, d, b));
                }
            }
        }
        let flown = super::controlled::parallel(&todo, |&(a, d, b)| {
            let (pa, pb) = (self.position(a as NodeId), self.position(b));
            let (di, dk) = DIRS[d];
            let l = ((di * di + dk * dk) as f32).sqrt();
            fly_link(world, s, pa, pb, (di as f32 / l, dk as f32 / l), self.cell)
        });
        let mut stats = RimStats { tried: todo.len(), ..RimStats::default() };
        let old = Body { rim_jumps: false, ..self.body };
        for (&(a, d, b), &(sprint, walk, flights)) in todo.iter().zip(&flown) {
            stats.flights += flights;
            if sprint {
                stats.kept += 1;
                wide.flown.push((a as u32, b));
                if walk {
                    stats.walks += 1;
                    wide.jump_walks.push((a as u32, b));
                }
                continue;
            }
            stats.cut += 1;
            let (i, k) = self.cell_xz(self.cell_of[a] as usize);
            let (di, dk) = DIRS[d];
            let back = self.cell_index(i + di, k + dk).and_then(|cell| self.ledge_capped(a, cell, None, old.jump_cap()));
            (wide.links[a][d], wide.kinds[a][d]) = back.map_or((NONE, LinkKind::Walk), |(to, kind)| (to, kind));
        }
        wide.flown.sort_unstable();
        wide.jump_walks.sort_unstable();
        stats
    }

    /// [`Body::rim_jumps`] over a gap: from each floor along each way, across cells with no floor within a step of it, to the
    /// first cell no farther than [`RIM_GAP_ACROSS_M`] with a floor higher than the gap links reach (the body's standing jump,
    /// [`MARGINAL_JUMP_M`] at most) and no higher than [`RIM_REACH_M`]; flown at the gap links' presses ([`fly_gap`]) and made a
    /// GapJump with the first press that lands every time. After the gap links (`wide.off`, kept sorted); the flight is its
    /// landing's proof (`wide.flown`).
    pub(super) fn fly_rim_gaps(&self, wide: &mut Wide, world: &Collision, s: &Settings, stats: &mut RimStats) {
        let body = &self.body;
        let low = body.max_jump.min(MARGINAL_JUMP_M);
        let mut todo: Vec<(u32, u32, usize)> = Vec::new();
        for a in 0..self.height.len() {
            let ha = self.height[a];
            let (i, k) = self.cell_xz(self.cell_of[a] as usize);
            for (d, &(di, dk)) in DIRS.iter().enumerate() {
                let stride = self.cell * ((di * di + dk * dk) as f32).sqrt();
                // A side the floor walks on (or jumps onto, a cell away) is not a gap.
                if wide.links[a][d] != NONE && wide.kinds[a][d] != LinkKind::Drop {
                    continue;
                }
                let mut g = 1;
                while g as f32 * stride <= RIM_GAP_ACROSS_M + 1e-4 {
                    let Some(cell) = self.cell_index(i + g * di, k + dk * g) else { break };
                    let floors = self.start[cell]..self.start[cell + 1];
                    if g >= 2
                        && let Some(b) = floors.clone().find(|&f| (low..=RIM_REACH_M).contains(&(self.height[f as usize] - ha)) && self.height[f as usize] - ha > low)
                    {
                        if wide.off.binary_search_by_key(&(a as u32, b), |l| (l.from, l.to)).is_err() {
                            todo.push((a as u32, b, d));
                        }
                        break;
                    }
                    // A floor within a step goes on (no gap), one higher than the reach is a wall: either ends the search.
                    if floors.clone().any(|f| {
                        let rise = self.height[f as usize] - ha;
                        rise.abs() <= body.step || rise > RIM_REACH_M
                    }) {
                        break;
                    }
                    g += 1;
                }
            }
        }
        let flown = super::controlled::parallel(&todo, |&(a, b, d)| {
            let (di, dk) = DIRS[d];
            let l = ((di * di + dk * dk) as f32).sqrt();
            fly_gap(world, s, self.position(a), self.position(b), (di as f32 / l, dk as f32 / l), self.cell)
        });
        stats.gaps_tried += todo.len();
        for (&(a, b, _), &(press, flights)) in todo.iter().zip(&flown) {
            stats.flights += flights;
            let Some(press) = press else { continue };
            match press {
                Takeoff::Press(_) => stats.gaps_walked += 1,
                _ => stats.gaps_sprinted += 1,
            }
            wide.off.push(OffLink { from: a, to: b, kind: LinkKind::GapJump, press });
            wide.flown.push((a, b));
        }
        wide.off.sort_unstable_by_key(|l| (l.from, l.to));
        wide.flown.sort_unstable();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::{OffMeshLinks, STANDING_JUMP_M};
    use crate::{Triangle, WalkerBehaviour};

    fn quad(tris: &mut Vec<Triangle>, a: Vec3, b: Vec3, c: Vec3, d: Vec3) {
        tris.push(Triangle { v: [a, b, c], walker: WalkerBehaviour::BySlope });
        tris.push(Triangle { v: [a, c, d], walker: WalkerBehaviour::BySlope });
    }

    /// Ground at 0 for x < 5 and a block from x 5 to 12 whose top is `top` high, 6 m wide; a body starts at x 1. With rim jumps
    /// on: the rim pass's stats and whether the top is reached.
    fn onto(top: f32, rim_jumps: bool) -> (RimStats, bool) {
        let mut tris = Vec::new();
        quad(&mut tris, Vec3::new(-2.0, 0.0, 0.0), Vec3::new(-2.0, 0.0, 6.0), Vec3::new(5.0, 0.0, 6.0), Vec3::new(5.0, 0.0, 0.0));
        quad(&mut tris, Vec3::new(5.0, top, 0.0), Vec3::new(5.0, top, 6.0), Vec3::new(12.0, top, 6.0), Vec3::new(12.0, top, 0.0));
        quad(&mut tris, Vec3::new(5.0, 0.0, 0.0), Vec3::new(5.0, top, 0.0), Vec3::new(5.0, top, 6.0), Vec3::new(5.0, 0.0, 6.0));
        let body = Body { rim_jumps, exe_collision: true, start_witness: true, max_jump: STANDING_JUMP_M, ..Body::client() };
        let start = [Vec3::new(1.0, 0.0, 3.0)];
        let (grid, stats) = WalkGrid::build_kept_with_links(&tris, [-2.0, 0.0, 12.0, 6.0], 0.25, body, &start, &start, OffMeshLinks::None);
        let up = grid.locate(Vec3::new(9.0, top, 3.0), 0.3).is_some_and(|n| grid.reached(n));
        (stats.rim, up)
    }

    /// A 1.30 m top (over the sprint jump's lowest top, 1.28) is got onto head-on by every sprint press the motor makes: the
    /// 24 straight links along the 6 m face are kept (the diagonal ones, flown across the face at 45 degrees, are not) and the
    /// top is reached; without rim jumps no link is made. A 1.40 m top is landed only from presses 1.55 m and more before the
    /// face (the jump module's table), never from the motor's 0.89-1.14 m: every link is cut.
    #[test]
    fn a_rim_jump_is_kept_where_every_motor_press_lands() {
        let (low, up) = onto(1.30, true);
        assert!(low.kept == 24 && low.cut == low.tried - 24 && up, "1.30: {low:?}");
        let (_, up_off) = onto(1.30, false);
        assert!(!up_off, "no rim jumps: 1.30 is over the old cap");
        let (high, up_high) = onto(1.40, true);
        assert!(high.tried > 0 && high.kept == 0 && high.cut == high.tried && !up_high, "1.40: {high:?}");
    }

    /// Vector Laboratory's shelf, made: ground at 0 for x < 5, a pit 3 m deep across `gap` m, then a top 1.10 m up (over the
    /// standing jump, so no gap link reaches it). With rim jumps and the gap links: the rim gap pass's stats and whether the top
    /// is reached.
    fn over_a_gap(gap: f32, rim_jumps: bool) -> (RimStats, bool) {
        let (x1, top) = (5.0 + gap, 1.10);
        let mut tris = Vec::new();
        quad(&mut tris, Vec3::new(-2.0, 0.0, 0.0), Vec3::new(-2.0, 0.0, 6.0), Vec3::new(5.0, 0.0, 6.0), Vec3::new(5.0, 0.0, 0.0));
        quad(&mut tris, Vec3::new(5.0, -3.0, 0.0), Vec3::new(5.0, -3.0, 6.0), Vec3::new(x1, -3.0, 6.0), Vec3::new(x1, -3.0, 0.0));
        quad(&mut tris, Vec3::new(5.0, -3.0, 0.0), Vec3::new(5.0, 0.0, 0.0), Vec3::new(5.0, 0.0, 6.0), Vec3::new(5.0, -3.0, 6.0));
        quad(&mut tris, Vec3::new(x1, top, 0.0), Vec3::new(x1, top, 6.0), Vec3::new(12.0, top, 6.0), Vec3::new(12.0, top, 0.0));
        quad(&mut tris, Vec3::new(x1, -3.0, 0.0), Vec3::new(x1, top, 0.0), Vec3::new(x1, top, 6.0), Vec3::new(x1, -3.0, 6.0));
        let body = Body { rim_jumps, exe_collision: true, start_witness: true, max_jump: STANDING_JUMP_M, ..Body::client() };
        let start = [Vec3::new(1.0, 0.0, 3.0)];
        let (grid, stats) = WalkGrid::build_kept_with_links(&tris, [-2.0, 0.0, 12.0, 6.0], 0.25, body, &start, &start, OffMeshLinks::Gaps);
        let up = grid.locate(Vec3::new(9.0, top, 3.0), 0.3).is_some_and(|n| grid.reached(n));
        (stats.rim, up)
    }

    /// A 1.10 m top across a 1.25 m pit (the shelf's, 1.26 m and 1.1 m) is got onto: the rim gap pass makes links whose every
    /// press lands, and the top is reached; without rim jumps it is not.
    #[test]
    fn a_top_over_a_gap_higher_than_the_standing_jump_is_reached_by_its_flown_press() {
        let (rim, up) = over_a_gap(1.25, true);
        assert!(rim.gaps_walked + rim.gaps_sprinted > 0 && up, "{rim:?}");
        let (_, up_off) = over_a_gap(1.25, false);
        assert!(!up_off, "no rim jumps: no gap link reaches 1.10 m");
    }
}
