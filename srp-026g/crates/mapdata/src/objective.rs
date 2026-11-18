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
use crate::grid::{Aim, LinkKind, NodeId, OffLink, Path, Reach, Riser, Share, SprintReach, WalkGrid};
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
        let spawners: Vec<Vec3> = world.map.battery_spawners.iter().map(|s| s.item).collect();
        let rows = spawners.iter().enumerate().flat_map(|(i, &at)| Self::rows_at(world, stands, inputs, Some(i), at)).collect();
        Self { rows }
    }

    /// The rows of a battery resting at `at` (dropped, not at a spawner), one per team.
    pub fn rows_for(world: &World, stands: &UseStands, inputs: &ContractInputs, at: Vec3) -> Vec<Row> {
        Self::rows_at(world, stands, inputs, None, at)
    }

    fn rows_at(world: &World, stands: &UseStands, inputs: &ContractInputs, spawner: Option<usize>, at: Vec3) -> Vec<Row> {
        let mut teams: Vec<i32> = world.map.containers.iter().map(|c| c.team).collect();
        teams.sort_unstable();
        teams.dedup();
        let takes = take_stands(&world.grid, &world.sight, inputs, at);
        teams.into_iter().map(|team| row(world, stands, spawner, at, team, &takes)).collect()
    }
}

/// The take stands of a battery at `at`: every floor within [`STAND_SEARCH_M`] across, standing where a standing body fits and
/// crouched, whose eye's use ray meets the battery ([`battery_reach`]) with the look down within the view's stop.
fn take_stands(grid: &WalkGrid, sight: &RayTree, inputs: &ContractInputs, at: Vec3) -> Vec<Stand> {
    let mut out = Vec::new();
    for node in 0..grid.len() as NodeId {
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
        (SprintReach::Cannot { .. }, Takeoff::Climb, Some((lead, share))) if l.aim == Aim::STRAIGHT => {
            Some((Takeoff::Press(lead), share, l.aim))
        }
        (SprintReach::Cannot { .. }, Takeoff::Climb, _) => None,
        (SprintReach::Cannot { .. }, Takeoff::Sprint(_) | Takeoff::SprintHeld(_), walk) => {
            walk.map(|(lead, share)| (Takeoff::Press(lead), share, l.aim))
        }
        _ => Some((l.press, l.share, l.aim)),
    }
}

/// Whether the path's links are each the client's own for a body of `reach`: every off-mesh link and every riser (a Walk link
/// jumped up, [`WalkGrid::riser`]) by a flown press whose own flight lands ([`Share::window`]), no plain Jump link (no press
/// stored for it); the first that is not, as the way's failure. Walks and drops are the build's own (a drop judged by where
/// its flight rests, F2).
fn judge(grid: &WalkGrid, path: &Path, reach: Reach) -> Way {
    let mut min_share: Option<u8> = None;
    let mut sprint = false;
    let mut flown = |share: Share| {
        if let Some(n) = share.flown() {
            min_share = Some(min_share.map_or(n, |m| m.min(n)));
        }
    };
    for (pair, &kind) in path.nodes.windows(2).zip(&path.kinds[1..]) {
        let (a, b) = (pair[0], pair[1]);
        let fail = |why| Way::Fails { at: Some((a, b, kind)), why };
        match kind {
            LinkKind::GapJump | LinkKind::GapDrop => {
                let Some(l) = grid.off_link(a, b) else { return fail(WayFail::Unproven) };
                let Some((press, share, aim)) = played(&l, reach) else { return fail(WayFail::NoWalkingPress) };
                if share.flown().is_none() {
                    return fail(WayFail::Unflown);
                }
                if share.window(press, aim).is_none() {
                    return fail(WayFail::PressMisses);
                }
                flown(share);
                sprint |= press.sprint_run_m().is_some();
            }
            LinkKind::Walk => match grid.riser(a, b) {
                None => {}
                Some(Riser::Pressed { press, share }) => {
                    let (press, share) = match (reach.sprint, press) {
                        (SprintReach::Cannot { .. }, Takeoff::Sprint(_) | Takeoff::SprintHeld(_)) => {
                            return fail(WayFail::NoWalkingPress);
                        }
                        _ => (press, share),
                    };
                    if share.window(press, Aim::STRAIGHT).is_none() {
                        return fail(WayFail::PressMisses);
                    }
                    flown(share);
                    sprint |= press.sprint_run_m().is_some();
                }
                Some(Riser::NoPress) => return fail(WayFail::PressMisses),
            },
            LinkKind::Jump => return fail(WayFail::PlainJump),
            LinkKind::Drop => {}
        }
    }
    Way::Proven { cost: path.cost, min_share, sprint }
}

