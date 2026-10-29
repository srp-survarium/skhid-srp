//! Crossing zones on all match on a bot's way: zones that never switch off
//! (`Hazard::switches`), such as a fire's soft damage or a swamp.
//!
//! The planner keeps off every zone's floors (`Tactics::allowed_from`). So
//! where only such a zone lay between a bot and its goal it had no way, and a
//! bot that fell into a patch such a zone rings stood there all match. On
//! Mamayev Kurgan TDM a bot holding the Cut spot fell 1.9 m onto 92 floors
//! ringed by a fire's soft damage of 0.04 a second, and its searches failed
//! for 100 s. Players walk through such a zone when the damage is small.
//!
//! After the motor's plain search (`mod.rs`):
//! - when it found no way, or a way much longer than the line
//!   (`stuck::may_be_long`) while the line crosses such a zone
//!   ([`wanted`]), the search runs again with those zones' floors open
//!   ([`crossable`]: zones the bot would get out of at their thinnest);
//! - that route is taken when the bot's health bears its walk ([`hurt`],
//!   `collect::bears`, the rule of a battery's crossing too), and in place of
//!   a long plain route only when the plain one is not worth it
//!   (`stuck::worth_round`);
//! - else, from a patch such zones ring (`Tactics::ringed`), the route is
//!   walked only to the first floor past the ring, toward the goal, when that
//!   leaves the bot at least `collect::CROSS_ABORT_LEFT` (the walk out of a
//!   battery's zone); from there it plans as ever.
//!
//! A route taken is a crossing for the motor (`Bot::crossing`): it is not
//! escaped from (`escaping`) nor shut ahead (`Tactics::route_shut_ahead`) on
//! those zones' floors.
//!
//! Best practice: the usual way is area costs in one search (Detour's
//! `dtQueryFilter::setAreaCost`, Unity's `NavMeshAgent.SetAreaCost`: a dear
//! area is still used when it is the only way). A second search is used
//! instead: the regions' graph is built once from the geometry (as HPA*'s
//! abstract graph), so prices that change with the zones would need a cost
//! hook at both levels and change every route; and health is a hard budget,
//! which a cost alone does not keep (a constrained shortest path: relax,
//! then check the path, as Carlyle, Royset and Wood). Every route a plain
//! search finds is the same as before.

use mapdata::{NodeId, Vec3};

use super::features::{Feature, Life};
use super::path::{self, Route};
use super::route_filter::RouteFilter;
use super::plays::collect::{bears, CROSS_ABORT_LEFT, CROSS_MPS, CROSS_STEP_M};
use super::stuck;
use super::tactics::Tactics;

/// A patch walled by zones on all match is at most this, m² (in floors,
/// [`ring_floors`]). GUESS: a room; the Mamayev Kurgan patch was 6 m²
/// (92 floors). A bigger one is a part of the map, where a bot's way out is
/// its goal's crossing or none.
const RING_M2: f32 = 100.0;

/// A crossing's search for a long plain route that kept the plain one is
/// not made again for this long, ms, to a goal this near, m
/// (`Bot::long_kept`). The bot walks the plain route meanwhile, and its
/// goal (a battery, a spot, a foe) mostly stays near: in the bench 2 in 3
/// crossing searches kept the plain route, on maps whose zones are too
/// wide to cross (Vostok Radar Station, Tarakanovsky Fort, School) nearly
/// all. GUESS: half a play's goal kept (`NO_WAY_KEEP_MS`), a room.
pub const LONG_RETRY_MS: u32 = 10_000;
pub const LONG_RETRY_M: f32 = 10.0;

/// [`RING_M2`] in floors of the grid.
fn ring_floors() -> usize {
    (RING_M2 / (mapdata::CELL * mapdata::CELL)) as usize
}

/// A crossing's search under way (`mod.rs` `Pending`): the plain search's
/// answer, taken if the crossing is not, and the health it is weighed for.
#[derive(Debug)]
pub struct Second {
    pub plain: Option<Route>,
    pub health: f32,
}

