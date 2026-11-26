//! [A389] The gap links flown by the client's own jump code ([`super::Body::gap_flights`]).
//!
//! The clip pass ([`WalkGrid::gap_links`]) judges a gap's presses by the jump clip's root carried over the grid's cells
//! (`SHORT_JUMP_AHEAD`, `fly`): an approximation of the client's flight, which misses ways the client makes. Mamayev Kurgan's
//! gully (A369, `nav-builder-20261112/A369-root-cause.md`): from the ridge (52.8, 40.55, 135.0) to the plateau (56.2, 37.6,
//! 133.0) the clip pass made no link (the tip's route went round, 97.4 m for 6.4 m); on the game's shapes world the client's
//! walking jump lands on the plateau at all 11 leads, the sprint jump in 44 of 44 flights, and 56 players crossed there.
//!
//! With the switch every gap is flown as the rim jumps are (A385-FORMAT, `controller::jump`): each press a client makes (the
//! short jump at the body's walk cap, the slow presses, the sprint jump with every variant) at the gap leads, on the shapes
//! world, the keys let go at the landing; the link goes to the floor the most presses land and stay on, and carries the press
//! whose neighbourhood lands most and its share ([`WalkGrid::best_press`], the rim jumps' own scoring). No clip approximation
//! decides a link. The walk-offs (no press) are made as before ([`WalkGrid::walk_off_link`]); the thinning, the saving and the
//! way back are the clip pass's ([`WalkGrid::thin_gap_links`]).
//!
//! A gap: from a floor along a way with no Walk or Jump link (none, or a Drop), across cells with no floor within a step of
//! the take-off, to the first cell with a floor two or more cells on that is higher, by more than a step, than every floor
//! crossed (walking would go down and up again), no higher than a gap link reaches (above is the rim pass's) and no lower than
//! the harmless drop; within the clip pass's own reach ([`SPRINT_REACH_M`] past the take-off's cell).

use super::gap::{GapStats, GridWay, OffLink, SPRINT_REACH_M};
use super::memo::{Codec, Memo, MemoCodec, MemoInput, Mode, Site, StableIds, Windows};
use super::rim::{gap_leads, Made, AIMED_LEADS, AIMS};
use super::gap::Aim;
use crate::digest::Digest128;
use super::*;
use crate::controller::jump::{self, FlightStep, Kits};
use crate::controller::tick::Settings;
use crate::controller::Collision;

/// A gap to fly: the take-off, the floor across it, the way, and the height a landing is over (a step over every floor
/// crossed: a press that comes down into the gap is no way across it, the Drop links' and walk-offs' way).
#[derive(Clone, Copy, Debug)]
struct Gap {
    a: u32,
    b: u32,
    d: usize,
    over: f32,
    /// The first cell on from the take-off's a landing may be in: 2 for a gap, past the thin top for [S4] a gap over one.
    past: i32,
    /// [S4] Over a thin top ([`WalkGrid::over_thin_top`]).
    thin: bool,
}

/// [S4] The highest thin top a gap over one is searched over, m over the take-off: a sprint jump clears a rail to 1.2 m
/// (`railings-design.md` §4, 725-805 of 805 presses, every lead 0-1.9 m). Only bounds the search; each link is flown.
pub const OVER_TOP_M: f32 = 1.2;
/// [S4] The longest a thin top runs along the way, m (the thin tops of the railings census, `railings-design.md` §3): in cells
/// along the way's stride, rounded (2 straight, 1 on a diagonal); a longer one is a ledge's top, the rim jumps' and the gap
/// links' onto it.
pub const THIN_TOP_M: f32 = 0.5;

impl WalkGrid {
    /// [row 31] Whether the floor `f` of `cell` lies overhead a jump from the height `h` that rises at most `reach`: more than
    /// `reach` over `h`, and nothing solid in the cell's column ([`super::Solids`]) from [`super::gap::AIR_FEET_M`] over `h` to
    /// `reach` and a standing body's height ([`super::Body::stand_height`]) over it: a deck the jump passes beneath, not a wall.
    /// The Vector Laboratory battery spawner's ledge (-2.24, 16.88, -11.04) lies four cells across a gap from the east ledge's
    /// 16.40, every cell between holding the walkway 20.6 m up (its underside 20.41); the gap search ended at the walkway as at a
    /// wall, so no jump onto the ledge was flown (the exe lands that jump, 8 of 31 presses, part 3 §18b). Only bounds the
    /// searches; each link is flown on the game's shapes.
    pub(super) fn overhead(&self, solids: &Solids, cell: usize, f: NodeId, h: f32, reach: f32) -> bool {
        if self.height[f as usize] - h <= reach {
            return false;
        }
        let (i, k) = self.cell_xz(cell);
        let (x, z) = (self.x0 + (i as f32 + 0.5) * self.cell, self.z0 + (k as f32 + 0.5) * self.cell);
        !self.solid_within(solids, x, z, 0.0, h + super::gap::AIR_FEET_M, h + reach + self.body.stand_height)
    }

    /// The floor across the gap from `a` along `d` (module documentation) and the gap's bottom: the highest floor crossed, and at
    /// least the floor a body walking off the edge comes down on (the way's Drop link, the walk-off at the walk cap,
    /// [`WalkGrid::walk_off`]: the cells by the edge may have no floor, the erosion's under a face, and the gap's floor beyond
    /// them is no landing across it); -inf when there is neither. None when there is no gap.
    fn gap_across(&self, wide: &Wide, solids: &Solids, a: usize, d: usize) -> Option<(u32, f32)> {
        let h = self.height[a];
        let body = &self.body;
        let reach = body.max_jump.min(MARGINAL_JUMP_M);
        let (i, k) = self.cell_xz(self.cell_of[a] as usize);
        let (di, dk) = DIRS[d];
        let stride = self.cell * ((di * di + dk * dk) as f32).sqrt();
        let linked = |f: u32| wide.links[a].contains(&f);
        let drop = (wide.links[a][d] != NONE && wide.kinds[a][d] == LinkKind::Drop).then(|| self.height[wide.links[a][d] as usize]);
        let walked = self.walk_off(a, d, jump::WALK_CAP_MS, solids).map(|f| self.height[f as usize]);
        let mut crossed = [drop, walked].into_iter().flatten().filter(|&y| y < h - body.step).fold(f32::NEG_INFINITY, f32::max);
        let mut g = 1;
        while (g as f32 - 0.5) * stride <= SPRINT_REACH_M {
            let cell = self.cell_index(i + g * di, k + dk * g)?;
            let floors = self.start[cell]..self.start[cell + 1];
            if g == 1 {
                // A floor within a step in the next cell: no gap; one up within a gap link's reach is the Jump links' (a ledge).
                // [row 31] Not a floor overhead with room under it ([`Self::overhead`]).
                if floors.clone().any(|f| self.height[f as usize] - h > -body.step && !self.overhead(solids, cell, f, h, reach)) {
                    return None;
                }
            } else {
                let target = floors.clone().filter(|&f| {
                    let y = self.height[f as usize];
                    y - h <= reach
                        && h - y <= body.max_drop
                        && y > crossed + body.step
                        && self.ceiling[f as usize] - y >= body.stand_height
                        && !linked(f)
                });
                if let Some(f) = target.min_by(|&p, &q| (self.height[p as usize] - h).abs().total_cmp(&(self.height[q as usize] - h).abs())) {
                    return Some((f, crossed));
                }
                // A floor within a step goes on (no gap), a higher one than the reach is a wall: either ends the search. [row 31]
                // Not a floor overhead with room under it ([`Self::overhead`]): a deck the jump passes beneath.
                if floors.clone().any(|f| {
                    let rise = self.height[f as usize] - h;
                    rise.abs() <= body.step || (rise > reach && !self.overhead(solids, cell, f, h, reach))
                }) {
                    return None;
                }
            }
            crossed = floors.map(|f| self.height[f as usize]).filter(|&y| y < h + body.step).fold(crossed, f32::max);
            g += 1;
        }
        None
    }

