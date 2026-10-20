//! Routes a body can walk: the walk grid's A* path, pulled straight.
//!
//! A grid path goes cell to cell in eight directions: followed as it is,
//! a body would zig-zag and turn every few steps, which no player does.
//! So the path is pulled tight: from each corner the route goes straight
//! to the farthest later floor it can walk to in a line, and only real
//! corners are left (a door, the end of a wall). A jump or a drop is
//! always a corner of its own: the line may not cross it.
//!
//! A [`Route`] is followed by looking ahead along it ([`Route::progress`],
//! [`Route::point_at`]): the body steers to a point a few metres on, so it
//! rounds the corners in a curve as a player does, instead of reaching
//! each one and turning on the spot.

#[cfg(test)]
use mapdata::regions::Stage;
use mapdata::regions::{Regions, RouteSearch};
use mapdata::{LinkKind, NodeId, Search, Vec3, WalkGrid};

/// A straight line is not tried farther than this (metres): the check
/// walks every cell of it.
const MAX_STRAIGHT_M: f32 = 40.0;

/// A corner of a route.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Waypoint {
    pub at: Vec3,
    pub node: NodeId,
    /// Reached by a jump from the waypoint before.
    pub jump: bool,
    /// The leg to it from the waypoint before goes over a floor a standing
    /// body does not fit under (`WalkGrid::can_stand`): walked crouched.
    pub low: bool,
}

#[derive(Clone, Debug)]
pub struct Route {
    points: Vec<Waypoint>,
    /// The distance along the route to each point.
    along: Vec<f32>,
    /// The segment the body was last found on.
    segment: usize,
}

/// The searches of this thread so far, for the tests' counts.
#[cfg(test)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Searches {
    pub searches: u64,
    pub failed: u64,
    pub us: f64,
    /// By [`mapdata::regions::Stage`]: how many, and their µs.
    pub stages: [u64; 4],
    pub stage_us: [f64; 4],
    pub worst_us: f64,
    /// Floors expanded, in all and by the largest search.
    pub expanded: u64,
    pub worst_expanded: u64,
    /// Searches by floors expanded ([`EXPANDED_BUCKETS`]).
    pub by_expanded: [u64; 8],
    /// Searches that ran over more than one slice, and the slices.
    pub sliced: u64,
    pub slices: u64,
    /// Paths pulled straight ([`pull`]), their µs, the floors in them and
    /// the straight-line checks made.
    pub pulls: u64,
    pub pull_us: f64,
    pub pull_floors: u64,
    pub straight_checks: u64,
}

#[cfg(test)]
thread_local! {
    static SEARCHES: std::cell::Cell<Searches> = const {
        std::cell::Cell::new(Searches {
            searches: 0,
            failed: 0,
            us: 0.0,
            stages: [0; 4],
            stage_us: [0.0; 4],
            worst_us: 0.0,
            expanded: 0,
            worst_expanded: 0,
            by_expanded: [0; 8],
            sliced: 0,
            slices: 0,
            pulls: 0,
            pull_us: 0.0,
            pull_floors: 0,
            straight_checks: 0,
        })
    };
}

#[cfg(test)]
pub fn searches() -> Searches {
    SEARCHES.with(|s| s.get())
}

/// The worst search's time and floors forgotten: a run's own from here.
#[cfg(test)]
pub fn reset_worst() {
    SEARCHES.with(|s| {
        let mut c = s.get();
        c.worst_us = 0.0;
        c.worst_expanded = 0;
        s.set(c);
    });
}

/// The route from one floor to another, if there is one.
pub fn plan(grid: &WalkGrid, from: NodeId, to: NodeId) -> Option<Route> {
    plan_where(grid, from, to, |_| true)
}

/// [`plan`] over the floors `allowed` lets through (an anomaly's, a spawn
/// area's are not): the A* keeps off them, and so do the straight legs.
/// The search covers the whole grid; [`plan_in`] goes by regions.
pub fn plan_where(
    grid: &WalkGrid,
    from: NodeId,
    to: NodeId,
    allowed: impl Fn(NodeId) -> bool,
) -> Option<Route> {
    let path = grid.path_where(from, to, &allowed)?;
    Some(pull(grid, path.nodes, allowed))
}

