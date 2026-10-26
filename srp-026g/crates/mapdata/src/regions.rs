//! The walk grid's coarse layer: the floors of each 8 m block cut into
//! regions, and the links between regions (hierarchical path finding, as
//! HPA* and the navigation meshes of games do it).
//!
//! A region is the floors of one block joined by walk links both ways and
//! of one label (the caller's: which spawn area, which anomalies cover the
//! floor), so any floor of a region reaches any other inside it, and a
//! filter of allowed floors that goes by the labels lets a region through
//! whole or not at all. Region A links to
//! region B when any floor of A links to a floor of B (a walk, drop or
//! jump; one way when the floor link is). So a way between two floors
//! exists exactly when one exists between their regions, and a search on
//! the regions (a few thousand, against up to 2.3 million floors on the
//! real maps) answers it in well under a millisecond.
//!
//! [`WalkGrid::route_search`] uses it: the regions' search gives the
//! corridor, the floors' A* runs inside the corridor only (what it expands
//! is the corridor's few thousand floors, not the map's), and its route is
//! the floors' own, so it bends no worse than a search of the whole grid
//! round anything the corridor holds. The regions' search keeps off the
//! regions the caller's region filter shuts, so a filter that goes by the
//! labels is met exactly; one that does not (the spots a bot got stuck
//! at) can still shut the corridor: then it is widened by the regions next
//! to it, and last the whole grid is searched.

use std::cell::RefCell;

use crate::grid::{Entry, LinkKind, NodeId, Open, Search, SlicedSearch, WalkGrid};
use crate::math::Vec3;

/// A block's side, in cells (8 m at 0.25 m cells).
pub const BLOCK_CELLS: usize = 32;
/// Rings of neighbour regions the first corridor takes in: with one, the
/// route found is the whole grid's within 1-2 % (`real_route_search`).
pub const CORRIDOR_RINGS: usize = 1;
/// Rings more when the allowed floors shut the corridor.
pub const WIDE_RINGS: usize = 3;

const NONE: u32 = u32::MAX;

/// See the module documentation.
#[derive(Clone, Debug, Default)]
pub struct Regions {
    /// Per floor, its region.
    of: Vec<u32>,
    /// Per region, the floor nearest its middle.
    middle: Vec<NodeId>,
    /// Per region, where its links out start in `out`; one more entry.
    out_start: Vec<u32>,
    /// The links out: the region reached and the cost (between the
    /// regions' middles, plus the link kind's penalty).
    out: Vec<(u32, f32)>,
    /// The links in, as `out_start` and `out` (the region linking).
    in_start: Vec<u32>,
    inn: Vec<u32>,
}

