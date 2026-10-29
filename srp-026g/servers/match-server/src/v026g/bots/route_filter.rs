//! What a route search keeps out besides the tactics' own tests.
//!
//! One owned value for every search of a body (the plain one, its second
//! through the spots, the crossing's, a zone's way out), so a search that
//! goes on over several polls ([`super::search::Pending`]) is the same
//! search it started as, and a new rule (a region set, a zone's cost) has
//! one place to join.
//!
//! The filter has a *hard* part, the facts of the body that no search may
//! ignore (the landings of jumps it gave up, [`super::stuck::Stuck::shut_landings`];
//! floors and regions a rule shuts), and a *soft* part, a preference the
//! plain search keeps and a second search drops when it finds no way or a
//! much longer one (the spots the body got stuck at,
//! [`super::stuck::Stuck::avoided_for`]).
//!
//! The floor and region parts agree: a region the filter shuts has all its
//! floors shut by the same rule ([`RouteFilter::shutting_regions`]), so the
//! whole-grid stage of a search, which tests floors only, cannot walk a
//! region the region stage refused; and floors shut do not shut their
//! region, which stays open while any of its floors may be.

use std::sync::Arc;

use mapdata::regions::Regions;
use mapdata::{NodeId, Vec3, WalkGrid};

use super::stuck;
use super::threat::{self, Covered};

type FloorRule = Arc<dyn Fn(NodeId) -> bool + Send + Sync>;
type RegionRule = Arc<dyn Fn(u32) -> bool + Send + Sync>;

/// What a search keeps out ([module](self)).
#[derive(Clone, Default)]
pub struct RouteFilter {
    /// Landings of jumps the body gave up: floors on them are shut (hard).
    landings: Vec<Vec3>,
    /// Spots the body got stuck at: floors near them are kept out (soft, as
    /// `soft` says).
    spots: Vec<Vec3>,
    /// A rule over floors, true for open (hard).
    floors: Option<FloorRule>,
    /// A rule over floors, true for open (soft, as `soft` says): the floors
    /// of the traps the body noticed ([`super::trap_react`]).
    soft_floors: Option<FloorRule>,
    /// A rule over regions, true for open (hard).
    regions: Option<RegionRule>,
    /// The highest jump link the body takes, metres (hard): a body that
    /// cannot sprint clears [`mapdata::grid::STANDING_JUMP_M`] at most.
    max_jump_rise: Option<f32>,
    /// The regions a held threat spot sees (soft, K1): kept out but near
    /// the route's ends.
    threat: Option<Arc<Threat>>,
    /// What the soft part (`spots`, `soft_floors`) does to a floor: shuts it,
    /// or costs it in a weighed search ([`Self::weighing_threat`]).
    soft: Soft,
}

/// What a filter's soft part does to the floors it keeps out.
#[derive(Clone, Debug, Default)]
enum Soft {
    /// Shut (a second search without the soft part goes through them).
    #[default]
    Shut,
    /// Costed [`SOFT_COST`] times, with a covered region's floor costed by
    /// the weights' lane cost (K1 A2's fine pass).
    Costed(Arc<Weights>),
}

/// What a weighed search costs a floor ([`RouteFilter::weight`]): a covered
/// region's floor `lane` times its length, but near the route's ends; a
/// floor the soft part shuts (traps it noticed, spots it got stuck at)
/// [`SOFT_COST`] times.
#[derive(Debug)]
pub struct Weights {
    covered: Arc<Covered>,
    ends: [Vec3; 2],
    lane: f32,
}

/// What a floor the soft part keeps out costs a weighed search, times its
/// length: the soft part's "only if there is no other way" as a cost, as
/// Valve's `func_nav_avoid` raises an area's cost (24 times there) rather
/// than shuts it. GUESS: 10, more than any lane (`ThreatTuning::lane_cost`
/// 3 makes 4), so a trap is walked round before a lane is.
pub const SOFT_COST: f32 = 10.0;

