//! [lane draw] A second lane for a long walk, and the draw between the two (the lane-spreading root cause, 2026-11-26).
//!
//! Before it, every play but Roam walked the one least-cost way of the drive's search (`RouteFilter::weight` has no per-bot or
//! teammate term), so between the same start and end a team's bots, and its lives, took the one lane: top lane share 0.878 on the
//! chain bench v47 against the live humans' 0.846, and a second lane within +15 % of the cheapest walked in 4-5 % of the groups of
//! long legs though the grids have one in 46 % of them (the C2 probe).
//!
//! What it does, at the start of a walk to a new goal (the drive's search when the bot holds no lane for the goal):
//! - **The lanes, on the regions' graph** (HPA*'s abstract level: the regions of 8 m blocks and their links, a few thousand on a
//!   map). L1 is the least-cost way between the start's and the goal's regions, its cost the links' (metres between the regions'
//!   middles plus the link kinds' penalties), counted between the walk's own ends ([`RegionWay::cost_between`]: the end regions'
//!   middles lie up to half a block off them). When it is longer than [`LaneDrawTuning::long_m`], L2 is the least-cost way with the
//!   regions near L1 shut: a region whose middle lies within `away_m` of L1's, but within `ends_m` of the walk's ends, where both
//!   lanes start and end (the C2 probe's rule, by region). Both see only the regions the plain search may enter (the hazards,
//!   the spawn areas, the filter's regions).
//! - **The floors' search, once.** L1 drawn: the plain search, as before. L2 drawn: the plain search kept to L2's room (its
//!   regions and the open ones next to them, `RouteFilter::keeping_to`, as K1 keeps its weighed search to the costed corridor).
//!   So a walk costs one floors' search either way: a second floors' search would not fit the poll's floors (10,000 a team; the
//!   probe's L2 floor search took 19,000 at the median), and no tick pays for two. The room's search has the plain search's own
//!   filter: its stuck spots, traps, shut landings, zones and reach all hold, so L2 is a way the plain search would take were L1
//!   shut. No way in the room (a region link the body's reach refuses): the plain search, and L1 for the goal.
//! - **The true cost's cap.** The room's route is walked only when its length is within `band` times L1's cost on the regions
//!   ([`Room::cap_m`]; between the walk's ends the regions' L1 cost is the floors' route's within -1.1 % and +4.1 % at the
//!   quartiles, -6.5 % and +7.4 % at p5 and p95, on the probe's 146 long walks, against -3.4 % and +3.6 %, -9.2 % and +9.8 %
//!   counted from the end regions' middles); over it, the plain search and L1 (a nav line says so). On the probe 5 of 49 L2s in
//!   the band are over their cap (8 of 48 from the middles; 1 of them in the band on the floors, 4 before), and the routes
//!   walked are at most +15.6 % on the floors' L1.
//! - **Danger wins.** L2 is a candidate only within `band` of L1's cost and, when the bot knows a held shooter's lanes, through no
//!   more of their covered regions than L1 (metres of links into covered regions); the drive's detour round a lane then runs on
//!   whichever was drawn, as on any route. A replan for danger (a held shooter's lane ahead, a zone shut ahead, a crossing given
//!   up) is the plain search, and the hold turns to L1 for the goal.
//! - **The draw.** Weights as Roam's `draw_route`: L1 1, L2 its place in the band (1 at L1's cost, 0 at the band's edge: a dearer
//!   lane is drawn less, and one at the cap no more than one past it), each times Roam's `friend_factor` per teammate on that lane
//!   now (a teammate's long walk under way) or lately (one ended within Roam's `fresh_s`). The draw's number is the life's own for
//!   the walk's 15 m start and end cells (`Bot::life_key`): the same walk in one life draws the same, the next life draws anew.
//! - **The walk keeps its lane.** The decision is held for the goal ([`LaneHold`]): a replan for a goal moved within `away_m` keeps
//!   L2's shut regions (its way searched again on the regions from where the bot is) or L1, and draws nothing.
//! - **The trace.** One nav line a long walk ([`line`]): the play, the goal, C1, C2 and their ratio, the teammates on each lane, the
//!   odds and the number drawn, the lane walked (rank 1 or 2) and its cost; or why there is one lane only.
//!
//! Roam keeps its human-route draw (its walks are not drawn here), and a carrier's walk home is a race (not drawn).
//!
//! The values ([`LaneDrawTuning`]) are measures, not fits: `long_m` is the lane test's long leg; `away_m` and `ends_m` the C2
//! probe's; `band` where the live humans' lanes off their cheapest thin out (per 5 % of cost: 870, 658, 529 legs to +15 %, then 295
//! and about 300 a 5 % on to +30 %; 27,255 legs, 2,712 recordings). The teammate weight and window are Roam's own.

use std::sync::Arc;

use mapdata::{NodeId, Vec3};

use super::tactics::Tactics;
use super::tuning::LaneDrawTuning;
use super::view::Place;

