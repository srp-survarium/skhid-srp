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
//!   up) is the plain search, and the hold turns to L1 for the walk.
//! - **No more leaps.** L2's floors' route is over the cap when it makes more leaps (jump, gap and drop links, [`route_leaps`])
//!   than L1's way on the regions has leap links ([`RegionWay::leaps`], [`Cap`]): a second lane asks no more jumps, gaps or drops
//!   of the body than the first (v65: L2's stuck per 100 starts 87-104 s against L1's 55-71 s from the same draws, its route-jump
//!   part 37.6 against 7.3 in TDM at 1.15). The test is on the floors, where L2's route is known: on the regions alone a way's
//!   leap links are not its route's (the probe's 177 lane groups: the regions' count would drop 13 of 23 L2s in the band the
//!   floors keep). L1's floors' route is not searched with L2's, so L1 counts its regions' leap links: they are its floors'
//!   route's in 130 of 174 groups (more in 35, fewer in 9), and the floors' test against them decides as against L1's floors'
//!   route in 49 of 53 L2s within +15 % and 102 of 110 within +30 %.
//! - **L1 the cheaper.** When L2 costs less than L1 between the walk's own ends (the regions rank the ways from their end regions'
//!   middles; 3 % of v65's two-lane draws), the two swap and the second lane is searched again off the new L1: the cap, the odds and
//!   the shut regions are measured from the cheaper lane.
//! - **Cover first.** More teammates on one lane now than on the other: the bot takes that lane, in their cover ([`cover`]; a
//!   teammate is on a lane by its walk's claim, or by its body within `away_m` of the lane's own stretch). Equal, or none: the
//!   draw by cost, times Roam's `death_factor` per teammate who died walking that lane within Roam's `fresh_s` ([`odds`]). (v65 C,
//!   from the same draws: a bot that left the lane its mates walked died unturned in 5.3 % of starts, one that went with them 3.6 %;
//!   Roam's weights changed only 593 of 5,081 draws and left a mate at ratio 1.12 a quarter of the time.)
//! - **The odds from the humans' lanes.** L2 weighs, against L1's 1, the live humans' legs in its 5 % bin of cost over L1's
//!   against their first bin's ([`HUMAN_LEGS`]), within the cap (`band`) and nothing past it: the cap and the odds are apart, and a
//!   dearer lane weighs what the humans' dearer lanes are walked, as measured. The draw's number is the life's own for the walk's 15 m goal cell (`Bot::life_key`): the same walk in one life draws the same,
//!   the next life draws anew.
//! - **One draw a walk.** The decision is held for the walk ([`LaneHold`]) until its route ends for good: arrived, a new play, the
//!   play's end, no goal or no way (`Bot::end_route`, [`ends_walk`]), or a new life. A replan in between, for any goal, keeps L2's
//!   shut regions (its way searched again on the regions from where the bot is to the goal now) or L1, and draws nothing (v65 C:
//!   303 of 1,763 L2 draws were drawn again within one play when the goal moved over 15 m, 173 of them lost L2 with three quarters
//!   of the way left).
//! - **The trace.** One nav line a long walk ([`line`]): the play, the goal, C1, C2 and their ratio, the teammates on each lane, the
//!   odds and the number drawn, the lane walked (rank 1 or 2) and its cost; or why there is one lane only.
//!
//! Roam keeps its human-route draw (its walks are not drawn here), and a carrier's walk home is a race (not drawn).
//!
//! The values ([`LaneDrawTuning`]) are measures, not fits: `long_m` is the lane test's long leg; `away_m` and `ends_m` the C2
//! probe's; `band` the cap: +15 %, where the live humans' lanes off their cheapest thin out ([`HUMAN_LEGS`]: 870, 658, 529 legs
//! per 5 % to +15 %, then 295), or +30 %, where the bins after hold about that level. The odds are the humans' bins as measured; the
//! death weight and window are Roam's own.

use std::sync::Arc;

use mapdata::{NodeId, Vec3};

use super::route_end::RouteEnd;
use super::tactics::Tactics;
use super::tuning::LaneDrawTuning;
use super::view::Place;

/// The lane test's cell, m: the draw's number is the life's own for the walk's goal cell.
const CELL_M: f32 = 15.0;

/// The live humans' legs off their group's cheapest lane, per 5 % bin of their lane's cost over the cheapest's: (1.00, 1.05],
/// (1.05, 1.10], ... (1.45, 1.50] (27,255 legs of 2,712 recordings in groups of 3+ legs by the lane test's 30 m Hausdorff;
/// `lane-band-humans-v2.py` on `legs3-H.tsv`). Not monotone: the bin (1.20, 1.25] holds more legs (355) than (1.15, 1.20] (295),
/// and (1.40, 1.45] (218) more than (1.35, 1.40] (190); the draw uses them as measured.
pub const HUMAN_LEGS: [u32; 10] = [870, 658, 529, 295, 355, 260, 240, 190, 218, 162];

/// The weight of a second lane `ratio` times L1's cost against L1's 1: its bin's humans' legs over the first bin's ([`HUMAN_LEGS`];
/// at or under L1's cost the first bin's); none past the table.
pub fn human_weight(ratio: f32) -> f32 {
    let bin = ((ratio - 1.0) / 0.05).ceil().max(1.0) as usize - 1;
    HUMAN_LEGS.get(bin).map_or(0.0, |&legs| legs as f32 / HUMAN_LEGS[0] as f32)
}