/// [`plan_where`] by `regions` first ([`Regions::route_search`]): a goal
/// with no way is known at once, and a way is searched in a corridor of
/// the grid, not the whole of it. `region_ok` shuts a region whose floors
/// `allowed` shuts (the tactics' labels make them agree).
pub fn plan_in(
    grid: &WalkGrid,
    regions: &Regions,
    from: NodeId,
    to: NodeId,
    allowed: impl Fn(NodeId) -> bool,
    region_ok: impl Fn(u32) -> bool,
) -> Option<Route> {
    let mut iters = usize::MAX;
    match plan_start(grid, regions, from, to, allowed, region_ok, &mut iters) {
        Ok(route) => route,
        Err(_) => unreachable!("an unbounded search ends"),
    }
}

/// A [`plan_in`] under way ([`plan_start`]), as Detour's sliced path
/// queries: it goes on in [`Planning::resume`].
pub struct Planning {
    search: RouteSearch,
    #[cfg(test)]
    us: f64,
    #[cfg(test)]
    slices: u64,
}

/// [`plan_in`] for at most `iters` floors expanded (taken off it): the
/// route, or none, once the search ends within them; else the search,
/// to go on with the same `allowed` and `region_ok`. A search that ends
/// within them gives what [`plan_in`] gives.
// The Err is the search under way, which the bot keeps: not an error to box.
#[allow(clippy::result_large_err)]
pub fn plan_start(
    grid: &WalkGrid,
    regions: &Regions,
    from: NodeId,
    to: NodeId,
    allowed: impl Fn(NodeId) -> bool,
    region_ok: impl Fn(u32) -> bool,
    iters: &mut usize,
) -> Result<Option<Route>, Planning> {
    #[cfg(test)]
    let started = std::time::Instant::now();
    match regions.start_route_search(grid, from, to, &allowed, &region_ok, usize::MAX) {
        Err((found, _stage)) => {
            #[cfg(test)]
            count(&found, _stage, started.elapsed().as_secs_f64() * 1e6, 0, 1);
            Ok(finish(grid, found, allowed))
        }
        Ok(search) => {
            let mut planning = Planning {
                search,
                #[cfg(test)]
                us: started.elapsed().as_secs_f64() * 1e6,
                #[cfg(test)]
                slices: 0,
            };
            match planning.resume(grid, regions, allowed, region_ok, iters) {
                Some(route) => Ok(route),
                None => Err(planning),
            }
        }
    }
}

impl Planning {
    /// Goes on for at most `iters` floors expanded (taken off it): the
    /// route, or none, once the search ends; `None` while it goes on.
    pub fn resume(
        &mut self,
        grid: &WalkGrid,
        regions: &Regions,
        allowed: impl Fn(NodeId) -> bool,
        region_ok: impl Fn(u32) -> bool,
        iters: &mut usize,
    ) -> Option<Option<Route>> {
        #[cfg(test)]
        let started = std::time::Instant::now();
        let ended = self.search.step(regions, grid, &allowed, region_ok, iters);
        #[cfg(test)]
        {
            self.us += started.elapsed().as_secs_f64() * 1e6;
            self.slices += 1;
        }
        let (found, _stage) = ended?;
        #[cfg(test)]
        count(
            &found,
            _stage,
            self.us,
            self.search.expanded() as u64,
            self.slices,
        );
        Some(finish(grid, found, allowed))
    }

    /// Floors expanded so far.
    pub fn expanded(&self) -> usize {
        self.search.expanded()
    }

    /// The way so far toward the goal, pulled straight, as Detour's
    /// partial paths: a body heads that way while the search goes on.
    /// None till a floor past the start was expanded.
    pub fn partial(&self, grid: &WalkGrid, allowed: impl Fn(NodeId) -> bool) -> Option<Route> {
        let path = self.search.partial(grid)?;
        (path.nodes.len() >= 2).then(|| pull(grid, path.nodes, allowed))
    }
}

