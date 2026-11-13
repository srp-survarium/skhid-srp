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
use crate::controller::jump::{self, Gait, Keys, Leg, Press};
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
    /// Of the made, those whose presses land on another floor of the top than the one tried: the link goes to the floor reached.
    pub retargeted: usize,
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

impl WalkGrid {
    /// The floor under `feet`: in the feet's own cell (the feet within half a cell of its centre on both axes, the grid's own
    /// test), the one within [`LAND_HEIGHT_M`] of the feet's height, the nearest in height.
    fn floor_under(&self, feet: Vec3) -> Option<NodeId> {
        let i = ((feet.x - self.x0) / self.cell).floor() as i32;
        let k = ((feet.z - self.z0) / self.cell).floor() as i32;
        let cell = self.cell_index(i, k)?;
        (self.start[cell]..self.start[cell + 1])
            .filter(|&f| (self.height[f as usize] - feet.y).abs() <= LAND_HEIGHT_M)
            .min_by(|&p, &q| (self.height[p as usize] - feet.y).abs().total_cmp(&(self.height[q as usize] - feet.y).abs()))
    }

    /// The floor a flight's `steps` ([`jump::fly`]) landed on, higher than `over` m (the take-off's height and what its Jump and
    /// gap links reach, [`MARGINAL_JUMP_M`] at most: the rim jumps' own range): after the jump (its first step in the air), the
    /// first step out of the jump on which the client has the body on the ground (`on_ground` 0x7EF90, the game's "can jump",
    /// [`jump::FlightStep::on_ground`]) over such a floor ([`Self::floor_under`]: the feet's own cell). The client's own landing,
    /// on any step: not where the flight's straight walk on ends, since a bot that has landed follows its route (Vector
    /// Laboratory's ledge by the shelf, (23.76, 25.39, -28.8), is landed on, then walked off its far end by a straight walk); and on
    /// the floor the body is on, whichever of the top it is (Vector Laboratory's (-23.72, 23.18, -22.0): a jump meant for the floor
    /// at 23.49 stops on the one at 23.18 beside it, part of the same top).
    fn landed_on(&self, steps: &[jump::FlightStep], over: f32) -> Option<NodeId> {
        let first_air = steps.iter().position(|f| f.in_jump)?;
        steps[first_air..]
            .iter()
            .filter(|f| !f.in_jump && f.on_ground)
            .find_map(|f| self.floor_under(f.feet).filter(|&g| self.height[g as usize] > over))
    }

