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
/// A door gate ([`Route::gate_within`]) is reached when the body is within this many metres of it along the route.
pub const GATE_REACHED_M: f32 = 0.25;

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
/// The most a body's progress along its route moves between two ticks and still is a walk over a drop's edge
/// ([`Route::left_drop_edge`]), metres: 0.2 m a tick at a sprint, with room for a jump in the progress.
const DROP_EDGE_STEP_M: f32 = 1.0;

/// How a route's jump is pressed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Leap {
    /// Up a jump link: the motor presses as it walks over the take-off.
    Ledge,
    /// Up a jump link above the grid body's standing jump (`Body::max_jump`): a sprint jump, run up to at a sprint. The
    /// planner labels each jump link once, from the grid it plans on; the motor reads the label.
    SprintLedge,
    /// Across a gap link (`mapdata::grid::gap`): pressed this far before the
    /// take-off, along the way to the landing, where the grid found the
    /// client's jump comes down on it.
    Gap(mapdata::PressLead),
    /// Across a gap link at a sprint (`mapdata::grid::gap::Takeoff::Sprint`):
    /// run into at a sprint, facing the landing, pressed this far before the
    /// take-off. The sprint itself is the motor's sprint-jump line-up (A55).
    ///
    /// The motor (`Feel::sprint_gaps`; off, a walking gap jump): the run-up (the grid guarantees 1.5 m of floor behind or
    /// beside the press) may turn onto the jump's way, so it sprints along the route and turns onto it over the last
    /// [`motor::SPRINT_GAP_TURN_M`].
    SprintGap(mapdata::PressLead),
}

/// A corner of a route.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Waypoint {
    pub at: Vec3,
    pub node: NodeId,
    /// Reached by a jump from the waypoint before, and how it is pressed.
    pub jump: Option<Leap>,
    /// The leg to it from the waypoint before goes over a floor a standing
    /// body does not fit under (`WalkGrid::can_stand`): walked crouched.
    pub low: bool,
}

/// The height a point of a route is given ([`Route::at`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RouteHeight {
    /// On the line between the legs' ends ([`Route::point_at`]): over a hump of the floor (a deck the leg crosses
    /// between two lower ends) it is up to the hump's height under the floor.
    Chord,
    /// On the floor the route walks ([`Route::floor_point`]).
    Floor,
}

mod wall_clear;
pub use wall_clear::counts as wall_clear_counts;

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
    /// Which floors behind a body count for where it is ([`PastDrop`]; the motor sets it from `Feel::past_drop`).
    past_drop: PastDrop,
    /// The landings of the route's drops whose flight at a sprint ([`SPRINT_FLIGHT_MPS`]) comes down within the
    /// harmless fall ([`WalkGrid::drop_harmless_at`]); the planner sets them. None known (a route made some other
    /// way) means every drop is taken to be unsafe at a sprint.
    sprint_drops: Vec<(Vec3, bool)>,
    /// The landings of the route's walking jumps up a ledge with a short top, and how far before the take-off each is pressed
    /// ([`WalkGrid::press_lead`]); the planner sets them. A landing not in it is pressed as the motor always has.
    ledge_leads: Vec<(Vec3, f32)>,
    /// The landings of the route's jump links, and how deep the top is past the face along the jump
    /// ([`top_depth`]); the planner sets them. A landing not in it has no known top (a lift is not tried up it).
    ledge_tops: Vec<(Vec3, f32)>,
    /// The middles of the doorways the route crosses ([`Route::door_centred`]), each a corner of it: the walk steers at the
    /// first one it has not reached however near the corner after it is ([`Route::gate_within`]).
    gates: Vec<Vec3>,
    /// Made by the planner from a path of floors ([`pull_straight`]), so `sprint_drops` holds every Drop link of it.
    planned: bool,
}

/// What the planner knows of the drops of a route within a stretch, as to a sprint ([`Route::drop_record`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DropRecord {
    /// No drop in the stretch.
    None,
    /// Every drop in it is harmless at a sprint.
    Safe,
    /// One is not.
    Unsafe,
    /// One has no record (the route was not made by the planner, or was made from another).
    Unknown,
    /// None is a Drop link: the planner's route goes down by walk links only (a ramp, stairs), however far down its
    /// corners are.
    Walk,
}

/// What a route's drop is: the one rule every reader of the route's drops asks ([`Route::drop_landings`],
/// [`Route::drop_record`], [`Route::next_drop_edge_along`], [`Route::next_drop`], [`Route::left_drop_edge`]; the lip
/// guard's planned landings, the drop aim, the landing watch, the drop walk).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DropTruth {
    /// A waypoint a lip's height ([`mapdata::grid::LIP_DROP_M`]) or more under the one before it, reached by no jump,
    /// whatever link leads to it: a stair or a ramp whose far corner is a metre lower counts, a Drop link of 0.8-1.0 m
    /// does not.
    Height,
    /// A Drop link of the planner's path ([`Route::sprint_drops`] holds every one, from the client's step height on);
    /// a route made some other way has no links to read, and keeps the waypoints of the height.
    #[default]
    Links,
}

impl DropTruth {
    /// How far a body is above a drop's landing, metres, while it is still to go over the edge.
    pub fn above_m(self) -> f32 {
        match self {
            Self::Height => mapdata::grid::LIP_DROP_M,
            Self::Links => DROP_LINK_MIN_M,
        }
    }
}

/// The shallowest Drop link, metres: the client's step height (`Body::client().step`, 0.8), from which a grid link is
/// a drop and not a step.
pub const DROP_LINK_MIN_M: f32 = 0.8;

/// Which of the route's floors behind a body count for where it may be along the route ([`Route::progress`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PastDrop {
    /// Every floor [`FALL_BACK_M`] back, the top of a drop as well: a body on a drop's top that is near its landing on the ground
    /// plane is on the legs after it. Mamayev Kurgan's walk-off at (105.4, 29.9, 128.9), 0.7 m across and 3.1 m down: a body
    /// at the edge was put on the leg past the landing, and once braked back it steered at that leg's end, 3.8 m on and 3.2 m
    /// down, into the wall 0.9 m ahead (PROVEN, the route code replayed on the map).
    #[default]
    Window,
    /// The floors back to the landing of the last drop behind the body only: a body is on a leg past a drop once it is down.
    /// A falling body is on the drop's own leg, whose floors run from the top down.
    Landed,
}

/// The speed a body leaves a drop's edge at when it sprints, m/s: a sprint is 5.0-5.1 m/s in the client (PROVEN: the
/// J0 and item 10 rows, 5.08 m/s in the air with the key held).
pub const SPRINT_FLIGHT_MPS: f32 = 5.1;

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
    /// Given up in its pocket (`think.shut_in_gate`): no search is asked for a goal outside
    /// it, and `NoWay` stays the count of failed searches.
    ShutIn,
}

#[cfg(test)]
thread_local! {
    static NO_ROUTE: std::cell::Cell<[u64; 6]> = const { std::cell::Cell::new([0; 6]) };
    static NO_ROUTE_BY_TEAM: std::cell::Cell<[[u64; 6]; 2]> = const { std::cell::Cell::new([[0; 6]; 2]) };
}

/// A tick of a bot with a goal and no route, counted by why.
#[cfg(test)]
pub fn no_route(team: u8, why: NoRoute) {
    NO_ROUTE.with(|c| {
        let mut n = c.get();
        n[why as usize] += 1;
        c.set(n);
    });
    NO_ROUTE_BY_TEAM.with(|c| {
        let mut n = c.get();
        n[usize::from(team & 1)][why as usize] += 1;
        c.set(n);
    });
}

#[cfg(not(test))]
pub fn no_route(_: u8, _: NoRoute) {}