    /// [S4] The floor across a thin top from `a` along `d` (a rail, a railing, a sill at the floor's edge, `railings-design.md`
    /// §4, "over a rail" and "jump off a rail's side": the Chemical Plant train's roof past the railing): across cells with no
    /// floor within a step of the take-off, a top of at most [`THIN_TOP_M`] along the way, more than a step and at most
    /// [`OVER_TOP_M`] over the take-off; then the first cell holding a floor at least a step under the top's, in a gap link's
    /// reach (no higher than the reach, no lower than the harmless drop), with standing room, that `a` has no link to. With the
    /// gap's bottom (the highest floor crossed under the take-off, the walk-off's floor at least, as [`Self::gap_across`]) and the
    /// first cell a landing may be in (past the top), and [S3] the top's cells with the height of the floor each holds on it
    /// ([`super::line_tops::ThinTopCell`]). None when the way meets no such top.
    fn over_thin_top(&self, wide: &Wide, solids: &Solids, a: usize, d: usize) -> Option<(u32, f32, i32, Vec<super::line_tops::ThinTopCell>)> {
        let h = self.height[a];
        let body = &self.body;
        let reach = body.max_jump.min(MARGINAL_JUMP_M);
        let (i, k) = self.cell_xz(self.cell_of[a] as usize);
        let (di, dk) = DIRS[d];
        let stride = self.cell * ((di * di + dk * dk) as f32).sqrt();
        let linked = |f: u32| wide.links[a].contains(&f);
        let walked = self.walk_off(a, d, jump::WALK_CAP_MS, solids).map(|f| self.height[f as usize]);
        let mut crossed = walked.filter(|&y| y < h - body.step).unwrap_or(f32::NEG_INFINITY);
        // Before the top: cells with no floor within a step of the take-off (a floor there is walked to: no edge here).
        let mut g = 1;
        let top = loop {
            if (g as f32 - 0.5) * stride > SPRINT_REACH_M {
                return None;
            }
            let cell = self.cell_index(i + g * di, k + dk * g)?;
            // [row 31] A floor overhead with room under it ([`Self::overhead`]) is no top: the body passes beneath it.
            let up: Vec<f32> = (self.start[cell]..self.start[cell + 1])
                .filter(|&f| !self.overhead(solids, cell, f, h, OVER_TOP_M))
                .map(|f| self.height[f as usize])
                .filter(|&y| y > h - body.step)
                .collect();
            if !up.is_empty() {
                if up.iter().any(|&y| y - h <= body.step || y - h > OVER_TOP_M) {
                    return None;
                }
                break up.iter().copied().fold(f32::NEG_INFINITY, f32::max);
            }
            crossed = (self.start[cell]..self.start[cell + 1]).map(|f| self.height[f as usize]).fold(crossed, f32::max);
            g += 1;
        };
        // The top: at most THIN_TOP_M along the way's stride.
        let thin = ((THIN_TOP_M / stride).round() as i32).max(1);
        let first = g;
        // [S3] Its cells, each with the floor on the top there: the first cell's highest (`top`), then each further cell's within
        // a step of it.
        let mut on_top = vec![super::line_tops::ThinTopCell { cell: self.cell_index(i + g * di, k + dk * g)?, height: top, way: d }];
        while g - first < thin {
            let cell = self.cell_index(i + (g + 1) * di, k + dk * (g + 1))?;
            let Some(f) = (self.start[cell]..self.start[cell + 1]).find(|&f| (self.height[f as usize] - top).abs() <= body.step) else {
                break;
            };
            on_top.push(super::line_tops::ThinTopCell { cell, height: self.height[f as usize], way: d });
            g += 1;
        }
        if g - first >= thin {
            return None;
        }
        // Past it: the first cell with a landing.
        let past = g + 1;
        let mut g = past;
        while (g as f32 - 0.5) * stride <= SPRINT_REACH_M {
            let cell = self.cell_index(i + g * di, k + dk * g)?;
            let floors = self.start[cell]..self.start[cell + 1];
            let target = floors.clone().filter(|&f| {
                let y = self.height[f as usize];
                y - h <= reach
                    && h - y <= body.max_drop
                    && y <= top - body.step
                    && y > crossed
                    && self.ceiling[f as usize] - y >= body.stand_height
                    && !linked(f)
            });
            if let Some(f) = target.max_by(|&p, &q| self.height[p as usize].total_cmp(&self.height[q as usize])) {
                return Some((f, crossed, past, on_top));
            }
            // A floor higher than a step under the top past it: a wider top or a wall, the rim jumps' and the gap links'. [row 31]
            // Not a floor overhead with room under it ([`Self::overhead`]).
            if floors.clone().any(|f| self.height[f as usize] > top - body.step && !self.overhead(solids, cell, f, h, reach)) {
                return None;
            }
            g += 1;
        }
        None
    }

    /// The floor a flight's `steps` from the take-off `a` lands and stays on (A385-FORMAT §1): its landing
    /// ([`jump::landing_and_rest`], the first step out of the jump on the ground, `on_ground` 0x7EF90) over a floor
    /// ([`Self::floor_under`]) two or more cells on from `a`'s, in a gap link's reach of it (no higher than the reach, no lower
    /// than the harmless drop) and over `over` (out of the gap, [`Gap::over`]), with standing room, not one `a` has a link to
    /// (`linked`), and at rest over such a floor: the floor it rests on. [backward landing] The floor must lie ahead of `a`'s
    /// cell along the way the press is flown (`dir`, a [`DIRS`] step: its cell's offset from `a`'s has a positive dot with it):
    /// the link is stored with that way ([`GridWay::of_dir`]) and a bot presses along it, so a flight that comes down behind the
    /// take-off (the press point on the landing's own floor, the body thrown back onto it) proves no crossing along the way.
    /// Before, such a flight made a link: Mamayev Kurgan TDM's GapJump (8.60, 25.20, 48.12) -> (10.10, 25.08, 48.12) stored the
    /// way (-1, 0) with its landing 1.5 m behind the take-off along it, its press point at x 10.2 on the landing floor.
    fn gap_lands_on(&self, steps: &[FlightStep], a: u32, dir: (i32, i32), linked: &[NodeId], over: f32, past: i32) -> Option<NodeId> {
        let h = self.height[a as usize];
        let reach = self.body.max_jump.min(MARGINAL_JUMP_M);
        let (i, k) = self.cell_xz(self.cell_of[a as usize] as usize);
        let onto = |feet: Vec3| {
            self.floor_under(feet).filter(|&g| {
                let y = self.height[g as usize];
                let (gi, gk) = self.cell_xz(self.cell_of[g as usize] as usize);
                (gi - i) * dir.0 + (gk - k) * dir.1 > 0
                    && (gi - i).abs().max((gk - k).abs()) >= past
                    && y - h <= reach
                    && h - y <= self.body.max_drop
                    && y > over
                    && self.ceiling[g as usize] - y >= self.body.stand_height
                    && !linked.contains(&g)
                    && !self.walk_reaches(a, g)
            })
        };
        let (_, rest) = jump::landing_and_rest(steps, |f| onto(f.feet).is_some())?;
        onto(rest.feet)
    }

    /// [body physics S5a fix] Whether a walk reaches the floor `g` from the take-off `a` at its own level: `g` within a step
    /// ([`super::Body::step`]) of `a`'s height, and every cell on the straight line between their cells holding a floor within a
    /// step of it with standing room. Such a landing crosses no gap: a walk reaches it (`clip_gap_bottom`'s rule for the clip
    /// presses, review of #2175). A floor at that height behind a wall or a rail is not reached so: the wall's cells hold no
    /// standing floor at the take-off's level.
    fn walk_reaches(&self, a: u32, g: NodeId) -> bool {
        let (h, step) = (self.height[a as usize], self.body.step);
        if (self.height[g as usize] - h).abs() > step {
            return false;
        }
        let (i, k) = self.cell_xz(self.cell_of[a as usize] as usize);
        let (gi, gk) = self.cell_xz(self.cell_of[g as usize] as usize);
        let n = (gi - i).abs().max((gk - k).abs());
        (1..n).all(|t| {
            let at = |o: i32, d: i32| o + ((d * t) as f32 / n as f32).round() as i32;
            self.cell_index(at(i, gi - i), at(k, gk - k)).is_some_and(|cell| {
                (self.start[cell]..self.start[cell + 1]).any(|f| {
                    let y = self.height[f as usize];
                    (y - h).abs() <= step && self.ceiling[f as usize] - y >= self.body.stand_height
                })
            })
        })
    }

    /// [F1] The bottom of the gap a clip link from `a` along `d` to `to` crosses ([`Self::gap_across`]'s): the highest of the
    /// way's Drop link's floor and the walk-off's floor under the take-off by more than a step, and of the floors crossed in
    /// the cells from `a`'s on to `to`'s (not counting it) that lie under the take-off's height plus a step; -inf when none.
    /// A landing over it by more than a step is out of the gap ([`Gap::over`]). A floor at the take-off's level in a crossed
    /// cell makes a level landing beyond it no crossing: harmless, a walk reaches that landing (review of #2175).
    fn clip_gap_bottom(&self, wide: &Wide, solids: &Solids, a: u32, d: usize, to: u32) -> f32 {
        let a = a as usize;
        let h = self.height[a];
        let step = self.body.step;
        let drop = (wide.links[a][d] != NONE && wide.kinds[a][d] == LinkKind::Drop).then(|| self.height[wide.links[a][d] as usize]);
        let walked = self.walk_off(a, d, jump::WALK_CAP_MS, solids).map(|f| self.height[f as usize]);
        let mut crossed = [drop, walked].into_iter().flatten().filter(|&y| y < h - step).fold(f32::NEG_INFINITY, f32::max);
        let (i, k) = self.cell_xz(self.cell_of[a] as usize);
        let (ti, tk) = self.cell_xz(self.cell_of[to as usize] as usize);
        let (di, dk) = DIRS[d];
        let cells = (ti - i).abs().max((tk - k).abs());
        for g in 1..cells {
            let Some(cell) = self.cell_index(i + g * di, k + g * dk) else { break };
            crossed = (self.start[cell]..self.start[cell + 1]).map(|f| self.height[f as usize]).filter(|&y| y < h + step).fold(crossed, f32::max);
        }
        crossed
    }