/// The lane test's cell, m: the draw's number is the life's own for the walk's start and end cells.
const CELL_M: f32 = 15.0;

/// A way on the regions' graph: its regions in order, from the start's to the goal's, each with the cost of the link into it (the
/// start's 0), and its cost.
#[derive(Clone, Debug, PartialEq)]
pub struct RegionWay {
    pub regions: Vec<(u32, f32)>,
    pub cost: f32,
}

impl RegionWay {
    /// The middles of its regions, in order.
    pub fn points(&self, tactics: &Tactics) -> Vec<Vec3> {
        let (grid, regions) = (&tactics.world.grid, tactics.regions());
        self.regions.iter().map(|&(r, _)| grid.position(regions.middle(r))).collect()
    }

    /// Its cost from the walk's own ends `from` and `to` rather than from its end regions' middles: the geometry of its first and last
    /// links (middle to middle) replaced by the legs from `from` to the second region's middle and from the last but one's to `to`, the
    /// links' kind penalties kept (one region: `from` to `to`; one link: its geometry `from` to `to`). The regions' cost counts the end
    /// regions' middles, up to half a block off the walk's ends at each; the floors' route starts and ends at the ends.
    pub fn cost_between(&self, tactics: &Tactics, from: Vec3, to: Vec3) -> f32 {
        let (grid, regions) = (&tactics.world.grid, tactics.regions());
        let at = |k: usize| grid.position(regions.middle(self.regions[k].0));
        let leg = |a: Vec3, b: Vec3| (b - a).length();
        let n = self.regions.len();
        match n {
            0 => 0.0,
            1 => leg(from, to),
            2 => self.cost - leg(at(0), at(1)) + leg(from, to),
            _ => self.cost - leg(at(0), at(1)) + leg(from, at(1)) - leg(at(n - 2), at(n - 1)) + leg(at(n - 2), to),
        }
    }

    /// Metres of its links into regions `covered` gives true for.
    pub fn into_covered(&self, covered: impl Fn(u32) -> bool) -> f32 {
        self.regions.iter().filter(|&&(r, _)| covered(r)).map(|&(_, step)| step).sum()
    }
}

/// The least-cost way between regions `from` and `to` (A*: the links' costs, the middles' distance as the estimate) through the
/// regions `open` gives true for (the goal's always). None with no way.
pub fn region_way(tactics: &Tactics, from: u32, to: u32, open: impl Fn(u32) -> bool) -> Option<RegionWay> {
    use std::cmp::Reverse;
    use std::collections::BinaryHeap;
    let (grid, regions) = (&tactics.world.grid, tactics.regions());
    let n = regions.len();
    if from as usize >= n || to as usize >= n {
        return None;
    }
    let goal = grid.position(regions.middle(to));
    let h = |r: u32| grid.position(regions.middle(r)).distance_xz(goal);
    let mut cost = vec![f32::INFINITY; n];
    let mut parent: Vec<Option<(u32, f32)>> = vec![None; n];
    let mut closed = vec![false; n];
    let mut queue = BinaryHeap::new();
    // Ordered by the bits of a non-negative f32, which order as its value.
    let key = |c: f32| Reverse(c.max(0.0).to_bits());
    cost[from as usize] = 0.0;
    queue.push((key(h(from)), from));
    while let Some((_, u)) = queue.pop() {
        if std::mem::replace(&mut closed[u as usize], true) {
            continue;
        }
        if u == to {
            let mut way = vec![];
            let mut at = to;
            while let Some((up, step)) = parent[at as usize] {
                way.push((at, step));
                at = up;
            }
            way.push((from, 0.0));
            way.reverse();
            return Some(RegionWay { regions: way, cost: cost[to as usize] });
        }
        let cu = cost[u as usize];
        for &(v, step) in regions.out(u) {
            if closed[v as usize] || (v != to && !open(v)) {
                continue;
            }
            let c = cu + step;
            if c < cost[v as usize] {
                cost[v as usize] = c;
                parent[v as usize] = Some((u, step));
                queue.push((key(c + h(v)), v));
            }
        }
    }
    None
}