fn finish(grid: &WalkGrid, found: Search, allowed: impl Fn(NodeId) -> bool) -> Option<Route> {
    match found {
        Search::Found(path) => Some(pull(grid, path.nodes, allowed)),
        Search::NoWay | Search::TooFar => None,
    }
}

/// A search ended, into the thread's counts.
#[cfg(test)]
fn count(found: &Search, stage: Stage, us: f64, expanded: u64, slices: u64) {
    SEARCHES.with(|s| {
        let mut c = s.get();
        c.searches += 1;
        c.failed += u64::from(!matches!(found, Search::Found(_)));
        c.us += us;
        c.stages[stage as usize] += 1;
        c.stage_us[stage as usize] += us;
        c.worst_us = c.worst_us.max(us);
        c.expanded += expanded;
        c.worst_expanded = c.worst_expanded.max(expanded);
        let bucket = EXPANDED_BUCKETS
            .iter()
            .position(|&top| expanded <= top)
            .unwrap_or(EXPANDED_BUCKETS.len());
        c.by_expanded[bucket] += 1;
        c.sliced += u64::from(slices > 1);
        c.slices += slices;
        s.set(c);
    });
}

/// The tops of the counts of searches by floors expanded
/// ([`Searches::by_expanded`]); the last count is those above.
#[cfg(test)]
pub const EXPANDED_BUCKETS: [u64; 7] = [1_000, 5_000, 10_000, 20_000, 50_000, 100_000, 500_000];

/// A grid path pulled straight: only the real corners left (module
/// documentation).
fn pull(grid: &WalkGrid, nodes: Vec<NodeId>, allowed: impl Fn(NodeId) -> bool) -> Route {
    #[cfg(test)]
    let (started, floors) = (std::time::Instant::now(), nodes.len() as u64);
    #[cfg(test)]
    let checks = std::cell::Cell::new(0u64);
    let route = pull_straight(grid, nodes, &allowed, &|| {
        #[cfg(test)]
        checks.set(checks.get() + 1);
    });
    #[cfg(test)]
    SEARCHES.with(|s| {
        let mut c = s.get();
        c.pulls += 1;
        c.pull_us += started.elapsed().as_secs_f64() * 1e6;
        c.pull_floors += floors;
        c.straight_checks += checks.get();
        s.set(c);
    });
    route
}

fn pull_straight(
    grid: &WalkGrid,
    nodes: Vec<NodeId>,
    allowed: impl Fn(NodeId) -> bool,
    checked: &dyn Fn(),
) -> Route {
    // The kind of the link into each node.
    let mut into = vec![LinkKind::Walk; nodes.len()];
    for k in 1..nodes.len() {
        into[k] = grid
            .links(nodes[k - 1])
            .find(|(next, _)| *next == nodes[k])
            .map_or(LinkKind::Walk, |(_, kind)| kind);
    }
    let low = |from: usize, to: usize| nodes[from..=to].iter().any(|&n| !grid.can_stand(n));
    let mut points = vec![Waypoint {
        at: grid.position(nodes[0]),
        node: nodes[0],
        jump: false,
        low: false,
    }];
    let mut i = 0;
    while i + 1 < nodes.len() {
        // A jump or a drop is taken as it is, corner to corner.
        if into[i + 1] != LinkKind::Walk {
            i += 1;
            points.push(Waypoint {
                at: grid.position(nodes[i]),
                node: nodes[i],
                jump: into[i] == LinkKind::Jump,
                low: low(i - 1, i),
            });
            continue;
        }
        let mut j = i + 1;
        while j + 1 < nodes.len() && into[j + 1] == LinkKind::Walk && {
            checked();
            straight_where(grid, nodes[i], nodes[j + 1], &allowed)
        } {
            j += 1;
        }
        let from = i;
        i = j;
        points.push(Waypoint {
            at: grid.position(nodes[i]),
            node: nodes[i],
            jump: false,
            low: low(from, i),
        });
    }
    Route::new(points)
}

/// Whether a body walks from `a` to `b` in a straight line: every half
/// cell of the line is on a floor walked to from the one before (no jump,
/// no drop), and the line stays within a step's height of the floors.
pub fn straight(grid: &WalkGrid, a: NodeId, b: NodeId) -> bool {
    straight_where(grid, a, b, |_| true)
}

