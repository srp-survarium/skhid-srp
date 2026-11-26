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
use super::memo::{Codec, Memo, MemoCodec, MemoInput, Mode, Site, StableIds, Windows};
use super::rim::{gap_leads, Riser, OWN_TOP_FLAT_M};
use super::{LinkKind, WalkGrid, Wide, DIRS, NONE};
use crate::controller::tick::{Controller, Settings};
use crate::motion::{self, ClientBody, Input, Kin, Pass};
use crate::controller::Collision;
use crate::digest::Digest128;
use crate::math::Vec3;

/// A link rising more than this is judged, m: the walk check's riser ([`super::controlled::CUT_RISE_M`]).
pub const RISE_M: f32 = super::controlled::CUT_RISE_M;
/// The exe's walk: the ticks the body stands to settle first …
pub const SETTLE_TICKS: usize = 20;
/// … and the ticks it walks.
pub const WALK_TICKS: usize = 40;
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
/// writes it), then walked toward `to` for [`WALK_TICKS`] ticks, gets onto `to`'s floor at any walk tick: its feet within
/// [`ONTO_DY_M`] of `to`'s height and [`ONTO_FLAT_M`] across. [body physics S6b] The body is the client's ([`ClientBody`]): the
/// settle with no keys, the walk with the forward key and the yaw along the way, its speed the client's blend from rest (the keys'
/// first step does not move, then up over six ticks). On the 57 links of the module's note it gives the direct walk's verdict
/// on every one (S6b-0, `s6b0-gaitfeed`).
pub fn exe_walk(world: &Collision, s: &Settings, from: Vec3, to: Vec3) -> bool {
    let flat = Vec3::new(to.x - from.x, 0.0, to.z - from.z);
    let length = flat.length();
    if length < 1e-4 {
        return true;
    }
    let yaw = motion::yaw_of((flat.x / length, flat.z / length));
    let mut c = Controller::standing_at(from);
    c.velocity = Vec3::new(0.0, -0.01, 0.0);
    let pass = Pass { world, settings: s };
    let mut b = ClientBody::of(c, yaw);
    for _ in 0..SETTLE_TICKS {
        b.tick(&pass, Kin::CLIENT, Input { keys: 0, yaw });
    }
    (0..WALK_TICKS).any(|_| {
        let feet = b.tick(&pass, Kin::CLIENT, Input { keys: motion::keys::FORWARD, yaw }).feet;
        (feet.y - to.y).abs() <= ONTO_DY_M && feet.distance_xz(to) <= ONTO_FLAT_M
    })
}