    /// [F1] The clip pass's pressed links (`clip`: a press from the jump clip's approximation over the cells, never flown) where
    /// no flown link of `found` covers their group (the thinning's key: a metre of edge, the way, the levels), proven by the
    /// client's own jump code before they may be kept (part 3 F1: a link is a proven press). Each group's first and last such
    /// link (the thinning keeps one a group; the flown groups' own bound) is flown as a flown group is: over every press a
    /// client makes ([`Self::best_press`]: the walk, the slow presses, the sprint with every variant, the sprint climb, at the
    /// gap leads; then the aimed presses where a flight came near), a flight landing by the gap pass's own rule
    /// ([`Self::gap_lands_on`]: two or more cells on, over the gap's bottom by more than a step, [`Self::clip_gap_bottom`], in
    /// a gap link's reach, with standing room, on no floor `a` links to already), and each link made goes to the floor its
    /// flights rest on, with its press, share, walking press and aim. A group none lands is left with no pressed link: no
    /// client press makes it (every client press was tried: teach, do not cut). Before, such a link stayed with the clip's
    /// press and no share, so the route priced it as a sure link (part 3 table 18c: 6 spots where no fed press lands and the
    /// bots failed 9 tries, the stored press missing in the exe and in the port). The walk-offs pass as they are (the Drop
    /// links' walk proves them, `drop_check`).
    #[allow(clippy::too_many_arguments)]
    fn prove_clip_presses(
        &self,
        wide: &Wide,
        solids: &Solids,
        found: &[(OffLink, usize)],
        clip: Vec<(OffLink, usize)>,
        world: &Collision,
        s: &Settings,
        kits: Kits,
        stats: &mut GapStats,
        memo: &mut Memo,
    ) -> Vec<(OffLink, usize)> {
        use super::gap::Takeoff;
        let key = |l: &OffLink, d: usize| {
            let (p, q) = (self.position(l.from), self.position(l.to));
            (
                p.x.floor() as i32,
                p.z.floor() as i32,
                (2.0 * p.y).round() as i32,
                if l.kind == LinkKind::GapDrop { usize::MAX } else { d },
                (2.0 * q.y).round() as i32,
            )
        };
        let covered: std::collections::HashSet<_> = found.iter().map(|(l, d)| key(l, *d)).collect();
        let mut out: Vec<(OffLink, usize)> = Vec::new();
        let mut presses: Vec<(OffLink, usize)> = Vec::new();
        for (l, d) in clip {
            if matches!(l.press, Takeoff::WalkOff | Takeoff::Release { .. }) || l.share.flown().is_some() {
                out.push((l, d));
            } else {
                presses.push((l, d));
            }
        }
        // The groups no flown link covers, each in the thinning's order (a walking press before a sprint, then floor order).
        presses.retain(|(l, d)| !covered.contains(&key(l, *d)));
        presses.sort_by_key(|&(l, d)| (key(&l, d), u8::from(matches!(l.press, Takeoff::Sprint(_) | Takeoff::SprintHeld(_) | Takeoff::Climb)), l.from, d, l.to));
        let mut groups: Vec<std::ops::Range<usize>> = Vec::new();
        for (i, (l, d)) in presses.iter().enumerate() {
            match groups.last_mut() {
                Some(r) if key(&presses[r.start].0, presses[r.start].1) == key(l, *d) => r.end = i + 1,
                _ => groups.push(i..i + 1),
            }
        }
        let leads = gap_leads();
        let ends_of = |r: &std::ops::Range<usize>| -> Vec<(OffLink, usize)> {
            let group = &presses[r.clone()];
            if group.len() <= 2 { group.to_vec() } else { vec![group[0], group[group.len() - 1]] }
        };
        // [incremental grids PR 8] The groups' flights through the job memo (site `gap.clip`, [`gap_clip_job`]); with the memo
        // off (the default), `parallel` over the same job. Each end's gap bottom ([`Self::clip_gap_bottom`]) and links are
        // read here, into the job's item.
        let on = memo.mode() != Mode::Off;
        let reach_m = self.flight_reach_m();
        let window_cells = ((reach_m + self.body.radius + self.cell) / self.cell).ceil() as i64;
        let windows = on.then(|| Windows::of_floors(self));
        let items: Vec<GapFlownItem> = groups
            .iter()
            .map(|r| GapFlownItem {
                ends: ends_of(r)
                    .into_iter()
                    .map(|(l, d)| GapEnd {
                        a: l.from,
                        b: l.to,
                        d,
                        over: self.clip_gap_bottom(wide, solids, l.from, d, l.to) + self.body.step,
                        past: 2,
                        // As the flown groups: the floors `a` has a link to already make no landing.
                        linked: wide.links[l.from as usize].iter().copied().filter(|&f| f != NONE).collect(),
                        window: windows.as_ref().map_or(0, |w| w.around(self.cell_of[l.from as usize] as usize, window_cells)),
                    })
                    .collect(),
            })
            .collect();
        if on {
            assert_presses_within(&leads, reach_m, "gap.clip");
        }
        let ctx = GapFlownCtx { leads: leads.clone(), kits, reach_m, window_cells, grid: self, world, s };
        let site = Site { id: "gap.clip", source: crate::source::site("gap.clip") };
        let ran = memo.run(site, self, &ctx, &items, gap_clip_key, gap_clip_job);
        if on {
            report_reach("gap.clip", ran.iter().map(|o| o.reach_m), ran.len(), reach_m, window_cells);
        }
        let proven: Vec<(Vec<(OffLink, usize)>, usize)> = groups
            .iter()
            .zip(ran)
            .map(|(r, o)| {
                let links = o.made.map_or_else(Vec::new, |(end, made)| {
                    let (l, d) = ends_of(r)[usize::from(end)];
                    made.into_iter()
                        .map(|m| {
                            let rise = self.height[m.to as usize] - self.height[l.from as usize];
                            let kind = if rise > -self.body.step { LinkKind::GapJump } else { LinkKind::GapDrop };
                            (OffLink { from: l.from, to: m.to, kind, press: m.press, share: m.share, walk: m.walk, aim: m.aim, way: Some(GridWay::of_dir(d)) }, d)
                        })
                        .collect()
                });
                (links, o.flights)
            })
            .collect();
        stats.clip_tried += groups.len();
        let mut pairs: std::collections::HashSet<(NodeId, NodeId)> = found.iter().map(|(l, _)| (l.from, l.to)).collect();
        for (made, flights) in proven {
            stats.clip_flights += flights;
            if made.is_empty() {
                stats.clip_unlanded += 1;
                continue;
            }
            stats.clip_proven += 1;
            stats.clip_retargeted += made.iter().filter(|(m, _)| !presses.iter().any(|(l, _)| (l.from, l.to) == (m.from, m.to))).count();
            // One link a (take-off, landing) pair: a flown link of the pair is there already, or an earlier group made it.
            for link in made {
                if pairs.insert((link.0.from, link.0.to)) {
                    stats.by_share[link.0.share.class()] += 1;
                    out.push(link);
                }
            }
        }
        out
    }

