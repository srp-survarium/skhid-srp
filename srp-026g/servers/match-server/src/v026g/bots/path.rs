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

/// A body whose feet are this far above or below the floor a leg passes
/// over where it is along it is not on that leg, however near it is on the
/// ground plane ([`Route::progress`]), metres. GUESS: over a step (0.6 m) and a jump's
/// rise (1.05 m), under a storey (about 2.5 m); in 0x4da3b51d a carrier
/// fell 2.9 m off a walkway its route went on along, 0.7 m off its spot
/// on the ground plane, and pushed into the wall under it for 273 s
/// (`docs/notes/player_bots_stuck.md` §8).
pub const OFF_ROUTE_HEIGHT_M: f32 = 1.5;

/// A body this far under the floor a walked leg passes over where it is
/// along it is not on that leg, metres: a walk goes up and down no more than
/// a step (0.6 m, [`mapdata::grid`]'s `STEP_UP_M`), so a body further under
/// the leg's floor than a step and its slack has fallen off it, into a hole
/// beside it (Mamayev Kurgan's pit at (96.6, 26.9, 136.2): 1.1 m under the
/// route, kept on it by [`OFF_ROUTE_HEIGHT_M`], pushed 4-5 s at a waypoint
/// 1.1 m up it had no link to). A leg that is a jump keeps
/// [`OFF_ROUTE_HEIGHT_M`]: its floor rises by the jump's height along it.
pub const OFF_WALK_BELOW_M: f32 = 0.9;

/// A body above its route's floor by more than [`OFF_ROUTE_HEIGHT_M`] is
/// still on it if the route was that high within this far back along it
/// (it is falling down the route's own drop), metres. GUESS: a body that
/// walks off a 3 m ledge at a run (3.3 m/s) falls for 0.8 s and comes down
/// about 2.6 m on; the bench's falls off the route went up 0.73 +- 0.16 a
/// match when a planned drop was called off the route in the air.
pub const FALL_BACK_M: f32 = 3.0;

/// A body under its route's floor is taken to have fallen off it there
/// ([`Route::fell_from`]) within this far of it across, metres. GUESS: a
/// body that walks off a 3 m ledge comes down about 2.6 m on
/// ([`FALL_BACK_M`]); farther, it came some other way.
pub const FELL_FROM_M: f32 = 3.0;

/// A route's floor heights ([`Route::progress`]) are kept where they change
/// by this much from the last kept (between, on the line), metres: well
/// under [`OFF_ROUTE_HEIGHT_M`], and a ramp of 30 m keeps a few dozen.
const FLOOR_SAMPLE_M: f32 = 0.1;

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
    /// The floors' heights along the route: (distance along it, height),
    /// in order. A straight leg up a ramp and along the walkway on top is
    /// not as high as the line between its ends.
    floors: Vec<(f32, f32)>,
    /// The segment the body was last found on.
    segment: usize,
    /// How far under a walked leg's floor a body is still on it
    /// ([`OFF_WALK_BELOW_M`]; the motor sets it from `Feel::off_walk_below_m`).
    below_walk_m: f32,
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

/// Why a bot with a goal to walk to had no route at a tick (the drive's
/// `drive_tick`), for the tests' counts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoRoute {
    /// A search lately found no way there.
    NoWay,
    /// Its own search goes on over several polls (no way so far yet).
    Searching,
    /// No search left to the poll, or another bot's goes on.
    Starved,
    /// The search it just made found none (or an end is on no floor).
    Searched,
    /// Its route ended this tick: arrived, or off it.
    Ended,
}

#[cfg(test)]
thread_local! {
    static NO_ROUTE: std::cell::Cell<[u64; 5]> = const { std::cell::Cell::new([0; 5]) };
}

/// A tick of a bot with a goal and no route, counted by why.
#[cfg(test)]
pub fn no_route(why: NoRoute) {
    NO_ROUTE.with(|c| {
        let mut n = c.get();
        n[why as usize] += 1;
        c.set(n);
    });
}

#[cfg(not(test))]
pub fn no_route(_: NoRoute) {}

/// The ticks counted by [`no_route`] so far, by [`NoRoute`] in order.
#[cfg(test)]
pub fn no_routes() -> [u64; 5] {
    NO_ROUTE.with(|c| c.get())
}

