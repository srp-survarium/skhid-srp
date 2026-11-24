//! [CPU hot paths] The route search's lookups made at load time, for the bots' searches: a runtime-only copy of the search
//! step ([`SlicedSearch::step_hot`]) that asks a [`HotIndex`] where [`SlicedSearch::step`] binary-searches the grid's
//! sorted lists.
//!
//! Why: on the bots' bench (2026-11-24, c8c012d62, perf on batt m2 s1, tdm m6 s1 and batt m6 s1) the binary searches of
//! [`WalkGrid::riser`], [`WalkGrid::walk_marked`] and the off-mesh links' `partition_point`s were 36-49 % of the route
//! searches' CPU: one search of each list at every Walk link relaxed, on floors nearly all of which have no entry.
//!
//! What stays the same: every answer. A floor whose bit is clear has no entry in that list, so the binary search it skips
//! would have found none; a floor whose bit is set runs the same search as before; an off-mesh range is the same slice of
//! the same list, found by rank instead of by bisection. The copied functions keep the originals' expressions token for
//! token (tested link by link against them, `tests::the_hot_copy_prices_every_link_as_the_original`), so the floats and the
//! open list's order are the originals'.
//!
//! Why a copy and not an edit: the originals are inside the grid builder's source digest (`mapdata-srchash`, `SRC_GRID`),
//! and an edit there names every built grid anew. New items with new names are reached by no root, so the grids' and the
//! sight trees' keys stay as they are.

use super::*;

/// Per floor, whether it is in a set, and for each floor in it a range of one of the grid's sorted lists: one bit a floor,
/// a running count a 64 floors, and a range a floor of the set.
#[derive(Clone, Debug, Default)]
struct Ranked {
    words: Vec<u64>,
    /// Floors of the set before each word.
    before: Vec<u32>,
    /// Per floor of the set, in floor order: the range's first index and its length.
    ranges: Vec<(u32, u32)>,
}

impl Ranked {
    /// The set of floors whose `range_of` is not empty, and those ranges.
    fn ranked_of(floors: usize, mut range_of: impl FnMut(NodeId) -> (usize, usize)) -> Self {
        let mut words = vec![0u64; floors.div_ceil(64)];
        let mut before = Vec::with_capacity(words.len());
        let mut ranges = Vec::new();
        for (w, word) in words.iter_mut().enumerate() {
            before.push(ranges.len() as u32);
            for bit in 0..64 {
                let node = w * 64 + bit;
                if node >= floors {
                    break;
                }
                let (first, len) = range_of(node as NodeId);
                if len > 0 {
                    *word |= 1 << bit;
                    ranges.push((first as u32, len as u32));
                }
            }
        }
        Self { words, before, ranges }
    }

    /// `node`'s range (empty for a floor not in the set or past the grid's).
    #[inline]
    fn ranked_range(&self, node: NodeId) -> std::ops::Range<usize> {
        let (w, bit) = (node as usize >> 6, node & 63);
        let Some(&word) = self.words.get(w) else {
            return 0..0;
        };
        if word >> bit & 1 == 0 {
            return 0..0;
        }
        let rank = self.before[w] as usize + (word & ((1u64 << bit) - 1)).count_ones() as usize;
        let (first, len) = self.ranges[rank];
        first as usize..first as usize + len as usize
    }

    /// Whether `node` is in the set.
    #[inline]
    fn ranked_has(&self, node: NodeId) -> bool {
        self.words.get(node as usize >> 6).is_some_and(|word| word >> (node & 63) & 1 == 1)
    }

    fn ranked_bytes(&self) -> usize {
        self.words.len() * 8 + self.before.len() * 4 + self.ranges.len() * 8
    }
}

/// A grid's route-search lookups, made once a map ([`Self::for_grid`]) and read by [`SlicedSearch::step_hot`]. It answers
/// for the grid it was built from only: [`Self::hot_fits`] checks the floor count.
#[derive(Clone, Debug, Default)]
pub struct HotIndex {
    floors: usize,
    /// The floors some riser leaves ([`WalkGrid::riser`]): ranges into `riser`.
    riser: Ranked,
    /// The floors some marked walk leaves ([`WalkGrid::walk_marked`]): ranges into `walk_marked`.
    marked: Ranked,
    /// [`WalkGrid::off_out`]'s ranges, into `off`.
    off_out: Ranked,
    /// [`WalkGrid::off_into`]'s ranges, into `off_in`.
    off_into: Ranked,
}