    /// [`super::Body::gap_flights`]: the gap links of the eroded grid flown by the client's jump on `world` (module
    /// documentation), with the walk-offs, kept and thinned as [`Self::gap_links`]'s, sorted by take-off then landing; and [S3]
    /// the cells of the thin tops the gaps over one cross ([`Self::over_thin_top`]), for [`Self::line_tops_of`].
    pub(super) fn gap_links_flown(
        &self,
        wide: &Wide,
        solids: &Solids,
        world: &Collision,
        s: &Settings,
        stats: &mut GapStats,
        memo: &mut Memo,
    ) -> (Vec<OffLink>, Vec<super::line_tops::ThinTopCell>) {
        if self.body.max_jump <= 0.0 {
            return (Vec::new(), Vec::new());
        }
        let n = self.len();
        // The clip pass's links (its walk-offs and presses) as well: a link goes only where no client action makes it
        // and no player does. A flown press wins its group in the thinning; a clip press is kept, unflown,
        // where none is flown (a gap the candidates below miss, or no flown press lands).
        let clip = self.gap_found(wide, solids, stats);
        let mut found: Vec<(OffLink, usize)> = Vec::new();
        let mut gaps: Vec<Gap> = Vec::new();
        let mut thin_tops: Vec<super::line_tops::ThinTopCell> = Vec::new();
        for a in 0..n {
            if self.ceiling[a] - self.height[a] < self.body.stand_height {
                continue;
            }
            for d in 0..DIRS.len() {
                let b = wide.links[a][d];
                // [S4] Over a thin top: along a way no Walk link takes (a Jump link onto the top, a Drop, or none).
                if (b == NONE || wide.kinds[a][d] != LinkKind::Walk)
                    && let Some((b, crossed, past, on_top)) = self.over_thin_top(wide, solids, a, d)
                {
                    gaps.push(Gap { a: a as u32, b, d, over: crossed, past, thin: true });
                    thin_tops.extend(on_top);
                }
                if b != NONE && wide.kinds[a][d] != LinkKind::Drop {
                    continue;
                }
                match self.gap_across(wide, solids, a, d) {
                    Some((b, crossed)) => gaps.push(Gap { a: a as u32, b, d, over: crossed + self.body.step, past: 2, thin: false }),
                    None => stats.no_gap += 1,
                }
            }
        }
        // The gaps in groups by the thinning's key (a metre of edge, the way, the level, the far floor's level): flown in floor
        // order until one is made (the one the thinning keeps of a group's made links anyway), at most the group's two ends.
        let key = |g: &Gap| {
            let (p, q) = (self.position(g.a), self.position(g.b));
            (p.x.floor() as i32, p.z.floor() as i32, (2.0 * p.y).round() as i32, g.d, (2.0 * q.y).round() as i32, g.thin)
        };
        gaps.sort_by_key(|g| (key(g), g.a));
        let mut groups: Vec<std::ops::Range<usize>> = Vec::new();
        for (i, g) in gaps.iter().enumerate() {
            match groups.last_mut() {
                Some(r) if key(&gaps[r.start]) == key(g) => r.end = i + 1,
                _ => groups.push(i..i + 1),
            }
        }
        stats.gaps = gaps.len();
        stats.groups = groups.len();
        let leads = gap_leads();
        let kits = Kits { body: jump::WALK_CAP_MS, fastest: self.body.fastest_walk_cap };
        // At most a group's first and its last take-off (in floor order): the thinning keeps one link a group, and a group's ends
        // span its metre of edge. A bound of the pass's cost (the flights are the build's dearest step), our choice.
        let ends_of = |r: &std::ops::Range<usize>| -> Vec<Gap> {
            let group = &gaps[r.clone()];
            if group.len() <= 2 { group.to_vec() } else { vec![group[0], group[group.len() - 1]] }
        };
        // [incremental grids PR 7] The groups' flights through the job memo (site `gap.flown`, [`gap_flown_job`]); with the
        // memo off (the default), `parallel` over the same job.
        let flown = self.fly_gap_groups(world, s, wide, &groups, &ends_of, &leads, kits, memo);
        stats.over_thin = gaps.iter().filter(|g| g.thin).count();
        let flown = groups.iter().zip(flown).map(|(r, o)| {
            let ends = ends_of(r);
            (o.made.map(|(end, made)| (ends[usize::from(end)], made)), o.tried, o.flights, o.aimed)
        });
        for (made, tried, flights, aimed) in flown {
            stats.flown += tried;
            stats.flights += flights;
            stats.aimed_tried += usize::from(aimed.is_some());
            stats.aimed_flights += aimed.unwrap_or(0);
            let Some((g, made)) = made else {
                stats.no_landing += 1;
                continue;
            };
            stats.aimed_made += usize::from(aimed.is_some());
            stats.over_thin_made += usize::from(g.thin) * made.len();
            for Made { to, press, share, walk, aim, .. } in made {
                let rise = self.height[to as usize] - self.height[g.a as usize];
                let kind = if rise > -self.body.step { LinkKind::GapJump } else { LinkKind::GapDrop };
                stats.made += 1;
                stats.sprint += usize::from(matches!(press, super::gap::Takeoff::Sprint(_) | super::gap::Takeoff::SprintHeld(_)));
                stats.by_share[share.class()] += 1;
                found.push((OffLink { from: g.a, to, kind, press, share, walk, aim, way: Some(GridWay::of_dir(g.d)) }, g.d));
            }
        }
        let proven = self.prove_clip_presses(wide, solids, &found, clip, world, s, kits, stats, memo);
        found.extend(proven);
        found.sort_by_key(|&(l, d)| (l.from, d, l.to));
        let out = self.thin_gap_links(wide, found, stats);
        stats.clip_kept = out.iter().filter(|l| l.press != super::gap::Takeoff::WalkOff && l.share.flown().is_none()).count();
        (out, thin_tops)
    }
}


/// [incremental grids PR 7] One take-off of a flown gap group's job: the gap (take-off `a`, the far floor `b`, way `d`, the
/// height a landing must be over, the first cell on a landing may be in), the floors `a` has a link to already (its sides: a
/// landing there makes no link) and the digest of the floors in its window ([`Windows::of_floors`]).
#[derive(Clone, Debug)]
pub(super) struct GapEnd {
    a: NodeId,
    b: NodeId,
    d: usize,
    over: f32,
    past: i32,
    linked: Vec<NodeId>,
    window: u128,
}

/// [incremental grids PR 7] One job of the flown gaps' site: a group's ends, flown in order.
#[derive(Clone, Debug)]
pub(super) struct GapFlownItem {
    ends: Vec<GapEnd>,
}

crate::memo_ctx! {
    /// [incremental grids PR 7] The flown gaps' site's context: the leads, the walk caps, the reach and the window (hashed),
    /// and the grid, the world and the tick's settings, read through the items' windows and the memo's global inputs.
    pub(super) struct GapFlownCtx<'a> {
        leads: Vec<f32>,
        kits: Kits,
        reach_m: f32,
        window_cells: i64,
    } through {
        grid: &'a WalkGrid,
        world: &'a Collision,
        s: &'a Settings,
    }
}

/// [incremental grids PR 7] A flown gap group's result: the end that made it (its index in the item) and its presses, the
/// ends tried, the flights, the aimed sweep's flights where it was flown, and how far from the take-offs the flights went.
#[derive(Clone, Debug, Default)]
pub(super) struct GapFlownOut {
    made: Option<(u8, Vec<Made>)>,
    tried: usize,
    flights: usize,
    aimed: Option<usize>,
    reach_m: f32,
}

/// [incremental grids PR 7] The flown gaps' site's key of a job: each end's floors by their stable ids, its way, the height
/// over and the first cell, the floors it has links to, and its window.
fn gap_flown_key(_: &GapFlownCtx<'_>, ids: &StableIds, item: &GapFlownItem) -> u128 {
    let mut d = Digest128::default();
    d.u64(item.ends.len() as u64);
    for end in &item.ends {
        ids.id(end.a).feed(&mut d);
        ids.id(end.b).feed(&mut d);
        d.u64(end.d as u64);
        end.over.feed(&mut d);
        end.past.feed(&mut d);
        d.u64(end.linked.len() as u64);
        for &n in &end.linked {
            ids.id(n).feed(&mut d);
        }
        d.u128(end.window);
    }
    d.finish()
}

/// [incremental grids PR 7] The flown gaps' site's job, as `gap_links_flown` flew a group: its ends in order over the line's
/// presses until one makes it (every press it makes: a jump's, and [S1] a sprint climb's onto its own floor, as the rim pass
/// keeps them); else [R83], where a flight came near the landing, the aimed presses from the first such end
/// (`R83-FORMAT.md`; Mamayev Kurgan day_2's box from the rock pile is a gap made only aimed).
fn gap_flown_job(ctx: &GapFlownCtx<'_>, item: &GapFlownItem) -> GapFlownOut {
    let grid = ctx.grid;
    let reach = std::cell::Cell::new(0.0f32);
    let way = |e: &GapEnd| {
        let (di, dk) = DIRS[e.d];
        let l = ((di * di + dk * dk) as f32).sqrt();
        (di as f32 / l, dk as f32 / l)
    };
    let lands = |e: &GapEnd, st: &[FlightStep]| {
        let takeoff = grid.position(e.a);
        for f in st {
            reach.set(reach.get().max(f.feet.distance_xz(takeoff)));
        }
        grid.gap_lands_on(st, e.a, DIRS[e.d], &e.linked, e.over, e.past)
    };
    let mut out = GapFlownOut::default();
    let mut near: Option<u8> = None;
    // [body physics S5a fix] The first end whose line presses landed only partly (no press in every variant), and those presses.
    let mut partial: Option<(u8, Vec<Made>)> = None;
    // [body physics S5b] L1: each end's links to its other tops ([`super::rim::RimPress::others`]), linked with the end that makes
    // the group, after the whole-press test and [`better`] judged step 2's floors.
    let mut others_of: Vec<Vec<Made>> = Vec::new();
    for (i, e) in item.ends.iter().enumerate() {
        out.tried += 1;
        let super::rim::RimPress { mut made, others, flights, near: close, .. } =
            grid.best_press(ctx.world, ctx.s, e.a, e.b, way(e), &ctx.leads, |st| lands(e, st), ctx.kits, grid.fall_ground());
        out.flights += flights;
        let i = u8::try_from(i).expect("a group's ends fit a byte");
        if made.iter().any(lands_whole) {
            made.extend(others);
            out.made = Some((i, made));
            out.reach_m = reach.get();
            return out;
        }
        others_of.push(others);
        if !made.is_empty() && partial.is_none() {
            partial = Some((i, made));
        }
        if close && near.is_none() {
            near = Some(i);
        }
    }
    // [R83] The aimed presses from the first end a flight came near; [body physics S5a fix] or, with only partial line presses,
    // from the first such end; the better share kept ([`better`]).
    if let Some(i) = near.or(partial.as_ref().map(|&(i, _)| i)) {
        let e = &item.ends[usize::from(i)];
        let sweep = grid.best_aimed(ctx.world, ctx.s, e.a, e.b, way(e), &|st| lands(e, st), ctx.kits, grid.fall_ground());
        out.flights += sweep.flights;
        out.aimed = Some(sweep.flights);
        out.made = better(partial, sweep.made.map(|m| (i, vec![m])));
    } else {
        out.made = partial;
    }
    if let Some((i, made)) = out.made.as_mut() {
        made.extend(others_of[usize::from(*i)].iter().copied());
    }
    out.reach_m = reach.get();
    out
}

/// [body physics S5a fix] Whether a made press lands in every variant the motor can play: its own flight's neighbour (the first of
/// [`jump::neighbourhood`], bit 0 of its share's mask), which lands only when every variant of it does. Only such a press makes a
/// gap group; with a partial one the aimed sweep is flown as well ([`better`]). Before, a line press one sprint variant of which
/// landed made the group and the aimed sweep was not flown (Chemical Plant batteries 27531: the straight sprint at 1.5 m, 1 of
/// 6 variants, over the aimed sprint at 1.4 m).
fn lands_whole(m: &Made) -> bool {
    m.share.mask().is_some_and(|k| k.lands(0))
}

