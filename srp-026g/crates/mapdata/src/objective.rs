//! [F4] The objective contract (owner-16 part 3 §16 F4, the format agreed with the batteries' session 2026-11-18): for every
//! battery spawner of a map and each team, whether a bot can take the battery, carry it home and put it in, by the client's
//! rules, worked out once when the map loads (no match needs to try it first).
//!
//! A row per (spawner, team):
//! - the **take stand**: a floor from whose eye the client's use ray ([`battery_reach`]: `detect_usable_object` 0x2D84C0, its
//!   length the cvar `usable_objects_detection_distance` 1.80 m, [`USE_RAY_CLIENT_M`]) meets the battery, the look down to it
//!   within the view's stop (an input: the match server's `pitch.rs`);
//! - the **way in**: the grid's path from the team's spawns to that stand; proven when every link on it is the client's own
//!   (a flown press whose own flight lands, [`crate::grid::OffLink::window`]; a walk; a drop the build judged by where its
//!   flight rests, F2), else the first link that is not, and why;
//! - the **way home**: the path from the stand to the team's put stand for a CARRIER's body (`base_player::can_sprint`
//!   0x2DBF40 refuses the sprint while carrying, A331: [`Reach::no_sprint`], a sprint link only by its walking press);
//! - the **put stand**: a stand of the team's container ([`UseStands`], the same 1.80 m ray, 0x31CCF0);
//! - the **verdict**.
//!
//! A battery dropped elsewhere is asked the same once it rests ([`Contract::rows_for`]). Zone costs are the match server's
//! (its hazards), not this module's.

use crate::grid::gap::Takeoff;
use crate::grid::{Aim, Entry, LinkKind, NodeId, OffLink, Open, Path, Reach, Riser, Share, SprintReach, WalkGrid};
use crate::stands::{UseStands, USE_RAY_CLIENT_M};
use crate::{RayTree, Vec3, World};

/// The battery's inner ball, m (the batteries' session's rule): the saved position is its box's centre, half sizes 0.30 x
/// 0.12 x 0.16 with no orientation known, so the use ray is asked to enter the ball of its least half size.
pub const BATTERY_INNER_M: f32 = 0.12;
/// How far short of the ray's entry the line from the eye must be clear of the bullet mesh, m (the batteries' session's rule:
/// the box's own surface and the floor it lies on are not in the way).
pub const USE_LINE_SLACK_M: f32 = 0.15;
/// How far across from the battery a take stand is looked for, m: the use ray's length and the inner ball.
const STAND_SEARCH_M: f32 = USE_RAY_CLIENT_M + BATTERY_INNER_M;
/// How far a spawn's floor is looked for under its point, m.
const SPAWN_FLOOR_M: f32 = 1.0;

/// [F4] The client's use ray from `eye` onto a battery at `at` (the batteries' session's rule, one test for both sessions): the
/// distance at which the ray toward `at` enters the battery's inner ball ([`BATTERY_INNER_M`]), when it is within the use ray
/// ([`USE_RAY_CLIENT_M`]) and the line to [`USE_LINE_SLACK_M`] short of it meets nothing of `sight`; None otherwise.
pub fn battery_reach(sight: &RayTree, eye: Vec3, at: Vec3) -> Option<f32> {
    let v = at - eye;
    let d = v.length();
    let entry = (d - BATTERY_INNER_M).max(0.0);
    if entry > USE_RAY_CLIENT_M {
        return None;
    }
    if d < 1e-3 {
        return Some(0.0);
    }
    let clear = (entry - USE_LINE_SLACK_M).max(0.0);
    sight.ray_cast(eye, v * (1.0 / d), clear).is_none().then_some(entry)
}

/// [F4] What the contract takes from the match server: the eye's height over the feet standing and crouched (`view::eye_height`)
/// and the view's steepest look down (`pitch.rs`, radians under the horizontal).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContractInputs {
    pub eye_standing: f32,
    pub eye_crouched: f32,
    pub look_down_max_rad: f32,
}

/// [F4] A stand: its floor, crouched or standing, and the look down to the battery from its eye (radians under the horizontal).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stand {
    pub node: NodeId,
    pub crouch: bool,
    pub look_down_rad: f32,
}

/// [F4] Why a way is not proven.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WayFail {
    /// No path at all.
    NoPath,
    /// The path's off-mesh link is not in the grid's table.
    Unproven,
    /// The link is not flown (no share).
    Unflown,
    /// The press played does not land itself (neighbour 0 of its share, [`Share::window`] None), or no press gets up the riser.
    PressMisses,
    /// A sprint link with no walking press, for a body with no sprint.
    NoWalkingPress,
    /// A plain Jump link on the path (in the grid's slots, no press stored or flown).
    PlainJump,
}

