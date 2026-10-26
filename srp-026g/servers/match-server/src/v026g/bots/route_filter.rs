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
    /// Spots the body got stuck at: floors near them are shut (soft).
    spots: Vec<Vec3>,
    /// A rule over floors, true for open (hard).
    floors: Option<FloorRule>,
    /// A rule over regions, true for open (hard).
    regions: Option<RegionRule>,
    /// The regions a held threat spot sees (soft, K1): kept out but near
    /// the route's ends.
    threat: Option<Arc<Threat>>,
}

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
            .field("regions", &self.regions.is_some())
            .field("threat", &self.threat.is_some())
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

    /// Keeps out the floors `open` gives false.
    pub fn with_floors(mut self, open: impl Fn(NodeId) -> bool + Send + Sync + 'static) -> Self {
        self.floors = Some(Arc::new(open));
        self
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
        !self.spots.is_empty() || self.threat.is_some()
    }

    /// This filter without its soft part.
    pub fn without_soft(&self) -> Self {
        RouteFilter {
            spots: Vec::new(),
            threat: None,
            ..self.clone()
        }
    }

    /// Whether floor `node` is open to a search (hard and soft).
    pub fn floor_open(&self, grid: &WalkGrid, node: NodeId) -> bool {
        self.hard_open(grid, node)
            && (self.spots.is_empty() || {
                let at = grid.position(node);
                !self.spots.iter().any(|&spot| stuck::near(spot, at))
            })
            && self.threat.as_ref().is_none_or(|t| t.floor_open(grid, node))
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
        if self.threat.is_none() {
            self.threat = other.threat.clone();
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
    fn an_open_filter_keeps_nothing_out() {
        let (world, _) = field();
        let filter = RouteFilter::open();
        assert!(!filter.has_soft());
        assert!(nodes(&world.grid).all(|n| filter.floor_open(&world.grid, n)));
        assert!(filter.region_open(0) && filter.region_open(7));
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