/// [`straight`] over allowed floors only.
pub fn straight_where(
    grid: &WalkGrid,
    a: NodeId,
    b: NodeId,
    allowed: impl Fn(NodeId) -> bool,
) -> bool {
    grid.walks_straight(a, b, MAX_STRAIGHT_M, allowed)
}

/// [`straight_where`] as it was before [`WalkGrid::walks_straight`]: the
/// floors' places looked up at every step. Kept to prove the new one gives
/// the same answers.
#[cfg(test)]
fn straight_by_places(
    grid: &WalkGrid,
    a: NodeId,
    b: NodeId,
    allowed: impl Fn(NodeId) -> bool,
) -> bool {
    let (from, to) = (grid.position(a), grid.position(b));
    let length = from.distance_xz(to);
    if length > MAX_STRAIGHT_M {
        return false;
    }
    let half = grid.cell_size() * 0.5;
    let steps = ((length / half).ceil() as usize).max(1);
    let mut node = a;
    for k in 1..=steps {
        let t = k as f32 / steps as f32;
        let point = Vec3::new(
            from.x + (to.x - from.x) * t,
            0.0,
            from.z + (to.z - from.z) * t,
        );
        let within = |n: NodeId| {
            let p = grid.position(n);
            (p.x - point.x).abs() <= half + 1e-4 && (p.z - point.z).abs() <= half + 1e-4
        };
        if within(node) {
            continue;
        }
        match grid
            .links(node)
            .filter(|(_, kind)| *kind == LinkKind::Walk)
            .map(|(next, _)| next)
            .find(|&next| within(next))
        {
            Some(next) if allowed(next) || next == b => node = next,
            _ => return false,
        }
    }
    node == b
}

impl Route {
    pub fn new(points: Vec<Waypoint>) -> Self {
        let mut along = Vec::with_capacity(points.len());
        let mut total = 0.0;
        for (k, point) in points.iter().enumerate() {
            if k > 0 {
                total += points[k - 1].at.distance_xz(point.at);
            }
            along.push(total);
        }
        Self {
            points,
            along,
            segment: 0,
        }
    }

    pub fn points(&self) -> &[Waypoint] {
        &self.points
    }

    pub fn length(&self) -> f32 {
        self.along.last().copied().unwrap_or(0.0)
    }

    pub fn end(&self) -> Vec3 {
        self.points.last().map_or(Vec3::ZERO, |point| point.at)
    }

    /// How far along the route the body at `feet` is, and how far it is
    /// off the route (both on the ground plane). The body is looked for on
    /// the segment it was last on and the next few, never behind: a route
    /// that passes the same place twice is followed in order.
    pub fn progress(&mut self, feet: Vec3) -> (f32, f32) {
        if self.points.len() < 2 {
            return (0.0, self.end().distance_xz(feet));
        }
        let last = self.points.len() - 2;
        let mut best = (f32::INFINITY, self.segment, 0.0);
        for segment in self.segment..=(self.segment + 3).min(last) {
            let (a, b) = (self.points[segment].at, self.points[segment + 1].at);
            let (dx, dz) = (b.x - a.x, b.z - a.z);
            let length2 = dx * dx + dz * dz;
            let t = if length2 < 1e-6 {
                0.0
            } else {
                (((feet.x - a.x) * dx + (feet.z - a.z) * dz) / length2).clamp(0.0, 1.0)
            };
            let (px, pz) = (a.x + dx * t, a.z + dz * t);
            let off = ((feet.x - px).powi(2) + (feet.z - pz).powi(2)).sqrt();
            if off < best.0 - 1e-3 {
                best = (off, segment, t * length2.sqrt());
            }
        }
        self.segment = best.1;
        (self.along[best.1] + best.2, best.0)
    }