impl Regions {
    /// The regions of `grid`, cut by `label` (one per floor) when given.
    pub fn build(grid: &WalkGrid, label: Option<&[u32]>) -> Self {
        let n = grid.len();
        let same = |a: NodeId, b: NodeId| label.is_none_or(|l| l[a as usize] == l[b as usize]);
        // Floors of one block joined by walk links both ways.
        let mut root: Vec<u32> = (0..n as u32).collect();
        for a in 0..n as NodeId {
            let block = grid.block_of(a, BLOCK_CELLS);
            for (b, kind) in grid.links(a) {
                if kind != LinkKind::Walk
                    || b < a
                    || grid.block_of(b, BLOCK_CELLS) != block
                    || !same(a, b)
                {
                    continue;
                }
                if !grid.links(b).any(|(c, k)| c == a && k == LinkKind::Walk) {
                    continue;
                }
                let (ra, rb) = (find(&mut root, a), find(&mut root, b));
                if ra != rb {
                    root[ra.max(rb) as usize] = ra.min(rb);
                }
            }
        }
        let mut of = vec![NONE; n];
        let mut count = 0u32;
        for a in 0..n as NodeId {
            let r = find(&mut root, a);
            if of[r as usize] == NONE {
                of[r as usize] = count;
                count += 1;
            }
            of[a as usize] = of[r as usize];
        }
        drop(root);
        // The middles: the mean of each region's floors, then the floor
        // nearest it.
        let regions = count as usize;
        let mut sum = vec![(0.0f64, 0.0f64, 0.0f64, 0u32); regions];
        for a in 0..n as NodeId {
            let p = grid.position(a);
            let s = &mut sum[of[a as usize] as usize];
            s.0 += f64::from(p.x);
            s.1 += f64::from(p.y);
            s.2 += f64::from(p.z);
            s.3 += 1;
        }
        let mean: Vec<Vec3> = sum
            .iter()
            .map(|s| {
                let k = f64::from(s.3.max(1));
                Vec3::new((s.0 / k) as f32, (s.1 / k) as f32, (s.2 / k) as f32)
            })
            .collect();
        drop(sum);
        let mut middle = vec![(NONE, f32::MAX); regions];
        for a in 0..n as NodeId {
            let r = of[a as usize] as usize;
            let d = (grid.position(a) - mean[r]).length();
            if d < middle[r].1 {
                middle[r] = (a, d);
            }
        }
        let middle: Vec<NodeId> = middle.into_iter().map(|(a, _)| a).collect();
        // The links between regions, the cheapest kind of each pair.
        let mut pairs: Vec<(u32, u32, u8)> = Vec::new();
        for a in 0..n as NodeId {
            let ra = of[a as usize];
            for (b, kind) in grid.links(a) {
                let rb = of[b as usize];
                if ra != rb && grid.plans(a, b) {
                    pairs.push((ra, rb, kind as u8));
                }
            }
        }
        pairs.sort_unstable();
        pairs.dedup_by(|later, first| later.0 == first.0 && later.1 == first.1);
        let penalty = |kind: u8| {
            match kind {
                0 => LinkKind::Walk,
                1 => LinkKind::Drop,
                _ => LinkKind::Jump,
            }
            .penalty()
        };
        let mut out_start = Vec::with_capacity(regions + 1);
        let mut out = Vec::with_capacity(pairs.len());
        let mut k = 0;
        for r in 0..regions as u32 {
            out_start.push(out.len() as u32);
            while k < pairs.len() && pairs[k].0 == r {
                let (_, to, kind) = pairs[k];
                let cost = (grid.position(middle[to as usize]) - grid.position(middle[r as usize]))
                    .length()
                    + penalty(kind);
                out.push((to, cost));
                k += 1;
            }
        }
        out_start.push(out.len() as u32);
        let mut in_count = vec![0u32; regions + 1];
        for &(to, _) in &out {
            in_count[to as usize + 1] += 1;
        }
        for r in 0..regions {
            in_count[r + 1] += in_count[r];
        }
        let in_start = in_count.clone();
        let mut inn = vec![0u32; out.len()];
        for r in 0..regions as u32 {
            for &(to, _) in &out[out_start[r as usize] as usize..out_start[r as usize + 1] as usize]
            {
                inn[in_count[to as usize] as usize] = r;
                in_count[to as usize] += 1;
            }
        }
        Self {
            in_start,
            inn,
            of,
            middle,
            out_start,
            out,
        }
    }

    pub fn len(&self) -> usize {
        self.middle.len()
    }

    pub fn is_empty(&self) -> bool {
        self.middle.is_empty()
    }

    /// The region of `node`.
    pub fn of(&self, node: NodeId) -> u32 {
        self.of[node as usize]
    }

    /// The floor nearest region `r`'s middle (any floor of it stands for
    /// its label).
    pub fn middle(&self, r: u32) -> NodeId {
        self.middle[r as usize]
    }

    /// The links out of region `r`.
    pub fn out(&self, r: u32) -> &[(u32, f32)] {
        &self.out[self.out_start[r as usize] as usize..self.out_start[r as usize + 1] as usize]
    }

    /// The regions linking into region `r`.
    pub fn into(&self, r: u32) -> &[u32] {
        &self.inn[self.in_start[r as usize] as usize..self.in_start[r as usize + 1] as usize]
    }