impl HotIndex {
    /// The index of `grid`'s lists, each range taken from the grid's own lookups.
    pub fn for_grid(grid: &WalkGrid) -> Self {
        let floors = grid.len();
        // The lists are sorted by (from, to) (their lookups bisect them), so a floor's entries are one run.
        let riser = Ranked::ranked_of(floors, |node| {
            let first = grid.riser.partition_point(|&(f, _, _)| f < node);
            (first, grid.riser.partition_point(|&(f, _, _)| f <= node) - first)
        });
        let marked = Ranked::ranked_of(floors, |node| {
            let first = grid.walk_marked.partition_point(|&(f, _)| f < node);
            (first, grid.walk_marked.partition_point(|&(f, _)| f <= node) - first)
        });
        let base_off = grid.off.as_ptr() as usize;
        let off_out = Ranked::ranked_of(floors, |node| {
            let links = grid.off_out(node);
            if links.is_empty() {
                return (0, 0);
            }
            ((links.as_ptr() as usize - base_off) / std::mem::size_of::<OffLink>(), links.len())
        });
        let base_in = grid.off_in.as_ptr() as usize;
        let off_into = Ranked::ranked_of(floors, |node| {
            let links = grid.off_into(node);
            if links.is_empty() {
                return (0, 0);
            }
            ((links.as_ptr() as usize - base_in) / std::mem::size_of::<u32>(), links.len())
        });
        Self { floors, riser, marked, off_out, off_into }
    }

    /// Whether it was built for a grid of `grid`'s size.
    pub fn hot_fits(&self, grid: &WalkGrid) -> bool {
        self.floors == grid.len()
    }

    /// The bytes it holds.
    pub fn hot_bytes(&self) -> usize {
        self.riser.ranked_bytes() + self.marked.ranked_bytes() + self.off_out.ranked_bytes() + self.off_into.ranked_bytes()
    }

    /// Every floor's lookups by this index against the grid's own: `links_back`, the off-mesh slices out of and onto it,
    /// and for each of its links the riser, the marked walk and the off-mesh link. The floors and links checked, or the
    /// first difference. For the drift tests on the real maps (match-server `hot_paths.rs`).
    #[doc(hidden)]
    pub fn hot_check_grid(&self, grid: &WalkGrid) -> Result<(usize, usize), String> {
        if !self.hot_fits(grid) {
            return Err(format!("an index of {} floors for a grid of {}", self.floors, grid.len()));
        }
        let mut links = 0;
        for u in 0..grid.len() as NodeId {
            let plain: Vec<_> = grid.links_back(u).collect();
            if plain != grid.links_back_hot(self, u).collect::<Vec<_>>() {
                return Err(format!("links back of {u}"));
            }
            if grid.off_out(u) != self.hot_off_out(grid, u) || grid.off_into(u) != self.hot_off_into(grid, u) {
                return Err(format!("off-mesh slices of {u}"));
            }
            for (v, _, _) in plain {
                if grid.riser(u, v) != self.hot_riser(grid, u, v)
                    || grid.walk_marked(u, v) != self.hot_walk_marked(grid, u, v)
                    || grid.off_link(u, v) != self.hot_off_link(grid, u, v)
                {
                    return Err(format!("the lookups of {u} -> {v}"));
                }
                links += 1;
            }
        }
        Ok((grid.len(), links))
    }

    /// [`WalkGrid::riser`]: the grid's own search, for a floor some riser leaves only.
    #[inline]
    fn hot_riser(&self, grid: &WalkGrid, a: NodeId, b: NodeId) -> Option<Riser> {
        if !self.riser.ranked_has(a) {
            return None;
        }
        grid.riser(a, b)
    }

    /// [`WalkGrid::walk_marked`]: the grid's own search, for a floor some marked walk leaves only.
    #[inline]
    fn hot_walk_marked(&self, grid: &WalkGrid, a: NodeId, b: NodeId) -> bool {
        if !self.marked.ranked_has(a) {
            return false;
        }
        grid.walk_marked(a, b)
    }