/// [F4] A way between two floors.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Way {
    /// Every link proven: the path's cost, the least landing count of its flown links (None when it has none), and whether it
    /// sprints (a sprint press or a sprint climb on it).
    Proven { cost: f32, min_share: Option<u8>, sprint: bool },
    /// The path's first link that is not proven, `(from, to, kind)` (None with no path), and why.
    Fails { at: Option<(NodeId, NodeId, LinkKind)>, why: WayFail },
}

impl Way {
    fn proven(&self) -> bool {
        matches!(self, Way::Proven { .. })
    }
}

/// [F4] A row's verdict, the first failing step in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Ok,
    NoTakeStand,
    NoWayIn,
    NoWayHome,
    NoPutStand,
}

/// [F4] One (battery, team) of the contract.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Row {
    /// The spawner's index in `MapData::battery_spawners` (None for a battery at rest elsewhere).
    pub spawner: Option<usize>,
    pub at: Vec3,
    pub team: i32,
    pub take: Option<Stand>,
    pub way_in: Way,
    pub put: Option<Stand>,
    pub way_home: Way,
    pub verdict: Verdict,
}

impl Row {
    /// The log's line: spawner, team, verdict, and the failing link with its why.
    pub fn line(&self, grid: &WalkGrid) -> String {
        let p = |n: NodeId| {
            let v = grid.position(n);
            format!("({:.2}, {:.2}, {:.2})", v.x, v.y, v.z)
        };
        let way = |w: &Way| match w {
            Way::Proven { cost, min_share, sprint } => format!("proven cost {cost:.1} share {min_share:?} sprint {sprint}"),
            Way::Fails { at: Some((a, b, k)), why } => format!("fails at {} -> {} {k:?}: {why:?}", p(*a), p(*b)),
            Way::Fails { at: None, why } => format!("fails: {why:?}"),
        };
        format!(
            "battery {:?} at ({:.2}, {:.2}, {:.2}) team {}: {:?}; take {}; in {}; put {}; home {}",
            self.spawner,
            self.at.x,
            self.at.y,
            self.at.z,
            self.team,
            self.verdict,
            self.take.map_or("none".into(), |s| format!("{} crouch {} look down {:.2}", p(s.node), s.crouch, s.look_down_rad)),
            way(&self.way_in),
            self.put.map_or("none".into(), |s| p(s.node)),
            way(&self.way_home),
        )
    }
}

/// [F4] The contract of a map: a row per battery spawner and team.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Contract {
    pub rows: Vec<Row>,
}

impl Contract {
    /// Every battery spawner of `world`'s map for each team that has a container ([module documentation](self)); `stands` the
    /// containers' stands at the client's ray ([`UseStands::build_ray`] with [`USE_RAY_CLIENT_M`]).
    pub fn build(world: &World, stands: &UseStands, inputs: &ContractInputs) -> Self {
        // Per team one search from its spawns (the ways in) and one back from its put stands (the ways home), shared by every
        // battery: four searches a map, not two a row (a row's own two cost 387 s on Vector Laboratory's 321,604 floors).
        let back = Backward::of(&world.grid);
        let teams: Vec<TeamTrees> = teams(world).into_iter().map(|team| TeamTrees::grow(world, stands, &back, team)).collect();
        let spawners: Vec<Vec3> = world.map.battery_spawners.iter().map(|s| s.item).collect();
        let rows = spawners
            .iter()
            .enumerate()
            .flat_map(|(i, &at)| {
                let takes = take_stands(&world.grid, &world.sight, inputs, at);
                teams.iter().map(move |t| t.row(&world.grid, Some(i), at, &takes)).collect::<Vec<_>>()
            })
            .collect();
        Self { rows }
    }

    /// The rows of a battery resting at `at` (dropped, not at a spawner), one per team: each way by one search
    /// ([`nearest_path`]).
    pub fn rows_for(world: &World, stands: &UseStands, inputs: &ContractInputs, at: Vec3) -> Vec<Row> {
        let takes = take_stands(&world.grid, &world.sight, inputs, at);
        teams(world).into_iter().map(|team| row(world, stands, None, at, team, &takes)).collect()
    }
}

/// The teams that have a container, in order.
fn teams(world: &World) -> Vec<i32> {
    let mut teams: Vec<i32> = world.map.containers.iter().map(|c| c.team).collect();
    teams.sort_unstable();
    teams.dedup();
    teams
}

/// A team's spawn floors.
fn spawn_floors(world: &World, team: i32) -> Vec<NodeId> {
    world.map.spawns.iter().filter(|s| s.team == team).filter_map(|s| world.grid.locate(s.position, SPAWN_FLOOR_M)).collect()
}

/// A team's put stands (its containers' stands, [`UseStands`]).
fn put_stands(world: &World, stands: &UseStands, team: i32) -> Vec<Stand> {
    world
        .map
        .containers
        .iter()
        .filter(|c| c.team == team)
        .flat_map(|c| stands.of(c.id))
        .filter_map(|s| world.grid.locate(s.feet, 0.3).map(|node| Stand { node, crouch: !s.standing, look_down_rad: -s.pitch }))
        .collect()
}

