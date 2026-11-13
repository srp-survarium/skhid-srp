//! [A364 C2 rows 11/12] The rim jumps ([`Body::rim_jumps`]): jumps up a ledge, or across a gap, to a top higher than the Jump
//! and gap links reach, each flown on the exact tick ([`crate::controller::jump`]) over the presses a client can make, made a
//! GapJump link carrying the press that lands, and not made where none does.
//!
//! The client's air branch never steps up (`player_step` 0x7BF26: the jump branch has no `step_up`; 0.0000 m over the clip in
//! 2,245 flights): a top over the clip's apex is got onto when the capsule's round bottom comes to rest on its edge as the clip
//! comes down and the ground steps after `end_jump` climb the rest, and only from a run-up (exactness `notes-mapflights.md` §2 C,
//! Chemical Plant's deck: walking 44 of 396, sprint 143 of 424, standing 0 of 396). Whether it does depends on the edge's
//! shape and where the jump is pressed, so no rule of heights says it: each link is flown.
//!
//! The presses ([`best_press`]): the short jump from a walk (3.5 m/s) and the sprint jump (4.9587 m/s, both legs and both ends
//! of the prepare's phase, which the run cycle picks, [`SPRINT_PHASES`]), each from the exactness rig's run-up
//! ([`jump::run_up`]: up to speed over the 0.3 s blend then 4 steady steps, else shorter) and pressed at each of the gap links'
//! leads ([`gap_leads`], the built file's range) before the take-off floor's centre. A link is made when some press lands (the
//! owner, 2026-11-13: the bots are taught the press, the path is not taken away), with the press that lands the most of its
//! gait's variants: a walking jump where one lands, else a sprint jump. Its press is the link's own ([`Takeoff`]), as every
//! gap link's: the motor presses there (`Leap::Gap`, `Leap::SprintGap`). Made after the erosion, from the floors a body stands
//! on, and before the reach, so a top only a rim jump gets to is kept.
use super::gap::{self, OffLink, PressLead, Takeoff};
use super::{LinkKind, NodeId, WalkGrid, Wide, DIRS, MARGINAL_JUMP_M, NONE, RIM_REACH_M};
use crate::controller::jump::{self, Gait, Leg};
use crate::controller::tick::Settings;
use crate::controller::Collision;
use crate::math::Vec3;

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
    /// Rim jumps flown …
    pub tried: usize,
    /// … made as a short jump's (a walking press lands) …
    pub walked: usize,
    /// … and as a sprint jump's; the rest no press made.
    pub sprinted: usize,
    /// Flights flown.
    pub flights: usize,
}

/// The sprint jump's gaits a press may play: both legs, both ends of the prepare's phase.
fn sprint_gaits() -> [Gait; 4] {
    [
        Gait::Sprint { leg: Leg::Left, first_air_frame: SPRINT_PHASES[0] },
        Gait::Sprint { leg: Leg::Left, first_air_frame: SPRINT_PHASES[1] },
        Gait::Sprint { leg: Leg::Right, first_air_frame: SPRINT_PHASES[0] },
        Gait::Sprint { leg: Leg::Right, first_air_frame: SPRINT_PHASES[1] },
    ]
}

/// Whether a jump of `gait` pressed `lead` m before the take-off floor's centre at `from`, toward the floor at `to` along `way`
/// (flat, unit), lands: the feet end within [`LAND_HEIGHT_M`] of `to`'s height and no more than half a cell short of it along
/// the way.
#[allow(clippy::too_many_arguments)]
fn lands(world: &Collision, s: &Settings, from: Vec3, to: Vec3, way: (f32, f32), cell: f32, lead: f32, gait: Gait) -> bool {
    let across = (to.x - from.x) * way.0 + (to.z - from.z) * way.1;
    let at = Vec3::new(from.x - way.0 * lead, from.y, from.z - way.1 * lead);
    jump::run_and_fly(world, s, at, way, gait, GROUND_STEPS).is_some_and(|steps| {
        let end = steps.last().map_or(at, |f| f.feet);
        let ahead = (end.x - from.x) * way.0 + (end.z - from.z) * way.1;
        (end.y - to.y).abs() <= LAND_HEIGHT_M && ahead >= across - 0.5 * cell
    })
}