    /// Into `digest` ([`crate::digest`]): every floor's region, every
    /// region's middle and links.
    pub fn digest_into(&self, digest: &mut crate::digest::Digest) {
        for array in [
            &self.of,
            &self.middle,
            &self.out_start,
            &self.in_start,
            &self.inn,
        ] {
            digest.len(array.len());
            array.iter().for_each(|&value| digest.u32(value));
        }
        digest.len(self.out.len());
        for &(region, cost) in &self.out {
            digest.u32(region);
            digest.f32(cost);
        }
    }

    /// Into a built file ([`crate::built`]).
    pub fn write_body(&self, out: &mut crate::built::Out) -> std::io::Result<()> {
        for array in [
            &self.of,
            &self.middle,
            &self.out_start,
            &self.in_start,
            &self.inn,
        ] {
            out.plain(array, |v| v.to_le_bytes())?;
        }
        out.plain(&self.out, |&(region, cost)| {
            let mut bytes = [0u8; 8];
            bytes[..4].copy_from_slice(&region.to_le_bytes());
            bytes[4..].copy_from_slice(&cost.to_bits().to_le_bytes());
            bytes
        })
    }

    /// From a built file, for a grid of `floors` floors: every index
    /// checked against its array.
    pub fn read_body(input: &mut crate::built::In, floors: usize) -> Result<Self, String> {
        let regions = Self {
            of: input.plain(u32::from_le_bytes)?,
            middle: input.plain(u32::from_le_bytes)?,
            out_start: input.plain(u32::from_le_bytes)?,
            in_start: input.plain(u32::from_le_bytes)?,
            inn: input.plain(u32::from_le_bytes)?,
            out: input.plain(|bytes: [u8; 8]| {
                let (region, cost) = bytes.split_at(4);
                (
                    u32::from_le_bytes(region.try_into().expect("4 bytes")),
                    f32::from_bits(u32::from_le_bytes(cost.try_into().expect("4 bytes"))),
                )
            })?,
        };
        let count = regions.middle.len();
        let starts = |start: &[u32], len: usize| {
            start.len() == count + 1
                && start[0] == 0
                && start[count] as usize == len
                && start.windows(2).all(|w| w[0] <= w[1])
        };
        let fine = regions.of.len() == floors
            && regions.of.iter().all(|&r| (r as usize) < count)
            && regions.middle.iter().all(|&node| (node as usize) < floors)
            && starts(&regions.out_start, regions.out.len())
            && starts(&regions.in_start, regions.inn.len())
            && regions.out.iter().all(|&(r, _)| (r as usize) < count)
            && regions.inn.iter().all(|&r| (r as usize) < count);
        if fine {
            Ok(regions)
        } else {
            Err("regions out of range".into())
        }
    }

    pub fn memory_bytes(&self) -> usize {
        self.of.len() * 4
            + self.middle.len() * 4
            + (self.out_start.len() + self.in_start.len()) * 4
            + self.out.len() * 12
    }

    /// The regions of the cheapest way from region `from` to `to` (both
    /// included), by A* over the regions; `None` when there is no way.
    fn corridor(
        &self,
        grid: &WalkGrid,
        from: u32,
        to: u32,
        region_ok: &impl Fn(u32) -> bool,
        marks: &mut Marks,
    ) -> Option<Vec<u32>> {
        let regions = self.len();
        marks.fit(regions);
        let (seen, closed) = marks.next();
        let Marks {
            mark,
            cost,
            parent,
            open,
            ..
        } = marks;
        let goal = grid.position(self.middle[to as usize]);
        let h = |r: u32| grid.position(self.middle[r as usize]).distance_xz(goal);
        open.clear();
        mark[from as usize] = seen;
        cost[from as usize] = 0.0;
        parent[from as usize] = NONE;
        open.push(Entry(h(from), from));
        while let Some(Entry(_, u)) = open.pop() {
            if mark[u as usize] == closed {
                continue;
            }
            mark[u as usize] = closed;
            if u == to {
                let mut way = vec![to];
                let mut at = to;
                while at != from {
                    at = parent[at as usize];
                    way.push(at);
                }
                return Some(way);
            }
            let cu = cost[u as usize];
            for &(v, step) in self.out(u) {
                let m = mark[v as usize];
                if m == closed || (v != to && !region_ok(v)) {
                    continue;
                }
                let c = cu + step;
                if m != seen || c < cost[v as usize] {
                    mark[v as usize] = seen;
                    cost[v as usize] = c;
                    parent[v as usize] = u;
                    open.push(Entry(c + h(v), v));
                }
            }
        }
        None
    }
}