/// Whether a crossing's search lets a bot of `health` onto floor `node`:
/// only zones on all match mark it (`Hazards::only_fixed_at`), and a walk
/// across them at their thinnest (the marks' margin each side) leaves it at
/// least [`CROSS_ABORT_LEFT`], the walk out of a ring's test. Zones that kill
/// in a few metres (Vostok Radar Station's and Cologne's 1.2 a second) stay
/// shut, and so does the margin's band round them (the gate is the zone's full hit, not what a body there loses,
/// `Hazards::hurt_per_s`: a drift into a lethal zone is not crossed for); whether a route over the floors let through
/// is taken is [`choose`]'s.
pub fn crossable(tactics: &Tactics, node: NodeId, health: f32) -> bool {
    let hazards = &tactics.hazards;
    hazards.only_fixed_at(node) && {
        let thinnest = 2.0 * hazards.margin.max(CROSS_STEP_M);
        health - hazards.hit_per_s(node) * thinnest / CROSS_MPS >= CROSS_ABORT_LEFT
    }
}

/// Whether the plain search's answer `plain`, from floor `from` to floor
/// `to`, for a body of map side `side` (bound for their base if
/// `objective`), calls for a crossing's search: on a map with zones on all
/// match, both ends in one piece of the grid, and
/// - no way, where the regions say only a crossing of such zones a bot of
///   `health` may cross gets there (`Tactics::only_across_fixed`): a goal no
///   way leads to for another reason (another side's spawn area, a zone that
///   switches, a floor apart) is not searched again;
/// - or a way much longer than the line (`stuck::may_be_long`) while the
///   line crosses a floor a bot of `health` may cross.
#[allow(clippy::too_many_arguments)]
pub fn wanted(
    tactics: &Tactics,
    plain: Option<&Route>,
    from: NodeId,
    to: NodeId,
    side: Option<i32>,
    objective: bool,
    health: f32,
) -> bool {
    let grid = &tactics.world.grid;
    if !tactics.hazards.zones.iter().any(|zone| !zone.switches()) || !grid.may_reach(from, to) {
        return false;
    }
    let (a, b) = (grid.position(from), grid.position(to));
    match plain {
        None => tactics.only_across_fixed(from, to, side, objective, |middle| {
            crossable(tactics, middle, health)
        }),
        Some(route) => {
            let long =
                stuck::may_be_long(route.length(), a, b) && line_crosses(tactics, a, b, health);
            if long {
                count(Count::Long);
            }
            long
        }
    }
}

/// Whether the line from `a` to `b` passes over a floor a bot of `health`
/// may cross ([`crossable`]), looked at every [`CROSS_STEP_M`].
fn line_crosses(tactics: &Tactics, a: Vec3, b: Vec3, health: f32) -> bool {
    let grid = &tactics.world.grid;
    let n = ((b - a).length() / CROSS_STEP_M).ceil().max(1.0) as usize;
    (0..=n).any(|k| {
        grid.locate(a + (b - a) * (k as f32 / n as f32), 1.5)
            .is_some_and(|node| crossable(tactics, node, health))
    })
}

/// The health a body loses walking `route` from `from_m` to `to_m` along it,
/// at [`CROSS_MPS`]: the zones on all match on its floors, looked at every
/// [`CROSS_STEP_M`] (a zone that switches is crossed only while off).
/// At the zones' full hit on every floor they mark, the margin's band included: the motor's own crossings of any goal
/// are weighed pool-blind (the body's health), so a band priced at the drift's share let the bench's bots into zones
/// they died in (Vector Laboratory, zone deaths 10 -> 15, `hurt_by_shape` alone); only a battery's crossing
/// (`collect`) takes the shape's pricing.
pub fn hurt(tactics: &Tactics, route: &Route, from_m: f32, to_m: f32) -> f32 {
    let grid = &tactics.world.grid;
    let d = (to_m - from_m).max(0.0);
    let n = (d / CROSS_STEP_M).ceil().max(1.0) as usize;
    let ds = d / n as f32;
    (0..n)
        .map(|i| {
            let point = route.floor_point(from_m + (i as f32 + 0.5) * ds);
            grid.locate(point, 0.6)
                .filter(|&node| tactics.hazards.only_fixed_at(node))
                .map_or(0.0, |node| tactics.hazards.hit_per_s(node))
                * ds
                / CROSS_MPS
        })
        .sum()
}