    /// [`WalkGrid::off_out`].
    #[inline]
    fn hot_off_out<'g>(&self, grid: &'g WalkGrid, node: NodeId) -> &'g [OffLink] {
        &grid.off[self.off_out.ranked_range(node)]
    }

    /// [`WalkGrid::off_into`].
    #[inline]
    fn hot_off_into<'g>(&self, grid: &'g WalkGrid, node: NodeId) -> &'g [u32] {
        &grid.off_in[self.off_into.ranked_range(node)]
    }

    /// [`WalkGrid::off_link`]. copy-of: `WalkGrid::off_link` (the copy guard: match-server `hot_paths.rs`).
    #[inline]
    fn hot_off_link(&self, grid: &WalkGrid, from: NodeId, to: NodeId) -> Option<OffLink> {
        self.hot_off_out(grid, from).iter().find(|l| l.to == to).copied()
    }
}

impl WalkGrid {
    /// [`Self::links_back`] by `hot`'s lookups, the floor's column and row read from `ik` (as [`Self::position`] reads them)
    /// instead of worked out from its cell. copy-of: `WalkGrid::links_back` (the copy guard: match-server `hot_paths.rs`).
    #[inline]
    fn links_back_hot<'a>(&'a self, hot: &'a HotIndex, node: NodeId) -> impl Iterator<Item = (NodeId, LinkKind, u16)> + 'a {
        let a = node as usize;
        let cell = self.cell_of[a] as usize;
        let (i, k) = match self.ik.get(a) {
            Some(&[i, k]) => (i32::from(i), i32::from(k)),
            None => self.cell_xz(cell),
        };
        let index = node - self.start[cell];
        debug_assert!(index < u32::from(WAY_OFF), "a cell of 2047 floors or more");
        (0..8)
            .filter_map(move |d| {
                let (v, kind) = self.link_of(a, d, i, k)?;
                let reach = (self.how[a] >> (4 * d + 2)) & 3;
                Some((v, kind, (index | (d as u32) << 11 | reach << 14) as u16))
            })
            .chain(hot.hot_off_out(self, node).iter().map(move |l| {
                // Its place among the links onto its landing.
                let slot = hot
                    .hot_off_into(self, l.to)
                    .iter()
                    .position(|&j| self.off[j as usize].from == node)
                    .unwrap_or(0) as u16;
                (l.to, l.kind, WAY_OFF | slot << 11)
            }))
    }