/// Whether a route ending ends the walk, and with it the walk's lane ([`LaneHold`]): the walk got there, or its play or goal is
/// gone. Every other ending (a replan, the body off the route, a failed or refused jump, no progress, a trap, an escape, the
/// reckoning) is the same walk going on.
pub fn ends_walk(why: RouteEnd) -> bool {
    matches!(why, RouteEnd::Arrived | RouteEnd::NewAssignment | RouteEnd::EndPlay | RouteEnd::NoGoal | RouteEnd::NoWay | RouteEnd::NewLife)
}

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

    /// Its leap links: those whose cost is over the distance between the two regions' middles by half the smallest leap kind's
    /// penalty or more (a link's cost is that distance plus its kind's penalty, `mapdata::regions`: 0 for a walk, a drop's or a
    /// jump's otherwise).
    pub fn leaps(&self, tactics: &Tactics) -> u32 {
        let (grid, regions) = (&tactics.world.grid, tactics.regions());
        let at = |r: u32| grid.position(regions.middle(r));
        let least = mapdata::grid::LinkKind::Drop.penalty() / 2.0;
        self.regions.windows(2).filter(|w| w[1].1 - (at(w[1].0) - at(w[0].0)).length() >= least).count() as u32
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

/// The teammates on each of a walk's two lanes ([`TeamLanes::mates_on`]): those on it now (alive, walking it or standing on its own
/// stretch), and those who died walking it lately (within Roam's `fresh_s` of their death).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mates {
    pub now: [u32; 2],
    pub died: [u32; 2],
}

/// The lane the teammates now on the walk's lanes give a bot ([`Mates::now`]): the one more of them are on, in their cover; none
/// when as many are on each (or none on either), and the cost draws.
pub fn cover(mates: Mates) -> Option<bool> {
    match mates.now[1].cmp(&mates.now[0]) {
        std::cmp::Ordering::Greater => Some(true),
        std::cmp::Ordering::Less => Some(false),
        std::cmp::Ordering::Equal => None,
    }
}

/// The draw's odds of L2 (`ratio` its cost over L1's) with no teammate's cover to keep to: L1 weighs 1 and L2 the humans' weight of
/// its bin ([`human_weight`]) within the cap `band`, nothing past it; each times Roam's `death` factor per teammate who died walking
/// it lately (`died`).
pub fn odds(ratio: f32, band: f32, died: [u32; 2], death: f32) -> f32 {
    let place = if ratio <= band { human_weight(ratio) } else { 0.0 };
    let weight = |lane: usize| death.powi(died[lane] as i32);
    let (w1, w2) = (weight(0), place * weight(1));
    if w1 + w2 > 0.0 { w2 / (w1 + w2) } else { 0.0 }
}

/// The life's number for a walk to `goal` (in [0, 1)): one per life and 15 m goal cell.
pub fn number(life_key: u64, goal: Vec3) -> f32 {
    let cell = |v: f32| (v / CELL_M).floor() as i32 as u32 as u64;
    let walk = ((cell(goal.x) << 32) ^ cell(goal.z)).wrapping_mul(0xD1B5_4A32_D192_ED03);
    super::rng::Rng::new(life_key ^ walk).unit()
}

/// A bot's lane for its walk, held until the walk ends ([`ends_walk`]; a feature of the bot's, reset at a new life), and its
/// pincer ([`super::pincer`]), if its walk is in one.
#[derive(Clone, Debug, Default)]
pub struct LaneHold {
    held: Option<Held>,
    pincer: Option<super::pincer::Pincer>,
    /// [pincer] The goal a pairing was last refused for (shared): not tried again for a goal near it (a new life forgets it).
    refused: Option<Vec3>,
}

#[derive(Clone, Debug)]
struct Held {
    /// The goal the walk goes to now (moved with each replan).
    goal: Vec3,
    /// What was drawn: L1 (none), or L2.
    lane: Option<Second>,
}

/// A held L2: the regions it keeps out of, and its route's cap.
#[derive(Clone, Debug)]
struct Second {
    shut: Arc<Vec<bool>>,
    cap: Cap,
}

/// The most an L2 route may ask, against L1 on the regions: its length at most `metres` (the band times L1's cost: the true
/// cost's cap), its leaps ([`route_leaps`]) no more than L1's leap links (`leaps`, [`RegionWay::leaps`]). A route over either is not walked.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cap {
    pub metres: f32,
    pub leaps: u32,
}

impl Cap {
    /// Whether a route of `length` m making `jumps` leaps is over it.
    pub fn over(&self, length: f32, jumps: u32) -> bool {
        length > self.metres || jumps > self.leaps
    }
}

/// Where the floors' search of an L2 walk keeps to: the room of its way on the regions (`RouteFilter::keeping_to`), and its route's
/// cap.
#[derive(Clone, Debug)]
pub struct Room {
    pub regions: Arc<Vec<bool>>,
    pub cap: Cap,
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
        self.pincer = None;
        self.refused = None;
    }
}

impl LaneHold {
    /// A replan for `goal`: a draw for a new walk; else the walk's held lane, its goal moved to `goal`. A replan for danger
    /// (`danger`) holds L1 for the walk and draws nothing.
    pub fn replan(&mut self, goal: Vec3, danger: bool) -> Replan {
        match self.held.as_mut() {
            Some(held) => {
                held.goal = goal;
                if danger {
                    held.lane = None;
                }
                Replan::Held(held.lane.as_ref().map(|second| second.shut.clone()))
            }
            None if danger => {
                self.held = Some(Held { goal, lane: None });
                Replan::Held(None)
            }
            None => Replan::Draw,
        }
    }

    /// The walk to `goal` was drawn: L1 (`lane` none) or L2 (the regions it keeps out of, and its route's cap).
    pub fn drawn(&mut self, goal: Vec3, lane: Option<(Arc<Vec<bool>>, Cap)>) {
        self.held = Some(Held { goal, lane: lane.map(|(shut, cap)| Second { shut, cap }) });
    }

    /// The route ended for `why`: the walk's lane goes with the walk ([`ends_walk`]), and its pincer. An arrival at a pincer's
    /// stage is the walk going on (the hold, then in).
    pub fn route_ended(&mut self, why: RouteEnd) {
        use super::pincer::Phase;
        if why == RouteEnd::Arrived && self.pincer.is_some_and(|p| p.phase != Phase::In) {
            return;
        }
        if ends_walk(why) {
            self.held = None;
            self.pincer = None;
        }
    }

    /// The point the walk heads for now: its pincer's stage until it goes in, else the play's `goal`.
    pub fn goal(&self, goal: Vec3) -> Vec3 {
        match self.pincer {
            Some(p) if p.phase != super::pincer::Phase::In => p.stage,
            _ => goal,
        }
    }

    /// [pincer] A pairing was refused for its walk to `goal` (shared).
    pub fn pincer_refused(&mut self, goal: Vec3) {
        self.refused = Some(goal);
    }