/// The regions held threat spots see ([`Covered`]) and the points of a
/// route's ends, round which nothing is kept out ([`threat::FREE_M`]).
#[derive(Debug)]
pub struct Threat {
    covered: Arc<Covered>,
    /// The route's start and goal.
    ends: [Vec3; 2],
    /// Covered regions with a middle near an end: open.
    free_regions: Vec<u32>,
}

impl Threat {
    fn floor_open(&self, grid: &WalkGrid, node: NodeId) -> bool {
        !self.covered.floor(node) || {
            let at = grid.position(node);
            self.ends.iter().any(|end| end.distance_xz(at) < threat::FREE_M)
        }
    }

    fn region_open(&self, region: u32) -> bool {
        !self.covered.region(region) || self.free_regions.contains(&region)
    }
}

impl std::fmt::Debug for RouteFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RouteFilter")
            .field("landings", &self.landings.len())
            .field("spots", &self.spots.len())
            .field("floors", &self.floors.is_some())
            .field("soft_floors", &self.soft_floors.is_some())
            .field("regions", &self.regions.is_some())
            .field("max_jump_rise", &self.max_jump_rise)
            .field("threat", &self.threat.is_some())
            .field("soft", &self.soft)
            .finish()
    }
}

impl RouteFilter {
    /// Keeps nothing out.
    pub fn open() -> Self {
        Self::default()
    }

    /// Keeps out the floors on `landings`, the landings of jumps the body
    /// gave up.
    pub fn shutting_landings(landings: Vec<Vec3>) -> Self {
        RouteFilter {
            landings,
            ..Self::default()
        }
    }

    /// With the floors near `spots` kept out as a preference (soft).
    pub fn avoiding(mut self, spots: Vec<Vec3>) -> Self {
        self.spots = spots;
        self
    }

    /// With the floors `open` gives false kept out as a preference (soft).
    pub fn avoiding_floors(mut self, open: impl Fn(NodeId) -> bool + Send + Sync + 'static) -> Self {
        self.soft_floors = Some(Arc::new(open));
        self
    }

    /// With the regions the team's held threat spots see kept out as a
    /// preference (soft), but for the floors within [`threat::FREE_M`] of
    /// the route's ends `from` and `to`, and the covered regions
    /// `free_regions` (with a middle near an end). Nothing when none is
    /// covered.
    pub fn avoiding_threat(mut self, covered: Arc<Covered>, from: Vec3, to: Vec3, free_regions: Vec<u32>) -> Self {
        self.threat = (!covered.is_empty()).then(|| {
            Arc::new(Threat {
                covered,
                ends: [from, to],
                free_regions,
            })
        });
        self
    }

    /// A weighed search (K1 A2, the owner's best-practice rule): a floor of
    /// a region `covered` costs `1 + lane_cost` times its length, but within
    /// [`threat::FREE_M`] of the route's ends `from` and `to`; the soft part
    /// (the spots and floors it keeps out) shuts nothing and costs
    /// [`SOFT_COST`] instead; the hard part is as it was. The search reads
    /// it as a weight on each link ([`Self::weight`]; Detour's
    /// `dtQueryFilter` area costs, Valve's path cost functor). The regions'
    /// corridor of the search is not weighed: the coarse pass is
    /// `threat::costed_corridor`, whose room the search is kept to
    /// ([`Self::keeping_to`]), and this is the fine pass in it (HPA*'s
    /// abstract path, then the refined one).
    pub fn weighing_threat(mut self, covered: Arc<Covered>, from: Vec3, to: Vec3, lane_cost: f32) -> Self {
        self.soft = Soft::Costed(Arc::new(Weights { covered, ends: [from, to], lane: 1.0 + lane_cost.max(0.0) }));
        self
    }

    /// Whether the search weighs its floors ([`Self::weighing_threat`]).
    pub fn weighs(&self) -> bool {
        matches!(self.soft, Soft::Costed(_))
    }