    /// [`Self::link_cost_reaching`] by `hot`'s lookups: the same expressions in the same order. copy-of:
    /// `WalkGrid::link_cost_reaching` (the copy guard: match-server `hot_paths.rs`).
    pub fn link_cost_hot(&self, hot: &HotIndex, from: NodeId, to: NodeId, kind: LinkKind, reach: Reach) -> f32 {
        if !self.plans(from, to) {
            return f32::INFINITY;
        }
        let down = matches!(kind, LinkKind::Drop | LinkKind::GapDrop);
        if down && reach.fall != FallRoom::Any {
            let height = self.height[from as usize] - self.height[to as usize];
            let start = match (kind, hot.hot_off_link(self, from, to).map(|l| l.press)) {
                (LinkKind::GapDrop, Some(gap::Takeoff::Press(_) | gap::Takeoff::Slow { .. })) => FallStart::Short,
                (LinkKind::GapDrop, Some(gap::Takeoff::Sprint(_))) if reach.sprints() => FallStart::Sprint,
                (LinkKind::GapDrop, Some(gap::Takeoff::Sprint(_))) => FallStart::Short,
                _ => self.drop_landings(from, to).map_or(FallStart::WalkOff, FallStart::Landed),
            };
            if let Some((pain, pool, start)) = reach.lethal_fall(height, start) {
                note_fall_closed(FallClosed { from, to, height, start, pain, pool });
                return f32::INFINITY;
            }
        }
        let lip = if !down && self.is_lip(to) {
            LIP_PENALTY
        } else {
            0.0
        };
        let hurt = if down && self.body.held_floors {
            held_floors::fall_cost_m(self.height[from as usize] - self.height[to as usize])
        } else {
            0.0
        };
        let fall = if down {
            hurt
        } else if self.near_fall(to) {
            FALL_EDGE_PENALTY
        } else if self.near_lip(to) {
            LIP_PENALTY
        } else {
            0.0
        };
        let (p, q) = (self.position(from), self.position(to));
        if kind.is_off_mesh() {
            let press = match hot.hot_off_link(self, from, to) {
                None => None,
                Some(l) => match (reach.sprint, l.press, l.walk) {
                    (SprintReach::Cannot { .. }, gap::Takeoff::Climb, Some((lead, share))) if l.aim == Aim::STRAIGHT => {
                        Some((gap::Takeoff::Press(lead), share))
                    }
                    (SprintReach::Cannot { .. }, gap::Takeoff::Climb, _) => return f32::INFINITY,
                    (SprintReach::Cannot { .. }, gap::Takeoff::Sprint(_) | gap::Takeoff::SprintHeld(_), Some((lead, share))) => {
                        Some((gap::Takeoff::Press(lead), share))
                    }
                    (SprintReach::Cannot { .. }, gap::Takeoff::Sprint(_) | gap::Takeoff::SprintHeld(_), None) => return f32::INFINITY,
                    _ => Some((l.press, l.share)),
                },
            };
            let cost = gap_cost(kind, (q - p).length(), p.distance_xz(q), p.y - q.y);
            if let Some((press, share)) = press.filter(|(_, share)| share.flown().is_some()) {
                return cost + share.failed_tries() * (cost + press.run_up_m()) + lip + fall;
            }
            let takeoff = press.map_or(0.0, |(press, _)| press.penalty());
            return cost + takeoff + lip + fall;
        }
        let descent = if kind == LinkKind::Drop {
            DROP_DESCENT_PENALTY * (self.height[from as usize] - self.height[to as usize]).max(0.0)
        } else {
            0.0
        };
        let sprint = if kind == LinkKind::Jump && q.y - p.y > self.body.max_jump { SPRINT_JUMP_PENALTY } else { 0.0 };
        let riser = match (kind, hot.hot_riser(self, from, to)) {
            (LinkKind::Walk, Some(r)) => r.share().failed_tries() * ((q - p).length() + r.run_up_m()),
            _ => 0.0,
        };
        let marked = if kind == LinkKind::Walk && hot.hot_walk_marked(self, from, to) {
            gap::Share::landed(0).failed_tries() * (q - p).length()
        } else {
            0.0
        };
        if marked > riser {
            MARKED_WALK_PRICED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        (q - p).length() + kind.penalty() + lip + fall + descent + sprint + riser.max(marked)
    }
}

impl SlicedSearch {
    /// [`Self::step`] by `hot`'s lookups ([`WalkGrid::links_back_hot`], [`WalkGrid::link_cost_hot`]): the same floors in
    /// the same order, the same costs, the same answer. `hot` must be `grid`'s ([`HotIndex::hot_fits`]). copy-of:
    /// `SlicedSearch::step` (the copy guard: match-server `hot_paths.rs`).
    pub fn step_hot(
        &mut self,
        grid: &WalkGrid,
        hot: &HotIndex,
        allowed: impl Fn(NodeId) -> bool,
        weight: impl Fn(NodeId) -> f32,
        iters: &mut usize,
    ) -> Option<Search> {
        assert!(hot.hot_fits(grid), "a hot index of another grid");
        let Self {
            scratch:
                Scratch {
                    mark,
                    cost,
                    back,
                    open,
                    ..
                },
            from,
            to,
            goal,
            seen,
            closed,
            expanded,
            budget,
            reach,
            best,
            best_h,
        } = self;
        let reach = *reach;
        let max_rise = reach.max_rise();
        let (from, to, goal, seen, closed) = (*from, *to, *goal, *seen, *closed);
        loop {
            if *iters == 0 {
                return None;
            }
            let Some(Entry(f, u)) = open.pop() else {
                return Some(Search::NoWay);
            };
            if mark[u as usize] == closed {
                continue;
            }
            mark[u as usize] = closed;
            if u == to {
                let (nodes, kinds) = way(grid, back, from, to);
                return Some(Search::Found(Path {
                    nodes,
                    kinds,
                    cost: cost[to as usize],
                }));
            }
            *expanded += 1;
            if *expanded > *budget {
                return Some(Search::TooFar);
            }
            *iters -= 1;
            let cu = cost[u as usize];
            if f - cu < *best_h {
                (*best, *best_h) = (u, f - cu);
            }
            for (v, kind, way_back) in grid.links_back_hot(hot, u) {
                let m = mark[v as usize];
                if m == closed || (v != to && !allowed(v)) {
                    continue;
                }
                if kind == LinkKind::Jump
                    && max_rise.is_some_and(|top| grid.height[v as usize] - grid.height[u as usize] > top)
                {
                    continue;
                }
                let c = cu + grid.link_cost_hot(hot, u, v, kind, reach) * weight(v);
                if !c.is_finite() {
                    continue;
                }
                if m != seen || c < cost[v as usize] {
                    mark[v as usize] = seen;
                    cost[v as usize] = c;
                    back[v as usize] = way_back;
                    open.push(Entry(c + grid.position(v).distance_xz(goal), v));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::tests::{ledge_yard, two_rooms};

    /// The reaches the bots search with.
    fn reaches() -> Vec<Reach> {
        let mut all = vec![Reach::FULL, Reach::no_sprint(Some(STANDING_JUMP_M)), Reach::no_sprint(None)];
        let jumps = JumpEnds { short: JumpEnd { rise: 0.4, descent: 3.0 }, sprint: JumpEnd { rise: 0.6, descent: 4.0 } };
        for r in all.clone() {
            for (pain, second) in [(0.05, SecondLanding::Adds), (0.5, SecondLanding::Adds), (5.0, SecondLanding::Adds)] {
                all.push(Reach { fall: FallRoom::Survives(FallBody { pain, fall_scale: 1.0, jumps, second }), ..r });
            }
        }
        all
    }

    /// Every link of a grid (grid links and off-mesh ones), priced by both, for every reach: equal to the bit.
    fn same_prices(grid: &WalkGrid) -> usize {
        let hot = HotIndex::for_grid(grid);
        let mut links = 0;
        for u in 0..grid.len() as NodeId {
            let plain: Vec<_> = grid.links_back(u).collect();
            let fast: Vec<_> = grid.links_back_hot(&hot, u).collect();
            assert_eq!(plain, fast, "links back of {u}");
            assert_eq!(grid.off_out(u), hot.hot_off_out(grid, u), "off out of {u}");
            assert_eq!(grid.off_into(u), hot.hot_off_into(grid, u), "off into of {u}");
            for (v, kind, _) in plain {
                assert_eq!(grid.riser(u, v), hot.hot_riser(grid, u, v));
                assert_eq!(grid.walk_marked(u, v), hot.hot_walk_marked(grid, u, v));
                assert_eq!(grid.off_link(u, v), hot.hot_off_link(grid, u, v));
                for reach in reaches() {
                    let (a, b) = (grid.link_cost_reaching(u, v, kind, reach), grid.link_cost_hot(&hot, u, v, kind, reach));
                    assert_eq!(a.to_bits(), b.to_bits(), "{u} -> {v} {kind:?} {reach:?}");
                }
                links += 1;
            }
        }
        links
    }

    #[test]
    fn the_hot_copy_prices_every_link_as_the_original() {
        assert!(same_prices(&two_rooms()) > 1000);
        for (rise, ramp) in [(1.0, None), (0.9, None), (1.0, Some(7.0)), (2.5, None)] {
            assert!(same_prices(&ledge_yard(rise, ramp)) > 100);
        }
    }

    /// A ranked set gives each member's range and nothing for the others, at word edges too.
    #[test]
    fn a_ranked_set_gives_each_floor_its_range() {
        let mut rng = 0x9e37_79b9_7f4a_7c15u64;
        let mut next = || {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            rng
        };
        for floors in [0usize, 1, 63, 64, 65, 1000, 4097] {
            let want: Vec<(usize, usize)> = (0..floors).map(|_| if next() % 3 == 0 { ((next() % 1000) as usize, (next() % 5 + 1) as usize) } else { (0, 0) }).collect();
            let ranked = Ranked::ranked_of(floors, |n| want[n as usize]);
            for (n, &(first, len)) in want.iter().enumerate() {
                let got = ranked.ranked_range(n as NodeId);
                assert_eq!((got.start, got.len()), if len > 0 { (first, len) } else { (0, 0) }, "floor {n} of {floors}");
                assert_eq!(ranked.ranked_has(n as NodeId), len > 0);
            }
            assert_eq!(ranked.ranked_range(floors as NodeId), 0..0);
        }
    }

    /// The hot step and the plain one, slice for slice: the same answers, the same floors expanded, the same partial way,
    /// from random starts and goals, with a floor test and a weight.
    #[test]
    fn the_hot_step_searches_as_the_plain_one() {
        for grid in [two_rooms(), ledge_yard(1.0, Some(7.0)), ledge_yard(0.9, None)] {
            let hot = HotIndex::for_grid(&grid);
            let n = grid.len() as u64;
            let mut rng = 0x2545_f491_4f6c_dd1du64;
            let mut next = || {
                rng ^= rng << 13;
                rng ^= rng >> 7;
                rng ^= rng << 17;
                rng
            };
            for round in 0..200 {
                let (a, b) = ((next() % n) as NodeId, (next() % n) as NodeId);
                let shut = (next() % 7) as NodeId;
                let allowed = |v: NodeId| v % 7 != shut || round % 2 == 0;
                let weight = |v: NodeId| 1.0 + (v % 3) as f32 * 0.25;
                let reach = reaches()[round % reaches().len()];
                let (Ok(mut plain), Ok(mut fast)) =
                    (grid.start_search_reaching(a, b, 5000, reach), grid.start_search_reaching(a, b, 5000, reach))
                else {
                    continue;
                };
                let slice = 1 + (next() % 40) as usize;
                loop {
                    let (mut i1, mut i2) = (slice, slice);
                    let (r1, r2) = if round % 3 == 0 {
                        (plain.step(&grid, allowed, |_| 1.0, &mut i1), fast.step_hot(&grid, &hot, allowed, |_| 1.0, &mut i2))
                    } else {
                        (plain.step(&grid, allowed, weight, &mut i1), fast.step_hot(&grid, &hot, allowed, weight, &mut i2))
                    };
                    assert_eq!(r1, r2, "round {round}: {a} -> {b}");
                    assert_eq!((i1, plain.expanded()), (i2, fast.expanded()));
                    assert_eq!(plain.partial(&grid), fast.partial(&grid));
                    if r1.is_some() {
                        break;
                    }
                }
            }
        }
    }

    /// The regions' route search by the hot step and by the plain one, slice for slice, through its stages (corridor,
    /// widened, whole): the same answers, stages, floors expanded and partial ways.
    #[test]
    fn the_hot_route_search_is_the_plain_one() {
        use crate::regions::Regions;
        for grid in [two_rooms(), ledge_yard(1.0, Some(7.0)), ledge_yard(2.5, None)] {
            let regions = Regions::build(&grid, None);
            let hot = HotIndex::for_grid(&grid);
            let n = grid.len() as u64;
            let mut rng = 0x1234_5678_9abc_def1u64;
            let mut next = || {
                rng ^= rng << 13;
                rng ^= rng >> 7;
                rng ^= rng << 17;
                rng
            };
            let mut ended = 0;
            for round in 0..300 {
                let (a, b) = ((next() % n) as NodeId, (next() % n) as NodeId);
                let shut = (next() % 5) as u32;
                let allowed = |v: NodeId| v % 5 != shut || round % 2 == 0;
                let region_ok = |r: u32| r % 5 != shut || round % 4 < 2;
                let reach = reaches()[round % reaches().len()];
                let start = || regions.start_route_search_reaching(&grid, a, b, allowed, region_ok, 4000, reach);
                let (Ok(mut plain), Ok(mut fast)) = (start(), start()) else {
                    continue;
                };
                let slice = 1 + (next() % 50) as usize;
                loop {
                    let (mut i1, mut i2) = (slice, slice);
                    let r1 = plain.step(&regions, &grid, allowed, region_ok, |_| 1.0, &mut i1);
                    let r2 = fast.step_hot(&regions, &grid, &hot, allowed, region_ok, |_| 1.0, &mut i2);
                    assert_eq!(r1, r2, "round {round}: {a} -> {b}");
                    assert_eq!((i1, plain.expanded()), (i2, fast.expanded()));
                    assert_eq!(plain.partial(&grid), fast.partial(&grid));
                    if r1.is_some() {
                        ended += 1;
                        break;
                    }
                }
            }
            assert!(ended > 50);
        }
    }
}