/// The press that gets a body from `from` to `to` along `way`, among the leads `leads` (metres before the take-off): a walking
/// jump at the lead it lands from, else the sprint jump at the lead where the most of its four variants land (the nearest to
/// the middle of the leads that do on a tie); None when no press lands. And the flights flown (a lead where every variant
/// lands ends the search: no press is better).
#[allow(clippy::too_many_arguments)]
fn best_press(world: &Collision, s: &Settings, from: Vec3, to: Vec3, way: (f32, f32), cell: f32, leads: &[f32]) -> (Option<Takeoff>, usize) {
    let mut flights = 0;
    let walking: Vec<f32> = leads
        .iter()
        .copied()
        .filter(|&lead| {
            flights += 1;
            lands(world, s, from, to, way, cell, lead, Gait::Walk)
        })
        .collect();
    if !walking.is_empty() {
        return (Some(Takeoff::Press(PressLead::from_metres(walking[walking.len() / 2]))), flights);
    }
    let mut counts: Vec<(f32, usize)> = Vec::new();
    for &lead in leads {
        let n = sprint_gaits()
            .into_iter()
            .filter(|&gait| {
                flights += 1;
                lands(world, s, from, to, way, cell, lead, gait)
            })
            .count();
        counts.push((lead, n));
        if n == sprint_gaits().len() {
            return (Some(Takeoff::Sprint(PressLead::from_metres(lead))), flights);
        }
    }
    let most = counts.iter().map(|&(_, n)| n).max().unwrap_or(0);
    if most == 0 {
        return (None, flights);
    }
    // Among the leads with the most landings, the one nearest the middle of the leads that land at all.
    let landing: Vec<f32> = counts.iter().filter(|&&(_, n)| n > 0).map(|&(lead, _)| lead).collect();
    let mid = landing.iter().sum::<f32>() / landing.len() as f32;
    let lead = counts
        .iter()
        .filter(|&&(_, n)| n == most)
        .map(|&(lead, _)| lead)
        .min_by(|a, b| (a - mid).abs().total_cmp(&(b - mid).abs()))
        .unwrap_or(mid);
    (Some(Takeoff::Sprint(PressLead::from_metres(lead))), flights)
}

/// The leads a gap link may carry: [`gap::GAP_PRESS_MIN_M`] to [`gap::GAP_PRESS_MAX_M`] by [`gap::GAP_PRESS_STEP_M`] (the built
/// file's range, `PressLead::is_valid`).
fn gap_leads() -> Vec<f32> {
    let n = ((gap::GAP_PRESS_MAX_M - gap::GAP_PRESS_MIN_M) / gap::GAP_PRESS_STEP_M).round() as usize;
    (0..=n).map(|i| gap::GAP_PRESS_MIN_M + gap::GAP_PRESS_STEP_M * i as f32).collect()
}

/// The farthest a rim gap's landing face is searched from the take-off floor's centre, metres: the sprint clip's travel to its
/// apex (2.65 m, frame 14, PROVEN, the clip) from the nearest press a gap link makes ([`gap::GAP_PRESS_MIN_M`] 0.6), and the
/// capsule's radius (0.38, it meets the face with its side): a face farther is not met before the clip comes down. Only bounds
/// the search; each link is flown.
pub const RIM_GAP_ACROSS_M: f32 = 2.65 - gap::GAP_PRESS_MIN_M + 0.38;