/// The links into each floor, every link of [`WalkGrid::links`] turned round (`WalkGrid::incoming` sees only the slot links of
/// the next cell): a search back from the put stands takes the same links at the same costs as one forward to them.
struct Backward {
    first: Vec<u32>,
    from: Vec<(NodeId, LinkKind)>,
}

impl Backward {
    fn of(grid: &WalkGrid) -> Self {
        let n = grid.len();
        let mut first = vec![0u32; n + 1];
        for u in 0..n as NodeId {
            for (v, _) in grid.links(u) {
                first[v as usize + 1] += 1;
            }
        }
        for v in 0..n {
            first[v + 1] += first[v];
        }
        let mut at = first.clone();
        let mut from = vec![(0, LinkKind::Walk); first[n] as usize];
        for u in 0..n as NodeId {
            for (v, kind) in grid.links(u) {
                from[at[v as usize] as usize] = (u, kind);
                at[v as usize] += 1;
            }
        }
        Self { first, from }
    }

    fn links_into(&self, v: NodeId) -> &[(NodeId, LinkKind)] {
        &self.from[self.first[v as usize] as usize..self.first[v as usize + 1] as usize]
    }
}

/// A search's tree: each floor's cost from the seeds (or, searched back, to them) and the link to its neighbour on the way.
struct Tree {
    cost: Vec<f32>,
    /// Forward: the floor it was reached from and the link's kind; back: the floor it goes on to and the link's kind; a seed: itself.
    next: Vec<(NodeId, LinkKind)>,
    back: bool,
}

impl Tree {
    /// The Dijkstra from every seed at once over the links' costs for `reach` ([`WalkGrid::link_cost_reaching`]); back over the
    /// links into each floor (`back`), each priced as the forward link it is.
    fn grow(grid: &WalkGrid, seeds: &[NodeId], reach: Reach, back: Option<&Backward>, links: Links) -> Self {
        let n = grid.len();
        let mut cost = vec![f32::INFINITY; n];
        let mut next: Vec<(NodeId, LinkKind)> = (0..n as NodeId).map(|v| (v, LinkKind::Walk)).collect();
        let mut open = Open::default();
        for &a in seeds {
            if (a as usize) < n && cost[a as usize] > 0.0 {
                cost[a as usize] = 0.0;
                open.push(Entry(0.0, a));
            }
        }
        let mut out: Vec<(NodeId, NodeId, LinkKind, f32)> = Vec::new();
        while let Some(Entry(c, u)) = open.pop() {
            // A stale entry: a cheaper one of `u` was settled.
            if c > cost[u as usize] {
                continue;
            }
            out.clear();
            match back {
                None => out.extend(
                    grid.links(u)
                        .filter(|&(v, kind)| links.take(grid, u, v, kind, reach))
                        .map(|(v, kind)| (v, u, kind, grid.link_cost_reaching(u, v, kind, reach))),
                ),
                Some(b) => out.extend(
                    b.links_into(u)
                        .iter()
                        .filter(|&&(w, kind)| links.take(grid, w, u, kind, reach))
                        .map(|&(w, kind)| (w, u, kind, grid.link_cost_reaching(w, u, kind, reach))),
                ),
            }
            for &(v, via, kind, price) in &out {
                let cv = c + price;
                if cv < cost[v as usize] {
                    cost[v as usize] = cv;
                    next[v as usize] = (via, kind);
                    open.push(Entry(cv, v));
                }
            }
        }
        Self { cost, next, back: back.is_some() }
    }

    /// The path from the seeds to `v` (forward) or from `v` to the seeds (back), as [`WalkGrid::path_where_reaching`] gives
    /// one (`kinds[i]` the link into `nodes[i]`, the first a Walk); None where the search did not get.
    fn path(&self, v: NodeId) -> Option<Path> {
        let cost = *self.cost.get(v as usize)?;
        if !cost.is_finite() {
            return None;
        }
        let (mut nodes, mut kinds) = (vec![v], vec![LinkKind::Walk]);
        let mut x = v;
        while self.next[x as usize].0 != x {
            let (y, kind) = self.next[x as usize];
            nodes.push(y);
            // Forward the kind is of the link into `x`; back, of the link into `y`.
            if self.back {
                kinds.push(kind);
            } else {
                *kinds.last_mut().expect("a kind") = kind;
                kinds.push(LinkKind::Walk);
            }
            x = y;
        }
        if !self.back {
            nodes.reverse();
            kinds.reverse();
        }
        Some(Path { nodes, kinds, cost })
    }
}