/// The regions the second lane's way keeps out of (by region): those whose middle lies within `away_m` of a point of the first lane
/// `first` and `ends_m` or more from both of the walk's `ends`. None when no such point is left (the walk is all ends).
pub fn shut_near(tactics: &Tactics, first: &[Vec3], ends: [Vec3; 2], set: &LaneDrawTuning) -> Option<Arc<Vec<bool>>> {
    let (away, ends_m) = (set.away_m.get(), set.ends_m.get());
    let off_ends = |p: Vec3| ends.iter().all(|e| p.distance_xz(*e) >= ends_m);
    let middle: Vec<Vec3> = first.iter().copied().filter(|&p| off_ends(p)).collect();
    if middle.is_empty() {
        return None;
    }
    // The middle points by `away` square: a region's middle is tested against the points of its square and the eight round it.
    let key = |p: Vec3| ((p.x / away).floor() as i32, (p.z / away).floor() as i32);
    let mut squares: std::collections::HashMap<(i32, i32), Vec<Vec3>> = std::collections::HashMap::new();
    for &p in &middle {
        squares.entry(key(p)).or_default().push(p);
    }
    let (grid, regions) = (&tactics.world.grid, tactics.regions());
    let shut = (0..regions.len() as u32)
        .map(|r| {
            let m = grid.position(regions.middle(r));
            let (i, k) = key(m);
            off_ends(m)
                && (-1..=1).any(|di| {
                    (-1..=1).any(|dk| squares.get(&(i + di, k + dk)).is_some_and(|ps| ps.iter().any(|p| p.distance_xz(m) < away)))
                })
        })
        .collect();
    Some(Arc::new(shut))
}

/// The room of a way for the floors' search (by region): its regions and those linked to them that `open` gives true for and
/// `shut` does not (as `threat::costed_corridor`'s room).
pub fn room(tactics: &Tactics, way: &RegionWay, shut: &[bool], open: impl Fn(u32) -> bool) -> Arc<Vec<bool>> {
    let regions = tactics.regions();
    let mut room = vec![false; regions.len()];
    for &(r, _) in &way.regions {
        room[r as usize] = true;
        for v in regions.out(r).iter().map(|&(v, _)| v).chain(regions.into(r).iter().copied()) {
            if !shut.get(v as usize).copied().unwrap_or(false) && open(v) {
                room[v as usize] = true;
            }
        }
    }
    Arc::new(room)
}

/// The points of `lane` farther than `away` from every point of `other`: the stretch only it walks.
fn own_stretch(lane: &[Vec3], other: &[Vec3], away: f32) -> Vec<Vec3> {
    lane.iter().copied().filter(|p| other.iter().all(|q| q.distance_xz(*p) >= away)).collect()
}

/// Which of two lanes (their own stretches, [`own_stretch`]) a teammate's lane `theirs` walks: the one more of whose own stretch
/// lies within `away` of it; none when it walks neither, or both alike.
fn walks(theirs: &[Vec3], stretches: [&[Vec3]; 2], away: f32) -> Option<usize> {
    let covered = |stretch: &[Vec3]| stretch.iter().filter(|p| theirs.iter().any(|q| q.distance_xz(**p) < away)).count();
    let (a, b) = (covered(stretches[0]), covered(stretches[1]));
    match a.cmp(&b) {
        std::cmp::Ordering::Greater => Some(0),
        std::cmp::Ordering::Less => Some(1),
        std::cmp::Ordering::Equal => None,
    }
}

/// The draw's odds of L2 (`ratio` its cost over L1's, within `band`), with `mates` teammates on each lane and Roam's `friend`
/// factor: L1 weighs `friend^mates[0]`, L2 its place in the band times `friend^mates[1]`.
pub fn odds(ratio: f32, band: f32, mates: [u32; 2], friend: f32) -> f32 {
    let place = ((band - ratio) / (band - 1.0)).clamp(0.0, 1.0);
    let (w1, w2) = (friend.powi(mates[0] as i32), place * friend.powi(mates[1] as i32));
    if w1 + w2 > 0.0 { w2 / (w1 + w2) } else { 0.0 }
}

/// The life's number for a walk from `from` to `to` (in [0, 1)): one per life and pair of 15 m cells.
pub fn number(life_key: u64, from: Vec3, to: Vec3) -> f32 {
    let cell = |p: Vec3| ((p.x / CELL_M).floor() as i32 as u32 as u64, (p.z / CELL_M).floor() as i32 as u32 as u64);
    let ((fx, fz), (tx, tz)) = (cell(from), cell(to));
    let walk = ((fx << 48) ^ (fz << 32) ^ (tx << 16) ^ tz).wrapping_mul(0xD1B5_4A32_D192_ED03);
    super::rng::Rng::new(life_key ^ walk).unit()
}

/// A bot's lane for its goal, held while the goal stays within `away_m` (a feature of the bot's, reset at a new life).
#[derive(Clone, Debug, Default)]
pub struct LaneHold {
    held: Option<Held>,
}

#[derive(Clone, Debug)]
struct Held {
    /// The goal the walk was drawn for (moved with the goal while held).
    goal: Vec3,
    /// What was drawn: L1 (none), or L2.
    lane: Option<Second>,
}

/// A held L2: the regions it keeps out of, and the longest its route may be ([`Room::cap_m`]).
#[derive(Clone, Debug)]
struct Second {
    shut: Arc<Vec<bool>>,
    cap_m: f32,
}

/// Where the floors' search of an L2 walk keeps to: the room of its way on the regions (`RouteFilter::keeping_to`), and the longest
/// its route may be: the band times L1's cost on the regions (the true cost's cap: a route over it is not walked).
#[derive(Clone, Debug)]
pub struct Room {
    pub regions: Arc<Vec<bool>>,
    pub cap_m: f32,
}