    /// The point `s` metres along the route (the end past it).
    pub fn point_at(&self, s: f32) -> Vec3 {
        if s <= 0.0 || self.points.len() < 2 {
            return self.points.first().map_or(Vec3::ZERO, |point| point.at);
        }
        let k = self.along.partition_point(|&a| a <= s);
        if k >= self.points.len() {
            return self.end();
        }
        let (a, b) = (self.points[k - 1].at, self.points[k].at);
        let span = self.along[k] - self.along[k - 1];
        let t = if span < 1e-6 {
            1.0
        } else {
            (s - self.along[k - 1]) / span
        };
        a + (b - a) * t
    }

    /// From `s` on: the distance to the next corner that turns by more than
    /// `angle` radians (or to the end), and that corner's waypoint index.
    pub fn straight_ahead(&self, s: f32, angle: f32) -> (f32, Option<usize>) {
        let first = self.along.partition_point(|&a| a <= s);
        for k in first..self.points.len().saturating_sub(1) {
            if self.points[k + 1].jump || self.turn_at(k) > angle {
                return (self.along[k] - s, Some(k));
            }
        }
        ((self.length() - s).max(0.0), None)
    }

    /// The next waypoint after `s` that is reached by a jump: its index and
    /// the distance to the waypoint before it (where the jump starts).
    pub fn next_jump(&self, s: f32) -> Option<(usize, f32)> {
        let first = self.along.partition_point(|&a| a <= s).max(1);
        (first..self.points.len())
            .find(|&k| self.points[k].jump)
            .map(|k| (k, self.along[k - 1] - s))
    }

    /// Whether the route from `s` to `within` metres on goes over a floor
    /// a standing body does not fit under ([`Waypoint::low`]).
    pub fn low_ahead(&self, s: f32, within: f32) -> bool {
        let first = self.along.partition_point(|&a| a <= s).max(1);
        (first..self.points.len())
            .take_while(|&k| self.along[k - 1] <= s + within)
            .any(|k| self.points[k].low)
    }

