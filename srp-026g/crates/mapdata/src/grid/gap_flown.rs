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
use super::rim::{gap_leads, Made};
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
    /// (`linked`), and at rest over such a floor: the floor it rests on.
    fn gap_lands_on(&self, steps: &[FlightStep], a: u32, linked: &[NodeId], over: f32, past: i32) -> Option<NodeId> {
        let h = self.height[a as usize];
        let reach = self.body.max_jump.min(MARGINAL_JUMP_M);
        let (i, k) = self.cell_xz(self.cell_of[a as usize] as usize);
        let onto = |feet: Vec3| {
            self.floor_under(feet).filter(|&g| {
                let y = self.height[g as usize];
                let (gi, gk) = self.cell_xz(self.cell_of[g as usize] as usize);
                (gi - i).abs().max((gk - k).abs()) >= past
                    && y - h <= reach
                    && h - y <= self.body.max_drop
                    && y > over
                    && self.ceiling[g as usize] - y >= self.body.stand_height
                    && !linked.contains(&g)
            })
        };
        let (_, rest) = jump::landing_and_rest(steps, |f| onto(f.feet).is_some())?;
        onto(rest.feet)
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
        let proven = super::controlled::parallel(&groups, |r| {
            let group = &presses[r.clone()];
            let ends: &[(OffLink, usize)] = if group.len() <= 2 { group } else { &[group[0], group[group.len() - 1]] };
            let mut flights = 0;
            for &(l, d) in ends {
                let (di, dk) = DIRS[d];
                let len = ((di * di + dk * dk) as f32).sqrt();
                let way = (di as f32 / len, dk as f32 / len);
                // As the flown groups: the floors `a` has a link to already make no landing.
                let linked: Vec<NodeId> = wide.links[l.from as usize].iter().copied().filter(|&f| f != NONE).collect();
                let over = self.clip_gap_bottom(wide, solids, l.from, d, l.to) + self.body.step;
                let lands = |st: &[FlightStep]| self.gap_lands_on(st, l.from, &linked, over, 2);
                let p = self.best_press(world, s, l.from, l.to, way, &leads, lands, kits);
                flights += p.flights;
                let made = if p.made.is_empty() && p.near {
                    let sweep = self.best_aimed(world, s, l.from, l.to, way, &lands, kits);
                    flights += sweep.flights;
                    sweep.made.into_iter().collect()
                } else {
                    p.made
                };
                if !made.is_empty() {
                    let links: Vec<(OffLink, usize)> = made
                        .into_iter()
                        .map(|m| {
                            let rise = self.height[m.to as usize] - self.height[l.from as usize];
                            let kind = if rise > -self.body.step { LinkKind::GapJump } else { LinkKind::GapDrop };
                            (OffLink { from: l.from, to: m.to, kind, press: m.press, share: m.share, walk: m.walk, aim: m.aim, way: Some(GridWay::of_dir(d)) }, d)
                        })
                        .collect();
                    return (links, flights);
                }
            }
            (Vec::new(), flights)
        });
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
        let flown = super::controlled::parallel(&groups, |r| {
            let mut flights = 0;
            let mut tried = 0;
            // At most its first and its last take-off (in floor order): the thinning keeps one link a group, and a group's ends
            // span its metre of edge. A bound of the pass's cost (the flights are the build's dearest step), our choice.
            let group = &gaps[r.clone()];
            let ends: &[Gap] = if group.len() <= 2 { group } else { &[group[0], group[group.len() - 1]] };
            let way = |g: &Gap| {
                let (di, dk) = DIRS[g.d];
                let l = ((di * di + dk * dk) as f32).sqrt();
                (di as f32 / l, dk as f32 / l)
            };
            // The floors `a` has a link to already (its sides and its off-mesh links): a landing there makes no link.
            let linked = |g: &Gap| -> Vec<NodeId> { wide.links[g.a as usize].iter().copied().filter(|&f| f != NONE).collect() };
            let mut near: Option<Gap> = None;
            for g in ends {
                tried += 1;
                let linked = linked(g);
                let super::rim::RimPress { made, flights: f, near: close, .. } =
                    self.best_press(world, s, g.a, g.b, way(g), &leads, |st| self.gap_lands_on(st, g.a, &linked, g.over, g.past), kits);
                flights += f;
                // Every press it makes (a jump's, and [S1] a sprint climb's onto its own floor), as the rim pass keeps them.
                if !made.is_empty() {
                    return (Some((*g, made)), tried, flights, None);
                }
                if close && near.is_none() {
                    near = Some(*g);
                }
            }
            // [R83] No press on the line made it and a flight came near the landing: the aimed presses, from that end
            // (`R83-FORMAT.md`; Mamayev Kurgan day_2's box from the rock pile is a gap made only aimed).
            if let Some(g) = near {
                let linked = linked(&g);
                let sweep = self.best_aimed(world, s, g.a, g.b, way(&g), &|st| self.gap_lands_on(st, g.a, &linked, g.over, g.past), kits);
                let (made, f) = (sweep.made, sweep.flights);
                flights += f;
                return (made.map(|m| (g, vec![m])), tried, flights, Some(f));
            }
            (None, tried, flights, None)
        });
        stats.over_thin = gaps.iter().filter(|g| g.thin).count();
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
        let proven = self.prove_clip_presses(wide, solids, &found, clip, world, s, kits, stats);
        found.extend(proven);
        found.sort_by_key(|&(l, d)| (l.from, d, l.to));
        let out = self.thin_gap_links(wide, found, stats);
        stats.clip_kept = out.iter().filter(|l| l.press != super::gap::Takeoff::WalkOff && l.share.flown().is_none()).count();
        (out, thin_tops)
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