impl WalkGrid {
    /// [`Body::rim_jumps`]: from each floor along each way whose side has no link up (none, or a drop), across cells with no
    /// floor within a step of it, to the first cell no farther than [`RIM_GAP_ACROSS_M`] with a floor higher than a gap link
    /// reaches (the body's standing jump, [`MARGINAL_JUMP_M`] at most) and no higher than [`RIM_REACH_M`] (a Jump link up a
    /// ledge goes to [`super::SPRINT_JUMP_LOW_M`]; one there takes the side, so a rim jump is above it); flown over the presses
    /// ([`best_press`]) and made a GapJump with the press that lands. After the gap links (`wide.off`, kept sorted). Returns
    /// the stats and the jumps no press made, as (take-off, landing) (none was a link before).
    pub(super) fn fly_rim_jumps(&self, wide: &mut Wide, world: &Collision, s: &Settings) -> (RimStats, Vec<(NodeId, NodeId)>) {
        let body = &self.body;
        let low = body.max_jump.min(MARGINAL_JUMP_M);
        let mut todo: Vec<(u32, u32, usize)> = Vec::new();
        for a in 0..self.height.len() {
            let ha = self.height[a];
            let (i, k) = self.cell_xz(self.cell_of[a] as usize);
            for (d, &(di, dk)) in DIRS.iter().enumerate() {
                let stride = self.cell * ((di * di + dk * dk) as f32).sqrt();
                if wide.links[a][d] != NONE && wide.kinds[a][d] != LinkKind::Drop {
                    continue;
                }
                let mut g = 1;
                while g as f32 * stride <= RIM_GAP_ACROSS_M + 1e-4 {
                    let Some(cell) = self.cell_index(i + g * di, k + g * dk) else { break };
                    let floors = self.start[cell]..self.start[cell + 1];
                    if let Some(b) = floors.clone().find(|&f| {
                        let rise = self.height[f as usize] - ha;
                        rise > low && rise <= RIM_REACH_M
                    }) {
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
        let leads = gap_leads();
        let flown = super::controlled::parallel(&todo, |&(a, b, d)| {
            let (di, dk) = DIRS[d];
            let l = ((di * di + dk * dk) as f32).sqrt();
            best_press(world, s, self.position(a), self.position(b), (di as f32 / l, dk as f32 / l), self.cell, &leads)
        });
        let mut stats = RimStats { tried: todo.len(), ..RimStats::default() };
        let mut none = Vec::new();
        for (&(a, b, _), &(press, flights)) in todo.iter().zip(&flown) {
            stats.flights += flights;
            let Some(press) = press else {
                none.push((a, b));
                continue;
            };
            match press {
                Takeoff::Press(_) => stats.walked += 1,
                _ => stats.sprinted += 1,
            }
            wide.off.push(OffLink { from: a, to: b, kind: LinkKind::GapJump, press });
        }
        wide.off.sort_unstable_by_key(|l| (l.from, l.to));
        (stats, none)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::{Body, OffMeshLinks, STANDING_JUMP_M};
    use crate::{Triangle, WalkerBehaviour};

    fn quad(tris: &mut Vec<Triangle>, a: Vec3, b: Vec3, c: Vec3, d: Vec3) {
        tris.push(Triangle { v: [a, b, c], walker: WalkerBehaviour::BySlope });
        tris.push(Triangle { v: [a, c, d], walker: WalkerBehaviour::BySlope });
    }

    fn body(rim_jumps: bool) -> Body {
        Body { rim_jumps, exe_collision: true, start_witness: true, max_jump: STANDING_JUMP_M, ..Body::client() }
    }

    /// Ground at 0 for x < 5 and a block from x 5 to 12 whose top is `top` high, 6 m wide; a body starts at x 1: the grid, its
    /// rim stats and whether the top is reached.
    fn onto(top: f32, rim_jumps: bool) -> (WalkGrid, RimStats, bool) {
        let mut tris = Vec::new();
        quad(&mut tris, Vec3::new(-2.0, 0.0, 0.0), Vec3::new(-2.0, 0.0, 6.0), Vec3::new(5.0, 0.0, 6.0), Vec3::new(5.0, 0.0, 0.0));
        quad(&mut tris, Vec3::new(5.0, top, 0.0), Vec3::new(5.0, top, 6.0), Vec3::new(12.0, top, 6.0), Vec3::new(12.0, top, 0.0));
        quad(&mut tris, Vec3::new(5.0, 0.0, 0.0), Vec3::new(5.0, top, 0.0), Vec3::new(5.0, top, 6.0), Vec3::new(5.0, 0.0, 6.0));
        let start = [Vec3::new(1.0, 0.0, 3.0)];
        let (grid, stats) = WalkGrid::build_kept_with_links(&tris, [-2.0, 0.0, 12.0, 6.0], 0.25, body(rim_jumps), &start, &start, OffMeshLinks::Gaps);
        let up = grid.locate(Vec3::new(9.0, top, 3.0), 0.3).is_some_and(|n| grid.reached(n));
        (grid, stats.rim, up)
    }

    /// A 1.30 m top (over the sprint jump's lowest top, 1.28) and a 1.40 m one are got onto by some client press: rim jumps are
    /// made with the press that lands (a gap link's, in the built file too) and the tops are reached; without rim jumps they are
    /// not. A 2.10 m top no press gets onto: no rim jump is made, and the build reports every one it flew.
    #[test]
    fn a_rim_jump_is_made_with_a_press_that_lands_and_not_where_none_does() {
        for top in [1.30, 1.40] {
            let (grid, rim, up) = onto(top, true);
            assert!(rim.walked + rim.sprinted > 0 && up, "{top}: {rim:?}");
            assert_eq!(grid.rim_unmade().len(), rim.tried - rim.walked - rim.sprinted, "{top}");
            let path = std::env::temp_dir().join(format!("srp-rim-{}-{top}.bin", std::process::id()));
            grid.write(&path, 11).unwrap();
            let back = WalkGrid::read(&path, 11, body(true)).unwrap();
            std::fs::remove_file(&path).ok();
            assert_eq!(back.digest(), grid.digest());
            let (_, _, up_off) = onto(top, false);
            assert!(!up_off, "{top}: no rim jumps, over the old cap");
        }
        let (grid, rim, up) = onto(2.10, true);
        assert!(rim.tried > 0 && rim.walked + rim.sprinted == 0 && !up, "2.10: {rim:?}");
        assert_eq!(grid.rim_unmade().len(), rim.tried);
    }

    /// Vector Laboratory's shelf, made: ground at 0 for x < 5, a pit 3 m deep across `gap` m, then a top 1.10 m up (over the
    /// standing jump, so no gap link reaches it). With rim jumps and the gap links: the rim stats and whether the top is reached.
    fn over_a_gap(gap: f32, rim_jumps: bool) -> (RimStats, bool) {
        let (x1, top) = (5.0 + gap, 1.10);
        let mut tris = Vec::new();
        quad(&mut tris, Vec3::new(-2.0, 0.0, 0.0), Vec3::new(-2.0, 0.0, 6.0), Vec3::new(5.0, 0.0, 6.0), Vec3::new(5.0, 0.0, 0.0));
        quad(&mut tris, Vec3::new(5.0, -3.0, 0.0), Vec3::new(5.0, -3.0, 6.0), Vec3::new(x1, -3.0, 6.0), Vec3::new(x1, -3.0, 0.0));
        quad(&mut tris, Vec3::new(5.0, -3.0, 0.0), Vec3::new(5.0, 0.0, 0.0), Vec3::new(5.0, 0.0, 6.0), Vec3::new(5.0, -3.0, 6.0));
        quad(&mut tris, Vec3::new(x1, top, 0.0), Vec3::new(x1, top, 6.0), Vec3::new(12.0, top, 6.0), Vec3::new(12.0, top, 0.0));
        quad(&mut tris, Vec3::new(x1, -3.0, 0.0), Vec3::new(x1, top, 0.0), Vec3::new(x1, top, 6.0), Vec3::new(x1, -3.0, 6.0));
        let start = [Vec3::new(1.0, 0.0, 3.0)];
        let (grid, stats) = WalkGrid::build_kept_with_links(&tris, [-2.0, 0.0, 12.0, 6.0], 0.25, body(rim_jumps), &start, &start, OffMeshLinks::Gaps);
        let up = grid.locate(Vec3::new(9.0, top, 3.0), 0.3).is_some_and(|n| grid.reached(n));
        (stats.rim, up)
    }

    /// A 1.10 m top across a 1.25 m pit (the shelf's, 1.26 m and 1.1 m) is got onto: the rim gap pass makes links with a press
    /// that lands, and the top is reached; without rim jumps it is not.
    #[test]
    fn a_top_over_a_gap_higher_than_the_standing_jump_is_reached_by_its_flown_press() {
        let (rim, up) = over_a_gap(1.25, true);
        assert!(rim.walked + rim.sprinted > 0 && up, "{rim:?}");
        let (_, up_off) = over_a_gap(1.25, false);
        assert!(!up_off, "no rim jumps: no gap link reaches 1.10 m");
    }
}