/// What a replan does with the lane ([`LaneHold::replan`]).
#[derive(Clone, Debug)]
pub enum Replan {
    /// A new walk: draw, if it is long.
    Draw,
    /// The walk's lane is held: L1 (the plain search), or L2 (its way kept out of these regions).
    Held(Option<Arc<Vec<bool>>>),
}

impl super::features::Feature for LaneHold {
    fn new_life(&mut self, _life: &super::features::Life) {
        self.held = None;
    }
}

impl LaneHold {
    /// A replan for `goal`: a draw for a goal it holds no lane for (or none within `away_m` of it); else the held lane, its goal
    /// moved to `goal`. A replan for danger (`danger`) holds L1 for the goal and draws nothing.
    pub fn replan(&mut self, goal: Vec3, away_m: f32, danger: bool) -> Replan {
        match self.held.as_mut() {
            Some(held) if held.goal.distance_xz(goal) <= away_m => {
                held.goal = goal;
                if danger {
                    held.lane = None;
                }
                Replan::Held(held.lane.as_ref().map(|second| second.shut.clone()))
            }
            _ if danger => {
                self.held = Some(Held { goal, lane: None });
                Replan::Held(None)
            }
            _ => Replan::Draw,
        }
    }

    /// The walk to `goal` was drawn: L1 (`lane` none) or L2 (the regions it keeps out of, and its route's cap, m).
    pub fn drawn(&mut self, goal: Vec3, lane: Option<(Arc<Vec<bool>>, f32)>) {
        self.held = Some(Held { goal, lane: lane.map(|(shut, cap_m)| Second { shut, cap_m }) });
    }

    /// A held L2's room from where the bot is now: its route's cap from there.
    pub fn capped(&mut self, cap_m: f32) {
        if let Some(second) = self.held.as_mut().and_then(|held| held.lane.as_mut()) {
            second.cap_m = cap_m;
        }
    }

    /// The cap of a held L2's route (`length`, m, to a goal within `away_m` of `goal`) when the route is over it.
    pub fn over_cap(&self, goal: Vec3, length: f32, away_m: f32) -> Option<f32> {
        let held = self.held.as_ref().filter(|held| held.goal.distance_xz(goal) <= away_m)?;
        held.lane.as_ref().map(|second| second.cap_m).filter(|&cap| length > cap)
    }

    /// Whether it holds L2 for a goal within `away_m` of `goal`.
    pub fn second_for(&self, goal: Vec3, away_m: f32) -> bool {
        self.held.as_ref().is_some_and(|held| held.lane.is_some() && held.goal.distance_xz(goal) <= away_m)
    }

    /// The held L2 had no way (a region link the body's reach refuses): L1 for the goal from now.
    pub fn drop_lane(&mut self) {
        if let Some(held) = self.held.as_mut() {
            held.lane = None;
        }
    }
}

/// A team's lanes walked now or lately (a feature of the team's): one claim a bot, its latest long walk's lane.
#[derive(Clone, Debug, Default)]
pub struct TeamLanes {
    claims: Vec<Claim>,
}

#[derive(Clone, Debug)]
struct Claim {
    who: Place,
    goal: Vec3,
    points: Vec<Vec3>,
    /// When the walk ended (its route gone or for another goal), none while under way.
    ended: Option<u32>,
}

impl TeamLanes {
    /// `who`'s route now goes to `goal` (none: it has no route): its claim ends at `time` when that is not the claim's walk
    /// (within `away_m` of its goal); claims ended more than `keep_ms` ago are dropped.
    pub fn walking(&mut self, who: Place, goal: Option<Vec3>, away_m: f32, time: u32, keep_ms: u32) {
        for claim in self.claims.iter_mut().filter(|c| c.who == who && c.ended.is_none()) {
            if goal.is_none_or(|g| g.distance_xz(claim.goal) > away_m) {
                claim.ended = Some(time);
            }
        }
        self.claims.retain(|c| c.ended.is_none_or(|end| time <= end.saturating_add(keep_ms)));
    }

    /// `who` walks `points` to `goal` (its claim before is replaced).
    pub fn claim(&mut self, who: Place, goal: Vec3, points: Vec<Vec3>) {
        self.claims.retain(|c| c.who != who);
        self.claims.push(Claim { who, goal, points, ended: None });
    }

    /// `who`'s walk under way goes on to `goal` (a held lane's goal moved).
    pub fn regoal(&mut self, who: Place, goal: Vec3) {
        if let Some(claim) = self.claims.iter_mut().find(|c| c.who == who && c.ended.is_none()) {
            claim.goal = goal;
        }
    }