    /// How much the route turns at waypoint `k`, radians (0 at the ends).
    pub fn turn_at(&self, k: usize) -> f32 {
        if k == 0 || k + 1 >= self.points.len() {
            return 0.0;
        }
        let (a, b, c) = (
            self.points[k - 1].at,
            self.points[k].at,
            self.points[k + 1].at,
        );
        let (ux, uz, vx, vz) = (b.x - a.x, b.z - a.z, c.x - b.x, c.z - b.z);
        let (nu, nv) = ((ux * ux + uz * uz).sqrt(), (vx * vx + vz * vz).sqrt());
        if nu < 1e-4 || nv < 1e-4 {
            return 0.0;
        }
        ((ux * vx + uz * vz) / (nu * nv)).clamp(-1.0, 1.0).acos()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v026g::bots::body::tests::walled;

    #[test]
    fn a_path_round_a_wall_is_pulled_to_a_few_corners() {
        let grid = walled();
        let from = grid.locate(Vec3::new(10.0, 0.0, 10.0), 1.0).unwrap();
        let to = grid.locate(Vec3::new(30.0, 0.0, 10.0), 1.0).unwrap();
        let path = grid.path(from, to).unwrap();
        let route = plan(&grid, from, to).unwrap();
        // Cell by cell the path has a hundred-odd floors; pulled straight,
        // a few corners at the wall's end.
        assert!(path.nodes.len() > 80, "{}", path.nodes.len());
        assert!(
            (3..=6).contains(&route.points().len()),
            "{:?}",
            route.points()
        );
        // Each leg is walkable straight, and it goes round past z = 30.
        for pair in route.points().windows(2) {
            assert!(straight(&grid, pair[0].node, pair[1].node));
        }
        assert!(route.points().iter().any(|point| point.at.z > 30.0));
        // About as long as the grid path, shorter than its zig-zag.
        assert!(route.length() <= path.cost + 0.01);
        assert!(route.length() > 2.0 * (20.0_f32.hypot(10.0)) - 1.0);
        // A line through the wall is not straight.
        assert!(!straight(&grid, from, to));
        // On open floor the route is one line.
        let near = grid.locate(Vec3::new(12.0, 0.0, 25.0), 1.0).unwrap();
        assert_eq!(plan(&grid, from, near).unwrap().points().len(), 2);
    }

    /// Lines between random floors, over all floors and with round holes
    /// shut: [`WalkGrid::walks_straight`] answers as the check by the
    /// floors' places did, on `grid`; how many were straight, and the
    /// microseconds of each check (the new, the old).
    fn straight_as_before(
        grid: &WalkGrid,
        seed: u64,
        pairs: usize,
        near_m: f32,
    ) -> (usize, f64, f64) {
        let mut rng = crate::v026g::bots::rng::Rng::new(seed);
        let mut lines = Vec::with_capacity(pairs);
        while lines.len() < pairs {
            let a = rng.below(grid.len()) as NodeId;
            let b = rng.below(grid.len()) as NodeId;
            let (pa, pb) = (grid.position(a), grid.position(b));
            if pa.distance_xz(pb) > near_m {
                continue;
            }
            let holes: Vec<Vec3> = (0..lines.len() % 3)
                .map(|_| pa + (pb - pa) * rng.range(0.2, 0.8))
                .collect();
            lines.push((a, b, holes));
        }
        let allowed = |holes: &[Vec3], n: NodeId| {
            let p = grid.position(n);
            holes.iter().all(|h| h.distance_xz(p) > 1.0)
        };
        // Each check over every line, in turns, the best of three: the
        // same lines in the same order, the caches as warm for both.
        let (mut new, mut old) = (Vec::new(), Vec::new());
        let (mut new_us, mut old_us) = (f64::MAX, f64::MAX);
        for _ in 0..3 {
            let started = std::time::Instant::now();
            new = lines
                .iter()
                .map(|(a, b, holes)| straight_where(grid, *a, *b, |n| allowed(holes, n)))
                .collect();
            new_us = new_us.min(started.elapsed().as_secs_f64() * 1e6);
            let started = std::time::Instant::now();
            old = lines
                .iter()
                .map(|(a, b, holes)| straight_by_places(grid, *a, *b, |n| allowed(holes, n)))
                .collect();
            old_us = old_us.min(started.elapsed().as_secs_f64() * 1e6);
        }
        for (k, (a, b, holes)) in lines.iter().enumerate() {
            assert_eq!(new[k], old[k], "floors {a} to {b}, holes {holes:?}");
        }
        let straight_lines = new.iter().filter(|&&straight| straight).count();
        (straight_lines, new_us / pairs as f64, old_us / pairs as f64)
    }

    #[test]
    fn straight_lines_are_checked_as_before() {
        let grid = walled();
        let (straight_lines, _, _) = straight_as_before(&grid, 7, 20_000, 45.0);
        // Both answers are met often.
        assert!(
            (2_000..18_000).contains(&straight_lines),
            "{straight_lines} of 20000 straight"
        );
    }

    /// [`straight_lines_are_checked_as_before`] on the real maps, only
    /// with `SRP_MAPDATA_DIR`, and the time of a check:
    ///
    /// ```text
    /// SRP_MAPDATA_DIR=<dir> cargo test -p match-server --release real_straight_lines -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore = "a measurement; needs SRP_MAPDATA_DIR"]
    fn real_straight_lines() {
        let Some(dir) = std::env::var_os("SRP_MAPDATA_DIR").map(std::path::PathBuf::from) else {
            eprintln!("skipping: SRP_MAPDATA_DIR is not set");
            return;
        };
        for project in [
            "level_06/day_2",
            "level_01/day_4",
            "level_03/day_3",
            "level_02/day_2",
        ] {
            let map =
                mapdata::MapData::read(&dir.join(mapdata::format::file_name(project))).unwrap();
            let world = mapdata::World::build(map, mapdata::CELL, mapdata::Body::default());
            let (straight_lines, new_us, old_us) =
                straight_as_before(&world.grid, 11, 100_000, 45.0);
            eprintln!(
                "{project}: {straight_lines} of 100000 straight, the same answers; \
                 {new_us:.2} µs a check, was {old_us:.2}"
            );
        }
    }

    #[test]
    fn progress_and_points_along_a_route() {
        let p = |x, z| Waypoint {
            at: Vec3::new(x, 0.0, z),
            node: 0,
            jump: false,
            low: false,
        };
        let mut route = Route::new(vec![p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0)]);
        assert_eq!(route.length(), 20.0);
        let (s, off) = route.progress(Vec3::new(4.0, 0.0, 1.0));
        assert!((s - 4.0).abs() < 1e-4 && (off - 1.0).abs() < 1e-4);
        assert_eq!(route.point_at(15.0), Vec3::new(10.0, 0.0, 5.0));
        assert_eq!(route.point_at(99.0), Vec3::new(10.0, 0.0, 10.0));
        // A right angle at the corner, 6 m ahead.
        let (ahead, corner) = route.straight_ahead(4.0, 0.5);
        assert!((ahead - 6.0).abs() < 1e-4 && corner == Some(1));
        assert!((route.turn_at(1) - std::f32::consts::FRAC_PI_2).abs() < 1e-4);
        // Progress never goes back to an earlier segment.
        let (s, _) = route.progress(Vec3::new(10.5, 0.0, 6.0));
        assert!((s - 16.0).abs() < 1e-4);
        let (s, _) = route.progress(Vec3::new(5.0, 0.0, 0.0));
        assert!(s >= 10.0, "{s}");
    }