/// [body physics S5a fix] Of a gap group's partial line presses (`line`: their end and presses) and its aimed press (`aimed`), the
/// one with the better share ([`super::gap::Share::rank`]: the priced count, then every landing neighbour; a link's cost follows from it,
/// [`super::gap::Share::failed_tries`]); the line's on a tie (the aimed sweep is the fallback, [R83]).
fn better(line: Option<(u8, Vec<Made>)>, aimed: Option<(u8, Vec<Made>)>) -> Option<(u8, Vec<Made>)> {
    let best = |m: &(u8, Vec<Made>)| m.1.iter().map(|m| m.share.rank()).max().flatten();
    match (line, aimed) {
        (Some(l), Some(a)) if best(&a) > best(&l) => Some(a),
        (Some(l), _) => Some(l),
        (None, a) => a,
    }
}

impl WalkGrid {
    /// [incremental grids PR 7] `gap_links_flown`'s groups through the memo's site `gap.flown` ([`gap_flown_job`]), each
    /// result at its group's place. With the memo on, a press point or a flight past [`WalkGrid::flight_reach_m`] stops the build:
    /// its window would not hold every floor its job read.
    #[allow(clippy::too_many_arguments)]
    fn fly_gap_groups(
        &self,
        world: &Collision,
        s: &Settings,
        wide: &Wide,
        groups: &[std::ops::Range<usize>],
        ends_of: &impl Fn(&std::ops::Range<usize>) -> Vec<Gap>,
        leads: &[f32],
        kits: Kits,
        memo: &mut Memo,
    ) -> Vec<GapFlownOut> {
        let on = memo.mode() != Mode::Off;
        let reach_m = self.flight_reach_m();
        let window_cells = ((reach_m + self.body.radius + self.cell) / self.cell).ceil() as i64;
        let windows = on.then(|| Windows::of_floors(self));
        let items: Vec<GapFlownItem> = groups
            .iter()
            .map(|r| GapFlownItem {
                ends: ends_of(r)
                    .into_iter()
                    .map(|g| GapEnd {
                        a: g.a,
                        b: g.b,
                        d: g.d,
                        over: g.over,
                        past: g.past,
                        // The floors `a` has a link to already (its sides): a landing there makes no link.
                        linked: wide.links[g.a as usize].iter().copied().filter(|&f| f != NONE).collect(),
                        window: windows.as_ref().map_or(0, |w| w.around(self.cell_of[g.a as usize] as usize, window_cells)),
                    })
                    .collect(),
            })
            .collect();
        if on {
            assert_presses_within(leads, reach_m, "gap.flown");
        }
        let ctx = GapFlownCtx { leads: leads.to_vec(), kits, reach_m, window_cells, grid: self, world, s };
        let site = Site { id: "gap.flown", source: crate::source::site("gap.flown") };
        let out = memo.run(site, self, &ctx, &items, gap_flown_key, gap_flown_job);
        if on {
            report_reach("gap.flown", out.iter().map(|o| o.reach_m), out.len(), reach_m, window_cells);
        }
        out
    }
}

/// [incremental grids PR 7, PR 8] The gap sites' press points within their window: the last lead of the line's and the aimed
/// presses, the neighbours' lead and side about it, and the widest aim's side. Past it, the build stops.
fn assert_presses_within(leads: &[f32], reach_m: f32, site: &str) {
    let n = jump::neighbourhood();
    let aim_m = AIMS.iter().map(|&(side, turn)| Aim::new(side, turn).expect("an aim in range").side_m().abs()).fold(0.0f32, f32::max);
    let press_m = leads.iter().chain(&AIMED_LEADS).copied().fold(0.0f32, f32::max)
        + n.iter().map(|n| n.lead.abs()).fold(0.0f32, f32::max)
        + n.iter().map(|n| n.side.abs()).fold(0.0f32, f32::max)
        + aim_m;
    assert!(press_m <= reach_m, "memo: ERROR site {site}: a press point {press_m} m from its take-off, past the window's {reach_m} m");
}

/// [incremental grids PR 7, PR 8] A gap site's farthest flight, logged, and the build stopped when it is past the window.
fn report_reach(site: &str, reaches: impl Iterator<Item = f32>, groups: usize, reach_m: f32, window_cells: i64) {
    let farthest = reaches.fold(0.0f32, f32::max);
    eprintln!(
        "memo: {site}: {groups} groups, the farthest flight {farthest:.2} m from its take-off (window {reach_m} m, {window_cells} cells)"
    );
    assert!(
        farthest <= reach_m,
        "memo: ERROR site {site}: a flight went {farthest} m from its take-off, past the window's {reach_m} m (WalkGrid::flight_reach_m)"
    );
}

/// [incremental grids PR 8] A clip group's result: the end that made it (its index in the item) and its presses, the flights,
/// and how far from the take-offs they went.
#[derive(Clone, Debug, Default)]
pub(super) struct GapClipOut {
    made: Option<(u8, Vec<Made>)>,
    flights: usize,
    reach_m: f32,
}

/// [incremental grids PR 8] The clip site's key of a job: as the flown gaps' ([`gap_flown_key`]): each end's floors, way,
/// gap bottom, links and window.
fn gap_clip_key(ctx: &GapFlownCtx<'_>, ids: &StableIds, item: &GapFlownItem) -> u128 {
    gap_flown_key(ctx, ids, item)
}

/// [incremental grids PR 8] The clip site's job, as `prove_clip_presses` flew a group: its ends in order, each over the line's
/// presses and, where none made it and a flight came near, the aimed presses ([R83]), until one end makes it.
fn gap_clip_job(ctx: &GapFlownCtx<'_>, item: &GapFlownItem) -> GapClipOut {
    let grid = ctx.grid;
    let reach = std::cell::Cell::new(0.0f32);
    let mut out = GapClipOut::default();
    for (i, e) in item.ends.iter().enumerate() {
        let (di, dk) = DIRS[e.d];
        let len = ((di * di + dk * dk) as f32).sqrt();
        let way = (di as f32 / len, dk as f32 / len);
        let takeoff = grid.position(e.a);
        let lands = |st: &[FlightStep]| {
            for f in st {
                reach.set(reach.get().max(f.feet.distance_xz(takeoff)));
            }
            grid.gap_lands_on(st, e.a, DIRS[e.d], &e.linked, e.over, e.past)
        };
        let p = grid.best_press(ctx.world, ctx.s, e.a, e.b, way, &ctx.leads, lands, ctx.kits, grid.fall_ground());
        out.flights += p.flights;
        let i = u8::try_from(i).expect("a group's ends fit a byte");
        // [body physics S5b] L1: this end's other tops, linked when this end makes the group ([`gap_flown_job`]).
        let others = p.others;
        // [body physics S5a fix] As the flown gaps ([`gap_flown_job`]): the line's presses make the end only when one lands in
        // every variant ([`lands_whole`]); else, where a flight came near or a line press landed partly, the aimed presses too,
        // the better share kept ([`better`]).
        let made = if p.made.iter().any(lands_whole) {
            Some((i, p.made))
        } else if p.near || !p.made.is_empty() {
            let sweep = grid.best_aimed(ctx.world, ctx.s, e.a, e.b, way, &lands, ctx.kits, grid.fall_ground());
            out.flights += sweep.flights;
            better((!p.made.is_empty()).then_some((i, p.made)), sweep.made.map(|m| (i, vec![m])))
        } else {
            None
        };
        if let Some((end, mut made)) = made {
            made.extend(others);
            out.made = Some((end, made));
            break;
        }
    }
    out.reach_m = reach.get();
    out
}

/// [incremental grids PR 8] The clip site's results: every field, the floors by their stable ids.
impl MemoCodec for GapClipOut {
    const TAG: u32 = 0x4743_4f01;
    fn encode(&self, out: &mut Codec<'_>) {
        match &self.made {
            None => out.put_u8(0),
            Some((end, made)) => {
                out.put_u8(1);
                out.put_u8(*end);
                made.encode(out);
            }
        }
        out.put_u32(u32::try_from(self.flights).expect("a group's flights fit 32 bits"));
        out.put_f32(self.reach_m);
    }
    fn decode(input: &mut Codec<'_>) -> Option<Self> {
        let made = match input.u8()? {
            0 => None,
            1 => Some((input.u8()?, Vec::<Made>::decode(input)?)),
            _ => return None,
        };
        Some(GapClipOut { made, flights: input.u32()? as usize, reach_m: input.f32()? })
    }
}