/// A team's two trees: from its spawns for the grid's own body (the ways in), back from its put stands for a carrier's (the
/// ways home, [`Reach::no_sprint`]).
struct TeamTrees {
    team: i32,
    spawns: Vec<NodeId>,
    /// Over proven links only ([`Links::Proven`]).
    from_spawns: Tree,
    /// Over proven links only, back from the put stands.
    to_puts: Tree,
    puts: Vec<Stand>,
}

impl TeamTrees {
    fn grow(world: &World, stands: &UseStands, back: &Backward, team: i32) -> Self {
        let puts = put_stands(world, stands, team);
        let put_nodes: Vec<NodeId> = puts.iter().map(|s| s.node).collect();
        let spawns = spawn_floors(world, team);
        Self {
            team,
            from_spawns: Tree::grow(&world.grid, &spawns, Reach::FULL, None, Links::Proven),
            to_puts: Tree::grow(&world.grid, &put_nodes, Reach::no_sprint(None), Some(back), Links::Proven),
            spawns,
            puts,
        }
    }

    /// The row of a battery at `at` with its take stands: the cheapest take stand in, its way home.
    fn row(&self, grid: &WalkGrid, spawner: Option<usize>, at: Vec3, takes: &[Stand]) -> Row {
        let take = takes
            .iter()
            .filter(|s| self.from_spawns.cost[s.node as usize].is_finite())
            .min_by(|a, b| self.from_spawns.cost[a.node as usize].total_cmp(&self.from_spawns.cost[b.node as usize]))
            .copied();
        let take_nodes: Vec<NodeId> = takes.iter().map(|s| s.node).collect();
        let mut any_take = None;
        let way_in = way_of(grid, Reach::FULL, take.and_then(|t| self.from_spawns.path(t.node)), || {
            let p = nearest_path(grid, &self.spawns, &take_nodes, Reach::FULL, Links::Any);
            any_take = p.as_ref().and_then(|p| p.nodes.last()).and_then(|&n| takes.iter().find(|s| s.node == n).copied());
            p
        });
        let take = take.or(any_take).or_else(|| takes.first().copied());
        let put_nodes: Vec<NodeId> = self.puts.iter().map(|s| s.node).collect();
        let proven_home = take.and_then(|t| self.to_puts.path(t.node));
        let mut home_end = proven_home.as_ref().map(|p| *p.nodes.last().expect("a path has its end"));
        let way_home = match take {
            Some(t) => way_of(grid, Reach::no_sprint(None), proven_home, || {
                let p = nearest_path(grid, &[t.node], &put_nodes, Reach::no_sprint(None), Links::Any);
                home_end = p.as_ref().map(|p| *p.nodes.last().expect("a path has its end"));
                p
            }),
            None => Way::Fails { at: None, why: WayFail::NoPath },
        };
        let put = home_end.and_then(|n| self.puts.iter().find(|s| s.node == n).copied()).or_else(|| self.puts.first().copied());
        verdict_row(spawner, at, self.team, take, way_in, put, way_home)
    }
}

/// The take stands of a battery at `at`: every floor within [`STAND_SEARCH_M`] across, standing where a standing body fits and
/// crouched, whose eye's use ray meets the battery ([`battery_reach`]) with the look down within the view's stop.
fn take_stands(grid: &WalkGrid, sight: &RayTree, inputs: &ContractInputs, at: Vec3) -> Vec<Stand> {
    let mut out = Vec::new();
    let r = STAND_SEARCH_M;
    for node in grid.floors_in([at.x - r, at.z - r, at.x + r, at.z + r]) {
        let feet = grid.position(node);
        if feet.distance_xz(at) > STAND_SEARCH_M || (feet.y - at.y).abs() > STAND_SEARCH_M {
            continue;
        }
        for (crouch, eye_h) in [(false, inputs.eye_standing), (true, inputs.eye_crouched)] {
            if !crouch && !grid.can_stand(node) {
                continue;
            }
            let eye = feet + Vec3::new(0.0, eye_h, 0.0);
            let v = at - eye;
            let look_down = (-v.y).atan2(v.x.hypot(v.z));
            if look_down > inputs.look_down_max_rad || battery_reach(sight, eye, at).is_none() {
                continue;
            }
            out.push(Stand { node, crouch, look_down_rad: look_down });
        }
    }
    out
}

/// The press a body of `reach` plays on the off-mesh link `l`, as the path search prices it (`WalkGrid::link_cost`): a body with
/// no sprint plays a sprint link's walking press ([`OffLink::walk`]; a sprint climb's on its own line only); None when it has
/// none.
fn played(l: &OffLink, reach: Reach) -> Option<(Takeoff, Share, Aim)> {
    match (reach.sprint, l.press, l.walk) {
        (SprintReach::Cannot { .. }, Takeoff::Climb, Some((walk, share))) if l.aim == Aim::STRAIGHT => {
            Some((walk.takeoff(), share, l.aim))
        }
        (SprintReach::Cannot { .. }, Takeoff::Climb, _) => None,
        (SprintReach::Cannot { .. }, Takeoff::Sprint(_) | Takeoff::SprintHeld(_), walk) => {
            walk.map(|(walk, share)| (walk.takeoff(), share, l.aim))
        }
        _ => Some((l.press, l.share, l.aim)),
    }
}