    /// [pincer] Whether a pairing was refused for its walk to a goal within `away_m` of `goal`.
    pub fn refused_for(&self, goal: Vec3, away_m: f32) -> bool {
        self.refused.is_some_and(|r| r.distance_xz(goal) <= away_m)
    }

    /// Its walk's pincer, if any.
    pub fn pincer(&self) -> Option<super::pincer::Pincer> {
        self.pincer
    }

    /// Its walk to `goal` is in a pincer from now: held (no lane drawn: the stage is its way), the pincer `p`.
    pub fn start_pincer(&mut self, goal: Vec3, p: super::pincer::Pincer) {
        self.held = Some(Held { goal, lane: None });
        self.pincer = Some(p);
    }

    /// Its pincer's phase is `phase` from now.
    pub fn pincer_phase(&mut self, phase: super::pincer::Phase) {
        if let Some(p) = self.pincer.as_mut() {
            p.phase = phase;
        }
    }

    /// Whether it walks a held walk now, and on L2.
    pub fn walking(&self) -> Option<bool> {
        self.held.as_ref().map(|held| held.lane.is_some())
    }

    /// A held L2's room from where the bot is now: its route's cap from there.
    pub fn capped(&mut self, cap: Cap) {
        if let Some(second) = self.held.as_mut().and_then(|held| held.lane.as_mut()) {
            second.cap = cap;
        }
    }

    /// The cap of a held L2's route (`length` m, `jumps` jumps, to a goal within `away_m` of the walk's) when the route is over it.
    pub fn over_cap(&self, goal: Vec3, length: f32, jumps: u32, away_m: f32) -> Option<Cap> {
        let held = self.held.as_ref().filter(|held| held.goal.distance_xz(goal) <= away_m)?;
        held.lane.as_ref().map(|second| second.cap).filter(|cap| cap.over(length, jumps))
    }

    /// Whether it holds L2 for a goal within `away_m` of `goal`.
    pub fn second_for(&self, goal: Vec3, away_m: f32) -> bool {
        self.held.as_ref().is_some_and(|held| held.lane.is_some() && held.goal.distance_xz(goal) <= away_m)
    }

    /// The held L2 had no way (a region link the body's reach refuses), or its route was over the cap: L1 for the walk from now.
    pub fn drop_lane(&mut self) {
        if let Some(held) = self.held.as_mut() {
            held.lane = None;
        }
    }
}

/// A team's lanes walked now, and walked by teammates who died on them lately (a feature of the team's).
#[derive(Clone, Debug, Default)]
pub struct TeamLanes {
    claims: Vec<Claim>,
}

#[derive(Clone, Debug)]
struct Claim {
    who: Place,
    /// The walk's two lanes' points (L2's empty for a walk of one lane).
    lanes: Arc<[Vec<Vec3>; 2]>,
    /// Whether it walks L2.
    second: bool,
    /// When its walker was first seen dead with the walk under way.
    died: Option<u32>,
    /// [pincer] What the walk is for: its play, its goal and its objective.
    play: Option<super::plays::PlayKind>,
    goal: Vec3,
    objective: Option<super::pincer::Objective>,
    /// [pincer] Its route as its walker publishes it ([`Ahead`]).
    ahead: Option<Ahead>,
    /// [pincer] Its pincer: the teammate, its stage, the objective's centre, and where it is.
    pincer: Option<Paired>,
}

/// [pincer] A walk's route as its walker publishes it for a teammate's pincer: the point [`super::pincer::final_leg_m`] before its
/// end and the metres to it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ahead {
    pub stage: Vec3,
    pub to_stage_m: f32,
}

/// [pincer] A claim's pincer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Paired {
    pub mate: Place,
    pub stage: Vec3,
    pub centre: Vec3,
    pub state: super::pincer::Shared,
}

/// [pincer] What a walk knows of its claim for a pincer: its play, goal and objective.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WalkFor {
    pub play: Option<super::plays::PlayKind>,
    pub goal: Vec3,
    pub objective: Option<super::pincer::Objective>,
}

impl Claim {
    fn points(&self) -> &[Vec3] {
        &self.lanes[usize::from(self.second)]
    }
}

impl TeamLanes {
    /// `who`'s walk at `time` (`walking`: a walk held, and whether on L2; none: no walk), with `alive` the team's living bodies: its
    /// claim follows the walk's lane, or goes with the walk; a claim under way whose walker is not alive died then; a death is kept
    /// `keep_ms`.
    pub fn walking(&mut self, who: Place, walking: Option<bool>, time: u32, keep_ms: u32, alive: impl Fn(Place) -> bool) {
        for claim in self.claims.iter_mut().filter(|c| c.died.is_none()) {
            if claim.who != who && !alive(claim.who) {
                claim.died = Some(time);
            }
        }
        match walking {
            Some(second) => {
                for claim in self.claims.iter_mut().filter(|c| c.who == who && c.died.is_none()) {
                    claim.second = second;
                }
            }
            None => self.claims.retain(|c| c.who != who || c.died.is_some()),
        }
        self.claims.retain(|c| c.died.is_none_or(|died| time <= died.saturating_add(keep_ms)));
    }

    /// `who` walks `lanes` (`second`: on L2) from now, for `walk` (its claim under way before is replaced).
    pub fn claim(&mut self, who: Place, lanes: Arc<[Vec<Vec3>; 2]>, second: bool, walk: WalkFor) {
        self.claims.retain(|c| c.who != who || c.died.is_some());
        self.claims.push(Claim {
            who,
            lanes,
            second,
            died: None,
            play: walk.play,
            goal: walk.goal,
            objective: walk.objective,
            ahead: None,
            pincer: None,
        });
    }

    fn live(&self, who: Place) -> Option<&Claim> {
        self.claims.iter().find(|c| c.who == who && c.died.is_none())
    }

    fn live_mut(&mut self, who: Place) -> Option<&mut Claim> {
        self.claims.iter_mut().find(|c| c.who == who && c.died.is_none())
    }

    /// [pincer] `who`'s walk's route facts (`ahead`), as it walks it.
    pub fn publish(&mut self, who: Place, ahead: Ahead) {
        if let Some(c) = self.live_mut(who) {
            c.ahead = Some(ahead);
        }
    }