    /// What a link into floor `node` costs times its length, at least 1:
    /// 1 unless the filter weighs ([`Self::weighing_threat`]).
    pub fn weight(&self, grid: &WalkGrid, node: NodeId) -> f32 {
        let Soft::Costed(w) = &self.soft else {
            return 1.0;
        };
        let at = || grid.position(node);
        let soft = !self.spots.is_empty() && self.spots.iter().any(|&spot| stuck::near(spot, at()))
            || self.soft_floors.as_ref().is_some_and(|f| !f(node));
        if soft {
            return SOFT_COST;
        }
        if w.covered.floor(node) && !w.ends.iter().any(|end| end.distance_xz(at()) < threat::FREE_M) {
            w.lane
        } else {
            1.0
        }
    }

    /// Keeps the search to the regions `room` gives true (by region) and
    /// their floors (hard): the room of a way weighed round the threat
    /// spots' lanes (`threat::costed_corridor`, K1 A2). Floors and regions
    /// agree, so no stage of the search walks out of the room.
    pub fn keeping_to(mut self, room: Arc<Vec<bool>>, region_of: threat::RegionOf) -> Self {
        let inside: RegionRule = Arc::new(move |region: u32| room.get(region as usize).copied().unwrap_or(false));
        let (rule, of) = (inside.clone(), region_of);
        let before = self.floors.take();
        self.floors = Some(Arc::new(move |node| rule(of(node)) && before.as_ref().is_none_or(|f| f(node))));
        let before = self.regions.take();
        self.regions = Some(Arc::new(move |region| inside(region) && before.as_ref().is_none_or(|f| f(region))));
        self
    }

    /// Keeps out the floors `open` gives false.
    pub fn with_floors(mut self, open: impl Fn(NodeId) -> bool + Send + Sync + 'static) -> Self {
        self.floors = Some(Arc::new(open));
        self
    }

    /// With the jump links above `rise` metres kept out (hard): the
    /// body cannot take them.
    pub fn with_max_jump_rise(mut self, rise: f32) -> Self {
        self.max_jump_rise = Some(rise);
        self
    }

    /// The highest jump link the body takes, if limited.
    pub fn max_jump_rise(&self) -> Option<f32> {
        self.max_jump_rise
    }

    /// Keeps out the regions `shut` gives true for, and every floor of
    /// them.
    pub fn shutting_regions(
        mut self,
        regions: Arc<Regions>,
        shut: impl Fn(u32) -> bool + Send + Sync + 'static,
    ) -> Self {
        let shut: Arc<dyn Fn(u32) -> bool + Send + Sync> = Arc::new(shut);
        let (of, rule) = (regions, shut.clone());
        let before = self.floors.take();
        self.floors = Some(Arc::new(move |node| {
            !rule(of.of(node)) && before.as_ref().is_none_or(|f| f(node))
        }));
        let before = self.regions.take();
        self.regions = Some(Arc::new(move |region| {
            !shut(region) && before.as_ref().is_none_or(|f| f(region))
        }));
        self
    }

    /// Whether any of the filter is soft (a second search without it is
    /// worth making).
    pub fn has_soft(&self) -> bool {
        !self.spots.is_empty() || self.threat.is_some() || self.soft_floors.is_some()
    }

    /// This filter without its soft part.
    pub fn without_soft(&self) -> Self {
        RouteFilter {
            spots: Vec::new(),
            threat: None,
            soft: Soft::Shut,
            soft_floors: None,
            ..self.clone()
        }
    }

    /// Whether floor `node` is open to a search (hard and soft; a weighed
    /// search costs the soft part instead, [`Self::weight`]).
    pub fn floor_open(&self, grid: &WalkGrid, node: NodeId) -> bool {
        if let Soft::Costed(_) = self.soft {
            return self.hard_open(grid, node) && self.threat.as_ref().is_none_or(|t| t.floor_open(grid, node));
        }
        self.hard_open(grid, node)
            && (self.spots.is_empty() || {
                let at = grid.position(node);
                !self.spots.iter().any(|&spot| stuck::near(spot, at))
            })
            && self.threat.as_ref().is_none_or(|t| t.floor_open(grid, node))
            && self.soft_floors.as_ref().is_none_or(|f| f(node))
    }