/// Whether the link `a` -> `b` of `kind` is the client's own for a body of `reach`: an off-mesh link and a riser (a Walk link
/// jumped up, [`WalkGrid::riser`]) by a flown press whose own flight lands ([`Share::window`]); a walk-off as the build's drop
/// (no press, no share: where its flight rests is the drop check's, F2, as a Drop link's); no plain Jump link (no press stored
/// for it); walks and drops the build's own. Ok: the flown share counted (None for a link with none) and whether it sprints.
fn link_judged(grid: &WalkGrid, a: NodeId, b: NodeId, kind: LinkKind, reach: Reach) -> Result<(Option<u8>, bool), WayFail> {
    match kind {
        LinkKind::GapJump | LinkKind::GapDrop => {
            let l = grid.off_link(a, b).ok_or(WayFail::Unproven)?;
            if l.press == Takeoff::WalkOff {
                return Ok((None, false));
            }
            let (press, share, aim) = played(&l, reach).ok_or(WayFail::NoWalkingPress)?;
            share.flown().ok_or(WayFail::Unflown)?;
            share.window(press, aim).ok_or(WayFail::PressMisses)?;
            Ok((share.flown(), press.sprint_run_m().is_some()))
        }
        LinkKind::Walk => match grid.riser(a, b) {
            None => Ok((None, false)),
            Some(Riser::Pressed { press: Takeoff::Sprint(_) | Takeoff::SprintHeld(_), .. })
                if matches!(reach.sprint, SprintReach::Cannot { .. }) =>
            {
                Err(WayFail::NoWalkingPress)
            }
            Some(Riser::Pressed { press, share }) => {
                share.window(press, Aim::STRAIGHT).ok_or(WayFail::PressMisses)?;
                Ok((share.flown(), press.sprint_run_m().is_some()))
            }
            Some(Riser::NoPress) => Err(WayFail::PressMisses),
        },
        LinkKind::Jump => Err(WayFail::PlainJump),
        LinkKind::Drop => Ok((None, false)),
    }
}

/// Whether the path's links are each the client's own for a body of `reach` ([`link_judged`]); the first that is not, as the
/// way's failure.
fn judge(grid: &WalkGrid, path: &Path, reach: Reach) -> Way {
    let mut min_share: Option<u8> = None;
    let mut sprint = false;
    for (pair, &kind) in path.nodes.windows(2).zip(&path.kinds[1..]) {
        let (a, b) = (pair[0], pair[1]);
        match link_judged(grid, a, b, kind, reach) {
            Ok((share, sprints)) => {
                if let Some(n) = share {
                    min_share = Some(min_share.map_or(n, |m| m.min(n)));
                }
                sprint |= sprints;
            }
            Err(why) => return Way::Fails { at: Some((a, b, kind)), why },
        }
    }
    Way::Proven { cost: path.cost, min_share, sprint }
}

/// [F4] Which links a search takes: every link, or only those [`link_judged`] proves (a way of them is a proven way).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Links {
    Any,
    Proven,
}

impl Links {
    fn take(self, grid: &WalkGrid, a: NodeId, b: NodeId, kind: LinkKind, reach: Reach) -> bool {
        self == Links::Any || link_judged(grid, a, b, kind, reach).is_ok()
    }
}

/// A way from the cheapest proven path (`proven`), else the first failing link of the cheapest path of any links (`any`,
/// searched only then), else no path: a battery a body reaches by a longer proven way is not refused for a cheaper unproven one.
fn way_of(grid: &WalkGrid, reach: Reach, proven: Option<Path>, any: impl FnOnce() -> Option<Path>) -> Way {
    match proven {
        Some(path) => judge(grid, &path, reach),
        None => match any() {
            Some(path) => judge(grid, &path, reach),
            None => Way::Fails { at: None, why: WayFail::NoPath },
        },
    }
}

/// The cheapest way from any of `from` to any of `to` for `reach` ([`way_of`]); with the goal it reached.
fn best_way(grid: &WalkGrid, from: &[NodeId], to: &[NodeId], reach: Reach) -> (Way, Option<NodeId>) {
    let proven = nearest_path(grid, from, to, reach, Links::Proven);
    let goal = proven.as_ref().map(|p| *p.nodes.last().expect("a path has its goal"));
    let mut any_goal = None;
    let way = way_of(grid, reach, proven, || {
        let p = nearest_path(grid, from, to, reach, Links::Any);
        any_goal = p.as_ref().map(|p| *p.nodes.last().expect("a path has its goal"));
        p
    });
    (way, goal.or(any_goal))
}