    /// [pincer] Whether `who` has a claim under way.
    pub fn claimed(&self, who: Place) -> bool {
        self.live(who).is_some()
    }

    /// [pincer] A teammate's walk (not `who`'s) under way to the same objective as `objective` (`same`), of a pincer's play, in no
    /// pincer yet, its route published: its walker, goal and route facts.
    pub fn pincer_mate(&self, who: Place, same: impl Fn(super::pincer::Objective) -> bool) -> Option<(Place, Vec3, Ahead)> {
        self.claims
            .iter()
            .filter(|c| c.who != who && c.died.is_none() && c.pincer.is_none() && c.play.is_some_and(super::pincer::plays))
            .find_map(|c| c.objective.filter(|o| same(*o)).and(c.ahead).map(|a| (c.who, c.goal, a)))
    }

    /// [pincer] `first` and `second` in one pincer: each claim's teammate, stage and the objective's centre, both walking to their
    /// stages. `second`'s claim must be under way.
    pub fn pair(&mut self, first: (Place, Vec3), second: (Place, Vec3), centre: Vec3) {
        use super::pincer::Shared;
        for (who, mate, stage) in [(first.0, second.0, first.1), (second.0, first.0, second.1)] {
            if let Some(c) = self.live_mut(who) {
                c.pincer = Some(Paired { mate, stage, centre, state: Shared::Walking });
            }
        }
    }

    /// [pincer] `who`'s claim's pincer, if any.
    pub fn paired(&self, who: Place) -> Option<Paired> {
        self.live(who).and_then(|c| c.pincer)
    }

    /// [pincer] `who`'s pincer's state is `state` from now.
    pub fn share(&mut self, who: Place, state: super::pincer::Shared) {
        if let Some(p) = self.live_mut(who).and_then(|c| c.pincer.as_mut()) {
            p.state = state;
        }
    }

    /// [pincer] Where `who`'s teammate in its pincer is: its state, or gone (its claim ended, its walker dead, or no pincer).
    pub fn mate_state(&self, who: Place) -> super::pincer::Shared {
        use super::pincer::Shared;
        self.paired(who)
            .and_then(|p| self.live(p.mate).and_then(|c| c.pincer).filter(|q| q.mate == who))
            .map_or(Shared::Gone, |q| q.state)
    }

    /// The teammates (not `who`) on each of two lanes (their own stretches): now, each living one of `bodies` (place, feet) by its
    /// walk's claim ([`walks`]), or with none on either, by its body within `away_m` of one stretch and not the other; died, each
    /// claim whose walker died within `keep_ms` of `time`.
    pub fn mates_on(&self, who: Place, stretches: [&[Vec3]; 2], away_m: f32, time: u32, keep_ms: u32, bodies: &[(Place, Vec3)]) -> Mates {
        let mut mates = Mates::default();
        for &(mate, feet) in bodies.iter().filter(|&&(mate, _)| mate != who) {
            let claimed = self.claims.iter().find(|c| c.who == mate && c.died.is_none()).and_then(|c| walks(c.points(), stretches, away_m));
            let on = |s: &[Vec3]| s.iter().any(|p| p.distance_xz(feet) < away_m);
            let body = match (on(stretches[0]), on(stretches[1])) {
                (true, false) => Some(0),
                (false, true) => Some(1),
                _ => None,
            };
            if let Some(lane) = claimed.or(body) {
                mates.now[lane] += 1;
            }
        }
        for claim in self.claims.iter().filter(|c| c.who != who) {
            if let Some(died) = claim.died
                && time <= died.saturating_add(keep_ms)
                && let Some(lane) = walks(claim.points(), stretches, away_m)
            {
                mates.died[lane] += 1;
            }
        }
        mates
    }
}

/// The draw's verdict on a long walk, for its line and the walk.
#[derive(Clone, Debug)]
pub enum Verdict {
    /// No second lane: why (no room off the ends, no way, over the band, more exposed).
    One(&'static str),
    /// Two lanes: L2's cost, the teammates on each; the lane their cover gave (`cover`), or the odds of L2 and the number drawn;
    /// `second` when L2 was taken; `swapped` when the regions' ranking was swapped (L1 the cheaper between the walk's ends).
    Two { c2: f32, mates: Mates, cover: bool, odds: f32, number: f32, second: bool, swapped: bool },
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
        Verdict::Two { c2, mates, cover, odds, number, second, swapped } => {
            let how = if *cover { "with the mates".to_string() } else { format!("p(L2) {odds:.3}, drew {number:.3}") };
            format!(
                "{head}, C2 {c2:.1} m ({:.3}){}, mates now L1 {} L2 {}, died L1 {} L2 {}, {how}: walks {} (rank {}), {:.1} m",
                c2 / c1.max(0.1),
                if *swapped { ", swapped" } else { "" },
                mates.now[0],
                mates.now[1],
                mates.died[0],
                mates.died[1],
                if *second { "L2" } else { "L1" },
                if *second { 2 } else { 1 },
                if *second { *c2 } else { c1 },
            )
        }
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
    /// [pincer] The play and the objective it walks for.
    pub play: Option<super::plays::PlayKind>,
    pub objective: Option<super::pincer::Objective>,
}

impl Walk<'_> {
    fn walk_for(&self) -> WalkFor {
        WalkFor { play: self.play, goal: self.goal, objective: self.objective }
    }
}