    /// The teammates (not `who`) on each of two lanes ([`walks`] by their own stretches): a claim under way of one `alive`, or one
    /// ended within `keep_ms` of `time`.
    pub fn mates_on(&self, who: Place, stretches: [&[Vec3]; 2], away_m: f32, time: u32, keep_ms: u32, alive: impl Fn(Place) -> bool) -> [u32; 2] {
        let mut on = [0; 2];
        for claim in &self.claims {
            let counts = claim.who != who
                && match claim.ended {
                    None => alive(claim.who),
                    Some(end) => time <= end.saturating_add(keep_ms),
                };
            if counts && let Some(lane) = walks(&claim.points, stretches, away_m) {
                on[lane] += 1;
            }
        }
        on
    }
}

/// The draw's verdict on a long walk, for its line and the walk.
#[derive(Clone, Debug)]
pub enum Verdict {
    /// No second lane: why (no room off the ends, no way, over the band, more exposed).
    One(&'static str),
    /// Two lanes: L2's cost, the teammates on each, the odds of L2 and the number drawn; `second` when L2 was drawn.
    Two { c2: f32, mates: [u32; 2], odds: f32, number: f32, second: bool },
}

/// Why a walk has no second lane ([`Verdict::One`]).
const NO_ROOM: &str = "no room off the ends";
const NO_WAY: &str = "no way";
const OVER_BAND: &str = "over the band";
const MORE_EXPOSED: &str = "more exposed";

/// The nav line of a drawn walk: `nav: lane draw: <play> to (x, z): C1 m, <verdict>`.
pub fn line(play: impl std::fmt::Debug, goal: Vec3, c1: f32, verdict: &Verdict) -> String {
    let head = format!("{}lane draw: {play:?} to ({:.1}, {:.1}): C1 {c1:.1} m", super::nav_log::PREFIX, goal.x, goal.z);
    match verdict {
        Verdict::One(why) => format!("{head}, one lane ({why}), walks L1"),
        Verdict::Two { c2, mates, odds, number, second } => format!(
            "{head}, C2 {c2:.1} m ({:.3}), mates on L1 {} L2 {}, p(L2) {odds:.3}, drew {:.3}: walks {} (rank {}), {:.1} m",
            c2 / c1.max(0.1),
            mates[0],
            mates[1],
            number,
            if *second { "L2" } else { "L1" },
            if *second { 2 } else { 1 },
            if *second { *c2 } else { c1 },
        ),
    }
}

/// What the drive knows of a new walk, for [`draw`] and [`held_room`].
pub(super) struct Walk<'a> {
    /// The start's and the goal's floors.
    pub ends: (NodeId, NodeId),
    pub goal: Vec3,
    pub time: u32,
    /// Whether a region is open to the plain search (`Tactics::region_allowed_from` and the filter's).
    pub open: &'a dyn Fn(u32) -> bool,
}

/// A new walk's lane ([module](self)): L1 and L2 on the regions, one drawn, held for the goal, claimed for the team and logged
/// (a walk not over `set.long_m` on the regions, or with no way there: L1 held, no line). The room the floors' search keeps to:
/// L2's when drawn, else none (the plain search).
#[allow(clippy::too_many_arguments)]
pub(super) fn draw(
    bot: &mut super::Bot,
    me: &super::view::PlaceView,
    view: &super::view::MatchView,
    tactics: &Tactics,
    tuning: &super::tuning::Tuning,
    set: &LaneDrawTuning,
    walk: Walk<'_>,
    lanes: &mut TeamLanes,
) -> Option<Room> {
    let (grid, regions) = (&tactics.world.grid, tactics.regions());
    let (away, band) = (set.away_m.get(), set.band.get());
    let (from, to) = (regions.of(walk.ends.0), regions.of(walk.ends.1));
    let ends = [grid.position(walk.ends.0), grid.position(walk.ends.1)];
    // The ways' costs between the walk's own ends (`RegionWay::cost_between`).
    let first = region_way(tactics, from, to, walk.open)
        .map(|first| {
            let c1 = first.cost_between(tactics, ends[0], ends[1]);
            (first, c1)
        })
        .filter(|&(_, c1)| c1 > set.long_m.get());
    let Some((first, c1)) = first else {
        bot.features.lane.drawn(walk.goal, None);
        return None;
    };
    let num = |name| tuning.number(super::plays::PlayKind::Roam, name);
    let keep_ms = (num("fresh_s") * 1000.0) as u32;
    let points1 = first.points(tactics);
    // Metres of a way's links into the regions the team's held shooters see (none known: 0).
    let exposed = |way: &RegionWay| {
        if tuning.threat.spots && !bot.threat.is_empty() { way.into_covered(|r| bot.threat.region(r)) } else { 0.0 }
    };
    let second = shut_near(tactics, &points1, ends, set).ok_or(NO_ROOM).and_then(|shut| {
        let way = region_way(tactics, from, to, |r| !shut[r as usize] && (walk.open)(r)).ok_or(NO_WAY)?;
        let c2 = way.cost_between(tactics, ends[0], ends[1]);
        if c2 > band * c1 {
            Err(OVER_BAND)
        } else if exposed(&way) > exposed(&first) {
            Err(MORE_EXPOSED)
        } else {
            Ok((way, c2, shut))
        }
    });
    let play = bot.assignment.as_ref().map(|a| a.kind);
    let (verdict, lane) = match second {
        Err(why) => (Verdict::One(why), None),
        Ok((way, c2, shut)) => {
            let points2 = way.points(tactics);
            let stretches = [own_stretch(&points1, &points2, away), own_stretch(&points2, &points1, away)];
            let team = me.team;
            let alive = |who: Place| view.team(team).any(|p| p.place == who);
            let mates = lanes.mates_on(bot.place, [&stretches[0], &stretches[1]], away, walk.time, keep_ms, alive);
            // DIAGNOSTIC (bench only, not for merge): the draw without the teammate weight (a factor of 1: the mates are
            // counted for the line, not weighed), to see whether the cover the spread costs comes back.
            let odds = odds(c2 / c1.max(0.1), band, mates, 1.0);
            let number = number(bot.life_key, me.feet, walk.goal);
            let second = number < odds;
            let verdict = Verdict::Two { c2, mates, odds, number, second };
            (verdict, second.then_some((way, points2, shut)))
        }
    };
    bot.nav_log.push(super::nav_log::Kind::Lane, walk.time, line(play, walk.goal, c1, &verdict));
    match lane {
        Some((way, points, shut)) => {
            let room = Room { regions: room(tactics, &way, &shut, walk.open), cap_m: band * c1 };
            bot.features.lane.drawn(walk.goal, Some((shut, room.cap_m)));
            lanes.claim(bot.place, walk.goal, points);
            Some(room)
        }
        None => {
            bot.features.lane.drawn(walk.goal, None);
            lanes.claim(bot.place, walk.goal, points1);
            None
        }
    }
}