fn find(root: &mut [u32], mut a: u32) -> u32 {
    while root[a as usize] != a {
        let up = root[root[a as usize] as usize];
        root[a as usize] = up;
        a = up;
    }
    a
}

/// A search's per-region arrays, kept by the thread, marked by a search
/// number as the floors' are.
#[derive(Default)]
struct Marks {
    mark: Vec<u32>,
    cost: Vec<f32>,
    parent: Vec<u32>,
    search: u32,
    open: Open,
}

impl Marks {
    fn fit(&mut self, n: usize) {
        if self.mark.len() < n {
            self.mark.resize(n, 0);
            self.cost.resize(n, f32::INFINITY);
            self.parent.resize(n, NONE);
        }
    }

    /// The marks of a new search: (seen, closed).
    fn next(&mut self) -> (u32, u32) {
        self.search += 1;
        if self.search >= u32::MAX / 2 - 1 {
            self.mark.fill(0);
            self.search = 1;
        }
        (2 * self.search, 2 * self.search + 1)
    }
}

thread_local! {
    static MARKS: RefCell<Marks> = RefCell::new(Marks::default());
}

/// Drops the thread's region marks ([`crate::free_search_memory`]).
pub(crate) fn free_marks() {
    let _ = MARKS.try_with(|marks| *marks.borrow_mut() = Marks::default());
}

/// The bytes of the thread's region marks.
pub(crate) fn marks_bytes() -> usize {
    MARKS
        .try_with(|marks| {
            let m = marks.borrow();
            m.mark.capacity() * 4
                + m.cost.capacity() * 4
                + m.parent.capacity() * 4
                + m.open.capacity() * std::mem::size_of::<u64>()
        })
        .unwrap_or(0)
}

/// How a [`WalkGrid::route_search`] went, for the logs and tests.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Stage {
    /// No way between the regions: answered without a floor searched.
    #[default]
    NoWay,
    /// Found inside the corridor.
    Corridor,
    /// Found inside the corridor and the regions next to it.
    Widened,
    /// Searched over the whole grid (the corridor shut by the filter).
    Whole,
}

impl Regions {
    /// The way from `from` to `to` on `grid` (these regions') over the
    /// floors `allowed` lets through (as [`WalkGrid::path_where`]), by the
    /// regions first (module documentation); `region_ok` shuts regions
    /// (the start's and the goal's never). With a `region_ok` that agrees
    /// with `allowed` on every floor of the region, a way is found whenever
    /// there is one. `budget` bounds only the last stage, the search of the
    /// whole grid when the corridor and its neighbours are shut.
    pub fn route_search(
        &self,
        grid: &WalkGrid,
        from: NodeId,
        to: NodeId,
        allowed: impl Fn(NodeId) -> bool,
        region_ok: impl Fn(u32) -> bool,
        budget: usize,
    ) -> (Search, Stage) {
        match self.start_route_search(grid, from, to, &allowed, &region_ok, budget) {
            Ok(mut search) => {
                let mut iters = usize::MAX;
                search
                    .step(self, grid, &allowed, &region_ok, &mut iters)
                    .expect("an unbounded step ends the search")
            }
            Err(done) => done,
        }
    }