/// How often at most a bot notes that its searches were dropped (its goal
/// moved on before the search over several polls came to an answer). GUESS:
/// a carrier chasing a moving teammate drops one a second or so; one note in
/// 5 s keeps the log readable and still shows it (`player_bots_stuck.md` §8).
pub const DROPPED_NOTE_MS: u32 = 5_000;

/// A bot's searches dropped since its last note of them.
#[derive(Debug, Default)]
pub struct Dropped {
    count: u32,
    noted_at: Option<u32>,
    due: Option<String>,
}

impl Dropped {
    /// Its search under way to `old` was dropped at `time` for a goal at
    /// `new`: a note is due if none was made in the last
    /// [`DROPPED_NOTE_MS`].
    pub fn dropped(&mut self, time: u32, old: Vec3, new: Vec3) {
        self.count += 1;
        if self.noted_at.is_some_and(|at| time < at + DROPPED_NOTE_MS) {
            return;
        }
        self.due = Some(format!(
            "dropped its route search to {old:?}: the goal moved {:.1} m to {new:?} ({} dropped since the last note)",
            old.distance_xz(new),
            self.count
        ));
        self.noted_at = Some(time);
        self.count = 0;
    }

    /// The note due, once.
    pub fn take(&mut self) -> Option<String> {
        self.due.take()
    }
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

/// How far across a goal's floor may be from it (the drive's search for a
/// goal), as the drive always had it.
pub const GOAL_REACH_M: f32 = 3.0;

/// How far above or below a goal on no floor its nearest floor may be.
/// GUESS: `WalkGrid::locate` looks from 1 m above a point to 2.5 m below it,
/// and a play's goal computed at another body's height (Buddy's, short of a
/// partner up a ramp) can be under the ramp: in 0x5d506a22 bot 14's goal was
/// 1.8-6 m under the floor its partner walked, and it stood 15 s with no
/// route; 6 m found a floor with a way for every second of it
/// (`player_bots_stuck.md` §8).
pub const GOAL_RISE_M: f32 = 6.0;

/// The floor a bot plans to for a goal at `at`: the nearest at its feet
/// ([`WalkGrid::locate`]), else the nearest within [`GOAL_REACH_M`] across
/// and [`GOAL_RISE_M`] above or below (a goal in the air or under a ramp).
pub fn goal_floor(grid: &WalkGrid, at: Vec3) -> Option<NodeId> {
    grid.locate(at, GOAL_REACH_M).or_else(|| {
        if !at.is_finite() {
            return None;
        }
        let r = GOAL_REACH_M;
        grid.floors_in([at.x - r, at.z - r, at.x + r, at.z + r])
            .map(|node| (node, grid.position(node)))
            .filter(|(_, p)| p.distance_xz(at) <= r && (p.y - at.y).abs() <= GOAL_RISE_M)
            .min_by(|a, b| (a.1 - at).length().total_cmp(&(b.1 - at).length()))
            .map(|(node, _)| node)
    })
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

/// The bench's count of the ticks a body was off its route only by the
/// walked-leg rule ([`OFF_WALK_BELOW_M`]): under the old rule it was on.
#[cfg(test)]
pub mod below_walk {
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNT: AtomicU64 = AtomicU64::new(0);

    pub fn count() {
        COUNT.fetch_add(1, Ordering::Relaxed);
    }

    pub fn counts() -> u64 {
        COUNT.load(Ordering::Relaxed)
    }
}

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
    // A line pulled straight crosses no lip, and no floor near a fall or
    // a lip ([`WalkGrid::near_fall`], [`WalkGrid::near_lip`]), that the
    // search kept off ([`WalkGrid::is_lip`]): cutting a corner over a
    // ledge's edge is how a bot strays off it (player_bots_stuck.md §8.5).
    // Half the unplanned falls on a route's leg were off such a cut (the
    // fall-class bench, 2026-10-25). A leg to a drop's take-off goes in
    // across the band, as the drop asks.
    let edge = |n: NodeId| grid.is_lip(n) || grid.near_fall(n) || grid.near_lip(n);
    let mut edges: Vec<NodeId> = nodes.iter().copied().filter(|&n| edge(n)).collect();
    edges.sort_unstable();
    let kept_back = |n: NodeId| allowed(n) && (!edge(n) || edges.binary_search(&n).is_ok());
    // The edge floors a drop's take-off is walked to over (the take-off,
    // and the edge floors before it to it).
    let mut to_drop = vec![false; nodes.len()];
    for k in (0..nodes.len().saturating_sub(1)).rev() {
        to_drop[k] = edge(nodes[k])
            && (into[k + 1] == LinkKind::Drop || (into[k + 1] == LinkKind::Walk && to_drop[k + 1]));
    }
    let mut points = vec![Waypoint {
        at: grid.position(nodes[0]),
        node: nodes[0],
        jump: false,
        low: false,
    }];
    // The floors of the leg from node `from` to node `to`, by distance
    // along the route (`start` at `from`).
    let mut floors = vec![(0.0, grid.position(nodes[0]).y)];
    let mut start = 0.0;
    let mut leg = |from: usize, to: usize, floors: &mut Vec<(f32, f32)>| {
        let (a, b) = (grid.position(nodes[from]), grid.position(nodes[to]));
        let length = a.distance_xz(b);
        let (dx, dz) = if length < 1e-6 {
            (0.0, 0.0)
        } else {
            ((b.x - a.x) / length, (b.z - a.z) / length)
        };
        for k in from + 1..=to {
            let p = grid.position(nodes[k]);
            let along = ((p.x - a.x) * dx + (p.z - a.z) * dz).clamp(0.0, length);
            let last = floors.last().map_or(f32::NAN, |f| f.1);
            if k == to || (p.y - last).abs() >= FLOOR_SAMPLE_M {
                floors.push((start + along, p.y));
            }
        }
        start += length;
    };
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
            leg(i - 1, i, &mut floors);
            continue;
        }
        let mut j = i + 1;
        while j + 1 < nodes.len() && into[j + 1] == LinkKind::Walk && {
            checked();
            if to_drop[j + 1] {
                straight_where(grid, nodes[i], nodes[j + 1], &allowed)
            } else {
                straight_where(grid, nodes[i], nodes[j + 1], kept_back)
            }
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
        leg(from, i, &mut floors);
    }
    Route::with_floors(points, floors)
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
    /// A route by its corners, each leg as high as the line between its
    /// ends.
    pub fn new(points: Vec<Waypoint>) -> Self {
        let mut route = Self::with_floors(points, Vec::new());
        route.floors = route
            .points
            .iter()
            .zip(&route.along)
            .map(|(point, &along)| (along, point.at.y))
            .collect();
        route
    }