/// A crossing's search from floor `from` to floor `to` for a bot of
/// `health` ([`Tactics::start_crossing`] with [`crossable`]), counted. It
/// keeps off the hard part of `filter` (the landings of jumps the body gave
/// up), not its spots.
#[allow(clippy::too_many_arguments)]
#[allow(clippy::result_large_err)]
pub fn start(
    tactics: &Tactics,
    from: NodeId,
    to: NodeId,
    time: u32,
    side: Option<i32>,
    objective: bool,
    health: f32,
    filter: &RouteFilter,
    floors: &mut usize,
) -> Result<Option<Route>, path::Planning> {
    count(Count::Searched);
    #[cfg(test)]
    let before = path::searches().expanded;
    let route = tactics.start_crossing(
        from,
        to,
        time,
        side,
        objective,
        |n| crossable(tactics, n, health) && filter.hard_open(&tactics.world.grid, n),
        floors,
    );
    #[cfg(test)]
    expanded(path::searches().expanded - before);
    route
}

/// A crossing's search ([`start`]) goes on.
#[allow(clippy::too_many_arguments)]
pub fn resume(
    tactics: &Tactics,
    planning: &mut path::Planning,
    from: NodeId,
    time: u32,
    side: Option<i32>,
    objective: bool,
    health: f32,
    filter: &RouteFilter,
    floors: &mut usize,
) -> Option<Option<Route>> {
    #[cfg(test)]
    let before = path::searches().expanded;
    let route = tactics.resume_crossing(
        planning,
        from,
        time,
        side,
        objective,
        |n| crossable(tactics, n, health) && filter.hard_open(&tactics.world.grid, n),
        floors,
    );
    #[cfg(test)]
    expanded(path::searches().expanded - before);
    route
}

/// A crossing to a goal is begun only at this health or more: the walk
/// out of a ring is not held to it. The bench (seeds 1-8, zone damage on,
/// with the abort): on Mamayev Kurgan's batteries 97 of the 115 stretches
/// in zones that left a bot under 0.6 health began under 0.7 (C: 10 such
/// stretches in all); 23 began within 3 s of an enemy's hit. GUESS: at
/// 0.7 the dearest crossing borne (half the health) leaves 0.35, over the
/// abort's `CROSS_ABORT_LEFT`.
pub const START_HEALTH: f32 = 0.7;

/// Whether a crossing walked `walked` m along `route` still leaves a bot
/// of `health` [`CROSS_ABORT_LEFT`] over the rest of its zones at
/// [`CROSS_MPS`], a carrier's crossing's abort (`collect`). Hurt in a fight
/// or held up in the zone, it stops crossing (`mod.rs`), and the escape
/// walks it out (`Tactics::escape`). The bench (seeds 1-8, zone damage on):
/// without it, on Mamayev Kurgan's batteries, 116 stretches in zones left a
/// bot under 0.6 health (10 before crossings), and bots died 10 s into a
/// zone of 0.25 a second.
pub fn still_borne(tactics: &Tactics, route: &Route, walked: f32, health: f32) -> bool {
    health - hurt(tactics, route, walked, route.length()) >= CROSS_ABORT_LEFT
}

/// What the motor takes after a crossing's search from floor `from`: the
/// route, whether it is a crossing, and whether it is the plain route kept
/// (a long one: not searched again soon, [`LONG_RETRY_MS`]). `plain` is the
/// plain search's answer, `crossed` the crossing's.
#[allow(clippy::too_many_arguments)]
pub fn choose(
    tactics: &Tactics,
    from: NodeId,
    plain: Option<Route>,
    crossed: Option<Route>,
    health: f32,
    time: u32,
    side: Option<i32>,
    objective: bool,
) -> (Option<Route>, bool, bool) {
    let Some(crossed) = crossed else {
        count(Count::Refused);
        let kept = plain.is_some();
        return (plain, false, kept);
    };
    let cost = hurt(tactics, &crossed, 0.0, crossed.length());
    let borne = cost <= 0.0 || (health >= START_HEALTH && bears(cost, health));
    match plain {
        Some(plain) if stuck::worth_round(plain.length(), crossed.length()) || !borne => {
            count(Count::Refused);
            (Some(plain), false, true)
        }
        _ if borne => {
            count(Count::Taken);
            (Some(crossed), cost > 0.0, false)
        }
        _ => match out_of_ring(tactics, from, &crossed, health, time, side, objective) {
            Some(out) => {
                count(Count::RingWalk);
                (Some(out), true, false)
            }
            None => {
                count(Count::Refused);
                (None, false, false)
            }
        },
    }
}