/// [incremental grids PR 7] The flown gaps' site's results: every field, the floors by their stable ids.
impl MemoCodec for GapFlownOut {
    const TAG: u32 = 0x4746_4f01;
    fn encode(&self, out: &mut Codec<'_>) {
        match &self.made {
            None => out.put_u8(0),
            Some((end, made)) => {
                out.put_u8(1);
                out.put_u8(*end);
                made.encode(out);
            }
        }
        out.put_u32(u32::try_from(self.tried).expect("a group's ends fit 32 bits"));
        out.put_u32(u32::try_from(self.flights).expect("a group's flights fit 32 bits"));
        match self.aimed {
            None => out.put_u8(0),
            Some(f) => {
                out.put_u8(1);
                out.put_u32(u32::try_from(f).expect("a sweep's flights fit 32 bits"));
            }
        }
        out.put_f32(self.reach_m);
    }
    fn decode(input: &mut Codec<'_>) -> Option<Self> {
        let made = match input.u8()? {
            0 => None,
            1 => Some((input.u8()?, Vec::<Made>::decode(input)?)),
            _ => return None,
        };
        let tried = input.u32()? as usize;
        let flights = input.u32()? as usize;
        let aimed = match input.u8()? {
            0 => None,
            1 => Some(input.u32()? as usize),
            _ => return None,
        };
        Some(GapFlownOut { made, tried, flights, aimed, reach_m: input.f32()? })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::gap::Takeoff;
    use crate::grid::{OffMeshLinks, STANDING_JUMP_M};
    use crate::WalkerBehaviour;

    const AREA: [f32; 4] = [0.0, 0.0, 14.0, 10.0];

    fn quad(x0: f32, x1: f32, y: f32) -> [Triangle; 2] {
        let (a, b, c, d) = (Vec3::new(x0, y, 0.0), Vec3::new(x1, y, 0.0), Vec3::new(x1, y, 10.0), Vec3::new(x0, y, 10.0));
        let walker = WalkerBehaviour::BySlope;
        [Triangle { v: [a, b, c], walker }, Triangle { v: [a, c, d], walker }]
    }

    fn face_x(x: f32, y0: f32, y1: f32) -> [Triangle; 2] {
        let (a, b, c, d) = (Vec3::new(x, y0, 0.0), Vec3::new(x, y0, 10.0), Vec3::new(x, y1, 10.0), Vec3::new(x, y1, 0.0));
        let walker = WalkerBehaviour::BySlope;
        [Triangle { v: [a, b, c], walker }, Triangle { v: [a, c, d], walker }]
    }

    /// Ground at `ground`, a block to x 6 with its top at `a`, a block from x `6 + gap` with its top at `b`.
    fn blocks(a: f32, b: f32, gap: f32, ground: f32) -> Vec<Triangle> {
        let mut t = quad(0.0, 14.0, ground).to_vec();
        t.extend(quad(0.0, 6.0, a));
        t.extend(face_x(6.0, ground, a));
        t.extend(quad(6.0 + gap, 14.0, b));
        t.extend(face_x(6.0 + gap, ground, b));
        t
    }

    fn grid(t: &[Triangle], flights: bool) -> WalkGrid {
        let body = Body { gap_flights: flights, exe_collision: true, start_witness: true, max_jump: STANDING_JUMP_M, ..Body::client() };
        WalkGrid::build_with_links(t, AREA, 0.25, body, OffMeshLinks::Gaps)
    }

    /// The flown links across x 6 (take-off before it, landing past `past`).
    fn across(g: &WalkGrid, past: f32) -> Vec<OffLink> {
        g.off_links().iter().copied().filter(|l| g.position(l.from).x < 6.0 && g.position(l.to).x > past && l.press != Takeoff::WalkOff).collect()
    }

    /// [incremental grids PR 7] A build through the memo is the full build, on `t` from `start`, for every site in `sites`
    /// (each must run with items): byte for byte ([`WalkGrid::digest_full`]) and its off links in memory, every field; a memo
    /// written (`write-only`), taken whole (`use`: every job of every site a hit), and run again and compared (`verify=1`).
    fn memo_build_is_the_full_build(t: &[Triangle], start: &[Vec3], sites: &[&str], name: &str) -> Vec<crate::grid::memo::SiteStats> {
        use crate::grid::memo::{Memo, Mode, SiteStats};
        let body = Body { gap_flights: true, exe_collision: true, start_witness: true, max_jump: STANDING_JUMP_M, ..Body::client() };
        let dir = std::env::temp_dir().join(format!("srp-memo-{name}-{}", std::process::id()));
        let build = |memo: &mut Memo| WalkGrid::build_kept_memo(t, None, AREA, 0.25, body, start, start, OffMeshLinks::Gaps, memo).0;
        let full_grid = build(&mut Memo::off());
        let (full, full_off) = (full_grid.digest_full(), full_grid.off_links().to_vec());
        assert!(!full_off.is_empty(), "{name}: off links to compare");
        let check = |memo: &Memo, grid: WalkGrid, what: &str, ok: &dyn Fn(&SiteStats) -> bool| {
            assert_eq!(grid.digest_full(), full, "{name} {what}");
            assert_eq!(grid.off_links(), &full_off[..], "{name} {what}: the off links in memory, every field");
            for site in sites {
                assert!(memo.stats.iter().any(|s| s.site == *site && s.items > 0), "{name} {what}: site {site} ran with items: {:?}", memo.stats);
            }
            for s in &memo.stats {
                assert!(ok(s), "{name} {what}: {s:?}");
            }
        };
        let mut written = Memo::new(Mode::WriteOnly, dir.clone(), 11);
        let g = build(&mut written);
        check(&written, g, "write-only", &|s| s.written == s.items && s.hits == 0);
        let mut used = Memo::new(Mode::Use, dir.clone(), 11);
        let g = build(&mut used);
        check(&used, g, "use", &|s| s.hits == s.items && s.misses == 0 && s.errors == 0);
        let use_stats = used.stats.clone();
        let mut verified = Memo::new(Mode::Verify { per_million: 1_000_000 }, dir.clone(), 11);
        let g = build(&mut verified);
        check(&verified, g, "verify=1", &|s| s.verified == s.items && s.errors == 0);
        let _ = std::fs::remove_dir_all(dir);
        use_stats
    }

    /// [incremental grids PR 7] The flown gaps through the memo build the full build's grid: the 1 m gap between level blocks.
    #[test]
    fn a_flown_gap_memo_build_is_the_full_build() {
        memo_build_is_the_full_build(&blocks(2.0, 2.0, 1.0, 0.0), &[Vec3::new(3.0, 2.0, 5.0)], &["gap.flown"], "gap-flown");
    }

    /// [incremental grids PR 8] The clip presses' proof through the memo builds the full build's grid, over the worlds of
    /// [`no_pressed_link_is_kept_unflown`] that have links (level gaps, a step up and down over a deep gully); the clip site
    /// has jobs in some.
    #[test]
    fn a_clip_press_memo_build_is_the_full_build() {
        let mut clip_items = 0;
        // Not the 6 m gap or the step down: they have no off link, so there is nothing to compare.
        for (a, b, gap) in [(2.0, 2.0, 1.0), (2.0, 2.0, 2.5), (2.0, 2.0, 4.0), (2.0, 2.9, 1.5)] {
            let t = blocks(a, b, gap, -3.0);
            let start = [Vec3::new(3.0, a, 5.0), Vec3::new(13.0, b, 5.0)];
            let stats = memo_build_is_the_full_build(&t, &start, &["gap.flown"], &format!("gap-clip-{a}-{b}-{gap}"));
            clip_items += stats.iter().filter(|s| s.site == "gap.clip").map(|s| s.items).sum::<usize>();
        }
        assert!(clip_items > 0, "the clip site had jobs");
    }

    /// [incremental grids PR 8] A clip result reads back as it was written: made at either end (share masks that are not the
    /// lowest bits, a walking press), not made; the flights and the reach.
    #[test]
    fn a_clip_press_result_reads_back() {
        use crate::grid::gap::{Aim, PressLead, Share};
        use crate::grid::memo::{Codec, MemoCodec, StableIds};
        let g = grid(&blocks(2.0, 2.0, 1.0, 0.0), true);
        let ids = StableIds::of(&g).expect("stable floor names");
        let made = |i: usize| Made {
            to: (i * 3 % g.len()) as NodeId,
            press: Takeoff::Sprint(PressLead::from_metres(1.0)),
            share: Share::landed(2).with_bits(0b0010_1000).expect("two bits"),
            walk: (i == 0).then_some((PressLead::from_metres(0.1), Share::landed(4).with_bits(0b1111_0000).expect("four bits"))),
            aim: Aim::new(6, -4).expect("an aim in range"),
            walker_lands: i == 1,
        };
        for out in [
            GapClipOut { made: Some((1, vec![made(0), made(1)])), flights: 77, reach_m: 5.5 },
            GapClipOut { made: None, flights: 3, reach_m: 1.0 },
            GapClipOut::default(),
        ] {
            let mut w = Codec::writer(&ids);
            out.encode(&mut w);
            let bytes = w.into_bytes();
            let mut r = Codec::reader(&ids, &bytes);
            let back = GapClipOut::decode(&mut r).expect("reads back");
            assert!(r.done());
            assert_eq!(format!("{back:?}"), format!("{out:?}"));
        }
    }

    /// [incremental grids PR 7] A flown gap result reads back as it was written: made at either end with several presses
    /// (share masks that are not the lowest bits, a walking press), made aimed, not made; the counts and the reach.
    #[test]
    fn a_flown_gap_result_reads_back() {
        use crate::grid::gap::{Aim, PressLead, Share};
        use crate::grid::memo::{Codec, MemoCodec, StableIds};
        let g = grid(&blocks(2.0, 2.0, 1.0, 0.0), true);
        let ids = StableIds::of(&g).expect("stable floor names");
        let lead = PressLead::from_metres(0.6);
        let made = |i: usize, press: Takeoff| Made {
            to: (i * 5 % g.len()) as NodeId,
            press,
            share: Share::landed(3).with_bits(0b0001_0110).expect("three bits"),
            walk: i.is_multiple_of(2).then_some((PressLead::from_metres(0.3), Share::landed(1).with_bits(0b1000_0000).expect("one bit"))),
            aim: Aim::new(-2, 3).expect("an aim in range"),
            walker_lands: i.is_multiple_of(2),
        };
        let outs = [
            GapFlownOut { made: Some((1, vec![made(0, Takeoff::Press(lead)), made(1, Takeoff::Sprint(lead))])), tried: 2, flights: 812, aimed: None, reach_m: 4.5 },
            GapFlownOut { made: Some((0, vec![made(2, Takeoff::Slow { lead, steps: 3 })])), tried: 2, flights: 1_000, aimed: Some(400), reach_m: 2.0 },
            GapFlownOut { made: None, tried: 1, flights: 33, aimed: Some(0), reach_m: 0.25 },
            GapFlownOut::default(),
        ];
        for out in outs {
            let mut w = Codec::writer(&ids);
            out.encode(&mut w);
            let bytes = w.into_bytes();
            let mut r = Codec::reader(&ids, &bytes);
            let back = GapFlownOut::decode(&mut r).expect("reads back");
            assert!(r.done());
            assert_eq!(format!("{back:?}"), format!("{out:?}"));
        }
    }

    /// A level gap of 1 m between blocks 2 m high: flown, the client's jump gets across, and each link carries its press and the
    /// share of its neighbourhood that lands (A385-FORMAT §2); the clip pass's links carry none.
    #[test]
    fn a_short_gap_is_jumped_with_a_flown_press_and_its_share() {
        let t = blocks(2.0, 2.0, 1.0, 0.0);
        let flown = across(&grid(&t, true), 7.0);
        assert!(!flown.is_empty(), "a flown link across");
        assert!(flown.iter().all(|l| l.kind == LinkKind::GapJump && l.share.flown().is_some_and(|n| n > 0)), "{flown:?}");
        let clip = across(&grid(&t, false), 7.0);
        assert!(clip.iter().all(|l| l.share.flown().is_none()), "{clip:?}");
    }

    /// [body physics S5a fix] A gap group is made only by a press that lands in every variant (its own neighbour, bit 0); of a
    /// partial line press and the aimed one, the better share is kept, the line's on a tie.
    #[test]
    fn a_partial_line_press_leaves_the_aimed_sweep_to_win_on_share() {
        use crate::grid::gap::{Aim, Mask, PressLead, Share};
        let made = |bits: u16| Made {
            to: 1,
            press: Takeoff::Sprint(PressLead::from_metres(1.4)),
            share: Share::of_mask(Mask::of_bits(bits).expect("bits of the neighbourhood")),
            walk: None,
            aim: Aim::STRAIGHT,
            walker_lands: false,
        };
        assert!(lands_whole(&made(0b1)), "the press itself lands in every variant");
        assert!(!lands_whole(&made(0b110)), "its neighbours do, it does not");
        assert!(!lands_whole(&made(0)));
        let line = Some((0u8, vec![made(0b10)]));
        let aimed = Some((1u8, vec![made(0b1_1110_0001)]));
        assert_eq!(better(line.clone(), aimed.clone()).map(|m| m.0), Some(1), "the aimed press's better share");
        assert_eq!(better(line.clone(), Some((1, vec![made(0b100)]))).map(|m| m.0), Some(0), "the line's on a tie");
        assert_eq!(better(None, aimed).map(|m| m.0), Some(1));
        assert_eq!(better(line, None).map(|m| m.0), Some(0));
    }

    /// [body physics S5a fix] A level landing a walk reaches is no gap's (Chemical Plant batteries 27932: a hop along the
    /// take-off's own top outvoted the gap's landing): on one level top 2 m up a floor 2.5 m on is reached; over a 1 m gap
    /// between level tops, or past a 2 m wall standing on the top between them, it is not, and the gap pass may land there.
    #[test]
    fn a_level_landing_a_walk_reaches_is_no_gap_landing_but_one_past_a_wall_or_a_gap_is() {
        let reaches = |t: &[Triangle]| {
            let g = grid(t, true);
            let (a, b) = (g.locate(Vec3::new(5.0, 2.0, 5.0), 0.3).expect("a take-off"), g.locate(Vec3::new(7.5, 2.0, 5.0), 0.3).expect("a landing"));
            g.walk_reaches(a, b)
        };
        let top = quad(0.0, 14.0, 2.0).to_vec();
        assert!(reaches(&top), "one level top");
        assert!(!reaches(&blocks(2.0, 2.0, 1.0, 0.0)), "a 1 m gap between level tops");
        let mut walled = top.clone();
        walled.extend(face_x(6.0, 2.0, 4.0));
        assert!(!reaches(&walled), "a wall on the top between them");
    }

    /// [body physics S5b] L1: a link to every top the jumps land and stay on. On flat ground, a `lands` that puts each flight on
    /// the far floor `b` when it rests more than 3 m on from the take-off, else on a near floor `n` 2 m on (Cologne TDM's
    /// (45.32, 36.39, 20.12): the long presses reach `b`, the short ones land on the near top and stay). Step 2 links `b`; the
    /// near top gets its own link (`others`) from its best press, whose own flights all rest there. Where every flight rests
    /// on `b`'s top, no other link is made (the press of before).
    #[test]
    fn every_top_the_presses_land_and_stay_on_gets_a_link() {
        let t = quad(0.0, 14.0, 0.0).to_vec();
        let body = Body { gap_flights: true, exe_collision: true, start_witness: true, max_jump: STANDING_JUMP_M, ..Body::client() };
        let g = WalkGrid::build_with_links(&t, AREA, 0.25, body, OffMeshLinks::None);
        let (caster, s) = body.controller();
        let world = Collision::build(&t).with_caster(caster);
        let at = |x: f32| g.locate(Vec3::new(x, 0.0, 5.0), 0.3).expect("a floor");
        let (a, near, far) = (at(2.0), at(4.0), at(12.0));
        let takeoff = g.position(a);
        let kits = Kits { body: jump::WALK_CAP_MS, fastest: jump::WALK_CAP_MS };
        let split = |st: &[FlightStep]| st.last().map(|f| if f.feet.x - takeoff.x > 3.0 { far } else { near });
        // The jumps alone (no sprint climb: on flat ground it runs on and stops near, a second step-2 floor of its own).
        let near_leads: Vec<f32> = (0..=4).map(|i| 0.1 * i as f32).collect();
        let presses = [(jump::Kind::Walk, gap_leads()), (jump::Kind::Sprint, gap_leads()), (jump::Kind::Slow(0), near_leads)];
        let p = g.best_press_with(&world, &s, a, far, (1.0, 0.0), &presses, Aim::STRAIGHT, split, kits, g.fall_ground());
        assert_eq!(p.made.first().map(|m| m.to), Some(far), "step 2 links b, which some press reaches");
        let other = p.others.iter().find(|m| m.to == near).unwrap_or_else(|| panic!("the near top gets its own link: near {near} far {far} {p:?}"));
        assert!(lands_whole(other), "its press's own flights all land and stay there: {other:?}");
        let all_far = |_: &[FlightStep]| Some(far);
        let q = g.best_press_with(&world, &s, a, far, (1.0, 0.0), &presses, Aim::STRAIGHT, all_far, kits, g.fall_ground());
        assert!(q.others.is_empty(), "one top: no other link");
        assert_eq!(q.made.len(), 1);
    }

    /// [body physics S5a fix] The farthest flight the passes fly fits the memo's window ([`WalkGrid::flight_reach_m`]): every
    /// neighbour (the fastest kit's among them) of the sprint at the longest lead, every variant, off a 6 m edge onto the
    /// ground below, flying on to the ground ([`WalkGrid::fall_ground`]): every step within the window of its take-off.
    #[test]
    fn the_farthest_scoring_flight_fits_the_memo_window() {
        let mut t = quad(0.0, 14.0, 0.0).to_vec();
        t.extend(quad(0.0, 6.0, 6.0));
        t.extend(face_x(6.0, 0.0, 6.0));
        let fastest = 3.941;
        let body = Body { gap_flights: true, exe_collision: true, start_witness: true, max_jump: STANDING_JUMP_M, fastest_walk_cap: fastest, ..Body::client() };
        let g = WalkGrid::build_with_links(&t, AREA, 0.25, body, OffMeshLinks::Gaps);
        let (caster, s) = body.controller();
        let world = Collision::build(&t).with_caster(caster);
        let takeoff = g.position(g.locate(Vec3::new(5.6, 6.0, 5.0), 0.3).expect("a take-off"));
        let (kits, reach) = (Kits { body: jump::WALK_CAP_MS, fastest }, g.flight_reach_m());
        let (mut farthest, mut lowest) = (0.0f32, f32::MAX);
        for n in jump::neighbourhood() {
            let flights = crate::motion::flights::press_flights(&world, &s, takeoff, (1.0, 0.0), jump::Kind::Sprint, 1.6, n, kits, g.fall_ground()).expect("a run-up");
            for f in flights.iter().flatten() {
                farthest = farthest.max(f.feet.distance_xz(takeoff));
                lowest = lowest.min(f.feet.y);
            }
        }
        assert!(lowest < 0.1, "the flights fall on to the ground 6 m below: the lowest step {lowest} m");
        assert!(farthest <= reach, "the farthest step {farthest} m, the window {reach} m");
    }

    /// [S4] A deck 2 m up ending at a railing 0.25 m thick, its top 1.0 m over the deck, with a roof 1.0 m under the deck past it
    /// (the Chemical Plant train's case, `railings-design.md` §4): the way over the railing is a gap searched and flown
    /// ([`WalkGrid::over_thin_top`]); whether a press clears the railing is the flight's. Before S4 the railing ended the search.
    #[test]
    fn a_way_over_a_railing_is_searched_as_a_gap() {
        let mut t = quad(0.0, 14.0, -3.0).to_vec();
        t.extend(quad(0.0, 6.0, 2.0));
        t.extend(face_x(6.0, -3.0, 3.0));
        t.extend(quad(6.0, 6.25, 3.0));
        t.extend(face_x(6.25, 1.0, 3.0));
        t.extend(quad(6.25, 14.0, 1.0));
        t.extend(face_x(6.25, -3.0, 1.0));
        let body = Body { gap_flights: true, exe_collision: true, start_witness: true, max_jump: STANDING_JUMP_M, ..Body::client() };
        let start = [Vec3::new(3.0, 2.0, 5.0)];
        let (g, stats) = WalkGrid::build_kept_with_links(&t, AREA, 0.25, body, &start, &start, OffMeshLinks::Gaps);
        assert!(stats.gaps.over_thin > 0, "{:?}", stats.gaps);
        let over: Vec<OffLink> = g.off_links().iter().copied().filter(|l| g.position(l.from).x < 6.0 && g.position(l.to).x > 6.25).collect();
        println!("over the railing: {} made, {over:?}", stats.gaps.over_thin_made);
    }

    /// [F1] With the flights, no pressed link is kept unflown: the clip pass's presses are each proven by the client's jump onto
    /// their own top or not kept, over gaps of 1, 2.5, 4 and 6 m between level blocks and up and down a step and a half.
    #[test]
    fn no_pressed_link_is_kept_unflown() {
        let body = Body { gap_flights: true, exe_collision: true, start_witness: true, max_jump: STANDING_JUMP_M, ..Body::client() };
        for (a, b, gap) in [(2.0, 2.0, 1.0), (2.0, 2.0, 2.5), (2.0, 2.0, 4.0), (2.0, 2.0, 6.0), (2.0, 2.9, 1.5), (2.0, 0.8, 2.0)] {
            let t = blocks(a, b, gap, -3.0);
            let start = [Vec3::new(3.0, a, 5.0), Vec3::new(13.0, b, 5.0)];
            let (g, stats) = WalkGrid::build_kept_with_links(&t, AREA, 0.25, body, &start, &start, OffMeshLinks::Gaps);
            let unflown: Vec<&OffLink> = g
                .off_links()
                .iter()
                .filter(|l| !matches!(l.press, Takeoff::WalkOff | Takeoff::Release { .. }) && l.share.flown().is_none())
                .collect();
            assert!(unflown.is_empty(), "{a} {b} {gap}: {unflown:?}");
            assert_eq!(stats.gaps.clip_kept, 0, "{a} {b} {gap}: {:?}", stats.gaps);
            assert_eq!(stats.gaps.clip_tried, stats.gaps.clip_proven + stats.gaps.clip_unlanded, "{:?}", stats.gaps);
            // The level gaps a press of the client's crosses keep a flown pressed link across (Opus's review of #2175).
            if a == b && gap <= 4.0 {
                let crossing = across(&g, 6.0 + gap);
                assert!(crossing.iter().any(|l| l.share.flown().is_some()), "{gap} m: {crossing:?} {:?}", stats.gaps);
            }
        }
    }

    /// [row 31] A gap of 1.5 m between blocks 2.0 and 2.4 m high under a deck over the whole area (the Vector Laboratory battery
    /// ledge under its walkway): with the deck 4.4 m over the take-off the gap is searched under it and a flown link made
    /// across; with the deck 2.3 m over the take-off (no room for a jump under it) the deck ends the search as a wall.
    #[test]
    fn a_gap_under_a_deck_is_jumped_across() {
        let under = |deck: Option<f32>| {
            let mut t = blocks(2.0, 2.4, 1.5, -1.5);
            if let Some(deck) = deck {
                t.extend(quad(0.0, 14.0, deck));
            }
            let body = Body { gap_flights: true, exe_collision: true, start_witness: true, max_jump: STANDING_JUMP_M, ..Body::client() };
            let start = [Vec3::new(3.0, 2.0, 5.0), Vec3::new(11.0, 2.4, 5.0)];
            let (g, stats) = WalkGrid::build_kept_with_links(&t, AREA, 0.25, body, &start, &start, OffMeshLinks::Gaps);
            let links: Vec<OffLink> = across(&g, 7.5).into_iter().filter(|l| l.share.flown().is_some()).collect();
            println!("deck {deck:?}: {} flown links across; {:?}", links.len(), stats.gaps);
            links
        };
        assert!(!under(None).is_empty(), "a flown link with no deck");
        assert!(!under(Some(6.4)).is_empty(), "a flown link under the high deck");
        assert!(under(Some(4.3)).is_empty(), "a low deck is a wall");
    }

    /// [backward landing] The flight of Mamayev Kurgan TDM's GapJump (8.60, 25.20, 48.12) -> (10.10, 25.08, 48.12), pressed along
    /// the way (-1, 0) and coming down 1.5 m behind the take-off on its own level: on the blocks (a gap of 1 m at x 6..7), the
    /// take-off at the gap's far rim (x 7.4) with the way back over the gap (-1, 0), a flight resting at x 8.9 on the take-off's
    /// block lands on no floor along that way; the same flight with the way (1, 0) lands on it, and one resting across the gap
    /// at x 5.0 lands with the way (-1, 0).
    #[test]
    fn a_flight_coming_down_behind_the_takeoff_lands_on_no_floor_along_its_way() {
        let g = grid(&blocks(2.0, 2.0, 1.0, -3.0), false);
        let a = g.locate(Vec3::new(7.4, 2.0, 5.0), 0.2).expect("the take-off");
        let flight = |to_x: f32| -> Vec<FlightStep> {
            let from = g.position(a);
            let mut steps = vec![FlightStep { n: 0, feet: from, in_jump: false, on_ground: true }];
            for n in 1..=10 {
                let t = n as f32 / 10.0;
                let feet = Vec3::new(from.x + (to_x - from.x) * t, from.y + 0.5 * (1.0 - (2.0 * t - 1.0).powi(2)), from.z);
                steps.push(FlightStep { n, feet, in_jump: n < 10, on_ground: n == 10 });
            }
            let rest = Vec3::new(to_x, from.y, from.z);
            steps.extend((11..14).map(|n| FlightStep { n, feet: rest, in_jump: false, on_ground: true }));
            steps
        };
        let behind = flight(8.9);
        assert_eq!(g.gap_lands_on(&behind, a, (-1, 0), &[], f32::NEG_INFINITY, 2), None, "the proven link's flight, behind along (-1, 0)");
        // [body physics S5a fix] Along (1, 0) it rests ahead, but on the take-off's own block at its level: a walk reaches it, no
        // gap landing ([`WalkGrid::walk_reaches`]).
        assert_eq!(g.gap_lands_on(&behind, a, (1, 0), &[], f32::NEG_INFINITY, 2), None, "the same flight along (1, 0): a walk's");
        let across = flight(5.0);
        let landed = g.gap_lands_on(&across, a, (-1, 0), &[], f32::NEG_INFINITY, 2).expect("a flight across the gap lands");
        assert!(g.position(landed).x < 6.0, "{:?}", g.position(landed));
        assert_eq!(g.gap_lands_on(&across, a, (1, 0), &[], f32::NEG_INFINITY, 2), None, "across the gap is behind along (1, 0)");
    }

    /// A level gap of 6 m: no press of the client's lands across (the sprint jump from a 0.6-1.6 m lead comes down about 4 m on),
    /// no link.
    #[test]
    fn a_gap_no_press_crosses_has_no_flown_link() {
        let t = blocks(2.0, 2.0, 6.0, -3.0);
        assert!(across(&grid(&t, true), 11.0).is_empty());
    }

    /// [A369] A lower block 1.4 m down across a 2.5 m gully 3 m deep (Mamayev Kurgan's ridge -> plateau), a bridge at z 9-10 and
    /// stairs the way back: the client's jump comes down on the block; a link from the ridge to it, its landing more than a step
    /// down a GapDrop.
    #[test]
    fn a_lower_block_across_a_gully_is_jumped_to() {
        let mut t = blocks(2.0, 0.6, 2.5, -1.0);
        let walker = WalkerBehaviour::BySlope;
        let mut flat = |x0: f32, x1: f32, y: f32| {
            let (a, b, c, d) = (Vec3::new(x0, y, 9.0), Vec3::new(x1, y, 9.0), Vec3::new(x1, y, 10.0), Vec3::new(x0, y, 10.0));
            t.extend([Triangle { v: [a, b, c], walker }, Triangle { v: [a, c, d], walker }]);
        };
        flat(6.0, 8.5, 2.0);
        flat(8.5, 9.0, 1.65);
        flat(9.0, 9.5, 1.3);
        flat(9.5, 10.0, 0.95);
        let flown: Vec<OffLink> = across(&grid(&t, true), 8.5).into_iter().filter(|l| l.kind == LinkKind::GapDrop).collect();
        assert!(!flown.is_empty(), "a flown link to the lower block");
        assert!(flown.iter().all(|l| l.share.flown().is_some()), "{flown:?}");
    }
}