    /// [`Self::route_search`] in slices ([`RouteSearch::step`]), each
    /// stage's floors in the same order, so it ends with the same answer.
    /// The checks and the regions' own search (a few thousand regions) are
    /// made here at once; `Err` when they answer it.
    pub fn start_route_search(
        &self,
        grid: &WalkGrid,
        from: NodeId,
        to: NodeId,
        allowed: impl Fn(NodeId) -> bool,
        region_ok: impl Fn(u32) -> bool,
        budget: usize,
    ) -> Result<RouteSearch, (Search, Stage)> {
        let n = grid.len();
        if from as usize >= n || to as usize >= n || !grid.may_reach(from, to) {
            return Err((Search::NoWay, Stage::NoWay));
        }
        // The goal is reached only from a floor that links into it and is
        // let through (or is the start): with none, there is no way, and a
        // search would only prove it the long way (a goal inside a zone the
        // body keeps out of: the whole grid flooded).
        if from != to && !grid.entered_from(to, |b| b == from || allowed(b)) {
            return Err((Search::NoWay, Stage::NoWay));
        }
        let (rf, rt) = (self.of(from), self.of(to));
        let corridor = MARKS.with(|marks| {
            let mut marks = marks.borrow_mut();
            self.corridor(grid, rf, rt, &region_ok, &mut marks)
        });
        let Some(way) = corridor else {
            return Err((Search::NoWay, Stage::NoWay));
        };
        let mut search = RouteSearch {
            from,
            to,
            budget,
            stage: Stage::Corridor,
            ring: way,
            inside: vec![0; self.len().div_ceil(64)],
            search: None,
            expanded: 0,
        };
        search.widen(self, grid, CORRIDOR_RINGS, &region_ok)?;
        Ok(search)
    }
}

/// A [`Regions::route_search`] under way ([`Regions::start_route_search`]).
pub struct RouteSearch {
    from: NodeId,
    to: NodeId,
    /// The whole grid's search's budget.
    budget: usize,
    /// The stage searching: [`Stage::Corridor`], [`Stage::Widened`] or
    /// [`Stage::Whole`].
    stage: Stage,
    /// The regions of the corridor searched now.
    ring: Vec<u32>,
    /// The same, a bit a region.
    inside: Vec<u64>,
    search: Option<SlicedSearch>,
    /// Floors expanded by the stages done.
    expanded: usize,
}

impl RouteSearch {
    /// Goes on for at most `iters` floors expanded (taken off it): the
    /// answer and the stage that gave it once the search ends, `None`
    /// while it goes on. `allowed` and `region_ok` must be the ones it was
    /// started with.
    pub fn step(
        &mut self,
        regions: &Regions,
        grid: &WalkGrid,
        allowed: impl Fn(NodeId) -> bool,
        region_ok: impl Fn(u32) -> bool,
        iters: &mut usize,
    ) -> Option<(Search, Stage)> {
        loop {
            let Some(search) = self.search.as_mut() else {
                return Some((Search::NoWay, self.stage));
            };
            let found = if self.stage == Stage::Whole {
                search.step(grid, &allowed, iters)?
            } else {
                let inside = &self.inside;
                search.step(
                    grid,
                    |v| {
                        let r = regions.of(v) as usize;
                        inside[r / 64] & (1 << (r % 64)) != 0 && allowed(v)
                    },
                    iters,
                )?
            };
            self.expanded += search.expanded();
            if self.stage == Stage::Whole || matches!(found, Search::Found(_)) {
                self.search = None;
                return Some((found, self.stage));
            }
            // Not in this corridor: wider, then the whole grid.
            let next = if self.stage == Stage::Corridor {
                self.stage = Stage::Widened;
                self.widen(regions, grid, WIDE_RINGS, &region_ok)
            } else {
                self.stage = Stage::Whole;
                self.search = None;
                grid.start_search(self.from, self.to, self.budget)
                    .map(|search| self.search = Some(search))
                    .map_err(|done| (done, Stage::Whole))
            };
            if let Err(done) = next {
                return Some(done);
            }
        }
    }