/// A new walk's lane ([module](self)): L1 and L2 on the regions, one taken, held for the walk, claimed for the team and logged
/// (a walk not over `set.long_m` on the regions, or with no way there: L1 held, no line). The room the floors' search keeps to:
/// L2's when taken, else none (the plain search).
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
    let Some((mut first, mut c1)) = first else {
        bot.features.lane.drawn(walk.goal, None);
        return None;
    };
    let num = |name| tuning.number(super::plays::PlayKind::Roam, name);
    let (death, keep_ms) = (num("death_factor"), (num("fresh_s") * 1000.0) as u32);
    // The second way off a first's points: its cost between the ends and the regions it keeps out of.
    let second_of = |points: &[Vec3]| {
        let shut = shut_near(tactics, points, ends, set).ok_or(NO_ROOM)?;
        let way = region_way(tactics, from, to, |r| !shut[r as usize] && (walk.open)(r)).ok_or(NO_WAY)?;
        let c2 = way.cost_between(tactics, ends[0], ends[1]);
        Ok::<_, &'static str>((way, c2, shut))
    };
    let mut points1 = first.points(tactics);
    let mut second = second_of(&points1);
    // L1 the cheaper between the walk's ends: the regions' ranking from the end regions' middles swapped, the second way searched
    // again off the new first.
    let mut swapped = false;
    if let Ok((way, c2, _)) = &second
        && *c2 < c1
    {
        (first, c1) = (way.clone(), *c2);
        points1 = first.points(tactics);
        second = second_of(&points1);
        swapped = true;
    }
    // Metres of a way's links into the regions the team's held shooters see (none known: 0).
    let exposed = |way: &RegionWay| {
        if tuning.threat.spots && !bot.threat.is_empty() { way.into_covered(|r| bot.threat.region(r)) } else { 0.0 }
    };
    let leaps1 = first.leaps(tactics);
    let second = second.and_then(|(way, c2, shut)| {
        if c2 > band * c1 {
            Err(OVER_BAND)
        } else if exposed(&way) > exposed(&first) {
            Err(MORE_EXPOSED)
        } else {
            Ok((way, c2, shut))
        }
    });
    let play = bot.assignment.as_ref().map(|a| a.kind);
    let (verdict, lane, lanes_walked) = match second {
        Err(why) => (Verdict::One(why), None, Arc::new([points1, Vec::new()])),
        Ok((way, c2, shut)) => {
            let points2 = way.points(tactics);
            let stretches = [own_stretch(&points1, &points2, away), own_stretch(&points2, &points1, away)];
            let bodies: Vec<(Place, Vec3)> = view.team(me.team).map(|p| (p.place, p.feet)).collect();
            let mates = lanes.mates_on(bot.place, [&stretches[0], &stretches[1]], away, walk.time, keep_ms, &bodies);
            let (number, covered) = (number(bot.life_key, walk.goal), cover(mates));
            let (odds, second) = match covered {
                Some(second) => (if second { 1.0 } else { 0.0 }, second),
                None => {
                    let odds = odds(c2 / c1.max(0.1), band, mates.died, death);
                    (odds, number < odds)
                }
            };
            let verdict = Verdict::Two { c2, mates, cover: covered.is_some(), odds, number, second, swapped };
            (verdict, second.then_some((way, shut)), Arc::new([points1, points2]))
        }
    };
    bot.nav_log.push(super::nav_log::Kind::Lane, walk.time, line(play, walk.goal, c1, &verdict));
    let cap = Cap { metres: band * c1, leaps: leaps1 };
    match lane {
        Some((way, shut)) => {
            let room = Room { regions: room(tactics, &way, &shut, walk.open), cap };
            bot.features.lane.drawn(walk.goal, Some((shut, cap)));
            lanes.claim(bot.place, lanes_walked, true, walk.walk_for());
            Some(room)
        }
        None => {
            bot.features.lane.drawn(walk.goal, None);
            lanes.claim(bot.place, lanes_walked, false, walk.walk_for());
            None
        }
    }
}

/// A held L2's room from where the bot is now (`walk.ends`) to the walk's goal now: its way on the regions kept out of `shut`, its
/// cap from L1 there; none with no way there.
pub(super) fn held_room(tactics: &Tactics, shut: &[bool], band: f32, walk: &Walk<'_>) -> Option<Room> {
    let regions = tactics.regions();
    let (from, to) = (regions.of(walk.ends.0), regions.of(walk.ends.1));
    let first = region_way(tactics, from, to, walk.open)?;
    let open = |r: u32| !shut.get(r as usize).copied().unwrap_or(false) && (walk.open)(r);
    let way = region_way(tactics, from, to, open)?;
    let grid = &tactics.world.grid;
    let c1 = first.cost_between(tactics, grid.position(walk.ends.0), grid.position(walk.ends.1));
    Some(Room { regions: room(tactics, &way, shut, walk.open), cap: Cap { metres: band * c1, leaps: first.leaps(tactics) } })
}

/// The leaps a floors' route makes: its legs that are one link of the grid's other than a walk (a jump, a gap's or a drop: the
/// planner's route has a corner at each end of every such link, `path::pull`).
pub fn route_leaps(route: &super::path::Route, grid: &mapdata::WalkGrid) -> u32 {
    use mapdata::grid::LinkKind;
    route.points().windows(2).filter(|w| grid.links(w[0].node).any(|(to, kind)| to == w[1].node && kind != LinkKind::Walk)).count() as u32
}