/// The ticks counted by [`no_route`] so far, by team and [`NoRoute`] in order.
#[cfg(test)]
pub fn no_routes_by_team() -> [[u64; 6]; 2] {
    NO_ROUTE_BY_TEAM.with(|c| c.get())
}

/// The ticks counted by [`no_route`] so far, by [`NoRoute`] in order.
#[cfg(test)]
pub fn no_routes() -> [u64; 6] {
    NO_ROUTE.with(|c| c.get())
}

/// How far apart in height a search's start floor and goal floor are, for
/// the tests' counts ([`request_gap`]): a storey or more
/// (`plays::memo::STOREY_GAP_M`) above or below, or one level.
#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gap {
    Same,
    Above,
    Below,
}

#[cfg(test)]
impl Gap {
    /// The gap from a start floor to a goal floor `dy` metres above it.
    pub fn of(dy: f32) -> Gap {
        if dy >= super::plays::memo::STOREY_GAP_M {
            Gap::Above
        } else if dy <= -super::plays::memo::STOREY_GAP_M {
            Gap::Below
        } else {
            Gap::Same
        }
    }
}

#[cfg(test)]
thread_local! {
    static REQUEST_GAP: std::cell::Cell<[u64; 6]> = const { std::cell::Cell::new([0; 6]) };
}

/// A route request that ended in its own poll (a search made and finished
/// there, with both ends on floors and the bot not escaping), counted by
/// whether it found a route and by the [`Gap`] between its floors.
#[cfg(test)]
pub fn request_gap(found: bool, gap: Gap) {
    REQUEST_GAP.with(|c| {
        let mut n = c.get();
        n[usize::from(!found) * 3 + gap as usize] += 1;
        c.set(n);
    });
}