/// The cheapest path for `reach` from any of `from` to any of `to`, in one search (the batteries' session's ask: a search per
/// pair of spawn floor and take stand cost 3.5 s on a 50,423-floor field): a Dijkstra from every floor of `from` at once over
/// the links' costs for `reach` ([`WalkGrid::link_cost_reaching`], what [`WalkGrid::path_where_reaching`] prices), stopped at
/// the first floor of `to` it settles. The same cost as the best of the pairs' searches; None when no floor of `to` is reached.
fn nearest_path(grid: &WalkGrid, from: &[NodeId], to: &[NodeId], reach: Reach, links: Links) -> Option<Path> {
    let n = grid.len();
    let mut goal = vec![false; n];
    for &b in to {
        if let Some(g) = goal.get_mut(b as usize) {
            *g = true;
        }
    }
    let mut cost = vec![f32::INFINITY; n];
    // The floor each was reached from and the link taken (the start's: itself, a Walk).
    let mut came: Vec<(NodeId, LinkKind)> = (0..n as NodeId).map(|v| (v, LinkKind::Walk)).collect();
    let mut open = Open::default();
    for &a in from {
        if (a as usize) < n && cost[a as usize] > 0.0 {
            cost[a as usize] = 0.0;
            open.push(Entry(0.0, a));
        }
    }
    while let Some(Entry(c, u)) = open.pop() {
        if c > cost[u as usize] {
            continue;
        }
        if goal[u as usize] {
            let mut nodes = vec![u];
            let mut kinds = vec![came[u as usize].1];
            let mut v = u;
            while came[v as usize].0 != v {
                v = came[v as usize].0;
                nodes.push(v);
                kinds.push(came[v as usize].1);
            }
            nodes.reverse();
            kinds.reverse();
            return Some(Path { nodes, kinds, cost: c });
        }
        for (v, kind) in grid.links(u).filter(|&(v, kind)| links.take(grid, u, v, kind, reach)) {
            let cv = c + grid.link_cost_reaching(u, v, kind, reach);
            if cv < cost[v as usize] {
                cost[v as usize] = cv;
                came[v as usize] = (u, kind);
                open.push(Entry(cv, v));
            }
        }
    }
    None
}

fn row(world: &World, stands: &UseStands, spawner: Option<usize>, at: Vec3, team: i32, takes: &[Stand]) -> Row {
    let grid = &world.grid;
    let spawns = spawn_floors(world, team);
    let take_nodes: Vec<NodeId> = takes.iter().map(|s| s.node).collect();
    let (way_in, reached) = if take_nodes.is_empty() {
        (Way::Fails { at: None, why: WayFail::NoPath }, None)
    } else {
        best_way(grid, &spawns, &take_nodes, Reach::FULL)
    };
    let take = reached.and_then(|n| takes.iter().find(|s| s.node == n).copied()).or_else(|| takes.first().copied());
    let puts = put_stands(world, stands, team);
    let put_nodes: Vec<NodeId> = puts.iter().map(|s| s.node).collect();
    let (way_home, put_node) = match take {
        Some(t) if !put_nodes.is_empty() => best_way(grid, &[t.node], &put_nodes, Reach::no_sprint(None)),
        _ => (Way::Fails { at: None, why: WayFail::NoPath }, None),
    };
    let put = put_node.and_then(|n| puts.iter().find(|s| s.node == n).copied()).or_else(|| puts.first().copied());
    verdict_row(spawner, at, team, take, way_in, put, way_home)
}