/// The walk out of a patch ringed by zones on all match, from floor `from`
/// ([`Tactics::ringed`]): `crossed` up to the first floor past the ring that
/// it may walk on with every floor next to it (clear of the zone's edge, as
/// `Tactics::escape`'s ways out), if that walk leaves a bot of `health` at
/// least [`CROSS_ABORT_LEFT`].
fn out_of_ring(
    tactics: &Tactics,
    from: NodeId,
    crossed: &Route,
    health: f32,
    time: u32,
    side: Option<i32>,
    objective: bool,
) -> Option<Route> {
    if !tactics.ringed(from, time, side, objective, ring_floors()) {
        return None;
    }
    let grid = &tactics.world.grid;
    let ok = |n: NodeId| tactics.allowed(n, time, side, objective);
    let mut in_zone = false;
    let mut s = 0.0;
    let exit = loop {
        if s > crossed.length() {
            return None;
        }
        let node = grid.locate(crossed.floor_point(s), 0.6);
        if let Some(node) = node {
            if tactics.hazards.only_fixed_at(node) {
                in_zone = true;
            } else if in_zone && ok(node) && grid.links(node).all(|(b, _)| ok(b)) {
                break node;
            }
        }
        s += CROSS_STEP_M;
    };
    if health - hurt(tactics, crossed, 0.0, s) < CROSS_ABORT_LEFT {
        return None;
    }
    // A few metres: searched to its end at once.
    let mut iters = usize::MAX;
    start(tactics, from, exit, time, side, objective, health, &RouteFilter::open(), &mut iters)
        .ok()
        .flatten()
        .or_else(|| path::plan(grid, from, exit))
}

/// What became of the crossings' searches, for the bench's counts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Count {
    /// A crossing's search was made (started).
    Searched,
    /// Its route was taken to the goal.
    Taken,
    /// Its route was walked out of a ring only.
    RingWalk,
    /// None, or one not borne or not worth it: the plain answer stands.
    Refused,
    /// Floors the searches expanded ([`expanded`]).
    Floors,
    /// A search wanted for a long plain route (not for none).
    Long,
    /// A crossing stopped on the way ([`still_borne`]).
    Aborted,
}

#[cfg(test)]
thread_local! {
    static COUNTS: std::cell::Cell<[u64; 7]> = const { std::cell::Cell::new([0; 7]) };
}

#[cfg(test)]
thread_local! {
    static LAST: std::cell::Cell<Option<Count>> = const { std::cell::Cell::new(None) };
}

/// Whether the last count was a ring's walk out (the bench's `why`).
#[cfg(test)]
pub fn last_ring() -> bool {
    LAST.with(|l| l.get()) == Some(Count::RingWalk)
}

/// A crossing's search counted ([`Count`]).
#[cfg(test)]
pub fn count(what: Count) {
    LAST.with(|l| l.set(Some(what)));
    COUNTS.with(|c| {
        let mut n = c.get();
        n[what as usize] += 1;
        c.set(n);
    });
}

#[cfg(not(test))]
pub fn count(_: Count) {}

/// Floors the crossings' searches expanded, counted after each slice.
#[cfg(test)]
pub fn expanded(floors: u64) {
    COUNTS.with(|c| {
        let mut n = c.get();
        n[Count::Floors as usize] += floors;
        c.set(n);
    });
}

#[cfg(not(test))]
pub fn expanded(_: u64) {}

/// The counts so far, by [`Count`]: searched, taken, ring walks, refused,
/// floors expanded, wanted for a long route, stopped on the way.
#[cfg(test)]
pub fn counts() -> [u64; 7] {
    COUNTS.with(|c| c.get())
}