    /// The press that gets a body from the take-off floor `a` up toward the floor `b` along `way`, among the leads `leads` (metres
    /// before the take-off), and the floor it lands on ([`Self::landed_on`]: a floor over what `a`'s links reach): on `b` itself
    /// when a press does (a walking jump at the
    /// middle of the leads it lands there from, else the sprint jump at the lead where the most of its four variants do, the
    /// nearest the middle on a tie); else on the floor the presses land on the most (re-targeted: the link goes where the body
    /// lands); None when no press lands on such a floor. And the flights flown (a lead where every sprint variant lands on
    /// `b` ends the search).
    fn best_press(&self, world: &Collision, s: &Settings, a: NodeId, b: NodeId, way: (f32, f32), leads: &[f32]) -> (Option<(NodeId, Takeoff)>, usize) {
        let from = self.position(a);
        let top = from.y + self.body.max_jump.min(MARGINAL_JUMP_M);
        let mut flights = 0;
        // Each press's landing floor: (lead, walking, floor).
        let mut landings: Vec<(f32, bool, NodeId)> = Vec::new();
        for &lead in leads {
            flights += 1;
            let at = Vec3::new(from.x - way.0 * lead, from.y, from.z - way.1 * lead);
            if let Some(f) = jump::run_and_fly(world, s, at, way, Gait::Walk, GROUND_STEPS).and_then(|st| self.landed_on(&st, top)) {
                landings.push((lead, true, f));
            }
        }
        // The walking presses' leads that land on `to`.
        fn walked_on(landings: &[(f32, bool, NodeId)], to: NodeId) -> Vec<f32> {
            landings.iter().filter(|&&(_, w, f)| w && f == to).map(|&(l, ..)| l).collect()
        }
        let on_b = walked_on(&landings, b);
        if !on_b.is_empty() {
            return (Some((b, Takeoff::Press(PressLead::from_metres(on_b[on_b.len() / 2])))), flights);
        }
        for &lead in leads {
            // One run-up for the four variants: it is the same for each ([`jump::run_and_fly`] runs it from the same start at the
            // same speed), and only the jump differs.
            let at = Vec3::new(from.x - way.0 * lead, from.y, from.z - way.1 * lead);
            let run = jump::run_up(world, s, at, way, jump::SPRINT_MS, true);
            let mut on_this = 0;
            for gait in sprint_gaits() {
                flights += 1;
                let (Some(run), Gait::Sprint { leg, first_air_frame }) = (run.as_ref(), gait) else { continue };
                let press =
                    if run.speed >= jump::WALK_CAP_MS { Press::Sprint { speed: run.speed, leg, first_air_frame } } else { Press::Walk { speed: run.speed } };
                let mut body = run.body;
                if let Some(f) = self.landed_on(&jump::fly(world, s, &mut body, way, press, Keys::BOT, GROUND_STEPS), top) {
                    landings.push((lead, false, f));
                    on_this += usize::from(f == b);
                }
            }
            if on_this == sprint_gaits().len() {
                return (Some((b, Takeoff::Sprint(PressLead::from_metres(lead)))), flights);
            }
        }
        // The sprint press onto `to`: the lead with the most variants landing there, the nearest the middle of those that land.
        let sprint_onto = |to: NodeId| -> Option<f32> {
            let leads_on: Vec<f32> = landings.iter().filter(|&&(_, w, f)| !w && f == to).map(|&(l, ..)| l).collect();
            let mid = leads_on.iter().sum::<f32>() / leads_on.len().max(1) as f32;
            let count = |l: f32| leads_on.iter().filter(|&&x| x == l).count();
            leads_on.iter().copied().max_by(|&p, &q| count(p).cmp(&count(q)).then((q - mid).abs().total_cmp(&(p - mid).abs())))
        };
        if let Some(lead) = sprint_onto(b) {
            return (Some((b, Takeoff::Sprint(PressLead::from_metres(lead)))), flights);
        }
        // Re-targeted: the floor of the top the presses land on the most (a walking press's first).
        let mut floors: Vec<NodeId> = landings.iter().map(|&(.., f)| f).collect();
        floors.sort_unstable();
        floors.dedup();
        let most = |walking: bool| {
            floors.iter().copied().filter(|&f| landings.iter().any(|&(_, w, g)| w == walking && g == f)).max_by_key(|&f| {
                (landings.iter().filter(|&&(_, w, g)| w == walking && g == f).count(), std::cmp::Reverse(f))
            })
        };
        if let Some(f) = most(true) {
            let on = walked_on(&landings, f);
            return (Some((f, Takeoff::Press(PressLead::from_metres(on[on.len() / 2])))), flights);
        }
        if let Some(f) = most(false) {
            return (sprint_onto(f).map(|lead| (f, Takeoff::Sprint(PressLead::from_metres(lead)))), flights);
        }
        (None, flights)
    }
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
            self.best_press(world, s, a, b, (di as f32 / l, dk as f32 / l), &leads)
        });
        let mut stats = RimStats { tried: todo.len(), ..RimStats::default() };
        let mut none = Vec::new();
        for (&(a, b, _), &(made, flights)) in todo.iter().zip(&flown) {
            stats.flights += flights;
            let Some((to, press)) = made else {
                none.push((a, b));
                continue;
            };
            match press {
                Takeoff::Press(_) => stats.walked += 1,
                _ => stats.sprinted += 1,
            }
            stats.retargeted += usize::from(to != b);
            wide.off.push(OffLink { from: a, to, kind: LinkKind::GapJump, press });
        }
        // Two candidates of one take-off may land on the same floor: one link (the first, by the way's order).
        wide.off.sort_by_key(|l| (l.from, l.to));
        wide.off.dedup_by_key(|l| (l.from, l.to));
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

    /// One rim jump on a real map, every press's outcome printed: `RIM_MAP` (a format 6 map), `RIM_FROM` and `RIM_TO` ("x z y",
    /// the take-off and landing floors). `cargo test -- --ignored --nocapture fly_one`.
    #[test]
    #[ignore]
    fn fly_one() {
        let var = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("set {k}"));
        let map = crate::MapData::from_bytes(&std::fs::read(var("RIM_MAP")).unwrap()).unwrap();
        let world = Collision::build_exe(&map).unwrap();
        let s = body(true).controller().1;
        let p = |k: &str| {
            let v: Vec<f32> = var(k).split_whitespace().map(|x| x.parse().unwrap()).collect();
            Vec3::new(v[0], v[2], v[1])
        };
        let (from, to) = (p("RIM_FROM"), p("RIM_TO"));
        let l = (to.x - from.x).hypot(to.z - from.z);
        let way = ((to.x - from.x) / l, (to.z - from.z) / l);
        for lead in gap_leads() {
            for gait in std::iter::once(Gait::Walk).chain(sprint_gaits()) {
                let at = Vec3::new(from.x - way.0 * lead, from.y, from.z - way.1 * lead);
                let Some(steps) = jump::run_and_fly(&world, &s, at, way, gait, GROUND_STEPS) else {
                    println!("lead {lead:.1} {gait:?}: no run-up");
                    continue;
                };
                let e = steps.last().unwrap().feet;
                let trace: Vec<String> = steps.iter().map(|f| format!("{:.2},{:.2},{:.2}", f.feet.x, f.feet.y, f.feet.z)).collect();
                println!("lead {lead:.1} {gait:?}: end ({:.3} {:.3} {:.3}) | {}", e.x, e.y, e.z, trace.join(" "));
            }
        }
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