/// A row with its verdict: the first failing step in order.
fn verdict_row(spawner: Option<usize>, at: Vec3, team: i32, take: Option<Stand>, way_in: Way, put: Option<Stand>, way_home: Way) -> Row {
    let verdict = if take.is_none() {
        Verdict::NoTakeStand
    } else if !way_in.proven() {
        Verdict::NoWayIn
    } else if put.is_none() {
        Verdict::NoPutStand
    } else if !way_home.proven() {
        Verdict::NoWayHome
    } else {
        Verdict::Ok
    };
    Row { spawner, at, team, take, way_in, put, way_home, verdict }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BatterySpawner, Body, Container, MapData, Rotation, Shape, ShapeKind, Spawn, Triangle, WalkerBehaviour};

    fn quad(x0: f32, z0: f32, x1: f32, z1: f32, y: f32) -> [Triangle; 2] {
        let t = |v: [Vec3; 3]| Triangle { v, walker: WalkerBehaviour::BySlope };
        [
            t([Vec3::new(x0, y, z0), Vec3::new(x0, y, z1), Vec3::new(x1, y, z1)]),
            t([Vec3::new(x0, y, z0), Vec3::new(x1, y, z1), Vec3::new(x1, y, z0)]),
        ]
    }

    const INPUTS: ContractInputs = ContractInputs { eye_standing: 1.46, eye_crouched: 1.04, look_down_max_rad: 1.15 };

    /// A flat yard 20 m square: a battery on the ground at (4, 0.1, 0); each team's spawn and container box at its end; a
    /// battery inside a closed box of walls (no ray reaches it) when `boxed`.
    fn world(boxed: bool) -> World {
        let mut tris = quad(-10.0, -10.0, 10.0, 10.0, 0.0).to_vec();
        let mut sight = tris.clone();
        if boxed {
            let wall = |a: Vec3, b: Vec3| {
                let t = |v: [Vec3; 3]| Triangle { v, walker: WalkerBehaviour::BySlope };
                let (a2, b2) = (a + Vec3::new(0.0, 3.0, 0.0), b + Vec3::new(0.0, 3.0, 0.0));
                [t([a, b, b2]), t([a, b2, a2])]
            };
            for (a, b) in [((3.0, -1.0), (5.0, -1.0)), ((5.0, -1.0), (5.0, 1.0)), ((5.0, 1.0), (3.0, 1.0)), ((3.0, 1.0), (3.0, -1.0))] {
                let w = wall(Vec3::new(a.0, 0.0, a.1), Vec3::new(b.0, 0.0, b.1));
                sight.extend(w);
                tris.extend(w);
            }
        }
        let grid = WalkGrid::build(&tris, [-10.0, -10.0, 10.0, 10.0], 0.25, Body::default());
        let shape = |x: f32| Shape { kind: ShapeKind::Box, center: Vec3::new(x, 1.0, 0.0), rotation: Rotation::IDENTITY, half: Vec3::new(0.4, 0.4, 0.4) };
        let spawn = |team: i32, x: f32| Spawn { position: Vec3::new(x, 0.0, 5.0), rotation: Vec3::ZERO, team, priority: 0, point_id: 0, layer: String::new() };
        let map = MapData {
            spawns: vec![spawn(0, -8.0), spawn(1, 8.0)],
            containers: vec![
                Container { position: Vec3::new(-8.0, 0.0, 0.0), team: 0, id: 0, use_shapes: vec![shape(-8.0)] },
                Container { position: Vec3::new(8.0, 0.0, 0.0), team: 1, id: 1, use_shapes: vec![shape(8.0)] },
            ],
            battery_spawners: vec![BatterySpawner { position: Vec3::new(4.0, 0.0, 0.0), item: Vec3::new(4.0, 0.1, 0.0), layer: String::new() }],
            ..MapData::default()
        };
        World::assemble(map, grid.into(), RayTree::build(&sight).into())
    }

    /// The one search's path costs what the best of the pairs' searches does, for the full reach and a carrier's, ends on a
    /// goal and starts on a source; with no goal reached, none.
    #[test]
    fn the_one_search_finds_the_best_pair() {
        let w = world(false);
        let g = &w.grid;
        let at = |x: f32, z: f32| g.locate(Vec3::new(x, 0.0, z), 0.3).unwrap();
        let from = [at(-8.0, 5.0), at(-8.0, -5.0), at(0.0, 8.0)];
        let to = [at(4.0, 1.0), at(5.0, 0.0), at(8.0, -6.0)];
        for reach in [Reach::FULL, Reach::no_sprint(None)] {
            let one = nearest_path(g, &from, &to, reach, Links::Any).expect("a path");
            let pairs = from
                .iter()
                .flat_map(|&a| to.iter().filter_map(move |&b| g.path_where_reaching(a, b, |_| true, reach)))
                .map(|p| p.cost)
                .fold(f32::INFINITY, f32::min);
            assert!((one.cost - pairs).abs() < 1e-3, "{} vs {pairs}", one.cost);
            assert!(from.contains(&one.nodes[0]) && to.contains(one.nodes.last().unwrap()));
            assert_eq!(one.nodes.len(), one.kinds.len());
        }
        let shut = world(true);
        let inside = shut.grid.locate(Vec3::new(4.0, 0.0, 0.0), 0.3).unwrap();
        let outside = shut.grid.locate(Vec3::new(-8.0, 0.0, 5.0), 0.3).unwrap();
        assert!(nearest_path(&shut.grid, &[outside], &[inside], Reach::FULL, Links::Any).is_none());
    }

    /// The map's rows by the teams' shared trees equal each battery's own searches ([`Contract::rows_for`]): the same verdicts and
    /// ways, the same costs; the backward tree's way home from a stand costs what a forward search from it to the put stands does.
    #[test]
    fn the_teams_trees_give_each_batterys_own_rows() {
        for boxed in [false, true] {
            let w = world(boxed);
            let stands = UseStands::build_ray(&w, crate::stands::EYE_STANDING, crate::stands::EYE_CROUCHED, USE_RAY_CLIENT_M);
            let shared = Contract::build(&w, &stands, &INPUTS);
            let own: Vec<Row> = w
                .map
                .battery_spawners
                .iter()
                .enumerate()
                .flat_map(|(i, s)| Contract::rows_for(&w, &stands, &INPUTS, s.item).into_iter().map(move |r| Row { spawner: Some(i), ..r }))
                .collect();
            assert_eq!(shared.rows.len(), own.len());
            let cost = |w: &Way| match w {
                Way::Proven { cost, .. } => Some(*cost),
                Way::Fails { .. } => None,
            };
            for (a, b) in shared.rows.iter().zip(&own) {
                assert_eq!((a.verdict, a.team), (b.verdict, b.team), "{boxed}: {}\n{}", a.line(&w.grid), b.line(&w.grid));
                for (x, y) in [(cost(&a.way_in), cost(&b.way_in)), (cost(&a.way_home), cost(&b.way_home))] {
                    assert!(x.zip(y).is_none_or(|(x, y)| (x - y).abs() < 1e-3) && x.is_some() == y.is_some(), "{x:?} {y:?}");
                }
            }
        }
        // Back from the put stands: the same cost as forward from a stand to them.
        let w = world(false);
        let stands = UseStands::build_ray(&w, crate::stands::EYE_STANDING, crate::stands::EYE_CROUCHED, USE_RAY_CLIENT_M);
        let puts: Vec<NodeId> = put_stands(&w, &stands, 0).iter().map(|s| s.node).collect();
        let back = Tree::grow(&w.grid, &puts, Reach::no_sprint(None), Some(&Backward::of(&w.grid)), Links::Proven);
        let from = w.grid.locate(Vec3::new(4.0, 0.0, 3.0), 0.3).unwrap();
        let forward = nearest_path(&w.grid, &[from], &puts, Reach::no_sprint(None), Links::Proven).unwrap();
        let backward = back.path(from).unwrap();
        assert!((forward.cost - backward.cost).abs() < 1e-3, "{} {}", forward.cost, backward.cost);
        assert_eq!(backward.nodes[0], from);
        assert!(puts.contains(backward.nodes.last().unwrap()) && backward.kinds.len() == backward.nodes.len());
    }

    /// The use ray meets a battery 1.5 m from the eye in the open, not one 2.0 m away, nor one behind a wall.
    #[test]
    fn the_use_ray_meets_a_battery_in_reach_and_in_sight() {
        let w = world(false);
        let eye = Vec3::new(4.0, 1.46, 1.0);
        assert!(battery_reach(&w.sight, eye, Vec3::new(4.0, 0.1, 0.0)).is_some());
        assert!(battery_reach(&w.sight, Vec3::new(4.0, 1.46, 2.0), Vec3::new(4.0, 0.1, 0.0)).is_none());
        let boxed = world(true);
        assert!(battery_reach(&boxed.sight, Vec3::new(4.0, 1.46, 1.6), Vec3::new(4.0, 0.1, 0.0)).is_none());
    }

    /// In the open yard both teams take the battery, walk in, carry it home and put it in: Ok, with stands and proven ways.
    /// Shut in a box of walls, a stand inside reaches it and no path does: NoWayIn. Out of every eye's reach: NoTakeStand.
    #[test]
    fn a_battery_in_the_open_is_ok_shut_in_has_no_way_and_high_has_no_stand() {
        let w = world(false);
        let stands = UseStands::build_ray(&w, crate::stands::EYE_STANDING, crate::stands::EYE_CROUCHED, USE_RAY_CLIENT_M);
        let c = Contract::build(&w, &stands, &INPUTS);
        assert_eq!(c.rows.len(), 2);
        for r in &c.rows {
            assert_eq!(r.verdict, Verdict::Ok, "{}", r.line(&w.grid));
            assert!(r.take.is_some() && r.put.is_some());
        }
        // Shut in walls: the floor inside the walls reaches it, no path does: NoWayIn, no failing link (no path at all).
        let shut = world(true);
        let c = Contract::build(&shut, &stands, &INPUTS);
        for r in &c.rows {
            assert_eq!(r.verdict, Verdict::NoWayIn, "{}", r.line(&shut.grid));
            assert_eq!(r.way_in, Way::Fails { at: None, why: WayFail::NoPath });
        }
        // A battery at rest elsewhere is asked the same.
        let rows = Contract::rows_for(&w, &stands, &INPUTS, Vec3::new(-4.0, 0.1, 0.0));
        assert!(rows.iter().all(|r| r.verdict == Verdict::Ok && r.spawner.is_none()), "{rows:?}");
        // One 3.5 m up is past every eye's use ray (2.04 m over the standing eye less the inner ball, 1.92 > 1.80): NoTakeStand.
        let rows = Contract::rows_for(&w, &stands, &INPUTS, Vec3::new(4.0, 3.5, 0.0));
        assert!(rows.iter().all(|r| r.verdict == Verdict::NoTakeStand && r.take.is_none()), "{rows:?}");
    }
}