/// A bot's crossing state, by feature ([`super::features`]): the answer of its route search that went on over
/// several polls ([`Pending`]) and the goal and the plain route it kept.
#[derive(Default)]
pub struct BotState {
    /// The goal, the route or none, and whether the route is a crossing. Taken at its next tick, and used if it
    /// still heads there.
    pub found: Option<(Vec3, Option<Route>, bool)>,
    /// The goal its route crosses zones on all match to: while it heads there, those zones' floors are not
    /// escaped from nor shut its route ahead.
    pub goal: Option<Vec3>,
    /// A goal a crossing's search for a long plain route kept the plain one to, and until when such a search is
    /// not made again to a goal near it (`LONG_RETRY_MS`).
    pub long_kept: Option<(Vec3, u32)>,
    /// The armour it wears against a zone's hits (`zone_cost.rs`), set at the match's start from its kit; it
    /// outlasts a life.
    pub armour: std::sync::Arc<super::zone_cost::ZoneArmour>,
}

impl Feature for BotState {
    fn new_life(&mut self, _life: &Life) {
        let armour = std::mem::take(&mut self.armour);
        *self = Self {
            armour,
            ..Self::default()
        };
    }
}

#[cfg(test)]
mod tests {
    use mapdata::{Rotation, Shape, ShapeKind, Vec3, Zone, ZoneKind};

    use super::*;
    use crate::v026g::bots::tactics::tests::field_with;

    /// A zone on all match hurting `per_s` a second: a box at `centre`, its
    /// half size across x and z `half`.
    fn zone(name: &str, per_s: f32, centre: (f32, f32), half: (f32, f32)) -> Zone {
        Zone {
            kind: ZoneKind::Damage,
            name: name.into(),
            team: -1,
            position: Vec3::new(centre.0, 0.0, centre.1),
            damage_type: "fire".into(),
            damage: Some(mapdata::Damage {
                max_hit: per_s / 2.0,
                parts: 2,
                continuous: true,
                ..mapdata::Damage::default()
            }),
            shapes: vec![Shape {
                kind: ShapeKind::Box,
                center: Vec3::new(centre.0, 1.0, centre.1),
                rotation: Rotation::IDENTITY,
                half: Vec3::new(half.0, 1.0, half.1),
            }],
        }
    }

    /// The field (`tactics::tests::field`: a wall across x = 40 from z = 0 to
    /// 25) with the way round it, past z = 25, under a zone of `gap` a
    /// second, and a 6 x 6 m patch at (20, 10) ringed by zones of `ring` a
    /// second (none at 0).
    fn tactics(gap: f32, ring: f32) -> Tactics {
        let world = field_with(|map| {
            if gap > 0.0 {
                map.zones.push(zone("gap", gap, (40.0, 32.5), (2.0, 7.5)));
            }
            if ring > 0.0 {
                map.zones.push(zone("west", ring, (16.0, 10.0), (1.0, 5.0)));
                map.zones.push(zone("east", ring, (24.0, 10.0), (1.0, 5.0)));
                map.zones.push(zone("south", ring, (20.0, 6.0), (5.0, 1.0)));
                map.zones.push(zone("north", ring, (20.0, 14.0), (5.0, 1.0)));
            }
        });
        Tactics::new(world, 1.0)
    }

    /// The plain search, then the crossing's, as the motor makes them.
    fn plan(tactics: &Tactics, from: Vec3, to: Vec3, health: f32) -> (Option<Route>, bool) {
        let grid = &tactics.world.grid;
        let (a, b) = (grid.locate(from, 1.5).unwrap(), grid.locate(to, 1.5).unwrap());
        let plain = tactics.route_nodes(a, b, 0, Some(0), false, &RouteFilter::open());
        if !wanted(tactics, plain.as_ref(), a, b, Some(0), false, health) {
            return (plain, false);
        }
        let mut iters = usize::MAX;
        let Ok(crossed) = start(tactics, a, b, 0, Some(0), false, health, &RouteFilter::open(), &mut iters) else {
            panic!("an unbounded search ends");
        };
        let (route, crossing, _) = choose(tactics, a, plain, crossed, health, 0, Some(0), false);
        (route, crossing)
    }

    const WEST: Vec3 = Vec3::new(30.0, 0.0, 30.0);
    const EAST: Vec3 = Vec3::new(50.0, 0.0, 10.0);