/// A held L2's room from where the bot is now (`walk.ends`): its way on the regions kept out of `shut`, its cap `band` times L1's cost
/// from there; none with no way there.
pub(super) fn held_room(tactics: &Tactics, shut: &[bool], band: f32, walk: &Walk<'_>) -> Option<Room> {
    let regions = tactics.regions();
    let (from, to) = (regions.of(walk.ends.0), regions.of(walk.ends.1));
    let first = region_way(tactics, from, to, walk.open)?;
    let open = |r: u32| !shut.get(r as usize).copied().unwrap_or(false) && (walk.open)(r);
    let way = region_way(tactics, from, to, open)?;
    let grid = &tactics.world.grid;
    let c1 = first.cost_between(tactics, grid.position(walk.ends.0), grid.position(walk.ends.1));
    Some(Room { regions: room(tactics, &way, shut, walk.open), cap_m: band * c1 })
}

/// The nav line of an L2 route over its cap: `nav: lane draw: L2 to (x, z) over its cap: L m against C m, walks L1`.
pub fn over_cap_line(goal: Vec3, length: f32, cap_m: f32) -> String {
    format!("{}lane draw: L2 to ({:.1}, {:.1}) over its cap: {length:.1} m against {cap_m:.1} m, walks L1", super::nav_log::PREFIX, goal.x, goal.z)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_odds_fall_with_cost_and_turn_from_mates() {
        // Equal cost and no mates: even. At the band's edge: L1 always.
        assert!((odds(1.0, 1.15, [0, 0], 0.6) - 0.5).abs() < 1e-6);
        assert_eq!(odds(1.15, 1.15, [0, 0], 0.6), 0.0);
        assert_eq!(odds(1.3, 1.15, [0, 0], 0.6), 0.0, "past the band: never");
        // Halfway: L2 weighs 0.5 against 1.
        assert!((odds(1.075, 1.15, [0, 0], 0.6) - 1.0 / 3.0).abs() < 1e-5);
        // A mate on L1 makes L2 likelier, one on L2 less.
        let alone = odds(1.05, 1.15, [0, 0], 0.6);
        assert!(odds(1.05, 1.15, [1, 0], 0.6) > alone && odds(1.05, 1.15, [0, 1], 0.6) < alone);
        assert!((odds(1.05, 1.15, [1, 1], 0.6) - alone).abs() < 1e-6, "one on each: as alone");
    }

    #[test]
    fn a_life_draws_one_number_a_walk() {
        let (a, b) = (Vec3::new(10.0, 0.0, 10.0), Vec3::new(200.0, 0.0, 50.0));
        let n = number(7, a, b);
        assert!((0.0..1.0).contains(&n));
        assert_eq!(n, number(7, a + Vec3::new(1.0, 0.0, 1.0), b), "the same cells: the same number");
        assert_ne!(n, number(8, a, b), "another life");
        assert_ne!(n, number(7, b, a), "the way back");
        // Spread: the numbers of many lives are about uniform.
        let below = (0..1000u64).filter(|&k| number(k.wrapping_mul(0x9E37_79B9_7F4A_7C15), a, b) < 0.5).count();
        assert!((400..600).contains(&below), "{below}");
    }

    #[test]
    fn a_held_lane_stays_with_its_goal_and_danger_turns_it_to_l1() {
        let mut hold = LaneHold::default();
        let goal = Vec3::new(100.0, 0.0, 0.0);
        assert!(matches!(hold.replan(goal, 15.0, false), Replan::Draw));
        let shut = Arc::new(vec![true, false]);
        hold.drawn(goal, Some((shut.clone(), 115.0)));
        // The goal moves a little: L2 held, no draw.
        let moved = goal + Vec3::new(5.0, 0.0, 0.0);
        assert!(matches!(hold.replan(moved, 15.0, false), Replan::Held(Some(_))));
        // It moves on, within reach of the last: still held.
        assert!(matches!(hold.replan(moved + Vec3::new(12.0, 0.0, 0.0), 15.0, false), Replan::Held(Some(_))));
        // Danger: L1 for the goal, kept after.
        assert!(matches!(hold.replan(moved + Vec3::new(12.0, 0.0, 0.0), 15.0, true), Replan::Held(None)));
        assert!(matches!(hold.replan(moved + Vec3::new(12.0, 0.0, 0.0), 15.0, false), Replan::Held(None)));
        // A far goal: a new draw.
        assert!(matches!(hold.replan(Vec3::new(0.0, 0.0, 100.0), 15.0, false), Replan::Draw));
        // A danger replan with no lane held: L1, held.
        let mut fresh = LaneHold::default();
        assert!(matches!(fresh.replan(goal, 15.0, true), Replan::Held(None)));
        assert!(matches!(fresh.replan(goal, 15.0, false), Replan::Held(None)));
    }

    /// Two lanes from (0, 0) to (100, 0): L1 straight, L2 by z 40 from x 20 to 80. A mate on either is counted on it, under way while alive, after
    /// its end for the window.
    #[test]
    fn mates_are_counted_on_the_lane_they_walk() {
        let line = |z: f32| (0..=50).map(|k| Vec3::new(k as f32 * 2.0, 0.0, if (10..=40).contains(&k) { z } else { 0.0 })).collect::<Vec<_>>();
        let (l1, l2) = (line(0.0), line(40.0));
        let (s1, s2) = (own_stretch(&l1, &l2, 15.0), own_stretch(&l2, &l1, 15.0));
        assert!(!s1.is_empty() && !s2.is_empty());
        let mut lanes = TeamLanes::default();
        lanes.claim(1, Vec3::new(100.0, 0.0, 0.0), l1.clone());
        lanes.claim(2, Vec3::new(100.0, 0.0, 0.0), l2.clone());
        lanes.claim(3, Vec3::new(100.0, 0.0, 0.0), l2.clone());
        let all = |_| true;
        assert_eq!(lanes.mates_on(0, [&s1, &s2], 15.0, 0, 15_000, all), [1, 2]);
        assert_eq!(lanes.mates_on(2, [&s1, &s2], 15.0, 0, 15_000, all), [1, 1], "not its own");
        assert_eq!(lanes.mates_on(0, [&s1, &s2], 15.0, 0, 15_000, |p| p != 3), [1, 1], "a dead mate's walk under way is not");
        // Mate 1's walk ends at 1 s: counted till 16 s.
        lanes.walking(1, None, 15.0, 1_000, 15_000);
        assert_eq!(lanes.mates_on(0, [&s1, &s2], 15.0, 16_000, 15_000, all), [1, 2]);
        lanes.walking(0, None, 15.0, 16_001, 15_000);
        assert_eq!(lanes.mates_on(0, [&s1, &s2], 15.0, 16_001, 15_000, all), [0, 2], "ended past the window: dropped");
        // A walk whose goal moved within reach goes on.
        lanes.regoal(2, Vec3::new(105.0, 0.0, 0.0));
        lanes.walking(2, Some(Vec3::new(108.0, 0.0, 0.0)), 15.0, 17_000, 15_000);
        assert_eq!(lanes.mates_on(0, [&s1, &s2], 15.0, 40_000, 15_000, all), [0, 2]);
        // A lane along neither stretch: no one's.
        assert_eq!(walks(&line(-60.0)[20..=30], [&s1, &s2], 15.0), None);
    }

    #[test]
    fn the_line_names_the_lanes_and_the_draw() {
        let goal = Vec3::new(1.0, 0.0, 2.0);
        let two = Verdict::Two { c2: 110.0, mates: [1, 0], odds: 0.4, number: 0.2, second: true };
        let text = line("Collect", goal, 100.0, &two);
        assert!(text.starts_with("nav: lane draw: \"Collect\" to (1.0, 2.0): C1 100.0 m, C2 110.0 m (1.100), mates on L1 1 L2 0"), "{text}");
        assert!(text.ends_with("p(L2) 0.400, drew 0.200: walks L2 (rank 2), 110.0 m"), "{text}");
        assert!(line("Cut", goal, 80.0, &Verdict::One(NO_WAY)).ends_with("one lane (no way), walks L1"));
    }

    /// On the test field (80 by 40 m, a wall at x 40 from z 0 to 25): the regions' way from (10, 10) to (70, 10) goes by the gap
    /// above the wall, its cost no less than the straight distance; with the regions near it shut (the ends' 25 m open) there is no
    /// second way, and a held lane's room is none; with nothing shut the room holds the way's regions.
    #[test]
    fn the_second_way_keeps_off_the_first_and_the_field_has_one_gap() {
        let world = crate::v026g::bots::tactics::tests::field_with(|_| {});
        let tactics = Tactics::new(world, 1.0);
        let grid = &tactics.world.grid;
        let (a, b) = (grid.locate(Vec3::new(10.0, 0.0, 10.0), 1.5).unwrap(), grid.locate(Vec3::new(70.0, 0.0, 10.0), 1.5).unwrap());
        let regions = tactics.regions();
        let way = region_way(&tactics, regions.of(a), regions.of(b), |_| true).unwrap();
        let points = way.points(&tactics);
        assert!(points.iter().any(|p| p.z > 25.0 && (35.0..45.0).contains(&p.x)), "{points:?}");
        assert!(way.cost >= grid.position(a).distance_xz(grid.position(b)), "{}", way.cost);
        assert_eq!(way.regions.first().map(|r| r.0), Some(regions.of(a)));
        assert_eq!(way.regions.last().map(|r| r.0), Some(regions.of(b)));
        let set = LaneDrawTuning::default();
        let ends = [grid.position(a), grid.position(b)];
        // Between the walk's own ends: no shorter than the straight line, and the way's own cost when the ends are its end
        // regions' middles; one region is the straight leg, one link its kind's penalty on the straight leg.
        let between = way.cost_between(&tactics, ends[0], ends[1]);
        assert!(between >= ends[0].distance_xz(ends[1]) && between < way.cost + 2.0 * 8.0, "{between} {}", way.cost);
        let middles = [grid.position(regions.middle(regions.of(a))), grid.position(regions.middle(regions.of(b)))];
        assert!((way.cost_between(&tactics, middles[0], middles[1]) - way.cost).abs() < 1e-3);
        let one = RegionWay { regions: vec![(regions.of(a), 0.0)], cost: 0.0 };
        assert!((one.cost_between(&tactics, ends[0], ends[1]) - (ends[1] - ends[0]).length()).abs() < 1e-4);
        let shut = shut_near(&tactics, &points, ends, &set).unwrap();
        assert!(shut.iter().any(|&s| s));
        assert!(region_way(&tactics, regions.of(a), regions.of(b), |r| !shut[r as usize]).is_none(), "one gap");
        let open = |_: u32| true;
        let walk = Walk { ends: (a, b), goal: grid.position(b), time: 0, open: &open };
        assert!(held_room(&tactics, &shut, 1.15, &walk).is_none());
        let room = held_room(&tactics, &vec![false; regions.len()], 1.15, &walk).unwrap();
        assert!(way.regions.iter().all(|&(r, _)| room.regions[r as usize]));
        assert!((room.cap_m - 1.15 * between).abs() < 1e-3, "nothing shut: L2 is L1, the cap the band on its cost between the ends");
        // The walk is all ends: no room for a second lane.
        assert!(shut_near(&tactics, &points[..2], [points[0], points[1]], &set).is_none());
    }

    #[test]
    fn a_held_second_lane_is_known_for_its_goal() {
        let mut hold = LaneHold::default();
        let goal = Vec3::new(100.0, 0.0, 0.0);
        assert!(!hold.second_for(goal, 15.0));
        hold.drawn(goal, Some((Arc::new(vec![true]), 115.0)));
        assert!(hold.second_for(goal + Vec3::new(10.0, 0.0, 0.0), 15.0) && !hold.second_for(Vec3::ZERO, 15.0));
        // Its cap: a route over it is named, one within it not; the cap from where the bot is now replaces it.
        assert_eq!(hold.over_cap(goal, 120.0, 15.0), Some(115.0));
        assert_eq!(hold.over_cap(goal, 110.0, 15.0), None);
        assert_eq!(hold.over_cap(Vec3::ZERO, 120.0, 15.0), None, "another goal");
        hold.capped(90.0);
        assert_eq!(hold.over_cap(goal, 100.0, 15.0), Some(90.0));
        assert!(over_cap_line(goal, 100.0, 90.0).ends_with("lane draw: L2 to (100.0, 0.0) over its cap: 100.0 m against 90.0 m, walks L1"));
        hold.drop_lane();
        assert!(!hold.second_for(goal, 15.0));
    }
}