/// A link's verdict ([`WalkGrid::walk_gait_links`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum GaitVerdict {
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
    pub(super) fn walk_gait_links(&self, wide: &mut Wide, world: &Collision, s: &Settings, memo: &mut Memo) -> WalkGaitStats {
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
        let verdicts = self.walk_gait_memo(world, s, start, &todo, memo);
        let mut stats = WalkGaitStats { judged: todo.len(), ..WalkGaitStats::default() };
        for (&(a, b, _), v) in todo.iter().zip(&verdicts) {
            match *v {
                GaitVerdict::Walks => {}
                GaitVerdict::ExeWalks => {
                    stats.unwalked += 1;
                    stats.exe_walked += 1;
                }
                GaitVerdict::Pressed(riser) => {
                    stats.unwalked += 1;
                    stats.pressed += 1;
                    wide.riser.push((a, b, riser));
                }
                GaitVerdict::Unwalked => {
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

    /// [body physics S6c] The verdicts of `todo` (take-off, top, way) through the memo's site `walk.gait` ([`walk_gait_job`]), each
    /// at its link's place. Its window is the rim line site's ([`WalkGrid::flight_reach_m`]: the press flights read the floors
    /// that far, the walks only the two floors' places); with the memo on, a press point or a flight past it stops the build.
    fn walk_gait_memo(&self, world: &Collision, s: &Settings, start: super::controlled::WalkStart, todo: &[(u32, u32, usize)], memo: &mut Memo) -> Vec<GaitVerdict> {
        let on = memo.mode() != Mode::Off;
        let reach_m = self.flight_reach_m();
        let window_cells = ((reach_m + self.body.radius + OWN_TOP_FLAT_M + self.cell) / self.cell).ceil() as i64;
        let windows = on.then(|| Windows::of_floors(self));
        let leads = gap_leads();
        if on {
            let n = crate::controller::jump::neighbourhood();
            let press_m = leads.iter().copied().fold(0.0f32, f32::max)
                + n.iter().map(|n| n.lead.abs()).fold(0.0f32, f32::max)
                + n.iter().map(|n| n.side.abs()).fold(0.0f32, f32::max);
            assert!(press_m <= reach_m, "memo: ERROR site walk.gait: a press point {press_m} m from its take-off, past the window's {reach_m} m");
        }
        let items: Vec<WalkGaitItem> = todo
            .iter()
            .map(|&(a, b, d)| WalkGaitItem {
                a,
                b,
                d,
                window: windows.as_ref().map_or(0, |w| w.around(self.cell_of[a as usize] as usize, window_cells)),
            })
            .collect();
        let kits = crate::controller::jump::Kits { body: crate::controller::jump::WALK_CAP_MS, fastest: self.body.fastest_walk_cap };
        let ctx = WalkGaitCtx { leads, kits, reach_m, window_cells, grid: self, world, s, start };
        let site = Site { id: "walk.gait", source: crate::source::site("walk.gait") };
        let out = memo.run(site, self, &ctx, &items, walk_gait_key, walk_gait_job);
        if on {
            let farthest = out.iter().map(|o| o.reach_m).fold(0.0f32, f32::max);
            eprintln!("memo: walk.gait: {} links, the farthest flight {farthest:.2} m from its take-off (window {reach_m} m, {window_cells} cells)", out.len());
            assert!(farthest <= reach_m, "memo: ERROR site walk.gait: a flight {farthest} m from its take-off, past the window's {reach_m} m");
        }
        out.into_iter().map(|o| o.verdict).collect()
    }
}

/// [body physics S6c] One job of the memo site `walk.gait` ([`walk_gait_job`]): the low riser (take-off `a`, top `b`, way `d`)
/// and the digest of the floors in its window ([`Windows::of_floors`]).
#[derive(Clone, Debug)]
pub(super) struct WalkGaitItem {
    a: super::NodeId,
    b: super::NodeId,
    d: usize,
    window: u128,
}

crate::memo_ctx! {
    /// [body physics S6c] The walk gait site's context: the press leads, the walk caps, the reach and the window (hashed), and the
    /// grid, the world, the tick's settings and the start, read through the items' windows and the memo's global inputs.
    pub(super) struct WalkGaitCtx<'a> {
        leads: Vec<f32>,
        kits: crate::controller::jump::Kits,
        reach_m: f32,
        window_cells: i64,
    } through {
        grid: &'a WalkGrid,
        world: &'a Collision,
        s: &'a Settings,
        start: super::controlled::WalkStart,
    }
}

/// [body physics S6c] A walk gait job's result: the verdict, and how far from the take-off its press flights went (the window's
/// check; 0 when no press was flown).
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct WalkGaitOut {
    verdict: GaitVerdict,
    reach_m: f32,
}

/// [body physics S6c] The walk gait site's key of a job: its floors by their stable ids, its way and its window.
fn walk_gait_key(_: &WalkGaitCtx<'_>, ids: &StableIds, item: &WalkGaitItem) -> u128 {
    let mut d = Digest128::default();
    ids.id(item.a).feed(&mut d);
    ids.id(item.b).feed(&mut d);
    d.u64(item.d as u64);
    d.u128(item.window);
    d.finish()
}

/// [body physics S6c] The walk gait site's job (module documentation's three steps): the walk check's test, the exe's walk, the
/// risers' flight.
fn walk_gait_job(ctx: &WalkGaitCtx<'_>, item: &WalkGaitItem) -> WalkGaitOut {
    let grid = ctx.grid;
    let (pa, pb) = (grid.position(item.a), grid.position(item.b));
    if walk_share(ctx.world, ctx.s, ctx.start, pa, pb).1 {
        return WalkGaitOut { verdict: GaitVerdict::Walks, reach_m: 0.0 };
    }
    // The exe's walk from the take-off's centre as the rig puts it (not moved out of the walls as the walk check's start).
    if exe_walk(ctx.world, ctx.s, pa, pb) {
        return WalkGaitOut { verdict: GaitVerdict::ExeWalks, reach_m: 0.0 };
    }
    let reach = std::cell::Cell::new(0.0f32);
    let verdict = match grid.walk_gait_press(ctx.world, ctx.s, item.a, item.b, item.d, &reach) {
        Some(riser) => GaitVerdict::Pressed(riser),
        None => GaitVerdict::Unwalked,
    };
    WalkGaitOut { verdict, reach_m: reach.get() }
}

/// [body physics S6c] The walk gait site's results as the memo stores them: every field.
impl MemoCodec for WalkGaitOut {
    const TAG: u32 = 0x5747_4f01;
    fn encode(&self, out: &mut Codec<'_>) {
        match self.verdict {
            GaitVerdict::Walks => out.put_u8(0),
            GaitVerdict::ExeWalks => out.put_u8(1),
            GaitVerdict::Unwalked => out.put_u8(2),
            GaitVerdict::Pressed(Riser::Pressed { press, share }) => {
                out.put_u8(3);
                press.encode(out);
                share.encode(out);
            }
            GaitVerdict::Pressed(Riser::NoPress) => out.put_u8(4),
        }
        out.put_f32(self.reach_m);
    }
    fn decode(input: &mut Codec<'_>) -> Option<Self> {
        let verdict = match input.u8()? {
            0 => GaitVerdict::Walks,
            1 => GaitVerdict::ExeWalks,
            2 => GaitVerdict::Unwalked,
            3 => GaitVerdict::Pressed(Riser::Pressed { press: MemoCodec::decode(input)?, share: MemoCodec::decode(input)? }),
            4 => GaitVerdict::Pressed(Riser::NoPress),
            _ => return None,
        };
        Some(WalkGaitOut { verdict, reach_m: input.f32()? })
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

    /// [body physics S6c] The walk check and the walk gait through the memo build the full build's grid, written, used and
    /// verified: the 0.3 m step (its low risers are judged) and a 0.5 m one (a riser the walk check walks by its wall), with the
    /// walk check on every link.
    #[test]
    fn a_walk_memo_build_is_the_full_build() {
        use crate::grid::memo::{Memo, Mode, SiteStats};
        let body = Body { client_controller: true, walk_start_clear: true, walk_every_link: true, ..body() };
        for top in [0.3f32, 0.5] {
            let tris = step(top);
            let start = [Vec3::new(1.0, 0.0, 3.0)];
            let name = format!("walk-memo-{top}");
            let dir = std::env::temp_dir().join(format!("srp-memo-{name}-{}", std::process::id()));
            let build = |memo: &mut Memo| WalkGrid::build_kept_memo(&tris, None, [-2.0, 0.0, 12.0, 6.0], 0.25, body, &start, &start, OffMeshLinks::Gaps, memo).0;
            let full_grid = build(&mut Memo::off());
            let (full, full_off) = (full_grid.digest_full(), full_grid.off_links().to_vec());
            let check = |memo: &Memo, grid: WalkGrid, what: &str, ok: &dyn Fn(&SiteStats) -> bool| {
                assert_eq!(grid.digest_full(), full, "{name} {what}");
                assert_eq!(grid.off_links(), &full_off[..], "{name} {what}: the off links in memory, every field");
                for site in ["walk.link", "walk.gait"] {
                    assert!(memo.stats.iter().any(|s| s.site == site && s.items > 0), "{name} {what}: site {site} ran with items: {:?}", memo.stats);
                }
                for s in &memo.stats {
                    assert!(ok(s), "{name} {what}: {s:?}");
                }
            };
            let mut written = Memo::new(Mode::WriteOnly, dir.clone(), 13);
            let g = build(&mut written);
            check(&written, g, "write-only", &|s| s.written == s.items && s.hits == 0);
            let mut used = Memo::new(Mode::Use, dir.clone(), 13);
            let g = build(&mut used);
            check(&used, g, "use", &|s| s.hits == s.items && s.misses == 0 && s.errors == 0);
            let mut verified = Memo::new(Mode::Verify { per_million: 1_000_000 }, dir.clone(), 13);
            let g = build(&mut verified);
            check(&verified, g, "verify=1", &|s| s.verified == s.items && s.errors == 0);
            let _ = std::fs::remove_dir_all(dir);
        }
    }

    /// [body physics S6c] The walk gait's whole job through the memo, its press path too: on a 1.2 m step the walk check's test
    /// and the exe's walk do not get up, so the risers' flight is flown (exe_walk, walk_gait_press, best_press, riser_lands_on);
    /// written, used and verified, the memo gives the job's own verdict.
    #[test]
    fn the_walk_gait_press_path_through_the_memo_is_the_job() {
        use crate::grid::memo::{Memo, Mode};
        let tris = step(1.2);
        let start = [Vec3::new(1.0, 0.0, 3.0)];
        let (grid, _) = WalkGrid::build_kept_with_links(&tris, [-2.0, 0.0, 12.0, 6.0], 0.25, body(), &start, &start, OffMeshLinks::Gaps);
        let (a, b) = (grid.locate(Vec3::new(4.4, 0.0, 3.1), 0.6).expect("a floor before the step"), grid.locate(Vec3::new(5.6, 1.2, 3.1), 0.6).expect("a floor on its top"));
        let d = DIRS.iter().position(|&dir| dir == (1, 0)).expect("the +x way");
        let (caster, s) = grid.body.controller();
        let world = Collision::build(&tris).with_caster(caster);
        let start = grid.body.walk_start();
        let todo = [(a, b, d)];
        let full = grid.walk_gait_memo(&world, &s, start, &todo, &mut Memo::off());
        assert!(matches!(full[0], GaitVerdict::Pressed(_) | GaitVerdict::Unwalked), "the press path ran: {full:?}");
        let dir = std::env::temp_dir().join(format!("srp-memo-walk-gait-press-{}", std::process::id()));
        for (mode, what) in [(Mode::WriteOnly, "write-only"), (Mode::Use, "use"), (Mode::Verify { per_million: 1_000_000 }, "verify=1")] {
            let mut memo = Memo::new(mode, dir.clone(), 17);
            assert_eq!(grid.walk_gait_memo(&world, &s, start, &todo, &mut memo), full, "{what}");
            let site = memo.stats.iter().find(|st| st.site == "walk.gait").expect("the site ran");
            let ok = match mode {
                Mode::WriteOnly => site.written == 1,
                Mode::Use => site.hits == 1 && site.errors == 0,
                _ => site.verified == 1 && site.errors == 0,
            };
            assert!(ok, "{what}: {site:?}");
        }
        let _ = std::fs::remove_dir_all(dir);
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