    /// Whether floor `node` is open to a search that drops the soft part
    /// (a crossing's, [`super::crossing`]).
    pub fn hard_open(&self, grid: &WalkGrid, node: NodeId) -> bool {
        (self.landings.is_empty() || {
            let at = grid.position(node);
            !self.landings.iter().any(|&landing| stuck::on_landing(landing, at))
        }) && self.floors.as_ref().is_none_or(|f| f(node))
    }

    /// Whether region `region` is open to the region stage of a search.
    pub fn region_open(&self, region: u32) -> bool {
        self.regions.as_ref().is_none_or(|f| f(region))
            && self.threat.as_ref().is_none_or(|t| t.region_open(region))
    }

    /// The landings the body gave up, for the tests that read them.
    pub fn landings(&self) -> &[Vec3] {
        &self.landings
    }

    /// Both filters at once: a floor or region open only if open to both;
    /// the soft part of both.
    pub fn and(mut self, other: &Self) -> Self {
        self.landings.extend(other.landings.iter().copied());
        self.spots.extend(other.spots.iter().copied());
        self.max_jump_rise = match (self.max_jump_rise, other.max_jump_rise) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        if self.threat.is_none() {
            self.threat = other.threat.clone();
        }
        if let Soft::Shut = self.soft {
            self.soft = other.soft.clone();
        }
        if let Some(theirs) = other.soft_floors.clone() {
            let mine = self.soft_floors.take();
            self.soft_floors = Some(Arc::new(move |node| {
                mine.as_ref().is_none_or(|f| f(node)) && theirs(node)
            }));
        }
        if let Some(theirs) = other.floors.clone() {
            let mine = self.floors.take();
            self.floors = Some(Arc::new(move |node| {
                mine.as_ref().is_none_or(|f| f(node)) && theirs(node)
            }));
        }
        if let Some(theirs) = other.regions.clone() {
            let mine = self.regions.take();
            self.regions = Some(Arc::new(move |region| {
                mine.as_ref().is_none_or(|f| f(region)) && theirs(region)
            }));
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v026g::bots::tactics::tests::field_with;

    /// A grid and its regions to test on.
    fn field() -> (Arc<mapdata::World>, Arc<Regions>) {
        let world = field_with(|_| {});
        let regions = Arc::new(Regions::build(&world.grid, None));
        (world, regions)
    }

    fn nodes(grid: &WalkGrid) -> impl Iterator<Item = NodeId> {
        0..grid.len() as NodeId
    }

    #[test]
    fn soft_floors_are_kept_out_by_the_plain_search_and_dropped_with_the_soft_part() {
        let (world, _) = field();
        let one = nodes(&world.grid).nth(10).unwrap();
        let filter = RouteFilter::open().avoiding_floors(move |n| n != one);
        assert!(filter.has_soft());
        assert!(!filter.floor_open(&world.grid, one));
        assert!(filter.floor_open(&world.grid, one + 1));
        assert!(filter.hard_open(&world.grid, one));
        let hard = filter.without_soft();
        assert!(!hard.has_soft() && hard.floor_open(&world.grid, one));
        let both = filter.and(&RouteFilter::open().avoiding_floors(move |n| n != one + 1));
        assert!(!both.floor_open(&world.grid, one) && !both.floor_open(&world.grid, one + 1));
    }

    #[test]
    fn an_open_filter_keeps_nothing_out() {
        let (world, _) = field();
        let filter = RouteFilter::open();
        assert!(!filter.has_soft());
        assert!(nodes(&world.grid).all(|n| filter.floor_open(&world.grid, n)));
        assert!(filter.region_open(0) && filter.region_open(7));
    }

    #[test]
    fn two_filters_keep_the_lower_jump() {
        let high = RouteFilter::open().with_max_jump_rise(1.0);
        let low = RouteFilter::open().with_max_jump_rise(0.8);
        assert_eq!(RouteFilter::open().max_jump_rise(), None);
        assert_eq!(high.clone().and(&RouteFilter::open()).max_jump_rise(), Some(1.0));
        assert_eq!(RouteFilter::open().and(&low).max_jump_rise(), Some(0.8));
        assert_eq!(high.and(&low).max_jump_rise(), Some(0.8));
        // Not a soft part: the second search without the spots keeps it.
        assert_eq!(low.without_soft().max_jump_rise(), Some(0.8));
    }

    #[test]
    fn landings_are_hard_and_spots_soft() {
        let (world, _) = field();
        let grid = &world.grid;
        let node = nodes(grid).next().unwrap();
        let at = grid.position(node);
        let filter = RouteFilter::shutting_landings(vec![at]);
        assert!(!filter.floor_open(grid, node) && !filter.hard_open(grid, node));
        let spots = RouteFilter::open().avoiding(vec![at]);
        assert!(spots.has_soft());
        assert!(!spots.floor_open(grid, node), "kept clear of");
        assert!(spots.hard_open(grid, node), "but not a hard fact");
        let through = spots.without_soft();
        assert!(!through.has_soft() && through.floor_open(grid, node));
        // Both: the second search keeps the hard part.
        let both = filter.clone().and(&spots);
        assert!(!both.without_soft().floor_open(grid, node));
        assert_eq!(both.landings().len(), 1);
    }

    /// A region the filter shuts has every floor of it shut; the floors it
    /// shuts do not shut their region.
    #[test]
    fn floors_and_regions_agree() {
        let (world, regions) = field();
        let grid = &world.grid;
        let shut_region = regions.of(nodes(grid).next().unwrap());
        let filter = RouteFilter::open().shutting_regions(regions.clone(), move |r| r == shut_region);
        assert!(!filter.region_open(shut_region));
        for node in nodes(grid) {
            assert_eq!(
                filter.floor_open(grid, node),
                regions.of(node) != shut_region,
                "floor {node:?}"
            );
        }
        let one = nodes(grid).next().unwrap();
        let floors = RouteFilter::open().with_floors(move |n| n != one);
        assert!(!floors.floor_open(grid, one));
        assert!(floors.region_open(regions.of(one)), "the region stays open");
        // And of the two shuts both.
        let both = filter.and(&floors);
        assert!(!both.floor_open(grid, one) && !both.region_open(shut_region));
    }

    /// A weighed filter (K1 A2): a covered floor costs `1 + lane_cost` but
    /// near the ends, a soft-shut floor costs `SOFT_COST` and stays open;
    /// the weighed search goes round a costly strip the plain one crosses.
    #[test]
    fn a_weighed_filter_costs_the_lanes_and_the_soft_part_and_the_search_goes_round() {
        let (world, regions) = field();
        let grid = &world.grid;
        // The strip x 20-30, z 0-20 covered (the wall at x 40 leaves the way
        // round by z 25-40).
        let strip: Vec<u32> = (0..regions.len() as u32)
            .filter(|&r| {
                let m = grid.position(regions.middle(r));
                (20.0..30.0).contains(&m.x) && m.z < 20.0
            })
            .collect();
        let of = regions.clone();
        let covered = Arc::new(Covered::build(strip.clone(), regions.len(), Arc::new(move |n| of.of(n))));
        let (a, b) = (Vec3::new(5.0, 0.0, 10.0), Vec3::new(38.0, 0.0, 10.0));
        let (na, nb) = (grid.locate(a, 1.5).unwrap(), grid.locate(b, 1.5).unwrap());
        // A covered floor far from both ends: the middle of the strip's region
        // nearest (25, 10).
        let mid = |r: u32| grid.position(regions.middle(r));
        let r = *strip.iter().min_by(|x, y| mid(**x).distance_xz(Vec3::new(25.0, 0.0, 10.0)).total_cmp(&mid(**y).distance_xz(Vec3::new(25.0, 0.0, 10.0)))).unwrap();
        let inside = regions.middle(r);
        let plain = RouteFilter::open();
        assert!(!plain.weighs() && plain.weight(grid, inside) == 1.0);
        let weighed = RouteFilter::open().weighing_threat(covered.clone(), a, b, 3.0);
        assert!(weighed.weighs());
        assert_eq!(weighed.weight(grid, inside), 4.0);
        assert_eq!(weighed.weight(grid, na), 1.0, "the start's own ground is free");
        // The soft part: open, at its cost.
        let soft = RouteFilter::open().avoiding_floors(move |n| n != inside).weighing_threat(covered.clone(), a, b, 3.0);
        assert!(soft.floor_open(grid, inside));
        assert_eq!(soft.weight(grid, inside), SOFT_COST);
        // The searches: the plain one crosses the strip, a costly one goes round.
        let search = |filter: &RouteFilter| {
            let weight = |n| filter.weight(grid, n);
            let mut iters = usize::MAX;
            crate::v026g::bots::path::plan_start(
                grid,
                &regions,
                na,
                nb,
                |n| filter.floor_open(grid, n),
                |r| filter.region_open(r),
                None,
                filter.weighs().then_some(&weight as &dyn Fn(NodeId) -> f32),
                &mut iters,
            )
            .ok()
            .flatten()
            .unwrap()
        };
        let in_strip = |route: &crate::v026g::bots::path::Route| {
            route.points().windows(2).any(|w| {
                (0..=10).any(|k| {
                    let p = w[0].at + (w[1].at - w[0].at) * (k as f32 / 10.0);
                    covered.exposed_at(grid, p) && p.distance_xz(a) > threat::FREE_M && p.distance_xz(b) > threat::FREE_M
                })
            })
        };
        assert!(in_strip(&search(&plain)), "the plain route crosses the strip");
        // The two passes (K1 A2): the costed regions' corridor gives the room,
        // the weighed floor search runs in it. The regions' own corridor of
        // `plan_start` is not weighed: alone, the costly strip is crossed at
        // its cost when the corridor runs through it.
        let free = [regions.of(na), regions.of(nb)];
        let room = threat::costed_corridor(grid, &regions, regions.of(na), regions.of(nb), &covered, &free, 30.0, |_| true).unwrap();
        let of = regions.clone();
        let round = search(
            &RouteFilter::open()
                .keeping_to(Arc::new(room), Arc::new(move |n| of.of(n)))
                .weighing_threat(covered.clone(), a, b, 30.0),
        );
        assert!(!in_strip(&round), "{round:?}");
    }

    /// The regions a held threat spot sees are kept out as a preference,
    /// but for the ends' own ground; a second search drops them.
    #[test]
    fn a_threats_covered_regions_are_soft_and_the_ends_stay_open() {
        let (world, regions) = field();
        let grid = &world.grid;
        let first = nodes(grid).next().unwrap();
        let covered_region = regions.of(first);
        let covered = Arc::new(Covered::build(vec![covered_region], regions.len(), { let regions = regions.clone(); Arc::new(move |n| regions.of(n)) }));
        let far = Vec3::new(1_000.0, 0.0, 1_000.0);
        let filter = RouteFilter::open().avoiding_threat(covered.clone(), far, far, vec![]);
        assert!(filter.has_soft());
        assert!(!filter.region_open(covered_region));
        for node in nodes(grid) {
            assert_eq!(filter.floor_open(grid, node), regions.of(node) != covered_region);
            assert!(filter.hard_open(grid, node), "not a hard fact");
        }
        // An end on the covered ground keeps the floors within 10 m of it
        // open, and a free region is open.
        let at = grid.position(first);
        let near = RouteFilter::open().avoiding_threat(covered.clone(), at, far, vec![covered_region]);
        assert!(near.floor_open(grid, first) && near.region_open(covered_region));
        // The second search is the plain one.
        let through = filter.without_soft();
        assert!(!through.has_soft() && through.region_open(covered_region));
        // Nothing covered: no soft part at all.
        let none = RouteFilter::open().avoiding_threat(Arc::default(), far, far, vec![]);
        assert!(!none.has_soft());
        // Joined with another filter, the threat stays.
        assert!(!RouteFilter::open().and(&filter).region_open(covered_region));
    }
}