    /// Floors expanded so far.
    pub fn expanded(&self) -> usize {
        self.expanded + self.search.as_ref().map_or(0, SlicedSearch::expanded)
    }

    /// The stage's way so far ([`SlicedSearch::partial`]): over floors its
    /// filter lets through. `grid` is the one searched.
    pub fn partial(&self, grid: &WalkGrid) -> Option<crate::grid::Path> {
        self.search.as_ref().map(|search| search.partial(grid))
    }

    /// The corridor so far and `rings` of regions round it (those
    /// `region_ok` lets through), and its floors' search started.
    fn widen(
        &mut self,
        regions: &Regions,
        grid: &WalkGrid,
        rings: usize,
        region_ok: impl Fn(u32) -> bool,
    ) -> Result<(), (Search, Stage)> {
        let inside = &mut self.inside;
        inside.fill(0);
        let mut add = |r: u32| {
            let (word, bit) = (r as usize / 64, 1u64 << (r % 64));
            let new = inside[word] & bit == 0;
            inside[word] |= bit;
            new
        };
        for &r in &self.ring {
            add(r);
        }
        let mut edge = self.ring.clone();
        for _ in 0..rings {
            let mut next = Vec::new();
            for &r in &edge {
                let near = regions
                    .out(r)
                    .iter()
                    .map(|&(v, _)| v)
                    .chain(regions.into(r).iter().copied());
                for v in near {
                    if region_ok(v) && add(v) {
                        next.push(v);
                    }
                }
            }
            self.ring.extend_from_slice(&next);
            edge = next;
        }
        self.search = None;
        let search = grid
            .start_search(self.from, self.to, usize::MAX)
            .map_err(|done| (done, self.stage))?;
        self.search = Some(search);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A goal no let-through floor links into has no way, answered before
    /// any search, and the answer is the whole grid's.
    #[test]
    fn a_goal_shut_in_by_floors_kept_out_of_is_no_way_at_once() {
        let grid = crate::grid::tests::two_rooms();
        let regions = Regions::build(&grid, None);
        let from = grid.locate(Vec3::new(3.0, 0.0, 6.0), 1.0).unwrap();
        let to = grid.locate(Vec3::new(25.0, 0.0, 6.0), 1.0).unwrap();
        let goal = grid.position(to);
        // A 2 m ring round the goal is kept out of: a zone the body avoids.
        let allowed = |v: NodeId| grid.position(v).distance_xz(goal) > 2.0;
        let (found, stage) = regions.route_search(&grid, from, to, allowed, |_| true, usize::MAX);
        assert_eq!((found, stage), (Search::NoWay, Stage::NoWay));
        assert_eq!(grid.search(from, to, allowed, usize::MAX), Search::NoWay);
        // A lane through the ring, along z = the goal's: found, as the whole
        // grid's search finds it.
        let lane = |v: NodeId| {
            let p = grid.position(v);
            allowed(v) || ((p.z - goal.z).abs() < 0.2 && p.x <= goal.x)
        };
        let (found, _) = regions.route_search(&grid, from, to, lane, |_| true, usize::MAX);
        assert!(matches!(found, Search::Found(_)));
        assert!(matches!(
            grid.search(from, to, lane, usize::MAX),
            Search::Found(_)
        ));
        let next = grid.links(to).next().unwrap().0;
        // The start itself next to the goal: found.
        let (found, _) = regions.route_search(&grid, next, to, allowed, |_| true, usize::MAX);
        assert!(matches!(found, Search::Found(_)));
    }

    /// `entered_from` is "some floor let through links into it", for every
    /// floor of the grid.
    #[test]
    fn entered_from_agrees_with_every_link() {
        let grid = crate::grid::tests::two_rooms();
        let n = grid.len() as NodeId;
        let mut into = vec![Vec::new(); n as usize];
        for b in 0..n {
            for (to, _) in grid.links(b) {
                into[to as usize].push(b);
            }
        }
        for node in 0..n {
            let some = |b: NodeId| !b.is_multiple_of(3);
            assert_eq!(
                grid.entered_from(node, |_| true),
                !into[node as usize].is_empty()
            );
            assert_eq!(
                grid.entered_from(node, some),
                into[node as usize].iter().any(|&b| some(b)),
                "floor {node}"
            );
        }
    }

    /// The regions' search in slices of a few floors ends with the one
    /// search's answer at every stage (the corridor, widened, the whole
    /// grid), with other searches made between its slices.
    #[test]
    fn a_route_search_in_slices_ends_as_one() {
        let grid = crate::grid::tests::two_rooms();
        let regions = Regions::build(&grid, None);
        let a = grid.locate(Vec3::new(2.0, 0.0, 2.0), 1.0).unwrap();
        let b = grid.locate(Vec3::new(28.0, 0.0, 10.0), 1.0).unwrap();
        let open = |_: NodeId| true;
        // The door shut on the floors only: every corridor fails, the
        // whole grid's search says no way.
        let door = |n: NodeId| {
            let p = grid.position(n);
            !((14.0..16.0).contains(&p.x) && (5.0..7.0).contains(&p.z))
        };
        // A wall of shut floors across the first room but for a gap at its
        // far side: a way round inside the corridor.
        let detour = |n: NodeId| {
            let p = grid.position(n);
            !((7.0..7.5).contains(&p.x) && p.z < 11.0)
        };
        let mut stages = Vec::new();
        for allowed in [&open as &dyn Fn(NodeId) -> bool, &door, &detour] {
            let whole = regions.route_search(&grid, a, b, allowed, |_| true, usize::MAX);
            let mut sliced = regions
                .start_route_search(&grid, a, b, allowed, |_| true, usize::MAX)
                .unwrap();
            let mut slices = 0;
            let found = loop {
                let mut iters = 7;
                if let Some(found) = sliced.step(&regions, &grid, allowed, |_| true, &mut iters) {
                    break found;
                }
                slices += 1;
                let _ = regions.route_search(&grid, b, a, |_| true, |_| true, usize::MAX);
            };
            assert_eq!(found, whole);
            assert!(slices > 10, "{slices}");
            stages.push(found.1);
        }
        // The door's case went through every stage.
        assert_eq!(stages, [Stage::Corridor, Stage::Whole, Stage::Corridor]);
    }

    #[test]
    fn a_way_through_regions_matches_the_whole_search() {
        // Two rooms joined by a door, 30 x 12 m: several blocks, a wall.
        let grid = crate::grid::tests::two_rooms();
        let regions = Regions::build(&grid, None);
        assert!(regions.len() >= 4, "{}", regions.len());
        let a = grid.locate(Vec3::new(2.0, 0.0, 2.0), 1.0).unwrap();
        let b = grid.locate(Vec3::new(28.0, 0.0, 10.0), 1.0).unwrap();
        let whole = grid.path(a, b).unwrap();
        let (found, stage) = regions.route_search(&grid, a, b, |_| true, |_| true, usize::MAX);
        let Search::Found(path) = found else {
            panic!("{found:?}");
        };
        assert_eq!(stage, Stage::Corridor);
        assert!(
            path.cost <= whole.cost * 1.05,
            "{} {}",
            path.cost,
            whole.cost
        );
        // The door's floors labelled apart and shut: no way, known from
        // the regions alone.
        let door = |n: NodeId| {
            let p = grid.position(n);
            (14.0..16.0).contains(&p.x) && (5.0..7.0).contains(&p.z)
        };
        let label: Vec<u32> = (0..grid.len() as NodeId)
            .map(|n| u32::from(door(n)))
            .collect();
        let cut = Regions::build(&grid, Some(&label));
        assert!(cut.len() > regions.len());
        let (found, stage) = cut.route_search(
            &grid,
            a,
            b,
            |n| !door(n),
            |r| !door(cut.middle(r)),
            usize::MAX,
        );
        assert_eq!((found, stage), (Search::NoWay, Stage::NoWay));
    }
}