/// The cheapest way from any of `from` to any of `to` for `reach`, judged ([`judge`]); with the goal it reached.
fn best_way(grid: &WalkGrid, from: &[NodeId], to: &[NodeId], reach: Reach) -> (Way, Option<NodeId>) {
    let mut best: Option<(Path, NodeId)> = None;
    for &a in from {
        for &b in to {
            if let Some(path) = grid.path_where_reaching(a, b, |_| true, reach)
                && best.as_ref().is_none_or(|(p, _)| path.cost < p.cost)
            {
                best = Some((path, b));
            }
        }
    }
    match best {
        Some((path, b)) => (judge(grid, &path, reach), Some(b)),
        None => (Way::Fails { at: None, why: WayFail::NoPath }, None),
    }
}

fn row(world: &World, stands: &UseStands, spawner: Option<usize>, at: Vec3, team: i32, takes: &[Stand]) -> Row {
    let grid = &world.grid;
    let spawns: Vec<NodeId> =
        world.map.spawns.iter().filter(|s| s.team == team).filter_map(|s| grid.locate(s.position, SPAWN_FLOOR_M)).collect();
    let take_nodes: Vec<NodeId> = takes.iter().map(|s| s.node).collect();
    let (way_in, reached) = if take_nodes.is_empty() {
        (Way::Fails { at: None, why: WayFail::NoPath }, None)
    } else {
        best_way(grid, &spawns, &take_nodes, Reach::FULL)
    };
    let take = reached.and_then(|n| takes.iter().find(|s| s.node == n).copied()).or_else(|| takes.first().copied());
    let puts: Vec<Stand> = world
        .map
        .containers
        .iter()
        .filter(|c| c.team == team)
        .flat_map(|c| stands.of(c.id))
        .filter_map(|s| grid.locate(s.feet, 0.3).map(|node| Stand { node, crouch: !s.standing, look_down_rad: -s.pitch }))
        .collect();
    let put_nodes: Vec<NodeId> = puts.iter().map(|s| s.node).collect();
    let (way_home, put_node) = match take {
        Some(t) if !put_nodes.is_empty() => best_way(grid, &[t.node], &put_nodes, Reach::no_sprint(None)),
        _ => (Way::Fails { at: None, why: WayFail::NoPath }, None),
    };
    let put = put_node.and_then(|n| puts.iter().find(|s| s.node == n).copied()).or_else(|| puts.first().copied());
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
    /// Shut in a box of walls, no stand reaches it: NoTakeStand.
    #[test]
    fn a_battery_in_the_open_is_ok_and_one_shut_in_has_no_stand() {
        let w = world(false);
        let stands = UseStands::build_ray(&w, crate::stands::EYE_STANDING, crate::stands::EYE_CROUCHED, USE_RAY_CLIENT_M);
        let c = Contract::build(&w, &stands, &INPUTS);
        assert_eq!(c.rows.len(), 2);
        for r in &c.rows {
            assert_eq!(r.verdict, Verdict::Ok, "{}", r.line(&w.grid));
            assert!(r.take.is_some() && r.put.is_some());
        }
        let shut = world(true);
        let c = Contract::build(&shut, &stands, &INPUTS);
        assert!(c.rows.iter().all(|r| r.verdict == Verdict::NoTakeStand), "{:?}", c.rows);
        // A battery at rest elsewhere is asked the same.
        let rows = Contract::rows_for(&w, &stands, &INPUTS, Vec3::new(-4.0, 0.1, 0.0));
        assert!(rows.iter().all(|r| r.verdict == Verdict::Ok && r.spawner.is_none()));
    }
}