    /// The search budget a real way needs, only with `SRP_MAPDATA_DIR`:
    /// random pairs of floors 10-150 m apart on each map, the least of a
    /// few budgets that finds the way, and the time of a full search.
    ///
    /// ```text
    /// SRP_MAPDATA_DIR=<dir> cargo test -p match-server --release real_search_budget -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore = "a measurement; needs SRP_MAPDATA_DIR"]
    fn real_search_budget() {
        use crate::v026g::bots::rng::Rng;
        use mapdata::Search;
        let Some(dir) = std::env::var_os("SRP_MAPDATA_DIR").map(std::path::PathBuf::from) else {
            eprintln!("skipping: SRP_MAPDATA_DIR is not set");
            return;
        };
        const BUDGETS: [usize; 7] = [5_000, 10_000, 20_000, 50_000, 100_000, 200_000, 500_000];
        for level in 1..=8 {
            let project = format!("level_{level:02}/day_2");
            let map =
                mapdata::MapData::read(&dir.join(mapdata::format::file_name(&project))).unwrap();
            let world = mapdata::World::build(map, mapdata::CELL, mapdata::Body::default());
            let grid = &world.grid;
            let mut rng = Rng::new(level);
            let (mut needed, mut none, mut over) = ([0usize; BUDGETS.len()], 0, 0);
            let (mut found_us, mut found_n, mut worst_us) = (0.0, 0, 0.0f64);
            let mut tried = 0;
            while tried < 300 {
                let a = rng.below(grid.len()) as NodeId;
                let b = rng.below(grid.len()) as NodeId;
                let d = grid.position(a).distance_xz(grid.position(b));
                if !(10.0..=150.0).contains(&d) || !grid.may_reach(a, b) {
                    continue;
                }
                tried += 1;
                let started = std::time::Instant::now();
                let full = grid.search(a, b, |_| true, usize::MAX);
                let us = started.elapsed().as_secs_f64() * 1e6;
                worst_us = worst_us.max(us);
                if !matches!(full, Search::Found(_)) {
                    none += 1;
                    continue;
                }
                found_us += us;
                found_n += 1;
                match BUDGETS.iter().position(|&budget| {
                    matches!(grid.search(a, b, |_| true, budget), Search::Found(_))
                }) {
                    Some(k) => needed[k] += 1,
                    None => over += 1,
                }
            }
            eprintln!(
                "{project}: {} floors; of {tried} pairs in one piece, {none} with no way; ways found within {:?}: {:?}, more {over}; full search {:.0} µs on average when found, worst {:.0} µs",
                grid.len(),
                BUDGETS,
                needed,
                found_us / found_n.max(1) as f64,
                worst_us
            );
        }
    }