/// The nav line of an L2 route over its cap: `nav: lane draw: L2 to (x, z) over its cap: L m against C m, J leaps against L1's K,
/// walks L1`.
pub fn over_cap_line(goal: Vec3, length: f32, jumps: u32, cap: Cap) -> String {
    format!(
        "{}lane draw: L2 to ({:.1}, {:.1}) over its cap: {length:.1} m against {:.1} m, {jumps} leaps against L1's {}, walks L1",
        super::nav_log::PREFIX,
        goal.x,
        goal.z,
        cap.metres,
        cap.leaps
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The odds are the humans' bins as measured: each bin's weight is its legs over the first bin's, the cap cuts them, and deaths
    /// weigh a lane down.
    #[test]
    fn the_odds_are_the_humans_bins_within_the_cap_and_fall_with_deaths() {
        let none = [0, 0];
        let p = |w: f32| w / (1.0 + w);
        for (k, &legs) in HUMAN_LEGS.iter().enumerate() {
            let w = legs as f32 / 870.0;
            for ratio in [1.0 + 0.05 * k as f32 + 0.01, 1.0 + 0.05 * (k + 1) as f32 - 0.01] {
                assert!((human_weight(ratio) - w).abs() < 1e-6, "bin {k} at {ratio}");
                assert!((odds(ratio, 1.5, none, 0.4) - p(w)).abs() < 1e-6, "bin {k} at {ratio}");
            }
        }
        assert_eq!(human_weight(1.0), 1.0, "at L1's cost: the first bin");
        assert_eq!(human_weight(0.9), 1.0);
        assert_eq!(human_weight(1.51), 0.0, "past the table");
        // Equal cost: even. The cap cuts the bins past it.
        assert!((odds(1.0, 1.15, none, 0.4) - 0.5).abs() < 1e-6);
        assert!((odds(1.12, 1.15, none, 0.4) - p(529.0 / 870.0)).abs() < 1e-6);
        assert_eq!(odds(1.17, 1.15, none, 0.4), 0.0, "past the cap: never");
        assert!((odds(1.17, 1.3, none, 0.4) - p(295.0 / 870.0)).abs() < 1e-6);
        assert!((odds(1.22, 1.3, none, 0.4) - p(355.0 / 870.0)).abs() < 1e-6, "as measured: more legs at +20-25 % than at +15-20 %");
        // A mate who died on L1 makes L2 likelier, one on L2 less; one on each: as none.
        let alone = odds(1.07, 1.15, none, 0.4);
        assert!(odds(1.07, 1.15, [1, 0], 0.4) > alone && odds(1.07, 1.15, [0, 1], 0.4) < alone);
        assert!((odds(1.07, 1.15, [1, 1], 0.4) - alone).abs() < 1e-6);
    }

    #[test]
    fn the_mates_on_a_lane_now_give_it_and_equal_counts_draw() {
        let m = |now: [u32; 2]| Mates { now, died: [0, 0] };
        assert_eq!(cover(m([0, 0])), None);
        assert_eq!(cover(m([1, 1])), None);
        assert_eq!(cover(m([1, 0])), Some(false));
        assert_eq!(cover(m([1, 2])), Some(true));
        // Deaths do not give a lane: they weigh the draw.
        assert_eq!(cover(Mates { now: [0, 0], died: [3, 0] }), None);
    }

    #[test]
    fn a_life_draws_one_number_a_walk() {
        let b = Vec3::new(200.0, 0.0, 50.0);
        let n = number(7, b);
        assert!((0.0..1.0).contains(&n));
        assert_eq!(n, number(7, b + Vec3::new(1.0, 0.0, 1.0)), "the same goal cell: the same number");
        assert_ne!(n, number(8, b), "another life");
        assert_ne!(n, number(7, Vec3::new(10.0, 0.0, 10.0)), "another goal");
        // Spread: the numbers of many lives are about uniform.
        let below = (0..1000u64).filter(|&k| number(k.wrapping_mul(0x9E37_79B9_7F4A_7C15), b) < 0.5).count();
        assert!((400..600).contains(&below), "{below}");
    }

    #[test]
    fn a_held_lane_lasts_the_walk_and_danger_turns_it_to_l1() {
        let mut hold = LaneHold::default();
        let goal = Vec3::new(100.0, 0.0, 0.0);
        assert!(matches!(hold.replan(goal, false), Replan::Draw));
        assert_eq!(hold.walking(), None);
        let shut = Arc::new(vec![true, false]);
        hold.drawn(goal, Some((shut.clone(), Cap { metres: 115.0, leaps: 0 })));
        assert_eq!(hold.walking(), Some(true));
        // The goal moves, a little or far: L2 held, no draw.
        assert!(matches!(hold.replan(goal + Vec3::new(5.0, 0.0, 0.0), false), Replan::Held(Some(_))));
        assert!(matches!(hold.replan(Vec3::new(0.0, 0.0, 100.0), false), Replan::Held(Some(_))));
        // Endings of the same walk keep it.
        for why in [RouteEnd::Replanned, RouteEnd::Off, RouteEnd::JumpFailed, RouteEnd::NoProgress, RouteEnd::TrapAhead] {
            hold.route_ended(why);
            assert_eq!(hold.walking(), Some(true), "{why:?}");
        }
        // Danger: L1 for the walk, kept after.
        assert!(matches!(hold.replan(goal, true), Replan::Held(None)));
        assert!(matches!(hold.replan(goal, false), Replan::Held(None)));
        assert_eq!(hold.walking(), Some(false));
        // The walk ends: a new draw.
        for why in [RouteEnd::Arrived, RouteEnd::NewAssignment, RouteEnd::EndPlay, RouteEnd::NoGoal, RouteEnd::NoWay] {
            let mut held = hold.clone();
            held.route_ended(why);
            assert!(matches!(held.replan(goal, false), Replan::Draw), "{why:?}");
        }
        // A danger replan with no lane held: L1, held.
        let mut fresh = LaneHold::default();
        assert!(matches!(fresh.replan(goal, true), Replan::Held(None)));
        assert!(matches!(fresh.replan(goal, false), Replan::Held(None)));
    }

    /// Two lanes from (0, 0) to (100, 0): L1 straight, L2 by z 40 from x 20 to 80. A living mate is on the lane its walk's claim
    /// walks, else on the stretch its body stands on; one who died walking a lane counts on it for the window after the death.
    #[test]
    fn mates_are_counted_on_the_lane_they_walk_or_stand_on_and_by_their_deaths() {
        let line = |z: f32| (0..=50).map(|k| Vec3::new(k as f32 * 2.0, 0.0, if (10..=40).contains(&k) { z } else { 0.0 })).collect::<Vec<_>>();
        let (l1, l2) = (line(0.0), line(40.0));
        let (s1, s2) = (own_stretch(&l1, &l2, 15.0), own_stretch(&l2, &l1, 15.0));
        assert!(!s1.is_empty() && !s2.is_empty());
        let both = Arc::new([l1.clone(), l2.clone()]);
        let mut lanes = TeamLanes::default();
        let w = WalkFor { play: None, goal: Vec3::new(100.0, 0.0, 0.0), objective: None };
        lanes.claim(1, both.clone(), false, w);
        lanes.claim(2, both.clone(), true, w);
        lanes.claim(3, both.clone(), true, w);
        let start = Vec3::ZERO;
        let bodies = |places: &[Place]| places.iter().map(|&p| (p, start)).collect::<Vec<_>>();
        let m = |now: [u32; 2], died: [u32; 2]| Mates { now, died };
        let s = [&s1[..], &s2[..]];
        assert_eq!(lanes.mates_on(0, s, 15.0, 0, 15_000, &bodies(&[0, 1, 2, 3])), m([1, 2], [0, 0]));
        assert_eq!(lanes.mates_on(2, s, 15.0, 0, 15_000, &bodies(&[0, 1, 2, 3])), m([1, 1], [0, 0]), "not its own");
        // Mate 3 dies at 2 s (seen by a teammate's tick): died on L2 till 17 s, then dropped.
        lanes.walking(0, None, 2_000, 15_000, |p| p != 3);
        assert_eq!(lanes.mates_on(0, s, 15.0, 17_000, 15_000, &bodies(&[0, 1, 2])), m([1, 1], [0, 1]));
        lanes.walking(0, None, 17_001, 15_000, |_| true);
        assert_eq!(lanes.mates_on(0, s, 15.0, 17_001, 15_000, &bodies(&[0, 1, 2])), m([1, 1], [0, 0]));
        // Its next life's claim does not wipe a death.
        lanes.walking(0, None, 18_000, 15_000, |p| p != 2);
        lanes.claim(2, both.clone(), false, w);
        assert_eq!(lanes.mates_on(0, s, 15.0, 18_000, 15_000, &bodies(&[0, 1, 2])), m([2, 0], [0, 1]));
        // Mate 1's walk ends: by its body now, on L1's own stretch, on L2's, or at the shared start (none).
        lanes.walking(1, None, 19_000, 15_000, |_| true);
        let on = |feet: Vec3| lanes.mates_on(0, s, 15.0, 19_000, 15_000, &[(1, feet)]);
        assert_eq!(on(Vec3::new(50.0, 0.0, 2.0)), m([1, 0], [0, 1]));
        assert_eq!(on(Vec3::new(50.0, 0.0, 38.0)), m([0, 1], [0, 1]));
        assert_eq!(on(start), m([0, 0], [0, 1]));
        // A walk turned to L1 counts on L1.
        lanes.walking(2, Some(false), 19_000, 15_000, |_| true);
        assert_eq!(lanes.mates_on(0, s, 15.0, 19_000, 15_000, &bodies(&[2])), m([1, 0], [0, 1]));
        // A lane along neither stretch: no one's.
        assert_eq!(walks(&line(-60.0)[20..=30], [&s1, &s2], 15.0), None);
    }

    #[test]
    fn the_line_names_the_lanes_and_the_draw() {
        let goal = Vec3::new(1.0, 0.0, 2.0);
        let mates = Mates { now: [1, 1], died: [0, 2] };
        let two = Verdict::Two { c2: 110.0, mates, cover: false, odds: 0.4, number: 0.2, second: true, swapped: false };
        let text = line("Collect", goal, 100.0, &two);
        assert!(
            text.starts_with("nav: lane draw: \"Collect\" to (1.0, 2.0): C1 100.0 m, C2 110.0 m (1.100), mates now L1 1 L2 1, died L1 0 L2 2"),
            "{text}"
        );
        assert!(text.ends_with("p(L2) 0.400, drew 0.200: walks L2 (rank 2), 110.0 m"), "{text}");
        let mates = Mates { now: [2, 0], died: [0, 0] };
        let cover = Verdict::Two { c2: 98.0, mates, cover: true, odds: 0.0, number: 0.2, second: false, swapped: true };
        let text = line("Collect", goal, 100.0, &cover);
        assert!(text.contains("C2 98.0 m (0.980), swapped, mates now L1 2 L2 0") && text.ends_with("with the mates: walks L1 (rank 1), 100.0 m"), "{text}");
        assert!(line("Cut", goal, 80.0, &Verdict::One(MORE_EXPOSED)).ends_with("one lane (more exposed), walks L1"));
    }

    /// On the test field (80 by 40 m, a wall at x 40 from z 0 to 25): the regions' way from (10, 10) to (70, 10) goes by the gap
    /// above the wall, its cost no less than the straight distance and with no leap; with the regions near it shut (the ends' 25 m
    /// open) there is no second way, and a held lane's room is none; with nothing shut the room holds the way's regions.
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
        assert_eq!(way.leaps(&tactics), 0, "a flat field: walk links only");
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
        let walk = Walk { ends: (a, b), goal: grid.position(b), time: 0, open: &open, play: None, objective: None };
        assert!(held_room(&tactics, &shut, 1.15, &walk).is_none());
        let room = held_room(&tactics, &vec![false; regions.len()], 1.15, &walk).unwrap();
        assert!(way.regions.iter().all(|&(r, _)| room.regions[r as usize]));
        assert!((room.cap.metres - 1.15 * between).abs() < 1e-3, "nothing shut: L2 is L1, the cap the band on its cost between the ends");
        assert_eq!(room.cap.leaps, 0);
        // The walk is all ends: no room for a second lane.
        assert!(shut_near(&tactics, &points[..2], [points[0], points[1]], &set).is_none());
    }

    /// A way's leap links are those whose cost is over its middles' distance by a drop's half penalty or more.
    #[test]
    fn a_way_counts_its_leap_links_by_their_penalty() {
        let world = crate::v026g::bots::tactics::tests::field_with(|_| {});
        let tactics = Tactics::new(world, 1.0);
        let grid = &tactics.world.grid;
        let (a, b) = (grid.locate(Vec3::new(10.0, 0.0, 10.0), 1.5).unwrap(), grid.locate(Vec3::new(70.0, 0.0, 10.0), 1.5).unwrap());
        let regions = tactics.regions();
        let mut way = region_way(&tactics, regions.of(a), regions.of(b), |_| true).unwrap();
        assert!(way.regions.len() >= 3);
        let drop = mapdata::grid::LinkKind::Drop.penalty();
        way.regions[1].1 += drop;
        assert_eq!(way.leaps(&tactics), 1, "a drop's penalty");
        way.regions[2].1 += drop / 2.0 - 0.01;
        assert_eq!(way.leaps(&tactics), 1, "under half a drop's penalty: a walk");
        way.regions[2].1 += 0.02;
        assert_eq!(way.leaps(&tactics), 2);
    }

    /// [pincer] A walk to an objective a teammate's walk under way heads for (its route published) pairs with it; the pair's states
    /// are read by each; a teammate gone (its claim ended) is gone. Walks of other objectives or plays do not pair.
    #[test]
    fn two_walks_to_one_objective_pair_and_read_each_other() {
        use super::super::pincer::{Objective, Shared};
        use super::super::plays::PlayKind;
        let mut lanes = TeamLanes::default();
        let lane = Arc::new([vec![Vec3::ZERO, Vec3::new(80.0, 0.0, 0.0)], vec![]]);
        let walk = |play, objective| WalkFor { play: Some(play), goal: Vec3::new(80.0, 0.0, 0.0), objective: Some(objective) };
        lanes.claim(1, lane.clone(), false, walk(PlayKind::Collect, Objective::Battery(3)));
        lanes.claim(4, lane.clone(), false, walk(PlayKind::Escort, Objective::Battery(3)));
        let ahead = Ahead { stage: Vec3::new(75.0, 0.0, 0.0), to_stage_m: 60.0 };
        let same = |o: Objective| o == Objective::Battery(3);
        assert_eq!(lanes.pincer_mate(2, same), None, "its route is not published yet");
        lanes.publish(1, ahead);
        lanes.publish(4, ahead);
        assert_eq!(lanes.pincer_mate(2, same), Some((1, Vec3::new(80.0, 0.0, 0.0), ahead)), "not the escort");
        assert_eq!(lanes.pincer_mate(2, |o| o == Objective::Battery(4)), None);
        lanes.claim(2, lane.clone(), false, walk(PlayKind::Cut, Objective::Battery(3)));
        lanes.pair((1, ahead.stage), (2, Vec3::new(80.0, 0.0, 5.0)), Vec3::new(80.0, 0.0, 0.0));
        assert_eq!(lanes.paired(2).map(|p| (p.mate, p.stage)), Some((1, Vec3::new(80.0, 0.0, 5.0))));
        assert_eq!(lanes.mate_state(2), Shared::Walking);
        assert_eq!(lanes.pincer_mate(3, same), None, "one pincer a walk");
        lanes.share(1, Shared::Staged);
        assert_eq!(lanes.mate_state(2), Shared::Staged);
        lanes.walking(1, None, 1_000, 15_000, |_| true);
        assert_eq!(lanes.mate_state(2), Shared::Gone, "its teammate's walk ended");
        assert_eq!(lanes.mate_state(9), Shared::Gone, "no pincer");
    }

    /// [pincer] A pincer's walk heads for its stage until it goes in; an arrival at the stage keeps the walk, any other end of the walk
    /// ends the pincer too.
    #[test]
    fn a_pincers_walk_heads_for_its_stage_until_it_goes_in() {
        use super::super::pincer::{Phase, Pincer};
        let goal = Vec3::new(80.0, 0.0, 0.0);
        let stage = Vec3::new(75.0, 0.0, 3.0);
        let mut hold = LaneHold::default();
        assert_eq!(hold.goal(goal), goal);
        hold.start_pincer(goal, Pincer { mate: 1, stage, centre: goal, phase: Phase::ToStage { checked_m: None, own: None }, flank: true, since: 0 });
        assert_eq!(hold.goal(goal), stage);
        assert!(matches!(hold.replan(stage, false), Replan::Held(None)), "no lane drawn: the stage is its way");
        hold.route_ended(RouteEnd::Arrived);
        assert!(hold.pincer().is_some() && hold.walking().is_some(), "at the stage: the walk goes on");
        hold.pincer_phase(Phase::Staged { since: 5 });
        assert_eq!(hold.goal(goal), stage);
        hold.pincer_phase(Phase::In);
        assert_eq!(hold.goal(goal), goal);
        hold.route_ended(RouteEnd::Arrived);
        assert!(hold.pincer().is_none() && hold.walking().is_none(), "in and there: the walk is over");
        hold.start_pincer(goal, Pincer { mate: 1, stage, centre: goal, phase: Phase::ToStage { checked_m: None, own: None }, flank: true, since: 0 });
        hold.route_ended(RouteEnd::NewAssignment);
        assert!(hold.pincer().is_none());
        // A refusal holds for its goal, past the walk's end (a bot at its goal stands there with no route), not for another goal.
        hold.pincer_refused(goal);
        assert!(hold.refused_for(goal + Vec3::new(3.0, 0.0, 0.0), 15.0) && !hold.refused_for(Vec3::ZERO, 15.0));
        hold.route_ended(RouteEnd::Arrived);
        assert!(hold.refused_for(goal, 15.0));
    }

    #[test]
    fn a_held_second_lane_is_known_for_its_goal_and_capped_by_length_and_jumps() {
        let mut hold = LaneHold::default();
        let goal = Vec3::new(100.0, 0.0, 0.0);
        assert!(!hold.second_for(goal, 15.0));
        let cap = Cap { metres: 115.0, leaps: 1 };
        hold.drawn(goal, Some((Arc::new(vec![true]), cap)));
        assert!(hold.second_for(goal + Vec3::new(10.0, 0.0, 0.0), 15.0) && !hold.second_for(Vec3::ZERO, 15.0));
        // Its cap: a route over it, by length or by jumps, is named; one within it not; the cap from where the bot is now replaces it.
        assert_eq!(hold.over_cap(goal, 120.0, 0, 15.0), Some(cap));
        assert_eq!(hold.over_cap(goal, 110.0, 2, 15.0), Some(cap));
        assert_eq!(hold.over_cap(goal, 110.0, 1, 15.0), None);
        assert_eq!(hold.over_cap(Vec3::ZERO, 120.0, 0, 15.0), None, "another goal");
        let near = Cap { metres: 90.0, leaps: 0 };
        hold.capped(near);
        assert_eq!(hold.over_cap(goal, 100.0, 0, 15.0), Some(near));
        assert!(over_cap_line(goal, 100.0, 1, near).ends_with("lane draw: L2 to (100.0, 0.0) over its cap: 100.0 m against 90.0 m, 1 leaps against L1's 0, walks L1"));
        hold.drop_lane();
        assert!(!hold.second_for(goal, 15.0));
    }
}