    /// The only way past the wall under a zone on all match: crossed when
    /// the bot's health bears it (`bears`, a battery's rule), else no way as
    /// before.
    #[test]
    fn the_only_way_under_a_weak_zone_on_all_match_is_crossed_when_health_bears_it() {
        let weak = tactics(0.04, 0.0);
        let (route, crossing) = plan(&weak, WEST, EAST, 1.0);
        let route = route.expect("a crossing");
        assert!(crossing);
        let cost = hurt(&weak, &route, 0.0, route.length());
        assert!(cost > 0.0 && cost < 0.2, "{cost}");
        // Too hurt to bear it: no way, as before.
        let (route, crossing) = plan(&weak, WEST, EAST, 0.42);
        assert!(route.is_none() && !crossing);
        // Hurt (under `START_HEALTH`), though it would bear it: not begun.
        let (route, crossing) = plan(&weak, WEST, EAST, START_HEALTH - 0.05);
        assert!(route.is_none() && !crossing);
        // A zone that would take more than half its health: no way.
        let harsh = tactics(0.5, 0.0);
        assert!(plan(&harsh, WEST, EAST, 1.0).0.is_none());
        // One that kills at its thinnest is not even searched over.
        let deadly = tactics(1.2, 0.0);
        let grid = &deadly.world.grid;
        let mid = grid.locate(Vec3::new(40.0, 0.0, 32.5), 1.5).unwrap();
        assert!(!crossable(&deadly, mid, 1.0));
    }

    /// A crossing hurt on the way is stopped once the rest of it would leave
    /// the bot under `CROSS_ABORT_LEFT`; past the zone it is borne.
    #[test]
    fn a_crossing_is_stopped_once_its_rest_would_leave_too_little_health() {
        let weak = tactics(0.04, 0.0);
        let route = plan(&weak, WEST, EAST, 1.0).0.expect("a crossing");
        let cost = hurt(&weak, &route, 0.0, route.length());
        assert!(still_borne(&weak, &route, 0.0, 1.0));
        let low = CROSS_ABORT_LEFT + cost / 2.0;
        assert!(!still_borne(&weak, &route, 0.0, low));
        assert!(still_borne(&weak, &route, route.length(), low));
    }

    /// With no zone, the plain route stands and no second search is made.
    #[test]
    fn a_plain_route_is_kept_and_no_crossing_is_searched_where_no_zone_is_on_all_match() {
        let plain = tactics(0.0, 0.0);
        let grid = &plain.world.grid;
        let (a, b) = (grid.locate(WEST, 1.5).unwrap(), grid.locate(EAST, 1.5).unwrap());
        let route = plain.route_nodes(a, b, 0, Some(0), false, &RouteFilter::open());
        assert!(route.is_some());
        assert!(!wanted(&plain, route.as_ref(), a, b, Some(0), false, 1.0));
    }

    /// A bot in a patch ringed by a weak zone on all match (fallen in): out
    /// by its crossing to the goal when it bears it; else, too hurt for that,
    /// only to the first floor past the ring toward the goal, when that walk
    /// leaves it `CROSS_ABORT_LEFT`; from there it plans as ever.
    #[test]
    fn a_bot_ringed_by_a_weak_zone_on_all_match_walks_out_toward_its_goal() {
        let ringed = tactics(0.0, 0.04);
        let inside = Vec3::new(20.0, 0.0, 10.0);
        let grid = &ringed.world.grid;
        let from = grid.locate(inside, 1.5).unwrap();
        assert!(ringed.ringed(from, 0, Some(0), false, ring_floors()));
        let (route, crossing) = plan(&ringed, inside, EAST, 1.0);
        assert!(crossing);
        let route = route.expect("a crossing");
        assert!(route.end().distance_xz(EAST) < 1.0, "{:?}", route.end());
        // Too hurt to bear the whole walk: out of the ring only, east.
        let (route, crossing) = plan(&ringed, inside, EAST, 0.41);
        assert!(crossing);
        let out = route.expect("a way out of the ring").end();
        assert!(out.x > 25.0 && out.x < 28.0, "{out:?}");
        let out = grid.locate(out, 0.6).unwrap();
        assert!(ringed.allowed(out, 0, Some(0), false));
        // Too hurt even for that: it stays, as before.
        assert!(plan(&ringed, inside, EAST, 0.26).0.is_none());
        // Off the patch the field is no ring.
        let west = grid.locate(WEST, 1.5).unwrap();
        assert!(!ringed.ringed(west, 0, Some(0), false, ring_floors()));
    }
}