    /// This route with a jump from its end onto `to` (on floor `node`): a
    /// way out of a pocket, which no link of the grid makes
    /// (`escape.rs`). The floors along it are kept; the jump's leg is as
    /// high as the line between its ends.
    pub fn then_jump(self, to: Vec3, node: NodeId) -> Self {
        let Self {
            mut points,
            mut floors,
            along,
            ..
        } = self;
        // A planned walk has its start at least (an escape's walk to an
        // exit's floor it stands on is that floor alone).
        debug_assert!(!points.is_empty(), "a jump after an empty route");
        let Some(&last) = points.last() else {
            return Self::new(Vec::new());
        };
        let along = along.last().copied().unwrap_or(0.0);
        points.push(Waypoint {
            at: to,
            node,
            jump: true,
            low: false,
        });
        floors.push((along + last.at.distance_xz(to), to.y));
        Self::with_floors(points, floors)
    }

    /// A route by its corners and the floors' heights along it
    /// ([`Route::floors`]).
    fn with_floors(points: Vec<Waypoint>, floors: Vec<(f32, f32)>) -> Self {
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
            floors,
            segment: 0,
            below_walk_m: OFF_WALK_BELOW_M,
        }
    }

    /// This route's rule for a body under a walked leg ([`OFF_WALK_BELOW_M`]).
    pub fn set_below_walk(&mut self, metres: f32) {
        self.below_walk_m = metres;
    }

    /// The highest floor from `s0` to `s1` metres along the route.
    fn highest_floor(&self, s0: f32, s1: f32) -> f32 {
        let k0 = self.floors.partition_point(|&(along, _)| along <= s0);
        let k1 = self.floors.partition_point(|&(along, _)| along < s1);
        self.floors[k0.min(k1)..k1]
            .iter()
            .map(|&(_, y)| y)
            .fold(self.floor_at(s0).max(self.floor_at(s1)), f32::max)
    }

    /// The point `s` metres along the route on its floor.
    pub fn floor_point(&self, s: f32) -> Vec3 {
        let at = self.point_at(s);
        Vec3::new(at.x, self.floor_at(s), at.z)
    }

    /// Where a body at `feet` fell off the route from, if it did: the
    /// route's floor over it on the segment the body was last on or the
    /// next, more than [`OFF_ROUTE_HEIGHT_M`] over the feet and within
    /// [`FELL_FROM_M`] of them across (the nearest such).
    pub fn fell_from(&self, feet: Vec3) -> Option<Vec3> {
        let last = self.points.len().checked_sub(2)?;
        (self.segment..=(self.segment + 1).min(last))
            .filter_map(|segment| {
                let (a, b) = (self.points[segment].at, self.points[segment + 1].at);
                let (dx, dz) = (b.x - a.x, b.z - a.z);
                let length2 = dx * dx + dz * dz;
                let t = if length2 < 1e-6 {
                    0.0
                } else {
                    (((feet.x - a.x) * dx + (feet.z - a.z) * dz) / length2).clamp(0.0, 1.0)
                };
                let at = self.floor_point(self.along[segment] + t * length2.sqrt());
                (at.y - feet.y > self.below_m(segment) && at.distance_xz(feet) <= FELL_FROM_M)
                    .then_some(at)
            })
            .min_by(|p, q| p.distance_xz(feet).total_cmp(&q.distance_xz(feet)))
    }

    /// The floor's height `s` metres along the route (on the line between
    /// the heights kept).
    fn floor_at(&self, s: f32) -> f32 {
        let k = self.floors.partition_point(|&(along, _)| along <= s);
        match (self.floors.get(k.wrapping_sub(1)), self.floors.get(k)) {
            (Some(&(a, ya)), Some(&(b, yb))) if b - a > 1e-6 => ya + (yb - ya) * (s - a) / (b - a),
            (Some(&(_, y)), _) | (None, Some(&(_, y))) => y,
            (None, None) => 0.0,
        }
    }

    /// How far under the floor of leg `segment` a body is still on it:
    /// [`Self::below_walk_m`] on a walked leg, [`OFF_ROUTE_HEIGHT_M`] on a jump.
    fn below_m(&self, segment: usize) -> f32 {
        if self.points[segment + 1].jump {
            OFF_ROUTE_HEIGHT_M
        } else {
            self.below_walk_m
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
    /// that passes the same place twice is followed in order. A segment
    /// whose floor where the body is along it is more than
    /// [`OFF_ROUTE_HEIGHT_M`] above the feet is not where the body is (a
    /// walkway over the floor it fell to), nor one that far under the feet
    /// and under every floor of the route [`FALL_BACK_M`] back (a body
    /// falling down the route's own drop is on it); with none left it is
    /// off the route (infinitely far).
    pub fn progress(&mut self, feet: Vec3) -> (f32, f32) {
        if self.points.len() < 2 {
            return (0.0, self.end().distance_xz(feet));
        }
        let last = self.points.len() - 2;
        let mut best = (f32::INFINITY, self.segment, 0.0);
        #[cfg(test)]
        let mut only_new_rule = false;
        for segment in self.segment..=(self.segment + 3).min(last) {
            let (a, b) = (self.points[segment].at, self.points[segment + 1].at);
            let (dx, dz) = (b.x - a.x, b.z - a.z);
            let length2 = dx * dx + dz * dz;
            let t = if length2 < 1e-6 {
                0.0
            } else {
                (((feet.x - a.x) * dx + (feet.z - a.z) * dz) / length2).clamp(0.0, 1.0)
            };
            let s = self.along[segment] + t * length2.sqrt();
            if feet.y < self.floor_at(s) - self.below_m(segment)
                || feet.y > self.highest_floor(s - FALL_BACK_M, s) + OFF_ROUTE_HEIGHT_M
            {
                #[cfg(test)]
                if feet.y >= self.floor_at(s) - OFF_ROUTE_HEIGHT_M
                    && feet.y <= self.highest_floor(s - FALL_BACK_M, s) + OFF_ROUTE_HEIGHT_M
                {
                    only_new_rule = true;
                }
                continue;
            }
            let (px, pz) = (a.x + dx * t, a.z + dz * t);
            let off = ((feet.x - px).powi(2) + (feet.z - pz).powi(2)).sqrt();
            if off < best.0 - 1e-3 {
                best = (off, segment, t * length2.sqrt());
            }
        }
        #[cfg(test)]
        if best.0.is_infinite() && only_new_rule {
            below_walk::count();
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

    /// The body is looked for from segment `segment` again (never later than
    /// it was): a world showed it still under the jump that segment leads
    /// up, which the reckoning had it up ([`Route::progress`] never goes
    /// back by itself).
    pub fn back_to(&mut self, segment: usize) {
        self.segment = self.segment.min(segment);
    }

    /// The segment the body was last found on ([`Route::progress`]).
    pub fn segment(&self) -> usize {
        self.segment
    }

    /// The next waypoint after `s` that is reached by a jump: its index and
    /// the distance to the waypoint before it (where the jump starts).
    pub fn next_jump(&self, s: f32) -> Option<(usize, f32)> {
        let first = self.along.partition_point(|&a| a <= s).max(1);
        (first..self.points.len())
            .find(|&k| self.points[k].jump)
            .map(|k| (k, self.along[k - 1] - s))
    }

    /// The landings of the drops of a lip's height or more the route makes from `s` to `within` metres on (a
    /// planned fall: the body walks off the edge there).
    pub fn drop_landings(&self, s: f32, within: f32) -> Vec<Vec3> {
        let first = self.along.partition_point(|&a| a <= s).max(1);
        (first..self.points.len())
            .take_while(|&k| self.along[k - 1] <= s + within)
            .filter(|&k| !self.points[k].jump && self.points[k - 1].at.y - self.points[k].at.y >= mapdata::grid::LIP_DROP_M)
            .map(|k| self.points[k].at)
            .collect()
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

    /// The landings of the route's drops of a metre or more within reach, and no step, jump or short drop.
    #[test]
    fn the_landings_of_a_routes_drops_are_found_within_reach() {
        let p = |x, y, jump| Waypoint {
            at: Vec3::new(x, y, 0.0),
            node: 0,
            jump,
            low: false,
        };
        // Along, down 0.5 m (a curb), along, down 2 m (a drop), along, up 1.2 m by a jump, along.
        let route = Route::new(vec![
            p(0.0, 3.0, false),
            p(4.0, 3.0, false),
            p(4.5, 2.5, false),
            p(8.0, 2.5, false),
            p(9.0, 0.5, false),
            p(14.0, 0.5, false),
            p(15.0, 1.7, true),
        ]);
        assert_eq!(route.drop_landings(0.0, 20.0), vec![Vec3::new(9.0, 0.5, 0.0)]);
        // The drop's leg starts about 8 m along: out of reach from the start, in reach from 3 m.
        assert!(route.drop_landings(0.0, 6.0).is_empty());
        assert_eq!(route.drop_landings(3.0, 6.0).len(), 1);
        // On the drop's leg, and past it, none behind it.
        assert_eq!(route.drop_landings(8.5, 1.0).len(), 1);
        assert!(route.drop_landings(10.0, 20.0).is_empty());
    }

    /// A body that fell off a walkway its route goes on along is off the
    /// route, though the walkway is right over it on the ground plane; one
    /// a step under it or a jump's rise over it is on it.
    #[test]
    fn a_body_on_the_floor_under_its_route_is_off_it() {
        let p = |x, y, z| Waypoint {
            at: Vec3::new(x, y, z),
            node: 0,
            jump: false,
            low: false,
        };
        // Up a ramp from x 20 to x 12, then along a walkway 3 m up to x 0.
        let route = Route::new(vec![p(20.0, 0.0, 0.0), p(12.0, 3.0, 0.0), p(0.0, 3.0, 0.0)]);
        let fell = Vec3::new(8.0, 0.0, 0.7);
        let mut on_walkway = route.clone();
        on_walkway.progress(Vec3::new(10.0, 3.0, 0.0));
        let (_, off) = on_walkway.clone().progress(fell);
        assert!(off.is_infinite(), "{off}");
        for y in [2.4, 3.0, 4.4] {
            let (s, off) = on_walkway.clone().progress(Vec3::new(8.0, y, 0.7));
            assert!(
                (s - 12.0).abs() < 1e-3 && (off - 0.7).abs() < 1e-3,
                "{y}: {s} {off}"
            );
        }
        // 1.4 m under the walked walkway is a fall off it, though a jump's rise is less.
        let (_, off) = on_walkway.clone().progress(Vec3::new(8.0, 1.6, 0.7));
        assert!(off.is_infinite(), "{off}");
        // On the ramp's foot the ramp is the leg it is on.
        let (s, off) = route.clone().progress(Vec3::new(18.0, 0.2, 0.5));
        assert!(
            (s - 2.0).abs() < 1e-3 && (off - 0.5).abs() < 1e-3,
            "{s} {off}"
        );
    }

    /// A body falling down its route's own drop is on the route while in
    /// the air past the drop's foot; one as high over the route further on
    /// (on a walkway over it) is off it.
    #[test]
    fn a_body_falling_down_its_routes_drop_is_on_it() {
        let p = |x, y, z| Waypoint {
            at: Vec3::new(x, y, z),
            node: 0,
            jump: false,
            low: false,
        };
        // Along a walkway 3 m up to x 10, down to the ground at x 10.5, on
        // to x 20.
        let route = Route::new(vec![
            p(0.0, 3.0, 0.0),
            p(10.0, 3.0, 0.0),
            p(10.5, 0.0, 0.0),
            p(20.0, 0.0, 0.0),
        ]);
        let mut falling = route.clone();
        falling.progress(Vec3::new(9.0, 3.0, 0.0));
        for (x, y) in [(10.5, 2.8), (11.5, 2.0), (12.5, 0.5)] {
            let (s, off) = falling.clone().progress(Vec3::new(x, y, 0.2));
            assert!(
                (s - x).abs() < 1e-3 && (off - 0.2).abs() < 1e-3,
                "{x} {y}: {s} {off}"
            );
        }
        let mut further = route.clone();
        further.progress(Vec3::new(15.0, 0.0, 0.0));
        let (_, off) = further.progress(Vec3::new(16.0, 3.0, 0.2));
        assert!(off.is_infinite(), "{off}");
        // Fallen off the walkway at x 6: from there; on the ground past the
        // drop, or under the walkway far across, it fell from nowhere.
        let mut walking = route.clone();
        walking.progress(Vec3::new(5.0, 3.0, 0.0));
        let from = walking.fell_from(Vec3::new(6.0, 0.0, 1.0)).unwrap();
        assert!(
            (from - Vec3::new(6.0, 3.0, 0.0)).length() < 1e-3,
            "{from:?}"
        );
        assert_eq!(walking.fell_from(Vec3::new(6.0, 0.0, 4.0)), None);
        assert_eq!(further.fell_from(Vec3::new(16.0, 0.0, 1.0)), None);
    }

    /// A body 1.1 m under a walked leg's floor (fallen into a hole beside
    /// it) is off the route at once; the same under a jump leg is still on
    /// it (the jump's floor rises along it), and one a step under a walked
    /// leg (on the lower of two stairs) is on it.
    #[test]
    fn a_body_a_step_and_more_under_a_walked_leg_is_off_it() {
        let p = |x, y, z, jump| Waypoint {
            at: Vec3::new(x, y, z),
            node: 0,
            jump,
            low: false,
        };
        let walked = Route::new(vec![p(0.0, 1.1, 0.0, false), p(10.0, 1.1, 0.0, false)]);
        let (_, off) = walked.clone().progress(Vec3::new(5.0, 0.0, 0.3));
        assert!(off.is_infinite(), "1.1 m under a walked leg: {off}");
        let (_, off) = walked.clone().progress(Vec3::new(5.0, 0.4, 0.3));
        assert!((off - 0.3).abs() < 1e-3, "0.7 m under: {off}");
        assert!(walked.clone().fell_from(Vec3::new(5.0, 0.0, 0.3)).is_some());
        let jumped = Route::new(vec![p(0.0, 0.0, 0.0, false), p(1.0, 1.05, 0.0, true)]);
        let (_, off) = jumped.clone().progress(Vec3::new(0.9, 0.0, 0.0));
        assert!(off.abs() < 1e-3, "short of a jump's landing: {off}");
    }

    /// A pulled leg keeps the heights of the floors it passes over, not
    /// only of its ends: a leg over a rise is not off for a body on top.
    #[test]
    fn a_pulled_leg_has_the_heights_of_its_floors() {
        use crate::v026g::bots::body::tests::floor;
        // A flat floor with a gentle mound (0.4 m over 4 m each way).
        let mut triangles = floor(0.0, 0.0, 30.0, 10.0, 0.0).to_vec();
        let walker = mapdata::WalkerBehaviour::BySlope;
        let tri = |a: Vec3, b: Vec3, c: Vec3| mapdata::Triangle {
            v: [a, b, c],
            walker,
        };
        for (x0, x1, y0, y1) in [(11.0, 15.0, 0.01, 1.8), (15.0, 19.0, 1.8, 0.01)] {
            let (a, b) = (Vec3::new(x0, y0, 0.0), Vec3::new(x1, y1, 0.0));
            let (c, d) = (Vec3::new(x1, y1, 10.0), Vec3::new(x0, y0, 10.0));
            triangles.push(tri(a, b, c));
            triangles.push(tri(a, c, d));
        }
        let grid = WalkGrid::build(
            &triangles,
            [0.0, 0.0, 30.0, 10.0],
            0.25,
            mapdata::Body::default(),
        );
        let from = grid.locate(Vec3::new(5.0, 0.0, 5.0), 1.0).unwrap();
        let to = grid.locate(Vec3::new(25.0, 0.0, 5.0), 1.0).unwrap();
        let mut route = plan(&grid, from, to).unwrap();
        let top = grid.locate(Vec3::new(15.0, 1.8, 5.0), 1.0).unwrap();
        let (s, off) = route.progress(grid.position(top));
        assert!(
            off < 1.0 && (8.0..12.0).contains(&s),
            "{s} {off}: {:?}",
            route.points()
        );
    }

    /// A goal under a floor or in the air over one (as a play computes it
    /// at another body's height) still has a floor to plan to, within
    /// [`GOAL_RISE_M`]; one far over any floor has none.
    #[test]
    fn a_goal_off_its_floor_plans_to_the_nearest_floor() {
        use crate::v026g::bots::body::tests::floor;
        let triangles = floor(0.0, 0.0, 10.0, 10.0, 3.0).to_vec();
        let grid = WalkGrid::build(
            &triangles,
            [0.0, 0.0, 10.0, 10.0],
            0.25,
            mapdata::Body::default(),
        );
        // At the feet: as `locate` has it.
        let on = Vec3::new(5.0, 3.0, 5.0);
        assert_eq!(goal_floor(&grid, on), grid.locate(on, GOAL_REACH_M));
        // 2 m under the floor and 5 m over it: `locate` finds none.
        for y in [1.0, 8.0] {
            let at = Vec3::new(5.0, y, 5.0);
            assert_eq!(grid.locate(at, GOAL_REACH_M), None);
            let node = goal_floor(&grid, at).unwrap();
            let p = grid.position(node);
            assert!((p.y - 3.0).abs() < 0.1 && p.distance_xz(at) < 0.3, "{p:?}");
        }
        // 2 m beyond the floor's edge, under it: the edge.
        let p = grid.position(goal_floor(&grid, Vec3::new(12.0, 1.0, 5.0)).unwrap());
        assert!(p.x > 9.0 && p.distance_xz(Vec3::new(12.0, 1.0, 5.0)) <= GOAL_REACH_M);
        // Too far over it.
        assert_eq!(
            goal_floor(&grid, Vec3::new(5.0, 3.0 + GOAL_RISE_M + 1.0, 5.0)),
            None
        );
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
