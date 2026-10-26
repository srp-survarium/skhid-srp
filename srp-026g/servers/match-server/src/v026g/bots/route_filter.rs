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
    /// The highest jump link the body takes, metres (hard): a body that
    /// cannot sprint clears [`mapdata::grid::STANDING_JUMP_M`] at most.
    max_jump_rise: Option<f32>,
}

impl std::fmt::Debug for RouteFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RouteFilter")
            .field("landings", &self.landings.len())
            .field("spots", &self.spots.len())
            .field("floors", &self.floors.is_some())
            .field("regions", &self.regions.is_some())
            .field("max_jump_rise", &self.max_jump_rise)
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
        !self.spots.is_empty()
    }

    /// This filter without its soft part.
    pub fn without_soft(&self) -> Self {
        RouteFilter {
            spots: Vec::new(),
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
}