    /// The regions' search ([`mapdata::regions`]) against the search of
    /// the whole grid on the real maps, only with `SRP_MAPDATA_DIR`: random
    /// pairs of floors 10-150 m apart, with no filter and with round
    /// "anomalies" shut (8 of 6 m on the way): the same answer on whether
    /// there is a way, the route's cost, the time.
    ///
    /// ```text
    /// SRP_MAPDATA_DIR=<dir> cargo test -p match-server --release real_route_search -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore = "a measurement; needs SRP_MAPDATA_DIR"]
    fn real_route_search() {
        use crate::v026g::bots::rng::Rng;
        use mapdata::Search;
        use mapdata::regions::Stage;
        let Some(dir) = std::env::var_os("SRP_MAPDATA_DIR").map(std::path::PathBuf::from) else {
            eprintln!("skipping: SRP_MAPDATA_DIR is not set");
            return;
        };
        for level in 1..=8 {
            let project = format!("level_{level:02}/day_2");
            let map =
                mapdata::MapData::read(&dir.join(mapdata::format::file_name(&project))).unwrap();
            let started = std::time::Instant::now();
            let world = mapdata::World::build(map, mapdata::CELL, mapdata::Body::default());
            let built = started.elapsed().as_secs_f32();
            let grid = &world.grid;
            let regions = mapdata::regions::Regions::build(grid, None);
            let mut rng = Rng::new(level);
            for filtered in [false, true] {
                let (mut tried, mut differ, mut worse) = (0, 0, 0usize);
                let (mut ratio_sum, mut ratio_max, mut found) = (0.0f64, 0.0f64, 0);
                let (mut whole_us, mut regions_us, mut regions_worst) = (0.0, 0.0, 0.0f64);
                let mut stages = [0usize; 4];
                while tried < 300 {
                    let a = rng.below(grid.len()) as NodeId;
                    let b = rng.below(grid.len()) as NodeId;
                    let (pa, pb) = (grid.position(a), grid.position(b));
                    let d = pa.distance_xz(pb);
                    if !(10.0..=150.0).contains(&d) {
                        continue;
                    }
                    tried += 1;
                    let holes: Vec<Vec3> = if filtered {
                        (0..8)
                            .map(|_| {
                                let t = rng.range(0.15, 0.85);
                                let p = pa + (pb - pa) * t;
                                Vec3::new(
                                    p.x + rng.range(-8.0, 8.0),
                                    p.y,
                                    p.z + rng.range(-8.0, 8.0),
                                )
                            })
                            .collect()
                    } else {
                        Vec::new()
                    };
                    let allowed = |n: NodeId| {
                        let p = grid.position(n);
                        holes.iter().all(|h| h.distance_xz(p) > 3.0)
                    };
                    let t = std::time::Instant::now();
                    let whole = grid.search(a, b, allowed, usize::MAX);
                    whole_us += t.elapsed().as_secs_f64() * 1e6;
                    let t = std::time::Instant::now();
                    let (by_regions, stage) =
                        regions.route_search(grid, a, b, allowed, |_| true, usize::MAX);
                    let us = t.elapsed().as_secs_f64() * 1e6;
                    regions_us += us;
                    regions_worst = regions_worst.max(us);
                    stages[stage as usize] += 1;
                    match (&whole, &by_regions) {
                        (Search::Found(w), Search::Found(r)) => {
                            let ratio = f64::from(r.cost / w.cost);
                            ratio_sum += ratio;
                            ratio_max = ratio_max.max(ratio);
                            found += 1;
                            if ratio > 1.10 {
                                worse += 1;
                            }
                        }
                        (Search::NoWay, Search::NoWay) => {}
                        _ => differ += 1,
                    }
                }
                eprintln!(
                    "{project} ({} floors, {} regions, built in {built:.2} s){}: {tried} pairs, {differ} answered otherwise; {found} ways, cost x{:.3} on average, worst x{:.2}, {worse} over x1.10; stages {:?} (no way, corridor, widened, whole); µs a search: whole grid {:.0}, by regions {:.0} (worst {:.0})",
                    grid.len(),
                    regions.len(),
                    if filtered { " with anomalies" } else { "" },
                    ratio_sum / found.max(1) as f64,
                    ratio_max,
                    stages,
                    whole_us / tried as f64,
                    regions_us / tried as f64,
                    regions_worst,
                );
                let _ = Stage::Corridor;
            }
        }
    }
}