/// The requests counted by [`request_gap`] so far: found (same level,
/// above, below), then failed (same, above, below).
#[cfg(test)]
pub fn request_gaps() -> [u64; 6] {
    REQUEST_GAP.with(|c| c.get())
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

/// How far a rest point is moved at most to leave a lip, metres (walked over the grid's links from its floor).
pub const REST_SHIFT_M: f32 = 1.5;

/// A rest point off the edge (the owner's rule: a body at rest keeps its centre on the floor, 2026-10-26): `at` moved to
/// the nearest floor that is neither a lip (the last floor before a ledge, its centre within a cell of the edge,
/// [`WalkGrid::is_lip`]) nor near one ([`WalkGrid::near_lip`]: within the body's radius and a margin of it), walking the
/// grid's links up to [`REST_SHIFT_M`]. None when `at` is not on such a floor, or there is none near.
pub fn off_lip(grid: &WalkGrid, at: Vec3) -> Option<Vec3> {
    let start = grid.locate(at, 1.0)?;
    let bad = |node: NodeId| grid.is_lip(node) || grid.near_lip(node);
    if !bad(start) {
        return None;
    }
    let mut seen = std::collections::HashSet::from([start]);
    let mut frontier = vec![start];
    let mut best: Option<(f32, NodeId)> = None;
    while !frontier.is_empty() {
        let mut next = Vec::new();
        for node in frontier {
            for (to, kind) in grid.links(node) {
                if kind != LinkKind::Walk || !seen.insert(to) {
                    continue;
                }
                let p = grid.position(to);
                if p.distance_xz(at) > REST_SHIFT_M {
                    continue;
                }
                if !bad(to) {
                    let d = p.distance_xz(at);
                    if best.is_none_or(|(b, _)| d < b) {
                        best = Some((d, to));
                    }
                } else {
                    next.push(to);
                }
            }
        }
        frontier = next;
    }
    best.map(|(_, node)| grid.position(node))
}

/// The rest points moved off a lip ([`off_lip`]) so far, for the bench's count.
pub mod rests {
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNT: AtomicU64 = AtomicU64::new(0);

    pub fn count() {
        COUNT.fetch_add(1, Ordering::Relaxed);
    }

    pub fn counts() -> u64 {
        COUNT.load(Ordering::Relaxed)
    }
}

/// How far below a body's feet a floor may be for it to plan from there when it stands on no floor
/// ([`stranded_floor`]), metres: the client's `harmless_height` (`docs/notes/fall_damage.md`, PROVEN: no damage up to
/// 5 m; the grid's own `Body::max_drop` is the same), so the drop off the top never hurts.
pub const STRANDED_DROP_M: f32 = 5.0;

/// The floor a body plans from when it stands on none ([`WalkGrid::locate`] finds the floors up to 2.5 m
/// under the feet): the nearest floor within [`GOAL_REACH_M`] across and [`STRANDED_DROP_M`] below, that is
/// where it lands when it walks off what it stands on (the top of a crate or a ledge the grid has no cells
/// on). None when there is none, or one only higher up.
pub fn stranded_floor(grid: &WalkGrid, feet: Vec3) -> Option<NodeId> {
    if !feet.is_finite() {
        return None;
    }
    let r = GOAL_REACH_M;
    grid.floors_in([feet.x - r, feet.z - r, feet.x + r, feet.z + r])
        .map(|node| (node, grid.position(node)))
        .filter(|(_, p)| p.distance_xz(feet) <= r && (0.0..=STRANDED_DROP_M).contains(&(feet.y - p.y)))
        .min_by(|a, b| a.1.distance_xz(feet).total_cmp(&b.1.distance_xz(feet)))
        .map(|(node, _)| node)
}

/// The bots' plans from a stand on no floor ([`stranded_floor`]), for the bench's count.
pub mod stranded {
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNT: AtomicU64 = AtomicU64::new(0);

    pub fn count() {
        COUNT.fetch_add(1, Ordering::Relaxed);
    }

    pub fn counts() -> u64 {
        COUNT.load(Ordering::Relaxed)
    }
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
    match plan_start(grid, regions, from, to, allowed, region_ok, None, None, &mut iters) {
        Ok(route) => route,
        Err(_) => unreachable!("an unbounded search ends"),
    }
}

/// A [`plan_in`] under way ([`plan_start`]), as Detour's sliced path
/// queries: it goes on in [`Planning::resume`].
pub struct Planning {
    search: RouteSearch,
    /// Its floors are weighed (a cost search, `RouteFilter::weight`): its
    /// route is pulled straight over floors of weight 1 only ([`finish`]).
    /// Set once, at its start: [`Self::resume`] reads the weight it is given
    /// only then.
    weighted: bool,
    #[cfg(test)]
    us: f64,
    #[cfg(test)]
    slices: u64,
}

/// [`plan_in`] for a body whose jumps rise at most `max_rise` metres
/// (`None`: the grid's own), for at most `iters` floors expanded (taken off it): the
/// route, or none, once the search ends within them; else the search,
/// to go on with the same `allowed`, `region_ok` and `weight`. A search
/// that ends within them gives what [`plan_in`] gives. `weight`: a cost on
/// each floor (at least 1, [`mapdata::SlicedSearch::step`]); none, the
/// plain search.
// The Err is the search under way, which the bot keeps: not an error to box.
#[allow(clippy::result_large_err, clippy::too_many_arguments)]
pub fn plan_start(
    grid: &WalkGrid,
    regions: &Regions,
    from: NodeId,
    to: NodeId,
    allowed: impl Fn(NodeId) -> bool,
    region_ok: impl Fn(u32) -> bool,
    max_rise: Option<f32>,
    weight: Option<&dyn Fn(NodeId) -> f32>,
    iters: &mut usize,
) -> Result<Option<Route>, Planning> {
    #[cfg(test)]
    let started = std::time::Instant::now();
    match regions.start_route_search_reaching(grid, from, to, &allowed, &region_ok, usize::MAX, max_rise) {
        Err((found, _stage)) => {
            #[cfg(test)]
            count(&found, _stage, started.elapsed().as_secs_f64() * 1e6, 0, 1);
            Ok(finish(grid, found, allowed, weight))
        }
        Ok(search) => {
            let mut planning = Planning {
                search,
                weighted: weight.is_some(),
                #[cfg(test)]
                us: started.elapsed().as_secs_f64() * 1e6,
                #[cfg(test)]
                slices: 0,
            };
            match planning.resume(grid, regions, allowed, region_ok, weight.unwrap_or(&|_| 1.0), iters) {
                Some(route) => Ok(route),
                None => Err(planning),
            }
        }
    }
}

impl Planning {
    /// Goes on for at most `iters` floors expanded (taken off it): the
    /// route, or none, once the search ends; `None` while it goes on.
    /// `weight`: the filter's (`RouteFilter::weight`), read only by a search
    /// that started weighed, so a plain search stays plain whatever it is.
    pub fn resume(
        &mut self,
        grid: &WalkGrid,
        regions: &Regions,
        allowed: impl Fn(NodeId) -> bool,
        region_ok: impl Fn(u32) -> bool,
        weight: &dyn Fn(NodeId) -> f32,
        iters: &mut usize,
    ) -> Option<Option<Route>> {
        let weight = self.weighted.then_some(weight);
        #[cfg(test)]
        let started = std::time::Instant::now();
        // The plain search keeps its own loop (no call through the weight).
        let ended = match weight {
            Some(w) => self.search.step(regions, grid, &allowed, region_ok, w, iters),
            None => self.search.step(regions, grid, &allowed, region_ok, |_| 1.0, iters),
        };
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
        Some(finish(grid, found, allowed, weight))
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

/// The route of a search that ended. A weighed search's path is pulled
/// straight only across floors of weight 1 or its own: a line pulled across
/// a costly floor it went round would undo the way round.
fn finish(
    grid: &WalkGrid,
    found: Search,
    allowed: impl Fn(NodeId) -> bool,
    weight: Option<&dyn Fn(NodeId) -> f32>,
) -> Option<Route> {
    match (found, weight) {
        (Search::Found(path), None) => Some(pull(grid, path.nodes, allowed)),
        (Search::Found(path), Some(w)) => {
            let own: std::collections::HashSet<NodeId> = path.nodes.iter().copied().collect();
            Some(pull(grid, path.nodes, |n| allowed(n) && (w(n) <= 1.0 || own.contains(&n))))
        }
        (Search::NoWay | Search::TooFar, _) => None,
    }
}

/// A search ended, into the thread's counts.
#[cfg(test)]
fn count(found: &Search, stage: Stage, us: f64, expanded: u64, slices: u64) {
    SEARCHES.with(|s| {
        let mut c = s.get();
        c.searches += 1;
        c.failed += u64::from(!matches!(found, Search::Found(_)));
        tag::ended(!matches!(found, Search::Found(_)), expanded);
        c.us += us;
        c.stages[stage as usize] += 1;
        c.stage_us[stage as usize] += us;
        c.worst_us = c.worst_us.max(us);
        c.expanded += expanded;
        super::bench_work::add(super::bench_work::Work::Floors, expanded);
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

/// The bench's grid searches by why they ran (the threat answer's work,
/// `tests/tallies/owner/threat_answer.rs`): a thread's tag, set by the
/// drive around a bot's search and by the poll around its resumed one;
/// each ended search adds its count, failure and floors to its tag's.
#[cfg(test)]
pub mod tag {
    use std::cell::Cell;

    /// The tags: another search; K1's whole-route search; an A3 replan's
    /// search for the cover alone; a K2 job play's search.
    pub const OTHER: u8 = 0;
    pub const K1: u8 = 1;
    pub const REPLAN: u8 = 2;
    pub const JOB: u8 = 3;
    pub const NAMES: [&str; 4] = ["other", "k1", "replan", "job"];

    thread_local! {
        static TAG: Cell<u8> = const { Cell::new(OTHER) };
        static BY_TAG: Cell<[[u64; 3]; 4]> = const { Cell::new([[0; 3]; 4]) };
    }

    /// The tag from now until it drops (the one before then again).
    pub struct Scope(u8);

    impl Scope {
        pub fn new(tag: u8) -> Self {
            Self(TAG.with(|t| t.replace(tag)))
        }
    }

    impl Drop for Scope {
        fn drop(&mut self) {
            TAG.with(|t| t.set(self.0));
        }
    }

    /// An ended search: failed or not, its floors.
    pub(super) fn ended(failed: bool, expanded: u64) {
        let tag = usize::from(TAG.with(Cell::get));
        BY_TAG.with(|c| {
            let mut all = c.get();
            all[tag][0] += 1;
            all[tag][1] += u64::from(failed);
            all[tag][2] += expanded;
            c.set(all);
        });
    }

    /// Searches, failed, floors by tag so far.
    pub fn counted() -> [[u64; 3]; 4] {
        BY_TAG.with(Cell::get)
    }
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

/// The bench's count of the legs [`PastDrop::Landed`] kept a body off (the window back to the last drop's landing): with
/// [`PastDrop::Window`] it would have been on them, past a drop it had not gone down.
#[cfg(test)]
pub mod past_drop {
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
        // A gap link's take-off is at an edge too.
        let leaves_edge = matches!(into[k + 1], LinkKind::Drop | LinkKind::GapDrop | LinkKind::GapJump);
        to_drop[k] = edge(nodes[k]) && (leaves_edge || (into[k + 1] == LinkKind::Walk && to_drop[k + 1]));
    }
    let mut points = vec![Waypoint {
        at: grid.position(nodes[0]),
        node: nodes[0],
        jump: None,
        low: false,
    }];
    let mut sprint_drops = Vec::new();
    let mut ledge_leads = Vec::new();
    let mut ledge_tops = Vec::new();
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
                jump: match into[i] {
                    LinkKind::Jump if grid.position(nodes[i]).y - grid.position(nodes[i - 1]).y > grid.body().max_jump => {
                        Some(Leap::SprintLedge)
                    }
                    LinkKind::Jump => Some(Leap::Ledge),
                    // A gap link (`mapdata::grid::gap`): at its own press.
                    // A gap link: pressed at its own point, or walked off
                    // with no key (a plain leg the body falls along).
                    kind if kind.is_off_mesh() => grid.off_link(nodes[i - 1], nodes[i]).and_then(|l| match l.press {
                        mapdata::grid::gap::Takeoff::Press(press) => Some(Leap::Gap(press)),
                        mapdata::grid::gap::Takeoff::Sprint(press) => Some(Leap::SprintGap(press)),
                        mapdata::grid::gap::Takeoff::WalkOff => None,
                    }),
                    _ => None,
                },
                low: low(i - 1, i),
            });
            if into[i] == LinkKind::Jump
                && let Some(lead) = grid.press_lead(nodes[i - 1], nodes[i])
            {
                ledge_leads.push((grid.position(nodes[i]), lead));
            }
            if into[i] == LinkKind::Jump {
                ledge_tops.push((grid.position(nodes[i]), top_depth(grid, nodes[i - 1], nodes[i])));
            }
            if into[i] == LinkKind::Drop {
                sprint_drops.push((grid.position(nodes[i]), grid.drop_harmless_at(nodes[i - 1], nodes[i], SPRINT_FLIGHT_MPS)));
            } else if grid.body().walk_off_lips
                && into[i] == LinkKind::GapDrop
                && grid.off_link(nodes[i - 1], nodes[i]).is_some_and(|l| l.press == mapdata::grid::gap::Takeoff::WalkOff)
            {
                // A drop walked off the edge: made only where the body lands on one level at every walk-off speed, a
                // sprint's included (`gap::WALK_OFF_SPEEDS`), so it is harmless at a sprint. It is the route's own
                // fall: the lip guard (its take-off is a lip) lets the body over the edge. Only with `grid.walk_off_lips`:
                // without it the take-off is no lip and nothing brakes the body there.
                sprint_drops.push((grid.position(nodes[i]), true));
            }
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
            jump: None,
            low: low(from, i),
        });
        leg(from, i, &mut floors);
    }
    let mut route = Route::with_floors(points, floors);
    route.sprint_drops = sprint_drops;
    route.ledge_leads = ledge_leads;
    route.ledge_tops = ledge_tops;
    route.planned = true;
    route
}

/// How deep the top of the jump link `a` to `b` is past its face, metres, along the jump: the floors walked on past `b`
/// in the jump's direction, each a Walk link (up to [`TOP_DEPTH_MAX_M`]), and the half cell from the face to `b`'s
/// centre. The sprint lift needs a top this deep to carry the body up ([`motor::LIFT_TOP_M`]).
pub(crate) fn top_depth(grid: &WalkGrid, a: NodeId, b: NodeId) -> f32 {
    let (pa, pb) = (grid.position(a), grid.position(b));
    let across = pa.distance_xz(pb);
    if across < 1e-3 {
        return 0.0;
    }
    let (ux, uz) = ((pb.x - pa.x) / across, (pb.z - pa.z) / across);
    let (mut at, mut run) = (b, 0.0f32);
    while run < TOP_DEPTH_MAX_M {
        let here = grid.position(at);
        let next = grid.links(at).find(|&(n, kind)| {
            let p = grid.position(n);
            let len = here.distance_xz(p);
            kind == LinkKind::Walk && len > 1e-3 && ((p.x - here.x) * ux + (p.z - here.z) * uz) / len > 0.99
        });
        let Some((n, _)) = next else { break };
        run += here.distance_xz(grid.position(n));
        at = n;
    }
    run + 0.5 * grid.cell_size()
}

/// The top of a jump link is measured to this depth at most, metres.
const TOP_DEPTH_MAX_M: f32 = 3.5;

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

    /// The floors' heights of this route with the distance to each of its points now `new_at` (one for each point): the
    /// distances along a leg are scaled to the leg's new length.
    fn rescaled_floors(&self, new_at: &[f32]) -> Vec<(f32, f32)> {
        let n = self.points.len();
        self.floors
            .iter()
            .map(|&(d, y)| {
                let k = self.along.partition_point(|&a| a < d);
                if k == 0 {
                    return (d, y);
                }
                if k >= n {
                    return (new_at[n - 1] + (d - self.along[n - 1]), y);
                }
                let old_len = self.along[k] - self.along[k - 1];
                let new_len = new_at[k] - new_at[k - 1];
                let scale = if old_len > 1e-6 { new_len / old_len } else { 1.0 };
                (new_at[k - 1] + (d - self.along[k - 1]) * scale, y)
            })
            .collect()
    }

    /// This route with the middle of each doorway it crosses as a corner and a gate (`doors::centre_of`, [`Route::gate_within`]):
    /// a walked leg that crosses a doorway gets a corner at its middle, kept where both new legs are straight walks over the
    /// floors. The floors' heights keep their places along the legs. Legs of a jump, a drop or under a low floor are left as
    /// they are.
    pub fn door_centred(self, grid: &WalkGrid) -> Self {
        let n = self.points.len();
        if n < 2 {
            return self;
        }
        let mut points: Vec<Waypoint> = Vec::with_capacity(n + 2);
        // Per old leg (the one into point k): the corner put in it, if any.
        let mut corner: Vec<Option<Vec3>> = vec![None; n];
        for k in 0..n {
            if k > 0 {
                let (a, b) = (self.points[k - 1], self.points[k]);
                if b.jump.is_none() && !b.low && b.at.y - a.at.y > -DROP_LINK_MIN_M
                    && let Some(c) = super::doors::centre_of(grid, a.at, b.at)
                    && let Some(node) = grid.locate(c, 0.3)
                    && straight(grid, a.node, node)
                    && straight(grid, node, b.node)
                {
                    points.push(Waypoint { at: c, node, jump: None, low: false });
                    corner[k] = Some(c);
                }
            }
            points.push(self.points[k]);
        }
        if corner.iter().all(Option::is_none) {
            return self;
        }
        let mut new_at = Vec::with_capacity(n);
        let mut total = 0.0f32;
        for k in 0..n {
            if k > 0 {
                let (a, b) = (self.points[k - 1].at, self.points[k].at);
                total += corner[k].map_or(a.distance_xz(b), |c| a.distance_xz(c) + c.distance_xz(b));
            }
            new_at.push(total);
        }
        let floors = self.rescaled_floors(&new_at);
        let Self { sprint_drops, ledge_leads, ledge_tops, mut gates, planned, below_walk_m, past_drop, .. } = self;
        gates.extend(corner.iter().flatten());
        let mut route = Self::with_floors(points, floors);
        route.sprint_drops = sprint_drops;
        route.ledge_leads = ledge_leads;
        route.ledge_tops = ledge_tops;
        route.gates = gates;
        route.planned = planned;
        route.below_walk_m = below_walk_m;
        route.past_drop = past_drop;
        route
    }

    /// This route with a jump from its end onto `to` (on floor `node`): a
    /// way out of a pocket, which no link of the grid makes
    /// (`escape.rs`). The floors along it are kept; the jump's leg is as
    /// high as the line between its ends. A rise over `max_jump` (the grid body's standing jump) is a sprint jump,
    /// labelled as the planner labels a jump link ([`Leap::SprintLedge`]).
    pub fn then_jump(self, to: Vec3, node: NodeId, max_jump: f32) -> Self {
        let Self {
            mut points,
            mut floors,
            along,
            planned,
            sprint_drops,
            ledge_leads,
            ledge_tops,
            gates,
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
            jump: Some(if to.y - last.at.y > max_jump { Leap::SprintLedge } else { Leap::Ledge }),
            low: false,
        });
        floors.push((along + last.at.distance_xz(to), to.y));
        let mut route = Self::with_floors(points, floors);
        route.sprint_drops = sprint_drops;
        route.ledge_leads = ledge_leads;
        route.ledge_tops = ledge_tops;
        route.gates = gates;
        route.planned = planned;
        route
    }

    /// This route with its stretch from `s0` to `s1` metres along it
    /// replaced by `mid`, a route from the point at `s0` to the point at
    /// `s1` (a local detour round a lane, `threat.rs`). The corners before
    /// `s0` and after `s1` and the floors' heights along them are kept.
    pub fn spliced(&self, s0: f32, s1: f32, mid: &Route) -> Self {
        const EPS: f32 = 0.05;
        let (s0, s1) = (s0.max(0.0), s1.max(s0));
        let before = self.along.partition_point(|&a| a < s0 - EPS);
        let after = self.along.partition_point(|&a| a <= s1 + EPS);
        let mut points: Vec<Waypoint> = self.points[..before].to_vec();
        let first_mid = points.len();
        points.extend(mid.points.iter().copied());
        let first_after = points.len();
        points.extend(self.points[after..].iter().copied());
        let mut route = Self::with_floors(points, Vec::new());
        let mut floors: Vec<(f32, f32)> = self
            .floors
            .iter()
            .copied()
            .filter(|&(a, _)| a < s0 - EPS)
            .collect();
        if let Some(&start) = route.along.get(first_mid) {
            floors.extend(mid.floors.iter().map(|&(a, y)| (start + a, y)));
        }
        if let (Some(&now), Some(&was)) = (route.along.get(first_after), self.along.get(after)) {
            floors.extend(
                self.floors
                    .iter()
                    .filter(|&&(a, _)| a >= self.along[after])
                    .map(|&(a, y)| (a - was + now, y)),
            );
        }
        route.floors = floors;
        route.sprint_drops = self.sprint_drops.iter().chain(&mid.sprint_drops).copied().collect();
        route.ledge_leads = self.ledge_leads.iter().chain(&mid.ledge_leads).copied().collect();
        route.ledge_tops = self.ledge_tops.iter().chain(&mid.ledge_tops).copied().collect();
        route.gates = self.gates.iter().chain(&mid.gates).copied().collect();
        route.planned = self.planned && mid.planned;
        route
    }

    /// Whether a corner reached by a jump lies between `s0` and `s1` metres
    /// along the route.
    pub fn jumps_between(&self, s0: f32, s1: f32) -> bool {
        self.points
            .iter()
            .zip(&self.along)
            .any(|(point, &a)| point.jump.is_some() && a >= s0 && a <= s1)
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
            past_drop: PastDrop::Window,
            sprint_drops: Vec::new(),
            ledge_leads: Vec::new(),
            ledge_tops: Vec::new(),
            gates: Vec::new(),
            planned: false,
        }
    }

    /// The route of a body that stands where the grid has no floor (on the top of an object): its own place
    /// first, then the route from the floor it lands on. The first leg is a drop at its start, so the body
    /// walking off the edge is on the route (a body may be [`FALL_BACK_M`] along it above the floor).
    pub fn from_stand(self, feet: Vec3) -> Self {
        let Some(first) = self.points.first().copied() else {
            return self;
        };
        let lead = feet.distance_xz(first.at);
        let mut points = vec![Waypoint { at: feet, ..first }];
        points.extend(self.points);
        let mut floors = vec![(0.0, feet.y)];
        floors.push((1e-3, first.at.y));
        floors.extend(self.floors.into_iter().map(|(along, y)| (along + lead + 1e-3, y)));
        let mut route = Self::with_floors(points, floors);
        route.sprint_drops = self.sprint_drops;
        route.ledge_leads = self.ledge_leads;
        route.ledge_tops = self.ledge_tops;
        route.gates = self.gates;
        route.planned = self.planned;
        route
    }

    /// The distance along the route to the first door gate ([`Route::door_centred`]) that the body at `s` metres along it
    /// has not reached and that is within `ahead` metres of it, if there is one: the walk steers at the gate, not past it,
    /// so the body goes through the middle of the doorway.
    pub fn gate_within(&self, s: f32, ahead: f32) -> Option<f32> {
        self.points
            .iter()
            .zip(&self.along)
            .filter(|(point, a)| **a > s + GATE_REACHED_M && **a <= s + ahead && self.gates.contains(&point.at))
            .map(|(_, &a)| a)
            .next()
    }

    /// The door gates of this route ([`Route::door_centred`]): the corners put at the middle of a doorway.
    pub fn gates(&self) -> &[Vec3] {
        &self.gates
    }

    /// This route with its corner at `at` a door gate (tests of the walk).
    #[cfg(test)]
    pub fn with_gate(mut self, at: Vec3) -> Self {
        self.gates.push(at);
        self
    }

    /// This route's rule for a body under a walked leg ([`OFF_WALK_BELOW_M`]).
    pub fn set_below_walk(&mut self, metres: f32) {
        self.below_walk_m = metres;
    }

    /// Which floors behind a body count for where it is ([`PastDrop`]).
    pub fn set_past_drop(&mut self, past_drop: PastDrop) {
        self.past_drop = past_drop;
    }

    /// Where the floors behind the body at `s` on `segment` that count for it start ([`PastDrop`]): [`FALL_BACK_M`] back,
    /// with [`PastDrop::Landed`] not past the landing of the last drop at or before the segment's start (a waypoint a lip's
    /// height or more under the one before it, reached by no jump).
    fn fall_back_from(&self, segment: usize, s: f32) -> f32 {
        let back = s - FALL_BACK_M;
        match self.past_drop {
            PastDrop::Window => back,
            PastDrop::Landed => (1..=segment)
                .rev()
                .find(|&k| self.points[k].jump.is_none() && self.points[k - 1].at.y - self.points[k].at.y >= mapdata::grid::LIP_DROP_M)
                .map_or(back, |k| back.max(self.along[k])),
        }
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

    /// The point `s` metres along the route at the given height.
    pub fn at(&self, s: f32, height: RouteHeight) -> Vec3 {
        match height {
            RouteHeight::Chord => self.point_at(s),
            RouteHeight::Floor => self.floor_point(s),
        }
    }

    /// A route by its corners over the floors' heights `floors` (distance along it, height), as the planner makes
    /// them: the tests' hump.
    #[cfg(test)]
    pub fn over_floors(points: Vec<Waypoint>, floors: Vec<(f32, f32)>) -> Self {
        Self::with_floors(points, floors)
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
        if self.points[segment + 1].jump.is_some() {
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
            let above = |from: f32| feet.y > self.highest_floor(from, s) + OFF_ROUTE_HEIGHT_M;
            #[cfg(test)]
            if self.past_drop == PastDrop::Landed
                && feet.y >= self.floor_at(s) - self.below_m(segment)
                && !above(s - FALL_BACK_M)
                && above(self.fall_back_from(segment, s))
            {
                past_drop::count();
            }
            if feet.y < self.floor_at(s) - self.below_m(segment) || above(self.fall_back_from(segment, s)) {
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

    /// How far along the route the body was last found: the start of its segment ([`Self::progress`]), metres.
    pub fn walked(&self) -> f32 {
        self.along.get(self.segment).copied().unwrap_or(0.0)
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
            if self.points[k + 1].jump.is_some() || self.turn_at(k) > angle {
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
            .find(|&k| self.points[k].jump.is_some())
            .map(|k| (k, self.along[k - 1] - s))
    }

    /// How far before the take-off the walking jump into waypoint `k` is pressed, when its ledge's top is short
    /// ([`WalkGrid::press_lead`]): the planner knew the grid; None is the motor's own lead.
    pub fn ledge_lead(&self, k: usize) -> Option<f32> {
        let landing = self.points[k].at;
        self.ledge_leads.iter().find(|(at, _)| *at == landing).map(|&(_, lead)| lead)
    }

    /// How deep the top is past the face of the jump link into waypoint `k`, metres ([`top_depth`]); None where the
    /// planner did not measure it (a jump made some other way).
    pub fn ledge_top(&self, k: usize) -> Option<f32> {
        let landing = self.points[k].at;
        self.ledge_tops.iter().find(|(at, _)| *at == landing).map(|&(_, depth)| depth)
    }

    /// This route with this top under the jump link landing at `landing` (the tests').
    #[cfg(test)]
    pub fn with_ledge_top(mut self, landing: Vec3, depth: f32) -> Self {
        self.ledge_tops.push((landing, depth));
        self
    }

    /// Whether waypoint `k` is reached by a sprint jump (the planner's label, [`Leap::SprintLedge`]).
    pub fn sprint_ledge(&self, k: usize) -> bool {
        self.points[k].jump == Some(Leap::SprintLedge)
    }

    /// The landings of the drops of a lip's height or more the route makes from `s` to `within` metres on (a
    /// planned fall: the body walks off the edge there).
    pub fn drop_landings(&self, s: f32, within: f32, truth: DropTruth) -> Vec<Vec3> {
        let first = self.along.partition_point(|&a| a <= s).max(1);
        (first..self.points.len())
            .take_while(|&k| self.along[k - 1] <= s + within)
            .filter(|&k| self.drop_into(k, truth).is_some())
            .map(|k| self.points[k].at)
            .collect()
    }

    /// Whether the route drops onto a floor within `within` metres of `feet` (anywhere along it): a drop the planner
    /// recorded ([`DropTruth::Links`]), whatever its height, so a drop link under a lip's height counts, and the
    /// body that lands on a lower floor beside its landing (a shelf at a ledge's foot) is the route's.
    #[cfg(test)]
    pub fn drops_near(&self, feet: Vec3, within: f32) -> bool {
        (1..self.points.len()).any(|k| self.points[k].at.distance_xz(feet) <= within && self.drop_into(k, DropTruth::Links).is_some())
    }

    /// What the planner knows of the drop that waypoint `k` is reached by (`k` at least 1), as to a sprint, if it is
    /// one by `truth`: the one place the rule is read, so every reader sees the same drops.
    fn drop_into(&self, k: usize, truth: DropTruth) -> Option<DropRecord> {
        if self.points[k].jump.is_some() {
            return None;
        }
        let landing = self.points[k].at;
        let known = self.sprint_drops.iter().find(|(at, _)| *at == landing).map(|&(_, safe)| safe);
        let by_height = || self.points[k - 1].at.y - landing.y >= mapdata::grid::LIP_DROP_M;
        match (truth, known) {
            (DropTruth::Links, Some(safe)) => Some(if safe { DropRecord::Safe } else { DropRecord::Unsafe }),
            (DropTruth::Links, None) if self.planned => None,
            (DropTruth::Links, None) => by_height().then_some(DropRecord::Unknown),
            (DropTruth::Height, Some(safe)) => by_height().then_some(if safe { DropRecord::Safe } else { DropRecord::Unsafe }),
            (DropTruth::Height, None) => by_height().then_some(if self.planned { DropRecord::Walk } else { DropRecord::Unknown }),
        }
    }

    /// What the planner knows of the drops of a lip's height or more from `s` to `within` metres on
    /// ([`Self::drop_landings`]), as to a sprint.
    pub fn drop_record(&self, s: f32, within: f32, truth: DropTruth) -> DropRecord {
        let mut record = DropRecord::None;
        let first = self.along.partition_point(|&a| a <= s).max(1);
        for this in (first..self.points.len())
            .take_while(|&k| self.along[k - 1] <= s + within)
            .filter_map(|k| self.drop_into(k, truth))
        {
            let rank = |r: DropRecord| match r {
                DropRecord::None => 0,
                DropRecord::Walk => 1,
                DropRecord::Safe => 2,
                DropRecord::Unknown => 3,
                DropRecord::Unsafe => 4,
            };
            if rank(this) > rank(record) {
                record = this;
            }
        }
        record
    }

    /// How far along the route the take-off of the first drop of a lip's height or more from `s` to `within` metres on is
    /// (as [`Self::drop_landings`]); at most `s` where the body is on the drop's leg already.
    pub fn next_drop_edge_along(&self, s: f32, within: f32, truth: DropTruth) -> Option<f32> {
        let first = self.along.partition_point(|&a| a <= s).max(1);
        (first..self.points.len())
            .take_while(|&k| self.along[k - 1] <= s + within)
            .find(|&k| self.drop_into(k, truth).is_some())
            .map(|k| self.along[k - 1])
    }

    /// This route as one the planner made, with no drop recorded (the tests').
    #[cfg(test)]
    pub fn planned_by_walks(mut self) -> Self {
        self.planned = true;
        self
    }

    /// This route with these landings found safe at a sprint (the tests').
    #[cfg(test)]
    pub fn with_ledge_lead(mut self, landing: Vec3, lead: f32) -> Self {
        self.ledge_leads.push((landing, lead));
        self
    }

    #[cfg(test)]
    pub fn with_sprint_safe(mut self, landings: Vec<Vec3>) -> Self {
        self.sprint_drops = landings.into_iter().map(|at| (at, true)).collect();
        self
    }

    /// Whether a drop of a lip's height or more the route makes from `s` to `within` metres on (as
    /// [`Self::drop_landings`]) is one a sprint carries over the harmless fall: its landing is not among those the
    /// planner found safe at a sprint ([`SPRINT_FLIGHT_MPS`]).
    pub fn sprint_unsafe_drop(&self, s: f32, within: f32, truth: DropTruth) -> bool {
        self.drop_record(s, within, truth) != DropRecord::Safe
    }

    /// The first drop of a lip's height or more the route makes from `s` to `within` metres on (as
    /// [`Self::drop_landings`]): how far along its landing is, and the landing.
    pub fn next_drop(&self, s: f32, within: f32, truth: DropTruth) -> Option<(f32, Vec3)> {
        let first = self.along.partition_point(|&a| a <= s).max(1);
        (first..self.points.len())
            .take_while(|&k| self.along[k - 1] <= s + within)
            .find(|&k| self.drop_into(k, truth).is_some())
            .map(|k| (self.along[k], self.points[k].at))
    }

    /// Whether the body went over the edge of a drop of a lip's height or more (the take-off of a leg that is a
    /// planned fall) between the progress `before` and `s` of two ticks. A step of more than [`DROP_EDGE_STEP_M`]
    /// is a new route, not a walk.
    pub fn left_drop_edge(&self, before: f32, s: f32, truth: DropTruth) -> bool {
        if !(s > before && s - before <= DROP_EDGE_STEP_M) {
            return false;
        }
        let edge = self.along.partition_point(|&a| a <= before);
        (edge..self.points.len().saturating_sub(1))
            .take_while(|&k| self.along[k] <= s)
            .any(|k| self.drop_into(k + 1, truth).is_some())
    }

    /// Whether the route from `s` to `within` metres on makes a planned link: a jump, or a leg that goes down by `drop_m`
    /// or more (a Drop link: the body leaves the ground there, and no walk in a line goes with it).
    pub fn link_ahead(&self, s: f32, within: f32, drop_m: f32) -> bool {
        let first = self.along.partition_point(|&a| a <= s).max(1);
        (first..self.points.len())
            .take_while(|&k| self.along[k - 1] <= s + within)
            .any(|k| self.points[k].jump.is_some() || self.points[k - 1].at.y - self.points[k].at.y >= drop_m)
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

    /// [`PastDrop::Landed`]: a body still on a drop's top past which the drop's landing lies on the ground plane (a walk-off
    /// 0.7 m across and 3 m down, Mamayev Kurgan's) is on the drop's own leg, heading for the landing; with
    /// [`PastDrop::Window`] it is on the leg after the landing, heading for its end. Once down it is on that leg with either.
    #[test]
    fn a_body_on_a_drops_top_is_not_on_the_legs_past_its_landing() {
        let wp = |x: f32, y: f32, z: f32| Waypoint { at: Vec3::new(x, y, z), node: 0, jump: None, low: false };
        let route = Route::new(vec![wp(0.0, 3.0, 0.0), wp(5.0, 3.0, 0.0), wp(4.5, 0.0, 0.5), wp(2.5, 0.0, 3.0)]);
        let on = |past_drop: PastDrop, path: &[Vec3]| {
            let mut route = route.clone();
            route.set_past_drop(past_drop);
            for &feet in path {
                route.progress(feet);
            }
            route.segment()
        };
        // Along the top to the edge, a little past the landing on the ground plane, braked back.
        let top = [Vec3::new(2.0, 3.0, 0.0), Vec3::new(4.6, 3.0, 0.3), Vec3::new(4.4, 3.0, 0.62), Vec3::new(4.9, 3.0, 0.05)];
        assert_eq!(on(PastDrop::Window, &top), 2, "the window puts it on the leg past the landing");
        assert_eq!(on(PastDrop::Landed, &top), 1, "landed: on the drop's leg");
        // Down on the landing and on.
        let down = [Vec3::new(2.0, 3.0, 0.0), Vec3::new(4.6, 3.0, 0.3), Vec3::new(4.4, 0.0, 0.62)];
        assert_eq!(on(PastDrop::Window, &down), 2);
        assert_eq!(on(PastDrop::Landed, &down), 2);
    }

    #[test]
    fn a_stretch_of_a_route_is_replaced_by_a_detour_and_the_rest_is_kept() {
        let wp = |x: f32, z: f32| Waypoint { at: Vec3::new(x, 0.0, z), node: 0, jump: None, low: false };
        let plain = Route::new(vec![wp(0.0, 0.0), wp(30.0, 0.0), wp(60.0, 0.0), wp(90.0, 0.0)]);
        // The stretch 20-70 m goes round by z = 10.
        let mid = Route::new(vec![wp(20.0, 0.0), wp(30.0, 10.0), wp(60.0, 10.0), wp(70.0, 0.0)]);
        let round = plain.spliced(20.0, 70.0, &mid);
        let xs: Vec<(f32, f32)> = round.points().iter().map(|p| (p.at.x, p.at.z)).collect();
        assert_eq!(
            xs,
            vec![(0.0, 0.0), (20.0, 0.0), (30.0, 10.0), (60.0, 10.0), (70.0, 0.0), (90.0, 0.0)]
        );
        // 20 m, the detour's own length, 20 m.
        assert!((round.length() - (20.0 + mid.length() + 20.0)).abs() < 0.01, "{}", round.length());
        assert!(round.floors.windows(2).all(|w| w[0].0 <= w[1].0), "{:?}", round.floors);
        // No corner at the cut: the first corner after it is kept.
        let whole = plain.spliced(0.0, 90.0, &Route::new(vec![wp(0.0, 0.0), wp(90.0, 5.0)]));
        assert_eq!(whole.points().len(), 2);
    }

    #[test]
    fn a_rest_point_on_a_lip_is_moved_inward_to_a_floor_off_the_edge() {
        use crate::v026g::bots::body::tests::floor;
        use mapdata::Body;
        // A 20 m deck at height 5 over a ground 5 m lower: a drop along x = 10.
        let mut tris = floor(0.0, 0.0, 10.0, 10.0, 5.0).to_vec();
        tris.extend(floor(10.0, 0.0, 20.0, 10.0, 0.0));
        let grid = mapdata::WalkGrid::build(&tris, [0.0, 0.0, 20.0, 10.0], 0.25, Body::default());
        // On the lip (0.1 m from the edge): moved inward (west), to a floor that is not a lip and not near one.
        let on_edge = Vec3::new(9.9, 5.0, 5.0);
        let moved = off_lip(&grid, on_edge).expect("a lip");
        assert!(moved.x < on_edge.x - 0.4 && moved.x > on_edge.x - REST_SHIFT_M - 0.01, "{moved:?}");
        let node = grid.locate(moved, 0.3).unwrap();
        assert!(!grid.is_lip(node) && !grid.near_lip(node));
        // Well inside the deck, or on the ground below: left as it is.
        assert_eq!(off_lip(&grid, Vec3::new(5.0, 5.0, 5.0)), None);
        assert_eq!(off_lip(&grid, Vec3::new(15.0, 0.0, 5.0)), None);
    }

    /// The planner marks the drops a sprint carries over the harmless fall: a 4 m ledge over a strip 3.8 m wide with a
    /// pit beyond it is unsafe at a sprint (the flight clears the strip), a 3 m ledge is safe, and a route made by
    /// hand (no grid) has every drop unsafe.
    #[test]
    fn a_drop_the_sprint_flies_past_its_landing_is_unsafe_and_a_plain_ledge_is_not() {
        use crate::v026g::bots::body::tests::floor;
        use mapdata::Body;
        let route_over = |deck: f32, strip: Option<f32>| {
            let mut tris = floor(0.0, 0.0, 10.0, 10.0, deck).to_vec();
            match strip {
                Some(pit) => {
                    tris.extend(floor(10.0, 0.0, 13.8, 10.0, 0.0));
                    tris.extend(floor(13.8, 0.0, 24.0, 10.0, pit));
                }
                None => tris.extend(floor(10.0, 0.0, 24.0, 10.0, 0.0)),
            }
            let grid = mapdata::WalkGrid::build(&tris, [0.0, 0.0, 24.0, 10.0], 0.25, Body::default());
            let from = grid.locate(Vec3::new(5.0, deck, 5.0), 0.3).unwrap();
            let to = grid.locate(Vec3::new(11.5, 0.0, 5.0), 0.3).unwrap();
            plan(&grid, from, to).expect("a way down")
        };
        let strip = route_over(4.0, Some(-3.5));
        let ledge = route_over(3.0, None);
        let truth = DropTruth::Height;
        assert!(!strip.drop_landings(0.0, 10.0, truth).is_empty() && !ledge.drop_landings(0.0, 10.0, truth).is_empty());
        assert!(strip.sprint_unsafe_drop(0.0, 10.0, truth), "the sprint flies past the strip");
        assert!(!ledge.sprint_unsafe_drop(0.0, 10.0, truth), "a 3 m ledge is harmless at a sprint");
        let by_hand = Route::new(ledge.points().to_vec());
        assert!(by_hand.sprint_unsafe_drop(0.0, 10.0, truth), "none known: unsafe");
    }

    /// A descent by walk links (stairs of 0.5 m steps, 2 m down in 16 m) is a descent of a lip's height between the corners
    /// of the planner's route, but no drop: its record says so (`DropRecord::Walk`), so the sprint is not let go of for it
    /// (`Feel::drop_walk`). A 3 m ledge in the same place is a drop with a record. The routes made from another one keep
    /// the record.
    #[test]
    fn a_descent_by_walk_links_is_no_drop_and_a_record_survives_a_splice() {
        use crate::v026g::bots::body::tests::floor;
        use mapdata::Body;
        let stairs = || {
            let mut tris = Vec::new();
            for (k, h) in [2.0, 1.5, 1.0, 0.5].into_iter().enumerate() {
                tris.extend(floor(4.0 * k as f32, 0.0, 4.0 * (k as f32 + 1.0), 10.0, h));
            }
            tris.extend(floor(16.0, 0.0, 24.0, 10.0, 0.0));
            tris
        };
        let route_of = |tris: &[mapdata::Triangle], from: Vec3, to: Vec3| {
            let grid = mapdata::WalkGrid::build(tris, [0.0, 0.0, 24.0, 10.0], 0.25, Body::default());
            let a = grid.locate(from, 0.3).unwrap();
            let b = grid.locate(to, 0.3).unwrap();
            plan(&grid, a, b).expect("a way down")
        };
        let walked = route_of(&stairs(), Vec3::new(2.0, 2.0, 5.0), Vec3::new(20.0, 0.0, 5.0));
        let ahead = walked.drop_landings(0.0, 30.0, DropTruth::Height);
        assert_eq!(walked.drop_record(0.0, 30.0, DropTruth::Height), if ahead.is_empty() { DropRecord::None } else { DropRecord::Walk }, "{ahead:?}");
        // By the links the stairs have no drop at all.
        assert!(walked.drop_landings(0.0, 30.0, DropTruth::Links).is_empty());
        assert_eq!(walked.drop_record(0.0, 30.0, DropTruth::Links), DropRecord::None);
        // A ledge: a drop with a record, kept over a splice by a detour.
        let mut ledge = floor(0.0, 0.0, 10.0, 10.0, 3.0).to_vec();
        ledge.extend(floor(10.0, 0.0, 24.0, 10.0, 0.0));
        let drop = route_of(&ledge, Vec3::new(5.0, 3.0, 5.0), Vec3::new(20.0, 0.0, 5.0));
        assert_eq!(drop.drop_record(0.0, 30.0, DropTruth::Height), DropRecord::Safe);
        assert_eq!(drop.drop_record(0.0, 30.0, DropTruth::Links), DropRecord::Safe);
        let detour = Route::new(vec![
            Waypoint { at: Vec3::new(1.0, 3.0, 5.0), node: 0, jump: None, low: false },
            Waypoint { at: Vec3::new(2.0, 3.0, 6.0), node: 0, jump: None, low: false },
            Waypoint { at: Vec3::new(3.0, 3.0, 5.0), node: 0, jump: None, low: false },
        ]);
        let spliced = drop.spliced(1.0, 3.0, &detour);
        assert_eq!(spliced.drop_record(0.0, 30.0, DropTruth::Height), DropRecord::Safe, "the record survives a splice");
        assert_eq!(spliced.drop_record(0.0, 30.0, DropTruth::Links), DropRecord::Safe, "the record survives a splice");
    }

    /// A Drop link of the planner under a lip's height (the client's step is 0.8 m, a lip 1.0) is a drop by the links
    /// and none by the height, so the lip guard's planned landings, the drop aim and the landing watch miss it; a
    /// 2 m stair is the other way round.
    #[test]
    fn a_drop_link_under_a_lips_height_is_a_drop_by_the_links_only() {
        use crate::v026g::bots::body::tests::floor;
        use mapdata::Body;
        let route_over = |deck: f32| {
            let mut tris = floor(0.0, 0.0, 10.0, 10.0, deck).to_vec();
            tris.extend(floor(10.0, 0.0, 24.0, 10.0, 0.0));
            let grid = mapdata::WalkGrid::build(&tris, [0.0, 0.0, 24.0, 10.0], 0.25, Body::client());
            let from = grid.locate(Vec3::new(5.0, deck, 5.0), 0.3).unwrap();
            let to = grid.locate(Vec3::new(20.0, 0.0, 5.0), 0.3).unwrap();
            plan(&grid, from, to).expect("a way down")
        };
        let shallow = route_over(0.9);
        assert!(shallow.drop_landings(0.0, 30.0, DropTruth::Height).is_empty(), "under a lip's height");
        assert_eq!(shallow.drop_landings(0.0, 30.0, DropTruth::Links).len(), 1, "a Drop link");
        assert!(shallow.next_drop(0.0, 30.0, DropTruth::Links).is_some());
        let at = shallow.next_drop_edge_along(0.0, 30.0, DropTruth::Links).expect("its edge");
        assert!(shallow.left_drop_edge(at - 0.1, at + 0.1, DropTruth::Links));
        assert!(!shallow.left_drop_edge(at - 0.1, at + 0.1, DropTruth::Height));
        let deep = route_over(3.0);
        for truth in [DropTruth::Height, DropTruth::Links] {
            assert_eq!(deep.drop_landings(0.0, 30.0, truth).len(), 1, "{truth:?}");
        }
    }

    #[test]
    fn a_body_on_no_floor_plans_from_the_floor_it_drops_to_within_five_metres() {
        let grid = walled();
        let feet = Vec3::new(10.0, 3.0, 10.0);
        // 3 m over the floor: `locate` sees floors 2.5 m under the feet at most.
        assert!(grid.locate(feet, 1.5).is_none());
        let from = stranded_floor(&grid, feet).expect("a floor 3 m under");
        assert!(grid.position(from).y.abs() < 0.2 && grid.position(from).distance_xz(feet) < 0.3);
        // 5.5 m up the drop would hurt (`docs/notes/fall_damage.md`): no start. Nor from under the floor.
        assert!(stranded_floor(&grid, Vec3::new(10.0, 5.5, 10.0)).is_none());
        assert!(stranded_floor(&grid, Vec3::new(10.0, -1.0, 10.0)).is_none());
        let to = grid.locate(Vec3::new(30.0, 0.0, 10.0), 1.0).unwrap();
        let mut route = plan(&grid, from, to).unwrap().from_stand(feet);
        assert_eq!(route.points()[0].at, feet);
        assert!(route.floors.windows(2).all(|w| w[0].0 <= w[1].0), "{:?}", route.floors);
        // The body on its stand is on the route, and so is it on the floor it landed on, half a metre on.
        let (s, off) = route.progress(feet);
        assert!(s < 0.01 && off < 0.01, "{s} {off}");
        let (_, off) = route.progress(Vec3::new(10.5, 0.0, 10.0));
        assert!(off < 1.0, "{off}");
    }

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
            jump: None,
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
        let p = |x, y, jump: bool| Waypoint {
            at: Vec3::new(x, y, 0.0),
            node: 0,
            jump: jump.then_some(Leap::Ledge),
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
        let truth = DropTruth::Height;
        assert_eq!(route.drop_landings(0.0, 20.0, truth), vec![Vec3::new(9.0, 0.5, 0.0)]);
        // The drop's leg starts about 8 m along: out of reach from the start, in reach from 3 m.
        assert!(route.drop_landings(0.0, 6.0, truth).is_empty());
        assert_eq!(route.drop_landings(3.0, 6.0, truth).len(), 1);
        // On the drop's leg, and past it, none behind it.
        assert_eq!(route.drop_landings(8.5, 1.0, truth).len(), 1);
        assert!(route.drop_landings(10.0, 20.0, truth).is_empty());
    }

    /// The edge of a route's drop is crossed by the tick whose progress goes over its take-off: not the curb's, not a
    /// jump's, not a tick before it, and not a new route's far-off progress.
    #[test]
    fn the_edge_of_a_routes_drop_is_crossed_once() {
        let p = |x, y, jump: bool| Waypoint {
            at: Vec3::new(x, y, 0.0),
            node: 0,
            jump: jump.then_some(Leap::Ledge),
            low: false,
        };
        let route = Route::new(vec![
            p(0.0, 3.0, false),
            p(4.0, 3.0, false),
            p(4.5, 2.5, false),
            p(8.0, 2.5, false),
            p(9.0, 0.5, false),
            p(14.0, 0.5, false),
            p(15.0, 1.7, true),
        ]);
        let (curb, edge, jump) = (route.along[1], route.along[3], route.along[5]);
        for truth in [DropTruth::Height, DropTruth::Links] {
            assert!(!route.left_drop_edge(curb - 0.1, curb + 0.1, truth));
            assert!(route.left_drop_edge(edge - 0.1, edge + 0.1, truth));
            assert!(!route.left_drop_edge(edge - 0.3, edge - 0.1, truth));
            assert!(!route.left_drop_edge(edge + 0.1, edge + 0.3, truth));
            // Backwards, or a step too long for a walk, is no crossing.
            assert!(!route.left_drop_edge(edge + 0.1, edge - 0.1, truth));
            assert!(!route.left_drop_edge(edge - 2.0, edge + 2.0, truth));
            // The jump's take-off is no drop.
            assert!(!route.left_drop_edge(jump - 0.1, jump + 0.1, truth));
        }
    }

    /// A body that fell off a walkway its route goes on along is off the
    /// route, though the walkway is right over it on the ground plane; one
    /// a step under it or a jump's rise over it is on it.
    #[test]
    fn a_body_on_the_floor_under_its_route_is_off_it() {
        let p = |x, y, z| Waypoint {
            at: Vec3::new(x, y, z),
            node: 0,
            jump: None,
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
            jump: None,
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
        let p = |x, y, z, jump: bool| Waypoint {
            at: Vec3::new(x, y, z),
            node: 0,
            jump: jump.then_some(Leap::Ledge),
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
